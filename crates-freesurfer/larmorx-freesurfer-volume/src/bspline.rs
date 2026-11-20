// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `fs.bspline` (queue 3.2-01): FreeSurfer 8.2.0's `utils/mriBSpline.cpp` (Martin Reuter, MGH),
//! the B-spline coefficients of a volume and the samples, transforms and factor-of-two pyramids
//! made from them.
//!
//! **EPFL's B-splines.** The file follows Philippe Thévenaz, Thierry Blu and Michael Unser,
//! "Interpolation Revisited", IEEE Transactions on Medical Imaging 19(7), 739-758, 2000, and the
//! 2-D example code of EPFL's Biomedical Imaging Group (<http://bigwww.epfl.ch/thevenaz/interpolation/>:
//! `ConvertToInterpolationCoefficients`, `InitialCausalCoefficient`,
//! `InitialAntiCausalCoefficient`, the poles and weights of degrees 2 to 9); its pyramid filters
//! are Michael Unser's (NIH, BEIP, 1992 and 1994), Patrick Brigger's (NIH, 1996) and Daniel Sage's
//! (EPFL, 1999), from <http://bigwww.epfl.ch/sage/pyramids/>. FreeSurfer distributes the file under
//! its licence with MGH's notice; this translation credits EPFL in `PROVENANCE.md` and
//! `crates-freesurfer/NOTICE` (docs/freesurfer/third-party.md, TP-05 and P-10).
//!
//! The functions: `MRIallocBSpline`, `MRIfreeBSpline`, `MRItoBSpline`, `MRIsampleBSpline`,
//! `MRIsampleSeqBSpline`, `MRIlinearTransformBSpline`, `MRIapplyRASlinearTransformBSpline`,
//! `LTAtransformBSpline`, `MRIdownsample2BSpline`, `MRIupsample2BSpline`; and the file's static
//! helpers they reach (the poles, the weights, the line filters, the centred cubic pyramid
//! filter). The other pyramid filters of the file (the standard and L2 families) are static and
//! never chosen: the two pyramid functions ask only for the centred cubic one (not translated).
//!
//! **Arithmetic.** In `double`, in the source's order; the official build (GCC 8.4.0, `-O3`, SSE2,
//! no fused operations) keeps that order and swaps only operands of commutative operations, which
//! decides which of two NaNs survives: those operand orders are read from the unit oracle's
//! `mriBSpline.cpp.o` (`docs/findings/freesurfer-volume.md` §30) and followed with SSE's NaN rules
//! ([`larmorx_freesurfer_core::sse`]). The coefficients are stored as `float` after each of the
//! three passes (x, y, z), as FreeSurfer stores them. The causal initialisation's horizon is
//! `ceil(log(DBL_EPSILON) / log(|z|))` and the full initialisation's first power `pow(z, n - 1)`:
//! the C library's `log` and `pow` ([`Libm`]: correctly rounded by default, glibc 2.35's with
//! `reproduce`, GLIBC-07). For every pole and every line length that reaches them the two give
//! the same bits but one (a unit test checks it): degree 6's third pole on a line of 5 samples,
//! where glibc's `pow(z, 4)` is one unit in the last place above the correctly rounded value; no
//! program asks for degree 6 (FreeSurfer's callers all use 3).
//!
//! **Threads.** FreeSurfer computes the lines of each pass in parallel (OpenMP); every line is
//! independent, so the result does not depend on the threads. The port computes them in order.
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §30):
//! - **FS-962** (rule A): `MRIupsample2BSpline` on an axis of one voxel (a volume one row high or
//!   one slice deep) copies the source's own voxels over the lines the previous pass expanded,
//!   reading the source past its extent on the expanded axes (other voxels of the source or memory
//!   it does not own). The default keeps the expanded lines (the pass is the identity on such an
//!   axis); `reproduce` reads as FreeSurfer (noted).
//! - **FS-963** (rule D, pending `mri_robust_register`'s two-dimensional path, 3.2-13): both
//!   pyramid functions change the voxel size of an axis of one voxel, which they do not resample
//!   (doubled, with the voxel moved half a source voxel, by the downsampling; halved by the
//!   upsampling). Both modes give FreeSurfer's geometry, noted.
//! - **FS-964** (memory): sampling a B-spline whose degree is not 2 to 9 (one that was allocated
//!   and never computed: `MRItoBSpline` refuses other degrees) reads indices never set or writes
//!   past the index arrays where FreeSurfer would not stop first with `Invalid spline degree`; an
//!   error in both modes, but for degrees above 9 on coefficients with an axis longer than one
//!   voxel: FreeSurfer stores a few indices past its arrays and then ends with `Invalid spline
//!   degree`, status 1 (deterministic while those bytes are inside the stack; a crash for degrees
//!   large enough to reach past its top), which `reproduce` gives, noted.

use std::io::Write;

use larmorx_core::defects::{FS_962, FS_963, FS_964};
use larmorx_core::math::Libm;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::matrix_inverse;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse::{self, add64, div64, mul64, sub64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::lta::{
    LINEAR_PHYSVOX_TO_PHYSVOX, LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX, Lta,
};
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_LONG, MRI_RGB, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri,
    mri_alloc_sequence, mri_copy_header, mri_free, use_vol_geom_to_mri,
};
use larmorx_freesurfer_mri::vol_geom::{extract_i_to_r, extract_r_to_i, reinit_cache};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::arith::mri_clear;
use crate::coords::index_not_in_volume;
use crate::frames::mri_clone_different_type;
use crate::ltaops::lta_change_type;
use crate::vgeom::{vg_i_to_r, vg_r_to_i};
use crate::xform::mri_ras_xform_to_voxel_xform;

/// `MRI_BSPLINE`: the degree, the coefficient volume (`float`), the source's type and whether
/// the source had a negative value.
#[derive(Debug)]
pub struct MriBspline {
    pub degree: i32,
    pub coeff: Option<Mri>,
    pub srctype: i32,
    pub srcneg: bool,
}

// ------------------------------------------------------------------------------------------
// The voxels of a line

/// A `float` widened to `double` as `cvtss2sd` does: a signalling NaN comes out quiet. (Written
/// with the bits, so that no build folds a widening and a narrowing into nothing; the porting
/// guide's "a float stored through a double".)
#[inline(always)]
fn wide(v: f32) -> f64 {
    let d = f64::from(v);
    if d.is_nan() {
        f64::from_bits(d.to_bits() | 0x0008_0000_0000_0000)
    } else {
        d
    }
}

/// [`wide`] of a value already widened by `f64::from` (the quiet bit set on a NaN).
#[inline(always)]
fn wide_bits(d: f64) -> f64 {
    if d.is_nan() {
        f64::from_bits(d.to_bits() | 0x0008_0000_0000_0000)
    } else {
        d
    }
}

/// A volume whose voxels are there and laid out as allocated (its header's extent the
/// allocation's), of a type `MRIgetVoxVal` reads as a number: its voxel `(c, r, s, f)` in the
/// extent is element `c + r vpr + s vps + f vpv`, which `MRIgetVoxVal` converts to `float` (and
/// `MRIsetVoxVal` stores exactly, for `float`); other accesses go through those functions.
#[derive(Clone, Copy)]
struct Flat {
    vpr: usize,
    vps: usize,
    vpv: usize,
    ext: [i32; 4],
}

impl Flat {
    fn of(m: &Mri) -> Option<Flat> {
        let size = match m.mtype {
            MRI_UCHAR => 1,
            MRI_SHORT | MRI_USHRT => 2,
            MRI_INT | MRI_RGB | MRI_FLOAT => 4,
            MRI_LONG => 8,
            _ => return None,
        };
        let v = &m.vg;
        let chunk = m.chunk.as_ref()?;
        let same = m.shape.width == i64::from(v.width)
            && m.shape.height == i64::from(v.height)
            && m.shape.depth == i64::from(v.depth)
            && m.shape.nframes == i64::from(m.nframes)
            && [v.width, v.height, v.depth, m.nframes]
                .iter()
                .all(|&e| e > 0);
        if !same {
            return None;
        }
        let total = (v.width as u64)
            .checked_mul(v.height as u64)?
            .checked_mul(v.depth as u64)?
            .checked_mul(m.nframes as u64)?
            .checked_mul(size as u64)?;
        if m.vox_per_row != v.width as u64
            || m.vox_per_slice != m.vox_per_row * v.height as u64
            || m.vox_per_vol != m.vox_per_slice * v.depth as u64
            || total > m.bytes_total
            || total > chunk.len() as u64
        {
            return None;
        }
        Some(Flat {
            vpr: m.vox_per_row as usize,
            vps: m.vox_per_slice as usize,
            vpv: m.vox_per_vol as usize,
            ext: [v.width, v.height, v.depth, m.nframes],
        })
    }

    /// The element of `(c, r, s, f)` when it lies in the extent.
    #[inline(always)]
    fn index(&self, c: i32, r: i32, s: i32, f: i32) -> Option<usize> {
        let e = &self.ext;
        if (c as u32) < e[0] as u32
            && (r as u32) < e[1] as u32
            && (s as u32) < e[2] as u32
            && (f as u32) < e[3] as u32
        {
            Some(c as usize + r as usize * self.vpr + s as usize * self.vps + f as usize * self.vpv)
        } else {
            None
        }
    }
}

/// Element `i` of a flat volume as `MRIgetVoxVal` converts it to `float`.
#[inline(always)]
fn flat_value(m: &Mri, i: usize) -> f32 {
    let b = m.chunk.as_deref().expect("voxels");
    match m.mtype {
        MRI_UCHAR => f32::from(b[i]),
        MRI_SHORT => f32::from(i16::from_le_bytes([b[2 * i], b[2 * i + 1]])),
        MRI_USHRT => f32::from(u16::from_le_bytes([b[2 * i], b[2 * i + 1]])),
        MRI_INT | MRI_RGB => i32::from_le_bytes(b[4 * i..4 * i + 4].try_into().expect("4")) as f32,
        MRI_LONG => i64::from_le_bytes(b[8 * i..8 * i + 8].try_into().expect("8")) as f32,
        _ => f32::from_le_bytes(b[4 * i..4 * i + 4].try_into().expect("4")),
    }
}

/// `MRIgetVoxVal(m, c, r, s, f)` widened to `double`.
#[allow(clippy::too_many_arguments)]
fn get(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &Mri,
    flat: Option<Flat>,
    c: i32,
    r: i32,
    s: i32,
    f: i32,
) -> Result<f64, Fatal> {
    if let Some(i) = flat.and_then(|fl| fl.index(c, r, s, f)) {
        return Ok(wide(flat_value(m, i)));
    }
    Ok(wide(mri_get_vox_val(g, err, m, c, r, s, f, 0)?))
}

/// `MRIsetVoxVal(m, c, r, s, f, v)` (a direct store for a flat `float` volume).
#[allow(clippy::too_many_arguments)]
fn set(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &mut Mri,
    flat: Option<Flat>,
    c: i32,
    r: i32,
    s: i32,
    f: i32,
    v: f32,
) -> Result<(), Fatal> {
    if let Some(i) = flat
        .filter(|_| m.mtype == MRI_FLOAT)
        .and_then(|fl| fl.index(c, r, s, f))
    {
        let b = m.chunk.as_mut().expect("voxels");
        b[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes());
        return Ok(());
    }
    mri_set_vox_val(g, err, m, c, r, s, f, v, 0)?;
    Ok(())
}

/// The axis a line runs along.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
    Z,
}

/// The voxel `k` of the line along `axis` through `(a, b)` (the other two coordinates in the
/// order x, y, z).
#[inline(always)]
fn place(axis: Axis, a: i32, b: i32, k: i32) -> (i32, i32, i32) {
    match axis {
        Axis::X => (k, a, b),
        Axis::Y => (a, k, b),
        Axis::Z => (a, b, k),
    }
}

/// `getXLine`, `getYLine`, `getZLine` (and the `Stop` forms with `n` voxels): the first `n`
/// voxels of the line, widened.
#[allow(clippy::too_many_arguments)]
fn get_line(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &Mri,
    axis: Axis,
    a: i32,
    b: i32,
    f: i32,
    n: i32,
    line: &mut [f64],
) -> Result<(), Fatal> {
    let flat = Flat::of(m);
    for k in 0..n.max(0) {
        let (x, y, z) = place(axis, a, b, k);
        line[k as usize] = get(g, err, m, flat, x, y, z, f)?;
    }
    Ok(())
}

/// The extent of `m` along `axis`.
fn extent(m: &Mri, axis: Axis) -> i32 {
    match axis {
        Axis::X => m.vg.width,
        Axis::Y => m.vg.height,
        Axis::Z => m.vg.depth,
    }
}

/// `setXLine`, `setYLine`, `setZLine`: a line index outside the volume ends the program (`X limit
/// error`, `Y limit error`, `Z limit error`, `Frames limit error` on standard output, status 1);
/// then the line's voxels (the volume's extent along it): a byte volume takes 255 above 255, 0
/// below 0, else `nint` of the value; another type the value as a `float`.
#[allow(clippy::too_many_arguments)]
fn set_line(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &mut Mri,
    axis: Axis,
    a: i32,
    b: i32,
    f: i32,
    line: &[f64],
) -> Result<(), Fatal> {
    let (na, nb) = match axis {
        Axis::X => (("Y", m.vg.height), ("Z", m.vg.depth)),
        Axis::Y => (("X", m.vg.width), ("Z", m.vg.depth)),
        Axis::Z => (("X", m.vg.width), ("Y", m.vg.height)),
    };
    for (v, (name, lim)) in [(a, na), (b, nb), (f, ("Frames", m.nframes))] {
        if v >= lim {
            let _ = writeln!(out, "{name} limit error");
            return Err(Fatal::Exit(1));
        }
    }
    let flat = Flat::of(m);
    for k in 0..extent(m, axis) {
        let v = line[k as usize];
        let (x, y, z) = place(axis, a, b, k);
        let fv = if m.mtype == MRI_UCHAR {
            if v > 255.0 {
                255.0
            } else if v < 0.0 {
                0.0
            } else {
                nint(g, v) as f32
            }
        } else {
            v as f32
        };
        set(g, err, m, flat, x, y, z, f, fv)?;
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------
// EPFL's recursive filter

/// `initPoles(Pole, degree)`: the poles of the B-spline of `degree` (2 to 9); another degree ends
/// the program (`Invalid spline degree` on standard output, status 1). The decimal poles are
/// FreeSurfer's (and EPFL's) 50-digit literals, read to the nearest `double` as C reads them.
#[allow(clippy::excessive_precision)]
fn init_poles(out: &mut dyn Write, degree: i32) -> Result<Vec<f64>, Fatal> {
    Ok(match degree {
        2 => vec![8.0f64.sqrt() - 3.0],
        3 => vec![3.0f64.sqrt() - 2.0],
        4 => vec![
            (664.0 - 438976.0f64.sqrt()).sqrt() + 304.0f64.sqrt() - 19.0,
            (664.0 + 438976.0f64.sqrt()).sqrt() - 304.0f64.sqrt() - 19.0,
        ],
        5 => vec![
            (135.0 / 2.0 - (17745.0f64 / 4.0).sqrt()).sqrt() + (105.0f64 / 4.0).sqrt() - 13.0 / 2.0,
            (135.0 / 2.0 + (17745.0f64 / 4.0).sqrt()).sqrt() - (105.0f64 / 4.0).sqrt() - 13.0 / 2.0,
        ],
        6 => vec![
            -0.488_294_589_303_044_755_130_118_038_883_789_062_112_279_161_239_38,
            -0.081_679_271_076_237_512_597_937_765_737_059_080_653_379_610_398_148,
            -0.001_414_151_808_325_817_751_087_243_976_558_592_527_864_169_055_346_7,
        ],
        7 => vec![
            -0.535_280_430_796_438_165_542_403_781_681_646_071_833_923_152_342_69,
            -0.122_554_615_192_326_690_515_272_264_359_357_343_605_486_549_427_30,
            -0.009_148_694_809_608_276_928_593_021_651_647_853_415_692_563_954_599_4,
        ],
        8 => vec![
            -0.574_686_909_248_765_430_530_139_304_128_745_424_290_661_578_041_25,
            -0.163_035_269_297_280_935_240_551_896_860_737_052_234_768_145_508_30,
            -0.023_632_294_694_844_850_023_403_919_296_361_320_612_665_920_854_629,
            -0.000_153_821_310_641_690_911_739_352_530_184_021_607_629_640_540_700_43,
        ],
        9 => vec![
            -0.607_997_389_168_625_779_007_720_823_954_289_769_439_634_718_539_91,
            -0.201_750_520_193_153_238_796_064_685_055_970_434_680_898_865_757_47,
            -0.043_222_608_540_481_752_133_321_142_979_429_688_265_852_380_231_497,
            -0.002_121_306_903_180_818_420_304_896_557_848_623_422_054_856_098_862_4,
        ],
        _ => {
            let _ = writeln!(out, "Invalid spline degree");
            return Err(Fatal::Exit(1));
        }
    })
}

/// x86-64's `(long) x` (`cvttsd2si`): `LONG_MIN` for NaN and values outside the `long` range.
fn cvtt64(x: f64) -> i64 {
    if x.is_nan() || x >= 9_223_372_036_854_775_808.0 || x < -9_223_372_036_854_775_808.0 {
        i64::MIN
    } else {
        x as i64
    }
}

/// The horizon of the causal initialisation for pole `z`: `ceil(log(DBL_EPSILON) /
/// log(fabs(z)))`.
fn horizon(libm: &Libm, z: f64) -> i64 {
    cvtt64((libm.log(f64::EPSILON) / libm.log(z.abs())).ceil())
}

/// `InitialCausalCoefficient(c, n, z, DBL_EPSILON)` (mirror boundaries).
fn initial_causal(libm: &Libm, c: &[f64], z: f64) -> f64 {
    let n = c.len();
    let hz = horizon(libm, z);
    if hz < n as i64 {
        // the accelerated loop: sum + c[k] * z^k
        let mut zn = z;
        let mut sum = c[0];
        for &ck in &c[1..hz.max(1) as usize] {
            sum = add64(sum, mul64(ck, zn));
            zn *= z;
        }
        sum
    } else {
        // the full loop
        let mut zn = z;
        let iz = 1.0 / z;
        let mut z2n = libm.pow(z, (n - 1) as f64);
        let mut sum = add64(mul64(c[n - 1], z2n), c[0]);
        // `z2n *= z2n * iz` (the official build multiplies `iz * z2n`: the same value)
        z2n = z2n * (iz * z2n);
        for &ck in &c[1..n - 1] {
            sum = add64(sum, mul64(zn + z2n, ck));
            zn *= z;
            z2n *= iz;
        }
        div64(sum, 1.0 - zn * zn)
    }
}

/// `ConvertToInterpolationCoefficients(c, n, poles, npoles, DBL_EPSILON)`: the samples of a line
/// made its interpolation coefficients in place (a line of one sample is left as it is).
fn convert_to_interpolation_coefficients(libm: &Libm, c: &mut [f64], poles: &[f64]) {
    let n = c.len();
    if n <= 1 {
        return;
    }
    let mut lambda = 1.0f64;
    for &z in poles {
        lambda = lambda * (1.0 - z) * (1.0 - 1.0 / z);
    }
    for v in c.iter_mut() {
        *v = mul64(*v, lambda);
    }
    for &z in poles {
        c[0] = initial_causal(libm, c, z);
        for k in 1..n {
            c[k] = add64(mul64(c[k - 1], z), c[k]);
        }
        // InitialAntiCausalCoefficient
        c[n - 1] = mul64(z / (z * z - 1.0), add64(mul64(c[n - 2], z), c[n - 1]));
        for k in (0..n - 1).rev() {
            c[k] = mul64(sub64(c[k + 1], c[k]), z);
        }
    }
}

/// `BsplineWeights(degree, &x, index, weights)`: the `degree + 1` weights of the samples at
/// `index[0..=degree]` for the point `x` (degrees 2 to 9; another ends the program, `Invalid
/// spline degree`). The weights are finite for the points that reach them, so plain arithmetic in
/// the source's order gives the official bits (`x *= x * y` as C writes it: `x * (x * y)`).
#[allow(clippy::misrefactored_assign_op)]
fn bspline_weights(
    out: &mut dyn Write,
    degree: i32,
    x: f64,
    index: &[i32; 10],
    weights: &mut [f64; 10],
) -> Result<(), Fatal> {
    let wt = weights;
    match degree {
        2 => {
            let w = x - f64::from(index[1]);
            wt[1] = 3.0 / 4.0 - w * w;
            wt[2] = (1.0 / 2.0) * (w - wt[1] + 1.0);
            wt[0] = 1.0 - wt[1] - wt[2];
        }
        3 => {
            let w = x - f64::from(index[1]);
            wt[3] = (1.0 / 6.0) * w * w * w;
            wt[0] = (1.0 / 6.0) + (1.0 / 2.0) * w * (w - 1.0) - wt[3];
            wt[2] = w + wt[0] - 2.0 * wt[3];
            wt[1] = 1.0 - wt[0] - wt[2] - wt[3];
        }
        4 => {
            let w = x - f64::from(index[2]);
            let w2 = w * w;
            let t = (1.0 / 6.0) * w2;
            wt[0] = 1.0 / 2.0 - w;
            wt[0] *= wt[0];
            wt[0] *= (1.0 / 24.0) * wt[0];
            let t0 = w * (t - 11.0 / 24.0);
            let t1 = 19.0 / 96.0 + w2 * (1.0 / 4.0 - t);
            wt[1] = t1 + t0;
            wt[3] = t1 - t0;
            wt[4] = wt[0] + t0 + (1.0 / 2.0) * w;
            wt[2] = 1.0 - wt[0] - wt[1] - wt[3] - wt[4];
        }
        5 => {
            let mut w = x - f64::from(index[2]);
            let mut w2 = w * w;
            wt[5] = (1.0 / 120.0) * w * w2 * w2;
            w2 -= w;
            let w4 = w2 * w2;
            w -= 1.0 / 2.0;
            let t = w2 * (w2 - 3.0);
            wt[0] = (1.0 / 24.0) * (1.0 / 5.0 + w2 + w4) - wt[5];
            let t0 = (1.0 / 24.0) * (w2 * (w2 - 5.0) + 46.0 / 5.0);
            let t1 = (-1.0 / 12.0) * w * (t + 4.0);
            wt[2] = t0 + t1;
            wt[3] = t0 - t1;
            let t0 = (1.0 / 16.0) * (9.0 / 5.0 - t);
            let t1 = (1.0 / 24.0) * w * (w4 - w2 - 5.0);
            wt[1] = t0 + t1;
            wt[4] = t0 - t1;
        }
        6 => {
            let w = x - f64::from(index[3]);
            wt[0] = 1.0 / 2.0 - w;
            wt[0] *= wt[0] * wt[0];
            wt[0] *= wt[0] / 720.0;
            wt[1] = (361.0 / 192.0
                - w * (59.0 / 8.0
                    + w * (-185.0 / 16.0
                        + w * (25.0 / 3.0 + w * (-5.0 / 2.0 + w) * (1.0 / 2.0 + w)))))
                / 120.0;
            wt[2] = (10543.0 / 960.0
                + w * (-289.0 / 16.0
                    + w * (79.0 / 16.0 + w * (43.0 / 6.0 + w * (-17.0 / 4.0 + w * (-1.0 + w))))))
                / 48.0;
            let w2 = w * w;
            wt[3] = (5887.0 / 320.0 - w2 * (231.0 / 16.0 - w2 * (21.0 / 4.0 - w2))) / 36.0;
            wt[4] = (10543.0 / 960.0
                + w * (289.0 / 16.0
                    + w * (79.0 / 16.0 + w * (-43.0 / 6.0 + w * (-17.0 / 4.0 + w * (1.0 + w))))))
                / 48.0;
            wt[6] = 1.0 / 2.0 + w;
            wt[6] *= wt[6] * wt[6];
            wt[6] *= wt[6] / 720.0;
            wt[5] = 1.0 - wt[0] - wt[1] - wt[2] - wt[3] - wt[4] - wt[6];
        }
        7 => {
            let w = x - f64::from(index[3]);
            wt[0] = 1.0 - w;
            wt[0] *= wt[0];
            wt[0] *= wt[0] * wt[0];
            wt[0] *= (1.0 - w) / 5040.0;
            let w2 = w * w;
            wt[1] = (120.0 / 7.0
                + w * (-56.0 + w * (72.0 + w * (-40.0 + w2 * (12.0 + w * (-6.0 + w))))))
                / 720.0;
            wt[2] = (397.0 / 7.0
                - w * (245.0 / 3.0
                    + w * (-15.0 + w * (-95.0 / 3.0 + w * (15.0 + w * (5.0 + w * (-5.0 + w)))))))
                / 240.0;
            wt[3] = (2416.0 / 35.0 + w2 * (-48.0 + w2 * (16.0 + w2 * (-4.0 + w)))) / 144.0;
            wt[4] = (1191.0 / 35.0
                - w * (-49.0 + w * (-9.0 + w * (19.0 + w * (-3.0 + w) * (-3.0 + w2)))))
                / 144.0;
            wt[5] = (40.0 / 7.0
                + w * (56.0 / 3.0 + w * (24.0 + w * (40.0 / 3.0 + w2 * (-4.0 + w * (-2.0 + w))))))
                / 240.0;
            wt[7] = w2;
            wt[7] *= wt[7] * wt[7];
            wt[7] *= w / 5040.0;
            wt[6] = 1.0 - wt[0] - wt[1] - wt[2] - wt[3] - wt[4] - wt[5] - wt[7];
        }
        8 => {
            let w = x - f64::from(index[4]);
            wt[0] = 1.0 / 2.0 - w;
            wt[0] *= wt[0];
            wt[0] *= wt[0];
            wt[0] *= wt[0] / 40320.0;
            let w2 = w * w;
            wt[1] = (39.0 / 16.0 - w * (6.0 + w * (-9.0 / 2.0 + w2)))
                * (21.0 / 16.0 + w * (-15.0 / 4.0 + w * (9.0 / 2.0 + w * (-3.0 + w))))
                / 5040.0;
            wt[2] = (82903.0 / 1792.0
                + w * (-4177.0 / 32.0
                    + w * (2275.0 / 16.0
                        + w * (-487.0 / 8.0
                            + w * (-85.0 / 8.0
                                + w * (41.0 / 2.0 + w * (-5.0 + w * (-2.0 + w))))))))
                / 1440.0;
            wt[3] = (310661.0 / 1792.0
                - w * (14219.0 / 64.0
                    + w * (-199.0 / 8.0
                        + w * (-1327.0 / 16.0
                            + w * (245.0 / 8.0
                                + w * (53.0 / 4.0 + w * (-8.0 + w * (-1.0 + w))))))))
                / 720.0;
            wt[4] = (2337507.0 / 8960.0
                + w2 * (-2601.0 / 16.0 + w2 * (387.0 / 8.0 + w2 * (-9.0 + w2))))
                / 576.0;
            wt[5] = (310661.0 / 1792.0
                - w * (-14219.0 / 64.0
                    + w * (-199.0 / 8.0
                        + w * (1327.0 / 16.0
                            + w * (245.0 / 8.0
                                + w * (-53.0 / 4.0 + w * (-8.0 + w * (1.0 + w))))))))
                / 720.0;
            wt[7] = (39.0 / 16.0 - w * (-6.0 + w * (-9.0 / 2.0 + w2)))
                * (21.0 / 16.0 + w * (15.0 / 4.0 + w * (9.0 / 2.0 + w * (3.0 + w))))
                / 5040.0;
            wt[8] = 1.0 / 2.0 + w;
            wt[8] *= wt[8];
            wt[8] *= wt[8];
            wt[8] *= wt[8] / 40320.0;
            wt[6] = 1.0 - wt[0] - wt[1] - wt[2] - wt[3] - wt[4] - wt[5] - wt[7] - wt[8];
        }
        9 => {
            let w = x - f64::from(index[4]);
            wt[0] = 1.0 - w;
            wt[0] *= wt[0];
            wt[0] *= wt[0];
            wt[0] *= wt[0] * (1.0 - w) / 362880.0;
            wt[1] = (502.0 / 9.0
                + w * (-246.0
                    + w * (472.0
                        + w * (-504.0
                            + w * (308.0
                                + w * (-84.0
                                    + w * (-56.0 / 3.0 + w * (24.0 + w * (-8.0 + w)))))))))
                / 40320.0;
            wt[2] = (3652.0 / 9.0
                - w * (2023.0 / 2.0
                    + w * (-952.0
                        + w * (938.0 / 3.0
                            + w * (112.0
                                + w * (-119.0
                                    + w * (56.0 / 3.0 + w * (14.0 + w * (-7.0 + w)))))))))
                / 10080.0;
            wt[3] = (44117.0 / 42.0
                + w * (-2427.0 / 2.0
                    + w * (66.0
                        + w * (434.0
                            + w * (-129.0
                                + w * (-69.0 + w * (34.0 + w * (6.0 + w * (-6.0 + w)))))))))
                / 4320.0;
            let w2 = w * w;
            wt[4] = (78095.0 / 63.0
                - w2 * (700.0 + w2 * (-190.0 + w2 * (100.0 / 3.0 + w2 * (-5.0 + w)))))
                / 2880.0;
            wt[5] = (44117.0 / 63.0
                + w * (809.0
                    + w * (44.0
                        + w * (-868.0 / 3.0
                            + w * (-86.0
                                + w * (46.0 + w * (68.0 / 3.0 + w * (-4.0 + w * (-4.0 + w)))))))))
                / 2880.0;
            wt[6] = (3652.0 / 21.0
                - w * (-867.0 / 2.0
                    + w * (-408.0
                        + w * (-134.0
                            + w * (48.0 + w * (51.0 + w * (-4.0 + w) * (-1.0 + w) * (2.0 + w)))))))
                / 4320.0;
            wt[7] = (251.0 / 18.0
                + w * (123.0 / 2.0
                    + w * (118.0
                        + w * (126.0
                            + w * (77.0
                                + w * (21.0 + w * (-14.0 / 3.0 + w * (-6.0 + w * (-2.0 + w)))))))))
                / 10080.0;
            wt[9] = w2 * w2;
            wt[9] *= wt[9] * w / 362880.0;
            wt[8] = 1.0 - wt[0] - wt[1] - wt[2] - wt[3] - wt[4] - wt[5] - wt[6] - wt[7] - wt[9];
        }
        _ => {
            let _ = writeln!(out, "Invalid spline degree");
            return Err(Fatal::Exit(1));
        }
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------
// Allocation

/// `MRIallocBSpline(width, height, depth, nframes)`: a `float` coefficient volume of that extent
/// (`MRIallocSequence`), degree and source type -1, `srcneg` 0.
#[allow(clippy::too_many_arguments)]
pub fn mri_alloc_bspline(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    width: i32,
    height: i32,
    depth: i32,
    nframes: i32,
) -> Result<MriBspline, Fatal> {
    let coeff = mri_alloc_sequence(g, out, err, width, height, depth, MRI_FLOAT, nframes)?;
    Ok(MriBspline {
        degree: -1,
        coeff: Some(coeff),
        srctype: -1,
        srcneg: false,
    })
}

/// `MRIfreeBSpline(&bspline)`: `NO_ERROR`, `bspline` `None` (its coefficients freed with
/// `MRIfree`, whose `MRIfree: null pointer` error a B-spline without them gives); `ERROR_BADPARM`
/// after `MRIfreeBSpline: null pointer` (as an error) when it is already `None`.
pub fn mri_free_bspline(
    g: &mut Globals,
    err: &mut dyn Write,
    bspline: &mut Option<MriBspline>,
) -> i32 {
    let Some(b) = bspline.as_mut() else {
        log::error_printf(g, err, ERROR_BADPARM, "MRIfreeBSpline: null pointer\n");
        return ERROR_BADPARM;
    };
    mri_free(g, err, &mut b.coeff);
    *bspline = None;
    NO_ERROR
}

/// The coefficient volume `MRItoBSpline` makes: the source's extent and frames in `float`, the
/// source's header and `outside_val`.
fn new_coeff(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
) -> Result<Mri, Fatal> {
    let v = &src.vg;
    let mut c = mri_alloc_sequence(
        g,
        out,
        err,
        v.width,
        v.height,
        v.depth,
        MRI_FLOAT,
        src.nframes,
    )?;
    mri_copy_header(g, out, err, src, Some(&mut c))?;
    c.outside_val = src.outside_val;
    Ok(c)
}

// ------------------------------------------------------------------------------------------
// The coefficients

/// `MRItoBSpline(mri_src, bspline, degree)`: prints `MRItoBSpline degree <d>` on standard output;
/// with no B-spline given (or one without coefficients) a `float` coefficient volume of the
/// source's extent with its header and `outside_val`. The degree, the source's type and
/// `srcneg` (0) are set; coefficients not `float` end the program (`ERROR MRItoBSpline: BSpline
/// Coeff MRI needs to be float!`), of another extent or frames too (`ERROR MRItoBSpline: BSPline
/// Coeff MRI dimensions must be same as Source MRI!`), a degree other than 2 to 9 too (`Invalid
/// spline degree`), each on standard output with status 1. Then each line along x of the source
/// (`MRIgetVoxVal`, every frame) is filtered ([`convert_to_interpolation_coefficients`]; `srcneg`
/// set when a voxel is below 0) and stored as `float`; then, for a volume more than one voxel
/// high, each line along y of the coefficients; then, more than one deep, along z.
pub fn mri_to_bspline(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    bspline: Option<MriBspline>,
    degree: i32,
) -> Result<MriBspline, Fatal> {
    let _ = writeln!(out, "MRItoBSpline degree {degree}");
    let (w, h, d, nf) = (src.vg.width, src.vg.height, src.vg.depth, src.nframes);
    let mut b = match bspline {
        Some(b) => b,
        None => {
            let mut b = mri_alloc_bspline(g, out, err, w, h, d, nf)?;
            let c = b.coeff.as_mut().expect("coefficients");
            mri_copy_header(g, out, err, src, Some(c))?;
            c.outside_val = src.outside_val;
            b
        }
    };
    if b.coeff.is_none() {
        b.coeff = Some(new_coeff(g, out, err, src)?);
    }
    b.degree = degree;
    b.srctype = src.mtype;
    b.srcneg = false;
    let mut coeff = b.coeff.take().expect("coefficients");
    if coeff.mtype != MRI_FLOAT {
        let _ = writeln!(
            out,
            "ERROR MRItoBSpline: BSpline Coeff MRI needs to be float!"
        );
        return Err(Fatal::Exit(1));
    }
    if coeff.vg.width != w || coeff.vg.depth != d || coeff.vg.height != h || coeff.nframes != nf {
        let _ = writeln!(
            out,
            "ERROR MRItoBSpline: BSPline Coeff MRI dimensions must be same as Source MRI!"
        );
        return Err(Fatal::Exit(1));
    }
    let poles = init_poles(out, degree)?;
    let libm = Libm::quiet(g.mode());
    let mut line = vec![0.0f64; w.max(1) as usize];
    for z in 0..d {
        for y in 0..h {
            for f in 0..nf {
                get_line(g, err, src, Axis::X, y, z, f, w, &mut line)?;
                if !b.srcneg && line.iter().any(|&v| v < 0.0) {
                    b.srcneg = true;
                }
                if w > 1 {
                    convert_to_interpolation_coefficients(&libm, &mut line, &poles);
                }
                set_line(g, out, err, &mut coeff, Axis::X, y, z, f, &line)?;
            }
        }
    }
    if h > 1 {
        let mut line = vec![0.0f64; h as usize];
        for z in 0..d {
            for x in 0..w {
                for f in 0..nf {
                    get_line(g, err, &coeff, Axis::Y, x, z, f, h, &mut line)?;
                    convert_to_interpolation_coefficients(&libm, &mut line, &poles);
                    set_line(g, out, err, &mut coeff, Axis::Y, x, z, f, &line)?;
                }
            }
        }
    }
    if d > 1 {
        let mut line = vec![0.0f64; d as usize];
        for y in 0..h {
            for x in 0..w {
                for f in 0..nf {
                    get_line(g, err, &coeff, Axis::Z, x, y, f, d, &mut line)?;
                    convert_to_interpolation_coefficients(&libm, &mut line, &poles);
                    set_line(g, out, err, &mut coeff, Axis::Z, x, y, f, &line)?;
                }
            }
        }
    }
    b.coeff = Some(coeff);
    Ok(b)
}

// ------------------------------------------------------------------------------------------
// Sampling

/// The indices and weights of one axis of extent `n` for the coordinate `x`: the `degree + 1`
/// samples around it (from `floor(x)` for an odd degree, `floor(x + 0.5)` for an even one),
/// mirrored into `[0, n)`; an axis of one voxel takes its voxel with weight 1.
fn axis_weights(
    out: &mut dyn Write,
    degree: i32,
    n: i32,
    x: f64,
    index: &mut [i32; 10],
    weights: &mut [f64; 10],
) -> Result<(), Fatal> {
    let start = if degree & 1 != 0 {
        cvtt64(x.floor() - f64::from(degree / 2))
    } else {
        cvtt64((x + 0.5).floor()).wrapping_sub(i64::from(degree / 2))
    };
    let mut i = start;
    for l in 0..=degree.max(-1) {
        index[l as usize] = i as i32;
        i = i.wrapping_add(1);
    }
    if n == 1 {
        weights[0] = 1.0;
    } else {
        bspline_weights(out, degree, x, index, weights)?;
    }
    // the mirror boundary conditions
    let n2 = n.wrapping_mul(2).wrapping_sub(2);
    for k in 0..=degree.max(-1) {
        let v = &mut index[k as usize];
        *v = if n == 1 {
            0
        } else if *v < 0 {
            let m = v.wrapping_neg();
            m.wrapping_sub(n2.wrapping_mul(m / n2))
        } else {
            v.wrapping_sub(n2.wrapping_mul(*v / n2))
        };
        if n <= *v {
            *v = n2.wrapping_sub(*v);
        }
    }
    Ok(())
}

/// The window of a sample: per axis the indices and weights, and how many samples it sums (0 for
/// an axis of one voxel, else the degree).
struct Window {
    xi: [i32; 10],
    yi: [i32; 10],
    zi: [i32; 10],
    xw: [f64; 10],
    yw: [f64; 10],
    zw: [f64; 10],
    sd: [i32; 3],
}

/// The window of `(x, y, z)` in the coefficients (FS-964: a degree outside 2 to 9 refused).
fn window(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    b: &MriBspline,
    c: &Mri,
    p: (f64, f64, f64),
    what: &str,
) -> Result<Window, Fatal> {
    let (w, h, d) = (c.vg.width, c.vg.height, c.vg.depth);
    let degree = b.degree;
    if !(2..=9).contains(&degree) {
        // FreeSurfer: with an axis longer than one voxel the weights end the program (`Invalid
        // spline degree`) before an index is used, unless the degree is above 9 (its indices
        // written past their arrays first); on a single voxel degrees 0 and 1 read it, a degree
        // below 0 reads indices never set
        if degree <= 9 && (w > 1 || h > 1 || d > 1) {
            let _ = writeln!(out, "Invalid spline degree");
            return Err(Fatal::Exit(1));
        }
        // above 9 FreeSurfer stores a few indices past its arrays, then ends the same way
        // (deterministic where those bytes are inside the stack; a crash for degrees large
        // enough to reach past its top, 100,000 in the probe): `reproduce` gives the end, noted
        if degree > 9 && (w > 1 || h > 1 || d > 1) && g.notices.reproduce() {
            g.notices.add(
                &FS_964,
                format!(
                    "{what} on a B-spline of degree {degree}: FreeSurfer writes past its index \
                     arrays before ending with Invalid spline degree"
                ),
            );
            let _ = writeln!(out, "Invalid spline degree");
            return Err(Fatal::Exit(1));
        }
        if !(0..=1).contains(&degree) {
            g.notices
                .add(&FS_964, format!("{what} on a B-spline of degree {degree}"));
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "{what}: a B-spline of degree {degree} (FreeSurfer reads or writes memory it does not own here)\n"
                ),
            )?;
            return Err(Fatal::Exit(ERROR_BADPARM));
        }
    }
    let mut win = Window {
        xi: [0; 10],
        yi: [0; 10],
        zi: [0; 10],
        xw: [0.0; 10],
        yw: [0.0; 10],
        zw: [0.0; 10],
        sd: [degree; 3],
    };
    axis_weights(out, degree, w, p.0, &mut win.xi, &mut win.xw)?;
    axis_weights(out, degree, h, p.1, &mut win.yi, &mut win.yw)?;
    axis_weights(out, degree, d, p.2, &mut win.zi, &mut win.zw)?;
    for (k, n) in [w, h, d].into_iter().enumerate() {
        if n == 1 {
            win.sd[k] = 0;
        }
    }
    Ok(win)
}

/// The weighted sum of the window's coefficients of frame `f`, in the official order: per row
/// `w + coef * wx` (`w` first), per slice `(w * wy) + w2`, over the slices `(w2 * wz) +
/// interpolated` (the new term first in the last two); then a source without a negative value
/// clips a negative sum to 0.
fn window_sum(
    g: &mut Globals,
    err: &mut dyn Write,
    b: &MriBspline,
    c: &Mri,
    win: &Window,
    f: i32,
) -> Result<f64, Fatal> {
    let flat = Flat::of(c).filter(|_| c.mtype == MRI_FLOAT);
    // the window inside the extent (the mirrored indices always are) of a flat float volume:
    // direct reads in the same order
    if let Some(fl) = flat.filter(|fl| (f as u32) < fl.ext[3] as u32) {
        let chunk = c.chunk.as_deref().expect("voxels");
        let at = |i: usize| {
            f64::from(f32::from_le_bytes([
                chunk[4 * i],
                chunk[4 * i + 1],
                chunk[4 * i + 2],
                chunk[4 * i + 3],
            ]))
        };
        let (ni, nj, nk) = (win.sd[0] as usize, win.sd[1] as usize, win.sd[2] as usize);
        let inside =
            |idx: &[i32; 10], n: usize, e: i32| idx[..=n].iter().all(|&v| (v as u32) < e as u32);
        if inside(&win.xi, ni, fl.ext[0])
            && inside(&win.yi, nj, fl.ext[1])
            && inside(&win.zi, nk, fl.ext[2])
        {
            let base = f as usize * fl.vpv;
            let mut interpolated = 0.0f64;
            for k in 0..=nk {
                let sk = base + win.zi[k] as usize * fl.vps;
                let mut w2 = 0.0f64;
                for j in 0..=nj {
                    let rj = sk + win.yi[j] as usize * fl.vpr;
                    let mut w = 0.0f64;
                    for i in 0..=ni {
                        w = add64(w, mul64(wide_bits(at(rj + win.xi[i] as usize)), win.xw[i]));
                    }
                    w2 = add64(mul64(w, win.yw[j]), w2);
                }
                interpolated = add64(mul64(w2, win.zw[k]), interpolated);
            }
            if !b.srcneg && interpolated < 0.0 {
                interpolated = 0.0;
            }
            return Ok(interpolated);
        }
    }
    let mut interpolated = 0.0f64;
    for k in 0..=win.sd[2] as usize {
        let mut w2 = 0.0f64;
        for j in 0..=win.sd[1] as usize {
            let mut w = 0.0f64;
            for i in 0..=win.sd[0] as usize {
                let v = get(g, err, c, flat, win.xi[i], win.yi[j], win.zi[k], f)?;
                w = add64(w, mul64(v, win.xw[i]));
            }
            w2 = add64(mul64(w, win.yw[j]), w2);
        }
        interpolated = add64(mul64(w2, win.zw[k]), interpolated);
    }
    if !b.srcneg && interpolated < 0.0 {
        interpolated = 0.0;
    }
    Ok(interpolated)
}

/// `MRIsampleBSpline(bspline, x, y, z, frame, &val)`: `NO_ERROR`; the coefficients' `outside_val`
/// for a frame outside the volume or a point unambiguously outside it (`MRIindexNotInVolume` 1,
/// NaN included); else the B-spline's value at the point (mirror boundaries; a source without a
/// negative value clips a negative value to 0).
#[allow(clippy::too_many_arguments)]
pub fn mri_sample_bspline(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    bspline: &MriBspline,
    x: f64,
    y: f64,
    z: f64,
    frame: i32,
    pval: &mut f64,
) -> Result<i32, Fatal> {
    let c = bspline.coeff.as_ref().expect("the B-spline's coefficients");
    if frame >= c.nframes || frame < 0 {
        *pval = c.outside_val;
        return Ok(NO_ERROR);
    }
    if index_not_in_volume(c, x, y, z) == 1 {
        *pval = c.outside_val;
        return Ok(NO_ERROR);
    }
    let win = window(g, out, err, bspline, c, (x, y, z), "MRIsampleBSpline")?;
    *pval = window_sum(g, err, bspline, c, &win, frame)?;
    Ok(NO_ERROR)
}

/// `MRIsampleSeqBSpline(bspline, x, y, z, valvect, firstframe, lastframe)`: `NO_ERROR`;
/// `valvect[f]` for `f` from `firstframe` to `lastframe`: the coefficients' `outside_val` (as a
/// `float`) for a point unambiguously outside the volume, else frame `f`'s value as
/// [`mri_sample_bspline`] computes it, as a `float`. The frames are not checked against the
/// volume's (a frame beyond them reads through `MRIgetVoxVal`, FS-52 and FS-53); `valvect` must
/// hold index `lastframe` (an index below 0 ends with a port error: FreeSurfer writes before the
/// array).
#[allow(clippy::too_many_arguments)]
pub fn mri_sample_seq_bspline(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    bspline: &MriBspline,
    x: f64,
    y: f64,
    z: f64,
    valvect: &mut [f32],
    firstframe: i32,
    lastframe: i32,
) -> Result<i32, Fatal> {
    let c = bspline.coeff.as_ref().expect("the B-spline's coefficients");
    let len = valvect.len();
    let slot = |g: &mut Globals, err: &mut dyn Write, f: i32| -> Result<usize, Fatal> {
        if f < 0 || f as usize >= len {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "MRIsampleSeqBSpline: frame {f} outside the output vector (FreeSurfer writes outside its array here)\n"
                ),
            )?;
        }
        Ok(f as usize)
    };
    if index_not_in_volume(c, x, y, z) == 1 {
        for f in firstframe..=lastframe {
            let k = slot(g, err, f)?;
            valvect[k] = c.outside_val as f32;
        }
        return Ok(NO_ERROR);
    }
    let win = window(g, out, err, bspline, c, (x, y, z), "MRIsampleSeqBSpline")?;
    for f in firstframe..=lastframe {
        let v = window_sum(g, err, bspline, c, &win, f)?;
        let k = slot(g, err, f)?;
        valvect[k] = v as f32;
    }
    Ok(NO_ERROR)
}

// ------------------------------------------------------------------------------------------
// Transforms

/// `MRIlinearTransformBSpline(bspline, mri_dst, A)`: `A` voxel-to-voxel; a singular `A` gives
/// `NULL` after the error `MRIlinearTransformSpline: xform is singular`. The destination is a
/// volume of the source's type with the coefficients' header (`MRIcloneDifferentType`), or the
/// given one cleared; each destination voxel (z outermost) of each frame of the coefficients is
/// the B-spline sampled at `A^-1` of the voxel (the product in `float`, `MatrixMultiply`), stored
/// with `MRIsetVoxVal`; the destination's `ras_good_flag` becomes 1.
pub fn mri_linear_transform_bspline(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    bspline: &MriBspline,
    dst: Option<&mut Mri>,
    ma: Option<&Matrix>,
) -> Result<Ret<Mri>, Fatal> {
    let Some(ainv) = matrix_inverse(g, out, err, ma, None)?.into_new() else {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            "MRIlinearTransformSpline: xform is singular",
        );
        return Ok(Ret::Null);
    };
    let c = bspline.coeff.as_ref().expect("the B-spline's coefficients");
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => {
            mri_clear(g, err, d)?;
            d
        }
        None => fresh.insert(mri_clone_different_type(g, out, err, c, bspline.srctype)?),
    };
    let (w, h, dd) = (d.vg.width, d.vg.height, d.vg.depth);
    let dflat = Flat::of(d);
    let mut vx = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    let mut vy = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    vy.set_relt(4, 1, 1.0);
    for y3 in 0..dd {
        vy.set_relt(3, 1, y3 as f32);
        for y2 in 0..h {
            vy.set_relt(2, 1, y2 as f32);
            for y1 in 0..w {
                vy.set_relt(1, 1, y1 as f32);
                matrix_multiply(g, out, err, Some(&ainv), Some(&vy), Some(&mut vx))?;
                let x1 = f64::from(vx.relt(1, 1));
                let x2 = f64::from(vx.relt(2, 1));
                let x3 = f64::from(vx.relt(3, 1));
                for frame in 0..c.nframes {
                    let mut val = 0.0f64;
                    mri_sample_bspline(g, out, err, bspline, x1, x2, x3, frame, &mut val)?;
                    set(g, err, d, dflat, y1, y2, y3, frame, val as f32)?;
                }
            }
        }
    }
    d.vg.ras_good_flag = 1;
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIapplyRASlinearTransformBSpline(bspline, mri_dst, m_ras)`: the RAS transform made a
/// voxel-to-voxel one between the coefficients and the destination (the coefficients themselves
/// when there is none; `MRIrasXformToVoxelXform`), then [`mri_linear_transform_bspline`].
pub fn mri_apply_ras_linear_transform_bspline(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    bspline: &MriBspline,
    dst: Option<&mut Mri>,
    m_ras: Option<&Matrix>,
) -> Result<Ret<Mri>, Fatal> {
    let c = bspline.coeff.as_ref().expect("the B-spline's coefficients");
    let mv = mri_ras_xform_to_voxel_xform(g, out, err, c, dst.as_deref(), m_ras)?;
    mri_linear_transform_bspline(g, out, err, bspline, dst, mv.as_ref())
}

/// The destination `LTAtransformBSpline` makes when it is given none: with the transform's
/// destination geometry valid (1), a volume of that extent, the source's type and the
/// coefficients' frames, with the coefficients' header and that geometry; else, with
/// `USE_AVERAGE305` set (any value; two `INFO` lines on standard error), a volume of the source's
/// type with the coefficients' header (`MRIcloneDifferentType`), no Talairach file name, the
/// centre of `average_305`, `ras_good_flag` 1 and its cached matrices recomputed; else (`INFO:
/// Transform dst volume info is not used (valid flag = 0).`) that volume with no Talairach file
/// name.
fn new_lta_destination(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    bspline: &MriBspline,
    lta: &Lta,
) -> Result<Mri, Fatal> {
    let c = bspline.coeff.as_ref().expect("the B-spline's coefficients");
    let tran = &lta.xforms[0].dst;
    if tran.valid == 1 {
        let mut d = mri_alloc_sequence(
            g,
            out,
            err,
            tran.width,
            tran.height,
            tran.depth,
            bspline.srctype,
            c.nframes,
        )?;
        mri_copy_header(g, out, err, c, Some(&mut d))?;
        use_vol_geom_to_mri(g, out, err, tran, &mut d.vg)?;
        return Ok(d);
    }
    if env.get("USE_AVERAGE305").is_some() {
        let _ = err.write_all(b"INFO: Environmental variable USE_AVERAGE305 set\n");
        let _ = err.write_all(b"INFO: Modifying dst c_(r,a,s), using average_305 values\n");
        let mut d = mri_clone_different_type(g, out, err, c, bspline.srctype)?;
        d.transform_fname.clear();
        d.vg.c_r = -0.0950f64 as f32;
        d.vg.c_a = -16.5100f64 as f32;
        d.vg.c_s = 9.7500f64 as f32;
        d.vg.ras_good_flag = 1;
        reinit_cache(g, out, err, &mut d.vg)?;
        return Ok(d);
    }
    let _ = err.write_all(b"INFO: Transform dst volume info is not used (valid flag = 0).\n");
    let mut d = mri_clone_different_type(g, out, err, c, bspline.srctype)?;
    d.transform_fname.clear();
    Ok(d)
}

/// `LTAtransformBSpline(bspline, mri_dst, lta)`: an array of other than one transform ends the
/// program (`LTAtransformBSpline: numxforms != 1 not supported`, `ERROR_BADPARM`); with no
/// destination given, [`new_lta_destination`]'s. `LINEAR_RAS_TO_RAS`:
/// [`mri_apply_ras_linear_transform_bspline`] of the matrix; `LINEAR_VOX_TO_VOX`: of the RAS
/// matrix `i2r(dst) M r2i(src)` (the transform's geometries where valid, else, with `INFO: LTA dst
/// (src) geometry information missing!` on standard error, the destination's or the
/// coefficients'); `LINEAR_PHYSVOX_TO_PHYSVOX` changes the array to `LINEAR_RAS_TO_RAS` first;
/// another type ends the program (`LTAtransformBSpline: unknown linear transform`).
pub fn lta_transform_bspline(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    bspline: &MriBspline,
    dst: Option<&mut Mri>,
    lta: &mut Lta,
) -> Result<Ret<Mri>, Fatal> {
    if lta.num_xforms != 1 {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "LTAtransformBSpline: numxforms != 1 not supported\n",
        )?;
        return Ok(Ret::Null);
    }
    let mut own = None;
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => own.insert(new_lta_destination(g, out, err, env, bspline, lta)?),
    };
    let r = if lta.ltype == LINEAR_RAS_TO_RAS {
        let m = lta.xforms[0].m_l.clone();
        mri_apply_ras_linear_transform_bspline(g, out, err, bspline, Some(d), m.as_ref())?
    } else if lta.ltype == LINEAR_VOX_TO_VOX {
        let c = bspline.coeff.as_ref().expect("the B-spline's coefficients");
        let lt = &lta.xforms[0];
        let i2r = if lt.dst.valid != 0 {
            vg_i_to_r(g, out, err, &lt.dst)?
        } else {
            let _ = err.write_all(
                b"INFO: LTA dst geometry information missing!\n      We assume that the dst volume passed is the\n      same as the dst for the transform.\n",
            );
            extract_i_to_r(g, out, err, &d.vg)?
        };
        let tmp = matrix_multiply(g, out, err, Some(&i2r), lt.m_l.as_ref(), None)?.into_new();
        let r2i = if lt.src.valid != 0 {
            vg_r_to_i(g, out, err, &lt.src)?
        } else {
            let _ = err.write_all(
                b"INFO: LTA src geometry information missing!\n      We assume that the src volume passed is the\n      same as the src for the transform.\n",
            );
            extract_r_to_i(g, out, err, &c.vg)?
        };
        // MatrixMultiply(tmp, r2i, tmp): FreeSurfer multiplies a copy of tmp into it
        let tmp = match tmp {
            Some(mut t) => {
                let t0 = t.clone();
                match matrix_multiply(g, out, err, Some(&t0), r2i.as_ref(), Some(&mut t))? {
                    Ret::Null => None,
                    _ => Some(t),
                }
            }
            None => matrix_multiply(g, out, err, None, r2i.as_ref(), None)?.into_new(),
        };
        mri_apply_ras_linear_transform_bspline(g, out, err, bspline, Some(d), tmp.as_ref())?
    } else if lta.ltype == LINEAR_PHYSVOX_TO_PHYSVOX {
        lta_change_type(g, out, err, env, lta, LINEAR_RAS_TO_RAS)?;
        let m = lta.xforms[0].m_l.clone();
        mri_apply_ras_linear_transform_bspline(g, out, err, bspline, Some(d), m.as_ref())?
    } else {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "LTAtransformBSpline: unknown linear transform\n",
        )?;
        Ret::Null
    };
    Ok(match (r, own) {
        (Ret::Given, Some(d)) => Ret::New(d),
        (Ret::Given, None) => Ret::Given,
        (Ret::New(m), _) => Ret::New(m),
        (Ret::Null, _) => Ret::Null,
    })
}

// ------------------------------------------------------------------------------------------
// The factor-of-two pyramids (Unser's, Brigger's and Sage's centred cubic spline filter)

/// `PyramidFilterCentered`'s order-3 filters: the reduce filter `g` (21 taps) and the expand
/// filter `h` (16 taps), as FreeSurfer's file writes them.
const CENTERED3_G: [f64; 21] = [
    0.708792,
    0.328616,
    -0.165157,
    -0.114448,
    0.0944036,
    0.0543881,
    -0.05193,
    -0.0284868,
    0.0281854,
    0.0152877,
    -0.0152508,
    -0.00825077,
    0.00824629,
    0.00445865,
    -0.0044582,
    -0.00241009,
    0.00241022,
    0.00130278,
    -0.00130313,
    -0.000704109,
    0.000704784,
];
const CENTERED3_H: [f64; 16] = [
    1.13726,
    0.625601,
    -0.0870191,
    -0.159256,
    0.0233167,
    0.0426725,
    -0.00624769,
    -0.0114341,
    0.00167406,
    0.00306375,
    -0.000448564,
    -0.000820929,
    0.000120192,
    0.000219967,
    -0.0000322054,
    -0.00005894,
];

/// `ReduceCentered_1D(In, n_in, Out, g, ng)`: the line filtered with the symmetric filter (the
/// pseudo-mirror borders of the even length `2 (n_in / 2)`; an odd last sample left out), then
/// pairs averaged (the Haar step, `(y[2k + 1] + y[2k]) * 0.5`).
fn reduce_centered(input: &[f64], output: &mut [f64], g: &[f64]) {
    let nred = (input.len() / 2) as i64;
    let n = nred * 2;
    let kn = 2 * n;
    let mut y = vec![0.0f64; n as usize];
    for k in 0..n {
        let mut v = mul64(input[k as usize], g[0]);
        for (i, &gi) in g.iter().enumerate().skip(1) {
            let i = i as i64;
            let (mut i1, mut i2) = (k - i, k + i);
            if i1 < 0 {
                i1 = (2 * n - 1 - i1) % kn;
                if i1 >= n {
                    i1 = kn - i1 - 1;
                }
            }
            if i2 > n - 1 {
                i2 %= kn;
                if i2 >= n {
                    i2 = kn - i2 - 1;
                }
            }
            v = add64(v, mul64(add64(input[i1 as usize], input[i2 as usize]), gi));
        }
        y[k as usize] = v;
    }
    for kk in 0..nred as usize {
        output[kk] = mul64(add64(y[2 * kk + 1], y[2 * kk]), 0.5);
    }
}

/// `ExpandCentered_1D(In, n_in, Out, h, nh)`: the line of `2 n_in` samples interpolated with the
/// symmetric filter (pseudo-mirror borders), then the backward Haar step (`Out[j] = (Out[j] +
/// Out[j - 1]) * 0.5` from the top, `Out[1] = (Out[0] + Out[1]) * 0.5`, `Out[0]` halved).
fn expand_centered(input: &[f64], output: &mut [f64], h: &[f64]) {
    let n = input.len() as i64;
    let nexp = 2 * n;
    let kn = 2 * n;
    let nh = h.len() as i64;
    let k0 = (nh / 2) * 2 - 1;
    for i in 0..n {
        let j = (i * 2) as usize;
        let mut v = mul64(input[i as usize], h[0]);
        let mut k = 2;
        while k < nh {
            let (mut i1, mut i2) = (i - k / 2, i + k / 2);
            if i1 < 0 {
                i1 = (2 * n - 1 - i1) % kn;
                if i1 >= n {
                    i1 = kn - i1 - 1;
                }
            }
            if i2 >= n {
                i2 %= kn;
                if i2 >= n {
                    i2 = kn - i2 - 1;
                }
            }
            v = add64(
                v,
                mul64(add64(input[i1 as usize], input[i2 as usize]), h[k as usize]),
            );
            k += 2;
        }
        output[j] = v;
        let mut v = 0.0f64;
        let mut k = -k0;
        while k < nh {
            let kk = k.abs();
            let mut i1 = i + (k + 1) / 2;
            if i1 < 0 {
                i1 = (2 * n - 1 - i1) % kn;
                if i1 > n - 1 {
                    i1 = kn - i1 - 1;
                }
            }
            if i1 >= n {
                i1 %= kn;
                if i1 >= n {
                    i1 = kn - i1 - 1;
                }
            }
            v = add64(v, mul64(h[kk as usize], input[i1 as usize]));
            k += 2;
        }
        output[j + 1] = v;
    }
    let nexp = nexp as usize;
    for j in (2..nexp).rev() {
        output[j] = mul64(add64(output[j], output[j - 1]), 0.5);
    }
    if nexp >= 2 {
        output[1] = mul64(add64(output[0], output[1]), 0.5);
    }
    output[0] = mul64(0.5, output[0]);
}

/// The pyramid functions' new centre: the source's voxel-to-RAS matrix (`extract_i_to_r`) times
/// the voxel `c` (a `float` vector), the product `MatrixMultiply`'s.
fn new_centre(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    c: [f64; 3],
    dst: &mut Mri,
) -> Result<(), Fatal> {
    let mut cv = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    cv.set_relt(1, 1, c[0] as f32);
    cv.set_relt(2, 1, c[1] as f32);
    cv.set_relt(3, 1, c[2] as f32);
    cv.set_relt(4, 1, 1.0);
    let v2r = extract_i_to_r(g, out, err, &src.vg)?;
    let p = match matrix_multiply(g, out, err, Some(&v2r), Some(&cv), None)? {
        Ret::New(p) => p,
        _ => return Err(Fatal::Exit(1)),
    };
    dst.vg.c_r = p.relt(1, 1);
    dst.vg.c_a = p.relt(2, 1);
    dst.vg.c_s = p.relt(3, 1);
    reinit_cache(g, out, err, &mut dst.vg)?;
    Ok(())
}

/// FS-963's notice: an axis of one voxel whose voxel size the pyramid function changes.
fn note_single(g: &mut Globals, src: &Mri, what: &str) {
    let v = &src.vg;
    if v.width == 1 || v.height == 1 || v.depth == 1 {
        g.notices.add(
            &FS_963,
            format!(
                "{what} changes the voxel size of an axis of one voxel ({} x {} x {})",
                v.width, v.height, v.depth
            ),
        );
    }
}

/// `MRIdownsample2BSpline(mri_src, mri_dst)`: each axis longer than one voxel halved (`n / 2`
/// voxels, an odd last voxel left out) with the centred cubic spline reduce filter
/// ([`reduce_centered`]), x, then y, then z, through `float` volumes (the last pass into the
/// destination: of the source's type with its header, or the given one, whose extent and frames
/// must be the new ones: `ERROR MRIupsample2BSpline: MRI Dest dimensions not correct!` on standard
/// output, status 1); an axis of one voxel is copied. Then `imnr0` and `imnr1`, the voxel sizes,
/// `thick` and `ps` doubled (FS-963), and the centre moved to the source's voxel `(width / 2 + 0.5,
/// height / 2 + 0.5, depth / 2 + 0.5)` (integer halves), the cached matrices recomputed.
pub fn mri_downsample2_bspline(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (nxi, nyi, nzi, nfi) = (src.vg.width, src.vg.height, src.vg.depth, src.nframes);
    let nxo = (nxi / 2).max(1);
    let nyo = (nyi / 2).max(1);
    let nzo = (nzi / 2).max(1);
    let g3 = &CENTERED3_G[..];
    // x
    let mut tmp = mri_alloc_sequence(g, out, err, nxo, nyi, nzi, MRI_FLOAT, nfi)?;
    let mut inb = vec![0.0f64; nxi.max(1) as usize];
    let mut outb = vec![0.0f64; nxo as usize];
    for f in 0..nfi {
        for z in 0..nzi {
            for y in 0..nyi {
                get_line(g, err, src, Axis::X, y, z, f, nxi, &mut inb)?;
                if nxi > 1 {
                    reduce_centered(&inb, &mut outb, g3);
                    set_line(g, out, err, &mut tmp, Axis::X, y, z, f, &outb)?;
                } else {
                    set_line(g, out, err, &mut tmp, Axis::X, y, z, f, &inb)?;
                }
            }
        }
    }
    // y
    let mut tmp2 = mri_alloc_sequence(g, out, err, nxo, nyo, nzi, MRI_FLOAT, nfi)?;
    let mut inb = vec![0.0f64; nyi.max(1) as usize];
    let mut outb = vec![0.0f64; nyo as usize];
    for f in 0..nfi {
        for z in 0..nzi {
            for x in 0..nxo {
                get_line(g, err, &tmp, Axis::Y, x, z, f, nyi, &mut inb)?;
                if nyi > 1 {
                    reduce_centered(&inb, &mut outb, g3);
                    set_line(g, out, err, &mut tmp2, Axis::Y, x, z, f, &outb)?;
                } else {
                    set_line(g, out, err, &mut tmp2, Axis::Y, x, z, f, &inb)?;
                }
            }
        }
    }
    drop(tmp);
    // z
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let mut d = mri_alloc_sequence(g, out, err, nxo, nyo, nzo, src.mtype, nfi)?;
            mri_copy_header(g, out, err, src, Some(&mut d))?;
            fresh.insert(d)
        }
    };
    if d.vg.width != nxo || d.vg.height != nyo || d.vg.depth != nzo || d.nframes != nfi {
        let _ = writeln!(
            out,
            "ERROR MRIupsample2BSpline: MRI Dest dimensions not correct!"
        );
        return Err(Fatal::Exit(1));
    }
    let mut inb = vec![0.0f64; nzi.max(1) as usize];
    let mut outb = vec![0.0f64; nzo as usize];
    for f in 0..nfi {
        for y in 0..nyo {
            for x in 0..nxo {
                get_line(g, err, &tmp2, Axis::Z, x, y, f, nzi, &mut inb)?;
                if nzi > 1 {
                    reduce_centered(&inb, &mut outb, g3);
                    set_line(g, out, err, d, Axis::Z, x, y, f, &outb)?;
                } else {
                    set_line(g, out, err, d, Axis::Z, x, y, f, &inb)?;
                }
            }
        }
    }
    drop(tmp2);
    d.imnr0 = src.imnr0;
    d.imnr1 = src.imnr0.wrapping_add(d.vg.depth).wrapping_sub(1);
    note_single(g, src, "MRIdownsample2BSpline");
    d.vg.xsize = sse::mul(src.vg.xsize, 2.0);
    d.vg.ysize = sse::mul(src.vg.ysize, 2.0);
    d.vg.zsize = sse::mul(src.vg.zsize, 2.0);
    d.thick = sse::mul(src.thick, 2.0);
    d.ps = sse::mul(src.ps, 2.0);
    let c = [
        f64::from(nxi / 2) + 0.5,
        f64::from(nyi / 2) + 0.5,
        f64::from(nzi / 2) + 0.5,
    ];
    new_centre(g, out, err, src, c, d)?;
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIupsample2BSpline(mri_src, mri_dst)`: each axis longer than one voxel doubled with the
/// centred cubic spline expand filter ([`expand_centered`]), x, then y, then z, in the destination
/// itself (of the source's type with its header, or the given one, whose extent and frames must
/// be the new ones: `ERROR MRIupsample2BSpline: MRI Dest dimensions not correct!` on standard
/// output, status 1): each pass reads the previous pass's lines from the destination (as its type
/// stores them). On an axis of one voxel, FS-962. Then `imnr0` and `imnr1`, the voxel sizes halved
/// (FS-963), and the centre moved to the source's voxel `(width / 2 - 0.25, height / 2 - 0.25,
/// depth / 2 - 0.25)`, the cached matrices recomputed.
pub fn mri_upsample2_bspline(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (nxi, nyi, nzi, nfi) = (src.vg.width, src.vg.height, src.vg.depth, src.nframes);
    let up = |n: i32| if n <= 1 { 1 } else { n.wrapping_mul(2) };
    let (nxo, nyo, nzo) = (up(nxi), up(nyi), up(nzi));
    let h3 = &CENTERED3_H[..];
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let mut d = mri_alloc_sequence(g, out, err, nxo, nyo, nzo, src.mtype, nfi)?;
            mri_copy_header(g, out, err, src, Some(&mut d))?;
            fresh.insert(d)
        }
    };
    if d.vg.width != nxo || d.vg.height != nyo || d.vg.depth != nzo || d.nframes != nfi {
        let _ = writeln!(
            out,
            "ERROR MRIupsample2BSpline: MRI Dest dimensions not correct!"
        );
        return Err(Fatal::Exit(1));
    }
    // x
    let mut inb = vec![0.0f64; nxi.max(1) as usize];
    let mut outb = vec![0.0f64; nxo.max(1) as usize];
    for f in 0..nfi {
        for z in 0..nzi {
            for y in 0..nyi {
                get_line(g, err, src, Axis::X, y, z, f, nxi, &mut inb)?;
                if nxi > 1 {
                    expand_centered(&inb, &mut outb, h3);
                    set_line(g, out, err, d, Axis::X, y, z, f, &outb)?;
                } else {
                    set_line(g, out, err, d, Axis::X, y, z, f, &inb)?;
                }
            }
        }
    }
    // y: FreeSurfer copies the source's own column over a single row (FS-962)
    let mut inb = vec![0.0f64; nyi.max(1) as usize];
    let mut outb = vec![0.0f64; nyo.max(1) as usize];
    for f in 0..nfi {
        for z in 0..nzi {
            for x in 0..nxo {
                if nyi > 1 {
                    get_line(g, err, d, Axis::Y, x, z, f, nyi, &mut inb)?;
                    expand_centered(&inb, &mut outb, h3);
                    set_line(g, out, err, d, Axis::Y, x, z, f, &outb)?;
                } else if single_axis_copy(g, nxo != nxi, "row") {
                    get_line(g, err, src, Axis::Y, x, z, f, nyi, &mut inb)?;
                    set_line(g, out, err, d, Axis::Y, x, z, f, &inb)?;
                }
            }
        }
    }
    // z: the same over a single slice
    let mut inb = vec![0.0f64; nzi.max(1) as usize];
    let mut outb = vec![0.0f64; nzo.max(1) as usize];
    for f in 0..nfi {
        for y in 0..nyo {
            for x in 0..nxo {
                if nzi > 1 {
                    get_line(g, err, d, Axis::Z, x, y, f, nzi, &mut inb)?;
                    expand_centered(&inb, &mut outb, h3);
                    set_line(g, out, err, d, Axis::Z, x, y, f, &outb)?;
                } else if single_axis_copy(g, nxo != nxi || nyo != nyi, "slice") {
                    get_line(g, err, src, Axis::Z, x, y, f, nzi, &mut inb)?;
                    set_line(g, out, err, d, Axis::Z, x, y, f, &inb)?;
                }
            }
        }
    }
    d.imnr0 = src.imnr0;
    d.imnr1 = src.imnr0.wrapping_add(d.vg.depth).wrapping_sub(1);
    note_single(g, src, "MRIupsample2BSpline");
    d.vg.xsize = (f64::from(src.vg.xsize) / 2.0) as f32;
    d.vg.ysize = (f64::from(src.vg.ysize) / 2.0) as f32;
    d.vg.zsize = (f64::from(src.vg.zsize) / 2.0) as f32;
    let c = [
        f64::from(nxi) / 2.0 - 0.25,
        f64::from(nyi) / 2.0 - 0.25,
        f64::from(nzi) / 2.0 - 0.25,
    ];
    new_centre(g, out, err, src, c, d)?;
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// FS-962: whether the upsampling copies the source's line over a single row or slice
/// (`reproduce`); by default the previous passes' lines stay. Noted when the copy would change
/// them (any of the earlier axes expanded).
fn single_axis_copy(g: &mut Globals, expanded: bool, axis: &str) -> bool {
    if expanded {
        g.notices.add(
            &FS_962,
            format!(
                "MRIupsample2BSpline copies the source over the expanded lines of a single {axis}"
            ),
        );
        return g.notices.reproduce();
    }
    // nothing was expanded before: the copy writes the values already there
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;
    use larmorx_core::math;

    /// The horizons and first powers of every pole and every line length that reaches them: the
    /// correctly rounded `log` and glibc's give the same bits; `pow` differs on one argument only
    /// (degree 6's third pole, a line of 5 samples: `pow(z, 4)` one unit in the last place), so the
    /// default and `reproduce` share every other line's coefficients.
    #[test]
    fn log_and_pow_agree_with_glibc_where_the_filter_calls_them() {
        let mut sink = Vec::new();
        let mut differ = Vec::new();
        for degree in 2..=9 {
            for z in init_poles(&mut sink, degree).unwrap() {
                let cr = Libm::quiet(UpstreamDefects::Fix);
                let gl = Libm::quiet(UpstreamDefects::Reproduce);
                let h = horizon(&cr, z);
                assert_eq!(h, horizon(&gl, z), "degree {degree} pole {z}");
                assert!(h >= 2);
                // the full loop runs for lengths up to the horizon
                for n in 2..=h {
                    let e = (n - 1) as f64;
                    if math::pow(z, e).to_bits() != math::pow_glibc(z, e).to_bits() {
                        differ.push((degree, z.to_bits(), n));
                    }
                }
                assert_eq!(
                    math::log(f64::EPSILON).to_bits(),
                    math::log_glibc(f64::EPSILON).to_bits()
                );
                assert_eq!(
                    math::log(z.abs()).to_bits(),
                    math::log_glibc(z.abs()).to_bits()
                );
            }
        }
        assert_eq!(
            differ,
            [(6, (-0.001_414_151_808_325_817_7f64).to_bits(), 5)]
        );
    }

    /// The cubic B-spline's interpolation: the coefficients of a line sampled at its knots give
    /// the line back (to rounding), and a constant line stays constant.
    #[test]
    fn cubic_coefficients_interpolate_their_samples() {
        let libm = Libm::quiet(UpstreamDefects::Fix);
        let mut sink = Vec::new();
        let poles = init_poles(&mut sink, 3).unwrap();
        for n in [2usize, 3, 5, 17, 40] {
            let s: Vec<f64> = (0..n).map(|i| ((i * 7 + 3) % 11) as f64 - 4.0).collect();
            let mut c = s.clone();
            convert_to_interpolation_coefficients(&libm, &mut c, &poles);
            for (k, &sk) in s.iter().enumerate() {
                // (c[k-1] + 4 c[k] + c[k+1]) / 6 with mirror borders
                let at = |i: i64| -> f64 {
                    let m = 2 * n as i64 - 2;
                    let mut i = i.rem_euclid(m.max(1));
                    if i >= n as i64 {
                        i = m - i;
                    }
                    c[i as usize]
                };
                let k = k as i64;
                let v = (at(k - 1) + 4.0 * at(k) + at(k + 1)) / 6.0;
                assert!((v - sk).abs() < 1e-9, "n {n} k {k}: {v} vs {sk}");
            }
        }
        let mut c = vec![2.5f64; 9];
        convert_to_interpolation_coefficients(&libm, &mut c, &poles);
        for v in c {
            assert!((v - 2.5).abs() < 1e-12);
        }
    }

    /// FS-964 above degree 9 on coefficients with an axis longer than one voxel: `reproduce` ends
    /// as FreeSurfer does (`Invalid spline degree` on standard output, status 1), noted; the
    /// default refuses the degree with FS-964's error; on one voxel both modes refuse.
    #[test]
    fn degrees_above_nine_end_as_freesurfer_under_reproduce() {
        for (shape, degree) in [
            ((3, 3, 3), 10),
            ((3, 3, 3), 11),
            ((3, 1, 1), 64),
            ((3, 4, 3), 2_147_483_647),
        ] {
            for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
                let mut g = Globals::new(mode);
                let (mut out, mut err) = (Vec::new(), Vec::new());
                let mut b =
                    mri_alloc_bspline(&mut g, &mut out, &mut err, shape.0, shape.1, shape.2, 1)
                        .unwrap();
                b.degree = degree;
                out.clear();
                let mut v = 7.0;
                let r =
                    mri_sample_bspline(&mut g, &mut out, &mut err, &b, 1.5, 0.25, 0.0, 0, &mut v);
                let noted = g.notices.contains(&FS_964);
                assert!(noted, "{shape:?} {degree} {mode:?}");
                if mode == UpstreamDefects::Reproduce {
                    assert!(matches!(r, Err(Fatal::Exit(1))), "{shape:?} {degree}");
                    assert_eq!(out, b"Invalid spline degree\n");
                    assert!(err.is_empty());
                } else {
                    assert!(
                        matches!(r, Err(Fatal::Exit(ERROR_BADPARM))),
                        "{shape:?} {degree}"
                    );
                    assert!(out.is_empty());
                }
            }
        }
        for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
            let mut g = Globals::new(mode);
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let mut b = mri_alloc_bspline(&mut g, &mut out, &mut err, 1, 1, 1, 1).unwrap();
            b.degree = 10;
            out.clear();
            let mut v = 7.0;
            let r = mri_sample_bspline(&mut g, &mut out, &mut err, &b, 0.0, 0.0, 0.0, 0, &mut v);
            assert!(matches!(r, Err(Fatal::Exit(ERROR_BADPARM))), "{mode:?}");
        }
    }

    /// The weights of every degree sum to 1.
    #[test]
    fn weights_sum_to_one() {
        let mut sink = Vec::new();
        for degree in 2..=9 {
            for x in [0.0, 0.25, 0.5, 0.75, 3.1, -0.4] {
                let mut idx = [0i32; 10];
                let mut wt = [0.0f64; 10];
                axis_weights(&mut sink, degree, 50, x + 10.0, &mut idx, &mut wt).unwrap();
                let s: f64 = wt[..=degree as usize].iter().sum();
                assert!((s - 1.0).abs() < 1e-12, "degree {degree} x {x}: {s}");
            }
        }
    }
}
