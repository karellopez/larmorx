// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl convert_xfm`: FSL's `convert_xfm` (`specs/flirt.md` §10).

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_core::math::Libm;

use super::Settings;
use crate::flirt::mat::{self, Mat4, Stream};
use crate::flirt::xfm;

/// The usage text (observed).
pub const USAGE: &str = "convert_xfm (Version 2.1)
Tool for manipulating FSL transformation matrices
Copyright(c) 1999-2007, University of Oxford (Mark Jenkinson)

Usage: convert_xfm [options] <input-matrix-filename>
  e.g. convert_xfm -omat <outmat> -inverse <inmat>
       convert_xfm -omat <outmat_AtoC> -concat <mat_BtoC> <mat_AtoB>

  Available options are:
        -omat <matrix-filename>            (4x4 ascii format)
        -concat <second-matrix-filename>
        -fixscaleskew <second-matrix-filename>
        -inverse                           (Reference image must be the one originally used)
        -help
";

/// A matrix file as `convert_xfm` reads it: `None` (with FSL's messages) unless it has 4 rows.
fn read(settings: &Settings, name: &str, err: &mut dyn Write) -> Option<Mat4> {
    let rows = match std::fs::read_to_string(settings.path(name)) {
        Ok(t) => match mat::parse_matrix_text(&t) {
            Ok(r) => r,
            Err(m) => {
                let _ = writeln!(err, "{m}");
                Vec::new()
            }
        },
        Err(_) => {
            let _ = writeln!(err, "Could not open matrix file {name}");
            Vec::new()
        }
    };
    mat::mat4_of(&rows)
}

/// `convert_xfm` with FSL's arguments; returns the exit status.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if args.is_empty() {
        let _ = write!(out, "{USAGE}");
        return 1;
    }
    let (mut omat, mut concat, mut fix, mut inverse) = (None, None, None, false);
    let mut verbose = 0i32;
    let mut input: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let w = args[i].as_str();
        if w.is_empty() {
            i += 1;
            continue;
        }
        if !w.starts_with('-') {
            if input.is_some() {
                let _ = writeln!(err, "Unknown option {w}\n");
                return 2;
            }
            input = Some(w.to_owned());
            i += 1;
            continue;
        }
        match w {
            "-help" => {
                let _ = write!(out, "{USAGE}");
                return 0;
            }
            "-inverse" => inverse = true,
            "-v" => verbose = 5,
            "-omat" | "-concat" | "-fixscaleskew" | "-verbose" => {
                let Some(v) = args.get(i + 1) else {
                    let _ = writeln!(err, "Lacking argument to option {w}\n");
                    return 2;
                };
                match w {
                    "-omat" => omat = Some(v.clone()),
                    "-concat" => concat = Some(v.clone()),
                    "-fixscaleskew" => fix = Some(v.clone()),
                    _ => verbose = mat::atoi(v),
                }
                i += 1;
            }
            _ => {
                let _ = writeln!(err, "Unrecognised option {w}\n");
                return 255;
            }
        }
        i += 1;
    }
    let Some(input) = input else {
        let _ = writeln!(err, "Input matrix filename not found\n");
        return 2;
    };
    let Some(mut m) = read(settings, &input, err) else {
        let _ = writeln!(err, "Cannot read input-matrix");
        return 254;
    };
    let mut stream = Stream::default();
    let show = |out: &mut dyn Write, stream: &mut Stream, title: &str, a: &Mat4| {
        let _ = write!(out, "{title}\n{}\n", stream.matrix(a));
    };
    if let Some(f) = &fix {
        let Some(s) = read(settings, f, err) else {
            let _ = writeln!(err, "Cannot read fixscaleskew-matrix");
            return 253;
        };
        if verbose > 3 {
            show(out, &mut stream, "Initial matrix:", &m);
            show(out, &mut stream, "Fix Scale-Skew matrix:", &s);
        }
        let mode = match settings.defects() {
            Ok(m) => m,
            Err(e) => {
                let _ = writeln!(err, "larmorx fsl convert_xfm: {e}");
                return 2;
            }
        };
        let fixed = xfm::fix_scale_skew(&m, &s, &Libm::new(mode));
        // GLIBC-07 (§10, §3): named where the other choice of arctangent, sine and cosine
        // writes another matrix
        let other = if mode.fix() {
            UpstreamDefects::Reproduce
        } else {
            UpstreamDefects::Fix
        };
        let fixed2 = xfm::fix_scale_skew(&m, &s, &Libm::new(other));
        if mat::file_text(&fixed) != mat::file_text(&fixed2) {
            let detail = if mode.fix() {
                "-fixscaleskew's matrix from the correctly rounded functions, where glibc's \
                 atan2f, sinf and cosf (FSL's) write another"
            } else {
                "-fixscaleskew's matrix from glibc's atan2f, sinf and cosf, as FSL, where the \
                 correctly rounded functions write another"
            };
            let n = Notice::new(&defects::GLIBC_07, mode, detail);
            super::common::print_notices(err, &[n]);
        }
        m = fixed;
    }
    if let Some(c) = &concat {
        let Some(s) = read(settings, c, err) else {
            let _ = writeln!(err, "Cannot read concat-matrix");
            return 253;
        };
        if verbose > 3 {
            show(out, &mut stream, "Initial matrix:", &m);
            show(out, &mut stream, "Second matrix:", &s);
        }
        m = xfm::concat(&m, &s);
    }
    if inverse {
        match mat::inverse(&m) {
            Some(i) => m = i,
            None => {
                let _ = writeln!(
                    err,
                    "terminate called after throwing an instance of 'std::runtime_error'\n  what():  {}",
                    mat::SINGULAR
                );
                let _ = writeln!(
                    err,
                    "larmorx fsl convert_xfm: the matrix is singular; FSL aborts here (signal 6) (larmorx)"
                );
                return 1;
            }
        }
    }
    if let Some(name) = &omat
        && std::fs::write(settings.path(name), mat::file_text(&m)).is_err()
    {
        let _ = writeln!(err, "Could not open file {name} for writing");
    }
    if verbose > 0 {
        let _ = writeln!(out, "{}", stream.matrix(&m));
    }
    0
}
