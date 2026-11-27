//! Site controls' made-up site.

use crate::*;

/// A made-up site, with permissions of each value and one a policy sets, in Chrome's words, the
/// longest of them Pop-ups and redirects.
pub(crate) fn preview_site() -> Site {
    let permission =
        |kind, key: &str, name: &str, value, state: &str, choices: &[Value]| Permission {
            kind,
            key: key.to_string().into(),
            name: name.to_string().into(),
            value,
            state: state.to_string().into(),
            choices: choices.to_vec(),
        };
    // PREVIEW_CONNECTION=not-secure shows a page over plain http, and =internal the browser's own.
    let (site, connection, summary) = match states::var("PREVIEW_CONNECTION").as_deref() {
        Ok("not-secure") => ("en.wikipedia.org", Connection::NotSecure, "Not secure"),
        Ok("internal") => ("ilo", Connection::Internal, "ilo"),
        _ => (
            "en.wikipedia.org",
            Connection::Secure,
            "Connection is secure",
        ),
    };
    Site {
        site: site.into(),
        connection,
        connection_summary: summary.into(),
        certificate: connection == Connection::Secure,
        permissions: vec![
            permission(
                5,
                "geolocation",
                "Location",
                Value::Ask,
                "Ask (default)",
                &Value::ALL,
            ),
            permission(
                6,
                "notifications",
                "Notifications",
                Value::Allow,
                "Allowed",
                &Value::ALL,
            ),
            permission(
                4,
                "popups",
                "Pop-ups and redirects",
                Value::Block,
                "Blocked",
                &Value::ALL,
            ),
            permission(2, "javascript", "JavaScript", Value::Allow, "Allowed", &[]),
        ],
        reset: Some("Reset permissions".into()),
        system_engine: None,
        logins: Vec::new(),
        devices: Vec::new(),
        ad_blocking: Some(true),
        ad_blocked: "12".into(),
        developer_mode: Some(false),
        // PREVIEW_BOOST=on or off gives the site a Boost, which shows its row.
        boostable: true,
        boost: match states::var("PREVIEW_BOOST").as_deref() {
            Ok("on") => Some(true),
            Ok("off") => Some(false),
            _ => None,
        },
        // PREVIEW_ARRANGEMENT=nofit or changed: what Site Controls says of the Boost's arrangement.
        arrangement_misfits: (states::var("PREVIEW_ARRANGEMENT").as_deref() == Ok("nofit"))
            .then(|| "1 change no longer fits".into()),
        arrangement_off: (states::var("PREVIEW_ARRANGEMENT").as_deref() == Ok("changed"))
            .then(|| concat!("Changed outside ", env!("PRODUCT_NAME"), ", off").into()),
        // PREVIEW_RECENT=on or off: the switch for what's recent in the site's cards (#3074).
        recent: match states::var("PREVIEW_RECENT").as_deref() {
            Ok("on") => Some(true),
            Ok("off") => Some(false),
            _ => None,
        },
        // The "…" menu's Sensitive Site mark, unchecked (#4369).
        sensitive: Some(false),
    }
}
