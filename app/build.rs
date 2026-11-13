use std::path::{Path, PathBuf};

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let header = root.join("../core/public/browser_core.h");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    // The web's preview has neither the core nor a host in C++: the page is its host.
    let web = std::env::var("CARGO_CFG_TARGET_FAMILY").is_ok_and(|family| family == "wasm");

    println!("cargo::rustc-check-cfg=cfg(engine_header_mismatch)");
    if !web {
        native(&root, &header, &os);
    }

    // The core's API, read from its header, so the two can't drift: on the web too, where the
    // site's demo reads its constants and calls none of it.
    bindgen::Builder::default()
        .header(header.to_string_lossy())
        .allowlist_function("bc_.*")
        .allowlist_type("bc_.*")
        .allowlist_var("BC_.*")
        .derive_default(true)
        .rust_edition(bindgen::RustEdition::Edition2024)
        .generate()
        .expect("couldn't read the core's header")
        .write_to_file(PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("browser_core.rs"))
        .unwrap();

    // The name the user reads, from branding/, which build.sh also reads for the app's bundle.
    let name = root.join("branding/name");
    let name_text = std::fs::read_to_string(&name).expect("no branding/name");
    println!("cargo:rustc-env=APP_NAME={}", name_text.trim());
    println!("cargo:rerun-if-changed={}", name.display());
    // Default service URLs a fork changes (principle 2): the site, for help and release notes, and
    // where a new issue is filed (src/build_info.rs).
    for (file, variable) in [
        ("branding/site", "APP_SITE"),
        ("branding/issues", "APP_ISSUES"),
    ] {
        let path = root.join(file);
        let url = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("no {file}"));
        println!("cargo:rustc-env={variable}={}", url.trim());
        println!("cargo:rerun-if-changed={}", path.display());
    }
    // The sync server the build signs in to unless the user names another (main.rs): ilo's, in
    // branding/sync-server, which a fork changes or deletes. With no file the build has none. The
    // folder is watched, as a file that isn't there would have this run on every build.
    let sync_server = std::fs::read_to_string(root.join("branding/sync-server"));
    println!(
        "cargo:rustc-env=SYNC_SERVER={}",
        sync_server.unwrap_or_default().trim()
    );
    println!("cargo:rerun-if-changed={}", root.join("branding").display());
    println!("cargo:rerun-if-changed=platform");
    println!("cargo:rerun-if-changed={}", header.display());

    // The look's values, from the file the code and the design file share (tools/design-sync.md).
    let tokens = root.join("design/tokens.toml");
    let tokens_text = std::fs::read_to_string(&tokens).expect("no design/tokens.toml");
    let tokens_rs_path = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("tokens.rs");
    std::fs::write(tokens_rs_path, tokens_rs(&tokens_text)).unwrap();
    println!("cargo:rerun-if-changed={}", tokens.display());
}

/// The C++ host for the OS and the engine the browser links.
fn native(root: &Path, header: &Path, os: &str) {
    let mut host = cc::Build::new();
    host.cpp(true)
        // The driver calls the core for what only agent code will, once there is some.
        .include(root.join("../core/public"))
        .define("NDEBUG", None);
    match os {
        "macos" => {
            host.file("platform/mac/host.mm")
                .file("platform/mac/drive.mm")
                .file("platform/mac/menu_window.mm")
                .file("platform/mac/emoji.mm")
                .file("platform/mac/dock.mm")
                .file("platform/mac/needs_you.mm")
                .file("platform/mac/updates.mm")
                .file("platform/mac/scripting.mm")
                .flag("-std=c++20")
                .flag("-fobjc-arc")
                // Sparkle's headers alone: updates.mm loads the framework at run time, from a
                // release build that carries it (setup.sh fetches it).
                .flag(format!("-F{}", root.join(".deps/Sparkle").display()));
        }
        "windows" => {
            host.file("platform/windows/host.cc")
                .file("platform/windows/drive.cc")
                .file("platform/windows/updates.cc")
                .file("platform/windows/ed25519.cc")
                .flag("/std:c++20")
                .flag("/utf-8")
                // C++/WinRT (the share sheet) throws, and its projection headers overflow the
                // object file's default section count.
                .flag("/EHsc")
                .flag("/bigobj")
                .define("UNICODE", None)
                .define("_UNICODE", None);
            // A release build updates itself (platform/windows/updates.cc), given the feed and the
            // key its updates are signed with, as build.sh gives the Mac's: what Info.plist holds
            // there is defined here. They're branding/update-feed's and branding/update-key's
            // unless UPDATE_FEED or UPDATE_PUBLIC_KEY is set, as the updates run sets them for its
            // own feed, and a fork that empties the key's file builds a copy with no updater. The
            // nightly feed beside it is branding/update-feed-nightly's, or UPDATE_FEED_NIGHTLY's,
            // where there is one. A check build has an updater only for a feed on the check's own
            // server (check/nightly.sh), as build.sh updates gives the Mac's copy.
            for name in [
                "UPDATE_PUBLIC_KEY",
                "UPDATE_FEED",
                "UPDATE_FEED_NIGHTLY",
                "APP_TRACK",
            ] {
                println!("cargo:rerun-if-env-changed={name}");
            }
            let key_file = root.join("branding/update-key");
            println!("cargo:rerun-if-changed={}", key_file.display());
            // The folder, so a nightly feed's file that comes or goes is seen too.
            println!("cargo:rerun-if-changed={}", root.join("branding").display());
            let key = match std::env::var("UPDATE_PUBLIC_KEY") {
                Ok(key) if !key.is_empty() => key,
                _ => std::fs::read_to_string(&key_file)
                    .map(|key| key.split_whitespace().collect())
                    .unwrap_or_default(),
            };
            let env = |name| {
                std::env::var(name)
                    .ok()
                    .filter(|value: &String| !value.is_empty())
            };
            let prod = std::env::var_os("CARGO_FEATURE_PROD").is_some();
            let local = |feed: String| feed.starts_with("http://127.0.0.1:");
            let check = !prod && env("UPDATE_FEED").is_some_and(local);
            if (prod || check) && !key.is_empty() {
                let feed = env("UPDATE_FEED").unwrap_or_else(|| {
                    std::fs::read_to_string(root.join("branding/update-feed"))
                        .expect("no branding/update-feed")
                });
                let nightly = match check {
                    true => env("UPDATE_FEED_NIGHTLY").unwrap_or_default(),
                    false => env("UPDATE_FEED_NIGHTLY").unwrap_or_else(|| {
                        std::fs::read_to_string(root.join("branding/update-feed-nightly"))
                            .unwrap_or_default()
                    }),
                };
                let quoted = |value: &str| format!("\"{value}\"");
                host.define("UPDATE_FEED", quoted(feed.trim()).as_str())
                    .define("UPDATE_FEED_NIGHTLY", quoted(nightly.trim()).as_str())
                    .define("UPDATE_PUBLIC_KEY", quoted(&key).as_str())
                    .define("APP_VERSION", quoted(env!("CARGO_PKG_VERSION")).as_str())
                    .define("APP_BUILD", quoted(&build_number(&root)).as_str());
                if check {
                    host.define("UPDATE_CHECK_BUILD", None);
                }
            }
            if env("APP_TRACK").as_deref() == Some("nightly") {
                host.define("APP_NIGHTLY", None);
            }
        }
        // Only what the preview asks of the host so far; the browser doesn't build here yet.
        "linux" => {
            // The desktop entry's name, for the default browser (branding/id).
            let id = std::fs::read_to_string(root.join("branding/id")).expect("no branding/id");
            println!(
                "cargo:rerun-if-changed={}",
                root.join("branding/id").display()
            );
            host.file("platform/linux/host.cc")
                .flag("-std=c++20")
                .define("APP_ID", format!("\"{}\"", id.trim()).as_str());
        }
        other => panic!("no host for {other}"),
    }
    host.compile("host");

    // Only the browser links the engine; the preview runs without it.
    if os == "windows" {
        // Chromium's framework on Windows is chrome.dll, which the browser loads from beside it.
        let engine = root.join(".deps/engine");
        println!(
            "cargo:rustc-link-arg-bin=Browser={}",
            engine.join("chrome.dll.lib").display()
        );
        // windowscodecs is WIC, for host_file_preview's thumbnails; uuid carries the COM GUIDs it
        // and host_drag_file name; windowsapp is WinRT, for the share sheet; advapi32 keeps when
        // updates.cc last checked, which downloads with winhttp and hashes with bcrypt.
        for lib in [
            "user32",
            "gdi32",
            "dwmapi",
            "shell32",
            "ole32",
            "comctl32",
            "comdlg32",
            "windowscodecs",
            "uuid",
            "windowsapp",
            "advapi32",
            "winhttp",
            "bcrypt",
        ] {
            println!("cargo:rustc-link-lib={lib}");
        }
    } else if os == "macos" {
        let frameworks = root.join(".deps/Chromium.app/Contents/Frameworks");
        for arg in [
            format!("-F{}", frameworks.display()),
            "-framework".into(),
            "Chromium Framework".into(),
            // The dev engine's framework loads from the run path, as Chromium's own executable finds it.
            "-Wl,-rpath,@executable_path/../Frameworks".into(),
        ] {
            println!("cargo:rustc-link-arg-bin=Browser={arg}");
        }
        println!("cargo:rustc-link-lib=framework=Cocoa");
        println!("cargo:rustc-link-lib=framework=UserNotifications");
        // The Apple Event Manager's C calls, for the quit Apple Event's sender (host.mm).
        println!("cargo:rustc-link-lib=framework=CoreServices");
        println!("cargo:rustc-link-lib=framework=Security");
        println!("cargo:rustc-link-lib=framework=CoreText");
        println!("cargo:rustc-link-lib=framework=ImageIO");
        println!("cargo:rustc-link-lib=framework=UniformTypeIdentifiers");
        println!("cargo:rustc-link-lib=framework=AVFoundation");
        println!("cargo:rustc-link-lib=framework=CoreMedia");
        println!("cargo:rustc-link-lib=framework=VideoToolbox");
        translator_swift(root);
    }

    // A local engine built from another header would run with shifted calls and layouts. Only the
    // browser, which links it, stops on that (main.rs); the tests and the preview don't link it.
    if let Some(why) = engine_mismatch(&root.join(".deps/Chromium.app"), header) {
        println!("cargo:rustc-cfg=engine_header_mismatch");
        println!("cargo:rustc-env=ENGINE_HEADER_MISMATCH={why}");
    }
}

/// The Mac's translator, Apple's Translation, which is Swift's alone (platform/mac/translate.swift):
/// a static library for the browser, whose objects name the Swift runtime and the framework they
/// need, which the linker finds in the SDK and the toolchain, the runtime at /usr/lib/swift.
fn translator_swift(root: &Path) {
    let source = root.join("platform/mac/translate.swift");
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let library = out.join("libilo_translate.a");
    let xcrun = |args: &[&str]| -> String {
        let run = std::process::Command::new("xcrun").args(args).output();
        let run = run.expect("no xcrun, for the translator's Swift");
        assert!(run.status.success(), "xcrun {args:?} failed");
        String::from_utf8_lossy(&run.stdout).trim().to_owned()
    };
    let sdk = xcrun(&["--sdk", "macosx", "--show-sdk-path"]);
    let swiftc = PathBuf::from(xcrun(&["--find", "swiftc"]));
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str() {
        "aarch64" => "arm64",
        other => other,
    }
    .to_owned();
    // The app's own minimum, which build.sh gives its Info.plist.
    let minimum = std::env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "15.0".into());
    let status = std::process::Command::new(&swiftc)
        .args([
            "-parse-as-library",
            "-O",
            "-swift-version",
            "5",
            "-static",
            "-emit-library",
        ])
        .args(["-module-name", "IloTranslate", "-sdk", &sdk])
        .arg("-target")
        .arg(format!("{arch}-apple-macos{minimum}"))
        .arg("-o")
        .arg(&library)
        .arg(&source)
        .status()
        .expect("couldn't run swiftc");
    assert!(
        status.success(),
        "swiftc couldn't build {}",
        source.display()
    );
    let toolchain = swiftc.parent().and_then(Path::parent).unwrap();
    for arg in [
        library.display().to_string(),
        format!("-L{sdk}/usr/lib/swift"),
        format!("-L{}", toolchain.join("lib/swift/macosx").display()),
        "-Wl,-rpath,/usr/lib/swift".into(),
    ] {
        println!("cargo:rustc-link-arg-bin=Browser={arg}");
    }
}

/// Why the Mac's engine, which setup.sh links at `link`, can't run with bindings from
/// `header`, if it can't: engine/build.sh keeps the header it built from beside it. A prebuilt
/// engine's header was checked by engine/fetch.sh, and Linux and Windows link only those. A header
/// that differs only in its comments is the same to the compiler, as tools/core-same.sh, which
/// fetch.sh asks too, says (#2195).
fn engine_mismatch(link: &Path, header: &Path) -> Option<String> {
    let engine = std::fs::canonicalize(link).ok()?;
    // A relinked or rebuilt engine checks again, and one without a record every build until it has.
    println!("cargo:rerun-if-changed={}", link.display());
    if engine
        .components()
        .any(|part| part.as_os_str() == "prebuilt")
    {
        return None;
    }
    let built = engine.parent()?.join("browser_core.h.built");
    println!("cargo:rerun-if-changed={}", built.display());
    let engine = engine.display();
    let header_text = std::fs::read(header).ok()?;
    match std::fs::read(&built) {
        Ok(built) if built == header_text => None,
        Ok(built) if same_to_compiler(header, &built, &header_text) => {
            println!(
                "cargo:warning=the engine at {engine} was built from a core/public/browser_core.h \
                 that differs from this checkout's only in comments and spacing, so it fits"
            );
            None
        }
        Ok(_) => Some(format!(
            "the engine at {engine} was built from another core/public/browser_core.h than this \
             checkout's, so the app would call it with shifted functions and layouts. Build it \
             from this checkout with engine/build.sh [dev], or link another with ./setup.sh."
        )),
        Err(_) => Some(format!(
            "the engine at {engine} doesn't say which core/public/browser_core.h it was built \
             from. Run engine/build.sh [dev] to record it."
        )),
    }
}

/// Whether the headers `a` and `b` are the same to the compiler, by tools/core-same.sh in the
/// checkout of `header`, core/public/browser_core.h. One it can't run is taken for a difference.
fn same_to_compiler(header: &Path, a: &[u8], b: &[u8]) -> bool {
    use std::io::Write;
    let Some(checkout) = header.ancestors().nth(3) else {
        return false;
    };
    let script = checkout.join("tools/core-same.sh");
    println!("cargo:rerun-if-changed={}", script.display());
    let strip = |text: &[u8]| -> Option<Vec<u8>> {
        let mut child = std::process::Command::new("bash")
            .arg(&script)
            .arg("--strip")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .ok()?;
        child.stdin.take()?.write_all(text).ok()?;
        let out = child.wait_with_output().ok()?;
        out.status.success().then_some(out.stdout)
    };
    matches!((strip(a), strip(b)), (Some(a), Some(b)) if a == b)
}

/// The build number updates are ordered by: the count of commits, as build.sh stamps the Mac's,
/// which only grows on main. A shallow clone's would be wrong, so a release build stops there.
fn build_number(root: &std::path::Path) -> String {
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output();
        let out = out.expect("a release build needs git");
        assert!(out.status.success(), "git {args:?} failed");
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    assert!(
        git(&["rev-parse", "--is-shallow-repository"]) == "false",
        "a release build needs the whole history: git fetch --unshallow"
    );
    // A new commit is a new build number.
    for path in [
        git(&["rev-parse", "--git-path", "HEAD"]),
        git(&["rev-parse", "--git-path", "refs"]),
    ] {
        println!("cargo:rerun-if-changed={}", root.join(path).display());
    }
    git(&["rev-list", "--count", "HEAD"])
}

/// Turns tokens.toml into Rust: a module for each table, a `pub const` for each key, and the
/// comments right above a key as its docs. Integers become `u32` (colours are hex), floats `f32`,
/// and a one-line array of either an array. It reads only that much TOML and stops the build on
/// anything else, so the file can't hold a value the code wouldn't get.
fn tokens_rs(toml: &str) -> String {
    let mut out =
        String::from("// Generated by build.rs from design/tokens.toml; edit that file.\n");
    let (mut path, mut closed, mut docs) =
        (Vec::<&str>::new(), Vec::<String>::new(), String::new());
    for (n, line) in toml.lines().enumerate() {
        let line = line.trim();
        let ident = |name: &str| {
            if !name.starts_with(|c: char| c.is_ascii_lowercase())
                || !name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                fail(n, line, "names are kebab-case");
            }
            name.replace('-', "_")
        };
        let number = |value: &str| match value.strip_prefix("0x") {
            Some(hex) if hex.chars().all(|c| c.is_ascii_hexdigit() || c == '_') => "u32",
            Some(_) => fail(n, line, "not a hex number"),
            None if value.chars().all(|c| c.is_ascii_digit() || c == '_') => "u32",
            None if value.replace('_', "").parse::<f64>().is_ok() => "f32",
            None => fail(n, line, "not a number"),
        };
        if line.is_empty() {
            docs.clear();
        } else if let Some(comment) = line.strip_prefix('#') {
            docs += &format!("///{comment}\n");
        } else if let Some(table) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            let table: Vec<&str> = table.split('.').collect();
            let shared = path.iter().zip(&table).take_while(|(a, b)| a == b).count();
            for depth in (shared..path.len()).rev() {
                closed.push(path[..=depth].join("."));
                out += "}\n";
            }
            if shared == table.len() || closed.contains(&table.join(".")) {
                fail(
                    n,
                    line,
                    "a table comes once, with the tables inside it right after it",
                );
            }
            for name in &table[shared..] {
                out += &format!("{docs}pub mod {} {{\n", ident(name));
            }
            (path, docs) = (table, String::new());
        } else if let Some((key, value)) = line.split_once('=') {
            if path.is_empty() {
                fail(n, line, "a key outside a table");
            }
            let value = value
                .split_once('#')
                .map_or(value, |(value, _)| value)
                .trim();
            let (ty, value) = match value
                .strip_prefix('[')
                .and_then(|value| value.strip_suffix(']'))
            {
                Some(items) => {
                    let items: Vec<&str> = items
                        .split(',')
                        .map(str::trim)
                        .filter(|i| !i.is_empty())
                        .collect();
                    let Some(ty) = items.first().map(|i| number(i)) else {
                        fail(n, line, "an empty array")
                    };
                    if items.iter().any(|i| number(i) != ty) {
                        fail(n, line, "an array of one type");
                    }
                    (
                        format!("[{ty}; {}]", items.len()),
                        format!("[{}]", items.join(", ")),
                    )
                }
                None => (number(value).to_string(), value.to_string()),
            };
            let name = ident(key.trim()).to_uppercase();
            out += &format!("{docs}pub const {name}: {ty} = {value};\n");
            docs.clear();
        } else {
            fail(n, line, "not a table, a key or a comment");
        }
    }
    out + &"}\n".repeat(path.len())
}

fn fail(n: usize, line: &str, why: &str) -> ! {
    panic!("design/tokens.toml:{}: {why}: {line}", n + 1)
}
