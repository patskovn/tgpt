use chatgpt::{
    prelude::Conversation,
    types::{ChatMessage, ResponseChunk},
};
use color_eyre::owo_colors::OwoColorize;
use crossterm::event::{self, Event, KeyModifiers};
use derive_new::new;
use futures::StreamExt;
use ratatui::{
    layout::{Constraint, Layout, Position, Rect, Size},
    style::{Style, Styled, Stylize},
    text::{Line, Span},
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

#[derive(Debug, Clone)]
struct DisplayableMessage {
    original: ChatMessage,
    display: Vec<MarkdownContent>,
}

impl PartialEq for DisplayableMessage {
    fn eq(&self, other: &Self) -> bool {
        self.original == other.original
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct State<'a> {
    textarea: textfield::State<'a>,
    current_focus: CurrentFocus,
    config: ChatGPTConfiguration,
    history: Vec<DisplayableMessage>,
    partial: Vec<DisplayableMessage>,
    scroll_state: scroll_view::State,
    scroll_view_dimentions: Option<ScrollViewDiementions>,
    is_streaming: bool,
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
            config,
            history: vec![DisplayableMessage {
                original: ChatMessage {
                    role: chatgpt::types::Role::User,
                    content: TEST.to_string(),
                },
                display: parse_markdown(TEST),
            }],
            partial: Default::default(),
            scroll_state: Default::default(),
            scroll_view_dimentions: Default::default(),
            is_streaming: false,
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
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
}

fn markdown_parse_options() -> markdown::ParseOptions {
    markdown::ParseOptions {
        constructs: markdown::Constructs {
            attention: true,
            autolink: false,
            block_quote: false,
            character_escape: true,
            character_reference: false,
            code_indented: false,
            code_fenced: true,
            code_text: true,
            definition: false,
            frontmatter: false,
            gfm_autolink_literal: false,
            gfm_label_start_footnote: false,
            gfm_footnote_definition: false,
            gfm_strikethrough: false,
            gfm_table: false,
            gfm_task_list_item: false,
            hard_break_escape: false,
            hard_break_trailing: false,
            heading_atx: false,
            heading_setext: false,
            html_flow: false,
            html_text: false,
            label_start_image: false,
            label_start_link: false,
            label_end: false,
            list_item: false,
            math_flow: false,
            math_text: false,
            mdx_esm: false,
            mdx_expression_flow: false,
            mdx_expression_text: false,
            mdx_jsx_flow: false,
            mdx_jsx_text: false,
            thematic_break: false,
        },
        gfm_strikethrough_single_tilde: false,
        math_text_single_dollar: false,
        ..Default::default()
    }
}

fn parse_markdown(message: &str) -> Vec<MarkdownContent> {
    let root_node = markdown::to_mdast(message, &markdown_parse_options()).unwrap();
    log::debug!("Parsed markdown {:#?}", root_node);
    let mut result: Vec<MarkdownContent> = Default::default();
    process_markdown(root_node, &Default::default(), &mut result);

    result
}

#[derive(PartialEq, Clone, Copy, Eq, Hash)]
enum TextModifier {
    Strong,
    InlineCode,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash, new)]
struct StyledText {
    content: String,
    style: Style,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash, new)]
struct Code {
    content: String,
    language: Option<String>,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
enum MarkdownContent {
    StyledText(StyledText),
    Code(Code),
}

impl MarkdownContent {
    fn into_paragraphs<'a>(value: Vec<MarkdownContent>) -> Vec<Paragraph<'a>> {
        let mut all_paragraphs: Vec<Paragraph> = vec![];
        let mut all_lines: Vec<ratatui::prelude::Line> = vec![];
        let mut paragraph_line: Vec<Span> = vec![];

        fn push_line<'a>(
            all_lines: &mut Vec<ratatui::prelude::Line<'a>>,
            paragraph_line: &mut Vec<Span<'a>>,
        ) {
            if paragraph_line.is_empty() {
                return;
            }
            all_lines.push(Line::from(paragraph_line.clone()));
            paragraph_line.clear();
        }

        fn push_paragraph<'a>(
            all_paragraphs: &mut Vec<Paragraph<'a>>,
            all_lines: &mut Vec<ratatui::prelude::Line<'a>>,
        ) {
            if all_lines.is_empty() {
                return;
            }
            all_paragraphs.push(Paragraph::new(all_lines.clone()));
            all_lines.clear();
        }

        for markdown in value.into_iter() {
            match markdown {
                Self::StyledText(styled_text) => {
                    for line in styled_text.content.split_inclusive("\n") {
                        let has_newline = line.contains("\n");
                        let line = line.strip_suffix('\n').unwrap_or(line);
                        let line = line.strip_suffix('\r').unwrap_or(line);

                        paragraph_line.push(Span::styled(line.to_owned(), styled_text.style));
                        if has_newline {
                            push_line(&mut all_lines, &mut paragraph_line);
                        }
                    }
                }
                Self::Code(code) => {
                    push_line(&mut all_lines, &mut paragraph_line);
                    push_paragraph(&mut all_paragraphs, &mut all_lines);
                    let mut lines: Vec<Line> = vec![];
                    lines.push(Line::from(
                        code.language
                            .map_or("```".to_string(), |lang| "```".to_string() + &lang),
                    ));
                    lines.append(
                        &mut code
                            .content
                            .lines()
                            .map(|l| Line::from(l.to_owned()))
                            .collect(),
                    );
                    lines.push(Line::from("```"));

                    all_paragraphs.push(
                        Paragraph::new(lines).set_style(Style::default().gray().on_dark_gray()),
                    );
                    all_paragraphs.push(Paragraph::new("\n"));
                }
            }
        }
        push_line(&mut all_lines, &mut paragraph_line);
        push_paragraph(&mut all_paragraphs, &mut all_lines);

        all_paragraphs
    }
}

impl<'a> From<StyledText> for ratatui::prelude::Span<'a> {
    fn from(value: StyledText) -> Self {
        Span::styled(value.content, value.style)
    }
}

fn process_markdown(
    node: markdown::mdast::Node,
    modifiers: &std::collections::HashSet<TextModifier>,
    result: &mut Vec<MarkdownContent>,
) {
    let process_node = { |n| process_markdown(n, modifiers, result) };
    match node {
        markdown::mdast::Node::Root(n) => n.children.into_iter().for_each(process_node),
        markdown::mdast::Node::Paragraph(n) => {
            n.children.into_iter().for_each(process_node);
            result.push(MarkdownContent::StyledText(StyledText::new(
                "\n\n".to_string(),
                Default::default(),
            )));
        }
        markdown::mdast::Node::Code(n) => {
            result.push(MarkdownContent::Code(Code::new(n.value, n.lang)))
        }
        markdown::mdast::Node::InlineCode(n) => {
            result.push(MarkdownContent::StyledText(process_text(
                n.value,
                &modifiers
                    .union(&maplit::hashset! {TextModifier::InlineCode})
                    .cloned()
                    .collect(),
            )))
        }
        markdown::mdast::Node::Text(text) => result.push(MarkdownContent::StyledText(
            process_text(text.value, modifiers),
        )),
        markdown::mdast::Node::Emphasis(n) => n.children.into_iter().for_each(|child| {
            process_markdown(
                child,
                &modifiers
                    .union(&maplit::hashset! {TextModifier::Strong})
                    .cloned()
                    .collect(),
                result,
            )
        }),
        markdown::mdast::Node::Strong(n) => n.children.into_iter().for_each(|child| {
            process_markdown(
                child,
                &modifiers
                    .union(&maplit::hashset! {TextModifier::Strong})
                    .cloned()
                    .collect(),
                result,
            )
        }),
        _ => (),
    }
}

fn process_text(text: String, modifiers: &std::collections::HashSet<TextModifier>) -> StyledText {
    let mut span_style = Style::default();
    if modifiers.contains(&TextModifier::Strong) {
        span_style = span_style.bold();
    }
    let mut text = text;
    if modifiers.contains(&TextModifier::InlineCode) {
        text = "`".to_string() + &text + "`";
        span_style = span_style.blue().italic();
    }
    StyledText {
        content: text,
        style: span_style,
    }
}

#[derive(Default)]
pub struct Feature {}

impl tca::Reducer<State<'_>, Action> for Feature {
    fn reduce(&self, state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::Delegated(_) => Effect::none(),
            Action::CommitMessage(msg) => {
                state.partial = Default::default();
                let parsed_content = parse_markdown(&msg.content);
                state.history.push(DisplayableMessage {
                    original: msg,
                    display: parsed_content,
                });

                Effect::none()
            }
            Action::UpdatePartial(msg) => {
                state.partial = msg
                    .into_iter()
                    .map(|original| {
                        let styled = StyledText {
                            content: original.content.clone(),
                            style: Style::default(),
                        };
                        DisplayableMessage {
                            original,
                            display: vec![MarkdownContent::StyledText(styled)],
                        }
                    })
                    .collect();
                Effect::none()
            }
            Action::ScrollView(scroll_view::Action::Delegated(delegated)) => match delegated {
                scroll_view::Delegated::Up => {
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
                    state.scroll_state.scroll.scroll_down();
                    Effect::none()
                }
                scroll_view::Delegated::Noop(e) => {
                    Effect::send(Action::Delegated(Delegated::Noop(e)))
                }
            },
            Action::ScrollView(action) => scroll_view::Feature::default()
                .reduce(&mut state.scroll_state, action)
                .map(Action::ScrollView),
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
            Action::TextField(action) => textfield::Feature::default()
                .reduce(&mut state.textarea, action)
                .map(Action::TextField),
            Action::Event(e) => match e {
                Event::Key(key)
                    if key.kind == event::KeyEventKind::Press
                        && (state.current_focus != CurrentFocus::TextArea
                            || state.textarea.editor.mode == Mode::Normal) =>
                {
                    match key.code {
                        event::KeyCode::Tab if key.modifiers == KeyModifiers::NONE => {
                            if state.current_focus == CurrentFocus::TextArea {
                                state.current_focus = CurrentFocus::Chat;
                            } else {
                                state.current_focus = CurrentFocus::TextArea;
                            };
                            Effect::none()
                        }
                        _ => match state.current_focus {
                            CurrentFocus::Chat => {
                                Effect::send(Action::ScrollView(scroll_view::Action::Event(e)))
                            }
                            CurrentFocus::TextArea => {
                                Effect::send(Action::TextField(textfield::Action::Event(e)))
                            }
                        },
                    }
                }
                _ => match state.current_focus {
                    CurrentFocus::Chat => {
                        Effect::send(Action::ScrollView(scroll_view::Action::Event(e)))
                    }
                    CurrentFocus::TextArea => {
                        Effect::send(Action::TextField(textfield::Action::Event(e)))
                    }
                },
            },
        }
    }
}

struct Inset {
    left: u16,
    top: u16,
    right: u16,
    bottom: u16,
}

impl Inset {
    fn new(left: u16, top: u16, right: u16, bottom: u16) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
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
    for msg in state.history.iter().chain(state.partial.iter()) {
        let role_block = Block::new()
            .title(Title::from(
                crate::gpt::openai::display(msg.original.role) + " ",
            ))
            .borders(Borders::TOP)
            .border_type(ratatui::widgets::BorderType::Double)
            .border_style(Style::new().dark_gray());

        let mut first_paragraph = true;

        for paragraph in MarkdownContent::into_paragraphs(msg.display.clone()).into_iter() {
            let (block, inner) = if first_paragraph {
                let b = role_block.clone();
                let inner = b.inner(sample);
                (b, inner)
            } else {
                let b = Block::default();
                let inner = b.inner(sample);
                (b, inner)
            };
            first_paragraph = false;
            let paragraph = paragraph.wrap(Wrap { trim: false }).block(block);

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
        // Make additional 1 point bottom offset
        prev_y += 1;
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
    let scroll_area = navigation.inner(body).as_size();
    let scroll_dimentions = ScrollViewDiementions {
        frame_size: scroll_area,
        scroll_size,
    };
    let max_offset = scroll_size.height.saturating_sub(scroll_area.height);
    renderable_state.set_offset(ratatui::layout::Position {
        x: 0,
        y: std::cmp::min(renderable_state.offset().y, max_offset),
    });

    frame.render_stateful_widget(scroll_view, navigation.inner(body), &mut renderable_state);

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
