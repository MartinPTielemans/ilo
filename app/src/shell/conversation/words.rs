//! Every word a task's conversation says as ilo: its lines, their buttons, the live line and the
//! composer's row. One table, so a change of wording after the owner has used a build is an edit
//! here (#4086). The tiers' names are the tier table's (agents/tier.rs).

use crate::agents::kept;
use crate::agents::task::Live;
use crate::agents::task::ilo::{
    HelpersLimit as Limit, IloButton, IloKind, LeftOut, Reach, SignedInWith,
};
use crate::agents::tier::{self, Tier, Why};
use crate::product::PRODUCT;

/// The product's name, before each of its lines' words.
pub const MARK: &str = PRODUCT;
/// The live line's button, and a Stopped line's first.
pub const TAKE_OVER: &str = "Take Over";
pub const STOP: &str = "Stop";

/// The live line while no step is under way: between steps, and after Hand Back or a change of
/// access, until the next step starts.
pub fn working(agent: &str) -> String {
    format!("{agent} is working")
}

/// The assistant's own words as a screen reader is told them, the first line of its answer: the
/// mark before them is the app's, and spoken only, so nothing an assistant writes is heard as a
/// line of the product's.
pub fn says(assistant: &str, said: &str) -> String {
    format!("{assistant} says {said}")
}

/// A task the user stopped, as the last line of its log says it.
const YOU_STOPPED: &str = "You stopped it";

/// The button under an answer that copies it, as a screen reader names it (Z18).
pub const COPY_ANSWER: &str = "Copy Answer";

/// The line over a stretch's helpers (Z16), for two or more: the table has no line for one.
pub fn split(strands: usize) -> Option<String> {
    (strands > 1).then(|| format!("Split into {strands} strands"))
}

/// The fold over a stretch of work (Z14), the one place the weaving pair stands: at work, its word
/// and the time so far; done, the time it took.
pub fn fold(took: Option<u64>, so_far: u64) -> (String, String) {
    use crate::agents::task::duration;
    match took {
        Some(took) => (format!("Woven in {}", duration(took)), String::new()),
        None => ("Weaving".to_string(), duration(so_far)),
    }
}

/// The live line's button while the user has the task's page.
pub const HAND_BACK: &str = "Hand Back";

/// The live line's words in a state other than working, one entry each, or None for no line:
/// waiting at a refused or a handed step, ilo's line with its own buttons is the last over the
/// field (the lead, 20:22Z).
pub fn live(live: &Live) -> Option<String> {
    match live {
        Live::Working => None,
        Live::Refused | Live::Handed => None,
        Live::Yours { site: None, .. } => Some("You're driving".into()),
        Live::Yours {
            site: Some(site), ..
        } => Some(format!("You're driving {site}")),
        // The question's own line says it, with its answers.
        Live::Asked => None,
    }
}

/// A line's words after the mark: ilo's own, then the name it takes from a page or a site, if it
/// has one, which is drawn apart from them.
pub fn head(kind: &IloKind, agent: &str) -> (String, Option<String>) {
    match kind {
        IloKind::Stopped {
            why: Why::Fenced | Why::OtherSite,
            at,
            ..
        } => (
            format!("stopped {agent} from going to"),
            Some(at.to_string()),
        ),
        // The refused step as it was under way, its control's name keeping its own quotes (B1).
        IloKind::Stopped { doing, .. } if !doing.is_empty() => {
            (format!("stopped {agent} from {doing}"), None)
        }
        // With no step traced to name, never a tool's own name or a raw argument (B1, 15:20Z).
        IloKind::Stopped { .. } => (format!("stopped one of {agent}'s steps"), None),
        // With more than one page, it names the step's (O1).
        IloKind::Yours {
            site: Some(site), ..
        } => ("handed this step on".into(), Some(site.to_string())),
        IloKind::Yours { .. } => ("handed this step to you".into(), None),
        IloKind::TabBack { .. } => (format!("· {agent} needs this tab back"), None),
        IloKind::SignedIn { site, .. } => ("signed in to".into(), Some(site.to_string())),
        IloKind::TierChanged { to, .. } => {
            (format!("· You set this thread to {}", to.label()), None)
        }
        // The pages it held then, by their count alone (Z17).
        IloKind::YouStopped { pages } => (
            match crate::agents::tools::pages_counted(*pages) {
                Some(pages) => format!("· {YOU_STOPPED} · {pages}"),
                None => format!("· {YOU_STOPPED}"),
            },
            None,
        ),
        // The helpers' limit (#4102), by the app's own count, each a whole sentence.
        IloKind::Helpers { limit, .. } => (
            match (*limit, crate::tokens::agents::helpers::MOST) {
                (Limit::Depth, _) => {
                    format!("stopped {agent} when a strand started a strand of its own")
                }
                (Limit::Kind, _) => {
                    format!("stopped {agent} after it started a strand that isn't allowed")
                }
                (Limit::Count, 1) => format!("stopped {agent} after it started more than 1 strand"),
                (Limit::Count, most) => {
                    format!("stopped {agent} after it started more than {most} strands")
                }
            },
            None,
        ),
        // A wall ilo couldn't put up (#4101). Each names what the user gave, and `tail` ends it.
        IloKind::StartedWithout {
            left: LeftOut::Folder(name) | LeftOut::File(name),
        } => ("started this thread without".into(), Some(name.to_string())),
        IloKind::StartedWithout {
            left: LeftOut::Files(files),
        } => (
            format!(
                "started this thread without your {files} files: {PRODUCT} couldn't limit {agent} to them"
            ),
            None,
        ),
        IloKind::KeptAt { at, folder, .. } => (
            format!(
                "kept this thread at {}: {PRODUCT} couldn't limit {agent} to",
                at.label()
            ),
            Some(folder.to_string()),
        ),
        IloKind::SetBack { at, folder, .. } => (
            format!(
                "set this thread back to {}: {PRODUCT} couldn't limit {agent} to",
                at.label()
            ),
            Some(folder.to_string()),
        ),
        IloKind::NotRestarted {
            left: LeftOut::Folder(name) | LeftOut::File(name),
        } => (
            format!("stopped this thread: {PRODUCT} couldn't restart {agent} without"),
            Some(name.to_string()),
        ),
        IloKind::NotRestarted {
            left: LeftOut::Files(files),
        } => (
            format!(
                "stopped this thread: {PRODUCT} couldn't restart {agent} without your {files} files"
            ),
            None,
        ),
        IloKind::NotRaised { refused } => (
            format!(
                "stopped this thread: {PRODUCT} couldn't restart {agent} at {}",
                refused.label()
            ),
            None,
        ),
        // What a run left running (#4101): what it can still reach, and what ends it.
        IloKind::LeftRunning {
            reach: Reach::Folder(folder) | Reach::Both(folder),
        } => (
            format!("couldn't end everything {agent} started: what's left can still reach"),
            Some(folder.to_string()),
        ),
        IloKind::LeftRunning {
            reach: Reach::Files,
        } => (
            format!(
                "couldn't end everything {agent} started: what's left can still reach this \
                 thread's files{RESTART_ENDS}"
            ),
            None,
        ),
        IloKind::EndedLeft => (format!("ended what {agent} had left running"), None),
        // The user is the subject, as of a change of level.
        IloKind::Revoked { folder } => ("\u{b7} You revoked".into(), Some(folder.to_string())),
        IloKind::FolderGone { folder } => ("revoked".into(), Some(folder.to_string())),
        // X6c: what the copy that failed would have given, by its count.
        IloKind::NotCopied {
            left: LeftOut::File(name) | LeftOut::Folder(name),
        } => ("started this thread without".into(), Some(name.to_string())),
        IloKind::NotCopied {
            left: LeftOut::Files(files),
        } => (
            format!("started this thread without {files} files: {PRODUCT} couldn't copy them"),
            None,
        ),
        IloKind::Resumed { tier } => (format!("resumed this thread at {}", tier.label()), None),
        // Where the assistant is the subject, a middle dot sets ilo's mark off from the sentence.
        // A kept task takes a reply after a quit too, so the words name no end; the reply field
        // says reply, and Settings is where one is removed, so the line says neither.
        IloKind::KeptHere => (format!("· {agent} still has this conversation."), None),
        IloKind::Gone => (format!("· {}", kept::gone_words(agent)), None),
        IloKind::NotKept => (format!("· {agent} didn't keep this conversation."), None),
        IloKind::KeepsNothing => (format!("· {agent} doesn't keep finished threads."), None),
        IloKind::NoProfile => ("· This thread's profile was deleted.".into(), None),
        IloKind::NoSpace => ("· This thread's profile has no Space left.".into(), None),
    }
}

/// A line's head as drawn: ilo's words `own`, with `name`, and where `name` stands in it. A
/// page's own words are quoted, so they read as its; a site's name never is, as it's the app's to
/// write.
pub fn with_name(kind: &IloKind, own: &str, name: &str) -> (String, std::ops::Range<usize>) {
    let site = matches!(
        kind,
        IloKind::Stopped {
            why: Why::Fenced | Why::OtherSite,
            ..
        } | IloKind::SignedIn { .. }
            | IloKind::Yours { .. }
    );
    let start = own.len() + 1;
    match (site, kind) {
        (true, IloKind::Yours { .. }) => {
            (format!("{own} {name} to you"), start..start + name.len())
        }
        (true, _) => (format!("{own} {name}"), start..start + name.len()),
        (false, _) => {
            let said = format!("{own} \u{201c}{name}\u{201d}");
            let end = said.len();
            (said, start..end)
        }
    }
}

/// How the line of what a run left running ends: what the user can do about it.
const RESTART_ENDS: &str = ". Restarting this computer ends it.";

/// What a line says after its name, to its sentence's end, if anything.
pub fn tail(kind: &IloKind, agent: &str) -> Option<String> {
    match kind {
        IloKind::StartedWithout {
            left: LeftOut::Folder(_),
        } => Some(format!(": {PRODUCT} couldn't limit {agent} to that folder")),
        IloKind::StartedWithout {
            left: LeftOut::File(_),
        } => Some(format!(": {PRODUCT} couldn't limit {agent} to that file")),
        IloKind::KeptAt { refused, .. } | IloKind::SetBack { refused, .. } => {
            Some(format!(" at {}", refused.label()))
        }
        IloKind::LeftRunning {
            reach: Reach::Folder(_),
        } => Some(RESTART_ENDS.into()),
        IloKind::LeftRunning {
            reach: Reach::Both(_),
        } => Some(format!(" and this thread's files{RESTART_ENDS}")),
        IloKind::FolderGone { .. } => Some(format!(": {PRODUCT} couldn't find it")),
        IloKind::NotCopied {
            left: LeftOut::File(_) | LeftOut::Folder(_),
        } => Some(format!(": {PRODUCT} couldn't copy it")),
        _ => None,
    }
}

/// Where Settings holds what a line's link points at.
const KEPT_IN: &str = "Settings › Agents";

/// The app's own quiet link under a line, which opens Settings where its words point and changes
/// nothing by itself: under a fence's refusal, the Off limits list.
pub fn link(kind: &IloKind) -> Option<&'static str> {
    match kind {
        IloKind::Stopped {
            why: Why::Fenced, ..
        } => Some(KEPT_IN),
        _ => None,
    }
}

/// What a line says under its head, if anything: why ilo stopped, in the user's terms, or what it
/// signed in with. `at` is the line's name, already cut to its length.
pub fn body(kind: &IloKind, at: &str) -> Option<String> {
    match kind {
        IloKind::Stopped { why, tier, .. } => Some(tier::why_line(*why, *tier, at)),
        IloKind::SignedIn { with, .. } => Some(match with {
            SignedInWith::Saved(account) => format!("With your saved login, {account}."),
            SignedInWith::Extension(name) => format!("With {name}."),
        }),
        IloKind::Yours { .. } | IloKind::TabBack { .. } | IloKind::TierChanged { .. } => None,
        IloKind::YouStopped { .. } => None,
        IloKind::Helpers { .. } => None,
        IloKind::StartedWithout { .. }
        | IloKind::KeptAt { .. }
        | IloKind::SetBack { .. }
        | IloKind::NotRestarted { .. }
        | IloKind::NotRaised { .. }
        | IloKind::LeftRunning { .. }
        | IloKind::EndedLeft
        | IloKind::Revoked { .. }
        | IloKind::FolderGone { .. }
        | IloKind::NotCopied { .. } => None,
        IloKind::Resumed { .. } => None,
        IloKind::KeptHere
        | IloKind::Gone
        | IloKind::NotKept
        | IloKind::KeepsNothing
        | IloKind::NoProfile
        | IloKind::NoSpace => None,
    }
}

/// A repeated refusal's count, at its line's end.
pub fn count(count: u32) -> Option<String> {
    (count > 1).then(|| format!("· {count} times"))
}

/// A button's words.
pub fn button(button: IloButton) -> &'static str {
    match button {
        IloButton::TakeOver => TAKE_OVER,
        IloButton::Raise(Tier::Look) => "Look only",
        IloButton::Raise(Tier::Work) => "Give Work on pages",
        IloButton::Raise(Tier::Full) => "Give Full access",
        IloButton::AnySite => "Any site",
        IloButton::HandBack | IloButton::PagesBack => "Hand Back",
        IloButton::ShowPage => "Show Page",
        IloButton::NotNow => "Not Now",
        IloButton::StartAgain => "Start Again",
    }
}

/// What a line's buttons promise, under them, if anything.
/// Under Full access, how long it lasts: the behaviours menu's foot, word for word (C3).
pub fn note(buttons: &[IloButton]) -> Option<&'static str> {
    (buttons.contains(&IloButton::Raise(Tier::Full))).then_some("For this thread only")
}

/// The quiet line in the reply field's place, if any: on an outside app's session, which `name`
/// is, where to reply (C2); while Settings › Threads' switch is off, that it is (C1).
pub fn in_reply_place(replies: bool, threads_off: bool, name: &str) -> Option<String> {
    match (replies, threads_off) {
        (false, _) => Some(format!("Reply in {name}.")),
        (true, true) => Some("Threads are off in Settings › Threads.".to_string()),
        (true, false) => None,
    }
}

/// The reply field's placeholder while its thread is at work (Z-who): typing there steers it.
pub const STEER: &str = "Steer it, or ask";

/// The reply field's placeholder otherwise (Z-who), and the name of the button that sends what's
/// typed, whoever is at work: the user reads ilo as the one at work, never the assistant.
pub const REPLY: &str = "Reply";

/// The reply field's placeholder, as its thread is `at_work` or not.
pub fn reply_field(at_work: bool) -> &'static str {
    match at_work {
        true => STEER,
        false => REPLY,
    }
}
