use super::*;

const SITE: &str = "/* example.com: your CSS for its pages. */\nbody { margin: 0; }\n";

/// The default writes nothing, and a file without a block reads as the default.
#[test]
fn the_default_writes_no_block() {
    assert_eq!(Style::default().block(), None);
    assert_eq!(Style::default().into_file(SITE), SITE);
    assert_eq!(Style::of(SITE), Style::default());
}

/// What the block writes reads back as the same edits.
#[test]
fn a_block_reads_back_as_its_edits() {
    let style = Style {
        ground: Some(Dot { x: 0.25, y: -0.5 }),
        links: Some(Dot {
            x: -0.125,
            y: 0.375,
        }),
        invert: true,
        face: Some(Face::Serif),
        size: 120,
        case: Some(Case::Upper),
        adjust: [110, 100, 80],
        advanced: true,
        dragged: true,
        name: Some("Reading; mode".into()),
    };
    let file = style.into_file(SITE);
    assert_eq!(Style::of(&file), style);
    let block = style.block().unwrap();
    assert!(block.starts_with(START) && block.ends_with(END), "{block}");
    for line in [
        "font-family: ui-serif, serif !important;",
        "text-transform: uppercase !important;",
        "html { font-size: 120% !important; }",
        "html { filter: invert(1) hue-rotate(180deg) contrast(110%) saturate(80%) !important; }",
        "a, a * { color: #",
    ] {
        assert!(block.contains(line), "{line} isn't in {block}");
    }
}

/// A change rewrites the block where it is, the user's lines and Zap's around it untouched, and
/// the default takes it out with its line.
#[test]
fn a_change_rewrites_the_block_alone() {
    let zap = "#ad { display: none !important; } /* Zap */\n";
    let bigger = Style {
        size: 110,
        ..Style::default()
    };
    let file = format!("{}{zap}", bigger.into_file(SITE));
    assert!(file.starts_with(SITE) && file.ends_with(zap), "{file}");
    let case = Style {
        case: Some(Case::Lower),
        ..bigger
    };
    let changed = case.into_file(&file);
    assert_eq!(changed.matches(START).count(), 1, "{changed}");
    assert!(
        changed.starts_with(SITE) && changed.ends_with(zap),
        "{changed}"
    );
    assert!(!changed.contains("font-size: 110%") || changed.contains("lowercase"));
    assert_eq!(Style::of(&changed), case);
    assert_eq!(Style::default().into_file(&changed), format!("{SITE}{zap}"));
}

/// The ground's text is black or white, whichever reads, and the page size stays in Arc's bounds
/// however the file was edited.
#[test]
fn text_reads_on_the_ground_and_sizes_stay_in_bounds() {
    assert_eq!(ink(0xffffff), 0x16140f);
    assert_eq!(ink(0x000000), 0xf3f0e8);
    assert_eq!(ink(0x3366cc), 0xf3f0e8);
    assert_eq!(ink(0xffd54a), 0x16140f);
    let file = format!("{START}\n/* size 400 */\n{END}\n");
    assert_eq!(Style::of(&file).size, LARGEST);
}

/// Shuffle picks two colours, a face this computer has and a case, and keeps the page size.
#[test]
fn shuffle_keeps_the_size_and_offers_only_faces_here() {
    let start = Style {
        size: 130,
        ..Style::default()
    };
    let mut looks = Vec::new();
    for seed in 1..40 {
        let shuffled = start.shuffled(seed);
        assert_eq!(shuffled.size, 130);
        let links = shuffled.links.unwrap();
        assert!(links.x.hypot(links.y) <= 0.75 + 1e-4, "{links:?}");
        assert!(shuffled.face.is_none_or(Face::resolves), "{shuffled:?}");
        looks.push(shuffled);
    }
    looks.dedup();
    assert!(looks.len() > 30, "Shuffle hardly changes");
}

/// A name keeps to one line and can't end the comment that holds it.
#[test]
fn a_name_stays_in_its_comment() {
    assert_eq!(named("  News */ body {  "), Some("News  body {".into()));
    assert_eq!(named("a\nb"), Some("a b".into()));
    assert_eq!(named(" */ "), None);
    let style = Style {
        name: named("Quiet */ news"),
        ..Style::default()
    };
    assert_eq!(Style::of(&style.into_file(SITE)), style);
}

/// Advanced's sliders filter the page by themselves, without Invert lightness, and a file edited
/// by hand still reads as values they can show: in their range, on their step.
#[test]
fn advanced_filters_the_page_and_reads_back_on_its_steps() {
    let mut style = Style::default();
    Adjust::Brightness.set(&mut style, 90);
    let block = style.block().unwrap();
    assert!(
        block.contains("html { filter: brightness(90%) !important; }"),
        "{block}"
    );
    assert!(!block.contains("img, video"), "{block}");
    let file =
        format!("{START}\n/* contrast 400; brightness 73; saturate -20; advanced */\n{END}\n");
    let read = Style::of(&file);
    assert_eq!(read.adjust, [150, 75, 0]);
    assert!(read.advanced);
}

/// Advanced left open is kept with the Boost, though it changes nothing on the page.
#[test]
fn advanced_open_is_kept_alone() {
    let open = Style {
        advanced: true,
        ..Style::default()
    };
    let block = open.block().unwrap();
    assert!(!block.contains("filter"), "{block}");
    assert_eq!(Style::of(&open.into_file(SITE)), open);
}
