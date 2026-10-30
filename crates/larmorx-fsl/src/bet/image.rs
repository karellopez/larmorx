// SPDX-License-Identifier: Apache-2.0
//! Images as BET reads and writes them (`specs/bet.md` §3).
//!
//! Reading (the names, the compressions by content, the conversion to float32, the scaling and
//! the x reversal of images stored with a positive determinant) is shared with the clean-room
//! head-motion correction (`larmorx_mri::hmc::image`), which follows the same FSL 6.0.7.23
//! behaviour (`specs/mcflirt.md` §4.1). Writing follows §3.3: the input's grid and geometry, a
//! fresh header, the output's own data type.

use std::path::Path;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_core::element::DataType;
use larmorx_core::ndarray::{ArrayD, IxDyn, ShapeBuilder};
use larmorx_core::{DynArray, parallel};
use larmorx_io::nifti::{self, NiftiHeader, NiftiVersion, WriteOptions};
use larmorx_mri::hmc::files::{Compression, ImageFiles};
pub use larmorx_mri::hmc::image::{ImageError, OUTPUT_TYPES, unknown_output_type_message};
use larmorx_mri::hmc::image::{output_name, read_series_files};

use super::Vol;

/// The `descrip` field of the images larmorx's BET writes.
pub const DESCRIP: &str = "larmorx fsl bet2";

/// An image as read: its first volume in the radiological frame and what writing results like
/// it needs.
#[derive(Clone, Debug)]
pub struct Input {
    pub vol: Vol,
    pub header: NiftiHeader,
    /// Whether the x axis is reversed in memory.
    pub flipped: bool,
    /// Whether the file had more than one volume (only the first is kept).
    pub volumes: usize,
}

/// Reads the image `files`: every volume, and the first as [`Input`].
pub fn read(files: &ImageFiles, n_threads: usize) -> Result<(Vec<Vol>, Input), ImageError> {
    if let Some(r) = read_analyze(files)? {
        return Ok(r);
    }
    let s = read_series_files(files, n_threads)?;
    let volumes = s.volumes.len();
    let vols: Vec<Vol> = s
        .volumes
        .into_iter()
        .map(|v| Vol {
            shape: v.shape,
            voxel: v.voxel_size.map(f64::from),
            data: v.data,
        })
        .collect();
    let first = vols.first().cloned().ok_or_else(|| {
        ImageError::Invalid(format!("{}: the image has no voxels", files.header.display()))
    })?;
    Ok((
        vols,
        Input {
            vol: first,
            header: s.header,
            flipped: s.flipped,
            volumes,
        },
    ))
}

/// The values of one output image.
pub enum Values {
    F32(Vec<f32>),
    I16(Vec<i16>),
}

/// How an output is written.
#[derive(Clone, Copy, Debug)]
pub struct Output<'a> {
    pub template: &'a NiftiHeader,
    pub flipped: bool,
    pub shape: [usize; 3],
    /// The number of volumes (`bet -F`'s brain is 4D).
    pub volumes: usize,
    /// `cal_min`, `cal_max` set (the mask's 0 and 1), else the input's kept.
    pub cal: Option<(f64, f64)>,
    /// Keep the input's extensions (`FSL_LOAD_NIFTI_EXTENSIONS`).
    pub extensions: bool,
    pub defects: UpstreamDefects,
    pub n_threads: usize,
}

/// The file name an output stem gets for `FSLOUTPUTTYPE` (`None`: an unknown type).
pub fn output_file(stem: &str, output_type: &str) -> Option<(String, NiftiVersion)> {
    output_name(stem, output_type)
}

/// The output header (§3.3).
pub fn output_header(o: &Output<'_>, version: NiftiVersion, data_type: DataType) -> NiftiHeader {
    let mut h = o.template.with_version(version);
    h.byte_order = larmorx_io::nifti::ByteOrder::Little;
    h.set_data_type(data_type);
    h.scl_slope = 1.0;
    h.scl_inter = 0.0;
    if let Some((lo, hi)) = o.cal {
        h.cal_min = lo;
        h.cal_max = hi;
    }
    h.dim_info = 0;
    h.slice_code = 0;
    h.slice_start = 0;
    h.slice_end = 0;
    h.slice_duration = 0.0;
    h.toffset = 0.0;
    h.xyzt_units = 10;
    h.descrip = [0; 80];
    let d = DESCRIP.as_bytes();
    h.descrip[..d.len()].copy_from_slice(d);
    if version == NiftiVersion::V1 {
        h.analyze.extents = 16384;
        h.analyze.regular = b'r';
    }
    if !o.extensions {
        h.extensions.clear();
    }
    let [nx, ny, nz] = o.shape;
    h.dim = [1; 8];
    if o.volumes > 1 {
        h.dim[..5].copy_from_slice(&[4, nx as i64, ny as i64, nz as i64, o.volumes as i64]);
    } else {
        // trailing axes of one voxel are not counted (observed: a single slice is written 2D)
        let ndim = if nz > 1 {
            3
        } else if ny > 1 {
            2
        } else {
            1
        };
        h.dim[..4].copy_from_slice(&[ndim, nx as i64, ny as i64, nz as i64]);
    }
    h
}

/// Writes `values` (radiological frame, x fastest, volumes last) to `path` (a name with its
/// image extension).
pub fn write(
    path: &str,
    version: NiftiVersion,
    o: &Output<'_>,
    values: Values,
) -> Result<Vec<Notice>, ImageError> {
    let files = ImageFiles::of(path)
        .ok_or_else(|| ImageError::Invalid(format!("{path}: not an image file name")))?;
    let [nx, ny, nz] = o.shape;
    let per = nx * ny * nz;
    let mut shape = vec![nx, ny, nz];
    if o.volumes > 1 {
        shape.push(o.volumes);
    } else {
        while shape.len() > 1 && shape[shape.len() - 1] == 1 {
            shape.pop();
        }
    }
    let unflip = |n: usize| {
        let mut out = Vec::with_capacity(n);
        for v in 0..o.volumes {
            for row in 0..ny * nz {
                let base = v * per + row * nx;
                for i in 0..nx {
                    out.push(if o.flipped { base + nx - 1 - i } else { base + i });
                }
            }
        }
        out
    };
    let order = unflip(per * o.volumes);
    let dims = IxDyn(&shape).f();
    let invalid = |e: larmorx_core::ndarray::ShapeError| ImageError::Invalid(e.to_string());
    let (array, data_type): (DynArray, DataType) = match values {
        Values::F32(v) => (
            ArrayD::from_shape_vec(dims, order.iter().map(|&k| v[k]).collect())
                .map_err(invalid)?
                .into(),
            DataType::F32,
        ),
        Values::I16(v) => (
            ArrayD::from_shape_vec(dims, order.iter().map(|&k| v[k]).collect())
                .map_err(invalid)?
                .into(),
            DataType::I16,
        ),
    };
    let h = output_header(o, version, data_type);
    let mut notices = Vec::new();
    let several = h.extensions.len() >= 2
        && h.extensions
            .iter()
            .any(|e| e.size_on_disk() != h.extensions[0].size_on_disk());
    let compression = Compression::of_name(&files.header.to_string_lossy());
    let fsl56 = several && files.data.is_none();
    if fsl56 {
        let n = h.extensions.len();
        let detail = match o.defects {
            UpstreamDefects::Fix => format!(
                "{path}: the data offset counts each of the {n} extensions' own sizes; FSL counts the first one's {n} times"
            ),
            UpstreamDefects::Reproduce => format!(
                "{path}: the data offset counts the first extension's size once per extension ({n}), as FSL does"
            ),
        };
        notices.push(Notice::new(&defects::FSL_56, o.defects, detail));
    }
    if files.standard_names()
        && matches!(compression, Compression::None | Compression::Gzip)
        && !(fsl56 && o.defects.reproduce())
    {
        let options = WriteOptions {
            compression_level: 2,
            n_threads: o.n_threads,
        };
        nifti::write_dyn(&files.header, &h, &array, &options)?;
        return Ok(notices);
    }
    write_pieces(&files, h, &array, fsl56 && o.defects.reproduce(), o.n_threads)?;
    Ok(notices)
}

/// Writes the header block and the data, each file compressed as its name says.
fn write_pieces(
    files: &ImageFiles,
    mut h: NiftiHeader,
    array: &DynArray,
    fsl56: bool,
    n_threads: usize,
) -> Result<(), ImageError> {
    let single = files.data.is_none();
    h.magic = h.version.magic(single);
    if h.version == NiftiVersion::V2 {
        h.nifti2.eol_check = *b"\r\n\x1a\n";
    }
    let base = h.version.min_vox_offset();
    let ext_bytes: usize = h
        .extensions
        .iter()
        .map(larmorx_io::nifti::Extension::size_on_disk)
        .sum();
    let offset = if !single {
        0
    } else if fsl56 {
        base + h.extensions.len() * h.extensions[0].size_on_disk()
    } else {
        base + ext_bytes
    };
    h.vox_offset = offset as f64;
    let mut head = h.to_bytes();
    if single {
        let mut ext = Vec::new();
        ext.extend_from_slice(&[u8::from(!h.extensions.is_empty()), 0, 0, 0]);
        for e in &h.extensions {
            e.write(&mut ext, h.byte_order)
                .map_err(|e| ImageError::Invalid(e.to_string()))?;
        }
        head.extend_from_slice(&ext);
        // FSL's offset: the data start inside the extensions or after a gap
        head.resize(offset, 0);
    }
    let write = |p: &Path, pieces: &[&[u8]]| -> Result<(), ImageError> {
        let io = |e: std::io::Error| ImageError::Invalid(format!("{}: {e}", p.display()));
        let file = std::fs::File::create(p).map_err(io)?;
        let w = std::io::BufWriter::with_capacity(1 << 16, file);
        Compression::of_name(&p.to_string_lossy())
            .write_pieces(pieces, w)
            .map_err(io)
    };
    let bytes: &[u8] = match array {
        DynArray::F32(a) => bytemuck::cast_slice(a.as_slice_memory_order().unwrap_or(&[])),
        DynArray::I16(a) => bytemuck::cast_slice(a.as_slice_memory_order().unwrap_or(&[])),
        _ => return Err(ImageError::Invalid("unexpected output type".into())),
    };
    parallel::with_threads(n_threads, || match &files.data {
        None => write(&files.header, &[&head, bytes]),
        Some(d) => {
            write(&files.header, &[&head])?;
            write(d, &[bytes])
        }
    })
    .map_err(|e| ImageError::Invalid(e.to_string()))?
}

/// An ANALYZE 7.5 pair (a 348-byte header without NIfTI's magic), as FSL reads it (§3.1;
/// observed): radiological storage, no scaling unless its slope field asks for it, and on
/// output a qform and sform (code 2) of the voxel sizes with the origin from the header's
/// `originator` (`-(origin - 1) d`). `None` when `files` is not ANALYZE.
fn read_analyze(files: &ImageFiles) -> Result<Option<(Vec<Vol>, Input)>, ImageError> {
    let Some(data_path) = files.data.as_deref() else {
        return Ok(None);
    };
    let invalid = |p: &Path, m: String| ImageError::Invalid(format!("{}: {m}", p.display()));
    let path = files.header.as_path();
    let load = |p: &Path| -> Result<Vec<u8>, ImageError> {
        let c = Compression::of_file(p).map_err(|e| invalid(p, e.to_string()))?;
        std::fs::read(p)
            .and_then(|raw| c.decompress(raw))
            .map_err(|e| invalid(p, e.to_string()))
    };
    let Ok(raw) = load(path) else {
        return Ok(None);
    };
    if raw.len() < 348 {
        return Ok(None);
    }
    let le = i32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) == 348;
    let be = i32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) == 348;
    if !(le || be) || &raw[344..347] == b"ni1" || &raw[344..347] == b"n+1" {
        return Ok(None);
    }
    let i16_at = |k: usize| {
        let b = [raw[k], raw[k + 1]];
        if le {
            i16::from_le_bytes(b)
        } else {
            i16::from_be_bytes(b)
        }
    };
    let origin = [i16_at(253), i16_at(255), i16_at(257)];
    let mut block = raw[..348].to_vec();
    block[344..348].copy_from_slice(b"ni1\0");
    // the NIfTI fields ANALYZE's history overlaps: no transforms
    for b in &mut block[252..328] {
        *b = 0;
    }
    let mut h = NiftiHeader::parse(&block).map_err(|e| invalid(path, e.to_string()))?;
    h.apply_load_fixes()
        .map_err(|e| invalid(path, e.to_string()))?;
    let shape = h.shape().map_err(|e| invalid(path, e.to_string()))?;
    let dt = h.data_type().map_err(|e| invalid(path, e.to_string()))?;
    let bytes = load(data_path)?;
    let offset = if h.vox_offset.is_finite() && h.vox_offset > 0.0 {
        h.vox_offset as usize
    } else {
        0
    };
    let n: usize = shape.iter().product();
    let size = dt.size();
    if bytes.len() < offset + n * size {
        return Err(invalid(
            data_path,
            format!("expected {} bytes of voxel data, found {}", n * size, bytes.len()),
        ));
    }
    let raw = &bytes[offset..offset + n * size];
    let swap = !h.byte_order.is_native();
    let value = |k: usize| -> f32 {
        let b = &raw[k * size..(k + 1) * size];
        let mut a = [0u8; 8];
        a[..size].copy_from_slice(b);
        if swap {
            a[..size].reverse();
        }
        match dt {
            DataType::U8 => f32::from(a[0]),
            DataType::I8 => f32::from(a[0] as i8),
            DataType::U16 => f32::from(u16::from_ne_bytes([a[0], a[1]])),
            DataType::I16 => f32::from(i16::from_ne_bytes([a[0], a[1]])),
            DataType::U32 => u32::from_ne_bytes([a[0], a[1], a[2], a[3]]) as f32,
            DataType::I32 => i32::from_ne_bytes([a[0], a[1], a[2], a[3]]) as f32,
            DataType::F32 => f32::from_ne_bytes([a[0], a[1], a[2], a[3]]),
            DataType::F64 => f64::from_ne_bytes(a) as f32,
            DataType::U64 => u64::from_ne_bytes(a) as f32,
            DataType::I64 => i64::from_ne_bytes(a) as f32,
            _ => 0.0,
        }
    };
    let slope = h.scl_slope;
    let scale = (slope != 0.0 && slope.is_finite() && slope != 1.0).then_some(slope as f32);
    let values: Vec<f32> = (0..n)
        .map(|k| match scale {
            Some(s) => value(k) * s,
            None => value(k),
        })
        .collect();
    let dims: [usize; 3] = [0, 1, 2].map(|k| shape.get(k).copied().unwrap_or(1));
    let per = dims.iter().product::<usize>().max(1);
    let voxel = larmorx_mri::hmc::image::voxel_size(&h).map(f64::from);
    // FSL gives an ANALYZE input a transform of positive determinant (below), so it works on
    // it with x reversed, as on any neurologically stored image (observed)
    let vols: Vec<Vol> = values
        .chunks_exact(per)
        .map(|c| flip_x(&Vol::new(dims, voxel, c.to_vec())))
        .collect();
    // the output geometry FSL gives an ANALYZE input (observed)
    let d = voxel;
    let off = [0, 1, 2].map(|k| -(f64::from(origin[k]) - 1.0) * d[k]);
    h.qform_code = 2;
    h.sform_code = 2;
    h.quatern_b = 0.0;
    h.quatern_c = 0.0;
    h.quatern_d = 0.0;
    h.qoffset_x = off[0];
    h.qoffset_y = off[1];
    h.qoffset_z = off[2];
    h.pixdim[0] = 1.0;
    h.srow_x = [d[0], 0.0, 0.0, off[0]];
    h.srow_y = [0.0, d[1], 0.0, off[1]];
    h.srow_z = [0.0, 0.0, d[2], off[2]];
    h.scl_slope = 1.0;
    h.scl_inter = 0.0;
    h.intent_code = 0;
    let volumes = vols.len();
    let first = vols
        .first()
        .cloned()
        .ok_or_else(|| invalid(path, "the image has no voxels".into()))?;
    Ok(Some((
        vols,
        Input {
            vol: first,
            header: h,
            flipped: true,
            volumes,
        },
    )))
}

/// `v` with its x axis reversed.
pub fn flip_x(v: &Vol) -> Vol {
    let [nx, _, _] = v.shape;
    let mut data = Vec::with_capacity(v.data.len());
    for row in v.data.chunks_exact(nx.max(1)) {
        data.extend(row.iter().rev());
    }
    Vol::new(v.shape, v.voxel, data)
}
