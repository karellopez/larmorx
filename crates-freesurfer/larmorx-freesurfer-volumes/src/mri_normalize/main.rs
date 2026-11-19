// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_normalize`'s `main` (`mri_normalize.cpp` 162-994, queue item 3.2-25): the program's
//! entry point [`run`] (registered as `mri_normalize` in the crate's `PROGRAMS`).
//!
//! The order of `main`: the command line kept for the output's header (`getAllInfo`), the version
//! options, `FSinit` and `DiagInit`; the options ([`super::options`]); fewer than two names: the
//! help (status 0). The input read (`reading mri_src from <name>...`) and the command line added
//! to it. Then, in order, what the options ask for: `-atlas`'s mask of the voxels outside the
//! brain (the atlas and its transform freed after it); `-lonly`'s label control points; `-renorm`
//! (its own program: the volume written and the end); `-surface` (its own program, ending with
//! `#VMPC# mri_normalize VmPeak <kB>` and `mri_normalize done`); `-conform`; `-mask`; `-r` (its own
//! program); `-l`; `-grad`. Then the input as `float` (`MRIchangeType`, no scaling), thresholded
//! at 0 when conformed; `-aseg` read (resampled to the input when its extent or type differs);
//! the timer started; `-f`'s control points; `-w`'s control-point volume name; the first pass
//! (`-gentle`'s control points from the file or the label, else a copy); with `-aseg` the white
//! matter's control points, their outliers removed, the bias field built, smoothed and applied;
//! without it the one-dimensional normalisation (`MRInormInit`, `MRInormalize`; on failure, or
//! with `-no1d`, the copy, the surface interiors or the high-signal normalisation); the gentle
//! three-dimensional pass; `num_3d_iter` passes of `MRI3dNormalize` (or of the gentle one); `-w`'s
//! bias field; a conformed float result clamped to [0, 255]; the result as `uchar`; written; `3D
//! bias adjustment took <m> minutes and <s> seconds.` from the timer.
//!
//! FS-1017: a step that returns no volume where `main` goes on with one (`MRIconform` and
//! `MRIresample` failing, `-no1d` with surface interiors after a failed one-dimensional pass,
//! `-r` without its names) makes FreeSurfer dereference `NULL`; both modes end with
//! `mri_normalize: <step> gave no volume (FreeSurfer crashes here)` (status 1, rule B).

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::FS_1017;
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::diag::{DIAG_WRITE, diag_init, verbose_on};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADPARM, ERROR_NO_FILE, ERROR_NOFILE, ERROR_UNSUPPORTED, Fatal,
};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::sse::{div, div64, mul64};
use larmorx_freesurfer_core::timer::Timer;
use larmorx_freesurfer_core::utils::{get_vm_peak, nint, stricmp};
use larmorx_freesurfer_core::version::{get_all_info, handle_version_option};
use larmorx_freesurfer_filter::conv::{mri_convolve_gaussian, mri_gaussian_smooth, mri_gaussian1d};
use larmorx_freesurfer_filter::norm::{CONTROL_MARKED, CONTROL_NONE, mri_soap_bubble};
use larmorx_freesurfer_filter::normalize::{
    DEFAULT_DESIRED_WHITE_MATTER_VALUE, Dst, Mni, NormState, mri_3d_gentle_normalize,
    mri_3d_normalize, mri_3d_use_file_control_points, mri_3d_use_label_control_points,
    mri_3d_write_control_points, mri_apply_bias_correction_same_geometry, mri_build_bias_image,
    mri_norm_add_file_control_points, mri_norm_init, mri_normalize,
    mri_normalize_high_signal_low_std,
};
use larmorx_freesurfer_filter::sobel::mri_sobel;
use larmorx_freesurfer_gca::surftransform::{Volumes, mris_transform};
use larmorx_freesurfer_gca::transform::transform_invert;
use larmorx_freesurfer_io::mriio::{
    mri_add_command_line, mri_read_volume, mri_remove_nans, mri_write,
};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_UCHAR, Mri, SAMPLE_NEAREST, SAMPLE_TRILINEAR};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io as SurfIo;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::readwrite;
use larmorx_freesurfer_surf::surfmri::compute_distance_to_surface;
use larmorx_freesurfer_volume::arith::{
    Pair, Tri, mri_binarize, mri_clone, mri_match, mri_match_dimensions, mri_multiply,
    mri_scalar_mul, mri_threshold,
};
use larmorx_freesurfer_volume::combine::mri_combine_distance_transforms;
use larmorx_freesurfer_volume::conform::mri_conform;
use larmorx_freesurfer_volume::conformed::mri_conformed;
use larmorx_freesurfer_volume::deriv::mri_non_max_suppress;
use larmorx_freesurfer_volume::frames::mri_clone_different_type;
use larmorx_freesurfer_volume::label_coords::Io as LabelIo;
use larmorx_freesurfer_volume::labels::{mri_erase_border_planes, mri_mean_in_label};
use larmorx_freesurfer_volume::mriset::{mri_copy_label, mri_erode, mri_mask};
use larmorx_freesurfer_volume::resample::mri_resample;
use larmorx_freesurfer_volume::resize::mri_change_type;

use super::helpers::{self as h, Run};
use super::options::{
    MRI_MGH_MPRAGE, MRI_WASHU_MPRAGE, Options, T2_CONTRAST, parse_options, usage_exit,
};

/// The program's name as `argv[0]` gives it (recon-all runs it by name).
pub const NAME: &str = "mri_normalize";

/// `aseg_wm_labels`: `Left_Cerebral_White_Matter`, `Right_Cerebral_White_Matter`.
const ASEG_WM_LABELS: [i32; 2] = [2, 41];

/// `DEFAULT_DESIRED_WHITE_MATTER_VALUE` as the `float` the normalisations take.
const WM: f32 = DEFAULT_DESIRED_WHITE_MATTER_VALUE as f32;

/// `mri_normalize`'s entry point: its original arguments (without the program's name), the
/// settings, the two streams; the exit status. The notices of the run follow on standard error
/// (`larmorx: ...`).
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut g = Globals::new(settings.mode);
    let status = match main(&mut g, args, settings, out, err) {
        Ok(()) => 0,
        Err(f) => f.status(),
    };
    let _ = out.flush();
    g.notices.print(err);
    status
}

/// The state of `main`: its streams, environment and folder, the options, `mrinorm.cpp`'s state,
/// `mni` and `mri_not_control`.
struct Prog<'a> {
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
    env: &'a Environment,
    dir: Option<&'a Path>,
    o: Options,
    st: NormState,
    mni: Mni,
    not_control: Option<Mri>,
}

impl Prog<'_> {
    fn run(&mut self) -> Run<'_, '_> {
        Run {
            out: &mut *self.out,
            err: &mut *self.err,
            env: self.env,
            dir: self.dir,
        }
    }

    fn read(&mut self, g: &mut Globals, fname: &str) -> Result<Option<Mri>, Fatal> {
        mri_read_volume(g, self.out, self.err, self.env, self.dir, fname)
    }

    fn write(&mut self, g: &mut Globals, m: &mut Mri, fname: &str) -> Result<(), Fatal> {
        mri_write(g, self.out, self.err, self.env, self.dir, m, fname, None, 0)?;
        Ok(())
    }

    fn read_surface(&mut self, g: &mut Globals, fname: &str) -> Result<Option<Mris>, Fatal> {
        let mut sg = SurfGlobals::default();
        let mut io = SurfIo {
            out: &mut *self.out,
            err: &mut *self.err,
            env: self.env,
        };
        readwrite::read(g, &mut sg, &mut io, self.dir, fname, true)
    }

    /// `ErrorExit(code, message)`.
    fn exit(&mut self, g: &mut Globals, code: i32, message: &str) -> Result<(), Fatal> {
        log::error_exit(g, self.err, code, message)
    }
}

/// `%<w>.<p>f`.
fn ff(v: f64, w: usize, p: usize) -> String {
    cfmt::pad(&cfmt::f(v, p), w)
}

/// FS-1017: a volume `main` uses after a step that gave none.
fn need<T>(g: &mut Globals, err: &mut dyn Write, v: Option<T>, step: &str) -> Result<T, Fatal> {
    match v {
        Some(v) => Ok(v),
        None => {
            g.notices.add(
                &FS_1017,
                format!("mri_normalize: {step} gave no volume, which FreeSurfer uses (a crash)"),
            );
            let _ = writeln!(
                err,
                "mri_normalize: {step} gave no volume (FreeSurfer crashes here)"
            );
            Err(Fatal::Exit(1))
        }
    }
}

/// A function's volume: the new one, or the caller's when it filled it.
fn kept(r: Ret<Mri>, given: Mri) -> Option<Mri> {
    match r {
        Ret::New(m) => Some(m),
        Ret::Given => Some(given),
        Ret::Null => None,
    }
}

/// `(int)` of a `float` argument passed to an `int` parameter.
fn to_int(g: &mut Globals, v: f32, what: &str) -> i32 {
    float_to_int(g, f64::from(v), what)
}

/// `1.25 * intensity_below` (a `double`) passed as a `float`.
fn below125(o: &Options) -> f32 {
    mul64(1.25, f64::from(o.intensity_below)) as f32
}

fn diag_write_verbose(g: &Globals) -> bool {
    g.gdiag & DIAG_WRITE != 0 && verbose_on(g)
}

fn main(
    g: &mut Globals,
    args: &[String],
    settings: &Settings,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), Fatal> {
    let env = &settings.env;
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let cmdline = get_all_info(g, env, &argv, NAME);
    let (nargs, left) = handle_version_option(g, env, out, &argv, NAME);
    if nargs != 0 && left.len() == 1 {
        return Err(Fatal::Exit(0));
    }
    // the arguments after the program's name
    let left = &left[1..];
    g.progname = NAME.to_owned();
    diag_init(g, env, out);
    let mut start = Timer::new();
    let mut p = Prog {
        out,
        err,
        env,
        dir: settings.dir.as_deref(),
        o: Options::default(),
        st: NormState::default(),
        mni: Mni::default(),
        not_control: None,
    };
    p.mni.max_gradient = p.o.max_gradient;
    let first = {
        let mut r = p.run();
        let mut o = Options::default();
        let i = parse_options(g, &mut r, &mut o, left)?;
        p.o = o;
        i
    };
    p.mni.max_gradient = p.o.max_gradient;
    let names = &left[first..];
    if names.len() < 2 {
        let mut r = p.run();
        return Err(usage_exit(g, &mut r, 0));
    }
    let (in_fname, out_fname) = (names[0].clone(), names[1].clone());

    let _ = writeln!(p.out, "reading mri_src from {in_fname}...");
    let src = p.read(g, &in_fname)?;
    let Some(mut src) = src else {
        let m = format!("{}: could not open source file {in_fname}", g.progname);
        p.exit(g, ERROR_NO_FILE, &m)?;
        return Ok(());
    };
    mri_add_command_line(g, p.err, &mut src, cmdline.as_bytes());

    if let Some(gca) = p.o.gca.take() {
        // atlas and transform given: the voxels that cannot have control points
        let mut xform = p.o.xform.take().expect("-atlas reads its transform");
        let mask = h::build_outside_of_brain_mask(
            g,
            p.out,
            p.err,
            p.env,
            &src,
            &gca,
            &mut xform,
            p.o.brain_distance,
        )?;
        p.not_control = Some(mask);
    }

    if let Some(label) = p.o.control_point_label.take() {
        let mut io = LabelIo {
            out: &mut *p.out,
            err: &mut *p.err,
            env: p.env,
        };
        mri_3d_use_label_control_points(g, &mut io, &mut p.st, &mut src, &label)?;
        p.o.control_point_label = Some(label);
    }

    if let Some(renorm) = p.o.renorm_fname.clone() {
        return renormalize(g, &mut p, &src, &renorm, &out_fname);
    }

    if !p.o.surfaces.is_empty() {
        return with_surfaces(g, &mut p, &mut src, &out_fname);
    }

    if mri_conformed(g, &src.vg) == 0 && p.o.conform > 0 {
        let _ = writeln!(p.out, "unconformed source detected - conforming...");
        let c = mri_conform(g, p.out, p.err, p.env, &src)?;
        src = need(g, p.err, c, "MRIconform")?;
    }

    if let Some(mask_fname) = p.o.mask_fname.clone() {
        let Some(mask) = p.read(g, &mask_fname)? else {
            let m = format!("{}: could not open mask volume {mask_fname}.\n", g.progname);
            p.exit(g, ERROR_NOFILE, &m)?;
            return Ok(());
        };
        mri_mask(g, p.out, p.err, p.env, Pair::Same(&mut src), &mask, 0, 0.0)?;
    }

    if p.o.read_flag != 0 {
        return read_bias(g, &mut p, &src, &out_fname);
    }

    let mut dst: Option<Mri> = None;
    if p.o.long_flag != 0 {
        dst = Some(longitudinal(g, &mut p, &src)?);
    }

    if p.o.grad_thresh > 0.0 {
        gradient_mask(g, &mut p, &src)?;
    }

    if src.mtype != MRI_FLOAT {
        let t = mri_change_type(g, p.out, p.err, p.env, &src, MRI_FLOAT, 0.0, 255.0, 1)?;
        src = need(g, p.err, t, "MRIchangeType")?;
    }

    if mri_conformed(g, &src.vg) != 0 {
        mri_threshold(g, p.out, p.err, Pair::Same(&mut src), 0.0)?;
    }

    let mut aseg: Option<Mri> = None;
    if let Some(aseg_fname) = p.o.aseg_fname.clone() {
        let _ = writeln!(p.out, "Reading aseg {aseg_fname}");
        let Some(a) = p.read(g, &aseg_fname)? else {
            let m = format!("{}: could not read aseg from file {aseg_fname}", g.progname);
            p.exit(g, ERROR_NOFILE, &m)?;
            return Ok(());
        };
        if p.o.conform != 0 && mri_conformed(g, &a.vg) == 0 {
            let m = format!("{}: aseg volume {aseg_fname} must be conformed", g.progname);
            p.exit(g, ERROR_UNSUPPORTED, &m)?;
        }
        let _ = writeln!(
            p.out,
            "aseg read with width {} (src width {})",
            a.vg.width, src.vg.width
        );
        aseg = Some(if mri_match(&a, &src) == 0 {
            let _ = writeln!(
                p.out,
                "************** resampling aseg to account for mismatch with source image ***************"
            );
            let t = mri_resample(g, p.out, p.err, &a, &src, SAMPLE_NEAREST)?;
            need(g, p.err, t, "MRIresample")?
        } else {
            a
        });
    }

    if p.o.verbose != 0 {
        let _ = writeln!(p.out, "normalizing image...");
    }
    let _ = p.out.flush();

    start.reset();

    if let Some(cp) = p.o.control_point_fname.clone() {
        mri_3d_use_file_control_points(g, p.out, p.err, p.env, p.dir, &mut p.st, &mut src, &cp)?;
    }
    if let Some(cv) = p.o.control_volume_fname.clone() {
        mri_3d_write_control_points(&mut p.st, Some(&cv));
    }

    // first a gentle normalisation to get things in the right intensity range
    if p.o.long_flag == 0 {
        if p.o.do_gentle_cp_file != 0
            && (p.o.control_point_fname.is_some() || p.o.control_point_label.is_some())
        {
            let _ = writeln!(
                p.out,
                "Doing gentle normalization with control points/label"
            );
            let ib2 = div(p.o.intensity_below, 2.0);
            let r = mri_3d_gentle_normalize(
                g,
                p.out,
                p.err,
                p.env,
                p.dir,
                &mut p.st,
                &mut src,
                None,
                WM,
                Dst::New,
                p.o.intensity_above,
                ib2,
                1,
                p.o.bias_sigma,
                p.not_control.as_ref(),
            )?;
            dst = r.into_new();
        } else {
            let _ = writeln!(
                p.out,
                "NOT doing gentle normalization with control points/label"
            );
            dst = Some(mri_copy(g, p.out, p.err, &src, None)?.unwrap_new());
        }
    }
    let _ = p.out.flush();

    if let Some(a) = aseg.take() {
        let d = need(g, p.err, dst.take(), "the first pass")?;
        dst = Some(with_aseg(g, &mut p, &src, d, a)?);
    } else {
        dst = without_aseg(g, &mut p, &mut src, dst)?;
    }
    let _ = p.out.flush();

    let mut dst = need(g, p.err, dst, "the initial normalization")?;
    let (ia, ib) = (p.o.intensity_above, p.o.intensity_below);
    if p.o.file_only == 0 {
        mri_3d_gentle_normalize(
            g,
            p.out,
            p.err,
            p.env,
            p.dir,
            &mut p.st,
            &mut dst,
            None,
            WM,
            Dst::Same,
            ia,
            div(ib, 2.0),
            p.o.file_only,
            p.o.bias_sigma,
            p.not_control.as_ref(),
        )?;
    }

    let mut orig = mri_copy(g, p.out, p.err, &dst, None)?.unwrap_new();
    let _ = writeln!(p.out);
    let _ = writeln!(p.out, "Iterating {} times", p.o.num_3d_iter);
    let mut n = 0;
    while n < p.o.num_3d_iter {
        if p.o.file_only != 0 {
            break;
        }
        let _ = writeln!(p.out, "---------------------------------");
        let _ = writeln!(
            p.out,
            "3d normalization pass {} of {}",
            n + 1,
            p.o.num_3d_iter
        );
        if p.o.gentle_flag != 0 {
            mri_3d_gentle_normalize(
                g,
                p.out,
                p.err,
                p.env,
                p.dir,
                &mut p.st,
                &mut dst,
                None,
                WM,
                Dst::Same,
                div(ia, 2.0),
                div(ib, 2.0),
                p.o.file_only,
                p.o.bias_sigma,
                p.not_control.as_ref(),
            )?;
        } else {
            mri_3d_normalize(
                g,
                p.out,
                p.err,
                p.env,
                p.dir,
                &mut p.st,
                &mut orig,
                &mut dst,
                WM,
                Dst::Same,
                ia,
                ib,
                p.o.file_only,
                p.o.prune,
                p.o.bias_sigma,
                p.o.scan_type,
                p.not_control.as_ref(),
            )?;
        }
        n += 1;
    }
    let _ = writeln!(p.out, "Done iterating ---------------------------------");
    drop(orig);

    if let Some(cv) = p.o.control_volume_fname.clone() {
        mri_3d_write_control_points(&mut p.st, Some(&cv));
    }

    if let Some(bf) = p.o.bias_volume_fname.clone() {
        let mut bias = h::compute_bias(g, p.out, p.err, &src, &dst)?;
        let _ = writeln!(p.out, "writing bias field to {bf}....");
        p.write(g, &mut bias, &bf)?;
    }

    if dst.mtype != MRI_UCHAR {
        if mri_conformed(g, &dst.vg) != 0 {
            clamp_to_bytes(g, p.err, &mut dst)?;
        }
        let t = mri_change_type(g, p.out, p.err, p.env, &dst, MRI_UCHAR, 0.0, 255.0, 1)?;
        dst = need(g, p.err, t, "MRIchangeType")?;
    }
    if p.o.verbose != 0 {
        let _ = writeln!(p.out, "writing output to {out_fname}");
    }
    p.write(g, &mut dst, &out_fname)?;
    let msec = start.milliseconds();
    drop(src);
    drop(dst);
    let seconds = nint(g, f64::from(div(msec as f32, 1000.0)));
    let (minutes, seconds) = (seconds / 60, seconds % 60);
    let _ = writeln!(
        p.out,
        "3D bias adjustment took {minutes} minutes and {seconds} seconds."
    );
    Err(Fatal::Exit(0))
}

/// The conformed result's clamp: each voxel (x outermost, z innermost) below 0 set to 0, above
/// 255 to 255, written back (`MRIgetVoxVal`, `MRIsetVoxVal`).
fn clamp_to_bytes(g: &mut Globals, err: &mut dyn Write, m: &mut Mri) -> Result<(), Fatal> {
    for x in 0..m.vg.width {
        for y in 0..m.vg.height {
            for z in 0..m.vg.depth {
                let mut v = mri_get_vox_val(g, err, m, x, y, z, 0, 0)?;
                if v < 0.0 {
                    v = 0.0;
                } else if v > 255.0 {
                    v = 255.0;
                }
                mri_set_vox_val(g, err, m, x, y, z, 0, v, 0)?;
            }
        }
    }
    Ok(())
}

/// The bias field smoothed in place with a Gaussian of `bias_sigma` (`MRIgaussian1d(sigma, -1)`,
/// `MRIconvolveGaussian(bias, bias, kernel)`).
fn smooth_bias(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    bias: &mut Mri,
    sigma: f32,
) -> Result<(), Fatal> {
    let kernel = mri_gaussian1d(g, out, err, sigma, -1)?;
    mri_convolve_gaussian(g, out, err, Pair::Same(bias), &kernel)?;
    Ok(())
}

/// `-renorm`'s program: the volume (resampled trilinearly when its geometry differs, `r.mgz`),
/// its voxels of `renorm_val` as control points (`c.mgz`; eroded `erode` times, `e.mgz`; the
/// file's or label's points added), the bias field (`b.mgz`; smoothed, `bs.mgz`), applied to the
/// input; the result written; the end (status 0).
fn renormalize(
    g: &mut Globals,
    p: &mut Prog,
    src: &Mri,
    renorm_fname: &str,
    out_fname: &str,
) -> Result<(), Fatal> {
    let Some(mut renorm) = p.read(g, renorm_fname)? else {
        let m = format!(
            "{}: could not read renormalization file {renorm_fname}\n",
            g.progname
        );
        p.exit(g, ERROR_NOFILE, &m)?;
        return Ok(());
    };
    if mri_match_dimensions(src, &renorm) == 0 {
        let _ = writeln!(p.out, "resampling renormalization volume");
        let t = mri_resample(g, p.out, p.err, &renorm, src, SAMPLE_TRILINEAR)?;
        let mut t = need(g, p.err, t, "MRIresample")?;
        p.write(g, &mut t, "r.mgz")?;
        renorm = t;
    }
    let mut ctrl = mri_clone_different_type(g, p.out, p.err, &renorm, MRI_UCHAR)?;
    mri_copy_label(g, p.err, &renorm, &mut ctrl, p.o.renorm_val)?;
    mri_binarize(
        g,
        p.out,
        p.err,
        Pair::Same(&mut ctrl),
        p.o.renorm_val as f32,
        CONTROL_NONE as f32,
        CONTROL_MARKED as f32,
    )?;
    p.write(g, &mut ctrl, "c.mgz")?;
    for _ in 0..p.o.erode.max(0) {
        let t = mri_erode(g, p.out, p.err, Pair::New(&ctrl))?.unwrap_new();
        mri_copy(g, p.out, p.err, &t, Some(&mut ctrl))?;
    }
    p.write(g, &mut ctrl, "e.mgz")?;
    if p.o.control_point_fname.is_some() || p.o.control_point_label.is_some() {
        mri_norm_add_file_control_points(
            g,
            p.out,
            p.err,
            &mut p.st,
            &mut ctrl,
            CONTROL_MARKED,
            Some(src),
        )?;
    }
    let mut bias = mri_build_bias_image(g, p.out, p.err, src, &mut ctrl, 0.0)?;
    p.write(g, &mut bias, "b.mgz")?;
    if p.o.bias_sigma > 0.0 {
        let kernel = mri_gaussian1d(g, p.out, p.err, p.o.bias_sigma, -1)?;
        if diag_write_verbose(g) {
            p.write(g, &mut bias, "b.mgz")?;
        }
        let _ = writeln!(
            p.out,
            "smoothing bias field with sigma={}",
            ff(f64::from(p.o.bias_sigma), 2, 3)
        );
        mri_convolve_gaussian(g, p.out, p.err, Pair::Same(&mut bias), &kernel)?;
        if diag_write_verbose(g) {
            p.write(g, &mut bias, "bs.mgz")?;
        }
        p.write(g, &mut bias, "bs.mgz")?;
    }
    let r = mri_apply_bias_correction_same_geometry(g, p.out, p.err, Pair::New(src), &bias, WM)?;
    let mut dst = need(g, p.err, r.into_new(), "MRIapplyBiasCorrectionSameGeometry")?;
    let _ = writeln!(p.out, "writing normalized volume to {out_fname}");
    p.write(g, &mut dst, out_fname)?;
    Err(Fatal::Exit(0))
}

/// `-surface`'s program (`mri_normalize.cpp` 281-505).
fn with_surfaces(
    g: &mut Globals,
    p: &mut Prog,
    src: &mut Mri,
    out_fname: &str,
) -> Result<(), Fatal> {
    let (ia, ib) = (p.o.intensity_above, p.o.intensity_below);
    let mut dst = if p.o.control_point_fname.is_some() || p.o.control_point_label.is_some() {
        // one pass with only the file's control points first
        if let Some(cp) = p.o.control_point_fname.clone() {
            mri_3d_use_file_control_points(g, p.out, p.err, p.env, p.dir, &mut p.st, src, &cp)?;
        } else if let Some(label) = p.o.control_point_label.take() {
            let mut io = LabelIo {
                out: &mut *p.out,
                err: &mut *p.err,
                env: p.env,
            };
            mri_3d_use_label_control_points(g, &mut io, &mut p.st, src, &label)?;
            p.o.control_point_label = Some(label);
        }
        let r = mri_3d_gentle_normalize(
            g,
            p.out,
            p.err,
            p.env,
            p.dir,
            &mut p.st,
            src,
            None,
            WM,
            Dst::New,
            ia,
            div(ib, 2.0),
            1,
            p.o.bias_sigma,
            p.not_control.as_ref(),
        )?;
        need(g, p.err, r.into_new(), "MRI3dGentleNormalize")?
    } else {
        let _ = writeln!(p.out, "Copying mri_dst from mri_src");
        mri_copy(g, p.out, p.err, src, None)?.unwrap_new()
    };

    let mut dist: Option<Mri> = None;
    let surfaces = std::mem::take(&mut p.o.surfaces);
    for (i, s) in surfaces.into_iter().enumerate() {
        let Some(mut mris) = p.read_surface(g, &s.fname)? else {
            let m = format!("{}: could not surface {}", g.progname, s.fname);
            p.exit(g, ERROR_NOFILE, &m)?;
            return Ok(());
        };
        let mut xform = s.xform;
        transform_invert(g, p.out, p.err, p.env, p.dir, &mut xform, None)?;
        if stricmp(g, &s.xform_fname, "identity.nofile") != 0 {
            mris_transform(
                g,
                p.out,
                p.err,
                p.env,
                &mut mris,
                Volumes::Separate(None, None),
                &mut xform,
            )?;
        }
        let mut one = mri_clone_different_type(g, p.out, p.err, &dst, MRI_FLOAT)?;
        let _ = writeln!(p.out, "computing distance transform");
        let xsize = one.vg.xsize;
        {
            let mut sg = SurfGlobals::default();
            let mut io = SurfIo {
                out: &mut *p.out,
                err: &mut *p.err,
                env: p.env,
            };
            compute_distance_to_surface(g, &mut sg, &mut io, &mut mris, Some(&mut one), xsize)?;
        }
        dist = Some(match (i, dist.take()) {
            (0, _) | (_, None) => mri_copy(g, p.out, p.err, &one, None)?.unwrap_new(),
            (_, Some(d)) => {
                // MRIcombineDistanceTransforms(one, dist, dist): voxel by voxel, so a new volume
                // of the same geometry holds the same values
                mri_combine_distance_transforms(g, p.out, p.err, &one, &d, None)?.unwrap_new()
            }
        });
    }
    let mut dist = dist.expect("-surface gives at least one surface");
    mri_scalar_mul(g, p.out, p.err, Pair::Same(&mut dist), -1.0)?;

    let _ = writeln!(p.out, "min_dist = {}", cfmt::g(p.o.min_dist, 6));

    let mut dist_sup: Option<Mri> = None;
    let mut ctrl = if p.o.nonmax_suppress != 0 {
        let _ = writeln!(p.out, "computing nonmaximum suppression");
        let sup = mri_non_max_suppress(g, p.out, p.err, &dist, None, 0.0, 1)?.unwrap_new();
        let mut c = mri_clone_different_type(g, p.out, p.err, &sup, MRI_UCHAR)?;
        mri_binarize(
            g,
            p.out,
            p.err,
            Pair::Given(&sup, &mut c),
            p.o.min_dist as f32,
            CONTROL_NONE as f32,
            CONTROL_MARKED as f32,
        )?;
        dist_sup = Some(sup);
        c
    } else if p.o.erode != 0 {
        let mut c = mri_clone_different_type(g, p.out, p.err, &dist, MRI_UCHAR)?;
        mri_binarize(
            g,
            p.out,
            p.err,
            Pair::Given(&dist, &mut c),
            p.o.min_dist as f32,
            CONTROL_NONE as f32,
            CONTROL_MARKED as f32,
        )?;
        for _ in 0..p.o.erode.max(0) {
            mri_erode(g, p.out, p.err, Pair::Same(&mut c))?;
        }
        c
    } else {
        let mut c = mri_clone_different_type(g, p.out, p.err, &dist, MRI_UCHAR)?;
        mri_binarize(
            g,
            p.out,
            p.err,
            Pair::Given(&dist, &mut c),
            p.o.min_dist as f32,
            CONTROL_NONE as f32,
            CONTROL_MARKED as f32,
        )?;
        c
    };

    if p.o.control_point_fname.is_some() || p.o.control_point_label.is_some() {
        mri_norm_add_file_control_points(
            g,
            p.out,
            p.err,
            &mut p.st,
            &mut ctrl,
            CONTROL_MARKED,
            Some(src),
        )?;
    }

    if p.o.mask_sigma > 0.0 {
        let smooth = mri_gaussian_smooth(
            g,
            p.out,
            p.err,
            Pair::New(&dst),
            f64::from(p.o.mask_sigma),
            1,
        )?
        .unwrap_new();
        let mut mag = mri_clone_different_type(g, p.out, p.err, &dst, MRI_FLOAT)?;
        let grad = mri_sobel(g, p.out, p.err, &smooth, None, Some(&mut mag))?;
        mri_binarize(
            g,
            p.out,
            p.err,
            Pair::Same(&mut mag),
            p.o.mask_thresh,
            1.0,
            0.0,
        )?;
        mri_mask(
            g,
            p.out,
            p.err,
            p.env,
            Pair::Same(&mut ctrl),
            &mag,
            0,
            CONTROL_NONE as f32,
        )?;
        drop((grad, mag, smooth));
    }

    if let Some(mo) = p.o.mask_orig_fname.clone() {
        let orig = p.read(g, &mo)?;
        let mut orig = need(g, p.err, orig, "MRIread of -mask_orig's volume")?;
        mri_binarize(
            g,
            p.out,
            p.err,
            Pair::Same(&mut orig),
            p.o.mask_orig_thresh,
            0.0,
            1.0,
        )?;
        mri_mask(
            g,
            p.out,
            p.err,
            p.env,
            Pair::Same(&mut ctrl),
            &orig,
            0,
            CONTROL_NONE as f32,
        )?;
    }

    if diag_write_verbose(g) {
        p.write(g, &mut dist, "d.mgz")?;
        if let Some(s) = dist_sup.as_mut() {
            p.write(g, s, "dm.mgz")?;
        }
        p.write(g, &mut ctrl, "c.mgz")?;
    }

    let _ = writeln!(p.out, "Erasing Border planes 4");
    mri_erase_border_planes(g, p.err, &mut ctrl, 4)?;

    if let Some(aseg_fname) = p.o.aseg_fname.clone() {
        let Some(a) = p.read(g, &aseg_fname)? else {
            let m = format!("{}: could not load aseg from {aseg_fname}", g.progname);
            p.exit(g, ERROR_NOFILE, &m)?;
            return Ok(());
        };
        let _ = writeln!(
            p.out,
            "aseg read with width {} (src width {})",
            a.vg.width, src.vg.width
        );
        let a = if mri_match(&a, &ctrl) == 0 {
            let _ = writeln!(p.out, "resampling aseg...");
            let t = mri_resample(g, p.out, p.err, &a, &ctrl, SAMPLE_NEAREST)?;
            need(g, p.err, t, "MRIresample")?
        } else {
            a
        };
        let _ = writeln!(p.out, "Removing non-wm voxels");
        h::remove_nonwm_voxels(g, p.out, p.err, &mut ctrl, &a)?;
    } else {
        let _ = writeln!(p.out, "Removing surface outliers");
        h::remove_surface_outliers();
    }

    if let Some(cn) = p.o.checknorm_fname.clone() {
        let Some(m) = p.read(g, &cn)? else {
            let msg = format!("{}: could not load {cn}", g.progname);
            p.exit(g, ERROR_NOFILE, &msg)?;
            return Ok(());
        };
        h::remove_controlpoints_outside_range(
            g,
            p.out,
            p.err,
            &mut ctrl,
            &m,
            p.o.check_min,
            p.o.check_max,
        )?;
    }

    h::threshold_control_points(g, p.out, p.err, &mut ctrl, &dst, 10.0)?;
    h::remove_outliers_near_surface();

    if p.o.contrast == T2_CONTRAST {
        let mut r = p.run();
        h::remove_surface_outliers_t2(g, &mut r, &mut ctrl, &dist, &dst)?;
    }

    let _ = writeln!(p.out, "Building bias image");
    let mut bias = mri_build_bias_image(g, p.out, p.err, &dst, &mut ctrl, 0.0)?;
    drop(dist);
    drop(dist_sup);

    if p.o.bias_sigma > 0.0 {
        let kernel = mri_gaussian1d(g, p.out, p.err, p.o.bias_sigma, -1)?;
        if diag_write_verbose(g) {
            p.write(g, &mut bias, "b.mgz")?;
        }
        let _ = writeln!(
            p.out,
            "smoothing bias field with sigma={}",
            ff(f64::from(p.o.bias_sigma), 2, 3)
        );
        mri_convolve_gaussian(g, p.out, p.err, Pair::Same(&mut bias), &kernel)?;
        if diag_write_verbose(g) {
            p.write(g, &mut bias, "bs.mgz")?;
        }
    }

    if let Some(cv) = p.o.control_volume_fname.clone() {
        let _ = writeln!(p.out, "Writing control volume to {cv}");
        p.write(g, &mut ctrl, &cv)?;
    }
    if let Some(bv) = p.o.bias_volume_fname.clone() {
        let _ = writeln!(p.out, "Writing bias volume to {bv}");
        p.write(g, &mut bias, &bv)?;
    }
    drop(ctrl);

    let _ = writeln!(p.out, "MRIapplyBiasCorrectionSameGeometry()");
    // out = in * DEFAULT_DESIRED_WHITE_MATTER_VALUE / bias
    mri_apply_bias_correction_same_geometry(g, p.out, p.err, Pair::Same(&mut dst), &bias, WM)?;
    mri_remove_nans(g, p.out, p.err, p.env, &mut dst);
    let _ = writeln!(p.out, "writing normalized volume to {out_fname}");
    p.write(g, &mut dst, out_fname)?;
    let peak = get_vm_peak(g);
    let _ = writeln!(p.out, "#VMPC# mri_normalize VmPeak  {peak}");
    let _ = writeln!(p.out, "mri_normalize done");
    Err(Fatal::Exit(0))
}

/// `-r`'s program: the bias field and the control volume read, the control volume binarised at 1
/// (0 and 128), the input times the bias field scaled so the mean in the control points is 110,
/// written; the end (status 0).
fn read_bias(g: &mut Globals, p: &mut Prog, src: &Mri, out_fname: &str) -> Result<(), Fatal> {
    let bias_fname = p.o.bias_volume_fname.clone();
    let bias_fname = need(g, p.err, bias_fname, "-r's bias volume name")?;
    let Some(bias) = p.read(g, &bias_fname)? else {
        let m = format!("{}: could not read bias volume {bias_fname}", g.progname);
        p.exit(g, ERROR_BADPARM, &m)?;
        return Ok(());
    };
    let ctrl_fname = p.o.control_volume_fname.clone();
    let ctrl_fname = need(g, p.err, ctrl_fname, "-r's control volume name")?;
    let Some(mut ctrl) = p.read(g, &ctrl_fname)? else {
        let m = format!("{}: could not read control volume {ctrl_fname}", g.progname);
        p.exit(g, ERROR_BADPARM, &m)?;
        return Ok(());
    };
    mri_binarize(g, p.out, p.err, Pair::Same(&mut ctrl), 1.0, 0.0, 128.0)?;
    let r = mri_multiply(g, p.out, p.err, Tri::New(&bias, src))?;
    let mut dst = need(g, p.err, r.into_new(), "MRImultiply")?;
    let scale = mri_mean_in_label(g, p.err, &dst, &ctrl, 128)?;
    let _ = writeln!(
        p.out,
        "mean in wm is {}, scaling by {}",
        ff(scale, 2, 0),
        ff(div64(110.0, scale), 2, 2)
    );
    let scale = div64(110.0, scale);
    mri_scalar_mul(g, p.out, p.err, Pair::Same(&mut dst), scale as f32)?;
    p.write(g, &mut dst, out_fname)?;
    Err(Fatal::Exit(0))
}

/// `-l`'s normalisation: the bias field (read and then rebuilt) and the control volume read, the
/// control volume binarised at 1 (0 and `CONTROL_MARKED`, as `uchar`), the input scaled so the
/// mean in it is 110, the white matter's outliers removed, the bias field built from the scaled
/// volume, filled by the soap bubble (50 iterations) and applied in place.
fn longitudinal(g: &mut Globals, p: &mut Prog, src: &Mri) -> Result<Mri, Fatal> {
    let bias_fname = p.o.long_bias_volume_fname.clone();
    let bias_fname = need(g, p.err, bias_fname, "-l's bias volume name")?;
    let bias = p.read(g, &bias_fname)?;
    if bias.is_none() {
        let m = format!("{}: could not read bias volume {bias_fname}", g.progname);
        p.exit(g, ERROR_BADPARM, &m)?;
    }
    let ctrl_fname = p.o.long_control_volume_fname.clone();
    let ctrl_fname = need(g, p.err, ctrl_fname, "-l's control volume name")?;
    let ctrl = p.read(g, &ctrl_fname)?;
    let Some(mut ctrl) = ctrl else {
        let m = format!("{}: could not read control volume {ctrl_fname}", g.progname);
        p.exit(g, ERROR_BADPARM, &m)?;
        return Err(Fatal::Exit(1));
    };
    drop(bias);
    mri_binarize(
        g,
        p.out,
        p.err,
        Pair::Same(&mut ctrl),
        1.0,
        0.0,
        CONTROL_MARKED as f32,
    )?;
    if ctrl.mtype != MRI_UCHAR {
        let t = mri_change_type(g, p.out, p.err, p.env, &ctrl, MRI_UCHAR, 0.0, 1.0, 1)?;
        ctrl = need(g, p.err, t, "MRIchangeType")?;
    }
    let scale = mri_mean_in_label(g, p.err, src, &ctrl, CONTROL_MARKED)?;
    let _ = writeln!(
        p.out,
        "mean in wm is {}, scaling by {}",
        ff(scale, 2, 0),
        ff(div64(110.0, scale), 2, 2)
    );
    let scale = div64(f64::from(DEFAULT_DESIRED_WHITE_MATTER_VALUE), scale);
    let mut dst = mri_scalar_mul(g, p.out, p.err, Pair::New(src), scale as f32)?.unwrap_new();
    let ib = to_int(g, div(p.o.intensity_below, 2.0), "MRIremoveWMOutliers");
    {
        let mut r = p.run();
        h::mri_remove_wm_outliers(g, &mut r, &dst, &mut ctrl, ib)?;
    }
    let mut bias = mri_build_bias_image(g, p.out, p.err, &dst, &mut ctrl, 0.0)?;
    mri_soap_bubble(g, p.out, p.err, Pair::Same(&mut bias), &ctrl, 50, 1.0)?;
    mri_apply_bias_correction_same_geometry(g, p.out, p.err, Pair::Same(&mut dst), &bias, WM)?;
    Ok(dst)
}

/// `-grad`'s mask of the voxels that cannot be control points: the input smoothed by a Gaussian
/// of 0.5 voxels, its Sobel gradient's magnitude (`m.mgz`) binarised at `grad_thresh` (0 and 1,
/// `nc.mgz`).
fn gradient_mask(g: &mut Globals, p: &mut Prog, src: &Mri) -> Result<(), Fatal> {
    let kernel = mri_gaussian1d(g, p.out, p.err, 0.5, -1)?;
    let mut nc = mri_clone_different_type(g, p.out, p.err, src, MRI_UCHAR)?;
    // the scan type's threshold (MGH 15, WashU 20, else 12) is replaced by -grad's
    let _thresh: f32 = match p.o.scan_type {
        MRI_MGH_MPRAGE => 15.0,
        MRI_WASHU_MPRAGE => 20.0,
        _ => 12.0,
    };
    let smooth = mri_convolve_gaussian(g, p.out, p.err, Pair::New(src), &kernel)?.unwrap_new();
    let thresh = p.o.grad_thresh;
    let mut mag = mri_clone_different_type(g, p.out, p.err, src, MRI_FLOAT)?;
    let grad = mri_sobel(g, p.out, p.err, &smooth, None, Some(&mut mag))?;
    p.write(g, &mut mag, "m.mgz")?;
    mri_binarize(
        g,
        p.out,
        p.err,
        Pair::Given(&mag, &mut nc),
        thresh,
        0.0,
        1.0,
    )?;
    p.write(g, &mut nc, "nc.mgz")?;
    drop((mag, grad, smooth, kernel));
    p.not_control = Some(nc);
    Ok(())
}

/// The normalisation with `-aseg` (`mri_normalize.cpp` 737-813): the segmentation's white matter
/// as control points, their outliers removed (`MRIremoveWMOutliersAndRetainMedialSurface`), the
/// control volume binarised and made `uchar`, the file's points added, the surface interiors'
/// points (`-surface_interior`), the bias field built, smoothed and applied in place.
fn with_aseg(
    g: &mut Globals,
    p: &mut Prog,
    src: &Mri,
    mut dst: Mri,
    aseg: Mri,
) -> Result<Mri, Fatal> {
    let _ = writeln!(p.out, "processing with aseg");
    let mut ctrl = mri_clone(g, p.out, p.err, &aseg, None)?.unwrap_new();
    for label in ASEG_WM_LABELS {
        mri_copy_label(g, p.err, &aseg, &mut ctrl, label)?;
    }
    let _ = writeln!(p.out, "removing outliers in the aseg WM...");
    let ib = to_int(
        g,
        p.o.intensity_below,
        "MRIremoveWMOutliersAndRetainMedialSurface",
    );
    {
        let mut r = p.run();
        h::mri_remove_wm_outliers_and_retain_medial_surface(g, &mut r, &dst, &mut ctrl, ib)?;
    }
    mri_binarize(
        g,
        p.out,
        p.err,
        Pair::Same(&mut ctrl),
        1.0,
        CONTROL_NONE as f32,
        CONTROL_MARKED as f32,
    )?;
    let t = mri_change_type(g, p.out, p.err, p.env, &ctrl, MRI_UCHAR, 0.0, 255.0, 1)?;
    let mut ctrl = need(g, p.err, t, "MRIchangeType")?;
    mri_norm_add_file_control_points(
        g,
        p.out,
        p.err,
        &mut p.st,
        &mut ctrl,
        CONTROL_MARKED,
        Some(src),
    )?;

    if let Some(i1) = p.o.interior_fname1.clone() {
        let i2 = p.o.interior_fname2.clone().unwrap_or_default();
        let mut s1 = read_interior(g, p, &i1)?;
        let mut s2 = read_interior(g, p, &i2)?;
        let (ia, ib125) = (p.o.intensity_above, below125(&p.o));
        let mut sg = SurfGlobals::default();
        let mut r = p.run();
        // add_interior_points(ctrl, dst, ..., aseg, ctrl)
        h::add_interior_points(
            g,
            &mut sg,
            &mut r,
            &mut ctrl,
            &dst,
            ia,
            ib125,
            &mut s1,
            &mut s2,
            Some(&aseg),
        )?;
    }
    if diag_write_verbose(g) {
        p.write(g, &mut ctrl, "norm_ctrl.mgz")?;
    }

    let _ = writeln!(p.out, "Building bias image");
    let _ = p.out.flush();
    let mut bias = mri_build_bias_image(g, p.out, p.err, &dst, &mut ctrl, 0.0)?;
    let _ = p.out.flush();

    if p.o.bias_sigma > 0.0 {
        let _ = writeln!(
            p.out,
            "Smoothing with sigma {}",
            cfmt::g(f64::from(p.o.bias_sigma), 6)
        );
        smooth_bias(g, p.out, p.err, &mut bias, p.o.bias_sigma)?;
        let _ = p.out.flush();
    }
    if let Some(ocp) = p.o.output_control_points_vol.clone() {
        let _ = writeln!(p.out, "Writing output control points volume to {ocp}");
        p.write(g, &mut ctrl, &ocp)?;
    }
    drop(ctrl);
    drop(aseg);
    let _ = writeln!(p.out, "Applying bias correction");
    mri_apply_bias_correction_same_geometry(g, p.out, p.err, Pair::Same(&mut dst), &bias, WM)?;
    if diag_write_verbose(g) {
        p.write(g, &mut dst, "norm_1.mgz")?;
    }
    let _ = p.out.flush();
    Ok(dst)
}

/// A surface of `-surface_interior` (`MRISread`), ending the program when it cannot be read.
fn read_interior(g: &mut Globals, p: &mut Prog, fname: &str) -> Result<Mris, Fatal> {
    match p.read_surface(g, fname)? {
        Some(m) => Ok(m),
        None => {
            let m = format!(
                "{}: could not read white matter surface from {fname}\n",
                g.progname
            );
            p.exit(g, ERROR_NOFILE, &m)?;
            Err(Fatal::Exit(1))
        }
    }
}

/// The normalisation without `-aseg` (`mri_normalize.cpp` 814-903): the one-dimensional
/// normalisation unless `-no1d`; when it fails (`1d normalization failed - trying no1d...`) or
/// with `-no1d`: the volume so far or a copy (`-file_only` with no SNR pass, or `-gentle` with a
/// control-point file), the surface interiors' bias field (`-surface_interior`), a copy, or the
/// high-signal normalisation (`-snr`).
fn without_aseg(
    g: &mut Globals,
    p: &mut Prog,
    src: &mut Mri,
    mut dst: Option<Mri>,
) -> Result<Option<Mri>, Fatal> {
    let _ = writeln!(p.out, "processing without aseg, no1d={}", p.o.no1d);
    if p.o.no1d == 0 {
        let _ = writeln!(p.out, "MRInormInit(): ");
        mri_norm_init(g, p.out, p.err, p.env, src, &mut p.mni, 0, 0, 0, 0, 0.0)?;
        let _ = writeln!(p.out, "MRInormalize(): ");
        let r = mri_normalize(g, p.out, p.err, p.env, Pair::New(src), Some(&mut p.mni))?;
        dst = r.into_new();
        if dst.is_none() {
            p.o.no1d = 1;
            let _ = writeln!(p.out, "1d normalization failed - trying no1d...");
        }
    }
    if p.o.no1d != 0 {
        if (p.o.file_only != 0 && p.o.nosnr != 0)
            || (p.o.gentle_flag != 0 && p.o.control_point_fname.is_some())
        {
            if dst.is_none() {
                dst = Some(mri_copy(g, p.out, p.err, src, None)?.unwrap_new());
            }
        } else if p.o.nosnr != 0 {
            if let Some(i1) = p.o.interior_fname1.clone() {
                let _ = writeln!(
                    p.out,
                    "computing initial normalization using surface interiors"
                );
                let mut ctrl = mri_clone_different_type(g, p.out, p.err, src, MRI_UCHAR)?;
                let i2 = p.o.interior_fname2.clone().unwrap_or_default();
                let mut s1 = read_interior(g, p, &i1)?;
                let mut s2 = read_interior(g, p, &i2)?;
                let vals = need(g, p.err, dst.take(), "MRInormalize")?;
                let (ia, ib125) = (p.o.intensity_above, below125(&p.o));
                {
                    let mut sg = SurfGlobals::default();
                    let mut r = p.run();
                    h::add_interior_points(
                        g, &mut sg, &mut r, &mut ctrl, &vals, ia, ib125, &mut s1, &mut s2, None,
                    )?;
                }
                let mut bias = mri_build_bias_image(g, p.out, p.err, &vals, &mut ctrl, 0.0)?;
                if p.o.bias_sigma > 0.0 {
                    smooth_bias(g, p.out, p.err, &mut bias, p.o.bias_sigma)?;
                }
                let mut vals = vals;
                let r = mri_apply_bias_correction_same_geometry(
                    g,
                    p.out,
                    p.err,
                    Pair::Given(src, &mut vals),
                    &bias,
                    WM,
                )?;
                dst = kept(r, vals);
            } else if p.o.long_flag == 0 {
                // no initial normalisation asked for
                dst = Some(mri_copy(g, p.out, p.err, src, None)?.unwrap_new());
            }
        } else {
            let _ = writeln!(p.out, "computing initial normalization using SNR...");
            let r = match dst.as_mut() {
                Some(d) => mri_normalize_high_signal_low_std(
                    g,
                    p.out,
                    p.err,
                    p.env,
                    p.dir,
                    &mut p.st,
                    src,
                    Dst::Given(d),
                    p.o.bias_sigma,
                    WM,
                )?,
                None => mri_normalize_high_signal_low_std(
                    g,
                    p.out,
                    p.err,
                    p.env,
                    p.dir,
                    &mut p.st,
                    src,
                    Dst::New,
                    p.o.bias_sigma,
                    WM,
                )?,
            };
            dst = match (r, dst) {
                (Ret::New(m), _) => Some(m),
                (Ret::Given, d) => d,
                (Ret::Null, _) => None,
            };
        }
        if dst.is_none() {
            let m = format!("{}: could not allocate volume", g.progname);
            p.exit(g, ERROR_BADPARM, &m)?;
        }
    }
    Ok(dst)
}
