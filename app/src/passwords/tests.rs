//! What the Passwords row and Settings say of a site's logins, and what each item does.

use super::*;

fn saved(site: &str, username: &str) -> Login {
    Login {
        site: site.to_string().into(),
        username: username.to_string().into(),
        never: false,
        key: format!("https://{site}/").into(),
    }
}

fn never(site: &str) -> Login {
    Login {
        never: true,
        ..saved(site, "")
    }
}

fn titles(items: &[(MenuEntry, Option<Action>)]) -> Vec<String> {
    items
        .iter()
        .map(|(item, _)| item.title.to_string())
        .collect()
}

/// No row for a site with none; the username for one; a count for more; Never on its own.
#[test]
fn the_row_says_what_the_site_keeps() {
    assert_eq!(state(&[]), None);
    let one = [saved("example.com", "me@example.com")];
    assert_eq!(state(&one), Some(("me@example.com".into(), true)));
    assert_eq!(
        state(&[saved("example.com", "")]),
        Some(("1 saved".into(), true))
    );
    let two = [
        saved("example.com", "me"),
        saved("login.example.com", "you"),
    ];
    assert_eq!(state(&two), Some(("2 saved".into(), true)));
    assert_eq!(
        state(&[never("example.com")]),
        Some(("Never saved".into(), false))
    );
    // A login saved on another of the domain's sites says more than Never on this one.
    let both = [saved("login.example.com", "me"), never("example.com")];
    assert_eq!(state(&both), Some(("me".into(), true)));
}

/// Each login is removed on its own, Never has its way back, and Settings lists them all.
#[test]
fn the_menu_removes_each_login_and_takes_back_never() {
    let logins = [
        saved("example.com", "me"),
        saved("example.com", ""),
        never("example.com"),
    ];
    let items = menu(&logins);
    let expected = [
        "Remove “me”…",
        "Remove Password Without Username…",
        "Offer to Save Passwords",
        "",
        "All Passwords…",
    ];
    assert_eq!(titles(&items), expected);
    assert_eq!(items[0].1, Some(Action::Remove(logins[0].clone())));
    assert_eq!(items[2].1, Some(Action::Allow(logins[2].clone())));
    assert_eq!(items[3].1, None);
    assert_eq!(items[4].1, Some(Action::ShowAll));
}

/// Removing asks first, naming the login, as nothing brings it back.
#[test]
fn removing_asks_first() {
    let (message, detail) = question(&saved("example.com", "me"));
    assert_eq!(message, "Remove the password for “me” on example.com?");
    assert!(detail.ends_with("won't fill it in again."));
    let (message, _) = question(&saved("example.com", ""));
    assert_eq!(message, "Remove the password saved for example.com?");
}

#[test]
fn settings_lists_the_site_over_the_username() {
    let named = lines(&saved("example.com", "me"));
    assert_eq!(named, ("example.com".into(), Some("me".into())));
    let unnamed = lines(&saved("example.com", ""));
    assert_eq!(unnamed, ("example.com".into(), Some("No username".into())));
    assert_eq!(lines(&never("example.com")), ("example.com".into(), None));
}

#[test]
fn the_log_names_logins_without_passwords() {
    assert_eq!(describe(&[]), "none");
    let logins = [
        saved("example.com", "me"),
        saved("a.com", ""),
        never("b.com"),
    ];
    assert_eq!(
        describe(&logins),
        "example.com me, a.com (no username), never b.com"
    );
}
