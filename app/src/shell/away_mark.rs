//! Mock only (#2338, not for merge): a pinned tab's or favorite's return mark at rest, in the
//! form PREVIEW_AWAY_MARK names (badge, notch or dot), on the icon it carries.

use std::sync::OnceLock;

use gpui::{AnyElement, Hsla, IntoElement, div, prelude::*, px, rgb, svg};

use super::{FAVICON_SIZE, HOVERED, Row, SECONDARY, Shell};
use crate::palette::Tone;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Form {
    Badge,
    Notch,
    Dot,
}

pub(super) fn form() -> Option<Form> {
    static FORM: OnceLock<Option<Form>> = OnceLock::new();
    *FORM.get_or_init(|| match std::env::var("PREVIEW_AWAY_MARK").ok()?.as_str() {
        "badge" => Some(Form::Badge),
        "notch" => Some(Form::Notch),
        "dot" => Some(Form::Dot),
        _ => None,
    })
}

impl Shell {
    /// `icon`, in its 18 pt box, with the return mark at its bottom right while `away`. `ground`
    /// is what's drawn under the icon, opaque, which rings the mark off it. An unloaded tab's
    /// mark dims with its icon, as the box around both does.
    pub(super) fn with_away_mark(&self, icon: AnyElement, away: bool, ground: Hsla) -> AnyElement {
        let Some(form) = form().filter(|_| away) else {
            return icon;
        };
        let ink = ground.blend(self.palette.text(SECONDARY));
        // Out past the box's corner by `out`, so the mark sits on the drawn icon's corner.
        let (size, ring, out) = match form {
            Form::Badge => (11., 1.5, 3.),
            Form::Notch => (11., 0., 3.),
            Form::Dot => (7., 1.5, 1.5),
        };
        let mark = div()
            .absolute()
            .right(px(-out))
            .bottom(px(-out))
            .size(px(size))
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .border(px(ring))
            .border_color(ground)
            .bg(match form {
                Form::Notch => ground,
                _ => ink,
            });
        let mark = match form {
            Form::Badge => mark.child(
                svg()
                    .path("icons/undo-2.svg")
                    .size(px(7.))
                    .text_color(ground),
            ),
            Form::Notch => mark.child(svg().path("icons/undo-2.svg").size(px(9.)).text_color(ink)),
            Form::Dot => mark,
        };
        div()
            .relative()
            .size(px(FAVICON_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .child(icon)
            .child(mark)
            .into_any_element()
    }

    /// The sidebar's colour where the rows are, as the preview draws it on Linux, sampled from its
    /// captures: the window's material isn't a colour the shell knows, and on the Mac it's
    /// translucent, so a build would cut the icon rather than paint over it.
    pub(super) fn sidebar_ground(&self) -> Hsla {
        match self.palette.tone {
            Tone::Dark => rgb(0x2f2f2f).into(),
            Tone::Light => rgb(0xd9d9d9).into(),
        }
    }

    /// What's under `tab`'s row's favicon, opaque: the window, with the selected plate or the
    /// hover fill over it. A split's segments are on their row's.
    pub(super) fn row_ground(&self, tab: u64) -> Hsla {
        let row = Row::Tab(self.panes(tab).first().copied().unwrap_or(tab));
        let hover = self.hover_fill(row).unwrap_or(0.);
        let under = self.sidebar_ground();
        match Some(row) == self.selected_row() {
            true => under.blend(self.palette.selected(hover)),
            false => under.blend(self.palette.plate(HOVERED).opacity(hover)),
        }
    }
}
