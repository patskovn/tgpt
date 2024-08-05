use crossterm::event::Event;
use ratatui::{layout::Rect, widgets::Paragraph, Frame};

use crate::{
    app::chat,
    app::navigation,
    gpt,
    tca::{self, Effect},
};

#[derive(Debug, Default, Eq, PartialEq, Clone)]
pub enum State<'a> {
    #[default]
    None,
    Chat(chat::State<'a>),
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
}

#[derive(Default)]
pub struct Feature {}

impl tca::Reducer<State<'_>, Action> for Feature {
    fn reduce<'effect>(&self, state: &mut State, action: Action) -> Effect<'effect, Action> {
        match action {
            Action::Event(e) => match state {
                State::None => Effect::send(Action::Delegated(Delegated::Noop(e))),
                State::Chat(_) => Effect::send(Action::Chat(chat::Action::Event(e))),
            },
            Action::Chat(chat::Action::Delegated(delegated)) => match delegated {
                chat::Delegated::Noop(e) => Effect::send(Action::Delegated(Delegated::Noop(e))),
            },
            Action::Chat(action) => match state {
                State::Chat(chat_state) => chat::Feature::default()
                    .reduce(chat_state, action)
                    .map(Action::Chat),
                _ => panic!("Attempted to send {:#?} for {:#?} state", action, state),
            },
            Action::Delegated(_) => Effect::none(),
            Action::ReloadConfig => match gpt::types::ChatGPTConfiguration::open() {
                Some(config) => {
                    *state = State::Chat(chat::State::new(config));
                    Effect::none()
                }
                None => Effect::none(),
            },
        }
    }
}

pub fn ui(frame: &mut Frame, area: Rect, state: &State) {
    match state {
        State::None => {
            let navigation = navigation::ui(navigation::CurrentScreen::Chat);
            frame.render_widget(
                Paragraph::new("Chat is not configured. Please go to configuration tab.")
                    .block(navigation),
                area,
            );
        }
        State::Chat(chat) => {
            chat::ui(frame, area, chat);
        }
    }
}
