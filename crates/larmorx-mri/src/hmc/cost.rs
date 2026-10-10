// SPDX-License-Identifier: Apache-2.0
//! The registration cost (`specs/mcflirt.md` §6): a reference grid compared with a test
//! volume at full resolution, sampled at the positions a candidate matrix maps the grid to.
//!
//! The sampling loops reproduce the float32 arithmetic of §6.1–§6.4: incremental float32
//! positions along each row, only points inside the test volume, a linear edge weight, and the
//! correlation's sums accumulated in three float32 levels (row, slice, grid), with its peculiar
//! count `N`.

use super::interp::trilinear_inside;

/// DEV ONLY: second variant word.
pub fn variant2() -> u64 {
    static V: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("LARMORX_HMC_V2")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    })
}

/// DEV ONLY: numeric variants under test (removed before commit).
pub fn variant() -> u64 {
    static V: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("LARMORX_HMC_VARIANT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    })
}
use super::rigid::{self, Mat4};
use super::volume::Volume;

/// The cost functions (`-cost`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CostFunction {
    /// Normalised correlation as mcflirt computes it (the default).
    #[default]
    NormCorr,
    /// Pearson's correlation, centred (larmorx's option; not an mcflirt cost).
    Pearson,
    LeastSquares,
    CorrRatio,
    Woods,
    MutualInfo,
    NormMi,
}

impl CostFunction {
    /// The cost named on the command line (`normcorr`, `leastsquares`, `corratio`, `woods`,
    /// `mutualinfo`, `normmi`).
    pub fn from_name(name: &str) -> Option<CostFunction> {
        Some(match name {
            "normcorr" => CostFunction::NormCorr,
            "leastsquares" => CostFunction::LeastSquares,
            "corratio" => CostFunction::CorrRatio,
            "woods" => CostFunction::Woods,
            "mutualinfo" => CostFunction::MutualInfo,
            "normmi" => CostFunction::NormMi,
            "pearson" => CostFunction::Pearson,
            _ => return None,
        })
    }

    /// The name of the cost.
    pub fn name(self) -> &'static str {
        match self {
            CostFunction::NormCorr => "normcorr",
            CostFunction::Pearson => "pearson",
            CostFunction::LeastSquares => "leastsquares",
            CostFunction::CorrRatio => "corratio",
            CostFunction::Woods => "woods",
            CostFunction::MutualInfo => "mutualinfo",
            CostFunction::NormMi => "normmi",
        }
    }
}

/// The voxel-to-voxel map of a candidate matrix `m` (test FSL-mm → reference FSL-mm):
/// `B = Dv⁻¹ · M⁻¹ · Dg` in double, each entry rounded to float32 (§6.1). Row `k` gives test
/// voxel coordinate `k` of a grid point `(x, y, z, 1)`.
pub fn voxel_map(m: &Mat4, test_voxel: [f32; 3], grid_voxel: [f32; 3]) -> [[f32; 4]; 3] {
    let v = variant();
    let m = &if v & 4194304 != 0 { m.map(|r| r.map(|x| f64::from(x as f32))) } else { *m };
    let dv_inv = rigid::diagonal(test_voxel.map(|d| 1.0 / f64::from(d)));
    let dg = rigid::diagonal(grid_voxel.map(f64::from));
    let mut minv = if v & (1 << 45) != 0 {
        // rigid inverse: R^T and -R^T t
        let mut r = rigid::IDENTITY;
        for i in 0..3 { for j in 0..3 { r[i][j] = m[j][i]; } }
        for i in 0..3 { r[i][3] = -(r[i][0] * m[0][3] + r[i][1] * m[1][3] + r[i][2] * m[2][3]); }
        r
    } else if v & (1 << 46) != 0 {
        larmorx_core::linalg::inverse4(m).unwrap_or(rigid::NAN_MATRIX)
    } else {
        rigid::inverse(m)
    };
    if v & 8388608 != 0 {
        minv = minv.map(|r| r.map(|x| f64::from(x as f32)));
    }
    let b = if v & 16777216 != 0 {
        // scale rows/cols explicitly
        let mut b = minv;
        for i in 0..3 { for j in 0..4 { b[i][j] = minv[i][j] / f64::from(test_voxel[i]); } }
        for i in 0..3 { for j in 0..3 { b[i][j] *= f64::from(grid_voxel[j]); } }
        b
    } else {
        rigid::mul(&rigid::mul(&dv_inv, &minv), &dg)
    };
    BD.with(|c| c.set(b));
    [0, 1, 2].map(|i| [0, 1, 2, 3].map(|j| b[i][j] as f32))
}

thread_local! {
    pub static BD: std::cell::Cell<[[f64; 4]; 4]> = const { std::cell::Cell::new([[0.0; 4]; 4]) };
}

/// What the cost needs about a test volume, computed once per volume and stage.
pub struct TestVolume<'a> {
    pub vol: &'a Volume,
    /// `n_k − 1.0001` in float32: the largest admissible position per axis.
    pub bound: [f32; 3],
    /// The edge-weighting width per axis in voxels (`smooth / d_k`); 0 turns weighting off.
    pub sigma: [f32; 3],
}

impl<'a> TestVolume<'a> {
    pub fn new(vol: &'a Volume, smooth: f32) -> TestVolume<'a> {
        TestVolume {
            vol,
            bound: vol.shape.map(|n| if variant() & 65536 != 0 { n as f32 - 1.0001f32 } else { (n as f64 - 1.0001) as f32 }),
            sigma: vol.voxel_size.map(|d| { let v2 = variant2(); if v2 & 4 != 0 { smooth } else if v2 & 8 != 0 { smooth / 8.0 } else if v2 & 16 != 0 { 2.0 * smooth / d } else if v2 & 32 != 0 { 0.5 * smooth / d } else { smooth / d } }),
        }
    }

    #[inline(always)]
    fn inside(&self, p: [f32; 3]) -> bool {
        p[0] >= 0.0
            && p[0] <= self.bound[0]
            && p[1] >= 0.0
            && p[1] <= self.bound[1]
            && p[2] >= 0.0
            && p[2] <= self.bound[2]
    }

    /// The edge weight at `p` (§6.3).
    #[inline(always)]
    fn weight(&self, p: [f32; 3]) -> f32 {
        let v = variant();
        if v & (1 << 47) != 0 {
            return 1.0;
        }
        let wv = (v >> 40) & 0x1f;
        if wv >= 5 {
            let axis = |k: usize| -> f32 {
                let d = self.vol.voxel_size[k];
                let s = self.sigma[k];
                let b = self.bound[k];
                if p[k] < s {
                    if wv == 5 { (p[k] * d) / 1.0 } else if wv == 6 { p[k] * (d / 1.0) } else { p[k] * d }
                } else if b - p[k] < s {
                    if wv == 5 { ((b - p[k]) * d) / 1.0 } else if wv == 6 { (b - p[k]) * (d / 1.0) } else { (b - p[k]) * d }
                } else {
                    1.0
                }
            };
            let w = axis(0) * axis(1) * axis(2);
            return if w < 0.0 { 0.0 } else { w };
        }
        if wv != 0 {
            let axis = |k: usize| -> f64 {
                let d = f64::from(self.vol.voxel_size[k]);
                let sig = 1.0 / d; // smooth = 1
                let pk = f64::from(p[k]);
                let b = if wv == 3 { self.vol.shape[k] as f64 - 1.0 } else if wv == 4 { f64::from(self.bound[k]) } else { self.vol.shape[k] as f64 - 1.0001 };
                if pk < sig { pk / sig } else if b - pk < sig { (b - pk) / sig } else { 1.0 }
            };
            if wv == 1 || wv == 3 || wv == 4 {
                let w = (axis(0) as f32) * (axis(1) as f32) * (axis(2) as f32);
                return if w < 0.0 { 0.0 } else { w };
            }
            let w = (axis(0) * axis(1) * axis(2)) as f32;
            return if w < 0.0 { 0.0 } else { w };
        }
        let axis = |k: usize| {
            let s = self.sigma[k];
            if v & 1 != 0 {
                let inv = 1.0 / s;
                if p[k] < s {
                    p[k] * inv
                } else if self.bound[k] - p[k] < s {
                    (self.bound[k] - p[k]) * inv
                } else {
                    1.0
                }
            } else if p[k] < s {
                p[k] / s
            } else if self.bound[k] - p[k] < s {
                (self.bound[k] - p[k]) / s
            } else {
                1.0
            }
        };
        let w = if v & 64 != 0 { axis(0) * (axis(1) * axis(2)) } else { axis(0) * axis(1) * axis(2) };
        if w < 0.0 { 0.0 } else { w }
    }
}

/// The admissible grid points of one row (§6.1): the x range where the row's positions
/// `row0 + x·step` lie inside the test volume, found analytically and intersected with the
/// grid. `None` if empty.
#[inline]
fn row_range(row0: [f32; 3], step: [f32; 3], bound: [f32; 3], nx: usize) -> Option<(usize, usize)> {
    if variant() & (1 << 30) != 0 {
        return Some((0, nx - 1));
    }
    if variant() & (1 << 31) != 0 {
        let mut lo = 0.0f32;
        let mut hi = nx as f32 - 1.0;
        for k in 0..3 {
            let (r, s, b) = (row0[k], step[k], bound[k]);
            if s == 0.0 {
                if !(r >= 0.0 && r <= b) {
                    return None;
                }
                continue;
            }
            let (a0, a1) = ((0.0 - r) / s, (b - r) / s);
            let (l, h) = if s > 0.0 { (a0, a1) } else { (a1, a0) };
            lo = lo.max(l);
            hi = hi.min(h);
        }
        let (lo, hi) = (lo.ceil(), hi.floor());
        return if lo <= hi && lo >= 0.0 { Some((lo as usize, hi as usize)) } else { None };
    }
    let mut lo = 0.0f64;
    let mut hi = nx as f64 - 1.0;
    for k in 0..3 {
        let (r, s, b) = (f64::from(row0[k]), f64::from(step[k]), f64::from(bound[k]));
        if s == 0.0 {
            if !(r >= 0.0 && r <= b) {
                return None;
            }
            continue;
        }
        let (a0, a1) = ((0.0 - r) / s, (b - r) / s);
        let (l, h) = if s > 0.0 { (a0, a1) } else { (a1, a0) };
        lo = lo.max(l);
        hi = hi.min(h);
    }
    // NaN bounds fail these comparisons and give an empty range.
    let (lo, hi) = (lo.ceil(), hi.floor());
    if lo <= hi && lo >= 0.0 {
        Some((lo as usize, hi as usize))
    } else {
        None
    }
}

/// The five weighted sums of the correlation and the total weight.
#[derive(Clone, Copy, Default)]
struct Sums {
    sx: f32,
    sy: f32,
    sxx: f32,
    syy: f32,
    sxy: f32,
}

impl Sums {
    #[inline(always)]
    fn add(&mut self, o: &Sums) {
        self.sx += o.sx;
        self.sy += o.sy;
        self.sxx += o.sxx;
        self.syy += o.syy;
        self.sxy += o.sxy;
    }
}

/// The sums of one slice of the grid and the total weight of each of its rows.
struct SliceSums {
    sums: Sums,
    rows: Vec<Sums>,
    /// The number of contributing points (the count of the unweighted, centred form).
    points: u32,
    row_weights: Vec<f32>,
}

/// Visits the admissible points of grid slice `z` (§6.1), calling `point(g, p, w)` with the
/// grid value, the test position and the edge weight, and collecting the per-row sums.
#[inline(always)]
fn slice_sums(
    grid: &Volume,
    test: &TestVolume<'_>,
    b: &[[f32; 4]; 3],
    z: usize,
    weighted: bool,
) -> SliceSums {
    let [gx, gy, _] = grid.shape;
    let step = [b[0][0], b[1][0], b[2][0]];
    let zf = z as f32;
    let mut out = SliceSums {
        sums: Sums::default(),
        rows: Vec::new(),
        points: 0,
        row_weights: Vec::with_capacity(gy),
    };
    let mut acc = [0, 1, 2].map(|k| zf * b[k][2] + b[k][3]);
    for y in 0..gy {
        let yf = y as f32;
        let row0 = if variant2() & (1 << 23) != 0 {
            let bd = BD.with(|c| c.get());
            [0, 1, 2].map(|k| (f64::from(yf) * bd[k][1] + f64::from(zf) * bd[k][2] + bd[k][3]) as f32)
        } else if variant() & (1 << 57) != 0 {
            let r = acc;
            acc = [acc[0] + b[0][1], acc[1] + b[1][1], acc[2] + b[2][1]];
            r
        } else if variant() & (1 << 58) != 0 {
            [0, 1, 2].map(|k| (f64::from(yf) * f64::from(b[k][1]) + f64::from(zf) * f64::from(b[k][2]) + f64::from(b[k][3])) as f32)
        } else if variant() & 1048576 != 0 {
            [0, 1, 2].map(|k| b[k][3] + yf * b[k][1] + zf * b[k][2])
        } else if variant() & 2097152 != 0 {
            [0, 1, 2].map(|k| yf * b[k][1] + (zf * b[k][2] + b[k][3]))
        } else {
            [0, 1, 2].map(|k| yf * b[k][1] + zf * b[k][2] + b[k][3])
        };
        let mut row = Sums::default();
        let mut row_weight = 0.0f32;
        let mut had_range = false;
        let mut had_points = false;
        let mut terms: Vec<(f32, f32, f32, f32, f32, f32)> = Vec::new();
        if let Some((x0, x1)) = row_range(row0, step, test.bound, gx) {
            had_range = true;
            let x0f = x0 as f32;
            let vv = variant();
            let mut p = if vv & (1 << 49) != 0 {
                [0, 1, 2].map(|k| (f64::from(row0[k]) + x0 as f64 * f64::from(step[k])) as f32)
            } else {
                [0, 1, 2].map(|k| row0[k] + x0f * step[k])
            };
            let grow = &grid.data[gx * (y + gy * z)..gx * (y + gy * z + 1)];
            let mut x = x0;
            while x <= x1 && !test.inside(p) {
                p = [p[0] + step[0], p[1] + step[1], p[2] + step[2]];
                x += 1;
            }
            while x <= x1 && test.inside(p) {
                let g = grow[x];
                let v = if variant() & 524288 != 0 {
                    super::interp::trilinear_weights(test.vol, p[0], p[1], p[2])
                } else {
                    trilinear_inside(test.vol, p[0], p[1], p[2])
                };
                let w = if weighted { test.weight(p) } else { 1.0 };
                let wg = w * g;
                let wv = w * v;
                let v2 = variant2();
                if v2 & (1 << 22) != 0 {
                    terms.push((wg, wv, wg * g, wv * v, wg * v, w));
                    out.points += 1;
                    had_points = true;
                    x += 1;
                    p = [p[0] + step[0], p[1] + step[1], p[2] + step[2]];
                    continue;
                }
                row.sx += wg;
                row.sy += wv;
                row.sxx += if v2 & 1024 != 0 { w * (g * g) } else { wg * g };
                row.syy += if v2 & 1024 != 0 { w * (v * v) } else { wv * v };
                row.sxy += if v2 & 2048 != 0 { wv * g } else if v2 & 4096 != 0 { w * (g * v) } else { wg * v };
                row_weight += w;
                out.points += 1;
                had_points = true;
                x += 1;
                if vv & (1 << 48) != 0 {
                    let xf = x as f32;
                    p = [0, 1, 2].map(|k| row0[k] + xf * step[k]);
                } else {
                    p = [p[0] + step[0], p[1] + step[1], p[2] + step[2]];
                }
            }
        }
        for t in terms.drain(..).rev() {
            row.sx += t.0;
            row.sy += t.1;
            row.sxx += t.2;
            row.syy += t.3;
            row.sxy += t.4;
            row_weight += t.5;
        }
        if std::env::var_os("LARMORX_HMC_DUMP").is_some() {
            println!("z {z} y {y} row0 {:?} pts {} w {:e} sx {:e} sy {:e} sxx {:e} syy {:e} sxy {:e}", row0, had_points as u8, row_weight, row.sx, row.sy, row.sxx, row.syy, row.sxy);
        }
        out.sums.add(&row);
        out.rows.push(row);
        let vv = variant();
        if vv & (1 << 52) != 0 && !had_range {
            continue;
        }
        if vv & (1 << 53) != 0 && !had_points {
            continue;
        }
        out.row_weights.push(row_weight);
    }
    out
}

/// The normalised-correlation cost `1 − |r|` of the grid against the test volume mapped by
/// `b` (§6.4). With weighting (`smooth > 0`), the correlation uses mcflirt's count `N`; without,
/// the number of contributing points.
pub fn normcorr(grid: &Volume, test: &TestVolume<'_>, b: &[[f32; 4]; 3]) -> f32 {
    let weighted = test.sigma.iter().all(|&s| s > 0.0);
    let mut total = Sums::default();
    let (mut c, mut a, mut n) = (0.0f32, 0.0f32, 0.0f32);
    let mut points = 0u32;
    let nv = variant() >> 32;
    let mut wsum = 0.0f32;
    for z in 0..grid.shape[2] {
        let s = slice_sums(grid, test, b, z, weighted);
        if nv == 2 {
            c = 0.0;
            a = 0.0;
        }
        for &w in &s.row_weights {
            c += w;
            a += c;
            wsum += w;
        }
        n += a;
        if variant2() & 256 != 0 {
            for r in &s.rows {
                total.add(r);
            }
        } else {
            total.add(&s.sums);
        }
        points += s.points;
    }
    if nv == 1 {
        n = a;
    } else if nv == 3 {
        n = wsum;
    } else if nv == 4 {
        n = 2.0 * n;
    } else if nv == 5 {
        n = 1.01 * n;
    } else if nv == 6 {
        n *= 1.0001;
    } else if nv == 7 {
        n *= 1.00001;
    } else if nv == 8 {
        n *= 1.000001;
    } else if nv == 9 {
        n = f32::from_bits(n.to_bits() + 1);
    }
    let v2 = variant2();
    if weighted && (v2 & (1 << 20) != 0 || v2 & (1 << 21) != 0) {
        let s = &total;
        if !(n > 2.0) {
            return 1.0;
        }
        let n1 = f64::from(n) - 1.0;
        let nn = n * n;
        let (cov, varx, vary) = if v2 & (1 << 20) != 0 {
            (
                f64::from((f64::from(s.sxy) / n1 - f64::from((s.sx * s.sy) / nn)) as f32),
                f64::from((f64::from(s.sxx) / n1 - f64::from((s.sx * s.sx) / nn)) as f32),
                f64::from((f64::from(s.syy) / n1 - f64::from((s.sy * s.sy) / nn)) as f32),
            )
        } else {
            (
                f64::from(s.sxy) / n1 - f64::from((s.sx * s.sy) / nn),
                f64::from(s.sxx) / n1 - f64::from((s.sx * s.sx) / nn),
                f64::from(s.syy) / n1 - f64::from((s.sy * s.sy) / nn),
            )
        };
        let r = if varx > 0.0 && vary > 0.0 { cov / varx.sqrt() / vary.sqrt() } else { 0.0 };
        return (1.0 - r.abs()) as f32;
    }
    let r = if weighted {
        correlation_weighted(&total, n)
    } else {
        correlation_counted(&total, points as f32)
    };
    1.0 - r.abs()
}

/// The correlation of §6.4 with mcflirt's count `N`: the divisions by `N − 1` in double, the
/// products and the second divisions in float32.
fn correlation_weighted(s: &Sums, n: f32) -> f32 {
    if !(n > 2.0) {
        return 0.0;
    }
    let v = variant();
    let n1 = f64::from(n) - 1.0;
    let nn = n * n;
    let v2 = variant2();
    if v2 & (1 << 18) != 0 || v2 & (1 << 19) != 0 {
        let (cov, varx, vary) = if v2 & (1 << 18) != 0 {
            (
                f64::from(s.sxy) / n1 - f64::from((s.sx * s.sy) / nn),
                f64::from(s.sxx) / n1 - f64::from((s.sx * s.sx) / nn),
                f64::from(s.syy) / n1 - f64::from((s.sy * s.sy) / nn),
            )
        } else {
            let nn = f64::from(n) * f64::from(n);
            let (sx, sy) = (f64::from(s.sx), f64::from(s.sy));
            (f64::from(s.sxy) / n1 - sx * sy / nn, f64::from(s.sxx) / n1 - sx * sx / nn, f64::from(s.syy) / n1 - sy * sy / nn)
        };
        return if varx > 0.0 && vary > 0.0 { (cov / varx.sqrt() / vary.sqrt()) as f32 } else { 0.0 };
    }
    let (cov, varx, vary) = if v & 16 != 0 {
        let n1 = n - 1.0;
        (s.sxy / n1 - (s.sx * s.sy) / nn, s.sxx / n1 - (s.sx * s.sx) / nn, s.syy / n1 - (s.sy * s.sy) / nn)
    } else if v & 268435456 != 0 {
        (
            (f64::from(s.sxy) / n1) as f32 - (s.sx * s.sy) / nn,
            (f64::from(s.sxx) / n1) as f32 - (s.sx * s.sx) / nn,
            (f64::from(s.syy) / n1) as f32 - (s.sy * s.sy) / nn,
        )
    } else if v & 536870912 != 0 {
        // divisions by N-1 as multiplications by a double reciprocal
        let inv = 1.0 / n1;
        (
            (f64::from(s.sxy) * inv - f64::from((s.sx * s.sy) / nn)) as f32,
            (f64::from(s.sxx) * inv - f64::from((s.sx * s.sx) / nn)) as f32,
            (f64::from(s.syy) * inv - f64::from((s.sy * s.sy) / nn)) as f32,
        )
    } else if v & 32 != 0 {
        let nn = f64::from(n) * f64::from(n);
        let (sx, sy) = (f64::from(s.sx), f64::from(s.sy));
        (
            (f64::from(s.sxy) / n1 - sx * sy / nn) as f32,
            (f64::from(s.sxx) / n1 - sx * sx / nn) as f32,
            (f64::from(s.syy) / n1 - sy * sy / nn) as f32,
        )
    } else {
        (
            (f64::from(s.sxy) / n1 - f64::from((s.sx * s.sy) / nn)) as f32,
            (f64::from(s.sxx) / n1 - f64::from((s.sx * s.sx) / nn)) as f32,
            (f64::from(s.syy) / n1 - f64::from((s.sy * s.sy) / nn)) as f32,
        )
    };
    if varx > 0.0 && vary > 0.0 {
        if v & 4 != 0 {
            (f64::from(cov) / f64::from(varx).sqrt() / f64::from(vary).sqrt()) as f32
        } else if v & 8 != 0 {
            cov / (varx.sqrt() * vary.sqrt())
        } else if v & 128 != 0 {
            (f64::from(cov) / (f64::from(varx) * f64::from(vary)).sqrt()) as f32
        } else {
            cov / varx.sqrt() / vary.sqrt()
        }
    } else {
        0.0
    }
}

/// The unweighted correlation (`-smooth 0`): Pearson's, in float32.
fn correlation_counted(s: &Sums, n: f32) -> f32 {
    if !(n > 2.0) {
        return 0.0;
    }
    let n1 = n - 1.0;
    let nn = n * n;
    let cov = s.sxy / n1 - (s.sx * s.sy) / nn;
    let varx = s.sxx / n1 - (s.sx * s.sx) / nn;
    let vary = s.syy / n1 - (s.sy * s.sy) / nn;
    if varx > 0.0 && vary > 0.0 {
        cov / varx.sqrt() / vary.sqrt()
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hmc::rigid::IDENTITY;

    fn blob(shape: [usize; 3], voxel: [f32; 3]) -> Volume {
        let mut data = Vec::new();
        for z in 0..shape[2] {
            for y in 0..shape[1] {
                for x in 0..shape[0] {
                    let idx = [x, y, z];
                    let d: f32 = (0..3)
                        .map(|k| {
                            let u = idx[k] as f32 - (shape[k] as f32 - 1.0) / 2.0;
                            u * u
                        })
                        .sum();
                    data.push(1000.0 * (-d / 30.0).exp() + 10.0);
                }
            }
        }
        Volume::new(shape, voxel, data)
    }

    #[test]
    fn identical_images_correlate_best() {
        let v = blob([20, 22, 16], [3.0, 3.0, 4.0]);
        let grid = crate::hmc::grid::subsample(&v, 4.0);
        let test = TestVolume::new(&v, 1.0);
        let at = |m: &Mat4| normcorr(&grid, &test, &voxel_map(m, v.voxel_size, grid.voxel_size));
        let c0 = at(&IDENTITY);
        let mut shifted = IDENTITY;
        shifted[0][3] = 2.0;
        assert!(c0 < at(&shifted), "{c0} {}", at(&shifted));
        assert!((0.0..0.01).contains(&c0));
    }

    #[test]
    fn row_ranges() {
        // A row stepping by 0.5 voxels from -1: positions inside [0, 8.9999] for x in 2..=19.
        let r = row_range([-1.0, 1.0, 1.0], [0.5, 0.0, 0.0], [8.9999, 5.0, 5.0], 40);
        assert_eq!(r, Some((2, 19)));
        // Backwards.
        let r = row_range([9.5, 1.0, 1.0], [-0.5, 0.0, 0.0], [8.9999, 5.0, 5.0], 40);
        assert_eq!(r, Some((2, 19)));
        // A fixed coordinate outside: empty.
        assert_eq!(row_range([1.0, -1.0, 1.0], [0.5, 0.0, 0.0], [8.9999, 5.0, 5.0], 40), None);
        assert_eq!(row_range([f32::NAN; 3], [0.5, 0.0, 0.0], [8.9999; 3], 40), None);
    }
}
