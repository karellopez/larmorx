// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! More of `utils/mri2.cpp` (3.1-37, relay 25): `MRIindexNo` (3771), `MRIzconcat` (6466),
//! `MRIapplyEdits` (7168) and `MRIshiftDim` (7338).
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

use std::io::Write;

use larmorx_core::defects::{FS_512, FS_513};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_mri::ctab::ctab_deep_copy;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, Mri, mri_alloc, mri_alloc_sequence, mri_copy_header, mri_copy_pulse_parameters,
};
use larmorx_freesurfer_mri::vol_geom::{set_vox2ras_from_matrix, xfm_crs2xyz};
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
