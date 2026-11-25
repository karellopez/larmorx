// SPDX-License-Identifier: Apache-2.0
//! Conversions between voxel and RAS transform arrays and their composition (spec §67):
//! `LTAvoxelToRasXform` (FS-955 kept), `LTArasToVoxelXform` (FS-954, FS-957) and `LTAcompose`
//! (FS-956, FS-504). `LTAinverseTransformInterp` is in `ltaxf.rs`.

use larmorx_core::defects::{FS_954, FS_956, FS_957};

use crate::maths::Returned;
use crate::maths::matrix::Matrix;
use crate::mri::lta::lta_type;
use crate::mri::{Lta, Mri};
use crate::process::{Process, Stop, codes};

/// Where `LTAcompose` puts its result (§67): a new array, another array, or the source itself.
pub enum ComposeInto<'a> {
    New,
    Given(&'a mut Lta),
    Itself,
}

/// What `LTAcompose` returned: the new array, or the given destination (the other array or the
/// source).
pub enum Composed {
    New(Lta),
    Given,
}

impl Process {
    /// `LTAvoxelToRasXform(lta, src, dst)` (§67): each transform's matrix (as voxel to voxel)
    /// replaced by its RAS form, with a source volume `MRIvoxelXformToRasXform(src, dst, M)`,
    /// else from the transform's own geometries `ras2vox(D) (M vox2ras(S))` (FS-955: the other
    /// direction, kept in both modes); the array's type RAS-to-RAS. Returns 0.
    pub fn lta_voxel_to_ras_xform(
        &mut self,
        lta: &mut Lta,
        src: Option<&Mri>,
        dst: Option<&Mri>,
    ) -> Result<i32, Stop> {
        for i in 0..lta.xforms.len() {
            let m = lta.xforms[i].m_l.clone();
            let new = match src {
                Some(s) => match self.mri_voxel_xform_to_ras_xform(s, dst, m.as_ref(), None)? {
                    Returned::New(x) => Some(x),
                    _ => None,
                },
                None => {
                    let (sg, dg) = (lta.xforms[i].src.clone(), lta.xforms[i].dst.clone());
                    let s = self.vg_i_to_r(&sg)?;
                    let d = self.vg_r_to_i(&dg)?;
                    let inner = match self.matrix_multiply(m.as_ref(), Some(&s), None)? {
                        Returned::New(x) => Some(x),
                        _ => None,
                    };
                    match self.matrix_multiply(d.as_ref(), inner.as_ref(), None)? {
                        Returned::New(x) => Some(x),
                        _ => None,
                    }
                }
            };
            lta.xforms[i].m_l = new;
        }
        lta.kind = lta_type::LINEAR_RAS_TO_RAS;
        Ok(codes::NONE)
    }

    /// `LTArasToVoxelXform(lta, src, dst)` (§67): an array already voxel-to-voxel left alone;
    /// else each transform's matrix (as RAS to RAS) replaced by `MRIrasXformToVoxelXform(src,
    /// dst, M)` (FS-954: `reproduce` stores every result in the first transform), every
    /// transform's geometries the volumes' (FS-957: without a destination volume the source's
    /// for both by default, `reproduce` ends the program after the conversions), the type
    /// voxel-to-voxel. Returns 0. A missing source with a transform to convert ends the program
    /// (FreeSurfer crashes there).
    pub fn lta_ras_to_voxel_xform(
        &mut self,
        lta: &mut Lta,
        src: Option<&Mri>,
        dst: Option<&Mri>,
    ) -> Result<i32, Stop> {
        if lta.kind == lta_type::LINEAR_VOX_TO_VOX {
            return Ok(codes::NONE);
        }
        let n = lta.xforms.len();
        if n == 0 {
            lta.kind = lta_type::LINEAR_VOX_TO_VOX;
            return Ok(codes::NONE);
        }
        let Some(s) = src else {
            self.error_exit(
                codes::BAD_PARM,
                b"LTArasToVoxelXform: no source volume (FreeSurfer reads through a null volume here)\n",
            )?;
            return Ok(codes::NONE);
        };
        let reproduce = self.mode().reproduce();
        if n >= 2 {
            self.note(
                &FS_954,
                if reproduce {
                    "LTArasToVoxelXform: every conversion stored in the first transform (FreeSurfer's), the others unconverted"
                } else {
                    "LTArasToVoxelXform: each transform's conversion stored in that transform (FreeSurfer stores every one in the first)"
                },
            );
        }
        for i in 0..n {
            let m = lta.xforms[i].m_l.clone();
            let v = self.mri_ras_xform_to_voxel_xform(s, dst, m.as_ref())?;
            let k = if reproduce { 0 } else { i };
            lta.xforms[k].m_l = v;
        }
        let d = match dst {
            Some(d) => d,
            None => {
                if reproduce {
                    self.note(
                        &FS_957,
                        "LTArasToVoxelXform: no destination volume: the program ended (FreeSurfer reads through a null volume)",
                    );
                    self.error_exit(
                        codes::BAD_PARM,
                        b"LTArasToVoxelXform: no destination volume (FreeSurfer reads through a null volume here)\n",
                    )?;
                    return Ok(codes::NONE);
                }
                self.note(
                    &FS_957,
                    "LTArasToVoxelXform: no destination volume: the source's geometry for the destinations (FreeSurfer reads through a null volume)",
                );
                s
            }
        };
        for i in 0..n {
            let mut g = lta.xforms[i].src.clone();
            self.copy_vol_geom_from_mri(s, &mut g);
            lta.xforms[i].src = g;
            let mut g = lta.xforms[i].dst.clone();
            self.copy_vol_geom_from_mri(d, &mut g);
            lta.xforms[i].dst = g;
        }
        lta.kind = lta_type::LINEAR_VOX_TO_VOX;
        Ok(codes::NONE)
    }

    /// `LTAcompose(src, left, right, dst)` (§67): the destination's first matrix `D` from the
    /// source's first `L`: `left L` with `left`, `L right` with `right` (FS-956: with both into a
    /// new or another array, `reproduce` starts the second product again from `L`); in place
    /// both apply. A new destination: one transform with the source's first geometries. FS-504
    /// for a source or destination of no transforms.
    pub fn lta_compose(
        &mut self,
        src: &mut Lta,
        left: Option<&Matrix>,
        right: Option<&Matrix>,
        dst: ComposeInto<'_>,
    ) -> Result<Composed, Stop> {
        if src.xforms.is_empty() {
            self.lta_fs504("LTAcompose", "a source array without transforms")?;
            return Ok(Composed::Given);
        }
        match dst {
            ComposeInto::Itself => {
                let mut d = src.xforms[0].m_l.take();
                // in place: `L` is `D` itself, so both products apply, `(left L) right`
                self.compose_into(&mut d, None, left, right, true)?;
                src.xforms[0].m_l = d;
                Ok(Composed::Given)
            }
            ComposeInto::Given(d) => {
                if d.xforms.is_empty() {
                    self.lta_fs504("LTAcompose", "a destination array without transforms")?;
                    return Ok(Composed::Given);
                }
                let l = src.xforms[0].m_l.clone();
                let mut dm = d.xforms[0].m_l.take();
                self.compose_into(&mut dm, l.as_ref(), left, right, false)?;
                d.xforms[0].m_l = dm;
                Ok(Composed::Given)
            }
            ComposeInto::New => {
                let mut n = self.lta_alloc(1, None)?;
                n.xforms[0].src = src.xforms[0].src.clone();
                n.xforms[0].dst = src.xforms[0].dst.clone();
                let l = src.xforms[0].m_l.clone();
                let mut dm = n.xforms[0].m_l.take();
                self.compose_into(&mut dm, l.as_ref(), left, right, false)?;
                n.xforms[0].m_l = dm;
                Ok(Composed::New(n))
            }
        }
    }

    /// The products of `LTAcompose` into `d`: with `in_place`, `d` holds `L` itself; otherwise
    /// `l` is `L`. A product into a destination without a matrix goes into a new matrix that is
    /// dropped (FS-956's default keeps the left one's for the right product).
    fn compose_into(
        &mut self,
        d: &mut Option<Matrix>,
        l: Option<&Matrix>,
        left: Option<&Matrix>,
        right: Option<&Matrix>,
        in_place: bool,
    ) -> Result<(), Stop> {
        if in_place {
            if let Some(a) = left {
                let cur = d.clone();
                self.compose_product(d, Some(a), cur.as_ref())?;
            }
            if let Some(b) = right {
                let cur = d.clone();
                self.compose_product(d, cur.as_ref(), Some(b))?;
            }
            return Ok(());
        }
        let both = left.is_some() && right.is_some();
        let reproduce = self.mode().reproduce();
        let mut kept: Option<Matrix> = None;
        if let Some(a) = left {
            let r = self.compose_product(d, Some(a), l)?;
            kept = r;
        }
        if let Some(b) = right {
            if both {
                if reproduce {
                    self.note(
                        &FS_956,
                        "LTAcompose: the right product started again from the source's matrix, the left product lost (FreeSurfer's)",
                    );
                    self.compose_product(d, l, Some(b))?;
                } else {
                    self.note(
                        &FS_956,
                        "LTAcompose: the right matrix multiplies the left product (FreeSurfer starts again from the source's matrix)",
                    );
                    match d.clone() {
                        Some(cur) => {
                            self.compose_product(d, Some(&cur), Some(b))?;
                        }
                        None => {
                            // a destination without a matrix: the left product's new matrix
                            self.compose_product(d, kept.as_ref(), Some(b))?;
                        }
                    }
                }
            } else {
                self.compose_product(d, l, Some(b))?;
            }
        }
        Ok(())
    }

    /// `a b` (`MatrixMultiply`) into `d` when it has a matrix (left as it was where the product
    /// fails), else into a new matrix, returned (and dropped by the caller).
    fn compose_product(
        &mut self,
        d: &mut Option<Matrix>,
        a: Option<&Matrix>,
        b: Option<&Matrix>,
    ) -> Result<Option<Matrix>, Stop> {
        match d.as_mut() {
            Some(m) => {
                self.matrix_multiply(a, b, Some(m))?;
                Ok(None)
            }
            None => Ok(match self.matrix_multiply(a, b, None)? {
                Returned::New(x) => Some(x),
                _ => None,
            }),
        }
    }
}
