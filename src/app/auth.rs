use ratatui::crossterm::event::Event;
use ratatui::{layout::Rect, Frame};
use tca::Effect;
use tca::Reducer;

use crate::{app::chat_gpt_configuration, gpt, list};

#[derive(Debug, PartialEq, Clone)]
pub struct State<'a> {
    providers: list::State<gpt::types::Provider>,

    configuration: Option<Configuration<'a>>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
enum Configuration<'a> {
    ChatGPT(chat_gpt_configuration::State<'a>),
}

impl Default for State<'_> {
    fn default() -> Self {
        Self {
            providers: list::State::new(vec![gpt::types::Provider::OpenAI]),
            configuration: None,
        }
    }
}

impl State<'_> {
    pub fn new() -> Self {
        let mut val = Self::default();
        val.update_config();
        val
    }

    pub fn update_config(&mut self) {}
}

#[derive(Debug)]
pub enum Action {
    Event(Event),
    List(list::Action),
    ChatGPTConfig(chat_gpt_configuration::Action),
    Delegated(Delegated),
}

#[derive(Debug)]
pub enum Delegated {
    Noop(Event),
}

#[derive(Default)]
pub struct AuthReducer {}
impl Reducer<State<'_>, Action> for AuthReducer {
    fn reduce(state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::ChatGPTConfig(chat_gpt_configuration::Action::Delegated(delegated)) => {
                match delegated {
                    chat_gpt_configuration::Delegated::Exit => {
                        state.configuration = None;
                        Effect::none()
                    }
                    chat_gpt_configuration::Delegated::Noop(e) => {
                        Effect::send(Action::Delegated(Delegated::Noop(e)))
                    }
                    chat_gpt_configuration::Delegated::Finished(config) => {
                        state.configuration = None;
                        config.save().unwrap();

                        Effect::none()
                    }
                }
            }
            Action::ChatGPTConfig(action) => match &mut state.configuration {
                Some(Configuration::ChatGPT(config_state)) => {
                    chat_gpt_configuration::Feature::reduce(config_state, action)
                        .map(Action::ChatGPTConfig)
                }
                _ => panic!(
                    "Attempted to send {:#?} for {:#?} state",
                    action, state.configuration
                ),
            },
            Action::List(list::Action::Delegated(delegated)) => match delegated {
                list::Delegated::Noop(e) => Effect::send(Action::Delegated(Delegated::Noop(e))),
                list::Delegated::Enter(idx) => match state.providers.items[idx] {
                    gpt::types::Provider::OpenAI => {
                        state.configuration =
                            Some(Configuration::ChatGPT(chat_gpt_configuration::State::new()));

                        Effect::none()
                    }
                },
                list::Delegated::Toogle => Effect::none(),
            },
            Action::List(action) => {
                list::ListFeature::reduce(&mut state.providers, action).map(Action::List)
            }
            Action::Delegated(_) => Effect::none(),
            Action::Event(e) => match state.configuration {
                Some(Configuration::ChatGPT(_)) => Effect::send(Action::ChatGPTConfig(
                    chat_gpt_configuration::Action::Event(e),
                )),
                None => Effect::send(Action::List(list::Action::Event(e))),
            },
        }
    }
}

pub fn ui(frame: &mut Frame, area: Rect, store: tca::Store<State, Action>) {
    let state = store.state();
    list::ui(frame, area, &state.providers);

    match &state.configuration {
        Some(Configuration::ChatGPT(state)) => chat_gpt_configuration::ui(frame, area, state),
        None => {}
    };
}
