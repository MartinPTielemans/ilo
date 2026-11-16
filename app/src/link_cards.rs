//! Link cards (#3080): ⇧ alone on a link, while its plate shows, asks the core for what the link's
//! page says about itself (bc_tab_link_card), and the plate grows into a card of it. The core's
//! fetch carries no cookie, referrer, certificate or sign-in; the app reads the page and decodes
//! its picture off the main thread, and asks the picture only if the ad blocker lets it through.
//! Nothing of it is logged but the card's states, and agents get none of it.

use super::*;
use browser::link_card::blocked;

/// Where Settings' switch is kept off. On by default, so only its absence is kept.
fn off_path() -> String {
    format!("{}/link-cards-off", profile_dir())
}

/// Whether ⇧ on a link asks for its card: Settings › Links' switch.
pub(crate) fn on() -> bool {
    !std::path::Path::new(&off_path()).exists()
}

/// Settings' switch. Turned off, a card that shows goes.
pub(crate) fn set_on(on: bool, cx: &mut App) {
    let path = off_path();
    _ = if on {
        std::fs::remove_file(path)
    } else {
        std::fs::write(path, "")
    };
    if !on {
        link_target::close_card(cx);
    }
}

thread_local! {
    /// The profile each card was asked in, for the sites ad blocking is off on there.
    static PROFILES: RefCell<HashMap<u64, bc::bc_id>> = RefCell::default();
}

/// ⇧ on the link the pointer is on in `tab`'s page: the card's id, or 0 if the core asked
/// nothing.
pub(crate) fn ask(tab: u64) -> u64 {
    let card = core::tab_link_card(tab);
    if card != 0 {
        let profile = unsafe { bc::bc_tab_profile(tab) };
        // A new card cancels the one before, so only the newest needs its profile.
        PROFILES.with_borrow_mut(|profiles| {
            profiles.clear();
            profiles.insert(card, profile);
        });
    }
    card
}

/// Asks `picture`, the picture `card`'s page at `page` named, if the ad blocker lets it through
/// as an image of that page. Whether it was asked: a blocked picture is no picture.
pub(crate) fn picture(card: u64, picture: &str, page: &str) -> bool {
    let Some(profile) = PROFILES.with_borrow(|profiles| profiles.get(&card).copied()) else {
        return false;
    };
    let picture = match ad_block_host::blocking_engine() {
        // The extension blocks, and never sees the core's own requests: the picture is asked as
        // it is, carrying no cookie, referrer or sign-in.
        None => Some(picture.to_string()),
        // Still loading the first engine, which the main thread doesn't wait for: no picture
        // rather than one unchecked.
        Some(None) => None,
        Some(Some(engine)) => {
            let off_sites = crate::name(bc::bc_ad_blocking_off_sites, profile);
            blocked::picture_to_ask(Some(&engine), &off_sites, picture, page)
        }
    };
    let Some(picture) = picture else {
        return false;
    };
    let Ok(picture) = CString::new(picture) else {
        return false;
    };
    core::link_card_picture(card, &picture);
    true
}

/// The card is gone: the core stops its wait or fetch.
pub(crate) fn cancel(card: u64) {
    PROFILES.with_borrow_mut(|profiles| profiles.remove(&card));
    core::link_card_cancel(card);
}

/// What `card`'s fetch came to (bc_host's link_card_fetched), copied for the card, which reads it
/// off the main thread.
#[allow(dead_code)] // Until bc_host has it, with #3085's header.
pub(crate) unsafe extern "C" fn link_card_fetched(
    _: *mut c_void,
    card: u64,
    status: c_int,
    url: *const c_char,
    mime: *const c_char,
    bytes: *const u8,
    size: usize,
) {
    let text = |text: *const c_char| {
        (!text.is_null())
            .then(|| {
                unsafe { CStr::from_ptr(text) }
                    .to_string_lossy()
                    .into_owned()
            })
            .unwrap_or_default()
    };
    let (url, mime) = (text(url), text(mime));
    let bytes = if bytes.is_null() || size == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(bytes, size) }.to_vec()
    };
    update_app_later(move |cx| link_target::card_fetched(card, status, url, mime, bytes, cx));
}

/// The core's calls, which come with #3085's header and the engines built from it: until then the
/// core asks nothing, so ⇧ shows no card.
mod core {
    use std::ffi::CStr;

    pub(super) fn tab_link_card(_tab: u64) -> u64 {
        0
    }

    pub(super) fn link_card_picture(_card: u64, _url: &CStr) {}

    pub(super) fn link_card_cancel(_card: u64) {}
}
