// SPDX-License-Identifier: Apache-2.0
//! The other interpolators: nearest neighbour, the windowed sincs of the final resampling and
//! of the stage-4 cost, and the cubic B-spline (`specs/mcflirt.md` §6.5, §9.3).

use std::f64::consts::PI;
use std::sync::OnceLock;

use larmorx_core::math;

use super::volume::Volume;

/// Nearest neighbour, rounding half away from zero, on the volume extended by its edge voxels.
pub fn nearest(vol: &Volume, p: [f32; 3]) -> f32 {
    let idx = [0, 1, 2].map(|k| (p[k].round() as i64).clamp(0, vol.shape[k] as i64 - 1) as usize);
    vol.at(idx[0], idx[1], idx[2])
}

/// `sin(πu)/(πu)`, and `1 − |u|` very near 0.
fn sinc(u: f64) -> f64 {
    if u.abs() < 1e-7 {
        1.0 - u.abs()
    } else {
        math::sin(PI * u) / (PI * u)
    }
}

/// A kernel of half-width 3 tabulated at `n` points over `[−3, 3]`, read by linear
/// interpolation.
struct Table {
    values: Vec<f32>,
    half: f32,
}

impl Table {
    fn new(points: usize, kernel: impl Fn(f64) -> f64) -> Table {
        let half = (points - 1) / 2;
        let values = (0..points)
            .map(|n| {
                let u = (n as f64 - half as f64) / half as f64 * 3.0;
                kernel(u) as f32
            })
            .collect();
        Table {
            values,
            half: half as f32,
        }
    }

    /// The kernel at `u`: 0 outside the table.
    #[inline]
    fn at(&self, u: f32) -> f32 {
        let x = u / 3.0 * self.half + self.half;
        let n = x.floor();
        if !(n >= 0.0 && (n as usize) + 1 < self.values.len()) {
            return 0.0;
        }
        let i = n as usize;
        let f = x - n;
        self.values[i] + f * (self.values[i + 1] - self.values[i])
    }
}

/// The final resampling's Blackman-windowed sinc, 1201 points.
fn blackman() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| {
        Table::new(1201, |u| {
            if u.abs() > 3.0 {
                0.0
            } else {
                sinc(u) * (0.42 + 0.5 * math::cos(PI * u / 3.0) + 0.08 * math::cos(2.0 * PI * u / 3.0))
            }
        })
    })
}

/// The stage-4 cost's Hanning-windowed sinc, 201 points.
fn hanning() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| {
        Table::new(201, |u| {
            if u.abs() > 3.0 {
                0.0
            } else {
                sinc(u) * (0.5 + 0.5 * math::cos(PI * u / 3.0))
            }
        })
    })
}

/// A 7 × 7 × 7 windowed-sinc interpolation over the voxels inside the volume, normalised by
/// the weight sum; `None` if that sum is at most 1e−9 in magnitude.
fn windowed(vol: &Volume, p: [f32; 3], table: &Table) -> Option<f32> {
    let base = p.map(|v| v.floor() as i64);
    let mut kx = [0.0f32; 7];
    let mut ky = [0.0f32; 7];
    let mut kz = [0.0f32; 7];
    for d in 0..7 {
        let off = d as i64 - 3;
        kx[d] = table.at(p[0] - (base[0] + off) as f32);
        ky[d] = table.at(p[1] - (base[1] + off) as f32);
        kz[d] = table.at(p[2] - (base[2] + off) as f32);
    }
    let [nx, ny, nz] = vol.shape.map(|n| n as i64);
    let (mut sum, mut wsum) = (0.0f32, 0.0f32);
    for (dz, &wz) in kz.iter().enumerate() {
        let z = base[2] + dz as i64 - 3;
        if z < 0 || z >= nz {
            continue;
        }
        for (dy, &wy) in ky.iter().enumerate() {
            let y = base[1] + dy as i64 - 3;
            if y < 0 || y >= ny {
                continue;
            }
            for (dx, &wx) in kx.iter().enumerate() {
                let x = base[0] + dx as i64 - 3;
                if x < 0 || x >= nx {
                    continue;
                }
                let w = wx * wy * wz;
                sum += w * vol.at(x as usize, y as usize, z as usize);
                wsum += w;
            }
        }
    }
    (wsum.abs() > 1e-9).then(|| sum / wsum)
}

/// The final `-sinc_final` interpolation (§9.3).
pub fn sinc_final(vol: &Volume, p: [f32; 3]) -> f32 {
    windowed(vol, p, blackman()).unwrap_or_else(|| {
        let i = [0, 1, 2].map(|k| (p[k].floor() as i64).clamp(0, vol.shape[k] as i64 - 1) as usize);
        vol.at(i[0], i[1], i[2])
    })
}

/// The stage-4 cost's sinc interpolation (§6.5); `background` when the weights vanish.
pub fn sinc_cost(vol: &Volume, p: [f32; 3], background: f32) -> f32 {
    windowed(vol, p, hanning()).unwrap_or(background)
}

/// Cubic B-spline coefficients of a volume (`-spline_final`, §9.3).
pub struct Spline {
    shape: [usize; 3],
    coef: Vec<f32>,
}

impl Spline {
    /// The coefficients by the recursive prefilter along x, y and z (pole `√3 − 2`).
    pub fn new(vol: &Volume) -> Spline {
        let [nx, ny, nz] = vol.shape;
        let mut c: Vec<f64> = vol.data.iter().map(|&v| f64::from(v)).collect();
        let mut line = Vec::new();
        for (axis, (n, stride)) in [(nx, 1), (ny, nx), (nz, nx * ny)].into_iter().enumerate() {
            if n < 2 {
                continue;
            }
            let others: Vec<usize> = (0..nx * ny * nz)
                .filter(|&i| {
                    let idx = [i % nx, (i / nx) % ny, i / (nx * ny)];
                    idx[axis] == 0
                })
                .collect();
            for start in others {
                line.clear();
                line.extend((0..n).map(|k| c[start + k * stride]));
                prefilter(&mut line);
                for (k, &v) in line.iter().enumerate() {
                    c[start + k * stride] = v;
                }
            }
        }
        Spline {
            shape: vol.shape,
            coef: c.into_iter().map(|v| v as f32).collect(),
        }
    }

    fn at(&self, x: i64, y: i64, z: i64) -> f32 {
        let [nx, ny, nz] = self.shape.map(|n| n as i64);
        let (x, y, z) = (x.clamp(0, nx - 1), y.clamp(0, ny - 1), z.clamp(0, nz - 1));
        self.coef[(x + nx * (y + ny * z)) as usize]
    }

    /// The spline's value at `p`; `background` where `⌊p⌋ < −1` or `⌊p⌋ ≥ n`.
    pub fn value(&self, p: [f32; 3], background: f32) -> f32 {
        let base = p.map(|v| v.floor() as i64);
        for k in 0..3 {
            if base[k] < -1 || base[k] >= self.shape[k] as i64 {
                return background;
            }
        }
        let w = [0, 1, 2].map(|k| bspline_weights(f64::from(p[k]) - base[k] as f64));
        let mut sum = 0.0f64;
        for (dz, &wz) in w[2].iter().enumerate() {
            for (dy, &wy) in w[1].iter().enumerate() {
                for (dx, &wx) in w[0].iter().enumerate() {
                    let v = self.at(
                        base[0] + dx as i64 - 1,
                        base[1] + dy as i64 - 1,
                        base[2] + dz as i64 - 1,
                    );
                    sum += wx * wy * wz * f64::from(v);
                }
            }
        }
        sum as f32
    }
}

/// The cubic B-spline weights of the four neighbours `⌊p⌋ − 1 … ⌊p⌋ + 2` at fraction `t`.
fn bspline_weights(t: f64) -> [f64; 4] {
    let s = 1.0 - t;
    [
        s * s * s / 6.0,
        (4.0 - 6.0 * t * t + 3.0 * t * t * t) / 6.0,
        (4.0 - 6.0 * s * s + 3.0 * s * s * s) / 6.0,
        t * t * t / 6.0,
    ]
}

/// The cubic B-spline prefilter of one line in place (gain 6, pole `√3 − 2`), the causal
/// sweep initialised from a mirror-symmetric extension truncated at precision 1e−8.
fn prefilter(c: &mut [f64]) {
    let n = c.len();
    let z = 3.0f64.sqrt() - 2.0;
    for v in c.iter_mut() {
        *v *= (1.0 - z) * (1.0 - 1.0 / z);
    }
    let horizon = ((1e-8f64.ln() / z.abs().ln()).ceil() as usize).min(n);
    let mut zn = z;
    let mut sum = c[0];
    for &v in &c[1..horizon] {
        sum += zn * v;
        zn *= z;
    }
    c[0] = sum;
    for k in 1..n {
        c[k] += z * c[k - 1];
    }
    c[n - 1] = (z / (z * z - 1.0)) * (c[n - 1] + z * c[n - 2]);
    for k in (0..n - 1).rev() {
        c[k] = z * (c[k + 1] - c[k]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp() -> Volume {
        let mut data = Vec::new();
        for z in 0..8 {
            for y in 0..8 {
                for x in 0..8 {
                    data.push((x + 2 * y + 3 * z) as f32);
                }
            }
        }
        Volume::new([8, 8, 8], [1.0; 3], data)
    }

    #[test]
    fn interpolators_at_grid_points_return_the_voxel() {
        let v = ramp();
        let p = [3.0, 4.0, 2.0];
        let expect = v.at(3, 4, 2);
        assert_eq!(nearest(&v, p), expect);
        assert!((sinc_final(&v, p) - expect).abs() < 1e-3);
        assert!((sinc_cost(&v, p, 0.0) - expect).abs() < 1e-3);
        assert!((Spline::new(&v).value(p, -1.0) - expect).abs() < 1e-3);
    }

    #[test]
    fn spline_reproduces_linear_data_inside() {
        let mut data = Vec::new();
        for z in 0..24 {
            for y in 0..24 {
                for x in 0..24 {
                    data.push((x + 2 * y + 3 * z) as f32);
                }
            }
        }
        let v = Volume::new([24, 24, 24], [1.0; 3], data);
        let s = Spline::new(&v);
        let got = s.value([11.5, 12.25, 11.75], 0.0);
        assert!((got - (11.5 + 24.5 + 35.25)).abs() < 1e-3, "{got}");
        assert_eq!(s.value([-2.5, 1.0, 1.0], -9.0), -9.0);
    }

    #[test]
    fn nearest_rounds_half_away_from_zero() {
        let v = ramp();
        assert_eq!(nearest(&v, [2.5, 0.0, 0.0]), 3.0);
        assert_eq!(nearest(&v, [-0.5, 0.0, 0.0]), 0.0);
    }
}
