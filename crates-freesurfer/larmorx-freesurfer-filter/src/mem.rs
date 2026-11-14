// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's direct voxel access through an `MRI`'s row pointers (`MRIvox`, `MRIseq_vox`,
//! `MRIFseq_vox`, `MRISseq_vox`, `MRIIseq_vox`: `slices[z + f depth][y]` plus `x` elements of the
//! macro's type, whatever the volume's own type), as fs.filter's functions use it.
//!
//! An access is `In` the volume (the macro's type is the volume's and the place one of its
//! voxels), `Wrapped` (inside the volume's buffer but not that voxel: an element wider or
//! narrower than the volume's, or a column past a row's end), or `Outside` (a row pointer the
//! volume does not have, or a place past its buffer: memory the program does not own). Each
//! function names the defects it notes for an access that is not `In` (one for `Wrapped`, one for
//! `Outside`); `Wrapped` places are read and written as FreeSurfer does, `Outside` reads give 0
//! and writes go nowhere (memory contents FreeSurfer's run alone can show).

use larmorx_core::defects::Defect;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_LONG, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri};

/// Where a direct access lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum At {
    In(usize),
    Wrapped(usize),
    Outside,
}

/// The bytes of one element of FreeSurfer's type `ty` (the direct types).
pub(crate) fn elem_size(ty: i32) -> usize {
    match ty {
        MRI_UCHAR => 1,
        MRI_SHORT | MRI_USHRT => 2,
        MRI_LONG => 8,
        _ => 4,
    }
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
    if size == m.bytes_per_vox && (0..i64::from(m.vg.width)).contains(&x) {
        At::In(b as usize)
    } else {
        At::Wrapped(b as usize)
    }
}

/// C's `(float)` of the element of type `ty` at byte `o` (an integer rounded to nearest).
pub(crate) fn value(m: &Mri, o: usize, ty: i32) -> f32 {
    let c = m.chunk.as_ref().expect("voxels");
    match ty {
        MRI_UCHAR => f32::from(c[o]),
        MRI_SHORT => f32::from(i16::from_le_bytes([c[o], c[o + 1]])),
        MRI_USHRT => f32::from(u16::from_le_bytes([c[o], c[o + 1]])),
        MRI_LONG => i64::from_le_bytes(c[o..o + 8].try_into().expect("8 bytes")) as f32,
        MRI_FLOAT => f32::from_le_bytes(c[o..o + 4].try_into().expect("4 bytes")),
        _ => i32::from_le_bytes(c[o..o + 4].try_into().expect("4 bytes")) as f32,
    }
}

/// The defects a direct access notes: `Wrapped`, `Outside`.
pub(crate) type Notes = (&'static Defect, &'static Defect);

/// A direct read of type `ty` at `(sl, y, x)` as a `float`: `d.0` noted when it is `Wrapped` (read
/// as FreeSurfer does), `d.1` when `Outside` (0).
#[allow(clippy::too_many_arguments)]
pub(crate) fn read(
    g: &mut Globals,
    d: Notes,
    what: &str,
    m: &Mri,
    sl: i64,
    y: i64,
    x: i64,
    ty: i32,
) -> f32 {
    match at(m, sl, y, x, elem_size(ty)) {
        At::In(o) => value(m, o, ty),
        At::Wrapped(o) => {
            g.notices.add(
                d.0,
                format!("{what}: a direct read at ({x}, {y}) of slice {sl} lands on another voxel"),
            );
            value(m, o, ty)
        }
        At::Outside => {
            g.notices.add(
                d.1,
                format!("{what}: a direct read at ({x}, {y}) of slice {sl} lies outside the volume's memory"),
            );
            0.0
        }
    }
}

/// A direct write of the bytes `b` (one element of `b.len()` bytes) at `(sl, y, x)`: `d.0` noted
/// when it is `Wrapped` (written as FreeSurfer does), `d.1` when `Outside` (nowhere).
#[allow(clippy::too_many_arguments)]
pub(crate) fn write(
    g: &mut Globals,
    d: Notes,
    what: &str,
    m: &mut Mri,
    sl: i64,
    y: i64,
    x: i64,
    b: &[u8],
) {
    match at(m, sl, y, x, b.len()) {
        At::In(o) => m.chunk.as_mut().expect("voxels")[o..o + b.len()].copy_from_slice(b),
        At::Wrapped(o) => {
            g.notices.add(
                d.0,
                format!("{what}: a direct write at ({x}, {y}) of slice {sl} lands on another voxel"),
            );
            m.chunk.as_mut().expect("voxels")[o..o + b.len()].copy_from_slice(b);
        }
        At::Outside => g.notices.add(
            d.1,
            format!("{what}: a direct write at ({x}, {y}) of slice {sl} lies outside the volume's memory"),
        ),
    }
}

/// Whether every direct access of type `ty` to the frame `f` of `m` over an extent `(w, h, d)`
/// is `In` (the macro's type is the volume's, the frame exists and the extent fits).
pub(crate) fn fits(m: &Mri, ty: i32, f: i32, w: i32, h: i32, d: i32) -> bool {
    m.chunk.is_some()
        && m.mtype == ty
        && (0..m.nframes).contains(&f)
        && w <= m.vg.width
        && h <= m.vg.height
        && d <= m.vg.depth
}

/// The frame `f` of `m` as `float`s by direct reads of type `ty` over the volume's own extent
/// (`[z][y][x]`), through [`read`] where an access is not `In`.
pub(crate) fn plane(g: &mut Globals, d: Notes, what: &str, m: &Mri, ty: i32, f: i32) -> Vec<f32> {
    let (w, h, dd) = (m.vg.width, m.vg.height, m.vg.depth);
    let n = (w.max(0) as usize) * (h.max(0) as usize) * (dd.max(0) as usize);
    let mut v = Vec::with_capacity(n);
    if fits(m, ty, f, w, h, dd) {
        let size = elem_size(ty);
        let base = (f as usize) * m.vox_per_vol as usize * size;
        for i in 0..n {
            v.push(value(m, base + i * size, ty));
        }
        return v;
    }
    let base = i64::from(f) * i64::from(dd);
    for z in 0..dd {
        for y in 0..h {
            for x in 0..w {
                v.push(read(
                    g,
                    d,
                    what,
                    m,
                    base + i64::from(z),
                    i64::from(y),
                    i64::from(x),
                    ty,
                ));
            }
        }
    }
    v
}
