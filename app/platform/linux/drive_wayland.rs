//! The test driver's input under Wayland (drive.rs has the commands' language), which no client
//! can hand another's surfaces: a virtual pointer and keyboard of sway's, as a device's, so the
//! compositor's hit testing, focus, and enter and leave are in what the runs check. Only in
//! tools/headless.sh's own sway session, which holds nothing but the app, as Xvfb does on X11:
//! WAYLAND_DISPLAY has to be the session's (BROWSER_HEADLESS), sway's outputs all headless, and the
//! pointer goes only where one of the app's own windows is, by their places sway reports.
//!
//! Points are in a window's content, which sway places at its rect's origin with no border, at the
//! session's scale of 1. Keys go where sway's keyboard focus is, the browser window or Settings,
//! and the core hands them on to the UI or an overlay as it does a real keyboard's. The done line's
//! key window is sway's focused window, and in a browser window the overlay that has the keys, else
//! whose the core says they are. Every window floats (tools/headless.sh), so sway stacks them as a
//! desktop does, and moves and sizes them as it's told. Drags from another app aren't here yet.

use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    io::{Read as _, Write as _},
    os::{fd::AsFd as _, unix::net::UnixStream},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use browser::log;
use gpui::{Keystroke, Modifiers};
use serde_json::Value;
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, delegate_noop,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_pointer, wl_registry, wl_seat::WlSeat},
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};
use xkbcommon::xkb;

use crate::{CX, WINDOWS, bc, drive, floats, overlays, panels, settings_window};

/// The modifiers, as drive.rs's X states hold them, which are the xkb masks of the keymap the
/// driver's keyboard has.
const SHIFT: u16 = 1;
const CONTROL: u16 = 4;
const ALT: u16 = 8;
/// The left modifier keys, as evdev numbers them.
const MODIFIER_KEYS: [(u16, u32); 3] = [(CONTROL, 29), (ALT, 56), (SHIFT, 42)];
const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;
/// The pointer's positions per point, so a point between pixels keeps its place.
const SUBPOINTS: f64 = 100.;
/// sway's IPC messages.
const RUN_COMMAND: u32 = 0;
const GET_OUTPUTS: u32 = 3;
const GET_TREE: u32 = 4;

struct Session {
    queue: EventQueue<State>,
    pointer: ZwlrVirtualPointerV1,
    keyboard: ZwpVirtualKeyboardV1,
    keymap: xkb::Keymap,
    /// The size of sway's layout, which the pointer's absolute positions are in.
    extent: (f64, f64),
    started: Instant,
}

struct State;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
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

delegate_noop!(State: ignore WlSeat);
delegate_noop!(State: ZwlrVirtualPointerManagerV1);
delegate_noop!(State: ZwlrVirtualPointerV1);
delegate_noop!(State: ZwpVirtualKeyboardManagerV1);
delegate_noop!(State: ZwpVirtualKeyboardV1);

thread_local! {
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
    /// Each browser window by sway's id for it, in the order the app made them: the window
    /// command's numbers.
    static PAIRED: RefCell<Vec<(bc::bc_id, i64)>> = const { RefCell::new(Vec::new()) };
    static DRIVEN: Cell<usize> = const { Cell::new(0) };
    /// The modifiers flags holds, and those the last key went with, as drive.rs keeps them.
    static FLAGS: Cell<u16> = const { Cell::new(0) };
    static KEY_FLAGS: Cell<u16> = const { Cell::new(0) };
    /// The last press: when, where in sway's layout, and its button.
    static LAST_CLICK: Cell<Option<(Instant, f64, f64, u32)>> = const { Cell::new(None) };
}

/// Makes the driver's pointer and keyboard under Wayland, before the app's windows, so Chromium
/// and GPUI have the seat's pointer and keyboard when they open.
pub(crate) fn start() {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return;
    }
    match open() {
        Ok(session) => {
            log!("browser: drive input through sway's virtual pointer and keyboard");
            SESSION.set(Some(session));
            ON.store(true, Ordering::Relaxed);
        }
        Err(why) => log!("browser: drive has no input under Wayland: {why}"),
    }
}

/// Whether the driver's input is Wayland's, for the thread that reads its commands.
static ON: AtomicBool = AtomicBool::new(false);

/// Waits, on the thread that reads the commands, until sway has given one of the app's windows the
/// keyboard, for up to 5 s. Input before that goes to no window, which lost a run's first command
/// sent as the window comes on screen: its key, or a menu for a window the driver couldn't find yet
/// (#1218, #1226).
pub(crate) fn await_first_focus() {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    let until = Instant::now() + Duration::from_secs(5);
    while !ours().iter().any(|node| node.focused) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Whether the driver's input is Wayland's.
pub(crate) fn on() -> bool {
    SESSION.with_borrow(Option::is_some)
}

fn open() -> Result<Session, String> {
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
    let headless = std::env::var("BROWSER_HEADLESS").unwrap_or_default();
    if display.is_empty() || display != headless {
        return Err("WAYLAND_DISPLAY isn't tools/headless.sh's session".into());
    }
    let outputs = sway(GET_OUTPUTS, "").ok_or("no sway at SWAYSOCK")?;
    let outputs = outputs.as_array().ok_or("sway listed no outputs")?;
    let headless = |output: &Value| {
        output["name"]
            .as_str()
            .is_some_and(|name| name.starts_with("HEADLESS-"))
    };
    if outputs.is_empty() || !outputs.iter().all(headless) {
        return Err("sway's outputs aren't all headless".into());
    }
    let extent = outputs
        .iter()
        .fold((0f64, 0f64), |(width, height), output| {
            let rect = &output["rect"];
            let right = number(&rect["x"]) + number(&rect["width"]);
            let bottom = number(&rect["y"]) + number(&rect["height"]);
            (width.max(right), height.max(bottom))
        });
    let connection = Connection::connect_to_env().map_err(|e| e.to_string())?;
    let (globals, mut queue) =
        registry_queue_init::<State>(&connection).map_err(|e| e.to_string())?;
    let handle = queue.handle();
    let seat: WlSeat = globals
        .bind(&handle, 1..=1, ())
        .map_err(|e| e.to_string())?;
    let pointers: ZwlrVirtualPointerManagerV1 = globals
        .bind(&handle, 1..=2, ())
        .map_err(|e| e.to_string())?;
    let keyboards: ZwpVirtualKeyboardManagerV1 = globals
        .bind(&handle, 1..=1, ())
        .map_err(|e| e.to_string())?;
    let pointer = pointers.create_virtual_pointer(Some(&seat), &handle, ());
    let keyboard = keyboards.create_virtual_keyboard(&seat, &handle, ());
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let keymap = xkb::Keymap::new_from_names(&context, "evdev", "pc105", "us", "", None, 0)
        .ok_or("xkbcommon has no us keymap")?;
    // The compositor maps the keymap from a file, with its NUL.
    let mut text = keymap
        .get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1)
        .into_bytes();
    text.push(0);
    let path = std::env::temp_dir().join(format!("ilo-drive-keymap-{}", std::process::id()));
    let mut file = std::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    _ = std::fs::remove_file(&path);
    file.write_all(&text).map_err(|e| e.to_string())?;
    keyboard.keymap(xkb::KEYMAP_FORMAT_TEXT_V1, file.as_fd(), text.len() as u32);
    queue.roundtrip(&mut State).map_err(|e| e.to_string())?;
    Ok(Session {
        queue,
        pointer,
        keyboard,
        keymap,
        extent,
        started: Instant::now(),
    })
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or(0.)
}

/// Asks sway, through its IPC socket, and gives its answer.
fn sway(kind: u32, payload: &str) -> Option<Value> {
    let mut stream = UnixStream::connect(std::env::var_os("SWAYSOCK")?).ok()?;
    let mut message = b"i3-ipc".to_vec();
    message.extend((payload.len() as u32).to_ne_bytes());
    message.extend(kind.to_ne_bytes());
    message.extend(payload.as_bytes());
    stream.write_all(&message).ok()?;
    let mut header = [0u8; 14];
    stream.read_exact(&mut header).ok()?;
    let length = u32::from_ne_bytes(header[6..10].try_into().ok()?) as usize;
    let mut body = vec![0; length];
    stream.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

/// Has sway run `command`, such as "[con_id=4] focus".
fn sway_command(command: &str) -> Option<()> {
    let answer = sway(RUN_COMMAND, command)?;
    (answer.as_array()?.iter())
        .all(|each| each["success"].as_bool() == Some(true))
        .then_some(())
}

/// A window of the app's as sway has it, with its content's frame in the layout.
#[derive(Clone)]
struct Node {
    id: i64,
    /// Chromium's windows have one, GPUI's Settings none.
    app_id: String,
    name: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    focused: bool,
    full_screen: bool,
}

impl Node {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    fn is_settings(&self) -> bool {
        self.app_id.is_empty()
    }
}

/// The app's windows, as sway's tree has them. In the Flatpak the process's pid is its sandbox's,
/// not the one sway knows, and the headless session (`open`) holds only the app the run started,
/// so every window in it is the app's.
fn ours() -> Vec<Node> {
    fn walk(node: &Value, out: &mut Vec<Node>) {
        let pid = node["pid"].as_u64();
        let own = u64::from(std::process::id());
        if pid.is_some_and(|pid| pid == own || browser::flatpak::in_flatpak()) {
            let (rect, window) = (&node["rect"], &node["window_rect"]);
            out.push(Node {
                id: node["id"].as_i64().unwrap_or(-1),
                app_id: node["app_id"].as_str().unwrap_or_default().to_string(),
                name: node["name"].as_str().unwrap_or_default().to_string(),
                x: number(&rect["x"]) + number(&window["x"]),
                y: number(&rect["y"]) + number(&window["y"]),
                width: number(&window["width"]),
                height: number(&window["height"]),
                focused: node["focused"].as_bool() == Some(true),
                full_screen: node["fullscreen_mode"]
                    .as_u64()
                    .is_some_and(|mode| mode != 0),
            });
        }
        for child in ["nodes", "floating_nodes"]
            .iter()
            .filter_map(|key| node[*key].as_array())
        {
            child.iter().for_each(|each| walk(each, out));
        }
    }
    let mut out = Vec::new();
    if let Some(tree) = sway(GET_TREE, "") {
        walk(&tree, &mut out);
    }
    out
}

/// Logs the app's windows as sway has them, for a run that fails to find one.
fn describe(nodes: &[Node]) {
    for node in nodes {
        log!(
            "browser: drive sway window {} app {:?} titled {:?} at {:.0},{:.0} {:.0}x{:.0}{}",
            node.id,
            node.app_id,
            node.name,
            node.x,
            node.y,
            node.width,
            node.height,
            if node.focused { ", focused" } else { "" }
        );
    }
}

/// The open browser windows, with sway's window for each. Both number their windows as they're
/// made, so a window new to each is paired with the other's new one.
fn browsers(nodes: &[Node]) -> Vec<(bc::bc_id, Node)> {
    let open: Vec<bc::bc_id> =
        WINDOWS.with_borrow(|windows| windows.iter().map(|w| w.id).collect());
    let shown: Vec<&Node> = nodes.iter().filter(|node| !node.is_settings()).collect();
    PAIRED.with_borrow_mut(|paired| {
        let mut ids: Vec<_> = (open.iter())
            .filter(|id| !paired.iter().any(|(each, _)| each == *id))
            .copied()
            .collect();
        ids.sort();
        let mut fresh: Vec<_> = (shown.iter())
            .filter(|node| !paired.iter().any(|&(_, each)| each == node.id))
            .map(|node| node.id)
            .collect();
        fresh.sort();
        paired.extend(ids.into_iter().zip(fresh));
        (paired.iter())
            .filter(|(id, _)| open.contains(id))
            .filter_map(|&(id, con)| Some((id, (*shown.iter().find(|n| n.id == con)?).clone())))
            .collect()
    })
}

/// The browser window the app made `index`th, from 0, while it's open.
fn browser(index: usize) -> Option<(bc::bc_id, Node)> {
    let nodes = ours();
    let open = browsers(&nodes);
    let id = PAIRED.with_borrow(|paired| paired.get(index).map(|&(id, _)| id))?;
    let found = open.into_iter().find(|&(each, _)| each == id);
    if found.is_none() {
        describe(&nodes);
    }
    found
}

fn driven() -> Option<(bc::bc_id, Node)> {
    browser(DRIVEN.get())
}

/// The driven browser window's id, for drive.rs's commands.
pub(crate) fn driven_id() -> Option<bc::bc_id> {
    driven().map(|(id, _)| id)
}

/// The open browser windows, front to back, as sway stacks them: its floating windows go up its
/// list as they're focused, and a window in full screen is over them all.
fn stacked() -> Vec<(bc::bc_id, Node)> {
    let nodes = ours();
    let open = browsers(&nodes);
    let mut open: Vec<_> = (nodes.iter().rev())
        .filter_map(|node| open.iter().find(|(_, each)| each.id == node.id).cloned())
        .collect();
    open.sort_by_key(|(_, node)| !node.full_screen);
    open
}

/// A browser window as order and front name it: "window <n>", numbered as the window command
/// numbers them.
fn name_of(id: bc::bc_id) -> String {
    let index = PAIRED.with_borrow(|paired| paired.iter().position(|&(each, _)| each == id));
    index.map_or_else(
        || "none".to_string(),
        |index| format!("window {}", index + 1),
    )
}

/// The browser window with sway's focus.
fn focused_browser() -> Option<bc::bc_id> {
    let nodes = ours();
    let focused = nodes.iter().find(|node| node.focused)?;
    browsers(&nodes)
        .into_iter()
        .find(|(_, node)| node.id == focused.id)
        .map(|(id, _)| id)
}

/// The title of the window with sway's focus, as drive.rs's key_title: a dialog of Chromium's in a
/// browser window, which sway has no window for, by the core's.
pub(crate) fn key_title() -> Option<String> {
    let nodes = ours();
    let focused = nodes.iter().find(|node| node.focused)?;
    if let Some((id, _)) = browsers(&nodes)
        .into_iter()
        .find(|(_, node)| node.id == focused.id)
        && unsafe { bc::bc_window_keyboard_in_child(id) }
    {
        return Some(drive::child_title(id));
    }
    Some(focused.name.clone())
}

/// Moves the driven browser window's content to (`x`, `y`) in sway's layout, or keeps it where it
/// is, as sway sizes a floating window about its centre, and sizes it `width` by `height`, which
/// the command waits for the UI to have.
fn frame(at: Option<(f64, f64)>, width: f64, height: f64) -> Option<()> {
    let (id, node) = driven()?;
    let handle =
        WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == id).map(|w| w.handle))?;
    let cx = CX.with_borrow(Clone::clone)?;
    let view = cx
        .update(|cx| handle.update(cx, |_, window, _| crate::ns_view(window)))
        .ok()?;
    let (x, y) = at.unwrap_or((node.x, node.y));
    let con = node.id;
    sway_command(&format!(
        "[con_id={con}] resize set width {width} px height {height} px, move position {x} {y}"
    ))?;
    drive::resizing(view as usize, width, height);
    Some(())
}

/// Settings while it has sway's focus.
fn focused_settings(nodes: &[Node]) -> Option<Node> {
    settings_window::shown()?;
    nodes.iter().find(|n| n.focused && n.is_settings()).cloned()
}

/// Where in the content of its browser window the overlay of `view` shows.
fn overlay_frame(view: *mut c_void) -> Option<(bc::bc_id, gpui::Bounds<gpui::Pixels>)> {
    let (ui_view, bounds) = (overlays::placed(view))
        .or_else(|| panels::placed(view))
        .or_else(|| floats::placed(view))?;
    let id = WINDOWS
        .with_borrow(|windows| windows.iter().find(|w| w.ui_view == ui_view).map(|w| w.id))?;
    Some((id, bounds))
}

/// Sway focuses Settings once it has mapped it, which as it first shows can come after the
/// driver's next click, which then went to the browser window (#1180): the first click after a
/// show waits for it. Only that one, as a browser window can take the focus while Settings shows.
fn await_settings_focus() {
    if !settings_window::take_shown_anew() {
        return;
    }
    let until = Instant::now() + Duration::from_secs(2);
    while focused_settings(&ours()).is_none() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The origin, in sway's layout, of the window with the keyboard, for clickkey: Settings with
/// sway's focus, the overlay with the keys in its browser window, or the browser window with sway's
/// focus, else the driven one.
fn key_origin() -> Option<(f64, f64)> {
    let nodes = ours();
    if let Some(settings) = focused_settings(&nodes) {
        return Some((settings.x, settings.y));
    }
    if let Some((id, bounds)) = gpui_linux::shown_keyboard_overlay().and_then(overlay_frame)
        && let Some((_, node)) = browsers(&nodes).into_iter().find(|&(each, _)| each == id)
    {
        // At whole points, as a subsurface's place is, which GPUI truncates the overlay's to: a
        // click in the theme picker, at a half point, landed half a point off.
        let origin = bounds.origin.map(|each| f64::from(each).trunc());
        return Some((node.x + origin.x, node.y + origin.y));
    }
    // As drive.rs's is the X focus's: a Little ilo window the driven one opened has the keys, and
    // its profile picker's click went to the driven window under it.
    let focused = nodes.iter().find(|node| node.focused).map(|node| node.id);
    let node = match browsers(&nodes)
        .into_iter()
        .find(|(_, node)| Some(node.id) == focused)
    {
        Some((_, node)) => node,
        None => driven()?.1,
    };
    Some((node.x, node.y))
}

/// The done line's key window, focus and whether the app is active, as drive.rs names them.
pub(crate) fn keyboard() -> (&'static str, &'static str, bool) {
    let nodes = ours();
    let Some(focused) = nodes.iter().find(|node| node.focused) else {
        return ("none", "none", false);
    };
    if focused.is_settings() {
        return ("GPUIWindow", "GPUIView", true);
    }
    if gpui_linux::shown_keyboard_overlay().is_some() {
        return ("GPUIPanel", "GPUIView", true);
    }
    let Some((id, _)) = browsers(&nodes)
        .into_iter()
        .find(|(_, node)| node.id == focused.id)
    else {
        return ("none", "none", true);
    };
    if unsafe { bc::bc_window_keyboard_in_child(id) } {
        return ("NativeWidgetMacNSWindow", "RenderWidgetHostViewCocoa", true);
    }
    match unsafe { bc::bc_window_keyboard_is_ui(id) } {
        true => ("BrowserCoreNSWindow", "AccessKitSubclassOfGPUIView", true),
        false => ("BrowserCoreNSWindow", "RenderWidgetHostViewCocoa", true),
    }
}

/// Waits for sway to have handled what the driver sent, and so to have sent the app what it made
/// of it.
pub(crate) fn sync() {
    SESSION.with_borrow_mut(|session| {
        if let Some(session) = session {
            _ = session.queue.roundtrip(&mut State);
        }
    });
}

fn with_session<R>(f: impl FnOnce(&Session, u32) -> R) -> Option<R> {
    SESSION.with_borrow(|session| {
        let session = session.as_ref()?;
        let time = session.started.elapsed().as_millis() as u32;
        let result = f(session, time);
        _ = session.queue.flush();
        Some(result)
    })
}

/// Has the keyboard hold `state`'s modifiers.
fn hold(state: u16) -> Option<()> {
    with_session(|session, _| session.keyboard.modifiers(u32::from(state), 0, 0, 0))
}

/// What the keyboard holds between commands: what flags holds, and Control after a key with it,
/// for the ⌃Tab walk, as drive.rs's X server holds it, until a pointer event lets it go.
fn held() -> u16 {
    FLAGS.get() | KEY_FLAGS.get() & CONTROL
}

/// The pointer to (`x`, `y`) in sway's layout, if that's in one of the app's windows.
fn motion(x: f64, y: f64) -> Option<()> {
    if !ours().iter().any(|node| node.contains(x, y)) {
        log!("browser: drive won't put the pointer at {x:.0},{y:.0}, outside the app's windows");
        return None;
    }
    with_session(|session, time| {
        let (width, height) = session.extent;
        let at = |each: f64| (each * SUBPOINTS).round() as u32;
        let extent = |each: f64| (each * SUBPOINTS) as u32;
        (session.pointer).motion_absolute(time, at(x), at(y), extent(width), extent(height));
        session.pointer.frame();
    })
}

fn button(which: u32, press: bool) -> Option<()> {
    with_session(|session, time| {
        let state = match press {
            true => wl_pointer::ButtonState::Pressed,
            false => wl_pointer::ButtonState::Released,
        };
        session.pointer.button(time, which, state);
        session.pointer.frame();
    })
}

/// Before a press, as drive.rs's `first_press`: unless it's `again`, the second of a double click,
/// waits until neither GPUI nor Chromium would count it with the last press.
fn first_press(x: f64, y: f64, which: u32, again: bool) {
    if !again
        && let Some((then, last_x, last_y, last_which)) = LAST_CLICK.get()
        && last_which == which
        && (last_x - x).abs() <= 5.
        && (last_y - y).abs() <= 5.
    {
        std::thread::sleep(std::time::Duration::from_millis(550).saturating_sub(then.elapsed()));
    }
    LAST_CLICK.set(Some((Instant::now(), x, y, which)));
}

/// Has the pointer do `events` with the modifiers `state`, and those flags holds. Control held
/// after a key for the ⌃Tab walk isn't one: drive.rs's X events each carry their own state, but
/// Wayland gives a pointer's the keyboard's, so it goes up for them, ending the walk as a click
/// would. With it, a wheel zoomed the page (#1219) and a drag pressed as a ⌃-click (#1221). It
/// stays up after, as a hand's has: put back down, the next pointer event's lift made it a press
/// of ⌃ alone, a key, which ended a hover card under the pointer before its click (#1223).
fn with_modifiers(state: u16, events: impl FnOnce() -> Option<()>) -> Option<()> {
    hold(state | FLAGS.get())?;
    KEY_FLAGS.set(KEY_FLAGS.get() & !CONTROL);
    let done = events();
    hold(held())?;
    done
}

/// A click of `which` at (`x`, `y`) in sway's layout with the modifiers `state`.
fn click(x: f64, y: f64, which: u32, state: u16, again: bool) -> Option<()> {
    first_press(x, y, which, again);
    motion(x, y)?;
    with_modifiers(state, || {
        button(which, true)?;
        button(which, false)
    })
}

/// (`x`, `y`) in the driven browser window's content, in sway's layout.
fn in_driven(x: f64, y: f64) -> Option<(f64, f64)> {
    let (_, node) = driven()?;
    Some((node.x + x, node.y + y))
}

/// The key that types `keysym`, as evdev numbers it, and whether with shift.
fn keycode(keysym: u32) -> Option<(u32, bool)> {
    SESSION.with_borrow(|session| {
        let keymap = &session.as_ref()?.keymap;
        let (first, last) = (keymap.min_keycode().raw(), keymap.max_keycode().raw());
        (first..=last).find_map(|code| {
            let key = xkb::Keycode::new(code);
            let levels = keymap.num_levels_for_key(key, 0).min(2);
            let level = (0..levels).find(|&level| {
                (keymap.key_get_syms_by_level(key, 0, level).iter()).any(|sym| sym.raw() == keysym)
            })?;
            Some((code.checked_sub(8)?, level == 1))
        })
    })
}

/// Sends `key`'s presses and releases, `kinds` true for a press, with the modifiers `state`.
fn key_events(key: &str, state: u16, kinds: &[bool]) -> Option<()> {
    let (code, shift) = keycode(drive::keysym(key)?)?;
    let state = state | FLAGS.get();
    KEY_FLAGS.set(state & (CONTROL | ALT | SHIFT));
    hold(state | if shift { SHIFT } else { 0 })?;
    with_session(|session, time| {
        for &press in kinds {
            session.keyboard.key(time, code, u32::from(press));
        }
    })?;
    hold(held())
}

/// The surface where drive.rs's scripted input method composes, as the compositor's would:
/// Settings' with sway's focus, or the browser window's while its UI or an overlay over it has the
/// keys. Not while a page has them, whose input method is Chromium's.
pub(crate) fn ime_surface() -> Option<*mut c_void> {
    if focused_settings(&ours()).is_some() {
        return settings_window::shown();
    }
    let id = focused_browser()?;
    let ui = gpui_linux::shown_keyboard_overlay().is_some()
        || unsafe { bc::bc_window_keyboard_is_ui(id) };
    ui.then_some(())?;
    WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == id).map(|w| w.ui_view))
}

/// Presses and releases `key` with the modifiers `state`, or composes it in the UI's field while
/// drive.rs's scripted input method types, as drive.rs's key does.
fn key(key: &str, state: u16) -> Option<()> {
    if drive::ime_key(key, state | FLAGS.get())? {
        return Some(());
    }
    key_events(key, state, &[true, false])
}

/// Commits `text` to the field of the UI, Settings or the overlay with the keys, as drive.rs's
/// insert does.
fn insert(text: &str) -> Option<()> {
    // A page with the keys gets the text typed, as drive.rs's insert types it there: its field is
    // Chromium's, which no GPUI keystroke reaches.
    if keyboard().1 == "RenderWidgetHostViewCocoa" {
        return text
            .chars()
            .try_for_each(|c| key_events(&c.to_string(), 0, &[true, false]));
    }
    let nodes = ours();
    let view = match gpui_linux::shown_keyboard_overlay() {
        Some(view) => view,
        None if focused_settings(&nodes).is_some() => settings_window::shown()?,
        None => {
            let id = driven_id()?;
            WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == id).map(|w| w.ui_view))?
        }
    };
    let cx = CX.with_borrow(Clone::clone)?;
    let typed = Keystroke {
        modifiers: Modifiers::default(),
        key: String::new(),
        key_char: Some(text.to_string()),
    };
    cx.update(|cx| {
        let handle = cx.windows().into_iter().find(|handle| {
            handle
                .update(cx, |_, window, _| crate::ns_view(window))
                .is_ok_and(|each| each == view)
        })?;
        handle
            .update(cx, |_, window, cx| window.dispatch_keystroke(typed, cx))
            .ok()
    })
    .filter(|&typed| typed)
    .map(drop)
}

/// A turn of the wheel by `notches` along `axis` at (`x`, `y`) in sway's layout, down or right
/// for a positive one.
fn wheel(x: f64, y: f64, axis: wl_pointer::Axis, notches: i32) -> Option<()> {
    motion(x, y)?;
    with_modifiers(0, || {
        with_session(|session, time| {
            for _ in 0..notches.unsigned_abs() {
                session.pointer.axis_source(wl_pointer::AxisSource::Wheel);
                let notch = notches.signum();
                session
                    .pointer
                    .axis_discrete(time, axis, 15. * f64::from(notch), notch);
                session.pointer.frame();
            }
        })
    })
}

/// Two fingers on the touchpad at (`x`, `y`), as the Mac's driver has them: `count` scrolls 8 ms
/// apart, each `by` points right and down, then the fingers lift `rest` ms after the last, which
/// ends the touch's phases. Done once they have.
fn swipe(x: f64, y: f64, by: (f64, f64), count: u32, rest: u32) -> Option<()> {
    motion(x, y)?;
    fingers(by)?;
    drive::go_on(Box::new(move |done| swiping(by, count - 1, rest, done)));
    Some(())
}

/// The swipe's `left` scrolls, then its fingers lifting.
fn swiping(by: (f64, f64), left: u32, rest: u32, done: Box<dyn FnOnce()>) {
    if left > 0 {
        drive::after(
            8,
            Box::new(move || {
                _ = fingers(by);
                swiping(by, left - 1, rest, done);
            }),
        );
        return;
    }
    drive::after(
        rest,
        Box::new(move || {
            _ = with_session(|session, time| {
                for axis in [
                    wl_pointer::Axis::HorizontalScroll,
                    wl_pointer::Axis::VerticalScroll,
                ] {
                    session.pointer.axis_stop(time, axis);
                    session.pointer.axis_source(wl_pointer::AxisSource::Finger);
                }
                session.pointer.frame();
            });
            done();
        }),
    );
}

/// One scroll of two fingers, `dx` points right and `dy` down. GPUI scrolls a touchpad's axis
/// value three times over, the other way. Each axis's source comes after it: wlroots gives a source
/// to the axis named last, and sway aborts on a frame whose axes have different ones.
fn fingers((dx, dy): (f64, f64)) -> Option<()> {
    with_session(|session, time| {
        let axes = [
            (wl_pointer::Axis::HorizontalScroll, dx),
            (wl_pointer::Axis::VerticalScroll, dy),
        ];
        for (axis, points) in axes.into_iter().filter(|&(_, points)| points != 0.) {
            session.pointer.axis(time, axis, -points / 3.);
            session.pointer.axis_source(wl_pointer::AxisSource::Finger);
        }
        session.pointer.frame();
    })
}

fn layout() -> Option<()> {
    let (id, node) = driven()?;
    let (mut width, mut height) = (0., 0.);
    unsafe { bc::bc_window_wayland_surface(id, &mut width, &mut height) };
    let pages = drive::pages(id, width, height).unwrap_or_default();
    let ring = drive::ring(id, height).unwrap_or_default();
    let size = format!("{width:.0}x{height:.0}");
    log!("browser: layout content {size}, GPUIView 0,0 {size}{pages}{ring}");
    log!(
        "browser: layout buttons {}",
        drive::buttons(id).unwrap_or_default()
    );
    if let Some(dialog) = drive::dialog(id) {
        log!("browser: layout child {dialog}");
    }
    for (each, bounds) in drive::overlay_views().into_iter().filter_map(overlay_frame) {
        if each == id {
            let [x, y, width, height] = [
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
            ]
            .map(f32::from);
            log!("browser: layout child {x:.0},{y:.0} {width:.0}x{height:.0}");
        }
    }
    log!(
        "browser: layout keyboard {}",
        drive::keyboard_page(id, height)
    );
    log!("browser: layout full screen {}", node.full_screen as u8);
    Some(())
}

/// Runs `action`, one of the commands whose input or windows are Wayland's here: Some with
/// whether it could, or None for drive.rs's own.
pub(crate) fn run(action: &str, line: &str) -> Option<Option<()>> {
    let mut words = line.split_whitespace();
    words.next();
    let rest = line.get(action.len() + 1..).unwrap_or("");
    Some(match action {
        "window" => (|| {
            let number: usize = words.next()?.parse().ok()?;
            let (_, node) = browser(number.checked_sub(1)?)?;
            DRIVEN.set(number - 1);
            log!("browser: drive window {number}");
            // To the front with the keyboard, as a click on it does.
            sway_command(&format!("[con_id={}] focus", node.id))
        })(),
        "click" | "rightclick" | "middleclick" | "doubleclick" => (|| {
            let (x, y) = drive::point(&mut words)?;
            let (x, y) = in_driven(x, y)?;
            let which = match action {
                "rightclick" => BTN_RIGHT,
                "middleclick" => BTN_MIDDLE,
                _ => BTN_LEFT,
            };
            click(x, y, which, drive::click_modifiers(words), false)?;
            if action == "doubleclick" {
                click(x, y, BTN_LEFT, 0, true)?;
            }
            Some(())
        })(),
        "clickkey" | "rightclickkey" | "doubleclickkey" => (|| {
            let (x, y) = drive::point(&mut words)?;
            await_settings_focus();
            // Settings' close button is the compositor's off the Mac, in its title bar.
            if action == "clickkey"
                && x.max(y) < drive::CLOSE_CORNER
                && let Some(settings) = focused_settings(&ours())
            {
                return sway_command(&format!("[con_id={}] kill", settings.id));
            }
            let (left, top) = key_origin()?;
            let (x, y) = (left + x, top + y);
            let which = if action == "rightclickkey" {
                BTN_RIGHT
            } else {
                BTN_LEFT
            };
            click(x, y, which, 0, false)?;
            if action == "doubleclickkey" {
                click(x, y, BTN_LEFT, 0, true)?;
            }
            Some(())
        })(),
        "drag" => (|| {
            let (x, y) = in_driven_point(&mut words)?;
            let (x2, y2) = in_driven_point(&mut words)?;
            first_press(x, y, BTN_LEFT, false);
            motion(x, y)?;
            with_modifiers(0, || {
                button(BTN_LEFT, true)?;
                motion(x2, y2)?;
                button(BTN_LEFT, false)
            })
        })(),
        "press" | "dragto" | "release" | "rightpress" | "rightdragto" | "rightrelease" => (|| {
            let (x, y) = in_driven_point(&mut words)?;
            let (which, step) = match action.strip_prefix("right") {
                Some(step) => (BTN_RIGHT, step),
                None => (BTN_LEFT, action),
            };
            // The keys named after the point, such as ⌥ for a drag that drops a copy.
            let keys = drive::click_modifiers(words);
            if step == "press" {
                first_press(x, y, which, false);
            }
            with_modifiers(keys, || {
                motion(x, y)?;
                match step {
                    "press" => button(which, true),
                    "release" => button(which, false),
                    _ => Some(()),
                }
            })
        })(
        ),
        "move" => (|| {
            let (x, y) = in_driven_point(&mut words)?;
            motion(x, y)
        })(),
        // A page, Chromium's, scrolls a notch of the wheel an event, down for content moving up, as
        // drive.rs's does. GPUI's windows scroll by the points asked, as the Mac's trackpad events
        // do, with two fingers on the touchpad, where a notch moves three lines.
        "scroll" => (|| {
            let (x, y) = drive::point(&mut words)?;
            let dy: f64 = words.next()?.parse().ok()?;
            let count: i32 = words.next()?.parse().ok()?;
            let over_page = drive::over_page(driven()?.0, x, y);
            let (x, y) = in_driven(x, y)?;
            if !over_page {
                return swipe(x, y, (0., -dy), count.max(1).unsigned_abs(), 0);
            }
            let notches = count.max(1) * if dy > 0. { 1 } else { -1 };
            wheel(x, y, wl_pointer::Axis::VerticalScroll, notches)
        })(),
        "swipe" => (|| {
            let (x, y) = in_driven_point(&mut words)?;
            let dx: f64 = words.next()?.parse().ok()?;
            let dy: f64 = words.next()?.parse().ok()?;
            let count: u32 = words.next()?.parse().ok().filter(|&count| count > 0)?;
            let rest: u32 = words.next().map_or(Some(0), |rest| rest.parse().ok())?;
            swipe(x, y, (dx, dy), count, rest)
        })(),
        // Lines right are the Mac's wheel's positive x, which GPUI reads from a turn left.
        "wheel" => (|| {
            let (x, y) = in_driven_point(&mut words)?;
            let dx: f64 = words.next()?.parse().ok()?;
            let notches = (dx.abs().round() as i32).max(1);
            let notches = if dx > 0. { -notches } else { notches };
            wheel(x, y, wl_pointer::Axis::HorizontalScroll, notches)
        })(),
        "key" => (|| {
            let name = words.next()?;
            let words: Vec<_> = words.collect();
            let mut state = drive::modifiers(words.iter().copied());
            // ⌃Tab stays Ctrl+Tab, as keys.rs presses it, since Alt+Tab is the desktop's.
            if name == "tab" && words.contains(&"ctrl") && !words.contains(&"cmd") {
                let option = if words.contains(&"alt") { ALT } else { 0 };
                state = state & !ALT | CONTROL | option;
            }
            key(name, state)
        })(),
        // Each modifier going down or up as its key does, after the modifiers the last key went
        // with, which are held until the flags change, as on drive.rs's X server.
        "flags" => (|| {
            let to = drive::modifiers(words);
            FLAGS.set(FLAGS.get() | KEY_FLAGS.take());
            for (mask, code) in MODIFIER_KEYS {
                let held = FLAGS.get();
                let press = match (held & mask != 0, to & mask != 0) {
                    (false, true) => true,
                    (true, false) => false,
                    _ => continue,
                };
                with_session(|session, time| session.keyboard.key(time, code, u32::from(press)))?;
                FLAGS.set(held ^ mask);
                hold(FLAGS.get())?;
            }
            Some(())
        })(),
        "keydown" => (|| key_events(words.next()?, 0, &[true]))(),
        "keyup" => (|| key_events(words.next()?, 0, &[false]))(),
        "type" => rest.chars().try_for_each(|c| key(&c.to_string(), 0)),
        "insert" => insert(rest),
        "resize" | "place" => (|| {
            let numbers: Vec<f64> = words.map(|w| w.parse().ok()).collect::<Option<_>>()?;
            match numbers[..] {
                [width, height] if action == "resize" => frame(None, width, height),
                [x, y, width, height] if action == "place" => frame(Some((x, y)), width, height),
                _ => None,
            }
        })(),
        "order" => {
            let mut order = String::new();
            for (id, node) in stacked() {
                let name = name_of(id);
                order += &format!(" {name}");
                let [x, y, width, height] = [node.x, node.y, node.width, node.height];
                log!("browser: frame {name} {x:.0} {y:.0} {width:.0} {height:.0}");
            }
            let key = focused_browser().map_or("none".to_string(), name_of);
            log!("browser: order{order}, key {key}");
            Some(())
        }
        "front" => {
            let driven = driven_id();
            let describe = |id: Option<bc::bc_id>| match id {
                Some(id) if Some(id) == driven => "driven".to_string(),
                Some(id) => name_of(id),
                None => "none".to_string(),
            };
            let first = describe(stacked().first().map(|&(id, _)| id));
            let key = describe(focused_browser());
            log!("browser: front {first}, main {key}, key {key}, ax focused {key}, ax main {key}");
            Some(())
        }
        // Handed to the app as drive.rs hands it, in the browser window's surface, where a
        // touchpad's swipe comes from (gpui_linux::watch_embedder_swipes).
        "pageswipe" => (|| {
            let (x, y) = drive::point(&mut words)?;
            let back = match words.next()? {
                "back" => true,
                "forward" => false,
                _ => return None,
            };
            let id = driven_id()?;
            let surface = WINDOWS
                .with_borrow(|windows| windows.iter().find(|w| w.id == id).map(|w| w.ui_view))?;
            crate::swiped(surface, x, y, back);
            Some(())
        })(),
        "dragover" | "dragdrop" | "dragleave" => None,
        "layout" => layout(),
        _ => return None,
    })
}

/// Two numbers from `words`, a point in the driven browser window's content, in sway's layout.
fn in_driven_point<'a>(words: &mut impl Iterator<Item = &'a str>) -> Option<(f64, f64)> {
    let (x, y) = drive::point(words)?;
    in_driven(x, y)
}
