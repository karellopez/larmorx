// SPDX-License-Identifier: Apache-2.0
//! The boundary-based cost (`specs/flirt.md` §6.5; Greve and Fischl 2009, "Accurate and robust
//! brain image alignment using boundary-based registration"): pairs of points on either side of
//! the white-matter boundary, the input's contrast across each pair, and a field map's shift.

use larmorx_core::math;
use larmorx_mri::hmc::volume::Volume;
use rayon::prelude::*;

use super::cost::{Log, Pair};
use super::mat::{self, Mat4};
use super::pyramid;

/// Boundary coordinates and their normals (`-wmcoords`, `-wmnorms`, mm).
pub type Coords = (Vec<[f64; 3]>, Vec<[f64; 3]>);

/// The field map's values at the grey- and white-matter points inside its mask.
pub type Inside = (Vec<Option<f32>>, Vec<Option<f32>>);

/// What the boundary-based cost needs besides the pair's images (§4.1, §6.5).
#[derive(Clone, Debug, Default)]
pub struct BbrSource {
    /// `-wmseg`, scaled voxel sizes, single slices tripled.
    pub wmseg: Option<Volume>,
    /// `-wmcoords` and `-wmnorms` (mm), when given.
    pub coords: Option<Coords>,
    /// `-fieldmap` and its mask, on the reference's grid, and the phase-encoding direction.
    pub fieldmap: Option<(Volume, Volume)>,
    pub pedir: i32,
    pub echo_spacing: f32,
    /// `-bbrtype` as given (checked when the cost is first used).
    pub bbr_type: String,
    pub slope: f32,
    /// The field map mask's distance map ([`mask_distance`]), made when first needed.
    pub mask_distance: std::sync::OnceLock<Vec<u16>>,
}

/// The boundary's point pairs in reference scaled mm: the grey-matter side and the white-matter
/// side of each boundary point.
#[derive(Clone, Debug)]
pub struct BbrPair {
    pub gm: Vec<[f32; 3]>,
    pub wm: Vec<[f32; 3]>,
    pub kind: Kind,
    /// The field map's value at each point whose reference voxel lies inside the field map's
    /// mask (fixed for the pair: only the points outside it search along the direction).
    pub fmap_inside: std::sync::OnceLock<Inside>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Signed,
    GlobalAbs,
    LocalAbs,
}

/// The message of an unknown `-bbrtype` (FSL's exception, §2.2).
pub fn bad_type_message(t: &str) -> String {
    format!(
        "Image Exception : #30 :: Unrecognised BBR type: {t}\nValid types are: signed, global_abs, local_abs\nUnrecognised BBR type: {t}\nValid types are: signed, global_abs, local_abs"
    )
}

/// The boundary pairs of `src` for a pair whose reference level has voxel sizes `ref_voxel`
/// (§6.5). `Err` for an unknown BBR type (the message).
pub fn points(src: &BbrSource, ref_voxel: [f32; 3]) -> Result<BbrPair, String> {
    let kind = match src.bbr_type.as_str() {
        "signed" => Kind::Signed,
        "global_abs" => Kind::GlobalAbs,
        "local_abs" => Kind::LocalAbs,
        other => return Err(bad_type_message(other)),
    };
    let (mut gm, mut wm) = (Vec::new(), Vec::new());
    if let Some((c, n)) = &src.coords {
        for (p, q) in c.iter().zip(n) {
            gm.push([0, 1, 2].map(|k| (p[k] + q[k] * 2.0) as f32));
            wm.push([0, 1, 2].map(|k| (p[k] - q[k] * 2.0) as f32));
        }
    } else if let Some(seg) = &src.wmseg {
        let [nx, ny, nz] = seg.shape;
        let sum = pyramid::box3(seg);
        let vox: [f64; 3] = seg.voxel_size.map(f64::from);
        // the segmentation smoothed by a Gaussian of σ = 2 mm (§6.5)
        let taps = [0, 1, 2].map(|k| gaussian(seg.voxel_size[k]));
        let smooth = pyramid::convolve(seg, &taps).data;
        let s = |x: i64, y: i64, z: i64| -> f32 {
            if x < 0 || y < 0 || z < 0 || x >= nx as i64 || y >= ny as i64 || z >= nz as i64 {
                return 0.0;
            }
            smooth[x as usize + nx * (y as usize + ny * z as usize)]
        };
        // the gradient's mask weights: dx·3^(1 − |dy| − |dz|) and its exchanges (single)
        let weight = |d: [i64; 3], k: usize| -> f32 {
            let others: i64 = (0..3).filter(|&j| j != k).map(|j| d[j].abs()).sum();
            (d[k] as f64 * 3f64.powi((1 - others) as i32)) as f32
        };
        let found: Vec<Vec<([f32; 3], [f32; 3])>> = (0..nz)
            .into_par_iter()
            .map(|z| {
                let mut out = Vec::new();
                for y in 0..ny {
                    for x in 0..nx {
                        let i = x + nx * (y + ny * z);
                        if !(seg.data[i] > 0.5 && sum.data[i] < 26.5) {
                            continue;
                        }
                        let (xi, yi, zi) = (x as i64, y as i64, z as i64);
                        let mut g = [0.0f32; 3];
                        for dz in -1i64..=1 {
                            for dy in -1i64..=1 {
                                for dx in -1i64..=1 {
                                    let v = s(xi + dx, yi + dy, zi + dz);
                                    let d = [dx, dy, dz];
                                    for (k, gk) in g.iter_mut().enumerate() {
                                        *gk += v * weight(d, k);
                                    }
                                }
                            }
                        }
                        // FSL-67 kept (rule C): divided by the reference level's voxel sizes
                        let n = [0, 1, 2].map(|k| -g[k] / ref_voxel[k]);
                        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                        let n = n.map(|v| v / len);
                        let p = [x as f64 * vox[0], y as f64 * vox[1], z as f64 * vox[2]];
                        let side = |sgn: f64| {
                            [0, 1, 2].map(|k| (p[k] + sgn * f64::from(n[k] * 2.0)) as f32)
                        };
                        out.push((side(1.0), side(-1.0)));
                    }
                }
                out
            })
            .collect();
        let all: Vec<([f32; 3], [f32; 3])> = found.into_iter().flatten().collect();
        for (a, b) in all {
            gm.push(a);
            wm.push(b);
        }
    }
    Ok(BbrPair {
        gm,
        wm,
        kind,
        fmap_inside: std::sync::OnceLock::new(),
    })
}

/// The taps of BBR's Gaussian of σ = 2 mm along an axis of voxel size `d` (§6.5): `σ = 2/d`
/// voxels (single), radius `2·trunc(σ − 0.001) + 3`, `exp(−j²/((2σ)·σ))` rounded to single and
/// summed in single in `j` order, each applied as `g_j·(1/Σ)` in double.
fn gaussian(d: f32) -> Vec<f64> {
    let sigma = 2.0f32 / d;
    let r = 2 * (f64::from(sigma) - 0.001).trunc() as i64 + 3;
    let den = (2.0 * f64::from(sigma)) * f64::from(sigma);
    let g: Vec<f32> = (-r..=r)
        .map(|j| larmorx_core::math::exp(-((j * j) as f64) / den) as f32)
        .collect();
    let mut sum = 0.0f32;
    for &v in &g {
        sum += v;
    }
    let inv = 1.0 / f64::from(sum);
    g.iter().map(|&v| f64::from(v) * inv).collect()
}

/// Trilinear interpolation with neighbours outside the volume 0 (any position).
fn trilinear_zero(v: &Volume, p: [f32; 3]) -> f32 {
    if !p.iter().all(|c| c.is_finite()) {
        return 0.0;
    }
    let [nx, ny, nz] = v.shape.map(|n| n as i64);
    let f = p.map(|c| c.floor());
    let (ix, iy, iz) = (f[0] as i64, f[1] as i64, f[2] as i64);
    let (fx, fy, fz) = (p[0] - f[0], p[1] - f[1], p[2] - f[2]);
    let at = |x: i64, y: i64, z: i64| -> f32 {
        if x < 0 || y < 0 || z < 0 || x >= nx || y >= ny || z >= nz {
            0.0
        } else {
            v.data[(x + nx * (y + ny * z)) as usize]
        }
    };
    let c = [0i64, 1].map(|c| [0i64, 1].map(|b| [0i64, 1].map(|a| at(ix + a, iy + b, iz + c))));
    larmorx_mri::hmc::interp::blend(c, fx, fy, fz)
}

/// The field map's value for a reference point `r` (reference voxels) along the
/// phase-encoding direction `dir` (§6.5).
fn fieldmap_value(
    fmap: &Volume,
    mask: &Volume,
    distance: &[u16],
    r: [f64; 3],
    dir: [f64; 3],
) -> f32 {
    let at = |q: [f64; 3]| q.map(|c| c as f32);
    if trilinear_zero(mask, at(r)) >= 0.95 {
        return trilinear_zero(fmap, at(r));
    }
    let extent = (0..3)
        .map(|k| fmap.shape[k] as f32 * fmap.voxel_size[k])
        .fold(0.0f32, f32::max) as i64;
    // the steps that cannot reach the mask are skipped, each side on its own (the same first
    // point found, sooner): from a point `q` of one side, a point `k` steps further
    // (`|dir| <= 1` voxel per step) has its eight neighbours within `k + 3` voxels (Chebyshev,
    // single precision's rounding included) of `q`'s voxel, and its interpolated mask exceeds
    // 0.95 only if one of them exceeds 0.9; a side that has left the grid moving outwards stays
    // outside (every neighbour 0)
    let n = mask.shape;
    let dist_at = |q: [f64; 3]| -> Option<u16> {
        if (0..3).all(|k| q[k] >= 0.0 && q[k] < n[k] as f64) {
            let v = q[0] as usize + n[0] * (q[1] as usize + n[1] * q[2] as usize);
            distance.get(v).copied()
        } else {
            None
        }
    };
    let gone = |q: [f64; 3], sgn: f64| {
        (0..3).any(|k| {
            let step = sgn * dir[k];
            (q[k] < -2.0 && step <= 0.0) || (q[k] > n[k] as f64 + 1.0 && step >= 0.0)
        })
    };
    let mut next = [0i64; 2];
    let mut dead = [false; 2];
    for d in 0..=extent {
        for (side, sgn) in [-1.0, 1.0].into_iter().enumerate() {
            if dead[side] || d < next[side] {
                continue;
            }
            let q = [0, 1, 2].map(|k| r[k] + sgn * d as f64 * dir[k]);
            match dist_at(q) {
                Some(u16::MAX) => {
                    dead[side] = true;
                    continue;
                }
                Some(c) if c > 4 => {
                    next[side] = d + i64::from(c) - 3;
                    continue;
                }
                Some(_) => {}
                None if gone(q, sgn) => {
                    dead[side] = true;
                    continue;
                }
                None => {}
            }
            if trilinear_zero(mask, at(q)) > 0.95 {
                return trilinear_zero(fmap, at(q));
            }
        }
        if dead[0] && dead[1] {
            break;
        }
    }
    0.0
}

/// The Chebyshev distance (voxels) from each voxel of `mask` to the nearest voxel above 0.9
/// (`u16::MAX` when there is none), by two passes over the 26 neighbours (exact for this
/// distance).
pub fn mask_distance(mask: &Volume) -> Vec<u16> {
    let [nx, ny, nz] = mask.shape;
    let far = u16::MAX;
    let mut d: Vec<u16> = mask
        .data
        .iter()
        .map(|&m| if m > 0.9 { 0 } else { far })
        .collect();
    let idx = |x: usize, y: usize, z: usize| x + nx * (y + ny * z);
    let pass = |d: &mut Vec<u16>, forward: bool| {
        for zi in 0..nz {
            let z = if forward { zi } else { nz - 1 - zi };
            for yi in 0..ny {
                let y = if forward { yi } else { ny - 1 - yi };
                for xi in 0..nx {
                    let x = if forward { xi } else { nx - 1 - xi };
                    let mut best = d[idx(x, y, z)];
                    if best == 0 {
                        continue;
                    }
                    for dz in -1i64..=1 {
                        for dy in -1i64..=1 {
                            for dx in -1i64..=1 {
                                // the neighbours already visited in this pass's order
                                let key = (dz, dy, dx);
                                let before = if forward {
                                    key < (0, 0, 0)
                                } else {
                                    key > (0, 0, 0)
                                };
                                if !before {
                                    continue;
                                }
                                let (a, b, c) = (x as i64 + dx, y as i64 + dy, z as i64 + dz);
                                if a < 0
                                    || b < 0
                                    || c < 0
                                    || a >= nx as i64
                                    || b >= ny as i64
                                    || c >= nz as i64
                                {
                                    continue;
                                }
                                let n = d[idx(a as usize, b as usize, c as usize)];
                                if n != far {
                                    best = best.min(n.saturating_add(1));
                                }
                            }
                        }
                    }
                    d[idx(x, y, z)] = best;
                }
            }
        }
    };
    pass(&mut d, true);
    pass(&mut d, false);
    d
}

/// A field map's shift of the input coordinates (§6.5) for one matrix.
pub struct Shift<'a> {
    fmap: &'a Volume,
    mask: &'a Volume,
    distance: &'a [u16],
    /// The input axis shifted.
    pub axis: usize,
    sigma: f32,
    dir: [f64; 3],
}

impl Shift<'_> {
    /// The shift (input voxels along [`Shift::axis`]) of the reference voxel `r`.
    pub fn at(&self, r: [f64; 3]) -> f64 {
        f64::from(self.sigma * fieldmap_value(self.fmap, self.mask, self.distance, r, self.dir))
    }

    /// The field map's value at `r` when `r` lies inside the mask (no search needed).
    fn inside(&self, r: [f64; 3]) -> Option<f32> {
        let q = r.map(|c| c as f32);
        (trilinear_zero(self.mask, q) >= 0.95).then(|| trilinear_zero(self.fmap, q))
    }

    /// The shift with the field map's value known.
    fn of_value(&self, v: f32) -> f64 {
        f64::from(self.sigma * v)
    }
}

/// The shift of matrix `a` (input mm → reference mm) for an input of `input_shape` and a
/// reference of voxel sizes `ref_voxel`; `None` without a field map.
pub fn field_shift<'a>(
    src: &'a BbrSource,
    a: &Mat4,
    input_shape: [usize; 3],
    ref_voxel: [f32; 3],
) -> Option<Shift<'a>> {
    let (fmap, mask) = src.fieldmap.as_ref().filter(|_| src.pedir != 0)?;
    let ax = src.pedir.unsigned_abs() as usize - 1;
    let n = input_shape[ax] as f32;
    let mut sigma = src.echo_spacing * n / (2.0 * std::f32::consts::PI);
    if src.pedir < 0 {
        sigma = -sigma;
    }
    let s_ref_inv = mat::diagonal(ref_voxel.map(|d| 1.0 / f64::from(d)));
    let mut e = [0.0, 0.0, 0.0, 1.0];
    e[ax] = 1.0;
    let d4 = mat::apply(&mat::mul(&s_ref_inv, a), e);
    let norm = (d4[0] * d4[0] + d4[1] * d4[1] + d4[2] * d4[2] + d4[3] * d4[3]).sqrt();
    Some(Shift {
        fmap,
        mask,
        distance: src.mask_distance.get_or_init(|| mask_distance(mask)),
        axis: ax,
        sigma,
        dir: [d4[0] / norm, d4[1] / norm, d4[2] / norm],
    })
}

/// The boundary-based cost of `a` (reference mm → input mm through `a⁻¹`, §6.5).
pub fn cost(pair: &Pair, a: &Mat4, log: &Log) -> f32 {
    cost_and_debug(pair, a, log, None)
}

/// [`cost`], and with `debug` the volume `-debug` saves (§9.4): two volumes on the reference
/// level's grid, zero but where each boundary pair used writes, at the rounded midpoint of its
/// two points (reference voxels), `w·u + (1 − w)` in the first and `w` in the second (`u` the
/// pair's term, `w` its weight, single precision; later pairs over earlier ones; a midpoint
/// outside the grid writes nothing).
pub fn cost_and_debug(pair: &Pair, a: &Mat4, log: &Log, debug: Option<&mut Vec<f32>>) -> f32 {
    let Some(src) = pair.bbr_source.as_ref() else {
        log.fatal("BBR cost requested without -wmseg or -wmcoords/-wmnorms");
        return f32::NAN;
    };
    let pts = pair
        .bbr
        .get_or_init(|| points(src, pair.reference.voxel_size));
    let pts = match pts {
        Ok(p) => p,
        Err(m) => {
            log.fatal(m.clone());
            return f32::NAN;
        }
    };
    if pts.gm.is_empty() {
        log.push("ERROR::set_bbr_seg: could not find any boundary points!");
        // 0 / 0: the default NaN (its sign set, printed -nan)
        return -f32::NAN;
    }
    let Some(ainv) = mat::inverse(a) else {
        log.fatal(mat::SINGULAR);
        return f32::NAN;
    };
    let s_in = mat::diagonal(pair.input.voxel_size.map(f64::from));
    let Some(t) = mat::solve(&s_in, &ainv) else {
        log.fatal(mat::SINGULAR);
        return f32::NAN;
    };
    let s_ref_inv = mat::diagonal(pair.reference.voxel_size.map(|d| 1.0 / f64::from(d)));
    let shift = field_shift(src, a, pair.input.shape, pair.reference.voxel_size);
    let slope = f64::from(src.slope);
    let local = pts.kind == Kind::LocalAbs;
    let step = pair.bbr_step();
    if step < 1 {
        log.fatal(
            "larmorx: setoption bbrstep below 1 with the boundary-based cost: FSL's loop over the boundary points never ends (FSL-71); refused",
        );
        return f32::NAN;
    }
    let in_w = pair.in_weight.as_ref();
    let ref_w = pair.ref_weight.as_ref();
    let inside = shift.as_ref().map(|sh| {
        pts.fmap_inside.get_or_init(|| {
            let at = |ps: &[[f32; 3]]| -> Vec<Option<f32>> {
                ps.par_iter()
                    .map(|p| {
                        let v = [f64::from(p[0]), f64::from(p[1]), f64::from(p[2]), 1.0];
                        let r = mat::apply(&s_ref_inv, v);
                        sh.inside([r[0], r[1], r[2]])
                    })
                    .collect()
            };
            (at(&pts.gm), at(&pts.wm))
        })
    });
    let sample = |p: [f32; 3], known: Option<f32>| -> (f32, f32) {
        let v = [f64::from(p[0]), f64::from(p[1]), f64::from(p[2]), 1.0];
        let mut q = mat::apply(&t, v);
        if let Some(sh) = &shift {
            q[sh.axis] += match known {
                Some(value) => sh.of_value(value),
                None => {
                    let r = mat::apply(&s_ref_inv, v);
                    sh.at([r[0], r[1], r[2]])
                }
            };
        }
        let qf = [q[0] as f32, q[1] as f32, q[2] as f32];
        let val = trilinear_zero(&pair.input, qf);
        let w = if in_w.is_some() || ref_w.is_some() {
            let wt = in_w.map_or(1.0, |w| trilinear_zero(w, qf));
            let r = mat::apply(&s_ref_inv, v);
            let wr = ref_w.map_or(1.0, |w| {
                trilinear_zero(w, [r[0] as f32, r[1] as f32, r[2] as f32])
            });
            wt * wr
        } else {
            1.0
        };
        (val, w)
    };
    let weighted = in_w.is_some() || ref_w.is_some();
    let indices: Vec<usize> = (0..pts.gm.len()).step_by(step).collect();
    let terms: Vec<(f64, f64)> = indices
        .par_iter()
        .with_min_len(256)
        .map(|&k| {
            let (kg, kw) = inside.map_or((None, None), |(a, b)| (a[k], b[k]));
            let (g, wg) = sample(pts.gm[k], kg);
            let (w, ww) = sample(pts.wm[k], kw);
            let (g, w) = (f64::from(g), f64::from(w));
            let q = if (g + w).abs() <= 1e-6 {
                0.0
            } else {
                200.0 * (g - w) / (g + w)
            };
            let mut u = 1.0 + math::tanh_glibc(slope * q);
            if local {
                u = u.min(2.0 - u);
            }
            let w12 = if weighted {
                f64::from(wg) * f64::from(ww)
            } else {
                1.0
            };
            (u, w12)
        })
        .collect();
    if let Some(out) = debug {
        let [nx, ny, nz] = pair.reference.shape;
        let per = nx * ny * nz;
        out.clear();
        out.resize(2 * per, 0.0);
        let vs = pair.reference.voxel_size;
        for (j, &k) in indices.iter().enumerate() {
            let (g, w) = (pts.gm[k], pts.wm[k]);
            let mut ix = [0i64; 3];
            for d in 0..3 {
                let mid = (f64::from(g[d]) + f64::from(w[d])) / 2.0 / f64::from(vs[d]);
                ix[d] = (mid + 0.5).floor() as i64;
            }
            if ix
                .iter()
                .zip([nx, ny, nz])
                .any(|(&i, n)| i < 0 || i >= n as i64)
            {
                continue;
            }
            let at = ix[0] as usize + nx * (ix[1] as usize + ny * ix[2] as usize);
            let (u, w12) = terms[j];
            let (u, w12) = (u as f32, w12 as f32);
            out[at] = w12 * u + (1.0 - w12);
            out[per + at] = w12;
        }
    }
    let (mut num, mut den) = (0.0f64, 0.0f64);
    let mut nan_at = None;
    for (i, &(u, w12)) in terms.iter().enumerate() {
        num += u * w12;
        den += w12;
        if num.is_nan() && nan_at.is_none() {
            nan_at = Some(i);
        }
    }
    if let Some(i) = nan_at {
        for _ in i..terms.len() {
            log.push("WARNING:: Found NaN in BBR cost");
        }
        log.push("WARNING:: Found NaN in BBR cost (2)");
    }
    let mut c = num / den;
    if pts.kind == Kind::GlobalAbs {
        c = c.min(2.0 - c);
    }
    c as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The field map's search as §6.5 states it, every step checked.
    fn plain(fmap: &Volume, mask: &Volume, r: [f64; 3], dir: [f64; 3]) -> f32 {
        let at = |q: [f64; 3]| q.map(|c| c as f32);
        if trilinear_zero(mask, at(r)) >= 0.95 {
            return trilinear_zero(fmap, at(r));
        }
        let extent = (0..3)
            .map(|k| fmap.shape[k] as f32 * fmap.voxel_size[k])
            .fold(0.0f32, f32::max) as i64;
        for d in 0..=extent {
            for sgn in [-1.0, 1.0] {
                let q = [0, 1, 2].map(|k| r[k] + sgn * d as f64 * dir[k]);
                if trilinear_zero(mask, at(q)) > 0.95 {
                    return trilinear_zero(fmap, at(q));
                }
            }
        }
        0.0
    }

    #[test]
    fn the_skipping_search_finds_the_same_point() {
        // a mask of two blobs and scattered voxels, a field map of distinct values
        let shape = [23, 19, 17];
        let n: usize = shape.iter().product();
        let mut seed = 12345u64;
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as u32
        };
        let mut m = vec![0.0f32; n];
        for (i, v) in m.iter_mut().enumerate() {
            let (x, y, z) = (i % 23, (i / 23) % 19, i / (23 * 19));
            let blob = (x as i64 - 6).pow(2) + (y as i64 - 5).pow(2) + (z as i64 - 8).pow(2) < 12
                || (x as i64 - 17).pow(2) + (y as i64 - 13).pow(2) + (z as i64 - 4).pow(2) < 6;
            *v = if blob || next() % 97 == 0 {
                1.0
            } else if next() % 13 == 0 {
                0.93
            } else {
                0.0
            };
        }
        let mask = Volume::new(shape, [1.0; 3], m);
        let fmap = Volume::new(
            shape,
            [1.0; 3],
            (0..n).map(|i| i as f32 * 0.5 - 30.0).collect(),
        );
        let empty = Volume::new(shape, [1.0; 3], vec![0.0; n]);
        let dist = mask_distance(&mask);
        let none = mask_distance(&empty);
        for t in 0..3000 {
            let r = [0, 1, 2].map(|k| f64::from(next() % 3000) / 100.0 - 3.0 + 0.0 * k as f64);
            let a = f64::from(next() % 1000) / 1000.0 - 0.5;
            let b = f64::from(next() % 1000) / 1000.0 - 0.5;
            let c = if t % 2 == 0 { 1.0 } else { 0.1 };
            let norm = (a * a + b * b + c * c + 1.0).sqrt();
            let dir = [a / norm, c / norm, b / norm];
            assert_eq!(
                fieldmap_value(&fmap, &mask, &dist, r, dir).to_bits(),
                plain(&fmap, &mask, r, dir).to_bits(),
                "{r:?} {dir:?}"
            );
            assert_eq!(fieldmap_value(&fmap, &empty, &none, r, dir), 0.0);
        }
    }
}
