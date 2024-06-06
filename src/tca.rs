pub struct Store<R, State, Action>
where
    R: Reducer<State, Action>,
    Action: std::fmt::Debug,
{
    state: State,
    reducer: R,
    phantom: std::marker::PhantomData<Action>,
    pub quit: bool,
}

macro_rules! extract {
    ($e:expr, $p:path) => {
        match $e {
            $p(value) => Some(value),
            _ => None,
        }
    };
}

impl<R, State, Action> Store<R, State, Action>
where
    R: Reducer<State, Action>,
    Action: std::fmt::Debug,
{
    pub fn new(state: State, reducer: R) -> Self {
        Self {
            state,
            reducer,
            phantom: std::marker::PhantomData,
            quit: false,
        }
    }

    pub fn send(&mut self, action: Action) {
        self.handle(Effect::send(action));
    }

    fn handle(&mut self, effect: Effect<Action>) {
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
        }
    }

    pub fn with_state<F>(&self, f: F)
    where
        F: FnOnce(&State),
    {
        f(&self.state);
    }
}

pub trait Reducer<State, Action: std::fmt::Debug> {
    fn reduce(&self, state: &mut State, action: Action) -> Effect<Action>;
}

pub struct EmptyReducer {}
impl<State, Action: std::fmt::Debug> Reducer<State, Action> for EmptyReducer {
    fn reduce(&self, _state: &mut State, _action: Action) -> Effect<Action> {
        Effect::none()
    }
}

#[derive(Debug)]
struct ReducerConfiguration<R, State, Action, ParentR, ParentState, ParentAction>
where
    R: Reducer<State, Action>,
    ParentR: Reducer<ParentState, ParentAction>,
    Action: std::fmt::Debug,
    ParentAction: std::fmt::Debug,
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
    Action: std::fmt::Debug,
    ParentAction: std::fmt::Debug,
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
        ChildAction: std::fmt::Debug,
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

pub struct Effect<Action: std::fmt::Debug> {
    value: EffectValue<Action>,
}

#[derive(Debug)]
enum EffectValue<Action: std::fmt::Debug> {
    None,
    Send(Action),
    Quit,
}

impl<Action: std::fmt::Debug> Effect<Action> {
    pub fn map<F, MappedAction: std::fmt::Debug>(self, map: F) -> Effect<MappedAction>
    where
        F: FnOnce(Action) -> MappedAction,
    {
        match self.value {
            EffectValue::None => Effect::none(),
            EffectValue::Quit => Effect::quit(),
            EffectValue::Send(a) => Effect::send(map(a)),
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
