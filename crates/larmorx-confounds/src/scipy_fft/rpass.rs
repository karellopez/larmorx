// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! The forward real passes of SciPy's single-precision FFTs for power-of-two lengths (the
//! halfcomplex result of FFTPACK: `r0, r1, i1, r2, i2, ...`), and the two libraries' plans:
//! pocketfft's `rfftp` (SciPy 1.15, `pocketfft_hdronly.h`: `radf2`, `radf4`, `exec` with `r2hc`,
//! `factorize`, `comp_twiddle`) and ducc0's `rfftpass` tree (SciPy 1.18.1,
//! `ducc0/fft/fft1d_impl.h`: `rfftp1`, `rfftp2`, `rfftp4`, `rfft_multipass`,
//! `rfftp_complexify`, `rfftpass::make_pass`; `ducc0/fft/fft.h`: `rfftpass::factorize`).
//!
//! SciPy's `fftpack.fft` of a real array runs the forward real transform in both directions
//! (the backward one negates the imaginary parts afterwards), so only the forward passes are
//! needed. pocketfft runs the passes of the factors (fours, a two moved to the front) from the
//! last to the first. ducc0 does the same up to length 1000; above (an even length that its
//! vectorizing plan, `n % 8 == 0`, or its size, `n > 10000`, sends there) it computes the real
//! transform as a complex transform of half the length on the pairs of values, then separates
//! the even and odd parts with the twiddles of the full length (`rfftp_complexify`). Every
//! operation is SSE's single-precision arithmetic (`larmorx_core::numpy::NpFloat`). The notice
//! of pocketfft and ducc0 is in `roots.rs`.

use super::cpass::{self, CPass};
use super::roots::UnityRoots;
use super::{C32, add, cadd, mul, sub};
use std::sync::Arc;

/// `1/sqrt(2)` as `float` (`Tfs(0.707106781186547524400844362104849L)`).
pub(crate) const HSQT2: f32 = f32::from_bits(0x3f35_04f3);

/// One forward pass of a real plan (`rfftpass`).
#[derive(Clone, Debug)]
pub(crate) enum RPass {
    /// Length one (`rfftp1`).
    One,
    /// `radf2` or `radf4` of `l1` blocks of `ido` values; `wa[i + x (ido - 1)]` (`WA(x, i)`).
    Radix {
        ip: usize,
        l1: usize,
        ido: usize,
        wa: Vec<f32>,
    },
    /// The factor passes of the whole length, run from the last to the first
    /// (`rfft_multipass` with `l1 = ido = 1`; pocketfft's `rfftp::exec`).
    Multi { passes: Vec<RPass> },
    /// ducc0's `rfftp_complexify` of length `n`: the complex transform of the `n / 2` pairs, then
    /// the split with `roots` (the table of `n`).
    Complexify {
        n: usize,
        roots: Arc<UnityRoots>,
        pass: CPass,
    },
}

/// `rfftpass::factorize` (and pocketfft's `rfftp::factorize`) of a power of two: fours, then a
/// two moved to the front.
fn factorize(mut n: usize) -> Vec<usize> {
    let mut f = Vec::new();
    while n.is_multiple_of(4) {
        f.push(4);
        n >>= 2;
    }
    if n.is_multiple_of(2) {
        n >>= 1;
        f.push(2);
        let last = f.len() - 1;
        f.swap(0, last);
    }
    debug_assert_eq!(n, 1, "powers of two only");
    f
}

fn radix(ip: usize, l1: usize, ido: usize, roots: &UnityRoots) -> RPass {
    let n = ip * l1 * ido;
    let rfct = roots.len() / n;
    assert_eq!(rfct * n, roots.len(), "mismatch");
    let mut wa = vec![0.0f32; (ip - 1) * ido.saturating_sub(1)];
    for j in 1..ip {
        for i in 1..=(ido.saturating_sub(1)) / 2 {
            let v = roots.get(rfct * j * l1 * i);
            wa[(j - 1) * (ido - 1) + 2 * i - 2] = v.re;
            wa[(j - 1) * (ido - 1) + 2 * i - 1] = v.im;
        }
    }
    RPass::Radix { ip, l1, ido, wa }
}

fn factor_passes(n: usize, roots: &UnityRoots) -> RPass {
    let mut passes = Vec::new();
    let mut l1 = 1;
    for ip in factorize(n) {
        passes.push(radix(ip, l1, n / (l1 * ip), roots));
        l1 *= ip;
    }
    if passes.len() == 1 {
        return passes.pop().expect("one pass");
    }
    RPass::Multi { passes }
}

/// pocketfft's real plan of a power of two `n` (`rfftp`).
pub(crate) fn pocketfft_plan(n: usize) -> RPass {
    if n == 1 {
        return RPass::One;
    }
    factor_passes(n, &UnityRoots::new(n))
}

/// ducc0's real plan of a power of two `n` (`pocketfft_r(n, vectorize = true)`:
/// `rfftpass::make_pass(1, 1, n, roots, true)`).
pub(crate) fn ducc0_plan(n: usize) -> RPass {
    if n == 1 {
        return RPass::One;
    }
    let roots = Arc::new(UnityRoots::new(n));
    // An even length above 1000 goes through the complex transform when vectorizing and n % 8
    // == 0, or when n > 10000 (a power of two has no factor above 5).
    if n > 1000 && n.is_multiple_of(2) && (n.is_multiple_of(8) || n > 10_000) {
        return RPass::Complexify {
            n,
            pass: cpass::ducc0_pass(1, 1, n / 2, &Arc::new(UnityRoots::new(n / 2)), true),
            roots,
        };
    }
    factor_passes(n, &roots)
}

/// `(a + i b) = conj(c + i d) (e + i f)`: `MULPM`.
#[inline(always)]
fn mulpm(c: f32, d: f32, e: f32, f: f32) -> (f32, f32) {
    (add(mul(c, e), mul(d, f)), sub(mul(c, f), mul(d, e)))
}

#[inline(always)]
fn pmf(c: f32, d: f32) -> (f32, f32) {
    (add(c, d), sub(c, d))
}

fn radf2(ido: usize, l1: usize, cc: &[f32], ch: &mut [f32], wa: &[f32]) {
    let w = |x: usize, i: usize| wa[i + x * (ido - 1)];
    let cci = |a: usize, b: usize, c: usize| a + ido * (b + l1 * c);
    let chi = |a: usize, b: usize, c: usize| a + ido * (b + 2 * c);
    for k in 0..l1 {
        (ch[chi(0, 0, k)], ch[chi(ido - 1, 1, k)]) = pmf(cc[cci(0, k, 0)], cc[cci(0, k, 1)]);
    }
    if ido & 1 == 0 {
        for k in 0..l1 {
            ch[chi(0, 1, k)] = -cc[cci(ido - 1, k, 1)];
            ch[chi(ido - 1, 0, k)] = cc[cci(ido - 1, k, 0)];
        }
    }
    if ido <= 2 {
        return;
    }
    for k in 0..l1 {
        for i in (2..ido).step_by(2) {
            let ic = ido - i;
            let (tr2, ti2) = mulpm(
                w(0, i - 2),
                w(0, i - 1),
                cc[cci(i - 1, k, 1)],
                cc[cci(i, k, 1)],
            );
            (ch[chi(i - 1, 0, k)], ch[chi(ic - 1, 1, k)]) = pmf(cc[cci(i - 1, k, 0)], tr2);
            (ch[chi(i, 0, k)], ch[chi(ic, 1, k)]) = pmf(ti2, cc[cci(i, k, 0)]);
        }
    }
}

fn radf4(ido: usize, l1: usize, cc: &[f32], ch: &mut [f32], wa: &[f32]) {
    let w = |x: usize, i: usize| wa[i + x * (ido - 1)];
    let cci = |a: usize, b: usize, c: usize| a + ido * (b + l1 * c);
    let chi = |a: usize, b: usize, c: usize| a + ido * (b + 4 * c);
    for k in 0..l1 {
        let (tr1, t) = pmf(cc[cci(0, k, 3)], cc[cci(0, k, 1)]);
        ch[chi(0, 2, k)] = t;
        let (tr2, t) = pmf(cc[cci(0, k, 0)], cc[cci(0, k, 2)]);
        ch[chi(ido - 1, 1, k)] = t;
        (ch[chi(0, 0, k)], ch[chi(ido - 1, 3, k)]) = pmf(tr2, tr1);
    }
    if ido & 1 == 0 {
        for k in 0..l1 {
            let ti1 = mul(-HSQT2, add(cc[cci(ido - 1, k, 1)], cc[cci(ido - 1, k, 3)]));
            let tr1 = mul(HSQT2, sub(cc[cci(ido - 1, k, 1)], cc[cci(ido - 1, k, 3)]));
            (ch[chi(ido - 1, 0, k)], ch[chi(ido - 1, 2, k)]) = pmf(cc[cci(ido - 1, k, 0)], tr1);
            (ch[chi(0, 3, k)], ch[chi(0, 1, k)]) = pmf(ti1, cc[cci(ido - 1, k, 2)]);
        }
    }
    if ido <= 2 {
        return;
    }
    for k in 0..l1 {
        for i in (2..ido).step_by(2) {
            let ic = ido - i;
            let (cr2, ci2) = mulpm(
                w(0, i - 2),
                w(0, i - 1),
                cc[cci(i - 1, k, 1)],
                cc[cci(i, k, 1)],
            );
            let (cr3, ci3) = mulpm(
                w(1, i - 2),
                w(1, i - 1),
                cc[cci(i - 1, k, 2)],
                cc[cci(i, k, 2)],
            );
            let (cr4, ci4) = mulpm(
                w(2, i - 2),
                w(2, i - 1),
                cc[cci(i - 1, k, 3)],
                cc[cci(i, k, 3)],
            );
            let (tr1, tr4) = pmf(cr4, cr2);
            let (ti1, ti4) = pmf(ci2, ci4);
            let (tr2, tr3) = pmf(cc[cci(i - 1, k, 0)], cr3);
            let (ti2, ti3) = pmf(cc[cci(i, k, 0)], ci3);
            (ch[chi(i - 1, 0, k)], ch[chi(ic - 1, 3, k)]) = pmf(tr2, tr1);
            (ch[chi(i, 0, k)], ch[chi(ic, 3, k)]) = pmf(ti1, ti2);
            (ch[chi(i - 1, 2, k)], ch[chi(ic - 1, 1, k)]) = pmf(tr3, ti4);
            (ch[chi(i, 2, k)], ch[chi(ic, 1, k)]) = pmf(tr4, ti3);
        }
    }
}

impl RPass {
    /// The forward real transform of `data` (halfcomplex order), left in `data`.
    pub(crate) fn exec(&self, data: &mut Vec<f32>, tmp: &mut Vec<f32>) {
        match self {
            RPass::One => {}
            RPass::Radix { ip, l1, ido, wa } => {
                tmp.clear();
                tmp.resize(data.len(), 0.0);
                match ip {
                    2 => radf2(*ido, *l1, data, tmp, wa),
                    4 => radf4(*ido, *l1, data, tmp, wa),
                    _ => unreachable!("powers of two only"),
                }
                std::mem::swap(data, tmp);
            }
            RPass::Multi { passes } => {
                for p in passes.iter().rev() {
                    p.exec(data, tmp);
                }
            }
            RPass::Complexify { n, roots, pass } => {
                let n = *n;
                let mut res: Vec<C32> = data
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|p| C32::new(p[0], p[1]))
                    .collect();
                let mut ctmp = Vec::new();
                pass.exec::<true>(&mut res, &mut ctmp);
                let half = 0.5f32;
                let out = tmp;
                out.clear();
                out.resize(n, 0.0);
                out[0] = add(res[0].re, res[0].im);
                let (mut i, mut xi) = (1usize, n / 2 - 1);
                while i <= xi {
                    // xe = res[i] + conj(res[xi]); xo = (res[i].i + res[xi].i, res[xi].r -
                    // res[i].r) * conj(roots[i]).
                    let xe = cadd(res[i], C32::new(res[xi].re, -res[xi].im));
                    let a = C32::new(add(res[i].im, res[xi].im), sub(res[xi].re, res[i].re));
                    let r = roots.get(i);
                    let o = C32::new(r.re, -r.im);
                    let xo = C32::new(
                        sub(mul(a.re, o.re), mul(a.im, o.im)),
                        add(mul(a.re, o.im), mul(a.im, o.re)),
                    );
                    out[2 * i - 1] = mul(half, add(xe.re, xo.re));
                    out[2 * i] = mul(half, add(xe.im, xo.im));
                    out[2 * xi - 1] = mul(half, sub(xe.re, xo.re));
                    out[2 * xi] = mul(half, sub(xo.im, xe.im));
                    i += 1;
                    xi -= 1;
                }
                out[n - 1] = sub(res[0].re, res[0].im);
                std::mem::swap(data, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factors() {
        assert_eq!(factorize(2), vec![2]);
        assert_eq!(factorize(4), vec![4]);
        assert_eq!(factorize(8), vec![2, 4]);
        assert_eq!(factorize(64), vec![4, 4, 4]);
        assert_eq!(factorize(512), vec![2, 4, 4, 4, 4]);
    }

    #[test]
    fn plans() {
        assert!(matches!(ducc0_plan(512), RPass::Multi { .. }));
        assert!(matches!(
            ducc0_plan(1024),
            RPass::Complexify { n: 1024, .. }
        ));
        assert!(matches!(pocketfft_plan(1024), RPass::Multi { .. }));
        assert!(matches!(pocketfft_plan(4), RPass::Radix { ip: 4, .. }));
    }
}
