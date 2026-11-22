// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_mask`'s program (`mri_mask/mri_mask.cpp` 89-655): the entry point [`run`] (registered as
//! `mri_mask` in the crate's `PROGRAMS`), [`main`] and the options ([`get_option`]).
//!
//! `mri_mask [options] <in vol> <mask vol> <out vol>`. The version options first; the options
//! while the next argument starts with `-` (their names compared without regard to case after
//! the dash: `--help`, `--usage`, `-version`, `-samseg`, `-abs`, `-invert`, `-no-invert`, `-lh`,
//! `-rh`, `-no_cerebellum`, `-dilate n`, `-erode n`, `-xform f`, `-T t` or `-threshold t`,
//! `-crop-to-fov c r s`, `-crop-to-fov-mm c r s`, `-BB n` (`-crop`, `-boundingbox`), `-crop-crs`,
//! `-BBM` (`-boundingboxm`) `c r s`, `-BBMM` (`-boundingboxmm`) six pads, `-BBEQ n`,
//! `-no-allow-diff-geom`, `-transfer v`, `-oval v`, `-lta_src f` (`-src`), `-lta_dst f`
//! (`-dst`), `-keep_mask_deletion_edits`, `-DEBUG_VOXEL x y z`, `-bgnoise scale sign`; anything
//! else `unknown option` and the help, status 1); then exactly three arguments (else `Incorrect
//! number of arguments, argc = <n>` and the help, status 1). The input and the mask read
//! (`MRIread`); the mask inverted and binarised (`-invert`), moved by the transform (`-xform`:
//! `TransformRead`, an array reduced to one transform, its geometry completed by `-lta_src` and
//! `-lta_dst`, made voxel to voxel; inverted when its geometry matches the other way round;
//! applied with nearest-neighbour sampling), thresholded (`-T`), made from a segmentation
//! (`-samseg`, `-lh`, `-rh`), the volumes cropped (`-BB`, `-BBEQ`, `-crop-to-fov`), the mask
//! dilated and eroded; `MRImask` with the mask value (0, or `-transfer`'s) and the output value;
//! `-keep_mask_deletion_edits` (the original mask's voxels of 1 set to 1); `-bgnoise` (the
//! C library's `drand48` outside the mask); the input's colour table kept; the output written.
//!
//! The defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-mri-mask.md`):
//! - **FS-1132** (crash): an option given last (or before too few arguments) whose number or file
//!   FreeSurfer reads past the arguments (`atoi`, `atof`, `sscanf` or `MRIreadHeader` of a
//!   missing argument: status 139). By default `<option> needs <n> values` and the help, status 1.
//! - **FS-1133** (crash): `-BBEQ` whose box cannot be made square with an odd number of slices
//!   (`REGIONgetBoundingBoxEqOdd`'s `ERROR` lines): FreeSurfer prints the region it does not have
//!   and crashes (status 139). By default an error, status 1.
//! - **FS-1134** (crash): `-xform` with a transform array whose transforms do not chain
//!   (`LTAreduce`'s `ERROR` line): FreeSurfer crashes (status 139). By default an error, status 1.
//! - **FS-1135** (crash or a late error): `-xform` whose voxel-to-voxel transform for the two
//!   volumes does not invert in single precision (`MRIlinearTransform: xform is singular`):
//!   FreeSurfer goes on without a mask, crashing (status 139) where a later step reads it (`-T`,
//!   `-invert`, `-samseg`, `-lh`, `-rh`, the crops, `-dilate`), else ending in `MRImask` (`must
//!   have a valid MRI (src)`, status 252; `-erode`'s messages before it). By default an error at
//!   once, status 1.

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::cscan::{sscanf_f64, sscanf_i32};
use larmorx_core::defects::{FS_1132, FS_1133, FS_1134, FS_1135};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::cscan;
use larmorx_freesurfer_core::cstd::{atof, atoi};
use larmorx_freesurfer_core::diag::diag_init;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::help_xml::output_help_xml;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_NOFILE, Fatal, error_exit};
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::utils::stricmp;
use larmorx_freesurfer_core::version::{get_version, handle_version_option};
use larmorx_freesurfer_gca::transform::{
    Xform, transform_apply_type, transform_get_dst_vol_geom, transform_get_src_vol_geom,
    transform_invert_replace, transform_read,
};
use larmorx_freesurfer_io::identify::MRI_VOLUME_TYPE_UNKNOWN;
use larmorx_freesurfer_io::mriio::{mri_read_header, mri_read_volume, mri_write};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::ctab::ctab_deep_copy;
use larmorx_freesurfer_mri::lta::{LINEAR_VOX_TO_VOX, MORPH_3D_TYPE};
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_UNKNOWN, Mri, MriRegion, mri_free};
use larmorx_freesurfer_mri::vol_geom::VolGeom;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{Pair, mri_clone, mri_extract_region};
use larmorx_freesurfer_volume::cma::{
    Air, CSF, CSF_ExtraCerebral, Head_ExtraCerebral, Skull, Unknown, is_cerebellum, is_lh_class,
    is_rh_class,
};
use larmorx_freesurfer_volume::coords::SAMPLE_NEAREST;
use larmorx_freesurfer_volume::ltaapply::lta_modify_src_dst_geom;
use larmorx_freesurfer_volume::ltaops::{lta_change_type, lta_reduce};
use larmorx_freesurfer_volume::mri2ops::mri_crop_to_fov;
use larmorx_freesurfer_volume::mriset::{
    NEAREST_NEIGHBOR_CORNER, mri_dilate, mri_erode_nn, mri_mask,
};
use larmorx_freesurfer_volume::region::{
    region_get_bounding_box_eq_odd, region_get_bounding_box_m, region_print,
};
use larmorx_freesurfer_volume::vgeom::{get_vol_geom, vg_is_equal};

/// The program's name as `argv[0]` gives it (recon-all runs it by name).
pub const NAME: &str = "mri_mask";

/// `mri_mask.help.xml`, the text `usage` renders (`outputHelpXml`).
pub const HELP_XML: &[u8] = include_bytes!("mri_mask.help.xml");

/// glibc's buffer of a standard output that is a pipe or a file: what a crash loses.
const STDOUT_BUFFER: usize = 4096;
/// The status of a process ended by `SIGSEGV`, as a shell reports it.
const SEGFAULT: i32 = 139;

/// The settings the options give (the file-scope variables of `mri_mask.cpp`).
#[derive(Default)]
pub struct Options {
    pub dilate: i32,
    pub erode: i32,
    pub lta_src: Option<Mri>,
    pub lta_dst: Option<Mri>,
    pub no_cerebellum: bool,
    pub do_lh: bool,
    pub do_rh: bool,
    pub samseg: bool,
    pub out_val: f32,
    pub xform_fname: Option<String>,
    pub threshold: f32,
    pub threshold_set: bool,
    pub do_transfer: bool,
    pub transfer_val: f32,
    pub keep_mask_deletion_edits: bool,
    pub do_abs: bool,
    pub do_bb: bool,
    pub npad_bb: [i32; 6],
    pub do_bb_eq: bool,
    pub bgnoisescale: f64,
    pub bgnoisesign: i32,
    pub invert_mask: bool,
    pub crop_to_fov: [f64; 3],
    pub do_crop_to_fov: bool,
    pub do_crop_to_fov_mm: bool,
}

impl Options {
    fn new() -> Options {
        Options {
            threshold: -1e10,
            ..Options::default()
        }
    }
}

/// Standard output as the C library buffers it: kept until the end, with what a crash would keep.
/// The buffer is written out when it fills and at each flush: an explicit one (`fflush(stdout)`:
/// [`Write::flush`]) and each message of the C++ error stream (`std::cerr` is tied to
/// `std::cout`, which writes through the C library's buffer: `ErrorPrintf`, `ErrorExit`).
pub struct Cout {
    pub buf: Vec<u8>,
    /// What was flushed (written out) at the last flush.
    flushed: usize,
    /// The C++ error stream's message count at the last look.
    seen: usize,
    cxx: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Cout {
    /// An empty standard output for `g`'s streams.
    pub fn new(g: &Globals) -> Cout {
        Cout {
            buf: Vec::new(),
            flushed: 0,
            seen: g.stderr.cxx_messages(),
            cxx: g.stderr.cxx_messages.clone(),
        }
    }

    /// A message on the C++ error stream since the last look flushed what was written before it.
    fn sync(&mut self) {
        let n = self.cxx.load(std::sync::atomic::Ordering::Relaxed);
        if n != self.seen {
            self.seen = n;
            self.flushed = self.buf.len();
        }
    }

    /// What a crash now keeps: the flushed part and the full buffers written after it.
    pub fn kept_by_crash(&mut self) -> usize {
        self.sync();
        self.flushed + (self.buf.len() - self.flushed) / STDOUT_BUFFER * STDOUT_BUFFER
    }
}

impl Write for Cout {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.sync();
        self.buf.extend_from_slice(b);
        Ok(b.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flushed = self.buf.len();
        Ok(())
    }
}

/// The program's streams, environment and working directory. Standard output is kept in `out`
/// until the end (FreeSurfer's buffer: a crash in `reproduce` loses what it would).
pub struct Run<'a, 'b> {
    pub out: &'a mut Cout,
    pub err: &'a mut dyn Write,
    /// The environment, changed by `-no-allow-diff-geom` (`setenv`).
    pub env: Environment,
    pub dir: Option<&'b std::path::Path>,
}

/// `mri_mask`'s entry point: its original arguments (without the program's name), the settings,
/// the two streams; the exit status. The notices of the run follow on standard error.
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut g = Globals::new(settings.mode);
    g.stderr.read_write = system::stderr_read_write();
    let mut cout = Cout::new(&g);
    let status = {
        let mut terr = log::TrackedErr::new(&g, &mut *err);
        let mut r = Run {
            out: &mut cout,
            err: &mut terr,
            env: settings.env.clone(),
            dir: settings.dir.as_deref(),
        };
        match main(&mut g, &mut r, args) {
            Ok(()) => 0,
            Err(f) => f.status(),
        }
    };
    let keep = if i32::from(status) == SEGFAULT && g.notices.reproduce() {
        cout.kept_by_crash()
    } else {
        cout.buf.len()
    };
    let _ = out.write_all(&cout.buf[..keep]);
    let _ = out.flush();
    g.notices.print(err);
    status
}

/// `usage(exit_val)`: the help, then `exit(exit_val)`.
fn usage(g: &mut Globals, r: &mut Run, code: i32) -> Fatal {
    output_help_xml(g, r.out, r.err, Some(HELP_XML));
    Fatal::Exit(code)
}

/// `get_option`'s option names in the order it compares them (the second `invert`, the
/// transform's, is never reached: the mask's comes first).
const OPTION_NAMES: [&str; 36] = [
    "-help",
    "-usage",
    "version",
    "samseg",
    "abs",
    "invert",
    "no-invert",
    "lh",
    "no_cerebellum",
    "rh",
    "dilate",
    "erode",
    "xform",
    "T",
    "threshold",
    "crop-to-fov",
    "crop-to-fov-mm",
    "BB",
    "crop",
    "boundingbox",
    "crop-crs",
    "BBM",
    "boundingboxm",
    "BBMM",
    "boundingboxmm",
    "BBEQ",
    "no-allow-diff-geom",
    "transfer",
    "oval",
    "lta_src",
    "src",
    "lta_dst",
    "dst",
    "keep_mask_deletion_edits",
    "DEBUG_VOXEL",
    "bgnoise",
];

/// FS-1132: `n` values of option `arg` read where `rest` (the option and what follows) has fewer.
fn need(g: &mut Globals, r: &mut Run, rest: &[String], n: usize) -> Result<(), Fatal> {
    if rest.len() > n {
        return Ok(());
    }
    let arg = &rest[0];
    g.notices.add(
        &FS_1132,
        format!(
            "mri_mask: option '{arg}' without its {n} value(s): FreeSurfer reads past its arguments and crashes"
        ),
    );
    if g.notices.reproduce() {
        return Err(Fatal::Exit(SEGFAULT));
    }
    let _ = writeln!(r.err, "{NAME}: option {arg} needs {n} value(s)");
    Err(usage(g, r, 1))
}

/// `atoi(argv[k])` of `rest[k - 1]`.
fn int_at(rest: &[String], k: usize) -> i32 {
    atoi(rest[k].as_bytes())
}

/// `get_option(argc, argv)` on `rest` (`argv[1]` and what follows): the arguments it takes after
/// its own.
pub fn get_option(
    g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    rest: &[String],
) -> Result<usize, Fatal> {
    let arg = &rest[0];
    let option = &arg[1..];
    // the names compared in the C's order (`stricmp`); the first that matches decides
    let matched = OPTION_NAMES
        .iter()
        .copied()
        .find(|n| stricmp(g, option, n) == 0);
    let is = |name: &str| matched == Some(name);
    let mut nargs = 0;
    if is("-help") || is("-usage") {
        return Err(usage(g, r, 1));
    } else if is("version") {
        let _ = writeln!(r.out, "{}", get_version());
        return Err(Fatal::Exit(1));
    } else if is("samseg") {
        o.samseg = true;
        let _ = writeln!(r.out, "masking nonbrain using samseg segmentation");
    } else if is("abs") {
        o.do_abs = true;
    } else if is("invert") {
        o.invert_mask = true;
    } else if is("no-invert") {
        o.invert_mask = false;
    } else if is("lh") {
        o.do_lh = true;
    } else if is("no_cerebellum") {
        o.no_cerebellum = true;
    } else if is("rh") {
        o.do_rh = true;
    } else if is("dilate") {
        need(g, r, rest, 1)?;
        o.dilate = int_at(rest, 1);
        nargs = 1;
        let _ = writeln!(r.out, "dilating mask {} times before applying", o.dilate);
    } else if is("erode") {
        need(g, r, rest, 1)?;
        o.erode = int_at(rest, 1);
        nargs = 1;
    } else if is("xform") {
        // a missing name is the NULL after the last argument: printed as `(null)`
        o.xform_fname = rest.get(1).cloned();
        nargs = 1;
        let _ = writeln!(
            r.err,
            "transform file name is {}",
            o.xform_fname.as_deref().unwrap_or("(null)")
        );
    } else if is("T") || is("threshold") {
        need(g, r, rest, 1)?;
        o.threshold = atof(rest[1].as_bytes()) as f32;
        nargs = 1;
        let _ = writeln!(
            r.err,
            "threshold mask volume at {}",
            cfmt::g(f64::from(o.threshold), 6)
        );
        o.threshold_set = true;
    } else if is("crop-to-fov") || is("crop-to-fov-mm") {
        need(g, r, rest, 3)?;
        for k in 0..3 {
            o.crop_to_fov[k] = f64::from(int_at(rest, k + 1));
        }
        if is("crop-to-fov") {
            o.do_crop_to_fov = true;
        } else {
            o.do_crop_to_fov_mm = true;
            let _ = writeln!(
                r.out,
                "CropToFoVmm {} {} {}",
                cfmt::g(o.crop_to_fov[0], 6),
                cfmt::g(o.crop_to_fov[1], 6),
                cfmt::g(o.crop_to_fov[2], 6)
            );
        }
        nargs = 3;
    } else if is("BB") || is("crop") || is("boundingbox") {
        need(g, r, rest, 1)?;
        o.npad_bb = [int_at(rest, 1); 6];
        o.do_bb = true;
        nargs = 1;
        let _ = writeln!(r.out, "cropping npad = {}", o.npad_bb[0]);
    } else if is("crop-crs") || is("BBM") || is("boundingboxm") {
        need(g, r, rest, 3)?;
        let (c, rr, s) = (int_at(rest, 1), int_at(rest, 2), int_at(rest, 3));
        o.npad_bb = [c, c, rr, rr, s, s];
        o.do_bb = true;
        nargs = 3;
        if is("crop-crs") {
            let _ = writeln!(
                r.out,
                "cropping npad  {} {} {}",
                o.npad_bb[0], o.npad_bb[1], o.npad_bb[2]
            );
        } else {
            let _ = writeln!(
                r.out,
                "bounding box M npad = {}, {}, {}",
                o.npad_bb[0], o.npad_bb[2], o.npad_bb[4]
            );
        }
    } else if is("BBMM") || is("boundingboxmm") {
        need(g, r, rest, 6)?;
        for k in 0..6 {
            o.npad_bb[k] = int_at(rest, k + 1);
        }
        o.do_bb = true;
        nargs = 6;
        let p = o.npad_bb;
        let _ = writeln!(
            r.out,
            "bounding box MM npad = {}, {}, {}, {}, {}, {}",
            p[0], p[1], p[2], p[3], p[4], p[5]
        );
    } else if is("BBEQ") {
        need(g, r, rest, 1)?;
        o.npad_bb[0] = int_at(rest, 1);
        o.do_bb_eq = true;
        nargs = 1;
        let _ = writeln!(r.out, "bounding box eq npad = {}", o.npad_bb[0]);
    } else if is("no-allow-diff-geom") {
        r.env.set("FS_MRIMASK_ALLOW_DIFF_GEOM", "0");
    } else if is("transfer") {
        need(g, r, rest, 1)?;
        o.do_transfer = true;
        o.transfer_val = atof(rest[1].as_bytes()) as f32;
        nargs = 1;
        let _ = writeln!(
            r.err,
            "transfer mask voxels={} to dst vol",
            cfmt::g(f64::from(o.transfer_val), 6)
        );
    } else if is("oval") {
        need(g, r, rest, 1)?;
        o.out_val = atof(rest[1].as_bytes()) as f32;
        nargs = 1;
        let _ = writeln!(
            r.err,
            "setting masked output voxels to {} instead of 0",
            cfmt::f(f64::from(o.out_val), 1)
        );
    } else if is("lta_src") || is("src") || is("lta_dst") || is("dst") {
        let src = is("lta_src") || is("src");
        let name = rest.get(1).map_or("(null)", String::as_str);
        if src {
            let _ = writeln!(
                r.err,
                "src volume for the given transform (given by -xform) is {name}"
            );
            let _ = writeln!(r.err, "Reading the src volume...");
        } else {
            let _ = writeln!(
                r.err,
                "dst volume for the transform (given by -xform) is {name}"
            );
            let _ = writeln!(r.err, "Reading the dst volume...");
        }
        need(g, r, rest, 1)?;
        let m = mri_read_header(
            g,
            r.out,
            r.err,
            &r.env,
            r.dir,
            &rest[1],
            MRI_VOLUME_TYPE_UNKNOWN,
        )?;
        let Some(m) = m else {
            error_exit(
                g,
                r.err,
                ERROR_BADPARM,
                &format!("Could not read file {}\n", rest[1]),
            )?;
            unreachable!("ErrorExit returns only for code 0");
        };
        if src {
            o.lta_src = Some(m);
        } else {
            o.lta_dst = Some(m);
        }
        nargs = 1;
    } else if is("keep_mask_deletion_edits") {
        o.keep_mask_deletion_edits = true;
        let _ = writeln!(r.err, "Transferring mask edits ('1' voxels) to dst vol");
    } else if is("DEBUG_VOXEL") {
        need(g, r, rest, 3)?;
        g.gx = int_at(rest, 1);
        g.gy = int_at(rest, 2);
        g.gz = int_at(rest, 3);
        nargs = 3;
        let _ = writeln!(r.out, "debugging node ({}, {}, {})", g.gx, g.gy, g.gz);
    } else if is("bgnoise") {
        need(g, r, rest, 2)?;
        if let Some(v) = sscanf_f64(rest[1].as_bytes()) {
            o.bgnoisescale = v;
        }
        if let Some(v) = sscanf_i32(rest[2].as_bytes()) {
            o.bgnoisesign = v;
        }
        nargs = 2;
    } else {
        let _ = writeln!(r.err, "unknown option {arg}");
        return Err(usage(g, r, 1));
    }
    Ok(nargs)
}

/// A geometry as `getVolGeom` copies it.
fn vol_geom_of(m: &Mri) -> VolGeom {
    let mut vg = VolGeom::default();
    get_vol_geom(&m.vg, &mut vg);
    vg
}

/// `main(argc, argv)` on the arguments after the program's name. `Ok` is status 0.
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let env0 = r.env.clone();
    let (nargs, argv) = handle_version_option(g, &env0, r.out, &argv, NAME);
    if nargs != 0 && argv.len() == 1 {
        return Err(Fatal::Exit(0));
    }
    g.progname = NAME.to_owned();
    diag_init(g, &env0, r.out);
    let mut o = Options::new();
    let mut argc = argv.len() as isize;
    let mut at = 1usize;
    while argc > 1 && argv[at].starts_with('-') {
        let nargs = get_option(g, r, &mut o, &argv[at..])?;
        argc -= nargs as isize + 1;
        at += nargs + 1;
    }
    if argc != 4 {
        let _ = writeln!(r.out, "Incorrect number of arguments, argc = {argc}");
        return Err(usage(g, r, 1));
    }
    let progname = g.progname.clone();
    let (in_name, mask_name, out_name) = (&argv[at], &argv[at + 1], &argv[at + 2]);
    let Some(mut mri_in) = mri_read_volume(g, r.out, r.err, &r.env, r.dir, in_name)? else {
        error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!("{progname}: could not read source volume {in_name}"),
        )?;
        unreachable!("ErrorExit returns only for code 0");
    };
    let ctab = match &mri_in.ct {
        Some(ct) => ctab_deep_copy(g, r.err, Some(ct)),
        None => None,
    };
    let Some(mut mask) = mri_read_volume(g, r.out, r.err, &r.env, r.dir, mask_name)? else {
        error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!("{progname}: could not read mask volume {mask_name}"),
        )?;
        unreachable!("ErrorExit returns only for code 0");
    };
    if o.invert_mask {
        let thresh = if o.threshold_set {
            f64::from(o.threshold)
        } else {
            0.5
        };
        let _ = writeln!(
            r.out,
            "Inverting and binarizing mask thresh = {}",
            cfmt::g(thresh, 6)
        );
        let mut count = 0i32;
        for c in 0..mask.vg.width {
            for rr in 0..mask.vg.height {
                for s in 0..mask.vg.depth {
                    let mut v = f64::from(mri_get_vox_val(g, r.err, &mask, c, rr, s, 0, 0)?);
                    if o.do_abs {
                        v = v.abs();
                    }
                    if v < thresh {
                        mri_set_vox_val(g, r.err, &mut mask, c, rr, s, 0, 1.0, 0)?;
                        count += 1;
                    } else {
                        mri_set_vox_val(g, r.err, &mut mask, c, rr, s, 0, 0.0, 0)?;
                    }
                }
            }
        }
        let _ = writeln!(r.out, "count {count}");
        let _ = writeln!(r.out, "Resetting threshold to 0.5");
        o.threshold = 0.5;
        o.threshold_set = true;
    }
    let _ = writeln!(r.out, "DoAbs = {}", i32::from(o.do_abs));
    if let Some(xform_fname) = o.xform_fname.clone()
        && xform_fname != "identity.nofile"
    {
        match apply_xform(g, r, &o, &xform_fname, &mri_in, mask)? {
            Some(m) => mask = m,
            None => return null_mask(g, r, &mut o, &xform_fname),
        }
    }
    o.lta_src = None;
    o.lta_dst = None;
    let mask_orig = mri_copy(g, r.out, r.err, &mask, None)?.unwrap_new();
    if o.threshold_set {
        let mut nmask = 0i32;
        let (w, h, d) = (mask.vg.width, mask.vg.height, mask.vg.depth);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let mut value = mri_get_vox_val(g, r.err, &mask, x, y, z, 0, 0)?;
                    if o.do_abs {
                        value = value.abs();
                    }
                    if value <= o.threshold {
                        mri_set_vox_val(g, r.err, &mut mask, x, y, z, 0, 0.0, 0)?;
                    } else {
                        mri_set_vox_val(g, r.err, &mut mask, x, y, z, 0, 1.0, 0)?;
                        nmask += 1;
                    }
                }
            }
        }
        print_found(r, nmask, &mask);
    }
    if o.samseg {
        let mut nmask = 0i32;
        let (w, h, d) = (mask.vg.width, mask.vg.height, mask.vg.depth);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let value = mri_get_vox_val(g, r.err, &mask, x, y, z, 0, 0)?;
                    let label = float_to_int(g, f64::from(value), "mri_mask -samseg");
                    if [
                        Unknown,
                        CSF,
                        Skull,
                        Air,
                        Head_ExtraCerebral,
                        CSF_ExtraCerebral,
                    ]
                    .contains(&label)
                    {
                        mri_set_vox_val(g, r.err, &mut mask, x, y, z, 0, 0.0, 0)?;
                    } else {
                        mri_set_vox_val(g, r.err, &mut mask, x, y, z, 0, 1.0, 0)?;
                        nmask += 1;
                    }
                }
            }
        }
        print_found(r, nmask, &mask);
    }
    if o.do_lh || o.do_rh {
        // the mask is a segmentation: made a mask of the hemisphere's labels
        let aseg = mask;
        mask = mri_clone(g, r.out, r.err, &aseg, None)?.unwrap_new();
        let (w, h, d) = (mask.vg.width, mask.vg.height, mask.vg.depth);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let v = mri_get_vox_val(g, r.err, &aseg, x, y, z, 0, 0)?;
                    let mut label = float_to_int(g, f64::from(v), "mri_mask -lh/-rh");
                    if is_cerebellum(label) && o.no_cerebellum {
                        label = 0;
                    }
                    let one = (o.do_lh && is_lh_class(label)) || (o.do_rh && is_rh_class(label));
                    let val = if one { 1.0 } else { 0.0 };
                    mri_set_vox_val(g, r.err, &mut mask, x, y, z, 0, val, 0)?;
                }
            }
        }
    }
    if o.do_bb || o.do_bb_eq || o.do_crop_to_fov || o.do_crop_to_fov_mm {
        let region = crop_region(g, r, &o, &mask)?;
        let Some(region) = region else {
            // `REGIONprint(stdout, NULL)`: FreeSurfer reads address 0
            g.notices.add(
                &FS_1133,
                "mri_mask -BBEQ: no square bounding box with an odd number of slices: FreeSurfer prints the region it does not have and crashes".to_owned(),
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(SEGFAULT));
            }
            let _ = writeln!(
                r.err,
                "{progname}: ERROR: -BBEQ found no bounding box with as many columns as rows and an odd number of slices"
            );
            return Err(Fatal::Exit(1));
        };
        region_print(r.out, &region);
        let Some(m) =
            mri_extract_region(g, r.out, r.err, &mut mask, None, Some(&region))?.into_new()
        else {
            return Err(Fatal::Exit(1));
        };
        mask = m;
        let Some(m) =
            mri_extract_region(g, r.out, r.err, &mut mri_in, None, Some(&region))?.into_new()
        else {
            return Err(Fatal::Exit(1));
        };
        mri_in = m;
    }
    let mut maskval = 0i32;
    if o.do_transfer {
        maskval = float_to_int(g, f64::from(o.transfer_val), "mri_mask -transfer");
        o.out_val = o.transfer_val;
    }
    if o.dilate > 0 {
        let _ = writeln!(r.out, "Dilating {}", o.dilate);
        for _ in 0..o.dilate {
            mri_dilate(g, r.out, r.err, Pair::Same(&mut mask))?;
        }
    }
    if o.erode > 0 {
        let _ = writeln!(r.out, "Eroding {}", o.erode);
        let mut tmp: Option<Mri> = None;
        for _ in 0..o.erode {
            match tmp.as_mut() {
                None => {
                    tmp = mri_erode_nn(
                        g,
                        r.out,
                        r.err,
                        Pair::New(&mask),
                        NEAREST_NEIGHBOR_CORNER,
                        0,
                    )?
                    .into_new();
                }
                Some(t) => {
                    mri_erode_nn(
                        g,
                        r.out,
                        r.err,
                        Pair::Given(&mask, t),
                        NEAREST_NEIGHBOR_CORNER,
                        0,
                    )?;
                }
            }
            if let Some(t) = tmp.as_ref() {
                mri_copy(g, r.out, r.err, t, Some(&mut mask))?;
            }
        }
    }
    let _ = writeln!(
        r.out,
        "maskval={maskval}, outval={}",
        cfmt::g(f64::from(o.out_val), 6)
    );
    let env = r.env.clone();
    let Some(mut mri_out) = mri_mask(
        g,
        r.out,
        r.err,
        &env,
        Pair::New(&mri_in),
        &mask,
        maskval,
        o.out_val,
    )?
    .into_new() else {
        let _ = writeln!(r.out, "{progname}: stripping failed");
        return Err(Fatal::Exit(1));
    };
    if o.keep_mask_deletion_edits {
        let Some(t) = mri_mask(
            g,
            r.out,
            r.err,
            &env,
            Pair::New(&mri_out),
            &mask_orig,
            1,
            1.0,
        )?
        .into_new() else {
            let _ = writeln!(
                r.out,
                "{progname}: stripping failed on keep_mask_deletion_edits"
            );
            return Err(Fatal::Exit(1));
        };
        mri_out = t;
    }
    if o.bgnoisescale > 0.0 {
        let _ = writeln!(
            r.out,
            "Adding background noise {} {}",
            cfmt::g(o.bgnoisescale, 6),
            o.bgnoisesign
        );
        let (w, h, d) = (mask.vg.width, mask.vg.height, mask.vg.depth);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let mv = mri_get_vox_val(g, r.err, &mask, x, y, z, 0, 0)?;
                    if float_to_int(g, f64::from(mv), "mri_mask -bgnoise") != 0 {
                        continue;
                    }
                    let u = g.drand48.drand48();
                    let v = (u - f64::from(o.bgnoisesign) / 2.0) * o.bgnoisescale;
                    mri_set_vox_val(g, r.err, &mut mri_out, x, y, z, 0, v as f32, 0)?;
                }
            }
        }
    }
    if let Some(ct) = &ctab {
        mri_out.ct = ctab_deep_copy(g, r.err, Some(ct));
    }
    let _ = write!(r.out, "Writing masked volume to {out_name}...");
    let status = mri_write(
        g,
        r.out,
        r.err,
        &env,
        r.dir,
        &mut mri_out,
        out_name,
        None,
        MGZ_INTENT_UNKNOWN,
    )?;
    if status != 0 {
        return Err(Fatal::Exit(1));
    }
    let _ = writeln!(r.out, "done.");
    Ok(())
}

/// `Found <n> voxels in mask (pct=<p>)` (`%6.2f` of `100.0 n / (width height depth)`, the
/// product an `int`).
fn print_found(r: &mut Run, nmask: i32, mask: &Mri) {
    let total = mask
        .vg
        .width
        .wrapping_mul(mask.vg.height)
        .wrapping_mul(mask.vg.depth);
    let pct = 100.0 * f64::from(nmask) / f64::from(total);
    let _ = writeln!(
        r.out,
        "Found {nmask} voxels in mask (pct={})",
        cfmt::pad(&cfmt::f(pct, 2), 6)
    );
}

/// The crop's region: `-BB` (and its forms), `-BBEQ`, `-crop-to-fov`, `-crop-to-fov-mm`, each
/// later one replacing the earlier; `None` where `REGIONgetBoundingBoxEqOdd` gives none.
fn crop_region(
    g: &mut Globals,
    r: &mut Run,
    o: &Options,
    mask: &Mri,
) -> Result<Option<MriRegion>, Fatal> {
    let mut region = None;
    if o.do_bb {
        let p = o.npad_bb;
        let _ = writeln!(
            r.out,
            "Computing bounding box, npad = {}, {}, {}, {}, {}, {}",
            p[0], p[1], p[2], p[3], p[4], p[5]
        );
        region = Some(region_get_bounding_box_m(g, r.err, mask, p)?);
    }
    if o.do_bb_eq {
        let _ = writeln!(
            r.out,
            "Computing bounding box with ncols=nrows, npad = {}",
            o.npad_bb[0]
        );
        region = region_get_bounding_box_eq_odd(g, r.out, r.err, mask, o.npad_bb[0])?;
    }
    if o.do_crop_to_fov {
        let fov = o.crop_to_fov.map(|v| v as i32);
        region = Some(mri_crop_to_fov(
            g,
            r.out,
            r.err,
            mask,
            fov,
            f64::from(o.threshold),
        )?);
    }
    if o.do_crop_to_fov_mm {
        let sizes = [mask.vg.xsize, mask.vg.ysize, mask.vg.zsize];
        let mut fov = [0i32; 3];
        for k in 0..3 {
            let c = (o.crop_to_fov[k] / f64::from(sizes[k])).ceil();
            fov[k] = float_to_int(g, c, "mri_mask -crop-to-fov-mm");
        }
        region = Some(mri_crop_to_fov(
            g,
            r.out,
            r.err,
            mask,
            fov,
            f64::from(o.threshold),
        )?);
    }
    Ok(region)
}

/// `-xform`: the transform read and applied to the mask (nearest neighbour), as the module's
/// text says; the moved mask, `None` where `TransformApplyType` gives none (FS-1135).
fn apply_xform(
    g: &mut Globals,
    r: &mut Run,
    o: &Options,
    xform_fname: &str,
    mri_in: &Mri,
    mask: Mri,
) -> Result<Option<Mri>, Fatal> {
    let progname = g.progname.clone();
    let env = r.env.clone();
    let _ = writeln!(r.out, "Applying transform to the mask volume");
    let Some(mut transform) = transform_read(g, r.out, r.err, &env, r.dir, xform_fname)? else {
        error_exit(
            g,
            r.err,
            ERROR_NOFILE,
            &format!("{progname}: could not read transform file {xform_fname}"),
        )?;
        unreachable!("ErrorExit returns only for code 0");
    };
    if transform.ttype != MORPH_3D_TYPE {
        let Some(mut lta) = lta_reduce(g, r.out, r.err, transform.lta())? else {
            // the reduced array's first transform read through NULL
            g.notices.add(
                &FS_1134,
                format!(
                    "mri_mask -xform {xform_fname}: the transforms do not chain: FreeSurfer crashes"
                ),
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(SEGFAULT));
            }
            let _ = writeln!(
                r.err,
                "{progname}: ERROR: the transforms of {xform_fname} do not chain into one"
            );
            return Err(Fatal::Exit(1));
        };
        if lta.xforms[0].src.valid == 0 {
            let Some(src) = o.lta_src.as_ref() else {
                let _ = writeln!(
                    r.err,
                    "The transform does not have the valid src volume info."
                );
                let _ = writeln!(
                    r.err,
                    "Either you give src volume info by option -lta_src or"
                );
                let _ = writeln!(r.err, "make the transform to have the valid src info.");
                error_exit(g, r.err, ERROR_BADPARM, "Bailing out...\n")?;
                unreachable!("ErrorExit returns only for code 0");
            };
            lta_modify_src_dst_geom(g, r.out, r.err, &mut lta, Some(src), None);
        }
        if lta.xforms[0].dst.valid == 0 {
            let Some(dst) = o.lta_dst.as_ref() else {
                let _ = writeln!(
                    r.err,
                    "The transform does not have the valid dst volume info."
                );
                let _ = writeln!(
                    r.err,
                    "Either you give src volume info by option -lta_dst or"
                );
                let _ = writeln!(r.err, "make the transform to have the valid dst info.");
                let _ = writeln!(r.err, "If the dst was average_305, then you can set");
                let _ = writeln!(r.err, "environmental variable USE_AVERAGE305 true");
                let _ = writeln!(
                    r.err,
                    "without giving the dst volume for RAS-to-RAS transform."
                );
                error_exit(g, r.err, ERROR_BADPARM, "Bailing out...\n")?;
                unreachable!("ErrorExit returns only for code 0");
            };
            lta_modify_src_dst_geom(g, r.out, r.err, &mut lta, None, Some(dst));
        }
        lta_change_type(g, r.out, r.err, &env, &mut lta, LINEAR_VOX_TO_VOX)?;
        transform.ttype = LINEAR_VOX_TO_VOX;
        transform.xform = Xform::Lta(lta);
    }
    // `vg_isEqual_Threshold = 10e-4`: the program's own tolerance, kept for the rest of the run
    g.vg_is_equal_threshold = 10e-4;
    let vg_src = transform_get_src_vol_geom(g, r.err, &transform)?;
    let vg_dst = transform_get_dst_vol_geom(g, r.err, &transform)?;
    let vg_mask = vol_geom_of(&mask);
    let vg_in = vol_geom_of(mri_in);
    if vg_is_equal(g, &vg_mask, &vg_src) == 0 || vg_is_equal(g, &vg_in, &vg_dst) == 0 {
        if vg_is_equal(g, &vg_mask, &vg_dst) == 0 || vg_is_equal(g, &vg_in, &vg_src) == 0 {
            error_exit(
                g,
                r.err,
                ERROR_BADPARM,
                "ERROR: transform geometry does not match",
            )?;
            unreachable!("ErrorExit returns only for code 0");
        }
        let _ = writeln!(r.out, "Inverting transform to match MRI geometries");
        transform_invert_replace(g, r.out, r.err, &env, r.dir, &mut transform, Some(mri_in))?;
    }
    let moved = transform_apply_type(
        g,
        r.out,
        r.err,
        &env,
        &mut transform,
        &mask,
        None,
        SAMPLE_NEAREST,
    )?;
    Ok(moved.into_new())
}

/// FS-1135: `TransformApplyType` gave no mask. By default an error (status 1); with `reproduce`
/// the rest of FreeSurfer's program without a mask: `MRIcopy(NULL, NULL)` gives none; the
/// threshold (`-T`, `-invert`), `-samseg`, `-lh` and `-rh`, the crops (`-BB` and its forms,
/// `-BBEQ`, `-crop-to-fov`, `-crop-to-fov-mm`: their first line printed) and `-dilate` (`Dilating
/// <n>`) read it (status 139); `-erode` prints `Eroding <n>`, `MRIerodeNN`'s in-place error once a
/// pass and `MRIfree: null pointer`; then `maskval=...` and `MRImask`'s `must have a valid MRI
/// (src)` (status 252).
fn null_mask(
    g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    xform_fname: &str,
) -> Result<(), Fatal> {
    g.notices.add(
        &FS_1135,
        format!(
            "mri_mask -xform {xform_fname}: the voxel-to-voxel transform is singular in single precision: FreeSurfer goes on without a mask (a crash or 'must have a valid MRI (src)')"
        ),
    );
    if !g.notices.reproduce() {
        let _ = writeln!(
            r.err,
            "{}: ERROR: the transform {xform_fname} could not be applied to the mask volume",
            g.progname
        );
        return Err(Fatal::Exit(1));
    }
    if o.threshold_set || o.samseg || o.do_lh || o.do_rh {
        return Err(Fatal::Exit(SEGFAULT));
    }
    if o.do_bb || o.do_bb_eq || o.do_crop_to_fov || o.do_crop_to_fov_mm {
        if o.do_bb {
            let p = o.npad_bb;
            let _ = writeln!(
                r.out,
                "Computing bounding box, npad = {}, {}, {}, {}, {}, {}",
                p[0], p[1], p[2], p[3], p[4], p[5]
            );
        } else if o.do_bb_eq {
            let _ = writeln!(
                r.out,
                "Computing bounding box with ncols=nrows, npad = {}",
                o.npad_bb[0]
            );
        }
        return Err(Fatal::Exit(SEGFAULT));
    }
    let mut maskval = 0i32;
    if o.do_transfer {
        maskval = float_to_int(g, f64::from(o.transfer_val), "mri_mask -transfer");
        o.out_val = o.transfer_val;
    }
    if o.dilate > 0 {
        let _ = writeln!(r.out, "Dilating {}", o.dilate);
        return Err(Fatal::Exit(SEGFAULT));
    }
    if o.erode > 0 {
        let _ = writeln!(r.out, "Eroding {}", o.erode);
        for _ in 0..o.erode {
            let _ = writeln!(r.out, "ERROR: MRIerodeNN(): cannot be done in-place");
        }
        mri_free(g, r.err, &mut None);
    }
    let _ = writeln!(
        r.out,
        "maskval={maskval}, outval={}",
        cfmt::g(f64::from(o.out_val), 6)
    );
    // `MRImask`'s start: its environment variable, then the mask's geometry from NULL
    let mut allow = 1;
    if let Some(a) = r.env.get("FS_MRIMASK_ALLOW_DIFF_GEOM") {
        let mut st = cscan::Bytes::new(a.as_encoded_bytes());
        if let Some(n) = cscan::Call::new().i32(&mut st, &mut g.errno) {
            allow = n;
        }
    }
    if g.gdiag > 0 {
        let _ = writeln!(r.out, "MRImask(): AllowDiffGeom = {allow}");
    }
    error_exit(g, r.err, ERROR_BADPARM, "must have a valid MRI (src)")?;
    unreachable!("ErrorExit returns only for code 0");
}
