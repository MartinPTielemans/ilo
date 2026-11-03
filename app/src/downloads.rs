//! Downloads: the list, their icons, and the question a quit with downloads running asks.

use super::*;

pub(crate) unsafe extern "C" fn downloads_changed(
    _: *mut c_void,
    downloads: *const bc::bc_download,
    count: usize,
) {
    let downloads = match downloads.is_null() {
        true => &[][..],
        false => unsafe { std::slice::from_raw_parts(downloads, count) },
    };
    let text = |text| {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
            .into()
    };
    let downloads: Vec<_> = downloads
        .iter()
        .map(|download| Download {
            id: download.id,
            name: text(download.name),
            url: text(download.url),
            path: text(download.path),
            received: download.received,
            total: download.total,
            state: match download.state as u32 {
                bc::BC_DOWNLOAD_RUNNING => DownloadState::Running,
                bc::BC_DOWNLOAD_PAUSED => DownloadState::Paused,
                bc::BC_DOWNLOAD_DONE => DownloadState::Done,
                bc::BC_DOWNLOAD_CANCELLED => DownloadState::Cancelled,
                _ => DownloadState::Failed,
            },
            open_when_done: download.open_when_done,
            held: download.held,
            started: download.started,
        })
        .collect();
    update_shells(|shell, cx| shell.set_downloads(downloads.clone(), cx));
}

pub(crate) unsafe extern "C" fn download_icon(
    _: *mut c_void,
    download: bc::bc_id,
    png: *const u8,
    size: usize,
) {
    let icon = (!png.is_null()).then(|| {
        let png = unsafe { std::slice::from_raw_parts(png, size) }.to_vec();
        Arc::new(Image::from_bytes(ImageFormat::Png, png))
    });
    update_shells(|shell, cx| shell.set_download_icon(download, icon.clone(), cx));
}

/// On Linux and Windows, Chrome asks whether to quit, or close the last private window, cancelling
/// `count` downloads running. In the words of Chrome's alert on the Mac, with the app's name where
/// it names Chromium, and ↩ on Continue Downloading, as there.
pub(crate) unsafe extern "C" fn window_downloads_question(
    _: *mut c_void,
    window: bc::bc_id,
    count: c_int,
    last_private: bool,
) {
    let question = dialog::Question {
        icon: None,
        message: match count {
            1 => "Download is in progress",
            _ => "Downloads are in progress",
        }
        .into(),
        detail: match last_private {
            true => "Quit Incognito mode anyway?",
            false => concat!("Quit ", env!("APP_NAME"), " anyway?"),
        }
        .into(),
        list: Vec::new(),
        action: "Quit".into(),
        cancel: "Continue Downloading".into(),
        // Quit cancels the downloads, which can't be taken back.
        tone: dialog::Tone::Destructive,
        also: None,
        fields: Vec::new(),
        check: None,
        on_return: dialog::Return::Cancel,
    };
    let answered = Box::new(move |cancel, _: &mut App| unsafe {
        bc::bc_window_answer_downloads(window, cancel)
    });
    // With no UI to ask in, the browser stays open, as it does with no dialog to ask in.
    if CX.with_borrow(Option::is_none) {
        unsafe { bc::bc_window_answer_downloads(window, false) };
        return;
    }
    // A private window's close can ask from inside the app's own call.
    update_app_later(move |cx| dialog::ask_chrome(question, answered, cx));
}

pub(crate) unsafe extern "C" fn window_show_downloads(_: *mut c_void, window: bc::bc_id) {
    update_window(window, |shell, cx| shell.show_downloads(cx));
}
