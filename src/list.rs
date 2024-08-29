use std::cmp::{max, min};
use tca::Effect;

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{List, ListItem, ListState, StatefulWidget},
    Frame,
};

use crate::gpt;

#[derive(Debug, PartialEq, Clone)]
pub struct State<T>
where
    T: for<'a> Into<ListItem<'a>>,
    T: Clone,
{
    list_state: ListState,
    pub items: Vec<T>,
}

impl<T> State<T>
where
    T: for<'a> Into<ListItem<'a>>,
    T: Clone,
{
    pub fn new(items: Vec<T>) -> Self {
        State {
            list_state: ListState::default(),
            items,
        }
    }
}

pub fn ui<T>(frame: &mut Frame, area: Rect, state: &State<T>)
where
    T: for<'a> Into<ListItem<'a>>,
    T: Clone,
{
    let items: Vec<ListItem> = state.items.iter().map(|i| i.clone().into()).collect();
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::REVERSED)
                .fg(ratatui::style::Color::Blue),
        )
        .highlight_symbol(">")
        .highlight_spacing(ratatui::widgets::HighlightSpacing::Always);
    let mut list_state = state.list_state.clone();
    StatefulWidget::render(list, area, frame.buffer_mut(), &mut list_state);
}

#[derive(Debug)]
pub enum Action {
    Event(Event),
    Delegated(Delegated),
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
    Toogle(usize),
    Enter(usize),
}

#[derive(Default)]
pub struct ListFeature {}

impl<'a> From<gpt::types::Provider> for ListItem<'a> {
    fn from(value: gpt::types::Provider) -> Self {
        Self::from(format!("{}", value))
    }
}

impl<T> tca::Reducer<State<T>, Action> for ListFeature
where
    T: for<'a> Into<ListItem<'a>>,
    T: Clone,
    T: Eq,
{
    fn reduce(state: &mut State<T>, action: Action) -> Effect<Action> {
        match action {
            Action::Event(e) => match e {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('j') => {
                        state.list_state.select(
                            state
                                .list_state
                                .selected()
                                .map(|selected| min(selected + 1, state.items.len() - 1))
                                .or(Some(0)),
                        );

                        Effect::none()
                    }
                    KeyCode::Char('k') => {
                        state.list_state.select(
                            state
                                .list_state
                                .selected()
                                .map(|selected| max(selected, 1) - 1)
                                .or(Some(0)),
                        );
                        Effect::none()
                    }
                    KeyCode::Char(' ') => state.list_state.selected().map_or(Effect::none(), |s| {
                        Effect::send(Action::Delegated(Delegated::Toogle(s)))
                    }),
                    KeyCode::Enter => state.list_state.selected().map_or(Effect::none(), |s| {
                        Effect::send(Action::Delegated(Delegated::Enter(s)))
                    }),
                    _ => Effect::send(Action::Delegated(Delegated::Noop(e))),
                },
                _ => Effect::send(Action::Delegated(Delegated::Noop(e))),
            },
            Action::Delegated(_) => Effect::none(),
        }
    }
}
