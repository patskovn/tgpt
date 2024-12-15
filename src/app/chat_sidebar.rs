use crossterm::event::{self, Event, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Style, Stylize},
    Frame,
};
use tca::{Effect, Reducer};

use super::{
    auth,
    chat::{CurrentFocus, SharedFocus},
    conversation_list::{self, ChatHistory, ConversationItem},
    navigation,
};

#[derive(Debug, PartialEq, Clone)]
pub struct State<'a> {
    pub current_focus: SharedFocus,
    pub conversation_list: conversation_list::State,
    pub auth: auth::State<'a>,
    pub focused_tab: FocusedTab,
}

impl State<'_> {
    pub fn new(current_focus: SharedFocus) -> Self {
        Self {
            current_focus,
            conversation_list: Default::default(),
            auth: Default::default(),
            focused_tab: FocusedTab::ConversationList,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum FocusedTab {
    ConversationList,
    Auth,
}

#[derive(Debug)]
pub enum Action {
    Event(Event),
    ConversationList(conversation_list::Action),
    Auth(auth::Action),
    Delegated(Delegated),
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
    NewConversation,
    Select((ConversationItem, ChatHistory)),
}

pub struct Feature {}

fn try_toggle_focus(state: &mut State, event: Event) -> tca::Effect<Action> {
    match event {
        Event::Key(KeyEvent {
            code: event::KeyCode::Char('1'),
            ..
        }) => {
            match state.focused_tab {
                FocusedTab::Auth => state.focused_tab = FocusedTab::ConversationList,
                FocusedTab::ConversationList => state.focused_tab = FocusedTab::Auth,
            }
            Effect::none()
        }
        _ => Effect::send(Action::Delegated(Delegated::Noop(event))),
    }
}

impl Reducer<State<'_>, Action> for Feature {
    fn reduce(state: &mut State<'_>, action: Action) -> tca::Effect<Action> {
        match action {
            Action::Event(e) => match state.focused_tab {
                FocusedTab::Auth => Effect::send(Action::Auth(auth::Action::Event(e))),
                FocusedTab::ConversationList => Effect::send(Action::ConversationList(
                    conversation_list::Action::Event(e),
                )),
            },
            Action::Auth(auth::Action::Delegated(delegated)) => match delegated {
                auth::Delegated::Noop(e) => try_toggle_focus(state, e),
            },
            Action::Auth(action) => {
                auth::AuthReducer::reduce(&mut state.auth, action).map(Action::Auth)
            }
            Action::ConversationList(conversation_list::Action::Delegated(delegated)) => {
                match delegated {
                    conversation_list::Delegated::Noop(e) => try_toggle_focus(state, e),
                    conversation_list::Delegated::NewConversation => {
                        Effect::send(Action::Delegated(Delegated::NewConversation))
                    }
                    conversation_list::Delegated::Select(i) => {
                        Effect::send(Action::Delegated(Delegated::Select(i)))
                    }
                }
            }
            Action::ConversationList(action) => {
                conversation_list::Feature::reduce(&mut state.conversation_list, action)
                    .map(Action::ConversationList)
            }
            Action::Delegated(_) => Effect::none(),
        }
    }
}

pub fn ui(frame: &mut Frame, area: Rect, store: tca::Store<State, Action>) {
    let state = store.state();
    let highlighted_navigation = match state.focused_tab {
        FocusedTab::Auth => navigation::CurrentScreen::Config,
        FocusedTab::ConversationList => navigation::CurrentScreen::Chat,
    };
    let navigation = navigation::ui_with_title(highlighted_navigation, Some("[1]".to_string()));
    let state = store.state();
    let navigation_style = if state.current_focus.value() == CurrentFocus::Sidebar {
        Style::new().green()
    } else {
        Style::default()
    };
    match state.focused_tab {
        FocusedTab::ConversationList => conversation_list::ui(
            frame,
            navigation.inner(area),
            store.scope(|s| &s.conversation_list, Action::ConversationList),
        ),
        FocusedTab::Auth => auth::ui(
            frame,
            navigation.inner(area),
            store.scope(|s| &s.auth, Action::Auth),
        ),
    }
    frame.render_widget(navigation.border_style(navigation_style), area);
}
