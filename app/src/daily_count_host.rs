//! The daily usage count's host (browser::daily_count, #1977): the switch and the last count's date
//! in the app's data, the launch's count about 30 s in, and one when the app becomes active on a
//! new UTC day. No timer runs after that first one, so an idle browser never wakes for it. The core
//! posts it, through Chromium's network stack, to the endpoint the build names.

use super::*;
use browser::daily_count;

/// How long after launch the count goes, out of the way of the launch itself.
const AFTER_LAUNCH: Duration = Duration::from_secs(30);

#[derive(Default)]
struct Count {
    /// Where it goes, if the build or a check names it.
    endpoint: Option<String>,
    /// The launch found no data of an earlier one.
    first_run: bool,
    /// The launch's count has had its time, so becoming active may send one.
    ready: bool,
    /// The day this launch last tried, so a failed count waits for the next launch.
    tried: Option<chrono::NaiveDate>,
    /// The day of the count on its way.
    sending: Option<chrono::NaiveDate>,
}

thread_local! {
    static COUNT: RefCell<Count> = RefCell::default();
}

/// As Chromium starts, before it writes its Local State: where the count goes, from the arguments
/// the process was `given` or the build's branding, and whether this is the copy's first run.
/// Returns the switch that tells the core, if the build doesn't have it from its arguments already.
pub(crate) fn prepare(given: &[String], profile: &str) -> Option<String> {
    // Only the prod build names ilo's server (branding/daily-count), as only it reaches users. A
    // fork names its own or none.
    let named = cfg!(feature = "prod").then(|| include_str!("../branding/daily-count"));
    let endpoint = daily_count::endpoint(given, named);
    let first_run = !std::path::Path::new(profile).join("Local State").exists();
    let given_switch = given.iter().any(|arg| arg.starts_with(daily_count::SWITCH));
    let switch = (endpoint.as_ref())
        .filter(|_| !given_switch)
        .map(|url| format!("{}={url}", daily_count::SWITCH));
    COUNT.with_borrow_mut(|count| {
        count.endpoint = endpoint;
        count.first_run = first_run;
    });
    switch
}

/// Once the app has started: the launch's count, after a while.
pub(crate) fn start(cx: &mut App) {
    if COUNT.with_borrow(|count| count.endpoint.is_none()) {
        // check.sh waits for this line.
        log!("browser: daily count none: no server");
        return;
    }
    // A check waits less (check/count.sh). The prod build always waits the whole time.
    let after = std::env::var("BROWSER_DAILY_COUNT_AFTER")
        .ok()
        .filter(|_| !cfg!(feature = "prod"))
        .and_then(|seconds| seconds.parse().ok())
        .map_or(AFTER_LAUNCH, Duration::from_secs);
    cx.spawn(async move |cx| {
        cx.background_executor().timer(after).await;
        cx.update(|cx| {
            COUNT.with_borrow_mut(|count| count.ready = true);
            send_if_due(cx);
        });
    })
    .detach();
}

/// Back from another app: a new UTC day's count, once the launch's has had its time.
pub(crate) fn became_active(cx: &mut App) {
    if COUNT.with_borrow(|count| count.ready) {
        send_if_due(cx);
    }
}

fn send_if_due(cx: &mut App) {
    let today = chrono::Utc::now().date_naive();
    let skip = COUNT.with_borrow(|count| {
        count.endpoint.is_none() || count.sending.is_some() || count.tried == Some(today)
    });
    if skip {
        return;
    }
    if !on() {
        // check.sh waits for this line.
        log!("browser: daily count not sent: off");
        return;
    }
    let last = std::fs::read_to_string(path(daily_count::LAST))
        .ok()
        .and_then(|text| daily_count::parse(&text));
    let first_run = COUNT.with_borrow(|count| count.first_run);
    let Some(flags) = daily_count::due(last, today, first_run) else {
        // check.sh waits for this line.
        log!("browser: daily count sent today already");
        return;
    };
    COUNT.with_borrow_mut(|count| {
        count.tried = Some(today);
        count.sending = Some(today);
    });
    let channel = match browser::updates::built(cx) {
        browser::updates::Track::Release => c"release",
        browser::updates::Track::Nightly => c"nightly",
    };
    let version = CString::new(browser::build_info::VERSION).unwrap_or_default();
    // check.sh reads this line.
    log!(
        "browser: daily count sending first {} weekly {} monthly {}",
        flags.first,
        flags.weekly,
        flags.monthly
    );
    unsafe {
        bc::bc_send_daily_count(
            version.as_ptr(),
            channel.as_ptr(),
            flags.first,
            flags.weekly,
            flags.monthly,
            Some(sent),
            std::ptr::null_mut(),
        )
    };
}

/// The core's answer: a count the endpoint took is today's, kept for the next launches.
unsafe extern "C" fn sent(_: *mut c_void, ok: bool) {
    let Some(day) = COUNT.with_borrow_mut(|count| count.sending.take()) else {
        return;
    };
    if ok {
        _ = std::fs::write(path(daily_count::LAST), daily_count::date(day));
    }
}

fn path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(&profile_dir()).join(name)
}

/// Whether the build sends the count at all, for Settings to offer the switch.
pub(crate) fn named() -> bool {
    COUNT.with_borrow(|count| count.endpoint.is_some())
}

/// Settings › Privacy › Send a Daily Usage Count: on without its file.
pub(crate) fn on() -> bool {
    !path(daily_count::OFF).exists()
}

/// Turns the count on or off, now and for the launches after. Off stops it at once: nothing more
/// is sent, though one already on its way finishes.
pub(crate) fn set_on(on: bool, _: &mut App) {
    _ = match on {
        true => std::fs::remove_file(path(daily_count::OFF)),
        false => std::fs::write(path(daily_count::OFF), ""),
    };
    // check.sh reads this line.
    log!(
        "browser: daily count switch {}",
        if on { "on" } else { "off" }
    );
}
