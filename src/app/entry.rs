use crate::app::auth;
use crate::app::chat_loader;
use crate::app::navigation;
use crate::navigation::CurrentScreen;
use crossterm::event::KeyEvent;
use ratatui::crossterm::event::Event;
use ratatui::crossterm::event::KeyEventKind;
use ratatui::Frame;
use tca::Effect;
use tca::Store;

#[derive(Debug, PartialEq, Clone)]
pub struct State<'a> {
    pub navigation: navigation::State,
    pub chat: chat_loader::State<'a>,
    pub auth: auth::State<'a>,
    size: (u16, u16),
}

impl<'a> Default for State<'a> {
    fn default() -> Self {
        Self {
            navigation: navigation::State::default(),
            chat: chat_loader::State::default(),
            auth: auth::State::new(),
            size: Default::default(),
        }
    }
}

#[derive(Debug)]
pub enum Action {
    Event(Event),
    Chat(chat_loader::Action),
    Config(auth::Action),
    Navigation(navigation::Action),
}
pub struct Feature {}

impl Feature {
    pub fn default() -> Self {
        Self {}
    }
}

impl tca::Reducer<State<'_>, Action> for Feature {
    fn reduce(state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::Chat(chat_loader::Action::Delegated(chat_loader::Delegated::Noop(e)))
            | Action::Config(auth::Action::Delegated(auth::Delegated::Noop(e))) => {
                navigation::NavigationReducer::reduce(
                    &mut state.navigation,
                    navigation::Action::Event(e),
                )
                .map(Action::Navigation)
            }
            Action::Config(action) => {
                auth::AuthReducer::reduce(&mut state.auth, action).map(Action::Config)
            }
            Action::Chat(action) => {
                chat_loader::Feature::reduce(&mut state.chat, action).map(Action::Chat)
            }
            Action::Event(e) => match e {
                Event::Paste(_)
                | Event::Key(KeyEvent {
                    kind: KeyEventKind::Press | KeyEventKind::Release,
                    ..
                }) => match state.navigation.current_screen {
                    CurrentScreen::Chat => {
                        Effect::send(Action::Chat(chat_loader::Action::Event(e)))
                    }
                    CurrentScreen::Config => Effect::send(Action::Config(auth::Action::Event(e))),
                },
                Event::Resize(w, h) => {
                    state.size = (w, h);
                    Effect::none()
                }
                _ => Effect::none(),
            },
            Action::Navigation(action) => match action {
                navigation::Action::Delegated(delegated) => match delegated {
                    navigation::DelegatedAction::Noop => Effect::none(),
                    navigation::DelegatedAction::ChangeScreen(screen) => {
                        state.navigation.current_screen = screen;
                        match screen {
                            CurrentScreen::Chat => {
                                Effect::send(Action::Chat(chat_loader::Action::ReloadConfig))
                            }
                            CurrentScreen::Config => Effect::none(),
                        }
                    }
                    navigation::DelegatedAction::Exit => Effect::quit(),
                },
                _ => Effect::none(),
            },
        }
    }
}

pub fn ui(frame: &mut Frame, state: &State, store: Store<State, Action>) {
    chat_loader::ui(
        frame,
        frame.area(),
        &state.chat,
        store.scope(|s| &s.chat, Action::Chat),
    )
}
