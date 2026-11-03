//! Whether the browser is the system's default, for Settings › General: until it is, its row offers
//! to make it the default, and then says it is. The system keeps the setting, and the way back is
//! its own, in its settings, so the row offers none. A Flatpak can't set it, so there the row says
//! where the desktop does, with no button. It reads the setting again when Settings shows and when
//! the app becomes active, as the user may have changed it there, never on a timer, which would
//! wake an idle browser. Agents get no way to change it.

use std::rc::Rc;

use gpui::{Context, SharedString};

/// The system's default browser, as the row reads and asks for it.
pub trait System {
    /// Whether the system opens http and https links with this app.
    fn is_default(&self) -> bool;
    /// Asks the system to, which asks the user first.
    fn make_default(&self);
}

/// The system the browser runs on, which sets the default its own way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Os {
    Mac,
    Windows,
    Linux,
    /// Linux in a Flatpak, which has neither xdg-settings nor a portal that sets the default.
    Flatpak,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(target_os = "windows") {
            Os::Windows
        } else if std::path::Path::new("/.flatpak-info").exists() {
            Os::Flatpak
        } else {
            Os::Linux
        }
    }
}

pub struct DefaultBrowser {
    system: Rc<dyn System>,
    os: Os,
    default: bool,
}

impl DefaultBrowser {
    pub fn new(system: Rc<dyn System>, os: Os) -> Self {
        let default = system.is_default();
        Self {
            system,
            os,
            default,
        }
    }

    /// Reads the system's setting again.
    pub fn check(&mut self, cx: &mut Context<Self>) {
        let default = self.system.is_default();
        if default != self.default {
            self.default = default;
            cx.notify();
        }
    }

    pub fn is_default(&self) -> bool {
        self.default
    }

    /// What the row says: what it offers, or that the browser is the default.
    pub fn title(&self) -> SharedString {
        match self.default {
            false => concat!("Make ", env!("APP_NAME"), " your default browser").into(),
            true => concat!(env!("APP_NAME"), " is your default browser").into(),
        }
    }

    /// What the system does on Make Default, and the way back, under the title.
    pub fn detail(&self) -> SharedString {
        detail(self.os, self.default).into()
    }

    /// Whether the row offers to make the browser the default: not once it is, nor in a Flatpak,
    /// where the system can't.
    pub fn offers(&self) -> bool {
        offers(self.os, self.default)
    }

    /// Asks the system to make the browser the default, if the row offers to.
    pub fn make_default(&self) {
        if self.offers() {
            self.system.make_default();
        }
    }
}

/// What `os` does on Make Default, and its way back, whether or not the browser is the `default`
/// yet, for Settings' row and the first launch's step.
pub fn detail(os: Os, default: bool) -> &'static str {
    match (os, default) {
        (Os::Mac, false) => {
            "macOS asks you first. You can change it back in System Settings › Desktop & Dock."
        }
        (Os::Mac, true) => "You can change it in System Settings › Desktop & Dock.",
        // Windows shows its chooser rather than asking.
        (Os::Windows, false) => concat!(
            "Windows opens Settings › Default apps, where you choose ",
            env!("APP_NAME"),
            "."
        ),
        (Os::Windows, true) => "You can change it in Settings › Default apps.",
        // xdg-settings sets it without asking.
        (Os::Linux, false) => {
            "Your desktop's setting changes at once. You can change it back in its settings \
             for default apps."
        }
        (Os::Linux, true) => "You can change it in your desktop's settings for default apps.",
        (Os::Flatpak, _) => {
            "Your desktop sets it, in GNOME Settings › Default Apps or KDE System Settings › \
             Default Applications."
        }
    }
}

/// Whether `os` can be asked to make the browser the default: not once it is, nor in a Flatpak.
pub fn offers(os: Os, default: bool) -> bool {
    !default && os != Os::Flatpak
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{AppContext, Entity, TestAppContext};

    use super::{DefaultBrowser, Os, System};

    /// The system's setting, which a test changes as the user would in System Settings.
    #[derive(Default)]
    struct Setting {
        default: Cell<bool>,
        asked: Cell<usize>,
    }

    impl System for Setting {
        fn is_default(&self) -> bool {
            self.default.get()
        }

        fn make_default(&self) {
            self.asked.set(self.asked.get() + 1);
        }
    }

    fn row(setting: &Rc<Setting>, cx: &mut TestAppContext) -> Entity<DefaultBrowser> {
        let system = setting.clone();
        cx.new(|_| DefaultBrowser::new(system, Os::Mac))
    }

    /// Not the default, the row offers to make it so and asks the system, which asks the user. It
    /// says nothing changed until the user has said yes and the app reads the setting again.
    #[gpui::test]
    fn the_row_offers_until_the_user_says_yes(cx: &mut TestAppContext) {
        let setting = Rc::new(Setting::default());
        let row = row(&setting, cx);
        row.read_with(cx, |row, _| {
            assert!(!row.is_default());
            assert!(row.title().starts_with("Make "));
            row.make_default();
        });
        assert_eq!(setting.asked.get(), 1);
        row.update(cx, |row, cx| row.check(cx));
        row.read_with(cx, |row, _| assert!(!row.is_default()));
        setting.default.set(true);
        row.update(cx, |row, cx| row.check(cx));
        row.read_with(cx, |row, _| {
            assert!(row.is_default());
            assert!(row.title().ends_with(" is your default browser"));
            // The default already, it doesn't ask again.
            row.make_default();
        });
        assert_eq!(setting.asked.get(), 1);
    }

    /// The user picked another browser in System Settings: back in the app, the row offers again,
    /// and Settings, which watches it, draws it so.
    #[gpui::test]
    fn the_row_follows_a_change_made_in_system_settings(cx: &mut TestAppContext) {
        let setting = Rc::new(Setting {
            default: Cell::new(true),
            ..Setting::default()
        });
        let row = row(&setting, cx);
        let redrawn = Rc::new(Cell::new(0));
        let _watch = cx.update(|cx| {
            let redrawn = redrawn.clone();
            cx.observe(&row, move |_, _| redrawn.set(redrawn.get() + 1))
        });
        row.update(cx, |row, cx| row.check(cx));
        assert_eq!(redrawn.get(), 0);
        setting.default.set(false);
        row.update(cx, |row, cx| row.check(cx));
        cx.run_until_parked();
        assert_eq!(redrawn.get(), 1);
        row.read_with(cx, |row, _| assert!(!row.is_default()));
    }

    /// Each system says what its own Make Default does and where its way back is, and a Flatpak,
    /// which can't set it, says where the desktop does and offers no button.
    #[gpui::test]
    fn each_system_says_its_own_way(cx: &mut TestAppContext) {
        let setting = Rc::new(Setting::default());
        let details = [Os::Mac, Os::Windows, Os::Linux, Os::Flatpak].map(|os| {
            let system = setting.clone();
            let row = cx.new(|_| DefaultBrowser::new(system, os));
            row.read_with(cx, |row, _| {
                assert_eq!(row.offers(), os != Os::Flatpak, "{os:?}");
                row.make_default();
                (os, row.detail())
            })
        });
        // The Flatpak's row asked nothing.
        assert_eq!(setting.asked.get(), 3);
        let detail = |os| {
            details
                .iter()
                .find(|(each, _)| *each == os)
                .unwrap()
                .1
                .clone()
        };
        assert!(detail(Os::Mac).starts_with("macOS asks you first."));
        assert!(detail(Os::Windows).starts_with("Windows opens Settings › Default apps, where"));
        assert!(detail(Os::Linux).contains("changes at once"));
        assert!(detail(Os::Flatpak).contains("GNOME Settings › Default Apps"));
        assert!(detail(Os::Flatpak).contains("KDE System Settings › Default Applications"));
        for (os, text) in &details {
            let others = details.iter().filter(|(other, _)| other != os);
            assert!(
                others.into_iter().all(|(_, other)| other != text),
                "{os:?} shares its text"
            );
        }
    }
}
