// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A surface's life (`fs.surfio`, queue item 3.1-47; `utils/mrisurf_base.cpp` 422-776): the
//! element destructors, `mrisDumpVertex`, `mrisDumpFace`, `mrisDumpShape`, the reallocation of the
//! vertex and face arrays (`MRISreallocVertices`, `MRISgrowNVertices`, `MRIStruncateNVertices`,
//! `MRISremovedVertices`, `MRISreallocFaces`, `MRISallocateFaces`, `MRISgrowNFaces`,
//! `MRIStruncateNFaces`, `MRISremovedFaces`, `MRISoverAllocVerticesAndFaces`,
//! `MRISreallocVerticesAndFaces`), `MRIScopyMetadata`, `MRISctr`, `MRISdtr`, `MRISoverAlloc`,
//! `MRISalloc` and `MRISfree`.
//!
//! FreeSurfer grows its arrays with `realloc`; glibc 2.35's `realloc` of a `NULL` array to 0 bytes
//! gives a valid empty array (`malloc(0)`), but of an existing array to 0 bytes frees it and
//! returns `NULL` (FS-117): [`c_realloc`] models both. The places `realloc` adds are undefined in
//! C; every caller here zeroes them, as [`c_realloc`]'s defaults do.

use std::io::Write;

use larmorx_core::defects::FS_117;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{Matrix, matrix_copy};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_mri::ctab::ctab_free;

use crate::aaa::{FaceNormCacheEntry, FaceNormDeferredEntry, MAX_CMDS, SurfGlobals};
use crate::lta::{lta_copy, lta_free};
use crate::mris::{Face, Mris, VertexTopology};
use crate::topology::vertex_vsize;

fn base_cpp() -> String {
    format!("{}/utils/mrisurf_base.cpp", diag::BUILD_ROOT)
}

fn assert_at(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    line: i32,
    tst: &str,
) -> Fatal {
    diag::assert_failed(g, out, err, &base_cpp(), line, tst)
}

/// `p = realloc(p, n * sizeof(T))` as glibc 2.35 does it: a `NULL` array becomes a new one of
/// `n` places (an empty one for `n` 0: `malloc(0)` is a valid pointer); an existing array of `n`
/// 0 is freed and the result is `NULL` (FS-117: by default an empty array is kept, with the
/// defect's notice; `reproduce` frees it); otherwise the array keeps its first `n` places and new
/// ones are `T::default()`. An allocation that fails gives `NULL` (the old array lost, as C's
/// `p = realloc(p, ...)` loses it). Returns whether the result is not `NULL`.
pub fn c_realloc<T: Clone + Default>(
    g: &mut Globals,
    p: &mut Option<Vec<T>>,
    n: usize,
    what: &str,
) -> bool {
    if n == 0 {
        if p.is_some() {
            g.notices.add(
                &FS_117,
                format!("{what} reallocated to no places: kept as an empty array"),
            );
            if g.notices.reproduce() {
                *p = None;
                return false;
            }
        }
        *p = Some(Vec::new());
        return true;
    }
    let mut v = p.take().unwrap_or_default();
    if v.len() < n {
        if v.try_reserve_exact(n - v.len()).is_err() {
            return false;
        }
        v.resize(n, T::default());
    } else {
        v.truncate(n);
    }
    *p = Some(v);
    true
}

fn nonneg(n: i32) -> usize {
    usize::try_from(n).unwrap_or(0)
}

/// `VERTEX` zeroed in place by `bzero`, the surface's distance buffers kept: FreeSurfer's
/// `v->dist` and `v->dist_orig` point into `dist_storage[vno]` and `dist_orig_storage[vno]`,
/// which `bzero` of the vertex does not touch (here the valid buffer moves back to the storage).
fn zero_vertex(mris: &mut Mris, vno: usize) {
    let v = std::mem::take(mris.vertex_mut(vno));
    if let Some(d) = v.dist
        && let Some(st) = mris.dist_storage.as_mut()
    {
        st[vno] = Some(d);
    }
    if let Some(d) = v.dist_orig
        && let Some(st) = mris.dist_orig_storage.as_mut()
    {
        st[vno] = Some(d);
    }
    *mris.topology_mut(vno) = VertexTopology::default();
}

/// `MRISreallocVertices(mris, max_vertices, nvertices)`: room for `max_vertices` (never fewer
/// than the surface has room for already), `nvertices` in use. The places from the old
/// `nvertices` to the new room are zeroed (vertices and their topology); the distance storage's
/// new places are `NULL`. Fails `cheapAssert(nvertices >= 0)` (line 521),
/// `cheapAssert(max_vertices >= nvertices)` (522) or `cheapAssert(!mris->nverticesFrozen)` (523)
/// (FS-03). Returns `false`, the counts unchanged and the vertex array `NULL`, when the vertex
/// array's reallocation gives `NULL` (FS-117: a surface with room for no vertex reallocated to
/// none; by default it keeps an empty array).
///
/// A surface whose `nvertices` is negative (only `MRISgrowNVertices` or `MRIStruncateNVertices`
/// with a negative count make one) is zeroed from before its array in C: out of range here (a
/// panic).
pub fn realloc_vertices(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sg: &SurfGlobals,
    mris: &mut Mris,
    max_vertices: i32,
    nvertices: i32,
) -> Result<bool, Fatal> {
    let max_vertices = max_vertices.max(mris.max_vertices);
    if nvertices < 0 {
        return Err(assert_at(g, out, err, 521, "nvertices >= 0"));
    }
    if max_vertices < nvertices {
        return Err(assert_at(g, out, err, 522, "max_vertices >= nvertices"));
    }
    if mris.nverticesFrozen != 0 {
        return Err(assert_at(g, out, err, 523, "!mris->nverticesFrozen"));
    }
    assert!(
        mris.nvertices >= 0,
        "MRISreallocVertices of a surface with a negative vertex count writes before its array"
    );
    let n = nonneg(max_vertices);
    if !c_realloc(g, &mut mris.vertices, n, "the vertex array") {
        return Ok(false);
    }
    c_realloc(g, &mut mris.vertices_topology, n, "the vertex topology");
    c_realloc(g, &mut mris.dist_storage, n, "the distance storage");
    c_realloc(
        g,
        &mut mris.dist_orig_storage,
        n,
        "the original distance storage",
    );
    let old_nv = nonneg(mris.nvertices);
    let old_max = nonneg(mris.max_vertices);
    for vno in old_nv..n {
        if vno < old_max {
            zero_vertex(mris, vno);
        }
        // places past the old room are new: already zero
    }
    // the storage's new places (old room .. new room) are NULL already
    mris.max_vertices = max_vertices;
    mris.nvertices = nvertices;
    notify_active_realm_trees_changed(sg);
    Ok(true)
}

/// `notifyActiveRealmTreesChangedNFacesNVertices(mris)`: updates the realm trees registered on the
/// surface. The realm trees are fs.surf's (`utils/realm.cpp`, wave 3.4); until they are ported no
/// tree is ever registered, and a surface with one is out of range (a panic).
fn notify_active_realm_trees_changed(sg: &SurfGlobals) {
    assert!(
        sg.active_realm_trees_size == 0,
        "realm trees are fs.surf's (not yet ported)"
    );
}

/// `MRISgrowNVertices(mris, nvertices)`: `nvertices` in use; fails
/// `cheapAssert(nvertices <= mris->max_vertices)` (line 559) (FS-03).
pub fn grow_nvertices(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &mut Mris,
    nvertices: i32,
) -> Result<(), Fatal> {
    if nvertices > mris.max_vertices {
        return Err(assert_at(g, out, err, 559, "nvertices <= mris->max_vertices"));
    }
    mris.nvertices = nvertices;
    Ok(())
}

/// `MRIStruncateNVertices(mris, nvertices)`: fails `cheapAssert(mris->nvertices >= nvertices)`
/// (line 565) (FS-03).
pub fn truncate_nvertices(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &mut Mris,
    nvertices: i32,
) -> Result<(), Fatal> {
    if mris.nvertices < nvertices {
        return Err(assert_at(g, out, err, 565, "mris->nvertices >= nvertices"));
    }
    mris.nvertices = nvertices;
    Ok(())
}

/// `MRISremovedVertices(mris, nvertices)`: as [`truncate_nvertices`] (its assertion on line 571).
pub fn removed_vertices(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &mut Mris,
    nvertices: i32,
) -> Result<(), Fatal> {
    if mris.nvertices < nvertices {
        return Err(assert_at(g, out, err, 571, "mris->nvertices >= nvertices"));
    }
    mris.nvertices = nvertices;
    Ok(())
}

/// `MRISreallocFaces(mris, max_faces, nfaces)`: the faces and their two face-normal caches get
/// exactly `max_faces` places (fewer than before too: the faces past it are dropped), `nfaces` in
/// use; the places from the old `nfaces` to `max_faces` zeroed. Fails `cheapAssert(nfaces >= 0)`
/// (line 579), `cheapAssert(max_faces >= nfaces)` (580), and, when a reallocation gives `NULL`,
/// `cheapAssert(mris->faces)` (584), `cheapAssert(mris->faceNormCacheEntries)` (588) or
/// `cheapAssert(mris->faceNormDeferredEntries)` (592) (FS-03): FreeSurfer's every reallocation to no
/// faces of a surface that has its face arrays (every surface `MRISalloc` made has them, even with
/// no face) frees them and fails the first (FS-117; by default empty arrays are kept). Returns
/// `true`.
pub fn realloc_faces(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &mut Mris,
    max_faces: i32,
    nfaces: i32,
) -> Result<bool, Fatal> {
    if nfaces < 0 {
        return Err(assert_at(g, out, err, 579, "nfaces >= 0"));
    }
    if max_faces < nfaces {
        return Err(assert_at(g, out, err, 580, "max_faces >= nfaces"));
    }
    assert!(
        mris.nfaces >= 0,
        "MRISreallocFaces of a surface with a negative face count writes before its array"
    );
    let n = nonneg(max_faces);
    if !c_realloc(g, &mut mris.faces, n, "the face array") {
        return Err(assert_at(g, out, err, 584, "mris->faces"));
    }
    if !c_realloc::<FaceNormCacheEntry>(
        g,
        &mut mris.faceNormCacheEntries,
        n,
        "the face-normal cache",
    ) {
        return Err(assert_at(g, out, err, 588, "mris->faceNormCacheEntries"));
    }
    if !c_realloc::<FaceNormDeferredEntry>(
        g,
        &mut mris.faceNormDeferredEntries,
        n,
        "the deferred-face list",
    ) {
        return Err(assert_at(g, out, err, 592, "mris->faceNormDeferredEntries"));
    }
    let old_nf = nonneg(mris.nfaces);
    for fno in old_nf..n {
        mris.faces.as_mut().expect("faces")[fno] = Face::default();
        mris.faceNormCacheEntries.as_mut().expect("cache")[fno] = FaceNormCacheEntry::default();
        mris.faceNormDeferredEntries.as_mut().expect("deferred")[fno] =
            FaceNormDeferredEntry::default();
    }
    mris.max_faces = max_faces;
    mris.nfaces = nfaces;
    Ok(true)
}

/// `MRISallocateFaces(mris, nfaces)`: [`realloc_faces`] with `max_faces` = `nfaces`.
pub fn allocate_faces(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &mut Mris,
    nfaces: i32,
) -> Result<bool, Fatal> {
    realloc_faces(g, out, err, mris, nfaces, nfaces)
}

/// `MRISgrowNFaces(mris, nfaces)`: fails `cheapAssert(nfaces <= mris->max_faces)` (line 613).
pub fn grow_nfaces(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &mut Mris,
    nfaces: i32,
) -> Result<(), Fatal> {
    if nfaces > mris.max_faces {
        return Err(assert_at(g, out, err, 613, "nfaces <= mris->max_faces"));
    }
    mris.nfaces = nfaces;
    Ok(())
}

/// `MRIStruncateNFaces(mris, nfaces)`: fails `cheapAssert(mris->nfaces >= nfaces)` (line 619).
pub fn truncate_nfaces(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &mut Mris,
    nfaces: i32,
) -> Result<(), Fatal> {
    if mris.nfaces < nfaces {
        return Err(assert_at(g, out, err, 619, "mris->nfaces >= nfaces"));
    }
    mris.nfaces = nfaces;
    Ok(())
}

/// `MRISremovedFaces(mris, nfaces)`: as [`truncate_nfaces`] (its assertion on line 625).
pub fn removed_faces(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &mut Mris,
    nfaces: i32,
) -> Result<(), Fatal> {
    if mris.nfaces < nfaces {
        return Err(assert_at(g, out, err, 625, "mris->nfaces >= nfaces"));
    }
    mris.nfaces = nfaces;
    Ok(())
}

/// `MRISoverAllocVerticesAndFaces(mris, max_vertices, max_faces, nvertices, nfaces)`: fails
/// `cheapAssert(nvertices >= 0)` (line 634) or `cheapAssert(nfaces >= 0)` (635); the room at
/// least the counts; then [`realloc_vertices`] (its result ignored) and [`realloc_faces`].
#[allow(clippy::too_many_arguments)]
pub fn over_alloc_vertices_and_faces(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sg: &SurfGlobals,
    mris: &mut Mris,
    max_vertices: i32,
    max_faces: i32,
    nvertices: i32,
    nfaces: i32,
) -> Result<(), Fatal> {
    if nvertices < 0 {
        return Err(assert_at(g, out, err, 634, "nvertices >= 0"));
    }
    if nfaces < 0 {
        return Err(assert_at(g, out, err, 635, "nfaces >= 0"));
    }
    let max_vertices = max_vertices.max(nvertices);
    let max_faces = max_faces.max(nfaces);
    realloc_vertices(g, out, err, sg, mris, max_vertices, nvertices)?;
    realloc_faces(g, out, err, mris, max_faces, nfaces)?;
    Ok(())
}

/// `MRISreallocVerticesAndFaces(mris, nvertices, nfaces)`: [`over_alloc_vertices_and_faces`] with
/// room for exactly the counts.
pub fn realloc_vertices_and_faces(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sg: &SurfGlobals,
    mris: &mut Mris,
    nvertices: i32,
    nfaces: i32,
) -> Result<(), Fatal> {
    over_alloc_vertices_and_faces(g, out, err, sg, mris, nvertices, nfaces, nvertices, nfaces)
}

fn copy_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Matrix,
) -> Result<Option<Matrix>, Fatal> {
    Ok(match matrix_copy(g, out, err, Some(m), None)? {
        Ret::New(m) => Some(m),
        Ret::Given | Ret::Null => None,
    })
}

/// `MRIScopyMetadata(source, target)`: into `target`, or into a new surface of the source's
/// vertex and face counts ([`alloc`]) when there is none: the source's `type`, `status`,
/// `hemisphere`, its volume geometry (`VOL_GEOM`'s assignment: fs.mri's `VolGeom::assign`), and
/// copies of its transform array (`LTAcopy`) and of its two Talairach matrices (`MatrixCopy`)
/// where the source has them (the target's own are replaced; where the source has none the
/// target's stay). `free_transform` is not copied.
pub fn copy_metadata(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sg: &SurfGlobals,
    source: &Mris,
    target: Option<Mris>,
) -> Result<Mris, Fatal> {
    let mut target = match target {
        Some(t) => t,
        None => alloc(g, out, err, sg, source.nvertices, source.nfaces)?,
    };
    target.r#type = source.r#type;
    target.status = source.status;
    target.hemisphere = source.hemisphere;
    target.vg.assign(g, &source.vg);
    if let Some(l) = &source.lta {
        target.lta = Some(lta_copy(l));
    }
    if let Some(m) = &source.SRASToTalSRAS_ {
        target.SRASToTalSRAS_ = copy_matrix(g, out, err, m)?;
    }
    if let Some(m) = &source.TalSRASToSRAS_ {
        target.TalSRASToSRAS_ = copy_matrix(g, out, err, m)?;
    }
    Ok(target)
}

/// `MRISctr(mris, max_vertices, max_faces, nvertices, nfaces)`: every field zeroed (`bzero`:
/// [`Mris::default`]), [`over_alloc_vertices_and_faces`], then `max_nsize` and `nsize` 1.
#[allow(clippy::too_many_arguments)]
pub fn ctr(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sg: &SurfGlobals,
    mris: &mut Mris,
    max_vertices: i32,
    max_faces: i32,
    nvertices: i32,
    nfaces: i32,
) -> Result<(), Fatal> {
    *mris = Mris::default();
    over_alloc_vertices_and_faces(
        g,
        out,
        err,
        sg,
        mris,
        max_vertices,
        max_faces,
        nvertices,
        nfaces,
    )?;
    mris.nsize = 1;
    mris.max_nsize = 1;
    Ok(())
}

/// `MRISdtr(mris)`: frees what the surface holds and sets those fields `NULL`: the extra
/// gradients, the labels, the colour table (`CTABfree`), each in-use vertex's topology arrays and
/// distances, the vertex and topology arrays, the face-normal caches, the faces (with each in-use
/// face's matrices), the edges and corners (with their in-use matrices), the transform array and
/// the Talairach matrices only when `free_transform` is set, the first `ncmds` command lines,
/// `m_sras2vox`, and the distance storage (each of `max_vertices` places, then the arrays). The
/// counts stay as they were.
pub fn dtr(g: &mut Globals, err: &mut dyn Write, mris: &mut Mris) {
    mris.dx2 = None;
    mris.dy2 = None;
    mris.dz2 = None;
    mris.labels = None;
    if mris.ct.is_some() {
        ctab_free(g, err, &mut mris.ct);
    }
    // the vertices' own arrays go with their array (VERTEX_TOPOLOGYdtr, VERTEXdtr: the distance
    // buffers belong to the storage, freed below)
    mris.vertices = None;
    mris.vertices_topology = None;
    mris.faceNormDeferredEntries = None;
    mris.faceNormCacheEntries = None;
    mris.faces = None;
    mris.edges = None;
    mris.corners = None;
    if mris.free_transform != 0 {
        mris.SRASToTalSRAS_ = None;
        mris.TalSRASToSRAS_ = None;
        if mris.lta.is_some() {
            lta_free(&mut mris.lta);
        }
    }
    for i in 0..nonneg(mris.ncmds).min(MAX_CMDS) {
        mris.cmdlines[i] = None;
    }
    mris.m_sras2vox = None;
    mris.dist_storage = None;
    mris.dist_orig_storage = None;
}

/// `MRISoverAlloc(max_vertices, max_faces, nvertices, nfaces)`: a new surface, [`ctr`].
pub fn over_alloc(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sg: &SurfGlobals,
    max_vertices: i32,
    max_faces: i32,
    nvertices: i32,
    nfaces: i32,
) -> Result<Mris, Fatal> {
    let mut mris = Mris::default();
    ctr(
        g,
        out,
        err,
        sg,
        &mut mris,
        max_vertices,
        max_faces,
        nvertices,
        nfaces,
    )?;
    Ok(mris)
}

/// `MRISalloc(nvertices, nfaces)`: [`over_alloc`] with room for exactly the counts.
pub fn alloc(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sg: &SurfGlobals,
    nvertices: i32,
    nfaces: i32,
) -> Result<Mris, Fatal> {
    over_alloc(g, out, err, sg, nvertices, nfaces, nvertices, nfaces)
}

/// `MRISfree(&mris)`: [`dtr`], the surface freed, `mris` `NULL`. A `NULL` surface is out of range
/// (FreeSurfer reads through it: a panic here).
pub fn free(g: &mut Globals, err: &mut dyn Write, pmris: &mut Option<Mris>) {
    let mut mris = pmris.take().expect("MRISfree of a NULL surface");
    dtr(g, err, &mut mris);
}

fn f6(x: f32) -> String {
    fmt_f(f64::from(x), 0, 6)
}

/// `mrisDumpVertex(file, mris, vno)`: one line `vno:<vno> ripflag:<r> marked:<m> nsizeCur:<c>
/// nsizeMax:<x> vnum:.. v2num:.. v3num:.. vtotal:.. nsizeCur:<c> nsizeMax:<x> x:%f y:%f z:%f`, then
/// for each of the vertex's [`vertex_vsize`] neighbours ` [i] v:<n> dist:%f dist_orig:%f` (-1 for
/// a `NULL` array) and a newline, then for each of its faces ` [i] f:<f> n:<n>` and a newline. A
/// neighbour array shorter than the size is memory FreeSurfer reads: a panic here.
pub fn dump_vertex(file: &mut dyn Write, mris: &Mris, vno: usize) {
    let vt = mris.topology(vno);
    let v = mris.vertex(vno);
    let _ = write!(
        file,
        "vno:{vno} ripflag:{} marked:{} nsizeCur:{} nsizeMax:{} vnum:{} v2num:{} v3num:{} vtotal:{} nsizeCur:{} nsizeMax:{} x:{} y:{} z:{}",
        v.ripflag,
        v.marked,
        vt.nsizeCur,
        vt.nsizeMax,
        vt.vnum,
        vt.v2num,
        vt.v3num,
        vt.vtotal,
        vt.nsizeCur,
        vt.nsizeMax,
        f6(v.x),
        f6(v.y),
        f6(v.z)
    );
    let vsize = nonneg(vertex_vsize(mris, vno));
    for i in 0..vsize {
        let n = vt.v.as_ref().expect("VERTEX_TOPOLOGY::v is NULL")[i];
        let d = v.dist.as_ref().map_or(-1.0, |d| d[i]);
        let o = v.dist_orig.as_ref().map_or(-1.0, |d| d[i]);
        let _ = write!(file, " [{i}] v:{n} dist:{} dist_orig:{}", f6(d), f6(o));
    }
    let _ = writeln!(file);
    for i in 0..usize::from(vt.num) {
        let f = vt.f.as_ref().expect("VERTEX_TOPOLOGY::f is NULL")[i];
        let n = vt.n.as_ref().expect("VERTEX_TOPOLOGY::n is NULL")[i];
        let _ = write!(file, " [{i}] f:{f} n:{n}");
    }
    let _ = writeln!(file);
}

/// `mrisDumpFace(mris, fno, file)`: `fno:<f> ripflag:<r> marked:<m>`, a newline, ` [i] v:<v>` for
/// its three corners, a newline.
pub fn dump_face(mris: &Mris, fno: usize, file: &mut dyn Write) {
    let f = mris.face(fno);
    let _ = writeln!(file, "fno:{fno} ripflag:{} marked:{}", f.ripflag, f.marked);
    for (i, v) in f.v.iter().enumerate() {
        let _ = write!(file, " [{i}] v:{v}");
    }
    let _ = writeln!(file);
}

/// `mrisDumpShape(file, mris)`: `mrisDumpShape {`, the counts (`nvertices:.. nfaces:..
/// max_nsize:.. nsize:..`), [`dump_vertex`] of the first ten vertices and [`dump_face`] of the
/// first ten faces, `} // mrisDumpShape`.
pub fn dump_shape(file: &mut dyn Write, mris: &Mris) {
    let _ = writeln!(file, "mrisDumpShape {{");
    let _ = writeln!(
        file,
        "nvertices:{} nfaces:{} max_nsize:{} nsize:{}",
        mris.nvertices, mris.nfaces, mris.max_nsize, mris.nsize
    );
    for vno in 0..nonneg(mris.nvertices.min(10)) {
        dump_vertex(file, mris, vno);
    }
    for fno in 0..nonneg(mris.nfaces.min(10)) {
        dump_face(mris, fno, file);
    }
    let _ = writeln!(file, "}} // mrisDumpShape");
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn realloc_to_zero_frees_only_an_existing_array() {
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        let mut p: Option<Vec<i32>> = None;
        assert!(c_realloc(&mut g, &mut p, 0, "x"));
        assert_eq!(p, Some(vec![]));
        assert!(!c_realloc(&mut g, &mut p, 0, "x"));
        assert_eq!(p, None);
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut p = Some(vec![1, 2]);
        assert!(c_realloc(&mut g, &mut p, 0, "x"));
        assert_eq!(p, Some(vec![]));
        assert!(g.notices.contains(&FS_117));
    }

    #[test]
    fn a_surface_starts_with_room_and_one_ring() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let m = over_alloc(&mut g, &mut o, &mut e, &SurfGlobals::default(), 5, 3, 2, 1).unwrap();
        assert_eq!((m.nvertices, m.max_vertices, m.nfaces, m.max_faces), (2, 5, 1, 3));
        assert_eq!((m.nsize, m.max_nsize), (1, 1));
        assert_eq!(m.vertices.as_ref().map(Vec::len), Some(5));
        assert_eq!(m.faces.as_ref().map(Vec::len), Some(3));
        assert_eq!(m.vg.xsize, 0.0);
    }
}
