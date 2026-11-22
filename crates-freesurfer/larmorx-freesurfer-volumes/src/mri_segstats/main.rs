// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_segstats`'s `main` (`mri_segstats.cpp` 203-1494): the entry point [`run`] (registered as
//! `mri_segstats` in the crate's `PROGRAMS`).
//!
//! The order of `main`: the version options (`handleVersionOption`), the command line kept for
//! the report (`argv2cmdline`), `uname`, `ErrorInit`, `DiagInit`; no argument: the usage (status
//! 1); the options ([`super::options`]) and their checks; the report (`dump_options`). With a
//! subject, `SUBJECTS_DIR` must be set. eTIV (`--etiv`, `--etiv-only`, `--old-etiv-only`:
//! `MRIestimateTIV` of `talairach.xfm`, or of `--talxfm`'s transform, or for the old one of
//! `talairach_with_skull.lta` with 2150); the summary file and the waveform file are opened for
//! writing and removed again (a file that does not open: status 1). `--euler`: the holes of
//! `?h.orig.nofix`. The segmentation: `--seg` (`--replace`, `--seg-erode`, `--vox`), else
//! `--annot` (the surface's annotation as a segmentation; its colour table written to a
//! temporary file and read back), else `--slabel` (the label's vertices 1), else every voxel 1 of
//! the input's size. The colour table (`--ctab` and the others, else the segmentation's own,
//! which excludes 0), `--ctab-gca`'s. The brain-volume statistics (the surface volumes,
//! `--brain-vol-from-seg`, `--supratent`, `--subcortgray`: from `stats/brainvol.stats` when
//! `mri/ribbon.mgz` exists). The input (`--reg`/`--regheader` resampled onto the segmentation;
//! `--mul`/`--div`, `--abs`, `--sqr`, `--sqrt`), the partial-volume volume (`--pv`), the mask
//! (`--mask`: binarised, eroded, applied to the segmentation). The voxel volume (or a vertex's
//! area); the ids (the segmentation's, the colour table's valid entries, or `--id`'s); for each
//! id its count, volume (with `--pv` the partial-volume corrected one) and the input's statistics
//! (`--robust`); the empty and excluded ids removed; the table (`--sum`), the colour table
//! (`--ctab-out`), the waveforms (`--avgwf`, `--sumwf`, `--sfavg`, `--avgwfvol`); `mri_segstats
//! done`.
//!
//! FreeSurfer computes the ids' statistics in an OpenMP loop: each id's are its own, so the
//! thread count never changes them, and this port runs them one after the other.
//!
//! The defects: FS-1101 and FS-1102 (names past FreeSurfer's buffers), FS-1106 (the table's
//! `MaskInvert` line), FS-1111 (`--ctab-out`, `--sfavg` files that do not open), FS-1113 (a C++
//! string of no text), FS-1114 (`--annot` without a colour table); the brain-volume statistics
//! computed from the surfaces (`ComputeBrainVolumeStats`: no cache, or `--no-cached`) are queue
//! item 3.5-36 and end the run (status 1) as not ported yet.

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_1101, FS_1102, FS_1106, FS_1113, FS_1114, FS_1115, FS_1116};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::diag::diag_init;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, Fatal, errno_of, strerror};
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::matrix_ops::matrix_print;
use larmorx_freesurfer_core::run::Settings;
use larmorx_freesurfer_core::sse::{add64, div64, mul, mul64, sub64};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::utils::{make_temp_file, resolve};
use larmorx_freesurfer_core::version::{
    argv2cmdline, get_version, handle_version_option, ver_file_time_stamp, ver_user,
};
use larmorx_freesurfer_gca::ctab_cma::gca_color_table_cma;
use larmorx_freesurfer_gca::read::gca_read;
use larmorx_freesurfer_gca::regdat::transform_lta2_reg_dat;
use larmorx_freesurfer_gca::transform::GcaLtaIo;
use larmorx_freesurfer_io::colortab::{
    ctab_copy_name, ctab_get_number_of_total_entries, ctab_get_number_of_valid_entries,
    ctab_is_entry_valid, ctab_read_ascii, ctab_rgb_at_indexi, ctab_write_file_ascii,
};
use larmorx_freesurfer_io::fio::fio_file_exists_readable;
use larmorx_freesurfer_io::identify::MRI_VOLUME_TYPE_UNKNOWN;
use larmorx_freesurfer_io::label::label_read;
use larmorx_freesurfer_io::mriio::{mri_read_header, mri_read_volume, mri_write};
use larmorx_freesurfer_io::registerio::RegisterLta;
use larmorx_freesurfer_io::streams::Io;
use larmorx_freesurfer_mri::ctab::ColorTable;
use larmorx_freesurfer_mri::lta::Lta;
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, MRI_FLOAT, MRI_INT, Mri, SAMPLE_NEAREST, mri_alloc, mri_alloc_sequence,
    mri_copy_header,
};
use larmorx_freesurfer_mri::tkreg::vox_to_vox_from_tkreg_mtx;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::annot::{annot2seg, read_annotation};
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::readwrite;
use larmorx_freesurfer_surf::toprest::compute_euler_number;
use larmorx_freesurfer_volume::arith::{Pair, mri_abs, mri_clone_by_space};
use larmorx_freesurfer_volume::cma::read_cached_brain_volume_stats;
use larmorx_freesurfer_volume::combine::{mri_dim_mismatch, mri_estimate_tiv};
use larmorx_freesurfer_volume::frames::mri_const;
use larmorx_freesurfer_volume::labels::mri_voxels_in_label_with_partial_volume_effects;
use larmorx_freesurfer_volume::ltawrite::lta_read;
use larmorx_freesurfer_volume::mriset::{mri_erode, mri_erode_segmentation, mri_replace_list};
use larmorx_freesurfer_volume::segs::{
    mri_seg_border, mri_seg_frame_avg, mri_seg_stats, mri_seg_stats_robust,
};
use larmorx_freesurfer_volume::vol2vol::mri_vol2vol_vsm;
use larmorx_freesurfer_volume::voxops::{mri_binarize, mri_seg_id_list};

use super::options::{
    ETIV_SCALE, Options, Run, RunInfo, check_options, dump_options, note_room, parse_commandline,
    usage_exit,
};
use super::qa::{cannot_open, or_null};
use super::table::{SegStat, StatSumEntry, ff, print_old_table, print_seg_stat};

/// The program's name as `argv[0]` gives it (the scripts run it by name).
pub const NAME: &str = "mri_segstats";

/// A fixed buffer a name is built into: `sprintf` into `room` bytes (FS-1102 past it), or
/// `snprintf(buf, size, ...)` into `room` bytes with its truncation's message (`<function>:
/// Truncation on line <line>`; FS-1101).
#[derive(Clone, Copy, Debug)]
pub enum Buf {
    Sprintf(usize),
    Snprintf(usize, usize, &'static str, u32),
}

/// A name built into one of FreeSurfer's fixed buffers ([`Buf`]): the whole name, or with
/// `snprintf` past its size in `reproduce` the name cut to `size - 1` bytes (and its message on
/// standard error).
pub fn name_in(g: &mut Globals, r: &mut Run, buf: Buf, text: String) -> String {
    match buf {
        Buf::Sprintf(room) => {
            note_name(g, &text, room, "a file name");
            text
        }
        Buf::Snprintf(room, size, func, line) => {
            if text.len() < size.min(room) {
                return text;
            }
            if size > room {
                note_name(g, &text, room, "a file name");
                return text;
            }
            g.notices.add(
                &FS_1101,
                format!(
                    "mri_segstats: a file name of {} bytes used whole (FreeSurfer cuts it at {} bytes)",
                    text.len(),
                    size - 1
                ),
            );
            if !g.notices.reproduce() {
                return text;
            }
            let _ = writeln!(r.err, "{func}: Truncation on line {line}");
            let mut cut = size - 1;
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            text[..cut].to_owned()
        }
    }
}

/// FS-1102: a name of `room` bytes or more (with its NUL past `room`) built with `sprintf` into
/// a buffer of `room` bytes: FreeSurfer writes past it; both modes use the whole name.
pub fn note_name(g: &mut Globals, text: &str, room: usize, what: &str) {
    if text.len() >= room {
        g.notices.add(
            &FS_1102,
            format!(
                "mri_segstats: {what} of {} bytes (FreeSurfer's buffer holds {}: it writes past it)",
                text.len(),
                room - 1
            ),
        );
    }
}

/// A `double` stored in a `float` (`cvtsd2ss`): rounded to nearest; a NaN keeps its sign and the
/// top 22 bits of its payload, quieted.
pub fn to_float(x: f64) -> f32 {
    if x.is_nan() {
        let b = x.to_bits();
        f32::from_bits(
            (((b >> 32) as u32) & 0x8000_0000) | 0x7fc0_0000 | (((b >> 29) as u32) & 0x003f_ffff),
        )
    } else {
        x as f32
    }
}

/// The `.lta` registrations of `--reg` (`regio_read_registermat`): fs.volume's `LTAread` and
/// `TransformLTA2RegDat`.
pub struct RegLta<'e> {
    pub env: &'e larmorx_freesurfer_core::run::Environment,
}

impl RegisterLta for RegLta<'_> {
    fn lta_read(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        dir: Option<&Path>,
        name: &str,
    ) -> Result<Option<Lta>, Fatal> {
        let mut io = GcaLtaIo { env: self.env };
        lta_read(g, out, err, dir, name, &mut io)
    }
    fn lta_to_reg_dat(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        lta: &mut Lta,
    ) -> Result<Option<Matrix>, Fatal> {
        transform_lta2_reg_dat(g, out, err, lta)
    }
}

/// FS-1113: a C++ string made from no text (`std::string(NULL)`): libstdc++ throws and the
/// program aborts (`reproduce`: its message, status 134); larmorx ends with a message (status 1).
fn null_string(g: &mut Globals, r: &mut Run, what: &str) -> Fatal {
    g.notices.add(
        &FS_1113,
        format!("mri_segstats: {what} is not set where the brain volume statistics are read (FreeSurfer aborts)"),
    );
    if g.notices.reproduce() {
        let _ = r.err.write_all(
            b"terminate called after throwing an instance of 'std::logic_error'\n  what():  basic_string::_M_construct null not valid\n",
        );
        return Fatal::Exit(134);
    }
    let _ = writeln!(
        r.err,
        "mri_segstats: {what} is not set: the brain volume statistics cannot be read (FreeSurfer aborts here)"
    );
    Fatal::Exit(1)
}

/// `ReadCachedBrainVolumeStats(subject, subjdir)` (FS-1113 for a missing subject or folder); the
/// statistics computed from the surfaces when the cache does not exist are not ported yet.
pub fn brain_vol_stats(
    g: &mut Globals,
    r: &mut Run,
    subject: Option<&str>,
    sdir: Option<&str>,
) -> Result<[f64; 16], Fatal> {
    let Some(subject) = subject else {
        return Err(null_string(g, r, "the subject"));
    };
    let Some(sdir) = sdir else {
        return Err(null_string(g, r, "SUBJECTS_DIR"));
    };
    let mut compute = |_g: &mut Globals, _o: &mut dyn Write, e: &mut dyn Write| {
        Err(not_ported(e))
    };
    read_cached_brain_volume_stats(g, r.out, r.err, r.dir, subject, sdir, &mut compute)
}

/// `ComputeBrainVolumeStats` (queue item 3.5-36): not ported yet (status 1).
fn not_ported(err: &mut dyn Write) -> Fatal {
    let _ = writeln!(
        err,
        "larmorx-freesurfer: mri_segstats: ComputeBrainVolumeStats (the brain volume statistics from the subject's surfaces and volumes) is not ported yet"
    );
    Fatal::Exit(1)
}

/// `mri_segstats`'s entry point: its original arguments (without the program's name), the
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
            threads: settings.threads,
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

/// Opens `name` for writing and removes it again (`fopen`, `fclose`, `unlink`): `Ok(false)` when
/// it does not open (`errno` set).
fn probe_write(g: &mut Globals, dir: Option<&Path>, name: &str) -> bool {
    let p = resolve(dir, name);
    match std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&p)
    {
        Ok(f) => {
            drop(f);
            let _ = std::fs::remove_file(&p);
            true
        }
        Err(e) => {
            g.errno = errno_of(&e).unwrap_or(0);
            false
        }
    }
}

/// `MRIread(name)`: the volume, or `None` (its messages written).
fn read(g: &mut Globals, r: &mut Run, name: &str) -> Result<Option<Mri>, Fatal> {
    mri_read_volume(g, r.out, r.err, &r.env, r.dir, name)
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

/// `MRIsegCount(seg, id, frame)`: the voxels of the frame whose `(int)` value is `id`.
pub fn mri_seg_count(
    g: &mut Globals,
    err: &mut dyn Write,
    seg: &Mri,
    id: i32,
    frame: i32,
) -> Result<i32, Fatal> {
    let mut nhits = 0i32;
    for c in 0..seg.vg.width {
        for r in 0..seg.vg.height {
            for s in 0..seg.vg.depth {
                let v = mri_get_vox_val(g, err, seg, c, r, s, frame, 0)?;
                if float_to_int(g, f64::from(v), "MRIsegCount") == id {
                    nhits = nhits.wrapping_add(1);
                }
            }
        }
    }
    Ok(nhits)
}

/// [`mri_seg_count`] of every id of frame 0 in one pass: each `(int)` value's count.
pub fn seg_counts(
    g: &mut Globals,
    err: &mut dyn Write,
    seg: &Mri,
) -> Result<std::collections::HashMap<i32, i32>, Fatal> {
    let mut counts = std::collections::HashMap::new();
    for c in 0..seg.vg.width {
        for r in 0..seg.vg.height {
            for s in 0..seg.vg.depth {
                let v = mri_get_vox_val(g, err, seg, c, r, s, 0, 0)?;
                let id = float_to_int(g, f64::from(v), "MRIsegCount");
                let k = counts.entry(id).or_insert(0i32);
                *k = k.wrapping_add(1);
            }
        }
    }
    Ok(counts)
}

/// The excluded ids' test (`DoExclSegId` and the list).
fn excluded(o: &Options, id: i32) -> bool {
    o.do_excl_seg_id && o.excl_seg_ids.contains(&id)
}

/// The eTIV of `main`: `atlas_icv (eTIV) = <int> mm^3    (det: <det> )`; `--etiv-only` and
/// `--old-etiv-only` end the program here (status 0).
fn etiv(g: &mut Globals, r: &mut Run, o: &Options, sdir: Option<&str>) -> Result<f64, Fatal> {
    let subject = or_null(o.subject.as_deref()).to_owned();
    let sdn = or_null(sdir).to_owned();
    let mut scale = ETIV_SCALE;
    let mut name = match &o.talxfm_file {
        Some(t) => name_in(g, r, Buf::Snprintf(1000, 1000, "main", 271), t.clone()),
        None => name_in(
            g,
            r,
            Buf::Snprintf(1000, 1000, "main", 282),
            format!("{sdn}/{subject}/mri/transforms/talairach.xfm"),
        ),
    };
    if o.do_old_etiv_only {
        name = name_in(
            g,
            r,
            Buf::Snprintf(1000, 1000, "main", 293),
            format!("{sdn}/{subject}/mri/transforms/talairach_with_skull.lta"),
        );
        scale = 2150.0;
    }
    let (icv, det) = {
        let mut io = GcaLtaIo { env: &r.env };
        mri_estimate_tiv(g, r.out, r.err, r.dir, &name, scale, &mut io)?
    };
    let shown = float_to_int(g, icv, "mri_segstats's eTIV");
    let _ = writeln!(
        r.out,
        "atlas_icv (eTIV) = {shown} mm^3    (det: {} )",
        cfmt::pad(&cfmt::f(det, 6), 3)
    );
    if o.do_etiv_only || o.do_old_etiv_only {
        return Err(Fatal::Exit(0));
    }
    Ok(icv)
}

/// `main(argc, argv)` on the arguments after the program's name. `Ok` is status 0.
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let (nargs, left) = handle_version_option(g, &r.env, r.out, &argv, NAME);
    if nargs != 0 && left.len() == 1 {
        return Err(Fatal::Exit(0));
    }
    let cmdline = argv2cmdline(&left);
    let uts = system::uname(&r.env);
    g.progname = NAME.to_owned();
    diag_init(g, &r.env, r.out);
    let args = &left[1..];
    if args.is_empty() {
        return Err(usage_exit(g, r));
    }
    let mut o = Options::default();
    parse_commandline(g, r, &mut o, args)?;
    check_options(r, &mut o)?;
    let info = RunInfo {
        cmdline,
        sysname: uts.as_ref().map(|u| u.sysname.clone()).unwrap_or_default(),
        hostname: uts.as_ref().map(|u| u.nodename.clone()).unwrap_or_default(),
        machine: uts.as_ref().map(|u| u.machine.clone()).unwrap_or_default(),
        user: ver_user(g, &r.env),
    };
    dump_options(r.out, &o, &info);

    let mut sdir: Option<String> = None;
    if o.subject.is_some() {
        sdir = super::qa::subjects_dir(r);
        if sdir.is_none() {
            let _ = writeln!(r.err, "ERROR: SUBJECTS_DIR not defined in environment");
            return Err(Fatal::Exit(1));
        }
    }
    let subject = o.subject.clone();
    let sdn = or_null(sdir.as_deref()).to_owned();
    let subj = or_null(subject.as_deref()).to_owned();

    let mut atlas_icv = 0.0f64;
    if o.do_etiv || o.do_etiv_only || o.do_old_etiv_only {
        atlas_icv = etiv(g, r, &o, sdir.as_deref())?;
    }
    let _ = r.out.flush();

    if let Some(name) = o.stat_table_file.clone()
        && !probe_write(g, r.dir, &name)
    {
        let _ = writeln!(r.out, "ERROR: could not open {name} for writing");
        let _ = writeln!(r.out, "Errno: {}", strerror(g.errno));
        return Err(Fatal::Exit(1));
    }
    if let Some(name) = o.frame_avg_file.clone()
        && !probe_write(g, r.dir, &name)
    {
        let _ = writeln!(r.out, "ERROR: could not open {name} for writing");
        return Err(Fatal::Exit(1));
    }

    let mut sg = SurfGlobals::default();
    let (mut lheno, mut rheno) = (0i32, 0i32);
    if o.do_euler {
        let lname = name_in(
            g,
            r,
            Buf::Snprintf(1000, 1000, "main", 355),
            format!("{sdn}/{subj}/surf/lh.orig.nofix"),
        );
        let lh = read_surf(g, &mut sg, r, &lname)?;
        let rname = name_in(g, r, Buf::Sprintf(1000), format!("{sdn}/{subj}/surf/rh.orig.nofix"));
        let rh = read_surf(g, &mut sg, r, &rname)?;
        match (&lh, &rh) {
            (Some(l), Some(rr)) => {
                let _ = writeln!(r.out, "Computing euler number");
                lheno = compute_euler_number(l).euler;
                rheno = compute_euler_number(rr).euler;
                let _ = writeln!(r.out, "orig.nofix lheno = {lheno:>4}, rheno = {rheno}");
                let _ = writeln!(
                    r.out,
                    "orig.nofix lhholes = {:>4}, rhholes = {}",
                    1 - lheno / 2,
                    1 - rheno / 2
                );
            }
            _ => {
                o.do_euler = false;
                let which = match (&lh, &rh) {
                    (None, None) => "lh.orig.nofix and rh.orig.nofix",
                    (None, _) => "lh.orig.nofix",
                    _ => "rh.orig.nofix",
                };
                let _ = writeln!(
                    r.out,
                    "Warning: error reading{which}, not computing euler number"
                );
            }
        }
    }

    // the segmentation
    let mut mris: Option<Mris> = None;
    let mut seg: Mri;
    if let Some(name) = o.seg_vol_file.clone() {
        let _ = writeln!(r.out, "Loading {name}");
        let Some(s) = read(g, r, &name)? else {
            let _ = writeln!(r.out, "ERROR: loading {name}");
            return Err(Fatal::Exit(1));
        };
        seg = s;
        if o.replace.n > 0 {
            let _ = writeln!(r.out, "Replacing {}", o.replace.n);
            let (src, trg) = o.replace.pairs();
            let (src, trg) = (src.to_vec(), trg.to_vec());
            match mri_replace_list(g, r.out, r.err, Pair::New(&seg), &src, &trg, None)? {
                larmorx_freesurfer_core::ret::Ret::New(m) => seg = m,
                _ => return Err(no_volume(g, r, "the replacements (MRIreplaceList)")),
            }
        }
        if o.n_erode_seg != 0 {
            let _ = writeln!(r.out, "Eroding seg {} times", o.n_erode_seg);
            match mri_erode_segmentation(g, r.out, r.err, Pair::New(&seg), o.n_erode_seg, 0)? {
                larmorx_freesurfer_core::ret::Ret::New(m) => seg = m,
                _ => return Err(no_volume(g, r, "the erosion (MRIerodeSegmentation)")),
            }
        }
        if o.do_vox {
            let _ = writeln!(
                r.out,
                "Replacing seg with a single voxel at {} {} {}",
                o.vox[0], o.vox[1], o.vox[2]
            );
            for c in 0..seg.vg.width {
                for rr in 0..seg.vg.height {
                    for s in 0..seg.vg.depth {
                        let v = if [c, rr, s] == o.vox { 1.0 } else { 0.0 };
                        mri_set_vox_val(g, r.err, &mut seg, c, rr, s, 0, v, 0)?;
                    }
                }
            }
        }
    } else if let Some(annot) = o.annot.clone() {
        let _ = writeln!(r.out, "Constructing seg from annotation");
        let hemi = o.hemi.clone().unwrap_or_default();
        let sname = name_in(
            g,
            r,
            Buf::Sprintf(1000),
            format!("{sdn}/{subj}/surf/{hemi}.{}", o.white_surf_name),
        );
        let Some(mut m) = read_surf(g, &mut sg, r, &sname)? else {
            return Err(Fatal::Exit(1));
        };
        let aname = if fio_file_exists_readable(g, r.dir, &annot) != 0 {
            name_in(g, r, Buf::Sprintf(1000), annot.clone())
        } else {
            name_in(
                g,
                r,
                Buf::Sprintf(1000),
                format!("{sdn}/{subj}/label/{hemi}.{annot}.annot"),
            )
        };
        let _ = writeln!(r.out, "\nReading annotation {aname}");
        let mut e = {
            let mut io = Io {
                out: &mut *r.out,
                err: &mut *r.err,
                env: &r.env,
            };
            read_annotation(g, &mut sg, &mut io, r.dir, &mut m, &aname, -1, None)?
        };
        if e != 0 {
            let _ = writeln!(r.out, " ... trying local annot");
            e = {
                let mut io = Io {
                    out: &mut *r.out,
                    err: &mut *r.err,
                    env: &r.env,
                };
                read_annotation(g, &mut sg, &mut io, r.dir, &mut m, &annot, -1, None)?
            };
            if e == 0 {
                let _ = writeln!(r.out, " Successfully read local annot\n");
            }
        }
        if e != 0 {
            return Err(Fatal::Exit(1));
        }
        if o.segbase == -1000 {
            o.segbase = match (annot.as_str(), hemi.as_str()) {
                ("aparc", "lh") => 1000,
                ("aparc", _) => 2000,
                ("aparc.a2005s", "lh") => 1100,
                ("aparc.a2005s", _) => 2100,
                _ => 0,
            };
        }
        let _ = writeln!(r.out, "Seg base {}", o.segbase);
        seg = {
            let mut io = Io {
                out: &mut *r.out,
                err: &mut *r.err,
                env: &r.env,
            };
            annot2seg(g, &mut sg, &mut io, r.dir, &m, o.segbase)?
        };
        // `mris->ct->idbase = segbase` before `if (mris->ct)`: FS-1114 without a colour table
        if m.ct.is_none() {
            g.notices.add(
                &FS_1114,
                "mri_segstats --annot: the annotation has no colour table (FreeSurfer crashes; larmorx goes on without one)",
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(139));
            }
        }
        if let Some(ct) = m.ct.as_mut() {
            ct.idbase = o.segbase;
            let tmp = make_temp_file(g, &r.env, r.dir, r.err, ".ctab")?;
            let tmp = name_in(g, r, Buf::Sprintf(1000), tmp);
            ctab_write_file_ascii(g, r.out, r.err, r.dir, ct, &tmp);
            o.ctab_file = Some(tmp);
        }
        mris = Some(m);
    } else if let Some(lfile) = o.label_file.clone() {
        let _ = writeln!(r.out, "Constructing seg from label");
        if o.use_label_thresh {
            let _ = writeln!(r.out, " Label Threshold = {}", cfmt::g(o.label_thresh, 6));
        }
        let Some(label) = label_read(g, &r.env, r.out, r.err, r.dir, None, &lfile)? else {
            return Err(Fatal::Exit(1));
        };
        let hemi = o.hemi.clone().unwrap_or_default();
        let sname = name_in(
            g,
            r,
            Buf::Sprintf(1000),
            format!("{sdn}/{subj}/surf/{hemi}.{}", o.white_surf_name),
        );
        let Some(m) = read_surf(g, &mut sg, r, &sname)? else {
            return Err(Fatal::Exit(1));
        };
        seg = mri_alloc(g, r.out, r.err, m.nvertices, 1, 1, MRI_INT)?;
        for lv in label.lv.iter().take(label.n_points.max(0) as usize) {
            if o.use_label_thresh && f64::from(lv.stat) < o.label_thresh {
                continue;
            }
            mri_set_vox_val(g, r.err, &mut seg, lv.vno, 0, 0, 0, 1.0, 0)?;
        }
        mris = Some(m);
    } else {
        let iname = o.in_vol_file.clone().unwrap_or_default();
        let _ = writeln!(r.out, "Creating a segmentation of all 1s from {iname}");
        let Some(h) = mri_read_header(g, r.out, r.err, &r.env, r.dir, &iname, MRI_VOLUME_TYPE_UNKNOWN)?
        else {
            // MRIreadHeader's NULL is read through: FreeSurfer crashes
            return Err(no_volume(g, r, "reading the input's header (MRIreadHeader)"));
        };
        let dims = [h.vg.width, h.vg.height, h.vg.depth, h.nframes];
        match mri_const(g, r.out, r.err, dims, 1.0, None)? {
            larmorx_freesurfer_core::ret::Ret::New(m) => seg = m,
            _ => return Err(no_volume(g, r, "MRIconst")),
        }
    }

    // the colour table
    let mut ctab: Option<ColorTable> = None;
    if let Some(cf) = o.ctab_file.clone() {
        let Some(ct) = ctab_read_ascii(g, r.out, r.err, r.dir, &cf) else {
            let _ = writeln!(r.out, "ERROR: reading {cf}");
            return Err(Fatal::Exit(1));
        };
        ctab = Some(ct);
    } else if let Some(ct) = seg.ct.as_ref() {
        ctab = larmorx_freesurfer_mri::ctab::ctab_deep_copy(g, r.err, Some(ct));
        let _ = writeln!(r.out, "Using embedded color table (and excluding seg 0)");
        o.excl_seg_ids.push(0);
        note_room(g, o.excl_seg_ids.len(), "excluded ids");
        o.do_excl_seg_id = true;
    }
    if let Some(gf) = o.gca_file.clone() {
        let Some(gca) = gca_read(g, r.out, r.err, r.dir, &gf)? else {
            let _ = writeln!(r.out, "ERROR: reading {gf}");
            return Err(Fatal::Exit(1));
        };
        ctab = Some(gca_color_table_cma(g, &r.env, r.out, r.err, r.dir, &gca)?);
    }

    let mut bvs: Option<[f64; 16]> = None;
    if o.do_surf_wm_vol
        || o.do_surf_ctx_vol
        || o.do_supra_tent
        || o.brain_vol_from_seg
        || o.do_sub_cort_gray_vol
    {
        let rib = name_in(g, r, Buf::Sprintf(1000), format!("{sdn}/{subj}/mri/ribbon.mgz"));
        if fio_file_exists_readable(g, r.dir, &rib) != 0 {
            if o.get_cached_brain_vol_stats {
                let _ = writeln!(r.out, "Getting Brain Volume Statistics");
                bvs = Some(brain_vol_stats(g, r, subject.as_deref(), sdir.as_deref())?);
            } else {
                let _ = writeln!(r.out, "Computing Brain Volume Statistics");
                return Err(not_ported(r.err));
            }
        } else {
            let _ = writeln!(
                r.out,
                "Warning: cannot find {rib}, not computing whole brain stats"
            );
            o.do_surf_wm_vol = false;
            o.do_surf_ctx_vol = false;
            o.do_supra_tent = false;
            o.brain_vol_from_seg = false;
            o.do_sub_cort_gray_vol = false;
            o.do_total_gray_vol = false;
        }
    }

    // the input volume
    let mut invol: Option<Mri> = None;
    if let Some(iname) = o.in_vol_file.clone() {
        let _ = writeln!(r.out, "Loading {iname}");
        let _ = r.out.flush();
        let Some(mut iv) = read(g, r, &iname)? else {
            let _ = writeln!(r.out, "ERROR: loading {iname}");
            return Err(Fatal::Exit(1));
        };
        if o.frame >= iv.nframes {
            let _ = writeln!(
                r.out,
                "ERROR: input frame = {}, input volume only has {} frames",
                o.frame, iv.nframes
            );
            return Err(Fatal::Exit(1));
        }
        if o.in_vol_reg.is_some() || o.in_vol_reg_header {
            if let Some(reg) = o.in_vol_reg.as_ref() {
                let _ = writeln!(r.out, "Input Volume Registration:");
                print_matrix(g, r, reg);
            }
            let Some(v2v) =
                vox_to_vox_from_tkreg_mtx(g, r.out, r.err, &iv.vg, &seg.vg, o.in_vol_reg.as_ref())?
            else {
                return Err(no_volume(g, r, "the registration's voxel matrix"));
            };
            let _ = writeln!(r.out, "Input Volume vox2vox:");
            print_matrix(g, r, &v2v);
            let _ = writeln!(r.out, "Allocating {} frames", iv.nframes);
            let mut tmp = mri_clone_by_space(g, r.out, r.err, &seg, -1, iv.nframes)?;
            let _ = writeln!(r.out, "Vol2Vol");
            let e = mri_vol2vol_vsm(
                g,
                r.out,
                r.err,
                &iv,
                &mut tmp,
                Some(&v2v),
                SAMPLE_NEAREST,
                -1.0,
                None,
                None,
                2,
            )?;
            if e != 0 {
                return Err(Fatal::Exit(1));
            }
            iv = tmp;
        }
        if mri_dim_mismatch(&iv, &seg, false) != 0 {
            let _ = writeln!(r.out, "ERROR: dimension mismatch between input volume and seg");
            let _ = writeln!(r.out, "  input {} {} {}", iv.vg.width, iv.vg.height, iv.vg.depth);
            let _ = writeln!(r.out, "  seg   {} {} {}", seg.vg.width, seg.vg.height, seg.vg.depth);
            return Err(Fatal::Exit(1));
        }
        if o.do_multiply {
            let _ = writeln!(r.out, "Multiplying input by {}", cfmt::f(o.mult_val, 6));
            multiply_const_in_place(g, r.err, &mut iv, o.mult_val)?;
        }
        if o.do_abs {
            let _ = writeln!(r.out, "Computing absolute value of input");
            mri_abs(g, r.out, r.err, Pair::Same(&mut iv))?;
        }
        if o.do_square {
            let _ = writeln!(r.out, "Computing square of input");
            square_in_place(g, r, &mut iv, false)?;
        }
        if o.do_square_root {
            let _ = writeln!(r.out, "Computing square root of input");
            square_in_place(g, r, &mut iv, true)?;
        }
        invol = Some(iv);
    }

    // the partial-volume volume
    let mut pvvol: Option<Mri> = None;
    if let Some(pname) = o.pv_vol_file.clone() {
        let _ = writeln!(r.out, "Loading {pname}");
        let _ = r.out.flush();
        let Some(pv) = read(g, r, &pname)? else {
            let _ = writeln!(r.out, "ERROR: loading {pname}");
            return Err(Fatal::Exit(1));
        };
        if mri_dim_mismatch(&pv, &seg, false) != 0 {
            let _ = writeln!(r.out, "ERROR: dimension mismatch between PV volume and seg");
            let _ = writeln!(r.out, "  pvvol {} {} {}", pv.vg.width, pv.vg.height, pv.vg.depth);
            let _ = writeln!(r.out, "  seg   {} {} {}", seg.vg.width, seg.vg.height, seg.vg.depth);
            return Err(Fatal::Exit(1));
        }
        pvvol = Some(pv);
    }

    // the mask
    if let Some(mname) = o.mask_vol_file.clone() {
        let _ = writeln!(r.out, "Loading {mname}");
        let _ = r.out.flush();
        let Some(mut mv) = read(g, r, &mname)? else {
            let _ = writeln!(r.out, "ERROR: loading {mname}");
            return Err(Fatal::Exit(1));
        };
        if o.mask_frame >= mv.nframes {
            let _ = writeln!(
                r.out,
                "ERROR: mask frame = {}, mask volume only has {} frames",
                o.mask_frame, mv.nframes
            );
            return Err(Fatal::Exit(1));
        }
        if mri_dim_mismatch(&mv, &seg, false) != 0 {
            let _ = writeln!(r.out, "ERROR: dimension mismatch between brain volume and seg");
            return Err(Fatal::Exit(1));
        }
        let sign = o.mask_sign.clone().unwrap_or_default();
        let mut nmaskhits = 0i32;
        mri_binarize(
            g,
            r.out,
            r.err,
            Pair::Same(&mut mv),
            o.mask_thresh,
            Some(sign.as_bytes()),
            o.mask_invert,
            &mut nmaskhits,
        )?;
        if nmaskhits == 0 {
            let _ = writeln!(r.out, "WARNING: no voxels in mask meet thresholding criteria.");
            let _ = writeln!(r.out, "The output table will be empty.");
            let _ = writeln!(
                r.out,
                "thresh = {}, sign = {sign}, inv = {}",
                cfmt::g(f64::from(o.mask_thresh), 6),
                i32::from(o.mask_invert)
            );
        }
        let _ = writeln!(r.out, "There were {nmaskhits} voxels in the orginal mask");
        if o.mask_erode > 0 {
            let _ = writeln!(r.out, "Eroding {} voxels in 3d", o.mask_erode);
            for _ in 0..o.mask_erode {
                mri_erode(g, r.out, r.err, Pair::Same(&mut mv))?;
            }
        }
        for c in 0..seg.vg.width {
            for rr in 0..seg.vg.height {
                for s in 0..seg.vg.depth {
                    let v = mri_get_vox_val(g, r.err, &mv, c, rr, s, o.mask_frame, 0)?;
                    if float_to_int(g, f64::from(v), "mri_segstats's mask") == 0 {
                        mri_set_vox_val(g, r.err, &mut seg, c, rr, s, 0, 0.0, 0)?;
                    }
                }
            }
        }
    }

    let voxelvolume: f32 = match &mris {
        None => {
            let v = mul(mul(seg.vg.xsize, seg.vg.ysize), seg.vg.zsize);
            let _ = writeln!(r.out, "Voxel Volume is {} mm^3", cfmt::g(f64::from(v), 6));
            v
        }
        Some(m) => {
            let v = if m.group_avg_surface_area > 0.0 {
                larmorx_freesurfer_core::sse::div(m.group_avg_surface_area, m.nvertices as f32)
            } else {
                larmorx_freesurfer_core::sse::div(m.total_area, m.nvertices as f32)
            };
            let _ = writeln!(r.out, "Vertex Area is {} mm^3", cfmt::g(f64::from(v), 6));
            v
        }
    };

    let _ = writeln!(r.out, "Generating list of segmentation ids");
    let _ = r.out.flush();
    let segidlist0 = mri_seg_id_list(g, r.err, &seg, 0)?;

    let mut table: Vec<StatSumEntry>;
    if ctab.is_none() && o.n_user_seg_ids == 0 {
        table = segidlist0
            .iter()
            .map(|&id| StatSumEntry {
                id,
                ..Default::default()
            })
            .collect();
    } else if let Some(ct) = ctab.as_ref() {
        if o.n_user_seg_ids == 0 {
            let nvalid = ctab_get_number_of_valid_entries(ct);
            let ntotal = ctab_get_number_of_total_entries(ct);
            table = Vec::with_capacity(nvalid.max(0) as usize);
            for n in 0..ntotal {
                if !ctab_is_entry_valid(g, r.err, ct, n).unwrap_or(false) {
                    continue;
                }
                table.push(ctab_entry(g, r, ct, n));
            }
        } else {
            table = Vec::with_capacity(o.n_user_seg_ids);
            // `int valid;`: an id outside the table leaves it as the last id set it (FS-1116)
            let mut valid: Option<bool> = None;
            for &id in o.user_ids() {
                match ctab_is_entry_valid(g, r.err, ct, id) {
                    Ok(v) => valid = Some(v),
                    Err(_) => {
                        g.notices.add(
                            &FS_1116,
                            format!("mri_segstats --id {id}: outside the colour table (FreeSurfer decides with the last id's answer, or memory contents for the first)"),
                        );
                        valid = if g.notices.reproduce() { valid } else { Some(false) };
                    }
                }
                if !valid.unwrap_or(false) {
                    let _ = writeln!(
                        r.out,
                        "ERROR: cannot find seg id {id} in {}",
                        or_null(o.ctab_file.as_deref())
                    );
                    return Err(Fatal::Exit(1));
                }
                table.push(ctab_entry(g, r, ct, id));
            }
        }
    } else {
        table = o
            .user_ids()
            .iter()
            .map(|&id| StatSumEntry {
                id,
                ..Default::default()
            })
            .collect();
    }

    let _ = writeln!(r.out, "Found {:>3} segmentations", table.len());
    if table.is_empty() {
        let _ = writeln!(r.out, "ERROR: no segmentations to report");
        return Err(Fatal::Exit(1));
    }
    let _ = writeln!(r.out, "Computing statistics for each segmentation");
    let _ = r.out.flush();

    let mut maxlabel = 0i32;
    let mut segborder: Option<Mri> = None;
    if pvvol.is_some() && o.use_seg_border {
        let (b, m) = mri_seg_border(g, r.out, r.err, &seg, 1)?;
        segborder = Some(b);
        maxlabel = m;
    }

    // MRIsegCount of every id at once: one pass over the voxels gives each id's count, the same
    // numbers as a pass per id
    let counts = if !o.dontrun && mris.is_none() {
        seg_counts(g, r.err, &seg)?
    } else {
        std::collections::HashMap::new()
    };
    let ctx = IdCtx {
        o: &o,
        counts: &counts,
        seg: &seg,
        pvvol: pvvol.as_ref(),
        segborder: segborder.as_ref(),
        invol: invol.as_ref(),
        mris: mris.as_ref(),
        voxelvolume,
        maxlabel,
    };
    let todo: Vec<usize> = (0..table.len())
        .filter(|&n| !excluded(&o, table[n].id) && segidlist0.contains(&table[n].id))
        .collect();
    let ids: Vec<i32> = table.iter().map(|e| e.id).collect();
    let results = per_id(g, &ctx, &ids, &todo, r.threads);
    for (n, res) in todo.iter().zip(results) {
        let res = res?;
        let _ = r.out.write_all(&res.out);
        let _ = r.err.write_all(&res.err);
        g.notices.extend(res.notices);
        let e = &mut table[*n];
        e.nhits = res.nhits;
        e.vol = res.vol;
        if let Some(st) = res.stats {
            e.min = st[0];
            e.max = st[1];
            e.range = st[2];
            e.mean = st[3];
            e.std = st[4];
            e.snr = st[5];
        }
        if let Some(f) = res.fatal {
            return Err(f);
        }
    }

    for (n, e) in table.iter().enumerate() {
        if excluded(&o, e.id) {
            continue;
        }
        if g.gdiag_no > 1 {
            let _ = r.out.write_all(format!("{n:>3}   {:>3}  ", e.id).as_bytes());
            let pad = 33usize.saturating_sub(e.name.len());
            let _ = r.out.write_all(&b" ".repeat(pad));
            let _ = r.out.write_all(&e.name);
            let _ = writeln!(r.out, "  {:>6}  {}", e.nhits, ff(e.vol, 10, 3));
            let _ = r.out.flush();
        }
    }
    let _ = writeln!(r.out);

    if o.non_empty_only || o.do_excl_seg_id {
        table.retain(|e| !(o.non_empty_only && e.nhits == 0) && !excluded(&o, e.id));
    }
    let _ = writeln!(r.out, "Reporting on {:>3} segmentations", table.len());

    if o.debug {
        for e in &table {
            let _ = write!(r.out, "{:>3}  {:>8} {}  ", e.id, e.nhits, ff(e.vol, 10, 1));
            if ctab.is_some() {
                let _ = r.out.write_all(&e.name);
                let pad = 30usize.saturating_sub(e.name.len());
                let _ = r.out.write_all(&b" ".repeat(pad));
                let _ = r.out.write_all(b" ");
            }
            if invol.is_some() {
                let _ = write!(
                    r.out,
                    "{} {} {} {} {} ",
                    ff(e.min, 10, 4),
                    ff(e.max, 10, 4),
                    ff(e.range, 10, 4),
                    ff(e.mean, 10, 4),
                    ff(e.std, 10, 4)
                );
                if o.do_snr {
                    let _ = write!(r.out, "{} ", ff(e.snr, 10, 4));
                }
            }
            let _ = writeln!(r.out);
        }
    }

    if let Some(name) = o.stat_table_file.clone() {
        let mut fp: Vec<u8> = Vec::new();
        let ctx = Header {
            o: &o,
            info: &info,
            mris: mris.is_some(),
            sdir: sdir.as_deref(),
            bvs: bvs.as_ref(),
            voxelvolume,
            atlas_icv,
            lheno,
            rheno,
        };
        header(g, r, &mut fp, &ctx);
        let s = SegStat {
            entry: &table,
            is_surf: mris.is_some(),
            use_name: ctab.is_some(),
            do_intensity: o.in_vol_file.is_some(),
            in_intensity_name: &o.in_intensity_name,
            in_intensity_units: &o.in_intensity_units,
            do_snr: o.do_snr,
        };
        if o.use_print_seg_stat {
            let _ = writeln!(r.out, "Using PrintSegStat");
            print_seg_stat(&mut fp, &s);
        } else {
            let _ = writeln!(r.out, "Not using PrintSegStat");
            print_old_table(&mut fp, &s);
        }
        let _ = std::fs::write(resolve(r.dir, &name), &fp);
    }

    if let Some(name) = o.ctab_file_out.clone() {
        let mut text: Vec<u8> = Vec::new();
        for e in &table {
            let _ = write!(text, "{} ", e.id);
            text.extend_from_slice(&e.name);
            text.extend(std::iter::repeat_n(b' ', 30usize.saturating_sub(e.name.len())));
            let _ = writeln!(text, " {:>3} {:>3} {:>3} 0", e.red, e.green, e.blue);
        }
        if std::fs::write(resolve(r.dir, &name), &text).is_err() {
            cannot_open(g, r, "--ctab-out", &name)?;
        }
    }

    if o.do_frame_avg {
        frame_averages(g, r, &o, &seg, invol.as_ref(), &table)?;
    }

    let _ = writeln!(r.out, "mri_segstats done");
    Ok(())
}

/// What each id's statistics read (shared by the threads).
struct IdCtx<'a> {
    o: &'a Options,
    /// Each id's `MRIsegCount(seg, id, 0)` ([`seg_counts`]).
    counts: &'a std::collections::HashMap<i32, i32>,
    seg: &'a Mri,
    pvvol: Option<&'a Mri>,
    segborder: Option<&'a Mri>,
    invol: Option<&'a Mri>,
    mris: Option<&'a Mris>,
    voxelvolume: f32,
    maxlabel: i32,
}

/// One id's results: count, volume, the input's statistics (minimum, maximum, range, mean,
/// standard deviation, signal-to-noise ratio), what its functions printed and noted, and the end
/// of the program when one of them ended it.
#[derive(Default)]
struct IdResult {
    nhits: i32,
    vol: f32,
    stats: Option<[f32; 6]>,
    out: Vec<u8>,
    err: Vec<u8>,
    notices: Vec<larmorx_core::defects::Notice>,
    fatal: Option<Fatal>,
}

/// The loop body of `main`'s OpenMP loop over the ids (`mri_segstats.cpp` 865-939) for the
/// `n`-th id: its count and volume (`MRIsegCount` and the voxel volume; with `--pv`
/// `MRIvoxelsInLabelWithPartialVolumeEffects`; on a surface the vertices' areas), and with an
/// input its statistics (`MRIsegStats`, or `MRIsegStatsRobust` with `--robust`; 0 for an empty
/// id). `--dontrun`: the count is `n`.
fn id_stats(g: &mut Globals, c: &IdCtx, id: i32, n: usize) -> IdResult {
    let mut res = IdResult::default();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let r = id_stats_wkr(g, c, id, n, &mut out, &mut err, &mut res);
    if let Err(f) = r {
        res.fatal = Some(f);
    }
    res.out = out;
    res.err = err;
    res.notices = g.notices.as_slice().to_vec();
    res
}

fn id_stats_wkr(
    g: &mut Globals,
    c: &IdCtx,
    id: i32,
    n: usize,
    out: &mut Vec<u8>,
    err: &mut Vec<u8>,
    res: &mut IdResult,
) -> Result<(), Fatal> {
    let o = c.o;
    let (nhits, vol): (i32, f32);
    if !o.dontrun {
        match c.mris {
            None => match c.pvvol {
                None => {
                    nhits = c.counts.get(&id).copied().unwrap_or(0);
                    vol = mul(nhits as f32, c.voxelvolume);
                }
                Some(pv) => {
                    vol = mri_voxels_in_label_with_partial_volume_effects(
                        g,
                        out,
                        err,
                        c.seg,
                        Some(pv),
                        id,
                        None,
                        None,
                        c.maxlabel + 1,
                        c.segborder,
                    )?;
                    nhits = c.counts.get(&id).copied().unwrap_or(0);
                }
            },
            Some(m) => {
                let (mut nh, mut v) = (0i32, 0.0f32);
                let verts = m.vertices.as_deref().unwrap_or(&[]);
                for k in 0..m.nvertices {
                    let sv = mri_get_vox_val(g, err, c.seg, k, 0, 0, 0, 0)?;
                    if sv == id as f32 {
                        nh += 1;
                        let vx = &verts[k as usize];
                        let a = if m.group_avg_vtxarea_loaded != 0 {
                            vx.group_avg_area
                        } else {
                            vx.area
                        };
                        v = larmorx_freesurfer_core::sse::add(v, a);
                    }
                }
                nhits = nh;
                vol = v;
            }
        }
    } else {
        nhits = n as i32;
        vol = 0.0;
    }
    res.nhits = nhits;
    res.vol = vol;
    if let Some(iv) = c.invol
        && !o.dontrun
    {
        let st = if nhits > 0 {
            let (_, st) = if !o.use_robust {
                mri_seg_stats(g, err, c.seg, id, iv, o.frame)?
            } else {
                mri_seg_stats_robust(g, err, c.seg, id, iv, o.frame, o.robust_pct)?
            };
            let snr = larmorx_freesurfer_core::sse::div(st.mean, st.std);
            let mean = if o.do_accumulate {
                mul(st.mean, nhits as f32)
            } else {
                st.mean
            };
            [st.min, st.max, st.range, mean, st.std, snr]
        } else {
            [0.0; 6]
        };
        res.stats = Some(st);
    }
    Ok(())
}

/// [`id_stats`] of each id in `todo` (indices into `ids`), on `threads` threads (0: every
/// logical CPU), each id with its own copy of the globals; the results in `todo`'s order, so that
/// what they print and note is FreeSurfer's one-thread order. `Err` only for a thread that
/// panicked.
fn per_id(
    g: &Globals,
    c: &IdCtx,
    ids: &[i32],
    todo: &[usize],
    threads: usize,
) -> Vec<Result<IdResult, Fatal>> {
    let procs = std::thread::available_parallelism().map_or(1, |p| p.get());
    let nt = if threads == 0 { procs } else { threads }.clamp(1, todo.len().max(1));
    if nt == 1 {
        return todo
            .iter()
            .map(|&n| Ok(id_stats(&mut g.clone(), c, ids[n], n)))
            .collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut slots: Vec<Option<IdResult>> = (0..todo.len()).map(|_| None).collect();
    // each thread its own copy of the globals (they are not shared), each id a copy of that
    let bases: Vec<Globals> = (0..nt).map(|_| g.clone()).collect();
    let next = &next;
    let done: Vec<Vec<(usize, IdResult)>> = std::thread::scope(|s| {
        let handles: Vec<_> = bases
            .into_iter()
            .map(|base| {
                s.spawn(move || {
                    let mut mine = Vec::new();
                    loop {
                        let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if k >= todo.len() {
                            break;
                        }
                        let n = todo[k];
                        mine.push((k, id_stats(&mut base.clone(), c, ids[n], n)));
                    }
                    mine
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_default())
            .collect()
    });
    for (k, res) in done.into_iter().flatten() {
        slots[k] = Some(res);
    }
    slots
        .into_iter()
        .map(|x| x.ok_or(Fatal::Exit(134)))
        .collect()
}

/// A colour table's entry as a row: its id, name (at most 999 bytes) and colour.
fn ctab_entry(g: &mut Globals, r: &mut Run, ct: &ColorTable, n: i32) -> StatSumEntry {
    let name = ctab_copy_name(g, r.err, ct, n, 1000).unwrap_or_default();
    let (red, green, blue) = ctab_rgb_at_indexi(g, r.err, ct, n).unwrap_or((0, 0, 0));
    StatSumEntry {
        id: n,
        name,
        red,
        green,
        blue,
        ..Default::default()
    }
}

/// `MatrixPrint(stdout, m)`.
fn print_matrix(g: &mut Globals, r: &mut Run, m: &Matrix) {
    let mut text = Vec::new();
    matrix_print(g, Some(&mut text), r.out, r.err, Some(m), 5);
    let _ = r.out.write_all(&text);
}

/// FS-1025's kind for this program: a step that gives no volume where `main` goes on with one
/// (FreeSurfer crashes): status 1 with a message (`reproduce`: 139).
fn no_volume(g: &mut Globals, r: &mut Run, step: &str) -> Fatal {
    g.notices.add(
        &FS_1115,
        format!("mri_segstats: {step} gave no volume (FreeSurfer goes on with none and crashes)"),
    );
    if g.notices.reproduce() {
        return Fatal::Exit(139);
    }
    let _ = writeln!(r.err, "mri_segstats: {step} gave no volume (FreeSurfer crashes here)");
    Fatal::Exit(1)
}

/// `MRImultiplyConst(vol, vconst, vol)`: each voxel times `vconst` in `double`, in place.
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
                    mri_set_vox_val(g, err, vol, c, r, s, f, to_float(mul64(v, vconst)), 0)?;
                }
            }
        }
    }
    Ok(())
}

/// `MRIsquare(vol, NULL, vol)` (`val * val`) or `MRIsquareRoot(vol, NULL, vol)`
/// (`sqrt(fabs(val))`) in place: each voxel and frame read (`double`) and written back.
fn square_in_place(g: &mut Globals, r: &mut Run, vol: &mut Mri, root: bool) -> Result<(), Fatal> {
    for c in 0..vol.vg.width {
        for rr in 0..vol.vg.height {
            for s in 0..vol.vg.depth {
                for f in 0..vol.nframes {
                    let v = mri_get_vox_val(g, r.err, vol, c, rr, s, f, 0)?;
                    // cvtss2sd: a signalling NaN quieted
                    let val = if v.is_nan() {
                        f64::from(f32::from_bits(v.to_bits() | 0x0040_0000))
                    } else {
                        f64::from(v)
                    };
                    let o = if root {
                        larmorx_freesurfer_core::sse::sqrt64(val.abs())
                    } else {
                        mul64(val, val)
                    };
                    mri_set_vox_val(g, r.err, vol, c, rr, s, f, to_float(o), 0)?;
                }
            }
        }
    }
    Ok(())
}

/// What the table's header reads besides the options.
struct Header<'a> {
    o: &'a Options,
    info: &'a RunInfo,
    mris: bool,
    sdir: Option<&'a str>,
    bvs: Option<&'a [f64; 16]>,
    voxelvolume: f32,
    atlas_icv: f64,
    lheno: i32,
    rheno: i32,
}

/// `%f` of a `double`.
fn f6(v: f64) -> String {
    cfmt::f(v, 6)
}

/// The summary file's header (`mri_segstats.cpp` 1078-1267).
fn header(g: &mut Globals, r: &mut Run, fp: &mut Vec<u8>, h: &Header) {
    let o = h.o;
    let _ = fp.write_all(b"# Title Segmentation Statistics \n# \n");
    let _ = writeln!(fp, "# generating_program {NAME}");
    let _ = writeln!(fp, "# cvs_version {}", get_version());
    let _ = writeln!(fp, "# cmdline {}", h.info.cmdline);
    let _ = writeln!(fp, "# sysname  {}", h.info.sysname);
    let _ = writeln!(fp, "# hostname {}", h.info.hostname);
    let _ = writeln!(fp, "# machine  {}", h.info.machine);
    let _ = writeln!(fp, "# user     {}", h.info.user);
    if h.mris {
        let _ = writeln!(fp, "# anatomy_type surface");
    } else {
        let _ = writeln!(fp, "# anatomy_type volume");
    }
    let _ = writeln!(fp, "# ");
    if let Some(subject) = &o.subject {
        let _ = writeln!(fp, "# SUBJECTS_DIR {}", or_null(h.sdir));
        let _ = writeln!(fp, "# subjectname {subject}");
    }
    if o.use_robust {
        let _ = writeln!(fp, "# RobustPct {}", cfmt::g(f64::from(o.robust_pct), 6));
    }
    if h.bvs.is_some() {
        if (f64::from(h.voxelvolume) - 1.0).abs() > 0.01 {
            let _ = writeln!(
                fp,
                "# BrainVolStatsFixed see surfer.nmr.mgh.harvard.edu/fswiki/BrainVolStatsFixed"
            );
        } else {
            let _ = writeln!(fp, "# BrainVolStatsFixed-NotNeeded because voxelvolume=1mm3");
        }
    }
    let bv = |i: usize| h.bvs.map(|b| b[i]).unwrap_or(0.0);
    if o.brain_vol_from_seg {
        let _ = writeln!(
            fp,
            "# Measure BrainSeg, BrainSegVol, Brain Segmentation Volume, {}, mm^3",
            f6(bv(0))
        );
        let _ = writeln!(
            fp,
            "# Measure BrainSegNotVent, BrainSegVolNotVent, Brain Segmentation Volume Without Ventricles, {}, mm^3",
            f6(bv(1))
        );
    }
    if h.bvs.is_some() {
        let _ = writeln!(
            fp,
            "# Measure VentricleChoroidVol, VentricleChoroidVol, Volume of ventricles and choroid plexus, {}, mm^3",
            f6(bv(15))
        );
    }
    if o.do_surf_ctx_vol {
        let _ = writeln!(fp, "# Measure lhCortex, lhCortexVol, Left hemisphere cortical gray matter volume, {}, mm^3", f6(bv(5)));
        let _ = writeln!(fp, "# Measure rhCortex, rhCortexVol, Right hemisphere cortical gray matter volume, {}, mm^3", f6(bv(6)));
        let _ = writeln!(fp, "# Measure Cortex, CortexVol, Total cortical gray matter volume, {}, mm^3", f6(bv(7)));
    }
    if o.do_surf_wm_vol {
        let _ = writeln!(fp, "# Measure lhCerebralWhiteMatter, lhCerebralWhiteMatterVol, Left hemisphere cerebral white matter volume, {}, mm^3", f6(bv(9)));
        let _ = writeln!(fp, "# Measure rhCerebralWhiteMatter, rhCerebralWhiteMatterVol, Right hemisphere cerebral white matter volume, {}, mm^3", f6(bv(10)));
        let _ = writeln!(fp, "# Measure CerebralWhiteMatter, CerebralWhiteMatterVol, Total cerebral white matter volume, {}, mm^3", f6(bv(11)));
    }
    if o.do_sub_cort_gray_vol {
        let _ = writeln!(fp, "# Measure SubCortGray, SubCortGrayVol, Subcortical gray matter volume, {}, mm^3", f6(bv(4)));
    }
    if o.do_total_gray_vol {
        let _ = writeln!(fp, "# Measure TotalGray, TotalGrayVol, Total gray matter volume, {}, mm^3", f6(bv(8)));
    }
    if o.do_supra_tent {
        let _ = writeln!(fp, "# Measure SupraTentorial, SupraTentorialVol, Supratentorial volume, {}, mm^3", f6(bv(2)));
        let _ = writeln!(fp, "# Measure SupraTentorialNotVent, SupraTentorialVolNotVent, Supratentorial volume, {}, mm^3", f6(bv(3)));
    }
    if o.brain_mask_file.is_some() && h.bvs.is_some() {
        let _ = writeln!(fp, "# Measure Mask, MaskVol, Mask Volume, {}, mm^3", f6(bv(12)));
    }
    if o.do_etiv && o.brain_vol_from_seg {
        let _ = writeln!(fp, "# Measure BrainSegVol-to-eTIV, BrainSegVol-to-eTIV, Ratio of BrainSegVol to eTIV, {}, unitless", f6(div64(bv(0), h.atlas_icv)));
        let _ = writeln!(fp, "# Measure MaskVol-to-eTIV, MaskVol-to-eTIV, Ratio of MaskVol to eTIV, {}, unitless", f6(div64(bv(12), h.atlas_icv)));
    }
    if o.do_euler {
        let (lh, rh) = (1 - h.lheno / 2, 1 - h.rheno / 2);
        let _ = writeln!(fp, "# Measure lhSurfaceHoles, lhSurfaceHoles, Number of defect holes in lh surfaces prior to fixing, {lh}, unitless");
        let _ = writeln!(fp, "# Measure rhSurfaceHoles, rhSurfaceHoles, Number of defect holes in rh surfaces prior to fixing, {rh}, unitless");
        let _ = writeln!(fp, "# Measure SurfaceHoles, SurfaceHoles, Total number of defect holes in surfaces prior to fixing, {}, unitless", lh + rh);
    }
    if o.do_etiv {
        let _ = writeln!(fp, "# Measure EstimatedTotalIntraCranialVol, eTIV, Estimated Total Intracranial Volume, {}, mm^3", f6(h.atlas_icv));
    }
    if o.do_stiv {
        let _ = writeln!(fp, "# Measure SegmentedTotalIntraCranialVol, sTIV, Segmented Total Intracranial Volume, {}, mm^3", f6(o.stiv));
    }
    let stamp = |g: &mut Globals, r: &mut Run, name: &str| -> String {
        let _ = g;
        ver_file_time_stamp(&r.env, r.dir, r.out, name).unwrap_or_else(|| "(null)".to_owned())
    };
    if let Some(sv) = &o.seg_vol_file {
        let _ = writeln!(fp, "# SegVolFile {sv} ");
        let t = stamp(g, r, sv);
        let _ = writeln!(fp, "# SegVolFileTimeStamp  {t} ");
    }
    if let Some(a) = &o.annot {
        let _ = writeln!(
            fp,
            "# Annot {} {} {a}",
            or_null(o.subject.as_deref()),
            or_null(o.hemi.as_deref())
        );
    }
    if let Some(l) = &o.label_file {
        let _ = writeln!(
            fp,
            "# Label {} {} {l}",
            or_null(o.subject.as_deref()),
            or_null(o.hemi.as_deref())
        );
    }
    if let Some(c) = &o.ctab_file {
        let _ = writeln!(fp, "# ColorTable {c} ");
        let t = stamp(g, r, c);
        let _ = writeln!(fp, "# ColorTableTimeStamp {t} ");
    }
    if let Some(gf) = &o.gca_file {
        let _ = writeln!(fp, "# ColorTableFromGCA {gf} ");
        let t = stamp(g, r, gf);
        let _ = writeln!(fp, "# GCATimeStamp {t} ");
    }
    if let Some(m) = &o.mask_vol_file {
        let _ = writeln!(fp, "# MaskVolFile  {m} ");
        let t = stamp(g, r, m);
        let _ = writeln!(fp, "#   MaskVolFileTimeStamp  {t} ");
        let _ = writeln!(fp, "#   MaskThresh {} ", f6(f64::from(o.mask_thresh)));
        let _ = writeln!(fp, "#   MaskSign   {} ", or_null(o.mask_sign.as_deref()));
        let _ = writeln!(fp, "#   MaskFrame  {} ", o.mask_frame);
        // FS-1106: FreeSurfer prints the mask's frame here
        if o.mask_frame != i32::from(o.mask_invert) {
            g.notices.add(
                &FS_1106,
                "mri_segstats --mask: the summary's MaskInvert line gives --maskinvert (FreeSurfer's gives the mask's frame)",
            );
        }
        let shown = if g.notices.reproduce() {
            o.mask_frame
        } else {
            i32::from(o.mask_invert)
        };
        let _ = writeln!(fp, "#   MaskInvert {shown} ");
    }
    if let Some(i) = &o.in_vol_file {
        let _ = writeln!(fp, "# InVolFile  {i} ");
        let t = stamp(g, r, i);
        let _ = writeln!(fp, "# InVolFileTimeStamp  {t} ");
        let _ = writeln!(fp, "# InVolFrame {} ", o.frame);
    }
    if let Some(p) = &o.pv_vol_file {
        let _ = writeln!(fp, "# PVVolFile  {p} ");
        let t = stamp(g, r, p);
        let _ = writeln!(fp, "# PVVolFileTimeStamp  {t} ");
    }
    if o.do_excl_ctx_gm_wm {
        let _ = writeln!(fp, "# Excluding Cortical Gray and White Matter");
    }
    if o.do_excl_seg_id {
        let _ = write!(fp, "# ExcludeSegId ");
        for x in &o.excl_seg_ids {
            let _ = write!(fp, "{x} ");
        }
        let _ = writeln!(fp);
    }
    if o.non_empty_only {
        let _ = writeln!(fp, "# Only reporting non-empty segmentations");
    }
    if !h.mris {
        let _ = writeln!(fp, "# VoxelVolume_mm3 {} ", cfmt::g(f64::from(h.voxelvolume), 6));
    } else {
        let _ = writeln!(fp, "# VertexArea_mm2 {} ", cfmt::g(f64::from(h.voxelvolume), 6));
    }
}

/// The waveforms (`DoFrameAvg`, 1424-1490): each segmentation's frame averages
/// (`MRIsegFrameAvg`; times the voxel count with `--sumwf`), their mean over the frames, the mean
/// removed (`--avgwf-remove-mean`) or scaled to `--avgwf-norm-mean`; `--sfavg`'s means (`%g`),
/// `--avgwf`'s table (`%11.5f ` per segmentation, a row per frame), `--avgwfvol`'s volume (a
/// `float` volume of one voxel per segmentation with the input's header).
fn frame_averages(
    g: &mut Globals,
    r: &mut Run,
    o: &Options,
    seg: &Mri,
    invol: Option<&Mri>,
    table: &[StatSumEntry],
) -> Result<(), Fatal> {
    let Some(invol) = invol else {
        return Ok(());
    };
    let _ = writeln!(r.out, "Computing spatial average of each frame");
    let nf = invol.nframes.max(0) as usize;
    let mut favg: Vec<Vec<f64>> = Vec::with_capacity(table.len());
    let mut favgmn: Vec<f64> = Vec::with_capacity(table.len());
    for (n, e) in table.iter().enumerate() {
        if o.debug {
            let _ = write!(r.out, "{n:>3}");
            if n % 20 == 19 {
                let _ = writeln!(r.out);
            }
            let _ = r.out.flush();
        }
        let (nvox, mut fa) = mri_seg_frame_avg(g, r.err, seg, e.id, invol)?;
        fa.resize(nf, 0.0);
        let mut mn = 0.0f64;
        for v in fa.iter_mut() {
            if o.do_frame_sum {
                *v = mul64(*v, f64::from(nvox));
            }
            mn = add64(mn, *v);
        }
        mn = div64(mn, f64::from(invol.nframes));
        if o.rm_frame_avg_mn {
            for v in fa.iter_mut() {
                *v = sub64(*v, mn);
            }
        }
        if o.norm_frame_avg_mn != 0.0 {
            let k = div64(o.norm_frame_avg_mn, mn);
            for v in fa.iter_mut() {
                *v = mul64(*v, k);
            }
        }
        favg.push(fa);
        favgmn.push(mn);
    }
    let _ = writeln!(r.out);

    if let Some(name) = o.spat_frame_avg_file.clone() {
        let _ = writeln!(r.out, "Writing to {name}");
        let mut text = String::new();
        let mut shown = String::new();
        for (n, mn) in favgmn.iter().enumerate() {
            text.push_str(&format!("{}\n", cfmt::g(*mn, 6)));
            shown.push_str(&format!("{n} {}\n", cfmt::g(*mn, 6)));
        }
        if std::fs::write(resolve(r.dir, &name), text.as_bytes()).is_err() {
            cannot_open(g, r, "--sfavg", &name)?;
        }
        let _ = r.out.write_all(shown.as_bytes());
    }

    if let Some(name) = o.frame_avg_file.clone() {
        let _ = writeln!(r.out, "Writing to {name}");
        let mut text = String::new();
        for f in 0..nf {
            for fa in &favg {
                text.push_str(&cfmt::pad(&cfmt::f(fa[f], 5), 11));
                text.push(' ');
            }
            text.push('\n');
        }
        let _ = std::fs::write(resolve(r.dir, &name), text.as_bytes());
    }

    if let Some(name) = o.frame_avg_vol_file.clone() {
        if table.is_empty() {
            let _ = writeln!(r.out, "ERROR: no voxels found in segmentation");
            return Err(Fatal::Exit(1));
        }
        let _ = writeln!(r.out, "Writing to {name}");
        let mut famri = mri_alloc_sequence(
            g,
            r.out,
            r.err,
            table.len() as i32,
            1,
            1,
            MRI_FLOAT,
            invol.nframes,
        )?;
        mri_copy_header(g, r.out, r.err, invol, Some(&mut famri))?;
        for f in 0..nf {
            for (n, fa) in favg.iter().enumerate() {
                mri_set_vox_val(g, r.err, &mut famri, n as i32, 0, 0, f as i32, to_float(fa[f]), 0)?;
            }
        }
        mri_write(g, r.out, r.err, &r.env, r.dir, &mut famri, &name, None, MGZ_INTENT_UNKNOWN)?;
    }
    Ok(())
}
