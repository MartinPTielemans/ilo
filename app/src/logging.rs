//! Debug logging the user opts into, by area: `BROWSER_LOG=keys,drag` turns on the areas it names,
//! for that run. Nothing logs by default, and lines go to stderr only, never to a file, so a
//! platform's bug can be followed on a machine or a CI run without a temporary commit. A line never
//! holds what the user browses or types: no page content, no URLs, no characters typed.

use std::sync::OnceLock;

/// The areas `BROWSER_LOG` can name.
pub const AREAS: &[&str] = &[
    // The shortcut a key in a page is and where it went: ilo, an extension, the page.
    "keys",
    // Which window takes the keyboard from an overlay, on Windows.
    "focus",
    // The system's file drag from the UI, on Windows: where it starts, the buttons, how it ends.
    "drag",
];

static ON: OnceLock<Vec<&'static str>> = OnceLock::new();

/// Whether `BROWSER_LOG` turns `area` on, one of `AREAS`.
pub fn on(area: &str) -> bool {
    debug_assert!(AREAS.contains(&area), "{area} isn't one of logging::AREAS");
    ON.get_or_init(|| {
        let (on, unknown) = parse(&std::env::var("BROWSER_LOG").unwrap_or_default());
        if !unknown.is_empty() {
            crate::log!(
                "browser: BROWSER_LOG names no area {}; the areas are {}",
                unknown.join(", "),
                AREAS.join(", ")
            );
        }
        on
    })
    .contains(&area)
}

/// The areas `value` names, as `BROWSER_LOG`, and the names in it that are no area.
fn parse(value: &str) -> (Vec<&'static str>, Vec<String>) {
    let mut on = Vec::new();
    let mut unknown = Vec::new();
    for name in value.split(',').map(str::trim).filter(|name| !name.is_empty()) {
        match AREAS.iter().find(|area| area.eq_ignore_ascii_case(name)) {
            Some(area) => on.push(*area),
            None => unknown.push(name.to_string()),
        }
    }
    (on, unknown)
}

/// Writes a line in `area`, as "browser: [area] …", if `BROWSER_LOG` turns it on.
#[macro_export]
macro_rules! log_in {
    ($area:literal, $($arg:tt)*) => {
        if $crate::logging::on($area) {
            $crate::log!("browser: [{}] {}", $area, format_args!($($arg)*));
        }
    };
}

/// The key the web names `key`, with the BC_MODIFIER_ `modifiers` held, as a log line may show it:
/// a named key, such as Enter or F5, or a character with Ctrl or ⌘, which is a shortcut. A
/// character that types, alone, with ⇧, or with ⌥ or AltGr (Ctrl and Alt on Windows), shows as
/// "a character".
pub fn chord(key: &str, modifiers: u32) -> String {
    use crate::bc;
    let has = |flag| modifiers & flag != 0;
    let mut chord = String::new();
    for (flag, name) in [
        (bc::BC_MODIFIER_CONTROL, "ctrl-"),
        (bc::BC_MODIFIER_ALT, "alt-"),
        (bc::BC_MODIFIER_SHIFT, "shift-"),
        (bc::BC_MODIFIER_META, "cmd-"),
    ] {
        if has(flag) {
            chord.push_str(name);
        }
    }
    let shortcut = has(bc::BC_MODIFIER_META)
        || (has(bc::BC_MODIFIER_CONTROL) && !has(bc::BC_MODIFIER_ALT));
    if key.chars().count() == 1 && !shortcut {
        return "a character".to_string();
    }
    chord + key
}

/// For the hosts' C++: whether `BROWSER_LOG` turns on `area`, a C string.
#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
pub extern "C" fn browser_log_on(area: *const std::ffi::c_char) -> bool {
    if area.is_null() {
        return false;
    }
    let area = unsafe { std::ffi::CStr::from_ptr(area) }.to_string_lossy();
    AREAS.contains(&area.as_ref()) && on(&area)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bc;

    #[test]
    fn names_the_areas_it_knows_and_the_rest_apart() {
        assert_eq!(parse(""), (vec![], vec![]));
        assert_eq!(parse("keys, Drag,,"), (vec!["keys", "drag"], vec![]));
        assert_eq!(parse("keys,urls"), (vec!["keys"], vec!["urls".to_string()]));
    }

    #[test]
    fn a_chord_shows_shortcuts_and_named_keys_but_never_what_is_typed() {
        let ctrl = bc::BC_MODIFIER_CONTROL;
        let alt = bc::BC_MODIFIER_ALT;
        let shift = bc::BC_MODIFIER_SHIFT;
        let cmd = bc::BC_MODIFIER_META;
        assert_eq!(chord("t", ctrl), "ctrl-t");
        assert_eq!(chord("T", ctrl | shift), "ctrl-shift-T");
        assert_eq!(chord("t", cmd), "cmd-t");
        assert_eq!(chord("Escape", 0), "Escape");
        assert_eq!(chord("ArrowLeft", alt), "alt-ArrowLeft");
        assert_eq!(chord("a", 0), "a character");
        assert_eq!(chord("A", shift), "a character");
        assert_eq!(chord(" ", 0), "a character");
        assert_eq!(chord("é", 0), "a character");
        // ⌥ on the Mac and AltGr on Windows type characters.
        assert_eq!(chord("ø", alt), "a character");
        assert_eq!(chord("@", ctrl | alt), "a character");
    }
}
