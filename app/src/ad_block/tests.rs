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
