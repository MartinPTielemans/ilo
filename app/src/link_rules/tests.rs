//! Link rules' routing and file, and the Links tab's clicks, typing and drags, on a host that
//! records what the tab keeps and the menus it shows.

use std::cell::{Cell, RefCell};

use gpui::{Modifiers, TestAppContext, VisualTestContext, size};

use super::*;

const WORK: u64 = 1;
const HOME: u64 = 2;

fn rule(condition: Condition, text: &str, space: u64) -> Rule {
    Rule { condition, text: text.to_string(), space }
}

/// The first rule a link meets sends it to its Space: a URL holding the text, whatever its case,
/// or one that is exactly it. A rule whose Space is gone, or without text, sends nothing, and
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
    assert_eq!(rules.route("https://mail.work.example/inbox", &spaces), (WORK, Some(1)));
    assert_eq!(rules.route("https://home.example/", &spaces), (HOME, Some(2)));
    // Not exactly it, so the rule after it, as its Space 9 is gone.
    assert_eq!(rules.route("https://www.home.example/", &spaces), (WORK, Some(4)));
    assert_eq!(rules.route("https://news.example/", &spaces), (0, None));
    let otherwise = Rules { otherwise: HOME, ..rules.clone() };
    assert_eq!(otherwise.route("https://news.example/", &spaces), (HOME, None));
    let gone = Rules { otherwise: 9, ..rules };
    assert_eq!(gone.route("https://news.example/", &spaces), (0, None));
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
    assert_eq!(rules.preview_route("https://work.example/doc", &spaces, HOME), Some((WORK, 0)));
    // Its rule's Space is the one shown, so it stays in the preview, and the next rule isn't tried.
    assert_eq!(rules.preview_route("https://work.example/home.example", &spaces, WORK), None);
    assert_eq!(rules.preview_route("https://news.example/", &spaces, WORK), None);
    let gone = Rules { rules: vec![rule(Condition::Contains, "work", 9)], otherwise: 0 };
    assert_eq!(gone.preview_route("https://work.example/", &spaces, HOME), None);
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
         rule\tcontains\t-\tgone\notherwise\t2\n"
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
    assert_eq!(Rules::parse("rule\tnear\t1\tx\nnonsense\n", &[15]), Rules::default());
}

#[derive(Default)]
struct Recorder {
    spaces: Vec<(u64, SharedString)>,
    rules: RefCell<Rules>,
    menus: RefCell<Vec<String>>,
    /// The item the next menu's user picks.
    answer: Cell<Option<usize>>,
}

impl Host for Recorder {
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
        self.menus.borrow_mut().push(format!("{x} {y}: {}", items.join(", ")));
        let answer = self.answer.get();
        cx.defer(move |cx| picked(answer, cx));
    }
}

/// The tab alone in a window as wide as Settings' boxes, on a host with two Spaces and `rules`.
fn tab(rules: Rules, cx: &mut TestAppContext) -> (Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder {
        spaces: vec![(WORK, "Work".into()), (HOME, "Home".into())],
        rules: RefCell::new(rules),
        ..Recorder::default()
    });
    let shown = host.clone();
    let (_, cx) = cx.add_window_view(move |_, cx| LinkRulesView::new(shown, cx));
    cx.simulate_resize(size(px(BOX_WIDTH), px(400.)));
    cx.run_until_parked();
    (host, cx)
}

fn click(cx: &mut VisualTestContext, (x, y): (f32, f32)) {
    cx.simulate_click(point(px(x), px(y)), Modifiers::none());
    cx.run_until_parked();
}

/// The box's width in Settings, and its places at the window's top-left here: the rows 41 apart
/// from y 21, 40 tall with the line under each, the pop-up of the condition from x 11, the
/// arrow ending 8 before the Space's pop-up, and − 24 across at the end, 11 in.
const BOX_WIDTH: f32 = WIDTH - 2. * SIDE;
const CONDITION_X: f32 = 1. + PADDING + 40.;
const SPACE_X: f32 = BOX_WIDTH - 1. - PADDING - CONTROL_HEIGHT - RULE_GAP - 40.;
const ARROW_X: f32 = SPACE_X + 40. - POPUP_WIDTH - RULE_GAP - CHEVRONS / 2.;
const REMOVE_X: f32 = BOX_WIDTH - 1. - PADDING - CONTROL_HEIGHT / 2.;
fn row_y(index: usize) -> f32 {
    1. + index as f32 * (PROFILE_ROW + 1.) + PROFILE_ROW / 2.
}

/// + under the box adds a rule to the first Space, whose field takes the keyboard, and what's
/// typed there is the rule's text.
#[gpui::test]
fn plus_adds_a_rule_whose_field_takes_the_typing(cx: &mut TestAppContext) {
    let (host, cx) = tab(Rules::default(), cx);
    // Under the empty box's one line.
    click(cx, (CONTROL_HEIGHT / 2., PROFILE_ROW + 2. + BUTTONS_TOP + CONTROL_HEIGHT / 2.));
    assert_eq!(
        host.rules(),
        Rules { rules: vec![rule(Condition::Contains, "", WORK)], otherwise: LITTLE }
    );
    cx.simulate_input("work.example");
    cx.run_until_parked();
    assert_eq!(host.rules().rules, [rule(Condition::Contains, "work.example", WORK)]);
}

/// A rule's pop-ups open on its condition and Space, checked, and set what's picked; a Space's
/// offers Little ilo after the Spaces, and Everything else's the Space in front first.
#[gpui::test]
fn pop_ups_set_a_rules_condition_and_space(cx: &mut TestAppContext) {
    let rules = Rules { rules: vec![rule(Condition::Contains, "work", WORK)], otherwise: 0 };
    let (host, cx) = tab(rules, cx);
    host.answer.set(Some(1));
    click(cx, (CONDITION_X, row_y(0)));
    click(cx, (SPACE_X, row_y(0)));
    host.answer.set(Some(2));
    // Under the box of one rule, + and the gap, in the middle of its 44-tall row.
    let otherwise_y = 2. + PROFILE_ROW + BUTTONS_TOP + CONTROL_HEIGHT + SECTION_GAP + 22.;
    click(cx, (BOX_WIDTH - PADDING - POPUP_END - 40., otherwise_y));
    let place = |which| {
        let at = menu_at(which, 1);
        format!("{} {}", f32::from(at.x), f32::from(at.y))
    };
    assert_eq!(
        host.menus.take(),
        [
            format!("{}: URL contains (checked), URL is equal to", place(Popup::Condition(0))),
            format!("{}: Work (checked), Home, Little ilo", place(Popup::Space(0))),
            format!(
                "{}: The Space in Front (checked), Work, Home, Little ilo",
                place(Popup::Otherwise)
            ),
        ]
    );
    assert_eq!(
        host.rules(),
        Rules { rules: vec![rule(Condition::Is, "work", HOME)], otherwise: HOME }
    );
}

/// A rule's row dragged onto another's takes its place, and − takes a rule away.
#[gpui::test]
fn a_drag_reorders_the_rules_and_minus_removes_one(cx: &mut TestAppContext) {
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
        host.rules().rules.into_iter().map(|rule| rule.text).collect()
    };
    assert_eq!(texts(&host), ["home", "work"]);
    click(cx, (REMOVE_X, row_y(0)));
    assert_eq!(texts(&host), ["work"]);
}

/// A rule whose Space is gone says No Space, and the log names each rule and everything else's.
#[test]
fn the_rules_read_with_their_spaces_names() {
    let rules = Rules {
        rules: vec![rule(Condition::Contains, "work", WORK), rule(Condition::Is, "x", 9)],
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
    assert_eq!(rules.route("https://news.example/", &spaces), (LITTLE, None));
    assert_eq!(rules.to_text(&spaces), "");
    assert_eq!(Rules::parse("", &spaces), rules);
    let names = [(WORK, SharedString::from("Work"))];
    assert_eq!(rules.describe(&names), "none; everything else Little ilo");
    let little = Rules { rules: vec![rule(Condition::Contains, "doc", LITTLE)], otherwise: 0 };
    assert_eq!(little.route("https://doc.example/", &spaces), (LITTLE, Some(0)));
    assert_eq!(little.route("https://news.example/", &spaces), (0, None));
    assert_eq!(little.preview_route("https://doc.example/", &spaces, WORK), None);
    let text = little.to_text(&spaces);
    assert_eq!(text, "rule\tcontains\tLittle ilo\tdoc\notherwise\t-\n");
    assert_eq!(Rules::parse(&text, &[15, 17]), little);
}

/// Little ilo, picked last in a rule's pop-up or Everything else's, is where they send links, and
/// the Space in front takes Everything else back from it.
#[gpui::test]
fn the_pop_ups_pick_little_ilo_and_back(cx: &mut TestAppContext) {
    let rules = Rules { rules: vec![rule(Condition::Contains, "work", WORK)], otherwise: LITTLE };
    let (host, cx) = tab(rules, cx);
    host.answer.set(Some(2));
    click(cx, (SPACE_X, row_y(0)));
    let otherwise_y = 2. + PROFILE_ROW + BUTTONS_TOP + CONTROL_HEIGHT + SECTION_GAP + 22.;
    let otherwise = (BOX_WIDTH - PADDING - POPUP_END - 40., otherwise_y);
    host.answer.set(Some(0));
    click(cx, otherwise);
    let picked = Rules { rules: vec![rule(Condition::Contains, "work", LITTLE)], otherwise: 0 };
    assert_eq!(host.rules(), picked);
    host.answer.set(Some(3));
    click(cx, otherwise);
    let menus = host.menus.take();
    let ends = |menu: usize, items: &str| menus[menu].ends_with(&format!(": {items}"));
    assert!(ends(1, "The Space in Front, Work, Home, Little ilo (checked)"), "{menus:?}");
    assert!(ends(2, "The Space in Front (checked), Work, Home, Little ilo"), "{menus:?}");
    assert_eq!(host.rules().otherwise, LITTLE);
}
