// SPDX-License-Identifier: LicenseRef-FSL
//! The replica's Kiss FFT (Armadillo 14.2.1's engine) on the inputs of
//! `validation/probes/fsl-avwutils-lib/fft_probe.cc`: the same lines, for `check_fft.py`.
//! `LARMORX_FFT_SIZES="60 38" cargo test --release -p larmorx-nc-core --test fft_probe --
//! --ignored --nocapture > rust.txt`.

use larmorx_nc_core::kissfft::{C, KissFft};

struct Lcg(u32);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from((self.0 >> 8) as f32 / 16_777_216.0f32 - 0.5f32)
    }
}

#[test]
#[ignore = "a probe against Armadillo: run by hand"]
fn kiss_fft_lines() {
    let sizes = std::env::var("LARMORX_FFT_SIZES").unwrap_or_else(|_| "60 38".into());
    let mut rng = Lcg(12345);
    for n in sizes
        .split_whitespace()
        .map(|s| s.parse::<usize>().unwrap())
    {
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
                KissFft::new(n, inverse).transform(&x, &mut out);
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

/// The twiddle factors' `cos` and `sin` as the replica computes them (correctly rounded,
/// `larmorx_core::math`), for `check_fft.py`'s comparison with mpmath.
#[test]
#[ignore = "a probe: run by hand"]
fn twiddles() {
    let sizes = std::env::var("LARMORX_FFT_SIZES").unwrap_or_else(|_| "60 38".into());
    for n in sizes
        .split_whitespace()
        .map(|s| s.parse::<usize>().unwrap())
    {
        for sign in [-2.0f64, 2.0] {
            let k = sign * larmorx_core::math::acos(-1.0) / n as f64;
            for i in 0..n {
                let y = i as f64 * k;
                println!(
                    "tw {n} {sign} {i} {:016x} {:016x} {:016x}",
                    y.to_bits(),
                    larmorx_core::math::cos(y).to_bits(),
                    larmorx_core::math::sin(y).to_bits()
                );
            }
        }
    }
}
