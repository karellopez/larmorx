// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-flirt 2111.4 (midtrans.cc). Copyright (C) 2010-2023
// University of Oxford (Mark Jenkinson). Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the original. Distributed under the FSL Licence (see LICENSE).
//! `midtrans`: the transform to the "middle" of several affine transforms of images to one
//! template (FSL's Procrustes average: a rigid correction of the template pose, a rigid
//! approximation of each transform, then an affine correction, each by an SVD or a 3 × 3
//! inverse), written to a file or printed, optionally with each input composed with it; FSL's
//! option parsing, messages and exit statuses.
//!
//! Every matrix expression follows Armadillo 14.2.1's evaluation (fsl-armawrap 0.7.0):
//! `glue_times`' choice of operand order, `syrk` for a product of an object with its own
//! transpose, the BLAS products and LAPACK's `dgesdd` on FSL's OpenBLAS ([`arma`]).

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_nc_core::arma::{self, Mat, Singular};
use larmorx_nc_core::cnum;
use larmorx_nc_core::miscmaths::{self, MatrixFileError};
use larmorx_nc_core::options::{ArgFlag, Opt, OptionParser, Value};
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volume::{self, FslError};

use crate::run::PREFIX;

const TITLE: &str = "midtrans \nCopyright(c) 2010, University of Oxford (Mark Jenkinson)";
const EXAMPLES: &str = "midtrans [options] transform1 transform2 ... transformN\n  e.g. midtrans -o temp2mid.mat A2temp.mat B2temp.mat C2temp.mat\n       midtrans -o C2mid.mat A2C.mat B2C.mat ident.mat";

enum End {
    Exit(u8),
    /// An exception caught by `main` (`catch (std::exception& e)`): `what()` printed, exit 0.
    Caught(String),
    Terminate {
        printed: String,
        type_name: &'static str,
        what: String,
    },
    Refuse(String),
}

impl From<FslError> for End {
    fn from(e: FslError) -> End {
        if e.type_name.is_empty() {
            End::Terminate {
                printed: e.printed_text(),
                type_name: e.type_name,
                what: e.what,
            }
        } else {
            // newimage's exceptions are std::exceptions: main catches them.
            End::Caught(format!("{}{}", e.printed_text(), e.what))
        }
    }
}

fn parser() -> OptionParser {
    let opt = |key, default, help, flag| Opt {
        key,
        default,
        help,
        compulsory: false,
        flag,
        visible: true,
    };
    OptionParser::new(
        TITLE,
        EXAMPLES,
        vec![
            opt(
                "-o,--out",
                Value::Str(String::new()),
                "output filename for matrix",
                ArgFlag::Requires(1),
            ),
            opt(
                "--template",
                Value::Str(String::new()),
                "input filename for template image (needed for fix origin)",
                ArgFlag::Requires(1),
            ),
            opt(
                "--separate",
                Value::Str(String::new()),
                "basename for output of separate matrices (final name includes a number; e.g. img2mid0001.mat)",
                ArgFlag::Requires(1),
            ),
            opt(
                "--debug",
                Value::Bool(false),
                "switch on debugging output",
                ArgFlag::NoArgument,
            ),
            opt(
                "-v,--verbose",
                Value::Bool(false),
                "switch on diagnostic messages",
                ArgFlag::NoArgument,
            ),
            opt(
                "-h,--help",
                Value::Bool(false),
                "display this message",
                ArgFlag::NoArgument,
            ),
        ],
    )
}

/// `cout << matrix` (armawrap's `doPrint`: fixed with 6 decimals, a blank after each value).
pub(crate) fn print_mat(m: &Mat) -> String {
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

/// The rows `r0..r1` and columns `c0..c1` (a `SubMatrix`).
fn sub(m: &Mat, r0: usize, r1: usize, c0: usize, c1: usize) -> Mat {
    m.sub(r0, r1, c0, c1)
}

/// `a /= (float) NN`: each element divided by the count in single precision.
fn div_count(m: &mut Mat, nn: usize) {
    let d = f64::from(nn as f32);
    *m = m.div_scalar(d);
}

/// `k * M` for a scalar (element by element).
fn times(k: f64, m: &Mat) -> Mat {
    m.scale(k)
}

/// `A * x` for a 3 × 3 `A` and a column (`gemv_emul_tinysq`).
fn mv(a: &Mat, x: &Mat) -> Mat {
    arma::mul(a, x, false)
}

/// The SVD of armawrap's `SVD(A, D, U, V)`: Armadillo's `svd_econ` (LAPACK `dgesdd`).
fn svd(a: &Mat) -> Result<(Mat, Vec<f64>, Mat), End> {
    arma::svd_econ(a).ok_or_else(|| {
        End::Refuse(
            "the SVD of a non-finite matrix (Armadillo's svd_econ fails and FSL reads its empty \
             result)"
                .to_owned(),
        )
    })
}

struct Ctx<'a> {
    out: &'a mut String,
    verbose: bool,
    debug: bool,
}

fn avg_affine_with_procrustes(
    matlist: &[Mat],
    temp_orig: &[f64],
    rmax: f32,
    cx: &mut Ctx,
) -> Result<Mat, End> {
    let nn = matlist.len();
    if cx.verbose {
        cx.out.push_str("Initialising averages\n");
    }
    let v0001 = Mat::col(&[temp_orig[0], temp_orig[1], temp_orig[2], 1.0]);
    let mut xavlist = Vec::with_capacity(nn);
    for m44 in matlist {
        if m44.rows() != 4 || m44.cols() != 4 {
            // inv(M44) * v0001: Armadillo's size check, caught by main.
            return Err(End::Caught(format!(
                "matrix multiplication: incompatible matrix dimensions: {}x{} and 4x1",
                m44.rows(),
                m44.cols()
            )));
        }
        // M44.i() * v0001: Armadillo's solve (LAPACK dgesv).
        let x = arma::solve(m44, &v0001).map_err(|Singular| {
            End::Caught(
                "matrix multiplication: problem with matrix inverse; suggest to use solve() instead"
                    .to_owned(),
            )
        })?;
        xavlist.push(Mat::col(&[x.get(0), x.get(1), x.get(2)]));
    }
    let rr = f64::from((rmax * rmax) / 5.0);
    // Stage 1
    if cx.verbose {
        cx.out.push_str("Stage 1\n");
    }
    let mut x_av = Mat::zeros(3, 1);
    let mut t_av = Mat::zeros(3, 1);
    let mut tx_av = Mat::zeros(3, 3);
    let mut px_av = Mat::zeros(3, 1);
    let mut pxx_av = Mat::zeros(3, 3);
    for (n, m) in matlist.iter().enumerate() {
        let x_i = &xavlist[n];
        let p_i = sub(m, 0, 3, 0, 3);
        let t_i = sub(m, 0, 3, 3, 4);
        x_av = x_av.plus(x_i);
        t_av = t_av.plus(&t_i);
        tx_av = tx_av.plus(&arma::mul_tb(&t_i, x_i));
        px_av = px_av.plus(&mv(&p_i, x_i));
        let inner = times(rr, &Mat::eye(3)).plus(&arma::syrk(x_i));
        pxx_av = pxx_av.plus(&arma::mul(&p_i, &inner, false));
        if cx.debug {
            cx.out.push_str(&format!("P_{} = \n{}\n", n + 1, print_mat(&p_i)));
        }
    }
    div_count(&mut x_av, nn);
    div_count(&mut t_av, nn);
    div_count(&mut tx_av, nn);
    div_count(&mut px_av, nn);
    div_count(&mut pxx_av, nn);
    if cx.debug {
        cx.out.push_str(&format!("Pxx_av = \n{}\n", print_mat(&pxx_av)));
    }
    let m2 = pxx_av
        .plus(&tx_av)
        .minus(&arma::mul_tb(&px_av, &x_av))
        .minus(&arma::mul_tb(&t_av, &x_av));
    let (u, _, v) = svd(&m2)?;
    let a = arma::mul_tb(&v, &u);
    let s = x_av.minus(&mv(&a, &px_av.plus(&t_av)));
    if cx.verbose {
        cx.out.push_str(&format!(
            "A = \n{}\ns = {}\n",
            print_mat(&a),
            print_mat(&s.t())
        ));
    }
    // Stage 2
    if cx.verbose {
        cx.out.push_str("Stage 2\n");
    }
    let mut matlist2 = Vec::with_capacity(nn);
    for (n, m) in matlist.iter().enumerate() {
        let mut m44 = m.clone();
        let p = arma::mul(&a, &sub(m, 0, 3, 0, 3), false);
        let t = mv(&a, &sub(m, 0, 3, 3, 4)).plus(&s);
        let (u, _, v) = svd(&p.t())?;
        let r = arma::mul_tb(&v, &u);
        let uu = mv(&p.minus(&r), &xavlist[n]).plus(&t);
        m44.set_sub(0, 0, &r);
        m44.set_sub(0, 3, &uu);
        matlist2.push(m44);
        if cx.verbose {
            cx.out.push_str(&format!("Subject number {n}\n"));
            cx.out.push_str(&format!(
                "R = \n{}\nu = {}\n",
                print_mat(&r),
                print_mat(&uu.t())
            ));
        }
    }
    // Stage 3
    if cx.verbose {
        cx.out.push_str("Stage 3\n");
    }
    let z31 = || Mat::zeros(3, 1);
    let z33 = || Mat::zeros(3, 3);
    let (mut x_av, mut u_av, mut t_av, mut px_av, mut rx_av) = (z31(), z31(), z31(), z31(), z31());
    let (mut tt_av, mut ut_av, mut pxt_av, mut rxt_av, mut uxp_av) =
        (z33(), z33(), z33(), z33(), z33());
    let (mut pp_av, mut rp_av, mut pxxp, mut rxxp) = (z33(), z33(), z33(), z33());
    for (n, m) in matlist.iter().enumerate() {
        let u_i = sub(&matlist2[n], 0, 3, 3, 4);
        let x_i = &xavlist[n];
        x_av = x_av.plus(x_i);
        u_av = u_av.plus(&u_i);
        let p_i = arma::mul(&a, &sub(m, 0, 3, 0, 3), false);
        let t_i = mv(&a, &sub(m, 0, 3, 3, 4)).plus(&s);
        t_av = t_av.plus(&t_i);
        tt_av = tt_av.plus(&arma::syrk(&t_i));
        ut_av = ut_av.plus(&arma::mul_tb(&u_i, &t_i));
        let px = mv(&p_i, x_i);
        px_av = px_av.plus(&px);
        let r_i = sub(&matlist2[n], 0, 3, 0, 3);
        let rx = mv(&r_i, x_i);
        rx_av = rx_av.plus(&rx);
        // P_i * x_i * t_i.t(): (P x) tᵀ
        pxt_av = pxt_av.plus(&arma::mul_tb(&px, &t_i));
        rxt_av = rxt_av.plus(&arma::mul_tb(&rx, &t_i));
        // uxP_av += u_i * x_i.t() * P_i.t(): `+=` evaluates the left product first
        // (`apply_inplace_plus` unwraps its left operand): (u xᵀ) Pᵀ.
        uxp_av = uxp_av.plus(&arma::mul_tb(&arma::mul_tb(&u_i, x_i), &p_i));
        pp_av = pp_av.plus(&arma::syrk(&p_i));
        rp_av = rp_av.plus(&arma::mul_tb(&r_i, &p_i));
        // P x xᵀ Pᵀ: ((P x) xᵀ) Pᵀ
        pxxp = pxxp.plus(&arma::mul_tb(&arma::mul_tb(&px, x_i), &p_i));
        rxxp = rxxp.plus(&arma::mul_tb(&arma::mul_tb(&rx, x_i), &p_i));
    }
    for m in [
        &mut x_av,
        &mut u_av,
        &mut t_av,
        &mut tt_av,
        &mut ut_av,
        &mut px_av,
        &mut rx_av,
        &mut pxt_av,
        &mut rxt_av,
        &mut uxp_av,
        &mut pp_av,
        &mut rp_av,
    ] {
        div_count(m, nn);
    }
    let d = f64::from(nn as f32);
    let pxxp = times(rr, &pp_av).plus(&pxxp.div_scalar(d));
    let rxxp = times(rr, &rp_av).plus(&rxxp.div_scalar(d));
    let aat = pxxp
        .minus(&arma::syrk(&px_av))
        .plus(&pxt_av)
        .plus(&pxt_av.t())
        .minus(&arma::mul_tb(&t_av, &px_av))
        .minus(&arma::mul_tb(&px_av, &t_av))
        .plus(&tt_av)
        .minus(&arma::syrk(&t_av));
    let inner = rxxp
        .minus(&arma::mul_tb(&rx_av, &px_av))
        .plus(&rxt_av)
        .minus(&arma::mul_tb(&rx_av, &t_av))
        .minus(&uxp_av)
        .plus(&arma::mul_tb(&u_av, &px_av))
        .minus(&ut_av)
        .plus(&arma::mul_tb(&u_av, &t_av));
    let ba = times(-1.0, &inner);
    // -ba * inv(aat): the inverse evaluated (Armadillo's inv), then the product with alpha -1.
    let inv = arma::inv(&aat).map_err(|Singular| {
        End::Caught("inv(): matrix is singular".to_owned())
    })?;
    let mm = times(-1.0, &arma::mul(&ba, &inv, false));
    // -M*Px_av - M*t_av + Rx_av + u_av
    let w = times(-1.0, &mv(&mm, &px_av))
        .minus(&mv(&mm, &t_av))
        .plus(&rx_av)
        .plus(&u_av);
    if cx.verbose {
        cx.out.push_str(&format!(
            "M = \n{}\nw = {}\n",
            print_mat(&mm),
            print_mat(&w.t())
        ));
        cx.out.push_str("Final calculations\n");
    }
    let mut mid = Mat::eye(4);
    mid.set_sub(0, 0, &arma::mul(&mm, &a, false));
    mid.set_sub(0, 3, &w.plus(&mv(&mm, &s)));
    Ok(mid)
}

fn body(
    args: &[String],
    settings: &Settings,
    out: &mut String,
    err: &mut String,
) -> Result<(), End> {
    let parser = parser();
    let mut argv = vec!["midtrans".to_owned()];
    argv.extend(args.iter().cloned());
    let parsed = match parser.parse_command_line(&argv, 0, true) {
        Ok(p) => p,
        Err((_, what)) => {
            err.push_str(&parser.usage());
            err.push_str(&format!("\n{what}\n"));
            return Err(End::Exit(1));
        }
    };
    let get = |k: &str| parsed.get(&parser, k);
    if get("-h,--help").bool() {
        err.push_str(&parser.usage());
        return Err(End::Exit(1));
    }
    let nonoptarg = parsed.stop;
    if argv.len() < nonoptarg + 2 {
        err.push_str("Must specify at least 2 transforms to find the mid-transform\n");
        err.push_str(&parser.usage());
        return Err(End::Exit(1));
    }
    let verbose = get("-v,--verbose").bool();
    let debug = get("--debug").bool();
    // do_work
    let mut matlist = Vec::new();
    for name in &argv[nonoptarg..] {
        let trans = match miscmaths::read_ascii_matrix(name) {
            Ok(m) => m,
            Err(MatrixFileError::CannotOpen) => {
                err.push_str(&format!("Could not open matrix file {name}\n"));
                Mat::zeros(0, 0)
            }
            Err(MatrixFileError::NotEnoughNumbers) => {
                return Err(End::Terminate {
                    printed: String::new(),
                    type_name: "armawrap::AWException",
                    what: "Could not find enough numbers in matrix file".to_owned(),
                });
            }
        };
        // Determinant(): an empty matrix (a file that cannot be read) has 1; a matrix that is
        // not square throws (caught by main); beyond 4 x 4 LAPACK's LU is not translated.
        let det = if trans.is_empty() {
            1.0
        } else if trans.rows() != trans.cols() {
            return Err(End::Caught("det(): given matrix must be square sized".to_owned()));
        } else if trans.rows() > 4 {
            return Err(End::Refuse(format!(
                "the matrix {name} is {}x{}: FSL's determinant of a matrix beyond 4 x 4 (LAPACK's \
                 LU) is not translated",
                trans.rows(),
                trans.cols()
            )));
        } else {
            arma::det(&trans)
        };
        if det.abs() < 1e-6 {
            err.push_str(&format!("Could not read matrix {name}\n"));
            return Err(End::Exit(1));
        }
        matlist.push(trans);
    }
    let nn = matlist.len();
    if verbose {
        out.push_str(&format!("Number of matrices = {nn}\n"));
    }
    let mut temp_orig = vec![0.0f64; 4];
    let templte = get("--template");
    if templte.set {
        let (templ, warning) = volume::read_volume::<f32>(templte.str(), settings)?;
        if let Some(w) = warning {
            err.push_str(&w);
            err.push('\n');
        }
        let mut w = Vec::new();
        let c = templ.cog_scaled_mm(&mut w);
        for l in &w {
            err.push_str(l);
            err.push('\n');
        }
        temp_orig = c.to_vec();
    } else {
        out.push_str("Assuming MNI origin as no template image specified\n");
        temp_orig[0] = 90.0;
        temp_orig[1] = 110.0;
        temp_orig[2] = 90.0;
        if debug {
            temp_orig[0] = 0.0;
            temp_orig[1] = 0.0;
            temp_orig[2] = 0.0;
        }
    }
    if verbose {
        out.push_str(&format!(
            "Centre of template = {}\n",
            print_mat(&Mat::col(&temp_orig).t())
        ));
    }
    let mut cx = Ctx {
        out,
        verbose,
        debug,
    };
    let mid = avg_affine_with_procrustes(&matlist, &temp_orig, 80.0, &mut cx)?;
    let outname = get("-o,--out");
    if outname.set {
        if let Err(e) = crate::convert_xfm::write_matrix(&mid, outname.str()) {
            err.push_str(&e);
        }
    }
    if verbose || !outname.set {
        cx.out.push_str(&print_mat(&mid));
        cx.out.push('\n');
    }
    let sep = get("--separate");
    if sep.set {
        for (n, m) in matlist.iter().enumerate() {
            let name = format!("{}{:04}.mat", sep.str(), n + 1);
            if let Err(e) = crate::convert_xfm::write_matrix(&arma::mul(&mid, m, false), &name) {
                err.push_str(&e);
            }
        }
    }
    Ok(())
}

/// `midtrans` with its arguments (without the program name).
pub fn run(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let (mut o, mut e) = (String::new(), String::new());
    let mut notices: Vec<Notice> = Vec::new();
    let code = match body(args, settings, &mut o, &mut e) {
        Ok(()) => 0,
        Err(End::Exit(c)) => c,
        Err(End::Caught(what)) => {
            // FSL-78 (rule B): main catches the exception and returns 0; an error by default.
            e.push_str(&what);
            e.push('\n');
            let detail = if mode.fix() {
                "the run failed: exit status 1 (midtrans prints the message and exits with status 0)"
            } else {
                "the run failed and exits with status 0, as midtrans does"
            };
            notices.push(Notice::new(&defects::FSL_78, mode, detail.to_owned()));
            u8::from(mode.fix())
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
                "{PREFIX}: midtrans aborts here (signal 6); the replica exits with status 1\n"
            ));
            1
        }
        Err(End::Refuse(what)) => {
            e.push_str(&format!("{PREFIX}: error: {what}\n"));
            1
        }
    };
    for n in &notices {
        e.push_str(&format!("{PREFIX}: {}\n", n.line()));
    }
    let _ = out.write_all(o.as_bytes());
    let _ = err.write_all(e.as_bytes());
    let _ = out.flush();
    let _ = err.flush();
    code
}
