// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's timer and date string (`fs.core`, queue item 3.1-22), translated from FreeSurfer
//! 8.2.0's `include/timer.h` (`Timer`, `TIMER_INTERVAL_BEGIN`, `TIMER_INTERVAL_END`) and
//! `utils/timer.cpp` (`Timer`'s methods, `currentDateTime`).
//!
//! FreeSurfer's `Timer` reads `std::chrono::high_resolution_clock`, which libstdc++ makes the
//! wall clock; the replica reads the monotonic clock, so a clock change during a run does not
//! show in its timings (timings never reach a result). The conversions are libstdc++'s
//! `duration_cast`s: whole milliseconds truncated, seconds `ns / 1e9`, minutes `ns / 6e10`, hours
//! `ns / 3.6e12`, each in `double`.

use std::time::Instant;

use crate::ctime;
use crate::run::Environment;

/// `Timer`: started when made.
#[derive(Clone, Copy, Debug)]
pub struct Timer {
    begin: Instant,
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

impl Timer {
    /// `Timer()`.
    pub fn new() -> Self {
        Timer {
            begin: Instant::now(),
        }
    }

    /// `nanoseconds()`.
    pub fn nanoseconds(&self) -> i64 {
        i64::try_from(self.begin.elapsed().as_nanos()).unwrap_or(i64::MAX)
    }

    /// `milliseconds()`.
    pub fn milliseconds(&self) -> i64 {
        self.nanoseconds() / 1_000_000
    }

    /// `seconds()`.
    pub fn seconds(&self) -> f64 {
        self.nanoseconds() as f64 / 1e9
    }

    /// `minutes()`.
    pub fn minutes(&self) -> f64 {
        self.nanoseconds() as f64 / 6e10
    }

    /// `hours()`.
    pub fn hours(&self) -> f64 {
        self.nanoseconds() as f64 / 3.6e12
    }

    /// `reset()`.
    pub fn reset(&mut self) {
        self.begin = Instant::now();
    }

    /// `TIMER_INTERVAL_END(NAME)`'s line: `<file>:<line> interval took <ms> msec`.
    pub fn interval_line(&self, file: &str, line: u32) -> String {
        format!("{file}:{line} interval took {} msec", self.milliseconds())
    }
}

/// `currentDateTime(allowOverride)` at time `t`: `FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING`
/// when it is set and the override is allowed, else the local time as `strftime("%c")` writes it
/// in the C locale (`Thu Nov 12 20:55:19 2026`).
pub fn current_date_time_at(env: &Environment, allow_override: bool, t: i64) -> String {
    if allow_override && let Some(v) = env.get("FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING") {
        return v.to_string_lossy().into_owned();
    }
    ctime::strftime_c(&ctime::localtime(env, t))
}

/// `currentDateTime(allowOverride)` now.
pub fn current_date_time(env: &Environment, allow_override: bool) -> String {
    current_date_time_at(env, allow_override, ctime::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_and_date() {
        let mut t = Timer::new();
        assert!(t.milliseconds() >= 0 && t.seconds() >= 0.0);
        t.reset();
        assert!(
            t.interval_line("f.cpp", 3)
                .starts_with("f.cpp:3 interval took ")
        );
        let env = Environment::from_pairs([("TZ", "UTC0")]);
        assert_eq!(
            current_date_time_at(&env, true, 0),
            "Thu Jan  1 00:00:00 1970"
        );
        let env = Environment::from_pairs([
            ("TZ", "UTC0"),
            ("FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING", "then"),
        ]);
        assert_eq!(current_date_time_at(&env, true, 0), "then");
        assert_eq!(
            current_date_time_at(&env, false, 0),
            "Thu Jan  1 00:00:00 1970"
        );
    }
}
