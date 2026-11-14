//! Which ad blocker a launch runs: uBlock Origin, the extension, or the browser's own (#2111). The
//! core reads it once, from `--ad-block-engine` (core/native_ad_block.cc), so a choice in Settings
//! takes effect at the next launch. The user's choice is one word in a file in the app's folder,
//! read as Chromium starts, never polled, and never sent anywhere. Neither engine deletes the
//! other's data, so going either way loses nothing.
//!
//! Settings › Ad blocking offers the choice only in a build whose default is the browser's own, so
//! no user is offered an engine before it has passed #2111's gate.

use std::rc::Rc;

use gpui::{App, SharedString};

/// The core's switch, which the app gives at every launch.
pub const SWITCH: &str = "--ad-block-engine";
/// Given beside the app's own switch. A restart starts the browser again with the switches it was
/// given, the app's among them, and the app's must not win over a choice made since, as a switch
/// a check gives does.
pub const MARK: &str = "--ad-block-engine-by-app";
/// A check's default for the launch, in place of the build's: not read in a prod build.
pub const DEFAULT_SWITCH: &str = "--ad-block-default";
/// The user's choice, in the app's folder.
pub const FILE: &str = "Ad Blocker";
/// What a launch runs when the user hasn't chosen.
pub const DEFAULT: Engine = Engine::UBlockOrigin;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    UBlockOrigin,
    /// The browser's own, adblock-rust on the same lists.
    Native,
}

impl Engine {
    /// The switch's and the file's word.
    pub fn word(self) -> &'static str {
        match self {
            Engine::UBlockOrigin => "ublock",
            Engine::Native => "native",
        }
    }

    /// The engine `text` names, None for anything else, as a word a newer build wrote.
    pub fn read(text: &str) -> Option<Engine> {
        match text.trim() {
            "ublock" => Some(Engine::UBlockOrigin),
            "native" => Some(Engine::Native),
            _ => None,
        }
    }
}

/// The last value of `switch` among the arguments `given`, as Chromium keeps a repeated switch's.
fn value<'a>(given: &'a [String], switch: &str) -> Option<&'a str> {
    let prefix = format!("{switch}=");
    (given.iter().rev()).find_map(|arg| arg.strip_prefix(prefix.as_str()))
}

/// What a launch runs, decided as Chromium starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Launch {
    pub engine: Engine,
    /// What it runs when the user hasn't chosen.
    pub default: Engine,
    /// The app decided, from the user's choice or the default. Otherwise a switch given decided,
    /// and the user's choice waits until a launch without it.
    pub by_app: bool,
}

impl Launch {
    /// From the arguments the process was `given` and the user's `chosen`: a switch given wins, as
    /// a check's does, unless it's the app's own from before a restart. Then the choice, then the
    /// default. The core runs its own only for "native", so any other word given is uBlock Origin.
    pub fn new(given: &[String], chosen: Option<Engine>, prod: bool) -> Launch {
        let default = (!prod)
            .then(|| value(given, DEFAULT_SWITCH).and_then(Engine::read))
            .flatten()
            .unwrap_or(DEFAULT);
        let restarted = given.iter().any(|arg| arg == MARK);
        match value(given, SWITCH).filter(|_| !restarted) {
            Some(word) => Launch {
                engine: Engine::read(word).unwrap_or(Engine::UBlockOrigin),
                default,
                by_app: false,
            },
            None => Launch {
                engine: chosen.unwrap_or(default),
                default,
                by_app: true,
            },
        }
    }

    /// Whether the app passes `arg`, one it was given, on to Chromium: not its own switch and mark
    /// from the launch before a restart, which it gives again as it decides now.
    pub fn passes(&self, arg: &str) -> bool {
        let switch = arg
            .strip_prefix(SWITCH)
            .is_some_and(|rest| rest.starts_with('='));
        arg != MARK && !(self.by_app && switch)
    }

    /// The switches the app adds.
    pub fn switches(&self) -> Vec<String> {
        match self.by_app {
            true => vec![format!("{SWITCH}={}", self.engine.word()), MARK.into()],
            false => Vec::new(),
        }
    }

    /// Whether Settings offers the choice: once the default is the browser's own.
    pub fn offered(&self) -> bool {
        self.default == Engine::Native
    }
}

/// What the row keeps the choice with.
pub trait Host {
    /// Keeps `engine` for the next launch.
    fn save(&self, engine: Engine);
    /// Quits and starts again, which runs the engine saved.
    fn restart(&self, cx: &mut App);
}

/// The restart, in the words Widevine's row has for it.
pub const RESTART: &str = crate::widevine::RESTART;

/// Settings › Ad blocking's row: a switch for uBlock Origin, and the restart a change waits for.
pub struct EngineRow {
    host: Rc<dyn Host>,
    launch: Launch,
    /// What the next launch runs.
    chosen: Engine,
}

impl EngineRow {
    pub fn new(host: Rc<dyn Host>, launch: Launch) -> Self {
        let chosen = launch.engine;
        Self {
            host,
            launch,
            chosen,
        }
    }

    pub fn shown(&self) -> bool {
        self.launch.offered()
    }

    pub fn title(&self) -> SharedString {
        "Block with uBlock Origin".into()
    }

    /// The switch: on, the next launch blocks with uBlock Origin.
    pub fn on(&self) -> bool {
        self.chosen == Engine::UBlockOrigin
    }

    /// What going one way or the other means, and once the user has chosen, when it happens.
    pub fn detail(&self) -> SharedString {
        let detail = match (self.chosen == self.launch.engine, self.chosen) {
            (true, Engine::Native) => concat!(
                "The extension ",
                env!("APP_NAME"),
                " blocked with before. It blocks again with these lists and My filters, ones you \
                 added since included."
            ),
            (true, Engine::UBlockOrigin) => concat!(
                "Turned off, ",
                env!("APP_NAME"),
                "'s own blocker blocks again, with these lists and My filters."
            ),
            (false, Engine::UBlockOrigin) => {
                concat!("uBlock Origin blocks once ", env!("APP_NAME"), " restarts.")
            }
            (false, Engine::Native) => {
                concat!(env!("APP_NAME"), "'s own blocker blocks once it restarts.")
            }
        };
        detail.into()
    }

    /// The restart, once the next launch runs another engine than this one.
    pub fn restart_offered(&self) -> bool {
        self.chosen != self.launch.engine
    }

    /// The switch's click: keeps the choice for the next launch.
    pub fn set(&mut self, on: bool) {
        self.chosen = match on {
            true => Engine::UBlockOrigin,
            false => Engine::Native,
        };
        self.host.save(self.chosen);
    }

    /// The restart's click, if the row still offers it: a click can come after the switch went back.
    pub fn restart(&self, cx: &mut App) {
        if self.restart_offered() {
            self.host.restart(cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{App, TestAppContext};

    use super::{DEFAULT, Engine, EngineRow, Host, Launch, MARK};

    fn args(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn the_users_choice_decides_unless_a_switch_was_given() {
        let launch = |given: &[&str], chosen| Launch::new(&args(given), chosen, false).engine;
        assert_eq!(
            launch(&[], None),
            DEFAULT,
            "no choice runs the build's default"
        );
        assert_eq!(launch(&[], Some(Engine::Native)), Engine::Native);
        assert_eq!(
            launch(&[], Some(Engine::UBlockOrigin)),
            Engine::UBlockOrigin
        );
        let check = ["--ad-block-engine=native"];
        assert_eq!(launch(&check, Some(Engine::UBlockOrigin)), Engine::Native);
        let twice = ["--ad-block-engine=native", "--ad-block-engine=ublock"];
        assert_eq!(
            launch(&twice, None),
            Engine::UBlockOrigin,
            "the last wins, as Chromium's"
        );
        let other = ["--ad-block-engine=brave"];
        assert_eq!(
            launch(&other, Some(Engine::Native)),
            Engine::UBlockOrigin,
            "as the core reads it"
        );
        let default = ["--ad-block-default=native"];
        assert_eq!(launch(&default, None), Engine::Native, "a check's default");
        assert_eq!(
            launch(&default, Some(Engine::UBlockOrigin)),
            Engine::UBlockOrigin
        );
        let prod = Launch::new(&args(&default), None, true);
        assert_eq!(
            prod.default, DEFAULT,
            "a prod build reads no check's default"
        );
    }

    #[test]
    fn a_restart_decides_afresh_but_keeps_a_switch_a_check_gave() {
        let first = Launch::new(&[], Some(Engine::Native), false);
        assert_eq!(first.switches(), args(&["--ad-block-engine=native", MARK]));
        // The user chose uBlock Origin since, and the restart gives the app's switches back.
        let restarted = args(&["--ad-block-engine=native", MARK, "--restore-last-session"]);
        let launch = Launch::new(&restarted, Some(Engine::UBlockOrigin), false);
        assert_eq!(launch.engine, Engine::UBlockOrigin);
        let passed: Vec<&String> = restarted.iter().filter(|arg| launch.passes(arg)).collect();
        assert_eq!(
            passed,
            vec!["--restore-last-session"],
            "the old switch and mark go"
        );
        assert_eq!(launch.switches(), args(&["--ad-block-engine=ublock", MARK]));

        let check = args(&["--ad-block-engine=native"]);
        let launch = Launch::new(&check, Some(Engine::UBlockOrigin), false);
        assert!(
            launch.switches().is_empty(),
            "the check's switch goes on as it was given"
        );
        assert!(check.iter().all(|arg| launch.passes(arg)));
        let restarted = args(&["--ad-block-engine=native", "--restore-last-session"]);
        let launch = Launch::new(&restarted, Some(Engine::UBlockOrigin), false);
        assert_eq!(
            launch.engine,
            Engine::Native,
            "after a restart, the check's still wins"
        );
        assert!(launch.passes("--ad-block-engines=native"));
    }

    #[test]
    fn the_file_holds_one_word_and_a_word_it_doesnt_know_is_no_choice() {
        for engine in [Engine::UBlockOrigin, Engine::Native] {
            assert_eq!(Engine::read(engine.word()), Some(engine));
            assert_eq!(Engine::read(&format!("{}\n", engine.word())), Some(engine));
        }
        assert_eq!(Engine::read("brave"), None);
        assert_eq!(Engine::read(""), None);
    }

    /// The app's folder and the restart, recorded.
    #[derive(Default)]
    struct Recorded {
        asked: RefCell<Vec<String>>,
    }

    impl Host for Recorded {
        fn save(&self, engine: Engine) {
            self.asked
                .borrow_mut()
                .push(format!("save {}", engine.word()));
        }

        fn restart(&self, _: &mut App) {
            self.asked.borrow_mut().push("restart".into());
        }
    }

    fn row(given: &[&str]) -> (Rc<Recorded>, EngineRow) {
        let host = Rc::new(Recorded::default());
        let launch = Launch::new(&args(given), None, false);
        (host.clone(), EngineRow::new(host, launch))
    }

    #[test]
    fn the_row_shows_only_once_the_default_is_the_browsers_own() {
        assert!(
            !row(&[]).1.shown(),
            "before the flip no user is offered an engine"
        );
        assert!(
            !row(&["--ad-block-engine=native"]).1.shown(),
            "nor with a check's switch alone"
        );
        assert!(row(&["--ad-block-default=native"]).1.shown());
    }

    #[gpui::test]
    fn the_row_goes_back_to_the_extension_and_forward_again_each_with_its_restart(
        cx: &mut TestAppContext,
    ) {
        let (host, mut row) = row(&["--ad-block-default=native"]);
        assert!(
            !row.on() && !row.restart_offered(),
            "the browser's own blocks, nothing to restart"
        );
        cx.update(|cx| row.restart(cx));
        assert!(
            host.asked.borrow().is_empty(),
            "no restart is offered, so none is asked"
        );

        row.set(true);
        assert!(row.on() && row.restart_offered());
        assert!(row.detail().contains("uBlock Origin blocks once"));
        cx.update(|cx| row.restart(cx));
        assert_eq!(*host.asked.borrow(), ["save ublock", "restart"]);

        // Forward again from the same row, before the restart, and the restart goes.
        row.set(false);
        assert!(
            !row.restart_offered(),
            "the next launch runs what this one does"
        );
        cx.update(|cx| row.restart(cx));
        assert_eq!(
            *host.asked.borrow(),
            ["save ublock", "restart", "save native"]
        );

        // After the restart, uBlock Origin blocks, and the same row goes forward.
        let host = Rc::new(Recorded::default());
        let given = args(&["--ad-block-default=native"]);
        let launch = Launch::new(&given, Some(Engine::UBlockOrigin), false);
        let mut row = EngineRow::new(host.clone(), launch);
        assert!(row.shown() && row.on() && !row.restart_offered());
        row.set(false);
        assert!(row.detail().contains("own blocker blocks once it restarts"));
        cx.update(|cx| row.restart(cx));
        assert_eq!(*host.asked.borrow(), ["save native", "restart"]);
    }
}
