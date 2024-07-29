use crate::tca::action_sender::ActionSender;
use crate::tca::effect::Effect;
use crate::tca::effect::EffectValue;
use crate::tca::reducer::Reducer;
use crossterm::event::{Event, EventStream};
use futures::future::BoxFuture;
use futures::{lock::Mutex, FutureExt, StreamExt};
use std::sync::Arc;

type EventSender<T> = tokio::sync::mpsc::UnboundedSender<T>;
type EventReceiver<T> = tokio::sync::mpsc::UnboundedReceiver<T>;

pub struct Store<R, State, Action>
where
    R: Reducer<State, Action>,
    Action: std::marker::Send,
    State: PartialEq + Clone + std::marker::Send,
{
    state: Arc<Mutex<State>>,
    reducer: R,
    event_sender: EventSenderHolder<Action>,
    event_reciever: Arc<Mutex<EventReceiver<StoreEvent<Action>>>>,
    phantom_action: std::marker::PhantomData<Action>,
}

pub enum StoreEvent<Action> {
    RedrawUI,
    Action(Action),
    Quit,
}

impl<R, State, Action> Store<R, State, Action>
where
    R: Reducer<State, Action> + std::marker::Sync,
    Action: std::fmt::Debug + std::marker::Send,
    State: PartialEq + Clone + std::marker::Send,
{
    pub fn new(state: State, reducer: R) -> Self {
        let (event_sender, event_reciever) =
            tokio::sync::mpsc::unbounded_channel::<StoreEvent<Action>>();
        Self {
            state: Arc::new(Mutex::new(state)),
            reducer,
            event_sender: EventSenderHolder::new(event_sender),
            event_reciever: Arc::new(Mutex::new(event_reciever)),
            phantom_action: std::marker::PhantomData,
        }
    }

    pub async fn run(
        &self,
        mut redraw: impl FnMut(&State),
        event_mapper: impl Fn(Event) -> Action,
        terminal_events: &mut EventStream,
    ) {
        let mut receiver_guard = self.event_reciever.lock().await;

        {
            let state = self.state.lock().await;
            redraw(&state);
        };
        loop {
            let crossterm_event = terminal_events.next().fuse();
            tokio::select! {
                Some(evt) = receiver_guard.recv() => {
                    match evt {
                    StoreEvent::RedrawUI => {
                        let state = self.state.lock().await;
                        redraw(&state);
                    },
                    StoreEvent::Action(action) => self.internal_send(action).await,
                    StoreEvent::Quit => { break }
                    }
                }
                maybe_event = crossterm_event => {
                    match maybe_event {
                      Some(Ok(evt)) => {
                        self.send(event_mapper(evt));
                      }
                      _ => { continue }
                    }
                },

            }
        }
    }

    pub fn send(&self, action: Action) {
        (&self.event_sender).send(action);
    }

    async fn internal_send(&self, action: Action) {
        handle(
            Effect::send(action),
            self.state.clone(),
            &self.event_sender,
            &self.reducer,
        )
        .await;
    }
}

fn handle<
    'a,
    State: Clone + std::cmp::PartialEq + std::marker::Send + 'a,
    Action: std::marker::Send + std::fmt::Debug,
    R: Reducer<State, Action> + std::marker::Sync,
>(
    effect: Effect<'a, Action>,
    state: Arc<Mutex<State>>,
    event_sender: &'a EventSenderHolder<Action>,
    reducer: &'a R,
) -> BoxFuture<'a, ()> {
    async move {
        log::debug!("Handling {:#?}", effect.value);
        match effect.value {
            EffectValue::None => {}
            EffectValue::Send(action) => {
                let effect = {
                    let mut state = state.lock().await;
                    let state_before = state.clone();
                    let effect = reducer.reduce(&mut state, action);
                    if state_before != *state {
                        event_sender.send_event(StoreEvent::RedrawUI);
                    }
                    effect
                };
                handle(effect, state, event_sender, reducer).await;
            }
            EffectValue::Quit => event_sender.send_event(StoreEvent::Quit),
            EffectValue::Async(job) => job(Box::new(event_sender)).await,
        }
    }
    .boxed()
}

struct EventSenderHolder<Action>
where
    Action: std::marker::Send,
{
    event_sender: EventSender<StoreEvent<Action>>,
}

impl<Action> EventSenderHolder<Action>
where
    Action: std::marker::Send,
{
    fn new(event_sender: EventSender<StoreEvent<Action>>) -> Self {
        Self { event_sender }
    }

    fn send_event(&self, evt: StoreEvent<Action>) {
        self.event_sender.send(evt).unwrap();
    }
}

impl<Action> ActionSender for &EventSenderHolder<Action>
where
    Action: std::marker::Send,
{
    type SendableAction = Action;

    fn send(&self, action: Action) {
        self.send_event(StoreEvent::Action(action));
    }
}
