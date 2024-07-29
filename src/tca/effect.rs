use crate::tca::action_mapper::ActionMapper;
use crate::tca::action_sender::ActionSender;
use futures::future::BoxFuture;
use futures::FutureExt;
use std::fmt::Debug;

pub struct Effect<'effect, Action: std::marker::Send> {
    pub value: EffectValue<'effect, Action>,
}

pub type BoxActionSender<'effect, Action> =
    Box<dyn ActionSender<SendableAction = Action> + 'effect>;
pub type AsyncActionJob<'effect, Action> = Box<
    dyn FnOnce(BoxActionSender<'effect, Action>) -> BoxFuture<'effect, ()>
        + 'effect
        + std::marker::Send,
>;

pub enum EffectValue<'effect, Action> {
    None,
    Send(Action),
    Async(AsyncActionJob<'effect, Action>),
    Quit,
}

impl<Action> Debug for EffectValue<'_, Action>
where
    Action: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::Send(action) => f.write_str(&format!("Send {:#?}", action)),
            Self::Async(_) => f.write_str("Async"),
            Self::Quit => f.write_str("Quit"),
        }
    }
}

impl<'effect, Action: std::marker::Send + 'effect> Effect<'effect, Action> {
    pub fn map<F, MappedAction: std::marker::Send + std::marker::Sync + 'effect>(
        self,
        map: F,
    ) -> Effect<'effect, MappedAction>
    where
        F: Fn(Action) -> MappedAction + std::marker::Send + std::marker::Sync + 'effect,
    {
        match self.value {
            EffectValue::None => Effect::none(),
            EffectValue::Quit => Effect::quit(),
            EffectValue::Send(a) => Effect::send(map(a)),
            EffectValue::Async(a) => Effect::run(|sender| {
                let mapper = ActionMapper::new(sender, map);
                async move { a(Box::new(mapper)).await }.boxed()
            }),
        }
    }

    pub fn run<T>(job: T) -> Self
    where
        T: FnOnce(
                Box<dyn ActionSender<SendableAction = Action> + 'effect>,
            ) -> BoxFuture<'effect, ()>
            + 'effect
            + std::marker::Send,
    {
        Self {
            value: EffectValue::Async(Box::new(job)),
        }
    }

    pub fn none() -> Self {
        Self {
            value: EffectValue::None,
        }
    }

    pub fn quit() -> Self {
        Self {
            value: EffectValue::Quit,
        }
    }

    pub fn send(action: Action) -> Self {
        Self {
            value: EffectValue::Send(action),
        }
    }
}
