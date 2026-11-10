use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use super::*;

/// A profile's folder the test made, removed when it ends.
struct Folder(PathBuf);

impl Folder {
    fn new() -> Self {
        static MADE: AtomicUsize = AtomicUsize::new(0);
        let made = MADE.fetch_add(1, Ordering::Relaxed);
        let name = format!("ilo-boosts-{}-{made}", std::process::id());
        let folder = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&folder).unwrap();
        Self(folder)
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn read(&self, name: &str) -> Option<String> {
        std::fs::read_to_string(self.0.join(name)).ok()
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn change(site: &str, css: &str) -> Change {
    Change {
        profile: 1,
        site: site.into(),
        css: css.into(),
    }
}

#[test]
fn a_site_is_its_host_without_www_on_http_pages_only() {
    assert_eq!(
        site_of("https://www.Example.com:8443/a?b#c").as_deref(),
        Some("example.com")
    );
    assert_eq!(
        site_of("http://user@news.example.co.uk/").as_deref(),
        Some("news.example.co.uk")
    );
    // Its own subdomain is another site, as the core reads it.
    assert_eq!(
        site_of("https://www2.example.com").as_deref(),
        Some("www2.example.com")
    );
    assert_eq!(site_of("https://www./").as_deref(), Some("www."));
    for none in [
        "file:///tmp/a.css",
        "chrome://settings",
        "about:blank",
        "http://[::1]/",
    ] {
        assert_eq!(site_of(none), None, "{none}");
    }
}

#[test]
fn zap_adds_a_rule_undo_takes_the_last_back_and_the_summary_counts_them() {
    let css = "/* mine */\nbody { color: red; }\n";
    let zapped = add_zap(&add_zap(css, "#ad"), "main > aside:nth-of-type(2)");
    assert_eq!(Summary::of(&zapped), Summary { zaps: 2, lines: 1 });
    assert!(
        zapped.ends_with("main > aside:nth-of-type(2) { display: none !important; } /* Zap */\n")
    );
    let undone = undo_zap(&zapped).unwrap();
    assert_eq!(undone, add_zap(css, "#ad"));
    assert_eq!(undo_zap(&undone).unwrap(), css);
    assert_eq!(undo_zap(css), None);
    assert_eq!(undo_zap(&add_zap("", "#ad")).unwrap(), "");
}

#[test]
fn a_boost_says_what_it_does() {
    let say = |zaps, lines| Summary { zaps, lines }.describe();
    assert_eq!(say(3, 12), "Hides 3 things and adds 12 lines of your CSS.");
    assert_eq!(say(1, 0), "Hides 1 thing.");
    assert_eq!(say(0, 1), "Adds 1 line of your CSS.");
    assert_eq!(
        say(0, 0),
        "Empty. Zap what you don't want to see, or add your own CSS."
    );
}

#[test]
fn the_core_is_given_each_sites_css_once_and_again_only_as_it_changes() {
    let folder = Folder::new();
    folder.write("Boosts/example.com.css", "p { color: red; }\n");
    folder.write("Boosts/notes.txt", "not a Boost");
    folder.write("Boosts/News.Example.css", "not a site's name");
    let mut boosts = Boosts::default();
    assert_eq!(
        boosts.load(1, folder.0.clone()),
        vec![change("example.com", "p { color: red; }\n")]
    );
    assert_eq!(boosts.reload(1), Vec::new());

    // Edited in the user's editor, then a second Boost, then the first deleted there.
    folder.write("Boosts/example.com.css", "p { color: blue; }\n");
    folder.write("Boosts/wiki.org.css", "nav { display: none; }\n");
    assert_eq!(
        boosts.reload(1),
        vec![
            change("example.com", "p { color: blue; }\n"),
            change("wiki.org", "nav { display: none; }\n"),
        ]
    );
    std::fs::remove_file(folder.0.join("Boosts/example.com.css")).unwrap();
    assert_eq!(boosts.reload(1), vec![change("example.com", "")]);
    assert_eq!(
        boosts
            .list(1)
            .iter()
            .map(|boost| boost.site.as_ref())
            .collect::<Vec<_>>(),
        ["wiki.org"]
    );
}

#[test]
fn a_site_turned_off_and_settings_all_off_show_nothing_until_turned_on() {
    let folder = Folder::new();
    folder.write("Boosts/example.com.css", "p {}\n");
    folder.write("Boosts/wiki.org.css", "nav {}\n");
    let mut boosts = Boosts::default();
    boosts.load(1, folder.0.clone());

    assert_eq!(
        boosts.set_on(1, "example.com", false),
        vec![change("example.com", "")]
    );
    assert_eq!(folder.read(OFF_FILE).as_deref(), Some("example.com\n"));
    assert!(!boosts.get(1, "example.com").unwrap().on);
    // Off is kept beside the folder, so it's still off once read again.
    let mut again = Boosts::default();
    assert_eq!(
        again.load(1, folder.0.clone()),
        vec![change("wiki.org", "nav {}\n")]
    );

    assert_eq!(boosts.set_all_off(true), vec![change("wiki.org", "")]);
    assert_eq!(boosts.set_on(1, "example.com", true), Vec::new());
    assert_eq!(folder.read(OFF_FILE), None);
    assert_eq!(
        boosts.set_all_off(false),
        vec![
            change("example.com", "p {}\n"),
            change("wiki.org", "nav {}\n")
        ]
    );
}

#[test]
fn zap_starts_a_boost_turns_it_on_and_undo_and_delete_take_it_back() {
    let folder = Folder::new();
    let mut boosts = Boosts::default();
    assert_eq!(boosts.load(1, folder.0.clone()), Vec::new());
    assert_eq!(boosts.get(1, "example.com"), None);

    let rule = "#ad { display: none !important; } /* Zap */\n";
    assert_eq!(
        boosts.zap(1, "example.com", "#ad"),
        vec![change("example.com", rule)]
    );
    assert_eq!(folder.read("Boosts/example.com.css").as_deref(), Some(rule));
    boosts.set_on(1, "example.com", false);
    // Zapping a Boost that's off turns it back on, so what was clicked goes.
    let both = format!("{rule}.banner {{ display: none !important; }} /* Zap */\n");
    assert_eq!(
        boosts.zap(1, "example.com", ".banner"),
        vec![change("example.com", &both)]
    );
    assert_eq!(boosts.get(1, "example.com").unwrap().summary.zaps, 2);

    assert_eq!(
        boosts.undo_zap(1, "example.com"),
        vec![change("example.com", rule)]
    );
    assert_eq!(
        boosts.delete(1, "example.com"),
        vec![change("example.com", "")]
    );
    assert_eq!(folder.read("Boosts/example.com.css"), None);
    assert_eq!(boosts.get(1, "example.com"), None);
}

#[test]
fn edit_starts_a_file_saying_what_it_is_and_leaves_one_there_as_it_is() {
    let folder = Folder::new();
    folder.write("Boosts/wiki.org.css", "nav {}\n");
    let mut boosts = Boosts::default();
    boosts.load(1, folder.0.clone());
    let path = boosts.file(1, "example.com").unwrap();
    assert_eq!(path, folder.0.join("Boosts/example.com.css"));
    let start = std::fs::read_to_string(&path).unwrap();
    assert!(start.starts_with("/* example.com: "), "{start}");
    // A comment alone shows nothing, so the core is given nothing for it.
    assert_eq!(Summary::of(&start), Summary::default());
    boosts.file(1, "wiki.org").unwrap();
    assert_eq!(
        folder.read("Boosts/wiki.org.css").as_deref(),
        Some("nav {}\n")
    );
    // A profile not read has no Boosts to give.
    assert_eq!(boosts.file(2, "example.com"), None);
}

#[test]
fn the_watcher_tells_of_a_write_in_the_folder() {
    let folder = Folder::new();
    let boosts = folder.0.join(FOLDER);
    let (told, tells) = mpsc::channel();
    assert!(watch(boosts.clone(), move || {
        let _ = told.send(());
    }));
    // The watch starts on its thread; a write before it starts would be missed, so write until it
    // tells, which a working watcher does on the first or second.
    let mut heard = false;
    for each in 0..20 {
        std::fs::write(
            boosts.join("example.com.css"),
            format!("p {{ order: {each}; }}"),
        )
        .unwrap();
        if tells.recv_timeout(Duration::from_millis(250)).is_ok() {
            heard = true;
            break;
        }
    }
    assert!(heard, "no notice of a write in {}", boosts.display());
}

fn given(site: &str, script: Option<&str>) -> ScriptChange {
    ScriptChange {
        profile: 1,
        site: site.into(),
        script: script.map(|script| script.as_bytes().to_vec()),
    }
}

#[test]
fn a_script_goes_to_the_core_only_while_its_boost_is_on_and_isnt_too_large() {
    let folder = Folder::new();
    folder.write("Boosts/example.com.css", "p {}\n");
    folder.write("Boosts/example.com.js", "run();\n");
    folder.write("Boosts/wiki.org.js", &"/".repeat(SCRIPT_LIMIT + 1));
    folder.write("Boosts/Not A Site.js", "run();\n");
    let mut boosts = Boosts::default();
    boosts.load(1, folder.0.clone());
    assert_eq!(
        boosts.script_changes(1),
        vec![given("example.com", Some("run();\n"))]
    );
    assert_eq!(boosts.script_changes(1), Vec::new());
    // New until the core says otherwise; one too large says so itself, with no CSS, and isn't
    // given at all.
    let script = |boosts: &Boosts, site| boosts.get(1, site).and_then(|boost| boost.script);
    assert_eq!(script(&boosts, "example.com"), Some(Script::New));
    assert_eq!(script(&boosts, "wiki.org"), Some(Script::TooLarge));
    boosts.set_script_state(1, "example.com", Script::On);
    assert_eq!(script(&boosts, "example.com"), Some(Script::On));

    // Turned off for its site, or all of them, the core gets none, and its state is forgotten.
    boosts.set_on(1, "example.com", false);
    assert_eq!(boosts.script_changes(1), vec![given("example.com", None)]);
    boosts.set_on(1, "example.com", true);
    assert_eq!(
        boosts.script_changes(1),
        vec![given("example.com", Some("run();\n"))]
    );
    assert_eq!(script(&boosts, "example.com"), Some(Script::New));
    boosts.set_all_off(true);
    assert_eq!(
        boosts.all_script_changes(),
        vec![given("example.com", None)]
    );
    boosts.set_all_off(false);
    boosts.all_script_changes();

    // A changed file goes again, and one grown too large is taken away.
    folder.write("Boosts/example.com.js", "run(2);\n");
    boosts.reload(1);
    assert_eq!(
        boosts.script_changes(1),
        vec![given("example.com", Some("run(2);\n"))]
    );
    folder.write("Boosts/example.com.js", &"/".repeat(SCRIPT_LIMIT + 1));
    boosts.reload(1);
    assert_eq!(boosts.script_changes(1), vec![given("example.com", None)]);
    assert_eq!(script(&boosts, "example.com"), Some(Script::TooLarge));
    assert_eq!(boosts.script_bytes(1, "example.com"), None);
}

#[test]
fn turning_on_keeps_a_copy_beside_the_folder_and_turning_off_or_delete_takes_it() {
    let folder = Folder::new();
    folder.write("Boosts/example.com.js", "run();\n");
    let mut boosts = Boosts::default();
    boosts.load(1, folder.0.clone());
    boosts.script_changes(1);
    assert_eq!(boosts.turned_on_copy(1, "example.com"), None);
    boosts.turned_on(1, "example.com", b"run();\n");
    let copy = format!("{TURNED_ON}/example.com.js");
    assert_eq!(folder.read(&copy).as_deref(), Some("run();\n"));
    assert_eq!(
        boosts.turned_on_copy(1, "example.com"),
        Some(b"run();\n".to_vec())
    );
    let script = |boosts: &Boosts| boosts.get(1, "example.com").and_then(|boost| boost.script);
    assert_eq!(script(&boosts), Some(Script::On));
    // The copy is outside the Boosts folder, so the folder never holds it.
    assert_eq!(boosts.list(1).len(), 1);

    boosts.turned_off(1, "example.com");
    assert_eq!(folder.read(&copy), None);
    assert_eq!(script(&boosts), Some(Script::New));

    boosts.turned_on(1, "example.com", b"run();\n");
    boosts.delete(1, "example.com");
    assert_eq!(folder.read(&copy), None);
    assert_eq!(folder.read("Boosts/example.com.js"), None);
    assert_eq!(boosts.get(1, "example.com"), None);
    assert_eq!(boosts.script_changes(1), vec![given("example.com", None)]);
}

#[test]
fn the_states_say_why_a_script_is_off_in_the_same_words_everywhere() {
    assert_eq!(Script::On.off_reason(), None);
    assert_eq!(Script::New.off_reason(), Some("New script, off"));
    assert_eq!(Script::Changed.off_reason(), Some("Script changed, off"));
    assert_eq!(
        Script::TooLarge.off_reason(),
        Some("Script over 256 KB, off")
    );
    assert!(Script::New.can_turn_on() && Script::Changed.can_turn_on());
    assert!(!Script::On.can_turn_on() && !Script::TooLarge.can_turn_on());
}

fn shown(code: &crate::dialog::Code) -> Vec<String> {
    use crate::dialog::LineKind;
    (code.lines.iter())
        .map(|line| {
            let sign = match line.kind {
                LineKind::Same => " ",
                LineKind::Added => "+",
                LineKind::Removed => "-",
                LineKind::Gap => "~",
            };
            let number = line
                .number
                .map_or(String::new(), |number| number.to_string());
            format!("{sign}{number} {}", line.text)
        })
        .collect()
}

#[test]
fn the_dialog_shows_what_a_line_hides() {
    let code = script_text(
        "example.com",
        "a\tb\r\nlet x = \"\u{202e}cba\";\u{200b}\nend\u{feff}",
    );
    assert_eq!(
        shown(&code),
        [
            " 1 a    b\\u{d}",
            " 2 let x = \"\\u{202e}cba\";\\u{200b}",
            " 3 end\\u{feff}",
        ]
    );
    assert_eq!(code.caption.as_ref(), "Boosts/example.com.js · 3 lines");
}

#[test]
fn the_diff_shows_changes_with_two_lines_around_them() {
    let before = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";
    let after = "1\n2\n3\n4\nfive\n6\n7\n8\n9\n10\neleven\n";
    let code = script_diff("example.com", before, after);
    assert_eq!(
        shown(&code),
        [
            "~ 2 lines unchanged",
            " 3 3",
            " 4 4",
            "- 5",
            "+5 five",
            " 6 6",
            " 7 7",
            "~ 1 line unchanged",
            " 9 9",
            " 10 10",
            "+11 eleven",
        ]
    );
    assert_eq!(
        code.caption.as_ref(),
        "Boosts/example.com.js · 2 lines added, 1 line removed"
    );
    // A line's ending changed shows, as the carriage return does.
    let code = script_diff("example.com", "a\nb\n", "a\r\nb\n");
    assert_eq!(shown(&code), ["- a", "+1 a\\u{d}", " 2 b"]);
    // Bytes that change no line, such as the last newline, show the whole text.
    let code = script_diff("example.com", "a\nb\n", "a\nb");
    assert_eq!(shown(&code), [" 1 a", " 2 b"]);
}

#[test]
fn a_diff_of_two_long_scripts_is_still_what_changed() {
    let before: String = (0..3000).map(|line| format!("old {line}\n")).collect();
    let after: String = (0..3000).map(|line| format!("new {line}\n")).collect();
    let (before, after) = (
        format!("head\n{before}tail\n"),
        format!("head\n{after}tail\n"),
    );
    let code = script_diff("example.com", &before, &after);
    let lines = shown(&code);
    assert_eq!(lines.first().map(String::as_str), Some(" 1 head"));
    assert_eq!(lines.last().map(String::as_str), Some(" 3002 tail"));
    assert_eq!(
        lines.iter().filter(|line| line.starts_with('-')).count(),
        3000
    );
    assert_eq!(
        lines.iter().filter(|line| line.starts_with('+')).count(),
        3000
    );
    assert_eq!(lines[1], "- old 0");
    assert_eq!(lines[3001], "+2 new 0");
}
