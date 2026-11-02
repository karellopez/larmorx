// SPDX-License-Identifier: Apache-2.0
//! The fits of FLIRT's other programs (`specs/flirt.md` §15.5, §15.6, §15.8): `midtrans`'s
//! transform to the middle of several transforms, `pointflirt`'s least-squares affine matrix
//! from point pairs and `aff2rigid`'s rigid approximation of a standard-space matrix.
//!
//! Written from the spec and black-box runs of FSL 6.0.7.23's programs (the findings in
//! `docs/findings/fsl-flirt-original.md`, "FLIRT's other programs"), never from FSL's source.
//! The singular value decompositions are `larmorx_core`'s (an accurate SVD: the spec's
//! thresholds, 1e-9 in every element of the matrices, §13, not LAPACK's last bits).

use larmorx_core::vnl_svd::vnl_svd;

use super::mat::{self, Mat4};

/// A 3 × 3 matrix, rows.
pub type M3 = [[f64; 3]; 3];
/// A 3-vector.
pub type V3 = [f64; 3];

const ZERO3: M3 = [[0.0; 3]; 3];

fn mul3(a: &M3, b: &M3) -> M3 {
    let mut c = ZERO3;
    for i in 0..3 {
        for j in 0..3 {
            c[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    c
}

fn mv(a: &M3, x: &V3) -> V3 {
    [0, 1, 2].map(|i| a[i][0] * x[0] + a[i][1] * x[1] + a[i][2] * x[2])
}

fn tr(a: &M3) -> M3 {
    [0, 1, 2].map(|i| [0, 1, 2].map(|j| a[j][i]))
}

fn outer(a: &V3, b: &V3) -> M3 {
    [0, 1, 2].map(|i| [0, 1, 2].map(|j| a[i] * b[j]))
}

fn add(a: &M3, b: &M3) -> M3 {
    [0, 1, 2].map(|i| [0, 1, 2].map(|j| a[i][j] + b[i][j]))
}

fn sub(a: &M3, b: &M3) -> M3 {
    [0, 1, 2].map(|i| [0, 1, 2].map(|j| a[i][j] - b[i][j]))
}

fn scale(a: &M3, s: f64) -> M3 {
    a.map(|r| r.map(|v| v * s))
}

fn addv(a: &V3, b: &V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn subv(a: &V3, b: &V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The orthogonal factor `V·Uᵀ` of the SVD `m = U·D·Vᵀ` (so `m`'s polar factor transposed);
/// `None` when `m` is not finite or the decomposition fails.
fn v_ut(m: &M3) -> Option<M3> {
    if m.iter().flatten().any(|v| !v.is_finite()) {
        return None;
    }
    let flat: Vec<f64> = m.iter().flatten().copied().collect();
    let (u, _, v) = vnl_svd(&flat, 3, 3)?;
    // u and v column-major: u[i + 3k] = U(i, k)
    Some([0, 1, 2].map(|i| [0, 1, 2].map(|j| (0..3).map(|k| v[i + 3 * k] * u[j + 3 * k]).sum())))
}

/// The steps `midtrans -v` and `--debug` print, in order (§15.5).
pub enum Step<'a> {
    /// `--debug`: each transform's 3 × 3 part (1-based).
    P(usize, &'a M3),
    /// `--debug`: `avg(P_n·(r·I + x_n·x_nᵀ))`.
    PxxAv(&'a M3),
    A(&'a M3),
    S(&'a V3),
    Stage2,
    /// One transform's rotation and offset (0-based).
    Subject(usize, &'a M3, &'a V3),
    Stage3,
    M(&'a M3),
    W(&'a V3),
    Final,
}

/// How `midtrans`'s computation stops (§15.5).
#[derive(Debug, PartialEq)]
pub enum MidError {
    /// A decomposition of a matrix that is not finite (FSL's fails; refused, rule B).
    NotFinite,
    /// `AAᵀ` is singular (`inv(): matrix is singular`, FSL-78).
    Singular,
}

/// `midtrans`'s transform from the template space of the 4 × 4 transforms `ts` to their
/// middle, about the centre `o` (§15.5), with `x_n` the first three components of
/// `T_n⁻¹·(o, 1)` (given: `xs`). `step` receives what `-v` and `--debug` print.
pub fn midtrans(ts: &[Mat4], xs: &[V3], step: &mut dyn FnMut(Step<'_>)) -> Result<Mat4, MidError> {
    let n = ts.len();
    let nf = f64::from(n as f32);
    let r = f64::from(80.0f32 * 80.0 / 5.0);
    let ps: Vec<M3> = ts
        .iter()
        .map(|t| [0, 1, 2].map(|i| [t[i][0], t[i][1], t[i][2]]))
        .collect();
    let tv: Vec<V3> = ts.iter().map(|t| [t[0][3], t[1][3], t[2][3]]).collect();
    let avg_m = |f: &dyn Fn(usize) -> M3| -> M3 {
        let mut s = ZERO3;
        for k in 0..n {
            s = add(&s, &f(k));
        }
        s.map(|r| r.map(|v| v / nf))
    };
    let avg_v = |f: &dyn Fn(usize) -> V3| -> V3 {
        let mut s = [0.0; 3];
        for k in 0..n {
            s = addv(&s, &f(k));
        }
        s.map(|v| v / nf)
    };
    // stage 1
    let rid: M3 = [[r, 0.0, 0.0], [0.0, r, 0.0], [0.0, 0.0, r]];
    for (k, p) in ps.iter().enumerate() {
        step(Step::P(k + 1, p));
    }
    let pxx_av = avg_m(&|k| mul3(&ps[k], &add(&rid, &outer(&xs[k], &xs[k]))));
    step(Step::PxxAv(&pxx_av));
    let tx_av = avg_m(&|k| outer(&tv[k], &xs[k]));
    let px_av = avg_v(&|k| mv(&ps[k], &xs[k]));
    let x_av = avg_v(&|k| xs[k]);
    let t_av = avg_v(&|k| tv[k]);
    let m1 = sub(
        &sub(&add(&pxx_av, &tx_av), &outer(&px_av, &x_av)),
        &outer(&t_av, &x_av),
    );
    let a = v_ut(&m1).ok_or(MidError::NotFinite)?;
    let s = subv(&x_av, &mv(&a, &addv(&px_av, &t_av)));
    step(Step::A(&a));
    step(Step::S(&s));
    // stage 2
    step(Step::Stage2);
    let pp: Vec<M3> = ps.iter().map(|p| mul3(&a, p)).collect();
    let tp: Vec<V3> = tv.iter().map(|t| addv(&mv(&a, t), &s)).collect();
    let mut rs = Vec::with_capacity(n);
    let mut us = Vec::with_capacity(n);
    for k in 0..n {
        let rk = v_ut(&tr(&pp[k])).ok_or(MidError::NotFinite)?;
        let uk = addv(&mv(&sub(&pp[k], &rk), &xs[k]), &tp[k]);
        step(Step::Subject(k, &rk, &uk));
        rs.push(rk);
        us.push(uk);
    }
    // stage 3
    step(Step::Stage3);
    let u_av = avg_v(&|k| us[k]);
    let tp_av = avg_v(&|k| tp[k]);
    let tt_av = avg_m(&|k| outer(&tp[k], &tp[k]));
    let ut_av = avg_m(&|k| outer(&us[k], &tp[k]));
    let ppx: Vec<V3> = (0..n).map(|k| mv(&pp[k], &xs[k])).collect();
    let rx: Vec<V3> = (0..n).map(|k| mv(&rs[k], &xs[k])).collect();
    let ppx_av = avg_v(&|k| ppx[k]);
    let rx_av = avg_v(&|k| rx[k]);
    let ppxt_av = avg_m(&|k| outer(&ppx[k], &tp[k]));
    let rxt_av = avg_m(&|k| outer(&rx[k], &tp[k]));
    let uxp_av = avg_m(&|k| mul3(&outer(&us[k], &xs[k]), &tr(&pp[k])));
    let ppt_av = avg_m(&|k| mul3(&pp[k], &tr(&pp[k])));
    let rpt_av = avg_m(&|k| mul3(&rs[k], &tr(&pp[k])));
    let mut sp = ZERO3;
    let mut sr = ZERO3;
    for k in 0..n {
        sp = add(&sp, &mul3(&outer(&ppx[k], &xs[k]), &tr(&pp[k])));
        sr = add(&sr, &mul3(&outer(&rx[k], &xs[k]), &tr(&pp[k])));
    }
    let nd = n as f64;
    let pxx = add(&scale(&ppt_av, r), &sp.map(|r| r.map(|v| v / nd)));
    let rxx = add(&scale(&rpt_av, r), &sr.map(|r| r.map(|v| v / nd)));
    let mut aat = sub(&pxx, &outer(&ppx_av, &ppx_av));
    aat = add(&aat, &ppxt_av);
    aat = add(&aat, &tr(&ppxt_av));
    aat = sub(&aat, &outer(&tp_av, &ppx_av));
    aat = sub(&aat, &outer(&ppx_av, &tp_av));
    aat = add(&aat, &tt_av);
    aat = sub(&aat, &outer(&tp_av, &tp_av));
    let mut bat = sub(&rxx, &outer(&rx_av, &ppx_av));
    bat = add(&bat, &rxt_av);
    bat = sub(&bat, &outer(&rx_av, &tp_av));
    bat = sub(&bat, &uxp_av);
    bat = add(&bat, &outer(&u_av, &ppx_av));
    bat = sub(&bat, &ut_av);
    bat = add(&bat, &outer(&u_av, &tp_av));
    let bat = scale(&bat, -1.0);
    let inv = inverse3(&aat).ok_or(MidError::Singular)?;
    let m = scale(&mul3(&bat, &inv), -1.0);
    let mut w = mv(&m, &ppx_av).map(|v| -v);
    w = subv(&w, &mv(&m, &tp_av));
    w = addv(&addv(&w, &rx_av), &u_av);
    step(Step::M(&m));
    step(Step::W(&w));
    step(Step::Final);
    let ma = mul3(&m, &a);
    let ms = mv(&m, &s);
    let mut out = mat::IDENTITY;
    for i in 0..3 {
        out[i][..3].copy_from_slice(&ma[i]);
        out[i][3] = w[i] + ms[i];
    }
    Ok(out)
}

/// The inverse of a 3 × 3 matrix by LU with partial pivoting; `None` for an exactly singular
/// one.
fn inverse3(a: &M3) -> Option<M3> {
    let mut m = *a;
    let mut perm = [0usize, 1, 2];
    for k in 0..3 {
        let p = (k..3)
            .max_by(|&i, &j| m[i][k].abs().total_cmp(&m[j][k].abs()).then(j.cmp(&i)))
            .unwrap_or(k);
        if m[p][k] == 0.0 || !m[p][k].is_finite() {
            return None;
        }
        m.swap(p, k);
        perm.swap(p, k);
        for i in k + 1..3 {
            let f = m[i][k] / m[k][k];
            m[i][k] = f;
            for j in k + 1..3 {
                m[i][j] -= f * m[k][j];
            }
        }
    }
    let mut inv = ZERO3;
    for c in 0..3 {
        let mut y = [0.0; 3];
        for i in 0..3 {
            let mut s = if perm[i] == c { 1.0 } else { 0.0 };
            for j in 0..i {
                s -= m[i][j] * y[j];
            }
            y[i] = s;
        }
        for i in (0..3).rev() {
            let mut s = y[i];
            for j in i + 1..3 {
                s -= m[i][j] * inv[j][c];
            }
            inv[i][c] = s / m[i][i];
        }
    }
    Some(inv)
}

/// The determinant of a square matrix by LU with partial pivoting (§15.5: `midtrans` reads a
/// transform whose determinant is at least 1e-6 in magnitude); 1 for an empty matrix. The 4 × 4
/// case is [`super::programs::determinant`]'s.
pub fn determinant(rows: &[Vec<f64>]) -> f64 {
    let n = rows.len();
    if n == 4
        && let Some(m) = mat::mat4_of(rows)
    {
        return super::programs::determinant(&m);
    }
    let mut m: Vec<Vec<f64>> = rows.to_vec();
    let mut det = 1.0;
    for k in 0..n {
        let p = (k..n)
            .max_by(|&i, &j| m[i][k].abs().total_cmp(&m[j][k].abs()).then(j.cmp(&i)))
            .unwrap_or(k);
        if m[p][k] == 0.0 {
            return 0.0 * det;
        }
        if p != k {
            m.swap(p, k);
            det = -det;
        }
        det *= m[k][k];
        for i in k + 1..n {
            let f = m[i][k] / m[k][k];
            for j in k + 1..n {
                m[i][j] -= f * m[k][j];
            }
        }
    }
    det
}

/// What `pointflirt`'s solve found (§15.6).
pub struct PointFit {
    pub matrix: Mat4,
    /// The singular values set to 0 (`WARNING: nullspace of dimension <k>`).
    pub nullspace: usize,
}

/// `pointflirt`'s affine matrix mapping the points `x` onto `y` (3 × n each, points as
/// columns) in the least-squares sense (§15.6): the 12 × 12 normal equations solved by their
/// SVD, singular values up to 1e-8 dropped. `None` when a coordinate is not finite (refused,
/// rule B: FSL's decomposition returns garbage or fails).
pub fn pointflirt(x: &[Vec<f64>; 3], y: &[Vec<f64>; 3]) -> Option<PointFit> {
    let n = x[0].len();
    if x.iter().chain(y.iter()).flatten().any(|v| !v.is_finite()) {
        return None;
    }
    let nf = n as f64;
    let mut s = ZERO3;
    let mut q = ZERO3;
    for i in 0..3 {
        for j in 0..3 {
            s[i][j] = (0..n).map(|k| x[i][k] * x[j][k]).sum();
            q[i][j] = (0..n).map(|k| x[i][k] * y[j][k]).sum();
        }
    }
    let mean = |v: &Vec<f64>| -> f64 { v.iter().map(|a| a / nf).sum() };
    let xb = [mean(&x[0]), mean(&x[1]), mean(&x[2])];
    let yb = [mean(&y[0]), mean(&y[1]), mean(&y[2])];
    let mut b = vec![0.0f64; 144];
    for blk in 0..3 {
        for i in 0..3 {
            for j in 0..3 {
                b[(3 * blk + i) * 12 + 3 * blk + j] = s[i][j];
            }
            // W·n at rows 1-9, columns 10-12; Wᵀ at rows 10-12, columns 1-9
            b[(3 * blk + i) * 12 + 9 + blk] = xb[i] * nf;
            b[(9 + blk) * 12 + 3 * blk + i] = xb[i];
        }
        b[(9 + blk) * 12 + 9 + blk] = 1.0;
    }
    let mut rhs = [0.0f64; 12];
    for c in 0..3 {
        for i in 0..3 {
            rhs[3 * c + i] = q[i][c];
        }
        rhs[9 + c] = yb[c];
    }
    let (u, d, v) = vnl_svd(&b, 12, 12)?;
    let mut nullspace = 0;
    let dinv: Vec<f64> = d
        .iter()
        .map(|&dk| {
            if dk > 1e-8 {
                1.0 / dk
            } else {
                nullspace += 1;
                0.0
            }
        })
        .collect();
    // p = V·(D·(Uᵀ·q))
    let utq: Vec<f64> = (0..12)
        .map(|k| (0..12).map(|i| u[i + 12 * k] * rhs[i]).sum::<f64>() * dinv[k])
        .collect();
    let p: Vec<f64> = (0..12)
        .map(|i| (0..12).map(|k| v[i + 12 * k] * utq[k]).sum())
        .collect();
    let matrix = [
        [p[0], p[1], p[2], p[9]],
        [p[3], p[4], p[5], p[10]],
        [p[6], p[7], p[8], p[11]],
        [0.0, 0.0, 0.0, 1.0],
    ];
    Some(PointFit { matrix, nullspace })
}

/// `aff2rigid`'s rigid matrix (§15.8): the input-to-standard affine `a`'s approximation that
/// keeps the anterior commissure, the AC-PC line and the mid-sagittal plane. `None` for a
/// singular `a`.
pub fn aff2rigid(a: &Mat4) -> Option<Mat4> {
    type V4 = [f64; 4];
    let ainv = mat::inverse(a)?;
    let dot = |p: &V4, q: &V4| p[0] * q[0] + p[1] * q[1] + p[2] * q[2] + p[3] * q[3];
    let norm = |p: &V4| -> V4 {
        let l = dot(p, p).sqrt();
        p.map(|v| v / l)
    };
    let cross = |p: &V4, q: &V4| -> V4 {
        [
            p[1] * q[2] - p[2] * q[1],
            p[2] * q[0] - p[0] * q[2],
            p[0] * q[1] - p[1] * q[0],
            0.0,
        ]
    };
    let minus = |p: &V4, q: &V4| -> V4 { [0, 1, 2, 3].map(|i| p[i] - q[i]) };
    let orth = |p: &V4, q: &V4| -> V4 {
        let d = dot(p, q);
        norm(&[0, 1, 2, 3].map(|i| p[i] - d * q[i]))
    };
    let x1: V4 = [91.0, 129.0, 67.0, 1.0];
    let x2: V4 = [91.0, 100.0, 70.0, 1.0];
    let x3: V4 = [91.0, 129.0, 117.0, 1.0];
    let v2 = norm(&minus(&x2, &x1));
    let v3 = orth(&minus(&x3, &x1), &v2);
    let v4 = cross(&v2, &v3);
    let w2 = norm(&mat::apply(&ainv, v2));
    let w3 = orth(&mat::apply(&ainv, v3), &w2);
    let w4 = cross(&w2, &w3);
    let mut r1 = mat::IDENTITY;
    let mut r2 = mat::IDENTITY;
    for i in 0..4 {
        r1[i][0] = w2[i];
        r1[i][1] = w3[i];
        r1[i][2] = w4[i];
    }
    r2[0] = v2;
    r2[1] = v3;
    r2[2] = v4;
    let mut r = mat::mul(&mat::transpose(&r2), &mat::transpose(&r1));
    let c = mat::apply(&r, mat::apply(&ainv, x1));
    for i in 0..3 {
        r[i][3] = x1[i] - c[i];
    }
    Some(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polar_and_inverse() {
        let r = v_ut(&[[0.0, 2.0, 0.0], [-3.0, 0.0, 0.0], [0.0, 0.0, 1.0]]).unwrap();
        // m = U·D·Vᵀ with m a scaled rotation: V·Uᵀ is the rotation's transpose
        let want = [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        for i in 0..3 {
            for j in 0..3 {
                assert!((r[i][j] - want[i][j]).abs() < 1e-14, "{r:?}");
            }
        }
        let a = [[2.0, 1.0, 0.0], [0.5, 3.0, -1.0], [0.0, 0.25, 4.0]];
        let p = mul3(&a, &inverse3(&a).unwrap());
        for (i, row) in p.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                assert!((v - f64::from(u8::from(i == j))).abs() < 1e-15);
            }
        }
        assert!(inverse3(&[[1.0, 2.0, 3.0], [2.0, 4.0, 6.0], [0.0, 0.0, 0.0]]).is_none());
        assert_eq!(determinant(&[]), 1.0);
        assert_eq!(determinant(&[vec![2.0, 1.0], vec![1.0, 3.0]]), 5.0);
    }

    #[test]
    fn pointflirt_recovers_an_affine_map() {
        let m = [
            [1.05, 0.02, -0.03, 2.0],
            [-0.01, 0.97, 0.04, -3.0],
            [0.02, -0.05, 1.1, 1.5],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let pts = [
            [10.0, 20.0, 30.0],
            [-12.0, 5.0, 40.0],
            [33.0, -7.0, 12.0],
            [0.0, 0.0, 0.0],
            [25.0, 25.0, -10.0],
        ];
        let x = [0, 1, 2].map(|i| pts.iter().map(|p| p[i]).collect::<Vec<_>>());
        let y = [0, 1, 2].map(|i| {
            pts.iter()
                .map(|p| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2] + m[i][3])
                .collect::<Vec<_>>()
        });
        let f = pointflirt(&x, &y).unwrap();
        assert_eq!(f.nullspace, 0);
        for i in 0..4 {
            for j in 0..4 {
                assert!((f.matrix[i][j] - m[i][j]).abs() < 1e-10, "{:?}", f.matrix);
            }
        }
    }

    #[test]
    fn aff2rigid_of_a_rigid_matrix_is_itself() {
        let (c, s) = (0.2f64.cos(), 0.2f64.sin());
        let m = [
            [c, -s, 0.0, 3.0],
            [s, c, 0.0, -2.0],
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let r = aff2rigid(&m).unwrap();
        for i in 0..4 {
            for j in 0..4 {
                assert!((r[i][j] - m[i][j]).abs() < 1e-12, "{r:?}");
            }
        }
    }
}
