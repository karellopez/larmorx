// SPDX-License-Identifier: Apache-2.0
//! The probe's volume commands: allocation, sizes and intents (§4), voxels (§5), the header and
//! full copies (§6, §13, §14), pulse parameters, colour tables (§7), names and shapes (§12, §15).

use larmorx_fs_core::maths::Returned;
use larmorx_fs_core::mri::names::{interp_code, interp_string, precision_code, precision_string};
use larmorx_fs_core::mri::volume::{intent_code, intent_name};
use larmorx_fs_core::mri::{VolGeom, sizeof};
use larmorx_fs_core::{Process, Stop};

use super::args::{Line, bad};

/// Runs command `c` when it is one of this module's; whether it was.
pub fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        "alloc" => {
            let m = l.getv(p)?;
            l.put_volume(&m);
            let mut m = Some(m);
            l.puti(i64::from(p.mri_free(&mut m)));
            l.puti(i64::from(m.is_none()));
        }
        "allocvg" | "fromvg" => {
            let mut vg = VolGeom::default();
            l.getg(&mut vg, true);
            let (kind, nf, hdr) = (l.geti(), l.geti(), l.geti());
            let m = p.mri_from_vol_geom(&vg, kind, nf, hdr != 0)?;
            l.put_volume(&m);
            if c == "fromvg" {
                let mut m = Some(m);
                l.puti(i64::from(p.mri_free(&mut m)));
            }
        }
        "free2" => {
            let mut m = Some(l.getv(p)?);
            l.puti(i64::from(p.mri_free(&mut m)));
            l.puti(i64::from(p.mri_free(&mut m)));
        }
        "sizeof" => {
            let k = l.geti();
            l.puti(sizeof(k) as i64);
        }
        "intent" => {
            let code = l.geti();
            l.puts(Some(intent_name(code).as_bytes()));
            let x = l.getx();
            l.puti(i64::from(intent_code(&x)));
        }
        "vox" => {
            let mut m = l.getv(p)?;
            let seed = l.geti();
            Line::fill(&mut m, seed);
            m.outside_val = l.getd();
            let n = l.geti();
            for _ in 0..n.max(0) {
                let op = l.tok().to_owned();
                let (col, row, sl, f) = (l.geti(), l.geti(), l.geti(), l.geti());
                match op.as_str() {
                    "get" => {
                        let mode = l.geti();
                        let v = p.mri_get_vox_val(&m, col, row, sl, f, mode)?;
                        l.putf(v);
                    }
                    "set" => {
                        let mode = l.geti();
                        let v = l.getf();
                        let r = p.mri_set_vox_val(&mut m, col, row, sl, f, v, mode)?;
                        l.puti(i64::from(r));
                    }
                    "dx" | "dy" | "dz" => {
                        let axis = usize::from(op.as_bytes()[1] - b'x');
                        let v = p.mri_get_vox_d(&m, axis, col, row, sl, f)?;
                        l.putf(v);
                    }
                    _ => bad("unknown voxel operation"),
                }
            }
            l.putvox(&m);
        }
        "copyhdr" | "copy" => {
            let mut src = l.getv(p)?;
            let seed = l.geti();
            Line::fill(&mut src, seed);
            l.seth(p, &mut src)?;
            let mut dst = None;
            if !l.take("none") {
                let mut d = l.getv(p)?;
                let seed = l.geti();
                Line::fill(&mut d, seed);
                if c == "copyhdr" {
                    l.seth(p, &mut d)?;
                } else {
                    d.ptype = l.geti();
                }
                dst = Some(d);
            }
            let r = if c == "copyhdr" {
                p.mri_copy_header(&src, dst.as_mut())?
            } else {
                p.mri_copy(&src, dst.as_mut())?
            };
            let res = match r {
                Returned::Nothing => {
                    l.out("null");
                    None
                }
                Returned::Output => {
                    l.out("given");
                    dst
                }
                Returned::New(m) => {
                    l.out("new");
                    Some(m)
                }
            };
            if let Some(m) = res {
                l.puthdr(&m);
                l.putd(m.mean);
                l.putct(m.ct.as_ref(), false);
                if c == "copy" {
                    l.putvox(&m);
                }
            }
        }
        "copyxfm" => {
            let mut src = p.mri_alloc_header(2, 2, 2, 0, 1)?;
            l.getxfm(&mut src);
            let mut dst = None;
            if !l.take("new") {
                let mut d = p.mri_alloc_header(2, 2, 2, 0, 1)?;
                l.getxfm(&mut d);
                dst = Some(d);
            }
            let r = p.mri_copy_header(&src, dst.as_mut())?;
            let res = match r {
                Returned::New(m) => {
                    l.out("new");
                    m
                }
                _ => {
                    l.out("given");
                    dst.expect("the given volume")
                }
            };
            l.putxfm(&res);
        }
        "pulse" => {
            let mut a = p.mri_alloc_header(1, 1, 1, 0, 1)?;
            let mut b = p.mri_alloc_header(1, 1, 1, 0, 1)?;
            a.flip_angle = l.getd();
            a.tr = l.getf();
            a.te = l.getf();
            a.ti = l.getf();
            l.puti(i64::from(p.copy_pulse_parameters(&a, &mut b)));
            l.putd(b.flip_angle);
            l.putf(b.tr);
            l.putf(b.te);
            l.putf(b.ti);
        }
        "ctab" => {
            let ct = l.getct(false);
            let counts = l.geti();
            let mut cp = p.ctab_deep_copy(ct.as_ref());
            l.putct(cp.as_ref(), counts != 0);
            l.puti(i64::from(p.ctab_free(&mut cp)));
            l.puti(i64::from(cp.is_none()));
        }
        "ctabnull" => {
            let mut none = None;
            l.puti(i64::from(p.ctab_free(&mut none)));
            let cp = p.ctab_deep_copy(None);
            l.putct(cp.as_ref(), false);
        }
        "codes" => {
            let a = l.getx();
            let ca = l.geti();
            let b = l.getx();
            let cb = l.geti();
            l.puti(i64::from(interp_code(&a)));
            l.puts(interp_string(ca).map(str::as_bytes));
            l.puti(i64::from(precision_code(&b)));
            l.puts(precision_string(cb).map(str::as_bytes));
        }
        "shape" => {
            let a: Vec<i32> = (0..4).map(|_| l.geti()).collect();
            let b: Vec<i32> = (0..4).map(|_| l.geti()).collect();
            let sa = p.mri_shape(a[0], a[1], a[2], a[3]);
            let sb = p.mri_shape(b[0], b[1], b[2], b[3]);
            l.puti(i64::from(sa == sb));
            l.puti(i64::from(sa != sb));
        }
        _ => return Ok(false),
    }
    Ok(true)
}
