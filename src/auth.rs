use std::default;

use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

use crate::{
    gpt, list, navigation,
    tca::{Effect, Reducer},
};

#[derive(Debug)]
pub struct State {
    providers: list::State<gpt::Provider>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            providers: list::State::new(vec![gpt::Provider::OpenAI]),
        }
    }
}

impl State {
    pub fn new() -> Self {
        let mut val = Self::default();
        val.update_config();
        val
    }

    pub fn update_config(&mut self) {}
}

#[derive(Debug)]
pub enum Action {
    Event(Event),
    List(list::Action),
    Delegated(Delegated),
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
}

#[derive(Default)]
pub struct AuthReducer {}
impl Reducer<State, Action> for AuthReducer {
    fn reduce(&self, state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::List(list::Action::Delegated(delegated)) => match delegated {
                list::Delegated::Noop(e) => Effect::send(Action::Delegated(Delegated::Noop(e))),
                list::Delegated::Enter(_) => Effect::none(),
                list::Delegated::Toogle(_) => Effect::none(),
            },
            Action::List(action) => list::ListFeature::default()
                .reduce(&mut state.providers, action)
                .map(Action::List),
            Action::Delegated(_) => Effect::none(),
            Action::Event(e) => Effect::send(Action::List(list::Action::Event(e))),
        }
    }
}

pub fn ui(frame: &mut Frame, area: Rect, state: &State) {
    let navigation = navigation::ui(navigation::CurrentScreen::Config);
    list::ui(frame, navigation.inner(area), &state.providers);
    frame.render_widget(navigation, area);
}

#[derive(Debug)]
struct ChatGPTSelectionState {}
