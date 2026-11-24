// SPDX-License-Identifier: Apache-2.0
//! The probe's commands of part 11a (`specs/freesurfer-volume.md` §59, §60) and of §64's and
//! §65's functions of `mri_cc` (the commands and the encodings M, BF, DF and VL:
//! `validation/investigations/freesurfer-volume-probe/README.md`).

use larmorx_fs_core::mri::Mri;
use larmorx_fs_core::volume::VolOut;
use larmorx_fs_core::volume::binvol::{BinarizeMri, DeMorphBinVol, FixSubCortMassHa};
use larmorx_fs_core::volume::fill::FillOut;
use larmorx_fs_core::volume::vlist::VoxelList;
use larmorx_fs_core::{Process, Stop};

use super::args::Line;
use super::bspline::getlta;
use super::{D, dst, getdst, getreg, getw, paint, putlta, putres, putv};

/// A painted W.
fn getwp(p: &mut Process, l: &mut Line) -> Result<Mri, Stop> {
    let mut m = getw(p, l)?;
    paint(p, l, &mut m)?;
    Ok(m)
}

/// M, a volume or none: `null`, or W P.
fn getwn(p: &mut Process, l: &mut Line) -> Result<Option<Mri>, Stop> {
    if l.take("null") {
        return Ok(None);
    }
    Ok(Some(getwp(p, l)?))
}

/// BF, a `BinarizeMRI`'s fields.
fn getbin(l: &mut Line) -> BinarizeMri {
    let mut b = BinarizeMri {
        bin_type: l.geti(),
        thmin: l.getd(),
        thmax: l.getd(),
        ..BinarizeMri::default()
    };
    let n = l.geti();
    for _ in 0..n {
        b.matchlist.push(l.geti());
    }
    b.maskthmin = l.getd();
    b.maskthmax = l.getd();
    b.zero_col_edges = l.geti();
    b.zero_row_edges = l.geti();
    b.zero_slice_edges = l.geti();
    b.on_val = l.getd();
    b.off_val = l.getd();
    b.fstart = l.geti();
    b.fend = l.geti();
    b.invert = l.geti();
    b.mritype = l.geti();
    b.do_abs = l.geti();
    b.m_debug = l.geti();
    b
}

/// The fields a binarize call may change: `fstart fend nhits OnVal OffVal`.
fn putbin(l: &mut Line, b: &BinarizeMri) {
    l.puti(i64::from(b.fstart));
    l.puti(i64::from(b.fend));
    l.puti(i64::from(b.nhits));
    l.putd(b.on_val);
    l.putd(b.off_val);
}

/// DF, a `DEMorphBinVol`'s fields.
fn getdem(l: &mut Line) -> DeMorphBinVol {
    DeMorphBinVol {
        morphtype: l.geti(),
        topo: l.geti(),
        nnbrsthresh: l.geti(),
        nmorph: l.geti(),
        frame: l.geti(),
        mm_mritype: l.geti(),
        mm_debug: l.geti(),
        ..DeMorphBinVol::default()
    }
}

/// VL, a voxel list: `null`, or `<alloc> <nvox>` (`VLSTalloc(alloc)`, the patterns, `nvox`
/// set); with the allocation (0 for none).
pub(super) fn getvl(p: &mut Process, l: &mut Line) -> Result<(Option<VoxelList>, i32), Stop> {
    if l.take("null") {
        return Ok((None, 0));
    }
    let n = l.geti();
    let nv = l.geti();
    let mut vl = p.vlst_alloc(n)?;
    for i in 0..n.max(0) as usize {
        let f = i as f32;
        vl.xi[i] = 1000 + i as i32;
        vl.yi[i] = 2000 + i as i32;
        vl.zi[i] = 3000 + i as i32;
        vl.fi[i] = 4000 + i as i32;
        vl.xd[i] = f + 0.5;
        vl.yd[i] = f + 0.25;
        vl.zd[i] = f + 0.125;
        vl.vsrc[i] = -1.0 - f;
        vl.vdst[i] = -2.0 - f;
    }
    vl.nvox = nv;
    Ok((Some(vl), n))
}

/// A voxel list (printed): `null`, or `max_vox nvox type`, `self` (its volume is the call's)
/// or `none`, then the first `len` voxels.
pub(super) fn putvl(l: &mut Line, vl: Option<&VoxelList>, len: usize) {
    let Some(vl) = vl else {
        l.out("null");
        return;
    };
    l.puti(i64::from(vl.max_vox));
    l.puti(i64::from(vl.nvox));
    l.puti(i64::from(vl.kind));
    l.out(if vl.has_mri { "self" } else { "none" });
    for i in 0..len {
        l.puti(i64::from(vl.xi[i]));
        l.puti(i64::from(vl.yi[i]));
        l.puti(i64::from(vl.zi[i]));
        l.puti(i64::from(vl.fi[i]));
        l.putf(vl.xd[i]);
        l.putf(vl.yd[i]);
        l.putf(vl.zd[i]);
        l.putf(vl.vsrc[i]);
        l.putf(vl.vdst[i]);
    }
}

/// A given output or none, as a destination.
fn given_d(o: Option<Mri>) -> D {
    match o {
        Some(m) => D::W(Box::new(m)),
        None => D::New,
    }
}

/// One of part 11a's commands, or `false` when `c` is none.
pub(super) fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        // -- §59
        "vvfill" => {
            let mut a = getwp(p, l)?;
            let m = getwn(p, l)?;
            let lta = getlta(p, l)?;
            let (usf, cons) = (l.geti(), l.geti());
            let mut d = if l.take("new") {
                D::New
            } else {
                D::W(Box::new(getw(p, l)?))
            };
            p.flush();
            let out = match &mut d {
                D::W(o) => FillOut::Given(o),
                _ => FillOut::New,
            };
            let r = p.mri_vol2vol_fill(&mut a, m.as_ref(), &lta, usf, cons, out)?;
            p.flush();
            putres(l, r, &a, None, &d);
            putlta(l, Some(&lta));
        }
        "fbnwm" => {
            let mut a = getwp(p, l)?;
            let mut b = getwp(p, l)?;
            p.flush();
            let r = p.mri_find_bright_non_wm(&mut a, &mut b)?;
            p.flush();
            putres(l, VolOut::New(r), &a, Some(&b), &D::New);
        }
        "qecc" => {
            let a = getwp(p, l)?;
            let m = getwn(p, l)?;
            let (x, y, z) = (l.geti(), l.geti(), l.geti());
            p.flush();
            let v = p.quad_euler_char_change(&a, m.as_ref(), x, y, z)?;
            p.flush();
            l.puti(i64::from(v));
        }
        "vtero" => {
            let mut a = getwp(p, l)?;
            let m = getwn(p, l)?;
            p.flush();
            let v = p.mri_vol_topo_erode_one(&mut a, m.as_ref())?;
            p.flush();
            l.puti(i64::from(v));
            putv(l, Some(&a));
        }
        "vtdil" => {
            let mut a = getwp(p, l)?;
            let on = l.geti();
            p.flush();
            let v = p.mri_vol_topo_dilate_one(&mut a, on)?;
            p.flush();
            l.puti(i64::from(v));
            putv(l, Some(&a));
        }
        // -- §60
        "fscm" => {
            let mut scm = getwp(p, l)?;
            let aseg = getwp(p, l)?;
            let mut f = FixSubCortMassHa {
                n_dilate: l.geti(),
                mask: None,
            };
            p.flush();
            let v = p.fix_scm(&mut f, &mut scm, &aseg)?;
            p.flush();
            l.puti(i64::from(v));
            putv(l, Some(&scm));
            putv(l, f.mask.as_ref());
        }
        "binmri" => {
            let mut b = getbin(l);
            let k = l.geti();
            for _ in 0..k {
                let mut a = getwn(p, l)?;
                let m = getwn(p, l)?;
                let mut d = getdst(p, l, false)?;
                p.flush();
                let r = p.binarize(&mut b, a.as_mut(), m.as_ref(), dst(&mut d))?;
                p.flush();
                match &a {
                    Some(av) => putres(l, r, av, None, &d),
                    None => {
                        // no input: the result is none; a given destination's state after it
                        putv(l, None);
                        if let D::W(g) = &d {
                            putv(l, Some(g));
                        }
                    }
                }
                putbin(l, &b);
            }
        }
        "binq" => {
            let b = getbin(l);
            let a = getwp(p, l)?;
            let m = getwn(p, l)?;
            let (x, y, z, f) = (l.geti(), l.geti(), l.geti(), l.geti());
            p.flush();
            let v = p.binarize_qualifies(&b, &a, m.as_ref(), x, y, z, f)?;
            p.flush();
            l.puti(i64::from(v));
        }
        "bindump" => {
            let b = getbin(l);
            let m = getwn(p, l)?;
            p.flush();
            let v = p.binarize_dump(&b, m.as_ref());
            p.flush();
            l.puti(i64::from(v));
        }
        "demchk" => {
            let d = getdem(l);
            let a = getwn(p, l)?;
            let m = getwn(p, l)?;
            let o = getwn(p, l)?;
            p.flush();
            let v = p.dem_check(&d, a.as_ref(), m.as_ref(), o.as_ref());
            p.flush();
            l.puti(i64::from(v));
        }
        "demdump" => {
            let d = getdem(l);
            p.flush();
            let v = p.dem_dump(&d);
            p.flush();
            l.puti(i64::from(v));
        }
        "demcf" => {
            let d = getdem(l);
            let a = getwp(p, l)?;
            let fr = l.geti();
            p.flush();
            let r = p.dem_copy_frame(&d, &a, fr)?;
            p.flush();
            putres(l, r.map_or(VolOut::Nothing, VolOut::New), &a, None, &D::New);
        }
        "demm" | "demt" | "deml" => {
            let mut d = getdem(l);
            let a = getwp(p, l)?;
            let (r, o) = if c == "demm" {
                p.flush();
                (p.dem_morph(&mut d, &a)?, None)
            } else {
                let (mut lo, mut hi) = (0.0, 0.0);
                let mut ml = Vec::new();
                if c == "demt" {
                    lo = l.getd();
                    hi = l.getd();
                } else {
                    let n = l.geti();
                    for _ in 0..n {
                        ml.push(l.geti());
                    }
                }
                let m = getwn(p, l)?;
                let o = getwn(p, l)?;
                p.flush();
                let list = if c == "deml" {
                    Some(ml.as_slice())
                } else {
                    None
                };
                let r = p.dem_morph_binarized(&mut d, &a, (lo, hi), list, m.as_ref(), o.as_ref())?;
                (r, o)
            };
            p.flush();
            putres(l, r.map_or(VolOut::Nothing, VolOut::New), &a, None, &given_d(o));
            l.puti(i64::from(d.nchangestot));
        }
        // -- §64, §65 (mri_cc's)
        "trns" => {
            let k = l.geti();
            let t = l.getm(p)?;
            let mut o = l.getm(p)?;
            p.flush();
            let e = if k == 0 {
                p.trns_extract_scale_matrix(t.as_ref(), o.as_mut())?
            } else {
                p.trns_extract_rotation_matrix(t.as_ref(), o.as_mut())?
            };
            p.flush();
            l.puti(i64::from(e));
            l.putm(o.as_ref());
            l.putm(t.as_ref());
        }
        "vlcir" => {
            let a = getwp(p, l)?;
            let (lo, hi) = (l.getf(), l.getf());
            let (vl, alloc) = getvl(p, l)?;
            let (skip, border) = (l.geti(), l.geti());
            let Some(r) = getreg(l) else {
                return Err(Stop::Exit(1));
            };
            let given = vl.is_some();
            p.flush();
            let res = p.vlst_create_in_region(&a, lo, hi, vl, skip, border, &r)?;
            p.flush();
            l.out(if given { "given" } else { "new" });
            let mut len = res.nvox.max(0) as usize;
            if given && alloc as usize > len {
                len = alloc as usize;
            }
            putvl(l, Some(&res), len);
        }
        _ => return Ok(false),
    }
    Ok(true)
}
