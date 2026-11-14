// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Writing volumes (`utils/mriio.cpp`, item 3.1-30): `mghWrite` (MGH and MGZ) and `MRITAGwrite`
//! (the tags after an MGH volume's data, and in FreeSurfer's NIfTI header extension), wired into
//! [`crate::mriio::mri_write_type`].
//!
//! An MGH file: version, width, height, depth, frames, type and degrees of freedom (big-endian
//! `int`s), the RAS flag (`short`: 1 or -1), the voxel sizes, direction cosines and centre (15
//! big-endian `float`s), 194 zero bytes (284 bytes in all); the voxels frame by frame, slice by
//! slice, row by row, big-endian; TR, the flip angle, TE, TI and the field of view (`float`s);
//! the tags. `.mgz` (and any name holding `mgh.gz`) is the same stream through zlib's gzip
//! writer (larmorx's compressed bytes are zlib-rs's, `larmorx_io::znz`; the stream they inflate to
//! is FreeSurfer's).
//!
//! Defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-io.md`, Part 13):
//! - **FS-439** (resource leak): an unsupported type, or a byte row that fails to write, ends
//!   with an error and leaves the file open: by default the file is closed (its header written),
//!   with `reproduce` left as FreeSurfer leaves it (an `.mgz` empty), noted.
//! - **FS-440** (a failed write reported as success): by default a write that fails, or a close
//!   that fails, gives `mghWrite: could not write <name>` and `ERROR_BADFILE`; `reproduce` 0.
//! - **FS-441** (memory): a volume without frames: its first frame's label and threshold are
//!   memory contents; both modes write no frames tag, noted.
//! - **FS-442** (wrong header): one chosen frame written under the volume's frame count: by
//!   default the header says 1 frame; `reproduce` the volume's count.
//! - **FS-443** (the volume changed): with `FS_MGZIO_USEVOXELBUFWRITE` the volume's own voxels are
//!   left byte-swapped: by default they are not touched; `reproduce` swaps them.
//! - **FS-444** (memory): one chosen frame outside the volume: by default an error before the
//!   file is opened; `reproduce` writes zeros for it, noted.
//! - **FS-445** (crash): with `FS_MGZIO_USEVOXELBUFWRITE`, a volume without slices: both modes
//!   write no voxels (there are none), noted.
//! - **FS-446** (memory): a volume made without voxels has its contiguous-buffer flag never
//!   written: with `FS_MGZIO_USEVOXELBUFWRITE` both modes take it as unset, noted.
//! - **FS-58**: a volume without voxels (FreeSurfer crashes): the program ends with a message.

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::{
    FS_55, FS_58, FS_439, FS_440, FS_441, FS_442, FS_443, FS_444, FS_445, FS_446,
};
use larmorx_freesurfer_core::diag::DIAG_INFO;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal, NO_ERROR,
};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::utils;
use larmorx_freesurfer_mri::mri::{
    self, MGH_VERSION, MGZ_INTENT_UNKNOWN, MRI_FLOAT, MRI_FLOAT_COMPLEX, MRI_INT, MRI_SHORT,
    MRI_UCHAR, MRI_USHRT, Mri, WARPFIELD_DTFMT_UNKNOWN,
};
use larmorx_io::znz::ZnzFile;

use crate::fio;
use crate::fstagsio::{self, FsTagsIo};
use crate::tags::{
    TAG_AUTO_ALIGN, TAG_CMDLINE, TAG_FIELDSTRENGTH, TAG_GCAMORPH_AFFINE, TAG_MGH_XFORM,
    TAG_ORIG_RAS2VOX, TAG_PEDIR,
};

/// `UNUSED_SPACE_SIZE - USED_SPACE_SIZE - sizeof(short)`: the zero bytes after the header's
/// geometry.
const UNUSED_AFTER_GEOMETRY: usize = 256 - 60 - 2;

fn getenv(env: &Environment, name: &str) -> bool {
    env.get(name).is_some()
}

/// `errno = 0; ErrorReturn(code, (code, message))`.
fn fail_code(g: &mut Globals, err: &mut dyn Write, code: i32, message: &str) -> i32 {
    g.errno = 0;
    log::error_printf(g, err, code, message);
    code
}

/// The bytes of a C string: up to its first NUL.
fn c_bytes(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// `znzwrite(b, 1, b.len(), fp)`, recording in `short` (FS-440) a write that gave fewer bytes.
fn put(fp: &mut ZnzFile, short: &mut bool, b: &[u8]) -> i64 {
    let r = fio::fs_znzwrite(fp, b, 1, b.len());
    if r != b.len() as i64 {
        *short = true;
    }
    r
}

/// The `n` bytes of the stored voxel row `row` of `m` (the row's index among all rows of all
/// frames in the contiguous buffer), zeros past the buffer.
fn row_bytes(m: &Mri, row: usize, n: usize) -> Vec<u8> {
    let chunk = m.chunk.as_deref().unwrap_or(&[]);
    let o = row
        .wrapping_mul(m.vox_per_row as usize)
        .wrapping_mul(m.bytes_per_vox);
    let end = o.saturating_add(n).min(chunk.len());
    let mut v = chunk[o.min(end)..end].to_vec();
    v.resize(n, 0);
    v
}

/// `mghWrite(mri, fname, frame, intent)`: the volume written as an MGH file (`.mgh`; `.mgz` and
/// names holding `mgh.gz` compressed): all frames for `frame` below 0, else that frame alone.
///
/// A name without `.mgh` or `.mgz` after its last dot (any case), and not holding `mgh.gz`, is
/// refused (`mghWrite: filename '<name>' needs to have an extension of .mgh or .mgz`,
/// `ERROR_BADPARM`); a file that does not open gives `mghWrite(<name>, <frame>): could not open
/// file` (`ERROR_BADPARM`). An `intent` other than -1 becomes the volume's intent and its version
/// `(intent & 0xffff) << 8 | 1` (the volume keeps both). Types: uchar, short, unsigned short, int,
/// float, float complex (real and imaginary parts); another type, when there is a row to write,
/// gives `mghWrite: unsupported type <t>` (`ERROR_UNSUPPORTED`, FS-439); a byte row that does not
/// write gives `mghWrite: could not write <n> bytes to <name>` (`ERROR_BADFILE`, FS-439). With
/// `FS_MGZIO_USEVOXELBUFWRITE` set the voxels are written slice by slice from the volume's buffer,
/// every type byte-swapped by its size (2 bytes; 4 bytes for sizes 4 and 8) (FS-443); with
/// `FS_MGZIO_TIMING` the time taken is printed; with `DIAG_INFO` the intent, the file positions
/// and the scan parameters. Then TR, the flip angle (as a `float`), TE, TI, the field of view and
/// the tags ([`mri_tag_write`]). `NO_ERROR` (0).
#[allow(clippy::too_many_arguments)]
pub fn mgh_write(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    m: &mut Mri,
    fname: &str,
    frame: i32,
    intent: i32,
) -> Result<i32, Fatal> {
    let (start_frame, end_frame) = if frame >= 0 {
        (frame, frame)
    } else {
        (0, m.nframes.wrapping_sub(1))
    };
    let mut gzipped = false;
    let mut valid_ext = false;
    if let Some(i) = fname.rfind('.') {
        let ext = &fname[i + 1..];
        if utils::stricmp(g, ext, "mgz") == 0 || fname.contains("mgh.gz") {
            gzipped = true;
            valid_ext = true;
        } else if utils::stricmp(g, ext, "mgh") == 0 {
            valid_ext = true;
        }
    }
    if !valid_ext {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADPARM,
            &format!("mghWrite: filename '{fname}' needs to have an extension of .mgh or .mgz"),
        ));
    }
    let outside_frame = frame >= 0 && frame >= m.nframes;
    if outside_frame {
        g.notices.add(
            &FS_444,
            format!(
                "mghWrite of frame {frame} of a volume of {} frames (FreeSurfer reads memory outside the volume)",
                m.nframes
            ),
        );
        if !g.notices.reproduce() {
            return Ok(fail_code(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "mghWrite({fname}, {frame}): the volume has {} frames",
                    m.nframes
                ),
            ));
        }
    }
    let path = utils::resolve(dir, fname);
    let Some(mut file) = ZnzFile::open(path.as_os_str().as_encoded_bytes(), b"wb", gzipped) else {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADPARM,
            &format!("mghWrite({fname}, {frame}): could not open file"),
        ));
    };
    let fp = &mut file;
    let (width, height, depth) = (m.vg.width, m.vg.height, m.vg.depth);
    if intent != MGZ_INTENT_UNKNOWN {
        m.intent = intent;
        m.version = ((intent & 0xffff) << 8) | MGH_VERSION;
    }
    let mut nframes_written = m.nframes;
    if frame >= 0 && m.nframes != 1 {
        g.notices.add(
            &FS_442,
            format!(
                "mghWrite of one frame of a volume of {} frames: the header's frame count",
                m.nframes
            ),
        );
        if !g.notices.reproduce() {
            nframes_written = 1;
        }
    }
    let info = g.gdiag & DIAG_INFO != 0;
    let mut short = false;
    let mut head = Vec::with_capacity(284);
    for v in [
        m.version,
        width,
        height,
        depth,
        nframes_written,
        m.mtype,
        m.dof,
    ] {
        head.extend_from_slice(&v.to_be_bytes());
    }
    let flag: i16 = if m.vg.ras_good_flag != 0 { 1 } else { -1 };
    head.extend_from_slice(&flag.to_be_bytes());
    let vg = &m.vg;
    for v in [
        vg.xsize, vg.ysize, vg.zsize, vg.x_r, vg.x_a, vg.x_s, vg.y_r, vg.y_a, vg.y_s, vg.z_r,
        vg.z_a, vg.z_s, vg.c_r, vg.c_a, vg.c_s,
    ] {
        head.extend_from_slice(&v.to_be_bytes());
    }
    head.resize(head.len() + UNUSED_AFTER_GEOMETRY, 0);
    // one write for FreeSurfer's 23 (their counts unchecked)
    put(fp, &mut short, &head);
    if info {
        let _ = writeln!(
            out,
            "[DEBUG] mghWrite() intent (user) = {intent}, mri->intent (new) = {}, mri->version (new) = {}",
            m.intent, m.version
        );
        let here = fp.tell();
        let _ = writeln!(
            out,
            "[DEBUG] mghWrite() fpos = {here:<6} (after header, before 4D data)"
        );
    }
    let timing = getenv(env, "FS_MGZIO_TIMING");
    let begin = std::time::Instant::now();
    if timing {
        let _ = out.write_all(b"INFO: Environment variable FS_MGZIO_TIMING set\n");
    }
    if m.chunk.is_none() && getenv(env, "FS_MGZIO_USEVOXELBUFWRITE") {
        g.notices.add(
            &FS_446,
            "mghWrite with FS_MGZIO_USEVOXELBUFWRITE of a volume made without voxels: its contiguous-buffer flag (memory contents in FreeSurfer) taken as unset",
        );
    }
    let usevoxelbuf = m.ischunked && getenv(env, "FS_MGZIO_USEVOXELBUFWRITE");
    // a volume without voxels: FreeSurfer reads its slices (a null pointer) at a byte volume's
    // first row, at another handled type's first voxel
    let rows = i64::from(depth.max(0))
        * i64::from(height.max(0))
        * i64::from((end_frame - start_frame + 1).max(0));
    let touches = match m.mtype {
        MRI_UCHAR => true,
        MRI_SHORT | MRI_USHRT | MRI_INT | MRI_FLOAT | MRI_FLOAT_COMPLEX => width > 0,
        _ => false,
    };
    if !usevoxelbuf && m.chunk.is_none() && rows > 0 && touches {
        g.notices.add(
            &FS_58,
            "mghWrite of a volume made without voxels".to_string(),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "mghWrite(): the volume has no voxels (FreeSurfer crashes here)\n",
        )?;
    }
    if usevoxelbuf {
        let _ = out.write_all(b"INFO: Environment variable FS_MGZIO_USEVOXELBUFWRITE set\n");
        let _ = out.write_all(b"mghWrite(): output voxel bufer ...\n");
        let size = mri::mri_sizeof(m.mtype);
        if i64::from(depth) * i64::from(m.nframes) <= 0 {
            // FreeSurfer takes the address of a first slice the volume does not have
            g.notices.add(
                &FS_445,
                "mghWrite with FS_MGZIO_USEVOXELBUFWRITE of a volume without slices: no voxels written (FreeSurfer crashes)",
            );
        } else if size == usize::MAX {
            // MRIsizeof's -1: FreeSurfer writes and swaps far past the volume
            g.notices.add(
                &FS_55,
                format!(
                    "mghWrite with FS_MGZIO_USEVOXELBUFWRITE of a volume of type {}, which MRIsizeof does not know: nothing written",
                    m.mtype
                ),
            );
        } else {
            let bpv = size as i32;
            let bytes_per_slice = bpv.wrapping_mul(width).wrapping_mul(height) as i64 as u64;
            let bytes_to_write = (bpv as i64 as u64)
                .wrapping_mul(width as i64 as u64)
                .wrapping_mul(height as i64 as u64)
                .wrapping_mul(depth as i64 as u64)
                .wrapping_mul((end_frame - start_frame + 1) as i64 as u64);
            let frame_bytes = (m.vox_per_row as usize)
                .wrapping_mul(height.max(0) as usize)
                .wrapping_mul(depth.max(0) as usize)
                .wrapping_mul(m.bytes_per_vox);
            // the first slice's first row: a tensor's (and another type the slices' set-up does
            // not step for) rows all start at the buffer's start
            let steps = matches!(
                m.mtype,
                MRI_UCHAR
                    | MRI_SHORT
                    | MRI_USHRT
                    | MRI_INT
                    | MRI_FLOAT
                    | MRI_FLOAT_COMPLEX
                    | mri::MRI_RGB
                    | mri::MRI_LONG
            );
            let mut place = if steps {
                (start_frame.max(0) as usize).wrapping_mul(frame_bytes)
            } else {
                0
            };
            let mut left = bytes_to_write;
            let unit = match bpv {
                2 => 2,
                4 | 8 => 4,
                _ => 1,
            };
            if unit > 1 && left > 0 {
                g.notices.add(
                    &FS_443,
                    "mghWrite with FS_MGZIO_USEVOXELBUFWRITE: the volume's voxels byte-swapped in memory",
                );
            }
            let swap_in_place = g.notices.reproduce();
            while left > 0 {
                let n = bytes_per_slice as usize;
                let total = m.chunk.as_ref().map_or(0, |c| c.len());
                let have = total.saturating_sub(place).min(n);
                let mut slice = vec![0u8; n];
                if let Some(c) = m.chunk.as_ref() {
                    slice[..have].copy_from_slice(&c[place..place + have]);
                }
                if unit > 1 {
                    for c in slice.chunks_exact_mut(unit) {
                        c.reverse();
                    }
                }
                if swap_in_place && have > 0 {
                    let c = m.chunk.as_mut().expect("voxels");
                    c[place..place + have].copy_from_slice(&slice[..have]);
                }
                let wrote = fio::fs_znzwrite(fp, &slice, 1, n).max(0) as u64;
                if wrote != bytes_per_slice {
                    file.close();
                    log::error_printf(
                        g,
                        err,
                        ERROR_BADFILE,
                        &format!(
                            "mghWrite: (FS_MGZIO_USEVOXELBUFWRITE) could not write {bytes_per_slice} bytes, wrote only {wrote} bytes"
                        ),
                    );
                    return Ok(ERROR_BADFILE);
                }
                left = left.wrapping_sub(wrote);
                place = place.wrapping_add(wrote as usize);
            }
        }
    } else {
        let row_len = |unit: usize| width.max(0) as usize * unit;
        for f in start_frame..=end_frame {
            for z in 0..depth {
                let mut slice: Vec<u8> = Vec::new();
                for y in 0..height {
                    // the row among all rows of the buffer; zeros for a frame outside (FS-444)
                    let row = |unit: usize| {
                        if outside_frame {
                            vec![0u8; row_len(unit)]
                        } else {
                            row_bytes(m, ((f * depth + z) * height + y) as usize, row_len(unit))
                        }
                    };
                    match m.mtype {
                        MRI_SHORT | MRI_USHRT | MRI_INT | MRI_FLOAT => {
                            let unit = if matches!(m.mtype, MRI_SHORT | MRI_USHRT) {
                                2
                            } else {
                                4
                            };
                            let mut b = row(unit);
                            for c in b.chunks_exact_mut(unit) {
                                c.reverse();
                            }
                            slice.extend_from_slice(&b);
                        }
                        MRI_FLOAT_COMPLEX => {
                            for (x, c) in row(8).chunks_exact(8).enumerate() {
                                let real = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                                let imag = f32::from_le_bytes([c[4], c[5], c[6], c[7]]);
                                slice.extend_from_slice(&real.to_be_bytes());
                                slice.extend_from_slice(&imag.to_be_bytes());
                                if info {
                                    let _ = writeln!(
                                        out,
                                        "[DEBUG] [{x} {y} {z} {f}] real = {}, imag = {}",
                                        cfmt::f(f64::from(real), 6),
                                        cfmt::f(f64::from(imag), 6)
                                    );
                                }
                            }
                        }
                        MRI_UCHAR => {
                            let b = row(1);
                            if fio::fs_znzwrite(fp, &b, 1, b.len()) as i32 != width {
                                let r = fail_code(
                                    g,
                                    err,
                                    ERROR_BADFILE,
                                    &format!("mghWrite: could not write {width} bytes to {fname}"),
                                );
                                left_open(g, file, fname, "a byte row that did not write");
                                return Ok(r);
                            }
                        }
                        t => {
                            let r = fail_code(
                                g,
                                err,
                                ERROR_UNSUPPORTED,
                                &format!("mghWrite: unsupported type {t}"),
                            );
                            left_open(g, file, fname, &format!("an unsupported type ({t})"));
                            return Ok(r);
                        }
                    }
                }
                if !slice.is_empty() {
                    // one write per slice: the same bytes as FreeSurfer's one write per voxel
                    // (whose counts it does not check)
                    put(fp, &mut short, &slice);
                }
            }
        }
    }
    if timing {
        let t = begin.elapsed();
        let _ = writeln!(
            out,
            "Total time (mghWrite) = {}.{:09} seconds{}",
            t.as_secs(),
            t.subsec_nanos(),
            if usevoxelbuf { " (USEVOXELBUF)" } else { "" }
        );
    }
    if info {
        let here = fp.tell();
        let _ = writeln!(
            out,
            "[DEBUG] mghWrite() fpos = {here:<6} (after 4D data, before scan parameters)"
        );
    }
    let mut scan = Vec::with_capacity(20);
    for v in [m.tr, m.flip_angle as f32, m.te, m.ti, m.fov] {
        scan.extend_from_slice(&v.to_be_bytes());
    }
    put(fp, &mut short, &scan);
    if info {
        let _ = writeln!(
            out,
            "[DEBUG] tr = {}, flip_angle = {}, te = {}, ti = {}, fov = {}",
            cfmt::f(f64::from(m.tr), 6),
            cfmt::f(m.flip_angle, 6),
            cfmt::f(f64::from(m.te), 6),
            cfmt::f(f64::from(m.ti), 6),
            cfmt::f(f64::from(m.fov), 6)
        );
        let here = fp.tell();
        let _ = writeln!(
            out,
            "[DEBUG] mghWrite() fpos = {here:<6} (after scan parameters, before TAG)"
        );
    }
    mri_tag_write(g, out, err, m, fp, false)?;
    let closed = file.close();
    if short || closed != 0 {
        g.notices.add(
            &FS_440,
            format!(
                "mghWrite of {fname}: a write or the close failed (FreeSurfer reports success)"
            ),
        );
        if !g.notices.reproduce() {
            return Ok(fail_code(
                g,
                err,
                ERROR_BADFILE,
                &format!("mghWrite: could not write {fname}"),
            ));
        }
    }
    Ok(NO_ERROR)
}

/// FS-439: FreeSurfer returns without closing the file. By default it is closed (what was written
/// stays, compressed or not); with `reproduce` it is dropped unclosed: a compressed file loses the
/// bytes zlib still held (an `.mgz` stays empty), a plain one keeps what was written (FreeSurfer's
/// stream writes it when the program ends).
fn left_open(g: &mut Globals, file: ZnzFile, fname: &str, why: &str) {
    g.notices.add(
        &FS_439,
        format!("mghWrite of {fname} after {why}: the file left open"),
    );
    if g.notices.reproduce() {
        drop(file);
    } else {
        file.close();
    }
}

/// `MRITAGwrite(mri, fp, niftiheaderext)`: the tags of `m` on `fp`.
///
/// A GCA morph (`warpFieldFormat` set): its two geometries shearless and with their shears, its
/// metadata (format, spacing, exponent), its affine matrix and labels when it has them, the
/// extension's end tag; nothing else. Otherwise: in the extension the degrees of freedom, the scan
/// parameters and the RAS transform with its extra part; the talairach transform's name (with its
/// NUL) when it has one; the auto-align matrix; in an `.mgz` the phase-encoding direction (with its
/// NUL; `UNKNOWN` without one when there is none); the original RAS-to-voxel matrix (`saving
/// original ras2vox` on standard output); in an `.mgz` the field strength (4 bytes in the machine's
/// order); the frames when there is more than one or the first has a label or a threshold of at
/// least `FLT_EPSILON` in magnitude (FS-441); the colour table; each command line (with its NUL);
/// the extension's end tag.
pub fn mri_tag_write(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Mri,
    fp: &mut ZnzFile,
    niftiheaderext: bool,
) -> Result<(), Fatal> {
    let info = g.gdiag & DIAG_INFO != 0;
    let mut io = FsTagsIo::new(fp, niftiheaderext);
    if m.warp_field_format != WARPFIELD_DTFMT_UNKNOWN {
        io.write_gcamorph_geom(
            g,
            out,
            err,
            &m.gcamorph_image_vg,
            &m.gcamorph_atlas_vg,
            true,
        )?;
        io.write_gcamorph_geom(
            g,
            out,
            err,
            &m.gcamorph_image_vg,
            &m.gcamorph_atlas_vg,
            false,
        )?;
        if info {
            let _ = writeln!(
                out,
                "[DEBUG] MRITAGwrite() TAG_GCAMORPH_META dlen = 12, warpFieldFormat = {}, gcamorphSpacing = {}, gcamorphExp_k = {}",
                m.warp_field_format,
                m.gcamorph_spacing,
                cfmt::f(m.gcamorph_exp_k, 6)
            );
        }
        io.write_gcamorph_meta(
            g,
            out,
            m.warp_field_format,
            m.gcamorph_spacing,
            m.gcamorph_exp_k,
        );
        if let Some(a) = &m.gcamorph_affine {
            if info {
                let _ = out.write_all(b"[DEBUG] MRITAGwrite() TAG_GCAMORPH_AFFINE\n");
                fstagsio::print_matrix(g, out, err, Some(a));
            }
            io.write_matrix(g, out, err, a, TAG_GCAMORPH_AFFINE);
        }
        if let Some(l) = &m.gcamorph_label {
            if info {
                let _ = out.write_all(b"[DEBUG] MRITAGwrite() TAG_GCAMORPH_LABELS\n");
            }
            io.write_gcamorph_labels(g, out, m.vg.width, m.vg.height, m.vg.depth, l);
        }
        if niftiheaderext {
            io.write_endtag(g, out);
        }
        return Ok(());
    }
    if niftiheaderext {
        io.write_dof(g, out, m.dof);
        io.write_scan_parameters(g, out, m);
        io.write_ras_xform(g, out, &m.vg);
        io.write_ras_xform_extra(g, out, &m.vg);
    }
    let t = c_bytes(m.transform_fname.as_bytes());
    if !t.is_empty() {
        let mut b = t.to_vec();
        b.push(0);
        let n = b.len() as i64;
        io.write_tag(g, out, TAG_MGH_XFORM, &b, n);
    }
    if let Some(a) = &m.auto_align {
        io.write_matrix(g, out, err, a, TAG_AUTO_ALIGN);
    }
    if !niftiheaderext {
        match &m.pedir {
            Some(p) => {
                let mut b = c_bytes(p).to_vec();
                b.push(0);
                let n = b.len() as i64;
                io.write_tag(g, out, TAG_PEDIR, &b, n);
            }
            None => {
                io.write_tag(g, out, TAG_PEDIR, b"UNKNOWN", 7);
            }
        }
    }
    if let Some(a) = &m.orig_ras2vox {
        let _ = out.write_all(b"saving original ras2vox\n");
        io.write_matrix(g, out, err, a, TAG_ORIG_RAS2VOX);
    }
    if !niftiheaderext {
        io.write_tag(
            g,
            out,
            TAG_FIELDSTRENGTH,
            &m.field_strength.to_le_bytes(),
            4,
        );
    }
    let first = m.frames.first();
    if first.is_none() && m.nframes <= 1 {
        g.notices.add(
            &FS_441,
            format!(
                "the tags of a volume of {} frames: its first frame's label and threshold (FreeSurfer reads memory past its frames)",
                m.nframes
            ),
        );
    }
    let first_marked = first.is_some_and(|f| f.label != 0 || f.thresh.abs() >= f32::EPSILON);
    if m.nframes > 1 || first_marked {
        io.write_mri_frames(g, out, err, m)?;
    }
    if let Some(ct) = &m.ct {
        io.write_old_colortable(g, out, err, ct);
    }
    for c in &m.cmdlines {
        let mut b = c_bytes(c.as_bytes()).to_vec();
        b.push(0);
        let n = b.len() as i64;
        io.write_tag(g, out, TAG_CMDLINE, &b, n);
    }
    if niftiheaderext {
        io.write_endtag(g, out);
    }
    Ok(())
}
