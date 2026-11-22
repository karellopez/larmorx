// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_info`'s command line (`mri_info.cpp` 56-600): its settings ([`Options`]),
//! `parse_commandline` ([`parse_commandline`]), `check_options`, the usage and the help.
//!
//! Every argument is compared with the options without case (`strcasecmp`); one that is no option
//! is an input volume. `--o`, `--otags` and `-it`/`--in_type` take the next argument (FreeSurfer
//! reads `argv[argc]`, a null pointer, when there is none: FS-1150); `--voxel` takes three
//! (`ERROR: --voxel flag needs 3 arguments`, status 255, when fewer follow), each read with `%d`
//! (one that is not a number keeps the earlier value, 0 at the start).
//!
//! Defects: FS-1146 (`--ysize` and `--zsize` print the column size; by default the row and slice
//! sizes), FS-1147 (`-it`/`--in_type` has no effect; by default the volume is read as that
//! type), FS-1149 (more than 100 inputs), FS-1150 (`--otags` or `-it` as the last argument).

use std::io::Write;

use larmorx_core::defects::{FS_1146, FS_1149, FS_1150};
use larmorx_freesurfer_core::cmdargs::cmd_arg_n_err;
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::diag::DIAG_INFO;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::version::get_version;
use larmorx_freesurfer_io::identify::{MRI_VOLUME_TYPE_UNKNOWN, string_to_type};

/// The program's streams, environment and working directory.
pub struct Run<'a, 'b> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: &'b Environment,
    pub dir: Option<&'b std::path::Path>,
}

/// `inputlist[100]`: the room FreeSurfer has for the input names (FS-1149).
pub const MAX_INPUTS: usize = 100;

/// The print flags, in the order `do_file` tests them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    pub tr: bool,
    pub te: bool,
    pub ti: bool,
    pub flip_angle: bool,
    pub flip_angle_deg: bool,
    pub pedir: bool,
    pub cres: bool,
    pub rres: bool,
    pub sres: bool,
    pub ptype: bool,
    pub conformed: bool,
    pub ctab: bool,
    pub conformed_to_min: bool,
    pub is_1mm_iso: bool,
    pub voxvol: bool,
    pub ncols: bool,
    pub nrows: bool,
    pub nslices: bool,
    pub dim: bool,
    pub res: bool,
    pub min_res: bool,
    pub dof: bool,
    pub nframes: bool,
    pub mid_frame: bool,
    pub format: bool,
    pub col_dc: bool,
    pub row_dc: bool,
    pub slice_dc: bool,
    pub vox2ras: bool,
    pub ras2vox: bool,
    pub ras_good: bool,
    pub vox2ras_tkr: bool,
    pub ras2vox_tkr: bool,
    pub vox2ras_fsl: bool,
    pub tkr2scanner: bool,
    pub scanner2tkr: bool,
    pub det: bool,
    pub orientation: bool,
    pub slice_direction: bool,
    pub cras: bool,
    pub center: bool,
    pub p0: bool,
    pub entropy: bool,
    pub voxvolsum: bool,
    pub stats: bool,
    pub voxel: bool,
    pub autoalign: bool,
    pub orig_ras2vox: bool,
    pub cmds: bool,
    pub dump: bool,
    pub to_file: bool,
    pub tags_to_file: bool,
}

/// `mri_info`'s settings after its command line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    pub flags: Flags,
    /// `inputlist`, every input (FreeSurfer's room ends at [`MAX_INPUTS`]).
    pub inputs: Vec<String>,
    /// `VoxelCRS`.
    pub voxel_crs: [i32; 3],
    /// `outfile` (`--o`; `None`: the null pointer FreeSurfer reads when it is the last argument).
    pub outfile: Option<String>,
    /// `outtagfile` (`--otags`; `None` as for `outfile`).
    pub outtagfile: Option<String>,
    pub debug: bool,
    /// `intype` as `-it` set it (`MRI_VOLUME_TYPE_UNKNOWN` without), and its text.
    pub intype: i32,
    pub intypestr: Option<String>,
    /// `ZeroCRAS`.
    pub zero_cras: bool,
}

impl Options {
    fn new() -> Options {
        Options {
            intype: MRI_VOLUME_TYPE_UNKNOWN,
            ..Options::default()
        }
    }
}

/// `print_usage()` (`USAGE: <progname> ...`; FreeSurfer's `Progname` is `argv[0]`).
pub fn print_usage(out: &mut dyn Write, progname: &str) {
    let _ = writeln!(out, "USAGE: {progname} fname1 <fname2> <options> ");
    let _ = out.write_all(USAGE.as_bytes());
}

/// The usage's lines after its first.
const USAGE: &str = concat!(
    "\n",
    "   --conformed : print whether a volume is conformed stdout\n",
    "   --conformed-to-min : print whether a volume is conformed-to-min stdout\n",
    "   --is-1mm-iso : print whether the voxel size is 1mm isotropic\n",
    "   --type : print the voxel type/precision (e.g. FLOAT) to stdout\n",
    "   --tr : print TR to stdout\n",
    "   --te : print TE to stdout\n",
    "   --ti : print TI to stdout\n",
    "   --fa : print flip angle to stdout\n",
    "   --pedir : print phase encode direction\n",
    "   --res : print col row slice and frame resolution \n",
    "   --min-res : print minimum of col row and slice resolutions (not frame) \n",
    "   --cres : print column voxel size (xsize)\n",
    "   --rres : print row    voxel size (ysize)\n",
    "   --sres : print slice  voxel size (zsize)\n",
    "   --voxvol : print voxel volume\n",
    "   --voxvolsum : compute sum of all voxels times the voxel volume \n",
    "   --ncols : print number of columns (width) to stdout\n",
    "   --nrows : print number of rows (height) to stdout\n",
    "   --nslices : print number of slices (depth) to stdout\n",
    "   --dim : print number of columns, rows, slices, and frames \n",
    "   --cdc : print column direction cosine (x_{r,a,s})\n",
    "   --rdc : print row    direction cosine (y_{r,a,s})\n",
    "   --sdc : print slice  direction cosine (z_{r,a,s})\n",
    "   --vox2ras : print the native/qform vox2ras matrix\n",
    "   --ras2vox : print the native/qform ras2vox matrix\n",
    "   --vox2ras-tkr : print the tkregister vox2ras matrix\n",
    "   --ras2vox-tkr : print the tkregister ras2vox matrix\n",
    "   --vox2ras-fsl : print the FSL/FLIRT vox2ras matrix\n",
    "   --tkr2scanner : print tkrRAS-to-scannerRAS matrix \n",
    "   --scanner2tkr : print scannerRAS-to-tkrRAS matrix \n",
    "   --ras_good : print the ras_good_flag\n",
    "   --cras : print the RAS near the center of the volume (at nv/2)\n",
    "   --center : print the RAS at the actual center of the volume (at (nv-1)/2)\n",
    "   --zero-cras : zero the center ras\n",
    "   --p0 : print the RAS at voxel (0,0,0)\n",
    "   --det : print the determinant of the vox2ras matrix\n",
    "   --dof : print the dof stored in the header\n",
    "   --nframes : print number of frames to stdout\n",
    "   --mid-frame : print number of middle frame to stdout\n",
    "   --format : file format\n",
    "   --orientation, --ori : orientation string (eg, LPS, RAS, RPI)\n",
    "   --slicedirection : primary slice direction (eg, axial)\n",
    "   --autoalign : print auto align matrix (if it exists)\n",
    "   --ctab : print embedded color lookup table\n",
    "   --cmds : print command-line provenance info\n",
    "   --dump : print FA, TR, TE, TI, etc \n",
    "   --voxel c r s : dump voxel value from col row slice (0-based, all frames)\n",
    "   --entropy : compute and print entropy \n",
    "   --o file : print flagged results to file \n",
    "   --otags file: print TAG info to stdout, and save the binary data to file \n",
    "   ----orig_ras2vox : print orig Ras2Vox matrix if present\n",
    "   --in_type type : explicitly specify file type (see mri_convert) \n",
    "\n",
);

/// `print_help()`'s text after the usage.
const HELP: &str = concat!(
    "\n",
    "Dumps information about the volume to stdout. Specific pieces \n",
    "of information can be printed out as well by specifying the proper\n",
    "flag (eg, --tr for TR). Time is in msec. Distance is in mm. Angles\n",
    "are in radians.\n",
    "\n",
    "The direction cosine outputs (--cdc, --rdc, --sdc) correspond to \n",
    "mri_convert flags -iid, -ijd, -ikd.\n",
);

/// `usage_exit()`: the usage, status 1.
pub fn usage_exit(out: &mut dyn Write, progname: &str) -> Fatal {
    print_usage(out, progname);
    Fatal::Exit(1)
}

/// `sscanf(arg, "%d", &v)`: `v` unchanged when the argument holds no number (`errno` as glibc
/// leaves it).
fn scan_int(g: &mut Globals, arg: &str, v: &mut i32) {
    let mut b = Bytes::new(arg.as_bytes());
    let mut sc = Scanf::new(&mut b);
    let n = sc.d(&mut g.errno, true);
    if sc.finish(&mut g.errno) == 1
        && let Some(n) = n
    {
        *v = n;
    }
}

/// `parse_commandline(argc, argv)` on the arguments after the program's name (at least one).
pub fn parse_commandline(
    g: &mut Globals,
    r: &mut Run,
    args: &[String],
    progname: &str,
) -> Result<Options, Fatal> {
    let mut o = Options::new();
    let mut i = 0;
    let n = args.len();
    while i < n {
        let option = args[i].as_str();
        if o.debug {
            let _ = writeln!(r.out, "{} {option}", n - i);
        }
        i += 1;
        // the argument after the option: `argv[argc]` (a null pointer) when there is none
        let next = |i: usize| args.get(i).cloned();
        let is = |name: &str| option.eq_ignore_ascii_case(name);
        let f = &mut o.flags;
        if is("--help") {
            print_usage(r.out, progname);
            let _ = r.out.write_all(HELP.as_bytes());
            return Err(Fatal::Exit(1));
        } else if is("--version") {
            let _ = writeln!(r.out, "{}", get_version());
            return Err(Fatal::Exit(1));
        } else if is("--dump") {
            f.dump = true;
        } else if is("--zero-cras") {
            o.zero_cras = true;
        } else if is("--debug") {
            o.debug = true;
        } else if is("--tr") {
            f.tr = true;
        } else if is("--te") {
            f.te = true;
        } else if is("--ti") {
            f.ti = true;
        } else if is("--fa") || is("--flip_angle") {
            f.flip_angle = true;
        } else if is("--fad") {
            f.flip_angle_deg = true;
        } else if is("--pedir") {
            f.pedir = true;
        } else if is("--cres") || is("--xsize") {
            f.cres = true;
        } else if is("--rres") {
            f.rres = true;
        } else if is("--ysize") || is("--zsize") {
            // FS-1146: FreeSurfer sets the column flag for both
            g.notices.add(
                &FS_1146,
                format!(
                    "mri_info {option}: FreeSurfer prints the column size (xsize) for it, not the {} size",
                    if is("--ysize") { "row" } else { "slice" }
                ),
            );
            if g.notices.reproduce() {
                f.cres = true;
            } else if is("--ysize") {
                f.rres = true;
            } else {
                f.sres = true;
            }
        } else if is("--sres") {
            f.sres = true;
        } else if is("--voxvol") {
            f.voxvol = true;
        } else if is("--type") {
            f.ptype = true;
        } else if is("--is-1mm-iso") {
            f.is_1mm_iso = true;
        } else if is("--conformed") {
            f.conformed = true;
        } else if is("--conformed-to-min") {
            f.conformed_to_min = true;
        } else if is("--ncols") || is("--width") {
            f.ncols = true;
        } else if is("--nrows") || is("--height") {
            f.nrows = true;
        } else if is("--nslices") || is("--depth") {
            f.nslices = true;
        } else if is("--dim") {
            f.dim = true;
        } else if is("--res") {
            f.res = true;
        } else if is("--min-res") {
            f.min_res = true;
        } else if is("--dof") {
            f.dof = true;
        } else if is("--cdc") {
            f.col_dc = true;
        } else if is("--rdc") {
            f.row_dc = true;
        } else if is("--sdc") {
            f.slice_dc = true;
        } else if is("--vox2ras") {
            f.vox2ras = true;
        } else if is("--ras2vox") {
            f.ras2vox = true;
        } else if is("--vox2ras-tkr") {
            f.vox2ras_tkr = true;
        } else if is("--ras2vox-tkr") {
            f.ras2vox_tkr = true;
        } else if is("--vox2ras-fsl") {
            f.vox2ras_fsl = true;
        } else if is("--tkr2scanner") {
            f.tkr2scanner = true;
        } else if is("--ctab") {
            f.ctab = true;
        } else if is("--scanner2tkr") {
            f.scanner2tkr = true;
        } else if is("--ras_good") {
            f.ras_good = true;
        } else if is("--cras") {
            f.cras = true;
        } else if is("--center") {
            f.center = true;
        } else if is("--p0") {
            f.p0 = true;
        } else if is("--det") {
            f.det = true;
        } else if is("--nframes") {
            f.nframes = true;
        } else if is("--mid-frame") {
            f.mid_frame = true;
        } else if is("--format") {
            f.format = true;
        } else if is("--orientation") || is("--ori") {
            f.orientation = true;
        } else if is("--slicedirection") {
            f.slice_direction = true;
        } else if is("--autoalign") {
            f.autoalign = true;
        } else if is("--orig_ras2vox") {
            f.orig_ras2vox = true;
        } else if is("--entropy") {
            f.entropy = true;
        } else if is("--voxvolsum") {
            f.voxvolsum = true;
        } else if is("--stats") {
            f.stats = true;
        } else if is("--cmds") {
            f.cmds = true;
        } else if is("--o") {
            f.to_file = true;
            o.outfile = next(i);
            i += 1;
        } else if is("--otags") {
            f.tags_to_file = true;
            o.outtagfile = next(i);
            i += 1;
        } else if is("--diag-debug") {
            g.gdiag |= DIAG_INFO;
        } else if is("-it") || is("--in_type") {
            let Some(s) = next(i) else {
                // FS-1150: `string_to_type(NULL)`
                return Err(no_argument(g, r, option));
            };
            o.intype = string_to_type(g, &s);
            o.intypestr = Some(s);
            i += 1;
        } else if is("--voxel") {
            if n - i < 3 {
                return Err(cmd_arg_n_err(r.err, option, 3));
            }
            f.voxel = true;
            for (k, v) in o.voxel_crs.iter_mut().enumerate() {
                scan_int(g, &args[i + k], v);
            }
            if g.gdiag_no > 1 {
                let v = o.voxel_crs;
                let _ = writeln!(r.out, "{} {} {}", v[0], v[1], v[2]);
            }
            i += 3;
        } else {
            // must be an input volume
            if o.inputs.len() == MAX_INPUTS {
                g.notices.add(
                    &FS_1149,
                    format!(
                        "mri_info: more than {MAX_INPUTS} input volumes, whose names FreeSurfer writes past its list, over other settings (its result depends on its build)"
                    ),
                );
            }
            o.inputs.push(option.to_owned());
        }
    }
    Ok(o)
}

/// FS-1150: an option whose argument FreeSurfer reads past the command line's end (a null
/// pointer) where it crashes; the message and status 1 in both modes.
pub fn no_argument(g: &mut Globals, r: &mut Run, option: &str) -> Fatal {
    g.notices.add(
        &FS_1150,
        format!(
            "mri_info {option}: the last argument, whose file name FreeSurfer reads past the command line (a crash)"
        ),
    );
    let _ = writeln!(
        r.err,
        "mri_info: {option} has no argument (FreeSurfer crashes here)"
    );
    Fatal::Exit(1)
}

/// `check_options()`: an input is needed (`ERROR: no input volume supplied`, status 1).
pub fn check_options(r: &mut Run, o: &Options) -> Result<(), Fatal> {
    if o.inputs.is_empty() {
        let _ = writeln!(r.out, "ERROR: no input volume supplied");
        return Err(Fatal::Exit(1));
    }
    Ok(())
}
