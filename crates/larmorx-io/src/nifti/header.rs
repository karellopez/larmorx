//! The NIfTI-1 and NIfTI-2 header.
//!
//! [`NiftiHeader`] holds every on-disk field of either version (NIfTI-1 values are widened
//! losslessly), so a header can be read and written back byte for byte. The interpretation of
//! the fields (shape, affine, scaling, load-time fixes) follows nibabel 5.x; see
//! `PROVENANCE.md`.

use larmorx_core::affine::Affine;
use larmorx_core::element::DataType;
use larmorx_core::linalg::{self, Mat3};
use larmorx_core::rotation;

use super::bytes::{FieldReader, FieldWriter};
use super::datatype;
use super::extension::Extension;

/// Size of a NIfTI-1 header block.
pub const NIFTI1_HEADER_SIZE: usize = 348;
/// Size of a NIfTI-2 header block.
pub const NIFTI2_HEADER_SIZE: usize = 540;
/// The 4-byte "extender" that follows the header block.
pub const EXTENDER_SIZE: usize = 4;

/// The NIfTI format version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NiftiVersion {
    V1,
    V2,
}

impl NiftiVersion {
    /// Size of the header block (`sizeof_hdr`).
    pub const fn header_size(self) -> usize {
        match self {
            NiftiVersion::V1 => NIFTI1_HEADER_SIZE,
            NiftiVersion::V2 => NIFTI2_HEADER_SIZE,
        }
    }

    /// Offset of the voxel data in a single file without extensions (352 or 544).
    pub const fn min_vox_offset(self) -> usize {
        self.header_size() + EXTENDER_SIZE
    }

    /// The magic string: `n+1`/`n+2` for single files, `ni1`/`ni2` for header/image pairs.
    pub const fn magic(self, single_file: bool) -> [u8; 4] {
        match (self, single_file) {
            (NiftiVersion::V1, true) => *b"n+1\0",
            (NiftiVersion::V1, false) => *b"ni1\0",
            (NiftiVersion::V2, true) => *b"n+2\0",
            (NiftiVersion::V2, false) => *b"ni2\0",
        }
    }
}

/// Byte order of the header and the voxel data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ByteOrder {
    Little,
    Big,
}

impl ByteOrder {
    /// The byte order of this machine.
    pub const NATIVE: ByteOrder = if cfg!(target_endian = "little") {
        ByteOrder::Little
    } else {
        ByteOrder::Big
    };

    pub const fn is_native(self) -> bool {
        matches!(
            (self, Self::NATIVE),
            (ByteOrder::Little, ByteOrder::Little) | (ByteOrder::Big, ByteOrder::Big)
        )
    }
}

/// Meaning of the qform/sform coordinates (`NIFTI_XFORM_*`).
pub mod xform {
    pub const UNKNOWN: i32 = 0;
    pub const SCANNER_ANAT: i32 = 1;
    pub const ALIGNED_ANAT: i32 = 2;
    pub const TALAIRACH: i32 = 3;
    pub const MNI_152: i32 = 4;
    pub const TEMPLATE_OTHER: i32 = 5;

    /// Whether `code` is one of the codes defined by the standard (as nibabel checks it).
    pub const fn is_valid(code: i32) -> bool {
        0 <= code && code <= 5
    }
}

/// Analyze 7.5 fields that NIfTI-1 keeps unused for backward compatibility.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AnalyzeFields {
    pub data_type: [u8; 10],
    pub db_name: [u8; 18],
    pub extents: i32,
    pub session_error: i16,
    pub regular: u8,
    pub glmax: i32,
    /// Also holds the vector length in FreeSurfer's large-vector convention (`dim[1] == -1`).
    pub glmin: i32,
}

/// NIfTI-2 fields with no NIfTI-1 counterpart.
#[derive(Clone, Debug, PartialEq)]
pub struct Nifti2Fields {
    /// Bytes 4..8 after the magic; `\r\n\x1a\n` to detect text-mode transfer corruption.
    pub eol_check: [u8; 4],
    pub unused_str: [u8; 15],
}

impl Default for Nifti2Fields {
    fn default() -> Self {
        Nifti2Fields {
            eol_check: *b"\r\n\x1a\n",
            unused_str: [0; 15],
        }
    }
}

/// A NIfTI-1 or NIfTI-2 header, with every on-disk field.
///
/// Numeric fields are widened to i64/f64 (exact for NIfTI-1's i16/f32 values). Codes are kept
/// as raw integers so unknown values survive a round trip. `==` compares floats with IEEE
/// semantics (a NaN `scl_slope` is never equal to itself); compare [`NiftiHeader::to_bytes`]
/// for "same on disk".
#[derive(Clone, Debug, PartialEq)]
pub struct NiftiHeader {
    pub version: NiftiVersion,
    pub byte_order: ByteOrder,
    /// The magic string as stored (4 bytes; NIfTI-2's continues in `nifti2.eol_check`).
    pub magic: [u8; 4],
    pub dim_info: u8,
    pub dim: [i64; 8],
    pub intent_p1: f64,
    pub intent_p2: f64,
    pub intent_p3: f64,
    pub intent_code: i32,
    /// Raw `NIFTI_TYPE_*` code; see [`NiftiHeader::data_type`].
    pub datatype: i16,
    pub bitpix: i16,
    pub slice_start: i64,
    pub pixdim: [f64; 8],
    pub vox_offset: f64,
    pub scl_slope: f64,
    pub scl_inter: f64,
    pub slice_end: i64,
    pub slice_code: i32,
    pub xyzt_units: i32,
    pub cal_max: f64,
    pub cal_min: f64,
    pub slice_duration: f64,
    pub toffset: f64,
    pub descrip: [u8; 80],
    pub aux_file: [u8; 24],
    pub qform_code: i32,
    pub sform_code: i32,
    pub quatern_b: f64,
    pub quatern_c: f64,
    pub quatern_d: f64,
    pub qoffset_x: f64,
    pub qoffset_y: f64,
    pub qoffset_z: f64,
    pub srow_x: [f64; 4],
    pub srow_y: [f64; 4],
    pub srow_z: [f64; 4],
    pub intent_name: [u8; 16],
    /// NIfTI-1 only (zero for NIfTI-2).
    pub analyze: AnalyzeFields,
    /// NIfTI-2 only.
    pub nifti2: Nifti2Fields,
    pub extensions: Vec<Extension>,
}

/// A header problem that makes a file unreadable, or an operation impossible.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum HeaderError {
    #[error("not a NIfTI-1 or NIfTI-2 header (sizeof_hdr is neither 348 nor 540)")]
    NotNifti,
    #[error("invalid magic string {0:?}")]
    BadMagic(String),
    #[error("data type code {0} is not supported")]
    UnsupportedDataType(i16),
    #[error("dim[0] = {0} is outside 0..=7")]
    BadNdim(i64),
    #[error("dim[{index}] = {value} is negative")]
    NegativeDim { index: usize, value: i64 },
    #[error("vox_offset {offset} is below the minimum {min} for a single-file image")]
    VoxOffsetTooSmall { offset: f64, min: usize },
    #[error("qform quaternion is not unit: 1 - (b² + c² + d²) = {0:e}")]
    NonUnitQuaternion(f64),
    #[error("scl_slope is valid but scl_inter = {0} is not finite")]
    NonFiniteIntercept(f64),
    #[error("FreeSurfer large-vector header has dim[1] = -1 but glmin = 0")]
    BadFreeSurferVector,
    #[error("shape {0:?} does not fit in this header version")]
    ShapeTooLarge(Vec<usize>),
    #[error("the affine cannot be stored in a qform: {0}")]
    BadAffine(&'static str),
}

/// A field that was fixed when the header was read (as nibabel's header checks do).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderFix(pub &'static str);

impl NiftiHeader {
    /// A fresh header for an image of `shape` and `data_type`: unit voxel sizes, no
    /// qform/sform, no scaling (`scl_slope` 1, `scl_inter` 0, as nibabel writes them), no
    /// extensions.
    pub fn new(
        version: NiftiVersion,
        shape: &[usize],
        data_type: DataType,
    ) -> Result<Self, HeaderError> {
        let mut hdr = NiftiHeader {
            version,
            byte_order: ByteOrder::NATIVE,
            magic: version.magic(true),
            dim_info: 0,
            dim: [0, 1, 1, 1, 1, 1, 1, 1],
            intent_p1: 0.0,
            intent_p2: 0.0,
            intent_p3: 0.0,
            intent_code: 0,
            datatype: 0,
            bitpix: 0,
            slice_start: 0,
            pixdim: [1.0; 8],
            vox_offset: 0.0,
            scl_slope: 1.0,
            scl_inter: 0.0,
            slice_end: 0,
            slice_code: 0,
            xyzt_units: 0,
            cal_max: 0.0,
            cal_min: 0.0,
            slice_duration: 0.0,
            toffset: 0.0,
            descrip: [0; 80],
            aux_file: [0; 24],
            qform_code: 0,
            sform_code: 0,
            quatern_b: 0.0,
            quatern_c: 0.0,
            quatern_d: 0.0,
            qoffset_x: 0.0,
            qoffset_y: 0.0,
            qoffset_z: 0.0,
            srow_x: [0.0; 4],
            srow_y: [0.0; 4],
            srow_z: [0.0; 4],
            intent_name: [0; 16],
            analyze: AnalyzeFields::default(),
            nifti2: Nifti2Fields::default(),
            extensions: Vec::new(),
        };
        hdr.set_data_type(data_type);
        hdr.set_shape(shape)?;
        Ok(hdr)
    }

    // ----------------------------------------------------------------------------------------
    // Binary layout

    /// Detects the version and byte order from the first bytes of a file (at least 4), as
    /// nibabel does: `sizeof_hdr` selects the version, `dim[0]` confirms the byte order.
    pub fn sniff(block: &[u8]) -> Result<(NiftiVersion, ByteOrder), HeaderError> {
        let head: [u8; 4] = block
            .get(..4)
            .ok_or(HeaderError::NotNifti)?
            .try_into()
            .unwrap();
        let (le, be) = (i32::from_le_bytes(head), i32::from_be_bytes(head));
        let (version, order) = match (le, be) {
            (348, _) => (NiftiVersion::V1, ByteOrder::Little),
            (_, 348) => (NiftiVersion::V1, ByteOrder::Big),
            (540, _) => (NiftiVersion::V2, ByteOrder::Little),
            (_, 540) => (NiftiVersion::V2, ByteOrder::Big),
            _ => return Err(HeaderError::NotNifti),
        };
        // nibabel trusts dim[0] over sizeof_hdr when it is a plausible 1..=7.
        let dim0 = |o: ByteOrder| -> Option<i64> {
            let r = FieldReader::new(block, o);
            match version {
                NiftiVersion::V1 if block.len() >= 42 => Some(i64::from(r.i16(40))),
                NiftiVersion::V2 if block.len() >= 24 => Some(r.i64(16)),
                _ => None,
            }
        };
        let swapped = match order {
            ByteOrder::Little => ByteOrder::Big,
            ByteOrder::Big => ByteOrder::Little,
        };
        let plausible = |d: Option<i64>| d.is_some_and(|d| (1..=7).contains(&d));
        if !plausible(dim0(order)) && plausible(dim0(swapped)) {
            return Ok((version, swapped));
        }
        Ok((version, order))
    }

    /// Parses a header block (348 bytes for NIfTI-1, 540 for NIfTI-2). Extensions are read
    /// separately. No fixes are applied; see [`NiftiHeader::apply_load_fixes`].
    pub fn parse(block: &[u8]) -> Result<Self, HeaderError> {
        let (version, order) = Self::sniff(block)?;
        if block.len() < version.header_size() {
            return Err(HeaderError::NotNifti);
        }
        let r = FieldReader::new(block, order);
        let hdr = match version {
            NiftiVersion::V1 => NiftiHeader {
                version,
                byte_order: order,
                magic: r.bytes(344),
                dim_info: r.u8(39),
                dim: std::array::from_fn(|i| i64::from(r.i16(40 + 2 * i))),
                intent_p1: f64::from(r.f32(56)),
                intent_p2: f64::from(r.f32(60)),
                intent_p3: f64::from(r.f32(64)),
                intent_code: i32::from(r.i16(68)),
                datatype: r.i16(70),
                bitpix: r.i16(72),
                slice_start: i64::from(r.i16(74)),
                pixdim: std::array::from_fn(|i| f64::from(r.f32(76 + 4 * i))),
                vox_offset: f64::from(r.f32(108)),
                scl_slope: f64::from(r.f32(112)),
                scl_inter: f64::from(r.f32(116)),
                slice_end: i64::from(r.i16(120)),
                slice_code: i32::from(r.u8(122)),
                xyzt_units: i32::from(r.u8(123)),
                cal_max: f64::from(r.f32(124)),
                cal_min: f64::from(r.f32(128)),
                slice_duration: f64::from(r.f32(132)),
                toffset: f64::from(r.f32(136)),
                descrip: r.bytes(148),
                aux_file: r.bytes(228),
                qform_code: i32::from(r.i16(252)),
                sform_code: i32::from(r.i16(254)),
                quatern_b: f64::from(r.f32(256)),
                quatern_c: f64::from(r.f32(260)),
                quatern_d: f64::from(r.f32(264)),
                qoffset_x: f64::from(r.f32(268)),
                qoffset_y: f64::from(r.f32(272)),
                qoffset_z: f64::from(r.f32(276)),
                srow_x: std::array::from_fn(|i| f64::from(r.f32(280 + 4 * i))),
                srow_y: std::array::from_fn(|i| f64::from(r.f32(296 + 4 * i))),
                srow_z: std::array::from_fn(|i| f64::from(r.f32(312 + 4 * i))),
                intent_name: r.bytes(328),
                analyze: AnalyzeFields {
                    data_type: r.bytes(4),
                    db_name: r.bytes(14),
                    extents: r.i32(32),
                    session_error: r.i16(36),
                    regular: r.u8(38),
                    glmax: r.i32(140),
                    glmin: r.i32(144),
                },
                nifti2: Nifti2Fields {
                    eol_check: [0; 4],
                    unused_str: [0; 15],
                },
                extensions: Vec::new(),
            },
            NiftiVersion::V2 => NiftiHeader {
                version,
                byte_order: order,
                magic: r.bytes(4),
                dim_info: r.u8(524),
                dim: std::array::from_fn(|i| r.i64(16 + 8 * i)),
                intent_p1: r.f64(80),
                intent_p2: r.f64(88),
                intent_p3: r.f64(96),
                intent_code: r.i32(504),
                datatype: r.i16(12),
                bitpix: r.i16(14),
                slice_start: r.i64(224),
                pixdim: std::array::from_fn(|i| r.f64(104 + 8 * i)),
                vox_offset: r.i64(168) as f64,
                scl_slope: r.f64(176),
                scl_inter: r.f64(184),
                slice_end: r.i64(232),
                slice_code: r.i32(496),
                xyzt_units: r.i32(500),
                cal_max: r.f64(192),
                cal_min: r.f64(200),
                slice_duration: r.f64(208),
                toffset: r.f64(216),
                descrip: r.bytes(240),
                aux_file: r.bytes(320),
                qform_code: r.i32(344),
                sform_code: r.i32(348),
                quatern_b: r.f64(352),
                quatern_c: r.f64(360),
                quatern_d: r.f64(368),
                qoffset_x: r.f64(376),
                qoffset_y: r.f64(384),
                qoffset_z: r.f64(392),
                srow_x: std::array::from_fn(|i| r.f64(400 + 8 * i)),
                srow_y: std::array::from_fn(|i| r.f64(432 + 8 * i)),
                srow_z: std::array::from_fn(|i| r.f64(464 + 8 * i)),
                intent_name: r.bytes(508),
                analyze: AnalyzeFields::default(),
                nifti2: Nifti2Fields {
                    eol_check: r.bytes(8),
                    unused_str: r.bytes(525),
                },
                extensions: Vec::new(),
            },
        };
        Ok(hdr)
    }

    /// The header block in this header's version and byte order (without the extender or
    /// extensions).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = FieldWriter::new(self.version.header_size(), self.byte_order);
        match self.version {
            NiftiVersion::V1 => {
                w.i32(0, NIFTI1_HEADER_SIZE as i32);
                w.bytes(4, &self.analyze.data_type);
                w.bytes(14, &self.analyze.db_name);
                w.i32(32, self.analyze.extents);
                w.i16(36, self.analyze.session_error);
                w.u8(38, self.analyze.regular);
                w.u8(39, self.dim_info);
                for (i, d) in self.dim.iter().enumerate() {
                    w.i16(40 + 2 * i, *d as i16);
                }
                w.f32(56, self.intent_p1 as f32);
                w.f32(60, self.intent_p2 as f32);
                w.f32(64, self.intent_p3 as f32);
                w.i16(68, self.intent_code as i16);
                w.i16(70, self.datatype);
                w.i16(72, self.bitpix);
                w.i16(74, self.slice_start as i16);
                for (i, p) in self.pixdim.iter().enumerate() {
                    w.f32(76 + 4 * i, *p as f32);
                }
                w.f32(108, self.vox_offset as f32);
                w.f32(112, self.scl_slope as f32);
                w.f32(116, self.scl_inter as f32);
                w.i16(120, self.slice_end as i16);
                w.u8(122, self.slice_code as u8);
                w.u8(123, self.xyzt_units as u8);
                w.f32(124, self.cal_max as f32);
                w.f32(128, self.cal_min as f32);
                w.f32(132, self.slice_duration as f32);
                w.f32(136, self.toffset as f32);
                w.i32(140, self.analyze.glmax);
                w.i32(144, self.analyze.glmin);
                w.bytes(148, &self.descrip);
                w.bytes(228, &self.aux_file);
                w.i16(252, self.qform_code as i16);
                w.i16(254, self.sform_code as i16);
                w.f32(256, self.quatern_b as f32);
                w.f32(260, self.quatern_c as f32);
                w.f32(264, self.quatern_d as f32);
                w.f32(268, self.qoffset_x as f32);
                w.f32(272, self.qoffset_y as f32);
                w.f32(276, self.qoffset_z as f32);
                for (base, row) in [
                    (280, &self.srow_x),
                    (296, &self.srow_y),
                    (312, &self.srow_z),
                ] {
                    for (i, v) in row.iter().enumerate() {
                        w.f32(base + 4 * i, *v as f32);
                    }
                }
                w.bytes(328, &self.intent_name);
                w.bytes(344, &self.magic);
            }
            NiftiVersion::V2 => {
                w.i32(0, NIFTI2_HEADER_SIZE as i32);
                w.bytes(4, &self.magic);
                w.bytes(8, &self.nifti2.eol_check);
                w.i16(12, self.datatype);
                w.i16(14, self.bitpix);
                for (i, d) in self.dim.iter().enumerate() {
                    w.i64(16 + 8 * i, *d);
                }
                w.f64(80, self.intent_p1);
                w.f64(88, self.intent_p2);
                w.f64(96, self.intent_p3);
                for (i, p) in self.pixdim.iter().enumerate() {
                    w.f64(104 + 8 * i, *p);
                }
                w.i64(168, self.vox_offset as i64);
                w.f64(176, self.scl_slope);
                w.f64(184, self.scl_inter);
                w.f64(192, self.cal_max);
                w.f64(200, self.cal_min);
                w.f64(208, self.slice_duration);
                w.f64(216, self.toffset);
                w.i64(224, self.slice_start);
                w.i64(232, self.slice_end);
                w.bytes(240, &self.descrip);
                w.bytes(320, &self.aux_file);
                w.i32(344, self.qform_code);
                w.i32(348, self.sform_code);
                w.f64(352, self.quatern_b);
                w.f64(360, self.quatern_c);
                w.f64(368, self.quatern_d);
                w.f64(376, self.qoffset_x);
                w.f64(384, self.qoffset_y);
                w.f64(392, self.qoffset_z);
                for (base, row) in [
                    (400, &self.srow_x),
                    (432, &self.srow_y),
                    (464, &self.srow_z),
                ] {
                    for (i, v) in row.iter().enumerate() {
                        w.f64(base + 8 * i, *v);
                    }
                }
                w.i32(496, self.slice_code);
                w.i32(500, self.xyzt_units);
                w.i32(504, self.intent_code);
                w.bytes(508, &self.intent_name);
                w.u8(524, self.dim_info);
                w.bytes(525, &self.nifti2.unused_str);
            }
        }
        w.buf
    }

    // ----------------------------------------------------------------------------------------
    // Validation

    /// Whether the magic string marks a single file (`n+1`/`n+2`) rather than a pair.
    pub fn is_single_file(&self) -> bool {
        self.magic[1] == b'+'
    }

    /// Checks the header and fixes what nibabel fixes when it reads a file, in the same order:
    /// `bitpix` from the data type, zero spatial `pixdim` to 1, negative ones to their absolute
    /// value, a `qfac` (`pixdim[0]`) other than ±1 to 1, and unknown qform/sform codes to 0.
    /// Returns what was fixed; fails where nibabel fails (unknown data type, bad magic, a
    /// single-file `vox_offset` below the header size).
    pub fn apply_load_fixes(&mut self) -> Result<Vec<HeaderFix>, HeaderError> {
        let mut fixes = Vec::new();
        let data_type = datatype::from_code(self.datatype)
            .ok_or(HeaderError::UnsupportedDataType(self.datatype))?;
        let bitpix = i16::try_from(8 * data_type.size()).unwrap_or(i16::MAX);
        if self.bitpix != bitpix {
            self.bitpix = bitpix;
            fixes.push(HeaderFix("bitpix set to match the data type"));
        }
        let spatial = &mut self.pixdim[1..4];
        if spatial.contains(&0.0) {
            spatial
                .iter_mut()
                .filter(|p| **p == 0.0)
                .for_each(|p| *p = 1.0);
            fixes.push(HeaderFix("zero pixdim[1..=3] set to 1"));
        }
        if spatial.iter().any(|&p| p < 0.0) {
            spatial.iter_mut().for_each(|p| *p = p.abs());
            fixes.push(HeaderFix(
                "negative pixdim[1..=3] set to their absolute value",
            ));
        }
        if self.pixdim[0] != 1.0 && self.pixdim[0] != -1.0 {
            self.pixdim[0] = 1.0;
            fixes.push(HeaderFix("qfac (pixdim[0]) set to 1"));
        }
        let expected = [self.version.magic(true), self.version.magic(false)];
        if !expected.contains(&self.magic) {
            return Err(HeaderError::BadMagic(
                String::from_utf8_lossy(&self.magic).into_owned(),
            ));
        }
        let min = self.version.min_vox_offset();
        if self.is_single_file() && self.vox_offset != 0.0 && self.vox_offset < min as f64 {
            return Err(HeaderError::VoxOffsetTooSmall {
                offset: self.vox_offset,
                min,
            });
        }
        for (code, name) in [
            (&mut self.qform_code, "qform_code"),
            (&mut self.sform_code, "sform_code"),
        ] {
            if !xform::is_valid(*code) {
                *code = 0;
                fixes.push(HeaderFix(if name == "qform_code" {
                    "invalid qform_code set to 0"
                } else {
                    "invalid sform_code set to 0"
                }));
            }
        }
        Ok(fixes)
    }

    // ----------------------------------------------------------------------------------------
    // Data type and shape

    /// The voxel data type.
    pub fn data_type(&self) -> Result<DataType, HeaderError> {
        datatype::from_code(self.datatype).ok_or(HeaderError::UnsupportedDataType(self.datatype))
    }

    /// Sets `datatype` and `bitpix`.
    pub fn set_data_type(&mut self, data_type: DataType) {
        self.datatype = datatype::to_code(data_type);
        self.bitpix = i16::try_from(8 * data_type.size()).unwrap_or(i16::MAX);
    }

    /// Number of dimensions (`dim[0]`).
    pub fn ndim(&self) -> Result<usize, HeaderError> {
        usize::try_from(self.dim[0])
            .ok()
            .filter(|&n| n <= 7)
            .ok_or(HeaderError::BadNdim(self.dim[0]))
    }

    /// The data shape, `dim[1..=dim[0]]` (`[0]` when `dim[0]` is 0), including FreeSurfer's
    /// conventions for long vectors (`dim[1] == -1`, length in `glmin`) and for the ico7
    /// surface (`27307 × 1 × 6` means `163842 × 1 × 1`).
    pub fn shape(&self) -> Result<Vec<usize>, HeaderError> {
        let ndim = self.ndim()?;
        if ndim == 0 {
            return Ok(vec![0]);
        }
        let raw = &self.dim[1..=ndim];
        let mut shape: Vec<i64> = raw.to_vec();
        if self.version == NiftiVersion::V1 && ndim >= 3 {
            if raw[..3] == [-1, 1, 1] {
                if self.analyze.glmin == 0 {
                    return Err(HeaderError::BadFreeSurferVector);
                }
                shape[0] = i64::from(self.analyze.glmin);
            } else if raw[..3] == [27307, 1, 6] {
                shape[..3].copy_from_slice(&[163_842, 1, 1]);
            }
        }
        shape
            .iter()
            .enumerate()
            .map(|(i, &d)| {
                usize::try_from(d).map_err(|_| HeaderError::NegativeDim {
                    index: i + 1,
                    value: d,
                })
            })
            .collect()
    }

    /// Sets `dim` from `shape` (nibabel's `set_data_shape`): unused dimensions become 1, and
    /// `pixdim` beyond the last dimension becomes 1. NIfTI-1 uses FreeSurfer's conventions for
    /// vectors longer than `i16::MAX` and for the ico7 surface.
    pub fn set_shape(&mut self, shape: &[usize]) -> Result<(), HeaderError> {
        let too_large = || HeaderError::ShapeTooLarge(shape.to_vec());
        if shape.len() > 7 {
            return Err(too_large());
        }
        let mut dims: Vec<i64> = shape
            .iter()
            .map(|&d| i64::try_from(d).map_err(|_| too_large()))
            .collect::<Result<_, _>>()?;
        if self.version == NiftiVersion::V1 {
            if dims.len() >= 3 && dims[..3] == [163_842, 1, 1] {
                dims[..3].copy_from_slice(&[27307, 1, 6]);
            } else if dims.len() >= 3 && dims[1..3] == [1, 1] && dims[0] > i64::from(i16::MAX) {
                self.analyze.glmin = i32::try_from(dims[0]).map_err(|_| too_large())?;
                dims[0] = -1;
            }
            if dims.iter().any(|&d| d > i64::from(i16::MAX)) {
                return Err(too_large());
            }
        }
        self.dim = [1; 8];
        self.dim[0] = dims.len() as i64;
        self.dim[1..=dims.len()].copy_from_slice(&dims);
        for p in &mut self.pixdim[dims.len() + 1..] {
            *p = 1.0;
        }
        Ok(())
    }

    /// Voxel sizes and time step for each dimension: `pixdim[1..=dim[0]]` (`[1.0]` when
    /// `dim[0]` is 0).
    pub fn zooms(&self) -> Result<Vec<f64>, HeaderError> {
        let ndim = self.ndim()?;
        Ok(if ndim == 0 {
            vec![1.0]
        } else {
            self.pixdim[1..=ndim].to_vec()
        })
    }

    // ----------------------------------------------------------------------------------------
    // Scaling

    /// The scaling `value = stored · slope + inter`, or `None` when the slope is 0 or not
    /// finite (nibabel's `get_slope_inter`).
    pub fn slope_inter(&self) -> Result<Option<(f64, f64)>, HeaderError> {
        let (slope, inter) = (self.scl_slope, self.scl_inter);
        if slope == 0.0 || !slope.is_finite() {
            return Ok(None);
        }
        if !inter.is_finite() {
            return Err(HeaderError::NonFiniteIntercept(inter));
        }
        Ok(Some((slope, inter)))
    }

    /// Whether reading applies a scaling other than the identity.
    pub fn is_scaled(&self) -> Result<bool, HeaderError> {
        Ok(self
            .slope_inter()?
            .is_some_and(|(s, i)| s != 1.0 || i != 0.0))
    }

    // ----------------------------------------------------------------------------------------
    // Spatial transforms

    /// The affine from the qform fields (quaternion, `pixdim`, offsets), whatever
    /// `qform_code` says. Follows nibabel's `get_qform`: the quaternion's `w` is
    /// `√(1 − b² − c² − d²)`, or 0 when that is within 3 machine epsilons (of f32 for
    /// NIfTI-1, f64 for NIfTI-2) of zero.
    pub fn qform(&self) -> Result<Affine, HeaderError> {
        let (b, c, d) = (self.quatern_b, self.quatern_c, self.quatern_d);
        let threshold = match self.version {
            NiftiVersion::V1 => 3.0 * f64::from(f32::EPSILON),
            NiftiVersion::V2 => 3.0 * f64::EPSILON,
        };
        let w2 = 1.0 - (b * b + c * c + d * d);
        let w = if w2.abs() < threshold {
            0.0
        } else if w2 < 0.0 {
            return Err(HeaderError::NonUnitQuaternion(w2));
        } else {
            w2.sqrt()
        };
        let r = rotation::quaternion_to_matrix([w, b, c, d]);
        let qfac = if self.pixdim[0] == -1.0 { -1.0 } else { 1.0 };
        let vox = [self.pixdim[1], self.pixdim[2], self.pixdim[3] * qfac];
        let linear: Mat3 = [0, 1, 2].map(|i| [0, 1, 2].map(|j| r[i][j] * vox[j]));
        Ok(Affine::from_linear(
            linear,
            [self.qoffset_x, self.qoffset_y, self.qoffset_z],
        ))
    }

    /// The affine from the sform rows, whatever `sform_code` says.
    pub fn sform(&self) -> Affine {
        Affine::from_rows([self.srow_x, self.srow_y, self.srow_z, [0.0, 0.0, 0.0, 1.0]])
    }

    /// The affine nibabel derives from the shape and voxel sizes alone, used when neither
    /// sform nor qform is set: the image is centred on the origin and the x axis flipped
    /// (Analyze's radiological default).
    pub fn base_affine(&self) -> Result<Affine, HeaderError> {
        let ndim = self.ndim()?;
        let mut shape = [1.0f64; 3];
        let mut zooms = [1.0f64; 3];
        for i in 0..ndim.min(3) {
            shape[i] = self.dim[i + 1] as f64;
            zooms[i] = self.pixdim[i + 1];
        }
        zooms[0] = -zooms[0];
        let translation = [0, 1, 2].map(|i| -((shape[i] - 1.0) / 2.0) * zooms[i]);
        Ok(Affine::from_zooms(zooms, translation))
    }

    /// The image affine: the sform if `sform_code > 0`, else the qform if `qform_code > 0`,
    /// else [`NiftiHeader::base_affine`] (nibabel's `get_best_affine`).
    pub fn best_affine(&self) -> Result<Affine, HeaderError> {
        if self.sform_code != 0 {
            Ok(self.sform())
        } else if self.qform_code != 0 {
            self.qform()
        } else {
            self.base_affine()
        }
    }

    /// Stores `affine` in the sform rows with `code`.
    pub fn set_sform(&mut self, affine: &Affine, code: i32) {
        let m = affine.rows();
        self.srow_x = m[0];
        self.srow_y = m[1];
        self.srow_z = m[2];
        self.sform_code = code;
    }

    /// Stores `affine` in the qform fields with `code` (nibabel's `set_qform`): voxel sizes
    /// go to `pixdim[1..=3]`, a reflection to `qfac`, and shears are removed by taking the
    /// closest rotation (polar decomposition).
    pub fn set_qform(&mut self, affine: &Affine, code: i32) -> Result<(), HeaderError> {
        let zooms = affine.voxel_sizes();
        if zooms.iter().any(|&z| z == 0.0 || !z.is_finite()) {
            return Err(HeaderError::BadAffine(
                "a voxel axis has zero or non-finite length",
            ));
        }
        let linear = affine.linear();
        let mut r: Mat3 = [0, 1, 2].map(|i| [0, 1, 2].map(|j| linear[i][j] / zooms[j]));
        let qfac = if linalg::det3(&r) > 0.0 {
            1.0
        } else {
            for row in &mut r {
                row[2] = -row[2];
            }
            -1.0
        };
        let rotation =
            linalg::polar_orthogonal(&r).ok_or(HeaderError::BadAffine("singular rotation part"))?;
        let [_, b, c, d] = rotation::matrix_to_quaternion(&rotation);
        let t = affine.translation();
        self.qoffset_x = t[0];
        self.qoffset_y = t[1];
        self.qoffset_z = t[2];
        self.pixdim[0] = qfac;
        self.pixdim[1..4].copy_from_slice(&zooms);
        self.quatern_b = b;
        self.quatern_c = c;
        self.quatern_d = d;
        self.qform_code = code;
        Ok(())
    }

    /// Sets the affine as nibabel does when an image's affine differs from its header's: sform
    /// with code 2 (aligned), qform with code 0 (unknown), voxel sizes from the affine.
    pub fn set_affine(&mut self, affine: &Affine) -> Result<(), HeaderError> {
        self.set_sform(affine, xform::ALIGNED_ANAT);
        self.set_qform(affine, xform::UNKNOWN)
    }

    /// Converts the header to another version, keeping every field the target can store.
    pub fn with_version(&self, version: NiftiVersion) -> Self {
        let mut out = self.clone();
        if version != self.version {
            out.version = version;
            out.magic = version.magic(self.is_single_file());
            out.analyze = AnalyzeFields::default();
            out.nifti2 = Nifti2Fields::default();
        }
        out
    }
}

/// Text fields (`descrip`, `aux_file`, `intent_name`) without their trailing NULs.
pub fn text_field(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oblique() -> Affine {
        let (c, s) = (0.3f64.cos(), 0.3f64.sin());
        Affine::from_linear(
            [
                [2.0 * c, 0.0, -2.5 * s],
                [0.0, 2.2, 0.0],
                [2.0 * s, 0.0, 2.5 * c],
            ],
            [-90.0, 126.0, -72.0],
        )
    }

    #[test]
    fn bytes_round_trip_in_both_versions_and_orders() {
        for version in [NiftiVersion::V1, NiftiVersion::V2] {
            for order in [ByteOrder::Little, ByteOrder::Big] {
                let mut h = NiftiHeader::new(version, &[64, 64, 30, 200], DataType::I16).unwrap();
                h.byte_order = order;
                h.set_affine(&oblique()).unwrap();
                h.pixdim[4] = 2.0;
                h.xyzt_units = 10;
                h.descrip[..5].copy_from_slice(b"hello");
                let bytes = h.to_bytes();
                assert_eq!(bytes.len(), version.header_size());
                let parsed = NiftiHeader::parse(&bytes).unwrap();
                // Headers hold NaN scale factors, so compare their serialised form.
                assert_eq!(parsed.to_bytes(), bytes);
                assert_eq!(parsed.byte_order, order);
                assert_eq!(parsed.shape().unwrap(), vec![64, 64, 30, 200]);
                assert_eq!(text_field(&parsed.descrip), "hello");
            }
        }
    }

    #[test]
    fn qform_round_trip_with_reflection() {
        let mut flipped = oblique().rows().to_owned();
        for row in flipped.iter_mut().take(3) {
            row[0] = -row[0]; // radiological (left-handed) storage
        }
        for affine in [
            oblique(),
            Affine::from_rows(flipped),
            Affine::from_zooms([-1.0, -1.0, 1.0], [0.0; 3]),
        ] {
            let mut h = NiftiHeader::new(NiftiVersion::V2, &[4, 5, 6], DataType::F32).unwrap();
            h.set_qform(&affine, xform::SCANNER_ANAT).unwrap();
            assert!(
                h.qform().unwrap().allclose(&affine, 0.0, 1e-12),
                "{:?} vs {affine:?}",
                h.qform()
            );
        }
    }

    #[test]
    fn best_affine_precedence() {
        let mut h = NiftiHeader::new(NiftiVersion::V1, &[3, 4, 5], DataType::U8).unwrap();
        h.pixdim[1..4].copy_from_slice(&[2.0, 3.0, 4.0]);
        let base = h.best_affine().unwrap();
        assert_eq!(base.rows()[0], [-2.0, 0.0, 0.0, 2.0]);
        assert_eq!(base.rows()[1], [0.0, 3.0, 0.0, -4.5]);
        let q = Affine::from_zooms([2.0, 3.0, 4.0], [1.0, 2.0, 3.0]);
        h.set_qform(&q, xform::SCANNER_ANAT).unwrap();
        assert_eq!(h.best_affine().unwrap(), q);
        let s = Affine::from_zooms([5.0, 5.0, 5.0], [0.0; 3]);
        h.set_sform(&s, xform::MNI_152);
        assert_eq!(h.best_affine().unwrap(), s);
    }

    #[test]
    fn load_fixes_follow_nibabel() {
        let mut h = NiftiHeader::new(NiftiVersion::V1, &[2, 2, 2], DataType::F32).unwrap();
        h.bitpix = 8;
        h.pixdim[1..4].copy_from_slice(&[0.0, -2.0, 3.0]);
        h.pixdim[0] = 0.0;
        h.qform_code = 9;
        let fixes = h.apply_load_fixes().unwrap();
        assert_eq!(fixes.len(), 5, "{fixes:?}");
        assert_eq!(
            (h.bitpix, h.pixdim[0], &h.pixdim[1..4], h.qform_code),
            (32, 1.0, &[1.0, 2.0, 3.0][..], 0)
        );
        h.magic = *b"xyz\0";
        assert!(matches!(
            h.apply_load_fixes(),
            Err(HeaderError::BadMagic(_))
        ));
        h.magic = NiftiVersion::V1.magic(true);
        h.datatype = 1536; // float128
        assert_eq!(
            h.apply_load_fixes(),
            Err(HeaderError::UnsupportedDataType(1536))
        );
    }

    #[test]
    fn freesurfer_shape_conventions() {
        let mut h = NiftiHeader::new(NiftiVersion::V1, &[100_000, 1, 1], DataType::F32).unwrap();
        assert_eq!(h.dim[1], -1);
        assert_eq!(h.analyze.glmin, 100_000);
        assert_eq!(h.shape().unwrap(), vec![100_000, 1, 1]);
        h.set_shape(&[163_842, 1, 1, 2]).unwrap();
        assert_eq!(&h.dim[..5], &[4, 27307, 1, 6, 2]);
        assert_eq!(h.shape().unwrap(), vec![163_842, 1, 1, 2]);
        assert!(NiftiHeader::new(NiftiVersion::V1, &[40_000, 2, 1], DataType::U8).is_err());
        assert!(NiftiHeader::new(NiftiVersion::V2, &[40_000, 2, 1], DataType::U8).is_ok());
    }

    #[test]
    fn scaling_rules() {
        let mut h = NiftiHeader::new(NiftiVersion::V1, &[1], DataType::I16).unwrap();
        assert_eq!(h.slope_inter(), Ok(Some((1.0, 0.0))));
        assert!(!h.is_scaled().unwrap());
        h.scl_slope = f64::NAN;
        assert_eq!(h.slope_inter(), Ok(None));
        h.scl_slope = 0.0;
        assert_eq!(h.slope_inter(), Ok(None));
        h.scl_slope = 2.0;
        h.scl_inter = f64::INFINITY;
        assert!(h.slope_inter().is_err());
        h.scl_inter = -1.0;
        assert_eq!(h.slope_inter(), Ok(Some((2.0, -1.0))));
        assert!(h.is_scaled().unwrap());
        h.scl_slope = 1.0;
        h.scl_inter = 0.0;
        assert!(!h.is_scaled().unwrap());
    }

    #[test]
    fn sniff_uses_dim0_like_nibabel() {
        let h = NiftiHeader::new(NiftiVersion::V1, &[2, 3, 4], DataType::U8).unwrap();
        let mut bytes = h.to_bytes();
        assert_eq!(
            NiftiHeader::sniff(&bytes).unwrap(),
            (NiftiVersion::V1, ByteOrder::NATIVE)
        );
        bytes[..4].copy_from_slice(&348i32.to_be_bytes()); // sizeof_hdr in the "wrong" order
        assert_eq!(NiftiHeader::sniff(&bytes).unwrap().1, ByteOrder::NATIVE);
        assert_eq!(NiftiHeader::sniff(b"\0\0\0\0"), Err(HeaderError::NotNifti));
    }
}
