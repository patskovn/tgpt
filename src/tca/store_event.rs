pub enum StoreEvent<Action> {
    RedrawUI,
    Action(Action),
    Quit,
}
