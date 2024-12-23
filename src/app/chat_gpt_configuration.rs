use ratatui::crossterm::event::Event;
use ratatui::{
    layout::{Constraint, Rect},
    widgets::{Block, Borders},
    Frame,
};
use tca::Effect;

use crate::{
    gpt::openai::ChatGPTConfiguration,
    single_line_input,
    uiutils::layout::{centered_constraint, centered_pct},
};

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct State<'a> {
    api_key: single_line_input::State<'a>,
}

impl State<'_> {
    pub fn new() -> Self {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title("Enter OpenAI API Key")
            .title_bottom("[q] Hide field");

        Self {
            api_key: single_line_input::State::new(block),
        }
    }
}

#[derive(Debug)]
pub enum Action {
    Event(Event),
    Input(single_line_input::Action),
    Delegated(Delegated),
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
    Finished(ChatGPTConfiguration),
    Exit,
}

#[derive(Default)]
pub struct Feature {}

impl tca::Reducer<State<'_>, Action> for Feature {
    fn reduce(state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::Delegated(_) => Effect::none(),
            Action::Event(e) => Effect::send(Action::Input(single_line_input::Action::Event(e))),
            Action::Input(single_line_input::Action::Delegated(delegated)) => match delegated {
                single_line_input::Delegated::Exit => {
                    Effect::send(Action::Delegated(Delegated::Exit))
                }
                single_line_input::Delegated::Noop(e) => {
                    Effect::send(Action::Delegated(Delegated::Noop(e)))
                }
                single_line_input::Delegated::Enter => {
                    let api_key = state
                        .api_key
                        .textarea
                        .textarea
                        .lines()
                        .first()
                        .cloned()
                        .unwrap_or_default();
                    let config = ChatGPTConfiguration::new(api_key);

                    Effect::send(Action::Delegated(Delegated::Finished(config)))
                }
            },
            Action::Input(action) => {
                single_line_input::Feature::reduce(&mut state.api_key, action).map(Action::Input)
            }
        }
    }
}

pub fn ui(frame: &mut Frame, area: Rect, state: &State) {
    let modal_x = centered_constraint(
        area,
        Constraint::Length(3),
        ratatui::layout::Direction::Vertical,
    );
    let modal = centered_pct(modal_x, ratatui::layout::Direction::Horizontal, 80);
    single_line_input::ui(frame, modal, &state.api_key);
}
