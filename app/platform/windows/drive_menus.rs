//! The app's menus and dialog for the Windows driver (drive.cc), which runs its menu and menubar
//! commands through them as the Mac's runs them through its menu bar, and its choose in the dialog
//! as Linux's does. Windows shows no menu bar, so an item is run in the driven window as AppKit's
//! menu bar runs it in the key one.

use std::ffi::{CStr, c_char, c_void};

use browser::log;
use gpui::{Menu, MenuItem};

use crate::{CX, MENUS, WINDOWS};

/// Chooses the item titled `title`, or with `bar` logs the items of the menu so titled, in the
/// browser window whose UI view is `ui_view`. False if there's none.
pub extern "C" fn run(ui_view: *mut c_void, title: *const c_char, bar: bool) -> bool {
    // SAFETY: the driver passes a C string it holds for the call.
    let Ok(title) = unsafe { CStr::from_ptr(title) }.to_str() else {
        return false;
    };
    let menus = MENUS.with_borrow(|menus| menus.build());
    if bar {
        let Some(menu) = menus.iter().find(|menu| menu.name.as_ref() == title) else {
            return false;
        };
        log!("browser: menu bar {title}: {}", titles(menu).join(", "));
        return true;
    }
    let Some(MenuItem::Action { action, .. }) = item(&menus, title) else {
        return false;
    };
    let Some(handle) = WINDOWS.with_borrow(|windows| {
        windows
            .iter()
            .find(|window| window.ui_view == ui_view)
            .map(|window| window.handle)
    }) else {
        return false;
    };
    let Some(cx) = CX.with_borrow(Clone::clone) else {
        return false;
    };
    cx.update(|cx| {
        handle.update(cx, |_, window, cx| {
            window.dispatch_action(action.boxed_clone(), cx)
        })
    })
    .is_ok()
}

/// Answers with the button `label` names in the app's dialog that shows. False if none shows.
pub extern "C" fn choose(label: *const c_char) -> bool {
    // SAFETY: the driver passes a C string it holds for the call.
    let Ok(label) = unsafe { CStr::from_ptr(label) }.to_str() else {
        return false;
    };
    CX.with_borrow(Clone::clone)
        .is_some_and(|cx| cx.update(|cx| browser::dialog::choose(label, cx)))
}

/// `menu`'s items as drive.mm's menubar logs them: a submenu's as "Submenu › Item", and each
/// "(checked)" and "(disabled)" as it is.
fn titles(menu: &Menu) -> Vec<String> {
    let mut titles = Vec::new();
    let mut add = |item: &MenuItem, prefix: &str| {
        if let MenuItem::Action {
            name,
            checked,
            disabled,
            ..
        } = item
        {
            let mut title = format!("{prefix}{name}");
            title += if *checked { " (checked)" } else { "" };
            title += if *disabled { " (disabled)" } else { "" };
            titles.push(title);
        }
    };
    for item in &menu.items {
        match item {
            MenuItem::Submenu(sub) => {
                let prefix = format!("{} › ", sub.name);
                sub.items.iter().for_each(|inner| add(inner, &prefix));
            }
            _ => add(item, ""),
        }
    }
    titles
}

/// The item of `menus` titled `title`, in a submenu too, or with "Submenu › Item", the item in the
/// submenu so titled.
fn item<'a>(menus: &'a [Menu], title: &str) -> Option<&'a MenuItem> {
    let (parent, title) = title
        .split_once(" › ")
        .map_or((None, title), |(parent, item)| (Some(parent), item));
    menus
        .iter()
        .find_map(|menu| find(&menu.items, parent, title, parent.is_none()))
}

fn find<'a>(
    items: &'a [MenuItem],
    parent: Option<&str>,
    title: &str,
    inside: bool,
) -> Option<&'a MenuItem> {
    items.iter().find_map(|item| match item {
        MenuItem::Action { name, .. } if inside && name.as_ref() == title => Some(item),
        MenuItem::Submenu(menu) => {
            let inside = inside || parent == Some(menu.name.as_ref());
            find(&menu.items, parent, title, inside)
        }
        _ => None,
    })
}
