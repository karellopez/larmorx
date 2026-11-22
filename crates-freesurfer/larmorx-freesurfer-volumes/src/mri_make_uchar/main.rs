// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_make_uchar`'s program (`mri_make_uchar.cpp` 52-285): the entry point [`run`] (registered
//! as `mri_make_uchar` in the crate's `PROGRAMS`), [`main`] and [`convert_to_uchar`]
//! (`MRIconvertToUchar`).
//!
//! `mri_make_uchar [options] <input volume> <talairach xform> <output volume>`. The version
//! options first (`handleVersionOption`; only version options: status 0). Then the options, while
//! the next argument starts with `-`, by the letter after the dash in any case: `n` (nothing), `u`
//! and `?` (the usage, status 0), `f`, `w`, `r` (`sscanf("%lf")` of the next argument into the
//! first percentile, the white-matter percentile, the ball's radius; a value that does not scan
//! leaves the setting), `h` (the cumulative histogram's text file), `v` (the ball volume); any
//! other `unknown option <arg>` on standard error, status 1. Fewer than two arguments left: the
//! usage, status 1. The settings printed (`%lf`); the input read (`MRIread`; none: `could not read
//! input volume`, `ERROR_NOFILE`), the transform (`LTAread`; none: `could not read <name>`).
//!
//! `MRIconvertToUchar`: a `uchar` volume of the input's header ([`mri_clone_different_type`]);
//! each voxel (x outermost, z innermost) whose scanner RAS through the transform
//! (`LTAtransformPoint`: [`lta_transform_at_point`]'s matrix times the point, in `float`) lies
//! within the radius of the origin (`r < MAX_R`, `r` in `double`) takes the input's value; the
//! ball written when `-v` names a file; its 100-bin histogram, the zero bin cleared, made a PDF in
//! place and a CDF; the CDF written when `-h` names a file; the bins nearest the two percentiles
//! (`i1`, `i2`); the line through `(bin i1, 255 first)` and `(bin i2, 110)` applied to every input
//! voxel, clamped to `[0, 255]` and written through `MRIsetVoxVal`; the mapping line. The output
//! written (`MRIwrite`), `type change took <m> minutes and <s> seconds.` on standard error,
//! status 0.
//!
//! The defects (`larmorx_core::defects`; the mechanisms in
//! `docs/findings/freesurfer-mri-make-uchar.md`):
//! - **FS-1127** (crash): two arguments (no output name): FreeSurfer runs the whole conversion and
//!   crashes writing it (status 139; standard output, a file's buffer, lost). By default the
//!   usage, status 1, before anything is read.
//! - **FS-1128** (crash): `-f`, `-w` or `-r` given last: FreeSurfer scans past the arguments
//!   (status 139). By default the usage, status 1 (as `-h` and `-v` given last).
//! - **FS-1129**: an output (or `-v`'s volume, or `-h`'s histogram) that cannot be written still
//!   ends with status 0. By default status 1 after the writer's messages (the timing line left
//!   out).
//! - **FS-1131**: a mapping that is not finite (both percentiles in bins of one value: an empty
//!   ball, a ball of one intensity; a percentile that is no number): FreeSurfer writes a volume
//!   of zeros (its NaNs converted) with status 0. By default an error, status 1, nothing written.
//! - **FS-1130**: an input that is not coronal prints `MRIvoxelToTalairachVoxel: unsupported slice
//!   direction <d>` (with `errno`'s message first) for every voxel, although the Talairach point
//!   is never used. By default no such message.

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::cscan::sscanf_f64;
use larmorx_core::defects::{FS_1127, FS_1128, FS_1129, FS_1130, FS_1131};
use larmorx_freesurfer_core::diag::diag_init;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::histo::{
    histo_clear_zero_bin, histo_find_bin_with_count, histo_make_cdf, histo_make_pdf_in_place,
    histo_write_txt,
};
use larmorx_freesurfer_core::log::{self, ERROR_NOFILE, Fatal, error_exit};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_multiply};
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::sse::{add64, div, div64, mul64, sub64};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::timer::Timer;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_core::version::handle_version_option;
use larmorx_freesurfer_gca::transform::GcaLtaIo;
use larmorx_freesurfer_io::mriio::{mri_read_volume, mri_write};
use larmorx_freesurfer_mri::lta::Lta;
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_UNKNOWN, MRI_UCHAR, Mri};
use larmorx_freesurfer_mri::vol_geom::voxel_to_world;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::coords::{MRI_CORONAL, get_slice_direction, voxel_to_talairach};
use larmorx_freesurfer_volume::frames::mri_clone_different_type;
use larmorx_freesurfer_volume::ltaapply::lta_transform_at_point;
use larmorx_freesurfer_volume::ltawrite::lta_read;
use larmorx_freesurfer_volume::mrihisto::mri_histogram;

/// The program's name as `argv[0]` gives it (`mri_nu_correct.mni` runs it by name).
pub const NAME: &str = "mri_make_uchar";

/// `DEFAULT_DESIRED_WHITE_MATTER_VALUE` (`mrinorm.h`).
const DESIRED_WHITE_MATTER_VALUE: f64 = 110.0;
/// glibc's buffer of a standard output that is a pipe or a file (`st_blksize`): what a crash
/// loses of what was printed since the last flush.
const STDOUT_BUFFER: usize = 4096;
/// The status of a process ended by `SIGSEGV`, as a shell reports it.
const SEGFAULT: i32 = 139;

/// The settings the options give (the file-scope variables of `mri_make_uchar.cpp`).
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// `FIRST_PERCENTILE` (`-f`).
    pub first_percentile: f64,
    /// `WM_PERCENTILE` (`-w`).
    pub wm_percentile: f64,
    /// `MAX_R` (`-r`).
    pub max_r: f64,
    /// `hcumfile` (`-h`).
    pub hcumfile: Option<String>,
    /// `radfile` (`-v`).
    pub radfile: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            first_percentile: 0.01,
            wm_percentile: 0.90,
            max_r: 50.0,
            hcumfile: None,
            radfile: None,
        }
    }
}

/// The program's streams, environment and working directory. Standard output is kept in `out`
/// until the end, so that a crash in `reproduce` loses what FreeSurfer's would (nothing is
/// flushed before the end but by a full buffer).
pub struct Run<'a, 'b> {
    pub out: &'a mut Vec<u8>,
    pub err: &'a mut dyn Write,
    pub env: &'b Environment,
    pub dir: Option<&'b std::path::Path>,
}

/// `mri_make_uchar`'s entry point: its original arguments (without the program's name), the
/// settings, the two streams; the exit status. The notices of the run follow on standard error
/// (`larmorx: ...`).
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut g = Globals::new(settings.mode);
    // the process's own standard error, as the C library's perror sees it
    g.stderr.read_write = system::stderr_read_write();
    let mut buf: Vec<u8> = Vec::new();
    let status = {
        let mut terr = log::TrackedErr::new(&g, &mut *err);
        let mut r = Run {
            out: &mut buf,
            err: &mut terr,
            env: &settings.env,
            dir: settings.dir.as_deref(),
        };
        match main(&mut g, &mut r, args) {
            Ok(()) => 0,
            Err(f) => f.status(),
        }
    };
    // a crash loses what the C library had not written yet (full buffers are written as they
    // fill)
    let keep = if i32::from(status) == SEGFAULT && g.notices.reproduce() {
        buf.len() / STDOUT_BUFFER * STDOUT_BUFFER
    } else {
        buf.len()
    };
    let _ = out.write_all(&buf[..keep]);
    let _ = out.flush();
    g.notices.print(err);
    status
}

/// `usage_exit(code)`: the usage on standard output (the defaults as the options left them),
/// then `exit(code)`.
fn usage_exit(r: &mut Run, o: &Options, code: i32) -> Fatal {
    let _ = write!(
        r.out,
        "Uses the Tal xform to find a ball of voxels that are mostly brain.\n\
         The top of the intensity histogram in this ball will then be white\n\
         matter, which allows us to center it at the desired value,\n\
         approximately (110).\n\n\
         usage: {NAME} [options] <input volume> <talairach xform> <output volume>\n"
    );
    let _ = writeln!(
        r.out,
        "-f FIRST_PERCENTILE (default {})",
        cfmt::g(o.first_percentile, 6)
    );
    let _ = writeln!(
        r.out,
        "-w WM_PERCENTILE (default {})",
        cfmt::g(o.wm_percentile, 6)
    );
    let _ = writeln!(r.out, "-r MAX_R (default {})", cfmt::g(o.max_r, 6));
    let _ = writeln!(r.out, "-h cumulative histo file");
    let _ = writeln!(
        r.out,
        "-v vradvol : volume with everthing outside of MAX_R set to 0"
    );
    Fatal::Exit(code)
}

/// `get_option(argc, argv)` on `rest` (`argv[1]` and what follows): the arguments it takes after
/// its own.
fn get_option(
    g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    rest: &[String],
) -> Result<usize, Fatal> {
    let arg = &rest[0];
    let letter = arg.as_bytes().get(1).map(u8::to_ascii_uppercase);
    match letter {
        Some(b'N') => Ok(0),
        Some(b'?' | b'U') => Err(usage_exit(r, o, 0)),
        Some(c @ (b'F' | b'W' | b'R')) => {
            let Some(value) = rest.get(1) else {
                // FS-1128: `sscanf(argv[2], ...)` of the NULL after the last argument
                g.notices.add(
                    &FS_1128,
                    format!(
                        "mri_make_uchar: option '{arg}' given last, without its value (FreeSurfer crashes)"
                    ),
                );
                if g.notices.reproduce() {
                    return Err(Fatal::Exit(SEGFAULT));
                }
                return Err(usage_exit(r, o, 1));
            };
            if let Some(v) = sscanf_f64(value.as_bytes()) {
                match c {
                    b'F' => o.first_percentile = v,
                    b'W' => o.wm_percentile = v,
                    _ => o.max_r = v,
                }
            }
            Ok(1)
        }
        Some(b'H') => {
            // the NULL after the last argument is a valid "no file"
            o.hcumfile = rest.get(1).cloned();
            Ok(1)
        }
        Some(b'V') => {
            o.radfile = rest.get(1).cloned();
            Ok(1)
        }
        _ => {
            let _ = writeln!(r.err, "unknown option {arg}");
            Err(Fatal::Exit(1))
        }
    }
}

/// FS-1129: a file asked for that was not written.
fn unwritten(g: &mut Globals, fname: &str) {
    g.notices.add(
        &FS_1129,
        if g.notices.reproduce() {
            format!("mri_make_uchar: '{fname}' was not written; status 0, as in FreeSurfer")
        } else {
            format!("mri_make_uchar: '{fname}' was not written: status 1 (FreeSurfer ends with 0)")
        },
    );
}

/// `MRIwrite(mri, fname)` with FS-1129: whether the file was written (its messages as the writer
/// writes them).
fn write_volume(g: &mut Globals, r: &mut Run, mri: &mut Mri, fname: &str) -> Result<bool, Fatal> {
    let status = mri_write(
        g,
        r.out,
        r.err,
        r.env,
        r.dir,
        mri,
        fname,
        None,
        MGZ_INTENT_UNKNOWN,
    )?;
    if status != 0 {
        unwritten(g, fname);
    }
    Ok(status == 0)
}

/// `main(argc, argv)` on the arguments after the program's name. `Ok` is status 0.
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let (nargs, argv) = handle_version_option(g, r.env, r.out, &argv, NAME);
    if nargs != 0 && argv.len() == 1 {
        return Err(Fatal::Exit(0));
    }
    g.progname = NAME.to_owned();
    diag_init(g, r.env, r.out);
    let start = Timer::new();
    let mut o = Options::default();
    // `argc` and `argv[1]`'s place, as FreeSurfer steps them (an option taking an argument at the
    // end leaves `argc` below 2: no more is read)
    let mut argc = argv.len() as isize;
    let mut at = 1usize;
    while argc > 1 && argv[at].starts_with('-') {
        let nargs = get_option(g, r, &mut o, &argv[at..])?;
        argc -= nargs as isize + 1;
        at += nargs + 1;
    }
    if argc < 3 {
        return Err(usage_exit(r, &o, 1));
    }
    let out_fname = if argc < 4 {
        // FS-1127: `argv[3]` is the NULL after the last argument
        g.notices.add(
            &FS_1127,
            if g.notices.reproduce() {
                "mri_make_uchar: no output volume named: FreeSurfer converts, then crashes writing it"
            } else {
                "mri_make_uchar: no output volume named: the usage, status 1 (FreeSurfer converts, then crashes writing it)"
            },
        );
        if g.notices.fix() {
            return Err(usage_exit(r, &o, 1));
        }
        None
    } else {
        Some(argv[at + 2].clone())
    };
    let _ = writeln!(r.out, "FIRST_PERCENTILE {}", cfmt::f(o.first_percentile, 6));
    let _ = writeln!(r.out, "WM_PERCENTILE    {}", cfmt::f(o.wm_percentile, 6));
    let _ = writeln!(r.out, "MAX_R {}", cfmt::f(o.max_r, 6));
    let progname = g.progname.clone();
    let Some(mut mri_in) = mri_read_volume(g, r.out, r.err, r.env, r.dir, &argv[at])? else {
        error_exit(
            g,
            r.err,
            ERROR_NOFILE,
            &format!("{progname}: could not read input volume {}", argv[at]),
        )?;
        unreachable!("ErrorExit returns only for code 0");
    };
    let mut io = GcaLtaIo { env: r.env };
    let Some(tal_xform) = lta_read(g, r.out, r.err, r.dir, &argv[at + 1], &mut io)? else {
        error_exit(
            g,
            r.err,
            ERROR_NOFILE,
            &format!("{progname}: could not read {}", argv[at + 1]),
        )?;
        unreachable!("ErrorExit returns only for code 0");
    };
    let (mut mri_out, written) = convert_to_uchar(g, r, &o, &mut mri_in, &tal_xform)?;
    drop(mri_in);
    let Some(out_fname) = out_fname else {
        // `MRIwrite(mri_out, NULL)`: the name's copy reads address 0
        return Err(Fatal::Exit(SEGFAULT));
    };
    let ok = write_volume(g, r, &mut mri_out, &out_fname)?;
    if g.notices.fix() && !(ok && written) {
        return Err(Fatal::Exit(1));
    }
    let msec = start.milliseconds();
    let seconds = nint(g, f64::from(div(msec as f32, 1000.0)));
    let (minutes, seconds) = (seconds / 60, seconds % 60);
    let _ = writeln!(
        r.err,
        "type change took {minutes} minutes and {seconds} seconds."
    );
    Ok(())
}

/// `MRIconvertToUchar(mri_in, tal_xform, NULL)`: the `uchar` volume (see the module's text) and
/// whether `-v`'s volume and `-h`'s histogram, when asked for, were written.
pub fn convert_to_uchar(
    g: &mut Globals,
    r: &mut Run,
    o: &Options,
    mri_in: &mut Mri,
    tal_xform: &Lta,
) -> Result<(Mri, bool), Fatal> {
    let mut v_x = Matrix::real(4, 1);
    let mut v_y = Matrix::real(4, 1);
    let mut mri_out = mri_clone_different_type(g, r.out, r.err, mri_in, MRI_UCHAR)?;
    v_x.set_relt(4, 1, 1.0);
    v_y.set_relt(4, 1, 1.0);
    // `LTAtransformPoint`'s matrix (a static of FreeSurfer's, cleared at each call)
    let mut m_l = Matrix::real(4, 4);
    let coronal = get_slice_direction(&mri_in.vg) == MRI_CORONAL;
    if !coronal {
        g.notices.add(
            &FS_1130,
            if g.notices.reproduce() {
                "mri_make_uchar: an input that is not coronal: MRIvoxelToTalairach's error for every voxel, as in FreeSurfer (its point is never used)"
            } else {
                "mri_make_uchar: an input that is not coronal: MRIvoxelToTalairach's error for every voxel left out (FreeSurfer prints it; its point is never used)"
            },
        );
    }
    let (w, h, d) = (mri_in.vg.width, mri_in.vg.height, mri_in.vg.depth);
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                // `MRIvoxelToTalairach`'s point is never used: only its error (not coronal) and
                // the geometry's cache, which `MRIvoxelToWorld` fills alike, are seen
                let cached = mri_in.vg.i_to_r.is_some() && mri_in.vg.r_to_i.is_some();
                if (!coronal && g.notices.reproduce()) || (coronal && !cached) {
                    voxel_to_talairach(
                        g,
                        r.out,
                        r.err,
                        mri_in,
                        f64::from(x),
                        f64::from(y),
                        f64::from(z),
                    )?;
                }
                let (xw, yw, zw) = voxel_to_world(
                    g,
                    r.out,
                    r.err,
                    &mut mri_in.vg,
                    f64::from(x),
                    f64::from(y),
                    f64::from(z),
                )?;
                v_x.set_relt(1, 1, xw as f32);
                v_x.set_relt(2, 1, yw as f32);
                v_x.set_relt(3, 1, zw as f32);
                // `LTAtransformPoint(tal_xform, v_X, v_Y)`
                let (px, py, pz) = (v_x.relt(1, 1), v_x.relt(2, 1), v_x.relt(3, 1));
                lta_transform_at_point(g, r.out, r.err, tal_xform, px, py, pz, Some(&mut m_l))?;
                matrix_multiply(g, r.out, r.err, Some(&m_l), Some(&v_x), Some(&mut v_y))?;
                let xt = f64::from(v_y.relt(1, 1));
                let yt = f64::from(v_y.relt(2, 1));
                let zt = f64::from(v_y.relt(3, 1));
                let rad = (xt * xt + yt * yt + zt * zt).sqrt();
                if rad < o.max_r {
                    let v = mri_get_vox_val(g, r.err, mri_in, x, y, z, 0, 0)?;
                    mri_set_vox_val(g, r.err, &mut mri_out, x, y, z, 0, v, 0)?;
                }
            }
        }
    }
    let mut written = true;
    if let Some(radfile) = &o.radfile {
        written = write_volume(g, r, &mut mri_out, radfile)?;
    }
    // only in a radius around the centre of the brain
    let mut hist = mri_histogram(g, r.err, &mri_out, 100)?;
    histo_clear_zero_bin(g, &mut hist);
    histo_make_pdf_in_place(&mut hist);
    let hcum = histo_make_cdf(g, r.err, &hist, None)?.unwrap_new();
    if let Some(hcumfile) = &o.hcumfile
        && histo_write_txt(&hcum, r.out, r.dir, hcumfile) != 0
    {
        unwritten(g, hcumfile);
        written = false;
    }
    let i1 = histo_find_bin_with_count(g, &hcum, o.first_percentile as f32);
    let i2 = histo_find_bin_with_count(g, &hcum, o.wm_percentile as f32);
    let _ = writeln!(r.out, "i1 = {i1}, i2 = {i2}");
    let x1 = f64::from(hist.bins[i1 as usize]);
    let x2 = f64::from(hist.bins[i2 as usize]);
    // the official build's operand orders (its NaNs: `255 y1`, `x2 m`)
    let y1 = mul64(255.0, o.first_percentile);
    // the ball around Talairach (0, 0, 0) holds almost only brain: its intensity range is mapped
    // around the middle of the uchar range, so that white matter ends near 110
    let y2 = DESIRED_WHITE_MATTER_VALUE;
    let m = div64(sub64(y2, y1), sub64(x2, x1));
    let b = sub64(y2, mul64(x2, m));
    if !(m.is_finite() && b.is_finite()) {
        // FS-1131: both percentiles in bins of one value (an empty ball, one intensity), or a
        // percentile that is no number: no line maps the intensities
        g.notices.add(
            &FS_1131,
            if g.notices.reproduce() {
                "mri_make_uchar: the ball's histogram gives no finite mapping: a volume of zeros, as in FreeSurfer"
            } else {
                "mri_make_uchar: the ball's histogram gives no finite mapping: an error, status 1 (FreeSurfer writes a volume of zeros)"
            },
        );
        if g.notices.fix() {
            let _ = writeln!(
                r.err,
                "{NAME}: no finite mapping to uchar (bins {i1} and {i2}: {} and {}; slope {}): nothing written",
                cfmt::g(x1, 6),
                cfmt::g(x2, 6),
                cfmt::g(m, 6)
            );
            return Err(Fatal::Exit(1));
        }
    }
    let (mut nzero, mut nsat) = (0i64, 0i64);
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                let mut val = f64::from(mri_get_vox_val(g, r.err, mri_in, x, y, z, 0, 0)?);
                val = add64(mul64(val, m), b);
                if val < 0.0 {
                    val = 0.0;
                    nzero += 1;
                } else if val > 255.0 {
                    val = 255.0;
                    nsat += 1;
                }
                mri_set_vox_val(g, r.err, &mut mri_out, x, y, z, 0, val as f32, 0)?;
            }
        }
    }
    let f2 = |v: f64| cfmt::pad(&cfmt::f(v, 0), 2);
    let _ = writeln!(
        r.out,
        "#mri_make_uchar# mapping {} {} to {} {}  :  b {} m {} : thresh {} maxsat {} : nzero {nzero} nsat {nsat}",
        f2(x1),
        f2(x2),
        f2(y1),
        f2(y2),
        cfmt::g(b, 6),
        cfmt::g(m, 6),
        cfmt::g(div64(-b, m), 6),
        cfmt::g(div64(sub64(255.0, b), m), 6),
    );
    Ok((mri_out, written))
}
