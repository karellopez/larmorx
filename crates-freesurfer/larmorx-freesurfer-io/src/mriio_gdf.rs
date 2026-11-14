// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! GDF and SDT volumes (`utils/mriio.cpp` 5970-6520 and 10566-10870, item 3.1-27): `gdfRead`
//! ([`gdf_read`]: a text header naming one image file per slice through a `*` pattern) and
//! `gdfWrite` ([`gdf_write`]); `sdtRead` ([`sdt_read`]: Varian's `.sdt` voxels with their `.spr`
//! header) with `MRIreadRaw` and its `local_buffer_to_image` helpers. Wired into
//! [`crate::mriio::mri_read`] and [`crate::mriio::mri_write_type`].
//!
//! Defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-io.md`, Part 20):
//! FS-673 (an `.spr` header's last line without a newline), FS-674 (an `.spr` that cannot be read
//! is read for ever), FS-675 (the `.spr` name of a name without an extension in a folder with a
//! `.`), FS-676 (a GDF field without its value: memory contents), FS-677 (a GDF `DATA_TYPE` line
//! read past its end), FS-678 (GDF's `cm` and `m` units), FS-679 (GDF's crop box outside the int
//! range), FS-249 (names past FreeSurfer's buffers).

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::{
    FS_58, FS_249, FS_673, FS_674, FS_675, FS_676, FS_677, FS_678, FS_679,
};
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NO_MEMORY, ERROR_UNSUPPORTED, Fatal, NO_ERROR,
};
use larmorx_freesurfer_core::{sse, utils};
use larmorx_freesurfer_mri::mri::{self, MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, Mri};

use crate::bfileio;
use crate::cfile::CFile;
use crate::mriio::{MRI_CORONAL, MRI_HORIZONTAL, MRI_SAGITTAL, set_direction_cosine};
use crate::mriio_nifti::{narrow, widen};

/// `STRLEN` (and `STR_LEN`): FreeSurfer's name and line buffers.
const STRLEN: usize = 1024;

/// `MRI_UNDEFINED` (`mri.h`): no orientation.
const MRI_UNDEFINED: i32 = -1;

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

/// `(extent x size) / 2.0` as GCC builds it: the product in `float`, halved.
fn half_extent(n: i32, size: f32) -> f32 {
    narrow(sse::div64(widen(sse::mul(n as f32, size)), 2.0))
}

/// The line buffer `fgets` fills: the bytes past the current line's end are an earlier line's (or
/// never written: `written` bytes were).
struct LineBuf {
    buf: Vec<u8>,
    written: usize,
}

impl LineBuf {
    fn new() -> LineBuf {
        LineBuf {
            buf: vec![0; STRLEN],
            written: 0,
        }
    }
    fn put(&mut self, raw: &[u8]) {
        self.buf[..raw.len()].copy_from_slice(raw);
        self.buf[raw.len()] = 0;
        self.written = self.written.max(raw.len() + 1);
    }
}

/// `sscanf(s, "%*s <conv> ...")`: the values assigned, in order (`None` where the call stopped).
fn scan_after_word<T>(
    g: &mut Globals,
    l: &[u8],
    n: usize,
    mut conv: impl FnMut(&mut Scanf, &mut i32) -> Option<T>,
) -> Vec<Option<T>> {
    let mut b = Bytes::new(l);
    let mut errno = g.errno;
    let mut sc = Scanf::new(&mut b);
    sc.s(&mut errno, usize::MAX, false);
    let mut v = Vec::new();
    for _ in 0..n {
        sc.text(&mut errno, b" ");
        v.push(conv(&mut sc, &mut errno));
    }
    sc.finish(&mut errno);
    g.errno = errno;
    v
}

/// A header value FreeSurfer reads into a variable with no initial value: the value, or 0 (or an
/// empty text) noted as FS-676 when it was never written.
fn memory<T: Default>(g: &mut Globals, v: Option<T>, what: &str) -> T {
    v.unwrap_or_else(|| {
        g.notices.add(
            &FS_676,
            format!("gdfRead: {what} never read: 0 or empty (FreeSurfer: memory contents)"),
        );
        T::default()
    })
}

/// `(int)x` of a crop bound (`cvttss2si`): FS-679 outside the int range or NaN (the lowest int in
/// FreeSurfer; `fix` clamps, NaN 0).
fn crop_int(g: &mut Globals, x: f32) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
        g.notices.add(
            &FS_679,
            "gdfRead: a crop bound outside the int range or NaN (FreeSurfer: the lowest int)",
        );
        if g.notices.reproduce() {
            return i32::MIN;
        }
        if x.is_nan() {
            return 0;
        }
        return if x > 0.0 { i32::MAX } else { i32::MIN };
    }
    x as i32
}

/// `gdfRead(fname, read_volume)`: a GDF volume. The header's lines are taken by their first
/// characters (`IMAGE_FILE_PATH`, `IP_RES`, `SL_THICK`, `UNITS`, `SIZE`, `FS_X_RAS` ... `FS_C_RAS`,
/// `DATA_TYPE`, `ORIENTATION`, `FILE_OFFSET`, `MIN_CROP`, `MAX_CROP`); the slices are the files the
/// path's `*` stands for, numbered from 1 (or zero-padded to three digits), opened as named (from
/// the working folder). See `specs/freesurfer-io.md` §107.
#[allow(clippy::too_many_arguments)]
pub fn gdf_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    let mut fp = match CFile::open(dir, fname, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            return Ok(fail0(
                g,
                err,
                ERROR_BADFILE,
                &format!("gdfRead(): error opening file {fname}"),
            ));
        }
    };
    // None: never written (memory contents in FreeSurfer)
    let mut file_path: Option<Vec<u8>> = None;
    let mut ipr: [Option<f32>; 2] = [None, None];
    let mut st: Option<f32> = None;
    let mut units: Option<Vec<u8>> = None;
    let mut size: [Option<i32>; 2] = [None, None];
    let mut ras: [[Option<f32>; 3]; 4] = [[None; 3]; 4];
    let mut data_type_string: Option<Vec<u8>> = None;
    let mut orientation_string: Option<Vec<u8>> = None;
    let mut os_orig: Option<Vec<u8>> = None;
    let mut file_offset = 0i32;
    let mut min_crop: [Option<f32>; 3] = [None; 3];
    let mut max_crop: [Option<f32>; 3] = [None; 3];
    let (mut path_d, mut ipr_d, mut st_d, mut u_d, mut dt_d, mut o_d, mut s_d) =
        (false, false, false, false, false, false, false);
    let mut ras_d = [false; 4];
    let (mut have_min_crop, mut have_max_crop) = (false, false);
    let mut lb = LineBuf::new();
    while let Some(raw) = fp.fgets(STRLEN) {
        lb.put(&raw);
        // the last newline of the line removed
        let len = cstr(&lb.buf).len();
        if let Some(p) = lb.buf[..len].iter().rposition(|&c| c == b'\n') {
            lb.buf[p] = 0;
        }
        let line = cstr(&lb.buf).to_vec();
        let l = line.as_slice();
        let has = |p: &[u8]| l.starts_with(p);
        let word = |g: &mut Globals| {
            scan_after_word(g, l, 1, |sc, e| sc.s(e, usize::MAX, true))
                .pop()
                .flatten()
        };
        let floats = |g: &mut Globals, n: usize| scan_after_word(g, l, n, |sc, e| sc.f(e, true));
        let set_f = |slots: &mut [Option<f32>], v: Vec<Option<f32>>| {
            for (s, x) in slots.iter_mut().zip(v) {
                if x.is_some() {
                    *s = x;
                }
            }
        };
        if has(b"IMAGE_FILE_PATH") {
            if let Some(w) = word(g) {
                file_path = Some(w);
            }
            path_d = true;
        } else if has(b"IP_RES") {
            let v = floats(g, 2);
            set_f(&mut ipr, v);
            ipr_d = true;
        } else if has(b"SL_THICK") {
            let v = floats(g, 1);
            set_f(std::slice::from_mut(&mut st), v);
            st_d = true;
        } else if has(b"UNITS") {
            if let Some(w) = word(g) {
                units = Some(w);
            }
            u_d = true;
        } else if has(b"SIZE") {
            let v = scan_after_word(g, l, 2, |sc, e| sc.d(e, true));
            for (s, x) in size.iter_mut().zip(v) {
                if x.is_some() {
                    *s = x;
                }
            }
            s_d = true;
        } else if let Some(k) = [b"FS_X_RAS", b"FS_Y_RAS", b"FS_Z_RAS", b"FS_C_RAS"]
            .iter()
            .position(|p| has(*p))
        {
            let v = floats(g, 3);
            set_f(&mut ras[k], v);
            ras_d[k] = true;
        } else if has(b"DATA_TYPE") {
            // strcpy(data_type_string, &line[10]): ten bytes in, whatever the line's length
            let s = cstr(&lb.buf[10..]).to_vec();
            if l.len() < 10 {
                if g.notices.reproduce() && 10 + s.len() < lb.written {
                    g.notices.add(
                        &FS_677,
                        "gdfRead: a DATA_TYPE line read past its end from an earlier line's bytes, as in FreeSurfer",
                    );
                    data_type_string = Some(s);
                } else {
                    g.notices.add(
                        &FS_677,
                        if g.notices.reproduce() {
                            "gdfRead: a DATA_TYPE line read past its end into memory never written (0 bytes here)"
                        } else {
                            "gdfRead: a DATA_TYPE line without its type: empty (FreeSurfer reads past the line's end)"
                        },
                    );
                    data_type_string = Some(if g.notices.reproduce() {
                        s
                    } else {
                        Vec::new()
                    });
                }
            } else {
                data_type_string = Some(s);
            }
            dt_d = true;
        } else if has(b"ORIENTATION") {
            if let Some(w) = word(g) {
                orientation_string = Some(w);
            }
            os_orig = orientation_string.clone();
            o_d = true;
        } else if has(b"FILE_OFFSET") {
            if let Some(v) = scan_after_word(g, l, 1, |sc, e| sc.d(e, true)).pop().flatten() {
                file_offset = v;
            }
        } else if has(b"MIN_CROP") {
            let v = floats(g, 3);
            set_f(&mut min_crop, v);
            have_min_crop = true;
        } else if has(b"MAX_CROP") {
            let v = floats(g, 3);
            set_f(&mut max_crop, v);
            have_max_crop = true;
        }
    }
    drop(fp);
    if !(path_d && ipr_d && st_d && s_d) {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("gdfRead(): missing field(s) from {fname}:"),
        );
        for (d, what) in [
            (path_d, "IMAGE_FILE_PATH"),
            (ipr_d, "IP_RES"),
            (st_d, "SL_THICK"),
            (s_d, "SIZE"),
        ] {
            if !d {
                log::error_printf(g, err, ERROR_BADPARM, &format!("  {what}"));
            }
        }
        return Ok(None);
    }
    let all_ras = ras_d[0] && ras_d[1] && ras_d[2];
    if !o_d {
        if all_ras {
            let _ = writeln!(
                out,
                "missing field ORIENTATION in file {fname}, but you've got {{xyz}}_{{ras}}, so never mind"
            );
        } else {
            let _ = writeln!(
                out,
                "missing field ORIENTATION in file {fname}; assuming 'coronal'"
            );
            orientation_string = Some(b"coronal".to_vec());
        }
    }
    if !dt_d {
        let _ = writeln!(
            out,
            "missing field DATA_TYPE in file {fname}; assuming 'short'"
        );
        data_type_string = Some(b"short".to_vec());
    }
    if !u_d {
        let _ = writeln!(out, "missing field UNITS in file {fname}; assuming 'mm'");
        units = Some(b"mm".to_vec());
    }
    let lower = |b: Vec<u8>| -> Vec<u8> { b.to_ascii_lowercase() };
    let dts = lower(memory(g, data_type_string, "the DATA_TYPE text"));
    // with the axes given and no ORIENTATION line, FreeSurfer lowers a text it never wrote; the
    // orientation is then not used
    let ors = lower(match orientation_string {
        Some(s) => s,
        None if !o_d => Vec::new(),
        None => memory(g, None, "the ORIENTATION text"),
    });
    let us = lower(memory(g, units, "the UNITS text"));
    let data_type = match dts.as_slice() {
        b"short" => MRI_SHORT,
        b"float" => MRI_FLOAT,
        b"unsigned char" => MRI_UCHAR,
        _ => {
            let mut m = b"gdfRead(): unknown data type '".to_vec();
            m.extend_from_slice(&dts);
            m.push(b'\'');
            g.errno = 0;
            log::error_printf_bytes(g, err, ERROR_BADPARM, &m);
            return Ok(None);
        }
    };
    let or_ptr: &[u8] = match ors.iter().rposition(|&c| c == b' ') {
        Some(p) => &ors[p + 1..],
        None => &ors,
    };
    let mut orientation = MRI_UNDEFINED;
    if or_ptr.starts_with(b"cor") {
        orientation = MRI_CORONAL;
    } else if or_ptr.starts_with(b"sag") {
        orientation = MRI_SAGITTAL;
    } else if or_ptr.starts_with(b"ax") || or_ptr.starts_with(b"hor") {
        orientation = MRI_HORIZONTAL;
    } else if !all_ras {
        let mut m = b"gdfRead(): can't determine orientation from string '".to_vec();
        m.extend_from_slice(&os_orig.unwrap_or_default());
        m.push(b'\'');
        g.errno = 0;
        log::error_printf_bytes(g, err, ERROR_BADPARM, &m);
        return Ok(None);
    }
    // FS-678: FreeSurfer divides centimetres by 1 and metres by 100; `fix` multiplies them by 10
    // and 1,000
    let (factor, multiply): (f32, bool) = match us.as_slice() {
        b"mm" => (1.0, false),
        b"cm" | b"m" => {
            let wrong = g.notices.reproduce();
            g.notices.add(
                &FS_678,
                if wrong {
                    "gdfRead: centimetres divided by 1 and metres by 100, as in FreeSurfer"
                } else {
                    "gdfRead: centimetres and metres converted to millimetres (FreeSurfer divides them by 1 and 100)"
                },
            );
            match (us.as_slice(), wrong) {
                (b"cm", true) => (1.0, false),
                (b"cm", false) => (10.0, true),
                (_, true) => (100.0, false),
                _ => (1000.0, true),
            }
        }
        _ => {
            let mut m = b"gdfRead(): unknown units '".to_vec();
            m.extend_from_slice(&us);
            m.push(b'\'');
            g.errno = 0;
            log::error_printf_bytes(g, err, ERROR_BADPARM, &m);
            return Ok(None);
        }
    };
    let scale = |x: f32| {
        if multiply {
            sse::mul(x, factor)
        } else {
            sse::div(x, factor)
        }
    };
    let ipr0 = scale(memory(g, ipr[0], "IP_RES's first number"));
    let ipr1 = scale(memory(g, ipr[1], "IP_RES's second number"));
    let st = scale(memory(g, st, "SL_THICK's number"));
    if g.gdf_crop_flag && !(have_min_crop && have_max_crop) {
        return Ok(fail0(
            g,
            err,
            ERROR_BADPARM,
            "gdfRead(): cropping desired but missing MIN_CROP or MAX_CROP\n",
        ));
    }
    let file_path = memory(g, file_path, "the IMAGE_FILE_PATH text");
    let Some(star) = file_path.iter().rposition(|&c| c == b'*') else {
        let mut m = b"gdfRead(): file path ".to_vec();
        m.extend_from_slice(&file_path);
        m.extend_from_slice(b" does not contain '*'\n");
        g.errno = 0;
        log::error_printf_bytes(g, err, ERROR_BADPARM, &m);
        return Ok(None);
    };
    let (p1, p2) = (text(&file_path[..star]), text(&file_path[star + 1..]));
    let name_of = |n: i32, pad: bool| {
        if pad {
            format!("{p1}{n:03}{p2}")
        } else {
            format!("{p1}{n}{p2}")
        }
    };
    let mut pad = false;
    let mut n_files = 0;
    let mut use_name;
    loop {
        n_files += 1;
        use_name = name_of(n_files, false);
        if !utils::file_exists(dir, &use_name) {
            break;
        }
    }
    if n_files == 1 {
        pad = true;
        n_files = 0;
        loop {
            n_files += 1;
            use_name = name_of(n_files, true);
            if !utils::file_exists(dir, &use_name) {
                break;
            }
        }
        if n_files == 1 {
            return Ok(fail0(
                g,
                err,
                ERROR_BADFILE,
                &format!("gdfRead(): can't find file {p1}1{p2} or {use_name}\n"),
            ));
        }
    }
    fits(g, "gdfRead", &use_name);
    n_files -= 1;
    let w = memory(g, size[0], "SIZE's first number");
    let h = memory(g, size[1], "SIZE's second number");
    let mut m = if read_volume {
        mri::mri_alloc_sequence(g, out, err, w, h, n_files, data_type, 1)?
    } else {
        mri::mri_alloc_header(g, out, err, w, h, n_files, data_type, 1)?
    };
    m.vg.xsize = ipr0;
    m.vg.ysize = ipr1;
    m.vg.zsize = st;
    m.xend = half_extent(m.vg.width, m.vg.xsize);
    m.xstart = -m.xend;
    m.yend = half_extent(m.vg.height, m.vg.ysize);
    m.ystart = -m.yend;
    m.zend = half_extent(m.vg.depth, m.vg.zsize);
    m.zstart = -m.zend;
    m.vg.fname.strcpy_bytes(fname.as_bytes());
    if all_ras {
        let mut v = [[0f32; 3]; 3];
        let names = ["FS_X_RAS", "FS_Y_RAS", "FS_Z_RAS"];
        for k in 0..3 {
            for c in 0..3 {
                v[k][c] = memory(g, ras[k][c], names[k]);
            }
        }
        (m.vg.x_r, m.vg.x_a, m.vg.x_s) = (v[0][0], v[0][1], v[0][2]);
        (m.vg.y_r, m.vg.y_a, m.vg.y_s) = (v[1][0], v[1][1], v[1][2]);
        (m.vg.z_r, m.vg.z_a, m.vg.z_s) = (v[2][0], v[2][1], v[2][2]);
        m.vg.ras_good_flag = 1;
    } else {
        if set_direction_cosine(g, err, &mut m.vg, orientation) != NO_ERROR {
            return Ok(None);
        }
        let _ = writeln!(out, "warning: gdf volume may be incorrectly oriented");
    }
    if ras_d[3] {
        m.vg.c_r = memory(g, ras[3][0], "FS_C_RAS");
        m.vg.c_a = memory(g, ras[3][1], "FS_C_RAS");
        m.vg.c_s = memory(g, ras[3][2], "FS_C_RAS");
    } else {
        (m.vg.c_r, m.vg.c_a, m.vg.c_s) = (0.0, 0.0, 0.0);
        let _ = writeln!(out, "warning: gdf volume may be incorrectly centered");
    }
    if !read_volume {
        return Ok(Some(m));
    }
    let bpv = match data_type {
        MRI_UCHAR => 1usize,
        MRI_SHORT => 2,
        _ => 4,
    };
    let wu = m.vg.width.max(0) as usize;
    for i in 1..=n_files {
        let name = name_of(i, pad);
        let mut fp = match CFile::open(dir, &name, "r") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("gdfRead(): error opening file {name}"),
                ));
            }
        };
        let mut errno = g.errno;
        fp.fseek(&mut errno, i64::from(file_offset), 0);
        g.errno = errno;
        for j in 0..m.vg.height {
            let mut row = vec![0u8; wu * bpv];
            if fp.fread(&mut row, bpv, wu) != wu {
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("gdfRead(): error reading from file {name}"),
                ));
            }
            if bpv > 1 {
                for v in row.chunks_mut(bpv) {
                    v.reverse();
                }
            }
            let o = ((i - 1) as u64 * m.vox_per_slice + j as u64 * m.vox_per_row) as usize * bpv;
            if let Some(c) = m.chunk.as_mut() {
                c[o..o + wu * bpv].copy_from_slice(&row);
            }
        }
    }
    if g.gdf_crop_flag {
        let _ = writeln!(out, "NOTICE: cropping GDF data");
        let lo: Vec<f32> = (0..3).map(|k| memory(g, min_crop[k], "MIN_CROP")).collect();
        let hi: Vec<f32> = (0..3).map(|k| memory(g, max_crop[k], "MAX_CROP")).collect();
        let (x0, x1) = (crop_int(g, lo[0]), crop_int(g, hi[0]));
        let (y0, y1) = (crop_int(g, lo[2]), crop_int(g, hi[2]));
        let (z0, z1) = (
            crop_int(g, lo[1]).wrapping_sub(1),
            crop_int(g, hi[1]).wrapping_sub(1),
        );
        let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
        let (vpr, vps) = (m.vox_per_row, m.vox_per_slice);
        if let Some(c) = m.chunk.as_mut() {
            for k in 0..d {
                for j in 0..h {
                    for i in 0..w {
                        if i < x0 || i > x1 || j < y0 || j > y1 || k < z0 || k > z1 {
                            let o = (k as u64 * vps + j as u64 * vpr + i as u64) as usize * bpv;
                            c[o..o + bpv].fill(0);
                        }
                    }
                }
            }
        }
    } else {
        let _ = writeln!(out, "NOTICE: not cropping GDF data");
    }
    Ok(Some(m))
}

/// `gdfWrite(mri, fname)`: the volume's slices as `<gdf_image_stem>_<n>.img` (from 1; big-endian
/// shorts and floats) and the header `fname`. Without an image stem: `GDF write attempted without
/// GDF image stem`; frames other than 1: `GDF write attempted with <n> frames (supported only for 1
/// frame)`; types other than bytes, floats, shorts: `gdfWrite(): bad data type (<t>) for GDF write
/// (only uchar, float, short supported)` (`ERROR_BADPARM`). See `specs/freesurfer-io.md` §107.
pub fn gdf_write(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    m: &Mri,
    fname: &str,
) -> Result<i32, Fatal> {
    if m.gdf_image_stem.is_empty() {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADPARM,
            "GDF write attempted without GDF image stem",
        ));
    }
    if m.nframes != 1 {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "GDF write attempted with {} frames (supported only for 1 frame)",
                m.nframes
            ),
        ));
    }
    if !matches!(m.mtype, MRI_UCHAR | MRI_FLOAT | MRI_SHORT) {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "gdfWrite(): bad data type ({}) for GDF write (only uchar, float, short supported)",
                m.mtype
            ),
        ));
    }
    let _ = writeln!(out, "writing GDF image files...");
    let bpv = m.bytes_per_vox;
    let buf_size = m.vg.width as i64 * bpv as i64;
    if buf_size < 0 {
        return Ok(fail_code(
            g,
            err,
            ERROR_NO_MEMORY,
            "gdfWrite(): no memory for voxel write buffer",
        ));
    }
    let Some(chunk) = m.chunk.as_ref() else {
        g.notices.add(
            &FS_58,
            "gdfWrite of a volume made without voxels (FreeSurfer crashes)",
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "gdfWrite(): the volume has no voxels (FreeSurfer crashes here)\n",
        )?;
        return Ok(ERROR_BADPARM);
    };
    let bs = buf_size as usize;
    for i in 0..m.vg.depth {
        let name = format!("{}_{}.img", m.gdf_image_stem, i + 1);
        let Ok(mut fp) = CFile::open(dir, &name, "w") else {
            return Ok(fail_code(
                g,
                err,
                ERROR_BADFILE,
                &format!("gdfWrite(): error opening file {name}"),
            ));
        };
        for j in 0..m.vg.height {
            let o = (i as u64 * m.vox_per_slice + j as u64 * m.vox_per_row) as usize * bpv;
            let mut buf = chunk[o..o + bs].to_vec();
            if m.mtype == MRI_FLOAT {
                bfileio::byteswapbuffloat(g, &mut buf, buf_size);
            }
            if m.mtype == MRI_SHORT {
                bfileio::byteswapbufshort(g, &mut buf, buf_size);
            }
            fp.fwrite(&buf, 1, bs);
        }
        fp.close();
    }
    let _ = writeln!(out, "writing GDF info file...");
    let Ok(mut fp) = CFile::open(dir, fname, "w") else {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADFILE,
            &format!("gdfWrite(): error opening file {fname}"),
        ));
    };
    let v = &m.vg;
    let gg = |x: f32| cfmt::g(f64::from(x), 6);
    let ff = |x: f32| cfmt::f(f64::from(x), 6);
    let mut s = format!(
        "GDF FILE VERSION3\nSTART MAIN HEADER\nIMAGE_FILE_PATH {}_*.img\nSIZE {} {}\nIP_RES {} {}\nSL_THICK {}\n",
        m.gdf_image_stem,
        v.width,
        v.height,
        gg(v.xsize),
        gg(v.ysize),
        gg(v.zsize)
    );
    if v.ras_good_flag != 0 {
        for (n, a, b, c) in [
            ("FS_X_RAS", v.x_r, v.x_a, v.x_s),
            ("FS_Y_RAS", v.y_r, v.y_a, v.y_s),
            ("FS_Z_RAS", v.z_r, v.z_a, v.z_s),
            ("FS_C_RAS", v.c_r, v.c_a, v.c_s),
        ] {
            s.push_str(&format!("{n} {} {} {}\n", ff(a), ff(b), ff(c)));
        }
    }
    s.push_str("FILE_OFFSET 0\n");
    s.push_str(match m.mtype {
        MRI_UCHAR => "DATA_TYPE unsigned char\n",
        MRI_FLOAT => "DATA_TYPE float\n",
        _ => "DATA_TYPE short\n",
    });
    s.push_str("END MAIN HEADER\n");
    fp.put_bytes(s.as_bytes());
    fp.close();
    Ok(NO_ERROR)
}

/// The voxel bytes `MRIreadRaw` reads per type (`data_size[]`: bytes, ints, longs, floats,
/// shorts).
fn data_size(t: i32) -> usize {
    match t {
        MRI_UCHAR => 1,
        MRI_SHORT => 2,
        _ => 4,
    }
}

/// `MRIreadRaw(fp, width, height, depth, type)`: `depth` big-endian slices of `width x height`
/// voxels; a short slice: `<Progname>: could not read <k>th slice (<pixels>)` (`ERROR_BADFILE`)
/// and `None`.
#[allow(clippy::too_many_arguments)]
pub fn mri_read_raw(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    fp: &mut CFile,
    width: i32,
    height: i32,
    depth: i32,
    mtype: i32,
) -> Result<Option<Mri>, Fatal> {
    let mut m = mri::mri_alloc(g, out, err, width, height, depth, mtype)?;
    let pixels = width.wrapping_mul(height);
    let ds = data_size(mtype);
    let n = pixels.max(0) as usize;
    let wu = width.max(0) as usize;
    for slice in 0..depth {
        let mut buf = vec![0u8; n * ds];
        if fp.fread(&mut buf, ds, n) != n {
            let p = g.progname.clone();
            return Ok(fail0(
                g,
                err,
                ERROR_BADFILE,
                &format!("{p}: could not read {slice}th slice ({pixels})"),
            ));
        }
        if ds > 1 {
            for v in buf.chunks_mut(ds) {
                v.reverse();
            }
        }
        let bpv = m.bytes_per_vox;
        let (vpr, vps) = (m.vox_per_row, m.vox_per_slice);
        if let Some(c) = m.chunk.as_mut() {
            for y in 0..height.max(0) as usize {
                let o = (slice as u64 * vps + y as u64 * vpr) as usize * bpv;
                c[o..o + wu * ds].copy_from_slice(&buf[y * wu * ds..(y + 1) * wu * ds]);
            }
        }
    }
    Ok(Some(m))
}

/// `sdtRead(fname, read_volume)`: a Varian SDT volume: the header `<name>.spr` (the name with
/// what follows its last `.` replaced by `spr`, or `.spr` added) of `name: value` lines (`numDim`,
/// `dim`, `dataType`, `interval` in cm, `sdtOrient`), the voxels the name's file. See
/// `specs/freesurfer-io.md` §108.
pub fn sdt_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    fits(g, "sdtRead", fname);
    let hname = match fname.rfind('.') {
        Some(d) => {
            let last_slash = fname.rfind('/');
            if last_slash.is_some_and(|s| s > d) {
                // FS-675: the '.' is in a folder's name
                g.notices.add(
                    &FS_675,
                    if g.notices.reproduce() {
                        "sdtRead: the header's name cut at a folder's '.', as in FreeSurfer"
                    } else {
                        "sdtRead: the header's name from the file's own name (FreeSurfer cuts it at a folder's '.')"
                    },
                );
                if g.notices.reproduce() {
                    format!("{}spr", &fname[..=d])
                } else {
                    format!("{fname}.spr")
                }
            } else {
                format!("{}spr", &fname[..=d])
            }
        }
        None => format!("{fname}.spr"),
    };
    let mut fp = match CFile::open(dir, &hname, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            return Ok(fail0(
                g,
                err,
                ERROR_BADFILE,
                &format!("sdtRead({fname}): could not open header file {hname}\n"),
            ));
        }
    };
    let mut ndim = -1i32;
    let mut data_type = -1i32;
    let mut dim = [-1i32; 4];
    let (mut xsize, mut ysize, mut zsize) = (1.0f32, 1.0f32, 1.0f32);
    let mut orientation = MRI_CORONAL;
    loop {
        let got = fp.fgets(STRLEN);
        if got.is_none() && fp.ferror() {
            log::error_printf(
                g,
                err,
                ERROR_BADFILE,
                &format!("stdRead({fname}): could not read file"),
            );
            g.notices.add(
                &FS_674,
                "sdtRead: a header that cannot be read (FreeSurfer tries again for ever): read as empty",
            );
            break;
        }
        if fp.feof() {
            if got.as_ref().is_some_and(|l| !l.is_empty()) && !g.notices.reproduce() {
                g.notices.add(
                    &FS_673,
                    "sdtRead: the header's last line without a newline read (FreeSurfer ignores it)",
                );
            } else {
                if got.as_ref().is_some_and(|l| !l.is_empty()) {
                    g.notices.add(
                        &FS_673,
                        "sdtRead: the header's last line without a newline ignored, as in FreeSurfer",
                    );
                }
                break;
            }
        }
        let at_end = fp.feof();
        let Some(raw) = got else { break };
        let line = cstr(&raw).to_vec();
        if let Some(c) = line.iter().position(|&b| b == b':') {
            let (name, value) = (&line[..c], &line[c + 1..]);
            let ints = |g: &mut Globals, n: usize| -> Vec<Option<i32>> {
                let mut b = Bytes::new(value);
                let mut errno = g.errno;
                let mut sc = Scanf::new(&mut b);
                let mut v = Vec::new();
                for k in 0..n {
                    if k > 0 {
                        sc.text(&mut errno, b" ");
                    }
                    v.push(sc.d(&mut errno, true));
                }
                sc.finish(&mut errno);
                g.errno = errno;
                v
            };
            let skip_space = |v: &[u8]| -> Vec<u8> {
                let k = v
                    .iter()
                    .take_while(|&&c| matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r'))
                    .count();
                v[k..].to_vec()
            };
            match name {
                b"numDim" => {
                    if let Some(v) = ints(g, 1)[0] {
                        ndim = v;
                    }
                    if !(3..=4).contains(&ndim) {
                        return Ok(fail0(
                            g,
                            err,
                            ERROR_UNSUPPORTED,
                            &format!(
                                "sdtRead({fname}): only 3 or 4 dimensions supported (numDim = {ndim})\n"
                            ),
                        ));
                    }
                }
                b"dim" => {
                    if ndim == -1 {
                        return Ok(fail0(
                            g,
                            err,
                            ERROR_BADFILE,
                            &format!(
                                "sdtRead({fname}): 'dim' before 'numDim' in header file {hname}\n"
                            ),
                        ));
                    }
                    let k = if ndim == 3 { 3 } else { 4 };
                    for (s, v) in dim.iter_mut().zip(ints(g, k)) {
                        if let Some(v) = v {
                            *s = v;
                        }
                    }
                    if ndim == 3 {
                        dim[3] = 1;
                    } else if dim[3] != 1 {
                        return Ok(fail0(
                            g,
                            err,
                            ERROR_UNSUPPORTED,
                            &format!(
                                "sdtRead({fname}): nframes != 1 unsupported for sdt (dim(4) = {})\n",
                                dim[3]
                            ),
                        ));
                    }
                }
                b"dataType" => {
                    let v = skip_space(value);
                    data_type = if v.starts_with(b"BYTE") {
                        MRI_UCHAR
                    } else if v.starts_with(b"WORD") {
                        MRI_SHORT
                    } else if v.starts_with(b"LWORD") {
                        MRI_INT
                    } else if v.starts_with(b"REAL") {
                        MRI_FLOAT
                    } else {
                        let (code, what) = if v.starts_with(b"COMPLEX") {
                            (ERROR_UNSUPPORTED, "unsupported")
                        } else {
                            (ERROR_BADFILE, "unknown")
                        };
                        let mut m = format!("sdtRead({fname}): {what} data type '").into_bytes();
                        m.extend_from_slice(&v);
                        m.extend_from_slice(b"'\n");
                        g.errno = 0;
                        log::error_printf_bytes(g, err, code, &m);
                        return Ok(None);
                    };
                }
                b"interval" => {
                    let k = if ndim == 3 { 3 } else { 4 };
                    let mut b = Bytes::new(value);
                    let mut errno = g.errno;
                    let mut sc = Scanf::new(&mut b);
                    let mut v = Vec::new();
                    for i in 0..k {
                        if i > 0 {
                            sc.text(&mut errno, b" ");
                        }
                        v.push(sc.f(&mut errno, true));
                    }
                    sc.finish(&mut errno);
                    g.errno = errno;
                    for (s, x) in [&mut xsize, &mut ysize, &mut zsize].into_iter().zip(v) {
                        if let Some(x) = x {
                            *s = x;
                        }
                    }
                    for s in [&mut xsize, &mut ysize, &mut zsize] {
                        *s = narrow(sse::mul64(widen(*s), 10.0));
                    }
                }
                b"sdtOrient" => {
                    let v = skip_space(value);
                    if v.starts_with(b"sag") {
                        orientation = MRI_SAGITTAL;
                    } else if v.starts_with(b"ax") {
                        orientation = MRI_HORIZONTAL;
                    } else if v.starts_with(b"cor") {
                        orientation = MRI_CORONAL;
                    } else {
                        let mut m = format!("sdtRead({fname}): unknown orientation ").into_bytes();
                        m.extend_from_slice(&v);
                        m.push(b'\n');
                        g.errno = 0;
                        log::error_printf_bytes(g, err, ERROR_BADFILE, &m);
                        return Ok(None);
                    }
                }
                _ => {}
            }
        }
        if at_end {
            break;
        }
    }
    drop(fp);
    if data_type == -1 {
        return Ok(fail0(
            g,
            err,
            ERROR_BADFILE,
            &format!("sdtRead({fname}): data type undefined\n"),
        ));
    }
    if dim[0] == -1 || dim[1] == -1 || dim[2] == -1 {
        return Ok(fail0(
            g,
            err,
            ERROR_BADFILE,
            &format!("sdtRead({fname}): one or more dimensions undefined\n"),
        ));
    }
    let mut m = if read_volume {
        let mut fp = match CFile::open(dir, fname, "r") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("sdtRead({fname}): error opening data file {fname}\n"),
                ));
            }
        };
        match mri_read_raw(g, out, err, &mut fp, dim[0], dim[1], dim[2], data_type)? {
            Some(m) => m,
            None => return Ok(None),
        }
    } else {
        mri::mri_alloc_header(g, out, err, dim[0], dim[1], dim[2], data_type, dim[3])?
    };
    m.vg.xsize = xsize;
    m.vg.ysize = ysize;
    m.vg.zsize = zsize;
    set_direction_cosine(g, err, &mut m.vg, orientation);
    m.thick = m.vg.zsize;
    m.xend = half_extent(m.vg.width, m.vg.xsize);
    m.yend = half_extent(m.vg.height, m.vg.ysize);
    m.zend = half_extent(m.vg.depth, m.vg.zsize);
    m.xstart = -m.xend;
    m.ystart = -m.yend;
    m.zstart = -m.zend;
    m.imnr0 = 1;
    m.imnr1 = dim[2];
    m.ps = 1.0;
    m.tr = 0.0;
    m.te = 0.0;
    m.ti = 0.0;
    m.vg.fname.strcpy_bytes(fname.as_bytes());
    Ok(Some(m))
}
