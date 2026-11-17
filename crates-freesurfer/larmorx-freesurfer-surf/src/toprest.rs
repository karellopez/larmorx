// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surf's `utils/mrisurf_topology.cpp`, the rest of the file (queue item 3.4-03, taken early;
//! relay 42 of track D): the edge and link edits (`edgeExists` line 320, `mrisCountValidLinks` 336,
//! `mrisAddEdge` 407, `mrisRemoveEdge` 414, `mrisRemoveVertexLink` 434, `MRISremoveTriangleLinks`
//! 454, `mrisRemoveLink` 2162, `mrisRemoveFace` 2698), the counts (`mrisCountTotalNeighbors` 509,
//! `MRIScomputeEulerNumber` 528, `MRIStopologicalDefectIndex` 562, `MRIScountTotalNeighbors` 921,
//! `MRISgetNeighborsBeginEnd` 995), the neighbourhood resets (`mrisStoreVtotalInV3num` 836,
//! `mrisVertexReplacingNeighbors` 878, `mrisForgetNeighborhoods` 898), the checks
//! (`MRISisSurfaceValid` 1688, `vertexInFace` 1957), the removal of ripped vertices and faces
//! (`removeRippedFaces` 2726, `removeRippedVertices` 2780,
//! `MRISrenumberRemovingRippedFacesAndVertices` 2857, `MRISremoveRipped` 2874), `MRISevertSurface`
//! (3143) and the spatially ordered neighbours (`MRISgetSpatOrderedNeighbors` 3355,
//! `MRISisNextSpatOrderedNeighbor` 3396). Not here: `mrisDivideEdgeTopologically`,
//! `mrisDivideFace` and `mrisInitializeNeighborhood` (`mris_fix_topology`'s, with fs.defect's
//! item) and `MRISfromVerticesAndFaces` (FreeSurfer's Python bindings only).
//!
//! Defects (`larmorx_core::defects`):
//! - **FS-986** (wrong result): `MRISremoveRipped` never updates the neighbour lists of the
//!   unripped vertices: its growth of the vertices to recompute skips every vertex already on its
//!   list, which is every vertex it walks, so no vertex is recomputed and the unripped vertices keep
//!   the ripped ones (and their distances) among their neighbours. larmorx grows the list as the
//!   function intends (each walked vertex's unlisted 1-hop neighbours, one ring per hop to the
//!   largest neighbourhood size plus one) and recomputes those vertices' rings without the ripped
//!   vertices (fs.surf's neighbour search, its sampled neighbours beyond the rings kept but the
//!   ripped ones, their distances carried by neighbour); `reproduce` FreeSurfer's.
//! - **FS-987** (wrong result): renumbering a surface without its ripped faces
//!   (`MRISrenumberRemovingRippedFacesAndVertices`) compacts each vertex's face list but not its list
//!   of places in those faces: after a removed face, each kept face is paired with the place of the
//!   face before it. larmorx compacts both; `reproduce` FreeSurfer's.
//! - **FS-988** (wrong result): the renumbering moves each kept vertex's data to its new number
//!   but not its neighbour and face lists: a moved vertex gets the lists of the vertex that had its
//!   new number before, filtered by the moved vertices' rip flags; a neighbour ripped with a flag
//!   other than 1 is kept, as -1. larmorx moves the lists with the vertex and drops every ripped
//!   neighbour by its own flag; `reproduce` FreeSurfer's.
//! - **FS-989** (kept, rule C): removing an edge or a link (`mrisRemoveEdge`,
//!   `mrisRemoveVertexLink`, `MRISremoveTriangleLinks`) leaves the 2- and 3-hop neighbour counts as
//!   they were (`mrisRemoveEdge` also the count in use, its last 1-hop neighbour then repeated
//!   after the ring); FreeSurfer's programs remove the triangle links of quadrangle surfaces at one
//!   hop, where only the 1-hop counts are read.
//! - **FS-990** (kept, rule D: to be measured at the programs that tear surfaces): `mrisRemoveLink`
//!   rips the faces that hold both vertices but removes no other link of them (its call of
//!   `mrisRemoveFace` waits for a rip flag of 2 or more, which it never sets), and `mrisRemoveFace`
//!   (`mrisCountValidLinks`) counts a link only in its own direction, so it removes a link that a
//!   face of the other direction still holds.
//! - **FS-991** (memory never written / crash): `MRISisSurfaceValid` starts each vertex's corner
//!   check from the first entry of its neighbour list, also for a vertex without neighbours: with
//!   an empty list it marks the vertex the memory there names, without a list it crashes. Both
//!   modes skip the mark of an empty list (noted); `reproduce` ends the program (signal 11) without
//!   a list; a list that still holds an entry (a vertex whose neighbours were removed) is used as
//!   FreeSurfer uses it.
//! - **FS-992** (crash): `mrisRemoveVertexLink` of a vertex whose neighbours in use are fewer than
//!   the place of the neighbour removed (a neighbourhood size of 0) moves a negative count of
//!   entries: the program crashes. `reproduce` ends it so (signal 11); the default removes the
//!   neighbour from the 1-hop ring only.
//! - **FS-993** (memory outside): `MRISgetSpatOrderedNeighbors` of a vertex without faces reads its
//!   face list's first entry from memory never written, and of a vertex its first face does not
//!   hold reads past that face's three vertices. Both modes return the neighbours found up to there
//!   (none for a vertex without faces), noted.
//! - **FS-995** (out of bounds): the renumbering of a surface whose unripped faces hold ripped
//!   vertices, or whose vertices have neighbours ripped with a flag other than 1, writes the vertex
//!   number -1 into those faces and lists; FreeSurfer's later reads of them go outside the
//!   surface's vertices. `reproduce` writes -1 (noted; the probe compares nothing after it); the
//!   default rips those faces first, so they are removed with the ripped ones (and the lists drop
//!   every ripped neighbour, FS-988).
//! - **FS-994** (memory): the renumbering copies each kept face (its normal matrices) and vertex
//!   (its user data) to its new place without moving them, so two places share them and releasing
//!   the surface releases them twice. larmorx's copies are its own; nothing is reproduced.

use std::io::Write;

use larmorx_core::defects::{
    FS_986, FS_987, FS_988, FS_989, FS_990, FS_991, FS_992, FS_993, FS_994, FS_995,
};
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_UNSUPPORTED, Fatal, NO_ERROR};

use crate::aaa::{SurfGlobals, VERTICES_PER_FACE, which_face_split};
use crate::alloc;
use crate::coords::{Io, set_vertex_face_index};
use crate::dist;
use crate::mris::Mris;
use crate::names::MRIS_BINARY_QUADRANGLE_FILE;
use crate::nbhd::{self, MAX_NEIGHBORS, NbrLists, add_edge_wkr, set_nsize_cur, topology_cpp};
use crate::surfread::verbose_show;
use crate::topology::{add_vnum, clear_vnum, mod_vnum, vertex_vnum};

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

fn nfaces(mris: &Mris) -> usize {
    usize::try_from(mris.nfaces).unwrap_or(0)
}

fn assert_at(g: &mut Globals, io: &mut Io, line: i32, tst: &str) -> Fatal {
    diag::assert_failed(g, io.out, io.err, &topology_cpp(), line, tst)
}

/// `vt->v[i]` (a missing list or an entry past it is FreeSurfer's invalid read: a panic).
fn v_of(mris: &Mris, vno: usize, i: usize) -> i32 {
    mris.topology(vno).v.as_ref().expect("VERTEX_TOPOLOGY::v is NULL")[i]
}

/// `vt->f[i]`.
fn f_of(mris: &Mris, vno: usize, i: usize) -> i32 {
    mris.topology(vno).f.as_ref().expect("VERTEX_TOPOLOGY::f is NULL")[i]
}

/// `EVEN(n)` (`macros.h`): `((n / 2) * 2) == n`, C's truncating division.
fn even(n: i32) -> bool {
    (n / 2) * 2 == n
}

// ---- the edges and links ---------------------------------------------------------------------

/// `edgeExists(mris, vno1, vno2)`: 1 when `vno2` is among the 1-hop neighbours of `vno1`, else 0.
pub fn edge_exists(mris: &Mris, vno1: usize, vno2: i32) -> i32 {
    let vnum = usize::try_from(mris.topology(vno1).vnum).unwrap_or(0);
    i32::from((0..vnum).any(|n| v_of(mris, vno1, n) == vno2))
}

/// `mrisCountValidLinks(mris, vno1, vno2)`: the unripped faces of `vno1` in which `vno2` follows
/// `vno1` (the face's vertex after it, the first after the last), each place of `vno1` counted
/// (FS-990: the other direction is not counted).
pub fn count_valid_links(mris: &Mris, vno1: usize, vno2: i32) -> i32 {
    let num = usize::from(mris.topology(vno1).num);
    let mut nvalid = 0;
    for fno in 0..num {
        let face = mris.face(f_of(mris, vno1, fno) as usize);
        if face.ripflag != 0 {
            continue;
        }
        for vno in 0..VERTICES_PER_FACE {
            if face.v[vno] == vno1 as i32 {
                let next = if vno == VERTICES_PER_FACE - 1 {
                    face.v[0]
                } else {
                    face.v[vno + 1]
                };
                if next == vno2 {
                    nvalid += 1;
                }
            }
        }
    }
    nvalid
}

/// `mrisAddEdge(mris, vno1, vno2)`: [`add_edge_wkr`] (its check that the two are not neighbours
/// already is compiled out).
pub fn add_edge(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    vno1: usize,
    vno2: usize,
) {
    add_edge_wkr(g, sg, io, mris, vno1, vno2);
}

/// `mrisRemoveEdge(mris, vno1, vno2)`: the first `vno2` among the 1-hop neighbours of `vno1`
/// removed (the 1-hop count one less, the 1-hop neighbours after it moved one place down); the
/// neighbours in use and the 2- and 3-hop counts are left as they were (FS-989), so the last 1-hop
/// neighbour then also stands after the ring. Nothing when `vno2` is not a 1-hop neighbour.
pub fn remove_edge(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    err: &mut dyn Write,
    mris: &mut Mris,
    vno1: usize,
    vno2: i32,
) {
    let vnum = usize::try_from(mris.topology(vno1).vnum).unwrap_or(0);
    let Some(i) = (0..vnum).find(|&i| v_of(mris, vno1, i) == vno2) else {
        return;
    };
    add_vnum(g, sg, err, mris, vno1, -1);
    let vt = mris.topology_mut(vno1);
    let vnum = usize::try_from(vt.vnum).unwrap_or(0);
    if vt.vtotal > vt.vnum || vt.v2num > vt.vnum || vt.v3num > vt.vnum {
        g.notices.add(
            &FS_989,
            format!(
                "mrisRemoveEdge({vno1}, {vno2}): the counts beyond the 1-hop ring are left as they were (kept)"
            ),
        );
    }
    if i < vnum {
        let v = vt.v.as_mut().expect("VERTEX_TOPOLOGY::v is NULL");
        v.copy_within(i + 1..vnum + 1, i);
    }
}

/// `mrisRemoveVertexLink(mris, vno1, vno2)`: the first `vno2` among the 1-hop neighbours of `vno1`
/// removed from its neighbours in use (those after it moved one place down; the 1-hop count and
/// the count in use one less; the 2- and 3-hop counts left as they were, FS-989). FS-992: fewer
/// neighbours in use than the place after the one removed: FreeSurfer moves a negative count of
/// entries and crashes (`reproduce`: signal 11); the default moves the 1-hop ring's entries only.
/// `NO_ERROR`.
pub fn remove_vertex_link(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    err: &mut dyn Write,
    mris: &mut Mris,
    vno1: usize,
    vno2: i32,
) -> Result<i32, Fatal> {
    let vnum = usize::try_from(mris.topology(vno1).vnum).unwrap_or(0);
    let Some(n) = (0..vnum).find(|&n| v_of(mris, vno1, n) == vno2) else {
        return Ok(NO_ERROR);
    };
    let vtotal = i32::from(mris.topology(vno1).vtotal);
    let count = vtotal - (n as i32 + 1);
    if count < 0 {
        g.notices.add(
            &FS_992,
            format!(
                "mrisRemoveVertexLink({vno1}, {vno2}): {vtotal} neighbours in use below the place {n} removed (FreeSurfer moves {count} entries and crashes)"
            ),
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        let v = mris
            .topology_mut(vno1)
            .v
            .as_mut()
            .expect("VERTEX_TOPOLOGY::v is NULL");
        v.copy_within(n + 1..vnum, n);
        add_vnum(g, sg, err, mris, vno1, -1);
        return Ok(NO_ERROR);
    }
    {
        let vt = mris.topology(vno1);
        if vt.v2num > vt.vnum || vt.v3num > vt.vnum {
            g.notices.add(
                &FS_989,
                format!(
                    "mrisRemoveVertexLink({vno1}, {vno2}): the 2- and 3-hop counts are left as they were (kept)"
                ),
            );
        }
    }
    let count = count as usize;
    let v = mris
        .topology_mut(vno1)
        .v
        .as_mut()
        .expect("VERTEX_TOPOLOGY::v is NULL");
    v.copy_within(n + 1..n + 1 + count, n);
    add_vnum(g, sg, err, mris, vno1, -1);
    let vt = mris.topology_mut(vno1);
    vt.vtotal -= 1;
    Ok(NO_ERROR)
}

/// `MRISremoveTriangleLinks(mris)`: for a quadrangle surface (type `MRIS_BINARY_QUADRANGLE_FILE`)
/// whose triangle links are not yet removed (the surface then marked so), with `DIAG_SHOW` and
/// verbose diagnostics `removing non-quadrangular links.` on standard output; then for every
/// unripped face of an even number the link across its quadrangle removed both ways
/// ([`remove_vertex_link`]): its second and third vertices' when `WHICH_FACE_SPLIT` of its first two
/// is even, else its first and third. `NO_ERROR`.
pub fn remove_triangle_links(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<i32, Fatal> {
    if mris.r#type != MRIS_BINARY_QUADRANGLE_FILE {
        return Ok(NO_ERROR);
    }
    if mris.triangle_links_removed != 0 {
        return Ok(NO_ERROR);
    }
    mris.triangle_links_removed = 1;
    if verbose_show(g) {
        let _ = writeln!(io.out, "removing non-quadrangular links.");
    }
    let mut fno = 0usize;
    while (fno as i64) < i64::from(mris.nfaces) {
        let f = mris.face(fno).clone();
        fno += 2;
        if f.ripflag != 0 {
            continue;
        }
        let which = which_face_split(g, f.v[0], f.v[1]);
        let (a, b) = if even(which) {
            (f.v[1], f.v[2])
        } else {
            (f.v[0], f.v[2])
        };
        remove_vertex_link(g, sg, io.err, mris, a as usize, b)?;
        remove_vertex_link(g, sg, io.err, mris, b as usize, a)?;
    }
    Ok(NO_ERROR)
}

/// `mrisRemoveFace(mris, fno)`: the face ripped; each of its edges whose first vertex keeps no
/// unripped face in which the second follows it ([`count_valid_links`], FS-990) removed both ways
/// ([`remove_edge`]). `NO_ERROR`.
pub fn remove_face(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    err: &mut dyn Write,
    mris: &mut Mris,
    fno: usize,
) -> i32 {
    mris.face_mut(fno).ripflag = 1;
    for vno in 0..VERTICES_PER_FACE {
        let face = mris.face(fno);
        let vno1 = face.v[vno];
        let vno2 = face.v[if vno < VERTICES_PER_FACE - 1 { vno + 1 } else { 0 }];
        if count_valid_links(mris, vno1 as usize, vno2) == 0 {
            if count_valid_links(mris, vno2 as usize, vno1) != 0 {
                g.notices.add(
                    &FS_990,
                    format!(
                        "mrisRemoveFace({fno}): the link {vno1} <--> {vno2} that a face of the other direction holds is removed (kept)"
                    ),
                );
            }
            remove_edge(g, sg, err, mris, vno1 as usize, vno2);
            remove_edge(g, sg, err, mris, vno2 as usize, vno1);
        }
    }
    NO_ERROR
}

/// `mrisRemoveLink(mris, vno1, vno2)`: the edge removed both ways ([`remove_edge`]); every face
/// of `vno1` holding `vno2` ripped (flag 1), and every face of `vno1` whose flag is then 2 or more
/// removed ([`remove_face`]; FS-990: never one it ripped); then each of the two vertices ripped
/// when none of its faces is left unripped. `NO_ERROR`.
pub fn remove_link(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    err: &mut dyn Write,
    mris: &mut Mris,
    vno1: usize,
    vno2: usize,
) -> i32 {
    remove_edge(g, sg, err, mris, vno1, vno2 as i32);
    remove_edge(g, sg, err, mris, vno2, vno1 as i32);
    let mut ripped_here = Vec::new();
    let mut fno = 0usize;
    while fno < usize::from(mris.topology(vno1).num) {
        let f = f_of(mris, vno1, fno) as usize;
        if mris.face(f).v.contains(&(vno2 as i32)) {
            mris.face_mut(f).ripflag = 1;
            ripped_here.push(f);
        }
        if mris.face(f).ripflag >= 2 {
            remove_face(g, sg, err, mris, f);
        }
        fno += 1;
    }
    // a link of a face it ripped that no unripped face holds any more stays (FS-990)
    let held = |mris: &Mris, a: i32, b: i32| {
        let vt = mris.topology(a as usize);
        (0..usize::from(vt.num)).any(|k| {
            let f = mris.face(f_of(mris, a as usize, k) as usize);
            f.ripflag == 0 && f.v.contains(&b)
        })
    };
    let kept = ripped_here.iter().any(|&f| {
        let fv = mris.face(f).v;
        (0..VERTICES_PER_FACE).any(|k| {
            let (a, b) = (fv[k], fv[(k + 1) % VERTICES_PER_FACE]);
            let linked = edge_exists(mris, a as usize, b) != 0 || edge_exists(mris, b as usize, a) != 0;
            linked && !held(mris, a, b)
        })
    });
    if kept {
        g.notices.add(
            &FS_990,
            format!(
                "mrisRemoveLink({vno1}, {vno2}): the links only its ripped faces held are not removed (kept)"
            ),
        );
    }
    for vno in [vno1, vno2] {
        let num = usize::from(mris.topology(vno).num);
        let nvalid = (0..num)
            .filter(|&k| mris.face(f_of(mris, vno, k) as usize).ripflag == 0)
            .count();
        if nvalid == 0 {
            mris.vertex_mut(vno).ripflag = 1;
        }
    }
    NO_ERROR
}

// ---- the counts --------------------------------------------------------------------------------

/// `mrisCountTotalNeighbors(mris)`: over the unripped vertices, the neighbours in use and the
/// vertex itself.
pub fn count_total_neighbors_in_use(mris: &Mris) -> i32 {
    let mut total = 0i32;
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        total = total.wrapping_add(i32::from(mris.topology(vno).vtotal) + 1);
    }
    total
}

/// The Euler counts: `(euler, nvertices, nfaces, nedges)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EulerCounts {
    pub euler: i32,
    pub nvertices: i32,
    pub nfaces: i32,
    pub nedges: i32,
}

fn euler_counts(mris: &Mris) -> EulerCounts {
    let nf = (0..nfaces(mris))
        .filter(|&f| mris.face(f).ripflag == 0)
        .count() as i32;
    let nv = (0..nverts(mris))
        .filter(|&v| mris.vertex(v).ripflag == 0)
        .count() as i32;
    let mut ne = 0i32;
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let vnum = usize::try_from(mris.topology(vno).vnum).unwrap_or(0);
        for i in 0..vnum {
            let vnb = v_of(mris, vno, i);
            if vnb > vno as i32 && mris.vertex(vnb as usize).ripflag == 0 {
                ne += 1;
            }
        }
    }
    EulerCounts {
        euler: nv - ne + nf,
        nvertices: nv,
        nfaces: nf,
        nedges: ne,
    }
}

/// `MRIScomputeEulerNumber(mris, &nvertices, &nfaces, &nedges)`: the unripped vertices and faces
/// counted, and the edges between unripped vertices (each 1-hop neighbour of a larger number);
/// the Euler number `nvertices - nedges + nfaces`.
pub fn compute_euler_number(mris: &Mris) -> EulerCounts {
    euler_counts(mris)
}

/// `MRIStopologicalDefectIndex(mris)`: with [`compute_euler_number`]'s counts, `|2 - euler| +
/// |2 nedges - 3 nfaces|`.
pub fn topological_defect_index(mris: &Mris) -> i32 {
    let c = euler_counts(mris);
    (2 - c.euler).wrapping_abs() + (2 * c.nedges - 3 * c.nfaces).wrapping_abs()
}

/// `MRIScountTotalNeighbors(mris, nsize)`: over the unripped vertices, 1 each (`nsize` 0) or the
/// count of their `nsize`-hop neighbourhood (1, 2, 3). Each such vertex must know `nsize` hops
/// (else a failed assertion, line 931); any other `nsize` ends the program with `MRIScountNeighbors(
/// <nsize>): max nbhd size = 3` (`ERROR_UNSUPPORTED`).
pub fn count_total_neighbors(g: &mut Globals, io: &mut Io, mris: &Mris, nsize: i32) -> Result<i32, Fatal> {
    let mut total = 0i32;
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let vt = mris.topology(vno);
        if nsize > i32::from(vt.nsizeMax) {
            return Err(assert_at(g, io, 931, "nsize <= vt->nsizeMax"));
        }
        total = total.wrapping_add(match nsize {
            0 => 1,
            1 => i32::from(vt.vnum),
            2 => i32::from(vt.v2num),
            3 => i32::from(vt.v3num),
            _ => {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_UNSUPPORTED,
                    &format!("MRIScountNeighbors({nsize}): max nbhd size = 3"),
                )?;
                0
            }
        });
    }
    Ok(total)
}

/// `MRISgetNeighborsBeginEnd(mris, vno, inner, outer, &begin, &end)`: the places in the vertex's
/// neighbour list of the neighbours from `inner` to `outer` hops (`begin` 0, `vnum`, `v2num` for
/// 1, 2, 3; `end` `vnum`, `v2num`, `v3num`); `(0, 0)` when `inner > outer`. Unless `outer` is at
/// most 1, the vertex's rings must be the surface's current ones (its clock, else a failed
/// assertion, line 1009); another `inner` or `outer` fails one (lines 1015, 1021).
pub fn get_neighbors_begin_end(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    vno: usize,
    inner: usize,
    outer: usize,
) -> Result<(usize, usize), Fatal> {
    let vt = mris.topology(vno);
    if inner > outer {
        return Ok((0, 0));
    }
    if !(mris.nsizeMaxClock == vt.nsizeMaxClock || outer <= 1) {
        return Err(assert_at(
            g,
            io,
            1009,
            "mris->nsizeMaxClock == vt->nsizeMaxClock || outer_nbhd_size <= 1",
        ));
    }
    let to = |x: i16| x as i64 as usize;
    let b = match inner {
        1 => 0,
        2 => to(vt.vnum),
        3 => to(vt.v2num),
        _ => return Err(assert_at(g, io, 1015, "false")),
    };
    let e = match outer {
        1 => to(vt.vnum),
        2 => to(vt.v2num),
        3 => to(vt.v3num),
        _ => return Err(assert_at(g, io, 1021, "false")),
    };
    Ok((b, e))
}

// ---- the neighbourhood resets ------------------------------------------------------------------

/// `mrisStoreVtotalInV3num(mris)`: every vertex's 3-hop count set to its count in use. `NO_ERROR`.
pub fn store_vtotal_in_v3num(mris: &mut Mris) -> i32 {
    for vno in 0..nverts(mris) {
        let vt = mris.topology_mut(vno);
        vt.v3num = vt.vtotal;
    }
    NO_ERROR
}

/// `mrisVertexReplacingNeighbors(mris, vno, vnum)`: the surface's largest neighbourhood size and
/// the vertex's sizes 1 (the surface's size -1 unless it was 1: its vertices now differ); every
/// vertex's distances released (`MRISfreeDistsButNotOrig`); the vertex's neighbour list made
/// `vnum` long (its new places zeroed), its 1-hop count `vnum`, its 2- and 3-hop counts 0 and its
/// count in use `vnum`.
pub fn vertex_replacing_neighbors(
    g: &Globals,
    sg: &mut SurfGlobals,
    err: &mut dyn Write,
    mris: &mut Mris,
    vno: usize,
    vnum: i32,
) {
    {
        let vt = mris.topology_mut(vno);
        vt.nsizeMax = 1;
        vt.nsizeCur = 1;
    }
    mris.max_nsize = 1;
    if mris.nsize != 1 {
        mris.nsize = -1;
    }
    dist::free_dists_but_not_orig(mris);
    let old = i32::from(mris.topology(vno).vnum);
    nbhd::resize_vertex_v(mris, vno, vnum, old);
    mod_vnum(g, sg, err, mris, vno, vnum as i16, true);
    let vt = mris.topology_mut(vno);
    vt.v2num = 0;
    vt.v3num = 0;
    vt.vtotal = vt.vnum;
}

/// `mrisForgetNeighborhoods(mris)`: [`vertex_replacing_neighbors`] of every vertex with its own
/// 1-hop count; then the surface's neighbourhood size 1.
pub fn forget_neighborhoods(g: &Globals, sg: &mut SurfGlobals, err: &mut dyn Write, mris: &mut Mris) {
    for vno in 0..nverts(mris) {
        let vnum = i32::from(mris.topology(vno).vnum);
        vertex_replacing_neighbors(g, sg, err, mris, vno, vnum);
    }
    mris.nsize = 1;
}

// ---- the checks --------------------------------------------------------------------------------

/// `vertexInFace(mris, vno, fno)`: 1 when the face is among the vertex's faces, else 0.
pub fn vertex_in_face(mris: &Mris, vno: usize, fno: i32) -> i32 {
    let num = usize::from(mris.topology(vno).num);
    i32::from((0..num).any(|n| f_of(mris, vno, n) == fno))
}

/// `MRISisSurfaceValid(mris, patch, verbose)`: counts the vertices whose 1-hop count is not their
/// face count (border vertices), the edges (1-hop entries) in fewer than two of their first
/// vertex's faces and in more than two, and the vertices whose 1-hop neighbours are not all
/// reached from the first through its faces (corner configurations), every vertex and face counted
/// whatever its rip flag; the vertices' marks are left as the corner checks set them. With
/// `verbose` 2 the checks' headings and each finding on standard error (the border and single-face
/// findings only when `patch` is 0); with any `verbose` the summary. 0 when anything was found
/// (for a patch only edges of more than two faces and corners count), else 1. FS-991: a vertex
/// without neighbours starts its corner check from its empty list's first entry.
pub fn is_surface_valid(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    patch: i32,
    verbose: i32,
) -> Result<i32, Fatal> {
    let nv = nverts(mris);
    if verbose == 2 {
        let _ = write!(io.err, "\nchecking for border faces...\n");
    }
    let mut nvf = 0i32;
    for n in 0..nv {
        let vn = mris.topology(n);
        if i32::from(vn.vnum) != i32::from(vn.num) {
            nvf += 1;
            if verbose == 2 && patch == 0 {
                let _ = writeln!(io.err, "vertex {n} : vnum = {} num={}", vn.vnum, vn.num);
            }
        }
    }
    if verbose == 2 {
        let _ = write!(io.err, "\nchecking for single or multiple faces...\n");
    }
    let (mut nffm, mut nffs, mut nedges) = (0i32, 0i32, 0i32);
    for n in 0..nv {
        let vnum = i32::from(mris.topology(n).vnum);
        nedges = nedges.wrapping_add(vnum);
        let num = usize::from(mris.topology(n).num);
        for p in 0..vnum.max(0) as usize {
            let vnop = v_of(mris, n, p);
            let mut cf = 0;
            for m in 0..num {
                let fm = mris.face(f_of(mris, n, m) as usize);
                if fm.v.contains(&vnop) {
                    cf += 1;
                }
            }
            if cf < 2 {
                nffs += 1;
            }
            if cf > 2 {
                nffm += 1;
            }
            if verbose == 2 {
                if cf < 2 && patch == 0 {
                    let _ = writeln!(io.err, "edge {n} <--> {vnop} : single face");
                }
                if cf > 2 {
                    let _ = writeln!(io.err, "edge {n} <--> {vnop} : multiple face");
                }
            }
        }
    }
    nedges /= 2;
    let euler = mris.nvertices - nedges + mris.nfaces;
    if verbose == 2 {
        let _ = write!(io.err, "\nchecking for corner configurations...\n");
    }
    let mut nvc = 0i32;
    for n in 0..nv {
        let vnum = usize::try_from(mris.topology(n).vnum).unwrap_or(0);
        for p in 0..vnum {
            let vnop = v_of(mris, n, p);
            mris.vertex_mut(vnop as usize).marked = 0;
        }
        let first = match mris.topology(n).v.as_deref() {
            Some(v) if !v.is_empty() => Some(v[0]),
            Some(_) => {
                g.notices.add(
                    &FS_991,
                    format!(
                        "MRISisSurfaceValid: vertex {n} has no neighbours: FreeSurfer marks the vertex its empty list's first entry names (memory never written): no mark"
                    ),
                );
                None
            }
            None => {
                g.notices.add(
                    &FS_991,
                    format!(
                        "MRISisSurfaceValid: vertex {n} has no neighbour list: FreeSurfer reads its first entry and crashes"
                    ),
                );
                if g.notices.reproduce() {
                    return Err(Fatal::Exit(139));
                }
                None
            }
        };
        if let Some(vnop) = first {
            mris.vertex_mut(vnop as usize).marked = 1;
        }
        let num = usize::from(mris.topology(n).num);
        let mut nfound = true;
        while nfound {
            nfound = false;
            for m in 0..num {
                let fv = mris.face(f_of(mris, n, m) as usize).v;
                let mark = fv
                    .iter()
                    .any(|&q| q != n as i32 && mris.vertex(q as usize).marked != 0);
                if mark {
                    for &q in &fv {
                        if q == n as i32 || mris.vertex(q as usize).marked != 0 {
                            continue;
                        }
                        mris.vertex_mut(q as usize).marked = 1;
                        nfound = true;
                        break;
                    }
                }
            }
        }
        for p in 0..vnum {
            let vnop = v_of(mris, n, p);
            if mris.vertex(vnop as usize).marked == 0 {
                nvc += 1;
                if verbose == 2 {
                    let _ = writeln!(io.err, "vertex {n} : corner configuration");
                }
                break;
            }
        }
    }
    if verbose != 0 {
        let _ = write!(
            io.err,
            "Surface Diagnostics:       \n   eno={euler} (nv={}, nf={}, ne={nedges})\n   # of border vertices [ #v ~ #f ]     {nvf} \n   # of edges with single face          {nffs} \n   # of edges with more than 2 faces    {nffm} \n   # of corner configurations           {nvc}\n",
            mris.nvertices, mris.nfaces
        );
    }
    if patch != 0 && (nffm != 0 || nvc != 0) {
        return Ok(0);
    }
    if nvf != 0 || nffs != 0 || nffm != 0 || nvc != 0 {
        return Ok(0);
    }
    Ok(1)
}

// ---- removing the ripped vertices and faces ---------------------------------------------------

/// `removeRippedFaces(mris)`: the unripped faces (with their face-normal cache entries) moved down
/// over the ripped ones in order; each vertex's face list renumbered, its removed faces dropped;
/// the surface's face count the unripped ones (`MRISremovedFaces`). FS-987: FreeSurfer leaves each
/// vertex's places in its faces where they were (`reproduce`); larmorx moves them with the faces.
fn remove_ripped_faces(g: &mut Globals, io: &mut Io, mris: &mut Mris) -> Result<(), Fatal> {
    let nf = nfaces(mris);
    let mut out_faces = vec![0i32; nf];
    let mut nfaces_left = mris.nfaces;
    let mut out = 0usize;
    let mut shared = false;
    for fno in 0..nf {
        if mris.face(fno).ripflag != 0 {
            out_faces[fno] = -1;
            nfaces_left -= 1;
        } else {
            out_faces[fno] = out as i32;
            if fno != out {
                let f = mris.face(fno).clone();
                if f.norm.is_some() || f.gradNorm.iter().any(Option::is_some) {
                    shared = true;
                }
                *mris.face_mut(out) = f;
                if let Some(c) = mris.faceNormCacheEntries.as_mut() {
                    c[out] = c[fno];
                }
            }
            out += 1;
        }
    }
    if shared {
        g.notices.add(
            &FS_994,
            "MRISrenumberRemovingRippedFacesAndVertices: a moved face's normal matrices stay shared with its old place (FreeSurfer releases them twice): copied",
        );
    }
    if nfaces_left != out as i32 {
        return Err(assert_at(g, io, 2756, "nfaces == out_fno"));
    }
    let reproduce = g.notices.reproduce();
    let mut misaligned = false;
    for vno in 0..nverts(mris) {
        let num = usize::from(mris.topology(vno).num);
        let mut k = 0usize;
        for n in 0..num {
            let fno = out_faces[f_of(mris, vno, n) as usize];
            if fno < 0 || mris.face(fno as usize).ripflag == 1 {
                continue;
            }
            let vt = mris.topology_mut(vno);
            vt.f.as_mut().expect("VERTEX_TOPOLOGY::f is NULL")[k] = fno;
            if k != n {
                let nn = vt.n.as_mut().expect("VERTEX_TOPOLOGY::n is NULL");
                if nn[k] != nn[n] {
                    misaligned = true;
                }
                if !reproduce {
                    nn[k] = nn[n];
                }
            }
            k += 1;
        }
        mris.topology_mut(vno).num = k as u8;
    }
    if misaligned {
        g.notices.add(
            &FS_987,
            "MRISrenumberRemovingRippedFacesAndVertices: the vertices' places in their faces are not moved with the faces (FreeSurfer pairs a kept face with the place of the face before it)",
        );
    }
    alloc::removed_faces(g, io.out, io.err, mris, nfaces_left)
}

/// `removeRippedVertices(mris)`: the unripped vertices moved down over the ripped ones in order;
/// each kept vertex's neighbour list filtered (its ripped neighbours dropped, the others
/// renumbered; its 1- and 2-hop counts those kept among the first `vnum` and `v2num`, its 3-hop
/// count all kept, its largest neighbourhood its current one, its count in use the surface's size
/// (1, 2 or 3; 1 for any other)); every face's vertices renumbered (a ripped one -1); the surface's
/// vertex count the unripped ones (`MRISremovedVertices`). Each vertex must know the surface's
/// neighbourhood size (else a failed assertion, line 2829). FS-988: FreeSurfer moves the vertices'
/// data but not their neighbour and face lists, and tests each neighbour by the rip flag of the
/// vertex that has its number after the move, dropping only flags of 1 (`reproduce`); larmorx moves
/// the lists with the vertices and drops every ripped neighbour by its own flag.
fn remove_ripped_vertices(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<(), Fatal> {
    let nv = nverts(mris);
    let reproduce = g.notices.reproduce();
    let old_rip: Vec<i8> = (0..nv).map(|v| mris.vertex(v).ripflag).collect();
    let mut out_vnos = vec![0i32; nv];
    let mut nvertices = mris.nvertices;
    let mut out = 0usize;
    let mut moved = false;
    let mut shared = false;
    for vno in 0..nv {
        if mris.vertex(vno).ripflag != 0 {
            nvertices -= 1;
            out_vnos[vno] = -1;
        } else {
            out_vnos[vno] = out as i32;
            if vno != out {
                moved = true;
                let v = mris.vertex(vno).clone();
                if v.vp.0.is_some() {
                    shared = true;
                }
                *mris.vertex_mut(out) = v;
                if !reproduce {
                    let t = mris.topology(vno).clone();
                    *mris.topology_mut(out) = t;
                }
            }
            out += 1;
        }
    }
    if moved {
        g.notices.add(
            &FS_988,
            "MRISrenumberRemovingRippedFacesAndVertices: the moved vertices' neighbour and face lists are not moved with them (FreeSurfer gives a moved vertex the lists of the vertex that had its number)",
        );
    }
    if shared {
        g.notices.add(
            &FS_994,
            "MRISrenumberRemovingRippedFacesAndVertices: a moved vertex's user data stays shared with its old place (FreeSurfer releases it twice): released once",
        );
    }
    let nkept = usize::try_from(nvertices).unwrap_or(0);
    for vno in 0..nkept {
        let (vnum, v2num, vtotal) = {
            let vt = mris.topology(vno);
            (
                i32::from(vt.vnum),
                i32::from(vt.v2num),
                usize::try_from(vt.vtotal).unwrap_or(0),
            )
        };
        {
            let vt = mris.topology_mut(vno);
            vt.v3num = 0;
            vt.v2num = 0;
        }
        clear_vnum(g, sg, io.err, mris, vno);
        for n in 0..vtotal {
            let vno2 = v_of(mris, vno, n);
            let ripped = if vno2 < 0 {
                true
            } else if reproduce {
                mris.vertex(vno2 as usize).ripflag == 1
            } else {
                old_rip[vno2 as usize] != 0
            };
            if ripped {
                continue;
            }
            if reproduce && old_rip[vno2 as usize] != 0 {
                g.notices.add(
                    &FS_988,
                    format!(
                        "MRISrenumberRemovingRippedFacesAndVertices: vertex {vno} keeps its ripped neighbour {vno2} (as {})",
                        out_vnos[vno2 as usize]
                    ),
                );
            }
            let mapped = out_vnos[vno2 as usize];
            if mapped < 0 {
                g.notices.add(
                    &FS_995,
                    format!(
                        "MRISrenumberRemovingRippedFacesAndVertices: vertex {vno}'s neighbour {vno2} ripped with flag {} kept as -1 (FreeSurfer's later reads of it go outside the vertices)",
                        old_rip[vno2 as usize]
                    ),
                );
            }
            let vt = mris.topology_mut(vno);
            let k = usize::try_from(vt.v3num).unwrap_or(0);
            vt.v.as_mut().expect("VERTEX_TOPOLOGY::v is NULL")[k] = mapped;
            vt.v3num += 1;
            if (n as i32) < v2num {
                vt.v2num += 1;
            }
            if (n as i32) < vnum {
                add_vnum(g, sg, io.err, mris, vno, 1);
            }
        }
        {
            let vt = mris.topology_mut(vno);
            vt.nsizeMax = vt.nsizeCur;
        }
        if i32::from(mris.nsize) > i32::from(mris.topology(vno).nsizeMax) {
            return Err(assert_at(g, io, 2829, "mris->nsize <= v->nsizeMax"));
        }
        let nsize = mris.nsize;
        let vt = mris.topology_mut(vno);
        match nsize {
            2 => {
                vt.vtotal = vt.v2num;
                vt.nsizeCur = 2;
            }
            3 => {
                vt.vtotal = vt.v3num;
                vt.nsizeCur = 3;
            }
            _ => {
                vt.vtotal = vt.vnum;
                vt.nsizeCur = 1;
            }
        }
    }
    let mut minus = false;
    for fno in 0..nfaces(mris) {
        let f = mris.face_mut(fno);
        for n in 0..VERTICES_PER_FACE {
            f.v[n] = out_vnos[f.v[n] as usize];
            minus |= f.v[n] < 0;
        }
    }
    if minus {
        g.notices.add(
            &FS_995,
            "MRISrenumberRemovingRippedFacesAndVertices: a kept face holds a ripped vertex, now -1 (FreeSurfer's later reads of it go outside the vertices)",
        );
    }
    alloc::removed_vertices(g, io.out, io.err, mris, nvertices)
}

/// `MRISrenumberRemovingRippedFacesAndVertices(mris)`: the surface must have no distances
/// allocated (else a failed assertion, line 2859); then the ripped faces removed, then the ripped
/// vertices (FS-987, FS-988, FS-994).
pub fn renumber_removing_ripped_faces_and_vertices(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<(), Fatal> {
    if mris.dist_alloced_flags != 0 {
        return Err(assert_at(g, io, 2859, "!mris->dist_alloced_flags"));
    }
    if !g.notices.reproduce() {
        let mut ripped = 0;
        for fno in 0..nfaces(mris) {
            let fv = mris.face(fno).v;
            if mris.face(fno).ripflag == 0
                && fv.iter().any(|&x| mris.vertex(x as usize).ripflag != 0)
            {
                mris.face_mut(fno).ripflag = 1;
                ripped += 1;
            }
        }
        if ripped > 0 {
            g.notices.add(
                &FS_995,
                format!(
                    "MRISrenumberRemovingRippedFacesAndVertices: {ripped} unripped faces hold ripped vertices: removed with the ripped faces (FreeSurfer keeps them with the vertex number -1)"
                ),
            );
        }
    }
    remove_ripped_faces(g, io, mris)?;
    remove_ripped_vertices(g, sg, io, mris)
}

/// `MRISremoveRipped(mris)`: every face of a ripped vertex ripped (each ripped vertex's count in
/// use must be that of its current neighbourhood size, else a failed assertion, line 2905, or
/// `VERTEXvnum`'s); the ripped faces' vertices set to 0 and each vertex of a ripped face, ripped or
/// not, listed; each listed unripped vertex's face list without its ripped faces (its places with
/// them). FS-986: the unripped vertices near the ripped ones are meant to have their rings found
/// again without them (their sampled neighbours beyond the rings kept but the ripped ones, their
/// distances carried by neighbour), but FreeSurfer's growth of that list adds no vertex
/// (`reproduce`); larmorx grows it, one ring per hop to the largest neighbourhood size of an
/// unripped vertex plus one, and finds those rings again (fs.surf's neighbour search).
pub fn remove_ripped(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<(), Fatal> {
    let nv = nverts(mris);
    let mut dist_cache = vec![0f32; nv];
    let mut dist_orig_cache = vec![0f32; nv];
    let mut affected = vec![0i32; nv];
    let mut head: i32 = 1;
    let mut size: i64 = 0;
    let mut largest: i32 = 0;
    for vno in 0..nv {
        if mris.vertex(vno).ripflag == 0 {
            largest = largest.max(i32::from(mris.topology(vno).nsizeMax));
            continue;
        }
        let cur = i32::from(mris.topology(vno).nsizeCur);
        let want = vertex_vnum(g, io.out, io.err, mris.topology(vno), cur)?;
        if mris.topology(vno).vtotal != want {
            return Err(assert_at(
                g,
                io,
                2905,
                "vt->vtotal == VERTEXvnum(vt, vt->nsizeCur)",
            ));
        }
        for n in 0..usize::from(mris.topology(vno).num) {
            let fno = f_of(mris, vno, n) as usize;
            mris.face_mut(fno).ripflag = 1;
        }
        affected[vno] = head;
        head = vno as i32 + 2;
        size += 1;
    }
    let start_of_ripped_tail = head;
    // the growth: FreeSurfer skips every walked vertex (each is on the list), so it adds none
    let reproduce = g.notices.reproduce();
    let mut end: i32 = 1;
    let mut would = false;
    for _ in 0..=largest {
        let start = head;
        let mut p = head;
        while p != end {
            let vno = (p - 2) as usize;
            let vnum = usize::try_from(mris.topology(vno).vnum).unwrap_or(0);
            for n in 0..vnum {
                let nbr = v_of(mris, vno, n) as usize;
                if affected[nbr] != 0 {
                    continue;
                }
                would = true;
                if reproduce {
                    continue;
                }
                affected[nbr] = head;
                head = nbr as i32 + 2;
                size += 1;
            }
            p = affected[vno];
        }
        end = start;
        if reproduce {
            break;
        }
    }
    if would {
        g.notices.add(
            &FS_986,
            "MRISremoveRipped: the unripped vertices next to the ripped ones keep them among their neighbours (FreeSurfer's growth of the vertices to recompute adds none)",
        );
    }
    let mut lists = NbrLists::default();
    let mut p = head;
    head = 1;
    while p != start_of_ripped_tail {
        let vno = (p - 2) as usize;
        let (old_max, old_cur, old_vtotal) = {
            let vt = mris.topology(vno);
            (
                i32::from(vt.nsizeMax),
                i32::from(vt.nsizeCur),
                usize::try_from(vt.vtotal).unwrap_or(0),
            )
        };
        let old_cached = usize::try_from(vertex_vnum(
            g,
            io.out,
            io.err,
            mris.topology(vno),
            old_max,
        )?)
        .unwrap_or(0);
        let tail: Vec<i32> = if old_vtotal > old_cached {
            (old_cached..old_vtotal)
                .map(|i| v_of(mris, vno, i))
                .collect()
        } else {
            Vec::new()
        };
        if mris.vertex(vno).dist.is_some() {
            for i in 0..old_vtotal {
                let n = v_of(mris, vno, i) as usize;
                dist_cache[n] = mris.vertex(vno).dist.as_ref().expect("dist")[i];
            }
        }
        if mris.vertex(vno).dist_orig.is_some() {
            for i in 0..old_vtotal {
                let n = v_of(mris, vno, i) as usize;
                dist_orig_cache[n] = mris.vertex(vno).dist_orig.as_ref().expect("dist_orig")[i];
            }
        }
        nbhd::find_new(g, sg, io, mris, vno, old_max, MAX_NEIGHBORS, &mut lists)?;
        set_nsize_cur(g, io, mris, vno, old_cur)?;
        if old_vtotal > old_cached {
            let mut new_vtotal = usize::try_from(vertex_vnum(
                g,
                io.out,
                io.err,
                mris.topology(vno),
                old_max,
            )?)
            .unwrap_or(0);
            let kept: Vec<i32> = tail
                .into_iter()
                .filter(|&x| mris.vertex(x as usize).ripflag == 0)
                .collect();
            let vt = mris.topology_mut(vno);
            let v = vt.v.get_or_insert_with(Vec::new);
            if v.len() < new_vtotal + kept.len() {
                v.resize(new_vtotal + kept.len(), 0);
            }
            for x in kept {
                v[new_vtotal] = x;
                new_vtotal += 1;
            }
            if old_vtotal < new_vtotal {
                return Err(assert_at(g, io, 3000, "old_vtotal >= new_vtotal"));
            }
            mris.topology_mut(vno).vtotal = new_vtotal as i16;
        }
        let vtotal = usize::try_from(mris.topology(vno).vtotal).unwrap_or(0);
        if mris.vertex(vno).dist.is_some() {
            for i in 0..vtotal {
                let n = v_of(mris, vno, i) as usize;
                let d = dist_cache[n];
                let dv = mris.vertex_mut(vno).dist.as_mut().expect("dist");
                if dv.len() <= i {
                    dv.resize(i + 1, 0.0);
                }
                dv[i] = d;
            }
        }
        if mris.vertex(vno).dist_orig.is_some() {
            for i in 0..vtotal {
                let n = v_of(mris, vno, i) as usize;
                let d = dist_orig_cache[n];
                let dv = mris.vertex_mut(vno).dist_orig.as_mut().expect("dist_orig");
                if dv.len() <= i {
                    dv.resize(i + 1, 0.0);
                }
                dv[i] = d;
            }
        }
        p = affected[vno];
        affected[vno] = 0;
        size -= 1;
    }
    while p != 1 {
        let vno = (p - 2) as usize;
        p = affected[vno];
        affected[vno] = 0;
        size -= 1;
    }
    if size != 0 {
        return Err(assert_at(g, io, 3024, "affectedSize == 0"));
    }
    if head != 1 {
        return Err(assert_at(g, io, 3025, "headAffectedVnosPlus2 == 1"));
    }
    for fno in 0..nfaces(mris) {
        if mris.face(fno).ripflag == 0 {
            continue;
        }
        for n in 0..VERTICES_PER_FACE {
            let vno = mris.face(fno).v[n];
            if !(0 <= vno && vno < mris.nvertices) {
                return Err(assert_at(
                    g,
                    io,
                    3038,
                    "(0 <= vno) && (vno < mris->nvertices)",
                ));
            }
            mris.face_mut(fno).v[n] = 0;
            let vno = vno as usize;
            if affected[vno] != 0 {
                continue;
            }
            affected[vno] = head;
            head = vno as i32 + 2;
            size += 1;
        }
    }
    if size > i64::from(mris.nvertices) {
        return Err(assert_at(g, io, 3049, "affectedSize <= mris->nvertices"));
    }
    let mut count: i64 = 0;
    let mut p = head;
    while p >= 2 {
        count += 1;
        if count > i64::from(mris.nvertices) {
            return Err(assert_at(g, io, 3058, "count <= mris->nvertices"));
        }
        let vno = (p - 2) as usize;
        p = affected[vno];
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let num = usize::from(mris.topology(vno).num);
        let mut k = 0usize;
        for i in 0..num {
            let fno = f_of(mris, vno, i);
            if mris.face(fno as usize).ripflag != 0 {
                continue;
            }
            if k != i {
                let vt = mris.topology_mut(vno);
                vt.f.as_mut().expect("f")[k] = fno;
                let nn = vt.n.as_mut().expect("VERTEX_TOPOLOGY::n is NULL");
                nn[k] = nn[i];
            }
            k += 1;
        }
        mris.topology_mut(vno).num = k as u8;
    }
    Ok(())
}

// ---- the orientation and the ordered neighbours ------------------------------------------------

/// `MRISevertSurface(mris)`: every unripped face's first two vertices swapped, their places in
/// the face set again in their face lists. `NO_ERROR`.
pub fn evert_surface(g: &mut Globals, io: &mut Io, mris: &mut Mris) -> Result<i32, Fatal> {
    for fno in 0..nfaces(mris) {
        if mris.face(fno).ripflag != 0 {
            continue;
        }
        let f = mris.face_mut(fno);
        let (vno0, vno1) = (f.v[0], f.v[1]);
        f.v[0] = vno1;
        f.v[1] = vno0;
        set_vertex_face_index(g, io, mris, vno0 as usize, fno)?;
        set_vertex_face_index(g, io, mris, vno1 as usize, fno)?;
    }
    Ok(NO_ERROR)
}

/// `MRISisNextSpatOrderedNeighbor(mris, centervno, vnotest, vnolist)`: 1 when `vnotest` is not in
/// the list and both the centre and the list's last vertex are among its 1-hop neighbours (two
/// hits among them, each entry counted for each it equals), else 0.
pub fn is_next_spat_ordered_neighbor(mris: &Mris, centervno: i32, vnotest: i32, vnolist: &[i32]) -> i32 {
    if vnolist.contains(&vnotest) {
        return 0;
    }
    let last = *vnolist.last().expect("a list of at least one vertex");
    let vnum = usize::try_from(mris.topology(vnotest as usize).vnum).unwrap_or(0);
    let mut nhits = 0;
    for n in 0..vnum {
        let x = v_of(mris, vnotest as usize, n);
        if x == centervno {
            nhits += 1;
        }
        if x == last {
            nhits += 1;
        }
        if nhits == 2 {
            return 1;
        }
    }
    0
}

/// `MRISgetSpatOrderedNeighbors(mris, centervno)`: the centre's 1-hop neighbours in order around
/// it: the two vertices after the centre in its first face, then for each further place the first
/// 1-hop neighbour (in list order) that [`is_next_spat_ordered_neighbor`] accepts, or none. FS-993:
/// a vertex without faces, or one its first face does not hold, reads memory outside its lists:
/// the list found up to there (noted).
pub fn get_spat_ordered_neighbors(g: &mut Globals, mris: &Mris, centervno: usize) -> Vec<i32> {
    let mut list = Vec::new();
    let vt = mris.topology(centervno);
    let first = vt.f.as_deref().filter(|_| vt.num > 0).map(|f| f[0]);
    let Some(f0) = first else {
        g.notices.add(
            &FS_993,
            format!(
                "MRISgetSpatOrderedNeighbors({centervno}): a vertex without faces (FreeSurfer reads its first face from memory never written)"
            ),
        );
        return list;
    };
    let face = mris.face(f0 as usize);
    let Some(mut k) = face.v.iter().position(|&x| x == centervno as i32) else {
        g.notices.add(
            &FS_993,
            format!(
                "MRISgetSpatOrderedNeighbors({centervno}): its first face {f0} does not hold it (FreeSurfer reads past the face's vertices)"
            ),
        );
        return list;
    };
    k = (k + 1) % 3;
    list.push(face.v[k]);
    k = (k + 1) % 3;
    list.push(face.v[k]);
    let vnum = usize::try_from(vt.vnum).unwrap_or(0);
    for _ in 2..vnum {
        for ntest in 0..vnum {
            let vnotest = v_of(mris, centervno, ntest);
            if is_next_spat_ordered_neighbor(mris, centervno as i32, vnotest, &list) != 0 {
                list.push(vnotest);
                break;
            }
        }
    }
    list
}

/// `nint` of `WHICH_FACE_SPLIT`'s sum, for the probe: [`which_face_split`].
pub fn face_split(g: &mut Globals, vno0: i32, vno1: i32) -> i32 {
    which_face_split(g, vno0, vno1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_truncates_as_c() {
        assert!(even(0) && even(2) && even(-2) && !even(1) && !even(-1) && even(i32::MIN));
    }
}
