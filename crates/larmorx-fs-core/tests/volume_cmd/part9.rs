// SPDX-License-Identifier: Apache-2.0
//! The probe's commands of part 9 (`specs/freesurfer-volume.md` §41 to §44, and §56's
//! `MRIvol2VolTkRegVSM`): volume-to-volume resampling, the trilinear kernel, the displacement
//! volume, a registration's array, the masked and upsampled source; regions and their boxes (the
//! commands and encodings: `validation/investigations/freesurfer-volume-probe/README.md`).

use larmorx_fs_core::mri::{Mri, Region};
use larmorx_fs_core::volume::region::{region_clear, region_copy};
use larmorx_fs_core::{Process, Stop};

use super::args::Line;
use super::bspline::getlta;
use super::{getw, paint, putlta, putv};

/// A painted W.
fn getwp(p: &mut Process, l: &mut Line) -> Result<Mri, Stop> {
    let mut m = getw(p, l)?;
    paint(p, l, &mut m)?;
    Ok(m)
}

/// `null` or a painted W.
fn null_or_wp(p: &mut Process, l: &mut Line) -> Result<Option<Mri>, Stop> {
    if l.take("null") {
        return Ok(None);
    }
    Ok(Some(getwp(p, l)?))
}

/// A region's six fields.
fn putreg(l: &mut Line, r: &Region) {
    for v in [r.x, r.y, r.z, r.dx, r.dy, r.dz] {
        l.puti(i64::from(v));
    }
}

/// One of part 9's commands, or `false` when `c` is none.
pub(super) fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        // -- §41, §56
        "vv" | "vvsm" | "vvtk" => {
            let mut s = getwp(p, l)?;
            s.outside_val = l.getd();
            let mut t = getwp(p, l)?;
            let m = l.getm(p)?;
            let interp = l.geti();
            let param = l.getf();
            let (mut vsm, mut lta, mut pedir) = (None, None, 0);
            if c != "vv" {
                if !l.take("null") {
                    let mut v = getwp(p, l)?;
                    v.outside_val = l.getd();
                    vsm = Some(v);
                }
                lta = if l.take("null") {
                    None
                } else {
                    Some(getlta(p, l)?)
                };
                pedir = l.geti();
            }
            p.flush();
            let r = match c {
                "vv" => p.mri_vol2vol(&s, &mut t, m.as_ref(), interp, param)?,
                "vvtk" => p.mri_vol2vol_tkreg_vsm(
                    &s,
                    &mut t,
                    m.as_ref(),
                    interp,
                    param,
                    vsm.as_ref(),
                    lta.as_ref(),
                    pedir,
                )?,
                _ => p.mri_vol2vol_vsm(
                    &s,
                    &mut t,
                    m.as_ref(),
                    interp,
                    param,
                    vsm.as_ref(),
                    lta.as_ref(),
                    pedir,
                )?,
            };
            p.flush();
            l.puti(i64::from(r));
            putv(l, Some(&t));
            if c != "vv" {
                putlta(l, lta.as_ref());
            }
        }
        "vvk" => {
            let s = getwp(p, l)?;
            let mut t = getwp(p, l)?;
            let m = l.getm(p)?;
            let r = p.mri_vol2vol_tl_kernel(&s, &mut t, m.as_ref())?;
            p.flush();
            l.puti(i64::from(r.is_none()));
            putv(l, Some(&t));
        }
        "vvd" => {
            let mv = getwp(p, l)?;
            let t = getwp(p, l)?;
            let m = l.getm(p)?;
            let r = p.mri_vol2vol_delta(&mv, &t, m.as_ref())?;
            p.flush();
            putv(l, r.as_ref());
        }
        "rd2lta" => {
            let t = getw(p, l)?;
            let mv = getw(p, l)?;
            let m = l.getm(p)?;
            let r = p.transform_regdat2lta(&t, &mv, m.as_ref())?;
            p.flush();
            putlta(l, Some(&r));
        }
        "maskup" => {
            let mut s = getwp(p, l)?;
            let mk = null_or_wp(p, l)?;
            let (up, npad, cons) = (l.geti(), l.geti(), l.geti());
            let r = p.mri_mask_and_upsample(&mut s, mk.as_ref(), up, npad, cons != 0)?;
            p.flush();
            match r {
                Some((v, a)) => {
                    putv(l, Some(&v));
                    putlta(l, Some(&a));
                }
                None => {
                    putv(l, None);
                    putlta(l, None);
                }
            }
        }
        // -- §43
        "rgbb" => {
            let m = getwp(p, l)?;
            let kind = l.geti();
            let r = match kind {
                0 => {
                    let npad = l.geti();
                    Some(p.region_get_bounding_box(&m, npad)?)
                }
                1 => {
                    let pads = [l.geti(), l.geti(), l.geti(), l.geti(), l.geti(), l.geti()];
                    Some(p.region_get_bounding_box_m(&m, pads)?)
                }
                _ => {
                    let npad = l.geti();
                    p.region_get_bounding_box_eq_odd(&m, npad)?
                }
            };
            p.flush();
            match r {
                Some(r) => putreg(l, &r),
                None => l.out("null"),
            }
        }
        "rgprt" => {
            let rg = Region {
                x: l.geti(),
                y: l.geti(),
                z: l.geti(),
                dx: l.geti(),
                dy: l.geti(),
                dz: l.geti(),
            };
            p.region_print(&rg);
            p.flush();
            let mut cp = region_copy(&rg, None);
            putreg(l, &cp);
            region_clear(&mut cp);
            putreg(l, &cp);
        }
        _ => return Ok(false),
    }
    Ok(true)
}
