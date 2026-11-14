// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Reading NIfTI-1 volumes (`utils/mriio.cpp`, item 3.1-30): `niiRead` ([`nii_read`]: `.nii`, and
//! compressed when the name ends in `z`) with FreeSurfer's header extension (code 14: its 4-byte
//! header and `MRITAGread`'s tags, the RAS transform checked against the sform or qform), and
//! `nifti1Read` ([`nifti1_read`]: the `.hdr`/`.img` pair); their helpers `swap_nifti_1_header`,
//! `niiPrintHdr`, `niftiQformToMri`, `niftiSformToMri`, `__niiReadSetVox2ras`,
//! `__readFSniiextensionHeader`, `__niiReadHeaderextension`. Wired into [`crate::mriio::mri_read`]
//! (`NII_FILE`, `NIFTI1_FILE`).
//!
//! The arithmetic follows the official build's object code (GCC 8.4.0, SSE2): the quaternion's
//! steps in `float` and `double` in its operand order (a NaN operand's payload passed on as SSE
//! passes it, [`larmorx_freesurfer_core::sse`]), a scaled voxel `value * slope + intercept` (for
//! `float` data `slope * value + intercept`), a `double` stored in a `float` as `cvtsd2ss` stores
//! it.
//!
//! Defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-io.md`, Part 15):
//! FS-453 (a pair's names cut at 1,023 bytes), FS-454 (scaled unsigned 32-bit data read twice),
//! FS-455 (big-endian `double` data swapped by halves), FS-456 (big-endian complex data not
//! swapped), FS-457 (unsigned 32-bit values above 2^31 - 1 read as negative `int`s: kept, rule C),
//! FS-458 (the extents past `dim[0]` used), FS-459 (big-endian extensions never read), FS-460
//! (another extension's data read as the next extension's size and code), FS-641 (the extender
//! read past the file's end: memory contents), FS-642 (the header's texts printed past the
//! header), FS-643 (`glmin` not swapped in a big-endian header).

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::{
    FS_453, FS_454, FS_455, FS_456, FS_457, FS_458, FS_459, FS_460, FS_641, FS_642, FS_643,
};
use larmorx_freesurfer_core::diag::DIAG_INFO;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_UNSUPPORTED, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_core::matrix_ops::matrix_print;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::{cstd, utils};
use larmorx_freesurfer_mri::mri::{
    self, MGH_VERSION, MGZ_INTENT_WARPMAP, MRI_FLOAT, MRI_FLOAT_COMPLEX, MRI_INT, MRI_SHORT,
    MRI_UCHAR, MRI_USHRT, Mri,
};
use larmorx_freesurfer_mri::vol_geom::{self, VolGeom};
use larmorx_io::znz::{Whence, ZnzFile};

use crate::cfile::CFile;
use crate::fio;

const STRLEN: usize = 1024;

const DT_UNSIGNED_CHAR: i16 = 2;
const DT_SIGNED_SHORT: i16 = 4;
const DT_SIGNED_INT: i16 = 8;
const DT_FLOAT: i16 = 16;
const DT_COMPLEX: i16 = 32;
const DT_DOUBLE: i16 = 64;
const DT_INT8: i16 = 256;
const DT_UINT16: i16 = 512;
const DT_UINT32: i16 = 768;

const NIFTI_UNITS_METER: u8 = 1;
const NIFTI_UNITS_MM: u8 = 2;
const NIFTI_UNITS_MICRON: u8 = 3;
const NIFTI_UNITS_SEC: u8 = 8;
const NIFTI_UNITS_MSEC: u8 = 16;
const NIFTI_UNITS_USEC: u8 = 24;

const NIFTI_INTENT_DISPVECT: i16 = 1006;
const NIFTI_ECODE_FREESURFER: i32 = 14;
const NIFTI_MAX_ECODE: i32 = 14;
/// `WarpfieldDTFMT::WARPFIELD_DTFMT_DISP_RAS`.
const WARPFIELD_DTFMT_DISP_RAS: i32 = 3;

/// `struct nifti_1_header` as it stands in memory (348 bytes, x86-64's little-endian fields);
/// `swap_nifti_1_header` reverses its fields in place.
#[derive(Clone)]
pub struct NiftiHdr(pub [u8; 348]);

impl NiftiHdr {
    fn i16_at(&self, o: usize) -> i16 {
        i16::from_le_bytes([self.0[o], self.0[o + 1]])
    }
    fn i32_at(&self, o: usize) -> i32 {
        i32::from_le_bytes([self.0[o], self.0[o + 1], self.0[o + 2], self.0[o + 3]])
    }
    fn f32_at(&self, o: usize) -> f32 {
        f32::from_bits(self.i32_at(o) as u32)
    }
    fn set_i16(&mut self, o: usize, v: i16) {
        self.0[o..o + 2].copy_from_slice(&v.to_le_bytes());
    }
    fn sizeof_hdr(&self) -> i32 {
        self.i32_at(0)
    }
    fn dim(&self, i: usize) -> i16 {
        self.i16_at(40 + 2 * i)
    }
    fn pixdim(&self, i: usize) -> f32 {
        self.f32_at(76 + 4 * i)
    }
    fn intent_code(&self) -> i16 {
        self.i16_at(68)
    }
    fn datatype(&self) -> i16 {
        self.i16_at(70)
    }
    fn vox_offset(&self) -> f32 {
        self.f32_at(108)
    }
    fn scl_slope(&self) -> f32 {
        self.f32_at(112)
    }
    fn scl_inter(&self) -> f32 {
        self.f32_at(116)
    }
    fn slice_code(&self) -> u8 {
        self.0[122]
    }
    fn xyzt_units(&self) -> u8 {
        self.0[123]
    }
    fn slice_duration(&self) -> f32 {
        self.f32_at(132)
    }
    fn glmin(&self) -> i32 {
        self.i32_at(144)
    }
    fn qform_code(&self) -> i16 {
        self.i16_at(252)
    }
    fn sform_code(&self) -> i16 {
        self.i16_at(254)
    }
    fn magic(&self) -> &[u8] {
        &self.0[344..348]
    }
}

/// `swap_nifti_1_header(hdr)`: every numeric field byte-reversed except `extents`,
/// `session_error`, `glmax` and `glmin` (FS-643).
fn swap_nifti_1_header(h: &mut NiftiHdr) {
    let mut rev = |o: usize, n: usize| h.0[o..o + n].reverse();
    rev(0, 4);
    for i in 0..8 {
        rev(40 + 2 * i, 2);
    }
    for o in [56, 60, 64] {
        rev(o, 4);
    }
    for o in [68, 70, 72, 74] {
        rev(o, 2);
    }
    for i in 0..8 {
        rev(76 + 4 * i, 4);
    }
    for o in [108, 112, 116] {
        rev(o, 4);
    }
    rev(120, 2);
    for o in [124, 128, 132, 136] {
        rev(o, 4);
    }
    rev(252, 2);
    rev(254, 2);
    for i in 0..18 {
        rev(256 + 4 * i, 4);
    }
}

/// A `float` widened to `double` (`cvtss2sd`): a NaN keeps its sign and payload, quieted.
#[inline]
fn widen(x: f32) -> f64 {
    if x.is_nan() {
        let b = x.to_bits();
        f64::from_bits(
            (u64::from(b & 0x8000_0000) << 32)
                | 0x7ff8_0000_0000_0000
                | (u64::from(b & 0x003f_ffff) << 29),
        )
    } else {
        f64::from(x)
    }
}

/// A `double` stored in a `float` (`cvtsd2ss`): rounded to nearest; a NaN keeps its sign and the
/// top 22 bits of its payload, quieted.
#[inline]
fn narrow(x: f64) -> f32 {
    if x.is_nan() {
        let b = x.to_bits();
        f32::from_bits(
            (((b >> 32) as u32) & 0x8000_0000) | 0x7fc0_0000 | (((b >> 29) as u32) & 0x003f_ffff),
        )
    } else {
        x as f32
    }
}

/// `(long)f` (`cvttss2si`): truncated; a NaN or a value outside the range gives the lowest `long`.
fn float_to_long(f: f32) -> i64 {
    if f.is_nan() || f >= 9.223_372e18 || f < -9.223_372e18 {
        i64::MIN
    } else {
        f as i64
    }
}

/// `niiPrintHdr(stdout, hdr)`: the header's fields one per line (FreeSurfer's labels and spacing);
/// a text field printed as a C string runs on into the fields after it up to a NUL byte (FS-642
/// past the header's end: printed up to the end).
fn nii_print_hdr(g: &mut Globals, out: &mut dyn Write, h: &NiftiHdr) {
    let cstr = |g: &mut Globals, o: usize| -> Vec<u8> {
        let rest = &h.0[o..];
        match rest.iter().position(|&c| c == 0) {
            Some(n) => rest[..n].to_vec(),
            None => {
                g.notices.add(
                    &FS_642,
                    format!(
                        "niiPrintHdr: the text at byte {o} has no NUL before the header's end (FreeSurfer prints the memory after it)"
                    ),
                );
                rest.to_vec()
            }
        }
    };
    let f = |v: f32| cfmt::f(widen(v), 6);
    let mut w = |b: &[u8]| {
        let _ = out.write_all(b);
    };
    w(format!("sizeof_hdr {} \n", h.sizeof_hdr()).as_bytes());
    let t = cstr(g, 4);
    w(b"data_type  ");
    w(&t);
    w(b" \n");
    let t = cstr(g, 14);
    w(b"db_name    ");
    w(&t);
    w(b" \n");
    w(format!("extents    {} \n", h.i32_at(32)).as_bytes());
    w(format!("session_error {} \n", h.i16_at(36)).as_bytes());
    w(b"regular    ");
    w(&[h.0[38]]);
    w(b" \n");
    w(b"dim_info   ");
    w(&[h.0[39]]);
    w(b" \n");
    for n in 0..8 {
        w(format!("dim[{n}] {}\n", h.dim(n)).as_bytes());
    }
    w(format!("intent_p1  {} \n", f(h.f32_at(56))).as_bytes());
    w(format!("intent_p2  {} \n", f(h.f32_at(60))).as_bytes());
    w(format!("intent_p3  {} \n", f(h.f32_at(64))).as_bytes());
    w(format!("intent_code  {} \n", h.intent_code()).as_bytes());
    w(format!("datatype      {} \n", h.datatype()).as_bytes());
    w(format!("bitpix        {} \n", h.i16_at(72)).as_bytes());
    w(format!("slice_start   {} \n", h.i16_at(74)).as_bytes());
    for n in 0..8 {
        w(format!("pixdim[{n}] {}\n", f(h.pixdim(n))).as_bytes());
    }
    w(format!("vox_offset    {}\n", f(h.vox_offset())).as_bytes());
    w(format!("scl_slope     {}\n", f(h.scl_slope())).as_bytes());
    w(format!("scl_inter     {}\n", f(h.scl_inter())).as_bytes());
    w(format!("slice_end     {}\n", h.i16_at(120)).as_bytes());
    w(b"slice_code    ");
    w(&[h.slice_code()]);
    w(b"\n");
    w(b"xyzt_units    ");
    w(&[h.xyzt_units()]);
    w(b"\n");
    w(format!("cal_max       {}\n", f(h.f32_at(124))).as_bytes());
    w(format!("cal_min       {}\n", f(h.f32_at(128))).as_bytes());
    w(format!("slice_duration {}\n", f(h.slice_duration())).as_bytes());
    w(format!("toffset        {} \n", f(h.f32_at(136))).as_bytes());
    w(format!("glmax          {} \n", h.i32_at(140)).as_bytes());
    w(format!("glmin          {} \n", h.glmin()).as_bytes());
    let t = cstr(g, 148);
    w(b"descrip        ");
    w(&t);
    w(b" \n");
    let t = cstr(g, 228);
    w(b"aux_file       ");
    w(&t);
    w(b" \n");
    w(format!("qform_code     {} \n", h.qform_code()).as_bytes());
    w(format!("sform_code     {} \n", h.sform_code()).as_bytes());
}

/// `niftiQformToMri(mri, hdr)`: the direction cosines from the quaternion (`quatern_to_mat44`'s
/// steps; the third column negated when `pixdim[0]` is negative) and the centre from the offsets,
/// in the official build's operations and operand order. `NO_ERROR`.
fn nifti_qform_to_mri(m: &mut Mri, h: &NiftiHdr) -> i32 {
    use sse::{add, add64, div, mul, mul64, sub, sub64};
    let (mut b, mut c, mut d) = (h.f32_at(256), h.f32_at(260), h.f32_at(264));
    let s = add(add(mul(b, b), mul(c, c)), mul(d, d));
    let a1 = sub(1.0, s);
    // `a < 1.0e-7` false for a NaN
    let (a2, a2f, aa);
    if widen(a1) < 1.0e-7 {
        let inv = div(1.0, sse::sqrt(s));
        b = mul(b, inv);
        c = mul(c, inv);
        d = mul(d, inv);
        a2 = 0.0f64;
        a2f = 0.0f32;
        aa = 0.0f32;
    } else {
        let a = sse::sqrt(a1);
        aa = mul(a, a);
        a2f = add(a, a);
        a2 = add64(widen(a), widen(a));
    }
    let (bb, cc, dd) = (mul(b, b), mul(c, c), mul(d, d));
    let (bd, cd, dd64) = (widen(b), widen(c), widen(d));
    let b2 = add64(bd, bd);
    let ab2 = mul64(bd, a2);
    let r11 = sub(sub(add(aa, bb), cc), dd);
    let ad2 = mul64(dd64, a2);
    let ac2 = mul64(cd, a2);
    let bc2 = mul64(b2, cd);
    let c2 = add64(cd, cd);
    let bd2 = mul64(b2, dd64);
    let cd2 = mul64(c2, dd64);
    let r21 = narrow(add64(bc2, ad2));
    let r12 = narrow(sub64(bc2, ad2));
    let mut r33 = sub(sub(add(aa, dd), cc), bb);
    let r22 = sub(sub(add(aa, cc), bb), dd);
    let mut r13 = narrow(add64(ac2, bd2));
    let mut r23 = narrow(sub64(cd2, ab2));
    let r31 = narrow(sub64(bd2, widen(mul(c, a2f))));
    let r32 = narrow(add64(cd2, widen(mul(b, a2f))));
    if h.pixdim(0) < 0.0 {
        r13 = -r13;
        r23 = -r23;
        r33 = -r33;
    }
    let v = &mut m.vg;
    v.x_r = r11;
    v.y_r = r12;
    v.z_r = r13;
    v.x_a = r21;
    v.y_a = r22;
    v.z_a = r23;
    v.x_s = r31;
    v.y_s = r32;
    v.z_s = r33;
    let (w2, h2, d2) = (
        f64::from(v.width) * 0.5,
        f64::from(v.height) * 0.5,
        f64::from(v.depth) * 0.5,
    );
    let (xs, ys, zs) = (v.xsize, v.ysize, v.zsize);
    let yr = mul64(widen(mul(r12, ys)), h2);
    let xr = mul64(widen(mul(r11, xs)), w2);
    let zr = mul64(widen(mul(r13, zs)), d2);
    let xa = mul64(widen(mul(r21, xs)), w2);
    let ya = mul64(widen(mul(r22, ys)), h2);
    let za = mul64(widen(mul(r23, zs)), d2);
    let xsr = mul64(widen(mul(xs, r31)), w2);
    let ysr = mul64(h2, widen(mul(r32, ys)));
    let zsr = mul64(d2, widen(mul(r33, zs)));
    v.c_r = narrow(add64(add64(add64(yr, xr), zr), widen(h.f32_at(268))));
    v.c_a = narrow(add64(add64(za, add64(xa, ya)), widen(h.f32_at(272))));
    v.c_s = narrow(add64(add64(add64(xsr, ysr), zsr), widen(h.f32_at(276))));
    NO_ERROR
}

/// `niftiSformToMri(mri, hdr, nii_sform)`: the sform's rows as a 4 x 4 matrix (the last row 0),
/// printed with `DIAG_INFO`, given to `MRIsetVox2RASFromMatrix`; copied into `nii_sform` with
/// its last element 1.
fn nifti_sform_to_mri(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &mut Mri,
    h: &NiftiHdr,
    nii_sform: Option<&mut Matrix>,
) -> Result<i32, Fatal> {
    let Some(mut sform) = matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)? else {
        return Ok(NO_ERROR);
    };
    for r in 0..3 {
        for c in 0..4 {
            sform.set_relt(r as i32 + 1, c as i32 + 1, h.f32_at(280 + 16 * r + 4 * c));
        }
    }
    if g.gdiag & DIAG_INFO != 0 {
        let _ = out.write_all(b"[DEBUG] niftiSformToMri() sform:\n");
        let mut buf = Vec::new();
        matrix_print(g, Some(&mut buf), out, err, Some(&sform), 10);
        let _ = out.write_all(&buf);
    }
    let e = vol_geom::set_vox2ras_from_matrix(g, out, err, &mut m.vg, &sform)?;
    if let Some(s) = nii_sform {
        *s = sform;
        s.set_relt(4, 4, 1.0);
    }
    Ok(e)
}

/// `__niiReadSetVox2ras(mri, hdr, nii_transform)`: the sform when its code is not 0 (`INFO: using
/// NIfTI-1 sform (sform_code=<c>)`), else the qform when its code is not 0 (`... qform ...`), else
/// FreeSurfer's guess (`WARNING: neither NIfTI-1 qform or sform are valid` and `WARNING: your
/// volume will probably be incorrectly oriented`: LIA-like axes, the centre half the extent), on
/// standard error; the RAS flag 1 for a form, 0 for the guess.
fn nii_read_set_vox2ras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &mut Mri,
    h: &NiftiHdr,
    nii_sform: Option<&mut Matrix>,
) -> Result<bool, Fatal> {
    if h.sform_code() != 0 {
        let _ = writeln!(
            err,
            "INFO: using NIfTI-1 sform (sform_code={})",
            h.sform_code()
        );
        if nifti_sform_to_mri(g, out, err, m, h, nii_sform)? != NO_ERROR {
            return Ok(true);
        }
        m.vg.ras_good_flag = 1;
    } else if h.qform_code() != 0 {
        let _ = writeln!(
            err,
            "INFO: using NIfTI-1 qform (qform_code={})",
            h.qform_code()
        );
        nifti_qform_to_mri(m, h);
        m.vg.ras_good_flag = 1;
    } else {
        let _ = err.write_all(
            b"WARNING: neither NIfTI-1 qform or sform are valid\nWARNING: your volume will probably be incorrectly oriented\n",
        );
        no_form_guess(m);
    }
    Ok(false)
}

/// FreeSurfer's orientation without a form: `x` to the left, `y` anterior, `z` superior, the
/// centre `voxel size x extent / 2` on each axis, the RAS flag 0.
fn no_form_guess(m: &mut Mri) {
    let v = &mut m.vg;
    v.x_r = -1.0;
    v.x_a = 0.0;
    v.x_s = 0.0;
    v.y_r = 0.0;
    v.y_a = 1.0;
    v.y_s = 0.0;
    v.z_r = 0.0;
    v.z_a = 0.0;
    v.z_s = 1.0;
    v.c_r = sse::mul(sse::mul(v.width as f32, v.xsize), 0.5);
    v.c_a = sse::mul(sse::mul(v.height as f32, v.ysize), 0.5);
    v.c_s = sse::mul(sse::mul(v.depth as f32, v.zsize), 0.5);
    v.ras_good_flag = 0;
}

/// `__readFSniiextensionHeader(fp, mri)`: 4 bytes (an endian mark, the intent big-endian, the
/// MGH version); the volume's intent and its version `intent << 8 | version`.
fn read_fs_nii_extension_header(g: &Globals, out: &mut dyn Write, fp: &mut ZnzFile, m: &mut Mri) {
    let mut ext = [0u8; 4];
    fio::fs_znzread(fp, &mut ext, 4, 1);
    let info = g.gdiag & DIAG_INFO != 0;
    if info {
        let _ = writeln!(
            out,
            "[DEBUG] __readFSniiextensionHeader(): {:02x} {:02x} {:02x} {:02x}",
            ext[0], ext[1], ext[2], ext[3]
        );
        let _ = writeln!(
            out,
            "[DEBUG] __readFSniiextensionHeader(): intent swapped byte order ({:02x} {:02x}) => ({:02x} {:02x})",
            ext[1], ext[2], ext[2], ext[1]
        );
    }
    let intent = i32::from(u16::from_be_bytes([ext[1], ext[2]]));
    m.version = (intent << 8) | i32::from(ext[3]);
    m.intent = intent;
    if info {
        let _ = writeln!(
            out,
            "[DEBUG] __readFSniiextensionHeader(): intent = {}, mgh version = {}, mri->version = {}",
            m.intent, ext[3], m.version
        );
    }
}

/// `__niiReadHeaderextension(fp, mri, fname, swapped_flag, &has_ras_xform)`: the extensions
/// after the extender, until FreeSurfer's (code 14: its header and its tags, `MRITAGread` in the
/// extension's form, the RAS flag 1) or one that does not read or has a code outside 0 to 14 or
/// odd (`[WARN] __niiReadHeaderextension(): no valid extension read, rollback bytes read`, the
/// bytes read given back). Returns the geometry the RAS transform tag gives (the volume's
/// geometry otherwise).
#[allow(clippy::too_many_arguments)]
fn nii_read_header_extension(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fp: &mut ZnzFile,
    m: &mut Mri,
    fname: &str,
    swapped: bool,
    vox_offset: i64,
    has_ras_xform: &mut bool,
) -> Result<VolGeom, Fatal> {
    let mut ras_xform = m.vg.clone();
    let info = g.gdiag & DIAG_INFO != 0;
    loop {
        let (mut esize, mut ecode) = (0i32, 0i32);
        let mut b = esize.to_le_bytes();
        let mut count = fio::fs_znzread(fp, &mut b, 4, 1);
        esize = i32::from_le_bytes(b);
        if count == 1 {
            let mut b = ecode.to_le_bytes();
            count += fio::fs_znzread(fp, &mut b, 4, 1);
            ecode = i32::from_le_bytes(b);
        }
        // FS-459: FreeSurfer checks the code before the byte swap
        let check = if swapped && count == 2 {
            g.notices.add(
                &FS_459,
                "niiRead of a big-endian file with an extension: its code checked before the byte swap",
            );
            if g.notices.fix() {
                ecode.swap_bytes()
            } else {
                ecode
            }
        } else {
            ecode
        };
        let valid = !(check < 0 || check > NIFTI_MAX_ECODE || check & 1 != 0);
        if count != 2 || !valid {
            let _ = out.write_all(
                b"[WARN] __niiReadHeaderextension(): no valid extension read, rollback bytes read\n",
            );
            fp.seek(-4 * count, Whence::Cur);
            break;
        }
        if swapped {
            if info {
                let _ = writeln!(
                    out,
                    "[DEBUG] __niiReadHeaderextension(): pre-swap exts: ecode {ecode}, esize {esize}"
                );
            }
            esize = esize.swap_bytes();
            ecode = ecode.swap_bytes();
        }
        if info {
            let _ = writeln!(
                out,
                "[DEBUG] __niiReadHeaderextension(): ecode {ecode}, esize {esize}"
            );
        }
        if ecode != NIFTI_ECODE_FREESURFER {
            // FS-460: FreeSurfer reads the extension's data as the next size and code
            g.notices.add(
                &FS_460,
                format!(
                    "niiRead: an extension of code {ecode} (size {esize}): its data not skipped"
                ),
            );
            if g.notices.fix() {
                let here = fp.tell();
                let next = here.saturating_add(i64::from(esize) - 8);
                if esize < 8 || next + 8 > vox_offset {
                    // no further extension fits before the voxels
                    break;
                }
                fp.seek(next, Whence::Set);
            }
            continue;
        }
        m.vg.ras_good_flag = 1;
        if info {
            let _ = out.write_all(
                b"[DEBUG] __niiReadHeaderextension(): processing NIFTI_ECODE_FREESURFER ...\n",
            );
        }
        let mgztaglen = i64::from(esize) - 12;
        read_fs_nii_extension_header(g, out, fp, m);
        if info {
            let _ = writeln!(
                out,
                "[DEBUG] __niiReadHeaderextension(): version = {}, intent = {} ({})",
                m.version,
                m.intent,
                mri::intent_name(m.intent)
            );
        }
        crate::mriio::mri_tag_read(
            g,
            out,
            err,
            dir,
            m,
            fp,
            fname,
            env,
            true,
            mgztaglen,
            Some(&mut ras_xform),
            Some(has_ras_xform),
        )?;
        break;
    }
    Ok(ras_xform)
}

/// The file's numbers type: the volume's type and the bytes a voxel takes (0 for scaled data:
/// each type's own size in its loop), or the message refusing it.
fn volume_type(
    out: &mut dyn Write,
    err: &mut dyn Write,
    who: &str,
    h: &NiftiHdr,
    scaled: bool,
    doubles: bool,
    name: &str,
) -> Result<(i32, usize), String> {
    let dt = h.datatype();
    if !scaled {
        return match dt {
            DT_UNSIGNED_CHAR => Ok((MRI_UCHAR, 1)),
            DT_SIGNED_SHORT => Ok((MRI_SHORT, 2)),
            DT_UINT16 => {
                let _ = err.write_all(b"INFO: This is an unsigned short.\n");
                Ok((MRI_USHRT, 2))
            }
            DT_SIGNED_INT | DT_UINT32 => Ok((MRI_INT, 4)),
            DT_FLOAT => Ok((MRI_FLOAT, 4)),
            DT_COMPLEX if doubles => Ok((MRI_FLOAT_COMPLEX, 8)),
            DT_DOUBLE if doubles => {
                let _ = out.write_all(
                    b"niiRead(): detected input as 64 bit double, reading in as 32 bit float\n",
                );
                Ok((MRI_FLOAT, 8))
            }
            _ => Err(format!(
                "{who}(): unsupported datatype {dt} (with scl_slope = 0) in {name}"
            )),
        };
    }
    if !matches!(
        dt,
        DT_UNSIGNED_CHAR
            | DT_SIGNED_SHORT
            | DT_SIGNED_INT
            | DT_UINT32
            | DT_FLOAT
            | DT_DOUBLE
            | DT_INT8
            | DT_UINT16
    ) {
        return Err(format!(
            "{who}(): unsupported datatype {dt} (with scl_slope != 0) in {name}"
        ));
    }
    Ok((MRI_FLOAT, 0))
}

/// A reader of the voxels: `znzread` (`niiRead`) or `fread` (`nifti1Read`).
enum Src<'a> {
    Znz(&'a mut ZnzFile),
    C(&'a mut CFile),
}

impl Src<'_> {
    fn read(&mut self, buf: &mut [u8], size: usize, nmemb: usize) -> i64 {
        match self {
            Src::Znz(fp) => fio::fs_znzread(fp, buf, size, nmemb),
            Src::C(fp) => {
                let n = size.saturating_mul(nmemb).min(buf.len());
                fp.fread(&mut buf[..n], size, nmemb) as i64
            }
        }
    }
}

/// The number of voxels of scaled data in `b` as `float`s (each type converted as C converts it).
fn scaled_values(dt: i16, b: &[u8], out: &mut [f32], as_int: bool) {
    match dt {
        DT_UNSIGNED_CHAR => {
            for (o, &v) in out.iter_mut().zip(b) {
                *o = f32::from(v);
            }
        }
        DT_INT8 => {
            for (o, &v) in out.iter_mut().zip(b) {
                *o = f32::from(v as i8);
            }
        }
        DT_SIGNED_SHORT => {
            for (o, v) in out.iter_mut().zip(b.chunks_exact(2)) {
                *o = f32::from(i16::from_le_bytes([v[0], v[1]]));
            }
        }
        DT_UINT16 => {
            for (o, v) in out.iter_mut().zip(b.chunks_exact(2)) {
                *o = f32::from(u16::from_le_bytes([v[0], v[1]]));
            }
        }
        DT_SIGNED_INT | DT_UINT32 => {
            for (o, v) in out.iter_mut().zip(b.chunks_exact(4)) {
                let u = u32::from_le_bytes([v[0], v[1], v[2], v[3]]);
                *o = if dt == DT_SIGNED_INT || as_int {
                    u as i32 as f32
                } else {
                    u as f32
                };
            }
        }
        DT_FLOAT => {
            for (o, v) in out.iter_mut().zip(b.chunks_exact(4)) {
                *o = f32::from_le_bytes([v[0], v[1], v[2], v[3]]);
            }
        }
        _ => {
            for (o, v) in out.iter_mut().zip(b.chunks_exact(8)) {
                *o = narrow(f64::from_le_bytes([
                    v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7],
                ]));
            }
        }
    }
}

/// The bytes a scaled voxel of type `dt` takes.
fn scaled_size(dt: i16) -> usize {
    match dt {
        DT_UNSIGNED_CHAR | DT_INT8 => 1,
        DT_SIGNED_SHORT | DT_UINT16 => 2,
        DT_DOUBLE => 8,
        _ => 4,
    }
}

/// What the voxel loops of the two readers do differently.
struct Reader<'a> {
    who: &'static str,
    name: &'a str,
    /// `niiRead`: the `ERROR: Read <n>, expected <w>` line before the failure.
    count_line: bool,
}

/// The voxels, row by row (frames, slices, rows), into `m`: unscaled rows into the volume's
/// buffer (byte-swapped by their size; `niiRead`'s `double`s through a `double` row; FS-455,
/// FS-456), or scaled rows `value * slope + intercept` in `float` (FS-454 for unsigned 32-bit
/// data). `Err(())` after the failure's messages.
#[allow(clippy::too_many_arguments)]
fn read_voxels(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &mut Src<'_>,
    m: &mut Mri,
    h: &NiftiHdr,
    rd: &Reader<'_>,
    scaled: bool,
    bpv: usize,
    swapped: bool,
) -> Result<(), ()> {
    let (w, hh, d, nf) = (
        m.vg.width.max(0) as usize,
        m.vg.height.max(0) as usize,
        m.vg.depth.max(0) as usize,
        m.nframes.max(0) as usize,
    );
    let rows = nf * d * hh;
    let dt = h.datatype();
    let fail = |g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write, n: i64| {
        if rd.count_line {
            let _ = writeln!(out, "ERROR: Read {n}, expected {w}");
        }
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            &format!("{}(): error reading from {}", rd.who, rd.name),
        );
    };
    let row_len = w * m.bytes_per_vox;
    if !scaled {
        let mut buf = vec![0u8; w * bpv];
        if swapped && bpv == 8 {
            if dt == DT_DOUBLE {
                g.notices.add(
                    &FS_455,
                    "niiRead of big-endian doubles: each value's halves swapped apart (FreeSurfer)",
                );
            } else {
                g.notices.add(
                    &FS_456,
                    "niiRead of big-endian complex values: not byte-swapped (FreeSurfer)",
                );
            }
        }
        let fix = g.notices.fix();
        let mut high_uint32 = false;
        for row in 0..rows {
            let n = src.read(&mut buf, bpv, w);
            if n != w as i64 {
                fail(g, out, err, n);
                return Err(());
            }
            if swapped {
                match bpv {
                    2 => buf.chunks_exact_mut(2).for_each(|c| c.reverse()),
                    4 => buf.chunks_exact_mut(4).for_each(|c| c.reverse()),
                    8 if dt == DT_DOUBLE => {
                        let k = if fix { 8 } else { 4 };
                        buf.chunks_exact_mut(k).for_each(|c| c.reverse());
                    }
                    8 if fix => buf.chunks_exact_mut(4).for_each(|c| c.reverse()),
                    _ => {}
                }
            }
            let Some(chunk) = m.chunk.as_mut() else {
                continue;
            };
            let o = row * row_len;
            if dt == DT_DOUBLE && bpv == 8 {
                for (i, v) in buf.chunks_exact(8).enumerate() {
                    let x = narrow(f64::from_le_bytes([
                        v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7],
                    ]));
                    chunk[o + 4 * i..o + 4 * i + 4].copy_from_slice(&x.to_le_bytes());
                }
            } else {
                chunk[o..o + row_len].copy_from_slice(&buf[..row_len]);
                if dt == DT_UINT32 {
                    high_uint32 |= buf.chunks_exact(4).any(|c| c[3] & 0x80 != 0);
                }
            }
        }
        if high_uint32 {
            g.notices.add(
                &FS_457,
                "a NIfTI volume of unsigned 32-bit values above 2^31 - 1: read as negative ints (FreeSurfer's int volume)",
            );
        }
        return Ok(());
    }
    let (slope, inter) = (h.scl_slope(), h.scl_inter());
    // the type's loops in FreeSurfer's order: unsigned 32-bit data run the int loop and their own
    let passes: &[bool] = if dt == DT_UINT32 {
        g.notices.add(
            &FS_454,
            "a scaled NIfTI volume of unsigned 32-bit values: FreeSurfer reads the data twice",
        );
        if g.notices.fix() {
            &[false]
        } else {
            &[true, false]
        }
    } else {
        &[false]
    };
    let size = scaled_size(dt);
    let mut buf = vec![0u8; w * size];
    let mut vals = vec![0f32; w];
    for &as_int in passes {
        for row in 0..rows {
            let n = src.read(&mut buf, size, w);
            if n != w as i64 {
                g.errno = 0;
                log::error_printf(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("{}(): error reading from {}", rd.who, rd.name),
                );
                return Err(());
            }
            if swapped && size > 1 {
                buf.chunks_exact_mut(size).for_each(|c| c.reverse());
            }
            scaled_values(dt, &buf, &mut vals, as_int);
            let Some(chunk) = m.chunk.as_mut() else {
                continue;
            };
            let o = row * row_len;
            for (i, &v) in vals.iter().enumerate() {
                let p = if dt == DT_FLOAT {
                    sse::mul(slope, v)
                } else {
                    sse::mul(v, slope)
                };
                let x = sse::add(p, inter);
                chunk[o + 4 * i..o + 4 * i + 4].copy_from_slice(&x.to_le_bytes());
            }
        }
    }
    Ok(())
}

/// The space and time units' factors (to mm and ms), or the message refusing the space unit.
/// `None` for the time factor: unknown (0).
fn units(h: &NiftiHdr) -> (Option<f32>, Option<f32>) {
    let space = match h.xyzt_units() & 0x07 {
        NIFTI_UNITS_METER => Some(1000.0),
        NIFTI_UNITS_MM | 0 => Some(1.0),
        NIFTI_UNITS_MICRON => Some(0.001),
        _ => None,
    };
    let time = match h.xyzt_units() & 0x38 {
        NIFTI_UNITS_SEC => Some(1000.0),
        NIFTI_UNITS_MSEC => Some(1.0),
        NIFTI_UNITS_USEC => Some(0.001),
        _ => None,
    };
    (space, time)
}

/// The slice-timing check: a pattern other than 1 to 6 with a slice dimension and a positive
/// duration is refused.
fn slice_code_refused(h: &NiftiHdr) -> bool {
    let code = h.slice_code();
    code != 0 && (h.0[39] >> 4) & 0x03 != 0 && h.slice_duration() > 0.0 && !(1..=6).contains(&code)
}

/// The extents FreeSurfer gives the volume: `dim[1]` (or `glmin` when it is not positive: the
/// column count FreeSurfer writes for 32,768 columns or more; FS-643), `dim[2]`, `dim[3]`
/// (FS-458: 1 past `dim[0]` by default).
fn extents(g: &mut Globals, who: &str, h: &NiftiHdr, swapped: bool) -> (i32, i32, i32) {
    let ncols = if h.dim(1) > 0 {
        i32::from(h.dim(1))
    } else if swapped {
        g.notices.add(
            &FS_643,
            format!("{who} of a big-endian header without a column count: glmin left unswapped"),
        );
        if g.notices.fix() {
            h.glmin().swap_bytes()
        } else {
            h.glmin()
        }
    } else {
        h.glmin()
    };
    let n = h.dim(0);
    let mut e = [ncols, i32::from(h.dim(2)), i32::from(h.dim(3))];
    for (i, x) in e.iter_mut().enumerate().skip(1) {
        if i as i16 >= n && *x != 1 {
            g.notices.add(
                &FS_458,
                format!(
                    "{who}: dim[{}] = {} past the header's {} dimensions, used as the extent",
                    i + 1,
                    *x,
                    n
                ),
            );
            if g.notices.fix() {
                *x = 1;
            }
        }
    }
    (e[0], e[1], e[2])
}

/// `niiRead(fname, read_volume)`: a NIfTI-1 file (`n+1`; compressed when the name ends in `z`).
/// See `specs/freesurfer-io.md` §97.
pub fn nii_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    let use_compression = fname.as_bytes().last() == Some(&b'z');
    if g.gdiag_no > 0 {
        let _ = writeln!(
            out,
            "niiRead: use_compression = {}",
            i32::from(use_compression)
        );
    }
    let ret = |g: &mut Globals, err: &mut dyn Write, code: i32, msg: String, clear: bool| {
        if clear {
            g.errno = 0;
        }
        log::error_printf(g, err, code, &msg);
        Ok(None)
    };
    let path = utils::resolve(dir, fname);
    let Some(mut fp) = ZnzFile::open(path.as_os_str().as_encoded_bytes(), b"r", use_compression)
    else {
        return ret(
            g,
            err,
            ERROR_BADFILE,
            format!("niiRead(): error opening file {fname}"),
            true,
        );
    };
    let mut h = NiftiHdr([0u8; 348]);
    if fio::fs_znzread(&mut fp, &mut h.0, 348, 1) != 1 {
        return ret(
            g,
            err,
            ERROR_BADFILE,
            format!("niiRead(): error reading header from {fname}"),
            true,
        );
    }
    let mut swapped = false;
    if !(1..=7).contains(&h.dim(0)) {
        swapped = true;
        swap_nifti_1_header(&mut h);
        if !(1..=7).contains(&h.dim(0)) {
            return ret(
                g,
                err,
                ERROR_BADFILE,
                format!(
                    "niiRead(): bad number of dimensions ({}) in {fname}",
                    h.dim(0)
                ),
                false,
            );
        }
    }
    if h.magic() != b"n+1\0" {
        return ret(
            g,
            err,
            ERROR_BADFILE,
            format!("niiRead(): bad magic number in {fname}"),
            false,
        );
    }
    if !(1..=5).contains(&h.dim(0)) {
        return ret(
            g,
            err,
            ERROR_UNSUPPORTED,
            format!("niiRead(): {} dimensions in {fname}; unsupported", h.dim(0)),
            false,
        );
    }
    if h.datatype() == 0 {
        return ret(
            g,
            err,
            ERROR_UNSUPPORTED,
            format!("niiRead(): unknown or no data type in {fname}; bailing out"),
            false,
        );
    }
    if h.dim(4) == 0 {
        let _ = out.write_all(b"WARNING: hdr.dim[4] = 0 (nframes), setting to 1\n");
        h.set_i16(48, 1);
    }
    let (space, time) = units(&h);
    let Some(space_factor) = space else {
        return ret(
            g,
            err,
            ERROR_BADFILE,
            format!(
                "niiRead(): unknown space units {} in {fname}",
                h.xyzt_units() & 0x07
            ),
            false,
        );
    };
    let time_factor = match time {
        Some(t) => t,
        None => {
            if h.dim(4) > 1 {
                let _ = writeln!(
                    out,
                    "WARNING: niiRead(): unknown time units {} in {fname}",
                    h.xyzt_units() & 0x38
                );
            }
            0.0
        }
    };
    if slice_code_refused(&h) {
        return ret(
            g,
            err,
            ERROR_UNSUPPORTED,
            format!(
                "niiRead(): unsupported slice timing pattern {} in {fname}",
                h.slice_code()
            ),
            false,
        );
    }
    let mut nslices: i32 = if h.dim(0) < 4 { 1 } else { i32::from(h.dim(4)) };
    if h.dim(0) > 4 && h.dim(5) > 0 {
        nslices = nslices.wrapping_mul(i32::from(h.dim(5)));
    }
    if g.gdiag_no > 0 {
        let _ = writeln!(
            out,
            "niiRead(): hdr.dim {} {} {} {} {} {}",
            h.dim(0),
            h.dim(1),
            h.dim(2),
            h.dim(3),
            h.dim(4),
            h.dim(5)
        );
    }
    let (slope, inter) = (h.scl_slope(), h.scl_inter());
    let scaled = slope != 0.0 && !(slope == 1.0 && inter == 0.0);
    let (fs_type, bpv) = match volume_type(out, err, "niiRead", &h, scaled, true, fname) {
        Ok(t) => t,
        Err(msg) => return ret(g, err, ERROR_UNSUPPORTED, msg, false),
    };
    let (ncols, d2, d3) = extents(g, "niiRead", &h, swapped);
    let is_ico7 = ncols.wrapping_mul(d2).wrapping_mul(d3) == 163842;
    if h.intent_code() == NIFTI_INTENT_DISPVECT {
        let _ = writeln!(
            out,
            "[INFO] niiRead(): intent_code = {}, dimensions = {{{}, {}, {}, {}, {}, {}, {}, {}}}",
            h.intent_code(),
            h.dim(0),
            h.dim(1),
            h.dim(2),
            h.dim(3),
            h.dim(4),
            h.dim(5),
            h.dim(6),
            h.dim(7)
        );
    }
    let mut m = if read_volume {
        mri::mri_alloc_sequence(g, out, err, ncols, d2, d3, fs_type, nslices)?
    } else {
        let mut m = if is_ico7 {
            mri::mri_alloc_header(g, out, err, 163842, 1, 1, fs_type, nslices)?
        } else {
            mri::mri_alloc_header(g, out, err, ncols, d2, d3, fs_type, nslices)?
        };
        m.nframes = nslices;
        m
    };
    m.vg.xsize = h.pixdim(1);
    m.vg.ysize = h.pixdim(2);
    m.vg.zsize = h.pixdim(3);
    m.tr = h.pixdim(4);
    let vox_offset = float_to_long(h.vox_offset());
    let info = g.gdiag & DIAG_INFO != 0;
    if info {
        let here = fp.tell();
        let _ = writeln!(
            out,
            "[DEBUG] niiRead(): hdr.vox_offset = {vox_offset}, file position = {here}"
        );
    }
    let mut nii_sform = matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)?;
    if nii_read_set_vox2ras(g, out, err, &mut m, &h, nii_sform.as_mut())? {
        return Ok(None);
    }
    if info {
        vol_geom::geomprint(
            out,
            &m.vg,
            Some("[DEBUG] niiRead(): geom from Nifti sform/qform:\n"),
        );
    }
    let mut ext = [0u8; 4];
    let got = fio::fs_znzread(&mut fp, &mut ext, 1, 4);
    if got < 4 && ext[0] != 1 {
        // FS-641: the extender's bytes the file did not give are memory contents
        g.notices.add(
            &FS_641,
            format!(
                "niiRead: the extender read {got} of 4 bytes (its first byte a stack byte in FreeSurfer)"
            ),
        );
    }
    if ext[0] == 1 {
        if info {
            let _ = out.write_all(b"[DEBUG] niiRead(): processing extension ...\n");
        }
        let mut has = false;
        let ras = nii_read_header_extension(
            g, out, err, env, dir, &mut fp, &mut m, fname, swapped, vox_offset, &mut has,
        )?;
        if has {
            let debug = info || env.get("TAG_RAS_XFORM_DEBUG").is_some();
            let mut differ = false;
            if !is_ico7 {
                let mut thresh = g.vg_is_equal_threshold;
                if let Some(v) = env.get("VOL_GEOM_THRESH") {
                    thresh = cstd::atof(v.as_encoded_bytes());
                }
                if debug {
                    let _ = writeln!(
                        out,
                        "[DEBUG] niiRead(): geometry check, thresh={} (ras_xform vs sform/qform) ...",
                        cfmt::g(thresh, 6)
                    );
                    if vol_geom::checkgeom(g, out, err, &ras, &m.vg, thresh, debug)? {
                        vol_geom::geomprint(
                            out,
                            &m.vg,
                            Some(&format!(
                                "[WARNING] niiRead({fname}): vol geom from Nifti sform/qform:\n"
                            )),
                        );
                        vol_geom::geomprint(
                            out,
                            &ras,
                            Some(&format!(
                                "[WARNING] niiRead({fname}): vol geom from TAG_RAS_XFORM in FS header extension:\n"
                            )),
                        );
                    }
                    let _ = writeln!(
                        out,
                        "[DEBUG] niiRead(): volume vox2ras check, thresh={} (ras_xform vs sform/qform) ...",
                        cfmt::g(thresh, 6)
                    );
                }
                differ = vol_geom::checkvox2ras(
                    g,
                    out,
                    err,
                    &ras,
                    &m.vg,
                    thresh,
                    debug,
                    nii_sform.as_ref(),
                )?;
                if differ && debug {
                    let t = cfmt::g(g.vg_is_equal_threshold, 6);
                    let _ = writeln!(
                        out,
                        "[WARNING] niiRead({fname}): VOX2RAS differs - TAG_RAS_XFORM vs Nifti sform/qform in FS header extension (thresh={t})"
                    );
                    let _ = writeln!(
                        out,
                        "[WARNING] niiRead({fname}): ignore TAG_RAS_XFORM in FS header extension, use Nifti sform/qform"
                    );
                }
            }
            if !differ {
                if debug {
                    let _ = out.write_all(
                        b"[INFO] niiRead(): update vol geom with TAG_RAS_XFORM in FS header extension\n",
                    );
                }
                update_ras_xform(&mut m.vg, &ras);
            }
        }
    } else if h.intent_code() == NIFTI_INTENT_DISPVECT {
        m.intent = MGZ_INTENT_WARPMAP;
        m.warp_field_format = WARPFIELD_DTFMT_DISP_RAS;
        m.version = ((MGZ_INTENT_WARPMAP & 0xffff) << 8) | MGH_VERSION;
        let _ = writeln!(
            out,
            "[INFO] niiRead(): (no header extension) NIFTI_INTENT_DISPVECT => MGZ_INTENT_WARPMAP, mri->version = {}, mri->intent = {} ({})",
            m.version,
            m.intent,
            mri::intent_name(m.intent)
        );
    }
    apply_units(&mut m, space_factor, time_factor, true);
    if g.gdiag_no > 0 {
        let _ = out.write_all(b"nifti header ---------------------------------\n");
        nii_print_hdr(g, out, &h);
        let _ = out.write_all(b"-----------------------------------------\n");
    }
    if !read_volume {
        return Ok(Some(m));
    }
    if info {
        let _ = writeln!(out, "[DEBUG] niiRead(): hdr.vox_offset = {vox_offset}");
    }
    if fp.seek(vox_offset, Whence::Set) == -1 {
        return ret(
            g,
            err,
            ERROR_BADFILE,
            format!("niiRead(): error finding voxel data in {fname}"),
            true,
        );
    }
    if info {
        let here = fp.tell();
        let _ = writeln!(
            out,
            "[DEBUG] niiRead(): zeek to image data, after znzseek({vox_offset}), file position = {here}"
        );
    }
    let rd = Reader {
        who: "niiRead",
        name: fname,
        count_line: true,
    };
    if read_voxels(
        g,
        out,
        err,
        &mut Src::Znz(&mut fp),
        &mut m,
        &h,
        &rd,
        scaled,
        bpv,
        swapped,
    )
    .is_err()
    {
        return Ok(None);
    }
    drop(fp);
    if is_ico7 {
        let nf = m.nframes;
        return Ok(Some(crate::mriio_write::reshape(
            g, out, err, &m, 163842, 1, 1, nf,
        )?));
    }
    Ok(Some(m))
}

/// `MRI::update_ras_xform(vg)`: the direction cosines, centre and shears of `vg`.
fn update_ras_xform(v: &mut VolGeom, r: &VolGeom) {
    v.x_r = r.x_r;
    v.x_a = r.x_a;
    v.x_s = r.x_s;
    v.y_r = r.y_r;
    v.y_a = r.y_a;
    v.y_s = r.y_s;
    v.z_r = r.z_r;
    v.z_a = r.z_a;
    v.z_s = r.z_s;
    v.c_r = r.c_r;
    v.c_a = r.c_a;
    v.c_s = r.c_s;
    v.s_r = r.s_r;
    v.s_a = r.s_a;
    v.s_s = r.s_s;
}

/// The units applied: voxel sizes and centre times the space factor, TR times the time factor
/// (`tr` false: `nifti1Read` of a header that is not 4-D leaves TR); with `extent` (`niiRead`) the
/// field of view and each axis's start and end from the voxel size.
fn apply_units(m: &mut Mri, space: f32, time: f32, extent: bool) {
    use sse::mul;
    let v = &mut m.vg;
    v.xsize = mul(v.xsize, space);
    v.ysize = mul(v.ysize, space);
    v.zsize = mul(v.zsize, space);
    v.c_r = mul(v.c_r, space);
    v.c_a = mul(v.c_a, space);
    v.c_s = mul(space, v.c_s);
    m.tr = mul(time, m.tr);
    if extent {
        let xw = mul(v.width as f32, v.xsize);
        let yh = mul(v.height as f32, v.ysize);
        let zd = mul(v.depth as f32, v.zsize);
        m.fov = xw;
        m.xend = mul(xw, 0.5);
        m.xstart = mul(-xw, 0.5);
        m.yend = mul(yh, 0.5);
        m.ystart = mul(-yh, 0.5);
        m.zstart = mul(-zd, 0.5);
        m.zend = mul(zd, 0.5);
    }
}

/// `nifti1Read(fname, read_volume)`: a NIfTI-1 pair (`ni1`): the header from `<stem>.hdr`, the
/// voxels from `<stem>.img` (`<stem>` the name without `.img` or `.hdr`). See
/// `specs/freesurfer-io.md` §98.
pub fn nifti1_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    let ret = |g: &mut Globals, err: &mut dyn Write, code: i32, msg: String, clear: bool| {
        if clear {
            g.errno = 0;
        }
        log::error_printf(g, err, code, &msg);
        Ok(None)
    };
    let mut stem = fname.as_bytes().to_vec();
    if let Some(dot) = stem.iter().rposition(|&c| c == b'.')
        && (&stem[dot..] == b".img" || &stem[dot..] == b".hdr")
    {
        stem.truncate(dot);
    }
    let named = |g: &mut Globals, err: &mut dyn Write, ext: &[u8], line: u32| -> String {
        let mut n = stem.clone();
        n.extend_from_slice(ext);
        if n.len() >= STRLEN {
            // FS-453: snprintf into a 1,024-byte buffer
            g.notices.add(
                &FS_453,
                format!(
                    "nifti1Read: the name {} bytes long cut to 1,023 (FreeSurfer's buffer)",
                    n.len()
                ),
            );
            if g.notices.reproduce() {
                let _ = writeln!(err, "nifti1Read: Truncation on line {line}");
                n.truncate(STRLEN - 1);
            }
        }
        String::from_utf8_lossy(&n).into_owned()
    };
    let hdr_fname = named(g, err, b".hdr", 7704);
    let img_fname = named(g, err, b".img", 7708);
    let mut fp = match CFile::open(dir, &hdr_fname, "r") {
        Ok(f) => f,
        Err(_) => {
            return ret(
                g,
                err,
                ERROR_BADFILE,
                format!("nifti1Read(): error opening file {hdr_fname}"),
                true,
            );
        }
    };
    let mut h = NiftiHdr([0u8; 348]);
    if fp.fread(&mut h.0, 348, 1) != 1 {
        return ret(
            g,
            err,
            ERROR_BADFILE,
            format!("nifti1Read(): error reading header from {hdr_fname}"),
            true,
        );
    }
    fp.close();
    let mut swapped = false;
    if !(1..=7).contains(&h.dim(0)) {
        swapped = true;
        swap_nifti_1_header(&mut h);
        if !(1..=7).contains(&h.dim(0)) {
            return ret(
                g,
                err,
                ERROR_BADFILE,
                format!(
                    "nifti1Read(): bad number of dimensions ({}) in {hdr_fname}",
                    h.dim(0)
                ),
                false,
            );
        }
    }
    if h.magic() != b"ni1\0" {
        return ret(
            g,
            err,
            ERROR_BADFILE,
            format!("nifti1Read(): bad magic number in {hdr_fname}"),
            false,
        );
    }
    if !(2..=4).contains(&h.dim(0)) {
        return ret(
            g,
            err,
            ERROR_UNSUPPORTED,
            format!(
                "nifti1Read(): {} dimensions in {hdr_fname}; unsupported",
                h.dim(0)
            ),
            false,
        );
    }
    if h.datatype() == 0 {
        return ret(
            g,
            err,
            ERROR_UNSUPPORTED,
            format!("nifti1Read(): unknown or no data type in {hdr_fname}; bailing out"),
            false,
        );
    }
    let (space, time) = units(&h);
    let Some(space_factor) = space else {
        return ret(
            g,
            err,
            ERROR_BADFILE,
            format!(
                "nifti1Read(): unknown space units {} in {hdr_fname}",
                h.xyzt_units() & 0x07
            ),
            false,
        );
    };
    let time_factor = match time {
        Some(t) => t,
        None => {
            if h.dim(4) > 1 {
                return ret(
                    g,
                    err,
                    ERROR_BADFILE,
                    format!(
                        "nifti1Read(): unknown time units {} in {hdr_fname}",
                        h.xyzt_units() & 0x38
                    ),
                    false,
                );
            }
            0.0
        }
    };
    if slice_code_refused(&h) {
        return ret(
            g,
            err,
            ERROR_UNSUPPORTED,
            format!(
                "nifti1Read(): unsupported slice timing pattern {} in {hdr_fname}",
                h.slice_code()
            ),
            false,
        );
    }
    let nslices: i32 = if h.dim(0) < 4 { 1 } else { i32::from(h.dim(4)) };
    let (slope, inter) = (h.scl_slope(), h.scl_inter());
    let scaled = slope != 0.0 && !(slope == 1.0 && inter == 0.0);
    let (fs_type, bpv) = match volume_type(out, err, "nifti1Read", &h, scaled, false, &hdr_fname) {
        Ok(t) => t,
        Err(msg) => return ret(g, err, ERROR_UNSUPPORTED, msg, false),
    };
    let (ncols, d2, d3) = extents(g, "nifti1Read", &h, swapped);
    let is_ico7 = ncols.wrapping_mul(d2).wrapping_mul(d3) == 163842;
    let mut m = if read_volume {
        mri::mri_alloc_sequence(g, out, err, ncols, d2, d3, fs_type, nslices)?
    } else {
        let mut m = if is_ico7 {
            mri::mri_alloc_header(g, out, err, 163842, 1, 1, fs_type, nslices)?
        } else {
            mri::mri_alloc_header(g, out, err, ncols, d2, d3, fs_type, nslices)?
        };
        m.nframes = nslices;
        m
    };
    m.vg.xsize = h.pixdim(1);
    m.vg.ysize = h.pixdim(2);
    m.vg.zsize = h.pixdim(3);
    let four = h.dim(0) == 4;
    if four {
        m.tr = h.pixdim(4);
    }
    if h.sform_code() != 0 {
        let _ = err.write_all(b"INFO: using NIfTI-1 sform \n");
        if nifti_sform_to_mri(g, out, err, &mut m, &h, None)? != NO_ERROR {
            return Ok(None);
        }
        m.vg.ras_good_flag = 1;
    } else if h.qform_code() != 0 {
        let _ = err.write_all(b"INFO: using NIfTI-1 qform \n");
        nifti_qform_to_mri(&mut m, &h);
        m.vg.ras_good_flag = 1;
    } else {
        let _ = out.write_all(
            b"WARNING: neither NIfTI-1 qform or sform are valid\nWARNING: your volume will probably be incorrectly oriented\n",
        );
        no_form_guess(&mut m);
    }
    let tr = m.tr;
    apply_units(&mut m, space_factor, time_factor, false);
    if !four {
        m.tr = tr;
    }
    if !read_volume {
        return Ok(Some(m));
    }
    let mut fp = match CFile::open(dir, &img_fname, "r") {
        Ok(f) => f,
        Err(_) => {
            return ret(
                g,
                err,
                ERROR_BADFILE,
                format!("nifti1Read(): error opening file {img_fname}"),
                true,
            );
        }
    };
    let rd = Reader {
        who: "nifti1Read",
        name: &img_fname,
        count_line: false,
    };
    let r = read_voxels(
        g,
        out,
        err,
        &mut Src::C(&mut fp),
        &mut m,
        &h,
        &rd,
        scaled,
        bpv,
        swapped,
    );
    fp.close();
    if r.is_err() {
        return Ok(None);
    }
    if is_ico7 {
        let nf = m.nframes;
        return Ok(Some(crate::mriio_write::reshape(
            g, out, err, &m, 163842, 1, 1, nf,
        )?));
    }
    Ok(Some(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widen_narrow_keep_nan_payloads() {
        let s = f32::from_bits(0x7f80_0001);
        assert_eq!(widen(s).to_bits(), 0x7ff8_0000_2000_0000);
        assert_eq!(narrow(widen(s)).to_bits(), 0x7fc0_0001);
        assert_eq!(
            narrow(f64::from_bits(0xfff0_0000_0000_0001)).to_bits(),
            0xffc0_0000
        );
        assert_eq!(float_to_long(f32::NAN), i64::MIN);
        assert_eq!(float_to_long(352.9), 352);
        assert_eq!(float_to_long(-1.5), -1);
        assert_eq!(float_to_long(1e19), i64::MIN);
    }
}
