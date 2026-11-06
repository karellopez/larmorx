// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-newimage 2601.0 (generalio.cc: `read_complexvolume`;
// newimagefns.cc: `length`, `abs` and `phase` of a real and an imaginary volume) and
// fsl-newnifti 5.0.0 (NewNifti.cc: `bpvOfDatatype`, the byte swap of `loadImageROI`). Copyright
// (C) 1999-2018 University of Oxford (Mark Jenkinson, Matthew Webster et al.). Rust translation
// Copyright 2026 Karel Lopez Vilaret; modified from the originals. Distributed under the FSL
// Licence (see LICENSE).
//! newimage's complex images as `prelude -c` reads them: `read_complexvolume(realvols,
//! imagvols, filename)` and the phase and magnitude of a real and an imaginary volume.
//!
//! What FSL does (and [`read_complexvolume`] with `corrected` false):
//!
//! - the header is read (a failure is newimage's exception #22); the sizes are `dim[1..=4]`
//!   (`dim[4]` at least 1): higher dimensions are left out;
//! - the data are NewNifti's raw buffer (no `scl_slope`, no conversion): a `DT_COMPLEX` image
//!   gives the real parts from its even floats and the imaginary parts from its odd ones; any
//!   other datatype is taken as floats as they are (only `DT_FLOAT` reads right) and the
//!   imaginary buffer is allocated and never written (memory FSL never wrote: zeros here);
//!   NewNifti gives some datatypes (complex128, RGB, ...) no width, so the buffer is empty and
//!   every value is read from memory FSL does not own (zeros here);
//! - in a file of the other byte order NewNifti swaps each complex64 value's 8 bytes whole, so
//!   the real and imaginary parts change places;
//! - the voxel sizes are the header's magnitudes (no zero made 1, no ANALYZE origin), the
//!   sform, qform, intent, display range and auxiliary file are the header's; a neurological
//!   image is swapped to radiological order; extensions are never kept.
//!
//! With `corrected` (larmorx's correction, FSL-264 to FSL-266): complex64 and complex128 images
//! are read as their file means them (each float or double in its own byte order; complex128
//! converted to float with C's conversion); any other datatype is no complex image
//! ([`Complex::not_complex`]).

use std::path::Path;

use crate::nifti::{self, dt};
use crate::settings::Settings;
use crate::sse::S32;
use crate::volume::{
    FslError, RADIOLOGICAL, Volume, header_left_right_order, mat44_to_mat,
    return_validimagefilename,
};

/// A complex image read as `prelude -c` reads it.
#[derive(Clone, Debug)]
pub struct Complex {
    pub re: Volume<f32>,
    pub im: Volume<f32>,
    /// The file's datatype.
    pub datatype: i16,
    /// Whether the file was in the other byte order.
    pub swapped: bool,
    /// FSL only: the imaginary values are memory FSL never wrote (a datatype other than
    /// complex64), filled with zeros here.
    pub imag_unwritten: bool,
    /// FSL only: how many real (and imaginary) values were read beyond NewNifti's buffer (memory
    /// FSL does not own), filled with zeros here.
    pub beyond_buffer: usize,
    /// Corrected only: the datatype is neither complex64 nor complex128 (no value read).
    pub not_complex: bool,
}

/// `read_complexvolume(realvols, imagvols, filename)`: `Ok(None)` for an empty name (FSL
/// returns -1 and leaves both volumes as they were).
pub fn read_complexvolume(
    filename: &str,
    settings: &Settings,
    corrected: bool,
) -> Result<Option<Complex>, FslError> {
    if filename.is_empty() {
        return Ok(None);
    }
    let fail = |what: &str| {
        FslError::imthrow(
            format!("Failed to read volume {filename}\nError : {what}"),
            22,
        )
    };
    // the header, in FSL's try block: any failure becomes newimage's #22
    let valid = match return_validimagefilename(filename) {
        Ok(v) => v,
        Err(e) => {
            let mut out = fail(&e.what);
            out.printed.splice(0..0, e.printed);
            return Err(out);
        }
    };
    let first =
        nifti::load_header(Path::new(&valid), &settings.open_options()).map_err(|e| fail(&e.0))?;
    let is_complex = first.datatype == dt::COMPLEX;
    let (sx, sy, sz) = (first.dim[1], first.dim[2], first.dim[3]);
    let st = first.dim[4].max(1);
    let volsize = [sx, sy, sz, st]
        .iter()
        .try_fold(1usize, |acc, &d| acc.checked_mul(d.max(0) as usize))
        .ok_or_else(|| FslError::imthrow("Out of memory", 99))?;
    // loadImageROI outside the try block: its exception is uncaught
    let image =
        nifti::open_image(Path::new(&valid), &settings.open_options()).map_err(FslError::nifti)?;
    let datatype = image.header.datatype;
    let swapped = image.header.was_wrong_endian;
    let mut out = Complex {
        re: Volume::new(sx, sy, sz, st),
        im: Volume::new(sx, sy, sz, st),
        datatype,
        swapped,
        imag_unwritten: false,
        beyond_buffer: 0,
        not_complex: false,
    };
    let header;
    if corrected {
        let (width, unit) = match datatype {
            dt::COMPLEX => (8, 4),
            dt::COMPLEX128 => (16, 8),
            _ => {
                out.not_complex = true;
                (0, 1)
            }
        };
        let mut raw: Vec<u8> = Vec::new();
        let loaded = image
            .reading_values(width, unit)
            .read(&mut |piece| raw.extend_from_slice(piece))
            .map_err(FslError::nifti)?;
        header = loaded.header;
        if !out.not_complex {
            for v in 0..volsize {
                let (r, i) = if width == 8 {
                    let at =
                        |k: usize| f32::from_le_bytes(raw[4 * k..4 * k + 4].try_into().unwrap());
                    (at(2 * v), at(2 * v + 1))
                } else {
                    let at =
                        |k: usize| f64::from_le_bytes(raw[8 * k..8 * k + 8].try_into().unwrap());
                    (at(2 * v) as f32, at(2 * v + 1) as f32)
                };
                out.re.data[v] = r;
                out.im.data[v] = i;
            }
        }
    } else {
        let mut raw: Vec<u8> = Vec::new();
        let loaded = image
            .read(&mut |piece| raw.extend_from_slice(piece))
            .map_err(FslError::nifti)?;
        header = loaded.header;
        let floats = raw.len() / 4;
        let at = |k: usize| f32::from_le_bytes(raw[4 * k..4 * k + 4].try_into().unwrap());
        if is_complex {
            for v in 0..volsize {
                out.re.data[v] = at(2 * v);
                out.im.data[v] = at(2 * v + 1);
            }
        } else {
            out.imag_unwritten = true;
            for v in 0..volsize {
                if v < floats {
                    out.re.data[v] = at(v);
                } else {
                    out.beyond_buffer += 1;
                }
            }
        }
    }
    for vol in [&mut out.re, &mut out.im] {
        set_properties(vol, &header);
    }
    Ok(Some(out))
}

/// `setdims` and `set_volume_properties(niihdr, vol)`, then `makeradiological()` for a
/// neurological file (FSL's complex reader leaves zero voxel sizes and ANALYZE origins as they
/// are, unlike `read_volume`).
fn set_properties(v: &mut Volume<f32>, header: &nifti::NiftiHeader) {
    for k in 0..4 {
        v.pixdims[k] = (header.pixdim[k + 1] as f32).abs();
    }
    for k in 4..7 {
        v.pixdims[k] = header.pixdim[k + 1] as f32;
    }
    v.sform = mat44_to_mat(&header.get_sform());
    v.qform = mat44_to_mat(&header.get_qform());
    v.sform_code = header.sform_code;
    v.qform_code = header.qform_code;
    v.radiological_file = header_left_right_order(header) == RADIOLOGICAL;
    v.intent_code = header.intent_code;
    v.intent_params = [
        header.intent_p1 as f32,
        header.intent_p2 as f32,
        header.intent_p3 as f32,
    ];
    v.display_min = header.cal_min as f32;
    v.display_max = header.cal_max as f32;
    let mut aux = header.aux_file.clone();
    aux.truncate(24);
    v.aux_file = aux;
    if !v.radiological_file {
        v.make_radiological();
    }
}

/// `length(x, y)`: `sqrt(x*x + y*y)` in `float`.
#[inline]
pub fn length(x: f32, y: f32) -> f32 {
    ((S32(x) * x) + (S32(y) * y)).sqrt().0
}

/// `abs(realvol, imagvol)`: `length(imag, real)` voxel by voxel, with the real volume's
/// properties (every volume of a series: `prelude` takes them one at a time, with the same
/// values).
pub fn abs(re: &Volume<f32>, im: &Volume<f32>) -> Volume<f32> {
    let mut out = re.clone();
    for (o, (&r, &i)) in out.data.iter_mut().zip(re.data.iter().zip(&im.data)) {
        *o = length(i, r);
    }
    out
}

/// `phase(realvol, imagvol)`: `atan2f(imag, real)` voxel by voxel (glibc's), with the real
/// volume's properties.
pub fn phase(re: &Volume<f32>, im: &Volume<f32>) -> Volume<f32> {
    let mut out = re.clone();
    for (o, (&r, &i)) in out.data.iter_mut().zip(re.data.iter().zip(&im.data)) {
        *o = crate::fdlibm::atan2f(i, r);
    }
    out
}
