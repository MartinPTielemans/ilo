//! Drags from other apps onto the UI, as host_take_drops says.
//! Under X11 (freedesktop.org's XDND, version 5), a source hands its drag to the top-level window
//! under the pointer, which is Chromium's, whose Aura hands drops only to pages, and Chromium's own
//! drags go to the deepest named window, which may be the UI's. So the XdndProxy of each browser
//! window and of its UI's windows names a window of the app's, which takes the drag where the UI is
//! and hands it on to Chromium's window where a page is. Chromium answers whichever window its
//! messages say they come from, so the proxy stands in for the source there, and passes Chromium's
//! answers back.
//! Under Wayland the compositor hands a drag to the surface under the pointer, the page's
//! Chromium's and the UI's GPUI's, whose data device hands it here (`take_embedder_drops`).

use std::{
    cell::{Cell, RefCell},
    ffi::{CString, c_int, c_void},
    os::fd::AsRawFd,
    rc::Rc,
};

use x11rb::{
    connection::Connection,
    protocol::{
        Event,
        xproto::{
            self, AtomEnum, ClientMessageEvent, ConnectionExt as _, CreateWindowAux, EventMask,
            PropMode, WindowClass,
        },
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

use crate::{WINDOWS, bc, platform};

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        XdndAware,
        XdndProxy,
        XdndEnter,
        XdndPosition,
        XdndStatus,
        XdndLeave,
        XdndDrop,
        XdndFinished,
        XdndSelection,
        XdndTypeList,
        XdndActionCopy,
        UTF8_STRING,
        URI_LIST: b"text/uri-list",
        TEXT_UTF8: b"text/plain;charset=utf-8",
        TEXT: b"text/plain",
        ILO_TAB: b"application/x-ilo-tab",
        ILO_DROPPED,
    }
}

type Dragged =
    extern "C" fn(*mut c_void, c_int, f64, f64, *const platform::host_dragged, usize) -> bool;

/// The app's own connection, the proxy window on it, and the atoms.
struct Proxy {
    connection: RustConnection,
    root: xproto::Window,
    window: xproto::Window,
    atoms: Atoms,
}

/// Where a drag is: over the UI's window in a browser window, at a point in its pixels, or over
/// the pages of the browser window whose top-level window is Chromium's.
#[derive(Clone, Copy)]
enum Over {
    Ui(xproto::Window, i16, i16),
    Page(xproto::Window),
}

/// The drag over one of the app's windows.
struct Drag {
    source: xproto::Window,
    /// The window the source's messages name, which its answers name too.
    target: xproto::Window,
    /// The source's XdndEnter, which Chromium hears as the drag reaches a page.
    enter: [u32; 5],
    types: Vec<xproto::Atom>,
    over: Option<Over>,
    /// What it carries once read, its URLs and whether each is a file's, or the app's tab.
    items: Option<Vec<(String, bool)>>,
    tab: u64,
    asked: bool,
    /// Over the UI before the items were read: the answer waits for them, as a source waits for
    /// one before it says more.
    waiting: bool,
}

thread_local! {
    static PROXY: RefCell<Option<Rc<Proxy>>> = const { RefCell::new(None) };
    static DRAG: RefCell<Option<Drag>> = const { RefCell::new(None) };
    static DRAGGED: Cell<Option<Dragged>> = const { Cell::new(None) };
}

#[unsafe(no_mangle)]
pub extern "C" fn host_take_drops(dragged: Dragged) {
    DRAGGED.set(Some(dragged));
    gpui_linux::take_embedder_drops(&TYPES, dropped);
}

/// The type of a tab host_drag_tab drags, this process's id and the tab's, which the UI takes only
/// from this process.
const TAB_TYPE: &str = "application/x-ilo-tab";

/// The app's tab `data` carries, as host_drag_tab writes it, else 0.
fn tab_of(data: &[u8]) -> u64 {
    let text = String::from_utf8_lossy(data);
    let mut words = text.split_whitespace();
    let pid = words.next().and_then(|pid| pid.parse::<u32>().ok());
    let tab = words.next().and_then(|tab| tab.parse::<u64>().ok());
    match (pid, tab) {
        (Some(pid), Some(tab)) if pid == std::process::id() => tab,
        _ => 0,
    }
}

/// The types the UI reads a drag as: the app's own tab, its links and files, else its text.
const TYPES: [&str; 5] = [
    TAB_TYPE,
    "text/uri-list",
    "UTF8_STRING",
    "text/plain;charset=utf-8",
    "text/plain",
];

/// A drag under Wayland, over the UI in the browser window whose surface is `view`.
fn dropped(
    view: *mut c_void,
    drag: gpui_linux::EmbedderDrag,
    x: f32,
    y: f32,
    carried: Option<(&str, &[u8])>,
) -> bool {
    let tab = carried
        .filter(|(kind, _)| *kind == TAB_TYPE)
        .map_or(0, |(_, data)| tab_of(data));
    let items = match tab {
        0 => carried.map_or_else(Vec::new, |(kind, data)| items(kind == TYPES[1], data)),
        _ => Vec::new(),
    };
    let carries = tab != 0 || !items.is_empty();
    let (x, y) = (f64::from(x), f64::from(y));
    match drag {
        gpui_linux::EmbedderDrag::Over => {
            carries && tell(view, platform::DRAG_OVER, x, y, &items, tab)
        }
        gpui_linux::EmbedderDrag::Left => {
            tell(view, DRAG_LEFT, x, y, &[], 0);
            false
        }
        gpui_linux::EmbedderDrag::Dropped => {
            let taken = carries && tell(view, platform::DRAG_DROP, x, y, &items, tab);
            // After a drop too, as the Mac's draggingEnded, which the UI keeps.
            tell(view, DRAG_LEFT, x, y, &[], 0);
            taken
        }
    }
}

fn proxy() -> Option<Rc<Proxy>> {
    if let Some(proxy) = PROXY.with_borrow(Clone::clone) {
        return Some(proxy);
    }
    let (connection, screen) = x11rb::connect(None).ok()?;
    let root = connection.setup().roots[screen].root;
    let atoms = Atoms::new(&connection).ok()?.reply().ok()?;
    let window = connection.generate_id().ok()?;
    (connection.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        window,
        root,
        -1,
        -1,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        x11rb::COPY_FROM_PARENT,
        &CreateWindowAux::new(),
    ))
    .ok()?;
    // A proxy names itself, which sources check so as not to follow a stale one.
    (connection.change_property32(
        PropMode::REPLACE,
        window,
        atoms.XdndAware,
        AtomEnum::ATOM,
        &[5],
    ))
    .ok()?;
    (connection.change_property32(
        PropMode::REPLACE,
        window,
        atoms.XdndProxy,
        AtomEnum::WINDOW,
        &[window],
    ))
    .ok()?;
    connection.flush().ok()?;
    let fd = connection.stream().as_raw_fd();
    let proxy = Rc::new(Proxy {
        connection,
        root,
        window,
        atoms,
    });
    PROXY.set(Some(proxy.clone()));
    extern "C" fn ready() {
        pump();
    }
    unsafe { platform::host_watch_fd(fd, ready) };
    Some(proxy)
}

/// Takes the drags over the browser window whose UI's window is `ui_view`, under X11.
pub(crate) fn take(ui_view: *mut c_void) {
    let Some(proxy) = proxy() else { return };
    let connection = &proxy.connection;
    let ui = ui_view as usize as u32;
    let Some(tree) = connection.query_tree(ui).ok().and_then(|c| c.reply().ok()) else {
        return;
    };
    // Chromium's window holds the UI's by now; the root's proxy would take every app's drags.
    if tree.parent == tree.root {
        return;
    }
    for window in [tree.parent, ui].into_iter().chain(tree.children) {
        _ = connection.change_property32(
            PropMode::REPLACE,
            window,
            proxy.atoms.XdndProxy,
            AtomEnum::WINDOW,
            &[proxy.window],
        );
    }
    _ = connection.flush();
    pump();
}

/// Handles what the X server has sent the proxy, and what it read while waiting for a reply.
fn pump() {
    let Some(proxy) = PROXY.with_borrow(Clone::clone) else {
        return;
    };
    while let Ok(Some(event)) = proxy.connection.poll_for_event() {
        match event {
            Event::ClientMessage(message) if message.format == 32 => handle(&proxy, &message),
            Event::SelectionNotify(notify) if notify.requestor == proxy.window => {
                read_items(&proxy, notify.property);
            }
            _ => {}
        }
    }
}

fn send(proxy: &Proxy, to: xproto::Window, kind: xproto::Atom, data: [u32; 5]) {
    let message = ClientMessageEvent::new(32, to, kind, data);
    _ = proxy
        .connection
        .send_event(false, to, EventMask::NO_EVENT, message);
    _ = proxy.connection.flush();
}

/// Sends Chromium's window `page` the source's message, as from the proxy.
fn to_page(proxy: &Proxy, page: xproto::Window, kind: xproto::Atom, mut data: [u32; 5]) {
    data[0] = proxy.window;
    send(proxy, page, kind, data);
}

/// Answers the source, as the window it sent to.
fn to_source(proxy: &Proxy, drag: &Drag, kind: xproto::Atom, mut data: [u32; 5]) {
    data[0] = drag.target;
    send(proxy, drag.source, kind, data);
}

fn handle(proxy: &Proxy, message: &ClientMessageEvent) {
    let atoms = &proxy.atoms;
    let data = message.data.as_data32();
    let kind = message.type_;
    if kind == atoms.XdndEnter {
        let types = match data[1] & 1 {
            0 => data[2..]
                .iter()
                .copied()
                .filter(|&atom| atom != 0)
                .collect(),
            _ => type_list(proxy, data[0]),
        };
        // Chromium reads the list from the window the drag says it comes from.
        _ = (proxy.connection).change_property32(
            PropMode::REPLACE,
            proxy.window,
            atoms.XdndTypeList,
            AtomEnum::ATOM,
            &types,
        );
        let drag = Drag {
            source: data[0],
            target: message.window,
            enter: data,
            types,
            over: None,
            items: None,
            tab: 0,
            asked: false,
            waiting: false,
        };
        DRAG.set(Some(drag));
        return;
    }
    let Some(mut drag) = DRAG.take() else { return };
    let page = match drag.over {
        Some(Over::Page(page)) => Some(page),
        _ => None,
    };
    // Chromium's answers, from the page's window, which go back to the source.
    if kind == atoms.XdndStatus || kind == atoms.XdndFinished {
        let from_page = page == Some(data[0]);
        if from_page {
            to_source(proxy, &drag, kind, data);
        }
        if !(from_page && kind == atoms.XdndFinished) {
            DRAG.set(Some(drag));
        }
        return;
    }
    if data[0] != drag.source {
        DRAG.set(Some(drag));
        return;
    }
    if kind == atoms.XdndPosition {
        let (x, y) = ((data[2] >> 16) as i16, data[2] as i16);
        let over = locate(proxy, x, y);
        leave(proxy, &mut drag, over);
        drag.over = over;
        match over {
            Some(Over::Page(page)) => to_page(proxy, page, kind, data),
            Some(Over::Ui(..)) if drag.items.is_some() => answer(proxy, &drag),
            Some(Over::Ui(..)) => match wanted(proxy, &drag) {
                Some(wanted) => {
                    drag.waiting = true;
                    if !std::mem::replace(&mut drag.asked, true) {
                        ask_items(proxy, wanted, data[3]);
                    }
                }
                None => {
                    drag.items = Some(Vec::new());
                    answer(proxy, &drag);
                }
            },
            None => to_source(proxy, &drag, atoms.XdndStatus, [0, 2, 0, 0, 0]),
        }
        DRAG.set(Some(drag));
    } else if kind == atoms.XdndLeave {
        leave(proxy, &mut drag, None);
    } else if kind == atoms.XdndDrop {
        match drag.over {
            Some(Over::Page(page)) => {
                to_page(proxy, page, kind, data);
                // Until Chromium says it's finished.
                DRAG.set(Some(drag));
            }
            Some(Over::Ui(ui, x, y)) => {
                let taken = drag
                    .items
                    .as_ref()
                    .is_some_and(|items| tell_ui(ui, platform::DRAG_DROP, x, y, items, drag.tab));
                // After a drop too, as the Mac's draggingEnded, which the UI keeps.
                tell_ui(ui, DRAG_LEFT, x, y, &[], 0);
                let action = if taken { atoms.XdndActionCopy } else { 0 };
                to_source(
                    proxy,
                    &drag,
                    atoms.XdndFinished,
                    [0, taken as u32, action, 0, 0],
                );
            }
            None => to_source(proxy, &drag, atoms.XdndFinished, [0, 0, 0, 0, 0]),
        }
    }
}

const DRAG_LEFT: c_int = 1;

/// Tells whichever of the UI and Chromium the drag was over that it left, unless it's still `over`
/// that, and Chromium that it came if it's over a page now.
fn leave(proxy: &Proxy, drag: &mut Drag, over: Option<Over>) {
    let page = |over: Option<Over>| match over {
        Some(Over::Page(page)) => Some(page),
        _ => None,
    };
    let ui = |over: Option<Over>| match over {
        Some(Over::Ui(ui, ..)) => Some(ui),
        _ => None,
    };
    if let Some(left) = page(drag.over).filter(|&left| page(over) != Some(left)) {
        to_page(proxy, left, proxy.atoms.XdndLeave, [0; 5]);
    }
    if let Some(left) = ui(drag.over).filter(|&left| ui(over) != Some(left)) {
        tell_ui(left, DRAG_LEFT, 0, 0, &[], 0);
    }
    if let Some(came) = page(over).filter(|&came| page(drag.over) != Some(came)) {
        to_page(proxy, came, proxy.atoms.XdndEnter, drag.enter);
    }
}

/// The browser window at (`x`, `y`) on the root, and whether it's the UI there or a page, as the X
/// server picks the window under the pointer: the UI's window's shape leaves holes for the pages.
fn locate(proxy: &Proxy, x: i16, y: i16) -> Option<Over> {
    let connection = &proxy.connection;
    let at = |window: xproto::Window| {
        Some(
            connection
                .translate_coordinates(proxy.root, window, x, y)
                .ok()?
                .reply()
                .ok()?,
        )
    };
    let top = at(proxy.root)?.child;
    let uis: Vec<u32> =
        WINDOWS.with_borrow(|windows| windows.iter().map(|w| w.ui_view as usize as u32).collect());
    uis.into_iter().find_map(|ui| {
        let toplevel = connection.query_tree(ui).ok()?.reply().ok()?.parent;
        // A window manager's frame holds Chromium's window.
        let mut window = toplevel;
        while window != top {
            let tree = connection.query_tree(window).ok()?.reply().ok()?;
            (tree.parent != tree.root).then_some(())?;
            window = tree.parent;
        }
        Some(match at(toplevel)?.child == ui {
            true => {
                let inside = at(ui)?;
                Over::Ui(ui, inside.dst_x, inside.dst_y)
            }
            false => Over::Page(toplevel),
        })
    })
}

fn type_list(proxy: &Proxy, source: xproto::Window) -> Vec<xproto::Atom> {
    let reply = (proxy.connection)
        .get_property(
            false,
            source,
            proxy.atoms.XdndTypeList,
            AtomEnum::ATOM,
            0,
            64,
        )
        .ok()
        .and_then(|cookie| cookie.reply().ok());
    reply
        .and_then(|reply| Some(reply.value32()?.collect()))
        .unwrap_or_default()
}

/// The first of TYPES the drag has.
fn wanted(proxy: &Proxy, drag: &Drag) -> Option<xproto::Atom> {
    let atoms = &proxy.atoms;
    [
        atoms.ILO_TAB,
        atoms.URI_LIST,
        atoms.UTF8_STRING,
        atoms.TEXT_UTF8,
        atoms.TEXT,
    ]
    .into_iter()
    .find(|kind| drag.types.contains(kind))
}

/// Asks the source for what the drag carries, as `kind`, which comes in a SelectionNotify.
fn ask_items(proxy: &Proxy, kind: xproto::Atom, time: u32) {
    _ = proxy.connection.convert_selection(
        proxy.window,
        proxy.atoms.XdndSelection,
        kind,
        proxy.atoms.ILO_DROPPED,
        time,
    );
    _ = proxy.connection.flush();
}

/// Reads what the drag carries, which the source has put in `property`, or none if it's NONE.
fn read_items(proxy: &Proxy, property: xproto::Atom) {
    let reply = (property != x11rb::NONE)
        .then(|| {
            (proxy.connection)
                .get_property(true, proxy.window, property, AtomEnum::ANY, 0, 1 << 20)
                .ok()?
                .reply()
                .ok()
        })
        .flatten();
    let tab = (reply.as_ref())
        .filter(|reply| reply.type_ == proxy.atoms.ILO_TAB)
        .map_or(0, |reply| tab_of(&reply.value));
    let items = match tab {
        0 => reply.map_or_else(Vec::new, |reply| {
            items(reply.type_ == proxy.atoms.URI_LIST, &reply.value)
        }),
        _ => Vec::new(),
    };
    read_done(proxy, items, tab);
}

fn read_done(proxy: &Proxy, items: Vec<(String, bool)>, tab: u64) {
    DRAG.with_borrow_mut(|drag| {
        let Some(drag) = drag.as_mut() else { return };
        drag.items = Some(items);
        drag.tab = tab;
        if std::mem::take(&mut drag.waiting) && matches!(drag.over, Some(Over::Ui(..))) {
            answer(proxy, drag);
        }
    });
}

/// The links and files in `data`, a URI list or text, as the Mac's HostUIView reads them: the
/// files a URI list holds, else its links, else the text, trimmed, which the UI opens only if it's
/// a link.
fn items(uri_list: bool, data: &[u8]) -> Vec<(String, bool)> {
    let text = String::from_utf8_lossy(data);
    if uri_list {
        let urls: Vec<&str> = (text.lines().map(str::trim))
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();
        let files: Vec<_> = (urls.iter())
            .filter(|url| url.starts_with("file:"))
            .map(|url| (url.to_string(), true))
            .collect();
        return match files.is_empty() {
            true => urls
                .into_iter()
                .map(|url| (url.to_string(), false))
                .collect(),
            false => files,
        };
    }
    let text = text.trim_matches(|c: char| c.is_whitespace() || c == '\0');
    match text.is_empty() {
        true => Vec::new(),
        false => vec![(text.to_string(), false)],
    }
}

/// Tells the UI the drag is over it, and the source whether a drop there would open something.
fn answer(proxy: &Proxy, drag: &Drag) {
    let Some(Over::Ui(ui, x, y)) = drag.over else {
        return;
    };
    let items = drag.items.as_deref().unwrap_or_default();
    let taken = (drag.tab != 0 || !items.is_empty())
        && tell_ui(ui, platform::DRAG_OVER, x, y, items, drag.tab);
    let action = if taken { proxy.atoms.XdndActionCopy } else { 0 };
    // Bit 1 asks for each move, as the UI's drop line follows the pointer.
    to_source(
        proxy,
        drag,
        proxy.atoms.XdndStatus,
        [0, taken as u32 | 2, 0, 0, action],
    );
}

/// Calls the UI's `dragged` for the UI's window `ui`, at (`x`, `y`) in its pixels.
fn tell_ui(
    ui: xproto::Window,
    phase: c_int,
    x: i16,
    y: i16,
    items: &[(String, bool)],
    tab: u64,
) -> bool {
    let scale = unsafe { bc::bc_display_scale() };
    let (x, y) = (f64::from(x) / scale, f64::from(y) / scale);
    tell(ui as usize as *mut c_void, phase, x, y, items, tab)
}

/// Calls the UI's `dragged` for the UI's view `view`, at (`x`, `y`) in its points, with the links
/// and files the drag carries, or the app's `tab` if it isn't 0.
fn tell(
    view: *mut c_void,
    phase: c_int,
    x: f64,
    y: f64,
    items: &[(String, bool)],
    tab: u64,
) -> bool {
    let Some(dragged) = DRAGGED.get() else {
        return false;
    };
    if tab != 0 {
        let item = platform::host_dragged {
            url: c"".as_ptr(),
            file: false,
            tab,
        };
        return dragged(view, phase, x, y, &item, 1);
    }
    let urls: Vec<CString> = (items.iter())
        .filter_map(|(url, _)| CString::new(url.as_str()).ok())
        .collect();
    let items: Vec<platform::host_dragged> = (urls.iter().zip(items))
        .map(|(url, &(_, file))| platform::host_dragged {
            url: url.as_ptr(),
            file,
            tab: 0,
        })
        .collect();
    dragged(view, phase, x, y, items.as_ptr(), items.len())
}
