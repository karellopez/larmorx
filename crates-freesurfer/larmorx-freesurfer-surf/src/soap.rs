// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surf's soap bubbles (`utils/mrisurf_deform.cpp`, queue item 3.4-04, taken early; relay 42
//! of track D): `MRISsoapBubbleVertexPositions` (line 2502: `mris_fix_topology`'s smoothing of the
//! retessellated vertices, `mris_make_surfaces`, `mris_smooth`, the surface deformation),
//! `MRISweightedSoapBubbleVertexPositions` (2600), `MRISsoapBubbleOrigVertexPositions` (2700) and
//! `MRISsoapBubbleTargetVertexPositions` (2784). The marks: 1 fixed, 2 computed by an earlier pass,
//! 3 computed in this pass (not yet usable), 0 never computed.
//!
//! Every sum is the official build's: `float` sums in neighbour order (the weighted bubble: each
//! term `norm * x` in `double` added to the `float` sum, then rounded back to `float`, its division
//! in `double` stored as `float`).
//!
//! Defects (`larmorx_core::defects`):
//! - **FS-996** (memory outside): `MRISsoapBubbleVertexPositions` and the weighted bubble read the
//!   diagnostic vertex (`Gdiag_no`) before each pass with no check that the surface has it: a
//!   `Gdiag_no` beyond the surface's vertices reads outside them (its report printed from memory
//!   there, or a crash). Both modes skip the report, noted.

use larmorx_core::defects::FS_996;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{Fatal, NO_ERROR};

use crate::coords::{Io, set_original_xyz};
use crate::mris::Mris;
use crate::props8::print_vertex_stats;
use crate::surfread::verbose_show;
use crate::valcopy::{copy_marked_to_marked3, copy_marked3_to_marked};
use crate::views::CURRENT_VERTICES;

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// The first `vnum` neighbours of `vno`.
fn ring(mris: &Mris, vno: usize) -> Vec<usize> {
    let vt = mris.topology(vno);
    let n = usize::try_from(vt.vnum).unwrap_or(0);
    match vt.v.as_deref() {
        Some(v) if n > 0 => v[..n].iter().map(|&x| x as usize).collect(),
        _ => Vec::new(),
    }
}

/// `%2.1f` of a `float`.
fn f1(x: f32) -> String {
    fmt_f(f64::from(x), 2, 1)
}

/// The report on `Gdiag_no` before each pass of the plain and weighted bubbles: when the vertex is
/// unmarked, `v <n> @ (<x>, <y>, <z>) will be moved during soap bubble:` and a line per 1-hop
/// neighbour (`\tnbr <k>=<vno>, marked = <m>, (<x>, <y>, <z>)`) on standard output. FS-996: a
/// diagnostic vertex the surface does not have: no report, noted.
fn report_diag(g: &mut Globals, io: &mut Io, mris: &Mris, who: &str) {
    let d = g.gdiag_no;
    if d < 0 {
        return;
    }
    if d >= mris.nvertices {
        g.notices.add(
            &FS_996,
            format!(
                "{who}: the diagnostic vertex {d} is beyond the surface's {} vertices (FreeSurfer reads it there): no report",
                mris.nvertices
            ),
        );
        return;
    }
    let v = mris.vertex(d as usize);
    if v.marked != 0 {
        return;
    }
    let _ = writeln!(
        io.out,
        "v {d} @ ({}, {}, {}) will be moved during soap bubble:",
        f1(v.x),
        f1(v.y),
        f1(v.z)
    );
    for (n, vn) in ring(mris, d as usize).into_iter().enumerate() {
        let w = mris.vertex(vn);
        let _ = writeln!(
            io.out,
            "\tnbr {n}={vn}, marked = {}, ({}, {}, {})",
            w.marked,
            f1(w.x),
            f1(w.y),
            f1(w.z)
        );
    }
}

/// The second loop of the plain and weighted bubbles: each unripped vertex not fixed takes its new
/// position when marked (`iter <i>: moving v <n> from (<new>) --> (<old>)` on standard output for
/// `Gdiag_no` first, the new position printed before the old one), and a mark of 3 becomes 2.
fn move_marked(io: &mut Io, g: &Globals, mris: &mut Mris, i: i32) {
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 || v.marked == 1 {
            continue;
        }
        if vno as i32 == g.gdiag_no {
            let _ = writeln!(
                io.out,
                "iter {i}: moving v {vno} from ({}, {}, {}) --> ({}, {}, {})",
                f1(v.tdx),
                f1(v.tdy),
                f1(v.tdz),
                f1(v.x),
                f1(v.y),
                f1(v.z)
            );
        }
        let v = mris.vertex_mut(vno);
        if v.marked != 0 {
            v.x = v.tdx;
            v.y = v.tdy;
            v.z = v.tdz;
        }
        if v.marked == 3 {
            v.marked = 2;
        }
    }
}

/// `MRISsoapBubbleVertexPositions(mris, navgs)`: the marks saved (`MRIScopyMarkedToMarked3`); then
/// `navgs` passes: [`report_diag`]; every unripped vertex not fixed (mark not 1) gets in `tdx tdy
/// tdz` the mean (in `float`, from its own position and its unripped 1-hop neighbours' in order,
/// over their count) and the mark 3 (counted when it was 0); then [`move_marked`]; with
/// `DIAG_SHOW` and verbose diagnostics `<i>: <n> vertices marked` when some were. The marks
/// restored (`MRIScopyMarked3ToMarked`). `NO_ERROR`.
pub fn soap_bubble_vertex_positions(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    navgs: i32,
) -> i32 {
    copy_marked_to_marked3(mris);
    for i in 0..navgs.max(0) {
        report_diag(g, io, mris, "MRISsoapBubbleVertexPositions");
        let mut nmarked = 0;
        for vno in 0..nverts(mris) {
            let v = mris.vertex(vno);
            if v.ripflag != 0 || v.marked == 1 {
                continue;
            }
            let (mut x, mut y, mut z) = (v.x, v.y, v.z);
            let mut num = 1f32;
            for vn in ring(mris, vno) {
                let w = mris.vertex(vn);
                if w.ripflag != 0 {
                    continue;
                }
                num += 1.0;
                x += w.x;
                y += w.y;
                z += w.z;
            }
            let v = mris.vertex_mut(vno);
            v.tdx = x / num;
            v.tdy = y / num;
            v.tdz = z / num;
            if v.marked == 0 {
                nmarked += 1;
            }
            v.marked = 3;
        }
        move_marked(io, g, mris, i);
        if nmarked != 0 && verbose_show(g) {
            let _ = writeln!(io.out, "{i}: {nmarked} vertices marked");
        }
    }
    copy_marked3_to_marked(mris);
    NO_ERROR
}

/// `MRISweightedSoapBubbleVertexPositions(mris, navgs)`: `navgs` passes: [`report_diag`]; a
/// ripped or fixed vertex's `tdx tdy tdz` its position; every other vertex's the weighted mean of
/// its position (weight 1) and every 1-hop neighbour's (weight 1, 20 for a ripped one; each term in
/// `double` added to the `float` sum), over the weights' sum (in `double`), and the mark 3 (counted
/// when it was 0); then [`move_marked`]; with `DIAG_SHOW` and verbose diagnostics `<i>: <n>
/// vertices marked`; with verbose diagnostics `MRISprintVertexStats` of `Gdiag_no` (current
/// positions). The marks are left as the passes set them. `NO_ERROR`.
pub fn weighted_soap_bubble_vertex_positions(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    navgs: i32,
) -> Result<i32, Fatal> {
    for i in 0..navgs.max(0) {
        report_diag(g, io, mris, "MRISweightedSoapBubbleVertexPositions");
        let mut nmarked = 0;
        for vno in 0..nverts(mris) {
            let v = mris.vertex(vno);
            if v.ripflag != 0 || v.marked == 1 {
                let v = mris.vertex_mut(vno);
                v.tdx = v.x;
                v.tdy = v.y;
                v.tdz = v.z;
                continue;
            }
            let mut total = 1f64;
            let (mut x, mut y, mut z) = (v.x, v.y, v.z);
            for vn in ring(mris, vno) {
                let w = mris.vertex(vn);
                let norm = if w.ripflag != 0 { 20.0 } else { 1.0 };
                x = (f64::from(x) + norm * f64::from(w.x)) as f32;
                y = (f64::from(y) + norm * f64::from(w.y)) as f32;
                z = (f64::from(z) + norm * f64::from(w.z)) as f32;
                total += norm;
            }
            let v = mris.vertex_mut(vno);
            if total > 0.0 {
                v.tdx = (f64::from(x) / total) as f32;
                v.tdy = (f64::from(y) / total) as f32;
                v.tdz = (f64::from(z) / total) as f32;
                if v.marked == 0 {
                    nmarked += 1;
                }
                v.marked = 3;
            }
        }
        move_marked(io, g, mris, i);
        if nmarked != 0 && verbose_show(g) {
            let _ = writeln!(io.out, "{i}: {nmarked} vertices marked");
        }
        // (FS-996: a diagnostic vertex the surface does not have is not read)
        if diag::verbose_on(g) && g.gdiag_no < mris.nvertices {
            print_vertex_stats(g, io, mris, g.gdiag_no, CURRENT_VERTICES)?;
        }
    }
    Ok(NO_ERROR)
}

/// Which positions [`soap_bubble_positions`] smooths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Positions {
    /// `origx origy origz` (`MRISsoapBubbleOrigVertexPositions`).
    Original,
    /// `targx targy targz` (`MRISsoapBubbleTargetVertexPositions`).
    Target,
}

/// `MRISsoapBubbleOrigVertexPositions(mris, navgs)` and `MRISsoapBubbleTargetVertexPositions`: up
/// to `navgs` passes: every unripped vertex not fixed gets in `tdx tdy tdz` the mean (in `float`)
/// of its own position when its mark is 2 and of its unripped 1-hop neighbours' of mark 1 or 2 (in
/// order), when there is one, and the mark 3 (counted when it was 0); then each unripped vertex not
/// fixed and marked takes it (the original positions through `MRISsetOriginalXYZ`, which reports
/// a logic problem when the original distances are kept), a mark of 3 becomes 2; with `DIAG_SHOW`
/// `<i>: <n> vertices marked`; the passes end after the sixth that marked nothing. Then every
/// unripped vertex not fixed is unmarked (0). `NO_ERROR`.
pub fn soap_bubble_positions(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    navgs: i32,
    which: Positions,
) -> Result<i32, Fatal> {
    let get = |v: &crate::mris::Vertex| match which {
        Positions::Original => (v.origx, v.origy, v.origz),
        Positions::Target => (v.targx, v.targy, v.targz),
    };
    let file = format!("{}/utils/mrisurf_deform.cpp", diag::BUILD_ROOT);
    let mut none = 0;
    for i in 0..navgs.max(0) {
        let mut nmarked = 0;
        for vno in 0..nverts(mris) {
            let v = mris.vertex(vno);
            if v.ripflag != 0 || v.marked == 1 {
                continue;
            }
            let (mut x, mut y, mut z) = (0f32, 0f32, 0f32);
            let mut num = 0f32;
            if v.marked == 2 {
                (x, y, z) = get(v);
                num += 1.0;
            }
            for vn in ring(mris, vno) {
                let w = mris.vertex(vn);
                if w.ripflag != 0 || w.marked == 0 || w.marked > 2 {
                    continue;
                }
                let (a, b, c) = get(w);
                num += 1.0;
                x += a;
                y += b;
                z += c;
            }
            if num > 0.0 {
                let v = mris.vertex_mut(vno);
                v.tdx = x / num;
                v.tdy = y / num;
                v.tdz = z / num;
                if v.marked == 0 {
                    nmarked += 1;
                }
                v.marked = 3;
            }
        }
        for vno in 0..nverts(mris) {
            let v = mris.vertex(vno);
            if v.ripflag != 0 || v.marked == 1 {
                continue;
            }
            if v.marked != 0 {
                let (x, y, z) = (v.tdx, v.tdy, v.tdz);
                match which {
                    Positions::Original => {
                        set_original_xyz(g, io, mris, vno as i32, x, y, z, &file, 2760)?
                    }
                    Positions::Target => {
                        let v = mris.vertex_mut(vno);
                        (v.targx, v.targy, v.targz) = (x, y, z);
                    }
                }
            }
            let v = mris.vertex_mut(vno);
            if v.marked == 3 {
                v.marked = 2;
            }
        }
        if g.gdiag & diag::DIAG_SHOW != 0 {
            let _ = writeln!(io.out, "{i}: {nmarked} vertices marked");
        }
        if nmarked == 0 {
            none += 1;
            if none > 5 {
                break;
            }
        }
    }
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 || v.marked == 1 {
            continue;
        }
        v.marked = 0;
    }
    Ok(NO_ERROR)
}
