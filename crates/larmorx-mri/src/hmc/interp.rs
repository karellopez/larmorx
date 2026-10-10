// SPDX-License-Identifier: Apache-2.0
//! Trilinear interpolation in float32 (`specs/mcflirt.md` §6.2), x first, then y, then z:
//!
//! ```text
//! e00 = (v100 − v000)·fx + v000    e01 = (v101 − v001)·fx + v001
//! e10 = (v110 − v010)·fx + v010    e11 = (v111 − v011)·fx + v011
//! h0  = (e10 − e00)·fy + e00       h1  = (e11 − e01)·fy + e01
//! value = (h1 − h0)·fz + h0
//! ```

use super::volume::Volume;

/// The formula of §6.2 on the eight neighbours `v[c][b][a]` (`a`, `b`, `c` = offsets in x, y,
/// z) and the fractions.
#[inline(always)]
pub fn blend(v: [[[f32; 2]; 2]; 2], fx: f32, fy: f32, fz: f32) -> f32 {
    let e00 = (v[0][0][1] - v[0][0][0]) * fx + v[0][0][0];
    let e01 = (v[1][0][1] - v[1][0][0]) * fx + v[1][0][0];
    let e10 = (v[0][1][1] - v[0][1][0]) * fx + v[0][1][0];
    let e11 = (v[1][1][1] - v[1][1][0]) * fx + v[1][1][0];
    let h0 = (e10 - e00) * fy + e00;
    let h1 = (e11 - e01) * fy + e01;
    (h1 - h0) * fz + h0
}

/// Trilinear interpolation at a position whose eight neighbours all lie inside the volume
/// (`0 ≤ p` and `⌊p⌋ + 1 ≤ n − 1` on every axis).
#[inline(always)]
pub fn trilinear_inside(vol: &Volume, x: f32, y: f32, z: f32) -> f32 {
    let (ix, iy, iz) = (x as usize, y as usize, z as usize);
    let (fx, fy, fz) = (x - ix as f32, y - iy as f32, z - iz as f32);
    let nx = vol.shape[0];
    let sxy = nx * vol.shape[1];
    let base = ix + nx * iy + sxy * iz;
    let d = &vol.data;
    let v = [
        [[d[base], d[base + 1]], [d[base + nx], d[base + nx + 1]]],
        [
            [d[base + sxy], d[base + sxy + 1]],
            [d[base + sxy + nx], d[base + sxy + nx + 1]],
        ],
    ];
    blend(v, fx, fy, fz)
}

/// Trilinear interpolation at a non-negative position, neighbours outside the volume counting
/// as 0 (the reference grids, §5.3).
pub fn trilinear_zero(vol: &Volume, x: f32, y: f32, z: f32) -> f32 {
    let (ix, iy, iz) = (x as usize, y as usize, z as usize);
    let (fx, fy, fz) = (x - ix as f32, y - iy as f32, z - iz as f32);
    let [nx, ny, nz] = vol.shape;
    let at = |a: usize, b: usize, c: usize| {
        if a < nx && b < ny && c < nz {
            vol.at(a, b, c)
        } else {
            0.0
        }
    };
    let v = [
        [
            [at(ix, iy, iz), at(ix + 1, iy, iz)],
            [at(ix, iy + 1, iz), at(ix + 1, iy + 1, iz)],
        ],
        [
            [at(ix, iy, iz + 1), at(ix + 1, iy, iz + 1)],
            [at(ix, iy + 1, iz + 1), at(ix + 1, iy + 1, iz + 1)],
        ],
    ];
    blend(v, fx, fy, fz)
}

/// Trilinear interpolation of the volume extended by one voxel on every side, the extension
/// repeating the edge voxels (the final resampling, §9.2). The position must lie in
/// `[−1, n]` on every axis.
pub fn trilinear_extended(vol: &Volume, x: f32, y: f32, z: f32) -> f32 {
    let [nx, ny, nz] = vol.shape;
    let (fx0, fy0, fz0) = (x.floor(), y.floor(), z.floor());
    let (fx, fy, fz) = (x - fx0, y - fy0, z - fz0);
    let clamp = |i: f32, n: usize| -> [usize; 2] {
        let i = i as i64;
        let c = |k: i64| k.clamp(0, n as i64 - 1) as usize;
        [c(i), c(i + 1)]
    };
    let (xs, ys, zs) = (clamp(fx0, nx), clamp(fy0, ny), clamp(fz0, nz));
    let v = [0, 1].map(|c| [0, 1].map(|b| [0, 1].map(|a| vol.at(xs[a], ys[b], zs[c]))));
    blend(v, fx, fy, fz)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp() -> Volume {
        let mut data = Vec::new();
        for z in 0..3 {
            for y in 0..3 {
                for x in 0..3 {
                    data.push((x + 10 * y + 100 * z) as f32);
                }
            }
        }
        Volume::new([3, 3, 3], [1.0; 3], data)
    }

    #[test]
    fn linear_data_are_reproduced() {
        let v = ramp();
        assert_eq!(trilinear_inside(&v, 0.5, 1.25, 0.75), 0.5 + 12.5 + 75.0);
        assert_eq!(trilinear_zero(&v, 0.5, 1.25, 0.75), 0.5 + 12.5 + 75.0);
        assert_eq!(trilinear_extended(&v, 0.5, 1.25, 0.75), 0.5 + 12.5 + 75.0);
    }

    #[test]
    fn edges() {
        let v = ramp();
        // Beyond the last voxel the zero-padded version fades to 0.
        assert_eq!(trilinear_zero(&v, 2.5, 0.0, 0.0), 1.0);
        // The extended version repeats the edge voxels.
        assert_eq!(trilinear_extended(&v, 2.5, 0.0, 0.0), 2.0);
        assert_eq!(trilinear_extended(&v, -1.0, 0.0, 0.0), 0.0);
        assert_eq!(trilinear_extended(&v, -0.5, 3.0, 0.0), 20.0);
    }
}
