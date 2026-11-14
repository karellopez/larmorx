// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! ANALYZE 7.5 volumes (`utils/mriio.cpp`, items 3.1-28 and 3.1-29): `analyzeRead`
//! ([`analyze_read`]: one `.img`/`.hdr` pair, or an SPM series of pairs with zero-padded frame
//! numbers, the voxel-to-RAS matrix from the SPM `.mat` file or the header's orientation code; a
//! NIfTI-1 pair handed to [`crate::mriio_nifti::nifti1_read`]) with `GetSPMStartFrame`,
//! `CountAnalyzeFiles`, `ReadAnalyzeHeader`, `swap_analyze_header` and `DumpAnalyzeHeader`; and the
//! writers `analyzeWrite` ([`analyze_write`]: one frame to a `.img` name, else the series),
//! `analyzeWriteFrame`, `analyzeWriteSeries` and `analyzeWrite4D` ([`analyze_write4d`], the
//! `.img` type `mri_identify` gives a new name), with `printDirCos`. Wired into
//! [`crate::mriio::mri_read`] (`MRI_ANALYZE_FILE`, `MRI_ANALYZE4D_FILE`) and
//! [`crate::mriio::mri_write_type`].
//!
//! Also niftilib's `is_nifti_file` (`packages/nifti/nifti1_io.c`, public domain) as `analyzeRead`
//! calls it (a header name ending in `.hdr`), and the path of fs.volume's `MRISeqchangeType` and
//! `MRImultiplyConst` the rescaling of a header's scale factor reaches (an integer volume to float,
//! then multiplied in place).
//!
//! The arithmetic follows the official build's object code (GCC 8.4.0, SSE2): `fabs` of a `float`
//! as a bit operation (`andps`), the scale test `|scale - 1| > FLT_EPSILON` in `float`, the
//! orientation matrix's centre column `(n * 0.5) * size` in `double`, `rint` of a `float`
//! (`rintf`, inlined) and the conversions `cvttss2si` to 32 bits.
//!
//! Defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-io.md`, Part 17):
//! FS-645 (an `SPM_START_FRAME` that is not a number), FS-646 (names past FreeSurfer's buffers),
//! FS-647 (an unsupported type's message read from freed memory), FS-648 (an orientation code
//! outside -1 to 5 without a `.mat`: a crash), FS-649 (double frames all into the first frame),
//! FS-650 (a `.mat` matrix of another shape: a crash), FS-651 (an SPM series of NIfTI pairs whose
//! first pair cannot be read: a crash), FS-652 (a big-endian header's extents and last
//! dimensions), FS-653 (an extent of exactly 32,768 written as -32,768), FS-654 (long volumes
//! written from half their bytes), FS-655 (the writers' closes unchecked), FS-656 (float-to-int
//! conversions outside the range), FS-457 (unsigned 32-bit values read as `int`s: kept), FS-58
//! (writing a volume without voxels).

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::{
    FS_58, FS_457, FS_645, FS_646, FS_647, FS_648, FS_649, FS_650, FS_651, FS_652, FS_653, FS_654,
    FS_655, FS_656,
};
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal, NO_ERROR,
};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{
    matrix_determinant, matrix_identity, matrix_inverse, matrix_print,
};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::{sse, utils};
use larmorx_freesurfer_mri::info::{mri_dump, mri_limits};
use larmorx_freesurfer_mri::mri::{
    self, MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri, MriFrame,
};
use larmorx_freesurfer_mri::tkreg::mtx_crs1_to_crs0;
use larmorx_freesurfer_mri::vol_geom;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_io::znz::ZnzFile;

use crate::bfileio;
use crate::cfile::CFile;
use crate::fio;
use crate::identify::NIFTI1_FILE;
use crate::matfile;
use crate::mriio_nifti::{narrow, widen};

const DT_UNSIGNED_CHAR: i16 = 2;
const DT_SIGNED_SHORT: i16 = 4;
const DT_SIGNED_INT: i16 = 8;
const DT_FLOAT: i16 = 16;
const DT_DOUBLE: i16 = 64;
const DT_UINT16: i16 = 512;
const DT_UINT32: i16 = 768;

/// `STRLEN`: the writers' name buffers.
const STRLEN: usize = 1024;
/// The reader's name buffers (`char fname[1000]`, `hdrfile[1000]`, ...).
const READ_NAME: usize = 1000;

/// The standard error line of glibc's buffer-overflow check (`_FORTIFY_SOURCE`), which ends the
/// program with `abort` (status 134).
const OVERFLOW: &[u8] = b"*** buffer overflow detected ***: terminated\n";

/// `dsr` (`include/analyze.h`): the ANALYZE 7.5 header as it stands in memory, 348 bytes of
/// x86-64's little-endian fields: `hk` (`sizeof_hdr` 0, `data_type` 4, `db_name` 14, `extents`
/// 32, `session_error` 36, `regular` 38, `hkey_un0` 39), `dime` (`dim` 40, `vox_units` 56,
/// `cal_units` 60, `unused1` 68, `datatype` 70, `bitpix` 72, `dim_un0` 74, `pixdim` 76,
/// `vox_offset` 108, `roi_scale` 112, `funused1` 116, `funused2` 120, `cal_max` 124, `cal_min`
/// 128, `compressed` 132, `verified` 136, `glmax` 140, `glmin` 144), `hist` (`descrip` 148,
/// `aux_file` 228, `orient` 252, `originator` 253, ..., `views` 316 to `smin` 344).
#[derive(Clone)]
pub struct Dsr(pub [u8; 348]);

impl Dsr {
    fn i16_at(&self, o: usize) -> i16 {
        i16::from_le_bytes([self.0[o], self.0[o + 1]])
    }
    fn i32_at(&self, o: usize) -> i32 {
        i32::from_le_bytes([self.0[o], self.0[o + 1], self.0[o + 2], self.0[o + 3]])
    }
    fn f32_at(&self, o: usize) -> f32 {
        f32::from_bits(self.i32_at(o) as u32)
    }
    fn set_i16(&mut self, o: usize, v: i16) {
        self.0[o..o + 2].copy_from_slice(&v.to_le_bytes());
    }
    fn set_i32(&mut self, o: usize, v: i32) {
        self.0[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn set_f32(&mut self, o: usize, v: f32) {
        self.0[o..o + 4].copy_from_slice(&v.to_bits().to_le_bytes());
    }
    /// The C string at `o`: its bytes up to a NUL (`%s` runs on into the fields after it; the
    /// header's memory is followed by zero bytes).
    fn cstr(&self, o: usize) -> &[u8] {
        let end = self.0[o..]
            .iter()
            .position(|&b| b == 0)
            .map_or(348, |p| o + p);
        &self.0[o..end]
    }
    pub fn sizeof_hdr(&self) -> i32 {
        self.i32_at(0)
    }
    pub fn dim(&self, i: usize) -> i16 {
        self.i16_at(40 + 2 * i)
    }
    pub fn datatype(&self) -> i16 {
        self.i16_at(70)
    }
    pub fn pixdim(&self, i: usize) -> f32 {
        self.f32_at(76 + 4 * i)
    }
    pub fn vox_offset(&self) -> f32 {
        self.f32_at(108)
    }
    pub fn roi_scale(&self) -> f32 {
        self.f32_at(112)
    }
    pub fn orient(&self) -> i8 {
        self.0[252] as i8
    }
}

/// `swap_analyze_header(hdr)`: the fields reversed in place. FS-652: FreeSurfer swaps `extents`
/// (an `int`) as a 16-bit number (its low half reversed, sign-extended) and leaves `dim[5]` to
/// `dim[7]` as they were; by default every field is reversed whole. Only the header dump prints
/// those fields: the notice is given when it does and their values differ.
fn swap_analyze_header(g: &mut Globals, h: &mut Dsr) {
    let rev = |h: &mut Dsr, o: usize, n: usize| h.0[o..o + n].reverse();
    let fix = g.notices.fix();
    rev(h, 0, 4);
    let ext = h.i32_at(32);
    let short = i32::from(i16::from_be_bytes([h.0[32], h.0[33]]));
    let whole = ext.swap_bytes();
    let late_dims = (5..8).any(|i| h.dim(i).swap_bytes() != h.dim(i));
    if (short != whole || late_dims) && g.gdiag_no > 0 {
        g.notices.add(
            &FS_652,
            "a big-endian ANALYZE header: its extents swapped as a 16-bit number and dim[5] to dim[7] left unswapped in the header dump (FreeSurfer)",
        );
    }
    h.set_i32(32, if fix { whole } else { short });
    rev(h, 36, 2);
    let ndims = if fix { 8 } else { 5 };
    for i in 0..ndims {
        rev(h, 40 + 2 * i, 2);
    }
    for o in [68, 70, 72, 74] {
        rev(h, o, 2);
    }
    for o in [108, 112, 116, 120, 124, 128, 132, 136, 144, 140] {
        rev(h, o, 4);
    }
    for i in 0..8 {
        rev(h, 76 + 4 * i, 4);
    }
    for o in (316..348).step_by(4) {
        rev(h, o, 4);
    }
    for i in 0..5 {
        h.0.swap(253 + 2 * i, 253 + 2 * i + 1);
    }
}

/// `%f`.
fn c_f(v: f64) -> String {
    cfmt::f(v, 6)
}

/// `DumpAnalyzeHeader(fp, hdr)`: the header's fields, one per line.
fn dump_analyze_header(fp: &mut dyn Write, h: &Dsr) {
    let mut t: Vec<u8> = Vec::new();
    let mut line = |label: &str, v: &[u8]| {
        t.extend_from_slice(label.as_bytes());
        t.extend_from_slice(v);
        t.push(b'\n');
    };
    line("Header Key", b"");
    line("  sizeof_hdr    ", h.sizeof_hdr().to_string().as_bytes());
    line("  data_type     ", h.cstr(4));
    line("  db_name       ", h.cstr(14));
    line("  extents       ", h.i32_at(32).to_string().as_bytes());
    line("  session_error ", h.i16_at(36).to_string().as_bytes());
    line("  regular       ", &[h.0[38]]);
    line("  hkey_un0      ", &[h.0[39]]);
    line("Image Dimension ", b"");
    let dims: Vec<String> = (0..8).map(|i| h.dim(i).to_string()).collect();
    line("  dim ", format!("{} ", dims.join(" ")).as_bytes());
    let pix: Vec<String> = (0..8).map(|i| c_f(widen(h.pixdim(i)))).collect();
    line("  pixdim ", format!("{} ", pix.join(" ")).as_bytes());
    line("  vox_units     ", h.cstr(56));
    line("  cal_units     ", h.cstr(60));
    line("  datatype      ", h.datatype().to_string().as_bytes());
    line("  bitpix        ", h.i16_at(72).to_string().as_bytes());
    line(
        "  glmax         ",
        cfmt::g(f64::from(h.i32_at(140) as f32), 6).as_bytes(),
    );
    line(
        "  glmin         ",
        cfmt::g(f64::from(h.i32_at(144) as f32), 6).as_bytes(),
    );
    line("  orient        ", h.orient().to_string().as_bytes());
    let _ = fp.write_all(&t);
}

/// The C library's overflow check on a name of `len` bytes written into a buffer of `size`:
/// FS-646. `Ok(())` when it fits, or by default; with `reproduce` the abort.
fn fits(
    g: &mut Globals,
    err: &mut dyn Write,
    who: &str,
    len: usize,
    size: usize,
) -> Result<(), Fatal> {
    if len < size {
        return Ok(());
    }
    g.notices.add(
        &FS_646,
        format!(
            "{who}: a name of {len} bytes, which FreeSurfer's {size}-byte buffer cannot hold (the program aborts)"
        ),
    );
    if g.notices.reproduce() {
        let _ = err.write_all(OVERFLOW);
        return Err(Fatal::Exit(134));
    }
    Ok(())
}

/// A name of bytes as the port's file functions take it.
fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// `fopen(name, "r")` and `fclose`: whether it opens; a failure sets `errno`.
fn opens(g: &mut Globals, dir: Option<&Path>, name: &str) -> bool {
    match CFile::open(dir, name, "r") {
        Ok(f) => {
            f.close();
            true
        }
        Err(e) => {
            g.errno = e;
            false
        }
    }
}

/// `C's %0<w>d` of `n`.
fn zero_padded(n: i32, w: i32) -> String {
    format!("{n:0width$}", width = w.max(0) as usize)
}

/// `GetSPMStartFrame()`: 1 without `SPM_START_FRAME`; else its number (`sscanf("%d")`, printed
/// as `Using env var SPM_START_FRAME = <n>`). FS-645: a text that does not start with a number
/// leaves FreeSurfer's number unwritten (memory contents): 1 in both modes, noted.
fn get_spm_start_frame(g: &mut Globals, out: &mut dyn Write, env: &Environment) -> i32 {
    let Some(s) = env.get("SPM_START_FRAME") else {
        return 1;
    };
    let bytes = s.as_encoded_bytes().to_vec();
    let mut b = Bytes::new(&bytes);
    let mut sc = Scanf::new(&mut b);
    let mut errno = g.errno;
    let v = sc.d(&mut errno, true);
    sc.finish(&mut errno);
    g.errno = errno;
    let start = match v {
        Some(v) => v,
        None => {
            g.notices.add(
                &FS_645,
                format!(
                    "SPM_START_FRAME '{}' is not a number: the frames counted from 1 (FreeSurfer: memory contents)",
                    String::from_utf8_lossy(&bytes)
                ),
            );
            1
        }
    };
    let _ = writeln!(out, "Using env var SPM_START_FRAME = {start}");
    start
}

/// `CountAnalyzeFiles(analyzefname, nzpad, &stem)`: the number of files the name stands for and
/// its stem. A name ending in `.img` is one file (with `nzpad` 0 or more: `ERROR:
/// CountAnalyzeFiles: file with .img extension specified        with zero pad variable.` on
/// standard output and -1); otherwise, with `nzpad` below 0, the file `<stem>.img`; else the
/// series `<stem><n>.img`, `n` zero-padded to `nzpad` digits from the SPM start frame on, counted
/// while they open. A single file: 1 when `<stem>.img` opens, else 0 (`errno` from the failure).
fn count_analyze_files(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    name: &[u8],
    nzpad: i32,
) -> Result<(i32, Vec<u8>), Fatal> {
    let len = name.len();
    let (ncopy, single) = if len > 4 && &name[len - 4..] == b".img" {
        if nzpad >= 0 {
            let _ = out.write_all(
                b"ERROR: CountAnalyzeFiles: file with .img extension specified        with zero pad variable.\n",
            );
            return Ok((-1, Vec::new()));
        }
        (len - 4, true)
    } else {
        (len, nzpad < 0)
    };
    let stem = name[..ncopy].to_vec();
    if single {
        fits(g, err, "CountAnalyzeFiles", ncopy + 4, READ_NAME)?;
        let fname = text(&[stem.as_slice(), b".img"].concat());
        return Ok((i32::from(opens(g, dir, &fname)), stem));
    }
    // the format `<stem>%0<nzpad>d.img`, then each frame's name
    fits(
        g,
        err,
        "CountAnalyzeFiles",
        ncopy + 7 + nzpad.to_string().len(),
        READ_NAME,
    )?;
    let startframe = get_spm_start_frame(g, out, env);
    let mut nfiles = 0i32;
    loop {
        let num = zero_padded(nfiles.wrapping_add(startframe), nzpad);
        fits(
            g,
            err,
            "CountAnalyzeFiles",
            ncopy + num.len() + 4,
            READ_NAME,
        )?;
        let fname = text(&[stem.as_slice(), num.as_bytes(), b".img"].concat());
        if !opens(g, dir, &fname) {
            break;
        }
        nfiles += 1;
    }
    Ok((nfiles, stem))
}

/// niftilib's `nifti_find_file_extension(name)`: the byte where a NIfTI extension (`.hdr`,
/// `.img`, `.nia`, `.nii`, each also with `.gz`) starts.
fn nifti_ext(name: &[u8]) -> Option<usize> {
    let len = name.len();
    if len < 4 {
        return None;
    }
    if [&b".hdr"[..], b".img", b".nia", b".nii"].contains(&&name[len - 4..]) {
        return Some(len - 4);
    }
    if len < 7 {
        return None;
    }
    [&b".hdr.gz"[..], b".img.gz", b".nia.gz", b".nii.gz"]
        .contains(&&name[len - 7..])
        .then_some(len - 7)
}

/// niftilib's `is_nifti_file(hname)` (`packages/nifti/nifti1_io.c`, public domain): 1 for a
/// NIfTI-1 single file, 2 for a NIfTI-1 pair's header, 0 for an ANALYZE header (`sizeof_hdr` 348
/// in either byte order), -1 otherwise. The header file is the name itself when it exists (and is
/// not an `.img` name), else the stem with `.nii`, `.nii.gz`, `.hdr`, `.hdr.gz` (`.hdr` first for
/// an `.img` name), the first that opens (none: `** no header file found for '<hname>'` on standard
/// error, niftilib's debug level being 1); its first 348 bytes (inflated for a `.gz` name). A
/// failed open sets `errno`.
fn is_nifti_file(g: &mut Globals, err: &mut dyn Write, dir: Option<&Path>, hname: &[u8]) -> i32 {
    let ext = nifti_ext(hname);
    if hname.is_empty() || ext == Some(0) {
        return -1;
    }
    let base = &hname[..ext.unwrap_or(hname.len())];
    let is_img = ext.is_some_and(|e| hname[e..].starts_with(b".img"));
    let found = if ext.is_some() && opens(g, dir, &text(hname)) && !is_img {
        Some(hname.to_vec())
    } else {
        let order: [&[u8]; 2] = if is_img {
            [b".hdr", b".nii"]
        } else {
            [b".nii", b".hdr"]
        };
        let mut found = None;
        'all: for e in order {
            for gz in [&b""[..], b".gz"] {
                let n = [base, e, gz].concat();
                if opens(g, dir, &text(&n)) {
                    found = Some(n);
                    break 'all;
                }
            }
        }
        found
    };
    let Some(hdrname) = found else {
        let mut t = b"** no header file found for '".to_vec();
        t.extend_from_slice(hname);
        t.extend_from_slice(b"'\n");
        let _ = err.write_all(&t);
        return -1;
    };
    let mut h = [0u8; 348];
    let n = if hdrname.len() >= 3 && hdrname.ends_with(b".gz") {
        let path = utils::resolve(dir, &text(&hdrname));
        let Some(mut fp) = ZnzFile::open(path.as_os_str().as_encoded_bytes(), b"rb", true) else {
            return -1;
        };
        let n = fio::fs_znzread(&mut fp, &mut h, 1, 348);
        fp.close();
        n.max(0) as usize
    } else {
        let Ok(mut fp) = CFile::open(dir, &text(&hdrname), "rb") else {
            return -1;
        };
        let n = fp.fread(&mut h, 1, 348);
        if let Some(e) = fp.read_errno() {
            g.errno = e;
        }
        fp.close();
        n
    };
    if n < 348 {
        return -1;
    }
    let m = &h[344..348];
    if m[0] == b'n' && m[3] == 0 && (m[1] == b'i' || m[1] == b'+') && (b'1'..=b'9').contains(&m[2])
    {
        return if m[1] == b'+' { 1 } else { 2 };
    }
    let size = i32::from_le_bytes([h[0], h[1], h[2], h[3]]);
    i32::from(size == 348 || size.swap_bytes() == 348) - 1
}

/// `ReadAnalyzeHeader(hdrfile, &swap, &mritype, &bytes_per_voxel)`: the header (a file cut short
/// gives `ReadAnalyzeHeader(): could not read file` as an error and the bytes it has, the rest 0),
/// byte-swapped when `sizeof_hdr` is not 348; its volume type: unsigned char, short, unsigned short
/// (`INFO: This is an unsigned short.` on standard error), int (signed or unsigned 32-bit), float,
/// or float from double; another type is `ReadAnalyzeHeader: unsupported data type <n>`
/// (`ERROR_UNSUPPORTED`; FS-647). A header that does not open: `ERROR: ReadAnalyzeHeader():
/// cannot open <name>` on standard output.
fn read_analyze_header(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    hdrfile: &str,
) -> Option<(Dsr, bool, i32, usize)> {
    let mut fp = match CFile::open(dir, hdrfile, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            let _ = writeln!(out, "ERROR: ReadAnalyzeHeader(): cannot open {hdrfile}");
            return None;
        }
    };
    let mut h = Dsr([0u8; 348]);
    if fp.fread(&mut h.0, 348, 1) != 1 {
        if let Some(e) = fp.read_errno() {
            g.errno = e;
        }
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            "ReadAnalyzeHeader(): could not read file",
        );
    }
    fp.close();
    let swap = h.sizeof_hdr() != 348;
    if swap {
        swap_analyze_header(g, &mut h);
    }
    let (mritype, bpv) = match h.datatype() {
        DT_UNSIGNED_CHAR => (MRI_UCHAR, 1),
        DT_SIGNED_SHORT => (MRI_SHORT, 2),
        DT_UINT16 => {
            let _ = err.write_all(b"INFO: This is an unsigned short.\n");
            (MRI_USHRT, 2)
        }
        DT_SIGNED_INT | DT_UINT32 => (MRI_INT, 4),
        DT_FLOAT => (MRI_FLOAT, 4),
        DT_DOUBLE => (MRI_FLOAT, 8),
        dt => {
            // FS-647: FreeSurfer prints the type from the header it has just freed (glibc keeps
            // those bytes: the same number)
            g.notices.add(
                &FS_647,
                "ReadAnalyzeHeader's message of an unsupported type read from freed memory (the same number in FreeSurfer's runs)",
            );
            g.errno = 0;
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                &format!("ReadAnalyzeHeader: unsupported data type {dt}"),
            );
            return None;
        }
    };
    Some((h, swap, mritype, bpv))
}

/// `(int)f` (`cvttss2si`): FS-656: outside the int range (or NaN) FreeSurfer gets the lowest int;
/// by default the value clamped (NaN 0), noted.
fn float_to_int(g: &mut Globals, f: f32, what: &str) -> i32 {
    if !f.is_nan() && f >= -2_147_483_648.0 && f < 2_147_483_648.0 {
        return f as i32;
    }
    g.notices.add(
        &FS_656,
        format!("{what}: {f} converted to an int, outside the int range"),
    );
    if g.notices.reproduce() {
        i32::MIN
    } else if f.is_nan() {
        0
    } else if f > 0.0 {
        i32::MAX
    } else {
        i32::MIN
    }
}

/// `(short)rint(f)` of a `float` (`rintf` inlined, then `cvttss2si` to 32 bits and its low 16):
/// FS-656 outside the `short` range (NaN included); by default clamped (NaN 0), noted.
fn rint_to_short(g: &mut Globals, f: f32, what: &str) -> i16 {
    let r = if f.abs() < 8_388_608.0 {
        f.round_ties_even()
    } else {
        f
    };
    if !r.is_nan() && (-32768.0..=32767.0).contains(&r) {
        return r as i16;
    }
    g.notices.add(
        &FS_656,
        format!("{what}: {f} rounded to a short, outside the short range"),
    );
    if g.notices.reproduce() {
        let i = if !r.is_nan() && r >= -2_147_483_648.0 && r < 2_147_483_648.0 {
            r as i32
        } else {
            i32::MIN
        };
        i as i16
    } else if r.is_nan() {
        0
    } else if r > 0.0 {
        i16::MAX
    } else {
        i16::MIN
    }
}

/// The voxel-to-RAS matrix of an orientation code (`hist.orient`, Mayo's 0 to 5, -1 unknown) and
/// its name: a 4 x 4 of the voxel sizes, signs and axes, its centre column `(n * 0.5) * size` in
/// `double` (`n` the extent; a negative size `-size`), as `analyzeRead` builds it.
fn orient_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Mri,
    orient: i8,
) -> Result<(Matrix, &'static str), Fatal> {
    let (xs, ys, zs) = (m.vg.xsize, m.vg.ysize, m.vg.zsize);
    let neg = |v: f32| f32::from_bits(v.to_bits() ^ 0x8000_0000);
    let half = |n: i32, s: f32| narrow(sse::mul64(f64::from(n) * 0.5, widen(s)));
    let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
    let mut t = matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)?.expect("a real 4 x 4 matrix");
    let (cells, name): (Vec<(i32, i32, f32)>, &str) = match orient {
        0 => (
            vec![
                (1, 1, neg(xs)),
                (2, 2, ys),
                (3, 3, zs),
                (1, 4, half(w, xs)),
                (2, 4, half(h, neg(ys))),
                (3, 4, half(d, neg(zs))),
            ],
            "transverse unflipped (default)",
        ),
        1 => (
            vec![
                (1, 1, neg(xs)),
                (2, 3, zs),
                (3, 2, ys),
                (1, 4, half(w, xs)),
                (2, 4, half(d, neg(zs))),
                (3, 4, half(h, neg(ys))),
            ],
            "coronal unflipped",
        ),
        2 => (
            vec![
                (1, 3, neg(zs)),
                (2, 1, xs),
                (3, 2, ys),
                (1, 4, half(d, zs)),
                (2, 4, half(w, neg(xs))),
                (3, 4, half(h, neg(ys))),
            ],
            "sagittal unflipped",
        ),
        4 => (
            vec![
                (1, 1, neg(xs)),
                (2, 3, zs),
                (3, 2, neg(ys)),
                (1, 4, half(w, xs)),
                (2, 4, half(d, neg(zs))),
                (3, 4, half(h, ys)),
            ],
            "coronal flipped",
        ),
        5 => (
            vec![
                (1, 3, neg(zs)),
                (2, 1, xs),
                (3, 2, neg(ys)),
                (1, 4, half(d, zs)),
                (2, 4, half(w, neg(xs))),
                (3, 4, half(h, ys)),
            ],
            "sagittal flipped",
        ),
        // 3, and -1 (unknown: assumed transverse flipped)
        _ => (
            vec![
                (1, 1, neg(xs)),
                (2, 2, neg(ys)),
                (3, 3, zs),
                (1, 4, half(w, xs)),
                (2, 4, half(h, ys)),
                (3, 4, half(d, neg(zs))),
            ],
            "transverse flipped",
        ),
    };
    for (r, c, v) in cells {
        t.set_relt(r, c, v);
    }
    t.set_relt(4, 4, 1.0);
    Ok((t, name))
}

/// The path of `MRISeqchangeType(vol, MRI_FLOAT, 0, 0, 0)` the rescaling reaches (fs.volume's
/// `mri_seq_change_type`): an unsigned char, short, unsigned short or int volume as one volume of
/// `depth x frames` slices, its header copied (`MRIcopyHeader`), its values copied unscaled
/// (`MRIgetVoxVal`, `MRIsetVoxVal`), then its extent and frames back (the frames' records new:
/// zeros and a 4 x 4 matrix each).
fn seq_to_float(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vol: &mut Mri,
) -> Result<Mri, Fatal> {
    let (w, h, nslices, nframes) = (vol.vg.width, vol.vg.height, vol.vg.depth, vol.nframes);
    let d = nslices.wrapping_mul(nframes);
    vol.vg.depth = d;
    vol.nframes = 1;
    let r = (|| -> Result<Mri, Fatal> {
        let mut dest = mri::mri_alloc_sequence(g, out, err, w, h, d, MRI_FLOAT, 1)?;
        mri::mri_copy_header(g, out, err, vol, Some(&mut dest))?;
        dest.mtype = MRI_FLOAT;
        for k in 0..d {
            for j in 0..h {
                for i in 0..w {
                    let v = mri_get_vox_val(g, err, vol, i, j, k, 0, 0)?;
                    mri_set_vox_val(g, err, &mut dest, i, j, k, 0, v, 0)?;
                }
            }
        }
        Ok(dest)
    })();
    vol.vg.depth = nslices;
    vol.nframes = nframes;
    let mut m = r?;
    m.vg.depth = nslices;
    m.nframes = nframes;
    m.vox_per_row = m.vg.width as u64;
    m.vox_per_slice = m.vox_per_row.wrapping_mul(m.vg.height as u64);
    m.vox_per_vol = m.vox_per_slice.wrapping_mul(m.vg.depth as u64);
    m.vox_total = m.vox_per_vol.wrapping_mul(nframes as u64);
    m.frames = (0..nframes.max(0))
        .map(|_| MriFrame {
            m_ras2vox: Some(Matrix::real(4, 4)),
            ..MriFrame::default()
        })
        .collect();
    Ok(m)
}

/// `MRImultiplyConst(mri, scale, mri)` (fs.volume's `mri2.cpp`) in place on a float volume: each
/// voxel `v * (double)scale` stored as a `float`.
fn multiply_const(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &mut Mri,
    scale: f32,
) -> Result<(), Fatal> {
    let k = widen(scale);
    for c in 0..m.vg.width {
        for r in 0..m.vg.height {
            for s in 0..m.vg.depth {
                for f in 0..m.nframes {
                    let v = mri_get_vox_val(g, err, m, c, r, s, f, 0)?;
                    let x = narrow(sse::mul64(widen(v), k));
                    mri_set_vox_val(g, err, m, c, r, s, f, x, 0)?;
                }
            }
        }
    }
    Ok(())
}

/// `analyzeRead(fname, read_volume)`: an ANALYZE 7.5 volume. See `specs/freesurfer-io.md` §101.
///
/// The files: [`count_analyze_files`] with `N_Zero_Pad_Input` (none: `ERROR: analyzeRead():
/// cannot find any files for <name>` on standard output); the header `<stem>.hdr` (a series:
/// `<stem><start frame>.hdr`), the SPM matrix `<stem>.mat`, the voxels `<stem>.img`. A NIfTI-1
/// pair's header (niftilib's `is_nifti_file` 2) of a single file is read by `nifti1Read`. The
/// frames: `dim[4]` (0: 1) of a single file, the number of files of a series; a volume of one
/// column or one row whose `.img` is not `columns x rows x slices x frames` voxels long takes its
/// other extent from the file's length (`INFO: <img> really has <n> rows/cols`). The voxel sizes
/// `|pixdim|`, the TR `1000 pixdim[4]`. The voxel-to-RAS matrix: the `.mat` file's matrix times
/// FreeSurfer's 1-based-to-0-based shift (the RAS flag set), or (no `.mat`, or one that MATLAB's
/// reader refuses: two `WARNING` lines) the orientation code's (FS-648 outside -1 to 5); an SPM
/// series of NIfTI-1 pairs takes its first pair's (FS-651). The voxels: row by row from the
/// header's voxel offset (FS-656), byte-swapped with the header, doubles as floats (FS-649);
/// a short file prints the frame, slice and row, `MRIdump` and `analyzeRead2(): error reading from
/// file <img>` (`ERROR_BADFILE`). Then `  analyzeRead() roi_scale <%13.9f>` on standard output, and
/// a scale neither 0 nor 1 (unless `FS_ANALYZE_NO_RESCALE`) multiplies the voxels (an integer
/// volume made float first). With `Gdiag_no` above 0 the header dump and `INFO: analyzeRead():
/// min = <g>, max = <g>`.
#[allow(clippy::too_many_arguments)]
pub fn analyze_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    let nzpad = g.n_zero_pad_input;
    let startframe = get_spm_start_frame(g, out, env);
    let (nfiles, stem) = count_analyze_files(g, out, err, env, dir, fname.as_bytes(), nzpad)?;
    if nfiles < 0 {
        return Ok(None);
    }
    if nfiles == 0 {
        let _ = writeln!(
            out,
            "ERROR: analyzeRead(): cannot find any files for {fname}"
        );
        return Ok(None);
    }
    let frame_name = |frame: i32, ext: &[u8]| -> String {
        let mut n = stem.clone();
        if nzpad > -1 {
            n.extend_from_slice(zero_padded(frame, nzpad).as_bytes());
        }
        n.push(b'.');
        n.extend_from_slice(ext);
        text(&n)
    };
    let hdrfile = frame_name(startframe, b"hdr");
    let matfile = frame_name(startframe, b"mat");
    let mut imgfile = frame_name(startframe, b"img");
    let nifticode = is_nifti_file(g, err, dir, hdrfile.as_bytes());
    if g.gdiag != 0 {
        let _ = writeln!(out, "nifticode = {nifticode}");
    }
    if nifticode == 2 && nfiles == 1 {
        return crate::mriio_nifti::nifti1_read(g, out, err, dir, &hdrfile, read_volume);
    }
    let Some((hdr, swap, mut mritype, mut bpv)) = read_analyze_header(g, out, err, dir, &hdrfile)
    else {
        return Ok(None);
    };
    if mritype == MRI_FLOAT && env.get("ATROPHY_SIMULATOR").is_some() {
        let _ = out.write_all(b"adjusting analyze type to uchar to correct simulator bug\n");
        mritype = MRI_UCHAR;
        bpv = 1;
    }
    if g.gdiag_no > 0 {
        dump_analyze_header(out, &hdr);
    }
    let nframes = if nfiles == 1 {
        match i32::from(hdr.dim(4)) {
            0 => 1,
            n => n,
        }
    } else {
        nfiles
    };
    let (mut ncols, mut nrows, nslcs) = (
        i32::from(hdr.dim(1)),
        i32::from(hdr.dim(2)),
        i32::from(hdr.dim(3)),
    );
    let nv = ncols
        .wrapping_mul(nrows)
        .wrapping_mul(nslcs)
        .wrapping_mul(nframes);
    if ncols == 1 || nrows == 1 {
        // lstat: the file's own length (a link's is its target's name)
        let size = match std::fs::symlink_metadata(utils::resolve(dir, &imgfile)) {
            Ok(md) => md.len() as i64,
            Err(e) => {
                g.errno = log::errno_of(&e).unwrap_or(2);
                -1
            }
        };
        if size >= 0 && size != i64::from(nv.wrapping_mul(bpv as i32)) {
            let nreal = (size / bpv as i64) as i32;
            if ncols == 1 {
                nrows = nreal;
            } else {
                ncols = nreal;
            }
            let _ = writeln!(out, "INFO: {imgfile} really has {nreal} rows/cols");
        }
    }
    let mut m = if read_volume {
        mri::mri_alloc_sequence(g, out, err, ncols, nrows, nslcs, mritype, nframes)?
    } else {
        let mut m = mri::mri_alloc_header(g, out, err, ncols, nrows, nslcs, mritype, nframes)?;
        m.nframes = nframes;
        m
    };
    let fabs = |v: f32| f32::from_bits(v.to_bits() & 0x7fff_ffff);
    m.vg.xsize = fabs(hdr.pixdim(1));
    m.vg.ysize = fabs(hdr.pixdim(2));
    m.vg.zsize = fabs(hdr.pixdim(3));
    m.tr = sse::mul(1000.0, hdr.pixdim(4));

    let t: Option<Matrix> = if nifticode != 2 {
        let mut t = None;
        let mut cantread = false;
        if utils::file_exists(dir, &matfile) {
            match matfile::matlab_read(g, out, err, dir, &matfile)? {
                None => {
                    let _ = writeln!(
                        out,
                        "WARNING: analyzeRead(): matfile {matfile} exists but could not read ... "
                    );
                    let _ = out
                        .write_all(b"  may not be matlab4 mat file ... proceeding without it.\n");
                    cantread = true;
                }
                Some(t1) => {
                    if t1.cols != 4 || t1.rows < 3 {
                        g.notices.add(
                            &FS_650,
                            format!(
                                "analyzeRead: {matfile} holds a {} x {} matrix (FreeSurfer crashes)",
                                t1.rows, t1.cols
                            ),
                        );
                        if g.notices.reproduce() {
                            let q = mtx_crs1_to_crs0(g, out, err, None)?.unwrap_new();
                            let _ = matrix_multiply(g, out, err, Some(&t1), Some(&q), None)?;
                            return Err(Fatal::Exit(139));
                        }
                        g.errno = 0;
                        log::error_printf(
                            g,
                            err,
                            ERROR_BADFILE,
                            &format!(
                                "analyzeRead(): {matfile} holds a {} x {} matrix, not a voxel-to-RAS matrix (4 columns, at least 3 rows)",
                                t1.rows, t1.cols
                            ),
                        );
                        return Ok(None);
                    }
                    let q = mtx_crs1_to_crs0(g, out, err, None)?.unwrap_new();
                    t = matrix_multiply(g, out, err, Some(&t1), Some(&q), None)?.into_new();
                    m.vg.ras_good_flag = 1;
                }
            }
        }
        if !utils::file_exists(dir, &matfile) || cantread {
            let mut orient = hdr.orient();
            if !(-1..=5).contains(&orient) {
                g.notices.add(
                    &FS_648,
                    format!(
                        "analyzeRead: the orientation code {orient} (none of -1 to 5) without a .mat file: assumed transverse flipped, as for -1 (FreeSurfer crashes)"
                    ),
                );
                if g.notices.reproduce() {
                    // the line FreeSurfer prints, its direction name never written, then the crash
                    let _ = write!(
                        err,
                        "-----------------------------------------------------------------\nINFO: could not find {matfile} file for direction cosine info.\nINFO: use Analyze 7.5 hdr->hist.orient value: {orient}, .\nINFO: if not valid, please provide the information in {matfile} file\n-----------------------------------------------------------------\n"
                    );
                    return Err(Fatal::Exit(139));
                }
                orient = -1;
            }
            let (tt, direction) = orient_matrix(g, out, err, &m, orient)?;
            if orient == -1 {
                m.vg.ras_good_flag = 0;
                let _ = write!(
                    err,
                    "WARNING: could not find {matfile} file for direction cosine info.\nWARNING: Analyze 7.5 hdr->hist.orient value = {}, not valid.\nWARNING: assuming {direction}\n",
                    hdr.orient()
                );
            } else {
                m.vg.ras_good_flag = 1;
                let _ = write!(
                    err,
                    "-----------------------------------------------------------------\nINFO: could not find {matfile} file for direction cosine info.\nINFO: use Analyze 7.5 hdr->hist.orient value: {orient}, {direction}.\nINFO: if not valid, please provide the information in {matfile} file\n-----------------------------------------------------------------\n"
                );
            }
            t = Some(tt);
        }
        t
    } else {
        // one frame's pair read as NIfTI for its voxel-to-RAS matrix
        match crate::mriio::mri_read_header(g, out, err, env, dir, &imgfile, NIFTI1_FILE)? {
            Some(tmp) => Some(vol_geom::xfm_crs2xyz(g, out, err, &tmp.vg, 0, false)?),
            None => {
                g.notices.add(
                    &FS_651,
                    format!(
                        "analyzeRead: the SPM series' pair {imgfile} could not be read as NIfTI: no volume (FreeSurfer crashes)"
                    ),
                );
                if g.notices.reproduce() {
                    return Err(Fatal::Exit(139));
                }
                return Ok(None);
            }
        }
    };
    let Some(t) = t else {
        // the product of a .mat matrix that was not multiplied: unreachable after FS-650's check
        return Ok(None);
    };
    let rel = |r: i32, c: i32| t.relt(r, c);
    m.vg.x_r = sse::div(rel(1, 1), m.vg.xsize);
    m.vg.x_a = sse::div(rel(2, 1), m.vg.xsize);
    m.vg.x_s = sse::div(rel(3, 1), m.vg.xsize);
    m.vg.y_r = sse::div(rel(1, 2), m.vg.ysize);
    m.vg.y_a = sse::div(rel(2, 2), m.vg.ysize);
    m.vg.y_s = sse::div(rel(3, 2), m.vg.ysize);
    m.vg.z_r = sse::div(rel(1, 3), m.vg.zsize);
    m.vg.z_a = sse::div(rel(2, 3), m.vg.zsize);
    m.vg.z_s = sse::div(rel(3, 3), m.vg.zsize);
    let mut pcrs = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a real 4 x 1 matrix");
    pcrs.set_relt(1, 1, (f64::from(m.vg.width) * 0.5) as f32);
    pcrs.set_relt(2, 1, (f64::from(m.vg.height) * 0.5) as f32);
    pcrs.set_relt(3, 1, (f64::from(m.vg.depth) * 0.5) as f32);
    pcrs.set_relt(4, 1, 1.0);
    if let Some(p) = matrix_multiply(g, out, err, Some(&t), Some(&pcrs), None)?.into_new() {
        m.vg.c_r = p.relt(1, 1);
        m.vg.c_a = p.relt(2, 1);
        m.vg.c_s = p.relt(3, 1);
    }
    if !read_volume {
        return Ok(Some(m));
    }

    let w = m.vg.width;
    if w.wrapping_mul(8) < 0 {
        let _ = out.write_all(b"ERROR: analyzeRead(): malloc failure\n");
        return Ok(None);
    }
    let wu = w.max(0) as usize;
    let mut buf = vec![0u8; wu * 8];
    let open_at = |g: &mut Globals, out: &mut dyn Write, name: &str| -> Option<CFile> {
        let mut fp = match CFile::open(dir, name, "r") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                let _ = writeln!(out, "ERROR: analyzeRead(): could not open {name}");
                return None;
            }
        };
        let off = float_to_int(g, hdr.vox_offset(), "analyzeRead's voxel offset");
        let mut errno = 0;
        if fp.fseek(&mut errno, i64::from(off), 0) != 0 {
            g.errno = errno;
        }
        Some(fp)
    };
    let mut fp: Option<CFile> = None;
    if nfiles == 1 {
        match open_at(g, out, &imgfile) {
            Some(f) => fp = Some(f),
            None => return Ok(None),
        }
    }
    let (h, depth) = (m.vg.height, m.vg.depth);
    let row_bytes = wu * m.bytes_per_vox;
    let doubles = bpv == 8 && hdr.datatype() == DT_DOUBLE && mritype == MRI_FLOAT;
    if doubles && nframes > 1 {
        g.notices.add(
            &FS_649,
            format!(
                "analyzeRead of {nframes} frames of doubles: FreeSurfer puts every frame's values into the first frame"
            ),
        );
    }
    let frame0 = doubles && g.notices.reproduce();
    let mut high_uint32 = false;
    for frame in 0..nframes {
        if nzpad > -1 {
            imgfile = frame_name(frame.wrapping_add(startframe), b"img");
            match open_at(g, out, &imgfile) {
                // the first file's stream is left open (FreeSurfer opens it again)
                Some(f) => fp = Some(f),
                None => return Ok(None),
            }
        }
        let f = fp.as_mut().expect("an open .img file");
        for slice in 0..depth {
            let k = slice.wrapping_add(depth.wrapping_mul(frame));
            for row in 0..h {
                let nread = f.fread(&mut buf[..wu * bpv], bpv, wu);
                if let Some(e) = f.read_errno() {
                    g.errno = e;
                }
                let thiserrno = g.errno;
                if nread != wu {
                    if f.feof() {
                        let _ = out.write_all(b"ERROR: premature end of file\n");
                    }
                    let _ = writeln!(out, "ERROR: {} ({thiserrno})", log::strerror(thiserrno));
                    g.errno = 0;
                    let _ = writeln!(out, "frame = {frame}, slice = {slice}, k={k}, row = {row}");
                    let _ = writeln!(out, "nread = {nread}, nexpected = {w}");
                    let mut text = Vec::new();
                    mri_dump(g, out, err, &m, &mut text)?;
                    let _ = out.write_all(&text);
                    log::error_printf(
                        g,
                        err,
                        ERROR_BADFILE,
                        &format!("analyzeRead2(): error reading from file {imgfile}\n"),
                    );
                    return Ok(None);
                }
                if swap {
                    let n = (bpv * wu) as i64;
                    match bpv {
                        2 => bfileio::byteswapbufshort(g, &mut buf, n),
                        4 => bfileio::byteswapbuffloat(g, &mut buf, n),
                        8 => bfileio::byteswapbufdouble(g, &mut buf, n),
                        _ => 0,
                    };
                }
                let Some(chunk) = m.chunk.as_mut() else {
                    continue;
                };
                if doubles {
                    // MRIFvox(mri, x, row, slice): frame 0 in FreeSurfer (FS-649)
                    let kk = if frame0 { slice } else { k };
                    let o = (kk as usize * h as usize + row as usize) * row_bytes;
                    for (i, v) in buf[..wu * 8].as_chunks::<8>().0.iter().enumerate() {
                        let x = narrow(f64::from_le_bytes(*v));
                        chunk[o + 4 * i..o + 4 * i + 4].copy_from_slice(&x.to_le_bytes());
                    }
                } else {
                    let o = (k as usize * h as usize + row as usize) * row_bytes;
                    let n = bpv * wu;
                    chunk[o..o + n].copy_from_slice(&buf[..n]);
                    if hdr.datatype() == DT_UINT32 {
                        high_uint32 |= buf[..n].as_chunks::<4>().0.iter().any(|c| c[3] & 0x80 != 0);
                    }
                }
            }
        }
        if nzpad > -1
            && let Some(f) = fp.take()
        {
            f.close();
        }
    }
    if nzpad < 0
        && let Some(f) = fp.take()
    {
        f.close();
    }
    if high_uint32 {
        g.notices.add(
            &FS_457,
            "an ANALYZE volume of unsigned 32-bit values above 2^31 - 1: read as negative ints (FreeSurfer's int volume)",
        );
    }
    let roi = hdr.roi_scale();
    let _ = writeln!(
        out,
        "  analyzeRead() roi_scale {}",
        cfmt::pad(&cfmt::f(widen(roi), 9), 13)
    );
    let norescale = env.get("FS_ANALYZE_NO_RESCALE").is_some();
    if norescale {
        let _ = out.write_all(b"FS_ANALYZE_NO_RESCALE set, so not rescaling\n");
    }
    let fabs = |v: f32| f32::from_bits(v.to_bits() & 0x7fff_ffff);
    if fabs(sse::sub(roi, 1.0)) > f32::EPSILON && fabs(roi) > f32::EPSILON && !norescale {
        if m.mtype != MRI_FLOAT {
            let _ = out.write_all(b"  analyzeRead() changing type to float\n");
            m = seq_to_float(g, out, err, &mut m)?;
        }
        let _ = writeln!(
            out,
            "  analyzeRead() scaling by {}",
            cfmt::pad(&cfmt::f(widen(roi), 9), 13)
        );
        multiply_const(g, err, &mut m, roi)?;
    }
    if g.gdiag_no > 0 {
        let (mut min, mut max) = (0f32, 0f32);
        mri_limits(g, err, &m, &mut min, &mut max)?;
        let _ = writeln!(
            out,
            "INFO: analyzeRead(): min = {}, max = {}",
            cfmt::g(widen(min), 6),
            cfmt::g(widen(max), 6)
        );
    }
    Ok(Some(m))
}

// ---------------------------------------------------------------- the writers

/// The ANALYZE data type and bytes per voxel of a volume type (long volumes as 32-bit ints).
fn analyze_type(mtype: i32) -> Option<(i16, usize)> {
    Some(match mtype {
        MRI_UCHAR => (DT_UNSIGNED_CHAR, 1),
        MRI_SHORT => (DT_SIGNED_SHORT, 2),
        MRI_USHRT => (DT_UINT16, 2),
        MRI_INT | MRI_LONG => (DT_SIGNED_INT, 4),
        MRI_FLOAT => (DT_FLOAT, 4),
        _ => return None,
    })
}

/// The writers' extent check: an extent above 32,768 prints `ANALYZE FORMAT ERROR: <what> <n> in
/// volume exceeds 32768` on standard output and ends the program (status 1). FS-653: FreeSurfer
/// lets 32,768 itself through (written as -32,768); by default it is refused too (`exceeds 32767`).
fn check_extents(g: &mut Globals, out: &mut dyn Write, m: &Mri) -> Result<(), Fatal> {
    let checks = [
        ("ncols", m.vg.width),
        ("nrows", m.vg.height),
        ("nslices", m.vg.depth),
        (" nframes", m.nframes),
    ];
    for (what, v) in checks {
        let limit = if v == 32768 {
            g.notices.add(
                &FS_653,
                format!(
                    "an ANALYZE extent of 32768 ({}): FreeSurfer writes -32768",
                    what.trim()
                ),
            );
            if g.notices.fix() { 32767 } else { 32768 }
        } else {
            32768
        };
        if v > limit {
            let _ = writeln!(
                out,
                "ANALYZE FORMAT ERROR: {what} {v} in volume exceeds {limit}"
            );
            return Err(Fatal::Exit(1));
        }
    }
    Ok(())
}

/// The header both writers fill: `sizeof_hdr` 348, the type, `bitpix`, `vox_units` `mm`, the
/// extents (`dim[0]` 4), the voxel sizes and `pixdim[4] = TR / 1000` (in `float`), `glmin` and
/// `glmax` from `MRIlimits` (FS-656; a volume without voxels: FS-58), the origin from the inverse
/// of the 1-based voxel-to-RAS matrix (FS-656). Also writes the `.mat` file (after `pre_mat` on
/// standard output) and prints the matrix (`print`). `Err(code)`: the `.mat` writer's error.
#[allow(clippy::too_many_arguments)]
fn fill_header(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    m: &Mri,
    dt: i16,
    bpv: usize,
    nframes_dim: i32,
    mat_fname: &str,
    print: bool,
    pre_mat: &[u8],
) -> Result<Result<Dsr, i32>, Fatal> {
    let mut h = Dsr([0u8; 348]);
    h.set_i32(0, 348);
    h.set_i16(70, dt);
    h.set_i16(72, (8 * bpv) as i16);
    h.0[56..59].copy_from_slice(b"mm\0");
    h.set_i16(40, 4);
    h.set_i16(42, m.vg.width as i16);
    h.set_i16(44, m.vg.height as i16);
    h.set_i16(46, m.vg.depth as i16);
    h.set_i16(48, nframes_dim as i16);
    h.set_f32(80, m.vg.xsize);
    h.set_f32(84, m.vg.ysize);
    h.set_f32(88, m.vg.zsize);
    h.set_f32(92, sse::div(m.tr, 1000.0));
    let (mut min, mut max) = (0f32, 0f32);
    if m.chunk.is_none() {
        g.notices.add(
            &FS_58,
            "an ANALYZE writer of a volume made without voxels: no limits (FreeSurfer: memory contents, then a crash)",
        );
    } else {
        mri_limits(g, err, m, &mut min, &mut max)?;
    }
    h.set_i32(
        144,
        float_to_int(g, min, "the ANALYZE writer's smallest value"),
    );
    h.set_i32(
        140,
        float_to_int(g, max, "the ANALYZE writer's largest value"),
    );
    let mut t = vol_geom::xfm_crs2xyz(g, out, err, &m.vg, 1, false)?;
    let det = matrix_determinant(g, err, &t);
    if det == 0.0 {
        let _ = out.write_all(
            b"WARNING: cannot determine volume orientation, assuming identity. It's ok if the output is a surface.\n",
        );
        matrix_identity(g, out, err, 4, Some(&mut t))?;
        if m.vg.xsize > 0.0 {
            t.set_relt(1, 1, m.vg.xsize);
        }
        if m.vg.ysize > 0.0 {
            t.set_relt(2, 2, m.vg.ysize);
        }
        if m.vg.zsize > 0.0 {
            t.set_relt(3, 3, m.vg.zsize);
        }
    }
    if print {
        let _ = out.write_all(b"Analyze Output Matrix\n");
        let mut text = Vec::new();
        matrix_print(g, Some(&mut text), out, err, Some(&t), 5);
        let _ = out.write_all(&text);
        let _ = out.write_all(b"--------------------\n");
    }
    let _ = out.write_all(pre_mat);
    let e = matfile::matlab_write(g, dir, &t, mat_fname, Some(b"M"));
    if e != NO_ERROR {
        return Ok(Err(e));
    }
    let inv = match matrix_inverse(g, out, err, Some(&t), None)?.into_new() {
        Some(i) => i,
        None => matrix_identity(g, out, err, 4, None)?.unwrap_new(),
    };
    for r in 1..=3 {
        let s = rint_to_short(g, inv.relt(r, 4), "the ANALYZE writer's origin");
        let o = 253 + 2 * (r as usize - 1);
        h.0[o..o + 2].copy_from_slice(&s.to_le_bytes());
    }
    Ok(Ok(h))
}

/// `printDirCos(mri)` on standard error.
fn print_dir_cos(err: &mut dyn Write, m: &Mri) {
    let v = &m.vg;
    let f84 = |x: f32| cfmt::pad(&cfmt::f(widen(x), 4), 8);
    let f104 = |x: f32| cfmt::pad(&cfmt::f(widen(x), 4), 10);
    let mut t = b"Direction cosines for ".to_vec();
    t.extend_from_slice(v.fname.c_bytes());
    t.extend_from_slice(b" are:\n");
    for (a, x, y, z, c) in [
        ("r", v.x_r, v.y_r, v.z_r, v.c_r),
        ("a", v.x_a, v.y_a, v.z_a, v.c_a),
        ("s", v.x_s, v.y_s, v.z_s, v.c_s),
    ] {
        t.extend_from_slice(
            format!(
                "  x_{a} = {}, y_{a} = {}, z_{a} = {}, c_{a} = {}\n",
                f84(x),
                f84(y),
                f84(z),
                f104(c)
            )
            .as_bytes(),
        );
    }
    let _ = err.write_all(&t);
}

/// The `.hdr` and `.mat` names of a `.img` name (`strcpy` into 1,024-byte buffers: FS-646), or
/// `<who>(): bad file name <fname>` (`ERROR_BADPARM`) for a name without the `.img` extension.
fn pair_names(
    g: &mut Globals,
    err: &mut dyn Write,
    who: &str,
    fname: &str,
) -> Result<Result<(String, String), i32>, Fatal> {
    let ok = fname.rfind('.').is_some_and(|c| &fname[c..] == ".img");
    if !ok {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("{who}(): bad file name {fname}"),
        );
        return Ok(Err(ERROR_BADPARM));
    }
    fits(g, err, who, fname.len(), STRLEN)?;
    let stem = &fname[..fname.len() - 4];
    Ok(Ok((format!("{stem}.hdr"), format!("{stem}.mat"))))
}

/// FS-655: a close that fails. FreeSurfer ignores it; by default `<who>(): error writing to file
/// <name>` (`ERROR_BADFILE`).
fn closed(g: &mut Globals, err: &mut dyn Write, who: &str, name: &str, fp: CFile) -> i32 {
    if fp.close() == 0 {
        return NO_ERROR;
    }
    g.notices.add(
        &FS_655,
        format!("{who}: the close of {name} failed (FreeSurfer reports success)"),
    );
    if g.notices.reproduce() {
        return NO_ERROR;
    }
    g.errno = 0;
    log::error_printf(
        g,
        err,
        ERROR_BADFILE,
        &format!("{who}(): error writing to file {name}"),
    );
    ERROR_BADFILE
}

/// Writes the header to `hdr_fname` (`<who>(): error opening file <hdr> for writing`, `...
/// error writing to file <hdr>`, `ERROR_BADFILE`), then dumps it with `Gdiag_no` above 0.
fn write_header(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    who: &str,
    hdr_fname: &str,
    h: &Dsr,
) -> i32 {
    let Ok(mut fp) = CFile::open(dir, hdr_fname, "w") else {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            &format!("{who}(): error opening file {hdr_fname} for writing"),
        );
        return ERROR_BADFILE;
    };
    if fp.fwrite(&h.0, 348, 1) != 1 {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            &format!("{who}(): error writing to file {hdr_fname}"),
        );
        return ERROR_BADFILE;
    }
    let e = closed(g, err, who, hdr_fname, fp);
    if e != NO_ERROR {
        return e;
    }
    if g.gdiag_no > 0 {
        dump_analyze_header(out, h);
    }
    NO_ERROR
}

/// Writes the rows of `frames` to `fname`: each row `width x bytes` from the volume (FS-654: a long
/// volume's rows by default as one 32-bit int per value, FreeSurfer's the first half of each row's
/// bytes). A volume without voxels: FS-58.
#[allow(clippy::too_many_arguments)]
fn write_data(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    who: &str,
    sep: &str,
    fname: &str,
    m: &Mri,
    bpv: usize,
    frames: std::ops::Range<i32>,
) -> Result<i32, Fatal> {
    let Ok(mut fp) = CFile::open(dir, fname, "w") else {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            &format!("{who}(): error opening file {fname} for writing"),
        );
        return Ok(ERROR_BADFILE);
    };
    let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
    let wu = w.max(0) as usize;
    let long = m.mtype == MRI_LONG;
    if long && !frames.is_empty() && d > 0 && h > 0 && w > 0 {
        g.notices.add(
            &FS_654,
            "an ANALYZE write of a long volume: FreeSurfer writes the first half of each row's bytes as ints",
        );
    }
    let row_bytes = wu * m.bytes_per_vox;
    for frame in frames {
        for i in 0..d {
            let k = i.wrapping_add(d.wrapping_mul(frame));
            for j in 0..h {
                let Some(chunk) = m.chunk.as_ref() else {
                    log::error_exit(
                        g,
                        err,
                        ERROR_BADPARM,
                        &format!("{who}(): the volume has no voxels (FreeSurfer crashes here)\n"),
                    )?;
                    return Ok(ERROR_BADPARM);
                };
                let o = (k as usize * h as usize + j as usize) * row_bytes;
                let row: Vec<u8> = if long && g.notices.fix() {
                    chunk[o..o + row_bytes]
                        .as_chunks::<8>()
                        .0
                        .iter()
                        .flat_map(|v| (i64::from_le_bytes(*v) as i32).to_le_bytes())
                        .collect()
                } else {
                    chunk[o..o + bpv * wu].to_vec()
                };
                if fp.fwrite(&row, bpv, wu) != wu {
                    g.errno = 0;
                    log::error_printf(
                        g,
                        err,
                        ERROR_BADFILE,
                        &format!("{who}():{sep}error writing to file {fname}"),
                    );
                    return Ok(ERROR_BADFILE);
                }
            }
        }
    }
    Ok(closed(g, err, who, fname, fp))
}

/// `analyzeWriteFrame(mri, fname, frame)`: one frame as an ANALYZE pair `<stem>.hdr`/`fname`
/// (`.img`) and the SPM matrix `<stem>.mat`. See `specs/freesurfer-io.md` §102.
pub fn analyze_write_frame(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    m: &Mri,
    fname: &str,
    frame: i32,
) -> Result<i32, Fatal> {
    const WHO: &str = "analyzeWriteFrame";
    if frame >= m.nframes {
        let _ = writeln!(
            err,
            "ERROR: analyzeWriteFrame(): frame number ({frame}) exceeds number of frames ({})",
            m.nframes
        );
        return Ok(1);
    }
    check_extents(g, out, m)?;
    let (hdr_fname, mat_fname) = match pair_names(g, err, WHO, fname)? {
        Ok(n) => n,
        Err(e) => return Ok(e),
    };
    let Some((dt, bpv)) = analyze_type(m.mtype) else {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("analyzeWriteFrame(): bad data type {}", m.mtype),
        );
        return Ok(ERROR_BADPARM);
    };
    let mut h = match fill_header(g, out, err, dir, m, dt, bpv, 1, &mat_fname, frame == 0, b"")? {
        Ok(h) => h,
        Err(e) => return Ok(e),
    };
    let v = &m.vg;
    let a = |x: f32| x.abs();
    const NAMES: [&str; 7] = [
        "transverse unflipped",
        "coronal unflipped",
        "sagittal unflipped",
        "transverse flipped",
        "coronal flipped",
        "sagittal flipped",
        "unknown",
    ];
    let mut orient = 0usize;
    if a(v.z_s) > a(v.z_r) && a(v.z_s) > a(v.z_a) {
        if v.x_r < 0.0 && v.y_a > 0.0 && v.z_s > 0.0 {
            orient = 0;
        } else if v.x_r < 0.0 && v.y_a < 0.0 && v.z_s > 0.0 {
            orient = 3;
        } else {
            let _ = err.write_all(b"No such orientation specified in Analyze7.5. Set orient to 0.\nNot a problem as long as you use the .mat file.\n");
            print_dir_cos(err, m);
            orient = 0;
        }
    }
    if a(v.z_a) > a(v.z_r) && a(v.z_a) > a(v.z_s) {
        if v.x_r < 0.0 && v.y_s > 0.0 && v.z_a > 0.0 {
            orient = 1;
        } else if v.x_r < 0.0 && v.y_s < 0.0 && v.z_a > 0.0 {
            orient = 4;
        } else {
            let _ =
                err.write_all(b"No such orientation specified in Analyze7.5. Set orient to 0\n");
            print_dir_cos(err, m);
            orient = 0;
        }
    }
    if a(v.z_r) > a(v.z_a) && a(v.z_r) > a(v.z_s) {
        if v.x_a > 0.0 && v.y_s > 0.0 && v.z_r < 0.0 {
            orient = 2;
        } else if v.x_a > 0.0 && v.y_s < 0.0 && v.z_r < 0.0 {
            orient = 5;
        } else {
            let _ =
                err.write_all(b"No such orientation specified in Analyze7.5. Set orient to 0\n");
            print_dir_cos(err, m);
            orient = 0;
        }
    }
    h.0[252] = orient as u8;
    let _ = writeln!(out, "INFO: set hdr.hist.orient to '{}'", NAMES[orient]);
    let e = write_header(g, out, err, dir, WHO, &hdr_fname, &h);
    if e != NO_ERROR {
        return Ok(e);
    }
    write_data(g, err, dir, WHO, "  ", fname, m, bpv, frame..frame + 1)
}

/// `analyzeWriteSeries(mri, fname)`: each frame `f` as [`analyze_write_frame`] to
/// `<fname><f + 1>.img`, the number zero-padded to `N_Zero_Pad_Output` digits (set to 3 when
/// below 0); the first error ends it.
pub fn analyze_write_series(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    m: &Mri,
    fname: &str,
) -> Result<i32, Fatal> {
    if g.n_zero_pad_output < 0 {
        g.n_zero_pad_output = 3;
    }
    for frame in 0..m.nframes {
        let num = zero_padded(frame + 1, g.n_zero_pad_output);
        fits(
            g,
            err,
            "analyzeWriteSeries",
            fname.len() + num.len() + 4,
            STRLEN,
        )?;
        let framename = format!("{fname}{num}.img");
        let e = analyze_write_frame(g, out, err, dir, m, &framename, frame)?;
        if e != 0 {
            return Ok(e);
        }
    }
    Ok(0)
}

/// `analyzeWrite(mri, fname)`: a name ending in `.img`: its first frame ([`analyze_write_frame`]);
/// otherwise the series ([`analyze_write_series`]).
pub fn analyze_write(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    m: &Mri,
    fname: &str,
) -> Result<i32, Fatal> {
    if fname.len() > 4 && fname.ends_with(".img") {
        return analyze_write_frame(g, out, err, dir, m, fname, 0);
    }
    analyze_write_series(g, out, err, dir, m, fname)
}

/// `analyzeWrite4D(mri, fname)`: every frame into one ANALYZE pair `<stem>.hdr`/`fname` (`.img`)
/// and the SPM matrix `<stem>.mat`; the orientation code always -1 (`INFO: set hdr.hist.orient
/// to -1`). See `specs/freesurfer-io.md` §102.
pub fn analyze_write4d(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    m: &Mri,
    fname: &str,
) -> Result<i32, Fatal> {
    const WHO: &str = "analyzeWrite4D";
    check_extents(g, out, m)?;
    let (hdr_fname, mat_fname) = match pair_names(g, err, WHO, fname)? {
        Ok(n) => n,
        Err(e) => return Ok(e),
    };
    let Some((dt, bpv)) = analyze_type(m.mtype) else {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("analyzeWrite4D(): bad data type {}", m.mtype),
        );
        return Ok(ERROR_BADPARM);
    };
    // the orientation code is worked out and then always set to -1, printed before the .mat file
    let info = b"INFO: set hdr.hist.orient to -1\n";
    let mut h = match fill_header(
        g, out, err, dir, m, dt, bpv, m.nframes, &mat_fname, true, info,
    )? {
        Ok(h) => h,
        Err(e) => return Ok(e),
    };
    h.0[252] = 0xff;
    let e = write_header(g, out, err, dir, WHO, &hdr_fname, &h);
    if e != NO_ERROR {
        return Ok(e);
    }
    write_data(g, err, dir, WHO, " ", fname, m, bpv, 0..m.nframes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    /// A little-endian ANALYZE header of extents `dims` (`dim[1..=4]`), type `dt`, orientation
    /// `orient` and voxel sizes 1.
    fn header(dims: [i16; 4], dt: i16, orient: i8) -> Dsr {
        let mut h = Dsr([0u8; 348]);
        h.set_i32(0, 348);
        h.set_i16(40, 4);
        for (i, d) in dims.iter().enumerate() {
            h.set_i16(42 + 2 * i, *d);
        }
        h.set_i16(70, dt);
        for i in 1..4 {
            h.set_f32(76 + 4 * i, 1.0);
        }
        h.set_f32(112, 1.0);
        h.0[252] = orient as u8;
        h
    }

    fn setup(mode: UpstreamDefects) -> (Globals, Vec<u8>, Vec<u8>, Environment) {
        (
            Globals::new(mode),
            Vec::new(),
            Vec::new(),
            Environment::default(),
        )
    }

    fn tmp() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("analyze ~")
            .tempdir()
            .unwrap()
    }

    #[test]
    fn fs645_a_start_frame_that_is_not_a_number_counts_from_1() {
        let (mut g, mut o, _, _) = setup(UpstreamDefects::Reproduce);
        let env = |v: &str| Environment::from_pairs([("SPM_START_FRAME", v)]);
        assert_eq!(
            get_spm_start_frame(&mut g, &mut o, &Environment::default()),
            1
        );
        assert!(o.is_empty());
        assert_eq!(get_spm_start_frame(&mut g, &mut o, &env(" 7x")), 7);
        assert_eq!(
            get_spm_start_frame(&mut g, &mut o, &env("99999999999")),
            1215752191
        );
        assert!(!g.notices.contains(&FS_645));
        assert_eq!(get_spm_start_frame(&mut g, &mut o, &env("abc")), 1);
        assert!(g.notices.contains(&FS_645));
        assert_eq!(
            String::from_utf8(o).unwrap(),
            "Using env var SPM_START_FRAME = 7\nUsing env var SPM_START_FRAME = 1215752191\nUsing env var SPM_START_FRAME = 1\n"
        );
    }

    #[test]
    fn fs646_a_long_name_aborts_in_freesurfer() {
        // no file is touched before the name's length is checked, so the test needs no long path
        let name = [b"a/".as_slice(), &[b'x'; 996]].concat();
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (mut g, mut o, mut e, env) = setup(mode);
            let d = tmp();
            let r = count_analyze_files(&mut g, &mut o, &mut e, &env, Some(d.path()), &name, -1);
            assert!(g.notices.contains(&FS_646));
            if mode == UpstreamDefects::Fix {
                assert_eq!(r.unwrap().0, 0);
            } else {
                assert_eq!(r.unwrap_err(), Fatal::Exit(134));
                assert_eq!(e, OVERFLOW);
            }
        }
        // 995 bytes of stem: `<stem>.img` fits the 1,000-byte buffer
        let (mut g, mut o, mut e, env) = setup(UpstreamDefects::Reproduce);
        let d = tmp();
        let name = [b"a/".as_slice(), &[b'x'; 993]].concat();
        let r = count_analyze_files(&mut g, &mut o, &mut e, &env, Some(d.path()), &name, -1);
        assert_eq!(r.unwrap().0, 0);
        assert!(!g.notices.contains(&FS_646));
    }

    #[test]
    fn a_series_is_counted_from_the_start_frame() {
        let (mut g, mut o, mut e, _) = setup(UpstreamDefects::Fix);
        let d = tmp();
        for n in ["s002.img", "s003.img", "s005.img"] {
            std::fs::write(d.path().join(n), b"").unwrap();
        }
        let env = Environment::from_pairs([("SPM_START_FRAME", "2")]);
        let r = count_analyze_files(&mut g, &mut o, &mut e, &env, Some(d.path()), b"s", 3);
        assert_eq!(r.unwrap(), (2, b"s".to_vec()));
        assert_eq!(g.errno, 2);
        // a .img name with zero padding: an error
        let r = count_analyze_files(&mut g, &mut o, &mut e, &env, Some(d.path()), b"s.img", 3);
        assert_eq!(r.unwrap().0, -1);
        assert!(String::from_utf8(o).unwrap().ends_with(
            "ERROR: CountAnalyzeFiles: file with .img extension specified        with zero pad variable.\n"
        ));
    }

    #[test]
    fn fs649_double_frames_each_in_their_place_by_default() {
        let d = tmp();
        std::fs::write(d.path().join("a.hdr"), header([2, 1, 1, 2], DT_DOUBLE, 0).0).unwrap();
        let vals = [1.5f64, -2.0, 3.25, 4.0];
        let img: Vec<u8> = vals.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(d.path().join("a.img"), img).unwrap();
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (mut g, mut o, mut e, env) = setup(mode);
            let m = analyze_read(&mut g, &mut o, &mut e, &env, Some(d.path()), "a.img", true)
                .unwrap()
                .unwrap();
            let f: Vec<f32> = m
                .chunk
                .as_ref()
                .unwrap()
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect();
            if mode == UpstreamDefects::Fix {
                assert_eq!(f, [1.5, -2.0, 3.25, 4.0]);
            } else {
                // FreeSurfer: the second frame's values written over the first's
                assert_eq!(f, [3.25, 4.0, 0.0, 0.0]);
            }
            assert!(g.notices.contains(&FS_649));
            assert_eq!(m.mtype, MRI_FLOAT);
        }
    }

    #[test]
    fn fs648_an_unknown_orientation_code_is_assumed_transverse_flipped() {
        let d = tmp();
        std::fs::write(
            d.path().join("a.hdr"),
            header([2, 2, 1, 1], DT_UNSIGNED_CHAR, 6).0,
        )
        .unwrap();
        std::fs::write(d.path().join("a.img"), [1u8, 2, 3, 4]).unwrap();
        let (mut g, mut o, mut e, env) = setup(UpstreamDefects::Fix);
        let m = analyze_read(&mut g, &mut o, &mut e, &env, Some(d.path()), "a.img", false)
            .unwrap()
            .unwrap();
        assert!(g.notices.contains(&FS_648));
        assert_eq!(
            (m.vg.ras_good_flag, m.vg.x_r, m.vg.y_a, m.vg.z_s),
            (0, -1.0, -1.0, 1.0)
        );
        assert!(
            String::from_utf8(e)
                .unwrap()
                .contains("orient value = 6, not valid.")
        );
        let (mut g, mut o, mut e, env) = setup(UpstreamDefects::Reproduce);
        let r = analyze_read(&mut g, &mut o, &mut e, &env, Some(d.path()), "a.img", false);
        assert_eq!(r.unwrap_err(), Fatal::Exit(139));
    }

    #[test]
    fn fs650_a_mat_file_of_another_shape_is_an_error() {
        let d = tmp();
        std::fs::write(
            d.path().join("a.hdr"),
            header([2, 2, 1, 1], DT_UNSIGNED_CHAR, 0).0,
        )
        .unwrap();
        std::fs::write(d.path().join("a.img"), [1u8, 2, 3, 4]).unwrap();
        let mut mat = Matrix::real(4, 3);
        mat.set_relt(1, 1, 2.0);
        let (mut g, ..) = setup(UpstreamDefects::Fix);
        assert_eq!(
            matfile::matlab_write(&mut g, Some(d.path()), &mat, "a.mat", Some(b"M")),
            NO_ERROR
        );
        let (mut g, mut o, mut e, env) = setup(UpstreamDefects::Fix);
        let r = analyze_read(&mut g, &mut o, &mut e, &env, Some(d.path()), "a.img", true);
        assert!(r.unwrap().is_none());
        assert!(g.notices.contains(&FS_650));
        assert!(
            String::from_utf8(e)
                .unwrap()
                .contains("holds a 4 x 3 matrix")
        );
        let (mut g, mut o, mut e, env) = setup(UpstreamDefects::Reproduce);
        let r = analyze_read(&mut g, &mut o, &mut e, &env, Some(d.path()), "a.img", true);
        assert_eq!(r.unwrap_err(), Fatal::Exit(139));
    }

    #[test]
    fn fs652_a_big_endian_header_dump() {
        let mut h = header([2, 2, 1, 1], DT_SIGNED_SHORT, 0);
        h.set_i32(32, 16384);
        h.set_i16(50, 7);
        // the header as a big-endian machine writes it
        let mut b = h.clone();
        for (o, n) in [(0, 4), (32, 4), (70, 2), (112, 4)] {
            b.0[o..o + n].reverse();
        }
        for i in 0..8 {
            b.0[40 + 2 * i..42 + 2 * i].reverse();
            b.0[76 + 4 * i..80 + 4 * i].reverse();
        }
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (mut g, ..) = setup(mode);
            g.gdiag_no = 1;
            let mut x = b.clone();
            swap_analyze_header(&mut g, &mut x);
            assert!(g.notices.contains(&FS_652));
            let mut text = Vec::new();
            dump_analyze_header(&mut text, &x);
            let text = String::from_utf8(text).unwrap();
            if mode == UpstreamDefects::Fix {
                assert!(text.contains("  extents       16384\n"));
                assert!(text.contains("  dim 4 2 2 1 1 7 0 0 \n"));
            } else {
                // 16,384 is 0x4000: its low 16 bits are 0
                assert!(text.contains("  extents       0\n"));
                assert!(text.contains("  dim 4 2 2 1 1 1792 0 0 \n"));
            }
        }
    }

    #[test]
    fn fs653_an_extent_of_32768_is_refused_by_default() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (mut g, mut o, mut e, _) = setup(mode);
            let m =
                mri::mri_alloc_header(&mut g, &mut o, &mut e, 32768, 1, 1, MRI_UCHAR, 1).unwrap();
            let r = check_extents(&mut g, &mut o, &m);
            assert!(g.notices.contains(&FS_653));
            if mode == UpstreamDefects::Fix {
                assert_eq!(r.unwrap_err(), Fatal::Exit(1));
                assert!(
                    String::from_utf8(o)
                        .unwrap()
                        .ends_with("ANALYZE FORMAT ERROR: ncols 32768 in volume exceeds 32767\n")
                );
            } else {
                assert!(r.is_ok());
            }
        }
    }

    #[test]
    fn fs654_a_long_volume_is_written_as_ints_by_default() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (mut g, mut o, mut e, _) = setup(mode);
            let d = tmp();
            let mut m = mri::mri_alloc(&mut g, &mut o, &mut e, 2, 1, 1, MRI_LONG).unwrap();
            let c = m.chunk.as_mut().unwrap();
            c[..8].copy_from_slice(&5i64.to_le_bytes());
            c[8..].copy_from_slice(&(-3i64).to_le_bytes());
            let r = analyze_write4d(&mut g, &mut o, &mut e, Some(d.path()), &m, "o.img");
            assert_eq!(r.unwrap(), 0);
            assert!(g.notices.contains(&FS_654));
            let img = std::fs::read(d.path().join("o.img")).unwrap();
            if mode == UpstreamDefects::Fix {
                assert_eq!(img, [5, 0, 0, 0, 0xfd, 0xff, 0xff, 0xff]);
            } else {
                assert_eq!(img, [5, 0, 0, 0, 0, 0, 0, 0]);
            }
            let hdr = std::fs::read(d.path().join("o.hdr")).unwrap();
            assert_eq!((hdr.len(), hdr[252]), (348, 0xff));
            assert!(d.path().join("o.mat").is_file());
        }
    }

    #[test]
    fn fs656_conversions_outside_the_range() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (mut g, ..) = setup(mode);
            assert_eq!(float_to_int(&mut g, 7.9, "x"), 7);
            assert_eq!(float_to_int(&mut g, -4.5, "x"), -4);
            assert_eq!(rint_to_short(&mut g, 2.5, "x"), 2);
            assert_eq!(rint_to_short(&mut g, -3.5, "x"), -4);
            assert!(!g.notices.contains(&FS_656));
            let fix = mode == UpstreamDefects::Fix;
            assert_eq!(
                float_to_int(&mut g, f32::NAN, "x"),
                if fix { 0 } else { i32::MIN }
            );
            assert_eq!(
                float_to_int(&mut g, 3.0e9, "x"),
                if fix { i32::MAX } else { i32::MIN }
            );
            // 40,000 wraps in 16 bits: -25,536
            assert_eq!(
                rint_to_short(&mut g, 40000.0, "x"),
                if fix { i16::MAX } else { -25536 }
            );
            assert_eq!(
                rint_to_short(&mut g, 1.0e12, "x"),
                if fix { i16::MAX } else { 0 }
            );
            assert!(g.notices.contains(&FS_656));
        }
    }

    #[test]
    fn a_pair_round_trip_with_the_scale_factor() {
        let d = tmp();
        let mut h = header([2, 1, 1, 0], DT_SIGNED_SHORT, 0);
        h.set_f32(112, 0.5);
        std::fs::write(d.path().join("a.hdr"), h.0).unwrap();
        let img: Vec<u8> = [6i16, -3].iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(d.path().join("a.img"), img).unwrap();
        let (mut g, mut o, mut e, env) = setup(UpstreamDefects::Fix);
        let m = analyze_read(&mut g, &mut o, &mut e, &env, Some(d.path()), "a.img", true)
            .unwrap()
            .unwrap();
        assert_eq!(m.mtype, MRI_FLOAT);
        let f: Vec<f32> = m
            .chunk
            .as_ref()
            .unwrap()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        assert_eq!(f, [3.0, -1.5]);
        let text = String::from_utf8(o).unwrap();
        assert!(text.ends_with(
            "  analyzeRead() roi_scale   0.500000000\n  analyzeRead() changing type to float\n  analyzeRead() scaling by   0.500000000\n"
        ));
        assert_eq!(m.frames.len(), 1);
    }
}
