//! A site's arrangement (#2347): what Arrange moved on its pages, `Boosts/<site>.arrange.json`
//! beside its CSS. The core makes the file and places what it says; the app keeps it, lists its
//! changes in the Boost card and drops one when the user puts it back. The app reads only what it
//! lists and passes the rest through untouched, as the place is the core's.
//!
//! The file is the user's, so anything can be in it: one over a cap, or not of this shape, is off
//! as a whole and never half listed, and its names only ever reach GPUI's text. The core checks
//! it again and its verdict is the one that counts (Arranged::Refused).

use serde_json::Value;

/// What ends an arrangement's file name, after its site.
pub const SUFFIX: &str = ".arrange.json";
/// The file's caps, as the core's: its bytes, its changes and any one string in it.
pub const MAX_BYTES: usize = 64 * 1024;
pub const MAX_CHANGES: usize = 64;
pub const MAX_STRING: usize = 2 * 1024;

/// What the core makes of a site's arrangement, as of a Boost script: it places it only while
/// the bytes are those the user kept in the browser on this computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arranged {
    On,
    /// Never kept here, as one copied from another computer: off until the user turns it on.
    New,
    /// Changed since the user kept it, outside ilo: off until the user turns it on.
    Changed,
    /// Over the core's caps or off its shape: off, and Turn On can't change that.
    Refused,
}

/// How the core draws Arrange's marks in the page: the Space's accent for the outline, the line
/// and the label, whose words are in `label`; `ink` for the dashed place a carried thing left; in
/// CSS px, the outline's width and its corners. The label names a thing with the app's word for
/// its kind (KINDS).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub accent: u32,
    pub label: u32,
    pub ink: u32,
    pub line: f32,
    pub radius: f32,
}

/// The app's word for each kind of thing on a page, by the token the core gives: its ARIA role,
/// or its tag where it has none. Any other is OTHER.
pub const KINDS: [(&str, &str); 39] = [
    ("article", "article"),
    ("aside", "side box"),
    ("complementary", "side box"),
    ("banner", "header"),
    ("header", "header"),
    ("button", "button"),
    ("contentinfo", "footer"),
    ("footer", "footer"),
    ("dialog", "dialog"),
    ("figure", "figure"),
    ("form", "form"),
    ("heading", "heading"),
    ("h1", "heading"),
    ("h2", "heading"),
    ("h3", "heading"),
    ("h4", "heading"),
    ("h5", "heading"),
    ("h6", "heading"),
    ("img", "picture"),
    ("image", "picture"),
    ("picture", "picture"),
    ("link", "link"),
    ("a", "link"),
    ("list", "list"),
    ("ul", "list"),
    ("ol", "list"),
    ("listitem", "list item"),
    ("li", "list item"),
    ("main", "main content"),
    ("navigation", "menu"),
    ("nav", "menu"),
    ("region", "section"),
    ("section", "section"),
    ("search", "search"),
    ("table", "table"),
    ("video", "video"),
    ("audio", "audio player"),
    ("iframe", "embed"),
    ("paragraph", "paragraph"),
];
pub const OTHER: &str = "part of the page";

/// The app's word for the kind `role`, a token of the core's.
pub fn kind_word(role: &str) -> &'static str {
    let found = KINDS.iter().find(|(token, _)| *token == role);
    found.map_or(OTHER, |(_, word)| word)
}

/// Where a float stays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pin {
    Screen,
    Page,
}

/// A thing on the page, as the core names it: the page's own words, or none, and its kind.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Thing {
    pub name: String,
    pub role: String,
}

impl Thing {
    /// The thing in a row's second line: its name quoted, or "the video" without one.
    fn said(&self) -> String {
        match self.name.is_empty() {
            true => format!("the {}", kind_word(&self.role)),
            false => format!("“{}”", self.name),
        }
    }
}

/// What a change did to the thing it moved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Move {
    /// Settled before or after another.
    Settle {
        before: bool,
        other: Thing,
    },
    /// Traded places with another.
    Trade {
        with: Thing,
    },
    Float {
        pin: Pin,
    },
}

/// One change, as the card lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub thing: Thing,
    pub moved: Move,
}

impl Change {
    /// Its row's name: the thing's own, or its kind's word without one, "Video".
    pub fn title(&self) -> String {
        if !self.thing.name.is_empty() {
            return self.thing.name.clone();
        }
        let word = kind_word(&self.thing.role);
        let mut chars = word.chars();
        let first = chars.next().map(|first| first.to_ascii_uppercase());
        first.into_iter().chain(chars).collect()
    }

    /// Its row's second line: "Before the article".
    pub fn describe(&self) -> String {
        match &self.moved {
            Move::Settle {
                before: true,
                other,
            } => format!("Before {}", other.said()),
            Move::Settle {
                before: false,
                other,
            } => format!("After {}", other.said()),
            Move::Trade { with } => format!("Traded with {}", with.said()),
            Move::Float { pin: Pin::Screen } => "Floats, stays on screen".into(),
            Move::Float { pin: Pin::Page } => "Floats, scrolls with the page".into(),
        }
    }
}

/// The changes in `bytes`, a site's file, or None for one over a cap or not of its shape.
pub fn read(bytes: &str) -> Option<Vec<Change>> {
    let file = parse(bytes)?;
    let changes = file.get("changes")?.as_array()?;
    changes.iter().map(change).collect()
}

/// `bytes` without its change at `index`, as ↺ and × put it back, everything else as it was. None
/// for a file `read` refuses or an index it hasn't.
pub fn without(bytes: &str, index: usize) -> Option<String> {
    read(bytes)?;
    let mut file = parse(bytes)?;
    let changes = file.get_mut("changes")?.as_array_mut()?;
    (index < changes.len()).then(|| changes.remove(index))?;
    let mut text = serde_json::to_string_pretty(&file).ok()?;
    text.push('\n');
    Some(text)
}

/// The file as JSON, within its caps, at version 1.
fn parse(bytes: &str) -> Option<Value> {
    if bytes.len() > MAX_BYTES {
        return None;
    }
    let file: Value = serde_json::from_str(bytes).ok()?;
    let fits = within(&file) && file.get("version")?.as_u64()? == 1;
    let changes = file.get("changes")?.as_array()?;
    (fits && changes.len() <= MAX_CHANGES).then_some(file)
}

/// Whether every string in `value`, its keys too, is within MAX_STRING.
fn within(value: &Value) -> bool {
    match value {
        Value::String(text) => text.len() <= MAX_STRING,
        Value::Array(each) => each.iter().all(within),
        Value::Object(fields) => {
            (fields.iter()).all(|(key, value)| key.len() <= MAX_STRING && within(value))
        }
        _ => true,
    }
}

/// A thing in the file: its name, and its role where the file has one.
fn thing(value: &Value) -> Option<Thing> {
    let role = match value.get("role") {
        None => String::new(),
        Some(role) => role.as_str()?.to_owned(),
    };
    Some(Thing {
        name: value.get("name")?.as_str()?.to_owned(),
        role,
    })
}

fn change(change: &Value) -> Option<Change> {
    let moved = match change.get("kind")?.as_str()? {
        "settle" => match (change.get("before"), change.get("after")) {
            (Some(other), None) => Move::Settle {
                before: true,
                other: thing(other)?,
            },
            (None, Some(other)) => Move::Settle {
                before: false,
                other: thing(other)?,
            },
            _ => return None,
        },
        "trade" => Move::Trade {
            with: thing(change.get("with")?)?,
        },
        "float" => Move::Float {
            pin: match change.get("pin")?.as_str()? {
                "screen" => Pin::Screen,
                "page" => Pin::Page,
                _ => return None,
            },
        },
        _ => return None,
    };
    Some(Change {
        thing: thing(change)?,
        moved,
    })
}

/// What the user asks of the mode on the page, as the core takes it (`bc_tab_arrange_do`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    /// ↑: the box around the one pointed at or selected.
    Outer,
    /// ↓: the box inside it.
    Inner,
    /// The core's last change undone: a settle, a trade, a float or a pin.
    Undo,
    /// The selected thing hidden, by Zap's rule.
    Hide,
    /// The selected thing back where the page had it.
    PutBack,
    /// A floating thing pinned to the screen, or to the page.
    StaysOnScreen,
    ScrollsWithPage,
    /// Every change on the page undone.
    Reset,
}

/// Where the user is in the mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Step {
    #[default]
    Pointing,
    Carrying,
    /// Carried and rested, the page showing what the drop would do.
    Preview,
    Selected,
}

/// What a drop does, or did, to the thing carried or selected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Outcome {
    #[default]
    None,
    Settles,
    Trades,
    Floats,
    Refused,
}

/// Why the core refuses a drop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Refused {
    #[default]
    Fits,
    DoesntFit,
    NotShown,
    Covered,
}

/// A thing on the page as the core names it: its kind's token and the page's own words, either
/// empty. An empty kind is nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Element {
    pub kind: String,
    pub name: String,
}

/// The mode on a tab's page, as the core tells it (`tab_arrange_state`): once a step changes and
/// once a scroll starts and rests, never each frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub step: Step,
    pub element: Element,
    pub change: Outcome,
    /// What a settle goes between, or a trade's other thing in `before`.
    pub before: Element,
    pub after: Element,
    pub refused: Refused,
    /// While a floating thing is selected, whether it stays on screen rather than scrolling.
    pub on_screen: bool,
    pub can_undo: bool,
    pub can_reset: bool,
    /// The selected thing's box in points from the page view's top-left, as x, y, width and
    /// height: none while nothing is selected and while the page scrolls.
    pub selection: Option<[f32; 4]>,
}

#[cfg(test)]
mod tests;
