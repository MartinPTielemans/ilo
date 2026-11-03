// Links chrome.dll, the engine's framework, from the engine setup.sh puts in .deps/engine.
fn main() {
    let engine = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.deps/engine");
    println!("cargo:rustc-link-arg={}", engine.join("chrome.dll.lib").display());
    // The name in the window title and the app's menus, as the browser's crate sets it.
    let name = std::fs::read_to_string("../branding/name").expect("no branding/name");
    println!("cargo:rustc-env=APP_NAME={}", name.trim());
    println!("cargo:rerun-if-changed=../branding/name");
    // The sync server it signs in to unless the user names another, as the browser's crate has it.
    let sync_server = std::fs::read_to_string("../branding/sync-server");
    println!("cargo:rustc-env=SYNC_SERVER={}", sync_server.unwrap_or_default().trim());
    println!("cargo:rerun-if-changed=../branding");
    // Its engine is always a prebuilt one, whose header engine/fetch.sh checked (../build.rs).
    println!("cargo::rustc-check-cfg=cfg(engine_header_mismatch)");
}
