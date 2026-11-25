//! Settings › Privacy's Always use secure connections (#4279), for a window's profile. The core
//! keeps it, and nothing here does: Privacy reads it as it draws and again after a click.

use gpui::App;

use super::Shell;

impl Shell {
    /// The window's profile's, as the core has it; None where the window shows no Space or the
    /// core hasn't loaded its profile.
    pub fn secure_connections(&self) -> Option<bool> {
        let profile = self.shown_space()?.profile;
        self.browser.secure_connections(profile)
    }

    /// The user's click on its switch: asks the core once to turn it `on` or off, and returns what
    /// the core has then, which a refused set leaves as it was.
    pub fn set_secure_connections(&self, on: bool, cx: &mut App) -> Option<bool> {
        let profile = self.shown_space()?.profile;
        self.browser.set_secure_connections(profile, on, cx);
        self.browser.secure_connections(profile)
    }
}
