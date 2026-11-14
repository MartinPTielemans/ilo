//! The Spaces' pages as they page (docs/ux-spec.md › Spaces › Switching): the URL field and the
//! favorites are the page's, so the shown Space's move with its page, and the page of the Space
//! beside draws its own, as they show once it's shown.

use std::collections::HashMap;

use gpui::{
    AnyElement, App, Context, MouseButton, MouseDownEvent, MouseUpEvent, Point, div, prelude::*, px,
};

#[cfg(test)]
use gpui::canvas;

use super::{
    Command, FOOTER_HEIGHT, LIST_TOP, PLATE_INSET, Shell, TILE_HEIGHT, URL_FIELD_HEIGHT,
    URL_FIELD_TOP, drag::in_sidebar,
};

/// What rests at a point of the shown Space's URL field and favorites.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum AtRest {
    Field,
    CopyUrl,
    SiteControls,
    /// DevTools' button, in Developer Mode.
    DevTools,
    /// A pinned extension's action, whose button takes its own presses.
    Action,
    Tile(u64),
}

impl Shell {
    /// How far the shown Space's URL field and favorites are from their places: with its page
    /// while another Space's comes in beside it, else in place, as past the last Space, where its
    /// title and tabs rubber-band under them (Arc).
    pub(super) fn top_offset(&self) -> f32 {
        let pages = self.pages();
        match pages.len() {
            0 | 1 => 0.,
            _ => pages
                .iter()
                .find(|&&(space, _)| space == self.space)
                .map_or(0., |&(_, x)| x),
        }
    }

    /// `part`, the shown Space's URL field or favorites, with its page.
    pub(super) fn with_page(&self, part: impl IntoElement) -> AnyElement {
        let x = self.top_offset();
        if x == 0. {
            return part.into_any_element();
        }
        let page = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(x))
            .w(px(self.width()))
            .child(part);
        div()
            .absolute()
            .size_full()
            .overflow_hidden()
            .child(page)
            .into_any_element()
    }

    /// The pages of the Spaces beside the shown one's, while they page: each with its URL field,
    /// its favorites and its rows under them, as they show once it's shown, at its offset. They
    /// take no input.
    pub(super) fn beside_pages(&self, cx: &App) -> Vec<gpui::Div> {
        let width = self.width();
        let pages = self
            .pages()
            .into_iter()
            .filter(|&(space, _)| space != self.space);
        pages
            .filter_map(|(space, x)| {
                let space = self.spaces.iter().find(|other| other.id == space)?;
                let tab = self.browser.space_tab(space.id);
                let favorites = self.favorites_of(space);
                let rows = div()
                    .absolute()
                    .top(px(self.list_top_with(favorites.len())))
                    .left_0()
                    .w(px(width))
                    .bottom(px(FOOTER_HEIGHT))
                    .overflow_hidden()
                    .child(self.still_page(space, 0., cx));
                let page = div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(x))
                    .w(px(width))
                    .child(self.still_url_field(tab))
                    .child(rows)
                    .children(self.still_tiles(&favorites, tab, cx));
                Some(div().absolute().size_full().overflow_hidden().child(page))
            })
            .collect()
    }

    /// What of the URL field and favorites is at `at` in the sidebar with their page `x` from its
    /// place and the tiles at `tiles`, under the favorites' top.
    fn top_at(&self, at: Point<f32>, x: f32, tiles: &HashMap<u64, [f32; 3]>) -> Option<AtRest> {
        let width = self.width();
        if !(0. ..width).contains(&at.x) {
            return None;
        }
        let x = at.x - x;
        let field = (URL_FIELD_TOP..URL_FIELD_TOP + URL_FIELD_HEIGHT).contains(&at.y);
        if field && (PLATE_INSET..width - PLATE_INSET).contains(&x) {
            return Some(self.field_part_at(x - PLATE_INSET));
        }
        // Tiles that scroll with the list go under the URL field.
        let top = self.tiles_top();
        let mut tiles = tiles.iter().filter(|_| at.y >= LIST_TOP);
        tiles.find_map(|(&tab, &[left, tile_top, tile_width])| {
            let over = (left..left + tile_width).contains(&x)
                && (top + tile_top..top + tile_top + TILE_HEIGHT).contains(&at.y);
            over.then_some(AtRest::Tile(tab))
        })
    }

    /// A press on the URL field or the favorites while the frame last drawn doesn't have them at
    /// rest: their page slides with a Space switch, or tiles move to their places. As a press on
    /// the list does (`press_at_rest`), it acts on what rests under the pointer, and its release
    /// clicks that, so a click does what it does once they settle. Over a place where nothing
    /// rests, it does nothing. A swipe's fingers hold the page where they have it, and a press
    /// then is what's drawn's. Returns whether it took the press.
    pub(super) fn press_top_at_rest(
        &mut self,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let busy = self.swipe.is_some() || self.form().is_some() || self.renaming.is_some();
        if busy || self.dragging().is_some() || (self.settle.is_some() && self.drag.is_some()) {
            return false;
        }
        let resting = self.resting_tiles();
        let at = in_sidebar(event.position, self.sidebar_origin.get());
        let (rest, drawn) = {
            let (x, tiles) = &*self.drawn_top.borrow();
            if *x == 0. && *tiles == resting {
                return false;
            }
            (self.top_at(at, 0., &resting), self.top_at(at, *x, tiles))
        };
        match (rest, event.button) {
            (None, _) if drawn.is_none() => return false,
            (None, _) => self.browser.tooltip(None, cx),
            (Some(AtRest::Tile(tab)), MouseButton::Left) => self.pressed_tile(tab, event, cx),
            (Some(AtRest::Tile(tab)), MouseButton::Right) => self.tab_menu(tab, event.position, cx),
            (Some(AtRest::Field), MouseButton::Left) => self.pressed_bar_target(cx),
            (Some(AtRest::SiteControls), MouseButton::Left) => {
                self.run(Command::SiteControls, "sidebar", cx)
            }
            (Some(AtRest::CopyUrl | AtRest::DevTools), MouseButton::Left) => {}
            _ => return false,
        }
        self.rest_press = rest.filter(|_| event.button == MouseButton::Left);
        true
    }

    /// The release of a press `press_top_at_rest` took: a click on what it rests on, if it rests
    /// under the pointer still.
    pub(super) fn release_at_rest(&mut self, event: &MouseUpEvent, cx: &mut Context<Self>) {
        let Some(pressed) = self.rest_press.take() else {
            return;
        };
        let at = in_sidebar(event.position, self.sidebar_origin.get());
        let resting = self.resting_tiles();
        if event.button != MouseButton::Left || self.top_at(at, 0., &resting) != Some(pressed) {
            return;
        }
        match pressed {
            AtRest::Field => self.clicked_bar_target(Command::EditUrl, "sidebar", cx),
            AtRest::CopyUrl => self.copy_url_from_button(cx),
            AtRest::DevTools => self.run(Command::DevTools, "sidebar", cx),
            AtRest::Tile(tab) => self.clicked_tile(tab, cx),
            AtRest::SiteControls | AtRest::Action => {}
        }
    }

    /// Records where the page beside drew `tab`'s tile, or its URL field as 0, `outset` past its
    /// padding box (`framed`), for the tests.
    #[cfg(test)]
    pub(super) fn beside_framed(&self, tab: u64, outset: f32) -> impl IntoElement + use<> {
        let frames = self.beside_frames.clone();
        canvas(
            move |bounds, _, _| _ = frames.borrow_mut().insert(tab, bounds),
            |_, _, _, _| {},
        )
        .absolute()
        .inset(px(-outset))
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::time::Duration;

    use gpui::{
        Entity, Modifiers, MouseButton, Point, TestAppContext, VisualTestContext, point, px,
    };

    use super::AtRest;
    use crate::shell::{
        PLATE_INSET, Shell, TILE_HEIGHT, URL_FIELD_HEIGHT, URL_FIELD_TOP, recorder::Recorder,
    };

    /// A window on the second of two Spaces, of two profiles, the first with favorites 10 and 11
    /// side by side and the second with 20, from which ⌃1 has begun to slide the first Space's
    /// page in from the left.
    fn sliding_back(
        cx: &mut TestAppContext,
    ) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = |id: u64| {
                (
                    id,
                    format!("Space {id}").into(),
                    None,
                    id,
                    "".into(),
                    "".into(),
                    "".into(),
                )
            };
            shell.set_spaces(vec![space(1), space(2)], cx);
            shell.set_favorites(vec![(10, 1), (11, 1), (20, 2)], cx);
            shell.set_space(2, cx);
            if let Some(settle) = &mut shell.settle {
                settle.start -= Duration::from_secs(1);
            }
        });
        cx.run_until_parked();
        shell.update(cx, |shell, cx| shell.set_space(1, cx));
        cx.run_until_parked();
        assert!(
            shell.read_with(cx, |shell, _| shell.settle.is_some()),
            "the Space didn't slide"
        );
        host.take();
        (shell, host, cx)
    }

    /// Steps the slide on, a frame at a time, until the frame drawn has `drawn` under `at`, in the
    /// window, where `rest` rests.
    fn slide_until(
        shell: &Entity<Shell>,
        cx: &mut VisualTestContext,
        at: Point<f32>,
        rest: AtRest,
        drawn: Option<AtRest>,
    ) {
        loop {
            let found = shell.read_with(cx, |shell, _| {
                let (x, tiles) = &*shell.drawn_top.borrow();
                let rests = shell.top_at(at, 0., &shell.resting_tiles());
                assert_eq!(rests, Some(rest), "the press isn't where it rests");
                shell.top_at(at, *x, tiles) == drawn && *x != 0.
            });
            if found {
                return;
            }
            shell.update(cx, |shell, cx| {
                let settle = shell.settle.as_mut().expect("the slide ended first");
                settle.start -= Duration::from_millis(4);
                cx.notify();
            });
            cx.run_until_parked();
        }
    }

    /// A click as a trackpad's tap makes it, while the slide goes on between its press and its
    /// release.
    fn click_mid_slide(shell: &Entity<Shell>, cx: &mut VisualTestContext, at: Point<f32>) {
        let origin = shell.read_with(cx, |shell, _| shell.sidebar_origin.get());
        let at = origin + point(px(at.x), px(at.y));
        cx.simulate_mouse_move(at, None, Modifiers::none());
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        shell.update(cx, |shell, cx| {
            if let Some(settle) = &mut shell.settle {
                settle.start -= Duration::from_millis(48);
            }
            cx.notify();
        });
        cx.run_until_parked();
        cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    }

    /// ⌃1, from the second Space: a click mid-slide where the first Space's favorite 10 rests,
    /// with its favorite 11 drawn there on its way in, shows 10, as it does once the slide has
    /// settled, and never just hangs 11's tooltip (#486).
    #[gpui::test]
    fn a_click_while_spaces_slide_shows_the_favorite_at_rest(cx: &mut TestAppContext) {
        let (shell, host, cx) = sliding_back(cx);
        let at = shell.read_with(cx, |shell, _| {
            let [left, top, width] = shell.resting_tiles()[&10];
            point(
                left + width / 2.,
                shell.tiles_top() + top + TILE_HEIGHT / 2.,
            )
        });
        slide_until(&shell, cx, at, AtRest::Tile(10), Some(AtRest::Tile(11)));
        click_mid_slide(&shell, cx, at);
        let calls = host.take();
        assert!(calls.contains(&"select 10".to_string()), "{calls:?}");
        assert!(!calls.contains(&"select 11".to_string()), "{calls:?}");
    }

    /// ⌃1, from the second Space: a click mid-slide where the first Space's URL field rests, at
    /// its end, with the second Space's field drawn there on its way out, opens the command bar
    /// on the shown Space's URL, as it does once the slide has settled (#486).
    #[gpui::test]
    fn a_click_while_spaces_slide_opens_the_url_field_at_rest(cx: &mut TestAppContext) {
        let (shell, host, cx) = sliding_back(cx);
        let at = shell.read_with(cx, |shell, _| {
            point(
                shell.width() - PLATE_INSET - 8.,
                URL_FIELD_TOP + URL_FIELD_HEIGHT / 2.,
            )
        });
        // The page beside's field takes no input: under the pointer, none of the shown one's is.
        slide_until(&shell, cx, at, AtRest::Field, None);
        host.command_bars.borrow_mut().clear();
        click_mid_slide(&shell, cx, at);
        assert_eq!(
            host.command_bars.borrow().len(),
            1,
            "the click mid-slide opened no bar"
        );
    }
}
