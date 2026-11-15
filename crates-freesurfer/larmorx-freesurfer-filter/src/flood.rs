// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mriflood.cpp`'s volume functions (queue item 3.1-43): `MRISfloodoutside` (253),
//! `MRISaccentuate` (433) and `distance` (1129). The surface functions of the file
//! (`MRISshell`, `MRISribbon`, `MRISmakeBoundingVolume`, `MRISfillInterior`) wait for the surface
//! library's voxel maps (`mrisurf_mri.cpp`, queue item 3.4-02) and `mriset.cpp`'s
//! `MRIcomplement` and `MRIintersect`.

use std::io::Write;

use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add, mul, sqrt};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri, mri_alloc, mri_copy_header};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::Pair;

/// `FILL_VAL`: the value of the flooded voxels.
const FILL_VAL: i32 = 1;

/// The destination of a [`Pair`]: the given volume or the source itself, or a new one made by
/// `make` from the source.
fn dest<'a>(
    p: Pair<'a>,
    fresh: &'a mut Option<Mri>,
    make: impl FnOnce(&Mri) -> Result<Mri, Fatal>,
) -> Result<(Option<&'a Mri>, &'a mut Mri), Fatal> {
    Ok(match p {
        Pair::New(s) => (Some(s), fresh.insert(make(s)?)),
        Pair::Given(s, d) => (Some(s), d),
        Pair::Same(d) => (None, d),
    })
}

/// `MRISfloodoutside(mri_src, mri_dst)`: the source copied into the destination (`MRIcopy`), the
/// eight corners set to 1, then sweeps until one fills nothing: eight passes, one from each
/// corner (columns outermost, then rows, then slices, each from its corner's side), each setting
/// to 1 every voxel whose value (truncated to `int`, FS-17 beyond `int`) is 0 and whose neighbour
/// towards the corner along any axis (the voxel itself at the volume's edge) is 1. Values other
/// than 0 and 1 are walls.
pub fn mris_flood_outside(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let given = matches!(p, Pair::Given(..));
    let d: &mut Mri = match p {
        Pair::New(s) => fresh.insert(mri_copy(g, out, err, s, None)?.unwrap_new()),
        Pair::Given(s, d) => {
            mri_copy(g, out, err, s, Some(&mut *d))?;
            d
        }
        Pair::Same(d) => d,
    };
    let (w, h, dep) = (d.vg.width, d.vg.height, d.vg.depth);
    for (i, j, k) in [
        (0, 0, 0),
        (w - 1, 0, 0),
        (0, h - 1, 0),
        (0, 0, dep - 1),
        (w - 1, h - 1, 0),
        (w - 1, 0, dep - 1),
        (0, h - 1, dep - 1),
        (w - 1, h - 1, dep - 1),
    ] {
        mri_set_vox_val(g, err, d, i, j, k, 0, FILL_VAL as f32, 0)?;
    }
    // every voxel's state from its (int) value: 0 floodable, 1 filled, 2 a wall (a voxel set to 1
    // reads back as 1 in every type)
    let (wu, hu, du) = (w.max(0) as usize, h.max(0) as usize, dep.max(0) as usize);
    let n = wu * hu * du;
    // a byte volume with its voxels in one dense block is read and written directly (its value
    // is its (int) value; MRIsetVoxVal of 1 stores the byte 1)
    let bytes = d.mtype == MRI_UCHAR
        && d.vox_per_row as usize == wu
        && d.vox_per_slice as usize == wu * hu
        && d.chunk.as_ref().is_some_and(|c| c.len() >= n);
    let mut st = vec![2u8; n];
    if bytes {
        let c = d.chunk.as_ref().expect("the voxels");
        for (s, &b) in st.iter_mut().zip(&c[..n]) {
            *s = match b {
                0 => 0,
                1 => 1,
                _ => 2,
            };
        }
    } else {
        let mut at = 0;
        for k in 0..dep {
            for j in 0..h {
                for i in 0..w {
                    let f = mri_get_vox_val(g, err, d, i, j, k, 0, 0)?;
                    st[at] = match float_to_int(g, f64::from(f), "MRISfloodoutside") {
                        0 => 0,
                        FILL_VAL => 1,
                        _ => 2,
                    };
                    at += 1;
                }
            }
        }
    }
    // FreeSurfer's sweeps (eight passes, one from each corner, until one fills nothing) end with
    // every floodable voxel 6-connected through floodable voxels to a filled one filled, and no
    // other: the closure is unique, so a breadth-first fill from each filled voxel in turn gives
    // the same voxels (bit for bit; relay 32 of track D, the speed work)
    let plane = wu * hu;
    let mut queue = std::collections::VecDeque::new();
    for seed in 0..n {
        if st[seed] != 1 {
            continue;
        }
        queue.push_back(seed);
        while let Some(p) = queue.pop_front() {
            let (i, j, k) = (p % wu, (p / wu) % hu, p / plane);
            let mut nb = [usize::MAX; 6];
            if i > 0 {
                nb[0] = p - 1;
            }
            if i + 1 < wu {
                nb[1] = p + 1;
            }
            if j > 0 {
                nb[2] = p - wu;
            }
            if j + 1 < hu {
                nb[3] = p + wu;
            }
            if k > 0 {
                nb[4] = p - plane;
            }
            if k + 1 < du {
                nb[5] = p + plane;
            }
            for q in nb {
                if q != usize::MAX && st[q] == 0 {
                    st[q] = 1;
                    if bytes {
                        d.chunk.as_mut().expect("the voxels")[q] = FILL_VAL as u8;
                        queue.push_back(q);
                        continue;
                    }
                    let (qi, qj, qk) = (q % wu, (q / wu) % hu, q / plane);
                    mri_set_vox_val(
                        g,
                        err,
                        d,
                        qi as i32,
                        qj as i32,
                        qk as i32,
                        0,
                        FILL_VAL as f32,
                        0,
                    )?;
                    queue.push_back(q);
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) if !given => Ret::New(m),
        _ => Ret::Given,
    })
}

/// The number of bytes of a `uchar` volume of extent `w` x `h` x `d` whose rows and slices hold no
/// padding (frame 0), when `m` is one.
fn dense_bytes(m: &Mri, w: i32, h: i32, d: i32) -> Option<usize> {
    let n = (w.max(0) as usize) * (h.max(0) as usize) * (d.max(0) as usize);
    (m.mtype == MRI_UCHAR
        && (m.vg.width, m.vg.height, m.vg.depth) == (w, h, d)
        && m.vox_per_row == w as u64
        && m.vox_per_slice == (w as u64) * (h as u64)
        && m.chunk.as_ref().is_some_and(|c| c.len() >= n))
    .then_some(n)
}

/// `MRISaccentuate(mri_src, mri_dst, lo_thresh, hi_thresh)`: frame 0 of each voxel set to 255
/// where the source's value truncated to `int` (FS-17 beyond `int`) is within `[lo_thresh,
/// hi_thresh]`, 0 elsewhere; a new destination is `MRIalloc` of the source's extent and type with
/// its header.
pub fn mris_accentuate(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    lo_thresh: i32,
    hi_thresh: i32,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let given = matches!(p, Pair::Given(..));
    let (src, d) = {
        let mut make = |s: &Mri| -> Result<Mri, Fatal> {
            let mut m = mri_alloc(g, out, err, s.vg.width, s.vg.height, s.vg.depth, s.mtype)?;
            mri_copy_header(g, out, err, s, Some(&mut m))?;
            Ok(m)
        };
        dest(p, &mut fresh, &mut make)?
    };
    let (w, h, dep) = (d.vg.width, d.vg.height, d.vg.depth);
    let (w, h, dep) = match src {
        Some(s) => (s.vg.width, s.vg.height, s.vg.depth),
        None => (w, h, dep),
    };
    // byte volumes of one extent, rows and slices without padding: each byte through a table (the
    // same values: a byte is its own `int`), in place or from the source (relay 33's speed work)
    if let Some(n) = dense_bytes(d, w, h, dep)
        && src.is_none_or(|s| dense_bytes(s, w, h, dep).is_some())
    {
        let lut: [u8; 256] = std::array::from_fn(|b| {
            let b = b as i32;
            if b >= lo_thresh && b <= hi_thresh {
                255
            } else {
                0
            }
        });
        let dc = &mut d.chunk.as_mut().expect("voxels")[..n];
        match src {
            Some(s) => {
                let sc = &s.chunk.as_ref().expect("voxels")[..n];
                for (o, &i) in dc.iter_mut().zip(sc) {
                    *o = lut[usize::from(i)];
                }
            }
            None => {
                for o in dc.iter_mut() {
                    *o = lut[usize::from(*o)];
                }
            }
        }
        return Ok(match fresh {
            Some(m) if !given => Ret::New(m),
            _ => Ret::Given,
        });
    }
    for k in 0..dep {
        for j in 0..h {
            for i in 0..w {
                let f = match src {
                    Some(s) => mri_get_vox_val(g, err, s, i, j, k, 0, 0)?,
                    None => mri_get_vox_val(g, err, d, i, j, k, 0, 0)?,
                };
                let vox = float_to_int(g, f64::from(f), "MRISaccentuate");
                let val = if vox >= lo_thresh && vox <= hi_thresh {
                    255.0
                } else {
                    0.0
                };
                mri_set_vox_val(g, err, d, i, j, k, 0, val, 0)?;
            }
        }
    }
    Ok(match fresh {
        Some(m) if !given => Ret::New(m),
        _ => Ret::Given,
    })
}

/// `distance(x, y, z)`: `sqrtf((x*x + y*y) + z*z)` in `float`.
pub fn distance(x: f32, y: f32, z: f32) -> f32 {
    sqrt(add(add(mul(x, x), mul(y, y)), mul(z, z)))
}
