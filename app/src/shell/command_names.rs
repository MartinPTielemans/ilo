//! What a person reads for each command, in one place. The menu bar, the footer's ilo menu, the
//! command bar and Settings › Shortcuts all take their names from here, so one name can't drift
//! from another (#3722).

use crate::{theme, updates};

use super::{commands::Command, trace};

/// The names of a command that does one thing or its opposite, by what it would do now.
pub(super) const PIN_TAB: &str = "Pin Tab";
pub(super) const UNPIN_TAB: &str = "Unpin Tab";
pub(super) const TOGGLE_SIDEBAR: &str = "Toggle Sidebar";
pub(super) const HIDE_SIDEBAR: &str = "Hide Sidebar";
pub(super) const SHOW_SIDEBAR: &str = "Show Sidebar";
pub(super) const ENTER_FULL_SCREEN: &str = "Enter Full Screen";
pub(super) const EXIT_FULL_SCREEN: &str = "Exit Full Screen";
pub const STAY_ON_TOP: &str = "Stay On Top";
pub(super) const DONT_STAY_ON_TOP: &str = "Don't Stay On Top";
pub(super) const SIDEBAR_TO_RIGHT: &str = "Move Sidebar to the Right";
pub(super) const SIDEBAR_TO_LEFT: &str = "Move Sidebar to the Left";
/// Arc's, in sentence case.
pub(super) const DEVELOPER_MODE_ON: &str = "Turn on Developer Mode for this site";
pub(super) const DEVELOPER_MODE_OFF: &str = "Turn off Developer Mode for this site";
/// Arc's: horizontal is side by side, vertical stacked (Arc 1.171).
pub(super) const CONVERT_VERTICAL: &str = "Convert to Vertical Split View";
pub(super) const CONVERT_HORIZONTAL: &str = "Convert to Horizontal Split View";
/// Arc's File menu item, which the menu bar keeps (docs/ux-spec.md › Menus).
pub const BLANK_WINDOW_ITEM: &str = "Blank Window";

impl Command {
    /// Every name a person reads for the command. The first is its name where what it would do
    /// isn't shown, as in Settings › Shortcuts. The others name what it does now, for the menu
    /// bar's item and the command bar's rows. A command named by what it acts on (a tab, a
    /// Space, a folder, a pane past the fourth, an extension, a sidebar backup) has none.
    pub(crate) const fn names(self) -> &'static [&'static str] {
        use Command::*;
        match self {
            NewTab => &["New Tab…"],
            SearchTabs => &["Search Tabs…"],
            NewWindow => &["New Window"],
            NewPrivateWindow => &["New Incognito Window"],
            NewLittleWindow => &["New Little ilo Window"],
            NewBlankWindow => &["New Blank Window", BLANK_WINDOW_ITEM],
            OpenInMainWindow => &["Open in Main Window"],
            CloseWindow => &["Close Window"],
            EditUrl => &["Open Command Bar"],
            CloseTab => &["Archive Tab"],
            ReopenClosedTab => &["Restore Last Closed Tab"],
            NextTab => &["Next Tab"],
            PreviousTab => &["Previous Tab"],
            RecentTab => &["Last Used Tab"],
            RecentTabBack => &[],
            SelectTab(_) => &[],
            SelectLastTab => &["Last Tab"],
            // Arc's menu names both with no tab shown.
            TogglePin => &["Pin/Unpin Tab", PIN_TAB, UNPIN_TAB],
            FavoriteTab => &["Favorite Tab"],
            DuplicateTab => &["Duplicate Current Tab"],
            RenameTab => &["Rename Current Tab…"],
            NewFolder => &["New Folder…"],
            CollapseFolders => &["Collapse All Folders"],
            CollapsePinned => &["Collapse Pinned Tabs"],
            ExpandPinned => &["Expand Pinned Tabs"],
            ExpandFolders => &["Expand All Folders"],
            RevealTab => &["Reveal Tab in Sidebar"],
            FocusSidebar => &["Focus Sidebar"],
            FocusAgentQuestion => &["Focus Agent Question"],
            // It rings the control, as Focus Sidebar rings a row; if it ever opens the menu in one
            // step, its name changes with it.
            FocusMenu => &[concat!("Focus ", env!("APP_NAME"), " Menu")],
            BackToPinnedUrl => &["Back to Pinned URL"],
            ResetSpaceTabs => &["Reset all tabs in this Space"],
            ToggleMute => &["Mute/Unmute Tab"],
            TurnOffBatterySaver => &["Turn Off Battery Saver for Now"],
            TranslatePage => &["Translate Page"],
            UseOriginal => &["Use Original"],
            MuteAll(true) => &["Mute All Tabs"],
            MuteAll(false) => &["Unmute All Tabs"],
            ToggleFullScreen => &[ENTER_FULL_SCREEN, EXIT_FULL_SCREEN],
            Exit => &["Exit"],
            Clear => &["Clear Today"],
            SortBySite => &["Sort Today Tabs by Site"],
            Undo | Redo => &[],
            CopyUrl => &["Copy URL"],
            CopyMarkdownLink => &["Copy URL as Markdown"],
            PasteUrlAsTab => &["Paste URL as New Tab"],
            Share => &["Share…"],
            Find => &["Find…"],
            SiteControls => &["Site Controls"],
            Cast => &["Cast…"],
            CapturePortion => &["Capture…"],
            CaptureFullPage => &["Capture Full Page"],
            CapturePortrait => &["Capture in Portrait Mode"],
            CapturePortraitNow => &[],
            FindNext => &["Find Next"],
            FindPrevious => &["Find Previous"],
            JumpToSelection => &["Jump to Selection"],
            Back => &["Go Back"],
            Forward => &["Go Forward"],
            Reload => &["Refresh the Page"],
            ForceReload => &["Force Refresh the Page"],
            ClearCacheAndRefresh => &["Clear Cache and Refresh"],
            ClearCookiesAndRefresh => &["Clear Cookies and Refresh"],
            Stop => &["Stop Loading"],
            ActualSize => &["Zoom to Actual Size"],
            ZoomIn => &["Zoom In"],
            ZoomOut => &["Zoom Out"],
            ViewSource => &["View Source"],
            SavePage => &["Save Page As"],
            Print => &["Print"],
            DevTools => &["Developer Tools"],
            InspectElements => &["Inspect Elements"],
            JavaScriptConsole => &["JavaScript Console"],
            TaskManager => &["Open Task Manager"],
            RecordTrace => &[trace::RECORD, trace::STOP_RECORDING],
            StayOnTop => &[STAY_ON_TOP, DONT_STAY_ON_TOP],
            MoveSidebar => &[SIDEBAR_TO_RIGHT, SIDEBAR_TO_LEFT],
            ToggleDeveloperMode => &[
                "Toggle Developer Mode",
                DEVELOPER_MODE_ON,
                DEVELOPER_MODE_OFF,
            ],
            ManageExtensions => &["Manage Extensions…"],
            AddExtension => &["Add Extension…"],
            NewSpace => &["New Space…"],
            RenameSpace => &["Rename Space"],
            EditTheme => &["Edit Theme…"],
            ExportSpace => &["Export Space as Markdown…"],
            ChangeProfile(None) => &["New Profile"],
            ChangeProfile(Some(_)) => &[],
            RestoreSidebar(_) => &[],
            RestoreEarlierSidebar => &["Restore Earlier Sidebar…"],
            SelectSpace(0) => &["Go to Space 1"],
            SelectSpace(1) => &["Go to Space 2"],
            SelectSpace(2) => &["Go to Space 3"],
            SelectSpace(3) => &["Go to Space 4"],
            SelectSpace(4) => &["Go to Space 5"],
            SelectSpace(5) => &["Go to Space 6"],
            SelectSpace(6) => &["Go to Space 7"],
            SelectSpace(7) => &["Go to Space 8"],
            SelectSpace(8) => &["Go to Space 9"],
            SelectSpace(_) => &[],
            MoveToSpace(_) | PinToSpace(_) | OpenFolder(_) | MoveToFolder(_) => &[],
            RunExtension(_) => &[],
            NextSpace => &["Next Space"],
            PreviousSpace => &["Previous Space"],
            ToggleSidebar => &[
                "Show/Hide Sidebar",
                TOGGLE_SIDEBAR,
                HIDE_SIDEBAR,
                SHOW_SIDEBAR,
            ],
            ViewLibrary => &["Open Library…"],
            ViewArchive => &["View Archive…"],
            ClearArchive => &["Clear Archive"],
            ViewDownloads => &["View Downloads…"],
            ViewMedia => &["View Media…"],
            ViewAgents => &["View Agents…"],
            NewBoost => &["New Boost"],
            EditBoost => &["Edit Boost"],
            ViewBoosts => &["View Boosts…"],
            ViewBoards => &["View Boards…"],
            AddSplit => &["Add Split View"],
            NewNote => &["New Note"],
            NewNoteSplit => &["New Note Split"],
            NewBoard => &["New Board"],
            DuplicateBoard => &["Duplicate Board"],
            CloseSplitPane => &["Close this Split Pane"],
            SeparatePane => &["Separate Page from Split View"],
            SeparateSplit => &["Separate All Tabs"],
            FlipSplit => &["Flip Split View"],
            ChangeOrientation => &[
                "Convert to Vertical/Horizontal Split View",
                CONVERT_VERTICAL,
                CONVERT_HORIZONTAL,
            ],
            ExpandSplit => &["Expand Current Split"],
            MovePaneLeft => &["Move Pane Left"],
            MovePaneRight => &["Move Pane Right"],
            NextPane => &["Next Pane"],
            PreviousPane => &["Previous Pane"],
            FocusPane(0) => &["Go to Pane 1"],
            FocusPane(1) => &["Go to Pane 2"],
            FocusPane(2) => &["Go to Pane 3"],
            FocusPane(3) => &["Go to Pane 4"],
            FocusPane(_) => &[],
            ViewHistory => &["View History"],
            SiteSearchSettings => &["Site Search Settings"],
            ClearBrowsingData => &["Clear Browsing Data…"],
            Settings => &["Settings…"],
            KeyboardShortcuts => &["Essential Keyboard Shortcuts"],
            About => &[concat!("About ", env!("APP_NAME"))],
            CheckForUpdates => &[updates::CHECK_FOR_UPDATES, updates::RESTART],
            Import => &["Import from Other Browsers…"],
            Passwords => &["Passwords"],
            // Edit › AutoFill's two items, which open the same page.
            AutoFill => &["Contact…", "Credit Card…"],
            AutoFillPasswords => &["Passwords…"],
            Welcome => &[concat!("Welcome to ", env!("APP_NAME"))],
            // View › Appearance's items name only the appearance, as Arc's.
            Appearance(theme::Appearance::Automatic) => &["Automatic Appearance", "Automatic"],
            Appearance(theme::Appearance::Light) => &["Light Appearance", "Light"],
            Appearance(theme::Appearance::Dark) => &["Dark Appearance", "Dark"],
            ReportProblem => &["Report a Problem…"],
            CopyBuildInfo => &["Copy Build Info"],
            HelpCenter => &["Help Center"],
            WhatsNew => &["What's New"],
            PrivacyPolicy => &["Privacy Policy"],
            Quit => &[concat!("Quit ", env!("APP_NAME"))],
        }
    }

    /// The command's name where what it would do isn't shown, or empty for one named by what it
    /// acts on.
    pub(crate) const fn title(self) -> &'static str {
        match self.names() {
            [name, ..] => name,
            [] => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Menu, MenuItem};

    use super::super::commands::{BAR_COMMANDS, Command, Run};
    use super::super::{MenuBar, ProfileChoice};
    use crate::menu_bar_items::menus;
    use crate::settings_shortcuts::{Keys, SHORTCUTS};

    /// The commands named more than one way: by what they'd do now, or by each of their items.
    /// Fixed here, so a name can't be made to pass by adding it to a command's list.
    const SEVERAL: &[Command] = &[
        Command::TogglePin,
        Command::ToggleSidebar,
        Command::ToggleFullScreen,
        Command::StayOnTop,
        Command::MoveSidebar,
        Command::ToggleDeveloperMode,
        Command::ChangeOrientation,
        Command::RecordTrace,
        Command::CheckForUpdates,
        Command::AutoFill,
        Command::NewBlankWindow,
        Command::Appearance(crate::theme::Appearance::Automatic),
        Command::Appearance(crate::theme::Appearance::Light),
        Command::Appearance(crate::theme::Appearance::Dark),
    ];

    fn items(menu: &Menu, out: &mut Vec<(String, Command)>) {
        for item in &menu.items {
            match item {
                MenuItem::Submenu(menu) => items(menu, out),
                MenuItem::Action { name, action, .. } => {
                    if let Some(Run(command)) = action.as_any().downcast_ref::<Run>() {
                        out.push((name.to_string(), *command));
                    }
                }
                _ => {}
            }
        }
    }

    /// The menu bar's items in each state its names follow.
    fn menu_bar_items() -> Vec<(String, Command)> {
        let mut out = Vec::new();
        for state in 0..2 {
            let on = state == 1;
            let bar = MenuBar {
                sidebar_hidden: on,
                pinned_shut: on,
                developer_mode: Some(on),
                tracing: on,
                on_top: Some(on),
                boosts: true,
                profiles: Some((Vec::<ProfileChoice>::new(), true)),
                ..MenuBar::default()
            };
            for pinned in [None, Some(true), Some(false)] {
                let bar = MenuBar {
                    pinned,
                    ..bar.clone()
                };
                for menu in menus(&bar, None, None, on, true) {
                    items(&menu, &mut out);
                }
            }
        }
        out
    }

    /// The menu bar, the command bar and Settings › Shortcuts name a command by one of its names,
    /// Settings with no "…". The footer's menu and the Today menu read `title` itself.
    #[test]
    fn every_surface_names_a_command_by_its_names() {
        let mut named: Vec<(&str, String, Command)> = Vec::new();
        for (name, command) in menu_bar_items() {
            named.push(("menu bar", name, command));
        }
        for (name, command, _) in BAR_COMMANDS {
            named.push(("command bar", name.to_string(), *command));
        }
        for (_, rows) in SHORTCUTS {
            for (name, keys) in rows.iter() {
                if let Keys::Of(command) = keys {
                    named.push(("Settings › Shortcuts", name.to_string(), *command));
                }
            }
        }
        let mut seen = Vec::new();
        named.retain(|each| {
            let new = !seen.contains(each);
            seen.extend(new.then(|| each.clone()));
            new
        });
        let wrong: Vec<_> = (named.iter())
            .filter(|(place, name, command)| {
                let names = command.names();
                let settings = *place == "Settings › Shortcuts";
                // Spaces, profiles and sidebar backups are named by what they are.
                !names.is_empty()
                    && !names
                        .iter()
                        .any(|each| *each == name || settings && each.trim_end_matches('…') == name)
            })
            .collect();
        let mut several: Vec<Command> = (named.iter())
            .map(|(_, _, command)| *command)
            .filter(|command| command.names().len() > 1 && !SEVERAL.contains(command))
            .collect();
        several.dedup();
        assert!(
            wrong.is_empty() && several.is_empty(),
            "named apart from Command::names: {wrong:#?}\nnamed several ways: {several:?}"
        );
    }

    /// A command a surface lists has a name for it, unless it's named by what it acts on.
    #[test]
    fn a_listed_command_has_a_name() {
        let menu = menu_bar_items().into_iter().map(|(_, command)| command);
        let bar = BAR_COMMANDS.iter().map(|(_, command, _)| *command);
        let unnamed: Vec<Command> = menu
            .chain(bar)
            .filter(|command| command.names().is_empty())
            .collect();
        assert!(unnamed.is_empty(), "no name: {unnamed:?}");
    }
}
