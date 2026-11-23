//! ilo's own lines in a task's conversation: what the browser stopped, a step that's the user's, a
//! tab the agent needs back, a sign-in ilo did and a change of the task's access.
//!
//! An ilo line is its own kind of entry, never a message. Only the app makes one, from its own
//! events: nothing an agent sends, no tool's argument and no page's text becomes one, so nothing
//! they write can draw its mark or its buttons. Its words come from its kind (the conversation's
//! words table) and its buttons from its kind alone (`buttons`). The only text it carries from
//! elsewhere is a name, the page's button ilo stopped at or a site, which it draws as capped plain
//! text set apart from its own words. Why a step is the user's is the agent's own message above
//! the line, in the agent's words; the line says only that it's theirs.

use gpui::SharedString;

use crate::agents::tier::Tier;

/// One of ilo's lines.
#[derive(Clone, Debug, PartialEq)]
pub struct IloLine {
    /// Its key, which the user's press on one of its buttons names.
    pub key: usize,
    pub kind: IloKind,
    /// Whether the user acted on it, or the task went on past it: its buttons go.
    pub settled: bool,
}

/// What an ilo line says.
#[derive(Clone, Debug, PartialEq)]
pub enum IloKind {
    /// ilo refused an act above the task's access, or on a site outside it, in `tab`.
    Stopped {
        tab: u64,
        /// Where, in the page's own words: the button ilo stopped the agent at, "Place order", or
        /// the site it was kept from.
        at: SharedString,
        why: Why,
        /// The access it was judged under.
        tier: Tier,
        /// The times it was refused the same, which stay one line.
        count: u32,
    },
    /// A step only the user can do in `tab`: a sign-in, a code, a CAPTCHA, a file or a passkey.
    Yours { tab: u64 },
    /// The agent needs `tab` back from the user.
    TabBack { tab: u64 },
    /// ilo signed in for the agent on `site`, in `tab`.
    SignedIn {
        tab: u64,
        site: SharedString,
        with: SignedInWith,
    },
    /// The user changed the task's access.
    TierChanged { from: Tier, to: Tier },
}

/// Why ilo refused, as the tier table names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// A change on a page: typing, pressing, choosing.
    Act,
    /// Sending a message as the user.
    Message,
    /// Paying or placing an order.
    Order,
    /// A bank's site.
    Bank,
    /// A download.
    Download,
    /// Throwing away what the user wrote in a tab the agent didn't open.
    Discard,
    /// A request that may change something.
    Request,
    /// A site on the user's Off limits list.
    Fenced,
    /// A site outside the pages the task was given.
    OtherSite,
}

/// What ilo signed in with.
#[derive(Clone, Debug, PartialEq)]
pub enum SignedInWith {
    /// A login saved in ilo, by its account's name.
    Saved(SharedString),
    /// A password extension, by its name.
    Extension(SharedString),
}

/// What a press on an ilo line's button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IloButton {
    /// The page is the user's.
    TakeOver,
    /// The task's access goes up to this one.
    Raise(Tier),
    /// The task may go to any site.
    AnySite,
    /// The tab goes back to the agent.
    HandBack,
    /// The user leaves the step for now.
    NotNow,
}

impl IloButton {
    /// Whether it widens what the agent may do, and so takes no press until its line has stood
    /// still for a moment: an agent decides when such a line comes.
    pub fn widens(self) -> bool {
        matches!(self, IloButton::Raise(_) | IloButton::AnySite)
    }
}

/// A line's buttons, first the one the user most likely wants, from its kind alone. A settled line
/// has none, and nor does one that only says what happened.
pub fn buttons(line: &IloLine) -> Vec<IloButton> {
    if line.settled {
        return Vec::new();
    }
    match &line.kind {
        // The Off limits list is the user's own, which no step up passes.
        IloKind::Stopped {
            why: Why::Fenced, ..
        } => vec![IloButton::TakeOver],
        IloKind::Stopped {
            why: Why::OtherSite,
            ..
        } => vec![IloButton::TakeOver, IloButton::AnySite],
        IloKind::Stopped { tier, .. } => std::iter::once(IloButton::TakeOver)
            .chain(tier.up().map(IloButton::Raise))
            .collect(),
        IloKind::Yours { .. } => vec![IloButton::HandBack, IloButton::NotNow],
        IloKind::TabBack { .. } => vec![IloButton::HandBack],
        IloKind::SignedIn { .. } | IloKind::TierChanged { .. } => Vec::new(),
    }
}

/// The tab a line is about, which Take over and Hand back act on.
pub fn tab(kind: &IloKind) -> Option<u64> {
    match kind {
        IloKind::Stopped { tab, .. }
        | IloKind::Yours { tab }
        | IloKind::TabBack { tab }
        | IloKind::SignedIn { tab, .. } => Some(*tab),
        IloKind::TierChanged { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stopped(why: Why, tier: Tier) -> IloLine {
        IloLine {
            key: 0,
            kind: IloKind::Stopped {
                tab: 1,
                at: "Place order".into(),
                why,
                tier,
                count: 1,
            },
            settled: false,
        }
    }

    /// Each line's buttons, from its kind: a refusal offers the one step up, none from Full access
    /// and none past the user's own Off limits list; Sites offers Any site; a settled line none.
    #[test]
    fn a_lines_buttons_come_from_its_kind() {
        use IloButton::*;
        let cases = [
            (
                stopped(Why::Act, Tier::Look),
                vec![TakeOver, Raise(Tier::Work)],
            ),
            (
                stopped(Why::Order, Tier::Work),
                vec![TakeOver, Raise(Tier::Full)],
            ),
            (stopped(Why::Order, Tier::Full), vec![TakeOver]),
            (stopped(Why::Fenced, Tier::Work), vec![TakeOver]),
            (stopped(Why::OtherSite, Tier::Full), vec![TakeOver, AnySite]),
            (
                IloLine {
                    settled: true,
                    ..stopped(Why::Order, Tier::Work)
                },
                vec![],
            ),
        ];
        for (line, want) in cases {
            assert_eq!(buttons(&line), want, "{line:?}");
        }
        let line = |kind| IloLine {
            key: 0,
            kind,
            settled: false,
        };
        assert_eq!(
            buttons(&line(IloKind::Yours { tab: 1 })),
            vec![HandBack, NotNow]
        );
        assert_eq!(buttons(&line(IloKind::TabBack { tab: 1 })), vec![HandBack]);
        let changed = IloKind::TierChanged {
            from: Tier::Work,
            to: Tier::Full,
        };
        assert_eq!(buttons(&line(changed)), vec![]);
    }
}
