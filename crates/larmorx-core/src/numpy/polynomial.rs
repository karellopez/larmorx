// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! numpy's Legendre basis polynomials as nipype's `regress_poly` evaluates them,
//! `Legendre.basis(deg)(x)` (`numpy/polynomial/_polybase.py`'s `basis` and `__call__`,
//! `polyutils.py`'s `mapdomain`/`mapparms`, `legendre.py`'s `legval`; numpy 2.5.4, 2.3.5 and
//! 2.2.6). Notice in `super`.
//!
//! `Legendre.basis(deg)` holds the coefficients `[0, ..., 0, 1]` (float64) on the domain and
//! window `[-1, 1]`; calling it maps `x` through `mapdomain`, `off + scl * x` with `off =
//! (1 · -1 - -1 · 1) / 2 = 0.0` and `scl = 2 / 2 = 1.0` (so `-0.0` becomes `+0.0`), then runs
//! `legval`'s Clenshaw recursion, element by element in `float64`.

use super::NpFloat;

/// The two forms of `legval`'s recursion, which give other bits from degree 3 on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LegvalForm {
    /// numpy 2.2: `c0 = c[-i] - (c1 * (nd - 1)) / nd`, `c1 = tmp + (c1 * x * (2 * nd - 1)) / nd`.
    V2_2,
    /// numpy 2.3 and later: `c0 = c[-i] - c1 * ((nd - 1) / nd)`, `c1 = tmp + c1 * x * ((2 * nd
    /// - 1) / nd)` (the integer quotients as Python floats). The default.
    #[default]
    V2_3,
}

impl LegvalForm {
    /// The form of numpy release `version` (`"2.2.6"`, `"2.5.4"`); numpy 1.x has 2.2's.
    pub fn from_version(version: &str) -> Option<LegvalForm> {
        let mut it = version.split('.');
        let major: u32 = it.next()?.parse().ok()?;
        let minor: u32 = it
            .next()?
            .trim_end_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .ok()?;
        Some(if major > 2 || (major == 2 && minor >= 3) {
            LegvalForm::V2_3
        } else {
            LegvalForm::V2_2
        })
    }
}

/// `numpy.polynomial.Legendre.basis(deg)(x)` of `float64` values: the Legendre polynomial of
/// degree `deg` at each `x`, computed as numpy computes it (module documentation; `deg = 0` is
/// `1 + 0 · x`, `deg = 1` is `0 + 1 · x`).
pub fn legendre_basis(deg: usize, x: &[f64], form: LegvalForm) -> Vec<f64> {
    x.iter().map(|&v| legendre_at(deg, v, form)).collect()
}

fn legendre_at(deg: usize, x: f64, form: LegvalForm) -> f64 {
    // mapdomain: off + scl * x
    let x = 0.0f64.add(1.0f64.mul(x));
    let len = deg + 1;
    let (c0, c1) = match len {
        1 => (1.0f64, 0.0f64),
        2 => (0.0, 1.0),
        _ => {
            let (mut c0, mut c1) = (0.0f64, 1.0f64); // c[-2], c[-1]
            let mut nd = len;
            for _ in 3..=len {
                let tmp = c0;
                nd -= 1;
                let ndf = nd as f64;
                // c[-i] is 0 for every i >= 3 (only the last coefficient is 1)
                match form {
                    LegvalForm::V2_3 => {
                        c0 = 0.0f64.sub(c1.mul((nd - 1) as f64 / ndf));
                        c1 = tmp.add(c1.mul(x).mul((2 * nd - 1) as f64 / ndf));
                    }
                    LegvalForm::V2_2 => {
                        c0 = 0.0f64.sub(c1.mul((nd - 1) as f64).div(ndf));
                        c1 = tmp.add(c1.mul(x).mul((2 * nd - 1) as f64).div(ndf));
                    }
                }
            }
            (c0, c1)
        }
    };
    c0.add(c1.mul(x))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_degrees_and_the_forms() {
        let x = [-1.0, -0.5, -0.0, 0.25, 1.0];
        assert_eq!(legendre_basis(0, &x, LegvalForm::V2_3), vec![1.0; 5]);
        let p1 = legendre_basis(1, &x, LegvalForm::V2_3);
        assert_eq!(p1[2].to_bits(), 0); // -0 mapped to +0
        assert_eq!(p1[1], -0.5);
        // P2 = 1.5 x^2 - 0.5, the same in both forms (x * 3 / 2 = x * 1.5 exactly)
        for f in [LegvalForm::V2_2, LegvalForm::V2_3] {
            let p2 = legendre_basis(2, &x, f);
            assert_eq!(p2, vec![1.0, -0.125, -0.5, -0.40625, 1.0]);
        }
        // from degree 3 on the forms may differ: (2/3) is not exact
        let xs: Vec<f64> = (0..200).map(|i| -1.0 + i as f64 / 99.5).collect();
        let a = legendre_basis(5, &xs, LegvalForm::V2_2);
        let b = legendre_basis(5, &xs, LegvalForm::V2_3);
        assert!(a.iter().zip(&b).any(|(p, q)| p != q));
        assert!(a.iter().zip(&b).all(|(p, q)| (p - q).abs() < 1e-14));
        assert_eq!(LegvalForm::from_version("2.2.6"), Some(LegvalForm::V2_2));
        assert_eq!(LegvalForm::from_version("2.5.4"), Some(LegvalForm::V2_3));
    }
}
