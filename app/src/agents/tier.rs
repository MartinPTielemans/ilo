//! A task's access: how far its agent may act on pages without the user. ilo runs what's inside it
//! and refuses what's above it, never asking, and only the user changes it (#4086). What each one
//! refuses is a row of this table, so nothing else decides.

/// Settings › Agents' group that explains each tier, the one place that does; the level's menu
/// opens Settings there by this name (settings::show_row).
pub const ACCESS: &str = "Access";

/// The level's menu's rows for where a task may go: every site, the default, or only the sites of
/// the pages the user added to it.
pub const ANY_SITE: &str = "Any site";
pub const ONLY_ADDED: &str = "Only the sites you add";

/// Settings › Agents' line under its name, the one place it says what a task never gets.
pub const NEVER_GETS: &str = "A thread never gets passwords, cookies or your other tabs.";

/// A task's access, lowest first, which their order follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    /// It opens pages, reads, scrolls and takes pictures, and changes nothing.
    Look,
    /// It types, presses, fills in and sends forms, but not what ilo recognises as paying, placing
    /// an order, sending a message as the user, or a bank's site. Where every task starts, but
    /// an outside client's whose others the user left lower (`Agents::starting_tier`).
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

    /// Its name in the saved settings, which stays when its label changes.
    pub fn id(self) -> &'static str {
        match self {
            Tier::Look => "look",
            Tier::Work => "work",
            Tier::Full => "full",
        }
    }

    pub fn from_id(id: &str) -> Option<Tier> {
        Tier::ALL.into_iter().find(|tier| tier.id() == id)
    }

    /// The lowest tier above this one at which an act refused for `why` happens, which its
    /// refusal offers, or None if none does.
    pub fn lifting(self, why: Why) -> Option<Tier> {
        (Tier::ALL.into_iter()).find(|tier| *tier as usize > self as usize && tier.allows(why))
    }

    /// What it lets a task do, under its name in Settings › Agents' Access, each past the one below.
    pub fn explained(self) -> &'static str {
        match self {
            Tier::Look => "Reads pages.",
            Tier::Work => "Also clicks, types and submits forms.",
            Tier::Full => "Also pays, orders, sends messages and uses bank sites.",
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
            Tier::Work => concat!(
                "Clicks, types and submits forms. ",
                env!("PRODUCT_NAME"),
                " stops at paying, ordering, sending messages as you and bank sites."
            ),
            Tier::Full => {
                "Everything you can do on a page, including paying, ordering, messages and bank \
                 sites."
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
    /// A site the user answered Never for this agent, which no tier lifts until they revoke it in
    /// Settings.
    Never,
    /// Signs in with what the browser keeps (#4098): a stub row until that tool comes.
    SignIn,
    /// Signs in on a bank's site.
    SignInBank,
}

/// A row: what each tier, Look only, Work on pages and Full access, does with an act of `why`.
struct Row {
    why: Why,
    allowed: [bool; 3],
    /// The user's line's whole sentence on the refusal, `{level}` standing for the tier: one per
    /// act, whose first words also name the act in the activity log. Fenced, OtherSite and Never
    /// name the site instead (`why_line`).
    refused: &'static str,
}

const ROWS: [Row; 12] = [
    Row {
        why: Why::Act,
        allowed: [false, true, true],
        refused: "Changing a page isn't part of {level}.",
    },
    Row {
        why: Why::Message,
        allowed: [false, false, true],
        refused: "Sending a message as you isn't part of {level}.",
    },
    Row {
        why: Why::Order,
        allowed: [false, false, true],
        refused: "Placing an order isn't part of {level}.",
    },
    Row {
        why: Why::Bank,
        allowed: [false, false, true],
        refused: "Using a bank site isn't part of {level}.",
    },
    Row {
        why: Why::Download,
        allowed: [false, true, true],
        refused: "Downloading a file isn't part of {level}.",
    },
    Row {
        why: Why::Discard,
        allowed: [false, false, true],
        refused: "Throwing away what you wrote isn't part of {level}.",
    },
    Row {
        why: Why::Request,
        allowed: [false, false, true],
        refused: "Sending changes to a site without its page isn't part of {level}.",
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
        why: Why::Never,
        allowed: [false, false, false],
        refused: "",
    },
    Row {
        why: Why::SignIn,
        allowed: [false, true, true],
        refused: "Signing in with your saved login isn't part of {level}.",
    },
    Row {
        why: Why::SignInBank,
        allowed: [false, false, true],
        refused: "Signing in to a bank site isn't part of {level}.",
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

/// How long after a refusal's line appears a press on its step up or Any site is ignored, as an
/// agent card's buttons ignore keys (agent_card_keys::PROTECT): a click meant for what was there
/// lands on a button that has just come. Where a line moves is the view's to protect.
pub const PROTECT: std::time::Duration = std::time::Duration::from_millis(500);

/// A refusal the user hasn't settled, which their task's conversation shows as ilo's line: the
/// same one again counts on it, so retrying can't fill the conversation or press the user.
#[derive(Clone, Debug, PartialEq)]
pub struct Stop {
    pub tab: u64,
    /// What it stopped, as the step names it, such as a button's name: page text, plain and capped.
    pub at: String,
    pub site: String,
    /// The refused step's under-way phrase, its first letter lowered (`IloKind::Stopped`).
    pub doing: String,
    pub why: Why,
    pub tier: Tier,
    pub count: u32,
    /// When it first showed, which a click already under way then mustn't answer.
    pub appeared: std::time::Instant,
    /// How many of the task's messages came before it, which places it among them.
    pub after: usize,
    /// How many of the task's steps came before it, which places it among them.
    pub traced: usize,
    /// Acted on, or passed: a change of tier, Take over, or the task's end.
    pub settled: bool,
}

impl Stop {
    /// Whether it's the same refusal as one of `why` on `tab` at `at` and `tier`.
    pub fn same(&self, tab: u64, at: &str, why: Why, tier: Tier) -> bool {
        !self.settled && self.tab == tab && self.at == at && self.why == why && self.tier == tier
    }
}

/// The user's line's sentence on why ilo stopped an act at `tier` on `site`: a closed set of the
/// app's own sentences, keyed by `why`, with nothing of the agent's or a page's in it but `site`.
pub fn why_line(why: Why, tier: Tier, site: &str) -> String {
    match why {
        Why::Fenced => format!("{site} is on your Off limits list."),
        Why::OtherSite => format!("{site} isn't one of the sites you added."),
        Why::Never => format!("{site} is set to Never for this agent."),
        _ => {
            let row = ROWS.iter().find(|row| row.why == why).unwrap();
            row.refused.replace("{level}", tier.label())
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
        Why::Never => format!(
            "ilo stopped this: {what}. The user set {site} to Never for this agent, whatever a \
             task's access. Only the user can change that, in ilo. Don't try another way to the \
             same end."
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

/// What a task starts with, as its start's entry counts it: never an address, a title or a name.
#[derive(Clone, Copy, Debug, Default)]
pub struct StartedWith {
    /// The pages the user added.
    pub pages: usize,
    /// Whether they gave it a folder.
    pub folder: bool,
    /// How many files they attached.
    pub files: usize,
    /// Whether ilo couldn't copy those files, so it started without them.
    pub left_out: bool,
}

/// The activity log's words for a change of the sites a task may use, as `changed_logged`'s for
/// its level: the menu's names with how many sites, never an address.
pub fn sites_logged(from: &Sites, to: &Sites) -> String {
    let name = |sites: &Sites| match sites {
        Sites::Any => ANY_SITE.to_string(),
        Sites::Only(only) if only.len() == 1 => format!("{ONLY_ADDED} · 1 site"),
        Sites::Only(only) => format!("{ONLY_ADDED} · {} sites", only.len()),
    };
    format!("{} → {}", name(from), name(to))
}

/// The activity log's words for the tier a task starts at, which its start's entry keeps as its
/// target, as a change's keeps `changed_logged`, with what it starts with, in this order: pages,
/// folder, files.
pub fn started_logged(tier: Tier, with: StartedWith) -> String {
    let mut said = format!("Started at {}", tier.label());
    match with.pages {
        0 => {}
        1 => said.push_str(" · 1 page"),
        pages => said.push_str(&format!(" · {pages} pages")),
    }
    if with.folder {
        said.push_str(" · a folder");
    }
    match with.files {
        0 => {}
        1 => said.push_str(" · 1 file"),
        files => said.push_str(&format!(" · {files} files")),
    }
    if with.left_out {
        said.push_str(" · files left out");
    }
    said
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
pub fn capped(text: &str, most: usize) -> String {
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

    /// The start's entry counts what the task starts with, pages, then folder, then files, and
    /// names none of them.
    #[test]
    fn a_start_counts_its_pages_folder_and_files_in_that_order() {
        let with = |pages, folder, files, left_out| StartedWith {
            pages,
            folder,
            files,
            left_out,
        };
        for (with, said) in [
            (with(0, false, 0, false), "Started at Work on pages"),
            (
                with(1, true, 3, false),
                "Started at Work on pages · 1 page · a folder · 3 files",
            ),
            (
                with(2, false, 1, false),
                "Started at Work on pages · 2 pages · 1 file",
            ),
            (
                with(0, true, 2, true),
                "Started at Work on pages · a folder · 2 files · files left out",
            ),
        ] {
            assert_eq!(started_logged(Tier::Work, with), said, "{with:?}");
        }
    }

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
        assert_eq!(at(Never), [false, false, false]);
        assert_eq!(at(SignIn), [false, true, true]);
        assert_eq!(at(SignInBank), [false, false, true]);
        assert_eq!(Tier::default(), Work);
        assert_eq!(Look.lifting(Act), Some(Work));
        assert_eq!(Look.lifting(Message), Some(Full));
        assert_eq!(Work.lifting(Order), Some(Full));
        assert_eq!(Full.lifting(Order), None);
        assert_eq!(Look.lifting(Fenced), None);
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

    /// An ilo line's sentence is the app's own: one per reason, whole, naming the tier, with
    /// nothing in it from the agent or a page but the site it's handed.
    #[test]
    fn each_refusal_is_a_whole_sentence_of_the_apps() {
        let mut seen = std::collections::HashSet::new();
        for row in &ROWS {
            for tier in Tier::ALL {
                let line = why_line(row.why, tier, "shop.example");
                assert!(line.ends_with('.') && !line.contains('{'), "{line}");
                match row.why {
                    Why::Fenced | Why::OtherSite | Why::Never => {
                        assert!(line.starts_with("shop.example "))
                    }
                    _ => assert!(
                        line.ends_with(&format!(" isn't part of {}.", tier.label())),
                        "{line}"
                    ),
                }
            }
            assert!(
                seen.insert(why_line(row.why, Tier::Look, "")),
                "{:?}",
                row.why
            );
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
