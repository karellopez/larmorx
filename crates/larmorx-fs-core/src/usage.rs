// SPDX-License-Identifier: Apache-2.0
//! Memory and usage reports (spec §16): the memory used, the memory usage and its peak, the
//! resource usage lines. Read from Linux's `/proc` without the C library (`getrusage`'s values
//! from `/proc/self/stat`, `status` and `io`); elsewhere the reads fail as FreeSurfer's do where
//! `/proc` is missing. These values describe the run, never a result.

use std::io::Write;

use crate::process::Process;

fn status_text() -> Option<Vec<u8>> {
    std::fs::read(format!("/proc/{}/status", std::process::id())).ok()
}

/// The `kB` value of `tag` in a `/proc/<pid>/status` text (the tag matched without regard to
/// case), read as a decimal integer.
fn status_value(text: &[u8], tag: &str) -> Option<i64> {
    text.split(|&c| c == b'\n').find_map(|l| {
        let (k, v) = l.split_at(l.iter().position(|&c| c == b':')?);
        if !k.eq_ignore_ascii_case(tag.as_bytes()) {
            return None;
        }
        let (n, used) = larmorx_core::cscan::strtol10(&v[1..]);
        (used > 0).then_some(n)
    })
}

/// The memory-usage tags in the order of their five values.
pub const MEM_TAGS: [&str; 5] = ["VmSize", "VmPeak", "VmRSS", "VmData", "VmStk"];

/// The resource-usage lines' names (each twelve characters, the value right after).
pub const RUSAGE_NAMES: [&str; 16] = [
    "utimesec    ",
    "stimesec    ",
    "ru_maxrss   ",
    "ru_ixrss    ",
    "ru_idrss    ",
    "ru_isrss    ",
    "ru_minflt   ",
    "ru_majflt   ",
    "ru_nswap    ",
    "ru_inblock  ",
    "ru_oublock  ",
    "ru_msgsnd   ",
    "ru_msgrcv   ",
    "ru_nsignals ",
    "ru_nvcsw    ",
    "ru_nivcsw   ",
];

/// The process's resource usage (`getrusage(RUSAGE_SELF)`): user and system seconds, then the
/// fourteen counters (those Linux keeps at 0 are 0).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResourceUsage {
    pub utime: f64,
    pub stime: f64,
    pub counters: [i64; 14],
}

/// The clock ticks per second of `/proc/<pid>/stat`'s times (Linux's user-visible rate).
const TICKS: f64 = 100.0;

/// Reads the process's resource usage (all zero where `/proc` is missing).
pub fn resource_usage() -> ResourceUsage {
    let mut r = ResourceUsage::default();
    if let Ok(stat) = std::fs::read("/proc/self/stat") {
        // the fields after the command's closing parenthesis, from the state (field 3)
        if let Some(close) = stat.iter().rposition(|&c| c == b')') {
            let f: Vec<i64> = stat[close + 1..]
                .split(|c| c.is_ascii_whitespace())
                .filter(|x| !x.is_empty())
                .map(|x| {
                    std::str::from_utf8(x)
                        .ok()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0)
                })
                .collect();
            let field = |n: usize| f.get(n - 3).copied().unwrap_or(0);
            r.utime = field(14) as f64 / TICKS;
            r.stime = field(15) as f64 / TICKS;
            r.counters[4] = field(10);
            r.counters[5] = field(12);
        }
    }
    if let Some(s) = status_text() {
        r.counters[0] = status_value(&s, "VmHWM").unwrap_or(0);
        r.counters[12] = status_value(&s, "voluntary_ctxt_switches").unwrap_or(0);
        r.counters[13] = status_value(&s, "nonvoluntary_ctxt_switches").unwrap_or(0);
    }
    if let Ok(io) = std::fs::read("/proc/self/io") {
        r.counters[7] = status_value(&io, "read_bytes").unwrap_or(0) / 512;
        r.counters[8] = status_value(&io, "write_bytes").unwrap_or(0) / 512;
    }
    r
}

/// The sixteen resource-usage lines `<prefix><name><value>`.
pub fn rusage_lines(prefix: &[u8], r: &ResourceUsage) -> Vec<u8> {
    let mut b = Vec::new();
    for (i, name) in RUSAGE_NAMES.iter().enumerate() {
        b.extend_from_slice(prefix);
        b.extend_from_slice(name.as_bytes());
        let v = match i {
            0 => format!("{:.6}", r.utime),
            1 => format!("{:.6}", r.stime),
            _ => r.counters[i - 2].to_string(),
        };
        b.extend_from_slice(v.as_bytes());
        b.push(b'\n');
    }
    b
}

impl Process {
    /// The memory used (`getMemoryUsed`): the process's `VmData` in kB; when it cannot be read,
    /// `getting memoryused failed` (no newline) on standard error and -1.
    pub fn memory_used(&mut self) -> i32 {
        match status_text().and_then(|s| status_value(&s, "VmData")) {
            Some(v) => v as i32,
            None => {
                self.put_err(b"getting memoryused failed");
                -1
            }
        }
    }

    /// The memory usage (`GetMemUsage`): `VmSize`, `VmPeak`, `VmRSS`, `VmData`, `VmStk` in kB
    /// into `values`, each left as it was when absent.
    pub fn mem_usage(&mut self, values: &mut [i32; 5]) {
        if let Some(s) = status_text() {
            for (v, tag) in values.iter_mut().zip(MEM_TAGS) {
                if let Some(n) = status_value(&s, tag) {
                    *v = n as i32;
                }
            }
        }
    }

    /// The peak (`GetVmPeak`): the memory usage's `VmPeak`, its five values kept from one call
    /// to the next.
    pub fn vm_peak(&mut self) -> i32 {
        let mut v = self.vm_peak;
        self.mem_usage(&mut v);
        self.vm_peak = v;
        v[1]
    }

    /// The resource-usage lines on standard output (`PrintRUsage(RUSAGE_SELF, prefix,
    /// stdout)`); 0.
    pub fn print_rusage(&mut self, prefix: &[u8]) -> i32 {
        let b = rusage_lines(prefix, &resource_usage());
        self.put_out(&b);
        0
    }

    /// The resource-usage lines written to a new file `path` (`WriteRUsage`): 0, or 1 when the
    /// file cannot be created.
    pub fn write_rusage(&mut self, prefix: &[u8], path: &[u8]) -> i32 {
        match std::fs::File::create(crate::files::path_of(path)) {
            Ok(mut f) => {
                let b = rusage_lines(prefix, &resource_usage());
                let _ = f.write_all(&b);
                0
            }
            Err(_) => 1,
        }
    }
}
