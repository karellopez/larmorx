// SPDX-License-Identifier: Apache-2.0 AND CC-BY-4.0
// Translated to Rust from AFNI 26.3.05 src/mri_lsqfit.c (check_ref_vectors, startup_lsqfit,
// delayed_lsqfit, mri_startup_lsqfit, mri_delayed_lsqfit). Major portions Copyright (C) 1994-2000
// Medical College of Wisconsin (AFNI, Robert W. Cox et al.), released by AFNI 26.3.05 under the
// Creative Commons Attribution 4.0 International licence (CC BY 4.0).
// CC BY 4.0: https://creativecommons.org/licenses/by/4.0/ (the material is provided as is, without
// warranties of any kind); source: https://github.com/afni/afni, tag AFNI_26.3.05.
// AFNI asks that work based on it cite its authors (LICENSE.txt).
// Rust translation Copyright 2026 Karel Lopez Vilaret, licensed under the Apache License 2.0;
// modified from the original: translated to Rust, with the changes PROVENANCE.md lists.
//! AFNI's weighted linear least squares of an image on a few reference images, set up once
//! (the normal equations' Cholesky factor, [`startup`]) and solved for each image
//! ([`Startup::solve`]).
//!
//! The normal equations are summed in double, each product of floats first rounded to float
//! (`ref[i][k]·ref[j][k]·wt[k]`, all `float`); the references are then multiplied by the
//! weights in place, and each image's right-hand side is the sum, in double, of the float
//! products `ref_w[i][k]·data[k]`.
//!
//! **Failures.** A reference that is zero everywhere ("lsqfit :: n vectors are all 0 for data
//! fitting") or a factorisation that meets a non-positive pivot ("Choleski factorization
//! failure in startup_lsqfit [row=r sum=s]") make AFNI's routines return nothing, which
//! 3dvolreg then dereferences (a crash: AFNI-89). They are [`LsqError`]s here, with AFNI's
//! messages.

/// Why the least squares could not be set up or solved, with the messages AFNI prints
/// (`ERROR_message`s, in order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LsqError {
    pub messages: Vec<String>,
}

/// `check_ref_vectors(veclen, nref, ref, "lsqfit")`: the number of references that are zero
/// everywhere, with AFNI's message when there are any.
fn check_ref_vectors(refs: &[Vec<f32>], messages: &mut Vec<String>) -> usize {
    let nbad = refs.iter().filter(|r| r.iter().all(|&v| v == 0.0)).count();
    if nbad > 0 {
        messages.push(format!(
            "lsqfit :: {nbad} vector{} are all 0 for data fitting",
            if nbad > 1 { "s" } else { "" }
        ));
    }
    nbad
}

/// The set-up least squares: the references (multiplied by the weights when there are any)
/// and the lower Cholesky factor of the normal equations (`cc`, row-major, `nref × nref`).
#[derive(Debug, Clone)]
pub struct Startup {
    pub refs: Vec<Vec<f32>>,
    cc: Vec<f64>,
    nref: usize,
}

/// `mri_startup_lsqfit(refim, wtim)`: consumes the references, which come back weighted.
#[allow(clippy::needless_range_loop)] // AFNI's index loops, as written
pub fn startup(mut refs: Vec<Vec<f32>>, wt: Option<&[f32]>) -> Result<Startup, LsqError> {
    let nref = refs.len();
    let veclen = refs.first().map_or(0, Vec::len);
    let mut messages = Vec::new();
    let fail = |mut messages: Vec<String>| {
        messages.push("mri_startup_lsqfit: bad call to startup_lsqfit!\x07\n".to_owned());
        LsqError { messages }
    };
    if nref < 1 || veclen < nref {
        messages.push(format!(
            "startup_lsqfit: nref={nref} veclen={veclen} ref=(pointer)"
        ));
        return Err(fail(messages));
    }
    if check_ref_vectors(&refs, &mut messages) > 0 {
        return Err(fail(messages));
    }
    let at = |i: usize, j: usize| i + j * nref;
    let mut cc = vec![0.0f64; nref * nref];
    for jj in 0..nref {
        for ii in 0..=jj {
            let mut sum = 0.0f64;
            match wt {
                Some(w) => {
                    for kk in 0..veclen {
                        sum += f64::from(refs[ii][kk] * refs[jj][kk] * w[kk]);
                    }
                }
                None => {
                    for kk in 0..veclen {
                        sum += f64::from(refs[ii][kk] * refs[jj][kk]);
                    }
                }
            }
            cc[at(ii, jj)] = sum;
            cc[at(jj, ii)] = sum;
        }
    }
    // Choleski decompose cc.
    for ii in 0..nref {
        for jj in 0..ii {
            let mut sum = cc[at(ii, jj)];
            for kk in 0..jj {
                sum -= cc[at(ii, kk)] * cc[at(jj, kk)];
            }
            cc[at(ii, jj)] = sum / cc[at(jj, jj)];
        }
        let mut sum = cc[at(ii, ii)];
        for kk in 0..ii {
            sum -= cc[at(ii, kk)] * cc[at(ii, kk)];
        }
        if sum <= 0.0 {
            messages.push(format!(
                "Choleski factorization failure in startup_lsqfit [row={ii} sum={}]",
                crate::cnum::fmt_g(sum)
            ));
            return Err(fail(messages));
        }
        cc[at(ii, ii)] = sum.sqrt();
    }
    if let Some(w) = wt {
        for r in &mut refs {
            for (x, &wk) in r.iter_mut().zip(w) {
                *x *= wk;
            }
        }
    }
    Ok(Startup { refs, cc, nref })
}

impl Startup {
    /// `mri_delayed_lsqfit(fitim, refim, cc)`: the coefficients of `data` (float, from the
    /// double solution). AFNI checks the (weighted) references again first.
    pub fn solve(&self, data: &[f32]) -> Result<Vec<f32>, LsqError> {
        let nref = self.nref;
        let mut messages = Vec::new();
        if check_ref_vectors(&self.refs, &mut messages) > 0 {
            return Err(LsqError { messages });
        }
        let at = |i: usize, j: usize| i + j * nref;
        let cc = &self.cc;
        let mut rr = vec![0.0f64; nref];
        for (ii, r) in rr.iter_mut().enumerate() {
            let mut sum = 0.0f64;
            for (&a, &b) in self.refs[ii].iter().zip(data) {
                sum += f64::from(a * b);
            }
            *r = sum;
        }
        // Forward solve.
        for ii in 0..nref {
            let mut sum = rr[ii];
            for jj in 0..ii {
                sum -= cc[at(ii, jj)] * rr[jj];
            }
            rr[ii] = sum / cc[at(ii, ii)];
        }
        // Backward solve.
        for ii in (0..nref).rev() {
            let mut sum = rr[ii];
            for jj in ii + 1..nref {
                sum -= cc[at(jj, ii)] * rr[jj];
            }
            rr[ii] = sum / cc[at(ii, ii)];
        }
        Ok(rr.iter().map(|&x| x as f32).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_a_combination() {
        let n = 50;
        let r0: Vec<f32> = (0..n).map(|i| 1.0 + (i % 7) as f32).collect();
        let r1: Vec<f32> = (0..n).map(|i| ((i * 3) % 11) as f32 - 5.0).collect();
        let data: Vec<f32> = (0..n).map(|i| 2.0 * r0[i] - 0.5 * r1[i]).collect();
        let s = startup(vec![r0, r1], None).unwrap();
        let fit = s.solve(&data).unwrap();
        assert!(
            (fit[0] - 2.0).abs() < 1e-5 && (fit[1] + 0.5).abs() < 1e-5,
            "{fit:?}"
        );
        let e = startup(vec![vec![1.0; 4], vec![0.0; 4]], None).unwrap_err();
        assert_eq!(
            e.messages[0],
            "lsqfit :: 1 vector are all 0 for data fitting"
        );
        let e = startup(vec![vec![1.0; 4], vec![2.0; 4]], None).unwrap_err();
        assert!(
            e.messages[0].starts_with("Choleski factorization failure"),
            "{e:?}"
        );
    }
}
