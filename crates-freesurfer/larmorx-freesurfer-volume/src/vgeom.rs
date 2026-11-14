// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/transform.cpp`'s `VOL_GEOM` functions (3.1-39, taken first for the surface readers):
//! the text block a linear transform file and a surface file carry (`writeVolGeom`,
//! `readVolGeom`), the obsolete printer `vg_print`, the copies `getVolGeom` and `copyVolGeom`, the
//! matrices `vg_i_to_r`, `vg_r_to_i` and `TkrVox2RASfromVolGeom`, and the comparison
//! `vg_isEqual` / `vg_isNotEqualThresh` (which, unlike `VOL_GEOM::operator==`, ignores the shears).

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_285, FS_286, FS_287, FS_288};
use larmorx_freesurfer_core::cscan::{self, Scan, Stream};
use larmorx_freesurfer_core::cstd::{fmt_f, is_space};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_mri::STRLEN;
use larmorx_freesurfer_mri::tkreg::xfm_crs2xyz_tkreg;
use larmorx_freesurfer_mri::vol_geom::{VolGeom, extract_i_to_r, extract_r_to_i, init_vol_geom};

/// `vg_print(vg)` (obsolete, `vg->vgprint()` replaces it): the geometry on standard error when
/// `valid` is 1 (extent, voxel sizes and the direction cosines and centre as `%7.4f`, the file
/// name), otherwise one line saying it is not valid.
pub fn vg_print(err: &mut dyn Write, vg: &VolGeom) {
    if vg.valid == 1 {
        let f = |x: f32| fmt_f(f64::from(x), 7, 4);
        let _ = err.write_all(b"volume geometry:\n");
        let _ = writeln!(err, "extent  : ({}, {}, {})", vg.width, vg.height, vg.depth);
        for (name, a, b, c) in [
            ("voxel   ", vg.xsize, vg.ysize, vg.zsize),
            ("x_(ras) ", vg.x_r, vg.x_a, vg.x_s),
            ("y_(ras) ", vg.y_r, vg.y_a, vg.y_s),
            ("z_(ras) ", vg.z_r, vg.z_a, vg.z_s),
            ("c_(ras) ", vg.c_r, vg.c_a, vg.c_s),
        ] {
            let _ = writeln!(err, "{name}: ({}, {}, {})", f(a), f(b), f(c));
        }
        let _ = err.write_all(b"file    : ");
        let _ = err.write_all(vg.fname.c_bytes());
        let _ = err.write_all(b"\n");
    } else {
        let _ = err.write_all(b"volume geometry: info is either not contained or not valid.\n");
    }
    let _ = err.flush();
}

/// The fields `getVolGeom` and `copyVolGeom` copy: the extent, voxel sizes, direction cosines,
/// centre and name (`strcpy`: the bytes after the name stay the destination's); never
/// `ras_good_flag`, the shears or the cached matrices.
fn copy_common(src: &VolGeom, dst: &mut VolGeom) {
    dst.width = src.width;
    dst.height = src.height;
    dst.depth = src.depth;
    dst.xsize = src.xsize;
    dst.ysize = src.ysize;
    dst.zsize = src.zsize;
    dst.x_r = src.x_r;
    dst.x_a = src.x_a;
    dst.x_s = src.x_s;
    dst.y_r = src.y_r;
    dst.y_a = src.y_a;
    dst.y_s = src.y_s;
    dst.z_r = src.z_r;
    dst.z_a = src.z_a;
    dst.z_s = src.z_s;
    dst.c_r = src.c_r;
    dst.c_a = src.c_a;
    dst.c_s = src.c_s;
    dst.fname.strcpy(&src.fname);
}

/// `getVolGeom(src, dst)`: the destination marked valid, then the common fields copied.
pub fn get_vol_geom(src: &VolGeom, dst: &mut VolGeom) {
    dst.valid = 1;
    copy_common(src, dst);
}

/// `copyVolGeom(src, dst)`: `valid` and the common fields copied.
pub fn copy_vol_geom(src: &VolGeom, dst: &mut VolGeom) {
    dst.valid = src.valid;
    copy_common(src, dst);
}

/// `writeVolGeom(fp, vg)`: the eight lines of the volume-geometry block (`valid = %d` with its
/// comment, `filename = %s`, `volume = %d %d %d`, then the voxel sizes, the three direction
/// cosines and the centre as `%.15e`).
pub fn write_vol_geom(fp: &mut CFile, vg: &VolGeom) {
    fp.put_bytes(&vol_geom_text(vg));
}

/// The bytes [`write_vol_geom`] writes (for writers into other streams: `LTAprint`).
pub fn vol_geom_text(vg: &VolGeom) -> Vec<u8> {
    let mut s = Vec::new();
    if vg.valid == 0 {
        s.extend_from_slice(format!("valid = {}  # volume info invalid\n", vg.valid).as_bytes());
    } else {
        s.extend_from_slice(format!("valid = {}  # volume info valid\n", vg.valid).as_bytes());
    }
    s.extend_from_slice(b"filename = ");
    s.extend_from_slice(vg.fname.c_bytes());
    s.push(b'\n');
    s.extend_from_slice(format!("volume = {} {} {}\n", vg.width, vg.height, vg.depth).as_bytes());
    let e = |x: f32| cfmt::e(f64::from(x), 15);
    for (name, a, b, c) in [
        ("voxelsize = ", vg.xsize, vg.ysize, vg.zsize),
        ("xras   = ", vg.x_r, vg.x_a, vg.x_s),
        ("yras   = ", vg.y_r, vg.y_a, vg.y_s),
        ("zras   = ", vg.z_r, vg.z_a, vg.z_s),
        ("cras   = ", vg.c_r, vg.c_a, vg.c_s),
    ] {
        s.extend_from_slice(format!("{name}{} {} {}\n", e(a), e(b), e(c)).as_bytes());
    }
    s
}

/// A line as `sscanf` reads it (its end is the C string's end), noting whether a read found the
/// end.
struct Line<'a> {
    s: &'a [u8],
    i: usize,
    met: bool,
}

impl Stream for Line<'_> {
    fn peek(&mut self) -> Option<u8> {
        let c = self.s.get(self.i).copied();
        if c.is_none() {
            self.met = true;
        }
        c
    }
    fn bump(&mut self) {
        self.i += 1;
    }
    fn hit_end(&mut self) -> bool {
        self.met
    }
}

/// One `sscanf` call's conversions on a line, with glibc 2.35's `errno` at the end of the input as
/// measured as a black box (the open `validation/investigations/glibc-scanf-probe/`): the first
/// read of a call that finds the end remembers `errno` (as 0, and sets it to 0, when the read is a
/// conversion's skip of the white space before its item), and every later read that finds the end
/// sets `errno` back to the remembered value; a number looks one character past itself, `%s` stops
/// at its width without looking further; a white-space directive is skipped by the next conversion
/// (so as a conversion's skip), or, at the end of the format, on its own.
struct Scanf<'a> {
    st: Line<'a>,
    saved: Option<i32>,
    /// Items assigned so far (`sscanf`'s value unless it is `EOF`).
    n: i32,
    /// Whether a conversion met the end before any assignment (`sscanf` returns `EOF`).
    eof: bool,
    /// Whether the call stopped (a matching or input failure).
    stopped: bool,
}

impl<'a> Scanf<'a> {
    fn new(line: &'a [u8]) -> Scanf<'a> {
        let end = line.iter().position(|&c| c == 0).unwrap_or(line.len());
        Scanf {
            st: Line {
                s: &line[..end],
                i: 0,
                met: false,
            },
            saved: None,
            n: 0,
            eof: false,
            stopped: false,
        }
    }

    /// A read found the end: the first one remembers `errno` (0 in a conversion's skip of white
    /// space), later ones restore it.
    fn meet(&mut self, errno: &mut i32, in_skip: bool) {
        match self.saved {
            None => {
                let v = if in_skip { 0 } else { *errno };
                self.saved = Some(v);
                *errno = v;
            }
            Some(v) => *errno = v,
        }
    }

    /// A conversion's skip of white space: `false` (an input failure) when no character is left.
    fn begin(&mut self, errno: &mut i32) -> bool {
        if self.stopped {
            return false;
        }
        cscan::skip_space(&mut self.st);
        if self.st.i >= self.st.s.len() {
            self.meet(errno, true);
            if self.n == 0 {
                self.eof = true;
            }
            self.stopped = true;
            return false;
        }
        self.st.met = false;
        true
    }

    /// After a conversion's item: the end it met, then its `ERANGE`.
    fn finish<T>(&mut self, r: Scan<T>, errno: &mut i32, assign: bool) -> Option<T> {
        if self.st.met {
            self.meet(errno, false);
        }
        match r {
            Scan::Value(v, erange) => {
                if erange {
                    *errno = cscan::ERANGE;
                }
                if assign {
                    self.n += 1;
                }
                Some(v)
            }
            Scan::Fail | Scan::Eof => {
                self.stopped = true;
                None
            }
        }
    }

    /// `%s` (`%<width>s` with `width`), assigned or not (`%*s`): white space skipped, then the bytes
    /// up to the next white space, the end, or `width` bytes.
    fn word(&mut self, errno: &mut i32, width: usize, assign: bool) -> Option<Vec<u8>> {
        if !self.begin(errno) {
            return None;
        }
        let mut w = Vec::new();
        while w.len() < width {
            match self.st.peek() {
                Some(c) if !is_space(c) => {
                    w.push(c);
                    self.st.bump();
                }
                _ => break,
            }
        }
        self.finish(Scan::Value(w, false), errno, assign)
    }

    fn f32(&mut self, errno: &mut i32) -> Option<f32> {
        if !self.begin(errno) {
            return None;
        }
        let r = cscan::scan_f32(&mut self.st);
        self.finish(r, errno, true)
    }

    fn i32(&mut self, errno: &mut i32) -> Option<i32> {
        if !self.begin(errno) {
            return None;
        }
        let r = cscan::scan_i32(&mut self.st);
        self.finish(r, errno, true)
    }

    /// A white-space directive at the end of the format (one between items is the next
    /// conversion's skip).
    fn end_space(&mut self, errno: &mut i32) {
        if self.stopped {
            return;
        }
        cscan::skip_space(&mut self.st);
        if self.st.i >= self.st.s.len() {
            self.meet(errno, false);
        }
    }

    /// `sscanf`'s value.
    fn result(&self) -> i32 {
        if self.eof { -1 } else { self.n }
    }
}

/// The words a geometry line's `sscanf` calls start with (`%s %s`): the key into a 64-byte buffer
/// and the `=` into a 2-byte buffer just before it (FreeSurfer 8.2.0's official build: GCC 8.4.0
/// puts the two side by side, then the 1,024-byte buffer of the file name; the measured layout is
/// in docs/findings/freesurfer-volume.md). A longer key runs into the name's buffer, which is
/// always written before it is read (FS-285: no effect on any result); a second word of 2 bytes or
/// more runs into the key's buffer (FS-286), so FreeSurfer then compares the second word's bytes
/// from its third on (empty for a word of 2 bytes) with the keys: `reproduce` does that, the
/// default keeps the first word. Both say so.
struct Words<'a> {
    sc: Scanf<'a>,
    key: Option<Vec<u8>>,
}

/// `first`: the line's first call, `%s %s %*s` (the third word read and dropped).
fn words<'a>(g: &mut Globals, line: &'a [u8], noted: &mut [bool; 2], first: bool) -> Words<'a> {
    let mut sc = Scanf::new(line);
    let mut key = sc.word(&mut g.errno, usize::MAX, true);
    if key.as_ref().is_some_and(|k| k.len() >= 64) && !noted[0] {
        noted[0] = true;
        g.notices.add(
            &FS_285,
            "a volume-geometry line whose first word has 64 bytes or more",
        );
    }
    if key.is_some() {
        let eq = sc.word(&mut g.errno, usize::MAX, true);
        if let Some(e) = eq.as_ref().filter(|e| e.len() >= 2) {
            if !noted[1] {
                noted[1] = true;
                g.notices.add(
                    &FS_286,
                    "a volume-geometry line whose second word has 2 bytes or more",
                );
            }
            if g.notices.reproduce() {
                key = Some(e[2..].to_vec());
            }
        }
        if first && eq.is_some() {
            sc.word(&mut g.errno, usize::MAX, false);
        }
    }
    Words { sc, key }
}

/// The keys of the volume-geometry block, in the order `readVolGeom` tests them.
const KEYS: [&[u8]; 8] = [
    b"valid",
    b"filename",
    b"volume",
    b"voxelsize",
    b"xras",
    b"yras",
    b"zras",
    b"cras",
];

/// `readVolGeom(fp, vg)`: reads lines (`fgets` into a buffer of `STRLEN + 16` bytes: a longer line
/// is read in pieces) until eight of them were recognised or the file ends; a line whose first
/// byte is NUL ends the reading. A line is recognised by its first word (`valid`, `filename`,
/// `volume`, `voxelsize`, `xras`, `yras`, `zras`, `cras`), whatever its second word, and gives its
/// numbers by `sscanf`'s rules (`%d`, `%f`; a number that does not convert, and the ones after it,
/// leave the field as it was); `filename` takes the third word (at most 1,023 bytes) when there is
/// one. Each recognised line counts, repeated ones included. When the reading stopped with a line
/// read but not used (the ninth, or the NUL line), the stream goes back to the end of the last line
/// used (`ftell`, `fseek`; not when that position is 0 or unknown). Without a `valid` line it prints
/// `INFO: volume info was not present.` and gives the geometry `initVolGeom`'s values.
///
/// A line with no word (blank or white space only) reuses the previous line's first word in
/// FreeSurfer (its buffer keeps it), so it counts as a repeat of the previous key and takes one of
/// the eight places (FS-287: by default such a line is skipped and not counted; `reproduce` counts
/// it); before any word that buffer holds memory contents (FS-288: no key here, in both modes).
pub fn read_vol_geom(g: &mut Globals, err: &mut dyn Write, fp: &mut CFile, vg: &mut VolGeom) {
    let mut vg_read = false;
    let mut counter = 0;
    let mut pos: i64 = 0;
    let mut stale: Option<Vec<u8>> = None;
    let mut noted = [false; 2];
    // `p`: whether the last `fgets` gave a line
    let mut p;
    loop {
        let raw = fp.fgets(STRLEN + 16);
        p = raw.is_some();
        let Some(raw) = raw else {
            break;
        };
        if counter >= 8 {
            break;
        }
        let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
        if end == 0 {
            break;
        }
        let line = &raw[..end];
        let mut w = words(g, line, &mut noted, true);
        let key = match w.key.take() {
            Some(k) => {
                stale = Some(k.clone());
                Some(k)
            }
            None => match &stale {
                Some(k) if KEYS.contains(&k.as_slice()) => {
                    g.notices.add(
                        &FS_287,
                        "a blank line in a volume-geometry block counted as a repeat of the line before",
                    );
                    if g.notices.reproduce() {
                        Some(k.clone())
                    } else {
                        None
                    }
                }
                Some(_) => None,
                None => {
                    g.notices.add(
                        &FS_288,
                        "a blank line before any word of a volume-geometry block",
                    );
                    None
                }
            },
        };
        if let Some(key) = key
            && let Some(k) = KEYS.iter().position(|&x| x == key.as_slice())
        {
            scan_value(g, line, k, vg, &mut noted);
            if k == 0 {
                vg_read = true;
            }
            counter += 1;
        }
        pos = fp.tell();
    }
    if p && pos > 0 {
        let mut errno = g.errno;
        fp.fseek(&mut errno, pos, 0);
        g.errno = errno;
    }
    if !vg_read {
        let _ = err.write_all(b"INFO: volume info was not present.\n");
        init_vol_geom(vg);
    }
}

/// The value `sscanf` of a recognised line (key number `k`) gives: the line is scanned again
/// from its start (`%s %s`, the item's conversions, the format's final white space).
fn scan_value(g: &mut Globals, line: &[u8], k: usize, vg: &mut VolGeom, noted: &mut [bool; 2]) {
    let mut w = words(g, line, noted, false);
    let sc = &mut w.sc;
    let errno = &mut g.errno;
    match k {
        0 => {
            if let Some(v) = sc.i32(errno) {
                vg.valid = v;
            }
        }
        1 => {
            let buf = sc.word(errno, STRLEN - 1, true);
            sc.word(errno, usize::MAX, false);
            if sc.result() >= 3 {
                vg.fname.strcpy_bytes(&buf.expect("the third word"));
            }
        }
        2 => {
            for f in [&mut vg.width, &mut vg.height, &mut vg.depth] {
                match sc.i32(errno) {
                    Some(v) => *f = v,
                    None => break,
                }
            }
        }
        _ => {
            let fields = match k {
                3 => [&mut vg.xsize, &mut vg.ysize, &mut vg.zsize],
                4 => [&mut vg.x_r, &mut vg.x_a, &mut vg.x_s],
                5 => [&mut vg.y_r, &mut vg.y_a, &mut vg.y_s],
                6 => [&mut vg.z_r, &mut vg.z_a, &mut vg.z_s],
                _ => [&mut vg.c_r, &mut vg.c_a, &mut vg.c_s],
            };
            for f in fields {
                match sc.f32(errno) {
                    Some(v) => *f = v,
                    None => break,
                }
            }
        }
    }
    sc.end_space(errno);
}

/// `vg_i_to_r(vg)`: the scanner voxel-to-RAS matrix (`extract_i_to_r`).
pub fn vg_i_to_r(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Matrix, Fatal> {
    extract_i_to_r(g, out, err, vg)
}

/// `vg_r_to_i(vg)`: its inverse (`extract_r_to_i`; `None` when the inverse fails).
pub fn vg_r_to_i(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Option<Matrix>, Fatal> {
    extract_r_to_i(g, out, err, vg)
}

/// `TkrVox2RASfromVolGeom(vg)`: the tkregister voxel-to-RAS matrix (`MRIxfmCRS2XYZtkreg`).
pub fn tkr_vox2ras_from_vol_geom(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Matrix, Fatal> {
    xfm_crs2xyz_tkreg(g, out, err, vg)
}

/// `vg_isNotEqualThresh(vg1, vg2, thresh)`: 0 when the geometries agree, otherwise the first field
/// that differs (1 `valid`, 2-4 the extent, 5-7 the voxel sizes, 8-16 the direction cosines, 17-19
/// the centre; a `float` field differs when the absolute value of its `float` difference is not
/// below `thresh`, so a NaN always differs). The shears are not compared.
pub fn vg_is_not_equal_thresh(vg1: &VolGeom, vg2: &VolGeom, thresh: f64) -> i32 {
    if vg1.valid != vg2.valid {
        return 1;
    }
    if vg1.width != vg2.width {
        return 2;
    }
    if vg1.height != vg2.height {
        return 3;
    }
    if vg1.depth != vg2.depth {
        return 4;
    }
    let fields = |v: &VolGeom| {
        [
            v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a,
            v.z_s, v.c_r, v.c_a, v.c_s,
        ]
    };
    for (k, (a, b)) in fields(vg1).into_iter().zip(fields(vg2)).enumerate() {
        // FZEROTHR: `fabs(d) < thresh`, false for NaN
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        let differs = !(f64::from((a - b).abs()) < thresh);
        if differs {
            return k as i32 + 5;
        }
    }
    0
}

/// `vg_isEqual(vg1, vg2)`: 1 when [`vg_is_not_equal_thresh`] at `vg_isEqual_Threshold`
/// (`g.vg_is_equal_threshold`) gives 0, otherwise 0.
pub fn vg_is_equal(g: &Globals, vg1: &VolGeom, vg2: &VolGeom) -> i32 {
    i32::from(vg_is_not_equal_thresh(vg1, vg2, g.vg_is_equal_threshold) == 0)
}
