// SPDX-License-Identifier: Apache-2.0
//! The probe's commands of part 10b (`specs/freesurfer-volume.md` §49 and §50): conforming, the
//! set operations, morphology, masks and counts (the commands and encodings:
//! `validation/investigations/freesurfer-volume-probe/README.md`).

use larmorx_fs_core::mri::Mri;
use larmorx_fs_core::{Process, Stop};

use super::args::Line;
use super::{D, dst, getdst, getw, paint, putres, putv};

/// A painted W.
fn getwp(p: &mut Process, l: &mut Line) -> Result<Mri, Stop> {
    let mut m = getw(p, l)?;
    paint(p, l, &mut m)?;
    Ok(m)
}

/// `null` or a painted W.
fn null_or_w(p: &mut Process, l: &mut Line) -> Result<Option<Mri>, Stop> {
    if l.take("null") {
        return Ok(None);
    }
    Ok(Some(getwp(p, l)?))
}

/// The probe's FNV-1a 64 hash of the bytes: the usual prime, from the start state
/// 1469598103934665603 (the usual offset basis 14695981039346656037 without its last digit;
/// observed on the probe's own outputs: one and four zero bytes, the ints 1 and 3, 1, all
/// from that one start state; `validation/investigations/freesurfer-cleanroom-volume/hand/`).
fn fnv1a64(b: &[u8]) -> u64 {
    let mut h: u64 = 1_469_598_103_934_665_603;
    for &x in b {
        h ^= u64::from(x);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// A printed volume whose voxels are `h` and their hash (`novox` without voxels).
fn putvh(l: &mut Line, m: Option<&Mri>) {
    let Some(m) = m else {
        l.out("null");
        return;
    };
    l.puthdr(m);
    l.putct(m.ct.as_ref(), true);
    match &m.voxels {
        None => l.out("novox"),
        Some(v) => l.out(format!("h{:016x}", fnv1a64(v))),
    }
}

/// One of part 10b's commands, or `false` when `c` is none.
pub(super) fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        // -- §49
        "conform" => {
            let k = l.geti();
            let mut a = getwp(p, l)?;
            match k {
                0 => {
                    p.flush();
                    let r = p.mri_conform(&a)?;
                    p.flush();
                    putvh(l, r.as_ref());
                }
                1 => {
                    p.flush();
                    let r = p.mri_get_conform_matrix(&mut a)?;
                    p.flush();
                    l.putm(r.as_ref());
                }
                _ => {
                    let (wd, sz, keep) = (l.geti(), l.getd(), l.geti());
                    p.flush();
                    let r = p.mri_conformed_template(&a, wd, sz, keep != 0)?;
                    p.flush();
                    putvh(l, r.as_ref());
                }
            }
            putvh(l, Some(&a));
        }
        // -- §50, two volumes into D
        "union" | "isect" | "mand" | "mxor" => {
            let mut a = getwp(p, l)?;
            let mut b = getwp(p, l)?;
            let mut d = getdst(p, l, true)?;
            let (t1, t2) = match c {
                "mand" => (l.geti(), 0),
                "mxor" => (l.geti(), l.geti()),
                _ => (0, 0),
            };
            p.flush();
            let r = match c {
                "union" => p.mri_union(&mut a, &mut b, dst(&mut d))?,
                "isect" => p.mri_intersect(&mut a, &mut b, dst(&mut d))?,
                "mand" => p.mri_and(&mut a, &mut b, dst(&mut d), t1)?,
                _ => p.mri_xor(&mut a, &mut b, dst(&mut d), t1, t2)?,
            };
            p.flush();
            putres(l, r, &a, Some(&b), &d);
        }
        "compl" | "erode" | "erode2d" | "dil6" | "mnot" | "b2c" => {
            let mut a = getwp(p, l)?;
            let mut d = getdst(p, l, false)?;
            p.flush();
            let r = match c {
                "compl" => p.mri_complement(&mut a, dst(&mut d))?,
                "erode" => p.mri_erode(&mut a, dst(&mut d))?,
                "erode2d" => p.mri_erode_2d(&mut a, dst(&mut d))?,
                "dil6" => p.mri_dilate6(&mut a, dst(&mut d))?,
                "mnot" => p.mri_not(&mut a, dst(&mut d))?,
                _ => p.mri_bin_mask_to_col(&mut a, dst(&mut d))?,
            };
            p.flush();
            putres(l, r, &a, None, &d);
        }
        "edges" => {
            let mut a = getwp(p, l)?;
            let v = l.getd();
            let mut d = getdst(p, l, false)?;
            p.flush();
            let r = p.mri_set_edges(&mut a, v, dst(&mut d))?;
            p.flush();
            putres(l, r, &a, None, &d);
        }
        "erodenn" | "erodeseg" => {
            let mut a = getwp(p, l)?;
            let mut d = getdst(p, l, false)?;
            let (i, j) = (l.geti(), l.geti());
            p.flush();
            let r = if c == "erodenn" {
                p.mri_erode_nn(&mut a, dst(&mut d), i, j)?
            } else {
                p.mri_erode_segmentation(&mut a, dst(&mut d), i, j)?
            };
            p.flush();
            putres(l, r, &a, None, &d);
        }
        "dilseg" => {
            let mut a = getwp(p, l)?;
            let mut d = getdst(p, l, false)?;
            let n = l.geti();
            let mask = null_or_w(p, l)?;
            let (f, t) = (l.geti(), l.getd());
            p.flush();
            let (r, nc) = p.mri_dilate_segmentation(&mut a, dst(&mut d), n, mask.as_ref(), f, t)?;
            p.flush();
            putres(l, r, &a, None, &d);
            l.puti(i64::from(nc.unwrap_or(7)));
        }
        "dtl" => {
            let k = l.geti();
            let mut a = getwp(p, l)?;
            let mut b = getwp(p, l)?;
            let mut d = getdst(p, l, true)?;
            let (lab, n, t) = (l.geti(), l.geti(), l.geti());
            p.flush();
            let r = if k == 0 {
                p.mri_dilate_thresh_label(&mut a, &mut b, dst(&mut d), lab, n, t)?
            } else {
                p.mri_dilate_inv_thresh_label(&mut a, &mut b, dst(&mut d), lab, n, t)?
            };
            p.flush();
            putres(l, r, &a, Some(&b), &d);
        }
        "dlab" => {
            let k = l.geti();
            let mut a = getwp(p, l)?;
            let mut d = getdst(p, l, false)?;
            let (lab, n) = (l.geti(), l.geti());
            p.flush();
            let r = if k == 0 {
                p.mri_dilate_label(&mut a, dst(&mut d), lab, n)?
            } else {
                p.mri_dilate_label_uchar(&mut a, dst(&mut d), lab, n)?
            };
            p.flush();
            putres(l, r, &a, None, &d);
        }
        "openn" => {
            let k = l.geti();
            let mut a = getwp(p, l)?;
            let mut d = getdst(p, l, false)?;
            let order = if k == 0 { l.geti() } else { 0 };
            p.flush();
            let r = match k {
                0 => p.mri_open_n(&mut a, dst(&mut d), order)?,
                1 => p.mri_open(&mut a, dst(&mut d))?,
                _ => p.mri_close(&mut a, dst(&mut d))?,
            };
            p.flush();
            putres(l, r, &a, None, &d);
        }
        "rlist" => {
            let mut a = getwp(p, l)?;
            let n = l.geti();
            let pairs: Vec<(i32, i32)> = (0..n.max(0)).map(|_| (l.geti(), l.geti())).collect();
            let mut mask = null_or_w(p, l)?;
            let two = mask.is_some();
            let mut d = getdst(p, l, two)?;
            p.flush();
            let r = p.mri_replace_list(&mut a, &pairs, mask.as_mut(), dst(&mut d))?;
            p.flush();
            putres(l, r, &a, mask.as_ref(), &d);
        }
        "rvonly" => {
            let mut a = getwp(p, l)?;
            let mut d = getdst(p, l, false)?;
            let (i, o) = (l.getf(), l.getf());
            p.flush();
            let r = p.mri_replace_values_only(&mut a, dst(&mut d), i, o)?;
            p.flush();
            putres(l, r, &a, None, &d);
        }
        "mmask" | "tmask" => {
            let k = if c == "mmask" { l.geti() } else { 2 };
            let mut a = getwp(p, l)?;
            let mut b = getwp(p, l)?;
            let mut d = getdst(p, l, true)?;
            p.flush();
            let r = if k == 2 {
                let (t, o) = (l.getf(), l.getf());
                p.mri_threshold_mask(&mut a, &mut b, dst(&mut d), t, o)?
            } else {
                let (mv, o) = (l.geti(), l.getf());
                if k == 0 {
                    p.mri_mask(&mut a, &mut b, dst(&mut d), mv, o)?
                } else {
                    p.mri_mask_different_geometry(&mut a, &mut b, dst(&mut d), mv, o)?
                }
            };
            p.flush();
            putres(l, r, &a, Some(&b), &d);
        }
        "grow" => {
            let a = getwp(p, l)?;
            let mut b = getwp(p, l)?;
            let (i, o) = (l.geti(), l.geti());
            p.flush();
            let r = p.mri_grow_label(&a, &mut b, i, o)?;
            p.flush();
            l.puti(i64::from(r));
            putv(l, Some(&a));
            putv(l, Some(&b));
        }
        "onfg" => {
            let mut a = getwp(p, l)?;
            let b = getwp(p, l)?;
            let mut g = getwp(p, l)?;
            p.flush();
            let r = p.mri_turn_on_fg(&mut a, &b, &mut g)?;
            p.flush();
            l.puti(i64::from(r));
            putv(l, Some(&a));
            putv(l, Some(&b));
            putv(l, Some(&g));
        }
        "offbg" => {
            let mut a = getwp(p, l)?;
            let b = getwp(p, l)?;
            p.flush();
            let r = p.mri_turn_off_bg(&mut a, &b)?;
            p.flush();
            l.puti(i64::from(r));
            putv(l, Some(&a));
            putv(l, Some(&b));
        }
        "cplab" => {
            let mut a = getwp(p, l)?;
            let mut d = getdst(p, l, false)?;
            let lab = l.geti();
            p.flush();
            let n = p.mri_copy_label(&mut a, dst(&mut d), lab)?;
            p.flush();
            l.puti(i64::from(n));
            put_d(l, &a, None, &d);
        }
        "cplv" => {
            let mut a = getwp(p, l)?;
            let mut b = getwp(p, l)?;
            let mut d = getdst(p, l, true)?;
            let lab = l.geti();
            p.flush();
            let n = p.mri_copy_labeled_voxels(&mut a, &mut b, dst(&mut d), lab)?;
            p.flush();
            l.puti(i64::from(n));
            put_d(l, &a, Some(&b), &d);
        }
        "slv" => {
            let mut a = getwp(p, l)?;
            let mut b = getwp(p, l)?;
            let mut d = getdst(p, l, true)?;
            let (lab, v) = (l.geti(), l.getf());
            p.flush();
            let r = p.mri_set_label_values(&mut a, &mut b, dst(&mut d), lab, v)?;
            p.flush();
            putres(l, r, &a, Some(&b), &d);
        }
        "cntv" => {
            let k = l.geti();
            let a = getwp(p, l)?;
            let i = if k == 3 { 0 } else { l.geti() };
            p.flush();
            let n = match k {
                0 => p.mri_total_voxels_on(&a, i)?,
                1 => p.mri_voxels_in_label(&a, i)?,
                2 => p.mri_label_in_volume(&a, i)?,
                _ => p.mri_n_mask(&a)?,
            };
            p.flush();
            l.puti(i64::from(n));
        }
        "cnbhd" => {
            let a = getwp(p, l)?;
            let n = l.geti();
            for _ in 0..n.max(0) {
                let (w, x, y, z, v) = (l.geti(), l.geti(), l.geti(), l.geti(), l.geti());
                p.flush();
                let k = p.mri_count_val_in_nbhd(&a, w, (x, y, z), v)?;
                p.flush();
                l.puti(i64::from(k));
            }
        }
        "cent" => {
            let a = getwp(p, l)?;
            p.flush();
            let (r, c3) = p.mri_compute_centroid(&a)?;
            p.flush();
            l.puti(i64::from(r));
            for v in c3 {
                l.putd(v);
            }
        }
        "pthr" => {
            let a = getwp(p, l)?;
            let mask = null_or_w(p, l)?;
            let (f, pct) = (l.geti(), l.getd());
            p.flush();
            let t = p.mri_percent_thresh(&a, mask.as_ref(), f, pct)?;
            p.flush();
            l.putd(t);
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// The destination after a call that returns a count (the volume alone).
fn put_d(l: &mut Line, a: &Mri, b: Option<&Mri>, d: &D) {
    match d {
        D::W(m) => putv(l, Some(m)),
        D::Src => putv(l, Some(a)),
        D::Other => putv(l, b),
        D::New => putv(l, None),
    }
}
