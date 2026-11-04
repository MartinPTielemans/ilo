//! The menu bar and its menus.

use super::*;

actions!(browser, [Hide, HideOthers, ShowAll, Minimize]);

actions!(browser, [CheckForUpdates]);

/// What the menu bar lists besides what never changes: what the key window's shell says of it, and
/// the titles Undo and Redo have for what the key window would undo and redo.
#[derive(Default)]
pub(crate) struct Menus {
    pub(crate) bar: shell::MenuBar,
    pub(crate) undo: Option<String>,
    pub(crate) redo: Option<String>,
    /// Check for Updates… reads Restart to Update once an update is ready.
    pub(crate) update_ready: bool,
}

impl Menus {
    pub(crate) fn build(&self) -> Vec<Menu> {
        menus(
            &self.bar,
            self.undo.as_deref(),
            self.redo.as_deref(),
            self.update_ready,
        )
    }
}

/// Puts up the menu bar built from MENUS, or, while a menu is open, once none is, as AppKit can
/// crash when the main menu is replaced under an open menu. What changes meanwhile goes in the one
/// rebuild.
pub(crate) fn show_menus(cx: &mut App) {
    if MENUS_HELD.get() {
        return;
    }
    if unsafe { platform::host_after_menus(menus_closed) } {
        // check.sh reads these lines.
        log!("browser: menu bar waits for the menu open");
        MENUS_HELD.set(true);
        return;
    }
    cx.set_menus(MENUS.with_borrow(Menus::build));
}

pub(crate) extern "C" fn menus_closed() {
    log!("browser: menu bar put up once the menus closed");
    MENUS_HELD.set(false);
    update_app(show_menus);
}

/// "Undo" or "Redo", with what it would undo or redo after it.
pub(crate) fn edit_title(verb: &str, what: Option<&str>) -> String {
    match what {
        Some(what) => format!("{verb} {what}"),
        None => verb.to_string(),
    }
}

/// The menu bar: Arc's menus, with the commands the browser has, what `bar` says of the key
/// window, its Spaces, its Space's profile's extensions and the titles that follow its state, and
/// the titles Undo and Redo have for what it would undo and redo (docs/ux-spec.md › Menus). The
/// Edit menu's standard items go to the first responder, so a focused page cuts, copies, pastes,
/// selects and undoes for itself.
pub(crate) fn menus(
    bar: &shell::MenuBar,
    undo: Option<&str>,
    redo: Option<&str>,
    update_ready: bool,
) -> Vec<Menu> {
    let menu = |name: &str, items| Menu {
        name: name.to_string().into(),
        items,
        disabled: false,
    };
    let item = shell::menu_item;
    let (spaces, extensions) = (&bar.spaces, &bar.extensions);
    let about = item(concat!("About ", env!("APP_NAME")), Command::About);
    let mut app = vec![about, MenuItem::separator()];
    // Only a release build that carries Sparkle has updates to check for.
    if updates_available() {
        let title = updates::title(update_ready);
        app.extend([
            MenuItem::action(title, CheckForUpdates),
            MenuItem::separator(),
        ]);
    }
    app.extend([
        item("Settings…", Command::Settings),
        item("Import from Other Browsers…", Command::Import),
        item("Privacy Policy", Command::PrivacyPolicy),
        MenuItem::separator(),
        MenuItem::os_submenu("Services", SystemMenuType::Services),
        MenuItem::separator(),
        MenuItem::action(concat!("Hide ", env!("APP_NAME")), Hide),
        MenuItem::action("Hide Others", HideOthers),
        MenuItem::action("Show All", ShowAll),
        MenuItem::separator(),
        item(concat!("Quit ", env!("APP_NAME")), Command::Quit),
    ]);
    vec![
        menu(env!("APP_NAME"), app),
        menu(
            "File",
            vec![
                item("New Tab…", Command::NewTab),
                item("New Window", Command::NewWindow),
                item("New Little ilo Window", Command::NewLittleWindow),
                item("New Incognito Window", Command::NewPrivateWindow),
                item("Restore Last Closed Tab", Command::ReopenClosedTab),
                MenuItem::separator(),
                item("Open Command Bar", Command::EditUrl),
                MenuItem::separator(),
                item("Close Window", Command::CloseWindow),
                item("Archive Tab", Command::CloseTab),
                MenuItem::separator(),
                item("Share…", Command::Share),
                item("Capture…", Command::CapturePortion),
                item("Capture Full Page", Command::CaptureFullPage),
                item("Save Page As", Command::SavePage),
                item("Print", Command::Print),
            ],
        ),
        menu(
            "Edit",
            vec![
                MenuItem::os_action(edit_title("Undo", undo), Undo, OsAction::Undo),
                MenuItem::os_action(edit_title("Redo", redo), Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action("Cut", Cut, OsAction::Cut),
                MenuItem::os_action("Copy", Copy, OsAction::Copy),
                item("Copy URL", Command::CopyUrl),
                item("Copy URL as Markdown", Command::CopyMarkdownLink),
                MenuItem::os_action("Paste", Paste, OsAction::Paste),
                MenuItem::os_action(
                    "Paste and Match Style",
                    PasteAndMatchStyle,
                    OsAction::PasteAndMatchStyle,
                ),
                item("Paste URL as New Tab", Command::PasteUrlAsTab),
                MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
                MenuItem::separator(),
                MenuItem::submenu(menu(
                    "Find",
                    vec![
                        item("Find…", Command::Find),
                        item("Find Next", Command::FindNext),
                        item("Find Previous", Command::FindPrevious),
                        item("Jump to Selection", Command::JumpToSelection),
                    ],
                )),
                // Where AppKit puts the system's, which ilo's replaces (platform/mac/host.mm).
                MenuItem::separator(),
                MenuItem::submenu(menu(
                    "AutoFill",
                    vec![
                        item("Contact…", Command::AutoFill),
                        item("Passwords…", Command::AutoFillPasswords),
                        item("Credit Card…", Command::AutoFill),
                    ],
                )),
            ],
        ),
        menu(
            "View",
            vec![
                MenuItem::submenu(menu(
                    "Appearance",
                    (Appearance::ALL.into_iter())
                        .map(|each| {
                            item(appearance_title(each), Command::Appearance(each))
                                .checked(bar.appearance == each)
                        })
                        .collect(),
                )),
                MenuItem::separator(),
                item(bar.sidebar_title(), Command::ToggleSidebar),
                item(bar.pinned_item().0, bar.pinned_item().1).disabled(!bar.pinned_offered),
                MenuItem::separator(),
                item("Stop Loading", Command::Stop),
                item("Refresh the Page", Command::Reload),
                item("Force Refresh the Page", Command::ForceReload),
                item("Clear Cookies and Refresh", Command::ClearCookiesAndRefresh),
                item("Clear Cache and Refresh", Command::ClearCacheAndRefresh),
                MenuItem::separator(),
                item("Add Split View", Command::AddSplit),
                item("Close this Split Pane", Command::CloseSplitPane),
                item("Separate Page from Split View", Command::SeparatePane),
                MenuItem::separator(),
                item("Zoom to Actual Size", Command::ActualSize),
                item("Zoom In", Command::ZoomIn),
                item("Zoom Out", Command::ZoomOut),
                MenuItem::separator(),
                item("Cast…", Command::Cast),
                MenuItem::separator(),
                MenuItem::submenu(menu(
                    "Developer",
                    vec![
                        item("View Source", Command::ViewSource),
                        item("Developer Tools", Command::DevTools),
                        item("Inspect Elements", Command::InspectElements),
                        item("JavaScript Console", Command::JavaScriptConsole),
                    ]
                    .into_iter()
                    .chain(bar.developer_mode_title().into_iter().flat_map(|title| {
                        [
                            MenuItem::separator(),
                            item(title, Command::ToggleDeveloperMode),
                        ]
                    }))
                    .collect(),
                )),
                MenuItem::separator(),
                item("Expand Current Split", Command::ExpandSplit),
            ],
        ),
        menu(
            "Spaces",
            [
                item("New Space…", Command::NewSpace),
                item("Rename Space", Command::RenameSpace),
            ]
            .into_iter()
            .chain(bar.profiles.as_ref().map(change_profile))
            .chain([
                MenuItem::separator(),
                item("Next Space", Command::NextSpace),
                item("Previous Space", Command::PreviousSpace),
            ])
            .chain((!spaces.is_empty()).then(MenuItem::separator))
            .chain(
                spaces.iter().enumerate().map(|(index, name)| {
                    shell::menu_item(name.clone(), Command::SelectSpace(index))
                }),
            )
            .collect(),
        ),
        menu(
            "Tabs",
            vec![
                item("New Tab…", Command::NewTab),
                item("Search Tabs…", Command::SearchTabs),
                item(bar.pin_title(), Command::TogglePin),
                item("New Folder…", Command::NewFolder),
                MenuItem::separator(),
                item("Next Tab", Command::NextTab),
                item("Previous Tab", Command::PreviousTab),
                item("Reveal Tab in Sidebar", Command::RevealTab),
                item("Focus Sidebar", Command::FocusSidebar),
                MenuItem::separator(),
                item("Clear Today", Command::Clear),
                item("Reset all tabs in this Space", Command::ResetSpaceTabs),
            ],
        ),
        menu(
            "Archive",
            vec![
                item("Go Back", Command::Back),
                item("Go Forward", Command::Forward),
                MenuItem::separator(),
                item("View History", Command::ViewHistory),
                item("View Archive…", Command::ViewArchive),
                item("Clear Archive", Command::ClearArchive),
                MenuItem::separator(),
                item("Clear Browsing Data…", Command::ClearBrowsingData),
            ],
        ),
        menu(
            "Extensions",
            (extensions.iter().map(shell::extension_menu_item))
                .chain((!extensions.is_empty()).then(MenuItem::separator))
                .chain((!extensions.is_empty()).then(|| {
                    let pins = extensions
                        .iter()
                        .map(shell::extension_pin_menu_item)
                        .collect();
                    MenuItem::submenu(menu("Pin to URL Field", pins))
                }))
                .chain([
                    item("Add Extension…", Command::AddExtension),
                    item("Manage Extensions…", Command::ManageExtensions),
                ])
                .collect(),
        ),
        menu(
            "Window",
            vec![
                MenuItem::action("Minimize Window", Minimize),
                stay_on_top(bar.on_top),
                MenuItem::separator(),
                item("Open Library…", Command::ViewLibrary),
                item("View Downloads…", Command::ViewDownloads),
                item("View Media…", Command::ViewMedia),
            ],
        ),
        menu(
            "Help",
            vec![
                item(concat!("Welcome to ", env!("APP_NAME")), Command::Welcome),
                item("Help Center", Command::HelpCenter),
                item("What's New", Command::WhatsNew),
                item("Essential Keyboard Shortcuts", Command::KeyboardShortcuts),
                MenuItem::separator(),
                item("Report a Problem…", Command::ReportProblem),
                restore_sidebar(&bar.sidebar_backups),
                MenuItem::submenu(menu(
                    "Troubleshooting",
                    vec![
                        item("Copy Build Info", Command::CopyBuildInfo),
                        item("Open Task Manager", Command::TaskManager),
                        item(bar.trace_title(), Command::RecordTrace),
                    ],
                )),
            ],
        ),
    ]
}

/// View › Appearance's item for `appearance`, as Arc's names them.
/// Window › Stay On Top, ticked while it's on, where the window can stay on top at all.
fn stay_on_top(on: Option<bool>) -> MenuItem {
    match on {
        Some(on) => shell::menu_item(shell::STAY_ON_TOP, Command::StayOnTop).checked(on),
        None => shell::menu_item(shell::STAY_ON_TOP, Command::StayOnTop).disabled(true),
    }
}

pub(crate) fn appearance_title(appearance: Appearance) -> &'static str {
    match appearance {
        Appearance::Automatic => "Automatic",
        Appearance::Light => "Light",
        Appearance::Dark => "Dark",
    }
}

/// Gives the host View › Appearance's choice, for the app's native parts and its pages.
pub(crate) fn give_host_appearance(appearance: Appearance) {
    let code = match appearance {
        Appearance::Automatic => 0,
        Appearance::Light => 1,
        Appearance::Dark => 2,
    };
    unsafe { platform::host_set_app_appearance(code) };
    // The Mac's pages follow NSApp's appearance, which the host sets.
    #[cfg(not(target_os = "macos"))]
    unsafe {
        bc::bc_set_appearance(code)
    };
}

/// Help › Restore Earlier Sidebar: the shown Space's profile's sidebar backups, newest first, each
/// by when it was taken. Dimmed until the profile has one.
pub(crate) fn restore_sidebar(backups: &[shell::SidebarBackup]) -> MenuItem {
    let items = (backups.iter())
        .map(|backup| shell::menu_item(backup.label.clone(), Command::RestoreSidebar(backup.time)))
        .collect();
    let disabled = backups.is_empty();
    MenuItem::submenu(Menu {
        name: "Restore Earlier Sidebar".into(),
        items,
        disabled,
    })
}

/// Spaces › Change Profile: the profiles the Space shown can be in, as its menu's Profile items
/// are, and New Profile.
pub(crate) fn change_profile((choices, new): &(Vec<shell::ProfileChoice>, bool)) -> MenuItem {
    let items = (choices.iter())
        .map(|choice| {
            shell::menu_item(
                choice.name.clone(),
                Command::ChangeProfile(Some(choice.profile)),
            )
            .checked(choice.checked)
            .disabled(!choice.enabled)
        })
        .chain([
            MenuItem::separator(),
            shell::menu_item("New Profile", Command::ChangeProfile(None)).disabled(!new),
        ])
        .collect();
    MenuItem::submenu(Menu {
        name: "Change Profile".into(),
        items,
        disabled: false,
    })
}
