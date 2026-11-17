// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer 8.2.0's `TRANSFORM` (`include/transform.h`, `utils/transform.cpp`; track A's relay 39
//! of FreeSurfer's foundations): a linear transform array (`LTA`, fs.mri and fs.volume) or a GCA
//! morph ([`Gcam`]) behind one type, as the atlas programs take them. `TransformAlloc` (2608),
//! `TransformRead` (2126), `TransformFree` (2177), `TransformInvert` (2642), `TransformSwapInverse`
//! (2625), `TransformSample` (2209), `TransformSampleReal` (2292), `TransformSampleReal2` (2365),
//! `TransformSampleInverse` (2449), `TransformSampleInverseFloat` (2518, with `GCAMsampleMorph`,
//! gcamorph.cpp 5274), `TransformSampleInverseVoxel` (2566), `TransformSampleDirection` (4553),
//! `TransformGetSrcVolGeom` (4793), `TransformGetDstVolGeom` (4810), `TransformSetMRIVolGeomToSrc`
//! (4827), `TransformSetMRIVolGeomToDst` (4835), `TransformCopyVolGeomToMRI` (411) and
//! `TransformSourceVoxelToAtlas` (4877).
//!
//! Since relay 40: `TransformWrite` (4140), `TransformCopy` (4154, with `GCAMcopy`, gcamorph.cpp
//! 17347), `TransformApplyType` (2660), `TransformApply` (2678) and `TransformCreateDensityMap`
//! (4122) for linear transform arrays.
//!
//! Since relay 41 (on track C's `LTAinverseTransformInterp`, `LTAcompose`, `LTArasToVoxelXform`
//! and `LTAvoxelToRasXform` in fs.volume): `TransformApplyInverse` (2699) and
//! `TransformApplyInverseType` (2717) for linear transform arrays, `TransformCompose` (4858),
//! `TransformRas2Vox` (4535) and `TransformVox2Ras` (4544, with `GCAMvoxToRas`, gcamorph.cpp
//! 11265).
//!
//! Since relay 43: a GCA morph's resamplings in `TransformApply`, `TransformApplyType`,
//! `TransformCreateDensityMap`, `TransformApplyInverse` and `TransformApplyInverseType`
//! (`GCAMmorphToAtlas`, its density form and `GCAMmorphFromAtlas`: `crate::morph`).
//!
//! Since relay 43 also `TransformConcat` (5026, with `GCAMconcat2`/`3`) and
//! `TransformInvertReplace` (5073, with `GCAMfillInverse`: `crate::concat`).
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-io.md` Part 34): FS-857 (the inverse
//! sample of a morph of spacing 0), FS-858 (a linear transform file whose type is the morph's),
//! FS-859 (a transform array without transforms), FS-860 (freeing no transform), FS-861 (the
//! inverse sample of a morph of no nodes), FS-862 (the nearest sample one voxel past the index
//! volumes), FS-875 (a morph's copy loses its inverse and affine), FS-876 (a copy into a transform
//! of the other kind), FS-928 (a conversion without its volume, a composition into a morph).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{
    FS_857, FS_858, FS_859, FS_860, FS_861, FS_862, FS_876, FS_928, FS_933,
};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_multiply};
use larmorx_freesurfer_core::matrix_io::matrix_ascii_write_into;
use larmorx_freesurfer_core::matrix_ops::{matrix_determinant, matrix_identity};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::utils::{file_name_only, nint};
use larmorx_freesurfer_io::identify::MRI_VOLUME_TYPE_UNKNOWN;
use larmorx_freesurfer_io::{mriio, registerio};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::lta::lta_copy;
use larmorx_freesurfer_mri::lta::{
    LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX, LinearTransform, Lta, MORPH_3D_TYPE, lta_alloc, lta_free,
};
use larmorx_freesurfer_mri::mri::{
    Mri, SAMPLE_NEAREST, SAMPLE_TRILINEAR, mri_copy_vol_geom_to_mri,
};
use larmorx_freesurfer_mri::vol_geom::VolGeom;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_volume::arith::{Pair, mri_scalar_mul};
use larmorx_freesurfer_volume::ltaapply::{
    lta_inverse_transform_interp, lta_transform, lta_transform_interp,
};
use larmorx_freesurfer_volume::ltaconv::{
    lta_compose, lta_compose_in_place, lta_ras_to_voxel_xform, lta_voxel_to_ras_xform,
};
use larmorx_freesurfer_volume::ltaops::{
    InvertInto, lta_change_type, lta_concat2, lta_fill_inverse, lta_invert,
};
use larmorx_freesurfer_volume::ltaread::{
    LtaIo, RegisterDat, lta_read_ex, transform_file_name_type,
};
use larmorx_freesurfer_volume::ltawrite::lta_write_ex;
use larmorx_freesurfer_volume::sample::sample_volume;
use larmorx_freesurfer_volume::talairachex::transform_with_matrix;
use larmorx_freesurfer_volume::vgeom::get_vol_geom;

use crate::gcam::{GCAM_POSITION_INVALID, Gcam, gcam_copy, gcam_invert, gcam_read};
use crate::morph::{
    gcam_morph_from_atlas, gcam_morph_to_atlas, gcam_morph_to_atlas_with_density_correction,
};

/// What a `TRANSFORM` holds (`xform`): a linear transform array or a GCA morph.
#[derive(Debug)]
pub enum Xform {
    Lta(Lta),
    Morph(Box<Gcam>),
}

/// `TRANSFORM`: its type (an LTA's type, or `MORPH_3D_TYPE` for a morph), the three unused node
/// volumes `mri_xn`, `mri_yn`, `mri_zn` (never set by FreeSurfer's functions) and the transform.
#[derive(Debug)]
pub struct Transform {
    pub ttype: i32,
    pub mri_xn: Option<Mri>,
    pub mri_yn: Option<Mri>,
    pub mri_zn: Option<Mri>,
    pub xform: Xform,
}

impl Transform {
    /// A transform of `ttype` holding `xform`.
    pub fn new(ttype: i32, xform: Xform) -> Transform {
        Transform {
            ttype,
            mri_xn: None,
            mri_yn: None,
            mri_zn: None,
            xform,
        }
    }

    /// The linear transform array (a transform whose type is not `MORPH_3D_TYPE`).
    pub fn lta(&self) -> &Lta {
        match &self.xform {
            Xform::Lta(l) => l,
            Xform::Morph(_) => panic!("a TRANSFORM of type {} holding a morph", self.ttype),
        }
    }

    /// [`Transform::lta`], to change.
    pub fn lta_mut(&mut self) -> &mut Lta {
        match &mut self.xform {
            Xform::Lta(l) => l,
            Xform::Morph(_) => panic!("a TRANSFORM of type {} holding a morph", self.ttype),
        }
    }

    /// The morph (a transform of type `MORPH_3D_TYPE`).
    pub fn gcam(&self) -> &Gcam {
        match &self.xform {
            Xform::Morph(m) => m,
            Xform::Lta(_) => panic!("a TRANSFORM of type MORPH_3D_TYPE holding a linear array"),
        }
    }

    /// [`Transform::gcam`], to change.
    pub fn gcam_mut(&mut self) -> &mut Gcam {
        match &mut self.xform {
            Xform::Morph(m) => m,
            Xform::Lta(_) => panic!("a TRANSFORM of type MORPH_3D_TYPE holding a linear array"),
        }
    }

    fn is_morph(&self) -> bool {
        self.ttype == MORPH_3D_TYPE
    }
}

/// FS-859: `lta->xforms[0]` of an array without transforms (`nxforms = 0` in its file) is past the
/// array in FreeSurfer (memory contents, or a crash): the program ends here in both modes.
fn first<'a>(
    g: &mut Globals,
    err: &mut dyn Write,
    lta: &'a Lta,
    what: &str,
) -> Result<&'a LinearTransform, Fatal> {
    if lta.xforms.is_empty() {
        g.notices.add(
            &FS_859,
            format!(
                "{what} of a transform array without transforms: FreeSurfer reads past the array"
            ),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "{what}: the transform array holds no transform (FreeSurfer reads past it here)"
            ),
        )?;
    }
    Ok(&lta.xforms[0])
}

/// The inverse transform `lta->inv_xforms[0]`, as [`first`].
fn first_inverse<'a>(
    g: &mut Globals,
    err: &mut dyn Write,
    lta: &'a Lta,
    what: &str,
) -> Result<&'a LinearTransform, Fatal> {
    first(g, err, lta, what)?;
    Ok(&lta.inv_xforms[0])
}

/// `TransformAlloc(type, mri)`: a linear transform of that type holding `LTAalloc(1, mri)`;
/// `MORPH_3D_TYPE` gives `TransformAlloc(MORPH_3D_TYPE): unsupported` (`ERROR_UNSUPPORTED`) and
/// `None`.
pub fn transform_alloc(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ttype: i32,
    mri: Option<&Mri>,
) -> Result<Option<Transform>, Fatal> {
    if ttype == MORPH_3D_TYPE {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            "TransformAlloc(MORPH_3D_TYPE): unsupported",
        );
        return Ok(None);
    }
    let lta = lta_alloc(g, out, err, 1, mri)?;
    Ok(Some(Transform::new(ttype, Xform::Lta(lta))))
}

/// fs.io's readers behind fs.volume's `LtaIo` (`LTAreadEx`'s calls): `MRIreadHeader` of a volume a
/// transform's comment names, and `regio_read_register` (an `.lta` never reaches it here:
/// `TransformFileNameType` sends those names to the `.lta` reader). Public for the programs that
/// read a linear transform array with `LTAread` themselves (`mris_sphere`).
pub struct GcaLtaIo<'e> {
    pub env: &'e Environment,
}

impl LtaIo for GcaLtaIo<'_> {
    fn env(&self) -> &Environment {
        self.env
    }

    fn read_header(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        dir: Option<&Path>,
        name: &str,
    ) -> Result<Option<VolGeom>, Fatal> {
        Ok(
            mriio::mri_read_header(g, out, err, self.env, dir, name, MRI_VOLUME_TYPE_UNKNOWN)?
                .map(|m| m.vg),
        )
    }

    fn read_register(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        dir: Option<&Path>,
        name: &str,
    ) -> Result<Option<RegisterDat>, Fatal> {
        let mut reg = registerio::Register::default();
        if registerio::regio_read_register(
            g,
            out,
            err,
            dir,
            name,
            &mut reg,
            &mut registerio::NoLta,
        )? != 0
        {
            return Ok(None);
        }
        Ok(reg.r.map(|r| RegisterDat {
            subject: reg.subject.unwrap_or_default(),
            inplaneres: reg.inplaneres,
            betplaneres: reg.betplaneres,
            intensity: reg.intensity,
            r,
            float2int: reg.float2int,
        }))
    }
}

/// `TransformRead(fname)`: a name whose file part is `identity.nofile` gives an identity
/// `LINEAR_RAS_TO_RAS` transform (no file read); a name `TransformFileNameType` calls a morph
/// (`.m3z`, `.m3d`, `.mgz`, `.nii`, `nii.gz`) is read by `GCAMread`, any other by `LTAreadEx`, the
/// transform taking the array's type. `None` when the reader gives nothing. FS-858: an array whose
/// type is `MORPH_3D_TYPE` is refused (`TransformRead(<f>): the linear transform has the morph's
/// type 11 (FreeSurfer then uses it as a morph)`, `ERROR_BADFILE`, `None`) in both modes.
pub fn transform_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Transform>, Fatal> {
    if file_name_only(fname) == "identity.nofile" {
        let mut lta = lta_alloc(g, out, err, 1, None)?;
        let Ret::New(id) = matrix_identity(g, out, err, 4, None)? else {
            unreachable!("MatrixIdentity without an output returns a new matrix")
        };
        lta.xforms[0].m_l = Some(id);
        lta.xforms[0].ltype = LINEAR_RAS_TO_RAS;
        lta.ltype = LINEAR_RAS_TO_RAS;
        return Ok(Some(Transform::new(LINEAR_RAS_TO_RAS, Xform::Lta(lta))));
    }
    let ttype = transform_file_name_type(g, fname.as_bytes());
    if ttype == MORPH_3D_TYPE {
        let Some(gcam) = gcam_read(g, out, err, env, dir, fname)? else {
            return Ok(None);
        };
        return Ok(Some(Transform::new(
            MORPH_3D_TYPE,
            Xform::Morph(Box::new(gcam)),
        )));
    }
    let mut io = GcaLtaIo { env };
    let Some(lta) = lta_read_ex(g, out, err, dir, fname, &mut io)? else {
        return Ok(None);
    };
    if lta.ltype == MORPH_3D_TYPE {
        g.notices.add(
            &FS_858,
            "TransformRead of a linear transform file of type 11 (MORPH_3D_TYPE): FreeSurfer then treats the array as a morph (memory contents or a crash)",
        );
        g.errno = 0;
        log::error_printf(
            g,
            err,
            larmorx_freesurfer_core::log::ERROR_BADFILE,
            &format!(
                "TransformRead({fname}): the linear transform has the morph's type 11 (FreeSurfer then uses it as a morph)"
            ),
        );
        return Ok(None);
    }
    Ok(Some(Transform::new(lta.ltype, Xform::Lta(lta))))
}

/// `TransformFree(&transform)`: the array (`LTAfree`) or the morph (`GCAMfree`) and the transform
/// freed, the pointer `NULL`; `NO_ERROR`. FS-860: with no transform FreeSurfer crashes; here
/// nothing is freed (both modes).
pub fn transform_free(g: &mut Globals, ptrans: &mut Option<Transform>) -> i32 {
    let Some(t) = ptrans.take() else {
        g.notices.add(
            &FS_860,
            "TransformFree of no transform: nothing to free (FreeSurfer crashes)",
        );
        return NO_ERROR;
    };
    match t.xform {
        Xform::Lta(l) => {
            let mut l = Some(l);
            lta_free(g, &mut l)
        }
        Xform::Morph(_) => NO_ERROR,
    }
}

/// `TransformInvert(transform, mri)`: a linear transform's inverses filled (`LTAfillInverse`); a
/// morph's inverse index volumes made on `mri`'s grid (`GCAMinvert`; without `mri` nothing).
/// `NO_ERROR`.
pub fn transform_invert(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    t: &mut Transform,
    mri: Option<&Mri>,
) -> Result<i32, Fatal> {
    if t.is_morph() {
        if let Some(m) = mri {
            gcam_invert(g, out, err, env, dir, t.gcam_mut(), Some(m))?;
        }
    } else {
        lta_fill_inverse(g, out, err, t.lta_mut())?;
    }
    Ok(NO_ERROR)
}

/// `TransformSwapInverse(transform)`: a linear array's transforms and inverse transforms
/// exchanged (`NO_ERROR`); a morph gives `TransformSwapInverse: MORPH_3D_TYPE not supported`
/// (`ERROR_UNSUPPORTED`).
pub fn transform_swap_inverse(g: &mut Globals, err: &mut dyn Write, t: &mut Transform) -> i32 {
    if t.is_morph() {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            "TransformSwapInverse: MORPH_3D_TYPE not supported",
        );
        return ERROR_UNSUPPORTED;
    }
    let l = t.lta_mut();
    std::mem::swap(&mut l.xforms, &mut l.inv_xforms);
    NO_ERROR
}

/// The sample functions' conversion of a linear array that is not voxel to voxel: `Converting to
/// LTA type LINEAR_VOXEL_TO_VOXEL...` on standard output, `LTAchangeType`, `After conversion:` and
/// each transform's matrix (`MatrixAsciiWriteInto`).
fn to_vox2vox(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    lta: &mut Lta,
) -> Result<(), Fatal> {
    if lta.ltype == LINEAR_VOX_TO_VOX {
        return Ok(());
    }
    let _ = out.write_all(b"Converting to LTA type LINEAR_VOXEL_TO_VOXEL...\n");
    lta_change_type(g, out, err, env, lta, LINEAR_VOX_TO_VOX)?;
    let _ = out.write_all(b"After conversion:\n");
    for i in 0..lta.num_xforms.max(0) as usize {
        if let Some(m) = &lta.xforms[i].m_l {
            matrix_ascii_write_into(g, out, m);
        }
    }
    Ok(())
}

/// `m_L * (x, y, z, 1)` through `float` vectors (`MatrixMultiply`): the first three elements.
fn apply(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: Option<&Matrix>,
    x: f32,
    y: f32,
    z: f32,
) -> Result<[f32; 3], Fatal> {
    let mut v_in = Matrix::real(4, 1);
    v_in.set_relt(1, 1, x);
    v_in.set_relt(2, 1, y);
    v_in.set_relt(3, 1, z);
    v_in.set_relt(4, 1, 1.0);
    let mut v_out = Matrix::real(4, 1);
    v_out.set_relt(4, 1, 1.0);
    matrix_multiply(g, out, err, m, Some(&v_in), Some(&mut v_out))?;
    Ok([v_out.relt(1, 1), v_out.relt(2, 1), v_out.relt(3, 1)])
}

/// The morph sample functions' check: `TransformSample: gcam has not been inverted!`
/// (`ERROR_UNSUPPORTED`) when the morph has no inverse index volumes.
fn not_inverted(g: &mut Globals, err: &mut dyn Write) -> i32 {
    log::error_printf(
        g,
        err,
        ERROR_UNSUPPORTED,
        "TransformSample: gcam has not been inverted!",
    );
    ERROR_UNSUPPORTED
}

/// A point clamped to the index volumes as the morph sample functions clamp it (`float`
/// comparisons: a NaN passes).
fn clamp_to(xv: f32, n: i32) -> f32 {
    let mut v = xv;
    if v < 0.0 {
        v = 0.0;
    }
    if v >= n as f32 {
        v = (n - 1) as f32;
    }
    v
}

/// The index volumes' extents (`mri_xind->width`, `mri_yind->height`, `mri_zind->depth`).
fn index_extents(gcam: &Gcam) -> Option<(&Mri, &Mri, &Mri)> {
    Some((
        gcam.mri_xind.as_ref()?,
        gcam.mri_yind.as_ref()?,
        gcam.mri_zind.as_ref()?,
    ))
}

/// `TransformSample(transform, xv, yv, zv, &px, &py, &pz)`: an image voxel to the atlas (or
/// morph) space. A morph: the point clamped to its inverse index volumes (`TransformSample: gcam
/// has not been inverted!`, `ERROR_UNSUPPORTED` and 0s without them), each index volume sampled
/// trilinearly (`MRIsampleVolume`) and times the spacing (`float`). A linear array (converted to
/// voxel to voxel first when it is not: [`to_vox2vox`]): the first transform's matrix times the
/// point (`float`). The status and the point.
#[allow(clippy::too_many_arguments)]
pub fn transform_sample(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    t: &mut Transform,
    xv: f32,
    yv: f32,
    zv: f32,
) -> Result<(i32, [f32; 3]), Fatal> {
    if t.is_morph() {
        let gcam = t.gcam();
        let Some((xi_m, yi_m, zi_m)) = index_extents(gcam) else {
            return Ok((not_inverted(g, err), [0.0; 3]));
        };
        let xv = clamp_to(xv, xi_m.vg.width);
        let yv = clamp_to(yv, yi_m.vg.height);
        let zv = clamp_to(zv, zi_m.vg.depth);
        let clampi = |v: i32, n: i32| {
            if v < 0 {
                0
            } else if v >= n {
                n - 1
            } else {
                v
            }
        };
        let xi = clampi(nint(g, f64::from(xv)), xi_m.vg.width);
        let yi = clampi(nint(g, f64::from(yv)), yi_m.vg.height);
        let zi = clampi(nint(g, f64::from(zv)), zi_m.vg.depth);
        let spacing = gcam.spacing as f32;
        // FreeSurfer's nearest index values, overwritten by the trilinear samples below (their
        // nint conversions only)
        for m in [xi_m, yi_m, zi_m] {
            let v = mri_get_vox_val(g, err, m, xi, yi, zi, 0, 0)?;
            let _ = nint(g, f64::from(sse::mul(v, spacing)));
        }
        let (mut xd, mut yd, mut zd) = (0.0, 0.0, 0.0);
        let (x, y, z) = (f64::from(xv), f64::from(yv), f64::from(zv));
        sample_volume(g, err, xi_m, x, y, z, &mut xd);
        sample_volume(g, err, yi_m, x, y, z, &mut yd);
        sample_volume(g, err, zi_m, x, y, z, &mut zd);
        let p = [
            sse::mul(xd as f32, spacing),
            sse::mul(yd as f32, spacing),
            sse::mul(zd as f32, spacing),
        ];
        return Ok((NO_ERROR, p));
    }
    let lta = t.lta_mut();
    to_vox2vox(g, out, err, env, lta)?;
    let m = first(g, err, lta, "TransformSample")?.m_l.as_ref();
    let p = apply(g, out, err, m, xv, yv, zv)?;
    Ok((NO_ERROR, p))
}

/// `TransformSampleReal(transform, xv, yv, zv, ...)`: as [`transform_sample`], a morph's index
/// volumes read at the nearest voxel (`nint` of the clamped point) times the spacing; a linear
/// array's result with each coordinate below 0 set to 0. FS-862: FreeSurfer does not clamp the
/// rounded voxel, so a point within half a voxel of the far face reads one voxel past the index
/// volumes (another voxel, or memory past the last slice): by default the last voxel; `reproduce`
/// FreeSurfer's index.
#[allow(clippy::too_many_arguments)]
pub fn transform_sample_real(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    t: &mut Transform,
    xv: f32,
    yv: f32,
    zv: f32,
) -> Result<(i32, [f32; 3]), Fatal> {
    if t.is_morph() {
        let gcam = t.gcam();
        let Some((xi_m, yi_m, zi_m)) = index_extents(gcam) else {
            return Ok((not_inverted(g, err), [0.0; 3]));
        };
        let xv = clamp_to(xv, xi_m.vg.width);
        let yv = clamp_to(yv, yi_m.vg.height);
        let zv = clamp_to(zv, zi_m.vg.depth);
        let mut xi = nint(g, f64::from(xv));
        let mut yi = nint(g, f64::from(yv));
        let mut zi = nint(g, f64::from(zv));
        let (w, h, d) = (xi_m.vg.width, yi_m.vg.height, zi_m.vg.depth);
        if xi >= w || yi >= h || zi >= d {
            g.notices.add(
                &FS_862,
                if g.notices.reproduce() {
                    "TransformSampleReal of a morph at a point within half a voxel of the index volumes' far face: read one voxel past them, as in FreeSurfer"
                } else {
                    "TransformSampleReal of a morph at a point within half a voxel of the index volumes' far face: the last voxel read (FreeSurfer reads one past it)"
                },
            );
            if g.notices.fix() {
                xi = xi.min(w - 1);
                yi = yi.min(h - 1);
                zi = zi.min(d - 1);
            }
        }
        let spacing = gcam.spacing as f32;
        let mut p = [0f32; 3];
        for (k, m) in [xi_m, yi_m, zi_m].into_iter().enumerate() {
            p[k] = sse::mul(mri_get_vox_val(g, err, m, xi, yi, zi, 0, 0)?, spacing);
        }
        return Ok((NO_ERROR, p));
    }
    let lta = t.lta_mut();
    to_vox2vox(g, out, err, env, lta)?;
    let m = first(g, err, lta, "TransformSampleReal")?.m_l.as_ref();
    let mut p = apply(g, out, err, m, xv, yv, zv)?;
    for v in &mut p {
        if *v < 0.0 {
            *v = 0.0;
        }
    }
    Ok((NO_ERROR, p))
}

/// `TransformSampleReal2(transform, xv, yv, zv, ...)`: as [`transform_sample`] for a morph, the
/// trilinear samples times the spacing in `double` (then `float`); a linear array as
/// [`transform_sample_real`].
#[allow(clippy::too_many_arguments)]
pub fn transform_sample_real2(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    t: &mut Transform,
    xv: f32,
    yv: f32,
    zv: f32,
) -> Result<(i32, [f32; 3]), Fatal> {
    if t.is_morph() {
        let gcam = t.gcam();
        let Some((xi_m, yi_m, zi_m)) = index_extents(gcam) else {
            return Ok((not_inverted(g, err), [0.0; 3]));
        };
        let xv = clamp_to(xv, xi_m.vg.width);
        let yv = clamp_to(yv, yi_m.vg.height);
        let zv = clamp_to(zv, zi_m.vg.depth);
        let spacing = f64::from(gcam.spacing);
        let (x, y, z) = (f64::from(xv), f64::from(yv), f64::from(zv));
        let mut p = [0f32; 3];
        for (k, m) in [xi_m, yi_m, zi_m].into_iter().enumerate() {
            let mut d = 0.0;
            sample_volume(g, err, m, x, y, z, &mut d);
            p[k] = sse::mul64(d, spacing) as f32;
        }
        return Ok((NO_ERROR, p));
    }
    let lta = t.lta_mut();
    to_vox2vox(g, out, err, env, lta)?;
    let m = first(g, err, lta, "TransformSampleReal2")?.m_l.as_ref();
    let mut p = apply(g, out, err, m, xv, yv, zv)?;
    for v in &mut p {
        if *v < 0.0 {
            *v = 0.0;
        }
    }
    Ok((NO_ERROR, p))
}

/// `TransformSampleInverse(transform, xv, yv, zv, ...)`: an atlas voxel to the image. A morph: the
/// node nearest to the voxel divided by the spacing (in `int`), clamped to the grid; its current
/// position, and `ERROR_BADPARM` when the node is not valid. FS-857: a spacing of 0 (an integer
/// division by zero in FreeSurfer) and FS-861: a morph of no nodes (a node past the grid) end the
/// program in both modes. A linear array (converted to voxel to voxel first: [`to_vox2vox`]): the
/// first inverse transform's matrix times the point (`float`).
#[allow(clippy::too_many_arguments)]
pub fn transform_sample_inverse(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    t: &mut Transform,
    xv: i32,
    yv: i32,
    zv: i32,
) -> Result<(i32, [f32; 3]), Fatal> {
    if t.is_morph() {
        let gcam = t.gcam();
        if gcam.spacing == 0 {
            g.notices.add(
                &FS_857,
                "TransformSampleInverse of a morph of spacing 0: an integer division by zero (FreeSurfer crashes)",
            );
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "TransformSampleInverse: the morph's spacing is 0 (FreeSurfer divides by it and crashes)",
            )?;
        }
        let sp = gcam.spacing;
        let mut xn = nint(g, f64::from(xv.wrapping_div(sp)));
        let mut yn = nint(g, f64::from(yv.wrapping_div(sp)));
        let mut zn = nint(g, f64::from(zv.wrapping_div(sp)));
        if xn >= gcam.width {
            xn = gcam.width - 1;
        }
        if yn >= gcam.height {
            yn = gcam.height - 1;
        }
        if zn >= gcam.depth {
            zn = gcam.depth - 1;
        }
        xn = xn.max(0);
        yn = yn.max(0);
        zn = zn.max(0);
        if gcam.width <= 0 || gcam.height <= 0 || gcam.depth <= 0 {
            g.notices.add(
                &FS_861,
                "TransformSampleInverse of a morph of no nodes: FreeSurfer reads a node past the grid",
            );
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "TransformSampleInverse: the morph has no nodes (FreeSurfer reads past them here)",
            )?;
        }
        let n = gcam.node(xn, yn, zn);
        let code = if n.invalid != 0 {
            ERROR_BADPARM
        } else {
            NO_ERROR
        };
        return Ok((code, [n.x as f32, n.y as f32, n.z as f32]));
    }
    let lta = t.lta_mut();
    to_vox2vox(g, out, err, env, lta)?;
    let m = first_inverse(g, err, lta, "TransformSampleInverse")?
        .m_l
        .as_ref();
    let p = apply(g, out, err, m, xv as f32, yv as f32, zv as f32)?;
    Ok((NO_ERROR, p))
}

/// `GCAMsampleMorph(gcam, x, y, z, ...)`: a point of the atlas (in voxels) to the image: divided
/// by the spacing (`float`); outside `[0, extent)` on an axis `ERROR_BADPARM`; else the eight nodes
/// around it (`(int)` of the point, the next ones clamped to the grid), `ERROR_BADPARM` when one
/// of them has an invalid position, else their current positions weighted by the distances
/// (`float` weights times the `double` positions, summed in `double`, stored as `float`).
///
/// FS-933: a NaN point (a spacing of 0 makes one of the voxel 0) passes the check; on a grid
/// without nodes along one of its axes FreeSurfer then reads outside the nodes (a crash): the
/// program ends here in both modes.
pub fn gcam_sample_morph(
    g: &mut Globals,
    err: &mut dyn Write,
    gcam: &Gcam,
    x: f32,
    y: f32,
    z: f32,
) -> Result<(i32, [f32; 3]), Fatal> {
    match sample_morph(gcam, x, y, z) {
        Some(r) => Ok(r),
        None => {
            g.notices.add(
                &FS_933,
                "GCAMsampleMorph of a NaN point on a morph without nodes along an axis: FreeSurfer reads outside the nodes (a crash)",
            );
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "GCAMsampleMorph: a NaN point of a morph without nodes along an axis (FreeSurfer reads outside its nodes and crashes here)",
            )?;
            Ok((ERROR_BADPARM, [0.0; 3]))
        }
    }
}

/// [`gcam_sample_morph`]'s arithmetic; `None` when a corner is outside the nodes.
fn sample_morph(gcam: &Gcam, x: f32, y: f32, z: f32) -> Option<(i32, [f32; 3])> {
    let sp = gcam.spacing as f32;
    let (x, y, z) = (sse::div(x, sp), sse::div(y, sp), sse::div(z, sp));
    let (width, height, depth) = (gcam.width, gcam.height, gcam.depth);
    if x >= width as f32 || y >= height as f32 || z >= depth as f32 || x < 0.0 || y < 0.0 || z < 0.0
    {
        return Some((ERROR_BADPARM, [0.0; 3]));
    }
    // (int) of a NaN is INT_MIN on x86-64 (cvttss2si): MAX(..., 0) gives 0 there and here
    let trunc = |v: f32| if v.is_nan() { i32::MIN } else { v as i32 };
    let xm = trunc(x).max(0);
    let xp = (width - 1).min(xm + 1);
    let ym = trunc(y).max(0);
    let yp = (height - 1).min(ym + 1);
    let zm = trunc(z).max(0);
    let zp = (depth - 1).min(zm + 1);
    let xmd = sse::sub(x, xm as f32);
    let ymd = sse::sub(y, ym as f32);
    let zmd = sse::sub(z, zm as f32);
    let xpd = sse::sub(1.0, xmd);
    let ypd = sse::sub(1.0, ymd);
    let zpd = sse::sub(1.0, zmd);
    // a NaN point on a grid without nodes along an axis: corners outside the nodes (FS-933)
    if [xm, xp, ym, yp, zm, zp]
        .iter()
        .zip([width, width, height, height, depth, depth])
        .any(|(&v, n)| v < 0 || v >= n)
    {
        return None;
    }
    let corners = [
        (xm, ym, zm, xpd, ypd, zpd),
        (xm, ym, zp, xpd, ypd, zmd),
        (xm, yp, zm, xpd, ymd, zpd),
        (xm, yp, zp, xpd, ymd, zmd),
        (xp, ym, zm, xmd, ypd, zpd),
        (xp, ym, zp, xmd, ypd, zmd),
        (xp, yp, zm, xmd, ymd, zpd),
        (xp, yp, zp, xmd, ymd, zmd),
    ];
    if corners
        .iter()
        .any(|&(a, b, c, ..)| gcam.node(a, b, c).invalid == GCAM_POSITION_INVALID)
    {
        return Some((ERROR_BADPARM, [0.0; 3]));
    }
    // the official build's operand orders (gcamorph.cpp.o; they decide which NaN a sum of two
    // keeps): the weights of the corners with xp are ypd * xmd (and zpd * (ymd * xmd) for
    // [xp][yp][zm]); x and y multiply each position by its weight and add the terms in order;
    // z multiplies the weight by the position (but [xm][yp][zm]'s) and adds the fourth term and
    // the eighth before the sum so far
    let w = [
        sse::mul(sse::mul(xpd, ypd), zpd),
        sse::mul(sse::mul(xpd, ypd), zmd),
        sse::mul(sse::mul(xpd, ymd), zpd),
        sse::mul(sse::mul(xpd, ymd), zmd),
        sse::mul(sse::mul(ypd, xmd), zpd),
        sse::mul(sse::mul(ypd, xmd), zmd),
        sse::mul(zpd, sse::mul(ymd, xmd)),
        sse::mul(sse::mul(ymd, xmd), zmd),
    ]
    .map(f64::from);
    let pos = |k: usize, i: usize| {
        let (a, b, c, ..) = corners[i];
        let n = gcam.node(a, b, c);
        [n.x, n.y, n.z][k]
    };
    let mut p = [0f32; 3];
    for (k, out) in p.iter_mut().enumerate().take(2) {
        let mut sum = sse::mul64(pos(k, 0), w[0]);
        for (i, &wi) in w.iter().enumerate().skip(1) {
            sum = sse::add64(sum, sse::mul64(pos(k, i), wi));
        }
        *out = sum as f32;
    }
    let t: Vec<f64> = (0..8)
        .map(|i| {
            if i == 2 {
                sse::mul64(pos(2, i), w[i])
            } else {
                sse::mul64(w[i], pos(2, i))
            }
        })
        .collect();
    let s = sse::add64(sse::add64(t[0], t[1]), t[2]);
    let s = sse::add64(t[3], s);
    let s = sse::add64(sse::add64(sse::add64(s, t[4]), t[5]), t[6]);
    p[2] = sse::add64(t[7], s) as f32;
    Some((NO_ERROR, p))
}

/// `TransformSampleInverseFloat(transform, xv, yv, zv, ...)`: a morph through
/// [`gcam_sample_morph`] (its status: an error outside the grid); a linear array as
/// [`transform_sample_inverse`] at the `float` point.
#[allow(clippy::too_many_arguments)]
pub fn transform_sample_inverse_float(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    t: &mut Transform,
    xv: f32,
    yv: f32,
    zv: f32,
) -> Result<(i32, [f32; 3]), Fatal> {
    if t.is_morph() {
        return gcam_sample_morph(g, err, t.gcam(), xv, yv, zv);
    }
    let lta = t.lta_mut();
    to_vox2vox(g, out, err, env, lta)?;
    let m = first_inverse(g, err, lta, "TransformSampleInverseFloat")?
        .m_l
        .as_ref();
    let p = apply(g, out, err, m, xv, yv, zv)?;
    Ok((NO_ERROR, p))
}

/// `TransformSampleInverseVoxel(transform, width, height, depth, xv, yv, zv, ...)`:
/// [`transform_sample_inverse`]'s point rounded (`nint`) and clamped to `[0, extent - 1]`;
/// `ERROR_BADPARM` when a coordinate had to be clamped, else the inverse sample's status.
#[allow(clippy::too_many_arguments)]
pub fn transform_sample_inverse_voxel(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    t: &mut Transform,
    width: i32,
    height: i32,
    depth: i32,
    xv: i32,
    yv: i32,
    zv: i32,
) -> Result<(i32, [i32; 3]), Fatal> {
    let (mut code, [xf, yf, zf]) = transform_sample_inverse(g, out, err, env, t, xv, yv, zv)?;
    let mut p = [
        nint(g, f64::from(xf)),
        nint(g, f64::from(yf)),
        nint(g, f64::from(zf)),
    ];
    for (v, n) in p.iter_mut().zip([width, height, depth]) {
        if *v < 0 {
            code = ERROR_BADPARM;
            *v = 0;
        }
        if *v >= n {
            code = ERROR_BADPARM;
            *v = n - 1;
        }
    }
    Ok((code, p))
}

/// `TransformSampleDirection(transform, x0, y0, z0, nx, ny, nz, ...)`: the direction `(nx, ny,
/// nz)` at `(x0, y0, z0)` carried by the transform: [`transform_sample_real`] of the point and of
/// the point plus twice the direction, their difference divided by its length (`float`).
#[allow(clippy::too_many_arguments)]
pub fn transform_sample_direction(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    t: &mut Transform,
    p0: [f32; 3],
    n: [f32; 3],
) -> Result<[f32; 3], Fatal> {
    let (_, a0) = transform_sample_real(g, out, err, env, t, p0[0], p0[1], p0[2])?;
    let q = [
        sse::add(p0[0], sse::mul(2.0, n[0])),
        sse::add(p0[1], sse::mul(2.0, n[1])),
        sse::add(p0[2], sse::mul(2.0, n[2])),
    ];
    let (_, a1) = transform_sample_real(g, out, err, env, t, q[0], q[1], q[2])?;
    let d = [
        sse::sub(a1[0], a0[0]),
        sse::sub(a1[1], a0[1]),
        sse::sub(a1[2], a0[2]),
    ];
    let sq = sse::add(
        sse::add(sse::mul(d[0], d[0]), sse::mul(d[1], d[1])),
        sse::mul(d[2], d[2]),
    );
    let mag = sse::sqrt(sq);
    Ok([
        sse::div(d[0], mag),
        sse::div(d[1], mag),
        sse::div(d[2], mag),
    ])
}

/// `TransformGetSrcVolGeom(transform, &vg)`: a morph's image geometry, a linear array's first
/// transform's source geometry (FS-859 without one).
pub fn transform_get_src_vol_geom(
    g: &mut Globals,
    err: &mut dyn Write,
    t: &Transform,
) -> Result<VolGeom, Fatal> {
    let mut vg = VolGeom::default();
    if t.is_morph() {
        vg.assign(g, &t.gcam().image);
    } else {
        let src = &first(g, err, t.lta(), "TransformGetSrcVolGeom")?.src;
        vg.assign(g, src);
    }
    Ok(vg)
}

/// `TransformGetDstVolGeom(transform, &vg)`: a morph's atlas geometry, a linear array's first
/// transform's destination geometry (FS-859 without one).
pub fn transform_get_dst_vol_geom(
    g: &mut Globals,
    err: &mut dyn Write,
    t: &Transform,
) -> Result<VolGeom, Fatal> {
    let mut vg = VolGeom::default();
    if t.is_morph() {
        vg.assign(g, &t.gcam().atlas);
    } else {
        let dst = &first(g, err, t.lta(), "TransformGetDstVolGeom")?.dst;
        vg.assign(g, dst);
    }
    Ok(vg)
}

/// `TransformSetMRIVolGeomToSrc(transform, mri)`: `MRIcopyVolGeomToMRI` of
/// [`transform_get_src_vol_geom`]. `NO_ERROR`.
pub fn transform_set_mri_vol_geom_to_src(
    g: &mut Globals,
    err: &mut dyn Write,
    t: &Transform,
    mri: &mut Mri,
) -> Result<i32, Fatal> {
    let vg = transform_get_src_vol_geom(g, err, t)?;
    mri_copy_vol_geom_to_mri(mri, &vg);
    Ok(NO_ERROR)
}

/// `TransformSetMRIVolGeomToDst(transform, mri)`: `MRIcopyVolGeomToMRI` of
/// [`transform_get_dst_vol_geom`]. `NO_ERROR`.
pub fn transform_set_mri_vol_geom_to_dst(
    g: &mut Globals,
    err: &mut dyn Write,
    t: &Transform,
    mri: &mut Mri,
) -> Result<i32, Fatal> {
    let vg = transform_get_dst_vol_geom(g, err, t)?;
    mri_copy_vol_geom_to_mri(mri, &vg);
    Ok(NO_ERROR)
}

/// `TransformCopyVolGeomToMRI(transform, mri)`: a linear array's first transform's destination
/// geometry into `mri` (`MRIcopyVolGeomToMRI`; FS-859 without one); a morph changes nothing.
/// `NO_ERROR`.
pub fn transform_copy_vol_geom_to_mri(
    g: &mut Globals,
    err: &mut dyn Write,
    t: &Transform,
    mri: &mut Mri,
) -> Result<i32, Fatal> {
    if !t.is_morph() {
        let dst = &first(g, err, t.lta(), "TransformCopyVolGeomToMRI")?.dst;
        mri_copy_vol_geom_to_mri(mri, dst);
    }
    Ok(NO_ERROR)
}

/// `TransformSourceVoxelToAtlas(transform, mri, xv, yv, zv, ...)`: a voxel-to-voxel array's first
/// matrix applied to the voxel (`TransformWithMatrix`); another linear type ends the program
/// (`RFAsourceVoxelToNode: needs vox-to-vox transform`, `ERROR_BADPARM`); a morph's
/// [`transform_sample`] widened to `double`. `NO_ERROR`.
#[allow(clippy::too_many_arguments)]
pub fn transform_source_voxel_to_atlas(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    t: &mut Transform,
    xv: i32,
    yv: i32,
    zv: i32,
) -> Result<(i32, [f64; 3]), Fatal> {
    if !t.is_morph() {
        if t.ttype != LINEAR_VOX_TO_VOX {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "RFAsourceVoxelToNode: needs vox-to-vox transform",
            )?;
        }
        let lt = first(g, err, t.lta(), "TransformSourceVoxelToAtlas")?;
        let m = lt.m_l.as_ref().expect("a transform's matrix");
        let p = transform_with_matrix(m, f64::from(xv), f64::from(yv), f64::from(zv));
        return Ok((NO_ERROR, p));
    }
    let (_, p) = transform_sample(g, out, err, env, t, xv as f32, yv as f32, zv as f32)?;
    Ok((
        NO_ERROR,
        [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])],
    ))
}

/// `TransformWrite(transform, fname)` (4140): a GCA morph through `GCAMwrite` (its default data
/// format, absolute voxel coordinates), a linear transform array through `LTAwriteEx` (the name's
/// type decides the format: `.lta`, `.xfm`, `register.dat`, FSL's `.mat`); their status.
pub fn transform_write(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    t: &mut Transform,
    fname: &str,
) -> Result<i32, Fatal> {
    match &mut t.xform {
        Xform::Morph(gcam) => crate::gcam_write::gcam_write(
            g,
            out,
            err,
            env,
            dir,
            gcam,
            fname,
            crate::gcam_write::WARPFIELD_DTFMT_ABS_CRS,
        ),
        Xform::Lta(lta) => lta_write_ex(g, out, err, env, dir, lta, fname),
    }
}

/// `TransformCopy(tsrc, tdst)` (4154): the type, copies of the three inverse index volumes (into
/// the destination's own when it has them), and the transform (`GCAMcopy`, FS-875; `LTAcopy`,
/// whose arrays of another count give FreeSurfer's message and no transform: `None`). FS-876: a
/// destination of the other kind is not used (FreeSurfer copies into its memory), noted.
pub fn transform_copy(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Transform,
    dst: Option<Transform>,
) -> Result<Option<Transform>, Fatal> {
    let morph = matches!(src.xform, Xform::Morph(_));
    let dst = match dst {
        Some(d) if matches!(d.xform, Xform::Morph(_)) != morph => {
            g.notices.add(
                &FS_876,
                "TransformCopy: a transform copied into one of the other kind (FreeSurfer copies into its memory)",
            );
            None
        }
        d => d,
    };
    let (mut xn, mut yn, mut zn, mut xform) = match dst {
        Some(d) => (d.mri_xn, d.mri_yn, d.mri_zn, Some(d.xform)),
        None => (None, None, None, None),
    };
    for (s, d) in [
        (&src.mri_xn, &mut xn),
        (&src.mri_yn, &mut yn),
        (&src.mri_zn, &mut zn),
    ] {
        *d = match s {
            None => None,
            Some(s) => match mri_copy(g, out, err, s, d.as_mut())? {
                Ret::New(m) => Some(m),
                Ret::Given => d.take(),
                Ret::Null => None,
            },
        };
    }
    let xform = match (&src.xform, xform.take()) {
        (Xform::Morph(s), Some(Xform::Morph(mut d))) => {
            gcam_copy(g, out, err, s, Some(&mut d))?;
            Xform::Morph(d)
        }
        (Xform::Morph(s), _) => match gcam_copy(g, out, err, s, None)? {
            Ret::New(m) => Xform::Morph(Box::new(m)),
            _ => return Ok(None),
        },
        (Xform::Lta(s), Some(Xform::Lta(mut d))) => match lta_copy(g, out, err, s, Some(&mut d))? {
            Ret::Given => Xform::Lta(d),
            _ => return Ok(None),
        },
        (Xform::Lta(s), _) => match lta_copy(g, out, err, s, None)? {
            Ret::New(l) => Xform::Lta(l),
            _ => return Ok(None),
        },
    };
    let mut t = Transform::new(src.ttype, xform);
    t.mri_xn = xn;
    t.mri_yn = yn;
    t.mri_zn = zn;
    Ok(Some(t))
}

/// `TransformApplyType(transform, mri_src, mri_dst, interp_type)` (2660): a linear transform array
/// through `LTAtransformInterp` (into `mri_dst` when given); a GCA morph through
/// `GCAMmorphToAtlas(mri_src, gcam, mri_dst, -1, interp_type)` (every frame:
/// [`gcam_morph_to_atlas`]).
#[allow(clippy::too_many_arguments)]
pub fn transform_apply_type(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    t: &mut Transform,
    src: &Mri,
    dst: Option<&mut Mri>,
    interp: i32,
) -> Result<Ret<Mri>, Fatal> {
    match &mut t.xform {
        Xform::Morph(gcam) => gcam_morph_to_atlas(g, out, err, env, src, gcam, dst, -1, interp),
        Xform::Lta(lta) => lta_transform_interp(g, out, err, env, src, dst, lta, interp),
    }
}

/// `TransformApply(transform, mri_src, mri_dst)` (2678): a new volume (`mri_dst` is not used): a
/// linear transform array through `LTAtransform` (trilinear); a GCA morph through
/// `GCAMmorphToAtlas(mri_src, gcam, NULL, -1, SAMPLE_TRILINEAR)`.
pub fn transform_apply(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    t: &mut Transform,
    src: &Mri,
) -> Result<Ret<Mri>, Fatal> {
    match &mut t.xform {
        Xform::Morph(gcam) => {
            gcam_morph_to_atlas(g, out, err, env, src, gcam, None, -1, SAMPLE_TRILINEAR)
        }
        Xform::Lta(lta) => lta_transform(g, out, err, env, src, None, lta),
    }
}

/// `TransformCreateDensityMap(transform, mri_src, mri_dst)` (4122): a GCA morph through
/// `GCAMmorphToAtlasWithDensityCorrection(mri_src, gcam, mri_dst, 0)` (the first frame, each voxel
/// times the morph's Jacobian: [`gcam_morph_to_atlas_with_density_correction`]); a linear
/// transform array: [`transform_apply`] (`mri_dst` not used), then `scaling volume by <det>...`
/// (`%2.3f` of the first transform's determinant, a `float`) on standard output and the volume
/// multiplied by it in place.
#[allow(clippy::too_many_arguments)]
pub fn transform_create_density_map(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    t: &mut Transform,
    src: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    if let Xform::Morph(gcam) = &mut t.xform {
        return gcam_morph_to_atlas_with_density_correction(
            g, out, err, env, dir, src, gcam, dst, 0,
        );
    }
    let r = transform_apply(g, out, err, env, t, src)?;
    let lta = t.lta();
    let Some(m_l) = first(g, err, lta, "TransformCreateDensityMap")?.m_l.clone() else {
        // MatrixDeterminant of no matrix (a transform array always has its matrices)
        return Err(Fatal::Exit(139));
    };
    let det = matrix_determinant(g, err, &m_l);
    let _ =
        out.write_all(format!("scaling volume by {}...\n", fmt_f(f64::from(det), 2, 3)).as_bytes());
    match r {
        Ret::New(mut m) => {
            mri_scalar_mul(g, out, err, Pair::Same(&mut m), det)?;
            Ok(Ret::New(m))
        }
        // no volume (a transform that cannot be applied): FreeSurfer scales NULL (FS-928)
        _ => {
            refused_928(
                g,
                err,
                "TransformCreateDensityMap of a transform that could not be applied",
            );
            Ok(Ret::Null)
        }
    }
}

/// `TransformApplyInverseType(transform, mri_src, mri_dst, interp_type)` (2717): a linear transform
/// array: its inverses filled (`LTAfillInverse`: a matrix that cannot be inverted ends the program)
/// and `LTAinverseTransformInterp` (into `mri_dst` when given); a GCA morph through
/// `GCAMmorphFromAtlas(mri_src, gcam, NULL, interp_type)` (a new volume, `mri_dst` not used:
/// [`gcam_morph_from_atlas`]).
#[allow(clippy::too_many_arguments)]
pub fn transform_apply_inverse_type(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    t: &mut Transform,
    src: &Mri,
    dst: Option<&mut Mri>,
    interp: i32,
) -> Result<Ret<Mri>, Fatal> {
    match &mut t.xform {
        Xform::Morph(gcam) => gcam_morph_from_atlas(g, out, err, env, dir, src, gcam, None, interp),
        Xform::Lta(lta) => {
            lta_fill_inverse(g, out, err, lta)?;
            lta_inverse_transform_interp(g, out, err, env, src, dst, lta, interp)
        }
    }
}

/// `TransformApplyInverse(transform, mri_src, mri_dst)` (2699): a linear transform array as
/// [`transform_apply_inverse_type`] with trilinear interpolation; a GCA morph through
/// `GCAMmorphFromAtlas(mri_src, gcam, NULL, SAMPLE_NEAREST)`.
#[allow(clippy::too_many_arguments)]
pub fn transform_apply_inverse(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    t: &mut Transform,
    src: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    if let Xform::Morph(gcam) = &mut t.xform {
        return gcam_morph_from_atlas(g, out, err, env, dir, src, gcam, None, SAMPLE_NEAREST);
    }
    transform_apply_inverse_type(g, out, err, env, dir, t, src, dst, SAMPLE_TRILINEAR)
}

/// FS-928: FreeSurfer dereferences a null pointer or uses a structure of the other kind here (a
/// crash or memory corruption): both modes stop with `what` as an error.
fn refused_928(g: &mut Globals, err: &mut dyn Write, what: &str) {
    g.notices.add(
        &FS_928,
        format!("{what}: FreeSurfer crashes or writes another structure's memory; refused"),
    );
    log::error_printf(
        g,
        err,
        ERROR_BADPARM,
        &format!("{what} (FreeSurfer crashes here)\n"),
    );
}

/// `TransformRas2Vox(transform, mri_src, mri_dst)` (4535): a GCA morph through `GCAMrasToVox(gcam,
/// mri_src)`; a linear transform array becomes `LINEAR_VOX_TO_VOX` and goes through
/// `LTArasToVoxelXform(lta, mri_src, mri_dst)` (which needs `mri_src` unless the array is already
/// of voxels: FS-928 without it). Their status.
pub fn transform_ras2vox(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    t: &mut Transform,
    src: Option<&Mri>,
    dst: Option<&Mri>,
) -> Result<i32, Fatal> {
    if !matches!(t.xform, Xform::Morph(_)) {
        t.ttype = LINEAR_VOX_TO_VOX;
    }
    match &mut t.xform {
        Xform::Morph(gcam) => crate::gcam::gcam_ras_to_vox(g, out, err, env, gcam, src),
        Xform::Lta(lta) => match src {
            Some(s) => lta_ras_to_voxel_xform(g, out, err, lta, s, dst),
            None if lta.ltype == LINEAR_VOX_TO_VOX => Ok(NO_ERROR),
            None => {
                refused_928(
                    g,
                    err,
                    "TransformRas2Vox of a linear transform without a source volume",
                );
                Ok(ERROR_BADPARM)
            }
        },
    }
}

/// `TransformVox2Ras(transform, mri_src, mri_dst)` (4544): a GCA morph through `GCAMvoxToRas`; a
/// linear transform array becomes `LINEAR_RAS_TO_RAS` and goes through `LTAvoxelToRasXform(lta,
/// mri_src, mri_dst)`. Their status.
pub fn transform_vox2ras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: &mut Transform,
    src: Option<&Mri>,
    dst: Option<&Mri>,
) -> Result<i32, Fatal> {
    if !matches!(t.xform, Xform::Morph(_)) {
        t.ttype = LINEAR_RAS_TO_RAS;
    }
    match &mut t.xform {
        Xform::Morph(gcam) => crate::gcam::gcam_vox_to_ras(g, out, err, gcam),
        Xform::Lta(lta) => lta_voxel_to_ras_xform(g, out, err, lta, src, dst),
    }
}

/// `TransformCompose(t_src, m_left, m_right, t_dst)` (4858) into another transform: with no `t_dst`,
/// a new one (`TransformAlloc(t_src's type, NULL)`: for a morph `TransformAlloc(MORPH_3D_TYPE):
/// unsupported`); a GCA morph: `TransformCompose: MORPH_3D_TYPE unsupported` (`ERROR_UNSUPPORTED`)
/// and `Null`; a linear transform array through `LTAcompose(lta_src, m_left, m_right, lta_dst)`.
/// FS-928: a destination that holds a GCA morph (FreeSurfer writes into it as if it were an array),
/// refused. For `t_dst` the source itself: [`transform_compose_in_place`].
#[allow(clippy::too_many_arguments)]
pub fn transform_compose(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t_src: &Transform,
    m_left: Option<&Matrix>,
    m_right: Option<&Matrix>,
    t_dst: Option<&mut Transform>,
) -> Result<Ret<Transform>, Fatal> {
    let mut fresh = None;
    let given = t_dst.is_some();
    let dst: Option<&mut Transform> = match t_dst {
        Some(d) => Some(d),
        None => match transform_alloc(g, out, err, t_src.ttype, None)? {
            Some(d) => Some(fresh.insert(d)),
            None => None,
        },
    };
    let Xform::Lta(lta_src) = &t_src.xform else {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            "TransformCompose: MORPH_3D_TYPE unsupported",
        );
        return Ok(Ret::Null);
    };
    let Some(dst) = dst else {
        return Ok(Ret::Null);
    };
    let Xform::Lta(lta_dst) = &mut dst.xform else {
        refused_928(
            g,
            err,
            "TransformCompose of a linear transform into a GCA morph",
        );
        return Ok(Ret::Null);
    };
    lta_compose(g, out, err, lta_src, m_left, m_right, Some(lta_dst))?;
    Ok(match fresh {
        Some(d) => Ret::New(d),
        None if given => Ret::Given,
        None => Ret::Null,
    })
}

/// `TransformCompose(t, m_left, m_right, t)` (4858) into the transform itself: a GCA morph:
/// `TransformCompose: MORPH_3D_TYPE unsupported` and `false` (FreeSurfer's `NULL`); a linear
/// transform array through `LTAcompose` in place (`lta_compose_in_place`); `true`.
pub fn transform_compose_in_place(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: &mut Transform,
    m_left: Option<&Matrix>,
    m_right: Option<&Matrix>,
) -> Result<bool, Fatal> {
    let Xform::Lta(lta) = &mut t.xform else {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            "TransformCompose: MORPH_3D_TYPE unsupported",
        );
        return Ok(false);
    };
    lta_compose_in_place(g, out, err, lta, m_left, m_right)?;
    Ok(true)
}

/// `TransformConcat(trxArray, numTrx)` (5026): the composition of the transforms, the first
/// applied to images first: a copy of the last ([`transform_copy`]), then each earlier one
/// composed into it. A morph after a morph: `GCAMconcat2(earlier, current, current)`
/// ([`gcam_concat2_into_second`]); a linear array before a morph: `GCAMconcat3(earlier, current,
/// NULL)`; a morph before a linear array: `GCAMconcat3(NULL, earlier, current)`, the transform
/// becoming a morph; two linear arrays: `LTAconcat2(earlier, current, 0)` (no array: `ERROR:
/// TransformConcat(): LTAs do not match`, the program ends; the transform keeps its type). The
/// earlier transforms can change (a morph in RAS is taken to voxels, a linear array inverted to
/// match). No transform: `TransformConcat(): no transform passed`, the program ends.
pub fn transform_concat(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    trx: &mut [Transform],
) -> Result<Transform, Fatal> {
    let Some((last, rest)) = trx.split_last_mut() else {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "TransformConcat(): no transform passed\n",
        )?;
        return Err(Fatal::Exit(1));
    };
    let Some(mut o) = transform_copy(g, out, err, last, None)? else {
        return Err(Fatal::Exit(139));
    };
    for next in rest.iter_mut().rev() {
        let (o_morph, n_morph) = (o.is_morph(), next.is_morph());
        match (o_morph, n_morph) {
            (true, true) => {
                crate::concat::gcam_concat2_into_second(
                    g,
                    out,
                    err,
                    env,
                    next.gcam_mut(),
                    o.gcam_mut(),
                )?;
            }
            (true, false) => {
                let m = crate::concat::gcam_concat3(
                    g,
                    out,
                    err,
                    env,
                    Some(next.lta()),
                    o.gcam_mut(),
                    None,
                )?;
                o.xform = Xform::Morph(Box::new(m));
            }
            (false, true) => {
                let m = crate::concat::gcam_concat3(
                    g,
                    out,
                    err,
                    env,
                    None,
                    next.gcam_mut(),
                    Some(o.lta()),
                )?;
                o.xform = Xform::Morph(Box::new(m));
                o.ttype = MORPH_3D_TYPE;
            }
            (false, false) => {
                let r = lta_concat2(g, out, err, env, next.lta_mut(), o.lta_mut(), false)?;
                match r {
                    Some(l) => o.xform = Xform::Lta(l),
                    None => {
                        log::error_exit(
                            g,
                            err,
                            ERROR_BADPARM,
                            "ERROR: TransformConcat(): LTAs do not match",
                        )?;
                    }
                }
            }
        }
    }
    Ok(o)
}

/// `TransformInvertReplace(transform, mri)` (5073): the transform replaced by its inverse. A GCA
/// morph: its image geometry first taken from `mri` when given (`getVolGeom`), then
/// `GCAMfillInverse` ([`crate::concat::gcam_fill_inverse`]; FS-936 when it makes no morph); a
/// linear array: `LTAinvert(lta, NULL)` (the transform's type kept).
#[allow(clippy::too_many_arguments)]
pub fn transform_invert_replace(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    t: &mut Transform,
    mri: Option<&Mri>,
) -> Result<(), Fatal> {
    match &mut t.xform {
        Xform::Morph(gcam) => {
            if let Some(m) = mri {
                get_vol_geom(&m.vg, &mut gcam.image);
            }
            match crate::concat::gcam_fill_inverse(g, out, err, env, dir, gcam)? {
                Some(inv) => t.xform = Xform::Morph(Box::new(inv)),
                None => crate::concat::no_inverse(g, err)?,
            }
        }
        Xform::Lta(lta) => {
            if let Ret::New(l) = lta_invert(g, out, err, InvertInto::New(lta))? {
                t.xform = Xform::Lta(l);
            }
        }
    }
    Ok(())
}
