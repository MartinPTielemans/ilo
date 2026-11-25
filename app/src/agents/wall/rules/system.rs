//! What of the system a program inside the wall may read to run, in one place: literal roots,
//! none of them a user's.
//!
//! Adapted from OpenAI Codex (https://github.com/openai/codex),
//! codex-rs/sandboxing/src/seatbelt_read_only_platform_defaults.sbpl at rust-v0.172.0, commit
//! 3d86e43b075adeba2193ec5c22ac92f55afd82e6: its platform defaults for a restricted read.
//! Copyright 2025 OpenAI, licensed under the Apache License, Version 2.0; see NOTICE.
//!
//! Taken as it is: every read, map and special file below. Changed: its one `path-ancestors` is
//! written as the literal names it matches, and its firmlinks' parents lose the
//! `(vnode-type DIRECTORY)` beside each, which as a second filter allows every folder's metadata.
//! Left out: its metadata of all of `/var` and
//! `/private/var`, which hold every user's temporary folders; `/opt/homebrew/lib` and
//! `/usr/local/lib`, which aren't the system's; the app sandbox's extensions, which another
//! program hands out; the syslog socket and the notification memory; and its lookups, of which
//! rules.rs takes the trust ones. Added: the one name of `/private/var` a shell's start reads,
//! the system's Perl, and the developer tools' pointer and their own folder.

/// One allow: its operations, then each root in the filter named.
pub struct Allow {
    pub ops: &'static str,
    pub filter: &'static str,
    pub roots: &'static [&'static str],
}

const fn allow(ops: &'static str, filter: &'static str, roots: &'static [&'static str]) -> Allow {
    Allow { ops, filter, roots }
}

const READ: &str = "file-read* file-test-existence";
const METADATA: &str = "file-read-metadata file-test-existence";

/// The frameworks and libraries the loader maps and reads.
const FRAMEWORKS: &[&str] = &[
    "/Library/Apple/System/Library/Frameworks",
    "/Library/Apple/System/Library/PrivateFrameworks",
    "/Library/Apple/usr/lib",
    "/System/Library/Frameworks",
    "/System/Library/PrivateFrameworks",
    "/System/Library/SubFrameworks",
    "/System/iOSSupport/System/Library/Frameworks",
    "/System/iOSSupport/System/Library/PrivateFrameworks",
    "/System/iOSSupport/System/Library/SubFrameworks",
    "/usr/lib",
];

pub const SYSTEM: &[Allow] = &[
    allow(
        READ,
        "subpath",
        &[
            "/Library/Apple",
            "/Library/Filesystems/NetFSPlugins",
            "/Library/Preferences/Logging",
            "/private/var/db/DarwinDirectory/local/recordStore.data",
            "/private/var/db/timezone",
            "/usr/lib",
            "/usr/share",
            "/Library/Preferences",
            "/var/db",
            "/private/var/db",
        ],
    ),
    allow("file-map-executable", "subpath", FRAMEWORKS),
    allow(
        "file-map-executable",
        "subpath",
        &["/System/Library/Extensions"],
    ),
    allow(READ, "subpath", FRAMEWORKS),
    // The system's Perl: /usr/bin/perl loads its library, its modules and their compiled parts
    // from here, on the sealed system volume.
    allow(
        "file-read* file-test-existence file-map-executable",
        "subpath",
        &["/System/Library/Perl"],
    ),
    // The system's links to its own folders, and the firmlinks' parents.
    allow(
        METADATA,
        "literal",
        &[
            "/etc",
            "/tmp",
            "/var",
            "/private/etc/localtime",
            "/System",
            "/System/Volumes",
            "/System/Volumes/Data",
            "/System/Volumes/Data/private",
            "/System/Volumes/Data/Users",
        ],
    ),
    // Added: the link `/bin/sh` reads to pick the shell it starts.
    allow(METADATA, "literal", &["/private/var/select/sh"]),
    // The system's developer tools: /usr/bin/git, python3 and their like read where the tools are
    // from this link, then start them from there. Their own folder, where they're installed, is
    // the system's. Without the link they ask to have the tools installed, which stays refused.
    allow(METADATA, "literal", &["/private/var/select/developer_dir"]),
    allow(
        "file-read* file-test-existence file-map-executable",
        "subpath",
        &["/Library/Developer/CommandLineTools"],
    ),
    // A program's own working folder's root.
    allow(READ, "literal", &["/"]),
    allow(
        READ,
        "literal",
        &[
            "/dev/autofs_nowait",
            "/dev/random",
            "/dev/urandom",
            "/private/etc/master.passwd",
            "/private/etc/passwd",
            "/private/etc/protocols",
            "/private/etc/services",
        ],
    ),
    allow(
        "file-read* file-test-existence file-write-data",
        "literal",
        &["/dev/null", "/dev/zero"],
    ),
    allow(
        "file-read-data file-test-existence file-write-data",
        "subpath",
        &["/dev/fd"],
    ),
    allow(
        "file-read* file-test-existence file-write-data file-ioctl",
        "literal",
        &["/dev/dtracehelper"],
    ),
    allow("file-read*", "subpath", &["/etc", "/private/etc"]),
    allow(
        READ,
        "literal",
        &[
            "/System/Library/CoreServices",
            "/System/Library/CoreServices/.SystemVersionPlatform.plist",
            "/System/Library/CoreServices/SystemVersion.plist",
        ],
    ),
    allow(
        "file-read*",
        "literal",
        &["/private/var/db/eligibilityd/eligibility.plist"],
    ),
    // The system's own commands, so a shell finds and runs them.
    allow(
        "file-read-data file-read-metadata",
        "subpath",
        &["/bin", "/sbin", "/usr/bin", "/usr/sbin", "/usr/libexec"],
    ),
    // A terminal's basics.
    allow("file-read*", "regex", &["^/dev/fd/(0|1|2)$"]),
    allow("file-write*", "regex", &["^/dev/fd/(1|2)$"]),
    allow(
        "file-read* file-write*",
        "literal",
        &["/dev/null", "/dev/tty"],
    ),
    allow(
        "file-read-metadata",
        "literal",
        &["/dev", "/dev/stdin", "/dev/stdout", "/dev/stderr"],
    ),
    allow(
        "file-read-metadata",
        "regex",
        &["^/dev/.*$", "^/dev/tty[^/]*$", "^/dev/pty[^/]*$"],
    ),
    allow("file-read* file-write*", "regex", &["^/dev/ttys[0-9]+$"]),
    allow("file-read* file-write*", "literal", &["/dev/ptmx"]),
    allow("file-ioctl", "regex", &["^/dev/ttys[0-9]+$"]),
];

/// What a program's start asks of the system that isn't a path, as upstream has it.
const CALLS: &str = "\
(allow system-mac-syscall (mac-policy-name \"vnguard\"))
(allow system-mac-syscall (require-all (mac-policy-name \"Sandbox\") (mac-syscall-number 67)))
(allow system-fsctl (fsctl-command FSIOC_CAS_BSDFLAGS))
";

/// The system's part of the text, one line an allow.
pub fn text() -> String {
    let mut text = String::from(CALLS);
    for allow in SYSTEM {
        let filters: Vec<String> = (allow.roots.iter())
            .map(|root| format!("({} \"{root}\")", allow.filter))
            .collect();
        text.push_str(&format!("(allow {} {})\n", allow.ops, filters.join(" ")));
    }
    text
}
