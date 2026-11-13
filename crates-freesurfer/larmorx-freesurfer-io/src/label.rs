// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Labels (`utils/label.cpp`, `include/label.h`; item 3.1-33): the `LABEL` structure and the
//! functions that need no surface or volume header: `LabelAlloc`, `LabelRealloc`, `LabelFree`,
//! `LabelCopy`, `LabelReadFrom`, `LabelRead` (with `labelLoadTransform`), `LabelWriteInto`,
//! `LabelWrite`, `LabelRemoveDuplicates`, `LabelHasVertex`.
//!
//! The functions that convert a label's coordinates with a volume's header (`LabelToScannerRAS`,
//! `LabelToSurfaceRAS`, `LabelToVoxel`, `LabelVoxelToSurfaceRAS`) need fs.volume's matrices (a
//! crate above this one) and the ones that walk a surface need fs.surf's: they come with those
//! crates. The pointers a label carries to a surface, its hash and a template volume are the
//! caller's handles here ([`Label::mris`], [`Label::mht`], [`Label::mri_template`]).
//!
//! D13 (larmorx_core::defects): FS-423 (a space text of 100 bytes or more), FS-424 (a count of 0
//! with point lines), FS-425 (a count larger than the point lines), FS-426 (counts beyond the
//! machine's memory), FS-427 (names longer than the 1,024-byte buffers), FS-428 (a write error
//! closes the file twice), FS-429 (a copy into a smaller label).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_423, FS_424, FS_425, FS_426, FS_427, FS_428, FS_429};
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::cstd::{self, fmt_f};
use larmorx_freesurfer_core::diag::DIAG_SHOW;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NO_FILE, ERROR_NOMEMORY, Fatal, NO_ERROR,
};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::utils;
use larmorx_freesurfer_mri::mri::{
    Transform4, get_inverse_linear_transform_ptr, get_linear_transform_ptr,
};
use larmorx_io::xfm::GeneralTransform;

use crate::cfile::CFile;
use crate::fio;
use crate::minc;

/// `STRLEN` (`const.h`): the label's name buffers and the line buffer.
pub const STRLEN: usize = 1024;

/// The coordinate-space field's size (`char space[100]`).
pub const SPACE_LEN: usize = 100;

/// `LABEL_COORDS_NONE` (`FS_COORDS_UNKNOWN`).
pub const LABEL_COORDS_NONE: i32 = 0;
/// `LABEL_COORDS_TKREG_RAS` (`FS_COORDS_TKREG_RAS`, tkregister's "surface" RAS).
pub const LABEL_COORDS_TKREG_RAS: i32 = 1;
/// `LABEL_COORDS_SCANNER_RAS` (`FS_COORDS_SCANNER_RAS`).
pub const LABEL_COORDS_SCANNER_RAS: i32 = 2;
/// `LABEL_COORDS_VOXEL` (`FS_COORDS_VOXEL`).
pub const LABEL_COORDS_VOXEL: i32 = 3;

/// `sizeof(LABEL_VERTEX)` on x86-64 (`int vno; float x, y, z; int xv, yv, zv; unsigned char
/// deleted; float stat;`: 36 bytes), as FreeSurfer's allocation messages print it.
pub const SIZEOF_LV: u64 = 36;

/// The largest point count larmorx allocates (FS-426: beyond it FreeSurfer's outcome depends on
/// the machine's memory): 2^26 points (2.4 GB in FreeSurfer).
pub const MAX_POINTS: i64 = 1 << 26;

/// `LABEL_VERTEX` (`LV`): one point of a label.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LabelVertex {
    /// The surface vertex (-1: none; 0 is a vertex).
    pub vno: i32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// Voxel coordinates into the label's template volume.
    pub xv: i32,
    pub yv: i32,
    pub zv: i32,
    pub deleted: u8,
    pub stat: f32,
}

/// `LABEL`: a list of surface vertices and/or points.
#[derive(Clone, Debug, Default)]
pub struct Label {
    /// The places allocated.
    pub max_points: i32,
    /// One of the `LABEL_COORDS_*` constants.
    pub coords: i32,
    /// The points in the label.
    pub n_points: i32,
    /// The label's file name (a C string's bytes).
    pub name: Vec<u8>,
    /// The subject's name (a C string's bytes).
    pub subject_name: Vec<u8>,
    /// The points: `max_points` places, as FreeSurfer allocates them (the places after
    /// `n_points` keep what the allocation put there).
    pub lv: Vec<LabelVertex>,
    /// The subject's Talairach transform as read (`None`: the zeroed `General_transform` of a
    /// label without one).
    pub transform: Option<GeneralTransform>,
    /// `linear_transform` (`None`: `NULL`).
    pub linear_transform: Option<Transform4>,
    /// `inverse_linear_transform` (`None`: `NULL`).
    pub inverse_linear_transform: Option<Transform4>,
    /// The coordinate-space description (a C string's bytes; FreeSurfer's field holds 99).
    pub space: Vec<u8>,
    pub avg_stat: f64,
    /// `vertex_label_ind` (`None`: `NULL`).
    pub vertex_label_ind: Option<Vec<i32>>,
    /// The caller's template volume (FreeSurfer's pointer; `None`: `NULL`).
    pub mri_template: Option<usize>,
    /// The caller's surface hash (`MHT *`; `None`: `NULL`).
    pub mht: Option<usize>,
    /// The caller's surface (`MRIS *`; `None`: `NULL`).
    pub mris: Option<usize>,
}

fn c_str(s: &[u8]) -> &[u8] {
    match s.iter().position(|&b| b == 0) {
        Some(n) => &s[..n],
        None => s,
    }
}

fn strstr(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// The environment's value of `name` (`getenv`) as bytes.
fn getenv(env: &Environment, name: &str) -> Option<Vec<u8>> {
    env.get(name)
        .map(|v| v.to_string_lossy().into_owned().into_bytes())
}

fn no_subjects_dir(g: &mut Globals, err: &mut dyn Write) -> Result<(), Fatal> {
    let m = format!(
        "{}: no subject's directory specified in environment (SUBJECTS_DIR)",
        g.progname
    );
    log::error_exit(g, err, ERROR_BADPARM, &m)
}

/// FS-427: a name copied with `strcpy` into a 1,024-byte buffer.
fn name_fits(g: &mut Globals, what: &str, len: usize) {
    if len >= STRLEN {
        g.notices.add(
            &FS_427,
            format!("{what} of {len} bytes kept whole (FreeSurfer's buffer holds 1,023)"),
        );
    }
}

/// FS-426: whether `n` places are beyond what larmorx allocates (noted).
fn too_many(g: &mut Globals, what: &str, n: i64) -> bool {
    if n <= MAX_POINTS {
        return false;
    }
    g.notices.add(
        &FS_426,
        format!(
            "{what} of {n} points (FreeSurfer's outcome depends on the machine's memory): refused"
        ),
    );
    true
}

/// The number `%d` prints for `sizeof(LV) * n` (a `size_t` passed for an `int`: its low 32 bits).
fn lv_bytes_as_int(n: i32) -> i32 {
    SIZEOF_LV.wrapping_mul(i64::from(n) as u64) as u32 as i32
}

/// `calloc(n, sizeof(LV))` of `n` places: the zeroed places, or `None` where it fails (a
/// negative count, which `calloc` sees as a huge one; FS-426 above [`MAX_POINTS`]).
fn calloc_lv(g: &mut Globals, what: &str, n: i32) -> Option<Vec<LabelVertex>> {
    if n < 0 || too_many(g, what, i64::from(n)) {
        g.errno = 12;
        return None;
    }
    Some(vec![LabelVertex::default(); n as usize])
}

/// `LabelAlloc(max_points, subject_name, label_name)`: a label of `max_points` places, each with
/// vertex -1, no points, space `TkReg`, tkregister coordinates. With a subject name,
/// `SUBJECTS_DIR` must be set (`<Progname>: no subject's directory specified in environment
/// (SUBJECTS_DIR)`, exit `ERROR_BADPARM`; its value is not used) and the name is the subject's;
/// without one, the label's name is `label_name` (cut at 1,023 bytes). Places that cannot be
/// allocated end the program (`<Progname>: LabelAlloc(<label_name>) could not allocate 0-sized
/// vector`, `ERROR_NOMEMORY`).
pub fn label_alloc(
    g: &mut Globals,
    env: &Environment,
    err: &mut dyn Write,
    max_points: i32,
    subject_name: Option<&[u8]>,
    label_name: Option<&[u8]>,
) -> Result<Label, Fatal> {
    let mut area = Label::default();
    if let Some(s) = subject_name {
        match getenv(env, "SUBJECTS_DIR") {
            Some(sdir) => name_fits(g, "LabelAlloc: SUBJECTS_DIR", sdir.len()),
            None => no_subjects_dir(g, err)?,
        }
        let s = c_str(s);
        name_fits(g, "LabelAlloc: a subject name", s.len());
        area.subject_name = s.to_vec();
    } else if let Some(l) = label_name {
        let l = c_str(l);
        area.name = l[..l.len().min(STRLEN - 1)].to_vec();
    }
    area.n_points = 0;
    area.max_points = max_points;
    let Some(mut lv) = calloc_lv(g, "LabelAlloc", max_points) else {
        let m = format!(
            "{}: LabelAlloc({}) could not allocate {}-sized vector",
            g.progname,
            lossy(c_str(label_name.unwrap_or(b""))),
            lv_bytes_as_int(area.n_points)
        );
        log::error_exit(g, err, ERROR_NOMEMORY, &m)?;
        return Ok(area);
    };
    for v in &mut lv {
        v.vno = -1;
    }
    area.lv = lv;
    area.space = b"TkReg".to_vec();
    area.coords = LABEL_COORDS_TKREG_RAS;
    Ok(area)
}

/// `LabelRealloc(lb, max_points)`: a new label of `max_points` places for `None`
/// ([`label_alloc`] without names); the label unchanged when `max_points` is not larger, or
/// when the places cannot be allocated (FS-426 beyond the machine's memory); else the label with
/// `max_points` places, the new ones zeroed (vertex 0).
pub fn label_realloc(
    g: &mut Globals,
    env: &Environment,
    err: &mut dyn Write,
    lb: Option<Label>,
    max_points: i32,
) -> Result<Label, Fatal> {
    let Some(mut lb) = lb else {
        return label_alloc(g, env, err, max_points, None, None);
    };
    if max_points <= lb.max_points {
        return Ok(lb);
    }
    if too_many(g, "LabelRealloc", i64::from(max_points)) {
        return Ok(lb);
    }
    lb.lv.resize(max_points as usize, LabelVertex::default());
    lb.max_points = max_points;
    Ok(lb)
}

/// `LabelFree(&area)`: the label freed, `area` `None`; `NO_ERROR`.
pub fn label_free(area: &mut Option<Label>) -> i32 {
    *area = None;
    NO_ERROR
}

/// `LabelCopy(asrc, adst)`: `asrc` copied into `adst`, or (`None`) into a new label of `asrc`'s
/// places ([`label_alloc`] with `asrc`'s subject name, so `SUBJECTS_DIR` must be set even for a
/// label without a subject). The copy takes the surface, template and hash handles, the
/// coordinates, the space, a copy of the vertex index table (of `asrc`'s `max_points`), the point
/// count, the names and the first `n_points` points; the destination keeps its own places after
/// them, its transforms and its average statistic. FS-429: a given destination with fewer places
/// than the points is made larger (FreeSurfer writes past it).
pub fn label_copy(
    g: &mut Globals,
    env: &Environment,
    err: &mut dyn Write,
    asrc: &Label,
    adst: Option<Label>,
) -> Result<Label, Fatal> {
    let mut adst = match adst {
        Some(a) => a,
        None => label_alloc(
            g,
            env,
            err,
            asrc.max_points,
            Some(&asrc.subject_name),
            Some(&asrc.name),
        )?,
    };
    adst.mris = asrc.mris;
    adst.mri_template = asrc.mri_template;
    adst.mht = asrc.mht;
    adst.coords = asrc.coords;
    adst.space = asrc.space.clone();
    if let Some(ind) = &asrc.vertex_label_ind {
        let n = asrc.max_points.max(0) as usize;
        let mut v = vec![0i32; n];
        let k = n.min(ind.len());
        v[..k].copy_from_slice(&ind[..k]);
        adst.vertex_label_ind = Some(v);
    }
    adst.n_points = asrc.n_points;
    adst.name = asrc.name.clone();
    adst.subject_name = asrc.subject_name.clone();
    let n = asrc.n_points.max(0) as usize;
    if adst.lv.len() < n {
        g.notices.add(
            &FS_429,
            format!(
                "LabelCopy of {n} points into a label of {} places: the places made",
                adst.lv.len()
            ),
        );
        adst.lv.resize(n, LabelVertex::default());
    }
    adst.lv[..n].copy_from_slice(&asrc.lv[..n]);
    Ok(adst)
}

fn fgets(g: &mut Globals, fp: &mut CFile, n: usize) -> Option<Vec<u8>> {
    let l = fp.fgets(n);
    if l.is_none()
        && let Some(e) = fp.read_errno()
    {
        g.errno = e;
    }
    l.map(|l| c_str(&l).to_vec())
}

/// `fgetl(line, n, fp)`: the next line read with a buffer of `n` bytes that is not empty or a
/// comment after its leading white space, cut at its first `#`, without its newline.
pub fn fgetl(g: &mut Globals, fp: &mut CFile, n: usize) -> Option<Vec<u8>> {
    loop {
        let line = fgets(g, fp, n)?;
        let start = line
            .iter()
            .position(|&b| !cstd::is_space(b))
            .unwrap_or(line.len());
        let cp = &line[start..];
        if cp.is_empty() || cp[0] == b'#' || cp[0] == b'\n' {
            continue;
        }
        let end = cp.iter().position(|&b| b == b'#').unwrap_or(cp.len());
        let mut cp = cp[..end].to_vec();
        if cp.last() == Some(&b'\n') {
            cp.pop();
        }
        return Some(cp);
    }
}

/// `labelLoadTransform(subject, sdir, &transform)`: `<sdir>/<subject>/mri/transforms/
/// talairach.xfm` read with `input_transform_file` when it exists (`<Progname>: could not load
/// transform file '<name>'` as an error when it does not read); the transform read and its
/// forward matrix.
fn label_load_transform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    subject: &[u8],
    sdir: &[u8],
) -> Result<(Option<GeneralTransform>, Option<Transform4>), Fatal> {
    let mut name = sdir.to_vec();
    name.push(b'/');
    name.extend_from_slice(subject);
    name.extend_from_slice(b"/mri/transforms/talairach.xfm");
    if !utils::file_exists(dir, &lossy(&name)) {
        return Ok((None, None));
    }
    match minc::input_transform_file(g, out, err, dir, &name) {
        Some(t) => {
            let m = get_linear_transform_ptr(err, &t)?;
            Ok((Some(t), Some(m)))
        }
        None => {
            let m = format!(
                "{}: could not load transform file '{}'",
                g.progname,
                lossy(&name)
            );
            log::error_printf(g, err, ERROR_NO_FILE, &m);
            Ok((None, None))
        }
    }
}

/// One point line read with `sscanf(cp, "%d %f %f %f %f", ...)`: the count and the values.
fn scan_point(errno: &mut i32, line: &[u8]) -> (i32, i32, [f32; 4]) {
    let mut b = Bytes::new(line);
    let mut sc = Scanf::new(&mut b);
    let vno = sc.d(errno, true).unwrap_or(0);
    let mut v = [0f32; 4];
    for x in &mut v {
        sc.text(errno, b" ");
        *x = sc.f(errno, true).unwrap_or(0.0);
    }
    (sc.finish(errno), vno, v)
}

/// `LabelReadFrom(subject_name, fp)`: a label read from a stream. Its first line (read whole,
/// up to 1,023 bytes): the text after `vox2ras=` (without the newline) is the space; the
/// coordinates are voxels when the space holds `voxel`, else scanner RAS when the line holds
/// `scanner`, else tkregister RAS. Then (lines read with `fgetl`: comments and blank lines
/// skipped, cut at `#`) the point count (`%d`), and up to that many points `<vertex> <x> <y> <z>
/// <stat>` (`%d %f %f %f %f`). `None` silently for an empty stream; `None` after an error
/// (`ERROR_BADFILE`) for a missing count (`<Progname>: empty label`), a count that does not read
/// (the line on standard output after an empty line, then `<Progname>: could not scan # of lines
/// from label file`), a point line that does not read (`<Progname>: could not parse <k>th line
/// '<line>' in label file`) or no point lines (`<Progname>: no data in label file`). A count
/// that cannot be allocated ends the program (`<Progname>: LabelReadFrom could not allocate
/// <bytes>-sized vector`, `ERROR_NOMEMORY`). With a subject name, `SUBJECTS_DIR` must be set (else
/// the program ends) and the subject's Talairach transform is read (`labelLoadTransform`).
/// FS-423, FS-424, FS-425, FS-426, FS-427.
#[allow(clippy::too_many_arguments)]
pub fn label_read_from(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    subject_name: Option<&[u8]>,
    fp: &mut CFile,
) -> Result<Option<Label>, Fatal> {
    let mut area = Label::default();
    let Some(mut cp) = fgets(g, fp, STRLEN) else {
        return Ok(None);
    };
    if let Some(at) = strstr(&cp, b"vox2ras=") {
        if cp.last() == Some(&b'\n') {
            cp.pop();
        }
        let text = cp[at + 8..].to_vec();
        if text.len() >= SPACE_LEN {
            g.notices.add(
                &FS_423,
                format!(
                    "LabelReadFrom: a space text of {} bytes kept whole (FreeSurfer's field holds 99)",
                    text.len()
                ),
            );
        }
        area.space = text;
    }
    area.coords = if strstr(&area.space, b"voxel").is_some() {
        LABEL_COORDS_VOXEL
    } else if strstr(&cp, b"scanner").is_some() {
        LABEL_COORDS_SCANNER_RAS
    } else {
        LABEL_COORDS_TKREG_RAS
    };
    let Some(cp) = fgetl(g, fp, STRLEN) else {
        let m = format!("{}: empty label", g.progname);
        log::error_printf(g, err, ERROR_BADFILE, &m);
        return Ok(None);
    };
    let mut errno = g.errno;
    let (n, count) = {
        let mut b = Bytes::new(&cp);
        let mut sc = Scanf::new(&mut b);
        let v = sc.d(&mut errno, true);
        (sc.finish(&mut errno), v)
    };
    g.errno = errno;
    if n == 0 {
        let _ = out.write_all(b"\n");
        let _ = out.write_all(&cp);
        let _ = out.write_all(b"\n");
        let m = format!("{}: could not scan # of lines from label file", g.progname);
        log::error_printf(g, err, ERROR_BADFILE, &m);
        return Ok(None);
    }
    area.n_points = count.unwrap_or(0);
    area.max_points = area.n_points;
    let Some(lv) = calloc_lv(g, "LabelReadFrom", area.n_points) else {
        let m = format!(
            "{}: LabelReadFrom could not allocate {}-sized vector",
            g.progname,
            lv_bytes_as_int(area.n_points)
        );
        log::error_exit(g, err, ERROR_NOMEMORY, &m)?;
        return Ok(None);
    };
    area.lv = lv;
    let mut nlines: i32 = 0;
    let mut outside = 0usize;
    while let Some(cp) = fgetl(g, fp, STRLEN) {
        let mut errno = g.errno;
        let (n, vno, [x, y, z, stat]) = scan_point(&mut errno, &cp);
        g.errno = errno;
        if n != 5 {
            let mut m =
                format!("{}: could not parse {}th line '", g.progname, nlines + 1).into_bytes();
            m.extend_from_slice(&cp);
            m.extend_from_slice(b"' in label file");
            log::error_printf_bytes(g, err, ERROR_BADFILE, &m);
            return Ok(None);
        }
        match area.lv.get_mut(nlines as usize) {
            Some(lv) => {
                lv.x = x;
                lv.y = y;
                lv.z = z;
                lv.stat = stat;
                lv.vno = vno;
            }
            None => {
                if outside == 0 {
                    g.notices.add(
                        &FS_424,
                        "LabelReadFrom: a point count of 0 with point lines: the points not stored (FreeSurfer writes them outside the label)",
                    );
                }
                outside += 1;
            }
        }
        nlines += 1;
        if nlines == area.n_points {
            break;
        }
    }
    if nlines == 0 {
        let m = format!("{}: no data in label file", g.progname);
        log::error_printf(g, err, ERROR_BADFILE, &m);
        return Ok(None);
    }
    if nlines < area.n_points {
        g.notices.add(
            &FS_425,
            format!(
                "LabelReadFrom: a point count of {} with {nlines} point lines (FreeSurfer adds points at vertex 0, (0, 0, 0))",
                area.n_points
            ),
        );
        if !g.notices.reproduce() {
            area.n_points = nlines;
        }
    }
    if let Some(subject) = subject_name {
        let Some(sdir) = getenv(env, "SUBJECTS_DIR") else {
            no_subjects_dir(g, err)?;
            return Ok(None);
        };
        name_fits(g, "LabelReadFrom: SUBJECTS_DIR", sdir.len() + 1);
        let subject = c_str(subject);
        area.subject_name = subject[..subject.len().min(STRLEN - 1)].to_vec();
        let (t, m) = label_load_transform(g, out, err, dir, subject, &sdir)?;
        area.linear_transform = m;
        area.inverse_linear_transform = match &t {
            Some(t) => Some(get_inverse_linear_transform_ptr(err, t)?),
            None => None,
        };
        area.transform = t;
    }
    Ok(Some(area))
}

/// `LabelRead(subject_name, label_name)`: the label file read with [`label_read_from`], its name
/// the file's name (cut at 1,023 bytes). With a subject name and a label name without `/`, the
/// file is `$SUBJECTS_DIR/<subject>/label/<name>.label` (the name up to its first `.label`;
/// `SUBJECTS_DIR` must be set); otherwise the name itself when it holds `.label`, else the name
/// with `.label` added. When that file cannot be opened for reading and the name as given can,
/// the name as given. The file is opened with `errno` cleared; when the open sets it, its text on
/// standard error (`perror`); a file that does not open is an error (`<Progname>: could not open
/// label file <name>`, `ERROR_NOFILE`) and `None`.
#[allow(clippy::too_many_arguments)]
pub fn label_read(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    subject_name: Option<&[u8]>,
    label_name: &str,
) -> Result<Option<Label>, Fatal> {
    let lname_bytes = label_name.as_bytes();
    name_fits(g, "LabelRead: a label name", lname_bytes.len());
    let mut fname: Vec<u8>;
    if subject_name.is_some() && !lname_bytes.contains(&b'/') {
        let Some(sdir) = getenv(env, "SUBJECTS_DIR") else {
            no_subjects_dir(g, err)?;
            return Ok(None);
        };
        name_fits(g, "LabelRead: SUBJECTS_DIR", sdir.len());
        let mut lname = lname_bytes.to_vec();
        if let Some(at) = strstr(&lname, b".label") {
            lname.truncate(at);
        }
        fname = sdir;
        fname.push(b'/');
        fname.extend_from_slice(c_str(subject_name.unwrap_or(b"")));
        fname.extend_from_slice(b"/label/");
        fname.extend_from_slice(&lname);
        fname.extend_from_slice(b".label");
    } else if strstr(lname_bytes, b".label").is_none() {
        if let Some(sdir) = getenv(env, "SUBJECTS_DIR") {
            name_fits(g, "LabelRead: SUBJECTS_DIR", sdir.len());
        }
        fname = lname_bytes.to_vec();
        fname.extend_from_slice(b".label");
    } else {
        if let Some(sdir) = getenv(env, "SUBJECTS_DIR") {
            name_fits(g, "LabelRead: SUBJECTS_DIR", sdir.len());
        }
        fname = lname_bytes.to_vec();
    }
    let fname_s = lossy(&fname);
    let fname_s = if fio::fio_file_exists_readable(g, dir, &fname_s) == 0
        && fio::fio_file_exists_readable(g, dir, label_name) != 0
    {
        fname = lname_bytes.to_vec();
        label_name.to_owned()
    } else {
        fname_s
    };
    g.errno = 0;
    let opened = CFile::open(dir, &fname_s, "r");
    if let Err(e) = &opened {
        g.errno = *e;
    }
    if g.errno != 0 {
        let _ = writeln!(err, "{}", log::strerror(g.errno));
    }
    let Ok(mut fp) = opened else {
        let mut m = format!("{}: could not open label file ", g.progname).into_bytes();
        m.extend_from_slice(&fname);
        log::error_printf_bytes(g, err, ERROR_NO_FILE, &m);
        return Ok(None);
    };
    let mut area = label_read_from(g, env, out, err, dir, subject_name, &mut fp)?;
    if let Some(a) = &mut area {
        a.name = fname[..fname.len().min(STRLEN - 1)].to_vec();
    }
    fp.close();
    Ok(area)
}

/// The bytes of a label file: `#!ascii label <name> , from subject <subject> vox2ras=<space>`,
/// the count of points not deleted, and each such point as `%d  %2.3f  %2.3f  %2.3f %10.10f`.
fn label_lines(area: &Label) -> Vec<Vec<u8>> {
    let n = area.n_points.max(0) as usize;
    let pts = &area.lv[..n.min(area.lv.len())];
    let num = pts.iter().filter(|v| v.deleted == 0).count();
    let mut lines = Vec::with_capacity(num + 2);
    let mut h = b"#!ascii label ".to_vec();
    h.extend_from_slice(c_str(&area.name));
    h.extend_from_slice(b" , from subject ");
    h.extend_from_slice(c_str(&area.subject_name));
    h.extend_from_slice(b" vox2ras=");
    h.extend_from_slice(c_str(&area.space));
    h.push(b'\n');
    lines.push(h);
    lines.push(format!("{num}\n").into_bytes());
    for v in pts.iter().filter(|v| v.deleted == 0) {
        lines.push(
            format!(
                "{}  {}  {}  {} {}\n",
                v.vno,
                fmt_f(f64::from(v.x), 2, 3),
                fmt_f(f64::from(v.y), 2, 3),
                fmt_f(f64::from(v.z), 2, 3),
                fmt_f(f64::from(v.stat), 10, 10)
            )
            .into_bytes(),
        );
    }
    lines
}

/// `LabelWriteInto(area, fp)`: the label's lines written to the stream; `NO_ERROR`, or 1 after
/// `ERROR: writing to label file <k>` on standard output when a line does not write (1 the
/// header, 2 the count, 3 a point). FS-428: FreeSurfer then closes the caller's stream, which
/// [`label_write`] closes again; here the caller closes it once.
pub fn label_write_into(g: &mut Globals, out: &mut dyn Write, area: &Label, fp: &mut CFile) -> i32 {
    for (k, line) in label_lines(area).iter().enumerate() {
        if !fp.put_bytes(line) {
            let _ = writeln!(out, "ERROR: writing to label file {}", (k + 1).min(3));
            g.notices.add(
                &FS_428,
                "LabelWriteInto: a write error (FreeSurfer closes the file, then LabelWrite closes it again): closed once",
            );
            return 1;
        }
    }
    NO_ERROR
}

/// `LabelWrite(area, label_name)`: the label written to a file: the name without a final
/// `.label` (any case); with no `/` in it and a subject name in the label,
/// `$SUBJECTS_DIR/<subject>/label/<name>.label` (`SUBJECTS_DIR` must be set); otherwise the name
/// as it now is when it still ends in `.label` (any case), else with `.label` added. A file that
/// does not open is an error (`<Progname>: could not open label file <name>`) and gives
/// `ERROR_NOFILE`; else [`label_write_into`]'s value.
#[allow(clippy::too_many_arguments)]
pub fn label_write(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    area: &Label,
    label_name: &str,
) -> Result<i32, Fatal> {
    let mut lname = label_name.as_bytes().to_vec();
    name_fits(g, "LabelWrite: a label name", lname.len());
    let is_label = |g: &mut Globals, l: &[u8]| -> Option<usize> {
        let dot = l.iter().rposition(|&c| c == b'.')?;
        (utils::stricmp(g, &lossy(&l[dot..]), ".label") == 0).then_some(dot)
    };
    if let Some(dot) = is_label(g, &lname) {
        lname.truncate(dot);
    }
    let fname: Vec<u8> = if !lname.contains(&b'/') && !c_str(&area.subject_name).is_empty() {
        let Some(sdir) = getenv(env, "SUBJECTS_DIR") else {
            no_subjects_dir(g, err)?;
            return Ok(ERROR_NO_FILE);
        };
        name_fits(g, "LabelWrite: SUBJECTS_DIR", sdir.len());
        let mut f = sdir;
        f.push(b'/');
        f.extend_from_slice(c_str(&area.subject_name));
        f.extend_from_slice(b"/label/");
        f.extend_from_slice(&lname);
        f.extend_from_slice(b".label");
        f
    } else if is_label(g, &lname).is_some() {
        lname.clone()
    } else {
        let mut f = lname.clone();
        f.extend_from_slice(b".label");
        f
    };
    let opened = CFile::open(dir, &lossy(&fname), "w");
    if let Err(e) = &opened {
        g.errno = *e;
    }
    let Ok(mut fp) = opened else {
        let mut m = format!("{}: could not open label file ", g.progname).into_bytes();
        m.extend_from_slice(&fname);
        log::error_printf_bytes(g, err, ERROR_NO_FILE, &m);
        return Ok(ERROR_NO_FILE);
    };
    let ret = label_write_into(g, out, area, &mut fp);
    fp.close();
    Ok(ret)
}

/// `FEQUAL(a, b)` of two `float`s: `|a - b| < FLT_EPSILON`, in `float`.
fn fequal(a: f32, b: f32) -> bool {
    (a - b).abs() < f32::EPSILON
}

/// `LabelRemoveDuplicates(area)`: marks as deleted each later point that repeats an earlier
/// point not deleted: the same vertex (both at least 0), or (both without a vertex) coordinates
/// equal within `FLT_EPSILON`. With `DIAG_SHOW`, `<k> duplicate vertices removed from label
/// <name>.` on standard error (`k` counts every mark, a point marked again included).
/// `NO_ERROR`.
pub fn label_remove_duplicates(g: &mut Globals, err: &mut dyn Write, area: &mut Label) -> i32 {
    let n = (area.n_points.max(0) as usize).min(area.lv.len());
    let mut deleted = 0i32;
    for n1 in 0..n {
        let lv1 = area.lv[n1];
        if lv1.deleted != 0 {
            continue;
        }
        for n2 in n1 + 1..n {
            let lv2 = &mut area.lv[n2];
            if lv1.vno >= 0 && lv2.vno >= 0 && lv1.vno == lv2.vno {
                deleted = deleted.wrapping_add(1);
                lv2.deleted = 1;
            } else if lv1.vno < 0
                && lv2.vno < 0
                && fequal(lv1.x, lv2.x)
                && fequal(lv1.y, lv2.y)
                && fequal(lv1.z, lv2.z)
            {
                deleted = deleted.wrapping_add(1);
                lv2.deleted = 1;
            }
        }
    }
    if g.gdiag & DIAG_SHOW != 0 {
        let mut m = format!("{deleted} duplicate vertices removed from label ").into_bytes();
        m.extend_from_slice(c_str(&area.name));
        m.extend_from_slice(b".\n");
        let _ = err.write_all(&m);
    }
    NO_ERROR
}

/// `LabelHasVertex(vtxno, lb)`: the first point of vertex `vtxno`, or -1.
pub fn label_has_vertex(vtxno: i32, lb: &Label) -> i32 {
    let n = (lb.n_points.max(0) as usize).min(lb.lv.len());
    lb.lv[..n]
        .iter()
        .position(|v| v.vno == vtxno)
        .map_or(-1, |i| i as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn writes_and_reads_a_label() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        g.progname = "t".into();
        let env = Environment::default();
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let dir = tempfile::tempdir().unwrap();
        let mut a = label_alloc(&mut g, &env, &mut e, 3, None, Some(b"x")).unwrap();
        assert_eq!(a.lv[2].vno, -1);
        a.n_points = 2;
        a.lv[0] = LabelVertex {
            vno: 7,
            x: 1.5,
            y: -2.25,
            z: 0.0,
            stat: 0.125,
            ..Default::default()
        };
        a.lv[1] = LabelVertex {
            vno: -1,
            x: 1e-3,
            deleted: 1,
            ..Default::default()
        };
        let r = label_write(
            &mut g,
            &env,
            &mut o,
            &mut e,
            Some(dir.path()),
            &a,
            "a.LABEL",
        )
        .unwrap();
        assert_eq!(r, 0);
        let bytes = std::fs::read(dir.path().join("a.label")).unwrap();
        assert_eq!(
            bytes,
            b"#!ascii label x , from subject  vox2ras=TkReg\n1\n7  1.500  -2.250  0.000 0.1250000000\n"
        );
        let b = label_read(&mut g, &env, &mut o, &mut e, Some(dir.path()), None, "a")
            .unwrap()
            .unwrap();
        assert_eq!(
            (b.n_points, b.max_points, b.coords),
            (1, 1, LABEL_COORDS_TKREG_RAS)
        );
        assert_eq!(b.lv[0].vno, 7);
        assert_eq!(b.space, b"TkReg");
        assert_eq!(b.name, b"a.label");
        assert_eq!(label_has_vertex(7, &b), 0);
        assert_eq!(label_has_vertex(8, &b), -1);
    }

    #[test]
    fn short_counts_follow_the_mode() {
        for (mode, n) in [(UpstreamDefects::Fix, 1), (UpstreamDefects::Reproduce, 3)] {
            let mut g = Globals::new(mode);
            let env = Environment::default();
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let mut fp = CFile::from_bytes(b"# scanner\n3\n5 1 2 3 4\n".to_vec());
            let a = label_read_from(&mut g, &env, &mut o, &mut e, None, None, &mut fp)
                .unwrap()
                .unwrap();
            assert_eq!(a.n_points, n);
            assert_eq!(a.coords, LABEL_COORDS_SCANNER_RAS);
            assert_eq!(a.lv[2], LabelVertex::default());
        }
    }
}
