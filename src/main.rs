use anyhow::anyhow;
use crossterm::event::DisableBracketedPaste;
use crossterm::event::EnableBracketedPaste;
use crossterm::event::Event;
use dirs::home_dir;
use futures::FutureExt;
use futures::StreamExt;
use tca::ActionSender;
mod app;
mod editor;
mod gpt;
mod list;
mod panic_handler;
mod scroll_view;
mod single_line_input;
mod textfield;
mod uiutils;
mod utils;

use crate::app::navigation;
use std::io::{self};
use std::path::PathBuf;

use anyhow::Context;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::Backend;
use ratatui::{backend::CrosstermBackend, Terminal};
use simplelog::{CombinedLogger, WriteLogger};

use crate::app::entry::ui;
use crate::app::entry::Action;
use crate::app::entry::Feature;
use crate::app::entry::State;
use std::fs::{create_dir_all, File};

use tca::ChangeObserver;

fn configure_logger() -> anyhow::Result<()> {
    CombinedLogger::init(vec![WriteLogger::new(
        log::LevelFilter::Debug,
        simplelog::Config::default(),
        create_log_file()?,
    )])
    .context("Failed to configure logging")
}

fn create_log_file() -> anyhow::Result<File> {
    let home = home_dir().ok_or_else(|| anyhow!("Failed to find home directory"))?;
    create_file_with_dirs(&home.join(".tgpt").join("latest.log"))
}

fn create_file_with_dirs(path: &PathBuf) -> anyhow::Result<File> {
    // Create all directories in the specified path
    let parent = std::path::Path::new(path)
        .parent()
        .ok_or_else(|| anyhow!("Failed to find configuration path"))?;
    create_dir_all(parent)?;
    // Create the file
    let file = File::create(path)?;
    Ok(file)
}

fn fixup_event(event: Event) -> Event {
    match event {
        Event::Paste(paste) => Event::Paste(paste.replace('\r', "\n")),
        _ => event,
    }
}

async fn event_loop<B: Backend>(terminal: &mut Terminal<B>) -> anyhow::Result<()> {
    let store = tca::Store::new::<Feature>(State::default());
    store.send(Action::Navigation(navigation::Action::Delegated(
        navigation::DelegatedAction::ChangeScreen(navigation::CurrentScreen::Chat),
    )));

    let mut redraw_events = store.observe();
    let mut terminal_events = crossterm::event::EventStream::new();

    loop {
        let crossterm_event = terminal_events.next().fuse();
        let redraw_event = redraw_events.recv().fuse();
        tokio::select! {
            maybe_redraw = redraw_event => {
                match maybe_redraw {
                Ok(()) => {
                    let state = store.state();
                    log::debug!("Render!");
                    terminal.draw(|f| ui(f, &state, store.clone()))?;
                },
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // We can afford lagging behind, we will just redraw on next step
                    continue;
                },
                }
            }
            maybe_event = crossterm_event => {
                match maybe_event {
                    Some(Ok(evt)) => store.send(Action::Event(fixup_event(evt))),
                    Some(Err(err)) => return Err(err.into()),
                    None => continue,
                }
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    panic_handler::initialize_panic_handler()?;
    configure_logger()?;
    enable_raw_mode()?;

    let mut stderr = io::stderr();
    execute!(
        stderr,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    event_loop(&mut terminal).await?;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableBracketedPaste,
    )?;
    terminal.show_cursor()?;

    Ok(())
}
