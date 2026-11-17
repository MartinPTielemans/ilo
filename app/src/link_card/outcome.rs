//! What a link card shows for what the core's fetch of its page came to (link_card_fetched): the
//! page's own words, the reason it couldn't, or nothing at all when the user moved on. Pure, so the
//! page is read off the main thread and its tests run without the engine.

use url::Url;

use super::page::{self, Page};

/// browser_core.h's BC_LINK_CARD_ refusals, each below every net error.
pub const PRIVATE: i32 = -20001;
pub const PROXY: i32 = -20002;
pub const REDIRECT: i32 = -20003;
pub const TOO_BIG: i32 = -20004;
/// Chromium's net errors a card tells apart.
const ABORTED: i32 = -3;
const TIMED_OUT: i32 = -7;
const NAME_NOT_RESOLVED: i32 = -105;
const CONNECTION_REFUSED: i32 = -102;
const INTERNET_DISCONNECTED: i32 = -106;
const CERT_FIRST: i32 = -200;
const CERT_LAST: i32 = -299;

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// What the page says; `lands` is the site the redirects ended on, when it isn't the link's.
    Page { page: Page, lands: Option<String> },
    /// "Couldn't load this page", and why.
    Failed(String),
    /// The user moved on before the wait was over, or the card was cancelled: it goes quietly.
    Gone,
}

/// The card's page from link_card_fetched's |status|, final |url|, |mime| and |bytes|, for a link
/// whose registrable domain, as the link target showed it, is |site|.
pub fn of_page(status: i32, url: &str, mime: &str, bytes: &[u8], site: &str) -> Outcome {
    if status == ABORTED {
        return Outcome::Gone;
    }
    if let Some(reason) = failure(status, mime) {
        return Outcome::Failed(reason);
    }
    let page = page::read(bytes, mime, url);
    if page.title.is_none() && page.description.is_none() {
        return Outcome::Failed("The page doesn't say what it is.".into());
    }
    Outcome::Page {
        page,
        lands: lands(url, site),
    }
}

/// Why a fetch with |status| and |mime| gave no page, or None if it did.
fn failure(status: i32, mime: &str) -> Option<String> {
    Some(match status {
        200..=299 => {
            let kind = mime
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            return match kind.as_str() {
                "text/html" | "application/xhtml+xml" => None,
                "application/pdf" => Some("It's a PDF, not a page.".into()),
                "" => Some("It isn't a page.".into()),
                _ => Some(format!("It's a file ({kind}), not a page.")),
            };
        }
        100..=599 => format!("The site answered {status}{}.", reason(status)),
        PRIVATE => "It's on a private network, which link cards never ask.".into(),
        PROXY => "Link cards don't ask through a proxy.".into(),
        REDIRECT => "It redirected too many times, or away from the web.".into(),
        TOO_BIG => "Its picture is too big.".into(),
        TIMED_OUT => "The site took more than 5 seconds to answer.".into(),
        NAME_NOT_RESOLVED => "The site's address wasn't found.".into(),
        CONNECTION_REFUSED => "The site refused the connection.".into(),
        INTERNET_DISCONNECTED => "You're offline.".into(),
        CERT_LAST..=CERT_FIRST => "The site's certificate isn't valid.".into(),
        _ => "The site couldn't be reached.".into(),
    })
}

fn reason(status: i32) -> &'static str {
    match status {
        400 => " Bad Request",
        401 => " Unauthorized",
        403 => " Forbidden",
        404 => " Not Found",
        410 => " Gone",
        429 => " Too Many Requests",
        500 => " Internal Server Error",
        502 => " Bad Gateway",
        503 => " Service Unavailable",
        504 => " Gateway Timeout",
        _ => "",
    }
}

/// The host the page ended on, without "www.", when it isn't |site| or a host under it.
fn lands(url: &str, site: &str) -> Option<String> {
    let url = Url::parse(url).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let site = site.trim_start_matches("www.").to_ascii_lowercase();
    if site.is_empty() || host == site || host.ends_with(&format!(".{site}")) {
        return None;
    }
    Some(host.trim_start_matches("www.").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &[u8] = b"<title>Me at the zoo</title><meta property='og:image' content='/i.jpg'>";

    #[test]
    fn a_page_says_what_it_is() {
        let Outcome::Page { page, lands } = of_page(
            200,
            "https://example.com/a",
            "text/html",
            HEAD,
            "example.com",
        ) else {
            panic!("no page");
        };
        assert_eq!(page.title.as_deref(), Some("Me at the zoo"));
        assert_eq!(page.picture.as_deref(), Some("https://example.com/i.jpg"));
        assert_eq!(lands, None);
    }

    #[test]
    fn a_redirect_to_another_site_shows_where_it_lands() {
        let lands = |url: &str, site: &str| match of_page(200, url, "text/html", HEAD, site) {
            Outcome::Page { lands, .. } => lands,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            lands("https://www.youtube.com/watch?v=jNQXAC9IVRw", "youtu.be").as_deref(),
            Some("youtube.com")
        );
        assert_eq!(lands("https://www.example.com/", "example.com"), None);
        assert_eq!(lands("https://blog.example.com/", "example.com"), None);
        assert_eq!(
            lands("https://notexample.com/", "example.com").as_deref(),
            Some("notexample.com")
        );
    }

    #[test]
    fn each_failure_says_why() {
        let failed = |status, mime| match of_page(
            status,
            "https://example.com/",
            mime,
            b"",
            "example.com",
        ) {
            Outcome::Failed(reason) => reason,
            other => panic!("{other:?}"),
        };
        assert_eq!(failed(403, ""), "The site answered 403 Forbidden.");
        assert_eq!(failed(418, ""), "The site answered 418.");
        assert_eq!(failed(200, "application/pdf"), "It's a PDF, not a page.");
        assert_eq!(
            failed(200, "image/png"),
            "It's a file (image/png), not a page."
        );
        assert_eq!(
            failed(PRIVATE, ""),
            "It's on a private network, which link cards never ask."
        );
        assert_eq!(failed(PROXY, ""), "Link cards don't ask through a proxy.");
        assert_eq!(
            failed(REDIRECT, ""),
            "It redirected too many times, or away from the web."
        );
        assert_eq!(
            failed(TIMED_OUT, ""),
            "The site took more than 5 seconds to answer."
        );
        assert_eq!(failed(-202, ""), "The site's certificate isn't valid.");
        assert_eq!(failed(-999, ""), "The site couldn't be reached.");
        assert_eq!(
            failed(200, "text/html; charset=utf-8"),
            "The page doesn't say what it is."
        );
    }

    #[test]
    fn moving_on_is_no_failure() {
        assert_eq!(of_page(ABORTED, "", "", b"", "example.com"), Outcome::Gone);
    }
}
