mod auth;
mod editor;
mod tca;
mod textfield;
mod uiutils;

use std::collections::HashMap;
use std::fs::File;
use std::io::{self};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use crossterm::execute;
use crossterm::{
    event::{self},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use ratatui::{backend::CrosstermBackend, Terminal};
use simplelog::{CombinedLogger, WriteLogger};
use tca::Effect;
use textfield::TextFieldReducer;

pub enum CurrentScreen {
    Main,
    Exiting,
}

pub enum CurrentlyEditing {
    Key,
    Value,
}

pub struct State<'a> {
    // The representation of our key and value pairs with serde Serialize support
    pub current_screen: CurrentScreen, // the current screen the user is looking at, and will later determine what is rendered.
    pub currently_editing: Option<CurrentlyEditing>,
    pub key_input: String, // the currently being edited json value.
    pub pairs: HashMap<String, String>, // the optional state containing which of the key or value pair the user is editing. It is an option, because when the user is not directly editing a key-value pair, this will be set to `None`.
    pub textarea: textfield::State<'a>,
    pub value_input: String,
}

impl<'a> Default for State<'a> {
    fn default() -> Self {
        Self {
            current_screen: CurrentScreen::Main,
            currently_editing: None,
            key_input: String::new(),
            pairs: HashMap::new(),
            value_input: String::new(),
            textarea: textfield::State::default(),
        }
    }
}

impl<'a> State<'a> {
    fn save_key_value(&mut self) {
        self.pairs
            .insert(self.key_input.clone(), self.value_input.clone());
        self.key_input = String::new();
        self.value_input = String::new();
        self.currently_editing = None;
    }

    pub fn toggle_editing(&mut self) {
        if let Some(edit_mode) = &self.currently_editing {
            match edit_mode {
                CurrentlyEditing::Key => self.currently_editing = Some(CurrentlyEditing::Value),
                CurrentlyEditing::Value => self.currently_editing = Some(CurrentlyEditing::Key),
            }
        }
    }

    pub fn print_json(&self) -> anyhow::Result<()> {
        let output = serde_json::to_string(&self.pairs)?;
        println!("{}", output);
        Ok(())
    }
}

#[derive(Debug)]
enum AppAction {
    Event(Event),
    TextField(textfield::Action),
}
struct AppFeature {}

impl AppFeature {
    fn default() -> Self {
        Self {}
    }
}

impl tca::Reducer<State<'_>, AppAction> for AppFeature {
    fn reduce(&self, state: &mut State, action: AppAction) -> Effect<AppAction> {
        match action {
            AppAction::TextField(action) => match action {
                textfield::Action::Delegated(delegated) => match delegated {
                    textfield::DelegatedAction::Quit => {
                        state.current_screen = CurrentScreen::Exiting;
                        Effect::none()
                    }
                },
                _ => TextFieldReducer::new()
                    .reduce(&mut state.textarea, action)
                    .map(AppAction::TextField),
            },
            AppAction::Event(e) => match e {
                Event::Key(key) if key.kind != event::KeyEventKind::Release => {
                    match state.current_screen {
                        CurrentScreen::Main => {
                            Effect::send(AppAction::TextField(textfield::Action::Event(e)))
                        }
                        CurrentScreen::Exiting => match key.code {
                            KeyCode::Char('y') | KeyCode::Char('q') => Effect::quit(),
                            KeyCode::Char('n') => {
                                state.current_screen = CurrentScreen::Main;
                                Effect::none()
                            }
                            _ => Effect::none(),
                        },
                    }
                }
                _ => Effect::none(),
            },
        }
    }
}

type AppStore<'a> = tca::Store<AppFeature, State<'a>, AppAction>;

fn ui(frame: &mut Frame, store: &AppStore) {
    store.with_state(|state| match state.current_screen {
        CurrentScreen::Main => ui_main_screen(frame, state),
        CurrentScreen::Exiting => ui_exit_screen(frame),
    })
}

fn ui_main_screen(frame: &mut Frame, state: &State) {
    let layout = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(vec![Constraint::Fill(1), Constraint::Max(10)])
        .split(frame.size());

    frame.render_widget(
        Paragraph::new("Hello, world!").block(Block::bordered()),
        layout[0],
    );

    frame.render_widget(state.textarea.widget(), layout[1]);
}

fn ui_exit_screen(frame: &mut Frame) {
    frame.render_widget(Clear, frame.size());
    let popup = Block::default().title("Exit").borders(Borders::all());
    let exit_text = Text::styled("Quit application? (q/y/n)", Style::default().fg(Color::Red));
    // the `trim: false` will stop the text from being cut off when over the edge of the block
    let exit_paragraph = Paragraph::new(exit_text)
        .block(popup)
        .wrap(Wrap { trim: false });

    let area = uiutils::centered_rect(frame.size(), 60, 25);
    frame.render_widget(exit_paragraph, area);
}

pub(crate) fn main() -> anyhow::Result<()> {
    CombinedLogger::init(vec![WriteLogger::new(
        log::LevelFilter::Debug,
        simplelog::Config::default(),
        File::create(".tgpt.latest.log").unwrap(),
    )])
    .unwrap();

    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let reducer = AppFeature::default();
    let mut store = AppStore::new(State::default(), reducer);

    loop {
        terminal.draw(|f| ui(f, &store))?;

        let event = event::read()?;
        store.send(AppAction::Event(event));
        if store.quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
