//! The link card (#3080): ⇧ alone on a link grows its plate into a card of what the page behind it
//! says about itself, in the plate's own window, whose last row stays the plate's address. The
//! core fetches the page and its picture as no user would (bc_tab_link_card); they're read and
//! decoded off the main thread (`crate::link_card`), and the card closes with its plate.

use std::{sync::Arc, time::Duration};

use gpui::{
    AnyElement, Context, FontWeight, Hsla, ImageSource, ObjectFit, RenderImage, SharedString, Task,
    div, img, prelude::*, px, svg,
};
use image::{Frame, RgbaImage};

use super::{LinkTarget, Plate, Target};
use crate::elevation::Level;
use crate::link_card::{outcome, outcome::Outcome, picture};
use crate::palette::Palette;
use crate::tokens::hover_card::{
    GROUND, PADDING, PICTURE_RADIUS, RADIUS, RIM, TEXT_TOP, TEXT_X, TITLE, TITLE_LINE,
};
use crate::tokens::link_card::{
    ALERT_GAP, BAR_DESC, BAR_DESC_SHARE, BAR_GAP, BAR_TITLE, BAR_TITLE_SHARE, DESC, DESC_GAP,
    DESC_LINE, DESC_LINES, LANDS_GAP, PICTURE_RATIO, ROW_BOTTOM, ROW_GAP, TITLE_LINES, WIDTH,
};
use crate::tokens::link_target::{HEIGHT, PRIMARY, SECONDARY, TEXT_SIZE};
use crate::tokens::text::{BODY, SMALL};

/// The picture's box, inside the card's padding.
pub const INNER: f32 = WIDTH - 2. * PADDING;
pub const PICTURE_HEIGHT: f32 = INNER * PICTURE_RATIO;
/// A picture not decoded in this long isn't waited for: the card stays without one.
pub const PICTURE_WAIT: Duration = Duration::from_secs(1);

/// What the card shows.
#[derive(Clone)]
pub enum State {
    /// Asked, with no answer yet: two bars in the title's place.
    Loading,
    Page {
        title: Option<SharedString>,
        description: Option<SharedString>,
        /// The site the redirects ended on, when it isn't the link's.
        lands: Option<SharedString>,
        picture: Picture,
    },
    /// "Couldn't load this page", and why.
    Failed(SharedString),
}

#[derive(Clone)]
pub enum Picture {
    None,
    /// Asked for; the card draws without it until it comes.
    Waiting,
    Shown(Arc<RenderImage>, (u32, u32)),
}

/// The card asked for the link the plate shows.
pub(super) struct Card {
    pub id: u64,
    /// The link it's for, whose ⇧ again, or auto-repeat, asks nothing more.
    pub link: Target,
    pub state: State,
    /// Whether it shows: after the plate's own delay, as the core waits that long before it asks,
    /// or with its page's answer.
    pub shown: bool,
    /// Its wait, its page being read or its picture being decoded. Dropping it cancels it.
    pub task: Option<Task<()>>,
}

impl State {
    /// For check.sh's line, which names the state alone, never the link or what the page said.
    pub fn name(&self) -> &'static str {
        match self {
            State::Loading => "loading",
            State::Page {
                picture: Picture::Shown(..),
                ..
            } => "page with picture",
            State::Page {
                picture: Picture::Waiting,
                ..
            } => "page, picture waiting",
            State::Page { .. } => "page",
            State::Failed(_) => "failed",
        }
    }
}

/// The card's state for the page's answer, as `outcome` read it: None if it goes quietly.
pub fn page_state(outcome: Outcome, picture_asked: bool) -> Option<State> {
    match outcome {
        Outcome::Gone => None,
        Outcome::Failed(reason) => Some(State::Failed(reason.into())),
        Outcome::Page { page, lands } => Some(State::Page {
            title: page.title.map(Into::into),
            description: page.description.map(Into::into),
            lands: lands.map(Into::into),
            picture: match picture_asked {
                true => Picture::Waiting,
                false => Picture::None,
            },
        }),
    }
}

/// The picture GPUI draws from `bytes`, BGRA as its atlas takes it, or None.
pub fn render_picture(bytes: &[u8]) -> Option<(Arc<RenderImage>, (u32, u32))> {
    let mut picture: RgbaImage = picture::decode(bytes)?;
    for pixel in picture.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let size = picture.dimensions();
    Some((Arc::new(RenderImage::new(vec![Frame::new(picture)])), size))
}

impl LinkTarget {
    /// The page's answer for `id`, read off the main thread.
    pub(super) fn card_page(
        &mut self,
        id: u64,
        status: i32,
        url: String,
        mime: String,
        bytes: Vec<u8>,
        cx: &mut Context<Self>,
    ) {
        let Some(card) = self.card.as_mut().filter(|card| card.id == id) else {
            return;
        };
        let site = card.link.text[card.link.domain.clone()].to_string();
        card.task = Some(cx.spawn(async move |target, cx| {
            let read_url = url.clone();
            let outcome = cx
                .background_spawn(async move {
                    outcome::of_page(status, &read_url, &mime, &bytes, &site)
                })
                .await;
            target
                .update(cx, |target, cx| target.card_read(id, outcome, &url, cx))
                .ok();
        }));
    }

    fn card_read(&mut self, id: u64, outcome: Outcome, url: &str, cx: &mut Context<Self>) {
        if !self.card.as_ref().is_some_and(|card| card.id == id) {
            return;
        }
        let mut asked = false;
        if let Outcome::Page { page, .. } = &outcome {
            // Asked only if the ad blocker lets it through as an image of the page it ended at.
            asked = (page.picture.as_ref())
                .is_some_and(|picture| self.host.card_picture(id, picture, url));
            // A page told keeps the core's card for its picture; with none to ask, it goes. Any
            // other answer ended it.
            if !asked {
                self.host.card_cancel(id);
            }
        }
        let Some(state) = page_state(outcome, asked) else {
            self.card = None;
            // check.sh reads this line.
            log!("browser: link card gone");
            cx.notify();
            return;
        };
        let card = self.card.as_mut().expect("checked above");
        card.state = state;
        card.shown = true;
        card.task = None;
        // check.sh reads this line.
        log!("browser: link card {}", card.state.name());
        cx.notify();
    }

    /// The picture's answer for `id`, decoded off the main thread within PICTURE_WAIT.
    pub(super) fn card_picture(
        &mut self,
        id: u64,
        status: i32,
        bytes: Vec<u8>,
        cx: &mut Context<Self>,
    ) {
        let Some(card) = self.card.as_mut().filter(|card| card.id == id) else {
            return;
        };
        if !(200..300).contains(&status) || bytes.is_empty() {
            return self.card_pictured(id, None, cx);
        }
        card.task = Some(cx.spawn(async move |target, cx| {
            let executor = cx.background_executor().clone();
            let decoded = executor.spawn(async move { render_picture(&bytes) });
            let timer = executor.timer(PICTURE_WAIT);
            let picture = futures::select_biased! {
                picture = futures::FutureExt::fuse(decoded) => picture,
                _ = futures::FutureExt::fuse(timer) => None,
            };
            target
                .update(cx, |target, cx| target.card_pictured(id, picture, cx))
                .ok();
        }));
    }

    fn card_pictured(
        &mut self,
        id: u64,
        picture: Option<(Arc<RenderImage>, (u32, u32))>,
        cx: &mut Context<Self>,
    ) {
        let Some(card) = self.card.as_mut().filter(|card| card.id == id) else {
            return;
        };
        card.task = None;
        if let State::Page { picture: slot, .. } = &mut card.state {
            *slot = match picture {
                Some((image, size)) => Picture::Shown(image, size),
                None => Picture::None,
            };
        }
        // check.sh reads this line.
        log!("browser: link card {}", card.state.name());
        cx.notify();
    }
}

/// The tallest card: what the window's room above the plate has to fit.
pub fn tallest() -> f32 {
    PADDING
        + PICTURE_HEIGHT
        + TEXT_TOP
        + TITLE_LINES as f32 * TITLE_LINE
        + DESC_GAP
        + DESC_LINES as f32 * DESC_LINE
        + ROW_GAP
        + HEIGHT
        + ROW_BOTTOM
}

/// The address row: the plate's address, and → the site it landed on.
fn address(plate: &Plate, lands: Option<&SharedString>, palette: &Palette) -> AnyElement {
    div()
        .h(px(HEIGHT))
        .flex()
        .items_center()
        .whitespace_nowrap()
        .overflow_hidden()
        .text_ellipsis()
        .text_size(px(TEXT_SIZE))
        .text_color(palette.text(SECONDARY))
        .child(plate.before.clone())
        .child(
            div()
                .text_color(palette.text(PRIMARY))
                .child(plate.site.clone()),
        )
        .child(plate.after.clone())
        .when_some(lands.cloned(), |row, lands| {
            row.child(div().px(px(LANDS_GAP)).child("\u{2192}"))
                .child(div().text_color(palette.text(PRIMARY)).child(lands))
        })
        .into_any_element()
}

fn bar(width: f32, height: f32, palette: &Palette) -> AnyElement {
    div()
        .w(px(width))
        .h(px(height))
        .rounded(px(height / 2.))
        .bg(palette.plate(GROUND * 2.))
        .into_any_element()
}

/// The card, `plate` its last row, on `fill` with `palette`'s ink, at `scale` pixels a point.
pub fn render(
    state: &State,
    plate: &Plate,
    fill: Hsla,
    palette: &Palette,
    scale: f32,
) -> AnyElement {
    let text_width = WIDTH - 2. * TEXT_X;
    let mut body = div().flex().flex_col().w(px(WIDTH));
    if let State::Page {
        picture: Picture::Shown(image, (width, height)),
        ..
    } = state
    {
        // A picture smaller than the box at the screen's scale sits whole on the well; a larger
        // one covers it.
        let small = (*width as f32) < INNER * scale && (*height as f32) < PICTURE_HEIGHT * scale;
        let picture = img(ImageSource::Render(image.clone())).rounded(px(PICTURE_RADIUS));
        let picture = match small {
            true => picture
                .w(px(*width as f32 / scale))
                .h(px(*height as f32 / scale)),
            false => picture.size_full().object_fit(ObjectFit::Cover),
        };
        body = body.child(
            div().pt(px(PADDING)).px(px(PADDING)).child(
                div()
                    .w(px(INNER))
                    .h(px(PICTURE_HEIGHT))
                    .rounded(px(PICTURE_RADIUS))
                    .overflow_hidden()
                    .bg(palette.plate(GROUND))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(picture),
            ),
        );
    }
    let mut text = div()
        .flex()
        .flex_col()
        .pt(px(TEXT_TOP))
        .px(px(TEXT_X))
        .w(px(WIDTH));
    let title = |words: SharedString| {
        div()
            .w(px(text_width))
            .line_clamp(TITLE_LINES as usize)
            .text_ellipsis()
            .overflow_hidden()
            .text_size(px(BODY))
            .line_height(px(TITLE_LINE))
            .font_weight(FontWeight::MEDIUM)
            .text_color(palette.on_overlay(TITLE))
            .child(words)
    };
    let description = |words: SharedString| {
        div()
            .mt(px(DESC_GAP))
            .w(px(text_width))
            .line_clamp(DESC_LINES as usize)
            .text_ellipsis()
            .overflow_hidden()
            .text_size(px(SMALL))
            .line_height(px(DESC_LINE))
            .text_color(palette.on_overlay(DESC))
            .child(words)
    };
    let lands = match state {
        State::Loading => {
            text = text
                .gap(px(BAR_GAP))
                .child(bar(text_width * BAR_TITLE_SHARE, BAR_TITLE, palette))
                .child(bar(text_width * BAR_DESC_SHARE, BAR_DESC, palette));
            None
        }
        State::Failed(reason) => {
            text = text
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(ALERT_GAP))
                        .child(
                            svg()
                                .path("icons/circle-alert.svg")
                                .size(px(BODY))
                                .flex_none()
                                .text_color(palette.on_overlay(DESC)),
                        )
                        .child(title("Couldn't load this page".into()).w_auto()),
                )
                .child(description(reason.clone()));
            None
        }
        State::Page {
            title: words,
            description: more,
            lands,
            ..
        } => {
            if let Some(words) = words {
                text = text.child(title(words.clone()));
            }
            if let Some(more) = more {
                text = text.child(description(more.clone()));
            }
            lands.as_ref()
        }
    };
    let row = div()
        .mt(px(ROW_GAP))
        .px(px(TEXT_X))
        .pb(px(ROW_BOTTOM))
        .child(address(plate, lands, palette));
    body = body.child(text).child(row);
    div()
        .relative()
        .w(px(WIDTH))
        .rounded(px(RADIUS))
        .shadow(Level::Popover.shadow())
        .bg(fill)
        .child(body)
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .rounded(px(RADIUS))
                .border(px(RIM))
                .border_color(Level::Popover.rim_in(palette)),
        )
        .into_any_element()
}
