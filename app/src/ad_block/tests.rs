use super::*;

/// A folder of its own for a test, laid out as uBlock Origin's: a list of its own, a list of a
/// third party's, its redirect table and a stand-in.
fn folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("ilo-ad-block-{name}-{}", std::process::id()));
    _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(folder.join("assets/ublock")).unwrap();
    fs::create_dir_all(folder.join("assets/thirdparties")).unwrap();
    fs::create_dir_all(folder.join("js")).unwrap();
    fs::create_dir_all(folder.join("web_accessible_resources")).unwrap();
    fs::write(
        folder.join("js/redirect-resources.js"),
        "export default new Map([\n    [ '1x1.gif', {\n        alias: '1x1-transparent.gif',\n        \
         data: 'blob',\n    } ],\n    [ 'noop.js', {\n        alias: [ 'noopjs', 'abp-resource:blank-js' ],\n        \
         data: 'text',\n    } ],\n    [ 'missing.js', {\n    } ],\n]);\n",
    )
    .unwrap();
    fs::write(
        folder.join("web_accessible_resources/noop.js"),
        "(function() {})();",
    )
    .unwrap();
    fs::write(folder.join("web_accessible_resources/1x1.gif"), b"GIF89a").unwrap();
    fs::write(
        folder.join("assets/ublock/filters.txt"),
        "||ads.example^\n||tracker.example^$script,redirect=noopjs\n@@||ads.example/ok.js\n",
    )
    .unwrap();
    fs::write(
        folder.join("assets/thirdparties/hosts.txt"),
        "0.0.0.0 hosted.example\n0.0.0.0 other.example\n",
    )
    .unwrap();
    folder
}

fn setup(folder: &Path, filters: &str) -> Setup {
    Setup {
        folder: folder.to_owned(),
        lists: vec![
            folder.join("assets/ublock/filters.txt"),
            folder.join("assets/thirdparties/hosts.txt"),
            folder.join("assets/thirdparties/gone.txt"),
        ],
        filters: filters.into(),
    }
}

#[test]
fn the_table_names_each_stand_in_with_its_aliases() {
    let source = fs::read_to_string(folder("table").join("js/redirect-resources.js")).unwrap();
    assert_eq!(
        redirect_table(&source),
        [
            ("1x1.gif".into(), vec!["1x1-transparent.gif".into()]),
            (
                "noop.js".into(),
                vec!["noopjs".into(), "abp-resource:blank-js".into()]
            ),
            ("missing.js".into(), vec![]),
        ]
    );
}

#[test]
fn the_lists_and_filters_block_except_where_an_exception_lets_go() {
    let folder = folder("check");
    let engine = compile(&setup(&folder, "||mine.example^\n"));
    let page = "https://news.example/";
    let verdict = |url: &str, kind: &str| check(&engine, url, page, kind);
    assert_eq!(
        verdict("https://ads.example/a.js", "script"),
        Verdict::Block
    );
    assert_eq!(
        verdict("https://ads.example/ok.js", "script"),
        Verdict::Allow
    );
    // A hosts file's line, as uBlock Origin reads it.
    assert_eq!(
        verdict("https://hosted.example/x.png", "image"),
        Verdict::Block
    );
    // The user's own.
    assert_eq!(
        verdict("https://mine.example/x", "xmlhttprequest"),
        Verdict::Block
    );
    assert_eq!(
        verdict("https://news.example/story", "document"),
        Verdict::Allow
    );
    // Not a request adblock-rust can read.
    assert_eq!(check(&engine, "not a url", page, "script"), Verdict::Allow);
}

#[test]
fn a_bare_hostname_blocks_its_host_and_its_pages_as_ublock_origin_reads_it() {
    let folder = folder("hostnames");
    let malware = folder.join("assets/thirdparties/malware.txt");
    fs::write(
        &malware,
        "! Title: URLhaus\n1.2.3.44\nBad.example\n@@ok.bad.example\n",
    )
    .unwrap();
    let mut setup = setup(&folder, "mine.example\n");
    setup.lists.push(malware);
    let engine = compile(&setup);
    let page = "https://news.example/";
    let verdict = |url: &str, kind: &str| check(&engine, url, page, kind);
    assert_eq!(verdict("http://1.2.3.44/x.js", "script"), Verdict::Block);
    assert_eq!(verdict("http://11.2.3.44/x.js", "script"), Verdict::Allow);
    assert_eq!(
        verdict("https://cdn.bad.example/x.js", "script"),
        Verdict::Block
    );
    assert_eq!(
        verdict("https://ok.bad.example/x.js", "script"),
        Verdict::Allow
    );
    // The page itself, as a link opens it, from no page.
    let opened = |url: &str| check(&engine, url, url, "document");
    assert_eq!(opened("http://1.2.3.44/"), Verdict::Block);
    assert_eq!(opened("https://bad.example/login"), Verdict::Block);
    assert_eq!(opened("https://ok.bad.example/"), Verdict::Allow);
    assert_eq!(opened("https://mine.example/"), Verdict::Block);
    // A hosts file's, and a filter as EasyList writes one, block their pages as they did.
    assert_eq!(opened("https://hosted.example/"), Verdict::Block);
    assert_eq!(opened("https://ads.example/"), Verdict::Block);
    assert_eq!(opened("https://news.example/"), Verdict::Allow);
}

#[test]
fn only_a_whole_hostname_is_read_as_one() {
    for (pattern, hostname) in [
        ("example.com", true),
        ("1.2.3.44", true),
        // A last label of one character isn't, in uBlock Origin's pattern, so it stays a pattern.
        ("1.2.3.4", false),
        ("a_b.example-1.org", true),
        ("localhost", true),
        ("UPPER.Example", true),
        ("_chartbeat.js", false),
        ("-didomi.js", false),
        ("example.c", false),
        ("example.com_", false),
        ("example..com", false),
        (".example.com", false),
        ("||example.com^", false),
        ("example.com$doc", false),
        ("example.com/ads", false),
        ("example.com##.ad", false),
        ("", false),
    ] {
        assert_eq!(is_hostname(pattern), hostname, "{pattern}");
    }
}

#[test]
fn a_popup_filter_closes_the_pages_it_names_and_blocks_no_request() {
    let folder = folder("popups");
    let popups = folder.join("assets/thirdparties/popups.txt");
    fs::write(
        &popups,
        "! Title: EasyList\n||pop.example^$popup\n@@||pop.example/ok^$popup\n\
         ||both.example^$doc,popup\n||site.example^$popup,domain=news.example\n\
         $popup,third-party,domain=spam.example\npop-only.example$popup\n",
    )
    .unwrap();
    let mut setup = setup(&folder, "||mine.example^$popup\n");
    setup.lists.push(popups);
    let engine = compile(&setup);
    let news = "https://news.example/";
    let opened = |url: &str, from: &str| check(&engine, url, from, "popup");
    assert_eq!(opened("https://pop.example/x", news), Verdict::Block);
    assert_eq!(opened("https://pop.example/ok", news), Verdict::Allow);
    assert_eq!(opened("https://both.example/", news), Verdict::Block);
    assert_eq!(opened("https://site.example/", news), Verdict::Block);
    assert_eq!(
        opened("https://site.example/", "https://other.example/"),
        Verdict::Allow
    );
    assert_eq!(
        opened("https://any.example/", "https://spam.example/"),
        Verdict::Block
    );
    assert_eq!(opened("https://any.example/", news), Verdict::Allow);
    assert_eq!(
        opened("https://www.pop-only.example/", news),
        Verdict::Block
    );
    assert_eq!(opened("https://mine.example/", news), Verdict::Block);
    // A filter of no type blocks the requests it names, but never closes a pop-up, as in uBlock
    // Origin.
    assert_eq!(opened("https://ads.example/", news), Verdict::Allow);
    // A pop-up's filter blocks no request, nor the page opened from a link, but one that names a
    // type too blocks that.
    assert_eq!(
        check(&engine, "https://pop.example/x", news, "script"),
        Verdict::Allow
    );
    let page = |url: &str| check(&engine, url, url, "document");
    assert_eq!(page("https://pop.example/x"), Verdict::Allow);
    assert_eq!(page("https://both.example/"), Verdict::Block);
}

#[test]
fn a_popup_filter_keeps_its_other_types_and_the_popups_get_its_other_options() {
    let (rest, popups) = popups_apart(
        "||a.example^$popup\n@@||b.example^$popup,domain=x.example\n||c.example^$doc,popup,3p\n\
         $popup,third-party\nexample.com##.ad[href$=popup]\n||d.example^$script\n"
            .to_string(),
    );
    assert_eq!(
        rest,
        "||c.example^$doc,3p\nexample.com##.ad[href$=popup]\n||d.example^$script\n"
    );
    assert_eq!(
        popups,
        "||a.example^\n@@||b.example^$domain=x.example\n||c.example^$3p\n*$third-party\n"
    );
}

#[test]
fn a_redirect_answers_with_its_stand_in_and_removeparam_rewrites() {
    let folder = folder("redirect");
    let engine = compile(&setup(&folder, "$removeparam=utm_source\n"));
    let page = "https://news.example/";
    let Verdict::Redirect(stand_in) =
        check(&engine, "https://tracker.example/t.js", page, "script")
    else {
        panic!("the request wasn't redirected");
    };
    assert_eq!(
        stand_in,
        format!(
            "data:application/javascript;base64,{}",
            BASE64_STANDARD.encode("(function() {})();")
        )
    );
    assert_eq!(
        check(
            &engine,
            "https://shop.example/item?utm_source=mail&id=4",
            page,
            "xmlhttprequest"
        ),
        Verdict::Rewrite("https://shop.example/item?id=4".into())
    );
}

#[test]
fn the_next_launch_loads_the_cache_and_a_changed_list_compiles_again() {
    let folder = folder("cache");
    let cache = folder.join("cache");
    let first = setup(&folder, "");
    let compiled = load(&first, Some(&cache));
    let cached: Vec<_> = fs::read_dir(&cache).unwrap().flatten().collect();
    assert_eq!(cached.len(), 1);
    let loaded = load(&first, Some(&cache));
    let page = "https://news.example/";
    for engine in [&compiled, &loaded] {
        assert_eq!(
            check(engine, "https://ads.example/a.js", page, "script"),
            Verdict::Block
        );
        assert!(matches!(
            check(engine, "https://tracker.example/t.js", page, "script"),
            Verdict::Redirect(_)
        ));
    }
    // A list that changed names another, and the last goes.
    fs::write(
        folder.join("assets/ublock/filters.txt"),
        "||other.example^\n",
    )
    .unwrap();
    let changed = load(&first, Some(&cache));
    assert_eq!(
        check(&changed, "https://ads.example/a.js", page, "script"),
        Verdict::Allow
    );
    assert_eq!(fs::read_dir(&cache).unwrap().flatten().count(), 1);
}

#[test]
fn a_request_waits_for_the_engine_asked_for() {
    let folder = folder("blocker");
    let blocker = Arc::new(Blocker::new());
    blocker.use_setup(setup(&folder, ""), None);
    let engine = blocker.engine().expect("the engine, once loaded");
    let page = "https://news.example/";
    assert_eq!(
        check(&engine, "https://ads.example/a.js", page, "script"),
        Verdict::Block
    );
    // A setup with the user's filter: the next request waits for it.
    blocker.use_setup(setup(&folder, "||mine.example^"), None);
    let engine = blocker.engine().unwrap();
    assert_eq!(
        check(&engine, "https://mine.example/x", page, "image"),
        Verdict::Block
    );
}

#[test]
fn a_page_hides_its_own_and_generic_selectors_and_runs_its_scriptlets() {
    let folder = folder("page");
    fs::create_dir_all(folder.join("js/resources")).unwrap();
    fs::write(
        folder.join("js/resources/set-constant.js"),
        "import { registerScriptlet } from './base.js';\nfunction setConstant(chain = '', value = '') {\n    \
         window[chain] = value === 'false' ? false : value;\n}\nregisterScriptlet(setConstant, {\n    \
         name: 'set-constant.js',\n    aliases: [ 'set.js' ],\n});\n",
    )
    .unwrap();
    let filters = "news.example##.sponsor\nnews.example##+js(set, adsShown, false)\n##.ad-banner\n\
                   ##.promo\nnews.example#@#.promo\n##[data-ad]\n";
    let engine = compile(&setup(&folder, filters));
    let answer: serde_json::Value =
        serde_json::from_str(&page(&engine, "https://news.example/today")).unwrap();
    // Its own, and the generic one that isn't a class or an id, which no element's classes find.
    assert_eq!(answer["hide"], serde_json::json!([".sponsor", "[data-ad]"]));
    assert_eq!(answer["generic"], true);
    let script = answer["script"].as_str().unwrap();
    assert!(script.starts_with("(function() {\n'use strict';\nconst scriptletGlobals = {};\n"));
    assert!(script.contains("function setConstant(chain = '', value = '') {"));
    assert!(
        script.contains("setConstant(\"adsShown\", \"false\")"),
        "{script}"
    );
    // The generic selectors that match its classes, but the one it makes an exception of.
    let exceptions: Vec<String> = (answer["exceptions"].as_array().unwrap().iter())
        .map(|each| each.as_str().unwrap().to_string())
        .collect();
    let classes = [
        "ad-banner".to_string(),
        "promo".to_string(),
        "story".to_string(),
    ];
    assert_eq!(
        hide(&engine, &classes, &[], &exceptions),
        r#"[".ad-banner"]"#
    );
    // Another site hides only the generic one of them, and runs nothing.
    let other: serde_json::Value =
        serde_json::from_str(&page(&engine, "https://other.example/")).unwrap();
    assert_eq!(other["hide"], serde_json::json!(["[data-ad]"]));
    assert_eq!(other["script"], "");
}

#[test]
fn a_pages_procedural_filters_run_isolated_and_a_plain_style_is_a_rule() {
    let folder = folder("procedural");
    let filters = "news.example##.post:has-text(Sponsored)\nnews.example##.x:style(height: 0 !important)\n\
                   news.example##.y:remove()\nnews.example##.ad:upward(2):style(opacity: 0)\n\
                   news.example##.z:others()\n";
    let engine = compile(&setup(&folder, filters));
    let answer: serde_json::Value =
        serde_json::from_str(&page(&engine, "https://news.example/today")).unwrap();
    let styles = answer["styles"].as_array().unwrap();
    assert!(
        styles.contains(&serde_json::json!([".x", "height: 0 !important"])),
        "{styles:?}"
    );
    // The upward one's style, by the attribute its script gives what it finds.
    assert_eq!(styles.len(), 2, "{styles:?}");
    let styled = styles
        .iter()
        .find(|style| style[1] == "opacity: 0")
        .unwrap()[0]
        .as_str()
        .unwrap();
    let hidden = answer["hide"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .as_str()
        .unwrap();
    for name in [hidden, styled] {
        assert!(name.starts_with("[x") && name.ends_with(']'), "{name}");
    }
    assert_ne!(hidden, styled);
    let script = answer["isolated"].as_str().unwrap();
    assert!(script.contains("const compileProceduralSelector = "));
    for part in ["\"has-text\"", "\"Sponsored\"", "\"upward\"", "\"remove\""] {
        assert!(script.contains(part), "{part} in the script");
    }
    assert!(script.contains(&styled[1..styled.len() - 1]));
    assert!(script.contains(&hidden[1..hidden.len() - 1]));
    // The plain style needs no script.
    assert!(!script.contains("height: 0"));
    // Another site has none of them.
    let other: serde_json::Value =
        serde_json::from_str(&page(&engine, "https://other.example/")).unwrap();
    assert_eq!(other["styles"], serde_json::json!([]));
    assert_eq!(other["isolated"], "");
}

#[test]
fn a_lists_conditions_keep_only_what_ublock_origin_on_chromium_loads() {
    let folder = folder("conditions");
    // One host a branch, each blocked only if its branch is kept.
    let list = "\
!#if env_chromium
||chromium.example^
!#endif
!#if env_firefox
||firefox.example^
!#else
||not-firefox.example^
!#endif
!#if !env_firefox
||negated.example^
!#endif
!#if cap_html_filtering
||html-filtering.example^
!#endif
!#if cap_ipaddress
||ipaddress.example^
!#endif
!#if cap_something_new
||new-cap.example^
!#endif
!#if something_unknown
||unknown.example^
!#else
||unknown-else.example^
!#endif
!#if env_safari
||safari.example^
!#endif
!#if env_mobile
||mobile.example^
!#endif
!#if env_mv3
||mv3.example^
!#endif
!#if ext_ubol
||ubol.example^
!#endif
!#if false
||false.example^
!#endif
!#if adguard_ext_opera
||opera.example^
!#endif
!#if (ext_ublock || adguard)
||either.example^
!#endif
!#if ext_ublock && !env_mobile
||both.example^
!#endif
!#if env_chromium
!#if env_mobile
||nested-mobile.example^
!#else
||nested-desktop.example^
!#endif
||nested-after.example^
!#endif
!#if env_firefox
!#if env_chromium
||inside-dropped.example^
!#endif
||after-inside-dropped.example^
!#endif
||outside.example^
";
    fs::write(folder.join("assets/ublock/filters.txt"), list).unwrap();
    let engine = compile(&setup(
        &folder,
        "!#if env_firefox\n||mine-firefox.example^\n!#endif\n",
    ));
    let blocks = |host: &str| {
        check(
            &engine,
            &format!("https://{host}/a.js"),
            "https://news.example/",
            "script",
        ) == Verdict::Block
    };
    let kept = [
        "chromium.example",
        "not-firefox.example",
        "negated.example",
        "ipaddress.example",
        "unknown.example",
        "unknown-else.example",
        "opera.example",
        "either.example",
        "both.example",
        "nested-desktop.example",
        "nested-after.example",
        "outside.example",
    ];
    let dropped = [
        "firefox.example",
        "html-filtering.example",
        "new-cap.example",
        "safari.example",
        "mobile.example",
        "mv3.example",
        "ubol.example",
        "false.example",
        "nested-mobile.example",
        "inside-dropped.example",
        "after-inside-dropped.example",
        "mine-firefox.example",
    ];
    for host in kept {
        assert!(
            blocks(host),
            "{host}'s branch holds on Chromium, so its filter should block"
        );
    }
    for host in dropped {
        assert!(
            !blocks(host),
            "{host}'s branch doesn't hold on Chromium, so its filter should be left out"
        );
    }
}

#[test]
fn a_condition_reads_as_the_preparser_reads_it() {
    // An unknown first token gives way to `||` and stays unknown through `&&`.
    assert_eq!(if_holds("unknown_token || env_chromium"), Some(true));
    assert_eq!(if_holds("unknown_token || env_firefox"), Some(false));
    assert_eq!(if_holds("unknown_token && env_chromium"), None);
    // Any later unknown token, or one not written as `op token`, makes it all unknown.
    assert_eq!(if_holds("env_chromium && unknown_token"), None);
    assert_eq!(if_holds("env_chromium &&env_mobile"), None);
    assert_eq!(if_holds("env_chromium env_mobile"), None);
    assert_eq!(if_holds("|| env_chromium"), None);
    assert_eq!(if_holds(""), None);
    // Left to right, without precedence.
    assert_eq!(
        if_holds("env_firefox && env_mobile || env_chromium"),
        Some(true)
    );
    assert_eq!(
        if_holds("env_chromium || env_firefox && env_mobile"),
        Some(false)
    );
    assert_eq!(if_holds("(!env_safari && !ext_abp)"), Some(true));
    assert_eq!(if_holds("!false"), Some(true));
    assert_eq!(if_holds("!cap_something_new"), Some(true));
    // `!#ifdef` and `!#include` aren't conditions; a list without one is left as it is.
    assert_eq!(if_directive("!#ifdef x"), None);
    assert_eq!(if_directive("!#include other.txt"), None);
    let plain = "||a.example^\n! comment\n".to_string();
    assert_eq!(if_evaluated(plain.clone()), plain);
}
