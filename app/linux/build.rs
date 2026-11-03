// The name in the window title and the app's menus, as the browser's crate sets it. The core's C
// API comes from chrome, which loads this library, so nothing here links the engine.
fn main() {
    let name = std::fs::read_to_string("../branding/name").expect("no branding/name");
    println!("cargo:rustc-env=APP_NAME={}", name.trim());
    println!("cargo:rerun-if-changed=../branding/name");
    // The build's own sync server, as the browser's crate reads it (its build.rs).
    let sync_server = std::fs::read_to_string("../branding/sync-server");
    println!("cargo:rustc-env=SYNC_SERVER={}", sync_server.unwrap_or_default().trim());
    println!("cargo:rerun-if-changed=../branding");

    // libwayland too comes from chrome, which exports the copy its own wl_display is from: two
    // copies can't share one display. GPUI's wayland-sys links -lwayland-client, which this empty
    // linker script answers ahead of the system's library, so the calls stay unresolved until chrome
    // loads this library and none names libwayland-client.so.0 as needed.
    let out = std::env::var("OUT_DIR").unwrap();
    std::fs::write(format!("{out}/libwayland-client.so"), "/* chrome's, at load */\n").unwrap();
    println!("cargo:rustc-link-search=native={out}");
}
