// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_coreg`'s `main` (`mri_coreg.cpp` 193-555): the entry point [`run`] (registered as
//! `mri_coreg` in the crate's `PROGRAMS`).
//!
//! The order of `main`: the version options (`handleVersionOption`), the command line kept for
//! the report, `uname`, `getcwd`; no argument: the usage (status 1); the options
//! ([`super::options`]), FreeSurfer's environment record (`FSENVgetenv`: `FREESURFER_HOME`, its
//! colour table, `SUBJECTS_DIR`; its messages), their checks; `--checkopts` ends here (status 0);
//! the report (`dump_options`). Then the seed, the moving and reference volumes (`--conf-ref`:
//! the reference conformed without rescaling), the moving volume's intensity dither, the masks
//! (each voxel whose mask value, as an `int`, is below 0.5 set to 0), `--ref-fwhm`'s smoothing;
//! the initial translation (the volumes' centres, or with `--centroid` their centroids, mapped to
//! RAS); the initial parameters and matrix (`--init-reg-save`, `--init-reg-save-only`); the
//! dithers (three generators seeded alike: the coordinates' for every voxel of the reference and
//! three frames, the reference's intensities, the moving volume's with its parameters 0, which
//! adds 0); `COREGprint`; the preprocessing (each volume's saturation percentile, rescaled to
//! bytes, smoothed for the smallest separation, dithered); `--movout`; the cost log; the overlap
//! test at separation 4 (`ERROR: mov and ref do not overlap`, status 1); `--init-cost-only`; for
//! each separation the brute-force search (the first only, unless `--no-bf`) and Powell's
//! minimisation; the registration written (voxel to voxel, or `--ras2ras`; `--regdat` as
//! `register.dat`), the final matrices and parameters, `--params`, `--final-cost`, the overlap,
//! the time, the `tkregisterfv` line, `mri_coreg done`.
//!
//! Defects: FS-1315 (a subject's file name FreeSurfer cannot form: no environment record, or a
//! name past its buffer), FS-1322 (`--dof` above 12 with the default schema: FreeSurfer reads and
//! writes past its twelve parameters), FS-1323 (a separation of 0 or less: FreeSurfer divides by
//! 0 or never ends), FS-1324 (an output file that does not open where FreeSurfer writes through
//! the `NULL` file: `--init-cost-only`, `--params`, `--final-cost`), FS-1325 (a singular final
//! voxel-to-voxel matrix: FreeSurfer writes a registration without a matrix and crashes); the
//! others in [`super::options`], [`super::standalone`] and [`super::coreg`].

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_1315, FS_1322, FS_1323, FS_1324, FS_1325};
use larmorx_core::math::Libm;
use larmorx_freesurfer_core::diag::diag_init;
use larmorx_freesurfer_core::fsenv::fsenv_getenv;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, Fatal};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{matrix_inverse, matrix_print};
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::version::{argv2cmdline, handle_version_option, ver_user};
use larmorx_freesurfer_io::colortab::ctab_read_ascii;
use larmorx_freesurfer_io::mriio::mri_write;
use larmorx_freesurfer_mri::lta::{LINEAR_RAS_TO_RAS, REGISTER_DAT};
use larmorx_freesurfer_core::cname::STRLEN;
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_UNKNOWN, Mri};
use larmorx_freesurfer_mri::vol_geom::xfm_crs2xyz;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::conform::mri_conform;
use larmorx_freesurfer_volume::ltaconv::lta_create;
use larmorx_freesurfer_volume::mriset::mri_compute_centroid;
use larmorx_freesurfer_volume::resample::mri_resample;
use larmorx_freesurfer_volume::sample::SAMPLE_NEAREST;
use larmorx_freesurfer_volume::rotmat::tranform_affine_params2_matrix;

use super::coreg::{
    Coreg, UniformField, coreg_fwhm, mri_get_percentile, mri_rescale_to_uchar, mri_to_uchar_vect,
    rf_synth_dither, smooth_in_place,
};
use super::options::{
    CmdArgs, RunInfo, check_options, dump_options, or_null, parse_commandline, usage_exit,
};
use super::standalone::{change_type, crash, read_volume, set_subject, write_lta};
use crate::mri_mask::main::Cout;

/// The program's name as `argv[0]` gives it (fMRIPrep and `bbregister` run it by name).
pub const NAME: &str = "mri_coreg";

/// `vcid`, the version line.
pub const VCID: &str = "$Id: mri_coreg.c,v 1.27 2016/04/30 15:11:49 greve Exp $";

/// `getcwd(cwd, 2000)`'s buffer: a longer name leaves it empty (as the zeroed global was).
const CWD_ROOM: usize = 2000;

/// The streams, environment and directory of a run (`--sd` changes the environment).
pub struct Run<'a, 'b> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: Environment,
    pub dir: Option<&'b Path>,
    /// The thread count (0: every logical CPU): the histogram's chunks run on it; FreeSurfer's
    /// `omp_get_max_threads()` is printed from it (or `--threads`).
    pub threads: usize,
}

/// `mri_coreg`'s entry point: its original arguments (without the program's name), the
/// settings, the two streams; the exit status. The notices of the run follow on standard error
/// (`larmorx: ...`).
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut g = Globals::new(settings.mode);
    g.stderr.read_write = system::stderr_read_write();
    let status = run_with(
        &mut g,
        args,
        settings.env.clone(),
        settings.dir.as_deref(),
        settings.threads,
        out,
        err,
    );
    g.notices.print(err);
    status
}

/// The program on `g`, its environment, directory and threads: standard output kept as the C
/// library buffers it (a crash's status in `reproduce` keeps only what FreeSurfer's buffer had
/// written out: what was flushed and the full buffers after it), then written to `out`.
pub fn run_with(
    g: &mut Globals,
    args: &[String],
    env: Environment,
    dir: Option<&Path>,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut cout = Cout::new(g);
    let status = {
        let mut terr = log::TrackedErr::new(g, &mut *err);
        let mut r = Run {
            out: &mut cout,
            err: &mut terr,
            env,
            dir,
            threads,
        };
        match main(g, &mut r, args) {
            Ok(()) => 0,
            Err(f) => f.status(),
        }
    };
    let keep = if matches!(status, 134 | 136 | 139) && g.notices.reproduce() {
        cout.kept_by_crash()
    } else {
        cout.buf.len()
    };
    let _ = out.write_all(&cout.buf[..keep]);
    let _ = out.flush();
    status
}

/// `MatrixPrint(stdout, m)`, which ends with `fflush(stdout)`.
pub fn print_matrix(g: &mut Globals, r: &mut Run, m: Option<&Matrix>) {
    let mut buf = Vec::new();
    matrix_print(g, Some(&mut buf), &mut std::io::sink(), r.err, m, 5);
    let _ = r.out.write_all(&buf);
    let _ = r.out.flush();
}

/// FS-1315: a subject's file name FreeSurfer cannot form (status 139 with `reproduce`: its
/// crash; a clear error by default).
pub fn fs_1315(g: &mut Globals, r: &mut Run, what: &str) -> Fatal {
    crash(g, r, &FS_1315, what)
}

/// An output file FreeSurfer opens and writes without a check (FS-1324).
fn write_unchecked(
    g: &mut Globals,
    r: &mut Run,
    name: &str,
    text: &str,
    option: &str,
) -> Result<(), Fatal> {
    let path = match r.dir {
        Some(d) => d.join(name),
        None => std::path::PathBuf::from(name),
    };
    if std::fs::write(&path, text.as_bytes()).is_err() {
        return Err(crash(
            g,
            r,
            &FS_1324,
            &format!("{option} {name}: the file does not open for writing (FreeSurfer writes through a NULL file and crashes)"),
        ));
    }
    Ok(())
}

/// The thread count FreeSurfer's OpenMP would report (`omp_get_max_threads()`): the last
/// `--threads`, else the run's.
fn omp_threads(c: &CmdArgs, r: &Run) -> usize {
    let procs = std::thread::available_parallelism().map_or(1, |n| n.get());
    let n = match c.threads {
        Some(t) if t > 0 => t as usize,
        Some(_) => 1,
        None if r.threads == 0 => procs,
        None => r.threads,
    };
    larmorx_freesurfer_core::romp::max_threads(n)
}

/// A mask applied: each voxel of `vol` whose mask value (as an `int`) is below 0.5 set to 0.
fn apply_mask(g: &mut Globals, r: &mut Run, vol: &mut Mri, mask: &Mri) -> Result<(), Fatal> {
    for c in 0..vol.vg.width {
        for rr in 0..vol.vg.height {
            for s in 0..vol.vg.depth {
                let m = super::coreg::to_int_x86(f64::from(mri_get_vox_val(
                    g, r.err, mask, c, rr, s, 0, 0,
                )?));
                if f64::from(m) < 0.5 {
                    mri_set_vox_val(g, r.err, vol, c, rr, s, 0, 0.0, 0)?;
                }
            }
        }
    }
    Ok(())
}

/// `MRIconformNoScale(mri, NULL)`: the volume resampled (nearest neighbour) onto `MRIconform`'s
/// grid, named `<name>/Conformed` (`snprintf` into `STRLEN` bytes: a longer name cut, with
/// `MRIconformNoScale: Truncation on line 2075` on standard error). The float volume FreeSurfer
/// allocates first is replaced unused (no output of it).
fn conform_no_scale(g: &mut Globals, r: &mut Run, mri: &Mri) -> Result<Option<Mri>, Fatal> {
    let env = r.env.clone();
    let Some(tmp) = mri_conform(g, r.out, r.err, &env, mri)? else {
        return Ok(None);
    };
    let Some(mut res) = mri_resample(g, r.out, r.err, mri, &tmp, SAMPLE_NEAREST)? else {
        return Ok(None);
    };
    let mut name = mri.vg.fname.c_bytes().to_vec();
    name.extend_from_slice(b"/Conformed");
    let room = STRLEN - 1;
    let n = name.len().min(room);
    let buf = res.vg.fname.buffer_mut();
    buf[..n].copy_from_slice(&name[..n]);
    buf[n] = 0;
    if name.len() >= STRLEN {
        let _ = writeln!(r.err, "MRIconformNoScale: Truncation on line 2075");
    }
    Ok(Some(res))
}

/// `main(argc, argv)` on the arguments after the program's name. `Ok` is status 0.
#[allow(clippy::too_many_lines)]
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    let start = std::time::Instant::now();
    let mut c = CmdArgs::default();
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let (nargs, left) = handle_version_option(g, &r.env, r.out, &argv, NAME);
    if nargs != 0 && left.len() == 1 {
        return Err(Fatal::Exit(0));
    }
    let cmdline = argv2cmdline(&left);
    let uts = system::uname(&r.env);
    let cwd = system::cwd(r.dir)
        .filter(|c| c.len() < CWD_ROOM)
        .unwrap_or_default();
    g.progname = NAME.to_owned();
    diag_init(g, &r.env, r.out);
    let args = &left[1..];
    if args.is_empty() {
        return Err(usage_exit(r.out, &c));
    }
    parse_commandline(g, r, &mut c, args)?;
    let env = r.env.clone();
    let fsenv = {
        let err = &mut *r.err;
        let dir = r.dir;
        fsenv_getenv(g, &env, dir, &mut *r.out, |g, out, file| {
            ctab_read_ascii(g, out, err, dir, file)
        })
    };
    let sd = fsenv.as_ref().map(|f| f.subjects_dir.as_deref());
    check_options(g, r, &mut c, sd)?;
    if c.checkoptsonly {
        return Ok(());
    }
    let info = RunInfo {
        cwd,
        cmdline,
        sysname: uts.as_ref().map(|u| u.sysname.clone()).unwrap_or_default(),
        hostname: uts.as_ref().map(|u| u.nodename.clone()).unwrap_or_default(),
        machine: uts.as_ref().map(|u| u.machine.clone()).unwrap_or_default(),
        user: ver_user(g, &r.env),
    };
    dump_options(r.out, &c, &info);

    // checks of what FreeSurfer cannot run (FS-1322, FS-1323), before the work
    if c.optschema == 1 && c.dof > 12 {
        return Err(crash(
            g,
            r,
            &FS_1322,
            &format!("--dof {}: more than 12 parameters (FreeSurfer reads and writes past its 12)", c.dof),
        ));
    }
    if let Some(&bad) = c.seplist.iter().find(|&&s| s <= 0) {
        g.notices.add(
            &FS_1323,
            format!("mri_coreg --sep {bad}: a separation of 0 or less (FreeSurfer divides by 0 or never ends)"),
        );
        let _ = writeln!(
            r.err,
            "{NAME}: --sep {bad}: the separation must be 1 or more (FreeSurfer divides by 0 or never ends)"
        );
        return Err(Fatal::Exit(1));
    }

    let _ = writeln!(r.out, "Seed {}", c.seed);
    let mov_name = c.mov.clone().expect("checked");
    let _ = writeln!(r.out, "Reading in mov {mov_name}");
    let Some(mut mov) = read_volume(g, r, &mov_name)? else {
        return Err(Fatal::Exit(1));
    };
    let ref_name = c.ref_vol.clone().expect("checked");
    let _ = writeln!(r.out, "Reading in ref {ref_name}");
    let Some(mut ref_vol) = read_volume(g, r, &ref_name)? else {
        return Err(Fatal::Exit(1));
    };
    if c.refconf != 0 {
        let _ = writeln!(r.out, "Conforming ref (no scaling or rehistogramming)");
        ref_vol = match conform_no_scale(g, r, &ref_vol)? {
            Some(m) => m,
            None => {
                return Err(crash(g, r, &FS_1325, "--conf-ref: the reference does not conform (FreeSurfer goes on with no volume and crashes)"));
            }
        };
    }
    let nvoxref = ref_vol
        .vg
        .width
        .wrapping_mul(ref_vol.vg.height)
        .wrapping_mul(ref_vol.vg.depth);

    let mut movidither = None;
    if let Some(name) = c.movidither_file.clone() {
        let _ = writeln!(r.out, "Reading mov intensity dither file {name}");
        let Some(m) = read_volume(g, r, &name)? else {
            return Err(Fatal::Exit(1));
        };
        movidither = Some(m);
    }
    if let Some(name) = c.refmask.clone() {
        let _ = writeln!(r.out, "Reading in and applying refmask {name}");
        let Some(m) = read_volume(g, r, &name)? else {
            return Err(Fatal::Exit(1));
        };
        apply_mask(g, r, &mut ref_vol, &m)?;
    }
    let libm = Libm::quiet(g.mode());
    if c.smooth_ref != 0 {
        let k = libm.log(256.0).sqrt();
        let std = [c.fwhmc / k, c.fwhmr / k, c.fwhms / k];
        let p = |v: f64| cfmt::pad(&cfmt::f(v, 2), 5);
        let _ = writeln!(
            r.out,
            "Smoothing ref by FWHM {} {} {}",
            p(c.fwhmc),
            p(c.fwhmr),
            p(c.fwhms)
        );
        smooth_in_place(g, r, &mut ref_vol, std)?;
    }
    if let Some(name) = c.movmask.clone() {
        let _ = writeln!(r.out, "Reading in movmask {name}");
        let Some(m) = read_volume(g, r, &name)? else {
            return Err(Fatal::Exit(1));
        };
        apply_mask(g, r, &mut mov, &m)?;
    }

    if c.cras0 != 0 || c.align_centroids != 0 {
        let centre = |g: &mut Globals, r: &mut Run, v: &Mri, align: bool| -> Result<[f64; 3], Fatal> {
            if align {
                let (_, x) = mri_compute_centroid(g, r.err, v)?;
                Ok(x)
            } else {
                Ok([
                    f64::from(v.vg.width - 1) / 2.0,
                    f64::from(v.vg.height - 1) / 2.0,
                    f64::from(v.vg.depth - 1) / 2.0,
                ])
            }
        };
        let align = c.align_centroids != 0;
        if align {
            let _ = writeln!(r.out, "Setting cras translation parameters to align centroids");
        } else {
            let _ = writeln!(
                r.out,
                "Setting cras translation parameters to align volume centers"
            );
        }
        let xr = centre(g, r, &ref_vol, align)?;
        if align {
            let _ = writeln!(
                r.out,
                "Ref CRS centroid {} {} {}",
                cfmt::g(xr[0], 6),
                cfmt::g(xr[1], 6),
                cfmt::g(xr[2], 6)
            );
        }
        let pr = mid_point(g, r, &ref_vol, xr)?;
        let xm = centre(g, r, &mov, align)?;
        if align {
            let _ = writeln!(
                r.out,
                "Mov CRS centroid {} {} {}",
                cfmt::g(xm[0], 6),
                cfmt::g(xm[1], 6),
                cfmt::g(xm[2], 6)
            );
        }
        let pm = mid_point(g, r, &mov, xm)?;
        for k in 0..3 {
            c.params[k] = f64::from(pm[k] - pr[k]);
        }
    }

    let mut co = Coreg {
        ref_vol,
        mov,
        seplist: c.seplist.clone(),
        sep: 0,
        sepmin: 100,
        sat_pct: c.sat_pct,
        refsat: 0.0,
        movsat: 0.0,
        gvec: Vec::new(),
        fvec: Vec::new(),
        m: None,
        v2v: None,
        reffwhm: [0.0; 3],
        refgstd: [0.0; 3],
        movfwhm: [0.0; 3],
        movgstd: [0.0; 3],
        histfwhm: [7.0, 7.0],
        params: [0.0; 12],
        nparams: c.dof,
        mparams: [0.0; 12],
        h0: vec![0.0; 256 * 256],
        cost: 0.0,
        n_cost_evaluations: 0,
        ftol: c.ftol,
        linmintol: c.linmintol,
        fret: 0.0,
        nitersmax: c.nitersmax,
        niters: 0,
        startmin: false,
        nhits: 0,
        nvoxref,
        pcthits: 0.0,
        do_coord_dither: c.do_coord_dither,
        cdither: Vec::new(),
        do_intensity_dither: c.do_intensity_dither,
        refirfs: None,
        movirfs: None,
        movidither,
        do_smoothing: c.do_smoothing,
        fplogcost: None,
        mov_oob_flag: c.mov_oob_flag,
        optschema: c.optschema,
        debug: c.debug,
        seed: c.seed,
        threads: if r.threads == 0 {
            std::thread::available_parallelism().map_or(1, |n| n.get())
        } else {
            r.threads
        },
        initcost: -1.0,
        mincost: -1.0,
        ppmin: [0.0; 100],
        libm,
    };

    let _ = write!(r.out, "Init matrix params ");
    for (n, v) in c.params.iter().enumerate() {
        let _ = write!(r.out, "{} ", cfmt::pad(&cfmt::f(*v, 4), 7));
        co.mparams[n] = *v;
    }
    let _ = writeln!(r.out);
    {
        let mut t = tranform_affine_params2_matrix(g, r.out, r.err, &c.params, None)?.unwrap_new();
        let tin = t.clone();
        matrix_inverse(g, r.out, r.err, Some(&tin), Some(&mut t))?;
        let mut lta = lta_create(g, r.out, r.err, &co.mov, &co.ref_vol, &t, LINEAR_RAS_TO_RAS)?;
        set_subject(&mut lta, c.subject.as_deref());
        let _ = writeln!(r.out, "Initial Matrix (RAS2RAS)");
        print_matrix(g, r, Some(&t));
        if let Some(name) = c.init_reg_save.clone() {
            let st = write_lta(g, r, &lta, &name)?;
            if st != 0 {
                return Err(Fatal::Exit(st));
            }
            if c.init_reg_save_only {
                let _ = writeln!(r.out, "--init-reg-save-only requested so exiting now");
                return Err(Fatal::Exit(0));
            }
        }
    }
    co.matrix_par2opt_schema(&c.params);
    let _ = write!(r.out, "Initial parameters to be opt ");
    for v in &co.params[..c.dof.clamp(0, 12) as usize] {
        let _ = write!(r.out, "{} ", cfmt::pad(&cfmt::f(*v, 4), 7));
    }
    let _ = writeln!(r.out);

    if co.do_coord_dither != 0 {
        let _ = writeln!(r.out, "Creating random numbers for coordinate dithering");
        let mut crfs = UniformField::new(g, co.seed, 0.0, 1.0);
        let (w, h, d) = (co.ref_vol.vg.width, co.ref_vol.vg.height, co.ref_vol.vg.depth);
        co.cdither = rf_synth_dither(&mut crfs, w, h, d, 3);
    } else {
        let _ = writeln!(r.out, "NOT Creating random numbers for coordinate dithering");
    }
    if co.do_intensity_dither != 0 {
        let _ = writeln!(r.out, "Performing intensity dithering");
        co.refirfs = Some(UniformField::new(g, co.seed, 0.0, 1.0));
        if co.movidither.is_some() {
            let _ = writeln!(
                r.out,
                "Performing intensity dithering on mov with input dither volume"
            );
        } else {
            let _ = writeln!(
                r.out,
                "Performing intensity dithering on mov with computed dither"
            );
            co.movirfs = Some(UniformField::new(g, co.seed, 0.0, 0.0));
        }
    } else {
        let _ = writeln!(r.out, "NOT Performing intensity dithering");
    }
    let _ = r.out.flush();
    for &s in &co.seplist {
        if s < co.sepmin {
            co.sepmin = s;
        }
    }
    coreg_print(r, &co, omp_threads(&c, r));
    preproc(g, r, &mut co)?;
    if let Some(name) = c.movoutfile.clone() {
        let _ = writeln!(r.out, "Saving mov to {name}");
        let env = r.env.clone();
        mri_write(g, r.out, r.err, &env, r.dir, &mut co.mov, &name, None, MGZ_INTENT_UNKNOWN)?;
    }
    if let Some(name) = c.logcost.clone() {
        let path = match r.dir {
            Some(d) => d.join(&name),
            None => std::path::PathBuf::from(&name),
        };
        match std::fs::File::create(&path) {
            Ok(f) => co.fplogcost = Some(f),
            Err(_) => {
                let _ = writeln!(r.out, "ERROR: could not open {name} for writing");
                return Err(Fatal::Exit(1));
            }
        }
    }
    let _ = writeln!(r.out, "Testing if mov and target overlap");
    co.sep = 4;
    co.cost(g, r)?;
    let _ = writeln!(r.out, "Init cost {}", cfmt::pad(&cfmt::f(co.cost, 10), 15));
    let overlap = |r: &mut Run, co: &Coreg| {
        let _ = writeln!(
            r.out,
            "nhits = {} out of {}, Percent Overlap: {}",
            co.nhits,
            co.nvoxref,
            cfmt::pad(&cfmt::f(co.pcthits, 1), 5)
        );
    };
    overlap(r, &co);
    if co.nhits == 0 {
        let _ = writeln!(r.out, "ERROR: mov and ref do not overlap");
        return Err(Fatal::Exit(1));
    }
    let _ = writeln!(r.out, "Initial  RefRAS-to-MovRAS");
    print_matrix(g, r, co.m.as_ref());
    let _ = writeln!(r.out, "Initial  RefVox-to-MovVox");
    print_matrix(g, r, co.v2v.as_ref());
    if let Some(name) = c.do_init_cost_only.clone() {
        let text = format!("{}\n", cfmt::pad(&cfmt::f(co.cost, 10), 20));
        write_unchecked(g, r, &name, &text, "--init-cost-only")?;
        return Err(Fatal::Exit(0));
    }
    for n in 0..co.seplist.len() {
        co.sep = co.seplist[n];
        let _ = writeln!(r.out, "sep = {} -----------------------------------", co.sep);
        if n == 0 && c.do_bf != 0 {
            co.opt_brute_force(g, r, c.bf_lim, 1, c.bf_nsamp)?;
        }
        co.startmin = true;
        co.min_powell(g, r)?;
    }
    co.fplogcost = None;

    let inv = matrix_inverse(g, r.out, r.err, co.v2v.as_ref(), None)?.into_new();
    let Some(inv) = inv else {
        return Err(crash(
            g,
            r,
            &FS_1325,
            "the final voxel-to-voxel matrix is singular (FreeSurfer writes a registration without a matrix and crashes)",
        ));
    };
    let mut lta = lta_create(g, r.out, r.err, &co.mov, &co.ref_vol, &inv, c.ltatype)?;
    set_subject(&mut lta, c.subject.as_deref());
    let outreg = c.outreg.clone().expect("checked");
    if write_lta(g, r, &lta, &outreg)? != 0 {
        return Err(Fatal::Exit(1));
    }
    if let Some(name) = c.regdat.clone() {
        change_type(g, r, &mut lta, REGISTER_DAT)?;
        if write_lta(g, r, &lta, &name)? != 0 {
            return Err(Fatal::Exit(1));
        }
    }
    let _ = writeln!(r.out, "Final  RefRAS-to-MovRAS");
    print_matrix(g, r, co.m.as_ref());
    let _ = writeln!(r.out, "Final  RefVox-to-MovVox");
    print_matrix(g, r, co.v2v.as_ref());
    let _ = write!(r.out, "Final matrix parameters ");
    let mut mpar = [0.0f64; 12];
    co.opt_schema2matrix_par(&mut mpar);
    for v in &mpar {
        let _ = write!(r.out, "{} ", cfmt::pad(&cfmt::f(*v, 4), 7));
    }
    let _ = writeln!(r.out);
    let nopt = co.nparams.clamp(0, 12) as usize;
    let _ = write!(r.out, "Final opt parameters ");
    for v in &co.params[..nopt] {
        let _ = write!(r.out, "{} ", cfmt::pad(&cfmt::f(*v, 4), 7));
    }
    let _ = writeln!(r.out);
    if let Some(name) = c.outparamfile.clone() {
        let mut text = String::new();
        for v in &co.params[..nopt] {
            text.push_str(&format!("{} ", cfmt::pad(&cfmt::f(*v, 4), 7)));
        }
        text.push('\n');
        write_unchecked(g, r, &name, &text, "--params")?;
    }
    if let Some(name) = c.outcostfile.clone() {
        let text = format!("{} \n", cfmt::pad(&cfmt::f(co.cost, 13), 20));
        write_unchecked(g, r, &name, &text, "--final-cost")?;
    }
    overlap(r, &co);
    let secs = start.elapsed().as_secs_f64();
    let _ = writeln!(
        r.out,
        "mri_coreg RunTimeSec {} sec",
        cfmt::pad(&cfmt::f(secs, 1), 4)
    );
    if c.refconf == 0 {
        let _ = writeln!(r.out, "To check run:");
        let _ = write!(
            r.out,
            "   tkregisterfv --mov {} --targ {} --reg {} ",
            mov_name,
            or_null(c.ref_vol.as_deref()),
            outreg
        );
        if let Some(s) = c.subject.as_deref() {
            let _ = write!(r.out, "--s {s} --surfs ");
        }
    }
    let _ = write!(r.out, "\n\n");
    let _ = write!(r.out, "mri_coreg done\n\n");
    Ok(())
}

/// The RAS point of the voxel `x` (`MRIxfmCRS2XYZ(mri, 0) * [x; 1]`, `MatrixMultiply` in
/// `float`).
fn mid_point(g: &mut Globals, r: &mut Run, v: &Mri, x: [f64; 3]) -> Result<[f32; 3], Fatal> {
    let vox2ras = xfm_crs2xyz(g, r.out, r.err, &v.vg, 0, false)?;
    let mut i = matrix_alloc(g, r.out, r.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 matrix");
    i.set_relt(1, 1, x[0] as f32);
    i.set_relt(2, 1, x[1] as f32);
    i.set_relt(3, 1, x[2] as f32);
    i.set_relt(4, 1, 1.0);
    let p = matrix_multiply(g, r.out, r.err, Some(&vox2ras), Some(&i), None)?.unwrap_new();
    Ok([p.relt(1, 1), p.relt(2, 1), p.relt(3, 1)])
}

/// `COREGprint(stdout, coreg)`.
fn coreg_print(r: &mut Run, co: &Coreg, nthreads: usize) {
    let mut line = format!("Separation list ({}): ", co.seplist.len());
    for s in &co.seplist {
        line.push_str(&format!("{} ", cfmt::pad(&s.to_string(), 2)));
    }
    let _ = writeln!(r.out, "{line}  min = {}", co.sepmin);
    let _ = writeln!(r.out, "DoSmoothing {}", co.do_smoothing);
    let _ = writeln!(r.out, "DoCoordDither {}", co.do_coord_dither);
    let _ = writeln!(r.out, "DoIntensityDither {}", co.do_intensity_dither);
    let _ = writeln!(r.out, "nitersmax {}", co.nitersmax);
    let _ = writeln!(r.out, "ftol {}", cfmt::pad(&cfmt::e(co.ftol, 3), 5));
    let _ = writeln!(r.out, "linmintol {}", cfmt::pad(&cfmt::e(co.linmintol, 3), 5));
    let _ = writeln!(r.out, "SatPct {}", cfmt::f(co.sat_pct, 6));
    let _ = writeln!(
        r.out,
        "Hist FWHM {} {}",
        cfmt::f(co.histfwhm[0], 6),
        cfmt::f(co.histfwhm[1], 6)
    );
    let _ = writeln!(r.out, "nthreads {nthreads}");
}

/// `COREGpreproc(coreg)`: each volume's saturation percentile, rescaled to bytes, smoothed for
/// the smallest separation (when an axis's standard deviation is above 0 and smoothing is on),
/// dithered into its byte vector.
fn preproc(g: &mut Globals, r: &mut Run, co: &mut Coreg) -> Result<(), Fatal> {
    let k = co.libm.log(256.0).sqrt();
    co.movsat = f64::from(mri_get_percentile(g, r, &co.mov, co.sat_pct, 0)?);
    let _ = writeln!(r.out, "movsat = {}", cfmt::pad(&cfmt::f(co.movsat, 4), 6));
    let mut tmp = mri_rescale_to_uchar(g, r, &co.mov, co.movsat)?;
    co.movfwhm = coreg_fwhm(&co.mov, f64::from(co.sepmin));
    let mut smooth = false;
    for n in 0..3 {
        co.movgstd[n] = co.movfwhm[n] / k;
        if co.movgstd[n] > 0.0 {
            smooth = true;
        }
    }
    let p = |v: f64| cfmt::pad(&cfmt::f(v, 4), 6);
    let _ = writeln!(
        r.out,
        "mov gstd {} {} {}",
        p(co.movgstd[0]),
        p(co.movgstd[1]),
        p(co.movgstd[2])
    );
    if smooth && co.do_smoothing != 0 {
        let _ = writeln!(r.out, "Smoothing mov");
        smooth_in_place(g, r, &mut tmp, co.movgstd)?;
    } else {
        let _ = writeln!(r.out, "NOT Smoothing mov");
    }
    let dither = co.movidither.take();
    co.fvec = mri_to_uchar_vect(g, r, &tmp, co.movirfs.as_mut(), dither.as_ref())?;
    co.movidither = dither;
    drop(tmp);
    let _ = r.out.flush();

    co.refsat = f64::from(mri_get_percentile(g, r, &co.ref_vol, co.sat_pct, 0)?);
    let _ = writeln!(r.out, "refsat = {}", cfmt::pad(&cfmt::f(co.refsat, 4), 6));
    let mut tmp = mri_rescale_to_uchar(g, r, &co.ref_vol, co.refsat)?;
    co.reffwhm = coreg_fwhm(&co.ref_vol, f64::from(co.sepmin));
    let mut smooth = false;
    for n in 0..3 {
        co.refgstd[n] = co.reffwhm[n] / k;
        if co.refgstd[n] > 0.0 {
            smooth = true;
        }
    }
    let _ = writeln!(
        r.out,
        "ref gstd {} {} {}",
        p(co.refgstd[0]),
        p(co.refgstd[1]),
        p(co.refgstd[2])
    );
    if smooth && co.do_smoothing != 0 {
        let _ = writeln!(r.out, "Smoothing ref");
        smooth_in_place(g, r, &mut tmp, co.refgstd)?;
    } else {
        let _ = writeln!(r.out, "NOT Smoothing ref");
    }
    co.gvec = mri_to_uchar_vect(g, r, &tmp, co.refirfs.as_mut(), None)?;
    let _ = r.out.flush();
    let _ = writeln!(r.out, "COREGpreproc() done");
    Ok(())
}
