use crate::tca::action_sender::ActionSender;
use crate::tca::effect::Effect;
use crate::tca::effect::EffectValue;
use crate::tca::event_sender_holder::EventSenderHolder;
use crate::tca::reducer::Reducer;
use crate::tca::store_event::StoreEvent;
use async_trait::async_trait;
use crossterm::event::{Event, EventStream};
use futures::future::BoxFuture;
use futures::{lock::Mutex, FutureExt, StreamExt};
use std::ops::Deref;
use std::sync::Arc;

type EventReceiver<T> = tokio::sync::mpsc::UnboundedReceiver<T>;

pub struct StoreEngine<'a, State, Action>
where
    Action: std::marker::Send,
    State: PartialEq + Clone + std::marker::Send,
{
    state: Arc<Mutex<State>>,
    reducer: Box<dyn Reducer<State, Action> + std::marker::Sync + std::marker::Send + 'a>,
    event_sender: EventSenderHolder<Action>,
    event_reciever: Arc<Mutex<EventReceiver<StoreEvent<Action>>>>,
}

impl<'a, State, Action> StoreEngine<'a, State, Action>
where
    Action: std::fmt::Debug + std::marker::Send,
    State: PartialEq + Clone + std::marker::Send,
{
    pub fn new<R: Reducer<State, Action> + std::marker::Sync + 'a + std::marker::Send>(
        state: State,
        reducer: R,
    ) -> Self {
        let (event_sender, event_reciever) =
            tokio::sync::mpsc::unbounded_channel::<StoreEvent<Action>>();
        Self {
            state: Arc::new(Mutex::new(state)),
            reducer: Box::new(reducer),
            event_sender: EventSenderHolder::new(event_sender),
            event_reciever: Arc::new(Mutex::new(event_reciever)),
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

    async fn internal_send(&self, action: Action) {
        Self::handle(
            Effect::send(action),
            self.state.clone(),
            &self.event_sender,
            self.reducer.deref(),
        )
        .await;
    }

    fn handle<'b>(
        effect: Effect<'b, Action>,
        state: Arc<Mutex<State>>,
        event_sender: &'b EventSenderHolder<Action>,
        reducer: &'b (dyn Reducer<State, Action> + std::marker::Sync),
    ) -> BoxFuture<'b, ()> {
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
                    Self::handle(effect, state, event_sender, reducer).await;
                }
                EffectValue::Quit => event_sender.send_event(StoreEvent::Quit),
                EffectValue::Async(job) => job(Box::new(event_sender)).await,
            }
        }
        .boxed()
    }
}

impl<'a, State, Action> ActionSender for StoreEngine<'a, State, Action>
where
    Action: std::marker::Send,
    State: PartialEq + Clone + std::marker::Send,
{
    type SendableAction = Action;

    fn send(&self, action: Action) {
        (&self.event_sender).send(action);
    }
}
