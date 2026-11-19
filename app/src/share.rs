//! What sharing as a link says (#680): before the first share, signed out, and once the link is
//! copied. The host asks and shows them; the words are here so they're the same from every menu.

/// What Share does next, from whether the account is on and whether the user has shared before.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    /// Signed out: a link needs the account, so Share points to Settings › Sync.
    SignIn,
    /// The first share says who can see what before it goes, once.
    Explain,
    /// After that, Share copies at once.
    Share,
}

pub fn step(signed_in: bool, shared_before: bool) -> Step {
    match (signed_in, shared_before) {
        (false, _) => Step::SignIn,
        (true, false) => Step::Explain,
        (true, true) => Step::Share,
    }
}

/// "1 tab" or "N tabs".
pub fn tabs(count: usize) -> String {
    match count {
        1 => "1 tab".into(),
        count => format!("{count} tabs"),
    }
}

/// The signed-out dialog's question, its detail and its action.
pub fn sign_in() -> (String, String, &'static str) {
    (
        "Sign in to share links".into(),
        "A shared link lives on your sync server, so you can unshare it from any of your devices."
            .into(),
        "Open Sync Settings",
    )
}

/// The first share's question, its detail and its action, for `title` holding `count` tabs.
pub fn explain(title: &str, count: usize) -> (String, String, &'static str) {
    let these = match count {
        1 => "this tab’s name and address".to_string(),
        count => format!("these {count} tabs’ names and addresses"),
    };
    (
        format!("Share “{title}” as a link?"),
        format!(
            "Anyone with the link can see {these}, in any browser. ilo locks them on this computer \
             before they reach your sync server, which can’t read them. Unshare any time in \
             Settings › Sync."
        ),
        "Copy Link",
    )
}

/// The toast once the link is on the clipboard: copied, or the same link updated when `title` was
/// shared before, now holding `count` tabs, and how many addresses that aren't the web's were left
/// out on this device.
pub fn copied(title: &str, replaced: bool, count: usize, left_out: usize) -> String {
    let mut words = match replaced {
        false => format!("Link to “{title}” copied"),
        true => format!("Link to “{title}” updated · now {}", tabs(count)),
    };
    if left_out > 0 {
        words.push_str(&format!(" · {} left out", tabs(left_out)));
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_asks_once_and_never_signed_out() {
        assert_eq!(step(false, false), Step::SignIn);
        assert_eq!(step(false, true), Step::SignIn);
        assert_eq!(step(true, false), Step::Explain);
        assert_eq!(step(true, true), Step::Share);
    }

    #[test]
    fn the_first_share_says_who_sees_what() {
        let (question, detail, action) = explain("Lisbon, March", 7);
        assert_eq!(question, "Share “Lisbon, March” as a link?");
        assert!(
            detail.starts_with("Anyone with the link can see these 7 tabs’ names and addresses")
        );
        assert!(detail.ends_with("Unshare any time in Settings › Sync."));
        assert_eq!(action, "Copy Link");
        let (_, detail, _) = explain("Notes", 1);
        assert!(detail.starts_with("Anyone with the link can see this tab’s name and address,"));
    }

    #[test]
    fn the_toast_says_copied_or_updated_and_what_was_left_out() {
        assert_eq!(
            copied("Lisbon, March", false, 7, 0),
            "Link to “Lisbon, March” copied"
        );
        assert_eq!(
            copied("Lisbon, March", true, 9, 0),
            "Link to “Lisbon, March” updated · now 9 tabs"
        );
        assert_eq!(
            copied("Lisbon, March", false, 7, 2),
            "Link to “Lisbon, March” copied · 2 tabs left out"
        );
        assert_eq!(
            copied("Notes", true, 1, 1),
            "Link to “Notes” updated · now 1 tab · 1 tab left out"
        );
    }
}
