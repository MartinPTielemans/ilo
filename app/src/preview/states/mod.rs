//! The preview's made-up states, one file per surface.

pub(crate) mod account;
pub(crate) mod library;
pub(crate) mod live_folder;
pub(crate) mod recent;
pub(crate) mod settings;
pub(crate) mod site_controls;
pub(crate) mod tasks;
pub(crate) mod update;

pub(crate) use {
    account::*, library::*, live_folder::*, recent::*, settings::*, site_controls::*, tasks::*,
    update::*,
};

/// A state the preview can show, set by an environment variable.
pub(crate) struct State {
    pub(crate) name: &'static str,
    /// What it takes; "1" for a switch, which any value turns on.
    #[expect(dead_code, reason = "read by people picking a state")]
    pub(crate) values: &'static str,
    /// What it shows.
    #[expect(dead_code, reason = "read by people picking a state")]
    pub(crate) shows: &'static str,
}

/// Every state the preview shows. A new one is a row here, its data in a file of this folder,
/// and its read through `var` or `var_os`, which take only a name that's a row.
#[rustfmt::skip]
pub(crate) const STATES: &[State] = &[
    State { name: "PREVIEW_ACCOUNT", values: "signed-out, no-server, words, waiting, signed-in", shows: "Settings › Account in that state" },
    State { name: "PREVIEW_AGENT", values: "1", shows: "Reading as Claude Code's Space, shown, with the agent at work" },
    State { name: "PREVIEW_APPEARANCE", values: "light, dark", shows: "the window as if the system's appearance were that" },
    State { name: "PREVIEW_APPEARANCE_PICKER", values: "automatic, light, dark", shows: "Settings › Appearance with that choice" },
    State { name: "PREVIEW_ARRANGEMENT", values: "nofit, changed", shows: "what Site Controls says of the Boost's arrangement" },
    State { name: "PREVIEW_ASSISTANTS", values: "start, signed-in, models", shows: "Settings › Agents' Your assistant, and Notifications with models" },
    State { name: "PREVIEW_BAR", values: "tabs, sites, commands, library, places", shows: "the command bar over the window, with those rows" },
    State { name: "PREVIEW_BOOST", values: "on, off", shows: "Site Controls' site with a Boost, which shows its row" },
    State { name: "PREVIEW_BOOST_CARD", values: "list, nofit, changed", shows: "the Boost card of the tab shown" },
    State { name: "PREVIEW_BOOST_EDITOR", values: "new, edited, advanced", shows: "the Boost editor for a site's Boost" },
    State { name: "PREVIEW_BROOM", values: "idle, sorting, failed, nothing", shows: "the broom before Clear on the New Tab line in that state: an idle one with the pointer over the sidebar (PREVIEW_HOVER), none with nothing to sort" },
    State { name: "PREVIEW_CHOOSER", values: "serial, hid, usb, bluetooth, stopped, none, off, denied", shows: "a page's device chooser" },
    State { name: "PREVIEW_CLICK", values: "<x>,<y>", shows: "a click there in the window" },
    State { name: "PREVIEW_COMMAND_LINE", values: "off, on, unsearched, flatpak-off, flatpak-on", shows: "Settings › Agents' apps that connect" },
    State { name: "PREVIEW_CONNECTION", values: "not-secure, internal", shows: "Site Controls over plain http, or the browser's own page" },
    State { name: "PREVIEW_DEVELOPER_MODE", values: "1", shows: "the tab shown, the third tab and a favorite in Developer Mode" },
    State { name: "PREVIEW_DIALOG", values: "1", shows: "the app's dialog over the window the first time it asks" },
    State { name: "PREVIEW_DRM", values: "1, default", shows: "a page asking for video ilo can't play, and its note" },
    State { name: "PREVIEW_EDGE_KNOT", values: "1", shows: "the sidebar hidden and the edge knot at its edge (with PREVIEW_TASKS=needs-you)" },
    State { name: "PREVIEW_EMPTY", values: "1, reading, private, new", shows: "no page, as a fresh ⌘N window, the second Space, a private window or a new user's Space" },
    State { name: "PREVIEW_FAVORITE", values: "<index>", shows: "that favorite selected" },
    State { name: "PREVIEW_FOCUS_SIDEBAR", values: "1", shows: "the sidebar with the keyboard, its ring on the tab shown's row" },
    State { name: "PREVIEW_FOOT_BAR", values: "arrange, point, long", shows: "the foot bar over the page's bottom centre" },
    State { name: "PREVIEW_HOVER", values: "<x>,<y>", shows: "the pointer resting there, and the tooltip it brings" },
    State { name: "PREVIEW_ICON_PICKER", values: "<emoji, or empty>", shows: "the icon picker for a Space with that icon" },
    State { name: "PREVIEW_LIBRARY", values: "1, downloads, spaces, boards, …", shows: "the Library at that section" },
    State { name: "PREVIEW_LIBRARY_ROW", values: "<n>", shows: "the Library's list with its nth row rung" },
    State { name: "PREVIEW_LIVE_FOLDER", values: "open, closed, signed-out, failed, states", shows: "a Live Folder of made-up pull requests, or that answer" },
    State { name: "PREVIEW_MENU", values: "<x>,<y>[,<key>…]", shows: "a right-click there, and the keys pressed in its menu" },
    State { name: "PREVIEW_MENU_LOOK", values: "light", shows: "menus as a light theme's Space draws them" },
    State { name: "PREVIEW_MINI_BAR", values: "selected, float, scrolls", shows: "Arrange's mini bar over a made-up thing on the page" },
    State { name: "PREVIEW_MODEL_MENU", values: "<claude|codex>[,<model>]", shows: "Settings › Agents' Model menu" },
    State { name: "PREVIEW_ONBOARDING", values: "1, light, dark", shows: "the onboarding over the welcome" },
    State { name: "PREVIEW_ONBOARDING_ASSISTANTS", values: "checking, start, signed-in", shows: "the onboarding's assistants' step with its rows in that state" },
    State { name: "PREVIEW_ONBOARDING_STEP", values: "1 to 7, 3b", shows: "the onboarding at that step" },
    State { name: "PREVIEW_PAGE", values: "<rrggbb>[,<tab>]", shows: "the page of the tab shown, or of that tab, shown, in that colour, which its line takes" },
    State { name: "PREVIEW_PAGE_MENU", values: "<link|image|text|field>,<x>,<y>[,<key>…]", shows: "a page's menu there" },
    State { name: "PREVIEW_PAIR", values: "pin, confirm, passkey", shows: "a Bluetooth device the first tab's page pairs with" },
    State { name: "PREVIEW_PEEK", values: "1", shows: "a link of the page shown in Peek" },
    State { name: "PREVIEW_PRESS", values: "<x>,<y>[,<ms>]", shows: "a press there, held that long" },
    State { name: "PREVIEW_PROFILES", values: "<name>,<name>…", shows: "those profiles, the Spaces' the first" },
    State { name: "PREVIEW_RECENT", values: "rows, none, draft, signedout, failed, stale, loading, on, off", shows: "a site's Recent card with that answer, or its switch" },
    State { name: "PREVIEW_SAFE_START", values: "1, unblocked", shows: "a safe start's card at the sidebar's foot" },
    State { name: "PREVIEW_SCROLL", values: "<points>", shows: "the account card scrolled that far" },
    State { name: "PREVIEW_SERVER", values: "<address>", shows: "the account signed in to a server of the user's" },
    State { name: "PREVIEW_SESSION", values: "working, done, open", shows: "an outside agent's card alone" },
    State { name: "PREVIEW_SETTINGS_GENERAL", values: "1", shows: "Settings › General" },
    State { name: "PREVIEW_SETTINGS_WIDTH", values: "<points>", shows: "Settings' pages in a narrower window" },
    State { name: "PREVIEW_SHARE_FAVORITES", values: "every, profile, space", shows: "Settings › Tabs & Sidebar's favorites sharing" },
    State { name: "PREVIEW_SIDEBAR", values: "<width>", shows: "the sidebar at that width" },
    State { name: "PREVIEW_SIDEBAR_POSITION", values: "left, right", shows: "Settings › Appearance's sidebar position" },
    State { name: "PREVIEW_SIDEBAR_RIGHT", values: "1", shows: "the sidebar on the window's right" },
    State { name: "PREVIEW_SITE_CONTROLS", values: "1", shows: "Site Controls of the tab shown, over a made-up site" },
    State { name: "PREVIEW_SITE_NOTES", values: "open, closed", shows: "Settings › Agents' Skills" },
    State { name: "PREVIEW_SIZE", values: "<width>x<height>", shows: "the window at that size" },
    State { name: "PREVIEW_SPACE_ICONS", values: "1", shows: "the Spaces with glyphs, for the footer's icons" },
    State { name: "PREVIEW_SPACES", values: "<count>", shows: "that many Spaces" },
    State { name: "PREVIEW_SWIPE", values: "1", shows: "the Library brought out and back by fingers, then clicks" },
    State { name: "PREVIEW_SYNC", values: "never, offline, server-error, refused, sign-in, throttled, stopped, update", shows: "the account's sync line" },
    State { name: "PREVIEW_TASK", values: "working, refused, off-limits, yours, tab-back, signed-in, raised, notes, helpers, helper, helpers-limit, helpers-ended, not-our-tools, ended, …", shows: "the working task's conversation beside its page, in that state" },
    State { name: "PREVIEW_TASK_RING", values: "none, header, back, page, take-over, stop", shows: "the working task's page watched, that stop rung" },
    State { name: "PREVIEW_TASK_STEP", values: "long", shows: "a step line too long for a narrow strip" },
    State { name: "PREVIEW_TASKS", values: "1, open, done, needs-you, stopped, interrupted, two", shows: "tasks' cards in Today: three, the working one's conversation, or one state's" },
    State { name: "PREVIEW_THEME", values: "#RRGGBB, or a theme as the core keeps it", shows: "the first Space with that theme" },
    State { name: "PREVIEW_THEME_PICKER", values: "1", shows: "the theme picker for the first Space" },
    State { name: "PREVIEW_TRANSLATE", values: "<language>", shows: "the made-up pages in that language, offering a translation" },
    State { name: "PREVIEW_TROUBLE", values: "crashed, hung, split", shows: "a page that died or stopped answering" },
    State { name: "PREVIEW_UPDATE", values: "ready, available, ready-open, available-open", shows: "the update banner on a made-up release" },
    State { name: "PREVIEW_UPDATE_CHECKS", values: "off", shows: "the update banner with automatic checks off" },
    State { name: "PREVIEW_WELCOME", values: "1", shows: "no page, with the first run's welcome" },
];

/// The value of `name`, a row of `STATES`, as `std::env::var` reads it.
pub(crate) fn var(name: &str) -> Result<String, std::env::VarError> {
    debug_assert!(listed(name), "{name} isn't a row of STATES");
    std::env::var(name)
}

/// The value of `name`, a row of `STATES`, as `std::env::var_os` reads it.
pub(crate) fn var_os(name: &str) -> Option<std::ffi::OsString> {
    debug_assert!(listed(name), "{name} isn't a row of STATES");
    std::env::var_os(name)
}

fn listed(name: &str) -> bool {
    STATES.iter().any(|state| state.name == name)
}

#[cfg(test)]
mod tests {
    use super::STATES;

    /// The preview's sources, which name the states they read.
    const SOURCES: &[&str] = &[
        include_str!("../../preview.rs"),
        include_str!("account.rs"),
        include_str!("library.rs"),
        include_str!("live_folder.rs"),
        include_str!("recent.rs"),
        include_str!("settings.rs"),
        include_str!("site_controls.rs"),
        include_str!("tasks.rs"),
        include_str!("update.rs"),
    ];

    /// Each state read is a row, each row is read, and each through the table, so the table is the
    /// list of what the preview shows.
    #[test]
    fn the_table_is_every_state_the_preview_reads() {
        let mut read = Vec::new();
        for source in SOURCES {
            assert!(
                !source.contains(concat!("std::env::var", "(\"PREVIEW_"))
                    && !source.contains(concat!("std::env::var_os", "(\"PREVIEW_")),
                "a state is read around the table"
            );
            for (at, _) in source
                .match_indices("var(\"PREVIEW_")
                .chain(source.match_indices("var_os(\"PREVIEW_"))
            {
                let name = &source[at..];
                let name = &name[name.find('"').unwrap() + 1..];
                read.push(name[..name.find('"').unwrap()].to_string());
            }
        }
        for name in &read {
            assert!(
                STATES.iter().any(|state| state.name == name),
                "{name} is read but isn't a row"
            );
        }
        for state in STATES {
            assert!(
                read.iter().any(|name| name == state.name),
                "{} is a row no one reads",
                state.name
            );
        }
        let mut names: Vec<_> = STATES.iter().map(|state| state.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), STATES.len(), "a state has two rows");
    }

    /// The working task's steps each name the page they acted on, the first going to it, so its
    /// log draws a page's line with its steps under it, and its done fold the page's icon, as
    /// the app's do.
    #[test]
    fn the_working_tasks_steps_are_on_its_page() {
        use browser::agents::task::{Line, TraceItem};
        let talk = super::tasks::task_conversation("claude".into());
        let Some(Line::Trace(trace)) = talk.lines.first() else {
            panic!("the working conversation starts with its trace");
        };
        let steps: Vec<_> = (trace.items.iter())
            .filter_map(|item| match item {
                TraceItem::Step(step) => Some(step),
                _ => None,
            })
            .collect();
        let pages: Vec<_> = steps.iter().map(|step| step.page.as_ref()).collect();
        assert!(
            pages.iter().all(Option::is_some),
            "a step has no page: {pages:?}"
        );
        let went: Vec<bool> = pages.iter().flatten().map(|page| page.went).collect();
        assert!(
            went[0] && !went[1..].contains(&true),
            "only the first goes to it: {went:?}"
        );
        for page in pages.iter().flatten().skip(1) {
            assert_eq!(
                page.tab, talk.on,
                "a step after the first is on the task's tab"
            );
        }
    }

    /// A done task's answer has its finish time under it, so its conversation draws the row with
    /// Copy and the time, as the app's does.
    #[test]
    fn a_done_tasks_answer_says_when_it_finished() {
        use browser::agents::task::{Line, Word};
        let cards = super::tasks::task_cards(0);
        let done: Vec<_> = cards
            .iter()
            .filter(|card| card.word == Word::Done)
            .collect();
        assert!(!done.is_empty(), "the preview has no done task");
        for card in done {
            let talk = super::tasks::task_conversation(card.agent.clone());
            let answer = (talk.lines.iter()).position(|line| matches!(line, Line::Said(_)));
            let Some(answer) = answer else {
                panic!("{}'s conversation has no answer", card.title);
            };
            let timed = talk.finished.iter().any(|(at, _)| *at == answer);
            assert!(timed, "{}'s answer has no finish time", card.title);
        }
    }

    /// The working task has a model to show, as a thread the user started does, so its composer's
    /// row of choices carries it beside the level: its CLI's mark and its name, as the app says it.
    #[test]
    fn the_working_task_has_a_model_for_its_row() {
        use browser::agents::runner::Cli;
        let talk = super::tasks::task_conversation("claude".into());
        let Some((mark, said, _, _)) = talk.model else {
            panic!("the preview's thread has no model, so its row shows the level alone");
        };
        assert_eq!(mark, Cli::Claude.glyph());
        assert!(said.starts_with(Cli::Claude.name()), "{said}");
    }
}
