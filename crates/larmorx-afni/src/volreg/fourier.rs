// SPDX-License-Identifier: Apache-2.0
//! The Fourier method's shears (`specs/3dvolreg.md` §5.5, §5.6), four row pairs at a time.
//!
//! Each pair of rows is one complex transform (§5.6 steps 3 to 6); four pairs share one pass
//! of a batched FFT (`crate::fft::Fft::forward_batch`, every lane computed with exactly the
//! single transform's operations) and the spec's float32 separation, phase recursion and
//! rejoin lane by lane. The in-plane shears (along x and y) run plane by plane in a buffer of
//! one plane per thread; a z-shear runs over the pencils of each y slab, writing a y-major
//! volume (`[y][k][x]`) so that threads write disjoint slabs. Every row's result is the same
//! as shifting the pair on its own, whatever the thread count.

use std::sync::Arc;

use rayon::prelude::*;

use crate::fft::Fft;

use super::rotate::{Plan, Shear, Trig, coefficients};
use super::shift;
use super::sweep::{Volumes, Window, pad_into};

/// Pairs per batch (four keep a batch's buffers in the first-level cache for rows of a few
/// hundred samples).
const B: usize = 4;

/// One Fourier shear: its float32 coefficients, the axes of its rows, the row length and the
/// transform length (§5.6 step 2).
#[derive(Clone, Copy, Debug)]
struct Stage {
    axis: usize,
    a: f32,
    b: f32,
    s: f32,
    cu: f32,
    cw: f32,
    nu: usize,
    l: usize,
}

impl Stage {
    fn new(sh: &Shear, dims: [usize; 3]) -> Option<Stage> {
        let (a, b, s, [_, st]) = coefficients(sh, dims)?;
        let mut others = (0..3).filter(|&x| x != sh.axis);
        let (u, w) = (others.next()?, others.next()?);
        Some(Stage {
            axis: sh.axis,
            a,
            b,
            s,
            cu: 0.5f32 * (dims[u] as f32 - 1.0),
            cw: 0.5f32 * (dims[w] as f32 - 1.0),
            nu: dims[u],
            l: shift::fourier_length(dims[sh.axis], st),
        })
    }

    /// The shift of row `(ui, wi)` by its own formula.
    #[inline]
    fn first(&self, ui: usize, wi: usize) -> f32 {
        self.a * (ui as f32 - self.cu) + self.b * (wi as f32 - self.cw) + self.s
    }

    /// The pairs of the rows of `wi` (§5.5): `(u of f, whether g exists, shift of f, shift of
    /// g or the lone row's second shift)`.
    fn pairs(&self, wi: usize) -> Vec<(usize, bool, f32, f32)> {
        let mut out = Vec::with_capacity(self.nu.div_ceil(2));
        let mut prev_second = 0.0f32;
        let mut u = 0;
        while u < self.nu {
            let a0 = self.first(u, wi);
            if u + 1 < self.nu {
                let a1 = a0 + self.a;
                out.push((u, true, a0, a1));
                prev_second = a1;
            } else {
                let second = if self.axis == 0 {
                    a0 + self.a
                } else {
                    prev_second
                };
                out.push((u, false, a0, second));
            }
            u += 2;
        }
        out
    }
}

/// FFT plans by length, built once.
fn plan(l: usize) -> Arc<Fft> {
    shift::plan(l)
}

/// The buffers of a batch of pairs of one transform length.
struct Batch {
    l: usize,
    fft: Arc<Fft>,
    re: Vec<[f64; B]>,
    im: Vec<[f64; B]>,
    sre: Vec<[f64; B]>,
    sim: Vec<[f64; B]>,
    fr: Vec<[f32; B]>,
    fi: Vec<[f32; B]>,
    gr: Vec<[f32; B]>,
    gi: Vec<[f32; B]>,
    dk: f32,
}

impl Batch {
    fn new(l: usize) -> Batch {
        let h = l / 2 + 1;
        Batch {
            l,
            fft: plan(l),
            re: vec![[0.0; B]; l],
            im: vec![[0.0; B]; l],
            sre: vec![[0.0; B]; l],
            sim: vec![[0.0; B]; l],
            fr: vec![[0.0; B]; h],
            fi: vec![[0.0; B]; h],
            gr: vec![[0.0; B]; h],
            gi: vec![[0.0; B]; h],
            dk: (2.0 * std::f64::consts::PI / l as f64) as f32,
        }
    }

    /// Clears `re` and `im` beyond the rows' length `n`.
    fn clear_tail(&mut self, n: usize) {
        for m in n..self.l {
            self.re[m] = [0.0; B];
            self.im[m] = [0.0; B];
        }
    }

    /// §5.6 steps 3 to 6 for the loaded lanes (`re` = f, `im` = g for `m < n`), the shifts
    /// `a1` (f) and `a2` (g). The results are left in `re` and `im` before the factor 0.5/L.
    #[allow(clippy::needless_range_loop)]
    fn run(&mut self, a1: &[f32; B], a2: &[f32; B], trig: Trig) {
        let l = self.l;
        let half = l / 2;
        self.fft
            .forward_batch(&mut self.re, &mut self.im, &mut self.sre, &mut self.sim);
        let (re, im) = (&self.re, &self.im);
        let (fr, fi, gr, gi) = (&mut self.fr, &mut self.fi, &mut self.gr, &mut self.gi);
        // Step 4: the two spectra (each twice its size).
        for q in 0..B {
            fr[0][q] = 2.0 * (re[0][q] as f32);
            fi[0][q] = 0.0;
            gr[0][q] = 2.0 * (im[0][q] as f32);
            gi[0][q] = 0.0;
        }
        for k in 1..half {
            for q in 0..B {
                let (ar, ai) = (re[k][q] as f32, im[k][q] as f32);
                let (br, bi) = (re[l - k][q] as f32, im[l - k][q] as f32);
                fr[k][q] = ar + br;
                fi[k][q] = ai - bi;
                gr[k][q] = ai + bi;
                gi[k][q] = -ar + br;
            }
        }
        for q in 0..B {
            fr[half][q] = 2.0 * (re[half][q] as f32);
            fi[half][q] = 0.0;
            gr[half][q] = 2.0 * (im[half][q] as f32);
            gi[half][q] = 0.0;
        }
        // Step 5: the phase factors, accumulated.
        for (sr, si, alpha) in [(&mut *fr, &mut *fi, a1), (&mut *gr, &mut *gi, a2)] {
            let mut er = [0f32; B];
            let mut ei = [0f32; B];
            for q in 0..B {
                let phi = -alpha[q] * self.dk;
                let (s, c) = trig.sincos(f64::from(phi));
                er[q] = c as f32;
                ei[q] = s as f32;
            }
            let mut pr = [1.0f32; B];
            let mut pi = [0.0f32; B];
            for k in 1..=half {
                for q in 0..B {
                    let (r, i) = (pr[q] * er[q] - pi[q] * ei[q], pr[q] * ei[q] + pi[q] * er[q]);
                    pr[q] = r;
                    pi[q] = i;
                    let (xr, xi) = (sr[k][q], si[k][q]);
                    sr[k][q] = r * xr - i * xi;
                    si[k][q] = r * xi + i * xr;
                }
            }
            si[half] = [0.0; B];
        }
        // Step 6: rejoin, transform back.
        let (re, im) = (&mut self.re, &mut self.im);
        for q in 0..B {
            re[0][q] = f64::from(fr[0][q]);
            im[0][q] = f64::from(gr[0][q]);
        }
        for k in 1..half {
            for q in 0..B {
                let (a, b, c, d) = (fr[k][q], fi[k][q], gr[k][q], gi[k][q]);
                re[k][q] = f64::from(a - d);
                im[k][q] = f64::from(b + c);
                re[l - k][q] = f64::from(a + d);
                im[l - k][q] = f64::from(-b + c);
            }
        }
        for q in 0..B {
            re[half][q] = f64::from(fr[half][q]);
            im[half][q] = f64::from(gr[half][q]);
        }
        self.fft
            .inverse_batch(&mut self.re, &mut self.im, &mut self.sre, &mut self.sim);
    }
}

/// Shifts the rows of one `w` value: `pairs` from [`Stage::pairs`], rows of length `n`;
/// sample `m` of row `u` is read at `src[isrc(u, m)]` (or `dst[...]` when `src` is `None`:
/// in place, as every batch reads all its rows before writing them) and written at
/// `dst[idst(u, m)]`.
#[allow(clippy::too_many_arguments)]
fn shift_rows(
    st: &Stage,
    batch: &mut Batch,
    pairs: &[(usize, bool, f32, f32)],
    n: usize,
    trig: Trig,
    src: Option<&[f32]>,
    dst: &mut [f32],
    isrc: impl Fn(usize, usize) -> usize,
    idst: impl Fn(usize, usize) -> usize,
) {
    let factor = 0.5f32 / st.l as f32;
    let beyond = |a: f32| a < -(n as f32) || a > n as f32;
    for chunk in pairs.chunks(B) {
        batch.clear_tail(n);
        let mut a1 = [0f32; B];
        let mut a2 = [0f32; B];
        for (q, &(_, _, s1, s2)) in chunk.iter().enumerate() {
            a1[q] = s1;
            a2[q] = s2;
        }
        for m in 0..n {
            let (re, im) = (&mut batch.re[m], &mut batch.im[m]);
            *re = [0.0; B];
            *im = [0.0; B];
            for (q, &(u, two, _, _)) in chunk.iter().enumerate() {
                let read = |i: usize| match src {
                    Some(s) => s[i],
                    None => dst[i],
                };
                re[q] = f64::from(read(isrc(u, m)));
                if two {
                    im[q] = f64::from(read(isrc(u + 1, m)));
                }
            }
        }
        batch.run(&a1, &a2, trig);
        for m in 0..n {
            let (re, im) = (&batch.re[m], &batch.im[m]);
            for (q, &(u, two, s1, s2)) in chunk.iter().enumerate() {
                // §5.6 step 1: both shifts beyond the row: zeros (a lone row too: AFNI-97's
                // correction, the command line names it).
                let clear = beyond(s1) && beyond(s2);
                dst[idst(u, m)] = if clear { 0.0 } else { factor * (re[q] as f32) };
                if two {
                    dst[idst(u + 1, m)] = if clear { 0.0 } else { factor * (im[q] as f32) };
                }
            }
        }
    }
}

/// The layout of a padded volume: planes (`[k][y][x]`) or y slabs (`[y][k][x]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layout {
    Planes,
    Slabs,
}

#[inline]
fn index(layout: Layout, dims: [usize; 3], x: usize, y: usize, k: usize) -> usize {
    match layout {
        Layout::Planes => x + dims[0] * (y + dims[1] * k),
        Layout::Slabs => x + dims[0] * (k + dims[2] * y),
    }
}

/// The in-plane shears `stages` of every plane: from `src` (in `layout`) to `dst` (planes),
/// which may be `src` itself when both are planes (then `src` is `None`).
fn in_plane(
    stages: &[Stage],
    dims: [usize; 3],
    trig: Trig,
    src: Option<(&[f32], Layout)>,
    dst: &mut [f32],
) {
    let (px, py) = (dims[0], dims[1]);
    let plane = px * py;
    dst.par_chunks_mut(plane).enumerate().for_each_init(
        || stages.iter().map(|st| Batch::new(st.l)).collect::<Vec<_>>(),
        |batches, (k, pl)| {
            if let Some((src, layout)) = src {
                for y in 0..py {
                    let at = index(layout, dims, 0, y, k);
                    pl[y * px..(y + 1) * px].copy_from_slice(&src[at..at + px]);
                }
            }
            for (st, batch) in stages.iter().zip(batches.iter_mut()) {
                let pairs = st.pairs(k);
                if st.axis == 0 {
                    let at = |y: usize, m: usize| y * px + m;
                    shift_rows(st, batch, &pairs, px, trig, None, pl, at, at);
                } else {
                    let at = |x: usize, m: usize| m * px + x;
                    shift_rows(st, batch, &pairs, py, trig, None, pl, at, at);
                }
            }
        },
    );
}

/// The z-shear `st` of every pencil: from `src` (in planes) to `dst` (slabs), or in place
/// in `dst` (slabs) when `src` is `None`.
fn pencils(st: &Stage, dims: [usize; 3], trig: Trig, src: Option<&[f32]>, dst: &mut [f32]) {
    let (px, pz) = (dims[0], dims[2]);
    dst.par_chunks_mut(px * pz).enumerate().for_each_init(
        || Batch::new(st.l),
        |batch, (y, slab)| {
            let pairs = st.pairs(y);
            let in_slab = |x: usize, m: usize| x + px * m;
            match src {
                Some(s) => {
                    let in_planes = |x: usize, m: usize| index(Layout::Planes, dims, x, y, m);
                    shift_rows(
                        st,
                        batch,
                        &pairs,
                        pz,
                        trig,
                        Some(s),
                        slab,
                        in_planes,
                        in_slab,
                    );
                }
                None => shift_rows(st, batch, &pairs, pz, trig, None, slab, in_slab, in_slab),
            }
        },
    );
}

/// Applies the plan `p` (with shears) with the Fourier method to `vol` (a volume of `n`,
/// padded by `z`), writing the window `win` into `out`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply(
    p: &Plan,
    vol: &[f32],
    n: [usize; 3],
    z: usize,
    win: &Window,
    out: &mut [f32],
    trig: Trig,
    bufs: &mut Volumes,
) {
    let shears = p.shears.as_ref().expect("a plan with shears");
    if win.is_empty() {
        return;
    }
    let dims = n.map(|d| d + 2 * z);
    let total = dims[0] * dims[1] * dims[2];
    let stages: Vec<Stage> = shears
        .iter()
        .filter_map(|sh| Stage::new(sh, dims))
        .collect();
    pad_into(vol, n, z, p.flip, dims, &mut bufs.pad);
    // `cur`: which buffer holds the data (true: pad) and its layout.
    let mut in_pad = true;
    let mut layout = Layout::Planes;
    let mut i = 0;
    while i < stages.len() {
        if stages[i].axis == 2 {
            let st = &stages[i];
            let (src, dst) = if in_pad {
                (&mut bufs.pad, &mut bufs.mid)
            } else {
                (&mut bufs.mid, &mut bufs.pad)
            };
            if layout == Layout::Slabs {
                // In place: every pencil of a y slab lies in that slab.
                pencils(st, dims, trig, None, src);
            } else {
                if dst.len() != total {
                    dst.clear();
                    dst.resize(total, 0.0);
                }
                pencils(st, dims, trig, Some(src), dst);
                in_pad = !in_pad;
                layout = Layout::Slabs;
            }
            i += 1;
        } else {
            let mut j = i;
            while j < stages.len() && stages[j].axis != 2 {
                j += 1;
            }
            let group = &stages[i..j];
            let (src, dst) = if in_pad {
                (&mut bufs.pad, &mut bufs.mid)
            } else {
                (&mut bufs.mid, &mut bufs.pad)
            };
            if layout == Layout::Planes {
                in_plane(group, dims, trig, None, src);
            } else {
                if dst.len() != total {
                    dst.clear();
                    dst.resize(total, 0.0);
                }
                in_plane(group, dims, trig, Some((src, Layout::Slabs)), dst);
                in_pad = !in_pad;
                layout = Layout::Planes;
            }
            i = j;
        }
    }
    let src: &[f32] = if in_pad { &bufs.pad } else { &bufs.mid };
    let [wx, wy, _] = win.n;
    out.par_chunks_mut(wx * wy)
        .enumerate()
        .for_each(|(kk, plane)| {
            let k = win.lo[2] + kk + z;
            for jj in 0..wy {
                let at = index(layout, dims, win.lo[0] + z, win.lo[1] + jj + z, k);
                plane[jj * wx..(jj + 1) * wx].copy_from_slice(&src[at..at + wx]);
            }
        });
}
