// SPDX-License-Identifier: Apache-2.0
//! Complex FFTs in double precision for lengths `2^a · 3^b · 5^c`.
//!
//! A self-sorting (Stockham) mixed-radix transform with radix-4, -2, -3 and -5 stages, written
//! for larmorx from the textbook algorithm (Van Loan, *Computational Frameworks for the Fast
//! Fourier Transform*, 1992, §1.7). Twiddle factors come from the correctly rounded `sin` and
//! `cos` of [`larmorx_core::math`], so every platform computes the same bits.

use larmorx_core::Complex;
use larmorx_core::math::{cos, sin};

type C = Complex<f64>;

/// A transform plan for one length.
#[derive(Clone, Debug)]
pub struct Fft {
    n: usize,
    stages: Vec<Stage>,
    /// `cos(2π/5)`, `cos(4π/5)`, `sin(2π/5)`, `sin(4π/5)`, `sin(2π/3)`.
    c5: [f64; 4],
    s3: f64,
}

#[derive(Clone, Debug)]
struct Stage {
    radix: usize,
    /// Length of the transforms already combined (`L / radix`, where `L` is this stage's).
    span: usize,
    /// `exp(−2πi·j·m/L)` at `j·(radix − 1) + m − 1`, for `j < span` and `1 ≤ m < radix`.
    twiddles: Vec<C>,
}

/// `exp(−2πi·k/l)`, exact at the quarter turns.
fn root(k: usize, l: usize) -> C {
    let k = k % l;
    if (4 * k).is_multiple_of(l) {
        return match 4 * k / l {
            0 => C::new(1.0, 0.0),
            1 => C::new(0.0, -1.0),
            2 => C::new(-1.0, 0.0),
            _ => C::new(0.0, 1.0),
        };
    }
    let angle = 2.0 * std::f64::consts::PI * (k as f64) / (l as f64);
    C::new(cos(angle), -sin(angle))
}

impl Fft {
    /// A plan for length `n`, or `None` unless `n = 2^a · 3^b · 5^c ≥ 1`.
    pub fn new(n: usize) -> Option<Fft> {
        if n == 0 {
            return None;
        }
        let mut radices = Vec::new();
        let mut rest = n;
        for p in [4, 2, 3, 5] {
            while rest.is_multiple_of(p) {
                radices.push(p);
                rest /= p;
            }
        }
        if rest != 1 {
            return None;
        }
        let mut stages = Vec::with_capacity(radices.len());
        let mut span = 1;
        for radix in radices {
            let l = span * radix;
            let twiddles = (0..span)
                .flat_map(|j| (1..radix).map(move |m| root(j * m, l)))
                .collect();
            stages.push(Stage {
                radix,
                span,
                twiddles,
            });
            span = l;
        }
        let tau = 2.0 * std::f64::consts::PI;
        Some(Fft {
            n,
            stages,
            c5: [
                cos(tau / 5.0),
                cos(2.0 * tau / 5.0),
                sin(tau / 5.0),
                sin(2.0 * tau / 5.0),
            ],
            s3: sin(tau / 3.0),
        })
    }

    /// The transform length.
    pub fn len(&self) -> usize {
        self.n
    }

    /// Whether the length is 0 (never, for a plan built by [`Fft::new`]).
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// In-place forward transform, `X[k] = Σ x[j]·exp(−2πi·jk/n)`. `scratch` must hold `n`
    /// values.
    pub fn forward(&self, data: &mut [C], scratch: &mut [C]) {
        self.run(data, scratch, false);
    }

    /// In-place inverse transform without the `1/n` factor, `x[j] = Σ X[k]·exp(2πi·jk/n)`.
    pub fn inverse(&self, data: &mut [C], scratch: &mut [C]) {
        self.run(data, scratch, true);
    }

    fn run(&self, data: &mut [C], scratch: &mut [C], inverse: bool) {
        assert_eq!(data.len(), self.n, "FFT input length");
        assert_eq!(scratch.len(), self.n, "FFT scratch length");
        let mut in_data = true;
        for stage in &self.stages {
            if in_data {
                self.stage(stage, data, scratch, inverse);
            } else {
                self.stage(stage, scratch, data, inverse);
            }
            in_data = !in_data;
        }
        if !in_data {
            data.copy_from_slice(scratch);
        }
    }

    /// One Stockham pass: the input is a `span × (n/span)` column-major array whose columns are
    /// transforms of length `span`; the output is `L × (n/L)` with transforms of length
    /// `L = span·radix`.
    fn stage(&self, st: &Stage, src: &[C], dst: &mut [C], inverse: bool) {
        let (p, span) = (st.radix, st.span);
        let l = span * p;
        let r = self.n / l;
        let mut u = [C::new(0.0, 0.0); 5];
        for j in 0..span {
            let tw = &st.twiddles[j * (p - 1)..(j + 1) * (p - 1)];
            for k in 0..r {
                u[0] = src[j + k * span];
                for m in 1..p {
                    let w = if inverse { tw[m - 1].conj() } else { tw[m - 1] };
                    u[m] = src[j + (k + m * r) * span] * w;
                }
                let base = j + k * l;
                match p {
                    2 => {
                        dst[base] = u[0] + u[1];
                        dst[base + span] = u[0] - u[1];
                    }
                    4 => {
                        let (a, b) = (u[0] + u[2], u[0] - u[2]);
                        let (c, d) = (u[1] + u[3], u[1] - u[3]);
                        // −i·d forward, +i·d inverse.
                        let rot = if inverse {
                            C::new(-d.im, d.re)
                        } else {
                            C::new(d.im, -d.re)
                        };
                        dst[base] = a + c;
                        dst[base + span] = b + rot;
                        dst[base + 2 * span] = a - c;
                        dst[base + 3 * span] = b - rot;
                    }
                    3 => {
                        let t1 = u[1] + u[2];
                        let t2 = u[0] - t1 * 0.5;
                        let t3 = (u[1] - u[2]) * self.s3;
                        // ∓i·t3.
                        let rot = if inverse {
                            C::new(-t3.im, t3.re)
                        } else {
                            C::new(t3.im, -t3.re)
                        };
                        dst[base] = u[0] + t1;
                        dst[base + span] = t2 + rot;
                        dst[base + 2 * span] = t2 - rot;
                    }
                    5 => {
                        let [c1, c2, s1, s2] = self.c5;
                        let (a1, b1) = (u[1] + u[4], u[1] - u[4]);
                        let (a2, b2) = (u[2] + u[3], u[2] - u[3]);
                        let r1 = u[0] + a1 * c1 + a2 * c2;
                        let r2 = u[0] + a1 * c2 + a2 * c1;
                        let i1 = b1 * s1 + b2 * s2;
                        let i2 = b1 * s2 - b2 * s1;
                        let (rot1, rot2) = if inverse {
                            (C::new(-i1.im, i1.re), C::new(-i2.im, i2.re))
                        } else {
                            (C::new(i1.im, -i1.re), C::new(i2.im, -i2.re))
                        };
                        dst[base] = u[0] + a1 + a2;
                        dst[base + span] = r1 + rot1;
                        dst[base + 2 * span] = r2 + rot2;
                        dst[base + 3 * span] = r2 - rot2;
                        dst[base + 4 * span] = r1 - rot1;
                    }
                    _ => unreachable!("radices are 2, 3, 4 and 5"),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn naive(x: &[C], inverse: bool) -> Vec<C> {
        let n = x.len();
        (0..n)
            .map(|k| {
                x.iter()
                    .enumerate()
                    .map(|(j, &v)| {
                        let w = root(j * k % n, n);
                        v * if inverse { w.conj() } else { w }
                    })
                    .sum()
            })
            .collect()
    }

    fn signal(n: usize) -> Vec<C> {
        // A deterministic pseudo-random signal.
        let mut state = 0x9E37_79B9_7F4A_7C15u64 ^ n as u64;
        (0..n)
            .map(|_| {
                let mut next = || {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    (state >> 11) as f64 / (1u64 << 53) as f64 - 0.5
                };
                C::new(next(), next())
            })
            .collect()
    }

    #[test]
    fn matches_the_definition_for_every_radix_mix() {
        for n in [
            1, 2, 3, 4, 5, 6, 8, 10, 12, 15, 16, 20, 24, 30, 32, 40, 45, 60, 64, 75, 120, 128, 240,
            256, 320, 480, 750,
        ] {
            let fft = Fft::new(n).unwrap();
            let x = signal(n);
            for inverse in [false, true] {
                let mut y = x.clone();
                let mut scratch = vec![C::new(0.0, 0.0); n];
                fft.run(&mut y, &mut scratch, inverse);
                let expected = naive(&x, inverse);
                let err = y
                    .iter()
                    .zip(&expected)
                    .map(|(a, b)| (a - b).norm())
                    .fold(0.0, f64::max);
                assert!(
                    err < 1e-12 * n as f64,
                    "n = {n}, inverse = {inverse}: {err}"
                );
            }
        }
    }

    #[test]
    fn round_trip_restores_the_input() {
        let n = 4096;
        let fft = Fft::new(n).unwrap();
        let x = signal(n);
        let mut y = x.clone();
        let mut scratch = vec![C::new(0.0, 0.0); n];
        fft.forward(&mut y, &mut scratch);
        fft.inverse(&mut y, &mut scratch);
        for (a, b) in y.iter().zip(&x) {
            assert!((a / n as f64 - b).norm() < 1e-14);
        }
    }

    #[test]
    fn rejects_other_lengths() {
        for n in [0, 7, 14, 22, 49] {
            assert!(Fft::new(n).is_none(), "{n}");
        }
    }
}
