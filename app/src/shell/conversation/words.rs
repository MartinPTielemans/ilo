//! Every word a task's conversation says as ilo: its lines, their buttons, the live line and the
//! composer's row. One table, so a change of wording after the owner has used a build is an edit
//! here (#4086). The tiers' names are the tier table's (agents/tier.rs).

use crate::agents::kept;
use crate::agents::task::ilo::{IloButton, IloKind, SignedInWith};
use crate::agents::tier::{self, Tier, Why};

/// ilo's mark, before each of its lines' words.
pub const MARK: &str = "ilo";
/// The live line's button, and a Stopped line's first.
pub const TAKE_OVER: &str = "Take over";
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
        IloKind::Yours { .. } => ("this step is yours".into(), None),
        IloKind::TabBack { .. } => (format!("{agent} needs this tab back"), None),
        IloKind::SignedIn { site, .. } => (
            "filled and sent the sign-in form on".into(),
            Some(site.to_string()),
        ),
        IloKind::TierChanged { to, .. } => (format!("· You set this task to {}", to.label()), None),
        IloKind::Resumed { tier } => (format!("resumed this task at {}", tier.label()), None),
        // Where the assistant is the subject, a middle dot sets ilo's mark off from the sentence.
        // A kept task takes a reply while the browser runs, and the words promise no more.
        IloKind::KeptHere => (
            format!(
                "· {agent} keeps this conversation on {} until you quit {}, so you can reply.",
                kept::COMPUTER,
                env!("APP_NAME")
            ),
            None,
        ),
        IloKind::Gone => (format!("· {agent} no longer has this conversation."), None),
        IloKind::NotKept => (format!("· {agent} didn't keep this conversation."), None),
        IloKind::KeepsNothing => (format!("· {agent} doesn't keep finished tasks."), None),
    }
}

/// Where the switch that keeps finished tasks is, under the line that says they're kept.
const KEPT_IN: &str = "Settings › Agents";

/// What a line says under its head, if anything: why ilo stopped, in the user's terms, or what it
/// signed in with. `at` is the line's name, already cut to its length.
pub fn body(kind: &IloKind, at: &str) -> Option<String> {
    match kind {
        IloKind::Stopped { why, tier, .. } => Some(tier::why_line(*why, *tier, at)),
        IloKind::SignedIn { with, .. } => Some(match with {
            // Labels, without a full stop.
            SignedInWith::Saved(account) => format!("With your saved login, {}", account.drawn()),
            SignedInWith::Extension(name) => format!("With {name}"),
        }),
        IloKind::Yours { .. } | IloKind::TabBack { .. } | IloKind::TierChanged { .. } => None,
        IloKind::KeptHere => Some(KEPT_IN.into()),
        IloKind::Resumed { .. } => None,
        IloKind::Gone | IloKind::NotKept | IloKind::KeepsNothing => None,
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
        IloButton::HandBack => "Hand back",
        IloButton::NotNow => "Not now",
        IloButton::StartAgain => "Start Again",
    }
}

/// What a line's buttons promise, under them, if anything.
pub fn note(buttons: &[IloButton]) -> Option<&'static str> {
    (buttons.contains(&IloButton::Raise(Tier::Full)))
        .then_some("Full access lasts for this task only.")
}
