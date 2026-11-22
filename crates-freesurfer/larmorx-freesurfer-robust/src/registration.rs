// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `Registration` and `RegRobust` (queue 3.2-03, part 2): the robust symmetric registration of two
//! volumes (Reuter et al., NeuroImage 2010), translated from FreeSurfer 8.2.0's
//! `mri_robust_register/Registration.cpp` (77-678, 2893-3404, 3533-4709), `Registration.h`,
//! `RegRobust.cpp` and `RegRobust.h`:
//!
//! - [`RegRobust::set_source_and_target`] (`setSourceAndTarget`): copies of both images, the
//!   source's axes reordered towards the target's (`reorientSource`), both resampled to isotropic
//!   voxels of the larger of their smallest sides (`needReslice`, `makeIsotropic`: a cubic B-spline,
//!   `float` unless the type is kept), the resampling matrices `Rsrc`, `Rtrg`, the background
//!   warnings and the transformation model (`setTransformation`);
//! - [`RegRobust::compute_multires_registration`] (`computeMultiresRegistration`): the Gaussian
//!   pyramids (`getGPLimits`, `buildGPLimits`: a 5-tap binomial blur and a B-spline halving), the
//!   initial transform (`initializeTransform`: RAS, the image sizes, the principal axes or the
//!   centroids), and the iterative registration on each level from the coarsest;
//! - [`RegRobust::compute_iterative_registration`] (`computeIterativeRegistration`) on the full
//!   images; [`RegRobust::find_saturation`] (`findSaturation`, `findSatMultiRes`): the bisection of
//!   the saturation by the weights' check on the coarse levels;
//! - the iteration itself (`iterativeRegistrationHelper<T>`, `T` float unless `doubleprec`): each
//!   step maps both images half way (`mapToNewSpace`: the matrix square root, `checkSqrtM`),
//!   computes a [`RegistrationStep`] and composes it, until the transform changes by at most
//!   `epsit` (and the intensity's log by at most 0.001); then the weights mapped to the target's
//!   space and the half-way maps;
//! - [`RegRobust::get_final_vox2vox`], [`RegRobust::get_half_way_maps`],
//!   [`RegRobust::get_minit_resampled`], the centroids (`getCentroidS`, `getCentroidT`,
//!   `getCentroidSinT`) and [`RegRobust::convert_p2md`].
//!
//! Not reached by either program: `flipInputs`, `init_scaling`, `buildGaussianPyramid` (only
//! `testRobust`), `setSource`, `setTarget`, `RegRobust::estimateIScale` (no caller). `testRobust`
//! (`--test`) and the `--debug` writes (the intermediate images, the keypress waits) come with
//! `mri_robust_register` (queue 3.2-13): a debug level above 0 ends here with "not ported yet".

use std::io::Write;

use larmorx_core::cfmt::{self, ostream};
use larmorx_core::defects::FS_1068;
use larmorx_core::math::Libm;
use larmorx_core::vnl_fixed;
use larmorx_core::vnl_matrix::{VnlMatrix, matlab_print_long};
use larmorx_freesurfer_core::cstd::note_glibc07;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_OUT_OF_BOUNDS, Fatal,
};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse::{add64, div64, mul64, sqrt64, sub, sub64};
use larmorx_freesurfer_filter::conv::{mri_convolve_gaussian, mri_gaussian1d};
use larmorx_freesurfer_io::reorder::mri_reorder;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri, mri_alloc, mri_alloc_header, mri_copy_header};
use larmorx_freesurfer_mri::vol_geom::extract_i_to_r;
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_volume::arith::{Pair, mri_clone};
use larmorx_freesurfer_volume::bspline::mri_downsample2_bspline;
use larmorx_freesurfer_volume::orient::mri_get_voxel_to_voxel_xform;
use larmorx_freesurfer_volume::resample::mri_resample;
use larmorx_freesurfer_volume::resize::mri_seq_change_type;
use larmorx_freesurfer_volume::xform::mri_get_resample_matrix;

use crate::costs::{centroid, orientation};
use crate::mymatrix::{
    Mat4, affine_trans_dist_sq, convert_matrix2vnl, get_frobenius_diff, matrix_sqrt,
    polar_decomposition, polar2_decomposition, rigid_trans_dist_sq,
};
use crate::mymri::{find_right_size, get_background, mri_linear_transform, mri_valscale, wide};
use crate::real::Real;
use crate::regstep::{Cost, RegistrationStep, StepParams};
use crate::transformation::{Kind, Transformation};

/// `SAMPLE_TRILINEAR` (`include/mri.h`): the only sample type `setSampleType` accepts.
pub const SAMPLE_TRILINEAR: i32 = 1;
/// `SAMPLE_CUBIC_BSPLINE`: `makeIsotropic`'s resampling.
const SAMPLE_CUBIC_BSPLINE: i32 = 5;

/// The 4 x 4 identity.
pub fn identity4() -> Mat4 {
    let mut m = [[0.0; 4]; 4];
    for (i, row) in m.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    m
}

/// `vnl_matrix_fixed<double, 4, 4>`'s product (`vnl_matrix_fixed_mat_mat_mult`): each element
/// `a(i, 0) b(0, j)`, then `+ a(i, k) b(k, j)` for `k` from 1 (the sum the first operand).
pub fn mul4(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut o = [[0.0; 4]; 4];
    for (i, row) in o.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            let mut acc = mul64(a[i][0], b[0][j]);
            for k in 1..4 {
                acc = add64(acc, mul64(a[i][k], b[k][j]));
            }
            *v = acc;
        }
    }
    o
}

/// `vnl_inverse(vnl_matrix_fixed<double, 4, 4>)`.
pub fn inv4(m: &Mat4) -> Mat4 {
    vnl_fixed::inverse4(m)
}

/// `vnl_inverse(vnl_matrix<double>)` of a 4 x 4 matrix (the same closed form).
fn inv_dyn(m: &VnlMatrix) -> VnlMatrix {
    let flat: Vec<f64> = (0..m.rows)
        .flat_map(|i| (0..m.cols).map(move |j| (i, j)))
        .map(|(i, j)| m.at(i, j))
        .collect();
    let inv = vnl_fixed::inverse(&flat, m.rows);
    let mut r = VnlMatrix::zeros(m.rows, m.cols);
    for i in 0..m.rows {
        for j in 0..m.cols {
            *r.at_mut(i, j) = inv[i * m.cols + j];
        }
    }
    r
}

/// `vnl_determinant` of a 4 x 4 matrix.
fn det4(m: &Mat4) -> f64 {
    let flat: Vec<f64> = m.iter().flatten().copied().collect();
    vnl_fixed::determinant(&flat, 4)
}

fn fixed(m: &VnlMatrix) -> Mat4 {
    m.to_4x4().expect("a 4 x 4 transform")
}

fn dynm(m: &Mat4) -> VnlMatrix {
    VnlMatrix::from_4x4(m)
}

/// `vnl_matlab_print(cout, M, name, vnl_matlab_print_format_long)` and `std::endl`.
fn print_m(out: &mut dyn Write, m: &Mat4, name: &str) {
    let _ = writeln!(out, "{}", matlab_print_long(&dynm(m), name));
}

/// `RegRobust` with the fields of its base `Registration` (both programs make only this one for
/// the robust cost; `RegPowell` comes with `mri_robust_register`'s other costs).
pub struct RegRobust {
    pub iscale: bool,
    pub iscaleonly: bool,
    pub transonly: bool,
    pub rigid: bool,
    pub isoscale: bool,
    pub affine: bool,
    /// `trans`: the transformation model (`setTransformation`).
    pub trans: Option<Transformation>,
    pub subsamplesize: i32,
    pub minsize: i32,
    pub maxsize: i32,
    pub name: String,
    pub nbase: String,
    pub debug: i32,
    pub verbose: i32,
    pub initorient: bool,
    pub inittransform: bool,
    pub initscaling: bool,
    pub highit: i32,
    pub mri_source: Option<Mri>,
    /// `gpS`, `gpT`: the Gaussian pyramids (finest first).
    pub gp_s: Vec<Mri>,
    pub centroid_s: Vec<f64>,
    pub mri_target: Option<Mri>,
    pub gp_t: Vec<Mri>,
    pub centroid_t: Vec<f64>,
    /// `Minit` (empty: `None`).
    pub minit: Option<VnlMatrix>,
    /// `Mfinal` (empty: `None`).
    pub mfinal: Option<VnlMatrix>,
    pub iscaleinit: f64,
    pub iscalefinal: f64,
    pub doubleprec: bool,
    pub symmetry: bool,
    pub sampletype: i32,
    pub resample: bool,
    /// `Rsrc`, `Rtrg`: from the resampled images back to the inputs (empty before
    /// `setSourceAndTarget`).
    pub rsrc: Option<VnlMatrix>,
    pub rtrg: Option<VnlMatrix>,
    pub mov2weights: Option<VnlMatrix>,
    pub dst2weights: Option<VnlMatrix>,
    pub costfun: Cost,
    pub converged: bool,
    /// `wcheck`, `wchecksqrt` (from the last step).
    pub wcheck: f64,
    pub wchecksqrt: f64,
    pub sat: f64,
    pub wlimit: f64,
    /// `getWeights()`: the last weights in the target's space.
    pub mri_weights: Option<Mri>,
    pub mri_hweights: Option<Mri>,
    pub mri_indexing: Option<Mri>,
}

impl Default for RegRobust {
    fn default() -> Self {
        RegRobust::new()
    }
}

/// The streams and environment every call writes to and reads.
pub struct Io<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: &'a Environment,
}

impl RegRobust {
    /// `RegRobust()`: `Registration()`'s defaults (rigid, symmetric, robust, trilinear, verbose
    /// 1, initial transform from the centroids), saturation -1, weight limit 0.16.
    pub fn new() -> RegRobust {
        RegRobust {
            iscale: false,
            iscaleonly: false,
            transonly: false,
            rigid: true,
            isoscale: false,
            affine: false,
            trans: None,
            subsamplesize: -1,
            minsize: -1,
            maxsize: -1,
            name: String::new(),
            nbase: String::new(),
            debug: 0,
            verbose: 1,
            initorient: false,
            inittransform: true,
            initscaling: false,
            highit: -1,
            mri_source: None,
            gp_s: Vec::new(),
            centroid_s: Vec::new(),
            mri_target: None,
            gp_t: Vec::new(),
            centroid_t: Vec::new(),
            minit: None,
            mfinal: None,
            iscaleinit: 1.0,
            iscalefinal: 1.0,
            doubleprec: false,
            symmetry: true,
            sampletype: SAMPLE_TRILINEAR,
            resample: false,
            rsrc: None,
            rtrg: None,
            mov2weights: None,
            dst2weights: None,
            costfun: Cost::Rob,
            converged: false,
            wcheck: 0.0,
            wchecksqrt: 0.0,
            sat: -1.0,
            wlimit: 0.16,
            mri_weights: None,
            mri_hweights: None,
            mri_indexing: None,
        }
    }

    /// `RegRobust::clear()`: `Registration::clear()` (the robust cost, no intensity scaling, no
    /// subsampling, no orientation start, debug 0, `Minit`, `Mfinal` and the half-way maps
    /// emptied; the images, pyramids, resampling matrices and model kept), saturation -1, the
    /// weights and the index image freed.
    pub fn clear(&mut self) {
        self.costfun = Cost::Rob;
        self.iscale = false;
        self.subsamplesize = -1;
        self.initorient = false;
        self.debug = 0;
        self.minit = None;
        self.mfinal = None;
        self.mov2weights = None;
        self.dst2weights = None;
        self.sat = -1.0;
        self.mri_indexing = None;
        self.mri_weights = None;
        self.mri_hweights = None;
    }

    pub fn set_transonly(&mut self) {
        self.rigid = false;
        self.transonly = true;
        self.isoscale = false;
        self.affine = false;
        self.iscaleonly = false;
    }

    pub fn set_iscale_only(&mut self) {
        self.rigid = false;
        self.transonly = false;
        self.isoscale = false;
        self.affine = false;
        self.iscale = true;
        self.initorient = false;
        self.inittransform = false;
        self.initscaling = false;
        self.iscaleonly = true;
    }

    pub fn set_rigid(&mut self) {
        self.rigid = true;
        self.transonly = false;
        self.isoscale = false;
        self.affine = false;
        self.iscaleonly = false;
    }

    pub fn set_isoscale(&mut self) {
        self.rigid = false;
        self.transonly = false;
        self.isoscale = true;
        self.affine = false;
        self.iscaleonly = false;
    }

    pub fn set_affine(&mut self) {
        self.rigid = false;
        self.transonly = false;
        self.isoscale = false;
        self.affine = true;
        self.iscaleonly = false;
    }

    /// `setDebug(d)`: a positive level also sets verbose 2.
    pub fn set_debug(&mut self, d: i32) {
        self.debug = d;
        if d > 0 {
            self.verbose = 2;
        }
    }

    /// `setVerbose(i)`: clipped to 0..=2.
    pub fn set_verbose(&mut self, i: i32) {
        self.verbose = i.clamp(0, 2);
    }

    /// `setSampleType(st)`: only `SAMPLE_TRILINEAR`; another one prints `ERROR
    /// Registration:setSampleType: <st> not supported type!` and ends with status 1.
    pub fn set_sample_type(&mut self, out: &mut dyn Write, st: i32) -> Result<(), Fatal> {
        if st != SAMPLE_TRILINEAR {
            let _ = writeln!(
                out,
                "ERROR Registration:setSampleType: {st} not supported type!"
            );
            return Err(Fatal::Exit(1));
        }
        self.sampletype = st;
        Ok(())
    }

    /// `setName(n)`: the name and its part after the last `/`.
    pub fn set_name(&mut self, n: &str) {
        self.name = n.to_owned();
        self.nbase = match n.rfind('/') {
            Some(p) => n[p + 1..].to_owned(),
            None => n.to_owned(),
        };
    }

    /// `getConverged()`.
    pub fn get_converged(&self) -> bool {
        self.converged
    }

    /// `getFinalIscale()`.
    pub fn get_final_iscale(&self) -> f64 {
        self.iscalefinal
    }

    fn not_ported_debug(&self, err: &mut dyn Write) -> Result<(), Fatal> {
        if self.debug > 0 {
            let _ = writeln!(
                err,
                "Registration: the debug output (--debug) is not ported yet"
            );
            return Err(Fatal::Exit(1));
        }
        Ok(())
    }

    /// `setTransformation(is2d)`: the model of the settings (translation, rigid, isotropic scale,
    /// affine, else the identity, which needs `iscale`: `Registration::setTransformation please
    /// specify either --iscale or a transformation model` and `ERROR_BADPARM` otherwise).
    pub fn set_transformation(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        is2d: bool,
    ) -> Result<(), Fatal> {
        let kind = if self.transonly {
            if is2d {
                Kind::Translate2d
            } else {
                Kind::Translate3d
            }
        } else if self.rigid {
            if is2d { Kind::Rigid2d } else { Kind::Rigid3d }
        } else if self.isoscale {
            if is2d {
                Kind::Isoscale2d
            } else {
                Kind::Isoscale3d
            }
        } else if self.affine {
            if is2d { Kind::Affine2d } else { Kind::Affine3d }
        } else {
            self.trans = Some(Transformation::new(Kind::Identity3d));
            if !self.iscale {
                log::error_exit(
                    g,
                    err,
                    ERROR_BADPARM,
                    "Registration::setTransformation please specify either --iscale or a transformation model\n",
                )?;
            }
            return Ok(());
        };
        self.trans = Some(Transformation::new(kind));
        Ok(())
    }

    /// `setSourceAndTarget(s, t, keeptype)`: see the module's notes. Types that differ make both
    /// `float`.
    pub fn set_source_and_target(
        &mut self,
        g: &mut Globals,
        io: &mut Io,
        s: &Mri,
        t: &Mri,
        keeptype: bool,
    ) -> Result<(), Fatal> {
        let mut keeptype = keeptype;
        if self.verbose > 0 {
            let kt = if keeptype { "TRUE" } else { "FALSE" };
            let _ = writeln!(
                io.out,
                "Registration::setSourceAndTarget(MRI s, MRI t, keeptype = {kt} )"
            );
        }
        if s.mtype != t.mtype {
            keeptype = false;
        }
        if self.verbose > 0 {
            let _ = write!(
                io.out,
                "   Type Source : {}  Type Target : {}",
                s.mtype, t.mtype
            );
            if keeptype {
                let _ = writeln!(io.out, "  keeping type ");
            } else {
                let _ = writeln!(io.out, "  ensure both FLOAT ({MRI_FLOAT})");
            }
        }
        self.rsrc = Some(VnlMatrix::identity(4));
        self.rtrg = Some(VnlMatrix::identity(4));
        self.mri_source = Some(mri_copy(g, io.out, io.err, s, None)?.unwrap_new());
        self.mri_target = Some(mri_copy(g, io.out, io.err, t, None)?.unwrap_new());
        self.reorient_source(g, io)?;

        let src = self.mri_source.as_ref().expect("the source");
        let trg = self.mri_target.as_ref().expect("the target");
        // the smallest side of each (a 2-D image's depth not counted), the larger of the two
        let mut mins = wide(src.vg.xsize.abs());
        if wide(src.vg.ysize.abs()) < mins {
            mins = wide(src.vg.ysize.abs());
        }
        if src.vg.depth > 1 && wide(src.vg.zsize.abs()) < mins {
            mins = wide(src.vg.zsize.abs());
        }
        let mut mint = wide(trg.vg.xsize.abs());
        if wide(trg.vg.ysize.abs()) < mint {
            mint = wide(t.vg.ysize.abs());
        }
        if t.vg.depth > 1 && wide(trg.vg.zsize.abs()) < mint {
            mint = wide(trg.vg.zsize.abs());
        }
        let isosize = if mint > mins { mint } else { mins };
        let mut s_dim = find_right_size(g, src, isosize as f32, false);
        let mut t_dim = find_right_size(g, trg, isosize as f32, false);
        if self.symmetry {
            for i in 0..3 {
                if s_dim[i] < t_dim[i] {
                    s_dim[i] = t_dim[i];
                } else {
                    t_dim[i] = s_dim[i];
                }
            }
        }
        if self.verbose > 0 {
            let f = |v: f32| ostream(wide(v));
            let _ = writeln!(
                io.out,
                "   Mov: ({}, {}, {})mm  and dim ({}, {}, {})",
                f(src.vg.xsize),
                f(src.vg.ysize),
                f(src.vg.zsize),
                src.vg.width,
                src.vg.height,
                src.vg.depth
            );
            let _ = writeln!(
                io.out,
                "   Dst: ({}, {}, {})mm  and dim ({}, {}, {})",
                f(trg.vg.xsize),
                f(trg.vg.ysize),
                f(trg.vg.zsize),
                trg.vg.width,
                trg.vg.height,
                trg.vg.depth
            );
            let _ = writeln!(
                io.out,
                "   Asserting both images: {}mm isotropic ",
                ostream(isosize)
            );
        }

        // the source
        let src = self.mri_source.take().expect("the source");
        if self.need_reslice(&src, isosize, s_dim[0], s_dim[1], s_dim[2], keeptype) {
            if self.verbose > 0 {
                let _ = writeln!(io.out, "    - reslicing Mov ...");
            }
            let (m, rm) =
                self.make_isotropic(g, io, &src, isosize, s_dim[0], s_dim[1], s_dim[2], keeptype)?;
            self.rsrc = Some(self.rsrc.take().expect("Rsrc").mul(&dynm(&rm)));
            self.mri_source = Some(m.expect("a resliced source"));
            self.not_ported_debug(io.err)?;
        } else {
            if self.verbose > 0 {
                let _ = writeln!(io.out, "    - no Mov reslice necessary");
            }
            self.mri_source = Some(src);
        }
        // the target
        let trg = self.mri_target.take().expect("the target");
        if self.need_reslice(&trg, isosize, t_dim[0], t_dim[1], t_dim[2], keeptype) {
            if self.verbose > 0 {
                let _ = writeln!(io.out, "    - reslicing Dst ...");
            }
            let (m, rm) =
                self.make_isotropic(g, io, &trg, isosize, t_dim[0], t_dim[1], t_dim[2], keeptype)?;
            self.rtrg = Some(self.rtrg.take().expect("Rtrg").mul(&dynm(&rm)));
            self.mri_target = Some(m.expect("a resliced target"));
            self.not_ported_debug(io.err)?;
        } else {
            if self.verbose > 0 {
                let _ = writeln!(io.out, "    - no Dst reslice necessary");
            }
            self.mri_target = Some(trg);
        }

        // the background warnings
        let src = self.mri_source.as_ref().expect("the source");
        let trg = self.mri_target.as_ref().expect("the target");
        let bgvals = get_background(g, io.err, src)?;
        let bgvalt = get_background(g, io.err, trg)?;
        if f64::from(bgvals) != src.outside_val {
            let _ = writeln!(
                io.out,
                "    - WARNING: src mri outside_val = {}  but suspected background: {}",
                ostream(src.outside_val),
                ostream(wide(bgvals))
            );
            let _ = writeln!(
                io.out,
                "               If background is black or darkgray, ignore this. If white, pass --whitebgmov"
            );
        }
        if f64::from(bgvalt) != trg.outside_val {
            let _ = writeln!(
                io.out,
                "    - WARNING: trg mri outside_val = {}  but suspected background: {}",
                ostream(trg.outside_val),
                ostream(wide(bgvalt))
            );
            let _ = writeln!(
                io.out,
                "               If background is black or darkgray, ignore this. If white, pass --whitebgdst"
            );
        }
        self.gp_s.clear();
        self.centroid_s.clear();
        self.gp_t.clear();
        self.centroid_t.clear();
        let is2d = s.vg.depth == 1 || t.vg.depth == 1;
        self.set_transformation(g, io.err, is2d)?;
        if self.verbose > 1 {
            let _ = writeln!(io.out, " DONE setSourceAndTarget ");
        }
        Ok(())
    }

    /// `reorientSource()`: the source's axes reordered (and flipped) so that the voxel-to-voxel
    /// transform to the target (`Minit` resampled, or the images' own) has its largest entry of each
    /// column on the diagonal and positive; a conflict between two columns keeps the third in its
    /// place (else all three). Prints the reordering and the new transform's determinant; one below
    /// 0 with `symmetry` is an error (status 1).
    fn reorient_source(&mut self, g: &mut Globals, io: &mut Io) -> Result<bool, Fatal> {
        let myinit = self.v2v_init(g, io)?;
        let a = |r: usize, c: usize| myinit[r][c].abs();
        let (mut xd, mut yd, mut zd) = (1i32, 1i32, 1i32);
        if a(1, 0) > a(0, 0) {
            xd = 2;
        }
        if a(2, 0) > a(0, 0) && a(2, 0) > a(1, 0) {
            xd = 3;
        }
        if a(1, 1) > a(0, 1) {
            yd = 2;
        }
        if a(2, 1) > a(0, 1) && a(2, 1) > a(1, 1) {
            yd = 3;
        }
        if a(1, 2) > a(0, 2) {
            zd = 2;
        }
        if a(2, 2) > a(0, 2) && a(2, 2) > a(1, 2) {
            zd = 3;
        }
        if myinit[(xd - 1) as usize][0] < 0.0 {
            xd = -xd;
        }
        if myinit[(yd - 1) as usize][1] < 0.0 {
            yd = -yd;
        }
        if myinit[(zd - 1) as usize][2] < 0.0 {
            zd = -zd;
        }
        if xd == 1 && yd == 2 && zd == 3 {
            return Ok(false);
        }
        if xd.abs() * yd.abs() * zd.abs() != 6 {
            let _ = writeln!(
                io.out,
                "  Registration::reorientSource: reorder not clear, solving conflict ..."
            );
            print_m(io.out, &myinit, "v2v");
            if xd.abs() == 2 && yd.abs() == 1 {
                zd = 3;
                if myinit[2][2] < 0.0 {
                    zd = -zd;
                }
            } else if xd.abs() == 3 && zd.abs() == 1 {
                yd = 2;
                if myinit[1][1] < 0.0 {
                    yd = -yd;
                }
            } else if yd.abs() == 3 && zd.abs() == 2 {
                xd = 1;
                if myinit[0][0] < 0.0 {
                    xd = -xd;
                }
            } else {
                xd = 1;
                if myinit[0][0] < 0.0 {
                    xd = -xd;
                }
                yd = 2;
                if myinit[1][1] < 0.0 {
                    yd = -yd;
                }
                zd = 3;
                if myinit[2][2] < 0.0 {
                    zd = -zd;
                }
            }
        }
        let _ = writeln!(
            io.out,
            "   Reordering axes in mov to better fit dst... ( {xd} {yd} {zd} )"
        );
        self.resample = true;
        let temp = self.mri_source.take().expect("the source");
        let reordered = match mri_reorder(g, io.out, io.err, &temp, None, xd, yd, zd)?.into_new() {
            Some(m) => m,
            None => return Err(crash(io.err, "MRIreorder")),
        };
        let v2v = mri_get_voxel_to_voxel_xform(g, io.out, io.err, &reordered, &temp)?;
        let sreorderinv = convert_matrix2vnl(&v2v.ok_or_else(|| crash(io.err, "v2v"))?);
        drop(temp);
        self.mri_source = Some(reordered);
        self.not_ported_debug(io.err)?;
        self.rsrc = Some(self.rsrc.take().expect("Rsrc").mul(&sreorderinv));
        let v2vfull2 = self.v2v_init(g, io)?;
        let det = det4(&v2vfull2);
        let _ = writeln!(io.out, " Determinant after swap : {}", ostream(det));
        if det < 0.0 && self.symmetry {
            let _ = writeln!(io.out, "ERROR: vox2vox det: {} < 0", ostream(det));
            print_m(io.out, &v2vfull2, "v2v");
            let _ = writeln!(
                io.out,
                "       Symmetry does not work with flipped volumes."
            );
            let _ = writeln!(
                io.out,
                "       Make sure volumes are in same voxel orientation."
            );
            let _ = writeln!(io.out, "       Or run with --nosym flag.");
            return Err(Fatal::Exit(1));
        }
        Ok(true)
    }

    /// `Minit` resampled, or `MRIgetVoxelToVoxelXform(source, target)` widened.
    fn v2v_init(&mut self, g: &mut Globals, io: &mut Io) -> Result<Mat4, Fatal> {
        if let Some(m) = self.get_minit_resampled() {
            return Ok(fixed(&m));
        }
        let src = self.mri_source.as_ref().expect("the source");
        let trg = self.mri_target.as_ref().expect("the target");
        let v2v = mri_get_voxel_to_voxel_xform(g, io.out, io.err, src, trg)?;
        Ok(fixed(&convert_matrix2vnl(
            &v2v.ok_or_else(|| crash(io.err, "v2v"))?,
        )))
    }

    /// `needReslice(mri, vsize, xdim, ydim, zdim, keeptype)`: whether the image needs another type
    /// (not `float` and the type not kept) or other voxels (not already of `vsize`, or of other
    /// dimensions than the given ones; no `vsize`: not isotropic; a 2-D image's depth counted as
    /// its height).
    fn need_reslice(
        &self,
        mri: &Mri,
        vsize: f64,
        xdim: i32,
        ydim: i32,
        zdim: i32,
        keeptype: bool,
    ) -> bool {
        let (mw, mh, md) = (mri.vg.width, mri.vg.height, mri.vg.depth);
        let mx = wide(mri.vg.xsize);
        let my = wide(mri.vg.ysize);
        let mut mz = wide(mri.vg.zsize);
        if md == 1 {
            mz = my;
        }
        let notypeconvert = keeptype || mri.mtype == MRI_FLOAT;
        let mut novoxconvert = vsize < 0.0 && mx == my && my == mz;
        let eps = 0.0001;
        let conformvsize = sub64(mx, vsize).abs() < eps
            && sub64(my, vsize).abs() < eps
            && sub64(mz, vsize).abs() < eps;
        novoxconvert = novoxconvert || (xdim < 0 && ydim < 0 && zdim < 0 && conformvsize);
        let dimagree = xdim == mw && ydim == mh && zdim == md;
        novoxconvert = novoxconvert || (conformvsize && dimagree);
        !(novoxconvert && notypeconvert)
    }

    /// `makeIsotropic(mri, NULL, vsize, xdim, ydim, zdim, keeptype)`: a copy of the image, of
    /// `float` unless the type is kept (`MRISeqchangeType`, no scaling), resampled with a cubic
    /// B-spline onto isotropic voxels of `vsize` (the smallest side when negative) of the given or
    /// the covering dimensions, the directions and centre kept; and the resampling matrix
    /// (identity when nothing is resampled). `None` and the identity when no reslice is needed.
    #[allow(clippy::too_many_arguments)]
    fn make_isotropic(
        &mut self,
        g: &mut Globals,
        io: &mut Io,
        mri: &Mri,
        vsize: f64,
        xdim: i32,
        ydim: i32,
        zdim: i32,
        keeptype: bool,
    ) -> Result<(Option<Mri>, Mat4), Fatal> {
        let mut rm = identity4();
        if !self.need_reslice(mri, vsize, xdim, ydim, zdim, keeptype) {
            return Ok((None, rm));
        }
        let mut out = mri_copy(g, io.out, io.err, mri, None)?.unwrap_new();
        if self.verbose > 1 {
            let _ = writeln!(io.out, "     - will resample image or adjust type");
        }
        let mut conform_size = vsize;
        if conform_size < 0.0 {
            conform_size = wide(mri.vg.xsize);
            if wide(mri.vg.ysize) < conform_size {
                conform_size = wide(mri.vg.ysize);
            }
            if wide(mri.vg.zsize) < conform_size {
                conform_size = wide(mri.vg.zsize);
            }
        }
        let mut dims = find_right_size(g, &out, conform_size as f32, false);
        if self.verbose > 1 {
            let _ = writeln!(
                io.out,
                "     - conform dim: {} {} {}",
                dims[0], dims[1], dims[2]
            );
        }
        for (k, want) in [xdim, ydim, zdim].into_iter().enumerate() {
            if want > 0 {
                if want < dims[k] {
                    let axis = ["xdim", "ydim", "zdim"][k];
                    let _ = writeln!(
                        io.err,
                        "Registration::makeConform specified {axis} too small to cover image"
                    );
                    return Err(Fatal::Exit(1));
                }
                dims[k] = want;
            }
        }
        let mut temp =
            mri_alloc_header(g, io.out, io.err, dims[0], dims[1], dims[2], MRI_FLOAT, 1)?;
        mri_copy_header(g, io.out, io.err, mri, Some(&mut temp))?;
        temp.mtype = MRI_FLOAT;
        temp.vg.width = dims[0];
        temp.vg.height = dims[1];
        temp.vg.depth = dims[2];
        temp.imnr0 = 1;
        temp.imnr1 = temp.vg.depth;
        let cs = conform_size as f32;
        temp.thick = cs;
        temp.ps = cs;
        temp.vg.xsize = cs;
        temp.vg.ysize = cs;
        temp.vg.zsize = cs;
        if mri.vg.depth == 1 {
            temp.vg.zsize = mri.vg.zsize;
        }
        temp.xstart = (-dims[0] / 2) as f32;
        temp.ystart = (-dims[1] / 2) as f32;
        temp.zstart = (-dims[2] / 2) as f32;
        temp.xend = (dims[0] / 2) as f32;
        temp.yend = (dims[1] / 2) as f32;
        temp.zend = (dims[2] / 2) as f32;
        if mri.mtype != temp.mtype && !keeptype {
            let _ = writeln!(
                io.out,
                "       -- changing data type from {} to {} (noscale = 0)...",
                mri.mtype, temp.mtype
            );
            match mri_seq_change_type(
                g, io.out, io.err, io.env, &mut out, temp.mtype, 0.0, 0.999, 0,
            )? {
                Some(m) => out = m,
                None => {
                    let _ = writeln!(io.out, "ERROR: MRISeqchangeType");
                    return Err(Fatal::Exit(1));
                }
            }
        }
        let eps = 0.00001;
        let differs = |a: f32, b: f32| f64::from(sub(a, b).abs()) > eps;
        if differs(mri.vg.xsize, temp.vg.xsize)
            || differs(mri.vg.ysize, temp.vg.ysize)
            || differs(mri.vg.zsize, temp.vg.zsize)
            || mri.vg.width != temp.vg.width
            || mri.vg.height != temp.vg.height
            || mri.vg.depth != temp.vg.depth
        {
            self.resample = true;
            let mtemp = mri_get_resample_matrix(g, io.out, io.err, &out, &temp)?
                .ok_or_else(|| crash(io.err, "MRIgetResampleMatrix"))?;
            rm = fixed(&convert_matrix2vnl(&mtemp));
            let gf = |v: f32| cfmt::g(wide(v), 6);
            let _ = writeln!(
                io.out,
                "       -- Original : ({}, {}, {})mm and ({}, {}, {}) voxels.",
                gf(mri.vg.xsize),
                gf(mri.vg.ysize),
                gf(mri.vg.zsize),
                mri.vg.width,
                mri.vg.height,
                mri.vg.depth
            );
            let _ = writeln!(
                io.out,
                "       -- Resampled: ({}, {}, {})mm and ({}, {}, {}) voxels.",
                gf(temp.vg.xsize),
                gf(temp.vg.ysize),
                gf(temp.vg.zsize),
                temp.vg.width,
                temp.vg.height,
                temp.vg.depth
            );
            let _ = write!(io.out, "       -- Reslicing using ");
            let _ = writeln!(io.out, "cubic bspline ");
            match mri_resample(g, io.out, io.err, &out, &temp, SAMPLE_CUBIC_BSPLINE)? {
                Some(m) => out = m,
                None => {
                    let _ = writeln!(io.err, "makeIsotropic: MRIresample did not return MRI");
                    return Err(Fatal::Exit(1));
                }
            }
        } else {
            let _ = writeln!(
                io.err,
                "makeIsotropic WARNING: not different enough, won't reslice!"
            );
        }
        Ok((Some(out), rm))
    }

    /// `getMinitResampled()`: `Minit` as the voxel transform of the resampled images
    /// (`Rtrg^-1 Minit Rsrc` when they were resampled); `None` without `Minit`.
    pub fn get_minit_resampled(&self) -> Option<VnlMatrix> {
        let minit = self.minit.as_ref()?;
        if !self.resample {
            return Some(minit.clone());
        }
        let rtrg = self.rtrg.as_ref().expect("Rtrg");
        let rsrc = self.rsrc.as_ref().expect("Rsrc");
        Some(inv_dyn(rtrg).mul(minit).mul(rsrc))
    }

    /// `getFinalVox2Vox()`: `Mfinal`, or `Rtrg Mfinal Rsrc^-1` after a resampling (printing
    /// `Adjusting final transform due to initial resampling (voxel or size changes) ...`).
    pub fn get_final_vox2vox(&self, out: &mut dyn Write) -> Mat4 {
        let mfinal = self.mfinal.as_ref().expect("Mfinal");
        if !self.resample {
            return fixed(mfinal);
        }
        let _ = writeln!(
            out,
            "Adjusting final transform due to initial resampling (voxel or size changes) ..."
        );
        let rtrg = self.rtrg.as_ref().expect("Rtrg");
        let rsrc = self.rsrc.as_ref().expect("Rsrc");
        fixed(&rtrg.mul(mfinal).mul(&inv_dyn(rsrc)))
    }

    /// `getHalfWayMaps()`: the maps from the inputs to the half-way space (`mov2weights Rsrc^-1`,
    /// `dst2weights Rtrg^-1` after a resampling). Before any iteration (FS-1068: FreeSurfer reads
    /// empty matrices) an error in both modes, noted.
    pub fn get_half_way_maps(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
    ) -> Result<(Mat4, Mat4), Fatal> {
        let (Some(m), Some(d)) = (self.mov2weights.as_ref(), self.dst2weights.as_ref()) else {
            g.notices.add(
                &FS_1068,
                "the half-way maps of a registration that ran no iteration (FreeSurfer reads empty matrices)",
            );
            let _ = writeln!(
                err,
                "Registration::getHalfWayMaps: no half-way maps, the registration ran no iteration (FreeSurfer crashes here)"
            );
            return Err(Fatal::Exit(1));
        };
        if !self.resample {
            return Ok((fixed(m), fixed(d)));
        }
        let rsrc = self.rsrc.as_ref().expect("Rsrc");
        let rtrg = self.rtrg.as_ref().expect("Rtrg");
        Ok((fixed(&m.mul(&inv_dyn(rsrc))), fixed(&d.mul(&inv_dyn(rtrg)))))
    }

    /// `getCentroidS()`: the source's centroid mapped back to the input (`Rsrc (c, 1)`).
    pub fn get_centroid_s(&self) -> [f64; 4] {
        centroid_back(self.rsrc.as_ref().expect("Rsrc"), &self.centroid_s)
    }

    /// `getCentroidT()`.
    pub fn get_centroid_t(&self) -> [f64; 4] {
        centroid_back(self.rtrg.as_ref().expect("Rtrg"), &self.centroid_t)
    }

    /// `getCentroidSinT()`: the source's centroid in the target (the final transform).
    pub fn get_centroid_s_in_t(&self, out: &mut dyn Write) -> [f64; 4] {
        let c = self.get_centroid_s();
        let v2v = self.get_final_vox2vox(out);
        let mut r = [0.0; 4];
        for (i, ri) in r.iter_mut().enumerate() {
            let mut acc = mul64(v2v[i][0], c[0]);
            for k in 1..4 {
                acc = add64(acc, mul64(v2v[i][k], c[k]));
            }
            *ri = acc;
        }
        r
    }

    /// `convertP2Md(p)`: the model's matrix of the parameters and, with `iscale`, the last one as
    /// the intensity parameter.
    pub fn convert_p2md(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        p: &[f64],
    ) -> Result<(Mat4, f64), Fatal> {
        let mut second = 0.0;
        if self.iscale {
            second = p[p.len() - 1];
        }
        let trans = self.trans.as_mut().expect("a transformation model");
        trans.set_parameters(g, p, false);
        Ok((trans.matrix(g, err)?, second))
    }

    /// `initializeTransform(mriS, mriT)`: the centroids (kept), then the voxel transform from the
    /// images' RAS (without `inittransform`), from their sizes (`initscaling`, only without
    /// `symmetry`), from their principal axes (`initorient`, when no axis turns by more than 84
    /// degrees) or from the centroids' difference.
    fn initialize_transform(
        &mut self,
        g: &mut Globals,
        io: &mut Io,
        mri_s: &Mri,
        mri_t: &Mri,
    ) -> Result<Mat4, Fatal> {
        if self.verbose > 0 {
            let _ = writeln!(io.out, "   - computing centroids ");
        }
        let mut myinit = identity4();
        self.centroid_s = centroid(g, io.err, mri_s)?.to_vec();
        self.centroid_t = centroid(g, io.err, mri_t)?.to_vec();
        let (cs, ct) = (&self.centroid_s, &self.centroid_t);
        if self.verbose > 1 {
            let _ = writeln!(
                io.out,
                "        Centroid S: {}, {}, {}",
                ostream(cs[0]),
                ostream(cs[1]),
                ostream(cs[2])
            );
            let _ = writeln!(
                io.out,
                "        Centroid T: {}, {}, {}",
                ostream(ct[0]),
                ostream(ct[1]),
                ostream(ct[2])
            );
        }
        if !self.inittransform {
            if self.verbose > 0 {
                let _ = writeln!(io.out, "   - initialize transform based on RAS");
            }
            let sv2r = fixed(&convert_matrix2vnl(&extract_i_to_r(
                g, io.out, io.err, &mri_s.vg,
            )?));
            let tv2r = fixed(&convert_matrix2vnl(&extract_i_to_r(
                g, io.out, io.err, &mri_t.vg,
            )?));
            return Ok(mul4(&inv4(&tv2r), &sv2r));
        }
        if self.verbose > 0 {
            let _ = writeln!(io.out, "   - computing initial transform");
        }
        if self.initscaling {
            if self.symmetry {
                let _ = writeln!(
                    io.out,
                    " ERROR: initscaling only meaningful with --nosym (else image sizes will agree)!"
                );
                return Err(Fatal::Exit(1));
            }
            let rw = div64(f64::from(mri_t.vg.width), f64::from(mri_s.vg.width));
            let rh = div64(f64::from(mri_t.vg.height), f64::from(mri_s.vg.height));
            let rd = div64(f64::from(mri_t.vg.depth), f64::from(mri_s.vg.depth));
            let s;
            if mri_t.vg.depth > 1 && mri_s.vg.depth > 1 {
                let libm = Libm::new(g.mode());
                s = libm.pow(mul64(mul64(rw, rh), rd), 1.0 / 3.0);
                note_glibc07(g, &libm, "pow");
                myinit[0][0] = s;
                myinit[1][1] = s;
                myinit[2][2] = s;
            } else {
                s = sqrt64(mul64(rw, rh));
                myinit[0][0] = s;
                myinit[1][1] = s;
            }
            let _ = writeln!(io.out, "     -- adjusting image sizes ( {} ) ", ostream(s));
            return Ok(myinit);
        }
        if self.initorient {
            if self.verbose > 0 {
                let _ = writeln!(
                    io.out,
                    "     -- trying to use orientation info (--initorient) "
                );
            }
            let ev_t = orientation(g, io.err, mri_t)?;
            let mut ev_s = orientation(g, io.err, mri_s)?;
            if self.verbose > 1 {
                let p3 = |m: &[[f64; 3]; 3]| {
                    let mut v = VnlMatrix::zeros(3, 3);
                    for r in 0..3 {
                        for c in 0..3 {
                            *v.at_mut(r, c) = m[r][c];
                        }
                    }
                    v
                };
                let _ = io
                    .err
                    .write_all(matlab_print_long(&p3(&ev_t), "evT").as_bytes());
                let _ = io
                    .err
                    .write_all(matlab_print_long(&p3(&ev_s), "evS").as_bytes());
            }
            for c in 0..3 {
                let mut d = 0.0f64;
                for r in 0..3 {
                    d = add64(d, mul64(ev_t[r][c], ev_s[r][c]));
                }
                if d.abs() < 0.1 {
                    let deg = div64(
                        mul64(360.0, larmorx_core::math::acos(d.abs())),
                        std::f64::consts::PI,
                    );
                    let _ = writeln!(
                        io.out,
                        "       WARNING: Over 84 degree difference ( {} ) in column {c}, skipping pre-orientation",
                        ostream(deg)
                    );
                    self.initorient = false;
                    break;
                }
                if d < 0.0 {
                    if self.verbose > 1 {
                        let _ = writeln!(io.out, "       --- flipping column {} ", c + 1);
                    }
                    for row in ev_s.iter_mut() {
                        row[c] = -row[c];
                    }
                }
            }
            if self.initorient {
                if self.verbose > 0 {
                    let _ = writeln!(io.out, "     -- using orientation info");
                }
                let (mut evs2, mut evt2) = (identity4(), identity4());
                for c in 0..3 {
                    for r in 0..3 {
                        evs2[r][c] = ev_s[r][c];
                        evt2[r][c] = ev_t[r][c];
                    }
                }
                for i in 0..3 {
                    myinit[i][3] = sub64(myinit[i][3], self.centroid_s[i]);
                }
                let evs2i = inv4(&evs2);
                myinit = mul4(&mul4(&evt2, &evs2i), &myinit);
                for i in 0..3 {
                    myinit[i][3] = add64(myinit[i][3], self.centroid_t[i]);
                }
            }
        }
        if !self.initorient {
            if self.verbose > 0 {
                let _ = writeln!(io.out, "     -- using translation info");
            }
            for i in 0..3 {
                myinit[i][3] = add64(myinit[i][3], sub64(self.centroid_t[i], self.centroid_s[i]));
            }
        }
        Ok(myinit)
    }

    /// `getGPLimits(mriS, mriT, min, max)`: how many halvings keep the smallest side (a 2-D
    /// pair's depth not counted) at least `min` (`maxsubdiv`), and from how many on the largest
    /// of the per-axis smaller sides is below `max` (`minsubdiv`; 0 without `max`). A side below
    /// `min`: FreeSurfer's four lines and status 1.
    fn get_gp_limits(
        &self,
        out: &mut dyn Write,
        mri_s: &Mri,
        mri_t: &Mri,
        min: i32,
        max: i32,
    ) -> Result<(i32, i32), Fatal> {
        if self.verbose > 0 {
            let _ = writeln!(
                out,
                "   - Get Gaussian Pyramid Limits ( min size: {min} max size: {max} ) "
            );
        }
        let (s, t) = (&mri_s.vg, &mri_t.vg);
        let mut smallest = s.width;
        if t.width < smallest {
            smallest = t.width;
        }
        if s.height < smallest {
            smallest = s.height;
        }
        if t.height < smallest {
            smallest = t.height;
        }
        if s.depth != 1 || t.depth != 1 {
            if s.depth < smallest {
                smallest = s.depth;
            }
            if t.depth < smallest {
                smallest = t.depth;
            }
        }
        if smallest < min {
            let _ = writeln!(out);
            let _ = writeln!(
                out,
                " Input image is smaller than min dimension: {smallest} < {min}"
            );
            let _ = writeln!(
                out,
                " Input dimensions (after conformalizing) MOV: {} {} {}",
                s.width, s.height, s.depth
            );
            let _ = writeln!(
                out,
                " Input dimensions (after conformalizing) DST: {} {} {}",
                t.width, t.height, t.depth
            );
            let _ = writeln!(out, " Specify larger minimum ...");
            return Err(Fatal::Exit(1));
        }
        let mut temp = smallest / 2;
        let mut maxsubdiv = 0;
        while temp > min {
            temp /= 2;
            maxsubdiv += 1;
        }
        if max == -1 {
            return Ok((0, maxsubdiv));
        }
        let t1 = if s.width < t.width { s.width } else { t.width };
        let t2 = if s.height < t.height {
            s.height
        } else {
            t.height
        };
        let t3 = if s.depth < t.depth { s.depth } else { t.depth };
        temp = t1;
        if t2 > temp {
            temp = t2;
        }
        if t3 > temp {
            temp = t3;
        }
        let mut minsubdiv = 0;
        while minsubdiv < maxsubdiv {
            if temp < max {
                break;
            }
            temp /= 2;
            minsubdiv += 1;
        }
        Ok((minsubdiv, maxsubdiv))
    }

    /// `buildGPLimits(mri_in, limits)`: `limits.1 - limits.0 + 1` levels: the image halved
    /// `limits.0` times, then each next level the previous one blurred (`MRIconvolveGaussian`
    /// with the kernel 1/16, 1/4, 3/8, 1/4, 1/16) and halved (`MRIdownsample2BSpline`); every
    /// level with the input's outside value.
    fn build_gp_limits(
        &self,
        g: &mut Globals,
        io: &mut Io,
        mri_in: &Mri,
        limits: (i32, i32),
    ) -> Result<Vec<Mri>, Fatal> {
        if self.verbose > 0 {
            let _ = writeln!(
                io.out,
                "   - Build Gaussian Pyramid ( Limits min steps: {} max steps: {} ) ",
                limits.0, limits.1
            );
        }
        let mut kernel = mri_gaussian1d(g, io.out, io.err, 1.08, 5)?;
        for (x, v) in [0.0625f32, 0.25, 0.375, 0.25, 0.0625]
            .into_iter()
            .enumerate()
        {
            mri_set_vox_val(g, io.err, &mut kernel, x as i32, 0, 0, 0, v, 0)?;
        }
        if self.verbose > 1 {
            let _ = writeln!(
                io.out,
                "        dim: {} {} {}",
                mri_in.vg.width, mri_in.vg.height, mri_in.vg.depth
            );
        }
        let mut p: Vec<Mri> = Vec::new();
        let mut first = mri_copy(g, io.out, io.err, mri_in, None)?.unwrap_new();
        let mut i = 0;
        let mut tmp_is_in = true;
        while i < limits.0 {
            let src = if tmp_is_in { mri_in } else { &first };
            let blurred = convolve(g, io, src, &kernel)?;
            first = halve(g, io, &blurred)?;
            tmp_is_in = false;
            i += 1;
        }
        first.outside_val = mri_in.outside_val;
        p.push(first);
        while i < limits.1 {
            let src = if tmp_is_in {
                mri_in
            } else {
                p.last().expect("a level")
            };
            let blurred = convolve(g, io, src, &kernel)?;
            let mut next = halve(g, io, &blurred)?;
            next.outside_val = mri_in.outside_val;
            p.push(next);
            tmp_is_in = false;
            i += 1;
        }
        Ok(p)
    }

    /// `computeIterativeRegistration(nmax, epsit)`: the iteration on the full images from `Minit`
    /// (resampled) and `iscaleinit`.
    pub fn compute_iterative_registration(
        &mut self,
        g: &mut Globals,
        io: &mut Io,
        nmax: i32,
        epsit: f64,
    ) -> Result<(), Fatal> {
        let s = self.mri_source.take().expect("the source");
        let t = self.mri_target.take().expect("the target");
        let m = self.get_minit_resampled();
        let r = self.compute_iterative_registration_full(
            g,
            io,
            nmax,
            epsit,
            &s,
            &t,
            m.as_ref(),
            self.iscaleinit,
        );
        self.mri_source = Some(s);
        self.mri_target = Some(t);
        r
    }

    /// `computeIterativeRegistrationFull(...)`: `iterativeRegistrationHelper<double>` with
    /// `doubleprec`, else `<float>`.
    #[allow(clippy::too_many_arguments)]
    fn compute_iterative_registration_full(
        &mut self,
        g: &mut Globals,
        io: &mut Io,
        nmax: i32,
        epsit: f64,
        mri_s: &Mri,
        mri_t: &Mri,
        m: Option<&VnlMatrix>,
        scaleinit: f64,
    ) -> Result<(), Fatal> {
        if self.doubleprec {
            self.iterative_registration_helper::<f64>(
                g, io, nmax, epsit, mri_s, mri_t, m, scaleinit,
            )
        } else {
            self.iterative_registration_helper::<f32>(
                g, io, nmax, epsit, mri_s, mri_t, m, scaleinit,
            )
        }
    }

    fn step_params(&self) -> StepParams {
        StepParams {
            sat: self.sat,
            iscale: self.iscale,
            transonly: self.transonly,
            rigid: self.rigid,
            isoscale: self.isoscale,
            costfun: self.costfun,
            subsamplesize: self.subsamplesize,
            debug: self.debug,
            verbose: self.verbose,
            iscalefinal: self.iscalefinal,
        }
    }

    /// `iterativeRegistrationHelper<T>(nmax, epsit, mriS, mriT, m, scaleinit)`: see the module's
    /// notes. Leaves `Mfinal`, `iscalefinal`, `converged`, the weights (`mri_hweights` in the
    /// half-way space, `mri_weights` in the target's) and the half-way maps.
    #[allow(clippy::too_many_arguments)]
    fn iterative_registration_helper<T: Real>(
        &mut self,
        g: &mut Globals,
        io: &mut Io,
        nmax: i32,
        epsit: f64,
        mri_s: &Mri,
        mri_t: &Mri,
        m: Option<&VnlMatrix>,
        scaleinit: f64,
    ) -> Result<(), Fatal> {
        self.not_ported_debug(io.err)?;
        let libm = Libm::new(g.mode());
        let mut fmd: (Mat4, f64) = (identity4(), 0.0);
        fmd.0 = match m {
            Some(m) if !m.is_empty() => fixed(m),
            _ => match self.get_minit_resampled() {
                Some(mi) => fixed(&mi),
                None => self.initialize_transform(g, io, mri_s, mri_t)?,
            },
        };
        self.iscalefinal = 1.0;
        if scaleinit != 1.0 {
            self.iscalefinal = scaleinit;
        } else {
            self.iscalefinal = self.iscaleinit;
        }
        fmd.1 = libm.log(self.iscalefinal);
        if self.verbose > 1 {
            let _ = writeln!(io.out, "   - initial iscale: {}", ostream(self.iscalefinal));
            let _ = writeln!(io.out, "   - initial transform:");
            print_m(io.out, &fmd.0, "Minit");
        }
        let mut swarp: Option<Mri> = None;
        let mut twarp: Option<Mri> = None;
        let mut mh2 = identity4();
        let mut mh = identity4();
        let mut mhi = identity4();
        let mut diff = 100.0f64;
        let mut idiff = 0.0f64;
        let ieps = 0.001;
        let mut i = 1;
        let mut rstep = RegistrationStep::<T>::new(self.step_params());
        rstep.set_float_svd(true);
        let tname = if T::SINGLE { "f" } else { "d" };
        self.converged = false;
        while !self.converged && i <= nmax {
            if self.verbose > 0 {
                let _ = writeln!(io.out, " Iteration({tname}): {i}");
            }
            if self.verbose == 1 && self.subsamplesize > 0 {
                let ss = self.subsamplesize;
                let v = &mri_s.vg;
                if v.width > ss && v.height > ss && (v.depth > ss || v.depth == 1) {
                    let _ = write!(io.out, " (subsample {ss}) ");
                    let _ = io.out.flush();
                }
            }
            if self.verbose > 1 {
                let _ = writeln!(io.out);
            }
            self.map_to_new_space(
                g,
                io,
                &fmd.0,
                self.iscalefinal,
                mri_s,
                mri_t,
                &mut swarp,
                &mut twarp,
                &mut mh,
                &mut mhi,
            )?;
            if self.verbose > 1 {
                let _ = writeln!(io.out, "   - compute new registration");
            }
            let sw = swarp.as_mut().expect("the mapped source");
            let tw = twarp.as_mut().expect("the mapped target");
            let cmd =
                rstep.compute_registration_step(g, io.out, io.err, self.trans.as_mut(), sw, tw)?;
            self.wcheck = rstep.wcheck;
            self.wchecksqrt = rstep.wchecksqrt;
            if self.verbose > 1 {
                let _ = writeln!(io.out, "   - recieved matrix update ");
                print_m(io.out, &cmd.0, "Mupdate");
                let _ = writeln!(io.out, "   - store old transform");
                print_m(io.out, &fmd.0, "Mold");
            }
            let fmdtmp = fmd.0;
            if self.symmetry {
                mh2 = inv4(&mhi);
                fmd.0 = mul4(&mul4(&mh2, &cmd.0), &mh);
            } else {
                fmd.0 = mul4(&cmd.0, &fmd.0);
            }
            if self.verbose > 1 {
                let _ = writeln!(io.out, "   - updated full transform");
                print_m(io.out, &fmd.0, "Mnew");
            }
            if self.iscale {
                fmd.1 = sub64(fmd.1, cmd.1);
                self.iscalefinal = libm.exp(fmd.1);
                idiff = cmd.1.abs();
                let istar = if idiff <= ieps {
                    format!(" <= {}  :-)", ostream(ieps))
                } else {
                    String::new()
                };
                if self.verbose > 0 {
                    let _ = writeln!(
                        io.out,
                        "     -- intensity log diff: abs({}) {istar}",
                        ostream(cmd.1)
                    );
                }
            }
            if !self.rigid {
                diff = get_frobenius_diff(&dynm(&fmd.0), &dynm(&fmdtmp));
            } else {
                diff = sqrt64(rigid_trans_dist_sq(
                    g,
                    io.err,
                    &dynm(&fmd.0),
                    Some(&dynm(&fmdtmp)),
                )?);
            }
            if self.verbose > 1 {
                let _ = writeln!(
                    io.out,
                    "     -- old diff. to prev. transform: {}",
                    ostream(diff)
                );
            }
            diff = sqrt64(affine_trans_dist_sq(
                &dynm(&fmd.0),
                Some(&dynm(&fmdtmp)),
                100.0,
            ));
            let star = if diff <= epsit {
                format!("  <= {}   :-)", ostream(epsit))
            } else if i == nmax {
                format!(" max it: {nmax} reached!")
            } else {
                String::new()
            };
            if self.verbose > 0 {
                let _ = writeln!(
                    io.out,
                    "     -- diff. to prev. transform: {}{star}",
                    ostream(diff)
                );
            }
            i += 1;
            self.converged = diff <= epsit && idiff <= ieps;
        }
        note_glibc07(g, &libm, "log, exp");
        self.converged = diff <= epsit;
        self.mri_hweights = None;
        if self.costfun == Cost::Rob {
            let w = rstep.mri_weights.as_ref().expect("the step's weights");
            self.mri_hweights = Some(mri_copy(g, io.out, io.err, w, None)?.unwrap_new());
        }
        self.mri_weights = None;
        if let Some(hw) = &self.mri_hweights {
            let v = &mri_t.vg;
            let mut w = mri_alloc(g, io.out, io.err, v.width, v.height, v.depth, MRI_FLOAT)?;
            mri_copy_header(g, io.out, io.err, mri_t, Some(&mut w))?;
            w.mtype = MRI_FLOAT;
            w.outside_val = 1.0;
            if self.symmetry && !self.iscaleonly {
                mri_linear_transform(g, io.out, io.err, hw, Some(&mut w), &mh2)?;
            } else {
                mri_copy(g, io.out, io.err, hw, Some(&mut w))?;
            }
            self.mri_weights = Some(w);
        }
        if self.verbose > 1 {
            let _ = writeln!(io.out, "     -- adjusting half-way maps ");
        }
        let ch = fixed(&matrix_sqrt(g, io.out, io.err, &dynm(&fmd.0))?);
        let ci = inv4(&fmd.0);
        let chi = mul4(&ch, &ci);
        self.mov2weights = Some(dynm(&ch));
        self.dst2weights = Some(dynm(&chi));
        drop(twarp);
        drop(swarp);
        self.mfinal = Some(dynm(&fmd.0));
        Ok(())
    }

    /// `mapToNewSpace(M, iscaleval, mriS, mriT, Swarp, Twarp, mh, mhi)`: both images to the half
    /// way space (`symmetry`: the source by `sqrt(M)`, the target by `sqrt(M) M^-1`, both on the
    /// source's grid, intensities by `sqrt(iscaleval)` and its inverse), or the source to the
    /// target (without `symmetry`), or only the intensities (`iscaleonly`).
    #[allow(clippy::too_many_arguments)]
    fn map_to_new_space(
        &self,
        g: &mut Globals,
        io: &mut Io,
        m: &Mat4,
        iscaleval: f64,
        mri_s: &Mri,
        mri_t: &Mri,
        swarp: &mut Option<Mri>,
        twarp: &mut Option<Mri>,
        mh: &mut Mat4,
        mhi: &mut Mat4,
    ) -> Result<(), Fatal> {
        if self.verbose > 1 {
            let _ = writeln!(io.out, " Registration::mapToNewSpace ");
        }
        if self.iscaleonly {
            *mh = identity4();
            *mhi = identity4();
            if self.verbose > 1 {
                let _ = writeln!(
                    io.out,
                    "   - adjusting only intensity ( {} ) ",
                    ostream(iscaleval)
                );
            }
            let si = sqrt64(iscaleval);
            valscale_into(g, io, mri_s, swarp, si)?;
            valscale_into(g, io, mri_t, twarp, div64(1.0, si))?;
            swarp.as_mut().expect("Swarp").outside_val = mri_s.outside_val;
            twarp.as_mut().expect("Twarp").outside_val = mri_t.outside_val;
            return Ok(());
        }
        if self.symmetry {
            if self.verbose > 1 {
                let _ = writeln!(io.out, "   - resampling MOV and DST (sqrt)");
            }
            *mh = fixed(&matrix_sqrt(g, io.out, io.err, &dynm(m))?);
            self.check_sqrt_m(g, io, mh, self.rigid && self.minit.is_none())?;
            let mi = inv4(m);
            *mhi = mul4(mh, &mi);
            let mut sw = mri_clone(g, io.out, io.err, mri_s, None)?.unwrap_new();
            mri_linear_transform(g, io.out, io.err, mri_s, Some(&mut sw), mh)?;
            let mut tw = mri_clone(g, io.out, io.err, mri_s, None)?.unwrap_new();
            mri_linear_transform(g, io.out, io.err, mri_t, Some(&mut tw), mhi)?;
            if self.iscale {
                if self.verbose > 1 {
                    let _ = writeln!(
                        io.out,
                        "   - adjusting intensity ( {} ) ",
                        ostream(iscaleval)
                    );
                }
                let si = sqrt64(iscaleval);
                mri_valscale(g, io.out, io.err, Pair::Same(&mut sw), si, 0.0)?;
                mri_valscale(g, io.out, io.err, Pair::Same(&mut tw), div64(1.0, si), 0.0)?;
            }
            sw.outside_val = mri_s.outside_val;
            tw.outside_val = mri_t.outside_val;
            *swarp = Some(sw);
            *twarp = Some(tw);
            return Ok(());
        }
        if self.verbose > 1 {
            let _ = writeln!(io.out, "   - resampling MOV to DST ");
        }
        let mut sw = mri_clone(g, io.out, io.err, mri_t, None)?.unwrap_new();
        *mh = *m;
        mri_linear_transform(g, io.out, io.err, mri_s, Some(&mut sw), mh)?;
        *mhi = identity4();
        if twarp.is_none() {
            *twarp = Some(mri_copy(g, io.out, io.err, mri_t, None)?.unwrap_new());
        }
        if self.iscale {
            if self.verbose > 1 {
                let _ = writeln!(
                    io.out,
                    "   - adjusting intensity ( {} ) ",
                    ostream(iscaleval)
                );
            }
            mri_valscale(g, io.out, io.err, Pair::Same(&mut sw), iscaleval, 0.0)?;
        }
        sw.outside_val = mri_s.outside_val;
        twarp.as_mut().expect("Twarp").outside_val = mri_t.outside_val;
        *swarp = Some(sw);
        Ok(())
    }

    /// `checkSqrtM(M, testrigid)`: the square root's upper 3 x 3 block by its polar decomposition:
    /// rigid, no reflection, no scaling (`|S - I|_F` at most 1e-6) and a rotation (`|A - R|_F` at
    /// most 1e-6); affine, no reflection and no scale below 0.001. Each failure an error
    /// (`ERROR_OUT_OF_BOUNDS`).
    fn check_sqrt_m(
        &self,
        g: &mut Globals,
        io: &mut Io,
        m: &Mat4,
        testrigid: bool,
    ) -> Result<bool, Fatal> {
        let mut a = VnlMatrix::zeros(3, 3);
        for r in 0..3 {
            for c in 0..3 {
                *a.at_mut(r, c) = m[r][c];
            }
        }
        if testrigid {
            let (r, s) = polar_decomposition(g, io.err, &a)?;
            if s.at(0, 0) < 0.0 || s.at(1, 1) < 0.0 || s.at(2, 2) < 0.0 {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_OUT_OF_BOUNDS,
                    "Internal Error: Matrix Sqrt produced reflection.\n",
                )?;
            }
            let eps = 0.000001;
            let fnorm1 = s.sub(&VnlMatrix::identity(3)).fro_norm();
            if fnorm1 > eps {
                let _ = writeln!(io.err, "Internal Error: ");
                let _ = writeln!(
                    io.err,
                    " Sqrt of Rotation should not scale ( fnorm(S-I) = {} )",
                    ostream(fnorm1)
                );
                let _ = writeln!(io.err, " Debug Info: ");
                let _ = writeln!(io.err, "{}", matlab_print_long(&a, "A"));
                let _ = writeln!(io.err, "{}", matlab_print_long(&r, "R"));
                let _ = writeln!(io.err, "{}", matlab_print_long(&s, "S"));
                log::error_exit(
                    g,
                    io.err,
                    ERROR_OUT_OF_BOUNDS,
                    "Internal Error: Sqrt of Rotation should not scale.\n",
                )?;
            }
            let fnorm2 = a.sub(&r).fro_norm();
            if fnorm2 > eps {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_OUT_OF_BOUNDS,
                    "Internal Error: Sqrt should be a rotation.\n",
                )?;
            }
        } else {
            let (_, _, d) = polar2_decomposition(g, io.err, &a)?;
            if d[0] < 0.0 || d[1] < 0.0 || d[2] < 0.0 {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_OUT_OF_BOUNDS,
                    "Internal Error: Matrix Sqrt produced reflection.\n",
                )?;
            }
            let eps = 0.001;
            if d[0] < eps || d[1] < eps || d[2] < eps {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_OUT_OF_BOUNDS,
                    "Internal Error: Matrix Sqrt produced near projection.\n",
                )?;
            }
        }
        Ok(true)
    }

    /// The pyramids of both images (`getGPLimits`, `buildGPLimits` where none is built yet) and
    /// their size checks (`Input images must be larger than 16^3.`, `ERROR_BADFILE`).
    fn pyramids(&mut self, g: &mut Globals, io: &mut Io) -> Result<(i32, i32), Fatal> {
        let mut mins = 16;
        if self.minsize > mins {
            mins = self.minsize;
        }
        let s = self.mri_source.take().expect("the source");
        let t = self.mri_target.take().expect("the target");
        let r = (|| {
            let limits = self.get_gp_limits(io.out, &s, &t, mins, self.maxsize)?;
            if self.gp_s.is_empty() {
                self.gp_s = self.build_gp_limits(g, io, &s, limits)?;
            }
            if self.gp_t.is_empty() {
                self.gp_t = self.build_gp_limits(g, io, &t, limits)?;
            }
            Ok(limits)
        })();
        self.mri_source = Some(s);
        self.mri_target = Some(t);
        r
    }

    fn too_small(v: &Mri, mins: i32) -> bool {
        v.vg.width < mins || v.vg.height < mins || (v.vg.depth < mins && v.vg.depth != 1)
    }

    /// `computeMultiresRegistration(stopres, n, epsit)`: see the module's notes. `highit` 0
    /// skips the finest level, a positive one sets its iterations.
    pub fn compute_multires_registration(
        &mut self,
        g: &mut Globals,
        io: &mut Io,
        stopres: i32,
        n: i32,
        epsit: f64,
    ) -> Result<(), Fatal> {
        if self.verbose > 0 {
            let _ = writeln!(io.out);
            let _ = writeln!(io.out);
            let _ = writeln!(io.out, " Registration::computeMultiresRegistration ");
        }
        let mut md: (Mat4, f64) = (identity4(), self.iscaleinit);
        md.0 = match self.get_minit_resampled() {
            Some(m) => fixed(&m),
            None => {
                let s = self.mri_source.take().expect("the source");
                let t = self.mri_target.take().expect("the target");
                let r = self.initialize_transform(g, io, &s, &t);
                self.mri_source = Some(s);
                self.mri_target = Some(t);
                r?
            }
        };
        self.not_ported_debug(io.err)?;
        let mut mins = 16;
        if self.minsize > mins {
            mins = self.minsize;
        }
        let limits = self.pyramids(g, io)?;
        if Self::too_small(&self.gp_t[0], mins) {
            log::error_exit(
                g,
                io.err,
                ERROR_BADFILE,
                "Input images must be larger than 16^3.\n",
            )?;
        }
        if Self::too_small(&self.gp_s[0], 16) {
            log::error_exit(
                g,
                io.err,
                ERROR_BADFILE,
                "Input images must be larger than 16^3.\n",
            )?;
        }
        let resolution = self.gp_s.len() as i32;
        if self.verbose > 0 {
            let _ = writeln!(io.out, "   - initial transform:");
            print_m(io.out, &md.0, "Ti");
            let _ = writeln!(io.out, "   - initial iscale:  Ii ={}", ostream(md.1));
        }
        for _ in 1..=limits.1 {
            for rr in 0..3 {
                md.0[rr][3] = mul64(0.5, md.0[rr][3]);
            }
        }
        if self.verbose > 1 {
            let _ = writeln!(io.out, "   - initial adjusted:");
            print_m(io.out, &md.0, "Tia");
        }
        let iscaletmp = self.iscale;
        if resolution - 1 < stopres {
            log::error_exit(
                g,
                io.err,
                ERROR_BADFILE,
                "Input images have insufficient resoltuion.\n",
            )?;
        }
        let mut m = n;
        self.converged = true;
        let gp_s = std::mem::take(&mut self.gp_s);
        let gp_t = std::mem::take(&mut self.gp_t);
        let res = (|| -> Result<(), Fatal> {
            let mut r = resolution - 1;
            while r >= stopres {
                let (s, t) = (&gp_s[r as usize], &gp_t[r as usize]);
                if self.verbose > 0 {
                    let _ = writeln!(io.out);
                    let _ = writeln!(
                        io.out,
                        "Resolution: {r}  S( {} {} {} )  T( {} {} {} )",
                        s.vg.width, s.vg.height, s.vg.depth, t.vg.width, t.vg.height, t.vg.depth
                    );
                }
                if r <= 1 {
                    self.iscale = iscaletmp;
                }
                if self.verbose > 1 {
                    let _ = writeln!(io.out, "- compute new iterative registration");
                }
                m = n;
                if r == 0 && self.highit == 0 {
                    let _ = writeln!(io.out, "  skipping (highit == 0) ...");
                    break;
                }
                if r == 0 && self.highit > 0 {
                    m = self.highit;
                }
                let mi = dynm(&md.0);
                self.compute_iterative_registration_full(g, io, m, epsit, s, t, Some(&mi), md.1)?;
                let mut cmd: (Mat4, f64) = (
                    fixed(self.mfinal.as_ref().expect("Mfinal")),
                    self.iscalefinal,
                );
                if self.verbose > 1 {
                    let _ = writeln!(io.out);
                    let _ = writeln!(io.out, " current : Matrix: ");
                    print_m(io.out, &cmd.0, "Tc");
                    let _ = writeln!(io.out, " intens:  Ic = {}", ostream(cmd.1));
                    let (mut tx, mut ty, mut tz) = (cmd.0[0][3], cmd.0[1][3], cmd.0[2][3]);
                    for _ in 0..r {
                        tx = mul64(tx, 2.0);
                        ty = mul64(ty, 2.0);
                        tz = mul64(tz, 2.0);
                    }
                    let _ = writeln!(
                        io.out,
                        " equiv trans on highres: {} {} {}",
                        ostream(tx),
                        ostream(ty),
                        ostream(tz)
                    );
                }
                if r != 0 {
                    for rr in 0..3 {
                        cmd.0[rr][3] = mul64(2.0, cmd.0[rr][3]);
                    }
                }
                md = cmd;
                if self.verbose > 1 {
                    let _ = writeln!(io.out);
                    let _ = writeln!(io.out, " Matrix: ");
                    print_m(io.out, &md.0, "Tcpost");
                    let _ = writeln!(io.out, " intens: Icpost = {}", ostream(md.1));
                }
                r -= 1;
            }
            Ok(())
        })();
        self.gp_s = gp_s;
        self.gp_t = gp_t;
        res?;
        let mut stopres = stopres;
        if stopres > 0 {
            stopres -= 1;
        }
        for _ in 0..stopres + limits.0 {
            for rr in 0..3 {
                md.0[rr][3] = mul64(2.0, md.0[rr][3]);
            }
        }
        if self.verbose == 1 {
            let _ = writeln!(io.out);
            let _ = writeln!(io.out, "   - final transform: ");
            print_m(io.out, &md.0, "Tf");
            let _ = writeln!(io.out, "   - final iscale:  If = {}", ostream(md.1));
        }
        if !self.converged && self.verbose > 0 {
            let star = "**********************************************************";
            let _ = writeln!(io.out);
            let _ = writeln!(io.out, "{star}");
            let _ = writeln!(io.out, "*");
            let _ = writeln!(
                io.out,
                "* WARNING: Registration did not converge in {m} steps! "
            );
            let _ = writeln!(io.out, "*          Problem might be ill posed. ");
            let _ = writeln!(io.out, "*          Please inspect output manually!");
            let _ = writeln!(io.out, "*");
            let _ = writeln!(io.out, "{star}");
        }
        self.mfinal = Some(dynm(&md.0));
        self.iscalefinal = md.1;
        Ok(())
    }

    /// `findSatMultiRes(mi, scaleinit)`: from the coarsest level up to the first level with a side
    /// of at least 64 voxels (`stopres`), three iterations each (one on `stopres`) with `epsit`
    /// 0.05, verbose 1 silenced; leaves the last step's weight check.
    fn find_sat_multi_res(
        &mut self,
        g: &mut Globals,
        io: &mut Io,
        mi: &Mat4,
        scaleinit: f64,
    ) -> Result<(), Fatal> {
        let mut md: (Mat4, f64) = (*mi, scaleinit);
        if Self::too_small(&self.gp_s[0], 16) {
            log::error_exit(
                g,
                io.err,
                ERROR_BADFILE,
                "Input images must be larger than 16^3.\n",
            )?;
        }
        let resolution = self.gp_s.len() as i32;
        let rstart = 1;
        let stopres = sat_stopres(&self.gp_s, resolution, rstart);
        let gp_s = std::mem::take(&mut self.gp_s);
        let gp_t = std::mem::take(&mut self.gp_t);
        let res = (|| -> Result<(), Fatal> {
            let mut r = resolution - rstart;
            while r >= stopres {
                let (s, t) = (&gp_s[r as usize], &gp_t[r as usize]);
                if self.verbose > 1 {
                    let _ = writeln!(io.out);
                    let _ = writeln!(io.out, "Resolution: {r}");
                    let _ = writeln!(
                        io.out,
                        " gpS ( {} , {} , {} )",
                        s.vg.width, s.vg.height, s.vg.depth
                    );
                    let _ = writeln!(
                        io.out,
                        " gpT ( {} , {} , {} )",
                        t.vg.width, t.vg.height, t.vg.depth
                    );
                }
                if self.verbose > 2 {
                    let _ = writeln!(io.out, "   - compute new iterative registration");
                }
                let n = if r == stopres { 1 } else { 3 };
                let vv = self.verbose;
                if self.verbose == 1 {
                    self.verbose = 0;
                }
                let mi = dynm(&md.0);
                let rr =
                    self.compute_iterative_registration_full(g, io, n, 0.05, s, t, Some(&mi), md.1);
                self.verbose = vv;
                rr?;
                let mut cmd: (Mat4, f64) = (
                    fixed(self.mfinal.as_ref().expect("Mfinal")),
                    self.iscalefinal,
                );
                if self.verbose > 1 {
                    let _ = writeln!(io.out);
                    let _ = writeln!(io.out, " current Matrix: ");
                    print_m(io.out, &cmd.0, "Tc");
                    let _ = writeln!(io.out, " intens: Ic = {}", ostream(cmd.1));
                    let (mut tx, mut ty, mut tz) = (cmd.0[0][3], cmd.0[1][3], cmd.0[2][3]);
                    for _ in 0..r {
                        tx = mul64(tx, 2.0);
                        ty = mul64(ty, 2.0);
                        tz = mul64(tz, 2.0);
                    }
                    let _ = writeln!(
                        io.out,
                        " equiv trans on highres: {} {} {}",
                        ostream(tx),
                        ostream(ty),
                        ostream(tz)
                    );
                }
                if r != 0 {
                    for rr in 0..3 {
                        cmd.0[rr][3] = mul64(2.0, cmd.0[rr][3]);
                    }
                }
                if r == stopres {
                    while r > 1 {
                        for rr in 0..3 {
                            cmd.0[rr][3] = mul64(4.0, cmd.0[rr][3]);
                        }
                        r -= 1;
                    }
                }
                md = cmd;
                if self.verbose > 1 {
                    let _ = writeln!(io.out);
                    let _ = writeln!(io.out, " Matrix: ");
                    print_m(io.out, &md.0, "T");
                    let _ = writeln!(io.out, " Intensity:  I = {}", ostream(md.1));
                }
                r -= 1;
            }
            Ok(())
        })();
        self.gp_s = gp_s;
        self.gp_t = gp_t;
        res
    }

    /// `findSaturation()`: the saturation whose weight check is at most `wlimit`, by bisection
    /// from 16 (halving below, doubling above) to within 0.5; a saturation below 6 whose check is
    /// below 0.04 ends the search. Returns and keeps it.
    pub fn find_saturation(&mut self, g: &mut Globals, io: &mut Io) -> Result<f64, Fatal> {
        if self.verbose > 0 {
            let _ = writeln!(io.out);
            let _ = writeln!(io.out);
            let _ = writeln!(io.out, " Registration::findSaturation ");
        }
        let mut md: (Mat4, f64) = (identity4(), self.iscaleinit);
        md.0 = match self.get_minit_resampled() {
            Some(m) => fixed(&m),
            None => {
                let s = self.mri_source.take().expect("the source");
                let t = self.mri_target.take().expect("the target");
                let r = self.initialize_transform(g, io, &s, &t);
                self.mri_source = Some(s);
                self.mri_target = Some(t);
                r?
            }
        };
        let mut mins = 16;
        if self.minsize > mins {
            mins = self.minsize;
        }
        self.pyramids(g, io)?;
        if Self::too_small(&self.gp_s[0], mins) {
            log::error_exit(
                g,
                io.err,
                ERROR_BADFILE,
                "Input images must be larger than 16^3.\n",
            )?;
        }
        if Self::too_small(&self.gp_t[0], mins) {
            log::error_exit(
                g,
                io.err,
                ERROR_BADFILE,
                "Input images must be larger than 16^3.\n",
            )?;
        }
        let resolution = self.gp_s.len() as i32;
        let rstart = 1;
        let stopres = sat_stopres(&self.gp_s, resolution, rstart);
        let (s, t) = (&self.gp_s[stopres as usize], &self.gp_t[stopres as usize]);
        if s.vg.width < 32 || s.vg.height < 32 || s.vg.depth < 32 {
            let bar = " ========================================================================";
            let _ = writeln!(io.out);
            let _ = writeln!(io.out, "{bar}");
            let _ = writeln!(
                io.out,
                " WARNING: image might be too small (or ill shaped) for --satit to work."
            );
            let _ = writeln!(
                io.out,
                "          Try to manually specify --sat # if not satisfied with result! "
            );
            let _ = writeln!(io.out, "{bar}");
            let _ = writeln!(io.out);
        }
        let _ = writeln!(io.out);
        let _ = writeln!(io.out, "   - Max Resolution used: {stopres}");
        let _ = writeln!(
            io.out,
            "     -- gpS ( {} , {} , {} )",
            s.vg.width, s.vg.height, s.vg.depth
        );
        let _ = writeln!(
            io.out,
            "     -- gpT ( {} , {} , {} )",
            t.vg.width, t.vg.height, t.vg.depth
        );
        if self.verbose > 1 {
            let _ = writeln!(io.out, "   - initial transform:");
            print_m(io.out, &md.0, "Ti");
            let _ = writeln!(io.out, "   - initial iscale:   Ii = {}", ostream(md.1));
        }
        for _ in 1..=resolution - rstart {
            for rr in 0..3 {
                md.0[rr][3] = mul64(0.5, md.0[rr][3]);
            }
        }
        if self.verbose > 1 {
            let _ = writeln!(io.out, "   - initial adjusted:");
            print_m(io.out, &md.0, "Tia");
        }
        let _ = writeln!(
            io.out,
            "   - running loop to estimate saturation parameter:"
        );
        let satdiff = 0.5;
        let (mut satmax, mut satmin) = (0.0f64, 0.0f64);
        let (mut wmin, mut wmax) = (-1.0f64, -1.0f64);
        let mut counter = 0;
        while satmin == 0.0 || satmax == 0.0 || sub64(satmax, satmin) > satdiff {
            counter += 1;
            self.sat = if satmin == 0.0 && satmax == 0.0 {
                16.0
            } else if satmin == 0.0 {
                mul64(0.5, satmax)
            } else if satmax == 0.0 {
                mul64(2.0, satmin)
            } else {
                mul64(0.5, add64(satmax, satmin))
            };
            if self.verbose > 0 {
                if counter > 1 {
                    let _ = writeln!(
                        io.out,
                        "         min sat: {} ( {} ), max sat: {} ( {} ), sat diff: {}, (wlimit={})",
                        ostream(satmin),
                        ostream(wmin),
                        ostream(satmax),
                        ostream(wmax),
                        ostream(sub64(satmax, satmin)),
                        ostream(self.wlimit)
                    );
                }
                let _ = writeln!(
                    io.out,
                    "     -- Iteration: {counter}  trying sat: {}",
                    ostream(self.sat)
                );
            }
            self.find_sat_multi_res(g, io, &md.0, md.1)?;
            if self.wcheck > self.wlimit {
                satmin = self.sat;
                wmin = self.wcheck;
            } else {
                satmax = self.sat;
                wmax = self.wcheck;
            }
            if self.sat < 6.0 && self.wcheck < 0.04 {
                satmax = self.sat;
                satmin = self.sat;
                wmax = self.wcheck;
                wmin = self.wcheck;
                break;
            }
        }
        if wmax <= self.wlimit {
            self.sat = satmax;
            self.wcheck = wmax;
        } else {
            self.sat = satmin;
            self.wcheck = wmin;
        }
        if self.verbose > 0 {
            let _ = writeln!(
                io.out,
                "   - final SAT: {} ( it: {counter} , weight check {} <= {} )",
                ostream(self.sat),
                ostream(self.wcheck),
                ostream(self.wlimit)
            );
        }
        self.not_ported_debug(io.err)?;
        Ok(self.sat)
    }
}

/// `findSaturation`'s and `findSatMultiRes`'s `stopres`: from the coarsest level down, the first
/// with a side of at least 64 voxels (0 at the finest).
fn sat_stopres(gp_s: &[Mri], resolution: i32, rstart: i32) -> i32 {
    let mut stopres = resolution - rstart;
    while stopres > 0 {
        let v = &gp_s[stopres as usize].vg;
        if v.width >= 64 || v.height >= 64 || v.depth >= 64 {
            break;
        }
        stopres -= 1;
    }
    stopres
}

/// `R (c, 1)` of a centroid (`vnl_matrix * vnl_vector`: sums from 0).
fn centroid_back(r: &VnlMatrix, c: &[f64]) -> [f64; 4] {
    let v = [c[0], c[1], c[2], 1.0];
    let mut o = [0.0; 4];
    for (i, oi) in o.iter_mut().enumerate() {
        let mut acc = 0.0f64;
        for (k, &vk) in v.iter().enumerate() {
            acc = add64(acc, mul64(r.at(i, k), vk));
        }
        *oi = acc;
    }
    o
}

/// `MRIconvolveGaussian(src, NULL, kernel)`.
fn convolve(g: &mut Globals, io: &mut Io, src: &Mri, kernel: &Mri) -> Result<Mri, Fatal> {
    match mri_convolve_gaussian(g, io.out, io.err, Pair::New(src), kernel)? {
        Ret::New(m) => Ok(m),
        _ => Err(crash(io.err, "MRIconvolveGaussian")),
    }
}

/// `MRIdownsample2BSpline(src, NULL)`.
fn halve(g: &mut Globals, io: &mut Io, src: &Mri) -> Result<Mri, Fatal> {
    match mri_downsample2_bspline(g, io.out, io.err, src, None)? {
        Ret::New(m) => Ok(m),
        _ => Err(crash(io.err, "MRIdownsample2BSpline")),
    }
}

/// `dst = MyMRI::MRIvalscale(src, dst, s)` (a clone of the source when there is none yet).
fn valscale_into(
    g: &mut Globals,
    io: &mut Io,
    src: &Mri,
    dst: &mut Option<Mri>,
    s: f64,
) -> Result<(), Fatal> {
    match dst.as_mut() {
        Some(d) => {
            mri_valscale(g, io.out, io.err, Pair::Given(src, d), s, 0.0)?;
        }
        None => match mri_valscale(g, io.out, io.err, Pair::New(src), s, 0.0)? {
            Ret::New(m) => *dst = Some(m),
            _ => return Err(crash(io.err, "MRIvalscale")),
        },
    }
    Ok(())
}

/// A null image or matrix where FreeSurfer dereferences it: an error (FreeSurfer crashes).
fn crash(err: &mut dyn Write, what: &str) -> Fatal {
    let _ = writeln!(
        err,
        "Registration: {what} returned nothing (FreeSurfer crashes here)"
    );
    Fatal::Exit(1)
}
