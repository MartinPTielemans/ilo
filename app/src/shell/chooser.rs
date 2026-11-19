//! A page asking for a device, as `navigator.serial.requestPort()`, `hid.requestDevice()`,
//! `usb.requestDevice()` and `bluetooth.requestDevice()` do, in the page's question card in place
//! of Chrome's chooser bubble, which hangs from a toolbar our layout hasn't: "<site> wants to" over
//! what for, then each device the core finds, its name over what tells two alike apart. One click
//! on a row grants it, once the card and the row have been on screen a moment (settling.rs); ×
//! cancels, and the page gets NotFoundError as from Chrome's Cancel. USB, HID and serial lists are
//! live: a device plugged in joins the end, and one unplugged dims where it was, so nothing moves
//! under the pointer. Bluetooth scans, says so at the list's foot, and offers Scan again once the
//! scan stops. There are no keys on the list: a grant is one deliberate click. The core keeps each
//! tab's request while another tab is shown, as it keeps a question, and cancels one an agent's
//! tab makes, as a device is the user's alone to give.

use std::{cell::Cell, rc::Rc};
use web_time::Instant;

use gpui::{Bounds, Context, FontWeight, Pixels, SharedString, div, prelude::*, px, svg};

use super::{
    NAV, SECONDARY, Shell, TITLE,
    cast::{clickable, device_list, device_row, foot_row},
    permission::{
        BESIDE_CLOSE, BUTTONS_TOP, card_button, card_close, card_listener, part_listener,
    },
};
use crate::settling::{SETTLING, Settling};
use crate::tokens::shell::cast::{GLYPH, LIST_TOP};
use crate::tokens::shell::permission::DETAIL;

/// A page's request for a device, as the core reports it.
#[derive(Clone, PartialEq, Debug)]
pub struct Chooser {
    /// Who asks, as the user should see it.
    pub site: SharedString,
    pub kind: DeviceKind,
    /// What the core has found, in the order it found them.
    pub devices: Vec<ChooserDevice>,
    pub scan: Scan,
}

/// What a page asks for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeviceKind {
    Serial,
    Hid,
    Usb,
    Bluetooth,
}

impl DeviceKind {
    /// What the page asks to do, after "wants to", in Chrome's words.
    fn asks(self) -> &'static str {
        match self {
            DeviceKind::Serial => "Connect to a serial port",
            DeviceKind::Hid => "Connect to a HID device",
            DeviceKind::Usb => "Connect to a USB device",
            DeviceKind::Bluetooth => "Pair with a Bluetooth device",
        }
    }

    /// Its glyph, in the chooser's rows and the Site Controls'.
    pub fn glyph(self) -> &'static str {
        match self {
            DeviceKind::Bluetooth => "icons/bluetooth.svg",
            _ => "icons/usb.svg",
        }
    }
}

/// A device the core found for the page.
#[derive(Clone, PartialEq, Debug)]
pub struct ChooserDevice {
    /// The core's, for `Browser::choose_device`.
    pub id: SharedString,
    pub name: SharedString,
    /// What tells two alike apart: a serial port's path, a USB or HID device's ids, or Bluetooth's
    /// "Paired". Empty for none.
    pub detail: SharedString,
}

/// How a list comes by its devices.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scan {
    /// It follows what's plugged in, as USB, HID and serial's do.
    Live,
    /// Bluetooth is looking.
    Scanning,
    /// Bluetooth stopped looking, and Scan again looks again.
    Stopped,
    /// Bluetooth is off. The core looks once it's on.
    Off,
    /// The system hasn't let ilo use Bluetooth, as the Mac asks first.
    Denied,
}

/// A tab's request as its card shows it: every device the core has reported, in the order they
/// came.
pub(super) struct Asked {
    site: SharedString,
    kind: DeviceKind,
    scan: Scan,
    rows: Vec<Row>,
}

struct Row {
    device: ChooserDevice,
    /// The core still lists it.
    here: bool,
    /// When it takes a click: a moment after it came, as a row that appears under a click on its
    /// way somewhere else mustn't take it.
    from: Instant,
    /// Where it was last drawn, which check.sh reads.
    drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl Asked {
    /// The core's latest list: a device that left keeps its place, dimmed, and one that comes back
    /// takes it again, so the rows under the pointer stay put. A new one joins the end.
    fn update(&mut self, chooser: Chooser, now: Instant) {
        self.site = chooser.site;
        self.kind = chooser.kind;
        self.scan = chooser.scan;
        for row in &mut self.rows {
            match chooser
                .devices
                .iter()
                .find(|device| device.id == row.device.id)
            {
                Some(device) => {
                    row.device = device.clone();
                    if !row.here {
                        row.from = now + SETTLING;
                    }
                    row.here = true;
                }
                None => row.here = false,
            }
        }
        for device in chooser.devices {
            if !self.rows.iter().any(|row| row.device.id == device.id) {
                self.rows.push(Row {
                    device,
                    here: true,
                    from: now + SETTLING,
                    drawn: Rc::default(),
                });
            }
        }
    }

    /// Whether a click on `id` now grants it.
    fn takes(&self, id: &str, now: Instant) -> bool {
        self.rows
            .iter()
            .any(|row| row.device.id == id && row.here && now >= row.from)
    }

    /// The note under what the page asks, if the list can't offer anything yet.
    fn note(&self) -> Option<&'static str> {
        match self.scan {
            Scan::Off => Some("Bluetooth is off"),
            Scan::Denied => Some("ilo needs permission to use Bluetooth"),
            Scan::Scanning => None,
            Scan::Live | Scan::Stopped => {
                (!self.rows.iter().any(|row| row.here)).then_some("No compatible devices found")
            }
        }
    }
}

impl Shell {
    /// The core reports what `tab`'s page asks for, as it finds devices, or that the request ended
    /// (None): granted, cancelled, the page gone or the tab closed.
    pub fn set_chooser(&mut self, tab: u64, chooser: Option<Chooser>, cx: &mut Context<Self>) {
        match chooser {
            Some(chooser) => {
                let now = cx.background_executor().now();
                let asked = self.choosers.entry(tab).or_insert_with(|| Asked {
                    site: chooser.site.clone(),
                    kind: chooser.kind,
                    scan: chooser.scan,
                    rows: Vec::new(),
                });
                asked.update(chooser, now);
                // The card needs the sidebar, so a hidden one comes out with it.
                if tab == self.selected && self.can_reveal() {
                    self.set_revealed(true, cx);
                }
            }
            None if self.choosers.remove(&tab).is_none() => return,
            None => {}
        }
        cx.notify();
    }

    /// The request of the tab shown, unless its page's question has the card.
    pub(super) fn chooser(&self) -> Option<&Asked> {
        self.choosers
            .get(&self.selected)
            .filter(|_| self.question().is_none())
    }

    /// Whether the tab shown's page asks the user something, a permission, a device or a device's
    /// pairing, whose card has the place under the URL field and keeps a revealed sidebar out.
    pub(super) fn page_asks(&self) -> bool {
        self.question().is_some() || self.chooser().is_some() || self.pairing().is_some()
    }

    fn choose(&mut self, tab: u64, id: SharedString, cx: &mut Context<Self>) {
        let settled = (self.chooser_settling.borrow().as_ref())
            .is_some_and(|(shown, settling)| *shown == tab && settling.settled(cx));
        let now = cx.background_executor().now();
        if !settled
            || !self
                .choosers
                .get(&tab)
                .is_some_and(|asked| asked.takes(&id, now))
        {
            return;
        }
        self.choosers.remove(&tab);
        log!("browser: command choose-device {id} via card");
        self.browser.choose_device(tab, id, cx);
        cx.notify();
    }

    fn cancel_chooser(&mut self, tab: u64, cx: &mut Context<Self>) {
        if self.choosers.remove(&tab).is_some() {
            log!("browser: command chooser-close via card");
            self.browser.cancel_chooser(tab, cx);
            cx.notify();
        }
    }

    /// Scan again: the devices that left go, and the foot says it's looking at once.
    fn scan_again(&mut self, tab: u64, cx: &mut Context<Self>) {
        let Some(asked) = self.choosers.get_mut(&tab) else {
            return;
        };
        asked.rows.retain(|row| row.here);
        asked.scan = Scan::Scanning;
        log!("browser: command chooser-scan via card");
        self.browser.scan_devices(tab, cx);
        cx.notify();
    }

    /// The tab shown's request, as a card under the URL field.
    pub(super) fn chooser_card(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let Some(asked) = self.chooser() else {
            // Drawn again, it's logged again, and settles again.
            self.chooser_drawn.set(None);
            self.chooser_settling.replace(None);
            return None;
        };
        let tab = self.selected;
        let mut settling = self.chooser_settling.borrow_mut();
        if settling.as_ref().is_none_or(|(shown, _)| *shown != tab) {
            let mut started = Settling::default();
            started.start(format!("chooser {tab}"), cx);
            *settling = Some((tab, started));
        }
        drop(settling);
        let listed = matches!(asked.scan, Scan::Live | Scan::Scanning | Scan::Stopped);
        let mut rows = Vec::new();
        for (index, row) in asked.rows.iter().enumerate().filter(|_| listed) {
            let device = &row.device;
            let item = device_row(
                &self.palette,
                ("chooser-device", index),
                asked.kind.glyph(),
                device.name.clone(),
                device.detail.clone(),
            )
            .relative()
            .child(part_listener(
                format!("chooser row {index}"),
                &row.drawn,
                tab,
            ));
            if !row.here {
                rows.push(item.opacity(0.5).into_any_element());
                continue;
            }
            let id = device.id.clone();
            let item = clickable(item, &self.palette)
                .on_click(cx.listener(move |shell, _, _, cx| shell.choose(tab, id.clone(), cx)));
            rows.push(item.into_any_element());
        }
        match asked.scan {
            Scan::Scanning => rows.push(self.looking_row().into_any_element()),
            Scan::Stopped => {
                let glyph = svg()
                    .path("icons/rotate-cw.svg")
                    .flex_none()
                    .size(px(GLYPH))
                    .text_color(self.palette.glyph(NAV));
                let again = foot_row(&self.palette, "chooser-scan", glyph, "Scan again");
                let again = clickable(again, &self.palette)
                    .on_click(cx.listener(move |shell, _, _, cx| shell.scan_again(tab, cx)));
                rows.push(again.into_any_element());
            }
            _ => {}
        }
        // Linux has no one place where Bluetooth is turned on, so there it's the note alone.
        let settings = match asked.scan {
            Scan::Off if cfg!(not(target_os = "linux")) => Some("Bluetooth Settings…"),
            Scan::Denied => Some("System Settings…"),
            _ => None,
        };
        let settings = settings.map(|label| {
            let button = card_button(&self.palette, "chooser-settings", label).on_click(
                cx.listener(move |shell, _, _, cx| {
                    log!("browser: command chooser-settings via card");
                    shell.browser.device_settings(tab, cx);
                }),
            );
            div().mt(px(BUTTONS_TOP)).flex().child(button)
        });
        let note = asked.note().map(|note| {
            div()
                .mt(px(LIST_TOP))
                .text_color(self.palette.on_overlay(SECONDARY))
                .child(note)
        });
        let list = (!rows.is_empty()).then(|| device_list().children(rows));
        let close = card_close(&self.palette, "chooser-close")
            .on_click(cx.listener(move |shell, _, _, cx| shell.cancel_chooser(tab, cx)));
        Some(
            self.plate("chooser")
                .child(card_listener("chooser", &self.chooser_drawn, tab))
                .child(
                    div()
                        .pr(px(BESIDE_CLOSE))
                        .text_color(self.palette.text(TITLE))
                        .font_weight(FontWeight::MEDIUM)
                        // Kept together, so "to" doesn't wrap alone under a long site.
                        .child(format!("{} wants\u{a0}to", asked.site)),
                )
                .child(
                    div()
                        .text_color(self.palette.on_overlay(DETAIL))
                        .child(asked.kind.asks()),
                )
                .children(note)
                .children(list)
                .children(settings)
                .child(close),
        )
    }
}

#[cfg(test)]
mod tests;
