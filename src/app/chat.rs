use chatgpt::{
    prelude::Conversation,
    types::{ChatMessage, ResponseChunk},
};
use crossterm::event::{self, Event, KeyModifiers};
use futures::{FutureExt, StreamExt};
use ratatui::{
    layout::{Constraint, Layout, Rect, Size},
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

#[derive(Debug, PartialEq, Clone)]
pub struct State<'a> {
    textarea: textfield::State<'a>,
    current_focus: CurrentFocus,
    config: ChatGPTConfiguration,
    history: Vec<ChatMessage>,
    partial: Vec<ChatMessage>,
    scroll_state: scroll_view::State,
    is_streaming: bool,
}

#[derive(Debug, PartialEq, Clone)]
enum CurrentFocus {
    TextArea,
    Chat,
}

const TEST: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Nam quis elementum velit, ac cursus nisi. Proin est elit, fermentum et risus quis, finibus gravida urna. Aenean sodales, erat vel congue placerat, urna nisi venenatis odio, et lobortis purus est vitae massa. Maecenas et libero quis diam faucibus faucibus. Quisque sollicitudin velit nibh, sed imperdiet sem blandit eu. Vestibulum nec semper lorem. In elementum urna id venenatis dapibus. Nunc placerat massa ac ligula finibus tempus. Ut ut accumsan nulla. Maecenas molestie tempus nibh, efficitur imperdiet neque vehicula ut. Quisque eu condimentum orci. Mauris ac semper diam, a tincidunt leo. Praesent bibendum gravida aliquet. Praesent mollis scelerisque dignissim. Fusce convallis convallis ligula in pretium. Quisque vitae felis nunc. Nam malesuada ex a tristique convallis. Vivamus at urna rhoncus, aliquam dui ac, placerat tortor. Aenean sed condimentum nunc. In massa velit, laoreet non tempus sit amet, tincidunt ac ex. Curabitur ac hendrerit risus. Donec mi lorem, molestie vel volutpat vitae, faucibus in enim. Fusce turpis tortor, placerat a pharetra quis, sollicitudin nec purus.";

impl State<'_> {
    pub fn new(config: ChatGPTConfiguration) -> Self {
        Self {
            textarea: textfield::State::default(),
            current_focus: CurrentFocus::TextArea,
            config,
            history: Default::default(),
            partial: Default::default(),
            scroll_state: Default::default(),
            is_streaming: false,
        }
    }
}

#[derive(Debug)]
pub enum Action {
    Event(Event),
    TextField(textfield::Action),
    ScrollView(scroll_view::Action),
    BegunStreaming,
    StoppedStreaming,
    Delegated(Delegated),
    CommitMessage(ChatMessage),
    UpdatePartial(Vec<ChatMessage>),
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
}

#[derive(Default)]
pub struct Feature {}

impl tca::Reducer<State<'_>, Action> for Feature {
    fn reduce(&self, state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::Delegated(_) => Effect::none(),
            Action::CommitMessage(msg) => {
                state.partial = Default::default();
                state.history.push(msg);
                Effect::none()
            }
            Action::UpdatePartial(msg) => {
                state.partial = msg;
                Effect::none()
            }
            Action::ScrollView(scroll_view::Action::Delegated(delegated)) => match delegated {
                scroll_view::Delegated::Up => {
                    state.scroll_state.scroll.scroll_up();
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
                    let history = state.history.clone();
                    let message = state.textarea.textarea.lines().join("\n");
                    state.textarea = crate::textfield::State::default();

                    Effect::run(|send| {
                        async move {
                            if message.is_empty() {
                                return;
                            }
                            send.send(Action::BegunStreaming);
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
                        }
                        .boxed()
                    })
                }
            },
            Action::BegunStreaming => {
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

pub fn ui(frame: &mut Frame, area: Rect, state: &State) {
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
        let block = Block::new()
            .title(Title::from(crate::gpt::openai::display(msg.role) + " "))
            .borders(Borders::TOP)
            .border_type(ratatui::widgets::BorderType::Double)
            .border_style(Style::new().dark_gray());
        let inner = block.inner(sample);

        let paragraph = Paragraph::new(msg.content.clone())
            .wrap(Wrap { trim: true })
            .block(block);

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
        prev_y += height + 2;

        messages.push((paragraph, text_area))
    }

    let mut scroll_view = ScrollView::new(Size::new(
        width,
        messages.last().map_or(0, |rect| rect.1.bottom()),
    ));
    messages.into_iter().for_each(|(msg, rect)| {
        msg.render(rect, scroll_view.buf_mut());
    });

    let mut renderable_state = state.scroll_state.scroll;
    renderable_state.set_offset(ratatui::layout::Position {
        x: 0,
        y: std::cmp::min(renderable_state.offset().y, scroll_view.size().height),
    });

    frame.render_stateful_widget(scroll_view, navigation.inner(body), &mut renderable_state);
    let navigation_style = if state.current_focus == CurrentFocus::Chat {
        Style::new().blue()
    } else {
        Style::default()
    };
    frame.render_widget(navigation.border_style(navigation_style), body);
}
