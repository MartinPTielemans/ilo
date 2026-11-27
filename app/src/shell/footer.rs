//! The sidebar's foot (#4400): the Library at its left, off the Mac the menu control after it, and
//! at its right the Space shown's button, whose menu holds every Space and what the + made.

use std::time::Instant;

use gpui::{
    AnyElement, Context, FontWeight, MouseButton, MouseDownEvent, Pixels, Point, SharedString,
    Window, div, prelude::*, px, rgb, svg,
};

use super::{
    FOOTER_HEIGHT, MenuEntry, NAV_GLYPH, NAV_HOVERED, NAV_RADIUS, NAV_SIZE, PLUS_INSET, Shell,
    access::Accessible, drag::EDGE_WAIT, menus::MenuAction,
};
use crate::elevation::Level;
use crate::palette::{Float, PAPER, Tone};
use crate::space_icon::SpaceIcon;
use crate::tokens::shell::space_button::motion::{PILL, PILL_FADE, SLIDE};
use crate::tokens::shell::{
    FOOT_GLYPH, FOOT_PLATE, FOOT_PLATE_RADIUS, ICON_INK, LIBRARY_BUTTON, PLUS_PLATE,
    SPACE_BUTTON_CHEVRON, SPACE_BUTTON_GAP, SPACE_BUTTON_PAD,
};
use crate::tokens::tooltip::{GAP, HEIGHT, NEAR_RADIUS, RIM, TEXT_SIZE, TEXT_X};
use crate::tooltip::{Anchor, Tip};

/// The Space button's menu's New Space…, for a tab dropped on it: no Space's id.
const NEW_SPACE: u64 = 0;

/// How far the Space button's right edge is from the sidebar's side: where the +'s plate ended,
/// centred where the first row's size of button would be.
pub(super) fn space_button_right() -> f32 {
    PLUS_INSET + (NAV_SIZE - FOOT_PLATE) / 2.
}

/// How wide the Space button is: its mark and chevron and the room round them.
pub(super) fn space_button_width() -> f32 {
    2. * SPACE_BUTTON_PAD + FOOT_GLYPH + SPACE_BUTTON_GAP + SPACE_BUTTON_CHEVRON
}

/// The Space button's change after a switch in this window, once the rows have slid: under
/// Reduce Motion, where the pages land at once, the Space before's mark cross-fades into the new
/// one's, and the new Space's name shows in a pill above the button for a moment, as a tooltip
/// would, so a switch by keys says where it went. Otherwise the marks have slid with the pages.
pub(super) struct ButtonSwitch {
    /// The mark leaving: the Space before's icon and name.
    before: (SharedString, SharedString),
    /// When the rows were in their places: None while they still slide.
    start: Option<Instant>,
    /// Whether the mark is in its place, which check.sh has heard.
    settled: bool,
}

impl ButtonSwitch {
    /// How far along the marks' cross-fade is, on its ease-out, `t` seconds in.
    fn along(t: f32) -> f32 {
        super::cubic_bezier(0., 0., 0.58, 1., (t / SLIDE).clamp(0., 1.))
    }

    /// How much the pill shows, `t` seconds in: fading in, held, then fading out by `PILL`.
    fn pill(t: f32) -> f32 {
        (t / PILL_FADE).min((PILL - t) / PILL_FADE).clamp(0., 1.)
    }
}

impl Shell {
    /// The window has switched from `before` to another Space: once the rows have slid, the Space
    /// button's name's pill shows, and under Reduce Motion its marks cross-fade.
    pub(super) fn switch_space_button(&mut self, before: u64) {
        let Some(space) = self.spaces.iter().find(|space| space.id == before) else {
            return;
        };
        self.space_button_switch = Some(ButtonSwitch {
            before: (space.icon.clone(), space.name.clone()),
            start: None,
            settled: false,
        });
    }

    /// Starts the Space button's switch once the rows' settle is over, keeps frames coming while
    /// its cross-fade or pill moves, and drops the switch once both are done.
    pub(super) fn step_space_button(&mut self, window: &mut Window) {
        let settling = self.settle.is_some();
        let now = self.clock.now();
        let Some(switch) = self.space_button_switch.as_mut() else {
            return;
        };
        let start = match switch.start {
            Some(start) => start,
            None if settling => return window.request_animation_frame(),
            None => *switch.start.insert(now),
        };
        let t = now.saturating_duration_since(start).as_secs_f32();
        if t >= SLIDE && !std::mem::replace(&mut switch.settled, true) {
            // check.sh waits for this line before it clicks the button or captures it.
            log!("browser: space button settled");
        }
        match t < SLIDE.max(PILL) {
            true => window.request_animation_frame(),
            false => self.space_button_switch = None,
        }
    }

    /// Where the Space button's marks are across its box, by their Spaces: each page the sidebar
    /// draws has its Space's mark as far across the box as the page is across the sidebar, so the
    /// mark goes with a swipe, its spring, a cancel and the rubber band at the ends (#4400).
    pub(super) fn space_button_places(&self) -> Vec<(u64, f32)> {
        let width = self.width();
        self.pages()
            .into_iter()
            .map(|(space, x)| (space, x / width * FOOT_GLYPH))
            .collect()
    }

    /// The Space button's mark box: each Space's mark where its page is, clipped to the box, and
    /// under Reduce Motion, while a switch's cross-fade is on, the Space before's fading out.
    fn space_button_marks(
        &self,
        icon: &str,
        name: &str,
        ink: gpui::Hsla,
        cx: &gpui::App,
    ) -> AnyElement {
        let mark = |icon: &str, name: &str| crate::space_icon::mark(icon, name, FOOT_GLYPH, ink);
        let placed = |element: AnyElement, x: f32, shown: f32| {
            div()
                .absolute()
                .top_0()
                .left(px(x))
                .size(px(FOOT_GLYPH))
                .opacity(shown)
                .child(element)
        };
        let marks = div()
            .relative()
            .flex_none()
            .size(px(FOOT_GLYPH))
            .overflow_hidden();
        let fading = self.space_button_switch.as_ref().and_then(|switch| {
            let along = ButtonSwitch::along(self.since(switch.start?));
            (crate::motion::still(cx) && along < 1.).then_some((switch, along))
        });
        if let Some((switch, along)) = fading {
            let (before_icon, before_name) = &switch.before;
            return marks
                .child(placed(mark(before_icon, before_name), 0., 1. - along))
                .child(placed(mark(icon, name), 0., along))
                .into_any_element();
        }
        let places = self.space_button_places();
        if let [(space, x)] = places[..]
            && space == self.space
            && x == 0.
        {
            return mark(icon, name);
        }
        let spaces = places.into_iter().filter_map(|(id, x)| {
            let space = self.spaces.iter().find(|space| space.id == id)?;
            Some(placed(mark(&space.icon, &space.name), x, 1.))
        });
        marks.children(spaces).into_any_element()
    }

    /// The pill naming the Space a switch in this window showed, above the Space button with
    /// their right edges together, in the tooltip's look, its corner toward the button squarer.
    pub(super) fn space_pill(&self) -> Option<impl IntoElement + use<>> {
        let switch = self.space_button_switch.as_ref()?;
        let shown = ButtonSwitch::pill(self.since(switch.start?));
        if shown <= 0. || self.menu_open {
            return None;
        }
        let space = self.spaces.iter().find(|space| space.id == self.space)?;
        let widest = self.width() - 2. * space_button_right();
        Some(
            div()
                .absolute()
                .right(px(space_button_right()))
                .bottom(px((FOOTER_HEIGHT + FOOT_PLATE) / 2. + GAP))
                .max_w(px(widest))
                .h(px(HEIGHT))
                .px(px(TEXT_X))
                .flex()
                .items_center()
                .rounded(px(HEIGHT / 2.))
                .rounded_br(px(NEAR_RADIUS))
                .border(px(RIM))
                .border_color(Level::Popover.rim())
                .shadow(Level::Popover.shadow())
                .bg(Float::Tooltip.fill(Tone::Dark))
                .opacity(shown)
                .text_size(px(TEXT_SIZE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(PAPER))
                .child(div().truncate().child(space.name.clone())),
        )
    }

    /// What a screen reader says for the Space button (words SP1): its label and value, the
    /// Space's place among the user's Spaces.
    fn space_button_said(&self, space: &super::Space) -> (SharedString, Option<SharedString>) {
        let spaces: Vec<u64> = (self.spaces.iter())
            .filter(|each| each.agent.is_empty())
            .map(|each| each.id)
            .collect();
        let at = spaces
            .iter()
            .position(|&id| id == space.id)
            .map(|at| (at + 1, spaces.len()));
        said(&space.name, at, cfg!(target_os = "linux"))
    }

    /// Whether the Space button wears its plate: while the menu up is its own.
    pub(super) fn space_button_lit(&self) -> bool {
        self.space_switcher_up && self.menu_open
    }

    /// The Space button at the foot's right: the Space shown's mark and a chevron. A click opens
    /// its menu, a right-click the Space's own. It wears the plate the + wore, under the pointer
    /// and while its menu is up.
    pub(super) fn space_button(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let space = self.spaces.iter().find(|space| space.id == self.space)?;
        let ink = self.palette.glyph(ICON_INK);
        Some(
            div()
                .id(SPACE_BUTTON)
                .when(self.a11y, |button| {
                    let (label, value) = self.space_button_said(space);
                    let button = button.button_named(label).opens_menu();
                    match value {
                        Some(value) => button.aria_value(value),
                        None => button,
                    }
                })
                .absolute()
                .right(px(space_button_right()))
                .bottom(px((FOOTER_HEIGHT - FOOT_PLATE) / 2.))
                .w(px(space_button_width()))
                .h(px(FOOT_PLATE))
                .px(px(SPACE_BUTTON_PAD))
                .flex()
                .items_center()
                .gap(px(SPACE_BUTTON_GAP))
                .rounded(px(FOOT_PLATE_RADIUS))
                .block_mouse_except_scroll()
                .when(self.space_button_lit(), |button| {
                    button.bg(self.palette.plate(PLUS_PLATE))
                })
                .hover(|style| style.bg(self.palette.plate(PLUS_PLATE)))
                // On the click, as the menu control's: the menu opens above the button, away from
                // the pointer, so a release after it opened would close it.
                .on_click(cx.listener(|shell, _, window, cx| shell.space_switcher(window, cx)))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|shell, event: &MouseDownEvent, _, cx| {
                        shell.space_menu(shell.space, event.position, cx)
                    }),
                )
                // In a box of its own: placed in the button's flow it would start at its padding, and
                // the menu and the drag's hit box with it.
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .child(self.button_frame(SPACE_BUTTON)),
                )
                .child(self.space_button_drawn())
                .child(self.space_button_marks(&space.icon, &space.name, ink.into(), cx))
                .child(
                    svg()
                        .path("icons/chevrons-up-down.svg")
                        .size(px(SPACE_BUTTON_CHEVRON))
                        .flex_none()
                        .text_color(ink),
                ),
        )
    }

    /// Whether `position`, in the window, is on the Space button's plate, where it takes the
    /// pointer.
    pub(super) fn on_space_button(&self, position: Point<Pixels>) -> bool {
        (self.button_frames.borrow().get(SPACE_BUTTON))
            .is_some_and(|frame| frame.contains(&position))
    }

    /// A tab dragged to `position`: held on the Space button for the edge's wait, the button's
    /// menu springs open, as Arc's Spaces page at the sidebar's edge, with what the tab can go to:
    /// another Space, or a new one. While it's up, the drag's moves highlight its rows.
    pub(super) fn drag_over_space_button(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self
            .drag
            .as_ref()
            .filter(|drag| drag.moving && !drag.dropped)
        else {
            return;
        };
        if drag.space_menu {
            return crate::menu::dragged_over(position, cx);
        }
        let on = self.moving_to_spaces(drag) && self.on_space_button(position);
        if drag.on_space_button.is_some() == on {
            return;
        }
        let since = on.then(Instant::now);
        if let Some(drag) = self.drag.as_mut() {
            drag.on_space_button = since;
        }
        let Some(since) = since else {
            return;
        };
        cx.spawn_in(window, async move |shell, cx| {
            cx.background_executor().timer(EDGE_WAIT).await;
            shell
                .update_in(cx, |shell, window, cx| {
                    let held = shell
                        .drag
                        .as_ref()
                        .filter(|drag| drag.moving && !drag.dropped);
                    if held.and_then(|drag| drag.on_space_button) != Some(since) {
                        return;
                    }
                    // check.sh waits for this line.
                    log!("browser: drag opened the space menu");
                    shell.space_menu_for_drag(window, cx);
                })
                .ok();
        })
        .detach();
    }

    /// The Space button's menu for a tab dragged onto it: the Spaces it can go to, the one it's in
    /// dimmed, and New Space…, led by the drag.
    fn space_menu_for_drag(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(frame) = self.button_frames.borrow().get(SPACE_BUTTON).copied() else {
            return;
        };
        let Some(tab) = self.drag.as_ref().map(|drag| drag.tab) else {
            return;
        };
        let mut items: Vec<(MenuEntry, MenuAction)> = Vec::new();
        let mut targets = Vec::new();
        let spaces = (self.spaces.iter()).filter(|space| space.agent.is_empty());
        for space in spaces {
            let mut entry = MenuEntry::named(space.name.clone(), space.id != self.space)
                .checked(space.id == self.space);
            entry.scrolls = true;
            match SpaceIcon::parse(&space.icon).glyph_path() {
                Some(path) => entry.icon = Some(path),
                None => entry.mark = Some(crate::space_icon::letter(&space.name).into()),
            }
            items.push((entry, Box::new(|_, _| {})));
            targets.push(Some(space.id));
        }
        items.push((MenuEntry::line(), Box::new(|_, _| {})));
        targets.push(None);
        items.push((MenuEntry::new("New Space…", true), Box::new(|_, _| {})));
        targets.push(Some(NEW_SPACE));
        let entries: Vec<MenuEntry> = items.iter().map(|(entry, _)| entry.clone()).collect();
        let width = crate::menu::root_width(&entries, window);
        let foot = frame.top() - px(crate::menu::FROM_CONTROL);
        let at = gpui::point(
            frame.right() - px(width),
            foot - px(crate::menu::height(&entries)),
        );
        crate::menu::lead_by_drag(cx);
        self.show_space_switcher(items, at, cx);
        self.space_menu_targets = targets;
        if let Some(drag) = self.drag.as_mut().filter(|drag| drag.tab == tab) {
            drag.space_menu = true;
        }
    }

    /// The drag let go with the Space button's menu up, which it led: on a Space's row the tab goes
    /// to that Space, as Move to sends it, and on New Space… it makes a Space with it. Anywhere
    /// else the menu closes and the drop is the list's. Returns whether the menu took it.
    pub(super) fn drop_on_space_menu(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(drag) = self
            .drag
            .as_mut()
            .filter(|drag| drag.space_menu && !drag.dropped)
        else {
            return false;
        };
        drag.space_menu = false;
        let tab = drag.tab;
        let target = crate::menu::drag_target(cx).and_then(|at| self.space_menu_targets.get(at));
        let target = target.copied().flatten();
        cx.defer(crate::menu::drag_ended);
        let Some(target) = target else {
            return false;
        };
        self.cancel_drag(cx);
        // check.sh waits for this line.
        log!("browser: dropped tab {tab} on the space menu");
        match target {
            NEW_SPACE => self.new_space_with(Some(tab), cx),
            space => self.move_to_space(tab, space, false, cx),
        }
        true
    }

    /// Logs where the Space button is drawn, its centre in the window, as "browser: space button
    /// in window `window` at x y", each time that moves, for check.sh to click it there whatever
    /// width the sidebar has.
    fn space_button_drawn(&self) -> impl IntoElement + use<> {
        let (drawn, window) = (self.space_button_drawn.clone(), self.browser.window());
        gpui::canvas(
            |_, _, _| {},
            move |bounds, _, _, _| {
                let centre = bounds.center();
                let x = f32::from(centre.x).round() as i32;
                let y = f32::from(centre.y).round() as i32;
                if drawn.replace(Some((x, y))) != Some((x, y)) {
                    // check.sh reads this line.
                    log!("browser: space button in window {window} at {x} {y}");
                }
            },
        )
        .absolute()
        .inset_0()
    }

    /// Off the Mac, the control after the Library button that opens ilo's menu, which the Mac's
    /// menu bar holds (#3148). It keeps its plate while its menu is up, as the name row keeps its fill.
    pub(super) fn menu_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let lit = self.app_menu && self.menu_open;
        div()
            .id(MENU_CONTROL)
            .when(self.a11y, |button| button.button_named(MENU_CONTROL_NAME))
            // A screen reader says the control as the keys' while the ring is on it.
            .when(self.a11y && self.keyed_menu.is_some(), |button| {
                button.aria_active_descendant()
            })
            .absolute()
            .left(px(super::library::LIBRARY_BACK_INSET + LIBRARY_BUTTON))
            .bottom(px((FOOTER_HEIGHT - NAV_SIZE) / 2.))
            .size(px(NAV_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(NAV_RADIUS))
            .block_mouse_except_scroll()
            .when(lit, |button| button.bg(self.palette.plate(NAV_HOVERED)))
            .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)))
            .on_hover(
                cx.listener(|shell, hovered: &bool, _, cx| shell.hover_menu_control(*hovered, cx)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, _, cx| shell.browser.tooltip(None, cx)),
            )
            // On the click, as the actions' overflow opens its menu: the menu opens above the
            // control, away from the pointer, so a release after it opened would read as a drag
            // that let go outside it and close it, as Windows' did.
            .on_click(cx.listener(|shell, _, _, cx| shell.app_menu(cx)))
            .child(self.button_frame(MENU_CONTROL))
            .children(self.menu_ring())
            .child(
                svg()
                    .path("icons/nav-menu.svg")
                    .size(px(NAV_GLYPH))
                    .text_color(self.palette.glyph(ICON_INK)),
            )
    }

    /// The pointer has come onto or left the menu control, whose tooltip names it as the header's
    /// buttons' name theirs, with no keys, as the menu has none.
    fn hover_menu_control(&self, hovered: bool, cx: &mut Context<Self>) {
        let busy = self.dragging().is_some() || self.menu_row.is_some() || self.menu_open;
        let frame = self.button_frames.borrow().get(MENU_CONTROL).copied();
        let tip = frame.filter(|_| hovered && !busy).map(|frame| {
            let tip = Tip::named(MENU_CONTROL_NAME.into());
            (tip, Anchor::under(frame, self.viewport))
        });
        self.hover_tip(tip, cx);
    }
}

/// The Space button's spoken name for the Space `name`, `at` n of m (words SP1): Linux's screen
/// readers take one string, the Mac's VoiceOver a label and a value, which it reads as
/// "Space, Work, 2 of 4, menu button". With one Space there's no place to say.
fn said(
    name: &str,
    at: Option<(usize, usize)>,
    one_string: bool,
) -> (SharedString, Option<SharedString>) {
    let value = match at {
        Some((n, m)) if m > 1 => format!("{name}, {n} of {m}"),
        _ => name.to_string(),
    };
    match one_string {
        true => (format!("Space: {value}").into(), None),
        false => ("Space".into(), Some(value.into())),
    }
}

/// The Space button's key among the buttons' frames, and its id.
pub(super) const SPACE_BUTTON: &str = "space-button";

/// The menu control's key among the buttons' frames, and its id.
pub(super) const MENU_CONTROL: &str = "app-menu";
/// What the menu control's tooltip and the screen reader call it: the menu the Mac's menu bar names
/// after the app.
const MENU_CONTROL_NAME: &str = concat!(env!("APP_NAME"), " menu");

#[cfg(test)]
mod tests {
    use std::{rc::Rc, time::Duration};

    use gpui::{Entity, Modifiers, Pixels, Point, TestAppContext, VisualTestContext, point, px};

    use super::{
        ButtonSwitch, EDGE_WAIT, PILL, PILL_FADE, SLIDE, space_button_right, space_button_width,
    };
    use crate::shell::drag::detents::{X, press};
    use crate::shell::{FOOTER_HEIGHT, Shell, library::LIBRARY_BACK_INSET, recorder::Recorder};
    use crate::tokens::shell::LIBRARY_BUTTON;

    /// A window with two Spaces, the first shown.
    fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        cx.update(|window, _| window.activate_window());
        shell.update(cx, |shell, cx| {
            let spaces = [1, 2]
                .map(|id| {
                    (
                        id,
                        format!("Space {id}").into(),
                        None,
                        1,
                        "".into(),
                        "".into(),
                        "".into(),
                    )
                })
                .to_vec();
            shell.set_spaces(spaces, cx);
            shell.set_space(1, cx);
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (shell, cx, host)
    }

    /// The middle of the foot's row, this far from the sidebar's far side.
    fn foot(shell: &Entity<Shell>, cx: &mut VisualTestContext, from_right: f32) -> Point<Pixels> {
        shell.read_with(cx, |shell, _| {
            let y = f32::from(shell.viewport.height) - FOOTER_HEIGHT / 2.;
            point(px(shell.width() - from_right), px(y))
        })
    }

    /// The middle of the Space button.
    fn button(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Point<Pixels> {
        foot(shell, cx, space_button_right() + space_button_width() / 2.)
    }

    /// The Space button wears its plate from the click that opens its menu until the menu closes,
    /// and another menu doesn't give it one.
    #[gpui::test]
    fn the_space_button_wears_its_plate_while_its_menu_is_up(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        assert!(!shell.read_with(cx, |shell, _| shell.space_button_lit()));
        let at = button(&shell, cx);
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        let titles = host.menus.borrow().last().cloned().unwrap_or_default();
        assert!(
            titles.iter().any(|(title, _)| title == "New Space…"),
            "the click opened no menu: {titles:?}"
        );
        // As check.sh's choose_space reads it: each Space, the shown one checked and the first not
        // last, then New Space…, with no line and no ⌃ key between them.
        let shown = crate::menu::described(&host.entries.borrow());
        assert!(
            shown.starts_with("Space 1 (checked), Space 2, New Space…, "),
            "the menu as the log has it: {shown}"
        );
        assert!(!shown.contains('⌃'), "the log has a key: {shown}");
        assert!(
            shell.read_with(cx, |shell, _| shell.space_button_lit()),
            "no plate while its menu is up"
        );
        cx.update(|_, cx| host.dismiss(cx));
        cx.run_until_parked();
        assert!(
            !shell.read_with(cx, |shell, _| shell.space_button_lit()),
            "the plate outlived the menu"
        );

        // A right-click opens the Space shown's own menu, which isn't the button's.
        let at = button(&shell, cx);
        cx.simulate_event(gpui::MouseDownEvent {
            button: gpui::MouseButton::Right,
            position: at,
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        });
        cx.run_until_parked();
        let titles = host.menus.borrow().last().cloned().unwrap_or_default();
        assert!(
            titles.iter().any(|(title, _)| title == "Rename Space…"),
            "a right-click opened no Space menu: {titles:?}"
        );
        assert!(shell.read_with(cx, |shell, _| shell.menu_open && !shell.space_button_lit()));
        cx.update(|_, cx| host.dismiss(cx));
    }

    /// The Space button takes the pointer in its plate's box and not past it.
    #[gpui::test]
    fn the_space_button_takes_the_pointer_in_its_plates_box(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        let past = foot(&shell, cx, space_button_right() + space_button_width() + 1.);
        cx.simulate_click(past, Modifiers::none());
        cx.run_until_parked();
        assert!(
            host.menus.borrow().is_empty(),
            "a click past the plate opened a menu"
        );
        let at = button(&shell, cx);
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.menus.borrow().len(), 1);
        // The frame the menu hangs from and a drag hits is the plate's own, not shifted by its
        // padding, so the menu's right edge is the plate's.
        let (frame, width) = shell.read_with(cx, |shell, _| {
            (
                shell.button_frames.borrow()[super::SPACE_BUTTON],
                shell.width(),
            )
        });
        let right = width - space_button_right();
        assert_eq!(f32::from(frame.right()), right);
        assert_eq!(f32::from(frame.left()), right - space_button_width());
    }

    #[gpui::test]
    fn the_library_button_wears_its_plate_while_the_library_is_open(cx: &mut TestAppContext) {
        let (shell, cx, _host) = window(cx);
        assert!(!shell.read_with(cx, |shell, _| shell.library_lit()));
        let y = shell.read_with(cx, |shell, _| {
            f32::from(shell.viewport.height) - FOOTER_HEIGHT / 2.
        });
        let button = point(px(LIBRARY_BACK_INSET + LIBRARY_BUTTON / 2.), px(y));
        cx.simulate_click(button, Modifiers::none());
        cx.run_until_parked();
        assert!(
            shell.read_with(cx, |shell, _| shell.library_open && shell.library_lit()),
            "no plate with the Library open"
        );
        shell.update(cx, |shell, cx| shell.set_library(false, cx));
        cx.run_until_parked();
        assert!(
            !shell.read_with(cx, |shell, _| shell.library_lit()),
            "the plate outlived the Library"
        );
    }

    /// Draws a frame, as the window's next would.
    fn frame(cx: &mut VisualTestContext) {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    /// A switch in this window: the button's marks slide with the rows, the new Space's coming in
    /// from its side, and once the rows are in place the Space's name shows in its pill over the
    /// button for its moment and goes.
    #[gpui::test]
    fn after_a_switch_the_marks_slide_with_the_rows_and_then_the_name_shows(
        cx: &mut TestAppContext,
    ) {
        let (shell, cx, _host) = window(cx);
        shell.update(cx, |shell, cx| shell.set_space(2, cx));
        let started = |shell: &Shell| {
            shell
                .space_button_switch
                .as_ref()
                .map(|s| s.start.is_some())
        };
        assert_eq!(shell.read_with(cx, |shell, _| started(shell)), Some(false));
        frame(cx);
        assert!(
            shell.read_with(cx, |shell, _| shell.settle.is_some()
                && started(shell) == Some(false)),
            "the pill came while the rows still slid"
        );
        let places = shell.read_with(cx, |shell, _| shell.space_button_places());
        assert!(
            matches!(places[..], [(2, x), (1, _)] if x > 0.),
            "the marks didn't slide with the rows: {places:?}"
        );
        cx.executor().advance_clock(Duration::from_secs(1));
        frame(cx);
        assert_eq!(shell.read_with(cx, |shell, _| started(shell)), Some(true));
        cx.executor().advance_clock(Duration::from_secs_f32(SLIDE));
        frame(cx);
        assert!(shell.read_with(cx, |shell, _| {
            shell
                .space_button_switch
                .as_ref()
                .is_some_and(|switch| switch.settled)
        }));
        assert!(
            shell.read_with(cx, |shell, _| shell.space_pill().is_some()),
            "no name's pill"
        );
        cx.executor().advance_clock(Duration::from_secs_f32(PILL));
        frame(cx);
        assert!(
            shell.read_with(cx, |shell, _| shell.space_button_switch.is_none()),
            "the pill outlived its moment"
        );
        // The core telling the window which Space it shows, with none before, slides nothing.
        shell.update(cx, |shell, cx| shell.set_space(2, cx));
        assert!(shell.read_with(cx, |shell, _| shell.space_button_switch.is_none()));
    }

    /// The button says the Space and its place among the Spaces, as one string on Linux and as a
    /// label and value elsewhere, and only the name with one Space (words SP1).
    #[test]
    fn the_space_button_says_the_space_and_its_place() {
        use super::said;
        let pair = |(label, value): (gpui::SharedString, Option<gpui::SharedString>)| {
            (label.to_string(), value.map(|value| value.to_string()))
        };
        let mac = |label: &str, value: &str| (label.to_string(), Some(value.to_string()));
        assert_eq!(
            pair(said("Work", Some((2, 4)), false)),
            mac("Space", "Work, 2 of 4")
        );
        assert_eq!(
            pair(said("Work", Some((1, 1)), false)),
            mac("Space", "Work")
        );
        assert_eq!(
            pair(said("Work", Some((2, 4)), true)),
            ("Space: Work, 2 of 4".to_string(), None)
        );
        assert_eq!(
            pair(said("Work", Some((1, 1)), true)),
            ("Space: Work".to_string(), None)
        );
    }

    /// The pill fades in, holds, and fades out by its end.
    #[test]
    fn the_pill_fades_in_and_out_over_its_moment() {
        assert_eq!(ButtonSwitch::pill(0.), 0.);
        assert_eq!(ButtonSwitch::pill(PILL / 2.), 1.);
        assert!(ButtonSwitch::pill(PILL_FADE / 2.) < 1.);
        assert!(ButtonSwitch::pill(PILL - PILL_FADE / 2.) < 1.);
        assert_eq!(ButtonSwitch::pill(PILL), 0.);
        assert_eq!(ButtonSwitch::along(SLIDE), 1.);
    }

    /// A tab held on the Space button for the edge's wait springs its menu open, with the Spaces
    /// it can go to, the one it's in dimmed, and New Space…; one only passing it opens nothing,
    /// and the right edge under the button doesn't page.
    #[gpui::test]
    fn a_tab_held_on_the_space_button_springs_its_menu_open(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        shell.update(cx, |shell, cx| {
            let folders = crate::shell::Folders::default();
            shell.set_tabs(1, Vec::new(), vec![3, 4], Vec::new(), folders, cx);
        });
        cx.run_until_parked();
        frame(cx);
        let y = press(&shell, cx, 4);
        let over = button(&shell, cx);
        let drag = |shell: &Entity<Shell>, cx: &mut VisualTestContext, to: Point<Pixels>| {
            shell.update_in(cx, |shell, window, cx| {
                shell.drag_to(to, cx);
                shell.drag_over_space_button(to, window, cx);
            });
        };
        drag(&shell, cx, point(px(X), px(y + 10.)));
        drag(&shell, cx, over);
        drag(&shell, cx, point(px(X), px(y + 10.)));
        cx.executor().advance_clock(EDGE_WAIT);
        cx.run_until_parked();
        assert!(
            host.menus.borrow().is_empty(),
            "a tab passing the button opened its menu"
        );

        drag(&shell, cx, over);
        cx.run_until_parked();
        assert!(
            host.menus.borrow().is_empty(),
            "the menu sprang open at once"
        );
        cx.executor().advance_clock(EDGE_WAIT);
        cx.run_until_parked();
        assert!(
            host.shown.take().is_empty(),
            "the edge under the button paged"
        );
        let titles = host
            .menus
            .borrow()
            .last()
            .cloned()
            .expect("no menu sprang open");
        let titles: Vec<(&str, bool)> = (titles.iter())
            .filter(|(title, _)| !title.is_empty())
            .map(|(title, enabled)| (title.as_ref(), *enabled))
            .collect();
        assert_eq!(
            titles,
            [("Space 1", false), ("Space 2", true), ("New Space…", true)]
        );
        assert!(shell.read_with(cx, |shell, _| {
            shell.drag.as_ref().is_some_and(|drag| drag.space_menu) && shell.space_button_lit()
        }));
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.space_menu_targets.clone()),
            [Some(1), Some(2), None, Some(super::NEW_SPACE)]
        );
    }
}
