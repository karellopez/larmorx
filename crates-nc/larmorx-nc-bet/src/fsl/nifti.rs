// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret (the translation).
//! FSL's NIfTI and ANALYZE file layer (fsl-newnifti 4.1.0: `NewNifti.cc`, `legacyFunctions.cc`),
//! as FSL 6.0.7.17's programs read and write images.
//!
//! - [`NiftiHeader`] mirrors NewNifti's `NiftiHeader`: the fields a program sees after reading
//!   (in `double` and `int64_t`, converted from the file's types) and the fields it writes.
//! - [`load_image`] reads a header, its extensions and its data (byte-swapped to the machine's
//!   order); [`save_image`] writes a header, extensions and data (`gzip` when asked).
//! - [`legacy`]: nifti1_io's quaternion and matrix routines in NewNifti's versions, with their
//!   `float` storage and `long double` arithmetic (`x87`).
//!
//! Files are read through gzip transparently, as `znzopen` with compression does (a plain file
//! is read as is); written files are compressed with zlib's default level, whose bytes are not
//! compared (only the decompressed bytes are, docs/validation).

// index loops, as the C and LAPACK code they translate
#![allow(clippy::needless_range_loop)]

use std::io::{Read, Write};
use std::path::Path;

use crate::fsl::x87;

/// NIfTI data type codes.
pub mod dt {
    pub const UNKNOWN: i16 = 0;
    pub const BINARY: i16 = 1;
    pub const UNSIGNED_CHAR: i16 = 2;
    pub const SIGNED_SHORT: i16 = 4;
    pub const SIGNED_INT: i16 = 8;
    pub const FLOAT: i16 = 16;
    pub const COMPLEX: i16 = 32;
    pub const DOUBLE: i16 = 64;
    pub const RGB: i16 = 128;
    pub const INT8: i16 = 256;
    pub const UINT16: i16 = 512;
    pub const UINT32: i16 = 768;
    pub const INT64: i16 = 1024;
    pub const UINT64: i16 = 1280;
    pub const FLOAT128: i16 = 1536;
    pub const COMPLEX128: i16 = 1792;
    pub const COMPLEX256: i16 = 2048;
    pub const RGBA32: i16 = 2304;
}

pub const XFORM_UNKNOWN: i32 = 0;
pub const XFORM_ALIGNED_ANAT: i32 = 2;
pub const UNITS_MM: i32 = 2;
pub const UNITS_SEC: i32 = 8;

/// A NewNifti exception: the message its `what()` returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NiftiError(pub String);

impl std::fmt::Display for NiftiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NiftiError {}

fn err(msg: impl Into<String>) -> NiftiError {
    NiftiError(msg.into())
}

/// A NIfTI extension (`esize`, `ecode` and its data).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extension {
    pub esize: i32,
    pub ecode: i32,
    pub edata: Vec<u8>,
}

impl Extension {
    pub fn size(&self) -> usize {
        8 + self.edata.len()
    }
}

/// The ANALYZE-only and NIfTI-1 legacy fields (NewNifti's `LegacyFields`).
#[derive(Clone, Debug, PartialEq)]
pub struct LegacyFields {
    pub data_type: [u8; 10],
    pub db_name: [u8; 18],
    pub extents: i32,
    pub session_error: i16,
    pub regular: u8,
    pub hkey_un0: u8,
    pub vox_units: [u8; 4],
    pub cal_units: [u8; 8],
    pub unused1: i16,
    pub dim_un0: i16,
    pub funused1: f32,
    pub funused2: f32,
    pub funused3: f32,
    pub compressed: i32,
    pub verified: i32,
    pub glmax: i32,
    pub glmin: i32,
    pub orient: u8,
    pub originator: [u8; 10],
    pub generated: [u8; 10],
    pub scannum: [u8; 10],
    pub patient_id: [u8; 10],
    pub exp_date: [u8; 10],
    pub exp_time: [u8; 10],
    pub hist_un0: [u8; 3],
    pub views: i32,
    pub vols_added: i32,
    pub start_field: i32,
    pub field_skip: i32,
    pub omax: i32,
    pub omin: i32,
    pub smax: i32,
    pub smin: i32,
}

impl Default for LegacyFields {
    fn default() -> Self {
        LegacyFields {
            data_type: [0; 10],
            db_name: [0; 18],
            extents: 16384,
            session_error: 0,
            regular: b'r',
            hkey_un0: 0,
            vox_units: [0; 4],
            cal_units: [0; 8],
            unused1: 0,
            dim_un0: 0,
            funused1: 0.0,
            funused2: 0.0,
            funused3: 0.0,
            compressed: 0,
            verified: 0,
            glmax: 0,
            glmin: 0,
            orient: 0,
            originator: [0; 10],
            generated: [0; 10],
            scannum: [0; 10],
            patient_id: [0; 10],
            exp_date: [0; 10],
            exp_time: [0; 10],
            hist_un0: [0; 3],
            views: 0,
            vols_added: 0,
            start_field: 0,
            field_skip: 0,
            omax: 0,
            omin: 0,
            smax: 0,
            smin: 0,
        }
    }
}

impl LegacyFields {
    /// The first three of the five shorts in `originator` (SPM's origin), in the machine's
    /// order (NewNifti swaps them for a byte-swapped ANALYZE file).
    pub fn origin(&self) -> [i16; 3] {
        let o = &self.originator;
        [
            i16::from_le_bytes([o[0], o[1]]),
            i16::from_le_bytes([o[2], o[3]]),
            i16::from_le_bytes([o[4], o[5]]),
        ]
    }

    pub fn set_origin(&mut self, origin: [i16; 3]) {
        for (k, v) in origin.iter().enumerate() {
            self.originator[2 * k..2 * k + 2].copy_from_slice(&v.to_le_bytes());
        }
    }
}

/// The header of an image as NewNifti holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct NiftiHeader {
    pub sizeof_hdr: i32,
    pub vox_offset: i64,
    /// 4 bytes for NIfTI-1 and ANALYZE, 8 for NIfTI-2.
    pub magic: Vec<u8>,
    pub datatype: i16,
    pub bits_per_voxel: i32,
    pub dim: [i64; 8],
    pub pixdim: [f64; 8],
    pub intent_code: i32,
    pub intent_p1: f64,
    pub intent_p2: f64,
    pub intent_p3: f64,
    /// The bytes of `intent_name` up to its first NUL.
    pub intent_name: Vec<u8>,
    pub scl_slope: f64,
    pub scl_inter: f64,
    pub cal_max: f64,
    pub cal_min: f64,
    pub slice_duration: f64,
    pub toffset: f64,
    pub slice_start: i64,
    pub slice_end: i64,
    /// The bytes of `descrip` up to its first NUL (the whole buffer when written fresh: 80
    /// NULs, which write as NULs).
    pub description: Vec<u8>,
    pub aux_file: Vec<u8>,
    pub qform_code: i32,
    pub sform_code: i32,
    pub qb: f64,
    pub qc: f64,
    pub qd: f64,
    pub qx: f64,
    pub qy: f64,
    pub qz: f64,
    pub sx: [f64; 4],
    pub sy: [f64; 4],
    pub sz: [f64; 4],
    pub slice_code: i32,
    pub units: i32,
    pub slice_ordering: u8,
    pub legacy: LegacyFields,
    pub was_wrong_endian: bool,
}

impl Default for NiftiHeader {
    /// NewNifti's `NiftiHeader()` (`initialise`).
    fn default() -> Self {
        NiftiHeader {
            sizeof_hdr: 0,
            vox_offset: 0,
            magic: vec![0; 4],
            datatype: 0,
            bits_per_voxel: 0,
            dim: [0; 8],
            pixdim: [0.0; 8],
            intent_code: 0,
            intent_p1: 0.0,
            intent_p2: 0.0,
            intent_p3: 0.0,
            intent_name: vec![0; 16],
            scl_slope: 1.0,
            scl_inter: 0.0,
            cal_max: 0.0,
            cal_min: 0.0,
            slice_duration: 0.0,
            toffset: 0.0,
            slice_start: 0,
            slice_end: 0,
            description: vec![0; 80],
            aux_file: vec![0; 24],
            qform_code: 0,
            sform_code: 0,
            qb: 0.0,
            qc: 0.0,
            qd: 0.0,
            qx: 0.0,
            qy: 0.0,
            qz: 0.0,
            sx: [0.0; 4],
            sy: [0.0; 4],
            sz: [0.0; 4],
            slice_code: 0,
            units: 0,
            slice_ordering: 0,
            legacy: LegacyFields::default(),
            was_wrong_endian: false,
        }
    }
}

/// NewNifti's `NIFTI2_VERSION`: 1 for a 348-byte header, 2 for a 540-byte one (either byte
/// order), 0 otherwise.
fn nifti2_version(sizeof_hdr: i32) -> i32 {
    match sizeof_hdr {
        348 | 1543569408 => 1,
        540 | 469893120 => 2,
        _ => 0,
    }
}

/// NIfTI-1's `NIFTI_VERSION` on the magic bytes.
fn nifti_version(magic: &[u8]) -> i32 {
    if magic.len() >= 4
        && magic[0] == b'n'
        && magic[3] == 0
        && (magic[1] == b'i' || magic[1] == b'+')
        && (b'1'..=b'9').contains(&magic[2])
    {
        i32::from(magic[2] - b'0')
    } else {
        0
    }
}

impl NiftiHeader {
    pub fn nifti_version(&self) -> i32 {
        nifti2_version(self.sizeof_hdr)
    }

    pub fn is_analyze(&self) -> bool {
        nifti2_version(self.sizeof_hdr) != 0 && nifti_version(&self.magic) == 0
    }

    pub fn single_file(&self) -> bool {
        self.magic.len() > 1 && self.magic[1] == b'+'
    }

    pub fn bpv_of_datatype(&self) -> i32 {
        match self.datatype {
            dt::UNSIGNED_CHAR | dt::INT8 => 8,
            dt::SIGNED_SHORT | dt::UINT16 => 16,
            dt::SIGNED_INT | dt::FLOAT | dt::UINT32 => 32,
            dt::INT64 | dt::UINT64 | dt::DOUBLE | dt::COMPLEX => 64,
            dt::FLOAT128 => 128,
            _ => 0,
        }
    }

    pub fn datum_byte_width(&self) -> usize {
        (self.bpv_of_datatype() / 8) as usize
    }

    /// The number of values: `dim[1] * ... * dim[dim[0]]` (C `size_t` arithmetic).
    pub fn n_elements(&self) -> usize {
        let mut n = self.dim[1] as usize;
        let d0 = self.dim[0];
        let mut k = 2;
        while k <= d0 && k <= 7 {
            n = n.wrapping_mul(self.dim[k as usize] as usize);
            k += 1;
        }
        n
    }

    pub fn left_handed(&self) -> f64 {
        if self.pixdim[0] < 0.0 { -1.0 } else { 1.0 }
    }

    /// `nominalVoxOffset`: 352 at least for a single file.
    pub fn nominal_vox_offset(&self) -> i64 {
        if self.single_file() && self.vox_offset < 352 {
            352
        } else {
            self.vox_offset
        }
    }

    /// `getQForm`: the matrix of the quaternion, in `float`.
    pub fn qform(&self) -> [[f32; 4]; 4] {
        legacy::quatern_to_mat44(
            self.qb as f32,
            self.qc as f32,
            self.qd as f32,
            self.qx as f32,
            self.qy as f32,
            self.qz as f32,
            self.pixdim[1] as f32,
            self.pixdim[2] as f32,
            self.pixdim[3] as f32,
            self.left_handed() as f32,
        )
    }

    /// `setQForm`: the quaternion of `m`, and its handedness into `pixdim[0]`.
    pub fn set_qform(&mut self, m: &[[f32; 4]; 4]) {
        let q = legacy::mat44_to_quatern(m);
        self.qb = q.qb;
        self.qc = q.qc;
        self.qd = q.qd;
        self.qx = q.qx;
        self.qy = q.qy;
        self.qz = q.qz;
        self.pixdim[0] = q.qfac;
    }

    /// `getSForm`, in `float`.
    pub fn sform(&self) -> [[f32; 4]; 4] {
        let mut r = [[0f32; 4]; 4];
        for j in 0..4 {
            r[0][j] = self.sx[j] as f32;
            r[1][j] = self.sy[j] as f32;
            r[2][j] = self.sz[j] as f32;
        }
        r[3][3] = 1.0;
        r
    }

    pub fn set_sform(&mut self, m: &[[f32; 4]; 4]) {
        for j in 0..4 {
            self.sx[j] = f64::from(m[0][j]);
            self.sy[j] = f64::from(m[1][j]);
            self.sz[j] = f64::from(m[2][j]);
        }
    }

    /// `setNiftiVersion`.
    pub fn set_nifti_version(&mut self, version: i32, single_file: bool) {
        self.sizeof_hdr = 348;
        if version == 0 {
            return;
        }
        let mut magic = vec![b'n', b'i', b'1', 0];
        if single_file {
            magic[1] = b'+';
        }
        magic[2] = b'0' + version as u8;
        if version == 2 {
            magic.extend_from_slice(b"\r\n\x1a\n");
            self.sizeof_hdr = 540;
        }
        self.magic = magic;
    }
}

// ------------------------------------------------------------------------------------------------
// Raw headers

struct Bytes<'a> {
    b: &'a [u8],
    swap: bool,
}

impl Bytes<'_> {
    fn arr<const N: usize>(&self, at: usize) -> [u8; N] {
        let mut a = [0u8; N];
        a.copy_from_slice(&self.b[at..at + N]);
        if self.swap {
            a.reverse();
        }
        a
    }
    fn i16(&self, at: usize) -> i16 {
        i16::from_le_bytes(self.arr(at))
    }
    fn i32(&self, at: usize) -> i32 {
        i32::from_le_bytes(self.arr(at))
    }
    fn i64(&self, at: usize) -> i64 {
        i64::from_le_bytes(self.arr(at))
    }
    fn f32(&self, at: usize) -> f32 {
        f32::from_le_bytes(self.arr(at))
    }
    fn f64(&self, at: usize) -> f64 {
        f64::from_le_bytes(self.arr(at))
    }
    fn raw(&self, at: usize, n: usize) -> &[u8] {
        &self.b[at..at + n]
    }
}

/// A C string read from a fixed field as `std::string(char*)` does: up to the first NUL. When
/// the field has none, C++ goes on reading the next fields; NewNifti's callers keep at most the
/// field's length of it (`strncpy` on writing), so the field's own bytes are kept.
fn c_string(field: &[u8]) -> Vec<u8> {
    let end = field.iter().position(|&c| c == 0).unwrap_or(field.len());
    field[..end].to_vec()
}

fn arr_copy<const N: usize>(b: &[u8]) -> [u8; N] {
    let mut a = [0u8; N];
    a.copy_from_slice(&b[..N]);
    a
}

fn read_nifti1_or_analyze(b: &[u8], swap: bool, analyze: bool) -> NiftiHeader {
    let r = Bytes { b, swap };
    let mut h = NiftiHeader {
        sizeof_hdr: r.i32(0),
        ..NiftiHeader::default()
    };
    for k in 0..8 {
        h.dim[k] = i64::from(r.i16(40 + 2 * k));
        h.pixdim[k] = f64::from(r.f32(76 + 4 * k));
    }
    h.datatype = r.i16(70);
    h.bits_per_voxel = i32::from(r.i16(72));
    h.vox_offset = r.f32(108) as i64;
    h.cal_max = f64::from(r.f32(124));
    h.cal_min = f64::from(r.f32(128));
    h.description = c_string(r.raw(148, 80));
    h.aux_file = c_string(r.raw(228, 24));
    let mut l = LegacyFields {
        data_type: arr_copy(r.raw(4, 10)),
        db_name: arr_copy(r.raw(14, 18)),
        extents: r.i32(32),
        session_error: r.i16(36),
        regular: b[38],
        glmax: r.i32(140),
        glmin: r.i32(144),
        ..LegacyFields::default()
    };
    if analyze {
        l.hkey_un0 = b[39];
        // NewNifti copies cal_units over vox_units (its readFieldsFromRaw), keeping cal_units
        // at its initial zeros.
        let cal: [u8; 8] = arr_copy(r.raw(60, 8));
        l.vox_units.copy_from_slice(&cal[..4]);
        l.unused1 = r.i16(68);
        l.dim_un0 = r.i16(74);
        l.funused1 = r.f32(112);
        l.funused2 = r.f32(116);
        l.funused3 = r.f32(120);
        l.compressed = r.i32(132);
        l.verified = r.i32(136);
        l.orient = b[252];
        let mut orig: [u8; 10] = arr_copy(r.raw(253, 10));
        if swap {
            for k in 0..5 {
                orig.swap(2 * k, 2 * k + 1);
            }
        }
        l.originator = orig;
        l.generated = arr_copy(r.raw(263, 10));
        l.scannum = arr_copy(r.raw(273, 10));
        l.patient_id = arr_copy(r.raw(283, 10));
        l.exp_date = arr_copy(r.raw(293, 10));
        l.exp_time = arr_copy(r.raw(303, 10));
        l.hist_un0 = arr_copy(r.raw(313, 3));
        l.views = r.i32(316);
        l.vols_added = r.i32(320);
        l.start_field = r.i32(324);
        l.field_skip = r.i32(328);
        l.omax = r.i32(332);
        l.omin = r.i32(336);
        l.smax = r.i32(340);
        l.smin = r.i32(344);
        h.legacy = l;
        return h;
    }
    h.legacy = l;
    h.intent_p1 = f64::from(r.f32(56));
    h.intent_p2 = f64::from(r.f32(60));
    h.intent_p3 = f64::from(r.f32(64));
    h.intent_name = c_string(r.raw(328, 16));
    h.intent_code = i32::from(r.i16(68));
    h.slice_ordering = b[39];
    h.slice_start = i64::from(r.i16(74));
    h.slice_end = i64::from(r.i16(120));
    h.slice_code = i32::from(b[122] as i8);
    h.slice_duration = f64::from(r.f32(132));
    h.scl_slope = f64::from(r.f32(112));
    h.scl_inter = f64::from(r.f32(116));
    for j in 0..4 {
        h.sx[j] = f64::from(r.f32(280 + 4 * j));
        h.sy[j] = f64::from(r.f32(296 + 4 * j));
        h.sz[j] = f64::from(r.f32(312 + 4 * j));
    }
    h.qb = f64::from(r.f32(256));
    h.qc = f64::from(r.f32(260));
    h.qd = f64::from(r.f32(264));
    h.qx = f64::from(r.f32(268));
    h.qy = f64::from(r.f32(272));
    h.qz = f64::from(r.f32(276));
    h.sform_code = i32::from(r.i16(254));
    h.qform_code = i32::from(r.i16(252));
    h.units = i32::from(b[123] as i8);
    h.toffset = f64::from(r.f32(136));
    h.magic = b[344..348].to_vec();
    h
}

fn read_nifti2(b: &[u8], swap: bool) -> NiftiHeader {
    let r = Bytes { b, swap };
    let mut h = NiftiHeader {
        sizeof_hdr: r.i32(0),
        ..NiftiHeader::default()
    };
    h.magic = b[4..12].to_vec();
    h.datatype = r.i16(12);
    h.bits_per_voxel = i32::from(r.i16(14));
    for k in 0..8 {
        h.dim[k] = r.i64(16 + 8 * k);
        h.pixdim[k] = r.f64(104 + 8 * k);
    }
    h.intent_p1 = r.f64(80);
    h.intent_p2 = r.f64(88);
    h.intent_p3 = r.f64(96);
    h.vox_offset = r.i64(168);
    h.scl_slope = r.f64(176);
    h.scl_inter = r.f64(184);
    h.cal_max = r.f64(192);
    h.cal_min = r.f64(200);
    h.slice_duration = r.f64(208);
    h.toffset = r.f64(216);
    h.slice_start = r.i64(224);
    h.slice_end = r.i64(232);
    h.description = c_string(r.raw(240, 80));
    h.aux_file = c_string(r.raw(320, 24));
    h.qform_code = r.i32(344);
    h.sform_code = r.i32(348);
    h.qb = r.f64(352);
    h.qc = r.f64(360);
    h.qd = r.f64(368);
    h.qx = r.f64(376);
    h.qy = r.f64(384);
    h.qz = r.f64(392);
    for j in 0..4 {
        h.sx[j] = r.f64(400 + 8 * j);
        h.sy[j] = r.f64(432 + 8 * j);
        h.sz[j] = r.f64(464 + 8 * j);
    }
    h.slice_code = r.i32(496);
    h.units = r.i32(500);
    h.intent_code = r.i32(504);
    h.intent_name = c_string(r.raw(508, 16));
    h.slice_ordering = b[524];
    h
}

/// `strncpy(dst, src, n)`: the bytes up to the first NUL, padded with NULs.
fn strncpy(dst: &mut [u8], src: &[u8]) {
    let end = src.iter().position(|&c| c == 0).unwrap_or(src.len());
    let n = end.min(dst.len());
    dst[..n].copy_from_slice(&src[..n]);
    for d in &mut dst[n..] {
        *d = 0;
    }
}

/// C's conversion of a wider integer to `short`/`char` (modular on x86-64).
fn to_i16(v: i64) -> i16 {
    v as i16
}

fn write_nifti1(h: &NiftiHeader, analyze: bool) -> Vec<u8> {
    let mut b = vec![0u8; 348];
    let put =
        |b: &mut Vec<u8>, at: usize, bytes: &[u8]| b[at..at + bytes.len()].copy_from_slice(bytes);
    put(&mut b, 0, &h.sizeof_hdr.to_le_bytes());
    for k in 0..8 {
        put(&mut b, 40 + 2 * k, &to_i16(h.dim[k]).to_le_bytes());
        put(&mut b, 76 + 4 * k, &(h.pixdim[k] as f32).to_le_bytes());
    }
    put(&mut b, 70, &h.datatype.to_le_bytes());
    put(&mut b, 72, &(h.bits_per_voxel as i16).to_le_bytes());
    put(&mut b, 108, &(h.vox_offset as f32).to_le_bytes());
    put(&mut b, 124, &(h.cal_max as f32).to_le_bytes());
    put(&mut b, 128, &(h.cal_min as f32).to_le_bytes());
    strncpy(&mut b[148..228], &h.description);
    strncpy(&mut b[228..252], &h.aux_file);
    let l = &h.legacy;
    put(&mut b, 4, &l.data_type);
    put(&mut b, 14, &l.db_name);
    put(&mut b, 32, &l.extents.to_le_bytes());
    put(&mut b, 36, &l.session_error.to_le_bytes());
    b[38] = l.regular;
    put(&mut b, 140, &l.glmax.to_le_bytes());
    put(&mut b, 144, &l.glmin.to_le_bytes());
    if analyze {
        b[39] = l.hkey_un0;
        put(&mut b, 56, &l.vox_units);
        put(&mut b, 60, &l.cal_units);
        put(&mut b, 68, &l.unused1.to_le_bytes());
        put(&mut b, 74, &l.dim_un0.to_le_bytes());
        put(&mut b, 112, &l.funused1.to_le_bytes());
        put(&mut b, 116, &l.funused2.to_le_bytes());
        put(&mut b, 120, &l.funused3.to_le_bytes());
        put(&mut b, 132, &l.compressed.to_le_bytes());
        put(&mut b, 136, &l.verified.to_le_bytes());
        b[252] = l.orient;
        put(&mut b, 253, &l.originator);
        put(&mut b, 263, &l.generated);
        put(&mut b, 273, &l.scannum);
        put(&mut b, 283, &l.patient_id);
        put(&mut b, 293, &l.exp_date);
        put(&mut b, 303, &l.exp_time);
        put(&mut b, 313, &l.hist_un0);
        put(&mut b, 316, &l.views.to_le_bytes());
        put(&mut b, 320, &l.vols_added.to_le_bytes());
        put(&mut b, 324, &l.start_field.to_le_bytes());
        put(&mut b, 328, &l.field_skip.to_le_bytes());
        put(&mut b, 332, &l.omax.to_le_bytes());
        put(&mut b, 336, &l.omin.to_le_bytes());
        put(&mut b, 340, &l.smax.to_le_bytes());
        put(&mut b, 344, &l.smin.to_le_bytes());
        return b;
    }
    let m = h.magic.len().min(4);
    put(&mut b, 344, &h.magic[..m]);
    put(&mut b, 56, &(h.intent_p1 as f32).to_le_bytes());
    put(&mut b, 60, &(h.intent_p2 as f32).to_le_bytes());
    put(&mut b, 64, &(h.intent_p3 as f32).to_le_bytes());
    strncpy(&mut b[328..344], &h.intent_name);
    put(&mut b, 68, &(h.intent_code as i16).to_le_bytes());
    b[39] = h.slice_ordering;
    put(&mut b, 74, &to_i16(h.slice_start).to_le_bytes());
    put(&mut b, 120, &to_i16(h.slice_end).to_le_bytes());
    b[122] = h.slice_code as u8;
    put(&mut b, 132, &(h.slice_duration as f32).to_le_bytes());
    put(&mut b, 112, &(h.scl_slope as f32).to_le_bytes());
    put(&mut b, 116, &(h.scl_inter as f32).to_le_bytes());
    for j in 0..4 {
        put(&mut b, 280 + 4 * j, &(h.sx[j] as f32).to_le_bytes());
        put(&mut b, 296 + 4 * j, &(h.sy[j] as f32).to_le_bytes());
        put(&mut b, 312 + 4 * j, &(h.sz[j] as f32).to_le_bytes());
    }
    put(&mut b, 256, &(h.qb as f32).to_le_bytes());
    put(&mut b, 260, &(h.qc as f32).to_le_bytes());
    put(&mut b, 264, &(h.qd as f32).to_le_bytes());
    put(&mut b, 268, &(h.qx as f32).to_le_bytes());
    put(&mut b, 272, &(h.qy as f32).to_le_bytes());
    put(&mut b, 276, &(h.qz as f32).to_le_bytes());
    put(&mut b, 254, &(h.sform_code as i16).to_le_bytes());
    put(&mut b, 252, &(h.qform_code as i16).to_le_bytes());
    b[123] = h.units as u8;
    put(&mut b, 136, &(h.toffset as f32).to_le_bytes());
    b
}

fn write_nifti2(h: &NiftiHeader) -> Vec<u8> {
    let mut b = vec![0u8; 540];
    let put =
        |b: &mut Vec<u8>, at: usize, bytes: &[u8]| b[at..at + bytes.len()].copy_from_slice(bytes);
    put(&mut b, 0, &h.sizeof_hdr.to_le_bytes());
    for k in 0..8 {
        put(&mut b, 16 + 8 * k, &h.dim[k].to_le_bytes());
        put(&mut b, 104 + 8 * k, &h.pixdim[k].to_le_bytes());
    }
    put(&mut b, 12, &h.datatype.to_le_bytes());
    put(&mut b, 14, &(h.bits_per_voxel as i16).to_le_bytes());
    put(&mut b, 168, &h.vox_offset.to_le_bytes());
    put(&mut b, 192, &h.cal_max.to_le_bytes());
    put(&mut b, 200, &h.cal_min.to_le_bytes());
    strncpy(&mut b[240..320], &h.description);
    strncpy(&mut b[320..344], &h.aux_file);
    let m = h.magic.len().min(8);
    put(&mut b, 4, &h.magic[..m]);
    put(&mut b, 80, &h.intent_p1.to_le_bytes());
    put(&mut b, 88, &h.intent_p2.to_le_bytes());
    put(&mut b, 96, &h.intent_p3.to_le_bytes());
    strncpy(&mut b[508..524], &h.intent_name);
    put(&mut b, 504, &h.intent_code.to_le_bytes());
    b[524] = h.slice_ordering;
    put(&mut b, 224, &h.slice_start.to_le_bytes());
    put(&mut b, 232, &h.slice_end.to_le_bytes());
    put(&mut b, 496, &h.slice_code.to_le_bytes());
    put(&mut b, 208, &h.slice_duration.to_le_bytes());
    put(&mut b, 176, &h.scl_slope.to_le_bytes());
    put(&mut b, 184, &h.scl_inter.to_le_bytes());
    for j in 0..4 {
        put(&mut b, 400 + 8 * j, &h.sx[j].to_le_bytes());
        put(&mut b, 432 + 8 * j, &h.sy[j].to_le_bytes());
        put(&mut b, 464 + 8 * j, &h.sz[j].to_le_bytes());
    }
    put(&mut b, 352, &h.qb.to_le_bytes());
    put(&mut b, 360, &h.qc.to_le_bytes());
    put(&mut b, 368, &h.qd.to_le_bytes());
    put(&mut b, 376, &h.qx.to_le_bytes());
    put(&mut b, 384, &h.qy.to_le_bytes());
    put(&mut b, 392, &h.qz.to_le_bytes());
    put(&mut b, 348, &h.sform_code.to_le_bytes());
    put(&mut b, 344, &h.qform_code.to_le_bytes());
    put(&mut b, 500, &h.units.to_le_bytes());
    put(&mut b, 216, &h.toffset.to_le_bytes());
    b
}

// ------------------------------------------------------------------------------------------------
// Files (znzlib)

/// The whole content of `path`, decompressed if it is gzip (`znzopen` with compression reads
/// plain files as they are), or NewNifti's message when it cannot be opened.
fn read_file(path: &str) -> Result<Vec<u8>, NiftiError> {
    let raw = std::fs::read(path).map_err(|_| err(format!("Error: cant open file {path}")))?;
    if raw.len() >= 2 && raw[0] == 0x1f && raw[1] == 0x8b {
        let mut out = Vec::new();
        let mut d = flate2::read::MultiGzDecoder::new(&raw[..]);
        // A truncated or damaged stream reads as far as it goes (gzread's behaviour).
        let _ = d.read_to_end(&mut out);
        Ok(out)
    } else {
        Ok(raw)
    }
}

/// The header of `path` (`loadHeader`).
pub fn load_header(path: &str) -> Result<NiftiHeader, NiftiError> {
    let content = read_file(path)?;
    parse_header(&content)
}

fn parse_header(content: &[u8]) -> Result<NiftiHeader, NiftiError> {
    if content.len() < 348 {
        return Err(err("Error: short read, file may be truncated"));
    }
    let size = i32::from_le_bytes([content[0], content[1], content[2], content[3]]);
    let swap = size == 1543569408 || size == 469893120;
    let v2 = nifti2_version(size);
    let v1 = nifti_version(&content[344..348]);
    // headerType: 0 (other) when NIFTI2_VERSION is 0; the NIfTI-1 magic's version when it is 1
    // (0 for ANALYZE); 2 for NIfTI-2.
    let kind = if v2 == 0 {
        -1
    } else if v2 == 1 {
        v1
    } else {
        v2
    };
    match kind {
        0 => {
            let mut h = read_nifti1_or_analyze(&content[..348], swap, true);
            h.was_wrong_endian = swap;
            Ok(h)
        }
        1 => {
            let mut h = read_nifti1_or_analyze(&content[..348], swap, false);
            h.was_wrong_endian = swap;
            Ok(h)
        }
        2 => {
            if content.len() < 540 {
                return Err(err("Error: short read, file may be truncated"));
            }
            let mut h = read_nifti2(&content[..540], swap);
            h.was_wrong_endian = swap;
            Ok(h)
        }
        _ => Err(err(
            "Error: file does not appear to be a valid NIFTI or ANALYZE image",
        )),
    }
}

/// The extensions after the header (`readExtensions`).
fn read_extensions(content: &[u8], h: &NiftiHeader) -> Result<Vec<Extension>, NiftiError> {
    let mut out = Vec::new();
    let mut pos = h.sizeof_hdr as usize;
    if content.len() < pos + 4 {
        return Ok(out); // short read: a pair without extension bytes
    }
    if content[pos] == 0 {
        return Ok(out);
    }
    pos += 4;
    let mut bytes_read = h.sizeof_hdr as usize + 4;
    loop {
        if h.single_file() && bytes_read + 8 > h.vox_offset as usize {
            return Ok(out);
        }
        if content.len() < pos + 8 {
            if h.single_file() {
                return Err(err("Error: short read, file may be truncated"));
            }
            return Ok(out);
        }
        let mut a = [
            content[pos],
            content[pos + 1],
            content[pos + 2],
            content[pos + 3],
        ];
        let mut c = [
            content[pos + 4],
            content[pos + 5],
            content[pos + 6],
            content[pos + 7],
        ];
        if h.was_wrong_endian {
            a.reverse();
            c.reverse();
        }
        let esize = i32::from_le_bytes(a);
        let ecode = i32::from_le_bytes(c);
        bytes_read += 8;
        pos += 8;
        if h.single_file() && (bytes_read as i64 + i64::from(esize) - 8) > h.vox_offset {
            return Ok(out);
        }
        if esize == 0 || esize % 16 != 0 {
            return Err(err("Error: esize must be positive multiple of 16"));
        }
        let n = (esize - 8) as usize;
        if content.len() < pos + n {
            if h.single_file() {
                return Err(err("Error: short read, file may be truncated"));
            }
            return Ok(out);
        }
        out.push(Extension {
            esize,
            ecode,
            edata: content[pos..pos + n].to_vec(),
        });
        pos += n;
        bytes_read += n;
        if h.single_file() && bytes_read as i64 == h.vox_offset {
            return Ok(out);
        }
    }
}

/// An image read from disk: its header (dimensions beyond `dim[0]` set to 1), extensions, and
/// data in the machine's byte order.
pub struct Loaded {
    pub header: NiftiHeader,
    pub extensions: Vec<Extension>,
    pub data: Vec<u8>,
}

/// `loadImageROI` with the whole image.
pub fn load_image(path: &str) -> Result<Loaded, NiftiError> {
    let content = read_file(path)?;
    let mut h = parse_header(&content)?;
    let d0 = h.dim[0];
    if !(0..=7).contains(&d0) {
        // NewNifti fills dim[dim[0]+1..8] with an iterator past the array: undefined.
        return Err(err(format!(
            "Error: dim[0] = {d0} is outside 0..7 in {path} (larmorx)"
        )));
    }
    for k in (d0 as usize + 1)..8 {
        h.dim[k] = 1;
    }
    let extensions = read_extensions(&content, &h)?;
    for k in 1..8 {
        if h.dim[k] - 1 < 0 {
            return Err(err(format!("Error: Nonsensical ROI for {path}")));
        }
    }
    let width = h.datum_byte_width();
    let n = h.n_elements();
    let nbytes = n
        .checked_mul(width)
        .ok_or_else(|| err("Error: short read, file may be truncated"))?;
    let (src, offset): (std::borrow::Cow<[u8]>, usize) = if h.single_file() {
        (
            std::borrow::Cow::Borrowed(&content[..]),
            h.nominal_vox_offset() as usize,
        )
    } else {
        // The data file: .img for .hdr (compressed or not, read transparently).
        let img = match path.rfind(".hdr") {
            Some(i) => format!("{}.img{}", &path[..i], &path[i + 4..]),
            // NewNifti replaces ".hdr" at string::npos: std::out_of_range, caught as an
            // exception by the reader.
            None => {
                return Err(err(format!(
                    "basic_string::replace: __pos (which is 18446744073709551615) > this->size() (which is {})",
                    path.len()
                )));
            }
        };
        (
            std::borrow::Cow::Owned(read_file(&img)?),
            h.nominal_vox_offset() as usize,
        )
    };
    if src.len() < offset || src.len() - offset < nbytes {
        return Err(err("Error: short read, file may be truncated"));
    }
    let mut data = src[offset..offset + nbytes].to_vec();
    if h.was_wrong_endian && width > 1 {
        for chunk in data.chunks_exact_mut(width) {
            chunk.reverse();
        }
    }
    Ok(Loaded {
        header: h,
        extensions,
        data,
    })
}

/// `saveImage`: the header (with `vox_offset` set), its extensions and the data, to `path`
/// (for a pair, the header to `.hdr` and the data to `.img`), gzip-compressed when asked.
pub fn save_image(
    path: &str,
    data: &[u8],
    extensions: &[Extension],
    header: &NiftiHeader,
    compress: bool,
    reproduce_offset: bool,
) -> Result<(), NiftiError> {
    let mut h = header.clone();
    h.vox_offset = i64::from(h.sizeof_hdr);
    let mut size_of_extensions = 4usize;
    for (i, e) in extensions.iter().enumerate() {
        // NewNifti adds the first extension's size once per extension (FSL-56); the correction
        // adds each one's.
        let _ = i;
        size_of_extensions += if reproduce_offset {
            extensions[0].size()
        } else {
            e.size()
        };
    }
    h.vox_offset += size_of_extensions as i64;
    if !h.single_file() {
        h.vox_offset = 0;
    }
    let raw = if h.is_analyze() {
        write_nifti1(&h, true)
    } else if h.nifti_version() == 1 {
        write_nifti1(&h, false)
    } else {
        write_nifti2(&h)
    };
    let mut first = raw;
    if !h.is_analyze() {
        first.extend_from_slice(&[u8::from(!extensions.is_empty()), 0, 0, 0]);
        for e in extensions {
            if e.esize % 16 != 0 {
                return Err(err("Error: extension size must be multiple of 16 bytes"));
            }
            first.extend_from_slice(&e.esize.to_le_bytes());
            first.extend_from_slice(&e.ecode.to_le_bytes());
            first.extend_from_slice(&e.edata);
        }
    }
    if h.single_file() {
        first.extend_from_slice(data);
        write_file(path, &first, compress)
    } else {
        write_file(path, &first, compress)?;
        let img = match path.rfind(".hdr") {
            Some(i) => format!("{}.img{}", &path[..i], &path[i + 4..]),
            None => path.to_owned(),
        };
        write_file(&img, data, compress)
    }
}

/// Writes `bytes` to `path` (gzip with zlib's default level when `compress`), or NewNifti's
/// message when the file cannot be created.
fn write_file(path: &str, bytes: &[u8], compress: bool) -> Result<(), NiftiError> {
    let f = std::fs::File::create(Path::new(path))
        .map_err(|_| err(format!("Error: cant open file {path}")))?;
    let mut w = std::io::BufWriter::new(f);
    let res = if compress {
        let mut enc = flate2::GzBuilder::new()
            .operating_system(3)
            .write(&mut w, flate2::Compression::new(6));
        enc.write_all(bytes).and_then(|_| enc.finish().map(|_| ()))
    } else {
        w.write_all(bytes)
    };
    res.and_then(|_| w.flush())
        .map_err(|_| err("Error: short write, output file will be truncated"))
}

// ------------------------------------------------------------------------------------------------
// Legacy routines (nifti1_io's, in NewNifti's versions)

pub mod legacy {
    use super::x87;

    pub type Mat44 = [[f32; 4]; 4];
    pub type Mat33 = [[f32; 3]; 3];

    /// The quaternion form of a matrix, as `nifti_mat44_to_quatern` returns it.
    #[derive(Clone, Copy, Debug)]
    pub struct Quatern {
        pub qb: f64,
        pub qc: f64,
        pub qd: f64,
        pub qx: f64,
        pub qy: f64,
        pub qz: f64,
        pub dx: f64,
        pub dy: f64,
        pub dz: f64,
        pub qfac: f64,
    }

    /// `nifti_mat33_determ` (computed in double, returned as float).
    pub fn mat33_determ(r: &Mat33) -> f32 {
        let (r11, r12, r13) = (f64::from(r[0][0]), f64::from(r[0][1]), f64::from(r[0][2]));
        let (r21, r22, r23) = (f64::from(r[1][0]), f64::from(r[1][1]), f64::from(r[1][2]));
        let (r31, r32, r33) = (f64::from(r[2][0]), f64::from(r[2][1]), f64::from(r[2][2]));
        (r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33 + r21 * r32 * r13 + r31 * r12 * r23
            - r31 * r22 * r13) as f32
    }

    fn det_f64(r: &Mat33) -> f64 {
        let (r11, r12, r13) = (f64::from(r[0][0]), f64::from(r[0][1]), f64::from(r[0][2]));
        let (r21, r22, r23) = (f64::from(r[1][0]), f64::from(r[1][1]), f64::from(r[1][2]));
        let (r31, r32, r33) = (f64::from(r[2][0]), f64::from(r[2][1]), f64::from(r[2][2]));
        r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33 + r21 * r32 * r13 + r31 * r12 * r23
            - r31 * r22 * r13
    }

    /// `nifti_mat33_inverse`.
    pub fn mat33_inverse(r: &Mat33) -> Mat33 {
        let g = |i: usize, j: usize| f64::from(r[i][j]);
        let (r11, r12, r13) = (g(0, 0), g(0, 1), g(0, 2));
        let (r21, r22, r23) = (g(1, 0), g(1, 1), g(1, 2));
        let (r31, r32, r33) = (g(2, 0), g(2, 1), g(2, 2));
        let mut deti = det_f64(r);
        if deti != 0.0 {
            deti = x87::div_to_f64(1.0, deti);
        }
        let mut q = [[0f32; 3]; 3];
        q[0][0] = (deti * (r22 * r33 - r32 * r23)) as f32;
        q[0][1] = (deti * (-r12 * r33 + r32 * r13)) as f32;
        q[0][2] = (deti * (r12 * r23 - r22 * r13)) as f32;
        q[1][0] = (deti * (-r21 * r33 + r31 * r23)) as f32;
        q[1][1] = (deti * (r11 * r33 - r31 * r13)) as f32;
        q[1][2] = (deti * (-r11 * r23 + r21 * r13)) as f32;
        q[2][0] = (deti * (r21 * r32 - r31 * r22)) as f32;
        q[2][1] = (deti * (-r11 * r32 + r31 * r12)) as f32;
        q[2][2] = (deti * (r11 * r22 - r21 * r12)) as f32;
        q
    }

    /// `nifti_mat44_inverse`.
    pub fn mat44_inverse(r: &Mat44) -> Mat44 {
        let g = |i: usize, j: usize| f64::from(r[i][j]);
        let (r11, r12, r13) = (g(0, 0), g(0, 1), g(0, 2));
        let (r21, r22, r23) = (g(1, 0), g(1, 1), g(1, 2));
        let (r31, r32, r33) = (g(2, 0), g(2, 1), g(2, 2));
        let (v1, v2, v3) = (g(0, 3), g(1, 3), g(2, 3));
        let mut deti =
            r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33 + r21 * r32 * r13 + r31 * r12 * r23
                - r31 * r22 * r13;
        if deti != 0.0 {
            deti = x87::div_to_f64(1.0, deti);
        }
        let mut q = [[0f32; 4]; 4];
        q[0][0] = (deti * (r22 * r33 - r32 * r23)) as f32;
        q[0][1] = (deti * (-r12 * r33 + r32 * r13)) as f32;
        q[0][2] = (deti * (r12 * r23 - r22 * r13)) as f32;
        q[0][3] = (deti
            * (-r12 * r23 * v3 + r12 * v2 * r33 + r22 * r13 * v3 - r22 * v1 * r33 - r32 * r13 * v2
                + r32 * v1 * r23)) as f32;
        q[1][0] = (deti * (-r21 * r33 + r31 * r23)) as f32;
        q[1][1] = (deti * (r11 * r33 - r31 * r13)) as f32;
        q[1][2] = (deti * (-r11 * r23 + r21 * r13)) as f32;
        q[1][3] = (deti
            * (r11 * r23 * v3 - r11 * v2 * r33 - r21 * r13 * v3 + r21 * v1 * r33 + r31 * r13 * v2
                - r31 * v1 * r23)) as f32;
        q[2][0] = (deti * (r21 * r32 - r31 * r22)) as f32;
        q[2][1] = (deti * (-r11 * r32 + r31 * r12)) as f32;
        q[2][2] = (deti * (r11 * r22 - r21 * r12)) as f32;
        q[2][3] = (deti
            * (-r11 * r22 * v3 + r11 * r32 * v2 + r21 * r12 * v3 - r21 * r32 * v1 - r31 * r12 * v2
                + r31 * r22 * v1)) as f32;
        q[3][3] = if deti == 0.0 { 0.0 } else { 1.0 };
        q
    }

    fn rownorm(a: &Mat33) -> f32 {
        let mut r1 = a[0][0].abs() + a[0][1].abs() + a[0][2].abs();
        let r2 = a[1][0].abs() + a[1][1].abs() + a[1][2].abs();
        let r3 = a[2][0].abs() + a[2][1].abs() + a[2][2].abs();
        if r1 < r2 {
            r1 = r2;
        }
        if r1 < r3 {
            r1 = r3;
        }
        r1
    }

    fn colnorm(a: &Mat33) -> f32 {
        let mut r1 = a[0][0].abs() + a[1][0].abs() + a[2][0].abs();
        let r2 = a[0][1].abs() + a[1][1].abs() + a[2][1].abs();
        let r3 = a[0][2].abs() + a[1][2].abs() + a[2][2].abs();
        if r1 < r2 {
            r1 = r2;
        }
        if r1 < r3 {
            r1 = r3;
        }
        r1
    }

    /// `nifti_mat33_polar`: the orthogonal matrix closest to `a`.
    pub fn mat33_polar(a: &Mat33) -> Mat33 {
        let mut x = *a;
        let mut dif: f32 = 1.0;
        let mut k = 0;
        let mut gam = mat33_determ(&x);
        while gam == 0.0 {
            gam = (0.00001 * (0.001 + f64::from(rownorm(&x)))) as f32;
            x[0][0] += gam;
            x[1][1] += gam;
            x[2][2] += gam;
            gam = mat33_determ(&x);
        }
        let mut z;
        loop {
            let y = mat33_inverse(&x);
            let (gm, gi);
            if dif > 0.3 {
                let alp = (rownorm(&x) * colnorm(&x)).sqrt();
                let bet = (rownorm(&y) * colnorm(&y)).sqrt();
                gm = (bet / alp).sqrt();
                gi = (1.0 / f64::from(gm)) as f32;
            } else {
                gm = 1.0;
                gi = 1.0;
            }
            z = [[0f32; 3]; 3];
            let h = |a: f32, b: f32| (0.5 * f64::from(a + b)) as f32;
            z[0][0] = h(gm * x[0][0], gi * y[0][0]);
            z[0][1] = h(gm * x[0][1], gi * y[1][0]);
            z[0][2] = h(gm * x[0][2], gi * y[2][0]);
            z[1][0] = h(gm * x[1][0], gi * y[0][1]);
            z[1][1] = h(gm * x[1][1], gi * y[1][1]);
            z[1][2] = h(gm * x[1][2], gi * y[2][1]);
            z[2][0] = h(gm * x[2][0], gi * y[0][2]);
            z[2][1] = h(gm * x[2][1], gi * y[1][2]);
            z[2][2] = h(gm * x[2][2], gi * y[2][2]);
            dif = (z[0][0] - x[0][0]).abs()
                + (z[0][1] - x[0][1]).abs()
                + (z[0][2] - x[0][2]).abs()
                + (z[1][0] - x[1][0]).abs()
                + (z[1][1] - x[1][1]).abs()
                + (z[1][2] - x[1][2]).abs()
                + (z[2][0] - x[2][0]).abs()
                + (z[2][1] - x[2][1]).abs()
                + (z[2][2] - x[2][2]).abs();
            k += 1;
            if k > 100 || f64::from(dif) < 3.0e-6 {
                break;
            }
            x = z;
        }
        z
    }

    /// `nifti_quatern_to_mat44`.
    #[allow(clippy::too_many_arguments)]
    pub fn quatern_to_mat44(
        qb: f32,
        qc: f32,
        qd: f32,
        qx: f32,
        qy: f32,
        qz: f32,
        dx: f32,
        dy: f32,
        dz: f32,
        qfac: f32,
    ) -> Mat44 {
        let mut r = [[0f32; 4]; 4];
        let (mut b, mut c, mut d) = (f64::from(qb), f64::from(qc), f64::from(qd));
        r[3][3] = 1.0;
        // a = 1.0l - (b*b + c*c + d*d), in long double, stored in a double.
        let mut a = x87::sub_to_f64(1.0, b * b + c * c + d * d);
        // `a < 1.e-7l`: the long double nearest 1e-7 is above the double nearest it, and no
        // double lies between them, so the test is `a <= 1e-7` in double.
        if a <= 1.0e-7 {
            a = x87::div_to_f64(1.0, (b * b + c * c + d * d).sqrt());
            b *= a;
            c *= a;
            d *= a;
            a = 0.0;
        } else {
            a = a.sqrt();
        }
        let xd = if dx > 0.0 { f64::from(dx) } else { 1.0 };
        let yd = if dy > 0.0 { f64::from(dy) } else { 1.0 };
        let mut zd = if dz > 0.0 { f64::from(dz) } else { 1.0 };
        if qfac < 0.0 {
            zd = -zd;
        }
        r[0][0] = ((a * a + b * b - c * c - d * d) * xd) as f32;
        r[0][1] = x87::mul_to_f32(2.0 * (b * c - a * d), yd);
        r[0][2] = x87::mul_to_f32(2.0 * (b * d + a * c), zd);
        r[1][0] = x87::mul_to_f32(2.0 * (b * c + a * d), xd);
        r[1][1] = ((a * a + c * c - b * b - d * d) * yd) as f32;
        r[1][2] = x87::mul_to_f32(2.0 * (c * d - a * b), zd);
        r[2][0] = x87::mul_to_f32(2.0 * (b * d - a * c), xd);
        r[2][1] = x87::mul_to_f32(2.0 * (c * d + a * b), yd);
        r[2][2] = ((a * a + d * d - c * c - b * b) * zd) as f32;
        r[0][3] = qx;
        r[1][3] = qy;
        r[2][3] = qz;
        r
    }

    /// `nifti_mat44_to_quatern`.
    pub fn mat44_to_quatern(r: &Mat44) -> Quatern {
        let qx = f64::from(r[0][3]);
        let qy = f64::from(r[1][3]);
        let qz = f64::from(r[2][3]);
        let g = |i: usize, j: usize| f64::from(r[i][j]);
        let (mut r11, mut r12, mut r13) = (g(0, 0), g(0, 1), g(0, 2));
        let (mut r21, mut r22, mut r23) = (g(1, 0), g(1, 1), g(1, 2));
        let (mut r31, mut r32, mut r33) = (g(2, 0), g(2, 1), g(2, 2));
        let mut xd = (r11 * r11 + r21 * r21 + r31 * r31).sqrt();
        let mut yd = (r12 * r12 + r22 * r22 + r32 * r32).sqrt();
        let mut zd = (r13 * r13 + r23 * r23 + r33 * r33).sqrt();
        if xd == 0.0 {
            r11 = 1.0;
            r21 = 0.0;
            r31 = 0.0;
            xd = 1.0;
        }
        if yd == 0.0 {
            r22 = 1.0;
            r12 = 0.0;
            r32 = 0.0;
            yd = 1.0;
        }
        if zd == 0.0 {
            r33 = 1.0;
            r13 = 0.0;
            r23 = 0.0;
            zd = 1.0;
        }
        let (dx, dy, dz) = (xd, yd, zd);
        r11 /= xd;
        r21 /= xd;
        r31 /= xd;
        r12 /= yd;
        r22 /= yd;
        r32 /= yd;
        r13 /= zd;
        r23 /= zd;
        r33 /= zd;
        let q: Mat33 = [
            [r11 as f32, r12 as f32, r13 as f32],
            [r21 as f32, r22 as f32, r23 as f32],
            [r31 as f32, r32 as f32, r33 as f32],
        ];
        let p = mat33_polar(&q);
        let pg = |i: usize, j: usize| f64::from(p[i][j]);
        r11 = pg(0, 0);
        r12 = pg(0, 1);
        r13 = pg(0, 2);
        r21 = pg(1, 0);
        r22 = pg(1, 1);
        r23 = pg(1, 2);
        r31 = pg(2, 0);
        r32 = pg(2, 1);
        r33 = pg(2, 2);
        let zdet =
            r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33 + r21 * r32 * r13 + r31 * r12 * r23
                - r31 * r22 * r13;
        let qfac;
        if zdet > 0.0 {
            qfac = 1.0;
        } else {
            qfac = -1.0;
            r13 = -r13;
            r23 = -r23;
            r33 = -r33;
        }
        // a = r11 + r22 + r33 + 1.0l: the double sum, then the long double addition.
        let mut a = x87::add_to_f64(r11 + r22 + r33, 1.0);
        let (mut b, mut c, mut d);
        if a > 0.5 {
            a = 0.5 * a.sqrt();
            b = x87::div_to_f64(0.25 * (r32 - r23), a);
            c = x87::div_to_f64(0.25 * (r13 - r31), a);
            d = x87::div_to_f64(0.25 * (r21 - r12), a);
        } else {
            let xd = 1.0 + r11 - (r22 + r33);
            let yd = 1.0 + r22 - (r11 + r33);
            let zd = 1.0 + r33 - (r11 + r22);
            if xd > 1.0 {
                b = 0.5 * xd.sqrt();
                c = x87::div_to_f64(0.25 * (r12 + r21), b);
                d = x87::div_to_f64(0.25 * (r13 + r31), b);
                a = x87::div_to_f64(0.25 * (r32 - r23), b);
            } else if yd > 1.0 {
                c = 0.5 * yd.sqrt();
                b = x87::div_to_f64(0.25 * (r12 + r21), c);
                d = x87::div_to_f64(0.25 * (r23 + r32), c);
                a = x87::div_to_f64(0.25 * (r13 - r31), c);
            } else {
                d = 0.5 * zd.sqrt();
                b = x87::div_to_f64(0.25 * (r13 + r31), d);
                c = x87::div_to_f64(0.25 * (r23 + r32), d);
                a = x87::div_to_f64(0.25 * (r21 - r12), d);
            }
            if a < 0.0 {
                b = -b;
                c = -c;
                d = -d;
            }
        }
        let _ = a;
        Quatern {
            qb: b,
            qc: c,
            qd: d,
            qx,
            qy,
            qz,
            dx,
            dy,
            dz,
            qfac,
        }
    }

    /// `mat44_to_mat33`.
    pub fn mat44_to_mat33(x: &Mat44) -> Mat33 {
        let mut y = [[0f32; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                y[i][j] = x[i][j];
            }
        }
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quaternion_round_trip_of_an_axial_las_grid() {
        // ds000005's boldref: LAS, 3.125 x 3.125 x 4 mm.
        let m: legacy::Mat44 = [
            [-3.125, 0.0, 0.0, 101.0],
            [0.0, 3.125, 0.0, -72.0],
            [0.0, 0.0, 4.0, -99.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let q = legacy::mat44_to_quatern(&m);
        assert_eq!((q.qb, q.qc, q.qd, q.qfac), (0.0, 1.0, 0.0, -1.0));
        let back =
            legacy::quatern_to_mat44(0.0, 1.0, 0.0, 101.0, -72.0, -99.0, 3.125, 3.125, 4.0, -1.0);
        assert_eq!(back, m);
    }

    #[test]
    fn header_bytes_round_trip() {
        let mut h = NiftiHeader::default();
        h.set_nifti_version(1, true);
        h.dim = [3, 4, 5, 6, 1, 1, 1, 1];
        h.pixdim = [1.0, 2.0, 2.0, 3.0, 0.0, 0.0, 0.0, 0.0];
        h.datatype = dt::FLOAT;
        h.bits_per_voxel = 32;
        h.vox_offset = 352;
        let raw = write_nifti1(&h, false);
        let mut content = raw.clone();
        content.extend_from_slice(&[0; 4]);
        let back = parse_header(&content).unwrap();
        assert_eq!(back.dim, h.dim);
        assert_eq!(back.pixdim, h.pixdim);
        assert!(back.single_file() && !back.is_analyze());
        assert_eq!(write_nifti1(&back, false), raw);
    }
}
