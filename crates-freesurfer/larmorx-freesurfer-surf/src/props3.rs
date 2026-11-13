// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Queue item 3.1-50, part b (relay 18): the vertex normals of a coordinate set
//! (`mrisOrigNormalFace`, `mrisWhiteNormalFace`, `mrisPialNormalFace`, `mrisComputeOrigNormal`,
//! `mrisComputeWhiteNormal`, `mrisComputePialNormal`, `MRIScomputeNormal`), the averaged surface
//! normals (`MRIScomputeSurfaceNormals`), the closest vertices across the cortex
//! (`MRISfindClosestOrigVertices`, `MRISfindClosestPialVerticesCanonicalCoords`) and the cortical
//! thickness (`MRISmeasureCorticalThickness`: recon-all's thickness, through
//! `mris_place_surface --thickness`) of `utils/mrisurf_metricProperties.cpp`.
//!
//! Arithmetic through fs.core's `sse` operations in the official build's operand order
//! (`docs/findings/freesurfer-surfio.md` §20). Defects (`larmorx_core::defects`):
//! - **FS-368** (rule D): `MRIScomputeSurfaceNormals` saves the current positions but ends by
//!   restoring the set it was asked for, so a surface asked for its white, pial or original
//!   normals is left at those positions. Kept by default (its callers may rely on it; measured
//!   with their ports); [`SurfGlobals::fs368_restore_current`] with `fix` restores the current
//!   positions (noted).
//! - **FS-369** (wrong result): the same function gives an unripped vertex all of whose faces are
//!   ripped the normal of the vertex before it (0 for the first). Fixed by default (the vertex
//!   keeps its normal, noted); `reproduce` as FreeSurfer.
//! - **FS-370** (memory): the closest-vertex searches and the thickness keep their counts in 100
//!   places and their vertex lists in 100,000: a neighbourhood size of 100 or more, or a search
//!   reaching more than 100,000 vertices, reads and writes outside them. Both modes count every
//!   size and list every vertex (noted).

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_368, FS_369, FS_370};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::verbose_on;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, sqrt, sub};

use crate::aaa::SurfGlobals;
use crate::coords::Io;
use crate::mp_slow;
use crate::mris::{Mris, Vertex};
use crate::positions::{restore_vertex_positions, save_vertex_positions};
use crate::views::{
    CURRENT_VERTICES, ORIGINAL_VERTICES, PIAL_VERTICES, TMP_VERTICES, WHITE_VERTICES,
};

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// The coordinate sets the face normals take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Set {
    Original,
    White,
    Pial,
}

fn coords(v: &Vertex, set: Set) -> [f32; 3] {
    match set {
        Set::Original => [v.origx, v.origy, v.origz],
        Set::White => [v.whitex, v.whitey, v.whitez],
        Set::Pial => [v.pialx, v.pialy, v.pialz],
    }
}

/// `mrisNormalize(v)` as the official build inlines it into the vertex normals: `d = sqrt(z² + (x²
/// + y²))` in `float`; each component divided by `d` when `d > 0`.
fn normalize_zfirst(v: &mut [f32; 3]) {
    let d = sqrt(add(mul(v[2], v[2]), add(mul(v[0], v[0]), mul(v[1], v[1]))));
    if d > 0.0 {
        for x in v.iter_mut() {
            *x = div(*x, d);
        }
    }
}

/// `mrisOrigNormalFace`, `mrisWhiteNormalFace`, `mrisPialNormalFace` (`mris, fno, n, norm`): with
/// `v` corner `n` of face `fno`, `vn0` the one before and `vn1` the one after, the edges `v0 = v -
/// vn0` and `v1 = vn1 - v` of the set made unit length (`mrisNormalize`), their cross product
/// (compiled as `v0[1] v1[2] - v1[1] v0[2]`, `v0[2] v1[0] - v1[2] v0[0]`, `v0[0] v1[1] - v1[0]
/// v0[1]`), not normalized.
pub fn normal_face(mris: &Mris, fno: usize, n: usize, set: Set) -> [f32; 3] {
    use crate::base::mris_normalize;
    let f = &mris.face(fno).v;
    let n0 = if n == 0 { 2 } else { n - 1 };
    let n1 = if n == 2 { 0 } else { n + 1 };
    let vn0 = coords(mris.vertex(f[n0] as usize), set);
    let vn1 = coords(mris.vertex(f[n1] as usize), set);
    let v = coords(mris.vertex(f[n] as usize), set);
    let mut v0 = [sub(v[0], vn0[0]), sub(v[1], vn0[1]), sub(v[2], vn0[2])];
    let mut v1 = [sub(vn1[0], v[0]), sub(vn1[1], v[1]), sub(vn1[2], v[2])];
    mris_normalize(&mut v0);
    mris_normalize(&mut v1);
    [
        sub(mul(v0[1], v1[2]), mul(v1[1], v0[2])),
        sub(mul(v0[2], v1[0]), mul(v1[2], v0[0])),
        sub(mul(v0[0], v1[1]), mul(v1[0], v0[1])),
    ]
}

/// `mrisComputeOrigNormal`, `mrisComputeWhiteNormal`, `mrisComputePialNormal` (`mris, vno,
/// norm`): the sum of [`normal_face`] over the vertex's unripped faces, normalized as
/// [`normalize_zfirst`]; `ERROR_BADPARM` (and 0, 0, 0) without one.
pub fn compute_set_normal(mris: &Mris, vno: usize, set: Set) -> (i32, [f32; 3]) {
    let vt = mris.topology(vno);
    let mut norm = [0.0f32; 3];
    let mut num = 0;
    let faces = vt.f.as_deref().unwrap_or(&[]);
    let corners = vt.n.as_deref().unwrap_or(&[]);
    for k in 0..usize::from(vt.num) {
        let fno = faces[k] as usize;
        if mris.face(fno).ripflag != 0 {
            continue;
        }
        num += 1;
        let s = normal_face(mris, fno, usize::from(corners[k]), set);
        for i in 0..3 {
            norm[i] = add(norm[i], s[i]);
        }
    }
    if num == 0 {
        return (ERROR_BADPARM, norm);
    }
    normalize_zfirst(&mut norm);
    (NO_ERROR, norm)
}

/// `MRIScomputeNormal(mris, which, vno, &nx, &ny, &nz)`: the sum over the vertex's unripped faces
/// of the face normals of `which` (current: `mrisNormalFace`; original, white, pial:
/// [`normal_face`]; any other set ends the program at the first such face with `MRIScomputeNormal:
/// which = <which> not supported`, `ERROR_BADPARM`), normalized (`mrisNormalize`, `(x² + y²) +
/// z²`) and widened; `None` (`ERROR_BADPARM`, the outputs unwritten) without an unripped face.
pub fn compute_normal(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    mris: &Mris,
    which: i32,
    vno: usize,
) -> Result<Option<[f64; 3]>, Fatal> {
    let vt = mris.topology(vno);
    let mut norm = [0.0f32; 3];
    let mut num = 0;
    let faces = vt.f.as_deref().unwrap_or(&[]);
    let corners = vt.n.as_deref().unwrap_or(&[]);
    for k in 0..usize::from(vt.num) {
        let fno = faces[k] as usize;
        if mris.face(fno).ripflag != 0 {
            continue;
        }
        num += 1;
        let n = usize::from(corners[k]);
        let s = match which {
            CURRENT_VERTICES => mp_slow::normal_face(sg, mris, fno, n),
            ORIGINAL_VERTICES => normal_face(mris, fno, n, Set::Original),
            WHITE_VERTICES => normal_face(mris, fno, n, Set::White),
            PIAL_VERTICES => normal_face(mris, fno, n, Set::Pial),
            _ => {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_BADPARM,
                    &format!("MRIScomputeNormal: which = {which} not supported"),
                )?;
                unreachable!("ErrorExit with a non-zero code always ends the call")
            }
        };
        for i in 0..3 {
            norm[i] = add(norm[i], s[i]);
        }
    }
    if num == 0 {
        return Ok(None);
    }
    crate::base::mris_normalize(&mut norm);
    Ok(Some([
        f64::from(norm[0]),
        f64::from(norm[1]),
        f64::from(norm[2]),
    ]))
}

/// The normal fields of a coordinate set (`default`: the current normals).
fn normal_of(v: &mut Vertex, which: i32) -> (&mut f32, &mut f32, &mut f32) {
    match which {
        WHITE_VERTICES => (&mut v.wnx, &mut v.wny, &mut v.wnz),
        PIAL_VERTICES => (&mut v.pnx, &mut v.pny, &mut v.pnz),
        ORIGINAL_VERTICES => (&mut v.onx, &mut v.ony, &mut v.onz),
        _ => (&mut v.nx, &mut v.ny, &mut v.nz),
    }
}

/// `MRIScomputeSurfaceNormals(mris, which, navgs)`: the current positions saved into the temporary
/// set and the set `which` restored; each unripped vertex's normal of `which` (the current normal
/// for any other set) set to [`compute_normal`] (FS-369 without an unripped face); then `navgs`
/// times each unripped vertex's normal replaced by the normalized sum (in `float`, each term added
/// in `double`; normalized by the `float` length widened, unless it is below `FLT_EPSILON`) of its
/// own and its unripped immediate neighbours'; at the end the set `which` is restored again
/// (FS-368).
pub fn compute_surface_normals(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    which: i32,
    navgs: i32,
) -> Result<i32, Fatal> {
    save_vertex_positions(g, io, mris, TMP_VERTICES)?;
    restore_vertex_positions(g, io, mris, which)?;
    let mut last = [0.0f64; 3];
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let n = match compute_normal(g, sg, io, mris, which, vno)? {
            Some(n) => n,
            None => {
                g.notices.add(
                    &FS_369,
                    format!("MRIScomputeSurfaceNormals: vertex {vno} has no unripped face and keeps its normal (FreeSurfer gives it the previous vertex's)"),
                );
                if !g.notices.reproduce() {
                    continue;
                }
                last
            }
        };
        last = n;
        let (x, y, z) = normal_of(mris.vertex_mut(vno), which);
        (*x, *y, *z) = (n[0] as f32, n[1] as f32, n[2] as f32);
    }
    for _ in 0..navgs {
        for vno in 0..nverts(mris) {
            if mris.vertex(vno).ripflag != 0 {
                continue;
            }
            let mut td = {
                let (x, y, z) = normal_of(mris.vertex_mut(vno), which);
                [*x, *y, *z]
            };
            let vt = mris.topology(vno);
            let nbrs: Vec<i32> =
                vt.v.as_deref().unwrap_or(&[])[..usize::try_from(vt.vnum).unwrap_or(0)].to_vec();
            for nb in nbrs {
                let vn = mris.vertex_mut(nb as usize);
                if vn.ripflag != 0 {
                    continue;
                }
                let (x, y, z) = normal_of(vn, which);
                let n = [*x, *y, *z];
                for i in 0..3 {
                    td[i] = add64(f64::from(td[i]), f64::from(n[i])) as f32;
                }
            }
            let norm = f64::from(sqrt(add(
                add(mul(td[0], td[0]), mul(td[1], td[1])),
                mul(td[2], td[2]),
            )));
            // `!FZERO(norm)`: a NaN length divides too
            if !(norm.abs() < f64::from(f32::EPSILON)) {
                for x in &mut td {
                    *x = div64(f64::from(*x), norm) as f32;
                }
            }
            let v = mris.vertex_mut(vno);
            (v.tdx, v.tdy, v.tdz) = (td[0], td[1], td[2]);
        }
        for vno in 0..nverts(mris) {
            let v = mris.vertex_mut(vno);
            if v.ripflag != 0 {
                continue;
            }
            let td = (v.tdx, v.tdy, v.tdz);
            let (x, y, z) = normal_of(v, which);
            (*x, *y, *z) = td;
        }
    }
    let restore = if sg.fs368_restore_current && g.notices.fix() && which != CURRENT_VERTICES {
        g.notices.add(
            &FS_368,
            "MRIScomputeSurfaceNormals: the current positions are restored (FreeSurfer leaves the surface at the positions whose normals it computed)",
        );
        TMP_VERTICES
    } else {
        which
    };
    restore_vertex_positions(g, io, mris, restore)?;
    Ok(NO_ERROR)
}

/// What the closest-vertex searches compare: the positions a neighbour is measured at and the
/// normals that must point the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Search {
    /// From the vertex's original position to the neighbours' current ones, current normals.
    OrigToCurrent,
    /// From the vertex's white position to the neighbours' pial ones, white normals.
    WhiteToPial,
    /// From the neighbours' original positions to the vertex's current one, current normals.
    CurrentToOrig,
}

/// `%g` of a `float`, as `printf` writes it.
fn g6(x: f32) -> String {
    cfmt::g(f64::from(x), 6)
}

/// The vertex search the three functions share: the vertex `vno` marked 1, then for `ns` from 1 to
/// `nbhd_size` the unmarked unripped immediate neighbours of the vertices listed so far (those
/// marked before `ns - 1` excepted) marked `ns` and listed; each whose offset points along the
/// vertex's normal and whose normal does too (`dot >= 0`, in `float`) at a distance below the
/// smallest so far becomes the nearest. The list's marks are cleared at the end. Returns the
/// nearest distance, its ring and vertex.
#[allow(clippy::too_many_arguments)]
fn search(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    vno: usize,
    nbhd_size: i32,
    kind: Search,
    min_dist: f32,
    pass: char,
) -> (f32, i32, i32) {
    let v = mris.vertex(vno).clone();
    let (nx, ny, nz) = match kind {
        Search::WhiteToPial => (v.wnx, v.wny, v.wnz),
        _ => (v.nx, v.ny, v.nz),
    };
    let mut min_dist = min_dist;
    mris.vertex_mut(vno).marked = 1;
    let mut vlist: Vec<i32> = vec![vno as i32];
    let (mut min_n, mut min_vno) = (0i32, vno as i32);
    for ns in 1..=nbhd_size {
        let vtotal = vlist.len();
        for i in 0..vtotal {
            let li = vlist[i] as usize;
            let vn = mris.vertex(li);
            if vn.ripflag != 0 {
                continue;
            }
            if vn.marked != 0 && i32::from(vn.marked) < ns - 1 {
                continue;
            }
            let vt = mris.topology(li);
            let nbrs: Vec<i32> =
                vt.v.as_deref().unwrap_or(&[])[..usize::try_from(vt.vnum).unwrap_or(0)].to_vec();
            for (n, nb) in nbrs.into_iter().enumerate() {
                let vn2 = mris.vertex(nb as usize);
                if vn2.ripflag != 0 || vn2.marked != 0 {
                    continue;
                }
                vlist.push(nb);
                if vlist.len() == 100_001 {
                    g.notices.add(
                        &FS_370,
                        format!("vertex {vno}'s search lists more than 100,000 vertices (FreeSurfer writes past its list)"),
                    );
                }
                mris.vertex_mut(nb as usize).marked = ns as i16;
                let vn2 = mris.vertex(nb as usize);
                let (dx, dy, dz, n2) = match kind {
                    Search::OrigToCurrent => (
                        sub(vn2.x, v.origx),
                        sub(vn2.y, v.origy),
                        sub(vn2.z, v.origz),
                        [vn2.nx, vn2.ny, vn2.nz],
                    ),
                    Search::WhiteToPial => (
                        sub(vn2.pialx, v.whitex),
                        sub(vn2.pialy, v.whitey),
                        sub(vn2.pialz, v.whitez),
                        [vn2.wnx, vn2.wny, vn2.wnz],
                    ),
                    Search::CurrentToOrig => (
                        sub(v.x, vn2.origx),
                        sub(v.y, vn2.origy),
                        sub(v.z, vn2.origz),
                        [vn2.nx, vn2.ny, vn2.nz],
                    ),
                };
                let dot = add(add(mul(dx, nx), mul(dy, ny)), mul(dz, nz));
                if dot < 0.0 {
                    continue;
                }
                let dot = add(add(mul(n2[0], nx), mul(n2[1], ny)), mul(n2[2], nz));
                if dot < 0.0 {
                    continue;
                }
                let dist = sqrt(add(add(mul(dx, dx), mul(dy, dy)), mul(dz, dz)));
                if pass != ' ' && g.gdiag_no == vno as i32 {
                    let (o, c) = if pass == 'A' {
                        ([v.origx, v.origy, v.origz], [v.x, v.y, v.z])
                    } else {
                        ([vn2.origx, vn2.origy, vn2.origz], [v.x, v.y, v.z])
                    };
                    let _ = writeln!(
                        io.out,
                        "vno={vno} {pass} {n:3} {nb:6} ({},{},{}) ({},{},{}) {} {}",
                        g6(o[0]),
                        g6(o[1]),
                        g6(o[2]),
                        g6(c[0]),
                        g6(c[1]),
                        g6(c[2]),
                        g6(dist),
                        g6(min_dist)
                    );
                }
                if dist < min_dist {
                    min_n = ns;
                    min_dist = dist;
                    if min_n == nbhd_size && verbose_on(g) {
                        let _ =
                            writeln!(io.out, "{vno} --> {nb} = {}", fmt_f(f64::from(dist), 2, 3));
                    }
                    min_vno = nb;
                }
            }
        }
    }
    for &l in &vlist {
        let vn = mris.vertex_mut(l as usize);
        if vn.ripflag != 0 {
            continue;
        }
        vn.marked = 0;
    }
    (min_dist, min_n, min_vno)
}

/// The counts of nearest rings, `nbr_count[100]` (FS-370 for a ring of 100 or more).
struct Counts(Vec<i32>);

impl Counts {
    fn new(nbhd_size: i32) -> Counts {
        Counts(vec![
            0;
            usize::try_from(nbhd_size.max(99)).unwrap_or(99) + 1
        ])
    }
    fn add(&mut self, g: &mut Globals, n: i32) {
        if n >= 100 {
            g.notices.add(
                &FS_370,
                format!(
                    "a count at distance {n} is kept (FreeSurfer writes it past its 100 counts)"
                ),
            );
        }
        self.0[n as usize] += 1;
    }
    fn print(&self, g: &mut Globals, io: &mut Io, nbhd_size: i32) {
        if nbhd_size >= 100 {
            g.notices.add(
                &FS_370,
                format!("the counts up to distance {nbhd_size} are printed (FreeSurfer reads past its 100 counts)"),
            );
        }
        for n in 0..=nbhd_size {
            let _ = writeln!(io.out, "{} vertices at {n} distance", self.0[n as usize]);
        }
    }
}

/// The starting distance of a search: `sqrt(z² + (x² + y²))` of the vertex's own offset, in
/// `float`.
fn own_distance(d: [f32; 3]) -> f32 {
    sqrt(add(mul(d[2], d[2]), add(mul(d[0], d[0]), mul(d[1], d[1]))))
}

/// `MRISfindClosestOrigVertices(mris, nbhd_size)`: for each unripped vertex the nearest current
/// position within `nbhd_size` rings of neighbours (see `search`) to its original position,
/// starting from its own; its number stored in the curvature (as a `float`); then `<count>
/// vertices at <n> distance` for each ring from 0 to `nbhd_size` on standard output.
pub fn find_closest_orig_vertices(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    nbhd_size: i32,
) -> i32 {
    let mut counts = Counts::new(nbhd_size);
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let v = mris.vertex(vno);
        let d = own_distance([sub(v.x, v.origx), sub(v.y, v.origy), sub(v.z, v.origz)]);
        let (_, min_n, min_vno) =
            search(g, io, mris, vno, nbhd_size, Search::OrigToCurrent, d, ' ');
        counts.add(g, min_n);
        mris.vertex_mut(vno).curv = min_vno as f32;
    }
    counts.print(g, io, nbhd_size);
    NO_ERROR
}

/// `MRISfindClosestPialVerticesCanonicalCoords(mris, nbhd_size)`: as
/// [`find_closest_orig_vertices`] from the white position to the pial ones (white normals); the
/// nearest vertex's number stored in the curvature and its canonical position in the current one.
pub fn find_closest_pial_vertices_canonical_coords(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    nbhd_size: i32,
) -> i32 {
    let mut counts = Counts::new(nbhd_size);
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let v = mris.vertex(vno);
        let d = own_distance([
            sub(v.pialx, v.whitex),
            sub(v.pialy, v.whitey),
            sub(v.pialz, v.whitez),
        ]);
        let (_, min_n, min_vno) = search(g, io, mris, vno, nbhd_size, Search::WhiteToPial, d, ' ');
        counts.add(g, min_n);
        let c = mris.vertex(min_vno as usize);
        let (cx, cy, cz) = (c.cx, c.cy, c.cz);
        let v = mris.vertex_mut(vno);
        v.curv = min_vno as f32;
        (v.x, v.y, v.z) = (cx, cy, cz);
    }
    counts.print(g, io, nbhd_size);
    NO_ERROR
}

/// `MRISmeasureCorticalThickness(mris, nbhd_size, max_thick)`: the thickness of each unripped
/// vertex (0 for a ripped one) as the mean of two distances, each at most `max_thick`: from its
/// original (white) position to the nearest current (pial) position within `nbhd_size` rings
/// (starting from its own), and from its current position to the nearest original one; stored in
/// the curvature as `(gray-to-white + white-to-gray) * 0.5`. Prints `<vno> of <n> vertices
/// processed` every 25,000 vertices of each pass, `thickness calculation complete, <w>:<g>
/// truncations.` and the ring counts.
pub fn measure_cortical_thickness(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    nbhd_size: i32,
    max_thick: f32,
) -> i32 {
    let mut counts = Counts::new(nbhd_size);
    let (mut nwg_bad, mut ngw_bad) = (0, 0);
    let nv = mris.nvertices;
    for vno in 0..nverts(mris) {
        if vno % 25000 == 0 {
            let _ = writeln!(io.out, "{vno} of {nv} vertices processed");
        }
        if mris.vertex(vno).ripflag != 0 {
            mris.vertex_mut(vno).curv = 0.0;
            continue;
        }
        let v = mris.vertex(vno);
        let d = own_distance([sub(v.x, v.origx), sub(v.y, v.origy), sub(v.z, v.origz)]);
        let (mut min_dist, min_n, _) =
            search(g, io, mris, vno, nbhd_size, Search::OrigToCurrent, d, 'A');
        counts.add(g, min_n);
        if min_dist > max_thick {
            nwg_bad += 1;
            min_dist = max_thick;
        }
        mris.vertex_mut(vno).curv = min_dist;
    }
    for vno in 0..nverts(mris) {
        if vno % 25000 == 0 {
            let _ = writeln!(io.out, "{vno} of {nv} vertices processed");
        }
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let v = mris.vertex(vno);
        let d = own_distance([sub(v.x, v.origx), sub(v.y, v.origy), sub(v.z, v.origz)]);
        let (mut min_dist, min_n, _) =
            search(g, io, mris, vno, nbhd_size, Search::CurrentToOrig, d, 'B');
        counts.add(g, min_n);
        let curv = mris.vertex(vno).curv;
        if verbose_on(g) && f64::from(sub(curv, min_dist).abs()) > 4.0 {
            let _ = writeln!(
                io.out,
                "v {vno}, white->gray={}, gray->white={}",
                fmt_f(f64::from(curv), 2, 2),
                fmt_f(f64::from(min_dist), 2, 2)
            );
        }
        if min_dist > max_thick {
            min_dist = max_thick;
            ngw_bad += 1;
        }
        let c = mul(add(min_dist, curv), 0.5);
        mris.vertex_mut(vno).curv = c;
        if g.gdiag_no == vno as i32 {
            let _ = writeln!(io.out, "vno = {vno}, final measurment {}", g6(c));
        }
    }
    let _ = writeln!(
        io.out,
        "thickness calculation complete, {nwg_bad}:{ngw_bad} truncations."
    );
    counts.print(g, io, nbhd_size);
    NO_ERROR
}
