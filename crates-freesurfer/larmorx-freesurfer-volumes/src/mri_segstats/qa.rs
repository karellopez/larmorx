// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_segstats`'s stand-alone measures: `CountEdits` (`--qa-stats`, `mri_segstats.cpp`
//! 2376-2577), `WMAnatStats` (2579-2657) and `MRIrescaleBySeg` (`--rescale-by-seg`, 2659-2714,
//! with its option's part of `parse_commandline`, 1766-1793).
//!
//! The defects: FS-1101 and FS-1102 (names past FreeSurfer's buffers, [`super::main::name_in`]),
//! FS-1103 (`--rescale-by-seg`'s numbers), FS-1111 (`--qa-stats`'s file that does not open),
//! FS-1112 (`WMAnatStats` without its volumes), FS-1113 (no `SUBJECTS_DIR`).

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_1111, FS_1112};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::cmdargs::cmd_nth_is_arg;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, matrix_alloc};
use larmorx_freesurfer_core::sse::{add64, div, div64, mul, mul64};
use larmorx_freesurfer_core::utils::resolve;
use larmorx_freesurfer_gca::transform::GcaLtaIo;
use larmorx_freesurfer_io::ctrpoints::{ControlInfo, mri_read_control_points};
use larmorx_freesurfer_io::fio::fio_file_exists_readable;
use larmorx_freesurfer_io::label::label_read;
use larmorx_freesurfer_io::mriio::{mri_read_volume, mri_write};
use larmorx_freesurfer_io::streams::Io;
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, MRI_FLOAT, MRI_INT, Mri, mri_alloc, mri_alloc_sequence, mri_copy_header,
    mri_copy_pulse_parameters,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::readwrite;
use larmorx_freesurfer_surf::toprest::compute_euler_number;
use larmorx_freesurfer_volume::arith::Pair;
use larmorx_freesurfer_volume::combine::mri_estimate_tiv;
use larmorx_freesurfer_volume::mriset::mri_erode;
use larmorx_freesurfer_volume::segs::{mri_seg_stats, mri_seg_stats_robust};

use super::main::{Buf, brain_vol_stats, name_in};
use super::options::{ETIV_SCALE, Run, number, scan_d, scan_lf};
use super::table::ff;

/// `WM_EDITED_OFF_VAL` and `WM_EDITED_ON_VAL` (`mri.h`): the white-matter edits' values.
const WM_EDITED_OFF_VAL: f64 = 1.0;
const WM_EDITED_ON_VAL: f64 = 255.0;

/// `MRIread(name)`: the volume, or `None` (its messages written).
fn read(g: &mut Globals, r: &mut Run, name: &str) -> Result<Option<Mri>, Fatal> {
    mri_read_volume(g, r.out, r.err, &r.env, r.dir, name)
}

/// `MRISread(name)`'s Euler number, or `None` when the surface does not read.
fn euler_of(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    r: &mut Run,
    name: &str,
) -> Result<Option<i32>, Fatal> {
    let mut io = Io {
        out: &mut *r.out,
        err: &mut *r.err,
        env: &r.env,
    };
    let s = readwrite::read(g, sg, &mut io, r.dir, name, true)?;
    Ok(s.map(|m| compute_euler_number(&m).euler))
}

/// `SUBJECTS_DIR` (`getenv`), `None` when unset.
pub fn subjects_dir(r: &Run) -> Option<String> {
    r.env
        .get("SUBJECTS_DIR")
        .map(|v| v.to_string_lossy().into_owned())
}

/// `%s` of a C string that may be `NULL` (glibc prints `(null)`).
pub fn or_null(s: Option<&str>) -> &str {
    s.unwrap_or("(null)")
}

/// `CountEdits(subject, outfile)` (`--qa-stats`): the control points, the white-matter edits
/// (voxels of 1 erased, of 255 filled), the brain-mask edits (`brainmask.auto.mgz` against
/// `brainmask.mgz`: erased where the mask is 1, cloned elsewhere; none when the automatic mask
/// does not exist), the segmentation's changes (`aseg.mgz` against `aseg.auto.mgz`), the holes of
/// `?h.orig.nofix` (`1 - euler / 2`, 0 for a surface that does not read), the mask volume over eTIV
/// (from the brain-volume cache), `WMAnatStats` of `norm.mgz` (3 erosions, 2 %) and the grey/white
/// contrast (`?h.w-g.pct.mgh` within `?h.cortex.label` on `?h.white`); printed, and written to
/// `outfile`. The return value is the status: 1 when a volume does not read.
pub fn count_edits(
    g: &mut Globals,
    r: &mut Run,
    subject: &str,
    outfile: Option<&str>,
) -> Result<i32, Fatal> {
    let sdir = subjects_dir(r);
    let sdn = or_null(sdir.as_deref()).to_owned();
    // char sd[4000], tmpstr[4000]
    let sd = name_in(g, r, Buf::Sprintf(4000), format!("{sdn}/{subject}"));
    let ctl = name_in(g, r, Buf::Sprintf(4000), format!("{sd}/tmp/control.dat"));
    let mut count = 0i32;
    if fio_file_exists_readable(g, r.dir, &ctl) != 0 {
        let mut info = ControlInfo {
            count: 0,
            use_real_ras: 0,
        };
        let _ = mri_read_control_points(g, &r.env, r.out, r.err, r.dir, &ctl, &mut info)?;
        count = info.count;
    }

    let wm = name_in(g, r, Buf::Sprintf(4000), format!("{sd}/mri/wm.mgz"));
    let Some(mri) = read(g, r, &wm)? else {
        return Ok(1);
    };
    let (mut n_wm_erase, mut n_wm_fill) = (0i32, 0i32);
    for c in 0..mri.vg.width {
        for rr in 0..mri.vg.height {
            for s in 0..mri.vg.depth {
                let v1 = f64::from(mri_get_vox_val(g, r.err, &mri, c, rr, s, 0, 0)?);
                if v1 == WM_EDITED_OFF_VAL {
                    n_wm_erase += 1;
                }
                if v1 == WM_EDITED_ON_VAL {
                    n_wm_fill += 1;
                }
            }
        }
    }
    drop(mri);

    let (mut n_bm_erase, mut n_bm_clone) = (0i32, 0i32);
    let bma = name_in(g, r, Buf::Sprintf(4000), format!("{sd}/mri/brainmask.auto.mgz"));
    if fio_file_exists_readable(g, r.dir, &bma) != 0 {
        let Some(mri2) = read(g, r, &bma)? else {
            return Ok(1);
        };
        let bm = name_in(g, r, Buf::Sprintf(4000), format!("{sd}/mri/brainmask.mgz"));
        let Some(mri) = read(g, r, &bm)? else {
            return Ok(1);
        };
        for c in 0..mri.vg.width {
            for rr in 0..mri.vg.height {
                for s in 0..mri.vg.depth {
                    let v1 = mri_get_vox_val(g, r.err, &mri, c, rr, s, 0, 0)?;
                    let v2 = mri_get_vox_val(g, r.err, &mri2, c, rr, s, 0, 0)?;
                    if f64::from(v1) == f64::from(v2) {
                        continue;
                    }
                    if f64::from(v1) == 1.0 {
                        n_bm_erase += 1;
                    } else {
                        n_bm_clone += 1;
                    }
                }
            }
        }
    }

    let aseg = name_in(g, r, Buf::Sprintf(4000), format!("{sd}/mri/aseg.mgz"));
    let Some(mri) = read(g, r, &aseg)? else {
        return Ok(1);
    };
    let aauto = name_in(g, r, Buf::Sprintf(4000), format!("{sd}/mri/aseg.auto.mgz"));
    let Some(mri2) = read(g, r, &aauto)? else {
        return Ok(1);
    };
    let mut n_aseg_changes = 0i32;
    for c in 0..mri.vg.width {
        for rr in 0..mri.vg.height {
            for s in 0..mri.vg.depth {
                let v1 = mri_get_vox_val(g, r.err, &mri, c, rr, s, 0, 0)?;
                let v2 = mri_get_vox_val(g, r.err, &mri2, c, rr, s, 0, 0)?;
                if f64::from(v1) == f64::from(v2) {
                    continue;
                }
                n_aseg_changes += 1;
            }
        }
    }
    drop((mri, mri2));

    let mut sg = SurfGlobals::default();
    let mut holes = [0i32; 2];
    for (h, hemi) in ["lh", "rh"].iter().enumerate() {
        let name = name_in(
            g,
            r,
            Buf::Sprintf(4000),
            format!("{sdn}/{subject}/surf/{hemi}.orig.nofix"));
        if let Some(eno) = euler_of(g, &mut sg, r, &name)? {
            holes[h] = 1 - eno / 2;
        }
    }
    let totholes = holes[0] + holes[1];

    let xfm = name_in(
        g,
        r,
        Buf::Sprintf(4000),
        format!("{sdn}/{subject}/mri/transforms/talairach.xfm"));
    let atlas_icv = {
        let mut io = GcaLtaIo { env: &r.env };
        mri_estimate_tiv(g, r.out, r.err, r.dir, &xfm, ETIV_SCALE, &mut io)?.0
    };
    let stats = brain_vol_stats(g, r, Some(subject), sdir.as_deref())?;
    let mask_vol_to_etiv = div64(stats[12], atlas_icv);

    let Some(wmstats) = wm_anat_stats(g, r, subject, "norm.mgz", 3, 2.0)? else {
        // WMAnatStats gave NULL: FreeSurfer's next line reads through it
        g.notices.add(
            &FS_1112,
            "mri_segstats --qa-stats: aparc+aseg.mgz or norm.mgz did not read (FreeSurfer crashes)",
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        let _ = writeln!(
            r.err,
            "mri_segstats: --qa-stats: the white-matter statistics need aparc+aseg.mgz and norm.mgz (FreeSurfer crashes here)"
        );
        return Ok(1);
    };

    let (mut gwconmeansum, mut gwconvarsum) = (0.0f64, 1e-10f64);
    for hemi in ["lh", "rh"] {
        let pct = name_in(
            g,
            r,
            Buf::Sprintf(4000),
            format!("{sdn}/{subject}/surf/{hemi}.w-g.pct.mgh"));
        if fio_file_exists_readable(g, r.dir, &pct) == 0 {
            let _ = writeln!(r.out, "Cannot find {pct} ... skipping");
            continue;
        }
        let Some(wgcon) = read(g, r, &pct)? else {
            return Err(Fatal::Exit(1));
        };
        let white = name_in(
            g,
            r,
            Buf::Sprintf(4000),
            format!("{sdn}/{subject}/surf/{hemi}.white"));
        let surf = {
            let mut io = Io {
                out: &mut *r.out,
                err: &mut *r.err,
                env: &r.env,
            };
            readwrite::read(g, &mut sg, &mut io, r.dir, &white, true)?
        };
        let Some(surf) = surf else {
            return Err(Fatal::Exit(1));
        };
        let lname = name_in(
            g,
            r,
            Buf::Sprintf(4000),
            format!("{sdn}/{subject}/label/{hemi}.cortex.label"));
        let Some(clabel) = label_read(g, &r.env, r.out, r.err, r.dir, None, &lname)? else {
            return Err(Fatal::Exit(1));
        };
        let mut seg = mri_alloc(g, r.out, r.err, surf.nvertices, 1, 1, MRI_INT)?;
        for lv in clabel.lv.iter().take(clabel.n_points.max(0) as usize) {
            mri_set_vox_val(g, r.err, &mut seg, lv.vno, 0, 0, 0, 1.0, 0)?;
        }
        let (_, st) = mri_seg_stats(g, r.err, &seg, 1, &wgcon, 0)?;
        gwconmeansum = add64(gwconmeansum, f64::from(st.mean));
        gwconvarsum = add64(gwconvarsum, f64::from(mul(st.std, st.std)));
        let _ = writeln!(
            r.out,
            " {hemi} cnrstats: {} {} {}",
            ff(st.mean, 6, 3),
            ff(st.std, 6, 3),
            ff(div(st.mean, st.std), 6, 3)
        );
    }
    let gwconmean = div64(gwconmeansum, 2.0);
    let gwconstd = larmorx_freesurfer_core::sse::sqrt64(div64(gwconvarsum, 2.0));
    let f3 = |v: f64| cfmt::pad(&cfmt::f(v, 3), 6);
    let _ = writeln!(
        r.out,
        "{subject} nc {count:>3}, nWMErase {n_wm_erase:>3}, nWMFill {n_wm_fill:>3}, nBMErase {n_bm_erase:>3}, nBMClone {n_bm_clone:>3}, nASegChanges {n_aseg_changes:>3}, lhholes {:>4}, rhholes {:>4}, MaskVolToETIV {}",
        holes[0],
        holes[1],
        cfmt::pad(&cfmt::f(mask_vol_to_etiv, 5), 7)
    );
    let _ = writeln!(
        r.out,
        "wmstats: {} {} {} {} {}",
        ff(wmstats[0], 6, 2),
        ff(wmstats[1], 6, 2),
        ff(wmstats[2], 6, 2),
        ff(wmstats[3], 6, 2),
        ff(wmstats[4], 6, 2)
    );
    let cnr = div64(gwconmean, gwconstd);
    let _ = writeln!(r.out, "cnrstats: {} {} {}", f3(gwconmean), f3(gwconstd), f3(cnr));

    if let Some(outfile) = outfile {
        let line = format!(
            "{subject} {count:>3}    {:>4} {:>4}    {:>4} {:>4}   {:>4}  {:>4} {:>4} {:>4}   {} {} {} {} {} {} {} {} {} {}\n",
            n_wm_erase,
            n_wm_fill,
            n_bm_erase,
            n_bm_clone,
            n_aseg_changes,
            holes[0],
            holes[1],
            totholes,
            cfmt::pad(&cfmt::f(mask_vol_to_etiv, 5), 7),
            ff(wmstats[0], 6, 2),
            ff(wmstats[1], 6, 2),
            ff(wmstats[2], 6, 2),
            ff(wmstats[3], 6, 2),
            ff(wmstats[4], 6, 2),
            ff(div(wmstats[0], wmstats[1]), 6, 2),
            f3(gwconmean),
            f3(gwconstd),
            f3(cnr)
        );
        match std::fs::File::create(resolve(r.dir, outfile)) {
            Ok(mut f) => {
                let _ = f.write_all(line.as_bytes());
            }
            Err(_) => return cannot_open(g, r, "--qa-stats", outfile).map(|_| 1),
        }
    }
    Ok(0)
}

/// FS-1111: a file FreeSurfer writes without checking its `fopen`: it crashes (`reproduce`:
/// status 139); larmorx ends with a message (status 1).
pub fn cannot_open(g: &mut Globals, r: &mut Run, option: &str, name: &str) -> Result<(), Fatal> {
    g.notices.add(
        &FS_1111,
        format!("mri_segstats {option}: {name} does not open for writing (FreeSurfer crashes)"),
    );
    if g.notices.reproduce() {
        return Err(Fatal::Exit(139));
    }
    let _ = writeln!(
        r.err,
        "mri_segstats: {option}: could not open {name} for writing (FreeSurfer crashes here)"
    );
    Err(Fatal::Exit(1))
}

/// `WMAnatStats(subject, volname, nErodes, Pct)`: the white matter of `aparc+aseg.mgz` (labels
/// 2, 41, 7, 46, 251-255, 77, 78, 79), eroded `nErodes` times in 3-D (`Eroding <n> voxels in
/// 3d`), and `MRIsegStatsRobust` of `<volname>` within it at `Pct`: mean, standard deviation,
/// minimum, maximum, range. `None` (`NULL`) when a volume does not read.
pub fn wm_anat_stats(
    g: &mut Globals,
    r: &mut Run,
    subject: &str,
    volname: &str,
    n_erodes: i32,
    pct: f32,
) -> Result<Option<[f32; 5]>, Fatal> {
    const WMIDS: [i32; 12] = [2, 41, 7, 46, 251, 252, 253, 254, 255, 77, 78, 79];
    let sdir = subjects_dir(r);
    let sdn = or_null(sdir.as_deref()).to_owned();
    let _sd = name_in(g, r, Buf::Sprintf(4000), format!("{sdn}/{subject}"));
    let apn = name_in(
        g,
        r,
        Buf::Sprintf(4000),
        format!("{sdn}/{subject}/mri/aparc+aseg.mgz"));
    let Some(mut apas) = read(g, r, &apn)? else {
        return Ok(None);
    };
    let wmn = name_in(g, r, Buf::Sprintf(4000), format!("{sdn}/{subject}/mri/{volname}"));
    let Some(wmvol) = read(g, r, &wmn)? else {
        return Ok(None);
    };
    for c in 0..apas.vg.width {
        for rr in 0..apas.vg.height {
            for s in 0..apas.vg.depth {
                let val = mri_get_vox_val(g, r.err, &apas, c, rr, s, 0, 0)?;
                // fabs(val - wmids[n]) < 2 * FLT_MIN, in float
                let hit = WMIDS.iter().any(|&w| {
                    let d = larmorx_freesurfer_core::sse::sub(val, w as f32);
                    d.abs() < 2.0 * f32::MIN_POSITIVE
                });
                let v = if hit { 1.0 } else { 0.0 };
                mri_set_vox_val(g, r.err, &mut apas, c, rr, s, 0, v, 0)?;
            }
        }
    }
    if n_erodes > 0 {
        let _ = writeln!(r.out, "Eroding {n_erodes} voxels in 3d");
        for _ in 0..n_erodes {
            mri_erode(g, r.out, r.err, Pair::Same(&mut apas))?;
        }
    }
    let mut nhits = 0i32;
    for c in 0..apas.vg.width {
        for rr in 0..apas.vg.height {
            for s in 0..apas.vg.depth {
                if mri_get_vox_val(g, r.err, &apas, c, rr, s, 0, 0)? < 0.5 {
                    continue;
                }
                nhits += 1;
            }
        }
    }
    // the values copied into a matrix nothing reads (its memory as FreeSurfer's)
    let v = matrix_alloc(g, r.out, r.err, nhits, 1, MATRIX_REAL)?;
    drop(v);
    let (_, st) = mri_seg_stats_robust(g, r.err, &apas, 1, &wmvol, 0, pct)?;
    Ok(Some([st.mean, st.std, st.min, st.max, st.range]))
}

/// `MRIrescaleBySeg(invol, seg, segids, targetmean, NULL)`: a `float` copy of `invol` (its header
/// and pulse parameters) scaled so that the mean over the voxels (every frame) whose label (`(int)`
/// of `seg`'s frame 0) is one of `segids` becomes `targetmean`. `None` (`ERROR: nhits = 0`) when
/// no voxel matches.
pub fn mri_rescale_by_seg(
    g: &mut Globals,
    r: &mut Run,
    invol: &Mri,
    seg: &Mri,
    segids: &[i32],
    targetmean: f64,
) -> Result<Option<Mri>, Fatal> {
    let mut outvol = mri_alloc_sequence(
        g,
        r.out,
        r.err,
        invol.vg.width,
        invol.vg.height,
        invol.vg.depth,
        MRI_FLOAT,
        invol.nframes,
    )?;
    mri_copy_header(g, r.out, r.err, invol, Some(&mut outvol))?;
    mri_copy_pulse_parameters(invol, &mut outvol);
    let _ = write!(
        r.out,
        "MRIrescaleBySeg() targetmean = {}, segids = ",
        cfmt::g(targetmean, 6)
    );
    for k in segids {
        let _ = write!(r.out, "{k:>2} ");
    }
    let _ = writeln!(r.out);
    let (mut sum, mut nhits) = (0.0f64, 0i32);
    for c in 0..invol.vg.width {
        for rr in 0..invol.vg.height {
            for s in 0..invol.vg.depth {
                let v = mri_get_vox_val(g, r.err, seg, c, rr, s, 0, 0)?;
                let segid = float_to_int(g, f64::from(v), "MRIrescaleBySeg");
                if !segids.contains(&segid) {
                    continue;
                }
                for f in 0..invol.nframes {
                    let val = f64::from(mri_get_vox_val(g, r.err, invol, c, rr, s, f, 0)?);
                    sum = add64(sum, val);
                    nhits = nhits.wrapping_add(1);
                }
            }
        }
    }
    if nhits == 0 {
        let _ = writeln!(r.out, "ERROR: nhits = 0");
        return Ok(None);
    }
    let mean = div64(sum, f64::from(nhits));
    let scale = div64(targetmean, mean);
    let _ = writeln!(
        r.out,
        "nhits = {nhits}, sum = {}, mean = {}, targetmean = {}, scale = {}",
        cfmt::g(sum, 6),
        cfmt::g(mean, 6),
        cfmt::g(targetmean, 6),
        cfmt::g(scale, 6)
    );
    for c in 0..invol.vg.width {
        for rr in 0..invol.vg.height {
            for s in 0..invol.vg.depth {
                for f in 0..invol.nframes {
                    let val = f64::from(mri_get_vox_val(g, r.err, invol, c, rr, s, f, 0)?);
                    let o = super::main::to_float(mul64(scale, val));
                    mri_set_vox_val(g, r.err, &mut outvol, c, rr, s, f, o, 0)?;
                }
            }
        }
    }
    Ok(Some(outvol))
}

/// `--rescale-by-seg invol seg targmean outvol segid1 segid2 ...`: the volumes read (status 1 when
/// one does not), the ids (`ERROR: no segids provided`, status 1, when none follow),
/// [`mri_rescale_by_seg`] written; the program's end with the write's status.
pub fn rescale_by_seg_option(
    g: &mut Globals,
    r: &mut Run,
    rest: &[String],
) -> Result<Fatal, Fatal> {
    const OPT: &str = "--rescale-by-seg";
    let Some(invol) = read(g, r, &rest[0])? else {
        return Ok(Fatal::Exit(1));
    };
    let Some(segvol) = read(g, r, &rest[1])? else {
        return Ok(Fatal::Exit(1));
    };
    let tm = scan_lf(g, &rest[2]);
    let targetmean = number(g, r.err, OPT, &rest[2], tm)?;
    let outvolpath = rest[3].clone();
    let mut segids = Vec::new();
    let mut nth = 4;
    let mut last: Option<i32> = None;
    while cmd_nth_is_arg(rest, nth) {
        // a fresh `int segid` each time: unread, it holds what the last one left (FS-1103)
        let v = match scan_d(g, &rest[nth]) {
            Some(v) => v,
            None => {
                let x = number(g, r.err, OPT, &rest[nth], None)?;
                last.unwrap_or(x as i32)
            }
        };
        segids.push(v);
        last = Some(v);
        nth += 1;
    }
    if segids.is_empty() {
        let _ = writeln!(r.out, "ERROR: no segids provided");
        return Ok(Fatal::Exit(1));
    }
    let Some(mut outvol) = mri_rescale_by_seg(g, r, &invol, &segvol, &segids, targetmean)? else {
        return Ok(Fatal::Exit(1));
    };
    let err = mri_write(
        g,
        r.out,
        r.err,
        &r.env,
        r.dir,
        &mut outvol,
        &outvolpath,
        None,
        MGZ_INTENT_UNKNOWN,
    )?;
    Ok(Fatal::Exit(err))
}
