// SPDX-License-Identifier: Apache-2.0
//! The probe's commands of part 10a (`specs/freesurfer-volume.md` §46 and §48): the Talairach
//! helpers, dilation and the segment helpers (the commands and encodings:
//! `validation/investigations/freesurfer-volume-probe/README.md`).

use larmorx_fs_core::mri::{Lta, Mri};
use larmorx_fs_core::volume::talairach::{TalPoint, transform_with_matrix};
use larmorx_fs_core::volume::{Dst, VolOut};
use larmorx_fs_core::{Process, Stop};

use super::args::Line;
use super::bspline::getlta;
use super::{UNSETD, dst, getdst, getw, paint, putres, putv};

/// A painted W.
fn getwp(p: &mut Process, l: &mut Line) -> Result<Mri, Stop> {
    let mut m = getw(p, l)?;
    paint(p, l, &mut m)?;
    Ok(m)
}

/// `null` or an array.
fn null_or_lta(p: &mut Process, l: &mut Line) -> Result<Option<Lta>, Stop> {
    if l.take("null") {
        return Ok(None);
    }
    Ok(Some(getlta(p, l)?))
}

/// One of part 10a's commands, or `false` when `c` is none.
pub(super) fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        // -- §46
        "talcras" => {
            let mut m = getwp(p, l)?;
            let a = null_or_lta(p, l)?;
            p.flush();
            let r = p.modify_talairach_cras(&mut m, a.as_ref())?;
            p.flush();
            l.puti(i64::from(r));
            putv(l, Some(&m));
        }
        "talmat" => {
            let kind = l.geti();
            let m = getwp(p, l)?;
            let a = null_or_lta(p, l)?;
            p.flush();
            let r = match kind {
                0 => p.mtalairach_from_voxel(&m, a.as_ref())?,
                1 => p.mtal_voxel_from_voxel(&m, a.as_ref())?,
                2 => p.mvoxel_from_talairach(&m, a.as_ref())?,
                _ => p.mvoxel_from_tal_voxel(&m, a.as_ref())?,
            };
            p.flush();
            l.putm(r.as_ref());
        }
        "talpt" => {
            let kind = l.geti();
            let (mut m, mut mat, mut a) = (None, None, None);
            if kind == 4 {
                mat = l.getm(p)?;
            } else {
                m = Some(getwp(p, l)?);
                a = null_or_lta(p, l)?;
            }
            let n = l.geti();
            for _ in 0..n {
                let (x, y, z) = (l.getd(), l.getd(), l.getd());
                p.flush();
                let (r, pt) = if kind == 4 {
                    (
                        0,
                        mat.as_ref()
                            .and_then(|mm| transform_with_matrix(mm, x, y, z)),
                    )
                } else {
                    let k = match kind {
                        0 => TalPoint::VoxelToTalairach,
                        1 => TalPoint::VoxelToTalairachVoxel,
                        2 => TalPoint::TalairachToVoxel,
                        _ => TalPoint::TalairachVoxelToVoxel,
                    };
                    let v = m.as_ref().expect("a volume");
                    p.talairach_point(k, v, (x, y, z), a.as_ref())?
                };
                p.flush();
                l.puti(i64::from(r));
                let q = pt.unwrap_or([UNSETD; 3]);
                for v in q {
                    l.putd(v);
                }
            }
        }
        "talvol" => {
            let kind = l.geti();
            let mut m = getwp(p, l)?;
            let mut t = if l.take("new") {
                None
            } else {
                Some(getwp(p, l)?)
            };
            let a = null_or_lta(p, l)?;
            let interp = if kind == 0 { l.geti() } else { 0 };
            p.flush();
            let r = match (kind, t.as_mut()) {
                (2, t) => p.mri_from_talairach_ex(&mut m, t, a.as_ref())?,
                (k, t) => {
                    let d = match t {
                        Some(t) => Dst::Given(t),
                        None => Dst::New,
                    };
                    if k == 0 {
                        p.mri_to_talairach_ex_interp(&mut m, d, a.as_ref(), interp)?
                    } else {
                        p.mri_to_talairach_ex(&mut m, d, a.as_ref())?
                    }
                }
            };
            p.flush();
            match r {
                VolOut::Given => {
                    l.out("given");
                    putv(l, t.as_ref());
                }
                VolOut::New(v) => putv(l, Some(&v)),
                _ => putv(l, None),
            }
        }
        // -- §48
        "dilate" | "dilu" => {
            let mut a = getwp(p, l)?;
            let mut d = getdst(p, l, false)?;
            p.flush();
            let r = if c == "dilate" {
                p.mri_dilate(&mut a, dst(&mut d))?
            } else {
                p.mri_dilate_uchar(&mut a, dst(&mut d))?
            };
            p.flush();
            putres(l, r, &a, None, &d);
        }
        "dilo" => {
            let mut a = getwp(p, l)?;
            a.outside_val = l.getd();
            let mut d = getdst(p, l, false)?;
            p.flush();
            let r = p.mri_dilate(&mut a, dst(&mut d))?;
            p.flush();
            putres(l, r, &a, None, &d);
        }
        "msegs" => {
            let mut a = getwp(p, l)?;
            let mut d = getdst(p, l, false)?;
            let n = l.geti();
            let list: Vec<i32> = (0..n.max(0)).map(|_| l.geti()).collect();
            let id = l.geti();
            p.flush();
            let r = p.mri_match_segs(&mut a, &list, id, dst(&mut d))?;
            p.flush();
            putres(l, r, &a, None, &d);
        }
        "sstat" | "hseg" => {
            let s = getwp(p, l)?;
            let v = getwp(p, l)?;
            let id = l.geti();
            if c == "sstat" {
                let f = l.geti();
                p.flush();
                let (n, st) = p.mri_seg_stats(&s, id, &v, f)?;
                p.flush();
                l.puti(i64::from(n));
                for x in [st.min, st.max, st.range, st.mean, st.std] {
                    l.putf(x);
                }
            } else {
                let (lo, hi, dl) = (l.getd(), l.getd(), l.getd());
                p.flush();
                let h = p.histo_seg(&s, id, &v, lo, hi, dl)?;
                p.flush();
                l.puti(i64::from(h.max_bins));
                l.puti(i64::from(h.nbins));
                for x in [h.bin_size, h.min, h.max] {
                    l.putf(x);
                }
                for i in 0..h.nbins.max(0) {
                    l.putf(h.bin(i));
                    l.putf(h.count(i));
                }
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}
