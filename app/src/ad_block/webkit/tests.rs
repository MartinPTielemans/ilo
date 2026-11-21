use super::*;

const ALL: Features = Features {
    unless_frame_url: true,
    request_method: true,
};

fn lists_of(text: &str, user: &str, features: Features) -> Lists {
    let sources = vec![
        Source {
            text: text.into(),
            options: ParseOptions::default(),
            user: false,
        },
        Source {
            text: user.into(),
            options: ParseOptions::default(),
            user: true,
        },
    ];
    from_sources(sources, features)
}

fn rules_in(list: &List) -> Vec<Value> {
    serde_json::from_str(&list.json).unwrap()
}

/// The one list `text`'s filters make, as JSON, and what it left out.
fn one(text: &str, features: Features) -> (Vec<Value>, BTreeMap<&'static str, usize>) {
    let lists = lists_of(text, "", features);
    assert!(
        lists.lists.len() <= 1,
        "{} lists for {text:?}",
        lists.lists.len()
    );
    let rules = lists.lists.first().map(rules_in).unwrap_or_default();
    (rules, lists.counts.left_out)
}

fn block(trigger: Value) -> Value {
    json!({"trigger": trigger, "action": {"type": "block"}})
}

fn allow(trigger: Value) -> Value {
    json!({"trigger": trigger, "action": {"type": "ignore-previous-rules"}})
}

fn hide(trigger: Value, selector: &str) -> Value {
    json!({"trigger": trigger, "action": {"type": "css-display-none", "selector": selector}})
}

/// Every type of a request a page makes, which a filter without types is for.
const SUBRESOURCES: &[&str] = &[
    "fetch",
    "font",
    "image",
    "media",
    "other",
    "ping",
    "raw",
    "script",
    "style-sheet",
    "svg-document",
    "websocket",
];

#[test]
fn a_filter_carries_over_with_its_address_types_party_and_sites() {
    let host = r"^[^:]+://+([^:/]+\.)?";
    let cases: &[(&str, &[Value])] = &[
        (
            "||ads.example^",
            &[block(
                json!({"url-filter": format!(r"{host}ads\.example[:/?]"), "resource-type": SUBRESOURCES}),
            )],
        ),
        (
            "||ads.example/x^$script,3p",
            &[
                block(json!({
                    "url-filter": format!(r"{host}ads\.example\/x{SEPARATOR}"),
                    "resource-type": ["script"], "load-type": ["third-party"],
                })),
                block(json!({
                    "url-filter": format!(r"{host}ads\.example\/x$"),
                    "resource-type": ["script"], "load-type": ["third-party"],
                })),
            ],
        ),
        (
            "|https://cdn.example/ad*.js|$image,css,1p",
            &[block(json!({
                "url-filter": r"^https:\/\/cdn\.example\/ad.*\.js$",
                "resource-type": ["image", "style-sheet", "svg-document"], "load-type": ["first-party"],
            }))],
        ),
        (
            "/banner/*/ad.$xhr,domain=news.example|blog.example",
            &[block(json!({
                "url-filter": r"\/banner\/.*\/ad\.",
                "resource-type": ["fetch"],
                "if-frame-url": [format!(r"{host}blog\.example[:/]"), format!(r"{host}news\.example[:/]")],
            }))],
        ),
        (
            "/^https?:\\/\\/[a-z]+\\.example\\/ad/$script",
            &[block(
                json!({"url-filter": r"^https?:\/\/[a-z]+\.example\/ad", "resource-type": ["script"]}),
            )],
        ),
        (
            "@@||ads.example/ok.js",
            &[allow(
                json!({"url-filter": format!(r"{host}ads\.example\/ok\.js"), "resource-type": SUBRESOURCES}),
            )],
        ),
        (
            // adblock-rust reads match-case only on a regular expression.
            "/Ad\\.JS/$match-case,font",
            &[block(
                json!({"url-filter": r"Ad\.JS", "url-filter-is-case-sensitive": true, "resource-type": ["font"]}),
            )],
        ),
        (
            "*$image,to=img.example",
            &[block(
                json!({"url-filter": format!(r"{host}img\.example[:/]"), "resource-type": ["image", "svg-document"]}),
            )],
        ),
    ];
    for (filter, expected) in cases {
        let (rules, left_out) = one(filter, ALL);
        assert!(left_out.is_empty(), "{filter} left out: {left_out:?}");
        let mut rules = rules;
        let mut expected = expected.to_vec();
        let key = |rule: &Value| rule.to_string();
        rules.sort_by_key(key);
        expected.sort_by_key(key);
        assert_eq!(rules, expected, "{filter}");
    }
}

#[test]
fn a_filter_webkit_cant_hold_exactly_is_left_out_and_counted() {
    let none = Features::default();
    let cases: &[(&str, Features, &str)] = &[
        (
            "||x.example^$method=post",
            none,
            "method= on WebKit that ignores request-method",
        ),
        (
            "||x.example^$method=get|post",
            ALL,
            "more than one method= (request-method takes one)",
        ),
        (
            "||x.example^$domain=~a.example",
            none,
            "domain=~ on WebKit without unless-frame-url",
        ),
        (
            "||x.example^$domain=a.example|~b.a.example",
            ALL,
            "domain= with sites in and out (a rule has one condition)",
        ),
        (
            "||x.example^$domain=a.*",
            ALL,
            "a site.* or a pattern for a site",
        ),
        (
            "||x.example^$script,redirect=noopjs",
            ALL,
            "a stand-in (redirect=): WebKit can't answer with one",
        ),
        (
            "||x.example^$redirect-rule=noopjs",
            ALL,
            "a stand-in (redirect=): WebKit can't answer with one",
        ),
        ("||x.example^$csp=script-src 'none'", ALL, "csp="),
        ("||x.example^$removeparam=utm", ALL, "removeparam="),
        (
            "||x.example^$tag=extra",
            ALL,
            "a tag= filter, off in ilo's engine",
        ),
        ("||x.example^$1p,3p", ALL, "neither party (1p,3p)"),
        (
            "||x.example^$document",
            ALL,
            "only pages or frames: the navigation check's",
        ),
        (
            "||x.example^$subdocument",
            ALL,
            "only pages or frames: the navigation check's",
        ),
        (
            "||x.example^$popup",
            ALL,
            "a pop-up filter: the navigation check's",
        ),
        (
            "/(ads|banners)\\/x/",
            ALL,
            "a regular expression WebKit doesn't read",
        ),
        (
            "/ad\\d+\\.js/",
            ALL,
            "a regular expression WebKit doesn't read",
        ),
        ("||x.example/ü^", ALL, "a non-ASCII address"),
        (
            "@@||x.example/path^$document",
            ALL,
            "a page exception for more than a site",
        ),
        ("example.com##+js(nobab)", ALL, "a scriptlet (+js)"),
        (
            "example.com##.ad:style(height: 0)",
            ALL,
            "a style or remove action",
        ),
        (
            "example.com##.ad:has-text(Sponsored)",
            ALL,
            "a procedural selector",
        ),
        ("example.*##.ad", ALL, "a site.* for hiding"),
        (
            "a.example,~b.a.example##.ad",
            ALL,
            "hiding with sites in and out (a rule has one condition)",
        ),
        (
            "||x.example^$elemhide",
            ALL,
            "a network filter adblock-rust refuses",
        ),
    ];
    for (filter, features, why) in cases {
        let (rules, left_out) = one(filter, *features);
        assert!(rules.is_empty(), "{filter} made {rules:?}");
        assert_eq!(left_out, BTreeMap::from([(*why, 1)]), "{filter}");
    }
}

#[test]
fn a_field_the_running_webkit_acts_on_is_written_only_then() {
    let filters = "||x.example^$method=post\n||y.example^$domain=~a.example\n";
    let (rules, _) = one(filters, Features::default());
    assert!(rules.is_empty(), "{rules:?}");
    let (rules, left_out) = one(filters, ALL);
    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0]["trigger"]["request-method"], "post", "{rules:?}");
    assert!(
        rules[1]["trigger"]["unless-frame-url"].is_array(),
        "{rules:?}"
    );
}

#[test]
fn a_badfilter_takes_its_filter_out_of_every_list() {
    let lists = lists_of(
        "||ads.example^\n||ads.example^$script\n",
        "||ads.example^$badfilter\n",
        ALL,
    );
    let rules: Vec<Value> = lists.lists.iter().flat_map(rules_in).collect();
    assert_eq!(rules.len(), 1, "{rules:?}");
    assert_eq!(rules[0]["trigger"]["resource-type"], json!(["script"]));
}

#[test]
fn a_page_exception_turns_off_blocking_and_hiding_on_its_site() {
    let (rules, left_out) = one("##.ad\n||ads.example^\n@@||news.example^$document\n", ALL);
    assert!(left_out.is_empty(), "{left_out:?}");
    // After the rules it undoes: WebKit's ignore-previous-rules undoes those before it.
    assert_eq!(
        rules,
        [
            hide(json!({"url-filter": ".*"}), ".ad"),
            block(
                json!({"url-filter": r"^[^:]+://+([^:/]+\.)?ads\.example[:/?]", "resource-type": SUBRESOURCES})
            ),
            allow(json!({"url-filter": ".*", "if-domain": ["*news.example"]})),
        ]
    );
}

#[test]
fn hiding_shares_rules_and_follows_its_exceptions() {
    let filters = "\
##.ad
##.banner
~shop.example##.promo
news.example,blog.example##.sponsor
news.example##.sponsor-box
example.com##.side
#@#.banner
quiet.example#@#.promo
sub.example.com#@#.side
@@||plain.example^$generichide
";
    let (rules, left_out) = one(filters, ALL);
    // adblock-rust refuses `#@#.banner`, so ilo's engine hides it everywhere.
    assert_eq!(
        left_out,
        BTreeMap::from([
            ("a hiding filter adblock-rust refuses", 1),
            ("hiding turned off on part of its site", 1)
        ])
    );
    let mut rules = rules;
    rules.sort_by_key(|rule| rule.to_string());
    let mut expected = vec![
        hide(
            json!({"url-filter": ".*", "unless-domain": ["*plain.example"]}),
            ".ad, .banner",
        ),
        hide(
            json!({"url-filter": ".*", "unless-domain": ["*plain.example", "*quiet.example", "*shop.example"]}),
            ".promo",
        ),
        hide(
            json!({"url-filter": ".*", "if-domain": ["*blog.example", "*news.example"]}),
            ".sponsor",
        ),
        hide(
            json!({"url-filter": ".*", "if-domain": ["*news.example"]}),
            ".sponsor-box",
        ),
    ];
    expected.sort_by_key(|rule| rule.to_string());
    assert_eq!(rules, expected);
}

#[test]
fn selectors_for_the_same_sites_share_a_rule_a_hundred_at_a_time() {
    let filters: String = (0..250).map(|n| format!("##.ad-{n}\n")).collect();
    let (rules, _) = one(&filters, ALL);
    let counts: Vec<usize> = rules
        .iter()
        .map(|rule| {
            rule["action"]["selector"]
                .as_str()
                .unwrap()
                .split(", ")
                .count()
        })
        .collect();
    assert_eq!(counts, [100, 100, 50]);
}

#[test]
fn a_hosts_list_blocks_each_host() {
    let sources = vec![Source {
        text: "# a hosts file\n0.0.0.0 tracker.example\n127.0.0.1 ads.example\n".into(),
        options: ParseOptions {
            format: FilterFormat::Hosts,
            ..Default::default()
        },
        user: false,
    }];
    let lists = from_sources(sources, ALL);
    let rules: Vec<Value> = lists.lists.iter().flat_map(rules_in).collect();
    let urls: Vec<&str> = rules
        .iter()
        .map(|rule| rule["trigger"]["url-filter"].as_str().unwrap())
        .collect();
    assert_eq!(lists.counts.filters, 2);
    assert_eq!(
        urls,
        [
            r"^[^:]+://+([^:/]+\.)?tracker\.example[:/?]",
            r"^[^:]+://+([^:/]+\.)?ads\.example[:/?]"
        ]
    );
}

#[test]
fn every_list_has_every_exception_and_important_blocks_after_them() {
    let mut text: String = (0..30_000)
        .map(|n| format!("||ads{n}.example^\n"))
        .collect();
    text.push_str("@@||ads7.example/ok.js\n||must.example^$important\n");
    let lists = lists_of(&text, "", ALL);
    assert!(lists.lists.len() >= 3, "{} lists", lists.lists.len());
    let mut blocks = 0;
    for list in &lists.lists {
        assert!(
            list.rules <= LIST_RULES + LIST_RULES / 10,
            "{} rules in a list",
            list.rules
        );
        assert_eq!(list.exceptions, 1);
        let rules = rules_in(list);
        let at = |kind: &str| rules.iter().position(|rule| rule["action"]["type"] == kind);
        let exception = at("ignore-previous-rules").expect("the exception");
        assert!(
            rules[..exception]
                .iter()
                .all(|rule| rule["action"]["type"] == "block")
        );
        for rule in &rules[exception + 1..] {
            assert!(
                rule["trigger"]["url-filter"]
                    .as_str()
                    .unwrap()
                    .contains("must"),
                "{rule} after the exception"
            );
        }
        blocks += rules.len() - 1;
    }
    assert_eq!(blocks, 30_001);
    assert_eq!(lists.counts.rules, 30_001 + lists.lists.len());
}

#[test]
fn a_change_to_the_users_filters_compiles_only_their_list_unless_its_an_exception() {
    let text: String = (0..20_000)
        .map(|n| format!("||ads{n}.example^\n"))
        .collect();
    let names = |user: &str| -> Vec<(String, bool)> {
        let lists = lists_of(&text, user, ALL);
        lists
            .lists
            .into_iter()
            .map(|list| (list.name, list.user))
            .collect()
    };
    let before = names("||mine.example^\n");
    let after = names("||mine.example^\n||also-mine.example^\n");
    assert_eq!(before.iter().filter(|(_, user)| *user).count(), 1);
    let kept = |from: &[(String, bool)]| -> Vec<String> {
        from.iter()
            .filter(|(_, user)| !user)
            .map(|(name, _)| name.clone())
            .collect()
    };
    assert_eq!(kept(&before), kept(&after));
    assert_ne!(before.last(), after.last());
    let excepted = names("||mine.example^\n@@||ads3.example^\n");
    assert!(
        kept(&before)
            .iter()
            .all(|name| !kept(&excepted).contains(name))
    );
}

#[test]
fn the_same_filters_make_the_same_lists() {
    let text: String = (0..20_000)
        .map(|n| format!("||ads{n}.example^\n##.ad-{n}\n"))
        .collect();
    let names = || -> Vec<String> {
        lists_of(&text, "", ALL)
            .lists
            .into_iter()
            .map(|list| list.name)
            .collect()
    };
    assert_eq!(names(), names());
}

#[test]
fn webkit_reads_only_regular_expressions_it_holds_as_written() {
    for regex in [
        r"^https?:\/\/ads\.",
        r"[a-z0-9]+\.js$",
        r"\/ad[^/]*\.gif",
        r"(ad)?banner",
    ] {
        assert!(webkit_reads(regex), "{regex}");
    }
    for regex in [
        r"a|b", r"ad{2}", r"\d+", r"(?:ad)", r"ad.*?x", r"a^b", r"a$b", r"[ab", r"\1", "é",
    ] {
        assert!(!webkit_reads(regex), "{regex}");
    }
}
