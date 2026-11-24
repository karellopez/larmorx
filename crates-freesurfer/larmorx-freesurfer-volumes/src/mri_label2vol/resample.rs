// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The resampling functions `mri_label2vol`'s part 2 reaches (queue item 3.2-40), translated from
//! FreeSurfer 8.2.0's `utils/resample.cpp`: `MRIseg2SegPVF` (2041-2343, as the program calls it:
//! no mask, no re-initialisation, a new output) with `VOXsegPVF2Seg` (2385-2441) for
//! `--new-aseg2vol`, and `MRIsurf2VolOpt` (1642-1798, one surface and one overlay) for
//! `--fill-ribbon`; the program's own `MRIsurfaceLabel2VolOpt` (`mri_label2vol.cpp` 1365-1389).
//! They move to fs.volume when a later program needs them (`mri_gtmpvc`, `mri_surf2vol`).
//!
//! `MRIseg2SegPVF` keeps its caches in `static` variables and samples the voxels on OpenMP's
//! threads; a program calls it once, so the caches are always made (their lines printed), and
//! the port samples on one thread: each voxel's counts are its own and the total of hits is a
//! sum of integers, so the result and the lines are those of any thread count, which is printed
//! as FreeSurfer's `omp_get_max_threads()` gives it.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-mri-label2vol.md`,
//! "Part 2"): FS-1263 (a label below 0), FS-1264 (a label not in the colour table), FS-1265 (a
//! label without a tissue type), FS-1266 (no sample per voxel), FS-1267 (the samples' count past
//! `int`), FS-1268 (an annotation index no vertex has), FS-1269 (a label point at a vertex the
//! surface does not have), FS-1270 (a transform whose ends are not the ribbon), FS-1271 (a ribbon
//! voxel the surface hash finds no vertex for).

use larmorx_core::defects::{
    FS_12, FS_1263, FS_1264, FS_1265, FS_1266, FS_1267, FS_1268, FS_1269, FS_1270, FS_1271,
};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::matrix_inverse;
use larmorx_freesurfer_core::romp;
use larmorx_freesurfer_core::timer::Timer;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_gca::regdat::transform_lta2_reg_dat;
use larmorx_freesurfer_io::label::Label;
use larmorx_freesurfer_io::streams::Io;
use larmorx_freesurfer_mri::ctab::ColorTable;
use larmorx_freesurfer_mri::lta::{LINEAR_VOX_TO_VOX, Lta, lta_copy};
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, Mri, mri_alloc, mri_alloc_from_vol_geom, mri_alloc_sequence,
    mri_copy_pulse_parameters, use_vol_geom_to_mri,
};
use larmorx_freesurfer_mri::tkreg::xfm_crs2xyz_tkreg;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::mht::Mht;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::props::find_closest_vertex;
use larmorx_freesurfer_surf::readwrite::RIGHT_HEMISPHERE;
use larmorx_freesurfer_surf::views::CURRENT_VERTICES;
use larmorx_freesurfer_volume::ltaops::{
    InvertInto, lta_change_type, lta_fill_inverse, lta_invert, lta_mri_is_source, lta_mri_is_target,
};
use larmorx_freesurfer_volume::vol2vol::transform_reg_dat2_lta;

use super::main::{Run, print_matrix};
use super::options::g_fmt;

/// `LEFT_HEMISPHERE` (`include/mrisurf.h`).
const LEFT_HEMISPHERE: i32 = 0;

/// FreeSurfer's `omp_get_max_threads()` for a run of `threads` threads (0: every logical CPU).
pub fn omp_max_threads(threads: usize) -> usize {
    let procs = std::thread::available_parallelism().map_or(1, |n| n.get());
    romp::max_threads(if threads == 0 { procs } else { threads })
}

/// C's `round` of a `double` (half away from zero) through its integer part, without the C
/// library (track A's relay 56: the optimiser may call the C library's `round`, and MinGW's are
/// not all right): exact for every value (beyond 2^52, NaN and the infinities the value itself).
fn round_half_away(v: f64) -> f64 {
    if v.is_nan() || v.abs() >= 4_503_599_627_370_496.0 {
        return v;
    }
    let t = v as i64 as f64;
    let d = v - t;
    if d >= 0.5 {
        t + 1.0
    } else if d <= -0.5 {
        t - 1.0
    } else {
        t
    }
}

/// A `double` assigned to an `int` (`cvttsd2si`): FS-12 outside the `int` range (NaN 0 and the
/// range's ends by default, `INT_MIN` with `reproduce`).
fn to_int(g: &mut Globals, x: f64, what: &str) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 {
        g.notices.add(
            &FS_12,
            format!("{what} of {x}, outside the int range: an undefined conversion"),
        );
        if g.notices.reproduce() {
            return i32::MIN;
        }
        if x.is_nan() {
            return 0;
        }
        return if x > 0.0 { i32::MAX } else { i32::MIN };
    }
    x.trunc() as i32
}

/// `MIN(a, b)`: `a < b ? a : b`.
fn cmin(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

/// `VOXsegPVF2Seg(segpvfvox, segidlist, nsegs, ct)`: the label of a voxel from its fractions.
/// Their sum (in `float`) below 0.5: 0 (mostly background). Else the label of the largest
/// fraction (the first of equal ones, fractions above 0 only; the first label when none is); with
/// a colour table and no fraction of 0.5 or more, the tissue types vote: each label's fraction
/// goes to its tissue type (the colour table's `TissueType` less 1, the table of tissue types'
/// places less 1 of them counted), and the label of the largest fraction within the type of the
/// most wins (the first type and label of equal ones). FS-1264 for a label the colour table does
/// not hold, FS-1265 for one without a tissue type (0 or unassigned).
fn vox_seg_pvf2seg(
    g: &mut Globals,
    r: &mut Run,
    pvf: &[f32],
    segidlist: &[i32],
    ct: Option<&ColorTable>,
) -> Result<i32, Fatal> {
    let (mut vsum, mut vmax, mut fmax) = (0.0f32, 0.0f32, 0usize);
    for (f, &v) in pvf.iter().enumerate() {
        vsum += v;
        if v > 0.0 && vmax < v {
            vmax = v;
            fmax = f;
        }
    }
    if vsum < 0.5 {
        return Ok(0);
    }
    if let Some(ct) = ct.filter(|_| vmax < 0.5) {
        let ntt = ct
            .ctab_tissue_type
            .as_ref()
            .map_or(0, |t| t.nentries())
            .saturating_sub(1);
        // the schemas' tables have 5 or 6 places: the 100 of FreeSurfer's array are never passed
        let ntt = ntt.clamp(0, 100) as usize;
        let mut vtt = [0.0f32; 100];
        let mut tts = Vec::with_capacity(segidlist.len());
        for (f, &segid) in segidlist.iter().enumerate() {
            let entry = usize::try_from(segid)
                .ok()
                .and_then(|i| ct.entries.get(i))
                .and_then(|e| e.as_deref());
            let Some(e) = entry else {
                g.notices.add(
                    &FS_1264,
                    format!(
                        "mri_label2vol --new-aseg2vol --ttype: the label {segid} is not in the colour table (FreeSurfer reads an entry it does not have and crashes)"
                    ),
                );
                if g.notices.reproduce() {
                    return Err(Fatal::Exit(139));
                }
                let _ = writeln!(
                    r.out,
                    "ERROR: MRIseg2SegPVF(): the segmentation's label {segid} is not in the colour table"
                );
                return Err(Fatal::Exit(1));
            };
            let tt = e.tissue_type.wrapping_sub(1);
            tts.push(tt);
            match usize::try_from(tt) {
                Ok(t) if t < 100 => vtt[t] += pvf[f],
                _ => {
                    g.notices.add(
                        &FS_1265,
                        format!(
                            "mri_label2vol --new-aseg2vol --ttype: the label {segid} has no tissue type (FreeSurfer adds its fraction outside its array: memory contents); it does not vote"
                        ),
                    );
                }
            }
        }
        let (mut vttmax, mut ttmax) = (0.0f32, 0i32);
        for (tt, &v) in vtt.iter().enumerate().take(ntt) {
            if vttmax < v {
                vttmax = v;
                ttmax = tt as i32;
            }
        }
        vmax = 0.0;
        fmax = 0;
        for (f, &tt) in tts.iter().enumerate() {
            if tt != ttmax {
                continue;
            }
            let v = pvf[f];
            if vmax < v {
                vmax = v;
                fmax = f;
            }
        }
    }
    Ok(segidlist[fmax])
}

/// The frame of label `segid` (`seg2frame[segid]`): its place in the sorted list.
fn frame_of(segidlist: &[i32], segid: i32) -> Option<usize> {
    segidlist.binary_search(&segid).ok()
}

/// `MRIseg2SegPVF(seg, seg2vol, resmm, segidlist, nsegs, NULL, 0, ct, NULL)`: each output voxel
/// (the grid of the transform's end that is not `seg`) sampled on a uniform grid of `ndc ndr
/// nds` points (`resmm` above 0: the voxel's size over `resmm`, rounded, at least 1; below 0:
/// `round(|resmm|)` in each direction; 0: the segmentation's smallest voxel size), each point
/// mapped to the nearest segmentation voxel (`nint`, the transform from the output's voxels to
/// the segmentation's in `double`, the official operand order) and counted for its label (not 0);
/// without a colour table the output has a `float` frame per label holding its fraction of the
/// voxel (written where above 0), with one an `int` frame of the voxel's label
/// ([`vox_seg_pvf2seg`]). The caches' lines (`Initilizing cache`, the sampling, `npervox`) and
/// the end's (`done t = <seconds>, nhits=<n>`) on standard output. `None` where FreeSurfer
/// returns `NULL` (a transform that does not invert).
#[allow(clippy::too_many_arguments)]
pub fn mri_seg2seg_pvf(
    g: &mut Globals,
    r: &mut Run,
    seg: &Mri,
    seg2vol: &Lta,
    resmm: f64,
    segidlist: &[i32],
    ct: Option<&ColorTable>,
) -> Result<Option<Mri>, Fatal> {
    let timer = Timer::new();
    let do_seg = ct.is_some();
    let nthreads = omp_max_threads(r.threads);
    let env = r.env.clone();
    let lta = if lta_mri_is_source(g, r.err, seg2vol, seg)? != 0 {
        lta_invert(g, r.out, r.err, InvertInto::New(seg2vol))?.into_new()
    } else {
        lta_copy(g, r.out, r.err, seg2vol, None)?.into_new()
    };
    let Some(mut lta) = lta else {
        return Ok(None);
    };
    if lta.ltype != LINEAR_VOX_TO_VOX {
        lta_change_type(g, r.out, r.err, &env, &mut lta, LINEAR_VOX_TO_VOX)?;
    }
    let vg = lta.xforms[0].src.clone();
    let mut resmm = resmm;
    if resmm == 0.0 {
        resmm = f64::from(cmin(cmin(seg.vg.xsize, seg.vg.ysize), seg.vg.zsize));
    }
    // FS-1267: a count past the int range (FreeSurfer's INT_MIN, a wrapped product): an error
    let too_many = |g: &mut Globals, r: &mut Run, what: String| -> Result<(), Fatal> {
        g.notices.add(
            &FS_1267,
            format!("mri_label2vol --new-aseg2vol: {what} (FreeSurfer's count is not the number)"),
        );
        if g.notices.reproduce() {
            return Ok(());
        }
        let _ = writeln!(
            r.out,
            "ERROR: MRIseg2SegPVF(): resmm {} asks for {what}",
            g_fmt(resmm)
        );
        Err(Fatal::Exit(1))
    };
    let past_int = |x: f64| !x.is_nan() && (x >= 2147483648.0 || x <= -2147483649.0);
    let (mut ndc, mut ndr, mut nds);
    if resmm > 0.0 {
        let n = |g: &mut Globals, r: &mut Run, size: f32| -> Result<i32, Fatal> {
            let x = round_half_away(f64::from(size) / resmm);
            if past_int(x) {
                too_many(g, r, format!("{} samples along a voxel's side", g_fmt(x)))?;
            }
            Ok(to_int(g, x, "MRIseg2SegPVF's samples").max(1))
        };
        ndc = n(g, r, vg.xsize)?;
        ndr = n(g, r, vg.ysize)?;
        nds = n(g, r, vg.zsize)?;
    } else {
        let x = round_half_away(resmm.abs());
        if past_int(x) {
            too_many(g, r, format!("an upsampling factor of {}", g_fmt(x)))?;
        }
        let k = to_int(g, x, "MRIseg2SegPVF's upsampling factor");
        ndc = k;
        ndr = k;
        nds = k;
        if k < 1 && !past_int(x) {
            g.notices.add(
                &FS_1266,
                format!(
                    "mri_label2vol --new-aseg2vol: an upsampling factor of {k} (resmm {}) samples no point of a voxel (FreeSurfer's fractions are 0 over 0); here 1",
                    g_fmt(resmm)
                ),
            );
            if !g.notices.reproduce() {
                ndc = 1;
                ndr = 1;
                nds = 1;
            }
        }
    }
    let dc = 1.0 / f64::from(ndc);
    let dcstart = -0.5 + dc / 2.0;
    let dr = 1.0 / f64::from(ndr);
    let drstart = -0.5 + dr / 2.0;
    let ds = 1.0 / f64::from(nds);
    let dsstart = -0.5 + ds / 2.0;
    let exact = f64::from(ndc) * f64::from(ndr) * f64::from(nds);
    let wrapped = ndc.wrapping_mul(ndr).wrapping_mul(nds);
    if f64::from(wrapped) != exact {
        too_many(
            g,
            r,
            format!("{ndc} x {ndr} x {nds} samples per voxel, past the int range"),
        )?;
    }
    let npervox = wrapped as f32;

    // the label-to-frame cache (FS-1263: labels below 0 before the array)
    if let Some(&neg) = segidlist.iter().find(|&&s| s < 0) {
        g.notices.add(
            &FS_1263,
            format!(
                "mri_label2vol --new-aseg2vol: the label {neg} is below 0 (FreeSurfer keeps its frame before its array: memory contents); here its own frame"
            ),
        );
    }
    let nsegs = segidlist.len() as i32;
    let nvox = vg.width.wrapping_mul(vg.height).wrapping_mul(vg.depth);
    let _ = writeln!(
        r.out,
        "MRIseg2SegPVF(): Initilizing cache nsegs = {nsegs}, nvox = {nvox}, nthreads={nthreads}, DoSeg={}",
        i32::from(do_seg)
    );
    let sampling = format!(
        "resmm={} c: {} {ndc} {} {}, r: {} {ndr} {} {}, s: {} {nds} {} {}\ndelta {} {} {}, npervox = {}\n",
        g_fmt(resmm),
        g_fmt(f64::from(vg.xsize)),
        g_fmt(dcstart),
        g_fmt(dc),
        g_fmt(f64::from(vg.ysize)),
        g_fmt(drstart),
        g_fmt(dr),
        g_fmt(f64::from(vg.zsize)),
        g_fmt(dsstart),
        g_fmt(ds),
        g_fmt(f64::from(vg.xsize / ndc as f32)),
        g_fmt(f64::from(vg.ysize / ndr as f32)),
        g_fmt(f64::from(vg.zsize / nds as f32)),
        g_fmt(f64::from(npervox)),
    );
    let _ = r.out.write_all(sampling.as_bytes());
    let _ = r.out.flush();

    let (nframesout, outtype) = if do_seg {
        (1, MRI_INT)
    } else {
        (nsegs, MRI_FLOAT)
    };
    let mut out = mri_alloc_sequence(
        g, r.out, r.err, vg.width, vg.height, vg.depth, outtype, nframesout,
    )?;
    use_vol_geom_to_mri(g, r.out, r.err, &vg, &mut out.vg)?;
    mri_copy_pulse_parameters(seg, &mut out);

    let m = lta.xforms[0].m_l.as_ref().expect("the transform's matrix");
    let e = |i: i32, j: i32| f64::from(m.relt(i, j));
    let (m11, m12, m13, m14) = (e(1, 1), e(1, 2), e(1, 3), e(1, 4));
    let (m21, m22, m23, m24) = (e(2, 1), e(2, 2), e(2, 3), e(2, 4));
    let (m31, m32, m33, m34) = (e(3, 1), e(3, 2), e(3, 3), e(3, 4));
    if g.gdiag_no > 0 {
        let _ = r.out.write_all(sampling.as_bytes());
        let _ = writeln!(
            r.out,
            "MRIseg2SegPVF(): starting fill t = {}, nvox={nvox}",
            g_fmt(timer.seconds())
        );
        let _ = r.out.flush();
    }

    let (sw, sh, sd) = (seg.vg.width, seg.vg.height, seg.vg.depth);
    let mut nperf = vec![0.0f32; segidlist.len()];
    let mut nhits = 0i32;
    for c in 0..vg.width {
        for rr in 0..vg.height {
            for s in 0..vg.depth {
                nperf.fill(0.0);
                for i in 0..ndc.max(0) {
                    let cf = (f64::from(c) + dcstart) + dc * f64::from(i);
                    let m11cf = m11 * cf + m14;
                    let m21cf = m21 * cf + m24;
                    let m31cf = m31 * cf + m34;
                    for j in 0..ndr.max(0) {
                        let rf = (f64::from(rr) + drstart) + dr * f64::from(j);
                        let m12rf = m12 * rf + m11cf;
                        let m22rf = m22 * rf + m21cf;
                        let m32rf = m32 * rf + m31cf;
                        for k in 0..nds.max(0) {
                            let sf = (f64::from(s) + dsstart) + ds * f64::from(k);
                            let ca = nint(g, m12rf + m13 * sf);
                            if ca < 0 || ca >= sw {
                                continue;
                            }
                            let ra = nint(g, m22rf + m23 * sf);
                            if ra < 0 || ra >= sh {
                                continue;
                            }
                            let sa = nint(g, m32rf + m33 * sf);
                            if sa < 0 || sa >= sd {
                                continue;
                            }
                            let v = mri_get_vox_val(g, r.err, seg, ca, ra, sa, 0, 0)?;
                            let segid = to_int(g, f64::from(v), "MRIseg2SegPVF's label");
                            if segid == 0 {
                                continue;
                            }
                            // every label of the segmentation is in the list (the same conversion)
                            if let Some(f) = frame_of(segidlist, segid) {
                                nperf[f] += 1.0;
                            }
                            nhits = nhits.wrapping_add(1);
                        }
                    }
                }
                if !do_seg {
                    for (f, &n) in nperf.iter().enumerate() {
                        if n > 0.0 {
                            mri_set_vox_val(
                                g,
                                r.err,
                                &mut out,
                                c,
                                rr,
                                s,
                                f as i32,
                                n / npervox,
                                0,
                            )?;
                        }
                    }
                } else {
                    for n in nperf.iter_mut() {
                        *n /= npervox;
                    }
                    let segid = vox_seg_pvf2seg(g, r, &nperf, segidlist, ct)?;
                    mri_set_vox_val(g, r.err, &mut out, c, rr, s, 0, segid as f32, 0)?;
                }
            }
        }
    }
    let _ = writeln!(
        r.out,
        "MRIseg2SegPVF(): done t = {}, nhits={nhits}, nthreads={nthreads}",
        g_fmt(timer.seconds())
    );
    let _ = r.out.flush();
    Ok(Some(out))
}

/// What [`mri_surf2vol_opt`] gives: the volume, or FreeSurfer's `NULL` after its message.
pub enum Surf2Vol {
    Done(Mri),
    Null,
}

/// `MRIsurfaceLabel2VolOpt(ribbon, surf, labels, nlabels, Q, DoStatThresh, StatThresh,
/// DoLabelStatVol, NULL)` (the program's own): an overlay of the surface's vertices (`float`, 0),
/// each label point's vertex set to 1 (`DoLabelStatVol`: its stat; points whose stat is below the
/// threshold skipped with `DoStatThresh`), then [`mri_surf2vol_opt`]. FS-1268 for a label that is
/// not there (an annotation index no vertex has), FS-1269 for a point at a vertex the surface
/// does not have.
#[allow(clippy::too_many_arguments)]
pub fn surface_label2vol_opt(
    g: &mut Globals,
    r: &mut Run,
    ribbon: &Mri,
    surf: &Mris,
    labels: &[Option<Label>],
    q: Option<&Lta>,
    do_stat_thresh: bool,
    stat_thresh: f64,
    do_label_stat_vol: bool,
) -> Result<Surf2Vol, Fatal> {
    let mut overlay = mri_alloc(g, r.out, r.err, surf.nvertices, 1, 1, MRI_FLOAT)?;
    for (nthlabel, label) in labels.iter().enumerate() {
        let Some(label) = label else {
            g.notices.add(
                &FS_1268,
                format!(
                    "mri_label2vol --fill-ribbon --annot: no vertex has the annotation index {nthlabel} (FreeSurfer reads its missing label and crashes); here it maps nothing"
                ),
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(139));
            }
            continue;
        };
        let n = usize::try_from(label.n_points)
            .unwrap_or(0)
            .min(label.lv.len());
        for (nthpoint, lv) in label.lv[..n].iter().enumerate() {
            if do_stat_thresh && f64::from(lv.stat) < stat_thresh {
                continue;
            }
            let vtxno = lv.vno;
            let val = if do_label_stat_vol {
                f64::from(lv.stat)
            } else {
                1.0
            };
            if !(0..surf.nvertices).contains(&vtxno) {
                g.notices.add(
                    &FS_1269,
                    format!(
                        "mri_label2vol --fill-ribbon: a label point at vertex {vtxno} of a surface of {} vertices (FreeSurfer writes outside its overlay)",
                        surf.nvertices
                    ),
                );
                if !g.notices.reproduce() {
                    let _ = writeln!(
                        r.out,
                        "ERROR: label point {nthpoint} is at vertex {vtxno}, not one of the surface's {} vertices",
                        surf.nvertices
                    );
                    return Err(Fatal::Exit(1));
                }
            }
            mri_set_vox_val(g, r.err, &mut overlay, vtxno, 0, 0, 0, val as f32, 0)?;
        }
    }
    mri_surf2vol_opt(g, r, ribbon, surf, &overlay, q)
}

/// `MRIsurf2VolOpt(ribbon, &surf, &overlay, 1, Q, NULL)`: the overlay's values filled into the
/// ribbon. `Q` must have the ribbon at one end (else `ERROR: MRIsurf2VolOpt(): ribbon is neither
/// source nor target` and `NULL`, FS-1270); the output takes the geometry of its other end
/// (`MRIsurf2VolOpt(): ribbon is source`, or `... inverting LTA`), the ribbon's without one.
/// Each output voxel (columns outermost) mapped to the ribbon's nearest voxel; outside the
/// ribbon's extent it stays 0; a ribbon value (`int`) other than 3 and 42 gives 0; else the
/// voxel's surface position (the output's tkregister matrix through the registration's inverse,
/// inverted twice as FreeSurfer does) gets the value of the surface's closest vertex (the surface
/// hash at 16 mm, `MHTfindClosestVertexNoXYZ`; none: FS-1271) unless the surface's hemisphere is
/// not the ribbon value's (left 3, right 42) or the vertex is 1,000 mm away or more (then 0).
pub fn mri_surf2vol_opt(
    g: &mut Globals,
    r: &mut Run,
    ribbon: &Mri,
    surf: &Mris,
    overlay: &Mri,
    q: Option<&Lta>,
) -> Result<Surf2Vol, Fatal> {
    let env = r.env.clone();
    if let Some(q) = q {
        let neither = lta_mri_is_source(g, r.err, q, ribbon)? == 0
            && lta_mri_is_target(g, r.err, q, ribbon)? == 0;
        if neither {
            let _ = writeln!(
                r.out,
                "ERROR: MRIsurf2VolOpt(): ribbon is neither source nor target"
            );
            g.notices.add(
                &FS_1270,
                "mri_label2vol --fill-ribbon --lta: the ribbon is at neither end of the transform (FreeSurfer then writes the missing volume and crashes); here status 1",
            );
            return Ok(Surf2Vol::Null);
        }
    }
    let q2 = match q {
        None => transform_reg_dat2_lta(g, r.out, r.err, &ribbon.vg, &ribbon.vg, None)?,
        Some(q) => {
            if lta_mri_is_source(g, r.err, q, ribbon)? != 0 {
                let _ = writeln!(r.out, "MRIsurf2VolOpt(): ribbon is source");
                lta_copy(g, r.out, r.err, q, None)?.into_new()
            } else {
                let _ = writeln!(r.out, "MRIsurf2VolOpt(): inverting LTA");
                lta_invert(g, r.out, r.err, InvertInto::New(q))?.into_new()
            }
        }
    };
    let Some(mut q2) = q2 else {
        return Ok(Surf2Vol::Null);
    };
    let nframes = overlay.nframes;
    let mut volsurf = mri_alloc_from_vol_geom(
        g,
        r.out,
        r.err,
        &q2.xforms[0].dst,
        MRI_FLOAT,
        nframes,
        false,
    )?;
    mri_copy_pulse_parameters(overlay, &mut volsurf);

    // the output's voxels to the ribbon's
    let Some(mut v) = lta_copy(g, r.out, r.err, &q2, None)?.into_new() else {
        return Ok(Surf2Vol::Null);
    };
    lta_change_type(g, r.out, r.err, &env, &mut v, LINEAR_VOX_TO_VOX)?;
    lta_fill_inverse(g, r.out, r.err, &mut v)?;

    // M: the output's voxels to the surface's coordinates
    let reg = transform_lta2_reg_dat(g, r.out, r.err, &mut q2)?;
    let reg = matrix_inverse(g, r.out, r.err, reg.as_ref(), None)?.into_new();
    let t = xfm_crs2xyz_tkreg(g, r.out, r.err, &volsurf.vg)?;
    let inv_r = matrix_inverse(g, r.out, r.err, reg.as_ref(), None)?.into_new();
    let mm = matrix_multiply(g, r.out, r.err, inv_r.as_ref(), Some(&t), None)?.into_new();

    let hash = Mht::create_vertex_table_resolution(g, r.err, surf, CURRENT_VERTICES, 16.0)?;

    let alloc = |g: &mut Globals, r: &mut Run| -> Result<Matrix, Fatal> {
        Ok(matrix_alloc(g, r.out, r.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 matrix"))
    };
    let mut crs = alloc(g, r)?;
    crs.set_relt(4, 1, 1.0);
    let mut crs_ribbon = alloc(g, r)?;
    let mut surf_ras = alloc(g, r)?;
    let vinv = v.inv_xforms[0].m_l.clone();
    for c in 0..volsurf.vg.width {
        for rr in 0..volsurf.vg.height {
            for s in 0..volsurf.vg.depth {
                crs.set_relt(1, 1, c as f32);
                crs.set_relt(2, 1, rr as f32);
                crs.set_relt(3, 1, s as f32);
                matrix_multiply(
                    g,
                    r.out,
                    r.err,
                    vinv.as_ref(),
                    Some(&crs),
                    Some(&mut crs_ribbon),
                )?;
                // FreeSurfer reads rptr[1][2] and rptr[1][3] of the 4 x 1 vector: its rows 2 and 3
                let c_r = nint(g, f64::from(crs_ribbon.relt(1, 1)));
                let r_r = nint(g, f64::from(crs_ribbon.relt(1, 2)));
                let s_r = nint(g, f64::from(crs_ribbon.relt(1, 3)));
                if c_r < 0 || c_r >= ribbon.vg.width {
                    continue;
                }
                if r_r < 0 || r_r >= ribbon.vg.height {
                    continue;
                }
                if s_r < 0 || s_r >= ribbon.vg.depth {
                    continue;
                }
                let rv = mri_get_vox_val(g, r.err, ribbon, c_r, r_r, s_r, 0, 0)?;
                let ribval = to_int(g, f64::from(rv), "MRIsurf2VolOpt's ribbon value");
                if ribval != 3 && ribval != 42 {
                    for f in 0..nframes {
                        mri_set_vox_val(g, r.err, &mut volsurf, c, rr, s, f, 0.0, 0)?;
                    }
                    continue;
                }
                matrix_multiply(
                    g,
                    r.out,
                    r.err,
                    mm.as_ref(),
                    Some(&crs),
                    Some(&mut surf_ras),
                )?;
                let x = surf_ras.relt(1, 1);
                let y = surf_ras.relt(2, 1);
                let z = surf_ras.relt(3, 1);
                let (dmin, mut nmin, mut vtxnomin) = (1000.0f32, -1i32, -1i32);
                let skip = (surf.hemisphere == LEFT_HEMISPHERE && ribval != 3)
                    || (surf.hemisphere == RIGHT_HEMISPHERE && ribval != 42);
                if !skip {
                    let (mut vtxno, mut d) =
                        hash.find_closest_vertex_no_xyz(g, r.err, surf, [x, y, z])?;
                    if vtxno < 0 {
                        g.notices.add(
                            &FS_1271,
                            format!(
                                "mri_label2vol --fill-ribbon: the ribbon voxel {c} {rr} {s} has no vertex within the surface hash's reach (FreeSurfer prints the search and crashes); here the closest vertex"
                            ),
                        );
                        if g.notices.reproduce() {
                            no_match(
                                g,
                                r,
                                surf,
                                (c, rr, s),
                                [x, y, z],
                                &v,
                                &t,
                                reg.as_ref(),
                                mm.as_ref(),
                            )?;
                            return Ok(Surf2Vol::Null);
                        }
                        let mut io = Io {
                            out: &mut *r.out,
                            err: &mut *r.err,
                            env: &env,
                        };
                        (vtxno, d) =
                            find_closest_vertex(g, &mut io, surf, [x, y, z], CURRENT_VERTICES)?;
                    }
                    if d < dmin {
                        nmin = 0;
                        vtxnomin = vtxno;
                    }
                }
                if nmin == -1 {
                    continue;
                }
                for f in 0..nframes {
                    let val = mri_get_vox_val(g, r.err, overlay, vtxnomin, 0, 0, f, 0)?;
                    mri_set_vox_val(g, r.err, &mut volsurf, c, rr, s, f, val, 0)?;
                }
            }
        }
    }
    let _ = r.out.flush();
    Ok(Surf2Vol::Done(volsurf))
}

/// The lines `MRIsurf2VolOpt` prints before it returns `NULL` for a voxel the hash finds no vertex
/// for (FS-1271, `reproduce`): the voxel and its position, the full search's vertex, the matrices.
#[allow(clippy::too_many_arguments)]
fn no_match(
    g: &mut Globals,
    r: &mut Run,
    surf: &Mris,
    (c, rr, s): (i32, i32, i32),
    p: [f32; 3],
    v: &Lta,
    t: &Matrix,
    reg: Option<&Matrix>,
    mm: Option<&Matrix>,
) -> Result<(), Fatal> {
    use larmorx_core::cfmt::{f, pad};
    let [x, y, z] = p.map(f64::from);
    let _ = writeln!(
        r.out,
        "ERROR: MRIsurf2VolOpt(): No Match: {c:3} {rr:3} {s:3}    {} {} {}",
        pad(&f(x, 2), 6),
        pad(&f(y, 2), 6),
        pad(&f(z, 2), 6)
    );
    let env = r.env.clone();
    let (vtxno, _) = {
        let mut io = Io {
            out: &mut *r.out,
            err: &mut *r.err,
            env: &env,
        };
        find_closest_vertex(g, &mut io, surf, p, CURRENT_VERTICES)?
    };
    let _ = writeln!(
        r.out,
        "0    {c} {rr} {s}     {} {} {}  {vtxno:5}",
        g_fmt(x),
        g_fmt(y),
        g_fmt(z)
    );
    let _ = writeln!(r.out, "V -------------------------");
    print_matrix(g, r, v.inv_xforms[0].m_l.as_ref());
    let _ = r.out.flush();
    let _ = writeln!(r.out, "T = [");
    print_matrix(g, r, Some(t));
    let _ = writeln!(r.out, "R = [");
    print_matrix(g, r, reg);
    let _ = writeln!(r.out, "M = [");
    print_matrix(g, r, mm);
    let _ = r.out.flush();
    Ok(())
}
