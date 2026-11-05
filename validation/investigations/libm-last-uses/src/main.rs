// SPDX-License-Identifier: Apache-2.0
// The libm crate's last uses against larmorx_core::math and the C library (glibc, black box,
// through Rust's std), on the inputs each use meets.
use rayon::prelude::*;
use larmorx_core::math;

fn main() {
    // (a) AFNI's oblique angle: every float32 x in [0, 1] (the smallest cosine is a float).
    const AFNI_PI: f64 = 3.141_592_653;
    let f = |acos: &dyn Fn(f64) -> f64, x: f32| (acos(f64::from(x)) * 180.0 / AFNI_PI) as f32;
    let top = 1.0f32.to_bits();
    let (d_lc, d_cg, d_lg, raw_cg) = (0..top + 1).into_par_iter().with_min_len(1 << 16).map(|b| {
        let x = f32::from_bits(b);
        let l = f(&|v| libm::acos(v), x);
        let c = f(&|v| math::acos(v), x);
        let g = f(&|v| v.acos(), x);
        let rc = math::acos(f64::from(x)).to_bits() != f64::from(x).acos().to_bits();
        ((l != c) as u64, (c != g) as u64, (l != g) as u64, rc as u64)
    }).reduce(|| (0, 0, 0, 0), |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2, a.3 + b.3));
    println!("(a) AFNI oblique angle over {} floats in [0,1]: libm vs core {d_lc}, core vs glibc {d_cg}, libm vs glibc {d_lg}; acos's double itself core vs glibc {raw_cg}", top as u64 + 1);
    // (b) nibabel-style obliquity: acos of doubles in [0, 1]
    let mut s: u64 = 12345;
    let mut next = || { s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (s >> 11) as f64 / (1u64 << 53) as f64 };
    let xs: Vec<f64> = (0..20_000_000).map(|_| next()).collect();
    let (b_lc, b_cg, b_lg) = xs.par_iter().map(|&x| { let (l, c, g) = (libm::acos(x), math::acos(x), x.acos()); ((l != c) as u64, (c != g) as u64, (l != g) as u64) }).reduce(|| (0, 0, 0), |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2));
    println!("(b) acos of 2e7 random doubles in [0,1): libm vs core {b_lc}, core vs glibc {b_cg}, libm vs glibc {b_lg}");
    // (c) mcflirt's decomposition: atan2 of float pairs (cosines and sines) and of (double, double)
    let pairs: Vec<(f64, f64)> = (0..20_000_000).map(|_| { let a = (next() * 2.0 - 1.0) as f32; let b = (next() * 2.0 - 1.0) as f32; (f64::from(a), f64::from(b)) }).collect();
    let (c_lc, c_cg, c_lg, c_f32) = pairs.par_iter().map(|&(y, x)| { let (l, c, g) = (libm::atan2(y, x), math::atan2(y, x), y.atan2(x)); ((l != c) as u64, (c != g) as u64, (l != g) as u64, ((l as f32) != (c as f32)) as u64) }).reduce(|| (0, 0, 0, 0), |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2, a.3 + b.3));
    println!("(c) atan2 of 2e7 float pairs in [-1,1]^2: libm vs core {c_lc} (as floats {c_f32}), core vs glibc {c_cg}, libm vs glibc {c_lg}");
    // near-identity rotations: sines tiny, cosines near 1 (the motion of a series)
    let small: Vec<(f64, f64)> = (0..20_000_000).map(|_| { let a = ((next() * 2.0 - 1.0) * 0.05) as f32; let c = (1.0 - f64::from(a) * f64::from(a) / 2.0) as f32; (f64::from(a), f64::from(c)) }).collect();
    let (s_lc, s_cg) = small.par_iter().map(|&(y, x)| { let (l, c, g) = (libm::atan2(y, x), math::atan2(y, x), y.atan2(x)); ((l != c) as u64, (c != g) as u64) }).reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
    println!("    atan2 of 2e7 near-identity float pairs (|sin| < 0.05): libm vs core {s_lc}, core vs glibc {s_cg}");
    // (d) the B-spline prefilter: z^(n-1) for ITK's poles (orders 2 to 5), lines of 2 to 4096
    let poles: Vec<f64> = vec![8f64.sqrt() - 3.0, 3f64.sqrt() - 2.0,
        (664.0 - 438_976f64.sqrt()).sqrt() + 304f64.sqrt() - 19.0, (664.0 + 438_976f64.sqrt()).sqrt() - 304f64.sqrt() - 19.0,
        (135.0 / 2.0 - (17745.0f64 / 4.0).sqrt()).sqrt() + (105.0f64 / 4.0).sqrt() - 13.0 / 2.0,
        (135.0 / 2.0 + (17745.0f64 / 4.0).sqrt()).sqrt() - (105.0f64 / 4.0).sqrt() - 13.0 / 2.0];
    let (mut d_n, mut d_cg2, mut d_lg2, mut used, mut total) = (0, 0, 0, 0, 0);
    for &z in &poles {
        let horizon = (math::log(1e-10) / math::log(z.abs())).ceil() as usize;
        for n in 2..4096usize {
            let y = (n - 1) as f64;
            let (l, c, g) = (libm::pow(z, y), math::pow(z, y), z.powf(y));
            total += 1;
            if horizon >= n { used += 1; if l != c { d_n += 1; println!("      differs: pole {z:.6} n {n}: libm {l:e} core {c:e}"); } if c != g { d_cg2 += 1; } if l != g { d_lg2 += 1; } }
        }
        println!("    pole {z:.17}: horizon {horizon}");
    }
    println!("(d) B-spline pow over {used} (pole, line length) pairs the prefilter uses (of {total}): libm vs core {d_n}, core vs glibc {d_cg2}, libm vs glibc {d_lg2}");
}
