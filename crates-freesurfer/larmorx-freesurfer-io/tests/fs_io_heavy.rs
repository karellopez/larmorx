// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The tool behind `cases::HEAVY_SEED1` (ignored; Linux, a debug build): the CPU time of each of
//! seed 1's 4,800 portable cases, and the indices of those above 0.25 s, which the recorded test
//! (`fs_io_official.rs`) leaves to the probe's runs. Run it after a change to the case generators:
//!
//!     cargo test -p larmorx-freesurfer-io --test fs_io_heavy -- --ignored --nocapture

#[path = "fs_io/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;

/// This thread's CPU time in seconds (`/proc/thread-self/schedstat`'s first field).
#[cfg(target_os = "linux")]
fn cpu() -> f64 {
    let s = std::fs::read_to_string("/proc/thread-self/schedstat").expect("schedstat");
    s.split_whitespace()
        .next()
        .and_then(|x| x.parse::<f64>().ok())
        .expect("a number")
        / 1e9
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "a measuring tool, not a check"]
fn heavy_cases_of_seed_1() {
    let dir = tempfile::Builder::new()
        .prefix("fs io ~")
        .tempdir()
        .expect("a temporary directory");
    let w = cases::Work {
        fix: dir.path().join("fix"),
        dat: dir.path().join("dat"),
    };
    cases::make_fixture(&w.fix);
    let (mut heavy, mut total, mut light) = (Vec::new(), 0.0, 0.0);
    for (i, line) in cases::generate(1, 4800, true).iter().enumerate() {
        let c0 = cpu();
        let _ = cases::run_line(line, UpstreamDefects::Reproduce, &w);
        let dt = cpu() - c0;
        total += dt;
        if dt > 0.25 {
            let head: String = line.chars().take(60).collect();
            eprintln!("HEAVY {i} {dt:.3} s  {head}");
            heavy.push(i);
        } else {
            light += dt;
        }
    }
    cases::remove_fixture(&w.fix);
    eprintln!(
        "CPU {total:.2} s in all, {light:.2} s without the heavy ones; HEAVY_SEED1 = {heavy:?}"
    );
}
