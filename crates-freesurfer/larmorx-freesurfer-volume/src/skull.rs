// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MRIfindApproximateSkullBoundingBox` (`utils/mri.cpp`, 3.1-35): a box around the head found by
//! walking out from the centre of mass along each axis and looking for long runs of dark voxels.
//!
//! Defect (`larmorx_core::defects`; `docs/findings/freesurfer-volume.md` §6):
//! - **FS-305** (a hang): a volume whose centre of mass is not a number (infinite voxels above the
//!   threshold) starts the walks at `nint(NaN)`; FreeSurfer then walks about 2^31 voxels outside
//!   the volume and, when the left-right box is narrow, retries forever (`means[1] -= 10` stays
//!   NaN). An error here in both modes.

use std::io::Write;

use larmorx_core::defects::FS_305;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::mri::{Mri, MriRegion};

use crate::coords::{center_of_mass, sample_volume_nearest};

/// Bright voxels in a row that end a dark run's claim (3 cm of bright stuff outside the brain).
const MAX_LIGHT: i32 = 30;
/// The shortest dark run taken as the edge.
const MIN_DARK: i32 = 10;

/// One walk from `from` (down to 0, or up to below `n`): each sample is the nearest voxel at
/// `at(c)`; a sample below `thresh` extends the dark run (its start the first), a bright one ends
/// it and, after more than [`MAX_LIGHT`] in a row, forgets the longest run; the longest run's
/// start (`ge`: a run as long as the longest replaces it) is the edge. Returns the edge and the
/// longest run.
#[allow(clippy::too_many_arguments)]
fn walk(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    thresh: i32,
    from: i32,
    down: bool,
    n: i32,
    ge: bool,
    at: &dyn Fn(i32) -> [i32; 3],
) -> (i32, i32) {
    let (mut nlight, mut ndark, mut max_dark) = (0i32, 0i32, 0i32);
    let (mut start, mut edge) = (from, from);
    let mut c = from;
    while if down { c >= 0 } else { c < n } {
        let [x, y, z] = at(c);
        let (_, val) = sample_volume_nearest(g, err, mri, f64::from(x), f64::from(y), f64::from(z));
        if val < f64::from(thresh) {
            if ndark == 0 {
                start = c;
            }
            ndark += 1;
            nlight = 0;
        } else {
            nlight += 1;
            if nlight > MAX_LIGHT {
                max_dark = 0;
            }
            if ndark > max_dark || (ge && ndark == max_dark) {
                max_dark = ndark;
                edge = start;
            }
            ndark = 0;
        }
        c = if down { c - 1 } else { c + 1 };
    }
    if ndark > max_dark {
        max_dark = ndark;
        edge = start;
    }
    (edge, max_dark)
}

/// `MRIfindApproximateSkullBoundingBox(mri, thresh, box)`: from the centre of mass of the voxels
/// above `(unsigned char) thresh` ([`center_of_mass`]), the box's left and right edges along x
/// (at the centre's y and z); while the box is 10 voxels wide or less, the centre's y moved down
/// by 10 (`left/right detection failed, moving y coord to <old> from <new>` on standard output)
/// and the search repeated, failing with `MRIfindApproximateSkullBoundingBox failed: check input
/// volume` (`ErrorExit`) once y is negative; then the superior edge (y downwards at x 20 voxels left
/// of the centre, a final dark run also taken when at least half the longest after more than 15
/// bright voxels), the inferior edge, the posterior and anterior edges (z), and when the box is
/// less than 5 slices deep the z edges again 15 voxels to the left. Each edge without a dark run
/// of 10 voxels is the volume's border. `NO_ERROR`.
pub fn find_approximate_skull_bounding_box(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    thresh: i32,
    bx: &mut MriRegion,
) -> Result<i32, Fatal> {
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let (_, mut means) = center_of_mass(g, err, mri, thresh as u8)?;
    if means.iter().any(|m| m.is_nan()) {
        g.notices.add(
            &FS_305,
            "MRIfindApproximateSkullBoundingBox of a volume whose centre of mass is not a number",
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "MRIfindApproximateSkullBoundingBox: the centre of mass is not a number (FreeSurfer walks 2^31 voxels or never returns here)\n",
        )?;
        return Ok(ERROR_BADPARM);
    }
    loop {
        let (y, z) = (nint(g, means[1]), nint(g, means[2]));
        let x0 = nint(g, means[0]);
        let (left, dark) = walk(g, err, mri, thresh, x0, true, w, false, &|x| [x, y, z]);
        bx.x = if dark < MIN_DARK { 0 } else { left };
        let x0 = nint(g, means[0]);
        let (right, dark) = walk(g, err, mri, thresh, x0, false, w, true, &|x| [x, y, z]);
        let right = if dark < MIN_DARK { w - 1 } else { right };
        bx.dx = right.wrapping_sub(bx.x).wrapping_add(1);
        if bx.dx <= 10 {
            means[1] -= 10.0;
            let to = nint(g, means[1]).wrapping_add(10);
            let from = nint(g, means[1]);
            let _ = writeln!(
                out,
                "left/right detection failed, moving y coord to {to} from {from}"
            );
            if means[1] < 0.0 {
                log::error_exit(
                    g,
                    err,
                    ERROR_BADPARM,
                    "MRIfindApproximateSkullBoundingBox failed: check input volume",
                )?;
                break;
            }
        } else {
            break;
        }
    }
    // the superior edge, away from the inter-hemispheric fissure
    let x = nint(g, means[0]).wrapping_sub(20).max(0);
    let z = nint(g, means[2]);
    let y0 = nint(g, means[1]);
    {
        let (mut nlight, mut ndark, mut max_dark, mut max_light) = (0i32, 0i32, 0i32, 0i32);
        let (mut start, mut y1) = (y0, y0);
        let mut y = y0;
        while y >= 0 {
            let (_, val) =
                sample_volume_nearest(g, err, mri, f64::from(x), f64::from(y), f64::from(z));
            if val < f64::from(thresh) {
                if nlight > max_light {
                    max_light = nlight;
                }
                if ndark == 0 {
                    start = y;
                }
                ndark += 1;
                nlight = 0;
            } else {
                nlight += 1;
                if nlight > MAX_LIGHT {
                    max_dark = 0;
                }
                if ndark >= max_dark {
                    max_dark = ndark;
                    y1 = start;
                    max_light = 0;
                }
                ndark = 0;
            }
            y -= 1;
        }
        if ndark > max_dark || (y < 0 && ndark > max_dark / 2 && max_light > MAX_LIGHT / 2) {
            max_dark = ndark;
            y1 = start;
        }
        bx.y = if max_dark < MIN_DARK { 0 } else { y1 };
    }
    let (x, z) = (nint(g, means[0]), nint(g, means[2]));
    let (inf, dark) = walk(g, err, mri, thresh, y0, false, h, true, &|y| [x, y, z]);
    let inf = if dark < MIN_DARK { h - 1 } else { inf };
    bx.dy = inf.wrapping_sub(bx.y).wrapping_add(1);
    let (mut x, y, z0) = (nint(g, means[0]), nint(g, means[1]), nint(g, means[2]));
    let z_edges = |g: &mut Globals, err: &mut dyn Write, x: i32, bx: &mut MriRegion| {
        let (post, dark) = walk(g, err, mri, thresh, z0, true, d, true, &|z| [x, y, z]);
        bx.z = if dark < MIN_DARK { 0 } else { post };
        let (ant, dark) = walk(g, err, mri, thresh, z0, false, d, true, &|z| [x, y, z]);
        let ant = if dark < MIN_DARK { d - 1 } else { ant };
        bx.dz = ant.wrapping_sub(bx.z).wrapping_add(1);
    };
    z_edges(g, err, x, bx);
    if bx.dz < 5 {
        // too close to the midline
        x = x.wrapping_sub(15);
        z_edges(g, err, x, bx);
    }
    Ok(NO_ERROR)
}
