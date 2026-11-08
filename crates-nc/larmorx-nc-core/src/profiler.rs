// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-utils 2412.6: FSLProfiler.{h,cpp}). Copyright (C)
// University of Oxford (Jesper Andersson). Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the originals. Distributed under the FSL Licence (see LICENSE).
//! `Utilities::FSLProfiler`: timed entries written to `<base name><file name>` once profiling is
//! on (`SetProfilingOn(base)` before the profiler is made, as topup's `--profile` does). An
//! entry's key is the clock's time since the epoch (seconds, as a double); ending it writes
//! `Entry: <description> Duration: <seconds>` (C++'s default stream format), each line
//! flushed. The durations are the clock's: only their form can be compared with FSL's.

use std::collections::BTreeMap;
use std::io::Write;

use crate::cnum;
use crate::volume::FslError;

/// `FSLProfilerException(msg)`: `what()` is `"FSLProfiler::" + msg`.
pub fn profiler_error(msg: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "Utilities::FSLProfilerException",
        what: format!("FSLProfiler::{msg}"),
    }
}

/// `std::chrono::system_clock::now().time_since_epoch()` as `duration<double>`: the
/// nanoseconds as a double over 1e9.
fn now() -> f64 {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    d.as_nanos() as f64 / 1e9
}

/// One `FSLProfiler` instance.
pub struct Profiler {
    out: Option<std::fs::File>,
    /// `_ent`: the open entries by key (the key's bits order positive doubles as their
    /// values).
    ent: BTreeMap<u64, String>,
}

impl Profiler {
    /// `FSLProfiler(fname)` with profiling on (`Some(base)`) or off: on, the file
    /// `base + fname` (`/` and blanks in `fname` made `_`) is opened, or the exception.
    pub fn new(base: Option<&str>, fname: &str) -> Result<Profiler, FslError> {
        let fname: String = fname
            .chars()
            .map(|c| if c == '/' || c == ' ' { '_' } else { c })
            .collect();
        let out = match base {
            None => None,
            Some(b) => {
                let name = format!("{b}{fname}");
                match std::fs::File::create(&name) {
                    Ok(f) => Some(f),
                    Err(_) => {
                        return Err(profiler_error(&format!(
                            "FSLProfiler::FSLProfiler: Unable to open logfile {name}"
                        )));
                    }
                }
            }
        };
        Ok(Profiler {
            out,
            ent: BTreeMap::new(),
        })
    }

    fn line(&mut self, s: &str) {
        if let Some(f) = self.out.as_mut() {
            let _ = writeln!(f, "{s}");
        }
    }

    /// `StartEntry(descrip)`: the entry's key (0 when profiling is off, or when the key
    /// exists already: then a line says so).
    pub fn start_entry(&mut self, descrip: &str) -> f64 {
        if self.out.is_none() {
            return 0.0;
        }
        let t = now();
        if self.ent.contains_key(&t.to_bits()) {
            self.line(&format!(
                "Invalid entry: StartEntry with descrip \"{descrip}\" resulted in duplicate key {}",
                cnum::stream(t)
            ));
            return 0.0;
        }
        self.ent.insert(t.to_bits(), descrip.to_owned());
        t
    }

    /// `EndEntry(key)`: the entry's line with its duration.
    pub fn end_entry(&mut self, key: f64) {
        if self.out.is_none() || key == 0.0 {
            return;
        }
        let dur = now() - key;
        match self.ent.remove(&key.to_bits()) {
            Some(d) => self.line(&format!("Entry: {d} Duration: {}", cnum::stream(dur))),
            None => self.line(&format!(
                "Invalid entry: EndEntry called with invalid key {}",
                cnum::stream(key)
            )),
        }
    }

    /// The destructor's `flush_and_close()` (a profiler that goes out of scope, not one an
    /// `exit()` leaves behind): every open entry reported.
    pub fn close(&mut self) {
        let open: Vec<String> = self.ent.values().cloned().collect();
        for d in open {
            self.line(&format!(
                "Invalid entry: Uncompleted entry: description: {d}"
            ));
        }
        self.ent.clear();
        self.out = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_and_failures() {
        let dir = std::env::temp_dir().join(format!("lx-nc-profiler-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let base = format!("{}/p", dir.display());
        let mut p = Profiler::new(Some(&base), "_a b/c.txt").unwrap();
        let k = p.start_entry("Begins run");
        // the keys are the clock's time: on a coarse clock (macOS) two entries started at once
        // share it, and the second is the duplicate-key line
        std::thread::sleep(std::time::Duration::from_millis(2));
        let l = p.start_entry("Begins level 1");
        p.end_entry(l);
        p.end_entry(12.5);
        p.close();
        let _ = k;
        let text = std::fs::read_to_string(format!("{base}_a_b_c.txt")).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("Entry: Begins level 1 Duration: "));
        assert_eq!(
            lines[1],
            "Invalid entry: EndEntry called with invalid key 12.5"
        );
        assert_eq!(
            lines[2],
            "Invalid entry: Uncompleted entry: description: Begins run"
        );
        let e = Profiler::new(Some(&format!("{}/nodir/p", dir.display())), "_x.txt").err();
        assert!(e.unwrap().what.contains("Unable to open logfile"));
        let mut off = Profiler::new(None, "_x.txt").unwrap();
        assert_eq!(off.start_entry("x"), 0.0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
