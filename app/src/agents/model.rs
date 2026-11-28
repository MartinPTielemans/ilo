//! Models ilo calls itself (#4465): a maker's own API with the user's key, or a server on this
//! computer that speaks OpenAI's chat completions. Each maker's adapter is a `Wire`, which only
//! turns a `Request` into a body and its stream's events into `Delta`s, so a new provider is one
//! more Wire and nothing generic changes. http.rs is the one place that touches the network, with
//! ureq on a thread of the turn's own and no async runtime, and it calls only the account's own
//! endpoint, never an ilo host. turns.rs runs a conversation over any of them.

use futures::channel::mpsc::UnboundedSender;
use serde_json::Value;

pub mod compat;
pub mod http;
mod sse;
#[cfg(test)]
mod tests;

/// What a model can do, as its maker's listing says, or its adapter's table where the listing
/// doesn't.
#[derive(Clone, Debug, PartialEq)]
pub struct Caps {
    pub tools: bool,
    pub parallel_tools: bool,
    pub images_in: bool,
    /// It answers in a JSON schema it's given, rather than being asked to in its instructions.
    pub structured: bool,
    pub thinking: Thinking,
    /// The effort levels it takes, in order, or none, where the picker isn't shown and no level
    /// is sent.
    pub efforts: Vec<String>,
    /// Its context window, in tokens.
    pub context: u32,
    pub steer: Steer,
}

impl Caps {
    /// A model its listing says nothing of: tools, text, and the context given.
    pub fn plain(context: u32) -> Self {
        Self {
            tools: true,
            parallel_tools: true,
            images_in: false,
            structured: false,
            thinking: Thinking::None,
            efforts: Vec::new(),
            context,
            steer: Steer::BetweenCalls,
        }
    }
}

/// What a model shows of its reasoning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Thinking {
    None,
    /// Its maker's summary of it.
    Summaries,
    Full,
}

/// Where a message the user sends while the model works goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Steer {
    /// In with the next tool results, so the model reads it before its next step.
    BetweenCalls,
    /// After the turn ends, as the next message.
    QueueOnly,
}

/// A tool the model may call, as the browser's server lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Tool {
    pub name: String,
    pub description: String,
    /// Its arguments' JSON schema.
    pub schema: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    Text(String),
    /// A picture, as its media type and its bytes in base64.
    Image {
        media: String,
        base64: String,
    },
}

/// Reasoning its maker signed, which goes back to that maker alone, unchanged. Another maker's
/// adapter leaves it out.
#[derive(Clone, PartialEq)]
pub struct Signed {
    /// The maker's id, as its Wire gives it.
    pub maker: String,
    pub text: String,
    /// Opaque, as the maker sent it.
    pub signature: String,
}

// A signature is the maker's to read, and a log has no use for it.
impl std::fmt::Debug for Signed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Signed")
            .field("maker", &self.maker)
            .field("text", &self.text)
            .finish_non_exhaustive()
    }
}

/// One entry of a conversation, in the order it happened, whichever maker is asked next.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    User(Vec<Part>),
    /// What the model said.
    Said(String),
    Thought(Signed),
    Call {
        id: String,
        name: String,
        args: Value,
    },
    /// A call's answer. `error` when the call failed or was stopped, with why in its parts.
    Result {
        id: String,
        parts: Vec<Part>,
        error: bool,
    },
}

/// One call of a model.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub instructions: String,
    pub items: Vec<Item>,
    pub tools: Vec<Tool>,
    /// One of its Caps' efforts, or None for its maker's own.
    pub effort: Option<String>,
    /// The JSON schema its answer is in, for a model whose Caps are `structured`.
    pub schema: Option<Value>,
    /// Whether it's asked to show its reasoning, for a model whose Caps say it thinks.
    pub thinking: bool,
    /// The most it may write, in tokens.
    pub max_out: u32,
}

/// What a model's stream says, as its adapter reads it.
#[derive(Clone, Debug, PartialEq)]
pub enum Delta {
    Text(String),
    Thinking(String),
    /// A finished piece of signed reasoning, to keep in the conversation before what follows it.
    Signed(Signed),
    Call {
        id: String,
        name: String,
        args: Value,
    },
    Usage {
        input: u32,
        output: u32,
    },
    Ended(Stop),
    Failed(Failure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Done,
    /// It called tools, whose results it waits for.
    Calls,
    /// It reached `max_out`.
    Length,
}

/// Why a call of a model failed.
#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    /// The key or sign-in was refused (401, 403).
    Refused,
    /// The account is out of credit (402).
    Credit,
    /// Too many requests, until the time given, in seconds since 1970, if the maker says (429).
    Limit(Option<u64>),
    /// The maker's servers are busy or failing (500, 502, 503, 529).
    Busy,
    /// No answer: no connection, none in time, or the stream broke off.
    Unreachable,
    /// The model is gone (404).
    Withdrawn,
    /// The maker said why, in its words.
    Other(String),
}

impl Failure {
    /// What an HTTP status other than 2xx means: `retry_after` in seconds, `now` in seconds since
    /// 1970, and the maker's message from its body.
    pub fn of_status(status: u16, retry_after: Option<u64>, now: u64, message: String) -> Self {
        match status {
            401 | 403 => Self::Refused,
            402 => Self::Credit,
            404 => Self::Withdrawn,
            429 => Self::Limit(retry_after.map(|after| now + after)),
            500 | 502 | 503 | 529 => Self::Busy,
            _ => Self::Other(message),
        }
    }
}

/// A model its maker lists.
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub id: String,
    /// Its maker's name for it.
    pub name: String,
    pub caps: Caps,
}

/// One maker's API: the bodies it takes and the stream it sends, with no IO of its own.
pub trait Wire: Sync {
    /// The maker's id, which signed reasoning carries.
    fn maker(&self) -> &'static str;
    /// The maker's own address, an account's endpoint unless the user names another; none for a
    /// server the user always names.
    fn home(&self) -> Option<&'static str> {
        None
    }
    /// The path its models are listed at, after the endpoint's base.
    fn listing(&self) -> &'static str;
    /// The headers a key goes in.
    fn auth(&self, key: &str) -> Vec<(&'static str, String)>;
    fn models(&self, listing: &Value) -> Vec<Listed>;
    /// The path a turn is posted to and its body.
    fn call(&self, model: &str, request: &Request) -> (String, Value);
    /// A reader for one turn's stream.
    fn reader(&self) -> Box<dyn Reader>;
}

/// One turn's stream, an event at a time.
pub trait Reader: Send {
    /// A server-sent event: its name, if it has one, and its data.
    fn event(&mut self, event: Option<&str>, data: &str) -> Vec<Delta>;
    /// The stream closed: what its last events left to say.
    fn end(&mut self) -> Vec<Delta> {
        Vec::new()
    }
}

/// A model a conversation runs on, by turns numbered by the conversation (turns.rs).
pub trait Model {
    fn caps(&self) -> &Caps;
    /// Starts turn `turn`, whose deltas come on `out` with its number.
    fn turn(&self, turn: u32, request: Request, out: UnboundedSender<(u32, Delta)>);
    /// Stops turn `turn`, if it's still going: nothing more of it comes.
    fn cancel(&self, turn: u32);
}

/// The levels of a table by model id prefix, the longest that matches, for a maker whose listing
/// doesn't say.
fn efforts_by_prefix(table: &[(&str, &[&str])], model: &str) -> Vec<String> {
    table
        .iter()
        .filter(|(prefix, _)| model.starts_with(prefix))
        .max_by_key(|(prefix, _)| prefix.len())
        .map(|(_, levels)| levels.iter().map(|level| level.to_string()).collect())
        .unwrap_or_default()
}

/// The text of a call's result parts, joined, for a maker that takes text alone there.
fn text_of(parts: &[Part]) -> String {
    parts
        .iter()
        .filter_map(|part| match part {
            Part::Text(text) => Some(text.as_str()),
            Part::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The name of the call `id` names, which a maker that matches results by name needs.
fn call_name<'a>(items: &'a [Item], id: &str) -> &'a str {
    items
        .iter()
        .find_map(|item| match item {
            Item::Call { id: call, name, .. } if call == id => Some(name.as_str()),
            _ => None,
        })
        .unwrap_or_default()
}

/// An id for a call its maker sent none, unique across conversations, as a maker switched to
/// asks of its calls'.
fn fresh_id() -> String {
    let mut bytes = [0u8; 6];
    getrandom::fill(&mut bytes).expect("the system has no random numbers");
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("call-{hex}")
}

/// A tool call's arguments as streamed: its JSON, or an empty object where it broke off.
fn args_of(json: &str) -> Value {
    match json.trim() {
        "" => Value::Object(Default::default()),
        json => serde_json::from_str(json).unwrap_or_else(|_| Value::Object(Default::default())),
    }
}
