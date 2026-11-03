//! A page as an agent sees it and acts on it: the outline of its accessibility tree, as the core's
//! bc_agent_page gives it without any field's value, with a ref for each element it can act on,
//! and which actions ask the user first.

use std::collections::HashMap;

use serde_json::Value;

use super::TabInfo;

/// Roles an agent can act on, each given a ref.
const INTERACTIVE: &[&str] = &[
    "button", "link", "textbox", "searchbox", "combobox", "checkbox", "radio", "switch", "slider",
    "spinbutton", "listbox", "option", "menuitem", "menuitemcheckbox", "menuitemradio", "tab",
    "treeitem",
];
/// Wrappers that only add nesting, whose children are outlined a level up.
const FLATTEN: &[&str] = &[
    "none", "generic", "presentation", "LayoutTable", "LayoutTableRow", "LayoutTableCell",
    "InlineTextBox", "RootWebArea",
];
/// Landmarks worth a line without a name.
const LANDMARKS: &[&str] = &[
    "form", "dialog", "alertdialog", "alert", "navigation", "main", "banner", "search",
    "contentinfo",
];
const STATES: &[&str] = &["checked", "selected", "expanded", "disabled", "required", "level"];
/// The outline's bytes at most, as a long page would fill the agent's context.
pub const MAX_OUTLINE: usize = 30_000;

/// The keys press_key presses, as the core knows them.
pub const KEYS: &[&str] = &[
    "Enter", "Tab", "Escape", "Backspace", "Delete", "Space", "ArrowUp", "ArrowDown", "ArrowLeft",
    "ArrowRight", "PageUp", "PageDown", "Home", "End",
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

/// The page's outline, from the nodes of its tree, which names each element it can act on in
/// `refs`: a line for each, as `- role "name" [e12] (states)`, and `- text: …` for its text.
pub fn outline(tab: &TabInfo, nodes: &[Value], refs: &mut Refs) -> String {
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
    if let Some(root) = nodes.first() {
        walk.node(root, 0, "", 0);
    }
    let mut notes = Vec::new();
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
    let lines = walk.lines.join("\n");
    let mut text = format!("Tab {}: {title}\nURL: {}\n\n{lines}", tab.id, tab.url);
    if !notes.is_empty() {
        text.push_str("\n\n");
        text.push_str(&notes.join("\n"));
    }
    text
}

struct Walk<'a> {
    by_id: HashMap<&'a str, &'a Value>,
    field_names: Vec<String>,
    refs: &'a mut Refs,
    lines: Vec<String>,
    size: usize,
    password: bool,
    captcha: bool,
}

impl Walk<'_> {
    fn add(&mut self, depth: usize, line: String) {
        if self.size > MAX_OUTLINE {
            return;
        }
        // Its indent and line break too.
        self.size += 2 * depth + line.len() + 1;
        self.lines.push(format!("{}{line}", "  ".repeat(depth)));
    }

    fn node(&mut self, node: &Value, depth: usize, parent_name: &str, levels: usize) {
        // A page's tree is shallow; one nested deeper is cut rather than overflow the stack.
        if levels > 200 || self.size > MAX_OUTLINE {
            return;
        }
        let ignored = node.get("ignored").and_then(Value::as_bool).unwrap_or(false);
        let role = role(node);
        let name = name(node);
        let lower = name.to_lowercase();
        let human = ["captcha", "not a robot", "verify you are human"];
        if human.iter().any(|words| lower.contains(words)) {
            self.captcha = true;
        }
        if !ignored && role == "StaticText" {
            let repeated = parent_name.contains(&name) || self.field_names.contains(&name);
            if !name.is_empty() && !repeated {
                self.add(depth, format!("- text: {name}"));
            }
            return;
        }
        let (mut next, mut next_name) = (depth, parent_name.to_string());
        let shown = INTERACTIVE.contains(&role) || !name.is_empty() || LANDMARKS.contains(&role);
        if !ignored && !FLATTEN.contains(&role) && shown {
            let mut line = format!("- {role}");
            if !name.is_empty() {
                line.push_str(&format!(" \"{}\"", clip(&name, 160)));
            }
            let backend = node.get("backendDOMNodeId").and_then(Value::as_i64);
            if let (true, Some(backend)) = (INTERACTIVE.contains(&role), backend) {
                let password = node.get("password").and_then(Value::as_bool).unwrap_or(false);
                let element = Element {
                    node: backend,
                    role: role.into(),
                    name: name.clone(),
                    password,
                };
                line.push_str(&format!(" [e{}]", self.refs.name(element)));
                if password {
                    self.password = true;
                    line.push_str(" (password: off limits)");
                }
            }
            for property in node.get("properties").and_then(Value::as_array).into_iter().flatten() {
                let Some(state) = property.get("name").and_then(Value::as_str) else {
                    continue;
                };
                if !STATES.contains(&state) {
                    continue;
                }
                let value = property.pointer("/value/value");
                match (state, value) {
                    ("level", Some(level)) => line.push_str(&format!(" (level {level})")),
                    (_, Some(Value::Bool(true))) => line.push_str(&format!(" ({state})")),
                    (_, Some(Value::String(text))) if text == "true" => {
                        line.push_str(&format!(" ({state})"))
                    }
                    _ => {}
                }
            }
            self.add(depth, line);
            next = depth + 1;
            next_name = name;
        }
        let children = node.get("childIds").and_then(Value::as_array);
        for child in children.into_iter().flatten() {
            if let Some(child) = child.as_str().and_then(|id| self.by_id.get(id).copied()) {
                self.node(child, next, &next_name, levels + 1);
            }
        }
    }
}

fn role(node: &Value) -> &str {
    node.pointer("/role/value").and_then(Value::as_str).unwrap_or_default()
}

/// The node's name, its runs of white space one space.
fn name(node: &Value) -> String {
    let name = node.pointer("/name/value").and_then(Value::as_str).unwrap_or_default();
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text`, cut at `chars` characters.
pub fn clip(text: &str, chars: usize) -> String {
    match text.char_indices().nth(chars) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.into(),
    }
}

/// Words that pay or order, as a button's name has them.
const PAYMENT: &[&str] = &[
    "pay", "buy", "purchase", "checkout", "check out", "place order", "place your order",
    "submit order", "order now", "subscribe", "donate", "transfer",
];
/// Words that send, as a button's name has them.
const SEND: &[&str] = &["send", "submit", "post", "publish", "reply", "tweet", "comment"];
const MESSAGING: &[&str] = &[
    "mail.google.com", "outlook.live.com", "outlook.office.com", "slack.com", "discord.com",
    "x.com", "twitter.com", "linkedin.com", "facebook.com", "messenger.com", "instagram.com",
    "whatsapp.com", "web.telegram.org", "reddit.com",
];
const BANKS: &[&str] = &[
    "chase", "wellsfargo", "bankofamerica", "citi", "capitalone", "schwab", "fidelity", "vanguard",
    "paypal", "venmo", "revolut", "wise", "monzo", "coinbase",
];

/// Why acting on the element named `label` on `host` asks the user first, if it does: it looks
/// like a payment or an order, or it sends a message or post from a site that sends them. Typing
/// with submit and pressing Enter are "send".
pub fn risk(host: &str, label: &str) -> Option<&'static str> {
    let label = label.to_lowercase();
    if PAYMENT.iter().any(|words| has_words(&label, words)) {
        return Some("It looks like a payment or an order.");
    }
    let sends = SEND.iter().any(|words| has_words(&label, words));
    if sends && MESSAGING.iter().any(|site| on(host, site)) {
        return Some("It sends a message or a post.");
    }
    None
}

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
        let outline = outline(&tab(), &nodes, &mut refs);
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
        let again = super::outline(&tab(), &nodes, &mut refs);
        assert!(again.contains("- textbox \"Coupon\" [e2]"));
    }

    /// A page bigger than an agent should take in is cut, and says so.
    #[test]
    fn a_long_outline_is_cut() {
        let ids: Vec<String> = (2..5000).map(|id| id.to_string()).collect();
        let children: Vec<&str> = ids.iter().map(String::as_str).collect();
        let mut nodes = vec![node("1", "RootWebArea", "Long", &children)];
        nodes.extend(ids.iter().map(|id| node(id, "paragraph", &format!("Paragraph {id}"), &[])));
        let outline = outline(&tab(), &nodes, &mut Refs::default());
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
