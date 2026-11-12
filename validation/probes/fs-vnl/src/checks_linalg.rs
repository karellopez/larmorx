// SPDX-License-Identifier: Apache-2.0
//! The linear algebra FreeSurfer calls through VNL (numerics.cpp's `Open*` wrappers,
//! `MatrixSqrt`, `sc_linalg_cholesky_decomp`, `mri_robust_register`), and `vnl_fft_1d<double>`.

use larmorx_core::x87::cases::SplitMix64;
use larmorx_image::vnl_fft::VnlFft1d;

use crate::{Lib, Tally};

pub fn run(_lib: &Lib, _scale: u64, _t: &mut Tally) {}

/// `vnl_fft_1d<double>` for every power of 2 from 2 to 512 (N4 pads its histogram to one: 512
/// for FreeSurfer's 200 bins), random signals, both directions.
pub fn run_fft(lib: &Lib, scale: u64, t: &mut Tally) {
    let mut rng = SplitMix64::new(0xff7_5eed);
    for _ in 0..(40 * scale) {
        // Powers of 2 to 512: bit for bit. From 1024 the port's correctly rounded twiddle factors
        // differ from glibc's sincos (larmorx-image's documented choice): counted, not compared.
        for k in 1..=9 {
            let n = 1usize << k;
            t.cases += 1;
            let x: Vec<f64> = (0..2 * n)
                .map(|_| match rng.below(3) {
                    0 => (rng.range(-1_000_000, 1_000_000) as f64) / 1024.0,
                    1 => 0.0,
                    _ => f64::from_bits(rng.next_u64() & 0x3fff_ffff_ffff_ffff) * 1e-300,
                })
                .collect();
            let fft = VnlFft1d::new(n).expect("a power of 2");
            for forward in [true, false] {
                let mut ours = x.clone();
                let mut theirs = x.clone();
                if forward {
                    fft.fwd_transform(&mut ours);
                } else {
                    fft.bwd_transform(&mut ours);
                }
                // SAFETY: 2n doubles.
                unsafe { (lib.fft)(n as i32, i32::from(forward), theirs.as_mut_ptr()) };
                t.f64s(&|| format!("fft n={n} forward={forward}"), &ours, &theirs);
            }
        }
    }
}
