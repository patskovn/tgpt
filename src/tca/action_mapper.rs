use crate::tca::action_sender::ActionSender;

pub struct ActionMapper<'a, Action, MappedAction, F>
where
    Action: std::marker::Send,
    MappedAction: std::marker::Send,
    F: Fn(Action) -> MappedAction + std::marker::Send,
{
    parent: Box<dyn ActionSender<SendableAction = MappedAction> + 'a>,
    map: F,
    phantom: std::marker::PhantomData<Action>,
}

impl<'a, Action, MappedAction, F> ActionMapper<'a, Action, MappedAction, F>
where
    Action: std::marker::Send,
    MappedAction: std::marker::Send,
    F: Fn(Action) -> MappedAction + std::marker::Send,
{
    pub fn new(parent: Box<dyn ActionSender<SendableAction = MappedAction> + 'a>, map: F) -> Self {
        Self {
            parent,
            map,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'a, Action, MappedAction, F> ActionSender for ActionMapper<'a, Action, MappedAction, F>
where
    Action: std::marker::Send,
    MappedAction: std::marker::Send,
    F: Fn(Action) -> MappedAction + std::marker::Send,
{
    type SendableAction = Action;

    fn send(&self, action: Action) {
        let mapped = (self.map)(action);
        self.parent.send(mapped);
    }
}
