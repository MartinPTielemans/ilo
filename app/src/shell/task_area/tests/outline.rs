//! The lines round a page a thread shows: the page's own outline, as any page has it, and the
//! agent's ring over it while the thread drives the page.

use gpui::{Hsla, rgba};

use super::*;
use crate::palette::Tone;
use crate::tokens::shell::agent_pointer::RING;
use crate::tokens::shell::{PAGE_OUTLINE, PAGE_OUTLINE_LIGHT, PAGE_OUTLINE_WIDTH};

/// Draws the window as it is in `tone`.
fn toned(shell: &Entity<Shell>, tone: Tone, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, _| shell.force_tone(Some(tone)));
    draw(shell, cx);
    draw(shell, cx);
    assert_eq!(shell.read_with(cx, |shell, _| shell.palette.tone), tone);
}

/// The watched page's card: its part of the page area.
fn page_card(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Bounds<Pixels> {
    shell.read_with(cx, |shell, _| {
        let (card, _) = shell.page_frame(shell.viewport);
        watched(shell.page_part(card))
    })
}

/// The card the outline was drawn round in the last frame, and where the ring was.
fn lines(
    shell: &Entity<Shell>,
    cx: &mut VisualTestContext,
) -> (Option<Bounds<Pixels>>, Option<Bounds<Pixels>>) {
    shell.read_with(cx, |shell, _| {
        let area = shell.task_area.as_ref().unwrap();
        (area.outlined.get(), area.ringed.get())
    })
}

/// The outline was drawn round `card` as a page's is: outside it by the line's width on its top,
/// right and bottom, none on its left, in `tone`'s own ink.
fn outlined(shell: &Entity<Shell>, card: Bounds<Pixels>, tone: Tone, cx: &mut VisualTestContext) {
    let width = px(PAGE_OUTLINE_WIDTH);
    assert_eq!(
        lines(shell, cx).0,
        Some(card),
        "{tone:?}: no outline round it"
    );
    let mut line = shell.read_with(cx, |shell, _| shell.watched_outline(card));
    let style = line.style();
    // Outside the card by its own width where it has a line, and at the card's edge on the left.
    let at = |edge: Pixels| Some(gpui::Length::from(edge));
    assert!(
        style.inset.left == at(card.left())
            && style.inset.top == at(card.top() - width)
            && style.size.width == at(card.size.width + width)
            && style.size.height == at(card.size.height + 2. * width),
        "{tone:?}: not outside the card on its top, right and bottom alone"
    );
    let sides = &style.border_widths;
    assert!(
        sides.left == Some(px(0.).into()),
        "{tone:?}: a line on the left"
    );
    for side in [sides.top, sides.right, sides.bottom] {
        assert!(side == Some(width.into()), "{tone:?}: a side with no line");
    }
    let ink = match tone {
        Tone::Dark => PAGE_OUTLINE,
        Tone::Light => PAGE_OUTLINE_LIGHT,
    };
    assert_eq!(style.border_color, Some(Hsla::from(rgba(ink))), "{tone:?}");
}

/// A page a thread shows has the page's own outline at rest, in Dark and in Light: on its top,
/// right and bottom and none on its left, with no ring while the thread isn't at work in it.
#[gpui::test]
fn a_watched_page_has_the_pages_own_outline_in_both_tones(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    for tone in [Tone::Dark, Tone::Light] {
        toned(&shell, tone, cx);
        let card = page_card(&shell, cx);
        outlined(&shell, card, tone, cx);
        assert_eq!(lines(&shell, cx).1, None, "{tone:?}: a ring at rest");
    }
}

/// Watched in its tile, the outline is round the page's card, the ground the tile shows on, in
/// both tones.
#[gpui::test]
fn a_page_watched_in_its_tile_has_the_pages_own_outline_in_both_tones(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching_tile(&shell, &host, cx);
    for tone in [Tone::Dark, Tone::Light] {
        toned(&shell, tone, cx);
        let card = page_card(&shell, cx);
        outlined(&shell, card, tone, cx);
        assert_eq!(lines(&shell, cx).1, None, "{tone:?}: a ring at rest");
    }
}

/// While the thread drives the page, the ring is drawn round it as before, over the outline: its
/// band covers the line on each side that has one, in both tones.
#[gpui::test]
fn the_ring_is_drawn_over_the_outline_while_the_thread_drives(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell_with_task(cx);
    watching_working(&shell, cx);
    for tone in [Tone::Dark, Tone::Light] {
        toned(&shell, tone, cx);
        let card = page_card(&shell, cx);
        outlined(&shell, card, tone, cx);
        let ring = lines(&shell, cx).1;
        assert_eq!(ring, Some(card.dilate(px(RING))), "{tone:?}");
    }
}

/// A page driven in its tile keeps its ring round the tile, with the outline round its card
/// under it, in both tones.
#[gpui::test]
fn the_ring_round_a_driven_tile_is_drawn_with_the_outline(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    host.shows_tiles.set(Some(true));
    watching_working(&shell, cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.watch_tile_shown()),
        Some(30)
    );
    for tone in [Tone::Dark, Tone::Light] {
        toned(&shell, tone, cx);
        let card = page_card(&shell, cx);
        outlined(&shell, card, tone, cx);
        let tile = tile_place(&shell, cx);
        let ring = lines(&shell, cx).1;
        assert_eq!(ring, Some(tile.dilate(px(RING))), "{tone:?}");
    }
}
