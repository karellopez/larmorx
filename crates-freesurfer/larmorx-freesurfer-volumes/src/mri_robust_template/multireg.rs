// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MultiRegistration` (queue 3.2-14): the robust registration of several images into one
//! template, translated from FreeSurfer 8.2.0's `mri_robust_register/MultiRegistration.cpp` and
//! `MultiRegistration.h`, on `fs.robust`'s `RegRobust`.
//!
//! - [`MultiRegistration::load_movables`] (`loadMovables`): the inputs read (several images of one
//!   frame, or the frames of one 4-D image), masked, each converted to cubic B-spline
//!   coefficients, their voxel sizes compared;
//! - [`MultiRegistration::load_ltas`], [`MultiRegistration::load_intensities`]: the initial
//!   transforms (`--ixforms`) and intensity scales (`--iscalein`);
//! - [`MultiRegistration::get_seed`] (`getSeed`): a seed from the images' centres;
//! - [`MultiRegistration::initial_xforms`] (`initialXforms`): each input registered to the
//!   initial target, then (without `--fixtp`) the mean space: the mean translation, the mean
//!   rotation ([`larmorx_freesurfer_robust::mymatrix::rotation_mean`]) and the template's
//!   geometry (`getAverageCosines`, `createTemplateGeo`);
//! - [`MultiRegistration::compute_template`] (`computeTemplate`): the global iterations (each
//!   input registered to the current template, the template recomputed) from the initial
//!   average (`initialAverageSet`: the centroids aligned) or the transforms given;
//! - [`MultiRegistration::map_and_average_mov`] (`mapAndAverageMov`), `averageSet` (mean, median,
//!   the voxel-wise and the joint Tukey biweight) and `averageConformSet` (`--conform`);
//! - the outputs: `writeMean`, `writeConformMean`, `writeLTAs`, `writeMapMovHdr`, `writeWarps`,
//!   `writeIntensities`, `writeWeights`.
//!
//! `halfWayTemplate` has no caller (the program's call is commented out) and is not translated.
//! The official build runs the inputs' registrations on OpenMP threads (`schedule(static, 1)`): the
//! port runs them one after the other, FreeSurfer's order at one thread, which its results depend
//! on (the sums of the centroids, the lines' order; FS-1081).
//!
//! The affine mean space (`--affine` without `--fixtp`) needs the matrix logarithm and exponential
//! (`MyMatrix::GeometricMean`), queue item 3.2-15: it ends here with FreeSurfer's step and `is not
//! ported yet`.

use std::io::Write;

use larmorx_core::cfmt::ostream;
use larmorx_core::defects::{
    FS_1081, FS_1082, FS_1084, FS_1085, FS_1086, FS_1087, FS_1088, FS_1089, FS_1090, FS_1091,
    FS_1092, FS_1093, FS_1094, FS_1095,
};
use larmorx_core::math::Libm;
use larmorx_core::vnl_fixed;
use larmorx_core::vnl_matrix::{
    VnlMatrix, matlab_print_long, matlab_print_long_diag, matlab_print_long_vector,
};
use larmorx_freesurfer_core::cstd::note_glibc07;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_NOFILE, ERROR_OUT_OF_BOUNDS, Fatal};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_copy, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{matrix_subtract, matrix_transpose};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add64, div64, mul64, sub, sub64};
use larmorx_freesurfer_core::utils::resolve;
use larmorx_freesurfer_gca::transform::{Xform, transform_read};
use larmorx_freesurfer_io::mriio::{mri_read_volume, mri_write};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::lta::{LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX, Lta};
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc_header, mri_alloc_sequence,
    mri_copy_header, mri_copy_pulse_parameters, mri_free,
};
use larmorx_freesurfer_mri::vol_geom::{extract_i_to_r, extract_r_to_i, reinit_cache};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_robust::costs::centroid;
use larmorx_freesurfer_robust::debug::print_fixed4;
use larmorx_freesurfer_robust::mymatrix::{
    Mat3, Mat4, convert_matrix2vnl, get_m_from_rt, get_rt_from_m, lta2matrix, mul3,
    polar_decomposition, polar2_decomposition, ras_matrix2lta, rotation_mean, vox_matrix2lta,
};
use larmorx_freesurfer_robust::mymri::{mri_linear_transform, mri_valscale};
use larmorx_freesurfer_robust::registration::{Io, RegRobust, identity4, inv4, mul4};
use larmorx_freesurfer_robust::regression::Regression;
use larmorx_freesurfer_robust::robust_gauss::median_i;
use larmorx_freesurfer_volume::arith::{
    Pair, mri_average, mri_clone, mri_copy_frame, mri_linear_scale,
};
use larmorx_freesurfer_volume::bspline::{MriBspline, lta_transform_bspline, mri_to_bspline};
use larmorx_freesurfer_volume::cma::istream_double;
use larmorx_freesurfer_volume::labels::mri_mask_zero;
use larmorx_freesurfer_volume::ltaapply::{
    lta_modify_src_dst_geom, lta_transform, lta_transform_interp,
};
use larmorx_freesurfer_volume::ltaops::lta_change_type;
use larmorx_freesurfer_volume::ltawrite::lta_write_ex;
use larmorx_freesurfer_volume::orient::mri_set_voxel_to_ras_xform;

use super::options::{Run, SAMPLE_CUBIC_BSPLINE, SAMPLE_NEAREST, SAMPLE_TRILINEAR};

/// `STRLEN`: the names' buffers.
const STRLEN: usize = 1024;

/// The not-ported message of a path that needs a later queue item.
pub const NOT_PORTED: &str = "is not ported yet";

/// `MultiRegistration`'s fields (`MultiRegistration()`'s defaults; the program sets them).
pub struct MultiRegistration {
    pub mov: Vec<String>,
    pub iltas: Vec<String>,
    pub iintens: Vec<String>,
    pub outdir: String,
    pub transonly: bool,
    pub rigid: bool,
    pub robust: bool,
    pub sat: f64,
    pub satit: bool,
    pub debug: i32,
    pub iscale: bool,
    pub iscaleonly: bool,
    pub nomulti: bool,
    pub subsamplesize: i32,
    pub highit: i32,
    pub fixvoxel: bool,
    pub keeptype: bool,
    pub average: i32,
    pub doubleprec: bool,
    pub backupweights: bool,
    pub sampletype: i32,
    pub crascenter: bool,
    pub resthresh: f32,
    pub frobnormthresh: f64,
    pub mri_mov: Vec<Mri>,
    pub mri_bsplines: Vec<Option<MriBspline>>,
    pub ltas: Vec<Option<Lta>>,
    pub mri_warps: Vec<Option<Mri>>,
    /// `mri_weights` (sized only for the robust registration, `robust`).
    pub mri_weights: Vec<Option<Mri>>,
    pub intensities: Vec<f64>,
    pub mri_mean: Option<Mri>,
    /// A write of the template or its outputs failed (FS-1090's default ends with status 1).
    pub write_failed: bool,
}

impl Default for MultiRegistration {
    fn default() -> Self {
        MultiRegistration::new()
    }
}

/// The library's streams from the program's.
fn io<'a>(r: &'a mut Run) -> Io<'a> {
    Io {
        out: &mut *r.out,
        err: &mut *r.err,
        env: r.env,
        dir: r.dir,
        input: None,
    }
}

/// `strncpy(buf, s, STRLEN - 1)`: at most 1,023 bytes of `s` (to its first NUL), the rest of those
/// 1,023 zeroed; the last byte stays.
fn strncpy_name(buf: &mut larmorx_freesurfer_core::cname::CName, s: &str) {
    let b = s.as_bytes();
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    let n = end.min(STRLEN - 1);
    let dst = buf.buffer_mut();
    dst[..n].copy_from_slice(&b[..n]);
    dst[n..STRLEN - 1].fill(0);
}

/// A 3 x 3 matrix as VNL's dynamic one (for its printing and decompositions).
fn dyn3(m: &Mat3) -> VnlMatrix {
    let mut v = VnlMatrix::zeros(3, 3);
    for i in 0..3 {
        for j in 0..3 {
            *v.at_mut(i, j) = m[i][j];
        }
    }
    v
}

/// `vnl_matrix<double> * vnl_matrix_fixed<double, 4, 4>` and the like: VNL's dynamic product
/// (each sum from `+0`).
fn dmul(a: &Mat4, b: &Mat4) -> Mat4 {
    VnlMatrix::from_4x4(a)
        .mul(&VnlMatrix::from_4x4(b))
        .to_4x4()
        .expect("4 x 4")
}

/// The `MATRIX` of a transform's first matrix.
fn m_l(lta: &Lta) -> &Matrix {
    lta.xforms[0].m_l.as_ref().expect("the transform's matrix")
}

impl MultiRegistration {
    /// `MultiRegistration()`: `./`, rigid, robust, saturation 4.685, average 1, cubic B-splines,
    /// a resolution threshold 0.01 and a Frobenius threshold 0.0001.
    pub fn new() -> MultiRegistration {
        MultiRegistration {
            mov: Vec::new(),
            iltas: Vec::new(),
            iintens: Vec::new(),
            outdir: "./".to_owned(),
            transonly: false,
            rigid: true,
            robust: true,
            sat: 4.685,
            satit: false,
            debug: 0,
            iscale: false,
            iscaleonly: false,
            nomulti: false,
            subsamplesize: -1,
            highit: -1,
            fixvoxel: false,
            keeptype: false,
            average: 1,
            doubleprec: false,
            backupweights: false,
            sampletype: SAMPLE_CUBIC_BSPLINE,
            crascenter: false,
            resthresh: 0.01,
            frobnormthresh: 0.0001,
            mri_mov: Vec::new(),
            mri_bsplines: Vec::new(),
            ltas: Vec::new(),
            mri_warps: Vec::new(),
            mri_weights: Vec::new(),
            intensities: Vec::new(),
            mri_mean: None,
            write_failed: false,
        }
    }

    /// `printParams()` (`--debug`): the settings, booleans as `true`/`false`.
    pub fn print_params(&self, out: &mut dyn Write) {
        let b = |x: bool| if x { "true" } else { "false" };
        let mut s = String::new();
        s.push_str("\n MultiRegistration Parameters \n\n");
        s.push_str(&format!(" Outdir:        {}\n", self.outdir));
        s.push_str(&format!(" TransOnly:     {}\n", b(self.transonly)));
        s.push_str(&format!(" Rigid:         {}\n", b(self.rigid)));
        s.push_str(&format!(" Robust:        {}\n", b(self.robust)));
        if self.satit {
            s.push_str(&format!(" Satit:         {}\n", b(self.satit)));
        } else {
            s.push_str(&format!(" Sat:           {}\n", ostream(self.sat)));
        }
        s.push_str(&format!(" Iscale:        {}\n", b(self.iscale)));
        s.push_str(&format!(" IscaleOnly:    {}\n", b(self.iscaleonly)));
        s.push_str(&format!(" NoMulti:       {}\n", b(self.nomulti)));
        s.push_str(&format!(" SubsampleSize: {}\n", self.subsamplesize));
        s.push_str(&format!(" HighIt:        {}\n", self.highit));
        s.push_str(&format!(" FixVoxel:      {}\n", b(self.fixvoxel)));
        s.push_str(&format!(" KeepType:      {}\n", b(self.keeptype)));
        s.push_str(&format!(" Average:       {}\n", self.average));
        s.push_str(&format!(" DoublePrec:    {}\n", b(self.doubleprec)));
        s.push_str(&format!(" BackupWeights: {}\n", b(self.backupweights)));
        s.push_str(&format!(" SampleType:    {}\n", self.sampletype));
        s.push_str(&format!(" CRASCenter:    {}\n", b(self.crascenter)));
        s.push_str(&format!(
            " Resthresh:     {}\n",
            ostream(f64::from(self.resthresh))
        ));
        s.push_str(&format!(
            " Frobnormthresh:     {}\n",
            ostream(self.frobnormthresh)
        ));
        s.push_str(&format!(" Debug:         {}\n", self.debug));
        s.push('\n');
        let _ = out.write_all(s.as_bytes());
    }

    /// `setSampleType(st)`: trilinear, cubic B-spline or nearest; another prints `ERROR
    /// MultiRegistration:setSampleType: <st> not supported type!` and ends with status 1.
    pub fn set_sample_type(&mut self, out: &mut dyn Write, st: i32) -> Result<(), Fatal> {
        match st {
            SAMPLE_TRILINEAR | SAMPLE_CUBIC_BSPLINE | SAMPLE_NEAREST => {}
            _ => {
                let _ = writeln!(
                    out,
                    "ERROR MultiRegistration:setSampleType: {st} not supported type!"
                );
                return Err(Fatal::Exit(1));
            }
        }
        self.sampletype = st;
        Ok(())
    }

    /// `getSeed()`: the absolute values of each image's voxels on the three axes through its
    /// centre (`width / 2`, ...; 20 voxels each way, the centre counted six times), summed in
    /// `double`; 0 gives ` WARNING: image seed is zero, are images really empty at center?`,
    /// otherwise the sum is multiplied by 10 until it is at least 10; then converted to `unsigned
    /// int`. FS-1082: a sum of 2^32 or more (C's conversion is undefined: x86-64 keeps the low 32
    /// bits of the 64-bit integer, 0 from 2^63 on): by default the largest `unsigned int`,
    /// `reproduce` x86-64's bits.
    pub fn get_seed(&self, g: &mut Globals, r: &mut Run) -> Result<u32, Fatal> {
        let mut dseed = 0.0f64;
        for m in &self.mri_mov {
            let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
            let (x, y, z) = (w / 2, h / 2, d / 2);
            for p in 0..20 {
                let (xup, xdown, yup, ydown, zup, zdown) =
                    (x + p, x - p, y + p, y - p, z + p, z - p);
                let mut add = |g: &mut Globals, c: i32, rr: i32, s: i32| -> Result<(), Fatal> {
                    let v = mri_get_vox_val(g, r.err, m, c, rr, s, 0, 0)?;
                    dseed = add64(dseed, f64::from(v.abs()));
                    Ok(())
                };
                if xdown >= 0 {
                    add(g, xdown, y, z)?;
                }
                if ydown >= 0 {
                    add(g, x, ydown, z)?;
                }
                if zdown >= 0 {
                    add(g, x, y, zdown)?;
                }
                if xup < w {
                    add(g, xup, y, z)?;
                }
                if yup < h {
                    add(g, x, yup, z)?;
                }
                if zup < d {
                    add(g, x, y, zup)?;
                }
            }
        }
        if dseed == 0.0 {
            let _ = writeln!(
                r.out,
                "WARNING: image seed is zero, are images really empty at center?"
            );
        } else {
            while dseed < 10.0 {
                dseed = mul64(dseed, 10.0);
            }
        }
        if dseed >= 4_294_967_296.0 {
            g.notices.add(
                &FS_1082,
                format!(
                    "an image seed of {} does not fit an unsigned int (C's conversion is undefined)",
                    ostream(dseed)
                ),
            );
            if !g.notices.reproduce() {
                return Ok(u32::MAX);
            }
            // cvttsd2si into a 64-bit register, its low 32 bits (the indefinite integer above 2^63)
            let wide = if dseed >= 9_223_372_036_854_775_808.0 {
                i64::MIN
            } else {
                dseed as i64
            };
            return Ok(wide as u32);
        }
        Ok(dseed as u32)
    }

    /// `loadMovables(pmov, pmasks, AllowDiffVoxSize)`: see the module's notes. Returns the number
    /// of inputs.
    pub fn load_movables(
        &mut self,
        g: &mut Globals,
        r: &mut Run,
        pmov: &[String],
        pmasks: &[String],
        allow_diff_vox_size: bool,
    ) -> Result<usize, Fatal> {
        let mut n = pmov.len();
        let mut m = pmasks.len();
        self.mov = pmov.to_vec();
        if n == 1 {
            let _ = writeln!(r.out, "trying to read 4D source '{}'...", self.mov[0]);
            let _ = r.out.flush();
            let Some(mri4d) = mri_read_volume(g, r.out, r.err, r.env, r.dir, &self.mov[0])? else {
                log::error_exit(
                    g,
                    r.err,
                    ERROR_NOFILE,
                    &format!(
                        "MultiRegistration::loadMovables: could not open input volume {}.\n",
                        self.mov[0]
                    ),
                )?;
                unreachable!("ErrorExit returns only for code 0");
            };
            n = mri4d.nframes.max(0) as usize;
            let _ = writeln!(r.out, " Input has {} frames.", mri4d.nframes);
            if n == 1 {
                let _ = r.out.flush();
                let _ = writeln!(r.err, "ERROR: only single 3D volume passed!");
                let _ = writeln!(r.err, "    Pass several inputs, or 4D input volume");
                return Err(Fatal::Exit(1));
            }
            let mut mask4d: Option<Mri> = None;
            if m == 1 {
                mask4d = mri_read_volume(g, r.out, r.err, r.env, r.dir, &pmasks[0])?;
                let Some(mk) = mask4d.as_ref() else {
                    // FS-1085: FreeSurfer checks the input again instead of the mask, then reads
                    // the mask's frames through NULL
                    g.notices.add(
                        &FS_1085,
                        format!(
                            "the 4-D input's mask {} does not read (FreeSurfer checks the input instead and crashes)",
                            pmasks[0]
                        ),
                    );
                    if g.notices.reproduce() {
                        let _ = r.out.flush();
                        return Err(Fatal::Exit(139));
                    }
                    log::error_exit(
                        g,
                        r.err,
                        ERROR_NOFILE,
                        &format!(
                            "MultiRegistration::loadMovables: could not open input volume {}.\n",
                            pmasks[0]
                        ),
                    )?;
                    unreachable!("ErrorExit returns only for code 0");
                };
                m = mk.nframes.max(0) as usize;
                if n != m {
                    let _ = r.out.flush();
                    let _ = writeln!(
                        r.err,
                        "ERROR: frame count in mask does not agree with frames in mov!"
                    );
                    return Err(Fatal::Exit(1));
                }
            } else if m > 1 {
                // FS-1086: several masks for one 4-D input: FreeSurfer reads none and copies the
                // frames of a NULL mask
                g.notices.add(
                    &FS_1086,
                    format!(
                        "{m} masks for one 4-D input (FreeSurfer reads none and crashes on the first frame)"
                    ),
                );
                if g.notices.reproduce() {
                    // the first frame's lines, then the crash in MRIcopyFrame of the mask
                    let first = mri_copy_frame(g, r.out, r.err, Pair::New(&mri4d), 0, 0)?;
                    drop(first);
                    let _ = writeln!(r.out, "masking source frame 0 ...");
                    let _ = r.out.flush();
                    return Err(Fatal::Exit(139));
                }
                let _ = r.out.flush();
                let _ = writeln!(
                    r.err,
                    "ERROR: pass one 4D mask (--masks) for a 4D input, not {m}"
                );
                return Err(Fatal::Exit(1));
            }
            self.mov = vec![String::new(); n];
            self.mri_mov = Vec::with_capacity(n);
            self.mri_bsplines = Vec::with_capacity(n);
            for i in 0..n {
                let mut f = mri_copy_frame(g, r.out, r.err, Pair::New(&mri4d), i as i32, 0)?
                    .into_new()
                    .expect("MRIcopyFrame's new volume");
                if m > 0 {
                    let _ = writeln!(r.out, "masking source frame {i} ...");
                    let mk = mask4d.as_ref().expect("the mask");
                    let mask = mri_copy_frame(g, r.out, r.err, Pair::New(mk), i as i32, 0)?
                        .into_new()
                        .expect("MRIcopyFrame's new volume");
                    mri_mask_zero(g, r.out, r.err, Pair::Same(&mut f), &mask)?;
                }
                let bs = if self.sampletype == SAMPLE_CUBIC_BSPLINE {
                    let _ = writeln!(r.out, "converting source frame {i} to bspline ...");
                    Some(mri_to_bspline(g, r.out, r.err, &f, None, 3)?)
                } else {
                    None
                };
                self.mri_mov.push(f);
                self.mri_bsplines.push(bs);
                self.mov[i] = format!("Frame_{i}");
            }
            drop(mri4d);
            // MRIfree(&mask4d) of a NULL mask: FreeSurfer's error message
            mri_free(g, r.err, &mut mask4d);
        } else {
            if m > 0 && m < n {
                // FS-1084: FreeSurfer reads the masks past the end of their list
                g.notices.add(
                    &FS_1084,
                    format!(
                        "{m} masks for {n} inputs (FreeSurfer reads the missing masks' names from memory past the list)"
                    ),
                );
            }
            self.mri_mov = Vec::with_capacity(n);
            self.mri_bsplines = Vec::with_capacity(n);
            for i in 0..n {
                let _ = writeln!(r.out, "reading source '{}'...", self.mov[i]);
                let _ = r.out.flush();
                let Some(mut mi) = mri_read_volume(g, r.out, r.err, r.env, r.dir, &self.mov[i])?
                else {
                    log::error_exit(
                        g,
                        r.err,
                        ERROR_NOFILE,
                        &format!(
                            "MultiRegistration::loadMovables: could not open input volume {}.\n",
                            self.mov[i]
                        ),
                    )?;
                    unreachable!("ErrorExit returns only for code 0");
                };
                if mi.nframes != 1 {
                    log::error_exit(
                        g,
                        r.err,
                        ERROR_NOFILE,
                        &format!(
                            "MultiRegistration::loadMovables: only pass single frame MRI {}.\n",
                            self.mov[i]
                        ),
                    )?;
                }
                if m > 0 {
                    let _ = writeln!(r.out, "masking source frame {i} ...");
                    if i >= m {
                        let _ = r.out.flush();
                        let _ = writeln!(
                            r.err,
                            "ERROR: no mask for input {} (--masks names {m} for {n} inputs)",
                            i + 1
                        );
                        return Err(Fatal::Exit(1));
                    }
                    let _ = r.out.flush();
                    let Some(mask) = mri_read_volume(g, r.out, r.err, r.env, r.dir, &pmasks[i])?
                    else {
                        log::error_exit(
                            g,
                            r.err,
                            ERROR_NOFILE,
                            &format!(
                                "MultiRegistration::loadMovables: could not open mask volume {}.\n",
                                pmasks[i]
                            ),
                        )?;
                        unreachable!("ErrorExit returns only for code 0");
                    };
                    if mask.nframes != 1 {
                        log::error_exit(
                            g,
                            r.err,
                            ERROR_NOFILE,
                            &format!(
                                "MultiRegistration::loadMovables: only pass single frame mask {}.\n",
                                pmasks[i]
                            ),
                        )?;
                    }
                    mri_mask_zero(g, r.out, r.err, Pair::Same(&mut mi), &mask)?;
                }
                let bs = if self.sampletype == SAMPLE_CUBIC_BSPLINE {
                    let _ = writeln!(r.out, "converting source '{}' to bspline ...", self.mov[i]);
                    Some(mri_to_bspline(g, r.out, r.err, &mi, None, 3)?)
                } else {
                    None
                };
                self.mri_mov.push(mi);
                self.mri_bsplines.push(bs);
            }
            let eps = self.resthresh;
            for i in 1..n {
                let (a, b) = (&self.mri_mov[i].vg, &self.mri_mov[0].vg);
                if sub(a.xsize, b.xsize).abs() > eps
                    || sub(a.ysize, b.ysize).abs() > eps
                    || sub(a.zsize, b.zsize).abs() > eps
                {
                    let f = |v: f32| ostream(f64::from(v));
                    if !allow_diff_vox_size {
                        let _ = r.out.flush();
                        let _ = writeln!(
                            r.err,
                            "ERROR: MultiRegistration::loadMovables: images have different voxel sizes."
                        );
                        let _ = writeln!(
                            r.err,
                            "  Currently not supported, maybe first make conform?"
                        );
                        let _ = writeln!(
                            r.err,
                            "  Debug info: size({i}) = {}, {}, {}   size(0) = {}, {}, {}",
                            f(a.xsize),
                            f(a.ysize),
                            f(a.zsize),
                            f(b.xsize),
                            f(b.ysize),
                            f(b.zsize)
                        );
                        log::error_exit(
                            g,
                            r.err,
                            ERROR_BADFILE,
                            &format!(
                                "MultiRegistration::loadMovables: voxel size is different {}.\n",
                                self.mov[i]
                            ),
                        )?;
                    } else {
                        let c = |v: f32| larmorx_core::cfmt::g(f64::from(v), 6);
                        let _ = writeln!(
                            r.out,
                            "WARNING: MultiRegistration::loadMovables: images have different voxel sizes."
                        );
                        let _ = writeln!(
                            r.out,
                            "  mov[0] = ({},{},{}) mov[{i}] = ({},{},{})",
                            c(b.xsize),
                            c(b.ysize),
                            c(b.zsize),
                            c(a.xsize),
                            c(a.ysize),
                            c(a.zsize)
                        );
                        let _ = writeln!(r.out, "  BUT AllowDiffVoxSize set so continuing ...");
                    }
                }
            }
        }
        let n = self.mri_mov.len();
        self.mri_warps = (0..n).map(|_| None).collect();
        self.intensities = vec![1.0; n];
        self.ltas = (0..n).map(|_| None).collect();
        if self.robust {
            self.mri_weights = (0..n).map(|_| None).collect();
        }
        Ok(n)
    }

    /// `loadLTAs(nltas)`: each name read by `TransformRead` (`identity.nofile` takes the input's
    /// geometry for both), its geometries required (`MultiRegistration::loadLTAs no source
    /// geometry, use lta with valid geometry ( <name> )`, the same for the target, `ERROR_BADFILE`),
    /// made voxel to voxel. Returns the number of transforms the inputs have room for.
    ///
    /// - FS-1087 (crash): a name that does not read: FreeSurfer reads the transform through NULL
    ///   (a segmentation fault, status 139); by default FreeSurfer's own message for it
    ///   (`MultiRegistration::loadLTAs could not read transform file <name>`, `ERROR_BADFILE`).
    /// - FS-1088 (memory): a morph (`.m3z`) is read as a transform array; an error in both modes.
    /// - FS-1089 (crash): one name for a 4-D input of several frames: the other frames keep no
    ///   transform, which FreeSurfer then applies (a segmentation fault); by default an error.
    pub fn load_ltas(
        &mut self,
        g: &mut Globals,
        r: &mut Run,
        nltas: &[String],
    ) -> Result<usize, Fatal> {
        self.iltas = nltas.to_vec();
        for (i, name) in nltas.iter().enumerate() {
            let trans = transform_read(g, r.out, r.err, r.env, r.dir, name)?;
            let Some(trans) = trans else {
                g.notices.add(
                    &FS_1087,
                    format!(
                        "--ixforms {name} does not read (FreeSurfer reads the transform through NULL)"
                    ),
                );
                if g.notices.reproduce() {
                    let _ = r.out.flush();
                    return Err(Fatal::Exit(139));
                }
                log::error_exit(
                    g,
                    r.err,
                    ERROR_BADFILE,
                    &format!("MultiRegistration::loadLTAs could not read transform file {name}"),
                )?;
                unreachable!("ErrorExit returns only for code 0");
            };
            let mut lta = match trans.xform {
                Xform::Lta(l) => l,
                Xform::Morph(_) => {
                    g.notices.add(
                        &FS_1088,
                        "--ixforms of a morph: FreeSurfer reads the morph as a transform array",
                    );
                    log::error_exit(
                        g,
                        r.err,
                        ERROR_BADFILE,
                        &format!(
                            "MultiRegistration::loadLTAs: {name} is not a linear transform (FreeSurfer reads the morph as one here)\n"
                        ),
                    )?;
                    unreachable!("ErrorExit returns only for code 0");
                }
            };
            if name == "identity.nofile" {
                if let Some(mi) = self.mri_mov.get(i) {
                    lta_modify_src_dst_geom(g, r.out, r.err, &mut lta, Some(mi), Some(mi));
                }
            }
            if lta.xforms[0].src.valid == 0 {
                log::error_exit(
                    g,
                    r.err,
                    ERROR_BADFILE,
                    &format!(
                        "MultiRegistration::loadLTAs no source geometry, use lta with valid geometry ( {name} )"
                    ),
                )?;
            }
            if lta.xforms[0].dst.valid == 0 {
                log::error_exit(
                    g,
                    r.err,
                    ERROR_BADFILE,
                    &format!(
                        "MultiRegistration::loadLTAs no target geometry, use lta with valid geometry ( {name} )"
                    ),
                )?;
            }
            lta_change_type(g, r.out, r.err, r.env, &mut lta, LINEAR_VOX_TO_VOX)?;
            if i < self.ltas.len() {
                self.ltas[i] = Some(lta);
            }
        }
        if nltas.len() < self.ltas.len() {
            g.notices.add(
                &FS_1089,
                format!(
                    "{} initial transforms for {} inputs (the frames of a 4-D input): FreeSurfer applies the missing ones through NULL",
                    nltas.len(),
                    self.ltas.len()
                ),
            );
            if !g.notices.reproduce() {
                let _ = r.out.flush();
                let _ = writeln!(
                    r.err,
                    "ERROR: --ixforms names {} transforms for {} frames: pass one per frame (FreeSurfer crashes)",
                    nltas.len(),
                    self.ltas.len()
                );
                return Err(Fatal::Exit(1));
            }
        }
        Ok(self.ltas.len())
    }

    /// `loadIntensities(nintens)`: each file's first number as C++'s `>>` reads it; a file that
    /// does not open: `MultiRegistration::loadIntensities no such file ( <name> )`, `ERROR_BADFILE`.
    /// Returns the number of inputs.
    pub fn load_intensities(
        &mut self,
        g: &mut Globals,
        r: &mut Run,
        nintens: &[String],
    ) -> Result<usize, Fatal> {
        self.iintens = nintens.to_vec();
        for (i, name) in nintens.iter().enumerate() {
            let path = resolve(r.dir, name);
            match std::fs::read(&path) {
                Ok(text) => {
                    if i < self.intensities.len() {
                        istream_double(&text, &mut self.intensities[i]);
                    }
                }
                // an ifstream opens a folder too (its reads then fail: the scale stays)
                Err(_) if path.is_dir() => {}
                Err(e) => {
                    g.errno = log::errno_of(&e).unwrap_or(2);
                    log::error_exit(
                        g,
                        r.err,
                        ERROR_BADFILE,
                        &format!("MultiRegistration::loadIntensities no such file ( {name} )"),
                    )?;
                }
            }
        }
        Ok(self.intensities.len())
    }

    /// `initRegistration(R)`: rigid or affine, the intensity scale, translation only, intensity
    /// only, the robust cost, the saturation, double precision, the subsampling (when above 0) and
    /// the highest resolution's iterations (when 0 or more).
    fn init_registration(&self, reg: &mut RegRobust) {
        if self.rigid {
            reg.set_rigid();
        } else {
            reg.set_affine();
        }
        reg.iscale = self.iscale;
        if self.transonly {
            reg.set_transonly();
        }
        if self.iscaleonly {
            reg.set_iscale_only();
        }
        reg.costfun = larmorx_freesurfer_robust::regstep::Cost::Rob;
        reg.sat = self.sat;
        reg.doubleprec = self.doubleprec;
        if self.subsamplesize > 0 {
            reg.subsamplesize = self.subsamplesize;
        }
        if self.highit >= 0 {
            reg.highit = self.highit;
        }
    }

    /// Writes `m` as `name` (`MRIwrite`): whether it worked.
    fn write(g: &mut Globals, r: &mut Run, m: &mut Mri, name: &str) -> Result<bool, Fatal> {
        let e = mri_write(
            g,
            r.out,
            r.err,
            r.env,
            r.dir,
            m,
            name,
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
        Ok(e == 0)
    }

    /// One input mapped by its transform (`LTAtransformBSpline` of its coefficients, or
    /// `LTAtransformInterp` of the image with the sample type), into `dst` when given.
    fn warp(
        &mut self,
        g: &mut Globals,
        r: &mut Run,
        i: usize,
        dst: Option<Mri>,
    ) -> Result<Mri, Fatal> {
        let mut lta = self.ltas[i].take().expect("the input's transform");
        let res = (|| {
            let mut dst = dst;
            let got = if self.sampletype == SAMPLE_CUBIC_BSPLINE {
                let bs = self.mri_bsplines[i].as_ref().expect("the coefficients");
                lta_transform_bspline(g, r.out, r.err, r.env, bs, dst.as_mut(), &mut lta)?
            } else {
                lta_transform_interp(
                    g,
                    r.out,
                    r.err,
                    r.env,
                    &self.mri_mov[i],
                    dst.as_mut(),
                    &mut lta,
                    self.sampletype,
                )?
            };
            Ok(match got {
                Ret::New(m) => m,
                _ => dst.expect("the given destination"),
            })
        })();
        self.ltas[i] = Some(lta);
        res
    }

    /// FS-1093: the inputs' maps applied with no transform (`--inittp 0` with `--noit` or two
    /// inputs, and no `--ixforms`): FreeSurfer reads the transform through NULL.
    fn no_transform(&self, g: &mut Globals, r: &mut Run) -> Result<(), Fatal> {
        g.notices.add(
            &FS_1093,
            "the inputs mapped with no transform (--inittp 0 without --ixforms, and --noit or two inputs: FreeSurfer reads the transform through NULL)",
        );
        let _ = r.out.flush();
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        let _ = writeln!(
            r.err,
            "ERROR: no initial transforms: --inittp 0 needs --ixforms, or more than two inputs without --noit (FreeSurfer crashes here)"
        );
        Err(Fatal::Exit(1))
    }

    /// `mapAndAverageMov(itdebug)`: each input mapped by its transform (`iscaleonly`: only its
    /// intensity scaled, `creating intitial template (iscale only)`), its pulse parameters, scaled
    /// by its intensity factor (`iscale`), written as `<outdir>tp<i>_to_template-it<itdebug>.mgz`
    /// with `--debug`; then [`MultiRegistration::average_set`].
    pub fn map_and_average_mov(
        &mut self,
        g: &mut Globals,
        r: &mut Run,
        itdebug: i32,
    ) -> Result<(), Fatal> {
        let nin = self.mri_mov.len();
        if self.iscaleonly {
            let _ = writeln!(r.out, "creating intitial template (iscale only)");
            for i in 0..nin {
                let s = self.intensities[i];
                let w = match self.mri_warps[i].take() {
                    Some(mut w) => {
                        mri_valscale(
                            g,
                            r.out,
                            r.err,
                            Pair::Given(&self.mri_mov[i], &mut w),
                            s,
                            0.0,
                        )?;
                        w
                    }
                    None => mri_valscale(g, r.out, r.err, Pair::New(&self.mri_mov[i]), s, 0.0)?
                        .into_new()
                        .expect("MRIvalscale's new volume"),
                };
                self.mri_warps[i] = Some(w);
                if self.debug != 0 {
                    let name = format!("{}tp{}_to_template-it{itdebug}.mgz", self.outdir, i + 1);
                    let w = self.mri_warps[i].as_mut().expect("the warp");
                    Self::write(g, r, w, &name)?;
                }
            }
        } else {
            let _ = writeln!(r.out, "mapping movs and creating initial template...");
            if self.iscale {
                let _ = writeln!(r.out, " allow intensity scaling");
            }
            if self.ltas.iter().any(Option::is_none) {
                return self.no_transform(g, r);
            }
            for i in 0..nin {
                self.mri_warps[i] = None;
                let mut w = self.warp(g, r, i, None)?;
                mri_copy_pulse_parameters(&self.mri_mov[i], &mut w);
                if self.iscale {
                    mri_valscale(
                        g,
                        r.out,
                        r.err,
                        Pair::Same(&mut w),
                        self.intensities[i],
                        0.0,
                    )?;
                }
                if self.debug != 0 {
                    let name = format!("{}tp{}_to_template-it{itdebug}.mgz", self.outdir, i + 1);
                    Self::write(g, r, &mut w, &name)?;
                }
                self.mri_warps[i] = Some(w);
            }
        }
        let warps: Vec<&Mri> = self
            .mri_warps
            .iter()
            .map(|w| w.as_ref().expect("the warp"))
            .collect();
        let mean = self.mri_mean.take();
        let mean = average_set(g, r, &warps, mean, self.average, self.sat)?;
        self.mri_mean = Some(mean);
        Ok(())
    }

    /// `averageConformSet(itdebug)` (`--conform`): each input mapped by `LTAtransform`
    /// (trilinear) into its transform's target, ` MultiRegistration::averageConformSet not
    /// implemented, yet !!!` on standard error for each (FreeSurfer's assertion after it is
    /// compiled out), scaled by its intensity, written with `--debug` as
    /// `<outdir>tp<i>_to_template_conform-it<itdebug>.mgz`; then their average (FS-1091: the
    /// template is not made conform).
    pub fn average_conform_set(
        &mut self,
        g: &mut Globals,
        r: &mut Run,
        itdebug: i32,
    ) -> Result<Mri, Fatal> {
        let nin = self.mri_mov.len();
        let _ = writeln!(r.out, "warping movs and creating conform template...");
        if self.iscale {
            let _ = writeln!(r.out, " allow intensity scaling");
        }
        if self.ltas.iter().any(Option::is_none) {
            self.no_transform(g, r)?;
        }
        let mut cwarps = Vec::with_capacity(nin);
        for i in 0..nin {
            let mut lta = self.ltas[i].take().expect("the input's transform");
            let got = lta_transform(g, r.out, r.err, r.env, &self.mri_mov[i], None, &mut lta);
            self.ltas[i] = Some(lta);
            let mut cw = got?.into_new().expect("LTAtransform's new volume");
            if let Some(w) = self.mri_warps[i].as_mut() {
                mri_copy_pulse_parameters(&self.mri_mov[i], w);
            }
            let _ = r.out.flush();
            let _ = writeln!(
                r.err,
                " MultiRegistration::averageConformSet not implemented, yet !!!"
            );
            g.notices.add(
                &FS_1091,
                "--conform writes the template resampled trilinearly in its own space, not conformed (FreeSurfer's 'not implemented, yet')",
            );
            if self.iscale {
                mri_valscale(
                    g,
                    r.out,
                    r.err,
                    Pair::Same(&mut cw),
                    self.intensities[i],
                    0.0,
                )?;
            }
            if self.debug != 0 {
                let name = format!(
                    "{}tp{}_to_template_conform-it{itdebug}.mgz",
                    self.outdir,
                    i + 1
                );
                Self::write(g, r, &mut cw, &name)?;
            }
            cwarps.push(cw);
        }
        let set: Vec<&Mri> = cwarps.iter().collect();
        average_set(g, r, &set, None, self.average, self.sat)
    }

    /// FS-1081 (nondeterminism): FreeSurfer runs the `k` registrations of a loop on OpenMP
    /// threads (`schedule(static, 1)`), the lines of each in the order the threads reach them and
    /// the centroids summed in the order they finish: with more than one thread and more than one
    /// registration its output and template depend on the run. The port's are FreeSurfer's at one
    /// thread, in both modes; noted when the run's thread count is not 1.
    fn threads_notice(&self, g: &mut Globals, r: &Run, k: usize) {
        if k > 1 && r.threads != 1 {
            g.notices.add(
                &FS_1081,
                format!(
                    "{k} registrations: FreeSurfer's lines and sums would depend on its OpenMP threads (the port gives its one-thread order)"
                ),
            );
        }
    }

    /// `normalizeIntensities()`: the factors divided by their geometric mean (`pow(product, 1 /
    /// n)`, the product in order from 1; each factor times `1 / mean`).
    fn normalize_intensities(&mut self, g: &mut Globals) {
        let nin = self.intensities.len();
        let mut mint = 1.0f64;
        for &x in &self.intensities {
            mint = mul64(mint, x);
        }
        let libm = Libm::new(g.mode());
        mint = libm.pow(mint, div64(1.0, nin as f64));
        note_glibc07(g, &libm, "pow");
        let f = div64(1.0, mint);
        for x in &mut self.intensities {
            *x = mul64(*x, f);
        }
    }

    /// `initialXforms(tpi, fixtp, maxres, iterate, epsit)`: see the module's notes; `tpi` from 1.
    /// FS-1092: a `tpi` past the inputs (FreeSurfer's index is then out of its vector: memory
    /// contents): an error in both modes.
    pub fn initial_xforms(
        &mut self,
        g: &mut Globals,
        r: &mut Run,
        tpi: i32,
        fixtp: bool,
        maxres: i32,
        iterate: i32,
        epsit: f64,
    ) -> Result<bool, Fatal> {
        if tpi <= 0 {
            return Ok(false);
        }
        let _ = write!(
            r.out,
            "\nMultiRegistration::initializing Xforms (init {tpi} , maxres {maxres} , iterate {iterate} , epsit {} ) : \n",
            ostream(epsit)
        );
        if self.debug != 0 {
            self.print_params(r.out);
        }
        let nin = self.mri_mov.len();
        let tpi = (tpi - 1) as usize;
        if tpi >= nin {
            g.notices.add(
                &FS_1092,
                format!(
                    "--inittp {} of {nin} inputs (FreeSurfer's index leaves its vector: memory contents)",
                    tpi + 1
                ),
            );
            let _ = r.out.flush();
            let _ = writeln!(
                r.err,
                "ERROR: --inittp {} is not one of the {nin} inputs (FreeSurfer reads past its images here)",
                tpi + 1
            );
            return Err(Fatal::Exit(1));
        }
        self.threads_notice(g, r, nin - 1);
        let mut index: Vec<usize> = (0..nin).collect();
        index[0] = tpi;
        index[tpi] = 0;
        let mut converged = vec![true; nin];
        let mut md: Vec<(Mat4, f64)> = vec![(identity4(), 1.0); nin];
        let mut centroid = [0.0f64; 4];
        for i in 1..nin {
            let j = index[i];
            let _ = write!(
                r.out,
                "\n[init] ========================= TP {} to TP {} ==============================\n",
                j + 1,
                tpi + 1
            );
            let _ = writeln!(r.out, "         Register TP {} ( {} )", j + 1, self.mov[j]);
            let _ = write!(
                r.out,
                "          to      TP {} ( {} )\n\n",
                tpi + 1,
                self.mov[tpi]
            );
            let name = format!("{}tp{}_to_tp{tpi}", self.outdir, j + 1);
            let mut reg = RegRobust::new();
            self.init_registration(&mut reg);
            reg.set_verbose(if self.debug != 0 { 1 } else { 0 });
            {
                let mut io = io(r);
                reg.set_source_and_target(
                    g,
                    &mut io,
                    &self.mri_mov[j],
                    &self.mri_mov[tpi],
                    self.keeptype,
                )?;
                reg.set_name(&name);
                if self.satit {
                    reg.find_saturation(g, &mut io)?;
                }
                if self.nomulti {
                    reg.compute_iterative_registration(g, &mut io, iterate, epsit)?;
                } else {
                    reg.compute_multires_registration(g, &mut io, maxres, iterate, epsit)?;
                }
            }
            md[i].0 = reg.get_final_vox2vox(r.out);
            md[i].1 = reg.get_final_iscale();
            converged[i] = reg.get_converged();
            let mut temp = reg.get_centroid_s_in_t(r.out);
            if i == 1 {
                let ct = reg.get_centroid_t();
                for k in 0..4 {
                    temp[k] = add64(temp[k], ct[k]);
                }
                if self.debug != 0 {
                    let _ = writeln!(r.out, "{}", matlab_print_long_vector(&ct, "CentroidT"));
                }
            }
            if self.debug != 0 {
                let cst = reg.get_centroid_s_in_t(r.out);
                let _ = writeln!(r.out, "{}", matlab_print_long_vector(&cst, "CentroidSinT"));
            }
            for k in 0..4 {
                centroid[k] = add64(centroid[k], temp[k]);
            }
        }
        let f = div64(1.0, nin as f64);
        for c in &mut centroid {
            *c = mul64(f, *c);
        }
        if self.debug != 0 {
            let _ = writeln!(r.out, "{}", matlab_print_long_vector(&centroid, "Centroid"));
        }
        for i in 1..nin {
            if !converged[i] {
                let _ = writeln!(
                    r.out,
                    "* WARNING: TP {} to {} did not converge !! ",
                    index[i] + 1,
                    tpi + 1
                );
            }
        }
        for i in 0..nin {
            let j = index[i];
            let lta = vox_matrix2lta(
                g,
                r.out,
                r.err,
                &md[i].0,
                &self.mri_mov[j],
                &self.mri_mov[tpi],
            )?;
            self.ltas[j] = Some(lta);
            self.intensities[j] = md[i].1;
            if self.debug != 0 && fixtp {
                self.debug_mcoord(g, r, j, md[i].1)?;
            }
        }
        if fixtp {
            return Ok(true);
        }
        if self.iscale {
            self.normalize_intensities(g);
        }
        let mut mras: Vec<Mat4> = vec![identity4(); nin];
        let mut mras3: Vec<Mat3> = vec![[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]; nin];
        let mut meant = [0.0f64; 3];
        for i in 1..nin {
            let j = index[i];
            mras[i] =
                voxel_xform_to_ras_xform(g, r, &self.mri_mov[j], &self.mri_mov[tpi], &md[i].0)?;
            let mrasinv = inv4(&mras[i]);
            let (lin, trans) = get_rt_from_m(&mrasinv);
            for k in 0..3 {
                meant[k] = add64(meant[k], trans[k]);
            }
            mras3[i] = lin;
        }
        for t in &mut meant {
            *t = mul64(f, *t);
        }
        if self.debug != 0 {
            let _ = writeln!(r.out, "{}", matlab_print_long_vector(&meant, "meant"));
        }
        let meanr = if self.rigid {
            rotation_mean(g, r.out, r.err, &mras3, self.frobnormthresh)?
        } else {
            let _ = r.out.flush();
            let _ = writeln!(
                r.err,
                "MyMatrix::GeometricMean (the affine mean space: --affine without --fixtp) {NOT_PORTED}"
            );
            return Err(Fatal::Exit(1));
        };
        if self.debug != 0 {
            let _ = writeln!(r.out, "{}", matlab_print_long(&dyn3(&meanr), "meanr"));
            let flat: Vec<f64> = meanr.iter().flatten().copied().collect();
            let _ = write!(
                r.out,
                " Determinant( meanr ) : {}\n\n",
                ostream(vnl_fixed::determinant(&flat, 3))
            );
            let _ = write!(r.out, " Decompose into Rot * Shear * Scale : \n\n");
            let (rot, shear, scale) = polar2_decomposition(g, r.err, &dyn3(&meanr))?;
            let _ = writeln!(r.out, "{}", matlab_print_long(&rot, "Rot"));
            let _ = writeln!(r.out, "{}", matlab_print_long(&shear, "Shear"));
            let _ = writeln!(r.out, "{}", matlab_print_long_diag(&scale, "Scale"));
        }
        let mm = get_m_from_rt(&meanr, &meant);
        let mut template_geo = self.create_template_geo(g, r)?;
        if !self.crascenter {
            let tpi_v2r =
                convert_matrix2vnl(&extract_i_to_r(g, r.out, r.err, &self.mri_mov[tpi].vg)?)
                    .to_4x4()
                    .expect("4 x 4");
            let ncenter = mat_vec4(&tpi_v2r, &centroid);
            let ncenter = mat_vec4(&mm, &ncenter);
            template_geo.vg.c_r = ncenter[0] as f32;
            template_geo.vg.c_a = ncenter[1] as f32;
            template_geo.vg.c_s = ncenter[2] as f32;
            template_geo.vg.ras_good_flag = 1;
            reinit_cache(g, r.out, r.err, &mut template_geo.vg)?;
        }
        for i in 0..nin {
            let j = index[i];
            let _ = writeln!(
                r.out,
                "  computing mean coord of TP {} ( {} ) ",
                j + 1,
                self.mov[j]
            );
            let mut m = mul4(&mm, &mras[i]);
            if self.rigid {
                let a = dyn3(&get_rt_from_m(&m).0);
                let (rr, s) = polar_decomposition(g, r.err, &a)?;
                if s.at(0, 0) < 0.0 || s.at(1, 1) < 0.0 || s.at(2, 2) < 0.0 {
                    log::error_exit(
                        g,
                        r.err,
                        ERROR_OUT_OF_BOUNDS,
                        "Internal Error:  produced reflection.\n",
                    )?;
                }
                let eps = 0.0001;
                let fnorm1 = s.sub(&VnlMatrix::identity(3)).fro_norm();
                if fnorm1 > eps {
                    let _ = r.out.flush();
                    let mut e = String::new();
                    e.push_str(&format!("Internal Error for tp {j} -> template\n"));
                    e.push_str(&format!(
                        " Rotation should not scale ( {} )\n",
                        ostream(fnorm1)
                    ));
                    e.push_str(" Debug Info: \n");
                    e.push_str(&matlab_print_long(&a, "A"));
                    e.push('\n');
                    e.push_str(&matlab_print_long(&rr, "R"));
                    e.push('\n');
                    e.push_str(&matlab_print_long(&s, "S"));
                    e.push('\n');
                    e.push_str(" Make sure input voxel sizes are identical for all images!\n");
                    let _ = r.err.write_all(e.as_bytes());
                    log::error_exit(
                        g,
                        r.err,
                        ERROR_OUT_OF_BOUNDS,
                        "Internal Error: Rotation should not scale.\n",
                    )?;
                }
                let _ = writeln!(r.out, "   mapping back to rot, err = {}", ostream(fnorm1));
                for (a, row) in m.iter_mut().enumerate().take(3) {
                    for (b, v) in row.iter_mut().enumerate().take(3) {
                        *v = rr.at(a, b);
                    }
                }
                m[3] = [0.0, 0.0, 0.0, 1.0];
            }
            let mut lta = ras_matrix2lta(g, r.out, r.err, &m, &self.mri_mov[j], &template_geo)?;
            lta_change_type(g, r.out, r.err, r.env, &mut lta, LINEAR_VOX_TO_VOX)?;
            self.ltas[j] = Some(lta);
            if self.debug != 0 {
                self.debug_mcoord(g, r, j, md[i].1)?;
            }
        }
        Ok(true)
    }

    /// `initialXforms`' debug output of input `j`: its transform (`<outdir>tp<j>_to_mcoord.lta`,
    /// `LTAwriteEx`), the input mapped by it, scaled by `iscale` (with the factor written as
    /// `-intensity.txt`), written as `.mgz`.
    fn debug_mcoord(
        &mut self,
        g: &mut Globals,
        r: &mut Run,
        j: usize,
        iscale: f64,
    ) -> Result<(), Fatal> {
        let base = format!("{}tp{}_to_mcoord", self.outdir, j + 1);
        {
            let lta = self.ltas[j].as_ref().expect("the transform");
            lta_write_ex(g, r.out, r.err, r.env, r.dir, lta, &format!("{base}.lta"))?;
        }
        let mut warped = self.warp(g, r, j, None)?;
        if self.iscale {
            let path = resolve(r.dir, &format!("{base}-intensity.txt"));
            let _ = std::fs::write(path, ostream(iscale));
            mri_valscale(g, r.out, r.err, Pair::Same(&mut warped), iscale, 0.0)?;
        }
        Self::write(g, r, &mut warped, &format!("{base}.mgz"))?;
        Ok(())
    }

    /// `getAverageCosines()`: the inputs' direction cosines; identical (within 1e-9) for all: the
    /// first's. Otherwise (` Cosines of input transforms not identical !` for each element that
    /// differs) each input's axes reordered and flipped towards the first's (`cosM[0]^T cosM[i]`'s
    /// largest element of each column: a reordering that is no permutation prints `WARNING:
    /// reorder not clear ...`, the matrix, the axes, and ends the program, status 1, after
    /// `ERROR: vox2vox det: <d> < 0` and two lines when its determinant is negative), their mean
    /// made a rotation (`MyMatrix::PolarDecomposition`).
    fn get_average_cosines(&self, g: &mut Globals, r: &mut Run) -> Result<Mat3, Fatal> {
        let nin = self.mri_mov.len();
        let mut cos_m: Vec<Mat3> = Vec::with_capacity(nin);
        let mut same = true;
        let eps = 0.000000001;
        for (i, m) in self.mri_mov.iter().enumerate() {
            let v = &m.vg;
            let c = [
                [f64::from(v.x_r), f64::from(v.y_r), f64::from(v.z_r)],
                [f64::from(v.x_a), f64::from(v.y_a), f64::from(v.z_a)],
                [f64::from(v.x_s), f64::from(v.y_s), f64::from(v.z_s)],
            ];
            cos_m.push(c);
            if i > 0 {
                for jj in 0..3 {
                    for k in 0..3 {
                        if sub64(cos_m[0][jj][k], cos_m[i][jj][k]).abs() > eps {
                            same = false;
                            let _ = writeln!(r.out, " Cosines of input transforms not identical !");
                        }
                    }
                }
            }
        }
        if same {
            return Ok(cos_m[0]);
        }
        let mut meanr = cos_m[0];
        for i in 1..nin {
            let t0 = larmorx_freesurfer_robust::mymatrix::transpose3(&cos_m[0]);
            let v2v = mul3(&t0, &cos_m[i]);
            let mut xd: i32 = 1;
            let mut yd: i32 = 1;
            let mut zd: i32 = 1;
            for (col, dd) in [&mut xd, &mut yd, &mut zd].into_iter().enumerate() {
                if v2v[1][col].abs() > v2v[0][col].abs() {
                    *dd = 2;
                }
                if v2v[2][col].abs() > v2v[0][col].abs() && v2v[2][col].abs() > v2v[1][col].abs() {
                    *dd = 3;
                }
            }
            if v2v[(xd - 1) as usize][0] < 0.0 {
                xd = -xd;
            }
            if v2v[(yd - 1) as usize][1] < 0.0 {
                yd = -yd;
            }
            if v2v[(zd - 1) as usize][2] < 0.0 {
                zd = -zd;
            }
            if xd != 1 || yd != 2 || zd != 3 {
                if xd.abs() + yd.abs() + zd.abs() != 6 {
                    let _ = writeln!(r.out, "WARNING: reorder not clear ...");
                    let _ = writeln!(r.out, "{}", matlab_print_long(&dyn3(&v2v), "v2v"));
                    let _ = writeln!(r.out, " xd: {xd} yd: {yd} zd: {zd}");
                    let flat: Vec<f64> = v2v.iter().flatten().copied().collect();
                    let det = vnl_fixed::determinant(&flat, 3);
                    if det < 0.0 {
                        let _ = writeln!(r.out, "ERROR: vox2vox det: {} < 0", ostream(det));
                        let _ = writeln!(
                            r.out,
                            "       Something might be wrong with RAS info in inputs."
                        );
                        let _ = writeln!(
                            r.out,
                            "       Make sure volumes are in same voxel orientation."
                        );
                    }
                    return Err(Fatal::Exit(1));
                }
                let mut q = [[0.0f64; 3]; 3];
                q[0][(xd.abs() - 1) as usize] = if xd >= 0 { 1.0 } else { -1.0 };
                q[1][(yd.abs() - 1) as usize] = if yd >= 0 { 1.0 } else { -1.0 };
                q[2][(zd.abs() - 1) as usize] = if zd >= 0 { 1.0 } else { -1.0 };
                cos_m[i] = mul3(&cos_m[i], &q);
            }
            for a in 0..3 {
                for b in 0..3 {
                    meanr[a][b] = add64(meanr[a][b], cos_m[i][a][b]);
                }
            }
        }
        let f = div64(1.0, nin as f64);
        for row in &mut meanr {
            for v in row.iter_mut() {
                *v = mul64(f, *v);
            }
        }
        let (pol_r, _) = polar_decomposition(g, r.err, &dyn3(&meanr))?;
        let mut out = [[0.0; 3]; 3];
        for a in 0..3 {
            for b in 0..3 {
                out[a][b] = pol_r.at(a, b);
            }
        }
        Ok(out)
    }

    /// `createTemplateGeo()`: isotropic voxels of the largest of the inputs' smallest sides, each
    /// side the largest `ceil(extent / size - 0.0001)` of the inputs; 2-D and 3-D inputs together:
    /// ` ERROR (createTemplateGeo) : mixing 2d and 3d input images not possible!`, status 1; the
    /// header (`MRIallocHeader` of `float`, then the type `uchar`, `imnr0` 1, `imnr1` the height,
    /// `thick` and `ps` the size, the start and end at half the extents), the average direction
    /// cosines, the centre the mean of the inputs' centres.
    fn create_template_geo(&self, g: &mut Globals, r: &mut Run) -> Result<Mri, Fatal> {
        let nin = self.mri_mov.len();
        let mut conform_size = 0.0f64;
        for m in &self.mri_mov {
            let (x, y, z) = (
                f64::from(m.vg.xsize.abs()),
                f64::from(m.vg.ysize.abs()),
                f64::from(m.vg.zsize.abs()),
            );
            let mut minvox = x;
            if y < minvox {
                minvox = y;
            }
            if z < minvox {
                minvox = z;
            }
            if minvox > conform_size {
                conform_size = minvox;
            }
        }
        let (mut maxw, mut maxh, mut maxd, mut count2d) = (0i32, 0i32, 0usize, 0usize);
        for m in &self.mri_mov {
            let fw = mul64(f64::from(m.vg.xsize.abs()), f64::from(m.vg.width));
            let fh = mul64(f64::from(m.vg.ysize.abs()), f64::from(m.vg.height));
            let fd = mul64(f64::from(m.vg.zsize.abs()), f64::from(m.vg.depth));
            let eps = 0.0001;
            let nw = ceil_int(sub64(div64(fw, conform_size), eps));
            let nh = ceil_int(sub64(div64(fh, conform_size), eps));
            let nd = ceil_int(sub64(div64(fd, conform_size), eps));
            if m.vg.depth == 1 {
                count2d += 1;
            }
            maxw = maxw.max(nw);
            maxh = maxh.max(nh);
            maxd = maxd.max(nd.max(0) as usize);
        }
        if count2d > 0 && count2d != nin {
            let _ = r.out.flush();
            let _ = writeln!(
                r.err,
                " ERROR (createTemplateGeo) : mixing 2d and 3d input images not possible!"
            );
            return Err(Fatal::Exit(1));
        }
        let maxd = maxd as i32;
        let mut t = mri_alloc_header(g, r.out, r.err, maxw, maxh, maxd, MRI_FLOAT, 1)?;
        let cs = conform_size as f32;
        t.vg.xsize = cs;
        t.vg.ysize = cs;
        t.vg.zsize = cs;
        t.imnr0 = 1;
        t.imnr1 = maxh;
        t.mtype = MRI_UCHAR;
        t.thick = cs;
        t.ps = cs;
        t.xstart = (-maxw / 2) as f32;
        t.ystart = (-maxw / 2) as f32;
        t.zstart = (-maxd / 2) as f32;
        t.xend = (maxw / 2) as f32;
        t.yend = (maxw / 2) as f32;
        t.zend = (maxd / 2) as f32;
        let avgcos = self.get_average_cosines(g, r)?;
        t.vg.x_r = avgcos[0][0] as f32;
        t.vg.x_a = avgcos[1][0] as f32;
        t.vg.x_s = avgcos[2][0] as f32;
        t.vg.y_r = avgcos[0][1] as f32;
        t.vg.y_a = avgcos[1][1] as f32;
        t.vg.y_s = avgcos[2][1] as f32;
        t.vg.z_r = avgcos[0][2] as f32;
        t.vg.z_a = avgcos[1][2] as f32;
        t.vg.z_s = avgcos[2][2] as f32;
        let (mut nr, mut na, mut ns) = (0.0f64, 0.0f64, 0.0f64);
        for m in &self.mri_mov {
            nr = add64(nr, f64::from(m.vg.c_r));
            na = add64(na, f64::from(m.vg.c_a));
            ns = add64(ns, f64::from(m.vg.c_s));
        }
        let n = nin as f64;
        t.vg.c_r = div64(nr, n) as f32;
        t.vg.c_a = div64(na, n) as f32;
        t.vg.c_s = div64(ns, n) as f32;
        t.vg.ras_good_flag = 1;
        reinit_cache(g, r.out, r.err, &mut t.vg)?;
        Ok(t)
    }

    /// `initialAverageSet()`: ` MultiRegistration::initialAverageSet `; with `iscaleonly` (`    --
    /// averaging without alignment (iscale only)`) the identity onto the first input; otherwise
    /// (`    -- aligning image centroids (translation)`) each input's centroid
    /// (`CostFunctions::centroid`) in RAS, their mean, a template of the first input's geometry
    /// centred there (when there is none yet) and each input's translation onto it; then
    /// [`MultiRegistration::map_and_average_mov`].
    pub fn initial_average_set(&mut self, g: &mut Globals, r: &mut Run) -> Result<(), Fatal> {
        let _ = writeln!(r.out, " MultiRegistration::initialAverageSet ");
        let n = self.mri_mov.len();
        if self.ltas.len() != n {
            self.ltas = (0..n).map(|_| None).collect();
        }
        if self.iscaleonly {
            let _ = writeln!(r.out, "    -- averaging without alignment (iscale only)");
            if self.mri_mean.is_none() {
                self.mri_mean = Some(
                    mri_clone(g, r.out, r.err, &self.mri_mov[0], None)?
                        .into_new()
                        .expect("MRIclone's new volume"),
                );
            }
            let mean = self.mri_mean.as_ref().expect("the mean");
            for i in 0..n {
                let lta = ras_matrix2lta(g, r.out, r.err, &identity4(), &self.mri_mov[i], mean)?;
                self.ltas[i] = Some(lta);
            }
        } else {
            let _ = writeln!(r.out, "    -- aligning image centroids (translation)");
            let mut c = [0.0f64; 3];
            let mut cs: Vec<[f64; 3]> = Vec::with_capacity(n);
            for i in 0..n {
                let ci = centroid(g, r.err, &self.mri_mov[i])?;
                let v2r =
                    convert_matrix2vnl(&extract_i_to_r(g, r.out, r.err, &self.mri_mov[i].vg)?)
                        .to_4x4()
                        .expect("4 x 4");
                let nc = mat_vec4(&v2r, &[ci[0], ci[1], ci[2], 1.0]);
                let ci = [nc[0], nc[1], nc[2]];
                for k in 0..3 {
                    c[k] = add64(c[k], ci[k]);
                }
                cs.push(ci);
            }
            let nn = n as f64;
            for x in &mut c {
                *x = div64(*x, nn);
            }
            if self.mri_mean.is_none() {
                let mut mean = mri_clone(g, r.out, r.err, &self.mri_mov[0], None)?
                    .into_new()
                    .expect("MRIclone's new volume");
                let v2r =
                    convert_matrix2vnl(&extract_i_to_r(g, r.out, r.err, &self.mri_mov[0].vg)?)
                        .to_4x4()
                        .expect("4 x 4");
                let nc = mat_vec4(&v2r, &[c[0], c[1], c[2], 1.0]);
                c = [nc[0], nc[1], nc[2]];
                mean.vg.c_r = nc[0] as f32;
                mean.vg.c_a = nc[1] as f32;
                mean.vg.c_s = nc[2] as f32;
                mean.vg.ras_good_flag = 1;
                reinit_cache(g, r.out, r.err, &mut mean.vg)?;
                self.mri_mean = Some(mean);
            }
            let mean = self.mri_mean.as_ref().expect("the mean");
            for i in 0..n {
                let mut mt = identity4();
                // a MATRIX of float: each translation stored as float
                mt[0][3] = f64::from(sub64(c[0], cs[i][0]) as f32);
                mt[1][3] = f64::from(sub64(c[1], cs[i][1]) as f32);
                mt[2][3] = f64::from(sub64(c[2], cs[i][2]) as f32);
                let lta = ras_matrix2lta(g, r.out, r.err, &mt, &self.mri_mov[i], mean)?;
                self.ltas[i] = Some(lta);
            }
        }
        self.map_and_average_mov(g, r, 0)
    }

    /// `computeTemplate(itmax, eps, iterate, epsit)`: see the module's notes.
    pub fn compute_template(
        &mut self,
        g: &mut Globals,
        r: &mut Run,
        itmax: i32,
        eps: f64,
        iterate: i32,
        epsit: f64,
    ) -> Result<bool, Fatal> {
        let nin = self.mri_mov.len();
        if self.debug != 0 {
            let _ = write!(
                r.out,
                "\n\nMultiRegistration::computeTemplate(avitmax: {itmax}, aveps {}, regit: {iterate}, regeps: {} )\n",
                ostream(eps),
                ostream(epsit)
            );
        }
        let _ = write!(r.out, "\nComputing first template\n");
        let havexforms = self.ltas.first().is_some_and(Option::is_some);
        if !havexforms {
            self.initial_average_set(g, r)?;
        } else {
            self.map_and_average_mov(g, r, 0)?;
        }
        if itmax == 0 {
            return Ok(true);
        }
        {
            let name = format!("{}template-it0.mgz", self.outdir);
            let mean = self.mri_mean.as_mut().expect("the mean");
            strncpy_name(&mut mean.vg.fname, &name);
            if self.debug != 0 {
                let _ = writeln!(r.out, "debug: saving template-it0.mgz");
                Self::write(g, r, mean, &name)?;
            }
        }
        let mut itcount = 0i32;
        let mut maxchange = 100.0f64;
        let mut transforms: Vec<Mat4> = vec![identity4(); nin];
        for i in 0..nin {
            let mut lta = self.ltas[i].take().expect("the transform");
            lta_change_type(g, r.out, r.err, r.env, &mut lta, LINEAR_RAS_TO_RAS)?;
            lta_modify_src_dst_geom(
                g,
                r.out,
                r.err,
                &mut lta,
                Some(&self.mri_mov[i]),
                self.mri_mean.as_ref(),
            );
            lta_change_type(g, r.out, r.err, r.env, &mut lta, LINEAR_VOX_TO_VOX)?;
            transforms[i] = convert_matrix2vnl(m_l(&lta)).to_4x4().expect("4 x 4");
            self.ltas[i] = Some(lta);
        }
        let mut noxformits = [3, 1, 0, 0];
        while itcount < itmax && maxchange > eps {
            itcount += 1;
            if self.ltas[0].is_some() {
                maxchange = 0.0;
            }
            let bar = "=======================================================";
            let _ = write!(r.out, "\n{bar}\n");
            let _ = write!(r.out, "\nWorking on global iteration : {itcount}\n");
            let _ = write!(r.out, "\n{bar}\n");
            if itcount <= 4 && noxformits[(itcount - 1) as usize] > 0 {
                let _ = writeln!(
                    r.out,
                    "  noxformits = {}",
                    noxformits[(itcount - 1) as usize]
                );
            }
            self.threads_notice(g, r, nin);
            let mut dists = vec![1000.0f64; nin];
            for i in 0..nin {
                let _ = write!(r.out, "\nWorking on TP {}\n\n", i + 1);
                let mut reg = RegRobust::new();
                reg.set_verbose(0);
                self.init_registration(&mut reg);
                let name = format!("{}tp{}_to_template-it{itcount}", self.outdir, i + 1);
                let mut maxres = 0;
                let mut subsamp = self.subsamplesize;
                if !self.iscaleonly
                    && !havexforms
                    && itcount <= 4
                    && noxformits[(itcount - 1) as usize] > 0
                {
                    match itcount {
                        1 => maxres = 3,
                        2 => maxres = 2,
                        3 => maxres = 1,
                        4 => subsamp = 180,
                        _ => {}
                    }
                }
                {
                    let mean = self.mri_mean.as_ref().expect("the mean");
                    let mut io = io(r);
                    reg.set_source_and_target(g, &mut io, &self.mri_mov[i], mean, self.keeptype)?;
                    reg.set_name(&name);
                    reg.subsamplesize = subsamp;
                    reg.iscaleinit = self.intensities[i];
                    reg.minit = Some(VnlMatrix::from_4x4(&transforms[i]));
                    if self.satit {
                        reg.find_saturation(g, &mut io)?;
                    }
                    if self.nomulti || self.iscaleonly {
                        let _ = writeln!(
                            io.out,
                            " - running high-res registration on TP {}...",
                            i + 1
                        );
                        reg.compute_iterative_registration(g, &mut io, iterate, epsit)?;
                    } else {
                        let _ = writeln!(
                            io.out,
                            " - running multi-resolutional registration on TP {}...",
                            i + 1
                        );
                        reg.compute_multires_registration(g, &mut io, maxres, iterate, epsit)?;
                    }
                }
                let md0 = reg.get_final_vox2vox(r.out);
                let md1 = reg.get_final_iscale();
                if !reg.get_converged() {
                    let _ = writeln!(
                        r.out,
                        "   *** WARNING: TP {} to template did not converge ***",
                        i + 1
                    );
                }
                transforms[i] = md0;
                self.intensities[i] = md1;
                let lastlta = self.ltas[i].take();
                let mut lta = {
                    let mean = self.mri_mean.as_ref().expect("the mean");
                    vox_matrix2lta(g, r.out, r.err, &md0, &self.mri_mov[i], mean)?
                };
                if let Some(mut last) = lastlta {
                    lta_change_type(g, r.out, r.err, r.env, &mut last, LINEAR_RAS_TO_RAS)?;
                    lta_change_type(g, r.out, r.err, r.env, &mut lta, LINEAR_RAS_TO_RAS)?;
                    let d2 = affine_trans_dist_sq_matrix(g, r, m_l(&last), m_l(&lta), 100.0)?;
                    dists[i] = larmorx_freesurfer_core::sse::sqrt64(d2);
                    if dists[i] > maxchange {
                        maxchange = dists[i];
                    }
                    let _ = writeln!(r.out, "   tp {} distance: {}", i + 1, ostream(dists[i]));
                }
                self.ltas[i] = Some(lta);
                self.mri_warps[i] = None;
                let clone = {
                    let mean = self.mri_mean.as_ref().expect("the mean");
                    mri_clone(g, r.out, r.err, mean, None)?
                        .into_new()
                        .expect("MRIclone's new volume")
                };
                if self.sampletype == SAMPLE_CUBIC_BSPLINE {
                    let _ = writeln!(
                        r.out,
                        " - mapping tp {} to template (cubic bspline) ...",
                        i + 1
                    );
                } else {
                    let _ = writeln!(r.out, " - mapping tp {} to template...", i + 1);
                }
                let mut w = self.warp(g, r, i, Some(clone))?;
                mri_copy_pulse_parameters(&self.mri_mov[i], &mut w);
                if reg.iscale && md1 > 0.0 {
                    let _ = writeln!(
                        r.out,
                        " - adjusting intensity of mapped tp {} by {}",
                        i + 1,
                        ostream(md1)
                    );
                    mri_valscale(g, r.out, r.err, Pair::Same(&mut w), md1, 0.0)?;
                }
                self.mri_warps[i] = Some(w);
                if self.backupweights {
                    let _ = writeln!(r.out, " - backup weights tp {} ...", i + 1);
                    if self.mri_weights.len() != nin {
                        // FS-1089 (the weights' list without the robust registration: FreeSurfer
                        // writes past it)
                        g.notices.add(
                            &FS_1094,
                            "--weights with --leastsquares: FreeSurfer keeps the weights past the end of a list it never sized (heap memory)",
                        );
                    } else if let Some(wsrc) = reg.mri_weights.as_ref() {
                        let prev = self.mri_weights[i].take();
                        let copied = match prev {
                            Some(mut p) => {
                                mri_copy(g, r.out, r.err, wsrc, Some(&mut p))?;
                                p
                            }
                            None => mri_copy(g, r.out, r.err, wsrc, None)?
                                .into_new()
                                .expect("MRIcopy's new volume"),
                        };
                        self.mri_weights[i] = Some(copied);
                    }
                }
                if self.debug != 0 {
                    self.debug_template_tp(g, r, &reg, i, &name, md1)?;
                }
                let _ = write!(r.out, "\nFinished TP : {}\n\n", i + 1);
                let _ = writeln!(
                    r.out,
                    "====================================================="
                );
            }
            if !havexforms && itcount <= 4 && noxformits[(itcount - 1) as usize] > 1 {
                noxformits[(itcount - 1) as usize] -= 1;
                itcount -= 1;
            }
            if dists[0] <= maxchange {
                let _ = write!(r.out, "\nGlobal Iteration {itcount} Distances: \n");
                for d in &dists {
                    let _ = write!(r.out, "{} ", ostream(*d));
                }
                let _ = write!(r.out, "\nMaxchange: {}\n\n", ostream(maxchange));
            }
            let _ = writeln!(r.out, "Computing new template {itcount}");
            {
                let warps: Vec<&Mri> = self
                    .mri_warps
                    .iter()
                    .map(|w| w.as_ref().expect("the warp"))
                    .collect();
                let mean = self.mri_mean.take();
                let mean = average_set(g, r, &warps, mean, self.average, self.sat)?;
                self.mri_mean = Some(mean);
            }
            if self.debug != 0 {
                let name = format!("{}template-it{itcount}.mgz", self.outdir);
                let _ = writeln!(r.out, "debug: writing template to {name}");
                let mean = self.mri_mean.as_mut().expect("the mean");
                Self::write(g, r, mean, &name)?;
            }
        }
        let _ = writeln!(r.out, " DONE : computeTemplate ");
        Ok(true)
    }

    /// `computeTemplate`'s debug output of input `i`: its transform (`<name>.lta`), its warp
    /// (`<name>.mgz`), its intensity factor (`<name>-intensity.txt`), and, with weights, the two
    /// half-way maps and the inverse of the second (`os << M`), the weights mapped into the
    /// template's space (`<name>-weights.mgz`) and as they are (`<name>-www.mgz`).
    fn debug_template_tp(
        &mut self,
        g: &mut Globals,
        r: &mut Run,
        reg: &RegRobust,
        i: usize,
        name: &str,
        md1: f64,
    ) -> Result<(), Fatal> {
        let _ = writeln!(
            r.out,
            " - debug tp {} : writing transforms, warps, weights ...",
            i + 1
        );
        {
            let lta = self.ltas[i].as_ref().expect("the transform");
            lta_write_ex(g, r.out, r.err, r.env, r.dir, lta, &format!("{name}.lta"))?;
        }
        {
            let w = self.mri_warps[i].as_mut().expect("the warp");
            Self::write(g, r, w, &format!("{name}.mgz"))?;
        }
        if reg.iscale && md1 > 0.0 {
            let path = resolve(r.dir, &format!("{name}-intensity.txt"));
            let _ = std::fs::write(path, ostream(md1));
        }
        let has_weights = self.mri_weights.get(i).is_some_and(Option::is_some);
        if has_weights {
            let (first, second) = reg.get_half_way_maps(g, r.err)?;
            let hinv = inv4(&second);
            let cout = larmorx_freesurfer_robust::debug::Cout::default();
            let _ = writeln!(r.out);
            print_fixed4(r.out, &first, cout);
            let _ = writeln!(r.out);
            print_fixed4(r.out, &second, cout);
            let _ = writeln!(r.out);
            print_fixed4(r.out, &hinv, cout);
            let _ = writeln!(r.out);
            let wi = self.mri_weights[i].as_ref().expect("the weights");
            let mean = self.mri_mean.as_ref().expect("the mean");
            let mut wtarg = mri_alloc_sequence(
                g,
                r.out,
                r.err,
                wi.vg.width,
                wi.vg.height,
                wi.vg.depth,
                MRI_FLOAT,
                wi.nframes,
            )?;
            mri_copy_header(g, r.out, r.err, wi, Some(&mut wtarg))?;
            let v2r = extract_i_to_r(g, r.out, r.err, &mean.vg)?;
            mri_set_voxel_to_ras_xform(g, r.out, r.err, r.env, &mut wtarg, &v2r)?;
            wtarg.mtype = MRI_FLOAT;
            wtarg.vg.i_to_r = mean.vg.i_to_r.clone();
            wtarg.vg.r_to_i = mean.vg.r_to_i.clone();
            let got = mri_linear_transform(g, r.out, r.err, wi, Some(&mut wtarg), &hinv)?;
            let mut wtarg = match got {
                Ret::New(m) => m,
                _ => wtarg,
            };
            Self::write(g, r, &mut wtarg, &format!("{name}-weights.mgz"))?;
            let wi = self.mri_weights[i].as_mut().expect("the weights");
            Self::write(g, r, wi, &format!("{name}-www.mgz"))?;
        }
        Ok(())
    }

    /// `writeMean(mean)`: ` ERROR: No average exists! Skipping output.` without one; else its name
    /// set (`strncpy`) and the image written.
    pub fn write_mean(&mut self, g: &mut Globals, r: &mut Run, mean: &str) -> Result<bool, Fatal> {
        let Some(m) = self.mri_mean.as_mut() else {
            let _ = writeln!(r.out, " ERROR: No average exists! Skipping output.");
            return Ok(false);
        };
        strncpy_name(&mut m.vg.fname, mean);
        let ok = Self::write(g, r, m, mean)?;
        self.write_failed |= !ok;
        Ok(ok)
    }

    /// `writeConformMean(mean)` (`--conform`): [`MultiRegistration::average_conform_set`] written.
    pub fn write_conform_mean(
        &mut self,
        g: &mut Globals,
        r: &mut Run,
        mean: &str,
    ) -> Result<bool, Fatal> {
        if self.mri_mean.is_none() {
            let _ = writeln!(r.out, " ERROR: No average exists! Skipping output.");
            return Ok(false);
        }
        let mut c = self.average_conform_set(g, r, 0)?;
        strncpy_name(&mut c.vg.fname, mean);
        let ok = Self::write(g, r, &mut c, mean)?;
        self.write_failed |= !ok;
        Ok(ok)
    }

    /// `writeLTAs(nltas, vox2vox, mean)`: each transform made voxel to voxel or RAS to RAS, its
    /// names the template's and the input's (`strncpy`), written (`LTAwriteEx`), then ` Determinant(
    /// lta[ <i> ]) : <d>` of its voxel matrix and, for an affine registration, its decomposition.
    pub fn write_ltas(
        &mut self,
        g: &mut Globals,
        r: &mut Run,
        nltas: &[String],
        vox2vox: bool,
        mean: &str,
    ) -> Result<bool, Fatal> {
        for (i, name) in nltas.iter().enumerate() {
            let Some(lta) = self.ltas.get_mut(i).and_then(Option::as_mut) else {
                let _ = writeln!(r.out, " ERROR: No ltas exist! Skipping output.");
                return Ok(false);
            };
            let t = if vox2vox {
                LINEAR_VOX_TO_VOX
            } else {
                LINEAR_RAS_TO_RAS
            };
            lta_change_type(g, r.out, r.err, r.env, lta, t)?;
            strncpy_name(&mut lta.xforms[0].dst.fname, mean);
            strncpy_name(&mut lta.xforms[0].src.fname, &self.mov[i]);
            let e = lta_write_ex(g, r.out, r.err, r.env, r.dir, lta, name)?;
            self.write_failed |= e != 0;
            let fmv2v = lta2matrix(g, r.out, r.err, lta, true)?;
            let flat: Vec<f64> = (0..4)
                .flat_map(|a| (0..4).map(move |b| (a, b)))
                .map(|(a, b)| fmv2v.at(a, b))
                .collect();
            let _ = write!(
                r.out,
                " Determinant( lta[ {i} ]) : {}\n\n",
                ostream(vnl_fixed::determinant(&flat, 4))
            );
            if !self.rigid {
                let _ = write!(r.out, " Decompose into Rot * Shear * Scale : \n\n");
                let mut a = VnlMatrix::zeros(3, 3);
                for x in 0..3 {
                    for y in 0..3 {
                        *a.at_mut(x, y) = fmv2v.at(x, y);
                    }
                }
                let (rot, shear, scale) = polar2_decomposition(g, r.err, &a)?;
                let _ = writeln!(r.out, "{}", matlab_print_long(&rot, "Rot"));
                let _ = writeln!(r.out, "{}", matlab_print_long(&shear, "Shear"));
                let _ = writeln!(r.out, "{}", matlab_print_long_diag(&scale, "Scale"));
            }
        }
        Ok(true)
    }

    /// `writeMapMovHdr(mapmovhdr)`: each input with its voxel-to-RAS matrix moved by its RAS
    /// transform (`MyMatrix::LTA2RASmatrix`, as `float`, times the input's), written; ` ERROR: No
    /// LTAs exist! Skipping output.` without transforms; a write that fails: `ERROR: Can't write
    /// <name>` and the rest skipped.
    pub fn write_map_mov_hdr(
        &mut self,
        g: &mut Globals,
        r: &mut Run,
        names: &[String],
    ) -> Result<bool, Fatal> {
        for (i, name) in names.iter().enumerate() {
            let Some(lta) = self.ltas.get(i).and_then(Option::as_ref) else {
                let _ = writeln!(r.out, " ERROR: No LTAs exist! Skipping output.");
                return Ok(false);
            };
            let fmr2r = lta2matrix(g, r.out, r.err, lta, false)?;
            let ras2ras =
                larmorx_freesurfer_robust::mymatrix::convert_vnl2matrix(g, r.out, r.err, &fmr2r)?;
            let vox2ras = extract_i_to_r(g, r.out, r.err, &self.mri_mov[i].vg)?;
            let vox2ras = matrix_multiply(g, r.out, r.err, Some(&ras2ras), Some(&vox2ras), None)?
                .into_new()
                .expect("a 4 x 4 product");
            let mut aligned = mri_copy(g, r.out, r.err, &self.mri_mov[i], None)?
                .into_new()
                .expect("MRIcopy's new volume");
            mri_set_voxel_to_ras_xform(g, r.out, r.err, r.env, &mut aligned, &vox2ras)?;
            let ok = Self::write(g, r, &mut aligned, name)?;
            if !ok {
                self.write_failed = true;
                let _ = writeln!(r.out, "ERROR: Can't write {name}");
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `writeWarps(nwarps)`: one name: ` Writing single output (4D volume) ...` and every warp as a
    /// frame of one volume (`MRIallocSequence` of the first's extent and type, `MRIcopyFrame`;
    /// FS-1090: its header is `MRIallocSequence`'s default, not the template's: by default the
    /// first warp's header, `reproduce` the default); otherwise each warp written. A missing warp:
    /// ` ERROR: Warps do not exist! Skipping output.` (` ERROR: Warp <i> does not exist! Skipping
    /// output.`).
    pub fn write_warps(
        &mut self,
        g: &mut Globals,
        r: &mut Run,
        nwarps: &[String],
    ) -> Result<bool, Fatal> {
        let mut error = 0;
        if nwarps.len() == 1 {
            let _ = writeln!(r.out, " Writing single output (4D volume) ...");
            let Some(w0) = self.mri_warps.first().and_then(Option::as_ref) else {
                let _ = writeln!(r.out, " ERROR: Warps do not exist! Skipping output.");
                return Ok(false);
            };
            let nf = self.mri_warps.len() as i32;
            let mut m4 = mri_alloc_sequence(
                g,
                r.out,
                r.err,
                w0.vg.width,
                w0.vg.height,
                w0.vg.depth,
                w0.mtype,
                nf,
            )?;
            g.notices.add(
                &FS_1090,
                "--mapmov with one name: FreeSurfer's 4-D output takes a default header, not the template's geometry",
            );
            if !g.notices.reproduce() {
                mri_copy_header(g, r.out, r.err, w0, Some(&mut m4))?;
            }
            for i in 0..self.mri_warps.len() {
                let Some(w) = self.mri_warps[i].as_ref() else {
                    let _ = writeln!(r.out, " ERROR: Warp {i} does not exist! Skipping output.");
                    return Ok(false);
                };
                mri_copy_frame(g, r.out, r.err, Pair::Given(w, &mut m4), 0, i as i32)?;
            }
            if !Self::write(g, r, &mut m4, &nwarps[0])? {
                error += 1;
            }
        } else {
            for (i, name) in nwarps.iter().enumerate() {
                let Some(w) = self.mri_warps.get_mut(i).and_then(Option::as_mut) else {
                    let _ = writeln!(r.out, " ERROR: Warp {i} does not exist! Skipping output.");
                    return Ok(false);
                };
                if !Self::write(g, r, w, name)? {
                    error += 1;
                }
            }
        }
        self.write_failed |= error != 0;
        Ok(error == 0)
    }

    /// `writeIntensities(nintens)`: each factor written as C++'s `<<` prints it (6 digits); a file
    /// that cannot be created is counted, nothing printed.
    pub fn write_intensities(&mut self, g: &mut Globals, r: &mut Run, names: &[String]) -> bool {
        let mut error = 0;
        for (i, name) in names.iter().enumerate() {
            let Some(&x) = self.intensities.get(i) else {
                break;
            };
            let path = resolve(r.dir, name);
            if let Err(e) = std::fs::write(path, ostream(x)) {
                g.errno = log::errno_of(&e).unwrap_or(2);
                error += 1;
            }
        }
        self.write_failed |= error != 0;
        error == 0
    }

    /// `writeWeights(nweights, oneminusweights)`: ` No weights constructed, skipping output of
    /// weights` without them; else each written (as `1 - w`, `MRIlinearScale(w, -1, 1)`, unless
    /// `--oneminusw`).
    pub fn write_weights(
        &mut self,
        g: &mut Globals,
        r: &mut Run,
        names: &[String],
        oneminusweights: bool,
    ) -> Result<bool, Fatal> {
        let mut error = 0;
        for (i, name) in names.iter().enumerate() {
            let Some(w) = self.mri_weights.get_mut(i).and_then(Option::as_mut) else {
                let _ = writeln!(r.out, " No weights constructed, skipping output of weights");
                return Ok(false);
            };
            let ok = if oneminusweights {
                let mut iw = mri_linear_scale(g, r.out, r.err, Pair::New(w), -1.0, 1.0, false)?
                    .into_new()
                    .expect("MRIlinearScale's new volume");
                Self::write(g, r, &mut iw, name)?
            } else {
                Self::write(g, r, w, name)?
            };
            if !ok {
                error += 1;
            }
        }
        self.write_failed |= error != 0;
        Ok(error == 0)
    }
}

/// `ceil(x)` converted to `int` (the official build's `roundsd` and `cvttsd2si`; a value outside
/// `int` the indefinite integer).
fn ceil_int(x: f64) -> i32 {
    let c = x.ceil();
    if c.is_nan() || !(-2_147_483_648.0..2_147_483_648.0).contains(&c) {
        i32::MIN
    } else {
        c as i32
    }
}

/// `vnl_matrix_fixed<double, 4, 4> * vnl_vector_fixed<double, 4>`: each element `m(i, 0) v(0)`,
/// then `+ m(i, k) v(k)`.
fn mat_vec4(m: &Mat4, v: &[f64; 4]) -> [f64; 4] {
    let mut o = [0.0; 4];
    for (i, oi) in o.iter_mut().enumerate() {
        let mut acc = mul64(m[i][0], v[0]);
        for k in 1..4 {
            acc = add64(acc, mul64(m[i][k], v[k]));
        }
        *oi = acc;
    }
    o
}

/// `MyMRI::MRIvoxelXformToRasXform(src, dst, m_vox)`: `vox2ras(dst) m_vox ras2vox(src)` with the
/// `float` matrices widened to `double` (VNL's dynamic products).
fn voxel_xform_to_ras_xform(
    g: &mut Globals,
    r: &mut Run,
    src: &Mri,
    dst: &Mri,
    m_vox: &Mat4,
) -> Result<Mat4, Fatal> {
    let r2v = extract_r_to_i(g, r.out, r.err, &src.vg)?.expect("an invertible geometry");
    let v2r = extract_i_to_r(g, r.out, r.err, &dst.vg)?;
    let a = convert_matrix2vnl(&v2r).to_4x4().expect("4 x 4");
    let b = convert_matrix2vnl(&r2v).to_4x4().expect("4 x 4");
    Ok(dmul(&dmul(&a, m_vox), &b))
}

/// `MyMatrix::AffineTransDistSq(a, b, r)` on `float` `MATRIX`es: `d = a - b`; the squares of its
/// translation (`float` products) summed in `double`, the translation and the last row zeroed;
/// `d^T d` (`MatrixMultiply`); its first three diagonal elements summed in `double`; `(1/5) r r tr
/// + tdq`.
fn affine_trans_dist_sq_matrix(
    g: &mut Globals,
    r: &mut Run,
    a: &Matrix,
    b: &Matrix,
    rr: f64,
) -> Result<f64, Fatal> {
    let mut d = matrix_copy(g, r.out, r.err, Some(a), None)?.unwrap_new();
    let dc = d.clone();
    matrix_subtract(g, r.out, r.err, &dc, b, Some(&mut d))?;
    let mut tdq = 0.0f64;
    for i in 1..=3 {
        let v = d.relt(i, 4);
        tdq = add64(tdq, f64::from(v * v));
        d.set_relt(i, 4, 0.0);
        d.set_relt(4, i, 0.0);
    }
    d.set_relt(4, 4, 0.0);
    let dt = matrix_transpose(g, r.out, r.err, &d, None)?.unwrap_new();
    let dc = d.clone();
    matrix_multiply(g, r.out, r.err, Some(&dt), Some(&dc), Some(&mut d))?;
    let mut tr = 0.0f64;
    for i in 1..=3 {
        tr = add64(tr, f64::from(d.relt(i, i)));
    }
    Ok(add64(mul64(mul64(mul64(1.0 / 5.0, rr), rr), tr), tdq))
}

/// `MultiRegistration::averageSet(set, mean, method, sat)`: 0 the mean (`    using mean`,
/// `MRIaverage` of each in turn), 1 the median (` using median `, each voxel's median of the
/// images' values, `RobustGaussian<float>::medianI`), 2 the Tukey biweight of each voxel's values
/// (`    using tukey biweight`, `Regression<float>::getRobustEst`), 3 the joint Tukey biweight of
/// every voxel at once (`    using tukey biweight (alltoghether)`); a new mean is a clone of the
/// first image. Another method: ` averageSet  method <m> unknown` on standard error, status 1.
/// Then `       -- Template : (<x>, <y>, <z>)mm and (<w>, <h>, <d>) voxels.`
///
/// FS-1095 (memory): method 3 builds a matrix of `n v` rows and `v` columns for `v` voxels
/// (`vnl_matrix<float>`, 4 `n v^2` bytes: 64 GiB for two images of 64^3): FreeSurfer runs out of
/// memory for any image of a useful size. Both modes refuse an image of more than 2,048 voxels
/// (FreeSurfer's matrix then passes 32 MiB), noted.
pub fn average_set(
    g: &mut Globals,
    r: &mut Run,
    set: &[&Mri],
    mean: Option<Mri>,
    method: i32,
    sat: f64,
) -> Result<Mri, Fatal> {
    let s0 = set[0];
    let (w, h, d) = (s0.vg.width, s0.vg.height, s0.vg.depth);
    let mut mean = mean;
    match method {
        0 => {
            let _ = writeln!(r.out, "    using mean");
            for (i, s) in set.iter().enumerate() {
                mean = Some(match mean.take() {
                    Some(mut m) => {
                        mri_average(g, r.out, r.err, Pair::Given(s, &mut m), i as i32)?;
                        m
                    }
                    None => mri_average(g, r.out, r.err, Pair::New(s), i as i32)?
                        .into_new()
                        .expect("MRIaverage's new volume"),
                });
            }
        }
        1 | 2 => {
            if method == 1 {
                let _ = writeln!(r.out, " using median ");
            } else {
                let _ = writeln!(r.out, "    using tukey biweight");
            }
            let mut m = match mean.take() {
                Some(m) => m,
                None => mri_clone(g, r.out, r.err, s0, None)?
                    .into_new()
                    .expect("MRIclone's new volume"),
            };
            let mut dd = vec![0.0f32; set.len()];
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        for (i, s) in set.iter().enumerate() {
                            dd[i] = mri_get_vox_val(g, r.err, s, x, y, z, 0, 0)?;
                        }
                        let v = if method == 1 {
                            let mut t = dd.clone();
                            median_i(g, &mut t).0
                        } else {
                            let mut b = dd.clone();
                            let mut reg = Regression::<f32>::new_b(&mut b);
                            let p = reg.get_robust_est(
                                g,
                                r.out,
                                r.err,
                                sat,
                                larmorx_freesurfer_robust::regression::SIG,
                            )?;
                            p[0]
                        };
                        mri_set_vox_val(g, r.err, &mut m, x, y, z, 0, v, 0)?;
                    }
                }
            }
            mean = Some(m);
        }
        3 => {
            let _ = writeln!(r.out, "    using tukey biweight (alltoghether)");
            let n = (w as i64) * (h as i64) * (d as i64);
            if n > 2048 {
                g.notices.add(
                    &FS_1095,
                    format!(
                        "--average 3 of images of {n} voxels: FreeSurfer's matrix of {} rows and {n} columns does not fit in memory",
                        n * set.len() as i64
                    ),
                );
                let _ = r.out.flush();
                let _ = writeln!(
                    r.err,
                    "ERROR: --average 3 needs a matrix of {} x {n} floats (FreeSurfer runs out of memory here)",
                    n * set.len() as i64
                );
                return Err(Fatal::Exit(1));
            }
            let mut m = match mean.take() {
                Some(m) => m,
                None => mri_clone(g, r.out, r.err, s0, None)?
                    .into_new()
                    .expect("MRIclone's new volume"),
            };
            let ns = set.len();
            let nv = n as usize;
            let mut a = larmorx_freesurfer_robust::real::Mat::<f32>::zeros(ns * nv, nv);
            let mut b = vec![0.0f32; ns * nv];
            for (i, s) in set.iter().enumerate() {
                let mut pcount = 0usize;
                for z in 0..d {
                    for y in 0..h {
                        for x in 0..w {
                            b[pcount * ns + i] = mri_get_vox_val(g, r.err, s, x, y, z, 0, 0)?;
                            *a.at_mut(pcount * ns + i, pcount) = 1.0;
                            pcount += 1;
                        }
                    }
                }
            }
            let mut reg = Regression::<f32>::new(&a, &mut b);
            let p = reg.get_robust_est(
                g,
                r.out,
                r.err,
                sat,
                larmorx_freesurfer_robust::regression::SIG,
            )?;
            let mut pcount = 0usize;
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        mri_set_vox_val(g, r.err, &mut m, x, y, z, 0, p[pcount], 0)?;
                        pcount += 1;
                    }
                }
            }
            mean = Some(m);
        }
        _ => {
            let _ = r.out.flush();
            let _ = writeln!(r.err, " averageSet  method {method} unknown");
            return Err(Fatal::Exit(1));
        }
    }
    let m = mean.expect("the mean");
    let c = |v: f32| larmorx_core::cfmt::g(f64::from(v), 6);
    let _ = writeln!(
        r.out,
        "       -- Template : ({}, {}, {})mm and ({}, {}, {}) voxels.",
        c(m.vg.xsize),
        c(m.vg.ysize),
        c(m.vg.zsize),
        m.vg.width,
        m.vg.height,
        m.vg.depth
    );
    Ok(m)
}
