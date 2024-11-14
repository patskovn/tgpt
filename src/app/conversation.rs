use std::time::Duration;

use crate::uiutils::moves;
use crate::uiutils::reflow::LineComposer;
use crate::uiutils::reflow::WordWrapper;
use crate::uiutils::text::StyledParagraph;
use crate::uiutils::text::StyledText;
use crate::utils::chat_renderer::parse_markdown;
use crate::utils::chat_renderer::IntermediateMarkdownPassResult;
use chatgpt::{
    prelude::Conversation,
    types::{ChatMessage, ResponseChunk},
};
use clipboard::ClipboardContext;
use clipboard::ClipboardProvider;
use derive_new::new;
use futures::StreamExt;
use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::{self, Event, KeyModifiers};
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::BorderType;
use ratatui::{
    layout::{Position, Rect, Size},
    style::{Style, Stylize},
    widgets::{block::Title, Block, Borders, Paragraph, Widget, Wrap},
    Frame,
};
use tca::ActionSender;
use tca::Effect;
use tui_scrollview::ScrollView;

use crate::{
    gpt::openai::{Api, ChatGPTConfiguration},
    scroll_view,
};

use super::chat::CurrentFocus;
use super::chat::SharedFocus;
use super::conversation_list::load_metadata;
use super::conversation_list::save_metadata;
use super::conversation_list::ChatHistory;
use super::conversation_list::ConversationItem;

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct ScrollViewDiementions {
    scroll_size: Size,
    frame_size: Size,
}

impl ScrollViewDiementions {
    fn ensure_within_bounds(&self, offset: Position) -> Position {
        Position {
            x: 0,
            y: offset.y.min(
                self.scroll_size
                    .height
                    .saturating_sub(self.frame_size.height),
            ),
        }
    }
}

#[derive(Debug, Clone, new)]
pub struct DisplayableMessage {
    original: ChatMessage,
    display: Vec<StyledParagraph>,
}

impl PartialEq for DisplayableMessage {
    fn eq(&self, other: &Self) -> bool {
        self.original == other.original
    }
}

impl DisplayableMessage {
    #[allow(dead_code)]
    fn from(text: &str) -> Self {
        Self {
            original: ChatMessage {
                role: chatgpt::types::Role::User,
                content: text.to_owned(),
            },
            display: IntermediateMarkdownPassResult::into_paragraphs(parse_markdown(
                text.to_string(),
            )),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct State {
    pub id: ConversationItem,
    pub cursor: CursorPosition,
    pub selection: Option<Selection>,
    pub config: ChatGPTConfiguration,
    pub history: Vec<DisplayableMessage>,
    pub partial: Vec<DisplayableMessage>,
    pub scroll_state: scroll_view::State,
    pub scroll_view_dimentions: Option<ScrollViewDiementions>,
    pub is_streaming: bool,
    pub tooltip: Option<Tooltip>,
    pub current_focus: SharedFocus,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, new)]
pub struct CursorPosition {
    row: usize,
    col: usize,
}

impl Ord for CursorPosition {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.row.cmp(&other.row).then(self.col.cmp(&other.col))
    }
}

impl PartialOrd for CursorPosition {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, PartialEq, Clone, new)]
pub struct ConcreteSelection<Idx> {
    start: Idx,
    range: std::ops::RangeInclusive<Idx>,
}

pub type LineSelection = ConcreteSelection<usize>;
pub type CharSelection = ConcreteSelection<CursorPosition>;

#[derive(Debug, PartialEq, Clone)]
pub enum Selection {
    Line(LineSelection),
    Char(CharSelection),
}

#[derive(Debug, PartialEq, Clone, new)]
pub struct Tooltip {
    kind: TooltipKind,
    text: String,
}

#[derive(Debug, PartialEq, Clone)]
enum TooltipKind {
    Success,
    Error,
}

#[allow(dead_code)]
const TEST: &str = "Here's a simple \"Hello, world!\" program in Rust:\n\n```rust\nfn main() {\n    println!(\"Hello, world!\");\n}\n```\n\nTo run it, save the code in a file named `main.rs` and use the command `cargo run` or `rustc main.rs` followed by `./main`.";

const CONVERSATION_SUMMARY: &str = "Read the following conversation history and create a brief, 2-4 word title that captures the main topic or purpose of the discussion. Ensure the title is clear, specific, and reflects the unique focus of the conversation. Avoid general terms, and keep it concise. Do not reply with any follow up questions. Just give me the answer based on what was already here.";

impl State {
    pub fn new(
        id: ConversationItem,
        config: ChatGPTConfiguration,
        current_focus: SharedFocus,
        history: Vec<ChatMessage>,
    ) -> Self {
        Self {
            id,
            cursor: CursorPosition::new(0, 0),
            selection: Default::default(),
            config,
            history: history
                .into_iter()
                .map(|msg| {
                    let markdown = parse_markdown(msg.content.clone());
                    let parahraphs = IntermediateMarkdownPassResult::into_paragraphs(markdown);
                    DisplayableMessage::new(msg, parahraphs)
                })
                .collect(),
            partial: Default::default(),
            scroll_state: Default::default(),
            scroll_view_dimentions: Default::default(),
            is_streaming: false,
            tooltip: None,
            current_focus,
        }
    }
}

#[derive(Debug)]
pub enum Action {
    Event(Event),
    NewMessage(String),
    Move(moves::Action),
    ScrollViewDimentionsChanged(ScrollViewDiementions),
    ScrollOffsetChanged(Position),
    BeganStreaming,
    StoppedStreaming,
    UpdateConversationTitle(String),
    Delegated(Delegated),
    CommitMessage(ChatMessage),
    UpdatePartial(Vec<ChatMessage>),
    SetTooltip(Option<Tooltip>),
    ScheduleTooltip(Tooltip),
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
    ConversationTitleUpdated,
}

pub struct Feature {}

impl Feature {
    fn total_lines(state: &State) -> usize {
        state
            .history
            .iter()
            .chain(state.partial.iter())
            .flat_map(|d| d.display.iter())
            .flat_map(|p| p.lines())
            .count()
    }

    fn line_width(state: &State, idx: usize) -> Option<usize> {
        state
            .history
            .iter()
            .chain(state.partial.iter())
            .flat_map(|d| d.display.iter())
            .flat_map(|p| p.lines())
            .nth(idx)
            .map(|line| {
                line.spans
                    .iter()
                    .fold(0, |length, span| length + span.content.len())
            })
    }

    fn update_cursor(state: &mut State) {
        if let Some(focused_line_width) = Self::line_width(state, state.cursor.row) {
            if focused_line_width < state.cursor.col {
                state.cursor.col = focused_line_width;
                // To account for newline symbol
                state.cursor.col = state.cursor.col.saturating_sub(1);
            }
        }
    }

    fn update_selection(state: &mut State) {
        match state.selection {
            Some(Selection::Line(ref mut selection)) => {
                Self::compare_and_update_selection(state.cursor.row, selection);
            }
            Some(Selection::Char(ref mut selection)) => {
                Self::compare_and_update_selection(state.cursor, selection);
            }
            None => {}
        }
    }

    fn compare_and_update_selection<T: Ord + Copy>(
        cursor: T,
        selection: &mut ConcreteSelection<T>,
    ) {
        selection.range = match selection.start.cmp(&cursor) {
            std::cmp::Ordering::Less => selection.start..=cursor,
            std::cmp::Ordering::Equal => cursor..=cursor,
            std::cmp::Ordering::Greater => cursor..=selection.start,
        };
    }

    fn selected_text(state: &State) -> Option<String> {
        let selection = if let Some(selection) = &state.selection {
            selection
        } else {
            return None;
        };
        let lines = state
            .history
            .iter()
            .chain(state.partial.iter())
            .flat_map(|d| d.display.iter())
            .flat_map(|paragraph| paragraph.lines.iter())
            .enumerate();
        match selection {
            Selection::Line(line_selection) => Some(
                lines
                    .filter(|(idx, _)| line_selection.range.contains(idx))
                    .fold(String::new(), |mut acc, next| {
                        for entry in next.1.content.iter() {
                            acc.push_str(&entry.content);
                        }
                        if !acc.ends_with('\n') {
                            acc.push('\n');
                        }
                        acc
                    }),
            ),
            Selection::Char(char_selection) => {
                let mut result = "".to_string();
                for (line_idx, line) in lines {
                    for (col_idx, letter) in line
                        .content
                        .iter()
                        .flat_map(|t| t.content.chars())
                        .enumerate()
                    {
                        let position = CursorPosition::new(line_idx, col_idx);
                        if char_selection.range.contains(&position) {
                            result.push(letter);
                        }
                    }

                    if !result.ends_with('\n') {
                        result.push('\n');
                    }
                }
                Some(result)
            }
        }
    }
}

impl tca::Reducer<State, Action> for Feature {
    fn reduce(state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::Delegated(_) => Effect::none(),
            Action::CommitMessage(msg) => {
                state.selection = None;
                state.partial = Default::default();
                let markdown = parse_markdown(msg.content.clone());
                let parahraphs = IntermediateMarkdownPassResult::into_paragraphs(markdown);
                state.history.push(DisplayableMessage::new(msg, parahraphs));
                state.cursor =
                    CursorPosition::new(Feature::total_lines(state).saturating_sub(2), 0);

                let history_msgs_to_save: Vec<ChatMessage> = state
                    .history
                    .iter()
                    .map(|msg| &msg.original)
                    .cloned()
                    .collect();
                let conversation_info = state.id.clone();
                let history_to_save = ChatHistory::new(history_msgs_to_save);
                let api = Api::new(state.config.clone());

                Effect::run(move |sender| async move {
                    let mut metadata = load_metadata().unwrap_or_default();

                    let (title, last_updated) = if history_to_save.history.len() > 4
                        && (history_to_save.history.len() - conversation_info.titlte_updated_at
                            >= 10
                            || conversation_info.titlte_updated_at == 0)
                    {
                        let mut conversation = Conversation::new_with_history(
                            api.client,
                            history_to_save.history.clone(),
                        );
                        if let Ok(res) = conversation.send_message(CONVERSATION_SUMMARY).await {
                            (
                                res.message_choices[0].message.content.clone(),
                                history_to_save.history.len(),
                            )
                        } else {
                            (conversation_info.title, conversation_info.titlte_updated_at)
                        }
                    } else {
                        (conversation_info.title, conversation_info.titlte_updated_at)
                    };

                    metadata.list.retain(|item| item.id != conversation_info.id);
                    metadata.list.insert(
                        0,
                        ConversationItem::new(conversation_info.id, title.clone(), last_updated),
                    );

                    let home_dir = dirs::home_dir().expect("Failed to get home directory");
                    let history_dir = home_dir.join(".tgpt").join("history");
                    std::fs::create_dir_all(&history_dir)
                        .expect("Failed to create history directory");
                    let file_path = history_dir.join(conversation_info.id.to_string());

                    let serialized = serde_json::to_string(&history_to_save)
                        .expect("Failed to serialize history");

                    std::fs::write(file_path, serialized).expect("Failed to write history to file");

                    save_metadata(metadata).expect("Failed to write metadata to file");

                    if history_to_save.history.len() == 1
                        || last_updated != conversation_info.titlte_updated_at
                    {
                        sender.send(Action::UpdateConversationTitle(title));
                    }
                })
            }
            Action::UpdateConversationTitle(title) => {
                state.id.title = title;
                Effect::send(Action::Delegated(Delegated::ConversationTitleUpdated))
            }
            Action::UpdatePartial(msg) => {
                state.partial = msg
                    .into_iter()
                    .map(|original| {
                        let styled = StyledText::new(original.content.clone(), Style::default());
                        let paragraphs = IntermediateMarkdownPassResult::into_paragraphs(vec![
                            IntermediateMarkdownPassResult::StyledText(styled),
                        ]);
                        DisplayableMessage::new(original, paragraphs)
                    })
                    .collect();
                Effect::none()
            }
            Action::Move(moves::Action::Delegated(delegated)) => match delegated {
                moves::Delegated::Up => {
                    state.cursor.row = state.cursor.row.saturating_sub(1);
                    Feature::update_selection(state);
                    Effect::none()
                }
                moves::Delegated::UpMore => {
                    state.cursor.row = state.cursor.row.saturating_sub(10);
                    Feature::update_selection(state);
                    Effect::none()
                }
                moves::Delegated::Down => {
                    state.cursor.row = state
                        .cursor
                        .row
                        .saturating_add(1)
                        .min(Self::total_lines(state).saturating_sub(1));
                    Feature::update_selection(state);
                    Effect::none()
                }
                moves::Delegated::DownMore => {
                    state.cursor.row = state
                        .cursor
                        .row
                        .saturating_add(10)
                        .min(Self::total_lines(state).saturating_sub(1));
                    Feature::update_selection(state);
                    Effect::none()
                }
                moves::Delegated::Left => {
                    Self::update_cursor(state);
                    state.cursor.col = state.cursor.col.saturating_sub(1);
                    Feature::update_selection(state);
                    Effect::none()
                }
                moves::Delegated::Right => {
                    state.cursor.col = state.cursor.col.saturating_add(1);
                    Self::update_cursor(state);
                    Feature::update_selection(state);
                    Effect::none()
                }
                moves::Delegated::Noop(e) => Effect::send(Action::Delegated(Delegated::Noop(e))),
            },
            Action::Move(action) => moves::Feature::reduce(&mut (), action).map(Action::Move),
            Action::ScheduleTooltip(tooltip) => Effect::run(|sender| async move {
                sender.send(Action::SetTooltip(Some(tooltip)));
                tokio::time::sleep(Duration::from_secs(3)).await;
                sender.send(Action::SetTooltip(None));
            }),
            Action::SetTooltip(tooltip) => {
                state.tooltip = tooltip;
                Effect::none()
            }
            Action::ScrollOffsetChanged(pos) => {
                state.scroll_state.scroll.set_offset(pos);
                Effect::none()
            }
            Action::ScrollViewDimentionsChanged(scroll_dimentions) => {
                if Some(scroll_dimentions) == state.scroll_view_dimentions {
                    return Effect::none();
                }
                state.scroll_view_dimentions = Some(scroll_dimentions);
                state.scroll_state.scroll.scroll_to_bottom();
                state.scroll_state.scroll.set_offset(
                    scroll_dimentions.ensure_within_bounds(state.scroll_state.scroll.offset()),
                );

                Effect::none()
            }
            Action::NewMessage(message) => {
                let api = Api::new(state.config.clone());
                let history: Vec<ChatMessage> = state
                    .history
                    .iter()
                    .map(|msg| &msg.original)
                    .cloned()
                    .collect();

                Effect::run(|send| async move {
                    if message.is_empty() {
                        return;
                    }
                    send.send(Action::BeganStreaming);
                    let user_message = ChatMessage {
                        role: chatgpt::types::Role::User,
                        content: message.clone(),
                    };
                    send.send(Action::CommitMessage(user_message));

                    let mut conversation = if history.is_empty() {
                        api.client.new_conversation()
                    } else {
                        Conversation::new_with_history(api.client, history)
                    };
                    let mut stream = match conversation.send_message_streaming(message).await {
                        Ok(stream) => stream,
                        Err(err) => {
                            let tooltip = Tooltip::new(
                                TooltipKind::Error,
                                format!("Completion error: {}", err),
                            );
                            send.send(Action::ScheduleTooltip(tooltip));
                            send.send(Action::StoppedStreaming);
                            return;
                        }
                    };

                    let mut output: Vec<ResponseChunk> = Vec::new();
                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(chunk) => {
                                output.push(chunk);
                                let partial = ChatMessage::from_response_chunks(output.clone());
                                send.send(Action::UpdatePartial(partial));
                            }
                            Err(err) => {
                                for message in ChatMessage::from_response_chunks(output).into_iter()
                                {
                                    send.send(Action::CommitMessage(message));
                                }
                                let tooltip = Tooltip::new(
                                    TooltipKind::Error,
                                    format!("Completion error: {}", err),
                                );
                                send.send(Action::ScheduleTooltip(tooltip));
                                send.send(Action::StoppedStreaming);
                                return;
                            }
                        }
                    }
                    for message in ChatMessage::from_response_chunks(output).into_iter() {
                        send.send(Action::CommitMessage(message));
                    }
                    send.send(Action::StoppedStreaming);
                })
            }
            Action::BeganStreaming => {
                state.is_streaming = true;
                Effect::none()
            }
            Action::StoppedStreaming => {
                state.is_streaming = false;
                Effect::none()
            }
            Action::Event(e) => match e {
                Event::Key(key) if key.kind == event::KeyEventKind::Press => match key.code {
                    KeyCode::Char('v') | KeyCode::Char('V') => {
                        if state.selection.is_some() {
                            state.selection = None;
                        } else {
                            let selection = if key.modifiers.contains(KeyModifiers::SHIFT) {
                                Selection::Line(LineSelection::new(
                                    state.cursor.row,
                                    state.cursor.row..=state.cursor.row,
                                ))
                            } else {
                                Selection::Char(CharSelection::new(
                                    state.cursor,
                                    state.cursor..=state.cursor,
                                ))
                            };
                            state.selection = Some(selection);
                        }
                        Effect::none()
                    }
                    KeyCode::Char('y') => {
                        if let Some(clipped_content) = Self::selected_text(state) {
                            let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                            let _ = ctx.set_contents(clipped_content);
                            state.selection = None;
                            Effect::run(|sender| async move {
                                let tooltip =
                                    Tooltip::new(TooltipKind::Success, "Yanked!".to_string());
                                sender.send(Action::ScheduleTooltip(tooltip));
                            })
                        } else {
                            Effect::none()
                        }
                    }
                    _ => Effect::send(Action::Move(moves::Action::Event(e))),
                },
                _ => Effect::send(Action::Move(moves::Action::Event(e))),
            },
        }
    }
}

const SCROLL_BAR_WIDTH: u16 = 1;
const SCROLL_BAR_PADDING: u16 = 1;

pub fn ui(frame: &mut Frame, area: Rect, store: tca::Store<State, Action>) {
    let state = store.state();
    let navigation = Block::default()
        .title(format!("[2] {}", state.id.title.clone()))
        .borders(Borders::all())
        .border_type(BorderType::Rounded);

    let width = navigation.inner(area).width - SCROLL_BAR_WIDTH - SCROLL_BAR_PADDING;
    let mut messages: Vec<(Paragraph, Rect)> = Default::default();
    let mut prev_y: u16 = 0;
    let mut line_offset = 0;
    let mut rendered_line_offset = 0;
    let mut resolved_rendered_cursor: Option<std::ops::RangeInclusive<u16>> = None;
    for msg in state.history.iter().chain(state.partial.iter()) {
        let role_block = Block::new()
            .title(Title::from(
                crate::gpt::openai::display(msg.original.role) + " ",
            ))
            .borders(Borders::TOP)
            .border_type(ratatui::widgets::BorderType::Double)
            .border_style(Style::new().dark_gray());

        let mut first_paragraph = true;

        for styled_paragraph in msg.display.iter() {
            let block = if first_paragraph {
                role_block.clone()
            } else {
                Block::default()
            };

            let mut lines = styled_paragraph.lines().collect::<Vec<_>>();
            let focused_line = if state.cursor.row >= line_offset
                && state.cursor.row < line_offset + lines.len()
            {
                Some(state.cursor.row - line_offset)
            } else {
                None
            };
            match &state.selection {
                Some(Selection::Line(selection)) => {
                    lines.iter_mut().enumerate().for_each(|(idx, line)| {
                        let global_idx = idx + line_offset;
                        if selection.range.contains(&global_idx) {
                            *line = line.clone().style(styled_paragraph.highlighted_style);
                        }
                    });
                }
                Some(Selection::Char(selection)) => {
                    let first_global_line_idx = line_offset;
                    let last_global_line_idx = lines.len().saturating_sub(1);

                    let participates_in_selection = (selection.range.start().row
                        <= first_global_line_idx
                        && first_global_line_idx <= selection.range.end().row)
                        || (selection.range.start().row <= last_global_line_idx
                            && last_global_line_idx <= selection.range.end().row);

                    if participates_in_selection {
                        let mut selected_lines: Vec<Line> = vec![];
                        for (local_line_idx, line) in lines.iter_mut().enumerate() {
                            let mut edited_line = Line::styled("", line.style);
                            let line_idx = local_line_idx + line_offset;
                            for (col_idx, grapheme) in line.styled_graphemes(line.style).enumerate()
                            {
                                let position = CursorPosition::new(line_idx, col_idx);

                                let style = if selection.range.contains(&position) {
                                    grapheme.style.patch(styled_paragraph.highlighted_style)
                                } else {
                                    grapheme.style
                                };
                                edited_line
                                    .push_span(Span::styled(grapheme.symbol.to_owned(), style));
                            }
                            selected_lines.push(edited_line);
                        }
                        lines = selected_lines;
                    }
                }
                None => {
                    if let Some(focused_line) = focused_line {
                        let focused_line_style = lines[focused_line].style;
                        let mut line = Line::styled("", focused_line_style);
                        let words_count = lines[focused_line].to_string().len();
                        let cursor_col = if state.cursor.col >= words_count {
                            words_count.saturating_sub(1)
                        } else {
                            state.cursor.col
                        };

                        for (idx, grapheme) in lines[focused_line]
                            .styled_graphemes(focused_line_style)
                            .enumerate()
                        {
                            let style = if idx == cursor_col {
                                grapheme.style.patch(styled_paragraph.highlighted_style)
                            } else {
                                grapheme.style
                            };
                            line.push_span(Span::styled(grapheme.symbol.to_owned(), style));
                        }
                        lines[focused_line] = line;
                    }
                }
            }

            let paragraph_text_width = width.max(0);

            resolved_rendered_cursor = try_resolve_cursor_if_needed(
                resolved_rendered_cursor,
                &lines,
                &mut rendered_line_offset,
                first_paragraph,
                focused_line,
                paragraph_text_width,
            );

            line_offset += lines.len();

            let mut paragraph = Paragraph::new(lines)
                .style(styled_paragraph.style)
                .block(block);
            if !styled_paragraph.is_empty_render() {
                paragraph = paragraph.wrap(Wrap { trim: false });
            }
            let paragraph_text_height = paragraph.line_count(paragraph_text_width) as u16;
            let height = paragraph_text_height;
            let text_area = Rect::new(1, prev_y, width - 1, height);
            prev_y += height;
            first_paragraph = false;

            messages.push((paragraph, text_area));
        }
    }

    let scroll_size = Size::new(width, messages.last().map_or(0, |rect| rect.1.bottom()));
    let mut scroll_view = ScrollView::new(scroll_size);
    messages.into_iter().for_each(|(msg, rect)| {
        msg.render(rect, scroll_view.buf_mut());
    });

    let mut renderable_state = state.scroll_state.scroll;
    let scroll_size = scroll_view.size();
    let chat_rect = navigation.inner(area);
    let scroll_area = chat_rect.as_size();
    let scroll_dimentions = ScrollViewDiementions {
        frame_size: scroll_area,
        scroll_size,
    };
    let max_offset = scroll_size.height.saturating_sub(scroll_area.height);
    renderable_state.set_offset(ratatui::layout::Position {
        x: 0,
        y: std::cmp::min(renderable_state.offset().y, max_offset),
    });
    let resolved_cursor = resolved_rendered_cursor.unwrap_or(0..=0);
    if *resolved_cursor.start() < renderable_state.offset().y {
        let new_y = if *resolved_cursor.start() <= 1 {
            // Special handling for first line that is block title that
            // we need to show.
            0
        } else {
            *resolved_cursor.end()
        };
        renderable_state.set_offset(Position::new(0, new_y));
        store.send(Action::ScrollOffsetChanged(renderable_state.offset()));
    } else if *resolved_cursor.end() >= renderable_state.offset().y + scroll_area.height {
        let new_y = resolved_cursor.end().saturating_sub(scroll_area.height) + 1;
        renderable_state.set_offset(Position::new(0, new_y));
        store.send(Action::ScrollOffsetChanged(renderable_state.offset()));
    }

    frame.render_stateful_widget(scroll_view, chat_rect, &mut renderable_state);

    if let Some(tooltip) = &state.tooltip {
        let tooltip_style = match tooltip.kind {
            TooltipKind::Success => Style::default().green(),
            TooltipKind::Error => Style::default().red(),
        };
        let tooltip_widget = Paragraph::new(tooltip.text.as_str())
            .alignment(ratatui::layout::Alignment::Center)
            .style(tooltip_style)
            .block(
                Block::default()
                    .borders(Borders::all())
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(Style::default().green()),
            );
        let width = tooltip_widget.line_width() as u16 + 2 + 2; // + block padding + padding
        let rect = Rect::new(chat_rect.width.saturating_sub(width), 1, width, 3);
        frame.render_widget(tooltip_widget, rect);
    }

    let navigation_style = if state.current_focus.value() == CurrentFocus::Conversation {
        Style::new().green()
    } else {
        Style::default()
    };
    frame.render_widget(navigation.border_style(navigation_style), area);

    if Some(scroll_dimentions) != state.scroll_view_dimentions {
        store.send(Action::ScrollViewDimentionsChanged(scroll_dimentions));
    }
}

/// Resolving logical per-line cursor position to actual rendered cursor position
/// respecting line wraps.
/// TODO: Can we use wrapped lines to do the actual rendering to avoid recomputation?
fn try_resolve_cursor_if_needed(
    resolved_cursor: Option<std::ops::RangeInclusive<u16>>,
    lines: &[ratatui::text::Line],
    rendered_line_offset: &mut u16,
    first_paragraph: bool,
    focused_line: Option<usize>,
    max_line_width: u16,
) -> Option<std::ops::RangeInclusive<u16>> {
    if resolved_cursor.is_some() {
        return resolved_cursor;
    }
    if first_paragraph {
        *rendered_line_offset += 1;
    }
    let mut upper_bound: u16 = 0;
    for (idx, line) in lines.iter().enumerate() {
        let is_focused_line = Some(idx) == focused_line;
        if is_focused_line {
            // Records drawed beginning position of cursor
            upper_bound = *rendered_line_offset;
        }
        if line.spans.len() == 1 && line.spans[0].content == " " {
            *rendered_line_offset += 1;
        } else {
            let line_ref = [line];
            let graphemes = line_ref.iter().map(|line| {
                let graphemes = line
                    .spans
                    .iter()
                    .flat_map(|span| span.styled_graphemes(line.style));
                let alignment = line.alignment.unwrap_or(ratatui::layout::Alignment::Left);
                (graphemes, alignment)
            });
            let mut line_composer = WordWrapper::new(graphemes, max_line_width, false);

            while line_composer.next_line().is_some() {
                *rendered_line_offset += 1;
            }
        }
        if is_focused_line {
            // Records drawed end position of cursor. It is different for lines
            // that wrap.
            let lower_bound = *rendered_line_offset - 1;
            return Some(upper_bound..=lower_bound);
        }
    }

    None
}
