// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! More of `utils/mri2.cpp` (3.1-37, relay 25): `MRIindexNo` (3771), `MRIzconcat` (6466),
//! `MRIapplyEdits` (7168), `MRIfixEntoWM` (7222), `LabelAmygalaCortalJunction` (7264),
//! `MRIshiftDim` (7338), `MRIcropToFoV` (7480), `MRIcropAroundCRS` (7641) and `MRIcropAroundRAS`
//! (7763).
//!
//! The voxels pass from `MRIgetVoxVal` to `MRIsetVoxVal` through a `double` FreeSurfer's compiler
//! removes: they are copied as `float`s (a signalling NaN keeps its bits).
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §24):
//! - **FS-512** (rule A): `MRIzconcat` copies as many rows as the volume has columns: rows past
//!   the width stay 0, and where the width is larger it reads and writes rows the volumes do not
//!   have. By default every row; `reproduce` FreeSurfer's rows (the voxel functions' own notices
//!   for rows outside).
//! - **FS-513** (rule A): `MRIshiftDim` along rows or slices wraps and drops by the width instead
//!   of the height or depth. By default by the shifted axis's extent; `reproduce` by the width.
//! - **FS-514** (b): `LabelAmygalaCortalJunction`'s threads write the left and the right label
//!   into a voxel next to both amygdalae in an order that depends on the threads; here in voxel
//!   order (one thread's), noted.
//! - **FS-515** (memory): `MRIcropAroundRAS` with an orientation that names an axis twice (or
//!   `???`) crops to an extent never set: an error in both modes.
//! - **FS-516** (rule A): `MRIcropAroundRAS` decides the LTA's direction backwards (it inverts it
//!   when the volume is its destination, goes on when the volume is neither end, and refuses when
//!   it is both), comparing the volume's `valid` flag, which no volume constructor sets. By default
//!   the LTA is inverted when the volume is its source only, refused when it is neither end, and
//!   the volume's geometry is compared as valid; `reproduce` FreeSurfer's decision on the flag.
//! - **FS-517** (hang): `MRIcropAroundCRS` whose last voxel lies beyond `int` (a centre that is not
//!   a number, or too far, with a FoV that overflows): FreeSurfer's loop never ends; an error in
//!   both modes.

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_512, FS_513, FS_514, FS_515, FS_516, FS_517};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{matrix_inverse, matrix_print};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_io::colortab::ctab_alloc;
use larmorx_freesurfer_mri::ctab::ctab_deep_copy;
use larmorx_freesurfer_mri::lta::{LINEAR_RAS_TO_RAS, Lta, lta_copy};
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, Mri, MriRegion, mri_alloc, mri_alloc_sequence, mri_copy_header,
    mri_copy_pulse_parameters,
};
use larmorx_freesurfer_mri::vol_geom::{VolGeom, p0_to_cras, set_vox2ras_from_matrix, xfm_crs2xyz};

use crate::arith::mri_extract_region;
use crate::coords::dircos_to_orientation_string;
use crate::labels::int_of;
use crate::ltaops::{InvertInto, lta_change_type, lta_invert, lta_print};
use crate::region::region_print;
use crate::sample::cvtt;
use crate::vgeom::vg_is_equal;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

/// `MRIindexNo(in, out)`: frame 0 of `out` (a new `float` volume of `in`'s grid and header when
/// there is none) holds each voxel's index `s nrows ncols + r ncols + c` (as a `float`).
pub fn mri_index_no(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let o: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let mut m = mri_alloc(
                g,
                out,
                err,
                src.vg.width,
                src.vg.height,
                src.vg.depth,
                MRI_FLOAT,
            )?;
            mri_copy_header(g, out, err, src, Some(&mut m))?;
            fresh.insert(m)
        }
    };
    let mut index = 0i32;
    for s in 0..src.vg.depth {
        for r in 0..src.vg.height {
            for c in 0..src.vg.width {
                mri_set_vox_val(g, err, o, c, r, s, 0, index as f32, 0)?;
                index = index.wrapping_add(1);
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIzconcat(mri1, mri2, nskip, out)`: `mri1`'s slices followed by `mri2`'s from slice `nskip`
/// (`out`: a new volume of `mri1`'s width, height, type and frames, `depth1 + depth2 - nskip`
/// slices and header, when there is none), every frame; `out` takes `mri1`'s pulse parameters and
/// its vox2ras (`MRIsetVox2RASFromMatrix`), extending `mri1`. Messages on standard output and
/// `Null`: `ERROR: MRIzconcat(): nskip=<n> >= nslices in mri2 <d>` (before anything), then the
/// width and height of the two, and `out`'s frames, width, height and depth (`mismatch`). FS-512.
pub fn mri_zconcat(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri1: &Mri,
    mri2: &Mri,
    nskip: i32,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (w1, h1, d1) = (mri1.vg.width, mri1.vg.height, mri1.vg.depth);
    let d2 = mri2.vg.depth;
    let nslices = d1.wrapping_add(d2).wrapping_sub(nskip);
    if nskip >= d2 {
        let _ = writeln!(
            out,
            "ERROR: MRIzconcat(): nskip={nskip} >= nslices in mri2 {d2}"
        );
        return Ok(Ret::Null);
    }
    let mut fresh = None;
    let o: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let mut m = mri_alloc_sequence(g, out, err, w1, h1, nslices, mri1.mtype, mri1.nframes)?;
            mri_copy_header(g, out, err, mri1, Some(&mut m))?;
            fresh.insert(m)
        }
    };
    let checks = [
        (
            w1 != mri2.vg.width,
            "mri1 and mri2 mismatch width",
            w1,
            mri2.vg.width,
        ),
        (
            h1 != mri2.vg.height,
            "mri1 and mri2 mismatch height",
            h1,
            mri2.vg.height,
        ),
        (
            mri1.nframes != o.nframes,
            "out nframes mismatch",
            mri1.nframes,
            o.nframes,
        ),
        (w1 != o.vg.width, "out width mismatch", w1, o.vg.width),
        (h1 != o.vg.height, "out height mismatch", h1, o.vg.height),
        (
            nslices != o.vg.depth,
            "out depth mismatch",
            nslices,
            o.vg.depth,
        ),
    ];
    for (bad, what, a, b) in checks {
        if bad {
            let _ = writeln!(out, "ERROR: MRIzconcat(): {what} {a} {b}");
            return Ok(Ret::Null);
        }
    }
    mri_copy_pulse_parameters(mri1, o);
    let rows = if w1 != h1 {
        g.notices.add(
            &FS_512,
            format!("MRIzconcat: {w1} rows copied (FreeSurfer's), the volumes have {h1}"),
        );
        if g.notices.reproduce() { w1 } else { h1 }
    } else {
        h1
    };
    for c in 0..w1 {
        for r in 0..rows {
            for f in 0..mri1.nframes {
                let mut sout = 0i32;
                for s in 0..d1 {
                    let v = mri_get_vox_val(g, err, mri1, c, r, s, f, 0)?;
                    mri_set_vox_val(g, err, o, c, r, sout, f, v, 0)?;
                    sout += 1;
                }
                for s in nskip..d2 {
                    let v = mri_get_vox_val(g, err, mri2, c, r, s, f, 0)?;
                    mri_set_vox_val(g, err, o, c, r, sout, f, v, 0)?;
                    sout += 1;
                }
            }
        }
    }
    let m = xfm_crs2xyz(g, out, err, &mri1.vg, 0, false)?;
    set_vox2ras_from_matrix(g, out, err, &mut o.vg, &m)?;
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIapplyEdits(newauto, oldauto, manedit, outvol)`: each voxel of every frame is `newauto`'s
/// where `oldauto` and `manedit` agree (as `double`s: a NaN never agrees) and `manedit`'s
/// otherwise; `outvol` a new volume of `oldauto`'s grid, type, frames, header and pulse parameters
/// when there is none. `MRIapplyEdits(): copying <n> from man to new` (the voxels taken from
/// `manedit`) on standard output. In place (`outvol` one of the inputs): give a copy of that input
/// (each voxel is read before it is written).
pub fn mri_apply_edits(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    newauto: &Mri,
    oldauto: &Mri,
    manedit: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let o: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let mut m = mri_alloc_sequence(
                g,
                out,
                err,
                oldauto.vg.width,
                oldauto.vg.height,
                oldauto.vg.depth,
                oldauto.mtype,
                oldauto.nframes,
            )?;
            mri_copy_header(g, out, err, oldauto, Some(&mut m))?;
            mri_copy_pulse_parameters(oldauto, &mut m);
            fresh.insert(m)
        }
    };
    let mut nhits = 0i32;
    for c in 0..oldauto.vg.width {
        for r in 0..oldauto.vg.height {
            for s in 0..oldauto.vg.depth {
                for f in 0..oldauto.nframes {
                    let vnew = mri_get_vox_val(g, err, newauto, c, r, s, f, 0)?;
                    let vold = mri_get_vox_val(g, err, oldauto, c, r, s, f, 0)?;
                    let vman = mri_get_vox_val(g, err, manedit, c, r, s, f, 0)?;
                    if vold == vman {
                        mri_set_vox_val(g, err, o, c, r, s, f, vnew, 0)?;
                    } else {
                        mri_set_vox_val(g, err, o, c, r, s, f, vman, 0)?;
                        nhits = nhits.wrapping_add(1);
                    }
                }
            }
        }
    }
    let _ = writeln!(out, "MRIapplyEdits(): copying {nhits} from man to new");
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// One index of `MRIshiftDim`: `i + nshift`, wrapped by `n` (or dropped, `None`) below 0 and at
/// or past `n`.
fn shifted(i: i32, nshift: i32, n: i32, wrap: bool) -> Option<i32> {
    let mut j = i.wrapping_add(nshift);
    if j < 0 {
        if !wrap {
            return None;
        }
        j = j.wrapping_add(n);
    }
    if j >= n {
        if !wrap {
            return None;
        }
        j = j.wrapping_sub(n);
    }
    Some(j)
}

/// `MRIshiftDim(src, dim, nshift, wrap)`: `MRIshiftDim() dim = <d> nshift = <n>  wrap = <w>` on
/// standard output; `dim` outside 1 to 3: `MRIshiftDim() dim out of range` and `None`. Otherwise a
/// new volume of the source's grid, type, frames, header, pulse parameters and colour table whose
/// voxel at the source's index plus `nshift` along `dim` (1 columns, 2 rows, 3 slices) is the
/// source's voxel, wrapped around (`wrap`) or dropped; the rest 0. The geometry is not changed.
/// FS-513.
pub fn mri_shift_dim(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dim: i32,
    nshift: i32,
    wrap: i32,
) -> Result<Option<Mri>, Fatal> {
    let _ = writeln!(
        out,
        "MRIshiftDim() dim = {dim} nshift = {nshift}  wrap = {wrap}"
    );
    if !(1..=3).contains(&dim) {
        let _ = out.write_all(b"MRIshiftDim() dim out of range\n");
        return Ok(None);
    }
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    let mut o = mri_alloc_sequence(g, out, err, w, h, d, src.mtype, src.nframes)?;
    mri_copy_header(g, out, err, src, Some(&mut o))?;
    mri_copy_pulse_parameters(src, &mut o);
    if src.ct.is_some() {
        o.ct = ctab_deep_copy(g, err, src.ct.as_ref());
    }
    let wrap = wrap != 0;
    // the extent FreeSurfer wraps rows and slices by: the width (FS-513)
    let axis = match dim {
        2 => h,
        3 => d,
        _ => w,
    };
    let by = if axis != w {
        let differs =
            (0..axis.max(0)).any(|i| shifted(i, nshift, w, wrap) != shifted(i, nshift, axis, wrap));
        if differs {
            g.notices.add(
                &FS_513,
                format!("MRIshiftDim: dimension {dim} wrapped by the width {w}, not {axis}"),
            );
        }
        if g.notices.reproduce() { w } else { axis }
    } else {
        w
    };
    for c in 0..w {
        let c2 = if dim == 1 {
            match shifted(c, nshift, w, wrap) {
                Some(j) => j,
                None => continue,
            }
        } else {
            c
        };
        for r in 0..h {
            let r2 = if dim == 2 {
                match shifted(r, nshift, by, wrap) {
                    Some(j) => j,
                    None => continue,
                }
            } else {
                r
            };
            for s in 0..d {
                let s2 = if dim == 3 {
                    match shifted(s, nshift, by, wrap) {
                        Some(j) => j,
                        None => continue,
                    }
                } else {
                    s
                };
                for f in 0..src.nframes {
                    let v = mri_get_vox_val(g, err, src, c, r, s, f, 0)?;
                    mri_set_vox_val(g, err, &mut o, c2, r2, s2, f, v, 0)?;
                }
            }
        }
    }
    Ok(Some(o))
}

/// A `double` passed as a `float` (`cvtsd2ss`): rounded; a NaN keeps its sign and the top of its
/// payload, quieted.
fn narrow(x: f64) -> f32 {
    if x.is_nan() {
        let b = x.to_bits();
        f32::from_bits(
            (((b >> 32) as u32) & 0x8000_0000) | 0x7fc0_0000 | (((b >> 29) as u32) & 0x003f_ffff),
        )
    } else {
        x as f32
    }
}

/// `printf("%g", x)`.
fn pg(x: f64) -> String {
    cfmt::g(x, 6)
}

/// `MRIfixEntoWM(invol, entowm, Level, lhVal, rhVal, ACJ)`: frame 0 of `invol` set to `lhVal`
/// where `entowm` is 3006 (`Level` 1 or 3) or the left gyrus ambiens (3201; 7030 with `ACJ`;
/// `Level` 2 or 3), to `rhVal` likewise for 4006 and 4201 (7031). `MRIfixEntoWM(): <lh> <rh>
/// Level=<l>` and `MRIfixEntoWM(): nchanged = <n>` on standard output; returns the count.
#[allow(clippy::too_many_arguments)]
pub fn mri_fix_ento_wm(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    invol: &mut Mri,
    entowm: &Mri,
    level: i32,
    lh_val: f64,
    rh_val: f64,
    acj: bool,
) -> Result<i32, Fatal> {
    let _ = writeln!(
        out,
        "MRIfixEntoWM(): {} {} Level={level}",
        pg(lh_val),
        pg(rh_val)
    );
    let (lhga, rhga) = if acj { (7030, 7031) } else { (3201, 4201) };
    let (l13, l23) = (level == 1 || level == 3, level == 2 || level == 3);
    let mut nchanged = 0i32;
    for c in 0..invol.vg.width {
        for r in 0..invol.vg.height {
            for s in 0..invol.vg.depth {
                let v = mri_get_vox_val(g, err, entowm, c, r, s, 0, 0)?;
                let i = int_of(g, "MRIfixEntoWM", f64::from(v));
                let val = if (i == 3006 && l13) || (i == lhga && l23) {
                    lh_val
                } else if (i == 4006 && l13) || (i == rhga && l23) {
                    rh_val
                } else {
                    continue;
                };
                mri_set_vox_val(g, err, invol, c, r, s, 0, narrow(val), 0)?;
                nchanged = nchanged.wrapping_add(1);
            }
        }
    }
    let _ = writeln!(out, "MRIfixEntoWM(): nchanged = {nchanged}");
    Ok(nchanged)
}

/// `LabelAmygalaCortalJunction(seg, topo, out)`: each cortex voxel (labels 3 and 42) within
/// `topo` steps of an inner amygdala voxel (18 left, 54 right) labelled 7030 (left) or 7031
/// (right) in frame 0 of `out` (a new `int` volume of the segmentation's grid, header and pulse
/// parameters when there is none, with a colour table of 7,032 places, `CTABalloc`'s random colours,
/// all empty but 7030 and 7031: `Left-` and `Right-Amygdala-Cortical-Junction`, red 255 and 254,
/// green 85, blue 255). `LabelAmygalaCortalJunction((): nhits = <n>` on standard output. FS-514.
pub fn label_amygala_cortal_junction(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    seg: &Mri,
    topo: i32,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (w, h, d) = (seg.vg.width, seg.vg.height, seg.vg.depth);
    let mut fresh = None;
    let o: &mut Mri = match dst {
        Some(m) => m,
        None => {
            let mut m = mri_alloc_sequence(g, out, err, w, h, d, MRI_INT, 1)?;
            mri_copy_header(g, out, err, seg, Some(&mut m))?;
            mri_copy_pulse_parameters(seg, &mut m);
            let mut ct = ctab_alloc(g, env, out, err, 7031 + 1)?;
            if let Some(ct) = ct.as_mut() {
                for e in ct.entries.iter_mut().take(7030) {
                    *e = None;
                }
                for (k, name, ri) in [
                    (7030usize, "Left-Amygdala-Cortical-Junction", 255),
                    (7031, "Right-Amygdala-Cortical-Junction", 254),
                ] {
                    if let Some(Some(e)) = ct.entries.get_mut(k) {
                        e.name = name.as_bytes().to_vec();
                        e.ri = ri;
                        e.gi = 85;
                        e.bi = 255;
                    }
                }
            }
            m.ct = ct;
            fresh.insert(m)
        }
    };
    let mut nhits = 0i32;
    let mut written: std::collections::HashMap<(i32, i32, i32), i32> =
        std::collections::HashMap::new();
    for c in 1..w.wrapping_sub(1) {
        for r in 1..h.wrapping_sub(1) {
            for s in 1..d.wrapping_sub(1) {
                let v = mri_get_vox_val(g, err, seg, c, r, s, 0, 0)?;
                let outlabel = match int_of(g, "LabelAmygalaCortalJunction", f64::from(v)) {
                    18 => 7030,
                    54 => 7031,
                    _ => continue,
                };
                for dc in -1i32..2 {
                    for dr in -1i32..2 {
                        for ds in -1i32..2 {
                            if dc.abs() + dr.abs() + ds.abs() > topo {
                                continue;
                            }
                            let v = mri_get_vox_val(g, err, seg, c + dc, r + dr, s + ds, 0, 0)?;
                            let did = int_of(g, "LabelAmygalaCortalJunction", f64::from(v));
                            if did != 3 && did != 42 {
                                continue;
                            }
                            let key = (c + dc, r + dr, s + ds);
                            if written.insert(key, outlabel).is_some_and(|l| l != outlabel) {
                                g.notices.add(
                                    &FS_514,
                                    "LabelAmygalaCortalJunction: a voxel next to both amygdalae: its label depends on FreeSurfer's threads",
                                );
                            }
                            mri_set_vox_val(g, err, o, key.0, key.1, key.2, 0, outlabel as f32, 0)?;
                            nhits = nhits.wrapping_add(1);
                        }
                    }
                }
            }
        }
    }
    let _ = writeln!(out, "LabelAmygalaCortalJunction((): nhits = {nhits}");
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIcropToFoV(mask, cFoV, rFoV, sFoV, thresh)`: the region of `cFoV x rFoV x sFoV` voxels (each
/// at most the mask's extent) around the centroid of the mask's voxels at or above `thresh` (its
/// corner `max(round(centroid - FoV / 2), 0)`, `FoV / 2` in whole voxels; a mask without such
/// voxels: a NaN centroid, corner 0). The numbers, `Region <x y z  dx dy dz>` and the warnings
/// (`WARNING: cFoV is <n>, could not achieve <m>`, `WARNING: clip cmin` ...) on standard output.
pub fn mri_crop_to_fov(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mask: &Mri,
    fov: [i32; 3],
    thresh: f64,
) -> Result<MriRegion, Fatal> {
    let (mut csum, mut rsum, mut ssum) = (0.0f64, 0.0f64, 0.0f64);
    let (mut mn, mut mx) = ([1_000_000i32; 3], [0i32; 3]);
    let mut nhits = 0i32;
    for c in 0..mask.vg.width {
        for r in 0..mask.vg.height {
            for s in 0..mask.vg.depth {
                let v = f64::from(mri_get_vox_val(g, err, mask, c, r, s, 0, 0)?);
                if v < thresh {
                    continue;
                }
                csum += f64::from(c);
                rsum += f64::from(r);
                ssum += f64::from(s);
                nhits = nhits.wrapping_add(1);
                for (k, i) in [c, r, s].into_iter().enumerate() {
                    mn[k] = mn[k].min(i);
                    mx[k] = mx[k].max(i);
                }
            }
        }
    }
    let n = f64::from(nhits);
    let (csum, rsum, ssum) = (
        sse::div64(csum, n),
        sse::div64(rsum, n),
        sse::div64(ssum, n),
    );
    let _ = write!(
        out,
        "MRIcropToFoV(): thresh={} FoV = ({},{},{}) \nnhits={nhits}  centroid = ({},{},{}) min = ({},{},{}) max = ({},{},{}) delta = ({},{},{}) \n",
        pg(thresh),
        fov[0],
        fov[1],
        fov[2],
        pg(csum),
        pg(rsum),
        pg(ssum),
        mn[0],
        mn[1],
        mn[2],
        mx[0],
        mx[1],
        mx[2],
        mx[0].wrapping_sub(mn[0]).wrapping_add(1),
        mx[1].wrapping_sub(mn[1]).wrapping_add(1),
        mx[2].wrapping_sub(mn[2]).wrapping_add(1)
    );
    // MAX(round(sum - FoV/2), 0) stored in an int
    let corner = |sum: f64, f: i32| {
        let v = sse::sub64(sum, f64::from(f / 2)).round();
        cvtt(if v > 0.0 { v } else { 0.0 })
    };
    let region = MriRegion {
        x: corner(csum, fov[0]),
        y: corner(rsum, fov[1]),
        z: corner(ssum, fov[2]),
        dx: fov[0].min(mask.vg.width),
        dy: fov[1].min(mask.vg.height),
        dz: fov[2].min(mask.vg.depth),
    };
    let _ = out.write_all(b"Region ");
    region_print(out, &region);
    for (got, want, name) in [
        (region.dx, fov[0], "cFoV"),
        (region.dy, fov[1], "rFoV"),
        (region.dz, fov[2], "sFoV"),
    ] {
        if got != want {
            let _ = writeln!(out, "WARNING: {name} is {got}, could not achieve {want}");
        }
    }
    let at = [region.x, region.y, region.z];
    let ext = [region.dx, region.dy, region.dz];
    for (k, a) in ["c", "r", "s"].into_iter().enumerate() {
        if at[k] > mn[k] {
            let _ = writeln!(out, "WARNING: clip {a}min ");
        }
        if ext[k] < mx[k].wrapping_sub(mn[k]) {
            let _ = writeln!(out, "WARNING: clip {a}max ");
        }
    }
    Ok(region)
}

/// `MRIcropAroundCRS(vol, crsCenter, crsFoV, iKeep)`: a volume of `crsFoV` voxels around
/// `crsCenter` (its first voxel `round(centre - floor((FoV - 1) / 2))`): `MRIextractRegion` when the
/// whole box lies in the volume, else a new volume of the volume's type, frames, header, pulse
/// parameters and colour table, its centre set from the first voxel's RAS (`MRIp0ToCRAS`), the
/// voxels inside copied and the rest 0. With `iKeep`, every voxel whose value is not in it is set to
/// 0. The steps on standard output (`MRIcropAroundCRS()`, sizes, centre, FoV, limits, ...).
pub fn mri_crop_around_crs(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vol: &mut Mri,
    center: [f64; 3],
    fov: [i32; 3],
    keep: &[i32],
) -> Result<Option<Mri>, Fatal> {
    let lim = |k: usize| {
        let half = (f64::from(fov[k].wrapping_sub(1)) / 2.0).floor();
        let a = cvtt(sse::sub64(center[k], half).round());
        (a, a.wrapping_add(fov[k]).wrapping_sub(1))
    };
    let ((c0, c1), (r0, r1), (s0, s1)) = (lim(0), lim(1), lim(2));
    let (w, h, d) = (vol.vg.width, vol.vg.height, vol.vg.depth);
    for (k, a) in [c0, r0, s0].into_iter().enumerate() {
        let last = i64::from(a) + i64::from(fov[k]) - 1;
        if last < i64::from(i32::MIN) || last > i64::from(i32::MAX) {
            g.notices.add(
                &FS_517,
                "MRIcropAroundCRS: a limit beyond int (FreeSurfer's loop over the crop never ends)",
            );
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "MRIcropAroundCRS: the limits {a} to {last} lie beyond int (FreeSurfer never ends here)\n"
                ),
            )?;
            return Ok(None);
        }
    }
    let _ = out.write_all(b"MRIcropAroundCRS()\n");
    let _ = writeln!(out, "vol size {w} {h} {d}");
    let _ = writeln!(
        out,
        "center {} {} {}",
        pg(center[0]),
        pg(center[1]),
        pg(center[2])
    );
    let _ = writeln!(out, "fov {} {} {}", fov[0], fov[1], fov[2]);
    let _ = writeln!(out, "limits {c0} {c1}   {r0} {r1}   {s0} {s1}");
    if !keep.is_empty() {
        let _ = out.write_all(b"iKeep ");
        for k in keep {
            let _ = write!(out, "{k} ");
        }
        let _ = out.write_all(b"\n");
    }
    let mut crop = if c0 >= 0 && c1 < w && r0 >= 0 && r1 < h && s0 >= 0 && s1 < d {
        let _ = out.write_all(b"  using MRIextractRegion()\n");
        let bx = MriRegion {
            x: c0,
            y: r0,
            z: s0,
            dx: fov[0],
            dy: fov[1],
            dz: fov[2],
        };
        mri_extract_region(g, out, err, vol, None, Some(&bx))?.into_new()
    } else {
        let mut m =
            mri_alloc_sequence(g, out, err, fov[0], fov[1], fov[2], vol.mtype, vol.nframes)?;
        mri_copy_header(g, out, err, vol, Some(&mut m))?;
        mri_copy_pulse_parameters(vol, &mut m);
        if vol.ct.is_some() {
            m.ct = ctab_deep_copy(g, err, vol.ct.as_ref());
        }
        let mut crs0 = Matrix::real(4, 1);
        crs0.set_relt(1, 1, c0 as f32);
        crs0.set_relt(2, 1, r0 as f32);
        crs0.set_relt(3, 1, s0 as f32);
        crs0.set_relt(4, 1, 1.0);
        let vox2ras = xfm_crs2xyz(g, out, err, &vol.vg, 0, false)?;
        let p0 = matrix_multiply(g, out, err, Some(&vox2ras), Some(&crs0), None)?.unwrap_new();
        let (pr, pa, ps) = (
            f64::from(p0.relt(1, 1)),
            f64::from(p0.relt(2, 1)),
            f64::from(p0.relt(3, 1)),
        );
        p0_to_cras(g, out, err, &mut m.vg, pr, pa, ps)?;
        let _ = writeln!(out, "P0 {} {} {}", pg(pr), pg(pa), pg(ps));
        let _ = writeln!(
            out,
            "c_ras of new volume {} {} {}",
            pg(f64::from(m.vg.c_r)),
            pg(f64::from(m.vg.c_a)),
            pg(f64::from(m.vg.c_s))
        );
        let mut cc = -1i32;
        let mut c = c0;
        while c <= c1 {
            cc += 1;
            if c >= 0 && c < w {
                let mut rc = -1i32;
                let mut r = r0;
                while r <= r1 {
                    rc += 1;
                    if r >= 0 && r < h {
                        let mut sc = -1i32;
                        let mut s = s0;
                        while s <= s1 {
                            sc += 1;
                            if s >= 0 && s < d {
                                for f in 0..vol.nframes {
                                    let v = mri_get_vox_val(g, err, vol, c, r, s, f, 0)?;
                                    mri_set_vox_val(g, err, &mut m, cc, rc, sc, f, v, 0)?;
                                }
                            }
                            s += 1;
                        }
                    }
                    r += 1;
                }
            }
            c += 1;
        }
        Some(m)
    };
    if !keep.is_empty() {
        let Some(m) = crop.as_mut() else {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "MRIcropAroundCRS: no crop to keep labels in (FreeSurfer crashes here)\n",
            )?;
            return Ok(None);
        };
        for c in 0..m.vg.width {
            for r in 0..m.vg.height {
                for s in 0..m.vg.depth {
                    for f in 0..m.nframes {
                        let v = mri_get_vox_val(g, err, m, c, r, s, f, 0)?;
                        if !keep.iter().any(|&k| f64::from(v) == f64::from(k)) {
                            mri_set_vox_val(g, err, m, c, r, s, f, 0.0, 0)?;
                        }
                    }
                }
            }
        }
    }
    Ok(crop)
}

/// `MatrixPrint(stdout, m)`.
fn print_m(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write, m: Option<&Matrix>) {
    let mut buf: Vec<u8> = Vec::new();
    matrix_print(g, Some(&mut buf), out, err, m, 5);
    let _ = out.write_all(&buf);
}

/// The volume's geometry as `vg_isEqual(.., vol)` reads it: its own `valid` flag (FS-516: no
/// constructor sets it) with `reproduce`, else marked valid as `getVolGeom` marks it.
fn vol_as_geom(g: &Globals, vol: &Mri) -> VolGeom {
    let mut v = VolGeom::default();
    crate::vgeom::copy_vol_geom(&vol.vg, &mut v);
    v.ras_good_flag = vol.vg.ras_good_flag;
    if !g.notices.reproduce() {
        v.valid = 1;
    }
    v
}

/// `MRIcropAroundRAS(vol, rasCenter, voxFoVRAS, lta0, iKeep)`: [`mri_crop_around_crs`] around the
/// voxel of `rasCenter` (`inv(vox2ras)`, through a copy of `lta0` made RAS-to-RAS when given,
/// `Chaning Source LTA type to ras2ras`), with the FoV of each voxel axis taken from the entry of
/// `voxFoVRAS` (left-right, front-back, up-down) its orientation letter names. The matrices and
/// numbers on standard output. An LTA neither end of which is the volume: `ERROR:
/// MRIcropAroundRAS(): vol is not src or dst of LTA (<a>, <b>)`, the LTA, `vg_isEqual_Threshold =
/// <t>` and `None`. FS-515, FS-516.
#[allow(clippy::too_many_arguments)]
pub fn mri_crop_around_ras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    vol: &mut Mri,
    ras_center: [f64; 3],
    vox_fov_ras: [i32; 3],
    lta0: Option<&Lta>,
    keep: &[i32],
) -> Result<Option<Mri>, Fatal> {
    let v2r = xfm_crs2xyz(g, out, err, &vol.vg, 0, false)?;
    let mut ras2vox = matrix_inverse(g, out, err, Some(&v2r), None)?.into_new();
    if let Some(l0) = lta0 {
        let Some(mut lta) = lta_copy(g, out, err, l0, None)?.into_new() else {
            return Ok(None);
        };
        if lta.num_xforms <= 0 {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "MRIcropAroundRAS: a transform array without transforms (FreeSurfer reads memory it does not own here)\n",
            )?;
            return Ok(None);
        }
        let vg = vol_as_geom(g, vol);
        if vol.vg.valid != 1 {
            g.notices.add(
                &FS_516,
                "MRIcropAroundRAS: the volume's valid flag, never set by FreeSurfer, compared",
            );
        }
        let ret1 = vg_is_equal(g, &lta.xforms[0].dst, &vg);
        let ret2 = vg_is_equal(g, &lta.xforms[0].src, &vg);
        // FreeSurfer's decision: inverts when the volume is the destination, refuses when it is
        // both ends; the correct one: inverts when it is the source only, refuses when neither
        let (fs_refuse, fs_invert) = (ret1 != 0 && ret2 != 0, ret1 != 0 && ret2 == 0);
        let (ok_refuse, ok_invert) = (ret1 == 0 && ret2 == 0, ret1 == 0 && ret2 != 0);
        if (fs_refuse, fs_invert) != (ok_refuse, ok_invert) {
            g.notices.add(
                &FS_516,
                format!("MRIcropAroundRAS: the LTA's direction decided backwards ({ret1}, {ret2})"),
            );
        }
        let (refuse, invert) = if g.notices.reproduce() {
            (fs_refuse, fs_invert)
        } else {
            (ok_refuse, ok_invert)
        };
        if refuse {
            let _ = writeln!(
                out,
                "ERROR: MRIcropAroundRAS(): vol is not src or dst of LTA ({ret1}, {ret2})"
            );
            lta_print(g, err, out, &lta)?;
            let _ = writeln!(
                out,
                "vg_isEqual_Threshold = {}",
                pg(g.vg_is_equal_threshold)
            );
            return Ok(None);
        }
        if invert {
            let _ = out.write_all(b"Inverting LTA\n");
            lta_invert(g, out, err, InvertInto::Same(&mut lta))?;
        }
        if lta.ltype != LINEAR_RAS_TO_RAS {
            let _ = out.write_all(b"Chaning Source LTA type to ras2ras\n");
            lta_change_type(g, out, err, env, &mut lta, LINEAR_RAS_TO_RAS)?;
        }
        let _ = out.write_all(b"Input RAS2VOX\n");
        print_m(g, out, err, ras2vox.as_ref());
        let _ = out.write_all(b"LTA RAS2RAS\n");
        print_m(g, out, err, lta.xforms[0].m_l.as_ref());
        // MatrixMultiply(ras2vox, m_L, ras2vox): FreeSurfer copies the input it writes into
        match ras2vox.as_mut() {
            Some(m) => {
                let c = m.clone();
                matrix_multiply(g, out, err, Some(&c), lta.xforms[0].m_l.as_ref(), Some(m))?;
            }
            None => {
                matrix_multiply(g, out, err, None, lta.xforms[0].m_l.as_ref(), None)?;
            }
        }
    }
    let _ = out.write_all(b"final ras2vox matrix\n");
    print_m(g, out, err, ras2vox.as_ref());
    let _ = out.write_all(b"--------------------------\n");
    let mut ras = Matrix::real(4, 1);
    for (k, &v) in ras_center.iter().enumerate() {
        ras.set_relt(k as i32 + 1, 1, narrow(v));
    }
    ras.set_relt(4, 1, 1.0);
    let crs = matrix_multiply(g, out, err, ras2vox.as_ref(), Some(&ras), None)?.unwrap_new();
    let crs_center = [
        f64::from(crs.relt(1, 1)),
        f64::from(crs.relt(2, 1)),
        f64::from(crs.relt(3, 1)),
    ];
    let (_, ostr) = dircos_to_orientation_string(&vol.vg);
    let mut fov: [Option<i32>; 3] = [None; 3];
    for (k, &o) in ostr.iter().enumerate() {
        match o {
            b'R' | b'L' => fov[0] = Some(vox_fov_ras[k]),
            b'A' | b'P' => fov[1] = Some(vox_fov_ras[k]),
            b'S' | b'I' => fov[2] = Some(vox_fov_ras[k]),
            _ => {}
        }
    }
    let Some(fov) = fov.iter().copied().collect::<Option<Vec<i32>>>() else {
        g.notices.add(
            &FS_515,
            "MRIcropAroundRAS: an orientation that names an axis twice: a FoV never set",
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "MRIcropAroundRAS: the orientation {} leaves a field of view unset (FreeSurfer reads memory it does not own here)\n",
                String::from_utf8_lossy(&ostr)
            ),
        )?;
        return Ok(None);
    };
    let fov = [fov[0], fov[1], fov[2]];
    let _ = writeln!(
        out,
        "ostr {} ({} {} {}) ({} {} {})",
        String::from_utf8_lossy(&ostr),
        vox_fov_ras[0],
        vox_fov_ras[1],
        vox_fov_ras[2],
        fov[0],
        fov[1],
        fov[2]
    );
    let _ = writeln!(
        out,
        "rasCenter {} {} {}",
        pg(ras_center[0]),
        pg(ras_center[1]),
        pg(ras_center[2])
    );
    let _ = writeln!(
        out,
        "crsCenter {} {} {}",
        pg(crs_center[0]),
        pg(crs_center[1]),
        pg(crs_center[2])
    );
    mri_crop_around_crs(g, out, err, vol, crs_center, fov, keep)
}
