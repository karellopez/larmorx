// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-flirt 2111.4 (img2imgcoord.cc, std2imgcoord.cc,
// img2stdcoord.cc) and fsl-warpfns (warpfns.h: NewimageCoord2NewimageCoord's affine path).
// Copyright (C) 1999-2023 University of Oxford (Mark Jenkinson, Jesper Andersson). Rust
// translation Copyright 2026 Karel Lopez Vilaret; modified from the originals. Distributed under
// the FSL Licence (see LICENSE).
//! `img2imgcoord`, `std2imgcoord` and `img2stdcoord`: coordinates (voxels or millimetres) mapped
//! between two images through an affine matrix, read from a file or standard input and printed
//! as FSL prints them, with their messages and exit statuses.
//!
//! The mapping is warpfns' `NewimageCoord2NewimageCoord` without a warp field: the source's
//! newimage voxel to its millimetres (`sampling_mat()`), through the matrix, to the target's
//! newimage voxel (`sampling_mat().i() *`, which Armadillo evaluates as a solve), and the
//! conversions between NIfTI voxels, newimage voxels and the images' millimetres around it.
//! `-warp` (a FNIRT warp field) is refused: FNIRT's fields come with FNIRT's port.

use std::io::{IsTerminal, Read, Write};

use larmorx_core::defects::UpstreamDefects;
use larmorx_nc_core::arma::{self, Mat};
use larmorx_nc_core::cnum;
use larmorx_nc_core::istream::{Got, IStream};
use larmorx_nc_core::miscmaths::{self, INV_ERROR, MatrixFileError, SOLVE_ERROR};
use larmorx_nc_core::nifti;
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volume::{self, FslError, Volume};

use crate::run::PREFIX;

/// The three programs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Program {
    Img2Img,
    Std2Img,
    Img2Std,
}

impl Program {
    fn name(self) -> &'static str {
        match self {
            Program::Img2Img => "img2imgcoord",
            Program::Std2Img => "std2imgcoord",
            Program::Img2Std => "img2stdcoord",
        }
    }
}

#[derive(Debug, Default)]
struct Opts {
    /// img2imgcoord's -dest, the others' -std
    dest: String,
    /// img2imgcoord's -src, the others' -img
    src: String,
    usestd: bool,
    prexfm: String,
    coords: String,
    warp: String,
    mm: bool,
    verbose: i32,
}

/// How the program ends early.
enum End {
    Exit(u8),
    /// An uncaught exception: `std::terminate`'s text, then abort (what was printed when it was
    /// thrown first).
    Terminate {
        printed: String,
        type_name: &'static str,
        what: String,
    },
    /// larmorx refuses.
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

fn runtime(what: &str) -> End {
    End::Terminate {
        printed: String::new(),
        type_name: "std::runtime_error",
        what: what.to_owned(),
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
}

fn usage(p: Program) -> String {
    let n = p.name();
    match p {
        Program::Img2Img => format!(
            "Usage: {n} [options] <filename containing coordinates>\n\n  Options are:\n        \
             -src <filename of source image>        \n        \
             -dest <filename of destination image>  \n        \
             -xfm <filename of affine transform     (e.g. source2dest.mat)>\n        \
             -warp <filename of warpfield (e.g. intermediate2dest_warp.nii.gz)>\n        \
             -premat <filename of pre-warp affine transform  (e.g. source2intermediate.mat)>   (default=identity)\n        \
             -vox                                   (all coordinates in voxels - default)\n        \
             -mm                                    (all coordinates in mm)\n        \
             -v                                     (verbose)\n        \
             -help\n\n Notes:\n  \
             (1) if '-' is used as coordinate filename then coordinates are read from standard input\n  \
             (2) the first three options are compulsory\n"
        ),
        Program::Std2Img => format!(
            "Usage: {n} [options] <filename containing coordinates>\n\n\
             e.g.   {n} -img <invol> -std <standard image> -xfm <img2standard mat file> <coordinate file>\n       \
             {n} -img <invol> -std <standard image> <coordinate file>\n       \
             {n} -img <invol> -std <standard image> - \n\n  Options are:\n        \
             -std <filename of standard image>\n        \
             -img <filename of input image>\n        \
             -xfm <filename of affine transform   (e.g. example_func2standard.mat)>\n        \
             -warp <filename of warpfield (e.g. highres2standard_warp.nii.gz)>\n        \
             -premat <filename of pre-warp affine transform  (e.g. example_func2highres.mat)>   (default=identity)\n        \
             -mm                                  (outputs coordinates in mm - default)\n        \
             -vox                                 (outputs coordinates in voxels)\n        \
             -v                                   (verbose output)\n        \
             -verbose                             (more verbose output)\n        \
             -help\n\n Notes:\n  \
             (1) if '-' is used as coordinate filename then coordinates are read from standard input\n  \
             (2) the -img option is compulsory\n"
        ),
        Program::Img2Std => format!(
            "Usage: {n} [options] <filename containing coordinates>\n\
             e.g.   {n} -img <invol> -std <standard image> -xfm <img2standard mat file> <coordinate file>\n       \
             {n} -img <invol> <coordinate file>\n       \
             {n} -img <invol> - \n\n  Options are:\n        \
             -img <filename of input image>\n        \
             -std <filename of standard image>\n        \
             -xfm <filename of affine transform   (e.g. example_func2standard.mat)>\n        \
             -warp <filename of warpfield (e.g. highres2standard_warp.nii.gz)>\n        \
             -premat <filename of pre-warp affine transform  (e.g. example_func2highres.mat)>   (default=identity)\n        \
             -vox                                 (input coordinates in voxels - default)\n        \
             -mm                                  (input coordinates in mm)\n        \
             -v                                   (verbose output)\n        \
             -verbose                             (more verbose output)\n        \
             -help\n\n Notes:\n  \
             (1) if '-' is used as coordinate filename then coordinates are read from standard input\n  \
             (2) the -img option is compulsory\n"
        ),
    }
}

fn parse(p: Program, args: &[String], out: &mut Cout, err: &mut String) -> Result<Opts, End> {
    let mut o = Opts {
        mm: p == Program::Std2Img,
        ..Opts::default()
    };
    let argc = args.len() + 1;
    if argc < 2 {
        out.s(&usage(p));
        return Err(End::Exit(1));
    }
    let mut n = 1usize;
    while n < argc {
        let arg = args[n - 1].as_str();
        if arg.is_empty() {
            n += 1;
            continue;
        }
        if !arg.starts_with('-') {
            o.coords = arg.to_owned();
            n += 1;
            continue;
        }
        let one = match arg {
            "-help" => {
                out.s(&usage(p));
                return Err(End::Exit(0));
            }
            "-vox" => {
                o.mm = false;
                true
            }
            "-mm" => {
                o.mm = true;
                true
            }
            "-flirt" => {
                if p != Program::Img2Img {
                    err.push_str("WARNING::Using outdated options, please update to new usage\n");
                }
                true
            }
            "-v" => {
                o.verbose = if p == Program::Img2Img { 5 } else { 1 };
                true
            }
            "-verbose" if p != Program::Img2Img => {
                o.verbose = 5;
                true
            }
            "-" if p != Program::Img2Img => {
                o.coords = "-".to_owned();
                true
            }
            _ => false,
        };
        if one {
            n += 1;
            continue;
        }
        if n + 1 >= argc {
            err.push_str(&format!("Lacking argument to option {arg}\n"));
            break;
        }
        let v = args[n].clone();
        let (dest_opt, src_opt) = if p == Program::Img2Img {
            ("-dest", "-src")
        } else {
            ("-std", "-img")
        };
        if arg == dest_opt {
            o.dest = v;
            o.usestd = true;
        } else if arg == src_opt {
            o.src = v;
        } else if arg == "-xfm" || arg == "-premat" {
            o.prexfm = v;
        } else if arg == "-warp" {
            o.warp = v;
        } else {
            err.push_str(&format!("Unrecognised option {arg}\n"));
            return Err(End::Exit(255));
        }
        n += 2;
    }
    match p {
        Program::Img2Img => {
            if o.src.is_empty() {
                err.push_str("ERROR:: Source volume filename not found\n\n");
            }
            if o.dest.is_empty() {
                err.push_str("ERROR:: Destination volume filename not found\n\n");
            }
        }
        Program::Std2Img => {
            if o.src.is_empty() {
                err.push_str("ERROR:: image filename not found\n\n");
            }
            if o.usestd && o.dest.is_empty() {
                err.push_str("ERROR:: standard image filename not found\n\n");
            }
        }
        Program::Img2Std => {
            if o.src.is_empty() {
                err.push_str("ERROR:: input image filename not found\n\n");
            }
            if o.usestd && o.dest.is_empty() {
                err.push_str("ERROR:: standard image filename not found\n\n");
            }
        }
    }
    Ok(o)
}

/// `newimagevox2mm_mat()`: the sform, else the qform, else the voxel sizes.
fn vox2mm(v: &Volume) -> Mat {
    if v.sform_code != nifti::XFORM_UNKNOWN {
        v.sform.clone()
    } else if v.qform_code != nifti::XFORM_UNKNOWN {
        v.qform.clone()
    } else {
        let mut m = Mat::eye(4);
        m.set(0, 0, f64::from(v.xdim()));
        m.set(1, 1, f64::from(v.ydim()));
        m.set(2, 2, f64::from(v.zdim()));
        m
    }
}

/// `A.i() * x`: Armadillo's solve (`dgesv`), its error when `A` is singular.
fn solve(a: &Mat, x: &Mat) -> Result<Mat, End> {
    arma::solve(a, x).map_err(|_| runtime(SOLVE_ERROR))
}

fn mv(a: &Mat, x: &Mat) -> Mat {
    Mat::col(&arma::gemv_tinysq(a, x.as_slice(), false))
}

/// `NewimageCoord2NewimageCoord(M, srcvol, destvol, srccoord)`: newimage voxels of the source
/// to newimage voxels of the target through `M` (source mm to target mm); the coordinate's
/// fourth element kept.
fn newimage2newimage(m: &Mat, src: &Volume, dest: &Volume, coord: &Mat) -> Result<Mat, End> {
    if m.rows() != 4 || m.cols() != 4 {
        return Err(
            FslError::imthrow("NewimageCoord2NewimageCoord: M must be a 4x4 matrix", 11).into(),
        );
    }
    let mut c = Mat::col(&[coord.get(0), coord.get(1), coord.get(2), 1.0]);
    c = mv(&src.sampling_mat(), &c);
    c = mv(m, &c);
    c = solve(&dest.sampling_mat(), &c)?;
    Ok(Mat::col(&[c.get(0), c.get(1), c.get(2), coord.get(3)]))
}

fn print_info(out: &mut Cout, v: &Volume, name: &str) {
    out.s(&format!(
        "{name}:: SIZE = {} x {} x {}\n",
        v.xsize(),
        v.ysize(),
        v.zsize()
    ));
    out.s(&format!("{name}:: DIMS = "));
    out.num(f64::from(v.xdim()));
    out.s(" x ");
    out.num(f64::from(v.ydim()));
    out.s(" x ");
    out.num(f64::from(v.zdim()));
    out.s(" mm\n\n");
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

/// The coordinates' text: a file, or standard input (read to its end).
enum Source {
    File(Vec<u8>),
    Stdin(Vec<u8>),
}

fn body(
    p: Program,
    args: &[String],
    settings: &Settings,
    out: &mut Cout,
    err: &mut String,
) -> Result<(), End> {
    let o = parse(p, args, out, err)?;
    let empty = || {
        let mut v = Volume::new7([0, 0, 0, 1, 1, 1, 1]);
        v.radiological_file = true;
        v
    };
    // img2imgcoord reads both images; the others the standard one only with -std.
    let srcvol = volume::read_volume_hdr_only(&o.src, settings)?;
    let destvol = if p == Program::Img2Img || o.usestd {
        volume::read_volume_hdr_only(&o.dest, settings)?
    } else {
        empty()
    };
    if o.verbose > 3 {
        match p {
            Program::Img2Img => {
                print_info(out, &destvol, "Destination Volume");
                print_info(out, &srcvol, "Source Volume");
            }
            _ => {
                if o.usestd {
                    print_info(out, &destvol, "standard image");
                }
                print_info(out, &srcvol, "input image");
            }
        }
    }
    let affmat = if p == Program::Img2Img {
        let m = if o.prexfm.is_empty() {
            Mat::eye(4)
        } else {
            read_matrix(&o.prexfm, err)?
        };
        if m.rows() < 4 {
            err.push_str("Cannot read transform file\n");
            return Err(End::Exit(254));
        }
        m
    } else if !o.prexfm.is_empty() {
        let m = read_matrix(&o.prexfm, err)?;
        if m.rows() < 4 {
            err.push_str("Cannot read transform file\n");
            return Err(End::Exit(254));
        }
        m
    } else {
        Mat::eye(4)
    };
    if o.verbose > 3 {
        out.s(" affmat =\n");
        out.mat(&affmat);
        out.s("\n\n");
        if p == Program::Std2Img {
            // an Armadillo expression: Armadillo's own layout, the stream's state kept
            let inv = inverse(&affmat)?;
            out.s(" Inverse affmat =\n");
            out.s(&arma::print_expr(&inv));
            out.s("\n\n");
        }
    }
    if !o.warp.is_empty() {
        // FnirtFileReader::Read: a file that is not an image ends the program (its messages,
        // then FSL's); a warp field is not replicated yet.
        if let Err(e) = volume::read_volume_hdr_only(&o.warp, settings) {
            err.push_str(&e.printed_text());
            err.push_str(&format!(
                "An error occured while reading file: {}\n",
                o.warp
            ));
            return Err(End::Exit(1));
        }
        return Err(End::Refuse(format!(
            "-warp {}: FNIRT's warp fields are not replicated yet (they come with FNIRT's port)",
            o.warp
        )));
    }
    if p != Program::Img2Img {
        if destvol.sform_code == nifti::XFORM_UNKNOWN && destvol.qform_code == nifti::XFORM_UNKNOWN
        {
            err.push_str(if p == Program::Std2Img {
                "WARNING:: standard coordinates not set in standard image\n"
            } else {
                "WARNING:: mm coordinate space not set in standard volume\n"
            });
        }
        if o.verbose > 3 {
            out.s(" stdvox2world =\n");
            out.mat(&vox2mm(&destvol));
            out.s("\n\n");
        }
    }
    let use_stdin = o.coords == "-" || o.coords.is_empty();
    match p {
        Program::Img2Img => {
            out.s(if o.mm {
                "Coordinates in Destination volume (in mm)\n"
            } else {
                "Coordinates in Destination volume (in voxels)\n"
            });
        }
        Program::Std2Img => {
            if o.verbose > 0 {
                out.s(if o.mm {
                    "Coordinates in input image (in mm):\n"
                } else {
                    "Coordinates in input image (in voxels):\n"
                });
            }
        }
        Program::Img2Std => {
            if o.verbose > 0 {
                out.s("Coordinates in standard image (in mm):\n");
            }
        }
    }
    let source = if use_stdin {
        if o.verbose > 0 {
            out.s(match (p, o.mm) {
                (Program::Std2Img, _) => "Please type in standard coordinates :\n",
                (_, true) => "Please type in input image coordinates (in mm) :\n",
                (_, false) => "Please type in input image coordinates (in voxels) :\n",
            });
        }
        if p != Program::Std2Img && std::io::stdin().is_terminal() {
            err.push_str(&format!(
                "Interactive mode has been removed. Please pipe input such as: cat \
                 coordinate_file.txt | {}, OR supply a coordinate file\n",
                p.name()
            ));
            return Err(End::Exit(1));
        }
        let mut buf = Vec::new();
        let _ = std::io::stdin().read_to_end(&mut buf);
        Source::Stdin(buf)
    } else {
        match std::fs::read(&o.coords) {
            Ok(b) => Source::File(b),
            Err(_) => {
                err.push_str(&format!("Could not open matrix file {}\n", o.coords));
                return Err(End::Exit(255));
            }
        }
    };
    let (text, from_stdin) = match &source {
        Source::File(b) => (b.as_slice(), false),
        Source::Stdin(b) => (b.as_slice(), true),
    };
    let mut s = IStream::new(text);
    // the coordinate vectors keep their values between lines (the fourth element is 1)
    let mut c = [0.0f64, 0.0, 0.0, 1.0];
    let mut old = [0.0f64; 4];
    loop {
        let mut ok = true;
        for slot in c.iter_mut().take(3) {
            match s.f64() {
                Got::Value(v) => *slot = v,
                Got::Failed(v) => {
                    *slot = v;
                    ok = false;
                }
                Got::Untouched => ok = false,
            }
            if !ok {
                break;
            }
        }
        if !ok {
            break;
        }
        let coord = Mat::col(&c);
        let r = match p {
            Program::Img2Img => {
                if o.mm {
                    let v = solve(&vox2mm(&srcvol), &coord)?;
                    let n = newimage2newimage(&affmat, &srcvol, &destvol, &v)?;
                    mv(&vox2mm(&destvol), &n)
                } else {
                    let v = mv(&srcvol.niftivox2newimagevox_mat(), &coord);
                    let n = newimage2newimage(&affmat, &srcvol, &destvol, &v)?;
                    solve(&destvol.niftivox2newimagevox_mat(), &n)?
                }
            }
            Program::Std2Img => {
                if from_stdin {
                    // a pipe that keeps sending the same coordinate ends the program
                    if old == c {
                        return Ok(());
                    }
                    old = c;
                }
                let inv = inverse(&affmat)?;
                let v = solve(&vox2mm(&destvol), &coord)?;
                let n = newimage2newimage(&inv, &destvol, &srcvol, &v)?;
                if o.mm {
                    mv(&vox2mm(&srcvol), &n)
                } else {
                    solve(&srcvol.niftivox2newimagevox_mat(), &n)?
                }
            }
            Program::Img2Std => {
                let v = if o.mm {
                    solve(&vox2mm(&srcvol), &coord)?
                } else {
                    mv(&srcvol.niftivox2newimagevox_mat(), &coord)
                };
                let n = newimage2newimage(&affmat, &srcvol, &destvol, &v)?;
                mv(&vox2mm(&destvol), &n)
            }
        };
        out.num(r.get(0));
        out.s("  ");
        out.num(r.get(1));
        out.s("  ");
        out.num(r.get(2));
        out.s("\n");
    }
    Ok(())
}

/// `affmat.i()`: Armadillo's inverse, its error when singular; matrices other than 4 x 4 are
/// beyond the ported LAPACK sizes.
fn inverse(m: &Mat) -> Result<Mat, End> {
    if m.rows() != m.cols() {
        return Err(End::Terminate {
            printed: String::new(),
            type_name: "std::logic_error",
            what: "inv(): given matrix must be square sized".to_owned(),
        });
    }
    if m.rows() > 4 {
        return Err(End::Refuse(format!(
            "the inverse of a {} x {} matrix: FSL computes it with LAPACK routines beyond 4 x 4, \
             which the replica does not translate",
            m.rows(),
            m.cols()
        )));
    }
    arma::inv(m).map_err(|_| runtime(INV_ERROR))
}

/// The program with its arguments (without the program name).
pub fn run(
    p: Program,
    args: &[String],
    settings: &Settings,
    _mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut o = Cout::default();
    let mut e_text = String::new();
    let code = match body(p, args, settings, &mut o, &mut e_text) {
        Ok(()) => 0,
        Err(End::Exit(c)) => c,
        Err(End::Terminate {
            printed,
            type_name,
            what,
        }) => {
            e_text.push_str(&printed);
            e_text.push_str(&format!(
                "terminate called after throwing an instance of '{type_name}'\n  what():  {what}\n"
            ));
            e_text.push_str(&format!(
                "{PREFIX}: {} aborts here (signal 6); the replica exits with status 1\n",
                p.name()
            ));
            1
        }
        Err(End::Refuse(what)) => {
            e_text.push_str(&format!("{PREFIX}: error: {what}\n"));
            1
        }
    };
    let _ = out.write_all(o.text.as_bytes());
    let _ = err.write_all(e_text.as_bytes());
    let _ = out.flush();
    let _ = err.flush();
    code
}

/// `img2imgcoord [options] <filename containing coordinates>`.
pub fn img2imgcoord(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    run(Program::Img2Img, args, settings, mode, out, err)
}

/// `std2imgcoord [options] <filename containing coordinates>`.
pub fn std2imgcoord(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    run(Program::Std2Img, args, settings, mode, out, err)
}

/// `img2stdcoord [options] <filename containing coordinates>`.
pub fn img2stdcoord(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    run(Program::Img2Std, args, settings, mode, out, err)
}
