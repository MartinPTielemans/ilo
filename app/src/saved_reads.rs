//! Saved reads (#3819): a named, read-only definition of one thing to read on one site, which an
//! agent proposes and the user saves, and which agents connected to ilo then use by name without
//! a tab. This is the part with no UI and no agent path (#3821): what a definition is and how it's
//! checked (definition), the address a use reads (address), the rows read out of an answer's body
//! and their shape (answer, table), and where a profile keeps its saved reads (store).
//!
//! Nothing here sends anything or runs anything a site sent: the core reads the address (#3820),
//! and a body is parsed as data, never run. What comes out is text, clipped, never markup.
//! It rewrites what the prototype in #3814 proved; nothing depends on spikes/.

pub mod address;
pub mod answer;
pub mod definition;
pub mod store;
pub mod table;

#[cfg(test)]
mod tests;

pub use address::{Refused, address};
pub use answer::{Answered, Broken, Outcome, Row, outcome, read};
pub use definition::{Column, Definition, Kind, Param, Type, Wrong};
pub use store::{Refusal, Saved, State, Store};

/// The most of a value an answer keeps, in characters; what's past it is cut with "…".
pub const VALUE_KEPT: usize = 500;

/// The most rows an answer keeps.
pub const ROWS_KEPT: usize = 500;

/// A site's text as one line of plain text: what isn't drawn (format and direction characters)
/// goes, line breaks, tabs, controls and blank glyphs become a space, runs of spaces one, and it's
/// cut at `kept` characters with "…". Nothing in it is read as markup.
pub fn text(value: &str, kept: usize) -> String {
    let mut out = String::with_capacity(value.len().min(kept * 4));
    let mut space = false;
    for c in value.chars() {
        if crate::agents::request::invisible(c) {
            continue;
        }
        if c.is_whitespace() || c.is_control() || crate::agents::request::blank(c) {
            space = !out.is_empty();
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
    }
    crate::agents::page::clip(&out, kept)
}
