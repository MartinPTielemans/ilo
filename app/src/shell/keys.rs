//! The user's keys for the browser's commands (docs/ux-spec.md › Keyboard): each command keeps its
//! keys in SHORTCUTS unless the user gave it others in Settings › Shortcuts, or none. GPUI's
//! bindings, the menus, the command bar and the keys the page hands back all ask this store, so a
//! change reaches them at once.

use std::time::{Duration, Instant};

use gpui::{
    Action, App, Global, KeyBinding, KeyBindingContextPredicate, Keystroke, Modifiers, NoAction,
    OperatingSystem,
};

use super::commands::{
    BAR_COMMANDS, KEY_CONTEXT, Run, SHORTCUTS, ShellHandle, keystroke_keys, keystroke_label,
};
use super::Command;

/// Keys that run something other than a command: AppKit's Hide, Hide Others and Minimize, which
/// main.rs binds, the Edit menu's, which reach the focused page or field, and the View menu's
/// full screen.
const SYSTEM_KEYS: &[(&str, &str)] = &[
    ("cmd-h", concat!("Hide ", env!("APP_NAME"))),
    ("alt-cmd-h", "Hide Others"),
    ("cmd-m", "Minimize"),
    ("cmd-x", "Cut"),
    ("cmd-c", "Copy"),
    ("cmd-v", "Paste"),
    ("cmd-a", "Select All"),
    ("ctrl-cmd-f", "Enter Full Screen"),
];

/// Characters typed with ⇧ on a US keyboard. GPUI names ⇧⌘= by the character, cmd-+, and a page
/// by the character and ⇧, shift-cmd-+.
const SHIFTED: &str = "~!@#$%^&*()_+{}|:\"<>?";

/// Who keeps a key the user asked for.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Holder {
    /// A reserved command, or one that can't be rebound.
    Command(Command),
    /// One of SYSTEM_KEYS, by its menu item's name.
    System(&'static str),
}

/// Why a key the user pressed wasn't given to the command.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Refused {
    /// Without ⌘ or ⌃ it would type, unless it's a function key.
    Plain,
    Taken(Holder),
}

/// The commands the user gave keys of their own, or none, whether ⌘1–⌘8 leave the favorites out,
/// and whether ⌘9 is the ninth tab.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct UserKeys {
    set: Vec<(Command, Vec<String>)>,
    /// Off by default, so ⌘1 is the first favorite, as in Arc.
    favorites_unnumbered: bool,
    /// Off by default, so ⌘9 is the last tab, as in Arc and Chrome.
    last_tab_ninth: bool,
    /// The commands a page gets the keys of first that the user has run before the page instead,
    /// as Arc's Prefer Arc does.
    ilo_first: Vec<Command>,
}

/// The Shortcuts file's line for favorites_unnumbered, which no command is named.
const FAVORITES_UNNUMBERED: &str = "favorites-unnumbered";
/// And its line for last_tab_ninth.
const LAST_TAB_NINTH: &str = "last-tab-ninth";
/// The start of its line for a command in ilo_first, before the command's name.
const PREFER_ILO: &str = "prefer-ilo";

impl Global for UserKeys {}

static DEFAULTS: UserKeys =
    UserKeys {
        set: Vec::new(),
        favorites_unnumbered: false,
        last_tab_ninth: false,
        ilo_first: Vec::new(),
    };

/// How soon after a page kept a shortcut a second press of it runs ilo's command, as Arc's does.
const SECOND_PRESS: Duration = Duration::from_secs(1);

/// The shortcut a page got first last, for a second press of it to run ilo's command if the page
/// kept the first, as Arc's does when a page and the browser use the same keys.
#[derive(Default)]
pub struct SecondPress(Option<(u64, String, Instant)>);

impl SecondPress {
    /// Whether this press of `keystroke` in `tab`'s page runs ilo's command at once, where
    /// `shortcut` says it's a shortcut the page gets first: the second press of one within
    /// SECOND_PRESS of the first, which the page kept. A key repeating as it's held isn't a press.
    pub fn press(
        &mut self,
        tab: u64,
        keystroke: &str,
        shortcut: bool,
        repeat: bool,
        now: Instant,
    ) -> bool {
        if repeat {
            return false;
        }
        let second = shortcut
            && (self.0.as_ref()).is_some_and(|(pressed, first, at)| {
                *pressed == tab && first == keystroke && now.duration_since(*at) <= SECOND_PRESS
            });
        self.0 = (shortcut && !second).then(|| (tab, keystroke.to_string(), now));
        second
    }

    /// The page didn't keep `keystroke`, so ilo ran its command, and the next press is a first.
    pub fn declined(&mut self, keystroke: &str) {
        if (self.0.as_ref()).is_some_and(|(_, first, _)| first == keystroke) {
            self.0 = None;
        }
    }
}

/// The user's keys, or the defaults before they're read.
pub fn user_keys(cx: &App) -> &UserKeys {
    cx.try_global::<UserKeys>().unwrap_or(&DEFAULTS)
}

/// Whether the user can give `command` other keys. Not Quit, whose ⌘Q is the way out of every Mac
/// app; not Undo and Redo, whose ⌘Z and ⇧⌘Z fall back to the Edit menu's for the page; not the
/// ⌃Tab walk, which lasts while ⌃ is held; not the numbered tabs, eight keys in one row; and not
/// the rows only the command bar lists, which Settings › Shortcuts doesn't. Each of ⌃1–⌃9's Spaces
/// can have others, as Arc's can.
pub fn rebindable(command: Command) -> bool {
    !matches!(
        command,
        Command::Quit
            | Command::Undo
            | Command::Redo
            | Command::RecentTab
            | Command::RecentTabBack
            | Command::SelectTab(_)
            | Command::MoveToSpace(_)
            | Command::PinToSpace(_)
            | Command::OpenFolder(_)
            | Command::EditTheme
    )
}

/// Whether `command` reaches the browser before the page, on whichever key it has.
pub fn reserved(command: Command) -> bool {
    SHORTCUTS.iter().any(|&(_, each, reserved)| each == command && reserved)
}

/// Chrome's keys on Linux and Windows that the Mac's Chrome and Arc have no key for.
const OFF_THE_MAC_ONLY: &[&str] = &["shift-escape"];

fn defaults(command: Command) -> Vec<String> {
    (SHORTCUTS.iter())
        .filter(|&&(_, each, _)| each == command)
        .filter(|&&(key, ..)| OFF_THE_MAC || !OFF_THE_MAC_ONLY.contains(&key))
        .map(|&(key, ..)| key.to_string())
        .collect()
}

/// The rebindable command `name` names, as Command::key_name writes it.
fn named(name: &str) -> Option<Command> {
    let commands = SHORTCUTS.iter().map(|&(_, command, _)| command);
    (commands.chain(BAR_COMMANDS.iter().map(|&(_, command, _)| command)))
        .find(|&command| command.key_name() == name && rebindable(command))
}

/// Whether `key` is one keystroke GPUI can read. Only its spelling is checked, which is every
/// platform's.
fn valid(key: &str) -> bool {
    let parsed = Keystroke::parse_with_os(key, OperatingSystem::Mac);
    !key.contains(char::is_whitespace) && parsed.is_ok()
}

/// GPUI's bindings of `key` to `action` while the UI has the keyboard, one for each way this
/// platform presses it.
fn bindings_of(key: &str, action: Box<dyn Action>, cx: &App) -> Vec<KeyBinding> {
    let Ok(context) = KeyBindingContextPredicate::parse(KEY_CONTEXT) else {
        return Vec::new();
    };
    let context = std::rc::Rc::new(context);
    let load = |key: String| {
        let action = action.boxed_clone();
        KeyBinding::load(&key, action, Some(context.clone()), false, None, cx).ok()
    };
    pressed_as(key).into_iter().filter_map(load).collect()
}

/// Off the Mac, as on Linux and Windows, the keys SHORTCUTS writes the Mac's way are pressed with
/// Ctrl for ⌘ and Alt for ⌃ (docs/ux-spec.md › Keyboard). The tests run the Mac's keys everywhere.
pub const OFF_THE_MAC: bool = cfg!(not(any(target_os = "macos", test)));

/// Chrome's Alt keys on Linux and Windows, which keep Chrome's meaning there, so no ⌃ key becomes
/// one, with the key of ours that does what Chrome's does, if one does.
const CHROME_ALT_KEYS: &[(&str, Option<&str>)] = &[
    ("alt-left", Some("cmd-left")),
    ("alt-right", Some("cmd-right")),
    ("alt-d", Some("cmd-l")),
    ("alt-f4", Some("shift-cmd-w")),
    ("alt-home", None),
    ("alt-e", None),
    ("alt-f", None),
];

/// `key`, as SHORTCUTS writes it, as this platform's keyboard presses it: none, one way or more.
pub(super) fn pressed_as(key: &str) -> Vec<String> {
    match OFF_THE_MAC {
        true => pressed_off_the_mac(key),
        false => vec![key.to_string()],
    }
}

fn pressed_off_the_mac(key: &str) -> Vec<String> {
    let ([ctrl, alt, shift, cmd], rest) = held(key);
    // ⌃Tab stays Ctrl+Tab, as Alt+Tab is the desktop's, and a key with ⌃ and ⌥ has no Alt of its
    // own.
    let pressed = match (ctrl, alt) {
        _ if ctrl && !cmd && rest == "tab" => key.to_string(),
        (true, true) => return Vec::new(),
        _ => written([cmd, ctrl || alt, shift, false], rest),
    };
    if CHROME_ALT_KEYS.iter().any(|&(chrome, _)| chrome == pressed) {
        return Vec::new();
    }
    let chrome = CHROME_ALT_KEYS.iter().filter(|&&(_, ours)| ours == Some(key));
    std::iter::once(pressed).chain(chrome.map(|&(chrome, _)| chrome.to_string())).collect()
}

/// A binding of `key`, written the Mac's way, to `action` while `context` has the keyboard, in a
/// view of the UI's own such as the command bar, as this platform presses it: off the Mac, Ctrl
/// for ⌘ and Alt for ⌃ or ⌥.
pub fn view_binding(key: &str, action: Box<dyn Action>, context: &str, cx: &App) -> KeyBinding {
    let ([ctrl, alt, shift, cmd], rest) = held(key);
    let pressed = match OFF_THE_MAC {
        true => written([cmd, ctrl || alt, shift, false], rest),
        false => key.to_string(),
    };
    let context = KeyBindingContextPredicate::parse(context).ok().map(std::rc::Rc::new);
    KeyBinding::load(&pressed, action, context, false, None, cx).expect("a valid key")
}

/// A key the core names (bc_key) as a keystroke in GPUI's terms, with the modifiers in the order
/// SHORTCUTS writes them, and Ctrl as ⌘ off the Mac. With ⌘ held, a layout that isn't Latin, such
/// as Hebrew, has its shortcuts by the key's place on the user's Latin layout, `latin_key`, as the
/// Mac's menus and Chrome's have them: Hebrew's ⌘ on the key it types "א" with is ⌘T, and on the
/// one it types "'" with is ⌘W, though that's ASCII.
pub fn page_keystroke(key: &str, latin_key: &str, modifiers: u32) -> String {
    written_as(&pressed_in_page(key, latin_key, modifiers, OFF_THE_MAC))
}

/// The key the page had, as the platform names it: by `latin_key` with the platform's ⌘ held,
/// which off the Mac is Ctrl, as Chrome's shortcuts there take a key by its place on the US layout.
fn pressed_in_page(key: &str, latin_key: &str, modifiers: u32, off_the_mac: bool) -> String {
    use crate::bc;
    let mut keystroke = String::new();
    for (flag, modifier) in [
        (bc::BC_MODIFIER_CONTROL, "ctrl-"),
        (bc::BC_MODIFIER_ALT, "alt-"),
        (bc::BC_MODIFIER_SHIFT, "shift-"),
        (bc::BC_MODIFIER_META, "cmd-"),
    ] {
        if modifiers & flag != 0 {
            keystroke.push_str(modifier);
        }
    }
    let command = match off_the_mac {
        true => bc::BC_MODIFIER_CONTROL,
        false => bc::BC_MODIFIER_META,
    };
    let name = if modifiers & command != 0 && !latin_key.is_empty() { latin_key } else { key };
    keystroke + &gpui_key(name)
}

/// The key the web names `key`, such as "ArrowLeft" or " ", as GPUI names it.
fn gpui_key(key: &str) -> String {
    match key {
        "ArrowDown" => "down".to_string(),
        "ArrowUp" => "up".to_string(),
        "ArrowLeft" => "left".to_string(),
        "ArrowRight" => "right".to_string(),
        " " => "space".to_string(),
        name => name.to_lowercase(),
    }
}

/// The keystroke GPUI has for the key the web names `key`, such as "G" or "ArrowLeft", with
/// `modifiers` held, and the text it types if it's a character without ctrl, alt or cmd: one the
/// core held from the page for a field of the app's (bc_hold_keys).
pub fn web_keystroke(key: &str, modifiers: Modifiers) -> Keystroke {
    let shortcut = modifiers.control || modifiers.alt || modifiers.platform;
    let types = key.chars().count() == 1 && !shortcut;
    Keystroke {
        modifiers,
        key: gpui_key(key),
        key_char: types.then(|| key.to_string()),
    }
}

/// The key `keystroke`, as this platform names it, as SHORTCUTS writes it: cmd-t for ctrl-t and
/// ctrl-2 for alt-2 off the Mac.
pub fn written_as(keystroke: &str) -> String {
    match OFF_THE_MAC {
        true => written_off_the_mac(keystroke),
        false => keystroke.to_string(),
    }
}

fn written_off_the_mac(keystroke: &str) -> String {
    if let Some(&(_, Some(ours))) = CHROME_ALT_KEYS.iter().find(|&&(chrome, _)| chrome == keystroke)
    {
        return ours.to_string();
    }
    let ([ctrl, alt, shift, superkey], rest) = held(keystroke);
    if superkey {
        // The desktop's, and no ⌘.
        return keystroke.replace("cmd-", "super-");
    }
    if ctrl && !alt && rest == "tab" {
        return keystroke.to_string();
    }
    // Alt alone is ⌃, since ⌥ alone is no shortcut. With Ctrl it's ⌥, unless ⌃⌘ makes a key ours.
    let as_control = written([alt, false, shift, ctrl], rest);
    let known = |key: &str| {
        let keys = SHORTCUTS.iter().map(|&(key, ..)| key);
        keys.chain(SYSTEM_KEYS.iter().map(|&(key, _)| key)).any(|each| each == key)
    };
    match alt && (!ctrl || known(&as_control)) {
        true => as_control,
        false => written([false, alt, shift, ctrl], rest),
    }
}

/// The modifiers `key` holds, ctrl, alt, shift and cmd, in the order SHORTCUTS writes them, and
/// the key.
fn held(key: &str) -> ([bool; 4], &str) {
    let mut rest = key;
    let held = MODIFIERS.map(|modifier| match rest.strip_prefix(modifier) {
        Some(after) => {
            rest = after;
            true
        }
        None => false,
    });
    (held, rest)
}

fn written(held: [bool; 4], key: &str) -> String {
    let modifiers = MODIFIERS.iter().zip(held).filter(|&(_, held)| held);
    modifiers.map(|(modifier, _)| *modifier).collect::<String>() + key
}

const MODIFIERS: [&str; 4] = ["ctrl-", "alt-", "shift-", "cmd-"];

/// The keys the user pressed, as SHORTCUTS writes them: the one GPUI matches first, then the one
/// a page names, where they differ.
pub fn recorded(keystroke: &Keystroke) -> Result<Vec<String>, Refused> {
    let modifiers = &keystroke.modifiers;
    let key = keystroke.key.as_str();
    let function = key.strip_prefix('f').is_some_and(|number| number.parse::<u8>().is_ok());
    let text = |shift: bool| {
        let mut text = String::new();
        for (held, name) in [
            (modifiers.control, "ctrl-"),
            (modifiers.alt, "alt-"),
            (shift, "shift-"),
            (modifiers.platform, "cmd-"),
        ] {
            if held {
                text.push_str(name);
            }
        }
        written_as(&(text + key))
    };
    let mut keys = vec![text(modifiers.shift)];
    if !modifiers.shift && key.chars().count() == 1 && SHIFTED.contains(key) {
        keys.push(text(true));
    }
    // Without ⌘ or ⌃, the Mac's way, it would type.
    keys.retain(|key| valid(key) && (function || key.contains("ctrl-") || key.contains("cmd-")));
    match keys.is_empty() {
        true => Err(Refused::Plain),
        false => Ok(keys),
    }
}

impl UserKeys {
    /// The keys that run `command`, the one menus show first.
    pub fn keys(&self, command: Command) -> Vec<&str> {
        match self.set.iter().find(|(each, _)| *each == command) {
            Some((_, keys)) => keys.iter().map(String::as_str).collect(),
            None => (SHORTCUTS.iter())
                .filter(|&&(_, each, _)| each == command)
                .map(|&(key, ..)| key)
                .collect(),
        }
    }

    /// Whether the user gave `command` keys other than its defaults.
    pub fn is_set(&self, command: Command) -> bool {
        self.set.iter().any(|(each, _)| *each == command)
    }

    /// The command `keystroke` runs, if it's a shortcut that reaches the browser before the page,
    /// with `reserved`, or one the page gets first.
    pub fn command_for(&self, keystroke: &str, reserved: bool) -> Option<Command> {
        self.command_with(keystroke).filter(|&command| self.before_page(command) == reserved)
    }

    /// Whether `command`'s keys reach the browser before the page: a reserved one's always, and
    /// another's if the user prefers ilo for it.
    pub fn before_page(&self, command: Command) -> bool {
        reserved(command) || self.ilo_first.contains(&command)
    }

    /// Whether the user can choose who gets `command`'s keys first, the page or ilo: a command
    /// they can give keys that isn't reserved.
    pub fn can_prefer(command: Command) -> bool {
        rebindable(command) && !reserved(command)
    }

    /// Has `command`'s keys reach ilo before the page, Prefer ilo, or the page first, Prefer
    /// Websites, the default (Settings › Shortcuts). Not for a reserved command's.
    pub fn set_prefers_ilo(&mut self, command: Command, ilo: bool) {
        self.ilo_first.retain(|&each| each != command);
        if ilo && Self::can_prefer(command) {
            self.ilo_first.push(command);
        }
    }

    /// Whether the user has `command`'s keys reach ilo before the page, Prefer ilo.
    pub fn prefers_ilo(&self, command: Command) -> bool {
        self.ilo_first.contains(&command)
    }

    /// Whether the user changed `command` from its defaults: its keys or who gets them first.
    pub fn changed(&self, command: Command) -> bool {
        self.is_set(command) || self.ilo_first.contains(&command)
    }

    /// Whether `keystroke` is a shortcut the page gets first, and may keep, that opens a field over
    /// it, such as ⌘L, so the keys typed right after it wait for that field. A reserved one's bar
    /// holds them itself, since it opens before the page could see them.
    pub fn holds_keys(&self, keystroke: &str) -> bool {
        self.command_for(keystroke, false).is_some_and(Command::opens_field)
    }

    fn command_with(&self, keystroke: &str) -> Option<Command> {
        let user = self.set.iter().find(|(_, keys)| keys.iter().any(|key| key == keystroke));
        user.map(|&(command, _)| command).or_else(|| {
            (SHORTCUTS.iter())
                .find(|&&(key, command, _)| key == keystroke && !self.is_set(command))
                .map(|&(_, command, _)| command)
        })
    }

    /// How a menu shows the first key that runs `command`, if one does.
    pub fn label(&self, command: Command) -> Option<String> {
        self.keys(command).first().map(|&key| keystroke_label(key))
    }

    /// The same keys one by one, for a tooltip's chips: ⇧, ⌘ and L for ⇧⌘L. None if no key runs
    /// `command`.
    pub fn chips(&self, command: Command) -> Vec<String> {
        self.keys(command).first().map(|&key| keystroke_keys(key)).unwrap_or_default()
    }

    /// Each key and the command it runs.
    fn bindings(&self) -> Vec<(&str, Command)> {
        let defaults = SHORTCUTS.iter().filter(|&&(_, command, _)| !self.is_set(command));
        let user = (self.set.iter())
            .flat_map(|(command, keys)| keys.iter().map(|key| (key.as_str(), *command)));
        (defaults.map(|&(key, command, _)| (key, command))).chain(user).collect()
    }

    /// Gives `command` `keys`, or none, taking each from the command that has it, as a user's
    /// binding beats the others (Dia). A reserved command's key, one that can't be rebound and the
    /// system's stay theirs. Returns the commands that lost a key.
    pub fn set(&mut self, command: Command, keys: Vec<String>) -> Result<Vec<Command>, Refused> {
        if !rebindable(command) {
            return Err(Refused::Taken(Holder::Command(command)));
        }
        for key in &keys {
            if let Some(&(_, name)) = SYSTEM_KEYS.iter().find(|(each, _)| each == key) {
                return Err(Refused::Taken(Holder::System(name)));
            }
            match self.command_with(key) {
                Some(holder) if holder != command && (reserved(holder) || !rebindable(holder)) => {
                    return Err(Refused::Taken(Holder::Command(holder)));
                }
                _ => {}
            }
        }
        let mut losers = Vec::new();
        for key in &keys {
            if let Some(holder) = self.command_with(key).filter(|&holder| holder != command) {
                let left = self.keys(holder).into_iter().filter(|each| each != key);
                let left = left.map(str::to_string).collect();
                self.put(holder, left);
                if !losers.contains(&holder) {
                    losers.push(holder);
                }
            }
        }
        self.put(command, keys);
        Ok(losers)
    }

    /// Gives `command` its default keys back, as set does, and the page its keys first.
    pub fn reset(&mut self, command: Command) -> Result<Vec<Command>, Refused> {
        self.set_prefers_ilo(command, false);
        self.set(command, defaults(command))
    }

    /// Every command's default keys, the favorites numbered again and ⌘9 the last tab.
    pub fn reset_all(&mut self) {
        *self = UserKeys::default();
    }

    /// Whether ⌘1–⌘8 count the favorites before the Space's tabs (Settings › Shortcuts).
    pub fn favorites_numbered(&self) -> bool {
        !self.favorites_unnumbered
    }

    pub fn set_favorites_numbered(&mut self, numbered: bool) {
        self.favorites_unnumbered = !numbered;
    }

    /// Whether Last Tab (⌘9) goes to the ninth tab, counted as ⌘1–⌘8 count, rather than the last
    /// (Settings › Shortcuts).
    pub fn last_tab_ninth(&self) -> bool {
        self.last_tab_ninth
    }

    pub fn set_last_tab_ninth(&mut self, ninth: bool) {
        self.last_tab_ninth = ninth;
    }

    /// Keeps `keys` for `command`, unless they're its defaults.
    fn put(&mut self, command: Command, keys: Vec<String>) {
        self.set.retain(|(each, _)| *each != command);
        if keys != defaults(command) {
            self.set.push((command, keys));
        }
    }

    /// As Settings saves them: a line a command, its name, then its keys, if any, a line saying the
    /// favorites are left out of ⌘1–⌘8 when they are, one saying ⌘9 is the ninth tab, and one for
    /// each command ilo gets the keys of before the page.
    pub fn to_text(&self) -> String {
        let lines = self.set.iter().map(|(command, keys)| {
            let name = command.key_name();
            let words = std::iter::once(name.as_str()).chain(keys.iter().map(String::as_str));
            words.collect::<Vec<_>>().join(" ") + "\n"
        });
        let unnumbered = self.favorites_unnumbered.then(|| format!("{FAVORITES_UNNUMBERED}\n"));
        let ninth = self.last_tab_ninth.then(|| format!("{LAST_TAB_NINTH}\n"));
        let ilo_first =
            self.ilo_first.iter().map(|command| format!("{PREFER_ILO} {}\n", command.key_name()));
        lines.chain(unnumbered).chain(ninth).chain(ilo_first).collect()
    }

    /// What to_text wrote. A line it can't read, or keys another command keeps, is left out.
    pub fn parse(text: &str) -> Self {
        let mut keys = UserKeys::default();
        for line in text.lines() {
            if line.trim() == FAVORITES_UNNUMBERED {
                keys.favorites_unnumbered = true;
                continue;
            }
            if line.trim() == LAST_TAB_NINTH {
                keys.last_tab_ninth = true;
                continue;
            }
            let mut words = line.split_whitespace();
            let first = words.next();
            if first == Some(PREFER_ILO) {
                if let Some(command) = words.next().and_then(named) {
                    keys.set_prefers_ilo(command, true);
                }
                continue;
            }
            let Some(command) = first.and_then(named) else {
                continue;
            };
            let set = words.filter(|key| valid(key)).map(str::to_string);
            keys.set(command, set.collect()).ok();
        }
        keys
    }
}

/// Changes the user's keys with `change`, and gives GPUI's bindings and the menus the keys of the
/// commands it changed. The command bar and the page's keys ask the store each time.
pub fn change_keys<R>(cx: &mut App, change: impl FnOnce(&mut UserKeys) -> R) -> R {
    let before = user_keys(cx).clone();
    let mut after = before.clone();
    let result = change(&mut after);
    let mut changed: Vec<Command> = Vec::new();
    for &(command, _) in before.set.iter().chain(&after.set) {
        if !changed.contains(&command) && before.keys(command) != after.keys(command) {
            changed.push(command);
        }
    }
    // GPUI's keymap can't take a binding out. A later NoAction on the same key and context
    // disables the earlier ones, for keystrokes and for the menus' key equivalents, and the
    // bindings after it win again.
    let old = changed.iter().flat_map(|&command| before.keys(command));
    let old: Vec<KeyBinding> = old.flat_map(|key| bindings_of(key, Box::new(NoAction), cx)).collect();
    let app: &App = cx;
    let new = changed.iter().flat_map(|&command| {
        let keys = after.keys(command).into_iter();
        keys.flat_map(move |key| bindings_of(key, Box::new(Run(command)), app))
    });
    let new: Vec<KeyBinding> = new.collect();
    cx.bind_keys(old);
    cx.bind_keys(new);
    // A NoAction on escape takes the note's escape too, which comes back after it.
    if changed.contains(&Command::Stop) {
        cx.bind_keys([super::drm_note::escape_binding()]);
    }
    for &command in &changed {
        let keys = after.keys(command);
        // check.sh waits for these lines.
        match keys.is_empty() {
            true => log!("browser: shortcut {} none", command.key_name()),
            false => log!("browser: shortcut {} {}", command.key_name(), keys.join(" ")),
        }
    }
    cx.set_global(after);
    if let Some(shell) = cx.try_global::<ShellHandle>().map(|handle| handle.0.clone()) {
        shell.update(cx, |shell, cx| shell.refresh_menus(cx)).ok();
    }
    result
}

/// GPUI's bindings for the keys the store has now.
pub(super) fn key_bindings(cx: &App) -> Vec<KeyBinding> {
    (user_keys(cx).bindings().into_iter())
        .flat_map(|(key, command)| bindings_of(key, Box::new(Run(command)), cx))
        .collect()
}

/// Whether the Shortcuts tab's search for `typed` finds the row for `name`, whose keys a menu
/// shows as `keys`: every word typed starts a word of its name, or what's typed names its keys, as
/// "cmd d", "shift cmd a" or ⌘D, which finds them among others held with the same key (Arc).
pub fn shortcut_found(typed: &str, name: &str, keys: Option<&str>) -> bool {
    let name = name.to_lowercase();
    let starts_a_word = |typed: &str| name.split([' ', '/']).any(|word| word.starts_with(typed));
    if typed.split_whitespace().all(|typed| starts_a_word(&typed.to_lowercase())) {
        return true;
    }
    let Some((held, key)) = keys_named_in(typed).filter(|&(held, _)| held != 0) else {
        return false;
    };
    let Some((row_held, row_key)) = keys.and_then(keys_named_in) else {
        return false;
    };
    held & row_held == held && key.is_none_or(|key| Some(key) == row_key)
}

/// The modifiers `text` names, a bit each, and the one other key it names, if any, from words such
/// as "shift cmd a" or a menu's ⇧⌘A or Ctrl+Shift+A. None if it names two other keys.
fn keys_named_in(text: &str) -> Option<(u8, Option<String>)> {
    let mut parts = Vec::new();
    let mut part = String::new();
    for character in text.chars() {
        // A + after a word joins it to the next, as in Ctrl+Shift+A, and one on its own is the key.
        let ends_part = "⌃⌥⇧⌘".contains(character)
            || character.is_whitespace()
            || (character == '+' && !part.is_empty());
        if ends_part {
            parts.extend((!part.is_empty()).then(|| std::mem::take(&mut part)));
            if "⌃⌥⇧⌘".contains(character) {
                parts.push(character.to_string());
            }
        } else {
            part.push(character);
        }
    }
    parts.extend((!part.is_empty()).then_some(part));
    // Off the Mac some keys take two words, as Ctrl+Page Down and Alt+Left Arrow name them.
    for at in (1..parts.len()).rev() {
        let pair = format!("{} {}", parts[at - 1], parts[at]).to_lowercase();
        if key_called(&pair).is_some() {
            parts[at - 1] = pair;
            parts.remove(at);
        }
    }
    let (ctrl, alt, shift, cmd) = (1, 2, 4, 8);
    let (mut held, mut key) = (0, None);
    for part in parts {
        let lower = part.to_lowercase();
        let modifier = match lower.as_str() {
            "⌃" | "ctrl" | "control" => Some(ctrl),
            "⌥" | "alt" | "option" | "opt" => Some(alt),
            "⇧" | "shift" => Some(shift),
            // Off the Mac a shortcut's cmd is pressed as Ctrl, as its menus show it.
            "⌘" | "cmd" | "command" => Some(if OFF_THE_MAC { ctrl } else { cmd }),
            "super" | "win" | "meta" => Some(cmd),
            _ => None,
        };
        match modifier {
            Some(bit) => held |= bit,
            None if key.is_none() => key = Some(key_called(&lower).unwrap_or(lower)),
            None => return None,
        }
    }
    Some((held, key))
}

/// The binding's name for the key `word` names, typed or as a menu shows it, on the Mac or off
/// it: "enter" for "return", ↩ or Enter. None for a letter or another key named as it's written.
fn key_called(word: &str) -> Option<String> {
    let key = match word {
        "return" | "enter" | "↩" => "enter",
        "esc" | "escape" | "⎋" => "escape",
        "tab" | "⇥" => "tab",
        "backspace" | "⌫" => "backspace",
        "delete" | "del" | "⌦" => "delete",
        "up" | "up arrow" | "↑" => "up",
        "down" | "down arrow" | "↓" => "down",
        "left" | "left arrow" | "←" => "left",
        "right" | "right arrow" | "→" => "right",
        "pageup" | "page up" | "⇞" => "pageup",
        "pagedown" | "page down" | "⇟" => "pagedown",
        "home" | "↖" => "home",
        "end" | "↘" => "end",
        _ => return None,
    };
    Some(key.into())
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use gpui::{
        Context, FocusHandle, Modifiers, TestAppContext, VisualTestContext, Window, div,
        prelude::*,
    };

    use super::*;

    /// Whether the Shortcuts tab's search for `typed` finds each of `rows`, a name and the
    /// command's keystroke as a binding writes it, or none.
    fn found(typed: &str, rows: &[(&str, &str)]) -> Vec<bool> {
        let found = |&(name, keys): &(&str, &str)| {
            shortcut_found(typed, name, (!keys.is_empty()).then(|| keystroke_label(keys)).as_deref())
        };
        rows.iter().map(found).collect()
    }

    const ROWS: &[(&str, &str)] = &[
        ("New Tab", "cmd-t"),
        ("Restore Last Closed Tab", "shift-cmd-t"),
        ("Search Tabs", "shift-cmd-a"),
        ("Zoom Out", "cmd--"),
        ("Copy URL", "shift-cmd-c"),
        ("Open Task Manager", ""),
    ];

    /// The search finds a row by the starts of its name's words, in any case and order.
    #[test]
    fn shortcuts_search_finds_rows_by_their_names_words() {
        assert_eq!(found("", ROWS), [true; 6]);
        assert_eq!(found("tab", ROWS), [true, true, true, false, false, false]);
        assert_eq!(found("task", ROWS), [false, false, false, false, false, true]);
        assert_eq!(found("TAB new", ROWS), [true, false, false, false, false, false]);
        assert_eq!(found("clo", ROWS), [false, true, false, false, false, false]);
        assert_eq!(found("ab", ROWS), [false; 6]);
    }

    /// Named as words or as a menu's glyphs, keys find the rows held with them and the same key,
    /// and a row with no keys only by its name.
    #[test]
    fn shortcuts_search_finds_rows_by_their_keys() {
        assert_eq!(found("cmd t", ROWS), [true, true, false, false, false, false]);
        assert_eq!(found("shift cmd t", ROWS), [false, true, false, false, false, false]);
        assert_eq!(found("⇧⌘A", ROWS), [false, false, true, false, false, false]);
        assert_eq!(found("Command+Shift+C", ROWS), [false, false, false, false, true, false]);
        assert_eq!(found("cmd -", ROWS), [false, false, false, true, false, false]);
        assert_eq!(found("shift", ROWS), [false, true, true, false, true, false]);
        assert_eq!(found("ctrl t", ROWS), [false; 6]);
        // Two keys besides the modifiers name no shortcut.
        assert_eq!(found("cmd t a", ROWS), [false; 6]);
    }

    /// A key named by its symbol on the Mac or in words off it finds the row whose chip names it
    /// either way, Page Up and Left Arrow in two words too (#772).
    #[test]
    fn shortcuts_search_finds_keys_by_their_symbols_and_their_words() {
        let found = |typed, keys| shortcut_found(typed, "Row", Some(keys));
        assert!(found("shift cmd return", "⇧⌘↩"));
        assert!(found("shift ctrl enter", "Ctrl+Shift+Enter"));
        assert!(found("ctrl esc", "Ctrl+Esc"));
        assert!(found("ctrl page down", "Ctrl+Page Down"));
        assert!(found("ctrl pagedown", "Ctrl+Page Down"));
        assert!(found("alt left", "Alt+Left Arrow"));
        assert!(found("alt ←", "Alt+Left Arrow"));
        assert!(!found("alt right", "Alt+Left Arrow"));
        assert!(!found("ctrl page up", "Ctrl+Page Down"));
    }

    /// A window of the UI with the keyboard, as the shell's root has it.
    struct Root(FocusHandle);

    impl Render for Root {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().key_context(KEY_CONTEXT).track_focus(&self.0).size_full()
        }
    }

    /// GPUI's bindings follow a change at once: the new key runs the command, the old one
    /// nothing, and the menus show the new one.
    #[gpui::test]
    fn gpui_follows_a_change(cx: &mut TestAppContext) {
        let ran: Rc<RefCell<Vec<Command>>> = Rc::default();
        let runs = ran.clone();
        cx.update(|cx| {
            cx.bind_keys(key_bindings(cx));
            cx.on_action(move |run: &Run, _| runs.borrow_mut().push(run.0));
        });
        let window = cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| {
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    Root(focus)
                })
            })
        });
        let mut cx = VisualTestContext::from_window(window.unwrap().into(), cx);
        cx.simulate_keystrokes("cmd-l");
        assert_eq!(ran.take(), [Command::EditUrl]);
        cx.update(|_, cx| change_keys(cx, |user| user.set(Command::EditUrl, keys(&["cmd-k"]))))
            .unwrap();
        cx.simulate_keystrokes("cmd-l cmd-k");
        assert_eq!(ran.take(), [Command::EditUrl]);
        let shown = cx.update(|_, cx| {
            let keymap = cx.key_bindings();
            let keymap = keymap.borrow();
            let binding = keymap.bindings_for_action(&Run(Command::EditUrl)).next();
            binding.map(|binding| binding.keystrokes()[0].inner().unparse(OperatingSystem::Mac))
        });
        assert_eq!(shown.as_deref(), Some("cmd-k"));
        // Back to ⌘L, which runs it again, and ⌘K nothing.
        cx.update(|_, cx| change_keys(cx, UserKeys::reset_all));
        cx.simulate_keystrokes("cmd-k cmd-l");
        assert_eq!(ran.take(), [Command::EditUrl]);
    }

    /// The sidebar reveal's window draws the shell's panel and has no focus of its own, and ⌘S
    /// there locks the sidebar, as in the window, not whatever a menu's key equivalent finds.
    #[gpui::test]
    fn cmd_s_toggles_the_sidebar_from_the_reveal_panel(cx: &mut TestAppContext) {
        use crate::shell::{Shell, drag::detents::shell_with};

        struct Reveal(gpui::Entity<Shell>);

        impl Render for Reveal {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                self.0.update(cx, |shell, cx| shell.render_reveal(window, cx))
            }
        }

        cx.update(|cx| {
            cx.bind_keys(key_bindings(cx));
            cx.on_action(|run: &Run, cx| {
                let shell = cx.global::<ShellHandle>().0.clone();
                shell.update(cx, |shell, cx| shell.run(run.0, "ui", cx)).ok();
            });
        });
        let (shell, _) = shell_with(cx, &[], &[1], Default::default());
        cx.update(|cx| cx.set_global(ShellHandle(shell.downgrade())));
        let window = cx.update(|cx| cx.open_window(Default::default(), |_, cx| {
            cx.new(|_| Reveal(shell.clone()))
        }));
        let mut cx = VisualTestContext::from_window(window.unwrap().into(), cx);
        // The panel comes out only over a hidden sidebar.
        shell.update(&mut cx, |shell, cx| {
            shell.sidebar_hidden = true;
            shell.set_revealed(true, cx);
        });
        cx.simulate_resize(gpui::size(gpui::px(800.), gpui::px(600.)));
        cx.run_until_parked();
        cx.simulate_keystrokes("cmd-s");
        assert!(!shell.read_with(&cx, |shell, _| shell.sidebar_hidden));
    }

    /// ⌃D in a text field of the UI deletes forward, as in any Mac field, and toggles nothing;
    /// with the UI's keyboard anywhere else it toggles Developer Mode.
    #[gpui::test]
    fn ctrl_d_deletes_forward_in_a_field_and_toggles_developer_mode_elsewhere(
        cx: &mut TestAppContext,
    ) {
        use gpui::Focusable;

        use crate::text_input::TextInput;

        struct Field(FocusHandle, gpui::Entity<TextInput>);

        impl Render for Field {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().key_context(KEY_CONTEXT).track_focus(&self.0).size_full().child(self.1.clone())
            }
        }

        let ran: Rc<RefCell<Vec<Command>>> = Rc::default();
        let runs = ran.clone();
        cx.update(|cx| {
            crate::text_input::bind_keys(cx);
            cx.bind_keys(key_bindings(cx));
            cx.on_action(move |run: &Run, _| runs.borrow_mut().push(run.0));
        });
        let window = cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| {
                    let input = cx.new(|cx| TextInput::new("", cx));
                    input.update(cx, |input, cx| input.set_text("ab".into(), 1, window, cx));
                    window.focus(&input.read(cx).focus_handle(cx), cx);
                    Field(cx.focus_handle(), input)
                })
            })
        });
        let window = window.unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        cx.simulate_keystrokes("ctrl-d");
        let field = window.update(&mut cx, |field, _, _| field.1.clone()).unwrap();
        assert_eq!(field.read_with(&cx, |input, _| input.text().to_string()), "a");
        assert_eq!(ran.take(), []);

        window
            .update(&mut cx, |field, window, cx| window.focus(&field.0, cx))
            .unwrap();
        cx.simulate_keystrokes("ctrl-d");
        assert_eq!(ran.take(), [Command::ToggleDeveloperMode]);
    }

    fn keys(keys: &[&str]) -> Vec<String> {
        keys.iter().map(|key| key.to_string()).collect()
    }

    /// With the page focused on Hebrew, Arabic or Armenian, ⌘'s shortcuts are the keys' on the
    /// Latin layout the core reads, the reserved ones and the rest, even where the layout types
    /// ASCII, as Hebrew's "'" on the W key and "/" on the Q key.
    #[test]
    fn command_keys_on_a_layout_that_isnt_latin_are_the_latin_layouts() {
        use crate::bc::{BC_MODIFIER_CONTROL, BC_MODIFIER_META, BC_MODIFIER_SHIFT};
        let ran = |key, latin, reserved| {
            DEFAULTS.command_for(&page_keystroke(key, latin, BC_MODIFIER_META), reserved)
        };
        assert_eq!(ran("א", "t", true), Some(Command::NewTab));
        assert_eq!(ran("'", "w", true), Some(Command::CloseTab));
        assert_eq!(ran("/", "q", true), Some(Command::Quit));
        assert_eq!(ran("ل", "l", false), Some(Command::EditUrl));
        assert_eq!(ran("ֆ", "f", false), Some(Command::Find));
        // ⇧ comes with the Latin layout's character, as a page's key does on a Latin layout.
        let shifted = BC_MODIFIER_META | BC_MODIFIER_SHIFT;
        assert_eq!(page_keystroke("ט", "T", shifted), page_keystroke("T", "", shifted));
        // A Latin layout's key, which the core gives none for, is its character, as on AZERTY.
        assert_eq!(page_keystroke("a", "", BC_MODIFIER_META), "cmd-a");
        assert_eq!(page_keystroke("ArrowLeft", "", BC_MODIFIER_META), "cmd-left");
        // Without ⌘ the layout's own character stays.
        assert_eq!(page_keystroke("א", "t", BC_MODIFIER_CONTROL), "ctrl-א");
    }

    /// Off the Mac, Ctrl's shortcuts with the page focused are the keys' on the US layout, which
    /// the core reads from Windows' virtual key, on Russian, Hebrew, Arabic and Armenian: Ctrl and
    /// the key Russian types "е" with is Ctrl+T. Alt alone keeps the layout's character, as ⌃
    /// does on the Mac.
    #[test]
    fn ctrl_keys_on_a_layout_that_isnt_latin_are_the_us_layouts_off_the_mac() {
        use crate::bc::{BC_MODIFIER_ALT, BC_MODIFIER_CONTROL, BC_MODIFIER_SHIFT};
        let pressed = |key, latin, modifiers| {
            written_off_the_mac(&pressed_in_page(key, latin, modifiers, true))
        };
        let ran = |key, latin, reserved| {
            DEFAULTS.command_for(&pressed(key, latin, BC_MODIFIER_CONTROL), reserved)
        };
        assert_eq!(ran("е", "t", true), Some(Command::NewTab));
        assert_eq!(ran("'", "w", true), Some(Command::CloseTab));
        assert_eq!(ran("ل", "l", false), Some(Command::EditUrl));
        assert_eq!(ran("ֆ", "f", false), Some(Command::Find));
        assert_eq!(ran("ת", ",", false), ran(",", "", false));
        let shifted = BC_MODIFIER_CONTROL | BC_MODIFIER_SHIFT;
        assert_eq!(pressed("Е", "T", shifted), pressed("T", "", shifted));
        assert_eq!(pressed("е", "t", BC_MODIFIER_ALT), "ctrl-е");
        // ⌘ is Ctrl there, so the Mac's ⌘ the core names isn't one.
        assert_eq!(pressed_in_page("е", "t", crate::bc::BC_MODIFIER_META, true), "cmd-е");
    }

    /// A rebound key runs its command, and the key it had runs nothing.
    /// Off the Mac, ⌘ is Ctrl and ⌃ is Alt, except where Chrome or the desktop keeps the key.
    #[test]
    fn ctrl_is_cmd_and_alt_is_ctrl_off_the_mac() {
        let pressed = |key| pressed_off_the_mac(key);
        assert_eq!(pressed("cmd-t"), ["ctrl-t"]);
        assert_eq!(pressed("ctrl-2"), ["alt-2"]);
        assert_eq!(pressed("ctrl-shift-="), ["alt-shift-="]);
        assert_eq!(pressed("alt-cmd-down"), ["ctrl-alt-down"]);
        assert_eq!(pressed("ctrl-tab"), ["ctrl-tab"]);
        assert_eq!(pressed("cmd-left"), ["ctrl-left", "alt-left"]);
        assert!(pressed("ctrl-left").is_empty() && pressed("ctrl-alt-k").is_empty());
        assert_eq!(written_off_the_mac("ctrl-t"), "cmd-t");
        assert_eq!(written_off_the_mac("alt-2"), "ctrl-2");
        assert_eq!(written_off_the_mac("ctrl-alt-shift-c"), "alt-shift-cmd-c");
        assert_eq!(written_off_the_mac("ctrl-alt-f"), "ctrl-cmd-f");
        assert_eq!(written_off_the_mac("ctrl-shift-tab"), "ctrl-shift-tab");
        assert_eq!(written_off_the_mac("alt-left"), "cmd-left");
        assert_eq!(written_off_the_mac("cmd-t"), "super-t");
    }

    /// Every default key has a way to press it off the Mac that reads back as it, and no two
    /// commands share one, but a key that would be one of Chrome's Alt keys, as ⌃D Alt+D, which
    /// has none.
    #[test]
    fn the_shortcuts_keep_their_keys_off_the_mac() {
        let mut taken: Vec<(String, Command)> = Vec::new();
        for &(key, command, _) in SHORTCUTS {
            let pressed = pressed_off_the_mac(key);
            let alt = key.replacen("ctrl-", "alt-", 1);
            if !key.contains("cmd-") && CHROME_ALT_KEYS.iter().any(|&(chrome, _)| chrome == alt) {
                assert!(pressed.is_empty(), "{key} takes Chrome's key");
                continue;
            }
            assert_eq!(pressed.first().map(|first| written_off_the_mac(first)).as_deref(), Some(key));
            for each in pressed {
                match taken.iter().find(|(other, _)| *other == each) {
                    Some(&(_, other)) => assert_eq!(other, command, "{each}"),
                    None => taken.push((each, command)),
                }
            }
        }
    }

    #[test]
    fn a_rebound_key_runs_its_command_and_the_old_one_nothing() {
        let mut user = UserKeys::default();
        assert_eq!(user.set(Command::EditUrl, keys(&["cmd-k"])), Ok(vec![]));
        assert_eq!(user.command_for("cmd-k", false), Some(Command::EditUrl));
        assert_eq!(user.command_for("cmd-l", false), None);
        assert_eq!(user.label(Command::EditUrl).as_deref(), Some("⌘K"));
        assert_eq!(user.set(Command::EditUrl, vec![]), Ok(vec![]));
        assert_eq!(user.command_for("cmd-k", false), None);
        assert_eq!(user.label(Command::EditUrl), None);
    }

    /// A tooltip shows a chip for each key of the shortcut a menu shows first, in the menus'
    /// order, and none for a command no key runs.
    #[test]
    fn a_shortcut_is_a_chip_for_each_key() {
        let mut user = UserKeys::default();
        assert_eq!(user.chips(Command::ViewLibrary), ["⇧", "⌘", "L"]);
        assert_eq!(user.chips(Command::Back), ["⌘", "["]);
        assert_eq!(user.chips(Command::Stop), ["⌘", "."]);
        user.set(Command::ViewLibrary, keys(&["ctrl-alt-space"])).unwrap();
        assert_eq!(user.chips(Command::ViewLibrary), ["⌃", "⌥", "Space"]);
        user.set(Command::ViewLibrary, vec![]).unwrap();
        assert!(user.chips(Command::ViewLibrary).is_empty());
    }

    /// Keys typed right after ⌘L or ⌘F with the page focused wait for the field it opens, on the
    /// user's keys too, and keys after the other shortcuts don't.
    #[test]
    fn keys_after_a_shortcut_that_opens_a_field_wait_for_it() {
        let mut user = UserKeys::default();
        assert!(user.holds_keys("cmd-l") && user.holds_keys("cmd-f") && user.holds_keys("ctrl-+"));
        assert!(!user.holds_keys("cmd-t") && !user.holds_keys("cmd-r") && !user.holds_keys("a"));
        user.set(Command::Find, keys(&["cmd-k"])).unwrap();
        assert!(user.holds_keys("cmd-k") && !user.holds_keys("cmd-f"));
    }

    /// A reserved command stays reserved on its new key, which the page then never sees, and the
    /// key it left is the page's.
    #[test]
    fn reserved_stays_reserved_on_its_new_key() {
        let mut user = UserKeys::default();
        user.set(Command::NewTab, keys(&["ctrl-cmd-t"])).unwrap();
        assert_eq!(user.command_for("ctrl-cmd-t", true), Some(Command::NewTab));
        assert_eq!(user.command_for("ctrl-cmd-t", false), None);
        assert_eq!(user.command_for("cmd-t", true), None);
        assert_eq!(user.command_for("cmd-t", false), None);
    }

    /// Each of ⌃1–⌃9's Spaces takes keys of its own, as Arc's do, and keeps them by its number
    /// across a relaunch; the one it gave up shows no Space.
    #[test]
    fn a_spaces_keys_change_by_its_number() {
        let mut user = UserKeys::default();
        assert_eq!(user.set(Command::SelectSpace(1), keys(&["alt-cmd-2"])), Ok(vec![]));
        assert_eq!(user.command_for("alt-cmd-2", false), Some(Command::SelectSpace(1)));
        assert_eq!(user.command_for("ctrl-2", false), None);
        assert_eq!(user.keys(Command::SelectSpace(0)), ["ctrl-1"]);
        // ⌃3 moves to the first Space, and the third has none.
        assert_eq!(user.set(Command::SelectSpace(0), keys(&["ctrl-3"])), Ok(vec![Command::SelectSpace(2)]));
        assert!(user.keys(Command::SelectSpace(2)).is_empty());
        let saved = user.to_text();
        assert!(saved.contains("select-space-2 alt-cmd-2\n"), "{saved}");
        assert_eq!(UserKeys::parse(&saved), user);
    }

    /// A key another command has moves to the one it's given, leaving the other without it, but a
    /// reserved command's, Quit's and the system's are refused.
    #[test]
    fn a_taken_key_moves_unless_it_is_reserved() {
        let mut user = UserKeys::default();
        assert_eq!(user.set(Command::EditUrl, keys(&["shift-cmd-k"])), Ok(vec![Command::Clear]));
        assert_eq!(user.command_for("shift-cmd-k", false), Some(Command::EditUrl));
        assert!(user.keys(Command::Clear).is_empty());
        // A command with two keys keeps the other.
        user.set(Command::Find, keys(&["cmd-["])).unwrap();
        assert_eq!(user.keys(Command::Back), ["cmd-left"]);
        let before = user.clone();
        let taken = |command| Err(Refused::Taken(Holder::Command(command)));
        assert_eq!(user.set(Command::EditUrl, keys(&["cmd-t"])), taken(Command::NewTab));
        assert_eq!(user.set(Command::EditUrl, keys(&["cmd-q"])), taken(Command::Quit));
        assert_eq!(user.set(Command::EditUrl, keys(&["cmd-1"])), taken(Command::SelectTab(0)));
        let system = Err(Refused::Taken(Holder::System("Copy")));
        assert_eq!(user.set(Command::EditUrl, keys(&["cmd-c"])), system);
        assert_eq!(user.set(Command::Quit, keys(&["cmd-k"])), taken(Command::Quit));
        assert_eq!(user, before);
    }

    /// Restoring a command's default takes its key back, and Restore Defaults puts every one back.
    #[test]
    fn reset_restores_the_defaults() {
        let mut user = UserKeys::default();
        user.set(Command::EditUrl, keys(&["shift-cmd-k"])).unwrap();
        assert_eq!(user.reset(Command::Clear), Ok(vec![Command::EditUrl]));
        assert_eq!(user.command_for("shift-cmd-k", false), Some(Command::Clear));
        assert!(user.is_set(Command::EditUrl) && !user.is_set(Command::Clear));
        user.set(Command::NewTab, keys(&["ctrl-cmd-t"])).unwrap();
        user.reset_all();
        assert_eq!(user, UserKeys::default());
        assert_eq!(user.command_for("cmd-t", true), Some(Command::NewTab));
    }

    /// What Settings saves reads back the same, and a line naming no command, or a command the
    /// user can't rebind, is left out.
    #[test]
    fn saved_keys_read_back() {
        let mut user = UserKeys::default();
        user.set(Command::EditUrl, keys(&["cmd-k"])).unwrap();
        user.set(Command::ToggleSidebar, vec![]).unwrap();
        user.set(Command::NewFolder, keys(&["alt-cmd-o"])).unwrap();
        assert_eq!(UserKeys::parse(&user.to_text()), user);
        user.set_favorites_numbered(false);
        assert_eq!(UserKeys::parse(&user.to_text()), user);
        user.set_last_tab_ninth(true);
        assert_eq!(UserKeys::parse(&user.to_text()), user);
        assert_eq!(UserKeys::parse("quit cmd-k\nnothing cmd-j\n"), UserKeys::default());
    }

    /// Prefer ilo has a shortcut the page gets first reach ilo before the page, as Arc's Prefer Arc
    /// does, kept with the keys, and ↺ or Restore Defaults gives the page it first again. A
    /// reserved command's keys are ilo's whatever is chosen.
    #[test]
    fn prefer_ilo_runs_a_shortcut_before_the_page() {
        let mut user = UserKeys::default();
        assert_eq!(user.command_for("cmd-f", false), Some(Command::Find));
        user.set_prefers_ilo(Command::Find, true);
        assert_eq!(user.command_for("cmd-f", true), Some(Command::Find));
        assert_eq!(user.command_for("cmd-f", false), None);
        assert!(user.prefers_ilo(Command::Find) && user.changed(Command::Find));
        assert_eq!(UserKeys::parse(&user.to_text()), user);
        assert_eq!(user.to_text(), "prefer-ilo find\n");
        // Its keys moved, the preference goes with the command.
        user.set(Command::Find, keys(&["ctrl-cmd-y"])).unwrap();
        assert_eq!(user.command_for("ctrl-cmd-y", true), Some(Command::Find));
        assert_eq!(UserKeys::parse(&user.to_text()), user);
        user.reset(Command::Find).unwrap();
        assert_eq!(user.command_for("cmd-f", false), Some(Command::Find));
        assert!(!user.changed(Command::Find));
        user.set_prefers_ilo(Command::Find, true);
        user.reset_all();
        assert!(!user.prefers_ilo(Command::Find));

        assert!(!UserKeys::can_prefer(Command::NewTab) && !UserKeys::can_prefer(Command::Quit));
        user.set_prefers_ilo(Command::NewTab, true);
        assert!(!user.prefers_ilo(Command::NewTab) && user.before_page(Command::NewTab));
        assert_eq!(UserKeys::parse("prefer-ilo new-tab\nprefer-ilo quit\n"), UserKeys::default());
    }

    /// A shortcut the page kept runs ilo's command on a second press within a second, as Arc's
    /// does, in the same tab, and the press after that is a first again. A key repeating as it's
    /// held, a press the page didn't keep and another key in between aren't second presses.
    #[test]
    fn a_second_press_of_a_kept_shortcut_runs_ilos() {
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let mut second = SecondPress::default();
        assert!(!second.press(1, "cmd-f", true, false, at(0)));
        assert!(!second.press(1, "cmd-f", true, true, at(200)), "a repeat was a press");
        assert!(second.press(1, "cmd-f", true, false, at(400)));
        assert!(!second.press(1, "cmd-f", true, false, at(600)), "a third press ran it again");
        assert!(!second.press(1, "cmd-f", true, false, at(1700)), "1.1 s apart ran it");
        assert!(!second.press(2, "cmd-f", true, false, at(1800)), "another tab's ran it");
        assert!(!second.press(2, "a", false, false, at(1900)));
        assert!(!second.press(2, "cmd-f", true, false, at(2000)), "a key between didn't count");
        second.declined("cmd-f");
        assert!(!second.press(2, "cmd-f", true, false, at(2100)), "a declined press counted");
        assert!(second.press(2, "cmd-f", true, false, at(2200)));
    }

    /// The favorites count in ⌘1–⌘8 and ⌘9 is the last tab until the user changes them, and
    /// Restore Defaults changes them back.
    #[test]
    fn favorites_are_numbered_by_default() {
        let mut user = UserKeys::default();
        assert!(user.favorites_numbered() && !user.last_tab_ninth());
        assert_eq!(user.to_text(), "");
        user.set_favorites_numbered(false);
        user.set_last_tab_ninth(true);
        assert!(!user.favorites_numbered() && user.last_tab_ninth());
        user.reset_all();
        assert!(user.favorites_numbered() && !user.last_tab_ninth());
    }

    /// A key needs ⌘ or ⌃, as one without would type, and ⇧ with a character a page names its
    /// way too.
    #[test]
    fn recorded_keys_are_written_as_the_shortcuts_are() {
        let keystroke = |control, shift, platform, key: &str| Keystroke {
            modifiers: Modifiers { control, shift, platform, ..Default::default() },
            key: key.into(),
            key_char: None,
        };
        assert_eq!(recorded(&keystroke(true, false, true, "t")), Ok(keys(&["ctrl-cmd-t"])));
        assert_eq!(recorded(&keystroke(false, true, true, "k")), Ok(keys(&["shift-cmd-k"])));
        let plus = recorded(&keystroke(false, false, true, "+"));
        assert_eq!(plus, Ok(keys(&["cmd-+", "shift-cmd-+"])));
        assert_eq!(recorded(&keystroke(false, false, false, "f5")), Ok(keys(&["f5"])));
        assert_eq!(recorded(&keystroke(false, true, false, "k")), Err(Refused::Plain));
    }
}
