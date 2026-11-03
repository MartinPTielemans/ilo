//! Google's Widevine module, which Netflix, Spotify and Disney+ play their video with in Chromium,
//! on Linux alone (#236). It's Google's proprietary file, which ilo never ships: the
//! protected-media note offers to download it from Google the first time a page needs it, and
//! Settings › Advanced shows it and removes it, or downloads it for a site whose note was closed.
//! Linux loads it at launch, so a page plays with it once the browser restarts, which brings the
//! page back. Elsewhere the core offers none, and neither do the note and Settings. Agents get no
//! path to the yes: the core takes it from the user's click alone.

use std::rc::Rc;

use gpui::{Context, SharedString};

use crate::bc;

/// Where the module is, as the core says (bc_widevine).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Widevine {
    /// Not offered here: the Mac and Windows.
    #[default]
    None,
    /// Not downloaded, or removed.
    Absent,
    Downloading,
    /// The download failed, and can be tried again.
    Failed,
    /// Downloaded, and pages play with it once the browser restarts.
    Restart,
    /// Loaded: pages play with it.
    Ready,
}

impl Widevine {
    pub fn from_core(state: i32) -> Self {
        match u32::try_from(state).unwrap_or(bc::BC_WIDEVINE_NONE) {
            bc::BC_WIDEVINE_ABSENT => Self::Absent,
            bc::BC_WIDEVINE_DOWNLOADING => Self::Downloading,
            bc::BC_WIDEVINE_FAILED => Self::Failed,
            bc::BC_WIDEVINE_RESTART => Self::Restart,
            bc::BC_WIDEVINE_READY => Self::Ready,
            _ => Self::None,
        }
    }

    /// The word check/widevine.sh reads in the log.
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Absent => "absent",
            Self::Downloading => "downloading",
            Self::Failed => "failed",
            Self::Restart => "restart",
            Self::Ready => "ready",
        }
    }

    /// Whether the user's yes downloads it now.
    pub fn downloadable(self) -> bool {
        matches!(self, Self::Absent | Self::Failed)
    }

    /// Whether there's a module to remove.
    pub fn removable(self) -> bool {
        matches!(self, Self::Restart | Self::Ready)
    }
}

/// The module, as Settings' row reads and asks for it.
pub trait Module {
    fn state(&self) -> Widevine;
    /// Downloads it from Google: the user's yes.
    fn download(&self);
    /// Deletes it, and the browser stops asking Google for it.
    fn remove(&self);
}

/// What the row's button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Download,
    Remove,
}

/// Settings › Advanced's row for the module, shown where the core offers it.
pub struct WidevineRow {
    module: Rc<dyn Module>,
    state: Widevine,
}

impl WidevineRow {
    pub fn new(module: Rc<dyn Module>) -> Self {
        let state = module.state();
        Self { module, state }
    }

    /// Reads the core's state again, as it says it changed.
    pub fn check(&mut self, cx: &mut Context<Self>) {
        let state = self.module.state();
        if state != self.state {
            self.state = state;
            cx.notify();
        }
    }

    pub fn state(&self) -> Widevine {
        self.state
    }

    /// Whether Settings shows the row: where the core offers the module.
    pub fn shown(&self) -> bool {
        self.state != Widevine::None
    }

    /// Ours, naming it as Google's.
    pub fn title(&self) -> SharedString {
        "Google's Widevine module".into()
    }

    /// Where it is, under the title.
    pub fn detail(&self) -> SharedString {
        let detail = match self.state {
            Widevine::None | Widevine::Absent => concat!(
                "Netflix and Spotify play their video with it. Proprietary, so ",
                env!("APP_NAME"),
                " downloads it from Google only if you choose to."
            ),
            Widevine::Downloading => "Downloading from Google…",
            Widevine::Failed => "It didn't download. Check your connection and try again.",
            Widevine::Restart => concat!(
                "Downloaded from Google. Sites play with it once ",
                env!("APP_NAME"),
                " restarts."
            ),
            Widevine::Ready => "Downloaded from Google, which keeps it up to date.",
        };
        detail.into()
    }

    /// The row's button, if it has one, and its label.
    pub fn action(&self) -> Option<(Action, &'static str)> {
        match self.state {
            Widevine::Absent => Some((Action::Download, "Download")),
            Widevine::Failed => Some((Action::Download, "Try Again")),
            Widevine::Restart | Widevine::Ready => Some((Action::Remove, "Remove")),
            Widevine::None | Widevine::Downloading => None,
        }
    }

    /// Does `action`, if it's still the row's: a click can come after the state changed.
    pub fn act(&self, action: Action) {
        match action {
            Action::Download if self.state.downloadable() => self.module.download(),
            Action::Remove if self.state.removable() => self.module.remove(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use gpui::{AppContext, Entity, TestAppContext};

    use super::{Action, Module, Widevine, WidevineRow};

    /// The core's module, which a test moves along as the core would.
    #[derive(Default)]
    struct Core {
        state: Cell<Widevine>,
        asked: RefCell<Vec<&'static str>>,
    }

    impl Module for Core {
        fn state(&self) -> Widevine {
            self.state.get()
        }

        fn download(&self) {
            self.asked.borrow_mut().push("download");
        }

        fn remove(&self) {
            self.asked.borrow_mut().push("remove");
        }
    }

    fn row(core: &Rc<Core>, cx: &mut TestAppContext) -> Entity<WidevineRow> {
        let module = core.clone();
        cx.new(|_| WidevineRow::new(module))
    }

    /// The Mac and Windows offer no module, so Settings shows no row.
    #[gpui::test]
    fn there_is_no_row_where_the_core_offers_none(cx: &mut TestAppContext) {
        let core = Rc::new(Core::default());
        let row = row(&core, cx);
        row.read_with(cx, |row, _| {
            assert!(!row.shown());
            assert_eq!(row.action(), None);
            row.act(Action::Download);
        });
        assert!(core.asked.borrow().is_empty());
    }

    /// Not downloaded, the row names it as Google's and downloads it on the user's click; then it
    /// says it's downloading and offers nothing until the core says where it got to.
    #[gpui::test]
    fn the_row_downloads_the_module_and_follows_the_core(cx: &mut TestAppContext) {
        let core = Rc::new(Core {
            state: Cell::new(Widevine::Absent),
            ..Core::default()
        });
        let row = row(&core, cx);
        row.read_with(cx, |row, _| {
            assert!(row.shown());
            assert!(row.title().starts_with("Google's"));
            assert_eq!(row.action(), Some((Action::Download, "Download")));
            row.act(Action::Download);
            // Not a module to remove yet.
            row.act(Action::Remove);
        });
        assert_eq!(*core.asked.borrow(), ["download"]);
        core.state.set(Widevine::Downloading);
        row.update(cx, |row, cx| row.check(cx));
        row.read_with(cx, |row, _| {
            assert_eq!(row.action(), None);
            row.act(Action::Download);
        });
        assert_eq!(core.asked.borrow().len(), 1, "a second download began");
        core.state.set(Widevine::Failed);
        row.update(cx, |row, cx| row.check(cx));
        row.read_with(cx, |row, _| {
            assert_eq!(row.action(), Some((Action::Download, "Try Again")))
        });
        core.state.set(Widevine::Restart);
        row.update(cx, |row, cx| row.check(cx));
        row.read_with(cx, |row, _| {
            assert!(row.detail().contains("restarts"));
            assert_eq!(row.action(), Some((Action::Remove, "Remove")));
        });
    }

    /// The way back: a module downloaded is removed from the row, which then offers it again, and
    /// Settings, which watches the row, draws it so.
    #[gpui::test]
    fn the_row_removes_the_module(cx: &mut TestAppContext) {
        let core = Rc::new(Core {
            state: Cell::new(Widevine::Ready),
            ..Core::default()
        });
        let row = row(&core, cx);
        let redrawn = Rc::new(Cell::new(0));
        let _watch = cx.update(|cx| {
            let redrawn = redrawn.clone();
            cx.observe(&row, move |_, _| redrawn.set(redrawn.get() + 1))
        });
        row.read_with(cx, |row, _| {
            assert_eq!(row.action(), Some((Action::Remove, "Remove")));
            row.act(Action::Download);
            row.act(Action::Remove);
        });
        assert_eq!(*core.asked.borrow(), ["remove"]);
        core.state.set(Widevine::Absent);
        row.update(cx, |row, cx| row.check(cx));
        cx.run_until_parked();
        assert_eq!(redrawn.get(), 1);
        row.read_with(cx, |row, _| {
            assert_eq!(row.action(), Some((Action::Download, "Download")))
        });
    }
}
