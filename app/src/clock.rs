//! The system's clock: whether it shows times on a 12- or a 24-hour clock, as the host reads it
//! (main.rs, follow_clock). History's row times follow it, as a native app's do (#2337).

use std::sync::atomic::{AtomicBool, Ordering};

/// Until the host has said, the 24-hour clock, which every locale reads.
static TWELVE_HOUR: AtomicBool = AtomicBool::new(false);

/// The system shows times on a 12-hour clock, or on a 24-hour one.
pub fn set_twelve_hour(twelve: bool) {
    TWELVE_HOUR.store(twelve, Ordering::Relaxed);
}

pub fn twelve_hour() -> bool {
    TWELVE_HOUR.load(Ordering::Relaxed)
}

/// The hour and minute, for chrono, on the clock asked for: 2:05 PM or 14:05.
pub fn hour_minute(twelve: bool) -> &'static str {
    match twelve {
        true => "%-I:%M %p",
        false => "%H:%M",
    }
}

/// Whether a strftime time format, as a Linux locale's, shows a 12-hour clock's hour: %I or %l,
/// or %r, which has %I.
pub fn twelve_hour_format(format: &str) -> bool {
    ["%I", "%l", "%r"].iter().any(|hour| format.contains(hour))
}

#[cfg(test)]
mod tests {
    use super::twelve_hour_format;

    #[test]
    fn a_locales_time_format_says_its_clock() {
        // en_US's, de_DE's and en_GB's, then one with the hour unpadded.
        assert!(twelve_hour_format("%r"));
        assert!(!twelve_hour_format("%T"));
        assert!(!twelve_hour_format("%H:%M:%S"));
        assert!(twelve_hour_format("%l:%M:%S %p"));
    }
}
