// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mris_calc`'s program (`mris_calc.cpp` 695-2628 but the operations, [`super::ops`]): the
//! entry point [`run`] (registered as `mris_calc` in the crate's `PROGRAMS`).
//!
//! The order of `main`: `ErrorInit` and `DiagInit`; the version options (`handleVersionOption`);
//! the options (`options_parse`: `-o`/`--output`, `threads`/`nthreads`, `-e`/`--strictExtensions`,
//! `-v`/`--version`, `--verbosity`, `-l`/`--label`, the help); two or three arguments (input 1,
//! the operation, input 2 or a number), else `Insufficient number of arguments` (status 10); the
//! inputs read (`fileRead`: a name that is no file but a whole number reads as a number; a
//! volume by `MRIread`, its values packed frame by frame, column, row, slice; a curvature file
//! by the program's own reader; anything else `I was unable to identify this file type`); their
//! sizes and types checked; the output's name (`out` and the input's extension without `-o`);
//! the operation (`CURV_process`: with `--label` on the label's values only, the others 0); the
//! statistics on standard error; the output written (`fileWrite`: the output's extension decides
//! its type, which must be the input's; a volume with the last volume read's header, as `float`;
//! a curvature file with the last one's face and value counts); `#VMPC# mris_calc VmPeak`.
//!
//! The defects: FS-1119 (the 65,536-byte text buffers), FS-1120 (a file that cannot be written
//! ends with status 0), FS-1121 (two volumes of other dimensions with as many values), FS-1122
//! (an operation of two values without a second input), FS-1123 (an option's value missing),
//! FS-1124 (`threads` with a value that is no number), FS-1125 (label points outside input 1),
//! FS-1126 (indices and counts through a `float`); FS-17 (`andbw`, `orbw`), FS-119, FS-167,
//! GLIBC-07 from the shared functions.

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::cscan as ccscan;
use larmorx_core::defects::{
    FS_1119, FS_1120, FS_1121, FS_1122, FS_1123, FS_1124, FS_1125, FS_1126,
};
use larmorx_core::math::Libm;
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::cscan::{self, Bytes, ERANGE, Scan, Scanf};
use larmorx_freesurfer_core::cstd::note_glibc07;
use larmorx_freesurfer_core::diag::diag_init;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::help_xml::output_help_xml;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_NOFILE, Fatal};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::run::Settings;
use larmorx_freesurfer_core::sse::{sqrt, sub};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::utils::{self, get_vm_peak, stricmp};
use larmorx_freesurfer_core::version::{get_version, handle_version_option};
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::fio::{
    fio_extension, fread_float, fread_int, fread3, fwrite_float, fwrite_int, fwrite3,
};
use larmorx_freesurfer_io::identify::{
    MRI_CURV_FILE, MRI_VOLUME_TYPE_UNKNOWN, mri_identify, type_to_string,
};
use larmorx_freesurfer_io::label::label_read;
use larmorx_freesurfer_io::mriio::{mri_read_volume, mri_write};
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, MRI_FLOAT, Mri, mri_alloc_sequence, mri_copy_header,
    mri_copy_pulse_parameters,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::basemisc::{G_LC, cprintd, cprintf, cprints};

use super::ops::{
    Lib, Op, Run, Statics, binary, is_binary, libm_names, operation_lookup, to_f32, unary,
    writes_output,
};

/// The program's name as `argv[0]` gives it (the scripts run it by name).
pub const NAME: &str = "mris_calc";

/// The help text (`mris_calc.help.xml`, FreeSurfer 8.2.0's, unchanged).
pub const HELP_XML: &[u8] = include_bytes!("mris_calc.help.xml");

/// `STRBUF`: the size of the program's text buffers.
const STRBUF: usize = 65536;
/// The curvature files' new-version number.
const NEW_VERSION_MAGIC_NUMBER: i32 = 16_777_215;
/// glibc's buffer of a standard output that is a pipe or a file (`st_blksize`): what an abort
/// loses of what was printed since the last flush.
const STDOUT_BUFFER: usize = 4096;

/// `e_FILETYPE`, with `Gppch_filetype`, `Gppch_fileExt` and `Gppch_fileDotExt` (`e_SurfaceFile`
/// is never found: `fileType_find`'s last branch cannot be reached).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ft {
    Unknown,
    Volume,
    Curvature,
    FloatArg,
}

impl Ft {
    fn name(self) -> &'static str {
        match self {
            Ft::Unknown => "Unknown",
            Ft::Volume => "Volume",
            Ft::Curvature => "Curvature",
            Ft::FloatArg => "FloatArg",
        }
    }
    fn ext(self) -> &'static str {
        match self {
            Ft::Unknown => "null",
            Ft::Volume => "mgz",
            Ft::Curvature => "crv",
            Ft::FloatArg => "dummy",
        }
    }
    fn dot_ext(self) -> String {
        format!(".{}", self.ext())
    }
}

/// `e_FILEACCESS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Access {
    Unspecified,
    WrongMagic,
    Ok,
    ReadError,
}

/// One input as `fileRead` leaves it.
struct Input {
    etype: Ft,
    fstype: i32,
    size: usize,
    data: Option<Vec<f32>>,
}

/// `mris_calc`'s file-scope variables and the program's two streams. Standard output is kept
/// here until the C program would have flushed it ([`Calc::flush`]), so that a crash or an abort
/// in `reproduce` loses what FreeSurfer's would.
struct Calc<'a> {
    out: Vec<u8>,
    flushed: usize,
    err: &'a mut dyn Write,
    env: &'a Environment,
    dir: Option<&'a Path>,
    verbosity: i32,
    strict: bool,
    file1: String,
    size1: usize,
    arr1: Vec<f32>,
    copy1: Option<Vec<f32>>,
    nfaces: i32,
    vals_per_vertex: i32,
    fstype1: i32,
    etype1: Ft,
    has2: bool,
    file2: Option<String>,
    size2: usize,
    arr2: Option<Vec<f32>>,
    copy2: Option<Vec<f32>>,
    etype2: Ft,
    /// `Gp_MRI`: the header of the last volume read; and input 1's own, for FS-1121.
    header: Option<Mri>,
    header1: Option<(i32, i32, i32, i32)>,
    op_name: String,
    op: Op,
    file3: String,
    size3: usize,
    arr3: Vec<f32>,
    copy3: Option<Vec<f32>>,
    size3_copy: usize,
    file3_given: bool,
    can_write: bool,
    fstype3: i32,
    etype3: Ft,
    label_mask: bool,
    label_file: String,
    mask: Vec<f32>,
    mask_index: Vec<usize>,
    size_label: usize,
    st: Statics,
    lib: Lib,
    /// FS-1120: a file that could not be written (`fix` ends with FreeSurfer's write error).
    unwritten: Option<String>,
}

/// `mris_calc`'s entry point: its original arguments (without the program's name), the
/// settings, the two streams; the exit status. The notices of the run follow on standard error
/// (`larmorx: ...`).
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut g = Globals::new(settings.mode);
    g.stderr.read_write = system::stderr_read_write();
    let status = {
        let mut terr = log::TrackedErr::new(&g, &mut *err);
        let mut c = Calc::new(&mut terr, &settings.env, settings.dir.as_deref(), &g);
        let res = c.main(&mut g, args);
        let status = match res {
            Ok(()) => 0,
            Err(f) => f.status(),
        };
        // a crash or an abort loses what the C library had not written yet
        let keep = if g.notices.reproduce() && (status == 139 || status == 134) {
            c.flushed + (c.out.len() - c.flushed) / STDOUT_BUFFER * STDOUT_BUFFER
        } else {
            c.out.len()
        };
        let _ = out.write_all(&c.out[..keep]);
        status
    };
    let _ = out.flush();
    g.notices.print(err);
    status
}

/// `stat(name)`: whether it succeeds; `errno` its error otherwise.
fn c_stat(g: &mut Globals, dir: Option<&Path>, name: &str) -> bool {
    match std::fs::metadata(utils::resolve(dir, name)) {
        Ok(_) => true,
        Err(e) => {
            g.errno = log::errno_of(&e).unwrap_or(2);
            false
        }
    }
}

/// `strtof(s, &end)`'s length read and its `ERANGE` (overflow, or an inexact result that is tiny
/// after rounding: `errno`).
fn c_strtof_len(g: &mut Globals, s: &str) -> usize {
    let (_, n) = ccscan::strtof(s.as_bytes());
    if erange_f32(&s.as_bytes()[..n]) {
        g.errno = ERANGE;
    }
    n
}

/// Whether glibc's `strtof` of the number `b` sets `ERANGE`.
fn erange_f32(b: &[u8]) -> bool {
    let mut st = Bytes::new(b);
    matches!(cscan::scan_f32(&mut st), Scan::Value(_, true))
}

/// `atof(s)` (`strtod(s, NULL)`), with its `ERANGE`.
fn c_atof(g: &mut Globals, s: &str) -> f64 {
    let (v, n) = ccscan::strtod(s.as_bytes());
    let mut st = Bytes::new(&s.as_bytes()[..n]);
    if matches!(cscan::scan_f64(&mut st), Scan::Value(_, true)) {
        g.errno = ERANGE;
    }
    v
}

/// `sscanf(s, "%d", &v)`: the value, or `None` when it assigns none.
fn scan_d(g: &mut Globals, s: &str) -> Option<i32> {
    let mut b = Bytes::new(s.as_bytes());
    let mut sc = Scanf::new(&mut b);
    let v = sc.d(&mut g.errno, true);
    if sc.finish(&mut g.errno) == 1 {
        v
    } else {
        None
    }
}

/// `toupper(*s)` of an option's first byte (0 for an empty one).
fn first_upper(s: &str) -> u8 {
    s.as_bytes().first().map_or(0, u8::to_ascii_uppercase)
}

/// `printf("%*s", width, s)`: `s` right-aligned in `width` bytes.
fn pad(width: usize, s: &str) -> String {
    let mut t = " ".repeat(width.saturating_sub(s.len()));
    t.push_str(s);
    t
}

impl<'a> Calc<'a> {
    fn new(
        err: &'a mut dyn Write,
        env: &'a Environment,
        dir: Option<&'a Path>,
        g: &Globals,
    ) -> Self {
        Calc {
            out: Vec::new(),
            flushed: 0,
            err,
            env,
            dir,
            verbosity: 0,
            strict: false,
            file1: String::new(),
            size1: 0,
            arr1: Vec::new(),
            copy1: None,
            nfaces: 0,
            vals_per_vertex: 0,
            fstype1: -1,
            etype1: Ft::Unknown,
            has2: false,
            file2: None,
            size2: 0,
            arr2: None,
            copy2: None,
            etype2: Ft::Unknown,
            header: None,
            header1: None,
            op_name: String::new(),
            op: Op::Unknown,
            file3: "out".to_owned(),
            size3: 0,
            arr3: Vec::new(),
            copy3: None,
            size3_copy: 0,
            file3_given: false,
            can_write: false,
            fstype3: -1,
            etype3: Ft::Unknown,
            label_mask: false,
            label_file: String::new(),
            mask: Vec::new(),
            mask_index: Vec::new(),
            size_label: 0,
            st: Statics::default(),
            lib: Lib {
                libm: Libm::new(g.mode()),
            },
            unwritten: None,
        }
    }

    /// `fflush(stdout)`.
    fn flush(&mut self) {
        self.flushed = self.out.len();
    }

    /// A write to `G_FP`: standard output with `--verbosity`, else `/dev/null`.
    fn gfp(&mut self, s: &str) {
        if self.verbosity != 0 {
            self.out.extend_from_slice(s.as_bytes());
        }
    }

    /// `fflush(G_FP)`.
    fn gfp_flush(&mut self) {
        if self.verbosity != 0 {
            self.flush();
        }
    }

    /// FS-1119: a text of `len` bytes copied into one of the program's 65,536-byte buffers. The
    /// official build checks them: past the buffer the program aborts (`*** buffer overflow
    /// detected ***: terminated`, status 134); larmorx uses the text whole.
    fn fits(&mut self, g: &mut Globals, len: usize, what: &str) -> Result<(), Fatal> {
        if len + 1 > STRBUF {
            g.notices.add(
                &FS_1119,
                format!(
                    "mris_calc: {what} of {len} bytes, past FreeSurfer's 65,536-byte buffer, used whole (FreeSurfer aborts)"
                ),
            );
            if g.notices.reproduce() {
                let _ = self
                    .err
                    .write_all(b"*** buffer overflow detected ***: terminated\n");
                return Err(Fatal::Exit(134));
            }
        }
        Ok(())
    }

    /// `ErrorExit(code, message)`.
    fn error_exit(&mut self, g: &mut Globals, code: i32, message: &str) -> Fatal {
        self.flush();
        match log::error_exit(g, self.err, code, message) {
            Err(f) => f,
            Ok(()) => Fatal::Exit(0),
        }
    }

    /// The program's own `error_exit(action, error, code)`: the message on standard error, then
    /// `exit(code)`.
    fn own_error_exit(
        &mut self,
        g: &mut Globals,
        action: &str,
        error: &str,
        code: i32,
    ) -> Result<(), Fatal> {
        let msg = format!(
            "\n{NAME}:\n\tSorry, but I seem to have encountered an error.\n\tWhile {action},\n\t{error}\n"
        );
        self.fits(g, msg.len(), "an error message")?;
        let _ = self.err.write_all(msg.as_bytes());
        let _ = self.err.write_all(b"\n");
        Err(Fatal::Exit(code))
    }

    /// `cprints(left, right)`.
    fn cprints(&mut self, g: &mut Globals, left: &str, right: &str) -> Result<(), Fatal> {
        cprints(g, self.err, left.as_bytes(), right.as_bytes())
    }

    /// `synopsis_show()`: the help, status 1.
    fn synopsis_show(&mut self, g: &mut Globals) -> Fatal {
        output_help_xml(g, &mut self.out, self.err, Some(HELP_XML));
        Fatal::Exit(1)
    }

    /// FS-1123: an option given last whose value FreeSurfer reads past the arguments (a crash,
    /// status 139); larmorx ends with a message (status 1, rule B).
    fn need_value(&mut self, g: &mut Globals, rest: &[String]) -> Result<(), Fatal> {
        if rest.len() >= 2 {
            return Ok(());
        }
        g.notices.add(
            &FS_1123,
            format!(
                "mris_calc: option '{}' given last, without its value (FreeSurfer crashes)",
                rest[0]
            ),
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        let _ = writeln!(self.err, "{NAME}: option '{}' needs a value", rest[0]);
        Err(Fatal::Exit(1))
    }

    /// `options_parse` of the option `rest[0]`: the arguments it takes after it.
    fn options_parse(&mut self, g: &mut Globals, rest: &[String]) -> Result<usize, Fatal> {
        let option = &rest[0][1..];
        let up = first_upper(option);
        if stricmp(g, option, "-output") == 0 || up == b'O' {
            self.need_value(g, rest)?;
            self.fits(g, rest[1].len(), "the output name")?;
            self.file3 = rest[1].clone();
            self.file3_given = true;
            return Ok(1);
        }
        if stricmp(g, option, "threads") == 0 || stricmp(g, option, "nthreads") == 0 {
            self.need_value(g, rest)?;
            let n = match scan_d(g, &rest[1]) {
                Some(n) => n,
                None => {
                    g.notices.add(
                        &FS_1124,
                        format!(
                            "mris_calc: threads '{}' is no number: FreeSurfer prints memory it never wrote (here 0)",
                            rest[1]
                        ),
                    );
                    0
                }
            };
            let _ = writeln!(self.out, "nthreads {n}");
            return Ok(1);
        }
        if stricmp(g, option, "-strictExtensions") == 0 || up == b'E' {
            self.strict = true;
            return Ok(0);
        }
        if stricmp(g, option, "-version") == 0 || up == b'V' {
            let _ = writeln!(self.err, "{}", get_version());
            return Err(Fatal::Exit(1));
        }
        if stricmp(g, option, "-verbosity") == 0 {
            self.need_value(g, rest)?;
            self.verbosity = larmorx_freesurfer_core::cstd::atoi(rest[1].as_bytes());
            return Ok(1);
        }
        if stricmp(g, option, "-label") == 0 || up == b'L' {
            self.need_value(g, rest)?;
            self.fits(g, rest[1].len(), "the label's name")?;
            self.label_file = rest[1].clone();
            self.label_mask = true;
            return Ok(1);
        }
        if stricmp(g, option, "-help") == 0 || stricmp(g, option, "-usage") == 0 {
            return Err(self.synopsis_show(g));
        }
        match up {
            b'T' | b'-' => Ok(0),
            b'?' | b'U' | b'H' => Err(self.synopsis_show(g)),
            _ => {
                let _ = writeln!(
                    self.err,
                    "Unknown option '{}'. Looking for help? Use '-u' instead.",
                    rest[0]
                );
                Err(Fatal::Exit(1))
            }
        }
    }

    /// `main(argc, argv)` on the arguments after the program's name. `Ok` is status 0.
    fn main(&mut self, g: &mut Globals, args: &[String]) -> Result<(), Fatal> {
        g.progname = NAME.to_owned();
        diag_init(g, self.env, &mut self.out);
        let argv: Vec<String> = std::iter::once(NAME.to_owned())
            .chain(args.iter().cloned())
            .collect();
        let (nargs, left) = handle_version_option(g, self.env, &mut self.out, &argv, NAME);
        if nargs != 0 && left.len() == 1 {
            return Err(Fatal::Exit(0));
        }
        let mut rest: &[String] = &left[1..];
        while !rest.is_empty() && rest[0].starts_with('-') {
            let n = self.options_parse(g, rest)?;
            rest = &rest[1 + n..];
        }
        if rest.len() != 3 && rest.len() != 2 {
            return Err(self.error_exit(
                g,
                10,
                &format!(
                    "{NAME}: Insufficient number of arguments.\nYou should specify '<input1> <ACTION> [<input2> | <floatNumber>]'\nUse a '-u' for full usage instructions."
                ),
            ));
        }
        self.has2 = rest.len() == 3;
        self.file1 = rest[0].clone();
        self.op_name = rest[1].clone();
        if self.has2 {
            self.file2 = Some(rest[2].clone());
        }
        // verbosity_set
        if self.verbosity != 0 {
            self.options_print(g)?;
        }

        let file1 = self.file1.clone();
        let (inp, acc) = self.file_read(g, &file1, true)?;
        self.etype1 = inp.etype;
        self.fstype1 = inp.fstype;
        self.size1 = inp.size;
        self.arr1 = inp.data.unwrap_or_default();
        if acc != Access::Ok {
            self.file_io_error(g, &file1, acc)?;
        }
        if let Some(file2) = self.file2.clone() {
            let (inp, _) = self.file_read(g, &file2, false)?;
            self.etype2 = inp.etype;
            self.size2 = inp.size;
            self.arr2 = inp.data;
            if self.etype2 == Ft::FloatArg {
                let f = to_f32(c_atof(g, &file2));
                self.size2 = self.size1;
                self.arr2 = Some(vec![f; self.size1]);
            }
        }
        if self.verbosity != 0 {
            cprintd(self.err, b"Size of input1: ", self.size1 as i32);
            self.cprints(g, "Type of input1: ", self.etype1.name())?;
            let op = self.op_name.clone();
            self.cprints(g, "ACTION:", &op)?;
            cprintd(self.err, b"Size of input2: ", self.size2 as i32);
            self.cprints(g, "Type of input2: ", self.etype2.name())?;
        }
        if self.size1 == 0 {
            return self.own_error_exit(
                g,
                "checking input 1",
                "it seems that the input file has no contents.",
                10,
            );
        }
        if self.size1 != self.size2 && self.has2 {
            return self.unmatched_sizes(g);
        }
        if self.etype1 != self.etype2 && self.etype2 != Ft::FloatArg && self.has2 {
            return self.own_error_exit(
                g,
                "checking on input filetypes",
                "it seems that you specified incompatible types.",
                10,
            );
        }
        self.volumes_alike(g)?;

        // output_init
        self.size3 = self.size1;
        self.arr3 = self.alloc(g, self.size1)?;
        if !self.file3_given {
            self.file3.push_str(&self.etype1.dot_ext());
        }
        let ret = self.process(g)?;
        if self.verbosity != 0 {
            let _ = writeln!(self.out);
        }
        if !ret {
            let op = self.op_name.clone();
            return Err(self.error_exit(
                g,
                ERROR_BADPARM,
                &format!("Unknown operation {op} failed.\n"),
            ));
        }
        if let Some(name) = self.unwritten.take() {
            return Err(self.error_exit(
                g,
                ERROR_BADPARM,
                &format!("\n{NAME}: could not establish write access to '{name}'.\n"),
            ));
        }
        let peak = get_vm_peak(g);
        let _ = writeln!(self.out, "#VMPC# mris_calc VmPeak  {peak}");
        Ok(())
    }

    fn unmatched_sizes(&mut self, g: &mut Globals) -> Result<(), Fatal> {
        self.own_error_exit(
            g,
            "checking on input filetype sizes",
            "I found a size mismatch, i.e. len(input1)!=len(input2)",
            10,
        )
    }

    /// FS-1121: two volumes of other dimensions with as many values: FreeSurfer combines them
    /// value by value in its packing order and writes input 2's dimensions; larmorx ends with the
    /// size mismatch (rule B).
    fn volumes_alike(&mut self, g: &mut Globals) -> Result<(), Fatal> {
        if !(self.has2 && self.etype1 == Ft::Volume && self.etype2 == Ft::Volume) {
            return Ok(());
        }
        let (Some(da), Some(b)) = (self.header1, &self.header) else {
            return Ok(());
        };
        let db = (b.vg.width, b.vg.height, b.vg.depth, b.nframes);
        if da == db {
            return Ok(());
        }
        g.notices.add(
            &FS_1121,
            format!(
                "mris_calc: volumes of {}x{}x{}x{} and {}x{}x{}x{} values (as many): FreeSurfer pairs them in its packing order and writes the second's dimensions",
                da.0, da.1, da.2, da.3, db.0, db.1, db.2, db.3
            ),
        );
        if g.notices.reproduce() {
            return Ok(());
        }
        self.unmatched_sizes(g)
    }

    /// `options_print()`.
    fn options_print(&mut self, g: &mut Globals) -> Result<(), Fatal> {
        let f1 = self.file1.clone();
        self.cprints(g, "Input file 1:", &f1)?;
        let op = self.op_name.clone();
        self.cprints(g, "ACTION:", &op)?;
        if let Some(f2) = self.file2.clone() {
            self.cprints(g, "Input file 2:", &f2)?;
        }
        let f3 = self.file3.clone();
        self.cprints(g, "Output file:", &f3)
    }

    /// `xmalloc(n * sizeof(float))` zeroed (`virtual memory exhausted`, status 10, when the
    /// memory cannot be had).
    fn alloc(&mut self, g: &mut Globals, n: usize) -> Result<Vec<f32>, Fatal> {
        let mut v: Vec<f32> = Vec::new();
        if n.checked_mul(4).is_none() || v.try_reserve_exact(n).is_err() {
            return Err(self.no_memory(g));
        }
        v.resize(n, 0.0);
        Ok(v)
    }

    /// `xmalloc`'s failure: `malloc` sets `ENOMEM`, then `virtual memory exhausted` (status 10).
    fn no_memory(&mut self, g: &mut Globals) -> Fatal {
        g.errno = 12;
        self.error_exit(g, 10, &format!("{NAME}: virtual memory exhausted."))
    }

    /// FS-1119's other sites: an extension added to an output name of 65,536 bytes or more
    /// (FreeSurfer's copy is not checked there: it writes past its buffer, and the name, too long
    /// for any file system, is never written). Both modes use the name whole.
    fn past_buffer(g: &mut Globals, len: usize) {
        if len + 1 > STRBUF {
            g.notices.add(
                &FS_1119,
                format!(
                    "mris_calc: an output name of {len} bytes with its extension, past FreeSurfer's 65,536-byte buffer (FreeSurfer writes past it)"
                ),
            );
        }
    }

    /// `fileType_find(name, &type)`: a name that is no file but whole a number is a number; else
    /// the type `mri_identify` finds (a curvature file, a volume, or unknown).
    fn file_type_find(&mut self, g: &mut Globals, name: &str) -> Result<(Ft, Option<i32>), Fatal> {
        if !c_stat(g, self.dir, name) {
            self.fits(g, name.len(), "a name")?;
            if c_strtof_len(g, name) == name.len() {
                return Ok((Ft::FloatArg, None));
            }
        }
        let ty = mri_identify(g, self.dir, self.env, &mut self.out, self.err, name)?;
        let ft = if ty != MRI_VOLUME_TYPE_UNKNOWN && ty != MRI_CURV_FILE {
            Ft::Volume
        } else if ty == MRI_CURV_FILE {
            Ft::Curvature
        } else {
            Ft::Unknown
        };
        Ok((ft, Some(ty)))
    }

    /// `fileRead(name, ...)`.
    fn file_read(
        &mut self,
        g: &mut Globals,
        name: &str,
        first: bool,
    ) -> Result<(Input, Access), Fatal> {
        let (etype, ty) = self.file_type_find(g, name)?;
        let mut inp = Input {
            etype,
            fstype: ty.unwrap_or(-1),
            size: 0,
            data: None,
        };
        let acc = match etype {
            Ft::Unknown => {
                let action = format!("accessing file '{name}'");
                self.fits(g, action.len(), "a message")?;
                self.own_error_exit(
                    g,
                    &action,
                    "I was unable to identify this file type. Does it exist?",
                    10,
                )?;
                Access::Unspecified
            }
            Ft::FloatArg => Access::Unspecified,
            Ft::Volume => self.vol_file_read(g, name, &mut inp, first)?,
            Ft::Curvature => self.curv_file_read(g, name, &mut inp)?,
        };
        Ok((inp, acc))
    }

    /// `fileIO_errorHander(name, access)`.
    fn file_io_error(&mut self, g: &mut Globals, name: &str, acc: Access) -> Result<(), Fatal> {
        let what = match acc {
            Access::ReadError => "could not establish read access to",
            Access::WrongMagic => "curvature file",
            Access::Unspecified | Access::Ok => return Ok(()),
        };
        let msg = if acc == Access::WrongMagic {
            format!("\n{NAME}: {what} '{name}' has wrong magic number.\n")
        } else {
            format!("\n{NAME}: {what} '{name}'.\n")
        };
        Err(self.error_exit(g, ERROR_BADPARM, &msg))
    }

    /// `VOL_fileRead(name, ...)`: the volume's values packed frame by frame, then by column, row
    /// and slice; its header kept (`Gp_MRI`).
    fn vol_file_read(
        &mut self,
        g: &mut Globals,
        name: &str,
        inp: &mut Input,
        first: bool,
    ) -> Result<Access, Fatal> {
        if self.verbosity != 0 {
            self.cprints(g, "Reading...", "")?;
        }
        let Some(m) = mri_read_volume(g, &mut self.out, self.err, self.env, self.dir, name)? else {
            return Ok(Access::ReadError);
        };
        if self.verbosity != 0 {
            self.cprints(g, "", "ok")?;
        }
        let n = [m.vg.width, m.vg.height, m.vg.depth, m.nframes]
            .iter()
            .fold(1u64, |a, &d| a.wrapping_mul(i64::from(d) as u64));
        let size = usize::try_from(n).unwrap_or(usize::MAX);
        let mut data = self.alloc(g, size)?;
        let _ = self.out.write_all(b"Packing\n");
        self.flush();
        let mut i = 0usize;
        for f in 0..m.nframes {
            for c in 0..m.vg.width {
                for r in 0..m.vg.height {
                    for s in 0..m.vg.depth {
                        data[i] = mri_get_vox_val(g, self.err, &m, c, r, s, f, 0)?;
                        i += 1;
                    }
                }
            }
        }
        inp.size = size;
        inp.data = Some(data);
        let _ = self.out.write_all(b"Done packing\n");
        self.flush();
        let h = match mri_copy_header(g, &mut self.out, self.err, &m, None)? {
            larmorx_freesurfer_core::ret::Ret::New(h) => Some(h),
            _ => None,
        };
        if first {
            self.header1 = Some((m.vg.width, m.vg.height, m.vg.depth, m.nframes));
        }
        self.header = h;
        Ok(Access::Ok)
    }

    /// `CURV_arrayProgress_print(asize, current, message)` on `G_FP`.
    fn progress(&mut self, asize: i32, current: i32, message: Option<&str>) {
        if self.verbosity == 0 {
            return;
        }
        let five = (0.05 * f64::from(asize)) as i32;
        if current == 0 {
            if let Some(m) = message {
                self.gfp(&pad(G_LC, m));
            }
            self.gfp(" [");
            self.gfp_flush();
        } else if five > 0 && current % five == five - 1 {
            self.gfp("#");
            self.gfp_flush();
        }
        if current == asize.wrapping_sub(1) {
            self.gfp("] ");
            if message.is_some() {
                self.gfp("[ ok ]\n");
            }
        }
    }

    /// `CURV_fileRead(name, ...)`: the new-version curvature format (its count, faces and values
    /// per vertex kept for the output), big-endian floats.
    fn curv_file_read(
        &mut self,
        g: &mut Globals,
        name: &str,
        inp: &mut Input,
    ) -> Result<Access, Fatal> {
        let mut fp = match CFile::open(self.dir, name, "r") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                return Ok(Access::ReadError);
            }
        };
        let (_, vnum) = fread3(&mut fp);
        if vnum != NEW_VERSION_MAGIC_NUMBER {
            // FreeSurfer leaves the file open
            return Ok(Access::WrongMagic);
        }
        let nvertices = fread_int(g, &mut fp);
        self.nfaces = fread_int(g, &mut fp);
        self.vals_per_vertex = fread_int(g, &mut fp);
        // an int count as an unsigned long: a negative one asks for more than any memory
        let size = i64::from(nvertices) as u64 as usize;
        let mut data = if nvertices < 0 {
            return Err(self.no_memory(g));
        } else {
            self.alloc(g, size)?
        };
        let msg = format!("Reading {name}");
        for (i, v) in data.iter_mut().enumerate() {
            self.progress(nvertices, i as i32, Some(&msg));
            *v = fread_float(g, self.err, &mut fp);
        }
        inp.size = size;
        inp.data = Some(data);
        fp.close();
        Ok(Access::Ok)
    }

    /// `CURV_functionRunABC(F)` for the operation `op`.
    fn run_abc(&mut self, g: &mut Globals, op: Op) -> Result<(), Fatal> {
        let Some(arr2) = self.arr2.as_ref() else {
            g.notices.add(
                &FS_1122,
                format!(
                    "mris_calc: '{}' takes two inputs and was given one (FreeSurfer crashes)",
                    self.op_name
                ),
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(139));
            }
            return self.own_error_exit(
                g,
                "checking inputs",
                "this operation needs a second input.",
                10,
            );
        };
        let lib = &self.lib;
        let mut to_int = |v: f32| float_to_int(g, f64::from(v), "mris_calc (int) of a value");
        let n = self.size1;
        for ((c, &a), &b) in self.arr3[..n]
            .iter_mut()
            .zip(&self.arr1[..n])
            .zip(&arr2[..n])
        {
            *c = to_f32(binary(op, a, b, lib, &mut to_int));
        }
        Ok(())
    }

    /// `CURV_functionRunAC(F)` for a voxelwise operation of one value.
    fn run_ac_values(&mut self, op: Op) {
        let lib = &self.lib;
        let n = self.size1;
        for (c, &a) in self.arr3[..n].iter_mut().zip(&self.arr1[..n]) {
            *c = to_f32(unary(op, a, lib));
        }
    }

    /// `CURV_functionRunAC(F)` for a running statistic: the last value it returned (0 for no
    /// values).
    fn run_ac(&mut self, f: Run) -> f64 {
        let mut v = 0.0;
        for &a in &self.arr1[..self.size1] {
            v = self.st.call(f, a);
        }
        v
    }

    /// `PrintStatToFile(name, stat)`: `%lf` and a newline (`ERROR: opening <f>` on standard
    /// output when it cannot be opened; FS-1120).
    fn print_stat_to_file(&mut self, g: &mut Globals, stat: f64) {
        let name = self.file3.clone();
        match CFile::open(self.dir, &name, "w") {
            Ok(mut fp) => {
                fp.put_bytes(format!("{}\n", cfmt::f(stat, 6)).as_bytes());
                fp.close();
            }
            Err(e) => {
                g.errno = e;
                let _ = writeln!(self.out, "ERROR: opening {name}");
                self.not_written(g, &name);
            }
        }
    }

    /// FS-1120: the output `name` could not be written and FreeSurfer goes on to status 0;
    /// larmorx ends with FreeSurfer's write-access error (status 252).
    fn not_written(&mut self, g: &mut Globals, name: &str) {
        g.notices.add(
            &FS_1120,
            format!("mris_calc: '{name}' could not be written (FreeSurfer ends with status 0)"),
        );
        if !g.notices.reproduce() && self.unwritten.is_none() {
            self.unwritten = Some(name.to_owned());
        }
    }

    /// FS-1126: a count or an index that FreeSurfer passes through a `float`: whether the `float`
    /// changes it (noted).
    fn through_float(g: &mut Globals, v: i64, what: &str) -> bool {
        let f = v as f32;
        if f64::from(f) == v as f64 {
            return false;
        }
        g.notices.add(
            &FS_1126,
            format!("mris_calc: {what} {v} passes through a float in FreeSurfer, as {f}"),
        );
        true
    }

    /// `CURV_process()`: `false` for an unknown operation.
    fn process(&mut self, g: &mut Globals) -> Result<bool, Fatal> {
        self.op = operation_lookup(&self.op_name);
        self.can_write = writes_output(self.op);
        if self.label_mask {
            self.primary_backup();
            self.label_initialize(g)?;
            self.primary_compress();
        }
        let op = self.op;
        match op {
            _ if is_binary(op) => self.run_abc(g, op)?,
            Op::Sig2p
            | Op::Abs
            | Op::Log10
            | Op::Inv
            | Op::Sign
            | Op::Sqr
            | Op::Sqrt
            | Op::Not
            | Op::Log => self.run_ac_values(op),
            Op::Norm => {
                let f_min = to_f32(self.run_ac(Run::Min));
                let f_max = to_f32(self.run_ac(Run::Max));
                let f_range = sub(f_max, f_min);
                self.arr2 = Some(vec![f_min; self.size1]);
                self.run_abc(g, Op::Sub)?;
                let n = self.size1;
                self.arr1[..n].copy_from_slice(&self.arr3[..n]);
                self.arr2 = Some(vec![f_range; self.size1]);
                self.run_abc(g, Op::Div)?;
            }
            Op::Unknown => return Ok(false),
            _ => {}
        }
        if let Some(names) = libm_names(op) {
            note_glibc07(g, &self.lib.libm, names);
        }
        if op == Op::Set {
            self.file3 = self.file1.clone();
        }
        let stats = op == Op::Stats;
        if op == Op::Size || stats {
            cprintd(self.err, b"Size", self.size1 as i32);
        }
        if op == Op::Min || stats {
            let f_min = to_f32(self.run_ac(Run::Min));
            let mut mini = self.run_ac(Run::Mini) as i32;
            if Self::through_float(g, i64::from(mini), "the index of the minimum")
                && g.notices.reproduce()
            {
                mini = f64::from(mini as f32) as i32;
            }
            let text = format!("{} ({mini})", cfmt::f(f64::from(f_min), 6));
            self.cprints(g, "Min@(index)", &text)?;
            if self.file3_given {
                self.print_stat_to_file(g, f64::from(f_min));
            }
        }
        if op == Op::Max || stats {
            let f_max = to_f32(self.run_ac(Run::Max));
            let maxi = self.run_ac(Run::Maxi) as i32;
            let text = format!("{} ({maxi})", cfmt::f(f64::from(f_max), 6));
            self.cprints(g, "Max@(index)", &text)?;
            if self.file3_given {
                self.print_stat_to_file(g, f64::from(f_max));
            }
        }
        if op == Op::Mean || stats {
            let f_mean = to_f32(self.run_ac(Run::Mean));
            cprintf(self.err, b"Mean", f_mean);
            if self.file3_given {
                self.print_stat_to_file(g, f64::from(f_mean));
            }
        }
        if op == Op::Std || stats {
            let f_dev = to_f32(self.run_ac(Run::Dev));
            let f_std = sqrt(ops_div(f_dev, self.size1.wrapping_sub(1) as u64 as f32));
            cprintf(self.err, b"Std", f_std);
            if self.file3_given {
                self.print_stat_to_file(g, f64::from(f_std));
            }
        }
        if op == Op::Sum || stats {
            self.st.gsum = 0.0;
            let sum = self.run_ac(Run::GSum);
            self.st.gsum = sum;
            cprintf(self.err, b"Sum", to_f32(sum));
            if self.file3_given {
                self.print_stat_to_file(g, sum);
            }
            let neg = self.run_ac(Run::NegCount) as i32;
            let zero = self.run_ac(Run::ZeroCount) as i32;
            let pos = self.run_ac(Run::PosCount) as i32;
            for (left, v) in [
                (&b"negCount"[..], neg),
                (b"zeroCount", zero),
                (b"posCount", pos),
            ] {
                if Self::through_float(g, i64::from(v), "a count") && !g.notices.reproduce() {
                    // the exact count
                    cprint_f64(self.err, left, f64::from(v));
                } else {
                    cprintf(self.err, left, v as f32);
                }
            }
        }
        if op == Op::Prod || stats {
            self.st.gprod = 1.0;
            let prod = self.run_ac(Run::GProd);
            cprintf(self.err, b"Prod", to_f32(prod));
            if self.file3_given {
                self.print_stat_to_file(g, prod);
            }
        }
        if op == Op::Ascii {
            if !self.file3_given {
                self.file3.push_str(".ascii");
            }
            let name = self.file3.clone();
            if !self.ascii_file_write(g, &name)? {
                let _ = write!(self.out, "Write error: {name}");
                self.not_written(g, &name);
            }
        }
        if self.label_mask {
            if self.can_write {
                self.expand();
            }
            self.size3 = self.size3_copy;
        }
        if self.can_write {
            let name = self.file3.clone();
            self.file_write(g, name)?;
        }
        Ok(true)
    }

    /// `primaryCurvs_backup()`.
    fn primary_backup(&mut self) {
        self.copy1 = Some(self.arr1.clone());
        if self.has2 {
            self.copy2 = self.arr2.clone();
        }
        if self.can_write {
            self.copy3 = Some(self.arr3.clone());
            self.size3_copy = self.size3;
        }
    }

    /// `label_initialize()`: the label read (`LabelRead(NULL, name)`), the mask of its vertices
    /// over input 1's values, their indices.
    fn label_initialize(&mut self, g: &mut Globals) -> Result<(), Fatal> {
        self.size_label = 0;
        if self.verbosity != 0 {
            self.cprints(g, "Reading label...", "")?;
        }
        let name = self.label_file.clone();
        let l = label_read(g, self.env, &mut self.out, self.err, self.dir, None, &name)?;
        if self.verbosity != 0 {
            self.cprints(g, "", "ok")?;
        }
        let Some(l) = l else {
            return Err(self.error_exit(
                g,
                ERROR_NOFILE,
                &format!("{NAME}: could not read label file {name}"),
            ));
        };
        self.mask = vec![0.0; self.size1];
        let n = usize::try_from(l.n_points).unwrap_or(0);
        for (k, p) in l.lv.iter().take(n).enumerate() {
            match usize::try_from(p.vno) {
                Ok(v) if v < self.size1 => self.mask[v] = 1.0,
                _ => {
                    g.notices.add(
                        &FS_1125,
                        format!(
                            "mris_calc: label point {k} names vertex {}, outside input 1's {} values (FreeSurfer writes outside its mask)",
                            p.vno, self.size1
                        ),
                    );
                    if !g.notices.reproduce() {
                        return self.own_error_exit(
                            g,
                            "reading the label",
                            "a label point is outside input 1's values.",
                            10,
                        );
                    }
                }
            }
        }
        self.size_label = self.mask.iter().filter(|&&m| m != 0.0).count();
        self.mask_index = (0..self.size1).filter(|&i| self.mask[i] != 0.0).collect();
        // FreeSurfer keeps the indices as floats (FS-1126)
        if self.mask_index.last().is_some_and(|&i| i >= 1 << 24) {
            let exact = !g.notices.reproduce();
            for i in self.mask_index.iter_mut() {
                let f = *i as f32;
                if f64::from(f) != *i as f64 {
                    g.notices.add(
                        &FS_1126,
                        format!(
                            "mris_calc: the label's index {i} passes through a float in FreeSurfer, as {f}"
                        ),
                    );
                    if !exact {
                        *i = f as usize;
                    }
                }
            }
        }
        Ok(())
    }

    /// `array_compressUsingLabelMask` of the three arrays (`primaryCurvs_compress`).
    fn primary_compress(&mut self) {
        let mask = &self.mask;
        let comp = |src: &[f32]| -> Vec<f32> {
            src.iter()
                .zip(mask)
                .filter(|&(_, &m)| m != 0.0)
                .map(|(&v, _)| v)
                .collect()
        };
        self.arr1 = comp(self.copy1.as_deref().unwrap_or(&[]));
        self.size1 = self.size_label;
        if self.has2 {
            self.arr2 = Some(comp(self.copy2.as_deref().unwrap_or(&[])));
            self.size2 = self.size_label;
        }
        if self.can_write {
            self.arr3 = comp(self.copy3.as_deref().unwrap_or(&[]));
            self.size3 = self.size_label;
        }
    }

    /// `array_expandUsingLabelMask(&curv3, ...)`: the output's values at the label's indices of
    /// the zeroed output.
    fn expand(&mut self) {
        let mut full = self.copy3.take().unwrap_or_default();
        for (n, &i) in self.mask_index.iter().enumerate() {
            if i < full.len() {
                full[i] = self.arr3[n];
            }
        }
        self.arr3 = full;
    }

    /// `ascii_fileWrite(name, curv1)`: `%f` per line; `false` when it cannot be opened.
    fn ascii_file_write(&mut self, g: &mut Globals, name: &str) -> Result<bool, Fatal> {
        let mut fp = match CFile::open(self.dir, name, "w") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                return Ok(false);
            }
        };
        let msg = format!("Saving result to '{name}' (type = ASCII)");
        self.cprints(g, &msg, "")?;
        let n = self.size1;
        let mut buf = String::new();
        for i in 0..n {
            self.progress(n as i32, i as i32, Some(&msg));
            buf.push_str(&cfmt::f(f64::from(self.arr1[i]), 6));
            buf.push('\n');
            if buf.len() > 1 << 16 {
                fp.put_bytes(buf.as_bytes());
                buf.clear();
            }
        }
        fp.put_bytes(buf.as_bytes());
        self.cprints(g, "", "ok")?;
        fp.close();
        Ok(true)
    }

    /// `fileWrite(name, size, curv3)`: the output's type from its extension (with
    /// `--strictExtensions` an extension added where it is unknown or another's), which must be
    /// input 1's, then the volume or curvature written.
    fn file_write(&mut self, g: &mut Globals, mut name: String) -> Result<(), Fatal> {
        let mut id_ok = false;
        match fio_extension(Some(&name)) {
            Some(ext) => {
                if ext == Ft::Curvature.ext() {
                    self.etype3 = Ft::Curvature;
                    self.fstype3 = MRI_CURV_FILE;
                    id_ok = true;
                } else {
                    let (et, ty) = self.file_type_find(g, &name)?;
                    self.etype3 = et;
                    if let Some(t) = ty {
                        self.fstype3 = t;
                    }
                    if et == Ft::Unknown {
                        if self.strict {
                            let dot = self.etype1.dot_ext();
                            Self::past_buffer(g, name.len() + dot.len());
                            name.push_str(&dot);
                        }
                    } else {
                        if ext != et.ext() && self.strict {
                            let dot = et.dot_ext();
                            Self::past_buffer(g, name.len() + dot.len());
                            name.push_str(&dot);
                            self.etype3 = self.etype1;
                        }
                        id_ok = true;
                    }
                }
            }
            None => {
                let dot = self.etype1.dot_ext();
                Self::past_buffer(g, name.len() + dot.len());
                name.push_str(&dot);
            }
        }
        if !id_ok {
            self.etype3 = self.etype1;
            self.fstype3 = self.fstype1;
        }
        if self.etype3 != self.etype1 {
            return self.own_error_exit(
                g,
                "checking on output filetype",
                "it seems there is a type mismatch to the input type.",
                10,
            );
        }
        let ok = match self.etype1 {
            Ft::Volume => self.vol_file_write(g, &name)?,
            Ft::Curvature => self.curv_file_write(g, &name)?,
            _ => true,
        };
        if !ok {
            self.not_written(g, &name);
        }
        Ok(())
    }

    /// `VOL_fileWrite(name, size, data)`: a `float` volume with `Gp_MRI`'s header and pulse
    /// parameters, the values unpacked as they were packed; `MRIwrite`. `false` when the write
    /// failed (` [ error ]`).
    fn vol_file_write(&mut self, g: &mut Globals, name: &str) -> Result<bool, Fatal> {
        let Some(h) = self.header.take() else {
            self.own_error_exit(
                g,
                "checking on output volume",
                "it seems that internal volume structure is invalid.",
                10,
            )?;
            return Ok(false);
        };
        let n = [h.vg.width, h.vg.height, h.vg.depth, h.nframes]
            .iter()
            .fold(1u64, |a, &d| a.wrapping_mul(i64::from(d) as u64));
        if n != self.size3 as u64 {
            self.own_error_exit(
                g,
                "checking on output volume",
                "I found a size mismatch, internal data does not fit into vol.",
                10,
            )?;
        }
        // FreeSurfer frees its inputs before the output's allocation
        self.arr1 = Vec::new();
        self.arr2 = None;
        self.copy1 = None;
        self.copy2 = None;
        self.fits(g, "Repacking ".len() + name.len(), "a message")?;
        self.flush();
        let mut out = mri_alloc_sequence(
            g,
            &mut self.out,
            self.err,
            h.vg.width,
            h.vg.height,
            h.vg.depth,
            MRI_FLOAT,
            h.nframes,
        )?;
        mri_copy_header(g, &mut self.out, self.err, &h, Some(&mut out))?;
        mri_copy_pulse_parameters(&h, &mut out);
        let _ = self.out.write_all(b"Unpacking\n");
        self.flush();
        let mut i = 0usize;
        for f in 0..h.nframes {
            for c in 0..h.vg.width {
                for r in 0..h.vg.height {
                    for s in 0..h.vg.depth {
                        mri_set_vox_val(g, self.err, &mut out, c, r, s, f, self.arr3[i], 0)?;
                        i += 1;
                    }
                }
            }
        }
        let msg = format!(
            "Saving result to '{name}' (type = {} )",
            type_to_string(self.fstype3)
        );
        self.fits(g, msg.len(), "a message")?;
        self.cprints(g, &msg, "")?;
        self.flush();
        let ret = mri_write(
            g,
            &mut self.out,
            self.err,
            self.env,
            self.dir,
            &mut out,
            name,
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
        let ok = ret == 0;
        self.cprints(g, "", if ok { "ok" } else { "error" })?;
        Ok(ok)
    }

    /// `CURV_fileWrite(name, size, data)`: the new-version curvature format with the count, the
    /// face count and the values per vertex of the last curvature file read. `false` when it
    /// cannot be opened (nothing printed).
    fn curv_file_write(&mut self, g: &mut Globals, name: &str) -> Result<bool, Fatal> {
        let mut fp = match CFile::open(self.dir, name, "w") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                return Ok(false);
            }
        };
        let n = self.size3;
        fwrite3(&mut fp, NEW_VERSION_MAGIC_NUMBER);
        fwrite_int(&mut fp, n as i32);
        fwrite_int(&mut fp, self.nfaces);
        fwrite_int(&mut fp, self.vals_per_vertex);
        let msg = format!("Saving result to '{name}' (type = MRI_CURV_FILE)");
        self.cprints(g, &msg, "")?;
        for i in 0..n {
            self.progress(n as i32, i as i32, Some(&msg));
            fwrite_float(&mut fp, self.arr3[i]);
        }
        self.cprints(g, "", "ok")?;
        fp.close();
        Ok(true)
    }
}

/// `a / b` in `float` with SSE's NaN.
fn ops_div(a: f32, b: f32) -> f32 {
    larmorx_freesurfer_core::sse::div(a, b)
}

/// `cprintf(left, v)` with the value printed `%f` as a `double` (FS-1126's exact counts).
fn cprint_f64(err: &mut dyn Write, left: &[u8], v: f64) {
    let l = std::str::from_utf8(left).unwrap_or(" ");
    let l = if l.is_empty() { " " } else { l };
    let _ = write!(err, "{}", pad(G_LC, l));
    let _ = write!(
        err,
        "{}",
        pad(
            larmorx_freesurfer_surf::basemisc::G_RC,
            &format!(" [ {} ]\n", cfmt::f(v, 6))
        )
    );
    let _ = err.flush();
}
