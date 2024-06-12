use crossterm::event::Event;
use ratatui::widgets::Widget;
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
    Quit,
}

pub struct State<'a> {
    editor: Vim,
    textarea: TextArea<'a>,
}

impl<'a> State<'a> {
    pub fn widget(&'a self) -> impl Widget + 'a {
        self.textarea.widget()
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
        }
    }
}

#[derive(Default)]
pub struct TextFieldReducer {}

impl tca::Reducer<State<'_>, Action> for TextFieldReducer {
    fn reduce(&self, state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::Event(event) => {
                match state
                    .editor
                    .transition(event.clone().into(), &mut state.textarea)
                {
                    Transition::Mode(mode) if state.editor.mode != mode => {
                        state.textarea.set_block(mode.block());
                        state.textarea.set_cursor_style(mode.cursor_style());
                        state.editor = Vim::new(mode);

                        Effect::none()
                    }
                    Transition::Nop => Effect::send(Action::Delegated(Delegated::Noop(event))),
                    Transition::Mode(_) => Effect::none(),
                    Transition::Pending(input) => {
                        state.editor = state.editor.clone().with_pending(input);
                        Effect::none()
                    }
                    Transition::Quit => Effect::send(Action::Delegated(Delegated::Quit)),
                }
            }
            Action::Delegated(_) => Effect::none(),
        }
    }
}
