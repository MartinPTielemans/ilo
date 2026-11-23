//! A task's access: how far its agent may act on pages without the user. ilo runs what's inside it
//! and refuses what's above it, never asking, and only the user changes it (#4086). What each one
//! refuses is a row of this table, so nothing else decides.

/// A task's access, lowest first, which their order follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    /// It opens pages, reads, scrolls and takes pictures, and changes nothing.
    Look,
    /// It types, presses, fills in and sends forms, but not what ilo recognises as paying, placing
    /// an order, sending a message as the user, or a bank's site. Where every task starts.
    #[default]
    Work,
    /// Everything the user could do on a page. Set by the user for one task, never a default and
    /// never remembered.
    Full,
}

impl Tier {
    /// Every tier, lowest first, as its menu lists them.
    pub const ALL: [Tier; 3] = [Tier::Look, Tier::Work, Tier::Full];

    /// Its name, as its chip, its menu and an ilo line say it.
    pub fn label(self) -> &'static str {
        match self {
            Tier::Look => "Look only",
            Tier::Work => "Work on pages",
            Tier::Full => "Full access",
        }
    }

    /// The tier one up, which a refusal offers, or None from the top.
    pub fn up(self) -> Option<Tier> {
        match self {
            Tier::Look => Some(Tier::Work),
            Tier::Work => Some(Tier::Full),
            Tier::Full => None,
        }
    }

    /// Whether an act refused for `why` happens at this tier.
    pub fn allows(self, why: Why) -> bool {
        let row = ROWS.iter().find(|row| row.why == why).unwrap();
        row.allowed[self as usize]
    }

    /// The tier's promise, as its menu says it.
    pub fn promise(self) -> &'static str {
        match self {
            Tier::Look => "Opens pages and reads them, and does nothing on them.",
            Tier::Work => {
                "Refuses what it recognises as paying, placing an order, sending a message as you, \
                 or a bank's site."
            }
            Tier::Full => {
                "Nothing but the agent's own judgment stands between a page and a payment. Full \
                 access lasts for this task only."
            }
        }
    }
}

/// What an act does that a tier may refuse. An act that only opens or reads a page has none and
/// happens at every tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Why {
    /// Types, presses, chooses or sends a key that acts on a page.
    Act,
    /// Sends a message or a post as the user (page::SENDS).
    Message,
    /// A payment or an order (page::ORDER).
    Order,
    /// Opens a bank's site, or acts on one.
    Bank,
    /// Saves a file to the user's computer.
    Download,
    /// Closes a page the user gave it, losing what it hadn't saved.
    Discard,
    /// An HTTP request that isn't GET or HEAD.
    Request,
    /// A site on the user's Off limits list, which no tier lifts.
    Fenced,
    /// A site outside the task's Sites, which no tier lifts either.
    OtherSite,
    /// Signs in with what the browser keeps (#4098): a stub row until that tool comes.
    SignIn,
    /// Signs in on a bank's site.
    SignInBank,
}

/// A row: what each tier, Look only, Work on pages and Full access, does with an act of `why`.
struct Row {
    why: Why,
    allowed: [bool; 3],
    /// What was refused, as the user's line says it, before "isn't part of <tier>".
    refused: &'static str,
}

const ROWS: [Row; 11] = [
    Row {
        why: Why::Act,
        allowed: [false, true, true],
        refused: "Acting on a page",
    },
    Row {
        why: Why::Message,
        allowed: [false, false, true],
        refused: "Sending a message as you",
    },
    Row {
        why: Why::Order,
        allowed: [false, false, true],
        refused: "Paying or placing an order",
    },
    Row {
        why: Why::Bank,
        allowed: [false, false, true],
        refused: "Using a bank's site",
    },
    Row {
        why: Why::Download,
        allowed: [false, true, true],
        refused: "Downloading a file",
    },
    Row {
        why: Why::Discard,
        allowed: [false, false, true],
        refused: "Closing a page you gave it without saving",
    },
    Row {
        why: Why::Request,
        allowed: [false, false, true],
        refused: "A request that may change something",
    },
    Row {
        why: Why::Fenced,
        allowed: [false, false, false],
        refused: "",
    },
    Row {
        why: Why::OtherSite,
        allowed: [false, false, false],
        refused: "",
    },
    Row {
        why: Why::SignIn,
        allowed: [false, true, true],
        refused: "Signing in",
    },
    Row {
        why: Why::SignInBank,
        allowed: [false, false, true],
        refused: "Signing in to a bank's site",
    },
];

/// The sites a task may use (#4094's Sites): any, or only the registrable domains of the pages the
/// user gave it. A site outside them is refused at every tier.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Sites {
    #[default]
    Any,
    Only(Vec<String>),
}

impl Sites {
    /// Whether the site whose registrable domain is `domain` is one of them. A page with none,
    /// off the web, is the core's to refuse.
    pub fn allows(&self, domain: Option<&str>) -> bool {
        match (self, domain) {
            (Sites::Any, _) | (_, None) => true,
            (Sites::Only(domains), Some(domain)) => domains.iter().any(|each| each == domain),
        }
    }
}

/// A refusal the user hasn't settled, which their task's conversation shows as ilo's line: the
/// same one again counts on it, so retrying can't fill the conversation or press the user.
#[derive(Clone, Debug, PartialEq)]
pub struct Stop {
    pub tab: u64,
    /// What it stopped, as the step names it, such as a button's name: page text, plain and capped.
    pub at: String,
    pub site: String,
    pub why: Why,
    pub tier: Tier,
    pub count: u32,
    /// When it first showed, which a click already under way then mustn't answer.
    pub appeared: std::time::Instant,
    /// How many of the task's messages came before it, which places it among them.
    pub after: usize,
    /// Acted on, or passed: a change of tier, Take over, or the task's end.
    pub settled: bool,
}

impl Stop {
    /// Whether it's the same refusal as one of `why` on `tab` at `at` and `tier`.
    pub fn same(&self, tab: u64, at: &str, why: Why, tier: Tier) -> bool {
        !self.settled && self.tab == tab && self.at == at && self.why == why && self.tier == tier
    }
}

/// The user's line's sentence on why ilo stopped an act at `tier` on `site`.
pub fn why_line(why: Why, tier: Tier, site: &str) -> String {
    match why {
        Why::Fenced => format!("{site} is on your Off limits list."),
        Why::OtherSite => format!("{site} isn't one of the sites this task may use."),
        _ => {
            let row = ROWS.iter().find(|row| row.why == why).unwrap();
            format!("{} isn't part of {}.", row.refused, tier.label())
        }
    }
}

/// What the agent's call answers: what was refused, that it's outside the task's access, that only
/// the user changes that, and not to try another way. It never names a tier to ask for. `what` is
/// the act as the agent's own call put it, page text included, so it's set in quotes, capped.
pub fn told(why: Why, tier: Tier, what: &str, site: &str) -> String {
    let what = capped(what, TOLD_MOST);
    match why {
        Why::Fenced => format!(
            "ilo stopped this: {what}. {site} is off limits to agents, whatever a task's access. \
             Only the user can change that, in ilo. Don't try another way to the same end."
        ),
        Why::OtherSite => format!(
            "ilo stopped this: {what}. {site} is outside the sites this task may use. Only the user \
             can change that, in ilo. Don't try another way to the same end; if they change it, \
             the same call will go through."
        ),
        _ => format!(
            "ilo stopped this: {what}. It isn't part of this task's access ({}). Only the user can \
             change a task's access, in ilo. Don't try another way to the same end; if they change \
             it, the same call will go through.",
            tier.label()
        ),
    }
}

/// The turn ilo sends a task it runs itself once the user changes its tier from `from` to `to`:
/// fixed words, with nothing of a page's or the agent's in them.
pub fn changed_turn(from: Tier, to: Tier) -> String {
    match to as usize > from as usize {
        true => format!(
            "The user changed this task's access to {}. You may go on.",
            to.label()
        ),
        false => format!(
            "The user changed this task's access to {}. What it doesn't allow will be refused.",
            to.label()
        ),
    }
}

/// The activity log's words for a change of tier, which its entry keeps as its target.
pub fn changed_logged(from: Tier, to: Tier) -> String {
    format!("{} → {}", from.label(), to.label())
}

/// The most of a page's text a line carries.
pub const TEXT_MOST: usize = 80;

/// The most of an act's description, page text in it quoted, a refusal tells its agent: enough
/// for a press that may click a button after typing into a field.
pub const TOLD_MOST: usize = 200;

/// `text` as one line of at most TEXT_MOST characters: what a line shows of a page's text.
pub fn plain(text: &str) -> String {
    capped(text, TEXT_MOST)
}

/// `text` as one line of at most `most` characters, its control characters spaces and its
/// direction marks gone, so it can't run on into ilo's words or turn them around.
fn capped(text: &str, most: usize) -> String {
    let line: String = (text.chars())
        .filter(|c| !matches!(c, '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'))
        .map(|c| match c.is_control() {
            true => ' ',
            false => c,
        })
        .collect();
    let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
    match line.chars().count() > most {
        true => format!("{}…", line.chars().take(most - 1).collect::<String>()),
        false => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_tier_allows_all_the_one_below_does() {
        for row in &ROWS {
            for pair in Tier::ALL.windows(2) {
                assert!(
                    !pair[0].allows(row.why) || pair[1].allows(row.why),
                    "{:?} allowed at {:?} but not at {:?}",
                    row.why,
                    pair[0],
                    pair[1]
                );
            }
        }
    }

    #[test]
    fn the_table_is_the_briefs() {
        use Tier::*;
        use Why::*;
        let at = |why| Tier::ALL.map(|tier| tier.allows(why));
        assert_eq!(at(Act), [false, true, true]);
        assert_eq!(at(Message), [false, false, true]);
        assert_eq!(at(Order), [false, false, true]);
        assert_eq!(at(Bank), [false, false, true]);
        assert_eq!(at(Download), [false, true, true]);
        assert_eq!(at(Discard), [false, false, true]);
        assert_eq!(at(Request), [false, false, true]);
        // A fence and the task's Sites are the user's own, and no tier lifts them.
        assert_eq!(at(Fenced), [false, false, false]);
        assert_eq!(at(OtherSite), [false, false, false]);
        assert_eq!(at(SignIn), [false, true, true]);
        assert_eq!(at(SignInBank), [false, false, true]);
        assert_eq!(Tier::default(), Work);
        assert_eq!(Look.up(), Some(Work));
        assert_eq!(Work.up(), Some(Full));
        assert_eq!(Full.up(), None);
    }

    #[test]
    fn a_refusal_names_no_tier_to_ask_for() {
        for row in &ROWS {
            for tier in Tier::ALL {
                let told = told(row.why, tier, "click “Pay”", "shop.example");
                for other in Tier::ALL.iter().filter(|other| **other != tier) {
                    assert!(
                        !told.contains(other.label()),
                        "{told} names {}",
                        other.label()
                    );
                }
                assert!(told.contains("Don't try another way"), "{told}");
            }
        }
    }

    #[test]
    fn page_text_is_one_capped_line() {
        let long = format!("Pay\nnow\u{2028}\u{202E}{}", "x".repeat(300));
        let line = plain(&long);
        assert!(line.starts_with("Pay now x"), "{line}");
        assert_eq!(line.chars().count(), TEXT_MOST);
        let told = told(Why::Order, Tier::Work, &long, "shop.example");
        assert!(!told.contains(['\n', '\u{2028}', '\u{202E}']), "{told}");
        assert!(
            told.contains(&format!("{}…. It isn't", "x".repeat(10))),
            "{told}"
        );
    }

    #[test]
    fn the_turn_after_a_change_is_ilos_own_words() {
        assert_eq!(
            changed_turn(Tier::Work, Tier::Full),
            "The user changed this task's access to Full access. You may go on."
        );
        assert!(changed_turn(Tier::Full, Tier::Look).contains("will be refused"));
    }
}

/// Why ilo refused an act: the rows the tier table keys on, which an ilo line words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Why {
    /// A change on a page: typing, pressing, choosing.
    Act,
    /// Sending a message as the user.
    Message,
    /// Paying or placing an order.
    Order,
    /// A bank's site.
    Bank,
    /// Signing in for the agent with a saved login (#4098).
    SignIn,
    /// Signing in for the agent on a bank's site.
    SignInBank,
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
