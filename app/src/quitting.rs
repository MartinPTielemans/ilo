//! Quitting and restarting, and what's kept for the next launch: the launch picture and frame.

use super::*;

/// The front window's frame, kept beside its launch picture, for the next launch's window to open
/// at before the core can say where the browser window will.
pub(crate) fn launch_frame_path() -> String {
    format!("{}/launch-frame", profile_dir())
}

/// Where the launch picture's size and layout are kept (launch_picture::line), beside it and
/// named as the hosts name it, so they and forget_launch_pictures delete it with it.
pub(crate) fn launch_layout_path() -> String {
    format!("{}/launch-picture-layout", profile_dir())
}

/// Quits the browser, saying what asked, as each way the browser ends logs a line that names it:
/// this, Chromium's shutdown (main_delegate.cc) and, on the Mac, a quit Apple Event (host.mm).
pub(crate) fn quit_because(why: &str) {
    log!("browser: quit for {why}");
    unsafe { bc::bc_quit() };
}

/// Restarts the browser, saying what asked, as quit_because does.
pub(crate) fn restart_because(why: &str) {
    log!("browser: restart for {why}");
    unsafe { bc::bc_restart() };
}

/// The quit goes ahead, and nothing can stop it now. A restart with an update ready relaunches
/// into the update, which installs first, rather than Chrome starting this build again. Only now,
/// so a quit a page stopped leaves nothing armed for the next (#1459).
pub(crate) unsafe extern "C" fn quit_going_ahead(_: *mut c_void, restarting: bool) -> bool {
    log!(
        "browser: quit going ahead{}",
        if restarting { ", restarting" } else { "" }
    );
    shell::quit_goes_ahead();
    // Chrome closes the core's windows as this returns (OnAppExiting), and destroys their surfaces
    // a task before the core says each has closed (window_closed): the overlays over them go now.
    #[cfg(target_os = "linux")]
    gpui_linux::embedder_windows_going();
    restarting && relaunch_into_update()
}

pub(crate) unsafe extern "C" fn private_session_ended(_: *mut c_void, profile: bc::bc_id) {
    // check.sh reads this line.
    log!("browser: private session ended {profile}");
}

pub(crate) unsafe extern "C" fn stopping(_: *mut c_void) {
    log!("browser: stopping");
    safe_start_host::stopping();
    // On Windows, a ready update installs once the app has gone.
    #[cfg(not(target_os = "linux"))]
    unsafe {
        platform::host_finish_update()
    };
    // The recovery words a Copy put on the clipboard don't outlast the app (settings.rs).
    unsafe { platform::host_forget_secret() };
    // Tasks live for the run: their CLIs stop and their Spaces go while the core still answers,
    // on every way out, the Dock's Quit and logging out included. A crash leaves them for the
    // next launch.
    let spaces = WINDOWS.with_borrow(|windows| !windows.is_empty());
    update_app(|cx| {
        if let Some(agents) = browser::agents::global(cx) {
            agents.update(cx, |agents, cx| agents.end_tasks(spaces, cx));
        }
    });
}

/// Quits and starts again, bringing back the windows and their Spaces: the protected-media note's
/// and Settings' way to load Google's Widevine module.
pub(crate) fn restart_browser(cx: &mut App) {
    // As Quit does: once the view that asked is done, and before Chromium's posted quit.
    cx.defer(save_launch_picture);
    unsafe { bc::bc_restart() };
}

// The next launch's first window opens where the regular window in front is, showing its sidebar
// until its UI has drawn. Taken as the app stops being the active one and on the user's Quit, as a
// window is already hidden when it closes, and the frame alone as the last regular window closes.
// Nothing of a private window is kept.
pub(crate) extern "C" fn launch_picture_on_resigning() {
    // Later: AppKit can say so from inside an update of ours, such as one that hides the app.
    update_app_later(save_launch_picture);
}

// The left |width| points of the frame |window| last drew, its sidebar, top-down BGRA, with how many
// pixels across: only the sidebar, as the page could be anything and shouldn't be kept on disk.
// From GPUI's own frame, as Windows' picture of the window has it repaint its frame, where the
// theme paints the caption under the UI, on screen too (#1262).
#[cfg(target_os = "windows")]
fn sidebar_pixels(window: AnyWindowHandle, width: f64, cx: &mut App) -> Option<(Vec<u8>, usize)> {
    let (image, scale) = cx
        .update_window(window, |_, window, _| {
            (window.render_to_image(), window.scale_factor())
        })
        .ok()?;
    let image = image.ok()?;
    let (across, rows) = (image.width() as usize, image.height() as usize);
    let columns = ((width * f64::from(scale)).round() as usize).min(across);
    if columns == 0 || rows == 0 {
        return None;
    }
    let mut pixels = Vec::with_capacity(columns * rows * 4);
    for row in image.as_raw().chunks_exact(across * 4) {
        for pixel in row[..columns * 4].chunks_exact(4) {
            pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }
    Some((pixels, columns))
}

pub(crate) fn save_launch_picture(cx: &mut App) {
    // The regular window in front, which the next launch opens first.
    let Some(window) = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.regular()).cloned())
    else {
        return;
    };
    save_launch_frame(window.ui_view);
    // A launch that doesn't bring the windows back shows their Spaces without Today tabs, so a
    // picture of this sidebar would show tabs that aren't there. Without one it shows the plain
    // window, as a new profile's first launch does.
    // The hosts take and show the column at the window's left, so with the sidebar on the right
    // the launch shows the plain window too.
    if !unsafe { bc::bc_restore_windows() } || browser::shell::sidebar_right(cx) {
        forget_launch_pictures();
        log!("browser: launch picture not taken");
        return;
    }
    let shell = window.shell.upgrade();
    let width = shell
        .as_ref()
        .map_or(0., |shell| shell.read(cx).card_left());
    // The frame the hosts name the picture by.
    let (mut x, mut y, mut frame_width, mut frame_height) = (0., 0., 0., 0.);
    let framed = unsafe {
        platform::host_window_frame(
            window.ui_view,
            &mut x,
            &mut y,
            &mut frame_width,
            &mut frame_height,
        )
    };
    let layout = shell
        .filter(|_| framed)
        .map(|shell| shell.read(cx).launch_picture(frame_width, frame_height));
    let Ok(profile) = CString::new(profile_dir()) else {
        return;
    };
    let profile = profile.as_ptr();
    #[cfg(not(target_os = "windows"))]
    let taken =
        unsafe { platform::host_save_launch_picture(window.ui_view, profile, width.into()) };
    #[cfg(target_os = "windows")]
    let taken = sidebar_pixels(window.handle, width.into(), cx).is_some_and(|(pixels, columns)| {
        let rows = pixels.len() / 4 / columns;
        unsafe {
            platform::host_save_launch_pixels(
                window.ui_view,
                profile,
                pixels.as_ptr(),
                columns,
                rows,
            )
        }
    });
    // Once the host has deleted the pictures before, and only for one it took.
    if let Some(layout) = layout.filter(|_| taken) {
        _ = std::fs::write(launch_layout_path(), launch_picture::line(&layout));
    }
    // check.sh reads this line.
    log!(
        "browser: launch picture {}",
        if taken { "taken" } else { "not taken" }
    );
}

/// Deletes the launch pictures the host kept in the profile directory, one a window size
/// (LaunchPicturePath in platform/mac/host.mm).
pub(crate) fn forget_launch_pictures() {
    let Ok(files) = std::fs::read_dir(profile_dir()) else {
        return;
    };
    for file in files.flatten() {
        let name = file.file_name();
        if name.to_string_lossy().starts_with("launch-picture-") {
            _ = std::fs::remove_file(file.path());
        }
    }
}

/// Keeps the frame of the browser window of `ui_view` for the next launch's launch window to show
/// at, before the core has read the windows it saved. Out of full screen only: the core keeps the
/// frame a window in full screen comes back at. Maximized on Windows, the frame it un-maximizes to
/// and "maximized" after it (launching), for the launch window alone: the browser window opens
/// maximized as the core saved it (bc_saved_window_maximized).
pub(crate) fn save_launch_frame(ui_view: *mut c_void) {
    let (mut x, mut y, mut width, mut height) = (0., 0., 0., 0.);
    if unsafe { platform::host_window_frame(ui_view, &mut x, &mut y, &mut width, &mut height) } {
        _ = std::fs::write(launch_frame_path(), format!("{x} {y} {width} {height}\n"));
        return;
    }
    #[cfg(target_os = "windows")]
    if unsafe {
        platform::host_window_restored_frame(ui_view, &mut x, &mut y, &mut width, &mut height)
    } {
        _ = std::fs::write(
            launch_frame_path(),
            format!("{x} {y} {width} {height} maximized\n"),
        );
    }
}
