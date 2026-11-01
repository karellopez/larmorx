// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-flirt 2111.4 (rmsdiff.cc). Copyright (C) 1999-2023
// University of Oxford (Mark Jenkinson). Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the original. Distributed under the FSL Licence (see LICENSE).
//! `rmsdiff`: the RMS deviation between two affine matrices over a sphere of 80 mm about a
//! volume's centre of gravity, or, with a mask, the largest and the RMS displacement over the
//! mask's voxels, with its messages and exit statuses.

use std::io::Write;

use larmorx_core::defects::UpstreamDefects;
use larmorx_nc_core::arma::{self, Mat};
use larmorx_nc_core::cnum;
use larmorx_nc_core::miscmaths::{self, MatrixFileError};
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::sse::{self, S64};
use larmorx_nc_core::volume::{self, FslError, Volume};

use crate::run::PREFIX;

enum End {
    Exit(u8),
    Terminate {
        printed: String,
        type_name: &'static str,
        what: String,
    },
    Refuse(String),
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

/// `cerr << matrix` (armawrap's `doPrint`: fixed with 6 decimals, a blank after each value).
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

fn read_vol(name: &str, settings: &Settings, err: &mut String) -> Result<Volume, End> {
    let (v, warning) = volume::read_volume::<f32>(name, settings)?;
    if let Some(w) = warning {
        err.push_str(&w);
        err.push('\n');
    }
    Ok(v)
}

fn body(args: &[String], settings: &Settings, out: &mut String, err: &mut String) -> Result<(), End> {
    let argc = args.len() + 1;
    if argc < 4 {
        err.push_str("Usage: rmsdiff matrixfile1 matrixfile2 refvol [mask]\n");
        err.push_str("        Outputs rms deviation between matrices (in mm)\n");
        return Err(End::Exit(255));
    }
    let a1 = read_matrix(&args[0], err)?;
    if a1.rows() < 4 {
        err.push_str(&format!("Could not read matrix {}\n", args[0]));
        return Err(End::Exit(254));
    }
    let a2 = read_matrix(&args[1], err)?;
    if a2.rows() < 4 {
        err.push_str(&format!("Could not read matrix {}\n", args[1]));
        return Err(End::Exit(254));
    }
    if a1.rows() != 4 || a1.cols() != 4 || a2.rows() != 4 || a2.cols() != 4 {
        return Err(End::Refuse(
            "matrices that are not 4 x 4: FSL computes their determinants with LAPACK routines \
             beyond 4 x 4, which the replica does not translate"
                .to_owned(),
        ));
    }
    // cerr << affmat: armawrap's doPrint switches cerr (not cout) to fixed notation
    if arma::det(&a1).abs() < 0.1 {
        err.push_str("WARNING:: matrix 1 has low determinant\n");
        err.push_str(&print_mat(&a1));
        err.push('\n');
    }
    if arma::det(&a2).abs() < 0.1 {
        err.push_str("WARNING:: matrix 2 has low determinant\n");
        err.push_str(&print_mat(&a2));
        err.push('\n');
    }
    let refvol = read_vol(&args[2], settings, err)?;
    if argc < 5 {
        let mut w = Vec::new();
        let centre = refvol.cog_scaled_mm(&mut w);
        for l in &w {
            err.push_str(l);
            err.push('\n');
        }
        // rms_deviation catches the inverse's exception: its message, then exit(-5)
        let Ok(rms) = miscmaths::rms_deviation(&a1, &a2, &Mat::col(&centre), 80.0) else {
            err.push_str("RMS_DEVIATION ERROR:: Could not invert matrix\n");
            return Err(End::Exit(251));
        };
        out.push_str(&cnum::stream(f64::from(rms)));
        out.push('\n');
    } else {
        let mask = read_vol(&args[3], settings, err)?;
        let d = a1.minus(&a2);
        let (mut maxdist, mut sumdistsq) = (0.0f64, S64(0.0));
        let mut nvox: i64 = 0;
        for z in 0..mask.zsize() {
            for y in 0..mask.ysize() {
                for x in 0..mask.xsize() {
                    if f64::from(mask.value(x, y, z)) > 0.5 {
                        // x * refvol.xdim(): an int times a float, in float
                        let c = [
                            f64::from(x as f32 * refvol.xdim()),
                            f64::from(y as f32 * refvol.ydim()),
                            f64::from(z as f32 * refvol.zdim()),
                            1.0,
                        ];
                        let v = arma::gemv_tinysq(&d, &c, false);
                        let dist = miscmaths::norm2(&Mat::col(&v));
                        // Max(dist, maxdist): the first when it is larger, else the second (NaN keeps maxdist)
                        maxdist = if dist > maxdist { dist } else { maxdist };
                        sumdistsq = sumdistsq + S64(dist) * S64(dist);
                        nvox += 1;
                    }
                }
            }
        }
        out.push_str(&cnum::stream(maxdist));
        out.push('\n');
        let rms = S64(sse::div_f64(sumdistsq.0, nvox as f64)).sqrt().0;
        out.push_str(&cnum::stream(rms));
        out.push('\n');
    }
    Ok(())
}

/// `rmsdiff` with its arguments (without the program name).
pub fn run(
    args: &[String],
    settings: &Settings,
    _mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let (mut o, mut e) = (String::new(), String::new());
    let code = match body(args, settings, &mut o, &mut e) {
        Ok(()) => 0,
        Err(End::Exit(c)) => c,
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
                "{PREFIX}: rmsdiff aborts here (signal 6); the replica exits with status 1\n"
            ));
            1
        }
        Err(End::Refuse(what)) => {
            e.push_str(&format!("{PREFIX}: error: {what}\n"));
            1
        }
    };
    let _ = out.write_all(o.as_bytes());
    let _ = err.write_all(e.as_bytes());
    let _ = out.flush();
    let _ = err.flush();
    code
}
