//! The updater against a fake of Flatpak's portal, served on a D-Bus connection of its own, which
//! says what Flatpak's would and notes what it's asked.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use futures_lite::future::{block_on, zip};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedFd, OwnedObjectPath, OwnedValue, Value};

use super::*;

const MONITOR_PATH: &str = "/org/freedesktop/portal/Flatpak/update_monitor/test/1";

type Calls = Arc<Mutex<Vec<String>>>;

/// How the fake's monitor answers an Update.
#[derive(Clone, Copy)]
enum Answer {
    Installs,
    /// Fails, as with no network.
    Fails,
    /// Installs once the updater has stopped waiting, as when the user is slow to say yes.
    Late,
}

/// The answers to the Updates to come, in turn; Installs once they've run out.
type Answers = Arc<Mutex<VecDeque<Answer>>>;

/// How long the updater waits on an answer in the tests, and how long a late one takes.
const WAIT: Duration = Duration::from_millis(50);
const LATE: Duration = Duration::from_millis(300);

struct FakePortal {
    calls: Calls,
    answers: Answers,
}

#[zbus::interface(name = "org.freedesktop.portal.Flatpak")]
impl FakePortal {
    async fn create_update_monitor(
        &self,
        _options: HashMap<String, OwnedValue>,
        #[zbus(object_server)] server: &zbus::ObjectServer,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        self.calls.lock().unwrap().push("CreateUpdateMonitor".into());
        let monitor = FakeMonitor { calls: self.calls.clone(), answers: self.answers.clone() };
        server.at(MONITOR_PATH, monitor).await?;
        Ok(OwnedObjectPath::try_from(MONITOR_PATH).unwrap())
    }

    async fn spawn(
        &self,
        cwd: Vec<u8>,
        argv: Vec<Vec<u8>>,
        fds: HashMap<u32, OwnedFd>,
        envs: HashMap<String, String>,
        flags: u32,
        _options: HashMap<String, OwnedValue>,
    ) -> u32 {
        let text = |bytes: &[u8]| {
            assert_eq!(bytes.last(), Some(&0), "each byte string ends with a NUL");
            String::from_utf8_lossy(&bytes[..bytes.len() - 1]).into_owned()
        };
        let argv: Vec<String> = argv.iter().map(|arg| text(arg)).collect();
        let call = format!("Spawn cwd={:?} flags={flags} fds={} envs={} {argv:?}", text(&cwd), fds.len(), envs.len());
        self.calls.lock().unwrap().push(call);
        4242
    }
}

struct FakeMonitor {
    calls: Calls,
    answers: Answers,
}

#[zbus::interface(name = "org.freedesktop.portal.Flatpak.UpdateMonitor")]
impl FakeMonitor {
    /// Installs at once, as Flatpak's does over a while, telling how it went.
    async fn update(
        &self,
        parent_window: String,
        _options: HashMap<String, OwnedValue>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        self.calls.lock().unwrap().push(format!("Update {parent_window:?}"));
        let answer = self.answers.lock().unwrap().pop_front().unwrap_or(Answer::Installs);
        if let Answer::Late = answer {
            async_io::Timer::after(LATE).await;
        }
        let status = |status: u32| HashMap::from([("status", Value::from(status))]);
        Self::progress(&emitter, status(RUNNING)).await?;
        if let Answer::Fails = answer {
            let mut info = status(ERROR);
            info.insert("error_message", Value::from("Couldn't reach Flathub"));
            Self::progress(&emitter, info).await?;
        } else {
            Self::progress(&emitter, status(DONE)).await?;
        }
        Ok(())
    }

    #[zbus(signal)]
    async fn update_available(
        emitter: &SignalEmitter<'_>,
        info: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn progress(emitter: &SignalEmitter<'_>, info: HashMap<&str, Value<'_>>) -> zbus::Result<()>;
}

/// The app's connection, and the fake's, on either end of a socket.
async fn connect(answers: &[Answer], calls: Calls) -> (zbus::Connection, zbus::Connection) {
    let (ours, theirs) = std::os::unix::net::UnixStream::pair().unwrap();
    let answers = Arc::new(Mutex::new(answers.iter().copied().collect()));
    let portal = FakePortal { calls, answers };
    let server = zbus::connection::Builder::unix_stream(theirs)
        .server(zbus::Guid::generate())
        .unwrap()
        .p2p()
        .serve_at(PATH, portal)
        .unwrap()
        .build();
    let client = zbus::connection::Builder::unix_stream(ours).p2p().build();
    let (server, client) = zip(server, client).await;
    (client.unwrap(), server.unwrap())
}

/// Says the fake's monitor found `local` installed and `remote` on Flathub, against `running`.
async fn announce(fake: &zbus::Connection, running: &str, local: &str, remote: &str) {
    let monitor = fake.object_server().interface::<_, FakeMonitor>(MONITOR_PATH).await.unwrap();
    let info = HashMap::from([
        ("running-commit", Value::from(running)),
        ("local-commit", Value::from(local)),
        ("remote-commit", Value::from(remote)),
    ]);
    FakeMonitor::update_available(monitor.signal_emitter(), info).await.unwrap();
}

/// The states the updater went through after `announced`, with each Update answered as `answers`
/// say and, if `retry`, Try Again chosen after the first failure, until it has gone through
/// `until` or the fake is done.
fn states_after(
    answers: &[Answer],
    announced: [&str; 3],
    retry: bool,
    until: usize,
) -> (Vec<State>, Vec<String>) {
    let calls = Calls::default();
    block_on(async {
        let (ours, fake) = connect(answers, calls.clone()).await;
        let (portal, events) = Portal::open(ours).await.unwrap();
        let states = Arc::new(Mutex::new(Vec::new()));
        let (retrying, retries) = futures::channel::mpsc::unbounded();
        let watching = {
            let states = states.clone();
            async move {
                let changed = |state: &State| states.lock().unwrap().push(state.clone());
                watch(&portal, events, retries, WAIT, changed).await
            }
        };
        let [running, local, remote] = announced;
        let announcing = async {
            announce(&fake, running, local, remote).await;
            // Turns for the updater's Updates and the fake's Progress, then the portal goes.
            let mut retry = retry;
            for _ in 0..400 {
                let states = states.lock().unwrap().clone();
                if states.len() >= until {
                    break;
                }
                if retry && matches!(states.last(), Some(State::Failed(_))) {
                    retrying.unbounded_send(()).unwrap();
                    retry = false;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
                futures_lite::future::yield_now().await;
            }
            drop(fake);
        };
        zip(watching, announcing).await;
        let states = states.lock().unwrap().clone();
        (states, calls.lock().unwrap().clone())
    })
}

#[test]
fn a_build_on_flathub_installs_and_is_ready_for_a_restart() {
    let (states, calls) = states_after(&[], ["a1", "a1", "b2"], false, 2);
    assert_eq!(states, [State::Installing, State::Ready]);
    assert_eq!(calls, ["CreateUpdateMonitor", r#"Update """#]);
}

#[test]
fn a_build_the_software_centre_installed_needs_only_a_restart() {
    let (states, calls) = states_after(&[], ["a1", "b2", "b2"], false, 1);
    assert_eq!(states, [State::Ready]);
    assert_eq!(calls, ["CreateUpdateMonitor"], "nothing to install");
}

#[test]
fn a_failed_install_says_why() {
    let (states, _) = states_after(&[Answer::Fails], ["a1", "a1", "b2"], false, 2);
    assert_eq!(states, [State::Installing, State::Failed("Couldn't reach Flathub".into())]);
}

// A desktop with nothing to ask whether ilo may update itself never answers: the wait runs out
// and says so, and a yes that comes after still installs.
#[test]
fn an_unanswered_install_says_so_and_a_late_yes_still_installs() {
    let (states, calls) = states_after(&[Answer::Late], ["a1", "a1", "b2"], false, 4);
    let unanswered = State::Failed(UNANSWERED.into());
    assert_eq!(states, [State::Installing, unanswered, State::Installing, State::Ready]);
    assert_eq!(calls, ["CreateUpdateMonitor", r#"Update """#]);
}

#[test]
fn try_again_installs_anew_after_a_failure() {
    let (states, calls) = states_after(&[Answer::Fails], ["a1", "a1", "b2"], true, 4);
    let failed = State::Failed("Couldn't reach Flathub".into());
    assert_eq!(states, [State::Installing, failed, State::Installing, State::Ready]);
    assert_eq!(calls, ["CreateUpdateMonitor", r#"Update """#, r#"Update """#]);
}

#[test]
fn a_restart_spawns_the_flatpaks_command_in_the_newest_build_once_the_profile_is_free() {
    let calls = Calls::default();
    block_on(async {
        let (ours, _fake) = connect(&[], calls.clone()).await;
        let (portal, _) = Portal::open(ours).await.unwrap();
        portal.restart(Path::new("/home/u/.var/app/ilo/SingletonLock")).await.unwrap();
    });
    let calls = calls.lock().unwrap();
    let spawn = calls.iter().find(|call| call.starts_with("Spawn")).expect("no Spawn");
    assert!(spawn.starts_with(r#"Spawn cwd="" flags=2 fds=0 envs=0 ["/bin/sh", "-c", "#), "{spawn}");
    // "ilo", through cobalt and zypak, never /app/ilo/chrome, which would start without the
    // sandbox zypak gives it.
    assert!(spawn.ends_with(r#""sh", "/home/u/.var/app/ilo/SingletonLock", "ilo"]"#), "{spawn}");
}

#[test]
fn the_wait_for_the_profile_runs_the_build_with_its_arguments() {
    // The script Spawn runs, here with sh, a lock that goes and echo for the browser.
    let dir = std::env::temp_dir().join(format!("ilo-restart-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let lock = dir.join("SingletonLock");
    _ = std::fs::remove_file(&lock);
    std::os::unix::fs::symlink("host-123", &lock).unwrap();
    let remover = {
        let lock = lock.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            std::fs::remove_file(lock).unwrap();
        })
    };
    let started = std::time::Instant::now();
    let out = std::process::Command::new("/bin/sh")
        .args(["-c", RESTART, "sh", lock.to_str().unwrap(), "echo", "started", "a b"])
        .output()
        .unwrap();
    remover.join().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "started a b\n");
    assert!(started.elapsed() >= std::time::Duration::from_millis(300), "it waited for the lock");
    std::fs::remove_dir_all(dir).unwrap();
}

/// The dialog's window, which shows nothing in a test: what it asks is read through its buttons.
struct DialogWindow;

impl crate::dialog::Host for DialogWindow {
    fn show(&self, _: Option<u64>, _: Option<gpui::Bounds<gpui::Pixels>>) {}
    fn hide(&self) {}
}

/// The dialog's window, and how many times Restart restarted.
fn updating(cx: &mut gpui::TestAppContext) -> std::rc::Rc<std::cell::Cell<usize>> {
    cx.update(|cx| crate::dialog::open(std::rc::Rc::new(DialogWindow), cx)).expect("no dialog");
    let restarts = std::rc::Rc::new(std::cell::Cell::new(0));
    let counted = restarts.clone();
    cx.update(|cx| on_restart(move |_| counted.set(counted.get() + 1), cx));
    restarts
}

/// Chooses the dialog's button `label`, and lets the dialog go, after which its action runs.
fn choose(label: &str, cx: &mut gpui::TestAppContext) -> bool {
    let chosen = cx.update(|cx| crate::dialog::choose(label, cx));
    cx.executor().advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    chosen
}

fn asking(cx: &mut gpui::TestAppContext) -> bool {
    cx.run_until_parked();
    cx.update(|cx| crate::dialog::asking(cx))
}

// Installing asks nothing, and nor does the build being in: Restart to Update at the sidebar's
// foot says it's ready, as on the Mac and Windows, and its restart starts it.
#[gpui::test]
fn a_build_installed_asks_nothing_and_restarts_when_told(cx: &mut gpui::TestAppContext) {
    let restarts = updating(cx);
    cx.update(|cx| changed(State::Installing, cx));
    assert!(!asking(cx), "nothing to restart into yet");
    cx.update(|cx| changed(State::Ready, cx));
    assert!(!asking(cx), "a dialog for a build that's in");
    assert!(cx.update(|cx| crate::updates::ready(cx)), "nothing says it's ready");
    cx.update(restart);
    assert_eq!(restarts.get(), 1);
}

// Check for Updates… called once a build is in, as the app calls Restart to Update in its place,
// still offers the restart.
#[gpui::test]
fn check_for_updates_offers_the_restart_once_a_build_is_in(cx: &mut gpui::TestAppContext) {
    let restarts = updating(cx);
    cx.update(|cx| changed(State::Ready, cx));
    cx.update(check);
    assert!(asking(cx), "Check for Updates… didn't offer the restart");
    assert!(choose("Later", cx));
    assert_eq!(restarts.get(), 0);
    cx.update(check);
    assert!(choose("Restart", cx));
    assert_eq!(restarts.get(), 1);
}

// Short of a build that's in, Check for Updates… only tells, with OK, which restarts nothing.
#[gpui::test]
fn check_for_updates_tells_where_the_update_stands(cx: &mut gpui::TestAppContext) {
    let restarts = updating(cx);
    for state in [State::Current, State::Installing, State::Failed("No network".into())] {
        cx.update(|cx| changed(state.clone(), cx));
        assert!(!asking(cx), "{state:?} asked unprompted");
        cx.update(check);
        assert!(asking(cx), "Check for Updates… said nothing at {state:?}");
        assert!(!choose("Restart", cx));
        assert!(choose("OK", cx));
    }
    assert_eq!(restarts.get(), 0);
}

// Try Again asks the portal again; Later leaves the failure, which Check for Updates… still tells.
#[gpui::test]
fn check_for_updates_tries_again_after_a_failure(cx: &mut gpui::TestAppContext) {
    let restarts = updating(cx);
    let retries = std::rc::Rc::new(std::cell::Cell::new(0));
    let counted = retries.clone();
    cx.update(|cx| on_retry(move |_| counted.set(counted.get() + 1), cx));
    cx.update(|cx| changed(State::Failed(UNANSWERED.into()), cx));
    assert!(!asking(cx), "a failure asked unprompted");
    cx.update(check);
    assert!(choose("Later", cx));
    assert_eq!(retries.get(), 0);
    cx.update(check);
    assert!(choose("Try Again", cx));
    assert_eq!((retries.get(), restarts.get()), (1, 0));
}

#[test]
fn what_check_for_updates_says() {
    let said = |state: Option<&State>| {
        let question = question(state, false);
        let words = [question.message, question.detail, question.action, question.cancel];
        words.map(|word| word.to_string())
    };
    let name = env!("APP_NAME");
    let looking = format!("{name} looks for one every half hour while it’s open.");
    assert_eq!(said(None), ["No update yet", &looking, "", "OK"]);
    assert_eq!(said(Some(&State::Current)), said(None));
    let installing = "Restart to Update shows in the sidebar once the new version is in.";
    let updating = format!("{name} is updating");
    assert_eq!(said(Some(&State::Installing)), [&updating, installing, "", "OK"]);
    // A portal that never answered says so, rather than that it's looking.
    let failed = State::Failed("Flatpak’s portal didn’t answer: no bus".into());
    assert_eq!(
        said(Some(&failed)),
        [&format!("{name} couldn’t update"), "Flatpak’s portal didn’t answer: no bus", "", "OK"]
    );
    // With the portal to ask, a failure offers another try.
    let retrying = question(Some(&failed), true);
    assert_eq!([&*retrying.action, &*retrying.cancel], ["Try Again", "Later"]);
    let ready = format!("A new version of {name} is ready");
    let restart = format!("Restart {name} to use it.");
    assert_eq!(said(Some(&State::Ready)), [&ready, &restart, "Restart", "Later"]);
}
