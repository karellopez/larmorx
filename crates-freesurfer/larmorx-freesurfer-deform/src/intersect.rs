// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MRISremoveIntersections` (`utils/mrisurf_deform.cpp` 4010-4083; queue item 3.4-06, taken
//! early; relay 44 of track D): mris_fix_topology's, mris_remove_intersection's,
//! mris_place_surface's, mris_remesh's and mris_topo_fixer's removal of self-intersections by
//! soap-bubble smoothing of the intersecting vertices.

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{Fatal, NO_ERROR};
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::discrete::{clear_marks, dilate_marked};
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::positions::{restore_vertex_positions, save_vertex_positions};
use larmorx_freesurfer_surf::props10::mris_mark_intersections;
use larmorx_freesurfer_surf::soap::soap_bubble_vertex_positions;
use larmorx_freesurfer_surf::valmarks::not_marked;
use larmorx_freesurfer_surf::views::TMP2_VERTICES;

/// `MAX_INT_REMOVAL_NEIGHBORS`.
const MAX_INT_REMOVAL_NEIGHBORS: i32 = 5;

/// `MRISremoveIntersections(mris, FillHoles)`: the intersecting faces' vertices marked
/// (`mrisMarkIntersections` with `FillHoles`); none: `NO_ERROR` at once. Otherwise `removing
/// intersecting faces`, the positions kept (`TMP2_VERTICES`), and until no vertex is marked: a
/// count that did not fall (`step <k> with no progress (num=<n>, old_num=<old>)`, the search
/// given up after 16 such steps; the neighbourhood size stays 0, FreeSurfer's growth of it being
/// commented out) or a new lowest count (the positions kept again); `<step>: <n> intersecting`
/// (`%03d`); the marks inverted and the unmarked vertices (the intersecting ones) soap-bubble
/// smoothed with the rest fixed (`MRISsoapBubbleVertexPositions`, 100 passes); at most 102 steps;
/// the marks cleared and counted again. A last count above the lowest restores the kept positions
/// and counts once more without filling holes. `terminating search with <n> intersecting`.
/// Standard output. `NO_ERROR`.
pub fn remove_intersections(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    fill_holes: i32,
) -> Result<i32, Fatal> {
    let mut n = 0i32;
    let mut num = mris_mark_intersections(g, io, mris, fill_holes)?;
    if num == 0 {
        return Ok(NO_ERROR);
    }
    let _ = writeln!(io.out, "removing intersecting faces");
    let mut old_num = mris.nvertices;
    let mut min_int = old_num;
    save_vertex_positions(g, io, mris, TMP2_VERTICES)?;
    let mut nbrs = 0i32;
    let mut no_progress = 0i32;
    clear_marks(mris);
    num = mris_mark_intersections(g, io, mris, fill_holes)?;
    while num > 0 {
        if num > old_num || (num == old_num && no_progress >= nbrs) {
            no_progress += 1;
            let _ = writeln!(
                io.out,
                "step {no_progress} with no progress (num={num}, old_num={old_num})"
            );
            if nbrs >= MAX_INT_REMOVAL_NEIGHBORS {
                if no_progress >= nbrs {
                    break;
                }
                let _ = writeln!(io.out, "max nbrs reached, resetting neighborhood size");
                nbrs = 0;
            } else if no_progress > 15 {
                break;
            }
        } else {
            if num < old_num {
                no_progress = 0;
            } else {
                no_progress += 1;
            }
            if num < min_int {
                min_int = num;
                save_vertex_positions(g, io, mris, TMP2_VERTICES)?;
            }
        }
        dilate_marked(mris, nbrs);
        old_num = num;
        let _ = writeln!(io.out, "{n:03}: {num} intersecting");
        not_marked(mris);
        soap_bubble_vertex_positions(g, io, mris, 100);
        let step = n;
        n += 1;
        if step > 100 {
            break;
        }
        clear_marks(mris);
        num = mris_mark_intersections(g, io, mris, fill_holes)?;
    }
    if num > min_int {
        restore_vertex_positions(g, io, mris, TMP2_VERTICES)?;
        num = mris_mark_intersections(g, io, mris, 0)?;
    }
    let _ = writeln!(io.out, "terminating search with {num} intersecting");
    Ok(NO_ERROR)
}
