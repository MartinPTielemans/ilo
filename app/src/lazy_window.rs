//! Overlay windows made the first time one is shown, not with the first browser window: a hidden
//! GPUI window still holds what its drawing needs, which a user who never opens it shouldn't pay
//! for (#2138). The command bar's is still made ahead, as it's the hot one.

use std::any::TypeId;
use std::collections::HashMap;

use gpui::{App, Global};

type Opener = Box<dyn FnOnce(&mut App)>;

#[derive(Default)]
struct Openers(HashMap<TypeId, Opener>);

impl Global for Openers {}

/// Has `open` make view `V`'s window, and add it to the host, the first time [`ready`] asks.
pub fn when_asked<V: 'static>(open: impl FnOnce(&mut App) + 'static, cx: &mut App) {
    cx.default_global::<Openers>()
        .0
        .insert(TypeId::of::<V>(), Box::new(open));
}

/// Whether view `V`'s window is still to be made, for a caller that asks whether it can show one.
pub fn pending<V: 'static>(cx: &App) -> bool {
    cx.try_global::<Openers>()
        .is_some_and(|openers| openers.0.contains_key(&TypeId::of::<V>()))
}

/// Makes view `V`'s window now if it's still to be made, before it's shown.
pub fn ready<V: 'static>(cx: &mut App) {
    let open = cx
        .try_global::<Openers>()
        .is_some()
        .then(|| cx.global_mut::<Openers>().0.remove(&TypeId::of::<V>()))
        .flatten();
    if let Some(open) = open {
        open(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    struct Card;
    struct Menu;

    #[gpui::test]
    fn a_window_is_made_once_when_first_asked_and_only_its_own(cx: &mut gpui::TestAppContext) {
        let made = Rc::new(Cell::new(0));
        cx.update(|cx| {
            let counted = made.clone();
            when_asked::<Card>(move |_| counted.set(counted.get() + 1), cx);
            ready::<Menu>(cx);
            assert_eq!(
                made.get(),
                0,
                "asking for another view's window made this one"
            );
            assert!(pending::<Card>(cx) && !pending::<Menu>(cx));
            ready::<Card>(cx);
            assert!(!pending::<Card>(cx));
            ready::<Card>(cx);
            assert_eq!(made.get(), 1);
        });
    }

    #[gpui::test]
    fn asking_with_no_opener_does_nothing(cx: &mut gpui::TestAppContext) {
        cx.update(ready::<Card>);
    }
}
