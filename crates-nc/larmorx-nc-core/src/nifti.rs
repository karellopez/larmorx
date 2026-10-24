// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.17: fsl-newnifti 4.1.0 (NewNifti.{h,cc}, legacyFunctions.
// {h,cc}, nifti1.h, nifti2.h) and fsl-znzlib 2111.0 (znzlib.c). Copyright (C) 2018 University of
// Oxford (Matthew Webster); legacyFunctions.cc includes code from nifti1_io.c released to the
// public domain by Robert W Cox (2003), modified by Mark Jenkinson (2004), Rick Reynolds (2004)
// and Matthew Webster (2018). Rust translation Copyright 2026 Karel Lopez Vilaret; modified from
// the originals. Distributed under the FSL Licence (see LICENSE).
//! NIfTI-1, NIfTI-2 and ANALYZE 7.5 files as FSL's NIfTI library reads and writes them.
//!
//! A [`NiftiHeader`] holds the fields of FSL's `NiftiIO::NiftiHeader` (doubles for every
//! floating field, 64-bit dimensions, the strings as FSL takes them from the raw header), so
//! that a file read and written again gives FSL's bytes. [`load_image`] reads a header, its
//! extensions and its data (gzip-compressed or not, whatever the name says: FSL opens every
//! file through `gzopen`); [`save_image`] writes one. The quaternion conversions
//! ([`quatern_to_mat44`], [`mat44_to_quatern`]) reproduce the x87 extended precision of FSL's
//! x86-64 build ([`crate::x87`]).
//!
//! Errors carry FSL's messages (`NiftiIO::NiftiException::what()`).

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use flate2::Compression;
use flate2::read::MultiGzDecoder;
use flate2::write::GzEncoder;

use crate::x87;

/// NIfTI data type codes (`nifti1.h`).
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

/// `NIFTI_XFORM_UNKNOWN`.
pub const XFORM_UNKNOWN: i32 = 0;
/// `NIFTI_XFORM_ALIGNED_ANAT`.
pub const XFORM_ALIGNED_ANAT: i32 = 2;
/// `NIFTI_UNITS_MM | NIFTI_UNITS_SEC`.
pub const UNITS_MM_SEC: i32 = 10;

/// An error with FSL's message (`NiftiException::what()`).
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

/// FSL's `mat44` and `mat33`: `float` elements.
pub type Mat44 = [[f32; 4]; 4];
pub type Mat33 = [[f32; 3]; 3];

/// The ANALYZE-only and NIfTI-1 legacy fields (`NiftiIO::LegacyFields`).
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
    /// `LegacyFields::LegacyFields()`: `extents` 16384, `regular` `'r'`, everything else 0.
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
    /// `origin()`: the first three shorts of `originator` (native byte order).
    pub fn origin(&self) -> [i16; 5] {
        std::array::from_fn(|i| i16::from_le_bytes([self.originator[2 * i], self.originator[2 * i + 1]]))
    }

    pub fn set_origin(&mut self, i: usize, v: i16) {
        let b = v.to_le_bytes();
        self.originator[2 * i] = b[0];
        self.originator[2 * i + 1] = b[1];
    }
}

/// One header extension (`NiftiIO::NiftiExtension`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NiftiExtension {
    pub esize: i32,
    pub ecode: i32,
    pub edata: Vec<u8>,
}

impl NiftiExtension {
    /// `extensionSize()`: 8 + the data's length.
    pub fn extension_size(&self) -> usize {
        8 + self.edata.len()
    }
}

/// FSL's `NiftiIO::NiftiHeader`.
#[derive(Clone, Debug, PartialEq)]
pub struct NiftiHeader {
    pub sizeof_hdr: i32,
    pub vox_offset: i64,
    /// The raw magic: 4 bytes (NIfTI-1, ANALYZE) or 8 (NIfTI-2).
    pub magic: Vec<u8>,
    pub datatype: i16,
    pub bits_per_voxel: i32,
    pub dim: [i64; 8],
    pub pixdim: [f64; 8],
    pub intent_code: i32,
    pub intent_p1: f64,
    pub intent_p2: f64,
    pub intent_p3: f64,
    /// The intent name as FSL's `std::string` holds it (up to the first NUL).
    pub intent_name: Vec<u8>,
    pub scl_slope: f64,
    pub scl_inter: f64,
    pub cal_max: f64,
    pub cal_min: f64,
    pub slice_duration: f64,
    pub toffset: f64,
    pub slice_start: i64,
    pub slice_end: i64,
    pub description: Vec<u8>,
    pub aux_file: Vec<u8>,
    pub qform_code: i32,
    pub sform_code: i32,
    pub q_b: f64,
    pub q_c: f64,
    pub q_d: f64,
    pub q_x: f64,
    pub q_y: f64,
    pub q_z: f64,
    pub s_x: [f64; 4],
    pub s_y: [f64; 4],
    pub s_z: [f64; 4],
    pub slice_code: i32,
    pub units: i32,
    pub slice_ordering: i8,
    pub legacy: LegacyFields,
    pub was_wrong_endian: bool,
}

impl Default for NiftiHeader {
    /// `NiftiHeader::initialise()`: zeros, `scl_slope` 1, the strings filled with NULs.
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
            q_b: 0.0,
            q_c: 0.0,
            q_d: 0.0,
            q_x: 0.0,
            q_y: 0.0,
            q_z: 0.0,
            s_x: [0.0; 4],
            s_y: [0.0; 4],
            s_z: [0.0; 4],
            slice_code: 0,
            units: 0,
            slice_ordering: 0,
            legacy: LegacyFields::default(),
            was_wrong_endian: false,
        }
    }
}

/// `NIFTI2_VERSION(h)` from `sizeof_hdr`.
fn nifti2_version(sizeof_hdr: i32) -> i32 {
    match sizeof_hdr {
        348 | 1_543_569_408 => 1,
        540 | 469_893_120 => 2,
        _ => 0,
    }
}

/// `NIFTI_VERSION(h)` from the magic.
fn nifti_version(magic: &[u8]) -> i32 {
    let m = |i: usize| magic.get(i).copied().unwrap_or(0);
    if m(0) == b'n' && m(3) == 0 && (m(1) == b'i' || m(1) == b'+') && (b'1'..=b'9').contains(&m(2))
    {
        i32::from(m(2) - b'0')
    } else {
        0
    }
}

impl NiftiHeader {
    /// `niftiVersion()`.
    pub fn nifti_version(&self) -> i32 {
        nifti2_version(self.sizeof_hdr)
    }

    /// `isAnalyze()`.
    pub fn is_analyze(&self) -> bool {
        nifti2_version(self.sizeof_hdr) == 1 && nifti_version(&self.magic) == 0
    }

    /// `singleFile()` (`NIFTI_ONEFILE`: the magic's second byte is `'+'`).
    pub fn single_file(&self) -> bool {
        self.magic.get(1) == Some(&b'+')
    }

    /// `nElements()`: the product of `dim[1..=dim[0]]`.
    pub fn n_elements(&self) -> usize {
        let mut e = self.dim[1].max(0) as usize;
        for d in 2..=(self.dim[0].clamp(0, 7) as usize) {
            e *= self.dim[d].max(0) as usize;
        }
        e
    }

    /// `bpvOfDatatype()`.
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

    /// `datumByteWidth()` (FSL reads without strict compliance: the datatype decides).
    pub fn datum_byte_width(&self) -> usize {
        (self.bpv_of_datatype() / 8) as usize
    }

    /// `leftHanded()`: -1 when `pixdim[0] < 0`.
    pub fn left_handed(&self) -> f32 {
        if self.pixdim[0] < 0.0 { -1.0 } else { 1.0 }
    }

    /// `nominalVoxOffset()`.
    pub fn nominal_vox_offset(&self) -> i64 {
        if self.single_file() && self.vox_offset < 352 {
            352
        } else {
            self.vox_offset
        }
    }

    /// `setNiftiVersion(version, isSingleFile)`.
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

    /// `getQForm()`.
    pub fn get_qform(&self) -> Mat44 {
        quatern_to_mat44(
            self.q_b as f32,
            self.q_c as f32,
            self.q_d as f32,
            self.q_x as f32,
            self.q_y as f32,
            self.q_z as f32,
            self.pixdim[1] as f32,
            self.pixdim[2] as f32,
            self.pixdim[3] as f32,
            self.left_handed(),
        )
    }

    /// `setQForm(qForm)`: the quaternion from the matrix; `pixdim[0]` receives `qfac`.
    pub fn set_qform(&mut self, q: &Mat44) {
        let r = mat44_to_quatern(q);
        self.q_b = r.qb;
        self.q_c = r.qc;
        self.q_d = r.qd;
        self.q_x = r.qx;
        self.q_y = r.qy;
        self.q_z = r.qz;
        self.pixdim[0] = r.qfac;
    }

    /// `getSForm()`.
    pub fn get_sform(&self) -> Mat44 {
        let mut r = [[0.0f32; 4]; 4];
        for j in 0..4 {
            r[0][j] = self.s_x[j] as f32;
            r[1][j] = self.s_y[j] as f32;
            r[2][j] = self.s_z[j] as f32;
        }
        r[3] = [0.0, 0.0, 0.0, 1.0];
        r
    }

    /// `setSForm(sForm)`.
    pub fn set_sform(&mut self, s: &Mat44) {
        for j in 0..4 {
            self.s_x[j] = f64::from(s[0][j]);
            self.s_y[j] = f64::from(s[1][j]);
            self.s_z[j] = f64::from(s[2][j]);
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Quaternions and small matrices (legacyFunctions.cc)

/// `nifti_quatern_to_mat44`, with the x87 extended precision of FSL's x86-64 build.
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
    let mut r = [[0.0f32; 4]; 4];
    let mut b = f64::from(qb);
    let mut c = f64::from(qc);
    let mut d = f64::from(qd);
    r[3] = [0.0, 0.0, 0.0, 1.0];
    // a = 1.0l - (b*b + c*c + d*d): a long double subtraction stored to a double.
    let mut a = x87::sub_to_f64(1.0, b * b + c * c + d * d);
    // `a < 1.e-7l`: the long double constant lies between the double nearest 1e-7 (below it)
    // and the next double, so for a double `a` the test is `a <= 1e-7` in double terms.
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

/// The outputs of `nifti_mat44_to_quatern`.
#[derive(Clone, Copy, Debug, PartialEq)]
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

/// `nifti_mat44_to_quatern`, with the x87 extended precision of FSL's x86-64 build.
pub fn mat44_to_quatern(rm: &Mat44) -> Quatern {
    let qx = f64::from(rm[0][3]);
    let qy = f64::from(rm[1][3]);
    let qz = f64::from(rm[2][3]);
    let mut r11 = f64::from(rm[0][0]);
    let mut r12 = f64::from(rm[0][1]);
    let mut r13 = f64::from(rm[0][2]);
    let mut r21 = f64::from(rm[1][0]);
    let mut r22 = f64::from(rm[1][1]);
    let mut r23 = f64::from(rm[1][2]);
    let mut r31 = f64::from(rm[2][0]);
    let mut r32 = f64::from(rm[2][1]);
    let mut r33 = f64::from(rm[2][2]);
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
    r11 = f64::from(p[0][0]);
    r12 = f64::from(p[0][1]);
    r13 = f64::from(p[0][2]);
    r21 = f64::from(p[1][0]);
    r22 = f64::from(p[1][1]);
    r23 = f64::from(p[1][2]);
    r31 = f64::from(p[2][0]);
    r32 = f64::from(p[2][1]);
    r33 = f64::from(p[2][2]);
    let zdet = r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33
        + r21 * r32 * r13
        + r31 * r12 * r23
        - r31 * r22 * r13;
    let qfac = if zdet > 0.0 {
        1.0
    } else {
        r13 = -r13;
        r23 = -r23;
        r33 = -r33;
        -1.0
    };
    // a = r11 + r22 + r33 + 1.0l: two double additions, then a long double one.
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

/// `nifti_mat33_determ` (a double expression returned as `float`).
pub fn mat33_determ(r: &Mat33) -> f32 {
    let g = |i: usize, j: usize| f64::from(r[i][j]);
    let (r11, r12, r13) = (g(0, 0), g(0, 1), g(0, 2));
    let (r21, r22, r23) = (g(1, 0), g(1, 1), g(1, 2));
    let (r31, r32, r33) = (g(2, 0), g(2, 1), g(2, 2));
    (r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33 + r21 * r32 * r13 + r31 * r12 * r23
        - r31 * r22 * r13) as f32
}

/// `nifti_mat33_inverse` (`1.0l / deti` in extended precision).
pub fn mat33_inverse(r: &Mat33) -> Mat33 {
    let g = |i: usize, j: usize| f64::from(r[i][j]);
    let (r11, r12, r13) = (g(0, 0), g(0, 1), g(0, 2));
    let (r21, r22, r23) = (g(1, 0), g(1, 1), g(1, 2));
    let (r31, r32, r33) = (g(2, 0), g(2, 1), g(2, 2));
    let mut deti = r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33
        + r21 * r32 * r13
        + r31 * r12 * r23
        - r31 * r22 * r13;
    if deti != 0.0 {
        deti = x87::div_to_f64(1.0, deti);
    }
    [
        [
            (deti * (r22 * r33 - r32 * r23)) as f32,
            (deti * (-r12 * r33 + r32 * r13)) as f32,
            (deti * (r12 * r23 - r22 * r13)) as f32,
        ],
        [
            (deti * (-r21 * r33 + r31 * r23)) as f32,
            (deti * (r11 * r33 - r31 * r13)) as f32,
            (deti * (-r11 * r23 + r21 * r13)) as f32,
        ],
        [
            (deti * (r21 * r32 - r31 * r22)) as f32,
            (deti * (-r11 * r32 + r31 * r12)) as f32,
            (deti * (r11 * r22 - r21 * r12)) as f32,
        ],
    ]
}

/// `nifti_mat44_inverse` (`1.0l / deti` in extended precision; `m[3][3]` 0 flags a singular
/// matrix).
pub fn mat44_inverse(r: &Mat44) -> Mat44 {
    let g = |i: usize, j: usize| f64::from(r[i][j]);
    let (r11, r12, r13) = (g(0, 0), g(0, 1), g(0, 2));
    let (r21, r22, r23) = (g(1, 0), g(1, 1), g(1, 2));
    let (r31, r32, r33) = (g(2, 0), g(2, 1), g(2, 2));
    let (v1, v2, v3) = (g(0, 3), g(1, 3), g(2, 3));
    let mut deti = r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33
        + r21 * r32 * r13
        + r31 * r12 * r23
        - r31 * r22 * r13;
    if deti != 0.0 {
        deti = x87::div_to_f64(1.0, deti);
    }
    let mut q = [[0.0f32; 4]; 4];
    q[0][0] = (deti * (r22 * r33 - r32 * r23)) as f32;
    q[0][1] = (deti * (-r12 * r33 + r32 * r13)) as f32;
    q[0][2] = (deti * (r12 * r23 - r22 * r13)) as f32;
    q[0][3] = (deti
        * (-r12 * r23 * v3 + r12 * v2 * r33 + r22 * r13 * v3
            - r22 * v1 * r33
            - r32 * r13 * v2
            + r32 * v1 * r23)) as f32;
    q[1][0] = (deti * (-r21 * r33 + r31 * r23)) as f32;
    q[1][1] = (deti * (r11 * r33 - r31 * r13)) as f32;
    q[1][2] = (deti * (-r11 * r23 + r21 * r13)) as f32;
    q[1][3] = (deti
        * (r11 * r23 * v3 - r11 * v2 * r33 - r21 * r13 * v3
            + r21 * v1 * r33
            + r31 * r13 * v2
            - r31 * v1 * r23)) as f32;
    q[2][0] = (deti * (r21 * r32 - r31 * r22)) as f32;
    q[2][1] = (deti * (-r11 * r32 + r31 * r12)) as f32;
    q[2][2] = (deti * (r11 * r22 - r21 * r12)) as f32;
    q[2][3] = (deti
        * (-r11 * r22 * v3 + r11 * r32 * v2 + r21 * r12 * v3
            - r21 * r32 * v1
            - r31 * r12 * v2
            + r31 * r22 * v1)) as f32;
    q[3][3] = if deti == 0.0 { 0.0 } else { 1.0 };
    q
}

fn mat33_rownorm(a: &Mat33) -> f32 {
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

fn mat33_colnorm(a: &Mat33) -> f32 {
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

/// `nifti_mat33_polar`: the orthogonal matrix closest to `a` (float matrices, float scalars).
pub fn mat33_polar(a: &Mat33) -> Mat33 {
    let mut x = *a;
    let mut z = [[0.0f32; 3]; 3];
    let mut dif: f32 = 1.0;
    let mut k = 0;
    let mut gam = mat33_determ(&x);
    while gam == 0.0 {
        gam = (0.00001 * (0.001 + f64::from(mat33_rownorm(&x)))) as f32;
        x[0][0] += gam;
        x[1][1] += gam;
        x[2][2] += gam;
        gam = mat33_determ(&x);
    }
    loop {
        let y = mat33_inverse(&x);
        let gmi: f32;
        if dif > 0.3 {
            let alp = (mat33_rownorm(&x) * mat33_colnorm(&x)).sqrt();
            let bet = (mat33_rownorm(&y) * mat33_colnorm(&y)).sqrt();
            gam = (bet / alp).sqrt();
            gmi = (1.0 / f64::from(gam)) as f32;
        } else {
            gam = 1.0;
            gmi = 1.0;
        }
        for i in 0..3 {
            for j in 0..3 {
                // 0.5 * (gam*X[i][j] + gmi*Y[j][i]): float sum, times a double 0.5.
                let s = gam * x[i][j] + gmi * y[j][i];
                z[i][j] = (0.5 * f64::from(s)) as f32;
            }
        }
        // fabs(Z00-X00)+fabs(Z01-X01)+... in row order, in float.
        dif = (z[0][0] - x[0][0]).abs();
        for (i, j) in [(0, 1), (0, 2), (1, 0), (1, 1), (1, 2), (2, 0), (2, 1), (2, 2)] {
            dif += (z[i][j] - x[i][j]).abs();
        }
        k += 1;
        if k > 100 || f64::from(dif) < 3.0e-6 {
            break;
        }
        x = z;
    }
    z
}

/// `mat44_to_mat33`.
pub fn mat44_to_mat33(x: &Mat44) -> Mat33 {
    std::array::from_fn(|i| std::array::from_fn(|j| x[i][j]))
}

// ------------------------------------------------------------------------------------------------
// Raw headers

struct Cursor<'a> {
    b: &'a [u8],
    swap: bool,
}

impl Cursor<'_> {
    fn bytes<const N: usize>(&self, off: usize) -> [u8; N] {
        let mut a = [0u8; N];
        a.copy_from_slice(&self.b[off..off + N]);
        if self.swap {
            a.reverse();
        }
        a
    }
    fn i16(&self, off: usize) -> i16 {
        i16::from_le_bytes(self.bytes::<2>(off))
    }
    fn i32(&self, off: usize) -> i32 {
        i32::from_le_bytes(self.bytes::<4>(off))
    }
    fn i64(&self, off: usize) -> i64 {
        i64::from_le_bytes(self.bytes::<8>(off))
    }
    fn f32(&self, off: usize) -> f32 {
        f32::from_le_bytes(self.bytes::<4>(off))
    }
    fn f64(&self, off: usize) -> f64 {
        f64::from_le_bytes(self.bytes::<8>(off))
    }
    fn raw<const N: usize>(&self, off: usize) -> [u8; N] {
        let mut a = [0u8; N];
        a.copy_from_slice(&self.b[off..off + N]);
        a
    }
}

/// `std::string(char*)`: the bytes up to the first NUL, reading past `len` into the following
/// fields of the raw header when the field has none (as FSL's `std::string` construction
/// does).
fn c_string(b: &[u8], off: usize) -> Vec<u8> {
    let rest = &b[off..];
    let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
    rest[..end].to_vec()
}

/// Which raw header the first 348 bytes announce (`headerType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeaderType {
    Analyze,
    Nifti1,
    Nifti2,
    Other,
}

fn header_type(first348: &[u8]) -> HeaderType {
    let sizeof_hdr = i32::from_le_bytes(first348[0..4].try_into().expect("4 bytes"));
    let v2 = nifti2_version(sizeof_hdr);
    let v1 = nifti_version(&first348[344..348]);
    if v2 == 0 {
        return HeaderType::Other;
    }
    let t = if v2 == 1 { v1 } else { v2 };
    match t {
        0 => HeaderType::Analyze,
        1 => HeaderType::Nifti1,
        2 => HeaderType::Nifti2,
        // HeaderType values beyond nifti2 (magic "ni3".."ni9" with a 348-byte header) fall
        // outside FSL's switch: no header is read and the defaults stand.
        _ => HeaderType::Other,
    }
}

fn parse_nifti1(raw: &[u8], h: &mut NiftiHeader) {
    let sizeof_hdr = i32::from_le_bytes(raw[0..4].try_into().expect("4 bytes"));
    let swap = sizeof_hdr == 1_543_569_408 || sizeof_hdr == 469_893_120;
    h.was_wrong_endian = swap;
    let c = Cursor { b: raw, swap };
    // readCommonFieldsFromRaw
    h.sizeof_hdr = c.i32(0);
    h.vox_offset = c.f32(108) as i64;
    h.datatype = c.i16(70);
    h.bits_per_voxel = i32::from(c.i16(72));
    for i in 0..8 {
        h.dim[i] = i64::from(c.i16(40 + 2 * i));
        h.pixdim[i] = f64::from(c.f32(76 + 4 * i));
    }
    h.aux_file = c_string(raw, 228);
    h.description = c_string(raw, 148);
    h.cal_max = f64::from(c.f32(124));
    h.cal_min = f64::from(c.f32(128));
    // readNiftiFieldsFromRaw
    h.intent_p1 = f64::from(c.f32(56));
    h.intent_p2 = f64::from(c.f32(60));
    h.intent_p3 = f64::from(c.f32(64));
    h.intent_name = c_string(raw, 328);
    h.intent_code = i32::from(c.i16(68));
    h.slice_ordering = raw[39] as i8;
    h.slice_start = i64::from(c.i16(74));
    h.slice_end = i64::from(c.i16(120));
    h.slice_code = i32::from(raw[122] as i8);
    h.slice_duration = f64::from(c.f32(132));
    h.scl_slope = f64::from(c.f32(112));
    h.scl_inter = f64::from(c.f32(116));
    for j in 0..4 {
        h.s_x[j] = f64::from(c.f32(280 + 4 * j));
        h.s_y[j] = f64::from(c.f32(296 + 4 * j));
        h.s_z[j] = f64::from(c.f32(312 + 4 * j));
    }
    h.q_b = f64::from(c.f32(256));
    h.q_c = f64::from(c.f32(260));
    h.q_d = f64::from(c.f32(264));
    h.q_x = f64::from(c.f32(268));
    h.q_y = f64::from(c.f32(272));
    h.q_z = f64::from(c.f32(276));
    h.sform_code = i32::from(c.i16(254));
    h.qform_code = i32::from(c.i16(252));
    h.units = i32::from(raw[123] as i8);
    h.toffset = f64::from(c.f32(136));
    h.magic = raw[344..348].to_vec();
    // legacyFields.readFieldsFromRaw(nifti_1_header)
    let l = &mut h.legacy;
    l.data_type = c.raw::<10>(4);
    l.db_name = c.raw::<18>(14);
    l.extents = c.i32(32);
    l.session_error = c.i16(36);
    l.regular = raw[38];
    l.glmax = c.i32(140);
    l.glmin = c.i32(144);
}

fn parse_analyze(raw: &[u8], h: &mut NiftiHeader) {
    let sizeof_hdr = i32::from_le_bytes(raw[0..4].try_into().expect("4 bytes"));
    let swap = sizeof_hdr == 1_543_569_408 || sizeof_hdr == 469_893_120;
    h.was_wrong_endian = swap;
    let c = Cursor { b: raw, swap };
    h.sizeof_hdr = c.i32(0);
    h.vox_offset = c.f32(108) as i64;
    h.datatype = c.i16(70);
    h.bits_per_voxel = i32::from(c.i16(72));
    for i in 0..8 {
        h.dim[i] = i64::from(c.i16(40 + 2 * i));
        h.pixdim[i] = f64::from(c.f32(76 + 4 * i));
    }
    h.aux_file = c_string(raw, 228);
    h.description = c_string(raw, 148);
    h.cal_max = f64::from(c.f32(124));
    h.cal_min = f64::from(c.f32(128));
    let l = &mut h.legacy;
    l.data_type = c.raw::<10>(4);
    l.db_name = c.raw::<18>(14);
    l.extents = c.i32(32);
    l.session_error = c.i16(36);
    l.regular = raw[38];
    l.hkey_un0 = raw[39];
    l.vox_units = c.raw::<4>(56);
    l.cal_units = c.raw::<8>(60);
    l.unused1 = c.i16(68);
    l.dim_un0 = c.i16(74);
    l.funused1 = c.f32(112);
    l.funused2 = c.f32(116);
    l.funused3 = c.f32(120);
    l.compressed = c.i32(132);
    l.verified = c.i32(136);
    l.glmax = c.i32(140);
    l.glmin = c.i32(144);
    l.orient = raw[252];
    // byteSwapAnalyzeFields swaps the originator as five shorts.
    let mut orig = c.raw::<10>(253);
    if swap {
        for k in 0..5 {
            orig.swap(2 * k, 2 * k + 1);
        }
    }
    l.originator = orig;
    l.generated = c.raw::<10>(263);
    l.scannum = c.raw::<10>(273);
    l.patient_id = c.raw::<10>(283);
    l.exp_date = c.raw::<10>(293);
    l.exp_time = c.raw::<10>(303);
    l.hist_un0 = c.raw::<3>(313);
    l.views = c.i32(316);
    l.vols_added = c.i32(320);
    l.start_field = c.i32(324);
    l.field_skip = c.i32(328);
    l.omax = c.i32(332);
    l.omin = c.i32(336);
    l.smax = c.i32(340);
    l.smin = c.i32(344);
    // ANALYZE has no magic: FSL's NiftiHeader keeps its initial four NULs.
}

fn parse_nifti2(raw: &[u8], h: &mut NiftiHeader) {
    let sizeof_hdr = i32::from_le_bytes(raw[0..4].try_into().expect("4 bytes"));
    let swap = sizeof_hdr == 1_543_569_408 || sizeof_hdr == 469_893_120;
    h.was_wrong_endian = swap;
    let c = Cursor { b: raw, swap };
    h.sizeof_hdr = c.i32(0);
    h.vox_offset = c.i64(168);
    h.datatype = c.i16(12);
    h.bits_per_voxel = i32::from(c.i16(14));
    for i in 0..8 {
        h.dim[i] = c.i64(16 + 8 * i);
        h.pixdim[i] = c.f64(104 + 8 * i);
    }
    h.aux_file = c_string(raw, 320);
    h.description = c_string(raw, 240);
    h.cal_max = c.f64(192);
    h.cal_min = c.f64(200);
    h.intent_p1 = c.f64(80);
    h.intent_p2 = c.f64(88);
    h.intent_p3 = c.f64(96);
    h.intent_name = c_string(raw, 508);
    h.intent_code = c.i32(504);
    h.slice_ordering = raw[524] as i8;
    h.slice_start = c.i64(224);
    h.slice_end = c.i64(232);
    h.slice_code = c.i32(496);
    h.slice_duration = c.f64(208);
    h.scl_slope = c.f64(176);
    h.scl_inter = c.f64(184);
    for j in 0..4 {
        h.s_x[j] = c.f64(400 + 8 * j);
        h.s_y[j] = c.f64(432 + 8 * j);
        h.s_z[j] = c.f64(464 + 8 * j);
    }
    h.q_b = c.f64(352);
    h.q_c = c.f64(360);
    h.q_d = c.f64(368);
    h.q_x = c.f64(376);
    h.q_y = c.f64(384);
    h.q_z = c.f64(392);
    h.sform_code = c.i32(348);
    h.qform_code = c.i32(344);
    h.units = c.i32(500);
    h.toffset = c.f64(216);
    h.magic = raw[4..12].to_vec();
}

fn put<const N: usize>(b: &mut [u8], off: usize, v: [u8; N]) {
    b[off..off + N].copy_from_slice(&v);
}

/// `strncpy(dst, src.c_str(), n)`: the string up to its first NUL, then NUL padding.
fn strncpy(b: &mut [u8], off: usize, src: &[u8], n: usize) {
    let end = src.iter().position(|&c| c == 0).unwrap_or(src.len());
    for i in 0..n {
        b[off + i] = if i < end { src[i] } else { 0 };
    }
}

/// `writeRawHeader<nifti_1_header>`.
fn write_nifti1(h: &NiftiHeader) -> Vec<u8> {
    let mut b = vec![0u8; 348];
    // writeCommonFieldsToRaw
    put(&mut b, 0, h.sizeof_hdr.to_le_bytes());
    for i in 0..8 {
        put(&mut b, 40 + 2 * i, (h.dim[i] as i16).to_le_bytes());
        put(&mut b, 76 + 4 * i, (h.pixdim[i] as f32).to_le_bytes());
    }
    put(&mut b, 70, h.datatype.to_le_bytes());
    put(&mut b, 72, (h.bits_per_voxel as i16).to_le_bytes());
    put(&mut b, 108, (h.vox_offset as f32).to_le_bytes());
    put(&mut b, 124, (h.cal_max as f32).to_le_bytes());
    put(&mut b, 128, (h.cal_min as f32).to_le_bytes());
    strncpy(&mut b, 148, &h.description, 80);
    strncpy(&mut b, 228, &h.aux_file, 24);
    // writeNiftiFieldsToRaw
    let m = h.magic.len().min(4);
    b[344..344 + m].copy_from_slice(&h.magic[..m]);
    put(&mut b, 56, (h.intent_p1 as f32).to_le_bytes());
    put(&mut b, 60, (h.intent_p2 as f32).to_le_bytes());
    put(&mut b, 64, (h.intent_p3 as f32).to_le_bytes());
    strncpy(&mut b, 328, &h.intent_name, 16);
    put(&mut b, 68, (h.intent_code as i16).to_le_bytes());
    b[39] = h.slice_ordering as u8;
    put(&mut b, 74, (h.slice_start as i16).to_le_bytes());
    put(&mut b, 120, (h.slice_end as i16).to_le_bytes());
    b[122] = h.slice_code as i8 as u8;
    put(&mut b, 132, (h.slice_duration as f32).to_le_bytes());
    put(&mut b, 112, (h.scl_slope as f32).to_le_bytes());
    put(&mut b, 116, (h.scl_inter as f32).to_le_bytes());
    for j in 0..4 {
        put(&mut b, 280 + 4 * j, (h.s_x[j] as f32).to_le_bytes());
        put(&mut b, 296 + 4 * j, (h.s_y[j] as f32).to_le_bytes());
        put(&mut b, 312 + 4 * j, (h.s_z[j] as f32).to_le_bytes());
    }
    put(&mut b, 256, (h.q_b as f32).to_le_bytes());
    put(&mut b, 260, (h.q_c as f32).to_le_bytes());
    put(&mut b, 264, (h.q_d as f32).to_le_bytes());
    put(&mut b, 268, (h.q_x as f32).to_le_bytes());
    put(&mut b, 272, (h.q_y as f32).to_le_bytes());
    put(&mut b, 276, (h.q_z as f32).to_le_bytes());
    put(&mut b, 254, (h.sform_code as i16).to_le_bytes());
    put(&mut b, 252, (h.qform_code as i16).to_le_bytes());
    b[123] = h.units as i8 as u8;
    put(&mut b, 136, (h.toffset as f32).to_le_bytes());
    // legacyFields.writeFieldsToRaw(nifti_1_header)
    let l = &h.legacy;
    b[4..14].copy_from_slice(&l.data_type);
    b[14..32].copy_from_slice(&l.db_name);
    put(&mut b, 32, l.extents.to_le_bytes());
    put(&mut b, 36, l.session_error.to_le_bytes());
    b[38] = l.regular;
    put(&mut b, 140, l.glmax.to_le_bytes());
    put(&mut b, 144, l.glmin.to_le_bytes());
    b
}

/// `writeRawHeader<analyzeHeader>`.
fn write_analyze(h: &NiftiHeader) -> Vec<u8> {
    let mut b = vec![0u8; 348];
    put(&mut b, 0, h.sizeof_hdr.to_le_bytes());
    for i in 0..8 {
        put(&mut b, 40 + 2 * i, (h.dim[i] as i16).to_le_bytes());
        put(&mut b, 76 + 4 * i, (h.pixdim[i] as f32).to_le_bytes());
    }
    put(&mut b, 70, h.datatype.to_le_bytes());
    put(&mut b, 72, (h.bits_per_voxel as i16).to_le_bytes());
    put(&mut b, 108, (h.vox_offset as f32).to_le_bytes());
    put(&mut b, 124, (h.cal_max as f32).to_le_bytes());
    put(&mut b, 128, (h.cal_min as f32).to_le_bytes());
    strncpy(&mut b, 148, &h.description, 80);
    strncpy(&mut b, 228, &h.aux_file, 24);
    let l = &h.legacy;
    b[4..14].copy_from_slice(&l.data_type);
    b[14..32].copy_from_slice(&l.db_name);
    put(&mut b, 32, l.extents.to_le_bytes());
    put(&mut b, 36, l.session_error.to_le_bytes());
    b[38] = l.regular;
    b[39] = l.hkey_un0;
    b[56..60].copy_from_slice(&l.vox_units);
    b[60..68].copy_from_slice(&l.cal_units);
    put(&mut b, 68, l.unused1.to_le_bytes());
    put(&mut b, 74, l.dim_un0.to_le_bytes());
    put(&mut b, 112, l.funused1.to_le_bytes());
    put(&mut b, 116, l.funused2.to_le_bytes());
    put(&mut b, 120, l.funused3.to_le_bytes());
    put(&mut b, 132, l.compressed.to_le_bytes());
    put(&mut b, 136, l.verified.to_le_bytes());
    put(&mut b, 140, l.glmax.to_le_bytes());
    put(&mut b, 144, l.glmin.to_le_bytes());
    b[252] = l.orient;
    b[253..263].copy_from_slice(&l.originator);
    b[263..273].copy_from_slice(&l.generated);
    b[273..283].copy_from_slice(&l.scannum);
    b[283..293].copy_from_slice(&l.patient_id);
    b[293..303].copy_from_slice(&l.exp_date);
    b[303..313].copy_from_slice(&l.exp_time);
    b[313..316].copy_from_slice(&l.hist_un0);
    put(&mut b, 316, l.views.to_le_bytes());
    put(&mut b, 320, l.vols_added.to_le_bytes());
    put(&mut b, 324, l.start_field.to_le_bytes());
    put(&mut b, 328, l.field_skip.to_le_bytes());
    put(&mut b, 332, l.omax.to_le_bytes());
    put(&mut b, 336, l.omin.to_le_bytes());
    put(&mut b, 340, l.smax.to_le_bytes());
    put(&mut b, 344, l.smin.to_le_bytes());
    b
}

/// `writeRawHeader<nifti_2_header>`.
fn write_nifti2(h: &NiftiHeader) -> Vec<u8> {
    let mut b = vec![0u8; 540];
    put(&mut b, 0, h.sizeof_hdr.to_le_bytes());
    for i in 0..8 {
        put(&mut b, 16 + 8 * i, h.dim[i].to_le_bytes());
        put(&mut b, 104 + 8 * i, h.pixdim[i].to_le_bytes());
    }
    put(&mut b, 12, h.datatype.to_le_bytes());
    put(&mut b, 14, (h.bits_per_voxel as i16).to_le_bytes());
    put(&mut b, 168, h.vox_offset.to_le_bytes());
    put(&mut b, 192, h.cal_max.to_le_bytes());
    put(&mut b, 200, h.cal_min.to_le_bytes());
    strncpy(&mut b, 240, &h.description, 80);
    strncpy(&mut b, 320, &h.aux_file, 24);
    let m = h.magic.len().min(8);
    b[4..4 + m].copy_from_slice(&h.magic[..m]);
    put(&mut b, 80, h.intent_p1.to_le_bytes());
    put(&mut b, 88, h.intent_p2.to_le_bytes());
    put(&mut b, 96, h.intent_p3.to_le_bytes());
    strncpy(&mut b, 508, &h.intent_name, 16);
    put(&mut b, 504, h.intent_code.to_le_bytes());
    b[524] = h.slice_ordering as u8;
    put(&mut b, 224, h.slice_start.to_le_bytes());
    put(&mut b, 232, h.slice_end.to_le_bytes());
    put(&mut b, 496, h.slice_code.to_le_bytes());
    put(&mut b, 208, h.slice_duration.to_le_bytes());
    put(&mut b, 176, h.scl_slope.to_le_bytes());
    put(&mut b, 184, h.scl_inter.to_le_bytes());
    for j in 0..4 {
        put(&mut b, 400 + 8 * j, h.s_x[j].to_le_bytes());
        put(&mut b, 432 + 8 * j, h.s_y[j].to_le_bytes());
        put(&mut b, 464 + 8 * j, h.s_z[j].to_le_bytes());
    }
    put(&mut b, 352, h.q_b.to_le_bytes());
    put(&mut b, 360, h.q_c.to_le_bytes());
    put(&mut b, 368, h.q_d.to_le_bytes());
    put(&mut b, 376, h.q_x.to_le_bytes());
    put(&mut b, 384, h.q_y.to_le_bytes());
    put(&mut b, 392, h.q_z.to_le_bytes());
    put(&mut b, 348, h.sform_code.to_le_bytes());
    put(&mut b, 344, h.qform_code.to_le_bytes());
    put(&mut b, 500, h.units.to_le_bytes());
    put(&mut b, 216, h.toffset.to_le_bytes());
    b
}

// ------------------------------------------------------------------------------------------------
// Files (znzlib through gzopen: compressed or plain, whatever the name)

/// The whole content of a file as `gzread` returns it: decompressed when it starts with the gzip
/// magic, as stored otherwise. `Err` carries FSL's message for a file that cannot be opened.
fn read_file(path: &Path) -> Result<Vec<u8>, NiftiError> {
    let mut f =
        File::open(path).map_err(|_| err(format!("Error: cant open file {}", path.display())))?;
    let mut raw = Vec::new();
    f.read_to_end(&mut raw)
        .map_err(|_| err(format!("Error: cant open file {}", path.display())))?;
    if raw.len() >= 2 && raw[0] == 0x1f && raw[1] == 0x8b {
        let mut out = Vec::new();
        let mut dec = MultiGzDecoder::new(&raw[..]);
        // A truncated stream: gzread returns what it could decompress.
        let _ = dec.read_to_end(&mut out);
        Ok(out)
    } else {
        Ok(raw)
    }
}

/// `readRawBytes`: `len` bytes at `pos`, or FSL's short-read error.
fn read_at(data: &[u8], pos: usize, len: usize) -> Result<&[u8], NiftiError> {
    if pos.checked_add(len).is_none_or(|end| end > data.len()) {
        return Err(err("Error: short read, file may be truncated"));
    }
    Ok(&data[pos..pos + len])
}

/// `fileIO::readHeader()` on the content of a file.
fn read_header(data: &[u8]) -> Result<NiftiHeader, NiftiError> {
    let first = read_at(data, 0, 348)?;
    let mut h = NiftiHeader::default();
    match header_type(first) {
        HeaderType::Analyze => parse_analyze(first, &mut h),
        HeaderType::Nifti1 => parse_nifti1(first, &mut h),
        HeaderType::Nifti2 => {
            let raw = read_at(data, 0, 540)?;
            parse_nifti2(raw, &mut h);
        }
        HeaderType::Other => {
            let sizeof_hdr = i32::from_le_bytes(first[0..4].try_into().expect("4 bytes"));
            if nifti2_version(sizeof_hdr) == 0 {
                return Err(err(
                    "Error: file does not appear to be a valid NIFTI or ANALYZE image",
                ));
            }
        }
    }
    Ok(h)
}

/// `fileIO::readExtensions`.
fn read_extensions(data: &[u8], h: &NiftiHeader) -> Result<Vec<NiftiExtension>, NiftiError> {
    let mut out = Vec::new();
    let start = h.sizeof_hdr.max(0) as usize;
    let Ok(flag) = read_at(data, start, 4) else {
        return Ok(out);
    };
    if flag[0] == 0 {
        return Ok(out);
    }
    let mut pos = start + 4;
    let mut bytes_read = start + 4;
    loop {
        if h.single_file() && bytes_read + 8 > h.vox_offset.max(0) as usize {
            return Ok(out);
        }
        let ib = match read_at(data, pos, 8) {
            Ok(b) => b,
            Err(e) => {
                if h.single_file() {
                    return Err(e);
                }
                return Ok(out);
            }
        };
        pos += 8;
        bytes_read += 8;
        let mut e0 = [ib[0], ib[1], ib[2], ib[3]];
        let mut e1 = [ib[4], ib[5], ib[6], ib[7]];
        if h.was_wrong_endian {
            e0.reverse();
            e1.reverse();
        }
        let esize = i32::from_le_bytes(e0);
        let ecode = i32::from_le_bytes(e1);
        if h.single_file()
            && (bytes_read as i64 + i64::from(esize) - 8) as usize > h.vox_offset.max(0) as usize
        {
            return Ok(out);
        }
        if esize == 0 || esize % 16 != 0 {
            return Err(err("Error: esize must be positive multiple of 16"));
        }
        let n = (esize - 8) as usize;
        let edata = match read_at(data, pos, n) {
            Ok(b) => b.to_vec(),
            Err(e) => {
                if h.single_file() {
                    return Err(e);
                }
                return Ok(out);
            }
        };
        pos += n;
        bytes_read += n;
        out.push(NiftiExtension {
            esize,
            ecode,
            edata,
        });
        if h.single_file() && bytes_read == h.vox_offset.max(0) as usize {
            return Ok(out);
        }
    }
}

/// A loaded image: its header (with `dim` beyond `dim[0]` set to 1), its extensions and its
/// raw data in native byte order.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub header: NiftiHeader,
    pub extensions: Vec<NiftiExtension>,
    pub data: Vec<u8>,
}

/// `loadHeader(filename)`.
pub fn load_header(path: &Path) -> Result<NiftiHeader, NiftiError> {
    let data = read_file(path)?;
    read_header(&data)
}

/// `loadImageROI(filename, buffer, extensions)` for the whole image.
pub fn load_image(path: &Path) -> Result<Loaded, NiftiError> {
    let content = read_file(path)?;
    let mut header = read_header(&content)?;
    let d0 = header.dim[0].clamp(-1, 7);
    let from = (d0 + 1).max(0) as usize;
    for d in header.dim.iter_mut().skip(from) {
        *d = 1;
    }
    let extensions = read_extensions(&content, &header)?;
    for d in 1..=7 {
        if header.dim[d] < 1 {
            // xmax = dim-1 < xmin = 0: FSL's ROI check.
            return Err(err(format!("Error: Nonsensical ROI for {}", path.display())));
        }
    }
    let width = header.datum_byte_width();
    let n: usize = header.dim[1..=7].iter().map(|&d| d as usize).product();
    let (source, offset) = if header.single_file() {
        (content, header.nominal_vox_offset().max(0) as usize)
    } else {
        // The data are in the .img beside the .hdr. FSL reads the .hdr's first four bytes
        // without decompressing: if they are a valid `sizeof_hdr` the header was not
        // compressed, and the .img is then read without decompression too.
        let name = path.to_string_lossy();
        let pos = name
            .rfind(".hdr")
            .ok_or_else(|| err(format!("Error: cant open file {name}")))?;
        let mut first4 = [0u8; 4];
        let was_compressed = match File::open(path).and_then(|mut f| f.read_exact(&mut first4)) {
            Ok(()) => nifti2_version(i32::from_le_bytes(first4)) == 0,
            Err(_) => return Err(err("Error: short read, file may be truncated")),
        };
        let img = format!("{}.img{}", &name[..pos], &name[pos + 4..]);
        let bytes = if was_compressed {
            read_file(Path::new(&img))?
        } else {
            std::fs::read(&img).map_err(|_| err(format!("Error: cant open file {img}")))?
        };
        (bytes, header.nominal_vox_offset().max(0) as usize)
    };
    let bytes = read_at(&source, offset, n * width)?;
    let mut data = bytes.to_vec();
    if header.was_wrong_endian && width > 1 {
        for chunk in data.chunks_mut(width) {
            chunk.reverse();
        }
    }
    Ok(Loaded {
        header,
        extensions,
        data,
    })
}

/// `saveImage(filename, buffer, extensions, header, useCompression)`.
pub fn save_image(
    filename: &str,
    data: &[u8],
    extensions: &[NiftiExtension],
    header: &NiftiHeader,
    use_compression: bool,
) -> Result<(), NiftiError> {
    let mut header = header.clone();
    let mut size_of_extensions = 4usize;
    for _ in extensions {
        // FSL adds the size of the first extension once per extension.
        size_of_extensions += extensions[0].extension_size();
    }
    header.vox_offset = i64::from(header.sizeof_hdr) + size_of_extensions as i64;
    if !header.single_file() {
        header.vox_offset = 0;
    }
    let raw = if header.is_analyze() {
        write_analyze(&header)
    } else if header.nifti_version() == 1 {
        write_nifti1(&header)
    } else {
        write_nifti2(&header)
    };
    let mut first = raw;
    if !header.is_analyze() {
        let mut flag = [0u8; 4];
        if !extensions.is_empty() {
            flag[0] = 1;
        }
        first.extend_from_slice(&flag);
        for e in extensions {
            if e.esize % 16 != 0 {
                return Err(err("Error: extension size must be multiple of 16 bytes"));
            }
            first.extend_from_slice(&e.esize.to_le_bytes());
            first.extend_from_slice(&e.ecode.to_le_bytes());
            let n = (e.esize - 8).max(0) as usize;
            let mut d = e.edata.clone();
            d.resize(n, 0);
            first.extend_from_slice(&d);
        }
    }
    let n = header.n_elements() * header.datum_byte_width();
    if header.single_file() {
        first.extend_from_slice(&data[..n.min(data.len())]);
        write_file(filename, &first, use_compression)
    } else {
        write_file(filename, &first, use_compression)?;
        let pos = filename.rfind(".hdr").expect("a pair's header name ends in .hdr");
        let img = format!("{}.img{}", &filename[..pos], &filename[pos + 4..]);
        write_file(&img, &data[..n.min(data.len())], use_compression)
    }
}

fn write_file(name: &str, bytes: &[u8], compress: bool) -> Result<(), NiftiError> {
    let open_err = || err(format!("Error: cant open file {name}"));
    let f = File::create(name).map_err(|_| open_err())?;
    let short = |_: io::Error| err("Error: short write, output file will be truncated");
    if compress {
        let mut enc = GzEncoder::new(f, Compression::default());
        enc.write_all(bytes).map_err(short)?;
        enc.finish().map_err(short)?;
    } else {
        let mut f = f;
        f.write_all(bytes).map_err(short)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quaternion_round_trip_of_a_rotation() {
        let r = quatern_to_mat44(0.1, -0.2, 0.05, 10.0, -20.0, 30.0, 2.0, 2.0, 3.0, -1.0);
        let q = mat44_to_quatern(&r);
        assert!((q.qb - 0.1).abs() < 1e-6);
        assert!((q.qc + 0.2).abs() < 1e-6);
        assert!((q.qd - 0.05).abs() < 1e-6);
        assert_eq!(q.qfac, -1.0);
        assert_eq!((q.qx, q.qy, q.qz), (10.0, -20.0, 30.0));
    }

    #[test]
    fn header_bytes_round_trip() {
        let mut h = NiftiHeader::default();
        h.set_nifti_version(1, true);
        h.dim = [4, 3, 4, 5, 2, 1, 1, 1];
        h.pixdim = [1.0, 2.0, 2.0, 3.0, 1.5, 0.0, 0.0, 0.0];
        h.datatype = dt::FLOAT;
        h.bits_per_voxel = 32;
        h.vox_offset = 352;
        h.description = b"larmorx".to_vec();
        h.sform_code = 1;
        h.s_x = [2.0, 0.0, 0.0, -3.0];
        let raw = write_nifti1(&h);
        let mut back = NiftiHeader::default();
        parse_nifti1(&raw, &mut back);
        assert_eq!(back.dim, h.dim);
        assert_eq!(back.pixdim, h.pixdim);
        assert_eq!(back.description, b"larmorx".to_vec());
        assert_eq!(back.s_x, h.s_x);
        assert_eq!(back.magic, b"n+1\0".to_vec());
        assert!(back.single_file() && !back.is_analyze());
        let mut h2 = h.clone();
        h2.set_nifti_version(2, true);
        let raw2 = write_nifti2(&h2);
        let mut back2 = NiftiHeader::default();
        parse_nifti2(&raw2, &mut back2);
        assert_eq!(back2.dim, h2.dim);
        assert_eq!(back2.magic, b"n+2\0\r\n\x1a\n".to_vec());
    }
}
