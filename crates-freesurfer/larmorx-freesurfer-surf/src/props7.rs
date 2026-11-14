// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Queue item 3.1-51, the rest (relay 20), third group: the gradients' averaging of
//! `utils/mrisurf_metricProperties.cpp` (lines 9395-10280): `MRISaverageGradients` (every surface
//! deformation's smoothing of the step; the official build always takes its own algorithm, the
//! fast one only by name), `MRISaverageGradientsFast` and `mrisAverageSignedGradients`.
//!
//! Arithmetic through fs.core's `sse` operations in the official build's operand order (the unit
//! oracle's object code, its OpenMP-outlined loops; `docs/findings/freesurfer-surfio.md` §22).
//! The loops' threads never meet: each vertex's new value reads only the previous pass, so the
//! result does not depend on the thread count. Defect (`larmorx_core::defects`):
//! - **FS-379** (memory): `MRISaverageGradients` of a surface whose neighbour lists are not
//!   symmetric (an unripped neighbour whose own neighbours are all ripped) counts that neighbour
//!   but leaves it out of its list, so the vertex is averaged with other vertices' neighbours and
//!   the last one reads past the list. Both modes average over the vertex's own unripped
//!   neighbours, the left-out one with its unchanged gradient (as the function's other forms do;
//!   noted).
//!
//! `MRISaverageGradientsFastCheck` (a self-test reading `fsaverage`'s `lh.white` from
//! `FREESURFER_HOME` and filling the gradients with `drand48`; no program calls it) is not ported.

use larmorx_core::defects::FS_379;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::verbose_on;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::NO_ERROR;
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul};

use crate::aaa::SurfGlobals;
use crate::coords::Io;
use crate::mris::Mris;

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// The first `vnum` neighbours of vertex `vno`.
fn neighbours(mris: &Mris, vno: usize) -> Vec<usize> {
    let vt = mris.topology(vno);
    let n = usize::try_from(vt.vnum).unwrap_or(0);
    vt.v.as_deref().unwrap_or(&[])[..n]
        .iter()
        .map(|&x| x as usize)
        .collect()
}

/// The vertex `Gdiag_no` when it is 0 or more (FreeSurfer's diagnostic vertex), if the surface
/// has it.
fn diag_vertex(g: &Globals, mris: &Mris) -> Option<usize> {
    usize::try_from(g.gdiag_no)
        .ok()
        .filter(|&v| v < nverts(mris))
}

/// `MRISaverageGradients(mris, num_avgs)`: `num_avgs` passes in which each unripped vertex with
/// an unripped neighbour (the first `vnum`) takes `(d + d1 + ... + dk) (1 / (k + 1))` of its own
/// and its unripped neighbours' gradients `dx`, `dy`, `dz` from the previous pass (the sum in
/// `float` in the neighbours' order, multiplied by the `float` reciprocal); the others keep
/// theirs; `tdx` ... are not touched. With `Gdiag_no` above 0 and `DIAG_VERBOSE`,
/// `MRISaverageGradients()` first; nothing more for `num_avgs` 0 or less. With `Gdiag_no` 0 or more,
/// `before averaging %d times dot = %2.2f ` and ` after dot = %2.2f D = (%2.2f, %2.2f, %2.2f), N =
/// (%2.1f, %2.1f, %2.1f)` of that vertex's gradient and normal (standard output). More than 150
/// passes read `FREESURFER_MRISaverageGradients_optimize_suppress` once in the program (when set:
/// `FREESURFER_MRISaverageGradients_optimize_suppress so not optimizing`; the passes' order of
/// work changes, not their numbers). FS-379.
pub fn average_gradients(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    num_avgs: i32,
) -> i32 {
    if g.gdiag_no > 0 && verbose_on(g) {
        let _ = writeln!(io.out, "MRISaverageGradients()");
    }
    if num_avgs <= 0 {
        return NO_ERROR;
    }
    let diag = diag_vertex(g, mris);
    if let Some(d) = diag {
        let v = mris.vertex(d);
        let dot = add(add(mul(v.dx, v.nx), mul(v.dy, v.ny)), mul(v.dz, v.nz));
        let _ = write!(
            io.out,
            "before averaging {num_avgs} times dot = {} ",
            fmt_f(f64::from(dot), 2, 2)
        );
    }
    let n = nverts(mris);
    // which vertices change: unripped, with an unripped neighbour (counted with repeats)
    let mut lists: Vec<Option<Vec<usize>>> = vec![None; n];
    for (vno, list) in lists.iter_mut().enumerate() {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let nb: Vec<usize> = neighbours(mris, vno)
            .into_iter()
            .filter(|&x| mris.vertex(x).ripflag == 0)
            .collect();
        if !nb.is_empty() {
            *list = Some(nb);
        }
    }
    let lonely = (0..n)
        .filter(|&vno| {
            lists[vno]
                .as_ref()
                .is_some_and(|nb| nb.iter().any(|&x| lists[x].is_none()))
        })
        .count();
    if lonely > 0 {
        g.notices.add(
            &FS_379,
            format!("MRISaverageGradients: {lonely} vertices have an unripped neighbour whose own neighbours are all ripped; averaged with it (FreeSurfer leaves it out of their lists and reads other vertices' neighbours)"),
        );
    }
    if num_avgs > 150 && sg.avg_gradients_optimize.is_none() {
        let suppress = io
            .env
            .get("FREESURFER_MRISaverageGradients_optimize_suppress")
            .is_some();
        if suppress {
            let _ = writeln!(
                io.out,
                "FREESURFER_MRISaverageGradients_optimize_suppress so not optimizing"
            );
        }
        sg.avg_gradients_optimize = Some(!suppress);
    }
    let mut cur: Vec<[f32; 3]> = (0..n)
        .map(|vno| {
            let v = mris.vertex(vno);
            [v.dx, v.dy, v.dz]
        })
        .collect();
    let mut next = cur.clone();
    for _ in 0..num_avgs {
        for (vno, list) in lists.iter().enumerate() {
            let Some(nb) = list else { continue };
            let mut d = cur[vno];
            for &x in nb {
                for k in 0..3 {
                    d[k] = add(d[k], cur[x][k]);
                }
            }
            let inv = div(1.0, (nb.len() + 1) as f32);
            next[vno] = [mul(d[0], inv), mul(d[1], inv), mul(d[2], inv)];
        }
        std::mem::swap(&mut cur, &mut next);
    }
    for (vno, list) in lists.iter().enumerate() {
        if list.is_some() {
            let v = mris.vertex_mut(vno);
            [v.dx, v.dy, v.dz] = cur[vno];
        }
    }
    if let Some(d) = diag {
        let v = mris.vertex(d);
        let dot = add(add(mul(v.nx, v.dx), mul(v.ny, v.dy)), mul(v.nz, v.dz));
        let f = |x: f32, p: usize| fmt_f(f64::from(x), 2, p);
        let _ = writeln!(
            io.out,
            " after dot = {} D = ({}, {}, {}), N = ({}, {}, {})",
            f(dot, 2),
            f(v.dx, 2),
            f(v.dy, 2),
            f(v.dz, 2),
            f(v.nx, 1),
            f(v.ny, 1),
            f(v.nz, 1)
        );
    }
    NO_ERROR
}

/// `MRISaverageGradientsFast(mris, num_avgs)` (the official build never takes it in
/// `MRISaverageGradients`): `num_avgs` passes in which each unripped vertex takes `(d + d1 + ... +
/// dk) / (k + 1)` of its own and its unripped neighbours' gradients from the previous pass (the
/// sum and quotient in `float`); at the end each unripped vertex's `tdx`, `tdy`, `tdz` copy its
/// gradient. With `Gdiag_no` above 0 and `DIAG_VERBOSE`, `MRISaverageGradientsFast()` first.
pub fn average_gradients_fast(g: &mut Globals, io: &mut Io, mris: &mut Mris, num_avgs: i32) -> i32 {
    if g.gdiag_no > 0 && verbose_on(g) {
        let _ = writeln!(io.out, "MRISaverageGradientsFast()");
    }
    let n = nverts(mris);
    let lists: Vec<Option<Vec<usize>>> = (0..n)
        .map(|vno| {
            (mris.vertex(vno).ripflag == 0).then(|| {
                neighbours(mris, vno)
                    .into_iter()
                    .filter(|&x| mris.vertex(x).ripflag == 0)
                    .collect()
            })
        })
        .collect();
    for _ in 0..num_avgs {
        let mut t: Vec<[f32; 3]> = vec![[0.0; 3]; n];
        for (vno, list) in lists.iter().enumerate() {
            let Some(nb) = list else { continue };
            let v = mris.vertex(vno);
            let mut s = [v.dx, v.dy, v.dz];
            for &x in nb {
                let vn = mris.vertex(x);
                s[0] = add(s[0], vn.dx);
                s[1] = add(s[1], vn.dy);
                s[2] = add(s[2], vn.dz);
            }
            let num = (nb.len() + 1) as f32;
            t[vno] = [div(s[0], num), div(s[1], num), div(s[2], num)];
        }
        for (vno, list) in lists.iter().enumerate() {
            if list.is_some() {
                let v = mris.vertex_mut(vno);
                [v.dx, v.dy, v.dz] = t[vno];
            }
        }
    }
    for (vno, list) in lists.iter().enumerate() {
        if list.is_some() {
            let v = mris.vertex_mut(vno);
            (v.tdx, v.tdy, v.tdz) = (v.dx, v.dy, v.dz);
        }
    }
    NO_ERROR
}

/// `mrisAverageSignedGradients(mris, num_avgs)`: `num_avgs` passes in which each unripped vertex
/// takes the mean of its own gradient and those of its unripped neighbours (the first `vnum`)
/// whose gradient's dot product with its own (`float`) is not below 0: the sums in `double`,
/// divided by the count, kept in `tdx`, `tdy`, `tdz` and then copied into `dx`, `dy`, `dz`. With
/// `Gdiag_no` 0 or more, `before averaging dot = %2.2f ` and ` after dot = %2.2f (%2.3f, %2.3f,
/// %2.3f)` of that vertex (standard output). Nothing for `num_avgs` 0 or less.
pub fn average_signed_gradients(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    num_avgs: i32,
) -> i32 {
    if num_avgs <= 0 {
        return NO_ERROR;
    }
    let diag = diag_vertex(g, mris);
    if let Some(d) = diag {
        let v = mris.vertex(d);
        let dot = add(add(mul(v.dx, v.nx), mul(v.dy, v.ny)), mul(v.dz, v.nz));
        let _ = write!(
            io.out,
            "before averaging dot = {} ",
            fmt_f(f64::from(dot), 2, 2)
        );
    }
    let n = nverts(mris);
    for _ in 0..num_avgs {
        for vno in 0..n {
            if mris.vertex(vno).ripflag != 0 {
                continue;
            }
            let v = mris.vertex(vno);
            let own = [v.dx, v.dy, v.dz];
            let mut d = [f64::from(v.dx), f64::from(v.dy), f64::from(v.dz)];
            let mut num = 0i32;
            for x in neighbours(mris, vno) {
                let vn = mris.vertex(x);
                if vn.ripflag != 0 {
                    continue;
                }
                let dot = add(
                    add(mul(own[0], vn.dx), mul(own[1], vn.dy)),
                    mul(own[2], vn.dz),
                );
                if dot < 0.0 {
                    continue;
                }
                num += 1;
                d[0] = add64(d[0], f64::from(vn.dx));
                d[1] = add64(d[1], f64::from(vn.dy));
                d[2] = add64(d[2], f64::from(vn.dz));
            }
            num += 1;
            let c = f64::from(num as f32);
            let v = mris.vertex_mut(vno);
            v.tdx = div64(d[0], c) as f32;
            v.tdy = div64(d[1], c) as f32;
            v.tdz = div64(d[2], c) as f32;
        }
        for vno in 0..n {
            let v = mris.vertex_mut(vno);
            if v.ripflag != 0 {
                continue;
            }
            (v.dx, v.dy, v.dz) = (v.tdx, v.tdy, v.tdz);
        }
    }
    if let Some(d) = diag {
        let v = mris.vertex(d);
        let dot = add(add(mul(v.nx, v.dx), mul(v.ny, v.dy)), mul(v.nz, v.dz));
        let _ = writeln!(
            io.out,
            " after dot = {} ({}, {}, {})",
            fmt_f(f64::from(dot), 2, 2),
            fmt_f(f64::from(v.dx), 2, 3),
            fmt_f(f64::from(v.dy), 2, 3),
            fmt_f(f64::from(v.dz), 2, 3)
        );
    }
    NO_ERROR
}
