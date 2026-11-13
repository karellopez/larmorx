// SPDX-License-Identifier: Apache-2.0
//! Times and dates (spec §14): the program's clock, its local time (the `TZ` and `TZDIR` of its
//! environment, read as the C library reads them: [`crate::clib::time`]), the current date
//! string and the timers.

use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::clib::time::{Tm, Zone, strftime_c};
use crate::process::Process;

/// The zone a run read, kept while its `TZ` and `TZDIR` stay the same.
pub(crate) struct ZoneCache {
    tz: Option<Vec<u8>>,
    tzdir: Option<Vec<u8>>,
    zone: Arc<Zone>,
}

/// The override of the current date string (§14).
pub const DATE_OVERRIDE: &str = "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING";

/// Seconds since 1970 of a system time (whole seconds, rounded down).
pub fn unix_seconds(t: SystemTime) -> i64 {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => {
            let d = e.duration();
            -(d.as_secs() as i64) - i64::from(d.subsec_nanos() > 0)
        }
    }
}

impl Process {
    /// The current time in seconds since 1970 ([`Process::clock`] when it is set).
    pub fn now(&self) -> i64 {
        self.clock
            .unwrap_or_else(|| unix_seconds(SystemTime::now()))
    }

    /// The run's time zone (read again when `TZ` or `TZDIR` changed).
    pub fn zone(&mut self) -> Arc<Zone> {
        let tz = self.env.get("TZ").map(<[u8]>::to_vec);
        let tzdir = self.env.get("TZDIR").map(<[u8]>::to_vec);
        if let Some(c) = &self.zone
            && c.tz == tz
            && c.tzdir == tzdir
        {
            return Arc::clone(&c.zone);
        }
        let zone = Arc::new(Zone::from_tz(tz.as_deref(), tzdir.as_deref()));
        self.zone = Some(ZoneCache {
            tz,
            tzdir,
            zone: Arc::clone(&zone),
        });
        zone
    }

    /// `localtime_r(t)` in the run's zone.
    pub fn localtime(&mut self, t: i64) -> Option<Tm> {
        self.zone().localtime(t)
    }

    /// `gmtime_r(t)` (the run's zone's leap seconds applied, as the C library does).
    pub fn gmtime(&mut self, t: i64) -> Option<Tm> {
        self.zone().gmtime(t)
    }

    /// `strftime("%c")` of `t`'s local time.
    pub fn local_c_string(&mut self, t: i64) -> Option<Vec<u8>> {
        self.localtime(t).map(|tm| strftime_c(&tm))
    }

    /// The current date string (`currentDateTime(allowOverride)`): the override variable's
    /// value when it is set and allowed (an empty value gives the empty string), else the local
    /// time in the C locale's `%c` form.
    pub fn current_date_time(&mut self, allow_override: bool) -> Vec<u8> {
        if allow_override && let Some(v) = self.env.get(DATE_OVERRIDE) {
            return v.to_vec();
        }
        let now = self.now();
        self.local_c_string(now).unwrap_or_default()
    }

    /// The interval line of a timer (`TIMER_INTERVAL_END`): `<file>:<line> interval took <ms>
    /// msec` on standard output.
    pub fn timer_interval_end(&mut self, file: &[u8], line: i32, timer: &Timer) {
        let mut b = file.to_vec();
        b.extend_from_slice(
            format!(":{line} interval took {} msec\n", timer.milliseconds()).as_bytes(),
        );
        self.put_out(&b);
    }
}

/// A timer (FreeSurfer's `Timer`): the time elapsed since it was made or reset. FreeSurfer's
/// clock is the wall clock; this one is monotonic (timings never reach a result).
#[derive(Clone, Copy, Debug)]
pub struct Timer {
    start: Instant,
}

impl Default for Timer {
    fn default() -> Timer {
        Timer::new()
    }
}

impl Timer {
    /// A timer started now.
    pub fn new() -> Timer {
        Timer {
            start: Instant::now(),
        }
    }

    /// Starts it again.
    pub fn reset(&mut self) {
        self.start = Instant::now();
    }

    /// Nanoseconds elapsed.
    pub fn nanoseconds(&self) -> i64 {
        i64::try_from(self.start.elapsed().as_nanos()).unwrap_or(i64::MAX)
    }

    /// Milliseconds elapsed, truncated.
    pub fn milliseconds(&self) -> i64 {
        self.nanoseconds() / 1_000_000
    }

    /// Seconds elapsed (`ns / 1e9` in double).
    pub fn seconds(&self) -> f64 {
        self.nanoseconds() as f64 / 1e9
    }

    /// Minutes elapsed (`ns / 6e10`).
    pub fn minutes(&self) -> f64 {
        self.nanoseconds() as f64 / 6e10
    }

    /// Hours elapsed (`ns / 3.6e12`).
    pub fn hours(&self) -> f64 {
        self.nanoseconds() as f64 / 3.6e12
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Environment;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn date_override_and_clock() {
        let env = Environment::from_pairs([("TZ", "UTC0"), (DATE_OVERRIDE, "x y")]);
        let mut p = Process::silent(env, UpstreamDefects::Fix);
        p.clock = Some(0);
        assert_eq!(p.current_date_time(true), b"x y");
        assert_eq!(p.current_date_time(false), b"Thu Jan  1 00:00:00 1970");
        assert_eq!(
            unix_seconds(UNIX_EPOCH - std::time::Duration::from_millis(1)),
            -1
        );
    }
}
