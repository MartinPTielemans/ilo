//! Shared links (#680), between the core and the app: Share's dialogs and what it came to, the
//! account's list for Settings › Sync, Copy Link, an unshare, and the offer over a share's page.
//! Nothing of a share goes in a log line, in any build: not its link, key or id, nor a title or an
//! address in it (bc_share). The user's alone: agents get none of it.

use super::*;
use browser::account::{ShareKind, SharedLink};
use browser::share::{self, Problem, Step};
use gpui::ClipboardItem;

/// How long the copied toast and a failed share's stay, as a capture's copied toast does.
const TOAST_TIME: Duration = Duration::from_secs(5);

thread_local! {
    // Each of the account's links by its id, as the core last listed or made them: what Copy Link
    // copies.
    static LINKS: RefCell<HashMap<String, String>> = RefCell::default();
    // The title each share asked for goes under, by its request, for its toast.
    static ASKED: RefCell<HashMap<u64, SharedString>> = RefCell::default();
    // The account has a link, as the core listed or made one since it signed in, so Share copies
    // at once.
    static SHARED_BEFORE: Cell<bool> = const { Cell::new(false) };
    // The list was asked for since the account signed in.
    static LISTED: Cell<bool> = const { Cell::new(false) };
}

fn kind_of(kind: i32) -> ShareKind {
    match kind as u32 {
        bc::BC_SHARE_SPACE => ShareKind::Space,
        bc::BC_SHARE_TABS => ShareKind::Tabs,
        _ => ShareKind::Folder,
    }
}

fn core_kind(kind: ShareKind) -> i32 {
    (match kind {
        ShareKind::Folder => bc::BC_SHARE_FOLDER,
        ShareKind::Space => bc::BC_SHARE_SPACE,
        ShareKind::Tabs => bc::BC_SHARE_TABS,
    }) as i32
}

/// The account says where it is: once it syncs, the core lists its links, so the first share
/// knows whether to explain; signed out, nothing of the last account's is kept.
pub(crate) fn account_changed(status: account::Status) {
    match status {
        account::Status::On if !LISTED.replace(true) => unsafe { bc::bc_share_list() },
        account::Status::SignedOut => {
            LISTED.set(false);
            SHARED_BEFORE.set(false);
            LINKS.with_borrow_mut(HashMap::clear);
        }
        _ => {}
    }
}

/// Shares `items` as a link of `kind` under `title`, from the browser window `window`: signed
/// out, it points to Settings › Sync; the first time, it says who can see what before it goes;
/// after that it goes at once.
pub(crate) fn share(
    window: bc::bc_id,
    kind: ShareKind,
    items: Vec<u64>,
    title: SharedString,
    cx: &mut App,
) {
    let (mut tabs, mut folders, mut left_out) = (0, 0, 0);
    let kind = core_kind(kind);
    let counted = unsafe {
        bc::bc_share_count(
            kind,
            items.as_ptr(),
            items.len(),
            &mut tabs,
            &mut folders,
            &mut left_out,
        )
    };
    if !counted {
        // check.sh reads these lines.
        log!("browser: share refused");
        return;
    }
    let signed_in = account::synced(cx).status == account::Status::On;
    let step = share::step(signed_in, SHARED_BEFORE.get());
    log!("browser: share {step:?}");
    let ask = move |(message, detail, action): (String, String, &'static str),
                    confirmed: Box<dyn FnOnce(&mut App)>,
                    cx: &mut App| {
        with_overlays(
            move |cx| dialog::ask(Some(window), message, detail, action, confirmed, cx),
            cx,
        )
    };
    match step {
        Step::SignIn => ask(share::sign_in(), Box::new(settings::show_sync), cx),
        Step::Explain => {
            let words = share::explain(&title, tabs);
            ask(words, Box::new(move |_| make(kind, &items, title)), cx)
        }
        Step::Share => make(kind, &items, title),
    }
}

fn make(kind: i32, items: &[u64], title: SharedString) {
    let request = unsafe { bc::bc_share_make(kind, items.as_ptr(), items.len()) };
    match request {
        0 => log!("browser: share refused"),
        request => {
            ASKED.with_borrow_mut(|asked| asked.insert(request, title));
            log!("browser: share asked {request}");
        }
    }
}

/// Puts the account's link `id` on the clipboard, for Settings › Sync's Copy Link.
pub(crate) fn copy(id: &str, cx: &mut App) {
    match LINKS.with_borrow(|links| links.get(id).cloned()) {
        Some(link) => {
            cx.write_to_clipboard(ClipboardItem::new_string(link));
            // check.sh reads this line.
            log!("browser: share link copied");
        }
        None => log!("browser: share link not listed"),
    }
}

/// The Space a share the tab `tab` opened goes in: the one its window shows, else the one the
/// most recently active window shows, as a Little ilo window shows none.
pub(crate) fn space_for(tab: u64) -> u64 {
    let shown = |window| unsafe { bc::bc_window_space(window) };
    match shown(unsafe { bc::bc_tab_window(tab) }) {
        0 => ids(bc::bc_windows)
            .into_iter()
            .map(shown)
            .find(|&space| space != 0)
            .unwrap_or(0),
        space => space,
    }
}

/// Adds the link `tab` opened, or doesn't, as the user answered its offer.
pub(crate) fn answer(tab: u64, add: bool) {
    let added = unsafe { bc::bc_tab_answer_share(tab, space_for(tab), add) };
    // check.sh reads this line.
    log!("browser: share offer {tab} add {add} added {}", added != 0);
}

pub(crate) unsafe extern "C" fn share_made(
    _: *mut c_void,
    request: u64,
    made: *const bc::bc_share,
    left_out: usize,
    replaced: bool,
    problem: i32,
) {
    let title = ASKED
        .with_borrow_mut(|asked| asked.remove(&request))
        .unwrap_or_default();
    let made = unsafe { made.as_ref() }.map(|made| {
        let (id, link) = (core_text(made.id), core_text(made.link));
        LINKS.with_borrow_mut(|links| links.insert(id.into(), link.to_string()));
        (link, made.tabs)
    });
    let (words, logged, link) = match (Problem::from_core(problem), made) {
        (None, Some((link, tabs))) => {
            SHARED_BEFORE.set(true);
            let words = share::copied(&title, replaced, tabs, left_out);
            let logged = format!("share copied replaced {replaced} left out {left_out}");
            (words, logged, Some(link))
        }
        (problem, _) => {
            let problem = problem.unwrap_or(Problem::Server);
            let words = share::not_shared(&title, problem);
            (words, format!("share not made {problem:?}"), None)
        }
    };
    // check.sh reads this line.
    log!("browser: share made {request}: {logged}");
    update_app_later(move |cx| {
        if let Some(link) = link {
            cx.write_to_clipboard(ClipboardItem::new_string(link.to_string()));
        }
        let show =
            move |cx: &mut App| toast::show_logged_as(&words, &logged, Vec::new(), TOAST_TIME, cx);
        with_overlays(show, cx);
    });
}

pub(crate) unsafe extern "C" fn shares_listed(
    _: *mut c_void,
    shares: *const bc::bc_share,
    count: usize,
    problem: i32,
) {
    if let Some(problem) = Problem::from_core(problem) {
        log!("browser: shares not listed {problem:?}");
        return;
    }
    let shares = match count {
        0 => &[][..],
        count => unsafe { std::slice::from_raw_parts(shares, count) },
    };
    let mut links = HashMap::new();
    let listed: Vec<SharedLink> = (shares.iter())
        .map(|listed| {
            let id = core_text(listed.id);
            links.insert(id.to_string(), core_text(listed.link).to_string());
            SharedLink {
                id,
                kind: kind_of(listed.kind),
                title: core_text(listed.title),
                tabs: listed.tabs,
                shared: listed.shared_at,
                color: listed.has_color.then(|| gpui::rgb(listed.color)),
                icon: core_text(listed.icon),
            }
        })
        .collect();
    LINKS.set(links);
    if !listed.is_empty() {
        SHARED_BEFORE.set(true);
    }
    // check.sh reads this line.
    log!("browser: shares listed {}", listed.len());
    update_app_later(move |cx| settings::shares_changed(listed, cx));
}

pub(crate) unsafe extern "C" fn unshared(_: *mut c_void, id: *const c_char, problem: i32) {
    let (id, problem) = (core_text(id), Problem::from_core(problem));
    // check.sh reads this line.
    log!("browser: unshared {problem:?}");
    update_app_later(move |cx| settings::unshared(id, problem, cx));
}

pub(crate) unsafe extern "C" fn tab_share_found(
    _: *mut c_void,
    tab: bc::bc_id,
    found: *const bc::bc_share_found,
) {
    let offer = unsafe { found.as_ref() }.map(|found| {
        let space = name(bc::bc_space_name, space_for(tab));
        let title = core_text(found.title);
        let (question, detail, action) =
            share::offer(kind_of(found.kind), &title, found.tabs, &space);
        offer::Offer {
            kind: offer::Kind::Share,
            title: question.into(),
            site: core_text(found.host),
            username: SharedString::default(),
            usernames: Vec::new(),
            password_length: 0,
            accept: action.into(),
            decline: "Not Now".into(),
            never: None,
            details: vec![detail.into()],
        }
    });
    update_app(|cx| offer::changed(tab, offer, cx));
}
