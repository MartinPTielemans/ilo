//! New documents in other services: the notes services New Note opens a new document in, as
//! Arc's Settings › Profiles › New Documents picks one per profile, the one setting for them
//! (#698), and the .new addresses the command bar names once typed. Each profile's pick is kept by
//! the core under its name here.

/// Each service: the name the profile keeps, its title, and the URL that opens a new document,
/// in its own .new shortcut, in the user's account there. Arc's list and order.
pub const SERVICES: &[(&str, &str, &str)] = &[
    ("notion", "Notion", "https://notion.new"),
    ("google-docs", "Google Docs", "https://docs.new"),
    ("word", "Word", "https://word.new"),
    ("confluence", "Confluence", "https://confluence.new"),
];

/// Ours: what New Note opens until the profile picks a service, Arc's own notes being gone.
const DEFAULT: usize = 1;

/// The index in `SERVICES` of the service the profile keeps as `kept`, or the default's for none
/// or one this build doesn't know.
pub fn service(kept: &str) -> usize {
    SERVICES
        .iter()
        .position(|&(name, _, _)| name == kept)
        .unwrap_or(DEFAULT)
}

/// The URL New Note opens for the service the profile keeps as `kept`.
pub fn new_document(kept: &str) -> &'static str {
    SERVICES[service(kept)].2
}

/// The .new addresses the command bar names once one is typed, each with what it makes, as a
/// service's own shortcut for a new document in the user's account there. The notes services'
/// come too. Typed, not listed, as the owner decided (#698): a word like "meet" or "linear" stays
/// a search until its dot.
pub const NEW_ADDRESSES: &[(&str, &str)] = &[
    ("docs.new", "New Google Doc"),
    ("sheets.new", "New Google Sheet"),
    ("slides.new", "New Google Slides"),
    ("forms.new", "New Google Form"),
    ("meet.new", "New Google Meet"),
    ("linear.new", "New Linear Issue"),
    ("figma.new", "New Figma Design"),
    ("figjam.new", "New FigJam Board"),
    ("replit.new", "New Repl"),
    ("notion.new", "New Notion Page"),
    ("word.new", "New Word Document"),
    ("confluence.new", "New Confluence Page"),
];

/// The .new address `typed` names, and what it makes: once what's typed reaches the address's
/// dot, as "docs." does, and not before, so the row never takes ↩ from a search for a word.
pub fn typed_new(typed: &str) -> Option<(&'static str, &'static str)> {
    let typed = typed.trim().to_lowercase();
    let typed = typed.strip_prefix("https://").unwrap_or(&typed);
    let typed = typed.strip_suffix('/').unwrap_or(typed);
    if !typed.contains('.') {
        return None;
    }
    NEW_ADDRESSES
        .iter()
        .copied()
        .find(|(address, _)| address.starts_with(typed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_with_no_service_picked_gets_google_docs() {
        assert_eq!(new_document(""), "https://docs.new");
        assert_eq!(new_document("arc-notes"), "https://docs.new");
        assert_eq!(new_document("notion"), "https://notion.new");
    }

    /// A .new row shows once the address is typed to its dot, in any case, and never for the word
    /// alone, which stays a search, or for an address that isn't one.
    #[test]
    fn a_new_address_is_named_once_typed_to_its_dot() {
        assert_eq!(typed_new("docs.new"), Some(("docs.new", "New Google Doc")));
        assert_eq!(typed_new(" Meet.N"), Some(("meet.new", "New Google Meet")));
        assert_eq!(
            typed_new("linear."),
            Some(("linear.new", "New Linear Issue"))
        );
        assert_eq!(
            typed_new("https://figjam.new/"),
            Some(("figjam.new", "New FigJam Board"))
        );
        for typed in [
            "",
            "docs",
            "meet",
            "linear",
            "docs.google.com",
            "docs.new/x",
            "d.",
            "new",
        ] {
            assert_eq!(typed_new(typed), None, "{typed:?}");
        }
        // Each address is the service's own .new, named by what it makes.
        for (address, name) in NEW_ADDRESSES {
            assert!(
                address.ends_with(".new") && name.starts_with("New "),
                "{address}"
            );
        }
        for (_, _, url) in SERVICES {
            let address = url.strip_prefix("https://").unwrap_or(url);
            assert!(typed_new(address).is_some(), "{address} has no row");
        }
    }
}
