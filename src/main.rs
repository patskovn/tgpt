mod auth;
mod chat;
mod chat_gpt_configuration;
mod chat_loader;
mod editor;
mod gpt;
mod list;
mod navigation;
mod panic_handler;
mod single_line_input;
mod tca;
mod textfield;
mod uiutils;

use crate::navigation::CurrentScreen;
use std::fs::File;
use std::io::{self};

use crate::tca::Effect;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::{
    event::{self},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::Frame;
use ratatui::{backend::CrosstermBackend, Terminal};
use simplelog::{CombinedLogger, WriteLogger};

#[derive(Debug, Eq, PartialEq, Clone)]
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
enum AppAction {
    Event(Event),
    Chat(chat_loader::Action),
    Config(auth::Action),
    Navigation(navigation::Action),
}
struct AppFeature {}

impl AppFeature {
    fn default() -> Self {
        Self {}
    }
}

impl tca::Reducer<State<'_>, AppAction> for AppFeature {
    fn reduce<'effect>(&self, state: &mut State, action: AppAction) -> Effect<'effect, AppAction> {
        match action {
            AppAction::Chat(chat_loader::Action::Delegated(chat_loader::Delegated::Noop(e)))
            | AppAction::Config(auth::Action::Delegated(auth::Delegated::Noop(e))) => {
                navigation::NavigationReducer::default()
                    .reduce(&mut state.navigation, navigation::Action::Event(e))
                    .map(AppAction::Navigation)
            }
            AppAction::Config(action) => auth::AuthReducer::default()
                .reduce(&mut state.auth, action)
                .map(AppAction::Config),
            AppAction::Chat(action) => chat_loader::Feature::default()
                .reduce(&mut state.chat, action)
                .map(AppAction::Chat),
            AppAction::Event(e) => match e {
                Event::Key(key) if key.kind != event::KeyEventKind::Release => {
                    match state.navigation.current_screen {
                        CurrentScreen::Chat => {
                            Effect::send(AppAction::Chat(chat_loader::Action::Event(e)))
                        }
                        CurrentScreen::Config => {
                            Effect::send(AppAction::Config(auth::Action::Event(e)))
                        }
                    }
                }
                _ => Effect::none(),
            },
            AppAction::Navigation(action) => match action {
                navigation::Action::Delegated(delegated) => match delegated {
                    navigation::DelegatedAction::Noop(_) => Effect::none(),
                    navigation::DelegatedAction::ChangeScreen(screen) => {
                        state.navigation.current_screen = screen;
                        match screen {
                            CurrentScreen::Chat => {
                                Effect::send(AppAction::Chat(chat_loader::Action::ReloadConfig))
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

type AppStore<'a> = tca::Store<AppFeature, State<'a>, AppAction>;

fn ui(frame: &mut Frame, state: State) {
    match state.navigation.current_screen {
        CurrentScreen::Chat => chat_loader::ui(frame, frame.size(), &state.chat),
        CurrentScreen::Config => auth::ui(frame, frame.size(), &state.auth),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    CombinedLogger::init(vec![WriteLogger::new(
        log::LevelFilter::Debug,
        simplelog::Config::default(),
        File::create(".tgpt.latest.log").unwrap(),
    )])
    .unwrap();
    panic_handler::initialize_panic_handler()?;

    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut terminal_events = crossterm::event::EventStream::new();

    let reducer = AppFeature::default();
    let store = AppStore::new(State::default(), reducer);
    store.send(AppAction::Navigation(navigation::Action::Delegated(
        navigation::DelegatedAction::ChangeScreen(CurrentScreen::Chat),
    )));
    store
        .run(
            |state| {
                log::debug!("Redrawing! {:#?}", state.chat);
                let _ = terminal.draw(|f| ui(f, state));
            },
            AppAction::Event,
            &mut terminal_events,
        )
        .await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
