// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MRIcopy` (`utils/mri.cpp` 5783): a volume's header and voxels into another (a new one when
//! none is given).
//!
//! FreeSurfer copies through the slice and row pointers: the same type row by row (`memmove` of
//! the source's row length), a few conversions element by element (`float` to `short`, `unsigned
//! short` and `unsigned char` through `nint`, `unsigned char`, `short` and `int` to `float`, `short`
//! to `unsigned char`), every other pair through `MRIgetVoxVal` and `MRIsetVoxVal` over the
//! destination's extent. The source's slice `z + frame depth` and row `y` are written to the
//! destination's slice of the same index and row (the same-type copy) or to its slice `z + frame
//! dst_depth` (the conversions), so a destination of another extent or with fewer frames gets the
//! rows in other places or is written outside (FS-163). FS-161 (the same-type copy of tensor and
//! complex volumes), FS-162 (the conversions' out-of-range values), FS-166 (a tensor volume's rows).

use std::io::Write;

use larmorx_core::defects::{FS_58, FS_161, FS_162, FS_163, FS_166};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::utils::nint;

use crate::mri::{
    MRI_FLOAT, MRI_FLOAT_COMPLEX, MRI_INT, MRI_LONG, MRI_RGB, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri,
    mri_alloc_header, mri_alloc_sequence, mri_copy_header,
};
use crate::voxel::{mri_get_vox_val, mri_set_vox_val};

/// The types whose rows `initSlices` points at their places (every other type's rows all point at
/// the first voxel: FS-166).
const ROWS_STEPPED: [i32; 8] = [
    MRI_UCHAR,
    MRI_SHORT,
    MRI_USHRT,
    MRI_RGB,
    MRI_INT,
    MRI_LONG,
    MRI_FLOAT,
    MRI_FLOAT_COMPLEX,
];

/// The byte offset of `slices[k][y]` in a volume's buffer, or `None` where FreeSurfer reads a slice
/// or row pointer it does not have (an index beyond its slices or rows: memory contents).
fn row_offset(m: &Mri, k: i64, y: i64) -> Option<u64> {
    let slices = i64::from(m.vg.depth) * i64::from(m.nframes);
    if m.chunk.is_none() || !(0..slices).contains(&k) || !(0..i64::from(m.vg.height)).contains(&y) {
        return None;
    }
    let vox = k as u64 * m.vox_per_slice + y as u64 * m.vox_per_row;
    Some(vox * m.bytes_per_vox as u64)
}

/// FS-163: the destination has no row, or no room, where FreeSurfer writes (both modes an error).
fn outside(g: &mut Globals, err: &mut dyn Write) -> Result<(), Fatal> {
    g.notices.add(
        &FS_163,
        "MRIcopy wrote outside a smaller destination (FreeSurfer writes memory it does not own)",
    );
    log::error_exit(
        g,
        err,
        ERROR_BADPARM,
        "MRIcopy: the destination is too small for the source's voxels\n",
    )
}

/// The `len` bytes at `off` of the destination's buffer, when they lie in it.
fn dst_bytes(d: &mut Mri, off: u64, len: u64) -> Option<&mut [u8]> {
    let total = d.bytes_total;
    let c = d.chunk.as_mut()?;
    (off.checked_add(len)? <= total && (off + len) as usize <= c.len())
        .then(|| &mut c[off as usize..(off + len) as usize])
}

/// The source's bytes at `off` (always in its buffer: the loops run over its own extent).
fn src_bytes(s: &Mri, off: u64, len: u64) -> &[u8] {
    &s.chunk.as_ref().expect("voxels")[off as usize..(off + len) as usize]
}

/// `MRIcopy(mri_src, mri_dst)`: the source's header (`MRIcopyHeader`, the destination keeping its
/// `ptype`) and voxels into `dst`, or into a new volume (`MRIallocSequence` of the source's extent,
/// type and frames; `MRIallocHeader` with the source's frame count when the source has no voxels,
/// which are then not copied). FreeSurfer's `mri_src == mri_dst` returns at once: a volume cannot
/// be both here. By default a given destination of another extent or frame count is refused
/// (FS-163); with `reproduce` FreeSurfer's copy where it stays in the destination, an error where
/// FreeSurfer writes outside it (both modes). A destination without voxels for a source with them
/// is an error (FS-58: FreeSurfer crashes).
pub fn mri_copy(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (width, height, depth) = (src.vg.width, src.vg.height, src.vg.depth);
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => {
            if src.chunk.is_some() {
                fresh.insert(mri_alloc_sequence(
                    g,
                    out,
                    err,
                    width,
                    height,
                    depth,
                    src.mtype,
                    src.nframes,
                )?)
            } else {
                let m = fresh.insert(mri_alloc_header(
                    g, out, err, width, height, depth, src.mtype, 1,
                )?);
                m.nframes = src.nframes;
                m
            }
        }
    };
    let dest_ptype = d.ptype;
    mri_copy_header(g, out, err, src, Some(d))?;
    d.ptype = dest_ptype;
    let done = |fresh: Option<Mri>| match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    };
    if src.chunk.is_none() {
        return Ok(done(fresh));
    }
    if d.chunk.is_none() {
        g.notices.add(
            &FS_58,
            "MRIcopy into a volume without voxels (FreeSurfer crashes)",
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "MRIcopy: the destination has no voxels\n",
        )?;
        return Ok(done(fresh));
    }
    let same_extent =
        (d.vg.width, d.vg.height, d.vg.depth, d.nframes) == (width, height, depth, src.nframes);
    if !same_extent {
        g.notices.add(
            &FS_163,
            format!(
                "MRIcopy of a {width} x {height} x {depth} x {} volume into a {} x {} x {} x {} one",
                src.nframes, d.vg.width, d.vg.height, d.vg.depth, d.nframes
            ),
        );
        if !g.notices.reproduce() {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "MRIcopy: a {} x {} x {} x {} destination for a {width} x {height} x {depth} x {} source\n",
                    d.vg.width, d.vg.height, d.vg.depth, d.nframes, src.nframes
                ),
            )?;
            return Ok(done(fresh));
        }
    }
    if src.mtype == d.mtype {
        copy_same_type(g, err, src, d)?;
    } else {
        convert(g, err, src, d)?;
    }
    d.vg.fname.strcpy(&src.vg.fname);
    Ok(done(fresh))
}

/// The same-type copy: each row's `width x sizeof(type)` bytes (`width` bytes for the types the
/// size switch does not list: FS-161).
fn copy_same_type(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    d: &mut Mri,
) -> Result<(), Fatal> {
    let width = src.vg.width as i64 as u64;
    let listed = match src.mtype {
        MRI_UCHAR => Some(1),
        MRI_SHORT | MRI_USHRT => Some(2),
        MRI_FLOAT | MRI_RGB | MRI_INT => Some(4),
        MRI_LONG => Some(8),
        _ => None,
    };
    let bytes = match listed {
        Some(s) => width.wrapping_mul(s),
        None => {
            let full = width.wrapping_mul(src.bytes_per_vox as u64);
            if full != width && src.vg.width > 0 && src.vg.height > 0 && src.vg.depth > 0 {
                g.notices.add(
                    &FS_161,
                    format!(
                        "MRIcopy of a volume of type {} (FreeSurfer copies {width} of each row's {full} bytes)",
                        src.mtype
                    ),
                );
            }
            if g.notices.reproduce() { width } else { full }
        }
    };
    // FS-166: initSlices leaves every row pointer of a type its switch does not list (a tensor
    // volume) at the first voxel
    let stuck = !ROWS_STEPPED.contains(&src.mtype);
    if stuck && (i64::from(src.vg.height) * i64::from(src.vg.depth) * i64::from(src.nframes) > 1) {
        g.notices.add(
            &FS_166,
            format!(
                "MRIcopy of a volume of type {} row by row (FreeSurfer's rows all start at its first voxel)",
                src.mtype
            ),
        );
    }
    let at_start = stuck && g.notices.reproduce();
    for frame in 0..i64::from(src.nframes) {
        for z in 0..i64::from(src.vg.depth) {
            for y in 0..i64::from(src.vg.height) {
                let k = z + frame * i64::from(src.vg.depth);
                let so = row_offset(src, k, y).expect("the source's own row");
                let Some(doff) = row_offset(d, k, y) else {
                    return outside(g, err);
                };
                let (so, doff) = if at_start { (0, 0) } else { (so, doff) };
                let row = src_bytes(src, so, bytes).to_vec();
                match dst_bytes(d, doff, bytes) {
                    Some(t) => t.copy_from_slice(&row),
                    None => return outside(g, err),
                }
            }
        }
    }
    Ok(())
}

/// A conversion's integer result of FreeSurfer's `(T) val` (`T` 16 or 8 bits: the low bits, as
/// GCC converts), or by default the value clamped to `T`'s range (FS-162) when it is outside.
fn narrow(g: &mut Globals, val: i64, lo: i64, hi: i64, bits: u32, what: &str) -> i64 {
    if (lo..=hi).contains(&val) {
        return val;
    }
    g.notices.add(
        &FS_162,
        format!("MRIcopy converted {val} to {what}, outside its range"),
    );
    if g.notices.reproduce() {
        let m = val & ((1i64 << bits) - 1);
        if lo < 0 && m >= 1i64 << (bits - 1) {
            m - (1i64 << bits)
        } else {
            m
        }
    } else {
        val.clamp(lo, hi)
    }
}

/// The element conversions and the voxel-by-voxel copy of the other pairs.
fn convert(g: &mut Globals, err: &mut dyn Write, src: &Mri, d: &mut Mri) -> Result<(), Fatal> {
    let (st, dt) = (src.mtype, d.mtype);
    let direct = matches!(
        (st, dt),
        (MRI_FLOAT, MRI_SHORT | MRI_USHRT | MRI_UCHAR)
            | (MRI_UCHAR, MRI_FLOAT)
            | (MRI_SHORT, MRI_FLOAT | MRI_UCHAR)
            | (MRI_RGB | MRI_INT, MRI_FLOAT)
    );
    if !direct {
        // for x, y, z, frame over the destination: MRIsetVoxVal(dst, ..., MRIgetVoxVal(src, ...))
        for x in 0..d.vg.width {
            for y in 0..d.vg.height {
                for z in 0..d.vg.depth {
                    for f in 0..d.nframes {
                        let v = mri_get_vox_val(g, err, src, x, y, z, f, 0)?;
                        mri_set_vox_val(g, err, d, x, y, z, f, v, 0)?;
                    }
                }
            }
        }
        return Ok(());
    }
    let ssize = src.bytes_per_vox as u64;
    let dsize = d.bytes_per_vox as u64;
    let width = src.vg.width as i64 as u64;
    for frame in 0..i64::from(src.nframes) {
        for z in 0..i64::from(src.vg.depth) {
            for y in 0..i64::from(src.vg.height) {
                let so = row_offset(src, z + frame * i64::from(src.vg.depth), y)
                    .expect("the source's own row");
                let Some(doff) = row_offset(d, z + frame * i64::from(d.vg.depth), y) else {
                    return outside(g, err);
                };
                let row = src_bytes(src, so, width * ssize).to_vec();
                let mut conv = Vec::with_capacity((width * dsize) as usize);
                for e in row.chunks_exact(ssize as usize) {
                    match (st, dt) {
                        (MRI_FLOAT, _) => {
                            let f = f32::from_le_bytes([e[0], e[1], e[2], e[3]]);
                            let val = i64::from(nint(g, f64::from(f)));
                            match dt {
                                MRI_SHORT => {
                                    let v = narrow(g, val, -32768, 32767, 16, "short");
                                    conv.extend_from_slice(&(v as i16).to_le_bytes());
                                }
                                MRI_USHRT => {
                                    let v = narrow(g, val, 0, 65535, 16, "unsigned short");
                                    conv.extend_from_slice(&(v as u16).to_le_bytes());
                                }
                                _ => {
                                    let v = narrow(g, val.min(255), 0, 255, 8, "unsigned char");
                                    conv.push(v as u8);
                                }
                            }
                        }
                        (MRI_UCHAR, _) => conv.extend_from_slice(&f32::from(e[0]).to_le_bytes()),
                        (MRI_SHORT, MRI_FLOAT) => {
                            let s = i16::from_le_bytes([e[0], e[1]]);
                            conv.extend_from_slice(&f32::from(s).to_le_bytes());
                        }
                        (MRI_SHORT, _) => {
                            // (unsigned char)(float)s: undefined outside [0, 255]; GCC keeps the
                            // low byte
                            let s = i64::from(i16::from_le_bytes([e[0], e[1]]));
                            conv.push(narrow(g, s, 0, 255, 8, "unsigned char") as u8);
                        }
                        _ => {
                            let i = i32::from_le_bytes([e[0], e[1], e[2], e[3]]);
                            conv.extend_from_slice(&(i as f32).to_le_bytes());
                        }
                    }
                }
                match dst_bytes(d, doff, width * dsize) {
                    Some(t) => t.copy_from_slice(&conv),
                    None => return outside(g, err),
                }
            }
        }
    }
    Ok(())
}
