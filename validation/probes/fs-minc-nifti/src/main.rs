// SPDX-License-Identifier: Apache-2.0
//! larmorx-io's ports of MINC 1.5.1's `.xfm` reader and writer (`larmorx_io::xfm`) and of
//! nifticlib 2.0.0's file names and znzlib (`larmorx_io::nifti::niftilib`, `larmorx_io::znz`),
//! and larmorx-core's `cscan` (C's `sscanf` and `strtod`), against the originals (`build.sh`):
//!
//! - `libup.so`: MINC 1.5.1's volume_io and nifticlib 2.0.0, from their release tarballs, built
//!   with GCC 8.4.0 and FreeSurfer 8.2.0's flags: the originals the ports follow;
//! - `libfs.so`: FreeSurfer 8.2.0's copies (`packages/minc`, `packages/nifti`) as the unit oracle
//!   built them: what FreeSurfer's programs run. The port is compared with it through a dialect
//!   (`dialect.rs`) that holds what FreeSurfer changed, read from its behaviour alone;
//! - the C library itself for `cscan` (glibc's `sscanf` and `strtod`).
//!
//! `fs-minc-nifti <build-dir> [scale] [--only <family>]`: seeded cases of every family, `scale`
//! times the default counts. Families: `scan`, `xfm` (generated and mutated `.xfm` files read by
//! all three), `ops` (inverses, products, points, concatenations, `expand_filename`), `write` (the
//! writer against MINC 1.5.1's), `nifti`, `znz`. Prints the counts per family and `VERIFY-PASS` or
//! `VERIFY-FAIL` (exit status 1).

mod checks_nifti;
mod checks_scan;
mod checks_xfm;
mod dialect;
mod ffi;
mod generate;

pub use ffi::{Capture, Lib};

/// Comparisons and differences of one family.
#[derive(Default)]
pub struct Tally {
    pub name: String,
    pub cases: u64,
    pub values: u64,
    pub differ: u64,
    pub shown: u32,
    pub notes: Vec<String>,
}

impl Tally {
    pub fn new(name: &str) -> Tally {
        Tally {
            name: name.to_string(),
            ..Default::default()
        }
    }

    pub fn diff(&mut self, what: &dyn Fn() -> String) {
        self.differ += 1;
        let limit: u32 = std::env::var("LXPROBE_SHOW").ok().and_then(|v| v.parse().ok()).unwrap_or(15);
        if self.shown < limit {
            self.shown += 1;
            println!("  DIFF [{}] {}", self.name, what());
        }
    }

    pub fn eq<T: PartialEq + std::fmt::Debug>(&mut self, what: &dyn Fn() -> String, ours: T, theirs: T) -> bool {
        self.values += 1;
        if ours != theirs {
            self.diff(&|| format!("{}:\n    port     {ours:?}\n    original {theirs:?}", what()));
            return false;
        }
        true
    }

    pub fn bits(&mut self, what: &dyn Fn() -> String, ours: f64, theirs: f64) -> bool {
        self.eq(what, ours.to_bits(), theirs.to_bits())
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--replay") {
        // fs-minc-nifti --replay <file> <mode> <comp 0|1> <reproduce 0|1> <ops in zt's spelling>...
        checks_nifti::replay(&args[2..]);
        return;
    }
    let dir = std::path::PathBuf::from(args.get(1).expect("usage: fs-minc-nifti <build-dir> [scale] [--only <family>]"));
    let mut scale = 1u64;
    let mut only: Option<String> = None;
    let mut i = 2;
    while i < args.len() {
        if args[i] == "--only" {
            only = args.get(i + 1).cloned();
            i += 2;
        } else {
            scale = args[i].parse().expect("a scale");
            i += 1;
        }
    }
    let up = ffi::load(dir.join("libup.so").to_str().unwrap());
    let fs = ffi::load(dir.join("libfs.so").to_str().unwrap());
    let work = dir.join("work");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let mut cap = Capture::new(&work);
    let want = |f: &str| only.as_deref().is_none_or(|o| o == f);
    let mut tallies: Vec<Tally> = Vec::new();
    if want("scan") {
        tallies.extend(checks_scan::check(scale));
    }
    if want("xfm") {
        tallies.extend(checks_xfm::check_read(&up, &fs, &work, scale, &mut cap));
    }
    if want("ops") {
        tallies.extend(checks_xfm::check_ops(&up, &fs, &work, scale, &mut cap));
    }
    if want("write") {
        tallies.extend(checks_xfm::check_write(&up, &work, scale, &mut cap));
    }
    if want("nifti") {
        tallies.extend(checks_nifti::check_names(&up, &fs, &work, scale, &mut cap));
    }
    if want("znz") {
        tallies.extend(checks_nifti::check_znz(&up, &fs, &work, scale, &mut cap));
    }
    let mut fail = false;
    for t in &tallies {
        println!(
            "{:<14} cases {:>10} values {:>12} differ {:>8}",
            t.name, t.cases, t.values, t.differ
        );
        for n in &t.notes {
            println!("    {n}");
        }
        fail |= t.differ > 0;
    }
    println!("{}", if fail { "VERIFY-FAIL" } else { "VERIFY-PASS" });
    std::process::exit(if fail { 1 } else { 0 });
}
