use crossterm::event::{Event, KeyCode};
use log::debug;
use ratatui::{layout::Rect, widgets::Block, Frame};

use crate::{
    tca::{self, Effect},
    textfield,
};

#[derive(Debug, Default, Eq, PartialEq)]
pub struct State<'a> {
    pub textarea: textfield::State<'a>,
}

impl<'a> State<'a> {
    pub fn new(block: Block<'a>) -> Self {
        Self {
            textarea: textfield::State::new(block),
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
    Enter,
    Exit,
}

#[derive(Default)]
pub struct Feature {}

impl tca::Reducer<State<'_>, Action> for Feature {
    fn reduce(&self, state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::Delegated(_) => Effect::none(),
            Action::TextField(textfield::Action::Delegated(delegated)) => match delegated {
                textfield::Delegated::Updated => {
                    if state.textarea.textarea.lines().len() > 1 {
                        state
                            .textarea
                            .textarea
                            .move_cursor(tui_textarea::CursorMove::Jump(0, u16::max_value()));
                        state.textarea.textarea.delete_str(usize::max_value());
                    }
                    Effect::none()
                }
                textfield::Delegated::Quit => Effect::send(Action::Delegated(Delegated::Exit)),
                textfield::Delegated::Noop(e) => match e {
                    Event::Key(key) => match key.code {
                        KeyCode::Enter => Effect::send(Action::Delegated(Delegated::Enter)),
                        _ => Effect::send(Action::Delegated(Delegated::Noop(e))),
                    },
                    _ => Effect::send(Action::Delegated(Delegated::Noop(e))),
                },
            },
            Action::TextField(action) => textfield::Feature::default()
                .reduce(&mut state.textarea, action)
                .map(Action::TextField),
            Action::Event(e) => Effect::send(Action::TextField(textfield::Action::Event(e))),
        }
    }
}

pub fn ui(frame: &mut Frame, area: Rect, state: &State) {
    frame.render_widget(state.textarea.widget(), area);
}
