// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Fast-marching distance maps: `MRIextractDistanceMap` (`utils/mri_fastmarching.cpp`) with
//! `include/fastmarching.h`'s `FastMarching<sign>` (its constructor and destructor, `Init`,
//! `AddTrialPoint`, `AddAlivePoint`, `InitTrialFromAlive`, `Run`, `_UpdatePoint`, `_GetValue`,
//! `_UpdateValue`, `InitFromMRI`, `_AddAlivePoint`), and `MRIdistanceTransform`
//! (`utils/mri.cpp` 15492-15540), which calls it.
//!
//! The trial points are a `std::priority_queue<stCoord, std::vector<stCoord>, HeapCompare>`:
//! libstdc++'s (GCC 8.4.0) `push_heap`/`pop_heap` (`__push_heap`, `__adjust_heap`, `__pop_heap`)
//! are translated here step for step, because the queue compares the volume's current values and
//! `_UpdatePoint` lowers the value of a point already queued without restoring the heap's order:
//! the order the points leave the queue, and so the distances, depend on the exact algorithm.
//!
//! `_UpdateValue`'s arithmetic is the official build's (its disassembly): single precision, `sqrtss`,
//! the negations of `FastMarching<-1>` as sign flips, and each operation's operands in the order of
//! the machine code ([`sse`] for the NaNs' bits). FS-539 to FS-544.

use std::io::Write;

use larmorx_core::defects::{FS_58, FS_539, FS_540, FS_541, FS_542, FS_543, FS_544};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc, mri_copy_header};
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_volume::arith::{Pair, mri_clear, mri_scalar_mul};
use larmorx_freesurfer_volume::resize::mri_change_type;
use larmorx_freesurfer_core::run::Environment;

/// `eAlive`, `eTrial`, `eFar`, `eForbidden`.
const ALIVE: u8 = 0;
const TRIAL: u8 = 1;
const FAR: u8 = 2;
const FORBIDDEN: u8 = 3;
/// `EPS`.
const EPS: f32 = 1e-6;

/// `DTRANS_MODE_SIGNED` (negative inside the label, positive outside).
pub const DTRANS_MODE_SIGNED: i32 = 1;
/// `DTRANS_MODE_UNSIGNED` (positive on both sides).
pub const DTRANS_MODE_UNSIGNED: i32 = 2;
/// `DTRANS_MODE_OUTSIDE` (zero inside, positive outside).
pub const DTRANS_MODE_OUTSIDE: i32 = 3;
/// `DTRANS_MODE_INSIDE`.
pub const DTRANS_MODE_INSIDE: i32 = 4;

/// `MRIextractDistanceMap`'s modes: outside, inside, both (signed), both unsigned.
const OUTSIDE: i32 = 1;
const INSIDE: i32 = 2;
const BOTH: i32 = 3;
const BOTH_UNSIGNED: i32 = 4;

/// C's `(int)` of a voxel value (truncated), FS-542 beyond the `int` range.
fn int_of(g: &mut Globals, v: f32, what: &str) -> i32 {
    if (-2147483648.0..2147483648.0).contains(&v) {
        return v as i32;
    }
    g.notices.add(
        &FS_542,
        format!("{what}: the value {v} converted to an int"),
    );
    if g.notices.reproduce() {
        return i32::MIN;
    }
    if v.is_nan() {
        0
    } else if v > 0.0 {
        i32::MAX
    } else {
        i32::MIN
    }
}

/// `static_cast<int>(round(v))` of a label value (C's `round`: halves away from zero), FS-542.
fn label_of(g: &mut Globals, v: f32) -> i32 {
    int_of(g, v.round(), "the distance map's label volume")
}

/// One `FastMarching<sign>` object: the distance volume's frame 0 (in its own bytes), the status
/// volume, the queue of trial points and the list of alive points.
struct Fm<'a> {
    /// `sign` is -1.
    neg: bool,
    w: i32,
    h: i32,
    d: i32,
    /// The distance volume's voxels (`MRIFvox`: its frame 0, rows of `w` floats).
    vals: &'a mut [u8],
    /// The status volume (`MRIvox(status, ...)`).
    status: Vec<u8>,
    heap: Vec<[i32; 3]>,
    alive: Vec<[i32; 3]>,
    limit: f32,
    /// The voxels the constructor set from the limit before it was set (FS-541; with a mask).
    undef: Option<Vec<bool>>,
}

impl Fm<'_> {
    fn idx(&self, x: i32, y: i32, z: i32) -> usize {
        ((z as usize * self.h as usize) + y as usize) * self.w as usize + x as usize
    }

    fn val(&self, i: usize) -> f32 {
        f32::from_le_bytes(self.vals[4 * i..4 * i + 4].try_into().expect("4 bytes"))
    }

    fn set(&mut self, i: usize, v: f32) {
        self.vals[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes());
        if let Some(u) = &mut self.undef {
            u[i] = false;
        }
    }

    fn at(&self, p: [i32; 3]) -> f32 {
        self.val(self.idx(p[0], p[1], p[2]))
    }

    /// `sign * value`.
    fn s(&self, v: f32) -> f32 {
        if self.neg { -v } else { v }
    }

    /// `HeapCompare(a, b)`: `sign * value(a) > sign * value(b)`.
    fn comp(&self, a: [i32; 3], b: [i32; 3]) -> bool {
        let (va, vb) = (self.at(a), self.at(b));
        self.s(va) > self.s(vb)
    }

    /// libstdc++'s `__push_heap(first, hole, top, value, comp)`.
    fn push_heap_hole(&mut self, mut hole: usize, top: usize, value: [i32; 3]) {
        let mut parent = hole.wrapping_sub(1) / 2;
        while hole > top && self.comp(self.heap[parent], value) {
            self.heap[hole] = self.heap[parent];
            hole = parent;
            parent = hole.wrapping_sub(1) / 2;
        }
        self.heap[hole] = value;
    }

    /// `priority_queue::push`: `push_back`, then `push_heap`.
    fn push(&mut self, p: [i32; 3]) {
        self.heap.push(p);
        let hole = self.heap.len() - 1;
        self.push_heap_hole(hole, 0, p);
    }

    /// libstdc++'s `__adjust_heap(first, hole, len, value, comp)`.
    fn adjust_heap(&mut self, hole0: usize, len: usize, value: [i32; 3]) {
        let top = hole0;
        let mut hole = hole0;
        let mut second = hole0;
        // `len` is at least 1 here: (len - 1) / 2 and (len - 2) / 2 as the signed `long`s
        while second < (len - 1) / 2 {
            second = 2 * (second + 1);
            if self.comp(self.heap[second], self.heap[second - 1]) {
                second -= 1;
            }
            self.heap[hole] = self.heap[second];
            hole = second;
        }
        if len & 1 == 0 && second == (len - 2) / 2 {
            second = 2 * (second + 1);
            self.heap[hole] = self.heap[second - 1];
            hole = second - 1;
        }
        self.push_heap_hole(hole, top, value);
    }

    /// `priority_queue::pop`: `pop_heap` (`__pop_heap`: the last element taken out, the top moved
    /// there, the hole at the top sifted down with the taken element), then `pop_back`.
    fn pop(&mut self) {
        let n = self.heap.len();
        if n > 1 {
            let last = n - 1;
            let value = self.heap[last];
            self.heap[last] = self.heap[0];
            self.adjust_heap(0, last, value);
        }
        self.heap.pop();
    }

    /// `_GetValue(x, y, z)`: the limit for a far point, else its distance. A neighbour beyond an
    /// axis of one voxel (the only reads outside the volume) is FS-539 or FS-540.
    fn get_value(&mut self, g: &mut Globals, x: i32, y: i32, z: i32) -> f32 {
        if x < self.w && y < self.h && z < self.d {
            let i = self.idx(x, y, z);
            if self.status[i] == FAR {
                return self.limit;
            }
            return self.val(i);
        }
        // only `x == w` (one voxel wide), `y == h` or `z == d` happen
        let n = self.status.len();
        let flat = (y < self.h && z < self.d)
            .then(|| self.idx(0, y, z) + x as usize)
            .filter(|&i| i < n);
        let Some(i) = flat else {
            g.notices.add(
                &FS_540,
                format!(
                    "a distance map of a {}x{}x{} volume reads the neighbour ({x}, {y}, {z}) outside the volume's memory",
                    self.w, self.h, self.d
                ),
            );
            return self.limit;
        };
        g.notices.add(
            &FS_539,
            format!(
                "a distance map of a volume one voxel wide reads the neighbour ({x}, {y}, {z}) from the next row"
            ),
        );
        if !g.notices.reproduce() {
            return self.limit;
        }
        if self.status[i] == FAR {
            return self.limit;
        }
        if self.undef.as_ref().is_some_and(|u| u[i]) {
            g.notices.add(
                &FS_541,
                "a distance map reads a masked voxel set from the limit before it was set",
            );
            return self.limit;
        }
        self.val(i)
    }

    /// `_UpdateValue(x, y, z)`: the eikonal update from the smaller neighbour on each axis.
    fn update_value(&mut self, g: &mut Globals, x: i32, y: i32, z: i32) -> f32 {
        let (w, h, d) = (self.w, self.h, self.d);
        let mut axis = |g: &mut Globals, at: i32, size: i32, p: [i32; 3], q: [i32; 3]| {
            if at == 0 {
                let v = self.get_value(g, p[0], p[1], p[2]);
                self.s(v)
            } else if at == size - 1 {
                let v = self.get_value(g, q[0], q[1], q[2]);
                self.s(v)
            } else {
                let a = self.get_value(g, p[0], p[1], p[2]);
                let b = self.get_value(g, q[0], q[1], q[2]);
                let (a, b) = (self.s(a), self.s(b));
                // `MIN(a, b)`
                if a < b { a } else { b }
            }
        };
        let mut a = axis(g, x, w, [x + 1, y, z], [x - 1, y, z]);
        let mut b = axis(g, y, h, [x, y + 1, z], [x, y - 1, z]);
        let mut c = axis(g, z, d, [x, y, z + 1], [x, y, z - 1]);
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        if b > c {
            std::mem::swap(&mut b, &mut c);
        }
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        let neg = self.neg;
        let sg = |v: f32| if neg { -v } else { v };
        let ret = sg(sse::add(a, 1.0));
        // the first trinome (sol >= C): the official build's operand orders
        let aa_bb = if neg {
            sse::add(sse::mul(a, a), sse::mul(b, b))
        } else {
            sse::add(sse::mul(b, b), sse::mul(a, a))
        };
        let ab = sse::add(a, b);
        let s3 = sse::add(ab, c);
        let c3 = sse::mul(sse::sub(sse::add(sse::mul(c, c), aa_bb), 1.0), 3.0);
        let delta = sse::sub(sse::mul(s3, s3), c3);
        if delta >= 0.0 {
            let sol = sse::div(sse::add(sse::sqrt(delta), s3), 3.0);
            if sse::add(EPS, sol) >= c {
                return sg(sol);
            }
        }
        // the second (B <= sol < C)
        let c2 = sse::sub(aa_bb, 1.0);
        let delta = sse::sub(sse::mul(ab, ab), sse::add(c2, c2));
        if delta >= 0.0 {
            let sol = sse::mul(sse::add(ab, sse::sqrt(delta)), 0.5);
            if sse::add(EPS, sol) >= b {
                return sg(sol);
            }
        }
        ret
    }

    /// `AddTrialPoint(x, y, z)`.
    fn add_trial_point(&mut self, g: &mut Globals, x: i32, y: i32, z: i32) {
        let i = self.idx(x, y, z);
        if self.status[i] == FAR {
            self.status[i] = TRIAL;
            let v = self.update_value(g, x, y, z);
            self.set(i, v);
            self.push([x, y, z]);
        }
    }

    /// `_UpdatePoint(x, y, z)`.
    fn update_point(&mut self, g: &mut Globals, x: i32, y: i32, z: i32) {
        let i = self.idx(x, y, z);
        match self.status[i] {
            FAR => {
                self.status[i] = TRIAL;
                let v = self.update_value(g, x, y, z);
                self.set(i, v);
                self.push([x, y, z]);
            }
            TRIAL => {
                let v = self.update_value(g, x, y, z);
                self.set(i, v);
            }
            _ => {}
        }
    }

    /// `_AddAlivePoint(x, y, z)`: a point not forbidden gets `sign * 0.5` and, when far, joins the
    /// alive list.
    fn add_alive(&mut self, x: i32, y: i32, z: i32) {
        let i = self.idx(x, y, z);
        if self.status[i] != FORBIDDEN {
            let half = if self.neg { -0.5 } else { 0.5 };
            self.set(i, half);
            if self.status[i] == FAR {
                self.status[i] = ALIVE;
                self.alive.push([x, y, z]);
            }
        }
    }

    /// `InitTrialFromAlive()`.
    fn init_trial_from_alive(&mut self, g: &mut Globals) {
        for k in 0..self.alive.len() {
            let [x, y, z] = self.alive[k];
            if x > 0 {
                self.add_trial_point(g, x - 1, y, z);
            }
            if x < self.w - 1 {
                self.add_trial_point(g, x + 1, y, z);
            }
            if y > 0 {
                self.add_trial_point(g, x, y - 1, z);
            }
            if y < self.h - 1 {
                self.add_trial_point(g, x, y + 1, z);
            }
            if z > 0 {
                self.add_trial_point(g, x, y, z - 1);
            }
            if z < self.d - 1 {
                self.add_trial_point(g, x, y, z + 1);
            }
        }
    }

    /// `InitFromMRI(src, label)`: the label's voxels (or, for the inside, the others) forbidden and
    /// the rest at the limit; the voxels on either side of the label's boundary alive.
    fn init_from_mri(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        src: &Mri,
        label: i32,
    ) -> Result<(), Fatal> {
        let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
        let mut lab = |g: &mut Globals, x: i32, y: i32, z: i32| -> Result<i32, Fatal> {
            let v = mri_get_vox_val(g, err, src, x, y, z, 0, 0)?;
            Ok(label_of(g, v))
        };
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let val = lab(g, x, y, z)?;
                    let i = self.idx(x, y, z);
                    if (val != label) != self.neg {
                        let l = self.limit;
                        self.set(i, l);
                    } else {
                        self.status[i] = FORBIDDEN;
                    }
                }
            }
        }
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let px = if x < self.w - 1 { x + 1 } else { x };
                    let py = if y < self.h - 1 { y + 1 } else { y };
                    let pz = if z < self.d - 1 { z + 1 } else { z };
                    let val1 = lab(g, x, y, z)?;
                    let mut add = false;
                    for (qx, qy, qz) in [(px, y, z), (x, py, z), (x, y, pz)] {
                        let val2 = lab(g, qx, qy, qz)?;
                        if val1 != val2 && (val1 == label || val2 == label) {
                            add = true;
                            self.add_alive(qx, qy, qz);
                        }
                    }
                    if add {
                        self.add_alive(x, y, z);
                    }
                }
            }
        }
        self.init_trial_from_alive(g);
        Ok(())
    }

    /// `Run(limit)`: the trial points leave the queue smallest first (for the inside, largest
    /// first) and become alive, updating their neighbours, until one at the limit leaves: then
    /// every point still queued is far again, at the limit.
    fn run(&mut self, g: &mut Globals, limit: f32) {
        self.limit = limit;
        while let Some(&pt) = self.heap.first() {
            let v = self.at(pt);
            if self.s(v) >= self.s(limit) {
                // the order the queue empties in changes nothing: every point ends far, at the limit
                for k in 0..self.heap.len() {
                    let [x, y, z] = self.heap[k];
                    let i = self.idx(x, y, z);
                    self.status[i] = FAR;
                    self.set(i, limit);
                }
                self.heap.clear();
                return;
            }
            self.pop();
            let [x, y, z] = pt;
            let i = self.idx(x, y, z);
            self.status[i] = ALIVE;
            if x > 0 {
                self.update_point(g, x - 1, y, z);
            }
            if x < self.w - 1 {
                self.update_point(g, x + 1, y, z);
            }
            if y > 0 {
                self.update_point(g, x, y - 1, z);
            }
            if y < self.h - 1 {
                self.update_point(g, x, y + 1, z);
            }
            if z > 0 {
                self.update_point(g, x, y, z - 1);
            }
            if z < self.d - 1 {
                self.update_point(g, x, y, z + 1);
            }
        }
    }
}

/// Whether the mask excludes `(x, y, z)`: `(int)MRIgetVoxVal(mask, x, y, z, 0) == 0` (FS-542).
fn masked(
    g: &mut Globals,
    err: &mut dyn Write,
    mask: &Mri,
    x: i32,
    y: i32,
    z: i32,
) -> Result<bool, Fatal> {
    let v = mri_get_vox_val(g, err, mask, x, y, z, 0, 0)?;
    Ok(int_of(g, v, "the distance map's mask") == 0)
}

/// One pass of `MRIextractDistanceMap`: `FastMarching<sign> fm(dist, mask); fm.SetLimit(limit);
/// fm.InitFromMRI(src, label); fm.Run(limit);` and its destructor.
#[allow(clippy::too_many_arguments)]
fn pass(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dist: &mut Mri,
    src: &Mri,
    mask: Option<&Mri>,
    label: i32,
    limit: f32,
    neg: bool,
) -> Result<(), Fatal> {
    let (w, h, d) = (dist.vg.width, dist.vg.height, dist.vg.depth);
    let status = mri_alloc(g, out, err, w, h, d, MRI_UCHAR)?
        .chunk
        .unwrap_or_default();
    let n = status.len();
    let reproduce = g.notices.reproduce();
    let chunk = dist.chunk.as_mut().expect("voxels");
    let mut fm = Fm {
        neg,
        w,
        h,
        d,
        vals: &mut chunk[..4 * n],
        status,
        heap: Vec::new(),
        alive: Vec::new(),
        limit,
        undef: None,
    };
    // `Init()`: every point far
    fm.status.fill(FAR);
    if let Some(m) = mask {
        if reproduce {
            fm.undef = Some(vec![false; n]);
        }
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if masked(g, err, m, x, y, z)? {
                        let i = fm.idx(x, y, z);
                        fm.status[i] = FORBIDDEN;
                        // FreeSurfer's limit is not set yet here (FS-541): its value is replaced
                        // before the map is returned, and read only through FS-539
                        fm.set(i, limit);
                        if let Some(u) = &mut fm.undef {
                            u[i] = true;
                        }
                    }
                }
            }
        }
    }
    fm.init_from_mri(g, err, src, label)?;
    fm.run(g, limit);
    // the destructor
    if let Some(m) = mask {
        fm.undef = None;
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if masked(g, err, m, x, y, z)? {
                        let i = fm.idx(x, y, z);
                        let l = fm.limit;
                        fm.set(i, l);
                    }
                }
            }
        }
    }
    Ok(())
}

/// `MRIextractDistanceMap(mri_src, mri_dst, label, max_distance, mode, mri_mask)`: the distance of
/// every voxel to the boundary of the voxels whose value rounds to `label`, by fast marching on the
/// voxel grid (unit spacing), in a float volume of the source's extent (a new one with its header
/// without a destination).
///
/// - `max_distance` at or below 0 becomes twice the largest extent; the march stops there and the
///   voxels beyond hold it (the inside's negated).
/// - `mode` 1 (outside): positive distances outside the label, 0 inside; 2 (inside): negative inside,
///   0 outside; 3 (both): both; 4: both, positive. The voxels on each side of the boundary hold
///   +-0.5. Another mode: FS-544.
/// - `mri_mask`: the voxels where the mask's value truncates to 0 are left out of the march and
///   hold the last pass's limit.
/// - A destination of another extent or not of floats: `ERROR : incompatible structure with
///   mri_src:` (or `mri_dst:`) and the differing field on standard error, for each, and the
///   destination returned unchanged.
#[allow(clippy::too_many_arguments)]
pub fn mri_extract_distance_map(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    label: i32,
    mut max_distance: f32,
    mode: i32,
    mask: Option<&Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    if max_distance <= 0.0 {
        max_distance = 2i32.wrapping_mul(w.max(h).max(d)) as f32;
    }
    let mut fresh = None;
    let given = dst.is_some();
    let dist: &mut Mri = match dst {
        Some(m) => m,
        None => {
            let mut m = mri_alloc(g, out, err, w, h, d, MRI_FLOAT)?;
            mri_copy_header(g, out, err, src, Some(&mut m))?;
            fresh.insert(m)
        }
    };
    let ret = |fresh: Option<Mri>| match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    };
    if dist.vg.width != w || dist.vg.height != h || dist.vg.depth != d || dist.mtype != MRI_FLOAT {
        let mut e = String::new();
        for (name, a, b) in [
            ("width", dist.vg.width, w),
            ("height", dist.vg.height, h),
            ("depth", dist.vg.depth, d),
        ] {
            if a != b {
                e += &format!(
                    "ERROR : incompatible structure with mri_src:\nmri_dst->{name}={a} != mri_src->{name}={b}\n"
                );
            }
        }
        if dist.mtype != MRI_FLOAT {
            e += &format!(
                "ERROR : incompatible structure with mri_dst:\nmri_dst->type={} != MRI_FLOAT\n",
                dist.mtype
            );
        }
        let _ = err.write_all(e.as_bytes());
        debug_assert!(given);
        return Ok(ret(fresh));
    }
    if dist.chunk.is_none() {
        g.notices
            .add(&FS_58, "MRIextractDistanceMap on a volume without voxels");
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "MRIextractDistanceMap(): the volume has no voxels (FreeSurfer crashes here)\n",
        )?;
        return Ok(ret(fresh));
    }
    let n = (w as usize) * (h as usize) * (d as usize);
    dist.chunk.as_mut().expect("voxels")[..4 * n].fill(0);
    if !(OUTSIDE..=BOTH_UNSIGNED).contains(&mode) {
        g.notices.add(
            &FS_544,
            format!("MRIextractDistanceMap with the unknown mode {mode}"),
        );
        if !g.notices.reproduce() {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "MRIextractDistanceMap: unknown mode {mode} (1 outside, 2 inside, 3 both, 4 both unsigned)\n"
                ),
            )?;
        }
        return Ok(ret(fresh));
    }
    if mode == OUTSIDE || mode == BOTH || mode == BOTH_UNSIGNED {
        pass(g, out, err, dist, src, mask, label, max_distance, false)?;
    }
    if mode == INSIDE || mode == BOTH || mode == BOTH_UNSIGNED {
        pass(g, out, err, dist, src, mask, label, -max_distance, true)?;
    }
    if mode == BOTH_UNSIGNED {
        let c = dist.chunk.as_mut().expect("voxels");
        for v in c[..4 * n].chunks_exact_mut(4) {
            let f = f32::from_le_bytes(v.try_into().expect("4 bytes")).abs();
            v.copy_from_slice(&f.to_le_bytes());
        }
    }
    Ok(ret(fresh))
}

/// `MRIdistanceTransform(mri_src, mri_dist, label, max_dist, mode, mri_mask)`: the distance map
/// ([`mri_extract_distance_map`]) in units of the source's column size (`xsize`), into a new float
/// volume with the source's header or into `mri_dist` (cleared first, every frame), its
/// `outside_val` set to `max_dist`. `mode`: `DTRANS_MODE_SIGNED` (negative inside), `_UNSIGNED`,
/// `_OUTSIDE`, `_INSIDE` (FS-543); another one FS-544. A source not of floats is changed to floats
/// first (`MRIchangeType(src, MRI_FLOAT, 0, 1, 1)`).
#[allow(clippy::too_many_arguments)]
pub fn mri_distance_transform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &Mri,
    dst: Option<&mut Mri>,
    label: i32,
    max_dist: f32,
    mode: i32,
    mask: Option<&Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    let mut fresh = None;
    let dist: &mut Mri = match dst {
        Some(m) => {
            mri_clear(g, err, m)?;
            m
        }
        None => {
            let mut m = mri_alloc(g, out, err, w, h, d, MRI_FLOAT)?;
            mri_copy_header(g, out, err, src, Some(&mut m))?;
            fresh.insert(m)
        }
    };
    let fm_mode = match mode {
        DTRANS_MODE_SIGNED => BOTH,
        DTRANS_MODE_UNSIGNED => BOTH_UNSIGNED,
        DTRANS_MODE_OUTSIDE => OUTSIDE,
        DTRANS_MODE_INSIDE => {
            g.notices.add(
                &FS_543,
                "MRIdistanceTransform with DTRANS_MODE_INSIDE",
            );
            if g.notices.reproduce() { BOTH_UNSIGNED } else { INSIDE }
        }
        m => {
            g.notices.add(
                &FS_544,
                format!("MRIdistanceTransform with the unknown mode {m}"),
            );
            if !g.notices.reproduce() {
                log::error_exit(
                    g,
                    err,
                    ERROR_BADPARM,
                    &format!(
                        "MRIdistanceTransform: unknown mode {m} (DTRANS_MODE_SIGNED 1, UNSIGNED 2, OUTSIDE 3, INSIDE 4)\n"
                    ),
                )?;
            }
            // FreeSurfer passes it on: 1 to 4 are MRIextractDistanceMap's own modes
            m
        }
    };
    if src.mtype != MRI_FLOAT {
        // a change to floats copies the values (MRIchangeType gives no NULL for floats)
        if let Some(tmp) = mri_change_type(g, out, err, env, src, MRI_FLOAT, 0.0, 1.0, 1)? {
            mri_extract_distance_map(g, out, err, &tmp, Some(&mut *dist), label, max_dist, fm_mode, mask)?;
        }
    } else {
        mri_extract_distance_map(g, out, err, src, Some(&mut *dist), label, max_dist, fm_mode, mask)?;
    }
    dist.outside_val = f64::from(max_dist);
    mri_scalar_mul(g, out, err, Pair::Same(dist), src.vg.xsize)?;
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}
