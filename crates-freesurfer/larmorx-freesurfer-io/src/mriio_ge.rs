// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! GE Genesis and GE Signa LX volumes (`utils/mriio.cpp` 4014-4745, item 3.1-27): `genesisRead`
//! ([`genesis_read`]: a series `I.001`, `I.002` ... or `<stem>1.MR`, `<stem>2.MR` ..., one slice
//! of big-endian shorts per file) and `gelxRead` ([`gelx_read`]: a series `<e>s<n>i<n>`). Wired
//! into [`crate::mriio::mri_read`] (`GENESIS_FILE`, `GE_LX_FILE`).
//!
//! FreeSurfer 8.2.0 reads neither (FS-680): both allocate their header through a null pointer
//! once the series' names are found, and crash (signal 11). `reproduce` prints what FreeSurfer
//! prints before it and ends with status 139; the default reads the series as the source means
//! (the header allocated with one frame). [`genesis_read_as_patched`] and
//! [`gelx_read_as_patched`] are the readers with FS-680 bypassed in both modes, the other defects
//! as the mode says: the fs-io probe compares them with FreeSurfer's two functions compiled from
//! the tag with that one line corrected (`validation/probes/fs-io/build.sh`, piece 19), whose
//! object code is the official one's but for that line.
//!
//! Defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-io.md`, Part 21):
//! FS-680 (the crash), FS-681 (an `I.` name's second format line, memory never written), FS-682
//! (a `%` in the names: C conversions), FS-683 (several echoes: one slice per file), FS-684
//! (gelxRead's messages name genesisRead), FS-685 (several echoes: the header freed past its frame
//! table), FS-167 (a header cut short: memory contents), FS-249
//! (names past FreeSurfer's buffers).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_167, FS_249, FS_680, FS_681, FS_682, FS_683, FS_684, FS_685};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::{cstd, sse, utils};
use larmorx_freesurfer_mri::mri::{self, MRI_SHORT, Mri};

use crate::cfile::CFile;
use crate::fio;
use crate::mriio_nifti::{narrow, widen};

/// `STRLEN`: FreeSurfer's name buffers.
const STRLEN: usize = 1024;

/// `2.0 * M_PI`, as `RADIANS` folds it.
const TWO_PI: f64 = 2.0 * std::f64::consts::PI;

/// `ErrorReturn(NULL, (code, message))` with `errno` cleared first.
fn fail0(g: &mut Globals, err: &mut dyn Write, code: i32, message: &str) -> Option<Mri> {
    g.errno = 0;
    log::error_printf(g, err, code, message);
    None
}

/// A series' file names: `<dir><pre><number><suf>`, the number as `%d` or `%03d`. FreeSurfer
/// builds them with `sprintf` and the folder and stem inside the format (FS-682: a `%` there is a
/// conversion); the port's are the text as it is.
#[derive(Clone)]
struct Series {
    dir: String,
    pre: String,
    pad: bool,
    suf: String,
}

impl Series {
    /// The format text FreeSurfer prints (`fname_format  : ...`).
    fn format(&self) -> String {
        format!(
            "{}{}{}{}",
            self.dir,
            self.pre,
            if self.pad { "%03d" } else { "%d" },
            self.suf
        )
    }

    fn name(&self, n: i32) -> String {
        if self.pad {
            format!("{}{}{n:03}{}", self.dir, self.pre, self.suf)
        } else {
            format!("{}{}{n}{}", self.dir, self.pre, self.suf)
        }
    }

    /// FS-682: a `%` in the folder or the stem.
    fn note_percent(&self, g: &mut Globals, who: &str) {
        if self.dir.contains('%') || self.pre.contains('%') {
            g.notices.add(
                &FS_682,
                format!(
                    "{who}: the series' name {} holds a % (FreeSurfer takes it as a C conversion: wrong names, a crash or an abort): names made from the text",
                    self.format()
                ),
            );
        }
    }
}

/// `snprintf(buf, STRLEN, ...)` of a format text with its `Truncation on line <n>` message when
/// it does not fit: `reproduce` prints the message (FreeSurfer then holds the cut text); both
/// modes keep the whole text (FS-249).
fn fit(g: &mut Globals, err: &mut dyn Write, who: &str, line: u32, text: &str) {
    if text.len() >= STRLEN {
        if g.notices.reproduce() {
            let _ = writeln!(err, "{who}: Truncation on line {line}");
        }
        g.notices.add(
            &FS_249,
            format!(
                "{who}: a format text of {} bytes used whole (FreeSurfer's buffer holds 1023)",
                text.len()
            ),
        );
    }
}

/// The lowest and highest numbers of a series' files around `init` (`step` 1 or 2): FreeSurfer
/// steps down while the files exist, then up.
fn extent(dir: Option<&Path>, s: &Series, init: i32, step: i32) -> (i32, i32) {
    let mut low = init;
    loop {
        low = low.wrapping_sub(step);
        if !utils::file_exists(dir, &s.name(low)) {
            break;
        }
    }
    low = low.wrapping_add(step);
    let mut high = init;
    loop {
        high = high.wrapping_add(step);
        if !utils::file_exists(dir, &s.name(high)) {
            break;
        }
    }
    high = high.wrapping_sub(step);
    (low, high)
}

/// The header's reads: each `fread(&x, 4, 1, fp)` with its message when the file ends, the
/// number's bytes as `orderIntBytes` / `orderFloatBytes` turn them (big-endian).
struct Reader<'a> {
    fp: CFile,
    who: &'a str,
}

impl Reader<'_> {
    fn seek(&mut self, g: &mut Globals, offset: i64) {
        let mut errno = g.errno;
        self.fp.fseek(&mut errno, offset, 0);
        g.errno = errno;
    }

    /// Four bytes into memory that held `init`; the bytes and how many the file gave.
    fn read4(&mut self, g: &mut Globals, err: &mut dyn Write, init: [u8; 4]) -> ([u8; 4], usize) {
        let mut b = init;
        let got = self.fp.read_bytes(&mut b);
        if got < 4 {
            self.sync(g);
            if self.who != "genesisRead" {
                lx_message(g, self.who);
            }
            log::error_printf(g, err, ERROR_BADFILE, "genesisRead(): could not read file");
        }
        (b, got)
    }

    /// A local number (`width`, a corner's coordinate ...): FreeSurfer's missing bytes are memory
    /// never written (FS-167): 0 here.
    fn local(&mut self, g: &mut Globals, err: &mut dyn Write, what: &str) -> [u8; 4] {
        let (mut b, got) = self.read4(g, err, [0; 4]);
        if got < 4 {
            fio::memory(g, &FS_167, &format!("{}: {what}", self.who), got, 4);
            b[got..].fill(0);
        }
        b
    }

    fn local_int(&mut self, g: &mut Globals, err: &mut dyn Write, what: &str) -> i32 {
        i32::from_be_bytes(self.local(g, err, what))
    }

    fn local_float(&mut self, g: &mut Globals, err: &mut dyn Write, what: &str) -> f32 {
        f32::from_be_bytes(self.local(g, err, what))
    }

    /// A header field read in place (`thick`, `xsize`, `ysize`): its missing bytes are the
    /// field's earlier value's in FreeSurfer (`reproduce`), 0 by default (FS-167).
    fn field(&mut self, g: &mut Globals, err: &mut dyn Write, old: f32, what: &str) -> f32 {
        let (mut b, got) = self.read4(g, err, old.to_le_bytes());
        if got < 4 {
            if g.notices.reproduce() {
                g.notices.add(
                    &FS_167,
                    format!(
                        "{}: {what}: the file ended after {got} of the number's 4 bytes: the others are the field's earlier value's, as in FreeSurfer",
                        self.who
                    ),
                );
            } else {
                b[got..].fill(0);
                g.notices.add(
                    &FS_167,
                    format!(
                        "{}: {what}: the file ended after {got} of the number's 4 bytes: the others read as 0 (FreeSurfer's are the field's earlier value's)",
                        self.who
                    ),
                );
            }
        }
        f32::from_be_bytes(b)
    }

    /// `freadInt(fp)` (FS-49: its missing bytes).
    fn fread_int(&mut self, g: &mut Globals) -> i32 {
        let v = fio::fread_int(g, &mut self.fp);
        self.sync(g);
        v
    }

    /// `freadShort(fp)` (`freadShort: fread failed` when the file does not hold it; FS-167).
    fn fread_short(&mut self, g: &mut Globals, err: &mut dyn Write) -> i16 {
        let v = fio::fread_short(g, err, &mut self.fp);
        self.sync(g);
        v
    }

    /// C's `errno` after a read that failed (a folder's `EISDIR`; a file's end sets none).
    fn sync(&self, g: &mut Globals) {
        if let Some(e) = self.fp.read_errno() {
            g.errno = e;
        }
    }
}

/// The corners and normal a GE header gives (`tl`, `tr`, `br` the top-left, top-right and
/// bottom-right corners, `n` the slice normal), into the header's direction cosines, centre,
/// extents and field of view as both readers compute them (`float` arithmetic, the centre in
/// `double`; the official object code's operand order, which decides a NaN's bits).
struct Corners {
    n: [f32; 3],
    tl: [f32; 3],
    tr: [f32; 3],
    br: [f32; 3],
}

/// `sqrt(a*a + b*b + c*c)` in `float`, the sums in the C order.
fn length(a: f32, b: f32, c: f32) -> f32 {
    sse::sqrt(sse::add(
        sse::add(sse::mul(a, a), sse::mul(b, b)),
        sse::mul(c, c),
    ))
}

/// The geometry both readers set from the corners; `lx`: gelxRead's order for the second
/// vector's length (its last square first: `c*c + (a*a + b*b)`).
fn geometry(h: &mut Mri, k: &Corners, lx: bool) {
    let x = [
        sse::sub(k.tr[0], k.tl[0]),
        sse::sub(k.tr[1], k.tl[1]),
        sse::sub(k.tr[2], k.tl[2]),
    ];
    let y = [
        sse::sub(k.br[0], k.tr[0]),
        sse::sub(k.br[1], k.tr[1]),
        sse::sub(k.br[2], k.tr[2]),
    ];
    let xlength = length(x[0], x[1], x[2]);
    let ylength = if lx {
        sse::sqrt(sse::add(
            sse::mul(y[2], y[2]),
            sse::add(sse::mul(y[0], y[0]), sse::mul(y[1], y[1])),
        ))
    } else {
        length(y[0], y[1], y[2])
    };
    let zlength = length(k.n[0], k.n[1], k.n[2]);
    h.vg.x_r = sse::div(x[0], xlength);
    h.vg.x_a = sse::div(x[1], xlength);
    h.vg.x_s = sse::div(x[2], xlength);
    h.vg.y_r = sse::div(y[0], ylength);
    h.vg.y_a = sse::div(y[1], ylength);
    h.vg.y_s = sse::div(y[2], ylength);
    h.vg.z_r = sse::div(k.n[0], zlength);
    h.vg.z_a = sse::div(k.n[1], zlength);
    h.vg.z_s = sse::div(k.n[2], zlength);
    // (tl + br) / 2.0 + n * zsize * (depth - 1.0) / 2.0, compiled as the second term plus the
    // first
    let dm1 = sse::sub64(f64::from(h.vg.depth), 1.0);
    let centre = |tl: f32, br: f32, n: f32| {
        let a = sse::mul64(widen(sse::add(tl, br)), 0.5);
        let b = sse::mul64(sse::mul64(widen(sse::mul(n, h.vg.zsize)), dm1), 0.5);
        narrow(sse::add64(b, a))
    };
    let c = [
        centre(k.tl[0], k.br[0], k.n[0]),
        centre(k.tl[1], k.br[1], k.n[1]),
        centre(k.tl[2], k.br[2], k.n[2]),
    ];
    (h.vg.c_r, h.vg.c_a, h.vg.c_s) = (c[0], c[1], c[2]);
    h.vg.ras_good_flag = 1;
    // size * (double)n / 2.0, compiled as n * size * 0.5
    let half = |n: i32, size: f32| narrow(sse::mul64(sse::mul64(f64::from(n), widen(size)), 0.5));
    h.xend = half(h.vg.width, h.vg.xsize);
    h.xstart = -h.xend;
    h.yend = half(h.vg.height, h.vg.ysize);
    h.ystart = -h.yend;
    h.zend = half(h.vg.depth, h.vg.zsize);
    h.zstart = -h.zend;
    let xfov = sse::sub(h.xend, h.xstart);
    let yfov = sse::sub(h.yend, h.ystart);
    let zfov = sse::sub(h.zend, h.zstart);
    h.fov = if xfov > yfov {
        if xfov > zfov { xfov } else { zfov }
    } else if yfov > zfov {
        yfov
    } else {
        zfov
    };
}

/// The three corners and the normal: fifteen floats from the reader's position, in the file's
/// order centre, normal, top-left, top-right, bottom-right (the centre is read and not used).
fn corners(g: &mut Globals, err: &mut dyn Write, r: &mut Reader) -> Corners {
    let names = [
        "c_r", "c_a", "c_s", "n_r", "n_a", "n_s", "tl_r", "tl_a", "tl_s", "tr_r", "tr_a", "tr_s",
        "br_r", "br_a", "br_s",
    ];
    let mut v = [0f32; 15];
    for (k, name) in names.iter().enumerate() {
        v[k] = r.local_float(g, err, name);
    }
    Corners {
        n: [v[3], v[4], v[5]],
        tl: [v[6], v[7], v[8]],
        tr: [v[9], v[10], v[11]],
        br: [v[12], v[13], v[14]],
    }
}

/// The big-endian shorts of one row into the volume's voxels at `(row, slice, frame)`
/// (`swab` after `fread`): `false` when the file ends first.
fn read_row(fp: &mut CFile, m: &mut Mri, y: i32, slice: i32, frame: i32) -> bool {
    let w = m.vg.width.max(0) as usize;
    let mut row = vec![0u8; w * 2];
    if fp.fread(&mut row, 2, w) != w {
        return false;
    }
    for v in row.chunks_mut(2) {
        v.swap(0, 1);
    }
    let o = (frame as u64 * m.vox_per_vol
        + slice as u64 * m.vox_per_slice
        + y as u64 * m.vox_per_row) as usize
        * 2;
    if let Some(c) = m.chunk.as_mut() {
        c[o..o + w * 2].copy_from_slice(&row);
    }
    true
}

/// `genesisRead(fname, read_volume)` as `mri_read` calls it (FS-680 as the mode says).
pub fn genesis_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    genesis(g, out, err, env, dir, fname, read_volume, true)
}

/// [`genesis_read`] with FS-680 bypassed in both modes (FreeSurfer's function with its header
/// allocated with one frame): the fs-io probe's piece 19.
pub fn genesis_read_as_patched(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    genesis(g, out, err, env, dir, fname, read_volume, false)
}

/// `genesisRead(fname, read_volume)`: the series of GE Genesis files `fname` belongs to.
///
/// `GE_ODD` (else `GE_EVEN`) set: only the odd (even) numbers, `only using odd # GE files`. A
/// missing `fname`: `genesisRead(): error opening file <fname>`. Its name (after the last `/`):
/// `I.<n>` gives the series `I.%03d`; `<stem><digits>.MR` the series `<stem>%d.MR` and
/// `<stem>%03d.MR`, the one with more files taken (the first on a tie); any other `genesisRead():
/// can't determine file name format for <fname>`. Then `fname_format  : <folder><format>` (and
/// `fname_format2 : ...` for `.MR` names) on standard output. The files from the lowest number to
/// the highest that exist around `fname`'s are the slices (every other one with `GE_ODD` or
/// `GE_EVEN`). The first file's header (big-endian): width and height at bytes 8 and 12, the
/// image header's offset at 148; in the image header thickness (+26), pixel sizes (+50, +54), TR,
/// TI, TE in microseconds (+194, +198, +202, ints), the flip angle in degrees (+254, a short), the
/// echoes (+210, a short: `multi-echo genesis file detected (<n> echoes)...`, 0 counted as 1:
/// `zero frames specified in file - setting to 1`), the centre, normal and three corners (+130,
/// fifteen floats). `KILLIANY_SWAP` set: the normal's A component negated (`WARNING - swapping
/// normal direction!`). Each file's voxels: big-endian shorts at the offset its bytes 4 to 7
/// give, row after row (`genesisRead(): error reading from file file <name>` when one ends
/// early). A header cut short: `genesisRead(): could not read file` for each number it lacks.
#[allow(clippy::too_many_arguments)]
fn genesis(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
    crash: bool,
) -> Result<Option<Mri>, Fatal> {
    const WHO: &str = "genesisRead";
    let (mut odd_only, mut even_only) = (false, false);
    if env.get("GE_ODD").is_some() {
        odd_only = true;
        let _ = out.write_all(b"only using odd # GE files\n");
    } else if env.get("GE_EVEN").is_some() {
        even_only = true;
        let _ = out.write_all(b"only using even # GE files\n");
    }
    if !utils::file_exists(dir, fname) {
        return Ok(fail0(
            g,
            err,
            ERROR_BADFILE,
            &format!("genesisRead(): error opening file {fname}"),
        ));
    }
    if fname.len() >= STRLEN {
        g.notices.add(
            &FS_249,
            format!(
                "{WHO}: a name of {} bytes used whole (FreeSurfer's buffer holds 1023)",
                fname.len()
            ),
        );
    }
    let (fdir, base) = match fname.rfind('/') {
        None => ("", fname),
        Some(i) => (&fname[..=i], &fname[i + 1..]),
    };
    let bb = base.as_bytes();
    let (mut im_init, first, second) = if bb.starts_with(b"I.") {
        let s = Series {
            dir: fdir.to_owned(),
            pre: "I.".into(),
            pad: true,
            suf: String::new(),
        };
        (cstd::atoi(&bb[2..]), s, None)
    } else if bb.len() >= 3 && bb.ends_with(b".MR") {
        let stem = &base[..base.len() - 3];
        let digits = stem
            .bytes()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .count();
        let (pre, num) = stem.split_at(stem.len() - digits);
        let s = Series {
            dir: fdir.to_owned(),
            pre: pre.to_owned(),
            pad: false,
            suf: ".MR".into(),
        };
        let s2 = Series {
            pad: true,
            ..s.clone()
        };
        (cstd::atoi(num.as_bytes()), s, Some(s2))
    } else {
        return Ok(fail0(
            g,
            err,
            ERROR_BADPARM,
            &format!("genesisRead(): can't determine file name format for {fname}"),
        ));
    };
    if let Some(s2) = &second {
        fit(g, err, WHO, 4093, &format!("{}%d.MR", s2.pre));
        fit(g, err, WHO, 4097, &format!("{}%03d.MR", s2.pre));
    }
    fit(g, err, WHO, 4114, &first.format());
    let _ = writeln!(out, "fname_format  : {}", first.format());
    match &second {
        Some(s2) => {
            fit(g, err, WHO, 4122, &s2.format());
            let _ = writeln!(out, "fname_format2 : {}", s2.format());
        }
        None => g.notices.add(
            &FS_681,
            format!(
                "{WHO}: the name {base}: FreeSurfer's second format text is memory never written (it prints it, or nothing): none printed"
            ),
        ),
    }
    first.note_percent(g, WHO);
    let step = if odd_only || even_only {
        let even = (im_init / 2) * 2 == im_init;
        if (odd_only && even) || (even_only && !even) {
            im_init = im_init.wrapping_add(1);
        }
        2
    } else {
        1
    };
    let (mut im_low, mut im_high) = extent(dir, &first, im_init, step);
    let mut series = first;
    if let Some(s2) = second {
        let (low2, high2) = extent(dir, &s2, im_init, 1);
        if high2.wrapping_sub(low2) > im_high.wrapping_sub(im_low) {
            series = s2;
            im_low = low2;
            im_high = high2;
        }
    }
    if crash {
        g.notices.add(
            &FS_680,
            format!(
                "{WHO}: the series {}: FreeSurfer allocates its header through a null pointer here and crashes (signal 11)",
                series.format()
            ),
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
    }
    let mut header = mri::mri_alloc_header(g, out, err, 1, 1, 1, MRI_SHORT, 1)?;
    let nfiles = if odd_only || even_only {
        im_high.wrapping_sub(im_low) / 2 + 1
    } else {
        im_high.wrapping_sub(im_low).wrapping_add(1)
    };
    header.vg.depth = nfiles;
    header.imnr0 = 1;
    header.imnr1 = header.vg.depth;
    let first_name = series.name(im_low);
    let fp = match CFile::open(dir, &first_name, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            return Ok(fail0(
                g,
                err,
                ERROR_BADFILE,
                &format!("genesisRead(): error opening file {first_name}\n"),
            ));
        }
    };
    let mut r = Reader { fp, who: WHO };
    r.seek(g, 8);
    let width = r.local_int(g, err, "the width");
    let height = r.local_int(g, err, "the height");
    r.seek(g, 148);
    let image_header_offset = r.local_int(g, err, "the image header's offset");
    header.vg.width = width;
    header.vg.height = height;
    header.vg.fname.strcpy_bytes(fname.as_bytes());
    let at = |k: i32| i64::from(image_header_offset.wrapping_add(k));
    r.seek(g, at(26));
    header.thick = r.field(g, err, header.thick, "the slice thickness");
    header.vg.zsize = header.thick;
    r.seek(g, at(50));
    header.vg.xsize = r.field(g, err, header.vg.xsize, "the pixel width");
    header.vg.ysize = r.field(g, err, header.vg.ysize, "the pixel height");
    header.ps = header.vg.xsize;
    // all in microseconds
    r.seek(g, at(194));
    header.tr = (f64::from(r.fread_int(g)) / 1e3) as f32;
    r.seek(g, at(198));
    header.ti = (f64::from(r.fread_int(g)) / 1e3) as f32;
    r.seek(g, at(202));
    header.te = (f64::from(r.fread_int(g)) / 1e3) as f32;
    r.seek(g, at(254));
    header.flip_angle = (TWO_PI * f64::from(r.fread_short(g, err))) / 360.0;
    r.seek(g, at(210));
    header.nframes = i32::from(r.fread_short(g, err));
    if header.nframes > 1 {
        let _ = writeln!(
            out,
            "multi-echo genesis file detected ({} echoes)...",
            header.nframes
        );
        g.notices.add(
            &FS_685,
            format!(
                "{WHO}: the header's frame count raised to {} after its allocation of one frame: FreeSurfer reads past its frame table when it frees it (a crash or not, as memory holds): no crash here",
                header.nframes
            ),
        );
    } else if header.nframes == 0 {
        let _ = out.write_all(b"zero frames specified in file - setting to 1\n");
        header.nframes = 1;
    }
    // FS-683: the echoes' files are slices of a volume nframes times less deep
    let echoes = header.nframes > 1;
    if echoes {
        let depth = (i64::from(nfiles) + i64::from(header.nframes) - 1) / i64::from(header.nframes);
        g.notices.add(
            &FS_683,
            format!(
                "{WHO}: {nfiles} files of {} echoes: FreeSurfer's volume has {nfiles} slices (those past {depth} empty){}",
                header.nframes,
                if g.notices.reproduce() {
                    ", as in FreeSurfer"
                } else {
                    "; here it has the files' slices"
                }
            ),
        );
        if !g.notices.reproduce() {
            header.vg.depth = depth as i32;
            header.imnr1 = header.vg.depth;
        }
    }
    r.seek(g, at(130));
    let mut k = corners(g, err, &mut r);
    let nlength = length(k.n[0], k.n[1], k.n[2]);
    for c in &mut k.n {
        *c = sse::div(*c, nlength);
    }
    if env.get("KILLIANY_SWAP").is_some() {
        let _ = out.write_all(b"WARNING - swapping normal direction!\n");
        k.n[1] = -k.n[1];
    }
    geometry(&mut header, &k, false);
    r.fp.close();
    let (w, h, d, t, nf) = (
        header.vg.width,
        header.vg.height,
        header.vg.depth,
        header.mtype,
        header.nframes,
    );
    let mut m = if read_volume {
        mri::mri_alloc_sequence(g, out, err, w, h, d, t, nf)?
    } else {
        mri::mri_alloc_header(g, out, err, w, h, d, t, nf)?
    };
    mri::mri_copy_header(g, out, err, &header, Some(&mut m))?;
    if !read_volume {
        return Ok(Some(m));
    }
    let fix_echoes = echoes && !g.notices.reproduce();
    let mut slice = 0i32;
    let mut used = 0i32;
    let mut i = im_low;
    while i <= im_high {
        let frame = if fix_echoes {
            used % m.nframes
        } else {
            i.wrapping_sub(im_low) % m.nframes
        };
        let s = if fix_echoes { used / m.nframes } else { slice };
        let name = series.name(i);
        let mut fp = match CFile::open(dir, &name, "r") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("genesisRead(): error opening file {name}"),
                ));
            }
        };
        let mut rr = Reader { fp, who: WHO };
        rr.seek(g, 4);
        let pixel_data_offset = rr.local_int(g, err, "the pixel data's offset");
        rr.seek(g, i64::from(pixel_data_offset));
        fp = rr.fp;
        for y in 0..m.vg.height {
            if !read_row(&mut fp, &mut m, y, s, frame) {
                if let Some(e) = fp.read_errno() {
                    g.errno = e;
                }
                fp.close();
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("genesisRead(): error reading from file file {name}"),
                ));
            }
        }
        fp.close();
        if frame != m.nframes - 1 {
            slice -= 1;
        }
        slice += 1;
        used += 1;
        i = if odd_only || even_only {
            i.wrapping_add(2)
        } else {
            i.wrapping_add(1)
        };
        if i < im_low {
            // the series ends at the largest int
            break;
        }
    }
    Ok(Some(m))
}

/// `gelxRead(fname, read_volume)` as `mri_read` calls it (FS-680 as the mode says).
pub fn gelx_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    gelx(g, out, err, dir, fname, read_volume, true)
}

/// [`gelx_read`] with FS-680 bypassed in both modes: the fs-io probe's piece 19.
pub fn gelx_read_as_patched(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    gelx(g, out, err, dir, fname, read_volume, false)
}

/// FS-684: gelxRead's and ximgRead's messages name genesisRead.
fn lx_message(g: &mut Globals, who: &str) {
    g.notices.add(
        &FS_684,
        format!("{who}: its messages name genesisRead, as FreeSurfer's do"),
    );
}

/// `gelxRead(fname, read_volume)`: the series of GE Signa LX files `fname` belongs to.
///
/// A missing `fname`: `genesisRead(): error opening file <fname>` (FS-684: the messages name
/// genesisRead). Its name (after the last `/`) must be digits with one `e`, one `s` and one `i`:
/// the series is `<name before the i>i%d` (else `genesisRead(): can't determine file name format
/// for <fname>`); its files from the lowest number to the highest that exist around `fname`'s are
/// the slices. The first file's header (big-endian): width and height at bytes 3,236 and 3,240,
/// thickness at 2,212, pixel sizes at 2,236 and 2,240, the centre, normal and three corners from
/// 2,320 (fifteen floats); one frame, no timing. Each file's voxels: big-endian shorts from byte
/// 8,432, row after row (`genesisRead(): error reading from file file <name>`).
fn gelx(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
    crash: bool,
) -> Result<Option<Mri>, Fatal> {
    const WHO: &str = "gelxRead";
    if !utils::file_exists(dir, fname) {
        lx_message(g, WHO);
        return Ok(fail0(
            g,
            err,
            ERROR_BADFILE,
            &format!("genesisRead(): error opening file {fname}"),
        ));
    }
    if fname.len() >= STRLEN {
        g.notices.add(
            &FS_249,
            format!(
                "{WHO}: a name of {} bytes used whole (FreeSurfer's buffer holds 1023)",
                fname.len()
            ),
        );
    }
    let (fdir, base) = match fname.rfind('/') {
        None => ("", fname),
        Some(i) => (&fname[..=i], &fname[i + 1..]),
    };
    let (mut e, mut s, mut ic, mut good) = (0, 0, 0, true);
    for c in base.bytes() {
        match c {
            b'e' => e += 1,
            b's' => s += 1,
            b'i' => ic += 1,
            c if c.is_ascii_digit() => {}
            _ => good = false,
        }
    }
    if !(good && e == 1 && s == 1 && ic == 1) {
        lx_message(g, WHO);
        return Ok(fail0(
            g,
            err,
            ERROR_BADPARM,
            &format!("genesisRead(): can't determine file name format for {fname}"),
        ));
    }
    let at_i = base.rfind('i').unwrap_or(0);
    let im_init = cstd::atoi(&base.as_bytes()[at_i + 1..]);
    let series = Series {
        dir: fdir.to_owned(),
        pre: format!("{}i", &base[..at_i]),
        pad: false,
        suf: String::new(),
    };
    fit(g, err, WHO, 4507, &format!("{}%d", series.pre));
    fit(g, err, WHO, 4517, &format!("{}%d", series.pre));
    fit(g, err, WHO, 4521, &series.format());
    series.note_percent(g, WHO);
    let (im_low, im_high) = extent(dir, &series, im_init, 1);
    if crash {
        g.notices.add(
            &FS_680,
            format!(
                "{WHO}: the series {}: FreeSurfer allocates its header through a null pointer here and crashes (signal 11)",
                series.format()
            ),
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
    }
    let mut header = mri::mri_alloc_header(g, out, err, 1, 1, 1, MRI_SHORT, 1)?;
    header.vg.depth = im_high.wrapping_sub(im_low).wrapping_add(1);
    header.imnr0 = 1;
    header.imnr1 = header.vg.depth;
    let first_name = series.name(im_low);
    let fp = match CFile::open(dir, &first_name, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            lx_message(g, WHO);
            return Ok(fail0(
                g,
                err,
                ERROR_BADFILE,
                &format!("genesisRead(): error opening file {first_name}\n"),
            ));
        }
    };
    let mut r = Reader { fp, who: WHO };
    r.seek(g, 3236);
    let width = r.local_int(g, err, "the width");
    let height = r.local_int(g, err, "the height");
    header.vg.width = width;
    header.vg.height = height;
    header.vg.fname.strcpy_bytes(fname.as_bytes());
    r.seek(g, 2184 + 28);
    header.thick = r.field(g, err, header.thick, "the slice thickness");
    header.vg.zsize = header.thick;
    r.seek(g, 2184 + 52);
    header.vg.xsize = r.field(g, err, header.vg.xsize, "the pixel width");
    header.vg.ysize = r.field(g, err, header.vg.ysize, "the pixel height");
    header.ps = header.vg.xsize;
    r.seek(g, 2184 + 136);
    let k = corners(g, err, &mut r);
    geometry(&mut header, &k, true);
    r.fp.close();
    let (w, h, d, nf) = (
        header.vg.width,
        header.vg.height,
        header.vg.depth,
        header.nframes,
    );
    let mut m = if read_volume {
        mri::mri_alloc(g, out, err, w, h, d, MRI_SHORT)?
    } else {
        mri::mri_alloc_header(g, out, err, w, h, d, MRI_SHORT, nf)?
    };
    mri::mri_copy_header(g, out, err, &header, Some(&mut m))?;
    if !read_volume {
        return Ok(Some(m));
    }
    let mut i = im_low;
    while i <= im_high {
        let name = series.name(i);
        let mut fp = match CFile::open(dir, &name, "r") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                lx_message(g, WHO);
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("genesisRead(): error opening file {name}"),
                ));
            }
        };
        let mut errno = g.errno;
        fp.fseek(&mut errno, 8432, 0);
        g.errno = errno;
        let slice = i.wrapping_sub(im_low);
        for y in 0..m.vg.height {
            if !read_row(&mut fp, &mut m, y, slice, 0) {
                if let Some(e) = fp.read_errno() {
                    g.errno = e;
                }
                fp.close();
                lx_message(g, WHO);
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("genesisRead(): error reading from file file {name}"),
                ));
            }
        }
        fp.close();
        i = i.wrapping_add(1);
        if i < im_low {
            break;
        }
    }
    Ok(Some(m))
}

/// XIMG's fixed offsets: the voxels and the image header (whatever the file says).
const XIMG_PIXEL_DATA_OFFSET: i32 = 8432;
const XIMG_IMAGE_HEADER_OFFSET: i32 = 2308;

/// `ximgRead(fname, read_volume)`: a GE XIMG series (Genesis files with XIMG's header
/// corrections), as `mri_read` calls it.
///
/// `XIMG: using XIMG header corrections` first (standard output). The name and the series as
/// genesisRead's (§110 of the spec) with one series for `.MR` names (`<stem>%d.MR`) and no
/// `GE_ODD`/`GE_EVEN`; the messages name genesisRead (FS-684). The first file's pixel data offset
/// (byte 4) and image header offset (byte 148) are printed (`XIMG: pixel data offset is <n>, using
/// offset 8432`, `XIMG: image header offset is <n>, using offset 2308`) and replaced by those
/// constants; the image header's fields at genesisRead's places but the thickness and pixel sizes 2
/// bytes later, TR and the corners 6 bytes later (TI and TE where genesisRead reads them, inside
/// TR's bytes); one frame; `KILLIANY_SWAP` as genesisRead. The voxels from byte 8,432 of each file.
pub fn ximg_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    const WHO: &str = "ximgRead";
    let _ = out.write_all(b"XIMG: using XIMG header corrections\n");
    if !utils::file_exists(dir, fname) {
        lx_message(g, WHO);
        return Ok(fail0(
            g,
            err,
            ERROR_BADFILE,
            &format!("genesisRead(): error opening file {fname}"),
        ));
    }
    if fname.len() >= STRLEN {
        g.notices.add(
            &FS_249,
            format!(
                "{WHO}: a name of {} bytes used whole (FreeSurfer's buffer holds 1023)",
                fname.len()
            ),
        );
    }
    let (fdir, base) = match fname.rfind('/') {
        None => ("", fname),
        Some(i) => (&fname[..=i], &fname[i + 1..]),
    };
    let bb = base.as_bytes();
    let (im_init, series) = if bb.starts_with(b"I.") {
        let s = Series {
            dir: fdir.to_owned(),
            pre: "I.".into(),
            pad: true,
            suf: String::new(),
        };
        (cstd::atoi(&bb[2..]), s)
    } else if bb.len() >= 3 && bb.ends_with(b".MR") {
        let stem = &base[..base.len() - 3];
        let digits = stem
            .bytes()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .count();
        let (pre, num) = stem.split_at(stem.len() - digits);
        fit(g, err, WHO, 7341, &format!("{pre}%d.MR"));
        let s = Series {
            dir: fdir.to_owned(),
            pre: pre.to_owned(),
            pad: false,
            suf: ".MR".into(),
        };
        (cstd::atoi(num.as_bytes()), s)
    } else {
        lx_message(g, WHO);
        return Ok(fail0(
            g,
            err,
            ERROR_BADPARM,
            &format!("genesisRead(): can't determine file name format for {fname}"),
        ));
    };
    let own = format!(
        "{}{}{}",
        series.pre,
        if series.pad { "%03d" } else { "%d" },
        series.suf
    );
    fit(g, err, WHO, 7356, &own);
    fit(g, err, WHO, 7360, &series.format());
    series.note_percent(g, WHO);
    let (im_low, im_high) = extent(dir, &series, im_init, 1);
    let mut header = mri::mri_alloc_header(g, out, err, 1, 1, 1, MRI_SHORT, 1)?;
    header.vg.depth = im_high.wrapping_sub(im_low).wrapping_add(1);
    header.imnr0 = 1;
    header.imnr1 = header.vg.depth;
    let first_name = series.name(im_low);
    let fp = match CFile::open(dir, &first_name, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            lx_message(g, WHO);
            return Ok(fail0(
                g,
                err,
                ERROR_BADFILE,
                &format!("genesisRead(): error opening file {first_name}\n"),
            ));
        }
    };
    let mut r = Reader { fp, who: WHO };
    r.seek(g, 4);
    let pixel = r.local_int(g, err, "the pixel data's offset");
    let _ = write!(out, "XIMG: pixel data offset is {pixel}, ");
    let _ = writeln!(out, "using offset {XIMG_PIXEL_DATA_OFFSET}");
    r.seek(g, 8);
    let width = r.local_int(g, err, "the width");
    let height = r.local_int(g, err, "the height");
    r.seek(g, 148);
    let ih = r.local_int(g, err, "the image header's offset");
    let _ = write!(out, "XIMG: image header offset is {ih}, ");
    let _ = writeln!(out, "using offset {XIMG_IMAGE_HEADER_OFFSET}");
    let at = |k: i32| i64::from(XIMG_IMAGE_HEADER_OFFSET + k);
    header.vg.width = width;
    header.vg.height = height;
    header.vg.fname.strcpy_bytes(fname.as_bytes());
    r.seek(g, at(26 + 2));
    header.thick = r.field(g, err, header.thick, "the slice thickness");
    header.vg.zsize = header.thick;
    r.seek(g, at(50 + 2));
    header.vg.xsize = r.field(g, err, header.vg.xsize, "the pixel width");
    header.vg.ysize = r.field(g, err, header.vg.ysize, "the pixel height");
    header.ps = header.vg.xsize;
    r.seek(g, at(194 + 6));
    header.tr = (f64::from(r.fread_int(g)) / 1e3) as f32;
    r.seek(g, at(198));
    header.ti = (f64::from(r.fread_int(g)) / 1e3) as f32;
    r.seek(g, at(202));
    header.te = (f64::from(r.fread_int(g)) / 1e3) as f32;
    r.seek(g, at(254));
    header.flip_angle = (TWO_PI * f64::from(r.fread_short(g, err))) / 360.0;
    r.seek(g, at(130 + 6));
    let mut k = corners(g, err, &mut r);
    let nlength = length(k.n[0], k.n[1], k.n[2]);
    for c in &mut k.n {
        *c = sse::div(*c, nlength);
    }
    if env.get("KILLIANY_SWAP").is_some() {
        let _ = out.write_all(b"WARNING - swapping normal direction!\n");
        k.n[1] = -k.n[1];
    }
    geometry(&mut header, &k, false);
    r.fp.close();
    let (w, h, d) = (header.vg.width, header.vg.height, header.vg.depth);
    let mut m = if read_volume {
        mri::mri_alloc(g, out, err, w, h, d, header.mtype)?
    } else {
        mri::mri_alloc_header(g, out, err, w, h, d, header.mtype, 1)?
    };
    mri::mri_copy_header(g, out, err, &header, Some(&mut m))?;
    if !read_volume {
        return Ok(Some(m));
    }
    let mut i = im_low;
    while i <= im_high {
        let name = series.name(i);
        let mut fp = match CFile::open(dir, &name, "r") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                lx_message(g, WHO);
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("genesisRead(): error opening file {name}"),
                ));
            }
        };
        let mut errno = g.errno;
        fp.fseek(&mut errno, i64::from(XIMG_PIXEL_DATA_OFFSET), 0);
        g.errno = errno;
        let slice = i.wrapping_sub(im_low);
        for y in 0..m.vg.height {
            if !read_row(&mut fp, &mut m, y, slice, 0) {
                if let Some(e) = fp.read_errno() {
                    g.errno = e;
                }
                fp.close();
                lx_message(g, WHO);
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("genesisRead(): error reading from file file {name}"),
                ));
            }
        }
        fp.close();
        i = i.wrapping_add(1);
        if i < im_low {
            break;
        }
    }
    Ok(Some(m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    fn put(b: &mut Vec<u8>, at: usize, v: &[u8]) {
        if b.len() < at + v.len() {
            b.resize(at + v.len(), 0);
        }
        b[at..at + v.len()].copy_from_slice(v);
    }

    /// A Genesis file of `w` x `h` voxels `base + k`, the image header at 156, `echoes` echoes,
    /// an axial slice 2 mm thick with 1 x 1.5 mm pixels around (0, 0, z).
    fn genesis_file(w: i32, h: i32, base: i16, echoes: i16, z: f32) -> Vec<u8> {
        let ih = 156usize;
        let pixel = ih + 256;
        let mut b = vec![0u8; pixel];
        b[..4].copy_from_slice(b"IMGF");
        put(&mut b, 4, &(pixel as i32).to_be_bytes());
        put(&mut b, 8, &w.to_be_bytes());
        put(&mut b, 12, &h.to_be_bytes());
        put(&mut b, 148, &(ih as i32).to_be_bytes());
        put(&mut b, ih + 26, &2.0f32.to_be_bytes());
        put(&mut b, ih + 50, &1.0f32.to_be_bytes());
        put(&mut b, ih + 54, &1.5f32.to_be_bytes());
        // centre, normal, top-left, top-right, bottom-right
        let v = [
            0.0f32, 0.0, z, 0.0, 0.0, 2.0, -2.0, 3.0, z, 2.0, 3.0, z, 2.0, -3.0, z,
        ];
        for (k, x) in v.iter().enumerate() {
            put(&mut b, ih + 130 + 4 * k, &x.to_be_bytes());
        }
        put(&mut b, ih + 194, &2_000_000i32.to_be_bytes());
        put(&mut b, ih + 198, &900_000i32.to_be_bytes());
        put(&mut b, ih + 202, &30_000i32.to_be_bytes());
        put(&mut b, ih + 210, &echoes.to_be_bytes());
        put(&mut b, ih + 254, &90i16.to_be_bytes());
        for k in 0..w * h {
            b.extend_from_slice(&(base + k as i16).to_be_bytes());
        }
        b
    }

    fn shorts(m: &Mri) -> Vec<i16> {
        m.chunk
            .as_ref()
            .unwrap()
            .chunks(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect()
    }

    /// FS-680: `reproduce` prints FreeSurfer's lines and crashes; the default reads the series.
    #[test]
    fn a_genesis_series_reads_by_default_and_crashes_as_freesurfer() {
        let dir = tempfile::tempdir().unwrap();
        for n in 1..=3 {
            let f = genesis_file(2, 2, 10 * n as i16, 1, n as f32 * 2.0);
            std::fs::write(dir.path().join(format!("I.{n:03}")), f).unwrap();
        }
        let p = Some(dir.path());
        let env = Environment::default();
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = genesis_read(&mut g, &mut o, &mut e, &env, p, "I.002", true);
        assert_eq!(r.unwrap_err(), Fatal::Exit(139));
        assert_eq!(String::from_utf8(o).unwrap(), "fname_format  : I.%03d\n");
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let m = genesis_read(&mut g, &mut o, &mut e, &env, p, "I.002", true)
            .unwrap()
            .unwrap();
        assert!(g.notices.as_slice().iter().any(|n| n.defect.id == "FS-680"));
        assert_eq!(
            (m.vg.width, m.vg.height, m.vg.depth, m.nframes),
            (2, 2, 3, 1)
        );
        assert_eq!(
            shorts(&m),
            vec![10, 11, 12, 13, 20, 21, 22, 23, 30, 31, 32, 33]
        );
        assert_eq!((m.vg.xsize, m.vg.ysize, m.vg.zsize), (1.0, 1.5, 2.0));
        assert_eq!((m.tr, m.ti, m.te), (2000.0, 900.0, 30.0));
        assert!((m.flip_angle - std::f64::consts::FRAC_PI_2).abs() < 1e-15);
        // x along the top edge, y down the right edge, z the normal
        assert_eq!((m.vg.x_r, m.vg.x_a, m.vg.x_s), (1.0, 0.0, 0.0));
        assert_eq!((m.vg.y_r, m.vg.y_a, m.vg.y_s), (0.0, -1.0, 0.0));
        assert_eq!((m.vg.z_r, m.vg.z_a, m.vg.z_s), (0.0, 0.0, 1.0));
        // the first slice's centre plus half the stack: (3 - 1) * 2 mm / 2
        assert_eq!((m.vg.c_r, m.vg.c_a, m.vg.c_s), (0.0, 0.0, 4.0));
    }

    /// FS-683: two echoes of two slices; FreeSurfer's volume has 4 slices (2 empty), the
    /// default's 2.
    #[test]
    fn echoes_are_frames_of_the_files_slices() {
        let dir = tempfile::tempdir().unwrap();
        for n in 1..=4 {
            let f = genesis_file(1, 1, n as i16, 2, 0.0);
            std::fs::write(dir.path().join(format!("s{n}.MR")), f).unwrap();
        }
        let p = Some(dir.path());
        let env = Environment::default();
        for (mode, depth, voxels) in [
            (UpstreamDefects::Fix, 2, vec![1, 3, 2, 4]),
            (UpstreamDefects::Reproduce, 4, vec![1, 3, 0, 0, 2, 4, 0, 0]),
        ] {
            let mut g = Globals::new(mode);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let m = genesis_read_as_patched(&mut g, &mut o, &mut e, &env, p, "s1.MR", true)
                .unwrap()
                .unwrap();
            assert_eq!((m.vg.depth, m.nframes), (depth, 2));
            assert_eq!(shorts(&m), voxels);
            let ids: Vec<_> = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
            assert!(ids.contains(&"FS-683") && ids.contains(&"FS-685"));
        }
    }

    /// A GE LX series: e1s2i<n>, voxels from byte 8,432.
    #[test]
    fn a_gelx_series_reads() {
        let dir = tempfile::tempdir().unwrap();
        for n in 5..=6 {
            let mut b = vec![0u8; 8432];
            put(&mut b, 3228, b"IMGF");
            put(&mut b, 3236, &3i32.to_be_bytes());
            put(&mut b, 3240, &1i32.to_be_bytes());
            put(&mut b, 2184 + 28, &1.0f32.to_be_bytes());
            put(&mut b, 2184 + 52, &1.0f32.to_be_bytes());
            put(&mut b, 2184 + 56, &1.0f32.to_be_bytes());
            let v = [
                0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, -1.0, 1.0, 0.0, -1.0, -1.0,
            ];
            for (k, x) in v.iter().enumerate() {
                put(&mut b, 2184 + 136 + 4 * k, &x.to_be_bytes());
            }
            for k in 0..3i16 {
                b.extend_from_slice(&(100 * n + k).to_be_bytes());
            }
            std::fs::write(dir.path().join(format!("e1s2i{n}")), b).unwrap();
        }
        let p = Some(dir.path());
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = gelx_read(&mut g, &mut o, &mut e, p, "e1s2i6", true);
        assert_eq!(r.unwrap_err(), Fatal::Exit(139));
        assert!(o.is_empty() && e.is_empty());
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let m = gelx_read(&mut g, &mut o, &mut e, p, "e1s2i6", true)
            .unwrap()
            .unwrap();
        assert_eq!((m.vg.width, m.vg.height, m.vg.depth), (3, 1, 2));
        assert_eq!(shorts(&m), vec![500, 501, 502, 600, 601, 602]);
        assert_eq!((m.vg.x_r, m.vg.x_a, m.vg.x_s), (0.0, -1.0, 0.0));
        assert_eq!((m.vg.y_r, m.vg.y_a, m.vg.y_s), (0.0, 0.0, -1.0));
        std::fs::write(dir.path().join("e1s2x6"), b"x").unwrap();
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        assert!(
            gelx_read(&mut g, &mut o, &mut e, p, "e1s2x6", true)
                .unwrap()
                .is_none()
        );
        let text = String::from_utf8(e).unwrap();
        assert!(text.contains("genesisRead(): can't determine file name format for e1s2x6"));
    }
}
