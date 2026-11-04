// SPDX-License-Identifier: LicenseRef-FSL
//! The replica's Kiss FFT (Armadillo 14.2.1's engine) on the inputs of
//! `validation/probes/fsl-avwutils-lib/fft_probe.cc`: the same lines, for `check_fft.py`
//! (which runs these ignored tests; see that directory's README):
//!
//! - `kiss_fft_lines`: the engine as fslmaths runs it (by default correctly rounded twiddle
//!   factors; `LARMORX_FFT_MODE=reproduce`: glibc's `sincos`, GLIBC-07);
//! - `kiss_fft_lines_with_twiddles`: the same engine handed Armadillo's own factors (the lines
//!   `fft_twiddles.cc` prints, in the file `LARMORX_FFT_TWIDDLES`), so that the transform's
//!   arithmetic is compared apart from the factors;
//! - `twiddles`: the factors the replica computes (in the mode `LARMORX_FFT_MODE` names).
//!
//! `LARMORX_FFT_SIZES="60 38" cargo test --release -p larmorx-nc-core --test fft_probe --
//! --ignored --nocapture kiss_fft_lines > rust.txt`.

use larmorx_core::defects::UpstreamDefects;
use larmorx_core::math::Libm;
use larmorx_nc_core::kissfft::{self, C, KissFft, parse_twiddle_table};

/// The mode `LARMORX_FFT_MODE` names: `fix` (the default) or `reproduce`.
fn libm() -> Libm {
    match std::env::var("LARMORX_FFT_MODE").as_deref() {
        Ok("reproduce") => Libm::quiet(UpstreamDefects::Reproduce),
        Ok("fix") | Err(_) => Libm::quiet(UpstreamDefects::Fix),
        Ok(other) => panic!("LARMORX_FFT_MODE={other}: fix or reproduce"),
    }
}

struct Lcg(u32);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from((self.0 >> 8) as f32 / 16_777_216.0f32 - 0.5f32)
    }
}

fn sizes() -> Vec<usize> {
    std::env::var("LARMORX_FFT_SIZES")
        .unwrap_or_else(|_| "60 38".into())
        .split_whitespace()
        .map(|s| s.parse::<usize>().unwrap())
        .collect()
}

/// `fft_probe.cc`'s lines, with the engines `make` gives.
fn lines(make: impl Fn(usize, bool) -> KissFft) {
    let mut rng = Lcg(12345);
    for n in sizes() {
        for rep in 0..3 {
            let x: Vec<C> = (0..n)
                .map(|_| {
                    let re = rng.next();
                    let im = if rep == 0 { 0.0 } else { rng.next() };
                    C { re, im }
                })
                .collect();
            for (dir, inverse) in [("fwd", false), ("inv", true)] {
                let mut out = vec![C::default(); n];
                make(n, inverse).transform(&x, &mut out);
                for (i, v) in out.iter().enumerate() {
                    println!(
                        "{n} {rep} {dir} {i} {:016x} {:016x}",
                        v.re.to_bits(),
                        v.im.to_bits()
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "a probe against Armadillo: run by hand"]
fn kiss_fft_lines() {
    let libm = libm();
    lines(|n, inverse| KissFft::new(n, inverse, &libm));
}

#[test]
#[ignore = "a probe against Armadillo: run by hand"]
fn kiss_fft_lines_with_twiddles() {
    let path = std::env::var("LARMORX_FFT_TWIDDLES").expect("LARMORX_FFT_TWIDDLES");
    let table = parse_twiddle_table(&std::fs::read_to_string(path).expect("the twiddle file"));
    lines(|n, inverse| {
        let coeffs = table
            .get(&(n, inverse))
            .unwrap_or_else(|| panic!("no factors for {n} {inverse}"))
            .clone();
        KissFft::with_twiddles(n, inverse, coeffs)
    });
}

/// The twiddle factors' `cos` and `sin` as the replica computes them (`larmorx_core::math`'s
/// correctly rounded ones, or glibc's `sincos` with `LARMORX_FFT_MODE=reproduce`), for
/// `check_fft.py`'s comparison with Armadillo's. At a zero angle the replica's factor is `(1,
/// angle)`, as glibc's `cexp` gives it.
#[test]
#[ignore = "a probe: run by hand"]
fn twiddles() {
    let libm = libm();
    for n in sizes() {
        for (sign, inverse) in [(-2.0f64, false), (2.0, true)] {
            let k = sign * larmorx_core::math::acos(-1.0) / n as f64;
            let factors = kissfft::twiddles(n, inverse, &libm);
            for (i, c) in factors.iter().enumerate() {
                let y = i as f64 * k;
                println!(
                    "tw {n} {sign} {i} {:016x} {:016x} {:016x}",
                    y.to_bits(),
                    c.re.to_bits(),
                    c.im.to_bits()
                );
            }
        }
    }
}
