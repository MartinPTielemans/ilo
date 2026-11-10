//! The tools an agent calls, as MCP lists them, and what they answer. Each works only in the
//! agent's own Space (decision 8): it reads and acts on pages through the core, never through
//! script of its own, and never sees a password, a cookie or autofill.

use serde_json::{Value, json};

use super::{TabInfo, page};

/// What `initialize` tells the agent about the browser.
pub const INSTRUCTIONS: &str = concat!(
    env!("APP_NAME"),
    " is the user's own browser, signed in wherever they are. You work in a Space of your own, \
     which shows in the user's sidebar with your name, so they can watch; your tabs never take \
     their focus. You can only see and use the tabs in your Space, never the user's, and never \
     passwords, cookies or autofill.\n\n\
     Open a page with open_tab, then click, type, press_key and scroll by ref (like e12), using \
     the page's outline. A new page answers with its outline whole; an action on the same page \
     answers with what changed in it since the last outline you had, and snapshot gives it whole \
     again. A ref stays the element's while its page does. Choose in a select or a dropdown with \
     select, by the option's text. Each action waits for its element to be \
     ready and for the page to settle before it answers, and says when the page was still busy. \
     Use read_page for a long page's text and screenshot when the layout matters. Where a site's \
     page uses an API, request sends its HTTP requests from the tab, signed in as the page is. split_tabs shows two or \
     three of your tabs side by side in a split view, add_to_split adds one to it, separate_tab \
     takes one out, and list_split says which are together.\n\n\
     Payments, orders, sending messages, banks' sites and every request that may change \
     something ask the user first. If they say no, \
     don't try again. You can never type into a password field: a sign-in, a 2FA prompt or a \
     CAPTCHA is the user's to do, so hand the tab to them with hand_off. The user can take a tab \
     over too; it's theirs until they hand it back, and ask_for_tab asks them for it. Use \
     run_steps for several actions in a row.\n\n\
     When the same steps repeat across pages, open them all in one turn, then do each step on all \
     of them in one turn, one call per tab with its tab. Calls on different tabs run at the same \
     time."
);
/// What a task's CLI is told, after its own system prompt (Agent task).
pub const TASK_INSTRUCTIONS: &str = concat!(
    "You do a task the user asked in ",
    env!("APP_NAME"),
    ", their web browser, signed in wherever they are. You work only through its tools, in a \
     Space of your own: open pages, read them and act in them. You have no shell, files or web \
     search, so find everything with the browser.\n\n\
     First call title with a name for the task in two to four words and at most 24 characters, as \
     a tab's title reads, such as “Lisbon museum hours”.\n\n\
     When the same steps repeat across pages, such as one change on ten cases, open them all in \
     one turn, then do each step on all of them in one turn, one call per tab with its tab. Calls \
     on different tabs run at the same time.\n\n\
     When you need the user to choose or confirm something, call ask with the question in one \
     short sentence, what they need to know to answer as its context, and up to three short \
     answers when there are a few to pick from. It waits for their answer.\n\n\
     Never ask for a password, a one-time code or a card number in the conversation. A sign-in, \
     a 2FA prompt or a CAPTCHA is the user's to do: hand them the tab with hand_off.\n\n\
     Never pay, and never place an order, book or send a message unless the user asked you to. \
     When the work ends on a page only the user may finish, such as a checkout, stop there.\n\n\
     When you're done, call finish with the result in a sentence or two, and its summary in one \
     short line, such as “Gulbenkian, Sat 10:00–21:00”. Give finish tabs only when the user must \
     act there themselves: pay, sign, submit, or take over a session. Otherwise give none. Keep \
     what you say to the user short and plain."
);
/// A page's text an agent gets at most, as a long page would fill its context.
pub const MAX_TEXT: usize = 40_000;

/// The tools `agent` may call, for tools/list: a task's own too.
pub fn list(agent: &str) -> Value {
    let mut tools = shared();
    if let Some(tools) = tools.as_array_mut() {
        match super::is_task(agent) {
            true => tools.extend(task_tools()),
            // A task's pages live in the task, in a Space the user doesn't see, so only an agent
            // from outside pins.
            false => tools.extend(pin_tools()),
        }
    }
    tools
}

/// The names of the tools `agent` may call.
pub fn names(agent: &str) -> Vec<String> {
    let list = list(agent);
    (list.as_array().into_iter().flatten())
        .filter_map(|tool| Some(tool.get("name")?.as_str()?.to_string()))
        .collect()
}

/// Pinning, as the user pins in the sidebar: an agent pins its own tabs, and unpins or removes
/// only those it pinned.
fn pin_tools() -> Vec<Value> {
    let tab = tab_schema();
    let tabs = json!({ "readOnlyHint": false, "destructiveHint": false, "openWorldHint": false });
    vec![
        json!({
            "name": "pin_tab",
            "title": "Pin tab",
            "description": "Pin a tab in your Space at the page it's at, where it stays, even closed, until you unpin or remove it.",
            "inputSchema": { "type": "object", "properties": { "tab": tab } },
            "annotations": tabs,
        }),
        json!({
            "name": "unpin_tab",
            "title": "Unpin tab",
            "description": "Unpin a tab you pinned, to the top of your Today tabs. Tabs the user pinned stay pinned.",
            "inputSchema": { "type": "object", "properties": { "tab": tab } },
            "annotations": tabs,
        }),
        json!({
            "name": "remove_tab",
            "title": "Remove pinned tab",
            "description": "Remove a tab you pinned from your Space: it's unpinned and closed, and goes to the archive, where the user can still find it. Tabs the user pinned stay.",
            "inputSchema": { "type": "object", "properties": { "tab": tab } },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": true,
                "openWorldHint": false,
            },
        }),
    ]
}

/// A task's own tools: its title, a question for the user, and finishing.
fn task_tools() -> Vec<Value> {
    let own = json!({ "readOnlyHint": false, "destructiveHint": false, "openWorldHint": false });
    vec![
        json!({
            "name": "title",
            "title": "Name the task",
            "description": "Name the task in two to four words and at most 24 characters, as a tab's title reads, such as “Lisbon museum hours”, for the user's sidebar.",
            "inputSchema": {
                "type": "object",
                "properties": { "text": { "type": "string", "description": "The task's name, two to four words, at most 24 characters." } },
                "required": ["text"],
            },
            "annotations": own,
        }),
        json!({
            "name": "ask",
            "title": "Ask the user",
            "description": "Ask the user a question, and wait for their answer. The question alone shows in the user's sidebar, so put what they need to know to answer in context. With up to three short answers to pick from, each shows as a button. Never ask for a password, a code or a card number.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "question": { "type": "string", "description": "The question alone, in one short sentence ending in “?”, such as “Which would you rather visit?”." },
                    "context": { "type": "string", "description": "What the user needs to know to answer, in a sentence or two, shown above the question in the conversation. Leave it out when the question says it all." },
                    "answers": {
                        "type": "array",
                        "items": { "type": "string" },
                        "maxItems": 3,
                        "description": "Up to three short answers, a few words each.",
                    },
                },
                "required": ["question"],
            },
            "annotations": own,
        }),
        json!({
            "name": "finish",
            "title": "Finish",
            "description": "End the task with its result, and its summary for the user's sidebar. Give tabs only when the user must act there themselves: pay, sign, submit, or take over a session, such as a checkout only they may finish. Otherwise give none.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "result": { "type": "string", "description": "What you did or found, in a sentence or two." },
                    "summary": {
                        "type": "string",
                        "maxLength": super::task::SUMMARY_LENGTH,
                        "description": format!(
                            "The result in one short line of {} characters at most, such as “Gulbenkian, Sat 10:00–21:00”.",
                            super::task::SUMMARY_LENGTH,
                        ),
                    },
                    "tabs": {
                        "type": "array",
                        "items": { "type": "integer" },
                        "maxItems": super::task::PAGES,
                        "description": format!(
                            "The tabs the user must act in themselves, from your Space, in the order they'll want them, {} at most. Leave it out when there's nothing left for them to do.",
                            super::task::PAGES,
                        ),
                    },
                },
                "required": ["result", "summary"],
            },
            "annotations": own,
        }),
    ]
}

/// A tool's tab argument.
fn tab_schema() -> Value {
    json!({
        "type": "integer",
        "description": "The tab's id, from list_tabs or open_tab. Without it, the tab you used last, else your first."
    })
}

/// The tools every agent may call.
fn shared() -> Value {
    let tab = tab_schema();
    let element = json!({
        "type": "string",
        "description": "The element's ref, from the latest outline, like e12."
    });
    let outline = json!({
        "type": "string",
        "enum": ["changes", "full", "none"],
        "description": "What of the page's outline the answer gives: what changed since the last outline you had of the tab (changes, the default, which is the whole outline on a new page), all of it (full), or only what the action did (none)."
    });
    let acts = json!({ "readOnlyHint": false, "destructiveHint": false, "openWorldHint": true });
    let tabs = json!({ "readOnlyHint": false, "destructiveHint": false, "openWorldHint": false });
    json!([
        {
            "name": "list_tabs",
            "title": "List tabs",
            "description": "List the tabs in your Space, pinned first: each one's id, title and URL, then who pinned it if it's pinned. * marks the tab you used last.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "open_tab",
            "title": "Open tab",
            "description": "Open a URL, or search for words, in a new tab in your Space. Answers with the page's outline.",
            "inputSchema": {
                "type": "object",
                "properties": { "url": { "type": "string", "description": "A URL, or words to search for." } },
                "required": ["url"],
            },
            "annotations": acts,
        },
        {
            "name": "close_tab",
            "title": "Close tab",
            "description": "Close a tab in your Space. A page that may have unsaved changes stays open unless discard is true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tab": tab,
                    "discard": { "type": "boolean", "description": "Close it even if its page may have unsaved changes, losing them. In a tab you didn't open, the user is asked first." },
                },
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": true,
                "openWorldHint": false,
            },
        },
        {
            "name": "split_tabs",
            "title": "Split tabs",
            "description": "Show two or three tabs of your Space side by side in a split view, left to right in the order given, as one row in the sidebar. A tab in another split leaves it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tabs": {
                        "type": "array",
                        "items": { "type": "integer" },
                        "minItems": 2,
                        "maxItems": SPLIT_TABS,
                        "description": "The tabs' ids, from list_tabs or open_tab, left to right.",
                    },
                },
                "required": ["tabs"],
            },
            "annotations": tabs,
        },
        {
            "name": "add_to_split",
            "title": "Add to split view",
            "description": "Add a tab of your Space to a split view, at its right end. A split view holds three tabs at most.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tab": { "type": "integer", "description": "The tab to add, from list_tabs or open_tab." },
                    "split": { "type": "integer", "description": "A tab of the split view." },
                },
                "required": ["tab", "split"],
            },
            "annotations": tabs,
        },
        {
            "name": "separate_tab",
            "title": "Separate from split view",
            "description": "Take a tab out of its split view, to just below the tabs left in it. The split view goes once one tab is left.",
            "inputSchema": { "type": "object", "properties": { "tab": tab }, "required": ["tab"] },
            "annotations": tabs,
        },
        {
            "name": "list_split",
            "title": "List split view",
            "description": "The tabs in a tab's split view, left to right, or that it's in none.",
            "inputSchema": { "type": "object", "properties": { "tab": tab } },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "navigate",
            "title": "Navigate",
            "description": "Go to a URL, or search for words, in a tab. \"back\" and \"forward\" move through its history. Answers with the page's outline.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "A URL, words to search for, \"back\" or \"forward\"." },
                    "tab": tab,
                },
                "required": ["url"],
            },
            "annotations": acts,
        },
        {
            "name": "snapshot",
            "title": "Snapshot",
            "description": "The page's outline, whole, from its accessibility tree. Elements you can act on have refs like [e12]. With a ref, only that element's part, such as a form without the page's menus.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ref": { "type": "string", "description": "An element's ref, to outline only it and what's in it." },
                    "tab": tab,
                },
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": true },
        },
        {
            "name": "read_page",
            "title": "Read page",
            "description": "Read the text of a page in your Space, as a screen reader would.",
            "inputSchema": { "type": "object", "properties": { "tab": tab } },
            "annotations": { "readOnlyHint": true, "openWorldHint": true },
        },
        {
            "name": "click",
            "title": "Click",
            "description": "Click an element by its ref. Answers with what changed in the page's outline.",
            "inputSchema": {
                "type": "object",
                "properties": { "ref": element, "outline": outline, "tab": tab },
                "required": ["ref"],
            },
            "annotations": acts,
        },
        {
            "name": "type",
            "title": "Type",
            "description": "Type text into a field by its ref, in place of what it holds. submit presses Enter after. Answers with what changed in the page's outline.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ref": element,
                    "text": { "type": "string" },
                    "submit": { "type": "boolean" },
                    "outline": outline,
                    "tab": tab,
                },
                "required": ["ref", "text"],
            },
            "annotations": acts,
        },
        {
            "name": "select",
            "title": "Select",
            "description": "Choose an option in a select or a dropdown by its ref and the option's text: the option with that name, else the one with it in another case. A dropdown whose options don't show is opened first. With no such option, or several, it chooses none and answers with the options there are. Answers with the page's outline.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ref": element,
                    "option": { "type": "string", "description": "The option's text, as the outline or the page shows it." },
                    "tab": tab,
                },
                "required": ["ref", "option"],
            },
            "annotations": acts,
        },
        {
            "name": "press_key",
            "title": "Press key",
            "description": "Press a key in a tab, in the element that has the keyboard. Answers with what changed in the page's outline.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "key": { "type": "string", "enum": page::KEYS },
                    "outline": outline,
                    "tab": tab,
                },
                "required": ["key"],
            },
            "annotations": acts,
        },
        {
            "name": "scroll",
            "title": "Scroll",
            "description": "Scroll down or up most of a screen, as a mouse wheel does at the middle of the page, or of the element whose ref you give, such as a panel or a frame. With a ref and no direction, brings the element into view. Answers with what changed in the page's outline, or an error when nothing scrolled, as at the end.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "direction": { "type": "string", "enum": ["down", "up"] },
                    "ref": element,
                    "outline": outline,
                    "tab": tab,
                },
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": true },
        },
        {
            "name": "screenshot",
            "title": "Screenshot",
            "description": "A picture of a tab's page, for when its layout or images matter.",
            "inputSchema": { "type": "object", "properties": { "tab": tab } },
            "annotations": { "readOnlyHint": true, "openWorldHint": true },
        },
        {
            "name": "request",
            "title": "Send HTTP requests",
            "description": "Send HTTP requests from a tab's page to its own site, as the page's own script could, signed in as the page is: for a site's API, such as the JSON its page reads, when that's quicker than its pages. GET and HEAD go at once. Any other method may change something, so it asks the user first, every time: all of a call's changes in one question, and none is sent if they say no. The browser adds the page's cookies, which you never see; you can't set Cookie, Authorization or a header the browser sets. Answers with each request's status, type and text body, never its headers, with values that look like sign-in tokens or secrets hidden. They go one after another; one that gets no answer stops the rest.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "requests": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": super::request::MAX,
                        "items": {
                            "type": "object",
                            "properties": {
                                "method": { "type": "string", "description": "GET, POST, PUT, PATCH, DELETE or another method." },
                                "path": { "type": "string", "description": "A path on the tab's site, such as /api/items?page=2, or a whole address on it." },
                                "headers": { "type": "object", "additionalProperties": { "type": "string" } },
                                "body": { "description": "JSON, sent as application/json, or a string, sent as text unless headers give its Content-Type. 1 MB at most." },
                            },
                            "required": ["method", "path"],
                        },
                    },
                    "tab": tab,
                },
                "required": ["requests"],
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": true, "openWorldHint": true },
        },
        {
            "name": "run_steps",
            "title": "Run steps",
            "description": "Run several actions in a tab in one call, in order: each is checked as it would be alone. Stops at the first that fails, asks the user first, or is refused. Answers with what each did, then what changed in the page's outline, as outline says.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "steps": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": MAX_STEPS,
                        "items": {
                            "type": "object",
                            "properties": {
                                "action": { "type": "string", "enum": STEPS },
                                "ref": element,
                                "text": { "type": "string" },
                                "submit": { "type": "boolean" },
                                "option": { "type": "string" },
                                "key": { "type": "string", "enum": page::KEYS },
                                "url": { "type": "string" },
                                "direction": { "type": "string", "enum": ["down", "up"] },
                                "gone": { "type": "string" },
                                "until": { "type": "string", "enum": ["quiet"] },
                                "seconds": { "type": "number", "exclusiveMinimum": 0, "maximum": MAX_WAIT },
                            },
                            "required": ["action"],
                        },
                        "description": "Each step is an action and what that tool takes: click and scroll a ref, type a ref and text, select a ref and an option, press_key a key, navigate a url. wait waits for text to show in the page, for gone text to go from it, or until \"quiet\", for the page to settle, each up to seconds (5 if not given); or with none of those, waits the seconds.",
                    },
                    "outline": outline,
                    "tab": tab,
                },
                "required": ["steps"],
            },
            "annotations": acts,
        },
        {
            "name": "hand_off",
            "title": "Hand off to the user",
            "description": "Hand a tab to the user for a step only they may do: a sign-in, 2FA, a CAPTCHA, or a choice that's theirs. They're asked in the browser, and this waits until they hand the tab back. Meanwhile the tab is theirs, and calls on it are refused.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "reason": { "type": "string", "description": "What the user is to do, in a short sentence." },
                    "tab": tab,
                },
                "required": ["reason"],
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "ask_for_tab",
            "title": "Ask for a tab back",
            "description": "Ask the user to hand back a tab they took over. They're asked in the browser, and this waits for their answer: the tab back, or that they keep it. Don't ask again for a tab they keep.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "reason": { "type": "string", "description": "Why you need the tab, in a short sentence." },
                    "tab": tab,
                },
                "required": ["reason"],
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
    ])
}

/// The actions run_steps takes: those of the tools that act in a tab, not a screenshot, and wait.
pub const STEPS: [&str; 8] = [
    "navigate",
    "snapshot",
    "click",
    "type",
    "select",
    "press_key",
    "scroll",
    "wait",
];
/// The tabs a split view holds at most, as the core keeps it (bc_tab_split_with).
pub const SPLIT_TABS: usize = 3;
/// The steps one run_steps takes at most.
pub const MAX_STEPS: usize = 20;
/// What a wait step until "quiet" waits for, as its title says it.
pub const SETTLE: &str = "the page to settle";
/// How long a wait step waits for its text, in seconds, unless it says, and at most.
pub const WAIT: f64 = 5.0;
pub const MAX_WAIT: f64 = 30.0;

/// list_tabs' answer.
pub fn tab_list(tabs: &[TabInfo], current: Option<u64>, mine: &[u64]) -> String {
    if tabs.is_empty() {
        return "Your Space has no tabs yet. The user can open pages in it for you.".into();
    }
    let lines = tabs.iter().map(|tab| {
        let mark = if Some(tab.id) == current { '*' } else { ' ' };
        let pinned = match (tab.pinned, mine.contains(&tab.id)) {
            (false, _) => "",
            (true, true) => "  pinned by you",
            (true, false) => "  pinned by the user",
        };
        format!("{mark} {}  {}  {}{pinned}", tab.id, tab.title, tab.url)
    });
    lines.collect::<Vec<_>>().join("\n")
}

/// list_split's answer, and what split_tabs, add_to_split and separate_tab answer with: the tabs
/// shown with `tab`, left to right, as the core lists them (bc_tab_split).
pub fn split_list(tab: u64, split: &[u64]) -> String {
    match split.len() > 1 {
        true => {
            let tabs: Vec<String> = split.iter().map(u64::to_string).collect();
            format!(
                "Tab {tab}'s split view, left to right: {}.",
                tabs.join(", ")
            )
        }
        false => format!("Tab {tab} is in no split view."),
    }
}

/// read_page's answer: which page it is, then its text, cut at `MAX_TEXT` bytes.
pub fn page_text(tab: &TabInfo, text: &str) -> String {
    let mut end = text.len().min(MAX_TEXT);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let cut = match end < text.len() {
        true => "\n\n[The page goes on: this is its first part.]",
        false => "",
    };
    format!(
        "Tab {}: {}\nURL: {}\n\n{}{cut}",
        tab.id,
        tab.title,
        tab.url,
        &text[..end]
    )
}

/// The `tab` argument, as a number or a string of one.
pub fn tab_argument(arguments: &Value) -> Result<Option<u64>, String> {
    match arguments.get("tab") {
        None | Some(Value::Null) => Ok(None),
        Some(id) => tab_id(id).map(Some).ok_or_else(not_a_tab),
    }
}

/// A tab's id, as a number or a string of one.
pub fn tab_id(id: &Value) -> Option<u64> {
    match id {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn not_a_tab() -> String {
    "tab is a tab's id, a number from list_tabs.".into()
}

/// A step of an agent's work, as a task's trace, its card and the activity log say it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Did {
    /// The tool, a run's step's action, "back" or "forward", or "thought".
    pub tool: String,
    /// The site it was on, or went to.
    pub site: String,
    /// What on the page: an element by its name in quotes or its role, a key, a direction, or a
    /// wait's text in quotes or its seconds.
    pub target: String,
    /// A thought's text, the address it opened, whole, what it typed when that may show (never
    /// into a password field, and at most `TYPED_SHOWN` characters), or the way it scrolled in an
    /// element.
    pub text: String,
    /// The tab it acted on, by which a turn's same step on several tabs is one line (#1476). None
    /// for a page it opens, whose tab it makes, and for what isn't on a tab.
    pub tab: Option<u64>,
    /// The address it was on or opened, without its scheme, as such a line says each tab.
    pub page: String,
}

/// What a step typed shows up to, in characters.
pub const TYPED_SHOWN: usize = 40;

/// Whether a step is under way, done, or failed, which its title says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tense {
    Now,
    Done,
    Failed,
}

/// A step's title, in one sentence: "Opening gulbenkian.pt", "Read maat.pt",
/// "Couldn't open maat.pt/en/visit".
pub fn step_title(did: &Did, tense: Tense) -> String {
    let site = |none: &str| match did.site.is_empty() {
        true => none.to_string(),
        false => did.site.clone(),
    };
    let target = |none: &str| match did.target.is_empty() {
        true => none.to_string(),
        false => did.target.clone(),
    };
    // Each step's verb in its three tenses, and what it acts on.
    let (now, done, failed, on) = match did.tool.as_str() {
        "open_tab" | "navigate" => ("Opening", "Opened", "Couldn't open", site("a page")),
        "back" => (
            "Going back on",
            "Went back on",
            "Couldn't go back on",
            site("the page"),
        ),
        "forward" => (
            "Going forward on",
            "Went forward on",
            "Couldn't go forward on",
            site("the page"),
        ),
        "read_page" | "snapshot" => ("Reading", "Read", "Couldn't read", site("the page")),
        "screenshot" => (
            "Looking at",
            "Looked at",
            "Couldn't look at",
            site("the page"),
        ),
        "click" => (
            "Clicking",
            "Clicked",
            "Couldn't click",
            target("on the page"),
        ),
        "type" => {
            let into = target("a field");
            return match (tense, did.text.is_empty()) {
                (Tense::Now, true) => format!("Typing in {into}"),
                (Tense::Now, false) => format!("Typing “{}” in {into}", did.text),
                (Tense::Done, true) => format!("Typed in {into}"),
                (Tense::Done, false) => format!("Typed “{}” in {into}", did.text),
                (Tense::Failed, _) => format!("Couldn't type in {into}"),
            };
        }
        "select" => {
            let into = target("a list");
            return match (tense, did.text.is_empty()) {
                (Tense::Now, true) => format!("Choosing in {into}"),
                (Tense::Now, false) => format!("Choosing “{}” in {into}", did.text),
                (Tense::Done, true) => format!("Chose in {into}"),
                (Tense::Done, false) => format!("Chose “{}” in {into}", did.text),
                (Tense::Failed, true) => format!("Couldn't choose in {into}"),
                (Tense::Failed, false) => format!("Couldn't choose “{}” in {into}", did.text),
            };
        }
        "press_key" => ("Pressing", "Pressed", "Couldn't press", target("a key")),
        "request" => {
            let to = format!("{} to {}", target("a request"), site("its site"));
            ("Sending", "Sent", "Couldn't send", to)
        }
        "scroll" => {
            let to = match (did.target.as_str(), did.text.as_str()) {
                ("up" | "down" | "", _) => format!("{} on {}", target("down"), site("the page")),
                (element, "") => format!("to {element}"),
                (element, way) => format!("{way} in {element}"),
            };
            ("Scrolling", "Scrolled", "Couldn't scroll", to)
        }
        "wait" => match did.target.strip_suffix(" to go") {
            // Text that should go: “Loading” to go.
            Some(text) if tense == Tense::Failed => return format!("Didn't see {text} go"),
            _ if did.target == SETTLE => {
                ("Waiting for", "Waited for", "Couldn't wait for", target(""))
            }
            _ if did.target.starts_with('“') => (
                "Waiting for",
                "Waited for",
                "Didn't see",
                target("the page"),
            ),
            _ => ("Waiting", "Waited", "Couldn't wait", target("")),
        },
        "list_tabs" => (
            "Looking over",
            "Looked over",
            "Couldn't look over",
            "its tabs".to_string(),
        ),
        "close_tab" => ("Closing", "Closed", "Couldn't close", site("a tab")),
        "pin_tab" => ("Pinning", "Pinned", "Couldn't pin", site("a tab")),
        "unpin_tab" => ("Unpinning", "Unpinned", "Couldn't unpin", site("a tab")),
        "remove_tab" => (
            "Removing",
            "Removed",
            "Couldn't remove",
            site("a pinned tab"),
        ),
        "split_tabs" => (
            "Putting",
            "Put",
            "Couldn't put",
            format!("{} in a split view", site("tabs")),
        ),
        "add_to_split" => (
            "Adding",
            "Added",
            "Couldn't add",
            format!("{} to a split view", site("a tab")),
        ),
        "separate_tab" => (
            "Separating",
            "Separated",
            "Couldn't separate",
            format!("{} from its split view", site("a tab")),
        ),
        "list_split" => (
            "Looking over",
            "Looked over",
            "Couldn't look over",
            "a split view".to_string(),
        ),
        "hand_off" => (
            "Handing you",
            "Handed you",
            "Couldn't hand you",
            site("a tab"),
        ),
        "ask_for_tab" => {
            let tab = site("a tab");
            return match tense {
                Tense::Now => format!("Asking for {tab} back"),
                Tense::Done => format!("Asked for {tab} back"),
                Tense::Failed => format!("Couldn't ask for {tab} back"),
            };
        }
        "ask" => ("Asking you", "Asked you", "Couldn't ask you", String::new()),
        // The page's question, which the agent can't answer (#2323).
        "notifications" => {
            return format!("{} asked to send notifications", site("The page"));
        }
        "thought" => ("Thinking", "Thought", "Thought", String::new()),
        action => return describe(action).to_string(),
    };
    let verb = match tense {
        Tense::Now => now,
        Tense::Done => done,
        Tense::Failed => failed,
    };
    match on.is_empty() {
        true => verb.to_string(),
        false => format!("{verb} {on}"),
    }
}

/// A turn's same step on `pages` tabs, as one line: "Opened 10 pages", "Clicked “Closed” on 10
/// pages".
pub fn group_title(did: &Did, pages: usize, tense: Tense) -> String {
    let on = |what: &str| Did {
        site: format!("{pages} {what}"),
        ..did.clone()
    };
    match did.tool.as_str() {
        "open_tab" | "navigate" | "back" | "forward" | "read_page" | "snapshot" | "screenshot" => {
            step_title(&on("pages"), tense)
        }
        "scroll" if matches!(did.target.as_str(), "up" | "down" | "") => {
            step_title(&on("pages"), tense)
        }
        "close_tab" | "pin_tab" | "unpin_tab" | "remove_tab" | "hand_off" | "ask_for_tab"
        | "split_tabs" | "add_to_split" | "separate_tab" => step_title(&on("tabs"), tense),
        _ => format!("{} on {pages} pages", step_title(did, tense)),
    }
}

/// A step's glyph, by its tool, from the shell's one family.
pub fn step_glyph(tool: &str) -> &'static str {
    match tool {
        "read_page" | "snapshot" => "icons/text-align-start.svg",
        "click" => "icons/mouse-pointer-2.svg",
        "type" => "icons/type.svg",
        "select" => "icons/chevrons-up-down.svg",
        "press_key" => "icons/keyboard.svg",
        "scroll" => "icons/arrow-down.svg",
        "screenshot" => "icons/eye.svg",
        "request" => "icons/arrow-left-right.svg",
        "wait" => "icons/clock.svg",
        "list_tabs" | "close_tab" | "remove_tab" | "hand_off" | "ask_for_tab" | "split_tabs"
        | "add_to_split" | "separate_tab" | "list_split" => "icons/app-window.svg",
        "pin_tab" | "unpin_tab" => "icons/pin.svg",
        "ask" => "icons/circle-question-mark.svg",
        "thought" => "icons/lightbulb.svg",
        _ => "icons/globe.svg",
    }
}

/// Why a step failed, in a few words, from its call's error: "not found", "timed out", "504".
/// Empty when the error gives no reason, as the step's title already says it couldn't.
pub fn reason(error: &str) -> String {
    let error = error.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|word| error.contains(word));
    // An HTTP error's status, as "HTTP 504" or "status 504" says it.
    let words: Vec<&str> = error
        .split(|char: char| !char.is_ascii_alphanumeric())
        .collect();
    let status = words.windows(2).find_map(|pair| {
        let [before, code] = pair else { return None };
        let status = code.len() == 3
            && code
                .parse::<u16>()
                .is_ok_and(|code| (400..600).contains(&code));
        (matches!(*before, "http" | "status" | "error") && status).then_some(*code)
    });
    let reason = if has(&["stopped you", "turned agents off", "connection closed"]) {
        "stopped"
    } else if has(&["you said no", "said never", "the user said no", "declined"]) {
        "you said no"
    } else if has(&["asks the user first"]) {
        "needs your OK"
    } else if has(&["password"]) {
        "password field"
    } else if has(&["http or https", "only http and https"]) {
        "not a web page"
    } else if has(&[
        "there's no element",
        "there's no option",
        "no tab",
        "isn't open",
        "not found",
        "404",
    ]) {
        "not found"
    } else if has(&["timed out", "timeout", "took too long"]) {
        "timed out"
    } else if has(&["still loading", "hasn't loaded", "is loading"]) {
        "still loading"
    } else if let Some(code) = status {
        code
    } else if has(&["didn't show"]) {
        "didn't show"
    } else if has(&[
        "user's already",
        "user has",
        "user doesn't have",
        "took it over",
    ]) {
        "it's yours"
    } else {
        ""
    };
    reason.to_string()
}

/// What an action an agent took is, for the activity log, by the name the core's entry gives it.
pub fn describe(action: &str) -> &'static str {
    match action {
        "list_tabs" => "Listed its tabs",
        "read_page" => "Read the page",
        "open_tab" => "Opened a page",
        "navigate" => "Went to a page",
        "back" => "Went back",
        "forward" => "Went forward",
        "snapshot" => "Looked at the page",
        "click" => "Clicked",
        "type" => "Typed",
        "select_option" => "Chose an option",
        "press_key" => "Pressed a key",
        "scroll" => "Scrolled",
        "screenshot" => "Took a screenshot",
        "request" => "Sent a request",
        "notifications" => "A page asked to send notifications",
        "connected" => "Connected",
        "disconnected" => "Disconnected",
        "released" => "Let go of the tab",
        "taken back" => "You took the tab over",
        "handed back" => "You handed the tab back",
        "given" => "You gave it a tab",
        "stopped" => "You stopped it",
        "stopped by the user" => "You stopped it, which closed its connection",
        "hand_off" => "Handed you a tab",
        "close_tab" => "Closed a tab",
        "discard_tab" => "Closed a tab without saving",
        "download" => "Downloaded a file",
        "pin_tab" => "Pinned a tab",
        "unpin_tab" => "Unpinned a tab",
        "remove_tab" => "Removed a pinned tab",
        "split_tabs" => "Put tabs in a split view",
        "add_to_split" => "Added a tab to a split view",
        "separate_tab" => "Separated a tab from its split view",
        "list_split" => "Looked over a split view",
        "passkey" => "Handed you a tab for your passkey",
        "ask_for_tab" => "Asked for a tab back",
        "kept" => "You kept the tab",
        "run_steps" => "Ran steps",
        "wait" => "Waited for the page",
        "revoked" => "You revoked its permission",
        "revoked all" => "You revoked all its permissions",
        "title" => "Named the task",
        "ask" => "Asked you",
        "answered" => "You answered",
        "finish" => "Finished",
        _ => "Acted",
    }
}
