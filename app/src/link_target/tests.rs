use gpui::{Bounds, point, px, size};

use super::{HEIGHT, MARGIN, NEAR, PAD_X, Side, Target, near, place, plate};

/// Every character 7 points wide, so a test counts characters.
fn measure(text: &str) -> f32 {
    7. * text.chars().count() as f32
}

/// A link whose site is the part of `text` that reads `site`.
fn target(text: &str, site: &str) -> Target {
    let start = text.find(site).unwrap();
    Target {
        text: text.to_string().into(),
        domain: start..start + site.len(),
    }
}

#[test]
fn an_address_that_fits_is_whole_with_its_site_apart() {
    let plate = plate(
        &target("www.example.com/docs", "example.com"),
        480.,
        measure,
    );
    assert_eq!(
        (
            plate.before.as_ref(),
            plate.site.as_ref(),
            plate.after.as_ref()
        ),
        ("www.", "example.com", "/docs")
    );
    assert_eq!(plate.width, measure("www.example.com/docs") + 2. * PAD_X);
}

#[test]
fn a_long_path_loses_its_middle_and_the_site_stays() {
    let path = "/a/very/long/path/that/goes/on/and/on/and/ends/in/report-final.pdf";
    let text = format!("login.example.co.uk{path}");
    let plate = plate(&target(&text, "example.co.uk"), 240., measure);
    // 240 less 2 × 10 of padding takes 31 characters.
    assert!(plate.width <= 240.);
    assert_eq!(plate.before.as_ref(), "login.");
    assert_eq!(plate.site.as_ref(), "example.co.uk");
    assert!(
        plate.after.starts_with("/a/"),
        "the path's start stays: {}",
        plate.after
    );
    assert!(plate.after.contains('…'), "{}", plate.after);
    assert!(
        plate.after.ends_with(".pdf"),
        "the file's end stays: {}",
        plate.after
    );
    assert!(plate.text().chars().count() <= 31);
}

#[test]
fn subdomains_go_from_the_left_before_the_site_does() {
    let text = "accounts.security.verification.example.com/";
    let plate = plate(&target(text, "example.com"), 200., measure);
    assert_eq!(plate.site.as_ref(), "example.com");
    assert!(plate.before.starts_with('…'), "{}", plate.before);
    assert!(plate.before.ends_with("verification."), "{}", plate.before);
    assert_eq!(plate.after.as_ref(), "");
    assert!(plate.width <= 200.);
}

#[test]
fn a_site_wider_than_the_plate_is_cut_at_its_end() {
    let host = "an-extremely-long-name.example.com";
    let plate = plate(&target(host, host), 120., measure);
    assert_eq!(plate.before.as_ref(), "");
    assert!(plate.site.ends_with('…'), "{}", plate.site);
    assert!(plate.width <= 120.);
}

#[test]
fn an_address_with_no_host_has_no_site() {
    let plate = plate(
        &Target {
            text: "mailto:someone@example.com".into(),
            domain: 0..0,
        },
        480.,
        measure,
    );
    assert_eq!(plate.site.as_ref(), "");
    assert_eq!(plate.text(), "mailto:someone@example.com");
    // A range that isn't in the text, or splits a character, is no site either.
    let wrong = Target {
        text: "日本語.jp/".into(),
        domain: 1..4,
    };
    assert_eq!(super::plate(&wrong, 480., measure).text(), "日本語.jp/");
    let past = Target {
        text: "a.com".into(),
        domain: 2..40,
    };
    assert_eq!(super::plate(&past, 480., measure).text(), "a.com");
}

#[test]
fn a_path_in_other_scripts_is_cut_on_characters() {
    let text = "例.jp/日本語の長いパス/日本語の長いパス/日本語の長いパス/ファイル.html";
    let plate = plate(&target(text, "例.jp"), 180., measure);
    assert!(plate.after.contains('…'));
    assert!(plate.after.ends_with(".html"));
    assert!(plate.width <= 180.);
}

#[test]
fn a_huge_address_is_cut_before_it_is_measured() {
    let text = format!("example.com/{}", "x".repeat(200_000));
    let measured = std::cell::Cell::new(0);
    let plate = plate(&target(&text, "example.com"), 480., |text| {
        measured.set(measured.get().max(text.chars().count()));
        measure(text)
    });
    assert!(
        measured.get() < 1000,
        "measured a string of {}",
        measured.get()
    );
    assert!(plate.after.contains('…'));
}

fn card() -> Bounds<gpui::Pixels> {
    Bounds::new(point(px(228.), px(10.)), size(px(762.), px(600.)))
}

#[test]
fn the_plate_sits_inside_the_cards_bottom_corners() {
    let left = place(card(), 200., Side::Left);
    assert_eq!(f32::from(left.left()), 228. + MARGIN);
    assert_eq!(f32::from(left.bottom()), 610. - MARGIN);
    assert_eq!(f32::from(left.size.height), HEIGHT);
    let right = place(card(), 200., Side::Right);
    assert_eq!(f32::from(right.right()), 990. - MARGIN);
    assert_eq!(f32::from(right.bottom()), f32::from(left.bottom()));
}

#[test]
fn the_pointer_near_the_plate_is_near_and_far_from_it_is_not() {
    let frame = place(card(), 200., Side::Left);
    let (x, y) = (f32::from(frame.left()), f32::from(frame.top()));
    assert!(near(frame, x + 50., y + 5.));
    assert!(near(frame, x - NEAR + 1., y - NEAR + 1.));
    assert!(!near(frame, x - NEAR - 1., y + 5.));
    assert!(!near(frame, x + 50., y - NEAR - 1.));
    assert!(!near(frame, x + 500., y + 5.));
}
