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
        shows: Shows::Css(css.into()),
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
    assert_eq!(
        Summary::of(&zapped),
        Summary {
            zaps: 2,
            lines: 1,
            arranges: 0
        }
    );
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
    let say = |zaps, arranges, lines| {
        Summary {
            zaps,
            lines,
            arranges,
        }
        .describe()
    };
    assert_eq!(
        say(3, 0, 12),
        "Hides 3 things and adds 12 lines of your CSS."
    );
    assert_eq!(
        say(2, 4, 12),
        "Hides 2 things, arranges 4 and adds 12 lines of your CSS."
    );
    assert_eq!(say(1, 0, 0), "Hides 1 thing.");
    assert_eq!(say(0, 1, 0), "Arranges 1.");
    assert_eq!(say(0, 2, 1), "Arranges 2 and adds 1 line of your CSS.");
    assert_eq!(say(0, 0, 1), "Adds 1 line of your CSS.");
    assert_eq!(
        say(0, 0, 0),
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

fn arranged(site: &str, bytes: &str) -> Change {
    Change {
        profile: 1,
        site: site.into(),
        shows: Shows::Arrangement(bytes.into()),
    }
}

const TWO: &str = r#"{"version":1,"changes":[{"kind":"trade","name":"Trending","with":{"name":"the sign-up box"}},{"kind":"float","name":"More like this","pin":"page"}]}"#;

/// A site's arrangement is part of its Boost: given to the core with its CSS, counted in its
/// summary, off with its switches, and gone with it.
#[test]
fn an_arrangement_goes_with_its_boost() {
    let folder = Folder::new();
    folder.write("Boosts/example.com.css", "p {}\n");
    folder.write("Boosts/example.com.arrange.json", TWO);
    folder.write("Boosts/wiki.org.arrange.json", TWO);
    let mut boosts = Boosts::default();
    assert_eq!(
        boosts.load(1, folder.0.clone()),
        vec![
            change("example.com", "p {}\n"),
            arranged("example.com", TWO),
            arranged("wiki.org", TWO),
        ]
    );
    let boost = boosts.get(1, "example.com").unwrap();
    assert_eq!(
        boost.summary.describe(),
        "Arranges 2 and adds 1 line of your CSS."
    );
    // An arrangement alone is a Boost.
    let listed: Vec<String> = (boosts.list(1).iter())
        .map(|each| each.site.to_string())
        .collect();
    assert_eq!(listed, ["example.com", "wiki.org"]);
    assert_eq!(
        boosts
            .get(1, "wiki.org")
            .unwrap()
            .arrangement
            .unwrap()
            .state,
        None
    );

    assert_eq!(
        boosts.set_on(1, "wiki.org", false),
        vec![arranged("wiki.org", "")]
    );
    assert_eq!(
        boosts.set_all_off(true),
        vec![change("example.com", ""), arranged("example.com", "")]
    );
    boosts.set_all_off(false);
    assert_eq!(
        boosts.delete(1, "example.com"),
        vec![change("example.com", ""), arranged("example.com", "")]
    );
    assert_eq!(folder.read("Boosts/example.com.arrange.json"), None);
}

/// A file over a cap or off its shape is never given to the core, and is listed as one the app
/// can't read; once fixed, it is.
#[test]
fn an_unreadable_arrangement_is_off_whole() {
    let folder = Folder::new();
    folder.write(
        "Boosts/example.com.arrange.json",
        r#"{"version":1,"changes":[{"kind":"resize"}]}"#,
    );
    folder.write(
        "Boosts/wiki.org.arrange.json",
        &" ".repeat(arrange::MAX_BYTES + 1),
    );
    let mut boosts = Boosts::default();
    assert_eq!(boosts.load(1, folder.0.clone()), Vec::new());
    for site in ["example.com", "wiki.org"] {
        let boost = boosts.get(1, site).unwrap();
        assert_eq!(boost.arrangement.unwrap().changes, None, "{site}");
        assert_eq!(boost.summary.arranges, 0);
        assert_eq!(boosts.arrangement(1, site), None);
    }
    folder.write("Boosts/example.com.arrange.json", TWO);
    assert_eq!(boosts.reload(1), vec![arranged("example.com", TWO)]);
}

/// The core's own save is written and not given back to it; putting a change back writes the
/// rest for the core to keep, and the last one taken back removes the file.
#[test]
fn putting_back_each_change_ends_the_arrangement() {
    let folder = Folder::new();
    let mut boosts = Boosts::default();
    boosts.load(1, folder.0.clone());
    boosts.set_on(1, "example.com", false);
    assert_eq!(boosts.save_arrangement(1, "example.com", "{}"), Vec::new());
    assert_eq!(folder.read("Boosts/example.com.arrange.json"), None);
    // Arrange turns the Boost on, as Zap does, with nothing more to give the core.
    assert_eq!(boosts.save_arrangement(1, "example.com", TWO), Vec::new());
    assert_eq!(
        folder.read("Boosts/example.com.arrange.json").as_deref(),
        Some(TWO)
    );
    let boost = boosts.get(1, "example.com").unwrap();
    assert!(boost.on);
    assert_eq!(boost.arrangement.unwrap().state, Some(Arranged::On));
    assert_eq!(boosts.reload(1), Vec::new());

    boosts.set_misfits(1, "example.com", vec![1]);
    let (kept, changes) = boosts.put_back(1, "example.com", 0);
    assert_eq!(changes, Vec::new());
    let kept = kept.unwrap();
    let left: Vec<String> = arrange::read(&kept)
        .unwrap()
        .into_iter()
        .map(|each| each.name)
        .collect();
    assert_eq!(left, ["More like this"]);
    assert_eq!(folder.read("Boosts/example.com.arrange.json"), Some(kept));
    assert_eq!(
        boosts
            .get(1, "example.com")
            .unwrap()
            .arrangement
            .unwrap()
            .misfits,
        [0],
        "the misfit didn't follow its change"
    );

    assert_eq!(
        boosts.put_back(1, "example.com", 0),
        (None, vec![arranged("example.com", "")])
    );
    assert_eq!(folder.read("Boosts/example.com.arrange.json"), None);
    assert_eq!(boosts.get(1, "example.com"), None);
}
