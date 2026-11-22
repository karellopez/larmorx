// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_info`'s `main` (`mri_info.cpp` 121-174), `PrettyMatrixPrint` (603), `do_file` (633),
//! `__printTagsToFile` (1334) and `__reportWarp` (1418): the program's entry point [`run`]
//! (registered as `mri_info` in the crate's `PROGRAMS`) and [`main`].
//!
//! The order of `main`: the version options (`handleVersionOption`; with only version options,
//! status 0); no argument left: the usage, status 1; the options ([`super::options`]) and their
//! check; `--o`'s file opened (`ERROR: could not open <file>`, status 1); each input reported by
//! `do_file`; status 0. `do_file`: the name's extension (none: `<progname>: could not parse
//! extension from <name>`, status 253); its type found from the name (`mri_identify`); unless it
//! is a morph (`.m3z`, `.m3d`), its header read (the whole volume for `--voxel`, `--entropy`,
//! `--stats` and `--voxvolsum`; a volume that cannot be read ends the program with status 1);
//! a warp field (`MGZ_INTENT_WARPMAP`) reported as such; a morph's report; then the first print
//! flag in `do_file`'s order decides what is printed (`--tr`, `--te`, `--fa`, `--fad` and `--ti`
//! together), and with none the full report. Most lines go to standard output; those of the print
//! flags, the report's first line and a morph's report to `--o`'s file when it is given.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-info.md`):
//! - **FS-1147**: `-it`/`--in_type` has no effect (by default the volume is read as that type).
//! - **FS-1148** (crash): a morph whose extension mixes cases (`.M3z`), which the type test calls
//!   a morph and the report does not (by default reported as a morph).
//! - **FS-1150** (crash): `--otags` as the last argument (no file name).
//! - **FS-1151**: `--otags` with a file that cannot be created: nothing written, no message (by
//!   default an error, status 1).
//! - **FS-1152** (crash): `--ras2vox` and `--ras2vox-tkr` of a matrix that cannot be inverted.
//! - **FS-1153**: `--stats`' mean divides by the voxel count in `int`, which wraps past
//!   2,147,483,647 voxels (by default the count in 64 bits).

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_1147, FS_1148, FS_1151, FS_1152, FS_1153};
use larmorx_core::x87::F80;
use larmorx_freesurfer_core::diag::{DIAG_INFO, diag_init};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{matrix_determinant, matrix_inverse, matrix_print_fmt};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Settings;
use larmorx_freesurfer_core::sse::mul;
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::utils::{nint, resolve};
use larmorx_freesurfer_core::version::handle_version_option;
use larmorx_freesurfer_gca::gcam::{GCAM_RAS, GCAM_VOX, cma_label_to_name, gcam_read};
use larmorx_freesurfer_io::colortab::ctab_print_ascii;
use larmorx_freesurfer_io::fio::{fio_extension, fio_is_directory};
use larmorx_freesurfer_io::identify::{MGH_MORPH, MRI_VOLUME_TYPE_UNKNOWN, mri_identify, type_to_string};
use larmorx_freesurfer_io::mriio::{mri_read_header, mri_read_volume};
use larmorx_freesurfer_io::mriio_write::mri_tag_write;
use larmorx_freesurfer_io::warpfield::{
    WARPFIELD_DTFMT_ABS_CRS, WARPFIELD_DTFMT_ABS_RAS, WARPFIELD_DTFMT_DISP_CRS,
    WARPFIELD_DTFMT_DISP_RAS,
};
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, MGZ_INTENT_WARPMAP, MRI_BITMAP, MRI_FLOAT, MRI_FLOAT_COMPLEX, MRI_INT,
    MRI_LONG, MRI_RGB, MRI_SHORT, MRI_TENSOR, MRI_UCHAR, MRI_USHRT, Mri, WARPFIELD_DTFMT_UNKNOWN,
    intent_name,
};
use larmorx_freesurfer_mri::tkreg::{xfm_crs2xyz_fsl, xfm_crs2xyz_tkreg};
use larmorx_freesurfer_mri::vol_geom::{VolGeom, extract_i_to_r, extract_r_to_i, vgprint};
use larmorx_freesurfer_mri::voxel::{MRI_COMPLEX_IMAG, MRI_COMPLEX_REAL, mri_get_vox_val};
use larmorx_freesurfer_volume::conformed::mri_conformed;
use larmorx_freesurfer_volume::coords::{
    dircos_to_orientation_string, slice_direction_name, surface_ras_from_ras,
};
use larmorx_freesurfer_volume::mrihisto::mri_histogram;
use larmorx_io::znz::ZnzFile;

use super::options::{Options, Run, check_options, no_argument, parse_commandline, usage_exit};

/// The program's name as `argv[0]` gives it (the scripts run it by name).
pub const NAME: &str = "mri_info";

/// `mri_info`'s entry point: its original arguments (without the program's name), the settings,
/// the two streams; the exit status. The notices of the run follow on standard error
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

/// `main(argc, argv)` on the arguments after the program's name. `Ok` is status 0.
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let (nargs, left) = handle_version_option(g, r.env, r.out, &argv, NAME);
    if nargs != 0 && left.len() == 1 {
        return Err(Fatal::Exit(0));
    }
    g.progname = NAME.to_owned();
    diag_init(g, r.env, r.out);
    let args = &left[1..];
    if args.is_empty() {
        return Err(usage_exit(r.out, NAME));
    }
    let o = parse_commandline(g, r, args, NAME)?;
    check_options(r, &o)?;
    let mut fp = if o.flags.to_file {
        let Some(name) = o.outfile.as_deref() else {
            // `fopen(NULL, "w")` fails in the C library
            let _ = writeln!(r.out, "ERROR: could not open (null)");
            return Err(Fatal::Exit(1));
        };
        match std::fs::File::create(resolve(r.dir, name)) {
            Ok(f) => Fp::File(std::io::BufWriter::new(f)),
            Err(e) => {
                g.errno = log::errno_of(&e).unwrap_or(2);
                let _ = writeln!(r.out, "ERROR: could not open {name}");
                return Err(Fatal::Exit(1));
            }
        }
    } else {
        Fp::Stdout
    };
    let result = (|| {
        for (index, name) in o.inputs.iter().enumerate() {
            if o.debug {
                let _ = writeln!(r.out, "{index} {name} ----- ");
            }
            do_file(g, r, &o, &mut fp, name)?;
        }
        Ok(())
    })();
    // `exit()` flushes the file in every case; `fclose` at the end
    if let Fp::File(f) = &mut fp {
        let _ = f.flush();
    }
    result
}

/// `fpout`: standard output, or `--o`'s file.
enum Fp {
    Stdout,
    File(std::io::BufWriter<std::fs::File>),
}

/// `fpout` as a writer: `--o`'s file, else standard output.
fn fpw<'a>(fp: &'a mut Fp, out: &'a mut dyn Write) -> &'a mut dyn Write {
    match fp {
        Fp::Stdout => out,
        Fp::File(f) => f,
    }
}

/// A C string's bytes: up to its first NUL.
fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// `%g` of a `float`.
fn gf(v: f32) -> String {
    cfmt::g(f64::from(v), 6)
}

/// `%g` of a `double`.
fn gd(v: f64) -> String {
    cfmt::g(v, 6)
}

/// `%<w>.<p>f` of a `double`.
fn fw(v: f64, w: usize, p: usize) -> String {
    cfmt::pad(&cfmt::f(v, p), w)
}

/// `DEGREES(rad)`: `(360.0 * rad) / (2.0 * M_PI)` in `double`.
fn degrees(rad: f64) -> f64 {
    (360.0 * rad) / (2.0 * std::f64::consts::PI)
}

/// The orientation string (`MRIdircosToOrientationString`).
fn orientation(vg: &VolGeom) -> String {
    let (_, o) = dircos_to_orientation_string(vg);
    String::from_utf8_lossy(cstr(&o)).into_owned()
}

/// Whether the name's extension is a morph's as `do_file` tests it: `m3d`, `m3z`, `M3D` or `M3Z`
/// anywhere in it.
fn morph_extension(ext: &str) -> bool {
    ["m3d", "m3z", "M3D", "M3Z"].iter().any(|m| ext.contains(m))
}

/// The voxel type's name in `--type` and `--dump` (`--dump` has no `float complex`); `None`: no
/// line.
fn precision_name(t: i32, complex: bool) -> Option<&'static str> {
    Some(match t {
        MRI_UCHAR => "uchar",
        MRI_FLOAT => "float",
        MRI_LONG => "long",
        MRI_SHORT => "short",
        MRI_USHRT => "ushrt",
        MRI_INT => "int",
        MRI_TENSOR => "tensor",
        MRI_FLOAT_COMPLEX if complex => "float complex",
        _ => return None,
    })
}

/// A 4 x 4 matrix as `%10.5f ` numbers, a row a line.
fn print_rows(fp: &mut dyn Write, m: &Matrix) {
    for row in 1..=4 {
        for c in 1..=4 {
            let _ = write!(fp, "{} ", fw(f64::from(m.relt(row, c)), 10, 5));
        }
        let _ = writeln!(fp);
    }
}

/// `PrettyMatrixPrint(mat, precision)`: the four rows as `%8.<p>f %8.<p>f %8.<p>f %10.<p>f`
/// after 14 blanks; a missing matrix, one that is not real or not 4 x 4 prints FreeSurfer's error
/// on standard error.
fn pretty_matrix_print(g: &mut Globals, r: &mut Run, mat: Option<&Matrix>, precision: usize) {
    let Some(mat) = mat else {
        log::error_printf(g, r.err, ERROR_BADPARM, "mat = NULL!");
        return;
    };
    if mat.mtype != MATRIX_REAL {
        log::error_printf(g, r.err, ERROR_BADPARM, "mat is not MATRIX_REAL type");
        return;
    }
    if mat.rows != 4 || mat.cols != 4 {
        log::error_printf(g, r.err, ERROR_BADPARM, "mat is not of 4 x 4");
        return;
    }
    let e = |c: i32, row: i32| f64::from(mat.relt(row, c));
    for row in 1..5 {
        let _ = writeln!(
            r.out,
            "              {} {} {} {}",
            fw(e(1, row), 8, precision),
            fw(e(2, row), 8, precision),
            fw(e(3, row), 8, precision),
            fw(e(4, row), 10, precision)
        );
    }
}

/// The type names of the report (`--dump`'s and `__reportWarp`'s lists differ: `complex` adds
/// `FLOAT COMPLEX` and `RGB`).
fn type_name(t: i32, complex: bool) -> &'static str {
    match t {
        MRI_UCHAR => "UCHAR",
        MRI_SHORT => "SHORT",
        MRI_USHRT => "USHRT",
        MRI_INT => "INT",
        MRI_LONG => "LONG",
        MRI_BITMAP => "BITMAP",
        MRI_TENSOR => "TENSOR",
        MRI_FLOAT => "FLOAT",
        MRI_FLOAT_COMPLEX if complex => "FLOAT COMPLEX",
        MRI_RGB if complex => "RGB",
        _ => "UNKNOWN",
    }
}

/// `MatrixFree`d matrices aside: the voxel-to-RAS matrix (`MRIgetVoxelToRasXform`).
fn vox2ras(g: &mut Globals, r: &mut Run, mri: &Mri) -> Result<Matrix, Fatal> {
    extract_i_to_r(g, r.out, r.err, &mri.vg)
}

/// FS-1152: the inverse FreeSurfer prints from a null pointer (a crash).
fn no_inverse(g: &mut Globals, r: &mut Run, what: &str, fname: &str) -> Fatal {
    g.notices.add(
        &FS_1152,
        format!(
            "mri_info {what} {fname}: a matrix that cannot be inverted, which FreeSurfer then prints (a crash)"
        ),
    );
    let _ = writeln!(
        r.err,
        "mri_info: {what}: the matrix of {fname} cannot be inverted (FreeSurfer crashes here)"
    );
    Fatal::Exit(1)
}

/// FS-1148 with `reproduce`: what FreeSurfer prints before it reads the volume it never read.
fn no_volume(g: &mut Globals, r: &mut Run, o: &Options, fp: &mut Fp, fname: &str) -> Fatal {
    g.notices.add(
        &FS_1148,
        format!(
            "mri_info {fname}: a morph by its name whose extension mixes cases, which FreeSurfer reports from a volume it never read (a crash)"
        ),
    );
    let f = &o.flags;
    let before = o.zero_cras
        || f.dump
        || f.tr
        || f.te
        || f.flip_angle
        || f.flip_angle_deg
        || f.is_1mm_iso
        || f.conformed
        || f.conformed_to_min
        || f.ptype
        || f.ti
        || f.pedir
        || f.cres
        || f.rres
        || f.sres
        || f.voxvol
        || f.ncols
        || f.nrows
        || f.nslices
        || f.dim
        || f.res
        || f.min_res
        || f.dof
        || f.nframes
        || f.mid_frame
        || f.col_dc
        || f.row_dc
        || f.slice_dc
        || f.cras
        || f.center
        || f.p0
        || f.det
        || f.vox2ras
        || f.ras_good
        || f.ras2vox
        || f.vox2ras_tkr
        || f.ras2vox_tkr
        || f.tkr2scanner
        || f.scanner2tkr
        || f.vox2ras_fsl
        || f.orientation
        || f.slice_direction
        || f.autoalign
        || f.orig_ras2vox
        || f.ctab
        || f.voxvolsum
        || f.entropy
        || f.stats
        || f.voxel
        || f.cmds;
    if !before {
        if f.tags_to_file {
            let _ = r
                .out
                .write_all(b"\n==============================================================\n");
            let shown = o.outtagfile.as_deref().unwrap_or("(null)");
            let _ = writeln!(r.out, "save TAG data as {shown}");
            if let Some(name) = o.outtagfile.as_deref() {
                let z = name.as_bytes().last() == Some(&b'z');
                let _ = writeln!(r.out, "use_compression = {}", i32::from(z));
                let path = resolve(r.dir, name);
                drop(ZnzFile::open(path.as_os_str().as_encoded_bytes(), b"w", z));
            }
        } else {
            let _ = writeln!(fpw(fp, r.out), "Volume information for {fname}");
            let _ = writeln!(r.out, "          type: {}", type_to_string(MGH_MORPH));
        }
    }
    let _ = writeln!(
        r.err,
        "mri_info: {fname}: a morph by its name that the report reads as a volume (FreeSurfer crashes here)"
    );
    Fatal::Exit(1)
}

/// `do_file(fname)`.
#[allow(clippy::cognitive_complexity)]
fn do_file(
    g: &mut Globals,
    r: &mut Run,
    o: &Options,
    fp: &mut Fp,
    fname: &str,
) -> Result<(), Fatal> {
    let f = &o.flags;
    let Some(ext) = fio_extension(Some(fname)) else {
        log::error_exit(
            g,
            r.err,
            ERROR_UNSUPPORTED,
            &format!("{}: could not parse extension from {fname}", g.progname),
        )?;
        return Ok(());
    };
    let mut intype = mri_identify(g, r.dir, r.env, r.out, r.err, fname)?;
    // FS-1147: `-it`'s type, which FreeSurfer replaces by the one found from the name
    let given = (o.intype != MRI_VOLUME_TYPE_UNKNOWN && o.intype != intype).then_some(o.intype);
    if let Some(t) = given {
        g.notices.add(
            &FS_1147,
            format!(
                "mri_info -it {}: FreeSurfer reads {fname} as the type its name gives ({}), not as the one given ({})",
                o.intypestr.as_deref().unwrap_or(""),
                type_to_string(intype),
                type_to_string(t)
            ),
        );
        if !g.notices.reproduce() {
            intype = t;
        }
    }
    let full = f.voxel || f.entropy || f.stats || f.voxvolsum;
    let mut mri: Option<Mri> = None;
    let mut mriintent = MGZ_INTENT_UNKNOWN;
    if intype != MGH_MORPH {
        let typed = given.is_some() && !g.notices.reproduce();
        let read = if !full {
            mri_read_header(g, r.out, r.err, r.env, r.dir, fname, intype)?
        } else if typed {
            // FS-1147's correction: `MRIread` of `<name>@<type>`, its way of naming the type
            let named = format!("{fname}@{}", o.intypestr.as_deref().unwrap_or(""));
            mri_read_volume(g, r.out, r.err, r.env, r.dir, &named)?
        } else {
            mri_read_volume(g, r.out, r.err, r.env, r.dir, fname)?
        };
        let Some(m) = read else {
            return Err(Fatal::Exit(1));
        };
        mriintent = m.intent;
        mri = Some(m);
    }
    if mriintent == MGZ_INTENT_WARPMAP {
        if let Some(m) = &mri {
            report_warp(g, r, m, fname)?;
        }
        return Ok(());
    }
    let named_morph = morph_extension(&ext);
    if !named_morph && intype == MGH_MORPH && !g.notices.reproduce() {
        // FS-1148: the type test's morph reported as one
        g.notices.add(
            &FS_1148,
            format!(
                "mri_info {fname}: a morph by its name whose extension mixes cases, which FreeSurfer reports from a volume it never read (a crash)"
            ),
        );
    }
    if named_morph || (intype == MGH_MORPH && !g.notices.reproduce()) {
        return report_morph(g, r, fp, fname);
    }
    if f.format {
        let t = mri_identify(g, r.dir, r.env, r.out, r.err, fname)?;
        let _ = writeln!(fpw(fp, r.out), "{}", type_to_string(t));
        return Ok(());
    }
    let Some(mut mri) = mri else {
        return Err(no_volume(g, r, o, fp, fname));
    };
    if o.zero_cras {
        mri.vg.c_r = 0.0;
        mri.vg.c_a = 0.0;
        mri.vg.c_s = 0.0;
    }
    let mri = &mri;
    let vg = &mri.vg;
    if f.dump {
        let mut lines = format!("FA {}\n", gd(degrees(mri.flip_angle)));
        lines += &format!("TR {}\n", gf(mri.tr));
        lines += &format!("TE {}\n", gf(mri.te));
        lines += &format!("TI {}\n", gf(mri.ti));
        lines += &format!(
            "Dim {} {} {} {}\n",
            vg.width, vg.height, vg.depth, mri.nframes
        );
        let p = |v: f32| fw(f64::from(v), 5, 3);
        lines += &format!(
            "Res {} {} {} {}\n",
            p(vg.xsize),
            p(vg.ysize),
            p(vg.zsize),
            p(mri.tr)
        );
        let _ = fpw(fp, r.out).write_all(lines.as_bytes());
        let m = vox2ras(g, r, mri)?;
        let det = matrix_determinant(g, r.err, &m);
        let mut lines = format!("Det {}\n", gf(det));
        lines += &format!("Orientation {}\n", orientation(vg));
        lines += &format!("SliceDir {}\n", slice_direction_name(vg));
        lines += "Precision ";
        if let Some(n) = precision_name(mri.mtype, false) {
            lines += n;
            lines += "\n";
        }
        let _ = fpw(fp, r.out).write_all(lines.as_bytes());
        return Ok(());
    }
    if f.tr {
        let _ = writeln!(fpw(fp, r.out), "{}", gf(mri.tr));
    }
    if f.te {
        let _ = writeln!(fpw(fp, r.out), "{}", gf(mri.te));
    }
    if f.flip_angle {
        let _ = writeln!(fpw(fp, r.out), "{}", gd(mri.flip_angle));
    }
    if f.flip_angle_deg {
        let _ = writeln!(fpw(fp, r.out), "{}", gd(degrees(mri.flip_angle)));
    }
    if f.is_1mm_iso {
        let near = |s: f32| (f64::from(s) - 1.0).abs() < 0.001;
        let yes = near(vg.xsize) && near(vg.ysize) && near(vg.zsize);
        let _ = writeln!(fpw(fp, r.out), "{}", if yes { "yes" } else { "no" });
        return Ok(());
    }
    if f.conformed {
        let yes = mri_conformed(g, vg) != 0;
        let _ = writeln!(fpw(fp, r.out), "{}", if yes { "yes" } else { "no" });
        return Ok(());
    }
    if f.conformed_to_min {
        let yes = vg.xsize == vg.ysize
            && vg.xsize == vg.zsize
            && vg.width == vg.height
            && vg.width == vg.depth;
        let _ = writeln!(fpw(fp, r.out), "{}", if yes { "yes" } else { "no" });
        return Ok(());
    }
    if f.ptype {
        if let Some(n) = precision_name(mri.mtype, true) {
            let _ = writeln!(fpw(fp, r.out), "{n}");
        }
        return Ok(());
    }
    if f.ti {
        let _ = writeln!(fpw(fp, r.out), "{}", gf(mri.ti));
    }
    if f.tr || f.te || f.flip_angle || f.flip_angle_deg || f.ptype || f.ti {
        return Ok(());
    }
    let w = fpw(fp, r.out);
    if f.pedir {
        match &mri.pedir {
            Some(p) => {
                let _ = w.write_all(cstr(p));
                let _ = writeln!(w);
            }
            None => {
                let _ = writeln!(w, "UNKNOWN");
            }
        }
        return Ok(());
    }
    if f.cres {
        let _ = writeln!(w, "{}", gf(vg.xsize));
        return Ok(());
    }
    if f.rres {
        let _ = writeln!(w, "{}", gf(vg.ysize));
        return Ok(());
    }
    if f.sres {
        let _ = writeln!(w, "{}", gf(vg.zsize));
        return Ok(());
    }
    if f.voxvol {
        let _ = writeln!(w, "{}", gf(mul(mul(vg.xsize, vg.ysize), vg.zsize)));
        return Ok(());
    }
    if f.ncols {
        let _ = writeln!(w, "{}", vg.width);
        return Ok(());
    }
    if f.nrows {
        let _ = writeln!(w, "{}", vg.height);
        return Ok(());
    }
    if f.nslices {
        let _ = writeln!(w, "{}", vg.depth);
        return Ok(());
    }
    if f.dim {
        let _ = writeln!(
            w,
            "{:<3} {:<3} {:<3} {:<3}",
            vg.width, vg.height, vg.depth, mri.nframes
        );
        return Ok(());
    }
    let r97 = |v: f32| fw(f64::from(v), 9, 7);
    if f.res {
        let _ = writeln!(
            w,
            "{} {} {} {}",
            r97(vg.xsize),
            r97(vg.ysize),
            r97(vg.zsize),
            r97(mri.tr)
        );
        return Ok(());
    }
    if f.min_res {
        let yz = if vg.ysize < vg.zsize {
            vg.ysize
        } else {
            vg.zsize
        };
        let m = if vg.xsize < yz { vg.xsize } else { yz };
        let _ = writeln!(w, "{}", r97(m));
        return Ok(());
    }
    if f.dof {
        let _ = writeln!(w, "{}", mri.dof);
        return Ok(());
    }
    if f.nframes {
        let _ = writeln!(w, "{}", mri.nframes);
        return Ok(());
    }
    if f.mid_frame {
        let v = nint(g, f64::from(mri.nframes / 2));
        let _ = writeln!(fpw(fp, r.out), "{v}");
        return Ok(());
    }
    let w = fpw(fp, r.out);
    let g3 = |a: f32, b: f32, c: f32| format!("{} {} {}", gf(a), gf(b), gf(c));
    if f.col_dc {
        let _ = writeln!(w, "{}", g3(vg.x_r, vg.x_a, vg.x_s));
        return Ok(());
    }
    if f.row_dc {
        let _ = writeln!(w, "{}", g3(vg.y_r, vg.y_a, vg.y_s));
        return Ok(());
    }
    if f.slice_dc {
        let _ = writeln!(w, "{}", g3(vg.z_r, vg.z_a, vg.z_s));
        return Ok(());
    }
    if f.cras {
        let _ = writeln!(w, "{}", g3(vg.c_r, vg.c_a, vg.c_s));
        return Ok(());
    }
    if f.center {
        let m = vox2ras(g, r, mri)?;
        let mut p = matrix_alloc(g, r.out, r.err, 4, 1, MATRIX_REAL)?.expect("a real vector");
        p.set_relt(1, 1, ((f64::from(vg.width) - 1.0) / 2.0) as f32);
        p.set_relt(2, 1, ((f64::from(vg.height) - 1.0) / 2.0) as f32);
        p.set_relt(3, 1, ((f64::from(vg.depth) - 1.0) / 2.0) as f32);
        p.set_relt(4, 1, 1.0);
        let m2 = matrix_multiply(g, r.out, r.err, Some(&m), Some(&p), None)?.into_new();
        let Some(m2) = m2 else {
            return Err(no_inverse(g, r, "--center", fname));
        };
        let _ = writeln!(
            fpw(fp, r.out),
            "{}",
            g3(m2.relt(1, 1), m2.relt(2, 1), m2.relt(3, 1))
        );
        return Ok(());
    }
    if f.p0 {
        let m = vox2ras(g, r, mri)?;
        let _ = writeln!(
            fpw(fp, r.out),
            "{}",
            g3(m.relt(1, 4), m.relt(2, 4), m.relt(3, 4))
        );
        return Ok(());
    }
    if f.det {
        let m = vox2ras(g, r, mri)?;
        let det = matrix_determinant(g, r.err, &m);
        let _ = writeln!(fpw(fp, r.out), "{}", gf(det));
        return Ok(());
    }
    if f.vox2ras {
        let m = vox2ras(g, r, mri)?;
        print_rows(fpw(fp, r.out), &m);
        return Ok(());
    }
    if f.ras_good {
        let _ = writeln!(fpw(fp, r.out), "{}", vg.ras_good_flag);
        return Ok(());
    }
    if f.ras2vox {
        let m = vox2ras(g, r, mri)?;
        let Some(minv) = matrix_inverse(g, r.out, r.err, Some(&m), None)?.into_new() else {
            return Err(no_inverse(g, r, "--ras2vox", fname));
        };
        print_rows(fpw(fp, r.out), &minv);
        return Ok(());
    }
    if f.vox2ras_tkr {
        let m = xfm_crs2xyz_tkreg(g, r.out, r.err, vg)?;
        print_rows(fpw(fp, r.out), &m);
        return Ok(());
    }
    if f.ras2vox_tkr {
        // `MatrixInverse(m, m)`: the input copied first
        let mut m = xfm_crs2xyz_tkreg(g, r.out, r.err, vg)?;
        let copy = m.clone();
        match matrix_inverse(g, r.out, r.err, Some(&copy), Some(&mut m))? {
            Ret::Given => {}
            Ret::New(n) => m = n,
            Ret::Null => return Err(no_inverse(g, r, "--ras2vox-tkr", fname)),
        }
        print_rows(fpw(fp, r.out), &m);
        return Ok(());
    }
    if f.tkr2scanner {
        let m = larmorx_freesurfer_mri::vol_geom::xfm_crs2xyz(g, r.out, r.err, vg, 0, false)?;
        let m2 = xfm_crs2xyz_tkreg(g, r.out, r.err, vg)?;
        let m2inv = matrix_inverse(g, r.out, r.err, Some(&m2), None)?.into_new();
        let p = matrix_multiply(g, r.out, r.err, Some(&m), m2inv.as_ref(), None)?.into_new();
        let Some(p) = p else {
            return Err(no_inverse(g, r, "--tkr2scanner", fname));
        };
        print_rows(fpw(fp, r.out), &p);
        return Ok(());
    }
    if f.scanner2tkr {
        let Some(p) = surface_ras_from_ras(g, r.out, r.err, vg)? else {
            return Err(no_inverse(g, r, "--scanner2tkr", fname));
        };
        print_rows(fpw(fp, r.out), &p);
        return Ok(());
    }
    if f.vox2ras_fsl {
        let m = xfm_crs2xyz_fsl(g, r.out, r.err, vg)?;
        print_rows(fpw(fp, r.out), &m);
        return Ok(());
    }
    if f.orientation {
        let _ = writeln!(fpw(fp, r.out), "{}", orientation(vg));
        return Ok(());
    }
    if f.slice_direction {
        let _ = writeln!(fpw(fp, r.out), "{}", slice_direction_name(vg));
        return Ok(());
    }
    if f.autoalign || f.orig_ras2vox {
        let (m, none) = if f.autoalign {
            (&mri.auto_align, "No auto align matrix present")
        } else {
            (&mri.orig_ras2vox, "No orig ras2vox matrix present")
        };
        match m {
            None => {
                let _ = writeln!(fpw(fp, r.out), "{none}");
            }
            Some(m) => {
                let w = fpw(fp, r.out);
                let mut sink = std::io::sink();
                matrix_print_fmt(g, Some(w), &mut sink, r.err, "%10f", Some(m));
            }
        }
        return Ok(());
    }
    if f.ctab {
        if let Some(ct) = &mri.ct {
            // its debug line (standard output) comes before the table
            let (mut debug, mut table) = (Vec::new(), Vec::new());
            ctab_print_ascii(g, &mut debug, ct, &mut table);
            let _ = r.out.write_all(&debug);
            let _ = fpw(fp, r.out).write_all(&table);
        }
        return Ok(());
    }
    if f.voxvolsum {
        let mut sum = F80::from_f64(0.0);
        for c in 0..vg.width {
            for row in 0..vg.height {
                for s in 0..vg.depth {
                    for fr in 0..mri.nframes {
                        let v = mri_get_vox_val(g, r.err, mri, c, row, s, fr, 0)?;
                        sum += F80::from_f32(v);
                    }
                }
            }
        }
        let v = sum * F80::from_f32(vg.xsize) * F80::from_f32(vg.ysize) * F80::from_f32(vg.zsize);
        let _ = writeln!(fpw(fp, r.out), "{}", cfmt::f80_f(v, 6));
        return Ok(());
    }
    if f.entropy {
        let h = mri_histogram(g, r.err, mri, 256)?;
        let e = larmorx_freesurfer_core::histo::histo_get_entropy(g, &h);
        let _ = writeln!(fpw(fp, r.out), "{}", cfmt::f(e, 6));
        return Ok(());
    }
    if f.stats {
        return print_stats(g, r, fp, mri);
    }
    if f.voxel {
        let [c, row, s] = o.voxel_crs;
        for fr in 0..mri.nframes {
            if mri.mtype == MRI_FLOAT_COMPLEX {
                let re = mri_get_vox_val(g, r.err, mri, c, row, s, fr, MRI_COMPLEX_REAL)?;
                let im = mri_get_vox_val(g, r.err, mri, c, row, s, fr, MRI_COMPLEX_IMAG)?;
                let _ = writeln!(
                    fpw(fp, r.out),
                    "[{c} {row} {s} {fr}] real = {}, imag = {}",
                    cfmt::f(f64::from(re), 6),
                    cfmt::f(f64::from(im), 6)
                );
            } else {
                let v = mri_get_vox_val(g, r.err, mri, c, row, s, fr, 0)?;
                let _ = writeln!(fpw(fp, r.out), "{}", cfmt::f(f64::from(v), 6));
            }
        }
        return Ok(());
    }
    if f.cmds {
        let _ = writeln!(r.out);
        for c in &mri.cmdlines {
            let _ = r.out.write_all(cstr(c));
            let _ = r.out.write_all(b"\n\n");
        }
        return Ok(());
    }
    if f.tags_to_file {
        return print_tags_to_file(g, r, o, fp, mri);
    }
    report(g, r, fp, mri, fname)
}

/// `--stats`: each frame's minimum, maximum and mean (`double`), the voxels slice by slice, row
/// by row; the mean's divisor the voxel count in `int` (FS-1153).
fn print_stats(g: &mut Globals, r: &mut Run, fp: &mut Fp, mri: &Mri) -> Result<(), Fatal> {
    let vg = &mri.vg;
    let (w, h, d) = (vg.width, vg.height, vg.depth);
    for fr in 0..mri.nframes {
        let mut min = f64::from(mri_get_vox_val(g, r.err, mri, 0, 0, 0, fr, 0)?);
        let mut max = min;
        let mut mean = 0.0f64;
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let val = f64::from(mri_get_vox_val(g, r.err, mri, x, y, z, fr, 0)?);
                    mean += val;
                    if val < min {
                        min = val;
                    }
                    if val > max {
                        max = val;
                    }
                }
            }
        }
        let wide = i64::from(d) * i64::from(h) * i64::from(w);
        let narrow = d.wrapping_mul(h).wrapping_mul(w);
        let count = if i64::from(narrow) != wide {
            g.notices.add(
                &FS_1153,
                format!(
                    "mri_info --stats: {wide} voxels, whose count FreeSurfer divides by in int ({narrow})"
                ),
            );
            if g.notices.reproduce() {
                f64::from(narrow)
            } else {
                wide as f64
            }
        } else {
            f64::from(narrow)
        };
        mean /= count;
        let _ = writeln!(
            fpw(fp, r.out),
            "frame {fr}:  min {}  max {}  mean {}",
            cfmt::f(min, 6),
            cfmt::f(max, 6),
            cfmt::f(mean, 6)
        );
    }
    Ok(())
}

/// The report (`do_file`'s end): the volume's header and its two matrices; the first line to
/// `--o`'s file when given, the rest to standard output.
fn report(
    g: &mut Globals,
    r: &mut Run,
    fp: &mut Fp,
    mri: &Mri,
    fname: &str,
) -> Result<(), Fatal> {
    let vg = &mri.vg;
    let precision = if g.gdiag & DIAG_INFO != 0 { 10 } else { 4 };
    let _ = writeln!(fpw(fp, r.out), "Volume information for {fname}");
    let t = mri_identify(g, r.dir, r.env, r.out, r.err, fname)?;
    let mut s = format!("          type: {}\n", type_to_string(t));
    s += &format!(
        "        intent: {} ({})\n",
        intent_name(mri.intent),
        mri.intent
    );
    if mri.nframes > 1 {
        s += &format!(
            "    dimensions: {} x {} x {} x {}\n",
            vg.width, vg.height, vg.depth, mri.nframes
        );
    } else {
        s += &format!(
            "    dimensions: {} x {} x {}\n",
            vg.width, vg.height, vg.depth
        );
    }
    let f66 = |v: f32| cfmt::f(f64::from(v), 6);
    s += &format!(
        "   voxel sizes: {}, {}, {}\n",
        f66(vg.xsize),
        f66(vg.ysize),
        f66(vg.zsize)
    );
    s += &format!(
        "          type: {} ({})\n",
        type_name(mri.mtype, true),
        mri.mtype
    );
    s += &format!("           fov: {}\n", fw(f64::from(mri.fov), 2, 3));
    s += &format!("           dof: {}\n", mri.dof);
    let f21 = |v: f32| fw(f64::from(v), 2, 1);
    s += &format!(
        "        xstart: {}, xend: {}\n",
        f21(mri.xstart),
        f21(mri.xend)
    );
    s += &format!(
        "        ystart: {}, yend: {}\n",
        f21(mri.ystart),
        f21(mri.yend)
    );
    s += &format!(
        "        zstart: {}, zend: {}\n",
        f21(mri.zstart),
        f21(mri.zend)
    );
    let f22 = |v: f64| fw(v, 2, 2);
    s += &format!(
        "            TR: {} msec, TE: {} msec, TI: {} msec, flip angle: {} degrees\n",
        f22(f64::from(mri.tr)),
        f22(f64::from(mri.te)),
        f22(f64::from(mri.ti)),
        f22(degrees(mri.flip_angle))
    );
    s += &format!("       nframes: {}\n", mri.nframes);
    let _ = r.out.write_all(s.as_bytes());
    match &mri.pedir {
        Some(p) => {
            let _ = r.out.write_all(b"       PhEncDir: ");
            let _ = r.out.write_all(cstr(p));
            let _ = r.out.write_all(b"\n");
        }
        None => {
            let _ = r.out.write_all(b"       PhEncDir: UNKNOWN\n");
        }
    }
    let mut s = format!(
        "       FieldStrength: {}\n",
        cfmt::f(f64::from(mri.field_strength), 6)
    );
    s += &format!(
        "ras xform {}present\n",
        if vg.ras_good_flag != 0 { "" } else { "not " }
    );
    let a = |v: f32| fw(f64::from(v), 8, precision);
    let c = |v: f32| fw(f64::from(v), 10, precision);
    s += &format!(
        "    xform info: x_r = {}, y_r = {}, z_r = {}, c_r = {}\n",
        a(vg.x_r),
        a(vg.y_r),
        a(vg.z_r),
        c(vg.c_r)
    );
    s += &format!(
        "              : x_a = {}, y_a = {}, z_a = {}, c_a = {}\n",
        a(vg.x_a),
        a(vg.y_a),
        a(vg.z_a),
        c(vg.c_a)
    );
    s += &format!(
        "              : x_s = {}, y_s = {}, z_s = {}, c_s = {}\n",
        a(vg.x_s),
        a(vg.y_s),
        a(vg.z_s),
        c(vg.c_s)
    );
    let _ = r.out.write_all(s.as_bytes());
    let talairach = if fio_is_directory(g, r.dir, fname) != 0 {
        true
    } else {
        matches!(fio_extension(Some(fname)).as_deref(), Some("mgz" | "mgh"))
    };
    if talairach {
        let _ = r.out.write_all(b"\ntalairach xfm : ");
        let _ = r.out.write_all(cstr(&mri.transform_fname));
        let _ = r.out.write_all(b"\n");
    }
    let _ = writeln!(r.out, "Orientation   : {}", orientation(vg));
    let _ = writeln!(r.out, "Primary Slice Direction: {}", slice_direction_name(vg));
    let m = vox2ras(g, r, mri)?;
    let _ = r.out.write_all(b"\nvoxel to ras transform:\n");
    pretty_matrix_print(g, r, Some(&m), precision);
    let det = matrix_determinant(g, r.err, &m);
    let _ = writeln!(r.out, "\nvoxel-to-ras determinant {}", gf(det));
    let minv = extract_r_to_i(g, r.out, r.err, vg)?;
    let _ = r.out.write_all(b"\nras to voxel transform:\n");
    pretty_matrix_print(g, r, minv.as_ref(), precision);
    for i in 0..mri.nframes {
        let Some(frame) = mri.frames.get(i as usize) else {
            break;
        };
        let thresh = f64::from(frame.thresh).abs() < f64::from(f32::EPSILON);
        if frame.label == 0 && thresh {
            continue;
        }
        let _ = writeln!(r.out, "frame {i} info:");
        if frame.label != 0 {
            let _ = writeln!(
                r.out,
                "\tlabel = {} ({})",
                cma_label_to_name(frame.label),
                frame.label
            );
        }
        if !thresh {
            let _ = writeln!(r.out, "\tthresh = {}", fw(f64::from(frame.thresh), 2, 1));
        }
    }
    Ok(())
}

/// A morph's report (`.m3z`, `.m3d`): its type, size and spacing to `--o`'s file (when given),
/// its two geometries on standard output; `Error reading <name>` when it cannot be read.
fn report_morph(g: &mut Globals, r: &mut Run, fp: &mut Fp, fname: &str) -> Result<(), Fatal> {
    let _ = writeln!(fpw(fp, r.out), "Input file is a 3D morph.");
    let Some(gcam) = gcam_read(g, r.out, r.err, r.env, r.dir, fname)? else {
        let _ = writeln!(fpw(fp, r.out), "Error reading {fname}");
        return Ok(());
    };
    let t = if gcam.mtype == GCAM_RAS {
        "GCAM_RAS"
    } else if gcam.mtype == GCAM_VOX {
        "GCAM_VOX"
    } else {
        "UNKNOWN"
    };
    let w = fpw(fp, r.out);
    let _ = writeln!(w, "type    : {t}");
    let _ = writeln!(
        w,
        "size    : ({}, {}, {})",
        gcam.width, gcam.height, gcam.depth
    );
    let _ = writeln!(w, "spacing : {}", gcam.spacing);
    let _ = writeln!(w);
    let _ = writeln!(w, "3D morph source geometry:");
    vgprint(r.out, &gcam.image, false);
    let _ = writeln!(fpw(fp, r.out));
    let _ = writeln!(fpw(fp, r.out), "3D morph atlas geometry:");
    vgprint(r.out, &gcam.atlas, false);
    let _ = r.out.flush();
    Ok(())
}

/// `__printTagsToFile(mri)`: the tags written to `--otags`' file (compressed when its name ends
/// in `z`), then printed on standard output (the matrices to `--o`'s file when given).
fn print_tags_to_file(
    g: &mut Globals,
    r: &mut Run,
    o: &Options,
    fp: &mut Fp,
    mri: &Mri,
) -> Result<(), Fatal> {
    let _ = r
        .out
        .write_all(b"\n==============================================================\n");
    let Some(name) = o.outtagfile.as_deref() else {
        let _ = writeln!(r.out, "save TAG data as (null)");
        return Err(no_argument(g, r, "--otags"));
    };
    let _ = writeln!(r.out, "save TAG data as {name}");
    let z = name.as_bytes().last() == Some(&b'z');
    let _ = writeln!(r.out, "use_compression = {}", i32::from(z));
    let path = resolve(r.dir, name);
    match ZnzFile::open(path.as_os_str().as_encoded_bytes(), b"w", z) {
        Some(mut file) => {
            mri_tag_write(g, r.out, r.err, mri, &mut file, false)?;
            file.close();
        }
        None => {
            g.notices.add(
                &FS_1151,
                format!(
                    "mri_info --otags {name}: a file that cannot be created, where FreeSurfer writes nothing and says nothing"
                ),
            );
            // the C library's errno after the failed open
            if let Err(e) = std::fs::File::create(&path) {
                g.errno = log::errno_of(&e).unwrap_or(2);
            }
            if !g.notices.reproduce() {
                let _ = writeln!(r.out, "ERROR: could not open {name}");
                return Err(Fatal::Exit(1));
            }
            // what the writer prints writing to no file: its own line and the colour table's
            // error (its debug lines with --diag-debug aside)
            if mri.warp_field_format == WARPFIELD_DTFMT_UNKNOWN {
                if mri.orig_ras2vox.is_some() {
                    let _ = writeln!(r.out, "saving original ras2vox");
                }
                if mri.ct.is_some() {
                    log::error_printf(g, r.err, ERROR_BADPARM, "CTABwriteIntoBinary: fp was NULL");
                }
            }
        }
    }
    if mri.warp_field_format != WARPFIELD_DTFMT_UNKNOWN {
        let valid = |vg: &VolGeom| if vg.valid != 0 { "valid" } else { "invalid" };
        let _ = writeln!(r.out, "TAG <TAG_GCAMORPH_GEOM>:");
        let _ = writeln!(
            r.out,
            "source volume geometry ({}):",
            valid(&mri.gcamorph_image_vg)
        );
        vgprint(r.out, &mri.gcamorph_image_vg, true);
        let _ = writeln!(
            r.out,
            "target volume geometry ({}):",
            valid(&mri.gcamorph_atlas_vg)
        );
        vgprint(r.out, &mri.gcamorph_atlas_vg, true);
        let _ = writeln!(r.out, "TAG <TAG_GCAMORPH_META>:");
        let _ = writeln!(r.out, "  warpFieldFormat: {}", mri.warp_field_format);
        let _ = writeln!(r.out, "  gcamorphSpacing: {}", mri.gcamorph_spacing);
        let _ = writeln!(
            r.out,
            "  gcamorphExp_k:   {}",
            cfmt::f(mri.gcamorph_exp_k, 6)
        );
        let _ = writeln!(r.out, "TAG <TAG_GCAMORPH_AFFINE>:");
        match &mri.gcamorph_affine {
            Some(m) => {
                let mut sink = std::io::sink();
                matrix_print_fmt(g, Some(fpw(fp, r.out)), &mut sink, r.err, "%10f", Some(m));
            }
            None => {
                let _ = writeln!(r.out, "  No gcamorphAffine matrix present");
            }
        }
        let _ = writeln!(r.out, "TAG <TAG_GCAMORPH_LABELS>:");
        if mri.gcamorph_label.is_some() {
            let _ = writeln!(r.out, "  gcamorphLabel info present");
        } else {
            let _ = writeln!(r.out, "  No gcamorphLabel info present");
        }
        return Ok(());
    }
    let _ = r.out.write_all(b"TAG <TAG_MGH_XFORM>: ");
    let _ = r.out.write_all(cstr(&mri.transform_fname));
    let _ = r.out.write_all(b"\n");
    let _ = writeln!(r.out, "TAG <TAG_AUTO_ALIGN>:");
    match &mri.auto_align {
        Some(m) => {
            let mut sink = std::io::sink();
            matrix_print_fmt(g, Some(fpw(fp, r.out)), &mut sink, r.err, "%10f", Some(m));
        }
        None => {
            let _ = writeln!(r.out, "  No auto align matrix present");
        }
    }
    if let Some(p) = &mri.pedir {
        let _ = r.out.write_all(b"TAG <TAG_PEDIR>: ");
        let _ = r.out.write_all(cstr(p));
        let _ = r.out.write_all(b"\n");
    }
    let _ = writeln!(r.out, "TAG <TAG_ORIG_RAS2VOX>:");
    match &mri.orig_ras2vox {
        Some(m) => {
            let mut sink = std::io::sink();
            matrix_print_fmt(g, Some(fpw(fp, r.out)), &mut sink, r.err, "%10f", Some(m));
        }
        None => {
            let _ = writeln!(r.out, "  No orig ras2vox matrix present");
        }
    }
    let _ = writeln!(
        r.out,
        "TAG <TAG_FIELDSTRENGTH>: {}",
        cfmt::f(f64::from(mri.field_strength), 6)
    );
    let _ = writeln!(r.out, "TAG <TAG_CMDLINE>:");
    for c in &mri.cmdlines {
        let _ = r.out.write_all(b"  ");
        let _ = r.out.write_all(cstr(c));
        let _ = r.out.write_all(b"\n\n");
    }
    let _ = writeln!(r.out, "TAG <TAG_MRI_FRAME>:");
    for fno in 0..mri.nframes {
        let Some(frame) = mri.frames.get(fno as usize) else {
            break;
        };
        let _ = writeln!(r.out, "  label: {}", frame.label);
        let _ = r.out.write_all(b"  name:  ");
        let _ = r.out.write_all(frame.name.c_bytes());
        let _ = r.out.write_all(b"\n");
        let _ = writeln!(
            r.out,
            "  thresh: {}",
            cfmt::f(f64::from(frame.thresh), 6)
        );
    }
    Ok(())
}

/// `__reportWarp(mri, fname)`: a warp field's report (`MGZ_INTENT_WARPMAP`), on standard output.
fn report_warp(g: &mut Globals, r: &mut Run, mri: &Mri, fname: &str) -> Result<(), Fatal> {
    let vg = &mri.vg;
    let _ = writeln!(r.out, "Input file is a 3D morph.");
    let _ = writeln!(r.out, "Volume information for {fname}");
    let t = mri_identify(g, r.dir, r.env, r.out, r.err, fname)?;
    let format = match mri.warp_field_format {
        WARPFIELD_DTFMT_ABS_CRS => "ABS_CRS",
        WARPFIELD_DTFMT_DISP_CRS => "DISP_CRS",
        WARPFIELD_DTFMT_ABS_RAS => "ABS_RAS",
        WARPFIELD_DTFMT_DISP_RAS => "DISP_RAS",
        _ => "UNKNOWN",
    };
    let mut s = format!("          type: {}\n", type_to_string(t));
    s += &format!(
        "        intent: {} ({})\n",
        intent_name(mri.intent),
        mri.intent
    );
    s += &format!(
        "        format: {format} ({})\n",
        mri.warp_field_format
    );
    s += &format!("       spacing: {}\n", mri.gcamorph_spacing);
    s += &format!("       exp_k  : {}\n", cfmt::f(mri.gcamorph_exp_k, 6));
    s += &format!(
        "    dimensions: {} x {} x {} x {}\n",
        vg.width, vg.height, vg.depth, mri.nframes
    );
    let f66 = |v: f32| cfmt::f(f64::from(v), 6);
    s += &format!(
        "   voxel sizes: {}, {}, {}\n",
        f66(vg.xsize),
        f66(vg.ysize),
        f66(vg.zsize)
    );
    s += &format!(
        "     data type: {} ({})\n",
        type_name(mri.mtype, false),
        mri.mtype
    );
    s += &format!(
        "\nras xform {}present\n",
        if vg.ras_good_flag != 0 { "" } else { "not " }
    );
    let a = |v: f32| fw(f64::from(v), 8, 4);
    let c = |v: f32| fw(f64::from(v), 10, 4);
    s += &format!(
        "    xform info: x_r = {}, y_r = {}, z_r = {}, c_r = {}\n",
        a(vg.x_r),
        a(vg.y_r),
        a(vg.z_r),
        c(vg.c_r)
    );
    s += &format!(
        "              : x_a = {}, y_a = {}, z_a = {}, c_a = {}\n",
        a(vg.x_a),
        a(vg.y_a),
        a(vg.z_a),
        c(vg.c_a)
    );
    s += &format!(
        "              : x_s = {}, y_s = {}, z_s = {}, c_s = {}\n",
        a(vg.x_s),
        a(vg.y_s),
        a(vg.z_s),
        c(vg.c_s)
    );
    s += &format!("Orientation   : {}\n", orientation(vg));
    s += &format!("Primary Slice Direction: {}\n", slice_direction_name(vg));
    s += "\n3D morph source geometry:\n";
    let _ = r.out.write_all(s.as_bytes());
    vgprint(r.out, &mri.gcamorph_image_vg, false);
    let _ = r.out.write_all(b"\n3D morph target geometry:\n");
    vgprint(r.out, &mri.gcamorph_atlas_vg, false);
    let _ = r.out.flush();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_of_a_morph() {
        assert!(morph_extension("m3z"));
        assert!(morph_extension("M3D"));
        assert!(morph_extension("xm3zy"));
        assert!(!morph_extension("M3z"));
        assert!(!morph_extension("mgz"));
    }

    #[test]
    fn degrees_as_freesurfer() {
        assert_eq!(degrees(std::f64::consts::PI), 180.0);
        assert_eq!(gd(degrees(0.5)), "28.6479");
    }

    #[test]
    fn names_of_types() {
        assert_eq!(precision_name(MRI_FLOAT_COMPLEX, false), None);
        assert_eq!(precision_name(MRI_FLOAT_COMPLEX, true), Some("float complex"));
        assert_eq!(type_name(MRI_RGB, false), "UNKNOWN");
        assert_eq!(type_name(MRI_RGB, true), "RGB");
    }
}
