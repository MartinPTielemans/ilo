//! What Settings asks the core for of a profile: its logins, addresses and cards, the account's
//! sync and devices, and what an import brought.

use super::*;

/// The window showing `tab`'s page, as the core says, else the one whose shell thinks it does,
/// else the one last key, such as for a key from a page no shell has heard is shown yet.
/// Removes the profile's `login`, or takes its site off Never, as the Site Controls and Settings
/// do. The user's alone: agents get sessions, never secrets.
pub(crate) fn remove_login(profile: bc::bc_id, login: &passwords::Login) {
    let text = |text: &str| CString::new(text).unwrap_or_default();
    let (key, username) = (text(&login.key), text(&login.username));
    // check.sh reads this line.
    log!(
        "browser: remove login {profile} {}",
        passwords::describe(std::slice::from_ref(login))
    );
    unsafe { bc::bc_profile_remove_login(profile, key.as_ptr(), username.as_ptr(), login.never) };
}

pub(crate) unsafe extern "C" fn history_added(_: *mut c_void, profile: bc::bc_id, added: usize) {
    log!("browser: history added {added} to profile {profile}");
    update_app_later(move |cx| import_host::history_added(added, cx));
}

/// The user deleted pages from the profile's history, and the command bar forgets what it learned
/// of them. The pages aren't logged: they're the user's.
pub(crate) unsafe extern "C" fn history_deleted(
    _: *mut c_void,
    profile: bc::bc_id,
    all: bool,
    begin: i64,
    end: i64,
    urls: *const *const c_char,
    count: usize,
) {
    let urls = match count {
        0 => &[][..],
        count => unsafe { std::slice::from_raw_parts(urls, count) },
    };
    let urls: Vec<String> = (urls.iter())
        .map(|&url| {
            unsafe { CStr::from_ptr(url) }
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    // check.sh reads this line.
    match all {
        true => log!("browser: history deleted all from profile {profile}"),
        false => log!(
            "browser: history deleted {} urls from profile {profile}",
            urls.len()
        ),
    }
    let deleted = match all {
        true => browser::shell::HistoryDeleted::All,
        false => browser::shell::HistoryDeleted::Some {
            begin: begin.max(0) as u64,
            end: end.max(0) as u64,
            urls,
        },
    };
    update_app_later(move |cx| {
        browser::shell::history_deleted(profile, deleted, cx);
        // Library › History lists what's left.
        update_shells_in(cx, |shell, cx| shell.history_forgotten(profile, cx));
    });
}

/// The profile's visited pages a window's History asked for with `request`, newest first, and
/// whether older ones are left. Each window keeps its own answers. The pages aren't logged:
/// they're the user's.
pub(crate) unsafe extern "C" fn history_listed(
    _: *mut c_void,
    profile: bc::bc_id,
    request: u64,
    pages: *const bc::bc_history_page,
    count: usize,
    more: bool,
) {
    let pages = match count {
        0 => &[][..],
        count => unsafe { std::slice::from_raw_parts(pages, count) },
    };
    let text = |text: *const c_char| -> SharedString {
        match text.is_null() {
            true => SharedString::default(),
            false => unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned()
                .into(),
        }
    };
    let pages: Vec<browser::shell::Visited> = (pages.iter())
        .map(|page| browser::shell::Visited {
            url: text(page.url),
            title: text(page.title),
            // The Library's times are milliseconds.
            visited: page.last_visit as f64 / 1000.,
        })
        .collect();
    log!(
        "browser: history listed {} for request {request} of profile {profile}, more {more}",
        pages.len()
    );
    update_app_later(move |cx| {
        update_shells_in(cx, |shell, cx| {
            shell.history_listed(profile, request, pages.clone(), more, cx)
        })
    });
}

pub(crate) unsafe extern "C" fn addresses_added(_: *mut c_void, profile: bc::bc_id, added: usize) {
    // check.sh waits for this line.
    log!("browser: addresses added {added} to profile {profile}");
    update_app_later(move |cx| import_host::addresses_added(added, cx));
}

/// The core's `count` logins at `logins`, as the app keeps them.
pub(crate) fn logins(logins: *const bc::bc_login, count: usize) -> Vec<passwords::Login> {
    let string = |text: *const c_char| -> SharedString {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
            .into()
    };
    let logins = match count {
        0 => &[][..],
        count => unsafe { std::slice::from_raw_parts(logins, count) },
    };
    (logins.iter())
        .map(|login| passwords::Login {
            site: string(login.site),
            username: string(login.username),
            never: login.never,
            key: string(login.key),
        })
        .collect()
}

/// The profile's saved addresses and cards, which Settings asked the core for.
pub(crate) unsafe extern "C" fn profile_autofill_changed(
    _: *mut c_void,
    profile: bc::bc_id,
    entries: *const bc::bc_autofill_entry,
    count: usize,
) {
    let string = |text: *const c_char| -> SharedString {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
            .into()
    };
    let entries = match count {
        0 => &[][..],
        count => unsafe { std::slice::from_raw_parts(entries, count) },
    };
    let saved: Vec<settings::Saved> = (entries.iter())
        .map(|entry| settings::Saved {
            title: string(entry.title),
            detail: string(entry.detail),
            card: entry.card,
            key: string(entry.key),
        })
        .collect();
    update_app(|cx| settings::autofill_changed(profile, saved, cx));
}

/// The profile's logins, which Settings asked the core for.
pub(crate) unsafe extern "C" fn profile_logins_changed(
    _: *mut c_void,
    profile: bc::bc_id,
    list: *const bc::bc_login,
    count: usize,
) {
    let list = logins(list, count);
    update_app(|cx| settings::logins_changed(profile, list, cx));
}

/// The browser's account, for Settings. Never logged here: the email is the user's.
pub(crate) unsafe extern "C" fn sync_changed(_: *mut c_void, state: *const bc::bc_sync_state) {
    let Some(state) = (unsafe { state.as_ref() }) else {
        return;
    };
    let status = match state.status as u32 {
        bc::BC_SYNC_SIGNING_IN => account::Status::SigningIn,
        bc::BC_SYNC_FIRST_DEVICE => account::Status::FirstDevice,
        bc::BC_SYNC_NEEDS_SECRET => account::Status::NeedsSecret,
        bc::BC_SYNC_PAIRING => account::Status::Pairing,
        bc::BC_SYNC_ON => account::Status::On,
        _ => account::Status::SignedOut,
    };
    let problem = match state.problem as u32 {
        bc::BC_SYNC_PROBLEM_WRONG_WORDS => Some(account::Problem::WrongWords),
        bc::BC_SYNC_PROBLEM_DECLINED => Some(account::Problem::Declined),
        bc::BC_SYNC_PROBLEM_EXPIRED => Some(account::Problem::Expired),
        bc::BC_SYNC_PROBLEM_REMOVED => Some(account::Problem::Removed),
        bc::BC_SYNC_PROBLEM_SERVER => Some(account::Problem::Server),
        _ => None,
    };
    let state = account::State {
        status,
        problem,
        server: core_text(state.server),
        default_server: state.default_server,
        email: core_text(state.email),
        code: core_text(state.code),
        settings: state.settings,
        extensions: state.extensions,
    };
    if CX.with_borrow(Option::is_none) {
        SYNC_STATE.set(Some(state));
        return;
    }
    update_app_later(move |cx| settings::sync_changed(state, cx));
}

/// Settings came from the account's other devices, and the core has them: what the app keeps of
/// them follows, and Settings shows them. Before the app runs there's nothing to follow, as its
/// windows read them as they open.
pub(crate) unsafe extern "C" fn settings_synced(_: *mut c_void) {
    update_app_later(|cx| {
        little_windows_followed(cx);
        settings::look_followed(cx);
    });
}

/// Another device asks to be added to the account: the dialog asks over the front window, or
/// leaves once the request has ended unanswered.
pub(crate) unsafe extern "C" fn sync_pairing_asked(
    _: *mut c_void,
    pairing: *const bc::bc_sync_pairing,
) {
    let Some(pairing) = (unsafe { pairing.as_ref() }) else {
        log!("browser: pairing withdrawn");
        update_app_later(dialog::withdraw_account);
        return;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0., |since| since.as_secs_f64() * 1e3);
    let mut day_starts = [0.; 4];
    unsafe { platform::host_day_starts(day_starts.as_mut_ptr()) };
    let asked_at = if pairing.asked_at > 0. {
        pairing.asked_at
    } else {
        now
    };
    let signed_in = match pairing.signed_in_at > 0. {
        true => account::since(now - pairing.signed_in_at),
        false => String::new(),
    };
    let pairing = account::Pairing {
        id: core_text(pairing.id),
        name: core_text(pairing.name),
        os: core_text(pairing.os),
        code: core_text(pairing.code),
        asked: account::clock(asked_at, day_starts[0]).into(),
        signed_in: signed_in.into(),
    };
    // check.sh waits for this line.
    log!("browser: pairing asked");
    update_app_later(move |cx| {
        let id = CString::new(pairing.id.as_str()).unwrap_or_default();
        let answered = Box::new(move |add, _: &mut App| unsafe {
            bc::bc_sync_answer_pairing(id.as_ptr(), add)
        });
        dialog::ask_account(account::pairing_question(&pairing), answered, cx);
    });
}

/// The account's devices, which Settings asked the core for.
pub(crate) unsafe extern "C" fn sync_devices_changed(
    _: *mut c_void,
    devices: *const bc::bc_sync_device,
    count: usize,
) {
    let devices = match count {
        0 => &[][..],
        count => unsafe { std::slice::from_raw_parts(devices, count) },
    };
    let devices = (devices.iter())
        .map(|device| account::Device {
            id: core_text(device.id),
            name: core_text(device.name),
            os: core_text(device.os),
            current: device.current,
        })
        .collect();
    update_app_later(move |cx| settings::devices_changed(devices, cx));
}

/// What came of a passwords file the import gave the core. Never logged: the counts are the
/// user's.
pub(crate) unsafe extern "C" fn passwords_imported(
    _: *mut c_void,
    profile: bc::bc_id,
    result: i32,
    imported: usize,
    conflicted: usize,
    skipped: usize,
) {
    let passwords = match result as u32 {
        bc::BC_PASSWORDS_IMPORTED => browser::import::Passwords::Imported {
            imported,
            conflicted,
            skipped,
        },
        bc::BC_PASSWORDS_IMPORT_UNREADABLE => browser::import::Passwords::Unreadable,
        bc::BC_PASSWORDS_IMPORT_NOT_EXPORT => browser::import::Passwords::NotExport,
        bc::BC_PASSWORDS_IMPORT_TOO_LARGE => browser::import::Passwords::TooLarge,
        _ => browser::import::Passwords::Failed,
    };
    update_app_later(move |cx| import_host::passwords_imported(profile, passwords, cx));
}
