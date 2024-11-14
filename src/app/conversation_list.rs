use anyhow::Context;
use atomic_write_file::AtomicWriteFile;
use core::fmt;
use serde::Deserialize;
use std::{collections::HashSet, io::Write, path::PathBuf};

use chatgpt::types::ChatMessage;
use crossterm::event::Event;
use derive_new::new;
use ratatui::{
    layout::Rect,
    style::{Style, Stylize},
    widgets::ListItem,
    Frame,
};
use serde::Serialize;
use tca::{ActionSender, Effect, Reducer};
use uuid::Uuid;

use crate::list;

use super::{
    chat::{CurrentFocus, SharedFocus},
    navigation,
};

#[derive(Serialize, Deserialize, Debug, new)]
pub struct ChatHistory {
    pub history: Vec<ChatMessage>,
}

#[derive(Default, Serialize, Deserialize, Debug, new)]
pub struct ConversationMetadata {
    pub list: Vec<ConversationItem>,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub enum ConversationListEntry {
    #[default]
    NewMessage,
    Item(ConversationItem),
}

impl fmt::Display for ConversationListEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NewMessage => f.write_str("* New conversation"),
            Self::Item(item) => std::fmt::Display::fmt(item, f),
        }
    }
}

#[derive(Debug, PartialEq, Default, Clone, Serialize, Deserialize, new)]
pub struct ConversationItem {
    pub id: Uuid,
    pub title: String,
    pub titlte_updated_at: usize,
}

impl fmt::Display for ConversationItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.title)
    }
}

impl<'a> From<ConversationListEntry> for ListItem<'a> {
    fn from(value: ConversationListEntry) -> Self {
        match value {
            ConversationListEntry::Item(item) => Self::from(item.title),
            ConversationListEntry::NewMessage => Self::from("* New conversation"),
        }
    }
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct State {
    pub current_focus: SharedFocus,
    pub conversations: list::State<ConversationListEntry>,
    pub _something: bool,
}

impl State {
    pub fn new(current_focus: SharedFocus) -> Self {
        Self {
            current_focus,
            ..Default::default()
        }
    }
}

#[derive(Debug)]
pub enum Action {
    Reload,
    UpdateList(ConversationMetadata),
    Event(Event),
    Delegated(Delegated),
    List(list::Action),
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
    Select((ConversationItem, ChatHistory)),
    NewConversation,
}

pub struct Feature {}

fn history_dir() -> anyhow::Result<PathBuf> {
    let home_dir = dirs::home_dir().with_context(|| "Failed to get home directory")?;
    Ok(home_dir.join(".tgpt").join("history"))
}

fn history_medata_path() -> anyhow::Result<PathBuf> {
    history_dir().map(|d| d.join("metadata.json"))
}

pub fn load_metadata() -> anyhow::Result<ConversationMetadata> {
    let history_metadata_path = history_medata_path()?;

    std::fs::read(history_metadata_path)
        .with_context(|| "Failed to open history metadata path")
        .and_then(|slice| {
            serde_json::from_slice::<ConversationMetadata>(&slice)
                .with_context(|| "Failed to parse history metadata file")
        })
}

pub fn save_metadata(metadata: ConversationMetadata) -> anyhow::Result<()> {
    let mut file = AtomicWriteFile::options().open(history_medata_path()?)?;
    file.write_all(&serde_json::to_vec(&metadata)?)?;
    file.commit()?;
    Ok(())
}

impl Reducer<State, Action> for Feature {
    fn reduce(state: &mut State, action: Action) -> tca::Effect<Action> {
        match action {
            Action::Reload => Effect::run(|sender| async move {
                let home_dir = dirs::home_dir().expect("Failed to get home directory");
                let history_dir = home_dir.join(".tgpt").join("history");

                let mut metadata = load_metadata().unwrap_or_default();

                let all_history_files = std::fs::read_dir(history_dir.clone())
                    .map(|entries| {
                        entries
                            .flatten()
                            .filter_map(|entry| {
                                entry
                                    .path()
                                    .file_name()
                                    .and_then(|s| s.to_str().map(String::from))
                            })
                            .collect::<HashSet<_>>()
                    })
                    .unwrap_or_default();
                metadata
                    .list
                    .retain(|entry| all_history_files.contains(&*entry.id.to_string()));

                sender.send(Action::UpdateList(metadata));
            }),
            Action::List(list::Action::Delegated(delegated)) => match delegated {
                list::Delegated::Noop(e) => Effect::send(Action::Delegated(Delegated::Noop(e))),
                list::Delegated::Enter(idx) => {
                    if idx == 0 {
                        return Effect::send(Action::Delegated(Delegated::NewConversation));
                    }
                    let item = match &state.conversations.items[idx] {
                        ConversationListEntry::Item(item) => item,
                        ConversationListEntry::NewMessage => {
                            panic!("Should be filetered out by zero index")
                        }
                    };
                    let home_dir = dirs::home_dir().expect("Failed to get home directory");
                    let file_path = home_dir
                        .join(".tgpt")
                        .join("history")
                        .join(item.id.to_string());
                    if let Ok(content) = std::fs::read(file_path) {
                        if let Ok(history) = serde_json::from_slice::<ChatHistory>(&content) {
                            return Effect::send(Action::Delegated(Delegated::Select((
                                item.clone(),
                                history,
                            ))));
                        }
                    }
                    Effect::none()
                }
                list::Delegated::Toogle => Effect::none(),
            },
            Action::List(action) => {
                list::ListFeature::reduce(&mut state.conversations, action).map(Action::List)
            }
            Action::UpdateList(metadata) => {
                let mut all_items: Vec<ConversationListEntry> =
                    vec![ConversationListEntry::NewMessage];
                all_items.extend(
                    metadata
                        .list
                        .into_iter()
                        .map(ConversationListEntry::Item)
                        .collect::<Vec<_>>(),
                );
                state.conversations = list::State::new(all_items);
                Effect::none()
            }
            Action::Event(e) => Effect::send(Action::List(list::Action::Event(e))),
            Action::Delegated(_) => Effect::none(),
        }
    }
}

pub fn ui(frame: &mut Frame, area: Rect, store: tca::Store<State, Action>) {
    let navigation =
        navigation::ui_with_title(navigation::CurrentScreen::Chat, Some("[1]".to_string()));
    let state = store.state();
    let navigation_style = if state.current_focus.value() == CurrentFocus::ConversationList {
        Style::new().green()
    } else {
        Style::default()
    };
    list::ui(frame, navigation.inner(area), &state.conversations);
    frame.render_widget(navigation.border_style(navigation_style), area);
}
