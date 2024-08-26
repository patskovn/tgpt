use crate::app::auth;
use crate::app::chat_loader;
use crate::app::navigation;
use crate::navigation::CurrentScreen;
use crossterm::event::Event;
use crossterm::event::KeyEventKind;
use ratatui::Frame;
use tca::Effect;
use tca::Store;

#[derive(Debug, PartialEq, Clone)]
pub struct State<'a> {
    pub navigation: navigation::State,
    pub chat: chat_loader::State<'a>,
    pub auth: auth::State<'a>,
}

impl<'a> Default for State<'a> {
    fn default() -> Self {
        Self {
            navigation: navigation::State::default(),
            chat: chat_loader::State::default(),
            auth: auth::State::new(),
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
    fn reduce(&self, state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::Chat(chat_loader::Action::Delegated(chat_loader::Delegated::Noop(e)))
            | Action::Config(auth::Action::Delegated(auth::Delegated::Noop(e))) => {
                navigation::NavigationReducer::default()
                    .reduce(&mut state.navigation, navigation::Action::Event(e))
                    .map(Action::Navigation)
            }
            Action::Config(action) => auth::AuthReducer::default()
                .reduce(&mut state.auth, action)
                .map(Action::Config),
            Action::Chat(action) => chat_loader::Feature::default()
                .reduce(&mut state.chat, action)
                .map(Action::Chat),
            Action::Event(e) => match e {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    match state.navigation.current_screen {
                        CurrentScreen::Chat => {
                            Effect::send(Action::Chat(chat_loader::Action::Event(e)))
                        }
                        CurrentScreen::Config => {
                            Effect::send(Action::Config(auth::Action::Event(e)))
                        }
                    }
                }
                _ => Effect::none(),
            },
            Action::Navigation(action) => match action {
                navigation::Action::Delegated(delegated) => match delegated {
                    navigation::DelegatedAction::Noop(_) => Effect::none(),
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
    match state.navigation.current_screen {
        CurrentScreen::Chat => chat_loader::ui(
            frame,
            frame.size(),
            &state.chat,
            store.scope(|s| &s.chat, Action::Chat),
        ),
        CurrentScreen::Config => auth::ui(frame, frame.size(), &state.auth),
    }
}
