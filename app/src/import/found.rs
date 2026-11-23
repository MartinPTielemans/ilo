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
    Split(Split),
}

/// A split view: its tabs, two or three, left to right, and the name, icon and orientation the
/// user gave it.
#[derive(Clone, Debug, PartialEq)]
pub struct Split {
    pub tabs: Vec<Tab>,
    /// Empty for none, as for an icon, which is an emoji.
    pub name: String,
    pub icon: String,
    /// Its pages top to bottom rather than side by side.
    pub stacked: bool,
}

/// The most tabs a split view holds, as the core keeps it (bc_tab_split_with).
pub const SPLIT_TABS: usize = 3;

/// Where a found Space came from, which decides the switch that brings its pinned tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpaceFrom {
    /// The browser's own sidebar (sidebar.rs), under Spaces.
    Sidebar,
    /// A profile's bookmarks, under Bookmarks, as one Space named after the browser.
    Bookmarks,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Space {
    pub from: SpaceFrom,
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
    /// Where Arc opens a link no rule matches, if not in Little Arc, ours' default too.
    pub link_otherwise: Option<Destination>,
    /// The profiles a sidebar names that aren't folders of the browser's, such as Zen's
    /// containers, each by the key its `FromProfile::Custom` gives and its name. Only the name is
    /// read, so each comes as the profile here with that name, or a new one with no sign-ins.
    pub profiles: Vec<(String, String)>,
    /// What the browser has that has no place here, said before anything comes.
    pub left_out: Vec<LeftOut>,
}

/// Things of one kind a browser has that won't be imported, as they have no place here, counted.
#[derive(Clone, Debug, PartialEq)]
pub struct LeftOut {
    pub item: Item,
    pub count: usize,
}

/// The words for a thing that won't be imported, as the import screen lists it: one, with its
/// article, "a live folder", and many, to follow a count, "live folders". Each is shared by every
/// source that has the thing, so the same thing reads the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub one: &'static str,
    pub many: &'static str,
}

pub const LIVE_FOLDER: Item = Item {
    one: "a live folder",
    many: "live folders",
};
/// A split view of more tabs than SPLIT_TABS.
pub const FOUR_TAB_SPLIT: Item = Item {
    one: "a four-tab split",
    many: "four-tab splits",
};
pub const SEPARATE_CONTAINER: Item = Item {
    one: "a tab in a separate container",
    many: "tabs in separate containers",
};
pub const SPACE_ICON: Item = Item {
    one: "a Space icon",
    many: "Space icons",
};
/// Zen's.
pub const ESSENTIALS_FOLDER: Item = Item {
    one: "a folder in Essentials",
    many: "folders in Essentials",
};
/// Ardia's.
pub const TOP_APPS_FOLDER: Item = Item {
    one: "a folder in Top Apps",
    many: "folders in Top Apps",
};
pub const TAB_GROUP: Item = Item {
    one: "a tab group",
    many: "tab groups",
};
/// Ardia's, where its own words call it stacked and it can't come as a split here.
pub const STACKED_SPLIT: Item = Item {
    one: "a stacked split",
    many: "stacked splits",
};
/// A tab at an address that can't open here and isn't one of the pages `quiet` names.
pub const NOT_WEB_PAGE: Item = Item {
    one: "a tab that isn't a web page",
    many: "tabs that aren't web pages",
};

impl Found {
    /// Counts `count` more of `item` among what won't be imported.
    pub fn leave_out(&mut self, item: Item, count: usize) {
        if count == 0 {
            return;
        }
        match self.left_out.iter_mut().find(|each| each.item == item) {
            Some(each) => each.count += count,
            None => self.left_out.push(LeftOut { item, count }),
        }
    }
}

/// A link rule: links whose URL meets `condition` with `text` open at `to`.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkRule {
    pub condition: Condition,
    pub text: String,
    pub to: Destination,
}

/// Where a link rule, or everything else, opens a link.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Destination {
    /// The Space at this index among the found Spaces.
    Space(usize),
    /// The Space the window in front shows.
    Front,
    Little,
}

/// Whether a tab at `url`, which doesn't open here, is one no one misses, so it's dropped without
/// being counted among the tabs that aren't web pages: a tab with no address, a blank page, or a
/// browser's own new tab, home or private window page, whatever the browser calls its scheme.
pub fn quiet(url: &str) -> bool {
    let url = url.trim().to_ascii_lowercase();
    let page = match url.split_once("://") {
        Some((_, page)) => page.trim_end_matches('/'),
        None => url.as_str(),
    };
    matches!(
        page,
        "" | "about:blank"
            | "about:newtab"
            | "about:home"
            | "about:privatebrowsing"
            | "newtab"
            | "new-tab-page"
    )
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
