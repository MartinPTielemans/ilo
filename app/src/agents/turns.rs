//! A conversation with a model ilo calls itself (#4465), with no IO of its own: it takes what the
//! user says, the model's deltas and the tools' results, and gives the steps to take, so every
//! order of them is a test. Its caller calls the model, runs each tool call through the browser's
//! one gate (`Agents::check`) and shows the outputs as it shows a CLI's.
//!
//! A turn's tool calls run as they come, and the model is called again once it has ended and every
//! result is in, in the order it called them. What the user sends meanwhile goes in with those
//! results where the model takes that, and otherwise waits for the turn's end. Stopping keeps the
//! conversation whole: each call still out gets a result saying it was stopped. A model's deltas
//! carry its turn's number, so one stopped or replaced is never heard.

use std::collections::VecDeque;
use std::time::Instant;

use serde_json::Value;

use super::model::{Caps, Delta, Failure, Item, Part, Request, Steer, Thinking, Tool};
use super::runner::Out;

#[cfg(test)]
mod tests;

/// The share of its context a request may fill before older results make room.
const ROOM: (u32, u32) = (4, 5);
/// What a picture is taken to cost in tokens, as makers price one of about a megapixel.
const PICTURE: usize = 1600;
/// What the model is told, as a call it made is stopped. Its words, never the user's.
const STOPPED: &str = "The user stopped this before it finished.";
/// What the model is told when it's asked to sum up the conversation so far.
const SUMMARIZE: &str = "Summarize the conversation so far for yourself to go on from: what the \
     user asked, what you found and did, and what is left. Keep names, addresses and numbers \
     exactly. Write only the summary.";
/// What the summary is headed by in the conversation it stands in for.
const SUMMARY: &str = "The conversation before this point, summarized:";

/// What the caller does next.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Call the model: its deltas come back to `delta` with this turn's number.
    Model {
        turn: u32,
        request: Request,
    },
    /// Stop the model's turn: nothing more of it is heard.
    Cancel(u32),
    /// Run a tool call through the gate, and hand its answer to `result`.
    Run {
        id: String,
        name: String,
        args: Value,
    },
    Out(Out),
    /// The turn ended, and the conversation waits for the user: well, or with why not.
    Ended(Option<Failure>),
}

/// A call made and its result, once it's in.
struct Waiting {
    id: String,
    result: Option<(Vec<Part>, bool)>,
}

pub struct Turns {
    /// The conversation as the model is shown it, older results made room for.
    pub items: Vec<Item>,
    instructions: String,
    tools: Vec<Tool>,
    effort: Option<String>,
    max_out: u32,
    caps: Caps,
    /// Messages for after the turn, in the order sent.
    queued: VecDeque<Vec<Part>>,
    /// Messages for the model's next step.
    steer: Vec<Vec<Part>>,
    /// The last turn's number.
    turn: u32,
    /// The model's turn being streamed.
    calling: Option<u32>,
    /// The turn summing up the conversation before `items[cut]`, and its summary so far.
    summing: Option<(u32, usize, String)>,
    /// Whether the conversation was summed up since the user last spoke, so it isn't again.
    summed: bool,
    waiting: Vec<Waiting>,
    writing: String,
    thinking: Option<(String, Instant)>,
}

impl Turns {
    pub fn new(
        caps: Caps,
        instructions: String,
        tools: Vec<Tool>,
        effort: Option<String>,
        max_out: u32,
    ) -> Self {
        Self {
            items: Vec::new(),
            instructions,
            tools,
            effort,
            max_out,
            caps,
            queued: VecDeque::new(),
            steer: Vec::new(),
            turn: 0,
            calling: None,
            summing: None,
            summed: false,
            waiting: Vec::new(),
            writing: String::new(),
            thinking: None,
        }
    }

    /// Whether a turn is under way.
    pub fn busy(&self) -> bool {
        self.calling.is_some() || self.summing.is_some() || !self.waiting.is_empty()
    }

    /// The messages waiting for the turn's end.
    pub fn queued(&self) -> impl Iterator<Item = &Vec<Part>> {
        self.queued.iter()
    }

    /// The user says something: to the model now if it's idle, and otherwise with the next tool
    /// results if `now` and the model takes that, or after the turn.
    pub fn say(&mut self, parts: Vec<Part>, now: bool) -> Vec<Step> {
        if self.busy() {
            match now && self.caps.steer == Steer::BetweenCalls {
                true => self.steer.push(parts),
                false => self.queued.push_back(parts),
            }
            return Vec::new();
        }
        // What waited from a turn the user stopped goes first, as it was sent first.
        let queued: Vec<_> = self.queued.drain(..).collect();
        self.items.extend(queued.into_iter().map(Item::User));
        self.items.push(Item::User(parts));
        self.summed = false;
        self.ask()
    }

    /// What the model's turn `turn` streamed, at `at`.
    pub fn delta(&mut self, turn: u32, delta: Delta, at: Instant) -> Vec<Step> {
        if let Some((summing, cut, summary)) = &mut self.summing
            && *summing == turn
        {
            match delta {
                Delta::Text(text) => summary.push_str(&text),
                Delta::Ended(_) => {
                    let (cut, summary) = (*cut, std::mem::take(summary));
                    self.summing = None;
                    self.summed = true;
                    if !summary.trim().is_empty() {
                        let head = Item::User(vec![Part::Text(format!(
                            "{SUMMARY}\n\n{}",
                            summary.trim()
                        ))]);
                        self.items.splice(..cut, [head]);
                    }
                    return self.ask();
                }
                Delta::Failed(failure) => {
                    self.summing = None;
                    return self.fail(failure);
                }
                _ => {}
            }
            return Vec::new();
        }
        if self.calling != Some(turn) {
            return Vec::new();
        }
        match delta {
            Delta::Thinking(text) => {
                self.thinking
                    .get_or_insert_with(|| (String::new(), at))
                    .0
                    .push_str(&text);
                Vec::new()
            }
            Delta::Signed(signed) => {
                let steps = self.thought(at);
                self.items.push(Item::Thought(signed));
                steps
            }
            Delta::Text(text) => {
                let mut steps = self.thought(at);
                self.writing.push_str(&text);
                if !self.writing.trim().is_empty() {
                    steps.push(Step::Out(Out::Writing(self.writing.trim().to_string())));
                }
                steps
            }
            Delta::Call { id, name, args } => {
                let mut steps = self.thought(at);
                steps.extend(self.said(true));
                self.items.push(Item::Call {
                    id: id.clone(),
                    name: name.clone(),
                    args: args.clone(),
                });
                self.waiting.push(Waiting {
                    id: id.clone(),
                    result: None,
                });
                steps.push(Step::Run { id, name, args });
                steps
            }
            Delta::Usage { .. } => Vec::new(),
            Delta::Ended(_) => {
                let mut steps = self.thought(at);
                steps.extend(self.said(false));
                self.calling = None;
                steps.extend(match self.waiting.is_empty() {
                    true => self.next(),
                    false => self.go_on(),
                });
                steps
            }
            Delta::Failed(failure) => {
                let mut steps = self.thought(at);
                steps.extend(self.said(false));
                self.calling = None;
                steps.extend(self.fail(failure));
                steps
            }
        }
    }

    /// A tool call's answer, `error` where it failed. One for a call no longer waited for, as the
    /// user stopped its turn, isn't heard.
    pub fn result(&mut self, id: &str, parts: Vec<Part>, error: bool) -> Vec<Step> {
        let Some(waiting) = self
            .waiting
            .iter_mut()
            .find(|waiting| waiting.id == id && waiting.result.is_none())
        else {
            return Vec::new();
        };
        waiting.result = Some((parts, error));
        self.go_on()
    }

    /// The user stops the turn. What waited for its end still waits.
    pub fn stop(&mut self) -> Vec<Step> {
        if !self.busy() {
            return Vec::new();
        }
        let mut steps: Vec<Step> = [self.calling.take(), self.summing.take().map(|it| it.0)]
            .into_iter()
            .flatten()
            .map(Step::Cancel)
            .collect();
        steps.extend(self.thought(Instant::now()));
        steps.extend(self.said(false));
        self.answer_waiting();
        steps.push(Step::Ended(None));
        steps
    }

    /// The model is called again once its turn has ended and every result is in, with what the
    /// user sent meanwhile.
    fn go_on(&mut self) -> Vec<Step> {
        if self.calling.is_some() || self.waiting.iter().any(|it| it.result.is_none()) {
            return Vec::new();
        }
        for Waiting { id, result } in self.waiting.drain(..) {
            let (parts, error) = result.unwrap_or_default();
            self.items.push(Item::Result { id, parts, error });
        }
        let steer: Vec<_> = self.steer.drain(..).collect();
        self.items.extend(steer.into_iter().map(Item::User));
        self.ask()
    }

    /// The turn ended with nothing to run: the next message, or the user's turn.
    fn next(&mut self) -> Vec<Step> {
        self.requeue();
        match self.queued.pop_front() {
            Some(parts) => {
                self.items.push(Item::User(parts));
                self.summed = false;
                self.ask()
            }
            None => vec![Step::Ended(None)],
        }
    }

    fn fail(&mut self, failure: Failure) -> Vec<Step> {
        self.answer_waiting();
        vec![Step::Ended(Some(failure))]
    }

    /// Each call still out is answered as stopped, so the conversation stays one a maker takes.
    fn answer_waiting(&mut self) {
        for Waiting { id, result } in self.waiting.drain(..) {
            let (parts, error) = result.unwrap_or_else(|| (vec![Part::Text(STOPPED.into())], true));
            self.items.push(Item::Result { id, parts, error });
        }
        self.requeue();
    }

    /// Messages meant for a next step that won't come wait for the next turn, ahead of the rest.
    fn requeue(&mut self) {
        for parts in self.steer.drain(..).rev() {
            self.queued.push_front(parts);
        }
    }

    /// The model is called on the conversation, made room for first.
    fn ask(&mut self) -> Vec<Step> {
        self.make_room();
        self.turn += 1;
        if self.estimate() > self.room()
            && !self.summed
            && let Some(cut) = self.cut()
        {
            self.summing = Some((self.turn, cut, String::new()));
            let mut items = self.items[..cut].to_vec();
            items.push(Item::User(vec![Part::Text(SUMMARIZE.into())]));
            let request = Request {
                items,
                effort: None,
                schema: None,
                ..self.request()
            };
            return vec![Step::Model {
                turn: self.turn,
                request,
            }];
        }
        self.calling = Some(self.turn);
        self.writing.clear();
        vec![
            Step::Out(Out::Writing(String::new())),
            Step::Model {
                turn: self.turn,
                request: self.request(),
            },
        ]
    }

    fn request(&self) -> Request {
        Request {
            instructions: self.instructions.clone(),
            items: self.items.clone(),
            tools: self.tools.clone(),
            effort: self.effort.clone(),
            schema: None,
            thinking: self.caps.thinking != Thinking::None,
            max_out: self.max_out,
        }
    }

    /// What it wrote is said, before any call it goes on to.
    fn said(&mut self, calling: bool) -> Vec<Step> {
        let text = std::mem::take(&mut self.writing);
        let text = text.trim();
        let mut steps = Vec::new();
        if !text.is_empty() {
            self.items.push(Item::Said(text.to_string()));
            steps.push(Step::Out(Out::Said(text.to_string())));
        }
        if calling {
            steps.push(Step::Out(Out::Calling));
        }
        steps
    }

    /// Its thinking is a thought once something else comes, which took until then.
    fn thought(&mut self, at: Instant) -> Vec<Step> {
        match self.thinking.take() {
            Some((text, since)) if !text.trim().is_empty() => vec![Step::Out(Out::Thought {
                text: text.trim().to_string(),
                took: at.saturating_duration_since(since),
            })],
            _ => Vec::new(),
        }
    }

    fn room(&self) -> usize {
        (self.caps.context as usize * ROOM.0 as usize / ROOM.1 as usize)
            .saturating_sub(self.max_out as usize)
    }

    /// The tokens the request would take, at four characters to one.
    fn estimate(&self) -> usize {
        let parts = |parts: &[Part]| -> usize {
            parts
                .iter()
                .map(|part| match part {
                    Part::Text(text) => text.len() / 4,
                    Part::Image { .. } => PICTURE,
                })
                .sum()
        };
        let items: usize = self
            .items
            .iter()
            .map(|item| match item {
                Item::User(it) | Item::Result { parts: it, .. } => parts(it),
                Item::Said(text) => text.len() / 4,
                Item::Thought(signed) => (signed.text.len() + signed.signature.len()) / 4,
                Item::Call { name, args, .. } => (name.len() + args.to_string().len()) / 4,
            })
            .sum();
        let tools: usize = self
            .tools
            .iter()
            .map(|tool| {
                (tool.name.len() + tool.description.len() + tool.schema.to_string().len()) / 4
            })
            .sum();
        self.instructions.len() / 4 + items + tools
    }

    /// The oldest results give way to a line naming them, until the request fits. The last step's
    /// results and what the user said since stay, as the model is about to read them.
    fn make_room(&mut self) {
        let fresh = self.items.len()
            - self
                .items
                .iter()
                .rev()
                .take_while(|item| matches!(item, Item::Result { .. } | Item::User(_)))
                .count();
        // Each call by its step, counting from the conversation's first.
        let steps: Vec<(String, String)> = self
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Call { id, name, .. } => Some((id.clone(), name.clone())),
                _ => None,
            })
            .collect();
        for index in 0..fresh {
            if self.estimate() <= self.room() {
                return;
            }
            let Item::Result { id, parts, .. } = &self.items[index] else {
                continue;
            };
            let Some(step) = steps.iter().position(|(call, _)| call == id) else {
                continue;
            };
            let stub = format!(
                "[{} result from step {} dropped to save room]",
                steps[step].1,
                step + 1
            );
            if matches!(parts.as_slice(), [Part::Text(text)] if *text == stub) {
                continue;
            }
            if let Item::Result { parts, .. } = &mut self.items[index] {
                *parts = vec![Part::Text(stub)];
            }
        }
    }

    /// Where the turns to sum up end: the start of the user's latest turn, where there's one
    /// before it.
    fn cut(&self) -> Option<usize> {
        (1..self.items.len()).rev().find(|&index| {
            matches!(self.items[index], Item::User(_))
                && !matches!(self.items[index - 1], Item::Result { .. })
        })
    }
}
