//! The `point` tool (#2352): an agent asks the user to show it something on a page. The ask waits
//! on the agent's card, as its other questions do: an outside agent's on the card under the URL
//! field, a ⌘T task's on its card at the sidebar's foot. The user's Point turns Point on in the tab
//! in front of them (bc_tab_point), the windows show its mode, and the user's Send is the answer,
//! their notes and nothing else. The user's press and their Send are its permission: nothing goes
//! without them. Every other way it ends answers `point::NOTHING`, so the agent learns nothing of a
//! tab that isn't its own, not even whether the user pressed Point.

use std::sync::mpsc;

use base64::Engine as _;
use gpui::{Context, SharedString, Task};
use serde_json::Value;

use super::task::{Message, State, Who};
use super::{ASK_WAIT, Agents, HANDOFF_WAIT, Question, ToolResult, page};
use crate::point::{self, Note};

/// Settings › Agents' Pointing switch is off, which the agent may know: it's about the tool, not
/// a tab.
pub(super) const POINT_OFF: &str = "The user turned off pointing for agents in Settings › Agents.";
const ASKED_ALREADY: &str = "You asked the user to point already. Wait for their answer.";

/// An agent's `point` call, waiting on the user.
pub(super) struct PointAsk {
    pub(super) id: u64,
    pub(super) agent: String,
    question: SharedString,
    /// The tab the user points in, from their press on Point.
    tab: Option<u64>,
    reply: mpsc::Sender<ToolResult>,
    /// Answers `point::NOTHING` once ASK_WAIT passes before Point, or HANDOFF_WAIT after it, as a
    /// handoff's user has: the call doesn't hold the agent while the user is away.
    _timer: Task<()>,
}

/// Point's mode, as the windows show it: the window that holds `tab` frames its page, shows the
/// foot bar and draws the marks.
#[derive(Clone, Debug, PartialEq)]
pub struct PointMode {
    pub id: u64,
    pub tab: u64,
    /// The agent's id, which the core's calls take, and its name, as its Space has it.
    pub from: SharedString,
    pub agent: SharedString,
    pub question: SharedString,
}

/// A task's ask to point, for its card: the ask, and whether the user is pointing for it, when the
/// card keeps the question and drops its chips, which the foot bar has.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TaskPoint {
    pub id: u64,
    pub pointing: bool,
}

impl Agents {
    /// The `point` call: the ask waits on the agent's card until the user answers.
    pub(super) fn point_tool(
        &mut self,
        agent: &str,
        arguments: &Value,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let question = (arguments.get("question").and_then(Value::as_str))
            .unwrap_or("")
            .trim();
        let refused = match () {
            _ if question.is_empty() => Some("question is what you ask the user to point at."),
            _ if self.point_off => Some(POINT_OFF),
            _ if self.points.iter().any(|ask| ask.agent == agent) => Some(ASKED_ALREADY),
            _ => None,
        };
        if let Some(why) = refused {
            self.note(agent, "point", "", why);
            _ = reply.send(ToolResult::error(why));
            return;
        }
        let id = self.next_request;
        self.next_request += 1;
        let question: SharedString = page::clip(question, 300).into();
        // check.sh reads this line.
        log!("browser: agent {agent} asks to point {id}");
        if let Some(task) = self.task_mut(agent) {
            task.messages
                .push(Message::new(Who::Asked, question.to_string()));
            task.state = State::Asking {
                question: question.clone(),
                answers: Vec::new(),
            };
        }
        self.points.push(PointAsk {
            id,
            agent: agent.into(),
            question,
            tab: None,
            reply,
            _timer: point_timer(id, ASK_WAIT, cx),
        });
        self.note(agent, "point", "", "");
        self.pointed(cx);
    }

    /// The user pressed Point on the ask `id`, with `tab` in front of them: Point turns on there,
    /// or the press does nothing, the card having said why not before it.
    pub fn point_start(&mut self, id: u64, tab: u64, cx: &mut Context<Self>) {
        // One at a time: the foot bar and the keys are the one mode's.
        if self.points.iter().any(|ask| ask.tab.is_some()) {
            return;
        }
        let Some(at) = self.points.iter().position(|ask| ask.id == id) else {
            return;
        };
        let can = self.browser.point(tab, &self.points[at].agent, true);
        if can != 0 {
            log!("browser: point {id} can't start in {tab}: {can}");
            return;
        }
        let ask = &mut self.points[at];
        ask.tab = Some(tab);
        ask._timer = point_timer(id, HANDOFF_WAIT, cx);
        // check.sh reads this line.
        log!("browser: point {id} started in {tab}");
        self.pointed(cx);
    }

    /// The user's Send: their notes are the agent's answer, as the note's panel showed them.
    pub fn point_send(&mut self, id: u64, notes: Vec<Note>, cx: &mut Context<Self>) {
        if notes.is_empty() {
            return;
        }
        let Some(ask) = self.take_point(id) else {
            return;
        };
        let (text, pictures) = point::answer(&notes);
        let engine = base64::engine::general_purpose::STANDARD;
        let pngs = pictures.iter().map(|png| engine.encode(png)).collect();
        _ = ask.reply.send(ToolResult {
            text,
            error: false,
            image: None,
            pngs,
        });
        let address = notes[0].element.address.clone();
        let site = super::site(&address);
        let count = match notes.len() {
            1 => "1 element".to_owned(),
            count => format!("{count} elements"),
        };
        // Its row opens to the page and the lines sent, as a request's does.
        let entry = (address.as_str(), point::sent(&notes));
        self.note_entry(&ask.agent, "pointed", &site, &count, "", entry);
        // check.sh reads this line.
        log!("browser: point {id} sent {}", notes.len());
        self.point_over(ask, cx);
    }

    /// Not Now, esc, the ask timing out or the agent going: the agent hears nothing was pointed at.
    pub fn point_drop(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(ask) = self.take_point(id) else {
            return;
        };
        _ = ask.reply.send(ToolResult::text(point::NOTHING));
        // check.sh reads this line.
        log!("browser: point {id} unanswered");
        self.point_over(ask, cx);
    }

    /// The core ended Point on `tab` (tab_point_ended): a new page, the tab moved or closed.
    pub fn point_ended(&mut self, tab: u64, cx: &mut Context<Self>) {
        let Some(ask) = self.points.iter_mut().find(|ask| ask.tab == Some(tab)) else {
            return;
        };
        // It's off already.
        ask.tab = None;
        let id = ask.id;
        self.point_drop(id, cx);
    }

    /// The agent went: its ask goes unanswered.
    pub(super) fn forget_points(&mut self, agent: &str, cx: &mut Context<Self>) {
        let ids: Vec<u64> = (self.points.iter())
            .filter(|ask| ask.agent == agent)
            .map(|ask| ask.id)
            .collect();
        for id in ids {
            self.point_drop(id, cx);
        }
    }

    fn take_point(&mut self, id: u64) -> Option<PointAsk> {
        let at = self.points.iter().position(|ask| ask.id == id)?;
        Some(self.points.remove(at))
    }

    fn point_over(&mut self, ask: PointAsk, cx: &mut Context<Self>) {
        if let Some(tab) = ask.tab {
            self.browser.point(tab, &ask.agent, false);
        }
        if let Some(task) = self.task_mut(&ask.agent)
            && task.asking.is_none()
            && matches!(task.state, State::Asking { .. })
        {
            task.state = State::Working;
        }
        self.pointed(cx);
    }

    /// Shows the asks and the mode as they are now, in every window.
    fn pointed(&self, cx: &mut Context<Self>) {
        self.browser.pointing(self.point_mode(), cx);
        self.asked(cx);
    }

    /// The mode the user is in, if they pressed Point.
    pub fn point_mode(&self) -> Option<PointMode> {
        let ask = self.points.iter().find(|ask| ask.tab.is_some())?;
        Some(PointMode {
            id: ask.id,
            tab: ask.tab?,
            from: ask.agent.clone().into(),
            agent: self.agent_name(&ask.agent),
            question: ask.question.clone(),
        })
    }

    /// The ids of `agent`'s asks to point, for the order its prompts show in.
    pub(super) fn point_ids(&self, agent: &str) -> impl Iterator<Item = u64> + '_ {
        let agent = agent.to_owned();
        (self.points.iter())
            .filter(move |ask| ask.agent == agent)
            .map(|ask| ask.id)
    }

    /// An outside agent's ask to point, as the windows' card asks it, when it's the agent's first
    /// prompt and the user isn't pointing for it yet. A task's asks on its own card.
    pub(super) fn point_question(&self) -> Option<Question> {
        let ask = self.points.iter().find(|ask| {
            ask.tab.is_none()
                && !super::is_task(&ask.agent)
                && self.first_prompt(&ask.agent) == Some(ask.id)
        })?;
        Some(Question {
            id: ask.id,
            agent: self.agent_name(&ask.agent),
            from: ask.agent.clone().into(),
            what: ask.question.clone(),
            point: true,
            ..Question::default()
        })
    }

    /// The task `agent`'s ask to point, for its card's chips.
    pub fn task_point(&self, agent: &str) -> Option<TaskPoint> {
        let ask = self.points.iter().find(|ask| ask.agent == agent)?;
        Some(TaskPoint {
            id: ask.id,
            pointing: ask.tab.is_some(),
        })
    }

    /// Settings › Agents' Pointing: off, the tool refuses with POINT_OFF, and an ask already
    /// waiting goes unanswered.
    pub fn set_point_on(&mut self, on: bool, cx: &mut Context<Self>) {
        if on != self.point_off {
            return;
        }
        self.point_off = !on;
        if !on {
            let ids: Vec<u64> = self.points.iter().map(|ask| ask.id).collect();
            for id in ids {
                self.point_drop(id, cx);
            }
        }
        self.save();
        cx.notify();
    }

    pub fn point_on(&self) -> bool {
        !self.point_off
    }
}

fn point_timer(id: u64, wait: std::time::Duration, cx: &mut Context<Agents>) -> Task<()> {
    cx.spawn(async move |agents, cx| {
        cx.background_executor().timer(wait).await;
        _ = agents.update(cx, |agents, cx| agents.point_drop(id, cx));
    })
}
