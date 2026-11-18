// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A volume resampled through a GCA morph (`utils/gcamorph.cpp` of FreeSurfer 8.2.0; track A's
//! relay 43 of FreeSurfer's foundations): `GCAMmorphToAtlas` (4313: an image volume onto the
//! atlas grid), `GCAMmorphToAtlasWithDensityCorrection` (4432: the same, each voxel times the
//! morph's Jacobian), `gcamCreateJacobianImage` (9900), `GCAMmorphFromAtlas` (3900: an atlas volume
//! back onto the image grid, through the morph's inverse) and `GCAMsampleInverseMorph` (5336). The
//! `TRANSFORM` functions of a morph (`TransformApply`, `TransformApplyType`,
//! `TransformApplyInverse`, `TransformApplyInverseType`, `TransformCreateDensityMap`) go through
//! them (`crate::transform`).
//!
//! FreeSurfer runs `GCAMmorphToAtlas`' loop over the atlas columns with OpenMP; each voxel is
//! computed alone, so the port's single loop gives the same voxels (its messages in the order of
//! one thread, as `OMP_NUM_THREADS=1` prints them). `GCAMmorphFromAtlas` takes the morph's atlas
//! (`gcam->gca`) path only for a morph made with an atlas in memory, which no morph read from a
//! file has: the port has the inverse path only.
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-io.md` Part 39): FS-930 (a voxel
//! whose sample fails takes the previous voxel's value), FS-931 (the first such voxel of a call:
//! memory contents), FS-932 (a point at or below the source's first slice of a 3-D volume gives
//! 0 in `GCAMmorphToAtlas`).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_930, FS_931, FS_932};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::{DIAG_WRITE, verbose_on};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_io::mriio;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, Mri, SAMPLE_CUBIC_BSPLINE, SAMPLE_TRILINEAR, mri_alloc, mri_alloc_sequence,
    use_vol_geom_to_mri,
};
use larmorx_freesurfer_mri::vol_geom::{reinit_cache, shearless_components};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::bspline::{mri_sample_bspline, mri_to_bspline};
use larmorx_freesurfer_volume::coords::index_not_in_volume;
use larmorx_freesurfer_volume::resize::mri_set_resolution;
use larmorx_freesurfer_volume::sample::{sample_volume, sample_volume_frame_type};

use crate::gcam::{Gcam, gcam_compute_metric_properties, gcam_invert};
use crate::transform::gcam_sample_morph;

/// A value no sampler writes (a signalling NaN with low bits a `float`'s NaN never has): a sample
/// that leaves it there wrote nothing.
const UNWRITTEN: u64 = 0x7ff4_dead_beef_0001;

/// `FZERO`: `fabs(f) < FLT_EPSILON`.
fn fzero(x: f32) -> bool {
    x.abs() < f32::EPSILON
}

/// The value a voxel whose sample was written or failed takes, as the official build keeps it in
/// one variable across the loop (`val`): a written sample becomes the new value; a failed one
/// (an interpolation the sampler does not support, or a volume type it does not read) leaves the
/// previous voxel's value under `reproduce` (FS-930; the first of a call: memory contents, FS-931,
/// taken as 0) and 0 by default.
struct Chain {
    val: Option<f64>,
    what: &'static str,
}

impl Chain {
    fn new(what: &'static str) -> Chain {
        Chain { val: None, what }
    }

    /// The value after one sample (`v` as the sampler left it).
    fn take(&mut self, g: &mut Globals, v: f64) -> f64 {
        if v.to_bits() != UNWRITTEN {
            self.val = Some(v);
            return v;
        }
        let what = self.what;
        match self.val {
            Some(prev) => {
                g.notices.add(
                    &FS_930,
                    format!(
                        "{what}: a voxel whose sample failed takes the previous voxel's value in FreeSurfer"
                    ),
                );
                if g.notices.reproduce() { prev } else { 0.0 }
            }
            None => {
                g.notices.add(
                    &FS_931,
                    format!(
                        "{what}: the first voxel whose sample failed takes memory contents in FreeSurfer"
                    ),
                );
                0.0
            }
        }
    }

    /// A value written without a sample (`val = 0.0`).
    fn set(&mut self, v: f64) -> f64 {
        self.val = Some(v);
        v
    }
}

/// `GCAMmorphToAtlas(mri_src, gcam, mri_morphed, frame, sample_type)`: `mri_src` (an image volume)
/// onto the morph's atlas grid. Frames: `frame` alone when it is one of the source's, else all of
/// them. The output: `mri_morphed` (whose extent must be the atlas's: `invalid input MRI size for
/// GCAMmorphToAtlas()`, the program ends), or a new volume of the atlas's extent, the source's type
/// and the frames (one for a `frame` of 0 or more); its geometry the atlas's without shears
/// ([`shearless_components`], `useVolGeomToMRI`). `MGH_TAL` set: `INFO: adding MGH tal offset
/// (-7.4, 24.9, -18.9) to xform` and those offsets added to every sampled point. Each atlas voxel
/// `(x, y, z)` (x slowest): its point in the image ([`gcam_sample_morph`]; a voxel outside the
/// morph's grid, or among invalid nodes, is not written), the offsets added (in `double`, stored
/// as `float`), then for each frame: inside the source (`x > -1`, `y > -1`, `z == 0` for a source
/// of one slice and `z > 0` for more (FS-932), below the extents; `float` comparisons) its sample
/// (`MRIsampleVolumeFrameType` with `sample_type`; FS-930, FS-931 when it fails), else 0, written
/// as a `float`. `USE_AVERAGE305` set: `INFO: Environmental variable USE_AVERAGE305 set` and `INFO:
/// Modifying dst c_(r,a,s), using average_305 values` on standard error and the centre (-0.0950,
/// -16.5100, 9.7500). Cubic B-spline sampling samples the source's cubic B-spline coefficients
/// (`MRItoBSpline` of degree 3, fs.bspline).
#[allow(clippy::too_many_arguments)]
pub fn gcam_morph_to_atlas(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &Mri,
    gcam: &Gcam,
    dst: Option<&mut Mri>,
    frame: i32,
    sample_type: i32,
) -> Result<Ret<Mri>, Fatal> {
    let (start_frame, end_frame) = if frame >= 0 && frame < src.nframes {
        (frame, frame)
    } else {
        (0, src.nframes - 1)
    };
    let (width, height, depth) = (gcam.atlas.width, gcam.atlas.height, gcam.atlas.depth);
    if let Some(d) = &dst
        && (d.vg.width != width || d.vg.height != height || d.vg.depth != depth)
    {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "invalid input MRI size for GCAMmorphToAtlas()",
        )?;
    }
    let mut fresh = None;
    let given = dst.is_some();
    let m: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let nf = if frame < 0 { src.nframes } else { 1 };
            fresh.insert(mri_alloc_sequence(
                g, out, err, width, height, depth, src.mtype, nf,
            )?)
        }
    };
    let mut trg_geom = gcam.atlas.clone();
    shearless_components(g, out, err, &mut trg_geom)?;
    use_vol_geom_to_mri(g, out, err, &trg_geom, &mut m.vg)?;
    let (xoff, yoff, zoff) = if env.get("MGH_TAL").is_some() {
        let o = (-7.42f64, 24.88f64, -18.85f64);
        let _ = writeln!(
            out,
            "INFO: adding MGH tal offset ({}, {}, {}) to xform",
            fmt_f(o.0, 2, 1),
            fmt_f(o.1, 2, 1),
            fmt_f(o.2, 2, 1)
        );
        o
    } else {
        (0.0, 0.0, 0.0)
    };
    let bspline = if sample_type == SAMPLE_CUBIC_BSPLINE {
        Some(mri_to_bspline(g, out, err, src, None, 3)?)
    } else {
        None
    };
    let (sw, sh, sd) = (src.vg.width, src.vg.height, src.vg.depth);
    let mut chain = Chain::new("GCAMmorphToAtlas");
    for x in 0..width.max(0) {
        for y in 0..height.max(0) {
            for z in 0..depth.max(0) {
                let (r, [xd, yd, zd]) =
                    gcam_sample_morph(g, err, gcam, x as f32, y as f32, z as f32)?;
                if r != NO_ERROR {
                    continue;
                }
                // the official build: the float widened, then plus the offset (its NaN kept)
                let xd = sse::add64(f64::from(xd), xoff) as f32;
                let yd = sse::add64(f64::from(yd), yoff) as f32;
                let zd = sse::add64(f64::from(zd), zoff) as f32;
                for f in start_frame..=end_frame {
                    let in_xy = xd > -1.0
                        && yd > -1.0
                        && xd < sw as f32
                        && yd < sh as f32
                        && zd < sd as f32;
                    let z_official = (sd == 1 && zd == 0.0) || (sd > 1 && zd > 0.0);
                    let mut inside = in_xy && z_official;
                    if in_xy && !z_official && sd > 1 && zd > -1.0 {
                        g.notices.add(
                            &FS_932,
                            "GCAMmorphToAtlas: a point at or below the source's first slice (z between -1 and 0) gives 0 in FreeSurfer",
                        );
                        if !g.notices.reproduce() {
                            inside = true;
                        }
                    }
                    let val = if inside {
                        let mut v = f64::from_bits(UNWRITTEN);
                        let (px, py, pz) = (f64::from(xd), f64::from(yd), f64::from(zd));
                        match &bspline {
                            Some(b) => {
                                mri_sample_bspline(g, out, err, b, px, py, pz, f, &mut v)?;
                            }
                            None => {
                                sample_volume_frame_type(
                                    g,
                                    err,
                                    src,
                                    px,
                                    py,
                                    pz,
                                    f,
                                    sample_type,
                                    &mut v,
                                );
                            }
                        }
                        chain.take(g, v)
                    } else {
                        chain.set(0.0)
                    };
                    mri_set_vox_val(g, err, m, x, y, z, f - start_frame, val as f32, 0)?;
                }
            }
        }
    }
    if env.get("USE_AVERAGE305").is_some() {
        let _ = writeln!(err, "INFO: Environmental variable USE_AVERAGE305 set");
        let _ = writeln!(
            err,
            "INFO: Modifying dst c_(r,a,s), using average_305 values"
        );
        m.vg.c_r = -0.0950f64 as f32;
        m.vg.c_a = -16.5100f64 as f32;
        m.vg.c_s = 9.7500f64 as f32;
        m.vg.ras_good_flag = 1;
        reinit_cache(g, out, err, &mut m.vg)?;
    }
    Ok(match fresh {
        Some(v) => Ret::New(v),
        None if given => Ret::Given,
        None => Ret::Null,
    })
}

/// `gcamCreateJacobianImage(gcam)`: the morph's metric properties computed
/// ([`gcam_compute_metric_properties`]), then a float volume of the grid's extent with voxel sizes
/// the spacing (`MRIsetResolution`) holding each node's Jacobian: `orig_area / area` (`float`), or
/// 1 where both areas are zero (`FZERO`), 0.001 where only `area` is. The node at
/// `(Gx, Gy, Gz)` prints `jacobian(<x>, <y>, <z>) = <%2.3f>`.
pub fn gcam_create_jacobian_image(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    gcam: &mut Gcam,
) -> Result<Mri, Fatal> {
    gcam_compute_metric_properties(g, out, err, env, gcam)?;
    let mut m = mri_alloc(g, out, err, gcam.width, gcam.height, gcam.depth, MRI_FLOAT)?;
    let sp = gcam.spacing as f32;
    mri_set_resolution(g, out, err, &mut m, sp, sp, sp)?;
    for x in 0..gcam.width.max(0) {
        for y in 0..gcam.height.max(0) {
            for z in 0..gcam.depth.max(0) {
                let n = gcam.node(x, y, z);
                let jacobian = if !fzero(n.area) {
                    sse::div(n.orig_area, n.area)
                } else if fzero(n.orig_area) {
                    1.0
                } else {
                    0.001f64 as f32
                };
                if x == g.gx && y == g.gy && z == g.gz {
                    let _ = writeln!(
                        out,
                        "jacobian({x}, {y}, {z}) = {}",
                        fmt_f(f64::from(jacobian), 2, 3)
                    );
                }
                mri_set_vox_val(g, err, &mut m, x, y, z, 0, jacobian, 0)?;
            }
        }
    }
    Ok(m)
}

/// `GCAMmorphToAtlasWithDensityCorrection(mri_src, gcam, mri_morphed, frame)`:
/// [`gcam_morph_to_atlas`] with trilinear sampling, then each voxel of the frames (as there)
/// times the morph's Jacobian ([`gcam_create_jacobian_image`]) sampled trilinearly at the voxel
/// divided by the spacing (`float`); voxels whose point is outside the Jacobian's grid
/// (`MRIindexNotInVolume`) keep their value. `Gdiag & DIAG_WRITE` with `DIAG_VERBOSE` writes the
/// Jacobian as `jac.mgz`.
#[allow(clippy::too_many_arguments)]
pub fn gcam_morph_to_atlas_with_density_correction(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    src: &Mri,
    gcam: &mut Gcam,
    dst: Option<&mut Mri>,
    frame: i32,
) -> Result<Ret<Mri>, Fatal> {
    let given = dst.is_some();
    let mut dst = dst;
    let r = gcam_morph_to_atlas(
        g,
        out,
        err,
        env,
        src,
        gcam,
        dst.as_deref_mut(),
        frame,
        SAMPLE_TRILINEAR,
    )?;
    let mut fresh = match r {
        Ret::New(m) => Some(m),
        Ret::Given => None,
        Ret::Null => return Ok(Ret::Null),
    };
    let m: &mut Mri = match (&mut fresh, dst) {
        (Some(m), _) => m,
        (None, Some(d)) => d,
        (None, None) => return Ok(Ret::Null),
    };
    let (width, height, depth) = (m.vg.width, m.vg.height, m.vg.depth);
    let (start_frame, end_frame) = if frame >= 0 && frame < src.nframes {
        (frame, frame)
    } else {
        (0, src.nframes - 1)
    };
    let mut jac = gcam_create_jacobian_image(g, out, err, env, gcam)?;
    if verbose_on(g) && g.gdiag & DIAG_WRITE != 0 {
        mriio::mri_write(g, out, err, env, dir, &mut jac, "jac.mgz", None, -1)?;
    }
    let sp = gcam.spacing as f32;
    for x in 0..width.max(0) {
        for y in 0..height.max(0) {
            for z in 0..depth.max(0) {
                let xd = sse::div(x as f32, sp);
                let yd = sse::div(y as f32, sp);
                let zd = sse::div(z as f32, sp);
                if index_not_in_volume(&jac, f64::from(xd), f64::from(yd), f64::from(zd)) != 0 {
                    continue;
                }
                let mut jacobian = 0.0f64;
                if sample_volume(
                    g,
                    err,
                    &jac,
                    f64::from(xd),
                    f64::from(yd),
                    f64::from(zd),
                    &mut jacobian,
                ) == NO_ERROR
                {
                    for f in start_frame..=end_frame {
                        let mut val = f64::from(mri_get_vox_val(g, err, m, x, y, z, f, 0)?);
                        val = sse::mul64(val, jacobian);
                        mri_set_vox_val(g, err, m, x, y, z, f - start_frame, val as f32, 0)?;
                    }
                }
            }
        }
    }
    Ok(match fresh {
        Some(v) => Ret::New(v),
        None if given => Ret::Given,
        None => Ret::Null,
    })
}

/// `GCAMsampleInverseMorph(gcam, c, r, s, ...)`: an image voxel to the atlas through the morph's
/// inverse: each index volume sampled trilinearly (`MRIsampleVolume`) at the point, times the
/// spacing (`double`, stored as `float`); 1 after `ERROR: GCAMsampleInverseMorph(): GCAM not
/// inverted` (standard output) when the morph has no inverse, or when a sample fails (the
/// coordinates found before it are kept); else 0.
pub fn gcam_sample_inverse_morph(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    gcam: &Gcam,
    c: f32,
    r: f32,
    s: f32,
) -> (i32, [f32; 3]) {
    let mut p = [0f32; 3];
    let (Some(xi), Some(yi), Some(zi)) = (&gcam.mri_xind, &gcam.mri_yind, &gcam.mri_zind) else {
        let _ = out.write_all(b"ERROR: GCAMsampleInverseMorph(): GCAM not inverted\n");
        return (1, p);
    };
    let sp = f64::from(gcam.spacing);
    for (k, ind) in [xi, yi, zi].into_iter().enumerate() {
        let mut v = 0.0f64;
        if sample_volume(
            g,
            err,
            ind,
            f64::from(c),
            f64::from(r),
            f64::from(s),
            &mut v,
        ) != NO_ERROR
        {
            return (1, p);
        }
        p[k] = sse::mul64(v, sp) as f32;
    }
    (0, p)
}

/// `GCAMmorphFromAtlas(mri_in, gcam, mri_morphed, sample_type)`: an atlas volume back onto the
/// morph's image grid. The output: `mri_morphed`, or a new volume of the image's extent with the
/// source's type and frames; its geometry the image's (`useVolGeomToMRI`). The morph's inverse is
/// made on it when it has none ([`gcam_invert`]). Each output voxel `(x, y, z)` (x slowest): its
/// point in the atlas ([`gcam_sample_inverse_morph`]: a voxel whose point cannot be found is not
/// written), then for each frame its sample (`MRIsampleVolumeFrameType` with `sample_type`;
/// FS-930, FS-931 when it fails), written as a `float`.
#[allow(clippy::too_many_arguments)]
pub fn gcam_morph_from_atlas(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    src: &Mri,
    gcam: &mut Gcam,
    dst: Option<&mut Mri>,
    sample_type: i32,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let given = dst.is_some();
    let m: &mut Mri = match dst {
        Some(d) => d,
        None => fresh.insert(mri_alloc_sequence(
            g,
            out,
            err,
            gcam.image.width,
            gcam.image.height,
            gcam.image.depth,
            src.mtype,
            src.nframes,
        )?),
    };
    let image = gcam.image.clone();
    use_vol_geom_to_mri(g, out, err, &image, &mut m.vg)?;
    gcam_invert(g, out, err, env, dir, gcam, Some(&*m))?;
    let mut chain = Chain::new("GCAMmorphFromAtlas");
    for x in 0..m.vg.width.max(0) {
        for y in 0..m.vg.height.max(0) {
            for z in 0..m.vg.depth.max(0) {
                let (r, [xf, yf, zf]) =
                    gcam_sample_inverse_morph(g, out, err, gcam, x as f32, y as f32, z as f32);
                if r != NO_ERROR {
                    continue;
                }
                let (xr, yr, zr) = (f64::from(xf), f64::from(yf), f64::from(zf));
                for f in 0..m.nframes {
                    let mut v = f64::from_bits(UNWRITTEN);
                    sample_volume_frame_type(g, err, src, xr, yr, zr, f, sample_type, &mut v);
                    let val = chain.take(g, v);
                    mri_set_vox_val(g, err, m, x, y, z, f, val as f32, 0)?;
                }
            }
        }
    }
    Ok(match fresh {
        Some(v) => Ret::New(v),
        None if given => Ret::Given,
        None => Ret::Null,
    })
}
