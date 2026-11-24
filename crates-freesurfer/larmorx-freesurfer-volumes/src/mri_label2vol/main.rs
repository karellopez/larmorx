// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_label2vol`'s `main` (`mri_label2vol.cpp` 145-549), its `--defects` path (731-778),
//! `get_crs` (1259-1289), `is_surface_label` (1291-1300) and `load_annotation` (1302-1328): the
//! program's entry point [`run`] (registered as `mri_label2vol` in the crate's `PROGRAMS`) and
//! [`main`].
//!
//! `mri_label2vol <options>`. The command line as `getAllInfo` reports it first; the version
//! options (`handleVersionOption`; only version options: status 0); no argument: the usage, status
//! 1; then the options ([`super::options`]), their checks and the settings printed. The template's
//! header (`ERROR: reading <t> header`, status 1) gives the tkregister (or, `--native-vox2ras`, the
//! scanner) RAS-to-voxel matrix, printed with the voxel volume and the hit threshold
//! (`--fillthresh` times the template's voxel volume over `--labvoxvol`'s); the registration from
//! `--reg` (`regio_read_register`), `--regheader` (`MRItkRegMtx` from the label volume's header) or
//! `--identity`, printed, inverted with `--invertmtx`, and multiplied into the matrix. With
//! `--proj` or `--annot` the surface `<sd>/<subject>/surf/<hemi>.<surf>` and its thickness, and
//! `--annot`'s annotation (its labels: the largest annotation index plus one). `--aparc+aseg`
//! takes `<sd>/<subject>/mri/aparc+aseg.mgz` (the registration's subject when no `--subject`). A
//! segmentation (`--seg`) is mapped by `MRIaseg2volMU` with `--upsample` and `--ttype`, its colour
//! table copied, written with the command line, then `--hits` and `--pvf` (the hits over the
//! template's voxel volume), `mri_label2vol done`, status 0. Labels and annotations: a hit volume
//! (`short`, one frame per label) counts the label points (or, `--proj`, their projections along
//! the normal from `start` to `stop` by `delta`) falling in each template voxel, `--label-stat`'s
//! volume keeps the last point's stat, `--stat-thresh` skips points whose stat is below it; then
//! `--label-stat`, `--hits`, `--pvf` written; the output (`int`) takes, at each voxel, the label
//! with the most hits above the threshold (the first of equal counts): its number plus one (an
//! annotation's index) plus `--offset`, 0 where none; written with the command line; `mri_label2vol
//! done `, status 0.
//!
//! Not ported yet (part 2 of queue item 3.2-40): `--new-aseg2vol` (`MRIseg2SegPVF`) and
//! `--fill-ribbon` (`MRIsurf2VolOpt`), which end with a message, status 1.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-mri-label2vol.md`):
//! FS-1249 and FS-1250 (fs.surf's `MRISdefects2Seg`), FS-1251 to FS-1253 (`--defects`), FS-1254
//! (`--pvf` with labels ends with status 1 and no output), FS-1255 (more than 32,767 hits in a
//! voxel), FS-1256 (`--proj none`), FS-1257 (a projection that never ends), FS-1258 (a label
//! point beyond the surface's vertices with `--proj`), FS-1259 ([`super::options`]), FS-1260 (a
//! segmentation without a template), FS-1262 (label codes past the `int` range).

use std::io::Write;

use larmorx_core::defects::{
    FS_1251, FS_1252, FS_1253, FS_1254, FS_1255, FS_1256, FS_1257, FS_1258, FS_1260, FS_1262,
};
use larmorx_freesurfer_core::diag::diag_init;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, Fatal};
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::matrix::matrix_multiply;
use larmorx_freesurfer_core::matrix_ops::{matrix_identity, matrix_inverse};
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::version::{get_all_info, get_version, handle_version_option};
use larmorx_freesurfer_io::identify::MRI_VOLUME_TYPE_UNKNOWN;
use larmorx_freesurfer_io::label::{LABEL_COORDS_TKREG_RAS, Label, label_read};
use larmorx_freesurfer_io::mriio::{
    mri_add_command_line, mri_read_header, mri_read_volume, mri_write,
};
use larmorx_freesurfer_io::registerio::{Register, regio_read_register};
use larmorx_freesurfer_io::streams::Io;
use larmorx_freesurfer_mri::ctab::ctab_deep_copy;
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, MRI_FLOAT, MRI_INT, MRI_SHORT, Mri, mri_alloc_sequence, mri_copy_header,
};
use larmorx_freesurfer_mri::tkreg::{tkreg_mtx, xfm_crs2xyz_tkreg};
use larmorx_freesurfer_mri::vol_geom::xfm_crs2xyz;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::annot::read_annotation;
use larmorx_freesurfer_surf::curvfile::read_curvature_file;
use larmorx_freesurfer_surf::defects2seg::{Defects2Seg, mris_defects2seg};
use larmorx_freesurfer_surf::labelsurf::mris_label2mask;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::readwrite;
use larmorx_freesurfer_volume::aseg2vol::mri_aseg2vol_mu;
use larmorx_freesurfer_volume::frames::mri_const;
use larmorx_freesurfer_volume::label_coords::{self, LabelDst, label_to_surface_ras};
use larmorx_freesurfer_volume::resize::mri_change_type;
use larmorx_freesurfer_volume::vol2vol::transform_reg_dat2_lta;

use super::options::{
    PROJ_TYPE_ABS, PROJ_TYPE_FRAC, check_options, dump_options, g_fmt, or_null, parse_commandline,
    usage_exit,
};
use super::surface::{
    annotation_index, annotation2label, mris_load_surf_subject, proj_norm_dist,
    proj_norm_frac_thick,
};
use crate::mri_segstats::main::RegLta;
use crate::mri_segstats::options::scan_d;

/// The program's name as `argv[0]` gives it (the scripts run it by name).
pub const NAME: &str = "mri_label2vol";

/// The program's streams, environment and working directory.
pub struct Run<'a, 'b> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: Environment,
    pub dir: Option<&'b std::path::Path>,
}

/// `mri_label2vol`'s entry point: its original arguments (without the program's name), the
/// settings, the two streams; the exit status. The notices of the run follow on standard error
/// (`larmorx: ...`).
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut g = Globals::new(settings.mode);
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

/// `MatrixPrint(stdout, m)`.
fn print_matrix(g: &mut Globals, r: &mut Run, m: Option<&Matrix>) {
    let mut buf = Vec::new();
    larmorx_freesurfer_core::matrix_ops::matrix_print(
        g,
        Some(&mut buf),
        &mut std::io::sink(),
        r.err,
        m,
        5,
    );
    let _ = r.out.write_all(&buf);
}

/// `MRIread(name)`.
fn read_volume(g: &mut Globals, r: &mut Run, name: &str) -> Result<Option<Mri>, Fatal> {
    let env = r.env.clone();
    mri_read_volume(g, r.out, r.err, &env, r.dir, name)
}

/// `MRIreadHeader(name, MRI_VOLUME_TYPE_UNKNOWN)`.
fn read_header(g: &mut Globals, r: &mut Run, name: &str) -> Result<Option<Mri>, Fatal> {
    let env = r.env.clone();
    mri_read_header(g, r.out, r.err, &env, r.dir, name, MRI_VOLUME_TYPE_UNKNOWN)
}

/// `MRIwrite(m, name)`: the writer's status.
fn write_volume(g: &mut Globals, r: &mut Run, m: &mut Mri, name: &str) -> Result<i32, Fatal> {
    let env = r.env.clone();
    mri_write(
        g,
        r.out,
        r.err,
        &env,
        r.dir,
        m,
        name,
        None,
        MGZ_INTENT_UNKNOWN,
    )
}

/// `LabelRead(NULL, name)`.
fn read_label(g: &mut Globals, r: &mut Run, name: &str) -> Result<Option<Label>, Fatal> {
    let env = r.env.clone();
    label_read(g, &env, r.out, r.err, r.dir, None, name)
}

/// `MRISread(name)`.
fn read_surf(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    r: &mut Run,
    name: &str,
) -> Result<Option<Mris>, Fatal> {
    let mut io = Io {
        out: &mut *r.out,
        err: &mut *r.err,
        env: &r.env,
    };
    readwrite::read(g, sg, &mut io, r.dir, name, true)
}

/// The byte offset of voxel `(c, r, s, f)` of an element of `size` bytes (FreeSurfer's
/// `MRI?seq_vox`) of a volume `mri_label2vol` allocated itself (its rows its own), in the extent.
fn seq_at(m: &Mri, c: i32, r: i32, s: i32, f: i32, size: usize) -> usize {
    let sl = s as u64 + f as u64 * m.vg.depth as u64;
    let row = (sl * m.vg.height as u64 + r as u64) * m.vox_per_row * m.bytes_per_vox as u64;
    (row + c as u64 * size as u64) as usize
}

/// `MRISseq_vox(HitVol, c, r, s, f)++`: a hit counted in the `short` volume. FreeSurfer's short
/// wraps from 32,767 to -32,768 (FS-1255, `reproduce`); by default the count stops at 32,767.
fn count_hit(g: &mut Globals, m: &mut Mri, c: i32, r: i32, s: i32, f: i32) {
    let o = seq_at(m, c, r, s, f, 2);
    let chunk = m.chunk.as_mut().expect("the hit volume's voxels");
    let v = i16::from_le_bytes([chunk[o], chunk[o + 1]]);
    let w = if v == i16::MAX {
        g.notices.add(
            &FS_1255,
            "mri_label2vol: more than 32,767 label points in one voxel (FreeSurfer's short count wraps to -32,768)",
        );
        if g.notices.reproduce() {
            i16::MIN
        } else {
            i16::MAX
        }
    } else {
        v.wrapping_add(1)
    };
    chunk[o..o + 2].copy_from_slice(&w.to_le_bytes());
}

/// `MRIFseq_vox(m, c, r, s, f) = v`.
fn set_float(m: &mut Mri, c: i32, r: i32, s: i32, f: i32, v: f32) {
    let o = seq_at(m, c, r, s, f, 4);
    let chunk = m.chunk.as_mut().expect("the stat volume's voxels");
    chunk[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// `MRIIseq_vox(m, c, r, s, f) = v`.
fn set_int(m: &mut Mri, c: i32, r: i32, s: i32, f: i32, v: i32) {
    let o = seq_at(m, c, r, s, f, 4);
    let chunk = m.chunk.as_mut().expect("the output's voxels");
    chunk[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// `(int) rint(x)` of a coordinate as x86-64 converts it: `INT_MIN` for NaN and beyond the `int`
/// range (outside every volume, the same answer as a saturated value would give).
fn rint_int(x: f32) -> i32 {
    let v = f64::from(x).round_ties_even();
    if v.is_nan() || v >= 2147483648.0 || v < -2147483648.0 {
        i32::MIN
    } else {
        v as i32
    }
}

/// `get_crs(Tras2vox, x, y, z, &c, &r, &s, vol)`: the point through the matrix's first three rows
/// in `float` (`m11 x + m12 y + m13 z + m14`), each rounded with `rint`; whether it is outside
/// `vol`'s extent.
fn get_crs(t: &Matrix, x: f32, y: f32, z: f32, vol: &Mri) -> ([i32; 3], bool) {
    let row = |i: i32| {
        add(
            add(
                add(mul(t.relt(i, 1), x), mul(t.relt(i, 2), y)),
                mul(t.relt(i, 3), z),
            ),
            t.relt(i, 4),
        )
    };
    let crs = [rint_int(row(1)), rint_int(row(2)), rint_int(row(3))];
    let [c, r, s] = crs;
    let oob =
        c < 0 || c >= vol.vg.width || r < 0 || r >= vol.vg.height || s < 0 || s >= vol.vg.depth;
    (crs, oob)
}

/// `is_surface_label(label)`: no point at vertex -1.
fn is_surface_label(label: &Label) -> bool {
    label.lv[..npoints(label)].iter().all(|lv| lv.vno != -1)
}

fn npoints(label: &Label) -> usize {
    usize::try_from(label.n_points)
        .unwrap_or(0)
        .min(label.lv.len())
}

/// `load_annotation(annotfile, Surf)`: the annotation read into the surface (`Loading annotations
/// from <file>`; `ERROR: could not load <file>`, status 1), the largest annotation index of its
/// vertices plus one (`annotidmax = <n>`).
fn load_annotation(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    r: &mut Run,
    annotfile: &str,
    surf: &mut Mris,
) -> Result<i32, Fatal> {
    let _ = writeln!(r.out, "Loading annotations from {annotfile}");
    let e = {
        let mut io = Io {
            out: &mut *r.out,
            err: &mut *r.err,
            env: &r.env,
        };
        read_annotation(g, sg, &mut io, r.dir, surf, annotfile, -1, None)?
    };
    if e != 0 {
        let _ = writeln!(r.out, "ERROR: could not load {annotfile}");
        return Err(Fatal::Exit(1));
    }
    let mut annotidmax = -1;
    for vtxno in 0..usize::try_from(surf.nvertices).unwrap_or(0) {
        let annot = surf.vertex(vtxno).annotation;
        let id = annotation_index(g, sg, r, surf, annot);
        if annotidmax < id {
            annotidmax = id;
        }
    }
    annotidmax += 1;
    let _ = writeln!(r.out, "annotidmax = {annotidmax}");
    Ok(annotidmax)
}

/// `MRImultiplyConst(src, k, NULL)`: a new `float` volume of `src`'s extent and frames, its
/// header, each voxel `v k` in `double` (columns outermost, then rows, slices, frames).
fn multiply_const(g: &mut Globals, r: &mut Run, src: &Mri, k: f64) -> Result<Mri, Fatal> {
    let mut dst = mri_alloc_sequence(
        g,
        r.out,
        r.err,
        src.vg.width,
        src.vg.height,
        src.vg.depth,
        MRI_FLOAT,
        src.nframes,
    )?;
    mri_copy_header(g, r.out, r.err, src, Some(&mut dst))?;
    for c in 0..src.vg.width {
        for rr in 0..src.vg.height {
            for s in 0..src.vg.depth {
                for f in 0..src.nframes {
                    let v = f64::from(mri_get_vox_val(g, r.err, src, c, rr, s, f, 0)?);
                    mri_set_vox_val(g, r.err, &mut dst, c, rr, s, f, mul64(v, k) as f32, 0)?;
                }
            }
        }
    }
    Ok(dst)
}

/// Part 2 of queue item 3.2-40, not ported yet (status 1).
fn not_ported(r: &mut Run, what: &str) -> Fatal {
    let _ = writeln!(
        r.err,
        "larmorx-freesurfer: mri_label2vol: {what} is not ported yet"
    );
    Fatal::Exit(1)
}

/// `--defects <surf> <defectno> <template> <offset> <merge> <out> [<label>]`, which FreeSurfer
/// runs inside `parse_commandline` and ends the program with (the status returned as the
/// [`Fatal`] to end with): the surface, the defect numbers and the template read (each status 1
/// when it does not read); the offset and the merge flag (`%d`); with a seventh argument that is
/// not an option the label (`Contraining to label <label>`; status 1 when it does not read), whose
/// mask zeroes the defect numbers outside it; without merging the template zeroed and made `int`
/// (`Changing input type <t> to MRI_INT`); `Converting defects to volume: offset=<o>, merge=<m>`,
/// [`mris_defects2seg`] (status 1 when it fails), `Writing to <out>`, the volume written (its
/// status 1 when the writer fails), status 0.
pub fn defects(g: &mut Globals, r: &mut Run, p: &[String]) -> Result<Fatal, Fatal> {
    let mut sg = SurfGlobals::default();
    let Some(defsurf) = read_surf(g, &mut sg, r, &p[0])? else {
        return Ok(Fatal::Exit(1));
    };
    let Some(mut defno) = read_volume(g, r, &p[1])? else {
        return Ok(Fatal::Exit(1));
    };
    let Some(mut deftemp) = read_volume(g, r, &p[2])? else {
        return Ok(Fatal::Exit(1));
    };
    let number = |g: &mut Globals, r: &mut Run, s: &str, what: &str| -> Result<i32, Fatal> {
        match scan_d(g, s) {
            Some(v) => Ok(v),
            None => {
                g.notices.add(
                    &FS_1252,
                    format!(
                        "mri_label2vol --defects: the {what} '{s}' is not a number (FreeSurfer uses memory contents)"
                    ),
                );
                if g.notices.reproduce() {
                    return Ok(0);
                }
                let _ = writeln!(r.out, "ERROR: --defects: the {what} '{s}' is not a number");
                Err(Fatal::Exit(1))
            }
        }
    };
    let defoffset = number(g, r, &p[3], "offset")?;
    let defmerge = number(g, r, &p[4], "merge flag")?;
    // FreeSurfer: nargc > 6 && CMDnthIsArg(nargc + 6, pargv, 0), whose `pargv[0]` is the surface
    let freesurfer_label = p.len() > 6 && !p[0].starts_with("--");
    let fixed_label = p.len() > 6 && !p[6].starts_with("--");
    if freesurfer_label != fixed_label {
        g.notices.add(
            &FS_1251,
            if fixed_label {
                format!(
                    "mri_label2vol --defects: the label '{}' used (FreeSurfer drops it because the surface's name '{}' starts with --)",
                    p[6], p[0]
                )
            } else {
                format!(
                    "mri_label2vol --defects: '{}' is an option, not a label (FreeSurfer reads it as the label)",
                    p[6]
                )
            },
        );
    }
    let use_label = if g.notices.reproduce() {
        freesurfer_label
    } else {
        fixed_label
    };
    if use_label {
        let _ = writeln!(r.out, "Contraining to label {}", p[6]);
        let Some(label) = read_label(g, r, &p[6])? else {
            return Ok(Fatal::Exit(1));
        };
        let tmp = {
            let mut io = Io {
                out: &mut *r.out,
                err: &mut *r.err,
                env: &r.env,
            };
            mris_label2mask(g, &mut io, &defsurf, &label, None, 0, false)?
        };
        let Some(tmp) = tmp.into_new() else {
            g.notices.add(
                &FS_1253,
                "mri_label2vol --defects: the label's mask could not be made (FreeSurfer then reads a NULL volume and crashes)",
            );
            if g.notices.reproduce() {
                return Ok(Fatal::Exit(139));
            }
            let _ = writeln!(
                r.out,
                "ERROR: --defects: the label {} does not fit the surface",
                p[6]
            );
            return Ok(Fatal::Exit(1));
        };
        for v in 0..defsurf.nvertices {
            if mri_get_vox_val(g, r.err, &tmp, v, 0, 0, 0, 0)? == 0.0 {
                mri_set_vox_val(g, r.err, &mut defno, v, 0, 0, 0, 0.0, 0)?;
            }
        }
    }
    if defmerge == 0 {
        let dims = [
            deftemp.vg.width,
            deftemp.vg.height,
            deftemp.vg.depth,
            deftemp.nframes,
        ];
        mri_const(g, r.out, r.err, dims, 0.0, Some(&mut deftemp))?;
        if deftemp.mtype != MRI_INT {
            let _ = writeln!(r.out, "Changing input type {} to MRI_INT", deftemp.mtype);
            let env = r.env.clone();
            let Some(t) = mri_change_type(g, r.out, r.err, &env, &deftemp, MRI_INT, 0.0, 0.0, 1)?
            else {
                // MRIchangeType of a zeroed volume to int does not fail
                return Ok(Fatal::Exit(1));
            };
            deftemp = t;
        }
    }
    let _ = writeln!(
        r.out,
        "Converting defects to volume: offset={defoffset}, merge={defmerge}"
    );
    let res = {
        let mut io = Io {
            out: &mut *r.out,
            err: &mut *r.err,
            env: &r.env,
        };
        mris_defects2seg(g, &sg, &mut io, &defsurf, &defno, defoffset, &mut deftemp)?
    };
    match res {
        Defects2Seg::Done => {}
        Defects2Seg::Crash => return Ok(Fatal::Exit(139)),
        Defects2Seg::Mismatch | Defects2Seg::Refused => return Ok(Fatal::Exit(1)),
    }
    let _ = writeln!(r.out, "Writing to {}", p[5]);
    if write_volume(g, r, &mut deftemp, &p[5])? != 0 {
        return Ok(Fatal::Exit(1));
    }
    Ok(Fatal::Exit(0))
}

/// `mri_label2vol`'s `main` on `args` (after the program's name).
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let env = r.env.clone();
    let cmdline = get_all_info(g, &env, &argv, NAME);
    let (nargs, argv) = handle_version_option(g, &env, r.out, &argv, NAME);
    if nargs > 0 && argv.len() == 1 {
        return Ok(());
    }
    g.progname = NAME.to_owned();
    diag_init(g, &env, r.out);
    let args = &argv[1..];
    if args.is_empty() {
        return Err(usage_exit(r));
    }
    let mut o = match parse_commandline(g, r, args) {
        Ok(o) => o,
        Err(Fatal::Exit(0)) => return Ok(()),
        Err(f) => return Err(f),
    };
    check_options(g, r, &mut o)?;
    dump_options(r, &o);
    let _ = writeln!(r.out, "{}", get_version());
    let sdir = o.subjects_dir.clone().unwrap_or_default();

    let mut temp: Option<Mri> = None;
    let mut tras2vox: Option<Matrix> = None;
    let mut rmat: Option<Matrix> = None;
    let mut regsubject: Option<Vec<u8>> = None;
    let mut temp_vox_vol = 0.0f64;
    let mut n_hits_thresh = 0.0f64;
    if let Some(tid) = o.temp_vol_id.clone() {
        let Some(t) = read_header(g, r, &tid)? else {
            let _ = writeln!(r.out, "ERROR: reading {tid} header");
            return Err(Fatal::Exit(1));
        };
        let tvox2ras = if !o.use_native_vox2ras {
            xfm_crs2xyz_tkreg(g, r.out, r.err, &t.vg)?
        } else {
            xfm_crs2xyz(g, r.out, r.err, &t.vg, 0, false)?
        };
        let mut t2v = matrix_inverse(g, r.out, r.err, Some(&tvox2ras), None)?.into_new();
        let _ = writeln!(r.out, "Template RAS-to-Vox: --------");
        print_matrix(g, r, t2v.as_ref());
        // TempVoxVol = xsize * ysize * zsize (float products, kept in a double)
        temp_vox_vol = f64::from(mul(mul(t.vg.xsize, t.vg.ysize), t.vg.zsize));
        n_hits_thresh = div64(mul64(o.fill_thresh, temp_vox_vol), o.label_vox_vol);
        let _ = writeln!(r.out, "Template Voxel Volume: {}", g_fmt(temp_vox_vol));
        let _ = writeln!(r.out, "nHits Thresh: {}", g_fmt(n_hits_thresh));
        if let Some(regfile) = o.reg_mat_file.clone() {
            let _ = writeln!(r.out, "Loading registration from {regfile}");
            let mut reg = Register {
                subject: None,
                inplaneres: 0.0,
                betplaneres: 0.0,
                intensity: 0.0,
                r: None,
                float2int: 0,
            };
            let mut lta_io = RegLta { env: &env };
            let st = regio_read_register(g, r.out, r.err, r.dir, &regfile, &mut reg, &mut lta_io)?;
            if st != 0 {
                return Err(Fatal::Exit(1));
            }
            regsubject = reg.subject;
            rmat = reg.r;
        } else {
            if o.reg_header {
                let _ = writeln!(r.out, "Computing registration based on header");
                let lvf = o.label_vol_file.clone().unwrap_or_default();
                let Some(lv) = read_header(g, r, &lvf)? else {
                    let _ = writeln!(r.out, "ERROR: reading {lvf}");
                    return Err(Fatal::Exit(1));
                };
                rmat = Some(tkreg_mtx(g, r.out, r.err, &lv.vg, &t.vg, None)?);
            }
            if o.reg_identity {
                let _ = writeln!(r.out, "Using Identity Matrix");
                rmat = matrix_identity(g, r.out, r.err, 4, None)?.into_new();
            }
        }
        let _ = writeln!(r.out, "RegMat: --------");
        print_matrix(g, r, rmat.as_ref());
        if o.invert_mtx {
            let _ = writeln!(r.out, "Inverting matrix");
            let copy = rmat.clone();
            if let Some(m) = rmat.as_mut() {
                matrix_inverse(g, r.out, r.err, copy.as_ref(), Some(m))?;
            }
            let _ = writeln!(r.out, "RegMat: --------");
            print_matrix(g, r, rmat.as_ref());
        }
        // MatrixMultiply(Tras2vox, R, Tras2vox): into its first input
        let a = t2v.clone();
        match t2v.as_mut() {
            Some(m) => {
                matrix_multiply(g, r.out, r.err, a.as_ref(), rmat.as_ref(), Some(m))?;
            }
            None => {
                t2v = matrix_multiply(g, r.out, r.err, a.as_ref(), rmat.as_ref(), None)?.into_new();
            }
        }
        let _ = writeln!(r.out, "Label RAS-to-Vox: --------");
        print_matrix(g, r, t2v.as_ref());
        tras2vox = t2v;
        temp = Some(t);
    }

    let mut sg = SurfGlobals::default();
    let mut surf: Option<Mris> = None;
    let mut nlabels = o.nlabels() as i32;
    if o.surf_needed {
        let subject = o.subject.clone().unwrap_or_default();
        let hemi = o.hemi.clone().unwrap_or_default();
        let mut s = mris_load_surf_subject(g, &mut sg, r, &subject, &hemi, &o.surf_id, &sdir)?;
        let fname = format!(
            "{sdir}/{}/surf/{hemi}.thickness",
            String::from_utf8_lossy(&subject)
        );
        let _ = writeln!(r.out, "Reading thickness {fname}");
        {
            let mut io = Io {
                out: &mut *r.out,
                err: &mut *r.err,
                env: &r.env,
            };
            read_curvature_file(g, &mut sg, &mut io, r.dir, &mut s, &fname, None, None)?;
        }
        if let Some(af) = o.annot_file.clone() {
            nlabels = load_annotation(g, &mut sg, r, &af, &mut s)?;
            if nlabels == 0 {
                let _ = writeln!(r.out, "ERROR: load annot file {af}");
                return Err(Fatal::Exit(1));
            }
        }
        surf = Some(s);
    }
    if o.use_aparc_plus_aseg {
        let subject = match o.subject.clone() {
            Some(s) => s,
            None => regsubject.clone().unwrap_or_default(),
        };
        o.aseg_fspec = Some(format!(
            "{sdir}/{}/mri/aparc+aseg.mgz",
            String::from_utf8_lossy(&subject)
        ));
    }
    if let Some(aspec) = o.aseg_fspec.clone() {
        let Some(mut aseg) = read_volume(g, r, &aspec)? else {
            let _ = writeln!(r.out, "ERROR: loading aseg {aspec}");
            return Err(Fatal::Exit(1));
        };
        if o.use_new_aseg2vol {
            return Err(not_ported(
                r,
                "--new-aseg2vol (MRIseg2SegPVF, part 2 of queue item 3.2-40)",
            ));
        }
        let Some(t) = temp.as_ref() else {
            g.notices.add(
                &FS_1260,
                "mri_label2vol: a segmentation with --fill-ribbon and no --temp (FreeSurfer maps it onto a template it never read and crashes)",
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(139));
            }
            let _ = writeln!(
                r.out,
                "ERROR: a segmentation needs a template volume (--temp)"
            );
            return Err(Fatal::Exit(1));
        };
        let Some(mut aseg2vol) =
            transform_reg_dat2_lta(g, r.out, r.err, &aseg.vg, &t.vg, rmat.as_ref())?
        else {
            return Err(Fatal::Exit(1));
        };
        let ct = o.ct_tissue_type.clone();
        let Some((mut outvol, mut hitvol)) = mri_aseg2vol_mu(
            g,
            r.out,
            r.err,
            &env,
            &mut aseg,
            &mut aseg2vol,
            o.fill_thresh,
            o.upsample_factor,
            ct.as_ref(),
        )?
        else {
            // only a colour table without tissue types gives none: the program's have them
            return Err(Fatal::Exit(1));
        };
        mri_add_command_line(g, r.err, &mut outvol, cmdline.as_bytes());
        if aseg.ct.is_some() {
            outvol.ct = ctab_deep_copy(g, r.err, aseg.ct.as_ref());
        }
        let out_name = o.out_vol_id.clone().unwrap_or_default();
        if write_volume(g, r, &mut outvol, &out_name)? != 0 {
            return Err(Fatal::Exit(1));
        }
        if let Some(h) = o.hit_vol_id.clone() {
            mri_add_command_line(g, r.err, &mut hitvol, cmdline.as_bytes());
            write_volume(g, r, &mut hitvol, &h)?;
        }
        if let Some(p) = o.pvf_vol_id.clone() {
            let _ = writeln!(r.out, "PVF {p}");
            let _ = writeln!(r.out, "Computing PVF {}", g_fmt(temp_vox_vol));
            let mut pvf = multiply_const(g, r, &hitvol, div64(1.0, temp_vox_vol))?;
            write_volume(g, r, &mut pvf, &p)?;
        }
        let _ = writeln!(r.out, "mri_label2vol done");
        return Ok(());
    }
    let _ = writeln!(r.out, "nlabels = {nlabels}");

    if o.fill_ribbon {
        return Err(not_ported(
            r,
            "--fill-ribbon (MRIsurf2VolOpt, part 2 of queue item 3.2-40)",
        ));
    }

    let t = temp.as_ref().expect("check_options asks for --temp");
    let t2v = tras2vox.as_ref().expect("the template's matrix");
    let nvox = i64::from(t.vg.width) * i64::from(t.vg.height) * i64::from(t.vg.depth);
    let _ = writeln!(
        r.out,
        "Allocating Hit Volume ({}) voxels",
        nvox.wrapping_mul(i64::from(nlabels))
    );
    let mut hitvol = mri_alloc_sequence(
        g,
        r.out,
        r.err,
        t.vg.width,
        t.vg.height,
        t.vg.depth,
        MRI_SHORT,
        nlabels,
    )?;
    mri_copy_header(g, r.out, r.err, t, Some(&mut hitvol))?;
    hitvol.nframes = nlabels;
    let mut statvol = None;
    if o.do_label_stat_vol {
        let mut s = mri_alloc_sequence(
            g,
            r.out,
            r.err,
            t.vg.width,
            t.vg.height,
            t.vg.depth,
            MRI_FLOAT,
            1,
        )?;
        mri_copy_header(g, r.out, r.err, t, Some(&mut s))?;
        s.nframes = 1;
        statvol = Some(s);
    }

    // `--proj`'s loop: FS-1257 where it would never end
    let proj_hangs = o.do_proj
        && o.proj_delta > 0.0
        && o.proj_start <= o.proj_stop
        && (o.proj_stop.is_infinite()
            || div64(o.proj_stop - o.proj_start, o.proj_delta) >= 4_503_599_627_370_496.0);

    let mut xyz = [0.0f32; 3];
    for nthlabel in 0..nlabels {
        if o.debug {
            let _ = write!(r.out, "{nthlabel:2} ");
            let _ = r.out.flush();
            if nthlabel % 20 == 19 {
                let _ = writeln!(r.out);
                let _ = r.out.flush();
            }
        }
        let mut srclabel: Option<Label> = None;
        if o.annot_file.is_none() {
            let name = o.label_list[nthlabel as usize].clone();
            let _ = writeln!(r.out, "Loading {name}");
            let Some(mut l) = read_label(g, r, &name)? else {
                let _ = writeln!(r.out, "ERROR reading {name}");
                return Err(Fatal::Exit(1));
            };
            if l.coords != LABEL_COORDS_TKREG_RAS {
                let Some(tk) = o.to_tkr_template.as_mut() else {
                    let _ = r.out.write_all(b"ERROR: label is not in tkreg coords so you need to specify atemplate with --tkr-template MRI. The MRI should be the volumethat the label was created on\n");
                    return Err(Fatal::Exit(1));
                };
                let _ = writeln!(r.out, "  Converting label coords to TkRegister RAS");
                let mut io = label_coords::Io {
                    out: &mut *r.out,
                    err: &mut *r.err,
                    env: &env,
                };
                let src = l.clone();
                l = label_to_surface_ras(g, &mut io, &src, tk, None, LabelDst::Same)?;
            }
            srclabel = Some(l);
        }
        if o.annot_file.is_some() {
            let s = surf.as_ref().expect("the annotation's surface");
            match annotation2label(g, &mut sg, r, nthlabel, s)? {
                Some(l) => srclabel = Some(l),
                None => continue,
            }
        }
        let label = srclabel.expect("a label");
        if o.do_proj && !is_surface_label(&label) {
            let name = o.label_list.get(nthlabel as usize).map(String::as_str);
            let _ = writeln!(
                r.out,
                "ERROR: label {} is not a surface label.",
                or_null(name)
            );
            return Err(Fatal::Exit(1));
        }
        for nthpoint in 0..npoints(&label) {
            let lv = label.lv[nthpoint];
            if o.do_stat_thresh && f64::from(lv.stat) < o.stat_thresh {
                continue;
            }
            if o.debug {
                let _ = writeln!(r.out, "  nthpoint = {nthpoint}");
            }
            if o.do_proj {
                let s = surf.as_ref().expect("the projection's surface");
                let vtxno = lv.vno;
                // the loop runs at least once; it reads the vertex for abs, frac and --debug
                let runs = o.proj_start <= o.proj_stop;
                let typed = o.proj_type_id == PROJ_TYPE_ABS || o.proj_type_id == PROJ_TYPE_FRAC;
                if runs && (typed || o.debug) && !(0..s.nvertices).contains(&vtxno) {
                    g.notices.add(
                        &FS_1258,
                        format!(
                            "mri_label2vol --proj: a label point at vertex {vtxno} of a surface of {} vertices (FreeSurfer reads outside the surface's vertices)",
                            s.nvertices
                        ),
                    );
                    if g.notices.reproduce() {
                        return Err(Fatal::Exit(139));
                    }
                    let _ = writeln!(
                        r.out,
                        "ERROR: label point {nthpoint} is at vertex {vtxno}, not one of the surface's {} vertices",
                        s.nvertices
                    );
                    return Err(Fatal::Exit(1));
                }
                if runs && !typed {
                    g.notices.add(
                        &FS_1256,
                        "mri_label2vol --proj none: the projected points are never set (FreeSurfer uses memory contents; here 0, 0, 0)",
                    );
                    if !g.notices.reproduce() {
                        let _ = writeln!(
                            r.out,
                            "ERROR: --proj none projects nothing: use abs or frac"
                        );
                        return Err(Fatal::Exit(1));
                    }
                }
                if proj_hangs {
                    g.notices.add(
                        &FS_1257,
                        format!(
                            "mri_label2vol --proj from {} to {} by {}: the depth never passes the end (FreeSurfer never ends)",
                            g_fmt(o.proj_start),
                            g_fmt(o.proj_stop),
                            g_fmt(o.proj_delta)
                        ),
                    );
                    let _ = writeln!(
                        r.out,
                        "ERROR: --proj from {} to {} by {} never reaches its end",
                        g_fmt(o.proj_start),
                        g_fmt(o.proj_stop),
                        g_fmt(o.proj_delta)
                    );
                    return Err(Fatal::Exit(1));
                }
                let vtx = vtxno.max(0) as usize;
                let mut depth = o.proj_start;
                while depth <= o.proj_stop {
                    if o.proj_type_id == PROJ_TYPE_ABS {
                        xyz = proj_norm_dist(s, vtx, depth as f32);
                    }
                    if o.proj_type_id == PROJ_TYPE_FRAC {
                        xyz = proj_norm_frac_thick(s, vtx, depth as f32);
                    }
                    let ([c, rr, ss], oob) = get_crs(t2v, xyz[0], xyz[1], xyz[2], t);
                    if o.debug {
                        let _ = writeln!(
                            r.out,
                            "   ProjDepth {}   {} {} {} ({})  {c} {rr} {ss}   {}",
                            g_fmt(depth),
                            g_fmt(f64::from(xyz[0])),
                            g_fmt(f64::from(xyz[1])),
                            g_fmt(f64::from(xyz[2])),
                            g_fmt(f64::from(s.vertex(vtx).curv)),
                            i32::from(oob)
                        );
                    }
                    if !oob {
                        count_hit(g, &mut hitvol, c, rr, ss, nthlabel);
                        if let Some(sv) = statvol.as_mut() {
                            set_float(sv, c, rr, ss, 0, lv.stat);
                        }
                    }
                    depth = add64(depth, o.proj_delta);
                    if o.proj_delta == 0.0 {
                        break;
                    }
                }
            } else {
                let ([c, rr, ss], oob) = get_crs(t2v, lv.x, lv.y, lv.z, t);
                if o.debug {
                    let _ = writeln!(
                        r.out,
                        "   {} {} {}   {c} {rr} {ss}   {}",
                        g_fmt(f64::from(lv.x)),
                        g_fmt(f64::from(lv.y)),
                        g_fmt(f64::from(lv.z)),
                        i32::from(oob)
                    );
                }
                if oob {
                    continue;
                }
                count_hit(g, &mut hitvol, c, rr, ss, nthlabel);
                if let Some(sv) = statvol.as_mut() {
                    set_float(sv, c, rr, ss, 0, lv.stat);
                }
            }
        }
    }
    let _ = writeln!(r.out);

    if let Some(sv) = statvol.as_mut() {
        let name = o.label_stat_vol_fspec.clone().unwrap_or_default();
        if write_volume(g, r, sv, &name)? != 0 {
            return Err(Fatal::Exit(1));
        }
    }
    if let Some(h) = o.hit_vol_id.clone() {
        write_volume(g, r, &mut hitvol, &h)?;
    }
    let _ = writeln!(r.out, "PVF {}", or_null(o.pvf_vol_id.as_deref()));
    if let Some(p) = o.pvf_vol_id.clone() {
        let voxvol = mul(mul(t.vg.xsize, t.vg.ysize), t.vg.zsize);
        let _ = writeln!(r.out, "Computing PVF {}", g_fmt(f64::from(voxvol)));
        let mut pvf = multiply_const(g, r, &hitvol, f64::from(div(1.0, voxvol)))?;
        write_volume(g, r, &mut pvf, &p)?;
        g.notices.add(
            &FS_1254,
            "mri_label2vol --pvf with labels: FreeSurfer ends with status 1 after the partial-volume file and never writes the output (here: the output too, status 0)",
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(1));
        }
    }

    let mut outvol = mri_alloc_sequence(
        g,
        r.out,
        r.err,
        t.vg.width,
        t.vg.height,
        t.vg.depth,
        MRI_INT,
        1,
    )?;
    mri_copy_header(g, r.out, r.err, t, Some(&mut outvol))?;
    outvol.nframes = 1;
    let _ = writeln!(r.out, "Thesholding hit volume.");
    for c in 0..outvol.vg.width {
        for rr in 0..outvol.vg.height {
            for s in 0..outvol.vg.depth {
                let mut nhitsmax = 0i32;
                let mut nhitsmax_label = -1i32;
                for nthlabel in 0..nlabels {
                    let v = mri_get_vox_val(g, r.err, &hitvol, c, rr, s, nthlabel, 0)?;
                    let nhits = v as i32;
                    if f64::from(nhits) <= n_hits_thresh {
                        continue;
                    }
                    if nhitsmax < nhits {
                        nhitsmax = nhits;
                        nhitsmax_label = nthlabel;
                    }
                }
                let code = if nhitsmax_label == -1 {
                    0i64
                } else if o.annot_file.is_some() {
                    i64::from(nhitsmax_label) + i64::from(o.label_code_offset)
                } else {
                    i64::from(nhitsmax_label) + 1 + i64::from(o.label_code_offset)
                };
                let code = if code > i64::from(i32::MAX) || code < i64::from(i32::MIN) {
                    g.notices.add(
                        &FS_1262,
                        format!(
                            "mri_label2vol --offset {}: a label code passes the int range (FreeSurfer's wraps)",
                            o.label_code_offset
                        ),
                    );
                    if !g.notices.reproduce() {
                        let _ = writeln!(
                            r.out,
                            "ERROR: --offset {}: the label codes pass the range of an int",
                            o.label_code_offset
                        );
                        return Err(Fatal::Exit(1));
                    }
                    code as i32
                } else {
                    code as i32
                };
                set_int(&mut outvol, c, rr, s, 0, code);
            }
        }
    }
    mri_add_command_line(g, r.err, &mut outvol, cmdline.as_bytes());
    let out_name = o.out_vol_id.clone().unwrap_or_default();
    if write_volume(g, r, &mut outvol, &out_name)? != 0 {
        return Err(Fatal::Exit(1));
    }
    let _ = writeln!(r.out, "mri_label2vol done ");
    Ok(())
}
