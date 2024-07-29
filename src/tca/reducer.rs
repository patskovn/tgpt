use crate::Effect;

pub trait Reducer<State, Action: std::marker::Send> {
    fn reduce<'effect>(&self, state: &mut State, action: Action) -> Effect<'effect, Action>;
}
