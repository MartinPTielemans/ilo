//! The page an empty Space rests on (#4140): the command bar where ⌘T's opens, whose ⇥ grows it
//! into the composer in place, and under it the recents (`recents.rs`). The bar is the same bar as
//! ⌘T's (`command_bar::in_place`), drawn in the page, with its own keys. Nothing of the Archive is
//! read for the page's first frame, and the bar asks the core nothing until it's typed in.

use gpui::{
    Bounds, Context, Entity, Hsla, Pixels, SharedString, Window, div, point, prelude::*, px, rgba,
};

use crate::agents::task::Card;
use crate::command_bar::CommandBar;
use crate::palette::{Palette, Tone};
use crate::shell::ShowStarted;
use crate::tokens::command_bar::{REST_HEIGHT as BAR_HEIGHT, REST_WIDTH as BAR_WIDTH};
use crate::tokens::shell::composer::*;

use super::super::Shell;

/// The bar the page rests on, while the Space shows no page.
pub(in crate::shell) struct Resting {
    pub(in crate::shell) bar: Entity<CommandBar>,
    /// The page has drawn its first frame, from when it draws the tasks.
    painted: bool,
    /// Whether the field is owed the keyboard, which it takes once, at the first frame that may
    /// give it (`give_resting_keys`), so whatever the user moves the keyboard to after keeps it.
    wants_keys: Owed,
    /// The Space it rests in.
    space: u64,
    /// What the recents take the keyboard by, and the row it's on while they have it.
    pub(super) recents: gpui::FocusHandle,
    pub(super) recent: usize,
}

/// Where a chip's menu hangs: 4 under the chip, its names under the chip's own word, or at the
/// click before the chip is drawn.
fn hung_under(chip: Option<Bounds<Pixels>>, clicked: gpui::Point<Pixels>) -> gpui::Point<Pixels> {
    let Some(chip) = chip else {
        return clicked;
    };
    let names = crate::menu::PADDING + crate::menu::ROW_X + crate::menu::CHECK + crate::menu::GAP;
    chip.origin + point(px(CHIP_INSET - names), chip.size.height + px(MENU_DROP))
}

/// The level's menu's last row, which opens Settings › Agents at Access.
pub(in crate::shell) const SETTINGS: &str = "Settings…";

/// Why the field is owed the keyboard, if it is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Owed {
    No,
    /// The Space came to show no page: the field takes the keyboard only from the window itself,
    /// never from a field or form the user is in.
    Shown,
    /// ⌘T or ⌘L asked for it.
    Asked,
}

impl Shell {
    /// Whether the card rests on the bar: the Space shows no page, the welcome hasn't the card,
    /// and the window isn't a Little ilo window's, which keeps its own bar inside it.
    pub(in crate::shell) fn rests_on_bar(&self) -> bool {
        self.selected == 0 && !self.welcome && self.little.is_none()
    }

    /// Makes the bar the page rests on as the Space comes to show no page, and drops it as a page
    /// shows, or the welcome takes the card.
    pub(in crate::shell) fn step_resting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.rests_on_bar() {
            if let Some(resting) = self.resting.take() {
                // GPUI's focus would go with the field, and the shortcuts with it, as the shell's
                // context holds them, until a click lands on the shell, which a header button's
                // never does. The shell holds it again; a page that took the keyboard keeps it.
                let held = resting.bar.read(cx).field_focus(cx).is_focused(window)
                    || resting.recents.is_focused(window);
                if held {
                    if self.page_keyboard.is_some() && self.page_has_keyboard(window, cx) {
                        self.page_keyboard = Some(Some(self.focus_handle.downgrade()));
                    }
                    window.focus(&self.focus_handle, cx);
                }
                log!("browser: resting field dropped");
            }
            return;
        }
        let space = self.space();
        if let Some(resting) = &mut self.resting {
            // Another Space empty too: its field is owed the keyboard, once the swipe settles.
            if resting.space != space {
                resting.space = space;
                resting.wants_keys = Owed::Shown;
                let bar = resting.bar.clone();
                bar.update(cx, |bar, cx| bar.space_changed(window, cx));
            }
            return self.give_resting_keys(window, cx);
        }
        let bar = self.browser.command_bar_in_place(window, cx);
        self.resting = Some(Resting {
            bar,
            painted: false,
            wants_keys: Owed::Shown,
            space,
            recents: cx.focus_handle(),
            recent: 0,
        });
        cx.on_next_frame(window, |shell, _, cx| {
            if let Some(resting) = &mut shell.resting {
                resting.painted = true;
                cx.notify();
            }
        });
        log!("browser: resting field made");
        self.give_resting_keys(window, cx);
    }

    /// ⌘T and ⌘L while the Space shows no page: the field takes the keyboard, and no bar opens
    /// over it (#4140). Whether it did.
    pub(in crate::shell) fn keys_to_resting(&mut self, cx: &mut Context<Self>) -> bool {
        // The field stays until the next frame drops it, so a page shown since isn't one.
        if !self.rests_on_bar() {
            return false;
        }
        let Some(resting) = &mut self.resting else {
            return false;
        };
        // The core shows a page the shell hasn't heard of yet, as at launch with a page to open:
        // the keys are that page's, and open the bar over it (#4268).
        if self.browser.shown_now() != 0 {
            return false;
        }
        resting.wants_keys = Owed::Asked;
        cx.notify();
        true
    }

    /// Gives the field the keyboard it's owed once the window is key, no overlay or Library is
    /// over it and no swipe is moving the pages: an overlay keeps the keys it has, and the field
    /// takes them only when the swipe to its Space has settled. Owed for being shown, it leaves
    /// the keyboard where the user put it, as in a Space's name or its form.
    fn give_resting_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let still = self.swipe.is_none() && self.settle.is_none();
        let free = window.is_window_active()
            && !self.browser.command_bar_open(cx)
            && self.library.is_none();
        let root = self.focus_handle.clone();
        let Some(resting) =
            (self.resting.as_mut()).filter(|resting| resting.wants_keys != Owed::No)
        else {
            return;
        };
        if !(still && free) {
            return;
        }
        let focus = resting.bar.read(cx).field_focus(cx);
        let elsewhere = window
            .focused(cx)
            .is_some_and(|at| at != root && at != focus);
        let owed = std::mem::replace(&mut resting.wants_keys, Owed::No);
        if owed == Owed::Shown && elsewhere {
            return;
        }
        window.focus(&focus, cx);
        // GPUI's focus alone draws the caret, while the Mac sends a key to its window's first
        // responder, which a click gives the UI by itself and this hold gives it without one. Not
        // while the core already shows a page the shell hasn't heard of: the keys are that page's.
        if self.browser.shown_now() == 0 {
            self.hold_keyboard_for_page(cx);
        }
        log!("browser: resting field has the keyboard");
    }

    /// Gives the resting field the keyboard, as a click on the page does.
    pub(in crate::shell) fn focus_resting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(resting) = &self.resting {
            let focus = resting.bar.read(cx).field_focus(cx);
            window.focus(&focus, cx);
        }
    }

    /// What the browser knows of the assistants changed, as once it has looked for them: the
    /// page's bar learns them again.
    pub(in crate::shell) fn assistants_learned(&mut self, cx: &mut Context<Self>) {
        if let Some(resting) = &self.resting {
            resting.bar.update(cx, |bar, cx| bar.agents_changed(cx));
        }
    }

    /// ⌘T's ↩ is starting a task from this window's Space, over the tab it shows: as the
    /// composer's does, its conversation takes that tab's place as its card comes in.
    pub fn bar_starting(&mut self) {
        self.show_started = Some(ShowStarted::Asked {
            home: self.space(),
            over: self.selected,
        });
    }

    /// The Agents' answer to the composer's or ⌘T's last ↩: the task it started, whose view alone
    /// opens as its card comes in, or None, when it started none and no other task's card opens a
    /// view.
    pub fn composed_started(&mut self, agent: Option<SharedString>) {
        let Some(ShowStarted::Asked { over, .. }) = self.show_started else {
            return;
        };
        self.show_started = agent.map(|agent| ShowStarted::Task { agent, over });
    }

    /// The composer's and ⌘T's ↩ show the task they started in the place of the page they were
    /// asked over, the empty page or a tab, while it still shows. The card comes before the window
    /// hears of the task's Space, which the core tells after the call that made it, so until then
    /// the start waits for that Space, by its task. `was`, the cards before these, tells the new
    /// ones while the Agents haven't said which task it started.
    pub(in crate::shell) fn show_started_task(
        &mut self,
        was: Option<&[SharedString]>,
        cx: &mut Context<Self>,
    ) {
        let started = |card: &&Card| match &self.show_started {
            Some(ShowStarted::Asked { home, .. }) => {
                card.home == *home && was.is_some_and(|was| !was.contains(&card.agent))
            }
            Some(ShowStarted::Task { agent, .. }) => card.agent == *agent,
            None => false,
        };
        let Some(card) = self.task_cards.iter().find(started) else {
            return;
        };
        let over = self.show_started.as_ref().map_or(0, ShowStarted::over);
        let (agent, home, space) = (card.agent.clone(), card.home, card.space);
        if space != 0 && self.task_space(space).is_none() {
            self.show_started = Some(ShowStarted::Task { agent, over });
            return;
        }
        self.show_started = None;
        if self.selected == over && self.space() == home && self.talk_agent().is_none() {
            self.show_task(agent, cx);
        }
    }

    /// The page in the card at `frame`: the bar a quarter of the way down, and under it the
    /// recents, which give way to the bar's rows once something's typed, and to the composer's
    /// choices once ⇥ grew it. ↓ in the bar with no rows moves into them.
    pub(in crate::shell) fn resting_page(
        &mut self,
        frame: Bounds<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Div> {
        let width = f32::from(frame.size.width);
        let resting = self.resting.as_ref()?;
        let (bar, painted) = (resting.bar.clone(), resting.painted);
        let top = group_top(f32::from(frame.size.height));
        // A narrow window keeps the bar inside the card's gutters.
        let bar_width = BAR_WIDTH.min(width - 2. * FIELD_GUTTER).max(0.);
        let idle = {
            let bar = bar.read(cx);
            bar.typed_text(cx).is_empty() && !bar.composing_thread()
        };
        // Nothing of the Archive is read for the page's first frame.
        let recents = (painted && idle)
            .then(|| self.recents_plate(bar_width, window, cx))
            .flatten();
        let bar = div()
            .debug_selector(|| "resting-bar".into())
            .absolute()
            .top(px(top))
            .left(px((width - bar_width) / 2.))
            .w(px(bar_width))
            .on_action(
                cx.listener(|shell, _: &crate::command_bar::Next, window, cx| {
                    shell.into_recents(window, cx)
                }),
            )
            .child(bar);
        let hint = self.landing_hint(top, width, cx);
        Some(
            div()
                .size_full()
                .relative()
                .flex()
                .flex_col()
                .items_center()
                .pt(px(top + BAR_HEIGHT))
                .children(hint)
                .children(recents)
                // Last, so its rows and choices are over the recents.
                .child(bar),
        )
    }
}

/// How far down a card `height` tall the page's bar starts: a quarter of the way, never above
/// `TOP`.
pub(in crate::shell) fn group_top(height: f32) -> f32 {
    (height * COLUMN_DOWN).max(TOP)
}

/// A plate's rim: white `dark` on a dark window, the ink on a light one, where white doesn't show.
pub(super) fn rim(palette: &Palette, dark: u32) -> Hsla {
    match palette.tone {
        Tone::Dark => rgba(dark).into(),
        Tone::Light => palette.plate(LIGHT_RIM),
    }
}
