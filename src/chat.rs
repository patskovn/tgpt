use crossterm::event::{self, Event, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    widgets::{block::Title, Paragraph},
    Frame,
};

use crate::{
    navigation,
    tca::{self, Effect},
    textfield,
};

#[derive(Default)]
pub struct State<'a> {
    textarea: textfield::State<'a>,
    text_focused: bool,
}

#[derive(Debug)]
pub enum Action {
    Event(Event),
    TextField(textfield::Action),
    Delegated(DelegatedAction),
}

#[derive(Debug)]
pub enum DelegatedAction {
    Noop(Event),
}

#[derive(Default)]
pub struct ChatReducer {}

impl tca::Reducer<State<'_>, Action> for ChatReducer {
    fn reduce(&self, state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::Delegated(_) => Effect::none(),
            Action::TextField(textfield::Action::Delegated(delegated)) => match delegated {
                textfield::Delegated::Quit => {
                    state.text_focused = false;
                    Effect::none()
                }
                textfield::Delegated::Noop(e) => {
                    Effect::send(Action::Delegated(DelegatedAction::Noop(e)))
                }
                textfield::Delegated::Updated => Effect::none(),
            },
            Action::TextField(action) => textfield::Feature::default()
                .reduce(&mut state.textarea, action)
                .map(Action::TextField),
            Action::Event(e) => {
                if state.text_focused {
                    Effect::send(Action::TextField(textfield::Action::Event(e)))
                } else {
                    match e {
                        Event::Key(key) if key.kind != event::KeyEventKind::Release => {
                            match key.code {
                                event::KeyCode::Char('c')
                                    if key.modifiers == KeyModifiers::NONE =>
                                {
                                    state.text_focused = true;
                                    Effect::none()
                                }
                                _ => Effect::send(Action::Delegated(DelegatedAction::Noop(e))),
                            }
                        }
                        _ => Effect::send(Action::Delegated(DelegatedAction::Noop(e))),
                    }
                }
            }
        }
    }
}

pub fn ui(frame: &mut Frame, area: Rect, state: &State) {
    let mut navigation = navigation::ui(navigation::CurrentScreen::Chat);
    if !state.text_focused {
        navigation = navigation.title(
            Title::from("[c] Show field").position(ratatui::widgets::block::Position::Bottom),
        );
    }
    let body = if state.text_focused {
        let layout = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(vec![Constraint::Fill(1), Constraint::Max(10)])
            .split(area);

        frame.render_widget(state.textarea.widget(), layout[1]);
        layout[0]
    } else {
        area
    };

    frame.render_widget(Paragraph::new("Hello, world!").block(navigation), body);
}
