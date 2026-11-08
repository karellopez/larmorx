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

/// `B` transforms at once, lane by lane (structure of arrays: element `m` of lane `q` is
/// `re[m][q] + i·im[m][q]`). Each lane is computed with exactly the operations of
/// [`Fft::forward`] and [`Fft::inverse`], so it gives their bits; the lanes run side by side,
/// which the compiler vectorises.
impl Fft {
    /// [`Fft::forward`] of every lane.
    pub fn forward_batch<const B: usize>(
        &self,
        re: &mut [[f64; B]],
        im: &mut [[f64; B]],
        sre: &mut [[f64; B]],
        sim: &mut [[f64; B]],
    ) {
        self.run_batch(re, im, sre, sim, false);
    }

    /// [`Fft::inverse`] of every lane.
    pub fn inverse_batch<const B: usize>(
        &self,
        re: &mut [[f64; B]],
        im: &mut [[f64; B]],
        sre: &mut [[f64; B]],
        sim: &mut [[f64; B]],
    ) {
        self.run_batch(re, im, sre, sim, true);
    }

    fn run_batch<const B: usize>(
        &self,
        re: &mut [[f64; B]],
        im: &mut [[f64; B]],
        sre: &mut [[f64; B]],
        sim: &mut [[f64; B]],
        inverse: bool,
    ) {
        assert!(re.len() == self.n && im.len() == self.n, "FFT input length");
        assert!(
            sre.len() == self.n && sim.len() == self.n,
            "FFT scratch length"
        );
        let mut in_data = true;
        for stage in &self.stages {
            if in_data {
                self.stage_batch(stage, re, im, sre, sim, inverse);
            } else {
                self.stage_batch(stage, sre, sim, re, im, inverse);
            }
            in_data = !in_data;
        }
        if !in_data {
            re.copy_from_slice(sre);
            im.copy_from_slice(sim);
        }
    }

    #[allow(clippy::needless_range_loop)]
    fn stage_batch<const B: usize>(
        &self,
        st: &Stage,
        sre: &[[f64; B]],
        sim: &[[f64; B]],
        dre: &mut [[f64; B]],
        dim: &mut [[f64; B]],
        inverse: bool,
    ) {
        let (p, span) = (st.radix, st.span);
        let l = span * p;
        let r = self.n / l;
        let mut ur = [[0.0f64; B]; 5];
        let mut ui = [[0.0f64; B]; 5];
        for j in 0..span {
            let tw = &st.twiddles[j * (p - 1)..(j + 1) * (p - 1)];
            for k in 0..r {
                ur[0] = sre[j + k * span];
                ui[0] = sim[j + k * span];
                for m in 1..p {
                    let w = if inverse { tw[m - 1].conj() } else { tw[m - 1] };
                    let (xr, xi) = (&sre[j + (k + m * r) * span], &sim[j + (k + m * r) * span]);
                    for q in 0..B {
                        ur[m][q] = xr[q] * w.re - xi[q] * w.im;
                        ui[m][q] = xr[q] * w.im + xi[q] * w.re;
                    }
                }
                let base = j + k * l;
                match p {
                    2 => {
                        for q in 0..B {
                            dre[base][q] = ur[0][q] + ur[1][q];
                            dim[base][q] = ui[0][q] + ui[1][q];
                            dre[base + span][q] = ur[0][q] - ur[1][q];
                            dim[base + span][q] = ui[0][q] - ui[1][q];
                        }
                    }
                    4 => {
                        for q in 0..B {
                            let (ar, ai) = (ur[0][q] + ur[2][q], ui[0][q] + ui[2][q]);
                            let (br, bi) = (ur[0][q] - ur[2][q], ui[0][q] - ui[2][q]);
                            let (cr, ci) = (ur[1][q] + ur[3][q], ui[1][q] + ui[3][q]);
                            let (dr, di) = (ur[1][q] - ur[3][q], ui[1][q] - ui[3][q]);
                            // −i·d forward, +i·d inverse.
                            let (rr, ri) = if inverse { (-di, dr) } else { (di, -dr) };
                            dre[base][q] = ar + cr;
                            dim[base][q] = ai + ci;
                            dre[base + span][q] = br + rr;
                            dim[base + span][q] = bi + ri;
                            dre[base + 2 * span][q] = ar - cr;
                            dim[base + 2 * span][q] = ai - ci;
                            dre[base + 3 * span][q] = br - rr;
                            dim[base + 3 * span][q] = bi - ri;
                        }
                    }
                    3 => {
                        for q in 0..B {
                            let (t1r, t1i) = (ur[1][q] + ur[2][q], ui[1][q] + ui[2][q]);
                            let (t2r, t2i) = (ur[0][q] - t1r * 0.5, ui[0][q] - t1i * 0.5);
                            let (t3r, t3i) = (
                                (ur[1][q] - ur[2][q]) * self.s3,
                                (ui[1][q] - ui[2][q]) * self.s3,
                            );
                            let (rr, ri) = if inverse { (-t3i, t3r) } else { (t3i, -t3r) };
                            dre[base][q] = ur[0][q] + t1r;
                            dim[base][q] = ui[0][q] + t1i;
                            dre[base + span][q] = t2r + rr;
                            dim[base + span][q] = t2i + ri;
                            dre[base + 2 * span][q] = t2r - rr;
                            dim[base + 2 * span][q] = t2i - ri;
                        }
                    }
                    5 => {
                        let [c1, c2, s1, s2] = self.c5;
                        for q in 0..B {
                            let (a1r, a1i) = (ur[1][q] + ur[4][q], ui[1][q] + ui[4][q]);
                            let (b1r, b1i) = (ur[1][q] - ur[4][q], ui[1][q] - ui[4][q]);
                            let (a2r, a2i) = (ur[2][q] + ur[3][q], ui[2][q] + ui[3][q]);
                            let (b2r, b2i) = (ur[2][q] - ur[3][q], ui[2][q] - ui[3][q]);
                            let (r1r, r1i) = (
                                ur[0][q] + a1r * c1 + a2r * c2,
                                ui[0][q] + a1i * c1 + a2i * c2,
                            );
                            let (r2r, r2i) = (
                                ur[0][q] + a1r * c2 + a2r * c1,
                                ui[0][q] + a1i * c2 + a2i * c1,
                            );
                            let (i1r, i1i) = (b1r * s1 + b2r * s2, b1i * s1 + b2i * s2);
                            let (i2r, i2i) = (b1r * s2 - b2r * s1, b1i * s2 - b2i * s1);
                            let ((o1r, o1i), (o2r, o2i)) = if inverse {
                                ((-i1i, i1r), (-i2i, i2r))
                            } else {
                                ((i1i, -i1r), (i2i, -i2r))
                            };
                            dre[base][q] = ur[0][q] + a1r + a2r;
                            dim[base][q] = ui[0][q] + a1i + a2i;
                            dre[base + span][q] = r1r + o1r;
                            dim[base + span][q] = r1i + o1i;
                            dre[base + 2 * span][q] = r2r + o2r;
                            dim[base + 2 * span][q] = r2i + o2i;
                            dre[base + 3 * span][q] = r2r - o2r;
                            dim[base + 3 * span][q] = r2i - o2i;
                            dre[base + 4 * span][q] = r1r - o1r;
                            dim[base + 4 * span][q] = r1i - o1i;
                        }
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

    /// Every lane of a batch gives the single transform's bits.
    #[test]
    fn batches_give_the_single_transforms_bits() {
        for n in [2usize, 6, 10, 30, 48, 60, 80, 120, 192, 240] {
            let fft = Fft::new(n).unwrap();
            let lanes: Vec<Vec<C>> = (0..4)
                .map(|q| {
                    signal(n)
                        .into_iter()
                        .map(|c| c * (1.0 + q as f64 * 0.37))
                        .collect()
                })
                .collect();
            for inverse in [false, true] {
                let mut re = vec![[0.0f64; 4]; n];
                let mut im = vec![[0.0f64; 4]; n];
                for (q, lane) in lanes.iter().enumerate() {
                    for (m, c) in lane.iter().enumerate() {
                        re[m][q] = c.re;
                        im[m][q] = c.im;
                    }
                }
                let (mut sre, mut sim) = (vec![[0.0f64; 4]; n], vec![[0.0f64; 4]; n]);
                if inverse {
                    fft.inverse_batch(&mut re, &mut im, &mut sre, &mut sim);
                } else {
                    fft.forward_batch(&mut re, &mut im, &mut sre, &mut sim);
                }
                for (q, lane) in lanes.iter().enumerate() {
                    let mut one = lane.clone();
                    let mut scratch = vec![C::new(0.0, 0.0); n];
                    if inverse {
                        fft.inverse(&mut one, &mut scratch);
                    } else {
                        fft.forward(&mut one, &mut scratch);
                    }
                    for m in 0..n {
                        assert_eq!(
                            re[m][q].to_bits(),
                            one[m].re.to_bits(),
                            "n {n} lane {q} m {m}"
                        );
                        assert_eq!(
                            im[m][q].to_bits(),
                            one[m].im.to_bits(),
                            "n {n} lane {q} m {m}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn rejects_other_lengths() {
        for n in [0, 7, 14, 22, 49] {
            assert!(Fft::new(n).is_none(), "{n}");
        }
    }
}

#[cfg(test)]
mod timing {
    use super::*;

    /// `cargo test --release -p larmorx-afni fft_timing -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn fft_timing() {
        for n in [120usize, 192, 240] {
            let fft = Fft::new(n).unwrap();
            let reps = 20000;
            let mut data: Vec<C> = (0..n).map(|i| C::new(i as f64, 1.0)).collect();
            let mut scratch = vec![C::new(0.0, 0.0); n];
            let t = std::time::Instant::now();
            for _ in 0..reps * 4 {
                fft.forward(&mut data, &mut scratch);
                data[0] = C::new(1.0, 0.0);
            }
            let single = t.elapsed();
            let mut re = vec![[1.0f64; 4]; n];
            let mut im = vec![[0.5f64; 4]; n];
            let (mut sre, mut sim) = (vec![[0.0f64; 4]; n], vec![[0.0f64; 4]; n]);
            let t = std::time::Instant::now();
            for _ in 0..reps {
                fft.forward_batch(&mut re, &mut im, &mut sre, &mut sim);
                re[0] = [1.0; 4];
            }
            eprintln!(
                "n {n}: 4 single {:?}, one batch of 4 {:?} (per 8 transforms)",
                single / reps as u32,
                t.elapsed() / reps as u32
            );
        }
    }
}
