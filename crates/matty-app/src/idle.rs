//! The automatic Away: how long the computer has had no keyboard or pointer
//! input, and what to tell the server about it, as the official desktop app
//! does. The server keeps a status the user picked (Away, Do Not Disturb,
//! Offline) and only moves an automatic Online to Away and back.
//!
//! Each platform reads the whole system's idle time, so someone typing in
//! another app stays Online. Where it can't be read (the web, a Wayland
//! desktop without an idle service), Matty says nothing, as before.

use std::time::Duration;

use web_time::Instant;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// Idle this long and the user is away: the official desktop app's five
/// minutes.
pub const AWAY_AFTER: Duration = Duration::from_secs(5 * 60);
/// How often the idle time is read.
pub const CHECK_EVERY: Duration = Duration::from_secs(30);
/// How often the server hears the same answer again. Active, it keeps the
/// server's last activity fresh; idle, it retries an Away the server held
/// back because something else counted as activity.
const REPEAT_EVERY: Duration = Duration::from_secs(60);

/// How long the computer has had no input, or `None` where that can't be
/// read.
pub async fn system_idle() -> Option<Duration> {
    #[cfg(target_os = "linux")]
    return linux::idle().await;
    #[cfg(target_os = "macos")]
    return macos::idle();
    #[cfg(target_os = "windows")]
    return windows::idle();
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    None
}

/// What the server was last told: active or not, and when.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Reported(Option<(bool, Instant)>);

impl Reported {
    /// Whether to tell the server the user is active (`Some(true)`) or
    /// idle (`Some(false)`) after `idle` without input, at `now`; `None`
    /// when the last word still stands. Records what it returns.
    pub fn next(&mut self, idle: Duration, now: Instant) -> Option<bool> {
        let active = idle < AWAY_AFTER;
        let due = match self.0 {
            Some((said, at)) => said != active || now.duration_since(at) >= REPEAT_EVERY,
            None => true,
        };
        if !due {
            return None;
        }
        self.0 = Some((active, now));
        Some(active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_past_five_minutes_says_away_and_input_says_active_again() {
        let start = Instant::now();
        let at = |seconds| start + Duration::from_secs(seconds);
        let mut reported = Reported::default();
        assert_eq!(reported.next(Duration::from_secs(5), at(0)), Some(true));
        assert_eq!(reported.next(Duration::from_secs(35), at(30)), None);
        assert_eq!(
            reported.next(Duration::from_secs(65), at(60)),
            Some(true),
            "active again after a minute"
        );
        assert_eq!(reported.next(AWAY_AFTER, at(300)), Some(false));
        assert_eq!(reported.next(AWAY_AFTER, at(330)), None);
        assert_eq!(
            reported.next(AWAY_AFTER, at(360)),
            Some(false),
            "away again after a minute"
        );
        assert_eq!(reported.next(Duration::ZERO, at(370)), Some(true));
    }
}
