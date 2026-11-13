// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Queue item 3.1-51, its leftovers, second group (relay 24 of track D): the surface-quality
//! functions of `utils/mrisurf_metricProperties.cpp` (`MRISmaxEdgeStatToOverlay` 13544-13591,
//! `MRISprintSurfQualityStats` 13593-13615, `MRISprettyPrintSurfQualityStats` 13617-13650,
//! `mrisFindMiddleOfGray` 13200-13222) with the pieces of fs.surf they stand on, taken early:
//! the edge and corner topology (`utils/mrisurf_topology.cpp`: `MRIScountEdges` 491-506,
//! `MRISedges` 605-726, `MRIScorners` 736-804), the face, edge and corner metrics without their
//! gradients (`utils/surfgrad.cpp`: `MRISfaceMetric` 45-62, `MRISedgeMetric` 69-83,
//! `MRIScornerMetric` 91-110, `MRISfaceNormalFace` 120-213, `MRISedgeMetricEdge` 425-473 with
//! `DoGrad` 0, `SurfGradUnitVector` 2157-2200 without a gradient, `MRIScornerMetricCorner`
//! 2289-2300, `MRIScornerStats` 2347-2387), the statistics of `utils/mrisutils.cpp`
//! (`MRIStriangleAreaStats` 3515-3541, `MRISedgeStats` 3552-3590), `StuffVertexCoords`
//! (`mrisurf_metricProperties.cpp` 5801-5807) and `MRISaverageCurvatures`
//! (`utils/mrisurf_vals.cpp` 1953-1987). fs.surf's items (3.4-02 to 3.4-04) add the gradients
//! (`DoGrad` 1 to 3) to these functions.
//!
//! The face normal and the edges' unit vectors are computed in x87 `long double`
//! (`larmorx_core::x87::F80`), as FreeSurfer writes them; the rest through fs.core's `sse`
//! operations in the official build's operand order (the unit oracle's object code,
//! `docs/findings/freesurfer-surfio.md` §26). The parallel loops are element by element and
//! write only their element, so their order never shows. `acos` is the correctly rounded one
//! (glibc's double `acos` agrees with it on every input measured, `docs/findings/freesurfer-libm.md`),
//! with glibc's default NaN for an argument beyond one.
//!
//! Defects (`larmorx_core::defects`): FS-535 (the corners after `MRISedges` failed), FS-536 (the
//! quality printers with an empty list), FS-537 (the corner statistics on the edges' old unit
//! vectors), FS-538 (the edge statistics on the faces' old normals, or none).

use std::io::Write;

use larmorx_core::defects::{FS_535, FS_536, FS_537, FS_538};
use larmorx_core::x87::F80;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::dmatrix::{DMatrix, dvector_dot};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{Fatal, NO_ERROR};
use larmorx_freesurfer_core::sse::{self, add, add64, div, div64, mul, mul64};
use larmorx_freesurfer_core::utils::dlist_stats;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri, mri_alloc};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::aaa::{MriCorner, MriEdge, SurfGlobals};
use crate::coords::{self, Io, metric_properties_cpp};
use crate::metric::compute_curvature_min_max;
use crate::mp;
use crate::mris::Mris;
use crate::positions::{restore_vertex_positions, save_vertex_positions};
use crate::views::{ORIGINAL_VERTICES, TMP_VERTICES};

/// `M_PI`.
const M_PI: f64 = std::f64::consts::PI;

fn surfgrad_cpp() -> String {
    format!("{}/utils/surfgrad.cpp", diag::BUILD_ROOT)
}

/// glibc's `acos`: the correctly rounded value; beyond one in magnitude the default NaN glibc's
/// `(x - x) / (x - x)` makes on x86-64; a NaN passed on quieted.
fn acos(x: f64) -> f64 {
    if x.abs() > 1.0 {
        sse::DEFAULT_NAN64
    } else {
        larmorx_core::math::acos(x)
    }
}

/// `180 * acos(x) / M_PI` and `acos(x) * 180 / M_PI` as the official build computes both (the
/// arc cosine times 180, then divided by `M_PI`).
fn degrees(x: f64) -> f64 {
    div64(mul64(acos(x), 180.0), M_PI)
}

/// A `float` as an x87 instruction's memory operand (`fsubs m32fp`): exact, a signalling NaN kept
/// signalling (`fld`, [`F80::from_f32`], makes it quiet), so that of it and a quiet NaN the x87
/// takes the quiet one.
fn m32(x: f32) -> F80 {
    let b = x.to_bits();
    let frac = b & ((1u32 << 23) - 1);
    if (b >> 23) & 0xff != 0xff || frac == 0 || frac >> 22 != 0 {
        return F80::from_f32(x);
    }
    let sign = u128::from(b >> 31) << 79;
    F80::from_bits(sign | (0x7fffu128 << 64) | (1u128 << 63) | (u128::from(frac) << 40))
}

/// A `double` as an x87 memory operand (`fsubrl m64fp`), as [`m32`].
fn m64(x: f64) -> F80 {
    let b = x.to_bits();
    let frac = b & ((1u64 << 52) - 1);
    if (b >> 52) & 0x7ff != 0x7ff || frac == 0 || frac >> 51 != 0 {
        return F80::from_f64(x);
    }
    let sign = u128::from(b >> 63) << 79;
    F80::from_bits(sign | (0x7fffu128 << 64) | (1u128 << 63) | (u128::from(frac) << 11))
}

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

fn nfaces(mris: &Mris) -> usize {
    usize::try_from(mris.nfaces).unwrap_or(0)
}

/// The vertex's first `n` neighbours (`v`), fewer when the list is shorter.
fn nbrs(mris: &Mris, vno: usize, n: i16) -> &[i32] {
    let vt = mris.topology(vno);
    let v = vt.v.as_deref().unwrap_or(&[]);
    &v[..usize::try_from(n).unwrap_or(0).min(v.len())]
}

/// The vertex's faces (`f`, `num` of them).
fn vfaces(mris: &Mris, vno: usize) -> &[i32] {
    let vt = mris.topology(vno);
    let f = vt.f.as_deref().unwrap_or(&[]);
    &f[..usize::from(vt.num).min(f.len())]
}

/// Whether a vertex leaves an element out of a statistic: ripped, or under 0.5 in the mask
/// (`MRIgetVoxVal(mask, vno, 0, 0, 0) < 0.5`).
fn skip_vertex(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    mask: Option<&Mri>,
    vno: i32,
) -> Result<bool, Fatal> {
    let mut skip = mris.vertex(vno as usize).ripflag != 0;
    if let Some(m) = mask
        && f64::from(mri_get_vox_val(g, io.err, m, vno, 0, 0, 0, 0)?) < 0.5
    {
        skip = true;
    }
    Ok(skip)
}

// ---- topology ----------------------------------------------------------------------------------

/// `MRIScountEdges(mris)`: the neighbours `v[0..vnum]` of every vertex numbered at least the
/// vertex's own.
pub fn mris_count_edges(mris: &Mris) -> i32 {
    let mut n = 0i32;
    for vno in 0..nverts(mris) {
        for &x in nbrs(mris, vno, mris.topology(vno).vnum) {
            if (x as i64) < vno as i64 {
                continue;
            }
            n = n.wrapping_add(1);
        }
    }
    n
}

/// `MRISedges(surf)`: the edge table (`nedges` = [`mris_count_edges`], zeroed) filled vertex by
/// vertex: for each neighbour `v1 >= v0` of `v0` the edge `(v0, v1)`, its two faces (the faces
/// of `v0` shared with `v1`, in `v0`'s face order), the opposite corners and the corner matrix;
/// then every vertex's edge numbers (`e`, `vtotal` of them, zeroed, set for its 1-hop edges).
/// An edge with more than two faces prints `ERROR: MRISedges(): too many faces: ...`, one with
/// fewer `ERROR: MRISedges(): not enough faces ...`, and returns 1 at once: the edges after it
/// stay zero (between vertex 0 and itself, faces 0 and 0) and no vertex gets its edge numbers
/// (FS-535's cause). Returns 0.
pub fn mris_edges(io: &mut Io, surf: &mut Mris) -> i32 {
    let ne = mris_count_edges(surf);
    surf.nedges = ne;
    surf.edges = Some(vec![MriEdge::default(); usize::try_from(ne).unwrap_or(0)]);
    let mut edgeno = 0usize;
    for v0 in 0..nverts(surf) {
        let vtx0 = v0 as i32;
        let n1 = nbrs(surf, v0, surf.topology(v0).vnum).to_vec();
        for vtx1 in n1 {
            if vtx1 < vtx0 {
                continue;
            }
            let f0s = vfaces(surf, v0).to_vec();
            let f1s = vfaces(surf, vtx1 as usize).to_vec();
            let mut e = MriEdge {
                vtxno: [vtx0, vtx1, 0, 0],
                ..MriEdge::default()
            };
            let mut k = 0usize;
            for (n, &fa) in f0s.iter().enumerate() {
                for (m, &fb) in f1s.iter().enumerate() {
                    if fa != fb {
                        continue;
                    }
                    if k > 1 {
                        let _ = writeln!(
                            io.out,
                            "ERROR: MRISedges(): too many faces: {vtx0} {vtx1} n={n}, m={m}, k={k}"
                        );
                        store_edge(surf, edgeno, e);
                        return 1;
                    }
                    let fv = surf.face(fa as usize).v;
                    for (c, &x) in fv.iter().take(2).enumerate() {
                        if x == vtx0 {
                            e.corner[k][0] = c as u8;
                        }
                        if x == vtx1 {
                            e.corner[k][1] = c as u8;
                        }
                    }
                    e.faceno[k] = fa;
                    k += 1;
                }
            }
            if k != 2 {
                let _ = writeln!(
                    io.out,
                    "ERROR: MRISedges(): not enough faces {vtx0} {vtx1} k={k}"
                );
                store_edge(surf, edgeno, e);
                return 1;
            }
            for k in 0..2 {
                let fv = surf.face(e.faceno[k] as usize).v;
                if let Some(&x) = fv.iter().find(|&&x| x != vtx0 && x != vtx1) {
                    e.vtxno[k + 2] = x;
                }
            }
            for n in 0..4 {
                let m = e.vtxno[n];
                for k in 0..2 {
                    let fv = surf.face(e.faceno[k] as usize).v;
                    if let Some(c) = fv.iter().position(|&x| x == m) {
                        e.corner[n][k] = c as u8;
                    }
                }
            }
            e.corner[2][1] = 9;
            e.corner[3][0] = 9;
            store_edge(surf, edgeno, e);
            edgeno += 1;
        }
    }
    for vno in 0..nverts(surf) {
        let vt = surf.topology_mut(vno);
        vt.e = Some(vec![0; usize::try_from(vt.vtotal).unwrap_or(0)]);
    }
    for edgeno in 0..usize::try_from(surf.nedges).unwrap_or(0) {
        let [a, b, ..] = surf.edges.as_ref().expect("the edges")[edgeno].vtxno;
        for (x, y) in [(a, b), (b, a)] {
            let vt = surf.topology_mut(x as usize);
            let tot = usize::try_from(vt.vtotal).unwrap_or(0);
            let v = vt.v.clone().unwrap_or_default();
            let e = vt.e.as_mut().expect("allocated above");
            for k in 0..tot.min(v.len()) {
                if v[k] == y {
                    e[k] = edgeno as i32;
                }
            }
        }
    }
    0
}

/// The edge as far as it was filled, in its place (FreeSurfer fills the table in place).
fn store_edge(surf: &mut Mris, edgeno: usize, e: MriEdge) {
    if let Some(slot) = surf.edges.as_mut().and_then(|v| v.get_mut(edgeno)) {
        *slot = e;
    }
}

/// `MRIScorners(surf)`: nothing when the corners exist; otherwise the edges first
/// ([`mris_edges`], when there are none), then three corners per face (vertex `k` of the face,
/// then the next two around it) and each corner's two edges, from its first vertex's edge
/// numbers, with their directions (+1 when the edge starts at the corner's vertex). A corner whose
/// vertex has no such neighbour prints `MRIScorners(): ERROR: could not find an edge for corner
/// ...` and keeps edge 0. FS-535: when `MRISedges` stopped early, the vertices have no edge
/// numbers and FreeSurfer crashes at the first corner that needs one; larmorx stops there with an
/// error and leaves the surface without corners (returns 1). Returns 0.
pub fn mris_corners(g: &mut Globals, io: &mut Io, surf: &mut Mris) -> Result<i32, Fatal> {
    if surf.corners.is_some() {
        return Ok(0);
    }
    if surf.edges.is_none() {
        mris_edges(io, surf);
    }
    let nc = 3usize * nfaces(surf);
    let mut corners = vec![MriCorner::default(); nc];
    let mut cornerno = 0usize;
    for faceno in 0..nfaces(surf) {
        let fv = surf.face(faceno).v;
        for k in 0..3 {
            let c = &mut corners[cornerno];
            c.cornerno = cornerno as i32;
            c.faceno = faceno as i32;
            c.vtxno = [fv[k], fv[(k + 1) % 3], fv[(k + 2) % 3]];
            cornerno += 1;
        }
    }
    for (cornerno, c) in corners.iter_mut().enumerate() {
        let vt = surf.topology(c.vtxno[0] as usize);
        let tot = usize::try_from(vt.vtotal).unwrap_or(0);
        let v = vt.v.as_deref().unwrap_or(&[]);
        for nthedge in 0..2 {
            let mut hit = false;
            for k in 0..tot.min(v.len()) {
                if v[k] != c.vtxno[nthedge + 1] {
                    continue;
                }
                let Some(&en) = vt.e.as_deref().and_then(|e| e.get(k)) else {
                    g.notices.add(
                        &FS_535,
                        format!(
                            "MRIScorners: corner {cornerno}'s vertex {} has no edge numbers (MRISedges stopped early; FreeSurfer crashes)",
                            c.vtxno[0]
                        ),
                    );
                    if g.notices.reproduce() {
                        return Err(Fatal::Exit(139));
                    }
                    let _ = writeln!(
                        io.out,
                        "ERROR: MRIScorners(): the surface's edges are not built (MRISedges failed): no corners"
                    );
                    surf.ncorners = 0;
                    surf.corners = None;
                    return Ok(1);
                };
                c.edgeno[nthedge] = en;
                let e0 = surf.edges.as_ref().expect("the edges")[en as usize].vtxno[0];
                c.edgedir[nthedge] = if e0 == c.vtxno[0] { 1 } else { -1 };
                hit = true;
                break;
            }
            if !hit {
                let _ = writeln!(
                    io.out,
                    "MRIScorners(): ERROR: could not find an edge for corner {cornerno}, edge {nthedge}, {}->{}",
                    c.vtxno[0],
                    c.vtxno[nthedge + 1]
                );
                let _ = io.out.flush();
            }
        }
    }
    surf.ncorners = nc as i32;
    surf.corners = Some(corners);
    Ok(0)
}

// ---- metrics -----------------------------------------------------------------------------------

/// `MRISfaceNormalFace(surf, faceno, pc, pcL)`: the face's normal in `long double`: `a = v1 - v0`,
/// `b = v2 - v1` (with `MRISfaceNormalFace_AddDelta` added to vertex `AddDeltaVertex` 0 to 2 when
/// it is set), `c = a x b`, `cL = sqrt(c . c)`; `*pcL = cL` and `*pc = c` (a 3 x 1 matrix made
/// when there is none) when given; the face's `norm` (made when there is none) `c / cL` as
/// `double`s. A face outside the surface prints `MRISfaceNormalFace(): faceno = <f> (0,<n>)` and
/// returns 1. Returns 0.
pub fn mris_face_normal_face(
    sg: &SurfGlobals,
    io: &mut Io,
    surf: &mut Mris,
    faceno: i32,
    pc: Option<&mut Option<DMatrix>>,
    pcl: Option<&mut f64>,
) -> i32 {
    if faceno < 0 || faceno >= surf.nfaces {
        let _ = writeln!(
            io.out,
            "MRISfaceNormalFace(): faceno = {faceno} (0,{})",
            surf.nfaces
        );
        return 1;
    }
    let fv = surf.face(faceno as usize).v;
    let p = |k: usize| {
        let v = surf.vertex(fv[k] as usize);
        [v.x, v.y, v.z]
    };
    let (p0, p1, p2) = (p(0), p(1), p(2));
    // `flds v1; fsubs v0` and `fsubrs v2`: v1 loaded (quieted), v0 and v2 memory operands
    let mut a: [F80; 3] = std::array::from_fn(|k| F80::from_f32(p1[k]) - m32(p0[k]));
    let mut b: [F80; 3] = std::array::from_fn(|k| m32(p2[k]) - F80::from_f32(p1[k]));
    let d = sg.face_normal_add_delta;
    match sg.face_normal_add_delta_vertex {
        0 => {
            for k in 0..3 {
                a[k] -= d[k];
            }
        }
        1 => {
            for k in 0..3 {
                a[k] += d[k];
                b[k] -= d[k];
            }
        }
        2 => {
            for k in 0..3 {
                b[k] += d[k];
            }
        }
        _ => {}
    }
    let c = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let cl = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
    if let Some(pcl) = pcl {
        *pcl = cl.to_f64();
    }
    if let Some(pc) = pc {
        let m = pc.get_or_insert_with(|| DMatrix::zeros(3, 1));
        for k in 0..3 {
            m.set(k as i32 + 1, 1, c[k].to_f64());
        }
    }
    let f = surf.face_mut(faceno as usize);
    let norm = f.norm.get_or_insert_with(|| DMatrix::zeros(3, 1));
    for k in 0..3 {
        norm.set(k as i32 + 1, 1, (c[k] / cl).to_f64());
    }
    0
}

/// `MRISfaceMetric(surf, 0)`: every face's normal ([`mris_face_normal_face`]). Returns 0.
pub fn mris_face_metric(sg: &SurfGlobals, io: &mut Io, surf: &mut Mris) -> i32 {
    for faceno in 0..surf.nfaces.max(0) {
        mris_face_normal_face(sg, io, surf, faceno, None, None);
    }
    0
}

/// `SurfGradUnitVector(v0, v1, u, NULL)`: `w = v1 - v0` and `L = sqrt(w . w)` in `long double`;
/// `u = w / L` as `double`s (zero when `L` is 0); returns `L`.
pub fn surf_grad_unit_vector(v0: &[f64; 3], v1: &[f64; 3], u: &mut [f64; 3]) -> F80 {
    // `fldl v0; fsubrl v1`: v0 loaded (quieted), v1 a memory operand
    let w: [F80; 3] = std::array::from_fn(|k| m64(v1[k]) - F80::from_f64(v0[k]));
    let mut l = F80::ZERO;
    for x in w {
        l += x * x;
    }
    let l = l.sqrt();
    if l == F80::ZERO {
        *u = [0.0; 3];
        return l;
    }
    for k in 0..3 {
        u[k] = (w[k] / l).to_f64();
    }
    l
}

/// `StuffVertexCoords(surf, vno, p)`: the vertex's current coordinates as `double`s.
pub fn stuff_vertex_coords(surf: &Mris, vno: i32) -> [f64; 3] {
    let v = surf.vertex(vno as usize);
    [f64::from(v.x), f64::from(v.y), f64::from(v.z)]
}

/// The edge's length and unit vector from its two vertices' current coordinates.
fn unit(surf: &Mris, e: &MriEdge) -> (f64, [f64; 3]) {
    let (a, b) = (
        stuff_vertex_coords(surf, e.vtxno[0]),
        stuff_vertex_coords(surf, e.vtxno[1]),
    );
    let mut u = [0.0; 3];
    let l = surf_grad_unit_vector(&a, &b, &mut u);
    (l.to_f64(), u)
}

/// `MRISedgeMetricEdge(surf, edgeno, 0)`: the edge's length and unit vector
/// ([`surf_grad_unit_vector`]), its faces' areas and the larger (`area[0] > area[1]`, else the
/// second), the dot product of its faces' normals (`DVectorDot`, clamped to [-1, 1]) and the hinge
/// angle `acos(dot) * 180 / M_PI`. A dot product beyond 1000 in magnitude (the `DVectorDot`
/// error's -10e10) prints three lines of FreeSurfer's and returns 1; FS-538: a face without a
/// normal crashes FreeSurfer while it prints them; larmorx computes the missing normal first
/// ([`mris_face_normal_face`]). Returns 0.
pub fn mris_edge_metric_edge(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    surf: &mut Mris,
    edgeno: i32,
) -> Result<i32, Fatal> {
    let mut e = surf.edges.as_ref().expect("the edges")[edgeno as usize].clone();
    let (len, u) = unit(surf, &e);
    e.len = len;
    e.u = u;
    let (fa, fb) = (e.faceno[0], e.faceno[1]);
    let missing = surf.face(fa as usize).norm.is_none() || surf.face(fb as usize).norm.is_none();
    if missing && !g.notices.reproduce() {
        g.notices.add(
            &FS_538,
            format!("MRISedgeMetricEdge: edge {edgeno}'s face normal computed first (FreeSurfer has none and crashes)"),
        );
        for f in [fa, fb] {
            if surf.face(f as usize).norm.is_none() {
                mris_face_normal_face(sg, io, surf, f, None, None);
            }
        }
    }
    let (a0, a1) = (surf.face(fa as usize).area, surf.face(fb as usize).area);
    e.area = [f64::from(a0), f64::from(a1)];
    e.maxarea = if a0 > a1 { e.area[0] } else { e.area[1] };
    e.dot = dvector_dot(
        io.out,
        surf.face(fa as usize).norm.as_ref(),
        surf.face(fb as usize).norm.as_ref(),
    );
    if e.dot.abs() > 1000.0 {
        let _ = writeln!(
            io.out,
            "ERROR: MRISedgeMetricEdge(): edgeno={edgeno} DoGrad=0"
        );
        let _ = writeln!(
            io.out,
            "   vno0={} vno1={} fno0={fa} fno1={fb}",
            e.vtxno[0], e.vtxno[1]
        );
        let (Some(n0), Some(n1)) = (
            surf.face(fa as usize).norm.as_ref(),
            surf.face(fb as usize).norm.as_ref(),
        ) else {
            g.notices.add(
                &FS_538,
                format!("MRISedgeMetricEdge: edge {edgeno}'s face has no normal (FreeSurfer crashes)"),
            );
            return Err(Fatal::Exit(139));
        };
        let f6 = |x: f64| fmt_f(x, 0, 6);
        let _ = writeln!(
            io.out,
            "   f0->norm ({},{},{}) f0->norm ({},{},{})",
            f6(n0.get(1, 1)),
            f6(n0.get(2, 1)),
            f6(n0.get(3, 1)),
            f6(n1.get(1, 1)),
            f6(n1.get(2, 1)),
            f6(n1.get(3, 1))
        );
        let _ = writeln!(io.out, "   break {}:462", surfgrad_cpp());
        surf.edges.as_mut().expect("the edges")[edgeno as usize] = e;
        return Ok(1);
    }
    if e.dot > 1.0 {
        e.dot = 1.0;
    }
    if e.dot < -1.0 {
        e.dot = -1.0;
    }
    e.angle = degrees(e.dot);
    surf.edges.as_mut().expect("the edges")[edgeno as usize] = e;
    Ok(0)
}

/// `MRISedgeMetric(surf, 0)`: every edge's metric ([`mris_edge_metric_edge`]). Returns 0.
pub fn mris_edge_metric(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    surf: &mut Mris,
) -> Result<i32, Fatal> {
    for edgeno in 0..surf.nedges.max(0) {
        mris_edge_metric_edge(g, sg, io, surf, edgeno)?;
    }
    Ok(0)
}

/// `MRIScornerMetricCorner(surf, cornerno, 0)`: the dot product of the corner's two edges' unit
/// vectors, each times its direction, summed over x, y, z from 0; the corner's angle
/// `180 * acos(dot) / M_PI`. Returns the dot product.
pub fn mris_corner_metric_corner(surf: &mut Mris, cornerno: i32) -> f64 {
    let c = &surf.corners.as_ref().expect("the corners")[cornerno as usize];
    let edges = surf.edges.as_ref().expect("the edges");
    let (u1, u2) = (
        edges[c.edgeno[0] as usize].u,
        edges[c.edgeno[1] as usize].u,
    );
    let (d0, d1) = (f64::from(c.edgedir[0]), f64::from(c.edgedir[1]));
    // the official build's order: (u1 d0)(u2 d1) for x and y, (d1 u2)(d0 u1) for z, each added
    // to the sum as `term + sum`
    let mut dot = add64(mul64(mul64(u1[0], d0), mul64(u2[0], d1)), 0.0);
    dot = add64(mul64(mul64(u1[1], d0), mul64(u2[1], d1)), dot);
    dot = add64(mul64(mul64(d1, u2[2]), mul64(d0, u1[2])), dot);
    let c = &mut surf.corners.as_mut().expect("the corners")[cornerno as usize];
    c.dot = dot;
    c.angle = degrees(dot);
    dot
}

/// `MRIScornerMetric(surf, 0)`: the corners first when there are none ([`mris_corners`]), then
/// every corner's metric. Returns 0.
pub fn mris_corner_metric(g: &mut Globals, io: &mut Io, surf: &mut Mris) -> Result<i32, Fatal> {
    if surf.corners.is_none() {
        mris_corners(g, io, surf)?;
    }
    for cornerno in 0..surf.ncorners.max(0) {
        mris_corner_metric_corner(surf, cornerno);
    }
    Ok(0)
}

// ---- statistics --------------------------------------------------------------------------------

/// `MRIStriangleAreaStats(surf, mask, NULL)`: `MRIScomputeMetricProperties`, then
/// `[n, mean, stddev, min, max]` (`DListStats`) of the areas of the faces none of whose vertices
/// is ripped or under 0.5 in the mask; `None` after `DListStats`' error when there is none.
pub fn mris_triangle_area_stats(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    surf: &mut Mris,
    mask: Option<&Mri>,
) -> Result<Option<[f64; 5]>, Fatal> {
    mp::compute_metric_properties(g, sg, io, surf)?;
    let mut area = Vec::with_capacity(nfaces(surf));
    for fno in 0..nfaces(surf) {
        let fv = surf.face(fno).v;
        let mut skip = false;
        for &vno in &fv {
            skip |= skip_vertex(g, io, surf, mask, vno)?;
        }
        if !skip {
            area.push(f64::from(surf.face(fno).area));
        }
    }
    Ok(dlist_stats(io.out, Some(area.as_slice()), area.len() as i32))
}

/// FS-538's correction where a statistic reads the faces' normals: every face's normal computed
/// again, noted when one was missing or changes (FreeSurfer uses the last ones computed, from
/// older coordinates, or crashes when there are none).
fn refresh_normals(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    surf: &mut Mris,
    who: &str,
) {
    let old: Vec<Option<DMatrix>> = (0..nfaces(surf))
        .map(|f| surf.face(f).norm.clone())
        .collect();
    mris_face_metric(sg, io, surf);
    let changed = old.iter().enumerate().filter(|(f, n)| {
        let new = surf.face(*f).norm.as_ref();
        match (n, new) {
            (Some(a), Some(b)) => a
                .data()
                .iter()
                .zip(b.data())
                .any(|(x, y)| x.to_bits() != y.to_bits()),
            _ => true,
        }
    });
    let n = changed.count();
    if n > 0 {
        g.notices.add(
            &FS_538,
            format!("{who}: {n} face normals computed again for the current coordinates (FreeSurfer uses older ones, or crashes without them)"),
        );
    }
}

/// FS-537's correction: the edges' lengths and unit vectors computed again before the corners
/// read them, noted when one changes (FreeSurfer uses the last edge metric's, from older
/// coordinates, or zero vectors when none ran: every corner then reads 90 degrees).
fn refresh_units(g: &mut Globals, surf: &mut Mris) {
    let ne = usize::try_from(surf.nedges).unwrap_or(0);
    let mut n = 0usize;
    for edgeno in 0..ne {
        let e = surf.edges.as_ref().expect("the edges")[edgeno].clone();
        let (len, u) = unit(surf, &e);
        let same = len.to_bits() == e.len.to_bits()
            && u.iter().zip(e.u).all(|(a, b)| a.to_bits() == b.to_bits());
        if !same {
            n += 1;
            let slot = &mut surf.edges.as_mut().expect("the edges")[edgeno];
            slot.len = len;
            slot.u = u;
        }
    }
    if n > 0 {
        g.notices.add(
            &FS_537,
            format!("MRIScornerStats: {n} edges' unit vectors computed again for the current coordinates (FreeSurfer uses the last edge metric's)"),
        );
    }
}

/// `MRISedgeStats(surf, metricid, mask, NULL)`: the edges first when there are none
/// ([`mris_edges`]), `MRIScomputeMetricProperties`, the edge metric ([`mris_edge_metric`]), then
/// the statistics of the length (0), dot product (1) or hinge angle (2) of the edges none of whose
/// four vertices is ripped or under 0.5 in the mask. Another `metricid` prints `ERROR:
/// MRISedgeStats() metricid <m> unrecognized` at the first edge kept and gives `None`.
pub fn mris_edge_stats(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    surf: &mut Mris,
    metricid: i32,
    mask: Option<&Mri>,
) -> Result<Option<[f64; 5]>, Fatal> {
    if surf.edges.is_none() {
        mris_edges(io, surf);
    }
    mp::compute_metric_properties(g, sg, io, surf)?;
    if !g.notices.reproduce() {
        refresh_normals(g, sg, io, surf, "MRISedgeStats");
    }
    mris_edge_metric(g, sg, io, surf)?;
    let mut metric = Vec::new();
    for edgeno in 0..usize::try_from(surf.nedges).unwrap_or(0) {
        let e = surf.edges.as_ref().expect("the edges")[edgeno].clone();
        let mut skip = false;
        for &vno in &e.vtxno {
            skip |= skip_vertex(g, io, surf, mask, vno)?;
        }
        if skip {
            continue;
        }
        metric.push(match metricid {
            0 => e.len,
            1 => e.dot,
            2 => e.angle,
            _ => {
                let _ = writeln!(
                    io.out,
                    "ERROR: MRISedgeStats() metricid {metricid} unrecognized"
                );
                return Ok(None);
            }
        });
    }
    Ok(dlist_stats(io.out, Some(metric.as_slice()), metric.len() as i32))
}

/// `MRIScornerStats(surf, metricid, mask, NULL)`: the corners first when there are none
/// ([`mris_corners`]), `MRIScomputeMetricProperties`, the corner metric ([`mris_corner_metric`]),
/// then the statistics of the dot product (0) or angle (1) of the corners none of whose three
/// vertices is ripped or under 0.5 in the mask. Another `metricid` prints `ERROR:
/// MRIScornerStats() metricid <m> unrecognized` at the first corner kept and gives `None`.
/// FS-537: larmorx computes the edges' unit vectors for the current coordinates first.
pub fn mris_corner_stats(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    surf: &mut Mris,
    metricid: i32,
    mask: Option<&Mri>,
) -> Result<Option<[f64; 5]>, Fatal> {
    if surf.corners.is_none() {
        mris_corners(g, io, surf)?;
    }
    mp::compute_metric_properties(g, sg, io, surf)?;
    if !g.notices.reproduce() && surf.corners.is_some() {
        refresh_units(g, surf);
    }
    mris_corner_metric(g, io, surf)?;
    let mut metric = Vec::new();
    for cornerno in 0..usize::try_from(surf.ncorners).unwrap_or(0) {
        let c = surf.corners.as_ref().expect("the corners")[cornerno].clone();
        let mut skip = false;
        for &vno in &c.vtxno {
            skip |= skip_vertex(g, io, surf, mask, vno)?;
        }
        if skip {
            continue;
        }
        metric.push(match metricid {
            0 => c.dot,
            1 => c.angle,
            _ => {
                let _ = writeln!(
                    io.out,
                    "ERROR: MRIScornerStats() metricid {metricid} unrecognized"
                );
                return Ok(None);
            }
        });
    }
    Ok(dlist_stats(io.out, Some(metric.as_slice()), metric.len() as i32))
}

/// `MRISmaxEdgeStatToOverlay(surf, metricid, mask)`: the face metric, the edges when there are
/// none, `MRIScomputeMetricProperties` and the edge metric; then a float overlay of one voxel per
/// vertex holding, for each of an edge's two ends, the edge statistic (length 0, dot product 1,
/// hinge angle 2) largest in magnitude over the edges kept (none of their vertices ripped or
/// under 0.5 in the mask: both ends for the length, all four for the others); prints
/// `MRISmaxEdgeStatToOverlay(): metric <m>   avg <mean>` (`%g`; `-nan` when no edge is kept).
/// Another `metricid` prints `ERROR: MRISmaxEdgeStatToOverlay() metricid <m> unrecognized` at the
/// first edge kept and gives `None`.
pub fn mris_max_edge_stat_to_overlay(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    surf: &mut Mris,
    metricid: i32,
    mask: Option<&Mri>,
) -> Result<Option<Mri>, Fatal> {
    mris_face_metric(sg, io, surf);
    if surf.edges.is_none() {
        mris_edges(io, surf);
    }
    mp::compute_metric_properties(g, sg, io, surf)?;
    mris_edge_metric(g, sg, io, surf)?;
    let mut overlay = mri_alloc(g, io.out, io.err, surf.nvertices, 1, 1, MRI_FLOAT)?;
    let (mut sum, mut nhits) = (0.0f64, 0i32);
    let nvmax = if metricid == 0 { 2 } else { 4 };
    for edgeno in 0..usize::try_from(surf.nedges).unwrap_or(0) {
        let e = surf.edges.as_ref().expect("the edges")[edgeno].clone();
        let mut skip = false;
        for &vno in &e.vtxno[..nvmax] {
            skip |= skip_vertex(g, io, surf, mask, vno)?;
        }
        if skip {
            continue;
        }
        let val = match metricid {
            0 => e.len,
            1 => e.dot,
            2 => e.angle,
            _ => {
                let _ = writeln!(
                    io.out,
                    "ERROR: MRISmaxEdgeStatToOverlay() metricid {metricid} unrecognized"
                );
                return Ok(None);
            }
        };
        for &vno in &e.vtxno[..2] {
            let ov = f64::from(mri_get_vox_val(g, io.err, &overlay, vno, 0, 0, 0, 0)?);
            if ov.abs() < val.abs() {
                mri_set_vox_val(g, io.err, &mut overlay, vno, 0, 0, 0, val as f32, 0)?;
            }
        }
        sum = add64(sum, val);
        nhits = nhits.wrapping_add(1);
    }
    let avg = div64(sum, f64::from(nhits));
    let _ = writeln!(
        io.out,
        "MRISmaxEdgeStatToOverlay(): metric {metricid}   avg {}",
        larmorx_core::cfmt::g(avg, 6)
    );
    Ok(Some(overlay))
}

/// The four statistics both printers take: the triangle areas, the corner angles (degrees), the
/// edge lengths and the hinge angles (degrees).
fn quality_stats(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    surf: &mut Mris,
) -> Result<[Option<[f64; 5]>; 4], Fatal> {
    let a = mris_triangle_area_stats(g, sg, io, surf, None)?;
    let c = mris_corner_stats(g, sg, io, surf, 1, None)?;
    let e = mris_edge_stats(g, sg, io, surf, 0, None)?;
    let h = mris_edge_stats(g, sg, io, surf, 2, None)?;
    Ok([a, c, e, h])
}

/// FS-536: a statistic without elements (`DListStats` gave `NULL`): FreeSurfer crashes when it
/// prints it, after the groups before it (`reproduce`: those written, then the crash); larmorx
/// prints none of the summary and the printer returns 1.
fn empty_list(
    g: &mut Globals,
    fp: &mut dyn Write,
    printed: &str,
    who: &str,
) -> Result<i32, Fatal> {
    g.notices.add(
        &FS_536,
        format!("{who}: a statistic has no element (FreeSurfer crashes printing it)"),
    );
    if g.notices.reproduce() {
        let _ = fp.write_all(printed.as_bytes());
        let _ = fp.flush();
        return Err(Fatal::Exit(139));
    }
    Ok(1)
}

/// `MRISprintSurfQualityStats(fp, surf)`: the mean, standard deviation, minimum and maximum
/// (`%7.5f`, two spaces after each group) of the triangle areas, corner angles, edge lengths and
/// hinge angles, on one line. FS-536 when a statistic has no element (returns 1). Returns 0.
pub fn mris_print_surf_quality_stats(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    fp: &mut dyn Write,
    surf: &mut Mris,
) -> Result<i32, Fatal> {
    let stats = quality_stats(g, sg, io, surf)?;
    let mut line = String::new();
    for s in &stats {
        let Some(s) = s else {
            return empty_list(g, fp, &line, "MRISprintSurfQualityStats");
        };
        for x in &s[1..5] {
            line.push_str(&fmt_f(*x, 7, 5));
            line.push(' ');
        }
        line.push(' ');
    }
    let _ = writeln!(fp, "{line}");
    let _ = fp.flush();
    Ok(0)
}

/// `MRISprettyPrintSurfQualityStats(fp, surf)`: the face metric first, then one line per
/// statistic: `Area`, `Corner`, `Edge`, `Hinge` (left-aligned in 7), the count (`%7d`), the mean
/// and standard deviation (`%8.5f`), the minimum (`%8.6f`) and the maximum (`%8.4f`). FS-536 when
/// a statistic has no element (returns 1). Returns 0.
pub fn mris_pretty_print_surf_quality_stats(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    fp: &mut dyn Write,
    surf: &mut Mris,
) -> Result<i32, Fatal> {
    mris_face_metric(sg, io, surf);
    let stats = quality_stats(g, sg, io, surf)?;
    let mut text = String::new();
    for (name, s) in ["Area  ", "Corner", "Edge  ", "Hinge "].iter().zip(&stats) {
        let Some(s) = s else {
            return empty_list(g, fp, &text, "MRISprettyPrintSurfQualityStats");
        };
        text.push_str(&format!(
            "{name} {:>7} {} {} {} {}\n",
            s[0] as i32,
            fmt_f(s[1], 8, 5),
            fmt_f(s[2], 8, 5),
            fmt_f(s[3], 8, 6),
            fmt_f(s[4], 8, 4)
        ));
    }
    let _ = fp.write_all(text.as_bytes());
    let _ = fp.flush();
    Ok(0)
}

// ---- curvatures and the middle of the grey matter ----------------------------------------------

/// `MRISaverageCurvatures(mris, navgs)`: `navgs` times, every unripped vertex's curvature
/// replaced by the mean (in `float`: its own plus its unripped 1-hop neighbours', counted from
/// 0, divided by their count plus one) computed from the previous step's values; then the
/// curvatures' extremes (`mrisComputeCurvatureMinMax`). Returns `NO_ERROR`.
pub fn mris_average_curvatures(mris: &mut Mris, navgs: i32) -> i32 {
    for _ in 0..navgs.max(0) {
        for vno in 0..nverts(mris) {
            if mris.vertex(vno).ripflag != 0 {
                continue;
            }
            let mut curv = mris.vertex(vno).curv;
            let mut num = 0.0f32;
            for &n in nbrs(mris, vno, mris.topology(vno).vnum) {
                let vn = mris.vertex(n as usize);
                if vn.ripflag != 0 {
                    continue;
                }
                num = add(num, 1.0);
                curv = add(curv, vn.curv);
            }
            num = add(num, 1.0);
            mris.vertex_mut(vno).tdx = div(curv, num);
        }
        for vno in 0..nverts(mris) {
            let v = mris.vertex_mut(vno);
            if v.ripflag != 0 {
                continue;
            }
            v.curv = v.tdx;
        }
    }
    compute_curvature_min_max(mris);
    NO_ERROR
}

/// `mrisFindMiddleOfGray(mris)`: the curvatures averaged three times, the current coordinates
/// saved as the temporary set, the original set restored and `MRIScomputeMetricProperties`; then
/// every unripped vertex moved to `orig + (0.5 curv) n` (`float`; `MRISsetXYZ` at line 13221).
pub fn mris_find_middle_of_gray(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<(), Fatal> {
    mris_average_curvatures(mris, 3);
    save_vertex_positions(g, io, mris, TMP_VERTICES)?;
    restore_vertex_positions(g, io, mris, ORIGINAL_VERTICES)?;
    mp::compute_metric_properties(g, sg, io, mris)?;
    let file = metric_properties_cpp();
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let t = mul(0.5, v.curv);
        // the official build's operand orders: t nx, ny t, nz t, each plus the original
        let x = add(mul(t, v.nx), v.origx);
        let y = add(mul(v.ny, t), v.origy);
        let z = add(mul(v.nz, t), v.origz);
        coords::set_xyz(g, io, mris, vno as i32, x, y, z, &file, 13221)?;
    }
    Ok(())
}
