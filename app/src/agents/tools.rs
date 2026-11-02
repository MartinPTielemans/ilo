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
     the outline each action answers with. A ref stays the element's while its page does. Use \
     read_page for a long page's text and screenshot when the layout matters.\n\n\
     Payments, orders, sending messages and banks' sites ask the user first. If they say no, \
     don't try again. You can never type into a password field: a sign-in, a 2FA prompt or a \
     CAPTCHA is the user's to do, so hand the tab to them with hand_off. The user can take a tab \
     over too; it's theirs until they hand it back, and ask_for_tab asks them for it. Use \
     run_steps for several actions in a row."
);
/// What a task's CLI is told, after its own system prompt (Agent task).
pub const TASK_INSTRUCTIONS: &str = concat!(
    "You do a task the user asked in ",
    env!("APP_NAME"),
    ", their web browser, signed in wherever they are. You work only through its tools, in a \
     Space of your own: open pages, read them and act in them. You have no shell, files or web \
     search, so find everything with the browser.\n\n\
     First call title with a name for the task in two to five words.\n\n\
     When you need the user to choose or confirm something, call ask with the question, and up \
     to three short answers when there are a few to pick from. It waits for their answer.\n\n\
     Never ask for a password, a one-time code or a card number in the conversation. A sign-in, \
     a 2FA prompt or a CAPTCHA is the user's to do: hand them the tab with hand_off.\n\n\
     Never pay, and never place an order, book or send a message unless the user asked you to. \
     When the work ends on a page only the user may finish, such as a checkout, stop there.\n\n\
     When you're done, call finish with the result in a sentence or two, and the tab the user \
     should open, if there is one. Keep what you say to the user short and plain."
);
/// A page's text an agent gets at most, as a long page would fill its context.
pub const MAX_TEXT: usize = 40_000;

/// The tools `agent` may call, for tools/list: a task's own too.
pub fn list(agent: &str) -> Value {
    let mut tools = shared();
    if super::is_task(agent)
        && let Some(tools) = tools.as_array_mut()
    {
        tools.extend(task_tools());
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

/// A task's own tools: its title, a question for the user, and finishing.
fn task_tools() -> Vec<Value> {
    let own = json!({ "readOnlyHint": false, "destructiveHint": false, "openWorldHint": false });
    vec![
        json!({
            "name": "title",
            "title": "Name the task",
            "description": "Name the task, in two to five words, for the user's sidebar.",
            "inputSchema": {
                "type": "object",
                "properties": { "text": { "type": "string", "description": "The task's name." } },
                "required": ["text"],
            },
            "annotations": own,
        }),
        json!({
            "name": "ask",
            "title": "Ask the user",
            "description": "Ask the user a question, and wait for their answer. With up to three short answers to pick from, each shows as a button. Never ask for a password, a code or a card number.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "question": { "type": "string", "description": "The question, in a sentence." },
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
            "description": "End the task with its result. Give the tab the user should open, if there is one, such as a checkout only they may finish.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "result": { "type": "string", "description": "What you did or found, in a sentence or two." },
                    "tab": { "type": "integer", "description": "The tab to offer the user, from your Space." },
                },
                "required": ["result"],
            },
            "annotations": own,
        }),
    ]
}

/// The tools every agent may call.
fn shared() -> Value {
    let tab = json!({
        "type": "integer",
        "description": "The tab's id, from list_tabs or open_tab. Without it, the tab you used last, else your first."
    });
    let element = json!({
        "type": "string",
        "description": "The element's ref, from the latest outline, like e12."
    });
    let acts = json!({ "readOnlyHint": false, "destructiveHint": false, "openWorldHint": true });
    json!([
        {
            "name": "list_tabs",
            "title": "List tabs",
            "description": "List the tabs in your Space: each one's id, title and URL. * marks the tab you used last.",
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
            "description": "Close a tab in your Space. It goes to the archive, where the user can still find it.",
            "inputSchema": { "type": "object", "properties": { "tab": tab } },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": false,
                "openWorldHint": false,
            },
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
            "description": "The page's outline, from its accessibility tree. Elements you can act on have refs like [e12].",
            "inputSchema": { "type": "object", "properties": { "tab": tab } },
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
            "description": "Click an element by its ref. Answers with the page's outline.",
            "inputSchema": {
                "type": "object",
                "properties": { "ref": element, "tab": tab },
                "required": ["ref"],
            },
            "annotations": acts,
        },
        {
            "name": "type",
            "title": "Type",
            "description": "Type text into a field by its ref, in place of what it holds. submit presses Enter after. Answers with the page's outline.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ref": element,
                    "text": { "type": "string" },
                    "submit": { "type": "boolean" },
                    "tab": tab,
                },
                "required": ["ref", "text"],
            },
            "annotations": acts,
        },
        {
            "name": "press_key",
            "title": "Press key",
            "description": "Press a key in a tab, in the element that has the keyboard. Answers with the page's outline.",
            "inputSchema": {
                "type": "object",
                "properties": { "key": { "type": "string", "enum": page::KEYS }, "tab": tab },
                "required": ["key"],
            },
            "annotations": acts,
        },
        {
            "name": "scroll",
            "title": "Scroll",
            "description": "Scroll a page down or up most of its height, or to bring an element into view by its ref. Answers with the page's outline.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "direction": { "type": "string", "enum": ["down", "up"] },
                    "ref": element,
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
            "name": "run_steps",
            "title": "Run steps",
            "description": "Run several actions in a tab in one call, in order: each is checked as it would be alone. Stops at the first that fails, asks the user first, or is refused. Answers with what each did and the last one's outline.",
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
                                "key": { "type": "string", "enum": page::KEYS },
                                "url": { "type": "string" },
                                "direction": { "type": "string", "enum": ["down", "up"] },
                                "seconds": { "type": "number", "exclusiveMinimum": 0, "maximum": MAX_WAIT },
                            },
                            "required": ["action"],
                        },
                        "description": "Each step is an action and what that tool takes: click and scroll a ref, type a ref and text, press_key a key, navigate a url. wait waits for text to show in the page, up to seconds (5 if not given), or without text, waits the seconds.",
                    },
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
pub const STEPS: [&str; 7] = ["navigate", "snapshot", "click", "type", "press_key", "scroll", "wait"];
/// The steps one run_steps takes at most.
pub const MAX_STEPS: usize = 20;
/// How long a wait step waits for its text, in seconds, unless it says, and at most.
pub const WAIT: f64 = 5.0;
pub const MAX_WAIT: f64 = 30.0;

/// list_tabs' answer.
pub fn tab_list(tabs: &[TabInfo], current: Option<u64>) -> String {
    if tabs.is_empty() {
        return "Your Space has no tabs yet. The user can open pages in it for you.".into();
    }
    let lines = tabs.iter().map(|tab| {
        let mark = if Some(tab.id) == current { '*' } else { ' ' };
        format!("{mark} {}  {}  {}", tab.id, tab.title, tab.url)
    });
    lines.collect::<Vec<_>>().join("\n")
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
        Some(Value::Number(number)) => number.as_u64().map(Some).ok_or_else(not_a_tab),
        Some(Value::String(text)) => text.trim().parse().map(Some).map_err(|_| not_a_tab()),
        Some(_) => Err(not_a_tab()),
    }
}

fn not_a_tab() -> String {
    "tab is a tab's id, a number from list_tabs.".into()
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
        "press_key" => "Pressed a key",
        "scroll" => "Scrolled",
        "screenshot" => "Took a screenshot",
        "connected" => "Connected",
        "disconnected" => "Disconnected",
        "released" => "Let go of the tab",
        "taken back" => "You took the tab over",
        "handed back" => "You handed the tab back",
        "given" => "You gave it a tab",
        "stopped" => "You stopped it",
        "hand_off" => "Handed you a tab",
        "close_tab" => "Closed a tab",
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
