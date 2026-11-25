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
    // A date or time input, which `type` fills segment by segment (#4281): DateTime is
    // datetime-local's, month's and week's.
    "Date",
    "DateTime",
    "InputTime",
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
/// The outline's characters at most, leaving room under the answer's cap (`tools::MAX_ANSWER`)
/// for what the action did and the outline's notes.
pub const MAX_OUTLINE: usize = 10_000;
/// The characters of a text's line the outline shows: a paragraph's start, as read_page has the
/// page's text whole.
const TEXT_SHOWN: usize = 300;
/// A text this long in one piece, on a page with next to nothing to act on, is a response the tab
/// shows, such as JSON, XML or plain text, and not a page's text.
const RESPONSE_TEXT: usize = 4_000;
/// The characters of a response its outline shows.
const RESPONSE_SHOWN: usize = 1_000;

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
    /// The DOM node of the form it's in, if it's in one.
    pub form: Option<i64>,
}

/// What has the keyboard in a page, as its outline says.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Focus {
    /// The page itself, with no element of it.
    Page,
    /// An element with a ref.
    On(Element),
    /// The outline doesn't say, or it's on an element without a ref.
    #[default]
    Unknown,
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

    /// Forgets the refs named after the `last`th, as an outline that named them won't show them.
    fn forget_after(&mut self, last: u32) {
        self.by_node.retain(|_, number| *number <= last);
        self.by_ref.retain(|number, _| *number <= last);
        self.next = last;
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
        form: None,
        focus: Focus::Unknown,
        elements: Vec::new(),
        messages: Vec::new(),
        editors: Vec::new(),
    };
    let backend = |node: &&Value| node.get("backendDOMNodeId").and_then(Value::as_i64);
    let root = match part {
        Some(part) => nodes.iter().find(|node| backend(node) == Some(part)),
        None => nodes.first(),
    };
    let mut notes = Vec::new();
    match (part, response(nodes)) {
        (None, Some(text)) => {
            walk.add(Line {
                text: format!("- text: {}", clip(&text, RESPONSE_SHOWN)),
                ..Line::default()
            });
            notes.push(RESPONSE);
        }
        _ => {
            if let Some(root) = root {
                walk.node(root, 0, "", 0);
            }
        }
    }
    if root.is_none() && part.is_some() {
        notes.push("That element isn't on the page any more. Take a snapshot without a ref.");
    }
    if walk.size > MAX_OUTLINE {
        notes.push(CUT);
    }
    if walk.password {
        notes.push(PASSWORD);
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
        focus: walk.focus,
        elements: walk.elements,
        messages: walk.messages,
        editors: walk.editors,
    }
}

/// What an outline too long for its answer says where it's cut.
pub const CUT: &str = "The outline is cut here. snapshot with the ref of a part, such as a list or \
     the main region, outlines that part alone, and read_page reads the page's text.";
/// What an action's answer says when most of the outline changed, on the same page.
pub const MOSTLY_CHANGED: &str = "Most of the outline's elements are new, though it reads much as \
     before, as when a page draws a table again: snapshot gives it whole, with their refs.";
/// What the outline of a response says, below its start.
pub const RESPONSE: &str = "This tab shows a response, such as JSON, XML or plain text, not a page: \
     above is its start. read_page reads it in parts, and request fetches it from the site with \
     its status and type.";

/// The text of a response the tab shows (`RESPONSE_TEXT`), if it shows one.
fn response(nodes: &[Value]) -> Option<String> {
    let acts = (nodes.iter())
        .filter(|node| INTERACTIVE.contains(&role(node)))
        .count();
    if acts > 2 {
        return None;
    }
    let long = (nodes.iter())
        .filter(|node| role(node) == "StaticText")
        .find(|node| name(node).chars().count() > RESPONSE_TEXT)?;
    // A large body can reach the page as several text nodes, by how its bytes arrived, so its text
    // is its parent's text nodes together, in order, from the first (#3827). They're joined before
    // their white space is made one space, which keeps a space where one was split.
    let id = long.get("nodeId").and_then(Value::as_str);
    let parent = nodes
        .iter()
        .find(|node| child_ids(node).any(|child| Some(child) == id));
    let Some(parent) = parent else {
        return Some(name(long));
    };
    let texts: HashMap<&str, &Value> = (nodes.iter())
        .filter(|node| role(node) == "StaticText")
        .filter_map(|node| Some((node.get("nodeId")?.as_str()?, node)))
        .collect();
    let raw = child_ids(parent)
        .filter_map(|child| texts.get(child))
        .filter_map(|node| node.pointer("/name/value").and_then(Value::as_str))
        .collect::<String>();
    Some(raw.split_whitespace().collect::<Vec<_>>().join(" "))
}
/// What an outline of a page with a password field says of it.
const PASSWORD: &str = "This page has a password field, which is off limits to you: signing in is \
                        the user's to do. Hand them the tab with hand_off.";

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
    /// What has the keyboard, and each element with a ref, for what a key press activates.
    focus: Focus,
    elements: Vec<Element>,
    /// The form of each message the page holds, a multi-line field or a rich-text editor that
    /// isn't empty, or none for one in no form: what a Send control would send.
    messages: Vec<Option<i64>>,
    /// The DOM node of each rich-text editor, where a line break may send what it holds.
    editors: Vec<i64>,
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
            changes.push(format!("was {}", quoted(&was.name, 60)));
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
    /// Whether a Send control in `form`, or in no form, would send a message the page holds: one
    /// in that form or in no form, or, for a control in no form, anywhere on the page, as a
    /// composer's Send often sits outside every form.
    pub fn sends_message(&self, form: Option<i64>) -> bool {
        (self.messages.iter()).any(|each| form.is_none() || each.is_none() || *each == form)
    }

    /// Whether the element on the DOM node `node` is a rich-text editor.
    pub fn is_editor(&self, node: i64) -> bool {
        self.editors.contains(&node)
    }

    /// Whether it's of the page at `url` and that page has a password field: a sign-in page, or one
    /// with a sign-in form.
    pub fn signs_in_at(&self, url: &str) -> bool {
        self.page == url.split('#').next().unwrap_or_default() && self.notes.contains(&PASSWORD)
    }

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
    /// new page is answered with the outline whole. Changes of more than half the outline are
    /// answered with a line saying so, as sending the page again would cost the agent as much as
    /// a snapshot it may not need (#3657). With it, whether the agent has this outline now.
    pub fn since(&self, last: &Outline) -> (String, bool) {
        if (self.page.as_str(), self.document) != (last.page.as_str(), last.document) {
            return (self.text(), true);
        }
        let Some(same) = same_lines(&last.lines, &self.lines) else {
            return (self.text(), true);
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
            let text = format!("{}\n\nNothing in the outline changed.", self.head);
            return (self.with_notes(text), true);
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
            // A view the page put in place of the last is new to the agent, and its outline
            // whole; the same one drawn again, most of its lines new elements that read as the
            // ones gone, is what the agent had. A list that closed leaves no new lines.
            let added: Vec<&Line> = (self.lines.iter().enumerate())
                .filter_map(|(at, line)| same[at].is_none().then_some(line))
                .collect();
            let gone: Vec<&Line> = (last.lines.iter().enumerate())
                .filter_map(|(at, line)| (!kept.contains(&at)).then_some(line))
                .collect();
            let redrawn = 2 * added.len() > self.lines.len() && reads_as(&gone, &added);
            return match redrawn {
                true => (
                    self.with_notes(format!("{}\n\n{MOSTLY_CHANGED}", self.head)),
                    false,
                ),
                false => (self.text(), true),
            };
        }
        let since = "What changed since the last outline you had of this tab:";
        (
            self.with_notes(format!("{}\n\n{since}\n\n{changes}", self.head)),
            true,
        )
    }

    fn with_notes(&self, mut text: String) -> String {
        if !self.notes.is_empty() {
            text.push_str("\n\n");
            text.push_str(&self.notes.join("\n"));
        }
        text
    }
}

/// Whether most of `new`'s lines read as lines of `old` do, their refs aside: the same view drawn
/// again, as a grid redraws its rows after a click.
fn reads_as(old: &[&Line], new: &[&Line]) -> bool {
    let unreferenced = |line: &Line| {
        let mut text = String::with_capacity(line.text.len());
        let mut rest = line.text.as_str();
        while let Some(at) = rest.find(" [e") {
            text.push_str(&rest[..at]);
            rest = &rest[at + 3..];
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            match rest[digits..].strip_prefix(']') {
                Some(after) if digits > 0 => rest = after,
                _ => text.push_str(" [e"),
            }
        }
        text.push_str(rest);
        text
    };
    let mut had: HashMap<String, usize> = HashMap::new();
    for line in old {
        *had.entry(unreferenced(line)).or_default() += 1;
    }
    let same = (new.iter())
        .filter(|line| match had.get_mut(&unreferenced(line)) {
            Some(left) if *left > 0 => {
                *left -= 1;
                true
            }
            _ => false,
        })
        .count();
    2 * same > new.len()
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
    /// The outline's characters so far, each line's indent and break too.
    size: usize,
    password: bool,
    captcha: bool,
    /// The form the walk is in, and what it found has the keyboard and can be acted on.
    form: Option<i64>,
    focus: Focus,
    elements: Vec<Element>,
    messages: Vec<Option<i64>>,
    editors: Vec<i64>,
}

impl<'a> Walk<'a> {
    fn add(&mut self, line: Line) {
        if self.size > MAX_OUTLINE {
            return;
        }
        self.size += 2 * line.depth + line.text.chars().count() + 1;
        self.lines.push(line);
    }

    /// Whether `name` asks the user to prove they're human.
    fn looks_for_captcha(&mut self, name: &str) {
        let lower = name.to_lowercase();
        let human = ["captcha", "not a robot", "verify you are human"];
        if human.iter().any(|words| lower.contains(words)) {
            self.captcha = true;
        }
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
        for child in self.children(node) {
            let each = self.text_of(child, levels + 1)?;
            if !each.is_empty() {
                text.push(each);
            }
        }
        Some(text.join(" "))
    }

    fn children(&self, node: &Value) -> Vec<&'a Value> {
        let children = node.get("childIds").and_then(Value::as_array);
        (children.into_iter().flatten())
            .filter_map(|child| child.as_str().and_then(|id| self.by_id.get(id).copied()))
            .collect()
    }

    /// An element's line without its indent or dash, as `button "Pay" [e4] (disabled)`, with
    /// its ref, which it names in the tab's refs if it's one an agent acts on or scrolls in, and
    /// its states. A row's line gives its name in its parts instead (`row`).
    fn element(
        &mut self,
        node: &Value,
        role: &str,
        name: &str,
        named: bool,
    ) -> (String, Option<u32>, Vec<String>) {
        let mut line = role.to_string();
        let mut reference = None;
        if named && !name.is_empty() {
            line.push_str(&format!(" {}", quoted(name, 160)));
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
                name: name.into(),
                password,
                form: self.form,
            };
            if state(node, "focused") {
                self.focus = Focus::On(element.clone());
            }
            self.elements.push(element.clone());
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
        // What a field holds, where the core can name its source as the agent's own typing,
        // unchanged, or the page's own value, and else only whether it holds anything: never
        // the user's typing, autofill's or a password manager's fill, or a password's, a card's
        // or a one-time code's contents, as in the agent's pictures (#2355).
        if let Some(holds) = node.get("holds").and_then(Value::as_str) {
            let holds = holds.split_whitespace().collect::<Vec<_>>().join(" ");
            line.push_str(&format!(" (holds {})", quoted(&holds, HOLDS_SHOWN)));
        }
        match node.get("contents").and_then(Value::as_str) {
            Some("empty") => line.push_str(" (empty)"),
            Some("filled") => line.push_str(" (filled)"),
            Some("changed") => line.push_str(" (filled, not as you typed it)"),
            _ => {}
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
        (line, reference, states)
    }

    /// A row of a table or an item of a list as one line, its parts in order: each cell's text and
    /// ref, each element's line and the text between them, so a long table or list fits the
    /// outline with its refs instead of being cut above them (#3657). None when it holds what a
    /// line can't: a list, a table or a landmark of its own, or more than `ROW_PARTS` parts.
    fn row(
        &mut self,
        node: &Value,
        depth: usize,
        role: &str,
        name: &str,
        levels: usize,
    ) -> Option<Line> {
        let (line, reference, _) = self.element(node, role, name, false);
        let mut line = format!("- {line}");
        let mut parts = Vec::new();
        self.parts(node, "", &mut parts, levels)?;
        // A link alone, as a menu's item holds, is its own line, shorter without the item's.
        if parts.is_empty() || (parts.len() == 1 && !parts[0].plain) {
            return None;
        }
        match parts.iter().all(|part| part.plain) {
            // Text alone, as a list of facts has it: the item named by it, as before.
            true => {
                let text: Vec<&str> = parts.iter().map(|part| part.text.as_str()).collect();
                let text = quoted(&text.join(" "), 160);
                line = match reference {
                    Some(number) => format!("- {role} {text} [e{number}]"),
                    None => format!("- {role} {text}"),
                };
            }
            false => {
                let parts: Vec<String> = (parts.iter())
                    .map(|part| match part.plain {
                        // Cut already, in `parts`.
                        true => quoted(&part.text, usize::MAX),
                        false => part.text.clone(),
                    })
                    .collect();
                line.push_str(": ");
                line.push_str(&parts.join(" · "));
            }
        }
        Some(Line {
            depth,
            text: line,
            element: reference,
            ..Line::default()
        })
    }

    /// The parts of what `node` holds, for `row`, or None if a line can't hold them.
    fn parts(
        &mut self,
        node: &Value,
        parent_name: &str,
        parts: &mut Vec<Part>,
        levels: usize,
    ) -> Option<()> {
        for child in self.children(node) {
            if levels > 200 || parts.len() > ROW_PARTS {
                return None;
            }
            let role = role(child);
            if NESTED.contains(&role) {
                return None;
            }
            let ignored = (child.get("ignored").and_then(Value::as_bool)).unwrap_or(false);
            let name = name(child);
            self.looks_for_captcha(&name);
            // As node() does: an element with a ref then says it has the keyboard (`element`).
            if state(child, "focused") {
                self.focus = Focus::Unknown;
            }
            if ignored || role == "ListMarker" {
                continue;
            }
            if role == "StaticText" {
                let repeated = parent_name.contains(&name) || self.field_names.contains(&name);
                if !name.is_empty() && !repeated {
                    let text = clip(&name, ROW_TEXT_SHOWN);
                    parts.push(Part { text, plain: true });
                }
                continue;
            }
            let cell = ROW_CELLS.contains(&role);
            let acts = INTERACTIVE.contains(&role);
            // A cell with something to act on in it is acted on by that.
            let cell = cell && !name.is_empty() && self.text_of(child, 0).is_some();
            if acts || (cell && child.get("backendDOMNodeId").is_some()) {
                let (text, _, _) = self.element(child, role, &name, true);
                let text = match cell {
                    true => text
                        .strip_prefix(role)
                        .unwrap_or(&text)
                        .trim_start()
                        .to_string(),
                    false => text,
                };
                parts.push(Part { text, plain: false });
                self.parts(child, &name, parts, levels + 1)?;
                continue;
            }
            // Its text is a part: a heading's, a paragraph's, a cell's without a ref.
            self.parts(child, parent_name, parts, levels + 1)?;
        }
        Some(())
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
        let rich = editable(node) == Some("richtext");
        if let (false, true, Some(backend)) = (ignored, rich, node.get("backendDOMNodeId")) {
            self.editors.extend(backend.as_i64());
        }
        if !ignored && (rich || (role == "textbox" && state(node, "multiline"))) && holds(node) {
            self.messages.push(self.form);
        }
        if state(node, "focused") {
            self.focus = match role {
                "RootWebArea" => Focus::Page,
                _ => Focus::Unknown,
            };
        }
        self.looks_for_captcha(&name);
        // A list's bullet or number, which its indent already says.
        if role == "ListMarker" {
            return;
        }
        if !ignored && role == "StaticText" {
            let repeated = parent_name.contains(&name) || self.field_names.contains(&name);
            if !name.is_empty() && !repeated {
                self.add(Line {
                    depth,
                    text: format!("- text: {}", clip(&name, TEXT_SHOWN)),
                    ..Line::default()
                });
            }
            return;
        }
        // A row or a list item that a line can hold is one line; else it's outlined line by line.
        if !ignored && ROWS.contains(&role) {
            let last = (self.refs.next, self.elements.len());
            match self.row(node, depth, role, &name, levels) {
                Some(line) => return self.add(line),
                // Its refs and elements are named again as its lines show them.
                None => {
                    self.refs.forget_after(last.0);
                    self.elements.truncate(last.1);
                }
            }
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
            let (text, reference, states) = self.element(node, role, &name, true);
            let before = (self.lines.len(), self.size);
            self.add(Line {
                depth,
                text: format!("- {text}"),
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
        let outer = self.form;
        if role == "form" {
            self.form = node
                .get("backendDOMNodeId")
                .and_then(Value::as_i64)
                .or(outer);
        }
        for child in self.children(node) {
            self.node(child, next, &next_name, levels + 1);
        }
        self.form = outer;
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

/// How the node is editable, as Chromium says: "plaintext" or "richtext", the latter for a
/// contenteditable editor.
fn editable(node: &Value) -> Option<&str> {
    let properties = node.get("properties").and_then(Value::as_array);
    let property = (properties.into_iter().flatten())
        .find(|property| property.get("name").and_then(Value::as_str) == Some("editable"))?;
    property.pointer("/value/value").and_then(Value::as_str)
}

/// Whether a field holds anything, as the core says: the agent's own typing or the page's value,
/// or that it's filled.
fn holds(node: &Value) -> bool {
    let shown = node.get("holds").and_then(Value::as_str);
    let contents = node.get("contents").and_then(Value::as_str);
    shown.is_some_and(|holds| !holds.trim().is_empty())
        || matches!(contents, Some("filled" | "changed"))
}

/// A part of a row's line (`Walk::row`): text, or a cell's or an element's line.
struct Part {
    text: String,
    plain: bool,
}

/// What `Walk::row` puts on one line.
const ROWS: &[&str] = &["row", "listitem"];
/// The cells a row's line gives with their refs, by their text.
const ROW_CELLS: &[&str] = &["cell", "gridcell", "rowheader", "columnheader"];
/// What a row's line can't hold, as it has lines of its own.
const NESTED: &[&str] = &[
    "row",
    "listitem",
    "list",
    "table",
    "grid",
    "treegrid",
    "tree",
    "form",
    "dialog",
    "alertdialog",
    "region",
    "main",
    "navigation",
    "complementary",
    "article",
    "feed",
    "tabpanel",
    "Iframe",
    "IframePresentational",
];
/// The parts a row's line holds at most, as a wider row reads better line by line.
const ROW_PARTS: usize = 30;
/// The characters of a text a row's line shows.
const ROW_TEXT_SHOWN: usize = 100;

/// Whether the node has the state `name`, such as "focused".
fn state(node: &Value, name: &str) -> bool {
    let properties = node.get("properties").and_then(Value::as_array);
    properties.into_iter().flatten().any(|property| {
        property.get("name").and_then(Value::as_str) == Some(name)
            && match property.pointer("/value/value") {
                Some(Value::Bool(value)) => *value,
                Some(Value::String(value)) => value == "true",
                _ => false,
            }
    })
}

fn child_ids(node: &Value) -> impl Iterator<Item = &str> {
    (node.get("childIds").and_then(Value::as_array).into_iter())
        .flatten()
        .filter_map(Value::as_str)
}

fn role(node: &Value) -> &str {
    node.pointer("/role/value")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// The node's name, its runs of white space one space. White space is Unicode's, a no-break
/// space's too, and the core's `select` matches names by the same rule (`OneSpaced` in
/// core/ax_text.cc), so a name shown here is one it finds (#3190).
fn name(node: &Value) -> String {
    let name = node
        .pointer("/name/value")
        .and_then(Value::as_str)
        .unwrap_or_default();
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text` as one line of a log, each character that could break it a space, so a line break in
/// what an agent asked, which an error echoes as it was asked, can't make a line of its own.
pub fn one_line(text: &str) -> String {
    text.chars()
        .map(
            |c| match c.is_control() || matches!(c, '\u{2028}' | '\u{2029}') {
                true => ' ',
                false => c,
            },
        )
        .collect()
}

/// `text`, cut at `chars` characters.
pub fn clip(text: &str, chars: usize) -> String {
    match text.char_indices().nth(chars) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.into(),
    }
}

/// The page's `text` in a line's straight quotes, cut at `chars` characters, with its `\` and `"`
/// escaped as a JSON string's are, so the quote ends where the line ends it.
pub fn quoted(text: &str, chars: usize) -> String {
    let text = clip(text, chars);
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `text` in ilo's curly quotes, cut at `chars` characters, as plain text in one paragraph
/// (`request::plain`), so nothing in it can close ilo's quote or move ilo's words after it.
pub fn curly(text: &str, chars: usize) -> String {
    format!("“{}”", clip(&super::request::plain(text), chars))
}

/// How many characters of what a field holds its line shows: enough to check a form's answers,
/// while a long text's line stays a line.
const HOLDS_SHOWN: usize = 200;

/// How many of a select's choices its line names: enough for any form's, while a country list's
/// says how many more there are, which `select` finds by name.
const CHOICES_NAMED: usize = 40;

/// A select's choices as its line names them, each quoted.
fn listed(choices: &[&str]) -> String {
    let mut named: Vec<_> = (choices.iter().take(CHOICES_NAMED))
        .map(|choice| quoted(choice, 80))
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
    "complete order",
    "confirm order",
    "confirm purchase",
    "confirm and pay",
    "confirm booking",
    "book now",
    "reserve",
    "bid",
    "pledge",
    // Danish.
    "betal",
    "køb",
    "bestil",
    "bekræft ordre",
    "støt",
    "donér",
    // German.
    "kaufen",
    "bezahlen",
    "bestellen",
    "buchen",
    "spenden",
    // French.
    "payer",
    "acheter",
    "commander",
    "faire un don",
    // Spanish.
    "pagar",
    "comprar",
    "donar",
    // Dutch, whose "bestellen" is German's too.
    "betalen",
    "doneren",
    // Swedish.
    "betala",
    "köp",
    "beställ",
    "skänk",
];
/// Words that send, as a button's name has them.
const SEND: &[&str] = &[
    "send",
    "submit",
    "post",
    "publish",
    "reply",
    "tweet",
    "comment",
    "forward",
    "share",
    "sende",
    "senden",
    "antworten",
    "envoyer",
    "enviar",
    "verzenden",
    "skicka",
    "create topic",
];
/// Words that move money, as a button's name has them, which ask on a bank's or money site
/// alone: a yes to opening one isn't a yes to what's done there.
const MOVES: &[&str] = &[
    "withdraw",
    "transfer",
    "confirm",
    // Danish.
    "hæv",
    "overfør",
    "bekræft",
    // German.
    "abheben",
    "überweisen",
    "bestätigen",
    // French.
    "retirer",
    "virement",
    "transférer",
    "confirmer",
    // Spanish.
    "retirar",
    "transferir",
    "confirmar",
    // Dutch.
    "opnemen",
    "overboeken",
    "overmaken",
    "bevestigen",
    // Swedish.
    "ta ut",
    "överför",
    "bekräfta",
];
/// Sites that send messages, where a Send control asks even with no message the outline shows:
/// one composed where the outline doesn't see it, as in a frame of its own.
const MESSAGING: &[&str] = &[
    "mail.proton.me",
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
/// Banks' and money sites without "bank" in their host, each a site whole, as each country's
/// site is one of its own.
const BANKS: &[&str] = &[
    "chase.com",
    "wellsfargo.com",
    "bankofamerica.com",
    "citi.com",
    "capitalone.com",
    "schwab.com",
    "fidelity.com",
    "vanguard.com",
    "paypal.com",
    "venmo.com",
    "revolut.com",
    "wise.com",
    "monzo.com",
    "coinbase.com",
    "americanexpress.com",
    "discover.com",
    "pnc.com",
    "td.com",
    "ally.com",
    "sofi.com",
    "robinhood.com",
    "etoro.com",
    "interactivebrokers.com",
    "binance.com",
    "kraken.com",
    "bitstamp.net",
    "crypto.com",
    "klarna.com",
    "n26.com",
    "bunq.com",
    "hsbc.com",
    "hsbc.co.uk",
    "barclays.co.uk",
    "barclaycard.co.uk",
    "natwest.com",
    "nationwide.co.uk",
    "halifax.co.uk",
    "santander.co.uk",
    "santander.de",
    "santander.es",
    "ing.com",
    "ing.de",
    "ing.nl",
    "sparkasse.de",
    "dkb.de",
    "comdirect.de",
    "postfinance.ch",
    "ubs.com",
    "nordea.com",
    "nordea.dk",
    "nordea.se",
    "nordea.no",
    "nordea.fi",
    "nykredit.dk",
    "lsb.dk",
    "lunar.app",
    "mobilepay.dk",
    "vipps.no",
];

/// Why acting on the element named `label` on `host` asks the user first, if it does: it looks
/// like a payment or an order, or it sends a message or post, from a site that sends them or
/// with a `message` in its form, or the page's for one in no form, for it to send, or it sends or
/// moves money on a bank's or money site (GHSA-6qxg-jpw9-5jrr). Typing with submit and pressing
/// Enter are "send".
pub fn risk(host: &str, label: &str, message: bool) -> Option<&'static str> {
    let label = label.to_lowercase();
    if PAYMENT.iter().any(|words| has_words(&label, words)) {
        return Some(ORDER);
    }
    let sends = SEND.iter().any(|words| has_words(&label, words));
    let moves = || MOVES.iter().any(|words| has_words(&label, words));
    if (sends || moves()) && opening_risk(host).is_some() {
        return Some(ORDER);
    }
    if sends && (message || MESSAGING.iter().any(|site| on(host, site))) {
        return Some(SENDS);
    }
    None
}

/// Roles a key types in, where Space is a space and Enter submits the form.
const FIELDS: &[&str] = &["textbox", "searchbox", "combobox", "spinbutton"];

/// Roles Enter clicks, as Chromium names them: a summary is a "DisclosureTriangle". Enter on any
/// other control, such as a checkbox, radio or slider, submits its form as Enter in a field does.
const ENTER_CLICKS: &[&str] = &[
    "button",
    "link",
    "menuitem",
    "menuitemcheckbox",
    "menuitemradio",
    "tab",
    "DisclosureTriangle",
];

/// A key press that asks first, as a click on `control` would, for `why`. `surely` when the press
/// is a click on it, and not when it's one of the controls the press may click.
#[derive(Debug, PartialEq)]
pub struct Pressing {
    pub why: &'static str,
    pub control: Element,
    pub surely: bool,
}

/// Whether pressing `key` asks the user first, as a click would on what it activates. Space
/// activates the control that has the keyboard, and Enter a button, link, menu item, tab or
/// summary, as a click on it does. Enter elsewhere, in a field (`typed_in` or the one with the
/// keyboard) or on another control, submits its form, as a click on its submit button does. The
/// outline doesn't say which button that is, nor whether one outside the form belongs to it, so
/// any control of the form or outside every form that would ask asks, or of the page when the
/// control has no form or the outline doesn't say what has the keyboard (GHSA-hqw3-jp9p-3rhj).
pub fn pressing_risk(
    host: &str,
    outline: Option<&Outline>,
    key: &str,
    typed_in: Option<&Element>,
) -> Option<Pressing> {
    if key != "Enter" && key != "Space" {
        return None;
    }
    let asks = |surely: bool| {
        move |element: &Element| {
            let message = outline.is_some_and(|outline| outline.sends_message(element.form));
            let why = risk(host, &element.name, message)?;
            let control = element.clone();
            Some(Pressing {
                why,
                control,
                surely,
            })
        }
    };
    let focus = match (typed_in, outline) {
        (Some(field), _) => Focus::On(field.clone()),
        (None, Some(outline)) => outline.focus.clone(),
        (None, None) => Focus::Unknown,
    };
    let form = match &focus {
        Focus::Page => return None,
        Focus::On(control) => {
            let role = control.role.as_str();
            let clicks = match key {
                "Space" => !FIELDS.contains(&role),
                _ => ENTER_CLICKS.contains(&role),
            };
            match (clicks, key) {
                (true, _) => return asks(true)(control),
                // Space in a field types a space.
                (false, "Space") => return None,
                (false, _) => control.form,
            }
        }
        Focus::Unknown => None,
    };
    let elements = outline.map(|outline| outline.elements.as_slice());
    let mut elements = elements.unwrap_or_default().iter();
    match form {
        Some(form) => (elements.filter(|each| each.form.is_none_or(|each| each == form)))
            .find_map(asks(false)),
        None => elements.find_map(asks(false)),
    }
}

/// Why a payment or an order asks first, which a session's receipt counts as an order placed.
pub const ORDER: &str = "It looks like a payment or an order.";
/// Why sending a message or a post asks first, which the receipt counts as a message sent.
pub const SENDS: &str = "It sends a message or a post.";

/// Why opening a page on `host` asks the user first, if it does: it's a bank's.
pub fn opening_risk(host: &str) -> Option<&'static str> {
    let host = host.to_lowercase();
    let bank = BANKS.iter().any(|bank| on(&host, bank)) || host.contains("bank");
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

    /// Unicode's white space, each code point the core's `OneSpaced` collapses: its test
    /// (`ANameIsOneSpacedAsTheOutlineShowsItWhateverItsSpaces` in core/ax_text_unittest.cc) has
    /// the same list, so the outline and `select` can't part on a name again (#3190).
    const UNICODE_WHITE_SPACE: [char; 25] = [
        '\u{0009}', '\u{000A}', '\u{000B}', '\u{000C}', '\u{000D}', '\u{0020}', '\u{0085}',
        '\u{00A0}', '\u{1680}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}',
        '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}', '\u{200A}', '\u{2028}',
        '\u{2029}', '\u{202F}', '\u{205F}', '\u{3000}',
    ];

    /// An error that echoes a request with line breaks in it is still one line of the log, and
    /// what isn't a break stays as it was asked.
    #[test]
    fn a_line_break_in_what_was_asked_makes_no_line_of_the_log() {
        let asked = "There's no option “a\nbrowser: agent page pressed in 5\r\u{85}\u{2028}\u{2029}\u{b}\u{c}b  c\u{a0}d” there.";
        let line = one_line(asked);
        assert!(
            !line
                .chars()
                .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
        );
        assert_eq!(line.lines().count(), 1);
        assert_eq!(
            line,
            "There's no option “a browser: agent page pressed in 5      b  c\u{a0}d” there."
        );
    }

    /// A name's white space, a no-break space beside a space or any other of Unicode's, is one
    /// space in the outline, as `select` matches it, and what isn't white space stays.
    #[test]
    fn names_lose_unicode_white_space_as_select_does() {
        let named = |text: &str| name(&node("1", "option", text, &[]));
        for space in UNICODE_WHITE_SPACE {
            assert!(space.is_whitespace(), "{space:?} isn't Rust's white space");
            assert_eq!(
                named(&format!("Bravo 5678{space} (closed)")),
                "Bravo 5678 (closed)"
            );
            assert_eq!(named(&format!("Bravo{space}5678")), "Bravo 5678");
            assert_eq!(named(&format!("{space}Bravo{space}{space}")), "Bravo");
        }
        // No other character is: the list is all of Rust's white space, as it's all of Chromium's.
        let others = (0..=0x10FFFF_u32)
            .filter_map(char::from_u32)
            .filter(|c| c.is_whitespace() && !UNICODE_WHITE_SPACE.contains(c));
        assert_eq!(others.collect::<Vec<_>>(), Vec::<char>::new());
        assert_eq!(named("Open\r\n  now"), "Open now");
        assert_eq!(named("a\u{200B}b\u{2060}c"), "a\u{200B}b\u{2060}c");
    }

    /// A field's line says what it holds where the core gives it, and else only whether it holds
    /// anything, and whether that's no longer what the agent typed (#2355).
    #[test]
    fn a_fields_line_says_what_it_holds_or_only_whether_its_filled() {
        let mut nodes = vec![
            node("1", "RootWebArea", "Form", &["2", "3", "4", "5", "6", "7"]),
            node("2", "textbox", "Name", &[]),
            node("3", "textbox", "Email", &[]),
            node("4", "textbox", "Password", &[]),
            node("5", "textbox", "Notes", &[]),
            node("6", "textbox", "Phone", &[]),
            node("7", "textbox", "City", &[]),
        ];
        for node in &mut nodes {
            node["backendDOMNodeId"] =
                json!(node["nodeId"].as_str().unwrap().parse::<i64>().unwrap());
        }
        nodes[1]["holds"] = json!("Jane Doe");
        nodes[2]["contents"] = json!("filled");
        nodes[3]["contents"] = json!("filled");
        nodes[3]["password"] = json!(true);
        nodes[4]["holds"] = json!(format!("First line\n\nsecond {}", "x".repeat(300)));
        nodes[5]["contents"] = json!("empty");
        nodes[6]["contents"] = json!("changed");
        let outline = outline(&tab(), &nodes, &mut Refs::default(), None).text();
        for line in [
            "- textbox \"Name\" [e1] (holds \"Jane Doe\")\n",
            "- textbox \"Email\" [e2] (filled)\n",
            "- textbox \"Password\" [e3] (password: off limits) (filled)\n",
            "- textbox \"Phone\" [e5] (empty)\n",
            "- textbox \"City\" [e6] (filled, not as you typed it)",
        ] {
            assert!(outline.contains(line), "{line} in {outline}");
        }
        assert!(
            outline.contains("(holds \"First line second xxx"),
            "{outline}"
        );
        assert!(outline.contains("x…\")\n"), "{outline}");
    }

    /// A date or time input gets a ref, as an agent types into it (#4281). A colour field and an
    /// image, which had none, still have none.
    #[test]
    fn date_and_time_fields_get_refs_and_a_colour_field_still_has_none() {
        let mut nodes = vec![
            node("1", "RootWebArea", "Form", &["2", "3", "4", "5", "6"]),
            node("2", "Date", "Day", &[]),
            node("3", "InputTime", "Start", &[]),
            node("4", "DateTime", "Meeting", &[]),
            node("5", "ColorWell", "Tint", &[]),
            node("6", "image", "Logo", &[]),
        ];
        for node in &mut nodes {
            node["backendDOMNodeId"] =
                json!(node["nodeId"].as_str().unwrap().parse::<i64>().unwrap());
        }
        nodes[1]["contents"] = json!("empty");
        nodes[4]["contents"] = json!("filled");
        let outline = outline(&tab(), &nodes, &mut Refs::default(), None).text();
        for line in [
            "- Date \"Day\" [e1] (empty)\n",
            "- InputTime \"Start\" [e2]\n",
            "- DateTime \"Meeting\" [e3]\n",
            "- ColorWell \"Tint\" (filled)\n",
            "- image \"Logo\"",
        ] {
            assert!(outline.contains(line), "{line} in {outline}");
        }
        assert!(!outline.contains("[e4]"), "{outline}");
    }

    /// The page's text inside a line's quotes, a name, a select's choices or what a field holds,
    /// has its `"` and `\` escaped, so the quote ends where the line ends it and nothing the page
    /// wrote reads as the line's own ref, state or choice. A plain name's line is as it was.
    #[test]
    fn the_pages_text_in_a_lines_quotes_is_escaped() {
        let mut nodes = vec![
            node("1", "RootWebArea", "Pay", &["2", "3", "4", "5"]),
            node("2", "button", "Cancel\" [e2] (focused) \"x", &[]),
            node("3", "button", "Pay now", &[]),
            node("4", "combobox", "Size", &[]),
            node("5", "textbox", "Note", &[]),
        ];
        for node in &mut nodes {
            node["backendDOMNodeId"] =
                json!(node["nodeId"].as_str().unwrap().parse::<i64>().unwrap());
        }
        nodes[3]["choices"] = json!(["S\", \"XL", "M\\"]);
        nodes[4]["holds"] = json!("a\") (empty) (\"b");
        let outline = outline(&tab(), &nodes, &mut Refs::default(), None).text();
        for line in [
            "- button \"Cancel\\\" [e2] (focused) \\\"x\" [e1]\n",
            "- button \"Pay now\" [e2]\n",
            "- combobox \"Size\" [e3] (options: \"S\\\", \\\"XL\", \"M\\\\\")\n",
            "- textbox \"Note\" [e4] (holds \"a\\\") (empty) (\\\"b\")",
        ] {
            assert!(outline.contains(line), "{line} in {outline}");
        }
        assert!(!outline.contains("\"Cancel\" [e2]"), "{outline}");
        assert!(!outline.contains("\"S\", \"XL\""), "{outline}");
    }

    /// A row's text, in a line of its own or among its parts, is quoted as a name is, so the
    /// page's `"` can't end the quote and write a ref or a part of its own.
    #[test]
    fn a_rows_text_in_its_quotes_is_escaped() {
        let mut nodes = vec![
            node("1", "RootWebArea", "Basket", &["2"]),
            node("2", "list", "", &["3", "4"]),
            node("3", "listitem", "", &["31"]),
            node("31", "StaticText", "One\" [e9] \"x", &[]),
            node("4", "listitem", "", &["41", "42"]),
            node("41", "StaticText", "Two\" · button \"Pay", &[]),
            node("42", "button", "Pay now", &[]),
        ];
        for node in &mut nodes {
            node["backendDOMNodeId"] =
                json!(node["nodeId"].as_str().unwrap().parse::<i64>().unwrap());
        }
        let outline = outline(&tab(), &nodes, &mut Refs::default(), None).text();
        for line in [
            "- listitem \"One\\\" [e9] \\\"x\"",
            "- listitem [e2]: \"Two\\\" · button \\\"Pay\" · button \"Pay now\" [e3]",
        ] {
            assert!(outline.contains(line), "{line} in {outline}");
        }
        assert!(!outline.contains("\"One\" [e9]"), "{outline}");
    }

    /// An element's old name, where its line says it changed, is quoted as its name is, with its
    /// `"` escaped, so it can't end the quote and write a state of its own.
    #[test]
    fn an_old_names_quotes_are_escaped_as_its_names_are() {
        let named = |name: &str| {
            let mut nodes = vec![
                node("1", "RootWebArea", "Form", &["2", "3", "4"]),
                node("2", "link", name, &[]),
                node("3", "button", "Back", &[]),
                node("4", "button", "Next", &[]),
            ];
            for node in &mut nodes {
                node["backendDOMNodeId"] =
                    json!(node["nodeId"].as_str().unwrap().parse::<i64>().unwrap());
            }
            nodes
        };
        let mut refs = Refs::default();
        let before = outline(&tab(), &named("Help\" now enabled \"x"), &mut refs, None);
        let after = outline(&tab(), &named("Help"), &mut refs, None);
        let (text, _) = after.since(&before);
        assert!(
            text.ends_with("- link \"Help\" [e1]: was \"Help\\\" now enabled \\\"x\""),
            "{text}"
        );
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
            open.since(&closed).0,
            format!(
                "{head}{since}New:\n\
                 - listbox \"Size\" [e8]\n  - option \"S\" [e9]\n  - option \"M\" [e10]\n\n\
                 Changed:\n- combobox \"Size\" [e6] (expanded): now expanded"
            )
        );
        assert_eq!(
            closed.since(&open).0,
            format!(
                "{head}{since}Changed:\n- combobox \"Size\" [e6]: now collapsed\n\n\
                 Gone:\n- listbox \"Size\" [e8]\n  - option \"S\" [e9]\n  - option \"M\" [e10]"
            )
        );
        assert_eq!(
            open.since(&open).0,
            format!("{head}Nothing in the outline changed.")
        );

        // Elements in another order are the same elements.
        let mut moved = shop(false);
        moved[1]["childIds"] = json!(["21", "23", "22", "24"]);
        let moved = outline(&tab(), &moved, &mut refs, None);
        assert_eq!(
            moved.since(&closed).0,
            format!("{head}Nothing in the outline changed.")
        );
        // Text is the same line by its text, in order.
        let mut text = shop(false);
        text[8]["name"]["value"] = json!("Heavy linen.");
        let text = outline(&tab(), &text, &mut refs, None);
        assert!(text.since(&closed).0.ends_with(
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
            after.since(&before).0.ends_with(
                "Changed:\n\
                 - textbox \"Name\" [e1] (read-only): now read-only\n\
                 - button \"Continue\" [e2]: now enabled\n\
                 - link \"Help\" [e3]: was \"Help and contact\""
            ),
            "{}",
            after.since(&before).0
        );
    }

    /// A table's rows and cells and a list's items have refs, as a person may click them, a row's
    /// line with its cells' (#3657); an item holding only text is named by it, and one holding a
    /// link alone leaves the ref to the link.
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
             \x20 - row [e1]: \"web-1\" [e2] · \"Running\" [e3]\n\
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
        assert_eq!(elsewhere.since(&closed).0, elsewhere.text());
        let mut reloaded = shop(false);
        reloaded[0]["backendDOMNodeId"] = json!(100);
        let reloaded = outline(&tab(), &reloaded, &mut refs, None);
        assert_eq!(reloaded.since(&closed).0, reloaded.text());
        let reviews = TabInfo {
            url: "https://shop.example/#reviews".into(),
            ..tab()
        };
        let reviews = outline(&reviews, &shop(false), &mut refs, None);
        assert!(
            reviews
                .since(&closed)
                .0
                .ends_with("Nothing in the outline changed.")
        );

        let mut thanks = shop(false);
        thanks[0]["childIds"] = json!(["3"]);
        thanks[6]["name"]["value"] = json!("Thanks for your order");
        let thanks = outline(&tab(), &thanks, &mut refs, None);
        assert_eq!(thanks.since(&closed).0, thanks.text());
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
        assert!(outline.ends_with(CUT));
    }

    /// A table too big for the outline's elements a line each fits it with a row a line, each
    /// with its refs; a long text gives its start.
    #[test]
    fn a_big_table_fits_with_its_rows_as_lines() {
        let mut nodes = vec![
            node("1", "RootWebArea", "Orders", &["2", "9"]),
            node("9", "StaticText", &"Terms. ".repeat(100), &[]),
        ];
        let rows: Vec<String> = (0..120).map(|row| format!("r{row}")).collect();
        nodes.push(node(
            "2",
            "table",
            "Orders",
            &rows.iter().map(String::as_str).collect::<Vec<_>>(),
        ));
        for (at, row) in rows.iter().enumerate() {
            let (cell, link) = (format!("{row}c"), format!("{row}l"));
            nodes.push(node(row, "row", "", &[&cell, &link]));
            nodes.push(node(
                &cell,
                "cell",
                &format!("Order {at}"),
                &[&format!("{cell}t")],
            ));
            nodes.push(node(
                &format!("{cell}t"),
                "StaticText",
                &format!("Order {at}"),
                &[],
            ));
            nodes.push(node(&link, "link", "Refund", &[]));
        }
        for (at, node) in nodes.iter_mut().enumerate() {
            node["backendDOMNodeId"] = json!(at);
        }
        let mut refs = Refs::default();
        let outline = outline(&tab(), &nodes, &mut refs, None).text();
        assert!(!outline.ends_with(CUT), "{outline}");
        assert!(outline.chars().count() < MAX_OUTLINE);
        let text_line = outline
            .lines()
            .find(|line| line.starts_with("- text: "))
            .unwrap();
        assert!(
            text_line.chars().count() <= "- text: ".len() + TEXT_SHOWN + 1,
            "{text_line}"
        );
        assert!(
            outline.contains("  - row [e358]: \"Order 119\" [e359] · link \"Refund\" [e360]"),
            "{outline}"
        );
        assert_eq!(
            refs.get("e360").map(|element| element.name.as_str()),
            Some("Refund")
        );
    }

    /// A row a line can't hold, as one holding a list, is outlined line by line, its refs
    /// numbered as if it had never been tried as a line; a checkbox's state shows on the row's
    /// line.
    #[test]
    fn a_row_holding_a_list_is_outlined_line_by_line() {
        let mut nodes = vec![
            node("1", "RootWebArea", "Mail", &["2"]),
            node("2", "list", "", &["3", "4"]),
            node("3", "listitem", "", &["31", "32"]),
            node("31", "checkbox", "Select", &[]),
            node("32", "list", "", &["33"]),
            node("33", "listitem", "", &["34"]),
            node("34", "StaticText", "Reply", &[]),
            node("4", "listitem", "", &["41", "42"]),
            node("41", "checkbox", "Select", &[]),
            node("42", "StaticText", "From Ana", &[]),
        ];
        for node in &mut nodes {
            node["backendDOMNodeId"] =
                json!(node["nodeId"].as_str().unwrap().parse::<i64>().unwrap());
        }
        nodes[8]["properties"] = json!([{ "name": "checked", "value": { "value": "true" } }]);
        let outline = outline(&tab(), &nodes, &mut Refs::default(), None).text();
        assert!(
            outline.ends_with(
                "\n\n- checkbox \"Select\" [e1]\n\
                 - listitem \"Reply\" [e2]\n\
                 - listitem [e3]: checkbox \"Select\" [e4] (checked) · \"From Ana\""
            ),
            "{outline}"
        );
    }

    /// A control in a row that's a line has the keyboard as one on a line of its own does, so
    /// Enter on it asks first as a click on it would.
    #[test]
    fn a_row_says_what_in_it_has_the_keyboard() {
        let mut nodes = vec![
            node("1", "RootWebArea", "Basket", &["2"]),
            node("2", "list", "", &["3"]),
            node("3", "listitem", "", &["31", "32"]),
            node("31", "StaticText", "Two shirts", &[]),
            node("32", "button", "Pay now", &[]),
        ];
        for node in &mut nodes {
            node["backendDOMNodeId"] =
                json!(node["nodeId"].as_str().unwrap().parse::<i64>().unwrap());
        }
        nodes[4]["properties"] = json!([{ "name": "focused", "value": { "value": true } }]);
        let outline = outline(&tab(), &nodes, &mut Refs::default(), None);
        let text = outline.text();
        assert!(
            text.ends_with("\n- listitem [e1]: \"Two shirts\" · button \"Pay now\" [e2]"),
            "{text}"
        );
        let pressing = pressing_risk("shop.example", Some(&outline), "Enter", None).unwrap();
        assert_eq!(
            (pressing.control.name.as_str(), pressing.surely),
            ("Pay now", true)
        );
    }

    /// A row holding a form of its own is outlined line by line, as one holding a list is, so its
    /// field and buttons have that form, and Enter in the field asks as a click on its own form's
    /// button would, not on the button of the form around the list.
    #[test]
    fn a_row_holding_a_form_keeps_its_form() {
        let mut nodes = vec![
            node("1", "RootWebArea", "Checkout", &["2"]),
            node("2", "form", "", &["3", "4"]),
            node("3", "button", "Place your order", &[]),
            node("4", "list", "", &["5"]),
            node("5", "listitem", "", &["6"]),
            node("6", "form", "", &["7", "8"]),
            node("7", "textbox", "Card number", &[]),
            node("8", "button", "Pay now", &[]),
        ];
        for node in &mut nodes {
            node["backendDOMNodeId"] =
                json!(node["nodeId"].as_str().unwrap().parse::<i64>().unwrap());
        }
        nodes[6]["properties"] = json!([{ "name": "focused", "value": { "value": true } }]);
        let outline = outline(&tab(), &nodes, &mut Refs::default(), None);
        let text = outline.text();
        assert!(
            text.contains("\n    - textbox \"Card number\" [e"),
            "{text}"
        );
        let pressing = pressing_risk("shop.example", Some(&outline), "Enter", None).unwrap();
        // Enter in a field submits its form, which may click this button: the outline can't say.
        assert_eq!(
            (
                pressing.control.name.as_str(),
                pressing.control.form,
                pressing.surely
            ),
            ("Pay now", Some(6), false)
        );
    }

    /// A tab showing a response, not a page, gives its start and says how to read the rest.
    #[test]
    fn a_response_gives_its_start() {
        let json = format!("{{\"items\": [{}]}}", "{\"id\": 1}, ".repeat(1_000));
        let nodes = vec![
            node("1", "RootWebArea", "", &["2"]),
            node("2", "generic", "", &["3"]),
            node("3", "StaticText", &json, &[]),
        ];
        let outline = outline(&tab(), &nodes, &mut Refs::default(), None).text();
        let start: String = json.chars().take(RESPONSE_SHOWN).collect();
        assert!(outline.contains(&format!("- text: {start}")), "{outline}");
        assert!(outline.ends_with(RESPONSE), "{outline}");
        assert!(outline.chars().count() < RESPONSE_SHOWN + 500);
        // A page with its own text is a page.
        let page = vec![
            node("1", "RootWebArea", "", &["2", "3", "4", "5"]),
            node("2", "StaticText", &json, &[]),
            node("3", "link", "Home", &[]),
            node("4", "link", "Docs", &[]),
            node("5", "button", "Sign in", &[]),
        ];
        assert!(!outline_of(&page).contains(RESPONSE));
    }

    /// A large response can reach the page as several text nodes, by how its bytes arrived, its
    /// first shorter than a response's: it still reads from its start, with the space where it
    /// was split (#3827).
    #[test]
    fn a_response_in_several_text_nodes_reads_from_its_start() {
        let json = format!("{{\"items\": [{}]}}", "{\"id\": 1}, ".repeat(1_000));
        // The first ends at a comma, before its space.
        let (first, rest) = json.split_at(1_000);
        let (second, third) = rest.split_at(9_000);
        let nodes = vec![
            node("1", "RootWebArea", "", &["2"]),
            node("2", "generic", "", &["3", "4", "5"]),
            node("3", "StaticText", first, &[]),
            node("4", "StaticText", second, &[]),
            node("5", "StaticText", third, &[]),
        ];
        let outline = outline_of(&nodes);
        let start: String = json.chars().take(RESPONSE_SHOWN).collect();
        assert!(outline.contains(&format!("- text: {start}")), "{outline}");
        assert!(outline.ends_with(RESPONSE), "{outline}");
    }

    fn outline_of(nodes: &[Value]) -> String {
        outline(&tab(), nodes, &mut Refs::default(), None).text()
    }

    /// The same view drawn again, its elements new, answers with a line, and the agent keeps the
    /// outline it had; a new view in the same page answers whole.
    #[test]
    fn a_view_drawn_again_answers_with_a_line() {
        let mut refs = Refs::default();
        let before = outline(&tab(), &shop(false), &mut refs, None);
        let mut again = shop(false);
        // Its document is the same; its elements are new.
        for node in &mut again[1..] {
            let id = node["backendDOMNodeId"].as_i64().unwrap();
            node["backendDOMNodeId"] = json!(id + 1_000);
        }
        let again = outline(&tab(), &again, &mut refs, None);
        let (text, has) = again.since(&before);
        assert_eq!(
            text,
            format!("Tab 20: Shop\nURL: https://shop.example/\n\n{MOSTLY_CHANGED}")
        );
        assert!(!has);
        // A list that closes takes most of a small outline with it, which reads as before, but
        // nothing in it is new: it's answered whole, with the refs the agent acts on next.
        let open = outline(&tab(), &shop(true), &mut refs, None);
        let mut closed = shop(false);
        closed[0]["childIds"] = json!(["3", "5", "6"]);
        let closed = outline(&tab(), &closed, &mut refs, None);
        let (text, has) = closed.since(&open);
        assert_eq!(text, closed.text());
        assert!(text.contains("- button \"Add to basket\" [e"), "{text}");
        assert!(has);
        let mut other = vec![
            node("1", "RootWebArea", "Shop", &["2", "3"]),
            node("2", "heading", "Your basket", &[]),
            node("3", "button", "Check out", &[]),
        ];
        other[2]["backendDOMNodeId"] = json!(5_000);
        let other = outline(&tab(), &other, &mut refs, None);
        let (text, has) = other.since(&before);
        assert_eq!(text, other.text());
        assert!(has);
    }

    /// Payments and orders ask first anywhere, sending on a site that sends messages, and opening
    /// a bank's site; the words have to be words, not inside others.
    #[test]
    fn what_asks_the_user_first() {
        assert!(risk("shop.example", "Place your order", false).is_some());
        assert!(risk("shop.example", "Proceed to checkout", false).is_some());
        assert!(risk("shop.example", "Pay now", false).is_some());
        assert_eq!(risk("shop.example", "Payment methods", false), None);
        assert_eq!(risk("butik.example", "Betalingsmetoder", false), None);
        assert_eq!(risk("shop.example", "Send", false), None);
        assert!(risk("shop.example", "Send", true).is_some());
        assert!(risk("mail.google.com", "Send", false).is_some());
        assert!(risk("old.reddit.com", "send", false).is_some());
        assert_eq!(risk("notreddit.com", "send", false), None);
        assert_eq!(risk("mail.google.com", "Sender", false), None);
        assert!(opening_risk("www.chase.com").is_some());
        assert!(opening_risk("netbank.nordea.dk").is_some());
        assert_eq!(opening_risk("notnordea.dk"), None);
        assert!(opening_risk("online.mybank.example").is_some());
        assert_eq!(opening_risk("example.com"), None);
        assert_eq!(opening_risk("notchase.com"), None);
    }

    /// What asks first beyond the word and site lists, each named when it doesn't ask as it
    /// should: an order in other words and languages, a Send or Comment with a message to send on
    /// a site off the list, opening a bank's site off the list, and moving money on one.
    #[test]
    fn what_asks_first_beyond_the_lists() {
        let mut missed = Vec::new();
        for (host, label) in [
            ("shop.example", "Complete order"),
            ("shop.example", "Confirm booking"),
            ("butik.example", "Betal"),
            ("butik.example", "Køb nu"),
            ("laden.example", "Jetzt kaufen"),
            ("mail.proton.me", "Send"),
            ("www.binance.com", "Withdraw"),
            ("netbank.nordea.dk", "Overfør"),
            ("netbank.nordea.dk", "Send"),
            ("online.mybank.example", "Confirm"),
        ] {
            if risk(host, label, false).is_none() {
                missed.push(format!("{label} on {host} asks nothing"));
            }
        }
        for (host, label) in [
            ("teams.microsoft.com", "Send"),
            ("github.com", "Comment"),
            ("meta.discourse.org", "Create Topic"),
        ] {
            let (outline, control) = composed(label, true, Some("filled"));
            let message = outline.sends_message(control.form);
            if risk(host, &control.name, message).is_none() {
                missed.push(format!("{label} on {host}, with a message, asks nothing"));
            }
        }
        for host in [
            "www.nordea.dk",
            "n26.com",
            "www.hsbc.co.uk",
            "www.binance.com",
            "bcol.barclaycard.co.uk",
        ] {
            if opening_risk(host).is_none() {
                missed.push(format!("opening {host} asks nothing"));
            }
        }
        if risk("shop.example", "Confirm", false).is_some() {
            missed.push("Confirm off the money sites asks".into());
        }
        assert!(missed.is_empty(), "{}", missed.join("\n"));
    }

    /// A page with a message field and a Send control named `send`: the field a multi-line one
    /// in the control's form, or with `rich`, a rich-text editor in no form, as a chat's is, its
    /// `contents` as the core says. Returns its outline and the control.
    fn composed(send: &str, rich: bool, contents: Option<&str>) -> (Outline, Element) {
        let mut nodes = match rich {
            true => vec![
                node("1", "RootWebArea", "Chat", &["2", "3"]),
                node("2", "textbox", "Message", &[]),
                node("3", "button", send, &[]),
            ],
            false => vec![
                node("1", "RootWebArea", "Issue", &["4"]),
                node("4", "form", "", &["2", "3"]),
                node("2", "textbox", "Body", &[]),
                node("3", "button", send, &[]),
            ],
        };
        let property = match rich {
            true => {
                json!({ "name": "editable", "value": { "type": "token", "value": "richtext" } })
            }
            false => json!({ "name": "multiline", "value": { "type": "boolean", "value": true } }),
        };
        for each in &mut nodes {
            let id: i64 = each["nodeId"].as_str().unwrap().parse().unwrap();
            each["backendDOMNodeId"] = json!(id * 10);
            if id == 2 {
                each["properties"] = json!([property]);
                if let Some(contents) = contents {
                    each["contents"] = json!(contents);
                }
            }
        }
        let outline = outline(&tab(), &nodes, &mut Refs::default(), None);
        let control = outline
            .elements
            .iter()
            .find(|each| each.name == send)
            .cloned();
        (outline, control.unwrap())
    }

    /// A Send control asks on any site when the page holds a message for it to send: a
    /// multi-line field in its form or a rich-text editor in no form, holding anything. A
    /// single-line field, an empty one, or one in another form doesn't make it ask, and a
    /// line break in an editor is its Enter.
    #[test]
    fn a_message_the_page_holds_makes_its_send_ask() {
        for (rich, label) in [(false, "Comment"), (true, "Send")] {
            let (outline, control) = composed(label, rich, Some("filled"));
            assert!(outline.sends_message(control.form), "{label}");
            assert_eq!(risk("site.example", label, true), Some(SENDS));
            let (empty, control) = composed(label, rich, Some("empty"));
            assert!(
                !empty.sends_message(control.form),
                "{label} with nothing to send"
            );
            let (unknown, control) = composed(label, rich, None);
            assert!(
                !unknown.sends_message(control.form),
                "{label} with nothing said"
            );
        }
        // What the agent typed counts, and a Send of another form doesn't send it.
        let mut nodes = vec![
            node("1", "RootWebArea", "Shop", &["4", "5"]),
            node("4", "form", "", &["2", "3"]),
            node("2", "textbox", "Notes", &[]),
            node("3", "button", "Comment", &[]),
            node("5", "form", "", &["6"]),
            node("6", "button", "Submit", &[]),
        ];
        for (at, backend) in [(1, 40), (2, 20), (3, 30), (4, 50), (5, 60)] {
            nodes[at]["backendDOMNodeId"] = json!(backend);
        }
        nodes[2]["properties"] =
            json!([{ "name": "multiline", "value": { "type": "boolean", "value": true } }]);
        nodes[2]["holds"] = json!("Looks good");
        let page = outline(&tab(), &nodes, &mut Refs::default(), None);
        let named = |name: &str| page.elements.iter().find(|each| each.name == name).unwrap();
        assert!(page.sends_message(named("Comment").form));
        assert!(
            !page.sends_message(named("Submit").form),
            "a Submit in another form"
        );
        // A single-line field holding text is no message.
        nodes[2]["properties"] = json!([]);
        let single = outline(&tab(), &nodes, &mut Refs::default(), None);
        assert!(!single.sends_message(Some(40)));
        // Enter in the chat's editor, as a line break typed there is, asks as a click on Send.
        let (chat, _) = composed("Send", true, Some("filled"));
        assert!(chat.is_editor(20));
        let editor = chat.elements.iter().find(|each| each.node == 20).unwrap();
        let pressing = pressing_risk("chat.example", Some(&chat), "Enter", Some(editor));
        assert_eq!(
            pressing.map(|pressing| pressing.control.name),
            Some("Send".into())
        );
    }

    #[test]
    fn a_name_is_clipped() {
        assert_eq!(clip("Example", 3), "Exa…");
        assert_eq!(clip("Ex", 3), "Ex");
    }
}
