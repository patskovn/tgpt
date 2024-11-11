use std::sync::Arc;

use crossterm::event::Event;
use ratatui::{
    layout::Rect,
    style::{Style, Stylize},
    text::Text,
    Frame,
};
use tca::{Effect, Reducer};

use super::{chat::CurrentFocus, navigation};

#[derive(Debug, PartialEq, Default, Clone)]
pub struct State {
    pub current_focus: Arc<CurrentFocus>,
    pub _something: bool,
}

impl State {
    pub fn new(current_focus: Arc<CurrentFocus>) -> Self {
        Self {
            current_focus,
            ..Default::default()
        }
    }
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

pub struct Feature {}

impl Reducer<State, Action> for Feature {
    fn reduce(_state: &mut State, action: Action) -> tca::Effect<Action> {
        match action {
            Action::Event(e) => Effect::send(Action::Delegated(Delegated::Noop(e))),
            Action::Delegated(_) => Effect::none(),
        }
    }
}

pub fn ui(frame: &mut Frame, area: Rect, store: tca::Store<State, Action>) {
    let navigation = navigation::ui(navigation::CurrentScreen::Chat);
    let state = store.state();
    let navigation_style = if *state.current_focus == CurrentFocus::ConversationList {
        Style::new().green()
    } else {
        Style::default()
    };
    let text = Text::from("Hello");
    frame.render_widget(text, navigation.inner(area));
    frame.render_widget(navigation.border_style(navigation_style), area);
}
