//! A sign-in's "you can close this tab" page, in a Little ilo window another app's link opened,
//! closes the window by itself (#2327), on a signal that doesn't read the page's words: a load onto
//! a loopback address carrying a `code`, from another site, as a CLI's or an MCP client's login
//! ends. The page's own window.close() would be the other signal, once the core lets it through for
//! such a tab (`bc_tab_set_page_may_close`, in the next core batch).

use super::*;

/// The pause between a sign-in's callback page loading and its window closing, long enough to see
/// it went through. Little ilo's own close motion follows, as ⌘W's.
const SHOWN_FOR: Duration = Duration::from_secs(2);

/// Where Settings' switch is kept off. On by default, so only its absence is kept.
fn off_path() -> String {
    format!("{}/sign-in-close-off", profile_dir())
}

/// Whether a sign-in's page in a Little ilo window another app opened closes it: Settings ›
/// Features' switch under Little ilo.
pub(crate) fn sign_in_close() -> bool {
    !std::path::Path::new(&off_path()).exists()
}

/// Settings' switch. Turned off, a window about to close stays.
pub(crate) fn set_sign_in_close(on: bool, cx: &mut App) {
    let path = off_path();
    _ = if on {
        std::fs::remove_file(path)
    } else {
        std::fs::write(path, "")
    };
    if !on {
        for window in
            LINK_TABS.with_borrow(|tabs| tabs.values().map(|l| l.window).collect::<Vec<_>>())
        {
            update_window_in(window, cx, |shell, cx| shell.set_little_closing(false, cx));
        }
    }
}

/// Each tab another app's link opened in a Little ilo window, with its window and the address it
/// loaded last, until it closes or leaves the window.
struct LinkTab {
    window: bc::bc_id,
    loaded: String,
}

thread_local! {
    static LINK_TABS: RefCell<HashMap<u64, LinkTab>> = RefCell::default();
}

/// `tab`, in Little ilo window `window`, opened for another app's link: the only kind a sign-in
/// closes. A tab the user opened, or moved into a Space, never is.
pub(crate) fn link_tab_opened(window: bc::bc_id, tab: u64) {
    LINK_TABS.with_borrow_mut(|tabs| {
        tabs.insert(
            tab,
            LinkTab {
                window,
                loaded: String::new(),
            },
        )
    });
}

/// `tab` left its Little ilo window for one of the user's Spaces (⌘O), where it's the user's and
/// nothing closes it by itself.
pub(crate) fn link_tab_left(tab: u64) {
    LINK_TABS.with_borrow_mut(|tabs| tabs.remove(&tab));
}

/// `tab` loaded `url`. A sign-in's callback closes its window after a pause, unless the tab has
/// loaded another page by then.
pub(crate) fn tab_loaded(tab: u64, url: &str, cx: &mut App) {
    let before = LINK_TABS.with_borrow_mut(|tabs| {
        tabs.get_mut(&tab).map(|link| {
            (
                link.window,
                std::mem::replace(&mut link.loaded, url.to_string()),
            )
        })
    });
    let Some((window, before)) = before else {
        return;
    };
    // The bar says the window's closing for as long as this page shows.
    let done = sign_in_close() && sign_in_done(&before, url);
    update_window_in(window, cx, |shell, cx| shell.set_little_closing(done, cx));
    if !done {
        return;
    }
    // check.sh reads this line.
    log!("browser: little window {window} sign-in done at {url}");
    let url = url.to_string();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(SHOWN_FOR).await;
        let still = LINK_TABS.with_borrow(|tabs| tabs.get(&tab).is_some_and(|l| l.loaded == url));
        let open = WINDOWS.with_borrow(|windows| windows.iter().any(|w| w.id == window));
        if still && open && sign_in_close() {
            // check.sh reads this line.
            log!("browser: little window {window} closed after its sign-in");
            // Outside the update, as ⌘W's: the window's close calls back into GPUI.
            unsafe { bc::bc_window_close(window) };
        }
    })
    .detach();
}

/// `tab` closed, with its window or by itself.
pub(crate) fn tab_closed(tab: u64) {
    LINK_TABS.with_borrow_mut(|tabs| tabs.remove(&tab));
}

/// Whether a load of `url` after `before` ends a sign-in: a redirect from another site onto a
/// loopback address whose query carries a `code` and no `error`. A failed sign-in stays, so it's
/// never hidden.
pub(crate) fn sign_in_done(before: &str, url: &str) -> bool {
    let (Some(from), Some(to)) = (host(before), host(url)) else {
        return false;
    };
    let keys = query_keys(url);
    loopback(to) && from != to && keys.contains(&"code") && !keys.contains(&"error")
}

/// The host of an http(s) URL, without its port, and lowercased by the core already.
fn host(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = match authority.strip_prefix('[') {
        Some(v6) => &authority[..v6.find(']')? + 2],
        None => authority.split(':').next()?,
    };
    (!host.is_empty()).then_some(host)
}

fn loopback(host: &str) -> bool {
    host == "localhost" || host == "[::1]" || host.starts_with("127.")
}

/// The names in `url`'s query.
fn query_keys(url: &str) -> Vec<&str> {
    let url = url.split('#').next().unwrap_or(url);
    let Some((_, query)) = url.split_once('?') else {
        return Vec::new();
    };
    query
        .split('&')
        .map(|pair| pair.split('=').next().unwrap_or(pair))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::sign_in_done;

    const PROVIDER: &str = "https://accounts.example.com/o/authorize?client_id=cli";

    #[test]
    fn a_redirect_onto_loopback_with_a_code_ends_a_sign_in() {
        for callback in [
            "http://localhost:54545/callback?code=abc&state=x",
            "http://127.0.0.1:8080/?state=x&code=abc",
            "http://[::1]:9000/oauth?code=abc#done",
        ] {
            assert!(sign_in_done(PROVIDER, callback), "{callback}");
        }
    }

    #[test]
    fn an_error_stays_open_even_with_a_code() {
        let failed = "http://localhost:54545/callback?error=access_denied";
        assert!(!sign_in_done(PROVIDER, failed));
        let both = "http://localhost:54545/callback?code=abc&error=x";
        assert!(!sign_in_done(PROVIDER, both));
    }

    #[test]
    fn only_loopback_callbacks_with_a_code_from_another_site_count() {
        // A site's own page with a code is no app's callback.
        let site = "https://example.com/callback?code=abc";
        assert!(!sign_in_done(PROVIDER, site));
        // No code, a code only in the fragment, or a name that merely starts with it.
        for url in [
            "http://localhost:54545/callback",
            "http://localhost:54545/callback#code=abc",
            "http://localhost:54545/callback?codes=abc",
        ] {
            assert!(!sign_in_done(PROVIDER, url), "{url}");
        }
        // The first page the link opened, or a load within the callback's own host.
        assert!(!sign_in_done("", "http://localhost:1/cb?code=abc"));
        assert!(!sign_in_done(
            "http://localhost:1/start",
            "http://localhost:1/cb?code=abc"
        ));
        // Another site that's loopback too, as a local identity server's, is still another site.
        assert!(sign_in_done(
            "http://localhost:1/sign-in",
            "http://127.0.0.1:1/cb?code=abc"
        ));
    }
}
