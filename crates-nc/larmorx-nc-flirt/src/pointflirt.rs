// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-flirt 2111.4 (pointflirt.cc). Copyright (C) 2001-2023
// University of Oxford (Mark Jenkinson). Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the original. Distributed under the FSL Licence (see LICENSE).
//! `pointflirt`: the 12-parameter affine matrix that maps one set of point coordinates onto
//! another in the least-squares sense (the normal equations solved by the pseudo-inverse of their
//! 12 × 12 matrix through an SVD, singular values up to 1e-8 dropped), written to a file or
//! printed, with FSL's option parsing, messages and exit statuses.
//!
//! The matrix expressions follow Armadillo 14.2.1 (fsl-armawrap 0.7.0) on FSL's OpenBLAS: `syrk`
//! for `x·xᵀ`, BLAS `dgemm` for `x·yᵀ`, LAPACK `dgesdd` for the SVD, and the four-factor
//! product `V·D·Uᵀ·q` evaluated right to left by `dgemv` ([`arma`]).

use std::io::Write;

use larmorx_core::defects::UpstreamDefects;
use larmorx_nc_core::arma::{self, Mat};
use larmorx_nc_core::miscmaths::{self, MatrixFileError};
use larmorx_nc_core::options::{ArgFlag, Opt, OptionParser, Value};
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volume::{self, FslError};

use crate::midtrans::print_mat;
use crate::run::PREFIX;

const TITLE: &str = "pointflirt \nCopyright(c) 2001, University of Oxford (Mark Jenkinson)";
const EXAMPLES: &str = "pointflirt -i <invol coords>  -r <refvol coords> -o <output matrix>\npointflirt -i <invol coords>  -r <refvol coords> -o <output matrix> --vox --invol=<input vol> --refvol=<ref vol>";

enum End {
    Exit(u8),
    /// The replica's refusal of what crashes FSL (`larmorx-nc: error: pointflirt: ...`, exit
    /// 1).
    Refuse(String),
    Terminate {
        printed: String,
        type_name: &'static str,
        what: String,
    },
}

impl From<FslError> for End {
    fn from(e: FslError) -> End {
        End::Terminate {
            printed: e.printed_text(),
            type_name: e.type_name,
            what: e.what,
        }
    }
}

fn parser() -> OptionParser {
    let opt = |key, default, help, compulsory, flag| Opt {
        key,
        default,
        help,
        compulsory,
        flag,
        visible: true,
    };
    OptionParser::new(
        TITLE,
        EXAMPLES,
        vec![
            opt(
                "-i",
                Value::Str(String::new()),
                "filename of input volume coordinates",
                true,
                ArgFlag::Requires(1),
            ),
            opt(
                "-r",
                Value::Str(String::new()),
                "filename of reference volume coordinates",
                true,
                ArgFlag::Requires(1),
            ),
            opt(
                "-o,--out",
                Value::Str(String::new()),
                "filename of affine matrix output",
                false,
                ArgFlag::Requires(1),
            ),
            opt(
                "--vox",
                Value::Bool(false),
                "use voxel coordinates, not mm, for input",
                false,
                ArgFlag::NoArgument,
            ),
            opt(
                "--invol",
                Value::Str(String::new()),
                "filename of input volume (needed for --vox option)",
                false,
                ArgFlag::Requires(1),
            ),
            opt(
                "--refvol",
                Value::Str(String::new()),
                "filename of reference volume (needed for --vox option)",
                false,
                ArgFlag::Requires(1),
            ),
            opt(
                "-v,--verbose",
                Value::Bool(false),
                "switch on diagnostic messages",
                false,
                ArgFlag::NoArgument,
            ),
            opt(
                "-h,--help",
                Value::Bool(false),
                "display this message",
                false,
                ArgFlag::NoArgument,
            ),
        ],
    )
}

/// `Mat::operator(): index out of bounds` and the other logic errors of a bad shape, which
/// `procrustes` (outside `main`'s `try`) does not catch.
fn logic(what: &str) -> End {
    End::Terminate {
        printed: String::new(),
        type_name: "std::logic_error",
        what: what.to_owned(),
    }
}

fn read_matrix(name: &str, err: &mut String) -> Result<Mat, End> {
    match miscmaths::read_ascii_matrix(name) {
        Ok(m) => Ok(m),
        Err(MatrixFileError::CannotOpen) => {
            err.push_str(&format!("Could not open matrix file {name}\n"));
            Ok(Mat::zeros(0, 0))
        }
        Err(MatrixFileError::NotEnoughNumbers) => Err(End::Terminate {
            printed: String::new(),
            type_name: "armawrap::AWException",
            what: "Could not find enough numbers in matrix file".to_owned(),
        }),
    }
}

/// MISCMATHS' `mean(mat, 2)`: each row's mean, `Σ x/N` in order.
fn mean_rows(x: &Mat) -> Mat {
    let n = x.cols() as f64;
    let mut out = Mat::zeros(x.rows(), 1);
    for r in 0..x.rows() {
        let mut s = 0.0f64;
        for c in 0..x.cols() {
            s += x.at(r, c) / n;
        }
        out.set(r, 0, s);
    }
    out
}

fn procrustes(
    get: &dyn Fn(&str) -> (bool, String),
    voxcoord: bool,
    verbose: bool,
    settings: &Settings,
    out: &mut String,
    err: &mut String,
) -> Result<(), End> {
    let mut x = read_matrix(&get("-i").1, err)?;
    let mut y = read_matrix(&get("-r").1, err)?;
    if x.rows() > x.cols() {
        x = x.t();
        y = y.t();
    }
    let n = x.cols();
    if y.cols() != n {
        err.push_str("ERROR: Different number of coordinates in the two files\n");
        return Err(End::Exit(255));
    }
    if voxcoord {
        let (iset, iname) = get("--invol");
        let (rset, rname) = get("--refvol");
        if !iset || !rset {
            err.push_str("ERROR: need to specify --invol and --refvol to use --vox\n");
            return Err(End::Exit(2));
        }
        let inv = volume::read_volume_hdr_only(&iname, settings)?;
        let refv = volume::read_volume_hdr_only(&rname, settings)?;
        if n > 0 && (x.rows() < 3 || y.rows() < 3) {
            return Err(logic("Mat::operator(): index out of bounds"));
        }
        for m in 0..n {
            x.set(0, m, x.at(0, m) * f64::from(inv.xdim()));
            x.set(1, m, x.at(1, m) * f64::from(inv.ydim()));
            x.set(2, m, x.at(2, m) * f64::from(inv.zdim()));
            y.set(0, m, y.at(0, m) * f64::from(refv.xdim()));
            y.set(1, m, y.at(1, m) * f64::from(refv.ydim()));
            y.set(2, m, y.at(2, m) * f64::from(refv.zdim()));
        }
    }
    if verbose {
        out.push_str(&format!(
            "Input volume coordinates are (in mm)\n{}\n",
            print_mat(&x)
        ));
        out.push_str(&format!(
            "Reference volume coordinates are (in mm)\n{}\n",
            print_mat(&y)
        ));
    }
    if n == 0 && x.rows() == 0 && y.rows() == 0 {
        // FSL-204 (rule B, both modes): two empty matrices (both files empty or missing) pass
        // the count test; the submatrix copies of their empty products then crash FSL
        // (signal 11), after its lines above. Never reproduced.
        return Err(End::Refuse(
            "pointflirt: no points in the coordinate files (FSL-204)".to_owned(),
        ));
    }
    if x.rows() != 3 || y.rows() != 3 {
        // The 12 x 12 system's blocks need three coordinates per point. FSL reads its
        // submatrices of x·yᵀ unchecked (past their elements when there are fewer than three
        // rows), then the size check of `q(10:12) = ybar` (or, with three rows in y, of
        // `w(1:3, 1) = xbar`) throws, outside main's try: nothing read past the elements is
        // used.
        let bad = if y.rows() != 3 { y.rows() } else { x.rows() };
        return Err(logic(&format!(
            "copy into submatrix: incompatible matrix dimensions: 3x1 and {bad}x1"
        )));
    }
    let s = arma::syrk(&x);
    let xbar = mean_rows(&x);
    let ybar = mean_rows(&y);
    let xyt = arma::mul_tb(&x, &y);
    let mut q = Mat::zeros(12, 1);
    for r in 0..3 {
        q.set(r, 0, xyt.at(r, 0));
        q.set(3 + r, 0, xyt.at(r, 1));
        q.set(6 + r, 0, xyt.at(r, 2));
        q.set(9 + r, 0, ybar.get(r));
    }
    let mut w = Mat::zeros(9, 3);
    for k in 0..3 {
        for r in 0..3 {
            w.set(3 * k + r, k, xbar.get(r));
        }
    }
    let mut b = Mat::zeros(12, 12);
    for k in 0..3 {
        b.set_sub(3 * k, 3 * k, &s);
    }
    b.set_sub(0, 9, &w.scale(n as f64));
    b.set_sub(9, 0, &w.t());
    b.set_sub(9, 9, &Mat::eye(3));
    let Some((uu, mut dd, vv)) = arma::svd_econ(&b) else {
        // A non-finite coordinate: Armadillo's svd_econ fails and leaves its outputs empty;
        // FSL then reads dd(1,1) of an empty matrix.
        return Err(logic("Mat::operator(): index out of bounds"));
    };
    let mut nullspace = 0;
    for v in dd.iter_mut() {
        if *v > 1e-8 {
            *v = 1.0 / *v;
        } else {
            *v = 0.0;
            nullspace += 1;
        }
    }
    if nullspace > 0 {
        out.push_str(&format!(
            "WARNING: nullspace of dimension {nullspace}\n  ... lacks sufficient independent \
             coordinates\nFinding non-unique solution\n"
        ));
    }
    let mut d = Mat::zeros(12, 12);
    for (k, &v) in dd.iter().enumerate() {
        d.set(k, k, v);
    }
    // p = vv*dd*uu.t()*q: Armadillo's four-factor rule gives vv*(dd*(uuᵀ*q)).
    let utq = arma::mul(&uu, &q, true);
    let p = arma::mul(&vv, &arma::mul(&d, &utq, false), false);
    let mut affmat = Mat::zeros(4, 4);
    for c in 0..3 {
        affmat.set(0, c, p.get(c));
        affmat.set(1, c, p.get(3 + c));
        affmat.set(2, c, p.get(6 + c));
    }
    affmat.set(0, 3, p.get(9));
    affmat.set(1, 3, p.get(10));
    affmat.set(2, 3, p.get(11));
    affmat.set(3, 3, 1.0);
    let (oset, oname) = get("-o,--out");
    if oset {
        if let Err(e) = crate::convert_xfm::write_matrix(&affmat, &oname) {
            err.push_str(&e);
        }
    } else {
        out.push_str(&print_mat(&affmat));
        out.push('\n');
    }
    Ok(())
}

fn body(
    args: &[String],
    settings: &Settings,
    out: &mut String,
    err: &mut String,
) -> Result<(), End> {
    let parser = parser();
    let mut argv = vec!["pointflirt".to_owned()];
    argv.extend(args.iter().cloned());
    let parsed = match parser.parse_command_line(&argv, 0, false) {
        Ok(p) => p,
        Err((_, what)) => {
            err.push_str(&parser.usage());
            err.push_str(&format!("\n{what}\n"));
            return Err(End::Exit(1));
        }
    };
    let get = |k: &str| {
        let st = parsed.get(&parser, k);
        (st.set, st.str().to_owned())
    };
    if parsed.get(&parser, "-h,--help").bool() {
        err.push_str(&parser.usage());
        return Err(End::Exit(1));
    }
    if let Some(t) = parser.check_compulsory(&parsed) {
        err.push_str(&t);
        err.push_str(&parser.usage());
        return Err(End::Exit(1));
    }
    let voxcoord = parsed.get(&parser, "--vox").bool();
    let verbose = parsed.get(&parser, "-v,--verbose").bool();
    procrustes(&get, voxcoord, verbose, settings, out, err)
}

/// `pointflirt` with its arguments (without the program name).
pub fn run(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let _ = mode;
    let (mut o, mut e) = (String::new(), String::new());
    let code = match body(args, settings, &mut o, &mut e) {
        Ok(()) => 0,
        Err(End::Exit(c)) => c,
        Err(End::Refuse(what)) => {
            e.push_str(&format!("{PREFIX}: error: {what}\n"));
            1
        }
        Err(End::Terminate {
            printed,
            type_name,
            what,
        }) => {
            e.push_str(&printed);
            e.push_str(&format!(
                "terminate called after throwing an instance of '{type_name}'\n  what():  {what}\n"
            ));
            e.push_str(&format!(
                "{PREFIX}: pointflirt aborts here (signal 6); the replica exits with status 1\n"
            ));
            1
        }
    };
    let _ = out.write_all(o.as_bytes());
    let _ = err.write_all(e.as_bytes());
    let _ = out.flush();
    let _ = err.flush();
    code
}
