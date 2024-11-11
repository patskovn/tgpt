use std::sync::Arc;

use crossterm::event::Event;
use ratatui::style::{Style, Stylize};
use ratatui::{layout::Rect, Frame};
use tca::{Effect, Reducer};

use crate::textfield;

use super::chat::CurrentFocus;

#[derive(Debug, PartialEq, Clone)]
pub struct State<'a> {
    pub textarea: textfield::State<'a>,
    pub current_focus: Arc<CurrentFocus>,
}

impl State<'_> {
    pub fn new(current_focus: Arc<CurrentFocus>) -> Self {
        Self {
            textarea: Default::default(),
            current_focus,
        }
    }
}

#[derive(Debug)]
pub enum Action {
    Event(Event),
    TextField(textfield::Action),
    Delegated(Delegated),
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
    Commit(String),
    Quit,
}

pub struct Feature {}

impl Reducer<State<'_>, Action> for Feature {
    fn reduce(state: &mut State, action: Action) -> tca::Effect<Action> {
        match action {
            Action::Event(e) => Effect::send(Action::TextField(textfield::Action::Event(e))),
            Action::TextField(textfield::Action::Delegated(delegated)) => match delegated {
                textfield::Delegated::Noop(e) => {
                    Effect::send(Action::Delegated(Delegated::Noop(e)))
                }
                textfield::Delegated::Commit => Effect::send(Action::Delegated(Delegated::Commit(
                    state.textarea.textarea.lines().join("\n"),
                ))),
                textfield::Delegated::Updated => Effect::none(),
                textfield::Delegated::Quit => Effect::send(Action::Delegated(Delegated::Quit)),
            },
            Action::TextField(action) => {
                textfield::Feature::reduce(&mut state.textarea, action).map(Action::TextField)
            }
            Action::Delegated(_) => Effect::none(),
        }
    }
}

pub fn ui(frame: &mut Frame, area: Rect, store: tca::Store<State, Action>) {
    let state = store.state();
    let mut cloned_area = state.textarea.clone();
    if *state.current_focus == CurrentFocus::TextArea {
        if let Some(block) = cloned_area.textarea.block() {
            cloned_area
                .textarea
                .set_block(block.clone().border_style(Style::new().green()))
        }
    };
    frame.render_widget(cloned_area.widget(), area);
}
