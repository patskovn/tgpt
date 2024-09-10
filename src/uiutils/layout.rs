use ratatui::layout::{Constraint, Layout, Rect};

pub fn centered_pct(r: Rect, direction: ratatui::layout::Direction, percent: u16) -> Rect {
    Layout::default()
        .direction(direction)
        .constraints([
            Constraint::Percentage((100 - percent) / 2),
            Constraint::Percentage(percent),
            Constraint::Percentage((100 - percent) / 2),
        ])
        .split(r)[1]
}

pub fn centered_constraint(
    r: Rect,
    constraint: Constraint,
    direction: ratatui::layout::Direction,
) -> Rect {
    Layout::default()
        .direction(direction)
        .constraints([Constraint::Fill(1), constraint, Constraint::Fill(1)])
        .split(r)[1]
}
