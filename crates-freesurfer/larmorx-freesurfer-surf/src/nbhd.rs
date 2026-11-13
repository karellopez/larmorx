// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A surface's neighbourhoods (fs.surf, wave 3.4, queue item 3.4-03, taken early for `MRISread`,
//! queue item 3.1-48, relay 16): from FreeSurfer 8.2.0's `utils/mrisurf_topology.cpp`, the
//! neighbour lists from the faces (`mrisCompleteTopology` with its two ways, lines 2522-2662;
//! `mrisAttachFaceToVertices` and what it calls, lines 375-410, 1850-1883, 2093-2157), the
//! neighbourhood sizes (`MRIS_setNsizeCur` 818-834, `resizeVertexV` 851-875,
//! `MRISresetNeighborhoodSize` 953-992), the 2- and 3-hop neighbour search
//! (`MRISfindNeighborsAtVertex` with its new and old searches, 1088-1488) and the vertex-vertex
//! consistency check it runs on an overflow (`mrisCheckVertexVertexTopologyWkr`, 33-176); from
//! `utils/mrisurf_metricProperties.cpp`, `MRISsetNeighborhoodSize` and
//! `MRISsetNeighborhoodSizeAndDist` (4249-4347). FS-357, FS-358.

#![allow(clippy::needless_range_loop)]

use larmorx_core::defects::{FS_357, FS_358};
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::utils::{
    LogicProblemResponse, cope_with_logic_problem2, spend_time_checking_for_logic_problem,
};

use crate::aaa::SurfGlobals;
use crate::coords::{Io, metric_properties_cpp};
use crate::dist;
use crate::metric;
use crate::mris::Mris;
use crate::surfread::verbose_show;
use crate::topology::{
    clear_vnum, mod_vnum, vertex_vnum, vertex_vsize, vertices_are_neighbors, vnum_add,
};

/// `MAX_NEIGHBORS` (`mrisurf_aaa.h`): the length of the neighbour lists callers hand the search.
pub const MAX_NEIGHBORS: usize = 1024;
/// `MRIS_MAX_NEIGHBORHOOD_LINKS` (`mrisurf_aaa.h`): the most hops a search may ask for.
pub const MRIS_MAX_NEIGHBORHOOD_LINKS: i32 = 50;

/// `utils/mrisurf_topology.cpp` as the official build names it (`__FILE__`).
pub fn topology_cpp() -> String {
    format!("{}/utils/mrisurf_topology.cpp", diag::BUILD_ROOT)
}

fn assert_at(g: &mut Globals, io: &mut Io, file: &str, line: i32, tst: &str) -> Fatal {
    diag::assert_failed(g, io.out, io.err, file, line, tst)
}

/// `vt->v[i]`; past the list's end (only after FS-357's reproduced reading, which can leave a
/// count above the list FreeSurfer reallocated) 0, a deterministic stand-in for the memory there.
fn v_at(mris: &Mris, vno: usize, i: usize) -> i32 {
    mris.topology(vno)
        .v
        .as_ref()
        .and_then(|v| v.get(i).copied())
        .unwrap_or(0)
}

/// The first `vnum` entries of a vertex's neighbour list ([`v_at`] past its end).
fn first_ring(mris: &Mris, vno: usize) -> Vec<i32> {
    let k = i32::from(mris.topology(vno).vnum).max(0) as usize;
    (0..k).map(|i| v_at(mris, vno, i)).collect()
}

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

fn nfaces(mris: &Mris) -> usize {
    usize::try_from(mris.nfaces).unwrap_or(0)
}

// ---- neighbourhood sizes -----------------------------------------------------------------------

/// `MRIS_setNsizeCur(mris, vno, nsize)`: the vertex's neighbours in use (`vtotal`) those within
/// `nsize` hops (0: none; 1, 2, 3: `vnum`, `v2num`, `v3num`) and `nsizeCur` `nsize`. A size above
/// the vertex's `nsizeMax`, or not 0 to 3, fails an assertion (lines 821, 828).
pub fn set_nsize_cur(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    vno: usize,
    nsize: i32,
) -> Result<(), Fatal> {
    let file = topology_cpp();
    if nsize > i32::from(mris.topology(vno).nsizeMax) {
        return Err(assert_at(g, io, &file, 821, "nsize <= vt->nsizeMax"));
    }
    let vt = mris.topology_mut(vno);
    vt.vtotal = match nsize {
        0 => 0,
        1 => vt.vnum,
        2 => vt.v2num,
        3 => vt.v3num,
        _ => return Err(assert_at(g, io, &file, 828, "false")),
    };
    vt.nsizeCur = nsize as u8;
    Ok(())
}

/// `resizeVertexV(mris, vno, newSize, oldSize)`: the vertex's neighbour list made `new_size` long
/// (left as it is for a size of 0 or less), the entries from `old_size` to `new_size` zeroed; its
/// distances (when it has them) grown to at least `new_size` (`MRISgrowDist`).
pub fn resize_vertex_v(mris: &mut Mris, vno: usize, new_size: i32, old_size: i32) {
    let vt = mris.topology_mut(vno);
    if new_size > 0 {
        vt.v.get_or_insert_with(Vec::new)
            .resize(new_size as usize, 0);
    }
    if old_size < new_size {
        let v = vt.v.as_mut().expect("a neighbour list of the new size");
        for x in &mut v[old_size.max(0) as usize..new_size as usize] {
            *x = 0;
        }
    }
    if mris.vertex(vno).dist.is_some() {
        dist::grow_dist(mris, vno, new_size);
    }
}

/// `MRISresetNeighborhoodSize(mris, nsize)`: every unripped vertex's neighbours in use set to
/// those within `nsize` hops (1, 2 or 3; any other size: the vertex's largest, `nsizeMax`, which
/// must be above 0 and the same for every vertex, else a failed assertion, lines 971, 974);
/// `mris->nsize` the size (or that largest), `avg_nbrs` the mean count in use over the unripped
/// vertices (a float division, a NaN without one). Returns `NO_ERROR`.
pub fn reset_neighborhood_size(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    nsize: i32,
) -> Result<i32, Fatal> {
    let file = topology_cpp();
    let mut new_mris_nsize = nsize;
    let (mut ntotal, mut vtotal) = (0i32, 0i32);
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        match nsize {
            1..=3 => set_nsize_cur(g, io, mris, vno, nsize)?,
            _ => {
                let max = i32::from(mris.topology(vno).nsizeMax);
                if max <= 0 {
                    return Err(assert_at(g, io, &file, 971, "vt->nsizeMax > 0"));
                }
                set_nsize_cur(g, io, mris, vno, max)?;
                if new_mris_nsize < 0 {
                    new_mris_nsize = max;
                } else if new_mris_nsize != max {
                    return Err(assert_at(
                        g,
                        io,
                        &file,
                        974,
                        "new_mris_nsize == vt->nsizeMax",
                    ));
                }
            }
        }
        vtotal = vtotal.wrapping_add(i32::from(mris.topology(vno).vtotal));
        ntotal += 1;
    }
    mris.nsize = new_mris_nsize as i8;
    mris.avg_nbrs = sse::div(vtotal as f32, ntotal as f32);
    Ok(0)
}

// ---- the neighbour lists from the faces ----------------------------------------------------------

/// `mrisAddEdgeWkr(mris, vno1, vno2)`: each vertex gets the other at the end of its neighbour
/// list (made one longer), its neighbours in use all of them (`vtotal` = `vnum`, sizes 1); with
/// `DIAG_SHOW` and verbose diagnostics `adding edge <vno1> <--> <vno2>` on standard output first;
/// a count above the largest seen (`static int maxVnum`, 20 at first) becomes the largest, with
/// the debug message `vertex <v> has <n> immediate neighbours`.
pub fn add_edge_wkr(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    vno1: usize,
    vno2: usize,
) {
    if verbose_show(g) {
        let _ = writeln!(io.out, "adding edge {vno1} <--> {vno2}");
    }
    for (a, b) in [(vno1, vno2), (vno2, vno1)] {
        let n = (i32::from(mris.topology(a).vnum) + 1).max(0) as usize;
        mris.topology_mut(a)
            .v
            .get_or_insert_with(Vec::new)
            .resize(n, 0);
        let i = vnum_add(g, sg, io.err, mris, a, 1);
        let vt = mris.topology_mut(a);
        vt.v.as_mut().expect("the neighbour list")[i as usize] = b as i32;
        vt.vtotal = vt.vnum;
        vt.nsizeCur = 1;
        vt.nsizeMax = 1;
        let vnum = i32::from(vt.vnum);
        if sg.add_edge_max_vnum < vnum {
            sg.add_edge_max_vnum = vnum;
            log::debug(
                g,
                io.err,
                &format!("vertex {a} has {vnum} immediate neighbours"),
            );
        }
    }
}

/// `mrisCountAttachedFaces(mris, vno0, vno1)`: the faces in `vno0`'s face list that hold `vno1`
/// (each of its places counted).
pub fn count_attached_faces(mris: &Mris, vno0: usize, vno1: i32) -> i32 {
    let vt = mris.topology(vno0);
    let f = vt.f.as_deref().unwrap_or(&[]);
    let mut count = 0;
    for n in 0..usize::from(vt.num) {
        let face = mris.face(f[n] as usize);
        count += face.v.iter().filter(|&&v| v == vno1).count() as i32;
    }
    count
}

/// `mrisCanAttachFaceToVertices(mris, vno0, vno1, vno2)`: the three vertices must differ (else a
/// failed assertion, line 1873); true when none of the three edges has two faces already.
pub fn can_attach_face_to_vertices(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    vno0: i32,
    vno1: i32,
    vno2: i32,
) -> Result<bool, Fatal> {
    if !(vno0 != vno1 && vno0 != vno2 && vno1 != vno2) {
        return Err(assert_at(
            g,
            io,
            &topology_cpp(),
            1873,
            "vno0 != vno1 && vno0 != vno2 && vno1 != vno2",
        ));
    }
    Ok(count_attached_faces(mris, vno0 as usize, vno1) < 2
        && count_attached_faces(mris, vno0 as usize, vno2) < 2
        && count_attached_faces(mris, vno1 as usize, vno2) < 2)
}

/// `mrisAddFaceToVertex(mris, vno, fno, n)`: the face's `n`th vertex `vno`, the face appended to
/// the vertex's face list (its one-byte count incremented) with the vertex's place `n`.
fn add_face_to_vertex(mris: &mut Mris, vno: usize, fno: usize, n: usize) {
    mris.face_mut(fno).v[n] = vno as i32;
    let vt = mris.topology_mut(vno);
    let i = usize::from(vt.num);
    vt.num = vt.num.wrapping_add(1);
    let len = usize::from(vt.num);
    let f = vt.f.get_or_insert_with(Vec::new);
    f.resize(len.max(i + 1), 0);
    f[i] = fno as i32;
    let nn = vt.n.get_or_insert_with(Vec::new);
    nn.resize(len.max(i + 1), 0);
    nn[i] = n as u8;
}

/// `mrisAttachFaceWkr(mris, fno, vno0, vno1, vno2, edgesMustExist)`: the face and vertex numbers
/// must be the surface's (else failed assertions, lines 2106-2109) and the face attachable
/// ([`can_attach_face_to_vertices`], else line 2111); without `edges_must_exist`, each missing
/// edge of the face added ([`add_edge_wkr`]); a face whose vertices are all 0 is attached to
/// them (otherwise it is taken as attached already).
#[allow(clippy::too_many_arguments)]
pub fn attach_face_wkr(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    fno: i32,
    vno0: i32,
    vno1: i32,
    vno2: i32,
    edges_must_exist: bool,
) -> Result<(), Fatal> {
    let file = topology_cpp();
    if !(0 <= fno && fno < mris.nfaces) {
        return Err(assert_at(
            g,
            io,
            &file,
            2106,
            "(0 <= fno) && (fno < mris->nfaces)",
        ));
    }
    for (line, name, v) in [
        (2107, "vno0", vno0),
        (2108, "vno1", vno1),
        (2109, "vno2", vno2),
    ] {
        if !(0 <= v && v < mris.nvertices) {
            let tst = format!("(0 <= {name}) && ({name} < mris->nvertices)");
            return Err(assert_at(g, io, &file, line, &tst));
        }
    }
    if !can_attach_face_to_vertices(g, io, mris, vno0, vno1, vno2)? {
        return Err(assert_at(
            g,
            io,
            &file,
            2111,
            "mrisCanAttachFaceToVertices(mris, vno0, vno1, vno2)",
        ));
    }
    let vno = [vno0, vno1, vno2, vno0];
    if !edges_must_exist {
        for i in 0..3 {
            if !vertices_are_neighbors(g, io.out, io.err, mris, vno[i], vno[i + 1])? {
                add_edge_wkr(g, sg, io, mris, vno[i] as usize, vno[i + 1] as usize);
            }
        }
    }
    let f = mris.face(fno as usize).v;
    if (f[0] | f[1] | f[2]) == 0 {
        for i in 0..3 {
            add_face_to_vertex(mris, vno[i] as usize, fno as usize, i);
        }
    }
    Ok(())
}

/// `mrisCompleteTopology(mris)`: every vertex's neighbour list from its faces. FreeSurfer's way
/// (`mrisCompleteTopology_old`, unless `mrisCompleteTopology_new` is set, read at the first call):
/// for each vertex, through its face list in order, the face's vertex before and after it (in the
/// face's order, wrapping) appended when not yet in the list (`finding surface neighbors...` on
/// standard output first with `DIAG_SHOW` and verbose diagnostics); the list in use all of it
/// (sizes 1); then every face must be in each of its vertices' face lists (else the fatal error
/// `mrisFindNeighbors: <surface name>: face[<f>].v[<k>] = <v>, but face <f> not in vertex <v>
/// face list`); `avg_nbrs` the mean neighbour count over the unripped vertices (a float division,
/// a NaN without one). The other way (`mrisCompleteTopology_new`) attaches each face to its
/// vertices ([`attach_face_wkr`], adding the missing edges; a face of a closed surface fails its
/// assertion), `avg_nbrs` with at least one vertex counted.
pub fn complete_topology(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<(), Fatal> {
    let use_new = *sg
        .complete_topology_new
        .get_or_insert_with(|| io.env.get("mrisCompleteTopology_new").is_some());
    if use_new {
        complete_topology_new(g, sg, io, mris)
    } else {
        complete_topology_old(g, sg, io, mris)
    }
}

fn unripped_avg(mris: &Mris, at_least_one: bool) -> f32 {
    let (mut ntotal, mut vtotal) = (0i32, 0i32);
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        vtotal = vtotal.wrapping_add(i32::from(mris.topology(vno).vtotal));
        ntotal += 1;
    }
    if at_least_one && ntotal == 0 {
        ntotal = 1;
    }
    sse::div(vtotal as f32, ntotal as f32)
}

fn complete_topology_new(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<(), Fatal> {
    for fno in 0..nfaces(mris) {
        let v = mris.face(fno).v;
        attach_face_wkr(g, sg, io, mris, fno as i32, v[0], v[1], v[2], false)?;
    }
    mris.avg_nbrs = unripped_avg(mris, true);
    Ok(())
}

fn complete_topology_old(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<(), Fatal> {
    if verbose_show(g) {
        let _ = write!(io.out, "finding surface neighbors...");
    }
    for k in 0..nverts(mris) {
        clear_vnum(g, sg, io.err, mris, k);
        let (num, flist, nlist) = {
            let vt = mris.topology(k);
            (
                usize::from(vt.num),
                vt.f.clone().unwrap_or_default(),
                vt.n.clone().unwrap_or_default(),
            )
        };
        let mut vtmp = vec![0i32; MAX_NEIGHBORS];
        for m in 0..num {
            let n = usize::from(nlist[m]);
            let f = mris.face(flist[m] as usize).v;
            let n0 = if n == 0 { 2 } else { n - 1 };
            let n1 = if n == 2 { 0 } else { n + 1 };
            for other in [f[n0], f[n1]] {
                let vnum = i32::from(mris.topology(k).vnum).max(0) as usize;
                if !vtmp[..vnum].contains(&other) {
                    let i = vnum_add(g, sg, io.err, mris, k, 1);
                    vtmp[i as usize] = other;
                }
            }
        }
        let vt = mris.topology_mut(k);
        let vnum = i32::from(vt.vnum).max(0) as usize;
        vt.v = Some(vtmp[..vnum].to_vec());
        vt.vtotal = vt.vnum;
        vt.nsizeMax = 1;
        vt.nsizeCur = 1;
    }
    for k in 0..nfaces(mris) {
        let fv = mris.face(k).v;
        for (m, &v) in fv.iter().enumerate() {
            let vt = mris.topology(v as usize);
            let f = vt.f.as_deref().unwrap_or(&[]);
            if !f[..usize::from(vt.num)].contains(&(k as i32)) {
                let msg = format!(
                    "mrisFindNeighbors: {}: face[{k}].v[{m}] = {v}, but face {k} not in vertex {v} face list\n",
                    mris.fname
                );
                log::error_exit(g, io.err, ERROR_BADPARM, &msg)?;
            }
        }
    }
    mris.avg_nbrs = unripped_avg(mris, false);
    Ok(())
}

// ---- the neighbour search ------------------------------------------------------------------------

/// The caller's neighbour and hop lists (`int vlist[MAX_NEIGHBORS], hops[MAX_NEIGHBORS]`), kept
/// from one vertex's search to the next as FreeSurfer's stack arrays are.
pub struct NbrLists {
    pub vlist: Vec<i32>,
    pub hops: Vec<i32>,
}

impl Default for NbrLists {
    fn default() -> Self {
        NbrLists {
            vlist: vec![0; MAX_NEIGHBORS],
            hops: vec![0; MAX_NEIGHBORS],
        }
    }
}

/// `MRISfindNeighborsAtVertex(mris, vno, nlinks, listCapacity, vlist, hops)`: the vertices within
/// `nlinks` hops of `vno` (not itself), ring by ring, into the lists (at most `list_capacity`
/// written); returns how many there are. FreeSurfer's search is the new one
/// (`MRISfindNeighborsAtVertex_new`); `MRISfindNeighborsAtVertex_old` set (read at the first
/// call) takes the old one, both set run both and compare them (`<file>:1143Testing
/// MRISfindNeighborsAtVertex` once; a difference prints the counts or the first differing
/// entries and fails an assertion, line 1162), the old one's answer kept. The new search's
/// neighbour list must hold the surface's vertices only (else `<file>:1133 Bad
/// mris->vertices_topology[<vno>].v[<i>]` and a failed assertion, line 1134).
#[allow(clippy::too_many_arguments)]
pub fn find_neighbors_at_vertex(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    vno: usize,
    nlinks: i32,
    list_capacity: usize,
    lists: &mut NbrLists,
) -> Result<i32, Fatal> {
    let file = topology_cpp();
    if sg.find_nbrs_calls == 0 {
        let mut use_new = io.env.get("MRISfindNeighborsAtVertex_new").is_some();
        let use_old = io.env.get("MRISfindNeighborsAtVertex_old").is_some();
        if !(use_new || use_old) {
            use_new = true;
        }
        sg.find_nbrs_use = (use_new, use_old);
    }
    sg.find_nbrs_calls = sg.find_nbrs_calls.wrapping_add(1);
    let (use_new, use_old) = sg.find_nbrs_use;
    let use_both = use_new && use_old;
    let result_old = if use_old {
        find_old(g, sg, io, mris, vno, nlinks, lists)?
    } else {
        0
    };
    let mut tmp = NbrLists::default();
    let result_new = if use_new {
        if use_both {
            find_new(g, sg, io, mris, vno, nlinks, MAX_NEIGHBORS, &mut tmp)?
        } else {
            find_new(g, sg, io, mris, vno, nlinks, list_capacity, lists)?
        }
    } else {
        0
    };
    if nlinks < 4 {
        for i in 0..result_new.max(0) as usize {
            let x = v_at(mris, vno, i);
            if x < 0 || x >= mris.nvertices {
                let _ = writeln!(
                    io.out,
                    "{file}:1133 Bad mris->vertices_topology[{vno}].v[{i}]"
                );
                return Err(assert_at(g, io, &file, 1134, "false"));
            }
        }
    }
    if use_both {
        let mut good = true;
        if !sg.find_nbrs_shown_testing {
            sg.find_nbrs_shown_testing = true;
            let _ = writeln!(io.out, "{file}:1143Testing MRISfindNeighborsAtVertex");
        }
        if result_old != result_new {
            let _ = writeln!(
                io.out,
                " result_old:{result_old} != result_new:{result_new}"
            );
            good = false;
        }
        let n = result_old.min(result_new).max(0) as usize;
        for i in 0..n {
            let (a, b) = (lists.vlist[i], tmp.vlist[i]);
            let (c, d) = (lists.hops[i], tmp.hops[i]);
            if a != b || c != d {
                let _ = writeln!(
                    io.out,
                    " vlist[{i}] old:{a} != new:{b}  or  hops[{i}] old:{c} != new:{d}"
                );
                good = false;
            }
        }
        if !good {
            let _ = writeln!(
                io.out,
                "{file}:1105 laterTime:{} interestingLaterTime:0",
                sg.find_nbrs_calls
            );
            return Err(assert_at(g, io, &file, 1162, "false"));
        }
    }
    Ok(if use_old { result_old } else { result_new })
}

/// `MRISfindNeighborsAtVertex_new`: [`find_new_wkr`]; more neighbours than `list_capacity`
/// print `MRISfindNeighborsAtVertex: vno:<vno> has <n> neighbours, but listCapacity is <cap>`,
/// search again with each vertex shown, check the surface ([`check_vertex_vertex_topology_wkr`],
/// always) and fail an assertion (line 1409).
#[allow(clippy::too_many_arguments)]
fn find_new(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    vno: usize,
    nlinks: i32,
    list_capacity: usize,
    lists: &mut NbrLists,
) -> Result<i32, Fatal> {
    let n = find_new_wkr(
        g,
        sg,
        io,
        mris,
        vno,
        nlinks,
        list_capacity,
        lists,
        false,
        false,
    )?;
    if n as usize > list_capacity {
        let file = topology_cpp();
        let _ = writeln!(
            io.out,
            "MRISfindNeighborsAtVertex: vno:{vno} has {} neighbours, but listCapacity is {list_capacity}",
            n as i32
        );
        find_new_wkr(
            g,
            sg,
            io,
            mris,
            vno,
            nlinks,
            list_capacity,
            lists,
            false,
            true,
        )?;
        check_vertex_vertex_topology_wkr(g, sg, io, mris, &file, 1408, true)?;
        return Err(assert_at(g, io, &file, 1409, "false"));
    }
    Ok(n as i32)
}

/// `MRISfindNeighborsAtVertex_newWkr(mris, vno, nlinks, listCapacity, vlist, hops, noCache,
/// debug)`: the rings the vertex knows (`vnum`, and, unless `no_cache`, `v2num` and `v3num` to its
/// `nsizeMax`) taken from its list, then each further ring the unripped, not yet found neighbours
/// of the previous ring's vertices in order; with `debug` each found vertex shown. Unless
/// `no_cache`, the rings found are kept in the vertex (its list, `vnum`/`v2num`/`v3num` to
/// `min(3, nlinks)`, `nsizeMax`, its distances grown). A ripped centre, more than 50 hops, a vertex
/// without known neighbours (`nsizeMax` 0) or no hop asked for fail assertions (lines 1246,
/// 1250, 1271, 1310). A ripped vertex among the known rings ends the use of the known rings
/// there (the ring is found again); FS-357: FreeSurfer then scans the next ring from list
/// entries it never filled (its count still the known one): the default ends the ring where it
/// was filled; `reproduce` scans the entries as the lists hold them (the previous vertex's
/// entries; zeros at first). Returns how many were found (more than the capacity possible).
#[allow(clippy::too_many_arguments)]
fn find_new_wkr(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    vno: usize,
    nlinks: i32,
    list_capacity: usize,
    lists: &mut NbrLists,
    no_cache: bool,
    debug: bool,
) -> Result<u32, Fatal> {
    let file = topology_cpp();
    let nv = nverts(mris);
    if sg.find_nbrs_status.len() < nv {
        sg.find_nbrs_status = vec![0; nv];
    }
    if mris.vertex(vno).ripflag != 0 {
        return Err(assert_at(g, io, &file, 1246, "!v->ripflag"));
    }
    if nlinks > MRIS_MAX_NEIGHBORHOOD_LINKS {
        return Err(assert_at(
            g,
            io,
            &file,
            1250,
            "nlinks <= MRIS_MAX_NEIGHBORHOOD_LINKS",
        ));
    }
    let mut vnums = [0i32; MRIS_MAX_NEIGHBORHOOD_LINKS as usize + 1];
    let mut nsize = 0usize;
    {
        let vt = mris.topology(vno);
        vnums[nsize] = 0;
        nsize += 1;
        if vt.nsizeMax >= 1 {
            vnums[nsize] = i32::from(vt.vnum);
            nsize += 1;
        }
        if !no_cache {
            if vt.nsizeMax >= 2 {
                vnums[nsize] = i32::from(vt.v2num);
                nsize += 1;
            }
            if vt.nsizeMax >= 3 {
                vnums[nsize] = i32::from(vt.v3num);
                nsize += 1;
            }
        }
    }
    sg.find_nbrs_status[vno] = 1;
    let mut count: u32 = 0;
    if mris.topology(vno).nsizeMax == 0 {
        return Err(assert_at(g, io, &file, 1271, "vt->nsizeMax > 0"));
    }
    let cap = list_capacity;
    let mut ring_links: i32 = 1;
    while (ring_links as usize) < nsize && ring_links <= nlinks {
        let r = ring_links as usize;
        let (begin, end) = (vnums[r - 1], vnums[r]);
        let mut ripped = false;
        for n in begin.max(0)..end.max(0) {
            let cand = v_at(mris, vno, n as usize);
            if mris.vertex(cand as usize).ripflag != 0 {
                nsize = r;
                ripped = true;
                continue;
            }
            sg.find_nbrs_status[cand as usize] = 1;
            if (count as usize) < cap {
                lists.vlist[count as usize] = cand;
                lists.hops[count as usize] = ring_links;
            }
            if debug {
                let _ = writeln!(
                    io.out,
                    "  vnoCandidate:{cand} known to be in ring:{}",
                    ring_links - 1
                );
            }
            count += 1;
        }
        if ripped {
            let filled = (count as usize).min(cap) as i32;
            if filled != vnums[r] {
                g.notices.add(
                    &FS_357,
                    format!(
                        "vertex {vno}: a ripped vertex among its {}-hop neighbours: the ring ends at the {filled} vertices found (FreeSurfer reads {} list entries, some never filled)",
                        ring_links, vnums[r]
                    ),
                );
                if !g.notices.reproduce() {
                    vnums[r] = filled;
                }
            }
        }
        ring_links += 1;
    }
    if ring_links < 2 {
        return Err(assert_at(g, io, &file, 1310, "ringLinks >= 2"));
    }
    while ring_links <= nlinks {
        let r = ring_links as usize;
        let (kb, ke) = (vnums[r - 2], vnums[r - 1]);
        for i in kb.max(0)..ke.max(0) {
            let vno_ring = lists.vlist[i as usize];
            if vno_ring < 0 || vno_ring as usize >= nv {
                // only after FS-357's reproduced reading of entries never filled
                return Err(Fatal::Exit(139));
            }
            let ring_v = first_ring(mris, vno_ring as usize);
            for cand in ring_v {
                if mris.vertex(cand as usize).ripflag != 0 {
                    continue;
                }
                if sg.find_nbrs_status[cand as usize] != 0 {
                    continue;
                }
                sg.find_nbrs_status[cand as usize] = 1;
                if (count as usize) < cap {
                    lists.vlist[count as usize] = cand;
                    lists.hops[count as usize] = ring_links;
                }
                if debug {
                    let _ = writeln!(
                        io.out,
                        "  vnoCandidate:{cand} is neighbour of vno:{vno_ring} in ring:{i}"
                    );
                }
                count += 1;
            }
        }
        vnums[r] = (count as usize).min(cap) as i32;
        ring_links += 1;
    }
    if nsize == 0 {
        return Err(assert_at(g, io, &file, 1353, "nsize > 0"));
    }
    let nsize = nsize - 1;
    if !no_cache {
        let npm = 3.min(ring_links - 1) as usize;
        if nsize < npm {
            let (old, new) = (vnums[nsize], vnums[npm]);
            resize_vertex_v(mris, vno, new, old);
            {
                let v = mris
                    .topology_mut(vno)
                    .v
                    .as_mut()
                    .expect("the neighbour list");
                for i in old.max(0)..new.max(0) {
                    v[i as usize] = lists.vlist[i as usize];
                }
            }
            let clock = mris.nsizeMaxClock;
            for cached in nsize + 1..=npm {
                match cached {
                    1 => {
                        mod_vnum(g, sg, io.err, mris, vno, vnums[1] as i16, true);
                    }
                    2 => mris.topology_mut(vno).v2num = vnums[2] as i16,
                    3 => mris.topology_mut(vno).v3num = vnums[3] as i16,
                    _ => return Err(assert_at(g, io, &file, 1376, "false")),
                }
                let vt = mris.topology_mut(vno);
                vt.nsizeMax = npm as u8;
                vt.nsizeMaxClock = clock;
            }
            let vt = mris.topology(vno);
            if vt.nsizeCur > vt.nsizeMax {
                return Err(assert_at(
                    g,
                    io,
                    &file,
                    1382,
                    "vt->nsizeCur <= vt->nsizeMax",
                ));
            }
            let cur = usize::from(vt.nsizeCur);
            mris.topology_mut(vno).vtotal = vnums[cur] as i16;
        }
    }
    sg.find_nbrs_status[vno] = 0;
    for i in 0..(count as usize).min(cap) {
        sg.find_nbrs_status[lists.vlist[i] as usize] = 0;
    }
    Ok(count)
}

/// One write of the old search's list: within the caller's 1,024 entries, or past them (FS-358:
/// refused by default with an error, which ends the program; `reproduce` writes on, the list
/// grown as if the memory were its own).
fn put_old(
    g: &mut Globals,
    io: &mut Io,
    list: &mut Vec<i32>,
    i: usize,
    x: i32,
) -> Result<(), Fatal> {
    if i >= MAX_NEIGHBORS {
        g.notices.add(
            &FS_358,
            format!("the old neighbour search reaches entry {i} of a {MAX_NEIGHBORS}-entry list"),
        );
        if !g.notices.reproduce() {
            log::error_exit(
                g,
                io.err,
                ERROR_BADPARM,
                &format!(
                    "MRISfindNeighborsAtVertex: more than {MAX_NEIGHBORS} neighbours for the old search's list"
                ),
            )?;
        }
    }
    if i >= list.len() {
        list.resize(i + 1, 0);
    }
    list[i] = x;
    Ok(())
}

/// `MRISfindNeighborsAtVertex_old(mris, vno, nlinks, listCapacity, vlist, hops)`: 0 for a ripped
/// vertex; at its first call `<file>:1426 checking marks initially clear` on standard output; every
/// vertex's mark must be 0 (else a failed assertion, line 1429). The vertex's neighbours in use
/// listed and marked with their hop counts (by their place in its list); `nlinks` within the
/// surface's size: the count at `nlinks` hops (`nlinks` 3 sets the vertex's `vtotal` to `v3num`
/// and counts 0; a size the vertex does not have fails an assertion, line 1440; below 1 the fatal
/// error `MRISfindNeighborsAtVertex: nlinks=<n> invalid`); beyond it, rings added from the
/// unripped listed vertices' immediate neighbours not yet marked. The listed vertices' marks
/// become their hops and are cleared (the vertex's too); the vertex's cache is not changed.
/// FS-358.
fn find_old(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    vno: usize,
    nlinks: i32,
    lists: &mut NbrLists,
) -> Result<i32, Fatal> {
    let file = topology_cpp();
    if mris.vertex(vno).ripflag != 0 {
        return Ok(0);
    }
    if !sg.find_nbrs_old_shown {
        sg.find_nbrs_old_shown = true;
        let _ = writeln!(io.out, "{file}:1426 checking marks initially clear");
    }
    for n in 0..nverts(mris) {
        if mris.vertex(n).marked != 0 {
            return Err(assert_at(
                g,
                io,
                &file,
                1429,
                "mris->vertices[n].marked == 0",
            ));
        }
    }
    mris.vertex_mut(vno).marked = -1;
    let (vtotal0, vnum, v2num) = {
        let vt = mris.topology(vno);
        (
            i32::from(vt.vtotal),
            i32::from(vt.vnum),
            i32::from(vt.v2num),
        )
    };
    for n in 0..vtotal0.max(0) {
        let x = v_at(mris, vno, n as usize);
        put_old(g, io, &mut lists.vlist, n as usize, x)?;
        mris.vertex_mut(x as usize).marked = if n < vnum {
            1
        } else if n < v2num {
            2
        } else {
            3
        };
    }
    let mut vtotal: i32 = 0;
    if nlinks <= i32::from(mris.nsize) {
        if nlinks > i32::from(mris.topology(vno).nsizeCur) {
            return Err(assert_at(g, io, &file, 1440, "nlinks <= vt->nsizeCur"));
        }
        match nlinks {
            1 => vtotal = vnum,
            2 => vtotal = v2num,
            3 => {
                let vt = mris.topology_mut(vno);
                vt.vtotal = vt.v3num;
            }
            _ => {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_BADPARM,
                    &format!("MRISfindNeighborsAtVertex: nlinks={nlinks} invalid"),
                )?;
            }
        }
    } else {
        let mut link_dist = i32::from(mris.topology(vno).nsizeCur);
        vtotal = vtotal0;
        while link_dist < nlinks {
            link_dist += 1;
            let mut ring_total = 0;
            for n in 0..vtotal.max(0) {
                let vn = lists.vlist[n as usize] as usize;
                if mris.vertex(vn).ripflag != 0 {
                    continue;
                }
                let ring_v = first_ring(mris, vn);
                for w in ring_v {
                    if mris.vertex(w as usize).marked == 0 {
                        put_old(g, io, &mut lists.vlist, (vtotal + ring_total) as usize, w)?;
                        mris.vertex_mut(w as usize).marked = link_dist as i16;
                        ring_total += 1;
                    }
                }
            }
            vtotal += ring_total;
        }
    }
    for n in 0..vtotal.max(0) as usize {
        let x = lists.vlist[n] as usize;
        let hop = i32::from(mris.vertex(x).marked);
        put_old(g, io, &mut lists.hops, n, hop)?;
        mris.vertex_mut(x).marked = 0;
    }
    mris.vertex_mut(vno).marked = 0;
    Ok(vtotal)
}

// ---- the neighbourhood size with distances -----------------------------------------------------

/// `MRISsetNeighborhoodSize(mris, nsize)`: [`set_neighborhood_size_and_optionally_dist`] without
/// forcing the distances.
pub fn set_neighborhood_size(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    nsize: i32,
) -> Result<(), Fatal> {
    set_neighborhood_size_and_optionally_dist(g, sg, io, mris, nsize, false)
}

/// `MRISsetNeighborhoodSizeAndDist(mris, nsize)`: [`set_neighborhood_size_and_optionally_dist`]
/// with the distances.
pub fn set_neighborhood_size_and_dist(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    nsize: i32,
) -> Result<(), Fatal> {
    set_neighborhood_size_and_optionally_dist(g, sg, io, mris, nsize, true)
}

/// `MRISsetNeighborhoodSizeAndOptionallyDist(mris, nsize, alwaysDoDist)`: `nsize` must be 1 to 3
/// (else a failed assertion, line 4276). Within the surface's largest size (`max_nsize`): the
/// neighbourhood reset to it ([`reset_neighborhood_size`]); beyond it, each vertex's rings found
/// and kept ([`find_neighbors_at_vertex`], 1,024 entries), then the reset, `max_nsize` the size.
/// When the distances are known to fewer hops (`dist_nsize`), they are computed again
/// (`mrisComputeVertexDistances`), and the original ones too where they are kept
/// (`dist_alloced_flags` bit 2). The distances' sizes are then checked (failed assertions, lines
/// 4260, 4261, 4345, 4346).
pub fn set_neighborhood_size_and_optionally_dist(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    nsize: i32,
    always_do_dist: bool,
) -> Result<(), Fatal> {
    set_wkr(g, sg, io, mris, nsize, always_do_dist)?;
    let file = metric_properties_cpp();
    if mris.dist_alloced_flags & 1 == 0 {
        if mris.dist_nsize != 0 {
            return Err(assert_at(g, io, &file, 4260, "mris->dist_nsize == 0"));
        }
    } else if i32::from(mris.dist_nsize) < nsize {
        return Err(assert_at(g, io, &file, 4261, "mris->dist_nsize >= nsize"));
    }
    Ok(())
}

fn set_wkr(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    nsize: i32,
    mut always_do_dist: bool,
) -> Result<(), Fatal> {
    let file = metric_properties_cpp();
    if !(1..4).contains(&nsize) {
        return Err(assert_at(g, io, &file, 4276, "1 <= nsize && nsize < 4"));
    }
    let grew = nsize > i32::from(mris.max_nsize);
    if !grew {
        reset_neighborhood_size(g, io, mris, nsize)?;
    } else {
        always_do_dist |= mris.dist_alloced_flags & 1 != 0;
        let mut lists = NbrLists::default();
        for vno in 0..nverts(mris) {
            find_neighbors_at_vertex(g, sg, io, mris, vno, nsize, MAX_NEIGHBORS, &mut lists)?;
        }
        reset_neighborhood_size(g, io, mris, nsize)?;
        mris.max_nsize = nsize as i8;
    }
    if i32::from(mris.nsize) != nsize {
        return Err(assert_at(g, io, &file, 4308, "mris->nsize == nsize"));
    }
    if nsize <= i32::from(mris.dist_nsize) {
        return Ok(());
    }
    let should_do_dist = always_do_dist || grew || i32::from(mris.dist_nsize) < nsize;
    let should_do_dist_orig = should_do_dist && mris.dist_alloced_flags & 2 != 0;
    if should_do_dist {
        metric::compute_vertex_distances(g, io, mris)?;
    }
    if should_do_dist_orig {
        metric::compute_original_vertex_distances(g, io, mris)?;
    }
    if !(mris.dist_alloced_flags & 1 == 0 || i32::from(mris.dist_nsize) >= nsize) {
        return Err(assert_at(
            g,
            io,
            &file,
            4345,
            "(!(mris->dist_alloced_flags&1)) || (mris->dist_nsize >= nsize)",
        ));
    }
    if !(mris.dist_alloced_flags & 2 == 0 || i32::from(mris.dist_orig_nsize) >= nsize) {
        return Err(assert_at(
            g,
            io,
            &file,
            4346,
            "(!(mris->dist_alloced_flags&2)) || (mris->dist_orig_nsize >= nsize)",
        ));
    }
    Ok(())
}

// ---- the vertex-vertex check ---------------------------------------------------------------------

/// `shouldReportWkr(line)` with the `shouldReport` macro: the logic problem `Bad vertex or face
/// found` (fixed, which fails an assertion at line 38, when `FREESURFER_crash_on_bad_topology` is
/// set); whether it was reported.
fn should_report(g: &mut Globals, io: &mut Io, line: i32) -> Result<bool, Fatal> {
    let file = topology_cpp();
    let (resp, reported) = cope_with_logic_problem2(
        g,
        io.env,
        io.out,
        io.err,
        Some("FREESURFER_crash_on_bad_topology"),
        "Bad vertex or face found",
        &file,
        line,
        "",
    )?;
    if resp == LogicProblemResponse::Fix {
        return Err(assert_at(g, io, &file, 38, "false"));
    }
    Ok(reported)
}

/// `mrisCheckVertexVertexTopologyWkr(file, line, mris, always)`: unless `always` (or
/// `FREESURFER_always_check_vv`, read at the first call, which prints `<file>:52
/// FREESURFER_always_check_vv set`), only when the place's checking is due
/// (`spendTimeCheckingForLogicProblem`). Each inconsistency is a logic problem
/// ([`should_report`]) whose line on standard output is printed when it is reported: the
/// surface's size above its largest, distances present without their flag, an unripped vertex's
/// size not the surface's, above its largest, its count in use not the count of its size, a
/// neighbour that is not a vertex of the surface, an immediate neighbour that does not list the
/// vertex back, a neighbour listed twice. With `FREESURFER_checkDist` (read once; `<file>:163
/// checking dist[] contents`) the distances are checked too. True when nothing was reported.
pub fn check_vertex_vertex_topology_wkr(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    file: &str,
    line: i32,
    always: bool,
) -> Result<bool, Fatal> {
    let tfile = topology_cpp();
    let force = match sg.check_vv_force_always {
        Some(f) => f,
        None => {
            let f = io.env.get("FREESURFER_always_check_vv").is_some();
            sg.check_vv_force_always = Some(f);
            if f {
                let _ = writeln!(io.out, "{tfile}:52 FREESURFER_always_check_vv set");
            }
            f
        }
    };
    if !(always || force) && !spend_time_checking_for_logic_problem(g, io.out, io.err, file, line)?
    {
        return Ok(true);
    }
    let mut reported = false;
    if mris.nsize > mris.max_nsize && should_report(g, io, 62)? {
        reported = true;
        let _ = writeln!(
            io.out,
            "mris->nsize:{} > mris->max_nsize:{}",
            mris.nsize, mris.max_nsize
        );
    }
    for vno1 in 0..nverts(mris) {
        let flags = mris.dist_alloced_flags;
        let (has_dist, has_orig, ripped) = {
            let v = mris.vertex(vno1);
            (v.dist.is_some(), v.dist_orig.is_some(), v.ripflag != 0)
        };
        if ((has_dist && flags & 1 == 0) || (has_orig && flags & 2 == 0))
            && should_report(g, io, 91)?
        {
            reported = true;
            let _ = writeln!(
                io.out,
                "dist or dist_orig non-null when !mris->dist_alloced"
            );
        }
        if ripped {
            continue;
        }
        let vsize = vertex_vsize(mris, vno1);
        let vt = mris.topology(vno1).clone();
        let nsize = i32::from(mris.nsize);
        if nsize > 0 && nsize != i32::from(vt.nsizeCur) && vsize > 0 && should_report(g, io, 103)? {
            reported = true;
            let _ = writeln!(
                io.out,
                "[vno1:{vno1}].nsizeCur:{} != mris->nsize:{nsize} vSize:{vsize} vnum:{} vtotal:{}",
                vt.nsizeCur, vt.vnum, vt.vtotal
            );
        }
        if vt.nsizeMax > 0 && vt.nsizeCur > vt.nsizeMax && should_report(g, io, 111)? {
            reported = true;
            let _ = writeln!(
                io.out,
                "[vno1:{vno1}].nsizeCur:{} exceeds nsizeMax:{}",
                vt.nsizeCur, vt.nsizeMax
            );
        }
        let expected = if vt.nsizeCur == 0 {
            0
        } else {
            vertex_vnum(g, io.out, io.err, &vt, i32::from(vt.nsizeCur))?
        };
        let off = if mris.vtotalsMightBeTooBig != 0 {
            vt.vtotal < expected
        } else {
            vt.vtotal != expected
        };
        if vt.nsizeCur > 0 && off && should_report(g, io, 123)? {
            reported = true;
            let _ = writeln!(
                io.out,
                "[vno1:{vno1}].vtotal:{} differs from expected:{expected} for nsize:{}, ripflag:0",
                vt.vtotal, vt.nsizeCur
            );
        }
        let list = vt.v.clone().unwrap_or_default();
        for n in 0..vsize.max(0) as usize {
            let vno2 = list[n];
            if (vno2 < 0 || mris.nvertices <= vno2) && should_report(g, io, 133)? {
                reported = true;
                let _ = writeln!(io.out, "[vno1:{vno1}].v[{n}] is bad vno2:{vno2}");
            }
            if i32::from(vt.vnum) <= n as i32 {
                continue;
            }
            if !vertices_are_neighbors(g, io.out, io.err, mris, vno2, vno1 as i32)?
                && should_report(g, io, 142)?
            {
                reported = true;
                let _ = writeln!(
                    io.out,
                    "[vno1:{vno1}].v[{n}] not found in [vno2:{vno2}].v[*]"
                );
            }
            for i in 0..n {
                if vno2 == list[i] && should_report(g, io, 151)? {
                    reported = true;
                    let _ = writeln!(
                        io.out,
                        "[vno1:{vno1}].v[{n}]:{vno2} same as [vno1:{vno1}].v[{i}]:{}",
                        list[i]
                    );
                }
            }
        }
    }
    let check_dist = match sg.check_vv_check_dist {
        Some(c) => c,
        None => {
            let c = io.env.get("FREESURFER_checkDist").is_some();
            sg.check_vv_check_dist = Some(c);
            if c {
                let _ = writeln!(io.out, "{tfile}:163 checking dist[] contents");
            }
            c
        }
    };
    if check_dist {
        if !metric::check_dist(g, io, mris)? {
            reported = true;
        }
        if !metric::check_dist_orig(g, io, mris)? {
            reported = true;
        }
    }
    Ok(!reported)
}
