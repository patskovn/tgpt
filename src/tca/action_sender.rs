pub trait ActionSender: std::marker::Send {
    type SendableAction;

    fn send(&self, action: Self::SendableAction);
}
