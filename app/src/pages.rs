//! What a tab's page asks of the user: its dialogs, permissions, Cast, its menu, the Site
//! Controls, offers, protected media and sharing.

use super::*;

/// Whether host_share opens a share sheet, which the tab menu's Share… and Site Controls' Share
/// tile both ask. Linux's host has none yet.
pub(crate) fn has_share_sheet() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}

/// Opens the system's share sheet in `window` for the page at `url`, hanging from `frame`.
pub(crate) fn share_page(window: &BrowserWindow, url: &str, title: &str, frame: Bounds<Pixels>) {
    let (Ok(url), Ok(title)) = (CString::new(url), CString::new(title)) else {
        return;
    };
    let frame = rect(frame);
    let (x, y, width, height) = (frame.x, frame.y, frame.width, frame.height);
    let (url, title) = (url.as_ptr(), title.as_ptr());
    unsafe { platform::host_share(window.ui_view, x, y, width, height, url, title) };
}

/// Asks `message` in an alert with `buttons`, the first the default if `returns`, and returns the
/// index of the one chosen, or none if AppKit ended it. It blocks in AppKit's loop until the alert
/// closes, so it's called in a task. Linux has no alert, and its loop can't wait for one
/// (settings.rs).
#[cfg(not(target_os = "linux"))]
pub(crate) fn ask(message: &str, detail: &str, buttons: &[&str], returns: bool) -> usize {
    let text = |text: &str| CString::new(text).unwrap_or_default();
    let (message, detail) = (text(message), text(detail));
    let buttons: Vec<CString> = buttons.iter().map(|button| text(button)).collect();
    let pointers: Vec<*const c_char> = buttons.iter().map(|button| button.as_ptr()).collect();
    let chosen = unsafe {
        platform::host_ask(
            message.as_ptr(),
            detail.as_ptr(),
            pointers.as_ptr(),
            pointers.len(),
            returns,
        )
    };
    usize::try_from(chosen).unwrap_or(usize::MAX)
}

/// Shows `tab`'s page in Safari's engine, in the tab, and has its profile keep the site for it: the
/// protected-video note's button and Site Controls' On.
pub(crate) fn use_system_engine(tab: u64) {
    // check.sh reads this line.
    let used = unsafe { bc::bc_tab_set_system_engine(tab, true) };
    log!(
        "browser: system engine {tab} {}",
        if used { "on" } else { "refused" }
    );
}

pub(crate) unsafe extern "C" fn tab_site_controls(
    _: *mut c_void,
    tab: bc::bc_id,
    controls: *const bc::bc_site_controls,
) {
    let controls = unsafe { &*controls };
    let string = |text: *const c_char| -> SharedString {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
            .into()
    };
    let value = |value: u32| match value {
        bc::BC_SITE_ALLOW => site_controls::Value::Allow,
        bc::BC_SITE_BLOCK => site_controls::Value::Block,
        _ => site_controls::Value::Ask,
    };
    let bit = |each: site_controls::Value| match each {
        site_controls::Value::Allow => bc::BC_SITE_ALLOW,
        site_controls::Value::Block => bc::BC_SITE_BLOCK,
        site_controls::Value::Ask => bc::BC_SITE_ASK,
    };
    let permissions = match controls.count {
        0 => &[][..],
        count => unsafe { std::slice::from_raw_parts(controls.permissions, count) },
    };
    let site = site_controls::Site {
        site: string(controls.site),
        connection: match controls.connection as u32 {
            bc::BC_CONNECTION_SECURE => site_controls::Connection::Secure,
            bc::BC_CONNECTION_INTERNAL => site_controls::Connection::Internal,
            _ => site_controls::Connection::NotSecure,
        },
        connection_summary: string(controls.connection_summary),
        certificate: controls.certificate,
        permissions: permissions
            .iter()
            .map(|permission| site_controls::Permission {
                kind: permission.type_,
                key: string(permission.key),
                name: string(permission.name),
                value: value(permission.value as u32),
                state: string(permission.state),
                choices: (site_controls::Value::ALL.into_iter())
                    .filter(|&each| permission.choices & (1 << bit(each)) != 0)
                    .collect(),
            })
            .collect(),
        reset: Some(string(controls.reset)).filter(|reset| !reset.is_empty()),
        system_engine: match (controls.system_engine, controls.system_engine_offered) {
            (true, _) => Some(true),
            (false, true) => Some(false),
            (false, false) => None,
        },
        ad_blocking: match controls.ad_blocking as u32 {
            bc::BC_AD_BLOCKING_ON => Some(true),
            bc::BC_AD_BLOCKING_OFF => Some(false),
            _ => None,
        },
        ad_blocked: string(controls.ad_blocked),
        developer_mode: match controls.developer_mode as u32 {
            bc::BC_DEVELOPER_MODE_ON => Some(true),
            bc::BC_DEVELOPER_MODE_OFF => Some(false),
            _ => None,
        },
        logins: logins(controls.logins, controls.login_count),
        devices: (match controls.device_count {
            0 => &[][..],
            count => unsafe { std::slice::from_raw_parts(controls.devices, count) },
        })
        .iter()
        .map(|device| site_controls::Device {
            name: string(device.name),
            kind: string(device.kind),
            glyph: match device.type_ as u32 {
                bc::BC_DEVICE_SERIAL => shell::DeviceKind::Serial,
                bc::BC_DEVICE_HID => shell::DeviceKind::Hid,
                bc::BC_DEVICE_BLUETOOTH => shell::DeviceKind::Bluetooth,
                _ => shell::DeviceKind::Usb,
            },
            removable: device.removable,
        })
        .collect(),
        boostable: false,
        boost: None,
    };
    update_app(|cx| {
        // The app keeps the Boosts, so its window says what the core can't.
        let shell = tab_window(tab, cx).and_then(|window| window.shell.upgrade());
        let (boostable, boost) =
            shell.map_or((false, None), |shell| shell.read(cx).boost_state(tab));
        let site = site_controls::Site {
            boostable,
            boost,
            ..site
        };
        site_controls::changed(tab, site, cx)
    });
}

pub(crate) unsafe extern "C" fn tab_offer_changed(
    _: *mut c_void,
    tab: bc::bc_id,
    offer: *const bc::bc_offer,
) {
    let offer = (!offer.is_null()).then(|| {
        let offer = unsafe { &*offer };
        let string = |text: *const c_char| -> SharedString {
            unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned()
                .into()
        };
        let usernames = match offer.username_count {
            0 => Vec::new(),
            count => unsafe { std::slice::from_raw_parts(offer.usernames, count) }
                .iter()
                .map(|username| string(*username))
                .collect(),
        };
        let never = string(offer.never);
        let details = string(offer.details);
        offer::Offer {
            kind: match offer.kind as u32 {
                bc::BC_OFFER_KIND_PASSWORD_UPDATE => offer::Kind::Update,
                bc::BC_OFFER_KIND_ADDRESS => offer::Kind::Address,
                bc::BC_OFFER_KIND_CARD => offer::Kind::Card,
                _ => offer::Kind::Save,
            },
            title: string(offer.title),
            site: string(offer.site),
            username: string(offer.username),
            usernames,
            password_length: offer.password_length,
            accept: string(offer.accept),
            decline: string(offer.decline),
            never: (!never.is_empty()).then_some(never),
            details: details
                .lines()
                .map(|line| line.to_string().into())
                .collect(),
        }
    });
    update_app(|cx| offer::changed(tab, offer, cx));
}

pub(crate) unsafe extern "C" fn tab_passwords_changed(
    _: *mut c_void,
    tab: bc::bc_id,
    passwords: c_int,
) {
    let key = match passwords as u32 {
        bc::BC_PASSWORDS_SAVED => Some(passwords::Key::Saved),
        bc::BC_PASSWORDS_OFFER => Some(passwords::Key::Offer),
        _ => None,
    };
    // check.sh reads this line.
    let words = key.map_or("none".into(), |key| format!("{key:?}").to_lowercase());
    log!("browser: key {tab} {words}");
    update_shells(|shell, cx| shell.set_key(tab, key, cx));
}

/// Tells the core the user's answer to the dialog `id` of `tab`'s page.
pub(crate) fn answer_page_dialog(tab: u64, id: u64, reply: shell::PageReply) {
    let text = CString::new(reply.text).unwrap_or_default();
    let password = CString::new(reply.password).unwrap_or_default();
    unsafe {
        bc::bc_tab_answer_dialog(
            tab,
            id,
            reply.accept,
            text.as_ptr(),
            password.as_ptr(),
            reply.checked,
        )
    };
}

/// A tab's page asks a dialog, such as its alert(), or no longer does (null).
pub(crate) unsafe extern "C" fn tab_dialog(
    _: *mut c_void,
    tab: bc::bc_id,
    asked: *const bc::bc_page_dialog,
) {
    let text = |text: *const c_char| -> SharedString {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
            .into()
    };
    let dialog = unsafe { asked.as_ref() }.map(|asked| shell::PageDialog {
        id: asked.id,
        kind: match asked.kind as u32 {
            bc::BC_DIALOG_CONFIRM => shell::DialogKind::Confirm,
            bc::BC_DIALOG_PROMPT => shell::DialogKind::Prompt,
            bc::BC_DIALOG_LEAVE => shell::DialogKind::Leave,
            bc::BC_DIALOG_SIGN_IN => shell::DialogKind::SignIn,
            bc::BC_DIALOG_OPEN_APP => shell::DialogKind::OpenApp,
            _ => shell::DialogKind::Alert,
        },
        title: text(asked.title),
        message: text(asked.message),
        text: text(asked.text),
        accept: text(asked.accept),
        cancel: text(asked.cancel),
        check: text(asked.check),
        username: text(asked.username),
        password: text(asked.password),
    });
    // check.sh waits for these lines.
    match &dialog {
        Some(dialog) => log!(
            "browser: page {tab} dialog {:?} {}",
            dialog.kind,
            dialog.title
        ),
        None => log!("browser: page {tab} dialog none"),
    }
    update_shells(|shell, cx| shell.set_page_dialog(tab, dialog.clone(), cx));
}

pub(crate) unsafe extern "C" fn tab_permission_asked(
    _: *mut c_void,
    tab: bc::bc_id,
    site: *const c_char,
    permissions: *const *const c_char,
    count: usize,
) {
    let string = |text: *const c_char| {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
    };
    let permissions: Vec<String> = match count {
        0 => Vec::new(),
        _ => unsafe { std::slice::from_raw_parts(permissions, count) }
            .iter()
            .map(|permission| string(*permission))
            .collect(),
    };
    let question = match count {
        0 => {
            log!("browser: page {tab} asks nothing");
            None
        }
        _ => {
            let site = string(site);
            let quiet = unsafe { bc::bc_tab_permission_quiet(tab) };
            // check.sh reads this line.
            log!(
                "browser: page {tab} asks {site} to {}{}",
                permissions.join("; "),
                if quiet { ", quietly" } else { "" }
            );
            Some(shell::Question {
                site: site.into(),
                permissions: permissions.into_iter().map(Into::into).collect(),
                quiet,
            })
        }
    };
    // A request held behind the bell, or none any more, which an agent's answers about the tab
    // say (#2323).
    let held = (question.as_ref())
        .filter(|question| question.quiet)
        .map(|question| question.site.to_string());
    agents_core::update(move |agents, cx| agents.held_notifications(tab, held.as_deref(), cx));
    update_shells(|shell, cx| shell.set_question(tab, question.clone(), cx));
}

/// A tab's Cast dialog, or that it closed (a null header), for the window showing the tab.
pub(crate) unsafe extern "C" fn tab_cast(
    _: *mut c_void,
    tab: bc::bc_id,
    header: *const c_char,
    devices: *const bc::bc_cast_device,
    count: usize,
) {
    let string = |text: *const c_char| -> SharedString {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
            .into()
    };
    let cast = (!header.is_null()).then(|| {
        let devices = match count {
            0 => &[][..],
            _ => unsafe { std::slice::from_raw_parts(devices, count) },
        };
        shell::Cast {
            header: string(header),
            devices: (devices.iter())
                .map(|device| shell::CastDevice {
                    id: string(device.id),
                    name: string(device.name),
                    state: match device.state as u32 {
                        bc::BC_CAST_AVAILABLE => shell::CastState::Available,
                        bc::BC_CAST_CONNECTING => shell::CastState::Connecting,
                        bc::BC_CAST_CONNECTED => shell::CastState::Connected,
                        bc::BC_CAST_DISCONNECTING => shell::CastState::Disconnecting,
                        _ => shell::CastState::Unavailable,
                    },
                    status: string(device.status),
                })
                .collect(),
        }
    });
    // check.sh waits for these lines.
    match &cast {
        Some(cast) => {
            let devices: Vec<String> = (cast.devices.iter())
                .map(|device| format!("{} {:?}", device.name, device.state))
                .collect();
            log!(
                "browser: cast {tab} {} devices {}",
                cast.header,
                devices.len()
            );
            for device in devices {
                log!("browser: cast {tab} device {device}");
            }
        }
        None => log!("browser: cast {tab} closed"),
    }
    update_shells(|shell, cx| shell.set_cast(tab, cast.clone(), cx));
}

/// What a tab's page asks a device for, as the core finds them, or that the request ended (a null
/// site), for the window showing the tab.
pub(crate) unsafe extern "C" fn tab_chooser(
    _: *mut c_void,
    tab: bc::bc_id,
    site: *const c_char,
    kind: i32,
    devices: *const bc::bc_chooser_device,
    count: usize,
    scan: i32,
) {
    let string = |text: *const c_char| -> SharedString {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
            .into()
    };
    let chooser = (!site.is_null()).then(|| {
        let devices = match count {
            0 => &[][..],
            _ => unsafe { std::slice::from_raw_parts(devices, count) },
        };
        shell::Chooser {
            site: string(site),
            kind: match kind as u32 {
                bc::BC_DEVICE_SERIAL => shell::DeviceKind::Serial,
                bc::BC_DEVICE_HID => shell::DeviceKind::Hid,
                bc::BC_DEVICE_BLUETOOTH => shell::DeviceKind::Bluetooth,
                _ => shell::DeviceKind::Usb,
            },
            devices: (devices.iter())
                .map(|device| shell::ChooserDevice {
                    id: string(device.id),
                    name: string(device.name),
                    detail: string(device.detail),
                })
                .collect(),
            scan: match scan as u32 {
                bc::BC_SCAN_SCANNING => shell::Scan::Scanning,
                bc::BC_SCAN_STOPPED => shell::Scan::Stopped,
                bc::BC_SCAN_OFF => shell::Scan::Off,
                bc::BC_SCAN_DENIED => shell::Scan::Denied,
                _ => shell::Scan::Live,
            },
        }
    });
    // check.sh waits for these lines.
    match &chooser {
        Some(chooser) => {
            log!(
                "browser: chooser {tab} {} {:?} {:?} devices {}",
                chooser.site,
                chooser.kind,
                chooser.scan,
                chooser.devices.len()
            );
            for device in &chooser.devices {
                log!(
                    "browser: chooser {tab} device {} ({})",
                    device.name,
                    device.detail
                );
            }
        }
        None => log!("browser: chooser {tab} ended"),
    }
    update_shells(|shell, cx| shell.set_chooser(tab, chooser.clone(), cx));
}

/// What a Bluetooth device a tab's page pairs with asks, or that it ended (a null device), for the
/// window showing the tab.
pub(crate) unsafe extern "C" fn tab_pair(
    _: *mut c_void,
    tab: bc::bc_id,
    device: *const c_char,
    kind: i32,
    passkey: *const c_char,
) {
    let string = |text: *const c_char| -> SharedString {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
            .into()
    };
    let pair = (!device.is_null()).then(|| shell::Pair {
        device: string(device),
        kind: match kind as u32 {
            bc::BC_PAIR_PIN => shell::PairKind::Pin,
            bc::BC_PAIR_PASSKEY if !passkey.is_null() => shell::PairKind::Passkey(string(passkey)),
            _ => shell::PairKind::Confirm,
        },
    });
    // check.sh waits for these lines.
    match &pair {
        Some(pair) => log!("browser: pair {tab} {} {:?}", pair.device, pair.kind),
        None => log!("browser: pair {tab} ended"),
    }
    update_shells(|shell, cx| shell.set_pair(tab, pair.clone(), cx));
}

/// The user right-clicked a page: its menu, as the core has Chrome build it, in ilo's menu over the
/// window that shows it. With no window, the menu went with its page.
pub(crate) unsafe extern "C" fn tab_page_menu(
    _: *mut c_void,
    window: bc::bc_id,
    tab: bc::bc_id,
    x: f64,
    y: f64,
    items: *const bc::bc_page_menu_item,
    count: usize,
) {
    if window == 0 {
        log!("browser: page menu {tab} gone");
        update_app(menu::dismissed);
        return;
    }
    let items = match count {
        0 => &[][..],
        _ => unsafe { std::slice::from_raw_parts(items, count) },
    };
    let items: Vec<shell::PageMenuItem> = (items.iter())
        .map(|item| shell::PageMenuItem {
            command: item.command,
            title: unsafe { CStr::from_ptr(item.title) }
                .to_string_lossy()
                .into_owned()
                .into(),
            parent: usize::try_from(item.parent).ok(),
            enabled: item.enabled,
            checked: item.checked,
        })
        .collect();
    log!("browser: page menu {tab} at {x:.0},{y:.0}");
    let at = point(px(x as f32), px(y as f32));
    if update_window(window, |shell, cx| shell.show_page_menu(tab, items, at, cx)).is_none() {
        // No window to show it in, and the page opens no other menu until this one closes.
        unsafe { bc::bc_tab_close_page_menu(tab, 0) };
    }
}

pub(crate) unsafe extern "C" fn tab_media_unsupported(
    _: *mut c_void,
    tab: bc::bc_id,
    key_system: *const c_char,
) {
    let key_system = unsafe { CStr::from_ptr(key_system) }
        .to_string_lossy()
        .into_owned();
    // check.sh reads these lines.
    match key_system.is_empty() {
        true => log!("browser: page {tab} needs nothing"),
        false => log!("browser: page {tab} needs {key_system}"),
    }
    update_shells(|shell, cx| shell.set_media_unsupported(tab, &key_system, cx));
}

/// Google's Widevine module moved along: the notes that offer it and Settings' row say where to.
/// Later, as the user's yes in a note or Settings calls back from inside their update.
pub(crate) unsafe extern "C" fn widevine_changed(_: *mut c_void, state: c_int) {
    // check/widevine.sh reads this line.
    log!(
        "browser: widevine state {}",
        Widevine::from_core(state).name()
    );
    update_app_later(|cx| {
        let shells: Vec<_> = WINDOWS
            .with_borrow(|windows| windows.iter().map(|window| window.shell.clone()).collect());
        for shell in shells {
            shell.update(cx, |_, cx| cx.notify()).ok();
        }
        settings::check_widevine(cx);
    });
}

pub(crate) unsafe extern "C" fn window_suggestions(
    _: *mut c_void,
    window: bc::bc_id,
    text: *const c_char,
    suggestions: *const bc::bc_suggestion,
    count: usize,
) {
    // The command bar shows over the window last key.
    if window != front().0 {
        return;
    }
    let string = |text: *const c_char| {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
    };
    let suggestions = match count {
        0 => &[][..],
        _ => unsafe { std::slice::from_raw_parts(suggestions, count) },
    };
    let rows = suggestions
        .iter()
        .map(|suggestion| command_bar::Suggestion {
            url: string(suggestion.url),
            text: string(suggestion.text).into(),
            detail: string(suggestion.detail).into(),
            tab: suggestion.tab,
            completion: string(suggestion.completion),
            removable: suggestion.removable,
            keyword: string(suggestion.keyword),
            site: string(suggestion.site).into(),
            ..command_bar::Suggestion::default()
        })
        .collect();
    let text = string(text);
    update_app(|cx| command_bar::suggested(text, rows, cx));
}
