// SPDX-License-Identifier: Apache-2.0
//! A volume moved by a transform array; the MNI and tkregister helpers (spec §63):
//! `LTAtransformAtPoint` (FS-504, FS-886, GLIBC-07 and `errno`'s rules), `LTAtransformInterp` and
//! `LTAtransform` (`ltaxf.rs`), `LTAvoxelTransformToCoronalRasTransform`, `FixMNITal`,
//! `LTAmodifySrcDstGeom` and `DevolveXFMWithSubjectsDir` (`devxfm.rs`).

use larmorx_core::defects::FS_886;
use larmorx_core::math;

use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::maths::{Output, Returned};
use crate::mri::lta::lta_type;
use crate::mri::{Lta, Mri};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// `ERANGE`.
const ERANGE: i32 = 34;

/// A 4 x 4 matrix of these rows (`float`).
pub(crate) fn rows4(v: [[f32; 4]; 4]) -> Matrix {
    let mut m = Matrix::zeros(4, 4, MATRIX_REAL);
    for (r, row) in v.into_iter().enumerate() {
        for (c, x) in row.into_iter().enumerate() {
            m.set(r as i32 + 1, c as i32 + 1, x);
        }
    }
    m
}

impl Process {
    /// `LTAtransformAtPoint(lta, x, y, z, m_L)` (§63): the transforms' matrices weighted by
    /// `exp(-d / (2 s s))` of the point's squared distance `d` to each centre (`s` the spread),
    /// into `m_L` (cleared) when given, else a new 4 x 4 matrix: the first matrix when the weights'
    /// sum is below 1e-15 in magnitude, else the sum of the matrices times their shares, a share
    /// below `0.1 / n` left out. No transform: FS-504; more than 4,096: FS-886 (both modes).
    /// `errno` is `ERANGE` after a weight whose finite argument underflows to 0 or overflows.
    pub fn lta_transform_at_point(
        &mut self,
        lta: &Lta,
        x: f32,
        y: f32,
        z: f32,
        m_l: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        let name = "LTAtransformAtPoint";
        let n = lta.xforms.len();
        if n == 0 {
            self.lta_fs504(name, "an array without transforms")?;
            return Ok(Returned::Nothing);
        }
        if n > 4096 {
            self.note(
                &FS_886,
                format!("{name}: an array of {n} transforms: the program ended (FreeSurfer keeps the weights of 4,096)"),
            );
            self.error_exit(
                codes::BAD_PARM,
                format!("{name}: {n} transforms, more than 4096 (FreeSurfer writes past its weights here)\n")
                    .as_bytes(),
            )?;
            return Ok(Returned::Nothing);
        }
        let mut w = Vec::with_capacity(n);
        let mut total = 0.0f64;
        for lt in &lta.xforms {
            let dx = sse::widen(sse::sub32(lt.x0, x));
            let dy = sse::widen(sse::sub32(lt.y0, y));
            let dz = sse::widen(sse::sub32(lt.z0, z));
            let d = sse::add64(
                sse::add64(sse::mul64(dx, dx), sse::mul64(dy, dy)),
                sse::mul64(dz, dz),
            );
            let s = sse::widen(lt.sigma);
            let a = sse::div64(-d, sse::mul64(sse::mul64(2.0, s), s));
            let e = if a.is_nan() {
                f64::from_bits(a.to_bits() | 0x0008_0000_0000_0000)
            } else {
                self.libm("exp", a, math::exp, math::exp_glibc)
            };
            if a.is_finite() && (e == 0.0 || e.is_infinite()) {
                self.errno = ERANGE;
            }
            total = sse::add64(total, e);
            w.push(e);
        }
        let mut out = Matrix::zeros(4, 4, MATRIX_REAL);
        if total.abs() < 1e-15 {
            let first = lta.xforms[0].m_l.clone();
            return self.at_point_result(first.as_ref(), m_l, true);
        }
        let limit = 0.1 / n as f64;
        for (lt, &wi) in lta.xforms.iter().zip(&w) {
            let share = sse::div64(wi, total);
            if share < limit {
                continue;
            }
            let Some(m) = lt.m_l.as_ref() else {
                continue;
            };
            let f = sse::narrow(share);
            for r in 1..=out.rows {
                for c in 1..=out.cols {
                    let p = sse::mul32(m.at(r, c), f);
                    out.set(r, c, sse::add32(p, out.at(r, c)));
                }
            }
        }
        self.at_point_result(Some(&out), m_l, false)
    }

    /// The point's matrix into the given one (cleared first) or a new one.
    fn at_point_result(
        &mut self,
        m: Option<&Matrix>,
        given: Option<&mut Matrix>,
        copy: bool,
    ) -> Result<Returned<Matrix>, Stop> {
        match given {
            Some(g) => {
                for v in g.data.iter_mut().skip(2) {
                    *v = 0.0;
                }
                if copy {
                    self.matrix_copy(m, Output::Given(g))?;
                } else if let Some(m) = m {
                    for r in 1..=g.rows.min(m.rows) {
                        for c in 1..=g.cols.min(m.cols) {
                            g.set(r, c, m.at(r, c));
                        }
                    }
                }
                Ok(Returned::Output)
            }
            None => {
                if copy {
                    let mut out = Matrix::zeros(4, 4, MATRIX_REAL);
                    self.matrix_copy(m, Output::Given(&mut out))?;
                    Ok(Returned::New(out))
                } else {
                    Ok(Returned::New(m.cloned().unwrap_or_else(|| Matrix::zeros(4, 4, MATRIX_REAL))))
                }
            }
        }
    }

    /// `LTAvoxelTransformToCoronalRasTransform(lta)` (§63): the first matrix `M` becomes `W (M
    /// V)` (`float` products, `V` the world-to-voxel and `W` the voxel-to-world matrices of a 256³
    /// coronal volume), the array's type RAS-to-RAS. Returns 0. No transform: FS-504.
    pub fn lta_voxel_transform_to_coronal_ras_transform(&mut self, lta: &mut Lta) -> Result<i32, Stop> {
        if lta.xforms.is_empty() {
            self.lta_fs504(
                "LTAvoxelTransformToCoronalRasTransform",
                "an array without transforms",
            )?;
            return Ok(0);
        }
        let v = rows4([
            [-1.0, 0.0, 0.0, 128.0],
            [0.0, 0.0, -1.0, 128.0],
            [0.0, 1.0, 0.0, 128.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let w = rows4([
            [-1.0, 0.0, 0.0, 128.0],
            [0.0, 0.0, 1.0, -128.0],
            [0.0, -1.0, 0.0, 128.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let m = lta.xforms[0].m_l.clone();
        let mv = match self.matrix_multiply(m.as_ref(), Some(&v), None)? {
            Returned::New(x) => Some(x),
            _ => None,
        };
        match self.matrix_multiply(Some(&w), mv.as_ref(), lta.xforms[0].m_l.as_mut())? {
            Returned::New(x) => lta.xforms[0].m_l = Some(x),
            Returned::Output => {}
            Returned::Nothing => {}
        }
        lta.kind = lta_type::LINEAR_RAS_TO_RAS;
        Ok(0)
    }

    /// `FixMNITal(x, y, z, &xt, &yt, &zt)` (§63): the point through Talairach's correction of
    /// MNI's brain (the matrix of `z >= 0`, else the other; `float` product). Returns 0.
    pub fn fix_mni_tal(&mut self, x: f32, y: f32, z: f32) -> Result<(i32, [f32; 3]), Stop> {
        let up = z >= 0.0;
        let m = if up {
            rows4([
                [0.9900, 0.0, 0.0, 0.0],
                [0.0, 0.9688, 0.0460, 0.0],
                [0.0, -0.0485, 0.9189, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ])
        } else {
            rows4([
                [0.9900, 0.0, 0.0, 0.0],
                [0.0, 0.9688, 0.0420, 0.0],
                [0.0, -0.0485, 0.8390, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ])
        };
        let mut p = Matrix::zeros(4, 1, MATRIX_REAL);
        for (k, v) in [x, y, z, 1.0].into_iter().enumerate() {
            p.set(k as i32 + 1, 1, v);
        }
        let r = match self.matrix_multiply(Some(&m), Some(&p), None)? {
            Returned::New(x) => x,
            _ => Matrix::zeros(4, 1, MATRIX_REAL),
        };
        Ok((0, [r.at(1, 1), r.at(2, 1), r.at(3, 1)]))
    }

    /// `LTAmodifySrcDstGeom(lta, src, dst)` (§63) at the program's `vg_isEqual_Threshold`
    /// (`threshold`): each transform's source (then destination) geometry takes the volume's
    /// (`getVolGeom`) when it is not valid, or when it differs from it (after the notice and both
    /// geometries printed). Returns 0.
    pub fn lta_modify_src_dst_geom(
        &mut self,
        lta: &mut Lta,
        src: Option<&Mri>,
        dst: Option<&Mri>,
        threshold: f64,
    ) -> Result<i32, Stop> {
        for i in 0..lta.xforms.len() {
            for (m, is_src) in [(src, true), (dst, false)] {
                let Some(m) = m else {
                    continue;
                };
                let mut vg = m.geom.clone();
                self.get_vol_geom(&m.geom, &mut vg);
                let lt = &mut lta.xforms[i];
                let g = if is_src { &mut lt.src } else { &mut lt.dst };
                if g.valid != 0 {
                    if self.vg_is_equal(&vg, g, threshold) == 0 {
                        self.put_err(if is_src {
                            b"INFO: src volume info differs from the one stored in lta. gets modified now.\n"
                        } else {
                            b"INFO: dst volume info differs from the one stored in lta.  gets modified now.\n"
                        });
                        let old = g.clone();
                        self.vg_print(&vg, false);
                        self.vg_print(&old, false);
                        let lt = &mut lta.xforms[i];
                        let g = if is_src { &mut lt.src } else { &mut lt.dst };
                        self.get_vol_geom(&m.geom, g);
                    }
                } else {
                    let lt = &mut lta.xforms[i];
                    let g = if is_src { &mut lt.src } else { &mut lt.dst };
                    self.get_vol_geom(&m.geom, g);
                }
            }
        }
        Ok(0)
    }
}
