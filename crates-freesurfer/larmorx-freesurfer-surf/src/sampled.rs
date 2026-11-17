// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The surface deformation's sampled energies (queue item 3.1-52, relay 45 of track D):
//! FreeSurfer 8.2.0's `utils/mrisurf_metricProperties_slow.h` (compiled into
//! `mrisurf_metricProperties.cpp`): `MRISsampleFaceCoordsCanonical` (970), a set of a surface's
//! coordinates sampled at a point through its canonical (spherical) positions, and the sampled
//! energies the deformation's gradients and sums take at a vertex moved to a point of the sphere:
//! `mrisSampleMinimizationEnergy` (734), `mrisSampleNormalEnergy` (753), `mrisSampleSpringEnergy`
//! (800), `mrisSampleParallelEnergyAtVertex` (846) and `mrisSampleParallelEnergy` (921). Their
//! integration parameters are read for the face table only (`parms->mht`), passed here as the
//! table itself until fs.deform's `INTEGRATION_PARMS` exists. Ashburner's sampled energy
//! (`mrisSampleAshburnerTriangleEnergy`) is `props11.rs`'s.
//!
//! Every operation that can make or meet a NaN goes through fs.core's [`sse`] helpers in the
//! official build's operand order, read from the unit oracle's `mrisurf_metricProperties.cpp.o`:
//! the minimization and normal energies' projection onto the sphere sums `z² + (x² + y²)`, the
//! others `(x² + y²) + z²` (`docs/findings/freesurfer-surfio.md` §44).
//!
//! Defects (`larmorx_core::defects`):
//! - **FS-1011** (a crash): a point with no face within 1,000 mm of the face table (a point that is
//!   not finite, a table without faces). By default the program ends with an error
//!   (`ERROR_BADPARM`); `reproduce` as FreeSurfer's does (status 139).
//! - **FS-1012** (memory outside the faces): a face number outside the surface's faces sampled (the
//!   spring and parallel energies when no face is found, or a vertex's assigned face -1 or beyond
//!   the faces). An error in both modes (`ERROR_BADPARM`).
//! - fs.surf's FS-366 (`face_barycentric_coords` of a degenerate face) and the hash table's
//!   FS-576, FS-909, FS-998 and FS-999 through the searches.
//!
//! [`sse`]: larmorx_freesurfer_core::sse

use larmorx_core::defects::{FS_1011, FS_1012};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sqrt, sub, sub64};

use crate::base::project_point_onto_sphere;
use crate::coords::Io;
use crate::mht::Mht;
use crate::mris::Mris;
use crate::props2::{face_barycentric_coords, sample_face_coords};
use crate::tritri::{narrow, widen};
use crate::views::{
    CANONICAL_VERTICES, PIAL_NORMALS, PIAL_VERTICES, WHITE_VERTICES, vertex_coord2xyz_float,
};

/// `FZERO` of a `float` (`fabs(f) < FLT_EPSILON` in `float`).
fn fzero(x: f32) -> bool {
    x.abs() < f32::EPSILON
}

/// `project_point_onto_sphere` as the minimization and normal energies' inlined copies order it:
/// `dist = sqrt(z² + (x² + y²))` in `float`, `d = 0` when `FZERO(dist)` else `1 - radius / dist`,
/// the point `c - d c`.
fn project_onto_sphere_z_first(cx: f32, cy: f32, cz: f32, radius: f32) -> (f32, f32, f32) {
    let x2 = mul(cx, cx);
    let y2 = mul(cy, cy);
    let z2 = mul(cz, cz);
    let dist = sqrt(add(z2, add(x2, y2)));
    let d = if fzero(dist) {
        0.0
    } else {
        sub(1.0, div(radius, dist))
    };
    (
        sub(cx, mul(d, cx)),
        sub(cy, mul(d, cy)),
        sub(cz, mul(d, cz)),
    )
}

/// A face number a sampled energy samples: outside the surface's faces it is FS-1012 (FreeSurfer
/// reads memory outside them), an error in both modes.
fn face_in_surface(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    fno: i32,
    who: &str,
) -> Result<usize, Fatal> {
    if (0..mris.nfaces).contains(&fno) {
        return Ok(fno as usize);
    }
    let message = format!(
        "{who}: face {fno} is outside the surface's {} faces (FreeSurfer reads memory outside them)",
        mris.nfaces
    );
    g.notices.add(&FS_1012, message.clone());
    log::error_exit(g, io.err, ERROR_BADPARM, &message)?;
    unreachable!("ErrorExit with a non-zero code always ends the call")
}

/// The closest face to `p` of the table that the point projects into within `mm` millimetres and
/// `shells` shells, else (-1) the closest within 1,000 mm with no limits: `MHTfindClosestFaceGeneric`
/// twice, as the sampled energies call it. Whether the first search found it, and the face (-1 when
/// neither did).
fn closest_face(
    g: &mut Globals,
    io: &mut Io,
    mht: &Mht,
    mris: &Mris,
    p: [f64; 3],
    mm: f64,
    shells: i32,
) -> Result<(bool, i32), Fatal> {
    let (fno, _) = mht.find_closest_face_generic(g, io.err, mris, p, mm, shells, 1)?;
    if fno >= 0 {
        return Ok((true, fno));
    }
    let (fno, _) = mht.find_closest_face_generic(g, io.err, mris, p, 1000.0, -1, -1)?;
    Ok((false, fno))
}

/// `MRISsampleFaceCoordsCanonical(mht, mris, x, y, z, which, &px, &py, &pz)`: the point projected
/// onto the sphere of the surface's radius when its `float` length is not within `FLT_EPSILON` of
/// it (`fabs(norm - radius)` in `double`; the radius rounded to `float` for the projection), then
/// the closest face of the table it projects into within 8 mm and 8 shells and its barycentric
/// coordinates at the canonical positions (fs.surf's `face_barycentric_coords`, whatever its
/// answer); else the closest face within 1,000 mm with a third for each coordinate (answer
/// `NO_ERROR`). The coordinates `which` of the face's corners so weighted, each sum `(c l) + p` in
/// `double` rounded to `float` per corner, from 0. FS-1011: no face within 1,000 mm. Returns the
/// barycentric coordinates' answer and the point.
#[allow(clippy::too_many_arguments)]
pub fn sample_face_coords_canonical(
    g: &mut Globals,
    io: &mut Io,
    mht: &Mht,
    mris: &Mris,
    x: f32,
    y: f32,
    z: f32,
    which: i32,
) -> Result<(i32, [f32; 3]), Fatal> {
    let norm = sqrt(add(add(mul(x, x), mul(y, y)), mul(z, z)));
    let (x, y, z) = if sub64(widen(norm), mris.radius).abs() < f64::from(f32::EPSILON) {
        (x, y, z)
    } else {
        project_point_onto_sphere(x, y, z, narrow(mris.radius))
    };
    let p = [widen(x), widen(y), widen(z)];
    let (first, fno) = closest_face(g, io, mht, mris, p, 8.0, 8)?;
    if fno < 0 {
        let message = format!(
            "MRISsampleFaceCoordsCanonical: no face within 1000 mm of ({x}, {y}, {z}) (FreeSurfer reads through a null pointer)"
        );
        g.notices.add(&FS_1011, message.clone());
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        log::error_exit(g, io.err, ERROR_BADPARM, &message)?;
        unreachable!("ErrorExit with a non-zero code always ends the call");
    }
    let fno = fno as usize;
    let (ret, lambda) = if first {
        face_barycentric_coords(g, io, mris, fno, CANONICAL_VERTICES, p)?
    } else {
        (NO_ERROR, [1.0 / 3.0; 3])
    };
    let mut out = [0f32; 3];
    for (n, l) in lambda.iter().enumerate() {
        let v = mris.vertex(mris.face(fno).v[n] as usize);
        let (xv, yv, zv) = vertex_coord2xyz_float(g, io.err, v, which)?;
        for (o, c) in out.iter_mut().zip([xv, yv, zv]) {
            *o = narrow(add64(mul64(widen(c), *l), widen(*o)));
        }
    }
    Ok((ret, out))
}

/// Vertex `vno`'s 1-hop neighbours (the first `vnum` of its list; none when `vnum` is not
/// positive).
fn neighbours(mris: &Mris, vno: usize) -> Vec<i32> {
    let vt = mris.topology(vno);
    let list = vt.v.as_deref().unwrap_or(&[]);
    list[..(vt.vnum.max(0) as usize).min(list.len())].to_vec()
}

/// The vertex's white coordinates (`MRISvertexCoord2XYZ_float(v, WHITE_VERTICES, ...)`).
fn white(g: &mut Globals, io: &mut Io, mris: &Mris, vno: usize) -> Result<[f32; 3], Fatal> {
    let (x, y, z) = vertex_coord2xyz_float(g, io.err, mris.vertex(vno), WHITE_VERTICES)?;
    Ok([x, y, z])
}

/// `MRISsampleFaceCoords(mris, fno, x, y, z, PIAL_VERTICES, CANONICAL_VERTICES, ...)` at a
/// position of the surface (widened), FS-1012 for a face outside the surface.
fn pial_at(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    fno: i32,
    p: [f32; 3],
    who: &str,
) -> Result<[f32; 3], Fatal> {
    let fno = face_in_surface(g, io, mris, fno, who)?;
    let p = [widen(p[0]), widen(p[1]), widen(p[2])];
    Ok(sample_face_coords(g, io, mris, fno, p, PIAL_VERTICES, CANONICAL_VERTICES)?.1)
}

/// `mrisSampleMinimizationEnergy(mris, vno, parms, cx, cy, cz)` with `parms->mht` as `mht`: the
/// point projected onto the sphere of the surface's radius (`z² + (x² + y²)`; the radius rounded
/// to `float`), the pial position sampled there ([`sample_face_coords_canonical`]); the squared
/// distance `(dx² + dy²) + dz²` in `float` from vertex `vno`'s white position (`pial - white`).
#[allow(clippy::too_many_arguments)]
pub fn sample_minimization_energy(
    g: &mut Globals,
    io: &mut Io,
    mht: &Mht,
    mris: &Mris,
    vno: usize,
    cx: f32,
    cy: f32,
    cz: f32,
) -> Result<f32, Fatal> {
    let (cx, cy, cz) = project_onto_sphere_z_first(cx, cy, cz, narrow(mris.radius));
    let w = white(g, io, mris, vno)?;
    let (_, p) = sample_face_coords_canonical(g, io, mht, mris, cx, cy, cz, PIAL_VERTICES)?;
    let (dx, dy, dz) = (sub(p[0], w[0]), sub(p[1], w[1]), sub(p[2], w[2]));
    Ok(add(add(mul(dx, dx), mul(dy, dy)), mul(dz, dz)))
}

/// `mrisSampleNormalEnergy(mris, vno, parms, cx, cy, cz)` with `parms->mht` as `mht`: the point
/// projected as [`sample_minimization_energy`] does, the pial position sampled there; `d = pial -
/// white` (vertex `vno`'s), `len = sqrt((dx² + dy²) + dz²)` in `float`; 0 when `len < 0.01` (in
/// `double`). Else `d / len`, `e1 = 1 - ((wnx dx + wny dy) + wnz dz)` against the vertex's white
/// normal; the pial normals sampled there, `n = sqrt(pnz² + (pnx² + pny²))`, 0 when below 0.01;
/// else `pn / n`, `e2 = 1 - (pnz dz + (dy pny + dx pnx))`. The energy `sqrt(e1²) + sqrt(e2²)`, each
/// root in `float`, the sum in `double`, as `float`.
#[allow(clippy::too_many_arguments)]
pub fn sample_normal_energy(
    g: &mut Globals,
    io: &mut Io,
    mht: &Mht,
    mris: &Mris,
    vno: usize,
    cx: f32,
    cy: f32,
    cz: f32,
) -> Result<f32, Fatal> {
    let (cx, cy, cz) = project_onto_sphere_z_first(cx, cy, cz, narrow(mris.radius));
    let w = white(g, io, mris, vno)?;
    let (_, p) = sample_face_coords_canonical(g, io, mht, mris, cx, cy, cz, PIAL_VERTICES)?;
    let (mut dx, mut dy, mut dz) = (sub(p[0], w[0]), sub(p[1], w[1]), sub(p[2], w[2]));
    let len = sqrt(add(add(mul(dx, dx), mul(dy, dy)), mul(dz, dz)));
    if widen(len) < 0.01 {
        return Ok(0.0);
    }
    dz = div(dz, len);
    dx = div(dx, len);
    dy = div(dy, len);
    let v = mris.vertex(vno);
    let dot = add(add(mul(v.wnx, dx), mul(v.wny, dy)), mul(v.wnz, dz));
    let e1 = sub(1.0, dot);
    let e1 = mul(e1, e1);
    let (_, pn) = sample_face_coords_canonical(g, io, mht, mris, cx, cy, cz, PIAL_NORMALS)?;
    let n = sqrt(add(
        mul(pn[2], pn[2]),
        add(mul(pn[0], pn[0]), mul(pn[1], pn[1])),
    ));
    if widen(n) < 0.01 {
        return Ok(0.0);
    }
    let (pnx, pny, pnz) = (div(pn[0], n), div(pn[1], n), div(pn[2], n));
    let dot = add(mul(pnz, dz), add(mul(dy, pny), mul(dx, pnx)));
    let e2 = sub(1.0, dot);
    let r2 = sqrt(mul(e2, e2));
    let r1 = sqrt(e1);
    Ok(narrow(add64(widen(r1), widen(r2))))
}

/// `mrisSampleSpringEnergy(mris, vno, cx, cy, cz, parms)` with `parms->mht` as `mht`: the point
/// projected onto the sphere of the surface's radius (`(x² + y²) + z²`); its face: the closest it
/// projects into within 4 mm and 4 shells, else the closest within 1,000 mm; the pial position
/// sampled on that face at the canonical positions (`MRISsampleFaceCoords`); the mean, in `float`
/// and in neighbour order, of the pial positions sampled on each unripped 1-hop neighbour's
/// assigned face (`fno`) at its current position (0 with none); the energy `((dx² + dy²) + dz²) /
/// avg_vertex_dist` (`mean - pial`, the sum in `float`, the quotient in `double`; a mean distance of
/// 0 taken as 1), as `float`. FS-1012 for a face outside the surface.
#[allow(clippy::too_many_arguments)]
pub fn sample_spring_energy(
    g: &mut Globals,
    io: &mut Io,
    mht: &Mht,
    mris: &Mris,
    vno: usize,
    cx: f32,
    cy: f32,
    cz: f32,
) -> Result<f32, Fatal> {
    const WHO: &str = "mrisSampleSpringEnergy";
    let mut vdist = mris.avg_vertex_dist;
    if vdist == 0.0 {
        vdist = 1.0;
    }
    let (cx, cy, cz) = project_point_onto_sphere(cx, cy, cz, narrow(mris.radius));
    let c = [widen(cx), widen(cy), widen(cz)];
    let (_, fno) = closest_face(g, io, mht, mris, c, 4.0, 4)?;
    let p = pial_at(g, io, mris, fno, [cx, cy, cz], WHO)?;
    let (mut xc, mut yc, mut zc) = (0f32, 0f32, 0f32);
    let mut num = 0i32;
    let nbrs = neighbours(mris, vno);
    for vn in nbrs {
        let n = mris.vertex(vn as usize);
        if n.ripflag != 0 {
            continue;
        }
        let (nfno, pos) = (n.fno, [n.x, n.y, n.z]);
        let q = pial_at(g, io, mris, nfno, pos, WHO)?;
        xc = add(xc, q[0]);
        yc = add(yc, q[1]);
        zc = add(zc, q[2]);
        num += 1;
    }
    if num > 0 {
        let k = num as f32;
        xc = div(xc, k);
        yc = div(yc, k);
        zc = div(zc, k);
    }
    let (dx, dy, dz) = (sub(xc, p[0]), sub(yc, p[1]), sub(zc, p[2]));
    let s = add(add(mul(dx, dx), mul(dy, dy)), mul(dz, dz));
    Ok(narrow(div64(widen(s), vdist)))
}

/// `mrisSampleParallelEnergyAtVertex(mris, vno, parms)` with `parms->mht` as `mht`: the vertex's
/// direction from its white position to the pial position sampled on its assigned face at its
/// current position (`MRISsampleFaceCoords`, the canonical positions), `len = sqrt((dx² + dy²) +
/// dz²)` in `float`, 0 when `FZERO(len)`; then over its unripped 1-hop neighbours in order, each
/// one's direction from its white position to the pial position sampled on its assigned face (or,
/// with none assigned, through the table: [`sample_face_coords_canonical`]) at its current
/// position, skipped when its length is `FZERO`, else `sse += sqrt(e²)` (`e = 1 - ((dxn dx + dyn
/// dy) + dzn dz)` of the unit directions, the root in `float`, the sum `root + sse` in `double`);
/// the mean over those counted (`sse` itself with none), as `float`. FS-1012 for an assigned face
/// outside the surface.
pub fn sample_parallel_energy_at_vertex(
    g: &mut Globals,
    io: &mut Io,
    mht: &Mht,
    mris: &Mris,
    vno: usize,
) -> Result<f32, Fatal> {
    const WHO: &str = "mrisSampleParallelEnergyAtVertex";
    let w = white(g, io, mris, vno)?;
    let (vfno, vpos) = {
        let v = mris.vertex(vno);
        (v.fno, [v.x, v.y, v.z])
    };
    let p = pial_at(g, io, mris, vfno, vpos, WHO)?;
    let (mut dx, mut dy, mut dz) = (sub(p[0], w[0]), sub(p[1], w[1]), sub(p[2], w[2]));
    let len = sqrt(add(add(mul(dx, dx), mul(dy, dy)), mul(dz, dz)));
    if fzero(len) {
        return Ok(0.0);
    }
    dx = div(dx, len);
    dy = div(dy, len);
    dz = div(dz, len);
    let nbrs = neighbours(mris, vno);
    let mut sse = 0f64;
    let mut num = 0i32;
    for vn in nbrs {
        let vn = vn as usize;
        let (rip, nfno, pos) = {
            let n = mris.vertex(vn);
            (n.ripflag, n.fno, [n.x, n.y, n.z])
        };
        if rip != 0 {
            continue;
        }
        let wn = white(g, io, mris, vn)?;
        let q = if nfno >= 0 {
            pial_at(g, io, mris, nfno, pos, WHO)?
        } else {
            sample_face_coords_canonical(g, io, mht, mris, pos[0], pos[1], pos[2], PIAL_VERTICES)?.1
        };
        let (mut dxn, mut dyn_, mut dzn) = (sub(q[0], wn[0]), sub(q[1], wn[1]), sub(q[2], wn[2]));
        let len = sqrt(add(add(mul(dxn, dxn), mul(dyn_, dyn_)), mul(dzn, dzn)));
        if fzero(len) {
            continue;
        }
        dxn = div(dxn, len);
        dyn_ = div(dyn_, len);
        dzn = div(dzn, len);
        let dot = add(add(mul(dxn, dx), mul(dyn_, dy)), mul(dzn, dz));
        let e = sub(1.0, dot);
        sse = add64(widen(sqrt(mul(e, e))), sse);
        num += 1;
    }
    if num > 0 {
        sse = div64(sse, f64::from(num));
    }
    Ok(narrow(sse))
}

/// `mrisSampleParallelEnergy(mris, vno, parms, cx, cy, cz)` with `parms->mht` as `mht`: the point
/// projected onto the sphere of the surface's radius (`(x² + y²) + z²`); its face (the closest it
/// projects into within 4 mm and 4 shells, else the closest within 1,000 mm) assigned to vertex
/// `vno` and the vertex moved there; the parallel energy at the vertex
/// ([`sample_parallel_energy_at_vertex`]) plus, in `double` and in order, that of each unripped
/// 1-hop neighbour, over their count; the vertex's position and face restored. As `float` (the
/// vertex's own energy as it is when it has no neighbours in its list).
#[allow(clippy::too_many_arguments)]
pub fn sample_parallel_energy(
    g: &mut Globals,
    io: &mut Io,
    mht: &Mht,
    mris: &mut Mris,
    vno: usize,
    cx: f32,
    cy: f32,
    cz: f32,
) -> Result<f32, Fatal> {
    let (cx, cy, cz) = project_point_onto_sphere(cx, cy, cz, narrow(mris.radius));
    let saved = {
        let v = mris.vertex(vno);
        (v.x, v.y, v.z, v.fno)
    };
    let c = [widen(cx), widen(cy), widen(cz)];
    let (_, fno) = closest_face(g, io, mht, mris, c, 4.0, 4)?;
    {
        let v = mris.vertex_mut(vno);
        v.fno = fno;
        (v.x, v.y, v.z) = (cx, cy, cz);
    }
    let own = sample_parallel_energy_at_vertex(g, io, mht, mris, vno)?;
    let nbrs = neighbours(mris, vno);
    let result = if nbrs.is_empty() {
        own
    } else {
        let mut sse = widen(own);
        let mut num = 1i32;
        for vn in nbrs {
            if mris.vertex(vn as usize).ripflag != 0 {
                continue;
            }
            sse = add64(
                sse,
                widen(sample_parallel_energy_at_vertex(
                    g,
                    io,
                    mht,
                    mris,
                    vn as usize,
                )?),
            );
            num += 1;
        }
        narrow(div64(sse, f64::from(num)))
    };
    let v = mris.vertex_mut(vno);
    (v.x, v.y, v.z, v.fno) = saved;
    Ok(result)
}
