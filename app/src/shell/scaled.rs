//! An element painted smaller or larger than it's laid out, as a layer's transform would.

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Point, Window, point,
};

/// `child`, painted at `factor` its size about `origin`, in the window's coordinates. It's laid
/// out and hit where it would be at full size.
pub fn scaled(child: impl IntoElement, factor: f32, origin: Point<Pixels>) -> Scaled {
    Scaled {
        child: child.into_any_element(),
        factor,
        origin: Origin::At(origin),
    }
}

/// `child`, painted at `factor` its size about the middle of its top edge.
pub fn scaled_from_top(child: impl IntoElement, factor: f32) -> Scaled {
    Scaled {
        child: child.into_any_element(),
        factor,
        origin: Origin::Top,
    }
}

/// `child`, painted at `factor` its size about its middle.
pub fn scaled_from_middle(child: impl IntoElement, factor: f32) -> Scaled {
    Scaled {
        child: child.into_any_element(),
        factor,
        origin: Origin::Middle,
    }
}

pub struct Scaled {
    child: AnyElement,
    factor: f32,
    origin: Origin,
}

/// Where a scaled element stays put.
enum Origin {
    At(Point<Pixels>),
    /// The middle of its top edge.
    Top,
    Middle,
}

impl IntoElement for Scaled {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Scaled {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.factor == 1. {
            return self.child.paint(window, cx);
        }
        let origin = match self.origin {
            Origin::At(origin) => origin,
            Origin::Top => point(bounds.center().x, bounds.top()),
            Origin::Middle => bounds.center(),
        };
        window.with_scale(self.factor, origin, |window| self.child.paint(window, cx));
    }
}
