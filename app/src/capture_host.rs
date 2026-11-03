//! The capture's host: screenshots and recordings of pages, where they're kept, and the elements
//! and pictures the core takes of a page.

use super::*;

/// Capture's requests, as calls to the core, to the host's window for capture mode and to the
/// system, over the window in front (`front`).
pub(crate) struct CaptureHost {
    /// The toast's view, which its Share button's menu shows from.
    pub(crate) toast: Rc<ToastHost>,
}

impl capture::Host for CaptureHost {
    fn show(&self, frame: Bounds<Pixels>) {
        let (left, top) = (f64::from(frame.origin.x), f64::from(frame.origin.y));
        let (width, height) = (f64::from(frame.size.width), f64::from(frame.size.height));
        unsafe { platform::host_show_capture(front().1, left, top, width, height) };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_capture() };
    }

    fn camera(&self, camera: bool) {
        unsafe { platform::host_capture_camera(camera) };
    }

    fn element_at(&self, tab: u64, at: Point<Pixels>) {
        unsafe { bc::bc_tab_element_at(tab, f64::from(at.x), f64::from(at.y)) };
    }

    fn capture(&self, tab: u64, request: u64, full_page: bool, rect: Option<Bounds<Pixels>>) {
        let rect = rect.map_or(
            bc::bc_rect {
                x: 0.,
                y: 0.,
                width: 0.,
                height: 0.,
            },
            |rect| bc::bc_rect {
                x: f64::from(rect.origin.x),
                y: f64::from(rect.origin.y),
                width: f64::from(rect.size.width),
                height: f64::from(rect.size.height),
            },
        );
        unsafe { bc::bc_tab_capture(tab, request, full_page, rect) };
    }

    fn end(&self, tab: u64) {
        unsafe { bc::bc_tab_capture_end(tab) };
    }

    fn copy(&self, png: &[u8]) {
        #[cfg(target_os = "linux")]
        system::copy_png(png);
        #[cfg(not(target_os = "linux"))]
        unsafe {
            platform::host_copy_png(png.as_ptr(), png.len())
        };
    }

    fn save(&self, png: &[u8]) {
        #[cfg(target_os = "linux")]
        system::save_png(png);
        #[cfg(not(target_os = "linux"))]
        unsafe {
            platform::host_save_png(front().1, png.as_ptr(), png.len())
        };
    }

    fn save_to_library(&self, png: &[u8]) {
        let (name, png) = (capture_name(), png.to_vec());
        // After the update under way, which the capture's menu is in.
        update_app_later(move |cx| {
            // A picture of a whole screen is megabytes, too many to write on the main thread.
            let saved = cx.background_spawn(async move { save_capture(&name, &png) });
            cx.spawn(async move |cx| {
                if saved.await {
                    cx.update(list_captures);
                }
            })
            .detach();
        });
    }

    fn share(&self, png: &[u8], share: capture::Share) {
        let (view, at, messages) = match share {
            capture::Share::Menu(at) => (front().1, at, false),
            capture::Share::Toast(at) => (
                self.toast.view.get(),
                Bounds::new(at, size(px(1.), px(1.))),
                false,
            ),
            capture::Share::Messages => (std::ptr::null_mut(), Bounds::default(), true),
        };
        let (x, y) = (f64::from(at.origin.x), f64::from(at.origin.y));
        let (width, height) = (f64::from(at.size.width), f64::from(at.size.height));
        unsafe {
            platform::host_share_png(png.as_ptr(), png.len(), view, x, y, width, height, messages)
        };
    }
}

/// Where the Library's captures are.
pub(crate) fn media_dir() -> PathBuf {
    PathBuf::from(profile_dir()).join("Media")
}

/// Saves a capture into the Library as `name`, and says whether it did.
pub(crate) fn save_capture(name: &str, png: &[u8]) -> bool {
    let media = media_dir();
    // Two captures in a second each keep theirs, as the system's screenshots do.
    let stem = name.trim_end_matches(".png");
    let path = (1..)
        .map(|n| match n {
            1 => media.join(name),
            n => media.join(format!("{stem} ({n}).png")),
        })
        .find(|path| !path.exists())
        .unwrap_or_default();
    match std::fs::create_dir_all(&media).and_then(|_| std::fs::write(&path, png)) {
        Ok(()) => {
            // check.sh reads this line.
            log!("browser: saved to library {}", path.display());
            true
        }
        Err(error) => {
            log!(
                "couldn't save to the library at {}: {error}",
                path.display()
            );
            false
        }
    }
}

/// Lists the Library's captures, newest first, away from the main thread, for every window.
pub(crate) fn list_captures(cx: &mut App) {
    let listed = cx.background_spawn(async {
        let entries = std::fs::read_dir(media_dir())
            .into_iter()
            .flatten()
            .flatten();
        let mut captures: Vec<_> = entries
            .filter_map(|entry| {
                let modified = entry.metadata().ok()?.modified().ok()?;
                let saved = modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()?
                    .as_millis();
                let name = entry.file_name().into_string().ok()?;
                let path = entry.path().into_os_string().into_string().ok()?;
                Some(shell::Captured {
                    path: path.into(),
                    name: name.into(),
                    saved: saved as f64,
                })
            })
            .collect();
        captures.sort_by(|a, b| b.saved.total_cmp(&a.saved));
        captures
    });
    cx.spawn(async move |_| {
        let captures = listed.await;
        // check.sh reads this line.
        log!("browser: library captures {}", captures.len());
        update_shells(|shell, cx| shell.set_captures(captures.clone(), cx));
    })
    .detach();
}

/// A capture's file name, as the system names a screenshot.
pub(crate) fn capture_name() -> String {
    let mut name = [0 as c_char; 64];
    unsafe { platform::host_capture_name(name.as_mut_ptr(), name.len()) };
    unsafe { CStr::from_ptr(name.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

/// The user left capture mode: a click outside it, another window or another app.
pub(crate) extern "C" fn capture_dismissed() {
    update_app(capture::dismissed);
}

pub(crate) unsafe extern "C" fn tab_element_box(
    _: *mut c_void,
    tab: bc::bc_id,
    x: f64,
    y: f64,
    box_: bc::bc_rect,
) {
    let at = point(px(x as f32), px(y as f32));
    let found = (box_.width > 0. && box_.height > 0.).then(|| {
        Bounds::new(
            point(px(box_.x as f32), px(box_.y as f32)),
            size(px(box_.width as f32), px(box_.height as f32)),
        )
    });
    update_app_later(move |cx| capture::element_box(tab, at, found, cx));
}

pub(crate) unsafe extern "C" fn tab_captured(
    _: *mut c_void,
    tab: bc::bc_id,
    request: u64,
    png: *const u8,
    size: usize,
    width: i32,
    height: i32,
) {
    // check.sh reads this line.
    log!("browser: tab {tab} captured {request}: {width}x{height}, {size} bytes");
    let png = (!png.is_null()).then(|| unsafe { std::slice::from_raw_parts(png, size) }.to_vec());
    update_app_later(move |cx| capture::captured(tab, request, png, cx));
}

pub(crate) unsafe extern "C" fn trace_recorded(_: *mut c_void, json: *const c_char, size: usize) {
    // check.sh reads this line.
    log!("browser: trace recorded {size} bytes");
    let json = (!json.is_null()).then(|| unsafe { std::slice::from_raw_parts(json.cast(), size) });
    let json = json.map(<[u8]>::to_vec);
    update_app_later(move |cx| shell::trace_recorded(json, cx));
}

pub(crate) unsafe extern "C" fn tab_pictured(
    _: *mut c_void,
    tab: bc::bc_id,
    request: u64,
    bgra: *const u8,
    width: i32,
    height: i32,
) {
    // check.sh reads this line.
    log!("browser: tab {tab} pictured {request}: {width}x{height}");
    if hover_card::is_card_request(request) {
        let (width, height) = (width.max(0) as u32, height.max(0) as u32);
        let size = width as usize * height as usize * 4;
        let picture = (!bgra.is_null() && size > 0)
            .then(|| unsafe { std::slice::from_raw_parts(bgra, size) }.to_vec())
            .and_then(|bgra| PagePicture::from_bgra(bgra, width, height));
        return update_app_later(move |cx| hover_card::answered(request, picture, cx));
    }
    browser::log_handover("picture in", &format!("tab {tab}, {width}x{height}"));
    let (width, height) = (width.max(0) as u32, height.max(0) as u32);
    let size = width as usize * height as usize * 4;
    let picture = (!bgra.is_null() && size > 0)
        .then(|| unsafe { std::slice::from_raw_parts(bgra, size) }.to_vec())
        .and_then(|bgra| PagePicture::from_bgra(bgra, width, height));
    if let Some(cx) = CX.with_borrow(Clone::clone) {
        let shells = WINDOWS
            .with_borrow(|windows| windows.iter().map(|window| window.shell.clone()).collect());
        shell::picture_answered(shells, request, picture, &cx);
    }
}
