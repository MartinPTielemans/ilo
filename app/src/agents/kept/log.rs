//! A kept thread's log as it is drawn closed (spec-thread-stage.md, "Kept across a quit"), in the
//! form its card's folder holds it (cards.rs, `log`): one record a line, each a kind from the list
//! below and its fields, tab apart. Only the app writes one, from what it draws, so a record's kind
//! is the app's word for it: nothing in a message, a title or a page's text can make a line read
//! back as the user's, as the product's own, or as anything with a button.
//!
//! A text keeps its line breaks through the one escape the writer makes, `\n` for a break and
//! `\\` for a backslash; every other control character is dropped as it's written. The reader
//! undoes that escape and nothing else, and a log that doesn't parse whole, is over `MOST`, or
//! holds a control character but the record's own breaks and tabs reads as none, silently.
//!
//! Never formatted into a log line: `Debug` says how many records there are and nothing of them.

/// The most a stored log may be, in bytes. A log over it is never written, and a file over it
/// reads as no log: the thread shows what a kept thread shows without one.
pub const MOST: usize = 256 * 1024;

/// A line of the log as it's drawn closed, by its kind.
#[derive(Clone, PartialEq)]
pub enum Record {
    /// The user's message.
    You(String),
    /// A chip under the user's message: the title of a page they added, as drawn. A file's or a
    /// folder's never is one (#3656).
    Chip(String),
    /// The assistant's words as a message of its own, after a stretch of work.
    Said(String),
    /// The assistant's words within a stretch of work, between its pages.
    Aside(String),
    /// A turn's answer, and the time it finished as the log draws it.
    Answer { text: String, at: String },
    /// A stretch of work's head, closed: how long it worked, in seconds, if it's over.
    Fold { took: Option<u64> },
    /// A page under the stretch above: its title, or its site where it had none or was withheld.
    Page(String),
    /// A step's own line: its glyph, its words, what came of it at the right, and whether it
    /// failed.
    Step {
        glyph: &'static str,
        words: String,
        came: String,
        failed: bool,
    },
    /// The product's own line, its words as drawn and never its buttons.
    Ilo(String),
}

/// How many there are, and nothing of what they say.
impl std::fmt::Debug for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.kind())
    }
}

/// The glyphs a stored step may name, so a file someone changed can't name an asset of its own.
/// A step whose glyph isn't here is stored with the first.
pub const GLYPHS: &[&str] = &[
    "icons/globe.svg",
    "icons/text-align-start.svg",
    "icons/mouse-pointer-2.svg",
    "icons/type.svg",
    "icons/chevrons-up-down.svg",
    "icons/keyboard.svg",
    "icons/arrow-down.svg",
    "icons/eye.svg",
    "icons/arrow-left-right.svg",
    "icons/clock.svg",
    "icons/text-search.svg",
    "icons/app-window.svg",
    "icons/pin.svg",
    "icons/circle-question-mark.svg",
    "icons/file.svg",
    "icons/ellipsis.svg",
    "icons/search.svg",
    "icons/x.svg",
];

/// `glyph` as one of `GLYPHS`.
pub fn known_glyph(glyph: &str) -> &'static str {
    (GLYPHS.iter())
        .find(|known| **known == glyph)
        .copied()
        .unwrap_or(GLYPHS[0])
}

impl Record {
    fn kind(&self) -> &'static str {
        match self {
            Record::You(_) => "you",
            Record::Chip(_) => "chip",
            Record::Said(_) => "said",
            Record::Aside(_) => "aside",
            Record::Answer { .. } => "answer",
            Record::Fold { .. } => "fold",
            Record::Page(_) => "page",
            Record::Step { .. } => "step",
            Record::Ilo(_) => "ilo",
        }
    }

    fn fields(&self) -> Vec<String> {
        let flag = |on: bool| if on { "1" } else { "0" }.to_string();
        match self {
            Record::You(text)
            | Record::Chip(text)
            | Record::Said(text)
            | Record::Aside(text)
            | Record::Page(text)
            | Record::Ilo(text) => vec![escape(text)],
            Record::Answer { text, at } => vec![escape(text), escape(at)],
            Record::Fold { took } => vec![took.map(|took| took.to_string()).unwrap_or_default()],
            Record::Step {
                glyph,
                words,
                came,
                failed,
            } => vec![
                known_glyph(glyph).to_string(),
                escape(words),
                escape(came),
                flag(*failed),
            ],
        }
    }

    fn parse(line: &str) -> Option<Record> {
        let mut fields = line.split('\t');
        let kind = fields.next()?;
        let fields: Vec<&str> = fields.collect();
        let text = |at: usize| unescape(fields.get(at)?);
        let record = match (kind, fields.len()) {
            ("you", 1) => Record::You(text(0)?),
            ("chip", 1) => Record::Chip(text(0)?),
            ("said", 1) => Record::Said(text(0)?),
            ("aside", 1) => Record::Aside(text(0)?),
            ("page", 1) => Record::Page(text(0)?),
            ("ilo", 1) => Record::Ilo(text(0)?),
            ("answer", 2) => Record::Answer {
                text: text(0)?,
                at: text(1)?,
            },
            ("fold", 1) => Record::Fold {
                took: match fields[0] {
                    "" => None,
                    took => Some(took.parse().ok()?),
                },
            },
            ("step", 4) => Record::Step {
                glyph: GLYPHS.iter().find(|known| **known == fields[0]).copied()?,
                words: text(1)?,
                came: text(2)?,
                failed: match fields[3] {
                    "0" => false,
                    "1" => true,
                    _ => return None,
                },
            },
            _ => return None,
        };
        Some(record)
    }
}

/// `text` as a field: a break as `\n`, a backslash as `\\`, any other control character dropped,
/// a tab with them.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for char in text.chars() {
        match char {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            char if char.is_control() => {}
            char => out.push(char),
        }
    }
    out
}

/// A field as its text: only the two escapes `escape` makes. Anything else after a backslash, or
/// a control character, isn't a field the app wrote.
fn unescape(field: &str) -> Option<String> {
    let mut out = String::with_capacity(field.len());
    let mut chars = field.chars();
    while let Some(char) = chars.next() {
        match char {
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                '\\' => out.push('\\'),
                _ => return None,
            },
            char if char.is_control() => return None,
            char => out.push(char),
        }
    }
    Some(out)
}

/// `records` as the file holds them, or None if that's over `MOST`, when none is written.
pub fn write(records: &[Record]) -> Option<String> {
    let mut out = String::new();
    for record in records {
        out.push_str(record.kind());
        for field in record.fields() {
            out.push('\t');
            out.push_str(&field);
        }
        out.push('\n');
        if out.len() > MOST {
            return None;
        }
    }
    Some(out)
}

/// What a file holds, if every line of it is a record the app writes; else none.
pub fn read(text: &str) -> Option<Vec<Record>> {
    if text.len() > MOST {
        return None;
    }
    let body = text.strip_suffix('\n').unwrap_or(text);
    if body.is_empty() {
        return Some(Vec::new());
    }
    body.split('\n').map(Record::parse).collect()
}

#[cfg(test)]
mod tests;
