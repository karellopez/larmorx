// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/transform.cpp`'s `LTAtransformTypeName` (4182), `LTAdumpLinearTransform` (4239) and
//! `LTAdump` (4265): the report `mri_convert` prints with `--diag` before a linear transform is
//! applied (queue item 3.2-07's transform.cpp lines, taken with 3.2-05). `LTAdumpVolGeom` is
//! fs.volume's (`ltawrite::lta_dump_vol_geom`).

use larmorx_core::cfmt;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_mri::lta::{
    FSLREG_TYPE, LINEAR_CORONAL_RAS_TO_CORONAL_RAS, LINEAR_PHYSVOX_TO_PHYSVOX, LINEAR_RAS_TO_RAS,
    LINEAR_VOX_TO_VOX, LinearTransform, Lta, MATLAB_ASCII_TYPE, MNI_TRANSFORM_TYPE, MORPH_3D_TYPE,
    REGISTER_DAT, TRANSFORM_ARRAY_TYPE,
};
use larmorx_freesurfer_volume::ltawrite::lta_dump_vol_geom;

use super::options::Run;

/// `LTAtransformTypeName(ltatype)`: the type's name (`unknown` for a code it does not name).
pub fn lta_transform_type_name(ltatype: i32) -> &'static str {
    match ltatype {
        LINEAR_VOX_TO_VOX => "linear_vox_to_vox",
        LINEAR_RAS_TO_RAS => "linear_ras_to_ras",
        LINEAR_PHYSVOX_TO_PHYSVOX => "linear_physvox_to_physvox",
        TRANSFORM_ARRAY_TYPE => "transform_array",
        MORPH_3D_TYPE => "morph_3d",
        MNI_TRANSFORM_TYPE => "mni_transform",
        MATLAB_ASCII_TYPE => "matlab_ascii",
        LINEAR_CORONAL_RAS_TO_CORONAL_RAS => "linear_cor_to_cor",
        REGISTER_DAT => "register.dat",
        FSLREG_TYPE => "FSL",
        _ => "unknown",
    }
}

/// `LTAdumpLinearTransform(fp, lt)`: its centre, `sigma` (`%f`), type, type name and label, the
/// matrix (`MatrixPrint` between `m_L Matrix ----------` and a rule, when it has one), the source
/// and destination geometries (`LTAdumpVolGeom`). 0.
pub fn lta_dump_linear_transform(g: &mut Globals, r: &mut Run, lt: &LinearTransform) -> i32 {
    let f = |x: f32| cfmt::f(f64::from(x), 6);
    let out = &mut *r.out;
    let _ = writeln!(out, "x0 {}", f(lt.x0));
    let _ = writeln!(out, "y0 {}", f(lt.y0));
    let _ = writeln!(out, "z0 {}", f(lt.z0));
    let _ = writeln!(out, "sigma {}", f(lt.sigma));
    let _ = writeln!(out, "type {}", lt.ltype);
    let _ = writeln!(out, "typename {}", lta_transform_type_name(lt.ltype));
    let _ = writeln!(out, "label {}", lt.label);
    if let Some(m) = &lt.m_l {
        let _ = writeln!(r.out, "m_L Matrix ----------");
        let mut sink = std::io::sink();
        let out: &mut dyn Write = &mut *r.out;
        larmorx_freesurfer_core::matrix_ops::matrix_print(
            g,
            Some(out),
            &mut sink,
            r.err,
            Some(m),
            5,
        );
        let _ = writeln!(r.out, "---------------------");
    }
    let _ = writeln!(r.out, "Source Geometry ----------");
    lta_dump_vol_geom(r.out, &lt.src);
    let _ = writeln!(r.out, "Destination Geometry ----------");
    lta_dump_vol_geom(r.out, &lt.dst);
    0
}

/// `LTAdump(fp, lta)`: `num_xforms`, the type and its name, then each transform after
/// `nthxform <n> =====================================`. 0.
pub fn lta_dump(g: &mut Globals, r: &mut Run, lta: &Lta) -> i32 {
    let _ = writeln!(r.out, "num_xforms {}", lta.num_xforms);
    let _ = writeln!(r.out, "type {}", lta.ltype);
    let _ = writeln!(r.out, "typename {}", lta_transform_type_name(lta.ltype));
    for n in 0..lta.num_xforms.max(0) {
        let _ = writeln!(r.out, "nthxform {n} =====================================");
        if let Some(lt) = lta.xforms.get(n as usize) {
            lta_dump_linear_transform(g, r, lt);
        }
    }
    0
}

use std::io::Write;
