//! Every word a task's conversation says as ilo: its lines, their buttons, the live line and the
//! composer's row. One table, so a change of wording after the owner has used a build is an edit
//! here (#4086). The tiers' names are the tier table's (agents/tier.rs).

use crate::agents::kept;
use crate::agents::task::ilo::{IloButton, IloKind, SignedInWith};
use crate::agents::tier::{self, Tier, Why};

/// ilo's mark, before each of its lines' words.
pub const MARK: &str = "ilo";
/// The live line's button, and a Stopped line's first.
pub const TAKE_OVER: &str = "Take Over";
pub const STOP: &str = "Stop";

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
        IloKind::Stopped { at, .. } => (format!("stopped {agent} at"), Some(at.to_string())),
        IloKind::Yours { .. } => ("handed this step to you".into(), None),
        IloKind::TabBack { .. } => (format!("· {agent} needs this tab back"), None),
        IloKind::SignedIn { site, .. } => ("signed in to".into(), Some(site.to_string())),
        IloKind::TierChanged { to, .. } => (format!("· You set this task to {}", to.label()), None),
        // The helpers' limit (#4102), by the app's own count, each a whole sentence.
        IloKind::Helpers { deep, .. } => (
            match (*deep, crate::tokens::agents::helpers::MOST) {
                (true, _) => format!("stopped {agent} when a helper started a helper of its own"),
                (false, 1) => format!("stopped {agent} after it started more than 1 helper"),
                (false, most) => {
                    format!("stopped {agent} after it started more than {most} helpers")
                }
            },
            None,
        ),
        IloKind::Resumed { tier } => (format!("resumed this task at {}", tier.label()), None),
        // Where the assistant is the subject, a middle dot sets ilo's mark off from the sentence.
        // A kept task takes a reply after a quit too, so the words name no end; the reply field
        // says reply, and Settings is where one is removed, so the line says neither.
        IloKind::KeptHere => (format!("· {agent} still has this conversation."), None),
        IloKind::Gone => (format!("· {}", kept::gone_words(agent)), None),
        IloKind::NotKept => (format!("· {agent} didn't keep this conversation."), None),
        IloKind::KeepsNothing => (format!("· {agent} doesn't keep finished tasks."), None),
        IloKind::NoProfile => ("· This task's profile was deleted.".into(), None),
        IloKind::NoSpace => ("· This task's profile has no Space left.".into(), None),
    }
}

/// A line's head as drawn: ilo's words `own`, then `name`. A page's own words are quoted, so they
/// read as its; a site's name never is, as it's the app's to write.
pub fn with_name(kind: &IloKind, own: &str, name: &str) -> String {
    let site = matches!(
        kind,
        IloKind::Stopped {
            why: Why::Fenced | Why::OtherSite,
            ..
        } | IloKind::SignedIn { .. }
    );
    match site {
        true => format!("{own} {name}"),
        false => format!("{own} \u{201c}{name}\u{201d}"),
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
        IloKind::Helpers { .. } => None,
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
        IloButton::HandBack => "Hand Back",
        IloButton::NotNow => "Not Now",
        IloButton::StartAgain => "Start Again",
    }
}

/// What a line's buttons promise, under them, if anything.
pub fn note(buttons: &[IloButton]) -> Option<&'static str> {
    (buttons.contains(&IloButton::Raise(Tier::Full)))
        .then_some("Full access lasts for this task only.")
}
