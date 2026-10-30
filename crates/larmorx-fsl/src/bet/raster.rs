// SPDX-License-Identifier: Apache-2.0
//! Drawing a surface into a grid and filling what it encloses (`specs/bet.md` §4.6).

use super::mesh::{Mesh, V3, add, dist, scale, sub};

/// How a point on the surface picks its voxel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    /// `floor(p / d + 0.5)`: the mask.
    Floor,
    /// `trunc(p / d + 0.5)`: the outline.
    Trunc,
}

/// Calls `f` with the linear index of every voxel the surface's triangles are drawn on. For a
/// triangle `(a, b, c)` (written `c a b` in mesh files): points from `a` towards `c` every
/// `min(d) / 2` mm (`0, s, 2s, ...` up to the edge's length), and from `b` towards each of
/// them at the same spacing; each point to its voxel (observed: these are the points whose
/// voxels FSL draws).
pub fn draw(
    mesh: &Mesh,
    points: &[V3],
    shape: [usize; 3],
    voxel: [f64; 3],
    rounding: Rounding,
    mut f: impl FnMut(usize),
) {
    let step = voxel.iter().copied().fold(f64::INFINITY, f64::min) / 2.0;
    if !(step > 0.0) {
        return;
    }
    let [nx, ny, _] = shape;
    let mut put = |p: V3| {
        let mut idx = [0usize; 3];
        for a in 0..3 {
            let x = p[a] / voxel[a] + 0.5;
            let i = match rounding {
                Rounding::Floor => x.floor(),
                Rounding::Trunc => x.trunc(),
            };
            if !(i >= 0.0 && i < shape[a] as f64) {
                return;
            }
            idx[a] = i as usize;
        }
        f((idx[2] * ny + idx[1]) * nx + idx[0]);
    };
    for t in &mesh.triangles {
        let (a, b, c) = (
            points[t[0] as usize],
            points[t[1] as usize],
            points[t[2] as usize],
        );
        segment_points(a, c, step, |e| {
            segment_points(b, e, step, &mut put);
        });
    }
}

/// The points `p + k s u` (`u` the unit direction from `p` to `q`, `k s` accumulated, up to
/// the distance from `p` to `q`).
fn segment_points(p: V3, q: V3, step: f64, mut f: impl FnMut(V3)) {
    let d = sub(q, p);
    let len = dist(p, q);
    if !len.is_finite() {
        return;
    }
    let u = if len > 0.0 { scale(d, 1.0 / len) } else { d };
    let mut k = 0.0;
    while k <= len {
        f(add(p, scale(u, k)));
        k += step;
    }
}

/// Flood fill: every voxel of value `from` 6-connected to `start` (which must hold `from`)
/// becomes `to`. Returns how many changed.
pub fn fill(grid: &mut [u8], shape: [usize; 3], start: [usize; 3], from: u8, to: u8) -> usize {
    let [nx, ny, nz] = shape;
    let s = (start[2] * ny + start[1]) * nx + start[0];
    if grid[s] != from {
        return 0;
    }
    let slice = nx * ny;
    let mut stack = vec![s as u32];
    grid[s] = to;
    let mut n = 1;
    while let Some(p) = stack.pop() {
        let p = p as usize;
        let x = p % nx;
        let y = (p / nx) % ny;
        let z = p / slice;
        let mut visit = |q: usize, stack: &mut Vec<u32>| {
            if grid[q] == from {
                grid[q] = to;
                n += 1;
                stack.push(q as u32);
            }
        };
        if x > 0 {
            visit(p - 1, &mut stack);
        }
        if x + 1 < nx {
            visit(p + 1, &mut stack);
        }
        if y > 0 {
            visit(p - nx, &mut stack);
        }
        if y + 1 < ny {
            visit(p + nx, &mut stack);
        }
        if z > 0 {
            visit(p - slice, &mut stack);
        }
        if z + 1 < nz {
            visit(p + slice, &mut stack);
        }
    }
    n
}

/// FSL's fill from a start outside the image, point by point by the linear index (§4.6,
/// FSL-43 reproduced): inside the volume exactly; beyond it nothing is written and every
/// read is "not 1". Returns the number of reads beyond the volume.
pub fn fill_by_index(grid: &mut [u8], shape: [usize; 3], start: [i64; 3]) -> usize {
    let [nx, ny, nz] = shape.map(|n| n as i64);
    let total = grid.len() as i64;
    let place = |p: [i64; 3]| (p[2] * ny + p[1]) * nx + p[0];
    let mut beyond = 0usize;
    let mut stack = vec![start];
    while let Some(p) = stack.pop() {
        let at = place(p);
        if (0..total).contains(&at) {
            grid[at as usize] = 0;
        }
        let candidates = [
            ([p[0] - 1, p[1], p[2]], p[0] - 1 >= 0),
            ([p[0], p[1] - 1, p[2]], p[1] - 1 >= 0),
            ([p[0], p[1], p[2] - 1], p[2] - 1 >= 0),
            ([p[0] + 1, p[1], p[2]], p[0] + 1 < nx),
            ([p[0], p[1] + 1, p[2]], p[1] + 1 < ny),
            ([p[0], p[1], p[2] + 1], p[2] + 1 < nz),
        ];
        for (q, within) in candidates {
            if !within {
                continue;
            }
            let at = place(q);
            if (0..total).contains(&at) {
                if grid[at as usize] == 1 {
                    stack.push(q);
                }
            } else {
                beyond += 1;
            }
        }
    }
    beyond
}

/// The voxel holding a point (`trunc(p / d)`), as signed indices, or `None` for a point
/// that is not a number.
pub fn voxel_of(p: V3, voxel: [f64; 3]) -> Option<[i64; 3]> {
    let mut out = [0i64; 3];
    for a in 0..3 {
        let x = p[a] / voxel[a];
        out[a] = super::surface::c_int(x)?;
    }
    Some(out)
}
