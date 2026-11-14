//! Point (#2352): an agent's `point` tool asks the user to show it something on a page, and the user
//! points at elements in the tab in front, writes a note on each and sends them, or doesn't. The
//! core does the pointing (bc_tab_point): the hit test, and gathering what's clicked, through the
//! tab's own DevTools client, with nothing added to the page. This is what the app keeps of it,
//! and the words: the label each mark has, the rows the note's panel shows, and the agent's answer,
//! which is those rows and nothing else.

use std::sync::Arc;

use gpui::{Bounds, Pixels, point, px, size};

use crate::agents::request::Sent;

/// The answer to every `point` call the user didn't send, whatever happened: Not Now, esc, a
/// timeout, or a page Point couldn't point into. One answer, so the agent can't tell which, as it
/// learns nothing of a tab that isn't its own.
pub const NOTHING: &str = "The user didn't point at anything.";

/// The app's word for each kind of thing on a page, by the token the core gives: its ARIA role, or
/// its tag where it has none. Arrange's words (#2347), as its marks and Point's are one look, and
/// a form control's, which Point can be pointed at. Any other is OTHER.
const KINDS: [(&str, &str); 47] = [
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
    ("p", "paragraph"),
    ("input", "field"),
    ("textarea", "field"),
    ("textbox", "field"),
    ("searchbox", "field"),
    ("select", "menu field"),
    ("combobox", "menu field"),
    ("checkbox", "checkbox"),
];
const OTHER: &str = "part of the page";

/// The app's word for the kind `token`, a token of the core's.
pub fn kind_word(token: &str) -> &'static str {
    let found = KINDS.iter().find(|(kind, _)| *kind == token);
    found.map_or(OTHER, |(_, word)| word)
}

/// A mark's label, as Arrange's reads: the kind's word, then the element's name if it has one,
/// "Section · Trending". It starts its line, so the word does too, with a capital.
pub fn label(kind: &str, name: &str) -> String {
    let word = kind_word(kind);
    let mut chars = word.chars();
    let word: String = (chars.next().into_iter())
        .flat_map(char::to_uppercase)
        .chain(chars)
        .collect();
    match name.trim() {
        "" => word,
        name => format!("{word} · {name}"),
    }
}

/// An element the user picked, as the core gathered it (bc_point_element): all that goes to the
/// agent with the user's note if they send it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Element {
    pub kind: String,
    pub name: String,
    /// The page's origin and path, never its query or fragment, and its title.
    pub address: String,
    pub title: String,
    pub tag: String,
    /// Its text, without any form control's value.
    pub text: String,
    pub selector: String,
    pub color: String,
    pub background: String,
    pub font: String,
    pub font_size: String,
    pub padding: String,
    /// Its border, in points from the top-left of the page's view.
    pub frame: Bounds<Pixels>,
    /// Whether a form control in it held a value, left out of the text and hatched in the picture.
    pub withheld: bool,
    /// Its picture as a PNG, `width` by `height` pixels, if the page gave one.
    pub png: Option<Arc<[u8]>>,
    pub width: u32,
    pub height: u32,
}

/// One line of what a note sends: its name, and what goes, or why nothing does.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: &'static str,
    pub value: String,
    /// The value says what isn't sent, rather than being sent, and shows quieter.
    pub withheld: bool,
}

/// What the note's panel says the agent gets, when it's opened, and the closed line's words.
const VALUE_WITHHELD: &str = "Not sent. A form field's value never goes, a password, an email or \
                              a card number alike.";

impl Element {
    pub fn label(&self) -> String {
        label(&self.kind, &self.name)
    }

    fn word(&self) -> &'static str {
        kind_word(&self.kind)
    }

    /// Every line of what goes to the agent, in the order the panel shows them and the answer
    /// sends them: the panel shows exactly this, so the user reads the payload itself.
    pub fn rows(&self) -> Vec<Row> {
        let row = |name, value: String| Row {
            name,
            value,
            withheld: false,
        };
        let frame = self.frame;
        let mut rows = vec![
            row("Page", self.address.clone()),
            row("Title", self.title.clone()),
            row("Element", format!("{}, <{}>", self.word(), self.tag)),
            row("Name", self.name.clone()),
            row("Text", self.text.clone()),
            row("Finds it", self.selector.clone()),
            row(
                "Box",
                format!(
                    "{} × {} at {}, {}",
                    f32::from(frame.size.width).round(),
                    f32::from(frame.size.height).round(),
                    f32::from(frame.origin.x).round(),
                    f32::from(frame.origin.y).round(),
                ),
            ),
            row(
                "Styles",
                [
                    ("color", &self.color),
                    ("background", &self.background),
                    ("font", &self.font),
                    ("size", &self.font_size),
                    ("padding", &self.padding),
                ]
                .iter()
                .filter(|(_, value)| !value.is_empty())
                .map(|(name, value)| format!("{name} {value}"))
                .collect::<Vec<_>>()
                .join("; "),
            ),
        ];
        if self.withheld {
            rows.push(Row {
                name: "Value",
                value: VALUE_WITHHELD.into(),
                withheld: true,
            });
        }
        rows.push(row(
            "Picture",
            match self.png {
                Some(_) => format!("{} × {} pixels", self.width, self.height),
                None => "None: the page didn't give one.".into(),
            },
        ));
        rows.retain(|row| !row.value.is_empty());
        rows
    }

    /// The panel's closed line, which says what goes without opening it, and what doesn't.
    pub fn folded(&self) -> String {
        let what = match (self.text.is_empty(), self.png.is_some()) {
            (false, true) => format!("Sends the {}, its text and this picture.", self.word()),
            (false, false) => format!("Sends the {} and its text.", self.word()),
            (true, true) => format!("Sends the {} and this picture.", self.word()),
            (true, false) => format!("Sends the {}.", self.word()),
        };
        match self.withheld {
            true => format!("{what} No field's value is sent."),
            false => what,
        }
    }
}

/// A note: the element the user picked and what they wrote about it.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub element: Element,
    pub words: String,
}

impl Note {
    /// Its mark's label once it's added: its number and the note's words, as the user wrote them.
    pub fn label(&self, number: usize) -> String {
        match self.words.trim() {
            "" => format!("{number} · {}", self.element.label()),
            words => format!("{number} · {words}"),
        }
    }
}

/// The agent's answer to its `point` call once the user sends `notes`: for each, the note and its
/// rows as the panel showed them, and its picture, the pictures in the notes' order.
pub fn answer(notes: &[Note]) -> (String, Vec<Arc<[u8]>>) {
    let count = match notes.len() {
        1 => "1 element".to_owned(),
        count => format!("{count} elements"),
    };
    let mut text = format!(
        "The user pointed at {count}. Each picture follows in the same order, for the elements \
         that have one."
    );
    for (index, note) in notes.iter().enumerate() {
        text.push_str(&format!("\n\n{}. ", index + 1));
        match note.words.trim() {
            "" => text.push_str("They wrote no note."),
            words => text.push_str(&format!("Their note: {words}")),
        }
        for row in note.element.rows() {
            text.push_str(&format!("\n{}: {}", row.name, row.value));
        }
    }
    let pictures = notes
        .iter()
        .filter_map(|note| note.element.png.clone())
        .collect();
    (text, pictures)
}

/// The log's lines for a sent answer, which its row in Settings › Agents opens to: the page's
/// title, then each element by its number, with the user's note and the panel's rows, its kind
/// alone, its value as "Not sent" and its picture as its size. Library › Agents' file keeps fewer
/// (agent_history::kept_lines).
pub fn sent(notes: &[Note]) -> Vec<Sent> {
    let line = |depth, name: &str, value: String| Sent {
        depth,
        name: name.to_owned(),
        value,
    };
    let title = notes.first().map(|note| note.element.title.clone());
    let mut lines: Vec<Sent> = title
        .map(|title| line(0, "title", title))
        .into_iter()
        .collect();
    for (index, note) in notes.iter().enumerate() {
        let element = &note.element;
        lines.push(line(0, &(index + 1).to_string(), String::new()));
        lines.push(line(1, "kind", label(&element.kind, "")));
        lines.push(line(1, "note", note.words.trim().to_owned()));
        for row in element.rows() {
            let (name, value) = match row.name {
                "Page" | "Title" | "Element" => continue,
                "Finds it" => ("found by", row.value),
                "Value" => ("value", "Not sent".to_owned()),
                name => (name, row.value),
            };
            lines.push(line(1, &name.to_lowercase(), value));
        }
    }
    lines.retain(|line| !line.value.is_empty() || line.depth == 0);
    lines
}

/// The log's title for a sent answer, "You pointed at 3 elements".
pub fn logged(count: usize) -> String {
    match count {
        1 => "You pointed at 1 element".into(),
        count => format!("You pointed at {count} elements"),
    }
}

/// Why Point can't start on the tab in front, from bc_tab_may_point, for its tooltip: the user's
/// to read, never the agent's.
pub fn why_not(can: i32) -> Option<&'static str> {
    match can {
        0 => None,
        2 => Some("Point can't point into a private window."),
        3 => Some("This tab is another agent's."),
        4 => Some("Finish arranging this page first."),
        _ => Some("Point works on web pages. Show one to point at it."),
    }
}

/// An element from the core's report, lasting only for the call.
///
/// # Safety
///
/// `element` must be the core's bc_point_element, its strings and bytes alive for the call.
pub unsafe fn from_core(element: &crate::bc::bc_point_element) -> Element {
    let text = |ptr: *const std::ffi::c_char| match ptr.is_null() {
        true => String::new(),
        false => unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned(),
    };
    let rect = element.box_;
    let png = (!element.png.is_null() && element.size > 0)
        .then(|| Arc::from(unsafe { std::slice::from_raw_parts(element.png, element.size) }));
    Element {
        kind: text(element.element.kind),
        name: text(element.element.name),
        address: text(element.address),
        title: text(element.title),
        tag: text(element.tag),
        text: text(element.text),
        selector: text(element.selector),
        color: text(element.color),
        background: text(element.background),
        font: text(element.font),
        font_size: text(element.font_size),
        padding: text(element.padding),
        frame: Bounds::new(
            point(px(rect.x as f32), px(rect.y as f32)),
            size(px(rect.width as f32), px(rect.height as f32)),
        ),
        withheld: element.withheld,
        png,
        width: element.width.max(0) as u32,
        height: element.height.max(0) as u32,
    }
}

#[cfg(test)]
mod tests;
