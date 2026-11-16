//! Updates: Sparkle on the Mac, ilo's own on Windows and Flatpak's portal on Linux, the track the
//! user follows, and the release notes an update's first launch opens.

use super::*;

/// The release notes of the update this launch is the first of, where the Mac's main hasn't kept
/// them before Chromium starts: taken with the first window, so a second launch, which hands its
/// links to the browser running and exits, never takes them.
pub(crate) fn first_launch_notes() -> Option<String> {
    #[cfg(target_os = "windows")]
    unsafe {
        platform::host_take_update_notes(keep_update_notes)
    };
    // On Linux, where an update can come from outside ilo, a version newer than the profile's
    // last. Only a release's, the Flatpak: a dev build's moves on under the checks' profiles.
    #[cfg(target_os = "linux")]
    if cfg!(feature = "prod") {
        let file = std::path::PathBuf::from(profile_dir()).join("last-version");
        return updates::first_launch_notes(&file, browser::build_info::VERSION);
    }
    UPDATE_NOTES.take()
}

#[cfg(not(target_os = "linux"))]
pub(crate) extern "C" fn keep_update_notes(url: *const c_char) {
    let url = unsafe { CStr::from_ptr(url) }
        .to_string_lossy()
        .into_owned();
    UPDATE_NOTES.set(Some(url));
}

/// Whether this build updates itself, asked once, as the menu bar is rebuilt often: a release
/// build with its updater, or on Linux the Flatpak.
pub(crate) fn updates_available() -> bool {
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    #[cfg(target_os = "linux")]
    let available = flatpak::in_flatpak;
    #[cfg(not(target_os = "linux"))]
    let available = || unsafe { platform::host_updates_available() };
    *AVAILABLE.get_or_init(available)
}

/// Starts checking for updates: Sparkle on the Mac, ilo's own on Windows, Flatpak's portal on
/// Linux. Settings › About's row, the update banner at the sidebar's foot and Check for Updates…'s
/// title follow what it says.
pub(crate) fn start_updates(cx: &mut App) {
    let updater = if updates_available() {
        updates::Updater::Own
    } else if updated_by_store() {
        updates::Updater::Store
    } else if updated_by_script() {
        // Read by tools/install-check.sh, as the native install's sign it knows what updates it.
        log!("browser: updated by install.sh");
        updates::Updater::Script
    } else {
        updates::Updater::None
    };
    updates::start(updater, Rc::new(SystemUpdater), cx);
    #[cfg(not(target_os = "linux"))]
    updates::checks(Some(automatic_checks()), cx);
    updates::observe(cx, |cx| {
        let ready = updates::ready(cx);
        if MENUS.with_borrow(|menus| menus.update_ready) != ready {
            MENUS.with_borrow_mut(|menus| menus.update_ready = ready);
            show_menus(cx);
        }
    })
    .detach();
    #[cfg(not(target_os = "linux"))]
    {
        // A nightly, or a release, and where the build carries a nightly feed, the builds the user
        // chose, else this one's: the updater reads that feed from its first check.
        let built = match unsafe { platform::host_nightly_build() } {
            true => updates::Track::Nightly,
            false => updates::Track::Release,
        };
        let own = updater == updates::Updater::Own;
        let followed = (own && unsafe { platform::host_nightly_feed() })
            .then(|| followed_track().unwrap_or(built));
        updates::tracks(built, followed, cx);
        unsafe { platform::host_follow_nightly(followed == Some(updates::Track::Nightly)) };
        // Restart to Update, whose update installs once the restart goes ahead (quit_going_ahead).
        extern "C" fn restart() {
            restart_because("the updater's install");
        }
        // An installer that failed after the last quit left this launch the old build (#1459).
        #[cfg(target_os = "windows")]
        {
            extern "C" fn installed(build: u64, own: u64) -> bool {
                let went_in = updates::went_in(build, own);
                if !went_in {
                    update_app_later(updates::didnt_install);
                }
                went_in
            }
            unsafe { platform::host_take_install(installed) };
        }
        // As the user left it, before the updater's first check could ask the feed anything.
        unsafe { platform::host_set_automatic_update_checks(automatic_checks()) };
        unsafe { platform::host_start_updater(restart, update_changed) };
    }
    #[cfg(target_os = "linux")]
    if updates_available() {
        cx.spawn(async move |cx| {
            let (portal, events) = match flatpak::Portal::open_session().await {
                Ok(opened) => opened,
                Err(error) => {
                    log!("browser: no updates: {error}");
                    let why = format!("Flatpak’s portal didn’t answer: {error}");
                    return cx.update(|cx| flatpak::changed(flatpak::State::Failed(why), cx));
                }
            };
            log!("browser: updater started");
            let relaunching = portal.clone();
            let retries = cx.update(|cx| {
                flatpak::on_restart(|_| restart_because("the restart into the newest build"), cx);
                flatpak::on_relaunch(move || relaunch_into_newest(&relaunching), cx);
                flatpak::changed(flatpak::State::Current, cx);
                flatpak::retries(cx)
            });
            flatpak::watch(&portal, events, retries, flatpak::ANSWER, |state| {
                log!("browser: update {state:?}");
                cx.update(|cx| flatpak::changed(state.clone(), cx));
            })
            .await;
        })
        .detach();
    }
}

/// Where Settings › About's "Check for updates automatically" is kept while it's off: the app's,
/// not a profile's, as its updates are. On without it.
#[cfg(not(target_os = "linux"))]
pub(crate) fn automatic_checks_path() -> String {
    format!("{}/no-automatic-update-checks", profile_dir())
}

/// Whether the updater checks on its schedule, which the user can turn off (#1793).
#[cfg(not(target_os = "linux"))]
pub(crate) fn automatic_checks() -> bool {
    !std::path::Path::new(&automatic_checks_path()).exists()
}

/// Turns the updater's own checks on or off, now and for the launches after.
#[cfg(not(target_os = "linux"))]
pub(crate) fn set_automatic_checks(on: bool) {
    let path = automatic_checks_path();
    _ = match on {
        true => std::fs::remove_file(path),
        false => std::fs::write(path, ""),
    };
    unsafe { platform::host_set_automatic_update_checks(on) };
}

/// Where the builds the user chose in Settings › About are kept: "nightly" or "release", for the
/// app, not a profile, as its updates are. Without it, the updater takes this build's own.
#[cfg(not(target_os = "linux"))]
pub(crate) fn followed_track_path() -> String {
    format!("{}/update-track", profile_dir())
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn followed_track() -> Option<updates::Track> {
    match std::fs::read_to_string(followed_track_path()).ok()?.trim() {
        "nightly" => Some(updates::Track::Nightly),
        "release" => Some(updates::Track::Release),
        _ => None,
    }
}

/// The app menu's and the command bar's Check for Updates…: Sparkle's on the Mac; on Windows, whose
/// updater has no window, the check, with Settings › About saying where it stands; on Linux, where
/// the Flatpak's update stands. Once an update is ready, it's Restart to Update.
pub(crate) fn check_for_updates(cx: &mut App) {
    if updates::ready(cx) {
        return updates::act(updates::Action::Restart, cx);
    }
    #[cfg(target_os = "windows")]
    settings::show_about(cx);
    #[cfg(not(target_os = "linux"))]
    unsafe {
        platform::host_check_for_updates()
    };
    #[cfg(target_os = "linux")]
    flatpak::check(cx);
}

/// Whether the Microsoft Store updates this copy, an MSIX from it.
pub(crate) fn updated_by_store() -> bool {
    #[cfg(not(target_os = "linux"))]
    return unsafe { platform::host_updated_by_store() };
    #[cfg(target_os = "linux")]
    false
}

/// Whether install.sh made this copy, the native Linux install, which keeps the script beside
/// chrome (tools/linux-tarball.sh) and is updated by running its line again.
fn updated_by_script() -> bool {
    cfg!(target_os = "linux")
        && std::env::current_exe().is_ok_and(|exe| exe.with_file_name("install.sh").is_file())
}

/// The system's updater, as Settings › About's row asks it.
pub(crate) struct SystemUpdater;

impl updates::Host for SystemUpdater {
    /// On the Mac, Sparkle's check in the background, whose download the row follows and which
    /// shows no window of its own; on Windows, the updater's, which has none; on Linux, where the
    /// Flatpak's update stands, or after a failed install, another try.
    fn check(&self, cx: &mut App) {
        #[cfg(not(target_os = "linux"))]
        {
            _ = cx;
            unsafe { platform::host_check_for_updates_quietly() };
        }
        #[cfg(target_os = "linux")]
        if !flatpak::retry(cx) {
            flatpak::check(cx);
        }
    }

    /// On the Mac, Sparkle's window on the update it found and won't download by itself. Only the
    /// Mac's updater has one to show; elsewhere it's a check, as About's.
    fn show(&self, cx: &mut App) {
        #[cfg(not(target_os = "linux"))]
        {
            _ = cx;
            unsafe { platform::host_check_for_updates() };
        }
        #[cfg(target_os = "linux")]
        flatpak::check(cx);
    }

    fn restart(&self, cx: &mut App) {
        #[cfg(not(target_os = "linux"))]
        {
            _ = cx;
            unsafe { platform::host_install_update() };
        }
        #[cfg(target_os = "linux")]
        flatpak::restart(cx);
    }

    /// Kept for the next launch, which reads it before the updater's first check. Linux offers no
    /// choice: a Flatpak updates only the branch it's installed from.
    fn check_automatically(&self, on: bool, _: &mut App) {
        #[cfg(not(target_os = "linux"))]
        set_automatic_checks(on);
        #[cfg(target_os = "linux")]
        let _ = on;
    }

    fn follow(&self, track: updates::Track, _: &mut App) {
        #[cfg(not(target_os = "linux"))]
        {
            let name = match track {
                updates::Track::Nightly => "nightly",
                updates::Track::Release => "release",
            };
            if let Err(error) = std::fs::write(followed_track_path(), name) {
                log!("browser: the update track wasn't kept: {error}");
            }
            unsafe { platform::host_follow_nightly(track == updates::Track::Nightly) };
        }
        #[cfg(target_os = "linux")]
        let _ = track;
    }
}

/// Sparkle or Windows's updater says where the update stands, and the version it's of, if it says.
#[cfg(not(target_os = "linux"))]
pub(crate) extern "C" fn update_changed(stage: c_int, version: *const c_char) {
    use updates::Stage;
    let version = (!version.is_null())
        .then(|| {
            unsafe { CStr::from_ptr(version) }
                .to_string_lossy()
                .into_owned()
        })
        .filter(|version| !version.is_empty())
        .map(SharedString::from);
    let stage = match stage {
        platform::UPDATE_CHECKING => Stage::Checking,
        platform::UPDATE_CURRENT => Stage::Current,
        platform::UPDATE_FOUND => Stage::Found(version),
        platform::UPDATE_AVAILABLE => Stage::Available(version),
        platform::UPDATE_DOWNLOADING => Stage::Downloading(version),
        platform::UPDATE_READY => Stage::Ready(version),
        platform::UPDATE_FAILED => Stage::Failed { found: false },
        // HOST_UPDATE_IDLE.
        _ => Stage::Idle,
    };
    // Sparkle can say it from inside the call that asked it to check.
    update_app_later(move |cx| updates::changed(stage, cx));
}

/// Has the portal start the build Flatpak installed once this one has let go of the profile, as
/// the restart into it goes ahead: true once the portal has.
#[cfg(target_os = "linux")]
pub(crate) fn relaunch_into_newest(portal: &flatpak::Portal) -> bool {
    let lock = std::path::Path::new(&profile_dir()).join("SingletonLock");
    match flatpak::start_newest(portal, &lock, flatpak::SPAWN) {
        Ok(()) => true,
        Err(error) => {
            log!("browser: no relaunch: {error}");
            false
        }
    }
}

pub(crate) fn relaunch_into_update() -> bool {
    #[cfg(not(target_os = "linux"))]
    return unsafe { platform::host_relaunch_into_update() };
    #[cfg(target_os = "linux")]
    {
        let mut relaunched = false;
        update_app(|cx| relaunched = flatpak::relaunch(cx));
        relaunched
    }
}
