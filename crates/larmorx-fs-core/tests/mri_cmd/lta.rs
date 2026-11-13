// SPDX-License-Identifier: Apache-2.0
//! The probe's bounding-box and linear-transform-array commands (§17-22).

use larmorx_fs_core::maths::Returned;
use larmorx_fs_core::mri::geom::fixed_name;
use larmorx_fs_core::mri::{LinearTransform, Lta, Region};
use larmorx_fs_core::{Process, Stop};

use super::args::{Line, bad};

/// LT: `x0 y0 z0 sigma m_L m_dL m_last_dL type label src dst` (each geometry Gf).
fn getlt(p: &mut Process, l: &mut Line, lt: &mut LinearTransform) -> Result<(), Stop> {
    lt.x0 = l.getf();
    lt.y0 = l.getf();
    lt.z0 = l.getf();
    lt.sigma = l.getf();
    lt.m_l = l.getm(p)?;
    lt.m_dl = l.getm(p)?;
    lt.m_last_dl = l.getm(p)?;
    lt.kind = l.geti();
    lt.label = l.geti();
    l.getg_full(p, &mut lt.src)?;
    l.getg_full(p, &mut lt.dst)?;
    Ok(())
}

fn putlt(l: &mut Line, lt: &LinearTransform) {
    l.putf(lt.x0);
    l.putf(lt.y0);
    l.putf(lt.z0);
    l.putf(lt.sigma);
    l.putm(lt.m_l.as_ref());
    l.putm(lt.m_dl.as_ref());
    l.putm(lt.m_last_dl.as_ref());
    l.puti(i64::from(lt.kind));
    l.puti(i64::from(lt.label));
    l.putg(&lt.src, true);
    l.putg(&lt.dst, true);
}

/// LTA: `<n> <type> <fscale> <subject>`, then the transforms and inverse transforms, over
/// `LTAalloc(n, none)`.
fn getlta(p: &mut Process, l: &mut Line) -> Result<Lta, Stop> {
    let n = l.geti();
    let mut lta = p.lta_alloc(n, None)?;
    lta.kind = l.geti();
    lta.fscale = l.getf();
    lta.subject = fixed_name(&l.getx(), 1024);
    for i in 0..lta.xforms.len() {
        getlt(p, l, &mut lta.xforms[i])?;
    }
    for i in 0..lta.inv_xforms.len() {
        getlt(p, l, &mut lta.inv_xforms[i])?;
    }
    Ok(lta)
}

fn putlta(l: &mut Line, lta: Option<&Lta>) {
    let Some(lta) = lta else {
        l.out("null");
        return;
    };
    l.puti(i64::from(lta.num_xforms()));
    l.puti(i64::from(lta.kind));
    l.putf(lta.fscale);
    l.puts(Some(&lta.subject));
    for lt in lta.xforms.iter().chain(&lta.inv_xforms) {
        putlt(l, lt);
    }
}

/// Runs command `c` when it is one of this module's; whether it was.
pub fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        "bbox" => {
            let m = l.getbv(p)?;
            let thresh = l.geti();
            let mut b = Region {
                x: -7,
                y: -7,
                z: -7,
                dx: -7,
                dy: -7,
                dz: -7,
            };
            let r = p.mri_bounding_box(&m, thresh, &mut b)?;
            l.puti(i64::from(r));
            for v in [b.x, b.y, b.z, b.dx, b.dy, b.dz] {
                l.puti(i64::from(v));
            }
        }
        "ltaalloc" => {
            let n = l.geti();
            let m = if l.take("none") {
                None
            } else {
                Some(l.getbv(p)?)
            };
            let lta = p.lta_alloc(n, m.as_ref())?;
            putlta(l, Some(&lta));
            let mut lta = Some(lta);
            l.puti(i64::from(p.lta_free(&mut lta)));
            l.puti(i64::from(lta.is_none()));
        }
        "ltacopy" => {
            let src = getlta(p, l)?;
            let mut dst = if l.take("none") {
                None
            } else {
                Some(getlta(p, l)?)
            };
            let r = p.lta_copy(&src, dst.as_mut())?;
            match r {
                Returned::Nothing => {
                    l.out("null");
                    l.out("null");
                    if let Some(d) = &dst {
                        putlta(l, Some(d));
                    }
                }
                Returned::Output => {
                    l.out("given");
                    putlta(l, dst.as_ref());
                }
                Returned::New(m) => {
                    l.out("new");
                    putlta(l, Some(&m));
                    if let Some(d) = &dst {
                        putlta(l, Some(d));
                    }
                }
            }
        }
        "ltcopy" => {
            let a = getlta(p, l)?;
            let none = l.take("null");
            let mut b = if none { None } else { Some(getlta(p, l)?) };
            if a.num_xforms() < 1 || b.as_ref().is_some_and(|b| b.num_xforms() < 1) {
                bad("ltcopy needs a transform on each side");
            }
            let dst = b.as_mut().map(|b| &mut b.xforms[0]);
            let given = dst.is_some();
            match p.lt_copy(&a.xforms[0], dst) {
                None => l.out("null"),
                Some(()) => {
                    l.out(if given { "given" } else { "other" });
                    putlt(l, &b.as_ref().expect("the destination").xforms[0]);
                }
            }
        }
        "ltafreenull" => {
            let mut none = None;
            l.puti(i64::from(p.lta_free(&mut none)));
        }
        _ => return Ok(false),
    }
    Ok(true)
}
