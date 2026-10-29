// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! Does the last bit of glibc's `cbrtf` ever change what 3dUnifize `-EPI` computes or prints?
//!
//! 3dUnifize `-EPI` sets its radius from the voxel volume `x = |dx * dy * dz|` (a float32
//! product): `r = (float)(18.3 / (double)cbrtf(x))`, at least 4.14. AFNI runs glibc 2.35's
//! `cbrtf` (LGPL-2.1-or-later); the main package has CORE-MATH's correctly rounded one (MIT,
//! `larmorx_core::math::cbrtf`), which differs from glibc's on 453,492,162 of the 2^32 inputs.
//! This tool compares, for every non-negative float32 `x` (all 2^31 bit patterns), everything
//! the radius feeds:
//!
//! - the radius itself (as float32 bits);
//! - the message `-EPI changes -Urad to %.1f voxels`;
//! - the ball of neighbours `MCW_spheremask(1, 1, 1, r)`: the offsets `(i, j, k)` with
//!   `|i|, |j|, |k| <= trunc(r)` and `i^2 + j^2 + k^2 <= fl(r * r)` (float sums). While
//!   `3 trunc(r)^2 < 2^24` (`trunc(r) <= 2364`) every sum is an exact integer, so the ball is
//!   fixed by `n = trunc(r)` and `M = floor(fl(r * r))`: two radii give the same ball exactly
//!   when they have the same `n` and no integer `m` in between their `M`s is a sum of three
//!   squares with each square at most `n^2` (Legendre's three-square theorem below `(n + 1)^2`,
//!   where the bound holds by itself; the one value `(n + 1)^2` that `fl(r * r)` can reach is
//!   searched). Radii above 2364 voxels are counted apart: their ball has more than 5.5 * 10^10
//!   offsets (more than 1.5 TB to build), beyond any machine;
//! - `percmean_args_ok` (a radius below 4 is refused; never reached, the radius is at least 4.14).
//!
//! The derivation is cross-checked by building both balls (`unifize::sphere`) and comparing
//! their offsets: in each block of 2^16 inputs, the first case found the same and the first
//! found different, among radii below 30 voxels.
//!
//! Usage: `cbrtf-measure [--threads N] [--list N]`: prints the counts and up to N cases of each
//! kind (default 40). Exit status 0 when no ball and no message changes, 3 otherwise.
#![forbid(unsafe_code)]

use larmorx_core::math::cbrtf as cbrtf_core;
use larmorx_gpl_afni::glibc_floatmath::cbrtf as cbrtf_glibc;
use larmorx_gpl_afni::unifize::sphere;
use rayon::prelude::*;

/// 3dUnifize `-EPI`'s radius from the cube root of the voxel volume (the replica's
/// `cli::unifize`, as AFNI's `main` computes it).
fn radius(c: f32) -> f32 {
    let mut uu = (18.3 / f64::from(c)) as f32;
    if uu < 4.14 {
        uu = 4.14;
    }
    uu
}

/// Below this `trunc(r)`, every float sum of three squares the ball tests is exact.
const EXACT_N: i64 = 2364;

/// `(n, M)`: `trunc(r)` (as `(int)r`, NaN as 0) and `floor(fl(r * r))`.
fn signature(r: f32) -> (i64, i64) {
    let n = i64::from(r as i32);
    let dist_q = r * r;
    let m = if dist_q.is_finite() {
        dist_q.floor() as i64
    } else {
        i64::MAX
    };
    (n, m)
}

/// Legendre: `m >= 0` is a sum of three squares unless it is `4^a (8b + 7)`.
fn three_squares(mut m: i64) -> bool {
    if m == 0 {
        return true;
    }
    while m % 4 == 0 {
        m /= 4;
    }
    m % 8 != 7
}

/// Whether `m` is `i^2 + j^2 + k^2` with `0 <= i, j, k <= n` (searched).
fn three_squares_bounded(m: i64, n: i64) -> bool {
    let mut i = 0;
    while i <= n && i * i <= m {
        let mut j = i;
        while j <= n && i * i + j * j <= m {
            let rest = m - i * i - j * j;
            let k = (rest as f64).sqrt() as i64;
            for kk in [k - 1, k, k + 1] {
                if kk >= 0 && kk <= n && kk * kk == rest {
                    return true;
                }
            }
            j += 1;
        }
        i += 1;
    }
    false
}

/// Whether the balls of radii `a` and `b` (both with `trunc(r) <= EXACT_N`) hold different
/// offsets.
fn balls_differ(a: f32, b: f32) -> bool {
    let (na, ma) = signature(a);
    let (nb, mb) = signature(b);
    if na != nb {
        // r crosses an integer N: (N, 0, 0) is in the larger ball only (when it has one).
        return na.max(nb) >= 1;
    }
    let n = na;
    if n < 1 {
        return false;
    }
    let cap = 3 * n * n;
    let (lo, hi) = (ma.min(mb).min(cap), ma.max(mb).min(cap));
    ((lo + 1)..=hi).any(|m| {
        if m < (n + 1) * (n + 1) {
            three_squares(m)
        } else {
            three_squares_bounded(m, n)
        }
    })
}

fn same_ball_built(a: f32, b: f32) -> bool {
    let mut sa = sphere(a);
    let mut sb = sphere(b);
    sa.sort_unstable();
    sb.sort_unstable();
    sa == sb
}

#[derive(Clone, Default)]
struct Found {
    cbrtf_differ: u64,
    radius_differ: u64,
    radius_differ_beyond: u64,
    message_differ: Vec<u32>,
    ball_differ: Vec<u32>,
    cross_checked: u64,
    cross_check_failed: Vec<u32>,
    /// Whether this block's cross-check of a same ball (0) and of a different ball (1) ran.
    sampled: [bool; 2],
    /// The voxel volumes of the cases whose radius differs, by decade: index `d` counts
    /// volumes in `[10^(d - 46), 10^(d - 45))` mm^3.
    radius_by_decade: Vec<u64>,
    /// The largest distance, in units in the last place, between the two cube roots.
    max_ulps: u32,
    /// Inputs where a cube root one unit in the last place away from CORE-MATH's would change
    /// the ball or the message (`sensitive`): all, from 0.001 mm^3, in [1, 100) mm^3.
    sensitive: [u64; 3],
    /// Inputs where the ball or the message differs but `sensitive` is false (must be none).
    unflagged: Vec<u32>,
}

impl Found {
    fn new() -> Found {
        Found {
            radius_by_decade: vec![0; 90],
            ..Found::default()
        }
    }

    fn merge(mut self, o: Found) -> Found {
        self.cbrtf_differ += o.cbrtf_differ;
        self.radius_differ += o.radius_differ;
        self.radius_differ_beyond += o.radius_differ_beyond;
        self.message_differ.extend(o.message_differ);
        self.ball_differ.extend(o.ball_differ);
        self.cross_checked += o.cross_checked;
        self.cross_check_failed.extend(o.cross_check_failed);
        for (a, b) in self.radius_by_decade.iter_mut().zip(o.radius_by_decade) {
            *a += b;
        }
        self.max_ulps = self.max_ulps.max(o.max_ulps);
        for (a, b) in self.sensitive.iter_mut().zip(o.sensitive) {
            *a += b;
        }
        self.unflagged.extend(o.unflagged);
        self
    }
}

/// Whether radii `a` and `b` give a different ball or message.
fn observable_differ(a: f32, b: f32) -> bool {
    if a.to_bits() == b.to_bits() {
        return false;
    }
    if format!("{:.1}", f64::from(a)) != format!("{:.1}", f64::from(b)) {
        return true;
    }
    // Beyond EXACT_N the float sums round: any other radius counts as another ball.
    signature(a).0.max(signature(b).0) > EXACT_N || balls_differ(a, b)
}

/// Whether a cube root one unit in the last place below or above `c` (CORE-MATH's, finite
/// and positive) would change the ball or the message.
fn sensitive(c: f32) -> bool {
    if !(c.is_finite() && c > 0.0) {
        return false;
    }
    let r = radius(c);
    [c.next_down(), c.next_up()]
        .into_iter()
        .any(|n| observable_differ(radius(n), r))
}

fn check(bits: u32, f: &mut Found) {
    let x = f32::from_bits(bits);
    let (cg, cm) = (cbrtf_glibc(x), cbrtf_core(x));
    let flagged = sensitive(cm);
    if flagged {
        f.sensitive[0] += 1;
        if x >= 1e-3 {
            f.sensitive[1] += 1;
        }
        if (1.0..100.0).contains(&x) {
            f.sensitive[2] += 1;
        }
    }
    if cg.to_bits() == cm.to_bits() || (cg.is_nan() && cm.is_nan()) {
        return;
    }
    f.cbrtf_differ += 1;
    f.max_ulps = f.max_ulps.max(cg.to_bits().abs_diff(cm.to_bits()));
    if !flagged && observable_differ(radius(cg), radius(cm)) {
        f.unflagged.push(bits);
    }
    let (rg, rm) = (radius(cg), radius(cm));
    if rg.to_bits() == rm.to_bits() {
        return;
    }
    f.radius_differ += 1;
    if x > 0.0 && x.is_finite() {
        let decade = (f64::from(x).log10().floor() + 46.0).clamp(0.0, 89.0) as usize;
        f.radius_by_decade[decade] += 1;
    }
    if format!("{:.1}", f64::from(rg)) != format!("{:.1}", f64::from(rm)) {
        f.message_differ.push(bits);
    }
    if signature(rg).0.max(signature(rm).0) > EXACT_N {
        f.radius_differ_beyond += 1;
        return;
    }
    let differ = balls_differ(rg, rm);
    if differ {
        f.ball_differ.push(bits);
    }
    // The cross-check: in each block of 2^16 inputs, the first case of each answer whose
    // radii are below 30 voxels, both balls built and compared.
    let slot = usize::from(differ);
    if rg.max(rm) < 30.0 && !f.sampled[slot] {
        f.sampled[slot] = true;
        f.cross_checked += 1;
        if differ == same_ball_built(rg, rm) {
            f.cross_check_failed.push(bits);
        }
    }
}

fn show(bits: u32) -> String {
    let x = f32::from_bits(bits);
    let (cg, cm) = (cbrtf_glibc(x), cbrtf_core(x));
    let (rg, rm) = (radius(cg), radius(cm));
    let (ng, nm) = (sphere_len(rg), sphere_len(rm));
    format!(
        "x = 0x{bits:08x} ({x:e} mm^3, voxel edge {:.6} mm): cbrtf glibc 0x{:08x} core 0x{:08x}; \
         radius glibc {rg:.9} (0x{:08x}) core {rm:.9} (0x{:08x}); printed {:.1} / {:.1}; \
         (n, floor r^2) {:?} / {:?}; ball offsets {ng} / {nm}",
        f64::from(x).cbrt(),
        cg.to_bits(),
        cm.to_bits(),
        rg.to_bits(),
        rm.to_bits(),
        f64::from(rg),
        f64::from(rm),
        signature(rg),
        signature(rm),
    )
}

/// The number of offsets of the ball (with its centre), counted without building it (for
/// radii up to `EXACT_N`), or "(beyond)".
fn sphere_len(r: f32) -> String {
    let (n, m) = signature(r);
    if n < 1 {
        return "1".into();
    }
    if n > EXACT_N {
        return "(beyond)".into();
    }
    let mut count: u64 = 0;
    for k in -n..=n {
        for j in -n..=n {
            let rest = m - k * k - j * j;
            if rest < 0 {
                continue;
            }
            let mut i = (rest as f64).sqrt() as i64;
            while i * i > rest {
                i -= 1;
            }
            while (i + 1) * (i + 1) <= rest {
                i += 1;
            }
            count += 2 * i.min(n) as u64 + 1;
        }
    }
    count.to_string()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let value = |name: &str, default: usize| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .map_or(default, |v| v.parse::<usize>().expect("a number"))
    };
    let threads = value("--threads", 0);
    let list = value("--list", 40);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("thread pool");
    // Every non-negative float32: 0x00000000 ..= 0x7fffffff (the volume is |dx dy dz|).
    let found = pool.install(|| {
        (0u32..0x8000)
            .into_par_iter()
            .map(|hi| {
                let mut f = Found::new();
                for lo in 0..=u16::MAX as u32 {
                    check((hi << 16) | lo, &mut f);
                }
                f
            })
            .reduce(Found::new, Found::merge)
    });
    let mut message = found.message_differ.clone();
    let mut ball = found.ball_differ.clone();
    // Largest volumes first: the realistic voxels (0.1 mm edges and up) lead the lists.
    message.sort_unstable_by(|a, b| b.cmp(a));
    ball.sort_unstable_by(|a, b| b.cmp(a));
    println!("inputs: every non-negative float32 voxel volume (2^31 bit patterns)");
    println!(
        "cbrtf differs (glibc 2.35 vs CORE-MATH): {}",
        found.cbrtf_differ
    );
    println!("the radius differs: {}", found.radius_differ);
    println!(
        "  of which with a radius above {EXACT_N} voxels (a ball beyond any machine): {}",
        found.radius_differ_beyond
    );
    let by_decade = |cases: &[u32]| {
        let mut d = vec![0u64; 90];
        for &b in cases {
            let x = f64::from(f32::from_bits(b));
            if x > 0.0 && x.is_finite() {
                d[(x.log10().floor() + 46.0).clamp(0.0, 89.0) as usize] += 1;
            }
        }
        d
    };
    let (msg_d, ball_d) = (by_decade(&message), by_decade(&ball));
    println!("by voxel volume (decades): radius differs / message differs / ball differs");
    for d in 0..90 {
        let r = found.radius_by_decade[d];
        if r > 0 || msg_d[d] > 0 || ball_d[d] > 0 {
            println!(
                "  [1e{}, 1e{}) mm^3: {r} / {} / {}",
                d as i32 - 46,
                d as i32 - 45,
                msg_d[d],
                ball_d[d]
            );
        }
    }
    // MRI voxels: edges from 0.1 mm (1e-3 mm^3) up; the radius is 4.14 (both) beyond ~86 mm^3.
    let mri = |cases: &[u32]| {
        cases
            .iter()
            .filter(|&&b| f32::from_bits(b) >= 1e-3)
            .count()
    };
    println!(
        "voxel volumes of at least 0.001 mm^3 (edges of 0.1 mm and up): radius differs {}, \
         message differs {}, ball differs {}",
        found.radius_by_decade[43..].iter().sum::<u64>(),
        mri(&message),
        mri(&ball)
    );
    println!(
        "the message '-EPI changes -Urad to %.1f voxels' differs: {} (largest volumes first)",
        message.len()
    );
    for &b in message.iter().take(list) {
        println!("  {}", show(b));
    }
    println!(
        "the ball of neighbours differs (radius up to {EXACT_N}): {} (largest volumes first)",
        ball.len()
    );
    for &b in ball.iter().take(list) {
        println!("  {}", show(b));
    }
    println!(
        "cross-check (both balls built, radius below 30, sampled): {} cases, {} disagree with the derivation",
        found.cross_checked,
        found.cross_check_failed.len()
    );
    for &b in found.cross_check_failed.iter().take(list) {
        println!("  {}", show(b));
    }
    println!(
        "the two cube roots differ by at most {} unit(s) in the last place",
        found.max_ulps
    );
    println!(
        "inputs where one unit in the last place of the cube root changes the ball or the \
         message (a superset of the differences, found without glibc): {} (from 0.001 mm^3: {}; \
         in [1, 100) mm^3: {})",
        found.sensitive[0], found.sensitive[1], found.sensitive[2]
    );
    println!(
        "differences outside that set: {} (must be 0)",
        found.unflagged.len()
    );
    for &b in found.unflagged.iter().take(list) {
        println!("  {}", show(b));
    }
    let none = message.is_empty() && ball.is_empty() && found.cross_check_failed.is_empty();
    println!(
        "{}",
        if none {
            "NOTHING 3dUnifize COMPUTES OR PRINTS DEPENDS ON THE LAST BIT"
        } else {
            "THE LAST BIT CAN CHANGE 3dUnifize -EPI"
        }
    );
    std::process::exit(if none { 0 } else { 3 });
}
