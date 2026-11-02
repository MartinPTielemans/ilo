//! The picture of a page that another window shows, which the window it left draws in its card, and
//! a click on it takes the page back (docs/ux-spec.md › Window › Several windows; ours).

use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{
        Arc, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use gpui::{
    AnyElement, AsyncApp, Bounds, Context, Corners, MouseButton, Pixels, RenderImage, WeakEntity,
    Window, canvas, div, hsla, point, prelude::*, px, size,
};
use image::{Frame, RgbaImage};

use super::{CARD_RADIUS, Shell, new_space::ease_out, pane_frames};
use crate::elevation::Level;

use crate::tokens::shell::picture::REST;
/// It steps back from the card's size to REST as the page goes to the other window, easing out.
const STEP_BACK: f32 = 0.2;
use crate::tokens::shell::picture::VEIL;
use crate::tokens::shell::picture::RIM;

static REQUESTS: AtomicU64 = AtomicU64::new(1);

/// The id of a new request for a picture, which no other request of the app's has.
pub(crate) fn next_request() -> u64 {
    REQUESTS.fetch_add(1, Ordering::Relaxed)
}

thread_local! {
    /// The pictures the windows show, by tab, held by the windows alone: the window a page leaves
    /// shows at once the one the window taking it showed, of the page as it was here.
    static SHOWN: RefCell<HashMap<u64, (Weak<RenderImage>, (f32, f32))>> =
        RefCell::new(HashMap::new());
}

/// The core's answer to `request`, a picture of a page, for whichever of `shells` asked. It reaches
/// them once the update under way, if any, is over: an answer that came inside the call that asked
/// would come while that shell is being updated, and a second update of it panics.
pub fn picture_answered(
    shells: Vec<WeakEntity<Shell>>,
    request: u64,
    picture: Option<PagePicture>,
    cx: &AsyncApp,
) {
    cx.spawn(async move |cx| {
        for shell in shells {
            shell.update(cx, |shell, cx| shell.page_pictured(request, picture.clone(), cx)).ok();
        }
    })
    .detach();
}

/// A page's picture, in the BGRA GPUI draws, and its size in pixels.
#[derive(Clone)]
pub struct PagePicture {
    image: Arc<RenderImage>,
    size: (f32, f32),
}

impl PagePicture {
    /// The core's picture: `width` by `height` pixels, row after row, 4 bytes each in BGRA order.
    pub fn from_bgra(bgra: Vec<u8>, width: u32, height: u32) -> Option<Self> {
        let image = RgbaImage::from_raw(width, height, bgra)?;
        Some(Self {
            image: Arc::new(RenderImage::new(vec![Frame::new(image)])),
            size: (width as f32, height as f32),
        })
    }

    /// The picture as GPUI draws it, and its size in pixels.
    pub(crate) fn image(&self) -> Arc<RenderImage> {
        self.image.clone()
    }

    pub(crate) fn size(&self) -> (f32, f32) {
        self.size
    }

    /// The one a window shows of `tab`, if any does.
    fn shown(tab: u64) -> Option<Self> {
        SHOWN.with_borrow(|shown| {
            let (image, size) = shown.get(&tab)?;
            Some(Self { image: image.upgrade()?, size: *size })
        })
    }

    /// A window shows it, of `tab`, in place of any other.
    fn share(&self, tab: u64) {
        SHOWN.with_borrow_mut(|shown| {
            shown.retain(|_, (image, _)| image.strong_count() > 0);
            shown.insert(tab, (Arc::downgrade(&self.image), self.size));
        });
    }
}

/// The scale of the picture going from `from` to `to`, since `start`.
#[derive(Clone, Copy)]
struct Move {
    start: Instant,
    from: f32,
    to: f32,
    duration: f32,
}

impl Move {
    /// From the card's size to REST, as the page goes.
    fn step_back() -> Self {
        Self { start: Instant::now(), from: 1., to: REST, duration: STEP_BACK }
    }

    fn at(&self, now: Instant) -> f32 {
        let t = now.saturating_duration_since(self.start).as_secs_f32() / self.duration;
        self.from + (self.to - self.from) * ease_out(t)
    }

    fn done(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.start).as_secs_f32() >= self.duration
    }
}

/// What a window shows in place of a page another window shows: the picture of the tab it has
/// selected. At most one per window.
pub(super) struct Picture {
    tab: u64,
    /// The last request made to the core, and whether its answer hasn't come.
    request: u64,
    pending: bool,
    image: Option<PagePicture>,
    /// Whether the window has drawn `image`, for the handover lines.
    drawn: bool,
    motion: Option<Move>,
    /// A press on it has asked for the page.
    taken: bool,
}

impl Picture {
    /// How big it is, as a share of the card, at `now`.
    fn scale(&self, now: Instant) -> f32 {
        self.motion.map_or(REST, |motion| motion.at(now))
    }
}

impl Shell {
    /// The core says another window shows the page of the tab this one has selected, or with 0 that
    /// this one shows a page again. The picture the window taking it showed, if it showed one,
    /// shows here at once, and a fresh one replaces it where it is once it comes.
    pub fn page_elsewhere(&mut self, tab: u64, cx: &mut Context<Self>) {
        if tab == 0 {
            return self.drop_picture(cx);
        }
        // A report that was overtaken by the window's own pick.
        if tab != self.selected || self.picture.as_ref().is_some_and(|picture| picture.tab == tab) {
            return;
        }
        self.drop_picture(cx);
        let image = PagePicture::shown(tab);
        self.picture = Some(Picture {
            tab,
            request: 0,
            pending: false,
            motion: image.is_some().then(Move::step_back),
            image,
            drawn: false,
            taken: false,
        });
        self.ask_picture(cx);
        cx.notify();
    }

    /// Whether the window shows `tab`'s picture, or waits for it, in place of its page.
    pub(super) fn pictured(&self, tab: u64) -> bool {
        self.picture.as_ref().is_some_and(|picture| picture.tab == tab)
    }

    /// The window selected `tab` or none: the picture of any other tab goes.
    pub(super) fn selected_for_picture(&mut self, tab: u64, cx: &mut Context<Self>) {
        if !self.pictured(tab) {
            self.drop_picture(cx);
        }
    }

    /// The window is in front: its picture is of the page as it is now.
    pub(super) fn refresh_picture(&mut self, cx: &mut Context<Self>) {
        let seen = self.picture.as_ref().is_some_and(|picture| {
            picture.image.is_some() && !picture.pending && !picture.taken
        });
        if seen {
            self.ask_picture(cx);
        }
    }

    fn ask_picture(&mut self, cx: &mut Context<Self>) {
        let Some(picture) = &mut self.picture else {
            return;
        };
        picture.request = next_request();
        picture.pending = true;
        let (tab, request) = (picture.tab, picture.request);
        let window = self.browser.window();
        crate::log_handover("picture asked", &format!("window {window}, tab {tab}"));
        self.browser.page_picture(tab, request, cx);
    }

    fn drop_picture(&mut self, cx: &mut Context<Self>) {
        let Some(picture) = self.picture.take() else {
            return;
        };
        if let Some(shown) = picture.image {
            self.stale_pictures.push(shown.image);
            let window = self.browser.window();
            crate::log_handover("picture dropped", &format!("window {window}, tab {}", picture.tab));
        }
        cx.notify();
    }

    /// The core answers `request` with the picture, or none.
    pub fn page_pictured(
        &mut self,
        request: u64,
        image: Option<PagePicture>,
        cx: &mut Context<Self>,
    ) {
        if self.talk_pictured(request, &image, cx) {
            return;
        }
        let window = self.browser.window();
        let Some(picture) = self.picture.as_mut().filter(|picture| picture.request == request)
        else {
            return;
        };
        picture.pending = false;
        let Some(image) = image else {
            return;
        };
        image.share(picture.tab);
        // The page steps back as it goes, the first time; a fresh picture of it replaces the old
        // where it is.
        if picture.image.is_none() {
            picture.motion = Some(Move::step_back());
        }
        if let Some(old) = picture.image.replace(image) {
            self.stale_pictures.push(old.image);
        }
        picture.drawn = false;
        crate::log_handover("picture shown", &format!("window {window}, tab {}", picture.tab));
        // check.sh waits for this line.
        log!("browser: window {window} pictures tab {}", picture.tab);
        cx.notify();
    }

    /// A press on the picture takes the page back at once, by the call a press on the tab's row
    /// makes. The page covers the picture as it comes.
    fn press_picture(&mut self, cx: &mut Context<Self>) {
        let Some(picture) = self.picture.as_mut() else {
            return;
        };
        if picture.taken || picture.image.is_none() {
            return;
        }
        picture.taken = true;
        let tab = picture.tab;
        log!("browser: command select via picture");
        self.select(tab, cx);
    }

    /// The card of the page the picture stands for: the focused pane's, in a split view.
    fn picture_card(&self) -> Bounds<Pixels> {
        let (frame, _) = self.page_frame(self.viewport);
        let panes = self.panes(self.selected);
        let focused = panes.iter().position(|&pane| pane == self.selected).unwrap_or(0);
        let (stacked, shares) = (self.stacked(self.selected), self.shares(self.selected));
        pane_frames(frame, panes.len(), stacked, &shares)[focused]
    }

    /// Where the picture is at `now`: fitted to its scale of the card, centred in it.
    pub(super) fn picture_bounds(&self, now: Instant) -> Option<Bounds<Pixels>> {
        let picture = self.picture.as_ref()?;
        let ready = picture.image.as_ref()?;
        Some(fit(self.picture_card(), ready.size, picture.scale(now)))
    }

    /// Steps the motion for a frame, and lets go of the pictures the window has no use for.
    pub(super) fn step_picture(&mut self, window: &mut Window) {
        for image in self.stale_pictures.drain(..) {
            window.drop_image(image).ok();
        }
        let now = Instant::now();
        let id = self.browser.window();
        let Some(picture) = &mut self.picture else {
            return;
        };
        if picture.image.is_some() && !std::mem::replace(&mut picture.drawn, true) {
            crate::log_handover("picture drawn", &format!("window {id}, tab {}", picture.tab));
        }
        let Some(motion) = picture.motion else {
            return;
        };
        if !motion.done(now) {
            window.request_animation_frame();
        } else {
            picture.motion = None;
            // check.sh waits for this line, which says the picture has stopped moving.
            log!("browser: window {id} picture rests");
        }
    }

    /// The picture over the card, which takes a press.
    pub(super) fn picture_element(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let picture = self.picture.as_ref()?;
        let ready = picture.image.as_ref()?;
        let now = Instant::now();
        let bounds = self.picture_bounds(now)?;
        let scale = picture.scale(now);
        let veil = VEIL * (1. - (scale - REST) / (1. - REST)).clamp(0., 1.);
        let image = ready.image.clone();
        let radius = px(CARD_RADIUS);
        let picture = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let corners = Corners::all(radius);
                window.paint_image(bounds, bounds, corners, image.clone(), 0, false).ok();
            },
        )
        .absolute()
        .size_full();
        Some(
            div()
                .id("page-picture")
                .group("page-picture")
                .absolute()
                .left(bounds.left())
                .top(bounds.top())
                .w(bounds.size.width)
                .h(bounds.size.height)
                .rounded(radius)
                .shadow(Level::Resting.shadow())
                .cursor_pointer()
                // A press here is the picture's, not the window background's, which moves the
                // window.
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|shell, _, _, cx| {
                        cx.stop_propagation();
                        shell.press_picture(cx);
                    }),
                )
                .child(picture)
                // The veil goes in one frame under the pointer, as header buttons light.
                .child(
                    div()
                        .absolute()
                        .size_full()
                        .rounded(radius)
                        .bg(hsla(0., 0., 0., veil))
                        .group_hover("page-picture", |veil| veil.bg(gpui::transparent_black())),
                )
                .child(
                    div()
                        .absolute()
                        .size_full()
                        .rounded(radius)
                        .border(px(RIM))
                        .border_color(Level::Resting.rim()),
                )
                .into_any_element(),
        )
    }
}

/// The largest box of the picture's shape that fits `scale` of `card`, centred in it.
fn fit(card: Bounds<Pixels>, picture: (f32, f32), scale: f32) -> Bounds<Pixels> {
    let (card_width, card_height) = (f32::from(card.size.width), f32::from(card.size.height));
    let (room_width, room_height) = (card_width * scale, card_height * scale);
    let zoom = (room_width / picture.0).min(room_height / picture.1);
    let (width, height) = (picture.0 * zoom, picture.1 * zoom);
    Bounds::new(
        point(
            card.left() + px((card_width - width) / 2.),
            card.top() + px((card_height - height) / 2.),
        ),
        size(px(width), px(height)),
    )
}

#[cfg(test)]
mod tests;
