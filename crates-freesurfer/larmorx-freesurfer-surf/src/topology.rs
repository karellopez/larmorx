// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The neighbour-count helpers every surface function uses (`fs.surfio`, queue items 3.1-45
//! and 3.1-46), translated from FreeSurfer 8.2.0's `include/mrisurf.h` (`setVnum`, `clearVnum`,
//! `vnumAdd`, `addVnum`, `VERTEXvnum`, `mrisVertexNeighborIndex`, `mrisVerticesAreNeighbors`,
//! `returnTrue`), `include/mrisurf_topology.h` (`vertexNeighbor`) and the function they call,
//! `modVnum` (`utils/mrisurf_topology.cpp` lines 360-372), and `mrisVertexVSize` (lines 907-918),
//! which the vertex hash uses.

use std::io::Write;

use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, Fatal};

use crate::aaa::SurfGlobals;
use crate::mris::{Mris, VertexTopology};

fn mrisurf_h() -> String {
    format!("{}/include/mrisurf.h", diag::BUILD_ROOT)
}

/// `modVnum(mris, vno, add, clear)`: `vnum` of the vertex set to 0 first when `clear`, then
/// `vnum += add` (in `short`, wrapping as C's conversion back to `short` does on x86-64); when it
/// passes the largest value seen so far (`static short maxVnumSeen`, 30 at the start), that
/// becomes the largest and the debug message `modVnum: vertex <vno> has <vnum> immediate
/// neighbours` is written (shown with `DIAG_VERBOSE`). Returns the new `vnum`.
pub fn mod_vnum(
    g: &Globals,
    sg: &mut SurfGlobals,
    err: &mut dyn Write,
    mris: &mut Mris,
    vno: usize,
    add: i16,
    clear: bool,
) -> i16 {
    let vt = mris.topology_mut(vno);
    if clear {
        vt.vnum = 0;
    }
    vt.vnum = (i32::from(vt.vnum) + i32::from(add)) as i16;
    let p = vt.vnum;
    if p > sg.max_vnum_seen {
        sg.max_vnum_seen = p;
        log::debug(
            g,
            err,
            &format!("modVnum: vertex {vno} has {p} immediate neighbours"),
        );
    }
    p
}

/// `setVnum(mris, vno, to)`: `modVnum(mris, vno, to, true)`.
pub fn set_vnum(
    g: &Globals,
    sg: &mut SurfGlobals,
    err: &mut dyn Write,
    mris: &mut Mris,
    vno: usize,
    to: i16,
) -> i16 {
    mod_vnum(g, sg, err, mris, vno, to, true)
}

/// `clearVnum(mris, vno)`: `modVnum(mris, vno, 0, true)`.
pub fn clear_vnum(
    g: &Globals,
    sg: &mut SurfGlobals,
    err: &mut dyn Write,
    mris: &mut Mris,
    vno: usize,
) -> i16 {
    mod_vnum(g, sg, err, mris, vno, 0, true)
}

/// `vnumAdd(mris, vno, add = +1)`: `modVnum(mris, vno, add, false) - add`, the value before.
pub fn vnum_add(
    g: &Globals,
    sg: &mut SurfGlobals,
    err: &mut dyn Write,
    mris: &mut Mris,
    vno: usize,
    add: i16,
) -> i16 {
    let r = mod_vnum(g, sg, err, mris, vno, add, false);
    (i32::from(r) - i32::from(add)) as i16
}

/// `addVnum(mris, vno, add = -1)`: `modVnum(mris, vno, add, false)`.
pub fn add_vnum(
    g: &Globals,
    sg: &mut SurfGlobals,
    err: &mut dyn Write,
    mris: &mut Mris,
    vno: usize,
    add: i16,
) -> i16 {
    mod_vnum(g, sg, err, mris, vno, add, false)
}

/// `VERTEXvnum(v, i)`: `vnum`, `v2num` or `v3num` for `i` = 1, 2, 3; any other `i` fails
/// `cheapAssert(false)` (`include/mrisurf.h`, line 2512), which ends the program (FS-03).
pub fn vertex_vnum(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    v: &VertexTopology,
    i: i32,
) -> Result<i16, Fatal> {
    match i {
        1 => Ok(v.vnum),
        2 => Ok(v.v2num),
        3 => Ok(v.v3num),
        _ => Err(diag::assert_failed(
            g,
            out,
            err,
            &mrisurf_h(),
            2512,
            "false",
        )),
    }
}

/// `mrisVertexNeighborIndex(mris, vno1, vno2)`: the position of `vno2` among the first `vnum`
/// neighbours of `vno1`, or -1; -1 when the vertex has no neighbour array ("happens in invalid
/// surfaces"). `vno1` outside `[0, nvertices)` fails `cheapAssert(0 <= vno1 && vno1 <
/// mris->nvertices)` (line 2517).
pub fn vertex_neighbor_index(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &Mris,
    vno1: i32,
    vno2: i32,
) -> Result<i32, Fatal> {
    if !(0 <= vno1 && vno1 < mris.nvertices) {
        return Err(diag::assert_failed(
            g,
            out,
            err,
            &mrisurf_h(),
            2517,
            "0 <= vno1 && vno1 < mris->nvertices",
        ));
    }
    let vt = mris.topology(vno1 as usize);
    let Some(v) = &vt.v else {
        return Ok(-1);
    };
    for n in 0..i32::from(vt.vnum).max(0) {
        if v[n as usize] == vno2 {
            return Ok(n);
        }
    }
    Ok(-1)
}

/// `mrisVerticesAreNeighbors(mris, vno1, vno2)`: [`vertex_neighbor_index`] is not negative.
pub fn vertices_are_neighbors(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &Mris,
    vno1: i32,
    vno2: i32,
) -> Result<bool, Fatal> {
    Ok(vertex_neighbor_index(g, out, err, mris, vno1, vno2)? >= 0)
}

/// `vertexNeighbor(mris, vno1, vno2)` (`include/mrisurf_topology.h`): 1 when `vno2` is among the
/// first `vnum` neighbours of `vno1`, else 0 (no check of `vno1` or of a missing array: FreeSurfer
/// reads through it; here a panic).
pub fn vertex_neighbor(mris: &Mris, vno1: usize, vno2: i32) -> i32 {
    let vt = mris.topology(vno1);
    let n = usize::try_from(vt.vnum).unwrap_or(0);
    if n == 0 {
        return 0;
    }
    let v = vt.v.as_ref().expect("VERTEX_TOPOLOGY::v is NULL");
    i32::from(v[..n].contains(&vno2))
}

/// `mrisVertexVSize(mris, vno)`: the size of the vertex's neighbour and distance arrays in use:
/// the count at `nsizeMax` hops (`vnum`, `v2num`, `v3num` for 1, 2, 3; 0 for any other), or
/// `vtotal` when that is larger.
pub fn vertex_vsize(mris: &Mris, vno: usize) -> i32 {
    let v = mris.topology(vno);
    let c = match v.nsizeMax {
        1 => v.vnum,
        2 => v.v2num,
        3 => v.v3num,
        _ => 0,
    };
    i32::from(c.max(v.vtotal))
}

/// `returnTrue()`: what `mrisCheckVertexFaceTopology` and `mrisCheckVertexVertexTopology`
/// expand to (their checks are switched off in the source).
pub fn return_true() -> bool {
    true
}
