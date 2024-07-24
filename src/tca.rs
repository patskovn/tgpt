use async_trait::async_trait;
use futures::future::BoxFuture;
use std::fmt::Debug;
use tokio::runtime::Runtime;

pub struct Store<'store, R, State, Action>
where
    R: Reducer<State, Action> + std::marker::Sync,
    Action: std::fmt::Debug + std::marker::Sync + std::marker::Send,
    State: Eq + Clone + std::marker::Sync,
{
    state: State,
    reducer: R,
    redraw: tokio::sync::mpsc::Sender<()>,
    phantom: std::marker::PhantomData<&'store Action>,
    pub quit: bool,
}

#[async_trait]
pub trait AsyncActionSender<Action>: std::marker::Sync + std::marker::Send {
    async fn async_send(&self, action: Action);
}

impl<'store, R, State, Action> Store<'store, R, State, Action>
where
    R: Reducer<State, Action> + std::marker::Sync,
    Action: std::fmt::Debug + std::marker::Sync + std::marker::Send,
    State: Eq + Clone + std::marker::Sync,
{
    pub fn new(state: State, redraw: tokio::sync::mpsc::Sender<()>, reducer: R) -> Self {
        Self {
            state,
            reducer,
            redraw,
            phantom: std::marker::PhantomData,
            quit: false,
        }
    }

    pub fn send(&mut self, action: Action) {
        let state_before = self.state.clone();
        self.handle(Effect::send(action));
        if state_before != self.state {
            _ = self.redraw.try_send(());
        }
    }

    fn handle<'a>(&'a mut self, effect: Effect<'a, Action>) {
        log::debug!("Handling {:#?}", effect.value);
        match effect.value {
            EffectValue::None => {}
            EffectValue::Send(action) => {
                let effect = self.reducer.reduce(&mut self.state, action);
                self.handle(effect)
            }
            EffectValue::Quit => {
                self.quit = true;
            }
            EffectValue::Async(job) => {
                let self_ref: &Self = self;
                let fut = job(Box::new(self_ref));
                let rt = Runtime::new().unwrap();
                rt.block_on(fut);

                // let s: &Self = self;
                // job(Box::new(s))
            }
        }
    }

    pub fn with_state<F>(&self, f: F)
    where
        F: FnOnce(&State),
    {
        f(&self.state);
    }
}

#[async_trait]
impl<'store, R, State, Action> AsyncActionSender<Action> for &Store<'store, R, State, Action>
where
    R: Reducer<State, Action> + std::marker::Sync,
    Action: std::fmt::Debug + std::marker::Sync + std::marker::Send,
    State: Eq + Clone + std::marker::Sync,
{
    async fn async_send(&self, _action: Action) {
        println!("Hello");
    }
}

pub trait Reducer<State, Action: std::fmt::Debug + std::marker::Sync + std::marker::Send> {
    fn reduce<'effect>(&self, state: &mut State, action: Action) -> Effect<'effect, Action>;
}

pub struct EmptyReducer {}
impl<State, Action: std::fmt::Debug + std::marker::Sync + std::marker::Send> Reducer<State, Action>
    for EmptyReducer
{
    fn reduce<'effect>(&self, _state: &mut State, _action: Action) -> Effect<'effect, Action> {
        Effect::none()
    }
}

#[derive(Debug)]
struct ReducerConfiguration<R, State, Action, ParentR, ParentState, ParentAction>
where
    R: Reducer<State, Action>,
    ParentR: Reducer<ParentState, ParentAction>,
    Action: std::fmt::Debug + std::marker::Sync + std::marker::Send,
    ParentAction: std::fmt::Debug + std::marker::Sync + std::marker::Send,
    State: Eq,
    State: Clone,
    ParentState: Eq,
    ParentState: Clone,
{
    _base: R,
    _parent: Option<ParentR>,

    phantom_state: std::marker::PhantomData<State>,
    phantom_action: std::marker::PhantomData<Action>,
    phantom_parent_state: std::marker::PhantomData<ParentState>,
    phantom_parent_action: std::marker::PhantomData<ParentAction>,
}

impl<R, State, Action, ParentR, ParentState, ParentAction>
    ReducerConfiguration<R, State, Action, ParentR, ParentState, ParentAction>
where
    R: Reducer<State, Action>,
    ParentR: Reducer<ParentState, ParentAction>,
    Action: std::fmt::Debug + std::marker::Sync + std::marker::Send,
    ParentAction: std::fmt::Debug + std::marker::Sync + std::marker::Send,
    State: Eq,
    State: Clone,
    ParentState: Eq,
    ParentState: Clone,
{
    pub fn scope<ChildReducer, ChildState, ChildAction, FState, FAction>(
        self,
        reducer: ChildReducer,
        _state: FState,
        _action: FAction,
    ) -> ReducerConfiguration<ChildReducer, ChildState, ChildAction, R, State, Action>
    where
        FState: Fn(&State) -> &ChildState,
        FAction: Fn(&Action) -> ChildAction,
        ChildReducer: Reducer<ChildState, ChildAction>,
        ChildAction: std::fmt::Debug + std::marker::Sync + std::marker::Send,
        ChildState: Eq,
        ChildState: Clone,
    {
        ReducerConfiguration {
            _base: reducer,
            _parent: Some(self._base),
            phantom_state: std::marker::PhantomData,
            phantom_action: std::marker::PhantomData,
            phantom_parent_state: std::marker::PhantomData,
            phantom_parent_action: std::marker::PhantomData,
        }
    }
}

pub struct Effect<
    'effect,
    Action: std::fmt::Debug + std::marker::Send + std::marker::Sync + 'effect,
> {
    value: EffectValue<'effect, Action>,
}

enum EffectValue<'effect, Action>
where
    Action: std::fmt::Debug,
{
    None,
    Send(Action),
    Async(
        Box<
            dyn FnOnce(Box<dyn AsyncActionSender<Action> + 'effect>) -> BoxFuture<'effect, ()>
                + 'effect
                + std::marker::Send,
        >,
    ),
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

struct ActionMapper<'a, Action, MappedAction, F>
where
    Action: std::fmt::Debug + std::marker::Send + std::marker::Sync,
    MappedAction: std::fmt::Debug + std::marker::Send,
    F: Fn(Action) -> MappedAction + std::marker::Sync + std::marker::Send,
{
    parent: Box<dyn AsyncActionSender<MappedAction> + 'a>,
    map: F,
    phantom: std::marker::PhantomData<Action>,
}

impl<'a, Action, MappedAction, F> ActionMapper<'a, Action, MappedAction, F>
where
    Action: std::fmt::Debug + std::marker::Send + std::marker::Sync,
    MappedAction: std::fmt::Debug + std::marker::Send,
    F: Fn(Action) -> MappedAction + std::marker::Sync + std::marker::Send,
{
    fn new(parent: Box<dyn AsyncActionSender<MappedAction> + 'a>, map: F) -> Self {
        Self {
            parent,
            map,
            phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<'a, Action, MappedAction, F> AsyncActionSender<Action>
    for ActionMapper<'a, Action, MappedAction, F>
where
    Action: std::fmt::Debug + std::marker::Send + std::marker::Sync,
    MappedAction: std::fmt::Debug + std::marker::Send,
    F: Fn(Action) -> MappedAction + std::marker::Sync + std::marker::Send,
{
    async fn async_send(&self, action: Action) {
        let mapped = (self.map)(action);
        self.parent.async_send(mapped).await;
    }
}

impl<'effect, Action: std::fmt::Debug + std::marker::Send + std::marker::Sync + 'effect>
    Effect<'effect, Action>
{
    pub fn map<F, MappedAction: std::fmt::Debug + std::marker::Send + std::marker::Sync + 'effect>(
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
                Box::pin(async move { a(Box::new(mapper)).await })
            }),
        }
    }

    pub fn run<T>(job: T) -> Self
    where
        T: FnOnce(Box<dyn AsyncActionSender<Action> + 'effect>) -> BoxFuture<'effect, ()>
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
