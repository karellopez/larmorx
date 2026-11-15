// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! NRRD volumes (`utils/mriio.cpp` 9787-9975, item 3.1-27): `mriNrrdRead` ([`mri_nrrd_read`]) over
//! NrrdIO's `nrrdLoad` (`larmorx_io::nrrd::load_with`, track C's port of NrrdIO r3991 as FreeSurfer
//! 8.2.0 builds its copy). Wired into [`crate::mriio::mri_read`].
//!
//! The file is read whole whatever `read_volume` says (FreeSurfer ignores it). A volume of three
//! or four axes in a three-dimensional space is taken: the first three axes are the columns, rows
//! and slices, a fourth the frames; anything else is refused after its description is printed
//! (`Nrrd input of diffusion data not supported!`). The key/value pairs are printed; the samples
//! of 8-, 16-, 32- and 64-bit integers and floats are taken (doubles and blocks refused).
//!
//! Defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-io.md`, Part 26):
//! FS-734 (samples a FreeSurfer type cannot hold stored wrapped: signed bytes as unsigned,
//! unsigned 16- and 32-bit as signed), FS-735 (unsigned 64-bit samples past the signed range),
//! FS-736 (the geometry: the space origin taken as the centre, the voxel sizes from the diagonal
//! of the directions, the axes from the space's name), FS-737 (a file without a space origin: a
//! NaN centre), FS-738 (an axis without a space direction: NaN voxel sizes), FS-739 (a volume of
//! 2^31 samples or more, or an axis past 2^31 - 1, indexed in 32 bits), FS-740 (the refusal's
//! element count and size printed modulo 2^32), FS-249 (names past FreeSurfer's buffer). NrrdIO's
//! own defects (FS-518 to FS-520, FS-761 to FS-773) follow the same mode inside `load_with`.

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt::Writers;
use larmorx_core::defects::{FS_734, FS_735, FS_736, FS_737, FS_738, FS_739, FS_740, FS_741};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_mri::mri::{self, MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, Mri};
use larmorx_freesurfer_mri::vol_geom;
use larmorx_io::nrrd::{self, LoadError, Nrrd, spaces, types};

use crate::mriio_nifti::narrow;

fn put(out: &mut dyn Write, parts: &[&[u8]]) {
    for p in parts {
        let _ = out.write_all(p);
    }
}

/// The `MRI` type FreeSurfer stores a NRRD type in (`None`: not taken).
fn mri_type_of(t: i32) -> Option<i32> {
    Some(match t {
        types::CHAR | types::UCHAR => MRI_UCHAR,
        types::SHORT | types::USHORT => MRI_SHORT,
        types::INT | types::UINT => MRI_INT,
        types::LLONG | types::ULLONG => MRI_LONG,
        types::FLOAT => MRI_FLOAT,
        _ => return None,
    })
}

/// The samples' bytes in `size`-byte pieces.
fn samples(data: &[u8], size: usize, n: usize) -> impl Iterator<Item = &[u8]> {
    data[..n * size].chunks_exact(size)
}

/// FS-734/FS-735 corrected: the type that holds every sample (FreeSurfer's when it does), and
/// the samples converted to it; `Err` for unsigned 64-bit samples past the signed range.
fn corrected_type(nin: &Nrrd, data: &[u8], n: usize) -> Result<Option<(i32, Vec<u8>)>, String> {
    match nin.type_ {
        types::CHAR => {
            let least = data[..n].iter().map(|&b| b as i8).min().unwrap_or(0);
            if least >= 0 {
                return Ok(None);
            }
            let v: Vec<u8> = data[..n]
                .iter()
                .flat_map(|&b| i16::from(b as i8).to_le_bytes())
                .collect();
            Ok(Some((MRI_SHORT, v)))
        }
        types::USHORT => {
            if samples(data, 2, n).all(|s| u16::from_le_bytes([s[0], s[1]]) <= i16::MAX as u16) {
                return Ok(None);
            }
            let v: Vec<u8> = samples(data, 2, n)
                .flat_map(|s| i32::from(u16::from_le_bytes([s[0], s[1]])).to_le_bytes())
                .collect();
            Ok(Some((MRI_INT, v)))
        }
        types::UINT => {
            let get = |s: &[u8]| u32::from_le_bytes(s.try_into().expect("4 bytes"));
            if samples(data, 4, n).all(|s| get(s) <= i32::MAX as u32) {
                return Ok(None);
            }
            let v: Vec<u8> = samples(data, 4, n)
                .flat_map(|s| i64::from(get(s)).to_le_bytes())
                .collect();
            Ok(Some((MRI_LONG, v)))
        }
        types::ULLONG => {
            let get = |s: &[u8]| u64::from_le_bytes(s.try_into().expect("8 bytes"));
            match samples(data, 8, n).map(get).find(|&v| v > i64::MAX as u64) {
                Some(v) => Err(format!("{v}")),
                None => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

/// FS-734's notice: the samples' type and the least or greatest sample FreeSurfer stores wrapped.
fn wrapped_detail(nin: &Nrrd, data: &[u8], n: usize) -> String {
    match nin.type_ {
        types::CHAR => {
            let least = data[..n].iter().map(|&b| b as i8).min().unwrap_or(0);
            format!(
                "mriNrrdRead: signed 8-bit samples read as 16-bit integers (FreeSurfer: unsigned bytes, {least} read as {})",
                least as u8
            )
        }
        types::USHORT => {
            let most = samples(data, 2, n)
                .map(|s| u16::from_le_bytes([s[0], s[1]]))
                .max()
                .unwrap_or(0);
            format!(
                "mriNrrdRead: unsigned 16-bit samples read as 32-bit integers (FreeSurfer: signed 16-bit, {most} read as {})",
                most as i16
            )
        }
        _ => {
            let most = samples(data, 4, n)
                .map(|s| u32::from_le_bytes(s.try_into().expect("4 bytes")))
                .max()
                .unwrap_or(0);
            format!(
                "mriNrrdRead: unsigned 32-bit samples read as 64-bit integers (FreeSurfer: signed 32-bit, {most} read as {})",
                most as i32
            )
        }
    }
}

/// `MRIorientationStringToDircos(mri, "RAS" | "LAS" | "LPS")`: the axes' direction cosines.
fn named_dircos(m: &mut Mri, space: i32) {
    let (x, y) = match space {
        spaces::RAS => (1.0f32, 1.0f32),
        spaces::LAS => (-1.0, 1.0),
        _ => (-1.0, -1.0),
    };
    let vg = &mut m.vg;
    (vg.x_r, vg.x_a, vg.x_s) = (x, 0.0, 0.0);
    (vg.y_r, vg.y_a, vg.y_s) = (0.0, y, 0.0);
    (vg.z_r, vg.z_a, vg.z_s) = (0.0, 0.0, 1.0);
}

/// The signs that take a point or direction of the file's space to RAS: NrrdIO's RAS as it is,
/// LAS's first coordinate negated, LPS's first two; another space's coordinates are taken as LPS
/// (as ITK's NRRD reader takes them).
fn to_ras(space: i32) -> [f64; 3] {
    match space {
        spaces::RAS => [1.0, 1.0, 1.0],
        spaces::LAS => [-1.0, 1.0, 1.0],
        _ => [-1.0, -1.0, 1.0],
    }
}

/// FS-736 to FS-738 corrected: the geometry the file states, as ITK's NRRD reader takes it. Each
/// of the first three axes' space direction (in RAS: [`to_ras`]) gives its voxel size (the
/// vector's length) and direction cosines (the vector over it); an axis without a direction has
/// size 1 along its own LPS axis (ITK's default); the space origin is the first voxel's centre (0
/// without one). `Err(axis)`: a direction of length 0.
fn stated_geometry(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &mut Mri,
    nin: &Nrrd,
) -> Result<Result<(), usize>, Fatal> {
    let s = to_ras(nin.space);
    let mut cols = [[0.0f64; 3]; 3];
    let mut sizes = [0.0f64; 3];
    for (i, (col, size)) in cols.iter_mut().zip(sizes.iter_mut()).enumerate() {
        let d = &nin.axis[i].space_direction;
        if d[..3].iter().all(|v| v.is_finite()) {
            let v = [d[0] * s[0], d[1] * s[1], d[2] * s[2]];
            let len = length(v);
            if len == 0.0 {
                return Ok(Err(i));
            }
            *col = [v[0] / len, v[1] / len, v[2] / len];
            *size = len;
        } else {
            let mut v = [0.0f64; 3];
            v[i] = [-1.0, -1.0, 1.0][i];
            *col = v;
            *size = 1.0;
        }
    }
    let o = &nin.space_origin;
    let p0 = if o[..3].iter().all(|v| v.is_finite()) {
        [o[0] * s[0], o[1] * s[1], o[2] * s[2]]
    } else {
        [0.0; 3]
    };
    let vg = &mut m.vg;
    (vg.xsize, vg.ysize, vg.zsize) = (sizes[0] as f32, sizes[1] as f32, sizes[2] as f32);
    (vg.x_r, vg.x_a, vg.x_s) = (cols[0][0] as f32, cols[0][1] as f32, cols[0][2] as f32);
    (vg.y_r, vg.y_a, vg.y_s) = (cols[1][0] as f32, cols[1][1] as f32, cols[1][2] as f32);
    (vg.z_r, vg.z_a, vg.z_s) = (cols[2][0] as f32, cols[2][1] as f32, cols[2][2] as f32);
    vol_geom::p0_to_cras(g, out, err, vg, p0[0], p0[1], p0[2])?;
    Ok(Ok(()))
}

/// What FreeSurfer's geometry differs in from the file's (FS-736 to FS-738), for the notices.
fn geometry_faults(g: &mut Globals, nin: &Nrrd) {
    let o = &nin.space_origin;
    if !o[0].is_finite() {
        g.notices.add(
            &FS_737,
            "mriNrrdRead: no space origin: the first voxel's centre at 0 (FreeSurfer: a NaN centre)",
        );
    }
    let mut none = Vec::new();
    for i in 0..3 {
        if !nin.axis[i].space_direction[0].is_finite() {
            none.push(i.to_string());
        }
    }
    if !none.is_empty() {
        g.notices.add(
            &FS_738,
            format!(
                "mriNrrdRead: axis {} without a space direction: voxel size 1 along its LPS axis (FreeSurfer: NaN sizes)",
                none.join(", ")
            ),
        );
    }
    g.notices.add(
        &FS_736,
        format!(
            "mriNrrdRead: the geometry the file states ({} space; FreeSurfer: the origin as the centre, the sizes from the directions' diagonal, the axes from the space's name)",
            nrrd::space_name(nin.space)
        ),
    );
}

/// `mriNrrdRead(fname, read_volume)`: the NRRD file `fname` as a volume (`None`: refused, the
/// reason printed). `read_volume` is not used: the samples are always read.
pub fn mri_nrrd_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    _read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    let rep = g.notices.reproduce();
    // nrrdSanity() passes on every platform larmorx supports
    let path = larmorx_freesurfer_core::utils::resolve(dir, fname);
    let on_disk = path.to_string_lossy().into_owned();
    let mut biff = nrrd::Biff::new();
    let (loaded, errno) = {
        let mut streams = Writers {
            out: &mut *out,
            err: &mut *err,
        };
        nrrd::load_with_errno(on_disk.as_bytes(), &mut biff, &mut g.notices, &mut streams)
    };
    // what C's stdio and allocator left in errno while NrrdIO read
    if let Some(e) = errno {
        g.errno = e;
    }
    let nin = match loaded {
        Ok(n) => n,
        Err(LoadError::Exit { status, message }) => {
            let _ = err.write_all(&message);
            return Err(Fatal::Exit(status));
        }
        Err(LoadError::Failed { .. }) => {
            let mut text = biff.get_done(nrrd::NRRD).unwrap_or_default();
            if let Some(d) = dir {
                // the messages name the file as the program gave it (NrrdIO opened it in `dir`)
                if on_disk != fname {
                    let pre = format!("{}/", d.to_string_lossy());
                    text = replace_all(&text, pre.as_bytes(), b"");
                }
            }
            put(
                err,
                &[
                    b"mriNrrdRead: trouble reading \"",
                    fname.as_bytes(),
                    b"\":\n",
                    &text,
                ],
            );
            return Ok(None);
        }
    };
    if (nin.dim != 3 && nin.dim != 4) || nin.space_dim != 3 {
        let _ = writeln!(
            out,
            "mriNrrdRead: \"{fname}\" is a {}-dimensional nrrd of type {} ({})",
            nin.dim,
            nin.type_,
            nrrd::type_name(nin.type_)
        );
        let (num, size) = (nin.element_number(), nin.element_size());
        let (pn, ps) = if (num > i32::MAX as usize || size > i32::MAX as usize) && !rep {
            g.notices.add(
                &FS_740,
                format!(
                    "mriNrrdRead: {num} elements of {size} bytes printed whole (FreeSurfer: modulo 2^32, as {} and {})",
                    num as i32, size as i32
                ),
            );
            (num.to_string(), size.to_string())
        } else {
            ((num as i32).to_string(), (size as i32).to_string())
        };
        let _ = writeln!(
            out,
            "mriNrrdRead: the array contains {pn} elements, {ps} bytes in size"
        );
        if let Some(c) = &nin.content {
            put(out, &[b"mriNrrdRead: content: ", c, b"\n"]);
        }
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            "Nrrd input of diffusion data not supported!",
        );
        return Ok(None);
    }
    for (kvi, (key, val)) in nin.kvp.iter().enumerate() {
        put(
            out,
            &[
                format!("mriNrrdRead: key:value {kvi} = ").as_bytes(),
                key,
                b":",
                val,
                b"\n",
            ],
        );
    }
    let Some(mut mtype) = mri_type_of(nin.type_) else {
        let _ = writeln!(
            out,
            "mriNrrdRead: Unsupported type: {} ({})",
            nin.type_,
            nrrd::type_name(nin.type_)
        );
        return Ok(None);
    };
    let sizes: Vec<usize> = (0..nin.dim as usize).map(|i| nin.axis[i].size).collect();
    let n: usize = sizes.iter().product();
    // FS-739: FreeSurfer's extents and index are ints
    if let Some(big) = sizes.iter().position(|&s| s > i32::MAX as usize) {
        g.notices.add(
            &FS_739,
            format!(
                "mriNrrdRead: axis {big} of {} samples refused (FreeSurfer: its extent cut to 32 bits, reads outside its data)",
                sizes[big]
            ),
        );
        let _ = writeln!(
            err,
            "mriNrrdRead: axis {big} of \"{fname}\" has {} samples: more than a volume's extent can hold",
            sizes[big]
        );
        return Ok(None);
    }
    if n > i32::MAX as usize {
        g.notices.add(
            &FS_739,
            format!(
                "mriNrrdRead: {n} samples indexed in 64 bits (FreeSurfer: a 32-bit index, reads outside its data)"
            ),
        );
    }
    let esize = nin.element_size();
    let data = nin.data.as_deref().unwrap_or(&[]);
    let mut converted: Option<Vec<u8>> = None;
    match corrected_type(&nin, data, n) {
        Ok(Some((t, v))) => {
            let detail = wrapped_detail(&nin, data, n);
            g.notices.add(&FS_734, detail);
            if !rep {
                mtype = t;
                converted = Some(v);
            }
        }
        Ok(None) => {}
        Err(v) => {
            g.notices.add(
                &FS_735,
                format!(
                    "mriNrrdRead: an unsigned 64-bit sample of {v} refused (FreeSurfer: stored as {})",
                    v.parse::<u64>().unwrap_or(0) as i64
                ),
            );
            if !rep {
                let _ = writeln!(
                    err,
                    "mriNrrdRead: \"{fname}\" holds the unsigned 64-bit sample {v}: more than a 64-bit signed voxel holds"
                );
                return Ok(None);
            }
        }
    }
    let (width, height, depth) = (sizes[0] as i32, sizes[1] as i32, sizes[2] as i32);
    let nframes = if nin.dim == 4 { sizes[3] as i32 } else { 1 };
    let mut m = mri::mri_alloc_sequence(g, out, err, width, height, depth, mtype, nframes)?;
    let bytes = converted.as_deref().unwrap_or(&data[..n * esize]);
    let long = matches!(nin.type_, types::LLONG | types::ULLONG);
    if long {
        g.notices.add(
            &FS_741,
            "mriNrrdRead: 64-bit samples stored as they are (FreeSurfer: their low 32 bits packed into the first half of each row)",
        );
    }
    if let Some(chunk) = m.chunk.as_mut() {
        if long && rep && mtype == MRI_LONG {
            // FreeSurfer's 64-bit voxels written through 32-bit row pointers: sample x's low
            // half at byte 4 x of its row of 8-byte voxels
            let w = sizes[0];
            for (row, src) in bytes.chunks_exact(8 * w).enumerate() {
                let at = row * 8 * w;
                for (x, v) in src.chunks_exact(8).enumerate() {
                    chunk[at + 4 * x..at + 4 * x + 4].copy_from_slice(&v[..4]);
                }
            }
        } else {
            chunk[..bytes.len()].copy_from_slice(bytes);
        }
    }
    m.vg.ras_good_flag = 1;
    if rep {
        let o = &nin.space_origin;
        (m.vg.c_r, m.vg.c_a, m.vg.c_s) = (narrow(o[0]), narrow(o[1]), narrow(o[2]));
        let d = |i: usize| narrow(nin.axis[i].space_direction[i]);
        (m.vg.xsize, m.vg.ysize, m.vg.zsize) = (d(0), d(1), d(2));
        if matches!(nin.space, spaces::RAS | spaces::LAS | spaces::LPS) {
            named_dircos(&mut m, nin.space);
        }
        geometry_faults(g, &nin);
    } else {
        match stated_geometry(g, out, err, &mut m, &nin)? {
            Ok(()) => geometry_faults(g, &nin),
            Err(axis) => {
                g.notices.add(
                    &FS_736,
                    format!(
                        "mriNrrdRead: axis {axis}'s space direction of length 0 refused (FreeSurfer: a voxel size of 0)"
                    ),
                );
                let _ = writeln!(
                    err,
                    "mriNrrdRead: axis {axis} of \"{fname}\" has a space direction of length 0"
                );
                return Ok(None);
            }
        }
    }
    Ok(Some(m))
}

/// A vector's length, scaled by its largest component so that no square overflows.
fn length(v: [f64; 3]) -> f64 {
    let big = v.iter().fold(0.0f64, |a, x| a.max(x.abs()));
    if big == 0.0 {
        return 0.0;
    }
    let u = [v[0] / big, v[1] / big, v[2] / big];
    big * (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt()
}

fn replace_all(text: &[u8], pat: &[u8], with: &[u8]) -> Vec<u8> {
    if pat.is_empty() {
        return text.to_vec();
    }
    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if text[i..].starts_with(pat) {
            out.extend_from_slice(with);
            i += pat.len();
        } else {
            out.push(text[i]);
            i += 1;
        }
    }
    out
}
