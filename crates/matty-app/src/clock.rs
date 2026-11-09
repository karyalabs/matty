//! The user's local time zone, looked up once a minute rather than on
//! every call.
//!
//! `jiff::Zoned::now()` and `TimeZone::system()` ask the platform for the
//! time zone each time. In the browser that builds an
//! `Intl.DateTimeFormat`, about a millisecond each, and several views do
//! it while they draw: about 3% of the main thread's work while switching
//! channels (#818). Changing the zone while Matty runs still shows within a
//! minute.

use std::cell::RefCell;
use std::time::Duration;

use jiff::{Timestamp, Zoned, tz::TimeZone};
use web_time::Instant;

/// How long a looked-up zone is kept.
const KEEP: Duration = Duration::from_secs(60);

thread_local! {
    static ZONE: RefCell<Option<(TimeZone, Instant)>> = const { RefCell::new(None) };
}

/// The user's time zone.
pub fn zone() -> TimeZone {
    ZONE.with(|zone| {
        let mut zone = zone.borrow_mut();
        match &*zone {
            Some((tz, at)) if at.elapsed() < KEEP => tz.clone(),
            _ => {
                let tz = TimeZone::system();
                *zone = Some((tz.clone(), Instant::now()));
                tz
            }
        }
    })
}

/// Now, on the user's clock: [`Zoned::now`] without the lookup.
pub fn now() -> Zoned {
    Timestamp::now().to_zoned(zone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_is_in_the_system_zone() {
        assert_eq!(now().time_zone(), &TimeZone::system());
        assert!(
            (now().timestamp() - Zoned::now().timestamp())
                .get_seconds()
                .abs()
                < 5
        );
    }
}
