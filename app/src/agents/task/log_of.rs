//! A kept thread's log as its card's folder stores it at the thread's Done (kept/log.rs): the
//! conversation as the panel draws it closed, line for line, by the panel's own grouping and
//! words, so what comes back is what was there. Left out by rule, not by size: a file's or a
//! folder's chip, anything of a page an agent was kept from but its site, and the same of a tab
//! the user handed the thread, the words a step opens to, the assistant's note on a step, and a
//! thought. What was typed into a field is in a step's words only where
//! `page::Element::shows_typed` let the step draw it.

use gpui::{App, SharedString};

use super::ilo::{IloKind, IloLine};
use super::{Line, OnPage, Opens, Task, Trace, TraceItem, TraceStep};
use crate::agents::Agents;
use crate::agents::kept::log::Record;
use crate::agents::lines::STOPS;
use crate::agents::tools;
use crate::shell::conversation::pages::{Page, Paged, paged};
use crate::shell::conversation::words;

impl Agents {
    /// `task`'s conversation as its kept log's records.
    pub(super) fn log_of(&self, task: &Task, cx: &App) -> Vec<Record> {
        let Some(talk) = self.conversation_with(&task.agent, false, cx) else {
            return Vec::new();
        };
        // The page the user asked on, as the chip over what they asked; one the agent was kept from,
        // by its site alone. So is it in a thread the user ever handed a tab: the page asked on may
        // be that tab, and nothing here says which tab it was.
        let chip = (talk.page.as_ref()).map(|asked| {
            let site = super::super::site(&asked.url);
            let users = task.sends.had_users();
            match users || self.kept_from(task, None, &site, cx) || asked.title.is_empty() {
                true => site,
                false => asked.title.to_string(),
            }
        });
        // What was asked heads the log, over the lines, with the page it was asked on.
        let mut records = Vec::new();
        if !talk.asked.is_empty() {
            records.push(Record::You(talk.asked.to_string()));
            records.extend(chip.map(Record::Chip));
        }
        for (at, line) in talk.lines.iter().enumerate() {
            match line {
                Line::You(text) => records.push(Record::You(text.to_string())),
                Line::Said(text) => {
                    records.push(match talk.finished.iter().find(|(index, _)| *index == at) {
                        Some((_, when)) => Record::Answer {
                            text: text.to_string(),
                            at: when.to_string(),
                        },
                        None => Record::Said(text.to_string()),
                    })
                }
                Line::Trace(trace) => {
                    records.push(Record::Fold { took: trace.took });
                    for each in paged(&trace.items) {
                        match each {
                            Paged::Page(page) => {
                                let kept_from = self.kept_from(task, page.tab, &page.site, cx);
                                records.push(Record::Page(page_title(&page, kept_from)));
                                (records).extend(
                                    page.shown().map(|step| self.step_of(task, step, kept_from)),
                                );
                            }
                            Paged::Item(_, TraceItem::Said(text)) => {
                                records.push(Record::Aside(text.to_string()))
                            }
                            Paged::Item(_, TraceItem::Step(step))
                            | Paged::Item(_, TraceItem::Helper { step, .. }) => {
                                records.push(self.on_its_page(task, step, cx))
                            }
                        }
                    }
                }
                Line::Step(step) => records.push(self.on_its_page(task, step, cx)),
                Line::Thinking { .. } => {}
                Line::Ilo(line) if logged(&line.kind) => {
                    records.push(Record::Ilo(ilo_stored(task, line)))
                }
                Line::Ilo(_) => {}
            }
        }
        records
    }

    /// A step's record, judged by the page it names, if any. The same step on several tabs is one
    /// line, which names what it clicked or typed in: where one of them is a tab the user handed
    /// the thread, it's judged as on that tab.
    fn on_its_page(&self, task: &Task, step: &TraceStep, cx: &App) -> Record {
        let users = |step: &TraceStep| {
            let tab = step.page.as_ref().and_then(|page| page.tab);
            tab.is_some_and(|tab| task.sends.was_users(tab))
        };
        let kept_from = (step.page.as_ref())
            .is_some_and(|page| self.kept_from(task, page.tab, &page.site, cx))
            || matches!(&step.opens, Opens::Steps(each) if each.iter().any(users));
        self.step_of(task, step, kept_from)
    }

    /// Whether the agent was kept from the page on `tab` at `site`: a site the user put off limits,
    /// a page off the web or another Space's, or a page a session's agent is kept from now. A tab
    /// the user handed the thread is kept as one is, by its site alone, held still or gone back:
    /// it is theirs, and its title and what is on it are never kept (handed.rs, `Sends::users`).
    fn kept_from(&self, task: &Task, tab: Option<u64>, site: &str, cx: &App) -> bool {
        self.fence_of(site).is_some()
            || tab.is_some_and(|tab| {
                task.sends.was_users(tab)
                    || self.withheld(&task.agent, tab)
                    || self.off_limits(&task.agent, tab, cx).is_some()
            })
    }

    /// A step's line as drawn, which shows what it typed only where `shows_typed` let it, and on a
    /// page the agent was kept from, the product's own words for its kind and nothing of the page.
    fn step_of(&self, task: &Task, step: &TraceStep, kept_from: bool) -> Record {
        let words = match kept_from {
            true => {
                let did = task.trace.get(step.key).map(|traced| &traced.did);
                tools::describe(did.map_or("", |did| did.tool.as_str())).to_string()
            }
            false => step.title.to_string(),
        };
        Record::Step {
            glyph: step.glyph,
            words,
            came: match kept_from {
                true => String::new(),
                false => step.description.to_string(),
            },
            failed: step.failed,
        }
    }
}

/// A page's line as drawn: the title its steps kept while it had one, else its site; one the
/// agent was kept from, its site alone. Never its tab's title at Done, which is whatever page the
/// tab went on to.
fn page_title(page: &Page, kept_from: bool) -> String {
    match kept_from || page.title.is_empty() {
        true => page.site.to_string(),
        false => page.title.to_string(),
    }
}

/// A kept log drawn back: its lines as a done thread draws them, the chip's title over what was
/// asked, and each answer's line with the time it finished.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Drawn {
    pub lines: Vec<Line>,
    pub chip: Option<SharedString>,
    pub finished: Vec<(usize, SharedString)>,
}

/// The keys of a kept log's ilo lines, from here up: none of them takes a press, and none is
/// the key of a line the app makes now.
const KEPT_LINES: usize = usize::MAX / 2;

/// `records` drawn back. A record's kind alone decides its line's form, and every line is settled:
/// no stretch under way, no step running, nothing that opens, no button. A page's line says the
/// title it stored, as one with no tab says its site, and is never an open tab's.
pub(crate) fn drawn(records: &[Record]) -> Drawn {
    let mut drawn = Drawn::default();
    let mut trace: Option<Trace> = None;
    let mut page: Option<SharedString> = None;
    let mut key = 0;
    let close = |trace: &mut Option<Trace>, lines: &mut Vec<Line>| {
        lines.extend(trace.take().map(Line::Trace));
    };
    for record in records {
        let step =
            |key: usize, glyph, words: &str, came: &str, failed, on: Option<OnPage>| TraceStep {
                key,
                glyph,
                title: words.to_string().into(),
                description: came.to_string().into(),
                running: false,
                failed,
                opens: Opens::Nothing,
                under: SharedString::default(),
                page: on,
                says: SharedString::default(),
            };
        match record {
            Record::You(text) => {
                close(&mut trace, &mut drawn.lines);
                drawn.lines.push(Line::You(text.clone().into()));
            }
            Record::Chip(title) => {
                drawn.chip.get_or_insert_with(|| title.clone().into());
            }
            Record::Said(text) => {
                close(&mut trace, &mut drawn.lines);
                drawn.lines.push(Line::Said(text.clone().into()));
            }
            Record::Answer { text, at } => {
                close(&mut trace, &mut drawn.lines);
                drawn.finished.push((drawn.lines.len(), at.clone().into()));
                drawn.lines.push(Line::Said(text.clone().into()));
            }
            Record::Ilo(words) => {
                close(&mut trace, &mut drawn.lines);
                let at = drawn.lines.len();
                drawn.lines.push(Line::Ilo(IloLine {
                    key: KEPT_LINES + at,
                    kind: IloKind::Kept {
                        words: words.clone().into(),
                    },
                    settled: true,
                }));
            }
            Record::Fold { took } => {
                close(&mut trace, &mut drawn.lines);
                page = None;
                trace = Some(Trace {
                    key,
                    running: false,
                    took: *took,
                    started: None,
                    items: Vec::new(),
                });
            }
            // Going to the page starts its line, as it does in the log; that step has no line of
            // its own.
            Record::Page(title) => {
                if let Some(trace) = trace.as_mut() {
                    let title: SharedString = title.clone().into();
                    let on = OnPage {
                        tab: None,
                        site: title.clone(),
                        title: SharedString::default(),
                        went: true,
                    };
                    let went = step(key, log_glyph(), "", "", false, Some(on));
                    trace.items.push(TraceItem::Step(went));
                    key += 1;
                    page = Some(title);
                }
            }
            Record::Step {
                glyph,
                words,
                came,
                failed,
            } => {
                let on = page.clone().map(|site| OnPage {
                    tab: None,
                    site,
                    title: SharedString::default(),
                    went: false,
                });
                let line = step(key, *glyph, words, came, *failed, on);
                key += 1;
                match trace.as_mut() {
                    Some(trace) => trace.items.push(TraceItem::Step(line)),
                    None => drawn.lines.push(Line::Step(line)),
                }
            }
            Record::Aside(text) => match trace.as_mut() {
                Some(trace) => {
                    page = None;
                    trace.items.push(TraceItem::Said(text.clone().into()))
                }
                None => drawn.lines.push(Line::Said(text.clone().into())),
            },
        }
    }
    close(&mut trace, &mut drawn.lines);
    drawn
}

/// ilo's line as the log stores it (`words::stored`). Of a stop on a tab the user handed the
/// thread, the step it stopped is said by its kind and the page's site alone, as a step there is,
/// never by what the page calls what it was stopped at; one whose step wasn't traced, by no words
/// of the step.
fn ilo_stored(task: &Task, line: &IloLine) -> String {
    let users = match &line.kind {
        IloKind::Stopped {
            tab,
            why,
            tier,
            count,
            ..
        } if task.sends.was_users(*tab) => {
            let stop = (line.key.checked_sub(STOPS)).and_then(|at| task.stops.get(at));
            let (site, kind) = stop.map_or_else(Default::default, |stop| {
                (stop.site.clone(), stop.kind.clone())
            });
            Some(IloKind::Stopped {
                tab: *tab,
                at: site.into(),
                doing: kind.into(),
                why: *why,
                tier: *tier,
                count: *count,
            })
        }
        _ => None,
    };
    words::stored(users.as_ref().unwrap_or(&line.kind), &task.name)
}

/// The glyph of the step that went to a page, which no line draws.
fn log_glyph() -> &'static str {
    crate::agents::kept::log::GLYPHS[0]
}

#[cfg(test)]
mod tests;

/// Whether ilo's line of `kind` goes in the log, `words::stored` taking out any file's or
/// folder's name. Every kind is named here, never a wildcard, so a kind added later is decided
/// before anything of it is kept: one whose words name a folder that can't be taken out, as the
/// links a task's end found in one do (X10, X11), is never stored.
fn logged(kind: &IloKind) -> bool {
    match kind {
        IloKind::Stopped { .. }
        | IloKind::Yours { .. }
        | IloKind::TabBack { .. }
        | IloKind::SignedIn { .. }
        | IloKind::TierChanged { .. }
        | IloKind::YouStopped { .. }
        | IloKind::Helpers { .. }
        | IloKind::NotOurTools
        | IloKind::StartedWithout { .. }
        | IloKind::EndedLeft { .. }
        | IloKind::Revoked { .. }
        | IloKind::FolderGone { .. }
        | IloKind::NotCopied { .. }
        | IloKind::Resumed { .. }
        | IloKind::KeptHere { .. }
        | IloKind::Gone { .. }
        | IloKind::NotKept { .. }
        | IloKind::KeepsNothing { .. }
        | IloKind::NoProfile { .. }
        | IloKind::NoSpace { .. }
        | IloKind::Kept { .. } => true,
    }
}
