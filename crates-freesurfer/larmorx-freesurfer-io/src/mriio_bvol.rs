// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! bshort and bfloat volumes (`utils/mriio.cpp` 2766-4013, item 3.1-27): `bvolumeRead`
//! ([`bvolume_read`]: one file per slice, `<stem>_<nnn>.bshort` or `.bfloat`, each with a text
//! header `<stem>_<nnn>.hdr` of rows, columns, frames and byte order, and the geometry in
//! `<stem>.bhdr` or else a coronal default) with `get_b_info`, `decompose_b_fname` and
//! `read_bhdr`; the writer `bvolumeWrite` ([`bvolume_write`]: the slices and their headers, then a
//! subject's `register.dat` and `analyse.dat`, or the geometry file) with `write_bhdr` and, for a
//! volume of another type, the unscaled path of fs.volume's `MRIchangeType` (copied here: fs.io
//! cannot call fs.volume). Wired into [`crate::mriio::mri_read`] (`BSHORT_FILE`, `BFLOAT_FILE`,
//! `BHDR`) and [`crate::mriio::mri_write_type`].
//!
//! The arithmetic follows the C expressions in `float` and `double` as written (GCC 8.4.0,
//! SSE2): `sqrt` of a `float` sum is correctly rounded either way.
//!
//! Defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-io.md`, Part 19):
//! FS-661 (a `.bhdr` that cannot be read is read for ever), FS-662 (a slice header without its
//! numbers: memory contents), FS-663 (the `register.dat` overwrite warning), FS-664 (the geometry
//! written to `<stem>.hdr`), FS-665 (a subject name without `SUBJECTS_DIR`: a crash), FS-667 (the
//! `register.dat`'s direction matrices), FS-668 (a `.bhdr`'s last line without a newline),
//! FS-669 (a `.bhdr` line that starts with a NUL byte), FS-670 (a bvolume in the root folder),
//! FS-58 (writing a volume without voxels), FS-249 (names past FreeSurfer's buffers).

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::{
    FS_58, FS_249, FS_661, FS_662, FS_663, FS_664, FS_665, FS_667, FS_668, FS_669, FS_670, FS_671,
};
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::diag::{DIAG_SHOW, verbose_on};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NO_MEMORY, ERROR_UNSUPPORTED, Fatal, NO_ERROR,
};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{matrix_determinant, matrix_inverse};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::{sse, utils};
use larmorx_freesurfer_mri::info::mri_limits;
use larmorx_freesurfer_mri::mri::{
    self, MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri, MriFrame,
};
use larmorx_freesurfer_mri::vol_geom;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::bfileio;
use crate::cfile::CFile;
use crate::mriio::{MRI_CORONAL, mri_read_info, set_direction_cosine};
use crate::mriio_nifti::{narrow, widen};

/// `STRLEN`: FreeSurfer's name and line buffers.
const STRLEN: usize = 1024;

/// `sizeof(MRI_FRAME)` on x86-64: what `calloc` of the frames asks for per frame.
const MRI_FRAME_BYTES: u64 = crate::fstagsio::SIZEOF_MRI_FRAME as u64;

/// The size above which `calloc` fails (the x86-64 user address space).
const CALLOC_LIMIT: u64 = 1 << 47;

/// The extension and voxel size of a bvolume's type.
fn ext_of(mtype: i32) -> Option<(&'static str, usize)> {
    match mtype {
        MRI_SHORT => Some(("bshort", 2)),
        MRI_FLOAT => Some(("bfloat", 4)),
        _ => None,
    }
}

/// A C string's bytes: up to its first NUL.
fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
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

/// `snprintf(buf, STRLEN, ...)` of `s`: the text cut to 1,023 bytes, and the caller's message on
/// standard error when it was cut (`std::cerr << __FUNCTION__ << ...`).
fn cut(err: &mut dyn Write, s: String, cut_message: &str) -> String {
    if s.len() < STRLEN {
        return s;
    }
    let _ = writeln!(err, "{cut_message}");
    let mut k = STRLEN - 1;
    while !s.is_char_boundary(k) {
        k -= 1;
    }
    s[..k].to_owned()
}

/// FS-249's check of a name copied with `strcpy` into a `STRLEN` buffer.
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

/// `decompose_b_fname(fname, dir, stem)`: the folder (`.` without a `/`) and the stem of a bshort
/// or bfloat name: the name after its last `/`, its extension `.bshort` or `.bfloat` removed,
/// then cut at its last `_` when that is its last character or the fourth from its end (the
/// slice number). An empty stem: `decompose_b_fname(): bad bshort/bfloat file specifier "<f>"`
/// (`ERROR_BADPARM`) and `Err`.
pub fn decompose_b_fname(
    g: &mut Globals,
    err: &mut dyn Write,
    fname: &str,
) -> Result<(String, String), i32> {
    fits(g, "decompose_b_fname", fname);
    let mut copy = fname.as_bytes().to_vec();
    let slash = copy.iter().rposition(|&c| c == b'/');
    let dot = copy.iter().rposition(|&c| c == b'.');
    let underscore = copy.iter().rposition(|&c| c == b'_');
    let (dir, start) = match slash {
        None => (".".to_owned(), 0),
        Some(s) => {
            copy[s] = 0;
            (text(&copy[..s]), s + 1)
        }
    };
    if copy.get(start).is_none_or(|&c| c == 0) {
        return Err(fail_code(
            g,
            err,
            ERROR_BADPARM,
            &format!("decompose_b_fname(): bad bshort/bfloat file specifier \"{fname}\""),
        ));
    }
    if let Some(d) = dot {
        let e = cstr(&copy[d..]);
        if e == b".bshort" || e == b".bfloat" {
            copy[d] = 0;
        }
    }
    let len = cstr(&copy[start..]).len() as isize;
    if let Some(u) = underscore {
        let pos = u as isize - start as isize;
        if pos == len - 1 || pos == len - 4 {
            copy[u] = 0;
        }
    }
    Ok((dir, text(cstr(&copy[start..]))))
}

/// `fscanf(fp, "%d %d %d %*d", &a, &b, &c)` (`which` 3) or `fscanf(fp, "%*d %*d %*d %d", &a)`
/// (`which` 1): the values assigned and the call's return value.
fn scan_hdr(g: &mut Globals, fp: &mut CFile, three: bool) -> (Vec<Option<i32>>, i32) {
    let mut errno = g.errno;
    let mut sc = Scanf::new(fp);
    let mut vals = Vec::new();
    for i in 0..4 {
        if i > 0 {
            sc.text(&mut errno, b" ");
        }
        let assign = if three { i < 3 } else { i == 3 };
        let v = sc.d(&mut errno, assign);
        if assign {
            vals.push(v);
        }
    }
    let n = sc.finish(&mut errno);
    g.errno = errno;
    (vals, n)
}

/// `%*s %<conv>` of a header line: the value, when the call assigns it.
fn scan_word_then<T>(
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

fn is_c_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// `read_bhdr(mri, fp)`: the geometry of a `.bhdr` file (`cols:`, `rows:`, `nslices:`,
/// `n_time_points:`, `slice_thick:`, `image_te:`, `image_tr:` in seconds, `image_ti:`,
/// `flip_angle:`, the top-left, top-right and bottom-right corners and the normal, each a name
/// and a value after optional blanks; other lines ignored); the stream is closed. The voxel
/// sizes and direction cosines come from the corners (a size of 0: 1 with the default axis), the
/// centre from the top-left corner and the volume's middle; `ras_good_flag` 1. FS-661, FS-668,
/// FS-669.
pub fn read_bhdr(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &mut Mri,
    mut fp: CFile,
) -> Result<i32, Fatal> {
    let (mut tlr, mut tla, mut tls) = (0f32, 0f32, 0f32);
    let (mut trr, mut tra, mut trs) = (0f32, 0f32, 0f32);
    let (mut brr, mut bra, mut brs) = (0f32, 0f32, 0f32);
    loop {
        let got = fp.fgets(STRLEN);
        if got.is_none() && fp.ferror() {
            log::error_printf(g, err, ERROR_BADFILE, "read_bhdr(): could not read file");
            // FreeSurfer reads again for ever (the end-of-file flag never comes): FS-661
            g.notices.add(
                &FS_661,
                "read_bhdr: a geometry file that cannot be read (FreeSurfer tries again for ever): read as empty",
            );
            break;
        }
        if fp.feof() {
            if got.as_ref().is_some_and(|l| !l.is_empty()) {
                if g.notices.reproduce() {
                    g.notices.add(
                        &FS_668,
                        "read_bhdr: the last line without a newline ignored, as in FreeSurfer",
                    );
                    break;
                }
                g.notices.add(
                    &FS_668,
                    "read_bhdr: the last line without a newline read (FreeSurfer ignores it)",
                );
            } else {
                break;
            }
        }
        let at_end = fp.feof();
        let Some(raw) = got else { break };
        let mut line = cstr(&raw).to_vec();
        if line.is_empty() {
            // strlen 0: FreeSurfer reads (and may write) the byte before its buffer
            g.notices.add(
                &FS_669,
                "read_bhdr: a line that starts with a NUL byte (FreeSurfer reads the byte before its buffer): ignored",
            );
        } else if line.last() == Some(&b'\n') {
            line.pop();
        }
        let start = line.iter().take_while(|&&c| is_c_space(c)).count();
        let l = &line[start..];
        if !l.is_empty() {
            let has = |p: &[u8]| l.starts_with(p);
            let f = |g: &mut Globals| scan_word_then(g, l, |sc, e| sc.f(e, true));
            let d = |g: &mut Globals| scan_word_then(g, l, |sc, e| sc.d(e, true));
            if has(b"cols: ") {
                if let Some(v) = d(g) {
                    m.vg.width = v;
                }
            } else if has(b"rows: ") {
                if let Some(v) = d(g) {
                    m.vg.height = v;
                }
            } else if has(b"nslices: ") {
                if let Some(v) = d(g) {
                    m.vg.depth = v;
                }
            } else if has(b"n_time_points: ") {
                if let Some(v) = d(g) {
                    m.nframes = v;
                }
            } else if has(b"slice_thick: ") {
                if let Some(v) = f(g) {
                    m.vg.zsize = v;
                }
            } else if has(b"image_te: ") {
                if let Some(v) = f(g) {
                    m.te = v;
                }
            } else if has(b"image_tr: ") {
                if let Some(v) = f(g) {
                    m.tr = v;
                }
                m.tr = narrow(sse::mul64(1000.0, widen(m.tr)));
            } else if has(b"image_ti: ") {
                if let Some(v) = f(g) {
                    m.ti = v;
                }
            } else if l.len() >= 10 && &l[..10] == b"flip_angle" {
                // strncmp(l, "flip_angle: ", 10): the name alone decides
                if let Some(v) = scan_word_then(g, l, |sc, e| sc.lf(e, true)) {
                    m.flip_angle = v;
                }
            } else {
                let slots: [(&[u8], &mut f32); 12] = [
                    (b"top_left_r: ", &mut tlr),
                    (b"top_left_a: ", &mut tla),
                    (b"top_left_s: ", &mut tls),
                    (b"top_right_r: ", &mut trr),
                    (b"top_right_a: ", &mut tra),
                    (b"top_right_s: ", &mut trs),
                    (b"bottom_right_r: ", &mut brr),
                    (b"bottom_right_a: ", &mut bra),
                    (b"bottom_right_s: ", &mut brs),
                    (b"normal_r: ", &mut m.vg.z_r),
                    (b"normal_a: ", &mut m.vg.z_a),
                    (b"normal_s: ", &mut m.vg.z_s),
                ];
                for (p, slot) in slots {
                    if has(p) {
                        if let Some(v) = f(g) {
                            *slot = v;
                        }
                        break;
                    }
                }
            }
        }
        if at_end {
            break;
        }
    }
    drop(fp);
    let w = m.vg.width as f32;
    let h = m.vg.height as f32;
    let len = |a: f32, b: f32, c: f32| {
        sse::sqrt(sse::add(
            sse::add(sse::mul(a, a), sse::mul(b, b)),
            sse::mul(c, c),
        ))
    };
    let (xr, xa, xs) = (
        sse::div(sse::sub(trr, tlr), w),
        sse::div(sse::sub(tra, tla), w),
        sse::div(sse::sub(trs, tls), w),
    );
    m.vg.xsize = len(xr, xa, xs);
    if m.vg.xsize != 0.0 {
        m.vg.x_r = sse::div(xr, m.vg.xsize);
        m.vg.x_a = sse::div(xa, m.vg.xsize);
        m.vg.x_s = sse::div(xs, m.vg.xsize);
    } else {
        m.vg.xsize = 1.0;
        (m.vg.x_r, m.vg.x_a, m.vg.x_s) = (-1.0, 0.0, 0.0);
    }
    let (yr, ya, ys) = (
        sse::div(sse::sub(brr, trr), h),
        sse::div(sse::sub(bra, tra), h),
        sse::div(sse::sub(brs, trs), h),
    );
    m.vg.ysize = len(yr, ya, ys);
    if m.vg.ysize != 0.0 {
        m.vg.y_r = sse::div(yr, m.vg.ysize);
        m.vg.y_a = sse::div(ya, m.vg.ysize);
        m.vg.y_s = sse::div(ys, m.vg.ysize);
    } else {
        m.vg.ysize = 1.0;
        (m.vg.y_r, m.vg.y_a, m.vg.y_s) = (0.0, 0.0, -1.0);
    }
    let mut t = vol_geom::xfm_crs2xyz(g, out, err, &m.vg, 0, false)?;
    t.set_relt(1, 4, tlr);
    t.set_relt(2, 4, tla);
    t.set_relt(3, 4, tls);
    let mut crs = col4(g, out, err)?;
    crs.set_relt(1, 1, (f64::from(m.vg.width) / 2.0) as f32);
    crs.set_relt(2, 1, (f64::from(m.vg.height) / 2.0) as f32);
    crs.set_relt(3, 1, (f64::from(m.vg.depth) / 2.0) as f32);
    crs.set_relt(4, 1, 1.0);
    let ras = matrix_multiply(g, out, err, Some(&t), Some(&crs), None)?.unwrap_new();
    m.vg.c_r = ras.relt(1, 1);
    m.vg.c_a = ras.relt(2, 1);
    m.vg.c_s = ras.relt(3, 1);
    m.vg.ras_good_flag = 1;
    m.thick = m.vg.zsize;
    m.ps = m.vg.xsize;
    Ok(NO_ERROR)
}

/// A 4 x 1 real matrix of zeros.
fn col4(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write) -> Result<Matrix, Fatal> {
    Ok(matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a real matrix"))
}

/// `write_bhdr(mri, fp)`: the geometry file's text: the extents, frames and slice thickness, the
/// top-left corner (the first voxel's centre), the top-right and bottom-right corners (the
/// centres one past the last column, and past the last row too: the edge-to-edge field of view
/// from the top-left), the normal (the slice axis divided by its length), TE, TR in seconds, TI,
/// the flip angle; every number `%f`.
pub fn write_bhdr(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Mri,
) -> Result<Vec<u8>, Fatal> {
    let t = vol_geom::xfm_crs2xyz(g, out, err, &m.vg, 0, false)?;
    let (tlr, tla, tls) = (t.relt(1, 4), t.relt(2, 4), t.relt(3, 4));
    let mut crs = col4(g, out, err)?;
    let mut ras = col4(g, out, err)?;
    crs.set_relt(4, 1, 1.0);
    crs.set_relt(1, 1, m.vg.width as f32);
    crs.set_relt(2, 1, 0.0);
    crs.set_relt(3, 1, 0.0);
    matrix_multiply(g, out, err, Some(&t), Some(&crs), Some(&mut ras))?;
    let (trr, tra, trs) = (ras.relt(1, 1), ras.relt(2, 1), ras.relt(3, 1));
    crs.set_relt(1, 1, m.vg.width as f32);
    crs.set_relt(2, 1, m.vg.height as f32);
    crs.set_relt(3, 1, 0.0);
    matrix_multiply(g, out, err, Some(&t), Some(&crs), Some(&mut ras))?;
    let (brr, bra, brs) = (ras.relt(1, 1), ras.relt(2, 1), ras.relt(3, 1));
    let v = &m.vg;
    let vl = sse::sqrt(sse::add(
        sse::add(sse::mul(v.z_r, v.z_r), sse::mul(v.z_a, v.z_a)),
        sse::mul(v.z_s, v.z_s),
    ));
    let (nr, na, ns) = (
        sse::div(v.z_r, vl),
        sse::div(v.z_a, vl),
        sse::div(v.z_s, vl),
    );
    let mut s = String::new();
    let fl = |s: &mut String, name: &str, x: f64| {
        s.push_str(&format!("{name}: {}\n", cfmt::f(x, 6)));
    };
    s.push_str(&format!("          cols: {}\n", v.width));
    s.push_str(&format!("          rows: {}\n", v.height));
    s.push_str(&format!("       nslices: {}\n", v.depth));
    s.push_str(&format!(" n_time_points: {}\n", m.nframes));
    fl(&mut s, "   slice_thick", f64::from(v.zsize));
    fl(&mut s, "    top_left_r", f64::from(tlr));
    fl(&mut s, "    top_left_a", f64::from(tla));
    fl(&mut s, "    top_left_s", f64::from(tls));
    fl(&mut s, "   top_right_r", f64::from(trr));
    fl(&mut s, "   top_right_a", f64::from(tra));
    fl(&mut s, "   top_right_s", f64::from(trs));
    fl(&mut s, "bottom_right_r", f64::from(brr));
    fl(&mut s, "bottom_right_a", f64::from(bra));
    fl(&mut s, "bottom_right_s", f64::from(brs));
    fl(&mut s, "      normal_r", f64::from(nr));
    fl(&mut s, "      normal_a", f64::from(na));
    fl(&mut s, "      normal_s", f64::from(ns));
    fl(&mut s, "      image_te", f64::from(m.te));
    fl(&mut s, "      image_tr", sse::div64(widen(m.tr), 1000.0));
    fl(&mut s, "      image_ti", f64::from(m.ti));
    fl(&mut s, "    flip_angle", m.flip_angle);
    Ok(s.into_bytes())
}

/// `calloc(n, sizeof(MRI_FRAME))` of a bvolume's frames: the frames, or `None` where `calloc`
/// fails (a negative count, or more than the address space).
fn frames_calloc(n: i32) -> Option<Vec<MriFrame>> {
    let bytes = (n as i64 as u64).checked_mul(MRI_FRAME_BYTES)?;
    if bytes > CALLOC_LIMIT {
        return None;
    }
    Some((0..n).map(|_| MriFrame::default()).collect())
}

/// `get_b_info(fname, read_volume, dir, stem, type)`: the header of a bvolume (allocated with
/// its voxels when `read_volume`), its folder and stem. With `<dir>/<stem>.bhdr`: its geometry
/// ([`read_bhdr`]) and the frames of `<stem>_000.hdr` (`cannot open <name>`; `bvolumeRead():
/// could not read file` when it lacks three numbers; frames that cannot be allocated end the
/// program: `get_b_info: could not allocate <n> frames`); otherwise the rows, columns and frames
/// of `<stem>_000.hdr` (`can't find file <name> (last resort);bailing out on read`, `get_b_info():
/// could not read file`), the slices counted while `<stem>_<nnn>.<ext>` exists (`can't find file
/// <name>; bailing out on read` without the first), 1 mm voxels and the coronal orientation
/// (`ras_good_flag` 0).
#[allow(clippy::too_many_arguments)]
fn get_b_info(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
    mtype: i32,
) -> Result<Option<(Mri, String, String)>, Fatal> {
    let Some((ext, _)) = ext_of(mtype) else {
        return Ok(fail0(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("internal error: get_b_info() passed type {mtype}"),
        )
        .map(|m| (m, String::new(), String::new())));
    };
    let Ok((mut directory, stem)) = decompose_b_fname(g, err, fname) else {
        return Ok(None);
    };
    if directory.is_empty() {
        g.notices.add(
            &FS_670,
            if g.notices.reproduce() {
                "a bvolume in the root folder read from the working folder, as in FreeSurfer"
            } else {
                "a bvolume in the root folder read from there (FreeSurfer reads it from the working folder)"
            },
        );
        directory = if g.notices.reproduce() { "." } else { "" }.to_owned();
    }
    let mut m = mri::mri_alloc_header(g, out, err, 1, 1, 1, mtype, 1)?;
    let bhdr_name = cut(
        err,
        format!("{directory}/{stem}.bhdr"),
        "get_b_info: bhdr_name truncated",
    );
    let mut nslices = 0i32;
    match CFile::open(dir, &bhdr_name, "r") {
        Ok(fp) => {
            read_bhdr(g, out, err, &mut m, fp)?;
            let name = cut(
                err,
                format!("{directory}/{stem}_000.hdr"),
                "get_b_info: fname truncated",
            );
            let mut fp = match CFile::open(dir, &name, "r") {
                Ok(f) => f,
                Err(e) => {
                    g.errno = e;
                    return Ok(fail0(g, err, ERROR_BADFILE, &format!("cannot open {name}"))
                        .map(|m| (m, String::new(), String::new())));
                }
            };
            let (v, n) = scan_hdr(g, &mut fp, true);
            if n != 3 {
                log::error_printf(g, err, ERROR_BADFILE, "bvolumeRead(): could not read file");
            }
            let nt = match v[2] {
                Some(nt) => nt,
                None => {
                    g.notices.add(
                        &FS_662,
                        "get_b_info: a slice header without its frame count: 1 frame (FreeSurfer: memory contents)",
                    );
                    1
                }
            };
            m.nframes = nt;
            if frames_calloc(nt).is_none() {
                log::error_exit(
                    g,
                    err,
                    ERROR_NO_MEMORY,
                    &format!("get_b_info: could not allocate {nt} frames"),
                )?;
            }
            drop(fp);
            m.vg.fname.strcpy_bytes(fname.as_bytes());
        }
        Err(e) => {
            g.errno = e;
            if g.gdiag & DIAG_SHOW != 0 && verbose_on(g) {
                let _ = write!(
                    err,
                    "-----------------------------------------------------------------\nCould not find the direction cosine information.\nWill use the CORONAL orientation.\nIf not suitable, please provide the information in {bhdr_name} file\n-----------------------------------------------------------------\n"
                );
            }
            let name = cut(
                err,
                format!("{directory}/{stem}_000.hdr"),
                "get_b_info: Truncation on line 3474",
            );
            let mut fp = match CFile::open(dir, &name, "r") {
                Ok(f) => f,
                Err(e) => {
                    g.errno = e;
                    return Ok(fail0(
                        g,
                        err,
                        ERROR_BADFILE,
                        &format!("can't find file {name} (last resort);bailing out on read"),
                    )
                    .map(|m| (m, String::new(), String::new())));
                }
            };
            let (v, n) = scan_hdr(g, &mut fp, true);
            if n != 3 {
                log::error_printf(g, err, ERROR_BADFILE, "get_b_info(): could not read file");
            }
            drop(fp);
            let memory = |g: &mut Globals, what: &str| {
                g.notices.add(
                    &FS_662,
                    format!(
                        "get_b_info: a slice header without its {what}: 1 (FreeSurfer: memory contents)"
                    ),
                );
                1
            };
            let ny = v[0].unwrap_or_else(|| memory(g, "rows"));
            let nx = v[1].unwrap_or_else(|| memory(g, "columns"));
            let nt = v[2].unwrap_or_else(|| memory(g, "frame count"));
            let first = cut(
                err,
                format!("{directory}/{stem}_000.{ext}"),
                "get_b_info: Truncation on line 3493",
            );
            if !utils::file_exists(dir, &first) {
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("can't find file {first}; bailing out on read"),
                )
                .map(|m| (m, String::new(), String::new())));
            }
            let mut name = first;
            while utils::file_exists(dir, &name) {
                name = cut(
                    err,
                    format!("{directory}/{stem}_{nslices:03}.{ext}"),
                    "get_b_info: Truncation on line 3507",
                );
                nslices += 1;
            }
            nslices -= 1;
            m.vg.width = nx;
            m.vg.height = ny;
            m.vg.depth = nslices;
            m.nframes = nt;
            m.imnr0 = 1;
            m.imnr1 = nslices;
            m.thick = 1.0;
            m.ps = 1.0;
            (m.vg.xsize, m.vg.ysize, m.vg.zsize) = (1.0, 1.0, 1.0);
            set_direction_cosine(g, err, &mut m.vg, MRI_CORONAL);
            m.vg.ras_good_flag = 0;
            m.vg.fname.strcpy_bytes(fname.as_bytes());
        }
    }
    m.imnr0 = 1;
    m.imnr1 = nslices;
    m.xend = half(sse::mul(m.vg.width as f32, m.vg.xsize));
    m.xstart = -m.xend;
    m.yend = half(sse::mul(m.vg.height as f32, m.vg.ysize));
    m.ystart = -m.yend;
    m.zend = half(sse::mul(m.vg.depth as f32, m.vg.zsize));
    m.zstart = -m.zend;
    let (dx, dy) = (sse::sub(m.xend, m.xstart), sse::sub(m.yend, m.ystart));
    m.fov = if dx > dy { dx } else { dy };
    // calloc(nframes): a count calloc refuses leaves no frames (nothing reads them)
    m.frames = frames_calloc(m.nframes).unwrap_or_default();
    if read_volume {
        let mut m2 = mri::mri_alloc_sequence(
            g,
            out,
            err,
            m.vg.width,
            m.vg.height,
            m.vg.depth,
            m.mtype,
            m.nframes,
        )?;
        mri::mri_copy_header(g, out, err, &m, Some(&mut m2))?;
        m = m2;
    }
    Ok(Some((m, directory, stem)))
}

/// `bvolumeRead(fname, read_volume, type)`: a bshort (`MRI_SHORT`) or bfloat (`MRI_FLOAT`)
/// volume, header ([`get_b_info`]) and slices; another type: `ERROR: bvolumeRead: type (<t>) is
/// not short or float` on standard error and `None`. The slices' byte order is the fourth number
/// of `<stem>_000.hdr` (0 big-endian, else little-endian; a header without it: `bvolumeRead():
/// could not read file` and FS-662); each slice file holds its rows frame after frame
/// (`bvolumeRead(): error opening file <name>`, `... error reading from file <name>`). Then
/// `INFO: bvolumeRead: min = <g>, max = <g>` on standard output; `imnr0` 1, `imnr1` the depth,
/// `thick` the slice size.
#[allow(clippy::too_many_arguments)]
pub fn bvolume_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
    mtype: i32,
) -> Result<Option<Mri>, Fatal> {
    let Some((ext, size)) = ext_of(mtype) else {
        let _ = writeln!(
            err,
            "ERROR: bvolumeRead: type ({mtype}) is not short or float"
        );
        return Ok(None);
    };
    let Some((mut m, directory, stem)) = get_b_info(g, out, err, dir, fname, read_volume, mtype)?
    else {
        return Ok(None);
    };
    if !read_volume {
        return Ok(Some(m));
    }
    let hdr = format!("{directory}/{stem}_000.hdr");
    let swap = match CFile::open(dir, &hdr, "r") {
        Err(e) => {
            g.errno = e;
            let _ = writeln!(
                err,
                "ERROR: can't open file {hdr}; assuming big-endian bvolume"
            );
            false
        }
        Ok(mut fp) => {
            let (v, n) = scan_hdr(g, &mut fp, false);
            if n != 1 {
                log::error_printf(g, err, ERROR_BADFILE, "bvolumeRead(): could not read file");
            }
            let flag = v[0].unwrap_or_else(|| {
                g.notices.add(
                    &FS_662,
                    "bvolumeRead: a slice header without its byte order: big-endian (FreeSurfer: memory contents)",
                );
                0
            });
            flag == 0
        }
    };
    let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
    let row_bytes = w.max(0) as usize * size;
    for slice in 0..d {
        let name = format!("{directory}/{stem}_{slice:03}.{ext}");
        let mut fp = match CFile::open(dir, &name, "r") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("bvolumeRead(): error opening file {name}"),
                ));
            }
        };
        for frame in 0..m.nframes {
            let k = slice + d * frame;
            for row in 0..h {
                let o = (k as u64 * m.vox_per_slice + row as u64 * m.vox_per_row) as usize * size;
                let mut buf = vec![0u8; row_bytes];
                let nread = fp.fread(&mut buf, size, w.max(0) as usize);
                if let Some(c) = m.chunk.as_mut() {
                    c[o..o + nread * size].copy_from_slice(&buf[..nread * size]);
                }
                if nread != w.max(0) as usize {
                    return Ok(fail0(
                        g,
                        err,
                        ERROR_BADFILE,
                        &format!("bvolumeRead(): error reading from file {name}"),
                    ));
                }
                if swap && let Some(c) = m.chunk.as_mut() {
                    let r = &mut c[o..o + row_bytes];
                    if mtype == MRI_SHORT {
                        for p in r.as_chunks_mut::<2>().0 {
                            p.swap(0, 1);
                        }
                    } else {
                        bfileio::byteswapbuffloat(g, r, row_bytes as i64);
                    }
                }
            }
        }
    }
    let (mut min, mut max) = (0f32, 0f32);
    mri_limits(g, err, &m, &mut min, &mut max)?;
    let _ = writeln!(
        out,
        "INFO: bvolumeRead: min = {}, max = {}",
        cfmt::g(f64::from(min), 6),
        cfmt::g(f64::from(max), 6)
    );
    m.imnr0 = 1;
    m.imnr1 = m.vg.depth;
    m.thick = m.vg.zsize;
    Ok(Some(m))
}

/// `MRIchangeType(src, type, min, max, 1)` as `bvolumeWrite` calls it (to `MRI_SHORT` or
/// `MRI_FLOAT`): a new volume of `type` with `src`'s header and its values unscaled
/// (`MRIgetVoxVal`, `MRIsetVoxVal`), after, for the pairs that may not fit, `MRIlimits` and, when
/// the range does not fit, `******* WARNING - forcing scaling of values to prevent cropping of
/// input [<min>, <max>]` and `***** DISABLING forcing and not doing scaling ********` on standard
/// output.
fn change_type_unscaled(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dest_type: i32,
) -> Result<Mri, Fatal> {
    let st = src.mtype;
    let plain = (st == MRI_UCHAR
        && matches!(dest_type, MRI_SHORT | MRI_INT | MRI_LONG | MRI_FLOAT))
        || (matches!(st, MRI_SHORT | MRI_USHRT)
            && matches!(dest_type, MRI_INT | MRI_LONG | MRI_FLOAT))
        || (st == MRI_LONG && matches!(dest_type, MRI_INT | MRI_FLOAT))
        || (st == MRI_INT && matches!(dest_type, MRI_LONG | MRI_FLOAT));
    if !plain {
        let (mut lo, mut hi) = (0f32, 0f32);
        mri_limits(g, err, src, &mut lo, &mut hi)?;
        let fit = match dest_type {
            MRI_UCHAR => lo >= 0.0 && hi <= 255.0,
            MRI_SHORT => lo >= -32768.0 && hi <= 32767.0,
            MRI_INT => f64::from(lo) >= f64::from(i32::MIN) && f64::from(hi) <= f64::from(i32::MAX),
            MRI_LONG => lo >= i64::MIN as f32 && hi <= i64::MAX as f32,
            _ => false,
        };
        if !fit {
            let _ = writeln!(
                out,
                "******* WARNING - forcing scaling of values to prevent cropping of input [{}, {}]",
                cfmt::pad(&cfmt::f(f64::from(lo), 0), 2),
                cfmt::pad(&cfmt::f(f64::from(hi), 0), 2)
            );
            let _ = writeln!(
                out,
                "***** DISABLING forcing and not doing scaling ********"
            );
        }
    }
    let mut dest = mri::mri_alloc_sequence(
        g,
        out,
        err,
        src.vg.width,
        src.vg.height,
        src.vg.depth,
        dest_type,
        src.nframes,
    )?;
    mri::mri_copy_header(g, out, err, src, Some(&mut dest))?;
    dest.mtype = dest_type;
    for f in 0..src.nframes {
        for k in 0..src.vg.depth {
            for j in 0..src.vg.height {
                for i in 0..src.vg.width {
                    let v = mri_get_vox_val(g, err, src, i, j, k, f, 0)?;
                    mri_set_vox_val(g, err, &mut dest, i, j, k, f, v, 0)?;
                }
            }
        }
    }
    Ok(dest)
}

/// A 4 x 4 real matrix of the sixteen values, row by row (`stuff_four_by_four`).
fn four(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    v: [f32; 16],
) -> Result<Matrix, Fatal> {
    let mut m = matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)?.expect("a real matrix");
    for r in 0..4 {
        for c in 0..4 {
            m.set_relt(r as i32 + 1, c as i32 + 1, v[r * 4 + c]);
        }
    }
    Ok(m)
}

/// `(a) / 2.0` of a `float` expression, back to `float`.
fn half(a: f32) -> f32 {
    narrow(sse::div64(widen(a), 2.0))
}

/// The bvolume writer's stem of `fname`: the name without `_<nnn>.bshort`, `_<nnn>.bfloat` or a
/// final `_<nnn>` (the `_` four from the end).
fn write_stem(fname: &str) -> String {
    let b = fname.as_bytes();
    let l = b.len();
    let mut stem = b.to_vec();
    if l > 11 {
        let c1 = l - 11;
        if b[c1] == b'_' && (&b[c1 + 4..] == b".bshort" || &b[c1 + 4..] == b".bfloat") {
            stem.truncate(c1);
        }
    }
    // the second test (`_` seven from the end, then `.bshort`) can never match: 3 bytes left
    if l > 4 && b[l - 4] == b'_' {
        stem.truncate(l - 4);
    }
    text(&stem)
}

/// The subject's `register.dat` matrix and the files `analyse.dat` and `register.dat`, when the
/// volume names a subject whose `mri/T1` or `mri/orig` volume can be read; `Ok(None)` when the
/// geometry file is to be written instead.
#[allow(clippy::too_many_arguments)]
fn subject_files(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    mri: &Mri,
    sn: &str,
    stem: &str,
    output_dir: &str,
) -> Result<Option<i32>, Fatal> {
    let Some(subjects_dir) = env
        .get("SUBJECTS_DIR")
        .map(|s| s.to_string_lossy().into_owned())
    else {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            "bvolumeWrite(): environment variable SUBJECTS_DIR unset",
        );
        return Ok(None);
    };
    let subject_dir = format!("{subjects_dir}/{sn}");
    let mut info = match std::fs::metadata(utils::resolve(dir, &subject_dir)) {
        Err(e) => {
            g.errno = log::errno_of(&e).unwrap_or(2);
            let _ = writeln!(err, "can't stat {subject_dir}; writing to bhdr instead");
            return Ok(None);
        }
        Ok(md) if !md.is_dir() => {
            let _ = writeln!(
                err,
                "{subject_dir} is not a directory; writing to bhdr instead"
            );
            return Ok(None);
        }
        Ok(_) => {
            let t1 = format!("{subject_dir}/mri/T1");
            match mri_read_info(g, out, err, env, dir, &t1)? {
                Some(m) => m,
                None => {
                    let orig = format!("{subject_dir}/mri/orig");
                    match mri_read_info(g, out, err, env, dir, &orig)? {
                        Some(m) => m,
                        None => {
                            let _ = writeln!(
                                err,
                                "can't read the subject's orig or T1 volumes; writing to bhdr instead"
                            );
                            return Ok(None);
                        }
                    }
                }
            }
        }
    };
    if info.vg.ras_good_flag == 0 {
        set_direction_cosine(g, err, &mut info.vg, MRI_CORONAL);
    }
    let s = &info.vg;
    let f = &mri.vg;
    // FS-667: the direction matrices' second and third rows repeat the R components (kept)
    g.notices.add(
        &FS_667,
        "bvolumeWrite's register.dat: the direction matrices' A and S rows from the R components, as in FreeSurfer",
    );
    let dirs = |v: &vol_geom::VolGeom| -> [f32; 16] {
        [
            v.x_r, v.y_r, v.z_r, v.c_r, v.y_r, v.y_r, v.y_r, v.c_r, v.z_r, v.z_r, v.z_r, v.c_r,
            0.0, 0.0, 0.0, 1.0,
        ]
    };
    let centre = |v: &vol_geom::VolGeom| -> [f32; 16] {
        [
            1.0,
            0.0,
            0.0,
            ((f64::from(v.width) - 1.0) / 2.0) as f32,
            0.0,
            1.0,
            0.0,
            ((f64::from(v.height) - 1.0) / 2.0) as f32,
            0.0,
            0.0,
            1.0,
            ((f64::from(v.depth) - 1.0) / 2.0) as f32,
            0.0,
            0.0,
            0.0,
            1.0,
        ]
    };
    let tk = |v: &vol_geom::VolGeom| -> [f32; 16] {
        [
            -v.xsize,
            0.0,
            0.0,
            half(sse::mul(v.width as f32, f.xsize)),
            0.0,
            0.0,
            v.zsize,
            -half(sse::mul(v.depth as f32, f.zsize)),
            0.0,
            -v.ysize,
            0.0,
            half(sse::mul(v.height as f32, f.ysize)),
            0.0,
            0.0,
            0.0,
            1.0,
        ]
    };
    let ms = four(g, out, err, dirs(s))?;
    let mf = four(g, out, err, dirs(f))?;
    let bs = four(g, out, err, centre(s))?;
    let bf = four(g, out, err, centre(f))?;
    // the subject's sizes on the diagonal, its extents times the volume's sizes for the centre
    let cs = four(g, out, err, tk(s))?;
    let cf = four(g, out, err, tk(f))?;
    let mut bad = false;
    for (mat, which) in [
        (&ms, "structural"),
        (&bs, "structural"),
        (&cs, "structural"),
        (&mf, "functional"),
        (&bf, "functional"),
        (&cf, "functional"),
    ] {
        if matrix_determinant(g, err, mat) == 0.0 {
            g.errno = 0;
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!("bvolumeWrite(): bad determinant in matrix (check {which} volume)"),
            );
            bad = true;
        }
    }
    if bad {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            "bvolumeWrite(): one or more zero determinants; aborting register.dat write and writing bhdr instead",
        );
        return Ok(None);
    }
    let mut inv = Vec::new();
    for m in [&mf, &bf, &cs] {
        match matrix_inverse(g, out, err, Some(m), None)?.into_new() {
            Some(i) => inv.push(Some(i)),
            None => {
                g.errno = 0;
                log::error_printf(
                    g,
                    err,
                    ERROR_BADPARM,
                    "bvolumeWrite(): error inverting matrix",
                );
                inv.push(None);
            }
        }
    }
    if inv.iter().any(Option::is_none) {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            "bvolumeWrite(): one or more zero determinants; aborting register.dat write and writing bhdr instead",
        );
        return Ok(None);
    }
    let (iaf, ibf, ics) = (
        inv[0].take().unwrap(),
        inv[1].take().unwrap(),
        inv[2].take().unwrap(),
    );
    let mul = |g: &mut Globals,
               out: &mut dyn Write,
               err: &mut dyn Write,
               a: &Matrix,
               b: &Matrix|
     -> Result<Matrix, Fatal> {
        Ok(matrix_multiply(g, out, err, Some(a), Some(b), None)?.unwrap_new())
    };
    let r1 = mul(g, out, err, &bs, &ics)?;
    let r2 = mul(g, out, err, &ms, &r1)?;
    let r3 = mul(g, out, err, &iaf, &r2)?;
    let r4 = mul(g, out, err, &ibf, &r3)?;
    let r = mul(g, out, err, &cf, &r4)?;
    let t1_path = if mri.path_to_t1.is_empty() {
        ".".to_owned()
    } else {
        mri.path_to_t1.clone()
    };
    let analyse = cut(
        err,
        format!("{output_dir}analyse.dat"),
        "bvolumeWrite: analyse_fname truncated",
    );
    let register = cut(
        err,
        format!("{output_dir}register.dat"),
        "bvolumeWrite: register_fname truncated",
    );
    if utils::file_exists(dir, &analyse) {
        let _ = writeln!(err, "warning: overwriting file {analyse}");
    }
    let Ok(mut fp) = CFile::open(dir, &analyse, "w") else {
        return Ok(Some(fail_code(
            g,
            err,
            ERROR_BADFILE,
            &format!("bvolumeWrite(): couldn't open file {analyse} for writing"),
        )));
    };
    let mut a = format!("{t1_path}\n{stem}_%03d.bshort\n");
    a.push_str(&format!("{} {}\n", mri.vg.depth, mri.nframes));
    a.push_str(&format!("{} {}\n", mri.vg.width, mri.vg.height));
    fp.put_bytes(a.as_bytes());
    fp.close();
    // FS-663: FreeSurfer tests analyse.dat, just written, so it always warns
    let warn = if g.notices.reproduce() {
        utils::file_exists(dir, &analyse)
    } else {
        let exists = utils::file_exists(dir, &register);
        if exists != utils::file_exists(dir, &analyse) {
            g.notices.add(
                &FS_663,
                "bvolumeWrite: no 'overwriting register.dat' warning for a register.dat that does not exist (FreeSurfer tests analyse.dat)",
            );
        }
        exists
    };
    if warn {
        let _ = writeln!(err, "warning: overwriting file {register}");
    }
    let Ok(mut fp) = CFile::open(dir, &register, "w") else {
        return Ok(Some(fail_code(
            g,
            err,
            ERROR_BADFILE,
            &format!("bvolumeWrite(): couldn't open file {register} for writing"),
        )));
    };
    let gg = |x: f32| cfmt::g(f64::from(x), 6);
    let mut t = format!(
        "{sn}\n{}\n{}\n{}\n",
        gg(mri.vg.xsize),
        gg(mri.vg.zsize),
        cfmt::g(1.0, 6)
    );
    for row in 1..=4 {
        t.push_str(&format!(
            "{} {} {} {}\n",
            gg(r.relt(row, 1)),
            gg(r.relt(row, 2)),
            gg(r.relt(row, 3)),
            gg(r.relt(row, 4))
        ));
    }
    fp.put_bytes(t.as_bytes());
    fp.close();
    Ok(Some(NO_ERROR))
}

/// `bvolumeWrite(vol, fname, type)`: the volume as bshort (`MRI_SHORT`) or bfloat
/// (`MRI_FLOAT`) slices: another type `ERROR: bvolumeWrite: type (<t>) is not short or float` on
/// standard error and 1. A volume of another type is converted first, unscaled
/// ([`change_type_unscaled`] over every frame's slices as one volume). `INFO: bvolumeWrite: stem =
/// <stem>` on standard output; for each slice `<stem>_<nnn>.hdr` (`<rows> <columns> <frames>
/// <0 big-endian | 1>`) and `<stem>_<nnn>.<ext>` (its rows frame after frame, big-endian unless
/// `BFILE_LITTLE_ENDIAN` is set, which prints two lines). Then, when the volume names a subject
/// with a readable `mri/T1` or `mri/orig` under `SUBJECTS_DIR`, `analyse.dat` and `register.dat`
/// in the stem's folder; otherwise the geometry file ([`write_bhdr`]; FS-664: FreeSurfer names it
/// `<stem>.hdr`). See `specs/freesurfer-io.md` §105.
#[allow(clippy::too_many_arguments)]
pub fn bvolume_write(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    vol: &mut Mri,
    fname: &str,
    mtype: i32,
) -> Result<i32, Fatal> {
    let Some((ext, size)) = ext_of(mtype) else {
        let _ = writeln!(
            err,
            "ERROR: bvolumeWrite: type ({mtype}) is not short or float"
        );
        return Ok(1);
    };
    if vol.chunk.is_none() {
        g.notices.add(
            &FS_58,
            "bvolumeWrite of a volume made without voxels (FreeSurfer crashes)",
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "bvolumeWrite(): the volume has no voxels (FreeSurfer crashes here)\n",
        )?;
        return Ok(ERROR_BADPARM);
    }
    let converted = if vol.mtype != mtype {
        if verbose_on(g) {
            let _ = writeln!(out, "INFO: bvolumeWrite: changing type");
        }
        let (nslices, nframes) = (vol.vg.depth, vol.nframes);
        vol.vg.depth = nslices.wrapping_mul(nframes);
        vol.nframes = 1;
        let (mut lo, mut hi) = (0f32, 0f32);
        mri_limits(g, err, vol, &mut lo, &mut hi)?;
        if verbose_on(g) {
            let _ = writeln!(
                out,
                "INFO: bvolumeWrite: range {} {}",
                cfmt::g(f64::from(lo), 6),
                cfmt::g(f64::from(hi), 6)
            );
        }
        let r = change_type_unscaled(g, out, err, vol, mtype);
        vol.vg.depth = nslices;
        vol.nframes = nframes;
        let mut c = r?;
        c.vg.depth = nslices;
        c.nframes = nframes;
        Some(c)
    } else {
        None
    };
    let dealloc = converted.is_some();
    if dealloc && vol.nframes >= 2 {
        // FS-671: FreeSurfer's converted copy has one frame's table, freed for every frame
        g.notices.add(
            &FS_671,
            "bvolumeWrite of a converted volume with two or more frames (FreeSurfer frees frames past the copy's table at its end: memory contents, a crash or not)",
        );
    }
    let mri: &Mri = converted.as_ref().unwrap_or(vol);
    let stem = write_stem(fname);
    let _ = writeln!(out, "INFO: bvolumeWrite: stem = {stem}");
    let output_dir = match stem.rfind('/') {
        None => String::new(),
        Some(c) => format!("{}/", &stem[..c]),
    };
    let mut swap = cfg!(target_endian = "little");
    let mut endian = 0;
    if env.get("BFILE_LITTLE_ENDIAN").is_some() {
        let _ = writeln!(
            out,
            "INFO: BFILE_LITTLE_ENDIAN is set, so writing as little endian"
        );
        endian = 1;
        swap = false;
        let _ = writeln!(
            out,
            "swap = {}, order=1234, le=1234 be=4321, endian=1",
            i32::from(swap)
        );
    }
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let wu = w.max(0) as usize;
    let row_bytes = wu * size;
    for i in 0..d {
        let name = format!("{stem}_{i:03}.hdr");
        let Ok(mut fp) = CFile::open(dir, &name, "w") else {
            return Ok(fail_code(
                g,
                err,
                ERROR_BADFILE,
                &format!("bvolumeWrite(): can't open file {name}"),
            ));
        };
        fp.put_bytes(format!("{} {} {} {}\n", h, w, mri.nframes, endian).as_bytes());
        fp.close();
        let name = format!("{stem}_{i:03}.{ext}");
        let Ok(mut fp) = CFile::open(dir, &name, "w") else {
            return Ok(fail_code(
                g,
                err,
                ERROR_BADFILE,
                &format!("bvolumeWrite(): can't open file {name}"),
            ));
        };
        let chunk = mri.chunk.as_ref().expect("voxels");
        for t in 0..mri.nframes {
            for j in 0..h {
                let k = (t * d + i) as u64;
                let o = (k * mri.vox_per_slice + j as u64 * mri.vox_per_row) as usize * size;
                let mut buf = chunk[o..o + row_bytes].to_vec();
                if swap {
                    if mtype == MRI_SHORT {
                        bfileio::byteswapbufshort(g, &mut buf, row_bytes as i64);
                    } else {
                        bfileio::byteswapbuffloat(g, &mut buf, row_bytes as i64);
                    }
                }
                fp.fwrite(&buf, size, wu);
            }
        }
        fp.close();
    }
    let sn: Option<String> = (!mri.subject_name.is_empty()).then(|| mri.subject_name.clone());
    let mut wrote_subject = None;
    if let Some(sn) = &sn {
        if env.get("SUBJECTS_DIR").is_none() && dealloc {
            // FreeSurfer frees the converted volume here and then writes its geometry from it
            g.notices.add(
                &FS_665,
                "bvolumeWrite of a converted volume naming a subject without SUBJECTS_DIR (FreeSurfer crashes)",
            );
            if g.notices.reproduce() {
                g.errno = 0;
                log::error_printf(
                    g,
                    err,
                    ERROR_BADPARM,
                    "bvolumeWrite(): environment variable SUBJECTS_DIR unset",
                );
                return Err(Fatal::Exit(139));
            }
        }
        wrote_subject = subject_files(g, out, err, env, dir, mri, sn, &stem, &output_dir)?;
    }
    if let Some(r) = wrote_subject {
        return Ok(r);
    }
    let name = if g.notices.reproduce() {
        g.notices.add(
            &FS_664,
            "bvolumeWrite: the geometry written to <stem>.hdr, as in FreeSurfer (its reader reads <stem>.bhdr)",
        );
        format!("{stem}.hdr")
    } else {
        g.notices.add(
            &FS_664,
            "bvolumeWrite: the geometry written to <stem>.bhdr, which its reader reads (FreeSurfer writes <stem>.hdr)",
        );
        format!("{stem}.bhdr")
    };
    let Ok(mut fp) = CFile::open(dir, &name, "w") else {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADFILE,
            &format!("bvolumeWrite(): can't open file {name}"),
        ));
    };
    let b = write_bhdr(g, out, err, mri)?;
    fp.put_bytes(&b);
    fp.close();
    Ok(NO_ERROR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    fn short_volume(g: &mut Globals) -> Mri {
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let mut m = mri::mri_alloc_sequence(g, &mut o, &mut e, 3, 2, 2, MRI_SHORT, 2).unwrap();
        for (i, b) in m.chunk.as_mut().unwrap().iter_mut().enumerate() {
            *b = (i * 7 % 251) as u8;
        }
        (m.vg.xsize, m.vg.ysize, m.vg.zsize) = (2.0, 1.5, 3.0);
        (m.vg.x_r, m.vg.x_a, m.vg.x_s) = (0.0, 1.0, 0.0);
        (m.vg.y_r, m.vg.y_a, m.vg.y_s) = (1.0, 0.0, 0.0);
        (m.vg.z_r, m.vg.z_a, m.vg.z_s) = (0.0, 0.0, -1.0);
        (m.vg.c_r, m.vg.c_a, m.vg.c_s) = (4.0, -2.0, 1.0);
        m.tr = 2000.0;
        m
    }

    #[test]
    fn decomposes_bvolume_names_and_stems() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut e = Vec::new();
        let d = |g: &mut Globals, e: &mut Vec<u8>, n: &str| decompose_b_fname(g, e, n);
        assert_eq!(
            d(&mut g, &mut e, "w/b_000.bshort"),
            Ok(("w".into(), "b".into()))
        );
        assert_eq!(d(&mut g, &mut e, "b.bfloat"), Ok((".".into(), "b".into())));
        assert_eq!(d(&mut g, &mut e, "w/b_"), Ok(("w".into(), "b".into())));
        assert_eq!(d(&mut g, &mut e, "w/x_y"), Ok(("w".into(), "x_y".into())));
        assert_eq!(d(&mut g, &mut e, "/a_001"), Ok(("".into(), "a".into())));
        assert!(d(&mut g, &mut e, "w/").is_err());
        assert!(String::from_utf8_lossy(&e).contains("bad bshort/bfloat file specifier \"w/\""));
        assert_eq!(write_stem("w/o_000.bshort"), "w/o");
        assert_eq!(write_stem("w/o_001"), "w/o");
        assert_eq!(write_stem("w/o.bshort"), "w/o.bshort");
        assert_eq!(write_stem("_000.bshort"), "_000.bshort");
    }

    /// The writer's geometry file is read back by default (FS-664) and lost with `reproduce`.
    #[test]
    fn a_written_bvolume_reads_back_with_its_geometry_by_default() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let dir = tempfile::tempdir().unwrap();
            let p = Some(dir.path());
            let mut g = Globals::new(mode);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let env = Environment::default();
            let mut m = short_volume(&mut g);
            let r = bvolume_write(
                &mut g,
                &mut o,
                &mut e,
                &env,
                p,
                &mut m,
                "v_000.bshort",
                MRI_SHORT,
            )
            .unwrap();
            assert_eq!(r, NO_ERROR);
            assert!(g.notices.contains(&FS_664));
            let back = bvolume_read(&mut g, &mut o, &mut e, p, "v_000.bshort", true, MRI_SHORT)
                .unwrap()
                .unwrap();
            assert_eq!(back.chunk, m.chunk, "{mode:?}");
            assert_eq!(
                (back.vg.width, back.vg.height, back.vg.depth, back.nframes),
                (3, 2, 2, 2)
            );
            if mode == UpstreamDefects::Fix {
                assert_eq!(
                    (back.vg.xsize, back.vg.ysize, back.vg.zsize),
                    (2.0, 1.5, 3.0)
                );
                assert_eq!((back.vg.x_a, back.vg.y_r, back.vg.z_s), (1.0, 1.0, -1.0));
                assert_eq!(back.tr, 2000.0);
                assert_eq!(back.vg.ras_good_flag, 1);
            } else {
                assert_eq!(back.vg.xsize, 1.0);
                assert_eq!(back.vg.x_r, -1.0);
                assert_eq!(back.vg.ras_good_flag, 0);
            }
        }
    }

    /// FS-666: the bhdr type with a name that is not `.bhdr`.
    #[test]
    fn the_bhdr_type_needs_a_bhdr_name() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let dir = tempfile::tempdir().unwrap();
            let p = Some(dir.path());
            let mut g = Globals::new(mode);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let env = Environment::default();
            let mut m = short_volume(&mut g);
            let r = crate::mriio::mri_write_type(
                &mut g,
                &mut o,
                &mut e,
                &env,
                p,
                &mut m,
                "v.mgz",
                crate::identify::BHDR,
                -1,
            )
            .unwrap();
            assert!(g.notices.contains(&larmorx_core::defects::FS_666));
            let written = dir.path().join("(null)_000.bshort").exists();
            if mode == UpstreamDefects::Fix {
                assert_eq!(r, ERROR_BADPARM);
                assert!(!written);
            } else {
                assert_eq!(r, NO_ERROR);
                assert!(written);
            }
        }
    }
}
