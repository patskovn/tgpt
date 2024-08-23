// use super::{action_mapper::ActionMapper, effect::BoxActionSender};

pub trait ActionSender: std::marker::Send + std::marker::Sync {
    type SendableAction;

    fn send(&self, action: Self::SendableAction);
}

// struct UIActionSender<'a, UIAction: std::marker::Send> {
//     val: BoxActionSender<'a, UIAction>,
// }
//
// impl<'a, UIAction> UIActionSender<'a, UIAction>
// where
//     UIAction: std::marker::Send,
// {
//     pub fn new(val: BoxActionSender<'a, UIAction>) -> Self {
//         Self { val }
//     }
//
//     pub fn scope<ChildAction>(
//         &'a self,
//         map: impl Fn(ChildAction) -> UIAction + std::marker::Send + 'a,
//     ) -> UIActionSender<'a, ChildAction>
//     where
//         ChildAction: std::marker::Send + 'a,
//     {
//         let mapper = ActionMapper::new(Box::new(self), map);
//         UIActionSender::new(Box::new(mapper))
//     }
// }
//
// impl<'a, UIAction> ActionSender for &UIActionSender<'a, UIAction>
// where
//     UIAction: std::marker::Send,
// {
//     type SendableAction = UIAction;
//
//     fn send(&self, action: UIAction) {
//         self.val.send(action)
//     }
// }
