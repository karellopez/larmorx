// SPDX-License-Identifier: Apache-2.0
//! Family `eignan`: `vnl_real_eigensystem` (EISPACK's `rg` with eigenvectors: `balanc`, `elmhes`,
//! `eltran`, `hqr2`, `balbak`; ITK 4.13.2's netlib as the unit oracle built it) on matrices with NaNs
//! and other special entries, every value compared bit for bit, NaNs included (their signs and
//! payloads come from the operand order of each `+` and `*`, which GCC chooses per statement and
//! per vectorized position).
//!
//! `balanc` never returns on an infinite or NaN entry off the diagonal of the part it balances
//! (ITK-112), so the matrices are built with that part finite but for its diagonal: a random
//! `low..igh` block of ordinary numbers (any value on its diagonal: `balanc`'s sums leave it out), the
//! rows above it and the columns right of it (where EISPACK's isolated eigenvalues live) with
//! special entries, zeros where `balanc` needs them, then rows and columns permuted alike so that
//! `balanc` has to find the structure. A share of dense matrices with a few special entries is
//! added; where the port says `balanc` never returns the case is counted, not run. VNL runs in
//! forked children with a time limit (a batch at a time, then case by case). Compared: `D`, `V`
//! (real and imaginary parts) and the columns of `Vreal` VNL writes, where `hqr2` succeeded; the
//! failure index otherwise (the eigenvalues it had not found are memory contents, ITK-111).

use std::ffi::c_int;

use larmorx_core::vnl_eigen;
use larmorx_core::vnl_fixed::cases;
use larmorx_core::x87::cases::SplitMix64;

use crate::checks_nonfinite::{Guarded, guarded};
use crate::{Lib, Tally};

fn ordinary(rng: &mut SplitMix64) -> f64 {
    if rng.below(5) == 0 {
        0.0
    } else {
        rng.range(-4096, 4096) as f64 / 256.0
    }
}

/// An `n x n` matrix (row-major) whose balanced part is finite (see the module's comment).
pub fn structured(rng: &mut SplitMix64, n: usize) -> Vec<f64> {
    let low = 1 + rng.below(n as u64) as usize;
    let igh = low + rng.below((n - low + 1) as u64) as usize;
    let share = 1 + rng.below(4);
    let free = |rng: &mut SplitMix64| {
        if rng.below(4) < share {
            cases::special_f64(rng)
        } else {
            ordinary(rng)
        }
    };
    let mut a = vec![0.0f64; n * n];
    for i in 1..=n {
        for j in 1..=n {
            let in_block = |k: usize| k >= low && k <= igh;
            let v = if in_block(i) && in_block(j) {
                // the diagonal may hold any value: balanc's sums leave it out
                if i == j && rng.below(4) == 0 { free(rng) } else { ordinary(rng) }
            } else if (i < low || i > igh) && j >= i {
                free(rng)
            } else if in_block(i) && j > igh {
                free(rng)
            } else {
                0.0
            };
            a[(i - 1) * n + (j - 1)] = v;
        }
    }
    // a random symmetric permutation
    let mut p: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        let j = rng.below((i + 1) as u64) as usize;
        p.swap(i, j);
    }
    let mut b = vec![0.0f64; n * n];
    for i in 0..n {
        for j in 0..n {
            b[p[i] * n + p[j]] = a[i * n + j];
        }
    }
    b
}

/// A dense matrix with a few special entries.
fn dense(rng: &mut SplitMix64, n: usize) -> Vec<f64> {
    let mut m: Vec<f64> = (0..n * n).map(|_| ordinary(rng)).collect();
    for _ in 0..1 + rng.below(3) {
        let i = rng.below((n * n) as u64) as usize;
        m[i] = cases::special_f64(rng);
    }
    m
}

/// `vnl_real_eigensystem` through the shim: D re, D im, V re, V im, Vreal.
fn lib_realeig(lib: &Lib, n: usize, m: &[f64]) -> Vec<f64> {
    let nn = n * n;
    let (mut vreal, mut vre, mut vim, mut dre, mut dim) =
        (vec![0.0f64; nn], vec![0.0f64; nn], vec![0.0f64; nn], vec![0.0f64; n], vec![0.0f64; n]);
    // SAFETY: n x n in, the shim's output sizes.
    unsafe {
        (lib.realeig)(
            n as c_int,
            m.as_ptr(),
            vreal.as_mut_ptr(),
            vre.as_mut_ptr(),
            vim.as_mut_ptr(),
            dre.as_mut_ptr(),
            dim.as_mut_ptr(),
        )
    };
    let mut all = dre;
    all.extend(dim);
    all.extend(vre);
    all.extend(vim);
    all.extend(vreal);
    all
}

fn to_bytes(v: &[f64]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn from_bytes(b: &[u8]) -> Vec<f64> {
    b.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect()
}

/// Values that differ other than as two NaNs (a number, an infinity or a zero on either side).
static NOT_NAN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn cmp(t: &mut Tally, what: &dyn Fn() -> String, ours: &[f64], theirs: &[f64]) {
    for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
        if !t.bits(&|| format!("{} [{i}]", what()), a.to_bits(), b.to_bits()) && !(a.is_nan() && b.is_nan()) {
            NOT_NAN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// The cases: `(n, matrix)` for every one the port says returns, and how many it says never do.
pub fn cases(rng: &mut SplitMix64, count: usize) -> (Vec<(usize, Vec<f64>)>, u64) {
    let mut out = Vec::new();
    let mut never = 0;
    for k in 0..count {
        // now and then more than 16 rows: the loops GCC vectorizes only then
        let n = if k % 50 == 49 { 17 + rng.below(4) as usize } else { 1 + rng.below(8) as usize };
        let m = if k % 5 == 4 { dense(rng, n) } else { structured(rng, n) };
        if vnl_eigen::real_eigensystem(&m, n).is_err() {
            never += 1;
        } else {
            out.push((n, m));
        }
    }
    (out, never)
}

/// VNL's results for `calls` (`None` where it did not return, empty where it crashed), batch by
/// batch in forked children.
pub fn official(lib: &Lib, calls: &[(usize, Vec<f64>)]) -> Vec<Option<Vec<f64>>> {
    // VNL writes past its arrays on ITK-117's matrices: each runs in a child of its own
    let lone = |n: usize, m: &[f64]| {
        vnl_eigen::real_eigensystem(m, n).is_ok_and(|es| es.notices.iter().any(|x| x.defect.id == "ITK-117"))
    };
    let mut slots: Vec<Option<Option<Vec<f64>>>> = vec![None; calls.len()];
    let mut grouped = Vec::new();
    for (k, (n, m)) in calls.iter().enumerate() {
        if lone(*n, m) {
            let len = 2 * n + 3 * n * n;
            slots[k] = Some(match guarded(20_000, || to_bytes(&lib_realeig(lib, *n, m))) {
                Guarded::Done(b) if b.len() == 8 * len => Some(from_bytes(&b)),
                Guarded::Done(_) => Some(Vec::new()),
                Guarded::Spinning(_) => None,
            });
        } else {
            grouped.push(k);
        }
    }
    let rest: Vec<(usize, Vec<f64>)> = grouped.iter().map(|&k| calls[k].clone()).collect();
    for (k, r) in grouped.iter().zip(batches(lib, &rest)) {
        slots[*k] = Some(r);
    }
    slots.into_iter().map(|s| s.expect("every case run")).collect()
}

fn batches(lib: &Lib, calls: &[(usize, Vec<f64>)]) -> Vec<Option<Vec<f64>>> {
    let mut all = Vec::new();
    for batch in calls.chunks(200) {
        let run = || {
            let mut out = Vec::new();
            for (n, m) in batch {
                out.extend(to_bytes(&lib_realeig(lib, *n, m)));
            }
            out
        };
        // a case that makes VNL crash (it writes past its arrays: ITK-117) ends the batch's child:
        // the cases from there on run alone
        let alone = |all: &mut Vec<Option<Vec<f64>>>, rest: &[(usize, Vec<f64>)]| {
            for (n, m) in rest {
                let len = 2 * n + 3 * n * n;
                all.push(match guarded(20_000, || to_bytes(&lib_realeig(lib, *n, m))) {
                    Guarded::Done(b) if b.len() == 8 * len => Some(from_bytes(&b)),
                    Guarded::Done(_) => Some(Vec::new()),
                    Guarded::Spinning(_) => None,
                });
            }
        };
        match guarded(60_000, run) {
            Guarded::Done(b) => {
                let v = from_bytes(&b);
                let mut at = 0;
                for (k, (n, _)) in batch.iter().enumerate() {
                    let len = 2 * n + 3 * n * n;
                    match v.get(at..at + len) {
                        Some(x) => all.push(Some(x.to_vec())),
                        None => {
                            alone(&mut all, &batch[k..]);
                            break;
                        }
                    }
                    at += len;
                }
            }
            Guarded::Spinning(_) => alone(&mut all, batch),
        }
    }
    all
}

/// Compares one case; returns whether every value was identical.
pub fn compare(t: &mut Tally, n: usize, m: &[f64], th: &[f64]) -> bool {
    let before = t.differ;
    let es = vnl_eigen::real_eigensystem(m, n).expect("checked");
    let w = |s: &str| format!("realeig n={n} {s} {:x?}", m.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    let nn = n * n;
    let (dre, rest) = th.split_at(n);
    let (dim, rest) = rest.split_at(n);
    let (vre, rest) = rest.split_at(nn);
    let (vim, vreal) = rest.split_at(nn);
    if es.ierr == 0 {
        cmp(t, &|| w("D re"), &es.d_re, dre);
        cmp(t, &|| w("D im"), &es.d_im, dim);
        cmp(t, &|| w("V re"), &es.v_re, vre);
        // ITK-117: the last column's imaginary parts are memory contents in VNL
        let past = es.notices.iter().any(|x| x.defect.id == "ITK-117");
        let cols = if past { n - 1 } else { n };
        for r in 0..n {
            cmp(t, &|| w(&format!("V im row {r}")), &es.v_im[r * n..r * n + cols], &vim[r * n..r * n + cols]);
        }
        for col in 0..n {
            if es.vreal_written[col] {
                let ours: Vec<f64> = (0..n).map(|r| es.vreal[r * n + col]).collect();
                let theirs: Vec<f64> = (0..n).map(|r| vreal[r * n + col]).collect();
                cmp(t, &|| w(&format!("Vreal column {col}")), &ours, &theirs);
            }
        }
    }
    t.differ == before
}

pub fn run(lib: &Lib, scale: u64, t: &mut Tally) {
    let mut rng = SplitMix64::new(0xe16e_0a7a);
    let (calls, never) = cases(&mut rng, 20_000 * scale as usize);
    t.cases += calls.len() as u64 + never;
    let theirs = official(lib, &calls);
    let (mut failed, mut same, mut crashed) = (0u64, 0u64, 0u64);
    for ((n, m), th) in calls.iter().zip(theirs) {
        if th.as_ref().is_some_and(Vec::is_empty) {
            crashed += 1;
            continue;
        }
        let Some(th) = th else {
            t.values += 1;
            t.differ += 1;
            if t.shown < 12 {
                t.shown += 1;
                println!("  DIFF [{}] realeig n={n} {m:?}: the port returns, VNL did not", t.name);
            }
            continue;
        };
        if vnl_eigen::real_eigensystem(m, *n).expect("checked").ierr != 0 {
            failed += 1;
        }
        if compare(t, *n, m, &th) {
            same += 1;
        }
    }
    t.notes.push(format!(
        "real eigensystems: {} compared ({same} identical, {failed} where hqr2 failed: D and V not compared), {crashed} where VNL crashed (not compared), {never} where the port says balanc never returns (not run); values differing other than as two NaNs: {}",
        calls.len() - crashed as usize,
        NOT_NAN.load(std::sync::atomic::Ordering::Relaxed)
    ));
}
