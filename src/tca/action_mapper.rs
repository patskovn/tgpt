use std::sync::Arc;

use crate::tca::action_sender::ActionSender;

pub struct ActionMapper<Action, MappedAction, F>
where
    Action: std::marker::Send,
    MappedAction: std::marker::Send,
    F: Fn(Action) -> MappedAction + std::marker::Send,
{
    parent: Arc<dyn ActionSender<SendableAction = MappedAction>>,
    map: F,
    phantom: std::marker::PhantomData<Action>,
}

impl<Action, MappedAction, F> ActionMapper<Action, MappedAction, F>
where
    Action: std::marker::Send,
    MappedAction: std::marker::Send,
    F: Fn(Action) -> MappedAction + std::marker::Send,
{
    pub fn new(parent: Arc<dyn ActionSender<SendableAction = MappedAction>>, map: F) -> Self {
        Self {
            parent,
            map,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<Action, MappedAction, F> ActionSender for ActionMapper<Action, MappedAction, F>
where
    Action: std::marker::Send + std::marker::Sync,
    MappedAction: std::marker::Send,
    F: Fn(Action) -> MappedAction + std::marker::Send + std::marker::Sync,
{
    type SendableAction = Action;

    fn send(&self, action: Action) {
        let mapped = (self.map)(action);
        self.parent.send(mapped);
    }
}
