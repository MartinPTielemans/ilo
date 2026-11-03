//! What the readers find in another browser's files, in the shape it comes in here: Spaces with
//! their pinned and Today tabs and folders, favorites, history and addresses, each in the profile
//! it was kept in, and Arc's link rules.

use super::extensions::Extension;
use crate::link_rules::Condition;
use crate::theme::Theme;

/// The profile something was kept in, by its folder in the browser's user data.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FromProfile {
    /// The browser's first profile, which becomes the one this browser started with.
    Default,
    /// Another, by its folder, such as "Profile 1".
    Custom(String),
}

impl FromProfile {
    /// A profile by its folder under the user data, Chromium's first named "Default".
    pub fn of(dir: &str) -> FromProfile {
        match dir {
            // Opera keeps its one profile in the user data folder itself.
            "Default" | "" => FromProfile::Default,
            dir => FromProfile::Custom(dir.into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tab {
    pub url: String,
    /// The name the user gave it, or empty for the page's title.
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Tab(Tab),
    Folder { name: String, children: Vec<Node> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Space {
    pub name: String,
    pub profile: FromProfile,
    /// As a Space keeps it here: an emoji, `glyph:` and one of ours, or empty.
    pub icon: String,
    pub theme: Option<Theme>,
    pub pinned: Vec<Node>,
    /// The tabs the browser had open, as the Space's Today tabs, in its order.
    pub today: Vec<Tab>,
}

/// A page from the browser's history.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub url: String,
    pub title: String,
    /// When it was last visited, in microseconds since 1970.
    pub last_visit: i64,
    pub visits: u32,
    /// How many of the visits were typed into the address bar, which ranks it among suggestions.
    pub typed: u32,
}

/// A saved address, as Chromium's Web Data keeps one: each of its fields by Chromium's FieldType
/// number, which the core reads as Chromium's own table does.
#[derive(Clone, Debug, PartialEq)]
pub struct Address {
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// Chromium's FieldType, such as 33 for the city.
    pub kind: i32,
    pub value: String,
    /// Chromium's VerificationStatus: how the value came to be, such as typed by the user or
    /// parsed from the rest.
    pub status: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    pub spaces: Vec<Space>,
    /// Each profile's favorites, in the browser's order.
    pub favorites: Vec<(FromProfile, Vec<Tab>)>,
    pub history: Vec<(FromProfile, Vec<Page>)>,
    /// The Web Store extensions of every profile, once each, pinned if any profile pinned it.
    pub extensions: Vec<Extension>,
    pub addresses: Vec<(FromProfile, Vec<Address>)>,
    /// Arc's Air Traffic Control rules, in its order.
    pub link_rules: Vec<LinkRule>,
}

/// A link rule: links whose URL meets `condition` with `text` open in the Space at `space` among
/// the found Spaces.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkRule {
    pub condition: Condition,
    pub text: String,
    pub space: usize,
}

/// Whether a page at `url` can open here: another browser's own pages, such as `arc://` or
/// `chrome://`, can't.
pub fn opens_here(url: &str) -> bool {
    url.split_once(':').is_some_and(|(scheme, _)| {
        matches!(
            scheme.to_ascii_lowercase().as_str(),
            "http" | "https" | "file"
        )
    })
}
