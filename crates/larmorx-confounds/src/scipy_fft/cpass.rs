// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! The complex passes of SciPy's single-precision FFTs for power-of-two lengths, and the two
//! libraries' plans: pocketfft's `cfftp` (SciPy 1.15, `pocketfft_hdronly.h`: `pass2`, `pass4`,
//! `pass8`, `pass_all`, `factorize`, `comp_twiddle`) and ducc0's `cfftpass` tree (SciPy 1.18.1,
//! `ducc0/fft/fft1d_impl.h`: `cfftp1`, `cfftp2`, `cfftp4`, `cfftp8`, `cfft_multipass`,
//! `cfftp_vecpass`, `cfftpass::make_pass`; `ducc0/fft/fft.h`: `cfftpass::factorize`,
//! `util1d::prime_factors`).
//!
//! The radix-2, -4 and -8 butterflies are the same operations in both libraries (the same
//! FFTPACK passes); the plans differ. pocketfft runs the passes of `n`'s factors (eights, fours,
//! a two moved to the front) one after the other, the twiddles of each from the table of `n`.
//! ducc0, asked to vectorize a one-dimensional transform (SciPy calls it so), splits a length
//! `300 < n <= 100000` divisible by four into a radix-4 pass followed by four independent
//! transforms of length `n / 4` (`cfftp_vecpass`, which runs those four in the lanes of an SSE
//! register), and a length with more than one factor above 10000 into two "packets" of its prime
//! factors (`cfft_multipass`), transforming the columns of the first and multiplying them by the
//! twiddles before the second. Up to length 256 the two plans are the same passes. The SIMD
//! lanes and the bunching of `cfft_multipass` change only where values sit, never an operation
//! (no fused multiply-add, no horizontal sum), so each pass is computed here on scalars in its
//! own order. Every operation is SSE's single-precision arithmetic, NaNs included
//! (`larmorx_core::numpy::NpFloat`). The notice of pocketfft and ducc0 is in `roots.rs`.

use super::roots::UnityRoots;
use super::{C32, cadd, csub, pm, pminplace, rotx45, rotx90, rotx135, special_mul};
use std::sync::Arc;

/// One pass of a complex plan (`cfftpass`).
#[derive(Clone, Debug)]
pub(crate) enum CPass {
    /// Length one: nothing (`cfftp1`).
    One,
    /// A radix-2, -4 or -8 butterfly of `l1` blocks of `ido` columns (`cfftp2`, `cfftp4`,
    /// `cfftp8`; pocketfft's `pass2`, `pass4`, `pass8`); `wa[(i - 1) (ip - 1) + j - 1]` the
    /// twiddle of column `i` and output `j`.
    Radix {
        ip: usize,
        l1: usize,
        ido: usize,
        wa: Vec<C32>,
    },
    /// Several passes over a length `ip` (`cfft_multipass`): `l1 = ido = 1` one after the other;
    /// `ido > 1` on each column, then the twiddles of the table; `l1 > 1` on each contiguous
    /// block, then transposed.
    Multi {
        l1: usize,
        ido: usize,
        ip: usize,
        passes: Vec<CPass>,
        rfct: usize,
        roots: Arc<UnityRoots>,
    },
    /// ducc0's `cfftp_vecpass` with four lanes: a radix-4 pass, then four transforms of length
    /// `ip / 4`.
    Vec4 {
        ip: usize,
        spass: Box<CPass>,
        vpass: Box<CPass>,
    },
}

/// `cfftpass::factorize` (and pocketfft's `cfftp::factorize`) of a power of two: eights, then
/// fours, then a two moved to the front.
pub(crate) fn factorize(mut n: usize) -> Vec<usize> {
    let mut f = Vec::new();
    while n & 7 == 0 {
        f.push(8);
        n >>= 3;
    }
    while n & 3 == 0 {
        f.push(4);
        n >>= 2;
    }
    if n & 1 == 0 {
        n >>= 1;
        f.push(2);
        let last = f.len() - 1;
        f.swap(0, last);
    }
    debug_assert_eq!(n, 1, "powers of two only");
    f
}

/// A radix pass of factor `ip` with its twiddles from `roots` (`roots[rfct j l1 i]`).
fn radix(ip: usize, l1: usize, ido: usize, roots: &UnityRoots) -> CPass {
    let n = ip * l1 * ido;
    let rfct = roots.len() / n;
    assert_eq!(rfct * n, roots.len(), "mismatch");
    let mut wa = Vec::with_capacity((ip - 1) * ido.saturating_sub(1));
    for i in 1..ido {
        for j in 1..ip {
            wa.push(roots.get(rfct * j * l1 * i));
        }
    }
    CPass::Radix { ip, l1, ido, wa }
}

/// pocketfft's complex plan of a power of two `n` (`cfftp`): the factor passes in order, the
/// twiddles from the table of `n`.
pub(crate) fn pocketfft_plan(n: usize) -> CPass {
    if n == 1 {
        return CPass::One;
    }
    let roots = Arc::new(UnityRoots::new(n));
    let mut passes = Vec::new();
    let mut l1 = 1;
    for ip in factorize(n) {
        passes.push(radix(ip, l1, n / (l1 * ip), &roots));
        l1 *= ip;
    }
    if passes.len() == 1 {
        return passes.pop().expect("one pass");
    }
    CPass::Multi {
        l1: 1,
        ido: 1,
        ip: n,
        passes,
        rfct: 1,
        roots,
    }
}

/// ducc0's `cfftpass::make_pass(l1, ido, ip, roots, vectorize)` for a power of two `ip`.
pub(crate) fn ducc0_pass(
    l1: usize,
    ido: usize,
    ip: usize,
    roots: &Arc<UnityRoots>,
    vectorize: bool,
) -> CPass {
    assert!(ip >= 1, "no zero-sized FFTs");
    if vectorize && ip > 300 && ip <= 100_000 && l1 == 1 && ido == 1 && ip.is_multiple_of(4) {
        return CPass::Vec4 {
            ip,
            spass: Box::new(ducc0_pass(1, ip / 4, 4, roots, false)),
            vpass: Box::new(ducc0_pass(1, 1, ip / 4, roots, false)),
        };
    }
    if ip == 1 {
        return CPass::One;
    }
    let factors = factorize(ip);
    if factors.len() == 1 {
        return radix(ip, l1, ido, roots);
    }
    // cfft_multipass
    let n = ip * l1 * ido;
    let rfct = roots.len() / n;
    assert_eq!(rfct * n, roots.len(), "mismatch");
    let mut passes = Vec::new();
    if ip <= 10_000 {
        let mut l1l = 1;
        for fct in factors {
            passes.push(ducc0_pass(l1l, ip / (fct * l1l), fct, roots, false));
            l1l *= fct;
        }
    } else {
        // Two packets of the prime factors (all twos), the larger first: util1d::prime_factors,
        // sorted descending, each factor to the smaller packet (the first on a tie).
        let mut packets = [1usize, 1];
        for _ in 0..ip.trailing_zeros() {
            if packets[0] > packets[1] {
                packets[1] *= 2;
            } else {
                packets[0] *= 2;
            }
        }
        let mut l1l = 1;
        for pkt in packets {
            passes.push(ducc0_pass(l1l, ip / (pkt * l1l), pkt, roots, false));
            l1l *= pkt;
        }
    }
    CPass::Multi {
        l1,
        ido,
        ip,
        passes,
        rfct,
        roots: Arc::clone(roots),
    }
}

/// ducc0's plan of a whole one-dimensional transform (`pocketfft_c(n, vectorize = true)`).
pub(crate) fn ducc0_plan(n: usize) -> CPass {
    ducc0_pass(1, 1, n, &Arc::new(UnityRoots::new(n)), true)
}

fn pass2<const FWD: bool>(ido: usize, l1: usize, cc: &[C32], ch: &mut [C32], wa: &[C32]) {
    let cci = |a: usize, b: usize, c: usize| a + ido * (b + 2 * c);
    let chi = |a: usize, b: usize, c: usize| a + ido * (b + l1 * c);
    for k in 0..l1 {
        ch[chi(0, k, 0)] = cadd(cc[cci(0, 0, k)], cc[cci(0, 1, k)]);
        ch[chi(0, k, 1)] = csub(cc[cci(0, 0, k)], cc[cci(0, 1, k)]);
        for i in 1..ido {
            ch[chi(i, k, 0)] = cadd(cc[cci(i, 0, k)], cc[cci(i, 1, k)]);
            ch[chi(i, k, 1)] =
                special_mul::<FWD>(csub(cc[cci(i, 0, k)], cc[cci(i, 1, k)]), wa[i - 1]);
        }
    }
}

fn pass4<const FWD: bool>(ido: usize, l1: usize, cc: &[C32], ch: &mut [C32], wa: &[C32]) {
    let cci = |a: usize, b: usize, c: usize| a + ido * (b + 4 * c);
    let chi = |a: usize, b: usize, c: usize| a + ido * (b + l1 * c);
    let w = |x: usize, i: usize| wa[x + (i - 1) * 3];
    for k in 0..l1 {
        {
            let (t2, t1) = pm(cc[cci(0, 0, k)], cc[cci(0, 2, k)]);
            let (t3, t4) = pm(cc[cci(0, 1, k)], cc[cci(0, 3, k)]);
            let t4 = rotx90::<FWD>(t4);
            (ch[chi(0, k, 0)], ch[chi(0, k, 2)]) = pm(t2, t3);
            (ch[chi(0, k, 1)], ch[chi(0, k, 3)]) = pm(t1, t4);
        }
        for i in 1..ido {
            let (cc0, cc1, cc2, cc3) = (
                cc[cci(i, 0, k)],
                cc[cci(i, 1, k)],
                cc[cci(i, 2, k)],
                cc[cci(i, 3, k)],
            );
            let (t2, t1) = pm(cc0, cc2);
            let (t3, t4) = pm(cc1, cc3);
            let t4 = rotx90::<FWD>(t4);
            ch[chi(i, k, 0)] = cadd(t2, t3);
            ch[chi(i, k, 1)] = special_mul::<FWD>(cadd(t1, t4), w(0, i));
            ch[chi(i, k, 2)] = special_mul::<FWD>(csub(t2, t3), w(1, i));
            ch[chi(i, k, 3)] = special_mul::<FWD>(csub(t1, t4), w(2, i));
        }
    }
}

fn pass8<const FWD: bool>(ido: usize, l1: usize, cc: &[C32], ch: &mut [C32], wa: &[C32]) {
    let cci = |a: usize, b: usize, c: usize| a + ido * (b + 8 * c);
    let chi = |a: usize, b: usize, c: usize| a + ido * (b + l1 * c);
    let w = |x: usize, i: usize| wa[x + (i - 1) * 7];
    for k in 0..l1 {
        {
            let (mut a1, mut a5) = pm(cc[cci(0, 1, k)], cc[cci(0, 5, k)]);
            let (mut a3, mut a7) = pm(cc[cci(0, 3, k)], cc[cci(0, 7, k)]);
            pminplace(&mut a1, &mut a3);
            a3 = rotx90::<FWD>(a3);

            a7 = rotx90::<FWD>(a7);
            pminplace(&mut a5, &mut a7);
            a5 = rotx45::<FWD>(a5);
            a7 = rotx135::<FWD>(a7);

            let (a0, a4) = pm(cc[cci(0, 0, k)], cc[cci(0, 4, k)]);
            let (a2, mut a6) = pm(cc[cci(0, 2, k)], cc[cci(0, 6, k)]);
            (ch[chi(0, k, 0)], ch[chi(0, k, 4)]) = pm(cadd(a0, a2), a1);
            (ch[chi(0, k, 2)], ch[chi(0, k, 6)]) = pm(csub(a0, a2), a3);
            a6 = rotx90::<FWD>(a6);
            (ch[chi(0, k, 1)], ch[chi(0, k, 5)]) = pm(cadd(a4, a6), a5);
            (ch[chi(0, k, 3)], ch[chi(0, k, 7)]) = pm(csub(a4, a6), a7);
        }
        for i in 1..ido {
            let (mut a1, mut a5) = pm(cc[cci(i, 1, k)], cc[cci(i, 5, k)]);
            let (mut a3, mut a7) = pm(cc[cci(i, 3, k)], cc[cci(i, 7, k)]);
            a7 = rotx90::<FWD>(a7);
            pminplace(&mut a1, &mut a3);
            a3 = rotx90::<FWD>(a3);
            pminplace(&mut a5, &mut a7);
            a5 = rotx45::<FWD>(a5);
            a7 = rotx135::<FWD>(a7);
            let (mut a0, mut a4) = pm(cc[cci(i, 0, k)], cc[cci(i, 4, k)]);
            let (mut a2, mut a6) = pm(cc[cci(i, 2, k)], cc[cci(i, 6, k)]);
            pminplace(&mut a0, &mut a2);
            ch[chi(i, k, 0)] = cadd(a0, a1);
            ch[chi(i, k, 4)] = special_mul::<FWD>(csub(a0, a1), w(3, i));
            ch[chi(i, k, 2)] = special_mul::<FWD>(cadd(a2, a3), w(1, i));
            ch[chi(i, k, 6)] = special_mul::<FWD>(csub(a2, a3), w(5, i));
            a6 = rotx90::<FWD>(a6);
            pminplace(&mut a4, &mut a6);
            ch[chi(i, k, 1)] = special_mul::<FWD>(cadd(a4, a5), w(0, i));
            ch[chi(i, k, 5)] = special_mul::<FWD>(csub(a4, a5), w(4, i));
            ch[chi(i, k, 3)] = special_mul::<FWD>(cadd(a6, a7), w(2, i));
            ch[chi(i, k, 7)] = special_mul::<FWD>(csub(a6, a7), w(6, i));
        }
    }
}

impl CPass {
    /// The transform of `data` (its length the pass's), left in `data`; `tmp` is scratch.
    pub(crate) fn exec<const FWD: bool>(&self, data: &mut Vec<C32>, tmp: &mut Vec<C32>) {
        match self {
            CPass::One => {}
            CPass::Radix { ip, l1, ido, wa } => {
                tmp.clear();
                tmp.resize(data.len(), C32::new(0.0, 0.0));
                match ip {
                    2 => pass2::<FWD>(*ido, *l1, data, tmp, wa),
                    4 => pass4::<FWD>(*ido, *l1, data, tmp, wa),
                    8 => pass8::<FWD>(*ido, *l1, data, tmp, wa),
                    _ => unreachable!("powers of two only"),
                }
                std::mem::swap(data, tmp);
            }
            CPass::Multi {
                l1,
                ido,
                ip,
                passes,
                rfct,
                roots,
            } => {
                let (l1, ido, ip) = (*l1, *ido, *ip);
                if l1 == 1 && ido == 1 {
                    for p in passes {
                        p.exec::<FWD>(data, tmp);
                    }
                } else if ido == 1 {
                    // l1 contiguous blocks of ip, each transformed, then CH(k, m) = ch[k + l1 m].
                    let mut out = vec![C32::new(0.0, 0.0); data.len()];
                    let (mut col, mut sub) = (Vec::with_capacity(ip), Vec::new());
                    for k in 0..l1 {
                        col.clear();
                        col.extend_from_slice(&data[k * ip..(k + 1) * ip]);
                        for p in passes {
                            p.exec::<FWD>(&mut col, &mut sub);
                        }
                        for (m, v) in col.iter().enumerate() {
                            out[k + l1 * m] = *v;
                        }
                    }
                    *data = out;
                } else if l1 == 1 {
                    // ido columns CC(i, m) = cc[i + ido m], each transformed, then multiplied by
                    // the twiddles roots[rfct m i] (column 0 and output 0 unchanged).
                    let (mut col, mut sub) = (Vec::with_capacity(ip), Vec::new());
                    for i in 0..ido {
                        col.clear();
                        col.extend((0..ip).map(|m| data[i + ido * m]));
                        for p in passes {
                            p.exec::<FWD>(&mut col, &mut sub);
                        }
                        data[i] = col[0];
                        for m in 1..ip {
                            data[i + ido * m] = if i == 0 {
                                col[m]
                            } else {
                                special_mul::<FWD>(col[m], roots.get(rfct * m * i))
                            };
                        }
                    }
                } else {
                    unreachable!("ducc0's multipass never has l1 > 1 and ido > 1");
                }
            }
            CPass::Vec4 { ip, spass, vpass } => {
                let q = ip / 4;
                spass.exec::<FWD>(data, tmp);
                let mut lane = Vec::with_capacity(q);
                let mut sub = Vec::new();
                let mut out = vec![C32::new(0.0, 0.0); *ip];
                for j in 0..4 {
                    lane.clear();
                    lane.extend_from_slice(&data[j * q..(j + 1) * q]);
                    vpass.exec::<FWD>(&mut lane, &mut sub);
                    for (i, v) in lane.iter().enumerate() {
                        out[i * 4 + j] = *v;
                    }
                }
                *data = out;
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
        assert_eq!(factorize(8), vec![8]);
        assert_eq!(factorize(16), vec![2, 8]);
        assert_eq!(factorize(32), vec![8, 4]);
        assert_eq!(factorize(128), vec![2, 8, 8]);
        assert_eq!(factorize(1024), vec![2, 8, 8, 8]);
    }

    #[test]
    fn packets() {
        // 2^14 inside a vecpass of 65536: two packets of 128.
        let roots = Arc::new(UnityRoots::new(65536));
        match ducc0_pass(1, 1, 16384, &roots, false) {
            CPass::Multi { passes, rfct, .. } => {
                assert_eq!(rfct, 4);
                assert_eq!(passes.len(), 2);
                match (&passes[0], &passes[1]) {
                    (
                        CPass::Multi {
                            l1: 1,
                            ido: 128,
                            ip: 128,
                            ..
                        },
                        CPass::Multi {
                            l1: 128,
                            ido: 1,
                            ip: 128,
                            ..
                        },
                    ) => {}
                    other => panic!("unexpected packets {other:?}"),
                }
            }
            other => panic!("unexpected plan {other:?}"),
        }
    }
}
