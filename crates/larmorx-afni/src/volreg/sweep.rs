// SPDX-License-Identifier: Apache-2.0
//! The four shears of a resampling (`specs/3dvolreg.md` §5.3 to §5.6) applied by sweeps over
//! planes, for the polynomial methods and nearest neighbour.
//!
//! A shear along x or y moves samples within a z plane; a shear along z moves them between
//! planes. So the shears are grouped: a group is an optional z-shear followed by the in-plane
//! shears after it, and each group is one sweep over the output planes. For each output plane
//! a thread computes the z-shear's plane straight from the group's source volume, applies the
//! in-plane shears on two plane buffers of its own (they stay in cache), and writes the plane
//! to the group's destination: the next group's source, or the output's window. A plan has
//! at most two z-shears (§5.3: X1, Y, Z, X2), so at most two sweeps over memory replace the
//! four passes and the transpositions of a row-by-row implementation.
//!
//! Every output value is computed with the spec's arithmetic and term order (§5.5, §5.6: the
//! pair rule of the rows' shifts, the float32 interior sums, the double sums at the rows'
//! ends): the y- and z-shears are vectorised across x, over runs of columns that share their
//! integer offset, each lane summing its own taps in the spec's order. So the result is the
//! same, bit for bit, as shifting every row on its own, whatever the thread count.

use rayon::prelude::*;

use super::rotate::{Plan, Shear, coefficients};
use super::shift::{self, Method};

/// One shear in float32 (§5.5) with the two axes of its rows (`u < w`) and their centres.
#[derive(Clone, Copy, Debug)]
struct Stage {
    axis: usize,
    a: f32,
    b: f32,
    s: f32,
    cu: f32,
    cw: f32,
}

impl Stage {
    fn new(sh: &Shear, dims: [usize; 3]) -> Option<Stage> {
        let (a, b, s, _) = coefficients(sh, dims)?;
        let mut others = (0..3).filter(|&x| x != sh.axis);
        let (u, w) = (others.next()?, others.next()?);
        Some(Stage {
            axis: sh.axis,
            a,
            b,
            s,
            cu: 0.5f32 * (dims[u] as f32 - 1.0),
            cw: 0.5f32 * (dims[w] as f32 - 1.0),
        })
    }

    /// The shift of the row `(ui, wi)` with the pair rule (§5.5): row `2m + 1` takes row
    /// `2m`'s shift plus `a`.
    #[inline]
    fn alpha(&self, ui: usize, wi: usize) -> f32 {
        let first =
            |u: usize| self.a * (u as f32 - self.cu) + self.b * (wi as f32 - self.cw) + self.s;
        if ui % 2 == 0 {
            first(ui)
        } else {
            first(ui - 1) + self.a
        }
    }
}

/// A row that becomes all zeros (§5.6: `ia ≤ −n` or `ia ≥ n`).
const ZERO: i32 = i32::MIN;

/// The taps of a method: their sample offsets in the order summed, and the lowest and highest.
fn taps(method: Method) -> (&'static [i64], i64, i64) {
    match method {
        Method::Linear => (&[0, 1], 0, 1),
        Method::Cubic => (&[-1, 0, 1, 2], -1, 2),
        Method::Quintic => (&[-2, -1, 0, 1, 2, 3], -2, 3),
        Method::Heptic => (&[-2, -1, 0, 1, 2, 3, -3, 4], -3, 4),
        Method::NN | Method::Fourier => (&[0], 0, 0),
    }
}

/// The integer offset and the weights of a row shifted by `alpha` (length `len`).
#[inline]
fn row(method: Method, alpha: f32, len: usize) -> (i32, [f32; 8]) {
    match shift::split(alpha, len) {
        None => (ZERO, [0.0; 8]),
        Some((ia, x)) => (ia as i32, shift::weights(method, x).0),
    }
}

/// The per-thread buffers of a sweep: two planes, and one plane's row offsets and weights.
struct Planes {
    a: Vec<f32>,
    b: Vec<f32>,
    ia: Vec<i32>,
    /// The weights by tap: `w[t·len + x]`.
    w: Vec<f32>,
}

impl Planes {
    fn new(plane: usize, line: usize) -> Planes {
        Planes {
            a: vec![0.0; plane],
            b: vec![0.0; plane],
            ia: vec![0; line],
            w: vec![0.0; 8 * line],
        }
    }
}

/// The value of one output from the double sum at a row's ends (§5.6): float32 weights,
/// samples as doubles, zeros outside, rounded at the end. `at(j)` reads sample `j` of the
/// row (`None` outside it).
#[inline]
fn edge_sum(
    k: &[i64],
    w: impl Fn(usize) -> f32,
    base: i64,
    at: impl Fn(i64) -> Option<f32>,
) -> f32 {
    let mut s = 0.0f64;
    for (t, &kt) in k.iter().enumerate() {
        let v = at(base + kt).map_or(0.0, f64::from);
        s += f64::from(w(t)) * v;
    }
    s as f32
}

/// `Σ_t w(t)·r(t)` in float32, the terms in order (one output of the interior).
#[inline(always)]
fn dot(n: usize, w: impl Fn(usize) -> f32, r: impl Fn(usize) -> f32) -> f32 {
    let mut acc = w(0) * r(0);
    for t in 1..n {
        acc = acc + w(t) * r(t);
    }
    acc
}

/// Runs shorter than this are summed voxel by voxel (no slices to set up).
const SHORT: usize = 8;

/// Runs of equal values in `ia[..len]`: `(start, end, value)`.
fn runs(ia: &[i32], mut f: impl FnMut(usize, usize, i32)) {
    let mut x0 = 0;
    while x0 < ia.len() {
        let v = ia[x0];
        let mut x1 = x0 + 1;
        while x1 < ia.len() && ia[x1] == v {
            x1 += 1;
        }
        f(x0, x1, v);
        x0 = x1;
    }
}

/// `dst[x] = Σ_t w_t[x]·src_t[x]` over a run (the interior: every sample inside), each lane's
/// terms in the method's order.
#[inline]
fn interior(dst: &mut [f32], w: &[&[f32]; 8], r: &[&[f32]; 8], n: usize) {
    let len = dst.len();
    match n {
        1 => dst.copy_from_slice(&r[0][..len]),
        2 => {
            for x in 0..len {
                dst[x] = w[0][x] * r[0][x] + w[1][x] * r[1][x];
            }
        }
        4 => {
            for x in 0..len {
                dst[x] =
                    w[0][x] * r[0][x] + w[1][x] * r[1][x] + w[2][x] * r[2][x] + w[3][x] * r[3][x];
            }
        }
        6 => {
            for x in 0..len {
                dst[x] = w[0][x] * r[0][x]
                    + w[1][x] * r[1][x]
                    + w[2][x] * r[2][x]
                    + w[3][x] * r[3][x]
                    + w[4][x] * r[4][x]
                    + w[5][x] * r[5][x];
            }
        }
        8 => {
            for x in 0..len {
                dst[x] = w[0][x] * r[0][x]
                    + w[1][x] * r[1][x]
                    + w[2][x] * r[2][x]
                    + w[3][x] * r[3][x]
                    + w[4][x] * r[4][x]
                    + w[5][x] * r[5][x]
                    + w[6][x] * r[6][x]
                    + w[7][x] * r[7][x];
            }
        }
        _ => unreachable!("taps are 1, 2, 4, 6 or 8"),
    }
}

/// The x-shear of plane `k`: each row along x shifted on its own (`src` to `dst`, planes of
/// `px × py`).
fn shear_x(st: &Stage, method: Method, k: usize, px: usize, src: &[f32], dst: &mut [f32]) {
    for (y, (s, d)) in src
        .chunks_exact(px)
        .zip(dst.chunks_exact_mut(px))
        .enumerate()
    {
        shift::shift_poly(method, s, st.alpha(y, k), d);
    }
}

/// The y-shear of plane `k` (`src` to `dst`): column `x` shifted along y by its own shift,
/// computed across x over runs of columns with the same integer offset.
fn shear_y(
    st: &Stage,
    method: Method,
    k: usize,
    dims: [usize; 3],
    src: &[f32],
    dst: &mut [f32],
    bufs: &mut [i32],
    wbuf: &mut [f32],
) {
    let (px, py) = (dims[0], dims[1]);
    let (kk, kmin, kmax) = taps(method);
    let nt = kk.len();
    for x in 0..px {
        let (ia, w) = row(method, st.alpha(x, k), py);
        bufs[x] = ia;
        for t in 0..nt {
            wbuf[t * px + x] = w[t];
        }
    }
    let (ia_all, wsoa) = (&bufs[..px], &wbuf[..]);
    if method == Method::NN {
        let pyi = py as i64;
        for y in 0..py {
            let d = &mut dst[y * px..(y + 1) * px];
            for (x, (o, &ia)) in d.iter_mut().zip(ia_all).enumerate() {
                let j = y as i64 + i64::from(ia);
                *o = if ia != ZERO && (0..pyi).contains(&j) {
                    src[j as usize * px + x]
                } else {
                    0.0
                };
            }
        }
        return;
    }
    runs(ia_all, |x0, x1, ia| {
        if ia == ZERO {
            for y in 0..py {
                dst[y * px + x0..y * px + x1].fill(0.0);
            }
            return;
        }
        let ia = i64::from(ia);
        let pyi = py as i64;
        let lo = (-ia - kmin).max(0);
        let hi = (pyi - 1 - ia - kmax).min(pyi - 1);
        let w: [&[f32]; 8] = std::array::from_fn(|t| {
            if t < nt {
                &wsoa[t * px + x0..t * px + x1]
            } else {
                &wsoa[..0]
            }
        });
        for y in 0..py {
            let yi = y as i64;
            let d = &mut dst[y * px + x0..y * px + x1];
            if yi >= lo && yi <= hi {
                if x1 - x0 < SHORT {
                    for (x, o) in d.iter_mut().enumerate() {
                        *o = dot(
                            nt,
                            |t| w[t][x],
                            |t| src[(yi + ia + kk[t]) as usize * px + x0 + x],
                        );
                    }
                    continue;
                }
                let r: [&[f32]; 8] = std::array::from_fn(|t| {
                    if t < nt {
                        let j = (yi + ia + kk[t]) as usize;
                        &src[j * px + x0..j * px + x1]
                    } else {
                        &src[..0]
                    }
                });
                interior(d, &w, &r, nt);
            } else if method == Method::NN {
                let j = yi + ia;
                if (0..pyi).contains(&j) {
                    let j = j as usize;
                    d.copy_from_slice(&src[j * px + x0..j * px + x1]);
                } else {
                    d.fill(0.0);
                }
            } else {
                for (x, o) in d.iter_mut().enumerate() {
                    *o = edge_sum(
                        kk,
                        |t| w[t][x],
                        yi + ia,
                        |j| (0..pyi).contains(&j).then(|| src[j as usize * px + x0 + x]),
                    );
                }
            }
        }
    });
}

/// A z-shear's rows: the integer offset and the weights (by tap) of every row `(x, y)`.
struct ZRows {
    ia: Vec<i32>,
    w: Vec<f32>,
}

impl ZRows {
    fn new(st: &Stage, method: Method, dims: [usize; 3]) -> ZRows {
        let (px, py, pz) = (dims[0], dims[1], dims[2]);
        let plane = px * py;
        let nt = taps(method).0.len();
        let rows: Vec<(i32, [f32; 8])> = (0..plane)
            .into_par_iter()
            .map(|q| row(method, st.alpha(q % px, q / px), pz))
            .collect();
        let ia = rows.iter().map(|r| r.0).collect();
        let mut w = vec![0f32; nt * plane];
        for (q, r) in rows.iter().enumerate() {
            for t in 0..nt {
                w[t * plane + q] = r.1[t];
            }
        }
        ZRows { ia, w }
    }
}

/// Plane `k` of the z-shear of `src` (a volume of `dims`) into `dst` (a plane).
fn shear_z(method: Method, zr: &ZRows, k: usize, dims: [usize; 3], src: &[f32], dst: &mut [f32]) {
    let (px, py, pz) = (dims[0], dims[1], dims[2]);
    let plane = px * py;
    let (kk, kmin, kmax) = taps(method);
    let nt = kk.len();
    let pzi = pz as i64;
    let ki = k as i64;
    if method == Method::NN {
        for (q, (o, &ia)) in dst[..plane].iter_mut().zip(&zr.ia).enumerate() {
            let j = ki + i64::from(ia);
            *o = if ia != ZERO && (0..pzi).contains(&j) {
                src[j as usize * plane + q]
            } else {
                0.0
            };
        }
        return;
    }
    for y in 0..py {
        let at = y * px;
        runs(&zr.ia[at..at + px], |x0, x1, ia| {
            let d = &mut dst[at + x0..at + x1];
            if ia == ZERO {
                d.fill(0.0);
                return;
            }
            let ia = i64::from(ia);
            let (r0, r1) = (at + x0, at + x1);
            let w: [&[f32]; 8] = std::array::from_fn(|t| {
                if t < nt {
                    &zr.w[t * plane + r0..t * plane + r1]
                } else {
                    &zr.w[..0]
                }
            });
            if ki + ia + kmin >= 0 && ki + ia + kmax <= pzi - 1 {
                if x1 - x0 < SHORT {
                    for (x, o) in d.iter_mut().enumerate() {
                        *o = dot(
                            nt,
                            |t| w[t][x],
                            |t| src[(ki + ia + kk[t]) as usize * plane + r0 + x],
                        );
                    }
                    return;
                }
                let r: [&[f32]; 8] = std::array::from_fn(|t| {
                    if t < nt {
                        let j = (ki + ia + kk[t]) as usize;
                        &src[j * plane + r0..j * plane + r1]
                    } else {
                        &src[..0]
                    }
                });
                interior(d, &w, &r, nt);
            } else if method == Method::NN {
                let j = ki + ia;
                if (0..pzi).contains(&j) {
                    let j = j as usize;
                    d.copy_from_slice(&src[j * plane + r0..j * plane + r1]);
                } else {
                    d.fill(0.0);
                }
            } else {
                for (x, o) in d.iter_mut().enumerate() {
                    *o = edge_sum(
                        kk,
                        |t| w[t][x],
                        ki + ia,
                        |j| {
                            (0..pzi)
                                .contains(&j)
                                .then(|| src[j as usize * plane + r0 + x])
                        },
                    );
                }
            }
        });
    }
}

/// A window of the (unpadded) grid: the output's voxels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub lo: [usize; 3],
    pub n: [usize; 3],
}

impl Window {
    pub fn full(n: [usize; 3]) -> Window {
        Window { lo: [0; 3], n }
    }

    pub fn len(&self) -> usize {
        self.n[0] * self.n[1] * self.n[2]
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The padded (and flipped, §5.2) copy of `vol` (a volume of `n`), on `dims`.
pub(crate) fn pad_into(
    vol: &[f32],
    n: [usize; 3],
    z: usize,
    flip: Option<(usize, usize)>,
    dims: [usize; 3],
    pad: &mut Vec<f32>,
) {
    let (px, py, pz) = (dims[0], dims[1], dims[2]);
    // Every voxel is written below (the data or a zero), so a buffer of the right size is
    // reused as it is.
    if pad.len() != px * py * pz {
        pad.clear();
        pad.resize(px * py * pz, 0.0);
    }
    let rev = |axis: usize, i: usize| match flip {
        Some((a, b)) if axis == a || axis == b => dims[axis] - 1 - i,
        _ => i,
    };
    let flip_x = matches!(flip, Some((a, b)) if a == 0 || b == 0);
    pad.par_chunks_mut(px * py)
        .enumerate()
        .for_each(|(kp, plane)| {
            // Padded plane kp holds the input's plane rev(kp) - z.
            let ks = rev(2, kp);
            if ks < z || ks >= z + n[2] {
                plane.fill(0.0);
                return;
            }
            let k = ks - z;
            for jp in 0..py {
                let js = rev(1, jp);
                let row = &mut plane[px * jp..px * (jp + 1)];
                if js < z || js >= z + n[1] {
                    row.fill(0.0);
                    continue;
                }
                let j = js - z;
                let src = &vol[n[0] * (j + n[1] * k)..][..n[0]];
                row[..z].fill(0.0);
                row[z + n[0]..].fill(0.0);
                if flip_x {
                    for (i, &v) in src.iter().enumerate() {
                        row[px - 1 - (z + i)] = v;
                    }
                } else {
                    row[z..z + n[0]].copy_from_slice(src);
                }
            }
        });
}

/// The reusable volumes of a sweep: the padded input and, for a plan with two z-shears, the
/// first sweep's output.
#[derive(Default)]
pub struct Volumes {
    pub pad: Vec<f32>,
    pub mid: Vec<f32>,
}

/// Applies the plan `p` (with shears) to `vol` (a volume of `n`, padded by `z`) with the
/// polynomial `method` (or NN), writing the window `win` of the result into `out` (its
/// voxels, x fastest).
pub(crate) fn apply(
    method: Method,
    p: &Plan,
    vol: &[f32],
    n: [usize; 3],
    z: usize,
    win: &Window,
    out: &mut [f32],
    bufs: &mut Volumes,
) {
    let shears = p.shears.as_ref().expect("a plan with shears");
    if win.is_empty() {
        return;
    }
    let dims = n.map(|d| d + 2 * z);
    let (px, py) = (dims[0], dims[1]);
    let plane = px * py;
    let stages: Vec<Stage> = shears
        .iter()
        .filter_map(|sh| Stage::new(sh, dims))
        .collect();
    // The groups: an optional z-shear, then the in-plane shears after it.
    let mut groups: Vec<(Option<Stage>, Vec<Stage>)> = Vec::new();
    for st in stages {
        if st.axis == 2 || groups.is_empty() {
            if st.axis == 2 {
                groups.push((Some(st), Vec::new()));
            } else {
                groups.push((None, vec![st]));
            }
        } else {
            groups.last_mut().expect("a group").1.push(st);
        }
    }
    pad_into(vol, n, z, p.flip, dims, &mut bufs.pad);
    let line = px.max(py);
    // The output window, in padded coordinates.
    let wlo = [win.lo[0] + z, win.lo[1] + z, win.lo[2] + z];
    let (wx, wy) = (win.n[0], win.n[1]);
    let write_window = |a: &[f32], dst: &mut [f32]| {
        for jj in 0..wy {
            let at = (wlo[1] + jj) * px + wlo[0];
            dst[jj * wx..(jj + 1) * wx].copy_from_slice(&a[at..at + wx]);
        }
    };
    if groups.is_empty() {
        out.par_chunks_mut(wx * wy)
            .enumerate()
            .for_each(|(kk, dst)| {
                let k = wlo[2] + kk;
                write_window(&bufs.pad[k * plane..(k + 1) * plane], dst);
            });
        return;
    }
    let inplane = |stages: &[Stage], k: usize, pl: &mut Planes| {
        for st in stages {
            match st.axis {
                0 => shear_x(st, method, k, px, &pl.a, &mut pl.b),
                _ => shear_y(st, method, k, dims, &pl.a, &mut pl.b, &mut pl.ia, &mut pl.w),
            }
            std::mem::swap(&mut pl.a, &mut pl.b);
        }
    };
    let last = groups.len() - 1;
    // `src_is_pad`: whether the current source volume is `pad` (else `mid`).
    let mut src_is_pad = true;
    for (g, (zst, ins)) in groups.iter().enumerate() {
        let zrows = zst.as_ref().map(|st| ZRows::new(st, method, dims));
        let fill = |k: usize, src: &[f32], pl: &mut Planes| {
            match &zrows {
                Some(zr) => shear_z(method, zr, k, dims, src, &mut pl.a),
                None => pl.a.copy_from_slice(&src[k * plane..(k + 1) * plane]),
            }
            inplane(ins, k, pl);
        };
        if g == last {
            let src: &[f32] = if src_is_pad { &bufs.pad } else { &bufs.mid };
            out.par_chunks_mut(wx * wy).enumerate().for_each_init(
                || Planes::new(plane, line),
                |pl, (kk, dst)| {
                    let k = wlo[2] + kk;
                    fill(k, src, pl);
                    write_window(&pl.a, dst);
                },
            );
        } else if zrows.is_none() {
            // In place: the in-plane shears of the first group, plane by plane.
            let src: &mut Vec<f32> = if src_is_pad {
                &mut bufs.pad
            } else {
                &mut bufs.mid
            };
            src.par_chunks_mut(plane).enumerate().for_each_init(
                || Planes::new(plane, line),
                |pl, (k, dst)| {
                    pl.a.copy_from_slice(dst);
                    inplane(ins, k, pl);
                    dst.copy_from_slice(&pl.a);
                },
            );
        } else {
            let (src, dst): (&[f32], &mut Vec<f32>) = if src_is_pad {
                (&bufs.pad, &mut bufs.mid)
            } else {
                (&bufs.mid, &mut bufs.pad)
            };
            // Every plane is written below.
            if dst.len() != src.len() {
                dst.clear();
                dst.resize(src.len(), 0.0);
            }
            dst.par_chunks_mut(plane).enumerate().for_each_init(
                || Planes::new(plane, line),
                |pl, (k, d)| {
                    fill(k, src, pl);
                    d.copy_from_slice(&pl.a);
                },
            );
            src_is_pad = !src_is_pad;
        }
    }
}
