//! Link rules (docs/ux-spec.md › Link rules): the Space a link from another app, or one that would
//! open in a link preview, opens in, or Little ilo, and Settings' Links tab, which makes them.
//! They're the user's alone: agents get no way to read or change them, as with the default
//! browser.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, ClickEvent, Context, Entity, Focusable, Hsla, MouseButton, Pixels, Point, SharedString,
    Subscription, Window, canvas, div, point, prelude::*, px, rgb, svg,
};

use crate::settings_rows::{
    Look, detailed_row, drawn, frame, group, icon_button, named_button, named_popup, popup,
};
use crate::shell::MenuEntry;
use crate::text_input::TextInput;
use crate::tokens::palette::{NO_THEME_ACCENT, SELECTION};
use crate::tokens::settings::{
    ADD_X, BUTTON_GLYPH, BUTTONS_TOP, CONTROL_HEIGHT, CONTROL_PADDING, CONTROL_RADIUS,
    FIELD_PADDING, GROUP_GAP, ROW_X, RULE_CONDITION, RULE_GAP, RULE_SPACE, SHORT_PADDING,
};

#[cfg(test)]
mod tests;

/// What a rule asks of a link's URL, as Arc 1.170's routes were measured to ask it (#692): to hold
/// its text anywhere, query and fragment too, or to be it, by host and path alone. Both ignore case,
/// the scheme and a leading www., on the URL and the text alike.
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

/// The row for links no rule matches, as it's labelled and an assistive app calls its pop-up.
const OTHERWISE: &str = "Everything else opens in";

/// What an assistive app calls rule `index`'s `part`, by its place from 1: "Rule 1 Condition".
fn rule_name(index: usize, part: &str) -> SharedString {
    format!("Rule {} {part}", index + 1).into()
}

/// Little ilo's name in the pop-ups, the rules' file and check.sh's lines.
const LITTLE_NAME: &str = "Little ilo";

/// Where a rule sends a link in place of a Space: the Space the regular window in front shows, as
/// Arc's Most Recent Space. No Space of the core's has this id.
pub const FRONT: u64 = u64::MAX - 1;

/// The Space in front's name in the rules' file.
const FRONT_KEY: &str = "front";

/// The rule a profile starts with, once, as Arc seeds its Google Meet route (#692): a call joins
/// in the Space in front, not in Little ilo, where everything else goes. The trash takes it for
/// good, as the file then says it was added.
const MEET: &str = "meet.google.com";

/// The line that says the Meet rule was added, so it's added once.
const SEEDED: &str = "seeded\tmeet";

/// A rule: links whose URL meets its condition open in its Space, the Space in front or Little ilo,
/// 0 for none.
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub condition: Condition,
    pub text: String,
    pub space: u64,
}

impl Rule {
    /// Whether a link to `url` meets it. A rule without text yet meets none.
    pub fn matches(&self, url: &str) -> bool {
        let text = match self.condition {
            Condition::Contains => bare(&self.text),
            Condition::Is => host_and_path(&self.text),
        };
        !text.is_empty()
            && match self.condition {
                Condition::Contains => bare(url).contains(&text),
                Condition::Is => host_and_path(url) == text,
            }
    }
}

/// `url` in lower case without its scheme or a leading www., as a rule's text is often typed
/// without them: Arc's "contains https://example.com/" meets http:// and www. links.
fn bare(url: &str) -> String {
    let url = url.trim().to_lowercase();
    let rest = match url.split_once("://") {
        Some((scheme, rest)) if is_scheme(scheme) => rest,
        _ => &url,
    };
    rest.strip_prefix("www.").unwrap_or(rest).to_string()
}

/// Whether `text` is a URL's scheme, so a "://" further on, in a query, isn't taken for one.
fn is_scheme(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// What Arc's "is equal to" compares of `url`: its host, without a user or www., and its path,
/// without a trailing slash, in lower case. The query and the fragment don't count, nor does the
/// scheme, but a port does, as one typed names another server.
fn host_and_path(url: &str) -> String {
    let rest = bare(url);
    let rest = &rest[..rest.find(['?', '#']).unwrap_or(rest.len())];
    let (host, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let host = host.rsplit_once('@').map_or(host, |(_, host)| host);
    let host = host.strip_prefix("www.").unwrap_or(host);
    format!("{host}{}", path.trim_end_matches('/'))
}

/// The rules, top to bottom, and the Space everything else opens in, 0 for the Space in front:
/// Little ilo unless the user picks another, as Arc's Little Arc (the owner, 2026-10-31). A
/// profile starts with the Meet rule.
#[derive(Clone, Debug, PartialEq)]
pub struct Rules {
    pub rules: Vec<Rule>,
    pub otherwise: u64,
}

impl Default for Rules {
    fn default() -> Rules {
        Rules {
            rules: vec![Rule::meet()],
            otherwise: LITTLE,
        }
    }
}

impl Rule {
    /// The rule a profile starts with: Google Meet's calls in the Space in front.
    fn meet() -> Rule {
        Rule {
            condition: Condition::Contains,
            text: MEET.to_string(),
            space: FRONT,
        }
    }
}

impl Rules {
    /// Where a link to `url` opens, of the user's `spaces`: the Space of the first rule it meets
    /// whose Space is still there, or Little ilo, with that rule, else everything else's, 0 being
    /// the Space in front.
    pub fn route(&self, url: &str, spaces: &[u64]) -> (u64, Option<usize>) {
        let there = |space: u64| space == LITTLE || spaces.contains(&space);
        let rule = self
            .rules
            .iter()
            .position(|rule| (rule.space == FRONT || there(rule.space)) && rule.matches(url));
        match rule {
            Some(index) => match self.rules[index].space {
                FRONT => (0, Some(index)),
                space => (space, Some(index)),
            },
            None => (
                if there(self.otherwise) {
                    self.otherwise
                } else {
                    0
                },
                None,
            ),
        }
    }

    /// Where a link that would open in a link preview goes instead, of the user's `spaces`: the
    /// Space of the first rule it meets, with that rule, if it's another than `shown`. A rule to
    /// the Space shown keeps it in the preview, as does one to the Space in front, which is the
    /// one shown, or to Little ilo, a quick look as the preview is, and everything else's Space is
    /// only for links from other apps.
    pub fn preview_route(&self, url: &str, spaces: &[u64], shown: u64) -> Option<(u64, usize)> {
        match self.route(url, spaces) {
            (space, Some(rule)) if space != shown && space != LITTLE && space != 0 => {
                Some((space, rule))
            }
            _ => None,
        }
    }

    /// The rules the file `text` holds, its Spaces by their places among the user's `spaces`, as
    /// the core's ids are new each launch and the Spaces come back in their order. A file with no
    /// line for everything else sends it to Little ilo, the default, and one that doesn't say the
    /// Meet rule was added gets it after its own, as a profile from before it does.
    pub fn parse(text: &str, spaces: &[u64]) -> Rules {
        let space = |place: &str| match place {
            LITTLE_NAME => Some(LITTLE),
            FRONT_KEY => Some(FRONT),
            place => {
                let place = place.parse::<usize>().ok()?;
                spaces.get(place.wrapping_sub(1)).copied()
            }
        };
        let mut rules = Rules {
            rules: Vec::new(),
            ..Rules::default()
        };
        let mut seeded = false;
        for line in text.lines() {
            if line == SEEDED {
                seeded = true;
                continue;
            }
            let mut fields = line.splitn(4, '\t');
            match (fields.next(), fields.next(), fields.next(), fields.next()) {
                (Some("rule"), Some(key), Some(place), Some(text)) => {
                    let Some(condition) = Condition::ALL.into_iter().find(|c| c.key() == key)
                    else {
                        continue;
                    };
                    let space = space(place).unwrap_or(0);
                    rules.rules.push(Rule {
                        condition,
                        text: text.to_string(),
                        space,
                    });
                }
                (Some("otherwise"), Some(place), None, None) => {
                    rules.otherwise = space(place).unwrap_or(0);
                }
                _ => {}
            }
        }
        if !seeded {
            rules.rules.push(Rule::meet());
        }
        rules
    }

    /// The file of the rules, as `parse` reads it. A Space not among `spaces`, such as one
    /// deleted, is written as none, and so is everything else's Space in front. It always says
    /// the Meet rule was added, so a trashed one stays gone.
    pub fn to_text(&self, spaces: &[u64]) -> String {
        let place = |space: u64| match spaces.iter().position(|&each| each == space) {
            _ if space == LITTLE => LITTLE_NAME.to_string(),
            _ if space == FRONT => FRONT_KEY.to_string(),
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
        text.push_str(SEEDED);
        text.push('\n');
        text
    }

    /// The rules as check.sh reads them: each one's condition, its text and its Space's name,
    /// then where everything else opens.
    pub fn describe(&self, spaces: &[(u64, SharedString)]) -> String {
        let rules: Vec<String> = (self.rules.iter())
            .map(|rule| {
                let (condition, text) = (rule.condition.title(), &rule.text);
                format!(
                    "{condition} \"{text}\" → {}",
                    space_name(rule.space, spaces)
                )
            })
            .collect();
        let rules = if rules.is_empty() {
            "none".to_string()
        } else {
            rules.join(", ")
        };
        format!(
            "{rules}; everything else {}",
            otherwise_name(self.otherwise, spaces)
        )
    }
}

/// The name a rule's pop-up shows for `space`: "No Space" for one deleted or none.
fn space_name(space: u64, spaces: &[(u64, SharedString)]) -> SharedString {
    match spaces.iter().find(|(id, _)| *id == space) {
        _ if space == LITTLE => LITTLE_NAME.into(),
        _ if space == FRONT => otherwise_name(0, spaces),
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
    /// Where each control was logged as drawn, for check.sh to click.
    drawn_at: Rc<RefCell<Vec<String>>>,
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
            drawn_at: Rc::default(),
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
        self.fields = (self.rules.rules.iter().enumerate())
            .map(|(index, rule)| {
                cx.new(|cx| {
                    TextInput::new("example.com", cx)
                        .placeholder_color(look.detail)
                        .caret_color(look.ink)
                        .with_text(rule.text.clone().into())
                        .spoken(rule_name(index, "Text"))
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

    /// + : a rule at the end, matching what its URL contains, to the Space in front, as Arc's New
    /// Route goes to Most Recent Space (#692), its field taking the keyboard.
    pub fn add(&mut self, cx: &mut Context<Self>) {
        let rule = Rule {
            condition: Condition::Contains,
            text: String::new(),
            space: FRONT,
        };
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
        // Each field is called by its rule's place, which moved.
        for (index, field) in self.fields.iter().enumerate() {
            field.update(cx, |field, cx| {
                field.set_spoken(rule_name(index, "Text"), cx)
            });
        }
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

    /// Opens `which`'s menu over its button: the conditions, or the Space in front, the user's
    /// Spaces, then Little ilo, as Arc's offers Most Recent Space, the Spaces and Little Arc.
    fn choose(&mut self, which: Popup, window: &Window, cx: &mut Context<Self>) {
        let entry = |title: SharedString, on: bool| MenuEntry {
            checked: on,
            over: on,
            ..MenuEntry::named(title, true)
        };
        let spaces = self.host.spaces();
        // What a Space pop-up picks from, in its order.
        let targets: Vec<u64> = spaces
            .iter()
            .map(|(space, _)| *space)
            .chain(std::iter::once(LITTLE))
            .collect();
        let items: Vec<MenuEntry> = match which {
            Popup::Condition(index) => {
                let chosen = self.rules.rules.get(index).map(|rule| rule.condition);
                (Condition::ALL.iter())
                    .map(|&condition| entry(condition.title().into(), Some(condition) == chosen))
                    .collect()
            }
            Popup::Space(index) => {
                let chosen = self.rules.rules.get(index).map_or(0, |rule| rule.space);
                std::iter::once(entry(space_name(FRONT, &spaces), chosen == FRONT))
                    .chain(
                        (targets.iter())
                            .map(|&space| entry(space_name(space, &spaces), space == chosen)),
                    )
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
                    (Popup::Space(_), Some(0)) => view.set_space(which, FRONT, cx),
                    (Popup::Otherwise, Some(0)) => view.set_space(which, 0, cx),
                    (Popup::Space(_) | Popup::Otherwise, Some(picked)) => {
                        view.set_space(which, targets[picked - 1], cx)
                    }
                    (_, None) => cx.notify(),
                }
            })
            .ok();
        });
        let texts = self.popup_texts.borrow();
        let Some(at) = texts
            .iter()
            .find(|&&(each, _)| each == which)
            .map(|&(_, at)| at)
        else {
            return;
        };
        drop(texts);
        self.choosing = Some(which);
        cx.notify();
        self.host
            .show_menu(items, at, self.look.light(), window, picked, cx);
    }

    fn popup(
        &self,
        which: Popup,
        label: SharedString,
        width: f32,
        a11y: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
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
        let name = match which {
            Popup::Condition(index) => format!("link condition {index}"),
            Popup::Space(index) => format!("link space {index}"),
            Popup::Otherwise => "link otherwise".into(),
        };
        // An assistive app reads it by what it sets, with what it shows as its value.
        let spoken = match which {
            Popup::Condition(index) => rule_name(index, "Condition"),
            Popup::Space(index) => rule_name(index, "Space"),
            Popup::Otherwise => OTHERWISE.into(),
        };
        popup(
            &self.look,
            label.clone(),
            width,
            self.choosing == Some(which),
        )
        .relative()
        .id(SharedString::from(name.clone()))
        .when(a11y, |control| named_popup(control, spoken, label))
        .child(at)
        .child(drawn(name, self.drawn_at.clone()))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |view, _, window, cx| {
                // A press on a control isn't the start of its row's drag.
                cx.stop_propagation();
                view.choose(which, window, cx)
            }),
        )
        // An assistive app's press is a press at its middle, which it offers for what takes
        // clicks; a key's click opens it as Settings' pop-ups do.
        .on_click(cx.listener(move |view, event: &ClickEvent, window, cx| {
            if event.is_keyboard() {
                view.choose(which, window, cx)
            }
        }))
    }

    fn rule_row(
        &self,
        index: usize,
        rule: &Rule,
        spaces: &[(u64, SharedString)],
        a11y: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let look = self.look;
        let condition = self.popup(
            Popup::Condition(index),
            rule.condition.title().into(),
            RULE_CONDITION,
            a11y,
            cx,
        );
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
        let space = self.popup(
            Popup::Space(index),
            space_name(rule.space, spaces),
            RULE_SPACE,
            a11y,
            cx,
        );
        let remove = icon_button(&look, ("remove-link-rule", index), "icons/trash.svg")
            .when(a11y, |remove| {
                named_button(remove, format!("Remove Rule {}", index + 1))
            })
            .relative()
            .child(drawn(format!("link remove {index}"), self.drawn_at.clone()))
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
                div()
                    .relative()
                    .child(
                        svg()
                            .path("icons/arrow-right.svg")
                            .size(px(BUTTON_GLYPH))
                            .text_color(look.detail),
                    )
                    .child(drawn(format!("link arrow {index}"), self.drawn_at.clone())),
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
        let a11y = window.is_a11y_active();
        let rows: Vec<gpui::Div> = (self.rules.rules.iter().enumerate())
            .map(|(index, rule)| self.rule_row(index, rule, &spaces, a11y, cx))
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
            .when(a11y, |add| named_button(add, "Add Rule"))
            .relative()
            .child(drawn("link add".into(), self.drawn_at.clone()))
            .on_click(cx.listener(|view, _, _, cx| view.add(cx)));
        let otherwise = otherwise_name(self.rules.otherwise, &spaces);
        let otherwise = self.popup(Popup::Otherwise, otherwise, RULE_SPACE, a11y, cx);
        let otherwise = detailed_row(
            &look,
            OTHERWISE,
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
