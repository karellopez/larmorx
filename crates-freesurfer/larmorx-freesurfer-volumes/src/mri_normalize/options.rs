// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_normalize`'s options (`get_option`, `mri_normalize.cpp` 1001-1340, with the file statics
//! they set, 47-160) and its help (`usage_exit` 1342-1348: `mri_normalize.help.xml`, FreeSurfer's
//! own text, rendered by `outputHelpXml`).
//!
//! An option is an argument starting with `-`; its name is compared without the first dash and
//! without case (`--help` and `--usage` with their second dash); a name that matches none is
//! read by its first letter (`-foo` is `-f`), and a letter that matches none ends the program
//! (`unknown option <arg>`, status 1). The help (`-u`, `-h`, `-?`, `--help`, `--usage`) ends it
//! with status 0. Each option prints FreeSurfer's message; the atlas (`-atlas`), the surfaces'
//! transforms (`-surface`) and the labels (`-label`, `-lonly`) are read when the option is.
//!
//! FS-1016: an option given fewer arguments than it takes reads past the command line: a number
//! read from the end crashes FreeSurfer (its `atoi`/`atof` of `NULL`), and so does a name the
//! option opens (the atlas, a transform, a label); a name it only keeps prints as `(null)`, and
//! the next one is the first variable of the process's environment. Both modes end with
//! `<program>: option <arg> needs more arguments` (status 1) where FreeSurfer crashes, and print
//! a missing name as `(null)` (the environment's variable noted).

use larmorx_core::cfmt;
use larmorx_core::defects::FS_1016;
use larmorx_freesurfer_core::cstd::{atof, atoi, strtol10};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::help_xml::output_help_xml;
use larmorx_freesurfer_core::log::{self, ERROR_NOFILE, ERROR_NOMEMORY, Fatal};
use larmorx_freesurfer_core::random::set_random_seed;
use larmorx_freesurfer_core::utils::stricmp;
use larmorx_freesurfer_filter::normalize::DEFAULT_DESIRED_WHITE_MATTER_VALUE;
use larmorx_freesurfer_gca::gca::Gca;
use larmorx_freesurfer_gca::read::gca_read;
use larmorx_freesurfer_gca::transform::{Transform, transform_read};
use larmorx_freesurfer_io::label::{Label, label_read};

use super::helpers::Run;

/// `mri_normalize.help.xml`, the text `usage_exit` renders.
pub const HELP_XML: &[u8] = include_bytes!("mri_normalize.help.xml");

/// `CONTRAST_UNKNOWN`, `T1_CONTRAST`, `T2_CONTRAST`.
pub const CONTRAST_UNKNOWN: i32 = 0;
pub const T1_CONTRAST: i32 = 1;
pub const T2_CONTRAST: i32 = 2;
/// `MRI_UNKNOWN`, `MRI_MGH_MPRAGE`, `MRI_WASHU_MPRAGE` (`mri.h`'s scan types).
pub const MRI_UNKNOWN: i32 = 0;
pub const MRI_MGH_MPRAGE: i32 = 1;
pub const MRI_WASHU_MPRAGE: i32 = 3;
/// `MAX_NORM_SURFACES`.
pub const MAX_NORM_SURFACES: usize = 10;
/// `MAX_GRADIENT` (`mrinorm.h`: `5.0f / 5.0f`), `main`'s value of `mni.max_gradient`.
pub const MAX_GRADIENT: f32 = 1.0;

/// A surface of `-surface` with its transform.
#[derive(Debug)]
pub struct NormSurface {
    pub fname: String,
    pub xform: Transform,
    pub xform_fname: String,
}

/// The file statics the options set, with FreeSurfer's initial values.
pub struct Options {
    pub contrast: i32,
    pub gca: Option<Gca>,
    pub xform: Option<Transform>,
    pub brain_distance: i32,
    pub conform: i32,
    pub gentle_flag: i32,
    pub noskull: i32,
    pub nosnr: i32,
    pub min_dist: f64,
    pub checknorm_fname: Option<String>,
    pub check_min: f32,
    pub check_max: f32,
    pub renorm_fname: Option<String>,
    pub renorm_val: i32,
    pub bias_sigma: f32,
    pub mask_fname: Option<String>,
    pub interior_fname1: Option<String>,
    pub interior_fname2: Option<String>,
    pub mask_sigma: f32,
    pub mask_thresh: f32,
    pub mask_orig_fname: Option<String>,
    pub mask_orig_thresh: f32,
    pub scan_type: i32,
    pub prune: i32,
    /// `mni.max_gradient` (the rest of `mni` is `MRInormInit`'s).
    pub max_gradient: f32,
    pub verbose: i32,
    pub num_3d_iter: i32,
    pub intensity_above: f32,
    pub intensity_below: f32,
    pub control_point_fname: Option<String>,
    pub long_control_volume_fname: Option<String>,
    pub long_bias_volume_fname: Option<String>,
    pub aseg_fname: Option<String>,
    pub control_volume_fname: Option<String>,
    pub bias_volume_fname: Option<String>,
    pub read_flag: i32,
    pub long_flag: i32,
    pub no1d: i32,
    pub file_only: i32,
    pub surfaces: Vec<NormSurface>,
    pub grad_thresh: f32,
    pub control_point_label: Option<Label>,
    pub output_control_points_vol: Option<String>,
    pub do_gentle_cp_file: i32,
    pub nonmax_suppress: i32,
    pub erode: i32,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            contrast: CONTRAST_UNKNOWN,
            gca: None,
            xform: None,
            brain_distance: 1,
            conform: 0,
            gentle_flag: 0,
            noskull: 0,
            nosnr: 1,
            min_dist: 2.5,
            checknorm_fname: None,
            check_min: 90.0,
            check_max: 120.0,
            renorm_fname: None,
            renorm_val: DEFAULT_DESIRED_WHITE_MATTER_VALUE,
            bias_sigma: 8.0,
            mask_fname: None,
            interior_fname1: None,
            interior_fname2: None,
            mask_sigma: 0.0,
            mask_thresh: 0.0,
            mask_orig_fname: None,
            mask_orig_thresh: 150.0,
            scan_type: MRI_UNKNOWN,
            prune: 0,
            max_gradient: MAX_GRADIENT,
            verbose: 1,
            num_3d_iter: 2,
            intensity_above: 25.0,
            intensity_below: 10.0,
            control_point_fname: None,
            long_control_volume_fname: None,
            long_bias_volume_fname: None,
            aseg_fname: None,
            control_volume_fname: None,
            bias_volume_fname: None,
            read_flag: 0,
            long_flag: 0,
            no1d: 0,
            file_only: 0,
            surfaces: Vec::new(),
            grad_thresh: -1.0,
            control_point_label: None,
            output_control_points_vol: None,
            do_gentle_cp_file: 1,
            nonmax_suppress: 1,
            erode: 0,
        }
    }
}

/// `usage_exit(code)`: the help on standard output, then the end of the program.
pub fn usage_exit(g: &mut Globals, r: &mut Run, code: i32) -> Fatal {
    output_help_xml(g, r.out, r.err, Some(HELP_XML));
    Fatal::Exit(code)
}

/// `%<w>.<p>f`.
fn ff(v: f64, w: usize, p: usize) -> String {
    cfmt::pad(&cfmt::f(v, p), w)
}

/// The option's arguments as `argv` holds them: given, the end of the list (`NULL`), or past it
/// (the environment).
struct Args<'s> {
    opt: &'s str,
    rest: &'s [String],
}

impl Args<'_> {
    /// `argv[1 + k]`: `Some` when given; `None` at or past the end.
    fn get(&self, k: usize) -> Option<&str> {
        self.rest.get(k - 1).map(String::as_str)
    }

    /// A name the option keeps: the name, or `(null)` as `printf` prints `NULL` (past the end,
    /// the environment's first variable in FreeSurfer: FS-1016, noted).
    fn name(&self, g: &mut Globals, k: usize) -> Option<String> {
        if k > self.rest.len() + 1 {
            g.notices.add(
                &FS_1016,
                format!(
                    "option {}: argument {k} read past the command line (the environment's first \
                     variable in FreeSurfer; none here)",
                    self.opt
                ),
            );
        }
        self.get(k).map(str::to_owned)
    }

    /// A value the option needs (a number, or a name it opens): FreeSurfer crashes when it is
    /// missing (FS-1016: both modes end with an error).
    fn need(&self, g: &mut Globals, r: &mut Run, k: usize) -> Result<&str, Fatal> {
        if let Some(a) = self.get(k) {
            return Ok(a);
        }
        if k > self.rest.len() + 1 {
            // past argv's NULL: one of the environment's strings in FreeSurfer, read as a
            // number (0 for a variable's name) or a name (none here)
            g.notices.add(
                &FS_1016,
                format!(
                    "option {}: argument {k} read past the command line (the environment's \
                     variables in FreeSurfer; an empty argument here)",
                    self.opt
                ),
            );
            return Ok("");
        }
        g.notices.add(
            &FS_1016,
            format!(
                "option {}: argument {k} missing (FreeSurfer dereferences NULL and crashes)",
                self.opt
            ),
        );
        let progname = g.progname.clone();
        log::error_exit(
            g,
            r.err,
            1,
            &format!("{progname}: option {} needs more arguments", self.opt),
        )?;
        Ok("")
    }
}

fn shown(s: &Option<String>) -> &str {
    s.as_deref().unwrap_or("(null)")
}

/// `get_option(argc, argv)` for the option `args[0]` (its arguments after it): what it sets,
/// its message on standard output; the number of arguments it took (`nargs`). The help and an
/// unknown option end the program ([`Fatal::Exit`]).
pub fn get_option(
    g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    args: &[String],
) -> Result<usize, Fatal> {
    let arg1 = args[0].as_str();
    let option = &arg1[1..];
    let a = Args {
        opt: arg1,
        rest: &args[1..],
    };
    let is = |g: &mut Globals, name: &str| stricmp(g, option, name) == 0;
    let mut nargs = 0;
    if is(g, "-help") || is(g, "-usage") {
        return Err(usage_exit(g, r, 0));
    } else if is(g, "no1d") {
        o.no1d = 1;
        let _ = writeln!(r.out, "disabling 1d normalization...");
    } else if is(g, "T1") {
        o.contrast = T1_CONTRAST;
        let _ = writeln!(r.out, "assuming in vivo T1 contrast");
    } else if is(g, "T2") || is(g, "PD") {
        o.contrast = T2_CONTRAST;
        let _ = writeln!(r.out, "assuming T2/PD contrast");
    } else if is(g, "nonmax_suppress") {
        o.nonmax_suppress = atoi(a.need(g, r, 1)?.as_bytes());
        let what = if o.nonmax_suppress != 0 {
            "using"
        } else {
            "disabling"
        };
        let _ = writeln!(r.out, "{what} nonmaximum suppression");
        nargs = 1;
    } else if is(g, "erode") {
        o.erode = atoi(a.need(g, r, 1)?.as_bytes());
        let _ = writeln!(r.out, "eroding interior of surface {} times", o.erode);
        nargs = 1;
    } else if is(g, "renorm") {
        o.renorm_fname = a.name(g, 1);
        let _ = writeln!(
            r.out,
            "renormalizing using voxels that are {DEFAULT_DESIRED_WHITE_MATTER_VALUE} in {}",
            shown(&o.renorm_fname)
        );
        nargs = 1;
    } else if is(g, "checknorm") {
        o.checknorm_fname = a.name(g, 1);
        o.check_min = atof(a.need(g, r, 2)?.as_bytes()) as f32;
        o.check_max = atof(a.need(g, r, 3)?.as_bytes()) as f32;
        let _ = writeln!(
            r.out,
            "removing control points that are not in [{} {}] in {}",
            ff(f64::from(o.check_min), 2, 0),
            ff(f64::from(o.check_max), 2, 0),
            shown(&o.checknorm_fname)
        );
        nargs = 3;
    } else if is(g, "ATLAS") {
        let (n2, n3) = (a.name(g, 1), a.name(g, 2));
        let _ = writeln!(
            r.out,
            "reading gca and atlas transform from {} and {}",
            shown(&n2),
            shown(&n3)
        );
        let gname = a.need(g, r, 1)?.to_owned();
        o.gca = gca_read(g, r.out, r.err, r.dir, &gname)?;
        let tname = a.need(g, r, 2)?.to_owned();
        o.xform = transform_read(g, r.out, r.err, r.env, r.dir, &tname)?;
        o.brain_distance = atoi(a.need(g, r, 3)?.as_bytes());
        let _ = writeln!(
            r.out,
            "reading gca from {gname}, xform from {tname} and setting brain distance to {} voxels",
            o.brain_distance
        );
        nargs = 3;
        if o.gca.is_none() || o.xform.is_none() {
            let progname = g.progname.clone();
            log::error_exit(
                g,
                r.err,
                ERROR_NOFILE,
                &format!("{progname}: could not read gca ({gname}) or transform ({tname})\n"),
            )?;
        }
    } else if is(g, "C") {
        o.output_control_points_vol = a.name(g, 1);
        nargs = 1;
        let _ = writeln!(
            r.out,
            "Writing output control points volume to {}",
            shown(&o.output_control_points_vol)
        );
    } else if is(g, "MASK") {
        o.mask_fname = a.name(g, 1);
        nargs = 1;
        let _ = writeln!(
            r.out,
            "using MR volume {} to mask input volume...",
            shown(&o.mask_fname)
        );
    } else if is(g, "NOSKULL") {
        o.noskull = 1;
        let _ = writeln!(r.out, "assuming input volume has been skull stripped");
    } else if is(g, "GRAD") {
        o.grad_thresh = atof(a.need(g, r, 1)?.as_bytes()) as f32;
        nargs = 1;
        let _ = writeln!(
            r.out,
            "using gradient volume thresholded at {} to prevent control points from crossing \
             edges...",
            ff(f64::from(o.grad_thresh), 2, 1)
        );
    } else if is(g, "MASK_SIGMA") {
        o.mask_sigma = atof(a.need(g, r, 1)?.as_bytes()) as f32;
        o.mask_thresh = atof(a.need(g, r, 2)?.as_bytes()) as f32;
        let _ = writeln!(
            r.out,
            "smoothing input volume with sigma = {} mm and thresholding at {} for mask",
            ff(f64::from(o.mask_sigma), 2, 3),
            ff(f64::from(o.mask_thresh), 2, 0)
        );
        nargs = 2;
    } else if is(g, "MASK_ORIG") {
        o.mask_orig_fname = a.name(g, 1);
        o.mask_orig_thresh = atof(a.need(g, r, 2)?.as_bytes()) as f32;
        let _ = writeln!(
            r.out,
            "removing control points that are below {} in {}",
            ff(f64::from(o.mask_orig_thresh), 2, 3),
            shown(&o.mask_orig_fname)
        );
        nargs = 2;
    } else if is(g, "SURFACE") {
        if o.surfaces.len() >= MAX_NORM_SURFACES {
            log::error_exit(
                g,
                r.err,
                ERROR_NOMEMORY,
                &format!("too many surfaces ({}) specified", o.surfaces.len()),
            )?;
        }
        let fname = a.name(g, 1).unwrap_or_else(|| "(null)".to_owned());
        let xname = a.need(g, r, 2)?.to_owned();
        let xform = transform_read(g, r.out, r.err, r.env, r.dir, &xname)?;
        match xform {
            Some(xform) => o.surfaces.push(NormSurface {
                fname,
                xform,
                xform_fname: xname,
            }),
            None => {
                let progname = g.progname.clone();
                log::error_exit(
                    g,
                    r.err,
                    ERROR_NOFILE,
                    &format!("{progname}:could not load xform from {xname}"),
                )?;
            }
        }
        nargs = 2;
    } else if is(g, "MIN_DIST") {
        o.min_dist = atof(a.need(g, r, 1)?.as_bytes());
        nargs = 1;
        let what = if o.nonmax_suppress != 0 {
            " nonmaximum suppressed"
        } else {
            ""
        };
        let _ = writeln!(
            r.out,
            "retaining {what} points that are at least {}mm from the boundary",
            ff(o.min_dist, 2, 3)
        );
    } else if is(g, "INTERIOR") {
        o.interior_fname1 = a.name(g, 1);
        o.interior_fname2 = a.name(g, 2);
        nargs = 2;
        let _ = writeln!(
            r.out,
            "using surfaces {} and {} to compute wm interior",
            shown(&o.interior_fname1),
            shown(&o.interior_fname2)
        );
    } else if is(g, "MGH_MPRAGE") || is(g, "MPRAGE") {
        o.scan_type = MRI_MGH_MPRAGE;
        let _ = writeln!(
            r.out,
            "assuming input volume is MGH (Van der Kouwe) MP-RAGE"
        );
        o.intensity_below = 15.0;
    } else if is(g, "WASHU_MPRAGE") {
        o.scan_type = MRI_WASHU_MPRAGE;
        let _ = writeln!(r.out, "assuming input volume is WashU MP-RAGE (dark GM)");
        o.intensity_below = 22.0;
    } else if is(g, "monkey") {
        o.no1d = 1;
        o.num_3d_iter = 1;
        let _ = writeln!(
            r.out,
            "disabling 1D normalization and setting niter=1, make sure to use -f to specify \
             control points"
        );
    } else if is(g, "nosnr") {
        o.nosnr = 1;
        let _ = writeln!(r.out, "disabling SNR normalization");
    } else if is(g, "snr") {
        o.nosnr = 0;
        let _ = writeln!(r.out, "enabling SNR normalization");
    } else if is(g, "sigma") {
        o.bias_sigma = atof(a.need(g, r, 1)?.as_bytes()) as f32;
        nargs = 1;
        let _ = writeln!(
            r.out,
            "using Gaussian smoothing of bias field, sigma={}",
            ff(f64::from(o.bias_sigma), 2, 3)
        );
    } else if is(g, "no-gentle-cp") {
        o.do_gentle_cp_file = 0;
    } else if is(g, "gentle-cp") {
        o.do_gentle_cp_file = 1;
    } else if is(g, "conform") {
        o.conform = 1;
        let _ = writeln!(r.out, "interpolating and embedding volume to be 256^3...");
    } else if is(g, "noconform") {
        o.conform = 0;
        let _ = writeln!(
            r.out,
            "not interpolating and embedding volume to be 256^3..."
        );
    } else if is(g, "aseg") || is(g, "segmentation") {
        o.aseg_fname = a.name(g, 1);
        nargs = 1;
        let _ = writeln!(
            r.out,
            "using segmentation for initial intensity normalization"
        );
    } else if is(g, "gentle") {
        o.gentle_flag = 1;
        let _ = writeln!(r.out, "performing kinder gentler normalization...");
    } else if is(g, "label") {
        let shown_name = a.name(g, 1);
        let _ = writeln!(
            r.out,
            "reading control points from label file {}",
            shown(&shown_name)
        );
        read_label(g, r, o, &a)?;
        nargs = 1;
    } else if is(g, "lonly") || is(g, "label_only") || is(g, "labelonly") {
        let shown_name = a.name(g, 1);
        let _ = writeln!(
            r.out,
            "only applying control points from label file {}",
            shown(&shown_name)
        );
        read_label(g, r, o, &a)?;
        nargs = 1;
        o.file_only = 1;
        o.no1d = 1;
    } else if is(g, "file_only") || is(g, "fonly") || is(g, "fileonly") {
        o.file_only = 1;
        o.control_point_fname = a.name(g, 1);
        o.no1d = 1;
        nargs = 1;
        let _ = writeln!(
            r.out,
            "using control points from file {}...",
            shown(&o.control_point_fname)
        );
        let _ = writeln!(r.out, "only using file control points...");
    } else if is(g, "seed") {
        let s = a.need(g, r, 1)?.as_bytes().to_vec();
        set_random_seed(g, strtol10(&s));
        let _ = writeln!(
            r.out,
            "setting seed for random number genererator to {}",
            atoi(&s)
        );
        nargs = 1;
    } else {
        let c = option.bytes().next().unwrap_or(0).to_ascii_uppercase();
        match c {
            b'D' | b'V' => {
                let x = atoi(a.need(g, r, 1)?.as_bytes());
                let y = atoi(a.need(g, r, 2)?.as_bytes());
                let z = atoi(a.need(g, r, 3)?.as_bytes());
                nargs = 3;
                if c == b'D' {
                    (g.gx, g.gy, g.gz) = (x, y, z);
                    let _ = writeln!(r.out, "debugging voxel ({x}, {y}, {z})");
                } else {
                    (g.gvx, g.gvy, g.gvz) = (x, y, z);
                    let _ = writeln!(r.out, "debugging alternative voxel ({x}, {y}, {z})");
                }
            }
            b'P' => {
                o.prune = atoi(a.need(g, r, 1)?.as_bytes());
                nargs = 1;
                let what = if o.prune > 0 { "on" } else { "off" };
                let _ = writeln!(r.out, "turning control point pruning {what}");
                if o.prune == 0 {
                    o.prune = -1;
                }
            }
            b'R' | b'L' => {
                let (ctrl, bias) = (a.name(g, 1), a.name(g, 2));
                nargs = 2;
                let _ = writeln!(
                    r.out,
                    "reading bias field from {} and ctrl points from {}",
                    shown(&bias),
                    shown(&ctrl)
                );
                if c == b'R' {
                    o.read_flag = 1;
                    o.control_volume_fname = ctrl;
                    o.bias_volume_fname = bias;
                } else {
                    o.long_flag = 1;
                    o.no1d = 1;
                    o.long_control_volume_fname = ctrl;
                    o.long_bias_volume_fname = bias;
                }
            }
            b'W' => {
                o.control_volume_fname = a.name(g, 1);
                o.bias_volume_fname = a.name(g, 2);
                nargs = 2;
                let _ = writeln!(
                    r.out,
                    "writing ctrl pts to   {}",
                    shown(&o.control_volume_fname)
                );
                let _ = writeln!(
                    r.out,
                    "writing bias field to {}",
                    shown(&o.bias_volume_fname)
                );
            }
            b'F' => {
                o.control_point_fname = a.name(g, 1);
                nargs = 1;
                let _ = writeln!(
                    r.out,
                    "using control points from file {}...",
                    shown(&o.control_point_fname)
                );
            }
            b'A' => {
                o.intensity_above = atof(a.need(g, r, 1)?.as_bytes()) as f32;
                let _ = writeln!(
                    r.out,
                    "using control point with intensity {} above target.",
                    ff(f64::from(o.intensity_above), 2, 1)
                );
                nargs = 1;
            }
            b'B' => {
                o.intensity_below = atof(a.need(g, r, 1)?.as_bytes()) as f32;
                let _ = writeln!(
                    r.out,
                    "using control point with intensity {} below target.",
                    ff(f64::from(o.intensity_below), 2, 1)
                );
                nargs = 1;
            }
            b'G' => {
                o.max_gradient = atof(a.need(g, r, 1)?.as_bytes()) as f32;
                let _ = writeln!(
                    r.out,
                    "using max gradient = {}",
                    ff(f64::from(o.max_gradient), 2, 3)
                );
                nargs = 1;
            }
            b'N' => {
                o.num_3d_iter = atoi(a.need(g, r, 1)?.as_bytes());
                nargs = 1;
                let _ = writeln!(r.out, "performing 3d normalization {} times", o.num_3d_iter);
            }
            b'?' | b'U' | b'H' => return Err(usage_exit(g, r, 0)),
            _ => {
                let _ = writeln!(r.out, "unknown option {arg1}");
                return Err(Fatal::Exit(1));
            }
        }
    }
    Ok(nargs)
}

/// `control_point_label = LabelRead(NULL, argv[2])`, ending the program when it fails (`<program>:
/// could not read label from <name>`).
fn read_label(g: &mut Globals, r: &mut Run, o: &mut Options, a: &Args) -> Result<(), Fatal> {
    let name = a.need(g, r, 1)?.to_owned();
    o.control_point_label = label_read(g, r.env, r.out, r.err, r.dir, None, &name)?;
    if o.control_point_label.is_none() {
        let progname = g.progname.clone();
        log::error_exit(
            g,
            r.err,
            ERROR_NOFILE,
            &format!("{progname}: could not read label from {name}"),
        )?;
    }
    Ok(())
}

/// `main`'s option loop (`for ( ; argc > 1 && ISOPTION(*argv[1]) ; ...)`) over `args` (the
/// arguments after the program's name, the version options already handled): each argument
/// starting with `-` and its own arguments; the index of the first argument that is not an
/// option (past the end when the last option took more arguments than were left).
pub fn parse_options(
    g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    args: &[String],
) -> Result<usize, Fatal> {
    let mut i = 0;
    while i < args.len() && args[i].starts_with('-') {
        let nargs = get_option(g, r, o, &args[i..])?;
        i += 1 + nargs;
    }
    Ok(i)
}
