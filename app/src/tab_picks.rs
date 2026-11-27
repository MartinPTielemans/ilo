//! The user's tabs picked for a message before it is sent: the add menu's row that lists them and
//! the chips over the field, one form for the bar's composer and the reply field under a thread.
//! A pick is the user's click in that menu and nothing else, kept in memory by the view that
//! draws it until its send, which alone turns it into what a thread is handed
//! (`handed::Picked`). No tool, command word, page or typed text reaches one.

use std::sync::Arc;

use gpui::{
    AnyElement, App, Hsla, Image, Role, SharedString, Window, div, img, prelude::*, px, svg,
};

use crate::agents::task::handed::Picked;
use crate::shell::MenuEntry;
use crate::tokens::tab_picks::{
    BETWEEN, CHIP_GAP, CHIP_HEIGHT, CHIP_LEFT, CHIP_MAX, CHIP_RADIUS, CHIP_RIGHT, CLOSE, ICON,
    INSET, NOT_HANDED, TITLE, TITLE_LINE,
};

/// Z-tab Z33: the add menu's row that lists the user's tabs.
pub const YOUR_TABS: &str = "Your Tabs";
const YOUR_TABS_ICON: &str = "icons/app-window.svg";
/// Z33a: the second menu's first row, which picks every tab it lists (#4278).
pub const ALL_TABS: &str = "All Tabs";
const NO_FAVICON: &str = "icons/globe.svg";
const CLOSE_ICON: &str = "icons/x.svg";

/// Z-tab Z36: what a screen reader says for the ✕ of the chip of the tab titled `title`.
pub fn remove(title: &str) -> String {
    format!("Remove {title}")
}

/// Z-tab Z40: what a screen reader says for the chip, in a sent message, of the tab titled
/// `title` that was picked and did not go.
pub fn not_moved(title: &str) -> String {
    format!("{title}, not moved to the thread")
}

/// A tab of the user's the composer may offer, as its menu lists it and its chip shows it.
#[derive(Clone)]
pub struct Offered {
    pub tab: u64,
    pub title: SharedString,
    pub favicon: Option<Arc<Image>>,
}

/// Which tab, never its title: no failed test names a tab of the user's.
impl std::fmt::Debug for Offered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Offered({})", self.tab)
    }
}

/// The tabs of the Space in view the composer may offer now, in the sidebar's order
/// (`Shell::tabs_to_offer`).
#[derive(Clone, Debug, Default)]
pub struct Offer {
    pub space: u64,
    pub tabs: Vec<Offered>,
}

/// The tabs picked for the message being written, in the order picked, with the Space they are
/// of. In memory alone, with the composer that holds it.
#[derive(Default)]
pub struct Picks {
    space: u64,
    chips: Vec<Offered>,
}

/// How many, and nothing of which.
impl std::fmt::Debug for Picks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Picks({})", self.chips.len())
    }
}

impl Picks {
    pub fn is_empty(&self) -> bool {
        self.chips.is_empty()
    }

    pub fn chips(&self) -> &[Offered] {
        &self.chips
    }

    pub fn tabs(&self) -> Vec<u64> {
        self.chips.iter().map(|chip| chip.tab).collect()
    }

    /// The user's click on `offered`'s row of the menu for the Space `space`: picks it, or
    /// unpicks it if it was picked. Whether it is picked now. Picks of another Space go first:
    /// they are that Space's.
    pub fn flip(&mut self, space: u64, offered: Offered) -> bool {
        if self.space != space {
            self.chips.clear();
            self.space = space;
        }
        if self.unpick(offered.tab) {
            return false;
        }
        self.chips.push(offered);
        true
    }

    /// The user's click on the menu's All Tabs row for the Space `space`: with `pick`, picks each
    /// of `listed` not picked yet, after those that are, in the menu's order; else unpicks each.
    pub fn set_all(&mut self, space: u64, listed: &[Offered], pick: bool) {
        if self.space != space {
            self.chips.clear();
            self.space = space;
        }
        let picked = |chips: &[Offered], tab: u64| chips.iter().any(|chip| chip.tab == tab);
        if !pick {
            return self.chips.retain(|chip| !picked(listed, chip.tab));
        }
        for each in listed {
            if !picked(&self.chips, each.tab) {
                self.chips.push(each.clone());
            }
        }
    }

    /// The user's press on `tab`'s chip's ✕. Whether it was picked.
    pub fn unpick(&mut self, tab: u64) -> bool {
        let before = self.chips.len();
        self.chips.retain(|chip| chip.tab != tab);
        self.chips.len() != before
    }

    /// ⌫ in the empty field: the last chip goes. Whether there was one.
    pub fn unpick_last(&mut self) -> bool {
        self.chips.pop().is_some()
    }

    /// Keeps the chips whose tabs `offer` still has, each as its tab is now, and silently drops
    /// the rest: a tab closed, moved to another Space or no longer one to hand. Whether a chip
    /// went or changed.
    pub fn keep(&mut self, offer: &Offer) -> bool {
        if offer.space != self.space {
            return !std::mem::take(&mut self.chips).is_empty();
        }
        let mut changed = false;
        self.chips.retain_mut(|chip| {
            let now = offer.tabs.iter().find(|each| each.tab == chip.tab);
            let Some(now) = now else {
                changed = true;
                return false;
            };
            let same_icon = match (&chip.favicon, &now.favicon) {
                (Some(was), Some(is)) => Arc::ptr_eq(was, is),
                (None, None) => true,
                _ => false,
            };
            if chip.title != now.title || !same_icon {
                (*chip, changed) = (now.clone(), true);
            }
            true
        });
        changed
    }

    /// The send: what the message hands, and no pick is left.
    pub fn take(&mut self) -> Picked {
        let tabs = self.chips.drain(..).map(|chip| chip.tab).collect();
        Picked::of_chips(self.space, tabs)
    }

    pub fn clear(&mut self) {
        self.chips.clear();
    }
}

/// The add menu's row for the user's tabs, with its second menu. `listed` is asked once, as the
/// add menu opens and not before, for the tabs to list and which of them are picked; with none
/// to list the row is not in the menu. A click on a tab's row calls `flip`, which answers
/// whether it is picked now, and the menu stays open. With two tabs or more, All Tabs comes
/// first: its click calls `set_all` with the tabs listed, to unpick them all where it showed each
/// picked and else to pick them all, and closes the menu, as the other rows' checks would be
/// left as they were.
pub fn row(
    listed: impl FnOnce(&mut App) -> (Offer, Vec<u64>) + 'static,
    flip: impl Fn(u64, Offered, &mut App) -> bool + Clone + 'static,
    set_all: impl FnOnce(u64, Vec<Offered>, bool, &mut App) + 'static,
) -> crate::plus::Row {
    let entry = MenuEntry::new(YOUR_TABS, true).icon(YOUR_TABS_ICON);
    crate::plus::Row::over(entry, move |cx| {
        let (offer, picked) = listed(cx);
        let all = all_checked(&offer, &picked).map(|checked| {
            let entry = MenuEntry {
                checked,
                spoken: Some(all_tabs_spoken(offer.tabs.len()).into()),
                ..MenuEntry::new(ALL_TABS, true)
            };
            let (space, tabs) = (offer.space, offer.tabs.clone());
            crate::plus::Row::new(entry, move |_, cx| set_all(space, tabs, !checked, cx))
        });
        let tabs = entries(&offer, &picked, flip).into_iter();
        all.into_iter()
            .chain(tabs.map(crate::plus::Row::staying))
            .collect()
    })
}

/// What a screen reader says for All Tabs (Z33b): its check is the row's role's to say.
fn all_tabs_spoken(tabs: usize) -> String {
    format!("{}, {tabs} tabs", MenuEntry::label(ALL_TABS))
}

/// The All Tabs row's check, where the menu has the row: with two tabs or more listed, whether
/// each is picked. With one it would be that tab's row again.
fn all_checked(offer: &Offer, picked: &[u64]) -> Option<bool> {
    (offer.tabs.len() >= 2).then(|| (offer.tabs.iter()).all(|each| picked.contains(&each.tab)))
}

/// The second menu's rows: `offer`'s tabs by icon and title, those in `picked` checked.
fn entries(
    offer: &Offer,
    picked: &[u64],
    flip: impl Fn(u64, Offered, &mut App) -> bool + Clone + 'static,
) -> Vec<MenuEntry> {
    let space = offer.space;
    (offer.tabs.iter())
        .map(|offered| {
            let (flip, tab) = (flip.clone(), offered.clone());
            MenuEntry {
                page: true,
                favicon: offered.favicon.clone(),
                checked: picked.contains(&offered.tab),
                ..MenuEntry::named(offered.title.clone(), true)
            }
            .stays(move |cx| flip(space, tab.clone(), cx))
        })
        .collect()
}

/// How a field draws its chips: their fill, their title's ink, the ✕'s, and whether a screen
/// reader is on.
pub struct Look {
    pub fill: Hsla,
    pub ink: Hsla,
    pub dim: Hsla,
    pub a11y: bool,
}

/// A page's icon at a chip's size: its favicon, or the globe without one or until it is decoded.
fn icon(favicon: Option<Arc<Image>>, ink: Hsla) -> AnyElement {
    let globe = move || {
        svg()
            .path(NO_FAVICON)
            .size(px(ICON))
            .flex_none()
            .text_color(ink)
            .into_any_element()
    };
    match favicon {
        Some(favicon) => img(favicon)
            .size(px(ICON))
            .flex_none()
            .with_loading(globe)
            .into_any_element(),
        None => globe(),
    }
}

/// A chip's ground with its icon and title, `id` its own among its neighbours.
fn chip(id: (&'static str, u64), offered: &Offered, look: &Look) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .max_w(px(CHIP_MAX))
        .h(px(CHIP_HEIGHT))
        .pl(px(CHIP_LEFT))
        .pr(px(CHIP_RIGHT))
        .flex()
        .items_center()
        .gap(px(CHIP_GAP))
        .rounded(px(CHIP_RADIUS))
        .bg(look.fill)
        .text_color(look.ink)
        .text_size(px(TITLE))
        .line_height(px(TITLE_LINE))
        .child(icon(offered.favicon.clone(), look.ink))
        .child(div().min_w_0().truncate().child(offered.title.clone()))
}

/// The chips of `picks` over a composer's field, wrapping, each with its ✕, whose press calls
/// `unpick` with its tab. Drawn only with a pick.
pub fn chips(
    picks: &Picks,
    look: &Look,
    unpick: impl Fn(u64, &mut Window, &mut App) + Clone + 'static,
) -> gpui::Div {
    let each = picks.chips.iter().map(|offered| {
        let (unpick, tab) = (unpick.clone(), offered.tab);
        let close = div()
            .id(("tab-pick-remove", tab))
            .flex_none()
            .cursor_pointer()
            .when(look.a11y, |close| {
                close.role(Role::Button).aria_label(remove(&offered.title))
            })
            .on_click(move |_, window, cx| unpick(tab, window, cx))
            .child(
                svg()
                    .path(CLOSE_ICON)
                    .size(px(CLOSE))
                    .flex_none()
                    .text_color(look.dim),
            );
        chip(("tab-pick", tab), offered, look).child(close)
    });
    div()
        .flex_none()
        .px(px(INSET))
        .pt(px(INSET))
        .flex()
        .flex_wrap()
        .gap(px(BETWEEN))
        .children(each)
}

/// A sent message's chips, inside its plate over its words: one per tab picked for it, by its
/// title at the send, with no ✕. One whose tab did not go is drawn fainter and spoken as such.
pub fn sent(message: usize, chips: &[crate::agents::task::handed::Chip], look: &Look) -> gpui::Div {
    let each = chips.iter().enumerate().map(|(at, sent)| {
        let offered = Offered {
            tab: sent.tab,
            title: sent.title.clone(),
            favicon: None,
        };
        let id = ("tab-sent", (message as u64) << 16 | at as u64);
        chip(id, &offered, look)
            .when(!sent.handed, |chip| chip.opacity(NOT_HANDED))
            .when(look.a11y && !sent.handed, |chip| {
                chip.aria_label(not_moved(&sent.title))
            })
    });
    div().flex().flex_wrap().gap(px(BETWEEN)).children(each)
}

#[cfg(test)]
mod tests;
