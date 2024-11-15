use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use tca::Effect;

type State = ();

#[derive(Debug)]
pub enum Action {
    Event(Event),
    Delegated(Delegated),
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
    Up,
    UpMore,
    Down,
    DownMore,
    Left,
    Right,
}

pub struct Feature {}

impl tca::Reducer<State, Action> for Feature {
    fn reduce(_state: &mut State, action: Action) -> tca::Effect<Action> {
        match action {
            Action::Event(e) => match e {
                Event::Key(KeyEvent {
                    code: KeyCode::Char('u'),
                    modifiers,
                    ..
                }) if modifiers.contains(KeyModifiers::CONTROL) => {
                    Effect::send(Action::Delegated(Delegated::UpMore))
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char('d'),
                    modifiers,
                    ..
                }) if modifiers.contains(KeyModifiers::CONTROL) => {
                    Effect::send(Action::Delegated(Delegated::DownMore))
                }
                Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                    KeyCode::Char('h') => Effect::send(Action::Delegated(Delegated::Left)),
                    KeyCode::Char('j') => Effect::send(Action::Delegated(Delegated::Down)),
                    KeyCode::Char('k') => Effect::send(Action::Delegated(Delegated::Up)),
                    KeyCode::Char('l') => Effect::send(Action::Delegated(Delegated::Right)),
                    _ => Effect::send(Action::Delegated(Delegated::Noop(e))),
                },
                _ => Effect::send(Action::Delegated(Delegated::Noop(e))),
            },
            Action::Delegated(_) => Effect::none(),
        }
    }
}
