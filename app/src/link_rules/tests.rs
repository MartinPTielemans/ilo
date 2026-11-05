//! Link rules' routing and file, and the Links tab's clicks, typing and drags, on a host that
//! records what the tab keeps and the menus it shows.

use std::cell::{Cell, RefCell};

use gpui::{Modifiers, TestAppContext, VisualTestContext, size};

use super::*;
use crate::tokens::settings::{PAGE_WIDTH, ROW_HEIGHT};

const WORK: u64 = 1;
const HOME: u64 = 2;

fn rule(condition: Condition, text: &str, space: u64) -> Rule {
    Rule {
        condition,
        text: text.to_string(),
        space,
    }
}

/// The first rule a link meets sends it to its Space: a URL holding the text, or one that is it. A rule whose Space is gone, or without text, sends nothing, and
/// everything else goes to its Space, else to the Space in front.
#[test]
fn rules_send_links_to_their_spaces() {
    let rules = Rules {
        rules: vec![
            rule(Condition::Contains, "", HOME),
            rule(Condition::Contains, "Work.Example", WORK),
            rule(Condition::Is, "https://home.example/", HOME),
            rule(Condition::Contains, "home", 9),
            rule(Condition::Contains, "home.example", WORK),
        ],
        otherwise: 0,
    };
    let spaces = [WORK, HOME];
    assert_eq!(
        rules.route("https://mail.work.example/inbox", &spaces),
        (WORK, Some(1))
    );
    assert_eq!(
        rules.route("https://home.example/", &spaces),
        (HOME, Some(2))
    );
    // A longer path isn't it, so the rule after it, as its Space 9 is gone.
    assert_eq!(
        rules.route("https://home.example/more", &spaces),
        (WORK, Some(4))
    );
    assert_eq!(rules.route("https://news.example/", &spaces), (0, None));
    let otherwise = Rules {
        otherwise: HOME,
        ..rules.clone()
    };
    assert_eq!(
        otherwise.route("https://news.example/", &spaces),
        (HOME, None)
    );
    let gone = Rules {
        otherwise: 9,
        ..rules
    };
    assert_eq!(gone.route("https://news.example/", &spaces), (0, None));
}

/// "Is equal to" compares host and path alone, as Arc 1.170's was measured to (#692): the query,
/// the fragment, the scheme, www., case and a trailing slash don't count, a longer path does.
#[test]
fn is_equal_to_compares_host_and_path() {
    let rule = rule(Condition::Is, "example.com/atc-eq", WORK);
    for url in [
        "https://example.com/atc-eq",
        "https://example.com/atc-eq?x=1",
        "https://example.com/atc-eq#frag",
        "http://example.com/atc-eq",
        "https://www.example.com/atc-eq",
        "https://EXAMPLE.com/ATC-EQ",
        "https://example.com/atc-eq/",
        "https://user@example.com/atc-eq",
    ] {
        assert!(rule.matches(url), "{url}");
    }
    for url in [
        "https://example.com/atc-eq/more",
        "https://example.com/atc",
        "https://example.com/",
        "https://sub.example.com/atc-eq",
        "https://example.com:8080/atc-eq",
        "https://other.example/?u=https://example.com/atc-eq",
    ] {
        assert!(!rule.matches(url), "{url}");
    }
    // The text is read the same way, so a URL pasted whole, its query and all, is its page.
    let pasted = Rule {
        text: "HTTPS://www.example.com/atc-eq/?from=mail#top".into(),
        ..rule.clone()
    };
    assert!(pasted.matches("http://example.com/atc-eq"));
    // A host alone is its home page, and a text with no host left meets nothing.
    let home = Rule {
        text: "example.com".into(),
        ..rule.clone()
    };
    assert!(home.matches("https://www.example.com/"));
    assert!(!home.matches("https://example.com/atc-eq"));
    for text in ["https://", "www.", "?x=1"] {
        let empty = Rule {
            text: text.into(),
            ..rule.clone()
        };
        assert!(!empty.matches("https://example.com/"), "{text}");
    }
}

/// "Contains" looks for the text without its scheme or a leading www., on the link too, and
/// ignores case, as Arc 1.170's was measured to (#692); it sees the query and the fragment.
#[test]
fn contains_ignores_the_scheme_www_and_case() {
    let rule = rule(Condition::Contains, "https://example.com/atc-scheme", WORK);
    for url in [
        "http://example.com/atc-scheme",
        "https://www.example.com/atc-scheme",
        "https://EXAMPLE.com/ATC-SCHEME/more",
    ] {
        assert!(rule.matches(url), "{url}");
    }
    let query = Rule {
        text: "tab=Work".into(),
        ..rule.clone()
    };
    assert!(query.matches("https://example.com/?tab=work"));
    assert!(query.matches("https://example.com/#TAB=WORK"));
    // A "://" in the query isn't a scheme.
    let inner = Rule {
        text: "u=https://example.com".into(),
        ..rule.clone()
    };
    assert!(inner.matches("https://other.example/?u=https://example.com/x"));
    let empty = Rule {
        text: "www.".into(),
        ..rule
    };
    assert!(!empty.matches("https://www.example.com/"));
}

/// A link that would open in a link preview goes by the rules too, but only to another Space than
/// the one shown, and only by a rule: everything else's Space is for links from other apps.
#[test]
fn a_rule_sends_a_previews_link_to_another_space() {
    let rules = Rules {
        rules: vec![
            rule(Condition::Contains, "work.example", WORK),
            rule(Condition::Contains, "home.example", HOME),
        ],
        otherwise: HOME,
    };
    let spaces = [WORK, HOME];
    assert_eq!(
        rules.preview_route("https://work.example/doc", &spaces, HOME),
        Some((WORK, 0))
    );
    // Its rule's Space is the one shown, so it stays in the preview, and the next rule isn't tried.
    assert_eq!(
        rules.preview_route("https://work.example/home.example", &spaces, WORK),
        None
    );
    assert_eq!(
        rules.preview_route("https://news.example/", &spaces, WORK),
        None
    );
    let gone = Rules {
        rules: vec![rule(Condition::Contains, "work", 9)],
        otherwise: 0,
    };
    assert_eq!(
        gone.preview_route("https://work.example/", &spaces, HOME),
        None
    );
}

/// The file keeps each rule's Space by its place among the user's Spaces, which come back in
/// their order under new ids, and a Space that's gone as none.
#[test]
fn the_file_keeps_spaces_by_their_place() {
    let rules = Rules {
        rules: vec![
            rule(Condition::Contains, "work\texample", 7),
            rule(Condition::Is, "https://home.example/", 5),
            rule(Condition::Contains, "gone", 9),
        ],
        otherwise: 7,
    };
    let text = rules.to_text(&[5, 7]);
    assert_eq!(
        text,
        "rule\tcontains\t2\twork example\nrule\tis\t1\thttps://home.example/\n\
         rule\tcontains\t-\tgone\notherwise\t2\nseeded\tmeet\n"
    );
    let back = Rules::parse(&text, &[15, 17]);
    assert_eq!(
        back,
        Rules {
            rules: vec![
                rule(Condition::Contains, "work example", 17),
                rule(Condition::Is, "https://home.example/", 15),
                rule(Condition::Contains, "gone", 0),
            ],
            otherwise: 17,
        }
    );
    assert_eq!(
        Rules::parse("rule\tnear\t1\tx\nnonsense\n", &[15]),
        Rules::default()
    );
}

#[derive(Default)]
struct Recorder {
    spaces: Vec<(u64, SharedString)>,
    rules: RefCell<Rules>,
    menus: RefCell<Vec<String>>,
    /// The item the next menu's user picks.
    answer: Cell<Option<usize>>,
    /// Little ilo is off in Settings › Features.
    little_off: Cell<bool>,
}

impl Host for Recorder {
    fn little_windows(&self) -> bool {
        !self.little_off.get()
    }
    fn spaces(&self) -> Vec<(u64, SharedString)> {
        self.spaces.clone()
    }
    fn rules(&self) -> Rules {
        self.rules.borrow().clone()
    }
    fn set_rules(&self, rules: &Rules) {
        self.rules.replace(rules.clone());
    }
    fn show_menu(
        &self,
        items: Vec<MenuEntry>,
        at: Point<Pixels>,
        _: bool,
        _: &Window,
        picked: Box<dyn FnOnce(Option<usize>, &mut App)>,
        cx: &mut App,
    ) {
        let items: Vec<String> = (items.iter())
            .map(|item| match item.checked {
                true => format!("{} (checked)", item.title),
                false => item.title.to_string(),
            })
            .collect();
        let (x, y) = (f32::from(at.x), f32::from(at.y));
        self.menus
            .borrow_mut()
            .push(format!("{x} {y}: {}", items.join(", ")));
        let answer = self.answer.get();
        cx.defer(move |cx| picked(answer, cx));
    }
}

/// The section alone in a window as wide as Settings' page, on a host with two Spaces and `rules`.
fn tab(rules: Rules, cx: &mut TestAppContext) -> (Rc<Recorder>, &mut VisualTestContext) {
    let (host, _, cx) = tab_view(rules, cx);
    (host, cx)
}

/// `tab`, and its view, which Settings passes ⌘Z on to.
fn tab_view(
    rules: Rules,
    cx: &mut TestAppContext,
) -> (Rc<Recorder>, Entity<LinkRulesView>, &mut VisualTestContext) {
    let host = Rc::new(Recorder {
        spaces: vec![(WORK, "Work".into()), (HOME, "Home".into())],
        rules: RefCell::new(rules),
        ..Recorder::default()
    });
    let shown = host.clone();
    let (view, cx) = cx.add_window_view(move |_, cx| LinkRulesView::new(shown, cx));
    cx.simulate_resize(size(px(BOX_WIDTH), px(400.)));
    cx.run_until_parked();
    (host, view, cx)
}

fn click(cx: &mut VisualTestContext, (x, y): (f32, f32)) {
    cx.simulate_click(point(px(x), px(y)), Modifiers::none());
    cx.run_until_parked();
}

/// The page's width in Settings, and the group's places at the window's top-left here: the rows
/// 40 tall, their controls 8 under their tops, the pop-up of the condition 14 in, the Space's
/// pop-up 8 before the trash, the arrow 8 before that, and the trash 24 across, 14 from the end.
const BOX_WIDTH: f32 = PAGE_WIDTH;
const RULE_ROW: f32 = CONTROL_HEIGHT + 2. * SHORT_PADDING;
const CONDITION_LEFT: f32 = ROW_X;
const SPACE_LEFT: f32 = BOX_WIDTH - ROW_X - CONTROL_HEIGHT - RULE_GAP - RULE_SPACE;
const CONDITION_X: f32 = CONDITION_LEFT + 40.;
const SPACE_X: f32 = SPACE_LEFT + 40.;
const ARROW_X: f32 = SPACE_LEFT - RULE_GAP - BUTTON_GLYPH / 2.;
const REMOVE_X: f32 = BOX_WIDTH - ROW_X - CONTROL_HEIGHT / 2.;
fn row_y(index: usize) -> f32 {
    index as f32 * RULE_ROW + RULE_ROW / 2.
}

/// Where `which` pop-up's text was drawn, which its menu puts the chosen item's at.
fn text_at(view: &Entity<LinkRulesView>, which: Popup, cx: &mut VisualTestContext) -> (f32, f32) {
    view.read_with(cx, |view, _| {
        let texts = view.popup_texts.borrow();
        let at = texts
            .iter()
            .find(|&&(each, _)| each == which)
            .expect("drawn")
            .1;
        (f32::from(at.x), f32::from(at.y))
    })
}

/// A click in the middle of Everything else's pop-up, wherever its row's text puts it.
fn otherwise(view: &Entity<LinkRulesView>, cx: &mut VisualTestContext) -> (f32, f32) {
    let (x, y) = text_at(view, Popup::Otherwise, cx);
    (x + 40., y + CONTROL_HEIGHT / 2.)
}

/// + under the box adds a rule to the Space in front, as Arc's New Route goes to Most Recent
/// Space, whose field takes the keyboard, and what's typed there is the rule's text.
#[gpui::test]
fn plus_adds_a_rule_whose_field_takes_the_typing(cx: &mut TestAppContext) {
    let none = Rules {
        rules: Vec::new(),
        otherwise: LITTLE,
    };
    let (host, cx) = tab(none, cx);
    // Under the empty group's one row, 4 in.
    click(
        cx,
        (
            ADD_X + CONTROL_HEIGHT / 2.,
            ROW_HEIGHT + BUTTONS_TOP + CONTROL_HEIGHT / 2.,
        ),
    );
    assert_eq!(
        host.rules(),
        Rules {
            rules: vec![rule(Condition::Contains, "", FRONT)],
            otherwise: LITTLE
        }
    );
    cx.simulate_input("work.example");
    cx.run_until_parked();
    assert_eq!(
        host.rules().rules,
        [rule(Condition::Contains, "work.example", FRONT)]
    );
}

/// A rule's pop-ups open on its condition and Space, checked, and set what's picked; a Space's
/// offers the Space in front, the Spaces, then Little ilo, as Everything else's does.
#[gpui::test]
fn pop_ups_set_a_rules_condition_and_space(cx: &mut TestAppContext) {
    let rules = Rules {
        rules: vec![rule(Condition::Contains, "work", WORK)],
        otherwise: 0,
    };
    let (host, view, cx) = tab_view(rules, cx);
    host.answer.set(Some(1));
    click(cx, (CONDITION_X, row_y(0)));
    host.answer.set(Some(2));
    click(cx, (SPACE_X, row_y(0)));
    let everything = otherwise(&view, cx);
    click(cx, everything);
    // Each menu puts the chosen item's text on its button's: a rule's 10 into its pop-ups, 8
    // under the row's top, and Everything else's at its row's end.
    let (otherwise_x, otherwise_y) = text_at(&view, Popup::Otherwise, cx);
    assert_eq!(
        otherwise_x,
        BOX_WIDTH - ROW_X - RULE_SPACE + CONTROL_PADDING
    );
    assert!(
        otherwise_y > RULE_ROW + BUTTONS_TOP + CONTROL_HEIGHT + GROUP_GAP,
        "{otherwise_y}"
    );
    assert_eq!(
        host.menus.take(),
        [
            format!(
                "{} {}: URL contains (checked), URL is equal to",
                CONDITION_LEFT + CONTROL_PADDING,
                SHORT_PADDING
            ),
            format!(
                "{} {}: The Space in Front, Work (checked), Home, Little ilo",
                SPACE_LEFT + CONTROL_PADDING,
                SHORT_PADDING
            ),
            format!(
                "{otherwise_x} {otherwise_y}: The Space in Front (checked), Work, Home, Little ilo"
            ),
        ]
    );
    assert_eq!(
        host.rules(),
        Rules {
            rules: vec![rule(Condition::Is, "work", HOME)],
            otherwise: HOME
        }
    );
}

/// A rule's row dragged onto another's takes its place, and the trash takes a rule away.
#[gpui::test]
fn a_drag_reorders_the_rules_and_the_trash_removes_one(cx: &mut TestAppContext) {
    let rules = Rules {
        rules: vec![
            rule(Condition::Contains, "work", WORK),
            rule(Condition::Contains, "home", HOME),
        ],
        otherwise: 0,
    };
    let (host, cx) = tab(rules, cx);
    let from = point(px(ARROW_X), px(row_y(1)));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    let to = point(px(ARROW_X), px(row_y(0)));
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let texts = |host: &Recorder| -> Vec<String> {
        host.rules()
            .rules
            .into_iter()
            .map(|rule| rule.text)
            .collect()
    };
    assert_eq!(texts(&host), ["home", "work"]);
    click(cx, (REMOVE_X, row_y(0)));
    assert_eq!(texts(&host), ["work"]);
}

/// ⌘Z brings back the rules the trash took, the last first, each at its place and with its
/// field, until Settings closes; a rule with nothing typed goes without a trace (#1377).
#[gpui::test]
fn undo_brings_back_a_rule_the_trash_took_until_settings_closes(cx: &mut TestAppContext) {
    let rules = Rules {
        rules: vec![
            rule(Condition::Contains, "work", WORK),
            rule(Condition::Is, "https://home.example/", HOME),
            rule(Condition::Contains, "", HOME),
        ],
        otherwise: 0,
    };
    let (host, view, cx) = tab_view(rules.clone(), cx);
    let undo = |cx: &mut VisualTestContext| {
        let done = view.update(cx, |view, cx| view.undo(cx));
        cx.run_until_parked();
        done
    };
    let texts = |host: &Recorder| -> Vec<String> {
        host.rules()
            .rules
            .into_iter()
            .map(|rule| rule.text)
            .collect()
    };
    // The empty rule goes, then the two others, the first each time.
    click(cx, (REMOVE_X, row_y(2)));
    click(cx, (REMOVE_X, row_y(0)));
    click(cx, (REMOVE_X, row_y(0)));
    assert!(texts(&host).is_empty());
    assert!(undo(cx));
    assert_eq!(texts(&host), ["https://home.example/"]);
    assert!(undo(cx));
    assert_eq!(host.rules().rules, rules.rules[..2]);
    // Nothing was typed in the empty one, so there's nothing to bring back.
    assert!(!undo(cx), "⌘Z brought back the empty rule");
    // The fields hold their texts again.
    let fields = view.read_with(cx, |view, cx| {
        view.fields
            .iter()
            .map(|field| field.read(cx).text().to_string())
            .collect::<Vec<_>>()
    });
    assert_eq!(fields, ["work", "https://home.example/"]);
    // Settings closed: what the trash takes from then on stays gone.
    click(cx, (REMOVE_X, row_y(0)));
    view.update(cx, |view, _| view.forget_removed());
    assert!(!undo(cx));
    assert_eq!(texts(&host), ["https://home.example/"]);
}

/// A rule whose Space is gone says No Space, and the log names each rule and everything else's.
#[test]
fn the_rules_read_with_their_spaces_names() {
    let rules = Rules {
        rules: vec![
            rule(Condition::Contains, "work", WORK),
            rule(Condition::Is, "x", 9),
        ],
        otherwise: 0,
    };
    let spaces = [(WORK, SharedString::from("Work"))];
    assert_eq!(
        rules.describe(&spaces),
        "URL contains \"work\" → Work, URL is equal to \"x\" → No Space; \
         everything else The Space in Front"
    );
}

/// Everything else goes to Little ilo until the user picks another, as Arc's goes to Little Arc,
/// and a rule can send its links there too: links from other apps, not a preview's, which is a
/// quick look already. The file names it, and leaves out the default.
#[test]
fn little_ilo_is_the_default_and_a_rules_place() {
    let spaces = [WORK, HOME];
    let rules = Rules::default();
    assert_eq!(
        rules.route("https://news.example/", &spaces),
        (LITTLE, None)
    );
    assert_eq!(
        rules.to_text(&spaces),
        "rule\tcontains\tfront\tmeet.google.com\nseeded\tmeet\n"
    );
    assert_eq!(Rules::parse("", &spaces), rules);
    let names = [(WORK, SharedString::from("Work"))];
    assert_eq!(
        rules.describe(&names),
        "URL contains \"meet.google.com\" → The Space in Front; everything else Little ilo"
    );
    let little = Rules {
        rules: vec![rule(Condition::Contains, "doc", LITTLE)],
        otherwise: 0,
    };
    assert_eq!(
        little.route("https://doc.example/", &spaces),
        (LITTLE, Some(0))
    );
    assert_eq!(little.route("https://news.example/", &spaces), (0, None));
    assert_eq!(
        little.preview_route("https://doc.example/", &spaces, WORK),
        None
    );
    let text = little.to_text(&spaces);
    assert_eq!(
        text,
        "rule\tcontains\tLittle ilo\tdoc\notherwise\t-\nseeded\tmeet\n"
    );
    assert_eq!(Rules::parse(&text, &[15, 17]), little);
}

/// Little ilo, picked last in a rule's pop-up or Everything else's, is where they send links, and
/// the Space in front takes Everything else back from it.
#[gpui::test]
fn the_pop_ups_pick_little_ilo_and_back(cx: &mut TestAppContext) {
    let rules = Rules {
        rules: vec![rule(Condition::Contains, "work", WORK)],
        otherwise: LITTLE,
    };
    let (host, view, cx) = tab_view(rules, cx);
    host.answer.set(Some(3));
    click(cx, (SPACE_X, row_y(0)));
    let otherwise = otherwise(&view, cx);
    host.answer.set(Some(0));
    click(cx, otherwise);
    let picked = Rules {
        rules: vec![rule(Condition::Contains, "work", LITTLE)],
        otherwise: 0,
    };
    assert_eq!(host.rules(), picked);
    host.answer.set(Some(3));
    click(cx, otherwise);
    let menus = host.menus.take();
    let ends = |menu: usize, items: &str| menus[menu].ends_with(&format!(": {items}"));
    assert!(
        ends(1, "The Space in Front, Work, Home, Little ilo (checked)"),
        "{menus:?}"
    );
    assert!(
        ends(2, "The Space in Front (checked), Work, Home, Little ilo"),
        "{menus:?}"
    );
    assert_eq!(host.rules().otherwise, LITTLE);
}

/// A rule can send its links to the Space in front, as Arc's to Most Recent Space: a link from
/// another app opens as a tab in the Space the front window shows, and a preview's link stays in
/// the preview, as that's the Space shown. The file names it.
#[test]
fn a_rule_sends_links_to_the_space_in_front() {
    let spaces = [WORK, HOME];
    let rules = Rules {
        rules: vec![
            rule(Condition::Contains, "call.example", FRONT),
            rule(Condition::Contains, "example", HOME),
        ],
        otherwise: LITTLE,
    };
    assert_eq!(
        rules.route("https://call.example/room", &spaces),
        (0, Some(0))
    );
    assert_eq!(
        rules.preview_route("https://call.example/room", &spaces, WORK),
        None
    );
    let text = rules.to_text(&spaces);
    assert_eq!(
        text,
        "rule\tcontains\tfront\tcall.example\nrule\tcontains\t2\texample\nseeded\tmeet\n"
    );
    assert_eq!(Rules::parse(&text, &spaces), rules);
    let names = [(WORK, SharedString::from("Work"))];
    assert!(
        (rules.describe(&names)).starts_with("URL contains \"call.example\" → The Space in Front,")
    );
}

/// A profile gets the Meet rule once, as Arc seeds its Google Meet route: a new one starts with
/// it, one from before gets it after its own rules, and once the trash has taken it, it stays
/// gone across launches.
#[test]
fn the_meet_rule_is_added_once() {
    let spaces = [WORK, HOME];
    let meet = rule(Condition::Contains, "meet.google.com", FRONT);
    assert_eq!(Rules::parse("", &spaces).rules, [meet.clone()]);
    assert_eq!(
        Rules::default().route("https://meet.google.com/abc-defg-hij", &spaces),
        (0, Some(0))
    );
    assert_eq!(
        Rules::parse("rule\tcontains\t1\twork\n", &spaces).rules,
        [rule(Condition::Contains, "work", WORK), meet]
    );
    let trashed = Rules {
        rules: Vec::new(),
        otherwise: LITTLE,
    };
    let text = trashed.to_text(&spaces);
    assert_eq!(text, "seeded\tmeet\n");
    assert_eq!(Rules::parse(&text, &spaces), trashed);
}

/// With Little ilo off in Settings › Features, a rule to it and everything else to it send links
/// to the Space in front, and the choice stays for when it's on again.
#[test]
fn off_little_ilo_sends_its_links_to_the_space_in_front() {
    let rules = Rules {
        rules: vec![rule(Condition::Contains, "quick", LITTLE)],
        otherwise: LITTLE,
    };
    let spaces = [WORK, HOME];
    assert_eq!(
        rules.route_with("https://quick.example/", &spaces, true),
        (LITTLE, Some(0))
    );
    assert_eq!(
        rules.route_with("https://news.example/", &spaces, true),
        (LITTLE, None)
    );
    assert_eq!(
        rules.route_with("https://quick.example/", &spaces, false),
        (0, Some(0))
    );
    assert_eq!(
        rules.route_with("https://news.example/", &spaces, false),
        (0, None)
    );
    assert_eq!(rules.otherwise, LITTLE);
}

/// With Little ilo off, neither a rule's Space pop-up nor Everything else's offers it.
#[gpui::test]
fn off_the_pop_ups_have_no_little_ilo(cx: &mut TestAppContext) {
    let rules = Rules {
        rules: vec![rule(Condition::Contains, "work", WORK)],
        otherwise: 0,
    };
    let (host, view, cx) = tab_view(rules, cx);
    host.little_off.set(true);
    click(cx, (SPACE_X, row_y(0)));
    let everything = otherwise(&view, cx);
    click(cx, everything);
    let menus = host.menus.take();
    assert_eq!(menus.len(), 2);
    for menu in &menus {
        assert!(
            menu.ends_with("The Space in Front, Work (checked), Home")
                || menu.ends_with("The Space in Front (checked), Work, Home"),
            "{menu}"
        );
    }
}
