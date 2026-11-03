//! Extensions: their actions, questions, side panels and popups.

use super::*;

/// Where Add Extension… goes, as Dia's does.
pub(crate) const WEB_STORE: &str = "https://chromewebstore.google.com/";

pub(crate) unsafe extern "C" fn extensions_changed(
    _: *mut c_void,
    profile: bc::bc_id,
    extensions: *const bc::bc_extension,
    count: usize,
) {
    let extensions = match extensions.is_null() {
        true => &[][..],
        false => unsafe { std::slice::from_raw_parts(extensions, count) },
    };
    let text = |text| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned().into();
    let extensions: Vec<_> = extensions
        .iter()
        .map(|extension| Extension {
            id: text(extension.id),
            name: text(extension.name),
            enabled: extension.enabled,
            pinned: extension.pinned,
            options: text(extension.options),
            removable: extension.removable,
            incognito: extension.incognito,
            crashed: extension.crashed,
            commands: match extension.commands.is_null() {
                true => &[][..],
                false => unsafe {
                    std::slice::from_raw_parts(extension.commands, extension.command_count)
                },
            }
            .iter()
            .map(|command| ExtensionCommand {
                name: text(command.name),
                description: text(command.description),
                keystroke: keystroke(&bc::bc_key {
                    key: command.key,
                    modifiers: command.modifiers,
                    editing: false,
                    latin_key: std::ptr::null(),
                })
                .into(),
                suggested: command.suggested,
            })
            .collect(),
        })
        .collect();
    update_shells(|shell, cx| shell.set_extensions(profile, extensions.clone(), cx));
    // Over the window in front, once.
    update_window(front().0, |shell, cx| shell.tell_crashed_extension(profile, cx));
}

pub(crate) unsafe extern "C" fn window_extension_actions(
    _: *mut c_void,
    window: bc::bc_id,
    actions: *const bc::bc_extension_action,
    count: usize,
) {
    let actions = match actions.is_null() {
        true => &[][..],
        false => unsafe { std::slice::from_raw_parts(actions, count) },
    };
    let text = |text| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned().into();
    let actions = actions
        .iter()
        .map(|action| ExtensionAction {
            id: text(action.id),
            title: text(action.title),
            icon: (!action.icon.is_null()).then(|| {
                let png = unsafe { std::slice::from_raw_parts(action.icon, action.icon_size) };
                Arc::new(Image::from_bytes(ImageFormat::Png, png.to_vec()))
            }),
            badge: text(action.badge),
            badge_color: action.badge_color,
            enabled: action.enabled,
            pinned: action.pinned,
        })
        .collect();
    update_window(window, |shell, cx| shell.set_extension_actions(actions, cx));
}

pub(crate) unsafe extern "C" fn window_extension_question(
    _: *mut c_void,
    window: bc::bc_id,
    question: *const bc::bc_extension_question,
) {
    let Some(question) = (unsafe { question.as_ref() }) else {
        if window == front().0 {
            // check.sh waits for this line.
            log!("browser: extension question none");
            update_app(dialog::withdraw_chrome);
        }
        return;
    };
    let id = question.id;
    let text = |text| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();
    let lines = |texts: *const *const c_char| match texts.is_null() {
        true => Vec::new(),
        false => unsafe { std::slice::from_raw_parts(texts, question.warning_count) }
            .iter()
            .map(|&line| text(line))
            .collect(),
    };
    let warnings = lines(question.warnings);
    // check.sh waits for this line.
    log!("browser: extension question {} asks {}", text(question.extension), warnings.join("; "));
    let list: Vec<_> = warnings
        .into_iter()
        .zip(lines(question.details))
        .map(|(warning, details)| {
            (warning.into(), details.lines().map(|line| line.to_owned().into()).collect())
        })
        .collect();
    let question = dialog::Question {
        icon: (!question.icon.is_null()).then(|| {
            let png = unsafe { std::slice::from_raw_parts(question.icon, question.icon_size) };
            Arc::new(Image::from_bytes(ImageFormat::Png, png.to_vec()))
        }),
        message: text(question.title).into(),
        // Chrome's heading, such as "It can:", leads its warnings, and there's none without them.
        detail: if list.is_empty() { "".into() } else { text(question.heading).into() },
        list,
        action: text(question.accept).into(),
        cancel: text(question.cancel).into(),
        tone: dialog::Tone::Accent,
        also: None,
        fields: Vec::new(),
        check: None,
        on_return: dialog::Return::Action,
    };
    let mut asked = false;
    if window == front().0 {
        update_app(|cx| {
            asked = true;
            let answered = Box::new(move |accept, _: &mut App| unsafe {
                bc::bc_extension_answer(id, accept)
            });
            dialog::ask_chrome(question, answered, cx);
        });
    }
    // With no UI to ask in, Chrome isn't left waiting.
    if !asked {
        unsafe { bc::bc_extension_answer(id, false) };
    }
}

pub(crate) unsafe extern "C" fn window_side_panel(
    _: *mut c_void,
    window: bc::bc_id,
    panel: *const bc::bc_side_panel,
) {
    let text = |text| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned().into();
    let panel = unsafe { panel.as_ref() }.map(|panel| SidePanel {
        extension: text(panel.extension),
        name: text(panel.name),
        icon: (!panel.icon.is_null()).then(|| {
            let png = unsafe { std::slice::from_raw_parts(panel.icon, panel.icon_size) };
            Arc::new(Image::from_bytes(ImageFormat::Png, png.to_vec()))
        }),
        title: text(panel.title),
        width: panel.width as f32,
    });
    update_window(window, |shell, cx| shell.set_side_panel(panel, cx));
}

pub(crate) unsafe extern "C" fn window_extension_popup(
    _: *mut c_void,
    window: bc::bc_id,
    extension: *const c_char,
) {
    let extension = unsafe { CStr::from_ptr(extension) }.to_string_lossy();
    if WINDOWS.with_borrow(|windows| windows.iter().any(|open| open.id == window)) {
        // check.sh waits for these lines.
        match extension.is_empty() {
            true => log!("browser: extension popup none"),
            false => log!("browser: extension popup {extension}"),
        }
    }
}
