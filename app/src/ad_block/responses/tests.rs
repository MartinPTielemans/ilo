use super::*;

/// Every filter of `header=`, `ipaddress=` or `requestheader=` in the lists uBlock Origin turns on,
/// as they were downloaded on 2026-11-14: ublock-badware 39, ublock-filters 58, ublock-privacy 6
/// and ublock-quick-fixes 6. The other lists have none.
const LISTS: &str = include_str!("lists-2026-11-14.txt");

fn asked<'a>(url: &'a str, source: &str, kind: &'a str) -> Asked<'a> {
    let host = host_of(url);
    let source_host = host_of(source);
    let site = |host: &str| host.rsplitn(3, '.').take(2).collect::<Vec<_>>().join(".");
    Asked {
        address: address_of(url, &host),
        url,
        kind,
        third_party: site(&host) != site(&source_host),
        host,
        source_host,
    }
}

fn of(lines: &str) -> Responses {
    let (responses, not_honoured) = Responses::of(lines);
    assert_eq!(not_honoured, [], "every filter here is one the app reads");
    responses
}

#[test]
fn every_response_time_filter_in_the_lists_parses_but_four_named() {
    let (responses, not_honoured) = Responses::of(LISTS);
    let named: Vec<(&str, &str)> = not_honoured
        .iter()
        .map(|(line, why)| {
            let kind = match () {
                _ if why.starts_with("replace=") => "replace=",
                _ if why.starts_with("requestheader=") => "requestheader=",
                _ => why.as_str(),
            };
            (line.as_str(), kind)
        })
        .collect();
    // replace= rewrites a body, which uBlock Origin on Chromium can't either, and requestheader=
    // reads the request's Cookie, which the orchestrator ruled out on 2026-11-14.
    let expected: Vec<(&str, &str)> = LISTS
        .lines()
        .filter(|line| line.contains(",replace=") || line.contains("$replace="))
        .map(|line| (line, "replace="))
        .chain([(
            "||workers.dev/index.js$script,3p,requestheader=Cookie:*doubleclick.net*",
            "requestheader=",
        )])
        .collect();
    assert_eq!(expected.len(), 4);
    assert_eq!(named, expected, "only these four aren't honoured");
    assert_eq!(responses.filters.len(), LISTS.lines().count() - 4);
}

#[test]
fn the_lists_filters_come_apart_from_what_adblock_rust_reads() {
    let mixed = format!(
        "! a comment, header=x\n||ads.example^\n{LISTS}example.com##.ad\n/ads.js?api_key=*&header=$all\n"
    );
    let (rest, lines) = apart(mixed);
    assert_eq!(lines, LISTS);
    assert_eq!(
        rest,
        "! a comment, header=x\n||ads.example^\nexample.com##.ad\n/ads.js?api_key=*&header=$all\n",
        "a header= in an address, a comment or a cosmetic filter isn't an option"
    );
}

#[test]
fn a_header_filter_asks_at_the_response_and_blocks_by_its_value() {
    let responses = of("||ads.example^$script,header=via:1.1 google\n");
    let script = asked(
        "https://ads.example/a.js",
        "https://news.example/",
        "script",
    );
    assert_eq!(
        responses.at_request(&script, || false),
        Some(AtRequest::Ask(vec!["via".into()]))
    );
    let blocks = |headers: &str| responses.at_response(&script, "GET", headers, || false);
    assert!(blocks("Via: 1.1 GOOGLE\n"), "names and values in any case");
    assert!(
        blocks("via: 2.0 x, 1.1 google, 1.1 y\n"),
        "anywhere in the value"
    );
    assert!(!blocks("via: 1.1 varnish\n"));
    assert!(!blocks(""), "a missing header doesn't match");
    assert!(
        !blocks("via: 1.1 varnish\nvia: 1.1 google\n"),
        "only the first of a name counts, as uBlock Origin's lookup finds it"
    );

    let image = asked(
        "https://ads.example/a.png",
        "https://news.example/",
        "image",
    );
    assert_eq!(responses.at_request(&image, || false), None, "another type");
    let other = asked(
        "https://cdn.example/a.js",
        "https://news.example/",
        "script",
    );
    assert_eq!(
        responses.at_request(&other, || false),
        None,
        "another address"
    );
}

#[test]
fn a_header_value_reads_as_ublock_origin_reads_it() {
    let test = |filter: &str, value: Option<&str>| {
        let responses = of(&format!("||ads.example^$script,{filter}\n"));
        let script = asked(
            "https://ads.example/a.js",
            "https://news.example/",
            "script",
        );
        let headers = value.map_or(String::new(), |value| format!("x-test: {value}\n"));
        responses.at_response(&script, "GET", &headers, || false)
    };
    assert!(
        test("header=x-test", Some("")),
        "no value: the header being there"
    );
    assert!(!test("header=x-test", None));
    assert!(test("header=x-test:~abc", Some("xyz")), "~ is not");
    assert!(test("header=x-test:~abc", None), "not, with no header");
    assert!(!test("header=x-test:~abc", Some("abc")));
    assert!(test("header=x-test:a*c", Some("abbbc")), "* is anything");
    assert!(test("header=x-test:a?c", Some("ac")), "? is one or none");
    assert!(test("header=x-test:a.c", Some("a.c")));
    assert!(!test("header=x-test:a.c", Some("abc")), "a dot is a dot");
    assert!(
        test("header=x-test:/^a\\d$/", Some("a1")),
        "a regular expression"
    );
    assert!(!test("header=x-test:/^a$/", Some("A")), "in its case");
    assert!(test("header=x-test:/^a$/i", Some("A")), "but with /i");
    assert!(
        test(r"header=x-test:/^referer\,accept/i", Some("Referer,Accept")),
        "an escaped comma stays in the value"
    );
    assert!(
        test("header=x-test:/^(?=.*b)a/", Some("ab")),
        "a lookahead, as JavaScript's"
    );
}

#[test]
fn an_address_filter_blocks_a_host_written_as_its_address_as_the_request_starts() {
    let responses = of("$doc,ipaddress=203.0.113.7\n*$script,ipaddress=/^2001:db8:/\n");
    let page = |url| asked(url, url, "document");
    assert_eq!(
        responses.at_request(&page("http://203.0.113.7/login"), || false),
        Some(AtRequest::Block(None))
    );
    assert_eq!(
        responses.at_request(&page("https://bank.example/login"), || false),
        None,
        "Chromium tells uBlock Origin no server's address: a host's name never matches"
    );
    let script = asked(
        "http://[2001:db8::1]/a.js",
        "https://news.example/",
        "script",
    );
    assert_eq!(
        responses.at_request(&script, || false),
        Some(AtRequest::Block(None))
    );
}

#[test]
fn an_address_filter_answers_with_the_stand_in_it_names() {
    let responses = of("*$script,3p,ipaddress=0.0.0.0,redirect=noopjs\n");
    let script = asked("http://0.0.0.0/a.js", "https://news.example/", "script");
    assert_eq!(
        responses.at_request(&script, || false),
        Some(AtRequest::Block(Some("noopjs".into())))
    );
}

#[test]
fn exceptions_let_a_request_go_but_for_important_filters() {
    let filters = "||ads.example^$script,header=x-ad\n";
    let script = asked(
        "https://ads.example/a.js",
        "https://news.example/",
        "script",
    );

    let responses = of(filters);
    assert_eq!(
        responses.at_request(&script, || true),
        None,
        "adblock-rust's"
    );
    assert!(!responses.at_response(&script, "GET", "x-ad: 1\n", || true));

    let responses = of(&format!("{filters}@@||ads.example/a.js$header=x-ad:1\n"));
    assert!(
        !responses.at_response(&script, "GET", "x-ad: 1\n", || false),
        "a header exception that matches"
    );
    assert!(responses.at_response(&script, "GET", "x-ad: 2\n", || false));

    let responses = of("||ads.example^$script,important,header=x-ad\n");
    assert!(responses.at_request(&script, || true).is_some());
    assert!(responses.at_response(&script, "GET", "x-ad: 1\n", || true));
}

#[test]
fn a_method_is_read_at_the_response() {
    let responses = of("||ads.example^$xhr,method=head,header=x-ad\n");
    let xhr = asked(
        "https://ads.example/",
        "https://news.example/",
        "xmlhttprequest",
    );
    assert!(responses.at_request(&xhr, || false).is_some());
    assert!(responses.at_response(&xhr, "HEAD", "x-ad: 1\n", || false));
    assert!(!responses.at_response(&xhr, "GET", "x-ad: 1\n", || false));
}

#[test]
fn a_page_is_never_asked_about_its_headers() {
    let responses = of("||ads.example^$header=x-ad\n");
    let page = asked("https://ads.example/", "https://ads.example/", "document");
    assert_eq!(
        responses.at_request(&page, || false),
        None,
        "uBlock Origin filters no page's own headers"
    );
}

#[test]
fn party_and_domains_narrow_a_filter() {
    let responses = of("*$script,3p,from=~edu,to=example|~safe.example,header=x-ad\n");
    let ask =
        |url, source| (responses.at_request(&asked(url, source, "script"), || false)).is_some();
    assert!(ask("https://cdn.example/a.js", "https://news.test/"));
    assert!(
        !ask("https://cdn.example/a.js", "https://www.cdn.example/"),
        "first party"
    );
    assert!(
        !ask("https://cdn.example/a.js", "https://school.edu/"),
        "from ~edu"
    );
    assert!(
        !ask("https://safe.example/a.js", "https://news.test/"),
        "to ~safe.example"
    );
    assert!(
        !ask("https://cdn.test/a.js", "https://news.example/"),
        "to example"
    );
}

#[test]
fn options_start_at_the_last_dollar_an_option_follows() {
    let responses = of("/^https:\\/\\/ads\\.example\\/a\\.js$/$script,match-case,header=x-ad\n");
    let ask = |url| {
        let script = asked(url, "https://news.example/", "script");
        responses.at_request(&script, || false).is_some()
    };
    assert!(ask("https://ads.example/a.js"));
    assert!(!ask("https://ads.example/A.js"), "match-case");
    assert!(
        !ask("https://ads.example/a.jsx"),
        "the regular expression's own $"
    );
}
