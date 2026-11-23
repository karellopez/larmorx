// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_robust_register --test <n> <image>` (queue 3.2-13, relay 48): `Registration::testRobust`
//! and `Registration::buildGaussianPyramid` (`Registration.cpp` 1315-1700 and 3053-3141).
//!
//! The image read (`MRIread`; ` Read MRI <name> successfully!` whatever the read gave), its Gaussian
//! pyramid (each level the previous one blurred by the kernel 1/16, 1/4, 3/8, 1/4, 1/16 and every
//! second voxel taken at the fixed table's random offsets, `MyMRI::subSample` with the header fixed; the first level the image blurred;
//! levels until one is below 16 voxels along an axis, that one dropped), then a pair of images made
//! from the pyramid's second level by the test `n` (written into the folder), registered by the
//! robust registration with an intensity scale and no geometric model at verbose level 2
//! (`setSourceAndTarget`, `computeMultiresRegistration(0, 5, 0.01)`); status 0, or 1 for an unknown
//! test.
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-robust-register.md` §8):
//! - **FS-1076** (out of bounds): an image of 16 to 31 voxels along its smallest axis gives a
//!   pyramid of one level, and `gpS[1]` reads past the vector's end: the level the pyramid dropped
//!   (its pointer still in the vector's storage). By default an error after the test's line,
//!   status 1; `reproduce` uses that dropped level, as FreeSurfer does.
//! - **FS-1077** (crash): an image that does not read: FreeSurfer prints its read error and
//!   ` Read MRI ... successfully!`, then dies of a segmentation fault in the pyramid. An error in
//!   both modes.
//! - **FS-1078** (memory: type): tests 5 and 6 write their pattern with `MRIvox`, a byte access,
//!   into the pyramid's `float` images: each value lands in byte `x` of row `(y, z)`, so the bytes
//!   of the first floats of the row change. By default the values are written as voxel values;
//!   `reproduce` writes the bytes.

use std::io::Write;

use larmorx_core::defects::{FS_1076, FS_1077, FS_1078, UpstreamDefects};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::matrix_ops::{
    Z_ROTATION, matrix_alloc_rotation, matrix_identity, matrix_inverse,
};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_filter::conv::{mri_convolve_gaussian, mri_gaussian1d};
use larmorx_freesurfer_io::mriio::{mri_read_volume, mri_write};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_UNKNOWN, Mri};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_volume::arith::Pair;
use larmorx_freesurfer_volume::xform::mri_linear_transform_interp;

use crate::mymri::{mri_valscale, sub_sample};
use crate::registration::{Io, RegRobust, SAMPLE_TRILINEAR};
use crate::regstep::Cost;

/// The kernel of the pyramids: `MRIgaussian1d(1.08, 5)` with its taps set to 1/16, 1/4, 3/8, 1/4,
/// 1/16.
fn kernel(g: &mut Globals, io: &mut Io) -> Result<Mri, Fatal> {
    let mut k = mri_gaussian1d(g, io.out, io.err, 1.08, 5)?;
    for (x, v) in [0.0625f32, 0.25, 0.375, 0.25, 0.0625]
        .into_iter()
        .enumerate()
    {
        mri_set_vox_val(g, io.err, &mut k, x as i32, 0, 0, 0, v, 0)?;
    }
    Ok(k)
}

fn convolve(g: &mut Globals, io: &mut Io, src: &Mri, k: &Mri) -> Result<Mri, Fatal> {
    match mri_convolve_gaussian(g, io.out, io.err, Pair::New(src), k)? {
        Ret::New(m) => Ok(m),
        _ => Err(nothing(io.err, "MRIconvolveGaussian")),
    }
}

fn subsample(g: &mut Globals, io: &mut Io, src: &Mri) -> Result<Mri, Fatal> {
    // `subSample(mri_tmp, NULL, true)`: `randpos` defaults to 0 (the table's positions)
    match sub_sample(g, io.out, io.err, src, None, true, 0)? {
        Ret::New(m) => Ok(m),
        _ => Err(nothing(io.err, "MyMRI::subSample")),
    }
}

fn nothing(err: &mut dyn Write, what: &str) -> Fatal {
    let _ = writeln!(
        err,
        "Registration: {what} returned nothing (FreeSurfer crashes here)"
    );
    Fatal::Exit(1)
}

fn write(g: &mut Globals, io: &mut Io, m: &mut Mri, name: &str) -> Result<(), Fatal> {
    mri_write(
        g,
        io.out,
        io.err,
        io.env,
        io.dir,
        m,
        name,
        None,
        MGZ_INTENT_UNKNOWN,
    )?;
    Ok(())
}

impl RegRobust {
    /// `buildGaussianPyramid(mri_in, min, max)`: the levels and the level dropped last (the one
    /// below `min`, whose pointer stays in the vector's storage past its end: FS-1076). `None` as
    /// the image: FreeSurfer's first line, then FS-1077's error.
    pub fn build_gaussian_pyramid(
        &self,
        g: &mut Globals,
        io: &mut Io,
        mri_in: Option<&Mri>,
        min: i32,
        mut max: i32,
    ) -> Result<(Vec<Mri>, Option<Mri>), Fatal> {
        if self.verbose > 0 {
            let _ = writeln!(
                io.out,
                "   - Build Gaussian Pyramid ( min: {min} max: {max} ) "
            );
        }
        let Some(mri_in) = mri_in else {
            g.notices.add(
                &FS_1077,
                "the test mode's image did not read (FreeSurfer dereferences it)",
            );
            let _ = io.out.flush();
            let _ = writeln!(
                io.err,
                "Registration::buildGaussianPyramid: no image (FreeSurfer crashes here)"
            );
            return Err(Fatal::Exit(1));
        };
        let (w, h, d) = (mri_in.vg.width, mri_in.vg.height, mri_in.vg.depth);
        if max == -1 {
            max = w + h + d;
        }
        if w < min || h < min || d < min {
            let _ = writeln!(io.out, " Input image is smaller than min dimension: {min}");
            let _ = writeln!(
                io.out,
                " Input dimensions (after conformalizing): {w} {h} {d}"
            );
            let _ = writeln!(io.out, " Specify larger minimum ...");
            return Err(Fatal::Exit(1));
        }
        let n = d;
        if self.verbose > 1 {
            let _ = writeln!(io.out, "        dim: {w} {h} {d}");
        }
        let k = kernel(g, io)?;
        let mut p0 = convolve(g, io, mri_in, &k)?;
        // `mri_tmp`: the image the next level is made from, `mri_in` until a level replaces it
        let mut from_input = true;
        while p0.vg.width > max || p0.vg.height > max || p0.vg.depth > max {
            let blurred = convolve(g, io, if from_input { mri_in } else { &p0 }, &k)?;
            p0 = subsample(g, io, &blurred)?;
            from_input = false;
        }
        let _ = writeln!(io.out, " downsample2 ");
        p0.outside_val = mri_in.outside_val;
        let mut p = vec![p0];
        let mut i = 1;
        while i < n {
            let last = p.last().expect("a level");
            if last.vg.width < min || last.vg.height < min || last.vg.depth < min {
                break;
            }
            let src: &Mri = if i == 1 && from_input { mri_in } else { last };
            let blurred = convolve(g, io, src, &k)?;
            let mut next = subsample(g, io, &blurred)?;
            next.outside_val = mri_in.outside_val;
            p.push(next);
            i += 1;
        }
        // `p.resize(i - 1)`: the last level made (below `min`) leaves the vector; the loop always
        // ends there (the depth halves faster than `i` grows), FreeSurfer's `assert` otherwise
        let dropped = if i < n { p.pop() } else { None };
        Ok((p, dropped))
    }

    /// `testRobust(fname, testno)`; ends the program (`Err`: status 0 after the registration, 1 for
    /// an unknown test or an error).
    pub fn test_robust(
        &mut self,
        g: &mut Globals,
        io: &mut Io,
        fname: &str,
        testno: i32,
    ) -> Result<(), Fatal> {
        let _ = writeln!(io.out, " testRobust {fname}  testno: {testno}");
        let mri = mri_read_volume(g, io.out, io.err, io.env, io.dir, fname)?;
        let _ = writeln!(io.out, " Read MRI {fname} successfully!");
        let (gp, dropped) = self.build_gaussian_pyramid(g, io, mri.as_ref(), 16, -1)?;
        drop(mri);
        let title = match testno {
            0 => "Identity",
            1 => "Translation",
            2 => "Rotation",
            3 => "Intensity",
            4 | 9 => "Rotation and Translation",
            5 => "Translation and Noise",
            6 => "Rotation and Noise",
            7 => "Scaling",
            30 => "Save GP lowest",
            _ => {
                let _ = writeln!(io.out, "Test {testno}unknown!");
                return Err(Fatal::Exit(1));
            }
        };
        let _ = writeln!(io.out, "Test {testno} : {title}");
        if testno == 30 {
            let mut low = gp.into_iter().last().expect("a level");
            write(g, io, &mut low, "gps-low.mgz")?;
            return Err(Fatal::Exit(0));
        }
        // `gpS[gpS.size() - level]` with `level = gpS.size() - 1`: the second level
        let second: &Mri = if gp.len() >= 2 {
            &gp[1]
        } else {
            g.notices.add(
                &FS_1076,
                "the test mode's pyramid has one level (FreeSurfer reads the level it dropped)",
            );
            match (g.mode() == UpstreamDefects::Reproduce, dropped.as_ref()) {
                (true, Some(d)) => d,
                _ => {
                    let _ = io.out.flush();
                    let _ = writeln!(
                        io.err,
                        "Registration::testRobust: the image's pyramid has one level, the test needs two (FreeSurfer reads past it)"
                    );
                    return Err(Fatal::Exit(1));
                }
            }
        };
        let interp = |g: &mut Globals, io: &mut Io, m: &Matrix| -> Result<Mri, Fatal> {
            match mri_linear_transform_interp(
                g,
                io.out,
                io.err,
                second,
                None,
                Some(m),
                SAMPLE_TRILINEAR,
            )? {
                Ret::New(m) => Ok(m),
                _ => Err(nothing(io.err, "MRIlinearTransformInterp")),
            }
        };
        let ident = |g: &mut Globals, io: &mut Io| -> Result<Matrix, Fatal> {
            match matrix_identity(g, io.out, io.err, 4, None)? {
                Ret::New(m) => Ok(m),
                _ => Err(nothing(io.err, "MatrixIdentity")),
            }
        };
        let rotation = |g: &mut Globals, io: &mut Io, theta: f64| -> Result<Matrix, Fatal> {
            match matrix_alloc_rotation(g, io.out, io.err, 4, theta as f32, Z_ROTATION)? {
                Ret::New(m) => Ok(m),
                _ => Err(nothing(io.err, "MatrixAllocRotation")),
            }
        };
        // the source and target moved by `a` and its inverse
        let moved = |g: &mut Globals, io: &mut Io, a: &Matrix| -> Result<(Mri, Mri), Fatal> {
            let ai = match matrix_inverse(g, io.out, io.err, Some(a), None)? {
                Ret::New(m) => m,
                _ => return Err(nothing(io.err, "MatrixInverse")),
            };
            Ok((interp(g, io, a)?, interp(g, io, &ai)?))
        };
        let copy = |g: &mut Globals, io: &mut Io| -> Result<Mri, Fatal> {
            Ok(mri_copy(g, io.out, io.err, second, None)?.unwrap_new())
        };
        let translate = |a: &mut Matrix, t: [f32; 3]| {
            a.set_relt(1, 4, t[0]);
            a.set_relt(2, 4, t[1]);
            a.set_relt(3, 4, t[2]);
        };
        let (mut s, mut t, names) = match testno {
            0 => (copy(g, io)?, copy(g, io)?, ("idS.mgz", "idT.mgz")),
            1 => {
                let mut a = ident(g, io)?;
                translate(&mut a, [2.2, 0.3, 1.4]);
                let (s, t) = moved(g, io, &a)?;
                (s, t, ("transS.mgz", "transT.mgz"))
            }
            2 => {
                let a = rotation(g, io, 0.08)?;
                let (s, t) = moved(g, io, &a)?;
                (s, t, ("rotS.mgz", "rotT.mgz"))
            }
            3 => {
                let s = copy(g, io)?;
                let t = match mri_valscale(g, io.out, io.err, Pair::New(second), 0.8, 0.0)? {
                    Ret::New(m) => m,
                    _ => return Err(nothing(io.err, "MyMRI::MRIvalscale")),
                };
                // the two swapped
                (t, s, ("iscaleS.mgz", "iscaleT.mgz"))
            }
            4 | 9 => {
                let mut a = rotation(g, io, 0.08)?;
                translate(&mut a, [2.2, 0.3, 1.4]);
                let (s, mut t) = moved(g, io, &a)?;
                // test 4 a scaled copy, test 9 in place: the same voxels
                mri_valscale(g, io.out, io.err, Pair::Same(&mut t), 0.8, 0.0)?;
                let names = if testno == 4 {
                    ("rottransS.mgz", "rottransT.mgz")
                } else {
                    ("rottransintS.mgz", "rottransintT.mgz")
                };
                (s, t, names)
            }
            5 | 6 => {
                let a = if testno == 5 {
                    let mut a = ident(g, io)?;
                    translate(&mut a, [0.2, 0.3, 0.4]);
                    a
                } else {
                    rotation(g, io, 0.02)?
                };
                let (mut s, t) = moved(g, io, &a)?;
                junk(g, io, &mut s)?;
                let names = if testno == 5 {
                    ("junktransS.mgz", "junktransT.mgz")
                } else {
                    ("junkrotS.mgz", "junkrotT.mgz")
                };
                (s, t, names)
            }
            _ => {
                let mut a = ident(g, io)?;
                a.set_relt(1, 1, 1.01);
                a.set_relt(2, 2, 1.04);
                a.set_relt(3, 3, 1.06);
                let (s, t) = moved(g, io, &a)?;
                (s, t, ("scaleS.mgz", "scaleT.mgz"))
            }
        };
        write(g, io, &mut s, names.0)?;
        write(g, io, &mut t, names.1)?;
        let _ = writeln!(io.out, " Transformed , now registering ...");
        self.transonly = false;
        self.rigid = false;
        self.costfun = Cost::Rob;
        self.iscale = true;
        self.set_verbose(2);
        self.set_source_and_target(g, io, &s, &t, false)?;
        drop(s);
        drop(t);
        drop(gp);
        self.compute_multires_registration(g, io, 0, 5, 0.01)?;
        Err(Fatal::Exit(0))
    }
}

/// Tests 5 and 6: `MRIvox(mriTs, rr, cc, dd) = rr + cc + dd + 1` over the first third of each
/// axis. `MRIvox` is a byte access: in an image of another type FreeSurfer writes byte `rr` of the
/// row (FS-1078, `reproduce`); by default the value is the voxel's.
fn junk(g: &mut Globals, io: &mut Io, m: &mut Mri) -> Result<(), Fatal> {
    let (w, h, d) = (m.vg.width / 3, m.vg.height / 3, m.vg.depth / 3);
    let bytes = m.mtype != larmorx_freesurfer_mri::mri::MRI_UCHAR;
    if bytes && w > 0 && h > 0 && d > 0 {
        g.notices.add(
            &FS_1078,
            "the test mode's pattern written as bytes into an image of another type",
        );
    }
    for dd in 0..d {
        for cc in 0..h {
            for rr in 0..w {
                let v = rr + cc + dd + 1;
                if bytes && g.mode() == UpstreamDefects::Reproduce {
                    let i = (dd as u64 * m.vox_per_slice + cc as u64 * m.vox_per_row) as usize
                        * m.bytes_per_vox
                        + rr as usize;
                    let c = m.chunk.as_mut().expect("the voxels");
                    c[i] = v as u8;
                } else {
                    mri_set_vox_val(g, io.err, m, rr, cc, dd, 0, v as f32, 0)?;
                }
            }
        }
    }
    Ok(())
}
