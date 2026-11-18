// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MyMRI` (queue 3.2-02, `fs.robust` 1 of 2): the image helpers of Martin Reuter's robust
//! registration that both `mri_robust_register` and `mri_robust_template` reach, translated from
//! FreeSurfer 8.2.0's `mri_robust_register/MyMRI.cpp` and `MyMRI.h`:
//!
//! - [`mri_valscale`] (`MRIvalscale`): `(v - b) * s` voxel by voxel in the voxel's type;
//! - [`get_prefilter`], [`get_derfilter`]: the five-tap smoothing and derivative kernels;
//! - [`sub_sample`] (`subSample`): every second voxel (optionally at the fixed pseudo-random
//!   offsets of [`get_rand`]), with the header adjusted to the coarser grid;
//! - [`get_blur`] (`getBlur`) and [`get_partials`] (`getPartials`): the separable smoothing and
//!   the three partial derivatives with the smoothed image;
//! - [`find_right_size`] (`findRightSize`), [`get_background`] (`getBackground`),
//!   [`get_rand`] (`getRand`), [`mri_linear_transform`] (`MyMRI::MRIlinearTransform`).
//!
//! The functions only `mri_robust_register` or only `mri_robust_template` calls (`MRInorm255`,
//! the entropy and NLSD images, the normalised, mean and median filters, `noiseVar`,
//! `gaussianCube`, `setMaxOutsideVal`, `setTypeUCHAR`, `MRIvoxelXformToRasXform`, `isConform`,
//! `isIsotropic`, `MRIgetZslice`) come with those programs' items (3.2-13, 3.2-14..16).
//!
//! Arithmetic in the C types and order; NaNs as the official build's SSE arithmetic gives them
//! ([`larmorx_freesurfer_core::sse`]). FreeSurfer's `assert`s are compiled out (`-DNDEBUG`).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{div64, mul64, sub64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_filter::conv::{
    MRI_DEPTH, MRI_HEIGHT, MRI_WIDTH, mri_convolve_gaussian, mri_convolve1d,
};
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri, mri_alloc, mri_alloc_sequence,
    mri_copy_header,
};
use larmorx_freesurfer_mri::vol_geom::{extract_i_to_r, reinit_cache};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{Pair, mri_clone, mri_copy_frame};
use larmorx_freesurfer_volume::xform;

use crate::mymatrix::{Mat4, to_matrix};

/// A `float` widened to `double` as `cvtss2sd` does: a signalling NaN comes out quiet (written
/// with the bits, so that no build folds the widening away).
#[inline(always)]
pub(crate) fn wide(v: f32) -> f64 {
    let d = f64::from(v);
    if d.is_nan() {
        f64::from_bits(d.to_bits() | 0x0008_0000_0000_0000)
    } else {
        d
    }
}

/// The element index of voxel `(0, y, z)` of frame `f` (the row pointer `MRI*seq_vox(m, 0, y, z,
/// f)`), in elements.
fn row_index(m: &Mri, y: i32, z: i32, f: i32) -> usize {
    (f as u64 * m.vox_per_vol + z as u64 * m.vox_per_slice + y as u64 * m.vox_per_row) as usize
}

/// The value `MRIvalscale` stores for `val` in a voxel of `ty`, as little-endian bytes (the
/// clipping and rounding of its type's branch).
fn scaled_bytes(g: &mut Globals, ty: i32, val: f32) -> Vec<u8> {
    match ty {
        MRI_SHORT => {
            let mut v = val;
            if v < f32::from(i16::MIN) {
                v = f32::from(i16::MIN);
            }
            if v > f32::from(i16::MAX) {
                v = f32::from(i16::MAX);
            }
            (nint(g, wide(v)) as i16).to_le_bytes().to_vec()
        }
        MRI_USHRT => {
            let mut v = val;
            if v < 0.0 {
                v = 0.0;
            }
            if v > f32::from(u16::MAX) {
                v = f32::from(u16::MAX);
            }
            (nint(g, wide(v)) as u16).to_le_bytes().to_vec()
        }
        MRI_UCHAR => {
            // no lower clip: a negative value wraps modulo 256 (C's conversion to BUFTYPE)
            let mut v = val;
            if v > 255.0 {
                v = 255.0;
            }
            vec![nint(g, wide(v)) as u8]
        }
        _ => val.to_le_bytes().to_vec(),
    }
}

/// The value of element `i` of a voxel buffer of type `ty` as `float` (the branch's read).
fn read_elem(c: &[u8], ty: i32, i: usize) -> f32 {
    match ty {
        MRI_FLOAT => f32::from_le_bytes(c[4 * i..4 * i + 4].try_into().expect("4 bytes")),
        MRI_SHORT => f32::from(i16::from_le_bytes([c[2 * i], c[2 * i + 1]])),
        MRI_USHRT => f32::from(u16::from_le_bytes([c[2 * i], c[2 * i + 1]])),
        _ => f32::from(c[i]),
    }
}

/// `MRIvalscale(mri_src, mri_dst, s, b)`: each voxel `v` of every frame becomes `(v - b) * s`,
/// computed as the type's branch does (`val -= b; val *= s;` on a `float`, each step through
/// `double`); a short result clipped to `[-32768, 32767]`, an unsigned short one to `[0, 65535]`, a
/// uchar one only above at 255 (a negative value wraps modulo 256), each then `nint`. Without a
/// destination, `MRIclone` of the source. Another source type: `MRIvalScale: unsupported type
/// <type>` (an error) and the destination as it is. The destination is read as the source's type
/// (FreeSurfer's callers pass a clone or the source itself).
pub fn mri_valscale(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    s: f64,
    b: f64,
) -> Result<Ret<Mri>, Fatal> {
    // the source's values first (the source may be the destination: every voxel is read before
    // it is written, as in the C loop)
    let src: &Mri = match &p {
        Pair::New(src) | Pair::Given(src, _) => src,
        Pair::Same(m) => m,
    };
    let ty = src.mtype;
    let (w, h, d, nf) = (src.vg.width, src.vg.height, src.vg.depth, src.nframes);
    let vals = if matches!(ty, MRI_FLOAT | MRI_SHORT | MRI_USHRT | MRI_UCHAR) {
        let c = src.chunk.as_ref().expect("the source's voxels");
        let mut vals = Vec::with_capacity((w.max(0) * h.max(0) * d.max(0) * nf.max(0)) as usize);
        for f in 0..nf {
            for z in 0..d {
                for y in 0..h {
                    let r = row_index(src, y, z, f);
                    for x in 0..w as usize {
                        vals.push(read_elem(c, ty, r + x));
                    }
                }
            }
        }
        Some(vals)
    } else {
        None
    };
    let mut fresh = None;
    let dst: &mut Mri = match p {
        Pair::New(src) => fresh.insert(mri_clone(g, out, err, src, None)?.unwrap_new()),
        Pair::Given(_, d) => d,
        Pair::Same(m) => m,
    };
    let Some(vals) = vals else {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("MRIvalScale: unsupported type {ty}"),
        );
        return Ok(match fresh {
            Some(m) => Ret::New(m),
            None => Ret::Given,
        });
    };
    let size = match ty {
        MRI_FLOAT => 4,
        MRI_SHORT | MRI_USHRT => 2,
        _ => 1,
    };
    let mut k = 0;
    for f in 0..nf {
        for z in 0..d {
            for y in 0..h {
                let r = row_index(dst, y, z, f);
                for x in 0..w as usize {
                    let mut val = vals[k];
                    k += 1;
                    val = sub64(wide(val), b) as f32;
                    val = mul64(wide(val), s) as f32;
                    let bytes = scaled_bytes(g, ty, val);
                    let c = dst.chunk.as_mut().expect("the destination's voxels");
                    let o = (r + x) * size;
                    c[o..o + size].copy_from_slice(&bytes);
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MyMRI::getPrefilter()`: a 5 x 1 x 1 float volume, `0.03504 0.24878 0.43234 0.24878 0.03504`.
pub fn get_prefilter(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write) -> Result<Mri, Fatal> {
    filter(g, out, err, &PREFILTER)
}

/// `MyMRI::getDerfilter()`: `-0.10689 -0.28461 0 0.28461 0.10689`.
pub fn get_derfilter(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write) -> Result<Mri, Fatal> {
    filter(g, out, err, &DERFILTER)
}

/// The prefilter's taps (`float` constants).
pub const PREFILTER: [f32; 5] = [0.03504, 0.24878, 0.43234, 0.24878, 0.03504];
/// The derivative filter's taps.
pub const DERFILTER: [f32; 5] = [-0.10689, -0.28461, 0.0, 0.28461, 0.10689];

fn filter(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    taps: &[f32; 5],
) -> Result<Mri, Fatal> {
    let mut m = mri_alloc(g, out, err, 5, 1, 1, MRI_FLOAT)?;
    let c = m.chunk.as_mut().expect("the kernel's voxels");
    for (i, t) in taps.iter().enumerate() {
        c[4 * i..4 * i + 4].copy_from_slice(&t.to_le_bytes());
    }
    Ok(m)
}

/// `MyMRI::getRand(randpos)`: the entry `randpos` of a fixed table of 101 numbers in `[0, 1)`;
/// a position above 100 is first taken modulo 101 (and given back so). A negative position reads
/// before the table in FreeSurfer (no caller passes one): by default an error here.
pub fn get_rand(randpos: &mut i32) -> f64 {
    if *randpos > 100 {
        *randpos %= 101;
    }
    UNIFORM[usize::try_from(*randpos).expect("getRand: a negative position")]
}

/// `MyMRI::getRand`'s table (`MyMRI.h`).
pub const UNIFORM: [f64; 101] = [
    0.825960894329359,
    0.391799656122844,
    0.606822280998442,
    0.923192169644637,
    0.772662867376799,
    0.394913419972908,
    0.562648884384293,
    0.821213453107628,
    0.077777173551299,
    0.536306917657496,
    0.514107239245956,
    0.761227511728186,
    0.978425890910315,
    0.157030594951506,
    0.147496203406248,
    0.783403101626463,
    0.637363106003275,
    0.257349279177570,
    0.109403726553149,
    0.136954287985675,
    0.654171911591805,
    0.204476823923606,
    0.164700938803686,
    0.566376934840801,
    0.854095286894540,
    0.430581644078462,
    0.397193805113614,
    0.045536084142315,
    0.140620297119782,
    0.724179303888238,
    0.210184975302471,
    0.346103835004574,
    0.226683913788046,
    0.308131967172401,
    0.451070768412025,
    0.831843821825446,
    0.086760750364257,
    0.854135129404520,
    0.742590231477529,
    0.053858310273945,
    0.122791324135168,
    0.526967625411493,
    0.320307444448230,
    0.520062337463421,
    0.878893684505441,
    0.443226585521493,
    0.320014638636649,
    0.868171615160687,
    0.764138797526944,
    0.864611801159897,
    0.083244000805187,
    0.340050247951733,
    0.581607039757426,
    0.291315250124114,
    0.467637373824935,
    0.596640965216293,
    0.897387480194169,
    0.121159463814373,
    0.018178277226624,
    0.747666956017887,
    0.670108792666926,
    0.092391423049263,
    0.917528192569663,
    0.347581829497044,
    0.857229173664249,
    0.632564501155682,
    0.363593396666436,
    0.385827512659221,
    0.003355759148503,
    0.969283884272977,
    0.459201707491767,
    0.800715562489883,
    0.296945204007638,
    0.619917383791628,
    0.748749776544515,
    0.394902341568457,
    0.354479046607957,
    0.867388084434840,
    0.627567204750354,
    0.984294463747630,
    0.824472893444168,
    0.295272747204669,
    0.927251152590456,
    0.119266587069099,
    0.241265513320426,
    0.527775796652046,
    0.060203502196487,
    0.835363100840541,
    0.148398316485924,
    0.121273963075904,
    0.683207184266160,
    0.500002874704566,
    0.561265939626174,
    0.234256657698203,
    0.486854859925734,
    0.071141206079346,
    0.442630693187859,
    0.327200604299592,
    0.226827609433358,
    0.971944076189183,
    0.302612670611030,
];

/// `MyMRI::subSample(mri_src, mri_dst, fixheader, randpos)`: a volume of `width / 2`, `height /
/// 2`, `depth / 2` voxels (a depth of 1 kept), every frame; voxel `(x, y, z)` is the source's `(2x
/// + dx, 2y + dy, 2z + dz)` (`MRIgetVoxVal`, `MRIsetVoxVal`), the offsets 0 unless `randpos >= 0`,
/// then each `(int) (2 getRand(randpos))` (x, y, and z unless the result is one slice deep), the
/// position advanced after each. Without a destination, `MRIallocSequence` of the source's type
/// and frames with the source's header. `fixheader`: the voxel sizes, `thick` and `ps` doubled,
/// `imnr1 = imnr0 + depth - 1`, and the centre `(c_r, c_a, c_s)` the source's voxel-to-RAS matrix
/// (`float`, `MatrixMultiply`) of `(width / 2 + 0.5, height / 2 + 0.5, depth / 2 + 0.5)`
/// (integer halves), then `MRIreInitCache`.
#[allow(clippy::too_many_arguments)]
pub fn sub_sample(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    fixheader: bool,
    mut randpos: i32,
) -> Result<Ret<Mri>, Fatal> {
    let w = src.vg.width / 2;
    let h = src.vg.height / 2;
    let mut d = src.vg.depth / 2;
    if src.vg.depth == 1 {
        d = 1;
    }
    let mut fresh = None;
    let dst: &mut Mri = match dst {
        Some(m) => m,
        None => {
            let mut m = mri_alloc_sequence(g, out, err, w, h, d, src.mtype, src.nframes)?;
            mri_copy_header(g, out, err, src, Some(&mut m))?;
            fresh.insert(m)
        }
    };
    let mut dz = 0;
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                if randpos >= 0 {
                    let dx = (2.0 * get_rand(&mut randpos)) as i32;
                    randpos += 1;
                    let dy = (2.0 * get_rand(&mut randpos)) as i32;
                    randpos += 1;
                    if d > 1 {
                        dz = (2.0 * get_rand(&mut randpos)) as i32;
                        randpos += 1;
                    }
                    for f in 0..src.nframes {
                        let v =
                            mri_get_vox_val(g, err, src, 2 * x + dx, 2 * y + dy, 2 * z + dz, f, 0)?;
                        mri_set_vox_val(g, err, dst, x, y, z, f, v, 0)?;
                    }
                } else {
                    for f in 0..src.nframes {
                        let v = mri_get_vox_val(g, err, src, 2 * x, 2 * y, 2 * z, f, 0)?;
                        mri_set_vox_val(g, err, dst, x, y, z, f, v, 0)?;
                    }
                }
            }
        }
    }
    if fixheader {
        dst.imnr0 = src.imnr0;
        dst.imnr1 = src.imnr0.wrapping_add(dst.vg.depth).wrapping_sub(1);
        dst.vg.xsize = src.vg.xsize * 2.0;
        dst.vg.ysize = src.vg.ysize * 2.0;
        dst.vg.zsize = src.vg.zsize * 2.0;
        dst.thick = src.thick * 2.0;
        dst.ps = src.ps * 2.0;
        let mut c = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
        // `mri_src->width/2+0.5`: an int plus a double, stored as a float
        c.set_relt(1, 1, (f64::from(src.vg.width / 2) + 0.5) as f32);
        c.set_relt(2, 1, (f64::from(src.vg.height / 2) + 0.5) as f32);
        c.set_relt(3, 1, (f64::from(src.vg.depth / 2) + 0.5) as f32);
        c.set_relt(4, 1, 1.0);
        let v2r = extract_i_to_r(g, out, err, &src.vg)?;
        let p = matrix_multiply(g, out, err, Some(&v2r), Some(&c), None)?.unwrap_new();
        dst.vg.c_r = p.relt(1, 1);
        dst.vg.c_a = p.relt(2, 1);
        dst.vg.c_s = p.relt(3, 1);
        reinit_cache(g, out, err, &mut dst.vg)?;
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MyMRI::getBlur(mriS, mriT)`: `MRIconvolveGaussian(mriS, mriT, getPrefilter())`.
pub fn get_blur(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
) -> Result<Ret<Mri>, Fatal> {
    let k = get_prefilter(g, out, err)?;
    mri_convolve_gaussian(g, out, err, p, &k)
}

/// The partial derivatives and the smoothed image of [`get_partials`]: `fz` is `None` for an
/// image one slice deep.
#[derive(Debug)]
pub struct Partials {
    pub fx: Mri,
    pub fy: Mri,
    pub fz: Option<Mri>,
    pub blur: Mri,
}

/// `MyMRI::getPartials(mri, fx, fy, fz, blur)`: `MRIclone`s of the image, then for each frame
/// `f` the separable products of the derivative filter `D` and the prefilter `P`
/// (`MRIconvolve1d`, the axes in this order): `fx = P_z P_y D_x`, `fy = P_z D_y P_x`, `fz = D_z
/// P_y P_x`, `blur = P_z P_y P_x`, each written into frame `f`; an image one slice deep skips the
/// z pass (`MRIcopyFrame` instead) and has no `fz`. `fz` takes the convolution's result: none
/// after a convolution that fails (a destination of a type it does not write: an `fz` of the image's
/// type int, long or unsigned short), then a new one-frame float volume for the next frame.
pub fn get_partials(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
) -> Result<Partials, Fatal> {
    let pre = PREFILTER;
    let der = DERFILTER;
    let is2d = mri.vg.depth == 1;
    let mut fx = mri_clone(g, out, err, mri, None)?.unwrap_new();
    let mut fy = mri_clone(g, out, err, mri, None)?.unwrap_new();
    let mut fz = if is2d {
        None
    } else {
        Some(mri_clone(g, out, err, mri, None)?.unwrap_new())
    };
    let mut blur = mri_clone(g, out, err, mri, None)?.unwrap_new();
    for f in 0..mri.nframes {
        // fx
        let mdx = mri_convolve1d(g, out, err, mri, None, &der, MRI_WIDTH, f, 0)?.unwrap_new();
        let mdxby = mri_convolve1d(g, out, err, &mdx, None, &pre, MRI_HEIGHT, 0, 0)?.unwrap_new();
        if is2d {
            mri_copy_frame(g, out, err, Pair::Given(&mdxby, &mut fx), 0, f)?;
        } else {
            mri_convolve1d(g, out, err, &mdxby, Some(&mut fx), &pre, MRI_DEPTH, 0, f)?;
        }
        // fy
        let mbx = mri_convolve1d(g, out, err, mri, None, &pre, MRI_WIDTH, f, 0)?.unwrap_new();
        let mbxdy = mri_convolve1d(g, out, err, &mbx, None, &der, MRI_HEIGHT, 0, 0)?.unwrap_new();
        let mbxby = mri_convolve1d(g, out, err, &mbx, None, &pre, MRI_HEIGHT, 0, 0)?.unwrap_new();
        if is2d {
            mri_copy_frame(g, out, err, Pair::Given(&mbxdy, &mut fy), 0, f)?;
        } else {
            mri_convolve1d(g, out, err, &mbxdy, Some(&mut fy), &pre, MRI_DEPTH, 0, f)?;
        }
        // fz (from mbxby): `outfz = MRIconvolve1d(mbxby, outfz, ...)`, so a convolution that
        // fails (a destination type it does not write) leaves no fz, and the next frame's goes
        // into a new float volume
        if !is2d {
            fz = match mri_convolve1d(g, out, err, &mbxby, fz.as_mut(), &der, MRI_DEPTH, 0, f)? {
                Ret::New(m) => Some(m),
                Ret::Given => fz,
                Ret::Null => None,
            };
        }
        // blur
        if is2d {
            mri_copy_frame(g, out, err, Pair::Given(&mbxby, &mut blur), 0, f)?;
        } else {
            mri_convolve1d(g, out, err, &mbxby, Some(&mut blur), &pre, MRI_DEPTH, 0, f)?;
        }
    }
    Ok(Partials { fx, fy, fz, blur })
}

/// `MyMRI::findRightSize(mri, conform_size, conform)`: the extent that covers the image with
/// voxels of `conform_size`: `(int) ceil(size_mm / conform_size - 0.0001)` per axis (`size_mm`
/// the absolute voxel size times the extent, in `double`), the depth 1 for an image one slice
/// deep. `conform`: a cube instead, of `(int) ceil(largest side / conform_size)` voxels (the
/// depth counted only when deeper than one slice), at least 256, and 256 when within 10 % above.
pub fn find_right_size(g: &mut Globals, mri: &Mri, conform_size: f32, conform: bool) -> [i32; 3] {
    let xsize = f64::from(mri.vg.xsize.abs());
    let ysize = f64::from(mri.vg.ysize.abs());
    let zsize = f64::from(mri.vg.zsize.abs());
    let fwidth = mul64(xsize, f64::from(mri.vg.width));
    let fheight = mul64(ysize, f64::from(mri.vg.height));
    let fdepth = mul64(zsize, f64::from(mri.vg.depth));
    let cs = wide(conform_size);
    let eps = 0.0001;
    let ceil_int = |g: &mut Globals, v: f64| {
        larmorx_freesurfer_core::affine::float_to_int(g, v.ceil(), "MyMRI::findRightSize")
    };
    let mut ret = [
        ceil_int(g, sub64(div64(fwidth, cs), eps)),
        ceil_int(g, sub64(div64(fheight, cs), eps)),
        ceil_int(g, sub64(div64(fdepth, cs), eps)),
    ];
    if mri.vg.depth == 1 {
        ret[2] = 1;
    }
    if !conform {
        return ret;
    }
    let mut fmax = fwidth;
    if fheight > fmax {
        fmax = fheight;
    }
    if mri.vg.depth > 1 && fdepth > fmax {
        fmax = fdepth;
    }
    let mut conform_width = ceil_int(g, div64(fmax, cs));
    if conform_width < 256 {
        conform_width = 256;
    } else if (f64::from(conform_width) - 256.0) / 256.0 < 0.1 {
        conform_width = 256;
    }
    ret = [conform_width; 3];
    if mri.vg.depth == 1 {
        ret[2] = 1;
    }
    ret
}

/// `MyMRI::getBackground(mri)`: the minimum and maximum of frame 0 (from voxel `(0, 0, 0)`,
/// `MRIgetVoxVal`), then the counts of voxels within `(max - min) / 255` of each (`max - v < eps`,
/// `v - min < eps`, in `float`): the minimum when more voxels sit at it, else the maximum.
pub fn get_background(g: &mut Globals, err: &mut dyn Write, mri: &Mri) -> Result<f32, Fatal> {
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let mut minval = mri_get_vox_val(g, err, mri, 0, 0, 0, 0, 0)?;
    let mut maxval = minval;
    let mut vals = Vec::with_capacity((w.max(0) * h.max(0) * d.max(0)) as usize);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let val = mri_get_vox_val(g, err, mri, x, y, z, 0, 0)?;
                if val > maxval {
                    maxval = val;
                }
                if val < minval {
                    minval = val;
                }
                vals.push(val);
            }
        }
    }
    let (mut mins, mut maxs) = (0i32, 0i32);
    // `(maxval-minval)/255.0`: the float difference through double, stored as a float
    let eps = larmorx_freesurfer_core::sse::div64(wide(larmorx_freesurfer_core::sse::sub(maxval, minval)), 255.0) as f32;
    for &val in &vals {
        if larmorx_freesurfer_core::sse::sub(maxval, val) < eps {
            maxs = maxs.wrapping_add(1);
        }
        if larmorx_freesurfer_core::sse::sub(val, minval) < eps {
            mins = mins.wrapping_add(1);
        }
    }
    Ok(if mins > maxs { minval } else { maxval })
}

/// `MyMRI::MRIlinearTransform(mriS, mriT, m)`: FreeSurfer's `MRIlinearTransform` with `m`
/// converted to a `float` `MATRIX` (`MyMatrix::convertVNL2MATRIX`).
pub fn mri_linear_transform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    m: &Mat4,
) -> Result<Ret<Mri>, Fatal> {
    let mm: Matrix = to_matrix(g, out, err, m)?;
    xform::mri_linear_transform(g, out, err, src, dst, Some(&mm))
}
