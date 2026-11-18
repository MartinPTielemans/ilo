//! Whether the ad blocker lets a link card ask for its picture. The page's own request isn't
//! checked: the user asked for that page, as a click would load it. Its picture is checked as a
//! third-party image of the link's page would be, before bc_link_card_picture, and a blocked picture
//! is no picture.

use crate::ad_block::{self, Engine, Verdict};

/// What to ask for `picture`, the picture the page at `page` named, or None if the ad blocker
/// blocks it there. `engine` is the browser's own blocker while it blocks (None with it off, or
/// while uBlock Origin is the blocker, which never sees the core's own requests), and `off_sites`
/// the profile's sites it's off on (bc_ad_blocking_off_sites), one a line, each a registrable
/// domain, which covers its subdomains, or a host alone.
pub fn picture_to_ask(
    engine: Option<&Engine>,
    off_sites: &str,
    picture: &str,
    page: &str,
) -> Option<String> {
    let Some(engine) = engine else {
        return Some(picture.to_string());
    };
    if off_at(off_sites, page) {
        return Some(picture.to_string());
    }
    match ad_block::check(engine, picture, page, "image") {
        // A stand-in is the filter's way of keeping the page working, which a card doesn't need.
        Verdict::Block | Verdict::Redirect(_) => None,
        Verdict::Rewrite(url) => Some(url),
        Verdict::Allow | Verdict::AtResponse(_) => Some(picture.to_string()),
    }
}

/// Whether `page`'s host is one of `off_sites` or under one.
fn off_at(off_sites: &str, page: &str) -> bool {
    let Some(host) = url::Url::parse(page)
        .ok()
        .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
    else {
        return false;
    };
    off_sites
        .lines()
        .map(|site| site.trim().to_ascii_lowercase())
        .filter(|site| !site.is_empty())
        .any(|site| host == site || host.ends_with(&format!(".{site}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ad_block::{Setup, compile};

    fn engine() -> Engine {
        compile(&Setup {
            folder: std::env::temp_dir().join("ilo-link-card-blocked-none"),
            lists: Vec::new(),
            filters: "||ads.example^\n||img.example/*$image,removeparam=utm_source\n".into(),
        })
    }

    const PAGE: &str = "https://blog.example.com/post";

    #[test]
    fn a_blocked_picture_is_no_picture() {
        let engine = engine();
        let ask = |picture| picture_to_ask(Some(&engine), "", picture, PAGE);
        assert_eq!(ask("https://ads.example/share.png"), None);
        assert_eq!(
            ask("https://cdn.example.net/share.png").as_deref(),
            Some("https://cdn.example.net/share.png")
        );
    }

    #[test]
    fn a_rewritten_picture_is_asked_as_rewritten() {
        let engine = engine();
        assert_eq!(
            picture_to_ask(
                Some(&engine),
                "",
                "https://img.example/a.png?utm_source=x",
                PAGE
            )
            .as_deref(),
            Some("https://img.example/a.png")
        );
    }

    #[test]
    fn the_sites_it_is_off_on_and_no_blocker_ask_it_as_it_is() {
        let engine = engine();
        let ad = "https://ads.example/share.png";
        assert_eq!(
            picture_to_ask(Some(&engine), "other.org\nexample.com\n", ad, PAGE).as_deref(),
            Some(ad)
        );
        assert_eq!(
            picture_to_ask(Some(&engine), "notexample.com", ad, PAGE),
            None
        );
        assert_eq!(picture_to_ask(None, "", ad, PAGE).as_deref(), Some(ad));
    }
}
