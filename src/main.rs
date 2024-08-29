use futures::{FutureExt, StreamExt};
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
use std::fs::File;
use std::io::{self};

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
use tca::ChangeObserver;

fn configure_logger() -> anyhow::Result<()> {
    CombinedLogger::init(vec![WriteLogger::new(
        log::LevelFilter::Debug,
        simplelog::Config::default(),
        File::create(".tgpt.latest.log").unwrap(),
    )])
    .context("Failed to configure logging")
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
                Err(err) => return Err(err.into()),
                }
            }
            maybe_event = crossterm_event => {
                match maybe_event {
                    Some(Ok(evt)) => store.send(Action::Event(evt)),
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
