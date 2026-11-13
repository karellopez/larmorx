// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Bruker ParaVision studies as FreeSurfer 8.2.0 reads them (`utils/Bruker.cpp`, queue item
//! 3.1-27): the format probe (`checkBrukerFiles`, `is_bruker`) and the readers (`brukerRead`,
//! `splitParameterValue`, `readBrukerD3proc`, `readBrukerReco`, `readBrukerAcqp`,
//! `buildVoxToRASTransform`, `readBrukerVolume`).
//!
//! Defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-io.md`, Part 7):
//! FS-259 (a parameter line's long name or value), FS-260 (files left open: both modes close
//! them, no notice), FS-401 (values never written), FS-402 (2 bytes per voxel), FS-403 (the
//! volume freed twice), FS-404 (`$RECO_byte_order` ignored).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_221, FS_259, FS_401, FS_402, FS_403, FS_404};
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::sse::{div64, mul64};
use larmorx_freesurfer_mri::mri::{self, MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, Mri};

use crate::cfile::CFile;
use crate::identify::c_stat;

/// A study file's part of the name, its message, and where its name goes.
type Part = (
    &'static str,
    &'static str,
    fn(&mut BrukerFiles) -> &mut String,
);

/// The bytes of each file name `is_bruker` builds (`char methodFile[512]` and the others).
const BRUKER_NAME: usize = 512;

/// The five files of a Bruker study directory, as `checkBrukerFiles` names them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BrukerFiles {
    pub method: String,
    pub acqp: String,
    pub data: String,
    pub d3proc: String,
    pub reco: String,
}

/// `checkBrukerFiles(fname, ...)`: 1 when `fname` is a directory holding `method`, `acqp`,
/// `pdata/1/2dseq`, `pdata/1/d3proc` and `pdata/1/reco` (each found by `stat`, in that order); else
/// 0 at the first missing one (`errno` as `stat` left it). With `flag` set it reports each step:
/// `ERROR: could not stat <f>.` or `ERROR: <f> is not a directory.` (standard error), `INFO:
/// directory <f> ` and the found files' lines (standard output; the reco file's line reads
/// `d3proc`, as FreeSurfer prints it). The names are built as `fname` and the file's part; one of
/// 512 bytes or more overflows FreeSurfer's buffer (FS-221: used whole).
pub fn check_bruker_files(
    g: &mut Globals,
    dir: Option<&Path>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    fname: &str,
    files: &mut BrukerFiles,
    flag: bool,
) -> i32 {
    check_files(g, dir, out, err, fname, files, flag, BRUKER_NAME)
}

/// [`check_bruker_files`] with the caller's buffers of `cap` bytes (`is_bruker`'s 512,
/// `brukerRead`'s 1,024).
#[allow(clippy::too_many_arguments)]
fn check_files(
    g: &mut Globals,
    dir: Option<&Path>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    fname: &str,
    files: &mut BrukerFiles,
    flag: bool,
    cap: usize,
) -> i32 {
    match c_stat(dir, fname) {
        Err(e) => {
            g.errno = e;
            if flag {
                let _ = writeln!(err, "ERROR: could not stat {fname}.");
            }
            return 0;
        }
        Ok(m) if !m.is_dir() => {
            if flag {
                let _ = writeln!(err, "ERROR: {fname} is not a directory.");
            }
            return 0;
        }
        Ok(_) => {}
    }
    if flag {
        let _ = writeln!(out, "INFO: directory {fname} ");
    }
    let parts: [Part; 5] = [
        ("/method", "INFO:    method {}\n", |f| &mut f.method),
        ("/acqp", "INFO:      acqp {}\n", |f| &mut f.acqp),
        ("/pdata/1/2dseq", "INFO:     2dseq {} \n", |f| &mut f.data),
        ("/pdata/1/d3proc", "INFO:    d3proc {} \n", |f| {
            &mut f.d3proc
        }),
        ("/pdata/1/reco", "INFO:    d3proc {} \n", |f| &mut f.reco),
    ];
    for (part, line, slot) in parts {
        let name = format!("{fname}{part}");
        if name.len() >= cap {
            g.notices.add(
                &FS_221,
                format!(
                    "checkBrukerFiles: a name of {} bytes used whole (FreeSurfer's buffer holds {cap})",
                    name.len()
                ),
            );
        }
        *slot(files) = name.clone();
        if let Err(e) = c_stat(dir, &name) {
            g.errno = e;
            if flag {
                let _ = writeln!(err, "ERROR: could not stat {name}.");
            }
            return 0;
        }
        if flag {
            let _ = out.write_all(line.replace("{}", &name).as_bytes());
        }
    }
    1
}

/// `is_bruker(fname)`: 1 when `fname` is a directory that [`check_bruker_files`] accepts (no
/// messages); 0 otherwise (`errno` as `stat` left it).
pub fn is_bruker(g: &mut Globals, dir: Option<&Path>, fname: &str) -> i32 {
    match c_stat(dir, fname) {
        Err(e) => {
            g.errno = e;
            0
        }
        Ok(m) if !m.is_dir() => 0,
        Ok(_) => {
            let mut files = BrukerFiles::default();
            check_bruker_files(
                g,
                dir,
                &mut std::io::sink(),
                &mut std::io::sink(),
                fname,
                &mut files,
                false,
            )
        }
    }
}

// ---------------------------------------------------------------- the readers

/// The bytes `fgets` reads per line (`char line[512]`).
const LINE: usize = 512;
/// The readers' `Parameter[256]` and `Value[128]`.
const PARAMETER: usize = 256;
const VALUE: usize = 128;

/// `brukerRead`'s `BrukerTransform`: what `pdata/1/reco` and `acqp` set. `None`: never written
/// (FreeSurfer's structure is on the stack: memory contents, FS-401).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BrukerTransform {
    pub transposition: Option<i32>,
    pub fov: [Option<f64>; 3],
    pub size: [Option<i32>; 3],
    pub ft_size: [Option<i32>; 3],
    /// Computed from `fov`, `size` and `ft_size` (`None` where one of them was never written).
    pub vox_size: [Option<f64>; 3],
    pub offset: [Option<f64>; 3],
    pub read_offset: Option<f64>,
    pub phase1_offset: Option<f64>,
    pub slice_offset: Option<f64>,
    pub grad_matrix: [Option<f64>; 9],
    /// `type`: the voxels' type (`MRI_SHORT`, `MRI_INT`).
    pub mtype: Option<i32>,
    pub dim: Option<i32>,
    /// larmorx's: `$RECO_byte_order` is `bigEndian` (FreeSurfer does not read it: FS-404).
    pub big_endian: bool,
}

impl BrukerTransform {
    /// Every field set to `v` (`vi` for the integers), as a caller that initialises its structure.
    pub fn filled(v: f64, vi: i32) -> BrukerTransform {
        BrukerTransform {
            transposition: Some(vi),
            fov: [Some(v); 3],
            size: [Some(vi); 3],
            ft_size: [Some(vi); 3],
            vox_size: [Some(v); 3],
            offset: [Some(v); 3],
            read_offset: Some(v),
            phase1_offset: Some(v),
            slice_offset: Some(v),
            grad_matrix: [Some(v); 9],
            mtype: Some(vi),
            dim: Some(vi),
            big_endian: false,
        }
    }

    /// The names of the fields never written.
    fn unwritten(&self) -> Vec<&'static str> {
        let mut u = Vec::new();
        let mut chk = |c: bool, n: &'static str| {
            if !c {
                u.push(n);
            }
        };
        chk(self.transposition.is_some(), "RECO_transposition");
        chk(self.fov.iter().all(Option::is_some), "RECO_fov");
        chk(self.size.iter().all(Option::is_some), "RECO_size");
        chk(self.ft_size.iter().all(Option::is_some), "RECO_ft_size");
        chk(self.mtype.is_some(), "RECO_wordtype");
        chk(self.dim.is_some(), "RECO_fov's dimension");
        chk(self.read_offset.is_some(), "ACQ_read_offset");
        chk(self.phase1_offset.is_some(), "ACQ_phase1_offset");
        chk(self.slice_offset.is_some(), "ACQ_slice_offset");
        chk(
            self.grad_matrix.iter().all(Option::is_some),
            "ACQ_grad_matrix",
        );
        u
    }
}

/// A line as C reads it: its bytes up to the first NUL.
fn c_str(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// `splitParameterValue(line, Parameter, Value)`: for a line holding `##` and, after it, `=`: the
/// parameter (between them) and the value (after the `=`, parentheses made spaces, ended at its
/// first new line); `line` ends at the `=` afterwards (FreeSurfer writes a NUL there). `None`
/// (0) for another line. The callers' buffers hold 255 bytes of parameter and 127 of value with
/// the new line: longer ones overflow them (FS-259: both modes use them whole, noted).
pub fn split_parameter_value(g: &mut Globals, line: &mut Vec<u8>) -> Option<(Vec<u8>, Vec<u8>)> {
    let p0 = line.windows(2).position(|w| w == b"##")? + 2;
    let p1 = p0 + line[p0..].iter().position(|&c| c == b'=')?;
    let parameter = line[p0..p1].to_vec();
    let mut value = line[p1 + 1..].to_vec();
    if parameter.len() >= PARAMETER || value.len() >= VALUE {
        g.notices.add(
            &FS_259,
            format!(
                "a Bruker parameter line with a {}-byte name and a {}-byte value used whole (FreeSurfer's buffers hold {} and {})",
                parameter.len(),
                value.len(),
                PARAMETER - 1,
                VALUE - 1
            ),
        );
    }
    line.truncate(p1);
    for i in 0..value.len() {
        match value[i] {
            b'(' | b')' => value[i] = b' ',
            b'\n' => {
                value.truncate(i);
                break;
            }
            _ => {}
        }
    }
    Some((parameter, value))
}

/// `sscanf(s, "%d")` into `slot` (left alone when nothing is read).
fn scan_d(g: &mut Globals, s: &[u8], slot: &mut Option<i32>) {
    let mut b = Bytes::new(s);
    let mut sc = Scanf::new(&mut b);
    if let Some(v) = sc.d(&mut g.errno, true) {
        *slot = Some(v);
    }
    sc.finish(&mut g.errno);
}

/// `sscanf(s, "%d %d ...")` (or `%lf`) into the slots, in order, until one is not read; a
/// trailing blank in the format when `blank`.
fn scan_list<T>(
    g: &mut Globals,
    s: &[u8],
    slots: &mut [&mut Option<T>],
    blank: bool,
    one: fn(&mut Scanf, &mut i32) -> Option<T>,
) {
    let mut b = Bytes::new(s);
    let mut sc = Scanf::new(&mut b);
    for (k, slot) in slots.iter_mut().enumerate() {
        if k > 0 {
            sc.text(&mut g.errno, b" ");
        }
        match one(&mut sc, &mut g.errno) {
            Some(v) => **slot = Some(v),
            None => break,
        }
    }
    if blank {
        sc.text(&mut g.errno, b" ");
    }
    sc.finish(&mut g.errno);
}

fn lf1(sc: &mut Scanf, e: &mut i32) -> Option<f64> {
    sc.lf(e, true)
}
fn d1(sc: &mut Scanf, e: &mut i32) -> Option<i32> {
    sc.d(e, true)
}

/// `fgets(line, 512, fp)` into the C string `line` (left as it was at the end of the file).
fn fgets(fp: &mut CFile, line: &mut Vec<u8>) -> bool {
    match fp.fgets(LINE) {
        Some(l) => {
            *line = c_str(&l).to_vec();
            true
        }
        None => false,
    }
}

/// The parameter files' `fopen(name, "r")`: `errno` set on a failure.
fn open(g: &mut Globals, dir: Option<&Path>, name: &str) -> Option<CFile> {
    match CFile::open(dir, name, "r") {
        Ok(f) => Some(f),
        Err(e) => {
            g.errno = e;
            None
        }
    }
}

/// `readBrukerD3proc(d3procFile, &width, &height, &depth, &type, &nframes)`: 1, the values the
/// file's `##$IM_SIX=`, `##$IM_SIY=`, `##$IM_SIZ=` and `##$IM_SIT=` lines give (`%d`; the depth
/// is the last of `IM_SIZ` and `IM_SIT`, kept as FreeSurfer reads them) and `##$DATTYPE=`
/// (`ip_short`, `ip_int`), up to `##END=`; the others are left alone. 0 after `ERROR: could not
/// open d3proc <f>` (no new line) or `ERROR: currently only short and int types are supported
/// (Value=<v>).` (standard error; the file left open, FS-260: closed by larmorx).
pub fn read_bruker_d3proc(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    file: &str,
    size: &mut [Option<i32>; 3],
    mtype: &mut Option<i32>,
) -> i32 {
    let Some(mut fp) = open(g, dir, file) else {
        let _ = write!(err, "ERROR: could not open d3proc {file}");
        return 0;
    };
    let mut line = Vec::new();
    while fgets(&mut fp, &mut line) {
        let Some((parameter, value)) = split_parameter_value(g, &mut line) else {
            continue;
        };
        let v = c_str(&value);
        match parameter.as_slice() {
            b"END" => break,
            b"$IM_SIX" => scan_d(g, v, &mut size[0]),
            b"$IM_SIY" => scan_d(g, v, &mut size[1]),
            b"$IM_SIZ" | b"$IM_SIT" => scan_d(g, v, &mut size[2]),
            b"$DATTYPE" => {
                if v == b"ip_short" {
                    *mtype = Some(MRI_SHORT);
                } else if v == b"ip_int" {
                    *mtype = Some(MRI_INT);
                } else {
                    let _ = err.write_all(
                        b"ERROR: currently only short and int types are supported (Value=",
                    );
                    let _ = err.write_all(v);
                    let _ = err.write_all(b").\n");
                    fp.close();
                    return 0;
                }
            }
            _ => {}
        }
    }
    fp.close();
    1
}

/// `readBrukerReco(recoFile, &bTran)`: 1, the transform's fields from the file's lines up to
/// `##END=`: `$RECO_transposition` (`%d` of the next line), `$RECO_fov`, `$RECO_size`,
/// `$RECO_ft_size` (the dimension from the value, `%d`; `INFO: <name> dimension is <d>. The data
/// is not a 3D volume.` for another than 3; the next line's 3 numbers, or 2 with the third the
/// first's), `$RECO_wordtype` (its first 13 characters `_16BIT_SGN_IN` short, `_32BIT_SGN_IN`
/// int; another: `INFO: unsupported data type <v>` and 0), `$RECO_mode` and `ORIGIN` (`INFO:`
/// lines on standard output); then each axis's voxel size `fov * 10 / size` and offset `-vox *
/// (ft_size - 1) / 2`. A missing next line: `ERROR: value must follow <name>` (no new line) and 0.
/// FreeSurfer never closes the file (FS-260: closed by larmorx). larmorx reads `$RECO_byte_order`
/// too (FS-404).
pub fn read_bruker_reco(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    file: &str,
    t: &mut BrukerTransform,
) -> i32 {
    let Some(mut fp) = open(g, dir, file) else {
        let _ = write!(err, "ERROR: could not open recoFile {file}");
        return 0;
    };
    let mut dim: Option<i32> = Some(0);
    let mut line = Vec::new();
    while fgets(&mut fp, &mut line) {
        let Some((parameter, value)) = split_parameter_value(g, &mut line) else {
            continue;
        };
        let v = c_str(&value).to_vec();
        match parameter.as_slice() {
            b"END" => break,
            b"$RECO_transposition" => {
                if !fgets(&mut fp, &mut line) {
                    let _ = write!(err, "ERROR: value must follow RECO_transposition");
                    fp.close();
                    return 0;
                }
                scan_d(g, &line, &mut t.transposition);
            }
            b"$RECO_fov" | b"$RECO_size" | b"$RECO_ft_size" => {
                scan_d(g, &v, &mut dim);
                let d = dim.unwrap_or(0);
                let which = match parameter.as_slice() {
                    b"$RECO_fov" => 0,
                    b"$RECO_size" => 1,
                    _ => 2,
                };
                if which == 0 {
                    t.dim = dim;
                }
                if d != 3 {
                    let name = ["fov", "size", "ft_size"][which];
                    let _ = writeln!(
                        err,
                        "INFO: {name} dimension is {d}. The data is not a 3D volume."
                    );
                }
                if !fgets(&mut fp, &mut line) {
                    // RECO_ft_size's message names RECO_size, as FreeSurfer prints it
                    let name = ["RECO_fov", "RECO_size", "RECO_size"][which];
                    let _ = write!(err, "ERROR: value must follow {name}");
                    fp.close();
                    return 0;
                }
                let n = match d {
                    3 => 3,
                    2 => 2,
                    _ => 0,
                };
                if which == 0 {
                    let [a, b, c] = &mut t.fov;
                    scan_list(g, &line, &mut [a, b, c][..n], false, lf1);
                    if n == 2 {
                        t.fov[2] = t.fov[0];
                    }
                } else {
                    let s = if which == 1 {
                        &mut t.size
                    } else {
                        &mut t.ft_size
                    };
                    let [a, b, c] = s;
                    scan_list(g, &line, &mut [a, b, c][..n], false, d1);
                    if n == 2 {
                        s[2] = s[0];
                    }
                }
            }
            b"$RECO_wordtype" => {
                if v.starts_with(b"_16BIT_SGN_IN") {
                    t.mtype = Some(MRI_SHORT);
                } else if v.starts_with(b"_32BIT_SGN_IN") {
                    t.mtype = Some(MRI_INT);
                } else {
                    let _ = err.write_all(b"INFO: unsupported data type ");
                    let _ = err.write_all(&v);
                    let _ = err.write_all(b"\n");
                    fp.close();
                    return 0;
                }
            }
            b"$RECO_byte_order" => {
                t.big_endian = v.starts_with(b"bigEndian");
            }
            b"$RECO_mode" => {
                let _ = out.write_all(b"INFO: reconstruction mode was ");
                let _ = out.write_all(&v);
                let _ = out.write_all(b"\n");
            }
            b"ORIGIN" => {
                let _ = out.write_all(b"INFO: software by ");
                let _ = out.write_all(&v);
                let _ = out.write_all(b"\n");
            }
            _ => {}
        }
    }
    fp.close();
    for i in 0..3 {
        t.vox_size[i] = match (t.fov[i], t.size[i]) {
            (Some(f), Some(s)) => Some(div64(mul64(f, 10.0), f64::from(s))),
            _ => None,
        };
        t.offset[i] = match (t.vox_size[i], t.ft_size[i]) {
            (Some(v), Some(f)) => Some(div64(mul64(-v, f64::from(f.wrapping_sub(1))), 2.0)),
            _ => None,
        };
    }
    1
}

/// `readBrukerAcqp(acqpFile, &TR, &TE, &TI, &flip_angle, &bTran)`: 1, the values of the file's
/// lines up to `##END=`: `OWNER` (the next two lines printed `INFO: <line>`; at the end of the
/// file `warning, no lines read by fgets` and the line before printed again), `$ACQ_dim` (`%d`;
/// another than 3: three `INFO:` lines), `$ACQ_flip_angle` (`%lf` of the value, then times
/// 3.141592653589793 over 180), `$ACQ_repetition_time`, `$ACQ_echo_time`, `$ACQ_inversion_time`,
/// `$ACQ_read_offset`, `$ACQ_phase1_offset`, `$ACQ_slice_offset` (`%lf` of the next line) and
/// `$ACQ_grad_matrix` (nine of the next line). A missing next line: `ERROR: float value must
/// follow <name>` (no new line; the gradient matrix's names `ACQ_slice_offset`) and 0.
#[allow(clippy::too_many_arguments)]
pub fn read_bruker_acqp(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    file: &str,
    times: &mut [Option<f64>; 3],
    flip_angle: &mut Option<f64>,
    t: &mut BrukerTransform,
) -> i32 {
    let Some(mut fp) = open(g, dir, file) else {
        let _ = write!(err, "ERROR: could not open acqpFile {file}");
        return 0;
    };
    let mut dim: Option<i32> = Some(0);
    let mut line = Vec::new();
    while fgets(&mut fp, &mut line) {
        let Some((parameter, value)) = split_parameter_value(g, &mut line) else {
            continue;
        };
        let v = c_str(&value).to_vec();
        let next_line: Option<(&str, usize)> = match parameter.as_slice() {
            b"$ACQ_repetition_time" => Some(("ACQ_repetition_time", 0)),
            b"$ACQ_echo_time" => Some(("ACQ_echo_time", 1)),
            b"$ACQ_inversion_time" => Some(("ACQ_inversion_time", 2)),
            b"$ACQ_read_offset" => Some(("ACQ_read_offset", 3)),
            b"$ACQ_phase1_offset" => Some(("ACQ_phase1_offset", 4)),
            b"$ACQ_slice_offset" => Some(("ACQ_slice_offset", 5)),
            b"$ACQ_grad_matrix" => Some(("ACQ_slice_offset", 6)),
            _ => None,
        };
        if let Some((name, k)) = next_line {
            if !fgets(&mut fp, &mut line) {
                let _ = write!(err, "ERROR: float value must follow {name}");
                fp.close();
                return 0;
            }
            match k {
                0..=2 => scan_list(g, &line, &mut [&mut times[k]], false, lf1),
                3 => scan_list(g, &line, &mut [&mut t.read_offset], false, lf1),
                4 => scan_list(g, &line, &mut [&mut t.phase1_offset], false, lf1),
                5 => scan_list(g, &line, &mut [&mut t.slice_offset], false, lf1),
                _ => {
                    let mut slots: Vec<&mut Option<f64>> = t.grad_matrix.iter_mut().collect();
                    scan_list(g, &line, &mut slots, true, lf1);
                }
            }
            continue;
        }
        match parameter.as_slice() {
            b"END" => break,
            b"OWNER" => {
                for _ in 0..2 {
                    if !fgets(&mut fp, &mut line) {
                        let _ = writeln!(err, "warning, no lines read by fgets");
                    }
                    let _ = out.write_all(b"INFO: ");
                    let _ = out.write_all(&line);
                }
            }
            b"$ACQ_dim" => {
                scan_d(g, &v, &mut dim);
                let d = dim.unwrap_or(0);
                if d != 3 {
                    let _ = writeln!(out, "INFO: acqp tells the dimension is not 3 but {d}");
                    let _ = writeln!(out, "INFO: usually the volume has multiple 2d slices");
                    let _ = writeln!(out, "INFO: direction cosine info is meaningless.");
                }
            }
            b"$ACQ_flip_angle" => {
                scan_list(g, &v, &mut [&mut *flip_angle], false, lf1);
                *flip_angle = flip_angle.map(|f| div64(mul64(f, std::f64::consts::PI), 180.0));
            }
            _ => {}
        }
    }
    fp.close();
    1
}

/// `stuff_four_by_four(m, ...)` of sixteen doubles (FreeSurfer's function takes `float`s: each
/// rounded to `float`).
fn four_by_four(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    v: [f64; 16],
) -> Result<Matrix, Fatal> {
    let mut m = matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)?.expect("a real matrix");
    for (k, x) in v.into_iter().enumerate() {
        m.set_relt(k as i32 / 4 + 1, k as i32 % 4 + 1, x as f32);
    }
    Ok(m)
}

fn mul(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    a: &Matrix,
    b: &Matrix,
) -> Result<Matrix, Fatal> {
    Ok(matrix_multiply(g, out, err, Some(a), Some(b), None)?.unwrap_new())
}

/// `buildVoxToRASTransform(mri, &bTran)`: the volume's geometry from the transform: `tmp =
/// rotation * translation * swap * vox->mm * transposition` (4 x 4 `float` matrices; FreeSurfer's
/// comment in `Bruker.cpp` explains each), the centre `tmp * (width/2, height/2, depth/2, 1)`,
/// the direction cosines the columns of `diag(1/vox) * tmp` (the first two sizes swapped when
/// transposed) for a 3-dimensional transform and fixed ones (`(-1,0,0)`, `(0,0,-1)`, `(0,1,0)`)
/// for another, the voxel sizes (swapped likewise), `ras_good_flag` 1. Fields never written
/// count as 0 (FS-401, noted by [`bruker_read`]).
pub fn build_vox_to_ras_transform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &mut Mri,
    t: &BrukerTransform,
) -> Result<i32, Fatal> {
    let z = |x: Option<f64>| x.unwrap_or(0.0);
    let vox = t.vox_size.map(z);
    let off = t.offset.map(z);
    let gm = t.grad_matrix.map(z);
    let transposed = t.transposition.unwrap_or(0) != 0;
    let transpos = if transposed {
        [
            0., 1., 0., 0., 1., 0., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
        ]
    } else {
        [
            1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
        ]
    };
    let transpos = four_by_four(g, out, err, transpos)?;
    let voxmm = four_by_four(
        g,
        out,
        err,
        [
            vox[0], 0., 0., off[0], 0., vox[1], 0., off[1], 0., 0., vox[2], off[2], 0., 0., 0., 1.,
        ],
    )?;
    let swap = four_by_four(
        g,
        out,
        err,
        [
            -1., 0., 0., 0., 0., -1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
        ],
    )?;
    let translation = four_by_four(
        g,
        out,
        err,
        [
            1.,
            0.,
            0.,
            z(t.read_offset),
            0.,
            1.,
            0.,
            z(t.phase1_offset),
            0.,
            0.,
            1.,
            z(t.slice_offset),
            0.,
            0.,
            0.,
            1.,
        ],
    )?;
    let rotation = four_by_four(
        g,
        out,
        err,
        [
            gm[0], gm[3], gm[6], 0., gm[1], gm[4], gm[7], 0., gm[2], gm[5], gm[8], 0., 0., 0., 0.,
            1.,
        ],
    )?;
    let mut tmp = mul(g, out, err, &voxmm, &transpos)?;
    tmp = mul(g, out, err, &swap, &tmp)?;
    tmp = mul(g, out, err, &translation, &tmp)?;
    tmp = mul(g, out, err, &rotation, &tmp)?;
    let mut ctr = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a real vector");
    let half = |n: i32| (f64::from(n) / 2.0) as f32;
    ctr.set_relt(1, 1, half(mri.vg.width));
    ctr.set_relt(2, 1, half(mri.vg.height));
    ctr.set_relt(3, 1, half(mri.vg.depth));
    ctr.set_relt(4, 1, 1.0);
    let rcs = mul(g, out, err, &tmp, &ctr)?;
    mri.vg.c_r = rcs.relt(1, 1);
    mri.vg.c_a = rcs.relt(2, 1);
    mri.vg.c_s = rcs.relt(3, 1);
    let inv = |x: f64| div64(1.0, x);
    let (i0, i1) = if transposed { (1, 0) } else { (0, 1) };
    let vox_inv = four_by_four(
        g,
        out,
        err,
        [
            inv(vox[i0]),
            0.,
            0.,
            0.,
            0.,
            inv(vox[i1]),
            0.,
            0.,
            0.,
            0.,
            inv(vox[2]),
            0.,
            0.,
            0.,
            0.,
            1.,
        ],
    )?;
    let rottran = mul(g, out, err, &vox_inv, &tmp)?;
    let v = &mut mri.vg;
    if t.dim.unwrap_or(0) == 3 {
        (v.x_r, v.x_a, v.x_s) = (rottran.relt(1, 1), rottran.relt(2, 1), rottran.relt(3, 1));
        (v.y_r, v.y_a, v.y_s) = (rottran.relt(1, 2), rottran.relt(2, 2), rottran.relt(3, 2));
        (v.z_r, v.z_a, v.z_s) = (rottran.relt(1, 3), rottran.relt(2, 3), rottran.relt(3, 3));
    } else {
        (v.x_r, v.x_a, v.x_s) = (-1.0, 0.0, 0.0);
        (v.y_r, v.y_a, v.y_s) = (0.0, 0.0, -1.0);
        (v.z_r, v.z_a, v.z_s) = (0.0, 1.0, 0.0);
    }
    let i0 = if t.transposition.unwrap_or(0) == 1 {
        1
    } else {
        0
    };
    v.xsize = vox[i0] as f32;
    v.ysize = vox[1 - i0] as f32;
    v.zsize = vox[2] as f32;
    v.ras_good_flag = 1;
    Ok(1)
}

/// Why [`read_bruker_volume`] failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VolumeFail {
    Open,
    Type,
    /// A short read: FreeSurfer frees the volume there (and `brukerRead` again, FS-403).
    Short,
}

/// `readBrukerVolume(mri, dataFile)`: 1 and the voxels of `pdata/1/2dseq`, row by row, the
/// volume's name `dataFile`; 0 after `ERROR: could not open dataFile <f>` (no new line), `INFO:
/// unknown size.  bail out` (a type other than bytes, short, float, int and long; the file left
/// open, FS-260) or a short read (FreeSurfer frees the volume there: FS-403 in
/// [`bruker_read`]).
///
/// - **FS-402** (rule A): FreeSurfer reads 2 bytes per voxel whatever the type, so a 32-bit volume
///   gets the first half of each row from the file (and a byte volume's rows overlap, the last
///   past the volume). Default: `bytes_per_vox` per voxel; `reproduce` FreeSurfer's reads (what
///   would land past the volume dropped). Noted where the type is not short.
/// - **FS-404** (rule A): FreeSurfer ignores `$RECO_byte_order` (this machine's byte order).
///   Default: a `bigEndian` file's voxels are byte-swapped; `reproduce` reads them as they are.
///   Noted when the reco says big-endian.
pub fn read_bruker_volume(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    mri: &mut Mri,
    data_file: &str,
    big_endian: bool,
) -> i32 {
    i32::from(read_voxels(g, err, dir, mri, data_file, big_endian).is_ok())
}

fn read_voxels(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    mri: &mut Mri,
    data_file: &str,
    big_endian: bool,
) -> Result<(), VolumeFail> {
    mri.vg.fname.strcpy_bytes(data_file.as_bytes());
    let Some(mut fp) = open(g, dir, data_file) else {
        let _ = write!(err, "ERROR: could not open dataFile {data_file}");
        return Err(VolumeFail::Open);
    };
    if ![MRI_UCHAR, MRI_SHORT, MRI_FLOAT, MRI_INT, MRI_LONG].contains(&mri.mtype) {
        let _ = writeln!(err, "INFO: unknown size.  bail out");
        fp.close();
        return Err(VolumeFail::Type);
    }
    let reproduce = g.notices.reproduce();
    let bpv = mri.bytes_per_vox;
    if bpv != 2 {
        g.notices.add(
            &FS_402,
            if reproduce {
                format!("Bruker 2dseq: 2 bytes per voxel of a {bpv}-byte type, as FreeSurfer")
            } else {
                format!("Bruker 2dseq: {bpv} bytes per voxel, not FreeSurfer's 2")
            },
        );
    }
    if big_endian {
        g.notices.add(
            &FS_404,
            if reproduce {
                "Bruker 2dseq: a big-endian file read as little-endian, as FreeSurfer"
            } else {
                "Bruker 2dseq: a big-endian file byte-swapped (FreeSurfer reads it as little-endian)"
            },
        );
    }
    let (w, h, d) = (
        mri.vg.width.max(0) as usize,
        mri.vg.height.max(0) as usize,
        mri.vg.depth.max(0) as usize,
    );
    let read_size = if reproduce { 2 } else { bpv };
    let row_bytes = w * bpv;
    let mut buf = vec![0u8; w * read_size];
    let total = mri.chunk.as_ref().map_or(0, Vec::len);
    let mut past = false;
    for k in 0..d {
        for j in 0..h {
            if fp.fread(&mut buf, read_size, w) != w {
                fp.close();
                return Err(VolumeFail::Short);
            }
            if !reproduce && big_endian && bpv > 1 {
                for v in buf.chunks_mut(bpv) {
                    v.reverse();
                }
            }
            let at = (k * h + j) * row_bytes;
            if let Some(chunk) = mri.chunk.as_mut() {
                let end = (at + buf.len()).min(total);
                past |= at + buf.len() > total;
                if at < end {
                    chunk[at..end].copy_from_slice(&buf[..end - at]);
                }
            }
        }
    }
    if past {
        g.notices.add(
            &FS_402,
            "Bruker 2dseq: FreeSurfer's last row lands past the volume (dropped)",
        );
    }
    fp.close();
    Ok(())
}

/// `brukerRead(fname, read_volume)`: the Bruker study directory `fname` ([`check_bruker_files`]
/// with its messages, 1,024-byte names), its `pdata/1/reco`, `pdata/1/d3proc` and `acqp` read in
/// that order (NULL at the first failure); a volume of the d3proc's sizes and the reco's type
/// (`MRIalloc`), TR, TE, TI and the flip angle from acqp, the geometry
/// ([`build_vox_to_ras_transform`]), the voxels when `read_volume`.
///
/// - **FS-401** (memory never written): a size, time, angle or transform field the files do not
///   give is FreeSurfer's stack memory. Both modes use 0, noted.
/// - **FS-403** (crash): a 2dseq shorter than the volume makes FreeSurfer free the volume twice
///   (glibc aborts). Both modes return NULL, noted.
pub fn bruker_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    let mut files = BrukerFiles::default();
    if check_files(g, dir, out, err, fname, &mut files, true, 1024) == 0 {
        return Ok(None);
    }
    let mut t = BrukerTransform::default();
    if read_bruker_reco(g, out, err, dir, &files.reco, &mut t) == 0 {
        return Ok(None);
    }
    let mut size: [Option<i32>; 3] = [None; 3];
    let mut d3type = None;
    if read_bruker_d3proc(g, err, dir, &files.d3proc, &mut size, &mut d3type) == 0 {
        return Ok(None);
    }
    let mut times: [Option<f64>; 3] = [None; 3];
    let mut flip = None;
    if read_bruker_acqp(g, out, err, dir, &files.acqp, &mut times, &mut flip, &mut t) == 0 {
        return Ok(None);
    }
    let mut unwritten = t.unwritten();
    for (v, n) in size.iter().zip(["IM_SIX", "IM_SIY", "IM_SIZ"]) {
        if v.is_none() {
            unwritten.push(n);
        }
    }
    for (v, n) in times
        .iter()
        .zip(["ACQ_repetition_time", "ACQ_echo_time", "ACQ_inversion_time"])
    {
        if v.is_none() {
            unwritten.push(n);
        }
    }
    if flip.is_none() {
        unwritten.push("ACQ_flip_angle");
    }
    if !unwritten.is_empty() {
        g.notices.add(
            &FS_401,
            format!(
                "brukerRead: {} never given (FreeSurfer's memory contents): 0",
                unwritten.join(", ")
            ),
        );
    }
    let z = |v: Option<i32>| v.unwrap_or(0);
    let mut mri = mri::mri_alloc(g, out, err, z(size[0]), z(size[1]), z(size[2]), z(t.mtype))?;
    mri.tr = times[0].unwrap_or(0.0) as f32;
    mri.te = times[1].unwrap_or(0.0) as f32;
    mri.ti = times[2].unwrap_or(0.0) as f32;
    mri.flip_angle = flip.unwrap_or(0.0);
    build_vox_to_ras_transform(g, out, err, &mut mri, &t)?;
    if read_volume {
        let big = t.big_endian;
        if let Err(f) = read_voxels(g, err, dir, &mut mri, &files.data, big) {
            if f == VolumeFail::Short {
                g.notices.add(
                    &FS_403,
                    "brukerRead: a 2dseq shorter than the volume (FreeSurfer frees the volume twice and aborts): NULL",
                );
            }
            return Ok(None);
        }
    }
    Ok(Some(mri))
}
