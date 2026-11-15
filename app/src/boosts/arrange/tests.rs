use super::*;

const FILE: &str = r#"{"version": 1, "changes": [
  {"kind": "settle", "name": "", "role": "video", "before": {"name": "", "role": "article", "at": [3, 1]}, "place": "x"},
  {"kind": "trade", "name": "Trending", "role": "aside", "with": {"name": "Get the newsletter", "role": "form"}},
  {"kind": "float", "name": "More like this", "role": "complementary", "pin": "screen", "box": [10, 20, 300, 200]},
  {"kind": "settle", "name": "", "role": "div", "after": {"name": "", "role": "contentinfo"}}
], "core": {"seen": 4}}"#;

/// A thing is its page's words, quoted, or the app's word for its kind where it has none.
#[test]
fn each_change_reads_as_the_card_lists_it() {
    let listed: Vec<(String, String)> = (read(FILE).unwrap().iter())
        .map(|change| (change.title(), change.describe()))
        .collect();
    let listed: Vec<(&str, &str)> = (listed.iter())
        .map(|(name, what)| (name.as_str(), what.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            ("Video", "Before the article"),
            ("Trending", "Traded with “Get the newsletter”"),
            ("More like this", "Floats, stays on screen"),
            ("Part of the page", "After the footer"),
        ]
    );
}

/// Putting one back drops that entry alone, and what the app doesn't read stays as it was.
#[test]
fn putting_one_back_keeps_the_rest_as_it_was() {
    let left = without(FILE, 1).unwrap();
    let names: Vec<String> = read(&left)
        .unwrap()
        .into_iter()
        .map(|each| each.title())
        .collect();
    assert_eq!(names, ["Video", "More like this", "Part of the page"]);
    let left: Value = serde_json::from_str(&left).unwrap();
    assert_eq!(
        left["changes"][0]["before"]["at"],
        serde_json::json!([3, 1])
    );
    assert_eq!(left["changes"][0]["place"], "x");
    assert_eq!(
        left["changes"][1]["box"],
        serde_json::json!([10, 20, 300, 200])
    );
    assert_eq!(left["core"]["seen"], 4);
    assert_eq!(without(FILE, 4), None, "an index past the end dropped one");
}

/// A file someone else wrote: over any cap or off its shape, it's refused whole, and nothing in
/// it panics.
#[test]
fn a_hostile_file_is_refused_whole() {
    let settle = |name: &str| format!(r#"{{"kind":"float","name":"{name}","pin":"page"}}"#);
    let file =
        |changes: Vec<String>| format!(r#"{{"version":1,"changes":[{}]}}"#, changes.join(","));
    assert_eq!(
        read(&file(vec![settle("a"); MAX_CHANGES])).map(|each| each.len()),
        Some(64)
    );
    let long = "a".repeat(MAX_STRING + 1);
    let deep = "[".repeat(MAX_BYTES / 2) + &"]".repeat(MAX_BYTES / 2);
    let refused = [
        file(vec![settle("a"); MAX_CHANGES + 1]),
        file(vec![settle(&long)]),
        // A long string the app doesn't read is still over the cap.
        format!(r#"{{"version":1,"changes":[],"x":"{long}"}}"#),
        format!(r#"{{"version":1,"changes":[],"{long}":1}}"#),
        format!(
            r#"{{"version":1,"changes":[],"pad":"{}"}}"#,
            " ".repeat(MAX_BYTES)
        ),
        // Of one change off its shape, none is listed.
        file(vec![settle("a"), r#"{"kind":"resize","name":"b"}"#.into()]),
        file(vec![
            r#"{"kind":"settle","name":"a","before":{"name":"b"},"after":{"name":"c"}}"#.into(),
        ]),
        file(vec![r#"{"kind":"settle","name":"a"}"#.into()]),
        file(vec![r#"{"kind":"trade","name":"a","with":"b"}"#.into()]),
        file(vec![r#"{"kind":"float","name":"a","pin":"window"}"#.into()]),
        file(vec![r#"{"kind":"float","name":7,"pin":"page"}"#.into()]),
        file(vec![
            r#"{"kind":"float","name":"a","role":1,"pin":"page"}"#.into(),
        ]),
        file(vec!["null".into()]),
        r#"{"version":2,"changes":[]}"#.into(),
        r#"{"version":"1","changes":[]}"#.into(),
        r#"{"version":1,"changes":{}}"#.into(),
        r#"{"changes":[]}"#.into(),
        r#"[]"#.into(),
        r#"{"version":1,"changes":[]"#.into(),
        "\u{feff}{}".into(),
        deep,
        String::new(),
    ];
    for bytes in refused {
        let start: String = bytes.chars().take(80).collect();
        assert_eq!(read(&bytes), None, "read {start}");
        assert_eq!(without(&bytes, 0), None, "dropped from {start}");
    }
}

/// Names reach the card as text, whatever they hold.
#[test]
fn a_name_is_text_whatever_it_holds() {
    // JSON's escapes, of a right-to-left override and a null.
    let name = "<img src=x onerror=alert(1)> \\u202e\\u0000 ✓";
    let bytes = format!(
        r#"{{"version":1,"changes":[{{"kind":"trade","name":"{name}","with":{{"name":"b"}}}}]}}"#
    );
    let read = read(&bytes).unwrap();
    assert_eq!(
        read[0].thing.name,
        "<img src=x onerror=alert(1)> \u{202e}\u{0} ✓"
    );
}
