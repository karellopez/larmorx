// SPDX-License-Identifier: Apache-2.0
//! The probe's commands of §55 to §57 (`specs/freesurfer-volume.md`: segmentations onto another
//! grid, SPM warps, the codes' names; segment borders, statistics, indices, concatenation,
//! edits, shifts, the entorhinal fix, the amygdala-cortex junction; crops; the commands and
//! encodings: `validation/investigations/freesurfer-volume-probe/README.md`).

use larmorx_fs_core::mri::Mri;
use larmorx_fs_core::volume::aseg::{float2int_code, interpolation_code};
use larmorx_fs_core::volume::{Dst, VolOut};
use larmorx_fs_core::{Process, Stop};

use super::args::Line;
use super::bspline::getlta;
use super::{D, dst, getdst, getw, paint, putlta, putres, putv};

/// A painted W.
fn getwp(p: &mut Process, l: &mut Line) -> Result<Mri, Stop> {
    let mut m = getw(p, l)?;
    paint(p, l, &mut m)?;
    Ok(m)
}

/// `new` (none) or a W.
fn new_or_w(p: &mut Process, l: &mut Line) -> Result<Option<Mri>, Stop> {
    if l.take("new") {
        return Ok(None);
    }
    Ok(Some(getw(p, l)?))
}

/// The labels kept by a crop: `n`, then `n` integers.
fn getkeep(l: &mut Line) -> Vec<i32> {
    let n = l.geti();
    (0..n.max(0)).map(|_| l.geti()).collect()
}

/// One of §55 to §57's commands, or `false` when `c` is none.
pub(super) fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        // -- §55
        // `a2vtwice` (not the probe's: the defect tests' FS-267): two `a2v` calls in one process
        "a2v" | "a2vtwice" => {
            for _ in 0..if c == "a2v" { 1 } else { 2 } {
                let a = getwp(p, l)?;
                let m = l.getm(p)?;
                let t = getw(p, l)?;
                let ft = l.getd();
                let ct = l.getct(false);
                let r = p.mri_aseg2vol(&a, m.as_ref(), &t, ft, ct.as_ref())?;
                p.flush();
                putv(l, r.as_ref().map(|x| &x.0));
                putv(l, r.as_ref().map(|x| &x.1));
            }
        }
        "a2vmu" => {
            let mut a = getwp(p, l)?;
            let mut lta = getlta(p, l)?;
            let ft = l.getd();
            let usf = l.geti();
            let ct = l.getct(false);
            let r = p.mri_aseg2vol_mu(&mut a, &mut lta, ft, usf, ct.as_ref())?;
            p.flush();
            putv(l, r.as_ref().map(|x| &x.0));
            putv(l, r.as_ref().map(|x| &x.1));
            putlta(l, Some(&lta));
        }
        "spmw" => {
            let v = getwp(p, l)?;
            let mut lta = if l.take("null") {
                None
            } else {
                Some(getlta(p, l)?)
            };
            let w = getwp(p, l)?;
            let (lr, interp) = (l.geti(), l.geti());
            let mut d = match new_or_w(p, l)? {
                None => D::New,
                Some(m) => D::W(Box::new(m)),
            };
            let r = p.mri_apply_spm_warp(&v, lta.as_mut(), &w, lr != 0, interp, dst(&mut d))?;
            p.flush();
            putres(l, r, &v, Some(&w), &d);
            putlta(l, lta.as_ref());
        }
        "icode" => {
            let s = l.getx();
            l.puti(i64::from(interpolation_code(&s)));
            l.puti(i64::from(float2int_code(&s)));
        }
        // -- §56
        "sbound" => {
            let a = getwp(p, l)?;
            let r = p.mri_seg_boundary(&a)?;
            putv(l, Some(&r));
        }
        "sborder" => {
            let a = getwp(p, l)?;
            let topo = l.geti();
            let mut ml = -7;
            let r = p.mri_seg_border(&a, &mut ml, topo)?;
            p.flush();
            l.puti(i64::from(ml));
            putv(l, Some(&r));
        }
        "idxno" => {
            let mut a = getwp(p, l)?;
            let mut d = getdst(p, l, false)?;
            let r = p.mri_index_no(&mut a, dst(&mut d))?;
            putres(l, r, &a, None, &d);
        }
        "ssrob" => {
            let s = getwp(p, l)?;
            let m = getwp(p, l)?;
            let (id, f) = (l.geti(), l.geti());
            let pct = l.getf();
            let mut out = [-7f32; 5];
            let n = p.mri_seg_stats_robust(&s, id, &m, f, pct, &mut out)?;
            l.puti(i64::from(n));
            for x in out {
                l.putf(x);
            }
        }
        "sfavg" => {
            let s = getwp(p, l)?;
            let m = getwp(p, l)?;
            let id = l.geti();
            let mut av = vec![-7f64; m.nframes.max(1) as usize];
            let n = p.mri_seg_frame_avg(&s, id, &m, &mut av)?;
            l.puti(i64::from(n));
            for x in av.iter().take(m.nframes.max(0) as usize) {
                l.putd(*x);
            }
        }
        "zcat" => {
            let mut a = getwp(p, l)?;
            let mut b = getwp(p, l)?;
            let nskip = l.geti();
            let mut d = getdst(p, l, true)?;
            let r = p.mri_zconcat(&mut a, &mut b, nskip, dst(&mut d))?;
            p.flush();
            putres(l, r, &a, Some(&b), &d);
        }
        "aedit" => {
            let mut n = getwp(p, l)?;
            let mut o = getwp(p, l)?;
            let m = getwp(p, l)?;
            let mut d = getdst(p, l, true)?;
            let r = p.mri_apply_edits(&mut n, &mut o, &m, dst(&mut d))?;
            p.flush();
            putres(l, r, &n, Some(&o), &d);
        }
        "shdim" => {
            let a = getwp(p, l)?;
            let (dim, ns, wrap) = (l.geti(), l.geti(), l.geti());
            let r = p.mri_shift_dim(&a, dim, ns, wrap)?;
            p.flush();
            putv(l, r.as_ref());
        }
        "fento" => {
            let mut a = getwp(p, l)?;
            let e = getwp(p, l)?;
            let level = l.geti();
            let (lh, rh) = (l.getd(), l.getd());
            let acj = l.geti();
            let n = p.mri_fix_ento_wm(&mut a, &e, level, lh, rh, acj)?;
            p.flush();
            l.puti(i64::from(n));
            putv(l, Some(&a));
        }
        "lacj" => {
            let a = getwp(p, l)?;
            let topo = l.geti();
            let mut d = match new_or_w(p, l)? {
                None => D::New,
                Some(m) => D::W(Box::new(m)),
            };
            // the new output's colour table draws its colours: a fixed seed first, as the probe
            p.set_random_seed(1);
            let given = match &mut d {
                D::W(m) => Some(m.as_mut()),
                _ => None,
            };
            let r = p.label_amygala_cortal_junction(&a, topo, given)?;
            p.flush();
            putres(l, r, &a, None, &d);
        }
        // -- §57
        "cfov" => {
            let a = getwp(p, l)?;
            let fov = [l.geti(), l.geti(), l.geti()];
            let th = l.getd();
            let r = p.mri_crop_to_fov(&a, fov, th)?;
            p.flush();
            for v in [r.x, r.y, r.z, r.dx, r.dy, r.dz] {
                l.puti(i64::from(v));
            }
        }
        "ccrs" | "crras" => {
            let mut a = getwp(p, l)?;
            if c == "crras" {
                a.geom.valid = l.geti();
            }
            let ctr = [l.getd(), l.getd(), l.getd()];
            let fov = [l.geti(), l.geti(), l.geti()];
            let keep = getkeep(l);
            let lta = if c == "crras" && !l.take("null") {
                Some(getlta(p, l)?)
            } else {
                None
            };
            let r = if c == "ccrs" {
                p.mri_crop_around_crs(&mut a, ctr, fov, &keep)?
            } else {
                p.mri_crop_around_ras(&mut a, ctr, fov, lta.as_ref(), &keep)?
            };
            p.flush();
            putv(l, r.as_ref());
            if lta.is_some() {
                putlta(l, lta.as_ref());
            }
        }
        _ => return Ok(false),
    }
    let _ = (Dst::New, VolOut::Nothing);
    Ok(true)
}
