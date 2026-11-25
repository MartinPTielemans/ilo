//! A reply to a task that has ended (#4105): the task model's side of kept.rs. A task's run is
//! kept when kept.rs says so, the user's reply after it ended takes it up at the access the app
//! decides, and what was kept goes with the task or with Settings › Agents' Remove and Remove
//! All. A task that has ended is there after a quit too: the next launch lists it in the Archive
//! by its title, and a reply takes it up in its own profile.
//!
//! Only the user's own reply in the task's conversation takes a task up: no tool of an agent's
//! and no outside client reaches any of this, to take one up, list them or remove one.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use gpui::{App, AppContext, Context, RenderImage, SharedString};

use super::ilo::{IloKind, IloLine};
use super::{Card, KEPT, Line, State, Task, Who, finding, last_tab, seconds};
use crate::agents::Agents;
use crate::agents::helpers::Helpers;
use crate::agents::kept::cards::{self, Picture};
use crate::agents::kept::{self, Keep, Keeps, Nowhere};
use crate::agents::runner::Cli;
use crate::agents::tier::{Sites, Tier};
use crate::tokens::shell::composer::CARDS;

/// The title of a task a run before left whose agent never named it: what was asked stood as
/// its title, and isn't kept.
const UNTITLED: &str = "Untitled task";

/// The keys of the lines made here, from the top of the range, apart from the lines that count
/// up from a task's events.
const IN_FIELD: usize = usize::MAX;
const KEPT_HERE: usize = usize::MAX - 1;
const RESUMED: usize = usize::MAX - 2;

/// The last picture of a kept task's last page its conversation took, as its card keeps it.
pub enum CardTile {
    /// Of `tab`: the picture, or none if it couldn't have been given to an agent.
    Taken(u64, Option<Arc<RenderImage>>),
    /// The card's folder holds what there is of `tab`'s, written at the Done.
    Written(u64),
}

impl Agents {
    /// Where the kept tasks' folders are, under the profile's. None with no profile folder, when
    /// nothing is kept.
    pub(super) fn kept_root(&self) -> Option<PathBuf> {
        (self.tasks.dir.as_ref()).map(|dir| dir.join(kept::FOLDER))
    }

    /// Where the kept tasks' cards are, beside their folders (kept/cards.rs).
    pub(super) fn cards_root(&self) -> Option<PathBuf> {
        (self.tasks.dir.as_ref()).map(|dir| dir.join(cards::FOLDER))
    }

    /// The conversation of `agent`'s task took a picture of `tab`: `picture`, or none if it
    /// couldn't have been given to an agent, as the window that took it says. Of the task's last
    /// page, a kept task's card keeps it for its Done, in place of any before; of another, nothing.
    pub fn card_pictured(
        &mut self,
        agent: &str,
        tab: u64,
        picture: Option<Arc<RenderImage>>,
        cx: &App,
    ) {
        let Some(task) = self.task(agent) else {
            return;
        };
        let kept = self.tasks.kept.of(agent).is_some() && !self.tasks.kept.off;
        if !kept || last_tab(task, &self.pages(task, cx)) != Some(tab) {
            return;
        }
        let tile = CardTile::Taken(tab, picture);
        self.tasks.card_tiles.insert(agent.to_string(), tile);
    }

    /// `agent`'s kept task is Done: its card's line and its last page's picture go to its card's
    /// folder, written off the UI's thread, the picture only if the last one its conversation took
    /// of that page could have been given to an agent, and the page still could be now. Nothing
    /// with the switch off, nor for a task with no row, as one deleted from the Archive: its card
    /// stays as it was while ⌘Z can bring it back, and goes when it goes for good. A failed write
    /// leaves the card as it was and says nothing.
    fn keep_card(&mut self, agent: &str, cx: &mut Context<Self>) {
        let (Some(root), Some(task)) = (self.cards_root(), self.task(agent)) else {
            return;
        };
        if self.tasks.kept.off || !self.tasks.kept.listed().any(|listed| listed == agent) {
            return;
        }
        let Some(card) = (self.tasks.kept.of(agent)).map(|record| record.card.clone()) else {
            return;
        };
        if !matches!(task.state, State::Done { .. }) {
            return;
        }
        let line = Some(finding(&task.messages).to_string()).filter(|line| !line.is_empty());
        let last = last_tab(task, &self.pages(task, cx));
        // A page that couldn't be kept now, as one the user held since its picture was taken,
        // keeps none, whatever its tile says.
        let last = last.filter(|tab| self.keeps_picture(*tab, cx));
        let (picture, written) = match self.tasks.card_tiles.remove(agent) {
            Some(CardTile::Taken(tab, Some(image))) if Some(tab) == last => {
                (Some(image), Some(tab))
            }
            Some(CardTile::Written(tab)) if Some(tab) == last => (None, Some(tab)),
            _ => (None, None),
        };
        let leave = picture.is_none() && written.is_some();
        if let Some(tab) = written {
            (self.tasks.card_tiles).insert(agent.to_string(), CardTile::Written(tab));
        }
        let picture = match (picture, leave) {
            (Some(image), _) => Picture::Taken(image),
            (None, true) => Picture::Leave,
            (None, false) => Picture::Remove,
        };
        let job = cards::Job::Write {
            root,
            id: card,
            line,
            picture,
        };
        self.tasks.card_jobs.ask(job, cx.background_executor());
    }

    /// Settings › Agents' Keep finished tasks so you can reply.
    pub fn keeps_tasks(&self) -> bool {
        !self.tasks.kept.off
    }

    /// The switch. Off, a new task's run keeps nothing; what's kept stays until it's removed,
    /// which Settings offers as it's turned off.
    pub fn set_keeps_tasks(&mut self, on: bool, cx: &mut Context<Self>) {
        if on != self.tasks.kept.off {
            return;
        }
        self.tasks.kept.off = !on;
        self.save();
        cx.notify();
    }

    /// How many finished tasks ilo holds a conversation's id for, and how many of them the last
    /// try couldn't remove.
    pub fn kept_tasks(&self) -> (usize, usize) {
        (self.tasks.kept.count(), self.tasks.kept.stuck())
    }

    /// What `agent`'s run that's about to start keeps, and the folder it runs in if it's kept. A
    /// task with a conversation kept goes on with it; a new one keeps what kept.rs decides.
    pub(super) fn keep_for(&self, agent: &str, cli: Cli) -> (Keep, Option<PathBuf>) {
        let Some(root) = self.kept_root() else {
            return (Keep::Nothing, None);
        };
        // A task with files runs where its assistant can keep no conversation, and reach none
        // it kept (wall.rs).
        let walled = self.walled(agent);
        let keep = match self.tasks.kept.of(agent) {
            Some(record) if record.gone || walled => Keep::Nothing,
            Some(record) => Keep::Resume(record.id.clone()),
            None => kept::decide(cli, self.keeps_tasks(), walled),
        };
        let folder = match &keep {
            Keep::Nothing => None,
            Keep::New(id) | Keep::Resume(id) => Some(kept::folder(&root, id)),
        };
        (keep, folder)
    }

    /// `agent`'s run started, keeping `keep`: a new conversation's id is held from now.
    pub(super) fn kept_started(&mut self, agent: &str, cli: Cli, keep: &Keep) {
        let (Keep::New(id), Some(root)) = (keep, self.kept_root()) else {
            return;
        };
        let (home, started) =
            (self.task(agent)).map_or((0, SystemTime::now()), |task| (task.home, task.started));
        let profile = self.browser.space_profile(home);
        // The one write a start makes says all a launch needs if the browser is killed before
        // the task ends: that it was under way, since when and in which profile (#4105).
        (self.tasks.kept).hold(agent, cli, id.clone(), &profile, seconds(started));
        self.tasks.kept.write(&root);
    }

    /// `agent`'s task has the name its agent gave it: the file's row for it has it from now, so
    /// a launch after the browser was killed lists it by its name. Once for each name, which an
    /// agent gives with its first call and again only as it moves on to another task.
    pub(super) fn kept_named(&mut self, agent: &str) {
        let (Some(root), Some(task)) = (self.kept_root(), self.task(agent)) else {
            return;
        };
        let title = title_of(task);
        let past = (self.tasks.kept.of_mut(agent)).and_then(|record| record.past.as_mut());
        let Some(past) = past.filter(|past| past.title != title) else {
            return;
        };
        past.title = title;
        self.tasks.kept.write(&root);
    }

    /// `agent`'s task, which had ended, is under way again, by a reply or Retry: the file's row
    /// for it says so from now, so if the browser is killed it comes back as one that broke off
    /// and not as it ended before.
    pub(super) fn kept_under_way(&mut self, agent: &str) {
        let (Some(root), Some(task)) = (self.kept_root(), self.task(agent)) else {
            return;
        };
        let under_way = kept::Past::under_way(title_of(task), seconds(SystemTime::now()));
        let record = (self.tasks.kept.of_mut(agent)).filter(|record| !record.gone);
        let Some(past) = record.and_then(|record| record.past.as_mut()) else {
            return;
        };
        *past = under_way;
        self.tasks.kept.write(&root);
    }

    /// Whether `agent`'s task has files, and so runs inside a wall.
    fn walled(&self, agent: &str) -> bool {
        self.task(agent).is_some_and(|task| task.files.any())
    }

    /// Whether `agent`'s next run goes on with a conversation its assistant kept, and so takes
    /// only what's new as its first message. A task with files never does: it starts from its
    /// own words, whatever was kept of it.
    pub(super) fn resumes(&self, agent: &str) -> bool {
        !self.walled(agent) && (self.tasks.kept.of(agent)).is_some_and(|record| !record.gone)
    }

    /// The user replies to `agent`'s task: if it had ended and its conversation is kept, the reply
    /// takes it up, at the access the app decides from the one it ended at. ilo's line over the
    /// reply says so only where there's something to say: the task comes back at another access
    /// than it ended at, or it's one a run before left, whose assistant ilo starts again. A reply
    /// to a task done or stopped in this run, at the access it had, is the resume and has no line.
    /// Nothing the assistant or a page says has a part in it, and its assistant is told nothing
    /// of the access: ilo's own check holds it. A run inside a wall the lower access changes is
    /// over, so the reply starts one inside the new level's (rewall.rs).
    pub(super) fn pick_up(&mut self, agent: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task(agent) else {
            return;
        };
        if !ended(task) || self.nowhere(agent) {
            return;
        }
        let was = task.state.clone();
        let said = replies(task);
        // A level the user picked since it ended is the one the reply starts at, as the chip
        // says; else the app decides from the one it ended at. Its wall was that one's.
        let ended_at = task.picked_from.unwrap_or(task.tier);
        let back = match task.picked_from {
            Some(_) => task.tier,
            None => kept::comes_back(task.tier),
        };
        let shown = task.tier;
        // One a run before left has no message of the user's until its first reply here.
        let after_a_quit = task.from_before() && said == 0;
        let Some(record) = self.tasks.kept.of_mut(agent).filter(|record| !record.gone) else {
            return;
        };
        if back != shown || after_a_quit {
            record.resumed.push((said, back));
        }
        self.tasks.picked_up.insert(agent.to_string(), was);
        self.kept_under_way(agent);
        if let Some(task) = self.task_mut(agent) {
            task.tier = back;
            task.picked_from = None;
        }
        log!("browser: task {agent} picked up");
        // The activity log's entry for it, under the task's own name.
        self.note(agent, 0, "resumed", "", "");
        let same = (self.task(agent)).is_none_or(|task| task.files.same_rules(ended_at, back));
        if !same {
            self.wall_outgrown(agent, cx);
        }
    }

    /// `agent`'s run went, saying `said`. If it was to go on with a conversation its assistant no
    /// longer has, the task is as it was before the reply, which never reached it, or failed if the
    /// reply came while it was under way, and its conversation says so in the reply field's place.
    /// Returns whether that's what happened, and what it said with the conversation's id out of
    /// it, for the log.
    pub(super) fn kept_gone(&mut self, agent: &str, said: &str) -> (bool, String) {
        // The reply that never reached it is its last message, and ilo's line over it goes too.
        let reply = self.task(agent).and_then(|task| {
            let last = task.messages.last()?;
            (last.who == Who::User).then(|| replies(task) - 1)
        });
        let Some(record) = self.tasks.kept.of_mut(agent) else {
            return (false, said.to_string());
        };
        let said = kept::without(said, &record.id);
        if record.gone || !kept::gone(&said) {
            return (false, said);
        }
        record.gone = true;
        if reply.is_some() && record.resumed.last().map(|(at, _)| *at) == reply {
            record.resumed.pop();
        }
        let was = self.tasks.picked_up.remove(agent);
        if let Some(task) = self.task_mut(agent) {
            if task
                .messages
                .last()
                .is_some_and(|last| last.who == Who::User)
            {
                task.messages.pop();
            }
            // A task picked up goes back to how it ended. One the reply reached while under way
            // ends here, though nobody stopped it.
            let why = kept::gone_words(&task.name);
            task.state = was.unwrap_or(State::Failed {
                why: why.into(),
                assistant: false,
            });
            task.waiting = None;
            task.writing = None;
        }
        (true, said)
    }

    /// ilo's lines in a kept task's log: where the messages of one a run before left would begin,
    /// that its assistant still has them, and over a reply that took a task up at another access
    /// or after a quit (`pick_up`), the access it came back at. Then, last, the line that stands in
    /// the reply field's place once a task is done, stopped, or failed as its conversation went,
    /// and can't take a reply: its assistant keeps none, didn't keep this one as the switch was
    /// off, or no longer has it. One that ended in this run with its conversation kept has no
    /// line: its messages are there, over its reply field. An outside agent's session has none.
    pub(super) fn kept_lines(&self, task: &Task, lines: &mut Vec<Line>) {
        let Some(cli) = task.cli.filter(|_| task.session.is_none()) else {
            return;
        };
        let line = |key, kind| {
            Line::Ilo(IloLine {
                key,
                kind,
                settled: false,
            })
        };
        let record = self.tasks.kept.of(&task.agent);
        // A task with nowhere to work says why in the field's place, however it ended.
        if let Some(why) = record.and_then(|record| record.nowhere) {
            let kind = match why {
                Nowhere::Deleted => IloKind::NoProfile,
                Nowhere::NoSpace => IloKind::NoSpace,
            };
            return lines.push(line(IN_FIELD, kind));
        }
        // All a task a run before left has to show is its title, so where its messages would
        // begin it says where they are, however it ended.
        let kept = record.is_some_and(|record| !record.gone) && self.kept_root().is_some();
        if task.from_before() && lines.is_empty() && kept {
            return lines.push(line(KEPT_HERE, IloKind::KeptHere));
        }
        let resumed = record.map_or(&[][..], |record| &record.resumed[..]);
        // The last first, so each one's place still counts the lines before it.
        for (index, (said, tier)) in resumed.iter().enumerate().rev() {
            let at = (lines.iter().enumerate())
                .filter(|(_, line)| matches!(line, Line::You(_)))
                .nth(*said)
                .map(|(at, _)| at);
            if let Some(at) = at {
                let kind = IloKind::Resumed { tier: *tier };
                lines.insert(at, line(RESUMED - index, kind));
            }
        }
        // One that couldn't go on keeps its field: at its assistant's limit it waits for Retry,
        // and failed, for what fixes it, unless what failed is that its conversation is gone,
        // which no reply mends. Nor does one a run before left whose conversation has gone:
        // nothing of it is here to go on with, however it ended.
        // And with no profile folder nothing is kept or looked for, so a task takes a reply as it
        // did before any was.
        let gone = record.is_some_and(|record| record.gone);
        let lost = task.from_before() && !record.is_some_and(|record| !record.gone);
        let ended = match task.state {
            State::Done { .. } | State::Stopped => true,
            State::Failed { .. } => gone,
            _ => false,
        };
        if !(lost || ended) || self.kept_root().is_none() {
            return;
        }
        match record {
            Some(record) if !record.gone => {}
            Some(_) => lines.push(line(IN_FIELD, IloKind::Gone)),
            None => lines.push(line(
                IN_FIELD,
                match kept::keeps(cli) {
                    Keeps::Never => IloKind::KeepsNothing,
                    // Remove All took it from under the task.
                    Keeps::WhenWanted if self.tasks.kept.had(&task.agent) => IloKind::Gone,
                    // The task started while Settings' switch was off.
                    Keeps::WhenWanted => IloKind::NotKept,
                },
            )),
        }
    }

    /// `agent`'s task ended, or its place in the Archive changed: what the file says of it
    /// follows at once, so it's there to reply to after a quit, or isn't. Its title is the name
    /// its agent gave it and never what was asked; one deleted from the Archive has no row, and
    /// ⌘Z gives it its row back.
    pub(super) fn keep_ended(&mut self, agent: &str, cx: &mut Context<Self>) {
        let (Some(root), Some(task)) = (self.kept_root(), self.task(agent)) else {
            return;
        };
        let past = past_of(task);
        let (Some(past), Some(record)) = (past, self.tasks.kept.of_mut(agent)) else {
            return;
        };
        if record.past == past {
            return;
        }
        record.past = past;
        self.tasks.kept.write(&root);
        self.keep_card(agent, cx);
    }

    /// At quit: a task still at work ends there, as the quit is the user's, so the file says it
    /// was stopped, and the next launch has it to reply to.
    pub(super) fn keep_at_quit(&mut self) {
        let Some(root) = self.kept_root() else {
            return;
        };
        let now = seconds(SystemTime::now());
        let mut changed = false;
        for task in &self.tasks.tasks {
            if ended(task) || task.deleted {
                continue;
            }
            let Some(record) = self.tasks.kept.of_mut(&task.agent) else {
                continue;
            };
            record.past = Some(kept::Past {
                title: title_of(task),
                end: kept::End::Stopped,
                at: now,
            });
            changed = true;
        }
        if changed {
            self.tasks.kept.write(&root);
        }
    }

    /// What's held changed where no removal is under way, as when a reply found a conversation
    /// gone: the file follows.
    pub(super) fn write_kept(&self) {
        if let Some(root) = self.kept_root() {
            self.tasks.kept.write(&root);
        }
    }

    /// Whether `agent`'s task is one a run before left whose profile has no Space of the user's
    /// any more, or is gone: it has nowhere to work, so it takes no reply and no Retry.
    pub(super) fn nowhere(&self, agent: &str) -> bool {
        (self.tasks.kept.of(agent)).is_some_and(|record| record.nowhere.is_some())
    }

    /// Before a reply or Retry reaches `agent`'s task: one a run before left takes its place
    /// first. False if it can't go on, with its profile gone or its conversation: nothing of what
    /// was asked is here to start it with, so nothing is started.
    pub(super) fn goes_on(&mut self, agent: &str) -> bool {
        self.place_past(agent);
        let from_before = self.task(agent).is_some_and(Task::from_before);
        !self.nowhere(agent) && (!from_before || self.resumes(agent))
    }

    /// A task a run before left takes its place among the user's Spaces as it's brought back or
    /// replied to: the Space of its own profile the user is in, else the first of theirs in it,
    /// whichever Space they stand in. Its own Space is then made like that one, so it works in
    /// the sessions it was asked in and no others. With no such Space, as when its profile was
    /// deleted, it has nowhere to work, and its card is where the user is, to start it again or
    /// remove it from; it looks again each time, as the profile may have a Space by then.
    pub(super) fn place_past(&mut self, agent: &str) {
        let placed = |task: &Task| !task.from_before() || task.home != 0;
        if self.task(agent).is_none_or(placed) && !self.nowhere(agent) {
            return;
        }
        let Some(record) = self.tasks.kept.of_mut(agent) else {
            return;
        };
        let mut home = match record.profile.is_empty() {
            true => 0,
            false => self.browser.profile_space(&record.profile),
        };
        // Its pages go to the user's Space of its profile when it hands one over or ends, so a
        // Space of its own made in a profile with none of theirs would leave them no place but
        // another profile's, where each would open again without the sessions it worked in.
        record.nowhere = match home {
            0 if self.browser.has_profile(&record.profile) => Some(Nowhere::NoSpace),
            0 => Some(Nowhere::Deleted),
            _ => None,
        };
        if home == 0 {
            home = self.browser.user_space(0);
        }
        if let Some(task) = self.task_mut(agent) {
            task.home = home;
        }
    }

    /// The profiles there are, each by its folder's name, as the core says them whenever they or
    /// the Spaces change. One that was there and has gone was deleted: the tasks asked in it go
    /// with it, by the way Remove takes one, so no row waits for a profile that later gets the
    /// same folder. A task with nowhere to work looks again, as its profile may have a Space now.
    pub fn profiles_are(&mut self, profiles: Vec<String>, cx: &mut Context<Self>) {
        // The core always has one, so none is a core with nothing to say yet.
        if profiles.is_empty() {
            return;
        }
        let gone: Vec<String> = (self.tasks.profiles.iter())
            .filter(|profile| !profiles.contains(profile))
            .cloned()
            .collect();
        self.tasks.profiles = profiles;
        let nowhere: Vec<String> = (self.tasks.tasks.iter())
            .filter(|task| self.nowhere(&task.agent))
            .map(|task| task.agent.to_string())
            .collect();
        for agent in &nowhere {
            self.place_past(agent);
        }
        if !nowhere.is_empty() {
            cx.notify();
        }
        if gone.is_empty() {
            return;
        }
        for task in &mut self.tasks.tasks {
            let theirs = (self.tasks.kept.of(&task.agent))
                .is_some_and(|record| gone.contains(&record.profile));
            // Its run ends first, so nothing writes the conversation back.
            if theirs {
                task.process = None;
            }
        }
        log!("browser: kept tasks of {} profiles deleted go", gone.len());
        self.remove_kept(move |record| gone.contains(&record.profile), cx);
    }

    /// Settings › Agents' list of what's kept: each finished task whose conversation takes a
    /// reply, the last to end first, by its task, its title and the Archive's own words for it.
    pub fn kept_list(&self, cx: &App) -> Vec<(SharedString, SharedString, SharedString)> {
        let mut listed: Vec<&Task> = (self.tasks.kept.listed())
            .filter_map(|agent| self.task(agent))
            .filter(|task| ended(task))
            .collect();
        listed.sort_by_key(|task| std::cmp::Reverse(task.ended));
        (listed.into_iter())
            .map(|task| {
                let agent = SharedString::from(task.agent.clone());
                (agent, task.title.clone(), self.about(task, cx).into())
            })
            .collect()
    }

    /// The tasks a run before left that haven't been brought back, the last to end first, as the
    /// composer's Latest tasks lists them until one is opened: each as its card, which has its
    /// title and how it ended and no more, and for its home the user's Space of the profile it was
    /// asked in, by which a window lists those of the profile in front alone. One whose
    /// conversation takes no reply, or whose profile has no Space of the user's, is in the
    /// Archive alone, as is one older than the newest a profile's composer has room for. Made
    /// from what a launch already holds: nothing is read for it, and no tool of an agent's
    /// reaches it.
    pub fn kept_cards(&self, cx: &App) -> Vec<Card> {
        let mut left: Vec<&Task> = (self.tasks.kept.listed())
            .filter_map(|agent| self.task(agent))
            .filter(|task| task.from_before() && task.home == 0)
            .filter(|task| task.dismissed && !task.deleted)
            .collect();
        left.sort_by_key(|task| std::cmp::Reverse(task.ended));
        // Each profile is asked for its Space once, and has no more cards made than the composer
        // shows, however many tasks are kept: this runs at every change of a task.
        let most = CARDS as usize;
        let mut profiles: Vec<(&str, u64, usize)> = Vec::new();
        (left.into_iter())
            .filter_map(|task| {
                let profile = self.tasks.kept.of(&task.agent)?.profile.as_str();
                let known = profiles.iter().position(|(each, ..)| *each == profile);
                let at = known.unwrap_or_else(|| {
                    let home = match profile.is_empty() {
                        true => 0,
                        false => self.browser.profile_space(profile),
                    };
                    profiles.push((profile, home, 0));
                    profiles.len() - 1
                });
                let (_, home, listed) = &mut profiles[at];
                if *home == 0 || *listed == most {
                    return None;
                }
                *listed += 1;
                // Its head says when it ended; no line follows, as no run keeps its answer or
                // its steps.
                Some(Card {
                    home: *home,
                    line: None,
                    ended_at: task
                        .ended
                        .map(|at| crate::agents::time_or_day(seconds(at)).into()),
                    ..self.card(task, cx)
                })
            })
            .collect()
    }

    /// Settings › Agents' Remove on a kept task's row: its conversation goes and its row leaves
    /// the file at once. The task stays where it is, and says its conversation is gone. One still
    /// at work keeps its own, as its assistant is writing it.
    pub fn remove_kept_task(&mut self, agent: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(agent).filter(|task| ended(task)) else {
            return;
        };
        // Its run ends first, so nothing writes the conversation back.
        task.process = None;
        self.remove_copies(agent, cx);
        let task = agent.to_string();
        self.remove_kept(
            move |record| record.task.as_deref() == Some(task.as_str()),
            cx,
        );
    }

    /// `agent`'s task goes for good: what its assistant kept of it goes with it.
    pub(in crate::agents) fn forget_kept(&mut self, agent: &str, cx: &mut Context<Self>) {
        self.tasks.picked_up.remove(agent);
        self.tasks.card_tiles.remove(agent);
        let task = agent.to_string();
        self.remove_kept(
            move |record| record.task.as_deref() == Some(task.as_str()),
            cx,
        );
        self.tasks.kept.forget(agent);
    }

    /// ilo's copies of the files attached to `agent`'s task go with its conversation, off the
    /// UI's thread, as they may be large: nothing can take its reply that would read them.
    fn remove_copies(&mut self, agent: &str, cx: &mut Context<Self>) {
        if let Some(task) = self.task_mut(agent) {
            task.files.attached = None;
        }
        let copies = self.attached_dir(agent);
        cx.background_spawn(async move { _ = std::fs::remove_dir_all(copies) })
            .detach();
    }

    /// Settings › Agents' Remove All: every finished task's conversation goes, and those an
    /// earlier try couldn't remove are tried again. A task still at work keeps its own, as its
    /// assistant is writing it; a finished one's run ends first, so nothing writes it back.
    pub fn remove_kept_tasks(&mut self, cx: &mut Context<Self>) {
        let mut working = Vec::new();
        let mut over = Vec::new();
        for task in &mut self.tasks.tasks {
            match ended(task) {
                true => {
                    task.process = None;
                    over.push(task.agent.clone());
                }
                false => working.push(task.agent.clone()),
            }
        }
        for agent in &over {
            self.remove_copies(agent, cx);
        }
        self.remove_kept(
            move |record| !(record.task.as_ref()).is_some_and(|task| working.contains(task)),
            cx,
        );
        // Every card goes, a task's at work too: it has none to show until it's done, and is
        // written again then.
        if let Some(root) = self.cards_root() {
            (self.tasks.card_jobs).ask(cards::Job::RemoveAll(root), cx.background_executor());
        }
    }

    /// At launch: the tasks an earlier run of the browser left that had ended are tasks again, in
    /// the Archive, each with its title and how it ended and no more, as its conversation is its
    /// assistant's: the last ones the Archive holds, and nothing is made or started for one until
    /// it's brought back. One under way when the browser went without a quit, as when it's
    /// killed, is among them as one that broke off. The other conversations that run left go: an
    /// older task's than those, and one whose task was deleted from the Archive, or was removed
    /// and couldn't be.
    /// From `load`, which the app calls once the first window's UI has shown, so none of this is
    /// in the time to the first frame. With no file of ids, as on every launch after a run that
    /// kept nothing, the one look for the file is all a launch pays: nothing is read, started or
    /// spawned. Then what the launch's cleanup already found of their runs' places is theirs.
    pub(crate) fn take_old_kept(&mut self, cx: &mut Context<Self>) {
        self.take_kept_file(cx);
        self.tasks.kept_taken = true;
        self.came_clean(Vec::new(), cx);
    }

    fn take_kept_file(&mut self, cx: &mut Context<Self>) {
        let (Some(root), Some(cards_root)) = (self.kept_root(), self.cards_root()) else {
            return;
        };
        if !self.tasks.kept.read(&root) {
            // Cards with no file of ids are what a run that went before it could remove them left.
            if cards_root.exists() {
                let sweep = cards::Job::Sweep(cards_root, Vec::new());
                self.tasks.card_jobs.ask(sweep, cx.background_executor());
            }
            return;
        }
        let mut left = self.tasks.kept.left();
        left.truncate(KEPT.saturating_sub(self.tasks.dismissed.len()));
        // The Archive lists the last to leave the foot first, so the oldest joins it first.
        for (id, cli, past) in left.iter().rev() {
            self.tasks.next += 1;
            let agent = format!("task-{}", self.tasks.next);
            self.tasks.kept.give(id, &agent);
            self.tasks.dismissed.push_back(agent.clone());
            let helpers = self.helpers_on() && *cli == Cli::Claude;
            (self.tasks.tasks).push(Task::past(agent, *cli, past, helpers));
        }
        if !left.is_empty() {
            log!("browser: {} tasks of a run before are kept", left.len());
            self.tokens_changed();
            cx.notify();
        }
        self.remove_kept(|record| record.task.is_none(), cx);
        let sweep = cards::Job::Sweep(cards_root, self.tasks.kept.cards());
        self.tasks.card_jobs.ask(sweep, cx.background_executor());
    }

    /// Removes the conversations `which` picks, off the UI's thread, as each waits on its
    /// assistant. One that couldn't be removed stays held and counted, so nothing reads as removed
    /// that wasn't, and the user can try again.
    fn remove_kept(&mut self, which: impl Fn(&kept::Record) -> bool, cx: &mut Context<Self>) {
        let (Some(root), Some(cards_root)) = (self.kept_root(), self.cards_root()) else {
            return;
        };
        let removing: Vec<_> = (self.tasks.kept.removing(which).into_iter())
            .map(|(id, cli)| {
                let found = self.tasks.found.get(&cli).cloned().unwrap_or_default();
                (id, found.program, found.path)
            })
            .collect();
        if removing.is_empty() {
            return;
        }
        // A card goes with its task at once, whether or not its conversation can be removed.
        let gone: Vec<_> = (removing.iter())
            .filter_map(|(id, ..)| self.tasks.kept.card_of(id))
            .collect();
        (self.tasks.card_jobs).ask(
            cards::Job::Remove(cards_root, gone),
            cx.background_executor(),
        );
        // Its row leaves the file at once; its id stays there until its assistant has removed
        // the conversation, so one that couldn't be is still held to try again.
        self.tasks.kept.write(&root);
        cx.notify();
        let folder = root.clone();
        let removed = cx.background_spawn(async move {
            (removing.into_iter())
                .map(|(id, program, path)| {
                    let removed = program.is_some_and(|program| {
                        kept::remove(&program, path.as_deref(), &folder, &id)
                    });
                    (id, removed)
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |agents, cx| {
            let removed = removed.await;
            _ = agents.update(cx, |agents, cx| {
                let stuck = removed.iter().filter(|(_, removed)| !removed).count();
                for (id, removed) in &removed {
                    agents.tasks.kept.removed(id, *removed);
                }
                log!(
                    "browser: kept tasks: {} removed, {stuck} couldn't be",
                    removed.len() - stuck
                );
                agents.tasks.kept.write(&root);
                cx.notify();
            });
        })
        .detach();
    }
}

/// How many messages of the user's `task` has.
fn replies(task: &Task) -> usize {
    (task.messages.iter())
        .filter(|message| message.who == Who::User)
        .count()
}

/// Whether the task has ended: done, stopped, failed or at its assistant's limit.
fn ended(task: &Task) -> bool {
    !matches!(task.state, State::Working | State::Asking { .. })
}

/// The title the file keeps for `task`: the name its agent gave it, in this run or, for one a run
/// before left, as the file had it, else none. What the user asked stands as a task's title until
/// its agent names it, and is never kept.
fn title_of(task: &Task) -> String {
    let named = task.named.is_some() || (task.from_before() && task.title != UNTITLED);
    match named {
        true => task.title.to_string(),
        false => String::new(),
    }
}

/// What the file is to say of `task`: nothing for one deleted from the Archive, and how and when
/// it ended for one that has. None leaves what it says as it is, for a task at work, whose row
/// is one under way's since its start or since a reply took it up.
fn past_of(task: &Task) -> Option<Option<kept::Past>> {
    let end = match task.state {
        _ if task.deleted => return Some(None),
        State::Working | State::Asking { .. } => return None,
        State::Done { .. } => kept::End::Done,
        State::Stopped => kept::End::Stopped,
        State::Failed { .. } => kept::End::Failed,
        State::Limit { .. } => kept::End::Limit,
    };
    Some(Some(kept::Past {
        title: title_of(task),
        end,
        at: seconds(task.ended?),
    }))
}

impl Task {
    /// Whether it's one a run before left: nothing of what was asked is here, where every task
    /// asked in this run has what it was asked.
    pub(super) fn from_before(&self) -> bool {
        self.session.is_none() && self.asked.is_empty()
    }

    /// A task a run before left, from what the file says of it: its title, how and when it
    /// ended, in the Archive, with no messages, as its conversation is its assistant's. It has no
    /// Space of the user's and none of its own until it's brought back (`Agents::place_past`),
    /// and the access every task starts at. `helpers` says whether its runs may start helpers.
    pub(super) fn past(agent: String, cli: Cli, past: &kept::Past, helpers: bool) -> Task {
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(past.at);
        let state = match past.end {
            kept::End::Done => State::Done {
                result: SharedString::default(),
                summary: None,
                tabs: Vec::new(),
            },
            kept::End::Stopped => State::Stopped,
            kept::End::Failed => State::Failed {
                why: SharedString::default(),
                assistant: false,
            },
            kept::End::Limit => State::Limit { resets: None },
        };
        let title = match past.title.is_empty() {
            true => UNTITLED.into(),
            false => past.title.clone().into(),
        };
        Task {
            agent,
            cli: Some(cli),
            name: cli.name().into(),
            session: None,
            home: 0,
            space: 0,
            asked: SharedString::default(),
            page: None,
            title,
            model: SharedString::default(),
            model_id: None,
            added: Vec::new(),
            state,
            ended: Some(at),
            end_entry: 0,
            started: at,
            // Library › Agents has its receipt from the run it ended in.
            kept: Some(at),
            orders: 0,
            sent: 0,
            messages: Vec::new(),
            trace: Vec::new(),
            dismissed: true,
            left: Some(at),
            deleted: false,
            token: crate::agents::random_token(),
            process: None,
            asking: None,
            waiting: None,
            writing: None,
            named: None,
            tier: Tier::default(),
            sites: Sites::default(),
            stops: Vec::new(),
            stopped_turns: Vec::new(),
            picked_from: None,
            placed: Vec::new(),
            told: None,
            helpers: Helpers::of_task(helpers),
            files: crate::agents::files::Given::default(),
            rising: None,
            rewalled: Vec::new(),
            ran_walled: Vec::new(),
            restart: None,
            copying: false,
            names: crate::agents::files::Names::default(),
        }
    }
}
