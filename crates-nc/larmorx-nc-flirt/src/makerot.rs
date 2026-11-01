// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-flirt 2111.4 (makerot.cc). Copyright (C) 2003-2023
// University of Oxford (Mark Jenkinson). Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the original. Distributed under the FSL Licence (see LICENSE).
//! `makerot`: the 4 x 4 matrix of a rotation by an angle in degrees about an axis through a
//! centre (or a volume's centre), written to a file or printed, with FSL's option parsing,
//! messages and exit statuses.
//!
//! Two defects of the original, each with its switch:
//! - FSL-76: the axis is divided by the square root of its length (`ax / sqrt(norm2(ax))`), so
//!   for an axis that is not of unit length the rotation is by `θ·√|a|`; corrected by default
//!   (the axis divided by its length, as the help text, "unnormalised axis vector", promises).
//! - FSL-77: an axis or a centre of fewer than three values reads past the vector's values (what
//!   its buffer still holds: the default's zeros, or an earlier value of the same option), and
//!   more than three are ignored; an error by default, FSL's reading with `reproduce`.

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_nc_core::arma::Mat;
use larmorx_nc_core::cnum;
use larmorx_nc_core::miscmaths;
use larmorx_nc_core::options::{ArgFlag, Opt, OptState, OptionParser, Value};
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::sse::S64;
use larmorx_nc_core::volume::{self, FslError};

use crate::run::PREFIX;

const TITLE: &str = "makerot \nCopyright(c) 2003, University of Oxford (Mark Jenkinson)";
const EXAMPLES: &str = "makerot [options] --theta=angle";

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

/// makerot's options, in the order it adds them.
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
                "-t,--theta",
                Value::Float(0.0),
                "angle of rotation (in degrees)",
                true,
                ArgFlag::Requires(1),
            ),
            opt(
                "-a,--axis",
                Value::FloatVec(vec![0.0; 3]),
                "~<ax,ay,az>\tunnormalised axis vector (comma separated)",
                false,
                ArgFlag::Requires(1),
            ),
            opt(
                "--cov",
                Value::Str(String::new()),
                "image filename used for centre of volume",
                false,
                ArgFlag::Requires(1),
            ),
            opt(
                "-c,--centre",
                Value::FloatVec(vec![0.0; 3]),
                "~<cx,cy,cz>\tcentre of rotation in mm (comma separated)",
                false,
                ArgFlag::Requires(1),
            ),
            opt(
                "-o,--out",
                Value::Str(String::new()),
                "output filename for matrix",
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

/// `cout << matrix` (armawrap's `doPrint`: fixed with 6 decimals, a blank after each value).
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

/// The three values `value()[0..3]` of a vector option that was set (FSL-77: fewer or more
/// than three refused by default; with `reproduce` read as libstdc++ reads them).
fn three(
    st: &OptState,
    what: &str,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) -> Result<[f64; 3], End> {
    let v = st.floats();
    if v.len() == 3 {
        return Ok([f64::from(v[0]), f64::from(v[1]), f64::from(v[2])]);
    }
    let given = v
        .iter()
        .map(|x| cnum::stream(f64::from(*x)))
        .collect::<Vec<_>>()
        .join(",");
    if mode.fix() {
        defects::push_once(
            notices,
            Notice::new(
                &defects::FSL_77,
                mode,
                format!("the {what} of {} values ({given}) is refused", v.len()),
            ),
        );
        return Err(End::Refuse(format!(
            "the {what} needs three values, not {} ({given}): makerot reads past the values it \
             was given, or ignores the rest (FSL-77)",
            v.len()
        )));
    }
    let b = st
        .floats_buffer(3)
        .expect("the default of three values is in the buffer");
    defects::push_once(
        notices,
        Notice::new(
            &defects::FSL_77,
            mode,
            format!(
                "the {what} has {} values ({given}): makerot reads {},{},{} (past the values, \
                 what the vector's buffer still holds)",
                v.len(),
                cnum::stream(f64::from(b[0])),
                cnum::stream(f64::from(b[1])),
                cnum::stream(f64::from(b[2]))
            ),
        ),
    );
    Ok([f64::from(b[0]), f64::from(b[1]), f64::from(b[2])])
}

fn body(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    out: &mut String,
    err: &mut String,
    notices: &mut Vec<Notice>,
) -> Result<(), End> {
    let parser = parser();
    let mut argv = vec!["makerot".to_owned()];
    argv.extend(args.iter().cloned());
    let parsed = match parser.parse_command_line(&argv, 0, false) {
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
    if let Some(t) = parser.check_compulsory(&parsed) {
        err.push_str(&t);
        err.push_str(&parser.usage());
        return Err(End::Exit(1));
    }
    // do_work
    let ax = if get("-a,--axis").set {
        three(get("-a,--axis"), "axis (-a)", mode, notices)?
    } else {
        [0.0, 0.0, 1.0]
    };
    let ax = Mat::col(&ax);
    let n = miscmaths::norm2(&ax);
    if n == 0.0 {
        err.push_str("ERROR:: Axis cannot be zero\n");
        return Err(End::Exit(1));
    }
    // FSL-76: ax / sqrt(norm2(ax)); corrected: ax / norm2(ax)
    let root = S64(n).sqrt().0;
    let theta = get("-t,--theta").float();
    let mut ctr = if get("-c,--centre").set {
        three(get("-c,--centre"), "centre (-c)", mode, notices)?
    } else {
        [0.0, 0.0, 0.0]
    };
    let cov = get("--cov");
    if cov.set {
        let (v1, warning) = volume::read_volume::<f32>(cov.str(), settings)?;
        if let Some(w) = warning {
            err.push_str(&w);
            err.push('\n');
        }
        ctr = [
            (v1.xsize() as f64 - 1.0) * f64::from(v1.xdim()) * 0.5,
            (v1.ysize() as f64 - 1.0) * f64::from(v1.ydim()) * 0.5,
            (v1.zsize() as f64 - 1.0) * f64::from(v1.zdim()) * 0.5,
        ];
    }
    // ax * theta * M_PI / 180.0, element by element in double
    let rotation = |by: f64| {
        let unit = ax.div_scalar(by);
        let angl: Vec<f64> = (0..3)
            .map(|k| ((unit.get(k) * f64::from(theta)) * std::f64::consts::PI) / 180.0)
            .collect();
        miscmaths::make_rot(&Mat::col(&angl), &Mat::col(&ctr))
    };
    let (fsl, corrected) = (rotation(root), rotation(n));
    let same =
        (0..4).all(|r| (0..4).all(|c| fsl.at(r, c).to_bits() == corrected.at(r, c).to_bits()));
    if !same {
        let detail = format!(
            "the axis is divided by the square root of its length ({}): the rotation is by {} \
             degrees instead of {}",
            cnum::stream(n),
            cnum::stream(f64::from(theta) * root),
            cnum::stream(f64::from(theta))
        );
        defects::push_once(notices, Notice::new(&defects::FSL_76, mode, detail));
    }
    let mat = if mode.fix() { corrected } else { fsl };
    let outname = get("-o,--out");
    if outname.set {
        if let Err(e) = crate::convert_xfm::write_matrix(&mat, outname.str()) {
            err.push_str(&e);
        }
    } else {
        out.push_str(&print_mat(&mat));
        out.push('\n');
    }
    Ok(())
}

/// `makerot` with its arguments (without the program name).
pub fn run(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let (mut o, mut e) = (String::new(), String::new());
    let mut notices = Vec::new();
    let code = match body(args, settings, mode, &mut o, &mut e, &mut notices) {
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
                "{PREFIX}: makerot aborts here (signal 6); the replica exits with status 1\n"
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
