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

/// What the core hands over after Arrange's changes on example.com: three, one of them a trade.
const ARRANGED: &str = r#"{"version":1,"changes":[{"kind":"settle","name":"The video","before":{"name":"the article"}},{"kind":"trade","name":"Trending","with":{"name":"the sign-up box"}},{"kind":"float","name":"More like this","pin":"screen"}]}"#;

impl Folder {
    fn arrangement(&self, site: &str) -> Option<String> {
        let name = format!("{site}{}", crate::boosts::arrange::SUFFIX);
        std::fs::read_to_string(self.0.join("Boosts").join(name)).ok()
    }
}

/// The names the site's arrangement lists, in its file's order.
fn arranged(folder: &Folder) -> Vec<String> {
    let bytes = folder.arrangement("example.com").unwrap_or_default();
    let changes = crate::boosts::arrange::read(&bytes).unwrap_or_default();
    changes.into_iter().map(|change| change.title()).collect()
}

/// The card's list of `count` changes ends this far over its bottom, over its two lines of
/// buttons, and each of its rows is this tall, its ↺ or × in the middle of its two lines.
const LIST_END: f32 = 88.;
const ROW: f32 = 42.;

/// Clicks the ↺ or × of the change at `index` of `count` in the card's list.
fn put_back(shell: &Entity<Shell>, index: usize, count: usize, cx: &mut VisualTestContext) {
    let card = card(shell, cx).expect("no Boost card");
    let up = LIST_END + (count - 1 - index) as f32 * ROW + 18.;
    let at = point(
        card.origin.x + card.size.width - px(24.),
        card.origin.y + card.size.height - px(up),
    );
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

/// The calls of Arrange and the Boosts', without the pointer's hovers.
fn arrange_calls(host: &Recorder) -> Vec<String> {
    let calls = host.take().into_iter();
    calls.filter(|call| !call.starts_with("card ")).collect()
}

/// The card's Arrange, beside Customize…, starts Arrange on the page shown in place of the card
/// and of Zap, and escape ends it; what the core saves meanwhile goes in the site's Boost, which
/// the card then counts and lists.
#[gpui::test]
fn arrange_starts_from_the_card_and_its_changes_are_kept(cx: &mut TestAppContext) {
    let (shell, host, folder, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.run(Command::NewBoost, "ui", cx);
        shell.start_zap(cx);
    });
    host.take();
    shell.update(cx, |shell, cx| shell.stop_zap(cx));
    host.take();
    // Arrange is the right of the card's last line of buttons.
    press(&shell, 0.75, LAST, cx);
    assert_eq!(arrange_calls(&host), ["arrange 10 true"]);
    assert_eq!(card(&shell, cx), None, "the card stayed over the page");
    assert!(shell.read_with(cx, |shell, _| shell.arranging(10)));

    shell.update(cx, |shell, cx| {
        shell.arrange_saved(10, "example.com", ARRANGED, cx);
        // Hide is Zap's rule.
        shell.boost_zapped(10, Some("#promo"), cx);
    });
    assert_eq!(
        host.take(),
        ["boost 1 example.com: #promo { display: none !important; } /* Zap */"],
        "the core was given back what it saved"
    );
    assert_eq!(folder.arrangement("example.com").as_deref(), Some(ARRANGED));

    cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(arrange_calls(&host), ["arrange 10 false"]);
    shell.update(cx, |shell, cx| {
        shell.arrange_ended(10, cx);
        shell.run(Command::EditBoost, "ui", cx);
    });
    assert!(card(&shell, cx).is_some());
    let detail = crate::boosts::with(|boosts| boosts.get(1, "example.com"))
        .map(|boost| boost.summary.describe());
    assert_eq!(detail.as_deref(), Some("Hides 1 thing and arranges 3."));

    // Starting Zap ends Arrange, and the other way round.
    press(&shell, 0.75, LAST, cx);
    host.take();
    shell.update(cx, |shell, cx| {
        shell.run(Command::EditBoost, "ui", cx);
        shell.start_zap(cx);
    });
    assert_eq!(arrange_calls(&host), ["arrange 10 false", "zap 10 true"]);
}

/// ↺ puts a change back: it goes from the file, and the core keeps the rest. A change that no
/// longer fits is dimmed with ×, which removes it the same way, and the last one taken back takes
/// the arrangement away.
#[gpui::test]
fn put_back_drops_one_change_and_the_last_ends_the_arrangement(cx: &mut TestAppContext) {
    let (shell, host, folder, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.arrange_saved(10, "example.com", ARRANGED, cx);
        shell.arrangement_fits(10, "example.com", vec![2], cx);
        shell.run(Command::EditBoost, "ui", cx);
    });
    cx.run_until_parked();
    host.take();
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.arrangement_note(10)),
        (Some("1 change no longer fits".into()), None)
    );

    put_back(&shell, 0, 3, cx);
    assert_eq!(
        arrange_calls(&host),
        ["keep arrangement 1 example.com: 2 changes"]
    );
    assert_eq!(arranged(&folder), ["Trending", "More like this"]);
    // The misfit followed its change down the list.
    assert_eq!(
        shell
            .read_with(cx, |shell, _| shell.arrangement_note(10))
            .0
            .as_deref(),
        Some("1 change no longer fits")
    );
    put_back(&shell, 1, 2, cx);
    assert_eq!(
        arrange_calls(&host),
        ["keep arrangement 1 example.com: 1 changes"]
    );
    assert_eq!(arranged(&folder), ["Trending"]);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.arrangement_note(10)),
        (None, None)
    );

    put_back(&shell, 0, 1, cx);
    assert_eq!(
        arrange_calls(&host),
        ["arrangement 1 example.com: 0 changes"]
    );
    assert_eq!(folder.arrangement("example.com"), None);
}

/// A file the core keeps off, as one changed outside ilo, says so in Site Controls and the card,
/// isn't edited from the list, and Turn On keeps it as it is.
#[gpui::test]
fn an_arrangement_changed_outside_ilo_is_off_until_turned_on(cx: &mut TestAppContext) {
    let (shell, host, folder, cx) = window(cx);
    host.arranged.set(Some(crate::boosts::Arranged::Changed));
    let boosts = folder.0.join("Boosts");
    std::fs::create_dir_all(&boosts).unwrap();
    std::fs::write(boosts.join("example.com.arrange.json"), ARRANGED).unwrap();
    shell.update(cx, |shell, cx| shell.run(Command::EditBoost, "ui", cx));
    cx.run_until_parked();
    assert_eq!(host.take(), ["arrangement 1 example.com: 3 changes"]);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.arrangement_note(10)),
        (None, Some("Changed outside ilo, off"))
    );
    // Where the first ↺ would be is the list's words, with nothing to press.
    put_back(&shell, 0, 3, cx);
    assert_eq!(
        arrange_calls(&host),
        Vec::<String>::new(),
        "an off file was editable"
    );
    assert_eq!(folder.arrangement("example.com").as_deref(), Some(ARRANGED));

    shell.update(cx, |shell, cx| shell.turn_on_arrangement(10, cx));
    assert_eq!(host.take(), ["keep arrangement 1 example.com: 3 changes"]);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.arrangement_note(10)),
        (None, None)
    );
    assert_eq!(folder.arrangement("example.com").as_deref(), Some(ARRANGED));
}

/// A file the core refuses for what's in it is off whatever the user does: the card says so, with
/// nothing to press, and Site Controls has no Turn On for it.
#[gpui::test]
fn an_arrangement_the_core_refuses_stays_off(cx: &mut TestAppContext) {
    let (shell, host, folder, cx) = window(cx);
    host.arranged.set(Some(crate::boosts::Arranged::Refused));
    let boosts = folder.0.join("Boosts");
    std::fs::create_dir_all(&boosts).unwrap();
    std::fs::write(boosts.join("example.com.arrange.json"), ARRANGED).unwrap();
    shell.update(cx, |shell, cx| shell.run(Command::EditBoost, "ui", cx));
    cx.run_until_parked();
    assert_eq!(host.take(), ["arrangement 1 example.com: 3 changes"]);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.arrangement_note(10)),
        (None, None)
    );
    put_back(&shell, 0, 3, cx);
    shell.update(cx, |shell, cx| shell.turn_on_arrangement(10, cx));
    assert_eq!(arrange_calls(&host), Vec::<String>::new());
    assert_eq!(folder.arrangement("example.com").as_deref(), Some(ARRANGED));
}

/// Where the core won't run Arrange, the app doesn't count it as on.
#[gpui::test]
fn arrange_is_on_only_where_the_core_says(cx: &mut TestAppContext) {
    let (shell, host, _folder, cx) = window(cx);
    shell.update(cx, |shell, cx| shell.run(Command::NewBoost, "ui", cx));
    host.take();
    host.arrange_refused.set(true);
    press(&shell, 0.75, LAST, cx);
    assert_eq!(arrange_calls(&host), ["arrange 10 true"]);
    assert!(!shell.read_with(cx, |shell, _| shell.arranging(10)));
}

/// Neither a private window nor an agent's Space offers Arrange, and what the core says of one
/// there is dropped: no file, no call.
#[gpui::test]
fn no_arrange_in_a_private_window_or_an_agents_space(cx: &mut TestAppContext) {
    let (shell, host, folder, cx) = window(cx);
    for private in [true, false] {
        shell.update(cx, |shell, cx| {
            shell.private = private;
            if !private {
                host.space_agents.borrow_mut().insert(1, "claude".into());
            }
            shell.run(Command::NewBoost, "ui", cx);
            shell.start_arrange(cx);
            shell.arrange_saved(10, "example.com", ARRANGED, cx);
            shell.arrangement_fits(10, "example.com", vec![0], cx);
            shell.turn_on_arrangement(10, cx);
        });
        assert_eq!(card(&shell, cx), None);
        assert_eq!(host.take(), Vec::<String>::new(), "private {private}");
        assert_eq!(folder.arrangement("example.com"), None);
        assert!(!shell.read_with(cx, |shell, _| shell.arranging(10)));
    }
}

/// Arrange on tab 10's page, from the card, with the calls so far taken.
fn arranging(shell: &Entity<Shell>, host: &Recorder, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, cx| shell.start_arrange(cx));
    assert!(shell.read_with(cx, |shell, _| shell.arranging(10)));
    host.take();
}

/// With the page focused, ↑ and ↓ ask the core for the box around or inside the one selected and
/// escape is Done; other keys, and these once the mode is off, are the page's.
#[gpui::test]
fn arrange_keys_with_the_page_focused(cx: &mut TestAppContext) {
    let (shell, host, _folder, cx) = window(cx);
    arranging(&shell, &host, cx);
    let key = |keystroke: &str, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.run_arrange_key(10, keystroke, cx))
    };
    assert!(key("up", cx));
    assert!(key("down", cx));
    assert!(!key("a", cx));
    assert!(!key("left", cx));
    assert_eq!(
        arrange_calls(&host),
        ["arrange 10 Outer", "arrange 10 Inner"]
    );
    assert!(
        !shell.update(cx, |shell, cx| shell.run_arrange_key(11, "up", cx)),
        "a key in another tab's page went to the mode"
    );

    assert!(key("escape", cx));
    assert_eq!(arrange_calls(&host), ["arrange 10 false"]);
    assert!(!key("up", cx));
    assert!(!key("escape", cx));
    assert_eq!(arrange_calls(&host), Vec::<String>::new());
}

/// With the UI holding the keyboard, the same keys do the same.
#[gpui::test]
fn arrange_keys_with_the_ui_focused(cx: &mut TestAppContext) {
    let (shell, host, _folder, cx) = window(cx);
    arranging(&shell, &host, cx);
    cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
    cx.simulate_keystrokes("up down");
    cx.run_until_parked();
    assert_eq!(
        arrange_calls(&host),
        ["arrange 10 Outer", "arrange 10 Inner"]
    );
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(arrange_calls(&host), ["arrange 10 false"]);
    cx.simulate_keystrokes("up down");
    cx.run_until_parked();
    assert_eq!(arrange_calls(&host), Vec::<String>::new());
}

/// ⌘Z takes back the mode's steps last first: the core's with its Undo, and a Hide, which is Zap's
/// rule, as Zap's Undo does. The save that follows the core's Undo is no step of the user's, and
/// with nothing left ⌘Z does nothing while the mode is on.
#[gpui::test]
fn undo_takes_back_the_modes_steps_last_first(cx: &mut TestAppContext) {
    let (shell, host, folder, cx) = window(cx);
    arranging(&shell, &host, cx);
    shell.update(cx, |shell, cx| {
        shell.arrange_saved(10, "example.com", ARRANGED, cx);
        shell.boost_zapped(10, Some("#promo"), cx);
        shell.arrange_saved(10, "example.com", ARRANGED, cx);
    });
    host.take();
    cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));

    cx.simulate_keystrokes("cmd-z");
    cx.run_until_parked();
    assert_eq!(arrange_calls(&host), ["arrange 10 Undo"]);
    // The core saves what its Undo left.
    shell.update(cx, |shell, cx| {
        shell.arrange_saved(10, "example.com", ARRANGED, cx)
    });
    host.take();

    // With the page focused, as the page hands ⌘Z over.
    assert!(shell.update(cx, |shell, cx| shell.run_arrange_key(10, "cmd-z", cx)));
    let calls = arrange_calls(&host);
    assert!(
        calls.iter().all(|call| !call.contains("#promo")) && !calls.is_empty(),
        "the Hide wasn't taken back: {calls:?}"
    );
    assert_eq!(
        folder
            .boost("example.com")
            .filter(|css| css.contains("#promo")),
        None
    );

    cx.simulate_keystrokes("cmd-z");
    cx.run_until_parked();
    assert_eq!(arrange_calls(&host), ["arrange 10 Undo"]);
    cx.simulate_keystrokes("cmd-z");
    cx.run_until_parked();
    assert_eq!(arrange_calls(&host), Vec::<String>::new());
}

/// Reset takes back the core's changes, so ⌘Z after it has only the Hides left.
#[gpui::test]
fn reset_leaves_undo_the_hides(cx: &mut TestAppContext) {
    let (shell, host, _folder, cx) = window(cx);
    arranging(&shell, &host, cx);
    shell.update(cx, |shell, cx| {
        shell.arrange_saved(10, "example.com", ARRANGED, cx);
        shell.boost_zapped(10, Some("#promo"), cx);
        shell.ask_arrange(10, crate::boosts::arrange::Ask::Reset, cx);
    });
    let calls = arrange_calls(&host);
    assert_eq!(calls.last().map(String::as_str), Some("arrange 10 Reset"));
    shell.update(cx, |shell, cx| {
        shell.arrange_saved(10, "example.com", "", cx)
    });
    host.take();

    assert!(shell.update(cx, |shell, cx| shell.run_arrange_key(10, "cmd-z", cx)));
    let calls = arrange_calls(&host);
    assert!(
        !calls.is_empty() && calls.iter().all(|call| !call.starts_with("arrange ")),
        "⌘Z after Reset asked the core: {calls:?}"
    );
    assert!(shell.update(cx, |shell, cx| shell.run_arrange_key(10, "cmd-z", cx)));
    assert_eq!(arrange_calls(&host), Vec::<String>::new());
}

/// What the core says of the mode is kept while it's on that tab, and goes with it.
#[gpui::test]
fn the_modes_state_goes_with_the_mode(cx: &mut TestAppContext) {
    use crate::boosts::arrange::{State, Step};
    let (shell, host, _folder, cx) = window(cx);
    let selected = State {
        step: Step::Selected,
        selection: Some([20., 40., 300., 120.]),
        can_undo: true,
        ..State::default()
    };
    shell.update(cx, |shell, cx| {
        shell.arrange_state(10, selected.clone(), cx)
    });
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.arrange_shown().cloned()),
        None
    );

    arranging(&shell, &host, cx);
    shell.update(cx, |shell, cx| {
        shell.arrange_state(11, selected.clone(), cx);
    });
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.arrange_shown().cloned()),
        Some(State::default()),
        "another tab's state was kept"
    );
    shell.update(cx, |shell, cx| {
        shell.arrange_state(10, selected.clone(), cx)
    });
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.arrange_shown().cloned()),
        Some(selected)
    );
    shell.update(cx, |shell, cx| shell.arrange_ended(10, cx));
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.arrange_shown().cloned()),
        None
    );
}

/// The mini bar's calls alone.
fn mini_bar_calls(host: &Recorder) -> Vec<String> {
    let calls = host.take().into_iter();
    calls.filter(|call| call.starts_with("mini bar")).collect()
}

/// Presses the mini bar's `press`, as a click on it does.
fn press_mini_bar(host: &Recorder, press: crate::mini_bar::Press, cx: &mut VisualTestContext) {
    let (tab, on_press) = host.mini_bar.borrow().clone().expect("no mini bar");
    cx.update(|_, cx| on_press(tab, press, cx));
    cx.run_until_parked();
}

/// The mini bar shows at the box of the thing selected, over its page: Hide, then Put back once it
/// has a change, and the pin first while it floats. It goes while the page scrolls, when the core
/// gives no box, and with the mode. Its presses are the core's asks.
#[gpui::test]
fn the_mini_bar_follows_the_thing_selected(cx: &mut TestAppContext) {
    use crate::boosts::arrange::{Outcome, State, Step};
    use crate::mini_bar::Press;
    let (shell, host, _folder, cx) = window(cx);
    arranging(&shell, &host, cx);
    let state = |change, on_screen, selection| State {
        step: Step::Selected,
        change,
        on_screen,
        selection,
        ..State::default()
    };
    let set = |state: State, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.arrange_state(10, state, cx));
        cx.run_until_parked();
    };
    let page = shell.read_with(cx, |shell, _| shell.pane_frame(10).unwrap().0);
    let at = |x: f32, y: f32| {
        let (x, y) = (f32::from(page.origin.x) + x, f32::from(page.origin.y) + y);
        format!("{x},{y}")
    };

    set(
        state(Outcome::None, false, Some([20., 40., 300., 120.])),
        cx,
    );
    assert_eq!(
        mini_bar_calls(&host),
        [format!("mini bar 10 at {}: Hide", at(20., 40.))]
    );
    press_mini_bar(&host, Press::Hide, cx);
    assert_eq!(arrange_calls(&host), ["arrange 10 Hide"]);

    set(
        state(Outcome::Trades, false, Some([20., 40., 300., 120.])),
        cx,
    );
    assert_eq!(
        mini_bar_calls(&host),
        [format!("mini bar 10 at {}: Hide, Put back", at(20., 40.))]
    );
    press_mini_bar(&host, Press::PutBack, cx);
    assert_eq!(arrange_calls(&host), ["arrange 10 PutBack"]);

    set(
        state(Outcome::Floats, true, Some([60., 80., 200., 100.])),
        cx,
    );
    assert_eq!(
        mini_bar_calls(&host),
        [format!(
            "mini bar 10 at {}: Stays on screen, Hide, Put back",
            at(60., 80.)
        )]
    );
    press_mini_bar(&host, Press::Pin, cx);
    assert_eq!(arrange_calls(&host), ["arrange 10 ScrollsWithPage"]);
    set(
        state(Outcome::Floats, false, Some([60., 80., 200., 100.])),
        cx,
    );
    host.take();
    press_mini_bar(&host, Press::Pin, cx);
    assert_eq!(arrange_calls(&host), ["arrange 10 StaysOnScreen"]);

    // The page scrolls: no box, so no bar, until it rests.
    set(state(Outcome::Floats, false, None), cx);
    assert_eq!(mini_bar_calls(&host), ["mini bar none"]);
    set(
        state(Outcome::Floats, false, Some([60., 30., 200., 100.])),
        cx,
    );
    assert_eq!(
        mini_bar_calls(&host),
        [format!(
            "mini bar 10 at {}: Scrolls with page, Hide, Put back",
            at(60., 30.)
        )]
    );

    // The same state again asks nothing.
    set(
        state(Outcome::Floats, false, Some([60., 30., 200., 100.])),
        cx,
    );
    assert_eq!(mini_bar_calls(&host), Vec::<String>::new());

    cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(mini_bar_calls(&host), ["mini bar none"]);
}
