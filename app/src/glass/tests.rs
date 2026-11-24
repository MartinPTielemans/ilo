//! What the host is told of a plate's glass, on a host recording it.

use std::cell::RefCell;

use gpui::{Context, Render, TestAppContext, div, prelude::*, px};

use super::*;
use crate::shell::scaled::scaled_from_top;

thread_local! {
    static PLATES: RefCell<Vec<Plate>> = RefCell::default();
}

fn record(_: &Window, plate: Plate) {
    PLATES.with_borrow_mut(|plates| plates.push(plate));
}

/// A plate 100 by 40 at 10, 20, drawn at `factor` its size about its top edge's middle.
struct Plated {
    factor: f32,
    opacity: f32,
}

impl Render for Plated {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(scaled_from_top(
            div()
                .absolute()
                .left(px(10.))
                .top(px(20.))
                .w(px(100.))
                .h(px(40.))
                .child(under(12., self.opacity, true)),
            self.factor,
        ))
    }
}

fn plate(left: f32, top: f32, width: f32, height: f32, radius: f32, opacity: f32) -> Plate {
    Plate {
        bounds: Bounds::new(point(px(left), px(top)), size(px(width), px(height))),
        radius,
        opacity,
        light: true,
    }
}

#[gpui::test]
fn the_glass_is_the_plate_as_drawn_scaled_and_faded_with_it(cx: &mut TestAppContext) {
    set_host(record);
    let (view, cx) = cx.add_window_view(|_, _| Plated {
        factor: 0.5,
        opacity: 0.25,
    });
    cx.run_until_parked();
    // Half its size about the middle of its top edge, at 60, 20.
    assert_eq!(
        PLATES.take(),
        [plate(35., 20., 50., 20., 6., 0.25)],
        "the glass wasn't the plate as it's drawn growing in"
    );
    view.update(cx, |plated, cx| {
        (plated.factor, plated.opacity) = (1., 1.);
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(PLATES.take(), [plate(10., 20., 100., 40., 12., 1.)]);
}

#[gpui::test]
fn a_frame_that_moves_nothing_tells_the_host_nothing(cx: &mut TestAppContext) {
    set_host(record);
    let (view, cx) = cx.add_window_view(|_, _| Plated {
        factor: 1.,
        opacity: 1.,
    });
    cx.run_until_parked();
    assert_eq!(PLATES.take().len(), 1);
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(PLATES.take(), [], "a frame at rest told the host again");
}

#[gpui::test]
fn without_a_host_nothing_is_told(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| Plated {
        factor: 1.,
        opacity: 1.,
    });
    cx.run_until_parked();
    assert_eq!(PLATES.take(), []);
}
