// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_normalize/mri_normalize.cpp`'s own functions besides `main`, `get_option` and
//! `usage_exit` (queue item 3.2-24): `compute_bias` 1350, `MRIremoveWMOutliersAndRetainMedialSurface`
//! 1386 and `MRIremoveWMOutliers` 1474 (the `#if 1` forms), `add_interior_points` 1721,
//! `remove_surface_outliers` 1800, `remove_nonwm_voxels` 1876, `remove_outliers_near_surface` 1956,
//! `build_outside_of_brain_mask` 1985, `remove_surface_outliers_T2` 2016,
//! `threshold_control_points` 2119 and `remove_controlpoints_outside_range` 2141.
//!
//! Each takes the volumes as `main` passes them: a control volume that is both the function's
//! source and its destination is one `&mut Mri`. The diagnostics are FreeSurfer's: with `Gdiag`'s
//! write bit (and its verbose bit where FreeSurfer asks for both) the files `sc.mgz`, `dc.mgz`,
//! `out.mgz`, `i.mgz`, `h.plt`, `hs.plt` and `o.mgz` in the working directory; the voxel `Gx`,
//! `Gy`, `Gz` printed where FreeSurfer prints it. FS-1014, FS-1015.

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_1014, FS_1015};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::diag::{DIAG_WRITE, diag_break, verbose_on};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::histo::{
    Histogram, histo_copy, histo_find_highest_peak_in_region, histo_plot, histo_smooth,
};
use larmorx_freesurfer_core::log::{self, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_multiply};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse::{add, div, div64, mul, mul64, sqrt};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_filter::fastmarch::{DTRANS_MODE_SIGNED, mri_distance_transform};
use larmorx_freesurfer_filter::norm::{CONTROL_MARKED, CONTROL_NONE};
use larmorx_freesurfer_filter::sobel::mri_sobel;
use larmorx_freesurfer_gca::gca::{Gca, gca_source_voxel_to_prior, voxel_to_prior_matrix};
use larmorx_freesurfer_gca::transform::Transform;
use larmorx_freesurfer_io::mriio::mri_write;
use larmorx_freesurfer_io::streams::Io;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc, mri_copy_header};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::mriflood::mris_fill_interior;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_volume::arith::{Pair, mri_binarize, mri_clone, mri_scalar_mul};
use larmorx_freesurfer_volume::cma::*;
use larmorx_freesurfer_volume::deriv::mri_non_max_suppress;
use larmorx_freesurfer_volume::frames::mri_clone_different_type;
use larmorx_freesurfer_volume::labels::{mri_labels_in_nbhd, mri_max_in_label_in_region};
use larmorx_freesurfer_volume::mrihisto::mri_histogram_label;
use larmorx_freesurfer_volume::mriset::{mri_copy_label, mri_erode};
use larmorx_freesurfer_volume::orient::mri_get_voxel_to_voxel_xform;

/// `FLT_EPSILON`, `FZERO`'s bound.
const FLT_EPSILON: f32 = f32::EPSILON;

/// `remove_surface_outliers_T2`'s `nonmax_thresh` (a file static no option changes).
const NONMAX_THRESH: f32 = 2.5;

fn get(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &Mri,
    x: i32,
    y: i32,
    z: i32,
) -> Result<f32, Fatal> {
    mri_get_vox_val(g, err, m, x, y, z, 0, 0)
}

fn get_frame(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &Mri,
    x: i32,
    y: i32,
    z: i32,
    f: i32,
) -> Result<f32, Fatal> {
    mri_get_vox_val(g, err, m, x, y, z, f, 0)
}

fn set(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &mut Mri,
    x: i32,
    y: i32,
    z: i32,
    v: f32,
) -> Result<(), Fatal> {
    mri_set_vox_val(g, err, m, x, y, z, 0, v, 0)?;
    Ok(())
}

fn ext(m: &Mri) -> (i32, i32, i32) {
    (m.vg.width, m.vg.height, m.vg.depth)
}

fn at_g(g: &Globals, x: i32, y: i32, z: i32) -> bool {
    x == g.gx && y == g.gy && z == g.gz
}

fn diag_write(g: &Globals) -> bool {
    g.gdiag & DIAG_WRITE != 0
}

fn diag_write_verbose(g: &Globals) -> bool {
    diag_write(g) && verbose_on(g)
}

/// The streams and the place relative names start in.
pub struct Run<'a, 'b> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: &'b Environment,
    pub dir: Option<&'b Path>,
}

fn write(g: &mut Globals, r: &mut Run, m: &mut Mri, fname: &str) -> Result<(), Fatal> {
    mri_write(g, r.out, r.err, r.env, r.dir, m, fname, None, 0)?;
    Ok(())
}

/// `%2.1f` and its kin: C's `%<w>.<p>f`.
fn ff(v: f64, w: usize, p: usize) -> String {
    cfmt::pad(&cfmt::f(v, p), w)
}

/// `compute_bias(mri_src, mri_dst, NULL)`: a new `float` volume with the source's extent (one
/// frame) and header, each voxel `dst / src` of frame 0 (`float`), or 1 where the source's value
/// is within `FLT_EPSILON` of 0.
pub fn compute_bias(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: &Mri,
) -> Result<Mri, Fatal> {
    let (w, h, d) = ext(src);
    let mut bias = mri_alloc(g, out, err, w, h, d, MRI_FLOAT)?;
    mri_copy_header(g, out, err, src, Some(&mut bias))?;
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                let s = get(g, err, src, x, y, z)?;
                let t = get(g, err, dst, x, y, z)?;
                let b = if s.abs() < FLT_EPSILON {
                    1.0
                } else {
                    div(t, s)
                };
                set(g, err, &mut bias, x, y, z, b)?;
            }
        }
    }
    Ok(bias)
}

/// `bins[b]` of the smoothed histogram's highest peak, converted to `int` (`wm_peak =
/// hsmooth->bins[wm_peak]`). FS-1014: a histogram without any count above 0 (no control point in
/// the label) has no peak (bin -1), and FreeSurfer reads the `float` before the bins (on glibc's
/// heap the upper half of the allocation's size field: 0); both modes use 0, noted.
fn peak_value(g: &mut Globals, h: &Histogram, b: i32, what: &str) -> i32 {
    let v = match usize::try_from(b).ok().and_then(|i| h.bins.get(i)) {
        Some(&v) => v,
        None => {
            g.notices.add(
                &FS_1014,
                format!(
                    "{what}: no white-matter peak (no control point in the label): bin {b} read \
                     outside the histogram (memory contents; 0 used)"
                ),
            );
            0.0
        }
    };
    float_to_int(g, f64::from(v), what)
}

/// The white-matter peak both outlier functions find: the label's histogram over 256 bins
/// (`MRIhistogramLabel`), a copy smoothed with sigma 2, the highest peak in bins 1 to the last,
/// plotted to `h.plt` and `hs.plt` with the verbose diagnostics.
fn wm_peak(
    g: &mut Globals,
    r: &mut Run,
    src: &Mri,
    labeled: &Mri,
    what: &str,
) -> Result<i32, Fatal> {
    let histo = mri_histogram_label(g, r.err, src, labeled, 1, 256)?;
    let mut hsmooth = histo_copy(g, &histo, None).unwrap_new();
    histo_smooth(g, &histo, Some(&mut hsmooth), 2.0);
    if diag_write_verbose(g) {
        histo_plot(&histo, r.dir, "h.plt");
        histo_plot(&hsmooth, r.dir, "hs.plt");
    }
    let b = histo_find_highest_peak_in_region(&hsmooth, 1, hsmooth.nbins - 1);
    Ok(peak_value(g, &hsmooth, b, what))
}

/// The removal loop of both outlier functions: each voxel of `ctrl` whose value rounds to a
/// number other than 0 (x outermost, z innermost) is removed (set to 0; 128 in the outlier volume)
/// when `val + intensity_below` is below the largest source value of `labeled`'s label 1 in the
/// 11-voxel cube around it (`MRImaxInLabelInRegion`, as a `float`) and `val` below `thresh`.
/// `labeled` `None`: `ctrl` itself, as it is at each voxel. The removed and visited counts.
#[allow(clippy::too_many_arguments)]
fn remove_loop(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    ctrl: &mut Mri,
    labeled: Option<&Mri>,
    intensity_below: i32,
    thresh: f32,
    mut outliers: Option<&mut Mri>,
) -> Result<(i64, i32), Fatal> {
    const WHALF: i32 = 5;
    let (w, h, d) = ext(src);
    let (mut nremoved, mut total) = (0i64, 0i32);
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                if at_g(g, x, y, z) {
                    diag_break();
                }
                let c = get(g, err, ctrl, x, y, z)?;
                if nint(g, f64::from(c)) == 0 {
                    continue;
                }
                let lab: &Mri = labeled.unwrap_or(&*ctrl);
                let max = mri_max_in_label_in_region(g, err, src, lab, 1, x, y, z, WHALF)? as f32;
                let val = get(g, err, src, x, y, z)?;
                total = total.wrapping_add(1);
                if val + (intensity_below as f32) < max && val < thresh {
                    set(g, err, ctrl, x, y, z, 0.0)?;
                    if let Some(o) = outliers.as_deref_mut() {
                        set(g, err, o, x, y, z, 128.0)?;
                    }
                    nremoved += 1;
                }
            }
        }
    }
    Ok((nremoved, total))
}

/// `MRIremoveWMOutliersAndRetainMedialSurface(mri_src, mri_ctrl, mri_ctrl, intensity_below)` (the
/// control volume both source and destination, as `main` calls it): `sc.mgz` with the verbose
/// diagnostics; the control volume binarised at 1 (0 and 1), its signed distance transform
/// (`MRIdistanceTransform`, label 1, no limit) negated, its non-maximum suppression, and the
/// control volume replaced by that binarised at 1; then the white-matter peak of the source in
/// it, `thresh = peak - intensity_below`, and the outliers removed with the control volume itself
/// as the label volume (each removal seen by the later voxels). `<n> control points removed`;
/// with the write diagnostics the outliers in `out.mgz` (`writing out.mgz outlier volume`); with
/// the verbose ones the control volume in `dc.mgz`.
pub fn mri_remove_wm_outliers_and_retain_medial_surface(
    g: &mut Globals,
    r: &mut Run,
    src: &Mri,
    ctrl: &mut Mri,
    intensity_below: i32,
) -> Result<(), Fatal> {
    if diag_write_verbose(g) {
        write(g, r, ctrl, "sc.mgz")?;
    }
    let bin = mri_binarize(g, r.out, r.err, Pair::New(ctrl), 1.0, 0.0, 1.0)?.unwrap_new();
    let mut dist = mri_distance_transform(
        g,
        r.out,
        r.err,
        r.env,
        &bin,
        None,
        1,
        -1.0,
        DTRANS_MODE_SIGNED,
        None,
    )?
    .unwrap_new();
    mri_scalar_mul(g, r.out, r.err, Pair::Same(&mut dist), -1.0)?;
    let sup = mri_non_max_suppress(g, r.out, r.err, &dist, None, 0.0, 1)?.unwrap_new();
    mri_binarize(g, r.out, r.err, Pair::Given(&sup, ctrl), 1.0, 0.0, 1.0)?;
    let peak = wm_peak(g, r, src, ctrl, "MRIremoveWMOutliersAndRetainMedialSurface")?;
    let thresh = peak.wrapping_sub(intensity_below) as f32;
    let mut outliers = if diag_write(g) {
        Some(mri_clone(g, r.out, r.err, ctrl, None)?.unwrap_new())
    } else {
        None
    };
    let (nremoved, _) = remove_loop(
        g,
        r.err,
        src,
        ctrl,
        None,
        intensity_below,
        thresh,
        outliers.as_mut(),
    )?;
    let _ = writeln!(r.out, "{nremoved} control points removed");
    if let Some(mut o) = outliers {
        let _ = writeln!(r.out, "writing out.mgz outlier volume");
        write(g, r, &mut o, "out.mgz")?;
    }
    if diag_write_verbose(g) {
        write(g, r, ctrl, "dc.mgz")?;
    }
    Ok(())
}

/// `MRIremoveWMOutliers(mri_src, mri_ctrl, mri_ctrl, intensity_below)` (the control volume both
/// source and destination, as `main` calls it): `sc.mgz` with the verbose diagnostics; the white-
/// matter peak of the source in the control volume binarised at 1 (0 and `CONTROL_MARKED`),
/// `thresh = peak - intensity_below`, and the outliers removed against that binarised volume
/// (fixed: the removals do not change it). `<n> control points removed (<p>%)` with `p = 100 n /
/// visited` (`-nan` when none was visited); `out.mgz` and `dc.mgz` as
/// [`mri_remove_wm_outliers_and_retain_medial_surface`].
pub fn mri_remove_wm_outliers(
    g: &mut Globals,
    r: &mut Run,
    src: &Mri,
    ctrl: &mut Mri,
    intensity_below: i32,
) -> Result<(), Fatal> {
    if diag_write_verbose(g) {
        write(g, r, ctrl, "sc.mgz")?;
    }
    let bin = mri_binarize(
        g,
        r.out,
        r.err,
        Pair::New(ctrl),
        1.0,
        0.0,
        CONTROL_MARKED as f32,
    )?
    .unwrap_new();
    let peak = wm_peak(g, r, src, &bin, "MRIremoveWMOutliers")?;
    let thresh = peak.wrapping_sub(intensity_below) as f32;
    let mut outliers = if diag_write(g) {
        Some(mri_clone(g, r.out, r.err, ctrl, None)?.unwrap_new())
    } else {
        None
    };
    let (nremoved, total) = remove_loop(
        g,
        r.err,
        src,
        ctrl,
        Some(&bin),
        intensity_below,
        thresh,
        outliers.as_mut(),
    )?;
    let pct = div64(mul64(100.0, nremoved as f64), f64::from(total));
    let _ = writeln!(
        r.out,
        "{nremoved} control points removed ({}%)",
        ff(pct, 2, 1)
    );
    if let Some(mut o) = outliers {
        let _ = writeln!(r.out, "writing out.mgz outlier volume");
        write(g, r, &mut o, "out.mgz")?;
    }
    if diag_write_verbose(g) {
        write(g, r, ctrl, "dc.mgz")?;
    }
    Ok(())
}

/// `add_interior_points(mri_ctrl, mri_vals, intensity_above, intensity_below, mris1, mris2,
/// mri_aseg, mri_ctrl)` (the control volume both source and destination, as `main` calls it):
/// the interiors of both surfaces (`MRISfillInterior` at the control volume's column size, into
/// clones of it) joined (the second's 1s copied into the first; `i.mgz` with the verbose
/// diagnostics), eroded once and then `nint(1 / xsize) - 1` more times (the core). Without a
/// segmentation every voxel of the core above 0 becomes a control point of its value; with one,
/// every voxel that is not a control point, is cortex or white matter in the segmentation
/// (`IS_GM`, `IS_WM`) and has a value within `[110 - intensity_below, 110 + intensity_above]`
/// in `vals` inside the core becomes 1 (each voxel written back, its value converted to `int`).
#[allow(clippy::too_many_arguments)]
pub fn add_interior_points(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    r: &mut Run,
    ctrl: &mut Mri,
    vals: &Mri,
    intensity_above: f32,
    intensity_below: f32,
    mris_white1: &mut Mris,
    mris_white2: &mut Mris,
    aseg: Option<&Mri>,
) -> Result<(), Fatal> {
    const WHAT: &str = "add_interior_points";
    let res = f64::from(ctrl.vg.xsize);
    let mut interior = mri_clone(g, r.out, r.err, ctrl, None)?.unwrap_new();
    {
        let mut io = Io {
            out: &mut *r.out,
            err: &mut *r.err,
            env: r.env,
        };
        mris_fill_interior(g, sg, &mut io, mris_white1, res, Some(&mut interior))?;
    }
    let mut tmp = mri_clone(g, r.out, r.err, ctrl, None)?.unwrap_new();
    {
        let mut io = Io {
            out: &mut *r.out,
            err: &mut *r.err,
            env: r.env,
        };
        mris_fill_interior(g, sg, &mut io, mris_white2, res, Some(&mut tmp))?;
    }
    mri_copy_label(g, r.err, &tmp, &mut interior, 1)?;
    if diag_write_verbose(g) {
        write(g, r, &mut interior, "i.mgz")?;
    }
    let mut core = mri_erode(g, r.out, r.err, Pair::New(&interior))?.unwrap_new();
    let n = nint(g, 1.0 / f64::from(ctrl.vg.xsize));
    for _ in 1..n {
        mri_copy(g, r.out, r.err, &core, Some(&mut tmp))?;
        mri_erode(g, r.out, r.err, Pair::Given(&tmp, &mut core))?;
    }
    drop(tmp);
    let (w, h, d) = ext(ctrl);
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                if at_g(g, x, y, z) {
                    diag_break();
                }
                match aseg {
                    None => {
                        let v = get(g, r.err, &core, x, y, z)?;
                        let c = float_to_int(g, f64::from(v), WHAT);
                        if c > 0 {
                            set(g, r.err, ctrl, x, y, z, c as f32)?;
                        }
                    }
                    Some(aseg) => {
                        let v = get(g, r.err, ctrl, x, y, z)?;
                        let mut c = float_to_int(g, f64::from(v), WHAT);
                        if c == 0 {
                            let lv = get(g, r.err, aseg, x, y, z)?;
                            let label = float_to_int(g, f64::from(lv), WHAT);
                            if is_gm(label) || is_wm(label) {
                                let val = get(g, r.err, vals, x, y, z)?;
                                if val >= 110.0 - intensity_below
                                    && val <= 110.0 + intensity_above
                                    && get(g, r.err, &core, x, y, z)? > 0.0
                                {
                                    c = 1;
                                }
                            }
                        }
                        set(g, r.err, ctrl, x, y, z, c as f32)?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// `remove_surface_outliers(mri_ctrl, mri_dist, mri_dst, mri_ctrl, min_dist)`: returns at once
/// (FreeSurfer's body after its first `return` never runs; the clone it makes first is never
/// used). Nothing to do.
pub fn remove_surface_outliers() -> i32 {
    0
}

/// `remove_outliers_near_surface(...)`: returns 0 at once (its body never runs).
pub fn remove_outliers_near_surface() -> i32 {
    0
}

/// The labels `remove_nonwm_voxels` removes control points on.
const NON_WM: [i32; 31] = [
    Left_Accumbens_area,
    Left_Thalamus_Proper,
    Left_Lateral_Ventricle,
    Left_Caudate,
    Left_Putamen,
    Left_choroid_plexus,
    Left_Pallidum,
    Left_Amygdala,
    Left_Hippocampus,
    Left_Cerebellum_Cortex,
    Left_Inf_Lat_Vent,
    WM_hypointensities,
    Left_WM_hypointensities,
    Right_WM_hypointensities,
    non_WM_hypointensities,
    Left_non_WM_hypointensities,
    Right_non_WM_hypointensities,
    Right_Accumbens_area,
    Right_choroid_plexus,
    Right_Thalamus_Proper,
    Right_Lateral_Ventricle,
    Right_Caudate,
    Right_Putamen,
    Right_Pallidum,
    Right_Amygdala,
    Right_Hippocampus,
    Right_Cerebellum_Cortex,
    Right_Inf_Lat_Vent,
    Third_Ventricle,
    Fourth_Ventricle,
    Unknown,
];

/// `remove_nonwm_voxels(mri_ctrl, mri_aseg, mri_ctrl)` (the control volume both source and
/// destination): each control point (a value other than 0) mapped into the segmentation
/// (`MRIgetVoxelToVoxelXform`, each coordinate rounded with `nint`); on a label of [`NON_WM`] it
/// is removed (`CONTROL_NONE`) and counted; on any other it is removed when the 3-voxel cube
/// around it holds a lateral ventricle (left or right), else set to `CONTROL_MARKED`. Then `<n>
/// non wm control points removed` (the first kind only). A point mapped outside the
/// segmentation reads it as `MRIgetVoxVal` does there (FS-52, FS-53).
pub fn remove_nonwm_voxels(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctrl: &mut Mri,
    aseg: &Mri,
) -> Result<i32, Fatal> {
    const WHAT: &str = "remove_nonwm_voxels";
    let m = mri_get_voxel_to_voxel_xform(g, out, err, ctrl, aseg)?;
    let mut v_src = Matrix::real(4, 1);
    let mut v_dst = Matrix::real(4, 1);
    v_src.set_relt(4, 1, 1.0);
    v_dst.set_relt(4, 1, 1.0);
    let mut removed = 0;
    let (w, h, d) = ext(ctrl);
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                if at_g(g, x, y, z) {
                    diag_break();
                }
                if get(g, err, ctrl, x, y, z)? == 0.0 {
                    continue;
                }
                v_src.set_relt(1, 1, x as f32);
                v_src.set_relt(2, 1, y as f32);
                v_src.set_relt(3, 1, z as f32);
                matrix_multiply(g, out, err, m.as_ref(), Some(&v_src), Some(&mut v_dst))?;
                let xa = nint(g, f64::from(v_dst.relt(1, 1)));
                let ya = nint(g, f64::from(v_dst.relt(2, 1)));
                let za = nint(g, f64::from(v_dst.relt(3, 1)));
                let lv = get(g, err, aseg, xa, ya, za)?;
                let label = float_to_int(g, f64::from(lv), WHAT);
                if NON_WM.contains(&label) {
                    removed += 1;
                    set(g, err, ctrl, x, y, z, CONTROL_NONE as f32)?;
                } else {
                    let near =
                        mri_labels_in_nbhd(g, err, aseg, xa, ya, za, 1, Left_Lateral_Ventricle)?
                            > 0
                            || mri_labels_in_nbhd(
                                g,
                                err,
                                aseg,
                                xa,
                                ya,
                                za,
                                1,
                                Right_Lateral_Ventricle,
                            )? > 0;
                    let v = if near { CONTROL_NONE } else { CONTROL_MARKED };
                    set(g, err, ctrl, x, y, z, v as f32)?;
                }
            }
        }
    }
    let _ = writeln!(out, "{removed} non wm control points removed");
    Ok(NO_ERROR)
}

/// `build_outside_of_brain_mask(mri_src, gca, xform, 0, whalf)`: a `uchar` volume of the source's
/// geometry, 1 where the voxel maps outside the atlas's prior grid (`GCAsourceVoxelToPrior`'s
/// code) or onto a prior point none of whose labels is brain (`IS_BRAIN`), then eroded `whalf -
/// 1` times in place.
#[allow(clippy::too_many_arguments)]
pub fn build_outside_of_brain_mask(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &Mri,
    gca: &Gca,
    xform: &mut Transform,
    whalf: i32,
) -> Result<Mri, Fatal> {
    let mut mask = mri_clone_different_type(g, out, err, src, MRI_UCHAR)?;
    let tal = voxel_to_prior_matrix(
        gca,
        gca.mri_tal.as_ref().expect("GCAread sets the atlas up"),
    );
    let (w, h, d) = ext(src);
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                let (code, p) =
                    gca_source_voxel_to_prior(g, out, err, env, gca, &tal, xform, [x, y, z])?;
                if code != NO_ERROR {
                    set(g, err, &mut mask, x, y, z, 1.0)?;
                    continue;
                }
                let brain = gca.prior(p[0], p[1], p[2]).is_some_and(|gcap| {
                    gcap.labels
                        .iter()
                        .take(gcap.nlabels.max(0) as usize)
                        .any(|&l| is_brain(i32::from(l)))
                });
                if !brain {
                    set(g, err, &mut mask, x, y, z, 1.0)?;
                }
            }
        }
    }
    for _ in 0..whalf.saturating_sub(1) {
        mri_erode(g, out, err, Pair::Same(&mut mask))?;
    }
    Ok(mask)
}

/// `remove_surface_outliers_T2(mri_ctrl, mri_dist, mri_src, mri_ctrl)` (the control volume both
/// source and destination): the source's Sobel gradient and magnitude, the distance's gradient
/// and its non-maximum suppression (written to `dg.mgz`, `g.mgz`, `m.mgz`), the magnitude's
/// non-maximum suppression at 2.5 (`nm1.mgz`) with each edge whose image gradient points along
/// the distance's gradient (dot product above 0) removed (`nm2.mgz`); the control volume
/// written to `c1.mgz`; then from each control point a walk in half-voxel steps along the
/// distance's unit gradient (at most 10 mm, the distance grown by half the column size first):
/// the walk stops on a zero gradient, at an edge (the point removed), where the direction turns
/// against the start's gradient, or on the distance's ridge; `c2.mgz` at the end. FS-1015: with
/// a column size of 0 or NaN (or one so small that 10 mm take more than [`MAX_WALK_STEPS`] steps)
/// a walk that meets nothing never ends in FreeSurfer: both modes end with an error at that many
/// steps (rule B).
pub fn remove_surface_outliers_t2(
    g: &mut Globals,
    r: &mut Run,
    ctrl: &mut Mri,
    dist: &Mri,
    src: &Mri,
) -> Result<(), Fatal> {
    let step_size: f32 = 0.5;
    let step_mm = mul(step_size, dist.vg.xsize);
    let mut mag = mri_clone_different_type(g, r.out, r.err, src, MRI_FLOAT)?;
    let mut grad = mri_sobel(g, r.out, r.err, src, None, Some(&mut mag))?.unwrap_new();
    let mut dist_grad = mri_sobel(g, r.out, r.err, dist, None, None)?.unwrap_new();
    let dist_sup = mri_non_max_suppress(g, r.out, r.err, dist, None, 0.0, 1)?.unwrap_new();
    write(g, r, &mut dist_grad, "dg.mgz")?;
    write(g, r, &mut grad, "g.mgz")?;
    write(g, r, &mut mag, "m.mgz")?;
    let mut nonmax =
        mri_non_max_suppress(g, r.out, r.err, &mag, None, NONMAX_THRESH, 1)?.unwrap_new();
    write(g, r, &mut nonmax, "nm1.mgz")?;
    let (w, h, d) = ext(dist);
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                if at_g(g, x, y, z) {
                    diag_break();
                }
                if get(g, r.err, &nonmax, x, y, z)?.abs() < FLT_EPSILON {
                    continue;
                }
                let ddx = get_frame(g, r.err, &dist_grad, x, y, z, 0)?;
                let ddy = get_frame(g, r.err, &dist_grad, x, y, z, 1)?;
                let ddz = get_frame(g, r.err, &dist_grad, x, y, z, 2)?;
                let sdx = get_frame(g, r.err, &grad, x, y, z, 0)?;
                let sdy = get_frame(g, r.err, &grad, x, y, z, 1)?;
                let sdz = get_frame(g, r.err, &grad, x, y, z, 2)?;
                let dot = sdx * ddx + sdy * ddy + sdz * ddz;
                if dot > 0.0 {
                    set(g, r.err, &mut nonmax, x, y, z, 0.0)?;
                }
            }
        }
    }
    write(g, r, &mut nonmax, "nm2.mgz")?;
    write(g, r, ctrl, "c1.mgz")?;
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                if at_g(g, x, y, z) {
                    diag_break();
                }
                if get(g, r.err, ctrl, x, y, z)? == 0.0 {
                    continue;
                }
                let odx = get_frame(g, r.err, &dist_grad, x, y, z, 0)?;
                let ody = get_frame(g, r.err, &dist_grad, x, y, z, 1)?;
                let odz = get_frame(g, r.err, &dist_grad, x, y, z, 2)?;
                let (mut xf, mut yf, mut zf) = (x as f32, y as f32, z as f32);
                let mut walked: f32 = 0.0;
                let (mut xv, mut yv, mut zv) = (
                    nint(g, f64::from(xf)),
                    nint(g, f64::from(yf)),
                    nint(g, f64::from(zf)),
                );
                let mut done;
                let mut steps: u64 = 0;
                loop {
                    steps += 1;
                    if steps > MAX_WALK_STEPS {
                        return walk_never_ends(g, r, dist, [x, y, z]);
                    }
                    walked = add(walked, step_mm);
                    let mut ddx = get_frame(g, r.err, &dist_grad, xv, yv, zv, 0)?;
                    let mut ddy = get_frame(g, r.err, &dist_grad, xv, yv, zv, 1)?;
                    let mut ddz = get_frame(g, r.err, &dist_grad, xv, yv, zv, 2)?;
                    let norm = sqrt(ddx * ddx + ddy * ddy + ddz * ddz);
                    if norm.abs() < FLT_EPSILON {
                        done = 1;
                        break;
                    }
                    ddx /= norm;
                    ddy /= norm;
                    ddz /= norm;
                    xf += ddx * step_size;
                    yf += ddy * step_size;
                    zf += ddz * step_size;
                    xv = nint(g, f64::from(xf));
                    yv = nint(g, f64::from(yf));
                    zv = nint(g, f64::from(zf));
                    if get(g, r.err, &nonmax, xv, yv, zv)? > 0.0 {
                        done = -1;
                    } else {
                        let dot = odx * ddx + ody * ddy + odz * ddz;
                        done = i32::from(dot <= 0.0);
                        if get(g, r.err, &dist_sup, xv, yv, zv)? > 0.0 {
                            done = 1;
                        }
                    }
                    if walked > 10.0 {
                        break;
                    }
                    if done != 0 {
                        break;
                    }
                }
                if done < 0 {
                    set(g, r.err, ctrl, x, y, z, 0.0)?;
                }
            }
        }
    }
    write(g, r, ctrl, "c2.mgz")?;
    Ok(())
}

/// The steps after which a walk of [`remove_surface_outliers_t2`] is taken never to end (one of
/// 10 mm in steps of half a voxel ends within 20 steps per millimetre of voxel size).
const MAX_WALK_STEPS: u64 = 10_000_000;

/// FS-1015: a walk past [`MAX_WALK_STEPS`] (FreeSurfer runs for hours or never ends): both modes
/// end with an error (rule B).
fn walk_never_ends(g: &mut Globals, r: &mut Run, dist: &Mri, v: [i32; 3]) -> Result<(), Fatal> {
    g.notices.add(
        &FS_1015,
        format!(
            "remove_surface_outliers_T2: the walk from control point ({}, {}, {}) has not ended              after {MAX_WALK_STEPS} steps (column size {}): FreeSurfer runs for hours or never ends",
            v[0], v[1], v[2], dist.vg.xsize
        ),
    );
    log::error_exit(
        g,
        r.err,
        1,
        &format!(
            "remove_surface_outliers_T2: the walk from control point ({}, {}, {}) along the              distance gradient does not end (voxel size {})",
            v[0], v[1], v[2], dist.vg.xsize
        ),
    )
}

/// `threshold_control_points(mri_ctrl, mri_intensity, thresh)`: each control point (a value other
/// than 0) whose intensity is below `thresh` removed (at `Gx`, `Gy`, `Gz`: `(x, y, z) - val <v>
/// lower than threshold <t> - removing`).
pub fn threshold_control_points(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctrl: &mut Mri,
    intensity: &Mri,
    thresh: f32,
) -> Result<i32, Fatal> {
    let (w, h, d) = ext(ctrl);
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                if get(g, err, ctrl, x, y, z)? == 0.0 {
                    continue;
                }
                let val = f64::from(get(g, err, intensity, x, y, z)?);
                if val < f64::from(thresh) {
                    if at_g(g, x, y, z) {
                        let _ = writeln!(
                            out,
                            "({x}, {y}, {z}) - val {} lower than threshold {} - removing",
                            ff(val, 2, 0),
                            ff(f64::from(thresh), 2, 0)
                        );
                    }
                    set(g, err, ctrl, x, y, z, 0.0)?;
                }
            }
        }
    }
    Ok(NO_ERROR)
}

/// `remove_controlpoints_outside_range(mri_ctrl, mri, fmin, fmax)`: over `mri`'s extent, each
/// control point (its value converted to `int`, other than 0) where `mri` is below `fmin` or
/// above `fmax` removed and counted (at `Gx`, `Gy`, `Gz`: `voxel(x, y, z) = <v>, removing due to
/// intensity outlier`), then `<n> control points removed in intensity range checking`. A volume
/// larger than the control volume reads and writes it as `MRIgetVoxVal` and `MRIsetVoxVal` do
/// there (FS-52, FS-53).
pub fn remove_controlpoints_outside_range(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctrl: &mut Mri,
    mri: &Mri,
    fmin: f32,
    fmax: f32,
) -> Result<i32, Fatal> {
    const WHAT: &str = "remove_controlpoints_outside_range";
    let mut nremoved = 0;
    let (w, h, d) = ext(mri);
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                if at_g(g, x, y, z) {
                    diag_break();
                }
                let c = get(g, err, ctrl, x, y, z)?;
                if float_to_int(g, f64::from(c), WHAT) == 0 {
                    continue;
                }
                let val = get(g, err, mri, x, y, z)?;
                if val < fmin || val > fmax {
                    if at_g(g, x, y, z) {
                        let _ = writeln!(
                            out,
                            "voxel({x}, {y}, {z}) = {}, removing due to intensity outlier",
                            ff(f64::from(val), 2, 1)
                        );
                    }
                    nremoved += 1;
                    set(g, err, ctrl, x, y, z, CONTROL_NONE as f32)?;
                }
            }
        }
    }
    let _ = writeln!(
        out,
        "{nremoved} control points removed in intensity range checking"
    );
    Ok(NO_ERROR)
}
