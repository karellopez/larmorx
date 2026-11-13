// SPDX-License-Identifier: Apache-2.0
//! Family `svdnan`: LINPACK's float SVD as `vnl_svd<float>` calls it (`ssvdc`, job 21; ITK 4.13.2's
//! netlib as the unit oracle built it) on NaNs and other special entries, every value compared bit
//! for bit, NaNs included (their signs and payloads come from the operand order of each `+` and `*`,
//! which GCC chooses per statement and per vectorized position).
//!
//! 1. `srot` (unit strides; the two vectors are columns of one array, the second after or before
//!    the first, as LINPACK passes them) on NaNs with distinct payloads: for every length to 300,
//!    every subset of `c`, `s`, `x`, `y` NaN, the NaN each element ends with; then on random
//!    vectors of special values.
//! 2. `srotg` on pairs of special values: `r`, `z`, `c`, `s`.
//! 3. `ssvdc` of matrices of 1 to 7 columns and up to three more rows, with special entries
//!    (NaNs of both signs with random payloads, quiet and signalling, infinities, signed zeros,
//!    overflowing and underflowing magnitudes) and finite ones: `info`, `U`, `W`, `V`. VNL runs in
//!    forked children with a time limit (a batch at a time, then case by case where a batch did
//!    not finish); where the port says VNL never returns (ITK-116) the case is counted, not run
//!    (the family `nonfinite` confirms those hangs).

use std::ffi::{c_int, c_long};

use larmorx_core::vnl_fixed::cases;
use larmorx_core::vnl_svd_f32;
use larmorx_core::x87::cases::SplitMix64;

use crate::checks_nonfinite::{Guarded, guarded};
use crate::{Lib, Tally};

fn nan(tag: u32) -> f32 {
    f32::from_bits(0x7fc0_0000 | tag)
}

fn srot_lib(lib: &Lib, x: &mut [f32], y: &mut [f32], c: f32, s: f32) {
    let (n, one) = (x.len() as c_long, 1 as c_long);
    // SAFETY: n elements at x and at y, unit strides (netlib's interface).
    unsafe { (lib.k_srot)(&n, x.as_mut_ptr(), &one, y.as_mut_ptr(), &one, &c, &s) };
}

/// Two columns of one array: `x` at `base`, `y` `d` elements after or before it.
struct Pair {
    buf: Vec<f32>,
    xo: usize,
    yo: usize,
    n: usize,
}

impl Pair {
    fn new(n: usize, d: usize, after: bool) -> Pair {
        let base = 8 + d;
        let (xo, yo) = if after {
            (base, base + d)
        } else {
            (base, base - d)
        };
        Pair {
            buf: vec![0.0; base + d + n + 8],
            xo,
            yo,
            n,
        }
    }

    fn set(&mut self, x: &[f32], y: &[f32]) {
        self.buf[self.xo..self.xo + self.n].copy_from_slice(x);
        self.buf[self.yo..self.yo + self.n].copy_from_slice(y);
    }

    fn get(&self) -> (Vec<f32>, Vec<f32>) {
        (
            self.buf[self.xo..self.xo + self.n].to_vec(),
            self.buf[self.yo..self.yo + self.n].to_vec(),
        )
    }

    /// Calls the library's `srot` on the two columns in place.
    fn rot(&mut self, lib: &Lib, c: f32, s: f32) {
        let (n, one) = (self.n as c_long, 1 as c_long);
        let p = self.buf.as_mut_ptr();
        // SAFETY: both ranges lie inside `buf` and do not overlap (|d| >= n).
        unsafe { (lib.k_srot)(&n, p.add(self.xo), &one, p.add(self.yo), &one, &c, &s) };
    }
}

fn cmp(t: &mut Tally, what: &dyn Fn() -> String, ours: &[f32], theirs: &[f32]) {
    for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
        t.bits(
            &|| format!("{} [{i}]", what()),
            u64::from(a.to_bits()),
            u64::from(b.to_bits()),
        );
    }
    if ours.len() != theirs.len() {
        t.bits(
            &|| format!("{} length", what()),
            ours.len() as u64,
            theirs.len() as u64,
        );
    }
}

/// Part 1: `srot`'s NaN orders for every length, then random special vectors.
fn rotations(lib: &Lib, scale: u64, rng: &mut SplitMix64, t: &mut Tally) {
    for n in 1..=300usize {
        let (d, after) = (n + (n % 7), n % 2 == 0);
        for sub in 1u32..16 {
            t.cases += 1;
            let c = if sub & 1 != 0 { nan(200_000) } else { 2.0 };
            let s = if sub & 2 != 0 { nan(300_000) } else { 3.0 };
            let x: Vec<f32> = (0..n)
                .map(|i| if sub & 4 != 0 { nan(1 + i as u32) } else { 5.0 })
                .collect();
            let y: Vec<f32> = (0..n)
                .map(|i| {
                    if sub & 8 != 0 {
                        nan(1000 + i as u32)
                    } else {
                        7.0
                    }
                })
                .collect();
            let mut p = Pair::new(n, d, after);
            p.set(&x, &y);
            p.rot(lib, c, s);
            let (tx, ty) = p.get();
            let (mut ox, mut oy) = (x.clone(), y.clone());
            vnl_svd_f32::srot(&mut ox, &mut oy, c, s);
            cmp(
                t,
                &|| format!("srot order n={n} subset {sub:#06b} x"),
                &ox,
                &tx,
            );
            cmp(
                t,
                &|| format!("srot order n={n} subset {sub:#06b} y"),
                &oy,
                &ty,
            );
        }
    }
    for k in 0..(4_000 * scale) {
        t.cases += 1;
        let n = if k < 300 {
            1 + k as usize
        } else {
            1 + rng.below(40) as usize
        };
        let d = n + rng.below(9) as usize;
        let after = rng.below(2) == 0;
        let pick = |rng: &mut SplitMix64| {
            if rng.below(3) == 0 {
                cases::special_f32(rng)
            } else {
                rng.range(-1000, 1000) as f32 / 16.0
            }
        };
        let (c, s) = (pick(rng), pick(rng));
        let x: Vec<f32> = (0..n).map(|_| pick(rng)).collect();
        let y: Vec<f32> = (0..n).map(|_| pick(rng)).collect();
        let mut p = Pair::new(n, d, after);
        p.set(&x, &y);
        p.rot(lib, c, s);
        let (tx, ty) = p.get();
        let (mut ox, mut oy) = (x.clone(), y.clone());
        vnl_svd_f32::srot(&mut ox, &mut oy, c, s);
        let w = |v: &str| {
            format!(
                "srot n={n} d={d} after={after} c={:#x} s={:#x} {v}",
                c.to_bits(),
                s.to_bits()
            )
        };
        cmp(t, &|| w("x"), &ox, &tx);
        cmp(t, &|| w("y"), &oy, &ty);
        // the unseparated form (two arrays) too
        let (mut lx, mut ly) = (x.clone(), y.clone());
        srot_lib(lib, &mut lx, &mut ly, c, s);
        cmp(t, &|| w("x (two arrays)"), &ox, &lx);
        cmp(t, &|| w("y (two arrays)"), &oy, &ly);
    }
}

/// Part 2: `srotg`.
fn givens(lib: &Lib, scale: u64, rng: &mut SplitMix64, t: &mut Tally) {
    let fixed = [
        (nan(5), nan(7)),
        (-nan(5), nan(7)),
        (nan(5), -2.0),
        (-3.0, nan(7)),
        (f32::INFINITY, f32::INFINITY),
        (f32::NEG_INFINITY, 1.0),
        (0.0, -0.0),
        (-0.0, f32::NAN),
    ];
    for k in 0..(40_000 * scale) as usize {
        t.cases += 1;
        let (a, b) = if k < fixed.len() {
            fixed[k]
        } else {
            let pick = |rng: &mut SplitMix64| {
                if rng.below(2) == 0 {
                    cases::special_f32(rng)
                } else {
                    rng.range(-1000, 1000) as f32 / 8.0
                }
            };
            (pick(rng), pick(rng))
        };
        let (mut sa, mut sb, mut c, mut s) = (a, b, 0.0f32, 0.0f32);
        // SAFETY: four floats (netlib's interface).
        unsafe { (lib.k_srotg)(&mut sa, &mut sb, &mut c, &mut s) };
        let (r, z, oc, os) = vnl_svd_f32::srotg(a, b);
        cmp(
            t,
            &|| format!("srotg({:#x}, {:#x}) r z c s", a.to_bits(), b.to_bits()),
            &[r, z, oc, os],
            &[sa, sb, c, s],
        );
    }
}

/// A `r x c` matrix (row-major) with special entries (or none).
fn matrix(rng: &mut SplitMix64, r: usize, c: usize) -> Vec<f32> {
    let rc = r * c;
    let mut m: Vec<f32> = (0..rc)
        .map(|_| rng.range(-4096, 4096) as f32 / 256.0)
        .collect();
    let count = match rng.below(9) {
        0 => 0,
        1..=3 => 1,
        4 => 2.min(rc),
        5 => 3.min(rc),
        6 => c,
        7 => rc / 2,
        _ => rc,
    };
    for _ in 0..count {
        let i = rng.below(rc as u64) as usize;
        m[i] = cases::special_f32(rng);
    }
    m
}

/// `ssvdc` through the shim: info, U, W, V (U and V row by row).
fn lib_ssvdc(lib: &Lib, r: usize, c: usize, a: &[f32]) -> Vec<f32> {
    let (mut uu, mut ww, mut vv) = (vec![0.0f32; r * c], vec![0.0f32; c], vec![0.0f32; c * c]);
    // SAFETY: r x c in, the shim's output sizes.
    let info: c_long = unsafe {
        (lib.ssvdc_f32)(
            r as c_int,
            c as c_int,
            a.as_ptr(),
            uu.as_mut_ptr(),
            ww.as_mut_ptr(),
            vv.as_mut_ptr(),
        )
    };
    let mut all = vec![f32::from_bits(info as u32)];
    all.extend_from_slice(&uu);
    all.extend_from_slice(&ww);
    all.extend_from_slice(&vv);
    all
}

fn to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn from_bytes(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

/// Part 3: `ssvdc`.
fn decompositions(lib: &Lib, scale: u64, rng: &mut SplitMix64, t: &mut Tally) {
    const BATCH: usize = 200;
    const BATCH_MS: i32 = 60_000;
    let total = 20_000 * scale as usize;
    let (mut never, mut compared, mut nonzero_info, mut alone) = (0u64, 0u64, 0u64, 0u64);
    let mut done = 0;
    while done < total {
        let mut calls = Vec::new();
        while calls.len() < BATCH && done < total {
            done += 1;
            t.cases += 1;
            let c = 1 + rng.below(7) as usize;
            let r = c + rng.below(4) as usize;
            let a = matrix(rng, r, c);
            let port = vnl_svd_f32::vnl_svd_f32_full(&a, r, c);
            if port.never_returns() {
                never += 1;
                continue;
            }
            calls.push((r, c, a, port));
        }
        let run_all = || {
            let mut out = Vec::new();
            for (r, c, a, _) in &calls {
                out.extend(to_bytes(&lib_ssvdc(lib, *r, *c, a)));
            }
            out
        };
        let theirs: Vec<Option<Vec<f32>>> = match guarded(BATCH_MS, run_all) {
            Guarded::Done(b) => {
                let all = from_bytes(&b);
                let mut at = 0;
                calls
                    .iter()
                    .map(|(r, c, _, _)| {
                        let len = 1 + r * c + c + c * c;
                        let v = all.get(at..at + len).map(<[f32]>::to_vec);
                        at += len;
                        v
                    })
                    .collect()
            }
            Guarded::Spinning(_) => calls
                .iter()
                .map(|(r, c, a, _)| {
                    alone += 1;
                    match guarded(20_000, || to_bytes(&lib_ssvdc(lib, *r, *c, a))) {
                        Guarded::Done(b) => Some(from_bytes(&b)),
                        Guarded::Spinning(_) => None,
                    }
                })
                .collect(),
        };
        for ((r, c, a, port), th) in calls.iter().zip(theirs) {
            let w = |s: &str| {
                format!(
                    "ssvdc {r}x{c} {s} {:?}",
                    a.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                )
            };
            let Some(th) = th else {
                t.values += 1;
                t.differ += 1;
                if t.shown < 12 {
                    t.shown += 1;
                    println!(
                        "  DIFF [{}] {}: the port returns, VNL did not",
                        t.name,
                        w("")
                    );
                }
                continue;
            };
            compared += 1;
            if port.info != 0 {
                nonzero_info += 1;
            }
            t.bits(&|| w("info"), port.info as u64, u64::from(th[0].to_bits()));
            let (tu, rest) = th[1..].split_at(r * c);
            let (tw, tv) = rest.split_at(*c);
            // the shim copies U and V out row by row
            let rows = |cm: &[f32], nr: usize, nc: usize| -> Vec<f32> {
                (0..nr * nc).map(|k| cm[k / nc + nr * (k % nc)]).collect()
            };
            cmp(t, &|| w("U"), &rows(&port.u, *r, *c), tu);
            cmp(t, &|| w("W"), &port.w, tw);
            cmp(t, &|| w("V"), &rows(&port.v, *c, *c), tv);
        }
    }
    t.notes.push(format!(
        "ssvdc: {compared} decompositions compared ({nonzero_info} with info not 0), {never} the port says VNL never returns (not run), {alone} run alone after a batch did not finish"
    ));
}

pub fn run(lib: &Lib, scale: u64, t: &mut Tally) {
    let mut rng = SplitMix64::new(0x5fd_0a7a);
    rotations(lib, scale, &mut rng, t);
    givens(lib, scale, &mut rng, t);
    decompositions(lib, scale, &mut rng, t);
}
