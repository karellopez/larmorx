// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-flirt 2111.4 (avscale.cc). Copyright (C) 1999-2023
// University of Oxford (Mark Jenkinson). Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the original. Distributed under the FSL Licence (see LICENSE).
//! `avscale`: the decomposition of an affine matrix (rotations, translations about a volume's
//! centre of gravity, scales, skews), its determinant, and its two half transforms, as FSL
//! prints them, with its messages and exit statuses. The half transforms' double `cos`
//! (sqrtaff) is correctly rounded by default and glibc 2.35's with `reproduce` (GLIBC-07), named
//! in both modes where the two print different reports.

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_core::math::Libm;
use larmorx_nc_core::arma::{self, Mat};
use larmorx_nc_core::cnum;
use larmorx_nc_core::miscmaths::{self, INV_ERROR, MatrixFileError};
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volume::{self, FslError};

use crate::run::PREFIX;

/// How the program ends early.
enum End {
    Exit(u8),
    /// An exception `main` catches: `catch (Exception)` prints its `what()` (the runtime errors:
    /// Armadillo's and newimage's), `catch (...)` prints "Image error"; both throw it again, and
    /// `std::terminate` aborts.
    Throw {
        printed: String,
        type_name: &'static str,
        what: String,
    },
    /// A matrix size whose arithmetic Armadillo hands to BLAS or LAPACK beyond 4 x 4 (not ported).
    Unsupported(String),
}

fn runtime(what: &str) -> End {
    End::Throw {
        printed: String::new(),
        type_name: "std::runtime_error",
        what: what.to_owned(),
    }
}

impl From<FslError> for End {
    fn from(e: FslError) -> End {
        End::Throw {
            printed: e.printed_text(),
            type_name: e.type_name,
            what: e.what,
        }
    }
}

/// `std::cout`: fixed notation for good once a matrix is printed (armawrap's `doPrint`).
#[derive(Default)]
struct Cout {
    text: String,
    fixed: bool,
}

impl Cout {
    fn s(&mut self, t: &str) {
        self.text.push_str(t);
    }

    fn num(&mut self, v: f64) {
        let t = if self.fixed {
            cnum::fmt_f(v, 6)
        } else {
            cnum::stream(v)
        };
        self.text.push_str(&t);
    }

    fn mat(&mut self, m: &Mat) {
        self.fixed = true;
        for r in 0..m.rows() {
            for c in 0..m.cols() {
                self.text.push_str(&cnum::fmt_f(m.at(r, c), 6));
                self.text.push(' ');
            }
            self.text.push('\n');
        }
    }

    fn row(&mut self, v: &[f64]) {
        self.mat(&Mat::from_rows(1, v.len(), v));
    }
}

fn read_matrix(name: &str, err: &mut String) -> Result<Mat, End> {
    match miscmaths::read_ascii_matrix(name) {
        Ok(m) => Ok(m),
        Err(MatrixFileError::CannotOpen) => {
            err.push_str(&format!("Could not open matrix file {name}\n"));
            Ok(Mat::zeros(0, 0))
        }
        Err(MatrixFileError::NotEnoughNumbers) => {
            Err(runtime("Could not find enough numbers in matrix file"))
        }
    }
}

fn body(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    out: &mut Cout,
    err: &mut String,
    notices: &mut Vec<Notice>,
) -> Result<(), End> {
    let argc = args.len() + 1;
    if argc < 2 {
        err.push_str(
            "Usage: avscale [--allparams/--inverteddies] matrixfile [non-reference-volume]\n",
        );
        return Err(End::Exit(255));
    }
    let mut offset = 1usize;
    let mut optionarg = args[0].clone();
    if !optionarg.starts_with('-') {
        optionarg.clear();
        offset = 0;
    }
    // argv[1 + offset]: past the arguments, a null pointer made into a std::string.
    let arg = |i: usize| -> Result<&str, End> {
        args.get(i - 1).map(String::as_str).ok_or(End::Throw {
            printed: String::new(),
            type_name: "std::logic_error",
            what: "basic_string: construction from null is not valid".to_owned(),
        })
    };
    let mut cor = [0.0f64; 3];
    let affmat;
    if argc > 2 + offset {
        let volname = arg(2 + offset)?.to_owned();
        let mut warnings = Vec::new();
        if volname.is_empty() {
            // an empty volume: no voxel, so its centre of gravity divides by a total of 1
            warnings.push("WARNING::in calculating COG, total = 0.0".to_owned());
        } else {
            let (testvol, _) = volume::read_volume::<f32>(&volname, settings)?;
            affmat = read_matrix(arg(1 + offset)?, err)?;
            if affmat.rows() < 4 {
                return Err(End::Exit(254));
            }
            cor = testvol.cog_scaled_mm(&mut warnings);
            for w in &warnings {
                err.push_str(w);
                err.push('\n');
            }
            return report_named(&affmat, cor, &optionarg, out, mode, notices);
        }
        affmat = read_matrix(arg(1 + offset)?, err)?;
        if affmat.rows() < 4 {
            return Err(End::Exit(254));
        }
        for w in &warnings {
            err.push_str(w);
            err.push('\n');
        }
    } else {
        affmat = read_matrix(arg(1 + offset)?, err)?;
        if affmat.rows() < 4 {
            return Err(End::Exit(254));
        }
    }
    let _ = &mut cor;
    report_named(&affmat, cor, &optionarg, out, mode, notices)
}

/// [`report`] with the mode's double `cos` (sqrtaff's: correctly rounded by default, glibc's with
/// `reproduce`, GLIBC-07), and GLIBC-07 named, in both modes, where the other choice prints
/// another report (the user's decision of 2026-11-04: a pointwise program names it exactly).
fn report_named(
    affmat: &Mat,
    cor: [f64; 3],
    optionarg: &str,
    out: &mut Cout,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) -> Result<(), End> {
    let start = out.text.len();
    let mut other_out = Cout {
        text: String::new(),
        fixed: out.fixed,
    };
    let r = report(affmat, cor, optionarg, out, &Libm::quiet(mode));
    let other_mode = if mode.reproduce() {
        UpstreamDefects::Fix
    } else {
        UpstreamDefects::Reproduce
    };
    let r_other = report(
        affmat,
        cor,
        optionarg,
        &mut other_out,
        &Libm::quiet(other_mode),
    );
    if other_out.text != out.text[start..] || r.is_ok() != r_other.is_ok() {
        let detail = if mode.reproduce() {
            "avscale's half transforms from glibc's cos, as FSL, where the correctly rounded cos \
             prints others"
        } else {
            "avscale's half transforms from the correctly rounded cos, where glibc's (FSL's) \
             prints others"
        };
        defects::push_once(notices, Notice::new(&defects::GLIBC_07, mode, detail));
    }
    r
}

fn report(
    affmat: &Mat,
    cor: [f64; 3],
    optionarg: &str,
    out: &mut Cout,
    libm: &Libm,
) -> Result<(), End> {
    if affmat.rows() != 4 || affmat.cols() != 4 {
        return Err(End::Unsupported(format!(
            "a {} x {} matrix",
            affmat.rows(),
            affmat.cols()
        )));
    }
    let params = miscmaths::decompose_aff_try(affmat, &Mat::col(&cor), false).map_err(runtime)?;
    let zero = Mat::col(&[0.0, 0.0, 0.0]);
    let rotmat = miscmaths::construct_rotmat_euler(&Mat::col(&params), 6, &zero);
    out.s("Rotation & Translation Matrix:\n");
    out.mat(&rotmat);
    out.s("\n");
    if optionarg == "--allparams" {
        out.s("Rotation Angles (x,y,z) [rads] = ");
        out.row(&params[0..3]);
        out.s("\n");
        out.s("Translations (x,y,z) [mm] = ");
        out.row(&params[3..6]);
        out.s("\n");
    }
    out.s("Scales (x,y,z) = ");
    out.row(&params[6..9]);
    out.s("\n");
    out.s("Skews (xy,xz,yz) = ");
    out.row(&params[9..12]);
    out.s("\n");
    let avscale = ((params[6] + params[7] + params[8]) / 3.0) as f32;
    out.s("Average scaling = ");
    out.num(f64::from(avscale));
    out.s("\n\n");
    let det = arma::det(affmat);
    out.s("Determinant = ");
    out.num(det);
    out.s("\nLeft-Right orientation: ");
    out.s(if det > 0.0 {
        "preserved\n"
    } else {
        "swapped\n"
    });
    out.s("\n");
    let mut swapmat = Mat::eye(4);
    if det < 0.0 {
        swapmat.set(0, 0, -1.0);
    }
    let m2 = miscmaths::sqrtaff(&arma::mul_tinysq(affmat, &swapmat), libm).map_err(runtime)?;
    // m2 * affmat.i(): the inverse evaluated, then the product.
    let ai = arma::inv(affmat).map_err(|_| runtime(INV_ERROR))?;
    let m0 = arma::mul_tinysq(&m2, &ai);
    out.s("Forward half transform =\n");
    out.mat(&m2);
    out.s("\n");
    out.s("Backward half transform =\n");
    out.mat(&m0);
    out.s("\n");
    let mut scale = Mat::eye(4);
    let mut skew = Mat::eye(4);
    scale.set(0, 0, params[6]);
    scale.set(1, 1, params[7]);
    scale.set(2, 2, params[8]);
    skew.set(0, 1, params[9]);
    skew.set(0, 2, params[10]);
    skew.set(1, 2, params[11]);
    // affmat.i() * rotmat * skew * scale: four operands, ((A^-1 R) K) S (Armadillo evaluates the
    // inverse with the others; no solve for a chain of four).
    let ans = arma::mul_tinysq(
        &arma::mul_tinysq(&arma::mul_tinysq(&ai, &rotmat), &skew),
        &scale,
    );
    if optionarg == "--inverteddies" {
        out.s("\nMatrix check: mat^-1 * rotmat * skew * scale =\n");
        out.mat(&ans);
        out.s("\n");
        scale.set(0, 0, params[6]);
        scale.set(1, 1, 1.0 / params[7]);
        scale.set(2, 2, params[8]);
        skew.set(0, 1, -params[9]);
        skew.set(0, 2, params[10]);
        skew.set(1, 2, -params[11]);
        let newxfm = arma::mul_tinysq(&arma::mul_tinysq(&rotmat, &skew), &scale);
        out.s("Inverted eddy matrix:\n");
        out.mat(&newxfm);
        out.s("\n");
    }
    Ok(())
}

/// `avscale` with its arguments (without the program name).
pub fn run(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut o = Cout::default();
    let mut e_text = String::new();
    let mut notices = Vec::new();
    let code = match body(args, settings, mode, &mut o, &mut e_text, &mut notices) {
        Ok(()) => 0,
        Err(End::Exit(c)) => c,
        Err(End::Throw {
            printed,
            type_name,
            what,
        }) => {
            e_text.push_str(&printed);
            if type_name == "std::runtime_error" {
                e_text.push_str(&what);
                e_text.push('\n');
            } else {
                e_text.push_str("Image error\n");
            }
            e_text.push_str(&format!(
                "terminate called after throwing an instance of '{type_name}'\n  what():  {what}\n"
            ));
            e_text.push_str(&format!(
                "{PREFIX}: avscale aborts here (signal 6); the replica exits with status 1\n"
            ));
            1
        }
        Err(End::Unsupported(what)) => {
            e_text.push_str(&format!(
                "{PREFIX}: error: {what}: FSL computes it with BLAS or LAPACK routines beyond \
                 4 x 4, which the replica does not translate\n"
            ));
            1
        }
    };
    for n in &notices {
        e_text.push_str(&format!("{PREFIX}: {}\n", n.line()));
    }
    let _ = out.write_all(o.text.as_bytes());
    let _ = err.write_all(e_text.as_bytes());
    let _ = out.flush();
    let _ = err.flush();
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rotation of Euler angles `(rx, ry, rz)` about the origin, its translation `t`.
    fn affine(r: [f64; 3], t: [f64; 3]) -> Mat {
        let zero = Mat::col(&[0.0, 0.0, 0.0]);
        let mut m = miscmaths::construct_rotmat_euler(&Mat::col(&[r[0], r[1], r[2]]), 3, &zero);
        for (i, v) in t.iter().enumerate() {
            m.set(i, 3, *v);
        }
        m
    }

    /// The argument of sqrtaff's `cos` for `m` (its expressions, `m` of positive determinant).
    fn cos_argument(m: &Mat) -> f64 {
        let zero = Mat::col(&[0.0, 0.0, 0.0]);
        let p = miscmaths::decompose_aff_try(m, &zero, true).unwrap();
        let sint = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        0.5 * larmorx_core::math::asin(sint)
    }

    /// GLIBC-07 in avscale (the user's decision of 2026-11-04: a pointwise program names it
    /// exactly): the half transforms' `cos` is glibc's with `reproduce` and correctly rounded by
    /// default, and GLIBC-07 is named, in both modes, only where the two print different
    /// reports. sqrtaff builds its rotation in `float` (construct_rotmat_quat), which hides the
    /// double factor's last bit: on the rotations about z whose `cos` argument glibc does not
    /// round correctly (the factor `1 / (2 cos)` differs in most), with translations of 10^13 mm
    /// (where a last bit would reach the printed digits), the half transforms and the reports are
    /// the same in both modes and nothing is named.
    #[test]
    fn glibc07_is_named_only_where_the_reports_differ() {
        let (cos, cos_glibc) = (larmorx_core::math::cos, larmorx_core::math::cos_glibc);
        let (mut compared, mut factors_differ) = (0, 0);
        for k in 1..60000 {
            let m = affine([0.0, 0.0, f64::from(k) * 5e-5], [3e13, -2e13, 1e13]);
            let u = cos_argument(&m);
            if cos(u).to_bits() == cos_glibc(u).to_bits() && k % 5000 != 0 {
                continue;
            }
            compared += 1;
            let factor = |c: f64| (1.0 / (2.0 * c)).to_bits();
            factors_differ += usize::from(factor(cos(u)) != factor(cos_glibc(u)));
            let half = |mode| {
                let h = miscmaths::sqrtaff(&m, &Libm::quiet(mode)).unwrap();
                h.as_slice().iter().map(|v| v.to_bits()).collect::<Vec<_>>()
            };
            assert_eq!(half(UpstreamDefects::Fix), half(UpstreamDefects::Reproduce));
            let mut texts = Vec::new();
            for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
                let (mut out, mut notices) = (Cout::default(), Vec::new());
                assert!(report_named(&m, [0.0; 3], "", &mut out, mode, &mut notices).is_ok());
                assert!(notices.is_empty(), "{k}: {notices:?}");
                texts.push(out.text);
            }
            assert_eq!(texts[0], texts[1]);
        }
        assert!(
            compared > 30 && factors_differ > 20,
            "{compared}, {factors_differ}"
        );
    }
}
