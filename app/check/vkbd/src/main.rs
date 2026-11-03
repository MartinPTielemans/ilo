//! A keyboard and a pointer in the Wayland session in WAYLAND_DISPLAY, for check/ime.sh: a headless
//! sway seat has neither until a client makes one.
//! - `vkbd <text>` types the text into the focused window through a virtual keyboard with the US
//!   keymap xkbcli makes: lowercase and capital letters, digits, `- ; . / , = : @`, spaces,
//!   newlines as Enter, tabs as Tab, ESC as Escape, and `^` for Ctrl with the next key.
//! - `vkbd --hold <seconds>` keeps a keyboard on the seat that long, as a desktop's is there all
//!   along: with none left the compositor sends the app a keyboard leave.
//! - `vkbd --click <x> <y>` clicks at that point of the 1280 by 800 output with a virtual pointer.
//! Each waits a settle for clients to bind the seat's new keyboard or pointer before using it,
//! VKBD_SETTLE milliseconds (500), as Xwayland takes longer than Wayland clients.

use std::{io::Write as _, os::fd::AsFd as _, process::Command, time::Duration};

use wayland_client::{
    Connection, QueueHandle, delegate_noop,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_pointer::ButtonState, wl_registry, wl_seat},
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};

struct State;

impl wayland_client::Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(State: ignore wl_seat::WlSeat);
delegate_noop!(State: ZwpVirtualKeyboardManagerV1);
delegate_noop!(State: ZwpVirtualKeyboardV1);
delegate_noop!(State: ZwlrVirtualPointerManagerV1);
delegate_noop!(State: ZwlrVirtualPointerV1);

const BUTTON_LEFT: u32 = 0x110;
// Evdev's codes and xkb's modifier masks for the keys held with others.
const CTRL: (u32, u32) = (29, 4);
const SHIFT: (u32, u32) = (42, 1);

fn settle() -> Duration {
    let ms = std::env::var("VKBD_SETTLE").ok().and_then(|v| v.parse().ok());
    Duration::from_millis(ms.unwrap_or(500))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: vkbd <text> | vkbd --hold <seconds> | vkbd --click <x> <y>";
    let number = |i: usize| -> u64 { args.get(i).and_then(|n| n.parse().ok()).expect(usage) };
    match args.first().map(String::as_str) {
        Some("--click") => click(number(1) as u32, number(2) as u32),
        Some("--hold") => keyboard(None, Duration::from_secs(number(1))),
        Some(text) => keyboard(Some(text), settle()),
        None => panic!("{usage}"),
    }
}

fn click(x: u32, y: u32) {
    let conn = Connection::connect_to_env().expect("no Wayland display");
    let (globals, mut queue) = registry_queue_init::<State>(&conn).unwrap();
    let qh = queue.handle();
    let seat: wl_seat::WlSeat = globals.bind(&qh, 1..=7, ()).unwrap();
    let manager: ZwlrVirtualPointerManagerV1 =
        globals.bind(&qh, 1..=2, ()).expect("no virtual pointer");
    let pointer = manager.create_virtual_pointer(Some(&seat), &qh, ());
    queue.roundtrip(&mut State).unwrap();
    std::thread::sleep(settle());
    let start = std::time::Instant::now();
    let now = || start.elapsed().as_millis() as u32;
    pointer.motion_absolute(now(), x, y, 1280, 800);
    pointer.frame();
    queue.roundtrip(&mut State).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    for state in [ButtonState::Pressed, ButtonState::Released] {
        pointer.button(now(), BUTTON_LEFT, state);
        pointer.frame();
        queue.roundtrip(&mut State).unwrap();
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(settle());
    pointer.destroy();
    queue.roundtrip(&mut State).unwrap();
    println!("vkbd: clicked {x},{y}");
}

/// Makes a keyboard, types `text` with it if there's any, and keeps it `after` before it goes.
fn keyboard(text: Option<&str>, after: Duration) {
    let keymap = Command::new("xkbcli")
        .args(["compile-keymap", "--layout", "us"])
        .output()
        .expect("no xkbcli")
        .stdout;
    assert!(!keymap.is_empty(), "xkbcli made no keymap");
    let conn = Connection::connect_to_env().expect("no Wayland display");
    let (globals, mut queue) = registry_queue_init::<State>(&conn).unwrap();
    let qh = queue.handle();
    let seat: wl_seat::WlSeat = globals.bind(&qh, 1..=7, ()).unwrap();
    let manager: ZwpVirtualKeyboardManagerV1 =
        globals.bind(&qh, 1..=1, ()).expect("no virtual keyboard");
    let keyboard = manager.create_virtual_keyboard(&seat, &qh, ());
    let mut file = memfd();
    file.write_all(&keymap).unwrap();
    file.write_all(&[0]).unwrap();
    // Format 1 is xkb's text keymap, ending in a NUL.
    keyboard.keymap(1, file.as_fd(), keymap.len() as u32 + 1);
    keyboard.modifiers(0, 0, 0, 0);
    queue.roundtrip(&mut State).unwrap();
    if let Some(text) = text {
        std::thread::sleep(settle());
        let start = std::time::Instant::now();
        let mut press = |code: u32, down: bool, mask: Option<u32>| {
            keyboard.key(start.elapsed().as_millis() as u32, code, down as u32);
            if let Some(mask) = mask {
                keyboard.modifiers(mask, 0, 0, 0);
            }
            queue.roundtrip(&mut State).unwrap();
            std::thread::sleep(Duration::from_millis(20));
        };
        let mut ctrl = false;
        for c in text.chars() {
            if c == '^' {
                ctrl = true;
                continue;
            }
            let (c, mut held) = match c {
                ':' => (';', vec![SHIFT]),
                '@' => ('2', vec![SHIFT]),
                c if c.is_ascii_uppercase() => (c.to_ascii_lowercase(), vec![SHIFT]),
                c => (c, vec![]),
            };
            if std::mem::take(&mut ctrl) {
                held.push(CTRL);
            }
            let code = code(c).unwrap_or_else(|| panic!("can't type {c:?}"));
            // Each modifier's key goes down with its mask, and up with the mask left.
            let mut mask = 0;
            for &(modifier, bit) in &held {
                mask |= bit;
                press(modifier, true, Some(mask));
            }
            press(code, true, None);
            press(code, false, None);
            for &(modifier, bit) in held.iter().rev() {
                mask &= !bit;
                press(modifier, false, Some(mask));
            }
        }
        println!("vkbd: typed {text:?}");
    }
    std::thread::sleep(after);
    keyboard.destroy();
    queue.roundtrip(&mut State).unwrap();
}

/// The evdev code of the key that types `c` on a US keyboard, without Shift.
fn code(c: char) -> Option<u32> {
    const ROWS: [(&str, u32); 4] = [
        ("1234567890", 2),
        ("qwertyuiop", 16),
        ("asdfghjkl", 30),
        ("zxcvbnm", 44),
    ];
    match c {
        ' ' => return Some(57),
        '\n' => return Some(28),
        '\u{1b}' => return Some(1),
        '\t' => return Some(15),
        _ => {}
    }
    if let Some(i) = "-;./,=".find(c) {
        return Some([12, 39, 52, 53, 51, 13][i]);
    }
    ROWS.iter().find_map(|(row, first)| row.find(c).map(|i| first + i as u32))
}

fn memfd() -> std::fs::File {
    use std::os::fd::FromRawFd as _;
    let fd = unsafe { libc::memfd_create(c"vkbd-keymap".as_ptr(), libc::MFD_CLOEXEC) };
    assert!(fd >= 0, "no memfd");
    unsafe { std::fs::File::from_raw_fd(fd) }
}
