// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A segmentation mapped onto another volume's grid, and SPM's warp fields (`utils/resample.cpp`,
//! 3.1-37's volume functions): `MRIaseg2vol` (2509) with `MostHitsInVolVox` (2777) and
//! `CompareAVIndices` (2845), `MRIaseg2volMU` (2699) and `MRIapplySpmWarp` (2879).
//! (`interpolation_code` is `MRIinterpCode`, [`larmorx_freesurfer_mri::mri::mri_interp_code`];
//! `float2int_code` is [`larmorx_freesurfer_io::registerio::float2int_code`].)
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §23):
//! - **FS-509** (memory): the tissue-type vote reads the colour table at a label it does not hold
//!   (outside the table or an empty place), counts tissue types of 100 or more, or lists more than
//!   1,000 labels in one voxel, past its arrays: an error in both modes.
//! - **FS-510** (rule A): the vote "no clear winner" tests the hits of the voxel's highest label
//!   instead of the label with the most hits; by default the most hits.
//! - **FS-511** (memory): `MRIapplySpmWarp`'s value where a sample fails before any succeeded:
//!   never written; 0 in both modes.

use std::io::Write;

use larmorx_core::defects::{FS_504, FS_509, FS_510, FS_511};
use larmorx_core::glibc_qsort::qsort;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_copy, matrix_multiply, matrix_multiply_d};
use larmorx_freesurfer_core::matrix_ops::{matrix_identity, matrix_inverse};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_mri::ctab::ColorTable;
use larmorx_freesurfer_mri::lta::{LINEAR_VOX_TO_VOX, Lta, REGISTER_DAT, lta_copy};
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, Mri, mri_alloc, mri_alloc_header, mri_alloc_sequence, mri_copy_header,
    mri_copy_pulse_parameters, use_vol_geom_to_mri,
};
use larmorx_freesurfer_mri::tkreg::xfm_crs2xyz_tkreg;
use larmorx_freesurfer_mri::vol_geom::xfm_crs2xyz;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::ltaops::{InvertInto, lta_change_type, lta_concat, lta_invert, lta_mri_is_source};
use crate::sample::{SAMPLE_NEAREST, SAMPLE_TRILINEAR, cvtt, sample_volume_frame_type};
use crate::vgeom::vg_get_voxel_to_ras_xform;
use crate::vol2vol::mri_mask_and_upsample;

/// `ASEGVOLINDEX`: a segmentation voxel and the output voxel it falls into.
#[derive(Clone, Copy, Debug)]
struct AvIndex {
    asegindex: i32,
    segid: i32,
    volindex: i32,
    cv: i32,
    rv: i32,
    sv: i32,
}

/// `CompareAVIndices`: by output voxel, then by label.
fn compare_av(a: &AvIndex, b: &AvIndex) -> i32 {
    if a.volindex > b.volindex {
        return 1;
    }
    if a.volindex < b.volindex {
        return -1;
    }
    if a.segid > b.segid {
        return 1;
    }
    if a.segid < b.segid {
        return -1;
    }
    0
}

/// `(int) round(x)` of a `float` (C's `round`, half away from zero, then `cvttsd2si`).
fn iround(x: f32) -> i32 {
    cvtt(f64::from(x).round())
}

/// FS-509: an error in both modes.
fn past_arrays(g: &mut Globals, err: &mut dyn Write, why: &str) -> Result<(), Fatal> {
    g.notices.add(
        &FS_509,
        format!("MostHitsInVolVox: {why} (FreeSurfer reads or writes memory it does not own)"),
    );
    log::error_exit(
        g,
        err,
        ERROR_BADPARM,
        &format!("MRIaseg2vol: {why} (FreeSurfer reads or writes memory it does not own here)\n"),
    )
}

/// `MostHitsInVolVox(avindsorted, N, &segidmost, ct)`: the label with the most hits among the `N`
/// entries of one output voxel (sorted by label; the first label wins ties) and its count. With a
/// colour table, each label's tissue type is read (`WARNING: color table entry <n> <name> has not
/// been assigned a tissue type` for -2) and, when there is no clear winner (FS-510), the label is
/// chosen among the tissue type with the most hits (the lowest such type; the last label of the
/// most hits within it), its count returned (0 and the first label when no tissue type counted).
fn most_hits(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    av: &[AvIndex],
    ct: Option<&ColorTable>,
) -> Result<(i32, i32), Fatal> {
    let n_all = av.len();
    let mut n = 0usize;
    let (mut nhits, mut nmost, mut segidmost) = (0usize, 0usize, 0i32);
    let mut segs: Vec<(i32, usize, i32)> = Vec::new();
    let mut per_ttype = [0usize; 100];
    while n < n_all {
        let segid = av[n].segid;
        nhits = 0;
        while n + nhits < n_all && segid == av[n + nhits].segid {
            nhits += 1;
        }
        if n == 0 {
            segidmost = segid;
            nmost = nhits;
        }
        if nmost < nhits {
            segidmost = segid;
            nmost = nhits;
        }
        n += nhits;
        if let Some(ct) = ct {
            if segs.len() >= 1000 {
                past_arrays(g, err, "more than 1000 labels in one voxel")?;
            }
            let entry = usize::try_from(segid)
                .ok()
                .and_then(|i| ct.entries.get(i))
                .and_then(|e| e.as_deref());
            let Some(e) = entry else {
                past_arrays(g, err, &format!("label {segid} has no colour table entry"))?;
                return Ok((0, 0));
            };
            let tt = e.tissue_type;
            segs.push((segid, nhits, tt));
            if tt >= 0 {
                match per_ttype.get_mut(tt as usize) {
                    Some(c) => *c += nhits,
                    None => {
                        past_arrays(g, err, &format!("tissue type {tt} of 100 or more"))?;
                    }
                }
            }
            if tt == -2 {
                let _ = out.write_all(b"WARNING: color table entry ");
                let _ = write!(out, "{segid} ");
                let _ = out.write_all(&e.name);
                let _ = out.write_all(b" has not been assigned a tissue type\n");
            }
        }
    }
    let Some(ct) = ct else {
        return Ok((nmost as i32, segidmost));
    };
    let last_ratio = nhits as f32 / n_all as f32;
    let most_ratio = nmost as f32 / n_all as f32;
    if (last_ratio < 0.5) != (most_ratio < 0.5) {
        g.notices.add(
            &FS_510,
            format!(
                "MostHitsInVolVox: the clear-winner test reads the last label's {nhits} hits, not the most ({nmost}) of {n_all}"
            ),
        );
    }
    let ratio = if g.notices.reproduce() {
        last_ratio
    } else {
        most_ratio
    };
    if ratio < 0.5 {
        let ntt = ct.ctab_tissue_type.as_ref().map_or(0, |t| t.nentries());
        if ntt > 100 {
            past_arrays(g, err, &format!("{ntt} tissue types (more than 100)"))?;
        }
        let (mut max, mut ttmax) = (0usize, 0i32);
        for (k, &c) in per_ttype.iter().enumerate().take(ntt.max(0) as usize) {
            if max < c {
                max = c;
                ttmax = k as i32;
            }
        }
        segidmost = segs[0].0;
        nmost = 0;
        for &(s, h, tt) in &segs {
            if tt != ttmax || nmost > h {
                continue;
            }
            nmost = h;
            segidmost = s;
        }
    }
    Ok((nmost as i32, segidmost))
}

/// `MRIaseg2vol(aseg, tkR, voltemp, fthresh, &volhit, ct)`: the segmentation mapped onto
/// `voltemp`'s grid by the tkregister registration `tkR` (from `voltemp` to `aseg`, as tkregister2
/// with `--targ aseg --mov voltemp`): each segmentation voxel goes to the output voxel nearest to
/// it (`inv(Kv) tkR Ka`, `round`); each output voxel takes the label with the most hits
/// ([`most_hits`]), 0 where its share of the voxel's hits is below `fthresh`; its count goes to
/// the hit volume. Output voxels no segmentation voxel reached take the label of the segmentation
/// voxel nearest to them (the inverse map; 0 outside the segmentation; hit count 1). Both volumes
/// are `int` with `voltemp`'s header. A colour table without tissue types: `ERROR: MRIaeg2vol: if
/// passing a ct it must have tissue type info` on standard output and `None`. With `Gdiag_no > 0`,
/// the steps on standard output (and every voxel's numbers above 1).
#[allow(clippy::too_many_arguments)]
pub fn mri_aseg2vol(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    aseg: &Mri,
    tk_r: Option<&Matrix>,
    voltemp: &Mri,
    fthresh: f64,
    ct: Option<&ColorTable>,
) -> Result<Option<(Mri, Mri)>, Fatal> {
    if let Some(ct) = ct
        && ct.ctab_tissue_type.is_none()
    {
        let _ =
            out.write_all(b"ERROR: MRIaeg2vol: if passing a ct it must have tissue type info\n");
        return Ok(None);
    }
    let ka = xfm_crs2xyz_tkreg(g, out, err, &aseg.vg)?;
    let kv = xfm_crs2xyz_tkreg(g, out, err, &voltemp.vg)?;
    let inv_kv = matrix_inverse(g, out, err, Some(&kv), None)?.into_new();
    let va2v = matrix_multiply(g, out, err, inv_kv.as_ref(), tk_r, None)?.into_new();
    // Va2v = MatrixMultiply(Va2v, Ka, Va2v)
    let va2v = match va2v {
        Some(mut m) => {
            let c = m.clone();
            match matrix_multiply(g, out, err, Some(&c), Some(&ka), Some(&mut m))? {
                Ret::Given => Some(m),
                _ => None,
            }
        }
        None => {
            matrix_multiply(g, out, err, None, Some(&ka), None)?;
            None
        }
    };
    let vv2a = matrix_inverse(g, out, err, va2v.as_ref(), None)?.into_new();
    let (aw, ah, ad) = (aseg.vg.width, aseg.vg.height, aseg.vg.depth);
    let (vw, vh, vd) = (voltemp.vg.width, voltemp.vg.height, voltemp.vg.depth);
    let na = aw.wrapping_mul(ah).wrapping_mul(ad).max(0) as usize;
    let mut avind: Vec<AvIndex> = Vec::with_capacity(na);
    if g.gdiag_no > 0 {
        let _ = out.write_all(b"ASeg2Vol: Building LUT\n");
    }
    let mut pa = Matrix::real(4, 1);
    pa.set_relt(4, 1, 1.0);
    let mut pv = Matrix::real(4, 1);
    let mut inda = 0i32;
    for sa in 0..ad {
        for ra in 0..ah {
            for ca in 0..aw {
                let segid = cvtt(f64::from(mri_get_vox_val(g, err, aseg, ca, ra, sa, 0, 0)?));
                pa.set_relt(1, 1, ca as f32);
                pa.set_relt(2, 1, ra as f32);
                pa.set_relt(3, 1, sa as f32);
                matrix_multiply(g, out, err, va2v.as_ref(), Some(&pa), Some(&mut pv))?;
                let (cv, rv, sv) = (
                    iround(pv.relt(1, 1)),
                    iround(pv.relt(2, 1)),
                    iround(pv.relt(3, 1)),
                );
                let e = if cv < 0 || cv >= vw || rv < 0 || rv >= vh || sv < 0 || sv >= vd {
                    AvIndex {
                        asegindex: inda,
                        segid,
                        volindex: -1,
                        cv: -1,
                        rv: -1,
                        sv: -1,
                    }
                } else {
                    AvIndex {
                        asegindex: inda,
                        segid,
                        volindex: cv
                            .wrapping_add(rv.wrapping_mul(vw))
                            .wrapping_add(sv.wrapping_mul(vw).wrapping_mul(vh)),
                        cv,
                        rv,
                        sv,
                    }
                };
                avind.push(e);
                inda += 1;
            }
        }
    }
    if g.gdiag_no > 0 {
        let _ = out.write_all(b"ASeg2Vol: Sorting \n");
    }
    qsort(&mut avind, compare_av);
    let mut volaseg = mri_alloc(g, out, err, vw, vh, vd, MRI_INT)?;
    mri_copy_header(g, out, err, voltemp, Some(&mut volaseg))?;
    let mut volhit = mri_alloc(g, out, err, vw, vh, vd, MRI_INT)?;
    mri_copy_header(g, out, err, voltemp, Some(&mut volhit))?;
    if g.gdiag_no > 0 {
        let _ = out.write_all(b"ASeg2Vol: Mapping\n");
    }
    let mut n = 0usize;
    let mut segid = 0i32;
    while n < na {
        let indv = avind[n].volindex;
        if indv < 0 {
            n += 1;
            continue;
        }
        let mut nhits = 0usize;
        while n + nhits < na && indv == avind[n + nhits].volindex {
            nhits += 1;
        }
        let (nhitsmost, s) = most_hits(g, out, err, &avind[n..n + nhits], ct)?;
        segid = s;
        let a = avind[n];
        mri_set_vox_val(
            g,
            err,
            &mut volhit,
            a.cv,
            a.rv,
            a.sv,
            0,
            nhitsmost as f32,
            0,
        )?;
        if f64::from(nhitsmost) / (nhits as f64) < fthresh {
            segid = 0;
        }
        mri_set_vox_val(g, err, &mut volaseg, a.cv, a.rv, a.sv, 0, segid as f32, 0)?;
        if g.gdiag_no > 1 {
            let _ = writeln!(
                out,
                "{n:6} {:6} {segid:5}    {:6}    {:3} {:3} {:3}     {nhits:3} {nhitsmost:3}",
                a.asegindex, a.volindex, a.cv, a.rv, a.sv
            );
        }
        n += nhits;
    }
    if g.gdiag_no > 0 {
        let _ = out.write_all(b"ASeg2Vol: Reverse Map\n");
    }
    let (mut nmisses, mut nfilled) = (0i32, 0i32);
    for sv in 0..vd {
        for rv in 0..vh {
            for cv in 0..vw {
                let nhits = cvtt(f64::from(mri_get_vox_val(
                    g, err, &volhit, cv, rv, sv, 0, 0,
                )?));
                if nhits != 0 {
                    continue;
                }
                nmisses += 1;
                pv.set_relt(1, 1, cv as f32);
                pv.set_relt(2, 1, rv as f32);
                pv.set_relt(3, 1, sv as f32);
                matrix_multiply(g, out, err, vv2a.as_ref(), Some(&pv), Some(&mut pa))?;
                let (ca, ra, sa) = (
                    iround(pa.relt(1, 1)),
                    iround(pa.relt(2, 1)),
                    iround(pa.relt(3, 1)),
                );
                if ca < 0 || ca >= aw || ra < 0 || ra >= ah || sa < 0 || sa >= ad {
                    mri_set_vox_val(g, err, &mut volaseg, cv, rv, sv, 0, 0.0, 0)?;
                } else {
                    segid = cvtt(f64::from(mri_get_vox_val(g, err, aseg, ca, ra, sa, 0, 0)?));
                    mri_set_vox_val(g, err, &mut volaseg, cv, rv, sv, 0, segid as f32, 0)?;
                    mri_set_vox_val(g, err, &mut volhit, cv, rv, sv, 0, 1.0, 0)?;
                    nfilled += 1;
                }
            }
        }
    }
    let _ = segid;
    if g.gdiag_no > 0 {
        let _ = writeln!(out, "nmisses = {nmisses} ({nfilled} filled)");
        let _ = out.write_all(b"ASeg2Vol: done\n");
    }
    Ok(Some((volaseg, volhit)))
}

/// `MRIaseg2volMU(aseg, aseg2vol, fthresh, &volhit, USF, ct)`: [`mri_aseg2vol`] of the segmentation
/// cut to its bounding box padded by 2 and upsampled by `USF` (`MRImaskAndUpsample`; `USF` -1: the
/// segmentation itself), with the transform from it to the volume (`aseg2vol` in either direction:
/// concatenated with the inverse of the cut's, reduced; the caller's array has its inverses filled
/// and may change type), made tkregister's (`REGISTER_DAT`, pointing from the volume), onto a
/// header of `aseg2vol`'s other geometry (`int`, the segmentation's pulse parameters). `None` where
/// [`mri_aseg2vol`] gives none. FS-504.
#[allow(clippy::too_many_arguments)]
pub fn mri_aseg2vol_mu(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    aseg: &mut Mri,
    aseg2vol: &mut Lta,
    fthresh: f64,
    usf: i32,
    ct: Option<&ColorTable>,
) -> Result<Option<(Mri, Mri)>, Fatal> {
    let mut asegmu_own: Option<Mri> = None;
    let asegmu2vol: Option<Lta> = if usf != -1 {
        let Some((mu, l)) = mri_mask_and_upsample(g, out, err, env, aseg, None, usf, 2, false)?
        else {
            // MRImaskAndUpsample returned NULL: LTAinvert of its unset LTA
            return no_array(g, err, "MRIaseg2volMU");
        };
        asegmu_own = Some(mu);
        let Some(a2mu) = l else {
            return no_array(g, err, "MRIaseg2volMU");
        };
        let Some(mut mu2a) = lta_invert(g, out, err, InvertInto::New(&a2mu))?.into_new() else {
            return no_array(g, err, "MRIaseg2volMU");
        };
        lta_concat(g, out, err, env, &mut [&mut mu2a, aseg2vol], true)?
    } else if lta_mri_is_source(g, err, aseg2vol, aseg)? == 1 {
        lta_copy(g, out, err, aseg2vol, None)?.into_new()
    } else {
        lta_invert(g, out, err, InvertInto::New(aseg2vol))?.into_new()
    };
    let asegmu: &Mri = asegmu_own.as_ref().unwrap_or(aseg);
    let Some(mut asegmu2vol) = asegmu2vol else {
        return no_array(g, err, "MRIaseg2volMU");
    };
    if lta_mri_is_source(g, err, &asegmu2vol, asegmu)? == 1 {
        lta_invert(g, out, err, InvertInto::Same(&mut asegmu2vol))?;
    }
    lta_change_type(g, out, err, env, &mut asegmu2vol, REGISTER_DAT)?;
    let src = lta_mri_is_source(g, err, aseg2vol, aseg)? == 1;
    let vg = if src {
        &aseg2vol.xforms[0].dst
    } else {
        &aseg2vol.xforms[0].src
    };
    let mut temp = mri_alloc_header(g, out, err, vg.width, vg.height, vg.depth, MRI_INT, 1)?;
    use_vol_geom_to_mri(g, out, err, vg, &mut temp.vg)?;
    mri_copy_pulse_parameters(aseg, &mut temp);
    if asegmu2vol.num_xforms <= 0 {
        return no_array(g, err, "MRIaseg2volMU");
    }
    let m = asegmu2vol.xforms[0].m_l.clone();
    let asegmu: &Mri = asegmu_own.as_ref().unwrap_or(aseg);
    mri_aseg2vol(g, out, err, asegmu, m.as_ref(), &temp, fthresh, ct)
}

/// FS-504: no transform array (or one without transforms) where FreeSurfer reads one.
fn no_array<T>(g: &mut Globals, err: &mut dyn Write, what: &str) -> Result<Option<T>, Fatal> {
    g.notices.add(
        &FS_504,
        format!("{what}: no transform array (FreeSurfer reads memory it does not own)"),
    );
    log::error_exit(
        g,
        err,
        ERROR_BADPARM,
        &format!("{what}: no transform array (FreeSurfer reads memory it does not own here)\n"),
    )?;
    Ok(None)
}

/// `MRIapplySpmWarp(vol, srclta, warp, LRRev, interp, out)`: the volume resampled into the warp's
/// space. Each warp voxel inside its border (1 to extent - 2 on each axis) holds an RAS point of
/// SPM's input space (frames 0 to 2); its 1-based voxel there (`MatrixMultiplyD` by the inverse of
/// the input's 1-based vox2ras: the volume's own, or with `srclta` the geometry of its other end),
/// minus 1, left-right reversed with `LRRev` (`ncols - 1 - c`), through `srclta`'s vox2vox (changed
/// to `LINEAR_VOX_TO_VOX` first, `MRIapplySpmWarp(): changing Source LTA type to vox2vox`; inverted,
/// `MRIapplySpmWarp(): inverting Source LTA`, when the volume is its source) gives the point to
/// sample (`MRIsampleVolumeFrameType`, every frame) unless it lies outside the volume. `interp`
/// other than nearest or trilinear: `ERROR: MRIapplySpmWarp():  sample type = <n>, must be 0 or 1`
/// and `Null`. The output (a new `float` volume of the warp's grid, header and the volume's frames
/// and pulse parameters, or the caller's) keeps its other voxels. FS-504, FS-511.
#[allow(clippy::too_many_arguments)]
pub fn mri_apply_spm_warp(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    vol: &Mri,
    srclta: Option<&mut Lta>,
    warp: &Mri,
    lrrev: bool,
    interp: i32,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    if interp != SAMPLE_NEAREST && interp != SAMPLE_TRILINEAR {
        let _ = writeln!(
            out,
            "ERROR: MRIapplySpmWarp():  sample type = {interp}, must be {SAMPLE_NEAREST} or {SAMPLE_TRILINEAR}"
        );
        return Ok(Ret::Null);
    }
    let mut fresh = None;
    let o: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let mut m = mri_alloc_sequence(
                g,
                out,
                err,
                warp.vg.width,
                warp.vg.height,
                warp.vg.depth,
                MRI_FLOAT,
                vol.nframes,
            )?;
            mri_copy_header(g, out, err, warp, Some(&mut m))?;
            mri_copy_pulse_parameters(vol, &mut m);
            fresh.insert(m)
        }
    };
    let has_lta = srclta.is_some();
    let (vox2ras1, vsrc, ncols) = match srclta {
        Some(l) => {
            if l.ltype != LINEAR_VOX_TO_VOX {
                let _ = out.write_all(b"MRIapplySpmWarp(): changing Source LTA type to vox2vox\n");
                lta_change_type(g, out, err, env, l, LINEAR_VOX_TO_VOX)?;
            }
            if lta_mri_is_source(g, err, l, vol)? == 1 {
                let lt = &l.xforms[0];
                let v = vg_get_voxel_to_ras_xform(g, out, err, &lt.dst, 1)?;
                let _ = out.write_all(b"MRIapplySpmWarp(): inverting Source LTA\n");
                let vs = matrix_inverse(g, out, err, lt.m_l.as_ref(), None)?.into_new();
                (v, vs, lt.dst.width)
            } else {
                let lt = &l.xforms[0];
                let vs = matrix_copy(g, out, err, lt.m_l.as_ref(), None)?.into_new();
                let v = vg_get_voxel_to_ras_xform(g, out, err, &lt.src, 1)?;
                (v, vs, lt.src.width)
            }
        }
        None => {
            let v = xfm_crs2xyz(g, out, err, &vol.vg, 1, false)?;
            let vs = matrix_identity(g, out, err, 4, None)?.into_new();
            (v, vs, vol.vg.width)
        }
    };
    let ras2vox1 = matrix_inverse(g, out, err, Some(&vox2ras1), None)?.into_new();
    let q = if lrrev {
        let mut q = matrix_identity(g, out, err, 4, None)?.unwrap_new();
        q.set_relt(1, 1, -1.0);
        q.set_relt(1, 4, ncols.wrapping_sub(1) as f32);
        Some(q)
    } else {
        None
    };
    let mut vbmi = Matrix::real(4, 1);
    vbmi.set_relt(4, 1, 1.0);
    let mut crs = Matrix::real(4, 1);
    crs.set_relt(4, 1, 1.0);
    let mut val: Option<f64> = None;
    for c in 1..warp.vg.width.wrapping_sub(1) {
        for r in 1..warp.vg.height.wrapping_sub(1) {
            for s in 1..warp.vg.depth.wrapping_sub(1) {
                for k in 0..3 {
                    let v = mri_get_vox_val(g, err, warp, c, r, s, k, 0)?;
                    vbmi.set_relt(k + 1, 1, v);
                }
                matrix_multiply_d(g, out, err, ras2vox1.as_ref(), Some(&vbmi), Some(&mut crs))?;
                for k in 1..=3 {
                    let v = sse::sub(crs.relt(k, 1), 1.0);
                    crs.set_relt(k, 1, v);
                }
                if let Some(q) = q.as_ref() {
                    let c2 = crs.clone();
                    matrix_multiply_d(g, out, err, Some(q), Some(&c2), Some(&mut crs))?;
                }
                if has_lta {
                    let c2 = crs.clone();
                    matrix_multiply_d(g, out, err, vsrc.as_ref(), Some(&c2), Some(&mut crs))?;
                }
                let (cc, rr, ss) = (
                    f64::from(crs.relt(1, 1)),
                    f64::from(crs.relt(2, 1)),
                    f64::from(crs.relt(3, 1)),
                );
                if cc < 0.0 || cc >= f64::from(vol.vg.width) {
                    continue;
                }
                if rr < 0.0 || rr >= f64::from(vol.vg.height) {
                    continue;
                }
                if ss < 0.0 || ss >= f64::from(vol.vg.depth) {
                    continue;
                }
                for f in 0..vol.nframes {
                    let mut v = val.unwrap_or(0.0);
                    if sample_volume_frame_type(g, err, vol, cc, rr, ss, f, interp, &mut v)
                        == NO_ERROR
                        || val.is_some()
                    {
                        val = Some(v);
                    } else {
                        g.notices.add(
                            &FS_511,
                            "MRIapplySpmWarp: a sample failed before any succeeded (FreeSurfer stores a value never written)",
                        );
                    }
                    mri_set_vox_val(g, err, o, c, r, s, f, v as f32, 0)?;
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}
