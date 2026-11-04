//! The logins a profile's password manager keeps, as the Site Controls' Passwords row and Settings'
//! Passwords tab show them (docs/ux-spec.md › Passwords and autofill). The core keeps the
//! passwords: a login here is its site and username, and the key the core takes back to remove it.
//! The user's alone, as everything about passwords is: agents get sessions, never secrets.

use gpui::SharedString;

use crate::shell::MenuEntry;

/// A saved login, or a site set to Never, where the offer to save a password doesn't come.
#[derive(Clone, PartialEq, Debug)]
pub struct Login {
    /// As the user should see it, such as "accounts.example.com".
    pub site: SharedString,
    /// Empty for a login without one, and for a site set to Never.
    pub username: SharedString,
    pub never: bool,
    /// The core's name for it, with the username.
    pub key: SharedString,
}

/// What the password manager has for a tab's page, as the key after the host in the URL field
/// shows it (ours, for Chrome's key icon). A page it has nothing for has no key.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    /// The site's saved logins, filled in or just saved, which the Site Controls list.
    Saved,
    /// An offer to save or update the page's login, which the key brings back once closed.
    Offer,
}

/// What the Passwords row's menu does with the item chosen.
#[derive(Clone, PartialEq, Debug)]
pub enum Action {
    /// Removes the login, once the user confirms.
    Remove(Login),
    /// Takes the site off Never, so the offer comes there again.
    Allow(Login),
    /// Shows every login, in Settings.
    ShowAll,
}

/// The Passwords row's words under its name, and whether its circle is filled, as a permission's
/// is while allowed: the username of a site's one login, how many it has, or that it's set to
/// Never. None when the site has none, and then there's no row.
pub fn state(logins: &[Login]) -> Option<(SharedString, bool)> {
    let saved: Vec<&Login> = logins.iter().filter(|login| !login.never).collect();
    match saved.as_slice() {
        [] if logins.is_empty() => None,
        [] => Some(("Never saved".into(), false)),
        [login] if !login.username.is_empty() => Some((login.username.clone(), true)),
        [_] => Some(("1 saved".into(), true)),
        saved => Some((format!("{} saved", saved.len()).into(), true)),
    }
}

/// The Passwords row's menu: Remove for each login, Never's way back, and every login in Settings.
pub fn menu(logins: &[Login]) -> Vec<(MenuEntry, Option<Action>)> {
    let item = |entry: MenuEntry, action: Action| (entry, Some(action));
    let remove = |login: &Login| MenuEntry::named(format!("Remove {}…", named(login)), true);
    let mut items: Vec<_> = (logins.iter().filter(|login| !login.never))
        .map(|login| item(remove(login), Action::Remove(login.clone())))
        .collect();
    items.extend((logins.iter().filter(|login| login.never)).map(|login| {
        item(
            MenuEntry::new("Offer to Save Passwords", true),
            Action::Allow(login.clone()),
        )
    }));
    items.push((MenuEntry::line(), None));
    items.push(item(
        MenuEntry::new("All Passwords…", true),
        Action::ShowAll,
    ));
    items
}

/// What removing `login` asks first, and the line under it. Nothing brings a removed password back,
/// so it asks, as removing an extension does.
pub fn question(login: &Login) -> (String, String) {
    let message = match login.username.is_empty() {
        true => format!("Remove the password saved for {}?", login.site),
        false => format!(
            "Remove the password for “{}” on {}?",
            login.username, login.site
        ),
    };
    (
        message,
        format!("{} won't fill it in again.", env!("APP_NAME")),
    )
}

/// A login as a menu item names it.
fn named(login: &Login) -> String {
    match login.username.is_empty() {
        true => "Password Without Username".into(),
        false => format!("“{}”", login.username),
    }
}

/// The lines Settings lists a login with: its site, and its username under it, which a login
/// without one says it lacks. A site set to Never has the site alone.
pub fn lines(login: &Login) -> (SharedString, Option<SharedString>) {
    match (login.never, login.username.is_empty()) {
        (true, _) => (login.site.clone(), None),
        (false, true) => (login.site.clone(), Some("No username".into())),
        (false, false) => (login.site.clone(), Some(login.username.clone())),
    }
}

/// The logins for the log: the usernames saved and the sites set to Never, never a password, which
/// the app doesn't have.
pub fn describe(logins: &[Login]) -> String {
    let named: Vec<String> = (logins.iter())
        .map(|login| match login.never {
            true => format!("never {}", login.site),
            false if login.username.is_empty() => format!("{} (no username)", login.site),
            false => format!("{} {}", login.site, login.username),
        })
        .collect();
    match named.is_empty() {
        true => "none".into(),
        false => named.join(", "),
    }
}

#[cfg(test)]
mod tests;
