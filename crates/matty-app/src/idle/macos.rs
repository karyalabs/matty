//! The idle time from Quartz: seconds since any input in the login session.
#![allow(unsafe_code)]

use std::time::Duration;

/// `kCGEventSourceStateCombinedSessionState`.
const COMBINED_SESSION_STATE: i32 = 0;
/// `kCGAnyInputEventType`.
const ANY_INPUT: u32 = u32::MAX;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
}

pub(super) fn idle() -> Option<Duration> {
    // SAFETY: a query that takes two plain values and touches no memory of
    // ours.
    let seconds =
        unsafe { CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT) };
    (seconds.is_finite() && seconds >= 0.).then(|| Duration::from_secs_f64(seconds))
}
