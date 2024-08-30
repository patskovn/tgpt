use std::time::Duration;

use crate::uiutils::layout::Inset;
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
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::{self, Event, KeyModifiers};
use derive_new::new;
use futures::StreamExt;
use ratatui::{
    layout::{Constraint, Layout, Position, Rect, Size},
    style::{Style, Stylize},
    widgets::{block::Title, Block, Borders, Paragraph, Widget, Wrap},
    Frame,
};
use tca::ActionSender;
use tca::Effect;
use tui_scrollview::ScrollView;

use crate::{
    app::navigation,
    editor::Mode,
    gpt::openai::{Api, ChatGPTConfiguration},
    scroll_view, textfield,
};

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
struct DisplayableMessage {
    original: ChatMessage,
    display: Vec<StyledParagraph>,
}

impl PartialEq for DisplayableMessage {
    fn eq(&self, other: &Self) -> bool {
        self.original == other.original
    }
}

impl DisplayableMessage {
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
pub struct State<'a> {
    textarea: textfield::State<'a>,
    current_focus: CurrentFocus,
    cursor: (usize, usize),
    selection: Option<(usize, std::ops::RangeInclusive<usize>)>,
    config: ChatGPTConfiguration,
    history: Vec<DisplayableMessage>,
    partial: Vec<DisplayableMessage>,
    scroll_state: scroll_view::State,
    scroll_view_dimentions: Option<ScrollViewDiementions>,
    is_streaming: bool,
    tooltip: Option<String>,
}

#[derive(Debug, PartialEq, Clone)]
enum CurrentFocus {
    TextArea,
    Chat,
}

const TEST: &str = "Here's a simple \"Hello, world!\" program in Rust:\n\n```rust\nfn main() {\n    println!(\"Hello, world!\");\n}\n```\n\nTo run it, save the code in a file named `main.rs` and use the command `cargo run` or `rustc main.rs` followed by `./main`.";

impl State<'_> {
    pub fn new(config: ChatGPTConfiguration) -> Self {
        Self {
            textarea: textfield::State::default(),
            current_focus: CurrentFocus::TextArea,
            cursor: (0, 0),
            selection: Default::default(),
            config,
            history: vec![DisplayableMessage::from(TEST)],
            partial: Default::default(),
            scroll_state: Default::default(),
            scroll_view_dimentions: Default::default(),
            is_streaming: false,
            tooltip: None,
        }
    }
}

#[derive(Debug)]
pub enum Action {
    Event(Event),
    TextField(textfield::Action),
    ScrollView(scroll_view::Action),
    ScrollViewDimentionsChanged(ScrollViewDiementions),
    BeganStreaming,
    StoppedStreaming,
    Delegated(Delegated),
    CommitMessage(ChatMessage),
    UpdatePartial(Vec<ChatMessage>),
    SetTooltip(Option<String>),
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
}

#[derive(Default)]
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

    fn update_selection(state: &mut State) {
        if let Some(ref mut selection) = state.selection {
            selection.1 = match selection.0.cmp(&state.cursor.0) {
                std::cmp::Ordering::Less => selection.0..=state.cursor.0,
                std::cmp::Ordering::Equal => state.cursor.0..=state.cursor.0,
                std::cmp::Ordering::Greater => state.cursor.0..=selection.0,
            };
        }
    }
}

impl tca::Reducer<State<'_>, Action> for Feature {
    fn reduce(state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::Delegated(_) => Effect::none(),
            Action::CommitMessage(msg) => {
                state.selection = None;
                state.partial = Default::default();
                let markdown = parse_markdown(msg.content.clone());
                let parahraphs = IntermediateMarkdownPassResult::into_paragraphs(markdown);
                state.history.push(DisplayableMessage::new(msg, parahraphs));
                state.cursor = (Feature::total_lines(state).saturating_sub(2), 0);

                Effect::none()
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
            Action::ScrollView(scroll_view::Action::Delegated(delegated)) => match delegated {
                scroll_view::Delegated::Up => {
                    state.cursor.0 = state.cursor.0.saturating_sub(1);
                    Feature::update_selection(state);

                    state.scroll_state.scroll.scroll_up();
                    if let Some(scroll_dimentions) = state.scroll_view_dimentions {
                        state.scroll_state.scroll.set_offset(
                            scroll_dimentions
                                .ensure_within_bounds(state.scroll_state.scroll.offset()),
                        );
                    }
                    Effect::none()
                }
                scroll_view::Delegated::Down => {
                    state.cursor.0 = state
                        .cursor
                        .0
                        .saturating_add(1)
                        .min(Self::total_lines(state).saturating_sub(1));
                    Feature::update_selection(state);
                    state.scroll_state.scroll.scroll_down();
                    Effect::none()
                }
                scroll_view::Delegated::Noop(e) => {
                    Effect::send(Action::Delegated(Delegated::Noop(e)))
                }
            },
            Action::ScrollView(action) => {
                scroll_view::Feature::reduce(&mut state.scroll_state, action)
                    .map(Action::ScrollView)
            }
            Action::SetTooltip(tooltip) => {
                state.tooltip = tooltip;
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
            Action::TextField(textfield::Action::Delegated(delegated)) => match delegated {
                textfield::Delegated::Quit => Effect::none(),
                textfield::Delegated::Noop(e) => {
                    Effect::send(Action::Delegated(Delegated::Noop(e)))
                }
                textfield::Delegated::Updated => Effect::none(),
                textfield::Delegated::Commit => {
                    if state.is_streaming {
                        return Effect::none();
                    }
                    let api = Api::new(state.config.clone());
                    let history: Vec<ChatMessage> = state
                        .history
                        .iter()
                        .map(|msg| &msg.original)
                        .cloned()
                        .collect();
                    let message = state.textarea.textarea.lines().join("\n");
                    state.textarea = crate::textfield::State::default();

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
                        let mut stream =
                            conversation.send_message_streaming(message).await.unwrap();

                        let mut output: Vec<ResponseChunk> = Vec::new();
                        while let Some(chunk) = stream.next().await {
                            output.push(chunk);
                            let partial = ChatMessage::from_response_chunks(output.clone());
                            send.send(Action::UpdatePartial(partial));
                        }
                        for message in ChatMessage::from_response_chunks(output).into_iter() {
                            send.send(Action::CommitMessage(message));
                        }
                        send.send(Action::StoppedStreaming);
                    })
                }
            },
            Action::BeganStreaming => {
                state.is_streaming = true;
                Effect::none()
            }
            Action::StoppedStreaming => {
                state.is_streaming = false;
                Effect::none()
            }
            Action::TextField(action) => {
                textfield::Feature::reduce(&mut state.textarea, action).map(Action::TextField)
            }
            Action::Event(e) => match e {
                Event::Key(KeyEvent {
                    code: event::KeyCode::Tab,
                    kind: event::KeyEventKind::Press,
                    modifiers: KeyModifiers::NONE,
                    ..
                }) if state.current_focus != CurrentFocus::TextArea
                    || state.textarea.editor.mode == Mode::Normal =>
                {
                    if state.current_focus == CurrentFocus::TextArea {
                        state.current_focus = CurrentFocus::Chat;
                    } else {
                        state.current_focus = CurrentFocus::TextArea;
                    };
                    Effect::none()
                }
                _ => match state.current_focus {
                    CurrentFocus::Chat => match e {
                        Event::Key(key) if key.kind == event::KeyEventKind::Press => match key.code
                        {
                            KeyCode::Char('v') => {
                                if state.selection.is_some() {
                                    state.selection = None;
                                } else {
                                    state.selection =
                                        Some((state.cursor.0, state.cursor.0..=state.cursor.0));
                                }
                                Effect::none()
                            }
                            KeyCode::Char('y') => {
                                if let Some(selection) = &state.selection {
                                    let mut ctx: ClipboardContext =
                                        ClipboardProvider::new().unwrap();
                                    let clipped_content: String = state
                                        .history
                                        .iter()
                                        .chain(state.partial.iter())
                                        .flat_map(|d| d.display.iter())
                                        .flat_map(|paragraph| paragraph.lines.iter())
                                        .enumerate()
                                        .filter(|(idx, _)| selection.1.contains(idx))
                                        .fold(Default::default(), |mut acc, next| {
                                            for entry in next.1.content.iter() {
                                                acc.push_str(&entry.content);
                                            }
                                            acc
                                        });
                                    log::debug!("Clip {}", clipped_content);
                                    let _ = ctx.set_contents(clipped_content);
                                    state.selection = None;
                                    Effect::run(|sender| async move {
                                        sender
                                            .send(Action::SetTooltip(Some("Yanked!".to_string())));
                                        tokio::time::sleep(Duration::from_secs(3)).await;
                                        sender.send(Action::SetTooltip(None));
                                    })
                                } else {
                                    Effect::none()
                                }
                            }
                            _ => Effect::send(Action::ScrollView(scroll_view::Action::Event(e))),
                        },
                        _ => Effect::send(Action::ScrollView(scroll_view::Action::Event(e))),
                    },
                    CurrentFocus::TextArea => {
                        Effect::send(Action::TextField(textfield::Action::Event(e)))
                    }
                },
            },
        }
    }
}

pub fn ui(frame: &mut Frame, area: Rect, state: &State, store: tca::Store<State, Action>) {
    let navigation = navigation::ui(navigation::CurrentScreen::Chat);
    let body = {
        let layout = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(vec![Constraint::Fill(1), Constraint::Max(10)])
            .split(area);

        let mut cloned_area = state.textarea.clone();
        if state.current_focus == CurrentFocus::TextArea {
            if let Some(block) = cloned_area.textarea.block() {
                cloned_area
                    .textarea
                    .set_block(block.clone().border_style(Style::new().blue()))
            }
        };
        frame.render_widget(cloned_area.widget(), layout[1]);
        layout[0]
    };

    let width = navigation.inner(body).width - 3;
    let sample = Rect::new(0, 0, 10, 10);
    let mut messages: Vec<(Paragraph, Rect)> = Default::default();
    let mut prev_y: u16 = 0;
    let mut line_offset = 0;
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
            let (block, inner) = if first_paragraph {
                let b = role_block.clone();
                let inner = b.inner(sample);
                // Role block adds one extra line
                (b, inner)
            } else {
                let b = Block::default();
                let inner = b.inner(sample);
                (b, inner)
            };
            first_paragraph = false;

            let mut lines = styled_paragraph.lines().collect::<Vec<_>>();
            let (cursor_row, _) = state.cursor;
            match &state.selection {
                Some(selection) => {
                    log::debug!("Has selection {:#?}", selection);
                    lines.iter_mut().enumerate().for_each(|(idx, line)| {
                        let global_idx = idx + line_offset;
                        if selection.1.contains(&global_idx) {
                            *line = line.clone().style(styled_paragraph.highlighted_style);
                        }
                    });
                }
                None => {
                    if cursor_row >= line_offset && cursor_row < line_offset + lines.len() {
                        let highlighted_line_idx = cursor_row - line_offset;
                        lines[highlighted_line_idx] = lines[highlighted_line_idx]
                            .clone()
                            .style(styled_paragraph.highlighted_style);
                    }
                }
            }
            line_offset += lines.len();

            let mut paragraph = Paragraph::new(lines)
                .style(styled_paragraph.style)
                .block(block);
            if !styled_paragraph.is_empty_render() {
                paragraph = paragraph.wrap(Wrap { trim: false });
            }

            let inset = Inset::new(
                inner.x,
                inner.y,
                sample.width - inner.x - inner.width + 1,
                sample.height - inner.y - inner.height,
            );

            let paragraph_text_width = std::cmp::max(0, width - inset.left - inset.right);
            let paragraph_text_height = paragraph.line_count(paragraph_text_width) as u16;
            let height = paragraph_text_height + inset.top + inset.bottom;

            let text_area = Rect::new(1, prev_y, width - 1, height);
            prev_y += height;

            messages.push((paragraph, text_area))
        }
    }

    let mut scroll_view = ScrollView::new(Size::new(
        width,
        messages.last().map_or(0, |rect| rect.1.bottom()),
    ));
    messages.into_iter().for_each(|(msg, rect)| {
        msg.render(rect, scroll_view.buf_mut());
    });

    let mut renderable_state = state.scroll_state.scroll;
    let scroll_size = scroll_view.size();
    let chat_rect = navigation.inner(body);
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

    frame.render_stateful_widget(scroll_view, chat_rect, &mut renderable_state);

    if let Some(tooltip) = &state.tooltip {
        let tooltip_widget = Paragraph::new(tooltip.as_str())
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().green())
            .block(
                Block::default()
                    .borders(Borders::all())
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(Style::default().green()),
            );
        let width = tooltip_widget.line_width() as u16 + 2 + 2; // + block padding + padding
        let rect = Rect::new(chat_rect.width - width, 1, width, 3);
        frame.render_widget(tooltip_widget, rect);
    }

    let navigation_style = if state.current_focus == CurrentFocus::Chat {
        Style::new().blue()
    } else {
        Style::default()
    };
    frame.render_widget(navigation.border_style(navigation_style), body);

    if Some(scroll_dimentions) != state.scroll_view_dimentions {
        store.send(Action::ScrollViewDimentionsChanged(scroll_dimentions));
    }
}
