use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, point, px};

use crate::shell::{Command, Folders, Page, Shell, bind_keys, recorder::Recorder};

/// The profile's folder, which the browser would give, removed when the test ends.
struct Folder(PathBuf);

impl Folder {
    fn new() -> Self {
        static MADE: AtomicUsize = AtomicUsize::new(0);
        let made = MADE.fetch_add(1, Ordering::Relaxed);
        let name = format!("ilo-boost-card-{}-{made}", std::process::id());
        let folder = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&folder).unwrap();
        Self(folder)
    }

    fn boost(&self, site: &str) -> Option<String> {
        std::fs::read_to_string(self.0.join("Boosts").join(format!("{site}.css"))).ok()
    }

    fn script(&self, site: &str) -> Option<String> {
        std::fs::read_to_string(self.0.join("Boosts").join(format!("{site}.js"))).ok()
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A window on profile 1's Space, its tab 10 on example.com and 11 on a file.
fn window(
    cx: &mut TestAppContext,
) -> (Entity<Shell>, Rc<Recorder>, Folder, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let folder = Folder::new();
    host.profile_folders
        .borrow_mut()
        .insert(1, folder.0.clone());
    let browser = host.clone();
    cx.update(bind_keys);
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11],
            Vec::new(),
            Folders::default(),
            cx,
        );
        let page = |url: &str| Page {
            url: url.to_owned().into(),
            ..Page::default()
        };
        shell.set_page(10, page("https://www.example.com/news"), cx);
        shell.set_page(11, page("file:///tmp/notes.html"), cx);
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    host.take();
    (shell, host, folder, cx)
}

fn offered(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> [bool; 3] {
    shell.read_with(cx, |shell, _| {
        [Command::NewBoost, Command::EditBoost, Command::ViewBoosts].map(|each| shell.offers(each))
    })
}

/// Where the card was drawn, after the frame.
fn card(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<gpui::Bounds<gpui::Pixels>> {
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| shell.boost_card_drawn.get())
}

/// How far over the card's bottom the middle of its last line of buttons is, and of the line
/// over that: Edit CSS… and Zap's, over Customize….
const LAST: f32 = 26.;
const OVER_LAST: f32 = 62.;

/// Clicks the card's button at `along` the line `up` from its bottom, 0.25 for the left of two
/// and 0.75 the right.
fn press(shell: &Entity<Shell>, along: f32, up: f32, cx: &mut VisualTestContext) {
    let card = card(shell, cx).expect("no Boost card");
    let at = point(
        card.origin.x + card.size.width * along,
        card.origin.y + card.size.height - px(up),
    );
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

/// New Boost for a site without one, Edit Boost once it has one, and View Boosts, each only on an
/// http(s) page and with Settings' switch on.
#[gpui::test]
fn the_command_bar_offers_new_boost_then_edit_boost(cx: &mut TestAppContext) {
    let (shell, _, _folder, cx) = window(cx);
    assert_eq!(offered(&shell, cx), [true, false, true]);
    shell.update(cx, |shell, cx| shell.run(Command::NewBoost, "ui", cx));
    shell.update(cx, |shell, cx| shell.boost_zapped(10, Some("#ad"), cx));
    assert_eq!(
        offered(&shell, cx),
        [true, false, true],
        "a click without Zap made a Boost"
    );
    shell.update(cx, |shell, cx| {
        shell.start_zap(cx);
        shell.boost_zapped(10, Some("#ad"), cx);
    });
    assert_eq!(offered(&shell, cx), [false, true, true]);
    shell.update(cx, |shell, cx| shell.set_selected(11, cx));
    assert_eq!(
        offered(&shell, cx),
        [false, false, true],
        "a file was offered a Boost"
    );
    crate::boosts::with(|boosts| boosts.set_all_off(true));
    assert_eq!(offered(&shell, cx), [false, false, false]);
}

/// The card's Zap turns Zap on; each click the core reports is hidden at once and kept in the
/// site's file, Undo takes the last back, and escape ends Zap, then closes the card.
#[gpui::test]
fn zap_hides_each_click_and_undo_takes_it_back(cx: &mut TestAppContext) {
    let (shell, host, folder, cx) = window(cx);
    shell.update(cx, |shell, cx| shell.run(Command::NewBoost, "ui", cx));
    assert!(card(&shell, cx).is_some(), "New Boost opened no card");
    press(&shell, 0.75, OVER_LAST, cx);
    assert_eq!(host.take(), ["zap 10 true"]);

    shell.update(cx, |shell, cx| {
        shell.boost_zapped(10, Some("#ad"), cx);
        shell.boost_zapped(10, None, cx);
        shell.boost_zapped(10, Some("main > aside:nth-of-type(2)"), cx);
    });
    let ad = "#ad { display: none !important; } /* Zap */";
    let aside = "main > aside:nth-of-type(2) { display: none !important; } /* Zap */";
    assert_eq!(
        host.take(),
        [
            format!("boost 1 example.com: {ad}"),
            format!("boost 1 example.com: {ad}\n{aside}"),
        ]
    );
    assert_eq!(
        folder.boost("example.com"),
        Some(format!("{ad}\n{aside}\n"))
    );

    // Undo is the left of the zapping card's two buttons, its last line.
    press(&shell, 0.25, LAST, cx);
    assert_eq!(host.take(), [format!("boost 1 example.com: {ad}")]);

    cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(host.take(), ["zap 10 false"]);
    assert!(
        card(&shell, cx).is_some(),
        "the first escape closed the card"
    );
    cx.simulate_keystrokes("escape");
    assert_eq!(
        card(&shell, cx),
        None,
        "the second escape left the card open"
    );
}

/// A new page ends Zap, as the core says, and a click it reports after is dropped.
#[gpui::test]
fn a_new_page_ends_zap(cx: &mut TestAppContext) {
    let (shell, host, folder, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.run(Command::NewBoost, "ui", cx);
        shell.start_zap(cx);
        shell.boost_zap_ended(10, cx);
        shell.boost_zapped(10, Some("#ad"), cx);
    });
    assert_eq!(host.take(), ["zap 10 true"]);
    assert_eq!(folder.boost("example.com"), None);
    assert!(!shell.read_with(cx, |shell, _| shell.zapping(10)));
}

/// Edit CSS… starts the site's file and opens it in the user's editor.
#[gpui::test]
fn edit_css_opens_the_sites_file(cx: &mut TestAppContext) {
    let (shell, host, folder, cx) = window(cx);
    shell.update(cx, |shell, cx| shell.run(Command::NewBoost, "ui", cx));
    press(&shell, 0.25, OVER_LAST, cx);
    let calls = host.take();
    assert_eq!(
        calls.last().map(String::as_str),
        Some("edit example.com.css")
    );
    let start = folder
        .boost("example.com")
        .expect("Edit CSS… started no file");
    assert!(start.starts_with("/* example.com: "), "{start}");
}

/// The card has no Edit Script…: a site with a script and no CSS shows the new Boost's card, whose
/// Customize… opens the editor, whose footer's Edit Script… opens the script as it is (#2297).
#[gpui::test]
fn a_script_without_css_is_reached_through_customize(cx: &mut TestAppContext) {
    let (shell, host, folder, cx) = window(cx);
    cx.update(|_, cx| crate::boost_editor::open(Rc::new(Unseen), cx))
        .expect("no window for the editor");
    let boosts = folder.0.join("Boosts");
    std::fs::create_dir_all(&boosts).unwrap();
    std::fs::write(boosts.join("example.com.js"), "mine();\n").unwrap();
    shell.update(cx, |shell, cx| shell.run(Command::NewBoost, "ui", cx));
    press(&shell, 0.5, LAST, cx);
    assert_eq!(card(&shell, cx), None, "Customize… left the card open");
    assert_eq!(
        cx.update(|_, cx| crate::boost_editor::open_for(cx))
            .as_deref(),
        Some("example.com")
    );
    host.take();
    shell.update(cx, |shell, cx| {
        let request = crate::boost_editor::Request::EditScript;
        shell.boost_requested(1, "example.com", request, cx);
    });
    assert_eq!(
        host.take().last().map(String::as_str),
        Some("edit example.com.js")
    );
    assert_eq!(folder.script("example.com").as_deref(), Some("mine();\n"));
}

/// Library › Boosts' row menu has Edit Script… under Edit CSS…, and each entry after it still does
/// its own thing.
#[gpui::test]
fn the_library_row_edits_the_script(cx: &mut TestAppContext) {
    let (shell, host, folder, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.run(Command::NewBoost, "ui", cx);
        shell.start_zap(cx);
        shell.boost_zapped(10, Some("#ad"), cx);
        shell.stop_zap(cx);
        shell.run(Command::ViewBoosts, "ui", cx);
    });
    cx.run_until_parked();
    host.take();
    let menu = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            shell.boost_menu("example.com".into(), point(px(40.), px(40.)), cx)
        });
    };
    menu(&shell, cx);
    let titles: Vec<String> = host
        .menus
        .borrow()
        .last()
        .unwrap()
        .iter()
        .map(|(title, _)| title.to_string())
        .collect();
    assert_eq!(&titles[..3], ["Edit CSS…", "Edit Script…", "Turn Off"]);
    cx.update(|_, cx| host.choose("Edit Script…", cx));
    assert_eq!(
        host.take().last().map(String::as_str),
        Some("edit example.com.js")
    );
    assert!(folder.script("example.com").is_some());

    menu(&shell, cx);
    cx.update(|_, cx| host.choose("Turn Off", cx));
    cx.run_until_parked();
    let on = crate::boosts::with(|boosts| boosts.get(1, "example.com")).map(|boost| boost.on);
    assert_eq!(on, Some(false), "Turn Off moved and did something else");
}

/// Neither a private window nor an agent's Space makes a Boost.
#[gpui::test]
fn no_boost_in_an_agents_space(cx: &mut TestAppContext) {
    let (shell, host, folder, cx) = window(cx);
    host.space_agents.borrow_mut().insert(1, "claude".into());
    shell.update(cx, |shell, cx| {
        shell.run(Command::NewBoost, "ui", cx);
        shell.start_zap(cx);
        shell.boost_zapped(10, Some("#ad"), cx);
    });
    assert_eq!(offered(&shell, cx), [false, false, false]);
    assert_eq!(card(&shell, cx), None);
    assert_eq!(host.take(), Vec::<String>::new());
    assert_eq!(folder.boost("example.com"), None);
}

/// Site Controls' switch and Library › Boosts' Turn Off take a Boost off its site, and back on.
#[gpui::test]
fn a_boost_turns_off_on_its_site_and_back_on(cx: &mut TestAppContext) {
    let (shell, host, _folder, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.run(Command::NewBoost, "ui", cx);
        shell.start_zap(cx);
        shell.boost_zapped(10, Some("#ad"), cx);
    });
    host.take();
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.boost_state(10)),
        (true, Some(true))
    );
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.boost_state(11)),
        (false, None)
    );
    shell.update(cx, |shell, cx| shell.set_tab_boost_on(10, false, cx));
    assert_eq!(host.take(), ["boost 1 example.com: "]);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.boost_state(10)),
        (true, Some(false))
    );
    shell.update(cx, |shell, cx| shell.set_tab_boost_on(10, true, cx));
    assert_eq!(
        host.take(),
        ["boost 1 example.com: #ad { display: none !important; } /* Zap */"]
    );

    shell.update(cx, |shell, cx| shell.run(Command::ViewBoosts, "ui", cx));
    cx.run_until_parked();
    let listed: Vec<String> = shell.read_with(cx, |shell, _| {
        shell
            .boost_list
            .iter()
            .map(|boost| boost.site.to_string())
            .collect()
    });
    assert_eq!(listed, ["example.com"]);
}

/// The titles of the last menu shown.
fn menu(host: &Recorder) -> Vec<String> {
    let menus = host.menus.borrow();
    menus
        .last()
        .unwrap()
        .iter()
        .map(|(title, _)| title.to_string())
        .collect()
}

/// A Today tab's menu has its Boost after Mute, as Arc's Customize Page…, and opens its card,
/// showing the tab first when it isn't; a file's tab has none.
#[gpui::test]
fn a_tabs_menu_opens_its_boost(cx: &mut TestAppContext) {
    let (shell, host, _folder, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.tab_menu(11, point(px(100.), px(200.)), cx)
    });
    assert!(
        !menu(&host).iter().any(|title| title.contains("Boost")),
        "{:?}",
        menu(&host)
    );
    host.take();
    shell.update(cx, |shell, cx| {
        shell.tab_menu(10, point(px(100.), px(200.)), cx)
    });
    let titles = menu(&host);
    let mute = titles
        .iter()
        .position(|title| title.starts_with("Mute"))
        .unwrap();
    let boost = titles
        .iter()
        .position(|title| title == "New Boost…")
        .expect("no New Boost…");
    assert!(mute < boost, "{titles:?}");
    shell.update(cx, |shell, cx| shell.set_selected(11, cx));
    host.take();
    cx.update(|_, cx| host.choose("New Boost…", cx));
    cx.run_until_parked();
    assert_eq!(host.take(), ["select 10"]);
    assert_eq!(
        card(&shell, cx),
        None,
        "the card opened before its tab showed"
    );
    shell.update(cx, |shell, cx| shell.set_selected(10, cx));
    assert!(
        card(&shell, cx).is_some(),
        "the tab showed without its card"
    );
}

/// The + menu makes the shown page's Boost after New Note, and edits it once there is one.
#[gpui::test]
fn the_plus_menu_makes_then_edits_a_boost(cx: &mut TestAppContext) {
    let (shell, host, _folder, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.plus_menu(point(px(100.), px(700.)), cx)
    });
    let titles = menu(&host);
    let note = titles.iter().position(|title| title == "New Note").unwrap();
    assert_eq!(titles[note + 1], "New Boost…", "{titles:?}");
    cx.update(|_, cx| host.choose("New Boost…", cx));
    assert!(card(&shell, cx).is_some(), "New Boost… opened no card");
    shell.update(cx, |shell, cx| {
        shell.start_zap(cx);
        shell.boost_zapped(10, Some("#ad"), cx);
        shell.close_boost_card(cx);
    });
    shell.update(cx, |shell, cx| {
        shell.plus_menu(point(px(100.), px(700.)), cx)
    });
    assert!(
        menu(&host).contains(&"Edit Boost…".to_string()),
        "{:?}",
        menu(&host)
    );
    cx.update(|_, cx| host.choose("Edit Boost…", cx));
    assert!(card(&shell, cx).is_some(), "Edit Boost… opened no card");
    shell.update(cx, |shell, cx| {
        shell.close_boost_card(cx);
        shell.set_selected(11, cx);
        shell.plus_menu(point(px(100.), px(700.)), cx);
    });
    assert!(
        !menu(&host).iter().any(|title| title.contains("Boost")),
        "{:?}",
        menu(&host)
    );
}

struct Unseen;

impl crate::boost_editor::Host for Unseen {
    fn show(&self, _: u64, _: f32, _: f32, _: &[(f32, f32, f32)]) {}
    fn resize(&self, _: f32, _: &[(f32, f32, f32)]) {}
    fn hide(&self) {}
}

/// Customize…, the card's last line, closes the card for the editor on the site; a change the
/// editor keeps goes in the site's file and to its pages, and its Zap brings the card back
/// zapping.
#[gpui::test]
fn customize_hands_the_site_to_the_editor(cx: &mut TestAppContext) {
    let (shell, host, folder, cx) = window(cx);
    cx.update(|_, cx| crate::boost_editor::open(Rc::new(Unseen), cx))
        .expect("no window for the editor");
    shell.update(cx, |shell, cx| shell.run(Command::NewBoost, "ui", cx));
    press(&shell, 0.5, LAST, cx);
    assert_eq!(card(&shell, cx), None, "Customize… left the card open");
    assert_eq!(
        cx.update(|_, cx| crate::boost_editor::open_for(cx))
            .as_deref(),
        Some("example.com")
    );

    let style = crate::boost_style::Style::default().shuffled(7);
    let block = style.block().expect("a shuffled style has no block");
    shell.update(cx, |shell, cx| {
        let request = crate::boost_editor::Request::Style(style, true);
        shell.boost_requested(1, "example.com", request, cx);
    });
    let file = folder.boost("example.com").expect("the edit made no file");
    assert!(file.contains(&block), "{file}");
    assert!(
        host.take()
            .iter()
            .any(|call| call.starts_with("boost 1 example.com:")),
        "the edit reached no page"
    );

    shell.update(cx, |shell, cx| {
        shell.boost_requested(1, "example.com", crate::boost_editor::Request::Zap, cx)
    });
    assert_eq!(host.take(), ["zap 10 true"]);
    assert!(card(&shell, cx).is_some(), "Zap brought no card back");
}

/// Window › View Boosts… opens Library › Boosts. Settings' switch off takes it away and on gives
/// it back, and an agent's Space has none.
#[gpui::test]
fn window_view_boosts_opens_the_list(cx: &mut TestAppContext) {
    let (shell, host, _folder, cx) = window(cx);
    let offered = |host: &Recorder| host.menu_bar.borrow().boosts;
    shell.update(cx, |shell, cx| shell.boosts_changed(cx));
    assert!(offered(&host));
    shell.update(cx, |shell, cx| shell.run(Command::ViewBoosts, "ui", cx));
    cx.run_until_parked();
    let section = shell.read_with(cx, |shell, _| shell.library_open.then_some(shell.section));
    assert_eq!(section, Some(crate::shell::library::Section::Boosts));
    shell.update(cx, |shell, cx| {
        crate::boosts::with(|boosts| boosts.set_all_off(true));
        shell.boosts_changed(cx);
    });
    assert!(
        !offered(&host),
        "View Boosts… stayed with Settings' switch off"
    );
    shell.update(cx, |shell, cx| {
        crate::boosts::with(|boosts| boosts.set_all_off(false));
        shell.boosts_changed(cx);
    });
    assert!(
        offered(&host),
        "View Boosts… didn't come back with the switch on"
    );
    host.space_agents.borrow_mut().insert(1, "claude".into());
    shell.update(cx, |shell, cx| shell.boosts_changed(cx));
    assert!(!offered(&host), "an agent's Space offered View Boosts…");
}
