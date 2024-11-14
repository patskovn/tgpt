use ratatui::crossterm::event::Event;
use ratatui::widgets::Block;
use tui_textarea::TextArea;

use crate::editor::{self, Mode, Transition, Vim};
use tca::Effect;

#[derive(Debug)]
pub enum Action {
    Event(Event),
    Delegated(Delegated),
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
    Commit,
    Updated,
    Quit,
}

#[derive(Debug, Clone)]
pub struct State<'a> {
    pub editor: Vim,
    pub textarea: TextArea<'a>,
    title: Option<String>,
    block: Option<Block<'a>>,
}

impl PartialEq for State<'_> {
    fn eq(&self, other: &Self) -> bool {
        let areas_eq = self.textarea.lines() == other.textarea.lines();
        let cursor_eq = self.textarea.cursor() == other.textarea.cursor();
        let alignment_eq = self.textarea.alignment() == other.textarea.alignment();
        self.editor == other.editor
            && self.block == other.block
            && areas_eq
            && cursor_eq
            && alignment_eq
    }
}
impl Eq for State<'_> {}

impl<'a> State<'a> {
    pub fn widget(&'a self) -> &TextArea<'a> {
        &self.textarea
    }

    pub fn new(block: Block<'a>) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_block(block);
        textarea.set_cursor_style(Mode::Normal.cursor_style());
        Self {
            editor: Vim::new(editor::Mode::Normal),
            textarea,
            block: None,
            title: None,
        }
    }

    pub fn new_with_title(title: String) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_block(Mode::Normal.block(Some(title.clone())));
        textarea.set_cursor_style(Mode::Normal.cursor_style());
        Self {
            editor: Vim::new(editor::Mode::Normal),
            textarea,
            block: None,
            title: Some(title),
        }
    }
}

impl<'a> Default for State<'a> {
    fn default() -> Self {
        let mut textarea = TextArea::default();
        textarea.set_block(Mode::Normal.block(None));
        textarea.set_cursor_style(Mode::Normal.cursor_style());
        Self {
            editor: Vim::new(editor::Mode::Normal),
            textarea,
            block: None,
            title: None,
        }
    }
}

#[derive(Default)]
pub struct Feature {}

impl tca::Reducer<State<'_>, Action> for Feature {
    fn reduce(state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::Event(event) => match event {
                Event::Paste(paste) => match state.editor.mode {
                    Mode::Insert => {
                        log::debug!("PASTE {}", paste);
                        state.textarea.insert_str(paste);
                        Effect::none()
                    }
                    _ => Effect::none(),
                },
                _ => match state
                    .editor
                    .transition(event.clone().into(), &mut state.textarea)
                {
                    Transition::Mode(mode) if state.editor.mode != mode => {
                        state.textarea.set_block(
                            state
                                .block
                                .clone()
                                .unwrap_or(mode.block(state.title.clone())),
                        );
                        state.textarea.set_cursor_style(mode.cursor_style());
                        state.editor = Vim::new(mode);

                        Effect::none()
                    }
                    Transition::Nop => match event {
                        Event::Key(key) => match key.code {
                            ratatui::crossterm::event::KeyCode::Enter => {
                                Effect::send(Action::Delegated(Delegated::Commit))
                            }
                            _ => Effect::send(Action::Delegated(Delegated::Noop(event))),
                        },
                        _ => Effect::send(Action::Delegated(Delegated::Noop(event))),
                    },
                    Transition::Mode(Mode::Insert) => {
                        Effect::send(Action::Delegated(Delegated::Updated))
                    }
                    Transition::Mode(_) => Effect::none(),
                    Transition::Quit => Effect::send(Action::Delegated(Delegated::Quit)),
                },
            },
            Action::Delegated(_) => Effect::none(),
        }
    }
}
