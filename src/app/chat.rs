use crate::editor::Mode;
use crossterm::event::{self, KeyModifiers};
use crossterm::event::{Event, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    Frame,
};
use tca::{Effect, Reducer};

use crate::{app::conversation, gpt::openai::ChatGPTConfiguration};

use super::conversation_input;

#[derive(Debug, Copy, PartialEq, Clone, Default)]
enum CurrentFocus {
    #[default]
    TextArea,
    Conversation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct State<'a> {
    conversation: conversation::State,
    conversation_input: conversation_input::State<'a>,
    current_focus: CurrentFocus,
}

impl State<'_> {
    pub fn new(config: ChatGPTConfiguration) -> Self {
        Self {
            conversation: conversation::State::new(config),
            conversation_input: conversation_input::State {
                focused: true,
                ..Default::default()
            },
            current_focus: Default::default(),
        }
    }

    pub fn update_config(&mut self, config: ChatGPTConfiguration) {
        self.conversation.config = config;
    }

    fn update_focus(&mut self, focus: CurrentFocus) {
        self.current_focus = focus;
        self.conversation_input.focused = false;
        self.conversation.focused = false;
        match focus {
            CurrentFocus::TextArea => self.conversation_input.focused = true,
            CurrentFocus::Conversation => self.conversation.focused = true,
        }
    }
}

#[derive(Debug)]
pub enum Action {
    Event(Event),
    Conversation(conversation::Action),
    ConversationInput(conversation_input::Action),
    Delegated(Delegated),
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
    Quit,
}

pub struct Feature {}

impl Reducer<State<'_>, Action> for Feature {
    fn reduce(state: &mut State, action: Action) -> tca::Effect<Action> {
        match action {
            Action::Event(e) => match e {
                Event::Key(KeyEvent {
                    code: event::KeyCode::Tab,
                    kind: event::KeyEventKind::Press,
                    modifiers: KeyModifiers::NONE,
                    ..
                }) if state.current_focus != CurrentFocus::TextArea
                    || state.conversation_input.textarea.editor.mode == Mode::Normal =>
                {
                    if state.current_focus == CurrentFocus::TextArea {
                        state.update_focus(CurrentFocus::Conversation);
                    } else {
                        state.update_focus(CurrentFocus::TextArea);
                    };
                    Effect::none()
                }
                _ => match state.current_focus {
                    CurrentFocus::Conversation => {
                        Effect::send(Action::Conversation(conversation::Action::Event(e)))
                    }
                    CurrentFocus::TextArea => Effect::send(Action::ConversationInput(
                        conversation_input::Action::Event(e),
                    )),
                },
            },
            Action::ConversationInput(conversation_input::Action::Delegated(delegated)) => {
                match delegated {
                    conversation_input::Delegated::Quit => {
                        Effect::send(Action::Delegated(Delegated::Quit))
                    }
                    conversation_input::Delegated::Noop(e) => {
                        Effect::send(Action::Delegated(Delegated::Noop(e)))
                    }
                    conversation_input::Delegated::Commit(message) => {
                        if message.is_empty() || state.conversation.is_streaming {
                            return Effect::none();
                        }
                        state.conversation_input.textarea = crate::textfield::State::default();
                        Effect::send(Action::Conversation(conversation::Action::NewMessage(
                            message,
                        )))
                    }
                }
            }
            Action::ConversationInput(action) => {
                conversation_input::Feature::reduce(&mut state.conversation_input, action)
                    .map(Action::ConversationInput)
            }
            Action::Conversation(conversation::Action::Delegated(delegated)) => match delegated {
                conversation::Delegated::Noop(e) => {
                    Effect::send(Action::Delegated(Delegated::Noop(e)))
                }
            },
            Action::Conversation(action) => {
                conversation::Feature::reduce(&mut state.conversation, action)
                    .map(Action::Conversation)
            }
            Action::Delegated(_) => Effect::none(),
        }
    }
}

pub fn ui(frame: &mut Frame, area: Rect, store: tca::Store<State, Action>) {
    let layout = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(vec![Constraint::Fill(1), Constraint::Max(10)])
        .split(area);
    let conversation_rect = layout[0];
    let conversation_input_rect = layout[1];

    conversation::ui(
        frame,
        conversation_rect,
        store.scope(|s| &s.conversation, Action::Conversation),
    );

    conversation_input::ui(
        frame,
        conversation_input_rect,
        store.scope(|s| &s.conversation_input, Action::ConversationInput),
    );
}
