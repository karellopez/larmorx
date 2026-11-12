// SPDX-License-Identifier: Apache-2.0
//! `larmorx_core`'s ports of VNL and netlib (FreeSurfer's foundations, item 3.1-07) against ITK
//! 4.13.2's own VNL, as the FreeSurfer unit oracle built it (GCC 8.4.0, FreeSurfer 8.2.0's flags;
//! `build.sh`).
//!
//! `fs-vnl <libfsvnl.so> [scale] [--only <family>]`: seeded random cases of every family, `scale`
//! times the default counts (1). The families: `fmt` (C's `printf` against `larmorx_core::cfmt`),
//! `min` (`vnl_bracket_minimum`, `vnl_brent_minimizer`'s four entry points, the deprecated
//! `vnl_brent` as `fs_powell` steps with it, `vnl_powell`, every point the cost function is called
//! at), `lbfgs` (netlib's `lbfgs_` in lock step with the port under `reproduce`: `x`, `w`, `diag`,
//! `iflag` and the whole state after every call, and the printed text), `linalg` (the small
//! matrices' determinants and inverses, QR in float and double, the float SVD, the SVD inverse,
//! the symmetric and the real eigensystems, Cholesky), `fft` (`vnl_fft_1d<double>`); `--only itk103` measures ITK-103's correction on the port alone
//! (not part of the default run). Prints the
//! counts per family and `VERIFY-PASS` or `VERIFY-FAIL` (exit status 1).

mod checks_lbfgs;
mod checks_linalg;
mod checks_min;
mod ffi;

use std::fmt::Write as _;

pub use ffi::Lib;

/// Comparisons and differences of one family.
#[derive(Default)]
pub struct Tally {
    pub name: &'static str,
    pub cases: u64,
    pub values: u64,
    pub differ: u64,
    pub shown: u32,
    pub notes: Vec<String>,
    /// Compare a NaN with a NaN as equal (their signs and payloads counted in `nan_bits`): where
    /// two NaNs meet, x86's result is the first operand's, an order the source leaves to the
    /// compiler (GCC's and LLVM's differ).
    pub nan_as_nan: bool,
    pub nan_bits: u64,
}

impl Tally {
    pub fn new(name: &'static str) -> Tally {
        Tally {
            name,
            ..Default::default()
        }
    }

    /// Compares two values bit for bit (`what` is built only for a difference).
    pub fn bits(&mut self, what: &dyn Fn() -> String, ours: u64, theirs: u64) -> bool {
        self.values += 1;
        if ours != theirs {
            if self.nan_as_nan && f64::from_bits(ours).is_nan() && f64::from_bits(theirs).is_nan() {
                self.nan_bits += 1;
                return true;
            }
            self.differ += 1;
            if self.shown < 12 {
                self.shown += 1;
                println!("  DIFF [{}] {}: port {ours:#018x} original {theirs:#018x}", self.name, what());
            }
            return false;
        }
        true
    }

    pub fn f64s(&mut self, what: &dyn Fn() -> String, ours: &[f64], theirs: &[f64]) -> bool {
        let mut ok = ours.len() == theirs.len();
        if !ok {
            self.values += 1;
            self.differ += 1;
            if self.shown < 12 {
                self.shown += 1;
                println!("  DIFF [{}] {}: lengths {} and {}", self.name, what(), ours.len(), theirs.len());
            }
            return false;
        }
        for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
            ok &= self.bits(&|| format!("{} [{i}]", what()), a.to_bits(), b.to_bits());
        }
        ok
    }

    pub fn f32s(&mut self, what: &dyn Fn() -> String, ours: &[f32], theirs: &[f32]) -> bool {
        let mut ok = true;
        for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
            if self.nan_as_nan && a.is_nan() && b.is_nan() {
                self.values += 1;
                if a.to_bits() != b.to_bits() {
                    self.nan_bits += 1;
                }
                continue;
            }
            ok &= self.bits(&|| format!("{} [{i}]", what()), u64::from(a.to_bits()), u64::from(b.to_bits()));
        }
        ok
    }

    pub fn text(&mut self, what: &dyn Fn() -> String, ours: &str, theirs: &str) -> bool {
        self.values += 1;
        if ours != theirs {
            self.differ += 1;
            if self.shown < 12 {
                self.shown += 1;
                println!("  DIFF [{}] {}:\n    port     {ours:?}\n    original {theirs:?}", self.name, what());
            }
            return false;
        }
        true
    }
}

fn check_fmt(lib: &Lib, scale: u64, t: &mut Tally) {
    use larmorx_core::cfmt;
    use larmorx_core::x87::cases::SplitMix64;
    let mut rng = SplitMix64::new(0x5eed_f0f0);
    let mut buf = vec![0u8; 512];
    let mut c = |fmt: &str, v: f64| -> String {
        let f = std::ffi::CString::new(fmt).unwrap();
        // SAFETY: a buffer of the stated length and a NUL-terminated format with one double.
        let n = unsafe { (lib.snprintf)(buf.as_mut_ptr().cast(), buf.len() as i32, f.as_ptr(), v) };
        String::from_utf8_lossy(&buf[..n as usize]).into_owned()
    };
    let specials = [
        0.0,
        -0.0,
        f64::NAN,
        -f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        0.5,
        1.5,
        2.5,
        0.125,
        999_999.5,
        9.999_995,
        1e-5,
        1e-4,
        0.000_099_999_95,
        123_456.5,
        f64::MIN_POSITIVE,
        5e-324,
        f64::MAX,
    ];
    for k in 0..(200_000 * scale) as usize {
        let v = if k < specials.len() {
            specials[k]
        } else {
            match rng.below(4) {
                0 => f64::from_bits(rng.next_u64()),
                1 => (rng.range(-100_000_000, 100_000_000) as f64) / 1024.0,
                2 => (rng.range(-1_000_000, 1_000_000) as f64) * 10f64.powi(rng.range(-12, 12) as i32),
                _ => (rng.range(-2000, 2000) as f64) / 8.0,
            }
        };
        t.cases += 1;
        let p = rng.below(9) as usize;
        let w = rng.below(14) as usize;
        t.text(&|| format!("%.{p}g of {v:e}"), &cfmt::g(v, p), &c(&format!("%.{p}g"), v));
        t.text(&|| format!("%.{p}e of {v:e}"), &cfmt::e(v, p), &c(&format!("%.{p}e"), v));
        if v.abs() < 1e30 || !v.is_finite() {
            t.text(&|| format!("%{w}.{p}f of {v:e}"), &cfmt::pad(&cfmt::f(v, p), w), &c(&format!("%{w}.{p}f"), v));
        }
        t.text(&|| format!("%10g of {v:e}"), &cfmt::ostream_setw(v, 10), &c("%10g", v));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lib_path = args.get(1).expect("usage: fs-vnl <libfsvnl.so> [scale] [--only <family>]");
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
    let lib = ffi::load(lib_path);
    let want = |f: &str| only.as_deref().is_none_or(|o| o == f);
    let mut tallies = Vec::new();
    if want("fmt") {
        let mut t = Tally::new("fmt");
        check_fmt(&lib, scale, &mut t);
        tallies.push(t);
    }
    if want("min") {
        let mut t = Tally::new("min");
        checks_min::run(&lib, scale, &mut t);
        tallies.push(t);
    }
    if want("lbfgs") {
        let mut t = Tally::new("lbfgs");
        checks_lbfgs::run(&lib, scale, &mut t);
        tallies.push(t);
    }
    if only.as_deref() == Some("itk103") {
        let mut t = Tally::new("itk103");
        checks_lbfgs::measure_itk103(20_000 * scale, &mut t);
        tallies.push(t);
    }
    if want("linalg") {
        let mut t = Tally::new("linalg");
        t.nan_as_nan = true;
        checks_linalg::run(&lib, scale, &mut t);
        tallies.push(t);
    }
    if want("fft") {
        let mut t = Tally::new("fft");
        checks_linalg::run_fft(&lib, scale, &mut t);
        tallies.push(t);
    }
    let mut report = String::new();
    let mut failed = false;
    for t in &tallies {
        let _ = writeln!(
            report,
            "{:<7} {:>10} cases {:>14} values compared {:>8} differ",
            t.name, t.cases, t.values, t.differ
        );
        for n in &t.notes {
            let _ = writeln!(report, "        {n}");
        }
        if t.nan_as_nan {
            let _ = writeln!(report, "        NaN compared as NaN: {} with other sign or payload bits", t.nan_bits);
        }
        failed |= t.differ > 0;
    }
    print!("{report}");
    println!("{}", if failed { "VERIFY-FAIL" } else { "VERIFY-PASS" });
    if failed {
        std::process::exit(1);
    }
}
