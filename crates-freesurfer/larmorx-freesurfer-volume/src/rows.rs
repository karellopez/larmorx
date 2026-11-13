// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's direct voxel access through an `MRI`'s row pointers (`slices[z + f depth][y]` plus
//! `x` elements: `MRIvox`, `MRIFseq_vox`, `MRISseq_vox`, ...), which many `mri.cpp` functions use
//! instead of `MRIgetVoxVal`/`MRIsetVoxVal`, with their element type taken from the code (not from
//! the volume's type), and the volumes they walk given by the first one's extent and frames.
//!
//! An access is `In` the volume, `Wrapped` (inside its buffer but at another voxel: a row walked
//! past its end, or an element type wider than the volume's), or `Outside` (a row pointer the
//! volume does not have, or a place past its buffer: memory the program does not own). FS-289:
//! `Wrapped` reads give FreeSurfer's other voxel with `reproduce` and 0 by default, writes go there
//! with `reproduce` and nowhere by default; FS-290: `Outside` reads give 0 and writes go nowhere in
//! both modes. Both say so.

use std::io::Write;

use larmorx_core::defects::{FS_58, FS_289, FS_290};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri,
};

/// Where a direct access lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum At {
    In(usize),
    Wrapped(usize),
    Outside,
}

/// The element of `size` bytes at `x` of row `y` of slice `sl` (`z + f depth`).
pub(crate) fn at(m: &Mri, sl: i64, y: i64, x: i64, size: usize) -> At {
    let nsl = i64::from(m.vg.depth) * i64::from(m.nframes);
    if m.chunk.is_none() || !(0..nsl).contains(&sl) || !(0..i64::from(m.vg.height)).contains(&y) {
        return At::Outside;
    }
    let row_bytes = i128::from(m.vox_per_row) * m.bytes_per_vox as i128;
    let base = (i128::from(sl) * i128::from(m.vg.height) + i128::from(y)) * row_bytes;
    let b = base + i128::from(x) * size as i128;
    if b < 0 || b + size as i128 > i128::from(m.bytes_total) {
        return At::Outside;
    }
    let inrow = x >= 0 && i128::from(x) * (size as i128) + size as i128 <= row_bytes;
    if inrow && x < i64::from(m.vg.width) {
        At::In(b as usize)
    } else {
        At::Wrapped(b as usize)
    }
}

/// Whether the volume has the row pointer `slices[sl][y]` (FreeSurfer reads it even when it then
/// moves no voxel); FS-290 noted when it does not (FreeSurfer reads past its arrays: a crash or a
/// wild pointer).
pub(crate) fn touch(g: &mut Globals, what: &str, m: &Mri, sl: i64, y: i64) -> bool {
    let nsl = i64::from(m.vg.depth) * i64::from(m.nframes);
    let ok =
        m.chunk.is_some() && (0..nsl).contains(&sl) && (0..i64::from(m.vg.height)).contains(&y);
    if !ok {
        g.notices.add(
            &FS_290,
            format!("{what} reads a row pointer a volume does not have (its extent or frames are smaller)"),
        );
    }
    ok
}

/// The byte offset of `slices[sl][y]` plus `off` bytes, when `n` bytes from there lie in the buffer.
pub(crate) fn row_bytes_at(m: &Mri, sl: i64, y: i64, off: i64, n: usize) -> Option<usize> {
    let nsl = i64::from(m.vg.depth) * i64::from(m.nframes);
    if m.chunk.is_none() || !(0..nsl).contains(&sl) || !(0..i64::from(m.vg.height)).contains(&y) {
        return None;
    }
    let row_bytes = i128::from(m.vox_per_row) * m.bytes_per_vox as i128;
    let b =
        (i128::from(sl) * i128::from(m.vg.height) + i128::from(y)) * row_bytes + i128::from(off);
    (b >= 0 && b + n as i128 <= i128::from(m.bytes_total)).then_some(b as usize)
}

/// `memset(slices[sl][y], 0, n)`: the first `n` bytes of a row (always inside the row here).
pub(crate) fn zero_row(m: &mut Mri, sl: i64, y: i64, n: usize) {
    if let At::In(o) | At::Wrapped(o) = at(m, sl, y, 0, 1) {
        let chunk = m.chunk.as_mut().expect("voxels");
        let end = (o + n).min(chunk.len());
        chunk[o..end].fill(0);
    }
}

/// The bytes of one element of FreeSurfer's type `ty` (`MRIsizeof` for the direct types).
pub(crate) fn elem_size(ty: i32) -> usize {
    match ty {
        MRI_UCHAR => 1,
        MRI_SHORT | MRI_USHRT => 2,
        MRI_INT | MRI_FLOAT => 4,
        MRI_LONG => 8,
        _ => 0,
    }
}

/// A value read directly: an integer type's value or a float.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Num {
    I(i64),
    F(f32),
}

impl Num {
    /// `(float)` of the value (C's conversion: an integer rounded to nearest).
    pub(crate) fn f(self) -> f32 {
        match self {
            Num::I(v) => v as f32,
            Num::F(v) => v,
        }
    }
}

fn decode(chunk: &[u8], o: usize, ty: i32) -> Num {
    let b = |n: usize| &chunk[o..o + n];
    match ty {
        MRI_UCHAR => Num::I(i64::from(chunk[o])),
        MRI_SHORT => Num::I(i64::from(i16::from_le_bytes(b(2).try_into().expect("2")))),
        MRI_USHRT => Num::I(i64::from(u16::from_le_bytes(b(2).try_into().expect("2")))),
        MRI_INT => Num::I(i64::from(i32::from_le_bytes(b(4).try_into().expect("4")))),
        MRI_LONG => Num::I(i64::from_le_bytes(b(8).try_into().expect("8"))),
        _ => Num::F(f32::from_le_bytes(b(4).try_into().expect("4"))),
    }
}

/// The bytes of a value stored as type `ty` (C's conversions: an integer kept in the type's low
/// bits, a float as it is).
pub(crate) fn encode(v: Num, ty: i32) -> Vec<u8> {
    match (ty, v) {
        (MRI_FLOAT, Num::F(f)) => f.to_le_bytes().to_vec(),
        (MRI_FLOAT, Num::I(i)) => (i as f32).to_le_bytes().to_vec(),
        (_, Num::F(_)) => unreachable!("an integer type stores integers"),
        (MRI_UCHAR, Num::I(i)) => vec![i as u8],
        (MRI_SHORT | MRI_USHRT, Num::I(i)) => (i as u16).to_le_bytes().to_vec(),
        (MRI_INT, Num::I(i)) => (i as i32).to_le_bytes().to_vec(),
        (_, Num::I(i)) => i.to_le_bytes().to_vec(),
    }
}

/// Notes FS-289 or FS-290 for an access that is not `In`; whether FreeSurfer's access is made.
fn note(g: &mut Globals, what: &str, a: At, write: bool) -> bool {
    let rw = if write { "writes" } else { "reads" };
    match a {
        At::In(_) => true,
        At::Wrapped(_) => {
            g.notices.add(
                &FS_289,
                format!("{what} {rw} another voxel of a volume (its extent or frames are smaller)"),
            );
            g.notices.reproduce()
        }
        At::Outside => {
            g.notices.add(
                &FS_290,
                format!("{what} {rw} memory outside a volume (its extent or frames are smaller)"),
            );
            false
        }
    }
}

/// A direct read of type `ty` at `(sl, y, x)`: 0 where FreeSurfer's access is not made.
pub(crate) fn read(g: &mut Globals, what: &str, m: &Mri, sl: i64, y: i64, x: i64, ty: i32) -> Num {
    let a = at(m, sl, y, x, elem_size(ty));
    let zero = if ty == MRI_FLOAT {
        Num::F(0.0)
    } else {
        Num::I(0)
    };
    match a {
        At::In(o) => decode(m.chunk.as_ref().expect("voxels"), o, ty),
        At::Wrapped(o) => {
            if note(g, what, a, false) {
                decode(m.chunk.as_ref().expect("voxels"), o, ty)
            } else {
                zero
            }
        }
        At::Outside => {
            note(g, what, a, false);
            zero
        }
    }
}

/// A direct write of `v` as type `ty` at `(sl, y, x)`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write(
    g: &mut Globals,
    what: &str,
    m: &mut Mri,
    sl: i64,
    y: i64,
    x: i64,
    ty: i32,
    v: Num,
) {
    let a = at(m, sl, y, x, elem_size(ty));
    let o = match a {
        At::In(o) => o,
        At::Wrapped(o) => {
            if !note(g, what, a, true) {
                return;
            }
            o
        }
        At::Outside => {
            note(g, what, a, true);
            return;
        }
    };
    let b = encode(v, ty);
    m.chunk.as_mut().expect("voxels")[o..o + b.len()].copy_from_slice(&b);
}

/// A volume without voxels walked through its row pointers: FreeSurfer crashes (FS-58); an error
/// here in both modes.
pub(crate) fn need_voxels(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    m: &Mri,
) -> Result<(), Fatal> {
    if m.chunk.is_some() {
        return Ok(());
    }
    g.notices
        .add(&FS_58, format!("{what} on a volume without voxels"));
    log::error_exit(
        g,
        err,
        ERROR_BADPARM,
        &format!("{what}(): the volume has no voxels (FreeSurfer crashes here)\n"),
    )
}
