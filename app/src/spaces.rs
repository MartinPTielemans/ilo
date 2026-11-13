//! What the core says of the Spaces, their favorites, the archive and the sidebar's backups.

use super::*;

/// Saves the Spaces' favorites and pinned tabs as a Markdown file of links where the user picks,
/// for Settings' Export…. The user's alone, as Settings is.
pub(crate) fn export_spaces(cx: &mut App) {
    if let Some(shell) = shell_with_view(front().1).upgrade() {
        shell.update(cx, |shell, cx| shell.export_spaces(cx));
    }
}

pub(crate) unsafe extern "C" fn spaces_changed(_: *mut c_void) {
    let profiles: Vec<_> = ids(bc::bc_profiles)
        .into_iter()
        .map(|profile| (profile, name(bc::bc_profile_name, profile)))
        .collect();
    let spaces: Vec<_> = ids(bc::bc_spaces)
        .into_iter()
        .map(|space| {
            let mut color = 0;
            let has_color = unsafe { bc::bc_space_color(space, &mut color) };
            let profile = unsafe { bc::bc_space_profile(space) };
            let name_of = name(bc::bc_space_name, space);
            let icon = name(bc::bc_space_icon, space);
            let agent = name(bc::bc_space_agent_name, space);
            let theme = name(bc::bc_space_theme, space);
            (
                space,
                name_of,
                has_color.then_some(color),
                profile,
                icon,
                agent,
                theme,
            )
        })
        .collect();
    let named: Vec<_> = profiles
        .iter()
        .map(|(id, name)| (*id, name.to_string()))
        .collect();
    little_sites_host::profiles_changed(&named);
    bar_picks_host::profiles_changed(&named);
    boosts_host::profiles_changed(&named);
    update_shells(|shell, cx| {
        shell.set_profiles(profiles.clone(), cx);
        shell.set_spaces(spaces.clone(), cx);
    });
    update_app(settings::refresh);
    // The rules name their Spaces by place, which a Space moved or deleted changes.
    link_rules_host::save();
    // After the update under way, as an import waiting here makes Spaces of its own.
    update_app_later(import_host::spaces_changed);
}

pub(crate) unsafe extern "C" fn archive_note_changed(_: *mut c_void) {
    // Before the toast's window is made, as at launch, making it shows the note.
    update_app_later(show_archive_note);
}

/// The note on the first tabs auto-archive took, as Arc's banner explains its own, while it's still
/// to be answered: a toast that stays until one of its buttons is pressed, as the crash report's
/// offer does, so it's never missed. Both buttons dismiss it for good.
pub(crate) fn show_archive_note(cx: &mut App) {
    if !unsafe { bc::bc_archive_note() } {
        return;
    }
    let view = toast::Button {
        label: toast::Label::Text("View Archive".into()),
        name: "view archive",
        run: Rc::new(|_, cx| {
            unsafe { bc::bc_dismiss_archive_note() };
            let shell = WINDOWS.with_borrow(|windows| windows.first().map(|w| w.shell.clone()));
            if let Some(shell) = shell {
                let view = |shell: &mut Shell, cx: &mut Context<Shell>| {
                    shell.run(Command::ViewArchive, "archive note", cx)
                };
                shell.update(cx, view).ok();
            }
        }),
    };
    let dismiss = toast::Button {
        label: toast::Label::Glyph("icons/x.svg"),
        name: "dismiss",
        run: Rc::new(|_, _| unsafe { bc::bc_dismiss_archive_note() }),
    };
    let text = format!(
        "Tabs unseen for {} went to the archive.",
        settings::archive_after_words()
    );
    // Launch shows it once the toast's window is made, and the core's change can ask again.
    if toast::showing(&text, cx) {
        return;
    }
    toast::show_until_answered(&text, vec![view, dismiss], cx);
}

pub(crate) unsafe extern "C" fn favorites_changed(_: *mut c_void) {
    // Each with its holder: the Space it belongs to under Each Space, else its profile.
    let holder = |tab| unsafe {
        match bc::bc_favorite_space(tab) {
            0 => bc::bc_tab_profile(tab),
            space => space,
        }
    };
    let tabs: Vec<_> = ids(bc::bc_favorites)
        .into_iter()
        .map(|tab| (tab, holder(tab)))
        .collect();
    // Those that are split views, each one favorite, and the tab each shows focused.
    let mut splits = Vec::new();
    let mut focus = HashMap::new();
    for &(tab, _) in &tabs {
        let split = split_of(tab);
        if split.tabs.len() > 1 && split.tabs[0] == tab {
            focus.insert(tab, unsafe { bc::bc_tab_split_focused(tab) });
            splits.push(split);
        }
    }
    update_shells(|shell, cx| {
        shell.set_split_favorites(tabs.clone(), splits.clone(), focus.clone(), cx)
    });
    update_app(settings::refresh);
}

/// `tab`'s split view as the core has it, else `tab` alone.
fn split_of(tab: bc::bc_id) -> Split {
    // The core's splits have up to three tabs.
    let mut split = [0; 3];
    let count = unsafe { bc::bc_tab_split(tab, split.as_mut_ptr(), split.len()) };
    let tabs = split[..count.min(split.len())].to_vec();
    if tabs.len() < 2 {
        return Split {
            tabs: vec![tab],
            ..Default::default()
        };
    }
    let shares: Vec<f64> = tabs
        .iter()
        .map(|&tab| unsafe { bc::bc_tab_split_share(tab) })
        .collect();
    // Even shares are the core's integer split, which the shell draws the same way.
    let even = shares.iter().all(|&share| share == shares[0]);
    Split {
        stacked: unsafe { bc::bc_tab_split_stacked(tab) },
        shares: if even { Vec::new() } else { shares },
        expanded: unsafe { bc::bc_tab_split_expanded(tab) },
        name: name(bc::bc_tab_split_name, tab),
        icon: name(bc::bc_tab_split_icon, tab),
        tabs,
    }
}

pub(crate) unsafe extern "C" fn space_changed(_: *mut c_void, space: bc::bc_id) {
    let list = |copy: unsafe extern "C" fn(bc::bc_id, *mut bc::bc_id, usize) -> usize| {
        let count = unsafe { copy(space, std::ptr::null_mut(), 0) };
        let mut tabs = vec![0; count];
        let count = unsafe { copy(space, tabs.as_mut_ptr(), tabs.len()) };
        tabs.truncate(count);
        tabs
    };
    let (pinned, today) = (list(bc::bc_space_pinned), list(bc::bc_space_today));
    let mut splits = Vec::new();
    for &tab in pinned.iter().chain(&today) {
        let split = split_of(tab);
        if split.tabs.len() > 1 && split.tabs[0] == tab {
            splits.push(split);
        }
    }
    let pinned_shut = unsafe { !bc::bc_space_pinned_open(space) };
    let mut folders = Folders {
        pinned_shut,
        ..Default::default()
    };
    for &row in &pinned {
        let mut open = false;
        if unsafe { bc::bc_folder(row, &mut open) } {
            folders
                .folders
                .insert(row, (name(bc::bc_folder_name, row), open));
            let icon = name(bc::bc_folder_icon, row);
            if !icon.is_empty() {
                folders.icons.insert(row, icon);
            }
        }
        match unsafe { bc::bc_folder_of(row) } {
            0 => {}
            folder => _ = folders.parents.insert(row, folder),
        }
    }
    let Some(cx) = CX.with_borrow(Clone::clone) else {
        return;
    };
    cx.update(|cx| {
        update_shells_in(cx, |shell, cx| {
            let (pinned, today) = (pinned.clone(), today.clone());
            shell.set_tabs(space, pinned, today, splits.clone(), folders.clone(), cx)
        });
        // A tab the user held from an agent that left its Space ends its hold.
        browser::agents::space_changed(space, cx);
    });
}

/// A private window's name, in its sidebar and the Window menu: Chrome's, and its profile's while
/// there's more than one, as each has its own private session.
pub(crate) fn private_name(profile: bc::bc_id) -> SharedString {
    match ids(bc::bc_profiles).len() {
        0 | 1 => "Incognito".into(),
        _ => format!("Incognito · {}", name(bc::bc_profile_name, profile)).into(),
    }
}

pub(crate) unsafe extern "C" fn window_space_shown(
    _: *mut c_void,
    window: bc::bc_id,
    space: bc::bc_id,
) {
    // check.sh reads this line, as a relaunch brings back each window's Space.
    log!(
        "browser: window {window} in space {}",
        name(bc::bc_space_name, space)
    );
    if name(bc::bc_space_agent, space).is_empty() {
        WINDOWS.with_borrow_mut(|windows| {
            if let Some(window) = windows.iter_mut().find(|w| w.id == window) {
                window.users_space = space;
            }
        });
    }
    update_window(window, |shell, cx| shell.set_space(space, cx));
}

pub(crate) unsafe extern "C" fn archive_changed(
    _: *mut c_void,
    tabs: *const bc::bc_archived_tab,
    count: usize,
) {
    let tabs = match tabs.is_null() {
        true => &[][..],
        false => unsafe { std::slice::from_raw_parts(tabs, count) },
    };
    let text = |text| {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
            .into()
    };
    let archive: Vec<_> = tabs
        .iter()
        .map(|tab| Archived {
            id: tab.id,
            url: text(tab.url),
            title: text(tab.title),
            archived: tab.archived,
            little: tab.little,
        })
        .collect();
    // check.sh waits for this line.
    log!("browser: archive {count} tabs");
    update_shells(|shell, cx| shell.set_archive(archive.clone(), cx));
}

pub(crate) unsafe extern "C" fn sidebar_backups_changed(
    _: *mut c_void,
    profile: bc::bc_id,
    backups: *const bc::bc_sidebar_backup,
    count: usize,
) {
    let backups = match backups.is_null() {
        true => &[][..],
        false => unsafe { std::slice::from_raw_parts(backups, count) },
    };
    let backups: Vec<_> = (backups.iter())
        .map(|backup| shell::SidebarBackup {
            time: backup.time as u64,
            label: unsafe { CStr::from_ptr(backup.label) }
                .to_string_lossy()
                .into_owned()
                .into(),
        })
        .collect();
    update_shells(|shell, cx| shell.set_sidebar_backups(profile, backups.clone(), cx));
}
