use std::rc::Rc;

use gpui::{Entity, Modifiers, Pixels, Point, TestAppContext, VisualTestContext, point, px};

use super::{Chooser, ChooserDevice, DeviceKind, Scan};
use crate::{
    settling::SETTLING,
    shell::{Folders, Question, Shell, recorder::Recorder},
    tokens::shell::cast::{ROW, ROW_GAP},
    tokens::shell::permission::PADDING,
};

/// One Space with tabs 10 and 11, tab 10 shown, in a window.
fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_selected(10, cx);
    });
    host.take();
    (shell, cx, host)
}

fn device(id: &str, name: &str, detail: &str) -> ChooserDevice {
    ChooserDevice {
        id: id.to_owned().into(),
        name: name.to_owned().into(),
        detail: detail.to_owned().into(),
    }
}

fn ports(devices: Vec<ChooserDevice>) -> Chooser {
    Chooser {
        site: "flasher.example.com".into(),
        kind: DeviceKind::Serial,
        devices,
        scan: Scan::Live,
    }
}

/// The middle of the list's row `index` of `rows`, counted up from the card's foot, as nothing
/// follows the list on a live one.
fn row(shell: &Shell, index: usize, rows: usize) -> Point<Pixels> {
    let card = shell.chooser_drawn.get().expect("the chooser wasn't drawn");
    let foot = card.origin.y + card.size.height - px(1. + PADDING);
    let up = (rows - 1 - index) as f32 * (ROW + ROW_GAP) + ROW / 2.;
    point(card.origin.x + card.size.width / 2., foot - px(up))
}

fn close(shell: &Shell) -> Point<Pixels> {
    let card = shell.chooser_drawn.get().expect("the chooser wasn't drawn");
    point(
        card.origin.x + card.size.width - px(18.),
        card.origin.y + px(18.),
    )
}

/// A page can time its request to come up under a double-click the user started, so for the
/// card's first moment a click on a device grants nothing, while × cancels at once. After it, one
/// click grants, and the card goes.
#[gpui::test]
fn one_click_grants_once_the_card_has_settled(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    let both = vec![
        device("serial:1", "Arduino Uno", "cu.usbmodem14101"),
        device("serial:2", "CP2102", "cu.SLAB"),
    ];
    shell.update(cx, |shell, cx| {
        shell.set_chooser(10, Some(ports(both.clone())), cx)
    });
    cx.run_until_parked();
    let second = shell.read_with(cx, |shell, _| row(shell, 1, 2));
    cx.simulate_click(second, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(host.take(), Vec::<String>::new());
    cx.executor().advance_clock(SETTLING);
    cx.simulate_click(second, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(host.take(), ["choose device 10 serial:2"]);
    shell.read_with(cx, |shell, _| {
        assert!(shell.chooser().is_none(), "the card stayed after the grant")
    });
    // Asked again, × needn't wait.
    shell.update(cx, |shell, cx| shell.set_chooser(10, Some(ports(both)), cx));
    cx.run_until_parked();
    let close = shell.read_with(cx, |shell, _| close(shell));
    cx.simulate_click(close, Modifiers::none());
    cx.run_until_parked();
    // With the card gone, the pointer was over tab 11's row, whose hover card it leaves.
    let asked: Vec<_> = host
        .take()
        .into_iter()
        .filter(|asked| !asked.starts_with("card "))
        .collect();
    assert_eq!(asked, ["cancel chooser 10"]);
}

/// A device unplugged keeps its row, dimmed, and takes no click; one plugged in joins the end and
/// takes a click only a moment after it came, as it may appear under a click meant for what was
/// there. The rows above stay where the pointer left them.
#[gpui::test]
fn a_device_that_leaves_dims_in_place_and_a_new_one_joins_the_end(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    let (uno, cp) = (
        device("serial:1", "Arduino Uno", "cu.usbmodem14101"),
        device("serial:2", "CP2102", "cu.SLAB"),
    );
    let pico = device("serial:3", "Pico", "cu.usbmodem2101");
    shell.update(cx, |shell, cx| {
        shell.set_chooser(10, Some(ports(vec![uno.clone(), cp.clone()])), cx)
    });
    cx.run_until_parked();
    cx.executor().advance_clock(SETTLING);
    shell.update(cx, |shell, cx| {
        shell.set_chooser(10, Some(ports(vec![cp, pico])), cx)
    });
    cx.run_until_parked();
    let names = shell.read_with(cx, |shell, _| {
        let rows = &shell.chooser().expect("the card went").rows;
        rows.iter()
            .map(|row| (row.device.name.to_string(), row.here))
            .collect::<Vec<_>>()
    });
    assert_eq!(
        names,
        [
            ("Arduino Uno".into(), false),
            ("CP2102".into(), true),
            ("Pico".into(), true)
        ]
    );
    let (gone, new) = shell.read_with(cx, |shell, _| (row(shell, 0, 3), row(shell, 2, 3)));
    cx.simulate_click(gone, Modifiers::none());
    cx.simulate_click(new, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(host.take(), Vec::<String>::new());
    // Plugged back in, it takes its place again.
    let all = vec![
        uno,
        device("serial:2", "CP2102", "cu.SLAB"),
        device("serial:3", "Pico", ""),
    ];
    shell.update(cx, |shell, cx| shell.set_chooser(10, Some(ports(all)), cx));
    cx.executor().advance_clock(SETTLING);
    cx.run_until_parked();
    cx.simulate_click(gone, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(host.take(), ["choose device 10 serial:1"]);
}

/// Bluetooth says it's looking at the list's foot, and once the scan stops with nothing found,
/// says so and offers Scan again, which looks again at once and lets the devices that left go.
/// With Bluetooth off, the card's button opens the system's settings.
#[gpui::test]
fn bluetooth_looks_then_offers_scan_again(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    let bluetooth = |devices, scan| Chooser {
        site: "heartrate.example.com".into(),
        kind: DeviceKind::Bluetooth,
        devices,
        scan,
    };
    let polar = device("bt:1", "Polar H10", "");
    shell.update(cx, |shell, cx| {
        shell.set_chooser(10, Some(bluetooth(Vec::new(), Scan::Scanning)), cx);
        assert_eq!(
            shell.chooser().unwrap().note(),
            None,
            "it said none found while looking"
        );
        shell.set_chooser(10, Some(bluetooth(vec![polar], Scan::Scanning)), cx);
        shell.set_chooser(10, Some(bluetooth(Vec::new(), Scan::Stopped)), cx);
        let asked = shell.chooser().unwrap();
        assert_eq!(asked.note(), Some("No compatible devices found"));
        assert_eq!(asked.rows.len(), 1, "the device that left lost its place");
    });
    cx.run_until_parked();
    // Scan again is the list's last row, under the dimmed device.
    let again = shell.read_with(cx, |shell, _| row(shell, 1, 2));
    cx.simulate_click(again, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(host.take(), ["scan devices 10"]);
    shell.read_with(cx, |shell, _| {
        let asked = shell.chooser().unwrap();
        assert_eq!((asked.scan, asked.rows.len()), (Scan::Scanning, 0));
    });
    shell.update(cx, |shell, cx| {
        shell.set_chooser(10, Some(bluetooth(Vec::new(), Scan::Denied)), cx)
    });
    cx.run_until_parked();
    // The button is the card's last line, as Off's is where the system has one place for it.
    let settings = shell.read_with(cx, |shell, _| {
        let card = shell.chooser_drawn.get().unwrap();
        point(
            card.origin.x + card.size.width / 2.,
            card.origin.y + card.size.height - px(1. + PADDING + 14.),
        )
    });
    cx.simulate_click(settings, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(host.take(), ["device settings 10"]);
}

/// The request is its tab's: another tab shown keeps it for when the tab is back, the page's
/// question has the card first, and the Cast card waits behind both.
#[gpui::test]
fn the_card_is_its_tab_s_after_the_page_s_question(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    let tv = crate::shell::Cast {
        header: "Cast tab".into(),
        devices: Vec::new(),
    };
    shell.update(cx, |shell, cx| {
        shell.set_chooser(11, Some(ports(Vec::new())), cx);
        assert!(
            shell.chooser().is_none(),
            "another tab's request took the card"
        );
        shell.set_selected(11, cx);
        assert!(shell.chooser().is_some());
        shell.set_cast(11, Some(tv), cx);
        let permissions = vec!["Use your camera".into()];
        shell.set_question(
            11,
            Some(Question {
                site: "example.com".into(),
                permissions,
                quiet: false,
            }),
            cx,
        );
        assert!(
            shell.chooser().is_none(),
            "the chooser covered the page's question"
        );
        shell.set_question(11, None, cx);
        assert!(shell.chooser().is_some() && shell.page_asks());
        shell.set_selected(10, cx);
        shell.set_selected(11, cx);
        assert!(
            shell.chooser().is_some(),
            "showing another tab ended the request"
        );
        shell.set_chooser(11, None, cx);
        assert!(!shell.page_asks());
    });
    assert!(!host.take().iter().any(|asked| asked.contains("chooser")));
}
