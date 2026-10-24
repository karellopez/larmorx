// SPDX-License-Identifier: Apache-2.0
//! 3dAutomask's own steps after the mask (`specs/3dAutomask.md` §6): the depth, `-dilate` and
//! `-erode` with their hole filling, the count, the empty planes and `-SI`.

use crate::geometry::{InputGeometry, plumb_test};
use crate::msg::Message;

use super::morph::{self, Neighbourhood};
use super::{cluster, count, invert};

/// The orientation of a voxel axis: the first and second letter of AFNI's orientation code
/// (`L`, `R` for an axis running from left to right), and the voxel size along it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axis {
    /// `None` for an axis AFNI could not orient (`??`).
    pub letters: Option<[char; 2]>,
    /// The voxel size (millimetres), positive.
    pub step: f32,
}

/// The axes of a grid read as `g`: each column's direction is the world axis of its largest
/// entry, from the negative to the positive end in NIfTI's RAS convention (an axis whose
/// entry is positive along x runs from left to right: `L`, `R`); the step is the column's
/// length (§6.1's steps, in millimetres). An axis whose world axis another column shares, or
/// whose column is zero, has no orientation.
pub fn axes_of(g: &InputGeometry, steps_mm: [f32; 3]) -> [Axis; 3] {
    let (_, rows, _) = plumb_test(&g.m);
    let letters = |j: usize| -> Option<[char; 2]> {
        let row = rows[j];
        let v = g.m[row][j];
        if v == 0.0 || !v.is_finite() || rows.iter().filter(|&&r| r == row).count() > 1 {
            return None;
        }
        let pos = v > 0.0;
        Some(match (row, pos) {
            (0, true) => ['L', 'R'],
            (0, false) => ['R', 'L'],
            (1, true) => ['P', 'A'],
            (1, false) => ['A', 'P'],
            (_, true) => ['I', 'S'],
            (_, false) => ['S', 'I'],
        })
    };
    [0, 1, 2].map(|j| Axis {
        letters: letters(j),
        step: steps_mm[j].abs(),
    })
}

/// The program's settings after the mask.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Steps {
    /// The neighbourhood of dilations, erosions and the depth (`-NN1/2/3`; default edges).
    pub nn: Neighbourhood,
    pub dilate: usize,
    pub erode: usize,
    /// `-SI` (millimetres).
    pub si: Option<f32>,
    /// `-depth`.
    pub depth: bool,
}

/// The mask was empty after the steps: AFNI's "No voxels? Quitting without saving mask".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoVoxels;

/// The depth (spec §6.3) of `mask`, with its messages.
pub fn depth_of(
    mask: &[u8],
    dims: [usize; 3],
    nn: Neighbourhood,
    log: &mut Vec<Message>,
) -> Vec<i16> {
    let (depth, layers) = morph::depth(mask, dims, nn);
    let mut left = count(mask);
    for n in layers {
        log.push(Message::More(format!("Eroded   {n} voxels")));
        log.push(Message::Info(format!(
            "Peeled {n} voxels from mask of {left} (now {})",
            left - n
        )));
        left -= n;
    }
    depth
}

/// Fills holes with side `w` until nothing is filled (spec §6.1).
fn fill(mask: &mut [u8], dims: [usize; 3], w: usize) {
    morph::fill_until_done(mask, dims, w);
}

/// `-dilate` and `-erode` (spec §6.1), with their messages.
pub fn dilate_erode(mask: &mut [u8], dims: [usize; 3], steps: &Steps, log: &mut Vec<Message>) {
    if steps.dilate == 0 && steps.erode == 0 {
        return;
    }
    let r = |n: usize| (0.032 * n as f64).round_ties_even() as usize;
    let w = 1.max(r(dims[0])).max(r(dims[1])).max(r(dims[2]));
    if steps.dilate > 0 {
        log.push(Message::Info("Dilating automask".into()));
        for _ in 0..steps.dilate {
            morph::dilate(mask, dims, steps.nn);
            fill(mask, dims, w);
        }
    }
    if steps.erode > 0 {
        log.push(Message::Info("Eroding automask".into()));
        for _ in 0..steps.erode {
            let n = morph::erode(mask, dims, steps.nn);
            if n > 0 {
                log.push(Message::More(format!("Eroded   {n} voxels")));
            }
            fill(mask, dims, w);
        }
    }
    // Every hole filled: the largest cluster of the outside, inverted back.
    invert(mask);
    log.push(Message::More("Clustering voxels ...".into()));
    let n = cluster::keep_largest(mask, dims);
    log.push(Message::More(format!("Largest cluster has {n} voxels")));
    invert(mask);
}

/// The count of mask voxels (spec §6.2), or [`NoVoxels`].
pub fn report_count(mask: &[u8], log: &mut Vec<Message>) -> Result<usize, NoVoxels> {
    let n = count(mask);
    let total = mask.len();
    let p = 100.0 * n as f64 / total as f64;
    log.push(Message::Info(format!(
        "{n} voxels in the mask [out of {total}: {p:.2}%]"
    )));
    if n == 0 { Err(NoVoxels) } else { Ok(n) }
}

/// The leading and trailing planes of each axis without a mask voxel, reported (spec §6.2),
/// and returned as `(first, last)` counts.
pub fn empty_planes(
    mask: &[u8],
    dims: [usize; 3],
    axes: &[Axis; 3],
    log: &mut Vec<Message>,
) -> [(usize, usize); 3] {
    let [nx, ny, nz] = dims;
    // Whether each plane of each axis holds a mask voxel.
    let mut has = [vec![false; nx], vec![false; ny], vec![false; nz]];
    for k in 0..nz {
        for j in 0..ny {
            let row = &mask[nx * (j + ny * k)..nx * (j + ny * k) + nx];
            let mut any = false;
            for (i, &b) in row.iter().enumerate() {
                if b != 0 {
                    has[0][i] = true;
                    any = true;
                }
            }
            if any {
                has[1][j] = true;
                has[2][k] = true;
            }
        }
    }
    let mut out = [(0, 0); 3];
    for a in 0..3 {
        let n = has[a].len();
        let first = has[a].iter().position(|&h| h).unwrap_or(n);
        let last = has[a].iter().rev().position(|&h| h).unwrap_or(n);
        out[a] = (first, last);
        let name = ["x", "y", "z"][a];
        let [c0, c1] = axes[a].letters.unwrap_or(['?', '?']);
        let code = format!("{c0}{c1}");
        log.push(Message::Info(format!(
            "first {first:3} {name}-planes are zero [from {}]",
            &code[..1]
        )));
        log.push(Message::Info(format!(
            "last  {last:3} {name}-planes are zero [from {}]",
            &code[1..]
        )));
    }
    out
}

/// `-SI h` (spec §6.2): on the axis whose first planes are superior, every plane from the
/// first non-empty one plus `h` millimetres on is cleared; on the axis whose last planes are
/// superior, every plane up to the last non-empty one minus `h`. The later axis wins when
/// both kinds qualify.
pub fn si_clip(
    mask: &mut [u8],
    dims: [usize; 3],
    axes: &[Axis; 3],
    planes: &[(usize, usize); 3],
    h: f32,
    log: &mut Vec<Message>,
) {
    let mut range: Option<(usize, i64, i64)> = None;
    for a in 0..3 {
        let Some([first, second]) = axes[a].letters else {
            continue;
        };
        let n = dims[a] as i64;
        let steps = (f64::from(h) / f64::from(axes[a].step) + 0.5) as i64;
        if first == 'S' {
            let a0 = planes[a].0 as i64;
            range = Some((a, a0 + steps, n - 1));
        } else if second == 'S' {
            let last = n - 1 - planes[a].1 as i64;
            range = Some((a, 0, last - steps));
        }
    }
    let Some((a, lo, hi)) = range else { return };
    let n = dims[a] as i64;
    let lo = lo.max(0);
    let hi = hi.min(n - 1);
    if lo > hi {
        return;
    }
    log.push(Message::Info(format!(
        "SI clipping mask along axis {} from {lo}..{hi}",
        ["x", "y", "z"][a]
    )));
    let [nx, ny, nz] = dims;
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let c = [i, j, k][a] as i64;
                if c >= lo && c <= hi {
                    mask[i + nx * (j + ny * k)] = 0;
                }
            }
        }
    }
    log.push(Message::Info(format!(
        "{} voxels left [out of {}]",
        count(mask),
        mask.len()
    )));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axes() -> [Axis; 3] {
        let a = |l: [char; 2]| Axis {
            letters: Some(l),
            step: 3.0,
        };
        [a(['L', 'R']), a(['P', 'A']), a(['I', 'S'])]
    }

    #[test]
    fn planes_and_si() {
        let dims = [4, 3, 5];
        let mut m = vec![0u8; 60];
        // Voxels at z = 1 and z = 3, x = 2, y = 1.
        m[2 + 4 * (1 + 3)] = 1;
        m[2 + 4 * (1 + 3 * 3)] = 1;
        let mut log = Vec::new();
        let planes = empty_planes(&m, dims, &axes(), &mut log);
        assert_eq!(planes, [(2, 1), (1, 1), (1, 1)]);
        assert_eq!(
            log[0],
            Message::Info("first   2 x-planes are zero [from L]".into())
        );
        assert_eq!(
            log[1],
            Message::Info("last    1 x-planes are zero [from R]".into())
        );
        // -SI 3 mm along z (superior last): planes 0 .. (3 - 1) cleared, the top one kept.
        let mut log = Vec::new();
        si_clip(&mut m, dims, &axes(), &planes, 3.0, &mut log);
        assert_eq!(
            log[0],
            Message::Info("SI clipping mask along axis z from 0..2".into())
        );
        assert_eq!(count(&m), 1);
    }
}
