//! The tabs of the user's the composer may offer to hand to a thread: which the window lists, and
//! of those which the app would hand (`crate::tab_picks`).

use gpui::App;

use super::Shell;
use crate::tab_picks::{Offer, Offered};

impl Shell {
    /// The tabs of the Space in view the composer's menu is drawn from, in the sidebar's order:
    /// today's, its folders' with them; none in a private window. Favourites and pinned tabs
    /// stay out while one a thread holds has no row or tile to be found by; with that drawn they
    /// go first, the favourites (`self.favorites`), then the pinned (`self.pinned()`, without
    /// its folders' rows).
    fn tabs_to_list(&self) -> Vec<u64> {
        match self.private {
            true => Vec::new(),
            false => self.today().to_vec(),
        }
    }

    /// Whether the composer's add menu may have its row for the user's tabs: the window lists
    /// some. Of the window's own lists alone, so a frame may ask it.
    pub fn lists_tabs_to_offer(&self) -> bool {
        !self.tabs_to_list().is_empty()
    }

    fn offered(&self, tab: u64) -> Offered {
        Offered {
            tab,
            title: self.title(tab),
            favicon: self.tabs.get(&tab).and_then(|tab| tab.favicon.clone()),
        }
    }

    /// The tabs the window lists for the composer whose page is one of the web's, each as it is
    /// now: what a composer's chips are kept against as the window's tabs change, where a tab
    /// closed, moved to another Space or gone to a page of the browser's own loses its chip. Of
    /// the window's own lists alone: nothing is asked of the agents or the core.
    pub fn tabs_listed(&self) -> Offer {
        let web = |tab: &u64| {
            let url = self.page(*tab).url;
            url.starts_with("https://") || url.starts_with("http://")
        };
        let tabs = (self.tabs_to_list().into_iter().filter(web))
            .map(|tab| self.offered(tab))
            .collect();
        Offer {
            space: self.space,
            tabs,
        }
    }

    /// What the composer may offer to hand to `agent`'s thread, or with none to one not yet
    /// started: of the tabs the window lists for it, those the app would hand, each as it is
    /// now. The composer's alone, asked as its add menu opens: never at launch, on a tab switch,
    /// as the bar opens, per key or at the send. It reads the agents (`Browser::tabs_to_hand`),
    /// whose answer reads this window, so it is never asked from inside an update of theirs or
    /// of this window's own: the plus's click is in neither.
    pub fn tabs_to_offer(&self, agent: Option<&str>, cx: &App) -> Offer {
        let space = self.space;
        let listed = self.tabs_to_list();
        let fit = (self.browser).tabs_to_hand(agent, space, &listed, cx);
        let tabs = fit.into_iter().map(|tab| self.offered(tab)).collect();
        Offer { space, tabs }
    }
}

#[cfg(test)]
mod tests;
