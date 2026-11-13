// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Queue item 3.1-50, part b (relay 19): the discrete curvatures of
//! `utils/mrisurf_metricProperties.cpp` (`FACES_Hcurvature_determineSign`,
//! `VERTEX_faceAngles_determine`, `FACES_angleNormal_find`, `VERTEX_faceMinMaxAngles_determine`,
//! `MRIS_Hcurvature_determineSign`, `MRIS_discreteKH_compute`, `MRIS_discretek1k2_compute`,
//! `MRIScomputeSecondFundamentalFormDiscrete`: `mris_curvature`'s discrete curvatures) and the
//! topology fixer's angle-weighted vertex normal (`computeVertexPseudoNormal`), with fs.surf's
//! helpers taken early: `FACE_vertexIndex_find`, `FACE_vertexIndexAtMask_find`,
//! `MRIS_facesAtVertices_reorder` with `FACES_aroundVertex_reorder` and `dumpFacesAroundVertex`
//! (`utils/mrisurf_topology.cpp`), `MRISsetMarks`, `MRISclearMarks`, `MRIScountAllMarked` and
//! `MRISdilateMarked` (`utils/mrisurf_vals.cpp`).
//!
//! Arithmetic through fs.core's `sse` operations in the official build's operand order
//! (`docs/findings/freesurfer-surfio.md` §21); `acosf` glibc's with `reproduce`, correctly rounded
//! otherwise ([`acosf`]). Defects (`larmorx_core::defects`):
//! - **FS-372** (out of bounds): the faces' walk around a vertex (`FACES_aroundVertex_reorder`),
//!   when a face has no unvisited neighbour across an edge before every face is placed (a vertex on
//!   the surface's border, two fans meeting at a vertex), starts its next step from an order entry
//!   it never wrote and reads the entry before the vertex's face list. Both modes stop the walk
//!   there; the vertex then fails as FreeSurfer reports it (the faces dumped, `packed / faces
//!   mismatch`, the vertex marked) (noted).
//! - **FS-373** (out of bounds): the curvature sign of two faces (`FACES_Hcurvature_determineSign`),
//!   when every vertex of one face is on their shared edge (a face naming a vertex twice), reads the
//!   vertex before the surface's first. Both modes end the program with a message (noted).

use std::io::Write;

use larmorx_core::defects::{FS_372, FS_373};
use larmorx_core::math::{Libm, acos, acosf_glibc};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sqrt, sub, sub64};

use crate::aaa::SurfGlobals;
use crate::basemisc::{cprintd, vertex_progress_print};
use crate::coords::Io;
use crate::facenorm::get_face_norm;
use crate::mp_slow::compute_triangle_properties;
use crate::mris::{Face, Mris};
use crate::props::{faces_common_edge_length_find, vertices_common_in_faces};

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

fn topology_cpp() -> String {
    format!("{}/utils/mrisurf_topology.cpp", diag::BUILD_ROOT)
}

/// The vertex's face numbers (`vt->f`, its first `num`).
fn faces_of(mris: &Mris, vno: usize) -> Vec<i32> {
    let vt = mris.topology(vno);
    let num = usize::from(vt.num);
    vt.f.as_deref()
        .map_or_else(Vec::new, |f| f[..num.min(f.len())].to_vec())
}

/// `acosf(x)`: glibc 2.35's (fdlibm's, [`acosf_glibc`]) with `reproduce`, the correctly rounded
/// arc cosine otherwise ([`acosf_cr`]); a NaN argument gives itself quieted in both.
pub fn acosf(g: &Globals, x: f32) -> f32 {
    if x.is_nan() {
        return f32::from_bits(x.to_bits() | 0x0040_0000);
    }
    Libm::quiet(g.mode()).choose32(|| acosf_cr(x), || acosf_glibc(x))
}

/// The correctly rounded arc cosine of a `float`: the correctly rounded `double` arc cosine
/// (`larmorx_core::math::acos`, CORE-MATH's) rounded to `float`, but for the two floats of the
/// 2,130,706,434 in [-1, 1] whose `double` result is exactly halfway between two floats, where
/// the true value lies above the halfway point (measured with CORE-MATH's `cr_acos` and decided
/// at 200 bits: `validation/investigations/acosf-double-rounding/`). Outside [-1, 1] a NaN.
pub fn acosf_cr(x: f32) -> f32 {
    match x.to_bits() {
        0x3288_85a3 => f32::from_bits(0x3fc9_0fdb),
        0x3982_6222 => f32::from_bits(0x3fc9_07b5),
        _ => acos(f64::from(x)) as f32,
    }
}

// ---- fs.surf's helpers (mrisurf_topology.cpp, mrisurf_vals.cpp) -------------------------------

/// `FACE_vertexIndex_find(face, vno)`: the last of the face's three places (0 to 2) holding
/// `vno`, -1 when none does.
pub fn face_vertex_index_find(face: &Face, vno: i32) -> i16 {
    let mut ret = -1;
    for (k, &v) in face.v.iter().enumerate() {
        if v == vno {
            ret = k as i16;
        }
    }
    ret
}

/// `FACE_vertexIndexAtMask_find(face, common)`: the first of the face's vertex numbers not among
/// `common` (`VERTICES_commonInFaces_find`'s three floats, the vertex numbers compared as floats);
/// -1 when all are.
pub fn face_vertex_index_at_mask_find(face: &Face, common: &[f32; 3]) -> i32 {
    for &v in &face.v {
        if !common.contains(&(v as f32)) {
            return v;
        }
    }
    -1
}

/// `MRISsetMarks(mris, mark)`: every unripped vertex's `marked`. Returns `NO_ERROR`.
pub fn set_marks(mris: &mut Mris, mark: i32) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 {
            // `int` into the vertex's `short`
            v.marked = mark as i16;
        }
    }
    NO_ERROR
}

/// `MRISclearMarks(mris)`: [`set_marks`] 0.
pub fn clear_marks(mris: &mut Mris) -> i32 {
    set_marks(mris, 0)
}

/// `MRIScountAllMarked(mris)`: the vertices (ripped ones too) whose `marked` is above 0.
pub fn count_all_marked(mris: &Mris) -> i32 {
    (0..nverts(mris))
        .filter(|&vno| mris.vertex(vno).marked > 0)
        .count() as i32
}

/// `MRISdilateMarked(mris, ndil)`: `ndil` times, each unripped vertex's `tx` becomes the largest
/// `marked` of it and its 1-hop neighbours (as a `float`), then its `marked` that `tx` (truncated).
/// Returns `NO_ERROR`.
pub fn dilate_marked(mris: &mut Mris, ndil: i32) -> i32 {
    for _ in 0..ndil {
        for vno in 0..nverts(mris) {
            let v = mris.vertex_mut(vno);
            if v.ripflag == 0 {
                v.tx = 0.0;
            }
        }
        for vno in 0..nverts(mris) {
            if mris.vertex(vno).ripflag != 0 {
                continue;
            }
            let mut mx = i32::from(mris.vertex(vno).marked);
            let vt = mris.topology(vno);
            let vnum = usize::try_from(vt.vnum).unwrap_or(0);
            if let Some(nb) = vt.v.as_deref() {
                for &n in &nb[..vnum.min(nb.len())] {
                    let m = i32::from(mris.vertex(n as usize).marked);
                    // MAX(vn->marked, mx): `a >= b ? a : b`
                    mx = if m >= mx { m } else { mx };
                }
            }
            mris.vertex_mut(vno).tx = mx as f32;
        }
        for vno in 0..nverts(mris) {
            let v = mris.vertex_mut(vno);
            if v.ripflag == 0 {
                v.marked = (v.tx as i32) as i16;
            }
        }
    }
    NO_ERROR
}

/// `dumpFacesAroundVertex(mris, vno)`: on standard output `Dumping faces around vno:<vno>`, then a
/// line per face of the vertex, `  vi:<vno> is face:<fno> with vertices` and its three vertex
/// numbers (the line names the vertex, not the face's place, as FreeSurfer's does).
fn dump_faces_around_vertex(out: &mut dyn Write, mris: &Mris, vno: usize) {
    let _ = writeln!(out, "Dumping faces around vno:{vno}");
    for fno in faces_of(mris, vno) {
        let _ = write!(out, "  vi:{vno} is face:{fno} with vertices");
        for v in mris.face(fno as usize).v {
            let _ = write!(out, " {v}");
        }
        let _ = writeln!(out);
    }
}

/// `FACES_aroundVertex_reorder(mris, vno, order)`: the vertex's faces in the order of a walk
/// around it, as places in its face list (`order`, from the first face, place 0): each step looks,
/// from the place equal to the step's number onwards (cyclically), for the first face sharing an
/// edge with the last one placed and not yet placed. A vertex without faces ends the program
/// (`FATAL ERROR encountered in function ''FACES_aroundVertex_reorder'.` and `Mesh structural
/// error. Vertex <vno> has no faces.` on standard error, exit status 1). With
/// `BEVIN_TESTING_dumpFacesAroundVertex` set at the 2nd, 3rd, 5th, 9th, ... call of the program
/// ([`SurfGlobals`]'s counters), the faces are dumped. When the walk does not place every face, the
/// faces are dumped and `FACES_aroundVertex_reorder: packed / faces mismatch; vertex = <vno>, faces
/// = <n>, packed = <k>` is reported (`ErrorReturn`): -4; else 1.
///
/// FreeSurfer goes on after a step that places nothing, from an order entry never written, and
/// reads before the vertex's face list (FS-372); larmorx stops the walk there (noted).
fn faces_around_vertex_reorder(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &Mris,
    vno: usize,
    order: &mut [i32],
) -> Result<i32, Fatal> {
    let f = faces_of(mris, vno);
    let nfaces = f.len();
    if nfaces == 0 {
        let _ = write!(
            io.err,
            "\n\nFATAL ERROR encountered in function ''FACES_aroundVertex_reorder'.\nMesh structural error. Vertex {vno} has no faces.\n\n"
        );
        return Err(Fatal::Exit(1));
    }
    order.fill(-1);
    order[0] = 0;
    let mut packed = 1usize;
    for i in 0..nfaces {
        if packed == nfaces {
            break;
        }
        let ii = order[i];
        if ii < 0 {
            g.notices.add(
                &FS_372,
                format!(
                    "the faces around vertex {vno} do not form one fan: FreeSurfer reads before its face list; the walk stops"
                ),
            );
            break;
        }
        let fi = mris.face(f[ii as usize] as usize);
        let mut found = false;
        for j in 0..nfaces {
            let k = (i + j) % nfaces;
            let fj = mris.face(f[k] as usize);
            let (common, _) = vertices_common_in_faces(fi, fj);
            if common == 2 && !order.contains(&(k as i32)) {
                order[i + 1] = k as i32;
                packed += 1;
                found = true;
                break;
            }
        }
        let _ = found;
    }
    let count = sg.around_vertex_count;
    sg.around_vertex_count += 1;
    if count == sg.around_vertex_limit
        && io.env.get("BEVIN_TESTING_dumpFacesAroundVertex").is_some()
    {
        sg.around_vertex_limit *= 2;
        dump_faces_around_vertex(io.out, mris, vno);
    }
    if packed != nfaces {
        dump_faces_around_vertex(io.out, mris, vno);
        log::error_printf(
            g,
            io.err,
            -4,
            &format!(
                "FACES_aroundVertex_reorder: packed / faces mismatch; vertex = {vno}, faces = {nfaces}, packed = {packed}"
            ),
        );
        return Ok(-4);
    }
    Ok(1)
}

/// `MRIS_facesAtVertices_reorder(mris)`: every vertex's face list (`f`, with `n`, the vertex's
/// place in each face) put in the order of a walk around it ([`faces_around_vertex_reorder`]); a
/// vertex whose walk fails is marked instead, then the marks dilated once ([`dilate_marked`]).
/// A newline and the progress bar (`Determining geometric order for vno faces...`) on standard
/// error. A surface with a marked vertex (ripped ones too) fails `cheapAssert(MRIScountAllMarked
/// (apmris) == 0)` first (FS-03). Returns the last vertex's walk's result (1, or -4).
pub fn faces_at_vertices_reorder(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<i32, Fatal> {
    if count_all_marked(mris) != 0 {
        return Err(diag::assert_failed(
            g,
            io.out,
            io.err,
            &topology_cpp(),
            3178,
            "MRIScountAllMarked(apmris) == 0",
        ));
    }
    let _ = io.err.write_all(b"\n");
    let mut ret = 1;
    for vno in 0..nverts(mris) {
        vertex_progress_print(
            g,
            io.err,
            mris,
            vno as i32,
            Some(b"Determining geometric order for vno faces..."),
        )?;
        let f = faces_of(mris, vno);
        let mut order = vec![-1i32; f.len()];
        ret = faces_around_vertex_reorder(g, sg, io, mris, vno, &mut order)?;
        if ret < 0 {
            mris.vertex_mut(vno).marked = 1;
        } else {
            let mut nf = Vec::with_capacity(f.len());
            let mut nn = Vec::with_capacity(f.len());
            for &o in &order {
                // the order's float entries read back as int (whole numbers, exact)
                let fno = f[o as usize];
                let at = mris
                    .face(fno as usize)
                    .v
                    .iter()
                    .position(|&x| x == vno as i32)
                    .map_or(-1, |p| p as i32);
                nf.push(fno);
                nn.push(at as u8);
            }
            let vt = mris.topology_mut(vno);
            if let Some(ff) = vt.f.as_mut() {
                ff[..nf.len()].copy_from_slice(&nf);
            }
            if let Some(n) = vt.n.as_mut() {
                n[..nn.len()].copy_from_slice(&nn);
            }
        }
    }
    dilate_marked(mris, 1);
    Ok(ret)
}

// ---- the discrete curvatures (mrisurf_metricProperties.cpp 5945-6473) --------------------------

/// `FACES_Hcurvature_determineSign(mris, fo, fi)`: with `O` and `I` the vertices of faces `fo` and
/// `fi` off their shared edge (the first not on it), and `nO`, `nI` the faces' cached normals: +1
/// when `|(O + nO) - (I + nI)|` is below `|(O - nO) - (I - nI)|`, else -1 (a NaN gives -1), in
/// `float` (`sqrt(z² + (x² + y²))`). Faces that do not share exactly two vertices end the program
/// (`FACES_Hcurvature_determineSign: During call <n>, the passed faces do not share an edge`, code
/// -4; `n` the program's count of this function's completed calls). A face all of whose vertices are
/// on the shared edge (a vertex named twice) ends the program in both modes (FS-373: FreeSurfer
/// reads the vertex before the surface's first).
pub fn faces_hcurvature_determine_sign(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    fo: usize,
    fi: usize,
) -> Result<i16, Fatal> {
    let n_o = get_face_norm(mris, fo);
    let n_i = get_face_norm(mris, fi);
    let (common, mask) = vertices_common_in_faces(mris.face(fo), mris.face(fi));
    if common != 2 {
        log::error_exit(
            g,
            io.err,
            -4,
            &format!(
                "FACES_Hcurvature_determineSign: During call {}, the passed faces do not share an edge",
                sg.hcurv_sign_calls
            ),
        )?;
    }
    let vo = face_vertex_index_at_mask_find(mris.face(fo), &mask);
    let vi = face_vertex_index_at_mask_find(mris.face(fi), &mask);
    if vo < 0 || vi < 0 {
        g.notices.add(
            &FS_373,
            "the curvature sign of a face naming a vertex twice: FreeSurfer reads the vertex before the surface's first",
        );
        log::error_exit(
            g,
            io.err,
            -4,
            "FACES_Hcurvature_determineSign: a face has no vertex off the shared edge (a vertex named twice)",
        )?;
        return Ok(-1);
    }
    let (o, i) = (mris.vertex(vo as usize), mris.vertex(vi as usize));
    let po = [o.x, o.y, o.z];
    let pi = [i.x, i.y, i.z];
    let mut no = [n_o.nx, n_o.ny, n_o.nz];
    let mut ni = [n_i.nx, n_i.ny, n_i.nz];
    let mut dist = [0.0f32; 2];
    for (k, d) in dist.iter_mut().enumerate() {
        if k == 1 {
            // `V3_SCALAR_MUL(.., -1, ..)`, compiled as a sign flip
            for x in no.iter_mut().chain(ni.iter_mut()) {
                *x = -*x;
            }
        }
        let c: Vec<f32> = (0..3)
            .map(|a| sub(add(po[a], no[a]), add(pi[a], ni[a])))
            .collect();
        *d = sqrt(add(mul(c[2], c[2]), add(mul(c[0], c[0]), mul(c[1], c[1]))));
    }
    sg.hcurv_sign_calls += 1;
    Ok(if dist[0] < dist[1] { 1 } else { -1 })
}

/// `VERTEX_faceAngles_determine(mris, vno, angles)`: for each face of the vertex, the angle
/// (`acosf`, [`acosf`]) between the vertex's normal `a` and the face's cached normal `f`:
/// `((fy ay + fx ax) + fz az) / (|f| |a|)` in `float` (`|a| = sqrt(nz² + (nx² + ny²))`, `|f| =
/// sqrt((fx² + fy²) + fz²)`), above 1 taken as 1 and below -1 as -1 (a NaN kept). `errno` is
/// set to 0 before each angle (FreeSurfer's check of `acosf`'s `errno` after it, with its message
/// `VERTEX_faceAngles_determine: acos error - angle set to zero ...`, never fires: the argument is
/// limited to [-1, 1], and glibc's `acosf` leaves `errno` alone for a NaN). Returns the angles
/// (the vertex's face count of them).
pub fn vertex_face_angles_determine(g: &mut Globals, mris: &mut Mris, vno: usize) -> Vec<f32> {
    let v = mris.vertex(vno);
    let a = [v.nx, v.ny, v.nz];
    let len_a = sqrt(add(mul(a[2], a[2]), add(mul(a[0], a[0]), mul(a[1], a[1]))));
    let mut angles = Vec::new();
    for fno in faces_of(mris, vno) {
        let n = get_face_norm(mris, fno as usize);
        let f = [n.nx, n.ny, n.nz];
        let len_f = sqrt(add(add(mul(f[0], f[0]), mul(f[1], f[1])), mul(f[2], f[2])));
        let lens = mul(len_f, len_a);
        let dot = add(add(mul(f[1], a[1]), mul(f[0], a[0])), mul(f[2], a[2]));
        g.errno = 0;
        let mut arg = div(dot, lens);
        if arg > 1.0 {
            arg = 1.0;
        } else if -1.0 > arg {
            arg = -1.0;
        }
        angles.push(acosf(g, arg));
    }
    angles
}

/// `FACES_angleNormal_find(mris, fi, fj)`: the angle between the two faces' cached normals,
/// `acosf(((ix jx + iy jy) + iz jz) / (|j| |i|))` (the argument limited to [-1, 1], a NaN kept) times
/// the faces' curvature sign ([`faces_hcurvature_determine_sign`], as a `float`), with `|i| =
/// sqrt(iz² + (ix² + iy²))` and `|j| = sqrt((jx² + jy²) + jz²)`. A normal longer than 1.0001
/// ends the program (`FACES_angleNormal_find: face normal not unit length -- Ni: <|i|>, Nj:
/// <|j|>`, code -4), before the sign is looked for.
pub fn faces_angle_normal_find(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    fi: usize,
    fj: usize,
) -> Result<f32, Fatal> {
    let a = get_face_norm(mris, fi);
    let b = get_face_norm(mris, fj);
    let len_i = sqrt(add(mul(a.nz, a.nz), add(mul(a.nx, a.nx), mul(a.ny, a.ny))));
    let len_j = sqrt(add(add(mul(b.nx, b.nx), mul(b.ny, b.ny)), mul(b.nz, b.nz)));
    if f64::from(len_i) > 1.0001 || f64::from(len_j) > 1.0001 {
        log::error_exit(
            g,
            io.err,
            -4,
            &format!(
                "FACES_angleNormal_find: face normal not unit length -- Ni: {}, Nj: {}\n",
                fmt_f(f64::from(len_i), 0, 6),
                fmt_f(f64::from(len_j), 0, 6)
            ),
        )?;
    }
    let lens = mul(len_j, len_i);
    let dot = add(add(mul(a.nx, b.nx), mul(a.ny, b.ny)), mul(a.nz, b.nz));
    let sign = faces_hcurvature_determine_sign(g, sg, io, mris, fi, fj)?;
    let mut arg = div(dot, lens);
    if arg > 1.0 {
        arg = 1.0;
    } else if -1.0 > arg {
        arg = -1.0;
    }
    Ok(mul(acosf(g, arg), f32::from(sign)))
}

/// `VERTEX_faceMinMaxAngles_determine(mris, vno, &min_index, &min, &max_index, &max)`: the
/// smallest and largest of [`vertex_face_angles_determine`]'s angles with their places (the first
/// angle starts both; a place is written only when a strictly smaller, or larger, angle comes
/// later, so the caller's values stay otherwise). A vertex without faces ends the program
/// (`VERTEX_faceMinMaxAngles_determine: error with determining face angles.`, code -4). Returns the
/// face count.
pub fn vertex_face_min_max_angles_determine(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    vno: usize,
    min_index: &mut i32,
    min_angle: &mut f32,
    max_index: &mut i32,
    max_angle: &mut f32,
) -> Result<i32, Fatal> {
    let angles = vertex_face_angles_determine(g, mris, vno);
    if angles.is_empty() {
        log::error_exit(
            g,
            io.err,
            -4,
            "VERTEX_faceMinMaxAngles_determine: error with determining face angles.",
        )?;
        return Ok(0);
    }
    *min_angle = angles[0];
    *max_angle = angles[0];
    for (face, &a) in angles.iter().enumerate().skip(1) {
        if a < *min_angle {
            *min_angle = a;
            *min_index = face as i32;
        }
        if a > *max_angle {
            *max_angle = a;
            *max_index = face as i32;
        }
    }
    Ok(angles.len() as i32)
}

/// `MRIS_Hcurvature_determineSign(mris)` (obsolete in FreeSurfer, kept): each vertex's `undefval`
/// 1 when the sum of [`faces_hcurvature_determine_sign`] over its consecutive faces (the last with
/// the first) is 0 or more, else -1; the progress bar (`Determining H sign for vertex faces...`).
/// Returns 1.
pub fn mris_hcurvature_determine_sign(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<i32, Fatal> {
    for vno in 0..nverts(mris) {
        vertex_progress_print(
            g,
            io.err,
            mris,
            vno as i32,
            Some(b"Determining H sign for vertex faces..."),
        )?;
        let f = faces_of(mris, vno);
        let n = f.len();
        let mut sum = 0i32;
        for face in 0..n {
            sum += i32::from(faces_hcurvature_determine_sign(
                g,
                sg,
                io,
                mris,
                f[face] as usize,
                f[(face + 1) % n] as usize,
            )?);
        }
        mris.vertex_mut(vno).undefval = if sum >= 0 { 1 } else { -1 };
    }
    Ok(1)
}

/// `MRIS_discreteKH_compute(mris)`: each vertex's discrete Gaussian and mean curvatures; a marked
/// vertex gets 0 and 0. With, over its faces in order (each with the next, the last with the
/// first), the `float` sums of the face areas `A`, of the face's angle at the vertex `D`, and of
/// [`faces_angle_normal_find`] times the shared edge's length ([`faces_common_edge_length_find`])
/// `S`: `K = (float)((3 / A) (2π - D))` (`3 / A` in `float`, the rest in `double`) and `H =
/// (float)(S (0.75 / A))` in `double`. The surface's `Kmin`, `Kmax`, `Hmin`, `Hmax` from the
/// unmarked vertices (the first vertex's, when unmarked, starting them; else 0), compared in
/// `double`; `Ktotal` the `float` sum of `K area` (`K area + Ktotal` in `double`). A face that
/// does not hold the vertex ends the program (`MRIS_discreteKH_compute:` and `\tangleIndex lookup
/// failure for vertex <vno>, face <k>`, code -4). The progress bar (`Determining KH
/// curvatures...`). Returns `NO_ERROR`.
pub fn mris_discrete_kh_compute(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<i32, Fatal> {
    let (mut kmin, mut kmax, mut hmin, mut hmax, mut ktotal) = (0f32, 0f32, 0f32, 0f32, 0f32);
    for vno in 0..nverts(mris) {
        vertex_progress_print(
            g,
            io.err,
            mris,
            vno as i32,
            Some(b"Determining KH curvatures..."),
        )?;
        if mris.vertex(vno).marked != 0 {
            let v = mris.vertex_mut(vno);
            v.K = 0.0;
            v.H = 0.0;
            continue;
        }
        let f = faces_of(mris, vno);
        let n = f.len();
        let (mut area_sum, mut deficit, mut normal_sum) = (0f32, 0f32, 0f32);
        for face in 0..n {
            let (fi, fj) = (f[face] as usize, f[(face + 1) % n] as usize);
            let angle_normal = faces_angle_normal_find(g, sg, io, mris, fi, fj)?;
            let (a, b) = (mris.face(fi).clone(), mris.face(fj).clone());
            let edge = faces_common_edge_length_find(g, io, mris, &a, &b)?;
            area_sum = add(area_sum, a.area);
            let k = face_vertex_index_find(&a, vno as i32);
            if k == -1 {
                log::error_exit(
                    g,
                    io.err,
                    -4,
                    &format!(
                        "MRIS_discreteKH_compute:\n\tangleIndex lookup failure for vertex {vno}, face {face}"
                    ),
                )?;
            }
            deficit = add(deficit, a.angle[k.max(0) as usize]);
            normal_sum = add(mul(angle_normal, edge), normal_sum);
        }
        let k = mul64(
            f64::from(div(3.0, area_sum)),
            sub64(2.0 * std::f64::consts::PI, f64::from(deficit)),
        );
        let h = mul64(f64::from(normal_sum), div64(0.75, f64::from(area_sum)));
        let (kf, hf) = (k as f32, h as f32);
        {
            let v = mris.vertex_mut(vno);
            v.K = kf;
            v.H = hf;
        }
        if vno == 0 {
            (kmin, kmax) = (kf, kf);
            (hmin, hmax) = (hf, hf);
        }
        if k > f64::from(kmax) {
            kmax = kf;
        }
        if f64::from(kmin) > k {
            kmin = kf;
        }
        if h > f64::from(hmax) {
            hmax = hf;
        }
        if f64::from(hmin) > h {
            hmin = hf;
        }
        let area = f64::from(mris.vertex(vno).area);
        ktotal = add64(mul64(k, area), f64::from(ktotal)) as f32;
    }
    mris.Kmax = kmax;
    mris.Kmin = kmin;
    mris.Hmax = hmax;
    mris.Hmin = hmin;
    mris.Ktotal = f64::from(ktotal);
    Ok(NO_ERROR)
}

/// `MRIS_discretek1k2_compute(mris, signed)`: each vertex's principal curvatures from its `K`
/// and `H`, in `float`: `d = H H - K` (0 when negative, counted), `A = H + sqrt(d)`, `B = H -
/// sqrt(d)`; `k1` the larger and `k2` the smaller (`k1 = A >= B ? A : B`, `k2 = A <= B ? A : B`)
/// with `signed`, else of the larger and smaller magnitude (`fabs`); a NaN takes `B` for both. The
/// progress bar (`Determining k1k2 curvatures...`); with negative `d`s, `cprintd("deltaViolations",
/// n)`. Returns `NO_ERROR`.
pub fn mris_discrete_k1k2_compute(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    signed: bool,
) -> Result<i32, Fatal> {
    let mut violations = 0;
    for vno in 0..nverts(mris) {
        vertex_progress_print(
            g,
            io.err,
            mris,
            vno as i32,
            Some(b"Determining k1k2 curvatures..."),
        )?;
        let v = mris.vertex_mut(vno);
        let mut d = sub(mul(v.H, v.H), v.K);
        if 0.0 > d {
            violations += 1;
            d = 0.0;
        }
        let s = sqrt(d);
        let a = add(v.H, s);
        let b = sub(v.H, s);
        let (k1, k2) = if signed {
            (if a >= b { a } else { b }, if a <= b { a } else { b })
        } else {
            (
                if a.abs() >= b.abs() { a } else { b },
                if a.abs() <= b.abs() { a } else { b },
            )
        };
        v.k1 = k1;
        v.k2 = k2;
    }
    if violations != 0 {
        cprintd(io.err, b"deltaViolations", violations);
    }
    Ok(NO_ERROR)
}

/// `MRIScomputeSecondFundamentalFormDiscrete(mris, signed)` (`mris_curvature`'s discrete
/// curvatures): the marks cleared ([`clear_marks`]), the triangle properties
/// ([`compute_triangle_properties`]), the faces put in order around each vertex
/// ([`faces_at_vertices_reorder`], which leaves the vertices it could not order marked, and their
/// neighbours), then [`mris_discrete_kh_compute`] and [`mris_discrete_k1k2_compute`]. Returns 0.
pub fn compute_second_fundamental_form_discrete(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    signed: bool,
) -> Result<i32, Fatal> {
    clear_marks(mris);
    compute_triangle_properties(g, sg, io, mris);
    faces_at_vertices_reorder(g, sg, io, mris)?;
    let kh = mris_discrete_kh_compute(g, sg, io, mris)?;
    let k12 = mris_discrete_k1k2_compute(g, io, mris, signed)?;
    Ok(kh | k12)
}

// ---- the topology fixer's vertex normal (mrisurf_metricProperties.cpp 7505) --------------------

/// `computeVertexPseudoNormal(mris, vno, norm, verbose)`: the sum over the vertex's faces of the
/// face's cached normal times the face's angle at the vertex in the original positions: with `n0`
/// the vertex's place in the face (`vt->n`), `n1` the next and `n2` the previous, `v1` and `v2` the
/// original positions of those two corners less the vertex's, `r = ((v1x v2x + v1y v2y) + v1z v2z)
/// / |v1| / |v2|` in `float` (`|v| = sqrt(vz² + (vx² + vy²))`), `α = acosf(r)` with `r` above 1
/// taken as 1 and 0 or below as 0 (a NaN kept), and `norm += n α` (`n α + norm`). Not normalized.
/// A face whose places do not match (the vertex not at `n0`, or at `n1` or `n2`, or `n1` and `n2`
/// the same vertex) is reported with `verbose` 3 or more on standard error (`error for vno in face
/// <f>`, `error for vn1 ...`, `error for vn2 ...`, `error for vn ...` as they apply, then `face <f>
/// (<a>,<b>,<c>) != (<vno>)`; with `verbose` 3 `computeVertexPseudoNormal: SHOULD NOT HAPPEN`; with 4
/// the program ends with it, `ERROR_BADPARM`), and summed all the same.
pub fn compute_vertex_pseudo_normal(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    vno: usize,
    verbose: i32,
) -> Result<[f32; 3], Fatal> {
    let mut norm = [0f32; 3];
    let f = faces_of(mris, vno);
    let places: Vec<u8> = mris
        .topology(vno)
        .n
        .as_deref()
        .map_or_else(Vec::new, |n| n[..f.len().min(n.len())].to_vec());
    let vnoi = vno as i32;
    for (k, &fno) in f.iter().enumerate() {
        let face = mris.face(fno as usize).v;
        let n0 = usize::from(places.get(k).copied().unwrap_or(0));
        let n1 = if n0 == 2 { 0 } else { n0 + 1 };
        let n2 = if n0 == 0 { 2 } else { n0 - 1 };
        let at = |i: usize| face.get(i).copied().unwrap_or(-1);
        let (a0, a1, a2) = (at(n0), at(n1), at(n2));
        if (a0 != vnoi || a1 == vnoi || a2 == vnoi || a2 == a1) && verbose >= 3 {
            if a0 != vnoi {
                let _ = write!(io.err, "error for vno in face {fno}");
            }
            if a1 == vnoi {
                let _ = write!(io.err, "error for vn1 in face {fno}");
            }
            if a2 == vnoi {
                let _ = write!(io.err, "error for vn2 in face {fno}");
            }
            if a2 == a1 {
                let _ = write!(io.err, "error for vn in face {fno}");
            }
            let _ = writeln!(io.err, "face {fno} ({a0},{a1},{a2}) != ({vno})");
            if verbose == 3 {
                let _ = io
                    .err
                    .write_all(b"computeVertexPseudoNormal: SHOULD NOT HAPPEN\n");
            }
            if verbose == 4 {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_BADPARM,
                    "computeVertexPseudoNormal: SHOULD NOT HAPPEN\n",
                )?;
            }
        }
        let v = mris.vertex(vno);
        let (b1, b2) = (mris.vertex(a1 as usize), mris.vertex(a2 as usize));
        let v1 = [
            sub(b1.origx, v.origx),
            sub(b1.origy, v.origy),
            sub(b1.origz, v.origz),
        ];
        let v2 = [
            sub(b2.origx, v.origx),
            sub(b2.origy, v.origy),
            sub(b2.origz, v.origz),
        ];
        let dot = add(add(mul(v1[0], v2[0]), mul(v1[1], v2[1])), mul(v1[2], v2[2]));
        let len = |w: [f32; 3]| sqrt(add(mul(w[2], w[2]), add(mul(w[0], w[0]), mul(w[1], w[1]))));
        let r = div(div(dot, len(v1)), len(v2));
        let alpha = if r > 1.0 {
            1.0
        } else if 0.0 >= r {
            0.0
        } else {
            r
        };
        let alpha = acosf(g, alpha);
        let n = get_face_norm(mris, fno as usize);
        norm[0] = add(mul(n.nx, alpha), norm[0]);
        norm[1] = add(mul(n.ny, alpha), norm[1]);
        norm[2] = add(mul(n.nz, alpha), norm[2]);
    }
    Ok(norm)
}
