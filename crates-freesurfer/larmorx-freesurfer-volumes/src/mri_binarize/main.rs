// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_binarize`'s `main` (`mri_binarize.cpp` 292-709) and `utils/randomfields.cpp`'s
//! `RFglobalStats` (313-345, which only this program's `--rmin` and `--rmax` reach): the entry
//! point [`run`] (registered as `mri_binarize` in the crate's `PROGRAMS`).
//!
//! The order of `main`: the version options (`handleVersionOption`), the command line kept for
//! the report (`argv2cmdline`), `uname`, `getcwd`, `ErrorInit` and `DiagInit`; no argument: the
//! usage (status 1); the options ([`super::options`]) and their checks; `--checkopts` ends here
//! (status 0); the report (`dump_options`) unless `--noverbose`. The input read (a frame past its
//! frames: `ERROR: requested frame=<f> >= nframes=<n>`, status 1), the mask (its dimensions the
//! input's); `--fdr`'s threshold (`MRIfdr2vwth`, its debug lines on); `--frame-sum` and
//! `--frame-and` (the frames summed into frame 0, the minimum `nframes - 0.5` for the latter);
//! `--abs`; `--neg`; `--rmin`/`--rmax` from the global mean (`RFglobalStats`); the merge and copy
//! volumes (their dimensions the input's); `--pct`'s threshold (`MRIpercentThresh`). The output
//! (`int`, or `uchar` with `--uchar`; the frames looped over, or only the one asked for) with the
//! input's header: each voxel on a zeroed edge or outside the mask gets the merge volume's value
//! (or `binvalnot`), a matching or in-range one `binval` (counted), any other the merge value.
//! Then the replacements (`--replace`: of the input, the binarisation discarded; `--replaceonly`:
//! of a copy of the input, one after the other), the nearest-neighbour replacements, `--dilate`,
//! `--erode2d`, `--erode-*`; the voxels equal to `binval` in frame 0 counted again (unless
//! `--no-count` or replacing only); `--bincol`; `--bb`'s crop; `--remove-islands`; `--fill-holes`;
//! `--fix-vol-topo`; the colour table (the input's with `--replaceonly` or `--copy-ctab`, then
//! `--ctab`'s); the output written; `--surf`'s tessellation of `binval` (smoothed, reversed); the
//! count (`Count: <hits> <volume> <voxels> <percent>`) and its file; `mri_binarize done`.
//!
//! The defects: FS-1018 (the match loop's counter shared by FreeSurfer's threads), FS-1019
//! (`--rmax`'s message), FS-1020 (`--fdr` without a threshold), FS-1021 and FS-1022
//! (`--fix-vol-topo`), FS-1025 (a step that gives no volume); FS-1023, FS-1024 and FS-1026 in
//! [`super::options`]. `--copy` reads and checks its volume and never uses it (FreeSurfer's
//! behaviour, rule C).

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_1018, FS_1019, FS_1020, FS_1021, FS_1025};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::diag::diag_init;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, Fatal};
use larmorx_freesurfer_core::run::Settings;
use larmorx_freesurfer_core::sse::{add64, div64, mul, mul64, sqrt64, sub64};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::version::{argv2cmdline, handle_version_option, ver_user};
use larmorx_freesurfer_deform::tess::{TopoFix, mri_vol_topo_fix, mris_tessellate};
use larmorx_freesurfer_filter::volcluster::{mri_remove_volume_holes, mri_remove_volume_islands};
use larmorx_freesurfer_io::mriio::{mri_read_volume, mri_write};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::ctab::ctab_deep_copy;
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, MRI_INT, MRI_UCHAR, Mri, mri_alloc_sequence, mri_copy_header,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::{Io, reverse_face_order};
use larmorx_freesurfer_surf::metric::average_vertex_positions;
use larmorx_freesurfer_surf::mp::compute_metric_properties;
use larmorx_freesurfer_surf::readwrite::{self, NO_HEMISPHERE};
use larmorx_freesurfer_volume::arith::{Pair, mri_abs, mri_extract_region, mri_replace_values};
use larmorx_freesurfer_volume::mriset::{
    mri_bin_mask_to_col, mri_dilate, mri_erode_2d, mri_erode_nn, mri_percent_thresh,
    mri_replace_list,
};
use larmorx_freesurfer_volume::pca::{FdrOut, FdrVol, mri_fdr2vwth};
use larmorx_freesurfer_volume::region::{region_get_bounding_box, region_print};

use super::options::{Options, Run, RunInfo, check_options, dump_options, parse_commandline};

/// The program's name as `argv[0]` gives it (the scripts run it by name).
pub const NAME: &str = "mri_binarize";

/// `getcwd(cwd, 2000)`'s buffer: a longer name leaves it empty (as the zeroed global was).
const CWD_ROOM: usize = 2000;

/// `mri_binarize`'s entry point: its original arguments (without the program's name), the
/// settings, the two streams; the exit status. The notices of the run follow on standard error
/// (`larmorx: ...`).
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut g = Globals::new(settings.mode);
    // the process's own standard error, as the C library's perror sees it
    g.stderr.read_write = system::stderr_read_write();
    let status = {
        let mut terr = log::TrackedErr::new(&g, &mut *err);
        let mut r = Run {
            out: &mut *out,
            err: &mut terr,
            env: &settings.env,
            dir: settings.dir.as_deref(),
        };
        match main(&mut g, &mut r, args) {
            Ok(()) => 0,
            Err(f) => f.status(),
        }
    };
    let _ = out.flush();
    g.notices.print(err);
    status
}

/// `RFglobalStats(rf, NULL, &gmean, &gstddev, &max)`: over every voxel and frame (columns
/// outermost), the mean, the standard deviation `sqrt(sumsq / n - mean^2)` and the maximum (from
/// -1,000,000), in `double`.
fn rf_global_stats(
    g: &mut Globals,
    err: &mut dyn Write,
    rf: &Mri,
) -> Result<(f64, f64, f64), Fatal> {
    let (mut sum, mut sumsq, mut nv, mut max) = (0.0f64, 0.0f64, 0i64, -1_000_000.0f64);
    for c in 0..rf.vg.width {
        for r in 0..rf.vg.height {
            for s in 0..rf.vg.depth {
                for f in 0..rf.nframes {
                    let v = f64::from(mri_get_vox_val(g, err, rf, c, r, s, f, 0)?);
                    sum = add64(sum, v);
                    sumsq = add64(sumsq, mul64(v, v));
                    nv += 1;
                    if max < v {
                        max = v;
                    }
                }
            }
        }
    }
    let n = nv as f64;
    let mean = div64(sum, n);
    let std = sqrt64(sub64(div64(sumsq, n), mul64(mean, mean)));
    Ok((mean, std, max))
}

/// `MRIframeSum(vol, vol)`: each voxel's frames summed (`double`) into its frame 0.
fn frame_sum_in_place(g: &mut Globals, err: &mut dyn Write, vol: &mut Mri) -> Result<(), Fatal> {
    for c in 0..vol.vg.width {
        for r in 0..vol.vg.height {
            for s in 0..vol.vg.depth {
                let mut v = 0.0f64;
                for f in 0..vol.nframes {
                    v = add64(v, f64::from(mri_get_vox_val(g, err, vol, c, r, s, f, 0)?));
                }
                mri_set_vox_val(g, err, vol, c, r, s, 0, v as f32, 0)?;
            }
        }
    }
    Ok(())
}

/// `MRImultiplyConst(vol, vconst, vol)`: each voxel (columns outermost, then rows, slices,
/// frames) times `vconst` in `double`, in place.
fn multiply_const_in_place(
    g: &mut Globals,
    err: &mut dyn Write,
    vol: &mut Mri,
    vconst: f64,
) -> Result<(), Fatal> {
    for c in 0..vol.vg.width {
        for r in 0..vol.vg.height {
            for s in 0..vol.vg.depth {
                for f in 0..vol.nframes {
                    let v = f64::from(mri_get_vox_val(g, err, vol, c, r, s, f, 0)?);
                    mri_set_vox_val(g, err, vol, c, r, s, f, mul64(v, vconst) as f32, 0)?;
                }
            }
        }
    }
    Ok(())
}

/// A volume read for one of the options that must have the input's dimensions (`ERROR: dimension
/// mismatch between input and <what> volumes`, status 1).
fn read_like(
    g: &mut Globals,
    r: &mut Run,
    name: &str,
    like: &Mri,
    what: &str,
) -> Result<Mri, Fatal> {
    let Some(m) = mri_read_volume(g, r.out, r.err, r.env, r.dir, name)? else {
        return Err(Fatal::Exit(1));
    };
    if m.vg.width != like.vg.width || m.vg.height != like.vg.height || m.vg.depth != like.vg.depth {
        let _ = writeln!(
            r.out,
            "ERROR: dimension mismatch between input and {what} volumes"
        );
        return Err(Fatal::Exit(1));
    }
    Ok(m)
}

/// FS-1025: a step that gives no volume where `main` goes on with one. FreeSurfer crashes later
/// (`reproduce`: status 139); larmorx ends with a message (status 1, rule B).
fn no_volume(g: &mut Globals, r: &mut Run, step: &str) -> Fatal {
    g.notices.add(
        &FS_1025,
        format!("mri_binarize: {step} gave no volume (FreeSurfer goes on with none and crashes)"),
    );
    if g.notices.reproduce() {
        return Fatal::Exit(139);
    }
    let _ = writeln!(
        r.err,
        "mri_binarize: {step} gave no volume (FreeSurfer crashes here)"
    );
    Fatal::Exit(1)
}

/// The number of threads FreeSurfer's OpenMP loops would use: the last `--threads`, else
/// `OMP_NUM_THREADS`'s leading number, else the machine's (more than one).
fn freesurfer_threads(o: &Options, r: &Run) -> i32 {
    if let Some(t) = o.threads {
        return t;
    }
    r.env
        .get("OMP_NUM_THREADS")
        .and_then(|v| {
            let s = v.to_string_lossy();
            let d: String = s.trim().chars().take_while(char::is_ascii_digit).collect();
            d.parse::<i32>().ok()
        })
        .unwrap_or(2)
}

/// `main(argc, argv)` on the arguments after the program's name. `Ok` is status 0.
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let (nargs, left) = handle_version_option(g, r.env, r.out, &argv, NAME);
    if nargs != 0 && left.len() == 1 {
        return Err(Fatal::Exit(0));
    }
    let cmdline = argv2cmdline(&left);
    let uts = system::uname(r.env);
    let cwd = system::cwd(r.dir)
        .filter(|c| c.len() < CWD_ROOM)
        .unwrap_or_default();
    g.progname = NAME.to_owned();
    diag_init(g, r.env, r.out);
    let args = &left[1..];
    let mut o = Options::default();
    let mut sg = SurfGlobals::default();
    if args.is_empty() {
        return Err(super::options::usage_exit(r, NAME));
    }
    parse_commandline(g, &mut sg, r, &mut o, args, NAME)?;
    check_options(r, &mut o)?;
    if o.checkoptsonly {
        return Ok(());
    }
    if !o.noverbose {
        let info = RunInfo {
            cwd,
            cmdline,
            sysname: uts.as_ref().map(|u| u.sysname.clone()).unwrap_or_default(),
            hostname: uts.as_ref().map(|u| u.nodename.clone()).unwrap_or_default(),
            machine: uts.as_ref().map(|u| u.machine.clone()).unwrap_or_default(),
            user: ver_user(g, r.env),
        };
        dump_options(r.out, &o, &info);
    }

    let in_name = o.in_vol.clone().expect("checked");
    let Some(mut invol) = mri_read_volume(g, r.out, r.err, r.env, r.dir, &in_name)? else {
        return Err(Fatal::Exit(1));
    };
    if o.frame >= invol.nframes {
        let _ = writeln!(
            r.out,
            "ERROR: requested frame={} >= nframes={}",
            o.frame, invol.nframes
        );
        return Err(Fatal::Exit(1));
    }

    let mut mask: Option<Mri> = None;
    if let Some(name) = o.mask_vol.clone() {
        let _ = writeln!(r.out, "Loading mask {name}");
        mask = Some(read_like(g, r, &name, &invol, "mask")?);
    }

    if o.do_fdr {
        let mut thresh = f64::NAN;
        let ret = {
            let mut vols = [FdrVol {
                vol: &mut invol,
                frame: o.frame,
                mask: mask.as_ref(),
                out: FdrOut::None,
            }];
            mri_fdr2vwth(
                g,
                r.out,
                r.err,
                &mut vols,
                o.fdr,
                o.fdr_sign,
                true,
                &mut thresh,
                true,
            )?
        };
        if ret != 0 {
            g.notices.add(
                &FS_1020,
                "mri_binarize --fdr: no threshold could be computed (FreeSurfer goes on with memory contents)",
            );
            if !g.notices.reproduce() {
                let _ = writeln!(
                    r.err,
                    "mri_binarize: --fdr: MRIfdr2vwth gave no threshold (FreeSurfer goes on with memory contents)"
                );
                return Err(Fatal::Exit(1));
            }
            thresh = 0.0;
        }
        let _ = writeln!(
            r.out,
            "FDR {}, Sign={}, Thresh = {}",
            cfmt::g(o.fdr, 6),
            o.fdr_sign,
            cfmt::g(thresh, 6)
        );
        match o.fdr_sign {
            0 => {
                o.do_abs = true;
                o.min_thresh = thresh;
                o.min_set = true;
            }
            1 => {
                o.min_thresh = thresh;
                o.min_set = true;
            }
            -1 => {
                o.max_thresh = -thresh;
                o.max_set = true;
            }
            _ => {}
        }
    }

    let mut frame = o.frame;
    if o.do_frame_sum || o.do_frame_and {
        let _ = writeln!(r.out, "Summing over {} frames", invol.nframes);
        frame_sum_in_place(g, r.err, &mut invol)?;
        frame = 0;
    }
    if o.do_frame_and {
        o.min_thresh = f64::from(invol.nframes) - 0.5;
    }
    if o.do_abs {
        let _ = writeln!(r.out, "Removing sign from input");
        mri_abs(g, r.out, r.err, Pair::Same(&mut invol))?;
    }
    if o.do_neg {
        let _ = writeln!(r.out, "Negating input");
        multiply_const_in_place(g, r.err, &mut invol, -1.0)?;
    }

    if o.rmin_set || o.rmax_set {
        let _ = writeln!(r.out, "Computing global statistics ");
        let (gmean, gstd, gmax) = rf_global_stats(g, r.err, &invol)?;
        let _ = writeln!(
            r.out,
            "mean = {}, std = {}, max = {}",
            cfmt::g(gmean, 6),
            cfmt::g(gstd, 6),
            cfmt::g(gmax, 6)
        );
        if o.rmin_set {
            o.min_thresh = mul64(gmean, o.rmin);
            o.min_set = true;
            let _ = writeln!(r.out, "Setting min thresh to {}", cfmt::g(o.min_thresh, 6));
        }
        if o.rmax_set {
            o.max_thresh = mul64(gmean, o.rmax);
            o.max_set = true;
            g.notices.add(
                &FS_1019,
                "mri_binarize --rmax: the message gives the maximum threshold (FreeSurfer prints the minimum)",
            );
            let shown = if g.notices.reproduce() {
                o.min_thresh
            } else {
                o.max_thresh
            };
            let _ = writeln!(r.out, "Setting max thresh to {}", cfmt::g(shown, 6));
        }
    }

    let merge = match o.merge_vol.clone() {
        Some(name) => Some(read_like(g, r, &name, &invol, "merge")?),
        None => None,
    };
    if let Some(name) = o.copy_vol.clone() {
        // read and checked, never used (FreeSurfer's behaviour)
        read_like(g, r, &name, &invol, "merge")?;
    }

    if o.do_percent {
        let _ = writeln!(
            r.out,
            "Computing threshold based on top {} percent",
            cfmt::g(o.top_percent, 6)
        );
        o.min_thresh = mri_percent_thresh(g, r.err, &invol, mask.as_ref(), frame, o.top_percent)?;
        let _ = writeln!(r.out, "  Threshold set to {}", cfmt::g(o.min_thresh, 6));
    }

    let (fstart, fend) = if o.do_frame_loop {
        (0, invol.nframes - 1)
    } else {
        (frame, frame)
    };
    let nframes = fend - fstart + 1;
    if !o.noverbose {
        let _ = writeln!(
            r.out,
            "fstart = {fstart}, fend = {fend}, nframes = {nframes}"
        );
    }

    let mtype = if o.uchar { MRI_UCHAR } else { MRI_INT };
    let (w, h, d) = (invol.vg.width, invol.vg.height, invol.vg.depth);
    let mut outvol = mri_alloc_sequence(g, r.out, r.err, w, h, d, mtype, nframes)?;
    mri_copy_header(g, r.out, r.err, &invol, Some(&mut outvol))?;

    let mut nhits = 0i32;
    if !o.replace_only {
        if o.do_match && freesurfer_threads(&o, r) > 1 {
            g.notices.add(
                &FS_1018,
                "mri_binarize: the matches computed as one thread computes them (FreeSurfer's threads share the loop's counter)",
            );
        }
        let bin_val = o.bin_val as f32;
        for f in fstart..=fend {
            for c in 0..w {
                let mut mergeval = f64::from(o.bin_val_not);
                for rr in 0..h {
                    for s in 0..d {
                        if let Some(m) = &merge {
                            mergeval = f64::from(mri_get_vox_val(g, r.err, m, c, rr, s, f, 0)?);
                        }
                        let edge = (o.zero_col_edges && (c == 0 || c == w - 1))
                            || (o.zero_row_edges && (rr == 0 || rr == h - 1))
                            || (o.zero_slice_edges && (s == 0 || s == d - 1));
                        if edge {
                            mri_set_vox_val(
                                g,
                                r.err,
                                &mut outvol,
                                c,
                                rr,
                                s,
                                f - fstart,
                                mergeval as f32,
                                0,
                            )?;
                            continue;
                        }
                        if let Some(m) = &mask {
                            let maskval = f64::from(mri_get_vox_val(g, r.err, m, c, rr, s, 0, 0)?);
                            if maskval < o.mask_thresh {
                                mri_set_vox_val(
                                    g,
                                    r.err,
                                    &mut outvol,
                                    c,
                                    rr,
                                    s,
                                    f - fstart,
                                    mergeval as f32,
                                    0,
                                )?;
                                continue;
                            }
                        }
                        let val = f64::from(mri_get_vox_val(g, r.err, &invol, c, rr, s, f, 0)?);
                        let hit = if o.do_match {
                            o.match_values.iter().any(|&m| {
                                (val - f64::from(m)).abs() < 2.0 * f64::from(f32::MIN_POSITIVE)
                            })
                        } else {
                            !((o.min_set && val < o.min_thresh)
                                || (o.max_set && val > o.max_thresh))
                        };
                        let v = if hit {
                            nhits += 1;
                            bin_val
                        } else {
                            mergeval as f32
                        };
                        mri_set_vox_val(g, r.err, &mut outvol, c, rr, s, f - fstart, v, 0)?;
                    }
                }
            }
        }
    }

    if !o.src_replace.is_empty() {
        let n = o.src_replace.len();
        if o.replace_only && o.transitive_replace {
            let _ = writeln!(r.out, "Replacing {n} and propagating source list");
            outvol = mri_copy(g, r.out, r.err, &invol, None)?.unwrap_new();
            for k in 0..n {
                let _ = writeln!(
                    r.out,
                    "{:2}:  {:4} {:4}",
                    k + 1,
                    o.src_replace[k],
                    o.trg_replace[k]
                );
                mri_replace_values(
                    g,
                    r.out,
                    r.err,
                    Pair::Same(&mut outvol),
                    o.src_replace[k] as f32,
                    o.trg_replace[k] as f32,
                )?;
            }
        } else {
            let _ = writeln!(r.out, "Replacing {n}");
            for k in 0..n {
                let _ = writeln!(
                    r.out,
                    "{:2}:  {:4} {:4}",
                    k + 1,
                    o.src_replace[k],
                    o.trg_replace[k]
                );
            }
            let rep = mri_replace_list(
                g,
                r.out,
                r.err,
                Pair::New(&invol),
                &o.src_replace,
                &o.trg_replace,
                mask.as_ref(),
            )?;
            match rep.into_new() {
                Some(m) => outvol = m,
                None => return Err(no_volume(g, r, "MRIreplaceList")),
            }
        }
    }

    if !o.src_replace_nn.is_empty() {
        let n = o.src_replace_nn.len();
        if o.replace_only {
            let _ = writeln!(
                r.out,
                "Replacing {n} with nearest neighbors and propagating source list"
            );
            outvol = mri_copy(g, r.out, r.err, &invol, None)?.unwrap_new();
        } else {
            let _ = writeln!(r.out, "Replacing {n} with nearest neighbors");
        }
        replace_nn(g, r, &o, &invol, mask.as_ref(), &mut outvol, fstart, fend)?;
    }

    if !o.noverbose && !o.replace_only {
        let _ = writeln!(r.out, "Found {nhits} values in range");
    }

    if o.n_dilate3d > 0 {
        let _ = writeln!(r.out, "Dilating {} voxels in 3d", o.n_dilate3d);
        for _ in 0..o.n_dilate3d {
            mri_dilate(g, r.out, r.err, Pair::Same(&mut outvol))?;
        }
    }
    if o.n_erode2d > 0 {
        let _ = writeln!(r.out, "Eroding {} voxels in 2d", o.n_erode2d);
        for _ in 0..o.n_erode2d {
            mri_erode_2d(g, r.out, r.err, Pair::Same(&mut outvol))?;
        }
    }
    if o.n_erode_nn > 0 {
        let _ = writeln!(
            r.out,
            "Eroding {} voxels using {} ErodeEdges={}",
            o.n_erode_nn,
            o.nn_type,
            i32::from(o.erode_edges)
        );
        let mut tmp: Option<Mri> = None;
        for _ in 0..o.n_erode_nn {
            let edges = i32::from(o.erode_edges);
            match tmp.as_mut() {
                None => {
                    let t = mri_erode_nn(g, r.out, r.err, Pair::New(&outvol), o.nn_type, edges)?;
                    tmp = t.into_new();
                }
                Some(t) => {
                    mri_erode_nn(g, r.out, r.err, Pair::Given(&outvol, t), o.nn_type, edges)?;
                }
            }
            if let Some(t) = tmp.as_ref() {
                mri_copy(g, r.out, r.err, t, Some(&mut outvol))?;
            }
        }
    }

    nhits = 0;
    if o.do_count && !o.replace_only {
        if !o.noverbose {
            let _ = writeln!(r.out, "Counting number of voxels in first frame");
        }
        let bin_val = f64::from(o.bin_val);
        for c in 0..outvol.vg.width {
            for rr in 0..outvol.vg.height {
                for s in 0..outvol.vg.depth {
                    let val = f64::from(mri_get_vox_val(g, r.err, &outvol, c, rr, s, 0, 0)?);
                    if (val - bin_val).abs() < 0.00001 {
                        nhits += 1;
                    }
                }
            }
        }
        if !o.noverbose {
            let _ = writeln!(r.out, "Found {nhits} voxels in final mask");
        }
    }

    if o.do_bin_col {
        let _ = writeln!(r.out, "Filling mask with column number");
        mri_bin_mask_to_col(g, r.out, r.err, Pair::Same(&mut outvol))?;
    }

    if o.do_bb {
        let _ = writeln!(r.out, "Computing bounding box, npad = {}", o.npad_bb);
        let region = region_get_bounding_box(g, r.err, &outvol, o.npad_bb)?;
        region_print(r.out, &region);
        let t = mri_extract_region(g, r.out, r.err, &mut outvol, None, Some(&region))?;
        let Some(t) = t.into_new() else {
            return Err(Fatal::Exit(1));
        };
        outvol = t;
    }

    let bin_val = o.bin_val;
    if o.remove_islands {
        let _ = writeln!(r.out, "Removing Volume Islands");
        let _ = r.out.flush();
        let t = mri_remove_volume_islands(
            g,
            r.out,
            r.err,
            Pair::New(&outvol),
            f64::from(bin_val) - 0.5,
            bin_val,
        )?;
        match t.into_new() {
            Some(t) => outvol = t,
            None => return Err(no_volume(g, r, "MRIremoveVolumeIslands")),
        }
    }
    if o.fill_holes {
        let _ = writeln!(r.out, "Removing Volume Holes");
        let _ = r.out.flush();
        let t = mri_remove_volume_holes(
            g,
            r.out,
            r.err,
            Pair::New(&outvol),
            f64::from(bin_val) - 0.5,
            bin_val,
            f64::from(bin_val),
        )?;
        match t.into_new() {
            Some(t) => outvol = t,
            None => return Err(no_volume(g, r, "MRIremoveVolumeHoles")),
        }
    }
    if o.fix_vol_topology {
        let _ = writeln!(r.out, "Fixing volume topology");
        let _ = r.out.flush();
        let fixed = {
            let mut io = Io {
                out: &mut *r.out,
                err: &mut *r.err,
                env: r.env,
            };
            mri_vol_topo_fix(g, &mut sg, &mut io, &mut outvol, bin_val, 1000, 1000, 1000)?
        };
        match fixed {
            TopoFix::New(m) => outvol = *m,
            TopoFix::Same => {
                g.notices.add(
                    &FS_1021,
                    "mri_binarize --fix-vol-topo: a volume already topologically correct is kept (FreeSurfer frees it, then writes it: a crash)",
                );
                if g.notices.reproduce() {
                    return Err(Fatal::Exit(139));
                }
            }
            TopoFix::Null => return Err(no_volume(g, r, "MRIvolTopoFix")),
        }
    }

    if (o.replace_only || o.copy_ctab) && invol.ct.is_some() {
        outvol.ct = ctab_deep_copy(g, r.err, invol.ct.as_ref());
    }
    if o.cmdlinectab.is_some() {
        outvol.ct = ctab_deep_copy(g, r.err, o.cmdlinectab.as_ref());
    }

    if let Some(name) = o.out_vol.clone() {
        let _ = writeln!(r.out, "Writing output to {name}");
        let e = mri_write(
            g,
            r.out,
            r.err,
            r.env,
            r.dir,
            &mut outvol,
            &name,
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
        if e != 0 {
            return Err(Fatal::Exit(1));
        }
    }

    if let Some(name) = o.surf_file.clone() {
        let _ = writeln!(r.out, "Creating surface {name}");
        let mut io = Io {
            out: &mut *r.out,
            err: &mut *r.err,
            env: r.env,
        };
        let mut surf = mris_tessellate(g, &mut sg, &mut io, &mut outvol, bin_val, false)?;
        surf.hemisphere = NO_HEMISPHERE;
        if o.nsmoothsurf > 0 {
            average_vertex_positions(g, &mut io, &mut surf, o.nsmoothsurf)?;
        }
        if o.reverse_face_order {
            let _ = writeln!(io.out, "Reversing face order");
            reverse_face_order(g, &mut io, &mut surf)?;
            compute_metric_properties(g, &mut sg, &mut io, &mut surf)?;
        }
        readwrite::write(g, &mut io, r.dir, &mut surf, &name)?;
    }

    let nvox = outvol
        .vg
        .width
        .wrapping_mul(outvol.vg.height)
        .wrapping_mul(outvol.vg.depth);
    let voxvol = f64::from(mul(mul(outvol.vg.xsize, outvol.vg.ysize), outvol.vg.zsize));
    let count_line = || {
        format!(
            "{} {} {} {}",
            nhits,
            cfmt::f(mul64(f64::from(nhits), voxvol), 6),
            nvox,
            cfmt::f(div64(mul64(100.0, f64::from(nhits)), f64::from(nvox)), 6)
        )
    };
    if !o.noverbose && !o.replace_only {
        let _ = writeln!(r.out, "Count: {}", count_line());
    }
    if let Some(name) = o.count_file.clone() {
        let path = match r.dir {
            Some(d) => d.join(&name),
            None => std::path::PathBuf::from(&name),
        };
        match std::fs::File::create(&path) {
            Ok(mut f) => {
                let _ = writeln!(f, "{}", count_line());
            }
            Err(_) => {
                let _ = writeln!(r.out, "ERROR: could not open {name}");
                return Err(Fatal::Exit(1));
            }
        }
    }
    if !o.noverbose {
        let _ = writeln!(r.out, "mri_binarize done");
    }
    Ok(())
}

/// The nearest-neighbour replacements (`--replace-nn`, `--replaceonly-nn`): each voxel in the
/// mask (above its threshold) whose value (as an `int`) is a source gets the value of the nearest
/// voxel (distance `sqrtf(dc^2 + dr^2 + ds^2)`, the first found at the smallest in slice, row,
/// column order) within the window of that source whose value is no source; the voxel's own
/// value when there is none.
#[allow(clippy::too_many_arguments)]
fn replace_nn(
    g: &mut Globals,
    r: &mut Run,
    o: &Options,
    invol: &Mri,
    mask: Option<&Mri>,
    outvol: &mut Mri,
    fstart: i32,
    fend: i32,
) -> Result<(), Fatal> {
    let (w, h, d) = (invol.vg.width, invol.vg.height, invol.vg.depth);
    for frame in fstart..=fend {
        for c in 0..w {
            for rr in 0..h {
                for s in 0..d {
                    if let Some(m) = mask {
                        let mv = mri_get_vox_val(g, r.err, m, c, rr, s, 0, 0)?;
                        if f64::from(mv) <= o.mask_thresh {
                            continue;
                        }
                    }
                    let v = mri_get_vox_val(g, r.err, invol, c, rr, s, frame, 0)?;
                    let segid =
                        float_to_int(g, f64::from(v), "mri_binarize --replace-nn (the voxel)");
                    let Some(n) = o.src_replace_nn.iter().position(|&x| x == segid) else {
                        continue;
                    };
                    let whalf = (o.window_nn[n] - 1) / 2;
                    let mut nn_val = segid;
                    let mut min_dist = 100_000.0f32;
                    for ds in -whalf..=whalf {
                        let s1 = s + ds;
                        if s1 < 0 || s1 >= d {
                            continue;
                        }
                        for dr in -whalf..=whalf {
                            let r1 = rr + dr;
                            if r1 < 0 || r1 >= h {
                                continue;
                            }
                            for dc in -whalf..=whalf {
                                let c1 = c + dc;
                                if c1 < 0 || c1 >= w {
                                    continue;
                                }
                                let nv = mri_get_vox_val(g, r.err, invol, c1, r1, s1, frame, 0)?;
                                let newsegid = float_to_int(
                                    g,
                                    f64::from(nv),
                                    "mri_binarize --replace-nn (a neighbour)",
                                );
                                if o.src_replace_nn.contains(&newsegid) {
                                    continue;
                                }
                                let dcf = dc as f32;
                                let sq = dcf * dcf
                                    + dr.wrapping_mul(dr) as f32
                                    + ds.wrapping_mul(ds) as f32;
                                let dist = sq.sqrt();
                                if dist < min_dist {
                                    min_dist = dist;
                                    nn_val = newsegid;
                                }
                            }
                        }
                    }
                    mri_set_vox_val(g, r.err, outvol, c, rr, s, frame - fstart, nn_val as f32, 0)?;
                }
            }
        }
    }
    Ok(())
}
