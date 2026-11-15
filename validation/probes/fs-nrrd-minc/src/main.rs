// SPDX-License-Identifier: Apache-2.0
//! larmorx-io's NRRD reader (`larmorx_io::nrrd`, NrrdIO r3991's `nrrdLoad`) and MINC volume
//! input (`larmorx_io::minc`) against the originals (`build.sh`):
//!
//! - `libnrrd_up.so`: NrrdIO r3991 from Teem's repository, built as FreeSurfer 8.2.0 builds its
//!   copy; `libnrrd_fs.so`: FreeSurfer's `libnrrdio.a` from the unit oracle;
//! - the MINC libraries of `build_minc.sh`.
//!
//! Every original call runs in a forked child (`shim_*.c`), so that a crash, an exit or a loop
//! that never ends is an outcome, counted, rather than the end of the probe. A case whose port
//! (with `reproduce`) notes a defect of undefined behaviour is not compared; the originals'
//! outcomes on those cases are counted.
//!
//! `fs-nrrd-minc <build-dir> [count] [seed] [--only nrrd|minc]`. Prints the counts and
//! `VERIFY-PASS` or `VERIFY-FAIL` (exit status 1).

mod ffi;
mod minc;
mod nrrd;

/// splitmix64.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x2545_f491_4f6c_dd1d)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    /// A value in `0..n`.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = args
        .get(1)
        .expect("usage: fs-nrrd-minc <build-dir> [count] [seed] [--only nrrd|minc]");
    let count: u64 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(2000);
    let seed: u64 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(1);
    let only = args
        .iter()
        .position(|a| a == "--only")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let work = std::path::Path::new(dir).join(format!("work-{seed}"));
    std::fs::create_dir_all(&work).unwrap();
    let mut fail = false;
    if only.as_deref().is_none_or(|o| o == "nrrd") {
        let up = ffi::Lib::open(&format!("{dir}/libnrrd_up.so"), "fm_nrrd_load");
        let fs = ffi::Lib::open(&format!("{dir}/libnrrd_fs.so"), "fm_nrrd_load");
        let mut t = nrrd::Tally::default();
        nrrd::run(seed, count, &work, &up, &fs, &mut t);
        println!(
            "nrrd: {} cases, {} comparisons identical ({} of them reads that succeeded), {} differ, {} not compared (undefined in the original)",
            t.cases, t.same, t.read_ok, t.differ, t.skipped
        );
        for (k, v) in &t.outcomes {
            println!("  original on undefined cases: {k}: {v}");
        }
        for (k, v) in &t.notices {
            println!("  notices (fix): {k}: {v}");
        }
        fail |= t.differ > 0;
    }
    if only.as_deref().is_none_or(|o| o == "minc") {
        let up = ffi::MincLib::open(&format!("{dir}/libminc_up.so"));
        let fs = ffi::MincLib::open(&format!("{dir}/libminc_fs.so"));
        if let Some(i) = args.iter().position(|a| a == "--files") {
            minc::files(std::path::Path::new(&args[i + 1]), &up);
            return;
        }
        if let Ok(one) = std::env::var("LXPROBE_ONE") {
            minc::one(one.as_bytes(), &up, &fs, &work);
            return;
        }
        let mut t = minc::Tally::default();
        minc::run(seed, count, &work, &up, &fs, &mut t);
        println!(
            "minc: {} cases ({} not run: an image of more than 2^22 values announced), {} comparisons identical ({} of them reads that succeeded), {} differ, {} not compared (undefined in the original)",
            t.cases, t.too_big, t.same, t.read_ok, t.differ, t.skipped
        );
        for (k, v) in &t.outcomes {
            println!("  original on undefined cases: {k}: {v}");
        }
        for (k, v) in &t.notices {
            println!("  notices (fix): {k}: {v}");
        }
        fail |= t.differ > 0;
    }
    println!("{}", if fail { "VERIFY-FAIL" } else { "VERIFY-PASS" });
    if fail {
        std::process::exit(1);
    }
}
