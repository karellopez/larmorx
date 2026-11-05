// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl img2imgcoord`, `std2imgcoord` and `img2stdcoord`: FSL's coordinate programs
//! (`specs/flirt.md` §15.2), coordinates mapped between images through a FLIRT matrix.

use std::io::{IsTerminal, Read, Write};

use larmorx_core::defects::{Notice, UpstreamDefects};

use super::Settings;
use super::common::{self, HeaderImage, abort, print_notices};
use crate::flirt::mat::{self, IDENTITY, Mat4, Stream};
use crate::flirt::programs::{self, Space};

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

    /// The usage text (observed; FSL names the program as invoked).
    pub fn usage(self) -> String {
        let p = self.name();
        match self {
            Program::Img2Img => format!(
                "Usage: {p} [options] <filename containing coordinates>\n\n  Options are:\n        -src <filename of source image>        \n        -dest <filename of destination image>  \n        -xfm <filename of affine transform     (e.g. source2dest.mat)>\n        -warp <filename of warpfield (e.g. intermediate2dest_warp.nii.gz)>\n        -premat <filename of pre-warp affine transform  (e.g. source2intermediate.mat)>   (default=identity)\n        -vox                                   (all coordinates in voxels - default)\n        -mm                                    (all coordinates in mm)\n        -v                                     (verbose)\n        -help\n\n Notes:\n  (1) if '-' is used as coordinate filename then coordinates are read from standard input\n  (2) the first three options are compulsory\n"
            ),
            Program::Std2Img => format!(
                "Usage: {p} [options] <filename containing coordinates>\n\ne.g.   {p} -img <invol> -std <standard image> -xfm <img2standard mat file> <coordinate file>\n       {p} -img <invol> -std <standard image> <coordinate file>\n       {p} -img <invol> -std <standard image> - \n\n  Options are:\n        -std <filename of standard image>\n        -img <filename of input image>\n        -xfm <filename of affine transform   (e.g. example_func2standard.mat)>\n        -warp <filename of warpfield (e.g. highres2standard_warp.nii.gz)>\n        -premat <filename of pre-warp affine transform  (e.g. example_func2highres.mat)>   (default=identity)\n        -mm                                  (outputs coordinates in mm - default)\n        -vox                                 (outputs coordinates in voxels)\n        -v                                   (verbose output)\n        -verbose                             (more verbose output)\n        -help\n\n Notes:\n  (1) if '-' is used as coordinate filename then coordinates are read from standard input\n  (2) the -img option is compulsory\n"
            ),
            Program::Img2Std => format!(
                "Usage: {p} [options] <filename containing coordinates>\ne.g.   {p} -img <invol> -std <standard image> -xfm <img2standard mat file> <coordinate file>\n       {p} -img <invol> <coordinate file>\n       {p} -img <invol> - \n\n  Options are:\n        -img <filename of input image>\n        -std <filename of standard image>\n        -xfm <filename of affine transform   (e.g. example_func2standard.mat)>\n        -warp <filename of warpfield (e.g. highres2standard_warp.nii.gz)>\n        -premat <filename of pre-warp affine transform  (e.g. example_func2highres.mat)>   (default=identity)\n        -vox                                 (input coordinates in voxels - default)\n        -mm                                  (input coordinates in mm)\n        -v                                   (verbose output)\n        -verbose                             (more verbose output)\n        -help\n\n Notes:\n  (1) if '-' is used as coordinate filename then coordinates are read from standard input\n  (2) the -img option is compulsory\n"
            ),
        }
    }
}

/// The options of one run.
#[derive(Clone, Debug, Default)]
struct Options {
    /// `-src` / `-img`.
    source: Option<String>,
    /// `-dest` / `-std`.
    target: Option<String>,
    matrix: Option<String>,
    warp: Option<String>,
    /// `Some(true)` with `-mm`, `Some(false)` with `-vox`.
    mm: Option<bool>,
    verbose: i32,
    coords: String,
}

enum Scan {
    Run(Options),
    Exit(u8),
}

fn scan(p: Program, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Scan {
    let mut o = Options::default();
    let mut i = 0;
    while i < args.len() {
        let w = args[i].as_str();
        i += 1;
        if w.is_empty() {
            continue;
        }
        if !w.starts_with('-') {
            o.coords = w.to_owned();
            continue;
        }
        match (w, p) {
            ("-help", _) => {
                let _ = write!(out, "{}", p.usage());
                return Scan::Exit(0);
            }
            ("-vox", _) => o.mm = Some(false),
            ("-mm", _) => o.mm = Some(true),
            ("-v", Program::Img2Img) => o.verbose = 5,
            ("-v", _) => o.verbose = 1,
            ("-verbose", Program::Std2Img | Program::Img2Std) => o.verbose = 5,
            ("-flirt", Program::Img2Img) => {}
            ("-flirt", _) => {
                let _ = writeln!(
                    err,
                    "WARNING::Using outdated options, please update to new usage"
                );
            }
            ("-", Program::Std2Img | Program::Img2Std) => o.coords = "-".into(),
            _ => {
                let Some(v) = args.get(i) else {
                    let _ = writeln!(err, "Lacking argument to option {w}");
                    break;
                };
                match (w, p) {
                    ("-src", Program::Img2Img) | ("-img", Program::Std2Img | Program::Img2Std) => {
                        o.source = Some(v.clone())
                    }
                    ("-dest", Program::Img2Img) | ("-std", Program::Std2Img | Program::Img2Std) => {
                        o.target = Some(v.clone())
                    }
                    ("-xfm" | "-premat", _) => o.matrix = Some(v.clone()),
                    ("-warp", _) => o.warp = Some(v.clone()),
                    _ => {
                        let _ = writeln!(err, "Unrecognised option {w}");
                        return Scan::Exit(255);
                    }
                }
                i += 1;
            }
        }
    }
    Scan::Run(o)
}

/// A C++ stream reading doubles from text (`std::istream >> double`, the "C" locale): blanks
/// skipped, then the characters a number can hold (a sign, digits, one point, an exponent with
/// its sign) taken and converted whole; nothing taken, a conversion that does not use them all,
/// or a value out of range fails the stream for good.
struct Numbers<'a> {
    text: &'a [u8],
    at: usize,
    failed: bool,
}

impl<'a> Numbers<'a> {
    fn new(text: &'a [u8]) -> Numbers<'a> {
        Numbers {
            text,
            at: 0,
            failed: false,
        }
    }

    fn next(&mut self) -> Option<f64> {
        if self.failed {
            return None;
        }
        let b = self.text;
        let mut i = self.at;
        while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r') {
            i += 1;
        }
        let start = i;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i < b.len() && b[i] == b'.' {
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
        }
        if i > start && i < b.len() && (b[i] == b'e' || b[i] == b'E') {
            i += 1;
            if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
                i += 1;
            }
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
        }
        self.at = i;
        let v = std::str::from_utf8(&b[start..i])
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|v| v.is_finite());
        if v.is_none() {
            self.failed = true;
        }
        v
    }

    fn triple(&mut self) -> Option<[f64; 3]> {
        Some([self.next()?, self.next()?, self.next()?])
    }
}

fn space(h: &HeaderImage) -> Option<Space> {
    Space::of_image(h.shape, h.voxel, h.form, h.neurological)
}

/// The coordinate program `p` with FSL's arguments; returns the exit status.
pub fn main(
    p: Program,
    args: &[String],
    settings: &Settings,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl {}: {e}", p.name());
            return 2;
        }
    };
    if args.is_empty() {
        let _ = write!(out, "{}", p.usage());
        return 1;
    }
    let o = match scan(p, args, out, err) {
        Scan::Run(o) => o,
        Scan::Exit(c) => return c,
    };
    let mut notices = Vec::new();
    let code = run(p, &o, settings, mode, &mut notices, out, err);
    print_notices(err, &notices);
    code
}

fn run(
    p: Program,
    o: &Options,
    settings: &Settings,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let program = p.name();
    let (source_word, target_word) = match p {
        Program::Img2Img => ("Source volume", "Destination volume"),
        Program::Std2Img => ("image", "standard image"),
        Program::Img2Std => ("input image", "standard image"),
    };
    let source = o.source.clone().unwrap_or_default();
    if source.is_empty() {
        let _ = writeln!(err, "ERROR:: {source_word} filename not found\n");
    }
    // img2imgcoord needs its destination; the others' standard image is optional
    let target = match p {
        Program::Img2Img => Some(o.target.clone().unwrap_or_default()),
        _ => o.target.clone(),
    };
    if target.as_deref() == Some("") {
        let _ = writeln!(err, "ERROR:: {target_word} filename not found\n");
    }
    let read = |name: &str, notices: &mut Vec<Notice>| {
        common::read_header(settings, name, mode, notices)
            .map_err(|e| common::image_exception(name, &e))
    };
    let failed = |err: &mut dyn Write, (lines, what): (String, String)| {
        let _ = write!(err, "{lines}");
        abort(err, program, "std::runtime_error", &what)
    };
    // img2imgcoord reads its source first, the others their input image first
    let src = match read(&source, notices) {
        Ok(h) => h,
        Err(e) => return failed(err, e),
    };
    let dst = match &target {
        Some(t) => match read(t, notices) {
            Ok(h) => Some(h),
            Err(e) => return failed(err, e),
        },
        None => None,
    };
    let mut s = Stream::default();
    if o.verbose > 3 {
        let mut text = String::new();
        let mut show = |label: &str, h: &HeaderImage| {
            let g = |v: f32| s.num(f64::from(v));
            text += &format!(
                "{label}:: SIZE = {} x {} x {}\n{label}:: DIMS = {} x {} x {} mm\n\n",
                h.shape[0],
                h.shape[1],
                h.shape[2],
                g(h.voxel[0]),
                g(h.voxel[1]),
                g(h.voxel[2])
            );
        };
        let (dl, sl) = match p {
            Program::Img2Img => ("Destination Volume", "Source Volume"),
            _ => ("standard image", "input image"),
        };
        if let Some(d) = &dst {
            show(dl, d);
        }
        show(sl, &src);
        let _ = write!(out, "{text}");
    }
    // the matrix: 4 x 4, or (img2imgcoord, img2stdcoord) another size their mapping refuses
    let (m, other): (Mat4, Option<Vec<Vec<f64>>>) = match &o.matrix {
        None => (IDENTITY, None),
        Some(name) => {
            let rows = match common::read_rows(settings, name, err) {
                Ok(r) => r,
                Err(m) => return abort(err, program, "std::runtime_error", &m),
            };
            if rows.len() < 4 {
                let _ = writeln!(err, "Cannot read transform file");
                return 254;
            }
            match common::exact4(&rows) {
                Some(m) => (m, None),
                None if p == Program::Std2Img => {
                    let (r, c) = common::size(&rows);
                    return common::refuse(
                        err,
                        program,
                        &format!(
                            "{name} holds a {r} x {c} matrix; std2imgcoord inverts it as a 4 x 4 one (FSL aborts at the inverse)"
                        ),
                    );
                }
                None => (IDENTITY, Some(rows)),
            }
        }
    };
    if o.verbose > 3 {
        let text = match &other {
            Some(rows) => {
                s.fixed = true;
                rows.iter()
                    .map(|r| {
                        r.iter()
                            .map(|&v| format!("{} ", mat::f6(v)))
                            .collect::<String>()
                            + "\n"
                    })
                    .collect()
            }
            None => s.matrix(&m),
        };
        let _ = write!(out, " affmat =\n{text}\n\n");
        if p == Program::Std2Img {
            match mat::inverse(&m) {
                Some(mi) => {
                    let _ = write!(out, " Inverse affmat =\n{}\n\n", arma_text(&mi));
                }
                None => return abort(err, program, "std::runtime_error", mat::SINGULAR),
            }
        }
    }
    if let Some(w) = &o.warp {
        // FSL reads the warp as a 4D image and takes exactly three volumes (a FNIRT coefficient
        // file or a displacement field, §15.2): any other image is its read error, exit 1
        let volumes = common::read_header_volumes(settings, w, mode, notices)
            .map_err(|e| common::image_exception(w, &e));
        return match volumes {
            Err((lines, _)) => {
                let _ = writeln!(err, "{lines}An error occured while reading file: {w}");
                1
            }
            Ok(n) if n != 3 => {
                let _ = writeln!(err, "An error occured while reading file: {w}");
                1
            }
            Ok(_) => common::refuse(
                err,
                program,
                "-warp (FNIRT's warp fields) is not in larmorx yet: FSL's warp-field libraries are still to be ported; refused",
            ),
        };
    }
    let unset_warning = || {
        if p == Program::Std2Img {
            "WARNING:: standard coordinates not set in standard image"
        } else {
            "WARNING:: mm coordinate space not set in standard volume"
        }
    };
    let target_space = match (p, &dst) {
        // a destination or standard image without forms: its voxel sizes (observed; the
        // standard programs warn as for no standard image)
        (_, Some(h)) if h.form.is_none() => {
            if p != Program::Img2Img {
                let _ = writeln!(err, "{}", unset_warning());
            }
            Some(Space::unset(h.voxel))
        }
        (_, Some(h)) => space(h),
        (Program::Img2Img, None) => None,
        (_, None) => {
            let _ = writeln!(
                err,
                "{}",
                if p == Program::Std2Img {
                    "WARNING:: standard coordinates not set in standard image"
                } else {
                    "WARNING:: mm coordinate space not set in standard volume"
                }
            );
            Some(Space::empty())
        }
    };
    let (Some(src_space), Some(dst_space)) = (space(&src), target_space) else {
        let _ = writeln!(err, "{}", programs::INVERSE_PROBLEM);
        return abort(
            err,
            program,
            "std::runtime_error",
            programs::INVERSE_PROBLEM,
        );
    };
    if p != Program::Img2Img && o.verbose > 3 {
        let _ = write!(out, " stdvox2world =\n{}\n\n", s.matrix(&dst_space.v));
    }
    let mm = o.mm.unwrap_or(p == Program::Std2Img);
    let unit = if mm { "mm" } else { "voxels" };
    match p {
        Program::Img2Img => {
            let _ = writeln!(out, "Coordinates in Destination volume (in {unit})");
        }
        Program::Std2Img if o.verbose > 0 => {
            let _ = writeln!(out, "Coordinates in input image (in {unit}):");
        }
        Program::Img2Std if o.verbose > 0 => {
            let _ = writeln!(out, "Coordinates in standard image (in mm):");
        }
        _ => {}
    }
    let stdin = o.coords.is_empty() || o.coords == "-";
    let text = if stdin {
        if o.verbose > 0 {
            let prompt = match p {
                Program::Std2Img => "Please type in standard coordinates :".to_owned(),
                _ => format!("Please type in input image coordinates (in {unit}) :"),
            };
            let _ = writeln!(out, "{prompt}");
        }
        let terminal = settings.stdin.is_none() && std::io::stdin().is_terminal();
        if terminal && p != Program::Std2Img {
            let _ = writeln!(
                err,
                "Interactive mode has been removed. Please pipe input such as: cat coordinate_file.txt | {program}, OR supply a coordinate file"
            );
            return 1;
        }
        match &settings.stdin {
            Some(t) => t.clone(),
            None => {
                let mut t = Vec::new();
                let _ = std::io::stdin().read_to_end(&mut t);
                t
            }
        }
    } else {
        match std::fs::read(settings.path(&o.coords)) {
            Ok(t) => t,
            Err(_) => {
                let _ = writeln!(err, "Could not open matrix file {}", o.coords);
                return 255;
            }
        }
    };
    let mut numbers = Numbers::new(&text);
    let mut previous: Option<[f64; 3]> = None;
    while let Some(c) = numbers.triple() {
        if p == Program::Std2Img && stdin && previous == Some(c) {
            break;
        }
        previous = Some(c);
        if other.is_some() {
            let what = "NewimageCoord2NewimageCoord: M must be a 4x4 matrix";
            let _ = writeln!(err, "Image Exception : #11 :: {what}");
            return abort(err, program, "std::runtime_error", what);
        }
        let r = match p {
            Program::Img2Img => programs::img2img(c, &m, &src_space, &dst_space, mm),
            Program::Img2Std => programs::img2std(c, &m, &src_space, &dst_space, mm),
            Program::Std2Img => match programs::std2img(c, &m, &src_space, &dst_space, !mm) {
                Ok(r) => r,
                Err(what) => return abort(err, program, "std::runtime_error", what),
            },
        };
        let Some(r) = r else {
            let _ = writeln!(err, "{}", programs::INVERSE_PROBLEM);
            return abort(
                err,
                program,
                "std::runtime_error",
                programs::INVERSE_PROBLEM,
            );
        };
        let _ = writeln!(out, "{}  {}  {}", s.num(r[0]), s.num(r[1]), s.num(r[2]));
    }
    0
}

/// A matrix as FSL's linear-algebra library prints an expression (§15): 4 decimals, right-aligned
/// in 9 columns, 10 when a value is at least 10 in magnitude, 13 in scientific notation when a
/// finite value is at least 100 in magnitude or non-zero and at most 1e-4; zeros as `0`.
pub fn arma_text(m: &Mat4) -> String {
    let finite = m.iter().flatten().filter(|v| v.is_finite());
    let scientific = finite
        .clone()
        .any(|&v| v.abs() >= 100.0 || (v != 0.0 && v.abs() <= 1e-4));
    let wide = finite.clone().any(|&v| v.abs() >= 10.0);
    let width = if scientific {
        13
    } else if wide {
        10
    } else {
        9
    };
    let mut s = String::new();
    for row in m {
        for &v in row {
            let cell = if v == 0.0 {
                "0".to_owned()
            } else if v.is_nan() {
                "nan".to_owned()
            } else if v.is_infinite() {
                if v > 0.0 { "inf" } else { "-inf" }.to_owned()
            } else if scientific {
                let t = format!("{v:.4e}");
                let (mant, exp) = t.split_once('e').unwrap_or((&t, "0"));
                let e: i32 = exp.parse().unwrap_or(0);
                format!("{mant}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
            } else {
                format!("{v:.4}")
            };
            s.push_str(&format!("{cell:>width$}"));
        }
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_as_a_stream_reads_them() {
        let mut n = Numbers::new(b"1 2 3 4 5 6 7 8 9\n10 11\n");
        assert_eq!(n.triple(), Some([1.0, 2.0, 3.0]));
        assert_eq!(n.triple(), Some([4.0, 5.0, 6.0]));
        assert_eq!(n.triple(), Some([7.0, 8.0, 9.0]));
        assert_eq!(n.triple(), None);
        for bad in [
            "1e400 2 3",
            "nan 2 3",
            "inf 1 1",
            "1 2 x",
            "1e 2 3",
            ". 1 2",
        ] {
            assert_eq!(Numbers::new(bad.as_bytes()).triple(), None, "{bad}");
        }
        let mut n = Numbers::new(b"5. .5 -1e2 0x10");
        assert_eq!(n.triple(), Some([5.0, 0.5, -100.0]));
        assert_eq!(n.next(), Some(0.0));
        assert_eq!(n.next(), None);
    }

    #[test]
    fn expressions_as_the_library_prints_them() {
        let m: Mat4 = [
            [0.99974, -0.006016, 0.022059, 12.8102],
            [0.0076, 0.997343, -0.072449, -5.976],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        assert_eq!(
            arma_text(&m).lines().next(),
            Some("    0.9997   -0.0060    0.0221   12.8102")
        );
        assert_eq!(
            arma_text(&IDENTITY).lines().next(),
            Some("   1.0000        0        0        0")
        );
        let mut big = IDENTITY;
        big[0][3] = 123.456;
        assert_eq!(
            arma_text(&big).lines().next(),
            Some("   1.0000e+00            0            0   1.2346e+02")
        );
    }

    #[test]
    fn the_scan() {
        let words = |w: &[&str]| w.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let r = scan(
            Program::Img2Img,
            &words(&["-src", "a", "-dest", "b", "-frob", "x", "c.txt"]),
            &mut out,
            &mut err,
        );
        assert!(matches!(r, Scan::Exit(255)));
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "Unrecognised option -frob\n"
        );
        let mut err = Vec::new();
        let r = scan(
            Program::Img2Img,
            &words(&["-src", "a", "-"]),
            &mut out,
            &mut err,
        );
        assert!(matches!(r, Scan::Run(_)));
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "Lacking argument to option -\n"
        );
    }

    /// `-warp` (§15.2): FSL takes exactly three volumes; any other image is its read error, exit
    /// 1, after the verbose lines (observed: a 3D image, 1, 2 and 4 volumes); three volumes are
    /// refused until FNIRT's warp fields are ported.
    #[test]
    fn warp_volumes() {
        use larmorx_core::element::DataType;
        use larmorx_io::nifti::{NiftiHeader, NiftiVersion};
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str, shape: &[usize]| {
            let h = NiftiHeader::new(NiftiVersion::V1, shape, DataType::F32).unwrap();
            let mut bytes = h.to_bytes();
            bytes.resize(352 + 4 * shape.iter().product::<usize>(), 0);
            std::fs::write(dir.path().join(name), bytes).unwrap();
        };
        write("a.nii", &[5, 4, 3]);
        write("w3d.nii", &[5, 4, 3]);
        write("w1.nii", &[5, 4, 3, 1]);
        write("w4.nii", &[5, 4, 3, 4]);
        write("w3.nii", &[5, 4, 3, 3]);
        std::fs::write(dir.path().join("c.txt"), "1 2 3\n").unwrap();
        let settings = Settings {
            dir: Some(dir.path().to_path_buf()),
            ..Settings::default()
        };
        let run = |warp: &str| {
            let args: Vec<String> = [
                "-src", "a.nii", "-dest", "a.nii", "-warp", warp, "-v", "c.txt",
            ]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let c = main(Program::Img2Img, &args, &settings, &mut out, &mut err);
            (
                c,
                String::from_utf8(out).unwrap(),
                String::from_utf8(err).unwrap(),
            )
        };
        for w in ["w3d.nii", "w1.nii", "w4.nii"] {
            let (c, out, err) = run(w);
            assert_eq!(
                (c, err),
                (1, format!("An error occured while reading file: {w}\n"))
            );
            assert!(
                out.starts_with("Destination Volume:: SIZE = 5 x 4 x 3\n"),
                "{out}"
            );
            assert!(!out.contains("Coordinates in"), "{out}");
        }
        let (c, _, err) = run("w3.nii");
        assert_eq!(c, 1);
        assert!(err.contains("not in larmorx yet"), "{err}");
    }
}
