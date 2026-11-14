// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrinorm.cpp` (queue item 3.1-43), second group: the intensity normalisation
//! (`MRIsplineNormalize` 98, `mriSplineNormalizeShort` 168, `MRInormInit` 368,
//! `MRInormFindPeaks` 555, `MRInormCheckPeaks` 610, `MRInormGentlyFindControlPoints` 1096,
//! `MRInormFindControlPointsInWindow` 1260, `MRIbuildBiasImage` 1347, `MRI3dGentleNormalize`
//! 1502, `mriRemoveOutliers` 2781, `MRI3dUseFileControlPoints` 2838,
//! `MRI3dUseLabelControlPoints` 2940, `MRI3dWriteControlPoints` 2985, `MRI3dWriteBias` 2991,
//! `MRInormAddFileControlPoints` 2997, `csf_in_window` 3292,
//! `MRIapplyBiasCorrectionSameGeometry` 3635; the file's state 75-81 as [`NormState`]).
//!
//! The control volumes are read and written as FreeSurfer's `MRIvox` does: bytes (FS-587 for a
//! control volume of another type, FS-588 outside it); windows reach as far as the source's index
//! tables (FS-589). FS-587 to FS-596.

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::{
    FS_587, FS_588, FS_589, FS_590, FS_591, FS_592, FS_593, FS_594, FS_595, FS_596,
};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::cstd::atof;
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE, DIAG_WRITE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::histo::{Histogram, histo_find_last_peak_in_region, histo_smooth};
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix_ops::matrix_print;
use larmorx_freesurfer_core::random::random_number;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::spline::{open_spline, open_splint};
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sub};
use larmorx_freesurfer_core::utils::{nint, stricmp};
use larmorx_freesurfer_io::ctrpoints::{
    ControlInfo, mri_read_control_points, mri_write_control_points,
};
use larmorx_freesurfer_io::label::{LABEL_COORDS_VOXEL, Label};
use larmorx_freesurfer_io::mriio::mri_write;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::lta::{LINEAR_RAS_TO_RAS, lta_alloc};
use larmorx_freesurfer_mri::mri::{
    MAX_INDEX, MRI_SHORT, MRI_UCHAR, Mri, MriRegion, mri_alloc, mri_copy_header,
};
use larmorx_freesurfer_mri::vol_geom::world_to_voxel;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{Pair, mri_binarize, mri_clone, mri_scalar_mul};
use larmorx_freesurfer_volume::coords::surface_ras_to_voxel;
use larmorx_freesurfer_volume::label_coords::{Io as LabelIo, LabelDst, label_to_voxel};
use larmorx_freesurfer_volume::labels::mri_mean_in_label;
use larmorx_freesurfer_volume::sample::sample_volume;
use larmorx_freesurfer_volume::skull::find_approximate_skull_bounding_box;
use larmorx_freesurfer_volume::talairachex::mri_talairach_to_voxel_ex;
use larmorx_freesurfer_volume::vgeom::get_vol_geom;

use crate::conv::{mri_convolve_gaussian, mri_gaussian1d};
use crate::mem::{self, Notes};
use crate::norm::{CONTROL_MARKED, CONTROL_NONE, mri_build_voronoi_diagram};

/// `MAX_SPLINE_POINTS` (`mrinorm.h`): the windows and spline points an `MNI` holds.
pub const MAX_SPLINE_POINTS: usize = 80;
/// `DEFAULT_DESIRED_WHITE_MATTER_VALUE`.
pub const DEFAULT_DESIRED_WHITE_MATTER_VALUE: i32 = 110;
const DEFAULT_SMOOTH_SIGMA: f32 = 2.0;
const DEFAULT_WINDOWS_BELOW_T0: i32 = 9;
const DEFAULT_WINDOWS_ABOVE_T0: i32 = 14;
const DEFAULT_WINDOW_SIZE: f32 = 10.0;
const MIN_HISTO_PCT: f32 = 0.15;
const HISTO_WINDOW_SIZE: i32 = 7;
const Z_OFFSET_SCALE: f64 = 0.2;
const OVERLAP: f32 = 0.5;
const MAX_GRADIENT: f32 = 1.0;
const MAX_SKIPPED: i32 = 2;
const STARTING_SLICE: i32 = 4;
const SLICE_OFFSET: i32 = 3;
/// `WINDOW_WIDTH` (millimetres).
const WINDOW_WIDTH: f32 = 120.0;
const BIAS_IMAGE_WIDTH: i32 = 25;
/// `FLT_EPSILON` (`float.h`'s: mrinorm.cpp's `FZERO`).
const FLT_EPSILON: f32 = f32::EPSILON;

/// The bit pattern of an `MNI` peak array's entry never written (FreeSurfer's arrays live on the
/// stack, uninitialised): a signalling NaN no arithmetic makes and the probe never writes.
pub const NEVER_WRITTEN: u32 = 0x7fa5_0dea;

/// The notes of a direct access to a control volume: another type (`Wrapped`), outside it.
const CTRL_NOTES: Notes = (&FS_587, &FS_588);

/// mrinorm.cpp's file-scope state: the control points `MRI3dUseFileControlPoints` and
/// `MRI3dUseLabelControlPoints` set (`num_control_points`, `xctrl`, `yctrl`, `zctrl`), the names
/// `MRI3dWriteControlPoints` and `MRI3dWriteBias` set, and the functions' own statics
/// (`MRInormAddFileControlPoints`' `first`, the diagnostic pass counters). One per program run.
#[derive(Clone, Debug, Default)]
pub struct NormState {
    pub ctrl: Vec<[i32; 3]>,
    pub control_volume_fname: Option<String>,
    pub bias_volume_fname: Option<String>,
    /// `first` of `MRInormAddFileControlPoints`: the volume size already reported.
    pub size_reported: bool,
    /// `MRI3dGentleNormalize`'s diagnostic `pass`.
    pub gentle_pass: i32,
    /// `MRI3dNormalize`'s diagnostic `pass`.
    pub normalize_pass: i32,
    /// `remove_gray_matter_control_points`' `ncalls`.
    pub remove_calls: i32,
}

/// `MRI_NORM_INFO` (`MNI`): the windows of the one-dimensional normalisation, as `calloc` leaves
/// it before `MRInormInit`.
#[derive(Clone, Debug, PartialEq)]
pub struct Mni {
    pub windows_above_t0: i32,
    pub windows_below_t0: i32,
    pub desired_wm_value: i32,
    pub smooth_sigma: f32,
    pub histograms: Vec<Histogram>,
    pub regions: Vec<MriRegion>,
    pub max_gradient: f32,
}

impl Default for Mni {
    fn default() -> Mni {
        let empty = Histogram {
            max_bins: 0,
            nbins: 0,
            bins: Vec::new(),
            counts: Vec::new(),
            bin_size: 0.0,
            min: 0.0,
            max: 0.0,
        };
        Mni {
            windows_above_t0: 0,
            windows_below_t0: 0,
            desired_wm_value: 0,
            smooth_sigma: 0.0,
            histograms: vec![empty; MAX_SPLINE_POINTS],
            regions: vec![MriRegion::default(); MAX_SPLINE_POINTS],
            max_gradient: 0.0,
        }
    }
}

/// `FZERO(f)` of a `float`.
fn fzero(f: f32) -> bool {
    f.abs() < FLT_EPSILON
}

/// `MAX(a, b)` (`(a) >= (b) ? (a) : (b)`).
fn cmax(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// `MIN(a, b)` (`(a) < (b) ? (a) : (b)`).
fn cmin(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

/// `%<w>.<p>f`.
fn ff(v: f64, w: usize, p: usize) -> String {
    cfmt::pad(&cfmt::f(v, p), w)
}

/// A volume's extent.
fn ext(m: &Mri) -> (i32, i32, i32) {
    (m.vg.width, m.vg.height, m.vg.depth)
}

/// The frame-0 values of `m` (`MRIgetVoxVal`), `[z][y][x]`.
fn values(g: &mut Globals, err: &mut dyn Write, m: &Mri) -> Result<Vec<f32>, Fatal> {
    let (w, h, d) = ext(m);
    let mut v = Vec::with_capacity((w.max(0) as usize) * (h.max(0) as usize) * (d.max(0) as usize));
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                v.push(mri_get_vox_val(g, err, m, x, y, z, 0, 0)?);
            }
        }
    }
    Ok(v)
}

/// The index tables of a volume for windows reaching `r` voxels past each end (`r` at most
/// `MAX_INDEX`, checked by [`check_window`]).
struct Idx {
    x: Vec<i32>,
    y: Vec<i32>,
    z: Vec<i32>,
    r: i32,
}

impl Idx {
    fn of(m: &Mri, r: i32) -> Idx {
        let r = r.clamp(0, MAX_INDEX);
        let f = |t: Option<&Vec<i32>>, len: i32| -> Vec<i32> {
            (-r..len.max(0) + r)
                .map(|i| t.map_or(0, |t| Mri::index(t, i)))
                .collect()
        };
        Idx {
            x: f(m.xi.as_ref(), m.vg.width),
            y: f(m.yi.as_ref(), m.vg.height),
            z: f(m.zi.as_ref(), m.vg.depth),
            r,
        }
    }
    fn x(&self, i: i32) -> i32 {
        self.x[(i + self.r) as usize]
    }
    fn y(&self, i: i32) -> i32 {
        self.y[(i + self.r) as usize]
    }
    fn z(&self, i: i32) -> i32 {
        self.z[(i + self.r) as usize]
    }
}

/// FS-589: a window of half-width `whalf` around every voxel reads the index tables `whalf`
/// entries past each end; beyond their `MAX_INDEX` margin (a tiny, zero or NaN voxel size)
/// FreeSurfer reads outside them: an error in both modes. `true` when the window fits.
fn check_window(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    whalf: i32,
) -> Result<bool, Fatal> {
    if whalf <= MAX_INDEX && whalf != i32::MIN {
        return Ok(true);
    }
    g.notices.add(
        &FS_589,
        format!("{what}: a window of half-width {whalf} voxels, wider than the index tables"),
    );
    log::error_exit(
        g,
        err,
        ERROR_BADPARM,
        &format!(
            "{what}: a window of half-width {whalf} voxels (the voxel size) reaches past the volume's \
             index tables (FreeSurfer reads outside them)"
        ),
    )?;
    Ok(false)
}

/// `MRIvox(ctrl, x, y, z)` (a byte; FS-587: another type's byte with `reproduce`, its value by
/// default; FS-588: 0 outside it).
fn cget(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    c: &Mri,
    x: i32,
    y: i32,
    z: i32,
) -> Result<f32, Fatal> {
    if c.mtype == MRI_UCHAR || g.notices.reproduce() {
        return Ok(mem::read(
            g,
            CTRL_NOTES,
            what,
            c,
            i64::from(z),
            i64::from(y),
            i64::from(x),
            MRI_UCHAR,
        ));
    }
    g.notices.add(
        &FS_587,
        format!(
            "{what}: a control volume of type {} read by value, not byte by byte",
            c.mtype
        ),
    );
    mri_get_vox_val(g, err, c, x, y, z, 0, 0)
}

/// `MRIvox(ctrl, x, y, z) = b` (as [`cget`]).
#[allow(clippy::too_many_arguments)]
fn cset(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    c: &mut Mri,
    x: i32,
    y: i32,
    z: i32,
    b: u8,
) -> Result<(), Fatal> {
    if c.mtype == MRI_UCHAR || g.notices.reproduce() {
        mem::write(
            g,
            CTRL_NOTES,
            what,
            c,
            i64::from(z),
            i64::from(y),
            i64::from(x),
            &[b],
        );
        return Ok(());
    }
    g.notices.add(
        &FS_587,
        format!(
            "{what}: a control volume of type {} written by value, not byte by byte",
            c.mtype
        ),
    );
    mri_set_vox_val(g, err, c, x, y, z, 0, f32::from(b), 0)?;
    Ok(())
}

/// `mri_not_control && MRIgetVoxVal(mri_not_control, x, y, z, 0) > 0`.
fn is_not_control(
    g: &mut Globals,
    err: &mut dyn Write,
    nc: Option<&Mri>,
    x: i32,
    y: i32,
    z: i32,
) -> Result<bool, Fatal> {
    match nc {
        Some(m) => Ok(mri_get_vox_val(g, err, m, x, y, z, 0, 0)? > 0.0),
        None => Ok(false),
    }
}

/// The `RAN` variable as `atof` reads it (0 when it is not set).
fn ran(env: &Environment) -> f64 {
    env.get("RAN")
        .map(|v| atof(v.to_string_lossy().as_bytes()))
        .unwrap_or(0.0)
}

/// `MRIsplineNormalize(mri_src, mri_dst, pmri_field, inputs, outputs, npoints)` (`Pair::Same`:
/// the source normalised in place; `field`: `pmri_field`, a bias picture of 25 x height bytes made
/// when it holds none): a 16-bit source by `mriSplineNormalizeShort`. `npoints` cut to
/// `MAX_SPLINE_POINTS`; `Starting OpenSpline(): npoints = <n>` on standard output (not for a
/// 16-bit source); the natural spline through `(inputs, outputs)`; for each row `y` the spline's
/// value `frac` at `y` (the bias picture's row: `nint(110 / frac)`'s low byte), each voxel
/// `nint(val * frac + randomNumber(0, RAN))` (one draw per voxel; `RAN` 0 when unset) clamped to
/// `[0, 255]`. Without a destination, `MRIclone` of the source. The 16-bit form reads the source's
/// and writes the destination's 16-bit numbers directly (FS-587 for a destination of another type).
#[allow(clippy::too_many_arguments)]
pub fn mri_spline_normalize(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    p: Pair,
    field: Option<&mut Option<Mri>>,
    inputs: &[f32],
    outputs: &[f32],
    npoints: i32,
) -> Result<Ret<Mri>, Fatal> {
    let (stype, (w, h, d)) = match &p {
        Pair::New(s) | Pair::Given(s, _) => (s.mtype, ext(s)),
        Pair::Same(s) => (s.mtype, ext(s)),
    };
    let short = stype == MRI_SHORT;
    let what = if short {
        "mriSplineNormalizeShort"
    } else {
        "MRIsplineNormalize"
    };
    let ran = ran(env);
    let mut field = field;
    if let Some(slot) = field.as_deref_mut()
        && slot.is_none()
    {
        *slot = Some(mri_alloc(g, out, err, BIAS_IMAGE_WIDTH, h, 1, MRI_UCHAR)?);
    }
    let npoints = npoints.min(MAX_SPLINE_POINTS as i32);
    if !short {
        let _ = writeln!(out, "Starting OpenSpline(): npoints = {npoints}");
    }
    let mut second = [0.0f32; MAX_SPLINE_POINTS];
    open_spline(g, out, inputs, outputs, npoints, 0.0, 0.0, &mut second)?;
    let mut fresh = None;
    let (src, dst): (Option<&Mri>, &mut Mri) = match p {
        Pair::New(s) => (
            Some(s),
            fresh.insert(mri_clone(g, out, err, s, None)?.unwrap_new()),
        ),
        Pair::Given(s, dd) => (Some(s), dd),
        Pair::Same(dd) => (None, dd),
    };
    let short_notes: Notes = (&FS_587, &FS_588);
    for y in 0..h {
        let mut frac = 0.0f32;
        open_splint(inputs, outputs, &second, npoints, y as f32, &mut frac);
        if let Some(Some(f)) = field.as_deref_mut() {
            let b = nint(g, f64::from(div(110.0, frac))) as u8;
            for i in 0..BIAS_IMAGE_WIDTH {
                mem::write(g, CTRL_NOTES, what, f, 0, i64::from(y), i64::from(i), &[b]);
            }
        }
        for z in 0..d {
            for x in 0..w {
                if short {
                    // `*psrc++`: the source is 16-bit
                    let s = src.unwrap_or(dst);
                    let sval = mem::read(
                        g,
                        short_notes,
                        what,
                        s,
                        i64::from(z),
                        i64::from(y),
                        i64::from(x),
                        MRI_SHORT,
                    );
                    let r = random_number(g, env, out, err, 0.0, ran)?;
                    let mut dval = nint(g, add64(f64::from(mul(sval, frac)), r));
                    dval = dval.clamp(0, 255);
                    if dst.mtype == MRI_SHORT || g.notices.reproduce() {
                        mem::write(
                            g,
                            short_notes,
                            what,
                            dst,
                            i64::from(z),
                            i64::from(y),
                            i64::from(x),
                            &(dval as i16).to_le_bytes(),
                        );
                    } else {
                        g.notices.add(
                            &FS_587,
                            format!(
                                "{what}: a destination of type {} written by value, not as 16-bit numbers",
                                dst.mtype
                            ),
                        );
                        mri_set_vox_val(g, err, dst, x, y, z, 0, dval as f32, 0)?;
                    }
                } else {
                    let val = mri_get_vox_val(g, err, src.unwrap_or(dst), x, y, z, 0, 0)?;
                    let r = random_number(g, env, out, err, 0.0, ran)?;
                    // an int's value: never NaN
                    let dval =
                        (nint(g, add64(f64::from(mul(val, frac)), r)) as f32).clamp(0.0, 255.0);
                    mri_set_vox_val(g, err, dst, x, y, z, 0, dval, 0)?;
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRInormInit(mri, mni, windows_above_t0, windows_below_t0, wsize, desired_wm_value,
/// smooth_sigma)`: the defaults for zero arguments (`wsize` `nint(10 / ysize)`, white matter 110,
/// sigma 2, 14 windows above the Talairach origin and 9 below); the origin from the volume's
/// Talairach transform (`talairach transform` and its matrix on standard error, the average_305
/// centre unless `NO_AVERAGE305` is set; `MRItalairachToVoxelEx` of `(0, 0, 0)`, rounded), or
/// without one from the skull's bounding box (`no Talairach xform detected - ...` on standard
/// output: x and z at the box's centre, y 75 mm below its top); then the windows: 120 voxels wide
/// and deep (plus a fifth in z), `wsize` high, each `nint(wsize / 2)` below the last, the first
/// `windows_above_t0` such steps above the origin. More than `MAX_SPLINE_POINTS` windows: FS-591.
#[allow(clippy::too_many_arguments)]
pub fn mri_norm_init(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    mri: &Mri,
    mni: &mut Mni,
    windows_above_t0: i32,
    windows_below_t0: i32,
    wsize: i32,
    desired_wm_value: i32,
    smooth_sigma: f32,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRInormInit";
    let mut wsize = wsize;
    if wsize <= 0 {
        wsize = nint(g, f64::from(div(DEFAULT_WINDOW_SIZE, mri.vg.ysize)));
    }
    mni.desired_wm_value = if desired_wm_value == 0 {
        DEFAULT_DESIRED_WHITE_MATTER_VALUE
    } else {
        desired_wm_value
    };
    mni.smooth_sigma = if fzero(smooth_sigma) {
        DEFAULT_SMOOTH_SIGMA
    } else {
        smooth_sigma
    };
    let (x0_tal, y0_tal, z0_tal);
    if mri.inverse_linear_transform.is_some() {
        let mut lta = lta_alloc(g, out, err, 1, None)?;
        lta.ltype = LINEAR_RAS_TO_RAS;
        {
            let lt = &mut lta.xforms[0];
            lt.sigma = 1.0;
            lt.x0 = 0.0;
            lt.y0 = 0.0;
            lt.z0 = 0.0;
            if let Some(t) = &mri.linear_transform {
                let m_l = lt.m_l.as_mut().expect("LTAalloc's matrix");
                for row in 1..=3 {
                    for col in 1..=4 {
                        *m_l.relt_mut(row, col) = t[(col - 1) as usize][(row - 1) as usize] as f32;
                    }
                }
                let _ = err.write_all(b"talairach transform\n");
                let mut buf: Vec<u8> = Vec::new();
                matrix_print(
                    g,
                    Some(&mut buf as &mut dyn Write),
                    &mut std::io::sink(),
                    err,
                    Some(m_l),
                    5,
                );
                let _ = err.write_all(&buf);
                get_vol_geom(&mri.vg, &mut lt.src);
                get_vol_geom(&mri.vg, &mut lt.dst);
                if env.get("NO_AVERAGE305").is_some() {
                    let _ = err.write_all(b"INFO: tal dst c_(r,a,s) not modified\n");
                } else {
                    let _ = err.write_all(
                        b"INFO: Modifying talairach volume c_(r,a,s) based on average_305\n",
                    );
                    lt.dst.valid = 1;
                    // double constants stored in floats
                    lt.dst.c_r = -0.095f64 as f32;
                    lt.dst.c_a = -16.51f64 as f32;
                    lt.dst.c_s = 9.75f64 as f32;
                }
            }
        }
        let (r, p) = mri_talairach_to_voxel_ex(g, out, err, mri, 0.0, 0.0, 0.0, Some(&lta))?;
        let Some(p) = p.filter(|_| r == NO_ERROR) else {
            let code = g.gerror;
            log::error_printf(
                g,
                err,
                code,
                "MRInormComputeWindows: could not find Talairach origin",
            );
            return Ok(code);
        };
        x0_tal = nint(g, p[0]);
        y0_tal = nint(g, p[1]);
        z0_tal = nint(g, p[2]);
    } else {
        let _ = writeln!(
            out,
            "no Talairach xform detected - using skull bounding box to determine origin"
        );
        let mut bbox = MriRegion::default();
        find_approximate_skull_bounding_box(g, out, err, mri, 50, &mut bbox)?;
        x0_tal = float_to_int(g, f64::from(bbox.x) + f64::from(bbox.dx) / 2.0, WHAT);
        z0_tal = float_to_int(g, f64::from(bbox.z) + f64::from(bbox.dz) / 2.0, WHAT);
        y0_tal = float_to_int(
            g,
            f64::from(add(bbox.y as f32, div(75.0, mri.vg.ysize))),
            WHAT,
        );
    }
    let windows_above_t0 = if windows_above_t0 > 0 {
        windows_above_t0
    } else {
        DEFAULT_WINDOWS_ABOVE_T0
    };
    mni.windows_above_t0 = windows_above_t0;
    mni.windows_below_t0 = if windows_below_t0 > 0 {
        windows_below_t0
    } else {
        DEFAULT_WINDOWS_BELOW_T0
    };
    let nwindows = mni.windows_above_t0.wrapping_add(mni.windows_below_t0);
    if g.gdiag & DIAG_SHOW != 0 {
        let _ = write!(
            err,
            "MRInormInit:\nTalairach origin at ({x0_tal}, {y0_tal}, {z0_tal})\nwsize {wsize}, windows {} above, {} below\n",
            mni.windows_above_t0, mni.windows_below_t0
        );
    }
    if nwindows > MAX_SPLINE_POINTS as i32 {
        g.notices.add(
            &FS_591,
            format!("{WHAT}: {nwindows} windows, more than the {MAX_SPLINE_POINTS} an MNI holds"),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "{WHAT}: {nwindows} windows, more than the {MAX_SPLINE_POINTS} an MNI holds \
                 (FreeSurfer writes past its arrays)"
            ),
        )?;
        return Ok(ERROR_BADPARM);
    }
    let step = nint(g, f64::from(mul(wsize as f32, OVERLAP)));
    let mut y = y0_tal.wrapping_sub(step.wrapping_mul(windows_above_t0));
    for i in 0..nwindows.max(0) {
        // `size_mod = pow(SIZE_MOD, nup)` above the origin: SIZE_MOD is 1, so 1 everywhere
        let size_mod = 1.0f32;
        let dx = nint(g, f64::from(mul(size_mod, WINDOW_WIDTH)));
        let z_offset = nint(g, f64::from(dx as f32) * Z_OFFSET_SCALE);
        let dz = nint(g, f64::from(mul(size_mod, WINDOW_WIDTH)));
        let reg = &mut mni.regions[i as usize];
        reg.dx = dx;
        reg.x = x0_tal.wrapping_sub(dx / 2);
        reg.z = z0_tal.wrapping_sub((dz / 2).wrapping_add(z_offset));
        reg.dz = dz.wrapping_add(z_offset);
        reg.dy = wsize;
        reg.y = y;
        y = y.wrapping_add(step);
        if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
            let r = *reg;
            let _ = writeln!(
                err,
                "window {i}: ({}, {}, {}) -> ({}, {}, {})",
                r.x, r.y, r.z, r.dx, r.dy, r.dz
            );
        }
    }
    Ok(NO_ERROR)
}

/// The arrays `MRInormFindPeaks` and `MRInormCheckPeaks` share: `inputs` (the windows' centres)
/// and `outputs` (their peaks), `MAX_SPLINE_POINTS` each; an entry never written holds
/// [`NEVER_WRITTEN`].
pub struct Peaks<'a> {
    pub inputs: &'a mut [f32],
    pub outputs: &'a mut [f32],
}

/// `MRInormFindPeaks(mni, inputs, outputs)`: each window's histogram smoothed (sigma
/// `smooth_sigma`, one histogram reused), its last peak (`HISTOfindLastPeakInRegion` with a window
/// of 7, 15 % of the largest count, between the bins 3 and `nbins - 3`): the window's centre row
/// and the peak's bin value, windows without a peak left out; then [`mri_norm_check_peaks`] and
/// each kept peak turned into `desired_wm_value / peak`. The number of peaks kept.
pub fn mri_norm_find_peaks(
    g: &mut Globals,
    err: &mut dyn Write,
    mni: &Mni,
    p: &mut Peaks,
) -> Result<i32, Fatal> {
    let whalf = (HISTO_WINDOW_SIZE - 1) / 2;
    let nwindows = mni.windows_above_t0.wrapping_add(mni.windows_below_t0);
    let mut deleted = 0usize;
    let mut hsmooth: Option<Histogram> = None;
    for i in 0..nwindows.max(0) as usize {
        let reg = mni.regions[i];
        match hsmooth.as_mut() {
            None => {
                hsmooth = histo_smooth(g, &mni.histograms[i], None, mni.smooth_sigma).into_new()
            }
            Some(h) => {
                histo_smooth(g, &mni.histograms[i], Some(h), mni.smooth_sigma);
            }
        }
        let hs = hsmooth.as_ref().expect("the smoothed histogram");
        let peak = histo_find_last_peak_in_region(
            hs,
            HISTO_WINDOW_SIZE,
            MIN_HISTO_PCT,
            whalf,
            hs.nbins - whalf,
        );
        if peak < 0 {
            deleted += 1;
        } else {
            p.inputs[i - deleted] = add(reg.y as f32, div(reg.dy as f32, 2.0));
            p.outputs[i - deleted] = mni.histograms[i].bins[peak as usize];
        }
    }
    let npeaks = nwindows - deleted as i32;
    let npeaks = mri_norm_check_peaks(g, err, mni, p, npeaks)?;
    for i in 0..npeaks.max(0) as usize {
        p.outputs[i] = div(mni.desired_wm_value as f32, p.outputs[i]);
    }
    Ok(npeaks)
}

/// An entry of a peak array as FreeSurfer reads it: [`NEVER_WRITTEN`] (memory never written)
/// noted (FS-596) and read as 0.
fn entry(g: &mut Globals, a: &[f32], i: i32) -> f32 {
    let v = a[i as usize];
    if v.to_bits() == NEVER_WRITTEN {
        g.notices.add(
            &FS_596,
            format!("MRInormCheckPeaks: entry {i} of a peak array never written (memory contents; read as 0)"),
        );
        return 0.0;
    }
    v
}

/// `MRInormCheckPeaks(mni, inputs, outputs, npeaks)`: none kept when fewer than a third of the
/// windows have a peak; the anchor among the peaks `windows_above_t0 - 4` and three either side
/// (the middle one when it lies between the others, else the upper when it does, else the lower);
/// from it forwards and backwards each peak whose gradient from the last kept one (intensity over
/// rows, `max_gradient` or 1) is larger, or that lies more than 2 peaks past it, is dropped; the
/// kept peaks moved to the front. FS-590: an anchor past the peaks reads entries that are not
/// peaks (with `reproduce`; by default the three candidates are taken among the peaks); FS-591:
/// fewer than 7 windows above the origin read before the arrays (an error).
pub fn mri_norm_check_peaks(
    g: &mut Globals,
    err: &mut dyn Write,
    mni: &Mni,
    p: &mut Peaks,
    npeaks: i32,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRInormCheckPeaks";
    let mut deleted = [false; MAX_SPLINE_POINTS];
    let nwindows = mni.windows_above_t0.wrapping_add(mni.windows_below_t0);
    if npeaks < nwindows / 3 {
        return Ok(0);
    }
    let max_gradient = if fzero(mni.max_gradient) {
        MAX_GRADIENT
    } else {
        mni.max_gradient
    };
    if g.gdiag & DIAG_SHOW != 0 {
        let _ = writeln!(
            err,
            "max gradient {}",
            ff(f64::from(mni.max_gradient), 2, 3)
        );
        for slice in 0..npeaks {
            let (a, b) = (entry(g, p.inputs, slice), entry(g, p.outputs, slice));
            let _ = writeln!(
                err,
                "{slice}: {} --> {}",
                ff(f64::from(a), 2, 0),
                ff(f64::from(b), 2, 0)
            );
        }
    }
    let mut starting_slice = mni.windows_above_t0.wrapping_sub(STARTING_SLICE);
    if starting_slice < SLICE_OFFSET || starting_slice + SLICE_OFFSET >= p.outputs.len() as i32 {
        g.notices.add(
            &FS_591,
            format!(
                "{WHAT}: {} windows above the origin: the starting peaks lie outside the arrays",
                mni.windows_above_t0
            ),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "{WHAT}: {} windows above the Talairach origin: the starting peaks lie outside the \
                 peak arrays (FreeSurfer reads outside them)",
                mni.windows_above_t0
            ),
        )?;
        return Ok(0);
    }
    let (mut iu, mut im, mut id) = (
        starting_slice - SLICE_OFFSET,
        starting_slice,
        starting_slice + SLICE_OFFSET,
    );
    if id >= npeaks {
        g.notices.add(
            &FS_590,
            format!(
                "{WHAT}: the starting peaks {iu}, {im} and {id} reach past the {npeaks} peaks found"
            ),
        );
        if !g.notices.reproduce() {
            let last = npeaks - 1;
            iu = iu.min(last);
            im = im.min(last);
            id = id.min(last);
            starting_slice = im;
        }
    }
    let iup = entry(g, p.outputs, iu);
    let i = entry(g, p.outputs, im);
    let idown = entry(g, p.outputs, id);
    if g.gdiag & DIAG_SHOW != 0 {
        let _ = writeln!(
            err,
            "testing slices {}-{}={} ({}), {} ({}), and {} ({})",
            mni.windows_above_t0,
            STARTING_SLICE,
            starting_slice,
            ff(f64::from(i), 2, 0),
            id,
            ff(f64::from(idown), 2, 0),
            iu,
            ff(f64::from(iup), 2, 0)
        );
    }
    if i >= cmin(iup, idown) && i <= cmax(iup, idown) {
        // the middle one
    } else if iup >= cmin(i, idown) && iup <= cmax(i, idown) {
        starting_slice = iu;
    } else {
        starting_slice = id;
    }
    if g.gdiag & DIAG_SHOW != 0 {
        let v = entry(g, p.outputs, starting_slice);
        let _ = writeln!(
            err,
            "starting slice {starting_slice} --> {}",
            ff(f64::from(v), 2, 0)
        );
    }
    let show = g.gdiag & DIAG_SHOW != 0;
    let mut old = starting_slice;
    for slice in starting_slice + 1..npeaks {
        let d_i = sub(entry(g, p.outputs, slice), entry(g, p.outputs, old));
        let dy = sub(entry(g, p.inputs, slice), entry(g, p.inputs, slice - 1));
        let grad = div(d_i, dy).abs();
        let del = grad > max_gradient || (slice - old) > MAX_SKIPPED;
        deleted[slice as usize] = del;
        if !del {
            old = slice;
        } else if show {
            let v = entry(g, p.outputs, slice);
            let _ = writeln!(
                err,
                "deleting peak[{slice}]={}, grad = {} / {} = {}",
                ff(f64::from(v), 2, 0),
                ff(f64::from(d_i), 2, 0),
                ff(f64::from(dy), 2, 0),
                ff(f64::from(grad), 2, 1)
            );
        }
    }
    old = starting_slice;
    let mut slice = starting_slice - 1;
    while slice >= 0 {
        let d_i = sub(entry(g, p.outputs, old), entry(g, p.outputs, slice));
        let dy = sub(entry(g, p.inputs, slice + 1), entry(g, p.inputs, slice));
        let grad = div(d_i, dy).abs();
        let del = grad > max_gradient || (old - slice) > MAX_SKIPPED;
        deleted[slice as usize] = del;
        if !del {
            old = slice;
        } else if show {
            let v = entry(g, p.outputs, slice);
            let _ = writeln!(
                err,
                "deleting peak[{slice}]={}, grad = {} / {} = {}",
                ff(f64::from(v), 2, 0),
                ff(f64::from(d_i), 2, 0),
                ff(f64::from(dy), 2, 0),
                ff(f64::from(grad), 2, 1)
            );
        }
        slice -= 1;
    }
    let mut n = 0usize;
    for (slice, &del) in deleted.iter().enumerate().take(npeaks.max(0) as usize) {
        if !del {
            p.outputs[n] = p.outputs[slice];
            p.inputs[n] = p.inputs[slice];
            n += 1;
        }
    }
    if show {
        let _ = err.flush();
    }
    Ok(n as i32)
}

/// `MRInormGentlyFindControlPoints(mri_src, wm_target, intensity_above, intensity_below,
/// mri_ctrl, mri_not_control)`: into the given control volume (or a new byte one with the source's
/// header), keeping its `CONTROL_MARKED` voxels: first each voxel whose value (truncated) lies in
/// `[wm_target - 1.5 below, wm_target + 1.5 above]` and whose whole window of half-width
/// `ceil(3 / xsize)` does (128, else 0), then among the rest those within `[wm_target - below,
/// wm_target + above]` in windows of `ceil(2 / xsize)`; the thresholds are bytes (FS-592: wrapped
/// with `reproduce`); voxels the `mri_not_control` volume marks are skipped. Then
/// `mriRemoveOutliers` (2 neighbours), the file's control points (255), and the volume
/// binarised to 0 and 1.
#[allow(clippy::too_many_arguments)]
pub fn mri_norm_gently_find_control_points(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    st: &mut NormState,
    src: &Mri,
    wm_target: f32,
    intensity_above: f32,
    intensity_below: f32,
    ctrl: Option<&mut Mri>,
    not_control: Option<&Mri>,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRInormGentlyFindControlPoints";
    let wm_target = if wm_target == 0.0 {
        DEFAULT_DESIRED_WHITE_MATTER_VALUE as f32
    } else {
        wm_target
    };
    let (w, h, d) = ext(src);
    let mut fresh = None;
    let c: &mut Mri = match ctrl {
        Some(c) => c,
        None => {
            let mut c = mri_alloc(g, out, err, w, h, d, MRI_UCHAR)?;
            mri_copy_header(g, out, err, src, Some(&mut c))?;
            fresh.insert(c)
        }
    };
    let sv = values(g, err, src)?;
    let at =
        |x: i32, y: i32, z: i32| ((z as usize * h as usize) + y as usize) * w as usize + x as usize;
    // the byte thresholds (`BUFTYPE`): the low byte of the truncated value (FS-592)
    let byte = |g: &mut Globals, v: f64, name: &str| -> i32 {
        let t = float_to_int(g, v, WHAT);
        let b = i32::from(t as u8);
        if b != t {
            g.notices.add(
                &FS_592,
                format!("{WHAT}: the {name} threshold {t} kept in a byte becomes {b}"),
            );
            if g.notices.reproduce() {
                return b;
            }
        }
        t
    };
    let low = byte(
        g,
        f64::from(wm_target) - 1.5 * f64::from(intensity_below),
        "low",
    );
    let hi = byte(
        g,
        f64::from(intensity_above) * 1.5 + f64::from(wm_target),
        "high",
    );
    let mut whalf = 0i32;
    let mut nctrl = 0i32;
    let mut idx: Option<Idx> = None;
    for pass in 0..2 {
        let (lo_t, hi_t) = if pass == 0 {
            (low, hi)
        } else {
            (
                byte(g, f64::from(sub(wm_target, intensity_below)), "low"),
                byte(g, f64::from(add(wm_target, intensity_above)), "high"),
            )
        };
        let half = if pass == 0 { 3.0f32 } else { 2.0 };
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if pass == 0 {
                        if cget(g, err, WHAT, c, x, y, z)? == CONTROL_MARKED as f32 {
                            nctrl += 1;
                            continue;
                        }
                    } else {
                        let v = mri_get_vox_val(g, err, c, x, y, z, 0, 0)?;
                        if float_to_int(g, f64::from(v), WHAT) != 0 {
                            continue;
                        }
                    }
                    let mut val0 = 0;
                    if pass == 0 {
                        val0 = float_to_int(g, f64::from(sv[at(x, y, z)]), WHAT);
                    }
                    if is_not_control(g, err, not_control, x, y, z)? {
                        continue;
                    }
                    if pass == 1 {
                        val0 = float_to_int(g, f64::from(sv[at(x, y, z)]), WHAT);
                    }
                    let mut ctrl = 0u8;
                    if val0 >= lo_t && val0 <= hi_t {
                        whalf = float_to_int(g, f64::from(div(half, src.vg.xsize).ceil()), WHAT);
                        if !check_window(g, err, WHAT, whalf)? {
                            return Ok(Ret::Null);
                        }
                        if idx.as_ref().is_none_or(|t| t.r < whalf) {
                            idx = Some(Idx::of(src, whalf));
                        }
                        let t = idx.as_ref().expect("the index tables");
                        ctrl = 128;
                        let mut zk = -whalf;
                        while ctrl != 0 && zk <= whalf {
                            let zi = t.z(z + zk);
                            let mut yk = -whalf;
                            while ctrl != 0 && yk <= whalf {
                                let yi = t.y(y + yk);
                                let mut xk = -whalf;
                                while ctrl != 0 && xk <= whalf {
                                    let xi = t.x(x + xk);
                                    let val = float_to_int(g, f64::from(sv[at(xi, yi, zi)]), WHAT);
                                    if val > hi_t || val < lo_t {
                                        ctrl = 0;
                                    }
                                    xk += 1;
                                }
                                yk += 1;
                            }
                            zk += 1;
                        }
                    }
                    if ctrl != 0 {
                        nctrl += 1;
                    }
                    cset(g, err, WHAT, c, x, y, z, ctrl)?;
                }
            }
        }
        if g.gdiag & DIAG_SHOW != 0 {
            let n = 2 * whalf + 1;
            if pass == 0 {
                let _ = writeln!(err, "{nctrl} {n}x{n}x{n} control points found");
            } else {
                let _ = writeln!(err, "{nctrl} {n}x{n}x{n} control points found above {lo_t}");
            }
        }
    }
    mri_remove_outliers(g, err, c, 2)?;
    nctrl += mri_norm_add_file_control_points(g, out, err, st, c, 255, Some(src))?;
    if g.gdiag & DIAG_SHOW != 0 {
        let _ = writeln!(err, "{nctrl} control points found.");
    }
    if g.gx >= 0 && g.gy >= 0 && g.gz >= 0 {
        let (gx, gy, gz) = (g.gx, g.gy, g.gz);
        let val = mri_get_vox_val(g, err, src, gx, gy, gz, 0, 0)?;
        let marked = cget(g, err, WHAT, c, gx, gy, gz)? > 0.0;
        let _ = writeln!(
            out,
            "({gx}, {gy}, {gz}) is{} a control point (T1={})",
            if marked { "" } else { " NOT" },
            ff(f64::from(val), 2, 0)
        );
    }
    mri_binarize(g, out, err, Pair::Same(c), 1.0, 0.0, 1.0)?;
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRInormFindControlPointsInWindow(mri_src, wm_target, intensity_above, intensity_below,
/// mri_ctrl, whalf_mm, debug_str, &nctrl, scan_type, mri_not_control)`: into the given control
/// volume (or a new byte one with the source's header), skipping its control voxels (above 0) and
/// those `mri_not_control` marks: each voxel whose value (truncated) and whole window of
/// half-width `nint(whalf_mm / xsize)` lie in `[floor(wm_target - below), floor(wm_target +
/// above)]` becomes 128, the others 0. The count of new control points.
#[allow(clippy::too_many_arguments)]
pub fn mri_norm_find_control_points_in_window(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    wm_target: f32,
    intensity_above: f32,
    intensity_below: f32,
    ctrl: Option<&mut Mri>,
    whalf_mm: f32,
    debug_str: Option<&str>,
    not_control: Option<&Mri>,
) -> Result<(Ret<Mri>, i32), Fatal> {
    const WHAT: &str = "MRInormFindControlPointsInWindow";
    let whalf = nint(g, f64::from(div(whalf_mm, src.vg.xsize)));
    let wm_target = if wm_target == 0.0 {
        DEFAULT_DESIRED_WHITE_MATTER_VALUE as f32
    } else {
        wm_target
    };
    let (w, h, d) = ext(src);
    let mut fresh = None;
    let c: &mut Mri = match ctrl {
        Some(c) => c,
        None => {
            let mut c = mri_alloc(g, out, err, w, h, d, MRI_UCHAR)?;
            mri_copy_header(g, out, err, src, Some(&mut c))?;
            fresh.insert(c)
        }
    };
    let low = sub(wm_target, intensity_below).floor();
    let hi = add(wm_target, intensity_above).floor();
    let sv = values(g, err, src)?;
    let at =
        |x: i32, y: i32, z: i32| ((z as usize * h as usize) + y as usize) * w as usize + x as usize;
    let mut idx: Option<Idx> = None;
    let mut nctrl = 0;
    let mut val = 0i32;
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                if cget(g, err, WHAT, c, x, y, z)? > 0.0 {
                    continue;
                }
                if is_not_control(g, err, not_control, x, y, z)? {
                    continue;
                }
                let val0 = float_to_int(g, f64::from(sv[at(x, y, z)]), WHAT);
                let mut ctrl = 0u8;
                if val0 as f32 >= low && val0 as f32 <= hi {
                    if idx.is_none() {
                        if !check_window(g, err, WHAT, whalf)? {
                            return Ok((Ret::Null, 0));
                        }
                        idx = Some(Idx::of(src, whalf));
                    }
                    let t = idx.as_ref().expect("the index tables");
                    ctrl = 128;
                    let mut zk = -whalf;
                    while ctrl != 0 && zk <= whalf {
                        let zi = t.z(z + zk);
                        let mut yk = -whalf;
                        while ctrl != 0 && yk <= whalf {
                            let yi = t.y(y + yk);
                            let mut xk = -whalf;
                            while ctrl != 0 && xk <= whalf {
                                let xi = t.x(x + xk);
                                val = float_to_int(g, f64::from(sv[at(xi, yi, zi)]), WHAT);
                                if val as f32 > hi || (val as f32) < low {
                                    ctrl = 0;
                                }
                                xk += 1;
                            }
                            yk += 1;
                        }
                        zk += 1;
                    }
                }
                if ctrl != 0 {
                    nctrl += 1;
                }
                if ctrl != 0
                    && x == g.gx
                    && y == g.gy
                    && z == g.gz
                    && cget(g, err, WHAT, c, x, y, z)? == 0.0
                {
                    let _ = writeln!(
                        out,
                        "val {val} - setting ({x}, {y}, {z}) as a control point"
                    );
                }
                cset(g, err, WHAT, c, x, y, z, ctrl)?;
            }
        }
    }
    if g.gdiag & DIAG_SHOW != 0
        && let Some(s) = debug_str
    {
        let n = whalf.wrapping_mul(2).wrapping_add(1);
        let _ = writeln!(err, "{s} {nctrl} {n}x{n}x{n} control points found");
    }
    Ok((
        match fresh {
            Some(m) => Ret::New(m),
            None => Ret::Given,
        },
        nctrl,
    ))
}

/// `mriRemoveOutliers(mri, min_nbrs)`: in scan order, each control voxel (a byte above 0) with
/// fewer than `min_nbrs` control voxels among its 26 neighbours (positions clamped: an edge voxel
/// counts itself again) is cleared, the later voxels seeing the cleared ones.
pub fn mri_remove_outliers(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
    min_nbrs: i32,
) -> Result<i32, Fatal> {
    const WHAT: &str = "mriRemoveOutliers";
    let (w, h, d) = ext(mri);
    let t = Idx::of(mri, 1);
    let mut deleted = 0;
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                if cget(g, err, WHAT, mri, x, y, z)? == 0.0 {
                    continue;
                }
                let mut nbrs = -1;
                for zk in -1..=1 {
                    let zi = t.z(z + zk);
                    for yk in -1..=1 {
                        let yi = t.y(y + yk);
                        for xk in -1..=1 {
                            let xi = t.x(x + xk);
                            if cget(g, err, WHAT, mri, xi, yi, zi)? != 0.0 {
                                nbrs += 1;
                            }
                        }
                    }
                }
                if nbrs < min_nbrs {
                    deleted += 1;
                    cset(g, err, WHAT, mri, x, y, z, 0)?;
                }
            }
        }
    }
    if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
        let _ = writeln!(err, "{deleted} control points deleted.");
    }
    Ok(NO_ERROR)
}

/// `MRI3dUseFileControlPoints(mri, fname)`: nothing for `nofile` (any case); else the points of
/// `MRIreadControlPoints` (none when it fails), each (in tkregister RAS) whose voxel value
/// (`nint` of its voxel position) is at most `FS_THRESH_CTRL` (`%f`; 0 when unset) reported as
/// `unlikely control point ...` and, when `FS_THRESH_CTRL` is set, removed (FS-594: FreeSurfer
/// keeps the point after each removed one) and the rest written to `control.edited.dat` (in
/// `dir`); then the state's points: each one's voxel position (`MRIsurfaceRASToVoxel`, or
/// `MRIworldToVoxel` for scanner RAS; any other `useRealRAS` ends the program), rounded.
/// FS-593: an `FS_THRESH_CTRL` that is not a number.
#[allow(clippy::too_many_arguments)]
pub fn mri_3d_use_file_control_points(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    st: &mut NormState,
    mri: &mut Mri,
    fname: &str,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRI3dUseFileControlPoints";
    if stricmp(g, fname, "nofile") == 0 {
        return Ok(NO_ERROR);
    }
    let mut info = ControlInfo::default();
    let mut points =
        mri_read_control_points(g, env, out, err, dir, fname, &mut info)?.unwrap_or_default();
    let mut count = if points.is_empty() {
        0
    } else {
        info.count.max(0)
    };
    let use_real_ras = info.use_real_ras;
    let cp = env
        .get("FS_THRESH_CTRL")
        .map(|v| v.to_string_lossy().into_owned());
    let thresh = match &cp {
        None => 0.0f32,
        Some(s) => match larmorx_core::cscan::sscanf_f32(s.as_bytes()) {
            Some(t) => t,
            None => {
                g.notices.add(
                    &FS_593,
                    format!("{WHAT}: FS_THRESH_CTRL '{s}' is not a number: the threshold is never written"),
                );
                if !g.notices.reproduce() {
                    log::error_exit(
                        g,
                        err,
                        ERROR_BADPARM,
                        &format!("{WHAT}: FS_THRESH_CTRL '{s}' is not a number"),
                    )?;
                }
                0.0
            }
        },
    };
    if thresh > 0.0 {
        let _ = writeln!(
            out,
            "thresholding control points at {}",
            ff(f64::from(thresh), 2, 1)
        );
    }
    let mut deleted = vec![false; count as usize];
    let mut ndel = 0;
    for i in 0..count as usize {
        if use_real_ras == 0 {
            let pt = points[i];
            let (_, v) = surface_ras_to_voxel(g, out, err, &mut mri.vg, pt.x, pt.y, pt.z)?;
            let (xi, yi, zi) = (nint(g, v[0]), nint(g, v[1]), nint(g, v[2]));
            if mri_get_vox_val(g, err, mri, xi, yi, zi, 0, 0)? <= thresh {
                let (tx, ty, tz) = (
                    float_to_int(g, v[0], WHAT),
                    float_to_int(g, v[1], WHAT),
                    float_to_int(g, v[2], WHAT),
                );
                let val = mri_get_vox_val(g, err, mri, tx, ty, tz, 0, 0)?;
                let _ = writeln!(
                    out,
                    "unlikely control point {i} - ({}, {}, {}) = {}",
                    ff(v[0], 2, 1),
                    ff(v[1], 2, 1),
                    ff(v[2], 2, 1),
                    ff(f64::from(val), 2, 0)
                );
                deleted[i] = true;
                ndel += 1;
            }
        }
    }
    if ndel > 0 && cp.is_some() {
        let _ = writeln!(
            out,
            "FS_THRESH_CTRL detected in env - deleting {ndel} control points"
        );
        let mut i = 0usize;
        let mut skipped = false;
        while i < count as usize {
            if deleted[i] {
                deleted.remove(i);
                points.remove(i);
                count -= 1;
                if i < count as usize && deleted[i] {
                    skipped = true;
                    if !g.notices.reproduce() {
                        continue;
                    }
                }
            }
            i += 1;
        }
        if skipped {
            g.notices.add(
                &FS_594,
                format!("{WHAT}: a control point below the threshold right after a removed one"),
            );
        }
        let _ = writeln!(
            out,
            "writing updated control point file to control.edited.dat"
        );
        mri_write_control_points(g, env, err, dir, &points, count, 0, "control.edited.dat");
    }
    let mut ctrl = Vec::with_capacity(count as usize);
    for pt in points.iter().take(count as usize) {
        let v = match use_real_ras {
            0 => surface_ras_to_voxel(g, out, err, &mut mri.vg, pt.x, pt.y, pt.z)?.1,
            1 => {
                let (a, b, c) = world_to_voxel(g, out, err, &mut mri.vg, pt.x, pt.y, pt.z)?;
                [a, b, c]
            }
            u => {
                log::error_exit(
                    g,
                    err,
                    ERROR_BADPARM,
                    &format!("{WHAT} has bad useRealRAS flag {u}\n"),
                )?;
                return Ok(ERROR_BADPARM);
            }
        };
        let c = [nint(g, v[0]), nint(g, v[1]), nint(g, v[2])];
        if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
            let _ = writeln!(err, "( {:5}, {:5}, {:5} )", c[0], c[1], c[2]);
        }
        ctrl.push(c);
    }
    st.ctrl = ctrl;
    Ok(NO_ERROR)
}

/// `MRI3dUseLabelControlPoints(mri, label)`: the state's points from the label's points (made
/// voxel coordinates by `LabelToVoxel` unless they are), each rounded;
/// `read in <n> label control points` on standard output.
pub fn mri_3d_use_label_control_points(
    g: &mut Globals,
    io: &mut LabelIo,
    st: &mut NormState,
    mri: &mut Mri,
    label: &Label,
) -> Result<i32, Fatal> {
    let made;
    let area: &Label = if label.coords != LABEL_COORDS_VOXEL {
        made = label_to_voxel(g, io, label, mri, None, LabelDst::New)?;
        &made
    } else {
        label
    };
    let count = area.n_points.max(0) as usize;
    let mut ctrl = Vec::with_capacity(count);
    for v in area.lv.iter().take(count) {
        let c = [
            nint(g, f64::from(v.x)),
            nint(g, f64::from(v.y)),
            nint(g, f64::from(v.z)),
        ];
        if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
            let _ = writeln!(io.err, "( {:5}, {:5}, {:5} )", c[0], c[1], c[2]);
        }
        ctrl.push(c);
    }
    st.ctrl = ctrl;
    let _ = writeln!(io.out, "read in {} label control points", area.n_points);
    Ok(NO_ERROR)
}

/// `MRI3dWriteControlPoints(fname)`: the control volume's name the normalisations write.
pub fn mri_3d_write_control_points(st: &mut NormState, fname: Option<&str>) -> i32 {
    st.control_volume_fname = fname.map(str::to_owned);
    NO_ERROR
}

/// `MRI3dWriteBias(fname)`: the bias field's name the normalisations write.
pub fn mri_3d_write_bias(st: &mut NormState, fname: Option<&str>) -> i32 {
    st.bias_volume_fname = fname.map(str::to_owned);
    NO_ERROR
}

/// `MRInormAddFileControlPoints(mri_ctrl, value, mri)`: each of the state's points inside the
/// control volume becomes `value` (its low byte), except where `mri` is given and its voxel is not
/// above 0 (`control point <i> @ (x, y, z), MRI=<v>, skipping!!!!!` on standard output); points
/// outside are reported (`... not in volume, skipping!!!!!`). When any was rejected, the error
/// `!!!!! <n> control points rejected for being out of bounds !!!!!!` (FS-595: FreeSurfer prints a
/// number it was not given) and, the first time in the run, `volume size (w, h, d)`. The count of
/// voxels that were 0.
#[allow(clippy::too_many_arguments)]
pub fn mri_norm_add_file_control_points(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    st: &mut NormState,
    ctrl: &mut Mri,
    value: i32,
    mri: Option<&Mri>,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRInormAddFileControlPoints";
    let mut nctrl = 0;
    let mut bad: i64 = 0;
    // what FreeSurfer's register holds for the missing number: the last point's value when it
    // was skipped for it, else unknown
    let mut register: Option<i64> = None;
    let points = st.ctrl.clone();
    for (i, &[x, y, z]) in points.iter().enumerate() {
        if larmorx_freesurfer_volume::coords::index_not_in_volume(
            ctrl,
            f64::from(x),
            f64::from(y),
            f64::from(z),
        ) == 0
        {
            if let Some(m) = mri
                && mri_get_vox_val(g, err, m, x, y, z, 0, 0)? <= 0.0
            {
                let v = mri_get_vox_val(g, err, m, x, y, z, 0, 0)?;
                let iv = float_to_int(g, f64::from(v), WHAT);
                let _ = writeln!(
                    out,
                    "control point {i} @ ({x}, {y}, {z}), MRI={iv}, skipping!!!!!"
                );
                bad += 1;
                register = Some(i64::from(iv as u32));
                continue;
            }
            if cget(g, err, WHAT, ctrl, x, y, z)? == 0.0 {
                nctrl += 1;
            }
            cset(g, err, WHAT, ctrl, x, y, z, value as u8)?;
            register = None;
        } else {
            let _ = writeln!(
                out,
                "control point {i} @ ({x}, {y}, {z}) not in volume, skipping!!!!!"
            );
            bad += 1;
            register = None;
        }
    }
    if bad > 0 {
        g.notices.add(
            &FS_595,
            "MRInormAddFileControlPoints: the rejected points' message is given no count",
        );
        let shown = if g.notices.reproduce() {
            match register {
                Some(r) => r,
                None => {
                    g.notices.add(
                        &FS_596,
                        "MRInormAddFileControlPoints: the rejected points' number is a register's contents (printed: the count)",
                    );
                    bad
                }
            }
        } else {
            bad
        };
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            &format!("!!!!! {shown} control points rejected for being out of bounds !!!!!!\n"),
        );
        if !st.size_reported {
            let (w, h, d) = ext(ctrl);
            log::error_printf(
                g,
                err,
                ERROR_BADFILE,
                &format!("volume size ({w}, {h}, {d})\n"),
            );
            st.size_reported = true;
        }
    }
    Ok(nctrl)
}

/// `MRIbuildBiasImage(mri_src, mri_ctrl, mri_bias, sigma)` (FreeSurfer makes a new volume whatever
/// `mri_bias` is): a clone of the source holding the control points' Voronoi diagram
/// (`building Voronoi diagram...`), smoothed by a Gaussian of `sigma` (`MRIgaussian1d(sigma, -1)`;
/// `performing soap bubble smoothing, sigma = <%g>...`), then the control voxels (bytes above 0)
/// set back to the source's values (in x, y, z order).
pub fn mri_build_bias_image(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    ctrl: &mut Mri,
    sigma: f32,
) -> Result<Mri, Fatal> {
    const WHAT: &str = "MRIbuildBiasImage";
    let kernel = mri_gaussian1d(g, out, err, sigma, -1)?;
    let mut bias = mri_clone(g, out, err, src, None)?.unwrap_new();
    let _ = writeln!(out, "building Voronoi diagram...");
    mri_build_voronoi_diagram(g, out, err, src, ctrl, Some(&mut bias))?;
    let _ = writeln!(
        out,
        "performing soap bubble smoothing, sigma = {}...",
        cfmt::g(f64::from(sigma), 6)
    );
    mri_convolve_gaussian(g, out, err, Pair::Same(&mut bias), &kernel)?;
    let (w, h, d) = ext(src);
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                if cget(g, err, WHAT, ctrl, x, y, z)? > 0.0 {
                    let v = mri_get_vox_val(g, err, src, x, y, z, 0, 0)?;
                    mri_set_vox_val(g, err, &mut bias, x, y, z, 0, v, 0)?;
                }
            }
        }
    }
    Ok(bias)
}

/// The destination of a normalisation: a new clone, a given volume, or the source itself.
pub enum Dst<'a> {
    New,
    Given(&'a mut Mri),
    Same,
}

/// Writes `m` to `fname` (`MRIwrite`).
fn write_vol(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    m: &Mri,
    fname: &str,
) -> Result<(), Fatal> {
    mri_write(g, out, err, env, dir, m, fname, None, 0)?;
    Ok(())
}

/// `MRI3dGentleNormalize(mri_src, mri_bias, wm_target, mri_norm, intensity_above,
/// intensity_below, only_file, bias_sigma, mri_not_control)` (`wm_target` 0: 110): without a bias
/// field, the control points (`MRInormGentlyFindControlPoints`, or with `only_file` the file's
/// alone: the source scaled in place so their mean is `wm_target`, `only using <n> control points
/// from file, mean <m>, scaling by <s>...`; none or a mean of 0: the source copied and the error
/// `MRI3dGentleNormalize: mean = ..., nctrl = ..., norm failed`), written to the control volume's
/// name when set, binarised, then `MRIbuildBiasImage` (written to the bias name when set); each
/// voxel (truncated) times `wm_target` over its bias (truncated; the value itself where the bias
/// is 0), at most 255 in a byte destination. `WRITE_CONTROL_POINTS` writes `c.mgz` (with
/// `only_file`).
#[allow(clippy::too_many_arguments)]
pub fn mri_3d_gentle_normalize(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    st: &mut NormState,
    src: &mut Mri,
    bias: Option<&Mri>,
    wm_target: f32,
    norm: Dst,
    intensity_above: f32,
    intensity_below: f32,
    only_file: i32,
    bias_sigma: f32,
    not_control: Option<&Mri>,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRI3dGentleNormalize";
    let wm_target = if wm_target == 0.0 {
        DEFAULT_DESIRED_WHITE_MATTER_VALUE as f32
    } else {
        wm_target
    };
    let (w, h, d) = ext(src);
    let mut fresh = match norm {
        Dst::New => Some(mri_clone(g, out, err, src, None)?.unwrap_new()),
        _ => None,
    };
    let mut given = match norm {
        Dst::Given(m) => Some(m),
        _ => None,
    };
    let made_bias;
    let bias: &Mri = match bias {
        Some(b) => b,
        None => {
            let mut ctrl = if only_file == 0 {
                mri_norm_gently_find_control_points(
                    g,
                    out,
                    err,
                    st,
                    src,
                    wm_target,
                    intensity_above,
                    intensity_below,
                    None,
                    not_control,
                )?
                .unwrap_new()
            } else {
                let mut c = mri_alloc(g, out, err, w, h, d, MRI_UCHAR)?;
                mri_copy_header(g, out, err, src, Some(&mut c))?;
                let nctrl =
                    mri_norm_add_file_control_points(g, out, err, st, &mut c, 255, Some(src))?;
                let mean = mri_mean_in_label(g, err, src, &c, 255)?;
                if nctrl == 0 || mean.abs() < f64::from(FLT_EPSILON) {
                    match (fresh.as_mut(), given.as_deref_mut()) {
                        (Some(n), _) | (None, Some(n)) => {
                            mri_copy(g, out, err, src, Some(n))?;
                        }
                        _ => {}
                    }
                    log::error_printf(
                        g,
                        err,
                        ERROR_BADPARM,
                        &format!(
                            "MRI3dGentleNormalize: mean = {}, nctrl = {nctrl}, norm failed",
                            ff(mean, 2, 1)
                        ),
                    );
                    return Ok(match fresh {
                        Some(n) => Ret::New(n),
                        None => Ret::Given,
                    });
                }
                let scale = div64(f64::from(wm_target), mean);
                let _ = writeln!(
                    err,
                    "only using {nctrl} control points from file, mean {}, scaling by {}...",
                    ff(mean, 2, 1),
                    ff(scale, 2, 2)
                );
                mri_scalar_mul(g, out, err, Pair::Same(src), scale as f32)?;
                if env.get("WRITE_CONTROL_POINTS").is_some() {
                    let _ = writeln!(out, "writing control point volume to c.mgz");
                    write_vol(g, out, err, env, dir, &c, "c.mgz")?;
                }
                if g.gdiag & DIAG_WRITE != 0 {
                    let _ = writeln!(out, "writing scaled intensity volume to scaled.mgz");
                    write_vol(g, out, err, env, dir, src, "scaled.mgz")?;
                }
                c
            };
            if let Some(name) = st.control_volume_fname.clone() {
                let _ = writeln!(err, "writing control point volume to {name}...");
                write_vol(g, out, err, env, dir, &ctrl, &name)?;
            }
            mri_binarize(
                g,
                out,
                err,
                Pair::Same(&mut ctrl),
                1.0,
                CONTROL_NONE as f32,
                CONTROL_MARKED as f32,
            )?;
            let b = mri_build_bias_image(g, out, err, src, &mut ctrl, bias_sigma)?;
            if let Some(name) = st.bias_volume_fname.clone() {
                let _ = writeln!(err, "writing bias field volume to {name}...");
                write_vol(g, out, err, env, dir, &b, &name)?;
            }
            if g.gdiag & DIAG_WRITE != 0 && g.gdiag & DIAG_VERBOSE != 0 {
                let pass = st.gentle_pass;
                let _ = writeln!(err, "writing out control and bias volumes...");
                write_vol(g, out, err, env, dir, src, &format!("src{pass}.mgz"))?;
                write_vol(g, out, err, env, dir, &ctrl, &format!("ctrl{pass}.mgz"))?;
                write_vol(g, out, err, env, dir, &b, &format!("bias{pass}.mgz"))?;
                st.gentle_pass += 1;
            }
            made_bias = b;
            &made_bias
        }
    };
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let sv = mri_get_vox_val(g, err, src, x, y, z, 0, 0)?;
                let s = float_to_int(g, f64::from(sv), WHAT);
                let bv = mri_get_vox_val(g, err, bias, x, y, z, 0, 0)?;
                let b = float_to_int(g, f64::from(bv), WHAT);
                let mut v = if b == 0 {
                    s as f32
                } else {
                    div(mul(s as f32, wm_target), b as f32)
                };
                let n: &mut Mri = match (fresh.as_mut(), given.as_deref_mut()) {
                    (Some(n), _) | (None, Some(n)) => n,
                    _ => src,
                };
                if v > 255.0 && n.mtype == MRI_UCHAR {
                    v = 255.0;
                }
                mri_set_vox_val(g, err, n, x, y, z, 0, v, 0)?;
            }
        }
    }
    Ok(match fresh {
        Some(n) => Ret::New(n),
        None => Ret::Given,
    })
}

/// `csf_in_window(mri, x0, y0, z0, max_dist, csf)`: 1 when a voxel within `max_dist` (voxel
/// distances from the clamped positions, `float` sums, a window of half-width
/// `nint(max_dist / xsize)`) is at most `csf` (bytes read directly in a byte volume), else 0.
#[allow(clippy::too_many_arguments)]
pub fn csf_in_window(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x0: i32,
    y0: i32,
    z0: i32,
    max_dist: f32,
    csf: f32,
) -> Result<f32, Fatal> {
    const WHAT: &str = "csf_in_window";
    let whalf = nint(g, f64::from(div(max_dist, mri.vg.xsize)));
    if !check_window(g, err, WHAT, whalf)? {
        return Ok(0.0);
    }
    let t = Idx::of(mri, whalf);
    let byte = mri.mtype == MRI_UCHAR;
    for xk in -whalf..=whalf {
        let xi = t.x(x0 + xk);
        let dx = (xi - x0).wrapping_mul(xi - x0) as f32;
        for yk in -whalf..=whalf {
            let yi = t.y(y0 + yk);
            let dy = (yi - y0).wrapping_mul(yi - y0) as f32;
            for zk in -whalf..=whalf {
                let zi = t.z(z0 + zk);
                let dz = (zi - z0).wrapping_mul(zi - z0) as f32;
                let dist = add(add(dx, dy), dz).sqrt();
                if dist > max_dist {
                    continue;
                }
                let val = if byte {
                    mem::read(
                        g,
                        CTRL_NOTES,
                        WHAT,
                        mri,
                        i64::from(zi),
                        i64::from(yi),
                        i64::from(xi),
                        MRI_UCHAR,
                    )
                } else {
                    mri_get_vox_val(g, err, mri, xi, yi, zi, 0, 0)?
                };
                if val <= csf {
                    return Ok(1.0);
                }
            }
        }
    }
    Ok(0.0)
}

/// `MRIapplyBiasCorrectionSameGeometry(mri_in, mri_bias, mri_out, target_val)` (`Pair::Same`: in
/// place): each voxel times `target_val / bias` (`double`; the bias sampled at the voxel's own
/// position, in x, y, z order). Without a destination, `MRIclone` of the input.
pub fn mri_apply_bias_correction_same_geometry(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    bias: &Mri,
    target_val: f32,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let (src, dst): (Option<&Mri>, &mut Mri) = match p {
        Pair::New(s) => (
            Some(s),
            fresh.insert(mri_clone(g, out, err, s, None)?.unwrap_new()),
        ),
        Pair::Given(s, d) => (Some(s), d),
        Pair::Same(d) => (None, d),
    };
    let (w, h, d) = ext(src.unwrap_or(dst));
    // `bias` is one double for the whole loop: a sample the sampler does not write keeps the last
    let mut b = 0.0f64;
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                let val = f64::from(mri_get_vox_val(g, err, src.unwrap_or(dst), x, y, z, 0, 0)?);
                sample_volume(
                    g,
                    err,
                    bias,
                    f64::from(x),
                    f64::from(y),
                    f64::from(z),
                    &mut b,
                );
                let v = mul64(val, div64(f64::from(target_val), b));
                mri_set_vox_val(g, err, dst, x, y, z, 0, v as f32, 0)?;
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}
