//! Another app's drag under Wayland, for check/drops.sh: a small window of its own, which the drag
//! starts from, and a virtual pointer, which a headless sway seat has no other of.
//! Usage: drag <width>x<height> [<type> <data>...]
//! The size is the output's, in logical pixels, which the pointer moves on. The drag carries each
//! type with its data, text/x-moz-url's in UTF-16 as browsers' drags carry a link. Each line on stdin is a step: `move <x> <y>` puts the pointer there, `press`
//! presses its left button, which over the window starts the drag, and `release` lets it go. What
//! happens comes on stdout: `drag: ready` once the window shows, `drag: entered` as the pointer
//! comes into it, `drag: started`, `drag: target <type>` or `drag: target none` as the surface under
//! the drag takes it or not, `drag: sent <type>`, and how it ends: `drag: finished` or
//! `drag: cancelled`.
//! Given only the size it's a pointer alone, with no window and no drag, which tools/parity.sh
//! hovers with: `move` and `press` and `release` go where the pointer is.

use std::{
    io::{BufRead as _, Write as _},
    os::fd::AsFd as _,
    sync::mpsc,
    time::Instant,
};

use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum, delegate_noop, event_created_child,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{
        wl_buffer, wl_callback, wl_compositor, wl_data_device, wl_data_device_manager,
        wl_data_offer, wl_data_source, wl_pointer, wl_registry, wl_seat, wl_shm, wl_shm_pool,
        wl_surface,
    },
};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1, zwlr_virtual_pointer_v1,
};

const SIZE: i32 = 48;
const BUTTON_LEFT: u32 = 0x110;

struct State {
    surface: wl_surface::WlSurface,
    buffer: wl_buffer::WlBuffer,
    device: wl_data_device::WlDataDevice,
    source: wl_data_source::WlDataSource,
    data: Vec<(String, Vec<u8>)>,
    shown: bool,
    pointer: Option<wl_pointer::WlPointer>,
    over: bool,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let extent = args.first().and_then(|size| {
        let (width, height) = size.split_once('x')?;
        Some((width.parse::<u32>().ok()?, height.parse::<u32>().ok()?))
    });
    let (Some((width, height)), true) = (extent, args.len() % 2 == 1) else {
        eprintln!("usage: drag <width>x<height> [<type> <data>...]");
        std::process::exit(2);
    };
    let data: Vec<(String, Vec<u8>)> = (args[1..].chunks(2))
        .map(|pair| {
            let data = match pair[0].as_str() {
                "text/x-moz-url" => pair[1].encode_utf16().flat_map(u16::to_le_bytes).collect(),
                _ => pair[1].clone().into_bytes(),
            };
            (pair[0].clone(), data)
        })
        .collect();

    let connection = Connection::connect_to_env().expect("no Wayland display");
    let (globals, mut queue) = registry_queue_init::<State>(&connection).expect("no registry");
    let qh = queue.handle();
    let compositor: wl_compositor::WlCompositor = globals.bind(&qh, 1..=4, ()).unwrap();
    let shm: wl_shm::WlShm = globals.bind(&qh, 1..=1, ()).unwrap();
    let wm: xdg_wm_base::XdgWmBase = globals.bind(&qh, 1..=1, ()).unwrap();
    let seat: wl_seat::WlSeat = globals.bind(&qh, 1..=5, ()).unwrap();
    let manager: wl_data_device_manager::WlDataDeviceManager =
        globals.bind(&qh, 3..=3, ()).unwrap();
    let pointers: zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1 =
        (globals.bind(&qh, 1..=2, ())).expect("no virtual pointers");

    let surface = compositor.create_surface(&qh, ());
    if !data.is_empty() {
        let toplevel = wm.get_xdg_surface(&surface, &qh, ()).get_toplevel(&qh, ());
        toplevel.set_app_id("ilo-drag".into());
        toplevel.set_title("drag".into());
        surface.commit();
    }

    // One opaque square, in a file that's gone once mapped.
    let path = std::env::temp_dir().join(format!("ilo-drag-{}", std::process::id()));
    let mut file = (std::fs::File::options()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true))
    .open(&path)
    .unwrap();
    _ = std::fs::remove_file(&path);
    let pixels: Vec<u8> = (0..SIZE * SIZE)
        .flat_map(|_| 0xff3366ccu32.to_le_bytes())
        .collect();
    file.write_all(&pixels).unwrap();
    let pool = shm.create_pool(file.as_fd(), pixels.len() as i32, &qh, ());
    let buffer = pool.create_buffer(0, SIZE, SIZE, SIZE * 4, wl_shm::Format::Argb8888, &qh, ());

    let data_is_empty = data.is_empty();
    let source = manager.create_data_source(&qh, ());
    for (kind, _) in &data {
        source.offer(kind.clone());
    }
    source.set_actions(wl_data_device_manager::DndAction::Copy);
    let mut state = State {
        surface,
        buffer,
        device: manager.get_data_device(&seat, &qh, ()),
        source,
        data,
        // A pointer alone has no window to wait for.
        shown: data_is_empty,
        pointer: None,
        over: false,
    };
    let pointer = pointers.create_virtual_pointer(Some(&seat), &qh, ());

    // The steps, which wake the queue with a round trip.
    let (steps, stepped) = mpsc::channel();
    {
        let (connection, qh) = (connection.clone(), qh.clone());
        std::thread::spawn(move || {
            for line in std::io::stdin().lock().lines() {
                let Ok(line) = line else { break };
                if steps.send(line).is_err() {
                    break;
                }
                connection.display().sync(&qh, ());
                _ = connection.flush();
            }
            _ = steps.send("quit".into());
            connection.display().sync(&qh, ());
            _ = connection.flush();
        });
    }
    let start = Instant::now();
    let mut ready = false;
    loop {
        queue
            .blocking_dispatch(&mut state)
            .expect("the display went");
        if !ready && state.shown && state.pointer.is_some() {
            ready = true;
            println!("drag: ready");
        }
        while let Ok(step) = stepped.try_recv() {
            let time = start.elapsed().as_millis() as u32;
            let words: Vec<&str> = step.split_whitespace().collect();
            match words.as_slice() {
                ["move", x, y] => {
                    let (Ok(x), Ok(y)) = (x.parse(), y.parse()) else {
                        eprintln!("drag: no step {step}");
                        continue;
                    };
                    pointer.motion_absolute(time, x, y, width, height);
                }
                ["press"] => pointer.button(time, BUTTON_LEFT, wl_pointer::ButtonState::Pressed),
                ["release"] => pointer.button(time, BUTTON_LEFT, wl_pointer::ButtonState::Released),
                ["quit"] => return,
                _ => {
                    eprintln!("drag: no step {step}");
                    continue;
                }
            }
            pointer.frame();
        }
        _ = connection.flush();
    }
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

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for State {
    fn event(
        _: &mut Self,
        wm: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for State {
    fn event(
        state: &mut Self,
        surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            surface.ack_configure(serial);
            state.surface.attach(Some(&state.buffer), 0, 0);
            state.surface.damage(0, 0, SIZE, SIZE);
            state.surface.commit();
            state.shown = true;
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
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter { surface, .. } if surface == state.surface => {
                state.over = true;
                println!("drag: entered");
            }
            wl_pointer::Event::Leave { .. } => state.over = false,
            wl_pointer::Event::Button {
                serial,
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } if state.over => {
                (state.device).start_drag(Some(&state.source), &state.surface, None, serial);
                println!("drag: started");
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_data_source::WlDataSource, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_data_source::WlDataSource,
        event: wl_data_source::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_source::Event::Target { mime_type } => {
                println!("drag: target {}", mime_type.as_deref().unwrap_or("none"));
            }
            wl_data_source::Event::Send { mime_type, fd } => {
                if let Some((_, data)) = state.data.iter().find(|(kind, _)| *kind == mime_type) {
                    _ = std::fs::File::from(fd).write_all(data);
                }
                println!("drag: sent {mime_type}");
            }
            wl_data_source::Event::DndFinished => println!("drag: finished"),
            wl_data_source::Event::Cancelled => println!("drag: cancelled"),
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
delegate_noop!(State: zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1);
delegate_noop!(State: zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1);
delegate_noop!(State: ignore wl_surface::WlSurface);
delegate_noop!(State: ignore wl_shm::WlShm);
delegate_noop!(State: ignore wl_buffer::WlBuffer);
delegate_noop!(State: ignore wl_callback::WlCallback);
delegate_noop!(State: ignore wl_data_offer::WlDataOffer);
delegate_noop!(State: ignore xdg_toplevel::XdgToplevel);
