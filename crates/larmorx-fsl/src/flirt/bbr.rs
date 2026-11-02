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

/// What the boundary-based cost needs besides the pair's images (§4.1, §6.5).
#[derive(Clone, Debug, Default)]
pub struct BbrSource {
    /// `-wmseg`, scaled voxel sizes, single slices tripled.
    pub wmseg: Option<Volume>,
    /// `-wmcoords` and `-wmnorms` (mm), when given.
    pub coords: Option<(Vec<[f64; 3]>, Vec<[f64; 3]>)>,
    /// `-fieldmap` and its mask, on the reference's grid, and the phase-encoding direction.
    pub fieldmap: Option<(Volume, Volume)>,
    pub pedir: i32,
    pub echo_spacing: f32,
    /// `-bbrtype` as given (checked when the cost is first used).
    pub bbr_type: String,
    pub slope: f32,
}

/// The boundary's point pairs in reference scaled mm: the grey-matter side and the white-matter
/// side of each boundary point.
#[derive(Clone, Debug)]
pub struct BbrPair {
    pub gm: Vec<[f32; 3]>,
    pub wm: Vec<[f32; 3]>,
    pub kind: Kind,
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
        let smooth = crate::betsurf::smooth(seg.data.clone(), seg.shape, vox, 2.0);
        let s = |x: i64, y: i64, z: i64| -> f32 {
            let c = |v: i64, n: usize| v.clamp(0, n as i64 - 1) as usize;
            smooth[c(x, nx) + nx * (c(y, ny) + ny * c(z, nz))]
        };
        // 3 × 3 × 3 derivative masks: the difference along the axis, [1, 2, 1] across it
        let wt = [1.0f32, 2.0, 1.0];
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
                                        let mut w = d[k] as f32;
                                        for (j, &dj) in d.iter().enumerate() {
                                            if j != k {
                                                w *= wt[(dj + 1) as usize];
                                            }
                                        }
                                        *gk += w * v;
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
        for (a, b) in found.into_iter().flatten() {
            gm.push(a);
            wm.push(b);
        }
    }
    Ok(BbrPair { gm, wm, kind })
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
fn fieldmap_value(fmap: &Volume, mask: &Volume, r: [f64; 3], dir: [f64; 3]) -> f32 {
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

/// A field map's shift of the input coordinates (§6.5) for one matrix.
pub struct Shift<'a> {
    fmap: &'a Volume,
    mask: &'a Volume,
    /// The input axis shifted.
    pub axis: usize,
    sigma: f32,
    dir: [f64; 3],
}

impl Shift<'_> {
    /// The shift (input voxels along [`Shift::axis`]) of the reference voxel `r`.
    pub fn at(&self, r: [f64; 3]) -> f64 {
        f64::from(self.sigma * fieldmap_value(self.fmap, self.mask, r, self.dir))
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
        axis: ax,
        sigma,
        dir: [d4[0] / norm, d4[1] / norm, d4[2] / norm],
    })
}

/// The boundary-based cost of `a` (reference mm → input mm through `a⁻¹`, §6.5).
pub fn cost(pair: &Pair, a: &Mat4, log: &Log) -> f32 {
    let Some(src) = pair.bbr_source.as_ref() else {
        log.fatal("BBR cost requested without -wmseg or -wmcoords/-wmnorms");
        return f32::NAN;
    };
    let pts = pair.bbr.get_or_init(|| points(src, pair.reference.voxel_size));
    let pts = match pts {
        Ok(p) => p,
        Err(m) => {
            log.fatal(m.clone());
            return f32::NAN;
        }
    };
    if pts.gm.is_empty() {
        log.push("ERROR::set_bbr_seg: could not find any boundary points!");
        return 0.0f32 / 0.0;
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
    let sample = |p: [f32; 3]| -> (f32, f32) {
        let v = [f64::from(p[0]), f64::from(p[1]), f64::from(p[2]), 1.0];
        let mut q = mat::apply(&t, v);
        if let Some(sh) = &shift {
            let r = mat::apply(&s_ref_inv, v);
            q[sh.axis] += sh.at([r[0], r[1], r[2]]);
        }
        let qf = [q[0] as f32, q[1] as f32, q[2] as f32];
        let val = trilinear_zero(&pair.input, qf);
        let w = if in_w.is_some() || ref_w.is_some() {
            let wt = in_w.map_or(1.0, |w| trilinear_zero(w, qf));
            let r = mat::apply(&s_ref_inv, v);
            let wr = ref_w.map_or(1.0, |w| trilinear_zero(w, [r[0] as f32, r[1] as f32, r[2] as f32]));
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
            let (g, wg) = sample(pts.gm[k]);
            let (w, ww) = sample(pts.wm[k]);
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
