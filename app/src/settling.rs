//! A card that grants something, a site's permission, an extension or a saved password, ignores
//! what would answer it for good until it has been on screen a moment, as Chrome's permission
//! bubbles do (`InputEventActivationProtector`). A page can time its question to come up under a
//! double-click the user already started, and the second click mustn't answer it. What only puts
//! the question off, such as ×, escape or Not Now, works at once.

use std::time::Duration;
use web_time::Instant;

use gpui::{App, Task};

/// The Mac's double-click interval as it ships, which Chrome's bubbles wait out.
pub const SETTLING: Duration = Duration::from_millis(500);

/// A card's moment on screen, by the executor's clock, which the tests move.
#[derive(Default)]
pub struct Settling {
    /// When it takes input. None until it shows.
    from: Option<Instant>,
    _logged: Option<Task<()>>,
}

impl Settling {
    /// The card came on screen, as `what` in the line check.sh waits for before it answers:
    /// "browser: `what` takes input". Called again, the moment starts over.
    pub fn start(&mut self, what: String, cx: &App) {
        let executor = cx.background_executor();
        self.from = Some(executor.now() + SETTLING);
        let settled = executor.timer(SETTLING);
        self._logged = Some(cx.foreground_executor().spawn(async move {
            settled.await;
            log!("browser: {what} takes input");
        }));
    }

    /// Whether the card has been on screen long enough to take an answer.
    pub fn settled(&self, cx: &App) -> bool {
        self.from
            .is_some_and(|from| cx.background_executor().now() >= from)
    }
}
