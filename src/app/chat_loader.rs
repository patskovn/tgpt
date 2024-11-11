use ratatui::crossterm::event::Event;
use ratatui::{layout::Rect, widgets::Paragraph, Frame};
use tca::Effect;

use crate::{app::chat, app::navigation, gpt};

#[derive(Debug, Default, PartialEq, Clone)]
pub enum State<'a> {
    #[default]
    None,
    Chat(chat::State<'a>),
}

impl<'a> State<'a> {
    fn chat(&self) -> &chat::State<'a> {
        match &self {
            State::None => panic!("Wrong"),
            State::Chat(c) => c,
        }
    }
}

#[derive(Debug)]
pub enum Action {
    Event(Event),
    ReloadConfig,
    Chat(chat::Action),
    Delegated(Delegated),
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
    Quit,
}

#[derive(Default)]
pub struct Feature {}

impl tca::Reducer<State<'_>, Action> for Feature {
    fn reduce(state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::Event(e) => match state {
                State::None => Effect::send(Action::Delegated(Delegated::Noop(e))),
                State::Chat(_) => Effect::send(Action::Chat(chat::Action::Event(e))),
            },
            Action::Chat(chat::Action::Delegated(delegated)) => match delegated {
                chat::Delegated::Noop(e) => Effect::send(Action::Delegated(Delegated::Noop(e))),
                chat::Delegated::Quit => Effect::send(Action::Delegated(Delegated::Quit)),
            },
            Action::Chat(action) => match state {
                State::Chat(chat_state) => {
                    chat::Feature::reduce(chat_state, action).map(Action::Chat)
                }
                _ => panic!("Attempted to send {:#?} for {:#?} state", action, state),
            },
            Action::Delegated(_) => Effect::none(),
            Action::ReloadConfig => match gpt::openai::ChatGPTConfiguration::open() {
                Some(config) => match state {
                    State::None => {
                        *state = State::Chat(chat::State::new(config));
                        Effect::none()
                    }
                    State::Chat(ref mut chat) => {
                        chat.update_config(config);
                        Effect::none()
                    }
                },
                None => Effect::none(),
            },
        }
    }
}

pub fn ui(frame: &mut Frame, area: Rect, state: &State, store: tca::Store<State, Action>) {
    match state {
        State::None => {
            let navigation = navigation::ui(navigation::CurrentScreen::Chat);
            frame.render_widget(
                Paragraph::new("Chat is not configured. Please go to configuration tab.")
                    .block(navigation),
                area,
            );
        }
        State::Chat(_) => {
            chat::ui(frame, area, store.scope(|s| s.chat(), Action::Chat));
        }
    }
}
