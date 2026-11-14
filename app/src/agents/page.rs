//! A page as an agent sees it and acts on it: the outline of its accessibility tree, as the core's
//! bc_agent_page gives it without any field's value, with a ref for each element it can act on,
//! and which actions ask the user first.

use std::collections::HashMap;

use serde_json::Value;

use super::TabInfo;

/// Roles an agent can act on, each given a ref.
const INTERACTIVE: &[&str] = &[
    "button",
    "link",
    "textbox",
    "searchbox",
    "combobox",
    "checkbox",
    "radio",
    "switch",
    "slider",
    "spinbutton",
    "listbox",
    "option",
    "menuitem",
    "menuitemcheckbox",
    "menuitemradio",
    "tab",
    "treeitem",
];
/// Roles given a ref too, to scroll in: a frame, and the parts of a page a site often scrolls on
/// their own.
const SCROLLS: &[&str] = &[
    "Iframe",
    "IframePresentational",
    "main",
    "region",
    "document",
    "article",
    "feed",
    "log",
    "list",
    "grid",
    "tree",
    "tabpanel",
    "dialog",
    "alertdialog",
    "complementary",
    "navigation",
];
/// Wrappers that only add nesting, whose children are outlined a level up.
const FLATTEN: &[&str] = &[
    "none",
    "generic",
    "presentation",
    "LayoutTable",
    "LayoutTableRow",
    "LayoutTableCell",
    "InlineTextBox",
    "RootWebArea",
];
/// Landmarks worth a line without a name.
const LANDMARKS: &[&str] = &[
    "form",
    "dialog",
    "alertdialog",
    "alert",
    "navigation",
    "main",
    "banner",
    "search",
    "contentinfo",
];
/// Parts of a table or list a person may click, as a console's list of things does, given a ref
/// too. A row or list item with no name of its own, as Chromium gives neither one, is named by its
/// text, when that's all it holds.
const PARTS: &[&str] = &[
    "row",
    "cell",
    "gridcell",
    "rowheader",
    "columnheader",
    "listitem",
];
const STATES: &[&str] = &[
    "checked", "selected", "expanded", "disabled", "readonly", "required", "level",
];
/// A frame's element, under which the core puts its document.
const FRAMES: &[&str] = &["Iframe", "IframePresentational"];
/// The outline's bytes at most, as a long page would fill the agent's context.
pub const MAX_OUTLINE: usize = 30_000;

/// The keys press_key presses, as the core knows them.
pub const KEYS: &[&str] = &[
    "Enter",
    "Tab",
    "Escape",
    "Backspace",
    "Delete",
    "Space",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "PageUp",
    "PageDown",
    "Home",
    "End",
];

/// An element of a page an agent may act on, by its ref.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    /// The DOM node's backend id, which the core acts on.
    pub node: i64,
    pub role: String,
    pub name: String,
    pub password: bool,
}

/// A tab's refs, which stay the same for an element while its page is the same.
#[derive(Debug, Default)]
pub struct Refs {
    next: u32,
    by_node: HashMap<i64, u32>,
    by_ref: HashMap<u32, Element>,
}

impl Refs {
    /// The element a ref such as "e12" names, from the latest outline.
    pub fn get(&self, name: &str) -> Option<&Element> {
        let number = name.trim().strip_prefix('e')?.parse().ok()?;
        self.by_ref.get(&number)
    }

    /// The number of the ref naming the element whose DOM node is `node`.
    pub fn number(&self, node: i64) -> Option<u32> {
        self.by_node.get(&node).copied()
    }

    fn name(&mut self, element: Element) -> u32 {
        let number = *self.by_node.entry(element.node).or_insert_with(|| {
            self.next += 1;
            self.next
        });
        self.by_ref.insert(number, element);
        number
    }
}

/// The outlines an action answers with changes, as what's left would rather be read whole.
const MAX_COMPARED: usize = 1_000_000;

/// A page's outline, from the nodes of its tree, which names each element it can act on in its
/// tab's `refs`: a line for each, as `- role "name" [e12] (states)`, and `- text: …` for its
/// text. With `part`, the outline of that element alone, by its DOM node.
pub fn outline(tab: &TabInfo, nodes: &[Value], refs: &mut Refs, part: Option<i64>) -> Outline {
    let by_id: HashMap<&str, &Value> = (nodes.iter())
        .filter_map(|node| Some((node.get("nodeId")?.as_str()?, node)))
        .collect();
    // Label text repeats the name of the field it labels.
    let field_names: Vec<String> = (nodes.iter())
        .filter(|node| INTERACTIVE.contains(&role(node)))
        .map(|node| name(node))
        .filter(|name| !name.is_empty())
        .collect();
    let mut walk = Walk {
        by_id,
        field_names,
        refs,
        lines: Vec::new(),
        size: 0,
        password: false,
        captcha: false,
    };
    let backend = |node: &&Value| node.get("backendDOMNodeId").and_then(Value::as_i64);
    let root = match part {
        Some(part) => nodes.iter().find(|node| backend(node) == Some(part)),
        None => nodes.first(),
    };
    if let Some(root) = root {
        walk.node(root, 0, "", 0);
    }
    let mut notes = Vec::new();
    if root.is_none() && part.is_some() {
        notes.push("That element isn't on the page any more. Take a snapshot without a ref.");
    }
    if walk.size > MAX_OUTLINE {
        notes.push("The outline is cut here. read_page has all of the page's text.");
    }
    if walk.password {
        notes.push(
            "This page has a password field, which is off limits to you: signing in is the user's \
             to do. Hand them the tab with hand_off.",
        );
    }
    if walk.captcha {
        notes.push(
            "This page shows a CAPTCHA, which is the user's to do. Hand them the tab with hand_off.",
        );
    }
    let title = nodes.first().map(name).filter(|title| !title.is_empty());
    let title = title.unwrap_or_else(|| tab.title.to_string());
    let url = tab.url.to_string();
    Outline {
        head: format!("Tab {}: {title}\nURL: {url}", tab.id),
        page: url.split('#').next().unwrap_or_default().to_string(),
        document: nodes.first().as_ref().and_then(backend),
        lines: walk.lines,
        notes,
    }
}

/// A page's outline, which an answer gives whole, or as what changed since one the agent had.
#[derive(Clone, Debug, Default)]
pub struct Outline {
    /// Which tab and page it is: "Tab 20: Shop\nURL: https://shop.example/".
    head: String,
    /// The page's address without its fragment, and its document's node: either changes on a new
    /// page.
    page: String,
    document: Option<i64>,
    lines: Vec<Line>,
    notes: Vec<&'static str>,
}

#[derive(Clone, Debug, Default)]
struct Line {
    depth: usize,
    /// The line without its indent, as `- button "Pay" [e4] (disabled)`.
    text: String,
    /// Its element's ref, by number, if it has one.
    element: Option<u32>,
    /// Its element's name and states, as in its text, for what changed in it.
    name: String,
    states: Vec<String>,
}

impl Line {
    /// What changed in the element since `was`, its line before: its name, and each state it has
    /// now and hadn't, or had and hasn't, as "now enabled".
    fn change(&self, was: &Line) -> String {
        let mut changes = Vec::new();
        if self.name != was.name {
            changes.push(format!("was “{}”", clip(&was.name, 60)));
        }
        for state in was
            .states
            .iter()
            .filter(|state| !self.states.contains(state))
        {
            changes.push(match state.as_str() {
                "disabled" => "now enabled".into(),
                "checked" => "now unchecked".into(),
                "expanded" => "now collapsed".into(),
                "read-only" => "now editable".into(),
                state => format!("no longer {state}"),
            });
        }
        for state in self
            .states
            .iter()
            .filter(|state| !was.states.contains(state))
        {
            changes.push(format!("now {state}"));
        }
        match changes.is_empty() {
            true => self.text.clone(),
            false => format!("{}: {}", self.text, changes.join(", ")),
        }
    }
}

impl Outline {
    /// Which tab and page it is, without the outline.
    pub fn head(&self) -> &str {
        &self.head
    }

    /// The outline whole.
    pub fn text(&self) -> String {
        let lines = (self.lines.iter())
            .map(|line| format!("{}{}", "  ".repeat(line.depth), line.text))
            .collect::<Vec<_>>();
        self.with_notes(format!("{}\n\n{}", self.head, lines.join("\n")))
    }

    /// What changed since `last`, the outline the agent had: the lines added, changed or gone,
    /// each the same line as before by its element's ref, else by its text in the same order. A
    /// new page, or changes of more than half the outline, are answered with the outline whole.
    pub fn since(&self, last: &Outline) -> String {
        if (self.page.as_str(), self.document) != (last.page.as_str(), last.document) {
            return self.text();
        }
        let Some(same) = same_lines(&last.lines, &self.lines) else {
            return self.text();
        };
        let kept: std::collections::HashSet<usize> = same.iter().flatten().copied().collect();
        // Each changed line, with what changed in it.
        let changed: Vec<(&Line, String)> = (self.lines.iter().zip(&same))
            .filter_map(|(line, was)| {
                let was = &last.lines[(*was)?];
                (line.element.is_some() && line.text != was.text).then(|| (line, line.change(was)))
            })
            .collect();
        let mut parts = Vec::new();
        let added = runs(&self.lines, |at| same[at].is_none());
        if !added.is_empty() {
            parts.push(format!("New:\n{added}"));
        }
        if !changed.is_empty() {
            let lines: Vec<&str> = changed.iter().map(|(_, said)| said.as_str()).collect();
            parts.push(format!("Changed:\n{}", lines.join("\n")));
        }
        let gone = runs(&last.lines, |at| !kept.contains(&at));
        if !gone.is_empty() {
            parts.push(format!("Gone:\n{gone}"));
        }
        if parts.is_empty() {
            return self.with_notes(format!("{}\n\nNothing in the outline changed.", self.head));
        }
        let changes = parts.join("\n\n");
        // What it says of each change counts for nothing here: how much changed does.
        let said: usize = (changed.iter())
            .map(|(line, said)| said.len() - line.text.len())
            .sum();
        let whole: usize = self
            .lines
            .iter()
            .map(|line| 2 * line.depth + line.text.len() + 1)
            .sum();
        if 2 * (changes.len() - said) > whole {
            return self.text();
        }
        let since = "What changed since the last outline you had of this tab:";
        self.with_notes(format!("{}\n\n{since}\n\n{changes}", self.head))
    }

    fn with_notes(&self, mut text: String) -> String {
        if !self.notes.is_empty() {
            text.push_str("\n\n");
            text.push_str(&self.notes.join("\n"));
        }
        text
    }
}

/// For each of `new`'s lines, the line of `old` it is, if any: the same element by its ref, else
/// the same text, in the longest run of them both have in order. None when they're too far apart
/// to compare.
fn same_lines(old: &[Line], new: &[Line]) -> Option<Vec<Option<usize>>> {
    let same = |a: &Line, b: &Line| match (a.element, b.element) {
        (None, None) => a.text == b.text,
        (a, b) => a == b,
    };
    let start = old.iter().zip(new).take_while(|(a, b)| same(a, b)).count();
    let end = (old[start..].iter().rev().zip(new[start..].iter().rev()))
        .take_while(|(a, b)| same(a, b))
        .count();
    let (old_rest, new_rest) = (&old[start..old.len() - end], &new[start..new.len() - end]);
    let (rows, columns) = (old_rest.len(), new_rest.len() + 1);
    if rows * columns > MAX_COMPARED {
        return None;
    }
    // At [i * columns + j], how many lines `old_rest[i..]` and `new_rest[j..]` have in common.
    let mut common = vec![0u16; (rows + 1) * columns];
    for i in (0..rows).rev() {
        for j in (0..columns - 1).rev() {
            common[i * columns + j] = match same(&old_rest[i], &new_rest[j]) {
                true => common[(i + 1) * columns + j + 1] + 1,
                false => common[(i + 1) * columns + j].max(common[i * columns + j + 1]),
            };
        }
    }
    let mut was: Vec<Option<usize>> = (0..new.len()).map(|_| None).collect();
    for at in 0..start {
        was[at] = Some(at);
    }
    for back in 1..=end {
        was[new.len() - back] = Some(old.len() - back);
    }
    let (mut i, mut j) = (0, 0);
    while i < rows && j < columns - 1 {
        if same(&old_rest[i], &new_rest[j]) {
            was[start + j] = Some(start + i);
            (i, j) = (i + 1, j + 1);
        } else if common[(i + 1) * columns + j] >= common[i * columns + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    // An element that moved is still the same element.
    let by_ref: HashMap<u32, usize> = (old.iter().enumerate())
        .filter_map(|(at, line)| Some((line.element?, at)))
        .collect();
    for (at, line) in new.iter().enumerate() {
        if was[at].is_none() {
            was[at] = line
                .element
                .and_then(|element| by_ref.get(&element).copied());
        }
    }
    Some(was)
}

/// The lines `pick` picks, each run of them in a row indented from its own outermost line.
fn runs(lines: &[Line], pick: impl Fn(usize) -> bool) -> String {
    let mut text = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        if !pick(at) {
            at += 1;
            continue;
        }
        let run = (at..lines.len()).take_while(|&at| pick(at)).count();
        let lines = &lines[at..at + run];
        let outermost = lines
            .iter()
            .map(|line| line.depth)
            .min()
            .unwrap_or_default();
        text.extend(
            lines
                .iter()
                .map(|line| format!("{}{}", "  ".repeat(line.depth - outermost), line.text)),
        );
        at += run;
    }
    text.join("\n")
}

struct Walk<'a> {
    by_id: HashMap<&'a str, &'a Value>,
    field_names: Vec<String>,
    refs: &'a mut Refs,
    lines: Vec<Line>,
    size: usize,
    password: bool,
    captcha: bool,
}

impl Walk<'_> {
    fn add(&mut self, line: Line) {
        if self.size > MAX_OUTLINE {
            return;
        }
        // Its indent and line break too.
        self.size += 2 * line.depth + line.text.len() + 1;
        self.lines.push(line);
    }

    /// The text a row or list item holds, if that's all it holds: no element to act on, which it
    /// would rather be acted on by.
    fn text_of(&self, node: &Value, levels: usize) -> Option<String> {
        if levels > 50 {
            return None;
        }
        if role(node) == "StaticText" {
            return Some(name(node));
        }
        if INTERACTIVE.contains(&role(node)) {
            return None;
        }
        self.text_inside(node, levels)
    }

    /// The text of what the node holds, if that's all it holds.
    fn text_inside(&self, node: &Value, levels: usize) -> Option<String> {
        let mut text = Vec::new();
        let children = node.get("childIds").and_then(Value::as_array);
        for child in children.into_iter().flatten() {
            if let Some(child) = child.as_str().and_then(|id| self.by_id.get(id).copied()) {
                let each = self.text_of(child, levels + 1)?;
                if !each.is_empty() {
                    text.push(each);
                }
            }
        }
        Some(text.join(" "))
    }

    fn node(&mut self, node: &Value, depth: usize, parent_name: &str, levels: usize) {
        // A page's tree is shallow; one nested deeper is cut rather than overflow the stack.
        if levels > 200 || self.size > MAX_OUTLINE {
            return;
        }
        let ignored = node
            .get("ignored")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let role = role(node);
        let name = name(node);
        let lower = name.to_lowercase();
        let human = ["captcha", "not a robot", "verify you are human"];
        if human.iter().any(|words| lower.contains(words)) {
            self.captcha = true;
        }
        // A list's bullet or number, which its indent already says.
        if role == "ListMarker" {
            return;
        }
        if !ignored && role == "StaticText" {
            let repeated = parent_name.contains(&name) || self.field_names.contains(&name);
            if !name.is_empty() && !repeated {
                self.add(Line {
                    depth,
                    text: format!("- text: {name}"),
                    ..Line::default()
                });
            }
            return;
        }
        let name = match (role, name.is_empty()) {
            ("listitem" | "row", true) => (self.text_of(node, 0))
                .map(|text| clip(&text, 160))
                .unwrap_or_default(),
            // A label the core shows as the radio or checkbox the page hid behind it, as choice
            // tiles do, which Chromium leaves unnamed.
            (_, true) if node.get("label").is_some_and(|label| label == true) => {
                let text = self.text_inside(node, 0);
                text.map(|text| clip(&text, 160)).unwrap_or_default()
            }
            _ => name,
        };
        let (mut next, mut next_name) = (depth, parent_name.to_string());
        let shown = INTERACTIVE.contains(&role) || !name.is_empty() || LANDMARKS.contains(&role);
        // Where a frame's element's line went, and the size before it.
        let mut frame = None;
        if !ignored && !FLATTEN.contains(&role) && shown {
            let mut line = format!("- {role}");
            let mut reference = None;
            if !name.is_empty() {
                line.push_str(&format!(" \"{}\"", clip(&name, 160)));
            }
            let backend = node.get("backendDOMNodeId").and_then(Value::as_i64);
            let refers =
                INTERACTIVE.contains(&role) || SCROLLS.contains(&role) || PARTS.contains(&role);
            if let (true, Some(backend)) = (refers, backend) {
                let password = node
                    .get("password")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let element = Element {
                    node: backend,
                    role: role.into(),
                    name: name.clone(),
                    password,
                };
                let number = self.refs.name(element);
                reference = Some(number);
                line.push_str(&format!(" [e{number}]"));
                if password {
                    self.password = true;
                    line.push_str(" (password: off limits)");
                }
            }
            let mut states = Vec::new();
            for property in node
                .get("properties")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(state) = property.get("name").and_then(Value::as_str) else {
                    continue;
                };
                if !STATES.contains(&state) {
                    continue;
                }
                let value = property.pointer("/value/value");
                let shown = if state == "readonly" {
                    "read-only"
                } else {
                    state
                };
                match (state, value) {
                    ("level", _) if role == "listitem" => {}
                    ("level", Some(level)) => states.push(format!("level {level}")),
                    (_, Some(Value::Bool(true))) => states.push(shown.into()),
                    (_, Some(Value::String(text))) if text == "true" => states.push(shown.into()),
                    _ => {}
                }
            }
            for state in &states {
                line.push_str(&format!(" ({state})"));
            }
            // A native select's choices, the page's own: the core leaves out which one is chosen.
            let choices = node.get("choices").and_then(Value::as_array);
            let choices: Vec<_> = choices
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            if !choices.is_empty() {
                line.push_str(&format!(" (options: {})", listed(&choices)));
            }
            let before = (self.lines.len(), self.size);
            self.add(Line {
                depth,
                text: line,
                element: reference,
                name: name.clone(),
                states,
            });
            if FRAMES.contains(&role) && self.lines.len() > before.0 {
                frame = Some(before);
            }
            next = depth + 1;
            next_name = name;
        }
        let children = node.get("childIds").and_then(Value::as_array);
        for child in children.into_iter().flatten() {
            if let Some(child) = child.as_str().and_then(|id| self.by_id.get(id).copied()) {
                self.node(child, next, &next_name, levels + 1);
            }
        }
        // A frame with nothing under its line, as a site's helper frames have, says nothing
        // (#1444).
        if let Some((at, size)) = frame
            && self.lines.len() == at + 1
        {
            self.lines.pop();
            self.size = size;
        }
    }
}

fn role(node: &Value) -> &str {
    node.pointer("/role/value")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// The node's name, its runs of white space one space.
fn name(node: &Value) -> String {
    let name = node
        .pointer("/name/value")
        .and_then(Value::as_str)
        .unwrap_or_default();
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text`, cut at `chars` characters.
pub fn clip(text: &str, chars: usize) -> String {
    match text.char_indices().nth(chars) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.into(),
    }
}

/// How many of a select's choices its line names: enough for any form's, while a country list's
/// says how many more there are, which `select` finds by name.
const CHOICES_NAMED: usize = 40;

/// A select's choices as its line names them, each quoted.
fn listed(choices: &[&str]) -> String {
    let mut named: Vec<_> = (choices.iter().take(CHOICES_NAMED))
        .map(|choice| format!("\"{}\"", clip(choice, 80)))
        .collect();
    if choices.len() > CHOICES_NAMED {
        named.push(format!("and {} more", choices.len() - CHOICES_NAMED));
    }
    named.join(", ")
}

/// Words that pay or order, as a button's name has them.
const PAYMENT: &[&str] = &[
    "pay",
    "buy",
    "purchase",
    "checkout",
    "check out",
    "place order",
    "place your order",
    "submit order",
    "order now",
    "subscribe",
    "donate",
    "transfer",
];
/// Words that send, as a button's name has them.
const SEND: &[&str] = &[
    "send", "submit", "post", "publish", "reply", "tweet", "comment",
];
const MESSAGING: &[&str] = &[
    "mail.google.com",
    "outlook.live.com",
    "outlook.office.com",
    "slack.com",
    "discord.com",
    "x.com",
    "twitter.com",
    "linkedin.com",
    "facebook.com",
    "messenger.com",
    "instagram.com",
    "whatsapp.com",
    "web.telegram.org",
    "reddit.com",
];
const BANKS: &[&str] = &[
    "chase",
    "wellsfargo",
    "bankofamerica",
    "citi",
    "capitalone",
    "schwab",
    "fidelity",
    "vanguard",
    "paypal",
    "venmo",
    "revolut",
    "wise",
    "monzo",
    "coinbase",
];

/// Why acting on the element named `label` on `host` asks the user first, if it does: it looks
/// like a payment or an order, or it sends a message or post from a site that sends them. Typing
/// with submit and pressing Enter are "send".
pub fn risk(host: &str, label: &str) -> Option<&'static str> {
    let label = label.to_lowercase();
    if PAYMENT.iter().any(|words| has_words(&label, words)) {
        return Some(ORDER);
    }
    let sends = SEND.iter().any(|words| has_words(&label, words));
    if sends && MESSAGING.iter().any(|site| on(host, site)) {
        return Some(SENDS);
    }
    None
}

/// Why a payment or an order asks first, which a session's receipt counts as an order placed.
pub const ORDER: &str = "It looks like a payment or an order.";
/// Why sending a message or a post asks first, which the receipt counts as a message sent.
pub const SENDS: &str = "It sends a message or a post.";

/// Why opening a page on `host` asks the user first, if it does: it's a bank's.
pub fn opening_risk(host: &str) -> Option<&'static str> {
    let host = host.to_lowercase();
    let bank = BANKS.iter().any(|bank| on(&host, &format!("{bank}.com"))) || host.contains("bank");
    bank.then_some("It's a bank's site.")
}

/// Whether `host` is `site` or under it.
fn on(host: &str, site: &str) -> bool {
    host.eq_ignore_ascii_case(site)
        || (host.len() > site.len()
            && host[host.len() - site.len()..].eq_ignore_ascii_case(site)
            && host.as_bytes()[host.len() - site.len() - 1] == b'.')
}

/// Whether `text` has `words` as words of its own, not inside longer ones.
fn has_words(text: &str, words: &str) -> bool {
    text.match_indices(words).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + words.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn tab() -> TabInfo {
        TabInfo {
            id: 20,
            title: "Shop".into(),
            url: "https://shop.example/".into(),
            ..TabInfo::default()
        }
    }

    fn node(id: &str, role: &str, name: &str, children: &[&str]) -> Value {
        json!({
            "nodeId": id,
            "role": { "value": role },
            "name": { "value": name },
            "childIds": children,
        })
    }

    /// A native select's line names its choices, the page's own, as the core gives them without
    /// which one is chosen, and a long list says how many more there are (#2355).
    #[test]
    fn a_selects_line_names_its_choices() {
        let countries: Vec<String> = (1..=45).map(|n| format!("Country {n}")).collect();
        let mut nodes = vec![
            node("1", "RootWebArea", "Form", &["2", "3"]),
            node("2", "combobox", "Answer", &[]),
            node("3", "combobox", "Country", &[]),
        ];
        nodes[1]["backendDOMNodeId"] = json!(20);
        nodes[1]["choices"] = json!(["Yes", "No"]);
        nodes[2]["backendDOMNodeId"] = json!(30);
        nodes[2]["choices"] = json!(countries);
        let outline = outline(&tab(), &nodes, &mut Refs::default(), None).text();
        assert!(
            outline.contains("- combobox \"Answer\" [e1] (options: \"Yes\", \"No\")\n"),
            "{outline}"
        );
        assert!(
            outline.contains("\"Country 39\", \"Country 40\", and 5 more)"),
            "{outline}"
        );
        assert!(!outline.contains("Country 41"), "{outline}");
    }

    /// The outline names what an agent can act on with refs that stay while the page does, gives
    /// the page's text once, and marks a password field off limits.
    #[test]
    fn a_page_is_outlined_with_refs() {
        let mut nodes = vec![
            node("1", "RootWebArea", "Shop", &["2"]),
            node("2", "generic", "", &["3", "4", "6", "7", "9"]),
            node("3", "heading", "Basket", &["10"]),
            node("4", "button", "Place your order", &["5"]),
            node("5", "StaticText", "Place your order", &[]),
            node("6", "textbox", "Coupon", &[]),
            node("7", "textbox", "Password", &[]),
            node("9", "checkbox", "Gift", &[]),
            node("10", "StaticText", "Basket", &[]),
        ];
        nodes[3]["backendDOMNodeId"] = json!(40);
        nodes[5]["backendDOMNodeId"] = json!(60);
        nodes[6]["backendDOMNodeId"] = json!(70);
        nodes[6]["password"] = json!(true);
        nodes[7]["backendDOMNodeId"] = json!(90);
        nodes[7]["properties"] = json!([{ "name": "checked", "value": { "value": "true" } }]);
        let mut refs = Refs::default();
        let outline = outline(&tab(), &nodes, &mut refs, None).text();
        assert_eq!(
            outline,
            "Tab 20: Shop\nURL: https://shop.example/\n\n\
             - heading \"Basket\"\n\
             - button \"Place your order\" [e1]\n\
             - textbox \"Coupon\" [e2]\n\
             - textbox \"Password\" [e3] (password: off limits)\n\
             - checkbox \"Gift\" [e4] (checked)\n\n\
             This page has a password field, which is off limits to you: signing in is the \
             user's to do. Hand them the tab with hand_off."
        );
        assert_eq!(refs.get("e2").map(|element| element.node), Some(60));
        assert!(refs.get("e3").unwrap().password);
        assert_eq!(refs.get("e9"), None);
        assert_eq!(refs.get("button"), None);

        // Outlined again, the same element keeps its ref.
        nodes.swap(4, 5);
        let again = super::outline(&tab(), &nodes, &mut refs, None).text();
        assert!(again.contains("- textbox \"Coupon\" [e2]"));
    }

    /// A shop's page, its menu and a size picker, open or not, each node's DOM node its id.
    fn shop(open: bool) -> Vec<Value> {
        let mut nodes = vec![
            node("1", "RootWebArea", "Shop", &["2", "3", "4", "5", "6"]),
            node("2", "navigation", "Menu", &["21", "22", "23", "24"]),
            node("21", "link", "Home", &[]),
            node("22", "link", "Shirts", &[]),
            node("23", "link", "Shoes", &[]),
            node("24", "link", "Basket", &[]),
            node("3", "heading", "Linen shirt", &[]),
            node("4", "paragraph", "", &["41"]),
            node(
                "41",
                "StaticText",
                "Light linen, cut loose, for long summer days.",
                &[],
            ),
            node("5", "combobox", "Size", if open { &["7"] } else { &[] }),
            node("6", "button", "Add to basket", &[]),
            node("7", "listbox", "Size", &["71", "72"]),
            node("71", "option", "S", &[]),
            node("72", "option", "M", &[]),
        ];
        for node in &mut nodes {
            node["backendDOMNodeId"] =
                json!(node["nodeId"].as_str().unwrap().parse::<i64>().unwrap());
        }
        if open {
            nodes[9]["properties"] = json!([{ "name": "expanded", "value": { "value": true } }]);
        }
        nodes
    }

    /// An action on the same page answers with the lines new, changed and gone since the outline
    /// the agent had, each the same line by its ref, else its text; nothing, when nothing changed.
    #[test]
    fn an_outline_gives_what_changed_since_another() {
        let mut refs = Refs::default();
        let closed = outline(&tab(), &shop(false), &mut refs, None);
        let open = outline(&tab(), &shop(true), &mut refs, None);
        let head = "Tab 20: Shop\nURL: https://shop.example/\n\n";
        let since = "What changed since the last outline you had of this tab:\n\n";
        assert_eq!(
            open.since(&closed),
            format!(
                "{head}{since}New:\n\
                 - listbox \"Size\" [e8]\n  - option \"S\" [e9]\n  - option \"M\" [e10]\n\n\
                 Changed:\n- combobox \"Size\" [e6] (expanded): now expanded"
            )
        );
        assert_eq!(
            closed.since(&open),
            format!(
                "{head}{since}Changed:\n- combobox \"Size\" [e6]: now collapsed\n\n\
                 Gone:\n- listbox \"Size\" [e8]\n  - option \"S\" [e9]\n  - option \"M\" [e10]"
            )
        );
        assert_eq!(
            open.since(&open),
            format!("{head}Nothing in the outline changed.")
        );

        // Elements in another order are the same elements.
        let mut moved = shop(false);
        moved[1]["childIds"] = json!(["21", "23", "22", "24"]);
        let moved = outline(&tab(), &moved, &mut refs, None);
        assert_eq!(
            moved.since(&closed),
            format!("{head}Nothing in the outline changed.")
        );
        // Text is the same line by its text, in order.
        let mut text = shop(false);
        text[8]["name"]["value"] = json!("Heavy linen.");
        let text = outline(&tab(), &text, &mut refs, None);
        assert!(text.since(&closed).ends_with(
            "New:\n- text: Heavy linen.\n\nGone:\n- text: Light linen, cut loose, for long summer days."
        ));
    }

    /// What changed in an element says so, as a state it no longer has can't be seen in its line:
    /// a button the page enabled, a field it made read-only, a link it renamed.
    #[test]
    fn a_change_says_what_changed() {
        let form = |enabled: bool| {
            let mut nodes = vec![
                node(
                    "1",
                    "RootWebArea",
                    "Form",
                    &["2", "3", "4", "5", "6", "7", "8"],
                ),
                node("2", "textbox", "Name", &[]),
                node("3", "button", "Continue", &[]),
                node(
                    "4",
                    "link",
                    if enabled { "Help" } else { "Help and contact" },
                    &[],
                ),
                node("5", "heading", "Your details, as on your passport", &[]),
                node("6", "textbox", "Date of birth", &[]),
                node("7", "textbox", "Country of birth", &[]),
                node("8", "textbox", "Passport number", &["9"]),
                node(
                    "9",
                    "StaticText",
                    "As it's printed on the page with your photo, in capitals, with no spaces, \
                     the letters and digits after the country's code. A passport that runs out \
                     within six months of travelling can't be used; renew it first.",
                    &[],
                ),
            ];
            for node in &mut nodes {
                node["backendDOMNodeId"] =
                    json!(node["nodeId"].as_str().unwrap().parse::<i64>().unwrap());
            }
            let state = |name: &str| json!([{ "name": name, "value": { "value": true } }]);
            match enabled {
                true => nodes[1]["properties"] = state("readonly"),
                false => nodes[2]["properties"] = state("disabled"),
            }
            nodes
        };
        let mut refs = Refs::default();
        let before = outline(&tab(), &form(false), &mut refs, None);
        assert!(
            before
                .text()
                .contains("- button \"Continue\" [e2] (disabled)")
        );
        let after = outline(&tab(), &form(true), &mut refs, None);
        assert!(after.text().contains("- textbox \"Name\" [e1] (read-only)"));
        assert!(
            after.since(&before).ends_with(
                "Changed:\n\
                 - textbox \"Name\" [e1] (read-only): now read-only\n\
                 - button \"Continue\" [e2]: now enabled\n\
                 - link \"Help\" [e3]: was “Help and contact”"
            ),
            "{}",
            after.since(&before)
        );
    }

    /// A table's rows and cells and a list's items have refs, as a person may click them; an item
    /// holding only text is named by it, and one holding a link leaves the ref to the link.
    #[test]
    fn rows_and_items_have_refs() {
        let mut nodes = vec![
            node("1", "RootWebArea", "Console", &["2", "5"]),
            node("2", "table", "Instances", &["3"]),
            // Chromium names neither a row nor a list item by what it holds.
            node("3", "row", "", &["4", "42"]),
            node("4", "cell", "web-1", &["41"]),
            node("41", "StaticText", "web-1", &[]),
            node("42", "cell", "Running", &["43"]),
            node("43", "StaticText", "Running", &[]),
            node("5", "list", "", &["6", "7"]),
            node("6", "listitem", "", &["60", "61"]),
            node("60", "ListMarker", "•", &[]),
            node("61", "StaticText", "Instances", &[]),
            node("7", "listitem", "", &["71"]),
            node("71", "link", "Volumes", &["72"]),
            node("72", "StaticText", "Volumes", &[]),
        ];
        for node in &mut nodes {
            node["backendDOMNodeId"] =
                json!(node["nodeId"].as_str().unwrap().parse::<i64>().unwrap());
        }
        // Its level says nothing a list item's indent doesn't.
        nodes[6]["properties"] = json!([{ "name": "level", "value": { "value": 1 } }]);
        let mut refs = Refs::default();
        assert_eq!(
            outline(&tab(), &nodes, &mut refs, None).text(),
            "Tab 20: Console\nURL: https://shop.example/\n\n\
             - table \"Instances\"\n\
             \x20 - row \"web-1 Running\" [e1]\n\
             \x20   - cell \"web-1\" [e2]\n\
             \x20   - cell \"Running\" [e3]\n\
             - listitem \"Instances\" [e4]\n\
             - link \"Volumes\" [e5]"
        );
        assert_eq!(refs.get("e1").map(|element| element.node), Some(3));
        assert_eq!(refs.get("e4").map(|element| element.node), Some(6));
    }

    /// Choice tiles whose radios the page hid have refs, as the radios the core shows their labels
    /// as, named by what they say and with which is chosen.
    #[test]
    fn hidden_choices_are_named_by_their_labels() {
        let mut nodes = vec![
            node("1", "RootWebArea", "Sign up", &["2"]),
            node("2", "group", "Account type", &["3", "6"]),
            // As the core marks a label whose radio has display: none.
            node("3", "radio", "", &["4", "5"]),
            node("4", "heading", "Personal", &["41"]),
            node("41", "StaticText", "Personal", &[]),
            node("5", "paragraph", "", &["51"]),
            node("51", "StaticText", "For you", &[]),
            node("6", "radio", "", &["7"]),
            node("7", "heading", "Business", &["71"]),
            node("71", "StaticText", "Business", &[]),
        ];
        for node in &mut nodes {
            node["backendDOMNodeId"] =
                json!(node["nodeId"].as_str().unwrap().parse::<i64>().unwrap());
        }
        for (at, checked) in [(2, "true"), (7, "false")] {
            nodes[at]["label"] = json!(true);
            nodes[at]["properties"] =
                json!([{ "name": "checked", "value": { "type": "tristate", "value": checked } }]);
        }
        let mut refs = Refs::default();
        assert_eq!(
            outline(&tab(), &nodes, &mut refs, None).text(),
            "Tab 20: Sign up\nURL: https://shop.example/\n\n\
             - group \"Account type\"\n\
             \x20 - radio \"Personal For you\" [e1] (checked)\n\
             \x20   - heading \"Personal\"\n\
             \x20 - radio \"Business\" [e2]\n\
             \x20   - heading \"Business\""
        );
        assert_eq!(refs.get("e2").map(|element| element.node), Some(6));
    }

    /// A new page, by its address or its document, answers whole, as do changes to more than half
    /// the outline; a link to a part of the same page doesn't make a new one.
    #[test]
    fn a_new_page_or_a_new_outline_is_whole() {
        let mut refs = Refs::default();
        let closed = outline(&tab(), &shop(false), &mut refs, None);
        let basket = TabInfo {
            url: "https://shop.example/basket".into(),
            ..tab()
        };
        let elsewhere = outline(&basket, &shop(false), &mut refs, None);
        assert_eq!(elsewhere.since(&closed), elsewhere.text());
        let mut reloaded = shop(false);
        reloaded[0]["backendDOMNodeId"] = json!(100);
        let reloaded = outline(&tab(), &reloaded, &mut refs, None);
        assert_eq!(reloaded.since(&closed), reloaded.text());
        let reviews = TabInfo {
            url: "https://shop.example/#reviews".into(),
            ..tab()
        };
        let reviews = outline(&reviews, &shop(false), &mut refs, None);
        assert!(
            reviews
                .since(&closed)
                .ends_with("Nothing in the outline changed.")
        );

        let mut thanks = shop(false);
        thanks[0]["childIds"] = json!(["3"]);
        thanks[6]["name"]["value"] = json!("Thanks for your order");
        let thanks = outline(&tab(), &thanks, &mut refs, None);
        assert_eq!(thanks.since(&closed), thanks.text());
    }

    /// With an element, the outline is of it alone; one gone from the page says so.
    #[test]
    fn an_outline_of_an_element_alone() {
        let mut refs = Refs::default();
        outline(&tab(), &shop(true), &mut refs, None);
        assert_eq!(
            outline(&tab(), &shop(true), &mut refs, Some(5)).text(),
            "Tab 20: Shop\nURL: https://shop.example/\n\n\
             - combobox \"Size\" [e6] (expanded)\n  - listbox \"Size\" [e7]\n    \
             - option \"S\" [e8]\n    - option \"M\" [e9]"
        );
        let gone = outline(&tab(), &shop(false), &mut refs, Some(999)).text();
        let note = "That element isn't on the page any more. Take a snapshot without a ref.";
        assert!(gone.ends_with(note), "{gone}");
    }

    /// A frame's document is outlined under its element, however deep, and a frame with nothing in
    /// it, as a site's helper frames, is left out (#1444).
    #[test]
    fn a_frame_is_outlined_only_with_what_is_in_it() {
        let nodes = vec![
            node("1", "RootWebArea", "Document", &["2", "3"]),
            node("2", "Iframe", "Not intended for user action", &["1:1"]),
            node("3", "Iframe", "Office on the web Frame", &["2:1"]),
            node("1:1", "RootWebArea", "", &[]),
            node("2:1", "RootWebArea", "Word", &["2:2", "2:3"]),
            node("2:2", "Iframe", "Not intended for user action", &[]),
            node("2:3", "Iframe", "Document", &["3:1"]),
            node("3:1", "RootWebArea", "", &["3:2"]),
            node("3:2", "paragraph", "", &["3:3"]),
            node("3:3", "StaticText", "Quarterly report", &[]),
        ];
        let outline = outline(&tab(), &nodes, &mut Refs::default(), None).text();
        assert_eq!(
            outline,
            "Tab 20: Document\nURL: https://shop.example/\n\n\
             - Iframe \"Office on the web Frame\"\n\
             \x20 - Iframe \"Document\"\n\
             \x20   - text: Quarterly report"
        );
    }

    /// A frame and a part of the page a site scrolls on its own have refs, for scroll to name, and
    /// a heading has none (#1445).
    #[test]
    fn what_scrolls_has_a_ref() {
        let mut nodes = vec![
            node("1", "RootWebArea", "Mail", &["2", "3", "4"]),
            node("2", "heading", "Inbox", &[]),
            node("3", "region", "Messages", &["5"]),
            node("4", "Iframe", "Message", &["6"]),
            node("5", "StaticText", "From Ana", &[]),
            node("6", "StaticText", "Hello", &[]),
        ];
        nodes[1]["backendDOMNodeId"] = json!(20);
        nodes[2]["backendDOMNodeId"] = json!(30);
        nodes[3]["backendDOMNodeId"] = json!(40);
        let mut refs = Refs::default();
        let outline = outline(&tab(), &nodes, &mut refs, None).text();
        assert!(
            outline.ends_with(
                "- heading \"Inbox\"\n\
                 - region \"Messages\" [e1]\n  - text: From Ana\n\
                 - Iframe \"Message\" [e2]\n  - text: Hello"
            ),
            "{outline}"
        );
        assert_eq!(refs.get("e2").map(|element| element.node), Some(40));
    }

    /// A page bigger than an agent should take in is cut, and says so.
    #[test]
    fn a_long_outline_is_cut() {
        let ids: Vec<String> = (2..5000).map(|id| id.to_string()).collect();
        let children: Vec<&str> = ids.iter().map(String::as_str).collect();
        let mut nodes = vec![node("1", "RootWebArea", "Long", &children)];
        nodes.extend(
            ids.iter()
                .map(|id| node(id, "paragraph", &format!("Paragraph {id}"), &[])),
        );
        let outline = outline(&tab(), &nodes, &mut Refs::default(), None).text();
        assert!(outline.len() < MAX_OUTLINE + 1000);
        assert!(outline.ends_with("read_page has all of the page's text."));
    }

    /// Payments and orders ask first anywhere, sending on a site that sends messages, and opening
    /// a bank's site; the words have to be words, not inside others.
    #[test]
    fn what_asks_the_user_first() {
        assert!(risk("shop.example", "Place your order").is_some());
        assert!(risk("shop.example", "Proceed to checkout").is_some());
        assert!(risk("shop.example", "Pay now").is_some());
        assert_eq!(risk("shop.example", "Payment methods"), None);
        assert_eq!(risk("shop.example", "Send"), None);
        assert!(risk("mail.google.com", "Send").is_some());
        assert!(risk("old.reddit.com", "send").is_some());
        assert_eq!(risk("notreddit.com", "send"), None);
        assert_eq!(risk("mail.google.com", "Sender"), None);
        assert!(opening_risk("www.chase.com").is_some());
        assert!(opening_risk("online.mybank.example").is_some());
        assert_eq!(opening_risk("example.com"), None);
        assert_eq!(opening_risk("notchase.com"), None);
    }

    #[test]
    fn a_name_is_clipped() {
        assert_eq!(clip("Example", 3), "Exa…");
        assert_eq!(clip("Ex", 3), "Ex");
    }
}
