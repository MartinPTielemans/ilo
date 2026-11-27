//! The daily usage count's host (browser::daily_count, #1977): the user's yes and the last count's
//! date in the app's data, the launch's count about 30 s in, and one when the app becomes active on a
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
    /// The launch's count has had its time, so becoming active may send one.
    ready: bool,
    /// The day this launch last tried, so a failed count waits for the next launch.
    tried: Option<daily_count::NaiveDate>,
    /// The day of the count on its way.
    sending: Option<daily_count::NaiveDate>,
}

thread_local! {
    static COUNT: RefCell<Count> = RefCell::default();
}

/// As Chromium starts: where the count goes, from the arguments the process was `given` or the
/// build's branding. Returns the switch that tells the core, if the build doesn't have it from its
/// arguments already.
pub(crate) fn prepare(given: &[String]) -> Option<String> {
    // Only the prod build names ilo's server (branding/daily-count), as only it reaches users. A
    // fork names its own or none.
    let named = cfg!(feature = "prod").then(|| include_str!("../branding/daily-count"));
    let endpoint = daily_count::endpoint(given, named);
    let given_switch = given.iter().any(|arg| arg.starts_with(daily_count::SWITCH));
    let switch = (endpoint.as_ref())
        .filter(|_| !given_switch)
        .map(|url| format!("{}={url}", daily_count::SWITCH));
    COUNT.with_borrow_mut(|count| count.endpoint = endpoint);
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
    let today = daily_count::today();
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
    let Some(flags) = daily_count::due(last, today) else {
        // check.sh waits for this line.
        log!("browser: daily count sent today already");
        return;
    };
    COUNT.with_borrow_mut(|count| {
        count.tried = Some(today);
        count.sending = Some(today);
    });
    let nightly = browser::updates::built(cx) == browser::updates::Track::Nightly;
    let channel = CString::new(browser::build_info::channel(nightly)).unwrap_or_default();
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

fn dir() -> std::path::PathBuf {
    std::path::PathBuf::from(profile_dir())
}

/// Whether the build sends the count at all, for Settings to offer the switch.
pub(crate) fn named() -> bool {
    COUNT.with_borrow(|count| count.endpoint.is_some())
}

/// Settings › Privacy › Send a Daily Usage Count: on only once the user has said yes.
pub(crate) fn on() -> bool {
    daily_count::on(&dir())
}

/// Turns the count on or off, now and for the launches after, from Settings or onboarding. Off
/// stops it at once: nothing more is sent, though one already on its way finishes. On sends the
/// day's count once the launch's has had its time.
pub(crate) fn set_on(on: bool, cx: &mut App) {
    _ = daily_count::answer(&dir(), on);
    // check.sh reads this line.
    log!(
        "browser: daily count switch {}",
        if on { "on" } else { "off" }
    );
    if on {
        became_active(cx);
    }
}
