use crossterm::event::Event;
use log::{debug, warn};
use ratatui::widgets::{Block, Widget};
use tui_textarea::TextArea;

use crate::editor::{self, Mode, Transition, Vim};
use crate::tca;
use crate::tca::Effect;

#[derive(Debug)]
pub enum Action {
    Event(Event),
    Delegated(Delegated),
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
    Updated,
    Quit,
}

#[derive(Debug)]
pub struct State<'a> {
    editor: Vim,
    pub textarea: TextArea<'a>,
    block: Option<Block<'a>>,
}

impl PartialEq for State<'_> {
    fn eq(&self, other: &Self) -> bool {
        let areas_eq = self.textarea.lines() == other.textarea.lines();
        self.editor == other.editor && self.block == other.block && areas_eq
    }
}
impl Eq for State<'_> {}

impl<'a> State<'a> {
    pub fn widget(&'a self) -> impl Widget + 'a {
        self.textarea.widget()
    }

    pub fn new(block: Block<'a>) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_block(block);
        textarea.set_cursor_style(Mode::Normal.cursor_style());
        Self {
            editor: Vim::new(editor::Mode::Normal),
            textarea,
            block: None,
        }
    }
}

impl<'a> Default for State<'a> {
    fn default() -> Self {
        let mut textarea = TextArea::default();
        textarea.set_block(Mode::Normal.block());
        textarea.set_cursor_style(Mode::Normal.cursor_style());
        Self {
            editor: Vim::new(editor::Mode::Normal),
            textarea,
            block: None,
        }
    }
}

#[derive(Default)]
pub struct Feature {}

impl tca::Reducer<State<'_>, Action> for Feature {
    fn reduce(&self, state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::Event(event) => {
                match state
                    .editor
                    .transition(event.clone().into(), &mut state.textarea)
                {
                    Transition::Mode(mode) if state.editor.mode != mode => {
                        state
                            .textarea
                            .set_block(state.block.clone().unwrap_or(mode.block()));
                        state.textarea.set_cursor_style(mode.cursor_style());
                        state.editor = Vim::new(mode);

                        Effect::none()
                    }
                    Transition::Nop => Effect::send(Action::Delegated(Delegated::Noop(event))),
                    Transition::Mode(Mode::Insert) => {
                        Effect::send(Action::Delegated(Delegated::Updated))
                    }
                    Transition::Mode(_) => Effect::none(),
                    Transition::Quit => Effect::send(Action::Delegated(Delegated::Quit)),
                }
            }
            Action::Delegated(_) => Effect::none(),
        }
    }
}
