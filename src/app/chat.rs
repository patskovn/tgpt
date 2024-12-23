use std::sync::{Arc, RwLock};

use crate::editor::Mode;
use crossterm::event::{self, KeyModifiers};
use crossterm::event::{Event, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    Frame,
};
use tca::{Effect, Reducer};
use uuid::Uuid;

use crate::{app::conversation, gpt::openai::ChatGPTConfiguration};

use super::conversation_list::ConversationItem;
use super::{chat_sidebar, conversation_input, conversation_list};

#[derive(Debug, Copy, PartialEq, Clone, Default)]
pub enum CurrentFocus {
    #[default]
    TextArea,
    Conversation,
    Sidebar,
}

#[derive(Debug, Clone, Default)]
pub struct SharedFocus {
    value: Arc<RwLock<CurrentFocus>>,
}

impl PartialEq for SharedFocus {
    fn eq(&self, other: &Self) -> bool {
        self.value() == other.value()
    }
}

impl SharedFocus {
    pub fn value(&self) -> CurrentFocus {
        *self.value.read().unwrap()
    }

    fn new(value: CurrentFocus) -> Self {
        Self {
            value: Arc::new(RwLock::new(value)),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct State<'a> {
    sidebar: chat_sidebar::State<'a>,
    conversation: conversation::State,
    conversation_input: conversation_input::State<'a>,
    current_focus: SharedFocus,
}

impl Clone for State<'_> {
    fn clone(&self) -> Self {
        let focus = self.current_focus.value();
        let current_focus = SharedFocus::new(focus);
        Self {
            sidebar: chat_sidebar::State {
                current_focus: current_focus.clone(),
                ..self.sidebar.clone()
            },
            conversation: conversation::State {
                current_focus: current_focus.clone(),
                ..self.conversation.clone()
            },
            conversation_input: conversation_input::State {
                current_focus: current_focus.clone(),
                ..self.conversation_input.clone()
            },
            current_focus,
        }
    }
}

impl State<'_> {
    pub fn new(id: Uuid, config: ChatGPTConfiguration) -> Self {
        let current_focus = SharedFocus::new(CurrentFocus::default());
        Self {
            sidebar: chat_sidebar::State::new(current_focus.clone()),
            conversation: conversation::State::new(
                ConversationItem::new(id, "Fresh conversation".to_string(), 0),
                config,
                current_focus.clone(),
                vec![],
            ),
            conversation_input: conversation_input::State::new(current_focus.clone()),
            current_focus,
        }
    }

    pub fn update_config(&mut self, config: ChatGPTConfiguration) {
        self.conversation.config = config;
    }
}

#[derive(Debug)]
pub enum Action {
    Event(Event),
    Sidebar(chat_sidebar::Action),
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
            Action::Event(e) => match state.current_focus.value() {
                CurrentFocus::Conversation => {
                    Effect::send(Action::Conversation(conversation::Action::Event(e)))
                }
                CurrentFocus::TextArea => Effect::send(Action::ConversationInput(
                    conversation_input::Action::Event(e),
                )),
                CurrentFocus::Sidebar => {
                    Effect::send(Action::Sidebar(chat_sidebar::Action::Event(e)))
                }
            },
            Action::Sidebar(chat_sidebar::Action::Delegated(chat_sidebar::Delegated::Noop(e)))
            | Action::Conversation(conversation::Action::Delegated(
                conversation::Delegated::Noop(e),
            ))
            | Action::ConversationInput(conversation_input::Action::Delegated(
                conversation_input::Delegated::Noop(e),
            )) => try_toggle_focus(state, e),
            Action::Sidebar(chat_sidebar::Action::Delegated(delegated)) => match delegated {
                chat_sidebar::Delegated::Noop(e) => {
                    Effect::send(Action::Delegated(Delegated::Noop(e)))
                }
                chat_sidebar::Delegated::Select(history) => {
                    state.conversation = conversation::State::new(
                        history.0,
                        state.conversation.config.clone(),
                        state.current_focus.clone(),
                        history.1.history,
                    );
                    Effect::none()
                }
                chat_sidebar::Delegated::NewConversation => {
                    state.conversation = conversation::State::new(
                        ConversationItem::new(Uuid::new_v4(), "Fresh conversation".to_string(), 0),
                        state.conversation.config.clone(),
                        state.current_focus.clone(),
                        vec![],
                    );
                    Effect::none()
                }
            },
            Action::Sidebar(action) => {
                chat_sidebar::Feature::reduce(&mut state.sidebar, action).map(Action::Sidebar)
            }
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
                        state.conversation_input.reset();
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
                conversation::Delegated::ConversationTitleUpdated => Effect::send(Action::Sidebar(
                    chat_sidebar::Action::ConversationList(conversation_list::Action::Reload),
                )),
            },
            Action::Conversation(action) => {
                conversation::Feature::reduce(&mut state.conversation, action)
                    .map(Action::Conversation)
            }
            Action::Delegated(_) => Effect::none(),
        }
    }
}

fn try_toggle_focus(state: &mut State, event: Event) -> Effect<Action> {
    match event {
        Event::Key(KeyEvent {
            code: event::KeyCode::Tab,
            kind: event::KeyEventKind::Press,
            modifiers: KeyModifiers::NONE,
            ..
        }) => match state.current_focus.value() {
            CurrentFocus::TextArea
                if state.conversation_input.textarea.editor.mode != Mode::Normal =>
            {
                Effect::send(Action::ConversationInput(
                    conversation_input::Action::Event(event),
                ))
            }
            CurrentFocus::TextArea => {
                *state.current_focus.value.write().unwrap() = CurrentFocus::Sidebar;
                Effect::none()
            }
            CurrentFocus::Sidebar => {
                *state.current_focus.value.write().unwrap() = CurrentFocus::Conversation;
                Effect::none()
            }
            CurrentFocus::Conversation => {
                *state.current_focus.value.write().unwrap() = CurrentFocus::TextArea;
                Effect::none()
            }
        },
        Event::Key(KeyEvent {
            code: event::KeyCode::Char('1'),
            ..
        }) => {
            *state.current_focus.value.write().unwrap() = CurrentFocus::Sidebar;
            Effect::none()
        }
        Event::Key(KeyEvent {
            code: event::KeyCode::Char('2'),
            ..
        }) => {
            *state.current_focus.value.write().unwrap() = CurrentFocus::Conversation;
            Effect::none()
        }
        Event::Key(KeyEvent {
            code: event::KeyCode::Char('3'),
            ..
        }) => {
            *state.current_focus.value.write().unwrap() = CurrentFocus::TextArea;
            Effect::none()
        }
        _ => Effect::send(Action::Delegated(Delegated::Noop(event))),
    }
}

pub fn ui(frame: &mut Frame, area: Rect, store: tca::Store<State, Action>) {
    let with_conversation_list = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints(vec![Constraint::Length(32), Constraint::Fill(1)])
        .split(area);

    let sidebar_rect = with_conversation_list[0];
    let layout = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(vec![Constraint::Fill(1), Constraint::Max(10)])
        .split(with_conversation_list[1]);
    let conversation_rect = layout[0];
    let conversation_input_rect = layout[1];

    chat_sidebar::ui(
        frame,
        sidebar_rect,
        store.scope(|s| &s.sidebar, Action::Sidebar),
    );

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
