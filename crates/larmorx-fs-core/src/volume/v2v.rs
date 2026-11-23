// SPDX-License-Identifier: Apache-2.0
//! Volume-to-volume resampling (spec §41): `MRIvol2Vol`, `MRIvol2VolTLKernel`, `MRIvol2VolVSM`
//! (FS-482), `MRIvol2VolDelta`, `TransformRegDat2LTA` and `MRImaskAndUpsample` (FS-269's box);
//! and §56's `MRIvol2VolTkRegVSM`.

use larmorx_core::defects::FS_482;

use super::sample::sample_type;
use super::{Dst, VolOut};
use crate::clib::printf::sprintf_f64;
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix, real_point_f};
use crate::mri::lta::lta_type;
use crate::mri::{Lta, Mri, types};
use crate::process::{DIAG_VERBOSE, Process, Stop};
use crate::sse;

/// The value of a `Returned` matrix, or none.
fn returned(r: Returned<Matrix>) -> Option<Matrix> {
    match r {
        Returned::New(m) => Some(m),
        _ => None,
    }
}

/// An element of a matrix (1-based), 0 past its size.
fn el(m: &Matrix, i: i32, j: i32) -> f32 {
    if i <= m.rows && j <= m.cols && !m.is_complex() {
        m.at(i, j)
    } else {
        0.0
    }
}

/// The order of a coordinate's three products' sum (§41): `(p1 + p2) + p3`, `p3 + (p2 + p1)`
/// or `p3 + (p1 + p2)`, then `+ m4`.
#[derive(Clone, Copy)]
enum Order {
    Left,
    RightRev,
    Right,
}

/// A target voxel's source point by `M`'s elements in `float`, each row summed in its order.
fn point_by(m: &Matrix, (c, r, s): (i32, i32, i32), orders: [Order; 3]) -> [f32; 3] {
    let (ct, rt, st) = (c as f32, r as f32, s as f32);
    let mut out = [0f32; 3];
    for (k, o) in orders.iter().enumerate() {
        let i = k as i32 + 1;
        let p1 = sse::mul32(el(m, i, 1), ct);
        let p2 = sse::mul32(el(m, i, 2), rt);
        let p3 = sse::mul32(el(m, i, 3), st);
        let sum = match o {
            Order::Left => sse::add32(sse::add32(p1, p2), p3),
            Order::RightRev => sse::add32(p3, sse::add32(p2, p1)),
            Order::Right => sse::add32(p3, sse::add32(p1, p2)),
        };
        out[k] = sse::add32(sum, el(m, i, 4));
    }
    out
}

/// `MRIvol2Vol`'s orders: rows 1 and 2 left to right, row 3 from the right.
const VOL2VOL: [Order; 3] = [Order::Left, Order::Left, Order::RightRev];
/// `MRIvol2VolTLKernel`'s orders: rows 1 and 3 left to right, row 2 from the right.
const KERNEL: [Order; 3] = [Order::Left, Order::Right, Order::Left];

/// The sampler of a resampling (§41) and the values it keeps from voxel to voxel.
struct Sampler {
    interp: i32,
    hw: i32,
    bspline: Option<crate::bspline::BSpline>,
    vals: Vec<f32>,
    sinc: f64,
}

impl Process {
    /// `M` absent (§41): the inverse of the source's scanner voxel-to-RAS matrix times the
    /// target's (a singular source geometry ends the program in the product).
    fn v2v_default(&mut self, src: &Mri, targ: &Mri) -> Result<Option<Matrix>, Stop> {
        let vs = self.vox2ras_scanner(&src.geom, 0, false)?;
        let vt = self.vox2ras_scanner(&targ.geom, 0, false)?;
        let inv = returned(self.matrix_inverse(Some(&vs), None)?);
        Ok(returned(self.matrix_multiply(
            inv.as_ref(),
            Some(&vt),
            None,
        )?))
    }

    /// The given matrix, or the default one; with `Gdiag_no > 0` the diagnostic print.
    fn v2v_matrix(
        &mut self,
        src: &Mri,
        targ: &Mri,
        m: Option<&Matrix>,
    ) -> Result<Option<Matrix>, Stop> {
        let (v, computed) = match m {
            Some(m) => (Some(m.clone()), 0),
            None => (self.v2v_default(src, targ)?, 1),
        };
        if self.gdiag_no > 0 {
            self.put_out(format!("MRIvol2Vol: Vt2s Matrix ({computed})\n").as_bytes());
            self.matrix_print(Some(false), v.as_ref(), 5);
        }
        Ok(v)
    }

    /// A coordinate's nearest voxel: `nint`, or `nint2` for a source one voxel thick.
    fn v2v_nearest(&mut self, x: f32, thin: bool) -> i32 {
        if thin {
            self.nint2(f64::from(x))
        } else {
            self.nint(f64::from(x))
        }
    }

    /// The sampler of §41's resamplings: with the cubic B-spline the source's coefficients
    /// (made now); sinc's half-width `nint(param)`.
    fn v2v_sampler(&mut self, src: &Mri, interp: i32, param: f32) -> Result<Sampler, Stop> {
        let bspline = if interp == sample_type::CUBIC_BSPLINE {
            Some(self.mri_to_bspline(src, None, 3)?)
        } else {
            None
        };
        let hw = if interp == sample_type::SINC {
            self.nint(f64::from(param))
        } else {
            0
        };
        Ok(Sampler {
            interp,
            hw,
            bspline,
            vals: vec![0f32; src.nframes.max(0) as usize],
            sinc: 0.0,
        })
    }

    /// One target voxel's values from the source point (§41), into the target; an unknown
    /// method prints `unknown` and ends the program (status 1).
    #[allow(clippy::too_many_arguments)]
    fn v2v_sample(
        &mut self,
        src: &Mri,
        targ: &mut Mri,
        sm: &mut Sampler,
        p: [f32; 3],
        (ic, ir, is): (i32, i32, i32),
        (c, r, s): (i32, i32, i32),
        unknown: &str,
    ) -> Result<(), Stop> {
        let nf = src.nframes.max(0);
        let (x, y, z) = (f64::from(p[0]), f64::from(p[1]), f64::from(p[2]));
        match sm.interp {
            sample_type::NEAREST => {
                for f in 0..nf {
                    sm.vals[f as usize] = self.vf(src, (ic, ir, is), f)?;
                }
            }
            sample_type::TRILINEAR => {
                let vals = &mut sm.vals;
                self.mri_sample_seq_volume(src, x, y, z, 0, nf - 1, &mut |f, v| {
                    if let Some(slot) = vals.get_mut(f.max(0) as usize) {
                        *slot = v;
                    }
                })?;
            }
            sample_type::SINC => {
                let hw = sm.hw;
                self.mri_sinc_sample_volume(src, x, y, z, hw, &mut sm.sinc)?;
                let v = sse::narrow(sm.sinc);
                sm.vals.iter_mut().for_each(|x| *x = v);
            }
            sample_type::CUBIC_BSPLINE => {
                let b = sm.bspline.take().expect("the coefficients");
                for f in 0..nf {
                    if let (_, Some(v)) = self.mri_sample_bspline(&b, x, y, z, f)? {
                        sm.vals[f as usize] = sse::narrow(v);
                    }
                }
                sm.bspline = Some(b);
            }
            m => {
                self.put_out(
                    format!("ERROR: {unknown}: interpolation method {m} unknown\n").as_bytes(),
                );
                return Err(Stop::Exit(1));
            }
        }
        for f in 0..nf {
            let v = sm.vals[f as usize];
            self.mri_set_vox_val(targ, c, r, s, f, v, 0)?;
        }
        Ok(())
    }

    /// `MRIvol2Vol(src, targ, M, interp, param)` (§41): each target voxel (columns outermost)
    /// from the source point `M (c, r, s, 1)` by the sample type; outside the source the voxel is
    /// left as it was. Frame counts that differ: the error and 1.
    pub fn mri_vol2vol(
        &mut self,
        src: &Mri,
        targ: &mut Mri,
        m: Option<&Matrix>,
        interp: i32,
        param: f32,
    ) -> Result<i32, Stop> {
        if src.nframes != targ.nframes {
            self.put_out(b"ERROR: MRIvol2vol: source and target have different number of frames\n");
            return Ok(1);
        }
        let v2v = self.v2v_matrix(src, targ, m)?;
        let Some(v2v) = v2v else {
            return Ok(1);
        };
        let mut sm = self.v2v_sampler(src, interp, param)?;
        let (sw, sh, sd) = (src.geom.width, src.geom.height, src.geom.depth);
        let thin = sw == 1 || sh == 1 || sd == 1;
        let (tw, th, td) = (targ.geom.width, targ.geom.height, targ.geom.depth);
        for c in 0..tw {
            for r in 0..th {
                for s in 0..td {
                    let p = point_by(&v2v, (c, r, s), VOL2VOL);
                    let ic = self.v2v_nearest(p[0], thin);
                    if ic < 0 || ic >= sw {
                        continue;
                    }
                    let ir = self.v2v_nearest(p[1], thin);
                    if ir < 0 || ir >= sh {
                        continue;
                    }
                    let is = self.v2v_nearest(p[2], thin);
                    if is < 0 || is >= sd {
                        continue;
                    }
                    self.v2v_sample(src, targ, &mut sm, p, (ic, ir, is), (c, r, s), "MRIvol2vol")?;
                }
            }
        }
        Ok(0)
    }

    /// `MRIvol2VolTLKernel(src, targ, M)` (§41): the eight trilinear weights of each target
    /// voxel's source point (inside the source) into its frames 0 to 7; an 8-frame work volume
    /// made and discarded; returns none (always).
    pub fn mri_vol2vol_tl_kernel(
        &mut self,
        src: &Mri,
        targ: &mut Mri,
        m: Option<&Matrix>,
    ) -> Result<Option<Mri>, Stop> {
        let v2v = match m {
            Some(m) => Some(m.clone()),
            None => self.v2v_default(src, targ)?,
        };
        let ext = (targ.geom.width, targ.geom.height, targ.geom.depth);
        let work = self.alloc_with_header(targ, ext, types::FLOAT, 8)?;
        drop(work);
        let Some(v2v) = v2v else {
            return Ok(None);
        };
        let (sw, sh, sd) = (src.geom.width, src.geom.height, src.geom.depth);
        for c in 0..ext.0 {
            for r in 0..ext.1 {
                for s in 0..ext.2 {
                    let p = point_by(&v2v, (c, r, s), KERNEL);
                    let ic = self.nint(f64::from(p[0]));
                    if ic < 0 || ic >= sw {
                        continue;
                    }
                    let ir = self.nint(f64::from(p[1]));
                    if ir < 0 || ir >= sh {
                        continue;
                    }
                    let is = self.nint(f64::from(p[2]));
                    if is < 0 || is >= sd {
                        continue;
                    }
                    let k = self.mri_trilin_kernel(
                        src,
                        f64::from(p[0]),
                        f64::from(p[1]),
                        f64::from(p[2]),
                    );
                    for (f, w) in k.iter().enumerate() {
                        self.mri_set_vox_val(targ, c, r, s, f as i32, sse::narrow(*w), 0)?;
                    }
                }
            }
        }
        Ok(None)
    }

    /// A 4 x 4 matrix times a point as `MatrixMultiply` (single-precision accumulation).
    fn v2v_point(&mut self, m: &Matrix, p: [f32; 4]) -> Result<[f32; 4], Stop> {
        if m.rows == 4 && m.cols == 4 && !m.is_complex() {
            return Ok(real_point_f(m, p));
        }
        let mut q = Matrix::zeros(4, 1, MATRIX_REAL);
        for (i, &x) in p.iter().enumerate() {
            q.set(i as i32 + 1, 1, x);
        }
        let mut o = [0f32; 4];
        if let Some(r) = returned(self.matrix_multiply(Some(m), Some(&q), None)?) {
            for (i, x) in o.iter_mut().enumerate().take(r.rows.max(0) as usize) {
                *x = r.at(i as i32 + 1, 1);
            }
        }
        Ok(o)
    }

    /// `MRIvol2VolVSM(src, targ, M, interp, param, vsm, vsmlta, pedir)` (§41): `MRIvol2Vol` with
    /// a voxel-shift map along the direction `pedir` (FS-482).
    #[allow(clippy::too_many_arguments)]
    pub fn mri_vol2vol_vsm(
        &mut self,
        src: &Mri,
        targ: &mut Mri,
        m: Option<&Matrix>,
        interp: i32,
        param: f32,
        vsm: Option<&Mri>,
        vsmlta: Option<&Lta>,
        pedir: i32,
    ) -> Result<i32, Stop> {
        if self.gdiag & DIAG_VERBOSE != 0 {
            self.put_out(b"Using MRIvol2VolVSM\n");
            let mut b = format!("MRIvol2VolVSM interp={interp}, param=").into_bytes();
            b.extend_from_slice(&sprintf_f64(b"%g", f64::from(param)));
            b.extend_from_slice(format!(", pedir={pedir}\n").as_bytes());
            self.put_out(&b);
        }
        if src.nframes != targ.nframes {
            self.put_out(
                b"ERROR: MRIvol2volVSM(): source and target have different number of frames\n",
            );
            return Ok(1);
        }
        if vsm.is_some() && !(1..=3).contains(&pedir.wrapping_abs()) {
            self.put_out(
                format!("ERROR: MRIvol2volVSM: pedir={pedir}, must be +/-1, +/-2, +/-3\n")
                    .as_bytes(),
            );
            return Err(Stop::Exit(1));
        }
        let map_v2v = match (vsm, vsmlta) {
            (Some(_), Some(l)) => {
                let mut c = match self.lta_copy(l, None)? {
                    Returned::New(c) => c,
                    _ => return Ok(1),
                };
                self.lta_change_type(&mut c, lta_type::LINEAR_VOX_TO_VOX)?;
                c.xforms.first().and_then(|x| x.m_l.clone())
            }
            _ => None,
        };
        let with_lta = vsm.is_some() && vsmlta.is_some();
        let v2v = self.v2v_matrix(src, targ, m)?;
        let Some(v2v) = v2v else {
            return Ok(1);
        };
        let mut sm = self.v2v_sampler(src, interp, param)?;
        let (sw, sh, sd) = (src.geom.width, src.geom.height, src.geom.depth);
        let (tw, th, td) = (targ.geom.width, targ.geom.height, targ.geom.depth);
        let axis = (pedir.wrapping_abs() - 1).clamp(0, 2) as usize;
        let sign = if pedir < 0 { -1.0f32 } else { 1.0 };
        let mut shift = 0f32;
        let mut shift_written = false;
        for c in 0..tw {
            for r in 0..th {
                for s in 0..td {
                    let q = self.v2v_point(&v2v, [c as f32, r as f32, s as f32, 1.0])?;
                    let mut p = [q[0], q[1], q[2]];
                    let ic = self.nint(f64::from(p[0]));
                    let ir = self.nint(f64::from(p[1]));
                    let is = self.nint(f64::from(p[2]));
                    if ic < 0 || ic >= sw || ir < 0 || ir >= sh || is < 0 || is >= sd {
                        continue;
                    }
                    let mut at = (ic, ir, is);
                    if let Some(map) = vsm {
                        let cv = super::sample::cvtt(f64::from(p[0].floor()));
                        let rv = super::sample::cvtt(f64::from(p[1].floor()));
                        if cv < 0 || cv.wrapping_add(1) >= sw || rv < 0 || rv.wrapping_add(1) >= sh
                        {
                            continue;
                        }
                        let mut flat = false;
                        for (dc, dr) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                            let v = self.vf(map, (cv + dc, rv + dr, is), 0)?;
                            if v.abs() < f32::MIN_POSITIVE {
                                flat = true;
                                break;
                            }
                        }
                        if flat {
                            continue;
                        }
                        let mut got = None;
                        let (x, y, z) = (f64::from(p[0]), f64::from(p[1]), f64::from(p[2]));
                        self.mri_sample_seq_volume(map, x, y, z, 0, 0, &mut |_, v| got = Some(v))?;
                        match got {
                            Some(v) => {
                                shift = v;
                                shift_written = true;
                            }
                            None if !shift_written => {
                                self.note(
                                    &FS_482,
                                    "MRIvol2VolVSM: a shift the map's sampler never wrote taken as 0 (FreeSurfer: a value never written)",
                                );
                                shift = 0.0;
                            }
                            None => {}
                        }
                        if shift == 0.0 {
                            continue;
                        }
                        p[axis] = sse::add32(p[axis], sse::mul32(shift, sign));
                        if with_lta {
                            let Some(mv) = map_v2v.as_ref() else {
                                let z = Matrix::zeros(4, 1, MATRIX_REAL);
                                self.matrix_multiply_d(None, Some(&z), None)?;
                                return Ok(1);
                            };
                            let mut col = Matrix::zeros(4, 1, MATRIX_REAL);
                            for (i, &x) in [p[0], p[1], p[2], 1.0].iter().enumerate() {
                                col.set(i as i32 + 1, 1, x);
                            }
                            if let Some(o) =
                                returned(self.matrix_multiply_d(Some(mv), Some(&col), None)?)
                            {
                                p = [o.at(1, 1), o.at(2, 1), o.at(3, 1)];
                            }
                        }
                        let ic = self.nint(f64::from(p[0]));
                        if ic < 0 || ic >= sw {
                            continue;
                        }
                        let ir = self.nint(f64::from(p[1]));
                        if ir < 0 || ir >= sh {
                            continue;
                        }
                        let is = self.nint(f64::from(p[2]));
                        if is < 0 || is >= sd {
                            continue;
                        }
                        at = (ic, ir, is);
                    }
                    self.v2v_sample(src, targ, &mut sm, p, at, (c, r, s), "MRIvol2volVSM")?;
                }
            }
        }
        Ok(0)
    }

    /// `MRIvol2VolTkRegVSM(mov, targ, R, interp, param, vsm, vsmlta, pedir)` (§56):
    /// `MRIvol2VolVSM` with the voxel matrix `(inv(Tmov) R) Ttarg` (the tkregister matrices), or
    /// none without `R`.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_vol2vol_tkreg_vsm(
        &mut self,
        mov: &Mri,
        targ: &mut Mri,
        reg: Option<&Matrix>,
        interp: i32,
        param: f32,
        vsm: Option<&Mri>,
        vsmlta: Option<&Lta>,
        pedir: i32,
    ) -> Result<i32, Stop> {
        let v2v = match reg {
            Some(r) => {
                let tmov = self.vox2ras_tkreg(&mov.geom)?;
                let ttarg = self.vox2ras_tkreg(&targ.geom)?;
                let inv = returned(self.matrix_inverse(Some(&tmov), None)?);
                let a = returned(self.matrix_multiply(inv.as_ref(), Some(r), None)?);
                let m = returned(self.matrix_multiply(a.as_ref(), Some(&ttarg), None)?);
                if m.is_none() {
                    return Ok(1);
                }
                m
            }
            None => None,
        };
        self.mri_vol2vol_vsm(mov, targ, v2v.as_ref(), interp, param, vsm, vsmlta, pedir)
    }

    /// `MRIvol2VolDelta(mov, targ, R)` (§41): a 3-frame `float` volume of the target's extent
    /// and header holding `T x - (R T) x` per voxel (`T` the target's tkregister matrix; `R`
    /// absent: `MRItkRegMtx(targ, mov)`).
    pub fn mri_vol2vol_delta(
        &mut self,
        mov: &Mri,
        targ: &Mri,
        reg: Option<&Matrix>,
    ) -> Result<Option<Mri>, Stop> {
        let ext = (targ.geom.width, targ.geom.height, targ.geom.depth);
        let mut out = self.alloc_with_header(targ, ext, types::FLOAT, 3)?;
        let r = match reg {
            Some(r) => Some(r.clone()),
            None => self.tkreg_from_native(&targ.geom, &mov.geom, None)?,
        };
        let t = self.vox2ras_tkreg(&targ.geom)?;
        let t2 = returned(self.matrix_multiply(r.as_ref(), Some(&t), None)?);
        let Some(t2) = t2 else {
            return Ok(None);
        };
        for c in 0..ext.0 {
            for rr in 0..ext.1 {
                for s in 0..ext.2 {
                    let x = [c as f32, rr as f32, s as f32, 1.0];
                    let a = self.v2v_point(&t, x)?;
                    let b = self.v2v_point(&t2, x)?;
                    for k in 0..3 {
                        self.mri_set_vox_val(
                            &mut out,
                            c,
                            rr,
                            s,
                            k as i32,
                            sse::sub32(a[k], b[k]),
                            0,
                        )?;
                    }
                }
            }
        }
        Ok(Some(out))
    }

    /// `TransformRegDat2LTA(targ, mov, R)` (§41): a one-transform `LINEAR_VOX_TO_VOX` array from
    /// the target's voxels to the moving volume's, `(inv(Tmov) R) Ttarg` (the scanner matrices
    /// and the identity without `R`, the tkregister ones with it).
    pub fn transform_regdat2lta(
        &mut self,
        targ: &Mri,
        mov: &Mri,
        reg: Option<&Matrix>,
    ) -> Result<Lta, Stop> {
        let mut lta = self.lta_alloc(1, None)?;
        lta.kind = lta_type::LINEAR_VOX_TO_VOX;
        let (ttarg, tmov, r) = match reg {
            None => (
                self.vox2ras_scanner(&targ.geom, 0, false)?,
                self.vox2ras_scanner(&mov.geom, 0, false)?,
                returned(self.matrix_identity(4, None)?),
            ),
            Some(r) => (
                self.vox2ras_tkreg(&targ.geom)?,
                self.vox2ras_tkreg(&mov.geom)?,
                Some(r.clone()),
            ),
        };
        let inv = returned(self.matrix_inverse(Some(&tmov), None)?);
        let a = returned(self.matrix_multiply(inv.as_ref(), r.as_ref(), None)?);
        let m = returned(self.matrix_multiply(a.as_ref(), Some(&ttarg), None)?);
        let mut src = lta.xforms[0].src.clone();
        let mut dst = lta.xforms[0].dst.clone();
        self.get_vol_geom(&targ.geom, &mut src);
        self.get_vol_geom(&mov.geom, &mut dst);
        let x = &mut lta.xforms[0];
        x.kind = lta_type::LINEAR_VOX_TO_VOX;
        x.src = src;
        x.dst = dst;
        x.m_l = m;
        Ok(lta)
    }

    /// `MRImaskAndUpsample(src, mask, factor, npad, conserve, &lta)` (§41): the box of the mask
    /// (or the source) padded by `npad` (§43, FS-269), cut out of the source and, with a factor
    /// above 1, upsampled; the array from the source's voxels to the result's.
    pub fn mri_mask_and_upsample(
        &mut self,
        src: &mut Mri,
        mask: Option<&Mri>,
        factor: i32,
        npad: i32,
        conserve: bool,
    ) -> Result<Option<(Mri, Lta)>, Stop> {
        let b = match mask {
            Some(m) => self.region_get_bounding_box(m, npad)?,
            None => self.region_get_bounding_box(src, npad)?,
        };
        if self.gdiag_no > 0 {
            let n = src
                .geom
                .width
                .wrapping_mul(src.geom.height)
                .wrapping_mul(src.geom.depth);
            let k = b.dx.wrapping_mul(b.dy).wrapping_mul(b.dz);
            let ratio = sse::div32(n as f32, k as f32);
            let mut line = b" mask-and-upsample bounding box ".to_vec();
            line.extend_from_slice(&sprintf_f64(b"%g", sse::widen(ratio)));
            line.push(b' ');
            line.extend_from_slice(&super::region::region_line(&b));
            self.put_out(&line);
        }
        let mut cut = match self.mri_extract_region(src, Dst::New, Some(&b))? {
            VolOut::New(m) => m,
            _ => return Ok(None),
        };
        let out = if factor > 1 {
            let r = if conserve {
                self.mri_upsample_n_conserve(&mut cut, Dst::New, factor)?
            } else {
                self.mri_upsample_n(&mut cut, Dst::New, factor)?
            };
            match r {
                VolOut::New(m) => m,
                _ => return Ok(None),
            }
        } else {
            cut
        };
        let lta = self.transform_regdat2lta(src, &out, None)?;
        Ok(Some((out, lta)))
    }
}
