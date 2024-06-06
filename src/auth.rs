use ratatui::{
    layout::{Constraint, Layout},
    widgets::{Block, Paragraph},
    Frame,
};

#[derive(Debug)]
pub struct State {}

#[derive(Default)]
pub struct AuthReducer {}

pub fn ui(frame: &mut Frame, state: &State) {
    let layout = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(vec![Constraint::Fill(1), Constraint::Max(10)])
        .split(frame.size());

    frame.render_widget(
        Paragraph::new("Hello, world!").block(Block::bordered()),
        layout[0],
    );
}

#[derive(Debug)]
struct ChatGPTSelectionState {}
