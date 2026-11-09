//! The Wayland driver's drags from another app (drive_wayland.rs): dragover, dragdrop and
//! dragleave, which drive.rs makes X11's through XDND. Wayland lets a client drag only what it
//! offers, from a surface of its own, with the serial of a press on it. So the driver has a client
//! of its own, as another app is, on a thread of its own: a small clear surface in the corner of
//! the output with sway's focus, on its overlay layer, which takes no keyboard, so the browser
//! window keeps it. The driver's virtual pointer presses there, the client starts the drag with
//! what it carries, and the pointer carries it over the window, where the compositor hands it to
//! the surface under it, Chromium's on a page and GPUI's on the UI, as it hands a real app's.
//!
//! The answer each dragover logs, "browser: drag over <view> taken" or "refused", is what the
//! source last heard: the type the target accepts, if any, and the action it agreed to. GPUI's
//! surface answers each move, and the answer counts once it has come and the source has heard
//! nothing more for a moment. Chromium accepts on entering and then tells the source only when its
//! answer changes, so over a page no word for a second is the answer standing, and the pointer
//! moves a point back and forth meanwhile, as a hand does, since Chromium hands a page the drag
//! only as it moves. dragdrop then lets go, and the source hears whether the target took it, as
//! XDND's source does: "browser: drag dropped", or "browser: drag refused". dragleave carries the
//! drag back to its own surface and lets go there, where nothing takes it.

use std::{
    cell::Cell,
    io::Write as _,
    os::fd::AsFd as _,
    sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, mpsc},
    time::{Duration, Instant},
};

use browser::log;
use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum, delegate_noop, event_created_child,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{
        wl_buffer, wl_callback, wl_compositor, wl_data_device, wl_data_device_manager,
        wl_data_offer, wl_data_source, wl_pointer, wl_registry, wl_seat, wl_shm, wl_shm_pool,
        wl_surface,
    },
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::{drive, drive_wayland};

/// The source's surface, a square this many points across.
const SIZE: i32 = 8;
/// How long the driver waits for its client to have done what it asked, as its surface showing.
const STEP: Duration = Duration::from_secs(2);
/// After a move, how long the source hears nothing more before GPUI's answer counts; and over a
/// page, how long Chromium's word stands, and how long it has to say nothing for that.
const SETTLED: Duration = Duration::from_millis(100);
const STANDS: Duration = Duration::from_secs(1);
const QUIET: Duration = Duration::from_millis(500);
/// How long the target gets before its silence is taken for a no, as drive.rs's XDND source gives.
const UNANSWERED: Duration = Duration::from_secs(5);
/// How often the pointer moves a point over a page while the answer waits.
const NUDGE: Duration = Duration::from_millis(100);

/// What the client has heard, which the driver reads.
#[derive(Default)]
struct Heard {
    /// The drag it's of, by the number the driver gave it.
    drag: u32,
    /// The source's surface has shown, the pointer has come into it, and the drag has started.
    shown: bool,
    entered: bool,
    started: bool,
    /// The type the target accepts, how many times it has said, and the action it agreed to.
    accepted: Option<String>,
    targets: u32,
    action: u32,
    /// When the source last heard anything of the drag.
    last: Option<Instant>,
    /// How the drag ended: taken, as the target finished with it, or not.
    ended: Option<bool>,
}

/// What the driver asks its client: a drag of the types and data given, or the drag's end.
enum Ask {
    Start(u32, Vec<(String, Vec<u8>)>),
    End,
}

/// The driver's side of its client: how to ask it, and wake it to read what was asked.
struct Client {
    connection: Connection,
    queue: QueueHandle<State>,
    asks: Mutex<mpsc::Sender<Ask>>,
    heard: Arc<(Mutex<Heard>, Condvar)>,
}

impl Client {
    fn ask(&self, ask: Ask) -> Option<()> {
        self.asks.lock().ok()?.send(ask).ok()?;
        self.connection.display().sync(&self.queue, Synced::Woken);
        self.connection.flush().ok()
    }

    fn heard(&self) -> MutexGuard<'_, Heard> {
        self.heard
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Waits up to STEP for `done` to be true of what the client has heard.
    fn await_heard(&self, done: impl Fn(&Heard) -> bool) -> bool {
        let (heard, changed) = &*self.heard;
        let heard = heard
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (heard, _) = changed
            .wait_timeout_while(heard, STEP, |heard| !done(heard))
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        done(&heard)
    }
}

/// What the driver waits for after a dragover or dragdrop: the target's answer at `at` in sway's
/// layout, over `view`, which counts once the source has heard more than `targets` answers, and
/// then with `drop` lets go; or, after the drop, whether the target took it.
#[derive(Clone, Copy)]
enum Waiting {
    Answer {
        drop: bool,
        since: Instant,
        targets: u32,
        view: &'static str,
        at: (f64, f64),
        page: bool,
        nudged: Instant,
        aside: bool,
    },
    Taken(Instant),
}

thread_local! {
    /// A drag is held, from the point in sway's layout where it started.
    static DRAGGING: Cell<Option<(f64, f64)>> = const { Cell::new(None) };
    static WAITING: Cell<Option<Waiting>> = const { Cell::new(None) };
    /// The last drag's number.
    static DRAGS: Cell<u32> = const { Cell::new(0) };
}

static CLIENT: OnceLock<Option<Client>> = OnceLock::new();

fn client() -> Option<&'static Client> {
    CLIENT
        .get_or_init(|| match connect() {
            Ok(client) => Some(client),
            Err(why) => {
                log!("browser: drive has no drags under Wayland: {why}");
                None
            }
        })
        .as_ref()
}

/// Moves the drag to (`x`, `y`) in the driven browser window, starting it with what `carried` says
/// if there's none, and has the target say whether it takes it there, then with `drop` let go.
pub(crate) fn over(x: f64, y: f64, carried: Option<(&str, &str)>, drop: bool) -> Option<()> {
    let (id, at) = drive_wayland::in_driven_window(x, y)?;
    if DRAGGING.get().is_none() {
        let (kind, value) = carried?;
        start(kind, value)?;
    }
    drive_wayland::pointer_to(at.0, at.1)?;
    let page = drive::over_page(id, x, y);
    let now = Instant::now();
    WAITING.set(Some(Waiting::Answer {
        drop,
        since: now,
        targets: client()?.heard().targets,
        // AppKit's names for the views a drag there goes to, as drive.rs's.
        view: if page {
            "WebContentsViewCocoa"
        } else {
            "HostUIView"
        },
        at,
        page,
        nudged: now,
        aside: false,
    }));
    Some(())
}

/// Starts a drag of `kind` carrying `value`, as drive.rs's XDND source carries it: a URL as a link
/// is, as Chromium and GPUI read one, a file's as a URI list, and text as text; or the app's tab,
/// as host_drag_tab carries one from another of its windows.
fn start(kind: &str, value: &str) -> Option<()> {
    let text = |value: &str| value.as_bytes().to_vec();
    let data: Vec<(&str, Vec<u8>)> = match kind {
        // A browser's link: Chromium reads a URI list alone as files.
        "url" => vec![
            (
                "text/x-moz-url",
                format!("{value}\r\n{value}")
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect(),
            ),
            ("text/uri-list", text(&format!("{value}\r\n"))),
            ("text/plain;charset=utf-8", text(value)),
        ],
        "file" => vec![(
            "text/uri-list",
            text(&format!("{}\r\n", drive::file_url(value))),
        )],
        "text" => vec![
            ("text/plain;charset=utf-8", text(value)),
            ("text/plain", text(value)),
        ],
        "tab" => vec![(
            "application/x-ilo-tab",
            text(&format!("{} {value}", std::process::id())),
        )],
        _ => return None,
    };
    let data = data
        .into_iter()
        .map(|(kind, data)| (kind.to_string(), data));
    let client = client()?;
    let number = DRAGS.get() + 1;
    DRAGS.set(number);
    client.ask(Ask::Start(number, data.collect()))?;
    // What the client hears of this drag, not of the last one.
    let of_it = |heard: &Heard, what: fn(&Heard) -> bool| heard.drag == number && what(heard);
    let corner = drive_wayland::output_corner(f64::from(SIZE))?;
    let fail = |why: &str| {
        log!("browser: drive couldn't start a drag: {why}");
        _ = client.ask(Ask::End);
        None
    };
    if !client.await_heard(|heard| of_it(heard, |heard| heard.shown)) {
        return fail("its surface didn't show");
    }
    drive_wayland::pointer_to(corner.0, corner.1)?;
    if !client.await_heard(|heard| of_it(heard, |heard| heard.entered)) {
        return fail("the pointer didn't come into its surface");
    }
    drive_wayland::left_button(true)?;
    if !client.await_heard(|heard| of_it(heard, |heard| heard.started)) {
        drive_wayland::left_button(false);
        return fail("the compositor didn't start it");
    }
    DRAGGING.set(Some(corner));
    Some(())
}

/// Carries the drag back to its own surface and lets go there, where nothing takes it, as a drag
/// cancelled is: the target hears it leave.
pub(crate) fn leave() {
    WAITING.set(None);
    let Some((x, y)) = DRAGGING.get() else {
        return;
    };
    if drive_wayland::pointer_to(x, y).is_some() {
        drive_wayland::left_button(false);
    }
    if let Some(client) = client() {
        client.await_heard(|heard| heard.ended.is_some());
    }
    end();
}

fn end() {
    WAITING.set(None);
    DRAGGING.set(None);
    if let Some(client) = client() {
        _ = client.ask(Ask::End);
    }
}

/// Whether the drag waits for its target's answer, from the loop that reports the command done,
/// which asks again until it doesn't: logs the answer, and with a drop lets go and waits for the
/// target to have taken it or not.
pub(crate) fn waits() -> bool {
    let (Some(waiting), Some(client)) = (WAITING.get(), CLIENT.get().and_then(Option::as_ref))
    else {
        return false;
    };
    let (accepted, targets, action, last, ended) = {
        let heard = client.heard();
        let accepted = heard.accepted.is_some();
        (
            accepted,
            heard.targets,
            heard.action,
            heard.last,
            heard.ended,
        )
    };
    match waiting {
        Waiting::Answer {
            drop,
            since,
            targets: before,
            view,
            at,
            page,
            nudged,
            aside,
        } => {
            if ended.is_some() {
                log!("browser: drag over {view} refused");
                log!("browser: drag refused, as the compositor ended it");
                end();
                return false;
            }
            let quiet = last.map_or(since, |last| last.max(since)).elapsed();
            let answered = (targets > before && quiet >= SETTLED)
                || (since.elapsed() >= STANDS && quiet >= QUIET)
                || since.elapsed() >= UNANSWERED;
            if !answered {
                if page && nudged.elapsed() >= NUDGE {
                    let x = if aside { at.0 } else { at.0 + 1. };
                    _ = drive_wayland::pointer_to(x, at.1);
                    WAITING.set(Some(Waiting::Answer {
                        drop,
                        since,
                        targets: before,
                        view,
                        at,
                        page,
                        nudged: Instant::now(),
                        aside: !aside,
                    }));
                }
                return true;
            }
            if aside {
                _ = drive_wayland::pointer_to(at.0, at.1);
            }
            let taken = accepted && action != 0;
            match since.elapsed() >= UNANSWERED && targets == before {
                true => log!(
                    "browser: drag over {view} {}, unanswered",
                    if taken { "taken" } else { "refused" }
                ),
                false => log!(
                    "browser: drag over {view} {}",
                    if taken { "taken" } else { "refused" }
                ),
            }
            if !drop {
                WAITING.set(None);
                return false;
            }
            // Let go there: the compositor drops it on a target that took it, and cancels it on
            // one that didn't.
            drive_wayland::left_button(false);
            WAITING.set(Some(Waiting::Taken(Instant::now())));
            true
        }
        Waiting::Taken(since) => match ended {
            Some(taken) => {
                log!(
                    "browser: drag {}",
                    if taken { "dropped" } else { "refused" }
                );
                end();
                false
            }
            None if since.elapsed() >= UNANSWERED => {
                log!("browser: drag refused, unanswered");
                end();
                false
            }
            None => true,
        },
    }
}

/// The client's own connection, its globals bound, and its thread, which reads its events.
fn connect() -> Result<Client, String> {
    let connection = Connection::connect_to_env().map_err(|e| e.to_string())?;
    let (globals, queue) = registry_queue_init::<State>(&connection).map_err(|e| e.to_string())?;
    let qh = queue.handle();
    let bind = |what: &str| format!("the compositor has no {what}");
    let compositor: wl_compositor::WlCompositor = globals
        .bind(&qh, 1..=4, ())
        .map_err(|_| bind("compositor"))?;
    let shm: wl_shm::WlShm = globals.bind(&qh, 1..=1, ()).map_err(|_| bind("shm"))?;
    let layers: zwlr_layer_shell_v1::ZwlrLayerShellV1 =
        (globals.bind(&qh, 1..=4, ())).map_err(|_| bind("layer shell"))?;
    let seat: wl_seat::WlSeat = globals.bind(&qh, 1..=5, ()).map_err(|_| bind("seat"))?;
    let manager: wl_data_device_manager::WlDataDeviceManager =
        (globals.bind(&qh, 3..=3, ())).map_err(|_| bind("data device manager"))?;
    let heard = Arc::new((Mutex::new(Heard::default()), Condvar::new()));
    let (asks, asked) = mpsc::channel();
    let mut state = State {
        connection: connection.clone(),
        qh: qh.clone(),
        compositor,
        shm,
        layers,
        device: manager.get_data_device(&seat, &qh, ()),
        manager,
        pointer: None,
        drag: None,
        asked,
        heard: heard.clone(),
    };
    let mut queue = queue;
    std::thread::Builder::new()
        .name("drive drags".into())
        .spawn(move || {
            while queue.blocking_dispatch(&mut state).is_ok() {
                state.answer_asks();
                if state.connection.flush().is_err() {
                    break;
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(Client {
        connection,
        queue: qh,
        asks: Mutex::new(asks),
        heard,
    })
}

/// The client, on its thread.
struct State {
    connection: Connection,
    qh: QueueHandle<State>,
    compositor: wl_compositor::WlCompositor,
    shm: wl_shm::WlShm,
    layers: zwlr_layer_shell_v1::ZwlrLayerShellV1,
    manager: wl_data_device_manager::WlDataDeviceManager,
    device: wl_data_device::WlDataDevice,
    pointer: Option<wl_pointer::WlPointer>,
    drag: Option<Drag>,
    asked: mpsc::Receiver<Ask>,
    heard: Arc<(Mutex<Heard>, Condvar)>,
}

/// A drag's surface, the layer it's on, its clear picture, and the source with what it carries.
struct Drag {
    surface: wl_surface::WlSurface,
    layer: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    pool: wl_shm_pool::WlShmPool,
    buffer: wl_buffer::WlBuffer,
    source: wl_data_source::WlDataSource,
    data: Vec<(String, Vec<u8>)>,
}

/// What a sync's callback says is done once the compositor has handled what came before it.
#[derive(Clone, Copy)]
enum Synced {
    Woken,
    Shown,
    Started,
}

impl State {
    /// Has the driver hear `change`, and wakes it.
    fn hear(&self, change: impl FnOnce(&mut Heard)) {
        let (heard, changed) = &*self.heard;
        let mut heard = heard
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        change(&mut heard);
        changed.notify_all();
    }

    fn answer_asks(&mut self) {
        while let Ok(ask) = self.asked.try_recv() {
            match ask {
                Ask::Start(number, data) => {
                    self.end();
                    self.hear(|heard| heard.drag = number);
                    self.start(data);
                }
                Ask::End => self.end(),
            }
        }
    }

    /// Shows the drag's surface on the overlay layer, in the corner, its picture clear, and makes
    /// the source of what it carries.
    fn start(&mut self, data: Vec<(String, Vec<u8>)>) {
        let qh = &self.qh;
        let Ok(file) = clear_picture() else {
            return;
        };
        let bytes = SIZE * SIZE * 4;
        let pool = self.shm.create_pool(file.as_fd(), bytes, qh, ());
        let buffer = pool.create_buffer(0, SIZE, SIZE, SIZE * 4, wl_shm::Format::Argb8888, qh, ());
        let surface = self.compositor.create_surface(qh, ());
        let layer = self.layers.get_layer_surface(
            &surface,
            None,
            zwlr_layer_shell_v1::Layer::Overlay,
            "ilo-drive-drag".into(),
            qh,
            (),
        );
        layer.set_size(SIZE as u32, SIZE as u32);
        layer.set_anchor(
            zwlr_layer_surface_v1::Anchor::Bottom | zwlr_layer_surface_v1::Anchor::Right,
        );
        layer.set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);
        surface.commit();
        let source = self.manager.create_data_source(qh, ());
        for (kind, _) in &data {
            source.offer(kind.clone());
        }
        source.set_actions(wl_data_device_manager::DndAction::Copy);
        self.drag = Some(Drag {
            surface,
            layer,
            pool,
            buffer,
            source,
            data,
        });
    }

    fn end(&mut self) {
        if let Some(drag) = self.drag.take() {
            drag.source.destroy();
            drag.layer.destroy();
            drag.surface.destroy();
            drag.buffer.destroy();
            drag.pool.destroy();
        }
        self.hear(|heard| *heard = Heard::default());
    }
}

/// A file holding a clear square of SIZE, gone from the disk once open.
fn clear_picture() -> std::io::Result<std::fs::File> {
    let path = std::env::temp_dir().join(format!("ilo-drive-drag-{}", std::process::id()));
    let mut file = (std::fs::File::options())
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(&path)?;
    _ = std::fs::remove_file(&path);
    file.write_all(&vec![0; (SIZE * SIZE * 4) as usize])?;
    Ok(file)
}

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

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for State {
    fn event(
        state: &mut Self,
        layer: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        connection: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if !state.drag.as_ref().is_some_and(|drag| drag.layer == *layer) {
            return;
        }
        match event {
            zwlr_layer_surface_v1::Event::Configure { serial, .. } => {
                layer.ack_configure(serial);
                if let Some(drag) = &state.drag {
                    drag.surface.attach(Some(&drag.buffer), 0, 0);
                    drag.surface.damage(0, 0, SIZE, SIZE);
                    drag.surface.commit();
                }
                connection.display().sync(qh, Synced::Shown);
            }
            zwlr_layer_surface_v1::Event::Closed => state.end(),
            _ => {}
        }
    }
}

impl Dispatch<wl_callback::WlCallback, Synced> for State {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        event: wl_callback::Event,
        synced: &Synced,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if !matches!(event, wl_callback::Event::Done { .. }) || state.drag.is_none() {
            return;
        }
        match synced {
            Synced::Woken => {}
            Synced::Shown => state.hear(|heard| heard.shown = true),
            Synced::Started => state.hear(|heard| heard.started = true),
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
            && capabilities.contains(wl_seat::Capability::Pointer)
            && state.pointer.is_none()
        {
            state.pointer = Some(seat.get_pointer(qh, ()));
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        connection: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let Some(drag) = state.drag.as_ref() else {
            return;
        };
        match event {
            wl_pointer::Event::Enter { surface, .. } if surface == drag.surface => {
                state.hear(|heard| heard.entered = true);
            }
            wl_pointer::Event::Leave { surface, .. } if surface == drag.surface => {
                state.hear(|heard| heard.entered = false);
            }
            // The press on its surface, whose serial the compositor starts the drag with.
            wl_pointer::Event::Button {
                serial,
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } => {
                let entered = state.heard.0.lock().is_ok_and(|heard| heard.entered);
                if entered {
                    (state.device).start_drag(Some(&drag.source), &drag.surface, None, serial);
                    connection.display().sync(qh, Synced::Started);
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_data_source::WlDataSource, ()> for State {
    fn event(
        state: &mut Self,
        source: &wl_data_source::WlDataSource,
        event: wl_data_source::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(drag) = state.drag.as_ref().filter(|drag| drag.source == *source) else {
            return;
        };
        let now = Some(Instant::now());
        match event {
            wl_data_source::Event::Target { mime_type } => state.hear(|heard| {
                heard.accepted = mime_type;
                heard.targets += 1;
                heard.last = now;
            }),
            wl_data_source::Event::Action {
                dnd_action: WEnum::Value(action),
            } => state.hear(|heard| {
                heard.action = action.bits();
                heard.last = now;
            }),
            // The target reads what it carries, as its type.
            wl_data_source::Event::Send { mime_type, fd } => {
                if let Some((_, data)) = drag.data.iter().find(|(kind, _)| *kind == mime_type) {
                    _ = std::fs::File::from(fd).write_all(data);
                }
                state.hear(|heard| heard.last = now);
            }
            wl_data_source::Event::DndDropPerformed => state.hear(|heard| heard.last = now),
            // Its first end is how it ended: wlroots cancels a source the target has finished with
            // once the offer goes, which isn't a drag refused.
            wl_data_source::Event::DndFinished => state.hear(|heard| {
                heard.ended.get_or_insert(true);
            }),
            wl_data_source::Event::Cancelled => state.hear(|heard| {
                heard.ended.get_or_insert(false);
            }),
            _ => {}
        }
    }
}

impl Dispatch<wl_data_device::WlDataDevice, ()> for State {
    fn event(
        _: &mut Self,
        _: &wl_data_device::WlDataDevice,
        _: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }

    event_created_child!(State, wl_data_device::WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (wl_data_offer::WlDataOffer, ()),
    ]);
}

delegate_noop!(State: wl_compositor::WlCompositor);
delegate_noop!(State: wl_shm_pool::WlShmPool);
delegate_noop!(State: wl_data_device_manager::WlDataDeviceManager);
delegate_noop!(State: zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(State: ignore wl_surface::WlSurface);
delegate_noop!(State: ignore wl_shm::WlShm);
delegate_noop!(State: ignore wl_buffer::WlBuffer);
delegate_noop!(State: ignore wl_data_offer::WlDataOffer);
