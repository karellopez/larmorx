// SPDX-License-Identifier: Apache-2.0
//! The probe's tkregister, FSL and coordinate commands (§9, §10).

use larmorx_fs_core::maths::matrix::Matrix;
use larmorx_fs_core::{Process, Stop};

use super::args::Line;
use super::geo::putm_res;

fn put_opt(l: &mut Line, m: Option<Matrix>) {
    l.putm(m.as_ref());
}

/// Runs command `c` when it is one of this module's; whether it was.
pub fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        "tkreg" => {
            let m = l.getvg(p)?;
            let (base, shear) = (l.geti(), l.geti() != 0);
            let g = &m.geom;
            let k = p.vox2ras_tkreg(g)?;
            l.putm(Some(&k));
            let f = p.vox2ras_fsl(g);
            l.putm(Some(&f));
            let v = p.vox2ras_scanner(g, base, shear)?;
            l.putm(Some(&v));
            let v = p.vox2ras_scanner(g, base, shear)?;
            let iv = p.inverse_of(&v)?;
            put_opt(l, iv);
            let k = p.vox2ras_tkreg(g)?;
            l.putm(Some(&k));
            let k = p.vox2ras_tkreg(g)?;
            let ik = p.inverse_of(&k)?;
            put_opt(l, ik);
            let r2t = p.vg_ras_to_tkreg(g)?;
            put_opt(l, r2t);
            let t2r = p.vg_tkreg_to_ras(g)?;
            put_opt(l, t2r);
        }
        "ras2s" => {
            let m = l.getvg(p)?;
            let given = l.getm(p)?;
            let mut g = given.clone();
            let r = p.tkreg_to_scanner(&m.geom, g.as_mut())?;
            putm_res(l, &r, g.as_ref());
            let t2r = p.vg_tkreg_to_ras(&m.geom)?;
            put_opt(l, t2r);
            let r2t = p.vg_ras_to_tkreg(&m.geom)?;
            put_opt(l, r2t);
            let s = p.scanner_to_tkreg(&m.geom)?;
            put_opt(l, s);
        }
        "tk2nat" | "nat2tk" | "v2v" | "tkv2v" => {
            let a = l.getvg(p)?;
            let b = l.getvg(p)?;
            let r = l.getm(p)?;
            let res = match c {
                "tk2nat" => p.tkreg_to_native(&a.geom, &b.geom, r.as_ref())?,
                "nat2tk" => p.tkreg_from_native(&a.geom, &b.geom, r.as_ref())?,
                "v2v" => p.tkreg_from_vox2vox(&a.geom, &b.geom, r.as_ref())?,
                _ => p.vox2vox_from_tkreg(&a.geom, &b.geom, r.as_ref())?,
            };
            put_opt(l, res);
        }
        "fixtk" => {
            let m = l.getvg(p)?;
            let seed = i64::from(l.geti());
            let r = l.getm(p)?;
            p.drand48_state().srand48(seed);
            let res = p.fix_tkreg(&m.geom, r.as_ref())?;
            put_opt(l, res);
            let d = p.drand48_state().drand48();
            l.putd(d);
        }
        "fsl2tk" | "tk2fsl" => {
            if l.take("unset") {
                p.env.remove(b"FSLOUTPUTTYPE");
            } else {
                let v = l.getx();
                let end = v.iter().position(|&b| b == 0).unwrap_or(v.len());
                p.env.set(b"FSLOUTPUTTYPE", &v[..end]);
            }
            let a = l.getvg(p)?;
            let b = l.getvg(p)?;
            let r = l.getm(p)?;
            let res = if c == "fsl2tk" {
                p.fsl_to_tkreg(&a.geom, &b.geom, r.as_ref())?
            } else {
                p.tkreg_to_fsl(&a.geom, &b.geom, r.as_ref())?
            };
            put_opt(l, res);
        }
        "crs1" => {
            let mut q = l.getm(p)?;
            let r = p.crs1_to_crs0(q.as_mut());
            putm_res(l, &r, q.as_ref());
        }
        "v2w" => {
            let mut m = l.getvg(p)?;
            match l.geti() {
                1 => {
                    m.geom.i_to_r = None;
                    m.geom.r_to_i = None;
                }
                2 => {
                    p.reinit_cache(&mut m.geom)?;
                }
                _ => {}
            }
            let n = l.geti();
            for _ in 0..n.max(0) {
                let (x, y, z) = (l.getd(), l.getd(), l.getd());
                let v = if l.geti() != 0 {
                    p.voxel_to_world(&mut m.geom, x, y, z)?
                } else {
                    let v = p.world_to_voxel(&mut m.geom, x, y, z)?;
                    let i = p.world_to_voxel_index(&mut m.geom, x, y, z)?;
                    for k in i {
                        l.puti(i64::from(k));
                    }
                    v
                };
                for d in v {
                    l.putd(d);
                }
            }
            l.putg(&m.geom, false);
        }
        _ => return Ok(false),
    }
    Ok(true)
}
