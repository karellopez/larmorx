// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-flirt 2111.4 (convert_xfm.cc). Copyright (C)
// 1999-2023 University of Oxford (Mark Jenkinson). Rust translation Copyright 2026 Karel Lopez
// Vilaret; modified from the original. Distributed under the FSL Licence (see LICENSE).
//! `convert_xfm`: FLIRT's matrix tool (`-omat`, `-concat`, `-fixscaleskew`, `-inverse`), with its
//! messages and exit statuses.

use std::io::Write;

use larmorx_core::defects::UpstreamDefects;
use larmorx_nc_core::arma::{self, Mat};
use larmorx_nc_core::cnum;
use larmorx_nc_core::miscmaths::{self, MatrixFileError};
use larmorx_nc_core::settings::Settings;

use crate::run::PREFIX;

const VERSION: &str = "2.1";

/// The parsed command line.
#[derive(Debug, Default)]
struct Opts {
    initmatfname: String,
    outputmatascii: String,
    concatfname: String,
    fixfname: String,
    verbose: i32,
    inverse: bool,
}

/// `print_usage(argc, argv)`, the program named `convert_xfm`.
fn usage() -> String {
    let p = "convert_xfm";
    format!(
        "convert_xfm (Version {VERSION})\nTool for manipulating FSL transformation matrices\n\
         Copyright(c) 1999-2007, University of Oxford (Mark Jenkinson)\n\n\
         Usage: {p} [options] <input-matrix-filename>\n  e.g. {p} -omat <outmat> -inverse <inmat>\n       \
         {p} -omat <outmat_AtoC> -concat <mat_BtoC> <mat_AtoB>\n\n  Available options are:\n        \
         -omat <matrix-filename>            (4x4 ascii format)\n        \
         -concat <second-matrix-filename>\n        \
         -fixscaleskew <second-matrix-filename>\n        \
         -inverse                           (Reference image must be the one originally used)\n        \
         -help\n"
    )
}

/// How the program ends early.
enum End {
    Exit(u8),
    /// An uncaught C++ exception: std::terminate's text, then abort.
    Terminate {
        type_name: &'static str,
        what: String,
    },
    /// A matrix size whose arithmetic Armadillo hands to BLAS or LAPACK beyond 4 x 4 (not ported).
    Unsupported(String),
}

fn parse(args: &[String], err: &mut String, out: &mut String) -> Result<Opts, u8> {
    let mut o = Opts::default();
    if args.is_empty() {
        out.push_str(&usage());
        return Err(1);
    }
    let argc = args.len() + 1;
    let mut n = 1usize;
    let mut initmatset = false;
    while n < argc {
        let arg = args[n - 1].as_str();
        if arg.is_empty() {
            n += 1;
            continue;
        }
        if !arg.starts_with('-') {
            if initmatset {
                err.push_str(&format!("Unknown option {arg}\n\n"));
                return Err(2);
            }
            o.initmatfname = arg.to_owned();
            initmatset = true;
            n += 1;
            continue;
        }
        match arg {
            "-help" => {
                out.push_str(&usage());
                return Err(0);
            }
            "-inverse" => {
                o.inverse = true;
                n += 1;
                continue;
            }
            "-v" => {
                o.verbose = 5;
                n += 1;
                continue;
            }
            _ => {}
        }
        if n + 1 >= argc {
            err.push_str(&format!("Lacking argument to option {arg}\n\n"));
            return Err(2);
        }
        let v = args[n].clone();
        match arg {
            "-concat" => o.concatfname = v,
            "-fixscaleskew" => o.fixfname = v,
            "-omat" => o.outputmatascii = v,
            "-verbose" => o.verbose = cnum::atoi(&v),
            _ => {
                err.push_str(&format!("Unrecognised option {arg}\n\n"));
                return Err(255);
            }
        }
        n += 2;
    }
    if o.initmatfname.is_empty() {
        err.push_str("Input matrix filename not found\n\n");
        return Err(2);
    }
    Ok(o)
}

/// `cout << matrix` (fixed, 6 decimals, a blank after each number).
fn print_mat(m: &Mat) -> String {
    let mut t = String::new();
    for r in 0..m.rows() {
        for c in 0..m.cols() {
            t.push_str(&cnum::fmt_f(m.at(r, c), 6));
            t.push(' ');
        }
        t.push('\n');
    }
    t
}

/// `write_ascii_matrix(mat, filename)`.
fn write_matrix(m: &Mat, name: &str) -> Result<(), String> {
    let mut t = String::new();
    for r in 0..m.rows() {
        for c in 0..m.cols() {
            t.push_str(&cnum::fmt_g(m.at(r, c), 10));
            t.push_str("  ");
        }
        t.push('\n');
    }
    std::fs::write(name, t).map_err(|_| format!("Could not open file {name} for writing\n"))
}

fn read(name: &str, err: &mut String) -> Result<Mat, End> {
    match miscmaths::read_ascii_matrix(name) {
        Ok(m) => Ok(m),
        Err(MatrixFileError::CannotOpen) => {
            err.push_str(&format!("Could not open matrix file {name}\n"));
            Ok(Mat::zeros(0, 0))
        }
        Err(MatrixFileError::NotEnoughNumbers) => Err(End::Terminate {
            type_name: "armawrap::AWException",
            what: "Could not find enough numbers in matrix file".to_owned(),
        }),
    }
}

fn is4(m: &Mat) -> bool {
    m.rows() == 4 && m.cols() == 4
}

fn body(o: &Opts, out: &mut String, err: &mut String) -> Result<(), End> {
    let mut affmat = read(&o.initmatfname, err)?;
    if affmat.rows() < 4 {
        err.push_str("Cannot read input-matrix\n");
        return Err(End::Exit(254));
    }
    let zero = Mat::col(&[0.0, 0.0, 0.0]);
    if !o.fixfname.is_empty() {
        let fixmat = read(&o.fixfname, err)?;
        if fixmat.rows() < 4 {
            err.push_str("Cannot read fixscaleskew-matrix\n");
            return Err(End::Exit(253));
        }
        if o.verbose > 2 {
            out.push_str("Initial matrix:\n");
            out.push_str(&print_mat(&affmat));
            out.push('\n');
            out.push_str("Fix Scale-Skew matrix:\n");
            out.push_str(&print_mat(&fixmat));
            out.push('\n');
        }
        if !is4(&affmat) || !is4(&fixmat) {
            return Err(End::Unsupported(
                "-fixscaleskew with matrices that are not 4 x 4".to_owned(),
            ));
        }
        let initp = miscmaths::decompose_aff(&affmat, &zero);
        let fixp = miscmaths::decompose_aff(&fixmat, &zero);
        let mut comb = [0.0f64; 12];
        comb[..6].copy_from_slice(&initp[..6]);
        comb[6..].copy_from_slice(&fixp[6..]);
        affmat = miscmaths::compose_aff(&Mat::col(&comb), 12, &zero);
    }
    if !o.concatfname.is_empty() {
        let concatmat = read(&o.concatfname, err)?;
        if concatmat.rows() < 4 {
            err.push_str("Cannot read concat-matrix\n");
            return Err(End::Exit(253));
        }
        if o.verbose > 2 {
            out.push_str("Initial matrix:\n");
            out.push_str(&print_mat(&affmat));
            out.push('\n');
            out.push_str("Second matrix:\n");
            out.push_str(&print_mat(&concatmat));
            out.push('\n');
        }
        if concatmat.cols() != affmat.rows() {
            return Err(End::Terminate {
                type_name: "std::logic_error",
                what: format!(
                    "matrix multiplication: incompatible matrix dimensions: {}x{} and {}x{}",
                    concatmat.rows(),
                    concatmat.cols(),
                    affmat.rows(),
                    affmat.cols()
                ),
            });
        }
        if !is4(&affmat) || !is4(&concatmat) {
            return Err(End::Unsupported(
                "-concat with matrices that are not 4 x 4".to_owned(),
            ));
        }
        affmat = arma::mul_tinysq(&concatmat, &affmat);
    }
    if o.inverse {
        if affmat.rows() != affmat.cols() {
            return Err(End::Terminate {
                type_name: "std::logic_error",
                what: "inv(): given matrix must be square sized".to_owned(),
            });
        }
        if !is4(&affmat) {
            return Err(End::Unsupported(
                "-inverse of a matrix that is not 4 x 4".to_owned(),
            ));
        }
        affmat = match arma::inv(&affmat) {
            Ok(m) => m,
            Err(_) => {
                return Err(End::Terminate {
                    type_name: "std::runtime_error",
                    what: "inv(): matrix is singular".to_owned(),
                });
            }
        };
    }
    if !o.outputmatascii.is_empty()
        && let Err(e) = write_matrix(&affmat, &o.outputmatascii)
    {
        err.push_str(&e);
    }
    if o.verbose > 0 {
        out.push_str(&print_mat(&affmat));
        out.push('\n');
    }
    Ok(())
}

/// `convert_xfm` with its arguments (without the program name).
pub fn run(
    args: &[String],
    _settings: &Settings,
    _mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let (mut o_text, mut e_text) = (String::new(), String::new());
    let code = match parse(args, &mut e_text, &mut o_text) {
        Err(c) => c,
        Ok(o) => match body(&o, &mut o_text, &mut e_text) {
            Ok(()) => 0,
            Err(End::Exit(c)) => c,
            Err(End::Terminate { type_name, what }) => {
                e_text.push_str(&format!(
                    "terminate called after throwing an instance of '{type_name}'\n  what():  {what}\n"
                ));
                e_text.push_str(&format!(
                    "{PREFIX}: convert_xfm aborts here (signal 6); the replica exits with status 1\n"
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
        },
    };
    let _ = out.write_all(o_text.as_bytes());
    let _ = err.write_all(e_text.as_bytes());
    let _ = out.flush();
    let _ = err.flush();
    code
}
