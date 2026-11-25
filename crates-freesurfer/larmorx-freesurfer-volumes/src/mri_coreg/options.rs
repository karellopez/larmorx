// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_coreg`'s `parse_commandline` (`mri_coreg.cpp` 557-1004), `usage_exit`, `print_usage`,
//! `print_help`, `print_version` (1005-1104), `check_options` (1105-1137) and `dump_options`
//! (1138-1160).
//!
//! The options are read one by one, each compared without case (`strcasecmp`) but `--sd`,
//! `-SDIR`, `--no-coord-dither`, `--no-intensity-dither` and `--no-dither` (exactly); with
//! `--debug` on, each later option is printed first (`<arguments left> <option>`). An option
//! missing its arguments: `ERROR: <option> flag needs <n> argument(s)` on standard error, status
//! 255 (`--rms` asks for 2 and names 4). An unknown option: `ERROR: Option <option> unknown` (and
//! `       Did you really mean -<option> ?` for a single dash) on standard error, status 255.
//! Numbers are read with `sscanf` (a value that does not read leaves the setting as it was).
//! `--help` prints the usage with the settings so far and the help (status 1), `--version` the
//! version (status 1). The stand-alone options ([`super::standalone`]) do their work at once and
//! end the program. `--s <subject>` also names the reference mask `aparc+aseg.mgz`; `--sd`
//! sets `SUBJECTS_DIR` for the rest of the program.
//!
//! FS-1313: more than 10 `--sep` write past FreeSurfer's list of 10 (memory contents): a clear
//! error by default and in `reproduce`. FS-1314: `--threads` with no number sets OpenMP's thread
//! count from an `int` never written: the count is left as it was, noted.

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_1313, FS_1314};
use larmorx_freesurfer_core::cmdargs::{cmd_arg_n_err, cmd_single_dash};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_io::fio::fio_file_exists_readable;
use larmorx_freesurfer_mri::lta::{LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX};

use super::main::{NAME, Run, VCID};
use super::standalone;
use crate::mri_segstats::options::{scan_d, scan_lf};

/// `seplist[10]`: the most spatial scales FreeSurfer keeps.
pub const MAX_SEP: usize = 10;

/// `CMDARGS` and the file-scope switches (`debug`, `checkoptsonly`), FreeSurfer's defaults.
#[derive(Clone, Debug)]
pub struct CmdArgs {
    pub mov: Option<String>,
    pub ref_vol: Option<String>,
    pub refmask: Option<String>,
    pub movmask: Option<String>,
    pub outreg: Option<String>,
    pub regdat: Option<String>,
    pub subject: Option<String>,
    pub do_coord_dither: i32,
    pub do_intensity_dither: i32,
    pub movidither_file: Option<String>,
    pub dof: i32,
    pub params: [f64; 12],
    pub seplist: Vec<i32>,
    pub do_init_cost_only: Option<String>,
    pub do_smoothing: i32,
    pub cras0: i32,
    pub align_centroids: i32,
    pub ftol: f64,
    pub linmintol: f64,
    pub nitersmax: i32,
    pub refconf: i32,
    pub logcost: Option<String>,
    pub do_bf: i32,
    pub bf_lim: f64,
    pub bf_nsamp: i32,
    pub outparamfile: Option<String>,
    pub outcostfile: Option<String>,
    pub fwhmc: f64,
    pub fwhmr: f64,
    pub fwhms: f64,
    pub smooth_ref: i32,
    pub sat_pct: f64,
    pub mov_oob_flag: i32,
    pub rusagefile: String,
    pub optschema: i32,
    pub seed: i32,
    pub movoutfile: Option<String>,
    pub init_reg_save: Option<String>,
    pub init_reg_save_only: bool,
    pub ltatype: i32,
    /// `debug` (the file-scope switch `--debug` sets).
    pub debug: i32,
    /// `checkoptsonly`.
    pub checkoptsonly: bool,
    /// The thread count the last `--threads` (`--nthreads`) gave OpenMP, if any.
    pub threads: Option<i32>,
}

impl Default for CmdArgs {
    fn default() -> CmdArgs {
        let mut params = [0.0f64; 12];
        params[6..9].fill(1.0);
        CmdArgs {
            mov: None,
            ref_vol: None,
            refmask: None,
            movmask: None,
            outreg: None,
            regdat: None,
            subject: None,
            do_coord_dither: 1,
            do_intensity_dither: 1,
            movidither_file: None,
            dof: 6,
            params,
            seplist: Vec::new(),
            do_init_cost_only: None,
            do_smoothing: 1,
            cras0: 1,
            align_centroids: 0,
            nitersmax: 4,
            ftol: 10e-8,
            linmintol: 0.001,
            refconf: 0,
            logcost: None,
            do_bf: 1,
            bf_lim: 30.0,
            bf_nsamp: 30,
            outparamfile: None,
            outcostfile: None,
            fwhmc: 0.0,
            fwhmr: 0.0,
            fwhms: 0.0,
            smooth_ref: 0,
            sat_pct: 99.99,
            mov_oob_flag: 0,
            rusagefile: String::new(),
            optschema: 1,
            seed: 53,
            movoutfile: None,
            init_reg_save: None,
            init_reg_save_only: false,
            ltatype: LINEAR_VOX_TO_VOX,
            debug: 0,
            checkoptsonly: false,
            threads: None,
        }
    }
}

/// `printf("%s", s)` of a name that may be `NULL` (glibc prints `(null)`).
pub fn or_null(s: Option<&str>) -> &str {
    s.unwrap_or("(null)")
}

/// `print_usage()`, with the settings the options gave so far.
pub fn print_usage(out: &mut dyn Write, c: &CmdArgs) {
    let text = format!(
        "USAGE: {NAME} \n\n   --mov movvol : source volume\n   --ref refvol : target volume (can use --targ too)\n   --reg reg.lta : output registration (can use --lta too)\n\n   --s subject (forces --ref-mask aparc+aseg.mgz)\n   --dof DOF : default is {dof} (also: --6, --9, --12)\n   --zscale : a 7 dof reg with xyz shift and rot and scaling in z\n   --xztrans+yrot : for 2D images uses shifts in x and z and rot about y (no scale) (was --2dz)\n   --xytrans+zrot : for 2D images uses shifts in x and y and rot about z (no scale)\n   --xytrans+zrot+xyscale+xyshear : for 2D images uses shifts in x and y, rot about y, scale in xy, and xy shear \n   --ref-mask refmaskvol : mask ref with refmaskvol\n   --no-ref-mask : do not mask ref (good to undo aparc+aseg.mgz, put AFTER --s)\n   --mov-mask movmaskvol : mask ref with movmaskvol\n   --threads nthreads\n   --sd SUBJECTS_DIR \n   --regdat reg.dat \n   --no-coord-dither: turn off coordinate dithering\n   --no-intensity-dither: turn off intensity dithering\n   --sep voxsep1 <--sep voxsep2> : set spatial scales (def is 2 vox and 4 vox)\n   --trans Tx Ty Tz : initial translation in mm (implies --no-cras0)\n   --rot   Rx Ry Rz : initial rotation in deg\n   --scale Sx Sy Sz : initial scale\n   --shear Hxy Hxz Hyz : initial shear\n   --init-reg reg0.lta : initialize with given registration file\n   --params outparamfile : save parameters in this file\n   --final-cost outcostfile : save final cost value in this file\n   --no-cras0 : do not set translation parameters to align centers of mov and ref\n   --centroid : intialize by aligning centeroids of mov and ref\n   --regheader : same as no-cras0\n   --ras2ras : save output LTA as RAS2RAS (default is VOX2VOX)\n   --nitersmax n : default is {niters}\n   --ftol ftol : default is {ftol}\n   --linmintol linmintol : default is {linmintol}\n   --seed seed : set random seed for dithering\n   --sat SatPct : saturation threshold, default {sat}\n   --conf-ref : conform the refernece without rescaling (good for gca)\n   --no-bf : do not do brute force search\n   --bf-lim lim : constrain brute force search to +/-lim ({bflim})\n   --bf-nsamp nsamples : number of samples in brute force search\n   --no-smooth : do not apply smoothing to either ref or mov\n   --ref-fwhm fwhm : apply smoothing to ref\n   --mov-oob : count mov voxels that are out-of-bounds as 0\n   --no-mov-oob : do not count mov voxels that are out-of-bounds as 0 (default)\n   --init-reg-save initreg.lta : save initial registration\n   --init-reg-save-only initreg.lta : save initial registration and exit\n   --mat2par reg.lta : extract parameters out of RAS2RAS registration (lta does not need to be RAS)\n   --mat2par-vox reg.lta : extract parameters out of VOX2VOX registration (lta does not need to be VOX)\n   --mat2rot reg.lta rotreg.lta: convert registration to a pure rotation\n   --par2mat par1-par12 srcvol trgvol reg.lta : convert parameters to a  registration\n      the subject in the output reg.lta can be set with --s before --par2mat\n   --lrrev reg.lta reg.lrrev.lta : approx reg if you were to left-right rev the pix of the input image\n   --landmarks sxyz txyz coords mov targ outlta <xformedpointset>: convert landmarks to a registration\n       sxyz and txyz are source and target coordinates at matching landmarks\n       coords are RAS, VOX, or TKR\n       STVP format is created with mris_apply_reg --stvp via MRISapplyReg() (call with sxyzfile stvp)\n       For json pointset (eg, from mri_segcentroids) just make sure both have .json extenstion\n       If xformedpointset is specified, it will write out a point set of the sxyz transformed by the LTA\n   --landmarks-2d : same as --landmarks but extracts 2D (6dof) parameters into a 4x4 matrix\n   --rms radius filename reg1 reg2 : compute RMS diff between two registrations using MJ's method (rad ~= 50mm)\n      The rms will be written to filename; if filename == nofile, then no file is created\n   --movout movout volume : save the mov after all preprocessing\n   --mov-idither intensity dither volume : save the mov intensity dither volume\n\n   --debug     turn on debugging\n   --checkopts don't run anything, just check options and exit\n   --help      print out information on how to use this program\n   --version   print out version and exit\n\n{VCID}\n\n",
        dof = c.dof,
        niters = c.nitersmax,
        ftol = cfmt::pad(&cfmt::e(c.ftol, 3), 5),
        linmintol = cfmt::pad(&cfmt::e(c.linmintol, 3), 5),
        sat = cfmt::pad(&cfmt::e(c.sat_pct, 3), 5),
        bflim = cfmt::g(c.bf_lim, 6),
    );
    let _ = out.write_all(text.as_bytes());
}

/// `usage_exit()`: the usage, status 1.
pub fn usage_exit(out: &mut dyn Write, c: &CmdArgs) -> Fatal {
    print_usage(out, c);
    Fatal::Exit(1)
}

/// `print_help()`: the usage and three lines, status 1.
fn print_help(out: &mut dyn Write, c: &CmdArgs) -> Fatal {
    print_usage(out, c);
    let _ = out.write_all(
        b"This is a program that performs a linear registration between\ntwo volumes in a way that should more-or-less replicates spm_coreg\nas called by the FreeSurfer program spmregister. \n",
    );
    Fatal::Exit(1)
}

/// `print_version()`: the version, status 1.
fn print_version(out: &mut dyn Write) -> Fatal {
    let _ = writeln!(out, "{VCID}");
    Fatal::Exit(1)
}

/// `sscanf(s, "%d", &v)` into `v` (left as it was when none is read).
fn scan_d_into(g: &mut Globals, s: &str, v: &mut i32) {
    if let Some(x) = scan_d(g, s) {
        *v = x;
    }
}

/// `sscanf(s, "%lf", &v)` into `v`.
fn scan_lf_into(g: &mut Globals, s: &str, v: &mut f64) {
    if let Some(x) = scan_lf(g, s) {
        *v = x;
    }
}

/// `parse_commandline(argc, argv)` on the arguments after the program's name.
pub fn parse_commandline(
    g: &mut Globals,
    r: &mut Run,
    c: &mut CmdArgs,
    args: &[String],
) -> Result<(), Fatal> {
    if args.is_empty() {
        return Err(usage_exit(r.out, c));
    }
    let mut i = 0;
    while i < args.len() {
        let option = args[i].as_str();
        if c.debug != 0 {
            let _ = writeln!(r.out, "{} {option}", args.len() - i);
        }
        i += 1;
        let rest = &args[i..];
        let need = |n: usize, r: &mut Run| -> Result<(), Fatal> {
            if rest.len() < n {
                Err(cmd_arg_n_err(r.err, option, n as i32))
            } else {
                Ok(())
            }
        };
        let is = |name: &str| option.eq_ignore_ascii_case(name);
        let mut used = 0;
        if is("--help") {
            return Err(print_help(r.out, c));
        } else if is("--version") {
            return Err(print_version(r.out));
        } else if is("--debug") {
            c.debug = 1;
        } else if is("--checkopts") {
            c.checkoptsonly = true;
        } else if is("--nocheckopts") {
            c.checkoptsonly = false;
        } else if is("--init-cost-only") {
            need(1, r)?;
            c.do_init_cost_only = Some(rest[0].clone());
            used = 1;
        } else if is("--no-smooth") {
            c.do_smoothing = 0;
        } else if is("--cras0") {
            c.cras0 = 1;
            c.align_centroids = 0;
        } else if is("--centroid") {
            c.align_centroids = 1;
            c.cras0 = 0;
        } else if is("--no-cras0") || is("--regheader") {
            c.cras0 = 0;
        } else if is("--ras2ras") {
            c.ltatype = LINEAR_RAS_TO_RAS;
        } else if is("--conf-ref") {
            c.refconf = 1;
        } else if is("--bf") {
            c.do_bf = 1;
        } else if is("--no-bf") {
            c.do_bf = 0;
        } else if is("--mov-oob") {
            c.mov_oob_flag = 1;
        } else if is("--no-mov-oob") {
            c.mov_oob_flag = 0;
        } else if is("--rusage") {
            need(1, r)?;
            c.rusagefile = rest[0].clone();
            used = 1;
        } else if is("--seed") {
            need(1, r)?;
            scan_d_into(g, &rest[0], &mut c.seed);
            used = 1;
        } else if is("--mov") {
            need(1, r)?;
            c.mov = Some(rest[0].clone());
            used = 1;
        } else if is("--movout") {
            need(1, r)?;
            c.movoutfile = Some(rest[0].clone());
            used = 1;
        } else if is("--mov-idither") {
            need(1, r)?;
            c.movidither_file = Some(rest[0].clone());
            used = 1;
        } else if is("--ref") || is("--targ") {
            need(1, r)?;
            c.ref_vol = Some(rest[0].clone());
            used = 1;
        } else if is("--ref-mask") {
            need(1, r)?;
            c.refmask = Some(rest[0].clone());
            used = 1;
        } else if is("--no-ref-mask") {
            c.refmask = None;
        } else if is("--mov-mask") {
            need(1, r)?;
            c.movmask = Some(rest[0].clone());
            used = 1;
        } else if is("--reg") || is("--lta") {
            need(1, r)?;
            c.outreg = Some(rest[0].clone());
            used = 1;
        } else if is("--regdat") {
            need(1, r)?;
            c.regdat = Some(rest[0].clone());
            used = 1;
        } else if is("--params") {
            need(1, r)?;
            c.outparamfile = Some(rest[0].clone());
            used = 1;
        } else if is("--final-cost") {
            need(1, r)?;
            c.outcostfile = Some(rest[0].clone());
            used = 1;
        } else if is("--log-cost") {
            need(1, r)?;
            c.logcost = Some(rest[0].clone());
            used = 1;
        } else if is("--s") {
            need(1, r)?;
            c.subject = Some(rest[0].clone());
            c.refmask = Some("aparc+aseg.mgz".to_owned());
            used = 1;
        } else if option == "--sd" || option == "-SDIR" {
            need(1, r)?;
            r.env.set("SUBJECTS_DIR", rest[0].clone());
            used = 1;
        } else if is("--sat") {
            need(1, r)?;
            scan_lf_into(g, &rest[0], &mut c.sat_pct);
            used = 1;
        } else if is("--dof") {
            need(1, r)?;
            scan_d_into(g, &rest[0], &mut c.dof);
            c.optschema = 1;
            used = 1;
        } else if is("--xztrans+yrot") || is("--2dz") {
            c.optschema = 2;
            c.dof = 3;
        } else if is("--xytrans+zrot") {
            c.optschema = 4;
            c.dof = 3;
        } else if is("--xytrans+zrot+xyscale+xyshear") {
            c.optschema = 5;
            c.dof = 6;
        } else if is("--zscale") {
            c.optschema = 3;
            c.dof = 7;
        } else if is("--bf-lim") {
            need(1, r)?;
            c.do_bf = 1;
            scan_lf_into(g, &rest[0], &mut c.bf_lim);
            used = 1;
        } else if is("--bf-nsamp") {
            need(1, r)?;
            c.do_bf = 1;
            scan_d_into(g, &rest[0], &mut c.bf_nsamp);
            used = 1;
        } else if is("--6") {
            c.dof = 6;
        } else if is("--9") {
            c.dof = 9;
        } else if is("--12") {
            c.dof = 12;
        } else if is("--nitersmax") {
            need(1, r)?;
            scan_d_into(g, &rest[0], &mut c.nitersmax);
            used = 1;
        } else if is("--mat2par") || is("--mat2par-vox") {
            need(1, r)?;
            return Err(standalone::mat2par(g, r, &rest[0], is("--mat2par"))?);
        } else if is("--mat2rot") {
            need(2, r)?;
            return Err(standalone::mat2rot(g, r, &rest[0], &rest[1])?);
        } else if is("--par2mat") {
            need(15, r)?;
            return Err(standalone::par2mat(g, r, c, &rest[..15])?);
        } else if is("--lrrev") {
            need(2, r)?;
            return Err(standalone::lrrev(g, r, &rest[0], &rest[1])?);
        } else if is("--landmarks") || is("--landmarks-2d") {
            need(6, r)?;
            return Err(standalone::landmarks(g, r, option));
        } else if is("--rms") {
            if rest.len() < 2 {
                return Err(cmd_arg_n_err(r.err, option, 4));
            }
            return Err(standalone::rms(g, r, rest)?);
        } else if is("--ftol") {
            need(1, r)?;
            scan_lf_into(g, &rest[0], &mut c.ftol);
            used = 1;
        } else if is("--linmintol") {
            need(1, r)?;
            scan_lf_into(g, &rest[0], &mut c.linmintol);
            used = 1;
        } else if is("--sep") {
            need(1, r)?;
            if c.seplist.len() >= MAX_SEP {
                g.notices.add(
                    &FS_1313,
                    format!(
                        "mri_coreg: more than {MAX_SEP} --sep (FreeSurfer writes past its list of {MAX_SEP})"
                    ),
                );
                let _ = writeln!(
                    r.err,
                    "{NAME}: more than {MAX_SEP} --sep: FreeSurfer keeps at most {MAX_SEP} (it writes past its list)"
                );
                return Err(Fatal::Exit(1));
            }
            // `sscanf` into the next slot, which `calloc` left 0
            let mut v = 0;
            scan_d_into(g, &rest[0], &mut v);
            c.seplist.push(v);
            used = 1;
        } else if is("--ref-fwhm") {
            need(1, r)?;
            scan_lf_into(g, &rest[0], &mut c.fwhmc);
            c.fwhmr = c.fwhmc;
            c.fwhms = c.fwhmc;
            c.smooth_ref = 1;
            c.do_smoothing = 0;
            used = 1;
        } else if is("--trans") {
            need(3, r)?;
            for (k, a) in rest.iter().take(3).enumerate() {
                scan_lf_into(g, a, &mut c.params[k]);
            }
            c.cras0 = 0;
            used = 3;
        } else if is("--rot") {
            need(3, r)?;
            for (k, a) in rest.iter().take(3).enumerate() {
                scan_lf_into(g, a, &mut c.params[3 + k]);
            }
            used = 3;
        } else if is("--scale") {
            need(3, r)?;
            for (k, a) in rest.iter().take(3).enumerate() {
                scan_lf_into(g, a, &mut c.params[6 + k]);
            }
            used = 3;
        } else if is("--shear") {
            need(3, r)?;
            for (k, a) in rest.iter().take(3).enumerate() {
                scan_lf_into(g, a, &mut c.params[9 + k]);
            }
            used = 3;
        } else if is("--init-reg") {
            need(1, r)?;
            standalone::init_reg(g, r, c, &rest[0])?;
            c.cras0 = 0;
            used = 1;
        } else if is("--init-reg-save") {
            need(1, r)?;
            c.init_reg_save = Some(rest[0].clone());
            used = 1;
        } else if is("--init-reg-save-only") {
            need(1, r)?;
            c.init_reg_save = Some(rest[0].clone());
            c.init_reg_save_only = true;
            used = 1;
        } else if option == "--no-coord-dither" {
            c.do_coord_dither = 0;
        } else if option == "--no-intensity-dither" {
            c.do_intensity_dither = 0;
        } else if option == "--no-dither" {
            c.do_coord_dither = 0;
            c.do_intensity_dither = 0;
        } else if is("--threads") || is("--nthreads") {
            need(1, r)?;
            match scan_d(g, &rest[0]) {
                Some(n) => c.threads = Some(n),
                None => g.notices.add(
                    &FS_1314,
                    format!(
                        "mri_coreg {option} {}: no number (FreeSurfer sets OpenMP's thread count from an int it never wrote); the count left as it was",
                        rest[0]
                    ),
                ),
            }
            used = 1;
        } else if is("--diag") {
            need(1, r)?;
            let mut v = g.gdiag_no;
            scan_d_into(g, &rest[0], &mut v);
            g.gdiag_no = v;
            used = 1;
        } else if is("--diag-show") {
            g.gdiag &= larmorx_freesurfer_core::diag::DIAG_SHOW;
        } else if is("--diag-verbose") {
            g.gdiag &= larmorx_freesurfer_core::diag::DIAG_VERBOSE;
        } else {
            let _ = writeln!(r.err, "ERROR: Option {option} unknown");
            if cmd_single_dash(option) {
                let _ = writeln!(r.err, "       Did you really mean -{option} ?");
            }
            return Err(Fatal::Exit(-1));
        }
        i += used;
    }
    Ok(())
}

/// `check_options()` (after `FSENVgetenv`, whose `SUBJECTS_DIR` it reads: `subjects_dir`, its
/// record `None` when it could not be made).
pub fn check_options(
    g: &mut Globals,
    r: &mut Run,
    c: &mut CmdArgs,
    subjects_dir: Option<Option<&str>>,
) -> Result<(), Fatal> {
    let Some(mov) = c.mov.as_deref() else {
        let _ = writeln!(r.out, "ERROR: must spec --mov");
        return Err(Fatal::Exit(1));
    };
    let _ = mov;
    if let Some(subject) = c.subject.clone() {
        if c.ref_vol.is_none() {
            c.ref_vol = Some("brainmask.mgz".to_owned());
        }
        let ref_vol = c.ref_vol.clone().expect("set above");
        if fio_file_exists_readable(g, r.dir, &ref_vol) == 0 {
            c.ref_vol = Some(subject_file(g, r, subjects_dir, &subject, &ref_vol)?);
        }
        if let Some(mask) = c.refmask.clone()
            && fio_file_exists_readable(g, r.dir, &mask) == 0
        {
            c.refmask = Some(subject_file(g, r, subjects_dir, &subject, &mask)?);
        }
    }
    if c.ref_vol.is_none() {
        let _ = writeln!(r.out, "ERROR: must spec --ref");
        return Err(Fatal::Exit(1));
    }
    if c.outreg.is_none() {
        let _ = writeln!(r.out, "ERROR: must spec --reg");
        return Err(Fatal::Exit(1));
    }
    if c.seplist.is_empty() {
        c.seplist = vec![4, 2];
    }
    Ok(())
}

/// `sprintf(tmpstr, "%s/%s/mri/%s", fsenv->SUBJECTS_DIR, subject, name)`: FS-1315 for no
/// environment record (FreeSurfer reads through a `NULL` record and crashes) and for a name of
/// 3,000 bytes or more (it writes past `tmpstr[3000]`).
fn subject_file(
    g: &mut Globals,
    r: &mut Run,
    subjects_dir: Option<Option<&str>>,
    subject: &str,
    name: &str,
) -> Result<String, Fatal> {
    let Some(sd) = subjects_dir else {
        return Err(super::main::fs_1315(
            g,
            r,
            "--s without FreeSurfer's environment record (FREESURFER_HOME and its colour table): FreeSurfer reads through the missing record and crashes",
        ));
    };
    let path = format!("{}/{subject}/mri/{name}", or_null(sd));
    if path.len() >= 3000 {
        return Err(super::main::fs_1315(
            g,
            r,
            "--s: a subject file's name of 3,000 bytes or more (FreeSurfer writes past its buffer of 3,000)",
        ));
    }
    Ok(path)
}

/// The machine's and the run's names `dump_options` prints.
pub struct RunInfo {
    pub cwd: String,
    pub cmdline: String,
    pub sysname: String,
    pub hostname: String,
    pub machine: String,
    pub user: String,
}

/// `dump_options(stdout)`.
pub fn dump_options(fp: &mut dyn Write, c: &CmdArgs, info: &RunInfo) {
    let lf = |v: f64| cfmt::f(v, 6);
    let _ = writeln!(fp);
    let _ = writeln!(fp, "{VCID}");
    let _ = writeln!(fp, "cwd {}", info.cwd);
    let _ = writeln!(fp, "cmdline {}", info.cmdline);
    let _ = writeln!(fp, "sysname  {}", info.sysname);
    let _ = writeln!(fp, "hostname {}", info.hostname);
    let _ = writeln!(fp, "machine  {}", info.machine);
    let _ = writeln!(fp, "user     {}", info.user);
    let _ = writeln!(fp, "dof    {}", c.dof);
    let _ = writeln!(fp, "nsep    {}", c.seplist.len());
    let _ = writeln!(fp, "cras0    {}", c.cras0);
    let _ = writeln!(fp, "ftol    {}", lf(c.ftol));
    let _ = writeln!(fp, "linmintol    {}", lf(c.linmintol));
    let _ = writeln!(fp, "bf       {}", c.do_bf);
    let _ = writeln!(fp, "bflim    {}", lf(c.bf_lim));
    let _ = writeln!(fp, "bfnsamp    {}", c.bf_nsamp);
    let _ = writeln!(fp, "SmoothRef {}", c.smooth_ref);
    let _ = writeln!(fp, "SatPct    {}", lf(c.sat_pct));
    let _ = writeln!(fp, "MovOOB {}", c.mov_oob_flag);
    let _ = writeln!(fp, "optschema {}", c.optschema);
}
