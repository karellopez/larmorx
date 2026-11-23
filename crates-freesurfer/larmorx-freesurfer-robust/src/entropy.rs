// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MyMRI::entropyImage(mri, radius, ball, correction, mask)` and `MyMRI::get3Dcorrection`
//! (queue 3.2-13, `mri_robust_register --cost ROBENT`), translated from FreeSurfer 8.2.0's
//! `mri_robust_register/MyMRI.cpp` (1177-1631): each voxel's local entropy, the Shannon entropy
//! of the intensities (0..255) of a box (or ball) of side `2 radius + 1` around it, weighted by a
//! normalised Gaussian (or, with `correction`, of the intensity densities of the box's
//! tetrahedra), as a `float` image of the source's header.
//!
//! - The image is first made bytes (`MRISeqchangeType(mri, MRI_UCHAR, 0, 0.999, FALSE)`, printed
//!   as `  - changing data type from <t> to 0 (noscale = 0)...`) unless it is bytes; a mask is
//!   resampled onto it (`MRIgetVoxelToVoxelXform`, `MRIlinearTransformInterp`, nearest), a voxel
//!   inside where its value truncated to `int` is above 0.
//! - The Gaussian: `sigma = (2 radius + 1) / (4 sqrt(2 log 2))`, the weights `exp(-xs - ys - zs)`
//!   with `xs = (x - radius)^2 / (2 sigma^2)` (`z` 0 in 2-D), normalised by their sum.
//! - Each voxel within `radius` of the border (one more on the far side with `correction`) and
//!   each voxel outside the mask is 0. Every other voxel: a 256-bin histogram of the box's
//!   voxels (`histo[I] += w`, the box in `z`, `y`, `x` order; with `ball`, the box's voxels
//!   outside the ball skipped), its sum, then `entropy -= p log p` for each bin above 0 (`p =
//!   histo / sum`), stored as `float`; a negative or NaN entropy prints `ERROR in
//!   MyMRI::entropyImage: entropy is negative or nan ?` on standard error (never reached: the
//!   weights are positive). With `correction` each box voxel adds the densities of five
//!   tetrahedra of its cube (four corners in 2-D) instead of a weight; the image is then moved
//!   half a voxel (`MRIlinearTransformInterp` by a translation of 0.5, cubic B-spline).
//! - FreeSurfer's OpenMP loop over the slices computes each voxel alone: here slices on the
//!   given threads, each voxel's arithmetic in FreeSurfer's order (the result does not depend
//!   on the threads); the thread count printed is FreeSurfer's `omp_get_max_threads()`.
//! - `log` and `exp` through [`larmorx_core::math::Libm`]: correctly rounded by default, glibc
//!   2.35's with `reproduce` (a filter of many values: nothing named).
//!
//! Defects (`larmorx_core::defects`): **FS-1074** (rule B): a radius below 0 asks for a box of
//! a negative side: FreeSurfer aborts (3-D: `new double[]` of a negative count throws
//! `std::bad_array_new_length`) or, in 2-D, computes every voxel from an empty histogram (an
//! image of zeros); by default an error (status 1), `reproduce` FreeSurfer's (the abort's lines
//! and status 134, or the zeros); the same where `(2 radius + 1)^3` passes `int`'s range
//! (FreeSurfer writes past its weights there: an error in both modes).

use std::io::Write;

use larmorx_core::cfmt::ostream;
use larmorx_core::defects::FS_1074;
use larmorx_core::math::Libm;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix_ops::matrix_identity;
use larmorx_freesurfer_core::romp;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc, mri_copy_header};
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_volume::orient::mri_get_voxel_to_voxel_xform;
use larmorx_freesurfer_volume::resize::mri_seq_change_type;
use larmorx_freesurfer_volume::sample::{SAMPLE_CUBIC_BSPLINE, SAMPLE_NEAREST};
use larmorx_freesurfer_volume::xform::mri_linear_transform_interp;

/// The histogram's bins (`nbins`).
const NBINS: usize = 256;

/// libstdc++'s lines when `new T[n]` of a negative `n` throws and nobody catches it.
const BAD_ARRAY_ABORT: &str = "terminate called after throwing an instance of 'std::bad_array_new_length'\n  what():  std::bad_array_new_length\n";

/// x86-64's `cvttss2si`: a `float` truncated to `int`, the `int` minimum for a NaN or a value
/// outside the range.
fn cvtt32(v: f32) -> i32 {
    if v.is_nan() || v >= 2_147_483_648.0 || v < -2_147_483_648.0 {
        i32::MIN
    } else {
        v as i32
    }
}

/// x86-64's `cvttsd2si`: a `double` truncated to `int`, the `int` minimum for a NaN or a value
/// outside the range.
fn cvtt64(v: f64) -> i32 {
    if v.is_nan() || v >= 2_147_483_648.0 || v < -2_147_483_648.0 {
        i32::MIN
    } else {
        v as i32
    }
}

/// `MyMRI::get3Dcorrection(histo, v1, v2, v3, v4, intRange)`: the density of intensities on a
/// tetrahedron of corner values `v1..v4` (sorted: `I3 <= I2 <= I1 <= I0`) added to `histo`: a
/// single 1 at `I3` when all four are equal; else the three pieces (from `I3` to `I2`, `I2` to
/// `I1`, `I1` to `I0`, each sampled at its integers with the ends moved in by 0.25 and halved,
/// a later piece's first value replacing an earlier piece's last), normalised by their sum.
pub fn get_3d_correction(histo: &mut [f64], v1: u32, v2: u32, v3: u32, v4: u32, int_range: u32) {
    let mut v = [v1, v2, v3, v4];
    v.sort_unstable();
    let (i3, i2, i1, i0) = (v[0], v[1], v[2], v[3]);
    let d = i3;
    let c = i2.wrapping_sub(d);
    let b = i1.wrapping_sub(d);
    let a = i0.wrapping_sub(d);
    let n = int_range as usize;
    // FreeSurfer's `vector<double> tempPDF(intRange, 0.0)` (the bins are 256: on the stack)
    let mut stack = [0.0f64; NBINS];
    let mut heap = Vec::new();
    let temp_pdf: &mut [f64] = if n <= NBINS {
        &mut stack[..n]
    } else {
        heap.resize(n, 0.0);
        &mut heap
    };
    // `3.0f / a`: a float division, then widened
    let three_a = f64::from(3.0f32 / a as f32);
    if a == 0 && b == 0 && c == 0 {
        temp_pdf[i3 as usize] = 1.0;
    } else {
        // one piece: its samples `lo..=hi` (the ends moved in by 0.25), `f` of each, the ends
        // halved, written over `temp_pdf[lo..=hi]`
        let piece = |temp_pdf: &mut [f64], lo: u32, hi: u32, f: &dyn Fn(f64) -> f64| {
            for i in lo..=hi {
                let mut t = f64::from(i);
                if i == lo {
                    t += 0.25;
                }
                if i == hi {
                    t -= 0.25;
                }
                let mut p = f(t);
                if i == lo {
                    p *= 0.5;
                }
                if i == hi {
                    p *= 0.5;
                }
                temp_pdf[i as usize] = p;
            }
        };
        let (fb, fc) = (f64::from(b), f64::from(c));
        if i3 != i2 {
            // 3.0f / a * (t - I3) / b * (t - I3) / c, left to right
            piece(temp_pdf, i3, i2, &|t| {
                three_a * (t - f64::from(i3)) / fb * (t - f64::from(i3)) / fc
            });
        }
        if i2 != i1 {
            let (bc, ac) = (f64::from(b - c), f64::from(a - c));
            piece(temp_pdf, i2, i1, &|t| {
                three_a
                    * ((((f64::from(i1) - t) / bc) * ((t - f64::from(i3)) / fb))
                        + (((f64::from(i0) - t) / ac) * ((t - f64::from(i2)) / bc)))
            });
        }
        if i1 != i0 {
            let (ac, ab) = (f64::from(a - c), f64::from(a - b));
            piece(temp_pdf, i1, i0, &|t| {
                three_a * (((f64::from(i0) - t) / ac) * ((f64::from(i0) - t) / ab))
            });
        }
        let mut sum = 0.0f64;
        for &p in temp_pdf.iter() {
            sum += p;
        }
        for p in temp_pdf.iter_mut() {
            *p /= sum;
        }
    }
    for (h, p) in histo.iter_mut().zip(temp_pdf.iter()) {
        *h += *p;
    }
}

/// The parameters of one entropy image.
#[derive(Clone, Copy, Debug)]
pub struct EntropyParams {
    /// `--entradius` (5 by default).
    pub radius: i32,
    /// `--entball`: the ball inside the box.
    pub ball: bool,
    /// `--entcorrection`: the tetrahedra's densities.
    pub correction: bool,
    /// The threads (0: every logical CPU; `--n-threads` or `OMP_NUM_THREADS`).
    pub threads: usize,
}

/// `MyMRI::entropyImage(mri, radius, ball, correction, mask)` (see the module's documentation):
/// the new `float` image, or the program's end (`Err`).
pub fn entropy_image(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    mri: &mut Mri,
    p: EntropyParams,
    mask: Option<&Mri>,
) -> Result<Mri, Fatal> {
    let EntropyParams {
        radius,
        ball,
        correction,
        ..
    } = p;
    if mri.nframes > 1 {
        let _ = writeln!(err, "MyMRI::entropyImage multiple frames not supported!");
        return Err(Fatal::Exit(1));
    }
    if correction && radius > 1 {
        let _ = writeln!(
            out,
            "WARNING: using correction mode with radius > 1 may take a very long time !"
        );
        let _ = writeln!(
            out,
            "         Correction usually allows small radis (e.g. 1 in 3D)."
        );
    }
    let (width, height, depth) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let is2d = depth == 1;

    let local_mask = match mask {
        Some(mk) => {
            let m = mri_get_voxel_to_voxel_xform(g, out, err, mk, mri)?;
            let mut lm = mri_copy(g, out, err, mri, None)?
                .into_new()
                .expect("MRIcopy's new volume");
            mri_linear_transform_interp(
                g,
                out,
                err,
                mk,
                Some(&mut lm),
                m.as_ref(),
                SAMPLE_NEAREST,
            )?;
            Some(lm)
        }
        None => None,
    };

    let converted;
    let src: &Mri = if mri.mtype != MRI_UCHAR {
        let _ = writeln!(
            out,
            "  - changing data type from {} to {MRI_UCHAR} (noscale = 0)...",
            mri.mtype
        );
        match mri_seq_change_type(g, out, err, env, mri, MRI_UCHAR, 0.0, 0.999, 0)? {
            Some(m) => {
                converted = m;
                &converted
            }
            None => {
                let _ = writeln!(out, "ERROR: MRISeqchangeType");
                return Err(Fatal::Exit(1));
            }
        }
    } else {
        mri
    };
    // the bytes (`(int)(value * 1.0)`) and the mask's 0 or 1, column fastest
    let (w, h, d) = (
        width.max(0) as usize,
        height.max(0) as usize,
        depth.max(0) as usize,
    );
    let mut mri_in = vec![0u8; w * h * d];
    let mut mri_mask: Option<Vec<u8>> = local_mask.as_ref().map(|_| vec![0u8; w * h * d]);
    let mut k = 0usize;
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                let v = mri_get_vox_val(g, err, src, x, y, z, 0, 0)?;
                mri_in[k] = cvtt64(f64::from(v) * (NBINS as f64 / 256.0)) as u8;
                if let (Some(lm), Some(mm)) = (local_mask.as_ref(), mri_mask.as_mut()) {
                    let t = cvtt32(mri_get_vox_val(g, err, lm, x, y, z, 0, 0)?);
                    mm[k] = u8::from(t > 0);
                }
                k += 1;
            }
        }
    }
    drop(local_mask);

    // the Gaussian cube
    let libm = Libm::quiet(g.mode());
    let ssize = radius.wrapping_mul(2).wrapping_add(1);
    let ln2 = libm.log(2.0);
    let mut sigma = f64::from(ssize) / (4.0 * (2.0 * ln2).sqrt());
    let _ = writeln!(
        out,
        "  - compute Gaussian cube( size: {ssize} )  sigma: {}",
        ostream(sigma)
    );
    let zssize = if is2d { 1 } else { ssize };
    let gsize = i64::from(ssize) * i64::from(ssize) * i64::from(zssize);
    let gsize32 = ssize.wrapping_mul(ssize).wrapping_mul(zssize);
    if gsize32 < 0 || gsize != i64::from(gsize32) || radius < 0 {
        g.notices.add(
            &FS_1074,
            format!(
                "MyMRI::entropyImage: radius {radius}: a box of {ssize} voxels a side (FreeSurfer aborts or reads an empty histogram)"
            ),
        );
        let fs_zeros = radius < 0 && gsize32 >= 0 && gsize == i64::from(gsize32);
        if g.notices.reproduce() && gsize32 < 0 {
            let _ = out.flush();
            let _ = err.write_all(BAD_ARRAY_ABORT.as_bytes());
            return Err(Fatal::Exit(134));
        }
        if !(g.notices.reproduce() && fs_zeros) {
            let _ = writeln!(
                err,
                "MyMRI::entropyImage: the entropy radius {radius} gives no box of voxels (FreeSurfer fails here)"
            );
            return Err(Fatal::Exit(1));
        }
    }
    let gsize = gsize.max(0) as usize;
    let mut gk = vec![0.0f64; gsize];
    let mut sum = 0.0f64;
    let mut count = 0usize;
    sigma = 2.0 * sigma * sigma;
    let mut zs = 0.0f64;
    for z in 0..zssize.max(0) {
        if !is2d {
            zs = f64::from(z - radius);
            zs = zs * zs / sigma;
        }
        for y in 0..ssize.max(0) {
            let mut ys = f64::from(y - radius);
            ys = ys * ys / sigma;
            for x in 0..ssize.max(0) {
                let mut xs = f64::from(x - radius);
                xs = xs * xs / sigma;
                let dtmp = libm.exp(-xs - ys - zs);
                gk[count] = dtmp;
                count += 1;
                sum += dtmp;
            }
        }
    }
    for v in gk.iter_mut() {
        *v /= sum;
    }

    let mut ent = mri_alloc(g, out, err, width, height, depth, MRI_FLOAT)?;
    mri_copy_header(g, out, err, mri, Some(&mut ent))?;
    ent.mtype = MRI_FLOAT;

    let mut xmax = width - radius - 1;
    let mut ymax = height - radius - 1;
    let mut zmax = depth - radius - 1;
    if correction {
        xmax -= 1;
        ymax -= 1;
        zmax -= 1;
    }
    if is2d {
        zmax = 0;
    }
    let procs = std::thread::available_parallelism().map_or(1, |n| n.get());
    let asked = if p.threads == 0 { procs } else { p.threads };
    let shown = romp::max_threads(asked);
    if correction {
        let _ = writeln!(
            out,
            "  - compute entropy image now with correction (threads={shown})... "
        );
    } else {
        let _ = writeln!(out, "  - compute entropy image now (threads={shown})... ");
    }

    let geo = Geo {
        width,
        height,
        depth,
        radius,
        ball,
        correction,
        xmax,
        ymax,
        zmax,
        is2d,
    };
    let slice_bytes = (ent.vox_per_slice * 4) as usize;
    let row_elems = ent.vox_per_row as usize;
    let chunk = ent.chunk.as_mut().expect("MRIalloc's voxels");
    let mut slices: Vec<(i32, &mut [u8])> = chunk
        .chunks_mut(slice_bytes.max(1))
        .take(d)
        .enumerate()
        .map(|(z, s)| (z as i32, s))
        .collect();
    let nt = asked.clamp(1, d.max(1));
    let (mri_in, mri_mask, gk, libm) = (&mri_in, mri_mask.as_deref(), &gk, &libm);
    // the slices round-robin (OpenMP's `schedule(static,1)`), each thread's messages per slice
    let mut per: Vec<Vec<(i32, &mut [u8])>> = (0..nt).map(|_| Vec::new()).collect();
    for (k, s) in slices.drain(..).enumerate() {
        per[k % nt].push(s);
    }
    let mut messages: Vec<(i32, usize)> = std::thread::scope(|sc| {
        let handles: Vec<_> = per
            .into_iter()
            .map(|mine| {
                sc.spawn(move || {
                    let mut msgs = Vec::new();
                    let mut histo = [0.0f64; NBINS];
                    for (z, bytes) in mine {
                        let n = slice(
                            &geo, z, bytes, row_elems, mri_in, mri_mask, gk, libm, &mut histo,
                        );
                        if n > 0 {
                            msgs.push((z, n));
                        }
                    }
                    msgs
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("an entropy thread"))
            .collect()
    });
    messages.sort_unstable();
    for (_, n) in messages {
        for _ in 0..n {
            let _ = writeln!(
                err,
                " ERROR in MyMRI::entropyImage: entropy is negative or nan ? "
            );
        }
    }

    if correction {
        // correct for the half voxel shift
        let mut t1 = matrix_identity(g, out, err, 4, None)?
            .into_new()
            .expect("MatrixIdentity's new matrix");
        t1.set_relt(1, 4, 0.5);
        t1.set_relt(2, 4, 0.5);
        t1.set_relt(3, 4, 0.5);
        ent =
            mri_linear_transform_interp(g, out, err, &ent, None, Some(&t1), SAMPLE_CUBIC_BSPLINE)?
                .into_new()
                .expect("MRIlinearTransformInterp's new volume");
    }
    Ok(ent)
}

/// The image's extent and the loop's limits.
#[derive(Clone, Copy)]
struct Geo {
    width: i32,
    height: i32,
    depth: i32,
    radius: i32,
    ball: bool,
    correction: bool,
    xmax: i32,
    ymax: i32,
    zmax: i32,
    is2d: bool,
}

/// One slice `z` of the entropy image into `bytes` (its `float`s, rows of `row` elements; the
/// border and the voxels outside the mask stay 0): the count of negative or NaN entropies.
#[allow(clippy::too_many_arguments)]
fn slice(
    geo: &Geo,
    z: i32,
    bytes: &mut [u8],
    row: usize,
    mri_in: &[u8],
    mri_mask: Option<&[u8]>,
    gk: &[f64],
    libm: &Libm,
    histo: &mut [f64; NBINS],
) -> usize {
    let Geo {
        width,
        height,
        depth,
        radius,
        ball,
        correction,
        xmax,
        ymax,
        zmax,
        is2d,
    } = *geo;
    let r2 = radius.wrapping_mul(radius);
    let wtimesh = (width as usize) * (height as usize);
    let w = width as usize;
    let mut bad = 0usize;
    for y in 0..height {
        for x in 0..width {
            if depth > 1 {
                if x < radius || y < radius || z < radius || x > xmax || y > ymax || z > zmax {
                    continue;
                }
            } else if x < radius || y < radius || x > xmax || y > ymax {
                continue;
            }
            if let Some(mm) = mri_mask {
                let mpos = ((z * height + y) * width + x) as usize;
                if mm[mpos] == 0 {
                    continue;
                }
            }
            histo.fill(0.0);
            let mut count = 0usize;
            let (zstart, zend) = if depth == 1 {
                (0, 1)
            } else {
                (-radius, radius + 1)
            };
            let mut z2 = 0;
            let mut y2 = 0;
            for zz in zstart..zend {
                if ball {
                    z2 = zz * zz - r2;
                }
                let zp = (z + zz) * height;
                for yy in -radius..=radius {
                    if ball {
                        y2 = z2 + yy * yy;
                    }
                    let yp = ((y + yy) + zp) * width;
                    for xx in -radius..=radius {
                        if ball {
                            let x2 = y2 + xx * xx;
                            if x2 > 0 {
                                count += 1;
                                continue;
                            }
                        }
                        let pos = ((x + xx) + yp) as usize;
                        if correction {
                            if is2d {
                                get_3d_correction(
                                    histo,
                                    u32::from(mri_in[pos]),
                                    u32::from(mri_in[pos + 1]),
                                    u32::from(mri_in[pos + w]),
                                    u32::from(mri_in[pos + w + 1]),
                                    NBINS as u32,
                                );
                            } else {
                                let v = |o: usize| u32::from(mri_in[pos + o]);
                                let (v1, v2, v3, v4) = (v(0), v(1), v(w), v(wtimesh));
                                let (v5, v6) = (v(w + 1), v(wtimesh + 1));
                                let (v7, v8) = (v(wtimesh + w), v(wtimesh + w + 1));
                                let nb = NBINS as u32;
                                get_3d_correction(histo, v1, v2, v3, v4, nb);
                                get_3d_correction(histo, v5, v2, v3, v8, nb);
                                get_3d_correction(histo, v7, v8, v3, v4, nb);
                                get_3d_correction(histo, v6, v2, v8, v4, nb);
                                get_3d_correction(histo, v8, v2, v3, v4, nb);
                            }
                        } else {
                            histo[mri_in[pos] as usize] += gk[count];
                            count += 1;
                        }
                    }
                }
            }
            let mut histosum = 0.0f64;
            for &hv in histo.iter() {
                histosum += hv;
            }
            let mut entropy = 0.0f64;
            for &hv in histo.iter() {
                if hv > 0.0 {
                    let etmp = hv / histosum;
                    entropy -= etmp * libm.log(etmp);
                }
            }
            if entropy < 0.0 || entropy.is_nan() {
                bad += 1;
            }
            let o = 4 * (y as usize * row + x as usize);
            bytes[o..o + 4].copy_from_slice(&(entropy as f32).to_le_bytes());
        }
    }
    bad
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_tetrahedron_adds_one_at_its_value() {
        let mut h = vec![0.0f64; 256];
        get_3d_correction(&mut h, 7, 7, 7, 7, 256);
        assert_eq!(h[7], 1.0);
        assert_eq!(h.iter().filter(|&&v| v != 0.0).count(), 1);
    }

    #[test]
    fn a_tetrahedron_density_sums_to_one() {
        let mut h = vec![0.0f64; 256];
        get_3d_correction(&mut h, 3, 10, 6, 20, 256);
        let s: f64 = h.iter().sum();
        assert!((s - 1.0).abs() < 1e-12, "{s}");
        assert!(h[2] == 0.0 && h[21] == 0.0 && h[3] > 0.0 && h[20] > 0.0);
    }
}
