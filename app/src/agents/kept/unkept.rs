//! The pages whose picture a kept card can't keep for what was put in them (#4090): a page the
//! user held, as Chrome fills passwords, addresses, cards and passkeys only in a tab the user
//! holds and a fill stays after the hand-back, and a page an agent typed text into that may not be
//! shown (page::Element::shows_typed), which only its screenshot hatches. Either holds until the
//! tab's main frame has loaded a new document that started loading after it.
//!
//! Not covered: a secret field with contents nobody typed while holding the page, as one the page
//! fills itself or keys an agent pressed one by one; the app isn't told a field has contents.
//!
//! The app is told of no document, so this reads them from what it is told: a page finished
//! loading in the main frame (`loaded`, Chrome's DidFinishLoad, which a same-document navigation
//! and a page the back-forward cache brings back never send), and the tab's address and whether
//! it loads (`state`). An address that changes without a load of its own is taken as the page
//! held again, as it may be one the back-forward cache brought back: wider than the rule, never
//! narrower. Only a tab ever marked is tracked, until it closes.

use std::collections::HashMap;

#[derive(Default)]
struct Page {
    /// Its picture can't be kept.
    marked: bool,
    /// A load started since the mark that wasn't the user's, so the document it ends on is new.
    fresh: bool,
    url: Option<String>,
    loading: bool,
}

#[derive(Default)]
pub struct Unkept {
    pages: HashMap<u64, Page>,
}

impl Unkept {
    /// The user holds `tab`, or an agent typed into a field of it what may not be shown.
    pub fn mark(&mut self, tab: u64) {
        let page = self.pages.entry(tab).or_default();
        page.marked = true;
        page.fresh = false;
    }

    /// `tab`'s address and whether it loads, as the core says them; `holding`, whether the user
    /// holds it now.
    pub fn state(&mut self, tab: u64, url: &str, loading: bool, holding: bool) {
        let Some(page) = self.pages.get_mut(&tab) else {
            return;
        };
        if page.url.as_deref() != Some(url) {
            page.url = Some(url.into());
            page.marked = true;
            // An address that comes with a load is that load's document, committing.
            if !loading {
                page.fresh = false;
            }
        }
        if loading && !page.loading && !holding {
            page.fresh = true;
        }
        page.loading = loading;
    }

    /// `tab`'s main frame finished loading a document.
    pub fn loaded(&mut self, tab: u64, holding: bool) {
        let Some(page) = self.pages.get_mut(&tab) else {
            return;
        };
        if page.fresh && !holding {
            page.marked = false;
        }
        page.fresh = false;
    }

    pub fn closed(&mut self, tab: u64) {
        self.pages.remove(&tab);
    }

    /// Whether `tab`'s picture can't be kept for what was put in it.
    pub fn marked(&self, tab: u64) -> bool {
        self.pages.get(&tab).is_some_and(|page| page.marked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAB: u64 = 7;

    /// Held, then a load of the agent's that ends on a new document.
    fn held_then_loaded() -> Unkept {
        let mut unkept = Unkept::default();
        unkept.state(TAB, "https://shop.example/basket", false, false);
        unkept.mark(TAB);
        assert!(unkept.marked(TAB));
        unkept.state(TAB, "https://shop.example/basket", false, false);
        assert!(unkept.marked(TAB), "letting go is no new document");
        unkept.state(TAB, "https://shop.example/basket", true, false);
        unkept.state(TAB, "https://shop.example/paid", true, false);
        unkept.state(TAB, "https://shop.example/paid", false, false);
        unkept.loaded(TAB, false);
        unkept
    }

    /// A page never marked keeps its picture.
    #[test]
    fn a_page_no_one_held_keeps_its_picture() {
        let mut unkept = Unkept::default();
        unkept.state(TAB, "https://shop.example/", true, false);
        unkept.loaded(TAB, false);
        unkept.state(TAB, "https://shop.example/#top", false, false);
        assert!(!unkept.marked(TAB));
    }

    /// A new document after the hold, loaded by the agent, keeps its picture again.
    #[test]
    fn a_new_document_after_the_hold_keeps_its_picture() {
        assert!(!held_then_loaded().marked(TAB));
    }

    /// A same-document navigation after the hold keeps the mark, whether or not the user still
    /// holds the page.
    #[test]
    fn a_same_document_navigation_keeps_the_mark() {
        let mut unkept = Unkept::default();
        unkept.state(TAB, "https://shop.example/basket", false, false);
        unkept.mark(TAB);
        unkept.state(TAB, "https://shop.example/basket#pay", false, false);
        unkept.state(TAB, "https://shop.example/pay", false, false);
        assert!(unkept.marked(TAB));
        // A new document's load, then, clears it.
        unkept.state(TAB, "https://shop.example/pay", true, false);
        unkept.state(TAB, "https://shop.example/thanks", true, false);
        unkept.loaded(TAB, false);
        assert!(!unkept.marked(TAB));
    }

    /// A page the back-forward cache brings back, which loads nothing, may be the one held: once
    /// a tab was held, an address that changes without a load marks it again.
    #[test]
    fn a_page_brought_back_without_a_load_is_marked_again() {
        let mut unkept = held_then_loaded();
        unkept.state(TAB, "https://shop.example/basket", false, false);
        assert!(unkept.marked(TAB));
    }

    /// The document the user held finishing its own load clears nothing: only a load that
    /// started after the hold does, and none that ends while the user holds the page.
    #[test]
    fn the_held_documents_own_load_clears_nothing() {
        let mut unkept = Unkept::default();
        unkept.state(TAB, "https://shop.example/basket", true, false);
        unkept.mark(TAB);
        unkept.loaded(TAB, false);
        assert!(unkept.marked(TAB));
        unkept.state(TAB, "https://shop.example/basket", false, true);
        unkept.state(TAB, "https://shop.example/basket", true, true);
        unkept.state(TAB, "https://shop.example/paid", true, true);
        unkept.loaded(TAB, true);
        assert!(unkept.marked(TAB));
    }

    /// A load that never ends on a document, as a download's, clears nothing.
    #[test]
    fn a_load_with_no_document_clears_nothing() {
        let mut unkept = Unkept::default();
        unkept.state(TAB, "https://shop.example/basket", false, false);
        unkept.mark(TAB);
        unkept.state(TAB, "https://shop.example/basket", true, false);
        unkept.state(TAB, "https://shop.example/basket", false, false);
        assert!(unkept.marked(TAB));
    }

    #[test]
    fn a_closed_tab_is_forgotten() {
        let mut unkept = Unkept::default();
        unkept.mark(TAB);
        unkept.closed(TAB);
        assert!(!unkept.marked(TAB));
        assert!(unkept.pages.is_empty());
    }
}
