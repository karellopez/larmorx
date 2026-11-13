// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Orders of a volume's voxels (`utils/mri.cpp`, 3.1-37's piece c): `MRIorderIndices` (sorted by
//! value, with `compare_sort_mri` and the C library's `qsort`) and `MRIcomputeVoxelPermutation`
//! (a random order from `randomNumber`).
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §13):
//! - **FS-474**: the order by value keeps each voxel's coordinates and value in bytes: a
//!   coordinate past 255 wraps and a value outside [0, 255] or NaN becomes the low byte of its
//!   32-bit truncation with `reproduce`; by default the coordinates are kept whole and the values
//!   clamped (NaN 0), noted.

use std::io::Write;

use larmorx_core::defects::FS_474;
use larmorx_core::glibc_qsort::qsort;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::random::random_number;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

use crate::labels::int_of;

/// The voxel count `width * height * depth` (`int`), the three tables checked to hold it.
fn count(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    mri: &Mri,
    lens: [usize; 3],
) -> Result<Option<usize>, Fatal> {
    let n = mri
        .vg
        .width
        .wrapping_mul(mri.vg.height)
        .wrapping_mul(mri.vg.depth);
    let n = usize::try_from(n).unwrap_or(0);
    if lens.iter().any(|&l| l < n) {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "{what}(): index tables of fewer than {n} entries (FreeSurfer writes past them)\n"
            ),
        )?;
        return Ok(None);
    }
    Ok(Some(n))
}

/// A coordinate kept in a byte (FS-474: wraps past 255 with `reproduce`, whole by default).
fn coord(g: &mut Globals, c: i32) -> i16 {
    if !(0..=255).contains(&c) {
        g.notices.add(
            &FS_474,
            format!("MRIorderIndices keeps the coordinate {c} in a byte"),
        );
        if !g.notices.reproduce() {
            return c as i16;
        }
    }
    i16::from(c as u8)
}

/// `MRIorderIndices(mri, x_indices, y_indices, z_indices)`: the voxels (x outermost, then y, z)
/// with their frame-0 values as bytes (FS-474), sorted by value by the C library's `qsort` (equal
/// values keep their order), their coordinates written in that order. The tables hold at least
/// `width * height * depth` entries (else an error: FreeSurfer writes past them). `NO_ERROR`.
pub fn mri_order_indices(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    xs: &mut [i16],
    ys: &mut [i16],
    zs: &mut [i16],
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIorderIndices";
    let Some(n) = count(g, err, WHAT, mri, [xs.len(), ys.len(), zs.len()])? else {
        return Ok(ERROR_BADPARM);
    };
    // (x, y, z, value); the value as `unsigned char` (x86-64: the low byte of the 32-bit
    // truncation), or clamped by default
    let mut v: Vec<(i16, i16, i16, i32)> = Vec::with_capacity(n);
    for x in 0..mri.vg.width {
        for y in 0..mri.vg.height {
            for z in 0..mri.vg.depth {
                let f = mri_get_vox_val(g, err, mri, x, y, z, 0, 0)?;
                let i = int_of(g, WHAT, f64::from(f));
                let val = if (0..=255).contains(&i) && !f.is_nan() {
                    i
                } else {
                    g.notices.add(
                        &FS_474,
                        format!("MRIorderIndices keeps the value {f} in a byte"),
                    );
                    if g.notices.reproduce() {
                        i32::from(i as u8)
                    } else if f.is_nan() {
                        0
                    } else {
                        i.clamp(0, 255)
                    }
                };
                v.push((coord(g, x), coord(g, y), coord(g, z), val));
            }
        }
    }
    qsort(&mut v, |a, b| {
        if a.3 > b.3 {
            1
        } else if a.3 < b.3 {
            -1
        } else {
            0
        }
    });
    for (k, e) in v.iter().enumerate() {
        xs[k] = e.0;
        ys[k] = e.1;
        zs[k] = e.2;
    }
    Ok(NO_ERROR)
}

/// `MRIcomputeVoxelPermutation(mri, x_indices, y_indices, z_indices)`: the voxels in index order
/// (`i % width`, `(i / width) % height`, `(i / (width * height)) % depth`), then for each `i` in
/// turn swapped with entry `(int)randomNumber(0, n - 0.0001)` (`n` the voxel count). The tables hold
/// at least `n` entries (else an error). `NO_ERROR`.
#[allow(clippy::too_many_arguments)]
pub fn mri_compute_voxel_permutation(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    xs: &mut [i16],
    ys: &mut [i16],
    zs: &mut [i16],
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIcomputeVoxelPermutation";
    let Some(n) = count(g, err, WHAT, mri, [xs.len(), ys.len(), zs.len()])? else {
        return Ok(ERROR_BADPARM);
    };
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    for i in 0..n {
        let i = i as i32;
        xs[i as usize] = (i % w) as i16;
        ys[i as usize] = ((i / w) % h) as i16;
        zs[i as usize] = ((i / w.wrapping_mul(h)) % d) as i16;
    }
    let hi = n as i32 as f64 - 0.0001;
    for i in 0..n {
        let r = random_number(g, env, out, err, 0.0, hi)?;
        let k = int_of(g, WHAT, r);
        let Ok(k) = usize::try_from(k) else {
            continue;
        };
        if k >= n {
            continue;
        }
        xs.swap(k, i);
        ys.swap(k, i);
        zs.swap(k, i);
    }
    Ok(NO_ERROR)
}
