//! Links as they're copied: without the query parameters that only tell a site where a click came
//! from, as Dia copies them (docs/ux-spec.md › Command bar).

/// Click IDs that ad and mail services add to links, besides the utm_ parameters (ours).
const TRACKERS: &[&str] = &[
    "_hsenc", "_hsmi", "dclid", "fbclid", "gbraid", "gclid", "igshid", "mc_cid", "mc_eid",
    "mkt_tok", "msclkid", "ttclid", "twclid", "wbraid", "yclid",
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
    let kept: Vec<&str> = query
        .split('&')
        .filter(|param| {
            let name = param
                .split('=')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            !name.starts_with("utm_") && !TRACKERS.contains(&name.as_str())
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

#[cfg(test)]
mod tests {
    use super::clean_link;

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
}
