// SPDX-License-Identifier: Apache-2.0
//! The probe's geometry commands: the scanner matrix and its helpers (§3), the geometry's copy,
//! assignment and volume copies (§2, §4, §6), the sphinx position (§9), printing and comparisons
//! (§11).

use larmorx_fs_core::maths::Returned;
use larmorx_fs_core::maths::matrix::Matrix;
use larmorx_fs_core::mri::VolGeom;
use larmorx_fs_core::{Process, Stop};

use super::args::Line;

/// A returned matrix: `given` when it is the given matrix itself, then its state.
pub fn putm_res(l: &mut Line, r: &Returned<Matrix>, given: Option<&Matrix>) {
    match r {
        Returned::Output => {
            l.out("given");
            l.putm(given);
        }
        Returned::New(m) => l.putm(Some(m)),
        Returned::Nothing => l.out("null"),
    }
}

/// Runs command `c` when it is one of this module's; whether it was.
pub fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        "xfm" => {
            let m = l.getvg(p)?;
            let (base, shear) = (l.geti(), l.geti());
            let given = l.getm(p)?;
            let s = p.vox2ras_scanner(&m.geom, base, shear != 0)?;
            l.putm(Some(&s));
            for k in 0..3 {
                let mut g = given.clone();
                let r = match k {
                    0 => p.matrix_of_direction_cosines(&m.geom, g.as_mut())?,
                    1 => p.matrix_of_voxel_sizes(&m.geom, g.as_mut())?,
                    _ => p.matrix_of_translations(&m.geom, g.as_mut())?,
                };
                putm_res(l, &r, g.as_ref());
            }
            let i2r = p.extract_i_to_r(&m.geom)?;
            l.putm(Some(&i2r));
            let r2i = p.extract_r_to_i(&m.geom)?;
            l.putm(r2i.as_ref());
        }
        "setv2r" => {
            let mut m = l.getvg(p)?;
            let v = l.getm(p)?;
            let Some(v) = v else {
                super::args::bad("setv2r needs a matrix");
            };
            let r = p.set_vox2ras_from_matrix(&mut m.geom, &v)?;
            l.puti(i64::from(r));
            l.putg(&m.geom, false);
        }
        "p0" => {
            let mut m = l.getvg(p)?;
            let (r0, a0, s0) = (l.getd(), l.getd(), l.getd());
            let r = p.p0_to_cras(&mut m.geom, r0, a0, s0)?;
            l.puti(i64::from(r));
            l.putg(&m.geom, false);
        }
        "reinit" => {
            let mut m = l.getvg(p)?;
            let r = p.reinit_cache(&mut m.geom)?;
            l.puti(i64::from(r));
            l.putg(&m.geom, false);
        }
        "vgcopy" => {
            let mut a = VolGeom::default();
            l.getg_full(p, &mut a)?;
            let b = a.copied();
            l.putg(&b, true);
        }
        "vgassign" => {
            let mut a = VolGeom::default();
            let mut b = VolGeom::default();
            l.getg_full(p, &mut a)?;
            l.getg_full(p, &mut b)?;
            p.vg_assign(&mut b, &a);
            l.putg(&b, true);
        }
        "usevg" | "vgtomri" => {
            let mut vg = VolGeom::default();
            l.getg(&mut vg, true);
            let mut m = l.getvg(p)?;
            if c == "usevg" {
                p.use_vol_geom(&vg, &mut m)?;
            } else {
                let r = p.copy_vol_geom_to_mri(&mut m, &vg);
                l.puti(i64::from(r));
            }
            l.putg(&m.geom, false);
        }
        "vgfrommri" => {
            let m = l.getvg(p)?;
            let mut vg = VolGeom::default();
            l.getg(&mut vg, true);
            let r = p.copy_vol_geom_from_mri(&m, &mut vg);
            l.puti(i64::from(r));
            l.putg(&vg, true);
        }
        "sphinx" => {
            let mut vg = VolGeom::default();
            l.getg(&mut vg, true);
            let r = vg.hfs_to_sphinx();
            l.puti(i64::from(r));
            l.putg(&vg, true);
        }
        "vgprint" => {
            let mut vg = VolGeom::default();
            l.getg(&mut vg, true);
            let (nocheck, fmt) = (l.geti(), l.geti());
            p.vg_print(&vg, nocheck != 0);
            if fmt != 0 {
                p.geom_print(&vg, Some(b"geometry of 7:\n"));
            } else {
                p.geom_print(&vg, None);
            }
        }
        "vgeq" => {
            let mut a = VolGeom::default();
            let mut b = VolGeom::default();
            l.getg(&mut a, true);
            l.getg(&mut b, true);
            let t = l.getd();
            let debug = l.geti() != 0;
            let sform = l.getm(p)?;
            let eq = p.vg_equal(&a, &b, t);
            l.puti(i64::from(eq));
            l.puti(i64::from(VolGeom::not_equal_code(&a, &b, t)));
            let cg = p.vg_check_geom(&a, &b, t, debug)?;
            l.puti(i64::from(cg));
            let cv = p.vg_check_vox2ras(&a, &b, t, debug, sform.as_ref())?;
            l.puti(i64::from(cv));
        }
        "diffv2r" => {
            let a = l.getm(p)?;
            let b = l.getm(p)?;
            let t = l.getd();
            let msg = l.getx();
            let debug = l.geti() != 0;
            let (Some(a), Some(b)) = (a, b) else {
                super::args::bad("diffv2r needs two matrices");
            };
            let r = p.vg_diff_vox2ras(&a, &b, t, &msg, debug);
            l.puti(i64::from(r));
        }
        "shearless" => {
            let mut m = l.getvg(p)?;
            p.shearless_components(&mut m.geom)?;
            l.putg(&m.geom, false);
        }
        "tomat" => {
            let mut vg = VolGeom::default();
            l.getg(&mut vg, true);
            let mut g = l.getm(p)?;
            let r = p.vg_to_matrix(&vg, g.as_mut())?;
            putm_res(l, &r, g.as_ref());
        }
        _ => return Ok(false),
    }
    Ok(true)
}
