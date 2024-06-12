use crossterm::event::Event;
use ratatui::{
    layout::{Constraint, Layout},
    widgets::{Block, Paragraph},
    Frame,
};

use crate::{
    navigation,
    tca::{Effect, Reducer},
};

#[derive(Debug, Default)]
pub struct State {}

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
            Action::Delegated(_) => Effect::none(),
            Action::Event(e) => Effect::send(Action::Delegated(Delegated::Noop(e))),
        }
    }
}

pub fn ui(frame: &mut Frame, state: &State) {
    let navigation = navigation::ui(navigation::CurrentScreen::Config);
    let layout = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(vec![Constraint::Fill(1), Constraint::Max(10)])
        .split(frame.size());

    frame.render_widget(Paragraph::new("Config").block(navigation), layout[0]);
}

#[derive(Debug)]
struct ChatGPTSelectionState {}
