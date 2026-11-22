// SPDX-License-Identifier: Apache-2.0
//! The probe's commands of fs.volume's part 8 (`specs/freesurfer-volume.md` §37 to §39; the
//! commands and encodings: `validation/investigations/freesurfer-volume-probe/README.md`).

use larmorx_fs_core::mri::Mri;
use larmorx_fs_core::volume::ltaread::FileSystem;
use larmorx_fs_core::volume::{Dst, VolOut};
use larmorx_fs_core::{Process, Stop};

use super::args::Line;
use super::{D, UNSET, UNSETD, dst, getdst, getreg, getw, paint, putres, putv};

/// A W that may be `null`, painted when `painted`.
fn getw_or_null(p: &mut Process, l: &mut Line, painted: bool) -> Result<Option<Mri>, Stop> {
    if l.take("null") {
        return Ok(None);
    }
    let mut m = getw(p, l)?;
    if painted {
        paint(p, l, &mut m)?;
    }
    Ok(Some(m))
}

/// A painted W.
fn getwp(p: &mut Process, l: &mut Line) -> Result<Mri, Stop> {
    let mut m = getw(p, l)?;
    paint(p, l, &mut m)?;
    Ok(m)
}

/// The result of a call that makes a new volume or fills the given one (the probe's `putres`
/// without sources): `given` and that volume, or the new volume (`null` for none) and then the
/// given volume's state.
fn putgen(l: &mut Line, r: VolOut, given: Option<&Mri>) {
    match r {
        VolOut::Given => {
            l.out("given");
            putv(l, given);
            return;
        }
        VolOut::New(m) => putv(l, Some(&m)),
        _ => putv(l, None),
    }
    if let Some(g) = given {
        putv(l, Some(g));
    }
}

/// One of part 8's commands, or `false` when `c` is none.
pub(super) fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        // -- §37
        "pvlab" => {
            let mut lab = getwp(p, l)?;
            let vals = getw_or_null(p, l, true)?;
            let (label, maxl) = (l.geti(), l.geti());
            let mut mix = getw_or_null(p, l, false)?;
            let mut nbr = getw_or_null(p, l, false)?;
            let sb = getw_or_null(p, l, true)?;
            let r = p.mri_voxels_in_label_with_partial_volume_effects(
                &mut lab,
                vals.as_ref(),
                label,
                mix.as_mut(),
                nbr.as_mut(),
                maxl,
                sb.as_ref(),
            )?;
            l.putf(r);
            putv(l, mix.as_ref());
            putv(l, nbr.as_ref());
        }
        // -- §38
        "randn" | "const" => {
            let n = [l.geti(), l.geti(), l.geti(), l.geti()];
            let a = l.getf();
            let b = if c == "randn" { l.getf() } else { 0.0 };
            let seed = if c == "randn" { l.geti() } else { 0 };
            let mut g = getw_or_null(p, l, false)?;
            let r = if c == "randn" {
                p.srand48(i64::from(seed));
                p.mri_randn(n[0], n[1], n[2], n[3], a, b, g.as_mut())?
            } else {
                p.mri_const(n[0], n[1], n[2], n[3], a, g.as_mut())?
            };
            putgen(l, r, g.as_ref());
        }
        "nseq" | "nfr" => {
            let mut a = getwp(p, l)?;
            let k = l.geti();
            let t = l.getf();
            let r = if c == "nseq" {
                p.mri_normalize_sequence(&mut a, t)?
            } else {
                p.mri_normalize_frames_kind(Some(&mut a), k.clamp(0, 2))?
            };
            l.puti(i64::from(r));
            putv(l, Some(&a));
        }
        "sort" => {
            let mut a = getwp(p, l)?;
            let mut m = getw_or_null(p, l, true)?;
            let mut d = getdst(p, l, m.is_some())?;
            let r = p.mri_sort(&mut a, m.as_mut(), dst(&mut d))?;
            putres(l, r, &a, m.as_ref(), &d);
        }
        "revsl" => {
            let mut a = getwp(p, l)?;
            a.bvals = l.getm(p)?;
            a.bvecs = l.getm(p)?;
            a.bvec_space = l.geti();
            let mut d = getdst(p, l, false)?;
            let r = p.mri_reverse_slice_order(&mut a, dst(&mut d))?;
            let tables = match (&r, &d) {
                (VolOut::New(m), _) => Some((m.bvals.clone(), m.bvecs.clone(), m.bvec_space)),
                (VolOut::Given, D::W(m)) => Some((m.bvals.clone(), m.bvecs.clone(), m.bvec_space)),
                _ => None,
            };
            putres(l, r, &a, None, &d);
            if let Some((bvals, bvecs, space)) = tables {
                l.putm(bvals.as_ref());
                l.putm(bvecs.as_ref());
                l.puti(i64::from(space));
            }
        }
        "cdtype" => {
            let a = getw(p, l)?;
            let kind = l.geti();
            let r = p.mri_clone_different_type(&a, kind)?;
            putv(l, Some(&r));
        }
        "ordidx" | "vperm" => {
            let a = getwp(p, l)?;
            let seed = l.geti();
            let g = &a.geom;
            let n = i64::from(g.width) * i64::from(g.height) * i64::from(g.depth);
            let na = n.max(1) as usize;
            let (mut xs, mut ys, mut zs) = (vec![-7i16; na], vec![-7i16; na], vec![-7i16; na]);
            let r = if c == "ordidx" {
                p.mri_order_indices(&a, &mut xs, &mut ys, &mut zs)?
            } else {
                p.set_random_seed(i64::from(seed));
                p.mri_compute_voxel_permutation(&a, &mut xs, &mut ys, &mut zs)?
            };
            l.puti(i64::from(r));
            for arr in [&xs, &ys, &zs] {
                for &v in arr.iter().take(n.max(0) as usize) {
                    l.puti(i64::from(v));
                }
            }
        }
        "nonmax" => {
            let mut a = getwp(p, l)?;
            let mut d = getdst(p, l, false)?;
            let t = l.getf();
            let dir = l.geti();
            let r = p.mri_non_max_suppress(&mut a, dst(&mut d), t, dir)?;
            putres(l, r, &a, None, &d);
        }
        "xrpad" => {
            let mut a = getwp(p, l)?;
            let reg = getreg(l);
            let pad = l.geti();
            let r = p.mri_extract_region_and_pad(&mut a, reg, pad)?;
            putv(l, r.as_ref());
        }
        "rms" => {
            let a = getwp(p, l)?;
            let mut o = getw(p, l)?;
            p.mri_rms(&a, &mut o)?;
            putv(l, Some(&o));
        }
        "vfrac" => {
            let a = getw(p, l)?;
            let m = l.getm(p)?;
            let s = getwp(p, l)?;
            let mut f = getw(p, l)?;
            let r = p.mri_compute_volume_fractions(&a, m.as_ref(), &s, &mut f)?;
            l.puti(i64::from(r));
            putv(l, Some(&f));
        }
        "cdt" => {
            let mut a = getwp(p, l)?;
            let mut b = getwp(p, l)?;
            let mut d = getdst(p, l, true)?;
            let r = p.mri_combine_distance_transforms(&mut a, &mut b, dst(&mut d))?;
            putres(l, r, &a, Some(&b), &d);
        }
        "diffv" => {
            let a = getwp(p, l)?;
            let b = getwp(p, l)?;
            let m = getw_or_null(p, l, true)?;
            let t = l.getd();
            match p.mri_diff2vect(Some(&a), Some(&b), t, m.as_ref())? {
                None => l.out("null"),
                Some(rows) => {
                    l.puti(rows.len() as i64);
                    for row in &rows {
                        for &x in row {
                            l.putd(x);
                        }
                    }
                }
            }
        }
        "applyv" => {
            let mut a = getwp(p, l)?;
            let mut m = getw_or_null(p, l, true)?;
            let n = l.geti();
            let mut rows = Vec::new();
            for _ in 0..n {
                rows.push([l.getd(), l.getd(), l.getd(), l.getd(), l.getd()]);
            }
            let mut d = getdst(p, l, m.is_some())?;
            let mut napply = -7;
            let r = p.mri_apply_diff_vect(&mut a, m.as_mut(), &rows, &mut napply, dst(&mut d))?;
            l.puti(i64::from(napply));
            putres(l, r, &a, m.as_ref(), &d);
        }
        "tiv" => {
            let name = l.getx();
            let have = !l.take("nofile");
            let bytes = if have { Some(l.getx()) } else { None };
            let avg = l.geti();
            let scale = l.getd();
            let pdet = l.geti();
            match avg {
                0 => p.env.remove(b"USE_AVERAGE305"),
                1 => p.env.set(b"USE_AVERAGE305", b"true"),
                _ => p.env.set(b"USE_AVERAGE305", b""),
            }
            let path = larmorx_fs_core::files::path_of(&name);
            // the probe's unlink: ENOENT left in errno when there was no file (its transform
            // reader's errors print it)
            if std::fs::remove_file(&path).is_err() {
                p.errno = 2;
            }
            if let Some(b) = &bytes
                && std::fs::write(&path, b).is_err()
            {
                super::args::bad("cannot write the transform file");
            }
            let mut det = UNSETD;
            let r = p.mri_estimate_tiv(
                &name,
                scale,
                if pdet != 0 { Some(&mut det) } else { None },
                &mut FileSystem,
            );
            // the probe removes the file only when the call returns (an end of the program
            // leaves it for the later lines)
            let r = r?;
            if have {
                let _ = std::fs::remove_file(&path);
            }
            l.putd(r);
            l.putd(det);
        }
        "dimmis" => {
            let a = getw(p, l)?;
            let b = getw(p, l)?;
            let f = l.geti();
            l.puti(i64::from(Process::mri_dim_mismatch(&a, &b, f != 0)));
        }
        // -- §39
        "fpow" => {
            let mut a = getwp(p, l)?;
            let n = l.geti();
            let mut powers = vec![UNSET];
            if n > 0 {
                powers = (0..n).map(|_| l.getf()).collect();
            }
            let r = p.mri_framepower(&mut a, &powers)?;
            l.puti(i64::from(r));
            putv(l, Some(&a));
        }
        "mbin" => {
            let mut a = getwp(p, l)?;
            let mut d = getdst(p, l, false)?;
            let t = l.getf();
            let tail = if l.take("null") { None } else { Some(l.getx()) };
            let inv = l.geti();
            let mut nover = -7;
            let r = p.mri_binarize_tail(
                &mut a,
                t,
                tail.as_deref(),
                inv != 0,
                dst(&mut d),
                &mut nover,
            )?;
            l.puti(i64::from(nover));
            putres(l, r, &a, None, &d);
        }
        "reshape" => {
            let mut a = getwp(p, l)?;
            let n = [l.geti(), l.geti(), l.geti(), l.geti()];
            let r = p.mri_reshape(&mut a, n[0], n[1], n[2], n[3])?;
            putv(l, r.as_ref());
            l.puti(i64::from(a.nframes));
        }
        "msq" => {
            let k = l.geti();
            let mut a = getwp(p, l)?;
            let mut m = getw_or_null(p, l, true)?;
            let mut d = getdst(p, l, m.is_some())?;
            let r = match k {
                0 => p.mri_square(&mut a, m.as_mut(), dst(&mut d))?,
                1 => p.mri_square_root(&mut a, m.as_mut(), dst(&mut d))?,
                // MRIsqrt reads no mask; the probe's `other` destination is the mask itself
                _ => match (&mut d, m.as_mut()) {
                    (D::Other, Some(mv)) => match p.mri_sqrt(&mut a, Dst::Given(mv))? {
                        VolOut::Given => VolOut::Other,
                        r => r,
                    },
                    _ => p.mri_sqrt(&mut a, dst(&mut d))?,
                },
            };
            putres(l, r, &a, m.as_ref(), &d);
        }
        "mmax" => {
            let mut a = getwp(p, l)?;
            let mut b = getwp(p, l)?;
            let mut d = getdst(p, l, true)?;
            let r = p.mri_max(&mut a, &mut b, dst(&mut d))?;
            putres(l, r, &a, Some(&b), &d);
        }
        "mconst" => {
            let mut a = getwp(p, l)?;
            let k = l.getd();
            let mul = l.geti();
            let mut d = getdst(p, l, false)?;
            let r = p.mri_const_op(&mut a, k, mul != 0, dst(&mut d))?;
            putres(l, r, &a, None, &d);
        }
        "fbin" => {
            let a = getwp(p, l)?;
            let t = l.getd();
            let mut m = getw_or_null(p, l, true)?;
            let r = p.mri_frame_binarize(&a, t, m.as_mut())?;
            putgen(l, r, m.as_ref());
        }
        "vote" => {
            let mut a = getwp(p, l)?;
            let mut m = getw_or_null(p, l, true)?;
            let mut d = getdst(p, l, m.is_some())?;
            let ex = l.geti();
            let r = p.mri_vote(&mut a, m.as_mut(), dst(&mut d), ex != 0)?;
            putres(l, r, &a, m.as_ref(), &d);
            putv(l, Some(&a));
        }
        "crop" => {
            let a = getwp(p, l)?;
            let b = [l.geti(), l.geti(), l.geti(), l.geti(), l.geti(), l.geti()];
            let r = p.mri_crop(&a, b[0], b[1], b[2], b[3], b[4], b[5])?;
            putv(l, r.as_ref());
        }
        "revs" => {
            let mut a = getwp(p, l)?;
            let mut d = getdst(p, l, false)?;
            let r = p.mri_reverse_slices(&mut a, dst(&mut d))?;
            putres(l, r, &a, None, &d);
        }
        "cutend" => {
            let a = getwp(p, l)?;
            let n = l.geti();
            let r = p.mri_cut_end_slices(&a, n)?;
            putv(l, r.as_ref());
        }
        "segids" => {
            let a = getwp(p, l)?;
            let (frame, not0) = (l.geti(), l.geti());
            let ids = p.mri_seg_id_list(&a, frame, not0 != 0)?;
            l.puti(ids.len() as i64);
            for v in ids {
                l.puti(i64::from(v));
            }
        }
        "hcos" => {
            let mut a = getwp(p, l)?;
            let al = l.getd();
            let mut d = getdst(p, l, false)?;
            let r = p.mri_half_cos_bias(&mut a, al, dst(&mut d))?;
            putres(l, r, &a, None, &d);
        }
        _ => return Ok(false),
    }
    Ok(true)
}
