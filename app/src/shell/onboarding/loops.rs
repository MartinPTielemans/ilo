//! First run's loops (#4093): a step's feature at work in a small window in the sheet's slot,
//! drawn from made-up rows by the step's time alone. The window is the real one's at a small size:
//! a Space's palette, its glow (glow.rs), its selected row's plate and its Space dots, all clipped
//! to the small window. Nothing in one is read from the profile or the core, and nothing it shows
//! is kept. It plays only while its step shows on a visible window (Shell::onboarding_looping),
//! from its first visible frame; with Reduce Motion it's its last frame, still.

use gpui::{AnyElement, PathBuilder, canvas, div, point, prelude::*, px};

use super::{Colors, Step, ease_in_out, ease_out};
use crate::glow;
use crate::palette::{Palette, Tone};
use crate::theme::Theme;
use crate::tokens::palette::INK;
use crate::tokens::shell::onboarding::loops as t;

/// What keeps where a loop draws a part, by its name, for a test (Shell::place).
pub(super) type Place<'a> = &'a dyn Fn(&'static str) -> Option<AnyElement>;

/// The steps with a loop in their slot.
pub(super) fn has_loop(step: Step) -> bool {
    matches!(
        step,
        Step::Intro
            | Step::Import
            | Step::Favorites
            | Step::DefaultBrowser
            | Step::SearchEngine
            | Step::Color
            | Step::Account
    )
}

/// Where a loop is `secs` after it started, or its last frame while `still`.
pub(super) fn at(secs: Option<f32>, still: bool) -> f32 {
    match secs {
        Some(secs) if !still => secs % t::PERIOD,
        _ => t::PERIOD,
    }
}

/// The welcome's: the pointer goes to a tab and presses it, and the tab takes the selected plate
/// and its page shows, as a tab switch does: the second, the third, then the first again. It
/// comes as the loop starts and goes before it ends, so the last frame is the window alone.
pub(super) fn welcome(t_: f32, colors: Colors, place: Place) -> impl IntoElement {
    const ROWS: [&str; 3] = ["Field notes", "Trail map", "Recipes"];
    const ORDER: [usize; 3] = [1, 2, 0];
    const PLACES: [&str; 3] = ["row-0", "row-1", "row-2"];
    let palette = Palette::of(None, tone(colors));
    let rest = (t::SIDEBAR + 110., t::ROWS_TOP + 4. * t::ROW + 40.);
    let mut pointer = rest;
    let mut shown = 0;
    let mut pressed = None;
    for (index, (&start, &row)) in t::WELCOME_AT.iter().zip(&ORDER).enumerate() {
        let from = match index {
            0 => rest,
            _ => row_point(ORDER[index - 1]),
        };
        if t_ >= start {
            let moved = ease_in_out(((t_ - start) / t::WELCOME_MOVE).min(1.));
            pointer = lerp(from, row_point(row), moved);
        }
        let press = start + t::WELCOME_MOVE + t::WELCOME_CLICK;
        if t_ >= press {
            shown = row;
            pressed = Some(t_ - press);
        }
    }
    let seen = (t_ / t::POINTER_FADE)
        .min((t::PERIOD - t::POINTER_FADE - t_) / t::POINTER_FADE)
        .clamp(0., 1.);
    let rows = ROWS.iter().enumerate().map(|(index, name)| {
        let plate = if index == shown { 1. } else { 0. };
        row(name, index, plate, &palette).children(place(PLACES[index]))
    });
    // The press's ring round the pointer's tip, growing as it fades.
    let ring = pressed
        .filter(|&since| since < t::WELCOME_PRESS)
        .map(|since| {
            let grown = since / t::WELCOME_PRESS;
            let side = 10. + 10. * ease_out(grown);
            div()
                .absolute()
                .left(px(pointer.0 - side / 2.))
                .top(px(pointer.1 - side / 2.))
                .size(px(side))
                .rounded_full()
                .border_1()
                .border_color(palette.faint(0.5 * (1. - grown)))
        });
    let weights = [(palette, 1.)];
    let sidebar = column(div().children(rows), &[(1., 1.), (0., 1.)], &palette);
    let shown = window(&palette, &weights, sidebar, page(shown, &palette))
        .children(place("window"))
        .children(ring)
        .child(arrow(pointer, colors, place).opacity(seen));
    centred(shown)
}

/// The import's: another browser's window above ilo's, its bookmarks and then its tabs dimming
/// one by one as each lands in ilo's sidebar, the bookmarks as favorites and the tabs as tabs,
/// then the dots of the Spaces that came and what came, said in the page. All of it goes again
/// at the loop's end, so it starts as it began; its last frame, still, is what came.
pub(super) fn import(t_: f32, colors: Colors, place: Place) -> impl IntoElement {
    const BOOKMARKS: [&str; 4] = ["Calendar", "Mail", "Wave", "Code"];
    const TABS: [&str; 4] = ["Roadmap", "Budget", "Lakeside", "Parks"];
    const TILES: [&str; 4] = ["tile-0", "tile-1", "tile-2", "tile-3"];
    const ROWS: [&str; 4] = ["tab-0", "tab-1", "tab-2", "tab-3"];
    let palette = Palette::of(None, tone(colors));
    // How far the loop's end has taken it all away again; none in the still last frame.
    let out = match t_ >= t::PERIOD {
        true => 0.,
        false => ((t_ - (t::PERIOD - t::IMPORT_OUT)) / t::IMPORT_OUT).clamp(0., 1.),
    };
    let [bookmarks_at, tabs_at] = t::IMPORT_AT;
    // How far the `index`th of a kind starting at `start` has landed.
    // How far a part starting at `from` has come over `over`: none before it starts, as the
    // easing's first step is a hair over nothing.
    let come = |from: f32, over: f32| match t_ > from {
        true => ease_out(((t_ - from) / over).min(1.)) * (1. - out),
        false => 0.,
    };
    let landed =
        |start: f32, index: usize| come(start + index as f32 * t::IMPORT_STAGGER, t::IMPORT_LAND);
    let seen = |start: f32| come(start, t::IMPORT_FADE);
    // In the other browser, each dims as it lands.
    let dimmed = |landed: f32| 1. - (1. - t::IMPORT_DIM) * landed;
    let chip = |name: &'static str, landed: f32| {
        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .opacity(dimmed(landed))
            .child(
                div()
                    .size(px(7.))
                    .rounded(px(t::ICON_RADIUS))
                    .bg(palette.faint(0.3)),
            )
            .child(name)
    };
    let tabs = TABS.iter().enumerate().map(|(index, &name)| {
        chip(name, landed(tabs_at, index))
            .h(px(16.))
            .px(px(6.))
            .rounded(px(t::PLATE_RADIUS))
            .bg(palette.faint(if index == 0 { 0.1 } else { 0. }))
    });
    let bookmarks = (BOOKMARKS.iter().enumerate())
        .map(|(index, &name)| chip(name, landed(bookmarks_at, index)));
    let lights = (0..3).map(|_| div().size(px(6.)).rounded_full().bg(palette.faint(0.22)));
    let other = div()
        .flex_none()
        .w(px(t::WINDOW_WIDTH))
        .h(px(t::STRIP_HEIGHT))
        .px(px(10.))
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(6.))
        .overflow_hidden()
        .rounded(px(t::WINDOW_RADIUS))
        .bg(palette.page)
        .border_1()
        .border_color(palette.faint(0.12))
        .text_size(px(t::TEXT_SIZE))
        .text_color(palette.text(0.75))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .children(lights)
                .child(div().ml(px(6.)).flex().gap(px(2.)).children(tabs)),
        )
        .child(div().flex().gap(px(12.)).pl(px(6.)).children(bookmarks))
        .children(place("other"));
    // In ilo's sidebar, the favorites as tiles above the tabs, each coming down into place.
    let tiles = (0..BOOKMARKS.len()).filter_map(|index| {
        let landed = landed(bookmarks_at, index);
        (landed > 0.).then(|| tile(index, landed, &palette).children(place(TILES[index])))
    });
    let rows = TABS.iter().enumerate().filter_map(|(index, &name)| {
        let landed = landed(tabs_at, index);
        (landed > 0.).then(|| {
            let top = t::ROWS_TOP + index as f32 * t::ROW - t::IMPORT_DROP * (1. - landed);
            row(name, index, 0., &palette)
                .top(px(top))
                .opacity(landed)
                .children(place(ROWS[index]))
        })
    });
    // The Spaces that came with them, a dot each beside the one shown.
    let spaces = seen(t::IMPORT_SPACES);
    let dots = [(1., 1.), (0., spaces), (0., spaces)];
    let sidebar =
        column(div().children(tiles).children(rows), &dots, &palette).children(place("sidebar"));
    // What came, said in the page as the import's Done says it.
    let came = seen(t::IMPORT_CAME);
    let said = (came > 0.).then(|| {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(6.))
            .opacity(came)
            .text_size(px(t::TEXT_SIZE))
            .text_color(palette.text(0.75))
            .child(div().size(px(9.)).rounded_full().bg(palette.faint(0.5)))
            .child("4 favorites, 4 tabs, 3 Spaces")
            .children(place("came"))
    });
    let ilo = window(
        &palette,
        &[(palette, 1.)],
        sidebar,
        div().size_full().children(said),
    )
    .children(place("window"));
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(t::STRIP_GAP))
        .child(other)
        .child(ilo)
}

/// The favorites': the sites the history visited most, in a strip over ilo's window, each ticked
/// in turn but the last, and each ticked landing in the sidebar as a favorite's tile, as the
/// import's do; then what came, said in the page. All of it goes again at the loop's end; its last
/// frame, still, is what came, the last site unticked.
pub(super) fn favorites(t_: f32, colors: Colors, place: Place) -> impl IntoElement {
    const SITES: [&str; 4] = ["Weather", "Recipes", "Trails", "Forum"];
    const TILES: [&str; 3] = ["tile-0", "tile-1", "tile-2"];
    const TICKS: [&str; 4] = ["tick-0", "tick-1", "tick-2", "tick-3"];
    let palette = Palette::of(None, tone(colors));
    let out = match t_ >= t::PERIOD {
        true => 0.,
        false => ((t_ - (t::PERIOD - t::IMPORT_OUT)) / t::IMPORT_OUT).clamp(0., 1.),
    };
    // How far a part starting at `from` has come over `over`: none before it starts.
    let come = |from: f32, over: f32| match t_ > from {
        true => ease_out(((t_ - from) / over).min(1.)) * (1. - out),
        false => 0.,
    };
    let ticked_at = |index: usize| t::FAVORITES_AT + index as f32 * t::FAVORITES_STAGGER;
    let ticked = |index: usize| come(ticked_at(index), t::IMPORT_FADE);
    let landed = |index: usize| come(ticked_at(index) + t::FAVORITES_TICK, t::IMPORT_LAND);
    // A tick as the import's list draws one: an empty ring, filled in the ink once ticked.
    let tick = |index: usize| {
        let filled = match index < TILES.len() {
            true => ticked(index),
            false => 0.,
        };
        div()
            .relative()
            .flex_none()
            .size(px(8.))
            .rounded_full()
            .border_1()
            .border_color(palette.faint(0.4))
            .child(
                div()
                    .absolute()
                    .inset(px(-1.))
                    .rounded_full()
                    .bg(palette.text(0.75))
                    .opacity(filled),
            )
            .children(place(TICKS[index]))
    };
    let sites = SITES.iter().enumerate().map(|(index, &name)| {
        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .child(tick(index))
            .child(
                div()
                    .size(px(7.))
                    .rounded(px(t::ICON_RADIUS))
                    .bg(palette.faint(0.3)),
            )
            .child(name)
    });
    let strip = div()
        .flex_none()
        .w(px(t::WINDOW_WIDTH))
        .h(px(t::STRIP_HEIGHT))
        .px(px(12.))
        .flex()
        .items_center()
        .justify_between()
        .overflow_hidden()
        .rounded(px(t::WINDOW_RADIUS))
        .bg(palette.page)
        .border_1()
        .border_color(palette.faint(0.12))
        .text_size(px(t::TEXT_SIZE))
        .text_color(palette.text(0.75))
        .children(sites)
        .children(place("strip"));
    let tiles = (0..TILES.len()).filter_map(|index| {
        let landed = landed(index);
        (landed > 0.).then(|| tile(index, landed, &palette).children(place(TILES[index])))
    });
    let rows = ["Field notes", "Trail map"]
        .iter()
        .enumerate()
        .map(|(index, name)| row(name, index, if index == 0 { 1. } else { 0. }, &palette));
    let sidebar = column(div().children(tiles).children(rows), &[(1., 1.)], &palette)
        .children(place("sidebar"));
    let came = come(t::FAVORITES_CAME, t::IMPORT_FADE);
    let said = (came > 0.).then(|| {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(6.))
            .opacity(came)
            .text_size(px(t::TEXT_SIZE))
            .text_color(palette.text(0.75))
            .child(div().size(px(9.)).rounded_full().bg(palette.faint(0.5)))
            .child("3 favorites added")
            .children(place("came"))
    });
    let ilo = window(
        &palette,
        &[(palette, 1.)],
        sidebar,
        div().size_full().children(said),
    )
    .children(place("window"));
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(t::STRIP_GAP))
        .child(strip)
        .child(ilo)
}

/// The default browser's: a message in another app, its link clicked, and the link opening in
/// ilo's window as a new tab at the top of the sidebar, selected, its page shown. The pointer
/// goes once it has clicked, and the tab goes again at the loop's end; its last frame, still, is
/// the link open in ilo.
pub(super) fn default_browser(t_: f32, colors: Colors, place: Place) -> impl IntoElement {
    const TABS: [&str; 2] = ["Field notes", "Recipes"];
    let palette = Palette::of(None, tone(colors));
    let out = match t_ >= t::PERIOD {
        true => 0.,
        false => ((t_ - (t::PERIOD - t::IMPORT_OUT)) / t::IMPORT_OUT).clamp(0., 1.),
    };
    // How far a part starting at `from` has come over `over`: none before it starts.
    let come = |from: f32, over: f32| match t_ > from {
        true => ease_out(((t_ - from) / over).min(1.)) * (1. - out),
        false => 0.,
    };
    // The pointer, in the other app: from rest to the link, its press, then gone.
    let [x, y] = t::DEFAULT_LINK;
    let rest = (t::WINDOW_WIDTH - 70., 16.);
    let moved = match t_ > t::DEFAULT_AT {
        true => ease_in_out(((t_ - t::DEFAULT_AT) / t::WELCOME_MOVE).min(1.)),
        false => 0.,
    };
    let pointer = lerp(rest, (x, y), moved);
    let press = t::DEFAULT_AT + t::WELCOME_MOVE + t::WELCOME_CLICK;
    let seen = (t_ / t::POINTER_FADE)
        .min((press + t::WELCOME_PRESS + t::POINTER_FADE - t_) / t::POINTER_FADE)
        .clamp(0., 1.);
    let ring = (t_ >= press && t_ < press + t::WELCOME_PRESS).then(|| {
        let grown = (t_ - press) / t::WELCOME_PRESS;
        let side = 10. + 10. * ease_out(grown);
        div()
            .absolute()
            .left(px(pointer.0 - side / 2.))
            .top(px(pointer.1 - side / 2.))
            .size(px(side))
            .rounded_full()
            .border_1()
            .border_color(palette.faint(0.5 * (1. - grown)))
    });
    // The link, underlined, its corner `default-link` less half a line in from the message's.
    let link = div()
        .absolute()
        .left(px(x - 12.))
        .top(px(y - 5.))
        .flex()
        .flex_col()
        .text_color(palette.text(0.9))
        .child("trailmap.example")
        .child(div().h(px(1.)).bg(palette.faint(0.5)))
        .children(place("link"));
    let other = div()
        .relative()
        .flex_none()
        .w(px(t::WINDOW_WIDTH))
        .h(px(t::STRIP_HEIGHT))
        .rounded(px(t::WINDOW_RADIUS))
        .bg(palette.page)
        .border_1()
        .border_color(palette.faint(0.12))
        .text_size(px(t::TEXT_SIZE))
        .text_color(palette.text(0.75))
        .child(
            div()
                .absolute()
                .left(px(12.))
                .top(px(12.))
                .size(px(10.))
                .rounded_full()
                .bg(palette.faint(0.3)),
        )
        .child(
            div()
                .absolute()
                .left(px(x - 12.))
                .top(px(12.))
                .child("Ana · the map for Saturday"),
        )
        .child(link)
        .children(ring)
        .child(arrow(pointer, colors, place).opacity(seen))
        .children(place("other"));
    // In ilo, the link lands as a new tab above the others, which make room, and takes the plate.
    let opened = come(press + t::DEFAULT_OPEN, t::IMPORT_LAND);
    let new = (opened > 0.).then(|| {
        row("Trail map", 0, opened, &palette)
            .top(px(t::ROWS_TOP - t::IMPORT_DROP * (1. - opened)))
            .opacity(opened)
            .children(place("tab-new"))
    });
    let rows = TABS.iter().enumerate().map(|(index, name)| {
        let plate = if index == 0 { 1. - opened } else { 0. };
        row(name, index, plate, &palette).top(px(t::ROWS_TOP + (index as f32 + opened) * t::ROW))
    });
    let sidebar = column(
        div().children(new).children(rows),
        &[(1., 1.), (0., 1.)],
        &palette,
    )
    .children(place("sidebar"));
    let pages = div()
        .absolute()
        .inset_0()
        .child(page(0, &palette).opacity(1. - opened))
        .child(
            page(2, &palette)
                .opacity(opened)
                .children((opened > 0.).then(|| place("page-new")).flatten()),
        );
    let ilo = window(&palette, &[(palette, 1.)], sidebar, pages).children(place("window"));
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(t::STRIP_GAP))
        .child(other)
        .child(ilo)
}

/// The search engine's: the bar comes up over a page, a few words are typed in it, and ↩ opens
/// the picked engine's results as a new tab at the top of the sidebar, selected, its page shown,
/// as the bar goes. All of it goes again at the loop's end; its last frame, still, is the results.
pub(super) fn search(t_: f32, colors: Colors, place: Place) -> impl IntoElement {
    const TABS: [&str; 2] = ["Roadmap", "Budget 2027"];
    const WORDS: &str = "largest city parks";
    let palette = Palette::of(None, tone(colors));
    let out = match t_ >= t::PERIOD {
        true => 0.,
        false => ((t_ - (t::PERIOD - t::IMPORT_OUT)) / t::IMPORT_OUT).clamp(0., 1.),
    };
    // How far a part starting at `from` has come over `over`: none before it starts.
    let come = |from: f32, over: f32| match t_ > from {
        true => ease_out(((t_ - from) / over).min(1.)) * (1. - out),
        false => 0.,
    };
    let typing = t::SEARCH_AT + t::IMPORT_FADE;
    let press = typing + t::SEARCH_TYPE + t::SEARCH_RETURN;
    let bar = (come(t::SEARCH_AT, t::IMPORT_FADE) - come(press, t::IMPORT_FADE)).max(0.);
    // The words typed so far, a letter at a time over `search-type`.
    let typed = ((t_ - typing) / t::SEARCH_TYPE).clamp(0., 1.);
    let typed = &WORDS[..(typed * WORDS.len() as f32).floor() as usize];
    let bar = (bar > 0.).then(|| {
        let [left, top] = t::SEARCH_BAR;
        div()
            .absolute()
            .left(px(left))
            .right(px(left))
            .top(px(top))
            .h(px(t::ROW + 6.))
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(px(t::PLATE_RADIUS))
            .bg(palette.page)
            .border_1()
            .border_color(palette.faint(0.2))
            .opacity(bar)
            .text_size(px(t::TEXT_SIZE))
            .text_color(palette.text(0.9))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(typed)
                    .child(div().w(px(1.)).h(px(10.)).bg(palette.faint(0.6)))
                    .children(place("words")),
            )
            .child(div().flex_1())
            .child(
                div()
                    .px(px(4.))
                    .rounded(px(t::ICON_RADIUS))
                    .bg(palette.faint(0.12))
                    .text_color(palette.text(0.6))
                    .child("↩"),
            )
            .children(place("bar"))
    });
    // ↩ opens the results as a new tab above the others, which make room, and it takes the plate.
    let opened = come(press, t::IMPORT_LAND);
    let new = (opened > 0.).then(|| {
        row("largest city parks", 0, opened, &palette)
            .top(px(t::ROWS_TOP - t::IMPORT_DROP * (1. - opened)))
            .opacity(opened)
            .children(place("tab-new"))
    });
    let rows = TABS.iter().enumerate().map(|(index, name)| {
        let plate = if index == 0 { 1. - opened } else { 0. };
        row(name, index, plate, &palette).top(px(t::ROWS_TOP + (index as f32 + opened) * t::ROW))
    });
    let sidebar = column(
        div().children(new).children(rows),
        &[(1., 1.), (0., 1.)],
        &palette,
    )
    .children(place("sidebar"));
    let pages = div()
        .absolute()
        .inset_0()
        .child(page(0, &palette).opacity(1. - opened))
        .child(
            page(1, &palette)
                .opacity(opened)
                .children((opened > 0.).then(|| place("page-new")).flatten()),
        );
    let shown = window(&palette, &[(palette, 1.)], sidebar, pages)
        .children(bar)
        .children(place("window"));
    centred(shown)
}

/// The sync's: a tab pinned in ilo's window crosses to a phone beside it under a lock and shows in
/// the phone's list; then a favorite added on the phone comes back the same way and lands in the
/// window's sidebar as a favorite's tile. All of it goes again at the loop's end; its last frame,
/// still, is both landed.
pub(super) fn sync(t_: f32, colors: Colors, place: Place) -> impl IntoElement {
    const TABS: [&str; 2] = ["Lakeside loop", "City parks"];
    let palette = Palette::of(None, tone(colors));
    let out = match t_ >= t::PERIOD {
        true => 0.,
        false => ((t_ - (t::PERIOD - t::IMPORT_OUT)) / t::IMPORT_OUT).clamp(0., 1.),
    };
    let come = |from: f32, over: f32| match t_ > from {
        true => ease_out(((t_ - from) / over).min(1.)) * (1. - out),
        false => 0.,
    };
    let [there, back] = t::SYNC_AT;
    let [phone_width, phone_height] = t::SYNC_PHONE;
    // The window and the phone, its right edge `sync-phone-out` past the window's, in one box.
    let width = t::WINDOW_WIDTH + t::SYNC_PHONE_OUT;
    let phone_left = width - phone_width;
    let phone_top = (t::WINDOW_HEIGHT - phone_height) / 2.;
    // Where the phone's list and its tiles are, in the box.
    let phone_row = (phone_left + 6., phone_top + 52.);
    let phone_tile = (phone_left + 8., phone_top + 24.);
    let mac_tile = (6., t::TILES_TOP);
    // Pinned, then carried across, the first tab's chip going from its row to the phone's list.
    let pinned = come(there, t::IMPORT_FADE);
    let crossing = |from: f32| match t_ > from {
        true => ease_in_out(((t_ - from) / t::SYNC_CROSS).min(1.)),
        false => 0.,
    };
    let across = crossing(there + t::IMPORT_FADE);
    let home = crossing(back + t::IMPORT_FADE);
    // The lock halfway along the way across, lit while something passes it.
    let lock_at = lerp((t::SIDEBAR - 20., t::ROWS_TOP), phone_row, 0.5);
    let passing = |gone: f32| (1. - 2. * (gone - 0.5).abs()).max(0.) * (1. - out);
    let lit = passing(across).max(passing(home));
    let lock = div()
        .absolute()
        .left(px(lock_at.0 - 5.))
        .top(px(lock_at.1 - 8.))
        .flex()
        .flex_col()
        .items_center()
        .opacity(0.35 + 0.65 * lit)
        .child(
            div()
                .w(px(6.))
                .h(px(5.))
                .rounded_t(px(t::BAR_RADIUS))
                .border_1()
                .border_b_0()
                .border_color(palette.text(0.9)),
        )
        .child(
            div()
                .w(px(10.))
                .h(px(8.))
                .rounded(px(t::ICON_RADIUS))
                .bg(palette.text(0.9)),
        )
        .children(place("lock"));
    let chip = |name: &'static str, at: (f32, f32), seen: f32| {
        div()
            .absolute()
            .left(px(at.0))
            .top(px(at.1))
            .h(px(16.))
            .px(px(6.))
            .flex()
            .items_center()
            .gap(px(4.))
            .rounded(px(t::PLATE_RADIUS))
            .bg(palette.page)
            .border_1()
            .border_color(palette.faint(0.2))
            .opacity(seen)
            .text_size(px(t::TEXT_SIZE))
            .text_color(palette.text(0.75))
            .child(
                div()
                    .size(px(7.))
                    .rounded(px(t::ICON_RADIUS))
                    .bg(palette.faint(0.3)),
            )
            .child(name)
    };
    let moving = (across > 0. && across < 1.).then(|| {
        let from = (6., t::ROWS_TOP);
        chip(TABS[0], lerp(from, phone_row, across), 1. - out).children(place("moving"))
    });
    let returning = (home > 0. && home < 1.).then(|| {
        chip("Trails", lerp(phone_tile, mac_tile, home), 1. - out).children(place("returning"))
    });
    // ilo's window: the pin beside the first tab, and the tile that came back.
    let pin = (pinned > 0.).then(|| {
        div()
            .absolute()
            .top(px(t::ROWS_TOP + (t::ROW - 5.) / 2.))
            .right(px(12.))
            .size(px(5.))
            .rounded_full()
            .bg(palette.text(0.75))
            .opacity(pinned)
            .children(place("pin"))
    });
    let landed_back = come(back + t::IMPORT_FADE + t::SYNC_CROSS, t::IMPORT_LAND);
    let tile_back =
        (landed_back > 0.).then(|| tile(0, landed_back, &palette).children(place("tile-back")));
    let rows = TABS
        .iter()
        .enumerate()
        .map(|(index, name)| row(name, index, if index == 0 { 1. } else { 0. }, &palette));
    let sidebar = column(
        div().children(tile_back).children(rows).children(pin),
        &[(1., 1.), (0., 1.)],
        &palette,
    )
    .children(place("sidebar"));
    let ilo = window(&palette, &[(palette, 1.)], sidebar, page(1, &palette))
        .absolute()
        .left_0()
        .top_0()
        .children(place("window"));
    // The phone: a tile for the favorite added on it, then its list, where the tab lands.
    let arrived = come(there + t::IMPORT_FADE + t::SYNC_CROSS, t::IMPORT_LAND);
    let added = come(back, t::IMPORT_FADE);
    let phone_rows = (arrived > 0.).then(|| {
        div()
            .absolute()
            .left(px(6.))
            .right(px(6.))
            .top(px(52. - t::IMPORT_DROP * (1. - arrived)))
            .h(px(t::ROW))
            .px(px(6.))
            .flex()
            .items_center()
            .gap(px(5.))
            .rounded(px(t::PLATE_RADIUS))
            .bg(palette.shown(0.))
            .opacity(arrived)
            .text_size(px(t::TEXT_SIZE))
            .text_color(palette.text(0.75))
            .child(
                div()
                    .size(px(7.))
                    .rounded(px(t::ICON_RADIUS))
                    .bg(palette.faint(0.3)),
            )
            .child(TABS[0])
            .children(place("phone-tab"))
    });
    let phone_tile = (added > 0.).then(|| {
        div()
            .absolute()
            .left(px(8.))
            .top(px(24.))
            .size(px(t::TILE))
            .rounded(px(t::PLATE_RADIUS))
            .bg(palette.shown(0.))
            .opacity(added)
            .children(place("phone-tile"))
    });
    let phone = div()
        .absolute()
        .left(px(phone_left))
        .top(px(phone_top))
        .w(px(phone_width))
        .h(px(phone_height))
        .overflow_hidden()
        .rounded(px(t::WINDOW_RADIUS))
        .bg(palette.underlay)
        .border_1()
        .border_color(palette.faint(0.2))
        .child(
            div()
                .absolute()
                .top(px(8.))
                .left(px(phone_width / 2. - 12.))
                .w(px(24.))
                .h(px(5.))
                .rounded_full()
                .bg(palette.faint(0.3)),
        )
        .children(phone_tile)
        .children(phone_rows)
        .children(place("phone"));
    let shown = div()
        .relative()
        .flex_none()
        .w(px(width))
        .h(px(t::WINDOW_HEIGHT))
        .child(ilo)
        .child(phone)
        .child(lock)
        .children(moving)
        .children(returning);
    centred(shown)
}

/// A favorite's tile at the sidebar's top, the `index`th, as far down into place as it has
/// `landed`.
fn tile(index: usize, landed: f32, palette: &Palette) -> gpui::Div {
    div()
        .absolute()
        .top(px(t::TILES_TOP - t::IMPORT_DROP * (1. - landed)))
        .left(px(6. + index as f32 * (t::TILE + t::TILE_GAP)))
        .size(px(t::TILE))
        .rounded(px(t::PLATE_RADIUS))
        .bg(palette.shown(0.))
        .opacity(landed)
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .size(px(7.))
                .rounded(px(t::ICON_RADIUS))
                .bg(palette.faint(0.3)),
        )
}

/// The colour's: the window glows with the colour picked, or none, and the sidebar's rows swipe
/// to another Space in another colour, the window's glow, its page and its Space dots going with
/// them, and back.
pub(super) fn colour(
    t_: f32,
    picked: Option<u32>,
    other: u32,
    colors: Colors,
    place: Place,
) -> impl IntoElement {
    const SPACES: [[&str; 3]; 2] = [["Inbox", "Notes", "Map"], ["Studio", "Sketches", "Palette"]];
    let tone = tone(colors);
    let next = Palette::of(Some(&Theme::of_color(other)), tone);
    // With nothing picked, the Space has the ground a color gives it and no glow, as the window
    // behind has, rather than the untinted material, lighter than its page in the dark.
    let first = match picked {
        Some(color) => Palette::of(Some(&Theme::of_color(color)), tone),
        None => Palette {
            underlay: next.underlay,
            ..Palette::of(None, tone)
        },
    };
    let palettes = [first, next];
    let [there, back] = t::COLOR_AT;
    let swiped = |start: f32| ease_in_out(((t_ - start) / t::COLOR_SWIPE).clamp(0., 1.));
    let across = swiped(there) - swiped(back);
    let weights = [(palettes[0], 1. - across), (palettes[1], across)];
    let near = palettes[if across < 0.5 { 0 } else { 1 }];
    // The Spaces side by side, slid along under the sidebar's column, which shows one at rest.
    let rows = div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(px(-t::SIDEBAR * across))
        .w(px(2. * t::SIDEBAR))
        .flex()
        .children(
            SPACES
                .iter()
                .zip(palettes)
                .enumerate()
                .map(|(space, (rows, palette))| {
                    div().relative().w(px(t::SIDEBAR)).h_full().children(
                        rows.iter().enumerate().map(move |(index, name)| {
                            let named = (space == 1 && index == 0).then(|| place("next-space-row"));
                            row(name, index, if index == 0 { 1. } else { 0. }, &palette)
                                .children(named.flatten())
                        }),
                    )
                }),
        );
    let pages = div()
        .absolute()
        .inset_0()
        .child(page(0, &palettes[0]).opacity(1. - across))
        .child(page(1, &palettes[1]).opacity(across));
    let dots = [(1. - across, 1.), (across, 1.)];
    let sidebar = column(rows, &dots, &near).children(place("sidebar"));
    let shown = window(&near, &weights, sidebar, pages).children(place("window"));
    centred(shown)
}

/// The window's tone, as the flow's colours have it.
fn tone(colors: Colors) -> Tone {
    match colors.ink == INK {
        true => Tone::Light,
        false => Tone::Dark,
    }
}

/// The sidebar's column: its rows, clipped to it, and at its foot a dot for each Space, `dots`,
/// as big and as strong as its Space shows, and as much there as the loop has it. A window of one
/// Space has a second for the Space it would swipe to, as the real foot does.
fn column(rows: impl IntoElement, dots: &[(f32, f32)], palette: &Palette) -> gpui::Div {
    let dots = dots
        .iter()
        .filter(|(_, there)| *there > 0.)
        .map(|&(shown, there)| {
            div()
                .size(px(t::DOT + (t::DOT_SHOWN - t::DOT) * shown))
                .rounded_full()
                .bg(palette.faint(t::DOT_ALPHA + (1. - t::DOT_ALPHA) * shown))
                .opacity(there)
        });
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left_0()
        .w(px(t::SIDEBAR))
        .overflow_hidden()
        .child(rows)
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(t::DOTS_BOTTOM))
                .flex()
                .justify_center()
                .items_center()
                .gap(px(t::DOT_GAP))
                .children(dots),
        )
}

/// The small window: its ground and the glows of the Spaces it shows, each as much as it shows,
/// its lights, its sidebar's column and its page card. What a loop adds over it goes in it, in its
/// terms, and nothing of it is drawn outside it.
fn window(
    palette: &Palette,
    weights: &[(Palette, f32)],
    sidebar: impl IntoElement,
    page: impl IntoElement,
) -> gpui::Div {
    let glows = weights.iter().flat_map(|&(palette, weight)| {
        [
            (glow::Corner::Foot, palette.glow),
            (glow::Corner::Far, palette.far_glow),
        ]
        .into_iter()
        .filter(move |(_, color)| color.a > 0. && weight > 0.)
        .map(move |(corner, color)| {
            glow::glow_scaled(corner, false, color, color.a * weight, t::SCALE)
        })
    });
    let lights = div()
        .absolute()
        .top(px(12.))
        .left(px(12.))
        .flex()
        .gap(px(5.))
        .children((0..3).map(|_| div().size(px(7.)).rounded_full().bg(palette.faint(0.22))));
    div()
        .relative()
        .flex_none()
        .w(px(t::WINDOW_WIDTH))
        .h(px(t::WINDOW_HEIGHT))
        .overflow_hidden()
        .rounded(px(t::WINDOW_RADIUS))
        .bg(palette.underlay)
        .border_1()
        .border_color(palette.faint(0.12))
        .children(glows)
        .child(lights)
        .child(sidebar)
        .child(
            div()
                .absolute()
                .top(px(6.))
                .right(px(6.))
                .bottom(px(6.))
                .left(px(t::SIDEBAR))
                .overflow_hidden()
                .rounded(px(t::PLATE_RADIUS))
                .bg(palette.page)
                .child(page),
        )
}

/// The small window centred in the slot.
fn centred(shown: gpui::Div) -> gpui::Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(shown)
}

/// A tab's row in the sidebar, the `index`th, with the selected plate as much as `plate`.
fn row(name: &'static str, index: usize, plate: f32, palette: &Palette) -> gpui::Div {
    div()
        .absolute()
        .top(px(t::ROWS_TOP + index as f32 * t::ROW))
        .left(px(6.))
        .w(px(t::SIDEBAR - 12.))
        .h(px(t::ROW))
        .child(
            div()
                .absolute()
                .inset_0()
                .rounded(px(t::PLATE_RADIUS))
                .bg(palette.shown(0.))
                .opacity(plate),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .px(px(7.))
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(t::TEXT_SIZE))
                .text_color(palette.text(0.75))
                .child(
                    div()
                        .size(px(9.))
                        .rounded(px(t::ICON_RADIUS))
                        .bg(palette.faint(0.3)),
                )
                .child(name),
        )
}

/// A made-up page: a heading's bar and a few lines, laid out its own way for each `index`.
fn page(index: usize, palette: &Palette) -> gpui::Div {
    let palette = *palette;
    let bar = move |width: f32, height: f32, alpha: f32| {
        div()
            .w(px(width))
            .h(px(height))
            .rounded(px(t::BAR_RADIUS))
            .bg(palette.faint(alpha))
    };
    let lines = move |count: usize| {
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .children((0..count).map(move |line| bar(150. - 22. * (line % 3) as f32, 5., 0.14)))
    };
    let body = match index {
        0 => div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(bar(180., 64., 0.1))
            .child(lines(3)),
        1 => div()
            .flex()
            .gap(px(10.))
            .child(bar(84., 110., 0.1))
            .child(lines(6)),
        _ => div().flex().flex_col().gap(px(10.)).child(lines(4)).child(
            div()
                .flex()
                .gap(px(8.))
                .children((0..3).map(move |_| bar(54., 40., 0.1))),
        ),
    };
    div()
        .absolute()
        .inset_0()
        .p(px(14.))
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(bar(96., 9., 0.3))
        .child(body)
}

/// Where the pointer presses the `index`th row and rests on it, in the window: on the row past
/// its name's end, so it covers none of it.
fn row_point(index: usize) -> (f32, f32) {
    (
        t::SIDEBAR * t::PRESS_ACROSS,
        t::ROWS_TOP + (index as f32 + 0.5) * t::ROW,
    )
}

fn lerp(from: (f32, f32), to: (f32, f32), t_: f32) -> (f32, f32) {
    (from.0 + (to.0 - from.0) * t_, from.1 + (to.1 - from.1) * t_)
}

/// The pointer, its tip at `at` in the window.
fn arrow(at: (f32, f32), colors: Colors, place: Place) -> gpui::Div {
    // An arrow's outline, its tip at the origin, as a system pointer is drawn.
    const OUTLINE: [(f32, f32); 7] = [
        (0., 0.),
        (0., 14.),
        (3.6, 10.6),
        (6.2, 16.),
        (8.4, 15.),
        (5.8, 9.8),
        (10.6, 9.8),
    ];
    let (ink, ground) = (colors.ink(0.9), colors.ground);
    div()
        .absolute()
        .left(px(at.0))
        .top(px(at.1))
        .size(px(18.))
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let shape = |grow: f32| {
                        let mut path = PathBuilder::fill();
                        let mut points = OUTLINE
                            .iter()
                            .map(|&(x, y)| bounds.origin + point(px(x * grow), px(y * grow)));
                        if let Some(first) = points.next() {
                            path.move_to(first);
                        }
                        for each in points {
                            path.line_to(each);
                        }
                        path.close();
                        path.build().ok()
                    };
                    // The ground's edge round the ink, so it reads on any page.
                    if let Some(edge) = shape(1.15) {
                        window.paint_path(edge, ground);
                    }
                    if let Some(tip) = shape(1.) {
                        window.paint_path(tip, ink);
                    }
                },
            )
            .size_full(),
        )
        .children(place("pointer"))
}
