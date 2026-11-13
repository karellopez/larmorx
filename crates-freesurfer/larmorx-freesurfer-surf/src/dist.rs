// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The neighbour distances' storage (`fs.surfio`; `utils/mrisurf_base.cpp` 270-420, queue item
//! 3.1-47's lines, brought forward because the coordinate operations of 3.1-49 free the
//! distances): `freeDistOrDistOrig`, `changeDistOrDistOrig`, `mrisStealDistStore`,
//! `mrisSetDist`, `MRISgrowDist`, `MRISgrowDistOrig`, `MRISmakeDist`, `MRISmakeDistOrig`,
//! `MRISfreeDistsButNotOrig`, `MRISfreeDistOrigs` (and the `MRIS_MP` overload, a failed
//! assertion: FreeSurfer never wrote it).
//!
//! FreeSurfer keeps each vertex's buffer in `mris->dist_storage[vno]` and points `v->dist` at
//! it, or at `NULL` once the distances are no longer valid (the buffer stays for the next
//! use). Here the buffer is in [`Vertex::dist`] while it is valid and in
//! `Mris::dist_storage[vno]` while it is not, never in both: the same values are reachable the
//! same way. A buffer's length is what C allocated; memory `realloc` adds is 0 here where C
//! leaves it undefined (FreeSurfer zeroes what it reads of it, but for a vertex whose `vtotal`
//! outgrew its buffer: `docs/findings/freesurfer-surfio.md` §8).

use std::io::Write;

use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;

use crate::mris::{Mris, Vertex};
use crate::topology::vertex_vsize;

fn base_cpp() -> String {
    format!("{}/utils/mrisurf_base.cpp", diag::BUILD_ROOT)
}

fn field(v: &mut Vertex, do_orig: bool) -> (&mut Option<Vec<f32>>, &mut i32) {
    if do_orig {
        (&mut v.dist_orig, &mut v.dist_orig_capacity)
    } else {
        (&mut v.dist, &mut v.dist_capacity)
    }
}

fn storage(mris: &mut Mris, do_orig: bool) -> &mut Vec<Option<Vec<f32>>> {
    let s = if do_orig {
        &mut mris.dist_orig_storage
    } else {
        &mut mris.dist_storage
    };
    s.as_mut().expect("MRIS::dist_storage is NULL")
}

/// `freeDistOrDistOrig(doOrig, v)`: the vertex's distances (or original distances) become
/// `NULL` with capacity 0; the buffer stays in the surface's storage.
fn free_dist_or_dist_orig(mris: &mut Mris, do_orig: bool, vno: usize) {
    let buf = {
        let (d, cap) = field(mris.vertex_mut(vno), do_orig);
        *cap = 0;
        d.take()
    };
    if let Some(b) = buf {
        storage(mris, do_orig)[vno] = Some(b);
    }
}

/// `changeDistOrDistOrig(doOrig, mris, vno, oldSize, neededCapacity)`: a capacity below
/// `needed` reallocates the stored buffer to `needed` (its values kept up to the shorter
/// length), sets the surface's "allocated" flag (bit `1 << doOrig`) when the field was `NULL`,
/// and makes it the field with that capacity; then the values from `old_size` to `needed` are
/// zeroed.
fn change_dist_or_dist_orig(
    mris: &mut Mris,
    do_orig: bool,
    vno: usize,
    old_size: i32,
    needed: i32,
) {
    let cap = *field(mris.vertex_mut(vno), do_orig).1;
    if cap < needed {
        let was_null = field(mris.vertex_mut(vno), do_orig).0.is_none();
        let mut buf = match field(mris.vertex_mut(vno), do_orig).0.take() {
            Some(b) => b,
            None => storage(mris, do_orig)[vno].take().unwrap_or_default(),
        };
        buf.resize(needed as usize, 0.0);
        if was_null {
            mris.dist_alloced_flags |= 1i8 << i8::from(do_orig);
        }
        let (d, c) = field(mris.vertex_mut(vno), do_orig);
        *d = Some(buf);
        *c = needed;
    }
    if needed > old_size {
        let (d, _) = field(mris.vertex_mut(vno), do_orig);
        let d = d.as_mut().expect("VERTEX::dist is NULL");
        for x in &mut d[old_size.max(0) as usize..needed as usize] {
            *x = 0.0;
        }
    }
}

/// `mrisStealDistStore(mris, vno, capacity)`: a buffer of `capacity` floats for the caller to
/// fill: the stored one when the vertex's distances are `NULL` (its values kept up to the
/// shorter length), else a new one.
pub fn steal_dist_store(mris: &mut Mris, vno: usize, capacity: i32) -> Vec<f32> {
    let mut p = if mris.vertex(vno).dist.is_none() {
        storage(mris, false)[vno].take().unwrap_or_default()
    } else {
        Vec::new()
    };
    p.resize(capacity.max(0) as usize, 0.0);
    p
}

/// `mrisSetDist(mris, vno, dist, newCapacity)`: `dist` becomes the vertex's distances with
/// that capacity (the stored buffer freed) and the "allocated" flag's bit 1 is set. A capacity
/// below the vertex's present one fails `cheapAssert(newCapacity >= v->dist_capacity)` (line
/// 334), which ends the program (FS-03).
pub fn set_dist(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &mut Mris,
    vno: usize,
    dist: Vec<f32>,
    new_capacity: i32,
) -> Result<(), Fatal> {
    mris.dist_alloced_flags |= 1;
    if new_capacity < mris.vertex(vno).dist_capacity {
        return Err(diag::assert_failed(
            g,
            out,
            err,
            &base_cpp(),
            334,
            "newCapacity >= v->dist_capacity",
        ));
    }
    storage(mris, false)[vno] = None;
    let v = mris.vertex_mut(vno);
    v.dist = Some(dist);
    v.dist_capacity = new_capacity;
    Ok(())
}

fn grow(mris: &mut Mris, do_orig: bool, vno: usize, minimum_capacity: i32) {
    let old_size = if do_orig {
        mris.vertex(vno).dist_orig_capacity
    } else {
        i32::from(mris.topology(vno).vtotal)
    };
    let vsize = vertex_vsize(mris, vno).max(minimum_capacity);
    change_dist_or_dist_orig(mris, do_orig, vno, old_size, vsize);
}

/// `MRISgrowDist(mris, vno, minimumCapacity)`: room for at least the larger of the vertex's
/// size in use ([`vertex_vsize`]) and `minimum_capacity`; the values from `vtotal` on zeroed.
pub fn grow_dist(mris: &mut Mris, vno: usize, minimum_capacity: i32) {
    grow(mris, false, vno, minimum_capacity);
}

/// `MRISgrowDistOrig(mris, vno, minimumCapacity)`: as [`grow_dist`] for the original
/// distances, the values from their old capacity on zeroed.
pub fn grow_dist_orig(mris: &mut Mris, vno: usize, minimum_capacity: i32) {
    grow(mris, true, vno, minimum_capacity);
}

/// `MRISmakeDist(mris, vno)`: zeroed distances of the vertex's size in use.
pub fn make_dist(mris: &mut Mris, vno: usize) {
    let vsize = vertex_vsize(mris, vno);
    change_dist_or_dist_orig(mris, false, vno, 0, vsize);
}

/// `MRISmakeDistOrig(mris, vno)`: zeroed original distances of the vertex's size in use.
pub fn make_dist_orig(mris: &mut Mris, vno: usize) {
    let vsize = vertex_vsize(mris, vno);
    change_dist_or_dist_orig(mris, true, vno, 0, vsize);
}

fn free_dists_or_dist_origs(mris: &mut Mris, do_orig: bool) {
    let flag = 1i8 << i8::from(do_orig);
    if mris.dist_alloced_flags & flag == 0 {
        return;
    }
    for vno in 0..usize::try_from(mris.nvertices).unwrap_or(0) {
        free_dist_or_dist_orig(mris, do_orig, vno);
    }
    mris.dist_alloced_flags ^= flag;
}

/// `MRISfreeDistsButNotOrig(mris)`: `dist_nsize` 0; when the "allocated" flag's bit 1 is set,
/// every vertex's distances become `NULL` with capacity 0 and the bit is cleared.
pub fn free_dists_but_not_orig(mris: &mut Mris) {
    mris.dist_nsize = 0;
    free_dists_or_dist_origs(mris, false);
}

/// `MRISfreeDistOrigs(mris)`: as [`free_dists_but_not_orig`] for the original distances (bit 2),
/// `dist_orig_nsize` untouched.
pub fn free_dist_origs(mris: &mut Mris) {
    free_dists_or_dist_origs(mris, true);
}

/// `MRISfreeDistsButNotOrig(MRIS_MP*)`: never written in FreeSurfer 8.2.0, a failed
/// `cheapAssert(!"MRISfreeDistsButNotOrig(MRIS_MP* mris) NYI")` (line 414) (FS-03).
pub fn free_dists_but_not_orig_mp(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Fatal {
    diag::assert_failed(
        g,
        out,
        err,
        &base_cpp(),
        414,
        "!\"MRISfreeDistsButNotOrig(MRIS_MP* mris) NYI\"",
    )
}
