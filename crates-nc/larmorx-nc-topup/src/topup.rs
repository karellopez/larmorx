// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-topup 2203.5: topup.cpp). Copyright (C) 2012
// University of Oxford (Jesper Andersson). Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the originals. Distributed under the FSL Licence (see LICENSE).
//! topup's `main`: the input scaled to a mean of 100 (all scans together, or each with
//! `--scale`), the first resolution level and then each further one (subsampling, smoothing,
//! warp resolution, movements, regularisation, a new minimisation from the last parameters),
//! and the outputs, with FSL's messages and exit statuses at every stage.

use std::io::Write;

use larmorx_core::defects::UpstreamDefects;
use larmorx_nc_core::exception;
use larmorx_nc_core::nonlin::{self, NlMethod, NonlinParam};
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volume::{self, FslError, Volume};

use crate::costfn::TopupCf;
use crate::options::{self, Clp, ParseEnd};
use crate::{Console, ExitNow, vol};

/// What a run needs besides its arguments.
pub struct Run<'a> {
    pub fsl: &'a Settings,
    pub mode: UpstreamDefects,
    /// Threads for the replica's own work (the results do not depend on them).
    pub threads: usize,
}

/// `f()`, with an exception thrown inside it (`exception::throw`) as its error.
fn guarded<T>(f: impl FnOnce() -> Result<T, FslError>) -> Result<T, FslError> {
    exception::catch(f).and_then(|r| r)
}

/// `cerr << msg << endl; cerr << "Exception thrown with message: " << what << endl;` after
/// the lines the exception printed when thrown.
fn report(con: &Console, msg: &str, e: &FslError) {
    con.err_put(&e.printed_text());
    con.err_line(msg);
    con.err_line(&format!("Exception thrown with message: {}", e.what));
}

/// `topup [options]`: the exit status.
pub fn main(argv: &[String], run: &Run, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let con = Console::new(out, err);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        exception::catch(|| body(argv, run, &con))
    }));
    match r {
        Ok(Ok(code)) => code,
        Ok(Err(e)) => {
            // main's own catch: an exception none of the stages caught
            con.err_put(&e.printed_text());
            con.line(&format!("TOPUP::: Topup failed with message {}", e.what));
            1
        }
        Err(payload) => match payload.downcast::<ExitNow>() {
            Ok(x) => {
                for l in &x.printed {
                    con.err_line(l);
                }
                x.code
            }
            Err(other) => std::panic::resume_unwind(other),
        },
    }
}

fn body(argv: &[String], run: &Run, con: &Console) -> u8 {
    let clp = {
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let end = options::parse(argv, run.fsl, &mut o, &mut e);
        con.put(&String::from_utf8_lossy(&o));
        con.err_put(&String::from_utf8_lossy(&e));
        match end {
            ParseEnd::Run(c) => c,
            ParseEnd::Exit(code) => return code,
        }
    };
    let mut inv: Volume = match volume::read_volume4d::<f32>(&clp.imain, run.fsl) {
        Ok((v, _, _)) => v,
        Err(e) => {
            report(
                con,
                &format!("Error occurred when reading --imain file {}", clp.imain),
                &e,
            );
            return 1;
        }
    };
    // scaled to a mean of 100: one weight against the regularisation
    let nt = inv.tsize() as usize;
    let nv = inv.nvoxels() as usize;
    let mut means = vec![0.0f64; nt];
    let mut gmean = 0.0f64;
    for (i, m) in means.iter_mut().enumerate() {
        let sv = inv.sub_volume(i as i64).expect("a volume of the series");
        *m = vol::mean(&sv);
        gmean += *m;
    }
    gmean /= nt as f64;
    if clp.indscale {
        for (i, m) in means.iter().enumerate() {
            let k = (100.0 / m) as f32;
            for x in &mut inv.data[i * nv..(i + 1) * nv] {
                *x *= k;
            }
        }
    } else {
        vol::scale(&mut inv, (100.0 / gmean) as f32);
    }
    // NoOfThreads(n): 0 is the default (1)
    let nthr = if clp.nthreads == 0 { 1 } else { clp.nthreads };
    let prepared = guarded(|| prepare(&clp, &inv, nthr, run, con));
    let (mut cf, mut nlpar) = match prepared {
        Ok(x) => x,
        Err(e) => {
            report(con, "Error occurred when preparing to run topup", &e);
            return 1;
        }
    };
    if let Err(e) = guarded(|| nonlin::nonlin(&mut nlpar, &mut cf).map(|_| ())) {
        report(con, "Error occurred when running first level of topup", &e);
        return 1;
    }
    for l in 2..=clp.nlev {
        if clp.verbose {
            con.line("***Going to next resolution level***");
        }
        let prep = guarded(|| {
            cf.set_level(l as u32);
            cf.subsample(clp.subsampling(l)?)?;
            cf.smooth(clp.fwhm(l)?);
            cf.set_warp_resolution(clp.warp_res(l)?)?;
            cf.set_movements_fixed(!clp.estimate_movements(l)?);
            cf.set_regularisation(clp.lambda(l)?, clp.regtype);
            new_nonlin(&clp, l, &cf, true)
        });
        nlpar = match prep {
            Ok(p) => p,
            Err(e) => {
                report(
                    con,
                    &format!("Error occurred when preparing to run level {l} of topup"),
                    &e,
                );
                return 1;
            }
        };
        if let Err(e) = guarded(|| nonlin::nonlin(&mut nlpar, &mut cf).map(|_| ())) {
            report(
                con,
                &format!("Error occurred when running level {l} of topup"),
                &e,
            );
            return 1;
        }
    }
    if let Err(e) = guarded(|| write_outputs(&clp, &mut cf, &inv, gmean)) {
        report(con, "Error occured while writing output from topup", &e);
        return 1;
    }
    0
}

/// The cost function and the minimisation of the first level.
fn prepare<'a, 'w>(
    clp: &Clp,
    inv: &Volume,
    nthr: u32,
    run: &'a Run,
    con: &'a Console<'w>,
) -> Result<(TopupCf<'a, 'w>, NonlinParam), FslError> {
    let d = &clp.datafile;
    let mut cf = TopupCf::new(
        inv,
        &d.pv,
        &d.rt,
        clp.warp_res(1)?,
        clp.sporder,
        nthr,
        con,
        run.fsl,
        run.mode,
    )?;
    cf.set_trace_print(clp.trace);
    cf.set_verbose(clp.verbose);
    cf.set_debug(clp.debug)?;
    cf.set_level(1);
    cf.set_interpolation_model(clp.interp);
    let size = [inv.xsize() as u32, inv.ysize() as u32, inv.zsize() as u32];
    cf.set_regridding(clp.regridding(size)?)?;
    cf.subsample(clp.subsampling(1)?)?;
    cf.smooth(clp.fwhm(1)?);
    cf.set_movements_fixed(!clp.estimate_movements(1)?);
    cf.set_regularisation(clp.lambda(1)?, clp.regtype);
    cf.set_ssq_lambda(clp.ssqlambda);
    cf.set_hessian_precision(clp.numprec);
    let np = new_nonlin(clp, 1, &cf, false)?;
    Ok((cf, np))
}

/// `NonlinParam(cf->NPar(), method, start)`: zeros at the first level, `cf->Par()` after;
/// Levenberg-Marquardt's equation solver at 500 iterations and 1e-3.
fn new_nonlin(clp: &Clp, l: usize, cf: &TopupCf, from_par: bool) -> Result<NonlinParam, FslError> {
    let start = if from_par {
        cf.par()
    } else {
        vec![0.0; cf.npar()]
    };
    let mut np = NonlinParam::new(cf.npar(), clp.optimisation_method(l)?, &start)?;
    if np.method() == NlMethod::Lm {
        np.set_equation_solver_max_iter(500);
        np.set_equation_solver_tol(1.0e-3);
    }
    np.set_max_iter(clp.max_iter(l)?);
    Ok(np)
}

/// Everything asked for, after the last level (at full resolution).
fn write_outputs(clp: &Clp, cf: &mut TopupCf, inv: &Volume, gmean: f64) -> Result<(), FslError> {
    if clp.subsampling(clp.nlev)? > 1 {
        cf.smooth(0.0);
        cf.subsample(1)?;
    }
    cf.write_coefficients(&clp.coef_fname())?;
    cf.write_movement_parameters(&clp.movpar_fname())?;
    if !clp.fout.is_empty() {
        cf.write_field(&clp.fout, inv, 1.0)?;
    }
    if !clp.iout.is_empty() {
        cf.write_unwarped(&clp.iout, inv, gmean / 100.0)?;
        cf.write_mean_unwarped(&format!("{}_mean", clp.iout), inv, gmean / 100.0)?;
    }
    if !clp.featout.is_empty() {
        cf.write_field(
            &format!("{}_fieldmap_rads", clp.featout),
            inv,
            2.0 * std::f64::consts::PI,
        )?;
        cf.write_mean_unwarped(&format!("{}_fieldmap_mag", clp.featout), inv, gmean / 100.0)?;
    }
    if !clp.dfout.is_empty() {
        cf.write_displacement_fields(&clp.dfout)?;
    }
    if !clp.rbmout.is_empty() {
        cf.write_rigid_body_matrices(&clp.rbmout)?;
    }
    if !clp.jacout.is_empty() {
        cf.write_jacobians(&clp.jacout)?;
    }
    Ok(())
}
