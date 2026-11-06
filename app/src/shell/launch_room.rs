//! What the launch's picture of the sidebar needs to show in a window a little shorter or taller
//! than the one it was taken in (crate::launch_picture).

use super::{LIST_TOP, Shell};
use crate::launch_picture::Taken;
use crate::tokens::shell::LIST_TRAIL;
use crate::tokens::shell::TALL_FAVORITES;

impl Shell {
    /// The sidebar's picture as the window `width` by `height`, in the host's points, draws it
    /// now: where its foot starts, and how much shorter and taller the window may get with every
    /// control where the UI draws it, the difference taken out of or added to the empty room under
    /// the list's last row.
    pub fn launch_picture(&self, width: f64, height: f64) -> Taken {
        let viewport = f32::from(self.viewport.height);
        // Hidden, the picture is the strip at the window's edge, with nothing on it.
        if self.sidebar_hidden || self.page_fullscreen {
            return Taken {
                width,
                height,
                foot_top: viewport.into(),
                shorter: height,
                taller: height,
            };
        }
        let foot_top = viewport - self.list_bottom();
        // Rows reaching under the foot would be cut higher in a shorter list and a taller one would
        // show rows the picture doesn't have. A scrolled list could scroll back in a taller one,
        // and the legacy knob, there while the list overflows, would change length.
        let scrolled = f32::from(self.list_scroll.offset().y) < -0.5;
        let overflows = f32::from(self.list_scroll.max_offset().y) > 0.5;
        let room = if scrolled || (self.legacy_scroller && overflows) {
            0.
        } else {
            let list = self.list_scroll.bounds();
            // The list ends in LIST_TRAIL of nothing, which is room too.
            let content_end = (0..self.list_scroll.children_count())
                .filter_map(|child| self.list_scroll.bounds_for_item(child))
                .map(|child| f32::from(child.bottom()))
                .fold(f32::from(list.top()) + LIST_TRAIL, f32::max);
            f32::from(list.bottom()).min(foot_top) - (content_end - LIST_TRAIL)
        };
        let (mut shorter, mut taller) = (room, room);
        // Past TALL_FAVORITES of the window's height, the favorites scroll with the list.
        let scrolls_from = (self.list_top() - LIST_TOP) / TALL_FAVORITES;
        if self.tiles_scroll() {
            taller = taller.min(scrolls_from - viewport);
        } else {
            shorter = shorter.min(viewport - scrolls_from);
        }
        // The opened foot is capped at a share of the window's height.
        if self.foot_cap().is_some() {
            (shorter, taller) = (0., 0.);
        }
        Taken {
            width,
            height,
            foot_top: foot_top.into(),
            shorter: shorter.max(0.).into(),
            taller: taller.max(0.).into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{Entity, TestAppContext, VisualTestContext, px, size};

    use crate::shell::{Folders, Shell, recorder::Recorder};
    use crate::tokens::shell::ROW_PITCH;

    /// A Space with `today`'s tabs in a window 1000 by `height`, drawn, and its picture.
    fn taken(
        today: Vec<u64>,
        height: f32,
        cx: &mut TestAppContext,
    ) -> crate::launch_picture::Taken {
        let host = Rc::new(Recorder::default());
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host, cx));
        cx.simulate_resize(size(px(1000.), px(height)));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(1, Vec::new(), today, Vec::new(), Folders::default(), cx);
        });
        draw(&shell, cx);
        draw(&shell, cx);
        shell.read_with(cx, |shell, _| shell.launch_picture(1000., height.into()))
    }

    fn draw(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }

    /// The room is what's free under the last row: a row more takes its pitch of it, and the
    /// foot stays at the window's bottom.
    #[gpui::test]
    fn a_short_list_leaves_the_room_under_its_last_row(cx: &mut TestAppContext) {
        let three = taken(vec![10, 11, 12], 700., cx);
        let four = taken(vec![10, 11, 12, 13], 700., cx);
        assert!(three.shorter > 100., "{three:?}");
        assert_eq!((three.shorter, three.taller), (three.taller, three.shorter));
        assert!(
            (three.shorter - four.shorter - f64::from(ROW_PITCH)).abs() < 0.5,
            "{three:?} {four:?}"
        );
        assert_eq!(three.foot_top, four.foot_top);
        assert!(three.foot_top < 700. && three.foot_top > 600., "{three:?}");
    }

    /// Rows reaching under the foot leave the window no other height.
    #[gpui::test]
    fn an_overflowing_list_leaves_no_room(cx: &mut TestAppContext) {
        let full = taken((10..40).collect(), 340., cx);
        assert_eq!((full.shorter, full.taller), (0., 0.));
    }
}
