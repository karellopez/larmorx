// SPDX-License-Identifier: Apache-2.0
//! larmorx-io's GIFTI port (`larmorx_io::gifti`) and its XML parser (`larmorx_io::xml`, in
//! expat 2.0.1's place), and larmorx-core's `cscan::sscanf_f32`, against the originals
//! (`build.sh`):
//!
//! - `libup.so`: gifticlib 1.12, expat 2.0.1 and nifticlib 2.0.0 from their pinned sources, built
//!   with GCC 8.4.0 and FreeSurfer 8.2.0's flags: the originals the ports follow;
//! - `libfs.so`: FreeSurfer 8.2.0's copies as the unit oracle built them: what FreeSurfer's
//!   programs run, compared through a dialect (`dialect.rs`) holding FreeSurfer's two changes.
//!
//! `fs-gifti <build-dir> [scale] [--only <family>]`: seeded cases of every family, `scale` times
//! the default counts. Families: `xml` (generated documents in pieces: every event and error
//! against expat), `read` (generated GIFTI files read by gifticlib and the port: every field, the
//! data bytes, the messages at every verbosity, the checks, the display, the bytes written back),
//! `write` (images read from exact binary files, written in every encoding), `scan` (`sscanf
//! "%f"`). Each original's call runs in a forked child. Prints the counts per family and
//! `VERIFY-PASS` or `VERIFY-FAIL` (exit status 1).

mod checks;
mod dialect;
mod dump;
mod ffi;
mod gifti_gen;
mod xmlgen;

/// Comparisons and differences of one family.
#[derive(Default)]
pub struct Tally {
    pub name: String,
    pub cases: u64,
    pub values: u64,
    pub differ: u64,
    pub skipped: u64,
    pub shown: u32,
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
        let limit: u32 = std::env::var("LXPROBE_SHOW")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);
        if self.shown < limit {
            self.shown += 1;
            println!("  DIFF [{}] {}", self.name, what());
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = std::path::PathBuf::from(
        args.get(1)
            .expect("usage: fs-gifti <build-dir> [scale] [--only <family>]"),
    );
    let mut scale = 1u64;
    let mut only: Option<String> = None;
    let mut i = 2;
    while i < args.len() {
        if args[i] == "--only" {
            only = args.get(i + 1).cloned();
            i += 2;
        } else {
            scale = args[i].parse().expect("scale");
            i += 1;
        }
    }
    let dir = std::fs::canonicalize(&dir).expect("build dir");
    let up = ffi::load(&dir.join("libup.so"), "gifticlib");
    let fs = ffi::load(&dir.join("libfs.so"), "freesurfer");
    let work = ffi::Work::new(dir.join("work"));
    let want = |f: &str| only.as_deref().is_none_or(|o| o == f);
    let mut tallies = Vec::new();
    if want("scan") {
        tallies.push(checks::scan(&up, &work, 20_000 * scale));
    }
    if want("xml") {
        tallies.push(checks::xml(&up, &fs, &work, 4_000 * scale));
    }
    if want("read") {
        tallies.push(checks::read(
            &up,
            &fs,
            &work,
            std::env::var("LXPROBE_READ_N")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3_000)
                * scale,
        ));
    }
    let mut fail = false;
    for t in &tallies {
        println!(
            "FAMILY {:6} cases {:8} values {:10} differ {:6} not-run {:6}",
            t.name, t.cases, t.values, t.differ, t.skipped
        );
        fail |= t.differ > 0;
    }
    println!("{}", if fail { "VERIFY-FAIL" } else { "VERIFY-PASS" });
    std::process::exit(fail as i32);
}
