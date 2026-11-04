//! Chrome's Battery Saver, its Energy Saver, as ilo shows it (#686): a toast as it turns on below
//! 20 % on battery, with Turn Off; a leaf among the URL field's buttons while it's on, whose press
//! shows the toast again; Turn Off Battery Saver for Now in the command bar while it's on; and a
//! page it froze out of sight says so in its tab's hover card. Plugging in turns it off.

use std::rc::Rc;
use std::time::Duration;

use gpui::{App, Context};

use super::Shell;

/// As long as the toast of a new tab opened unseen stays.
const NOTICE_TIME: Duration = Duration::from_millis(4800);

/// What the toast says as Battery Saver turns on at `percent`, or before the battery's read (-1).
pub fn notice(percent: i32) -> String {
    match percent {
        0.. => format!("Battery Saver is on at {percent} %"),
        _ => "Battery Saver is on".into(),
    }
}

/// What it says once it's turned off for now.
pub const TURNED_OFF: &str = "Battery Saver is off until you plug in";

/// What a frozen tab's hover card says in its host's place.
pub const PAUSED: &str = "Paused to save battery";

impl Shell {
    /// Battery Saver turned on at the battery's `percent` (`Some`), or off.
    pub fn set_battery_saver(&mut self, on: Option<i32>, cx: &mut Context<Self>) {
        if self.battery_saver == on {
            return;
        }
        if on.is_none() && self.battery_leaf_drawn.take().is_some() {
            // check.sh reads this line.
            log!("browser: battery saver leaf hidden");
        }
        self.battery_saver = on;
        cx.notify();
    }

    /// Battery Saver's percent while it's on.
    pub fn battery_saver(&self) -> Option<i32> {
        self.battery_saver
    }

    /// Says Battery Saver is on, with Turn Off, as it turns on or its leaf is pressed.
    pub fn show_battery_notice(&mut self, cx: &mut Context<Self>) {
        let Some(percent) = self.battery_saver else {
            return;
        };
        let shell = cx.entity().downgrade();
        let off = Rc::new(move |cx: &mut App| {
            shell
                .update(cx, |shell, cx| shell.turn_off_battery_saver(cx))
                .ok();
        });
        self.browser.toast_button(
            &notice(percent),
            ("Turn Off", "turn off battery saver"),
            off,
            NOTICE_TIME,
            cx,
        );
    }

    /// Turns Battery Saver off until the computer's plugged in, and says so. The core says it's
    /// off, which takes the leaf away.
    pub(super) fn turn_off_battery_saver(&mut self, cx: &mut Context<Self>) {
        if self.battery_saver.is_none() {
            return;
        }
        self.browser.battery_saver_off_for_now(cx);
        self.browser.toast(TURNED_OFF, None, NOTICE_TIME, cx);
    }

    /// `tab`'s page froze, as Battery Saver freezes one busy out of sight, or thawed.
    pub fn set_tab_frozen(&mut self, tab: u64, frozen: bool, cx: &mut Context<Self>) {
        let changed = match frozen {
            true => self.frozen.insert(tab),
            false => self.frozen.remove(&tab),
        };
        if changed {
            cx.notify();
        }
    }

    /// Whether `tab`'s page is frozen.
    pub(super) fn frozen(&self, tab: u64) -> bool {
        self.frozen.contains(&tab)
    }
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use super::*;
    use crate::hover_card;
    use crate::shell::{Command, Folders, Page, drag::detents::shell_with};

    /// Turned on, it says so with the battery's percent and Turn Off, which turns it off for now
    /// and says until when; the command bar offers the same only while it's on.
    #[gpui::test]
    fn the_notice_turns_battery_saver_off_for_now(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[], &[1], Folders::default());
        let offered = |cx: &mut TestAppContext| {
            shell.read_with(cx, |shell, _| shell.offers(Command::TurnOffBatterySaver))
        };
        assert!(!offered(cx), "Turn Off offered with Battery Saver off");
        shell.update(cx, |shell, cx| {
            shell.set_battery_saver(Some(19), cx);
            shell.show_battery_notice(cx);
        });
        assert!(offered(cx), "Turn Off not offered with Battery Saver on");
        assert_eq!(host.toasts.take(), ["Battery Saver is on at 19 %"]);
        let (label, run) = host.toast_run.take().expect("the notice has no button");
        assert_eq!(label, "Turn Off");
        cx.update(|cx| run(cx));
        assert!(
            host.take()
                .contains(&"battery saver off for now".to_owned())
        );
        assert_eq!(host.toasts.take(), [TURNED_OFF]);
        // The core says it's off, and the command goes with it.
        shell.update(cx, |shell, cx| shell.set_battery_saver(None, cx));
        assert!(!offered(cx), "Turn Off offered once Battery Saver's off");
    }

    /// The command bar's row does what the toast's button does.
    #[gpui::test]
    fn the_command_bar_turns_battery_saver_off_for_now(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[], &[1], Folders::default());
        shell.update(cx, |shell, cx| {
            shell.set_battery_saver(Some(12), cx);
            shell.run(Command::TurnOffBatterySaver, "ui", cx);
        });
        assert!(
            host.take()
                .contains(&"battery saver off for now".to_owned())
        );
        assert_eq!(host.toasts.take(), [TURNED_OFF]);
    }

    /// A page Battery Saver froze says so in its tab's hover card, and thawed it doesn't.
    #[gpui::test]
    fn a_frozen_tabs_card_says_its_paused(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[], &[1, 2], Folders::default());
        shell.update(cx, |shell, cx| {
            let page = Page {
                url: "https://busy.test/".into(),
                ..Default::default()
            };
            shell.set_page(2, page, cx);
        });
        let paused = |cx: &mut TestAppContext| {
            shell.read_with(cx, |shell, _| shell.card(2).map(|card| card.paused))
        };
        assert_eq!(paused(cx), Some(false));
        shell.update(cx, |shell, cx| {
            shell.card_event(2, hover_card::Event::Shown, cx)
        });
        assert!(host.take().contains(&"card picture 2".to_owned()));
        shell.update(cx, |shell, cx| shell.set_tab_frozen(2, true, cx));
        assert_eq!(paused(cx), Some(true));
        // Its card keeps the picture it had, as taking one would thaw the page.
        host.take();
        shell.update(cx, |shell, cx| {
            shell.card_event(2, hover_card::Event::Shown, cx)
        });
        assert!(
            !host
                .take()
                .iter()
                .any(|call| call.starts_with("card picture")),
            "the frozen page's card took its picture again"
        );
        shell.update(cx, |shell, cx| shell.set_tab_frozen(2, false, cx));
        assert_eq!(paused(cx), Some(false));
    }

    #[test]
    fn the_notice_says_the_percent_once_the_battery_is_read() {
        assert_eq!(notice(19), "Battery Saver is on at 19 %");
        assert_eq!(notice(-1), "Battery Saver is on");
    }
}
