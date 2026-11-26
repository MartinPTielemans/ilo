//! ilo's own lines in a task's conversation: what the browser stopped, a step that's the user's, a
//! tab the agent needs back, a sign-in ilo did, a change of the task's access and that the user
//! stopped it. And of a task
//! that has ended (#4105): that the user's reply took it up, and in the reply field's place, that
//! its assistant no longer has the conversation or keeps none, or that its profile was deleted.
//! And of a task with files (#4101) whose assistant ilo couldn't limit to them: that it started
//! without them, stayed at its level or went back to it, or stopped; and that ilo couldn't end
//! everything its assistant had started, or later did.
//!
//! An ilo line is its own kind of entry, never a message. Only the app makes one, from its own
//! events: nothing an agent sends, no tool's argument and no page's text becomes one, so nothing
//! they write can draw its mark or its buttons. Its words come from its kind (the conversation's
//! words table) and its buttons from its kind alone (`buttons`). The only text it carries from
//! elsewhere is a name, the page's button ilo stopped at, a site, or a folder or file the user
//! gave, which it draws as capped plain text set apart from its own words. Why a step is the user's is the agent's own message above
//! the line, in the agent's words; the line says only that it's theirs.

use gpui::SharedString;

use crate::agents::tier::{Tier, Why};

/// One of ilo's lines.
#[derive(Clone, Debug, PartialEq)]
pub struct IloLine {
    /// Its key, which the user's press on one of its buttons names.
    pub key: usize,
    pub kind: IloKind,
    /// Whether the user acted on it, or the task went on past it: its buttons go.
    pub settled: bool,
}

/// Why ilo stopped a task over its helpers (#4102).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpersLimit {
    /// It started more than a task may for one message of the user's: its turn is stopped, and
    /// the task goes on.
    Count,
    /// A helper started a helper of its own: the task is stopped.
    Depth,
    /// A helper ilo doesn't allow: of a kind ilo didn't make, one in the background, or one that
    /// called a tool that is the task's alone. The task is stopped.
    Kind,
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
        /// The refused step's under-way phrase, its first letter lowered, as the line says it
        /// after "from": "clicking “Place order”". Empty where its step isn't traced.
        doing: SharedString,
        why: Why,
        /// The access it was judged under.
        tier: Tier,
        /// The times it was refused the same, which stay one line.
        count: u32,
    },
    /// A step only the user can do in `tab`: a sign-in, a code, a CAPTCHA, a file or a passkey.
    /// `site` is the page's, named while the task has more than one page, so the line says which.
    Yours {
        tab: u64,
        site: Option<SharedString>,
    },
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
    /// The user stopped the task, which held `pages` pages then: a count, never a page's title or
    /// its site.
    YouStopped { pages: usize },
    /// ilo stopped the task over its helpers (#4102), by `limit`: its turn where it started more
    /// than a task may, the task itself where a helper started one of its own or isn't one ilo
    /// allows. `tab` is the page the task stood on, or 0 with none. The count its line says is
    /// the app's own.
    Helpers { tab: u64, limit: HelpersLimit },
    /// The task started, or went on, without files the user gave it, as ilo couldn't limit its
    /// assistant to them (#4101).
    StartedWithout { left: LeftOut },
    /// The task stays at `at`, as ilo couldn't limit its assistant to `folder` at `refused`, the
    /// level the user asked for, which it never had.
    KeptAt {
        at: Tier,
        refused: Tier,
        folder: SharedString,
    },
    /// The same once the task had been at `refused`, under the line that says the user set it:
    /// ilo set it back to `at`.
    SetBack {
        at: Tier,
        refused: Tier,
        folder: SharedString,
    },
    /// The task stopped, as ilo couldn't start its assistant again without what the user took
    /// back.
    NotRestarted { left: LeftOut },
    /// The task stopped, as ilo couldn't start its assistant again at `refused`, the level the
    /// user raised it to.
    NotRaised { refused: Tier },
    /// ilo couldn't end everything the assistant had started, which can still reach what the
    /// task was given.
    LeftRunning { reach: Reach },
    /// ilo later ended what the assistant had left running.
    EndedLeft,
    /// The user revoked the task's folder; it goes on without it (J2).
    Revoked { folder: SharedString },
    /// The task's folder was moved, renamed, deleted or its disk ejected, so ilo ended its grant;
    /// it goes on without it (J6).
    FolderGone { folder: SharedString },
    /// ilo couldn't copy the files the user attached, so the task started without them.
    NotCopied { left: LeftOut },
    /// The user's reply took up the task after it had ended, at `tier`, which the app decided.
    Resumed { tier: Tier },
    /// The assistant keeps the task's conversation on the user's computer, said once per profile.
    KeptHere,
    /// The assistant no longer has the task's conversation, which it kept. In the reply field's
    /// place.
    Gone,
    /// The assistant kept no conversation of the task, which ended while Settings' switch was
    /// off. In the reply field's place.
    NotKept,
    /// The assistant keeps no finished task's conversation. In the reply field's place.
    KeepsNothing,
    /// The profile the task was asked in was deleted, so it has nowhere to work: a task a run of
    /// the browser before left. In the reply field's place.
    NoProfile,
    /// The profile the task was asked in has no Space of the user's left, so the task has
    /// nowhere to work and its pages nowhere to go: a task a run of the browser before left. In
    /// the reply field's place.
    NoSpace,
}

/// What a task went on without, or couldn't be started again without, by the name the app shows
/// for it.
#[derive(Clone, Debug, PartialEq)]
pub enum LeftOut {
    /// The folder the user gave.
    Folder(SharedString),
    /// The one file they attached.
    File(SharedString),
    /// The files they attached, more than one, which the line counts.
    Files(usize),
}

/// What the processes a run left can still reach of what its task was given: the folder, by the
/// name the app shows for it, the files the user attached, or both.
#[derive(Clone, Debug, PartialEq)]
pub enum Reach {
    Folder(SharedString),
    Files,
    Both(SharedString),
}

/// Whether a line stands in the reply field's place, under the log, as the task takes no reply.
pub fn in_field(kind: &IloKind) -> bool {
    matches!(
        kind,
        IloKind::Gone
            | IloKind::NotKept
            | IloKind::KeepsNothing
            | IloKind::NoProfile
            | IloKind::NoSpace
    )
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
    /// The tab goes back to the agent.
    HandBack,
    /// The pages of an outside agent's task the user stopped are its client's again.
    PagesBack,
    /// The page is shown beside the conversation, and nothing else changes: a step's first
    /// button while its page isn't the one shown.
    ShowPage,
    /// The user leaves the step for now.
    NotNow,
    /// The command bar opens with what the task was first asked, to ask it anew: nothing is sent
    /// until the user sends it.
    StartAgain,
}

impl IloButton {
    /// Whether it widens what the agent may do, and so takes no press until its line has stood
    /// still for a moment: an agent decides when such a line comes.
    pub fn widens(self) -> bool {
        matches!(self, IloButton::Raise(_) | IloButton::PagesBack)
    }
}

/// A line's buttons, first the one the user most likely wants, from its kind alone. A settled line
/// has none, and nor does one that only says what happened.
pub fn buttons(line: &IloLine) -> Vec<IloButton> {
    if line.settled {
        return Vec::new();
    }
    match &line.kind {
        // The Off limits list and a Never are the user's own, which no step up passes.
        IloKind::Stopped {
            why: Why::Fenced | Why::Never,
            ..
        } => vec![IloButton::TakeOver],
        // A step up only where it would let the act through.
        IloKind::Stopped { tier, why, .. } => std::iter::once(IloButton::TakeOver)
            .chain(tier.lifting(*why).map(IloButton::Raise))
            .collect(),
        // No level lifts the limit, so no step up stands under its line; with no page to take,
        // nothing does, nor where ilo stopped the task itself.
        IloKind::Helpers { tab: 0, .. } => Vec::new(),
        IloKind::Helpers {
            limit: HelpersLimit::Depth | HelpersLimit::Kind,
            ..
        } => Vec::new(),
        IloKind::Helpers { .. } => vec![IloButton::TakeOver],
        IloKind::Yours { .. } => vec![IloButton::HandBack, IloButton::NotNow],
        // Not Now keeps the tab, as the user's own.
        IloKind::TabBack { .. } => vec![IloButton::HandBack, IloButton::NotNow],
        IloKind::SignedIn { .. } | IloKind::TierChanged { .. } => Vec::new(),
        // Its one button, on an outside agent's task only, until pressed: the line of a task ilo
        // runs is settled from the start.
        IloKind::YouStopped { .. } => vec![IloButton::PagesBack],
        // What happened, which no press undoes: the files are given again at a task's start.
        IloKind::StartedWithout { .. }
        | IloKind::KeptAt { .. }
        | IloKind::SetBack { .. }
        | IloKind::NotRestarted { .. }
        | IloKind::NotRaised { .. }
        | IloKind::LeftRunning { .. }
        | IloKind::EndedLeft
        | IloKind::Revoked { .. }
        | IloKind::FolderGone { .. }
        | IloKind::NotCopied { .. } => Vec::new(),
        IloKind::Resumed { .. } | IloKind::KeptHere => Vec::new(),
        IloKind::Gone
        | IloKind::NotKept
        | IloKind::KeepsNothing
        | IloKind::NoProfile
        | IloKind::NoSpace => {
            vec![IloButton::StartAgain]
        }
    }
}

/// A line's buttons as drawn with `shown`, the page in the area: a step's page opens by itself as
/// it turns to Needs you, so its line offers Hand Back at once, and Show Page only while the user
/// has put another of the task's pages there (O2).
pub fn drawn(line: &IloLine, shown: Option<u64>) -> Vec<IloButton> {
    let away = matches!(&line.kind, IloKind::Yours { tab, site: Some(_) }
        if shown.is_some_and(|shown| shown != *tab));
    (buttons(line).into_iter())
        .map(|button| match button {
            IloButton::HandBack if away => IloButton::ShowPage,
            button => button,
        })
        .collect()
}

/// The tab a line is about, which Take over and Hand back act on.
pub fn tab(kind: &IloKind) -> Option<u64> {
    match kind {
        IloKind::Stopped { tab, .. }
        | IloKind::Helpers { tab, .. }
        | IloKind::Yours { tab, .. }
        | IloKind::TabBack { tab }
        | IloKind::SignedIn { tab, .. } => Some(*tab),
        IloKind::TierChanged { .. } | IloKind::YouStopped { .. } => None,
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
        IloKind::Resumed { .. } | IloKind::KeptHere => None,
        IloKind::Gone
        | IloKind::NotKept
        | IloKind::KeepsNothing
        | IloKind::NoProfile
        | IloKind::NoSpace => None,
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
                doing: "clicking “Place order”".into(),
                why,
                tier,
                count: 1,
            },
            settled: false,
        }
    }

    /// Each line's buttons, from its kind: a refusal offers the one step up, none from Full access
    /// and none past the user's own Off limits list; a settled line none.
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
            // Work on pages sends no message, so a message refused at Look only offers Full.
            (
                stopped(Why::Message, Tier::Look),
                vec![TakeOver, Raise(Tier::Full)],
            ),
            (stopped(Why::Fenced, Tier::Work), vec![TakeOver]),
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
            buttons(&line(IloKind::Yours { tab: 1, site: None })),
            vec![HandBack, NotNow]
        );
        assert_eq!(
            buttons(&line(IloKind::TabBack { tab: 1 })),
            vec![HandBack, NotNow]
        );
        let changed = IloKind::TierChanged {
            from: Tier::Work,
            to: Tier::Full,
        };
        assert_eq!(buttons(&line(changed)), vec![]);
    }

    /// O2: a step on a page while the user has another of the task's pages in the area offers
    /// Show Page, never Hand Back, until its page is the one shown. With none there its page opens
    /// by itself (i), and with one page it's the task's, so either offers Hand Back at once.
    #[test]
    fn a_step_shows_its_page_before_it_offers_it_back() {
        use IloButton::*;
        let line = |site: Option<&str>| IloLine {
            key: 0,
            kind: IloKind::Yours {
                tab: 2,
                site: site.map(SharedString::from),
            },
            settled: false,
        };
        let two = line(Some("boxoffice.example"));
        assert_eq!(drawn(&two, None), vec![HandBack, NotNow]);
        assert_eq!(drawn(&two, Some(1)), vec![ShowPage, NotNow]);
        assert_eq!(drawn(&two, Some(2)), vec![HandBack, NotNow]);
        assert_eq!(drawn(&line(None), None), vec![HandBack, NotNow]);
        let settled = IloLine {
            settled: true,
            ..two
        };
        assert_eq!(drawn(&settled, None), vec![]);
    }
}
