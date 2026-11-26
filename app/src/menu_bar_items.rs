//! The menu bar's menus: Arc's, with every command named by `Command::names`.

use gpui::{Menu, MenuItem, OsAction, SystemMenuType, actions};

use crate::shell::{self, Command};
use crate::text_input::{Copy, Cut, Paste, PasteAndMatchStyle, Redo, SelectAll, Undo};
use crate::theme::Appearance;
use crate::updates;

actions!(browser, [Hide, HideOthers, ShowAll, Minimize]);

actions!(browser, [CheckForUpdates]);

/// "Undo" or "Redo", with what it would undo or redo after it.
pub fn edit_title(verb: &str, what: Option<&str>) -> String {
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
pub fn menus(
    bar: &shell::MenuBar,
    undo: Option<&str>,
    redo: Option<&str>,
    update_ready: bool,
    updates: bool,
) -> Vec<Menu> {
    let menu = |name: &str, items| Menu {
        name: name.to_string().into(),
        items,
        disabled: false,
    };
    let item = shell::menu_item;
    let named = |command: Command| item(command.title(), command);
    let (spaces, extensions) = (&bar.spaces, &bar.extensions);
    let about = named(Command::About);
    let mut app = vec![about, MenuItem::separator()];
    // Only a release build that carries Sparkle has updates to check for.
    if updates {
        let title = updates::title(update_ready);
        app.extend([
            MenuItem::action(title, CheckForUpdates),
            MenuItem::separator(),
        ]);
    }
    app.extend([
        named(Command::Settings),
        named(Command::Import),
        named(Command::PrivacyPolicy),
        MenuItem::separator(),
        MenuItem::os_submenu("Services", SystemMenuType::Services),
        MenuItem::separator(),
        MenuItem::action(concat!("Hide ", env!("PRODUCT_NAME")), Hide),
        MenuItem::action("Hide Others", HideOthers),
        MenuItem::action("Show All", ShowAll),
        MenuItem::separator(),
        named(Command::Quit),
    ]);
    vec![
        menu(env!("PRODUCT_NAME"), app),
        menu(
            "File",
            [
                Some(named(Command::NewTab)),
                Some(named(Command::NewWindow)),
                // Not with Little ilo off in Settings › Features.
                (!bar.little_off).then(|| named(Command::NewLittleWindow)),
            ]
            .into_iter()
            .flatten()
            .chain([
                named(Command::NewPrivateWindow),
                item(shell::BLANK_WINDOW_ITEM, Command::NewBlankWindow),
                named(Command::NewBoard),
                named(Command::ReopenClosedTab),
                MenuItem::separator(),
                named(Command::EditUrl),
            ])
            // Arc's place for it. As Change Profile's, only an empty Space takes a new profile, and
            // a private window has none.
            .chain(
                bar.profiles
                    .as_ref()
                    .map(|(_, new)| named(Command::ChangeProfile(None)).disabled(!new)),
            )
            .chain([
                MenuItem::separator(),
                named(Command::CloseWindow),
                named(Command::CloseTab),
                MenuItem::separator(),
                named(Command::Share),
                named(Command::CapturePortion),
                named(Command::CaptureFullPage),
                named(Command::CapturePortrait),
                named(Command::SavePage),
                named(Command::Print),
            ])
            .collect(),
        ),
        menu(
            "Edit",
            vec![
                MenuItem::os_action(edit_title("Undo", undo), Undo, OsAction::Undo),
                MenuItem::os_action(edit_title("Redo", redo), Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action("Cut", Cut, OsAction::Cut),
                MenuItem::os_action("Copy", Copy, OsAction::Copy),
                named(Command::CopyUrl),
                named(Command::CopyMarkdownLink),
                MenuItem::os_action("Paste", Paste, OsAction::Paste),
                MenuItem::os_action(
                    "Paste and Match Style",
                    PasteAndMatchStyle,
                    OsAction::PasteAndMatchStyle,
                ),
                named(Command::PasteUrlAsTab),
                MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
                MenuItem::separator(),
                MenuItem::submenu(menu(
                    "Find",
                    vec![
                        named(Command::Find),
                        named(Command::FindNext),
                        named(Command::FindPrevious),
                        named(Command::JumpToSelection),
                    ],
                )),
                // Where AppKit puts the system's, which ilo's replaces (platform/mac/host.mm).
                MenuItem::separator(),
                MenuItem::submenu(menu(
                    "AutoFill",
                    vec![
                        named(Command::AutoFill),
                        named(Command::AutoFillPasswords),
                        item(Command::AutoFill.names()[1], Command::AutoFill),
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
                named(Command::Stop),
                named(Command::Reload),
                named(Command::ForceReload),
                named(Command::ClearCookiesAndRefresh),
                named(Command::ClearCacheAndRefresh),
                MenuItem::separator(),
                named(Command::AddSplit),
                named(Command::CloseSplitPane),
                named(Command::SeparatePane),
                MenuItem::separator(),
                named(Command::ActualSize),
                named(Command::ZoomIn),
                named(Command::ZoomOut),
                MenuItem::separator(),
                named(Command::Cast),
                MenuItem::separator(),
                MenuItem::submenu(menu(
                    "Developer",
                    vec![
                        named(Command::ViewSource),
                        named(Command::DevTools),
                        named(Command::InspectElements),
                        named(Command::JavaScriptConsole),
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
                named(Command::ExpandSplit),
            ],
        ),
        menu(
            "Spaces",
            [
                named(Command::NewSpace),
                named(Command::EditTheme),
                named(Command::RenameSpace),
            ]
            .into_iter()
            .chain(bar.profiles.as_ref().map(change_profile))
            .chain([
                MenuItem::separator(),
                named(Command::NextSpace),
                named(Command::PreviousSpace),
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
                named(Command::NewTab),
                named(Command::SearchTabs),
                item(bar.pin_title(), Command::TogglePin),
                named(Command::NewFolder),
                MenuItem::separator(),
                named(Command::NextTab),
                named(Command::PreviousTab),
                named(Command::RevealTab),
                named(Command::FocusSidebar),
                MenuItem::separator(),
                named(Command::Clear),
                named(Command::SortBySite).disabled(!bar.sort_offered),
                named(Command::SortIntoFolders).disabled(!bar.broom_offered),
                named(Command::ResetSpaceTabs),
            ],
        ),
        menu(
            "Archive",
            vec![
                named(Command::Back),
                named(Command::Forward),
                MenuItem::separator(),
                named(Command::ViewHistory),
                named(Command::ViewArchive),
                named(Command::ClearArchive),
                MenuItem::separator(),
                named(Command::ClearBrowsingData),
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
                    named(Command::AddExtension),
                    named(Command::ManageExtensions),
                ])
                .collect(),
        ),
        menu(
            "Window",
            [
                Some(MenuItem::action("Minimize Window", Minimize)),
                Some(stay_on_top(bar.on_top)),
                Some(MenuItem::separator()),
                Some(named(Command::ViewLibrary)),
                Some(named(Command::ViewDownloads)),
                Some(named(Command::ViewMedia)),
                Some(named(Command::ViewBoards)),
                // Arc's, after View Media…, where the window lists Boosts (Library › Boosts).
                bar.boosts.then(|| named(Command::ViewBoosts)),
                Some(named(Command::ViewAgents)),
            ]
            .into_iter()
            .flatten()
            .collect(),
        ),
        menu(
            "Help",
            vec![
                named(Command::Welcome),
                named(Command::HelpCenter),
                named(Command::WhatsNew),
                named(Command::KeyboardShortcuts),
                MenuItem::separator(),
                named(Command::ReportProblem),
                restore_sidebar(&bar.sidebar_backups),
                MenuItem::submenu(menu(
                    "Troubleshooting",
                    vec![
                        named(Command::CopyBuildInfo),
                        named(Command::TaskManager),
                        item(bar.trace_title(), Command::RecordTrace),
                    ],
                )),
            ],
        ),
    ]
}

/// Window › Stay On Top, ticked while it's on, where the window can stay on top at all.
fn stay_on_top(on: Option<bool>) -> MenuItem {
    match on {
        Some(on) => shell::menu_item(shell::STAY_ON_TOP, Command::StayOnTop).checked(on),
        None => shell::menu_item(shell::STAY_ON_TOP, Command::StayOnTop).disabled(true),
    }
}

/// View › Appearance's item for `appearance`, as Arc's names them.
pub fn appearance_title(appearance: Appearance) -> &'static str {
    Command::Appearance(appearance).names()[1]
}

/// Help › Restore Earlier Sidebar: the shown Space's profile's sidebar backups, newest first, each
/// by when it was taken. Dimmed until the profile has one.
pub fn restore_sidebar(backups: &[shell::SidebarBackup]) -> MenuItem {
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
pub fn change_profile((choices, new): &(Vec<shell::ProfileChoice>, bool)) -> MenuItem {
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
            shell::menu_item(
                Command::ChangeProfile(None).title(),
                Command::ChangeProfile(None),
            )
            .disabled(!new),
        ])
        .collect();
    MenuItem::submenu(Menu {
        name: "Change Profile".into(),
        items,
        disabled: false,
    })
}
