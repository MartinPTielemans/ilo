//! Links as they're copied: without the query parameters that only tell a site where a click came
//! from, as Dia copies them (docs/ux-spec.md › Copy URL). A name goes only where it's known to be a
//! tracker, and a name nobody listed always stays: the same name can be a tracker on one site and
//! the content on another (`t` is a share's ID on X and a video's time on YouTube), so a name is on
//! the every-site list only if it means the same everywhere.

/// Click IDs that ad and mail services add to links on every site, besides the utm_ parameters
/// (ours).
const TRACKERS: &[&str] = &[
    "_hsenc", "_hsmi", "dclid", "fbclid", "gbraid", "gclid", "igshid", "mc_cid", "mc_eid",
    "mkt_tok", "msclkid", "ttclid", "twclid", "wbraid", "yclid",
];

/// Names that are trackers on their sites alone, with the sites, each a host and its subdomains.
/// A row grows by a PR with its source and a test (#2351).
const SITE_TRACKERS: &[(&[&str], &[&str])] = &[
    // The share sheet's ID. ClearURLs' rules, provider "youtube".
    (&["youtube.com", "youtu.be"], &["si"]),
    // The share sheet's ID. ClearURLs' rules, provider "instagram".
    (&["instagram.com"], &["igsh"]),
    // The share's client and its ID. ClearURLs' rules, providers "twitter" and "x".
    (&["x.com", "twitter.com"], &["s", "t"]),
];

/// `url` without its tracking parameters, and otherwise as it was.
pub fn clean_link(url: &str) -> String {
    let (rest, fragment) = match url.split_once('#') {
        Some((rest, fragment)) => (rest, Some(fragment)),
        None => (url, None),
    };
    let Some((base, query)) = rest.split_once('?') else {
        return url.to_string();
    };
    let host = host(base);
    let on_site = |sites: &[&str]| {
        (sites.iter()).any(|site| {
            host == *site
                || host
                    .strip_suffix(site)
                    .is_some_and(|sub| sub.ends_with('.'))
        })
    };
    let site_trackers: Vec<&str> = (SITE_TRACKERS.iter())
        .filter(|(sites, _)| on_site(sites))
        .flat_map(|(_, names)| names.iter().copied())
        .collect();
    let kept: Vec<&str> = query
        .split('&')
        .filter(|param| {
            let name = param
                .split('=')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            let name = name.as_str();
            !name.starts_with("utm_") && !TRACKERS.contains(&name) && !site_trackers.contains(&name)
        })
        .collect();
    let mut clean = base.to_string();
    if !kept.is_empty() {
        clean.push('?');
        clean.push_str(&kept.join("&"));
    }
    if let Some(fragment) = fragment {
        clean.push('#');
        clean.push_str(fragment);
    }
    clean
}

/// The host of `base`, a link up to its query, in lower case, without a user or a port.
fn host(base: &str) -> String {
    let after = base.split_once("://").map_or(base, |(_, after)| after);
    let authority = after.split('/').next().unwrap_or_default();
    let authority = authority.rsplit('@').next().unwrap_or_default();
    let host = match authority.rsplit_once(':') {
        Some((host, port)) if port.bytes().all(|byte| byte.is_ascii_digit()) => host,
        _ => authority,
    };
    host.trim_end_matches('.').to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::clean_link;

    fn unchanged(urls: &[&str]) {
        for url in urls {
            assert_eq!(clean_link(url), *url, "{url}");
        }
    }

    #[test]
    fn leaves_out_trackers_and_keeps_the_rest() {
        let cases = [
            (
                "https://a.test/p?utm_source=x&id=3&utm_Medium=y#top",
                "https://a.test/p?id=3#top",
            ),
            ("https://a.test/?fbclid=1&gclid=2", "https://a.test/"),
            (
                "https://a.test/?q=utm_source&page=2",
                "https://a.test/?q=utm_source&page=2",
            ),
            (
                "https://a.test/#utm_source=x",
                "https://a.test/#utm_source=x",
            ),
            ("https://a.test/path", "https://a.test/path"),
            ("https://a.test/?", "https://a.test/?"),
        ];
        for (url, clean) in cases {
            assert_eq!(clean_link(url), clean, "{url}");
        }
    }

    /// The owner's worry (#2351): a video's time never goes. YouTube's share ID does.
    #[test]
    fn a_videos_time_stays() {
        let cases = [
            ("https://youtu.be/ID?si=X&t=42", "https://youtu.be/ID?t=42"),
            (
                "https://www.youtube.com/watch?v=ID&si=X&t=1m30s",
                "https://www.youtube.com/watch?v=ID&t=1m30s",
            ),
            (
                "https://m.youtube.com/watch?si=X&v=ID",
                "https://m.youtube.com/watch?v=ID",
            ),
        ];
        for (url, clean) in cases {
            assert_eq!(clean_link(url), clean, "{url}");
        }
        unchanged(&[
            "https://www.youtube.com/watch?v=ID&t=1m30s&list=L&index=3",
            "https://youtu.be/ID?t=42",
            "https://www.youtube.com/watch?v=ID#t=42",
            "https://youtu.be/ID#t=1m30s",
            "https://www.youtube.com/watch?v=ID&start=10&end=20",
        ]);
    }

    /// `t` and `s` are a share's ID and client on X and Twitter alone; `si` and `igsh` are trackers
    /// on their sites alone.
    #[test]
    fn a_sites_trackers_go_on_that_site_alone() {
        let cases = [
            (
                "https://x.com/user/status/1?s=20&t=abc",
                "https://x.com/user/status/1",
            ),
            (
                "https://twitter.com/user/status/1?t=abc&s=46",
                "https://twitter.com/user/status/1",
            ),
            (
                "https://mobile.twitter.com/user/status/1?s=20",
                "https://mobile.twitter.com/user/status/1",
            ),
            (
                "https://www.instagram.com/p/ID/?igsh=X",
                "https://www.instagram.com/p/ID/",
            ),
            (
                "https://WWW.YouTube.com:443/watch?v=ID&SI=X",
                "https://WWW.YouTube.com:443/watch?v=ID",
            ),
        ];
        for (url, clean) in cases {
            assert_eq!(clean_link(url), clean, "{url}");
        }
        unchanged(&[
            "https://example.com/?t=42&s=3",
            "https://www.youtube.com/watch?v=ID&t=42&s=3",
            "https://open.spotify.com/track/ID?si=X",
            "https://example.com/?si=X&igsh=Y",
            "https://x.com/search?q=rust&src=typed_query",
            // A host that only ends in a site's name, or has it before its own.
            "https://notx.com/?t=1",
            "https://youtube.com.example/?si=X",
            "https://example.com/?next=https://x.com/a?t=1",
        ]);
    }

    /// What a page is, never a tracker: a search, a shop's item and page, a video and its list, a
    /// language, and a signed link's token, signature and expiry.
    #[test]
    fn a_pages_content_stays() {
        unchanged(&[
            "https://www.google.com/search?q=gpui&hl=en",
            "https://shop.example/item?id=42&page=2&ref=nav&lang=de",
            "https://www.youtube.com/watch?v=ID&list=L",
            "https://files.example/doc.pdf?token=abc&sig=def&expires=1700000000",
            "https://bucket.s3.amazonaws.com/f?X-Amz-Signature=abc&X-Amz-Expires=60",
        ]);
    }
}
