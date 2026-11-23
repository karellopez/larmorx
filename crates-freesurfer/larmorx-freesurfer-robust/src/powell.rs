// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `RegPowell` (queue 3.2-13): `mri_robust_register`'s registration for the cost functions other
//! than the robust one, translated from FreeSurfer 8.2.0's `mri_robust_register/RegPowell.cpp`
//! (`costFunction`, `computeIterativeRegistrationFull`, `setTransformation`; the last is
//! [`RegRobust::set_transformation`] with `powell` set).
//!
//! Each level (or the whole images with `--nomulti`): both images as bytes (`MRISeqchangeType(...,
//! MRI_UCHAR, 0, 0.999, FALSE)`, printed as `changing data type from <t> to 0 (noscale = 0)...`,
//! or a copy of a byte image) stretched to 0..255 (`MyMRI::MRInorm255`: `(v - min) * (255 /
//! (max - min))`, the factor in `float`), their outside values kept; the initial transform (the one
//! passed, else `Minit` resampled, else `initializeTransform` of the byte images); with symmetry
//! its square root `mh1` and `mh2 = (mh1 * M^-1)^-1`, so that `M = mh2 * mh1`; Powell's method
//! (`fs_powell`, the model's steps as the initial directions, `0.02` for the intensity's log)
//! over the model's parameters from the identity (and the log of the initial intensity scale),
//! each evaluation the cost of `mh2 * T(p) * mh1` with its half-way maps (`msi = (mh2 T mh1)^-1
//! sqrt(mh2 T mh1)`, `mti = sqrt(mh2 T mh1)`; without symmetry `msi = (T mh1)^-1`, `mti = I`),
//! every second voxel once all three target extents pass the subsampling size (150 unless set).
//!
//! The costs here: least squares (`LS`) and absolute differences (`SAD`) with the intensity scale
//! split between the images (`sqrt(exp(s))`), Tukey's biweight (`TB`), the normalised correlation
//! (`NCC`) and its local form (`LNCC`, blocks of 5^3 every fifth voxel): [`crate::costs`]; the
//! joint-histogram costs (`MI`, `NMI`, `ECC`, `SCR`: [`crate::jointhisto`], the histogram of every
//! evaluation smoothed by 7 bins, its sign turned so that Powell minimises) and the SB cost
//! ([`crate::costs::sb_cost`], on the shared emulation of libstdc++'s `std::sort`).

use std::cell::RefCell;
use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::math::Libm;
use larmorx_core::vnl_matrix::{VnlMatrix, matlab_print_long_vector};
use larmorx_core::vnl_minimize::ReturnCode;
use larmorx_freesurfer_core::cstd::note_glibc07;
use larmorx_freesurfer_core::fs_vnl::{FsCostFunction, FsPowell, NoFsFunctions};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::sse::{div64, sqrt64};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri};
use larmorx_freesurfer_volume::arith::Pair;
use larmorx_freesurfer_volume::resize::mri_seq_change_type;

use crate::costs::{abs_diff, least_squares, local_ncc, minmax, ncc, sb_cost, tukey_biweight};
use crate::jointhisto::JointHisto;
use crate::mymatrix::{Mat4, matrix_sqrt};
use crate::mymri::{mri_valscale, wide};
use crate::registration::{Io, RegRobust, dynm, fixed, identity4, inv4, mul4};
use crate::regstep::Cost;
use crate::transformation::Transformation;

/// A stream that appends to a shared buffer: Powell's lines and the cost function's, in order.
struct Shared<'a>(&'a RefCell<Vec<u8>>);

impl Write for Shared<'_> {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// `vnl_matrix_fixed::print` (`std::cout << M`): each row's numbers separated by a blank, at the
/// stream's precision, one row per line.
fn print_fixed(out: &mut dyn Write, m: &Mat4, prec: usize) {
    for row in m {
        let line: Vec<String> = row.iter().map(|&v| cfmt::g(v, prec)).collect();
        let _ = writeln!(out, "{}", line.join(" "));
    }
}

/// `MyMRI::MRInorm255(mri, mri)` of a byte image: `MRIvalscale` by `255 / (max - min)` (a
/// `float` division) from `min`.
fn norm255(g: &mut Globals, io: &mut Io, mri: &mut Mri) -> Result<(), Fatal> {
    let (lo, hi) = minmax(mri);
    let s = 255.0f32 / (hi - lo);
    mri_valscale(g, io.out, io.err, Pair::Same(mri), wide(s), wide(lo))?;
    Ok(())
}

/// The image as `RegPowell` uses it: bytes stretched to 0..255, the outside value kept.
fn as_bytes(g: &mut Globals, io: &mut Io, mri: &mut Mri) -> Result<Mri, Fatal> {
    let outside = mri.outside_val;
    let mut b = if mri.mtype != MRI_UCHAR {
        let _ = writeln!(
            io.out,
            "changing data type from {} to {MRI_UCHAR} (noscale = 0)...",
            mri.mtype
        );
        match mri_seq_change_type(g, io.out, io.err, io.env, mri, MRI_UCHAR, 0.0, 0.999, 0)? {
            Some(m) => m,
            None => {
                let _ = writeln!(io.out, "ERROR: MRISeqchangeType");
                return Err(Fatal::Exit(1));
            }
        }
    } else {
        mri_copy(g, io.out, io.err, mri, None)?.unwrap_new()
    };
    norm255(g, io, &mut b)?;
    b.outside_val = outside;
    Ok(b)
}

/// `RegPowell::costFunction`'s state (its statics).
struct Ctx<'a> {
    libm: &'a Libm,
    trans: &'a mut Transformation,
    iscale: bool,
    symmetry: bool,
    cost: Cost,
    subsamp: i32,
    mh1: Mat4,
    mh2: Mat4,
    scf: &'a mut Mri,
    tcf: &'a mut Mri,
    /// The joint histogram (`costFunction`'s static `H`).
    histo: JointHisto,
    /// The standard output's precision (the histogram's error lines).
    prec: usize,
    debug: bool,
}

impl Ctx<'_> {
    /// `RegPowell::costFunction(p)`.
    fn cost(&mut self, g: &mut Globals, io: &mut Io, p: &[f64]) -> Result<f64, Fatal> {
        let mut second = 0.0;
        if self.iscale {
            second = p[p.len() - 1];
        }
        self.trans.set_parameters(g, p, false);
        let m = self.trans.matrix(g, io.err)?;
        let full = mul4(&mul4(&self.mh2, &m), &self.mh1);
        let (msi, mti) = if self.symmetry {
            let mti = fixed(&matrix_sqrt(g, io.out, io.err, &dynm(&full))?);
            (mul4(&inv4(&full), &mti), mti)
        } else {
            (inv4(&full), identity4())
        };
        let (mut si, mut sii) = (1.0, 1.0);
        if self.iscale {
            let fullscale = self.libm.exp(second);
            si = sqrt64(fullscale);
            sii = div64(1.0, si);
        }
        let d = [self.subsamp; 3];
        let (s, t) = (&mut *self.scf, &mut *self.tcf);
        match self.cost {
            Cost::Ls => return Ok(least_squares(g, io.err, s, t, &msi, &mti, d, si, sii)),
            Cost::Tb => return Ok(tukey_biweight(g, io.err, s, t, &msi, &mti, d, 4.6851)),
            Cost::Lncc => return Ok(-local_ncc(g, io.out, io.err, s, t, &msi, &mti, [5, 5, 5])?),
            Cost::Ncc => return Ok(-ncc(g, io.out, io.err, s, t, &msi, &mti, d)?),
            Cost::Sad => return Ok(abs_diff(g, io.err, s, t, &msi, &mti, d, si, sii)),
            Cost::Sb => return sb_cost(g, io.out, io.err, s, t, &msi, &mti, d),
            _ => {}
        }
        // the costs of a joint histogram
        self.histo
            .create(g, io.out, io.err, s, t, &msi, &mti, d, self.prec)?;
        self.histo.smooth(self.libm, 7.0);
        let dd = match self.cost {
            Cost::Mi => -self.histo.compute_mi(self.libm),
            Cost::Nmi => -self.histo.compute_nmi(self.libm),
            Cost::Ecc => -self.histo.compute_ecc(self.libm),
            Cost::Scr => -self.histo.compute_scr(),
            other => {
                let _ = writeln!(
                    io.out,
                    " RegPowell::costFunction ERROR cannot deal with cost function {} ! ",
                    other as i32
                );
                return Err(Fatal::Exit(1));
            }
        };
        if self.debug {
            // the evaluation's report and the first evaluation's half-way images
            let _ = writeln!(
                io.err,
                "RegPowell: the debug output (--debug) is not ported yet"
            );
            return Err(Fatal::Exit(1));
        }
        Ok(dd)
    }
}

impl RegRobust {
    /// `RegPowell::computeIterativeRegistrationFull(nmax, epsit, mriS, mriT, m, scaleinit)` (`nmax`
    /// and `epsit` unused): see the module's notes. Leaves `Mfinal`, `iscalefinal`, the
    /// half-way maps (`mov2weights`, `dst2weights`) and the half-way geometry
    /// ([`RegRobust::powell_hw`]: the byte target's header, FS-1075).
    pub(crate) fn powell_full(
        &mut self,
        g: &mut Globals,
        io: &mut Io,
        mri_s: &mut Mri,
        mri_t: &mut Mri,
        m: Option<&VnlMatrix>,
        scaleinit: f64,
    ) -> Result<(), Fatal> {
        let _ = writeln!(io.out, "RegPowell::computeIterativeRegistrationFull");
        let _ = writeln!(io.out, " sym: {}", i32::from(self.symmetry));
        if (mri_s.vg.depth == 1 || mri_t.vg.depth == 1) && mri_t.vg.depth != mri_s.vg.depth {
            let _ = writeln!(io.out, "ERROR: both source and target need to be 2D or 3D");
            return Err(Fatal::Exit(1));
        }
        let mut scf = as_bytes(g, io, mri_s)?;
        let mut tcf = as_bytes(g, io, mri_t)?;

        let first: Mat4 = if let Some(m) = m.filter(|m| !m.is_empty()) {
            if self.verbose > 1 {
                let _ = writeln!(io.out, "     initial M passed ... ");
            }
            fixed(m)
        } else if let Some(mr) = self.get_minit_resampled() {
            if self.verbose > 1 {
                let _ = writeln!(io.out, "     initial M from resample ... ");
            }
            fixed(&mr)
        } else {
            let t = self.initialize_transform(g, io, &scf, &tcf)?;
            if self.verbose > 1 {
                let _ = writeln!(io.out, "     initialize transform M ... ");
            }
            t
        };
        let initial_m = first;
        self.iscalefinal = if scaleinit != 1.0 {
            scaleinit
        } else {
            self.iscaleinit
        };
        let libm = Libm::new(g.mode());
        let second = libm.log(self.iscalefinal);
        if self.verbose > 1 {
            let _ = writeln!(io.out, "   - initial iscale: {}", self.os(self.iscalefinal));
            let _ = writeln!(io.out, "   - initial transform:");
            print_fixed(io.out, &first, self.cout_precision);
            let _ = writeln!(io.out);
        }
        let (mh1, mh2) = if self.symmetry {
            let mh1 = fixed(&matrix_sqrt(g, io.out, io.err, &dynm(&first))?);
            let mhi = mul4(&mh1, &inv4(&first));
            (mh1, inv4(&mhi))
        } else {
            (first, identity4())
        };

        let mut trans = self.trans.take().expect("a transformation model");
        let mut pcount = trans.dof();
        if self.iscale {
            pcount += 1;
        }
        let mut subsamp = 1;
        if self.subsamplesize <= 0 {
            self.subsamplesize = 150;
        }
        let sz = self.subsamplesize;
        if sz > 0 && tcf.vg.width > sz && tcf.vg.height > sz && tcf.vg.depth > sz {
            let _ = writeln!(io.out, "   - subsampling this resolution ... ");
            subsamp = 2;
        }
        let _ = writeln!(io.out, "   - compute new registration ( {pcount} params )");
        trans.set_identity();
        let mut p = vec![0.0f64; pcount];
        for (i, &v) in trans.parameters.iter().enumerate() {
            p[i] = v;
        }
        let steps = trans.steps();
        let mut xi = vec![0.0f64; pcount * pcount];
        for i in 0..pcount {
            if i < steps.len() {
                xi[i * pcount + i] = steps[i];
            }
        }
        if self.iscale {
            p[pcount - 1] = second;
            xi[(pcount - 1) * pcount + pcount - 1] = 20.0 * 0.001;
        }

        let result = {
            let mut ctx = Ctx {
                libm: &libm,
                trans: &mut trans,
                iscale: self.iscale,
                symmetry: self.symmetry,
                cost: self.costfun,
                subsamp,
                mh1,
                mh2,
                scf: &mut scf,
                tcf: &mut tcf,
                histo: JointHisto::new(),
                prec: self.cout_precision,
                debug: self.debug > 0,
            };
            self.powell_minimize(g, io, &mut ctx, &mut p, &mut xi, pcount)
        };
        self.trans = Some(trans);
        result?;

        let (mut fm, fs) = self.convert_p2md(g, io.err, &p)?;
        if self.symmetry {
            fm = mul4(&mul4(&mh2, &fm), &mh1);
            if self.verbose > 1 {
                let _ = writeln!(io.out, "     -- adjusting half-way maps ");
            }
            let ch = fixed(&matrix_sqrt(g, io.out, io.err, &dynm(&fm))?);
            let ci = inv4(&fm);
            let chi = mul4(&ch, &ci);
            self.mov2weights = Some(dynm(&ch));
            self.dst2weights = Some(dynm(&chi));
        } else {
            fm = mul4(&fm, &initial_m);
            self.mov2weights = Some(dynm(&fm));
            self.dst2weights = Some(VnlMatrix::identity(4));
        }
        if self.iscale {
            self.iscalefinal = libm.exp(fs);
        }
        self.mfinal = Some(dynm(&fm));
        note_glibc07(g, &libm, "log, exp");
        let _ = writeln!(io.out);
        let _ = writeln!(io.out, " DONE ");
        // `MRIfree(&mriT)`: FreeSurfer frees the byte target that `tcf` still names (FS-1075);
        // its voxels go, its header stays as the half-way geometry
        drop(scf);
        tcf.chunk = None;
        self.powell_hw = Some(tcf);
        Ok(())
    }

    /// The initial cost, then `fs_powell` from `p` along `xi`, with the source's messages.
    fn powell_minimize(
        &mut self,
        g: &mut Globals,
        io: &mut Io,
        ctx: &mut Ctx,
        p: &mut [f64],
        xi: &mut [f64],
        pcount: usize,
    ) -> Result<(), Fatal> {
        let min_sse = ctx.cost(g, io, p)?;
        let _ = writeln!(io.out, "   - initial cost: {}", self.os(min_sse));
        let _ = writeln!(io.out, " START POWELL");
        let mut failed: Option<Fatal> = None;
        // the cost function's lines and Powell's own go to the same streams in their order
        let buf_out = RefCell::new(Vec::new());
        let buf_err = RefCell::new(Vec::new());
        let code = {
            let mut f = |x: &[f64]| -> Option<f64> {
                let mut sub = Io {
                    out: &mut Shared(&buf_out),
                    err: &mut Shared(&buf_err),
                    env: io.env,
                    dir: io.dir,
                };
                match ctx.cost(g, &mut sub, x) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        // FreeSurfer's exit inside the cost function: Powell stops here
                        failed = Some(e);
                        None
                    }
                }
            };
            let mut none = NoFsFunctions;
            let mut functor = FsCostFunction::with_double(&mut none, pcount as i32, &mut f);
            let mut minimizer = FsPowell::new();
            minimizer.base.set_x_tolerance(self.xtol);
            minimizer.base.ftol = self.ftol;
            let code = minimizer.minimize(
                &mut functor,
                p,
                xi,
                &mut Shared(&buf_out),
                &mut Shared(&buf_err),
            );
            (
                code,
                minimizer.base.num_iterations,
                minimizer.base.end_error,
            )
        };
        let _ = io.out.write_all(&buf_out.into_inner());
        let _ = io.err.write_all(&buf_err.into_inner());
        if let Some(e) = failed {
            return Err(e);
        }
        let (code, iterations, end_error) = code;
        match code {
            ReturnCode::ErrorFailure
            | ReturnCode::ErrorDodgyInput
            | ReturnCode::FailedFtolTooSmall
            | ReturnCode::FailedXtolTooSmall
            | ReturnCode::FailedGtolTooSmall => {
                log::error_exit(g, io.err, ERROR_BADPARM, "powell error.")?;
            }
            _ => {
                let _ = writeln!(io.out, " iter: {iterations}");
                // setprecision(14) stays on the stream
                self.cout_precision = 14;
                let _ = writeln!(io.out, " final e = {}", self.os(end_error));
                let _ = write!(io.out, "{}", matlab_print_long_vector(p, "p"));
                let _ = writeln!(io.out);
            }
        }
        Ok(())
    }
}
