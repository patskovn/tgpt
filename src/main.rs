mod app;
mod editor;
mod gpt;
mod list;
mod panic_handler;
mod single_line_input;
mod tca;
mod textfield;
mod uiutils;

use crate::app::navigation;
use std::fs::File;
use std::io::{self};

use crate::tca::Effect;
use anyhow::Context;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::Backend;
use ratatui::{backend::CrosstermBackend, Terminal};
use simplelog::{CombinedLogger, WriteLogger};

use crate::app::entry::ui;
use crate::app::entry::Action;
use crate::app::entry::Feature;
use crate::app::entry::State;

fn configure_logger() -> anyhow::Result<()> {
    CombinedLogger::init(vec![WriteLogger::new(
        log::LevelFilter::Debug,
        simplelog::Config::default(),
        File::create(".tgpt.latest.log").unwrap(),
    )])
    .context("Failed to configure logging")
}

async fn event_loop<B: Backend>(terminal: &mut Terminal<B>) -> anyhow::Result<()> {
    let mut terminal_events = crossterm::event::EventStream::new();

    let reducer = Feature::default();
    let store = tca::Store::new(State::default(), reducer);
    store.send(Action::Navigation(navigation::Action::Delegated(
        navigation::DelegatedAction::ChangeScreen(navigation::CurrentScreen::Chat),
    )));
    store
        .run(
            |state| {
                let _ = terminal.draw(|f| ui(f, state));
            },
            Action::Event,
            &mut terminal_events,
        )
        .await;

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    panic_handler::initialize_panic_handler()?;
    configure_logger()?;
    enable_raw_mode()?;

    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    event_loop(&mut terminal).await?;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
