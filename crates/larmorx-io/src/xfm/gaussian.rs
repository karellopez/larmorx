// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-MNI
//! volume_io's `Geometry/gaussian.c`: scaled maximal-pivoting Gaussian elimination and the
//! matrix inverse built on it, in its order of operations (every product and quotient in double
//! precision, in the order the C code writes them).

/// `scaled_maximal_pivoting_gaussian_elimination(n, row, a, n_values, solution)`: `a` (n x n)
/// and `solution` (n x n_values) are eliminated in place; `row` receives the pivot order. `false`
/// when a row is all zeros or a pivot is zero.
fn elimination(a: &mut [Vec<f64>], row: &mut [usize], solution: &mut [Vec<f64>]) -> bool {
    let n = a.len();
    let n_values = solution.first().map_or(0, Vec::len);
    for (i, r) in row.iter_mut().enumerate() {
        *r = i;
    }
    let mut s = vec![0.0f64; n];
    for i in 0..n {
        s[i] = a[i][0].abs();
        for j in 1..n {
            if a[i][j].abs() > s[i] {
                s[i] = a[i][j].abs();
            }
        }
        if s[i] == 0.0 {
            return false;
        }
    }
    let mut success = true;
    for i in 0..n.saturating_sub(1) {
        let mut p = i;
        let mut best_val = (a[row[i]][i] / s[row[i]]).abs();
        for j in i + 1..n {
            let val = (a[row[j]][i] / s[row[j]]).abs();
            if val > best_val {
                best_val = val;
                p = j;
            }
        }
        if a[row[p]][i] == 0.0 {
            success = false;
            break;
        }
        if i != p {
            row.swap(i, p);
        }
        for j in i + 1..n {
            if a[row[i]][i] == 0.0 {
                success = false;
                break;
            }
            let m = a[row[j]][i] / a[row[i]][i];
            for k in i + 1..n {
                let v = m * a[row[i]][k];
                a[row[j]][k] -= v;
            }
            for v in 0..n_values {
                let t = m * solution[row[i]][v];
                solution[row[j]][v] -= t;
            }
        }
        if !success {
            break;
        }
    }
    if success && a[row[n - 1]][n - 1] == 0.0 {
        success = false;
    }
    if success {
        for i in (0..n).rev() {
            for j in i + 1..n {
                let scale_factor = a[row[i]][j];
                for v in 0..n_values {
                    let t = scale_factor * solution[row[j]][v];
                    solution[row[i]][v] -= t;
                }
            }
            for v in 0..n_values {
                solution[row[i]][v] /= a[row[i]][i];
            }
        }
    }
    success
}

/// `scaled_maximal_pivoting_gaussian_elimination_real`: solves `coefs × x = values[v]` for each
/// of the `n_values` right-hand sides `values[v]` (length n), in place.
fn elimination_real(coefs: &[Vec<f64>], values: &mut [Vec<f64>]) -> bool {
    let n = coefs.len();
    let n_values = values.len();
    let mut row = vec![0usize; n];
    let mut a: Vec<Vec<f64>> = coefs.to_vec();
    let mut solution = vec![vec![0.0; n_values]; n];
    for i in 0..n {
        for v in 0..n_values {
            solution[i][v] = values[v][i];
        }
    }
    let ok = elimination(&mut a, &mut row, &mut solution);
    if ok {
        for i in 0..n {
            for v in 0..n_values {
                values[v][i] = solution[row[i]][v];
            }
        }
    }
    ok
}

/// `invert_square_matrix`: the inverse of an n x n matrix, or `None` (volume_io's `FALSE`).
pub fn invert_square_matrix(matrix: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = matrix.len();
    let mut inverse = vec![vec![0.0; n]; n];
    for (i, r) in inverse.iter_mut().enumerate() {
        r[i] = 1.0;
    }
    if !elimination_real(matrix, &mut inverse) {
        return None;
    }
    for i in 0..n.saturating_sub(1) {
        for j in i + 1..n {
            let tmp = inverse[i][j];
            inverse[i][j] = inverse[j][i];
            inverse[j][i] = tmp;
        }
    }
    Some(inverse)
}
