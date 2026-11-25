// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_fill`'s voxel accesses (`mri_fill/mri_fill.cpp`): `MRIvox` through the rows' bytes, the
//! index tables (`mri->xi[]`, ...), `MRIgetVoxVal` of a volume the program only reads, and the
//! constants of `include/mri.h` the program uses.
//!
//! FreeSurfer's `MRIvox(mri, x, y, z)` is `((BUFTYPE *)mri->slices[z][y])[x]`: a column past a
//! row's end is the next (or previous) row's byte in the volume's buffer (FS-1291: `reproduce`
//! gives FreeSurfer's byte of the buffer, the default the row's border voxel); a row or slice the
//! volume does not have, or an index table read past its margin, is memory it does not own
//! (FS-1300: 0 read and nothing written in both modes).

use std::io::Write;

use larmorx_core::defects::{FS_1291, FS_1300};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_mri::mri::{MAX_INDEX, Mri};
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

/// `WM_MIN_VAL` and `MIN_WM_VAL` (`include/mri.h`).
pub const WM_MIN_VAL: i32 = 5;
/// `WM_EDITED_ON_VAL`, `WM_EDITED_OFF_VAL` (`include/mri.h`).
pub const WM_EDITED_ON_VAL: i32 = 255;
pub const WM_EDITED_OFF_VAL: i32 = 1;
/// `MRI_LEFT_HEMISPHERE`, `MRI_RIGHT_HEMISPHERE` (`include/mri.h`): the default fill values.
pub const MRI_LEFT_HEMISPHERE: i32 = 255;
pub const MRI_RIGHT_HEMISPHERE: i32 = 127;
/// `THICKEN_FILL`, `NBHD_FILL`, `VENTRICLE_FILL`, `DIAGONAL_FILL`, `DEGENERATE_FILL`
/// (`include/mri.h`): the labels the program saves and erases before the cutting planes.
pub const FILL_LABELS: [i32; 5] = [200, 210, 220, 230, 240];
/// `DEFAULT_DESIRED_WHITE_MATTER_VALUE / 2 - 1` (`include/mrinorm.h`: 110).
pub const WM_THRESH: f32 = 54.0;
/// `MRI_HORIZONTAL` (`include/mri.h`).
pub const MRI_HORIZONTAL: i32 = 2;
/// The status of a process ended by `SIGSEGV`, as a shell reports it.
pub const SEGFAULT: i32 = 139;

/// `(int) v` of a voxel value (`cvttss2si`; FS-17 beyond the `int` range or NaN).
#[inline]
pub fn int(g: &mut Globals, v: f32) -> i32 {
    if v.is_finite() && v > -2147483648.0 && v < 2147483648.0 {
        return v as i32;
    }
    float_to_int(g, f64::from(v), "mri_fill")
}

/// `(int) v` of a `double` (`cvttsd2si`; FS-17 beyond the `int` range or NaN).
#[inline]
pub fn int64(g: &mut Globals, v: f64) -> i32 {
    if v.is_finite() && v > -2147483649.0 && v < 2147483648.0 {
        return v as i32;
    }
    float_to_int(g, v, "mri_fill")
}

/// An axis of the index tables.
#[derive(Clone, Copy)]
pub enum Ax {
    X,
    Y,
    Z,
}

/// `m->xi[i]`, `m->yi[i]`, `m->zi[i]`: the boundary index tables (0 at or below 0, the last voxel
/// at or past the end). Past the tables' margin (`MAX_INDEX` entries on each side) FreeSurfer
/// reads memory it does not own: FS-1300, the table's end value in both modes.
#[inline]
pub fn ix(g: &mut Globals, m: &Mri, a: Ax, i: i32) -> i32 {
    let (t, len) = match a {
        Ax::X => (m.xi.as_deref(), m.vg.width),
        Ax::Y => (m.yi.as_deref(), m.vg.height),
        Ax::Z => (m.zi.as_deref(), m.vg.depth),
    };
    let t = t.unwrap_or(&[]);
    let k = i64::from(i) + i64::from(MAX_INDEX);
    if k >= 0 && (k as usize) < t.len() {
        return t[k as usize];
    }
    g.notices.add(
        &FS_1300,
        format!("mri_fill reads index table entry {i} of an axis of {len} voxels, past the table"),
    );
    if i <= 0 { 0 } else { (len - 1).max(0) }
}

/// The byte offset of `MRIvox(m, x, y, z)` (frame 0, every type) when `(x, y, z)` lies in the
/// volume.
#[inline]
pub fn inside(m: &Mri, x: i32, y: i32, z: i32) -> Option<usize> {
    if x < 0 || y < 0 || z < 0 || x >= m.vg.width || y >= m.vg.height || z >= m.vg.depth {
        return None;
    }
    m.chunk.as_ref()?;
    let row = m.vox_per_row as usize * m.bytes_per_vox;
    Some((z as usize * m.vg.height as usize + y as usize) * row + x as usize)
}

/// Where a direct access outside the volume lands: in the buffer at another voxel (a column past
/// a row's end), or outside it (a row or slice the volume does not have).
enum Off {
    Wrapped(usize),
    Outside,
}

fn off(m: &Mri, x: i32, y: i32, z: i32) -> Off {
    let nsl = i64::from(m.vg.depth) * i64::from(m.nframes);
    if m.chunk.is_none()
        || !(0..i64::from(m.vg.height)).contains(&i64::from(y))
        || !(0..nsl).contains(&i64::from(z))
    {
        return Off::Outside;
    }
    let row = i128::from(m.vox_per_row) * m.bytes_per_vox as i128;
    let b = (i128::from(z) * i128::from(m.vg.height) + i128::from(y)) * row + i128::from(x);
    if b < 0 || b >= i128::from(m.bytes_total) {
        return Off::Outside;
    }
    Off::Wrapped(b as usize)
}

/// `MRIvox(m, x, y, z)`: byte `x` of row `y` of slice `z`. Outside the volume: FS-1291 or FS-1300
/// (see the module's text).
#[inline]
pub fn vox(g: &mut Globals, m: &Mri, x: i32, y: i32, z: i32) -> u8 {
    if let Some(i) = inside(m, x, y, z) {
        return m.chunk.as_ref().map_or(0, |c| c[i]);
    }
    let o = off(m, x, y, z);
    g.notices.add(
        if matches!(o, Off::Wrapped(_)) {
            &FS_1291
        } else {
            &FS_1300
        },
        format!("mri_fill reads voxel ({x}, {y}, {z}) outside a volume"),
    );
    match o {
        Off::Wrapped(b) if g.notices.reproduce() => m.chunk.as_ref().map_or(0, |c| c[b]),
        Off::Wrapped(_) => {
            let xc = x.clamp(0, (m.vg.width - 1).max(0));
            inside(m, xc, y, z).map_or(0, |i| m.chunk.as_ref().map_or(0, |c| c[i]))
        }
        Off::Outside => 0,
    }
}

/// `MRIvox(m, x, y, z) = v` (see [`vox`]): outside the volume FreeSurfer's other byte of the
/// buffer with `reproduce`, nowhere by default (FS-1291); a row or slice it does not have nowhere
/// (FS-1300).
#[inline]
pub fn set_vox(g: &mut Globals, m: &mut Mri, x: i32, y: i32, z: i32, v: u8) {
    if let Some(i) = inside(m, x, y, z) {
        if let Some(c) = m.chunk.as_mut() {
            c[i] = v;
        }
        return;
    }
    let o = off(m, x, y, z);
    g.notices.add(
        if matches!(o, Off::Wrapped(_)) {
            &FS_1291
        } else {
            &FS_1300
        },
        format!("mri_fill writes voxel ({x}, {y}, {z}) outside a volume"),
    );
    if let Off::Wrapped(b) = o
        && g.notices.reproduce()
        && let Some(c) = m.chunk.as_mut()
    {
        c[b] = v;
    }
}

/// A byte volume's frame 0 as a flat slice (`z` outermost, rows of `vox_per_row`): the fast path of
/// the program's whole-volume loops (every index in the volume). `None` for another type or a
/// volume without voxels.
pub fn bytes(m: &Mri) -> Option<(&[u8], usize, usize)> {
    if m.bytes_per_vox != 1 {
        return None;
    }
    let row = m.vox_per_row as usize;
    let slice = row * m.vg.height.max(0) as usize;
    let n = slice * m.vg.depth.max(0) as usize;
    let c = m.chunk.as_ref()?;
    (c.len() >= n).then(|| (&c[..n], row, slice))
}

/// [`bytes`], writable.
pub fn bytes_mut(m: &mut Mri) -> Option<(&mut [u8], usize, usize)> {
    if m.bytes_per_vox != 1 {
        return None;
    }
    let row = m.vox_per_row as usize;
    let slice = row * m.vg.height.max(0) as usize;
    let n = slice * m.vg.depth.max(0) as usize;
    let c = m.chunk.as_mut()?;
    if c.len() < n {
        return None;
    }
    Some((&mut c[..n], row, slice))
}

/// `MRIgetVoxVal(m, x, y, z, 0)` of a volume the program only reads (the segmentation): frame 0
/// kept as `float`s; an index outside the volume through `MRIgetVoxVal` itself (FS-52, FS-53).
pub struct Vals<'a> {
    pub m: &'a Mri,
    v: Vec<f32>,
}

impl<'a> Vals<'a> {
    /// The values of `m`'s frame 0 (`MRIgetVoxVal`, z outermost).
    pub fn new(g: &mut Globals, err: &mut dyn Write, m: &'a Mri) -> Result<Self, Fatal> {
        let (w, h, d) = (m.vg.width.max(0), m.vg.height.max(0), m.vg.depth.max(0));
        let mut v = Vec::with_capacity(w as usize * h as usize * d as usize);
        let fr = m.frame_reader(0);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let val = match fr.as_ref().and_then(|f| f.get(x, y, z)) {
                        Some(v) => v,
                        None => mri_get_vox_val(g, err, m, x, y, z, 0, 0)?,
                    };
                    v.push(val);
                }
            }
        }
        Ok(Vals { m, v })
    }

    /// `MRIgetVoxVal(m, x, y, z, 0)`.
    #[inline]
    pub fn get(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        x: i32,
        y: i32,
        z: i32,
    ) -> Result<f32, Fatal> {
        let (w, h, d) = (self.m.vg.width, self.m.vg.height, self.m.vg.depth);
        if x >= 0 && y >= 0 && z >= 0 && x < w && y < h && z < d {
            return Ok(self.v[(z as usize * h as usize + y as usize) * w as usize + x as usize]);
        }
        mri_get_vox_val(g, err, self.m, x, y, z, 0, 0)
    }

    /// `(int) MRIgetVoxVal(m, x, y, z, 0)` (an `int label = MRIgetVoxVal(...)`).
    #[inline]
    pub fn label(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        x: i32,
        y: i32,
        z: i32,
    ) -> Result<i32, Fatal> {
        let v = self.get(g, err, x, y, z)?;
        Ok(int(g, v))
    }
}
