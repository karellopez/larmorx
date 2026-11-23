// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_concat`'s `main` (`mri_concat.cpp` 139-772), `GroupedMeanMatrix` (1373-1394) and
//! `MRIfirstNonZero` (1471-1497): the program's entry point [`run`] (registered as `mri_concat`
//! in the crate's `PROGRAMS`) and [`main`].
//!
//! `mri_concat [options] <in>... --o <out>`. The version options first (`handleVersionOption`;
//! only version options: status 0); no argument: the usage, status 1; then the options
//! ([`super::options`]). The inputs' headers are read (`--no-check`: only the first's; `ERROR:
//! reading <in>`, status 1): all of the first's dimensions (`ERROR: dimension mismatch between
//! <first> and <in>`), their frames counted (one each with `--in-frame`); an output of that many
//! frames is made (one with `--rms`), float or (`--keep-datatype`) the last input's type, and each
//! input read whole (`ERROR: loading <in>`), its frame `--in-frame` extracted, signs changed
//! (`--abs`, `--pos`, `--neg`), and its frames copied in (times their `--w` weights). Then, in
//! this order: `--combine`, the prune mask (`--prune`), `--norm-mean`, `--norm1`, the matrix
//! (`--asl`, `--gmean`, `--mtx`), the pairs (`--paired-*`), `--max-bonfcor`'s constant, `--mean`,
//! `--median`, `--mean-div-n`, `--sum`, `--fnorm`, `--tar1`, `--std`/`--var`, `--max`,
//! `--max-index` (`-prune`, `-add`), `--conjunct`, `--min`, `--sort`, `--vote`,
//! `--first-non-zero`, `--mul`/`--div`, `--add`, `--cumsum`, `--scm`, `--pca` (its `.u.mtx` and
//! `.stats.dat` beside the output's stem), `--rep`, the prune mask applied, `--rms`; the colour
//! table (`--ctab`, or the first input's) embedded; the output written (`Writing to <out>`; the
//! writer's status ends the program); `--rusage`'s file. Status 0.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-mri-concat.md`):
//! FS-1194 to FS-1198 and FS-1247 ([`super::options`]); here FS-1200 (`--rms` with an input that
//! does not read whole), FS-1241 (`--no-check` with inputs of other dimensions or frames),
//! FS-1242 (frame counts that pass the `int` range), FS-1243 (`--w`'s weights past the matrix),
//! FS-1244 (`--pca` with an output name of 1,990 bytes or more), FS-1245 (`--pca` with an output
//! without a volume extension), FS-1246 (the matrix's size message) and FS-1248 (a mask of other
//! dimensions).

use std::io::Write;

use larmorx_core::defects::{
    FS_1200, FS_1241, FS_1242, FS_1243, FS_1244, FS_1245, FS_1246, FS_1248,
};
use larmorx_core::math::Libm;
use larmorx_freesurfer_core::cstd::{fmt_f, note_glibc07};
use larmorx_freesurfer_core::diag::diag_init;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, Fatal};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_core::matrix_io::matrix_write_txt;
use larmorx_freesurfer_core::matrix_ops::matrix_print;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::sse::{add64, div64, mul64, sqrt64, sub64};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::utils::{print_rusage, write_rusage};
use larmorx_freesurfer_core::version::handle_version_option;
use larmorx_freesurfer_filter::fmri::{
    asl_interp_matrix, fmri_covariance, fmri_extract_frames, fmri_matrix_multiply,
    fmri_spatial_cor_matrix, fmri_temporal_ar1, mri_conjunct, mri_frame_mean, mri_frame_median,
    mri_frame_norm, mri_frame_sum, mri_vol_max, mri_vol_max_index, mri_vol_min,
};
use larmorx_freesurfer_io::fio::fio_basename;
use larmorx_freesurfer_io::identify::{MRI_VOLUME_TYPE_UNKNOWN, id_stem_from_name};
use larmorx_freesurfer_io::mriio::{mri_read_header, mri_read_volume, mri_write};
use larmorx_freesurfer_mri::ctab::ctab_deep_copy;
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc_sequence, mri_copy_header,
    mri_copy_pulse_parameters,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{Pair, mri_abs, mri_clone_by_space, mri_neg, mri_pos};
use larmorx_freesurfer_volume::combine::mri_rms;
use larmorx_freesurfer_volume::frames::{
    mri_normalize_frames_first, mri_normalize_frames_mean, mri_sort,
};
use larmorx_freesurfer_volume::mriset::{mri_mask, mri_n_mask};
use larmorx_freesurfer_volume::pca::{mri_pca, write_pca_stats};
use larmorx_freesurfer_volume::voxops::{mri_frame_binarize, mri_vote};

use super::options::{check_options, parse_commandline, usage_exit};

/// The program's name as `argv[0]` gives it (the scripts run it by name).
pub const NAME: &str = "mri_concat";

/// The program's streams, environment and working directory.
pub struct Run<'a, 'b> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: Environment,
    pub dir: Option<&'b std::path::Path>,
}

/// `mri_concat`'s entry point: its original arguments (without the program's name), the
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
            env: settings.env.clone(),
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

/// `MRIgetVoxVal(m, c, r, s, f)` as a `double`.
fn get(
    g: &mut Globals,
    r: &mut Run,
    m: &Mri,
    c: i32,
    rr: i32,
    s: i32,
    f: i32,
) -> Result<f64, Fatal> {
    Ok(f64::from(mri_get_vox_val(g, r.err, m, c, rr, s, f, 0)?))
}

/// `MRIsetVoxVal(m, c, r, s, f, v)` of a `double` (converted to the `float` it takes).
#[allow(clippy::too_many_arguments)]
fn set(
    g: &mut Globals,
    r: &mut Run,
    m: &mut Mri,
    c: i32,
    rr: i32,
    s: i32,
    f: i32,
    v: f64,
) -> Result<(), Fatal> {
    mri_set_vox_val(g, r.err, m, c, rr, s, f, v as f32, 0)?;
    Ok(())
}

/// A volume function's new volume; `NULL` ends the program with status 1 (FreeSurfer's later use
/// of it would crash: none of the calls here returns `NULL` on the program's paths but those
/// noted).
fn fresh(ret: Ret<Mri>) -> Result<Mri, Fatal> {
    match ret {
        Ret::New(m) => Ok(m),
        _ => Err(Fatal::Exit(1)),
    }
}

/// `MRImultiplyConst(m, k, m)` (or `MRIaddConst`, `mul` false): each voxel (columns outermost,
/// then rows, slices, frames) `v * k` (`v + k`) in `double`, in place.
fn const_op_in_place(
    g: &mut Globals,
    r: &mut Run,
    m: &mut Mri,
    k: f64,
    mul: bool,
) -> Result<(), Fatal> {
    for c in 0..m.vg.width {
        for rr in 0..m.vg.height {
            for s in 0..m.vg.depth {
                for f in 0..m.nframes {
                    let v = get(g, r, m, c, rr, s, f)?;
                    let w = if mul { mul64(v, k) } else { add64(v, k) };
                    set(g, r, m, c, rr, s, f, w)?;
                }
            }
        }
    }
    Ok(())
}

/// `MRIsqrt(m, m)` (`MRIsquareRoot(m, NULL, m)`): each voxel `sqrt(fabs(v))` in `double`, in
/// place (columns outermost, then rows, slices, frames).
fn sqrt_in_place(g: &mut Globals, r: &mut Run, m: &mut Mri) -> Result<(), Fatal> {
    for c in 0..m.vg.width {
        for rr in 0..m.vg.height {
            for s in 0..m.vg.depth {
                for f in 0..m.nframes {
                    let v = get(g, r, m, c, rr, s, f)?;
                    set(g, r, m, c, rr, s, f, sqrt64(v.abs()))?;
                }
            }
        }
    }
    Ok(())
}

/// `fMRIcumSum(m, mask, m)`: frame `f` the sum of frames 0 to `f` (`double`, each value plus the
/// running sum), in place, over the voxels whose mask value is 0.5 or more (slices outermost).
fn cum_sum_in_place(
    g: &mut Globals,
    r: &mut Run,
    m: &mut Mri,
    mask: Option<&Mri>,
) -> Result<(), Fatal> {
    for s in 0..m.vg.depth {
        for rr in 0..m.vg.height {
            for c in 0..m.vg.width {
                if let Some(mk) = mask
                    && get(g, r, mk, c, rr, s, 0)? < 0.5
                {
                    continue;
                }
                let mut val = 0.0f64;
                for f in 0..m.nframes {
                    val = add64(get(g, r, m, c, rr, s, f)?, val);
                    set(g, r, m, c, rr, s, f, val)?;
                }
            }
        }
    }
    Ok(())
}

/// `GroupedMeanMatrix(ngroups, ntotal)`: `nper = ntotal / ngroups` rows of `ntotal` columns,
/// `1 / ngroups` (`float`) at columns `r`, `r + nper`, ... of row `r`; `ERROR: --gmean, Ng must be
/// integer divisor of Ntotal` and `None` when `nper` times `ngroups` is not `ntotal`.
fn grouped_mean_matrix(
    g: &mut Globals,
    r: &mut Run,
    ngroups: i32,
    ntotal: i32,
) -> Result<Option<Matrix>, Fatal> {
    let nper = ntotal.wrapping_div(ngroups);
    if nper.wrapping_mul(ngroups) != ntotal {
        let _ = writeln!(
            r.out,
            "ERROR: --gmean, Ng must be integer divisor of Ntotal"
        );
        return Ok(None);
    }
    let Some(mut m) = matrix_alloc(g, r.out, r.err, nper, ntotal, MATRIX_REAL)? else {
        return Ok(None);
    };
    let w = div64(1.0, f64::from(ngroups)) as f32;
    for row in 1..=nper {
        let mut c = row;
        while c <= ntotal {
            m.set_relt(row, c, w);
            c += nper;
        }
    }
    Ok(Some(m))
}

/// `MRIfirstNonZero(mri, mask, NULL)`: a one-frame volume of the input's type (its header, colour
/// table and pulse parameters), each voxel the first frame's value that is not 0 (NaN included),
/// else 0; 0 where the mask is below 0.5.
fn mri_first_non_zero(
    g: &mut Globals,
    r: &mut Run,
    mri: &Mri,
    mask: Option<&Mri>,
) -> Result<Mri, Fatal> {
    let mut o = mri_alloc_sequence(
        g,
        r.out,
        r.err,
        mri.vg.width,
        mri.vg.height,
        mri.vg.depth,
        mri.mtype,
        1,
    )?;
    mri_copy_header(g, r.out, r.err, mri, Some(&mut o))?;
    if mri.ct.is_some() {
        o.ct = ctab_deep_copy(g, r.err, mri.ct.as_ref());
    }
    mri_copy_pulse_parameters(mri, &mut o);
    for c in 0..mri.vg.width {
        for rr in 0..mri.vg.height {
            for s in 0..mri.vg.depth {
                set(g, r, &mut o, c, rr, s, 0, 0.0)?;
                if let Some(mk) = mask
                    && get(g, r, mk, c, rr, s, 0)? < 0.5
                {
                    continue;
                }
                for f in 0..mri.nframes {
                    let v = get(g, r, mri, c, rr, s, f)?;
                    if v != 0.0 {
                        set(g, r, &mut o, c, rr, s, 0, v)?;
                        break;
                    }
                }
            }
        }
    }
    Ok(o)
}

/// FS-1248: a mask that `--vote`, `--sort`, `--first-non-zero` or `--cumsum` reads at the output's
/// voxels must have its dimensions (by default `ERROR: --mask's dimensions differ from the
/// volume's`, status 1; `reproduce` reads it as FreeSurfer does, past its voxels).
fn check_mask(g: &mut Globals, r: &mut Run, mask: Option<&Mri>, like: &Mri) -> Result<(), Fatal> {
    let Some(mk) = mask else {
        return Ok(());
    };
    if [mk.vg.width, mk.vg.height, mk.vg.depth] == [like.vg.width, like.vg.height, like.vg.depth] {
        return Ok(());
    }
    g.notices.add(
        &FS_1248,
        if g.notices.reproduce() {
            format!(
                "mri_concat --mask of {}x{}x{} on a volume of {}x{}x{}: read at the volume's voxels, as in FreeSurfer",
                mk.vg.width, mk.vg.height, mk.vg.depth, like.vg.width, like.vg.height, like.vg.depth
            )
        } else {
            format!(
                "mri_concat --mask of {}x{}x{} on a volume of {}x{}x{}: refused, status 1 (FreeSurfer reads it at the volume's voxels)",
                mk.vg.width, mk.vg.height, mk.vg.depth, like.vg.width, like.vg.height, like.vg.depth
            )
        },
    );
    if g.notices.fix() {
        let _ = writeln!(r.out, "ERROR: --mask's dimensions differ from the volume's");
        return Err(Fatal::Exit(1));
    }
    Ok(())
}

/// FS-1242: a frame count past the `int` range (`reproduce`: wrapped, as the official build
/// computes it; by default `ERROR: <what> passes 2147483647 frames`, status 1).
fn frames_product(g: &mut Globals, r: &mut Run, a: i32, b: i32, what: &str) -> Result<i32, Fatal> {
    let exact = i64::from(a) * i64::from(b);
    frames_checked(g, r, exact, what)
}

fn frames_checked(g: &mut Globals, r: &mut Run, exact: i64, what: &str) -> Result<i32, Fatal> {
    if i32::try_from(exact).is_ok() {
        return Ok(exact as i32);
    }
    g.notices.add(
        &FS_1242,
        if g.notices.reproduce() {
            format!("mri_concat: {what} of {exact} frames wraps in an int, as in FreeSurfer")
        } else {
            format!(
                "mri_concat: {what} of {exact} frames refused, status 1 (FreeSurfer's int wraps)"
            )
        },
    );
    if g.notices.fix() {
        let _ = writeln!(r.out, "ERROR: {what} passes 2147483647 frames");
        return Err(Fatal::Exit(1));
    }
    Ok(exact as i32)
}

/// `main(argc, argv)` on the arguments after the program's name. `Ok` is status 0.
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let (nargs, argv) = handle_version_option(g, &r.env, r.out, &argv, NAME);
    if nargs > 0 && argv.len() == 1 {
        return Ok(());
    }
    g.progname = NAME.to_owned();
    diag_init(g, &r.env, r.out);
    let args = &argv[1..];
    if args.is_empty() {
        return Err(usage_exit(r));
    }
    let mut o = parse_commandline(g, r, args)?;
    check_options(r, &o)?;
    let inlist = o.inputs.clone();
    let out_name = o.out.clone().unwrap_or_default();
    let verbose = g.gdiag_no > 0 || o.debug;

    let mut mask = None;
    if let Some(mf) = o.maskfile.clone() {
        let _ = writeln!(r.out, "Loading mask {mf}");
        match mri_read_volume(g, r.out, r.err, &r.env, r.dir, &mf)? {
            Some(m) => mask = Some(m),
            None => return Err(Fatal::Exit(1)),
        }
    }

    let _ = writeln!(r.out, "ninputs = {}", inlist.len());
    let (mut nc, mut nr, mut ns) = (0, 0, 0);
    let mut nframestot = 0i64;
    let mut input_datatype = MRI_UCHAR;
    if o.do_check {
        let _ = writeln!(r.out, "Checking inputs");
        for (nthin, name) in inlist.iter().enumerate() {
            if verbose {
                let _ = writeln!(r.out, "Checking {nthin:2} {name}");
                let _ = r.out.flush();
            }
            let Some(h) = mri_read_header(
                g,
                r.out,
                r.err,
                &r.env,
                r.dir,
                name,
                MRI_VOLUME_TYPE_UNKNOWN,
            )?
            else {
                let _ = writeln!(r.out, "ERROR: reading {name}");
                return Err(Fatal::Exit(1));
            };
            if nthin == 0 {
                (nc, nr, ns) = (h.vg.width, h.vg.height, h.vg.depth);
            }
            if h.vg.width != nc || h.vg.height != nr || h.vg.depth != ns {
                let _ = writeln!(
                    r.out,
                    "ERROR: dimension mismatch between {} and {name}",
                    inlist[0]
                );
                return Err(Fatal::Exit(1));
            }
            nframestot += if o.in_frame_no < 0 {
                i64::from(h.nframes)
            } else {
                1
            };
            input_datatype = h.mtype;
        }
    } else {
        let _ = writeln!(r.out, "NOT Checking inputs, assuming nframestot = ninputs");
        nframestot = inlist.len() as i64;
        let Some(h) = mri_read_header(
            g,
            r.out,
            r.err,
            &r.env,
            r.dir,
            &inlist[0],
            MRI_VOLUME_TYPE_UNKNOWN,
        )?
        else {
            let _ = writeln!(r.out, "ERROR: reading {}", inlist[0]);
            return Err(Fatal::Exit(1));
        };
        (nc, nr, ns) = (h.vg.width, h.vg.height, h.vg.depth);
    }
    let nframestot = frames_checked(g, r, nframestot, "the inputs' frames")?;
    let _ = writeln!(r.out, "nframestot = {nframestot}");

    if o.do_rms {
        if inlist.len() != 1 {
            let _ = writeln!(
                r.out,
                "ERROR: --rms supports only single input w/ multiple frames"
            );
            return Err(Fatal::Exit(1));
        }
        if nframestot == 1 {
            let _ = writeln!(r.out, "ERROR: --rms input must have multiple frames");
            return Err(Fatal::Exit(1));
        }
    }

    if o.ngroups != 0 {
        let _ = writeln!(
            r.out,
            "Creating grouped mean matrix ngroups={}, nper={}",
            o.ngroups,
            nframestot.wrapping_div(o.ngroups)
        );
        let Some(m) = grouped_mean_matrix(g, r, o.ngroups, nframestot)? else {
            return Err(Fatal::Exit(1));
        };
        if o.debug {
            let mut buf = Vec::new();
            matrix_print(g, Some(&mut buf), &mut std::io::sink(), r.err, Some(&m), 5);
            let _ = r.out.write_all(&buf);
        }
        o.m = Some(m);
    }

    if let Some(m) = &o.m
        && nframestot != m.cols
    {
        let shown = if m.rows != m.cols {
            g.notices.add(
                &FS_1246,
                if g.notices.reproduce() {
                    format!(
                        "mri_concat: the matrix's {} columns refused for {nframestot} frames; its rows ({}) printed, as in FreeSurfer",
                        m.cols, m.rows
                    )
                } else {
                    format!(
                        "mri_concat: the matrix's {} columns refused for {nframestot} frames, the columns printed (FreeSurfer prints its rows, {})",
                        m.cols, m.rows
                    )
                },
            );
            if g.notices.reproduce() {
                m.rows
            } else {
                m.cols
            }
        } else {
            m.rows
        };
        let _ = writeln!(
            r.out,
            "ERROR: dimension mismatch between inputs ({nframestot}) and matrix ({shown})"
        );
        return Err(Fatal::Exit(1));
    }

    if o.do_paired && libm_remainder(f64::from(nframestot), 2.0) != 0.0 {
        let _ = r
            .out
            .write_all(b"ERROR: --paired-xxx specified but there are an odd number of frames\n");
        return Err(Fatal::Exit(1));
    }

    if let Some(w) = &o.frame_weight
        && nframestot != w.rows
    {
        let _ = writeln!(
            r.out,
            "ERROR: dimension mismatch between inputs ({nframestot}) and FrameWeight ({})",
            w.rows
        );
        return Err(Fatal::Exit(1));
    }

    let _ = writeln!(r.out, "Allocing output");
    let _ = r.out.flush();
    let datatype = if o.do_keep_datatype {
        input_datatype
    } else {
        MRI_FLOAT
    };
    let mut mriout = mri_alloc_sequence(
        g,
        r.out,
        r.err,
        nc,
        nr,
        ns,
        datatype,
        if o.do_rms { 1 } else { nframestot },
    )?;
    let _ = writeln!(r.out, "Done allocing");

    let mut ctab = o.ctab.take();
    let mut fout = 0i32;
    for (nthin, name) in inlist.iter().enumerate() {
        if o.do_rms {
            break; // MRIrms reads the input frames
        }
        if verbose {
            let base = fio_basename(Some(name), None).unwrap_or_default();
            let _ = writeln!(r.out, "Loading {}th input {base}", nthin + 1);
            let _ = r.out.flush();
        }
        let Some(mut mritmp) = mri_read_volume(g, r.out, r.err, &r.env, r.dir, name)? else {
            let _ = writeln!(r.out, "ERROR: loading {name}");
            return Err(Fatal::Exit(1));
        };
        if o.in_frame_no >= 0 {
            let _ = writeln!(r.out, "Extracting frame {}", o.in_frame_no);
            if o.in_frame_no >= mritmp.nframes {
                let _ = writeln!(
                    r.out,
                    "ERROR: InFrameNo >= mritmp->nframes={}",
                    mritmp.nframes
                );
                return Err(Fatal::Exit(1));
            }
            match fmri_extract_frames(g, r.out, r.err, &mritmp, &[o.in_frame_no])? {
                Some(m) => mritmp = m,
                None => return Err(Fatal::Exit(1)),
            }
        }
        if !o.do_check {
            no_check_mismatch(
                g,
                r,
                &inlist[0],
                name,
                &mritmp,
                [nc, nr, ns],
                fout,
                nframestot,
            )?;
        }
        if nthin == 0 {
            mri_copy_header(g, r.out, r.err, &mritmp, Some(&mut mriout))?;
            if ctab.is_none() && mritmp.ct.is_some() {
                ctab = ctab_deep_copy(g, r.err, mritmp.ct.as_ref());
            }
        }
        if o.do_abs {
            if verbose {
                let _ = writeln!(r.out, "Removing sign from input");
            }
            mri_abs(g, r.out, r.err, Pair::Same(&mut mritmp))?;
        }
        if o.do_pos {
            if verbose {
                let _ = writeln!(r.out, "Setting input negatives to 0.");
            }
            mri_pos(g, r.out, r.err, Pair::Same(&mut mritmp))?;
        }
        if o.do_neg {
            if verbose {
                let _ = writeln!(r.out, "Setting input positives to 0.");
            }
            mri_neg(g, r.out, r.err, Pair::Same(&mut mritmp))?;
        }
        for f in 0..mritmp.nframes {
            let wt = o.frame_weight.as_ref().map(|w| weight(g, w, fout));
            for c in 0..nc {
                for rr in 0..nr {
                    for s in 0..ns {
                        let mut v = get(g, r, &mritmp, c, rr, s, f)?;
                        if let Some(w) = wt {
                            v = mul64(v, w);
                        }
                        set(g, r, &mut mriout, c, rr, s, fout, v)?;
                    }
                }
            }
            fout = fout.wrapping_add(1);
        }
    }

    if o.do_combine {
        let mut mritmp = mri_alloc_sequence(g, r.out, r.err, nc, nr, ns, MRI_FLOAT, 1)?;
        mri_copy_header(g, r.out, r.err, &mriout, Some(&mut mritmp))?;
        for c in 0..nc {
            for rr in 0..nr {
                for s in 0..ns {
                    let mut nhits = 0i32;
                    let mut vsum = 0.0f64;
                    for f in 0..mriout.nframes {
                        let v = get(g, r, &mriout, c, rr, s, f)?;
                        if v > 0.0 {
                            vsum = add64(vsum, v);
                            nhits += 1;
                        }
                    }
                    if nhits > 0 {
                        set(
                            g,
                            r,
                            &mut mritmp,
                            c,
                            rr,
                            s,
                            0,
                            div64(vsum, f64::from(nhits)),
                        )?;
                    }
                }
            }
        }
        mriout = mritmp;
    }

    let mut prune_mask = None;
    if o.do_prune {
        let _ = writeln!(r.out, "Computing prune mask ");
        let pm = fresh(mri_frame_binarize(
            g,
            r.out,
            r.err,
            &mriout,
            f64::from(f32::MIN_POSITIVE),
            None,
        )?)?;
        let n = mri_n_mask(g, r.err, &pm)?;
        let _ = writeln!(r.out, "Found {n} voxels in prune mask");
        prune_mask = Some(pm);
    }

    if o.do_norm_mean {
        let _ = writeln!(r.out, "Normalizing by mean across frames");
        mri_normalize_frames_mean(g, r.err, &mut mriout)?;
    }
    if o.do_norm1 {
        let _ = writeln!(r.out, "Normalizing by first across frames");
        mri_normalize_frames_first(g, r.err, &mut mriout)?;
    }

    if o.do_asl {
        let _ = writeln!(r.out, "Computing ASL matrix matrix");
        o.m = asl_interp_matrix(g, r.out, r.err, mriout.nframes)?;
    }

    if let Some(m) = &o.m {
        let _ = writeln!(r.out, "Multiplying by matrix");
        match fmri_matrix_multiply(g, r.out, r.err, &mriout, m, None)? {
            Ret::New(t) => mriout = t,
            _ => return Err(Fatal::Exit(1)),
        }
    }

    if o.do_paired {
        let _ = writeln!(r.out, "Combining pairs");
        let mut mritmp = mri_clone_by_space(g, r.out, r.err, &mriout, -1, mriout.nframes / 2)?;
        for c in 0..nc {
            for rr in 0..nr {
                for s in 0..ns {
                    let mut fo = 0;
                    let mut f = 0;
                    while f < mriout.nframes {
                        let v1 = get(g, r, &mriout, c, rr, s, f)?;
                        let v2 = get(g, r, &mriout, c, rr, s, f + 1)?;
                        let mut v = 0.0f64;
                        if o.do_paired_avg {
                            v = div64(add64(v1, v2), 2.0);
                        }
                        if o.do_paired_sum {
                            v = add64(v1, v2);
                        }
                        if o.do_paired_diff {
                            v = sub64(v1, v2);
                        }
                        if o.do_paired_diff_norm {
                            v = sub64(v1, v2);
                            let vavg = div64(add64(v1, v2), 2.0);
                            v = if vavg != 0.0 { div64(v, vavg) } else { 0.0 };
                        }
                        if o.do_paired_diff_norm1 {
                            v = sub64(v1, v2);
                            v = if v1 != 0.0 { div64(v, v1) } else { 0.0 };
                        }
                        if o.do_paired_diff_norm2 {
                            v = sub64(v1, v2);
                            v = if v2 != 0.0 { div64(v, v2) } else { 0.0 };
                        }
                        set(g, r, &mut mritmp, c, rr, s, fo, v)?;
                        fo += 1;
                        f += 2;
                    }
                }
            }
        }
        mriout = mritmp;
    }
    let nframes = mriout.nframes;
    let _ = writeln!(r.out, "nframes = {nframes}");

    if o.do_bonf_cor {
        o.do_add = true;
        let libm = Libm::new(g.mode());
        o.add_val = -libm.log10(f64::from(mriout.nframes));
        note_glibc07(g, &libm, "log10");
    }

    if o.do_mean {
        let _ = writeln!(r.out, "Computing mean across frames");
        mriout = fresh(mri_frame_mean(g, r.out, r.err, &mriout, None, &[])?)?;
    }
    if o.do_median {
        let _ = writeln!(r.out, "Computing median across frames");
        mriout = fresh(mri_frame_median(g, r.out, r.err, &mriout, None)?)?;
    }
    if o.do_mean_div_n {
        let _ = writeln!(r.out, "Computing mean2 = sum/(nframes^2)");
        mriout = fresh(mri_frame_sum(g, r.out, r.err, &mriout, None)?)?;
        let n2 = frames_product(g, r, nframes, nframes, "--mean-div-n's nframes^2")?;
        const_op_in_place(g, r, &mut mriout, div64(1.0, f64::from(n2)), true)?;
    }
    if o.do_sum {
        let _ = writeln!(r.out, "Computing sum across frames");
        mriout = fresh(mri_frame_sum(g, r.out, r.err, &mriout, None)?)?;
    }
    if o.do_fnorm {
        let _ = writeln!(r.out, "Normalizing across frames");
        mriout = fresh(mri_frame_norm(g, r.out, r.err, &mriout, None, None)?)?;
    }
    if o.do_tar1 {
        let _ = writeln!(
            r.out,
            "Computing temoral AR1 {}",
            mriout.nframes.wrapping_sub(o.tar1_dof_adjust)
        );
        mriout = fresh(fmri_temporal_ar1(
            g,
            r.out,
            r.err,
            &mriout,
            o.tar1_dof_adjust as f32,
            None,
            None,
        )?)?;
    }

    if o.do_std || o.do_var {
        let _ = writeln!(r.out, "Computing std/var across frames");
        if mriout.nframes < 2 {
            let _ = writeln!(r.out, "ERROR: cannot compute std from one frame");
            return Err(Fatal::Exit(1));
        }
        // the frames are no longer needed once the variance is made (FreeSurfer frees them after
        // the square root, which it takes in place)
        mriout = fresh(fmri_covariance(
            g, r.out, r.err, &mriout, 0, -1.0, None, None,
        )?)?;
        if o.do_std {
            sqrt_in_place(g, r, &mut mriout)?;
        }
    }

    if o.do_max {
        let _ = writeln!(r.out, "Computing max across all frames ");
        mriout = fresh(mri_vol_max(g, r.out, r.err, &mriout, None)?)?;
    }

    if o.do_max_index {
        let _ = writeln!(r.out, "Computing max index across all frames ");
        let mut mritmp = fresh(mri_vol_max_index(g, r.out, r.err, &mriout, 1, None, None)?)?;
        if o.do_max_index_prune {
            let _ = writeln!(r.out, "Pruning max index");
            for c in 0..nc {
                for rr in 0..nr {
                    for s in 0..ns {
                        let mut all_zero = true;
                        for f in 0..mriout.nframes {
                            if get(g, r, &mriout, c, rr, s, f)?.abs() > 0.0 {
                                all_zero = false;
                                break;
                            }
                        }
                        if all_zero {
                            set(g, r, &mut mritmp, c, rr, s, 0, 0.0)?;
                        }
                    }
                }
            }
        }
        mriout = mritmp;
        if o.do_max_index_add {
            let _ = writeln!(r.out, "Adding {} to index", o.max_index_add);
            for c in 0..nc {
                for rr in 0..nr {
                    for s in 0..ns {
                        // f = MRIgetVoxVal(...): the float made an int
                        let f = cvtt(mri_get_vox_val(g, r.err, &mriout, c, rr, s, 0, 0)?);
                        if f == 0 {
                            continue;
                        }
                        let v = f.wrapping_add(o.max_index_add) as f32;
                        mri_set_vox_val(g, r.err, &mut mriout, c, rr, s, 0, v, 0)?;
                    }
                }
            }
        }
    }

    if o.do_conjunction {
        let _ = writeln!(r.out, "Computing conjunction across all frames ");
        mriout = fresh(mri_conjunct(g, r.out, r.err, &mriout, None)?)?;
    }
    if o.do_min {
        let _ = writeln!(r.out, "Computing min across all frames ");
        mriout = fresh(mri_vol_min(g, r.out, r.err, &mriout, None)?)?;
    }
    if o.do_sort {
        let _ = writeln!(r.out, "Sorting ");
        check_mask(g, r, mask.as_ref(), &mriout)?;
        mriout = fresh(mri_sort(g, r.out, r.err, &mriout, mask.as_ref(), None)?)?;
    }
    if o.do_vote {
        let _ = writeln!(r.out, "Voting ");
        check_mask(g, r, mask.as_ref(), &mriout)?;
        mriout = fresh(mri_vote(
            g,
            r.out,
            r.err,
            &mut mriout,
            mask.as_ref(),
            None,
            o.vote_exclude0,
        )?)?;
    }
    if o.do_first_non_zero {
        let _ = writeln!(r.out, "FirstNonZero ");
        check_mask(g, r, mask.as_ref(), &mriout)?;
        mriout = mri_first_non_zero(g, r, &mriout, mask.as_ref())?;
    }
    if o.do_multiply {
        let _ = writeln!(r.out, "Multiplying by {}", fmt_f(o.multiply_val, 0, 6));
        const_op_in_place(g, r, &mut mriout, o.multiply_val, true)?;
    }
    if o.do_add {
        let _ = writeln!(r.out, "Adding {}", fmt_f(o.add_val, 0, 6));
        const_op_in_place(g, r, &mut mriout, o.add_val, false)?;
    }
    if o.do_cum_sum {
        let _ = writeln!(r.out, "Computing cumulative sum");
        check_mask(g, r, mask.as_ref(), &mriout)?;
        cum_sum_in_place(g, r, &mut mriout, mask.as_ref())?;
    }

    if o.do_scm {
        let _ = writeln!(
            r.out,
            "Computing spatial correlation matrix ({})",
            mriout.nframes
        );
        match fmri_spatial_cor_matrix(g, r.out, r.err, &mriout)? {
            Some(m) => mriout = m,
            None => return Err(Fatal::Exit(1)),
        }
    }

    if o.do_pca {
        let _ = writeln!(r.out, "Computing PCA");
        let mut pca_mask = None;
        if let Some(pm) = o.pca_mask_file.clone() {
            let _ = writeln!(r.out, "  PCA Mask {pm}");
            match mri_read_volume(g, r.out, r.err, &r.env, r.dir, &pm)? {
                Some(m) => pca_mask = Some(m),
                None => return Err(Fatal::Exit(1)),
            }
        }
        let (err, pca) = mri_pca(g, r.out, r.err, &mriout, pca_mask.as_ref())?;
        let Some(pca) = pca.filter(|_| err == 0) else {
            return Err(Fatal::Exit(1));
        };
        let stem = match id_stem_from_name(&out_name) {
            Some(s) => s,
            None => {
                g.notices.add(
                    &FS_1245,
                    if g.notices.reproduce() {
                        format!(
                            "mri_concat --pca: the output {out_name} has no volume extension: its matrices named (null).u.mtx and (null).stats.dat, as in FreeSurfer"
                        )
                    } else {
                        format!(
                            "mri_concat --pca: the output {out_name} has no volume extension: its matrices named after the whole name (FreeSurfer's are (null).u.mtx and (null).stats.dat)"
                        )
                    },
                );
                if g.notices.reproduce() {
                    "(null)".to_owned()
                } else {
                    out_name.clone()
                }
            }
        };
        let umtx = format!("{stem}.u.mtx");
        pca_name(g, r, &umtx)?;
        matrix_write_txt(g, r.err, r.dir, &umtx, &pca.u);
        let stats = format!("{stem}.stats.dat");
        pca_name(g, r, &stats)?;
        write_pca_stats(g, r.out, r.dir, &stats, &pca.s);
        mriout = pca.v;
    }

    if o.n_replications > 0 {
        let _ = writeln!(r.out, "NReplications {}", o.n_replications);
        let nf = frames_product(g, r, mriout.nframes, o.n_replications, "--rep's frames")?;
        let mut mritmp = mri_alloc_sequence(
            g,
            r.out,
            r.err,
            mriout.vg.width,
            mriout.vg.height,
            mriout.vg.depth,
            mriout.mtype,
            nf,
        )?;
        let _ = writeln!(r.out, "Done allocing");
        mri_copy_header(g, r.out, r.err, &mriout, Some(&mut mritmp))?;
        for c in 0..mriout.vg.width {
            for rr in 0..mriout.vg.height {
                for s in 0..mriout.vg.depth {
                    let mut outf = 0i32;
                    for _ in 0..o.n_replications {
                        for f in 0..mriout.nframes {
                            let v = get(g, r, &mriout, c, rr, s, f)?;
                            set(g, r, &mut mritmp, c, rr, s, outf, v)?;
                            outf = outf.wrapping_add(1);
                        }
                    }
                }
            }
        }
        mriout = mritmp;
    }

    if let Some(pm) = &prune_mask {
        let _ = writeln!(r.out, "Applying prune mask ");
        mri_mask(g, r.out, r.err, &r.env, Pair::Same(&mut mriout), pm, 0, 0.0)?;
    }

    if o.do_rms {
        let _ = writeln!(r.out, "Computing RMS across input frames");
        let Some(mritmp) = mri_read_volume(g, r.out, r.err, &r.env, r.dir, &inlist[0])? else {
            g.notices.add(
                &FS_1200,
                format!(
                    "mri_concat --rms: {} does not read whole: status 1 (FreeSurfer goes on with no volume and crashes)",
                    inlist[0]
                ),
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(139));
            }
            return Err(Fatal::Exit(1));
        };
        mri_copy_header(g, r.out, r.err, &mritmp, Some(&mut mriout))?;
        mri_rms(g, r.err, &mritmp, &mut mriout)?;
    }

    if let Some(t) = ctab {
        mriout.ct = Some(t);
    }

    let _ = writeln!(r.out, "Writing to {out_name}");
    let err = mri_write(
        g,
        r.out,
        r.err,
        &r.env,
        r.dir,
        &mut mriout,
        &out_name,
        None,
        MGZ_INTENT_UNKNOWN,
    )?;
    if err != 0 {
        return Err(Fatal::Exit(err));
    }
    if o.debug {
        print_rusage("mri_ca_label ", r.out);
    }
    if let Some(rf) = &o.rusage_file {
        write_rusage(r.dir, "", rf);
    }
    Ok(())
}

/// `--pca`'s `sprintf(tmpstr, "%s.<ext>", stem)` into `tmpstr[2000]` (FS-1244: a name of 2,000
/// bytes or more with its NUL passes the buffer: the official build aborts, `reproduce`; by
/// default the whole name).
fn pca_name(g: &mut Globals, r: &mut Run, name: &str) -> Result<(), Fatal> {
    if name.len() < 2000 {
        return Ok(());
    }
    g.notices.add(
        &FS_1244,
        format!(
            "mri_concat --pca: a matrix name of {} bytes used whole (FreeSurfer's buffer holds 1,999: the official build aborts)",
            name.len()
        ),
    );
    if g.notices.reproduce() {
        let _ = r
            .err
            .write_all(b"*** buffer overflow detected ***: terminated\n");
        return Err(Fatal::Exit(134));
    }
    Ok(())
}

/// `FrameWeight->rptr[fout + 1][1]` as a `double`; past the matrix's rows (`--no-check` with more
/// frames than inputs, FS-1243, `reproduce`) memory contents, filled with 0.
fn weight(g: &mut Globals, w: &Matrix, fout: i32) -> f64 {
    let row = fout.wrapping_add(1);
    match w.relt_index(row, 1) {
        Some(_) if (1..=w.rows).contains(&row) => f64::from(w.relt(row, 1)),
        _ => {
            g.notices.add(
                &FS_1243,
                format!(
                    "mri_concat --w: frame {} has no weight (the matrix has {} rows): FreeSurfer reads memory contents (filled with 0)",
                    fout + 1,
                    w.rows
                ),
            );
            0.0
        }
    }
}

/// FS-1241: with `--no-check` an input of other dimensions than the first, or more frames than
/// the output has left: by default `ERROR: dimension mismatch between <first> and <in>` (or
/// `ERROR: <in> has more frames than counted`), status 1; `reproduce` reads and writes past the
/// volumes as FreeSurfer does.
#[allow(clippy::too_many_arguments)]
fn no_check_mismatch(
    g: &mut Globals,
    r: &mut Run,
    first: &str,
    name: &str,
    m: &Mri,
    dims: [i32; 3],
    fout: i32,
    nframestot: i32,
) -> Result<(), Fatal> {
    let other = [m.vg.width, m.vg.height, m.vg.depth] != dims;
    let more = i64::from(fout) + i64::from(m.nframes) > i64::from(nframestot);
    if !other && !more {
        return Ok(());
    }
    g.notices.add(
        &FS_1241,
        if g.notices.reproduce() {
            format!(
                "mri_concat --no-check: {name} ({}x{}x{}, {} frames) does not fit the output ({}x{}x{}, {nframestot} frames): read and written past the volumes, as in FreeSurfer",
                m.vg.width, m.vg.height, m.vg.depth, m.nframes, dims[0], dims[1], dims[2]
            )
        } else {
            format!(
                "mri_concat --no-check: {name} ({}x{}x{}, {} frames) does not fit the output ({}x{}x{}, {nframestot} frames): refused, status 1 (FreeSurfer reads and writes past the volumes)",
                m.vg.width, m.vg.height, m.vg.depth, m.nframes, dims[0], dims[1], dims[2]
            )
        },
    );
    if g.notices.fix() {
        if other {
            let _ = writeln!(
                r.out,
                "ERROR: dimension mismatch between {first} and {name}"
            );
        } else {
            let _ = writeln!(r.out, "ERROR: {name} has more frames than counted");
        }
        return Err(Fatal::Exit(1));
    }
    Ok(())
}

/// C's `remainder(x, y)` (round to nearest, ties to even) for the frame count's parity.
fn libm_remainder(x: f64, y: f64) -> f64 {
    let q = (x / y).round_ties_even();
    x - q * y
}

/// `(int)` of a `float` as x86-64 converts it (`cvttss2si`: NaN and values outside the range
/// give `INT_MIN`; the max index's values are whole numbers of the frame count's range).
fn cvtt(x: f32) -> i32 {
    if x.is_nan() || !(-2_147_483_648.0..2_147_483_648.0).contains(&x) {
        i32::MIN
    } else {
        x as i32
    }
}
