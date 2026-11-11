// SPDX-License-Identifier: Apache-2.0
//! The echo-count groups and the log-linear fit (`specs/t2smap.md` §7.1, §7.2, §7.4).
//!
//! tedana solves every voxel of a group as one least-squares problem (numpy's `lstsq`, an SVD of
//! the m × 2 design with N right-hand sides). The original solves each voxel alone in closed
//! form, as §16 allows: the design's columns are (1, −TE_e) with T_u rows per echo, so the
//! centred normal equations need only the per-echo sums s_e = Σ_t y_et of the observations:
//! O(n · T_u) per voxel, no batch, the same result at any thread count (§13.4: the closed form
//! writes the same float32 maps within §17.3's two units in the last place). The rank rule is
//! lstsq's: singular values below eps · max(m, 2) · s_max are dropped (equal echo times: the
//! minimum-norm solution of the rank-1 problem).

use larmorx_core::defects::{self, Notices};
use larmorx_core::math;
use larmorx_core::numpy::NpFloat;
use rayon::prelude::*;

use crate::input::Data;
use crate::log::{Log, PyError};

/// The echo-count groups of §7.1: (n, the voxels fitted with echoes 1..n), ascending.
pub fn groups(count: &[u8]) -> Vec<(usize, Vec<usize>)> {
    let mut present = [false; 256];
    for &c in count {
        present[c as usize] = true;
    }
    if present[1] {
        present[2] = true;
    }
    let mut out = Vec::new();
    for (n, _) in present.iter().enumerate().skip(2).filter(|(_, p)| **p) {
        let members: Vec<usize> = (0..count.len())
            .filter(|&v| {
                let c = count[v] as usize;
                if n == 2 { c == 1 || c == 2 } else { c == n }
            })
            .collect();
        out.push((n, members));
    }
    out
}

/// One group's design: n echoes, T_u rows per echo (§7.2).
struct Design {
    tu: usize,
    xbar: f64,
    /// −TE_e − x̄.
    c: Vec<f64>,
    /// −TE_e.
    u: Vec<f64>,
    sxx: f64,
    /// Whether lstsq keeps both singular values.
    rank2: bool,
    /// The leading eigenvector of XᵀX and its eigenvalue (the rank-1 solution).
    v1: (f64, f64),
    lam1: f64,
}

impl Design {
    fn new(tes: &[f64], n: usize, tu: usize) -> Design {
        let u: Vec<f64> = tes[..n].iter().map(|&t| -t).collect();
        let xbar = u.iter().sum::<f64>() / n as f64;
        let c: Vec<f64> = u.iter().map(|&x| x - xbar).collect();
        let tuf = tu as f64;
        let sxx = tuf * c.iter().map(|&x| x * x).sum::<f64>();
        let m = (n * tu) as f64;
        let su = tuf * u.iter().sum::<f64>();
        let suu = tuf * u.iter().map(|&x| x * x).sum::<f64>();
        let disc = ((m - suu) * (m - suu) + 4.0 * su * su).sqrt();
        let lam1 = 0.5 * (m + suu + disc);
        let lam2 = if lam1 > 0.0 { m * sxx / lam1 } else { 0.0 };
        let eps = f64::EPSILON * m.max(2.0);
        let rank2 = lam2 > 0.0 && lam2.sqrt() > eps * lam1.sqrt();
        let (a, b) = (lam1 - suu, su);
        let norm = (a * a + b * b).sqrt();
        let v1 = if norm > 0.0 {
            (a / norm, b / norm)
        } else {
            (1.0, 0.0)
        };
        Design {
            tu,
            xbar,
            c,
            u,
            sxx,
            rank2,
            v1,
            lam1,
        }
    }

    /// (β0, β1) from the per-echo sums of the observations.
    fn solve(&self, s: &[f64]) -> (f64, f64) {
        if self.tu == 0 {
            return (0.0, 0.0);
        }
        let n = self.c.len();
        let total: f64 = s.iter().sum();
        if self.rank2 {
            let ybar = total / (n * self.tu) as f64;
            let sxy: f64 = self.c.iter().zip(s).map(|(c, s)| c * s).sum();
            let b1 = sxy / self.sxx;
            (ybar - b1 * self.xbar, b1)
        } else {
            let uy: f64 = self.u.iter().zip(s).map(|(u, s)| u * s).sum();
            let p = (self.v1.0 * total + self.v1.1 * uy) / self.lam1;
            (p * self.v1.0, p * self.v1.1)
        }
    }
}

/// The full maps of the fit (§7.5): per voxel (and volume in `ts`) T2* (ms) and S0, the
/// log-linear S0 with the other exponential (for §15.2's GLIBC-07 count) and whether the
/// voxel's S0 is the log-linear one. Indexed `[v * cols + t]`; 0 outside the fit voxels.
#[derive(Clone, Debug)]
pub struct Maps {
    pub cols: usize,
    pub t2s: Vec<f64>,
    pub s0: Vec<f64>,
    /// exp(β0) with the other function (glibc's in the default, the correctly rounded one in
    /// `reproduce`); NaN where the S0 is not a log-linear one.
    pub s0_other: Vec<f64>,
}

/// The observation's logarithm (§13.3): correctly rounded by default, numpy's own float32
/// kernel in `reproduce` (NP-03).
fn obs(x: f32, reproduce: bool) -> f64 {
    let a = x.abs().add(1.0);
    let y = if reproduce {
        math::logf_numpy(a)
    } else {
        math::logf(a)
    };
    y as f64
}

/// The log-linear fit of every group (§7.2), `volumes` the used ones (`None`: all), or with
/// `ts` one fit per volume (§7.4).
pub fn loglin(
    data: &Data,
    count: &[u8],
    tes: &[f64],
    volumes: Option<&[usize]>,
    ts: bool,
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Result<Maps, PyError> {
    let reproduce = notices.reproduce();
    let cols = if ts { data.n_t } else { 1 };
    let n_vox = data.n_vox;
    let mut maps = Maps {
        cols,
        t2s: vec![0.0; n_vox * cols],
        s0: vec![0.0; n_vox * cols],
        s0_other: vec![f64::NAN; n_vox * cols],
    };
    let tu = if ts {
        1
    } else {
        volumes.map_or(data.n_t, <[usize]>::len)
    };
    let mut divide_by_zero = false;
    for (n, members) in groups(count) {
        if tes[..n].iter().any(|t| !t.is_finite()) {
            // TED-10 in `reproduce` (the default refuses an infinite echo time earlier)
            return Err(PyError::new(
                "numpy.linalg.LinAlgError",
                "SVD did not converge in Linear Least Squares",
            ));
        }
        let design = Design::new(tes, n, tu);
        // per member and column: the per-echo sums, whether an observation is infinite
        let fits: Vec<(f64, f64, bool)> = members
            .par_iter()
            .with_min_len(64)
            .flat_map_iter(|&v| {
                let design = &design;
                (0..cols).map(move |t| {
                    let mut s = [0.0f64; 256];
                    let mut inf = false;
                    for (e, se) in s.iter_mut().enumerate().take(n) {
                        let series = data.series(v, e);
                        let mut acc = 0.0f64;
                        if ts {
                            acc = obs(series[t], reproduce);
                        } else {
                            match volumes {
                                None => {
                                    for &x in series {
                                        acc += obs(x, reproduce);
                                    }
                                }
                                Some(vols) => {
                                    for &tt in vols {
                                        acc += obs(series[tt], reproduce);
                                    }
                                }
                            }
                        }
                        inf |= acc.is_infinite();
                        *se = acc;
                    }
                    let (b0, b1) = design.solve(&s[..n]);
                    (b0, b1, inf)
                })
            })
            .collect();
        let n_inf = fits.iter().filter(|f| f.2).count();
        // tedana's batch of a column (every volume's own in `ts`) is NaN when one of its
        // voxels has an infinite observation (TED-09)
        let mut poison = vec![false; cols];
        if reproduce {
            for (k, f) in fits.iter().enumerate() {
                poison[k % cols] |= f.2;
            }
        }
        if n_inf > 0 {
            let detail = if reproduce {
                format!(
                    "an infinite sample makes all {} voxels of the {n}-echo fit NaN",
                    members.len()
                )
            } else {
                format!(
                    "{n_inf} voxel(s) with an infinite sample fitted alone (NaN); tedana's \
                     {n}-echo fit makes all {} voxels of that count NaN",
                    members.len()
                )
            };
            notices.add(&defects::TED_09, detail);
        }
        for (j, &v) in members.iter().enumerate() {
            for t in 0..cols {
                let (mut b0, mut b1, inf) = fits[j * cols + t];
                if poison[t] || inf {
                    b0 = f64::NAN;
                    b1 = f64::NAN;
                }
                if b1 == 0.0 {
                    divide_by_zero = true;
                }
                let k = v * cols + t;
                maps.t2s[k] = 1.0f64.div(b1);
                let (ours, other) = if reproduce {
                    (math::exp_glibc(b0), math::exp(b0))
                } else {
                    (math::exp(b0), math::exp_glibc(b0))
                };
                maps.s0[k] = ours;
                maps.s0_other[k] = other;
            }
        }
    }
    if divide_by_zero {
        log.py_warning(
            "fit.rs:200",
            "RuntimeWarning",
            "divide by zero encountered in divide",
        );
    }
    Ok(maps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_add_two_for_one() {
        let g = groups(&[0, 1, 3, 3, 2, 0]);
        assert_eq!(g, vec![(2, vec![1, 4]), (3, vec![2, 3])]);
        assert_eq!(groups(&[0, 0]), vec![]);
    }

    #[test]
    fn closed_form_matches_a_line() {
        // y = 7 - 0.03 · (-TE) exactly: β = (7, -0.03)... with u = -TE: y = b0 + b1 u
        let tes = [12.0, 28.0, 44.0];
        let d = Design::new(&tes, 3, 4);
        assert!(d.rank2);
        let s: Vec<f64> = tes.iter().map(|&t| 4.0 * (7.0 - 0.03 * t)).collect();
        let (b0, b1) = d.solve(&s);
        assert!((b0 - 7.0).abs() < 1e-12 && (b1 - 0.03).abs() < 1e-14);
        // equal echo times: the minimum-norm solution ȳ · (1, u0) / (1 + u0²)
        let d = Design::new(&[20.0, 20.0], 2, 3);
        assert!(!d.rank2);
        let (b0, b1) = d.solve(&[3.0 * 5.0, 3.0 * 5.0]);
        let u0 = -20.0f64;
        assert!((b0 - 5.0 / (1.0 + u0 * u0)).abs() < 1e-15);
        assert!((b1 - 5.0 * u0 / (1.0 + u0 * u0)).abs() < 1e-13);
        // no row: zero
        assert_eq!(Design::new(&tes, 3, 0).solve(&[0.0; 3]), (0.0, 0.0));
    }
}
