//! A site's arrangement (#2347): what Arrange moved on its pages, `Boosts/<site>.arrange.json`
//! beside its CSS. The core makes the file and places what it says; the app keeps it, lists its
//! changes in the Boost card and drops one when the user puts it back. The app reads only what it
//! lists and passes the rest through untouched, as the place is the core's.
//!
//! The file is the user's, so anything can be in it: one over a cap, or not of this shape, is off
//! as a whole and never half listed, and its names only ever reach GPUI's text.

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
}

/// Where a float stays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pin {
    Screen,
    Page,
}

/// What a change did to the thing it moved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Move {
    /// Settled before or after another.
    Settle {
        before: bool,
        other: String,
    },
    /// Traded places with another.
    Trade {
        with: String,
    },
    Float {
        pin: Pin,
    },
}

/// One change, as the card lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// The thing it moved, as the core named it: "The video".
    pub name: String,
    pub moved: Move,
}

impl Change {
    /// Its row's second line: "Before the article".
    pub fn describe(&self) -> String {
        match &self.moved {
            Move::Settle {
                before: true,
                other,
            } => format!("Before {other}"),
            Move::Settle {
                before: false,
                other,
            } => format!("After {other}"),
            Move::Trade { with } => format!("Traded with {with}"),
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

fn change(change: &Value) -> Option<Change> {
    let name = |value: &Value| Some(value.get("name")?.as_str()?.to_owned());
    let moved = match change.get("kind")?.as_str()? {
        "settle" => match (change.get("before"), change.get("after")) {
            (Some(other), None) => Move::Settle {
                before: true,
                other: name(other)?,
            },
            (None, Some(other)) => Move::Settle {
                before: false,
                other: name(other)?,
            },
            _ => return None,
        },
        "trade" => Move::Trade {
            with: name(change.get("with")?)?,
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
        name: name(change)?,
        moved,
    })
}

#[cfg(test)]
mod tests;
