// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! COR volumes (`utils/mriio.cpp` 1362-1903, item 3.1-27): `corRead` ([`cor_read`]: a folder of
//! 8-bit coronal slices `COR-001` ... with the header `COR-.info`) and `corWrite`
//! ([`cor_write`]). Wired into [`crate::mriio::mri_read`] and [`crate::mriio::mri_write_type`]
//! (`MRI_CORONAL_SLICE_DIRECTORY`).
//!
//! Defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-io.md`, Part 19):
//! FS-657 (a required field without its number: memory contents), FS-658 (the flip angle read
//! eleven bytes into its line), FS-659 (writing a volume that is not bytes), FS-660 (writing more
//! slices than the volume has), FS-58 (writing a volume without voxels), FS-249 (names past
//! FreeSurfer's buffers).

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_166, FS_249, FS_657, FS_658, FS_659, FS_660, FS_672};
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::diag::{DIAG_SHOW, verbose_on};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BAD_FILE, ERROR_BADFILE, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::{sse, utils};
use larmorx_freesurfer_mri::mri::{
    self, MRI_FLOAT, MRI_FLOAT_COMPLEX, MRI_INT, MRI_LONG, MRI_RGB, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri,
};

use crate::cfile::CFile;
use crate::minc;
use crate::mriio_nifti::{narrow, widen};

/// `STRLEN`: FreeSurfer's name and line buffers.
const STRLEN: usize = 1024;

/// `EEXIST`.
const EEXIST: i32 = 17;

/// `2.0 * M_PI`, as `RADIANS` and `DEGREES` fold it.
const TWO_PI: f64 = 2.0 * std::f64::consts::PI;

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// A C string's bytes: up to its first NUL.
fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// `ErrorReturn(NULL, (code, message))` with `errno` cleared first.
fn fail0(g: &mut Globals, err: &mut dyn Write, code: i32, message: &str) -> Option<Mri> {
    g.errno = 0;
    log::error_printf(g, err, code, message);
    None
}

/// `ErrorReturn(code, (code, message))` with `errno` cleared first.
fn fail_code(g: &mut Globals, err: &mut dyn Write, code: i32, message: &str) -> i32 {
    g.errno = 0;
    log::error_printf(g, err, code, message);
    code
}

fn fits(g: &mut Globals, who: &str, name: &str) {
    if name.len() >= STRLEN {
        g.notices.add(
            &FS_249,
            format!(
                "{who}: a name of {} bytes used whole (FreeSurfer's buffer holds 1023)",
                name.len()
            ),
        );
    }
}

/// `stat(name)`: whether it is a folder (`Err`: `stat` failed, `errno` set).
fn stat_dir(g: &mut Globals, dir: Option<&Path>, name: &str) -> Result<bool, ()> {
    if name.is_empty() {
        g.errno = 2;
        return Err(());
    }
    match std::fs::metadata(utils::resolve(dir, name)) {
        Ok(md) => Ok(md.is_dir()),
        Err(e) => {
            g.errno = log::errno_of(&e).unwrap_or(2);
            Err(())
        }
    }
}

/// `sscanf(s, "%*s <conv>")` of the line's C string.
fn word_then<T>(
    g: &mut Globals,
    l: &[u8],
    conv: impl FnOnce(&mut Scanf, &mut i32) -> Option<T>,
) -> Option<T> {
    let mut b = Bytes::new(l);
    let mut errno = g.errno;
    let mut sc = Scanf::new(&mut b);
    sc.s(&mut errno, usize::MAX, false);
    sc.text(&mut errno, b" ");
    let v = conv(&mut sc, &mut errno);
    sc.finish(&mut errno);
    g.errno = errno;
    v
}

/// A required header field: its flag and its value (`None`: never written, memory contents in
/// FreeSurfer).
#[derive(Clone, Copy, Default)]
struct Req<T> {
    got: bool,
    v: Option<T>,
}

impl<T: Copy + Default> Req<T> {
    fn set(&mut self, v: Option<T>) {
        self.got = true;
        if v.is_some() {
            self.v = v;
        }
    }
    /// The value; FS-657 (0) when FreeSurfer's is memory contents.
    fn get(&self, g: &mut Globals, name: &str) -> T {
        match self.v {
            Some(v) => v,
            None => {
                g.notices.add(
                    &FS_657,
                    format!(
                        "corRead: the field {name} without a number: 0 (FreeSurfer: memory contents)"
                    ),
                );
                T::default()
            }
        }
    }
}

/// The flip angle's number as FreeSurfer reads it: `sscanf(line + 11, "%f")`, eleven bytes into
/// the line buffer whatever the line's length. `buf` holds what earlier lines left in the buffer
/// past this line's end (`written` of its bytes were ever written). FS-658.
fn flip_number(g: &mut Globals, buf: &[u8], written: usize, line_len: usize) -> Option<f32> {
    let scan = |g: &mut Globals, s: &[u8]| -> Option<f32> {
        let mut b = Bytes::new(s);
        let mut errno = g.errno;
        let mut sc = Scanf::new(&mut b);
        let v = sc.f(&mut errno, true);
        sc.finish(&mut errno);
        g.errno = errno;
        v
    };
    if line_len >= 11 {
        return scan(g, cstr(&buf[11..]));
    }
    if g.notices.fix() {
        g.notices.add(
            &FS_658,
            "corRead: a flip line shorter than 'flip angle ': no number (FreeSurfer reads past the line's end)",
        );
        // glibc's sscanf of an empty text: errno 0
        return scan(g, b"");
    }
    // the bytes after the line's end: an earlier line's, or never written (memory contents)
    let s = cstr(&buf[11..]);
    if 11 + s.len() >= written {
        g.notices.add(
            &FS_658,
            "corRead: the flip angle read past the line's end into memory never written (0 bytes here)",
        );
    } else {
        g.notices.add(
            &FS_658,
            "corRead: the flip angle read past the line's end from an earlier line's bytes, as in FreeSurfer",
        );
    }
    scan(g, s)
}

/// FS-672: FreeSurfer frees a volume whose Talairach transform failed to load (its transform
/// never written, but marked as the volume's to free).
fn freed(g: &mut Globals, bad_transform: bool) {
    if bad_transform {
        g.notices.add(
            &FS_672,
            "a volume whose Talairach transform failed to load, freed (FreeSurfer frees a transform never written: memory contents, the program may end with status 1)",
        );
    }
}

/// `corRead(fname, read_volume)`: a COR volume: the folder `fname` (or the folder of a file in
/// it), its header `COR-.info` and, when `read_volume`, its slices `COR-<nnn>` from `imnr0` to
/// `imnr1`, `x` bytes by `y` rows each. The header's lines are a name and values: `imnr0`,
/// `imnr1`, `ptype`, `x`, `y`, `thick`, `psiz`, `strtx` ... `endz` required (`missing fields in
/// file <name>:` and one line per missing field as errors, then `None`); `fov`, `locatn`, `tr`,
/// `te`, `ti`, `flip angle` (degrees), `ras_good_flag`, `x_ras` ... `c_ras`, `xform` (or
/// `transform`: the Talairach transform's name, relative to the folder; else
/// `../transforms/talairach.xfm`). Non-standard values print warnings; sizes and positions are in
/// metres (times 1,000). See `specs/freesurfer-io.md` §104.
#[allow(clippy::too_many_arguments)]
pub fn cor_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    fits(g, "corRead", fname);
    let mut use_name = fname.as_bytes().to_vec();
    match stat_dir(g, dir, fname) {
        Err(()) => {
            return Ok(fail0(
                g,
                err,
                ERROR_BADFILE,
                &format!("corRead(): can't stat {fname}"),
            ));
        }
        Ok(true) => {}
        Ok(false) => {
            // the name up to its last '/' (its final character not looked at); none: empty
            let n = use_name.len();
            let cut = use_name[..n.saturating_sub(1)]
                .iter()
                .rposition(|&c| c == b'/')
                .unwrap_or(0);
            use_name.truncate(cut);
            let s = text(&use_name);
            match stat_dir(g, dir, &s) {
                Err(()) => {
                    return Ok(fail0(
                        g,
                        err,
                        ERROR_BADFILE,
                        &format!("corRead(): can't stat {s}"),
                    ));
                }
                Ok(false) => {
                    return Ok(fail0(
                        g,
                        err,
                        ERROR_BADFILE,
                        &format!("corRead(): {s} isn't a directory"),
                    ));
                }
                Ok(true) => {}
            }
        }
    }
    if use_name.last() != Some(&b'/') {
        use_name.push(b'/');
    }
    let base = text(&use_name);
    let info = format!("{base}COR-.info");
    let mut fp = match CFile::open(dir, &info, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            return Ok(fail0(
                g,
                err,
                ERROR_BADFILE,
                &format!("corRead(): can't open file {info}"),
            ));
        }
    };
    let mut xform: Vec<u8> = Vec::new();
    let mut ras_good_flag = 0i32;
    let (mut x_r, mut x_a, mut x_s) = (0f32, 0f32, 0f32);
    let (mut y_r, mut y_a, mut y_s) = (0f32, 0f32, 0f32);
    let (mut z_r, mut z_a, mut z_s) = (0f32, 0f32, 0f32);
    let (mut c_r, mut c_a, mut c_s) = (0f32, 0f32, 0f32);
    let (mut flip_angle, mut tr, mut te, mut ti) = (0f32, 0f32, 0f32, 0f32);
    let mut fov = 0f64;
    let mut locatn = 0f64;
    let (mut imnr0, mut imnr1, mut ptype, mut x, mut y) = (
        Req::<i32>::default(),
        Req::<i32>::default(),
        Req::<i32>::default(),
        Req::<i32>::default(),
        Req::<i32>::default(),
    );
    let (mut thick, mut psiz) = (Req::<f64>::default(), Req::<f64>::default());
    let mut se: [Req<f32>; 6] = [Req::default(); 6];
    // the line buffer, as fgets leaves it (its bytes after the current line's end come from
    // earlier lines; `written` of them were ever written)
    let mut buf = vec![0u8; STRLEN];
    let mut written = 0usize;
    while let Some(raw) = fp.fgets(STRLEN) {
        buf[..raw.len()].copy_from_slice(&raw);
        buf[raw.len()] = 0;
        written = written.max(raw.len() + 1);
        let line = cstr(&buf).to_vec();
        let l = line.as_slice();
        let has = |p: &[u8]| l.starts_with(p);
        let d = |g: &mut Globals| word_then(g, l, |sc, e| sc.d(e, true));
        let f = |g: &mut Globals| word_then(g, l, |sc, e| sc.f(e, true));
        let lf = |g: &mut Globals| word_then(g, l, |sc, e| sc.lf(e, true));
        let f3 = |g: &mut Globals, v: [&mut f32; 3]| {
            let mut b = Bytes::new(l);
            let mut errno = g.errno;
            let mut sc = Scanf::new(&mut b);
            sc.s(&mut errno, usize::MAX, false);
            for slot in v {
                sc.text(&mut errno, b" ");
                if let Some(x) = sc.f(&mut errno, true) {
                    *slot = x;
                }
            }
            sc.finish(&mut errno);
            g.errno = errno;
        };
        if has(b"imnr0 ") {
            let v = d(g);
            imnr0.set(v);
        } else if has(b"imnr1 ") {
            let v = d(g);
            imnr1.set(v);
        } else if has(b"ptype ") {
            let v = d(g);
            ptype.set(v);
        } else if has(b"x ") {
            let v = d(g);
            x.set(v);
        } else if has(b"y ") {
            let v = d(g);
            y.set(v);
        } else if has(b"fov ") {
            if let Some(v) = lf(g) {
                fov = v;
            }
        } else if has(b"thick ") {
            let v = lf(g);
            thick.set(v);
        } else if has(b"flip ") {
            if let Some(v) = flip_number(g, &buf, written, l.len()) {
                flip_angle = v;
            }
            flip_angle = narrow(sse::div64(sse::mul64(widen(flip_angle), TWO_PI), 360.0));
        } else if has(b"psiz ") {
            let v = lf(g);
            psiz.set(v);
        } else if has(b"locatn ") {
            if let Some(v) = lf(g) {
                locatn = v;
            }
        } else if let Some(i) = [
            b"strtx ".as_slice(),
            b"endx ",
            b"strty ",
            b"endy ",
            b"strtz ",
            b"endz ",
        ]
        .iter()
        .position(|p| has(p))
        {
            let v = f(g);
            se[i].set(v);
        } else if has(b"tr ") {
            if let Some(v) = f(g) {
                tr = v;
            }
        } else if has(b"te ") {
            if let Some(v) = f(g) {
                te = v;
            }
        } else if has(b"ti ") {
            if let Some(v) = f(g) {
                ti = v;
            }
        } else if has(b"ras_good_flag ") {
            if let Some(v) = d(g) {
                ras_good_flag = v;
            }
        } else if has(b"x_ras ") {
            f3(g, [&mut x_r, &mut x_a, &mut x_s]);
        } else if has(b"y_ras ") {
            f3(g, [&mut y_r, &mut y_a, &mut y_s]);
        } else if has(b"z_ras ") {
            f3(g, [&mut z_r, &mut z_a, &mut z_s]);
        } else if has(b"c_ras ") {
            f3(g, [&mut c_r, &mut c_a, &mut c_s]);
        } else if (has(b"xform") || has(b"transform"))
            && let Some(w) = word_then(g, l, |sc, e| sc.s(e, usize::MAX, true))
        {
            xform = w;
        }
    }
    let _ = locatn;
    drop(fp);
    let required = [
        (imnr0.got, "imnr0"),
        (imnr1.got, "imnr1"),
        (ptype.got, "ptype"),
        (x.got, "x"),
        (y.got, "y"),
        (thick.got, "thick"),
        (psiz.got, "psiz"),
        (se[0].got, "strtx"),
        (se[1].got, "endx"),
        (se[2].got, "strty"),
        (se[3].got, "endy"),
        (se[4].got, "strtz"),
        (se[5].got, "endz"),
    ];
    if required.iter().any(|(got, _)| !got) {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            &format!("missing fields in file {info}:"),
        );
        for (got, name) in required {
            if !got {
                log::error_printf(g, err, ERROR_BADFILE, &format!("  {name} field missing"));
            }
        }
        return Ok(None);
    }
    let imnr0 = imnr0.get(g, "imnr0");
    let imnr1 = imnr1.get(g, "imnr1");
    let ptype = ptype.get(g, "ptype");
    let xv = x.get(g, "x");
    let yv = y.get(g, "y");
    let thick = thick.get(g, "thick");
    let psiz = psiz.get(g, "psiz");
    let names = ["strtx", "endx", "strty", "endy", "strtz", "endz"];
    let mut sev = [0f32; 6];
    for i in 0..6 {
        sev[i] = se[i].get(g, names[i]);
    }
    for (bad, what) in [
        (imnr0 != 1, format!("imnr0 ({imnr0}, usually 1)")),
        (imnr1 != 256, format!("imnr1 ({imnr1}, usually 256)")),
        (ptype != 2, format!("ptype ({ptype}, usually 2)")),
        (xv != 256, format!("x ({xv}, usually 256)")),
        (yv != 256, format!("y ({yv}, usually 256)")),
        (
            thick != 0.001,
            format!("thick ({}, usually 0.001)", cfmt::g(thick, 6)),
        ),
        (
            psiz != 0.001,
            format!("psiz ({}, usually 0.001)", cfmt::g(psiz, 6)),
        ),
    ] {
        if bad {
            let _ = writeln!(out, "warning: non-standard value for {what} in file {info}");
        }
    }
    let depth = imnr1.wrapping_sub(imnr0).wrapping_add(1);
    let mut m = if read_volume {
        mri::mri_alloc(g, out, err, xv, yv, depth, MRI_UCHAR)?
    } else {
        mri::mri_alloc_header(g, out, err, xv, yv, depth, MRI_UCHAR, 1)?
    };
    if [x_r, x_a, x_s, y_r, y_a, y_s, z_r, z_a, z_s]
        .iter()
        .all(|&v| v == 0.0)
        && g.gdiag & DIAG_SHOW != 0
        && verbose_on(g)
    {
        let _ = write!(
            err,
            "----------------------------------------------------\nCould not find the direction cosine information.\nWill use the CORONAL orientation.\nIf not suitable, please provide the information in COR-.info file\n----------------------------------------------------\n"
        );
        x_r = -1.0;
        y_s = -1.0;
        z_a = 1.0;
        ras_good_flag = 0;
    }
    m.imnr0 = imnr0;
    m.imnr1 = imnr1;
    m.fov = narrow(sse::mul64(fov, 1000.0));
    m.thick = narrow(sse::mul64(thick, 1000.0));
    m.ps = narrow(sse::mul64(psiz, 1000.0));
    m.vg.xsize = m.ps;
    m.vg.ysize = m.ps;
    m.vg.zsize = m.thick;
    m.xstart = sse::mul(sev[0], 1000.0);
    m.xend = sse::mul(sev[1], 1000.0);
    m.ystart = sse::mul(sev[2], 1000.0);
    m.yend = sse::mul(sev[3], 1000.0);
    m.zstart = sse::mul(sev[4], 1000.0);
    m.zend = sse::mul(sev[5], 1000.0);
    m.vg.fname.strcpy_bytes(fname.as_bytes());
    m.tr = tr;
    m.te = te;
    m.ti = ti;
    m.flip_angle = f64::from(flip_angle);
    m.vg.ras_good_flag = ras_good_flag;
    (m.vg.x_r, m.vg.x_a, m.vg.x_s) = (x_r, x_a, x_s);
    (m.vg.y_r, m.vg.y_a, m.vg.y_s) = (y_r, y_a, y_s);
    (m.vg.z_r, m.vg.z_a, m.vg.z_s) = (z_r, z_a, z_s);
    (m.vg.c_r, m.vg.c_a, m.vg.c_s) = (c_r, c_a, c_s);
    let mut bad_transform = false;
    if !xform.is_empty() {
        let mut t = if xform[0] == b'/' {
            xform.clone()
        } else {
            let mut t = fname.as_bytes().to_vec();
            t.push(b'/');
            t.extend_from_slice(&xform);
            if t.len() >= STRLEN {
                let _ = writeln!(err, "corRead: Truncation on line 1699");
                t.truncate(STRLEN - 1);
            }
            t
        };
        m.transform_fname = t.clone();
        if !utils::file_exists(dir, &text(&t)) {
            t = format!("{fname}/../transforms/talairach.xfm").into_bytes();
            fits(g, "corRead", &text(&t));
            if !utils::file_exists(dir, &text(&t)) {
                let mut s = b"can't find talairach file '".to_vec();
                s.extend_from_slice(&t);
                s.extend_from_slice(b"'\n");
                let _ = out.write_all(&s);
            }
        }
        if utils::file_exists(dir, &text(&t)) {
            match minc::input_transform_file(g, out, err, dir, &t) {
                Some(tr) => {
                    m.linear_transform = Some(mri::get_linear_transform_ptr(err, &tr)?);
                    m.inverse_linear_transform =
                        Some(mri::get_inverse_linear_transform_ptr(err, &tr)?);
                    m.transform = Some(tr);
                    m.free_transform = 1;
                    m.transform_fname = t.clone();
                    if verbose_on(g) {
                        let mut s = b"INFO: loaded talairach xform : ".to_vec();
                        s.extend_from_slice(&t);
                        s.push(b'\n');
                        let _ = err.write_all(&s);
                    }
                }
                None => {
                    bad_transform = true;
                    g.errno = 0;
                    let mut s = b"error loading transform from ".to_vec();
                    s.extend_from_slice(&t);
                    log::error_printf_bytes(g, err, ERROR_BAD_FILE, &s);
                    m.linear_transform = None;
                    m.inverse_linear_transform = None;
                    m.free_transform = 1;
                    m.transform_fname.clear();
                }
            }
        }
    }
    if !read_volume {
        return Ok(Some(m));
    }
    let w = m.vg.width.max(0) as usize;
    let mut i = m.imnr0;
    while i <= imnr1 {
        let name = format!("{base}COR-{i:03}");
        let mut fp = match CFile::open(dir, &name, "r") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                freed(g, bad_transform);
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("corRead(): can't open file {name}"),
                ));
            }
        };
        let k = (i - m.imnr0) as u64;
        for j in 0..m.vg.height {
            let o = (k * m.vox_per_slice + j as u64 * m.vox_per_row) as usize;
            let mut row = vec![0u8; w];
            let n = fp.fread(&mut row, 1, w);
            if let Some(c) = m.chunk.as_mut() {
                c[o..o + n].copy_from_slice(&row[..n]);
            }
            if n < w {
                freed(g, bad_transform);
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("corRead(): error reading from file {name}"),
                ));
            }
        }
        if i == i32::MAX {
            break;
        }
        i += 1;
    }
    Ok(Some(m))
}

/// `corWrite(mri, fname)`: the volume as a COR folder `fname` (made when missing): `COR-.info`
/// and the slices `COR-<nnn>` from `imnr0` to `imnr1`, each row's `width` bytes. A volume without
/// voxels: `corWrite(): mri structure to be written contains no voxel data` (`ERROR_BADPARM`).
/// Warnings on standard output for values other than COR's (`imnr0` 1, `imnr1` 256, bytes,
/// 256 x 256, 1 mm). A folder that cannot be made: `ERROR: creating directory <name>/` and the
/// system's message, 1. See `specs/freesurfer-io.md` §104.
pub fn cor_write(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    m: &Mri,
    fname: &str,
) -> Result<i32, Fatal> {
    let Some(chunk) = m.chunk.as_ref() else {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADPARM,
            "corWrite(): mri structure to be written contains no voxel data",
        ));
    };
    if m.imnr0 != 1 {
        let _ = writeln!(
            out,
            "non-standard value for imnr0 ({}, usually 1) in volume structure",
            m.imnr0
        );
    }
    if m.imnr1 != 256 {
        let _ = writeln!(
            out,
            "non-standard value for imnr1 ({}, usually 256) in volume structure",
            m.imnr1
        );
    }
    if m.mtype != MRI_UCHAR {
        let _ = writeln!(
            out,
            "non-standard value for type ({}, usually {MRI_UCHAR}) in volume structure",
            m.mtype
        );
    }
    if m.vg.width != 256 {
        let _ = writeln!(
            out,
            "non-standard value for width ({}, usually 256) in volume structure",
            m.vg.width
        );
    }
    if m.vg.height != 256 {
        let _ = writeln!(
            out,
            "non-standard value for height ({}, usually 256) in volume structure",
            m.vg.height
        );
    }
    if m.thick != 1.0 {
        let _ = writeln!(
            out,
            "non-standard value for thick ({}, usually 1) in volume structure",
            cfmt::g(f64::from(m.thick), 6)
        );
    }
    if m.ps != 1.0 {
        let _ = writeln!(
            out,
            "non-standard value for ps ({}, usually 1) in volume structure",
            cfmt::g(f64::from(m.ps), 6)
        );
    }
    let (d, nf) = (m.vg.depth, m.nframes);
    let count = i64::from(m.imnr1) - i64::from(m.imnr0) + 1;
    if m.mtype != MRI_UCHAR {
        g.notices.add(
            &FS_659,
            if g.notices.reproduce() {
                "corWrite of a volume that is not bytes: each row's first bytes written, as in FreeSurfer"
            } else {
                "corWrite of a volume that is not bytes: refused (FreeSurfer writes each row's first bytes)"
            },
        );
        if g.notices.fix() {
            return Ok(fail_code(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "corWrite(): COR slices hold bytes; the volume's type is {}",
                    m.mtype
                ),
            ));
        }
    }
    if count > i64::from(d.max(0)) {
        g.notices.add(
            &FS_660,
            if g.notices.reproduce() {
                "corWrite: imnr0 to imnr1 asks for more slices than the volume has: the next frames' slices, as in FreeSurfer"
            } else {
                "corWrite: imnr0 to imnr1 asks for more slices than the volume has: refused (FreeSurfer writes other frames' slices or crashes)"
            },
        );
        if g.notices.fix() {
            return Ok(fail_code(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "corWrite(): imnr0 {} to imnr1 {} asks for {count} slices; the volume has {d}",
                    m.imnr0, m.imnr1
                ),
            ));
        }
    }
    fits(g, "corWrite", fname);
    let mut use_name = fname.to_owned();
    if !use_name.ends_with('/') {
        use_name.push('/');
    }
    if let Err(e) = std::fs::create_dir(utils::resolve(dir, &use_name)) {
        let code = log::errno_of(&e).unwrap_or(5);
        g.errno = code;
        if code != EEXIST {
            let _ = writeln!(out, "ERROR: creating directory {use_name}");
            let _ = writeln!(err, "{}", log::strerror(code));
            return Ok(1);
        }
    }
    match stat_dir(g, dir, fname) {
        Err(()) => {
            return Ok(fail_code(
                g,
                err,
                ERROR_BADFILE,
                &format!("corWrite(): can't stat {fname}"),
            ));
        }
        Ok(false) => {
            return Ok(fail_code(
                g,
                err,
                ERROR_BADFILE,
                &format!("corWrite(): {fname} isn't a directory"),
            ));
        }
        Ok(true) => {}
    }
    let info = format!("{use_name}COR-.info");
    let Ok(mut fp) = CFile::open(dir, &info, "w") else {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADFILE,
            &format!("corWrite(): can't open file {info} for writing"),
        ));
    };
    let gk = |v: f32| cfmt::g(sse::div64(widen(v), 1000.0), 6);
    let f6 = |v: f32| cfmt::f(f64::from(v), 6);
    let mut s = format!(
        "imnr0 {}\nimnr1 {}\nptype 2\nx {}\ny {}\n",
        m.imnr0, m.imnr1, m.vg.width, m.vg.height
    );
    s.push_str(&format!("fov {}\n", gk(m.fov)));
    s.push_str(&format!("thick {}\n", gk(m.vg.zsize)));
    s.push_str(&format!("psiz {}\n", gk(m.vg.xsize)));
    s.push_str(&format!("locatn {}\n", cfmt::g(0.0, 6)));
    for (name, v) in [
        ("strtx", m.xstart),
        ("endx", m.xend),
        ("strty", m.ystart),
        ("endy", m.yend),
        ("strtz", m.zstart),
        ("endz", m.zend),
    ] {
        s.push_str(&format!("{name} {}\n", gk(v)));
    }
    s.push_str(&format!("tr {}\nte {}\nti {}\n", f6(m.tr), f6(m.te), f6(m.ti)));
    s.push_str(&format!(
        "flip angle {}\n",
        cfmt::f(sse::div64(sse::mul64(360.0, m.flip_angle), TWO_PI), 6)
    ));
    s.push_str(&format!("ras_good_flag {}\n", m.vg.ras_good_flag));
    let v = &m.vg;
    for (name, a, b, c) in [
        ("x_ras", v.x_r, v.x_a, v.x_s),
        ("y_ras", v.y_r, v.y_a, v.y_s),
        ("z_ras", v.z_r, v.z_a, v.z_s),
        ("c_ras", v.c_r, v.c_a, v.c_s),
    ] {
        s.push_str(&format!("{name} {} {} {}\n", f6(a), f6(b), f6(c)));
    }
    let mut b = s.into_bytes();
    let tf = cstr(&m.transform_fname);
    if !tf.is_empty() {
        b.extend_from_slice(b"xform ");
        b.extend_from_slice(tf);
        b.push(b'\n');
    }
    fp.put_bytes(&b);
    fp.close();
    let w = m.vg.width.max(0) as usize;
    let bpv = m.bytes_per_vox;
    let slices = i64::from(d.max(0)) * i64::from(nf.max(0));
    // FreeSurfer's row pointers: the types initSlices does not step all point at the first
    // voxel (FS-166; reached only with `reproduce`, FS-659 refuses such volumes by default)
    let stepped = matches!(
        m.mtype,
        MRI_UCHAR | MRI_SHORT | MRI_USHRT | MRI_RGB | MRI_INT | MRI_LONG | MRI_FLOAT | MRI_FLOAT_COMPLEX
    );
    if !stepped {
        g.notices.add(
            &FS_166,
            "corWrite of a volume whose rows all point at its first voxel, as in FreeSurfer",
        );
    }
    let mut i = i64::from(m.imnr0);
    while i <= i64::from(m.imnr1) {
        let name = format!("{use_name}COR-{i:03}");
        let Ok(mut fp) = CFile::open(dir, &name, "w") else {
            return Ok(fail_code(
                g,
                err,
                ERROR_BADFILE,
                &format!("corWrite(): can't open file {name} for writing"),
            ));
        };
        let k = i - i64::from(m.imnr0);
        if k >= slices {
            // past FreeSurfer's slice pointers (fix mode refused this above)
            g.notices.add(
                &FS_660,
                "corWrite: a slice past the volume's voxels (FreeSurfer crashes)",
            );
            fp.close();
            return Err(Fatal::Exit(139));
        }
        for j in 0..m.vg.height {
            let o = if stepped {
                (k as u64 * m.vox_per_slice + j as u64 * m.vox_per_row) as usize * bpv
            } else {
                0
            };
            if fp.fwrite(&chunk[o..o + w], 1, w) < w {
                fp.close();
                return Ok(fail_code(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("corWrite(): error writing to file {name} "),
                ));
            }
        }
        fp.close();
        i += 1;
    }
    Ok(NO_ERROR)
}
