// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_edit_wm_with_aseg`'s small functions (`mri_edit_wm_with_aseg/mri_edit_wm_with_aseg.cpp`):
//! the voxel accesses (`MRIvox` through the rows' bytes, `MRIgetVoxVal` of the segmentation, the
//! index tables), `neighborLabel`, `distance_to_label`, `distance_to_nonzero`,
//! `distance_to_zero`, `MRInonfilledInNbhd`, `medial_edge_of_hippocampus`,
//! `anterior_edge_of_amygdala`, `distance_to_superior_edge`, `distance_to_lateral_edge`,
//! `distance_to_inferior_edge`, `distance_to_anterior_edge`, `spackle_wm_superior_to_mtl` and
//! `KeepHAILVCP`.

#![allow(non_upper_case_globals)]

use std::io::Write;

use larmorx_core::defects::FS_1236;
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_mri::mri::{MAX_INDEX, Mri};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::cma::{
    Brain_Stem, Left_Amygdala, Left_Hippocampus, Left_VentralDC, Right_VentralDC, Unknown,
    is_amygdala, is_cortex, is_hippo,
};

/// `AUTO_FILL` (`include/mri.h`): the value of the voxels the program turns on.
pub const AUTO_FILL: u8 = 250;
/// `WM_MIN_VAL` and `MIN_WM_VAL` (`include/mri.h`).
pub const WM_MIN_VAL: u8 = 5;

/// `(int) v` of a voxel value (`cvttss2si`; FS-17 beyond the `int` range or NaN).
#[inline]
pub fn int(g: &mut Globals, v: f32) -> i32 {
    if v.is_finite() && v > -2147483648.0 && v < 2147483648.0 {
        return v as i32;
    }
    float_to_int(g, f64::from(v), "mri_edit_wm_with_aseg")
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
/// reads memory it does not own: FS-1236, the table's end value in both modes.
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
        &FS_1236,
        format!("mri_edit_wm_with_aseg reads index table entry {i} of an axis of {len} voxels, past the table"),
    );
    if i <= 0 { 0 } else { (len - 1).max(0) }
}

/// The byte offset of `MRIvox(m, x, y, z)` (frame 0, every type) when `(x, y, z)` lies in the
/// volume.
#[inline]
fn inside(m: &Mri, x: i32, y: i32, z: i32) -> Option<usize> {
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

/// `MRIvox(m, x, y, z)`: byte `x` of row `y` of slice `z` (FreeSurfer's byte pointer, whatever the
/// volume's type). Outside the volume (FS-1236): a column past a row's end is FreeSurfer's next
/// or previous row with `reproduce` and the row's border voxel by default; a row or slice the
/// volume does not have reads 0 in both modes.
#[inline]
pub fn vox(g: &mut Globals, m: &Mri, x: i32, y: i32, z: i32) -> u8 {
    if let Some(i) = inside(m, x, y, z) {
        return m.chunk.as_ref().map_or(0, |c| c[i]);
    }
    let o = off(m, x, y, z);
    g.notices.add(
        &FS_1236,
        format!("mri_edit_wm_with_aseg reads voxel ({x}, {y}, {z}) outside a volume"),
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

/// `MRIvox(m, x, y, z) = v` (see [`vox`]): outside the volume FreeSurfer's other voxel of the
/// buffer with `reproduce`, nowhere by default; a row or slice it does not have nowhere (FS-1236).
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
        &FS_1236,
        format!("mri_edit_wm_with_aseg writes voxel ({x}, {y}, {z}) outside a volume"),
    );
    if let Off::Wrapped(b) = o
        && g.notices.reproduce()
        && let Some(c) = m.chunk.as_mut()
    {
        c[b] = v;
    }
}

/// `MRIgetVoxVal(m, x, y, z, 0)` of a volume the program only reads (the segmentation): frame 0
/// kept as `float`s; an index outside the volume through `MRIgetVoxVal` itself (FS-52, FS-53).
pub struct Seg<'a> {
    pub m: &'a Mri,
    v: Vec<f32>,
}

impl<'a> Seg<'a> {
    /// The values of `m`'s frame 0 (`MRIgetVoxVal`, z outermost).
    pub fn new(g: &mut Globals, err: &mut dyn Write, m: &'a Mri) -> Result<Self, Fatal> {
        let (w, h, d) = (m.vg.width.max(0), m.vg.height.max(0), m.vg.depth.max(0));
        let mut v = Vec::with_capacity(w as usize * h as usize * d as usize);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    v.push(mri_get_vox_val(g, err, m, x, y, z, 0, 0)?);
                }
            }
        }
        Ok(Seg { m, v })
    }

    /// `MRIgetVoxVal(seg, x, y, z, 0)`.
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

    /// `(int) MRIgetVoxVal(seg, x, y, z, 0)`.
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

/// `MRIgetVoxVal(m, x, y, z, 0)` of a byte volume the program changes (the white matter): its
/// byte; outside the volume through `MRIgetVoxVal` (FS-52, FS-53).
#[inline]
pub fn val(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &Mri,
    x: i32,
    y: i32,
    z: i32,
) -> Result<f32, Fatal> {
    if m.mtype == larmorx_freesurfer_mri::mri::MRI_UCHAR
        && let Some(i) = inside(m, x, y, z)
    {
        return Ok(f32::from(m.chunk.as_ref().map_or(0, |c| c[i])));
    }
    mri_get_vox_val(g, err, m, x, y, z, 0, 0)
}

/// `neighborLabel(mri, x, y, z, whalf, label)`: whether a voxel of the window (z outermost,
/// through the index tables) holds `label` (`MRIgetVoxVal == label` in `float`).
pub fn neighbor_label(
    g: &mut Globals,
    err: &mut dyn Write,
    seg: &Seg,
    x: i32,
    y: i32,
    z: i32,
    whalf: i32,
    label: i32,
) -> Result<bool, Fatal> {
    let l = label as f32;
    for zk in -whalf..=whalf {
        let zi = ix(g, seg.m, Ax::Z, z + zk);
        for yk in -whalf..=whalf {
            let yi = ix(g, seg.m, Ax::Y, y + yk);
            for xk in -whalf..=whalf {
                let xi = ix(g, seg.m, Ax::X, x + xk);
                if seg.get(g, err, xi, yi, zi)? == l {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// `distance_to_label(seg, label, x, y, z, dx, dy, dz, max_dist)`: the first step `d` (1 to
/// `max_dist`) whose voxel `(x, y, z) + d (dx, dy, dz)` (through the index tables) holds `label`;
/// `max_dist + 1` when none does.
#[allow(clippy::too_many_arguments)]
pub fn distance_to_label(
    g: &mut Globals,
    err: &mut dyn Write,
    seg: &Seg,
    label: i32,
    x: i32,
    y: i32,
    z: i32,
    dx: i32,
    dy: i32,
    dz: i32,
    max_dist: i32,
) -> Result<i32, Fatal> {
    let l = label as f32;
    let mut d = 1;
    while d <= max_dist {
        let xi = ix(g, seg.m, Ax::X, x.wrapping_add(d.wrapping_mul(dx)));
        let yi = ix(g, seg.m, Ax::Y, y.wrapping_add(d.wrapping_mul(dy)));
        let zi = ix(g, seg.m, Ax::Z, z.wrapping_add(d.wrapping_mul(dz)));
        if seg.get(g, err, xi, yi, zi)? == l {
            break;
        }
        d += 1;
    }
    Ok(d)
}

/// `distance_to_nonzero(wm, ...)` (`nonzero`) or `distance_to_zero(wm, ...)`: the first step whose
/// voxel (through the index tables) is at least `MIN_WM_VAL` (below it); `max_dist + 1` when none.
#[allow(clippy::too_many_arguments)]
pub fn distance_to_wm(
    g: &mut Globals,
    err: &mut dyn Write,
    wm: &Mri,
    x: i32,
    y: i32,
    z: i32,
    dx: i32,
    dy: i32,
    dz: i32,
    max_dist: i32,
    nonzero: bool,
) -> Result<i32, Fatal> {
    let mut d = 1;
    while d <= max_dist {
        let xi = ix(g, wm, Ax::X, x + d * dx);
        let yi = ix(g, wm, Ax::Y, y + d * dy);
        let zi = ix(g, wm, Ax::Z, z + d * dz);
        let on = val(g, err, wm, xi, yi, zi)? >= f32::from(WM_MIN_VAL);
        if on == nonzero {
            break;
        }
        d += 1;
    }
    Ok(d)
}

/// `MRInonfilledInNbhd(mri, x, y, z, whalf)`: the voxels of the window (through the index tables)
/// that are at least `MIN_WM_VAL` and not `AUTO_FILL`.
pub fn nonfilled_in_nbhd(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &Mri,
    x: i32,
    y: i32,
    z: i32,
    whalf: i32,
) -> Result<i32, Fatal> {
    let mut count = 0;
    for zk in -whalf..=whalf {
        let zi = ix(g, m, Ax::Z, z + zk);
        for yk in -whalf..=whalf {
            let yi = ix(g, m, Ax::Y, y + yk);
            for xk in -whalf..=whalf {
                let xi = ix(g, m, Ax::X, x + xk);
                let v = val(g, err, m, xi, yi, zi)?;
                if v >= f32::from(WM_MIN_VAL) && v != f32::from(AUTO_FILL) {
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}

/// `medial_edge_of_hippocampus(seg, x, y, z, dx)`: an unknown, brain stem or ventral DC voxel
/// (the left one for `dx < 0`) within 9 steps along `dx`, and no hippocampus or amygdala 5 to 10
/// steps along it (the index tables).
pub fn medial_edge_of_hippocampus(
    g: &mut Globals,
    err: &mut dyn Write,
    seg: &Seg,
    x: i32,
    y: i32,
    z: i32,
    dx: i32,
) -> Result<bool, Fatal> {
    let vdc = if dx < 0 {
        Left_VentralDC
    } else {
        Right_VentralDC
    };
    if distance_to_label(g, err, seg, Unknown, x, y, z, dx, 0, 0, 10)? < 10
        || distance_to_label(g, err, seg, Brain_Stem, x, y, z, dx, 0, 0, 10)? < 10
        || distance_to_label(g, err, seg, vdc, x, y, z, dx, 0, 0, 10)? < 10
    {
        for xk in 5..=10 {
            let xi = ix(g, seg.m, Ax::X, x + xk * dx);
            let label = seg.label(g, err, xi, y, z)?;
            if is_hippo(label) || is_amygdala(label) {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    Ok(false)
}

/// `anterior_edge_of_amygdala(seg, x, y, z)`: an amygdala voxel with no voxel of its label within
/// 9 steps anterior (+z).
pub fn anterior_edge_of_amygdala(
    g: &mut Globals,
    err: &mut dyn Write,
    seg: &Seg,
    x: i32,
    y: i32,
    z: i32,
) -> Result<bool, Fatal> {
    let label = seg.label(g, err, x, y, z)?;
    if !is_amygdala(label) {
        return Ok(false);
    }
    Ok(distance_to_label(g, err, seg, label, x, y, z, 0, 0, 1, 10)? >= 10)
}

/// The edge searches: from `dist` 0 (below the volume's `bound`), the first whose voxel `(x, y, z)`
/// moved `dist` along the axis (through the index tables) has no voxel of `label` within 9 further
/// steps; -1 (`distance_to_lateral_edge`: `bound`) when none.
#[allow(clippy::too_many_arguments)]
fn edge(
    g: &mut Globals,
    err: &mut dyn Write,
    seg: &Seg,
    label: i32,
    (x, y, z): (i32, i32, i32),
    (dx, dy, dz): (i32, i32, i32),
    bound: i32,
    none: i32,
) -> Result<i32, Fatal> {
    for dist in 0..bound {
        let (px, py, pz) = (
            if dx != 0 {
                ix(g, seg.m, Ax::X, x + dist * dx)
            } else {
                x
            },
            if dy != 0 {
                ix(g, seg.m, Ax::Y, y + dist * dy)
            } else {
                y
            },
            if dz != 0 {
                ix(g, seg.m, Ax::Z, z + dist * dz)
            } else {
                z
            },
        );
        if distance_to_label(g, err, seg, label, px, py, pz, dx, dy, dz, 10)? >= 10 {
            return Ok(dist);
        }
    }
    Ok(none)
}

/// `distance_to_superior_edge(seg, label, x, y, z)` (towards -y, up to the height).
pub fn distance_to_superior_edge(
    g: &mut Globals,
    err: &mut dyn Write,
    seg: &Seg,
    label: i32,
    x: i32,
    y: i32,
    z: i32,
) -> Result<i32, Fatal> {
    edge(
        g,
        err,
        seg,
        label,
        (x, y, z),
        (0, -1, 0),
        seg.m.vg.height,
        -1,
    )
}

/// `distance_to_inferior_edge(seg, label, x, y, z)` (towards +y, up to the height).
pub fn distance_to_inferior_edge(
    g: &mut Globals,
    err: &mut dyn Write,
    seg: &Seg,
    label: i32,
    x: i32,
    y: i32,
    z: i32,
) -> Result<i32, Fatal> {
    edge(
        g,
        err,
        seg,
        label,
        (x, y, z),
        (0, 1, 0),
        seg.m.vg.height,
        -1,
    )
}

/// `distance_to_anterior_edge(seg, label, x, y, z)` (towards +z, up to the HEIGHT: FreeSurfer's
/// bound).
pub fn distance_to_anterior_edge(
    g: &mut Globals,
    err: &mut dyn Write,
    seg: &Seg,
    label: i32,
    x: i32,
    y: i32,
    z: i32,
) -> Result<i32, Fatal> {
    edge(
        g,
        err,
        seg,
        label,
        (x, y, z),
        (0, 0, 1),
        seg.m.vg.height,
        -1,
    )
}

/// `distance_to_lateral_edge(seg, label, x, y, z, left)` (towards +x for the left hemisphere, -x
/// for the right, up to the width; the width when none).
pub fn distance_to_lateral_edge(
    g: &mut Globals,
    err: &mut dyn Write,
    seg: &Seg,
    label: i32,
    x: i32,
    y: i32,
    z: i32,
    left: bool,
) -> Result<i32, Fatal> {
    let dx = if left { 1 } else { -1 };
    let w = seg.m.vg.width;
    edge(g, err, seg, label, (x, y, z), (dx, 0, 0), w, w.max(0))
}

/// The debug voxel's line (`if (x == Gx && y == Gy && z == Gz) printf(...)`).
pub fn at_debug(g: &Globals, x: i32, y: i32, z: i32) -> bool {
    x == g.gx && y == g.gy && z == g.gz
}

/// `spackle_wm_superior_to_mtl(wm, T1, aseg)`: for each hippocampus or amygdala voxel (x
/// outermost, y from 1) at most 1 below its label's superior edge and not within 1 of its anterior
/// edge (there and one voxel down), the cortex voxel above it (through the index table) turned on
/// when off; for the amygdala, also the cortex voxels medial (left `x + 1`, right `x - 1`) at its
/// row and the row above.
pub fn spackle_wm_superior_to_mtl(
    g: &mut Globals,
    err: &mut dyn Write,
    wm: &mut Mri,
    seg: &Seg,
) -> Result<(), Fatal> {
    let (w, h, d) = (wm.vg.width, wm.vg.height, wm.vg.depth);
    for x in 0..w {
        for y in 1..h {
            for z in 0..d {
                let label = seg.label(g, err, x, y, z)?;
                if !is_hippo(label) && !is_amygdala(label) {
                    continue;
                }
                let left = label == Left_Amygdala || label == Left_Hippocampus;
                if distance_to_superior_edge(g, err, seg, label, x, y, z)? > 1 {
                    continue;
                }
                if distance_to_anterior_edge(g, err, seg, label, x, y, z)? < 2
                    && distance_to_anterior_edge(g, err, seg, label, x, y + 1, z)? < 2
                {
                    continue;
                }
                let yi = ix(g, seg.m, Ax::Y, y - 1);
                let slabel = seg.label(g, err, x, yi, z)?;
                if is_cortex(slabel) && val(g, err, wm, x, yi, z)? < f32::from(WM_MIN_VAL) {
                    set_vox(g, wm, x, yi, z, AUTO_FILL);
                }
                let xi = if left { x + 1 } else { x - 1 };
                if is_amygdala(label)
                    && is_cortex(seg.get(g, err, xi, y, z)?)
                    && vox(g, wm, xi, y, z) < WM_MIN_VAL
                {
                    set_vox(g, wm, xi, y, z, AUTO_FILL);
                }
                if is_amygdala(label)
                    && is_cortex(seg.get(g, err, xi, yi, z)?)
                    && vox(g, wm, xi, yi, z) < WM_MIN_VAL
                {
                    set_vox(g, wm, xi, yi, z, AUTO_FILL);
                }
            }
        }
    }
    Ok(())
}

/// `KeepHAILVCP(in, seg, H, A, ILV, CP, lhval, rhval, out)` with `out` the input (the program's
/// call): each voxel whose label (`(int)` of frame 0) is a kept structure's (hippocampus 17/53,
/// amygdala 18/54, inferior lateral ventricle 5/44, choroid plexus 31/63) set to the left or right
/// value (`MRIsetVoxVal`). FreeSurfer runs the columns in parallel; each voxel is its own.
#[allow(clippy::too_many_arguments)]
pub fn keep_hailvcp(
    g: &mut Globals,
    err: &mut dyn Write,
    out: &mut Mri,
    seg: &Seg,
    (h, a, ilv, cp): (bool, bool, bool, bool),
    lhval: f64,
    rhval: f64,
) -> Result<(), Fatal> {
    let (lh, rh) = (lhval as f32, rhval as f32);
    let (w, hh, d) = (seg.m.vg.width, seg.m.vg.height, seg.m.vg.depth);
    for c in 0..w {
        for r in 0..hh {
            for s in 0..d {
                let segid = seg.label(g, err, c, r, s)?;
                let mut set = |v: f32| mri_set_vox_val(g, err, out, c, r, s, 0, v, 0);
                if h && segid == 17 {
                    set(lh)?;
                }
                if h && segid == 53 {
                    set(rh)?;
                }
                if a && segid == 18 {
                    set(lh)?;
                }
                if a && segid == 54 {
                    set(rh)?;
                }
                if ilv && segid == 5 {
                    set(lh)?;
                }
                if ilv && segid == 44 {
                    set(rh)?;
                }
                if cp && segid == 31 {
                    set(lh)?;
                }
                if cp && segid == 63 {
                    set(rh)?;
                }
            }
        }
    }
    Ok(())
}
