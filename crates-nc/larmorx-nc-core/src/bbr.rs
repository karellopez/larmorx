// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-newimage 2601.0 (costfns.{h,cc}: Costfn::set_bbr_seg,
// set_bbr_coords, set_bbr_type, set_bbr_fmap, fmap_extrap, vox_coord_calc, bbr, bbr_resamp).
// Copyright (C) 1999-2023 University of Oxford (Mark Jenkinson et al.). Rust translation
// Copyright 2026 Karel Lopez Vilaret; modified from the original. Distributed under the FSL
// Licence (see LICENSE).
//! FLIRT's boundary-based registration cost (`-cost bbr`, Greve and Fischl's BBR as FSL computes
//! it): pairs of points 2 mm either side of the white-matter boundary of the reference, mapped
//! into the test volume through the affine matrix (and, optionally, a field map along the phase
//! encoding direction); the cost is the mean of `1 + tanh(slope (q - offset))` over the pairs,
//! `q = 200 (g - w) / (g + w)` the percent contrast of the test volume's values there.
//!
//! The boundary points come from a white-matter segmentation (`-wmseg`: voxels above 0.5 with
//! fewer than 27 such voxels in their 3x3x3 neighbourhood, the normal from the gradient of the
//! segmentation smoothed by a 2 mm Gaussian) or from coordinates and normals given as text
//! matrices (`-wmcoords`, `-wmnorms`).

use crate::arma::{self, Mat};
use crate::cnum;
use crate::fdlibm;
use crate::newimagefns::{convolve_separable, gradient_components, smooth};
use crate::volume::{FslError, Volume};

/// `bbr_type`: `signed` (the default), `global_abs`, `local_abs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BbrType {
    Signed,
    GlobalAbs,
    LocalAbs,
}

impl BbrType {
    /// `set_bbr_type(typenm)`; FSL's exception (`imthrow`, number 30) for any other name.
    pub fn parse(name: &str) -> Result<BbrType, FslError> {
        match name {
            "signed" => Ok(BbrType::Signed),
            "local_abs" => Ok(BbrType::LocalAbs),
            "global_abs" => Ok(BbrType::GlobalAbs),
            _ => Err(FslError::imthrow(
                format!(
                    "Unrecognised BBR type: {name}\nValid types are: signed, global_abs, local_abs"
                ),
                30,
            )),
        }
    }
}

/// The boundary points, in the reference's (FLIRT) millimetres: `gm[k]` 2 mm outside the white
/// matter along the normal, `wm[k]` 2 mm inside.
#[derive(Clone, Debug, Default)]
pub struct BbrPoints {
    pub gm: [Vec<f32>; 3],
    pub wm: [Vec<f32>; 3],
}

impl BbrPoints {
    /// `no_coords`.
    pub fn len(&self) -> usize {
        self.gm[0].len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// `bbr_dist`: the half distance of a pair, in mm.
pub const BBR_DIST: f32 = 2.0;

/// `set_bbr_seg(bbr_seg)`: the boundary voxels of the segmentation (above 0.5, and the 3x3x3 box
/// sum of the segmentation below 26.5) in z, y, x order, with their normals: minus the gradient
/// of `smooth(wmseg, 2.0)`, each component divided by the reference's voxel size along it
/// (`refdims`: the cost's reference, which is not the segmentation's grid when FLIRT resampled
/// it), normalised (`sqrtf` of the float sum of squares). `Err` holds FSL's message when there
/// is no boundary voxel (`ERROR::set_bbr_seg: could not find any boundary points!`).
pub fn points_from_seg(wmseg: &Volume, refdims: [f32; 3], threads: usize) -> (BbrPoints, bool) {
    let box3 = [1.0f64, 1.0, 1.0];
    let wmsum = convolve_separable(wmseg, &box3, &box3, &box3, threads);
    let wmsmooth = smooth(wmseg, 2.0, threads);
    let grad = gradient_components(&wmsmooth);
    let samp = wmseg.sampling_mat();
    let mut pts = BbrPoints::default();
    for z in 0..wmseg.zsize() {
        for y in 0..wmseg.ysize() {
            for x in 0..wmseg.xsize() {
                if f64::from(wmseg.value(x, y, z)) > 0.5 && f64::from(wmsum.value(x, y, z)) < 26.5 {
                    let coord =
                        arma::gemv_tinysq(&samp, &[x as f64, y as f64, z as f64, 1.0], false);
                    let k = grad[0].idx(x, y, z);
                    let mut normx = -grad[0].data[k] / refdims[0];
                    let mut normy = -grad[1].data[k] / refdims[1];
                    let mut normz = -grad[2].data[k] / refdims[2];
                    let normnorm = (normx * normx + normy * normy + normz * normz).sqrt();
                    normx /= normnorm;
                    normy /= normnorm;
                    normz /= normnorm;
                    let n = [normx, normy, normz];
                    for d in 0..3 {
                        pts.gm[d].push((coord[d] + f64::from(n[d] * BBR_DIST)) as f32);
                        pts.wm[d].push((coord[d] - f64::from(n[d] * BBR_DIST)) as f32);
                    }
                }
            }
        }
    }
    let found = !pts.is_empty();
    (pts, found)
}

/// `set_bbr_coords(coords, norms)`: the points from given coordinates and normals (rows of
/// `x y z`, millimetres). `None` when they are empty or of different lengths (FSL's
/// `ERROR::set_bbr_coords: coords and norms are different sizes or zero size`).
pub fn points_from_coords(coords: &Mat, norms: &Mat) -> Option<BbrPoints> {
    if coords.rows() == 0 || norms.rows() == 0 || coords.rows() != norms.rows() {
        return None;
    }
    let mut pts = BbrPoints::default();
    for r in 0..coords.rows() {
        for d in 0..3 {
            let c = coords.at(r, d);
            let n = norms.at(r, d);
            // coords(npts+1,1) + norms(npts+1,1) * bbr_dist: doubles (bbr_dist promoted).
            pts.gm[d].push((c + n * f64::from(BBR_DIST)) as f32);
            pts.wm[d].push((c - n * f64::from(BBR_DIST)) as f32);
        }
    }
    Some(pts)
}

/// The field map of a BBR cost (`set_bbr_fmap`): the map in rad/s on the reference's grid, its
/// mask (ones when none is given), the phase-encoding direction (`1/2/3`, negative for the
/// reverse; 0: no field map).
#[derive(Clone, Debug)]
pub struct FieldMap {
    pub fmap: Volume,
    pub mask: Volume,
    pub pe_dir: i32,
}

/// The settings of a BBR cost besides its points.
#[derive(Clone, Debug)]
pub struct BbrSettings {
    pub offset: f32,
    pub slope: f32,
    pub kind: BbrType,
    /// `vertex_step`: every `step`-th pair is used (`setoption bbrstep`).
    pub step: i32,
    pub fieldmap: Option<FieldMap>,
}

impl Default for BbrSettings {
    fn default() -> BbrSettings {
        BbrSettings {
            offset: 0.0,
            slope: -0.5,
            kind: BbrType::Signed,
            step: 1,
            fieldmap: None,
        }
    }
}

/// `in_bounds(x, y, z)` for floating coordinates: `floor` of each at least 0 and `ceil` below the
/// size.
fn in_bounds_f(v: &Volume, x: f32, y: f32, z: f32) -> bool {
    x.floor() >= 0.0
        && y.floor() >= 0.0
        && z.floor() >= 0.0
        && x.ceil() < v.xsize() as f32
        && y.ceil() < v.ysize() as f32
        && z.ceil() < v.zsize() as f32
}

/// `fmap_extrap(x, y, z, v_pe)`: the field map at the nearest point along the phase-encoding
/// direction (in 1-voxel steps, both ways) where the mask is above 0.95; 0 if none within the
/// map's largest extent in mm.
fn fmap_extrap(fm: &FieldMap, x: f64, y: f64, z: f64, v_pe: &[f64; 4]) -> f32 {
    let f = &fm.fmap;
    let size = |n: i64, d: f32| f64::from(n as f32 * d);
    let a = size(f.xsize(), f.xdim());
    let b = size(f.ysize(), f.ydim());
    let c = size(f.zsize(), f.zdim());
    let fmap_max_size = if a > b { a } else { b };
    let fmap_max_size = if fmap_max_size > c { fmap_max_size } else { c };
    let mut dist = 0.0f64;
    while dist <= fmap_max_size {
        for dir in [-1.0f64, 1.0] {
            let x_ref = (x + dir * dist * v_pe[0]) as f32;
            let y_ref = (y + dir * dist * v_pe[1]) as f32;
            let z_ref = (z + dir * dist * v_pe[2]) as f32;
            if in_bounds_f(&fm.mask, x_ref, y_ref, z_ref)
                && fm.mask.interpolate(x_ref, y_ref, z_ref) > 0.95
            {
                return f.interpolate(x_ref, y_ref, z_ref);
            }
        }
        dist += 1.0;
    }
    0.0
}

/// The geometry of one cost evaluation.
struct Frame {
    iaffbig: Mat,
    mm2vox: Mat,
    pe_dir_vec: [f64; 4],
}

impl Frame {
    /// `iaffbig = testvol.sampling_mat().i() * aff.i()` (`solve(Stest, inv(aff))`), `mm2vox =
    /// refvol.sampling_mat().i()`, and the unit phase-encoding direction in the reference's
    /// voxels (`refvol.sampling_mat().i() * aff * [0 1 0 1]`, divided by its norm, the fourth
    /// component included).
    fn new(refvol: &Volume, testvol: &Volume, aff: &Mat) -> Frame {
        let nan4 = || Mat::from_rows(4, 4, &[f64::NAN; 16]);
        let ia = arma::inv(aff).unwrap_or_else(|_| nan4());
        let iaffbig = arma::solve(&testvol.sampling_mat(), &ia).unwrap_or_else(|_| nan4());
        let sref = refvol.sampling_mat();
        let mm2vox = arma::inv(&sref).unwrap_or_else(|_| nan4());
        let v = arma::gemv_tinysq(aff, &[0.0, 1.0, 0.0, 1.0], false);
        let v = arma::solve(&sref, &Mat::col(&v)).unwrap_or_else(|_| Mat::col(&[f64::NAN; 4]));
        let n = crate::miscmaths::norm2(&v);
        let pe_dir_vec = [v.get(0) / n, v.get(1) / n, v.get(2) / n, v.get(3) / n];
        Frame {
            iaffbig,
            mm2vox,
            pe_dir_vec,
        }
    }

    /// `vox_coord_calc(tvc, rvc, ...)`: the test voxel of a reference point in mm, and the point
    /// in the reference's voxels; with a field map, the test coordinate along the phase-encoding
    /// axis shifted by `nonlin * fmap` (the map extrapolated along the direction where its mask is
    /// below 0.95).
    fn vox(&self, rvc_mm: [f64; 4], nonlin: f64, fm: Option<&FieldMap>) -> ([f64; 4], [f64; 4]) {
        let tv = arma::gemv_tinysq(&self.iaffbig, &rvc_mm, false);
        let rv = arma::gemv_tinysq(&self.mm2vox, &rvc_mm, false);
        let mut tvc = [tv[0], tv[1], tv[2], tv[3]];
        let rvc = [rv[0], rv[1], rv[2], rv[3]];
        if let Some(fm) = fm.filter(|f| f.pe_dir != 0) {
            let (rx, ry, rz) = (rvc[0] as f32, rvc[1] as f32, rvc[2] as f32);
            let axis = (fm.pe_dir.unsigned_abs() - 1) as usize;
            let shift = if fm.mask.interpolate(rx, ry, rz) < 0.95 {
                fmap_extrap(fm, rvc[0], rvc[1], rvc[2], &self.pe_dir_vec)
            } else {
                fm.fmap.interpolate(rx, ry, rz)
            };
            tvc[axis] += nonlin * f64::from(shift);
        }
        (tvc, rvc)
    }
}

/// What one cost evaluation printed (`WARNING:: Found NaN in BBR cost`, once per pair after the
/// sum became NaN, and `(2)` after the division).
#[derive(Clone, Debug, Default)]
pub struct BbrMessages {
    pub nan_pairs: u64,
    pub nan_final: bool,
}

/// `bbr(aff, nonlin_params)`: the cost over the pairs `0, step, 2 step, ...`, test values by the
/// test volume's interpolation; with `weights` (`rweight`, `tweight`), each pair weighted by the
/// product of the weights at its two points. `nonlin` is `nonlin_params(1)` (1 without a field
/// map).
#[allow(clippy::too_many_arguments)]
pub fn bbr_cost(
    refvol: &Volume,
    testvol: &Volume,
    weights: Option<(&Volume, &Volume)>,
    aff: &Mat,
    nonlin: f64,
    pts: &BbrPoints,
    s: &BbrSettings,
    msgs: &mut BbrMessages,
) -> f32 {
    let fr = Frame::new(refvol, testvol, aff);
    let fm = s.fieldmap.as_ref();
    let (mut bbrval, mut bbrwsum) = (0.0f64, 0.0f64);
    let n = pts.len() as i64;
    let step = i64::from(s.step);
    let mut row: i64 = 0;
    while row < n {
        let r = row as usize;
        let p1 = [
            f64::from(pts.gm[0][r]),
            f64::from(pts.gm[1][r]),
            f64::from(pts.gm[2][r]),
            1.0,
        ];
        let p2 = [
            f64::from(pts.wm[0][r]),
            f64::from(pts.wm[1][r]),
            f64::from(pts.wm[2][r]),
            1.0,
        ];
        let (tvc1, rvc1) = fr.vox(p1, nonlin, fm);
        let (tvc2, rvc2) = fr.vox(p2, nonlin, fm);
        let gv = f64::from(testvol.interpolate(tvc1[0] as f32, tvc1[1] as f32, tvc1[2] as f32));
        let wv = f64::from(testvol.interpolate(tvc2[0] as f32, tvc2[1] as f32, tvc2[2] as f32));
        let w12 = match weights {
            Some((rweight, tweight)) => {
                let w1 = tweight.interpolate(tvc1[0] as f32, tvc1[1] as f32, tvc1[2] as f32)
                    * rweight.interpolate(rvc1[0] as f32, rvc1[1] as f32, rvc1[2] as f32);
                let w2 = tweight.interpolate(tvc2[0] as f32, tvc2[1] as f32, tvc2[2] as f32)
                    * rweight.interpolate(rvc2[0] as f32, rvc2[1] as f32, rvc2[2] as f32);
                f64::from(w1) * f64::from(w2)
            }
            None => 1.0,
        };
        let qv = if (gv + wv).abs() > 1e-6 {
            200.0 * (gv - wv) / (gv + wv)
        } else {
            0.0
        };
        let update = 1.0 + fdlibm::tanh(f64::from(s.slope) * (qv - f64::from(s.offset)));
        if s.kind == BbrType::LocalAbs {
            let other = 2.0 - update;
            bbrval += w12 * if update < other { update } else { other };
        } else {
            bbrval += w12 * update;
        }
        bbrwsum += w12;
        if bbrval.is_nan() {
            msgs.nan_pairs += 1;
        }
        // row += vertex_step, as int: a step of 0 never ends in FSL (refused before, by the
        // caller); a negative one wraps through the int range (here: stops).
        row = match row.checked_add(step) {
            Some(r) if step > 0 => r,
            _ => n,
        };
    }
    // SSE's NaN: no points (0 / 0) gives the default NaN, `-nan`; the compiler would otherwise
    // fold the empty loop's 0.0 / 0.0 into a positive NaN (the trial of 2026-11-01).
    bbrval = crate::sse::div_f64(bbrval, bbrwsum);
    if s.kind == BbrType::GlobalAbs {
        let other = 2.0 - bbrval;
        bbrval = if bbrval < other { bbrval } else { other };
    }
    if bbrval.is_nan() {
        msgs.nan_final = true;
    }
    bbrval as f32
}

/// `bbr_resamp(aff, nonlin_params, resampvol)`: the test volume on the reference's grid through the
/// same mapping as the cost (the field map's shift included), by its interpolation; the
/// reference's voxels visited x, then y, then z innermost, each at `(x xdim, y ydim, z zdim)` mm.
pub fn bbr_resamp(
    refvol: &Volume,
    testvol: &Volume,
    aff: &Mat,
    nonlin: f64,
    s: &BbrSettings,
) -> Volume {
    let fr = Frame::new(refvol, testvol, aff);
    let fm = s.fieldmap.as_ref();
    let mut out = refvol.clone();
    for refx in 0..refvol.xsize() {
        for refy in 0..refvol.ysize() {
            for refz in 0..refvol.zsize() {
                let rvc = [
                    f64::from(refx as f32 * refvol.xdim()),
                    f64::from(refy as f32 * refvol.ydim()),
                    f64::from(refz as f32 * refvol.zdim()),
                    1.0,
                ];
                let (tvc, _) = fr.vox(rvc, nonlin, fm);
                let v = testvol.interpolate(tvc[0] as f32, tvc[1] as f32, tvc[2] as f32);
                out.set(refx, refy, refz, v);
            }
        }
    }
    out
}

/// `MISCMATHS::round` of a non-negative coordinate, for the debug volume (not used by the cost).
#[allow(dead_code)]
fn round_index(x: f64) -> i64 {
    i64::from(cnum::f64_to_i32(if x > 0.0 { x + 0.5 } else { x - 0.5 }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_boundary_points_give_sses_default_nan() {
        // FLIRT prints this cost as `-nan` (0 / 0 on SSE); a folded 0.0 / 0.0 would be `nan`.
        let mut v = Volume::new(4, 4, 4, 1);
        v.data.fill(1.0);
        let mut msgs = BbrMessages::default();
        let c = bbr_cost(
            &v,
            &v,
            None,
            &Mat::eye(4),
            1.0,
            &BbrPoints::default(),
            &BbrSettings::default(),
            &mut msgs,
        );
        assert_eq!(c.to_bits(), crate::sse::DEFAULT_NAN_F32.to_bits());
        assert!(msgs.nan_final);
    }
}
