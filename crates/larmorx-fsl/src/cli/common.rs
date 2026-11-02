// SPDX-License-Identifier: Apache-2.0
//! What FLIRT's other programs share on their command lines (`specs/flirt.md` §15): matrix files
//! as FSL reads them, its image layer's messages, the C++ runtime's abort lines, images read
//! for their header only, the volumes whose centre of gravity they use.

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_io::nifti::NiftiHeader;
use larmorx_mri::hmc::files::{self, Compression};
use larmorx_mri::hmc::image::{ImageError, is_neurological};
use larmorx_mri::hmc::volume::Volume;

use super::Settings;
use crate::flirt::mat::{self, Mat4};

/// The 4D warning of FSL's image layer when one volume is read from a series.
pub const TRUNCATED: &str = "Warning: An input intended to be a single 3D volume has multiple timepoints. Input will be truncated to first volume, but this functionality is deprecated and will be removed in a future release.";

/// A matrix file as FSL's matrix reader reads it (§2): its rows (empty, after FSL's message, when
/// the file cannot be opened); `Err` with the message FSL throws when a row holds fewer numbers
/// than the first.
pub fn read_rows(
    settings: &Settings,
    name: &str,
    err: &mut dyn Write,
) -> Result<Vec<Vec<f64>>, String> {
    match std::fs::read_to_string(settings.path(name)) {
        Ok(t) => mat::parse_matrix_text(&t),
        Err(_) => {
            let _ = writeln!(err, "Could not open matrix file {name}");
            Ok(Vec::new())
        }
    }
}

/// The size of a matrix's rows (`rows x columns`, the first row's count).
pub fn size(rows: &[Vec<f64>]) -> (usize, usize) {
    (rows.len(), rows.first().map_or(0, Vec::len))
}

/// The 4 × 4 matrix of `rows`, `None` unless it is exactly 4 × 4.
pub fn exact4(rows: &[Vec<f64>]) -> Option<Mat4> {
    (rows.len() == 4 && rows.iter().all(|r| r.len() == 4))
        .then(|| mat::mat4_of(rows))
        .flatten()
}

/// The C++ runtime's lines for an uncaught exception (FSL aborts with signal 6), and larmorx's
/// line: the original exits with status 1 after the same text (`docs/porting-guide.md`, failure
/// cases).
pub fn abort(err: &mut dyn Write, program: &str, kind: &str, what: &str) -> u8 {
    let _ = writeln!(
        err,
        "terminate called after throwing an instance of '{kind}'\n  what():  {what}"
    );
    let _ = writeln!(
        err,
        "larmorx fsl {program}: FSL aborts here (signal 6); exit status 1 (larmorx)"
    );
    1
}

/// A refusal of the original (a case whose FSL result is undefined or a size FSL cannot compute
/// with): larmorx's message, exit status 1.
pub fn refuse(err: &mut dyn Write, program: &str, why: &str) -> u8 {
    let _ = writeln!(err, "larmorx fsl {program}: {why} (larmorx)");
    1
}

/// The image layer's exception for `name`: the lines it prints and the exception's message
/// (§15: `#63` and `#22`, then `Error : ...`).
pub fn image_exception(name: &str, e: &ImageError) -> (String, String) {
    let text = e.to_string();
    let missing = text.starts_with("No image files match") || text.contains("No such file");
    if missing || text.starts_with("Multiple possible") {
        let stem = files::strip_image_extension(name);
        let what = if missing {
            format!("No image files match: {stem}")
        } else {
            format!("Multiple possible filenames detected for basename: {stem}")
        };
        (
            format!(
                "Image Exception : #63 :: {what}\nImage Exception : #22 :: Failed to read volume {name}\nError : {what}\n"
            ),
            format!("Failed to read volume {name}\nError : {what}"),
        )
    } else {
        let what = format!("Failed to read volume {name}\nError : {text}");
        (format!("Image Exception : #22 :: {what}\n"), what)
    }
}

/// An image as the coordinate programs and `makerot --cov` read it, its header only (§15.2):
/// sizes below 1 count 1, voxel sizes as stored (their magnitudes), its sform (else qform) and
/// whether it is stored neurologically.
#[derive(Clone, Debug, PartialEq)]
pub struct HeaderImage {
    pub shape: [usize; 3],
    pub voxel: [f32; 3],
    pub form: Option<Mat4>,
    pub neurological: bool,
}

/// Reads the header of the image `name` (FSL's names, compressions by content, ANALYZE headers
/// without transforms).
pub fn read_header(
    settings: &Settings,
    name: &str,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) -> Result<HeaderImage, ImageError> {
    let f = super::bet2::resolve_image(settings, name, mode, notices)?;
    let path = f.header.as_path();
    let invalid = |m: String| ImageError::Invalid(format!("{}: {m}", path.display()));
    let c = Compression::of_file(path).map_err(|e| invalid(e.to_string()))?;
    let raw = header_bytes(path, c).map_err(|e| invalid(e.to_string()))?;
    if raw.len() < 4 {
        return Err(invalid("the file ends inside the header".into()));
    }
    let le = i32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
    let be = i32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);
    let size = if le == 540 || be == 540 { 540 } else { 348 };
    if raw.len() < size {
        return Err(invalid("the file ends inside the header".into()));
    }
    let mut block = raw[..size].to_vec();
    if size == 348 && &block[344..347] != b"ni1" && &block[344..347] != b"n+1" {
        // ANALYZE: no transforms (the fields its history overlaps cleared)
        block[344..348].copy_from_slice(b"ni1\0");
        for b in &mut block[252..328] {
            *b = 0;
        }
    }
    let h = NiftiHeader::parse(&block).map_err(|e| invalid(e.to_string()))?;
    Ok(header_image(&h))
}

/// The first 540 bytes of an image file, decompressed (at most a NIfTI-2 header): only the
/// header is read, as FSL reads it for the coordinate programs (2026-11-02: reading a whole
/// 0.8 mm T1w for its header needed 89 MiB where FSL needs 10).
fn header_bytes(path: &std::path::Path, c: Compression) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    const N: u64 = 540;
    let file = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut out = Vec::with_capacity(N as usize);
    match c {
        Compression::None => {
            file.take(N).read_to_end(&mut out)?;
        }
        Compression::Gzip => {
            flate2::read::MultiGzDecoder::new(file)
                .take(N)
                .read_to_end(&mut out)?;
        }
        Compression::Bzip2 => {
            bzip2::read::MultiBzDecoder::new(file)
                .take(N)
                .read_to_end(&mut out)?;
        }
        Compression::Zstd => {
            out = c.decompress(std::fs::read(path)?)?;
            out.truncate(N as usize);
        }
    }
    Ok(out)
}

/// [`HeaderImage`] of a parsed header.
pub fn header_image(h: &NiftiHeader) -> HeaderImage {
    let shape = [1, 2, 3].map(|k| usize::try_from(h.dim[k]).unwrap_or(0).max(1));
    let voxel = [1, 2, 3].map(|k| (h.pixdim[k] as f32).abs());
    // the qform as FSL holds it: the NIfTI-1 reference library's conversion, its elements in
    // single precision (specs/flirt.md §4.1; observed, 2026-11-02: the coordinates' 6th digit)
    let form = if h.sform_code > 0 {
        Some(*h.sform().rows())
    } else if h.qform_code > 0 && h.qform().is_ok() {
        let r = crate::bet::quatern::quatern_to_mat(
            h.quatern_b,
            h.quatern_c,
            h.quatern_d,
            [h.pixdim[0], h.pixdim[1], h.pixdim[2], h.pixdim[3]],
        );
        let off = [h.qoffset_x, h.qoffset_y, h.qoffset_z].map(|v| f64::from(v as f32));
        Some([
            [r[0][0], r[0][1], r[0][2], off[0]],
            [r[1][0], r[1][1], r[1][2], off[1]],
            [r[2][0], r[2][1], r[2][2], off[2]],
            [0.0, 0.0, 0.0, 1.0],
        ])
    } else {
        None
    };
    HeaderImage {
        shape,
        voxel,
        form,
        neurological: form.is_some() && is_neurological(h),
    }
}

/// The first volume of the image `name` read in full (FSL's 4D warning on `err`), and its
/// header.
pub fn read_volume(
    settings: &Settings,
    name: &str,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
    err: &mut dyn Write,
) -> Result<Volume, ImageError> {
    let input = super::bet2::read_image(settings, name, mode, notices, settings.threads())?;
    if input.volumes > 1 {
        let _ = writeln!(err, "{TRUNCATED}");
    }
    let v = input.vol;
    Ok(Volume::new(v.shape, v.voxel.map(|d| d as f32), v.data))
}

/// A volume's centre of gravity in FSL millimetres (`specs/mcflirt.md` §6.1), with FSL's warning
/// when its total is 0. With NaN or infinite voxels, FSL-14: they are left out by default (FSL's
/// centre is NaN), named in both modes.
pub fn centre(
    v: &Volume,
    program: &'static str,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
    err: &mut dyn Write,
) -> [f64; 3] {
    let finite = mode.fix() && v.has_non_finite();
    let m = if finite {
        v.data
            .iter()
            .copied()
            .filter(|x| x.is_finite())
            .fold(f32::INFINITY, f32::min)
    } else {
        v.min()
    };
    let total: f64 = v
        .data
        .iter()
        .filter(|x| !finite || x.is_finite())
        .map(|&x| f64::from(x - m))
        .sum();
    if total < 1e-5 {
        let _ = writeln!(err, "WARNING::in calculating COG, total = 0.0");
    }
    if v.has_non_finite() {
        let detail = match mode {
            UpstreamDefects::Fix => format!(
                "{program}'s centre of gravity leaves out the volume's NaN and infinite voxels (FSL's is NaN)"
            ),
            UpstreamDefects::Reproduce => format!(
                "{program}'s centre of gravity takes the volume's NaN and infinite voxels in, as FSL does"
            ),
        };
        defects::push_once(notices, Notice::new(&defects::FSL_14, mode, detail));
    }
    crate::flirt::cost::centre_of_mass(v, finite)
}

/// Prints the notices (one line each).
pub fn print_notices(err: &mut dyn Write, notices: &[Notice]) {
    for n in notices {
        let _ = writeln!(err, "larmorx: {}", n.line());
    }
}
