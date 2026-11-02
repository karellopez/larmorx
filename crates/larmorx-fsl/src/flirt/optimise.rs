// SPDX-License-Identifier: Apache-2.0
//! One optimisation (`specs/flirt.md` §8.2) and the parameter subsets (§8.3), on the shared
//! engine's parametrisation (`larmorx_mri::hmc::rigid`) and line search
//! (`larmorx_mri::hmc::search::line_search_from`).

use larmorx_mri::hmc::rigid::Params;
use larmorx_mri::hmc::search::line_search_from;

use super::cost::Log;
use super::mat::Mat4;
use super::param::{self, AngleRep};

/// FLIRT's default tolerances (§8.2): rotations (rad), translations (mm), scales, skews.
pub const DEFAULT_TOLERANCES: [f32; 12] = [
    0.005, 0.005, 0.005, 0.2, 0.2, 0.2, 0.002, 0.002, 0.002, 0.001, 0.001, 0.001,
];

/// A parameter subset (§8.3): the 12 × n mask `P` (`x = P·y + x₀`) and its pseudo-inverse.
#[derive(Clone, Debug, PartialEq)]
pub struct Subset {
    /// `P`, 12 rows of `n` columns.
    pub mask: Vec<Vec<f64>>,
    /// `pinv(P)`, `n` rows of 12.
    pub pinv: Vec<Vec<f64>>,
}

impl Subset {
    /// The subset of the columns `cols` (each 12 values). `None` for a mask with a NaN or
    /// infinite value (refused, rule B).
    pub fn new(cols: &[[f64; 12]]) -> Option<Subset> {
        if cols.iter().flatten().any(|v| !v.is_finite()) {
            return None;
        }
        let n = cols.len();
        let mask: Vec<Vec<f64>> = (0..12)
            .map(|i| (0..n).map(|c| cols[c][i]).collect())
            .collect();
        Some(Subset {
            pinv: pinv(&mask, n),
            mask,
        })
    }

    pub fn len(&self) -> usize {
        self.pinv.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pinv.is_empty()
    }

    /// `P·y` (plain sums of rounded products, in order).
    fn expand(&self, y: &[f64]) -> [f64; 12] {
        let mut x = [0.0; 12];
        for (i, xi) in x.iter_mut().enumerate() {
            let mut s = 0.0;
            for (c, &yc) in y.iter().enumerate() {
                s += self.mask[i][c] * yc;
            }
            *xi = s;
        }
        x
    }

    /// `pinv(P)·d`.
    fn reduce(&self, d: &[f64; 12]) -> Vec<f64> {
        self.pinv
            .iter()
            .map(|row| row.iter().zip(d).map(|(a, b)| a * b).sum())
            .collect()
    }
}

/// The pseudo-inverse of a 12 × n mask (§8.3): masks of zeros and ones with at most one
/// non-zero per row by the closed form `(1/√k)²`, diagonal masks element by element, any other
/// by a singular value decomposition (one-sided Jacobi), singular values below
/// `max(12, n)·s₁·ε` dropped.
fn pinv(p: &[Vec<f64>], n: usize) -> Vec<Vec<f64>> {
    let simple = p.iter().all(|row| {
        row.iter().all(|&v| v == 0.0 || v == 1.0) && row.iter().filter(|&&v| v != 0.0).count() <= 1
    });
    if simple {
        return (0..n)
            .map(|c| {
                let k = p.iter().filter(|row| row[c] == 1.0).count() as f64;
                let r = 1.0 / k.sqrt();
                let v = r * r;
                (0..12)
                    .map(|i| if p[i][c] == 1.0 { v } else { 0.0 })
                    .collect()
            })
            .collect();
    }
    let diagonal = n == 12 && (0..12).all(|i| (0..12).all(|j| i == j || p[i][j] == 0.0));
    if diagonal {
        let max = (0..12).map(|i| p[i][i].abs()).fold(0.0f64, f64::max);
        let tol = 12.0 * max * f64::EPSILON;
        return (0..12)
            .map(|i| {
                (0..12)
                    .map(|j| {
                        if i == j && p[i][i].abs() >= tol {
                            1.0 / p[i][i]
                        } else {
                            0.0
                        }
                    })
                    .collect()
            })
            .collect();
    }
    // one-sided Jacobi: A·V with orthogonal columns; s = column norms, U = A·V / s
    let mut a: Vec<Vec<f64>> = p.to_vec();
    let mut v: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
        .collect();
    for _ in 0..60 {
        let mut off = 0.0f64;
        for i in 0..n {
            for j in i + 1..n {
                let (mut alpha, mut beta, mut gamma) = (0.0, 0.0, 0.0);
                for row in &a {
                    alpha += row[i] * row[i];
                    beta += row[j] * row[j];
                    gamma += row[i] * row[j];
                }
                if gamma == 0.0 {
                    continue;
                }
                off = off.max(gamma.abs() / (alpha * beta).sqrt().max(f64::MIN_POSITIVE));
                let zeta = (beta - alpha) / (2.0 * gamma);
                let t = zeta.signum().max(0.0).mul_add(2.0, -1.0)
                    / (zeta.abs() + (1.0 + zeta * zeta).sqrt());
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = c * t;
                for row in a.iter_mut() {
                    let (x, y) = (row[i], row[j]);
                    row[i] = c * x - s * y;
                    row[j] = s * x + c * y;
                }
                for row in v.iter_mut() {
                    let (x, y) = (row[i], row[j]);
                    row[i] = c * x - s * y;
                    row[j] = s * x + c * y;
                }
            }
        }
        if off < 1e-15 {
            break;
        }
    }
    let s: Vec<f64> = (0..n)
        .map(|j| a.iter().map(|row| row[j] * row[j]).sum::<f64>().sqrt())
        .collect();
    let s1 = s.iter().copied().fold(0.0f64, f64::max);
    let tol = 12f64.max(n as f64) * s1 * f64::EPSILON;
    let mut out = vec![vec![0.0; 12]; n];
    for k in 0..n {
        if s[k] < tol || s[k] == 0.0 {
            continue;
        }
        let inv = 1.0 / s[k];
        for i in 0..n {
            for r in 0..12 {
                // V(i, k)·(1/s_k)·U(r, k), U(r, k) = A(r, k)/s_k
                out[i][r] += v[i][k] * inv * (a[r][k] * inv);
            }
        }
    }
    out
}

/// The settings of one optimisation (§7.3, §8.2).
#[derive(Clone, Debug)]
pub struct Settings {
    /// The tolerances as stored (`setoption tolerance` divides by the requested scale).
    pub tolerances: Vec<f32>,
    /// The requested scale (§7.2).
    pub scale: f32,
    /// The bound guesses of the sweeps (the last for later sweeps).
    pub bound_guess: Vec<f32>,
    /// Powell's correction (`optimisationtype powell`).
    pub powell: bool,
    /// The rotation's parameters (`-anglerep`; the search always uses Euler angles).
    pub rep: AngleRep,
    /// Where the quaternion's messages go.
    pub log: Option<std::sync::Arc<Log>>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            tolerances: DEFAULT_TOLERANCES.to_vec(),
            scale: 1.0,
            bound_guess: vec![10.0, 1.0],
            powell: false,
            rep: AngleRep::Euler,
            log: None,
        }
    }
}

impl Settings {
    /// The tolerance of parameter `i` at the requested scale (`None` when the list is short).
    pub fn tolerance(&self, i: usize) -> Option<f32> {
        self.tolerances.get(i).map(|t| t * self.scale)
    }

    fn guess(&self, sweep: usize) -> f32 {
        self.bound_guess
            .get(sweep)
            .or(self.bound_guess.last())
            .copied()
            .unwrap_or(1.0)
    }
}

/// The clamp of `dof` to 6..12 (§8.2), with FLIRT's message when it changes.
pub fn clamp_dof(dof: i64) -> (usize, Option<String>) {
    if dof < 6 {
        (6, Some(format!("Erroneous dof {dof} : using 6 instead")))
    } else if dof > 12 {
        (12, Some(format!("Erroneous dof {dof} : using 12 instead")))
    } else {
        (dof as usize, None)
    }
}

/// A line search along `dir` from `v` with tolerance `u` and first step `guess·u`; returns the
/// moved point and its cost.
fn along(
    cost: &dyn Fn(&[f64]) -> f32,
    v: &[f64],
    dir: &[f64],
    y0: f32,
    u: f32,
    guess: f32,
) -> (Vec<f64>, f32) {
    let mut phi = |a: f32| {
        let q: Vec<f64> = v
            .iter()
            .zip(dir)
            .map(|(x, d)| x + f64::from(a) * d)
            .collect();
        cost(&q)
    };
    let (a, y) = line_search_from(&mut phi, y0, u, guess);
    let moved = v
        .iter()
        .zip(dir)
        .map(|(x, d)| x + f64::from(a) * d)
        .collect();
    (moved, y)
}

/// The optimisation of a vector `v0` with tolerances `tol` (§8.2): up to `maxit` sweeps of line
/// searches along the current directions, the convergence test after each, Powell's correction
/// when asked. `n_tol` is the convergence test's count of tolerances: 12 for the parameters,
/// the subset's size for a subset's variables (observed: with the search's four variables FSL
/// sweeps again where a count of 12 would stop). Returns the final vector and its cost.
fn minimise(
    cost: &dyn Fn(&[f64]) -> f32,
    v0: Vec<f64>,
    tol: &[f32],
    n_tol: usize,
    maxit: usize,
    s: &Settings,
) -> (Vec<f64>, f32) {
    let n = v0.len();
    let mut dirs: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
        .collect();
    let dir_tol = |d: &[f64]| -> f32 {
        // a unit direction keeps its parameter's tolerance; any other the step of one tolerance
        // unit along it
        if let Some(k) = (0..n).find(|&k| d[k] == 1.0 && (0..n).all(|j| j == k || d[j] == 0.0)) {
            return tol[k];
        }
        let ss: f64 = d
            .iter()
            .zip(tol)
            .map(|(x, &t)| {
                if t.abs() > 1e-15 {
                    (x / f64::from(t)).powi(2)
                } else {
                    0.0
                }
            })
            .sum();
        if ss > 0.0 {
            (1.0 / ss.sqrt()) as f32
        } else {
            1.0
        }
    };
    let mut v = v0;
    let mut best = 0.0f32;
    for sweep in 0..maxit {
        let guess = s.guess(sweep);
        let start = v.clone();
        if best == 0.0 {
            best = cost(&v);
        }
        let f0 = best;
        let mut deltas = Vec::with_capacity(n);
        for d in &dirs {
            if best == 0.0 {
                best = cost(&v);
            }
            let (moved, y) = along(cost, &v, d, best, dir_tol(d), guess);
            deltas.push(y - best);
            v = moved;
            best = y;
        }
        // convergence: Σ |(x_start − x_end)/t| / n_tol below 1 (two interleaved accumulators)
        let (mut even, mut odd) = (0.0f64, 0.0f64);
        for i in 0..n {
            let t = f64::from(tol[i]);
            let term = if t.abs() <= 1e-15 {
                0.0
            } else {
                ((start[i] - v[i]) / t).abs()
            };
            if i % 2 == 0 {
                even += term;
            } else {
                odd += term;
            }
        }
        if (even + odd) / (n_tol as f64) < 1.0 {
            break;
        }
        if s.powell {
            let f1 = best;
            let diff: Vec<f64> = v.iter().zip(&start).map(|(a, b)| a - b).collect();
            let extrap: Vec<f64> = start.iter().zip(&diff).map(|(a, d)| a + 2.0 * d).collect();
            let fe = cost(&extrap);
            let (m, dm) =
                deltas
                    .iter()
                    .enumerate()
                    .fold((0usize, f32::INFINITY), |(bi, bv), (i, &d)| {
                        if d < bv { (i, d) } else { (bi, bv) }
                    });
            let adm = dm.abs();
            let lhs = 2.0 * (f0 - 2.0 * f1 + fe) * (f0 - f1 - adm) * (f0 - f1 - adm);
            let rhs = (f0 - fe) * (f0 - fe) * adm;
            if lhs < rhs && fe < f0 {
                let (moved, y) = along(cost, &v, &diff, best, dir_tol(&diff), guess);
                v = moved;
                best = y;
                dirs[m] = diff;
            }
        }
    }
    (v, best)
}

/// One optimisation of the trial matrix `start` (§8.2): its parameters about `centre`, the first
/// `dof` optimised (or, with `subset`, the subset's variables from `start`'s own parameters),
/// `maxit` sweeps. `eval` is the cost of a trial matrix. Returns the matrix and its cost.
pub fn optimise(
    eval: &dyn Fn(&Mat4) -> f32,
    start: &Mat4,
    centre: [f64; 3],
    dof: usize,
    maxit: usize,
    s: &Settings,
    subset: Option<&Subset>,
) -> Result<(Mat4, f32), String> {
    let x0: Params = param::decompose(s.rep, start, centre);
    let (x, c) = optimise_params(eval, x0, centre, dof, maxit, s, subset)?;
    let quiet = Log::default();
    let log = s.log.as_deref().unwrap_or(&quiet);
    let n = if subset.is_some() { 12 } else { dof };
    Ok((param::compose(s.rep, &x, n, centre, log), c))
}

/// [`optimise`] from the parameters `x0` themselves (the search's grid points and minima: FSL
/// starts there from the angles it sampled, not from a decomposition of their matrix, observed):
/// returns the optimised parameters (the first `dof`, or all twelve with `subset`) and the cost.
pub fn optimise_params(
    eval: &dyn Fn(&Mat4) -> f32,
    x0: Params,
    centre: [f64; 3],
    dof: usize,
    maxit: usize,
    s: &Settings,
    subset: Option<&Subset>,
) -> Result<(Params, f32), String> {
    let quiet = Log::default();
    let log = s.log.as_deref().unwrap_or(&quiet);
    let tol: Vec<f32> = (0..12)
        .map(|i| {
            s.tolerance(i)
                .ok_or_else(|| "Mat::operator(): index out of bounds".to_owned())
        })
        .collect::<Result<_, _>>()?;
    match subset {
        None => {
            let compose = |v: &[f64]| {
                let mut x = x0;
                x[..dof].copy_from_slice(v);
                param::compose(s.rep, &x, dof, centre, log)
            };
            let cost = |v: &[f64]| eval(&compose(v));
            let (v, c) = minimise(&cost, x0[..dof].to_vec(), &tol[..dof], 12, maxit, s);
            let mut x = x0;
            x[..dof].copy_from_slice(&v);
            Ok((x, c))
        }
        Some(p) => {
            let mut x0d = [0.0; 12];
            x0d.copy_from_slice(&x0);
            let compose = |y: &[f64]| {
                let d = p.expand(y);
                let mut x = [0.0; 12];
                for i in 0..12 {
                    x[i] = d[i] + x0d[i];
                }
                param::compose(s.rep, &x, 12, centre, log)
            };
            let mut tplus = [0.0; 12];
            let mut zero = [0.0; 12];
            for i in 0..12 {
                tplus[i] = (f64::from(tol[i]) + x0d[i]) - x0d[i];
                // x0 - x0 as the spec writes it: 0, or NaN for a parameter that is not finite
                zero[i] = if x0d[i].is_finite() { 0.0 } else { f64::NAN };
            }
            let a = p.reduce(&tplus);
            let b = p.reduce(&zero);
            let ty: Vec<f32> = a.iter().zip(&b).map(|(x, y)| (x - y) as f32).collect();
            let y0 = p.reduce(&zero);
            let cost = |y: &[f64]| eval(&compose(y));
            let (y, c) = minimise(&cost, y0, &ty, ty.len(), maxit, s);
            let d = p.expand(&y);
            let mut x = [0.0; 12];
            for i in 0..12 {
                x[i] = d[i] + x0d[i];
            }
            Ok((x, c))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_mri::hmc::rigid;

    #[test]
    fn closed_form_pinv() {
        let mut scale = [0.0; 12];
        scale[6] = 1.0;
        scale[7] = 1.0;
        scale[8] = 1.0;
        let mut tx = [0.0; 12];
        tx[3] = 1.0;
        let s = Subset::new(&[scale, tx]).unwrap();
        assert_eq!(s.pinv[0][6], f64::from_bits(0x3FD5_5555_5555_5557));
        assert_eq!(s.pinv[1][3], 1.0);
    }

    #[test]
    fn general_pinv_inverts() {
        let mut a = [0.0; 12];
        a[2] = 1.0;
        let mut b = [0.0; 12];
        b[3] = 1.0;
        b[4] = 0.5;
        let s = Subset::new(&[a, b]).unwrap();
        // pinv·P = I
        for i in 0..2 {
            for j in 0..2 {
                let v: f64 = (0..12).map(|r| s.pinv[i][r] * s.mask[r][j]).sum();
                assert!((v - if i == j { 1.0 } else { 0.0 }).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn a_quadratic_bowl_is_minimised() {
        let target = [0.03, -0.02, 0.01, 1.5, -2.0, 0.7];
        let eval = |m: &Mat4| {
            let p = rigid::decompose(m, [0.0; 3]);
            let mut s = 0.0f64;
            for k in 0..6 {
                let w = if k < 3 { 100.0 } else { 1.0 };
                s += w * (p[k] - target[k]).powi(2);
            }
            s as f32 + 1.0
        };
        let s = Settings::default();
        let (m, c) = optimise(&eval, &rigid::IDENTITY, [0.0; 3], 6, 8, &s, None).unwrap();
        let p = rigid::decompose(&m, [0.0; 3]);
        assert!(c < 1.01, "{c}");
        for k in 3..6 {
            assert!((p[k] - target[k]).abs() < 0.3, "{k} {p:?}");
        }
    }
}
