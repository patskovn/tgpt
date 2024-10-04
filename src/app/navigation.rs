use anyhow::anyhow;
use core::fmt;
use ratatui::{
    layout::Alignment,
    style::{Style, Stylize},
    text::Line,
    widgets::{block::Title, Block, BorderType, Borders},
};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyModifiers};

use tca::Effect;

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub enum CurrentScreen {
    #[default]
    Chat,
    Config,
}

impl fmt::Display for CurrentScreen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CurrentScreen::Chat => f.write_str("AI"),
            CurrentScreen::Config => f.write_str("Configure"),
        }
    }
}

impl TryFrom<KeyCode> for CurrentScreen {
    type Error = anyhow::Error;
    fn try_from(value: KeyCode) -> anyhow::Result<Self, Self::Error> {
        match value {
            KeyCode::Char('1') => Ok(CurrentScreen::Chat),
            KeyCode::Char('2') => Ok(CurrentScreen::Config),
            _ => Err(anyhow!("Not a screen char")),
        }
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct State {
    pub current_screen: CurrentScreen,
}

#[derive(Debug)]
pub enum Action {
    Event(Event),
    Delegated(DelegatedAction),
}

#[derive(Debug)]
pub enum DelegatedAction {
    Noop,
    ChangeScreen(CurrentScreen),
    Exit,
}

#[derive(Default)]
pub struct NavigationReducer {}

impl tca::Reducer<State, Action> for NavigationReducer {
    fn reduce(_state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::Delegated(_) => Effect::none(),
            Action::Event(e) => match e {
                Event::Key(key) if key.kind != event::KeyEventKind::Release => match key.code {
                    KeyCode::Char('q') => Effect::send(Action::Delegated(DelegatedAction::Exit)),
                    KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => {
                        Effect::send(Action::Delegated(DelegatedAction::Exit))
                    }
                    _ => match CurrentScreen::try_from(key.code) {
                        Result::Ok(screen) => {
                            Effect::send(Action::Delegated(DelegatedAction::ChangeScreen(screen)))
                        }
                        Result::Err(_) => Effect::send(Action::Delegated(DelegatedAction::Noop)),
                    },
                },
                _ => Effect::send(Action::Delegated(DelegatedAction::Noop)),
            },
        }
    }
}

pub fn ui<'a>(current_screen: CurrentScreen) -> Block<'a> {
    Block::default()
        .title(title(CurrentScreen::Chat, current_screen, 1))
        .title(title(CurrentScreen::Config, current_screen, 2))
        .borders(Borders::all())
        .border_type(BorderType::Rounded)
}

fn title<'a>(screen: CurrentScreen, current_screen: CurrentScreen, index: u8) -> Title<'a> {
    let style = if screen == current_screen {
        Style::new().green()
    } else {
        Style::default()
    };
    Title::from(Line::from(format!("[{} {}]", index, screen)).style(style))
        .alignment(Alignment::Left)
}
