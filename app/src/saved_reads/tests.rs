use serde_json::{Map, Value, json};
use url::Url;

use super::address::{Checks, build};
use super::answer::{Answered, Outcome, outcome};
use super::definition::{Definition, safe_pattern};
use super::store::{Refusal, State, Store};
use super::table::tables;

const SITE: &str = "https://shop.example";

/// The prototype's item read (#3814): one string parameter with a strict pattern.
fn item() -> Definition {
    Definition::parse(&json!({
        "name": "item",
        "site": SITE,
        "kind": "json",
        "url": "/items/{id}",
        "params": { "id": { "type": "text", "pattern": "A-[0-9]{1,6}" } },
        "columns": [{ "name": "name", "type": "text", "from": "name" }],
    }))
    .unwrap()
}

fn params(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect()
}

/// The prototype's ten hostile values for a parameter.
const HOSTILE: [&str; 10] = [
    "../../away",
    "..%2F..%2Faway",
    "//127.0.0.2:8899/x",
    "A-1041/../../away",
    "A-1041?extra=1",
    "A-1041#x",
    "@127.0.0.2",
    "http://127.0.0.2:8899/",
    "A-1041/",
    "%2e%2e",
];

/// Whether `url` stayed where item's reads go: on the site, at /items/ and one segment that is
/// no separator, dot segment or encoding even decoded, with no query, fragment or user. It's the
/// tests' own reading, written apart from the address check it tests.
fn stays(url: &Url) -> bool {
    let Some(segment) = url.path().strip_prefix("/items/") else {
        return false;
    };
    let decoded = percent_decoded(segment);
    url.as_str().starts_with("https://shop.example/items/")
        && url.host_str() == Some("shop.example")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && !segment.is_empty()
        && !decoded.contains(['/', '\\', '%'])
        && !decoded.trim_matches('.').is_empty()
}

fn percent_decoded(text: &str) -> String {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' && at + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&text[at + 1..at + 3], 16) {
                out.push(byte);
                at += 3;
                continue;
            }
        }
        out.push(bytes[at]);
        at += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

const VALUES_ONLY: Checks = Checks {
    values: true,
    address: false,
};
const ADDRESS_ONLY: Checks = Checks {
    values: false,
    address: true,
};
const NEITHER: Checks = Checks {
    values: false,
    address: false,
};

#[test]
fn a_good_value_reads_its_item() {
    let url = super::address(&item(), &params(&[("id", json!("A-1041"))])).unwrap();
    assert_eq!(url.as_str(), "https://shop.example/items/A-1041");
}

#[test]
fn the_values_check_alone_refuses_each_hostile_value() {
    for value in HOSTILE {
        let built = build(&item(), &params(&[("id", json!(value))]), VALUES_ONLY);
        assert!(
            built.is_err(),
            "the values check let {value:?} through, to {built:?}"
        );
    }
}

#[test]
fn the_address_check_alone_keeps_each_hostile_value_on_the_path() {
    for value in HOSTILE {
        match build(&item(), &params(&[("id", json!(value))]), ADDRESS_ONLY) {
            Err(_) => {}
            Ok(url) => assert!(stays(&url), "the address check let {value:?} go to {url}"),
        }
    }
}

/// Without either check the same values leave, so the two tests above see a check taken out.
#[test]
fn without_the_checks_hostile_values_leave_the_path() {
    for value in HOSTILE {
        let url = build(&item(), &params(&[("id", json!(value))]), NEITHER).unwrap();
        // "@127.0.0.2" in a path is a segment on the site, which the checks may let pass.
        if value != "@127.0.0.2" {
            assert!(!stays(&url), "{value:?} stayed even with no check: {url}");
        }
    }
}

/// The prototype's finding: a value of "..%2F..%2Faway" stays on the path as a browser reads it,
/// but a server that decodes %2F before it routes sees a path off it. The address check reads a
/// segment decoded, and no pattern a definition may have matches a "%".
#[test]
fn an_encoded_separator_is_off_the_path() {
    let value = "..%2F..%2Faway";
    let naive = build(&item(), &params(&[("id", json!(value))]), NEITHER).unwrap();
    assert_eq!(
        naive.path(),
        "/items/..%2F..%2Faway",
        "the browser reads it as one segment"
    );
    assert!(build(&item(), &params(&[("id", json!(value))]), ADDRESS_ONLY).is_err());
    for loose in [
        ".*",
        ".+",
        "[^/]+",
        "[a-zA-Z0-9%.]+",
        "[+-0]+",
        "\\S+",
        "\\w+",
        "(?s).*",
        "[a-z]|.",
    ] {
        assert!(
            safe_pattern(loose).is_err(),
            "the pattern {loose:?} was allowed"
        );
    }
}

#[test]
fn a_parameter_the_read_has_not_is_refused_or_goes_nowhere() {
    let asked = params(&[("id", json!("A-1041")), ("path", json!("/away"))]);
    assert!(build(&item(), &asked, VALUES_ONLY).is_err());
    let url = build(&item(), &asked, ADDRESS_ONLY).unwrap();
    assert!(stays(&url) && !url.as_str().contains("away"), "{url}");
}

#[test]
fn a_missing_parameter_is_refused() {
    assert!(super::address(&item(), &Map::new()).is_err());
}

#[test]
fn narrow_patterns_are_allowed_and_match_no_separator() {
    for pattern in [
        "[a-z0-9-]{1,40}",
        "A-\\d{4}",
        "[0-9]+(\\.[0-9]+)*",
        "serde|tokio",
        "[a-z_~]+",
        "v\\.[0-9]",
    ] {
        assert_eq!(safe_pattern(pattern), Ok(()), "{pattern}");
        let whole = fancy_regex::Regex::new(&format!("^(?:{pattern})$")).unwrap();
        for bad in ["/", "%2F", "a/b", "?", "#", "\\", "a b"] {
            assert!(!whole.is_match(bad).unwrap(), "{pattern} matched {bad:?}");
        }
    }
    for refused in [
        "", "a.b", "[^a]", "a/b", "a%b", "[a-z", "(a", "a)", "a{99999}", "(?i)a", "\\x2F",
        "[a-\\d]", "[9-0]",
    ] {
        assert!(safe_pattern(refused).is_err(), "{refused:?} was allowed");
    }
}

#[test]
fn integers_and_lists_are_checked_and_encoded() {
    let read = Definition::parse(&json!({
        "name": "downloads",
        "site": "https://crates.example",
        "kind": "json",
        "url": "/api/v1/crates/{krate}/downloads?days={days}&sort=recent",
        "params": {
            "krate": { "type": "one_of", "values": ["serde", "tokio"] },
            "days": { "type": "integer", "min": 1, "max": 90 },
        },
        "rows": "version_downloads",
        "columns": [{ "name": "version", "from": "version" }],
    }))
    .unwrap();
    let url = super::address(
        &read,
        &params(&[("krate", json!("serde")), ("days", json!(7))]),
    )
    .unwrap();
    assert_eq!(
        url.as_str(),
        "https://crates.example/api/v1/crates/serde/downloads?days=7&sort=recent"
    );
    for (krate, days) in [
        ("serde", json!(0)),
        ("serde", json!(91)),
        ("serde", json!("7&extra=1")),
        ("other", json!(7)),
        ("serde", json!("1e3")),
        ("serde", json!([7])),
    ] {
        let asked = params(&[("krate", json!(krate)), ("days", days.clone())]);
        assert!(super::address(&read, &asked).is_err(), "{krate} {days}");
    }
    // A value that adds a query key or a fragment is off the read's address, with the values
    // check out too.
    for days in ["7&extra=1", "7#top"] {
        let asked = params(&[("krate", json!("serde")), ("days", json!(days))]);
        assert!(build(&read, &asked, ADDRESS_ONLY).is_err(), "{days}");
    }
}

#[test]
fn a_definition_off_its_site_or_shape_is_refused() {
    let base = || {
        json!({
            "name": "item", "site": SITE, "kind": "json", "url": "/items/{id}",
            "params": { "id": { "type": "integer", "min": 1, "max": 9 } },
            "columns": [{ "name": "name" }],
        })
    };
    let refused = |change: &dyn Fn(&mut Value)| {
        let mut value = base();
        change(&mut value);
        Definition::parse(&value).is_err()
    };
    assert!(Definition::parse(&base()).is_ok());
    for site in [
        "https://shop.example/items",
        "ftp://shop.example",
        "https://user@shop.example",
        "shop.example",
        "https://shop.example?x=1",
        "javascript:alert(1)",
    ] {
        assert!(refused(&|value| value["site"] = json!(site)), "site {site}");
    }
    for url in [
        "items/{id}",
        "//away.example/{id}",
        "/items/{id}#top",
        "/items/../{id}",
        "/items/{other}",
        "/items",
        "/items/{id}/{id}",
        "/items?{id}=1",
        "/items/{id}\\x",
        "/items/{ id}",
        "/items/{id",
    ] {
        assert!(refused(&|value| value["url"] = json!(url)), "url {url}");
    }
    assert!(refused(&|value| value["name"] = json!("")));
    assert!(refused(&|value| value["name"] = json!("a\u{202E}b")));
    assert!(refused(&|value| value["name"] = json!("x".repeat(41))));
    assert!(refused(&|value| value["columns"] = json!([])));
    assert!(refused(
        &|value| value["columns"] = json!([{ "name": "a" }, { "name": "A" }])
    ));
    assert!(refused(
        &|value| value["columns"] = json!([{ "name": "a", "type": "html" }])
    ));
    assert!(refused(&|value| value["kind"] = json!("script")));
    assert!(refused(&|value| value["least_rows"] = json!(0)));
    assert!(refused(
        &|value| value["sign_in"] = json!("https://away.example/login")
    ));
    for sign_in in [
        "//away.example/login",
        "/a/../login",
        "/login?next=1",
        "/%2e%2e/login",
        "login",
    ] {
        assert!(
            refused(&|value| value["sign_in"] = json!(sign_in)),
            "sign_in {sign_in}"
        );
    }
    assert!(refused(
        &|value| value["params"]["id"] = json!({ "type": "text", "pattern": ".*" })
    ));
    assert!(refused(
        &|value| value["params"]["id"] = json!({ "type": "one_of", "values": ["a/b"] })
    ));
    assert!(refused(
        &|value| value["params"]["id"] = json!({ "type": "one_of", "values": [".."] })
    ));
    assert!(refused(
        &|value| value["params"]["id"] = json!({ "type": "integer", "min": 9, "max": 1 })
    ));
    let site = |site: &str| {
        let mut value = base();
        value["site"] = json!(site);
        Definition::parse(&value).map(|read| read.site)
    };
    assert_eq!(
        site("https://Shop.Example/"),
        Ok("https://shop.example".into())
    );
    assert_eq!(
        site("https://shop.example:443"),
        Ok("https://shop.example".into())
    );
    assert_eq!(
        site("http://127.0.0.1:8811"),
        Ok("http://127.0.0.1:8811".into())
    );
}

#[test]
fn a_definition_reads_back_as_it_was_kept() {
    let mut read = item();
    read.sign_in = Some("/login".into());
    assert_eq!(Definition::parse(&read.to_json()), Ok(read));
    assert_eq!(Definition::parse(&buildings().to_json()), Ok(buildings()));
}

// The data kind.

fn downloads() -> Definition {
    Definition::parse(&json!({
        "name": "crate downloads",
        "site": "https://crates.example",
        "kind": "json",
        "url": "/api/v1/crates/serde/downloads",
        "rows": "version_downloads",
        "columns": [
            { "name": "version", "type": "text", "from": "version" },
            { "name": "downloads", "type": "number", "from": "downloads" },
            { "name": "date", "from": "date" },
        ],
        "least_rows": 2,
    }))
    .unwrap()
}

fn answered<'a>(url: &'a str, mime: &'a str, body: &'a [u8]) -> Answered<'a> {
    Answered {
        status: 200,
        url,
        left: false,
        mime,
        body,
        cut: false,
    }
}

const DOWNLOADS: &str = "https://crates.example/api/v1/crates/serde/downloads";

fn rows(outcome: Outcome) -> Vec<Vec<String>> {
    match outcome {
        Outcome::Rows(rows) => rows.into_iter().map(|row| row.0).collect(),
        other => panic!("not rows: {other:?}"),
    }
}

fn broken(outcome: Outcome) -> String {
    match outcome {
        Outcome::Broken(why) => why.0,
        other => panic!("not broken: {other:?}"),
    }
}

#[test]
fn the_data_kind_reads_fields_by_path() {
    let body = json!({ "version_downloads": [
        { "version": "1.0.219", "downloads": 412330, "date": "2026-11-18" },
        { "version": "1.0.218", "downloads": 98012, "date": "2026-11-17", "extra": [1] },
    ]})
    .to_string();
    let read = rows(outcome(
        &downloads(),
        &answered(DOWNLOADS, "application/json", body.as_bytes()),
    ));
    assert_eq!(
        read,
        [
            ["1.0.219", "412330", "2026-11-18"],
            ["1.0.218", "98012", "2026-11-17"]
        ]
    );
}

#[test]
fn a_changed_answer_reads_as_broken() {
    let read = downloads();
    let two = |first: Value| json!({ "version_downloads": [first, { "version": "2", "downloads": 1, "date": "d" }] });
    let cases: [(Value, &str); 5] = [
        (
            json!({ "versions": [] }),
            "The site's data is missing version_downloads.",
        ),
        (
            json!({ "version_downloads": 3 }),
            "version_downloads isn't formatted as rows.",
        ),
        (
            two(json!({ "version": "1", "date": "d" })),
            "Row 1: downloads is missing.",
        ),
        (
            two(json!({ "version": "1", "downloads": "many", "date": "d" })),
            "Row 1: downloads isn't a number.",
        ),
        (
            two(json!({ "version": ["1"], "downloads": 1, "date": "d" })),
            "Row 1: version isn't a single value.",
        ),
    ];
    for (body, why) in cases {
        let body = body.to_string();
        assert_eq!(
            broken(outcome(
                &read,
                &answered(DOWNLOADS, "application/json", body.as_bytes())
            )),
            why
        );
    }
    let one = json!({ "version_downloads": [{ "version": "1", "downloads": 1, "date": "d" }] })
        .to_string();
    let answer = answered(DOWNLOADS, "application/json", one.as_bytes());
    assert_eq!(
        broken(outcome(&read, &answer)),
        "Found 1 row; expected at least 2."
    );
    let page = answered(DOWNLOADS, "text/html", b"<html>Sign in</html>");
    assert_eq!(
        broken(outcome(&read, &page)),
        concat!(
            "The site didn't send data ",
            env!("PRODUCT_NAME"),
            " can read."
        )
    );
}

#[test]
fn values_come_out_as_text_clipped() {
    let long = "9".repeat(600);
    let body = json!({ "version_downloads": [
        { "version": "<b>1.0</b>\u{202E}\n\tnext", "downloads": "412,330", "date": long },
        { "version": "2", "downloads": "1 000", "date": "d" },
    ]})
    .to_string();
    let read = rows(outcome(
        &downloads(),
        &answered(DOWNLOADS, "application/json", body.as_bytes()),
    ));
    assert_eq!(
        read[0][0], "<b>1.0</b> next",
        "markup is kept as text, and what isn't drawn goes"
    );
    assert_eq!(read[0][1], "412,330");
    assert_eq!(read[0][2].chars().count(), super::VALUE_KEPT + 1);
    assert!(read[0][2].ends_with('…'));
}

#[test]
fn the_answer_decides_a_sign_in() {
    let mut read = downloads();
    read.sign_in = Some("/login".into());
    let body = b"{}";
    for status in [401, 403] {
        let answer = Answered {
            status,
            ..answered(DOWNLOADS, "application/json", body)
        };
        assert_eq!(outcome(&read, &answer), Outcome::NeedsSignIn);
    }
    let left = Answered {
        left: true,
        ..answered(DOWNLOADS, "", body)
    };
    assert_eq!(outcome(&read, &left), Outcome::NeedsSignIn);
    let sign_in = answered(
        "https://crates.example/login?next=%2Fapi",
        "text/html",
        body,
    );
    assert_eq!(outcome(&read, &sign_in), Outcome::NeedsSignIn);
    let elsewhere = answered(
        "https://crates.example/logins-are-elsewhere",
        "text/html",
        body,
    );
    assert_eq!(
        broken(outcome(&read, &elsewhere)),
        "The site redirected to another address."
    );
    let failed = Answered {
        status: 500,
        ..answered(DOWNLOADS, "", body)
    };
    assert_eq!(
        broken(outcome(&read, &failed)),
        "The site returned error 500."
    );
    let cut = Answered {
        cut: true,
        ..answered(DOWNLOADS, "application/json", body)
    };
    assert_eq!(
        broken(outcome(&read, &cut)),
        "The site sent more than 4 MB."
    );
}

/// The read's site is one origin: scheme, host and port, never the registrable domain.
#[test]
fn the_site_is_one_origin() {
    for elsewhere in [
        "http://shop.example/items/A-1",
        "https://shop.example:8443/items/A-1",
        "https://www.shop.example/items/A-1",
        "https://example/items/A-1",
    ] {
        let url = Url::parse(elsewhere).unwrap();
        assert!(!super::address::fits(&item(), &url), "{elsewhere} fit");
    }
    assert!(super::address::fits(
        &item(),
        &Url::parse("https://shop.example:443/items/A-1").unwrap()
    ));
    // A sign-in path on another origin is no sign-in: the core stops that redirect itself, and
    // an answer landing there otherwise is off the read.
    let mut read = downloads();
    read.sign_in = Some("/login".into());
    let away = answered("http://crates.example/login", "text/html", b"");
    assert_eq!(
        broken(outcome(&read, &away)),
        "The site redirected to another address."
    );
    let other = answered("https://login.crates.example/login", "text/html", b"");
    assert_eq!(
        broken(outcome(&read, &other)),
        "The site redirected to another address."
    );
    // A name a person writes in its own letters is the same origin the url crate reads.
    let mut value = item().to_json();
    value["site"] = json!("https://Bücher.example");
    let read = Definition::parse(&value).unwrap();
    assert_eq!(read.site, "https://xn--bcher-kva.example");
    assert!(super::address(&read, &params(&[("id", json!("A-1"))])).is_ok());
}

/// Settings shows a broken reason and the log keeps it, and neither keeps a row's content, so a
/// reason built from a site's text holds none of it.
#[test]
fn a_broken_reason_holds_none_of_the_sites_text() {
    let hostile = "SECRET-7f3a <img src=x onerror=alert(1)>";
    let read = downloads();
    let bodies = [
        json!({ "version_downloads": [{ "version": "1", "downloads": hostile, "date": "d" }] }),
        json!({ "version_downloads": [{ "version": [hostile], "downloads": 1, "date": "d" }] }),
        json!({ "version_downloads": [{ hostile: "1", "downloads": 1, "date": "d" }] }),
        json!({ "version_downloads": hostile }),
        json!({ hostile: [] }),
        json!([hostile]),
    ];
    let mut reasons: Vec<String> = (bodies.iter())
        .map(|body| {
            broken(outcome(
                &read,
                &answered(DOWNLOADS, "application/json", body.to_string().as_bytes()),
            ))
        })
        .collect();
    let not_json = format!("{hostile} {{");
    reasons.push(broken(outcome(
        &read,
        &answered(DOWNLOADS, "application/json", not_json.as_bytes()),
    )));
    let cells = [
        format!(
            "<table><tr><th>Name</th><th>City</th><th>Height (m)</th></tr><tr><td>{hostile}</td><td>{hostile}</td><td>{hostile}</td></tr></table>"
        ),
        format!("<table><tr><th>{hostile}</th></tr><tr><td>{hostile}</td></tr></table>"),
        format!(
            "<table><tr><th>Name</th><th>City</th><th>Height (m)</th></tr><tr><td>{hostile}</td><td>{hostile}</td><td></td></tr></table>"
        ),
    ];
    reasons.extend(cells.iter().map(|html| broken(table(html))));
    let mut links = Definition::parse(&json!({
        "name": "links", "site": "https://wiki.example", "kind": "table", "url": "/wiki/Tallest_buildings",
        "columns": [{ "name": "page", "type": "address", "from": "Name" }],
    }))
    .unwrap();
    let html = format!(
        "<table><tr><th>Name</th></tr><tr><td><a href='javascript:{hostile}'>{hostile}</a></td></tr></table>"
    );
    reasons.push(broken(outcome(
        &links,
        &answered(BUILDINGS, "text/html", html.as_bytes()),
    )));
    links.least_rows = 3;
    let html = format!(
        "<table><tr><th>Name</th></tr><tr><td><a href='/{hostile}'>{hostile}</a></td></tr></table>"
    );
    reasons.push(broken(outcome(
        &links,
        &answered(BUILDINGS, "text/html", html.as_bytes()),
    )));
    for reason in reasons {
        for word in ["SECRET", "7f3a", "img", "onerror", "alert"] {
            assert!(
                !reason.contains(word),
                "the reason “{reason}” holds the site's {word}"
            );
        }
    }
}

// The table kind.

fn buildings() -> Definition {
    Definition::parse(&json!({
        "name": "city buildings",
        "site": "https://wiki.example",
        "kind": "table",
        "url": "/wiki/Tallest_buildings",
        "columns": [
            { "name": "name", "from": "Name" },
            { "name": "city", "from": "City" },
            { "name": "height", "type": "number", "from": "Height (m)" },
        ],
    }))
    .unwrap()
}

const BUILDINGS: &str = "https://wiki.example/wiki/Tallest_buildings";

fn table(html: &str) -> Outcome {
    outcome(
        &buildings(),
        &answered(BUILDINGS, "text/html; charset=utf-8", html.as_bytes()),
    )
}

#[test]
fn a_cell_spanning_rows_doesnt_shift_a_column() {
    let html = "<table><tr><th>Name</th><th>City</th><th>Height (m)</th></tr>\
        <tr><td>One</td><td rowspan=2>Rotterdam</td><td>150</td></tr>\
        <tr><td>Two</td><td>140</td></tr>\
        <tr><td>Three</td><td>Utrecht</td><td>130</td></tr></table>";
    assert_eq!(
        rows(table(html)),
        [
            ["One", "Rotterdam", "150"],
            ["Two", "Rotterdam", "140"],
            ["Three", "Utrecht", "130"]
        ]
    );
}

#[test]
fn spans_across_rows_and_columns_together_fill_each_place() {
    let html = "<table><tr><th>Name</th><th>City</th><th>Height (m)</th></tr>\
        <tr><td rowspan=2 colspan=2>Twin towers, Amsterdam</td><td>120</td></tr>\
        <tr><td>119</td></tr>\
        <tr><td>Last</td><td>Delft</td><td>90</td></tr></table>";
    let twin = "Twin towers, Amsterdam";
    assert_eq!(
        rows(table(html)),
        [
            [twin, twin, "120"],
            [twin, twin, "119"],
            ["Last", "Delft", "90"]
        ]
    );
}

#[test]
fn cells_and_rows_without_end_tags_end_at_the_next() {
    let html = "<table><thead><tr><th>Name<th>City<th>Height (m)<tbody>\
        <tr><td>One<td>Leiden<td>100<tr><td>Two<td>Gouda<td>99</table>";
    assert_eq!(
        rows(table(html)),
        [["One", "Leiden", "100"], ["Two", "Gouda", "99"]]
    );
    // A table the page never closes ends with the page.
    let open = "<table><tr><th>Name<th>City<th>Height (m)<tr><td>One<td>Leiden<td>100<tr><td>Two<td>Gouda<td>99";
    assert_eq!(rows(table(open)).len(), 2);
}

#[test]
fn head_body_and_foot_are_read_as_html_lays_them_out() {
    let html = "<table><thead><tr><th>Name</th><th>City</th><th>Height (m)</th></tr></thead>\
        <tfoot><tr><td>Total</td><td></td><td>199</td></tr></tfoot>\
        <tbody><tr><td>One</td><td rowspan=5>Leiden</td><td>100</td></tr>\
        <tr><td>Two</td><td>99</td></tr></tbody>\
        <tbody><tr><td>Three</td><td>Gouda</td><td>98</td></tr>\
        <tr><th>Name</th><th>City</th><th>Height (m)</th></tr>\
        <tr><td>Four</td><td>Delft</td><td>97</td></tr></tbody></table>";
    assert_eq!(
        rows(table(html)),
        [
            ["One", "Leiden", "100"],
            ["Two", "Leiden", "99"],
            ["Three", "Gouda", "98"],
            ["Four", "Delft", "97"]
        ],
        "a foot's total isn't a row, a rowspan stops at its body's end, and a header repeated down the table isn't a row"
    );
}

#[test]
fn a_table_inside_a_cell_is_not_the_outer_tables_rows() {
    let html = "<table><tr><th>Name</th><th>City</th><th>Height (m)</th></tr>\
        <tr><td>One<table><tr><th>Name</th><th>City</th><th>Height (m)</th></tr>\
        <tr><td>Inner</td><td>Nowhere</td><td>1</td></tr></table></td><td>Leiden</td><td>100</td></tr>\
        </table>";
    // Both have the columns, so neither is read.
    assert_eq!(
        broken(table(html)),
        concat!(
            "2 tables have these columns; ",
            env!("PRODUCT_NAME"),
            " can't pick one."
        )
    );
    let found = tables(html);
    assert_eq!(found.len(), 2);
    assert_eq!(
        found[0].rows.len(),
        1,
        "the outer table has its own row only"
    );
    assert_eq!(
        found[0].rows[0][0].text, "One",
        "the inner table's text isn't the outer cell's"
    );
    assert_eq!(
        found[0].rows[0][1].text, "Leiden",
        "the outer row goes on after the inner table"
    );
    assert_eq!(found[1].rows[0][0].text, "Inner");
    let other = "<table><tr><th>Name</th><th>City</th><th>Height (m)</th></tr>\
        <tr><td>One<table><tr><th>Floor</th></tr><tr><td>Ground</td></tr></table></td><td>Leiden</td><td>100</td></tr>\
        </table>";
    assert_eq!(rows(table(other)), [["One", "Leiden", "100"]]);
}

#[test]
fn a_table_in_a_comment_script_style_or_template_isnt_a_table() {
    let inner = "<table><tr><th>Name</th><th>City</th><th>Height (m)</th></tr><tr><td>Fake</td><td>Nowhere</td><td>1</td></tr></table>";
    let real = "<table><tr><th>Name</th><th>City</th><th>Height (m)</th></tr><tr><td>Real</td><td>Leiden</td><td>100</td></tr></table>";
    for wrap in [
        "<!--{}-->",
        "<script>let t = '{}';</script>",
        "<style>/* {} */</style>",
        "<template>{}</template>",
        "<noscript>{}</noscript>",
        "<textarea>{}</textarea>",
    ] {
        let html = format!("{}{real}", wrap.replace("{}", inner));
        assert_eq!(rows(table(&html)), [["Real", "Leiden", "100"]], "{wrap}");
    }
}

#[test]
fn entities_whitespace_and_footnotes_in_cells_read_plainly() {
    let html = "<table><tr><th>Name</th><th>City</th><th>Height&nbsp;(m)<sup>[a]</sup></th></tr>\
        <tr><td>\n  Caf&eacute; &amp; <b>Tower</b><br>West&#32;Wing </td><td> Den&#x20;Haag[1] </td><td>1&#44;234<sup>[note 3]</sup></td></tr>\
        <tr><td>&lt;script&gt;x&lt;/script&gt;</td><td>Leiden</td><td>100</td></tr></table>";
    assert_eq!(
        rows(table(html)),
        [
            ["Café & Tower West Wing", "Den Haag", "1,234"],
            ["<script>x</script>", "Leiden", "100"]
        ]
    );
}

#[test]
fn two_tables_with_the_reads_columns_read_as_broken() {
    let one = "<table><tr><th>Name</th><th>City</th><th>Height (m)</th></tr><tr><td>A</td><td>B</td><td>1</td></tr></table>";
    let html = format!("{one}<p>and</p>{one}");
    assert_eq!(
        broken(table(&html)),
        concat!(
            "2 tables have these columns; ",
            env!("PRODUCT_NAME"),
            " can't pick one."
        )
    );
}

#[test]
fn a_changed_layout_reads_as_broken() {
    let renamed = "<table><tr><th>Building</th><th>City</th><th>Height (m)</th></tr><tr><td>One</td><td>Leiden</td><td>100</td></tr></table>";
    assert_eq!(
        broken(table(renamed)),
        "No table on the page has these columns."
    );
    let one = Definition::parse(&json!({
        "name": "names", "site": "https://wiki.example", "kind": "table", "url": "/wiki/Tallest_buildings",
        "columns": [{ "name": "name", "from": "Name" }],
    }))
    .unwrap();
    let answer = answered(BUILDINGS, "text/html", renamed.as_bytes());
    assert_eq!(
        broken(outcome(&one, &answer)),
        "No table on the page has this column."
    );
    let moved = "<table><tr><th>Name</th><th>City</th><th>Height (m)</th></tr><tr><td>One</td><td>Leiden</td><td>tall</td></tr></table>";
    assert_eq!(broken(table(moved)), "Row 1: height isn't a number.");
    let empty = "<table><tr><th>Name</th><th>City</th><th>Height (m)</th></tr></table>";
    assert_eq!(broken(table(empty)), "Found 0 rows; expected at least 1.");
}

#[test]
fn a_column_finds_its_header_by_its_words() {
    let html = "<table><tr><th rowspan=2>Name</th><th rowspan=2>City</th><th colspan=2>Height</th></tr>\
        <tr><th>(m)</th><th>(ft)</th></tr>\
        <tr><td>One</td><td>Leiden</td><td>100</td><td>328</td></tr></table>";
    assert_eq!(
        rows(table(html)),
        [["One", "Leiden", "100"]],
        "Height (m) is the header the two rows make"
    );
}

#[test]
fn an_address_column_reads_a_cells_link() {
    let read = Definition::parse(&json!({
        "name": "links", "site": "https://wiki.example", "kind": "table", "url": "/wiki/Tallest_buildings",
        "columns": [{ "name": "name", "from": "Name" }, { "name": "page", "type": "address", "from": "Name" }],
    }))
    .unwrap();
    let good =
        "<table><tr><th>Name</th></tr><tr><td><a href=\"/wiki/One?a=1&amp;b=2\">One</a></td></tr>";
    let html = format!("{good}<tr><td><a href='javascript:alert(1)'>Two</a></td></tr></table>");
    let answer = answered(BUILDINGS, "text/html", html.as_bytes());
    assert_eq!(
        broken(outcome(&read, &answer)),
        "Row 2: page isn't a web address."
    );
    let answer = answered(BUILDINGS, "text/html", good.as_bytes());
    assert_eq!(
        rows(outcome(&read, &answer)),
        [["One", "https://wiki.example/wiki/One?a=1&b=2"]]
    );
}

// Storage.

#[test]
fn reads_are_saved_changed_and_removed() {
    let mut store = Store::default();
    store
        .save(item(), "Claude Code", State::Ready, 100)
        .unwrap();
    assert_eq!(
        store.save(item(), "Codex", State::Ready, 101),
        Err(Refusal::Taken)
    );
    let mut renamed = item();
    renamed.name = "ITEM".into();
    assert_eq!(
        store.save(renamed, "Codex", State::Ready, 101),
        Err(Refusal::Taken),
        "names are one in any case"
    );
    store
        .save(downloads(), "Codex", State::NeedsSignIn, 102)
        .unwrap();

    let mut changed = downloads();
    changed.least_rows = 5;
    store
        .change("Crate Downloads", changed, State::Ready, 200)
        .unwrap();
    let saved = store.get("crate downloads").unwrap();
    assert_eq!(
        (
            saved.read.least_rows,
            saved.saved,
            saved.asked_by.as_str(),
            saved.since
        ),
        (5, 102, "Codex", 200)
    );

    store.read_at("item", State::Ready, 300).unwrap();
    assert_eq!(
        store.get("item").unwrap().since,
        100,
        "since moves only with the state"
    );
    store
        .read_at("item", State::Broken("Row 1 has no name.".into()), 400)
        .unwrap();
    assert_eq!(
        (
            store.get("item").unwrap().since,
            store.get("item").unwrap().last_read
        ),
        (400, 400)
    );

    let at = store.at("item").unwrap();
    let removed = store.remove("item").unwrap();
    assert!(store.get("item").is_none());
    store.restore(removed, at).unwrap();
    assert_eq!(
        store.list()[0].read.name,
        "item",
        "an undo puts it back where it was"
    );
    assert_eq!(
        store.change("gone", item(), State::Ready, 1),
        Err(Refusal::NotFound)
    );
}

#[test]
fn reads_survive_a_restart_in_the_profiles_folder() {
    let folder = std::env::temp_dir().join(format!("saved-reads-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let mut store = Store::default();
    store
        .save(
            item(),
            "Claude Code",
            State::Broken("The page changed.".into()),
            100,
        )
        .unwrap();
    store.save(buildings(), "Codex", State::Ready, 101).unwrap();
    store.keep(Some(&folder)).unwrap();
    assert_eq!(Store::load(Some(&folder)), store);

    // A read the file can't hold as a definition, as one kept before a rule tightened, is left out.
    let file = std::fs::read_to_string(folder.join(super::store::FILE)).unwrap();
    let mut value: Value = serde_json::from_str(&file).unwrap();
    value["reads"][0]["read"]["params"]["id"]["pattern"] = json!(".*");
    std::fs::write(folder.join(super::store::FILE), value.to_string()).unwrap();
    let loaded = Store::load(Some(&folder));
    assert_eq!(loaded.list().len(), 1);
    assert_eq!(loaded.list()[0].read.name, "city buildings");

    Store::default().keep(Some(&folder)).unwrap();
    assert!(
        !folder.join(super::store::FILE).exists(),
        "none leaves no file"
    );
    std::fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn a_private_window_has_none_and_keeps_none() {
    assert_eq!(Store::load(None), Store::default());
    let mut store = Store::default();
    store.save(item(), "Claude Code", State::Ready, 1).unwrap();
    assert!(store.keep(None).is_ok());
    assert_eq!(Store::load(None).list().len(), 0);
}

#[test]
fn text_is_one_plain_line_clipped() {
    assert_eq!(super::text("a\u{200B}b\n\n c\u{2800}d", 40), "ab c d");
    assert_eq!(super::text("abcdef", 3), "abc…");
}
