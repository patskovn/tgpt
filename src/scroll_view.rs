use crossterm::event::{Event, KeyCode, KeyEventKind};
use tui_scrollview::ScrollViewState;

use tca::{self, Effect};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct State {
    pub scroll: ScrollViewState,
}

#[derive(Debug)]
pub enum Action {
    Event(Event),
    Delegated(Delegated),
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
    Up,
    Down,
}

#[derive(Default)]
pub struct Feature {}

impl tca::Reducer<State, Action> for Feature {
    fn reduce(_state: &mut State, action: Action) -> tca::Effect<Action> {
        match action {
            Action::Event(e) => match e {
                Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                    KeyCode::Char('j') => Effect::send(Action::Delegated(Delegated::Down)),
                    KeyCode::Char('k') => Effect::send(Action::Delegated(Delegated::Up)),
                    _ => Effect::send(Action::Delegated(Delegated::Noop(e))),
                },
                _ => Effect::send(Action::Delegated(Delegated::Noop(e))),
            },
            Action::Delegated(_) => Effect::none(),
        }
    }
}
