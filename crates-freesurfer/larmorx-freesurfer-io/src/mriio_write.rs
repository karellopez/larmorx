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
//!
//! NIfTI: `niiWrite` ([`nii_write`]: `.nii`, `.nii.gz`, FS-447 to FS-449) and `nifti1Write`
//! ([`nifti1_write`]: the `.hdr`/`.img` pair; FS-447, FS-449, FS-451, FS-452).

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::{
    FS_55, FS_58, FS_439, FS_440, FS_441, FS_442, FS_443, FS_444, FS_445, FS_446, FS_447, FS_448,
    FS_449, FS_451, FS_452,
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
use larmorx_freesurfer_mri::vol_geom;
use larmorx_io::znz::ZnzFile;

use crate::cfile::CFile;
use crate::fio;
use crate::fstagsio::{self, FsTagsIo};
use crate::tags::{
    self, TAG_AUTO_ALIGN, TAG_CMDLINE, TAG_FIELDSTRENGTH, TAG_GCAMORPH_AFFINE, TAG_MGH_XFORM,
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
                if let Some(c) = m.chunk.as_ref().filter(|_| have > 0) {
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
                            for (x, c) in row(8).as_chunks::<8>().0.iter().enumerate() {
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
    if frames_marked(g, m) {
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

// ---------------------------------------------------------------- NIfTI (`niiWrite`)

/// The date of FreeSurfer 8.2.0's official build (`__DATE__` in its `mriio.cpp`, the same in the
/// unit oracle's build): a NIfTI file's description is `FreeSurfer <date>`.
pub const BUILD_DATE: &str = "Mar 14 2026";

const DT_UNSIGNED_CHAR: i16 = 2;
const DT_SIGNED_SHORT: i16 = 4;
const DT_SIGNED_INT: i16 = 8;
const DT_FLOAT: i16 = 16;
const DT_COMPLEX: i16 = 32;
const DT_UINT16: i16 = 512;
const NIFTI_INTENT_DISPVECT: i16 = 1006;
const NIFTI_ECODE_FREESURFER: i32 = 14;
const NIFTI_XFORM_SCANNER_ANAT: i16 = 1;
const SHORTMAX: i32 = 32768;
/// `WarpfieldDTFMT::WARPFIELD_DTFMT_DISP_RAS`.
const WARPFIELD_DTFMT_DISP_RAS: i32 = 3;

/// `struct nifti_1_header` (348 bytes) in x86-64's memory: the fields at their offsets,
/// little-endian.
struct NiftiHeader([u8; 348]);

impl NiftiHeader {
    fn i32_at(&mut self, o: usize, v: i32) {
        self.0[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn i16_at(&mut self, o: usize, v: i16) {
        self.0[o..o + 2].copy_from_slice(&v.to_le_bytes());
    }
    fn f32_at(&mut self, o: usize, v: f32) {
        self.0[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn dim(&mut self, i: usize, v: i16) {
        self.i16_at(40 + 2 * i, v);
    }
    fn dim_of(&self, i: usize) -> i16 {
        i16::from_le_bytes([self.0[40 + 2 * i], self.0[41 + 2 * i]])
    }
    fn pixdim(&mut self, i: usize, v: f32) {
        self.f32_at(76 + 4 * i, v);
    }
    fn vox_offset(&self) -> f32 {
        f32::from_le_bytes([self.0[108], self.0[109], self.0[110], self.0[111]])
    }
}

/// `mriToNiftiQform(mri, hdr)`: the quaternion of the direction cosines (`mat44_to_quatern`'s
/// steps in `float`, the constants and square roots in `double`), `qfac` 1 for a positive
/// determinant, -1 (the third column negated) for a negative one, and -100000 with `WARNING: bad
/// orientation matrix (determinant = 0) in nifti1 file ...` and ` ... continuing.` on standard
/// output for 0; the offsets from `extract_i_to_r`.
fn mri_to_nifti_qform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Mri,
    hdr: &mut NiftiHeader,
) -> Result<i32, Fatal> {
    let v = &m.vg;
    let (r11, r12, mut r13) = (v.x_r, v.y_r, v.z_r);
    let (r21, r22, mut r23) = (v.x_a, v.y_a, v.z_a);
    let (r31, r32, mut r33) = (v.x_s, v.y_s, v.z_s);
    let mut qfac: f32 = -100000.0;
    let r_det: f32 = r11 * (r22 * r33 - r32 * r23) - r12 * (r21 * r33 - r31 * r23)
        + r13 * (r21 * r32 - r31 * r22);
    if r_det == 0.0 {
        let _ = out.write_all(
            b"WARNING: bad orientation matrix (determinant = 0) in nifti1 file ...\n ... continuing.\n",
        );
    } else if r_det > 0.0 {
        qfac = 1.0;
    } else {
        r13 = -r13;
        r23 = -r23;
        r33 = -r33;
        qfac = -1.0;
    }
    let d64 = f64::from;
    let mut a = (d64(r11 + r22 + r33) + 1.0) as f32;
    let (mut b, mut c, mut d);
    if d64(a) > 0.5 {
        a = (0.5 * d64(a).sqrt()) as f32;
        b = (0.25 * d64(r32 - r23) / d64(a)) as f32;
        c = (0.25 * d64(r13 - r31) / d64(a)) as f32;
        d = (0.25 * d64(r21 - r12) / d64(a)) as f32;
    } else {
        let xd = (1.0 + d64(r11) - d64(r22 + r33)) as f32;
        let yd = (1.0 + d64(r22) - d64(r11 + r33)) as f32;
        let zd = (1.0 + d64(r33) - d64(r11 + r22)) as f32;
        if d64(xd) > 1.0 {
            b = (0.5 * d64(xd).sqrt()) as f32;
            c = (0.25 * d64(r12 + r21) / d64(b)) as f32;
            d = (0.25 * d64(r13 + r31) / d64(b)) as f32;
            a = (0.25 * d64(r32 - r23) / d64(b)) as f32;
        } else if d64(yd) > 1.0 {
            c = (0.5 * d64(yd).sqrt()) as f32;
            b = (0.25 * d64(r12 + r21) / d64(c)) as f32;
            d = (0.25 * d64(r23 + r32) / d64(c)) as f32;
            a = (0.25 * d64(r13 - r31) / d64(c)) as f32;
        } else {
            d = (0.5 * d64(zd).sqrt()) as f32;
            b = (0.25 * d64(r13 + r31) / d64(d)) as f32;
            c = (0.25 * d64(r23 + r32) / d64(d)) as f32;
            a = (0.25 * d64(r21 - r12) / d64(d)) as f32;
        }
        if d64(a) < 0.0 {
            b = -b;
            c = -c;
            d = -d;
        }
    }
    hdr.i16_at(252, NIFTI_XFORM_SCANNER_ANAT);
    hdr.pixdim(0, qfac);
    hdr.f32_at(256, b);
    hdr.f32_at(260, c);
    hdr.f32_at(264, d);
    let i_to_r = vol_geom::extract_i_to_r(g, out, err, &m.vg)?;
    hdr.f32_at(268, i_to_r.relt(1, 4));
    hdr.f32_at(272, i_to_r.relt(2, 4));
    hdr.f32_at(276, i_to_r.relt(3, 4));
    Ok(NO_ERROR)
}

/// `mriToNiftiSform(mri, hdr)`: the rows of `MRIxfmCRS2XYZ(mri, 0)`, the scanner code.
fn mri_to_nifti_sform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Mri,
    hdr: &mut NiftiHeader,
) -> Result<(), Fatal> {
    let v = vol_geom::xfm_crs2xyz(g, out, err, &m.vg, 0, false)?;
    for c in 0..4 {
        hdr.f32_at(280 + 4 * c, v.relt(1, c as i32 + 1));
        hdr.f32_at(296 + 4 * c, v.relt(2, c as i32 + 1));
        hdr.f32_at(312 + 4 * c, v.relt(3, c as i32 + 1));
    }
    hdr.i16_at(254, NIFTI_XFORM_SCANNER_ANAT);
    Ok(())
}

/// Whether the frames tag is written (`MRITAGwrite`, `__getMRITAGlength`; FS-441).
fn frames_marked(g: &mut Globals, m: &Mri) -> bool {
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
    m.nframes > 1 || first.is_some_and(|f| f.label != 0 || f.thresh.abs() >= f32::EPSILON)
}

/// `__getMRITAGlength(mri, niftiheaderext)`: the bytes [`mri_tag_write`] writes, each tag with its
/// number and length (`[DEBUG] __getMRITAGlength(): ...` lines with `DIAG_INFO`).
pub fn mri_tag_length(g: &mut Globals, out: &mut dyn Write, m: &Mri, nifti: bool) -> i64 {
    let info = g.gdiag & DIAG_INFO != 0;
    let mut dlen: i64 = 0;
    if info {
        let _ = writeln!(out, "[DEBUG] __getMRITAGlength(): dlen = {dlen:<6}");
    }
    let add = |g: &Globals, out: &mut dyn Write, dlen: &mut i64, taglen: i64, tag: i32| {
        *dlen += taglen;
        if g.gdiag & DIAG_INFO != 0 {
            let _ = writeln!(
                out,
                "[DEBUG] __getMRITAGlength(): +{taglen:<6}, dlen = {:<6} (TAG = {tag:<2})",
                *dlen
            );
        }
    };
    if m.warp_field_format != WARPFIELD_DTFMT_UNKNOWN {
        let (src, trg) = (
            m.gcamorph_image_vg.fname.c_bytes().to_vec(),
            m.gcamorph_atlas_vg.fname.c_bytes().to_vec(),
        );
        let t = fstagsio::getlen_gcamorph_geom(g, &src, &trg, nifti, true, true);
        add(g, out, &mut dlen, t, tags::TAG_GCAMORPH_GEOM);
        let t = fstagsio::getlen_gcamorph_geom(g, &src, &trg, nifti, true, false);
        add(g, out, &mut dlen, t, tags::TAG_GCAMORPH_GEOM_PLUSSHEAR);
        add(
            g,
            out,
            &mut dlen,
            fstagsio::getlen_gcamorph_meta(true),
            tags::TAG_GCAMORPH_META,
        );
        if m.gcamorph_affine.is_some() {
            add(
                g,
                out,
                &mut dlen,
                fstagsio::getlen_matrix(nifti, true),
                TAG_GCAMORPH_AFFINE,
            );
        }
        if m.gcamorph_label.is_some() {
            let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
            let t = fstagsio::getlen_gcamorph_labels(g, w, h, d, 4, nifti, true);
            add(g, out, &mut dlen, t, tags::TAG_GCAMORPH_LABELS);
        }
        if nifti {
            add(
                g,
                out,
                &mut dlen,
                fstagsio::getlen_endtag(true),
                tags::TAG_END_NIIHDREXTENSION,
            );
        }
        return dlen;
    }
    if nifti {
        add(g, out, &mut dlen, fstagsio::getlen_dof(true), tags::TAG_DOF);
        let t = fstagsio::getlen_scan_parameters(m, true);
        add(g, out, &mut dlen, t, tags::TAG_SCAN_PARAMETERS);
        add(
            g,
            out,
            &mut dlen,
            fstagsio::getlen_ras_xform(true),
            tags::TAG_RAS_XFORM,
        );
        let t = fstagsio::getlen_ras_xform_extra(true);
        add(g, out, &mut dlen, t, tags::TAG_RAS_XFORM_EXTRA);
    }
    let flen = c_bytes(m.transform_fname.as_bytes()).len() as i64;
    if flen > 0 {
        let t = fstagsio::getlen_tag(TAG_MGH_XFORM, flen + 1, nifti, true);
        add(g, out, &mut dlen, t, TAG_MGH_XFORM);
    }
    if m.auto_align.is_some() {
        add(
            g,
            out,
            &mut dlen,
            fstagsio::getlen_matrix(nifti, true),
            TAG_AUTO_ALIGN,
        );
    }
    if !nifti {
        let n = match &m.pedir {
            Some(p) => c_bytes(p).len() as i64 + 1,
            None => 7,
        };
        add(
            g,
            out,
            &mut dlen,
            fstagsio::getlen_tag(TAG_PEDIR, n, nifti, true),
            TAG_PEDIR,
        );
    }
    if m.orig_ras2vox.is_some() {
        add(
            g,
            out,
            &mut dlen,
            fstagsio::getlen_matrix(nifti, true),
            TAG_ORIG_RAS2VOX,
        );
    }
    if !nifti {
        let t = fstagsio::getlen_tag(TAG_FIELDSTRENGTH, 4, nifti, true);
        add(g, out, &mut dlen, t, TAG_FIELDSTRENGTH);
    }
    if frames_marked(g, m) {
        let t = fstagsio::getlen_mri_frames(m, nifti, true);
        add(g, out, &mut dlen, t, tags::TAG_MRI_FRAME);
    }
    if let Some(ct) = &m.ct {
        let t = fstagsio::getlen_old_colortable(ct, nifti, true);
        add(g, out, &mut dlen, t, tags::TAG_OLD_COLORTABLE);
    }
    for c in &m.cmdlines {
        let n = c_bytes(c.as_bytes()).len() as i64 + 1;
        add(
            g,
            out,
            &mut dlen,
            fstagsio::getlen_tag(TAG_CMDLINE, n, nifti, true),
            TAG_CMDLINE,
        );
    }
    if nifti {
        add(
            g,
            out,
            &mut dlen,
            fstagsio::getlen_endtag(true),
            tags::TAG_END_NIIHDREXTENSION,
        );
    }
    dlen
}

/// `__writeFSniiextensionHeader(fp, mri, intent)`: `>`, the intent (16 bits, big-endian) and the
/// version's low byte; an `intent` other than -1 becomes the volume's intent (truncated to 16
/// bits) and version first.
fn write_fs_nii_extension_header(
    g: &mut Globals,
    out: &mut dyn Write,
    fp: &mut ZnzFile,
    m: &mut Mri,
    intentcode: i32,
) {
    let info = g.gdiag & DIAG_INFO != 0;
    if info {
        let _ = writeln!(
            out,
            "[DEBUG] __writeFSniiextensionHeader(): intent (user) = {intentcode}, mri->intent (orig) = {}, mri->version (orig) = {}",
            m.intent, m.version
        );
    }
    let mut intent = ((m.version >> 8) & 0xffff) as u16;
    if intentcode != MGZ_INTENT_UNKNOWN {
        intent = intentcode as u16;
        m.intent = i32::from(intent);
        m.version = ((i32::from(intent) & 0xffff) << 8) | MGH_VERSION;
    }
    let le = intent.to_le_bytes();
    let ext = [b'>', le[1], le[0], (m.version & 0xff) as u8];
    if info {
        let _ = writeln!(
            out,
            "[DEBUG] __writeFSniiextensionHeader(): intent swapped byte order ({:02x} {:02x}) => ({:02x} {:02x})",
            le[0], ext[1], ext[1], ext[2]
        );
        let _ = writeln!(
            out,
            "[DEBUG] __writeFSniiextensionHeader(): intent = {intent}, mgh version = {}, mri->intent (new) = {}, mri->version (new) = {}",
            ext[3], m.intent, m.version
        );
        let _ = writeln!(
            out,
            "[DEBUG] __writeFSniiextensionHeader(): {:02x} {:02x} {:02x} {:02x}",
            ext[0], ext[1], ext[2], ext[3]
        );
    }
    fio::fs_znzwrite(fp, &ext, 1, 4);
}

/// The ico7 surface's reshape (`mri_reshape(mri, 27307, 1, 6, nframes)`).
fn reshape_ico7(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Mri,
) -> Result<Mri, Fatal> {
    reshape(g, out, err, m, 27307, 1, 6, m.nframes)
}

/// `mri_reshape(mri, w, h, d, nf)` as the NIfTI readers and writers call it (the same number of
/// voxels): a new volume of that extent with the volume's header and pulse parameters and, for
/// uchar, short, unsigned short, float, int and long volumes, the voxels in the same order (the
/// others' zeros).
#[allow(clippy::too_many_arguments)]
pub(crate) fn reshape(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Mri,
    w: i32,
    h: i32,
    d: i32,
    nf: i32,
) -> Result<Mri, Fatal> {
    let copied = matches!(
        m.mtype,
        MRI_UCHAR | MRI_SHORT | MRI_USHRT | MRI_FLOAT | MRI_INT | mri::MRI_LONG
    );
    if copied && m.chunk.is_none() && m.nframes > 0 {
        // FreeSurfer reads the first voxel through slices the volume does not have (a crash)
        g.notices.add(
            &FS_58,
            "mri_reshape of a volume made without voxels".to_string(),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "mri_reshape(): the volume has no voxels (FreeSurfer crashes here)\n",
        )?;
    }
    let mut r = mri::mri_alloc_sequence(g, out, err, w, h, d, m.mtype, nf)?;
    let _ = mri::mri_copy_header(g, out, err, m, Some(&mut r))?;
    mri::mri_copy_pulse_parameters(m, &mut r);
    if copied && let (Some(s), Some(d)) = (m.chunk.as_ref(), r.chunk.as_mut()) {
        let n = s.len().min(d.len());
        d[..n].copy_from_slice(&s[..n]);
    }
    Ok(r)
}

/// FS-447: whether a long volume's rows are written as 32-bit numbers (the default) rather than
/// as their first bytes (FreeSurfer's, `reproduce`), noted when the volume has voxels to write.
fn long_rows_as_ints(g: &mut Globals, m: &Mri, func: &str) -> bool {
    let long_rows = m.mtype == mri::MRI_LONG;
    let (w, h, d, nf) = (m.vg.width, m.vg.height, m.vg.depth, m.nframes);
    if long_rows && w > 0 && h > 0 && d > 0 && nf > 0 {
        g.notices.add(
            &FS_447,
            format!(
                "{func} of a long volume: each row's first bytes as 32-bit numbers (FreeSurfer)"
            ),
        );
    }
    long_rows && g.notices.fix()
}

/// The bytes a NIfTI writer writes for the stored row `row` of `m`: its first `width x unit`
/// bytes, or (FS-447's default for a long volume) each value as a 32-bit number, clamped (noted).
fn nifti_row(
    g: &mut Globals,
    m: &Mri,
    row: usize,
    unit: usize,
    as_ints: bool,
    func: &str,
) -> Vec<u8> {
    let w = m.vg.width.max(0) as usize;
    if !as_ints {
        return row_bytes(m, row, w * unit);
    }
    let b = row_bytes(m, row, w * 8);
    let mut r = Vec::with_capacity(w * 4);
    for v in b.as_chunks::<8>().0 {
        let l = i64::from_le_bytes(*v);
        let i = l.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        if i64::from(i) != l {
            g.notices.add(
                &FS_447,
                format!("{func}: the long {l} clamped to a 32-bit number"),
            );
        }
        r.extend_from_slice(&i.to_le_bytes());
    }
    r
}

/// The NIfTI writers' size checks: more than 32,768 rows, slices or frames end the program
/// (`NIFTI FORMAT ERROR: nrows <n> in volume exceeds 32768` on standard output, status 1); FS-449
/// for exactly 32,768.
fn nifti_size_checks(
    g: &mut Globals,
    out: &mut dyn Write,
    func: &str,
    h: i32,
    d: i32,
    nf: i32,
) -> Result<(), Fatal> {
    for (n, what, sp) in [
        (h, "nrows", " "),
        (d, "nslices", " "),
        (nf, "nframes", "  "),
    ] {
        // FS-449: 32,768 passes FreeSurfer's check and is stored as -32768; corrected, refused as
        // the larger ones are, against the field's true limit
        let limit = if n == SHORTMAX {
            g.notices.add(
                &FS_449,
                format!("{func}: {n} {what}, which the header's 16-bit field holds as -32768"),
            );
            g.notices.fix().then_some(SHORTMAX - 1)
        } else {
            (n > SHORTMAX).then_some(SHORTMAX)
        };
        if let Some(limit) = limit {
            let _ = writeln!(
                out,
                "NIFTI FORMAT ERROR:{sp}{what} {n} in volume exceeds {limit}"
            );
            return Err(Fatal::Exit(1));
        }
    }
    Ok(())
}

/// `niiWrite(mri, fname, intent)`: the volume as a NIfTI-1 file (`.nii`; compressed when the name
/// ends in `z`), with FreeSurfer's header extension (code 14). See `specs/freesurfer-io.md` §93.
#[allow(clippy::too_many_arguments)]
pub fn nii_write(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    m0: &mut Mri,
    fname: &str,
    intent: i32,
) -> Result<i32, Fatal> {
    let _ = env;
    let use_compression = fname.as_bytes().last() == Some(&b'z');
    if g.gdiag_no > 0 {
        let _ = writeln!(
            out,
            "niiWrite: use_compression = {}",
            i32::from(use_compression)
        );
    }
    let mut reshaped = if m0.vg.width == 163842 && m0.vg.height == 1 && m0.vg.depth == 1 {
        Some(reshape_ico7(g, out, err, m0)?)
    } else {
        None
    };
    let m: &mut Mri = match reshaped.as_mut() {
        Some(r) => r,
        None => m0,
    };
    let (w, h, d, nf) = (m.vg.width, m.vg.height, m.vg.depth, m.nframes);
    nifti_size_checks(g, out, "niiWrite", h, d, nf)?;
    let mut hdr = NiftiHeader([0u8; 348]);
    hdr.i32_at(0, 348);
    for t in 0..8 {
        hdr.dim(t, 1);
        hdr.pixdim(t, 1.0);
    }
    hdr.dim(0, if nf == 1 { 3 } else { 4 });
    if w < SHORTMAX {
        hdr.dim(1, w as i16);
    } else {
        hdr.dim(1, -1);
        hdr.i32_at(144, w);
    }
    hdr.dim(2, h as i16);
    hdr.dim(3, d as i16);
    hdr.dim(4, nf as i16);
    hdr.pixdim(1, m.vg.xsize);
    hdr.pixdim(2, m.vg.ysize);
    hdr.pixdim(3, m.vg.zsize);
    hdr.pixdim(4, (f64::from(m.tr) / 1000.0) as f32);
    let (datatype, bitpix): (i16, i16) = match m.mtype {
        MRI_UCHAR => (DT_UNSIGNED_CHAR, 8),
        MRI_INT | mri::MRI_LONG => (DT_SIGNED_INT, 32),
        MRI_FLOAT => (DT_FLOAT, 32),
        MRI_FLOAT_COMPLEX => (DT_COMPLEX, 64),
        MRI_SHORT => (DT_SIGNED_SHORT, 16),
        MRI_USHRT => (DT_UINT16, 16),
        mri::MRI_BITMAP => {
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                "niiWrite(): data type MRI_BITMAP unsupported",
            );
            return Ok(ERROR_UNSUPPORTED);
        }
        mri::MRI_TENSOR => {
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                "niiWrite(): data type MRI_TENSOR unsupported",
            );
            return Ok(ERROR_UNSUPPORTED);
        }
        t => {
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!("niiWrite(): unknown data type {t}"),
            );
            return Ok(ERROR_BADPARM);
        }
    };
    hdr.i16_at(70, datatype);
    hdr.i16_at(72, bitpix);
    hdr.f32_at(108, 352.0);
    hdr.0[123] = 2 | 8; // NIFTI_UNITS_MM | NIFTI_UNITS_SEC
    let descrip = format!("FreeSurfer {BUILD_DATE}");
    hdr.0[148..148 + descrip.len()].copy_from_slice(descrip.as_bytes());
    if m.intent == mri::MGZ_INTENT_WARPMAP {
        hdr.i16_at(68, NIFTI_INTENT_DISPVECT);
        hdr.dim(0, 5);
        hdr.dim(4, 1);
        hdr.dim(5, nf as i16);
        let dims: Vec<String> = (0..8).map(|i| hdr.dim_of(i).to_string()).collect();
        let _ = writeln!(
            out,
            "[INFO] niiWrite(): MGZ_INTENT_WARPMAP => NIFTI_INTENT_DISPVECT, intent_code = {NIFTI_INTENT_DISPVECT}, dimensions = {{{}}}",
            dims.join(", ")
        );
        if m.warp_field_format != WARPFIELD_DTFMT_DISP_RAS {
            let _ = writeln!(
                out,
                "[INFO] niiWrite(): convert displacement from {} to {WARPFIELD_DTFMT_DISP_RAS}",
                m.warp_field_format
            );
            let _ = writeln!(
                err,
                "larmorx-freesurfer: writing a warp map's displacements in another format (Warpfield::changeFormat) is not ported yet ({fname})"
            );
            return Ok(-1);
        }
    }
    let r = mri_to_nifti_qform(g, out, err, m, &mut hdr)?;
    if r != NO_ERROR {
        return Ok(r);
    }
    mri_to_nifti_sform(g, out, err, m, &mut hdr)?;
    hdr.0[344..348].copy_from_slice(b"n+1\0");
    let path = utils::resolve(dir, fname);
    let Some(mut file) = ZnzFile::open(path.as_os_str().as_encoded_bytes(), b"w", use_compression)
    else {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADFILE,
            &format!("niiWrite(): error opening file {fname}"),
        ));
    };
    let fp = &mut file;
    let mgztaglen = mri_tag_length(g, out, m, true) as i32;
    let mut esize = mgztaglen;
    let info = g.gdiag & DIAG_INFO != 0;
    if esize > 0 {
        esize = esize.wrapping_add(12);
        if info {
            let _ = writeln!(
                out,
                "[DEBUG] niiWrite(): mgztaglen = {mgztaglen}, esize = {esize}"
            );
        }
        if esize & 0xf != 0 {
            esize = esize.wrapping_add(0xf) & !0xf;
        }
        let vo = hdr.vox_offset() + esize as f32;
        hdr.f32_at(108, vo);
        if info {
            let _ = writeln!(
                out,
                "[DEBUG] niiWrite(): updated vox_offset = {} (mgztaglen = {mgztaglen}, esize = {esize})",
                hdr.vox_offset() as i64
            );
        }
    }
    if fio::fs_znzwrite(fp, &hdr.0, 348, 1) != 1 {
        file.close();
        return Ok(fail_code(
            g,
            err,
            ERROR_BADFILE,
            &format!("niiWrite(): error writing header to {fname}"),
        ));
    }
    let chbuf = [u8::from(esize > 0), 0, 0, 0];
    if fio::fs_znzwrite(fp, &chbuf, 1, 4) != 4 {
        file.close();
        return Ok(fail_code(
            g,
            err,
            ERROR_BADFILE,
            &format!("niiWrite(): error writing data to {fname}"),
        ));
    }
    if esize > 0 {
        fio::fs_znzwrite(fp, &esize.to_le_bytes(), 4, 1);
        fio::fs_znzwrite(fp, &NIFTI_ECODE_FREESURFER.to_le_bytes(), 4, 1);
        write_fs_nii_extension_header(g, out, fp, m, intent);
        mri_tag_write(g, out, err, m, fp, true)?;
        let reminder = esize.wrapping_sub(mgztaglen.wrapping_add(12));
        if reminder > 0 {
            let pad = vec![0u8; reminder as usize];
            fio::fs_znzwrite(fp, &pad, 1, pad.len());
        }
        if info {
            let _ = writeln!(
                out,
                "[DEBUG] niiWrite(): mgztaglen = {mgztaglen}, esize = {esize}, reminder = {reminder}"
            );
            let _ = writeln!(
                out,
                "[DEBUG] niiWrite(): write freesurfer header-extension, mgztaglen = {mgztaglen}, esize = {esize}, ecode = {NIFTI_ECODE_FREESURFER}"
            );
        }
    }
    let unit = (bitpix / 8) as usize;
    // a volume without voxels: FreeSurfer takes its first row's address from slices it does not
    // have (a crash)
    if m.chunk.is_none() && i64::from(d.max(0)) * i64::from(nf.max(0)) * i64::from(h.max(0)) > 0 {
        g.notices.add(
            &FS_58,
            "niiWrite of a volume made without voxels".to_string(),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "niiWrite(): the volume has no voxels (FreeSurfer crashes here)\n",
        )?;
    }
    let as_ints = long_rows_as_ints(g, m, "niiWrite");
    for t in 0..nf {
        for k in 0..d {
            let mut slice = Vec::with_capacity(w.max(0) as usize * h.max(0) as usize * unit);
            for j in 0..h {
                let row = ((t * d + k) * h + j) as usize;
                slice.extend_from_slice(&nifti_row(g, m, row, unit, as_ints, "niiWrite"));
            }
            let n = (w.max(0) as usize) * (h.max(0) as usize);
            if fio::fs_znzwrite(fp, &slice, unit, n) != n as i64 {
                file.close();
                return Ok(fail_code(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("niiWrite(): error writing data to {fname}"),
                ));
            }
        }
    }
    if file.close() != 0 {
        g.notices.add(
            &FS_448,
            format!("niiWrite of {fname}: the close failed (FreeSurfer reports success)"),
        );
        if g.notices.fix() {
            return Ok(fail_code(
                g,
                err,
                ERROR_BADFILE,
                &format!("niiWrite(): error writing data to {fname}"),
            ));
        }
    }
    Ok(NO_ERROR)
}

// ---------------------------------------------------------------- NIfTI-1 pairs (`nifti1Write`)

/// `STRLEN`: the size of `nifti1Write`'s name buffers.
const STRLEN: usize = 1024;

/// `nifti1Write(mri, fname)`: the volume as a NIfTI-1 pair, the 348-byte header in `<stem>.hdr`
/// (magic `ni1`, voxel offset 0, no extension) and the voxels in `<stem>.img`, the stem the name
/// without a final `.img` or `.hdr`. As [`nii_write`] for the ico7 reshape, the size checks, the
/// header's fields, quaternion and affine, and the voxels; complex volumes are an unknown type here.
/// See `specs/freesurfer-io.md` §95.
///
/// **FS-451** (wrong names; an abort): FreeSurfer builds the names in buffers of 1,024 bytes: from
/// a stem of 1,020 bytes (a name without `.img` or `.hdr`) the names are cut to 1,023 bytes, with
/// `nifti1Write: Truncation of hdr_fname` and `... img_fname` on standard error (from a stem of
/// 1,022 bytes the header and the image are one file, the image's), and a name of 1,024 bytes or
/// more ends the program in the C library's overflow check (`*** buffer overflow detected ***:
/// terminated` on standard error, status 134). By default the whole names; `reproduce` as
/// FreeSurfer, noted (observed with the fs-io probe, `specs/freesurfer-io.md` §95).
/// **FS-452**: the two files' closes are not checked (a full disk: reported as written); by default
/// a failed close gives the write's error (`nifti1Write(): error writing header to <hdr>`, `...
/// error writing data to <img>`, `ERROR_BADFILE`); `reproduce` 0.
pub fn nifti1_write(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    m0: &mut Mri,
    fname: &str,
) -> Result<i32, Fatal> {
    let reshaped = if m0.vg.width == 163842 && m0.vg.height == 1 && m0.vg.depth == 1 {
        Some(reshape_ico7(g, out, err, m0)?)
    } else {
        None
    };
    let m: &Mri = reshaped.as_ref().unwrap_or(m0);
    let (w, h, d, nf) = (m.vg.width, m.vg.height, m.vg.depth, m.nframes);
    nifti_size_checks(g, out, "nifti1Write", h, d, nf)?;
    let mut hdr = NiftiHeader([0u8; 348]);
    hdr.i32_at(0, 348);
    for t in 0..8 {
        hdr.dim(t, 1);
        hdr.pixdim(t, 1.0);
    }
    hdr.dim(0, if nf == 1 { 3 } else { 4 });
    if w < SHORTMAX {
        hdr.dim(1, w as i16);
    } else {
        hdr.dim(1, -1);
        hdr.i32_at(144, w);
    }
    hdr.dim(2, h as i16);
    hdr.dim(3, d as i16);
    hdr.dim(4, nf as i16);
    hdr.pixdim(1, m.vg.xsize);
    hdr.pixdim(2, m.vg.ysize);
    hdr.pixdim(3, m.vg.zsize);
    hdr.pixdim(4, (f64::from(m.tr) / 1000.0) as f32);
    let (datatype, bitpix): (i16, i16) = match m.mtype {
        MRI_UCHAR => (DT_UNSIGNED_CHAR, 8),
        MRI_INT | mri::MRI_LONG => (DT_SIGNED_INT, 32),
        MRI_FLOAT => (DT_FLOAT, 32),
        MRI_SHORT => (DT_SIGNED_SHORT, 16),
        MRI_USHRT => (DT_UINT16, 16),
        mri::MRI_BITMAP => {
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                "nifti1Write(): data type MRI_BITMAP unsupported",
            );
            return Ok(ERROR_UNSUPPORTED);
        }
        mri::MRI_TENSOR => {
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                "nifti1Write(): data type MRI_TENSOR unsupported",
            );
            return Ok(ERROR_UNSUPPORTED);
        }
        t => {
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!("nifti1Write(): unknown data type {t}"),
            );
            return Ok(ERROR_BADPARM);
        }
    };
    hdr.i16_at(70, datatype);
    hdr.i16_at(72, bitpix);
    hdr.0[123] = 2 | 8; // NIFTI_UNITS_MM | NIFTI_UNITS_SEC
    let descrip = format!("FreeSurfer {BUILD_DATE}");
    hdr.0[148..148 + descrip.len()].copy_from_slice(descrip.as_bytes());
    let r = mri_to_nifti_qform(g, out, err, m, &mut hdr)?;
    if r != NO_ERROR {
        return Ok(r);
    }
    mri_to_nifti_sform(g, out, err, m, &mut hdr)?;
    hdr.0[344..348].copy_from_slice(b"ni1\0");

    // the names: the stem without a final `.img` or `.hdr` (after the last dot of the whole name)
    let fb = fname.as_bytes();
    let mut stem = fb.to_vec();
    if let Some(p) = stem.iter().rposition(|&c| c == b'.')
        && matches!(&stem[p..], b".img" | b".hdr")
    {
        stem.truncate(p);
    }
    if fb.len() >= STRLEN {
        // the copy into the 1,024-byte buffer: the C library's overflow check ends the program
        g.notices.add(
            &FS_451,
            format!(
                "nifti1Write: a name of {} bytes, which FreeSurfer's 1,024-byte buffer cannot hold (the program aborts)",
                fb.len()
            ),
        );
        if g.notices.reproduce() {
            let _ = err.write_all(b"*** buffer overflow detected ***: terminated\n");
            return Err(Fatal::Exit(134));
        }
    }
    let name_of = |g: &mut Globals, err: &mut dyn Write, suffix: &[u8], var: &str| {
        let mut n = [stem.as_slice(), suffix].concat();
        if fb.len() < STRLEN && n.len() >= STRLEN {
            g.notices.add(
                &FS_451,
                format!(
                    "nifti1Write: the {} name of {} bytes, which FreeSurfer cuts to 1,023",
                    &var[..3],
                    n.len()
                ),
            );
            if g.notices.reproduce() {
                let _ = writeln!(err, "nifti1Write: Truncation of {var}");
                n.truncate(STRLEN - 1);
            }
        }
        String::from_utf8_lossy(&n).into_owned()
    };
    let hdr_fname = name_of(g, err, b".hdr", "hdr_fname");
    let img_fname = name_of(g, err, b".img", "img_fname");

    let Ok(mut fp) = CFile::open(dir, &hdr_fname, "w") else {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADFILE,
            &format!("nifti1Write(): error opening file {hdr_fname}"),
        ));
    };
    if fp.fwrite(&hdr.0, 348, 1) != 1 {
        fp.close();
        return Ok(fail_code(
            g,
            err,
            ERROR_BADFILE,
            &format!("nifti1Write(): error writing header to {hdr_fname}"),
        ));
    }
    if fp.close() != 0 && unchecked_close(g, &hdr_fname) {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADFILE,
            &format!("nifti1Write(): error writing header to {hdr_fname}"),
        ));
    }
    let Ok(mut fp) = CFile::open(dir, &img_fname, "w") else {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADFILE,
            &format!("nifti1Write(): error opening file {img_fname}"),
        ));
    };
    // a volume without voxels: FreeSurfer takes its first row's address from slices it does not
    // have (a crash)
    if m.chunk.is_none() && i64::from(d.max(0)) * i64::from(nf.max(0)) * i64::from(h.max(0)) > 0 {
        g.notices.add(
            &FS_58,
            "nifti1Write of a volume made without voxels".to_string(),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "nifti1Write(): the volume has no voxels (FreeSurfer crashes here)\n",
        )?;
    }
    let unit = (bitpix / 8) as usize;
    let as_ints = long_rows_as_ints(g, m, "nifti1Write");
    let wn = w.max(0) as usize;
    for t in 0..nf {
        for k in 0..d {
            for j in 0..h {
                let row = ((t * d + k) * h + j) as usize;
                let b = nifti_row(g, m, row, unit, as_ints, "nifti1Write");
                if fp.fwrite(&b, unit, wn) != wn {
                    fp.close();
                    return Ok(fail_code(
                        g,
                        err,
                        ERROR_BADFILE,
                        &format!("nifti1Write(): error writing data to {img_fname}"),
                    ));
                }
            }
        }
    }
    if fp.close() != 0 && unchecked_close(g, &img_fname) {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADFILE,
            &format!("nifti1Write(): error writing data to {img_fname}"),
        ));
    }
    Ok(NO_ERROR)
}

/// FS-452: a close of one of `nifti1Write`'s files that failed, which FreeSurfer does not check:
/// noted; whether it is an error (the default).
fn unchecked_close(g: &mut Globals, name: &str) -> bool {
    g.notices.add(
        &FS_452,
        format!("nifti1Write of {name}: the close failed (FreeSurfer reports success)"),
    );
    g.notices.fix()
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    fn volume(g: &mut Globals, mtype: i32, nframes: i32) -> Mri {
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let mut m = mri::mri_alloc_sequence(g, &mut o, &mut e, 2, 1, 1, mtype, nframes).unwrap();
        let n = m.chunk.as_ref().unwrap().len();
        let c = m.chunk.as_mut().unwrap();
        for (i, b) in c.iter_mut().enumerate().take(n) {
            *b = (i * 37 + 5) as u8;
        }
        m
    }

    fn write(
        mode: UpstreamDefects,
        env: &Environment,
        m: &mut Mri,
        name: &str,
        frame: i32,
    ) -> (i32, Vec<u8>, Vec<&'static str>) {
        let dir = tempfile::Builder::new()
            .prefix("mgh w ~")
            .tempdir()
            .unwrap();
        let mut g = Globals::new(mode);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = mgh_write(
            &mut g,
            &mut o,
            &mut e,
            env,
            Some(dir.path()),
            m,
            name,
            frame,
            -1,
        )
        .unwrap();
        let b = std::fs::read(dir.path().join(name)).unwrap_or_default();
        let ids = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
        (r, b, ids)
    }

    #[test]
    fn an_mgh_file_is_header_voxels_scan_parameters_and_tags() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut m = volume(&mut g, MRI_FLOAT, 1);
        m.tr = 2.0;
        let env = Environment::default();
        let (r, b, ids) = write(UpstreamDefects::Fix, &env, &mut m, "a.mgh", -1);
        assert_eq!((r, ids.len()), (0, 0));
        assert_eq!(&b[..8], &[0, 0, 0, 1, 0, 0, 0, 2]);
        assert_eq!(&b[28..30], &[0, 1]); // the RAS flag
        // the voxels: the buffer's floats big-endian
        let c = m.chunk.as_ref().unwrap();
        assert_eq!(&b[284..288], &[c[3], c[2], c[1], c[0]]);
        assert_eq!(&b[292..296], &2.0f32.to_be_bytes());
        // the phase-encoding direction (UNKNOWN, no NUL) and the field strength's tags
        let tags = &b[312..];
        assert_eq!(&tags[..12], &[0, 0, 0, 41, 0, 0, 0, 0, 0, 0, 0, 7]);
        assert_eq!(&tags[12..19], b"UNKNOWN");
        assert_eq!(&tags[19..31], &[0, 0, 0, 43, 0, 0, 0, 0, 0, 0, 0, 4]);
        assert_eq!(tags.len(), 35);
    }

    #[test]
    fn fs_439_an_unsupported_type_leaves_an_mgz_empty_only_with_reproduce() {
        let env = Environment::default();
        for (mode, empty) in [
            (UpstreamDefects::Fix, false),
            (UpstreamDefects::Reproduce, true),
        ] {
            let mut g = Globals::new(mode);
            let mut m = volume(&mut g, mri::MRI_RGB, 1);
            let (r, b, ids) = write(mode, &env, &mut m, "a.mgz", -1);
            assert_eq!(r, ERROR_UNSUPPORTED);
            assert_eq!(b.is_empty(), empty);
            assert!(ids.contains(&"FS-439"));
        }
    }

    #[test]
    fn fs_442_one_frame_counts_one_frame_by_default() {
        let env = Environment::default();
        for (mode, count) in [(UpstreamDefects::Fix, 1u8), (UpstreamDefects::Reproduce, 3)] {
            let mut g = Globals::new(mode);
            let mut m = volume(&mut g, MRI_UCHAR, 3);
            let (r, b, ids) = write(mode, &env, &mut m, "a.mgh", 1);
            assert_eq!((r, b[19]), (0, count));
            assert_eq!(&b[284..286], &m.chunk.as_ref().unwrap()[2..4]);
            assert!(ids.contains(&"FS-442"));
        }
    }

    #[test]
    fn fs_443_the_voxel_buffer_write_swaps_the_volume_only_with_reproduce() {
        let env = Environment::from_pairs([("FS_MGZIO_USEVOXELBUFWRITE", "1")]);
        for (mode, swapped) in [
            (UpstreamDefects::Fix, false),
            (UpstreamDefects::Reproduce, true),
        ] {
            let mut g = Globals::new(mode);
            let mut m = volume(&mut g, mri::MRI_SHORT, 1);
            let before = m.chunk.clone().unwrap();
            let (r, b, _) = write(mode, &env, &mut m, "a.mgh", -1);
            assert_eq!(r, 0);
            assert_eq!(&b[284..286], &[before[1], before[0]]);
            assert_eq!(m.chunk.as_ref().unwrap()[0] == before[1], swapped);
        }
    }

    fn nii(mode: UpstreamDefects, m: &mut Mri) -> (Result<i32, Fatal>, Vec<u8>) {
        let dir = tempfile::Builder::new()
            .prefix("nii w ~")
            .tempdir()
            .unwrap();
        let mut g = Globals::new(mode);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let env = Environment::default();
        let r = nii_write(
            &mut g,
            &mut o,
            &mut e,
            &env,
            Some(dir.path()),
            m,
            "a.nii",
            -1,
        );
        (
            r,
            std::fs::read(dir.path().join("a.nii")).unwrap_or_default(),
        )
    }

    #[test]
    fn a_nifti_file_is_header_extension_and_voxels() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut m = volume(&mut g, MRI_FLOAT, 1);
        let (r, b) = nii(UpstreamDefects::Fix, &mut m);
        assert_eq!(r, Ok(0));
        assert_eq!(&b[0..4], &348i32.to_le_bytes());
        assert_eq!(&b[344..348], b"n+1\0");
        assert_eq!(&b[148..170], b"FreeSurfer Mar 14 2026");
        let vox = f32::from_le_bytes(b[108..112].try_into().unwrap()) as usize;
        assert_eq!(
            (vox % 16, b[348], &b[356..360]),
            (0, 1, &14i32.to_le_bytes()[..])
        );
        assert_eq!(&b[vox..], &m.chunk.as_ref().unwrap()[..]);
    }

    #[test]
    fn fs_447_a_long_volume_is_clamped_by_default() {
        for (mode, want) in [
            (UpstreamDefects::Fix, i32::MAX.to_le_bytes()),
            (UpstreamDefects::Reproduce, [0, 0, 0, 0]),
        ] {
            let mut g = Globals::new(mode);
            let mut m = volume(&mut g, mri::MRI_LONG, 1);
            let c = m.chunk.as_mut().unwrap();
            c[..8].copy_from_slice(&(1i64 << 40).to_le_bytes());
            let (r, b) = nii(mode, &mut m);
            assert_eq!(r, Ok(0));
            let vox = f32::from_le_bytes(b[108..112].try_into().unwrap()) as usize;
            assert_eq!(&b[vox..vox + 4], &want);
        }
    }

    #[test]
    fn fs_449_32768_rows_are_refused_by_default() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let mut g = Globals::new(mode);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let mut m =
                mri::mri_alloc_sequence(&mut g, &mut o, &mut e, 1, 32768, 1, MRI_UCHAR, 1).unwrap();
            let (r, b) = nii(mode, &mut m);
            if mode == UpstreamDefects::Fix {
                assert_eq!(r, Err(Fatal::Exit(1)));
            } else {
                assert_eq!(r, Ok(0));
                assert_eq!(&b[44..46], &(-32768i16).to_le_bytes());
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn fs_440_a_full_disk_is_an_error_by_default() {
        let dir = tempfile::Builder::new()
            .prefix("mgh f ~")
            .tempdir()
            .unwrap();
        std::os::unix::fs::symlink("/dev/full", dir.path().join("a.mgh")).unwrap();
        for (mode, want) in [
            (UpstreamDefects::Fix, ERROR_BADFILE),
            (UpstreamDefects::Reproduce, 0),
        ] {
            let mut g = Globals::new(mode);
            let mut m = volume(&mut g, MRI_FLOAT, 1);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let env = Environment::default();
            let r = mgh_write(
                &mut g,
                &mut o,
                &mut e,
                &env,
                Some(dir.path()),
                &mut m,
                "a.mgh",
                -1,
                -1,
            )
            .unwrap();
            assert_eq!(r, want);
        }
    }

    /// A pair write's status, standard error, files (name and bytes) and whether FS-451 was noted.
    type PairWrite = (Result<i32, Fatal>, Vec<u8>, Vec<(String, Vec<u8>)>, bool);

    /// `nifti1Write` of `m` as `name` in a new folder (`sub` made in it first): the status, the
    /// standard error and the files.
    fn pair(mode: UpstreamDefects, m: &mut Mri, sub: &str, name: &str) -> PairWrite {
        let dir = tempfile::Builder::new()
            .prefix("ni1 w ~")
            .tempdir()
            .unwrap();
        if !sub.is_empty() {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
        }
        let mut g = Globals::new(mode);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = nifti1_write(&mut g, &mut o, &mut e, Some(dir.path()), m, name);
        let folder = dir.path().join(sub);
        let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(&folder)
            .unwrap()
            .filter_map(|x| x.ok())
            .filter(|x| x.path().is_file())
            .map(|x| {
                (
                    x.file_name().to_string_lossy().into_owned(),
                    std::fs::read(x.path()).unwrap(),
                )
            })
            .collect();
        files.sort();
        (r, e, files, g.notices.contains(&FS_451))
    }

    #[test]
    fn a_nifti1_pair_is_a_header_and_an_image() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut m = volume(&mut g, MRI_SHORT, 2);
        for name in ["a.img", "a.hdr", "a"] {
            let (r, e, files, _) = pair(UpstreamDefects::Fix, &mut m, "", name);
            assert_eq!((r, e.len()), (Ok(0), 0));
            let names: Vec<&str> = files.iter().map(|f| f.0.as_str()).collect();
            assert_eq!(names, ["a.hdr", "a.img"]);
            let h = &files[0].1;
            assert_eq!(h.len(), 348);
            assert_eq!(&h[344..348], b"ni1\0");
            assert_eq!(&h[108..112], &[0, 0, 0, 0]); // the voxel offset
            assert_eq!(&h[40..42], &4i16.to_le_bytes()); // 4 dimensions: 2 frames
            assert_eq!(&h[70..74], &[4, 0, 16, 0]); // signed short, 16 bits
            assert_eq!(&h[148..170], b"FreeSurfer Mar 14 2026");
            assert_eq!(&files[1].1, m.chunk.as_ref().unwrap());
        }
        // `.IMG` is not an extension the writer cuts; a complex volume is an unknown type here
        let (r, _, files, _) = pair(UpstreamDefects::Fix, &mut m, "", "A.IMG");
        assert_eq!(r, Ok(0));
        assert_eq!(files[0].0, "A.IMG.hdr");
        let mut c = volume(&mut g, MRI_FLOAT_COMPLEX, 1);
        let (r, e, files, _) = pair(UpstreamDefects::Fix, &mut c, "", "c.img");
        assert_eq!((r, files.len()), (Ok(ERROR_BADPARM), 0));
        assert!(String::from_utf8_lossy(&e).contains("nifti1Write(): unknown data type 7"));
    }

    // macOS refuses a path of more than 1,024 bytes (its PATH_MAX; ENAMETOOLONG), and these names of 1,019 to
    // 1,024 bytes sit in a temporary folder: the test needs Linux's or Windows' longer paths (CI, 2026-11-14).
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn fs_451_long_names_cut_or_abort_only_with_reproduce() {
        // four folders of 200 bytes keep every name within the file system's 255 bytes
        let sub = ["a", "b", "c", "d"].map(|c| c.repeat(200)).join("/");
        let top = format!("{sub}/");
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut m = volume(&mut g, MRI_UCHAR, 1);
        for (len, cut) in [(1019usize, false), (1021, true), (1023, true)] {
            let name = format!("{top}{}", "x".repeat(len - top.len()));
            let (r, e, files, noted) = pair(UpstreamDefects::Reproduce, &mut m, &sub, &name);
            assert_eq!((r, noted), (Ok(0), cut));
            let leaf = &name[top.len()..];
            let names: Vec<String> = files.iter().map(|f| f.0.clone()).collect();
            if len == 1019 {
                assert_eq!(names, [format!("{leaf}.hdr"), format!("{leaf}.img")]);
            } else if len == 1021 {
                assert_eq!(names, [format!("{leaf}.h"), format!("{leaf}.i")]);
                assert_eq!(
                    e,
                    b"nifti1Write: Truncation of hdr_fname\nnifti1Write: Truncation of img_fname\n"
                );
            } else {
                // one file: the image written over the header
                assert_eq!(names, [leaf.to_string()]);
                assert_eq!(files[0].1, m.chunk.clone().unwrap());
            }
            let (r, e, files, noted) = pair(UpstreamDefects::Fix, &mut m, &sub, &name);
            assert_eq!((r, e.len(), noted), (Ok(0), 0, cut));
            assert_eq!(
                files.iter().map(|f| f.0.clone()).collect::<Vec<_>>(),
                [format!("{leaf}.hdr"), format!("{leaf}.img")]
            );
        }
        let name = format!("{top}{}.img", "y".repeat(1024 - top.len() - 4));
        let (r, e, files, _) = pair(UpstreamDefects::Reproduce, &mut m, &sub, &name);
        assert_eq!((r, files.len()), (Err(Fatal::Exit(134)), 0));
        assert_eq!(e, b"*** buffer overflow detected ***: terminated\n");
        let (r, _, files, noted) = pair(UpstreamDefects::Fix, &mut m, &sub, &name);
        assert_eq!((r, files.len(), noted), (Ok(0), 2, true));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn fs_452_a_full_disk_is_an_error_by_default() {
        let dir = tempfile::Builder::new()
            .prefix("ni1 f ~")
            .tempdir()
            .unwrap();
        std::os::unix::fs::symlink("/dev/full", dir.path().join("a.hdr")).unwrap();
        for (mode, want) in [
            (UpstreamDefects::Fix, ERROR_BADFILE),
            (UpstreamDefects::Reproduce, 0),
        ] {
            let mut g = Globals::new(mode);
            let mut m = volume(&mut g, MRI_FLOAT, 1);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let r =
                nifti1_write(&mut g, &mut o, &mut e, Some(dir.path()), &mut m, "a.img").unwrap();
            assert_eq!(r, want);
            assert!(g.notices.contains(&FS_452));
        }
    }
}
