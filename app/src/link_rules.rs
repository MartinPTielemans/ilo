//! Link rules (docs/ux-spec.md › Link rules): the Space a link from another app, or one that would
//! open in a link preview, opens in, or Little ilo, and Settings' Links tab, which makes them.
//! They're the user's alone: agents get no way to read or change them, as with the default
//! browser.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, Context, Entity, Focusable, Hsla, MouseButton, Pixels, Point, SharedString, Subscription,
    Window, canvas, div, point, prelude::*, px, rgb, svg,
};

use crate::settings_rows::{Look, detailed_row, frame, group, icon_button, popup};
use crate::shell::MenuEntry;
use crate::text_input::TextInput;
use crate::tokens::palette::{NO_THEME_ACCENT, SELECTION};
use crate::tokens::settings::{
    ADD_X, BUTTON_GLYPH, BUTTONS_TOP, CONTROL_HEIGHT, CONTROL_PADDING, CONTROL_RADIUS,
    FIELD_PADDING, GROUP_GAP, ROW_X, RULE_CONDITION, RULE_GAP, SHORT_PADDING, RULE_SPACE,
};

#[cfg(test)]
mod tests;

/// What a rule asks of a link's URL: to hold its text anywhere, ignoring case, or to be it exactly,
/// scheme and www included, as Arc's "is equal to".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    Contains,
    Is,
}

impl Condition {
    pub const ALL: [Condition; 2] = [Condition::Contains, Condition::Is];

    pub fn title(self) -> &'static str {
        match self {
            Condition::Contains => "URL contains",
            Condition::Is => "URL is equal to",
        }
    }

    /// Its name in the rules' file.
    fn key(self) -> &'static str {
        match self {
            Condition::Contains => "contains",
            Condition::Is => "is",
        }
    }
}

/// Where a rule or everything else sends a link in place of a Space: a new Little ilo window, for
/// the Space in front. No Space of the core's has this id.
pub const LITTLE: u64 = u64::MAX;

/// Little ilo's name in the pop-ups, the rules' file and check.sh's lines.
const LITTLE_NAME: &str = "Little ilo";

/// A rule: links whose URL meets its condition open in its Space, or Little ilo, 0 for none chosen.
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub condition: Condition,
    pub text: String,
    pub space: u64,
}

impl Rule {
    /// Whether a link to `url` meets it. A rule without text yet meets none.
    pub fn matches(&self, url: &str) -> bool {
        let text = self.text.trim();
        !text.is_empty()
            && match self.condition {
                Condition::Contains => url.to_lowercase().contains(&text.to_lowercase()),
                Condition::Is => url == text,
            }
    }
}

/// The rules, top to bottom, and the Space everything else opens in, 0 for the Space in front:
/// Little ilo unless the user picks another, as Arc's Little Arc (the owner, 2026-10-31).
#[derive(Clone, Debug, PartialEq)]
pub struct Rules {
    pub rules: Vec<Rule>,
    pub otherwise: u64,
}

impl Default for Rules {
    fn default() -> Rules {
        Rules { rules: Vec::new(), otherwise: LITTLE }
    }
}

impl Rules {
    /// Where a link to `url` opens, of the user's `spaces`: the Space of the first rule it meets
    /// whose Space is still there, or Little ilo, with that rule, else everything else's, 0 being
    /// the Space in front.
    pub fn route(&self, url: &str, spaces: &[u64]) -> (u64, Option<usize>) {
        let there = |space: u64| space == LITTLE || spaces.contains(&space);
        let rule = self.rules.iter().position(|rule| there(rule.space) && rule.matches(url));
        match rule {
            Some(index) => (self.rules[index].space, Some(index)),
            None => (if there(self.otherwise) { self.otherwise } else { 0 }, None),
        }
    }

    /// Where a link that would open in a link preview goes instead, of the user's `spaces`: the
    /// Space of the first rule it meets, with that rule, if it's another than `shown`. A rule to
    /// the Space shown keeps it in the preview, as does one to Little ilo, a quick look as the
    /// preview is, and everything else's Space is only for links from other apps.
    pub fn preview_route(&self, url: &str, spaces: &[u64], shown: u64) -> Option<(u64, usize)> {
        match self.route(url, spaces) {
            (space, Some(rule)) if space != shown && space != LITTLE => Some((space, rule)),
            _ => None,
        }
    }

    /// The rules the file `text` holds, its Spaces by their places among the user's `spaces`, as
    /// the core's ids are new each launch and the Spaces come back in their order. A file with no
    /// line for everything else sends it to Little ilo, the default.
    pub fn parse(text: &str, spaces: &[u64]) -> Rules {
        let space = |place: &str| match place {
            LITTLE_NAME => Some(LITTLE),
            place => {
                let place = place.parse::<usize>().ok()?;
                spaces.get(place.wrapping_sub(1)).copied()
            }
        };
        let mut rules = Rules::default();
        for line in text.lines() {
            let mut fields = line.splitn(4, '\t');
            match (fields.next(), fields.next(), fields.next(), fields.next()) {
                (Some("rule"), Some(key), Some(place), Some(text)) => {
                    let Some(condition) = Condition::ALL.into_iter().find(|c| c.key() == key)
                    else {
                        continue;
                    };
                    let space = space(place).unwrap_or(0);
                    rules.rules.push(Rule { condition, text: text.to_string(), space });
                }
                (Some("otherwise"), Some(place), None, None) => {
                    rules.otherwise = space(place).unwrap_or(0);
                }
                _ => {}
            }
        }
        rules
    }

    /// The file of the rules, as `parse` reads it. A Space not among `spaces`, such as one
    /// deleted, is written as none, and so is the Space in front.
    pub fn to_text(&self, spaces: &[u64]) -> String {
        let place = |space: u64| match spaces.iter().position(|&each| each == space) {
            _ if space == LITTLE => LITTLE_NAME.to_string(),
            Some(index) => (index + 1).to_string(),
            None => "-".to_string(),
        };
        let mut text = String::new();
        for rule in &self.rules {
            let line = rule.text.replace(['\t', '\n', '\r'], " ");
            let key = rule.condition.key();
            text.push_str(&format!("rule\t{key}\t{}\t{line}\n", place(rule.space)));
        }
        if self.otherwise != LITTLE {
            text.push_str(&format!("otherwise\t{}\n", place(self.otherwise)));
        }
        text
    }

    /// The rules as check.sh reads them: each one's condition, its text and its Space's name,
    /// then where everything else opens.
    pub fn describe(&self, spaces: &[(u64, SharedString)]) -> String {
        let rules: Vec<String> = (self.rules.iter())
            .map(|rule| {
                let (condition, text) = (rule.condition.title(), &rule.text);
                format!("{condition} \"{text}\" → {}", space_name(rule.space, spaces))
            })
            .collect();
        let rules = if rules.is_empty() { "none".to_string() } else { rules.join(", ") };
        format!("{rules}; everything else {}", otherwise_name(self.otherwise, spaces))
    }
}

/// The name a rule's pop-up shows for `space`: "No Space" for one deleted or none.
fn space_name(space: u64, spaces: &[(u64, SharedString)]) -> SharedString {
    match spaces.iter().find(|(id, _)| *id == space) {
        _ if space == LITTLE => LITTLE_NAME.into(),
        Some((_, name)) => name.clone(),
        None => "No Space".into(),
    }
}

/// The name Everything else's pop-up shows for `space`: the Space in front for 0, or one deleted.
fn otherwise_name(space: u64, spaces: &[(u64, SharedString)]) -> SharedString {
    match spaces.iter().find(|(id, _)| *id == space) {
        _ if space == LITTLE => LITTLE_NAME.into(),
        Some((_, name)) => name.clone(),
        None => MenuEntry::label("The Space in Front"),
    }
}

/// What the Links tab asks of the browser.
pub trait Host {
    /// The user's Spaces, in order, with their names: never an agent's.
    fn spaces(&self) -> Vec<(u64, SharedString)>;
    fn rules(&self) -> Rules;
    /// Keeps `rules`, for the links from now on and across launches.
    fn set_rules(&self, rules: &Rules);
    /// Shows `items` in ilo's menu over `window`, the chosen one's text at `at`, as a pop-up
    /// button's menu opens, light or dark as the tab is, and calls `picked` with the one picked,
    /// if any, once it closes.
    fn show_menu(
        &self,
        items: Vec<MenuEntry>,
        at: Point<Pixels>,
        light: bool,
        window: &Window,
        picked: Box<dyn FnOnce(Option<usize>, &mut App)>,
        cx: &mut App,
    );
}

/// A pop-up button of the tab's: a rule's condition or Space, by the rule's place, or everything
/// else's Space.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Popup {
    Condition(usize),
    Space(usize),
    Otherwise,
}

/// A rule's row being dragged, by its place.
struct RuleDrag(usize);

/// What follows the pointer while a rule's row is dragged: its condition and text.
struct DraggedRule(SharedString, Look);

impl Render for DraggedRule {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let look = self.1;
        div()
            .h(px(CONTROL_HEIGHT))
            .px(px(CONTROL_PADDING))
            .flex()
            .items_center()
            .rounded(px(CONTROL_RADIUS))
            .bg(look.control)
            .text_color(look.ink)
            .child(self.0.clone())
    }
}

/// Settings' Links section: a group of the rules, a row each with its trash, + under it, and
/// where everything else opens.
pub struct LinkRulesView {
    host: Rc<dyn Host>,
    look: Look,
    rules: Rules,
    /// Each rule's text field, in the rules' order.
    fields: Vec<Entity<TextInput>>,
    choosing: Option<Popup>,
    /// Where each pop-up's text starts in the window, as last drawn, where its menu puts the
    /// chosen item's.
    popup_texts: Rc<RefCell<Vec<(Popup, Point<Pixels>)>>>,
    /// The rule whose field takes the keyboard at the next draw: the one + added.
    focus: Option<usize>,
    /// What the last draw showed, which check.sh reads once each time it changes.
    drawn: Option<String>,
    /// The rules the trash took, the last at the end, each with its place, which ⌘Z brings back
    /// until Settings closes. A rule with no text goes without one: there's nothing to lose.
    removed: Vec<(usize, Rule)>,
    _typed: Vec<Subscription>,
}

impl LinkRulesView {
    pub fn new(host: Rc<dyn Host>, cx: &mut Context<Self>) -> LinkRulesView {
        let mut view = LinkRulesView {
            rules: host.rules(),
            host,
            look: Look::settings(true),
            fields: Vec::new(),
            choosing: None,
            popup_texts: Rc::default(),
            focus: None,
            drawn: None,
            removed: Vec::new(),
            _typed: Vec::new(),
        };
        view.make_fields(cx);
        view
    }

    /// Draws it in `look`, as Settings' window turns light or dark.
    pub fn set_look(&mut self, look: Look, cx: &mut Context<Self>) {
        if self.look == look {
            return;
        }
        self.look = look;
        let selection = Hsla::from(rgb(NO_THEME_ACCENT)).opacity(SELECTION);
        for field in &self.fields {
            field.update(cx, |field, cx| {
                field.set_colors(look.detail.into(), look.ink.into(), selection);
                cx.notify();
            });
        }
        cx.notify();
    }

    /// A field for each rule, holding its text, whose typing changes the rule.
    fn make_fields(&mut self, cx: &mut Context<Self>) {
        let look = self.look;
        self.fields = (self.rules.rules.iter())
            .map(|rule| {
                cx.new(|cx| {
                    TextInput::new("example.com", cx)
                        .placeholder_color(look.detail)
                        .caret_color(look.ink)
                        .with_text(rule.text.clone().into())
                })
            })
            .collect();
        self._typed = (self.fields.iter())
            .map(|field| cx.observe(field, |view, field, cx| view.typed(&field, cx)))
            .collect();
    }

    fn typed(&mut self, field: &Entity<TextInput>, cx: &mut Context<Self>) {
        let Some(index) = self.fields.iter().position(|each| each == field) else {
            return;
        };
        let field = field.read(cx);
        if field.composing() || field.text() == self.rules.rules[index].text {
            return;
        }
        self.rules.rules[index].text = field.text().to_string();
        self.keep(cx);
    }

    /// Keeps the rules as they are now and draws them.
    fn keep(&mut self, cx: &mut Context<Self>) {
        self.host.set_rules(&self.rules);
        cx.notify();
    }

    /// + : a rule at the end, matching what its URL contains, to the first Space, its field taking
    /// the keyboard.
    pub fn add(&mut self, cx: &mut Context<Self>) {
        let space = self.host.spaces().first().map_or(0, |(space, _)| *space);
        let rule = Rule { condition: Condition::Contains, text: String::new(), space };
        self.rules.rules.push(rule);
        self.make_fields(cx);
        self.focus = Some(self.rules.rules.len() - 1);
        self.keep(cx);
    }

    /// The trash on the rule at `index`.
    pub fn remove(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.rules.rules.len() {
            let rule = self.rules.rules.remove(index);
            if !rule.text.trim().is_empty() {
                self.removed.push((index, rule));
            }
            self.make_fields(cx);
            self.keep(cx);
        }
    }

    /// ⌘Z: the last rule the trash took, back at its place. Whether there was one.
    pub fn undo(&mut self, cx: &mut Context<Self>) -> bool {
        let Some((index, rule)) = self.removed.pop() else {
            return false;
        };
        let index = index.min(self.rules.rules.len());
        self.rules.rules.insert(index, rule);
        self.make_fields(cx);
        self.keep(cx);
        true
    }

    /// Settings closed: what the trash took stays gone.
    pub fn forget_removed(&mut self) {
        self.removed.clear();
    }

    /// The rule at `from` dragged onto the row at `to`, which it takes the place of.
    pub fn move_rule(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let count = self.rules.rules.len();
        if from == to || from >= count || to >= count {
            return;
        }
        let rule = self.rules.rules.remove(from);
        self.rules.rules.insert(to, rule);
        let field = self.fields.remove(from);
        self.fields.insert(to, field);
        self.keep(cx);
    }

    fn set_condition(&mut self, index: usize, condition: Condition, cx: &mut Context<Self>) {
        if let Some(rule) = self.rules.rules.get_mut(index) {
            rule.condition = condition;
            self.keep(cx);
        }
    }

    fn set_space(&mut self, which: Popup, space: u64, cx: &mut Context<Self>) {
        match which {
            Popup::Space(index) if index < self.rules.rules.len() => {
                self.rules.rules[index].space = space;
            }
            Popup::Otherwise => self.rules.otherwise = space,
            _ => return,
        }
        self.keep(cx);
    }

    /// Opens `which`'s menu over its button: the conditions, or the user's Spaces then Little ilo,
    /// Everything else's after the Space in front.
    fn choose(&mut self, which: Popup, window: &Window, cx: &mut Context<Self>) {
        let entry = |title: SharedString, on: bool| MenuEntry {
            checked: on,
            over: on,
            ..MenuEntry::named(title, true)
        };
        let spaces = self.host.spaces();
        // What a Space pop-up picks from, in its order.
        let targets: Vec<u64> =
            spaces.iter().map(|(space, _)| *space).chain(std::iter::once(LITTLE)).collect();
        let items: Vec<MenuEntry> = match which {
            Popup::Condition(index) => {
                let chosen = self.rules.rules.get(index).map(|rule| rule.condition);
                (Condition::ALL.iter())
                    .map(|&condition| entry(condition.title().into(), Some(condition) == chosen))
                    .collect()
            }
            Popup::Space(index) => {
                let chosen = self.rules.rules.get(index).map_or(0, |rule| rule.space);
                (targets.iter())
                    .map(|&space| entry(space_name(space, &spaces), space == chosen))
                    .collect()
            }
            Popup::Otherwise => {
                let chosen = self.rules.otherwise;
                let front = !targets.contains(&chosen);
                std::iter::once(entry(otherwise_name(0, &spaces), front))
                    .chain(
                        (targets.iter())
                            .map(|&space| entry(otherwise_name(space, &spaces), space == chosen)),
                    )
                    .collect()
            }
        };
        let view = cx.weak_entity();
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            view.update(cx, |view, cx| {
                view.choosing = None;
                match (which, picked) {
                    (Popup::Condition(index), Some(picked)) => {
                        view.set_condition(index, Condition::ALL[picked], cx)
                    }
                    (Popup::Space(_), Some(picked)) => view.set_space(which, targets[picked], cx),
                    (Popup::Otherwise, Some(0)) => view.set_space(which, 0, cx),
                    (Popup::Otherwise, Some(picked)) => {
                        view.set_space(which, targets[picked - 1], cx)
                    }
                    (_, None) => cx.notify(),
                }
            })
            .ok();
        });
        let texts = self.popup_texts.borrow();
        let Some(at) = texts.iter().find(|&&(each, _)| each == which).map(|&(_, at)| at) else {
            return;
        };
        drop(texts);
        self.choosing = Some(which);
        cx.notify();
        self.host.show_menu(items, at, self.look.light(), window, picked, cx);
    }

    fn popup(
        &self,
        which: Popup,
        label: SharedString,
        width: f32,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let texts = self.popup_texts.clone();
        let at = canvas(
            |_, _, _| {},
            move |bounds, _, _, _| {
                let text = point(bounds.origin.x + px(CONTROL_PADDING), bounds.origin.y);
                let mut texts = texts.borrow_mut();
                texts.retain(|&(each, _)| each != which);
                texts.push((which, text));
            },
        )
        .absolute()
        .inset_0();
        popup(&self.look, label, width, self.choosing == Some(which))
            .relative()
            .child(at)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, _, window, cx| {
                    // A press on a control isn't the start of its row's drag.
                    cx.stop_propagation();
                    view.choose(which, window, cx)
                }),
            )
    }

    fn rule_row(
        &self,
        index: usize,
        rule: &Rule,
        spaces: &[(u64, SharedString)],
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let look = self.look;
        let condition =
            self.popup(Popup::Condition(index), rule.condition.title().into(), RULE_CONDITION, cx);
        let field = div()
            .flex_1()
            .min_w_0()
            .h(px(CONTROL_HEIGHT))
            .px(px(FIELD_PADDING))
            .flex()
            .items_center()
            .rounded(px(CONTROL_RADIUS))
            .bg(look.control)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(self.fields[index].clone());
        let space =
            self.popup(Popup::Space(index), space_name(rule.space, spaces), RULE_SPACE, cx);
        let remove = icon_button(&look, ("remove-link-rule", index), "icons/trash.svg")
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |view, _, _, cx| view.remove(index, cx)));
        let label: SharedString = format!("{} {}", rule.condition.title(), rule.text).into();
        let row = div()
            .id(("link-rule", index))
            .px(px(ROW_X))
            .py(px(SHORT_PADDING))
            .flex()
            .items_center()
            .gap(px(RULE_GAP))
            .on_drag(RuleDrag(index), move |_, _, _, cx| {
                cx.new(|_| DraggedRule(label.clone(), look))
            })
            .drag_over::<RuleDrag>(move |style, _, _, _| style.bg(look.control))
            .on_drop(
                cx.listener(move |view, drag: &RuleDrag, _, cx| view.move_rule(drag.0, index, cx)),
            )
            .child(condition)
            .child(field)
            .child(
                svg().path("icons/arrow-right.svg").size(px(BUTTON_GLYPH)).text_color(look.detail),
            )
            .child(space)
            .child(remove);
        div().child(row)
    }
}

impl Render for LinkRulesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let spaces = self.host.spaces();
        let described = self.rules.describe(&spaces);
        if self.drawn.as_ref() != Some(&described) {
            // check.sh waits for this line.
            crate::log!("browser: link rules drawn {described}");
            self.drawn = Some(described);
        }
        if let Some(index) = self.focus.take()
            && let Some(field) = self.fields.get(index)
        {
            window.focus(&field.focus_handle(cx), cx);
            // check.sh waits for this line, which says the field has the keyboard.
            crate::log!("browser: link rule {index} field has the keyboard");
        }
        let look = self.look;
        let rows: Vec<gpui::Div> = (self.rules.rules.iter().enumerate())
            .map(|(index, rule)| self.rule_row(index, rule, &spaces, cx))
            .collect();
        let rules = match rows.is_empty() {
            true => group(
                &look,
                [frame()
                    .text_color(look.detail)
                    .child("Rules send links from other apps to the Space they belong in.")],
            ),
            false => group(&look, rows),
        };
        let add = icon_button(&look, "add-link-rule", "icons/plus.svg")
            .on_click(cx.listener(|view, _, _, cx| view.add(cx)));
        let otherwise = otherwise_name(self.rules.otherwise, &spaces);
        let otherwise = self.popup(Popup::Otherwise, otherwise, RULE_SPACE, cx);
        let otherwise = detailed_row(
            &look,
            "Everything else opens in",
            "Links no rule matches.",
            Some(otherwise.into_any_element()),
        );
        div()
            .text_color(look.ink)
            .child(rules)
            .child(div().mt(px(BUTTONS_TOP)).ml(px(ADD_X)).flex().child(add))
            .child(group(&look, [otherwise]).mt(px(GROUP_GAP)))
    }
}
