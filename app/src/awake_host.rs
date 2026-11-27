//! The awake hold's host (browser::awake): the platform layer's, host_keep_awake on the Mac and
//! Windows, and on Linux the desktop's or logind's over D-Bus (platform/linux/awake.rs).

use super::*;

pub(crate) struct Awake;

impl browser::awake::Host for Awake {
    fn hold(&self, on: bool) {
        #[cfg(not(target_os = "linux"))]
        {
            let reason = CString::new(browser::awake::REASON).unwrap_or_default();
            unsafe { platform::host_keep_awake(on, reason.as_ptr()) };
        }
        #[cfg(target_os = "linux")]
        update_app_later(move |cx| awake::hold(on, cx));
    }
}

/// Starts following the threads under way.
pub(crate) fn start(cx: &mut App) {
    browser::awake::open(Rc::new(Awake), cx);
}
