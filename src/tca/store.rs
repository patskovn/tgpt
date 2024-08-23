use crate::tca::action_sender::ActionSender;
use crate::tca::reducer::Reducer;
use crossterm::event::{Event, EventStream};

use super::{action_mapper::ActionMapper, engine::StoreEngine, event_sender_holder::EventSender};

pub struct Store<'a, State, Action>
where
    Action: std::marker::Send,
    State: PartialEq + Clone + std::marker::Send,
{
    engine: EngineHolder<'a, State, Action>,
}

enum EngineHolder<'a, State, Action>
where
    Action: std::marker::Send,
    State: PartialEq + Clone + std::marker::Send,
{
    Engine(StoreEngine<'a, State, Action>),
    Parent(Box<dyn ActionSender<SendableAction = Action>>),
}

impl<'a, State, Action> Store<'a, State, Action>
where
    Action: std::fmt::Debug + std::marker::Send,
    State: PartialEq + Clone + std::marker::Send,
{
    pub fn new<R: Reducer<State, Action> + std::marker::Sync + std::marker::Send + 'a>(
        state: State,
        reducer: R,
    ) -> Self {
        Self {
            engine: EngineHolder::Engine(StoreEngine::new(state, reducer)),
        }
    }

    fn new_from_parent(parent: Box<dyn ActionSender<SendableAction = Action>>) -> Self {
        Self {
            engine: EngineHolder::Parent(parent),
        }
    }

    pub async fn run(
        &self,
        redraw: impl FnMut(&State),
        event_mapper: impl Fn(Event) -> Action,
        terminal_events: &mut EventStream,
    ) {
        match &self.engine {
            EngineHolder::Engine(engine) => engine.run(redraw, event_mapper, terminal_events).await,
            EngineHolder::Parent(_) => panic!("Can't run non parent store"),
        }
    }

    pub fn scope<ChildAction>(
        &self,
        action: impl Fn(ChildAction) -> Action + std::marker::Send,
    ) -> Store<'a, State, ChildAction>
    where
        ChildAction: std::fmt::Debug + std::marker::Send,
    {
        let parent = Box::new(self);
        let action_mapper = ActionMapper::new(parent, action);
        Store::new_from_parent(Box::new(action_mapper))
    }
}

impl<'a, State, Action> ActionSender for Store<'a, State, Action>
where
    Action: std::marker::Send,
    State: PartialEq + Clone + std::marker::Send,
{
    type SendableAction = Action;

    fn send(&self, action: Action) {
        match &self.engine {
            EngineHolder::Engine(engine) => engine.send(action),
            EngineHolder::Parent(sender) => sender.send(action),
        }
    }
}

impl<'a, State, Action> ActionSender for &Store<'a, State, Action>
where
    Action: std::marker::Send,
    State: PartialEq + Clone + std::marker::Send,
{
    type SendableAction = Action;

    fn send(&self, action: Action) {
        match &self.engine {
            EngineHolder::Engine(engine) => engine.send(action),
            EngineHolder::Parent(sender) => sender.send(action),
        }
    }
}
