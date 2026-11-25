// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The port's side of the fs-coreg probe (`validation/probes/fs-coreg/`):
//!
//!     fs_coreg_probe cases <seed> <n>            the case file, on standard output
//!     fs_coreg_probe golden-cases                the golden cases (`tests/mri_coreg_golden.rs`)
//!     fs_coreg_probe run <cases> fix|reproduce   one line per case: the output, a tab, the
//!                                               defects the port noted (comma-separated); the
//!                                               cases side by side on the machine's threads

#[path = "../tests/fs_coreg/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;

fn mode(m: &str) -> UpstreamDefects {
    match m {
        "fix" => UpstreamDefects::Fix,
        "reproduce" => UpstreamDefects::Reproduce,
        m => panic!("mode {m}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("cases") if args.len() == 4 => {
            let seed: u64 = args[2].parse().expect("seed");
            let n: usize = args[3].parse().expect("n");
            for line in cases::generate(seed, n) {
                println!("{line}");
            }
        }
        Some("golden-cases") => {
            for line in cases::golden_cases() {
                println!("{line}");
            }
        }
        Some("run") if args.len() == 4 => {
            let m = mode(&args[3]);
            let text = std::fs::read_to_string(&args[2]).expect("cases");
            let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
            let jobs: usize = std::env::var("PROBE_JOBS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(4);
            let next = std::sync::atomic::AtomicUsize::new(0);
            let outs: Vec<std::sync::Mutex<String>> = lines
                .iter()
                .map(|_| std::sync::Mutex::new(String::new()))
                .collect();
            std::thread::scope(|sc| {
                for _ in 0..jobs.max(1) {
                    sc.spawn(|| {
                        loop {
                            let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if k >= lines.len() {
                                break;
                            }
                            let (out, ids) = cases::run_line(lines[k], m);
                            *outs[k].lock().expect("a line") = format!("{out}\t{}", ids.join(","));
                        }
                    });
                }
            });
            for o in outs {
                println!("{}", o.into_inner().expect("a line"));
            }
        }
        _ => {
            eprintln!(
                "usage: fs_coreg_probe cases <seed> <n> | golden-cases | run <cases> fix|reproduce"
            );
            std::process::exit(2);
        }
    }
}
