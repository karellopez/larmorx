// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Voxel access (`utils/mri.cpp` 1567-1880): `MRIgetVoxVal`, `MRIsetVoxVal` and the derivatives
//! `MRIgetVoxDx`, `MRIgetVoxDy`, `MRIgetVoxDz`; and `MRIboundingBox` (3014), which
//! `LTAalloc` calls.
//!
//! FreeSurfer reads the contiguous buffer at the element `c + r vox_per_row + s vox_per_slice + f
//! vox_per_vol` (in `size_t`) and writes it there and again through the slice and row pointers;
//! complex volumes go through the row pointers only. Indices outside the volume are not checked
//! (but negative columns, rows and slices when reading): FS-52 where the element still lies in the
//! buffer (another voxel: by default reading gives `outside_val` and writing does nothing, with
//! `reproduce` FreeSurfer's voxel), FS-53 where it does not (memory the program does not own: both
//! modes read `outside_val` by default, 0 with `reproduce`, and write nothing). `MRI_LONG` volumes:
//! FS-54. A volume without voxels: FS-58.

use std::io::Write;

use larmorx_core::defects::{FS_52, FS_53, FS_54, FS_58};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::utils::nint;

use crate::mri::{
    MRI_FLOAT, MRI_FLOAT_COMPLEX, MRI_INT, MRI_LONG, MRI_RGB, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri,
    MriRegion,
};

/// `MRI_COMPLEX_REAL`: the real part of a complex voxel.
pub const MRI_COMPLEX_REAL: i32 = 0;
/// `MRI_COMPLEX_IMAG`.
pub const MRI_COMPLEX_IMAG: i32 = 1;

/// Where an access lands: in the volume, in the buffer at another voxel, or outside the buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    In(u64),
    Wrapped(u64),
    Outside,
}

impl Mri {
    fn in_extent(&self, c: i32, r: i32, s: i32, f: i32) -> bool {
        (0..self.vg.width).contains(&c)
            && (0..self.vg.height).contains(&r)
            && (0..self.vg.depth).contains(&s)
            && (0..self.nframes).contains(&f)
    }

    /// The byte offset of an access of `size` bytes at `b` (an exact offset), when it lies in the
    /// buffer.
    fn fits(&self, b: i128, size: u64) -> Option<u64> {
        let total = i128::from(self.bytes_total);
        (b >= 0 && b + i128::from(size) <= total).then_some(b as u64)
    }

    /// The contiguous buffer's element of type size `size` for `(c, r, s, f)`, in `size_t`
    /// arithmetic (C's pointer arithmetic wraps around the address space).
    fn chunk_place(&self, c: i32, r: i32, s: i32, f: i32, size: u64) -> Place {
        let sz = |v: i32| v as i64 as u64;
        let idx = sz(c)
            .wrapping_add(sz(r).wrapping_mul(self.vox_per_row))
            .wrapping_add(sz(s).wrapping_mul(self.vox_per_slice))
            .wrapping_add(sz(f).wrapping_mul(self.vox_per_vol));
        // the address chunk + idx * size, as an offset that may be negative (wrapped)
        let b = idx.wrapping_mul(size) as i64;
        match self.fits(i128::from(b), size) {
            Some(o) if self.in_extent(c, r, s, f) => Place::In(o),
            Some(o) => Place::Wrapped(o),
            None => Place::Outside,
        }
    }

    /// The row pointer `slices[s + f depth][r]` plus `c` elements of `size` bytes (`elem` bytes
    /// the row's stride per voxel: FreeSurfer's row pointers step by the voxel's size): `Outside`
    /// when the row pointer itself lies outside FreeSurfer's slice and row arrays.
    fn row_place(&self, c: i32, r: i32, s: i32, f: i32, size: u64, elem: u64) -> Place {
        let sl = s.wrapping_add(f.wrapping_mul(self.vg.depth));
        let nslices = self.vg.depth.wrapping_mul(self.nframes);
        if !(0..nslices).contains(&sl) || !(0..self.vg.height).contains(&r) {
            return Place::Outside;
        }
        let row = (i128::from(sl) * i128::from(self.vg.height) + i128::from(r))
            * i128::from(self.vox_per_row)
            * i128::from(elem);
        let b = row + i128::from(c) * i128::from(size);
        match self.fits(b, size) {
            Some(o) if self.in_extent(c, r, s, f) => Place::In(o),
            Some(o) => Place::Wrapped(o),
            None => Place::Outside,
        }
    }

    fn bytes<const N: usize>(&self, o: u64) -> [u8; N] {
        let chunk = self.chunk.as_ref().expect("voxels");
        let mut b = [0u8; N];
        b.copy_from_slice(&chunk[o as usize..o as usize + N]);
        b
    }

    fn put(&mut self, o: u64, b: &[u8]) {
        let chunk = self.chunk.as_mut().expect("voxels");
        chunk[o as usize..o as usize + b.len()].copy_from_slice(b);
    }

    /// The stored value at byte `o` as `MRIgetVoxVal` converts it to `float`.
    fn value_at(&self, o: u64) -> f32 {
        match self.mtype {
            MRI_UCHAR => f32::from(self.bytes::<1>(o)[0]),
            MRI_SHORT => f32::from(i16::from_le_bytes(self.bytes(o))),
            MRI_USHRT => f32::from(u16::from_le_bytes(self.bytes(o))),
            MRI_RGB | MRI_INT => i32::from_le_bytes(self.bytes(o)) as f32,
            MRI_LONG => i64::from_le_bytes(self.bytes(o)) as f32,
            MRI_FLOAT => f32::from_le_bytes(self.bytes(o)),
            _ => NO_TYPE,
        }
    }
}

/// What `MRIgetVoxVal` returns for a type it does not read (`-10000000000.9` as a `float`).
const NO_TYPE: f32 = -10000000000.9_f64 as f32;

/// The notice of an access outside the volume, and whether FreeSurfer's is reproduced.
fn outside(g: &mut Globals, what: &str, p: Place, c: i32, r: i32, s: i32, f: i32) -> bool {
    let (d, kind) = match p {
        Place::Wrapped(_) => (&FS_52, "another voxel of the volume"),
        _ => (&FS_53, "memory outside the volume's buffer"),
    };
    g.notices.add(
        d,
        format!("{what}({c}, {r}, {s}, {f}): an index outside the volume, at {kind}"),
    );
    g.notices.reproduce()
}

fn no_voxels(g: &mut Globals, err: &mut dyn Write, what: &str) -> Result<(), Fatal> {
    g.notices
        .add(&FS_58, format!("{what} on a volume without voxels"));
    log::error_exit(
        g,
        err,
        ERROR_BADPARM,
        &format!("{what}(): the volume has no voxels (FreeSurfer crashes here)\n"),
    )
}

/// `MRIgetVoxVal(mri, c, r, s, f, mode)`: the voxel as a `float`; `outside_val` for a negative
/// column, row or slice; `mode` picks a complex voxel's real or imaginary part (another mode ends
/// the program, `MRIgetVoxVal(): unsupported operation <mode>`); `-1e10` for a type it does not
/// read (`MRI_TENSOR` and the others). FS-52, FS-53, FS-58.
#[allow(clippy::too_many_arguments)]
pub fn mri_get_vox_val(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    c: i32,
    r: i32,
    s: i32,
    f: i32,
    mode: i32,
) -> Result<f32, Fatal> {
    if c < 0 || r < 0 || s < 0 {
        return Ok(mri.outside_val as f32);
    }
    if mri.chunk.is_none() {
        no_voxels(g, err, "MRIgetVoxVal")?;
        return Ok(mri.outside_val as f32);
    }
    if mri.mtype == MRI_FLOAT_COMPLEX {
        let p = mri.row_place(c, r, s, f, 8, 8);
        let o = match p {
            Place::In(o) => Some(o),
            Place::Wrapped(o) => outside(g, "MRIgetVoxVal", p, c, r, s, f).then_some(o),
            Place::Outside => {
                let rep = outside(g, "MRIgetVoxVal", p, c, r, s, f);
                return Ok(if rep { 0.0 } else { mri.outside_val as f32 });
            }
        };
        let Some(o) = o else {
            return Ok(mri.outside_val as f32);
        };
        return match mode {
            MRI_COMPLEX_REAL => Ok(f32::from_le_bytes(mri.bytes(o))),
            MRI_COMPLEX_IMAG => Ok(f32::from_le_bytes(mri.bytes(o + 4))),
            _ => {
                log::error_exit(
                    g,
                    err,
                    ERROR_UNSUPPORTED,
                    &format!("MRIgetVoxVal(): unsupported operation {mode}"),
                )?;
                Ok(0.0)
            }
        };
    }
    let size = match mri.mtype {
        MRI_UCHAR => 1,
        MRI_SHORT | MRI_USHRT => 2,
        MRI_RGB | MRI_INT | MRI_FLOAT => 4,
        MRI_LONG => 8,
        _ => return Ok(NO_TYPE),
    };
    let p = mri.chunk_place(c, r, s, f, size);
    match p {
        Place::In(o) => Ok(mri.value_at(o)),
        Place::Wrapped(o) => {
            if outside(g, "MRIgetVoxVal", p, c, r, s, f) {
                Ok(mri.value_at(o))
            } else {
                Ok(mri.outside_val as f32)
            }
        }
        Place::Outside => {
            let rep = outside(g, "MRIgetVoxVal", p, c, r, s, f);
            Ok(if rep { 0.0 } else { mri.outside_val as f32 })
        }
    }
}

/// `nint(x)` to a `long` without FreeSurfer's `int`: `x - 0.5` or `x + 0.5` truncated, saturated
/// at the `long` range, NaN 0 (FS-54's correction).
fn nint_long(x: f64) -> i64 {
    let v = if x < 0.0 { x - 0.5 } else { x + 0.5 };
    if v.is_nan() { 0 } else { v as i64 }
}

/// `MRIsetVoxVal(mri, c, r, s, f, voxval, mode)`: the value clipped to the type's range (`uchar`
/// [0, 255], `short` [-32768, 32767], `ushort` [0, 65535], `int` and `long` the `int` range in
/// `float`), rounded with `nint` (FS-12 beyond `int`) for the integer types, stored; a complex
/// voxel's part by `mode` (another mode ends the program, `MRIsetVoxVal(): unsupported operation
/// <mode>`), not clipped. 0; 1 for a type it does not write (nothing written). FS-52, FS-53,
/// FS-54, FS-58.
#[allow(clippy::too_many_arguments)]
// C's two comparisons per type, as written (the same values as `clamp`, NaN kept)
#[allow(clippy::manual_clamp)]
pub fn mri_set_vox_val(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
    c: i32,
    r: i32,
    s: i32,
    f: i32,
    voxval: f32,
    mode: i32,
) -> Result<i32, Fatal> {
    if mri.chunk.is_none() {
        no_voxels(g, err, "MRIsetVoxVal")?;
        return Ok(0);
    }
    if mri.mtype == MRI_FLOAT_COMPLEX {
        let p = mri.row_place(c, r, s, f, 8, 8);
        let o = match p {
            Place::In(o) => o,
            Place::Wrapped(o) => {
                if !outside(g, "MRIsetVoxVal", p, c, r, s, f) {
                    return Ok(0);
                }
                o
            }
            Place::Outside => {
                outside(g, "MRIsetVoxVal", p, c, r, s, f);
                return Ok(0);
            }
        };
        match mode {
            MRI_COMPLEX_REAL => mri.put(o, &voxval.to_le_bytes()),
            MRI_COMPLEX_IMAG => mri.put(o + 4, &voxval.to_le_bytes()),
            _ => {
                log::error_exit(
                    g,
                    err,
                    ERROR_UNSUPPORTED,
                    &format!("MRIsetVoxVal(): unsupported operation {mode}"),
                )?;
            }
        }
        return Ok(0);
    }
    let mut v = voxval;
    match mri.mtype {
        MRI_UCHAR => {
            if f64::from(v) < 0.0 {
                v = 0.0;
            }
            if v > 255.0 {
                v = 255.0;
            }
        }
        MRI_SHORT => {
            if f64::from(v) < -32768.0 {
                v = -32768.0;
            }
            if f64::from(v) > 32767.0 {
                v = 32767.0;
            }
        }
        MRI_USHRT => {
            if v < 0.0 {
                v = 0.0;
            }
            if v > 65535.0 {
                v = 65535.0;
            }
        }
        MRI_INT => {
            // INT_MIN and INT_MAX compared and assigned as floats: -2^31 and 2^31
            if v < -2147483648.0 {
                v = -2147483648.0;
            }
            if v > 2147483648.0 {
                v = 2147483648.0;
            }
        }
        MRI_LONG => {
            // LONG_MIN and LONG_MAX as floats: -2^63 and 2^63
            if v < -9.223372e18 {
                v = -9.223372e18_f32;
            }
            if v > 9.223372e18 {
                v = 9.223372e18_f32;
            }
        }
        _ => {}
    }
    let size = match mri.mtype {
        MRI_UCHAR => 1,
        MRI_SHORT | MRI_USHRT => 2,
        MRI_RGB | MRI_INT | MRI_FLOAT => 4,
        MRI_LONG => 8,
        _ => return Ok(1),
    };
    let p = mri.chunk_place(c, r, s, f, size);
    // FreeSurfer's second write, through the row pointers (`long32`, 4 bytes, for MRI_LONG)
    let second = if mri.mtype == MRI_LONG {
        mri.row_place(c, r, s, f, 4, 8)
    } else {
        mri.row_place(c, r, s, f, size, size)
    };
    let first = match p {
        Place::In(o) => Some(o),
        Place::Wrapped(o) => outside(g, "MRIsetVoxVal", p, c, r, s, f).then_some(o),
        Place::Outside => {
            outside(g, "MRIsetVoxVal", p, c, r, s, f);
            None
        }
    };
    if !mri.in_extent(c, r, s, f) && !g.notices.reproduce() {
        return Ok(0);
    }
    let rounded = |g: &mut Globals| nint(g, f64::from(v));
    if mri.mtype == MRI_LONG {
        return set_long(g, mri, first, second, v, (c, r, s, f));
    }
    if let Some(o) = first {
        match mri.mtype {
            MRI_UCHAR => {
                let n = rounded(g);
                mri.put(o, &[n as u8]);
            }
            MRI_SHORT => {
                let n = rounded(g);
                mri.put(o, &(n as i16).to_le_bytes());
            }
            MRI_USHRT => {
                let n = rounded(g);
                mri.put(o, &(n as u16).to_le_bytes());
            }
            MRI_RGB | MRI_INT => {
                let n = rounded(g);
                mri.put(o, &n.to_le_bytes());
            }
            _ => mri.put(o, &v.to_le_bytes()),
        }
    }
    if matches!(second, Place::Outside) && !matches!(p, Place::Outside) {
        // the element is in the buffer but its row pointer is not: FreeSurfer's second write
        // goes through memory outside its arrays
        outside(g, "MRIsetVoxVal", Place::Outside, c, r, s, f);
    }
    Ok(0)
}

/// `MRIsetVoxVal` of an `MRI_LONG` volume (FS-54): FreeSurfer stores `nint(v)` (an `int`) as the
/// voxel's `long`, then again as a 4-byte `int` through row pointers that step 8 bytes a voxel,
/// so into half of voxel `c / 2` of the row. By default one `long` write of `v` rounded to the
/// `long` range; `reproduce` both writes.
fn set_long(
    g: &mut Globals,
    mri: &mut Mri,
    first: Option<u64>,
    second: Place,
    v: f32,
    (c, r, s, f): (i32, i32, i32, i32),
) -> Result<i32, Fatal> {
    let Some(o) = first else {
        return Ok(0);
    };
    let reproduce = g.notices.reproduce();
    let before = mri.chunk.clone();
    if reproduce || !mri.in_extent(c, r, s, f) {
        let n = nint(g, f64::from(v));
        mri.put(o, &i64::from(n).to_le_bytes());
        match second {
            Place::In(o2) | Place::Wrapped(o2) => mri.put(o2, &n.to_le_bytes()),
            Place::Outside => {
                outside(g, "MRIsetVoxVal", Place::Outside, c, r, s, f);
            }
        }
        if !reproduce {
            mri.chunk = before;
        }
        g.notices.add(
            &FS_54,
            format!("MRIsetVoxVal({c}, {r}, {s}, {f}) of a long volume: stored twice"),
        );
        return Ok(0);
    }
    // what FreeSurfer would store, for the notice
    let mut fs = mri.chunk.clone().expect("voxels");
    let n = fs_nint(v);
    fs[o as usize..o as usize + 8].copy_from_slice(&i64::from(n).to_le_bytes());
    if let Place::In(o2) | Place::Wrapped(o2) = second {
        fs[o2 as usize..o2 as usize + 4].copy_from_slice(&n.to_le_bytes());
    }
    mri.put(o, &nint_long(f64::from(v)).to_le_bytes());
    if mri.chunk.as_deref() != Some(&fs[..]) {
        g.notices.add(
            &FS_54,
            format!(
                "MRIsetVoxVal({c}, {r}, {s}, {f}) of a long volume: FreeSurfer stores it twice, \
                 the second time as an int into half of another voxel"
            ),
        );
    }
    Ok(0)
}

/// `nint`'s value as FreeSurfer's build gives it (INT_MIN beyond the `int` range), for comparing.
fn fs_nint(v: f32) -> i32 {
    let x = f64::from(v);
    let t = if x < 0.0 { x - 0.5 } else { x + 0.5 };
    if t.is_nan() || t >= 2147483648.0 || t <= -2147483649.0 {
        i32::MIN
    } else {
        t.trunc() as i32
    }
}

/// `(Ip1 - Im1) / (2.0 * size)`: the `float` difference divided in `double`.
fn derivative(ip1: f32, im1: f32, size: f32) -> f32 {
    sse::div64(f64::from(sse::sub(ip1, im1)), 2.0 * f64::from(size)) as f32
}

/// `MRIgetVoxDx(mri, c, r, s, f)`: the central difference along columns over twice the voxel's
/// width, from [`mri_get_vox_val`] at `c + 1` and `c - 1`.
pub fn mri_get_vox_dx(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    c: i32,
    r: i32,
    s: i32,
    f: i32,
) -> Result<f32, Fatal> {
    let ip1 = mri_get_vox_val(g, err, mri, c.wrapping_add(1), r, s, f, MRI_COMPLEX_REAL)?;
    let im1 = mri_get_vox_val(g, err, mri, c.wrapping_sub(1), r, s, f, MRI_COMPLEX_REAL)?;
    Ok(derivative(ip1, im1, mri.vg.xsize))
}

/// `MRIgetVoxDy`: along rows.
pub fn mri_get_vox_dy(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    c: i32,
    r: i32,
    s: i32,
    f: i32,
) -> Result<f32, Fatal> {
    let ip1 = mri_get_vox_val(g, err, mri, c, r.wrapping_add(1), s, f, MRI_COMPLEX_REAL)?;
    let im1 = mri_get_vox_val(g, err, mri, c, r.wrapping_sub(1), s, f, MRI_COMPLEX_REAL)?;
    Ok(derivative(ip1, im1, mri.vg.ysize))
}

/// `MRIgetVoxDz`: along slices.
pub fn mri_get_vox_dz(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    c: i32,
    r: i32,
    s: i32,
    f: i32,
) -> Result<f32, Fatal> {
    let ip1 = mri_get_vox_val(g, err, mri, c, r, s.wrapping_add(1), f, MRI_COMPLEX_REAL)?;
    let im1 = mri_get_vox_val(g, err, mri, c, r, s.wrapping_sub(1), f, MRI_COMPLEX_REAL)?;
    Ok(derivative(ip1, im1, mri.vg.zsize))
}

/// `MRIboundingBox(mri, thresh, &box)`: the smallest box holding every voxel of the first frame
/// above `thresh` (compared as an `int` for the integer types, as a `float` for `MRI_FLOAT`: a NaN
/// is never above), as its first corner and its extent; `NO_ERROR`. A volume with no voxel above
/// gives the corner at the last voxel and the extents `2 - width`, `2 - height`, `2 - depth`.
/// Types other than `MRI_UCHAR`, `MRI_FLOAT`, `MRI_SHORT`, `MRI_USHRT` and `MRI_INT`: the error
/// `MRIboundingBox: unsupported type <type>`, `ERROR_UNSUPPORTED` returned and the box left at the
/// last voxel with the volume's extents. A volume without voxels: FS-58.
pub fn mri_bounding_box(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    thresh: i32,
) -> Result<(i32, MriRegion), Fatal> {
    let (width, height, depth) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let mut b = MriRegion {
        x: width.wrapping_sub(1),
        y: height.wrapping_sub(1),
        z: depth.wrapping_sub(1),
        dx: width,
        dy: height,
        dz: depth,
    };
    let size: u64 = match mri.mtype {
        MRI_UCHAR => 1,
        MRI_SHORT | MRI_USHRT => 2,
        MRI_FLOAT | MRI_INT => 4,
        t => {
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                &format!("MRIboundingBox: unsupported type {t}"),
            );
            return Ok((ERROR_UNSUPPORTED, b));
        }
    };
    let (mut x1, mut y1, mut z1) = (0i32, 0i32, 0i32);
    if height > 0 && depth > 0 && mri.chunk.is_none() {
        // an error exit (FreeSurfer crashes reading the first row)
        no_voxels(g, err, "MRIboundingBox")?;
        return Ok((0, b));
    }
    for z in 0..depth.max(0) {
        for y in 0..height.max(0) {
            let row = (y as u64) * mri.vox_per_row + (z as u64) * mri.vox_per_slice;
            for x in 0..width.max(0) {
                let o = (row + x as u64) * size;
                let above = match mri.mtype {
                    MRI_UCHAR => i32::from(mri.bytes::<1>(o)[0]) > thresh,
                    MRI_FLOAT => f32::from_le_bytes(mri.bytes(o)) > thresh as f32,
                    MRI_SHORT => i32::from(i16::from_le_bytes(mri.bytes(o))) > thresh,
                    MRI_USHRT => i32::from(u16::from_le_bytes(mri.bytes(o))) > thresh,
                    _ => i32::from_le_bytes(mri.bytes(o)) > thresh,
                };
                if above {
                    b.x = b.x.min(x);
                    b.y = b.y.min(y);
                    b.z = b.z.min(z);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                    z1 = z1.max(z);
                }
            }
        }
    }
    b.dx = x1.wrapping_sub(b.x).wrapping_add(1);
    b.dy = y1.wrapping_sub(b.y).wrapping_add(1);
    b.dz = z1.wrapping_sub(b.z).wrapping_add(1);
    Ok((0, b))
}
