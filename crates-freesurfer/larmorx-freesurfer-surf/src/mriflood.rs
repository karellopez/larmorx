// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mriflood.cpp`'s surface functions (queue item 3.1-43): `MRISribbon` (97), `MRISshell`
//! (154), `MRISmakeBoundingVolume` (1421) and `MRISfillInterior` (1495). The file's volume
//! functions (`MRISfloodoutside`, `MRISaccentuate`, `distance`) are larmorx-freesurfer-filter's
//! `flood`; these four are here, beside the surface, because they draw it into volumes (this
//! crate builds on the filter crate, never the other way).
//!
//! The arithmetic follows the official build's object code: `MRISshell` in `float` (`sqrtf`,
//! `ceilf`, each sample `p0 + ((p1 - p0) u) / n`), `MRISfillInterior` in `double`; a sample's
//! voxel is `nint` of its coordinates, and a coordinate that is NaN or rounds beyond `int` is
//! outside every volume (x86-64's `INT_MIN`, as FreeSurfer's build gives; no conversion notice).
//!
//! D13 (larmorx_core::defects): FS-606 (a face whose first two corners coincide draws nothing,
//! kept), FS-607 (a bounding volume beyond `int`), FS-608 (an interior in a volume the surface
//! does not cross), and through the surface-to-voxel maps FS-604.

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_606, FS_607, FS_608};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{matrix_identity, matrix_inverse};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::timer::Timer;
use larmorx_freesurfer_filter::flood::{mris_accentuate, mris_flood_outside};
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_UCHAR, Mri, mri_alloc, mri_alloc_from_vol_geom, mri_copy_header,
};
use larmorx_freesurfer_mri::vol_geom::xfm_crs2xyz;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{Pair, Tri, mri_clear};
use larmorx_freesurfer_volume::frames::mri_const;
use larmorx_freesurfer_volume::mriset::{mri_complement, mri_intersect};
use larmorx_freesurfer_volume::orient::mri_set_voxel_to_ras_xform;
use larmorx_freesurfer_volume::region::{region_get_bounding_box, region_print};
use larmorx_freesurfer_volume::resize::mri_set_resolution;

use crate::aaa::SurfGlobals;
use crate::coords::Io;
use crate::facenorm::get_face_norm;
use crate::mp::compute_metric_properties;
use crate::mris::Mris;
use crate::sras::{
    mris_load_ras2voxel_map, mris_make_ras2voxel_map, mris_surface_ras_to_voxel,
    mris_use_ras2voxel_map,
};

/// FreeSurfer's `nint(f)` as its x86-64 build computes it, `(int)(f - 0.5)` below 0 and `(int)(f
/// + 0.5)` otherwise, with `INT_MIN` for NaN and beyond `int` (outside every volume).
fn vox(f: f64) -> i32 {
    let x = if f < 0.0 { f - 0.5 } else { f + 0.5 };
    if x.is_nan() || x >= 2_147_483_648.0 || x <= -2_147_483_649.0 {
        i32::MIN
    } else {
        x.trunc() as i32
    }
}

/// `(int) c` of a `ceil` result in `float` (`cvttss2si`): `INT_MIN` for NaN and beyond `int`.
fn count32(c: f32) -> i32 {
    if c.is_nan() || c >= 2_147_483_648.0 || c < -2_147_483_648.0 {
        i32::MIN
    } else {
        c as i32
    }
}

/// `(int) c` of a `ceil` result in `double` (`cvttsd2si`): `INT_MIN` for NaN and beyond `int`.
fn count64(c: f64) -> i32 {
    if c.is_nan() || c >= 2_147_483_648.0 || c <= -2_147_483_649.0 {
        i32::MIN
    } else {
        c.trunc() as i32
    }
}

fn inside(m: &Mri, i: i32, j: i32, k: i32) -> bool {
    i >= 0 && i < m.vg.width && j >= 0 && j < m.vg.height && k >= 0 && k < m.vg.depth
}

/// The three corners of face `fno` as `float` coordinates.
fn corners(mris: &Mris, fno: usize) -> [[f32; 3]; 3] {
    let f = mris.face(fno);
    let p = |k: usize| {
        let v = mris.vertex(f.v[k] as usize);
        [v.x, v.y, v.z]
    };
    [p(0), p(1), p(2)]
}

/// The FS-606 notice of a face whose first two corners coincide.
fn degenerate_face(g: &mut Globals, what: &str, fno: usize) {
    g.notices.add(
        &FS_606,
        format!(
            "{what}: face {fno}'s first two corners are at one point: FreeSurfer draws nothing for it (kept)"
        ),
    );
}

/// `MRISshell(mri_src, mris, mri_dst, clearflag)`: the surface drawn into a volume of the source's
/// geometry as voxels of 255. Without a destination, `MRIalloc` of the source's extent and type
/// with its header (`MRIcopyHeader`); `clearflag` clears it first (`MRIclear`). Each face: its
/// side lengths `d0 = |v1 - v0| / xsize`, `d1 = |v2 - v1| / ysize`, `d2 = |v0 - v2| / zsize` (the
/// destination's voxel sizes; `float`), the longest `dmax` (`d0 >= d1 && d0 >= d2 ? d0 : d1 >= d0
/// && d1 >= d2 ? d1 : d2`), `numu = (int) ceilf(2 d0)` and `numv = (int) ceilf(2 dmax)`; for each
/// `v` in `0..=numv` the points `p0 = v0 + ((v2 - v0) v) / numv` and `p1 = v1 + ((v2 - v1) v) /
/// numv`, and on the line between them, for each `u` in `0..=numu`, `p = p0 + ((p1 - p0) u) /
/// numu` (all `float`), converted to the source's voxels by the cached conversion
/// ([`crate::sras::mris_surface_ras_to_voxel_cached`], FS-604) and rounded (`nint`); a voxel inside the
/// destination is set to 255. FS-606: a face whose first two corners coincide (`numu` 0) gives
/// only NaN samples and draws nothing (kept, noted).
#[allow(clippy::too_many_arguments)]
pub fn mris_shell(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mri_src: &Mri,
    mris: &Mris,
    mri_dst: Option<&mut Mri>,
    clearflag: i32,
) -> Result<Ret<Mri>, Fatal> {
    let (width, height, depth) = (mri_src.vg.width, mri_src.vg.height, mri_src.vg.depth);
    let mut fresh = None;
    let given = mri_dst.is_some();
    let dst: &mut Mri = match mri_dst {
        Some(d) => d,
        None => {
            let mut d = mri_alloc(g, io.out, io.err, width, height, depth, mri_src.mtype)?;
            mri_copy_header(g, io.out, io.err, mri_src, Some(&mut d))?;
            fresh.insert(d)
        }
    };
    if clearflag != 0 {
        mri_clear(g, io.err, dst)?;
    }
    let (xs, ys, zs) = (dst.vg.xsize, dst.vg.ysize, dst.vg.zsize);
    let mut loaded = false;
    for fno in 0..usize::try_from(mris.nfaces).unwrap_or(0) {
        let [[x0, y0, z0], [x1, y1, z1], [x2, y2, z2]] = corners(mris, fno);
        let d0 =
            ((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0) + (z1 - z0) * (z1 - z0)).sqrt() / xs;
        let d1 =
            ((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1) + (z2 - z1) * (z2 - z1)).sqrt() / ys;
        let d2 =
            ((x0 - x2) * (x0 - x2) + (y0 - y2) * (y0 - y2) + (z0 - z2) * (z0 - z2)).sqrt() / zs;
        let dmax = if d0 >= d1 && d0 >= d2 {
            d0
        } else if d1 >= d0 && d1 >= d2 {
            d1
        } else {
            d2
        };
        let numu = count32((2.0 * d0).ceil());
        let numv = count32((2.0 * dmax).ceil());
        if numv < 0 || numu < 0 {
            continue;
        }
        if numu == 0 {
            degenerate_face(g, "MRISshell", fno);
        }
        let (fnu, fnv) = (numu as f32, numv as f32);
        let (ex0, ey0, ez0) = (x2 - x0, y2 - y0, z2 - z0);
        let (ex1, ey1, ez1) = (x2 - x1, y2 - y1, z2 - z1);
        for v in 0..=numv {
            let fv = v as f32;
            let (px0, py0, pz0) = (
                x0 + ex0 * fv / fnv,
                y0 + ey0 * fv / fnv,
                z0 + ez0 * fv / fnv,
            );
            let (dx, dy, dz) = (
                (x1 + ex1 * fv / fnv) - px0,
                (y1 + ey1 * fv / fnv) - py0,
                (z1 + ez1 * fv / fnv) - pz0,
            );
            for u in 0..=numu {
                let fu = u as f32;
                let (px, py, pz) = (
                    px0 + dx * fu / fnu,
                    py0 + dy * fu / fnu,
                    pz0 + dz * fu / fnu,
                );
                // the cached conversion (`MRISsurfaceRASToVoxelCached`): its map loaded at the
                // first sample; the later loads of this call find the same volume and surface
                // and change nothing, so the map is used directly (the same bits, faster)
                if !loaded {
                    mris_load_ras2voxel_map(g, io, &mut sg.sras_map, mri_src, mris)?;
                    loaded = true;
                }
                let [fi, fj, fk] = mris_use_ras2voxel_map(
                    g,
                    io,
                    &mut sg.sras_map,
                    f64::from(px),
                    f64::from(py),
                    f64::from(pz),
                )?;
                let (i, j, k) = (vox(fi), vox(fj), vox(fk));
                if inside(dst, i, j, k) {
                    mri_set_vox_val(g, io.err, dst, i, j, k, 0, 255.0, 0)?;
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) if !given => Ret::New(m),
        _ => Ret::Given,
    })
}

/// A new volume of the source's extent and type (`MRIalloc`) with its header (`MRIcopyHeader`).
fn like(g: &mut Globals, io: &mut Io, src: &Mri) -> Result<Mri, Fatal> {
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    let mut m = mri_alloc(g, io.out, io.err, w, h, d, src.mtype)?;
    mri_copy_header(g, io.out, io.err, src, Some(&mut m))?;
    Ok(m)
}

/// `MRISribbon(inner_mris, outer_mris, mri_src, mri_dst)`: the voxels between two surfaces as 255
/// (else 0). The outer surface's shell ([`mris_shell`] into a volume like the source), flooded
/// from outside (`MRISfloodoutside`), its values 1 to 254 accentuated (`MRISaccentuate`: the
/// flooded outside, 255 elsewhere) and complemented (`MRIcomplement`): 1 inside the outer surface.
/// The inner surface's shell in the destination (a volume like the source when none is given),
/// flooded from outside: 1 outside the inner surface, 255 on it. Their intersection
/// (`MRIintersect`, the smaller value) accentuated from 1 to 255. With `Gdiag`'s show and verbose
/// bits the steps are printed on standard output.
pub fn mris_ribbon(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    inner: &Mris,
    outer: &Mris,
    mri_src: &Mri,
    mri_dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let verbose = g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0;
    let (w, h, d) = (mri_src.vg.width, mri_src.vg.height, mri_src.vg.depth);
    if verbose {
        let _ = writeln!(
            io.out,
            "MRISribbon: Creating new (_inter)MRI of {w}, {h},{d}..."
        );
    }
    let mut inter = like(g, io, mri_src)?;
    let mut fresh = None;
    let given = mri_dst.is_some();
    let dst: &mut Mri = match mri_dst {
        Some(m) => m,
        None => {
            if verbose {
                let _ = writeln!(io.out, "MRISribbon: Creating new (_dst)MRI...");
            }
            fresh.insert(like(g, io, mri_src)?)
        }
    };
    if verbose {
        let _ = writeln!(io.out, "Creating volume inside outer shell...");
    }
    mris_shell(g, sg, io, mri_src, outer, Some(&mut inter), 1)?;
    mris_flood_outside(g, io.out, io.err, Pair::Same(&mut inter))?;
    mris_accentuate(g, io.out, io.err, Pair::Same(&mut inter), 1, 254)?;
    mri_complement(g, io.out, io.err, Pair::Same(&mut inter))?;
    if verbose {
        let _ = writeln!(io.out, "Creating volume outside inner shell...");
    }
    mris_shell(g, sg, io, mri_src, inner, Some(&mut *dst), 1)?;
    mris_flood_outside(g, io.out, io.err, Pair::Same(&mut *dst))?;
    if verbose {
        let _ = writeln!(io.out, "Finding intersection of volumes...");
    }
    mri_intersect(g, io.out, io.err, Tri::Second(&inter, &mut *dst))?;
    mris_accentuate(g, io.out, io.err, Pair::Same(&mut *dst), 1, 255)?;
    if verbose {
        let _ = writeln!(io.out, "Done with intersection...");
    }
    Ok(match fresh {
        Some(m) if !given => Ret::New(m),
        _ => Ret::Given,
    })
}

/// `(int) floor(x)` or `(int) ceil(x)` of a `double` (FS-17 beyond `int`).
fn to_int(g: &mut Globals, x: f64) -> i32 {
    float_to_int(g, x, "MRISmakeBoundingVolume")
}

/// `MRISmakeBoundingVolume(mris, resolution)`: a `float` volume that holds the surface at
/// `resolution` mm. The surface's volume geometry as a header (`MRIallocFromVolGeom`, `uchar`); its
/// bounding box's two corners (`xlo ylo zlo`, `xhi yhi zhi`) as voxels of it
/// ([`mris_surface_ras_to_voxel`]); per axis the voxel range `lo = (int) floor(min)`, `hi = (int)
/// ceil(max)` (`std::min`/`std::max`: the first corner's value when they do not compare), `n = hi -
/// lo + 1`; the extent `(int) ceil((float) n * size / resolution)` (the product in `float`, the
/// quotient in `double`). The new volume (`MRIalloc`) takes the geometry's direction cosines, the
/// voxel size `resolution` (as `float`) on every axis and as its centre the scanner RAS of the voxel
/// `(lo + n / 2.0)` per axis (stored as `float`s, through the geometry's vox2ras in `float`). Its
/// cached matrices are not made again (as in FreeSurfer). FS-607: an extent `hi - lo + 1` beyond
/// `int` (a surface beyond 2^31 voxels, or not finite) wraps in FreeSurfer; by default an error
/// (rule B), noted.
pub fn mris_make_bounding_volume(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    resolution: f64,
) -> Result<Mri, Fatal> {
    let src = mri_alloc_from_vol_geom(g, io.out, io.err, &mris.vg, MRI_UCHAR, 1, true)?;
    let lo = mris_surface_ras_to_voxel(
        g,
        io,
        mris,
        &src,
        f64::from(mris.xlo),
        f64::from(mris.ylo),
        f64::from(mris.zlo),
    )?;
    let hi = mris_surface_ras_to_voxel(
        g,
        io,
        mris,
        &src,
        f64::from(mris.xhi),
        f64::from(mris.yhi),
        f64::from(mris.zhi),
    )?;
    let mut real_lo = [0i32; 3];
    let mut real_hi = [0i32; 3];
    for k in 0..3 {
        let (a, b) = (lo[k], hi[k]);
        let mn = if b < a { b } else { a };
        real_lo[k] = to_int(g, mn.floor());
    }
    for k in 0..3 {
        let (a, b) = (lo[k], hi[k]);
        let mx = if a < b { b } else { a };
        real_hi[k] = to_int(g, mx.ceil());
    }
    let mut n = [0i32; 3];
    for k in 0..3 {
        let wide = i64::from(real_hi[k]) - i64::from(real_lo[k]) + 1;
        n[k] = match i32::try_from(wide) {
            Ok(x) => x,
            Err(_) => {
                g.notices.add(
                    &FS_607,
                    format!(
                        "MRISmakeBoundingVolume: an extent of {wide} voxels (from {} to {}), beyond int",
                        real_lo[k], real_hi[k]
                    ),
                );
                if !g.notices.reproduce() {
                    log::error_exit(
                        g,
                        io.err,
                        ERROR_BADPARM,
                        &format!(
                            "MRISmakeBoundingVolume: the surface spans {wide} voxels of its volume geometry, more than an int holds\n"
                        ),
                    )?;
                }
                wide as i32
            }
        };
    }
    let size = [src.vg.xsize, src.vg.ysize, src.vg.zsize];
    let mut ext = [0i32; 3];
    for k in 0..3 {
        let c = (f64::from(n[k] as f32 * size[k]) / resolution).ceil();
        ext[k] = to_int(g, c);
    }
    let mut dst = mri_alloc(g, io.out, io.err, ext[0], ext[1], ext[2], MRI_FLOAT)?;
    let (s, t) = (&src.vg, &mut dst.vg);
    (t.x_r, t.x_a, t.x_s) = (s.x_r, s.x_a, s.x_s);
    (t.y_r, t.y_a, t.y_s) = (s.y_r, s.y_a, s.y_s);
    (t.z_r, t.z_a, t.z_s) = (s.z_r, s.z_a, s.z_s);
    let r = resolution as f32;
    (t.xsize, t.ysize, t.zsize) = (r, r, r);
    let vox2ras = xfm_crs2xyz(g, io.out, io.err, &src.vg, 0, false)?;
    let mut c_crs = matrix_alloc(g, io.out, io.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    for k in 0..3 {
        let c = f64::from(real_lo[k]) + f64::from(n[k]) * 0.5;
        c_crs.set_relt(k as i32 + 1, 1, c as f32);
    }
    c_crs.set_relt(4, 1, 1.0);
    let c_ras = matrix_multiply(g, io.out, io.err, Some(&vox2ras), Some(&c_crs), None)?
        .into_new()
        .expect("a 4 x 4 matrix times a 4 x 1 vector");
    (dst.vg.c_r, dst.vg.c_a, dst.vg.c_s) = (c_ras.relt(1, 1), c_ras.relt(2, 1), c_ras.relt(3, 1));
    Ok(dst)
}

/// `MRISfillInterior(mris, resolution, mri_dst)`: 1 for the voxels inside the surface, 0
/// elsewhere ("much faster but slightly less accurate than ray tracing"). First
/// `MRIScomputeMetricProperties`. Without a destination, a `float` volume of `(int) ceil((hi -
/// lo) / resolution)` voxels per axis (the difference in `float`, the quotient in `double`; FS-17
/// beyond `int`) with the voxel size `resolution`, the vox2ras `resolution` on the diagonal and
/// `lo + c` (the bounding box's low corner plus the geometry's centre, `float`) as its
/// translation (`MRIsetVoxelToRasXform`). Then the destination is cleared (`MRIclear`), and a
/// shell of the surface drawn into an `int` volume of its extent and header as in [`mris_shell`]
/// but in `double` with the destination's voxel sizes per axis (`d0 = |(v1 - v0) / size|`, ...),
/// through the destination's surface-to-voxel map ([`mris_make_ras2voxel_map`], FS-604); for each
/// sample inside, the voxel set to 255 and, when the distance from the sample to the voxel's
/// centre in surface RAS (the map's inverse in `float` times the voxel) is not above the smallest
/// so far (10 at first), that distance kept with the cosine between the unit vector from the
/// centre to the sample and the face's normal (`getFaceNorm`; `float` volumes of its header).
/// Then the shell is cut to its bounding box with 2 voxels' padding (`REGIONgetBoundingBox`;
/// with `Gdiag_no > 0` printed after `  Bounding box around shell: `) as `uchar`, flooded from
/// outside (`MRISfloodoutside`), and each voxel of the box that is not (outside and off the
/// shell) and whose cosine is at least 0 set to 1. With `Gdiag_no > 0` the time is printed (`
/// MRISfillInterior t = <seconds>`). FS-606 as in [`mris_shell`]. FS-608: a surface whose shell
/// has no voxel inside the volume gives a box of negative size, whose allocation ends FreeSurfer's
/// program (`MRI::Shape::Shape(): volume size must be greater than 0 in every dimension`, `could
/// not allocate memory for -999996 slices`); by default the program ends with `MRISfillInterior:
/// the surface does not cross the volume (no voxel of its shell is inside it)` (rule B, status 1),
/// noted.
pub fn mris_fill_interior(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    resolution: f64,
    mri_dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let start = Timer::new();
    compute_metric_properties(g, sg, io, mris)?;
    let mut fresh = None;
    let given = mri_dst.is_some();
    let dst: &mut Mri = match mri_dst {
        Some(m) => m,
        None => {
            let wd = (f64::from(mris.xhi - mris.xlo) / resolution).ceil();
            let ht = (f64::from(mris.yhi - mris.ylo) / resolution).ceil();
            let dp = (f64::from(mris.zhi - mris.zlo) / resolution).ceil();
            let (w, h, d) = (
                float_to_int(g, wd, "MRISfillInterior"),
                float_to_int(g, ht, "MRISfillInterior"),
                float_to_int(g, dp, "MRISfillInterior"),
            );
            let mut m = mri_alloc(g, io.out, io.err, w, h, d, MRI_FLOAT)?;
            let r = resolution as f32;
            mri_set_resolution(g, io.out, io.err, &mut m, r, r, r)?;
            let mut v2r = matrix_identity(g, io.out, io.err, 4, None)?.unwrap_new();
            v2r.set_relt(1, 1, r);
            v2r.set_relt(2, 2, r);
            v2r.set_relt(3, 3, r);
            v2r.set_relt(1, 4, mris.xlo + mris.vg.c_r);
            v2r.set_relt(2, 4, mris.ylo + mris.vg.c_a);
            v2r.set_relt(3, 4, mris.zlo + mris.vg.c_s);
            mri_set_voxel_to_ras_xform(g, io.out, io.err, io.env, &mut m, &v2r)?;
            fresh.insert(m)
        }
    };
    mri_clear(g, io.err, dst)?;
    let (dcol, drow, dslc) = (
        f64::from(dst.vg.xsize),
        f64::from(dst.vg.ysize),
        f64::from(dst.vg.zsize),
    );
    let mut crs = matrix_alloc(g, io.out, io.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    crs.set_relt(4, 1, 1.0);
    let dims = [dst.vg.width, dst.vg.height, dst.vg.depth, 1];
    let mut mri_cosa = mri_const(g, io.out, io.err, dims, 0.0, None)?.unwrap_new();
    mri_copy_header(g, io.out, io.err, dst, Some(&mut mri_cosa))?;
    let mut mri_vlen = mri_const(g, io.out, io.err, dims, 10.0, None)?.unwrap_new();
    mri_copy_header(g, io.out, io.err, dst, Some(&mut mri_vlen))?;
    let mut mri_shell = mri_alloc(g, io.out, io.err, dims[0], dims[1], dims[2], MRI_INT)?;
    mri_copy_header(g, io.out, io.err, dst, Some(&mut mri_shell))?;
    let mut map = mris_make_ras2voxel_map(g, io, dst, mris)?;
    mris_load_ras2voxel_map(g, io, &mut map, dst, mris)?;
    let mut vox2sras: Option<Matrix> = None;
    let mut xyz: Option<Matrix> = None;
    for fno in 0..usize::try_from(mris.nfaces).unwrap_or(0) {
        let fnorm = get_face_norm(mris, fno);
        let c = corners(mris, fno);
        let p = |k: usize| [f64::from(c[k][0]), f64::from(c[k][1]), f64::from(c[k][2])];
        let ([x0, y0, z0], [x1, y1, z1], [x2, y2, z2]) = (p(0), p(1), p(2));
        let sq = |a: f64| a * a;
        let d0 = (sq((x1 - x0) / dcol) + sq((y1 - y0) / drow) + sq((z1 - z0) / dslc)).sqrt();
        let d1 = (sq((x2 - x1) / dcol) + sq((y2 - y1) / drow) + sq((z2 - z1) / dslc)).sqrt();
        let d2 = (sq((x0 - x2) / dcol) + sq((y0 - y2) / drow) + sq((z0 - z2) / dslc)).sqrt();
        let dmax = if d0 >= d1 && d0 >= d2 {
            d0
        } else if d1 >= d0 && d1 >= d2 {
            d1
        } else {
            d2
        };
        let numv = count64((2.0 * dmax).ceil());
        let numu = count64((2.0 * d0).ceil());
        if numv < 0 || numu < 0 {
            continue;
        }
        if numu == 0 {
            degenerate_face(g, "MRISfillInterior", fno);
        }
        let (fnu, fnv) = (f64::from(numu), f64::from(numv));
        let (ex0, ey0, ez0) = (x2 - x0, y2 - y0, z2 - z0);
        let (ex1, ey1, ez1) = (x2 - x1, y2 - y1, z2 - z1);
        for v in 0..=numv {
            let fv = f64::from(v);
            let (px0, py0, pz0) = (
                x0 + ex0 * fv / fnv,
                y0 + ey0 * fv / fnv,
                z0 + ez0 * fv / fnv,
            );
            let (dx, dy, dz) = (
                (x1 + ex1 * fv / fnv) - px0,
                (y1 + ey1 * fv / fnv) - py0,
                (z1 + ez1 * fv / fnv) - pz0,
            );
            for u in 0..=numu {
                let fu = f64::from(u);
                let (px, py, pz) = (
                    px0 + dx * fu / fnu,
                    py0 + dy * fu / fnu,
                    pz0 + dz * fu / fnu,
                );
                let [fcol, frow, fslc] = mris_use_ras2voxel_map(g, io, &mut map, px, py, pz)?;
                if vox2sras.is_none() {
                    vox2sras =
                        matrix_inverse(g, io.out, io.err, map.sras2vox.as_ref(), None)?.into_new();
                }
                let (col, row, slc) = (vox(fcol), vox(frow), vox(fslc));
                if !inside(dst, col, row, slc) {
                    continue;
                }
                mri_set_vox_val(g, io.err, &mut mri_shell, col, row, slc, 0, 255.0, 0)?;
                crs.set_relt(1, 1, col as f32);
                crs.set_relt(2, 1, row as f32);
                crs.set_relt(3, 1, slc as f32);
                match xyz.as_mut() {
                    Some(m) => {
                        matrix_multiply(g, io.out, io.err, vox2sras.as_ref(), Some(&crs), Some(m))?;
                    }
                    None => {
                        xyz = matrix_multiply(
                            g,
                            io.out,
                            io.err,
                            vox2sras.as_ref(),
                            Some(&crs),
                            None,
                        )?
                        .into_new();
                    }
                }
                let m = xyz.as_ref().expect("a 4 x 4 matrix times a 4 x 1 vector");
                let (vx, vy, vz) = (
                    f64::from(m.relt(1, 1)),
                    f64::from(m.relt(2, 1)),
                    f64::from(m.relt(3, 1)),
                );
                let vlen = (sq(px - vx) + sq(py - vy) + sq(pz - vz)).sqrt();
                let held = mri_get_vox_val(g, io.err, &mri_vlen, col, row, slc, 0, 0)?;
                if vlen > f64::from(held) {
                    continue;
                }
                let (ux, uy, uz) = ((px - vx) / vlen, (py - vy) / vlen, (pz - vz) / vlen);
                let cosa =
                    ux * f64::from(fnorm.nx) + uy * f64::from(fnorm.ny) + uz * f64::from(fnorm.nz);
                mri_set_vox_val(g, io.err, &mut mri_cosa, col, row, slc, 0, cosa as f32, 0)?;
                mri_set_vox_val(g, io.err, &mut mri_vlen, col, row, slc, 0, vlen as f32, 0)?;
            }
        }
    }
    drop(map);
    drop(mri_vlen);
    let region = region_get_bounding_box(g, io.err, &mri_shell, 2)?;
    if g.gdiag_no > 0 {
        let _ = write!(io.out, "  Bounding box around shell: ");
        region_print(io.out, &region);
    }
    if region.dx <= 0 || region.dy <= 0 || region.dz <= 0 {
        g.notices.add(
            &FS_608,
            "MRISfillInterior: no voxel of the surface's shell is inside the volume (FreeSurfer then \
             allocates a box of negative size and ends the program)",
        );
        if !g.notices.reproduce() {
            log::fatal(
                g,
                io.err,
                1,
                "MRISfillInterior: the surface does not cross the volume (no voxel of its shell is inside it)",
            )?;
        }
    }
    let mut shellbb = mri_alloc(
        g, io.out, io.err, region.dx, region.dy, region.dz, MRI_UCHAR,
    )?;
    for col in region.x..region.x.wrapping_add(region.dx) {
        for row in region.y..region.y.wrapping_add(region.dy) {
            for slc in region.z..region.z.wrapping_add(region.dz) {
                let val = mri_get_vox_val(g, io.err, &mri_shell, col, row, slc, 0, 0)?;
                let (c, r, s) = (col - region.x, row - region.y, slc - region.z);
                mri_set_vox_val(g, io.err, &mut shellbb, c, r, s, 0, val, 0)?;
            }
        }
    }
    drop(mri_shell);
    let outsidebb = mris_flood_outside(g, io.out, io.err, Pair::New(&shellbb))?.unwrap_new();
    for col in region.x..region.x.wrapping_add(region.dx) {
        for row in region.y..region.y.wrapping_add(region.dy) {
            for slc in region.z..region.z.wrapping_add(region.dz) {
                let (c, r, s) = (col - region.x, row - region.y, slc - region.z);
                let val = mri_get_vox_val(g, io.err, &shellbb, c, r, s, 0, 0)?;
                let val2 = mri_get_vox_val(g, io.err, &outsidebb, c, r, s, 0, 0)?;
                if val < 0.5 && val2 > 0.5 {
                    continue;
                }
                let cosa = mri_get_vox_val(g, io.err, &mri_cosa, col, row, slc, 0, 0)?;
                if cosa >= 0.0 {
                    mri_set_vox_val(g, io.err, dst, col, row, slc, 0, 1.0, 0)?;
                }
            }
        }
    }
    if g.gdiag_no > 0 {
        let _ = writeln!(
            io.out,
            "  MRISfillInterior t = {}",
            cfmt::g(start.seconds(), 6)
        );
    }
    Ok(match fresh {
        Some(m) if !given => Ret::New(m),
        _ => Ret::Given,
    })
}
