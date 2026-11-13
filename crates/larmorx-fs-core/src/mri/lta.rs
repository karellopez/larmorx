// SPDX-License-Identifier: Apache-2.0
//! The bounding box (spec §17), the transform type numbers (§18) and the linear transform arrays
//! (§19-22): allocation, the copies (FS-195, FS-196, FS-198) and freeing (FS-197).

use larmorx_core::defects::{FS_195, FS_196, FS_197, FS_198};

use super::geom::VolGeom;
use super::types;
use super::volume::{ENOMEM, Mri, Region};
use super::voxels::Place;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::maths::{Output, Returned};
use crate::process::{Process, Stop, codes};

/// `TransformType` (§18).
pub mod transform_type {
    pub const MINC: i32 = 0;
    pub const TKREG: i32 = 1;
    pub const GENERIC: i32 = 2;
    pub const UNKNOWN: i32 = -1;
}

/// The linear transform array's types (§18).
pub mod lta_type {
    pub const LINEAR_VOX_TO_VOX: i32 = 0;
    pub const LINEAR_RAS_TO_RAS: i32 = 1;
    pub const LINEAR_PHYSVOX_TO_PHYSVOX: i32 = 2;
    pub const TRANSFORM_ARRAY_TYPE: i32 = 10;
    pub const MORPH_3D_TYPE: i32 = 11;
    pub const MNI_TRANSFORM_TYPE: i32 = 12;
    pub const MATLAB_ASCII_TYPE: i32 = 13;
    pub const REGISTER_DAT: i32 = 14;
    pub const FSLREG_TYPE: i32 = 15;
    pub const LINEAR_CORONAL_RAS_TO_CORONAL_RAS: i32 = 21;
}

/// A linear transform (§19).
#[derive(Clone, Debug, PartialEq)]
pub struct LinearTransform {
    pub x0: f32,
    pub y0: f32,
    pub z0: f32,
    pub sigma: f32,
    /// The transform.
    pub m_l: Option<Matrix>,
    /// The gradient.
    pub m_dl: Option<Matrix>,
    /// The last step.
    pub m_last_dl: Option<Matrix>,
    pub kind: i32,
    pub src: VolGeom,
    pub dst: VolGeom,
    pub label: i32,
}

/// A linear transform array (`LTA`, §19).
#[derive(Clone, Debug, PartialEq)]
pub struct Lta {
    pub xforms: Vec<LinearTransform>,
    pub inv_xforms: Vec<LinearTransform>,
    pub kind: i32,
    pub fscale: f32,
    /// The subject's name (at most 1,023 bytes).
    pub subject: Vec<u8>,
}

impl Lta {
    /// `num_xforms`.
    pub fn num_xforms(&self) -> i32 {
        self.xforms.len() as i32
    }
}

/// `initVolGeom` of a zeroed geometry (§20): not valid, 256 x 256 x 256 voxels of 1 mm, the
/// coronal cosines, centre and shears 0, `ras_good_flag` 0, the name `unknown`, no matrices.
pub fn init_vol_geom() -> VolGeom {
    VolGeom {
        width: 256,
        height: 256,
        depth: 256,
        fname: b"unknown".to_vec(),
        ..VolGeom::default()
    }
}

fn identity4() -> Matrix {
    let mut m = Matrix::zeros(4, 4, MATRIX_REAL);
    for k in 1..=4 {
        m.set(k, k, 1.0);
    }
    m
}

impl Process {
    /// `MRIboundingBox(mri, thresh, &box)` (§17): the box of the first frame's voxels above
    /// `thresh`; 0, or `ERROR_UNSUPPORTED` for another type.
    pub fn mri_bounding_box(&mut self, m: &Mri, thresh: i32, b: &mut Region) -> Result<i32, Stop> {
        let g = &m.geom;
        let (w, h, d) = (g.width, g.height, g.depth);
        b.x = w - 1;
        b.y = h - 1;
        b.z = d - 1;
        b.dx = w;
        b.dy = h;
        b.dz = d;
        if !matches!(
            m.kind,
            types::UCHAR | types::SHORT | types::USHRT | types::INT | types::FLOAT
        ) {
            self.error_printf(
                codes::UNSUPPORTED,
                format!("MRIboundingBox: unsupported type {}", m.kind).as_bytes(),
            );
            return Ok(codes::UNSUPPORTED);
        }
        if !m.has_voxels() && h > 0 && d > 0 {
            self.no_voxels("MRIboundingBox")?;
            return Ok(0);
        }
        let (mut xmax, mut ymax, mut zmax) = (0, 0, 0);
        let tf = thresh as f32;
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let Place::In(o) = m.flat_place(x, y, z, 0) else {
                        continue;
                    };
                    let v = m.voxels.as_ref().expect("voxels");
                    let o = o as usize;
                    let above = match m.kind {
                        types::UCHAR => i32::from(v[o]) > thresh,
                        types::SHORT => i32::from(i16::from_le_bytes([v[o], v[o + 1]])) > thresh,
                        types::USHRT => i32::from(u16::from_le_bytes([v[o], v[o + 1]])) > thresh,
                        types::INT => {
                            i32::from_le_bytes(v[o..o + 4].try_into().expect("4")) > thresh
                        }
                        _ => f32::from_le_bytes(v[o..o + 4].try_into().expect("4")) > tf,
                    };
                    if above {
                        b.x = b.x.min(x);
                        b.y = b.y.min(y);
                        b.z = b.z.min(z);
                        xmax = xmax.max(x);
                        ymax = ymax.max(y);
                        zmax = zmax.max(z);
                    }
                }
            }
        }
        b.dx = xmax - b.x + 1;
        b.dy = ymax - b.y + 1;
        b.dz = zmax - b.z + 1;
        Ok(0)
    }

    /// `LTAalloc(n, mri)` (§20): `n` transforms and `n` inverse transforms centred on the
    /// volume's bounding box (threshold 70) or the origin; a negative count ends the program.
    pub fn lta_alloc(&mut self, n: i32, mri: Option<&Mri>) -> Result<Lta, Stop> {
        let mut centre = [0.0f32; 3];
        if let Some(m) = mri {
            let mut b = Region::default();
            self.mri_bounding_box(m, 70, &mut b)?;
            centre = [
                (b.x + b.dx / 2) as f32,
                (b.y + b.dy / 2) as f32,
                (b.z + b.dz / 2) as f32,
            ];
        }
        if n < 0 {
            self.errno = ENOMEM;
            self.error_exit(
                codes::NO_MEMORY,
                format!("LTAalloc({n}): could not allocate xforms").as_bytes(),
            )?;
        }
        let lt = LinearTransform {
            x0: centre[0],
            y0: centre[1],
            z0: centre[2],
            sigma: 10000.0,
            m_l: Some(identity4()),
            m_dl: Some(Matrix::zeros(4, 4, MATRIX_REAL)),
            m_last_dl: Some(Matrix::zeros(4, 4, MATRIX_REAL)),
            kind: transform_type::UNKNOWN,
            src: init_vol_geom(),
            dst: init_vol_geom(),
            label: 0,
        };
        let n = n.max(0) as usize;
        Ok(Lta {
            xforms: vec![lt.clone(); n],
            inv_xforms: vec![lt; n],
            kind: 0,
            fscale: 0.0,
            subject: Vec::new(),
        })
    }

    /// One matrix of a transform's copy (§21: FreeSurfer's matrix copy, FS-196, FS-198).
    fn lt_copy_matrix(&mut self, src: &Option<Matrix>, dst: &mut Option<Matrix>) {
        let reproduce = self.mode().reproduce();
        match (src, dst.as_mut()) {
            (Some(s), Some(d)) => {
                let fits = d.rows >= s.rows && d.cols >= s.cols;
                if fits {
                    let _ = self.matrix_copy(Some(s), Output::Given(d));
                } else if reproduce {
                    self.note(
                        &FS_196,
                        "a smaller destination matrix kept its old numbers (FreeSurfer's copy)",
                    );
                } else {
                    self.note(&FS_196, "the source's matrix copied over a smaller one");
                    *dst = Some(s.clone());
                }
            }
            (Some(s), None) => {
                if reproduce {
                    self.note(&FS_196, "a missing destination matrix stayed missing (FreeSurfer's copy)");
                } else {
                    self.note(&FS_196, "the source's matrix copied into a missing one");
                    *dst = Some(s.clone());
                }
            }
            (None, Some(_)) => {
                self.note(
                    &FS_198,
                    "the destination's matrix dropped (FreeSurfer frees it yet still shows it)",
                );
                *dst = None;
            }
            (None, None) => {}
        }
    }

    /// `LTcopy(lt, dst)` (§21): the centre, sigma, the three matrices and the two geometries
    /// into `dst`; FS-195 for the type and label. No destination: the error line and none.
    pub fn lt_copy(
        &mut self,
        lt: &LinearTransform,
        dst: Option<&mut LinearTransform>,
    ) -> Option<()> {
        let Some(d) = dst else {
            self.put_out(b"ERROR: LTcopy() destination LT cannot be NULL\n");
            return None;
        };
        d.x0 = lt.x0;
        d.y0 = lt.y0;
        d.z0 = lt.z0;
        d.sigma = lt.sigma;
        self.lt_copy_matrix(&lt.m_l, &mut d.m_l);
        self.lt_copy_matrix(&lt.m_dl, &mut d.m_dl);
        self.lt_copy_matrix(&lt.m_last_dl, &mut d.m_last_dl);
        self.vg_assign(&mut d.src, &lt.src);
        self.vg_assign(&mut d.dst, &lt.dst);
        if d.kind != lt.kind || d.label != lt.label {
            if self.mode().reproduce() {
                self.note(
                    &FS_195,
                    "the copy kept the destination's own type and label",
                );
            } else {
                self.note(&FS_195, "the transform's type and label copied");
                d.kind = lt.kind;
                d.label = lt.label;
            }
        }
        Some(())
    }

    /// `LTAcopy(lta, dst)` (§21): into a given array of the same count, or a new one.
    pub fn lta_copy(&mut self, lta: &Lta, dst: Option<&mut Lta>) -> Result<Returned<Lta>, Stop> {
        let mut fresh = None;
        let given = dst.is_some();
        let d: &mut Lta = match dst {
            Some(d) => d,
            None => fresh.insert(self.lta_alloc(lta.num_xforms(), None)?),
        };
        if d.num_xforms() != lta.num_xforms() {
            self.put_out(
                format!(
                    "LTAcopy(): ERROR: number of xforms does not match ({},{})\n",
                    lta.num_xforms(),
                    d.num_xforms()
                )
                .as_bytes(),
            );
            return Ok(Returned::Nothing);
        }
        d.kind = lta.kind;
        d.fscale = lta.fscale;
        let end = lta.subject.iter().position(|&b| b == 0).unwrap_or(lta.subject.len());
        d.subject = lta.subject[..end].to_vec();
        for i in 0..lta.xforms.len() {
            self.lt_copy(&lta.xforms[i], Some(&mut d.xforms[i]));
            self.lt_copy(&lta.inv_xforms[i], Some(&mut d.inv_xforms[i]));
        }
        Ok(match fresh {
            Some(m) => Returned::New(m),
            None if given => Returned::Output,
            None => Returned::Nothing,
        })
    }

    /// `LTAfree(&lta)` (§22): 0, the caller left without it; FS-197 for no array (nothing, 0).
    pub fn lta_free(&mut self, lta: &mut Option<Lta>) -> i32 {
        if lta.take().is_none() {
            self.note(
                &FS_197,
                "freeing no transform array: nothing done (FreeSurfer crashes)",
            );
        }
        0
    }
}
