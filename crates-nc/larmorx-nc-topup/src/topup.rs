// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-topup 2203.5: topup.cpp). Copyright (C) 2012
// University of Oxford (Jesper Andersson). Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the originals. Distributed under the FSL Licence (see LICENSE).
//! topup's `main`: the input scaled to a mean of 100 (all scans together, or each with
//! `--scale`), the first resolution level and then each further one (subsampling, smoothing,
//! warp resolution, movements, regularisation, a new minimisation from the last parameters),
//! and the outputs, with FSL's messages and exit statuses at every stage.

use std::io::Write;

use larmorx_core::defects::{self, UpstreamDefects};
use larmorx_nc_core::exception;
use larmorx_nc_core::nonlin::{self, NlMethod, NonlinParam};
use larmorx_nc_core::profiler::Profiler;
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

/// The threads of topup's work (the transforms' slices): `--nthr` when it is given, on the
/// command line or in the configuration file (`NoOfThreads(n)`: 0 is the default, 1), else the
/// run's thread count (larmorx's `--n-threads`, else `OMP_NUM_THREADS`, else every logical CPU:
/// the user's decision of 2026-11-11, a tool's own thread option winning only when it is
/// given). The results are FSL's one-thread results at any count (FSL-116), and the log keeps
/// `--nthr`'s own value.
fn work_threads(nthr_given: bool, nthr: u32, run_threads: usize) -> u32 {
    if !nthr_given {
        u32::try_from(run_threads.max(1)).unwrap_or(u32::MAX)
    } else if nthr == 0 {
        1
    } else {
        nthr
    }
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
    let con = Console::with_mode(out, err, run.mode);
    let code = main_streams(argv, run, &con);
    con.print_notices();
    code
}

fn main_streams(argv: &[String], run: &Run, con: &Console) -> u8 {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        exception::catch(|| body(argv, run, con))
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
        let end = options::parse(argv, run.fsl, &mut o, &mut e, con);
        con.put(&String::from_utf8_lossy(&o));
        con.err_put(&String::from_utf8_lossy(&e));
        match end {
            ParseEnd::Run(c) => c,
            ParseEnd::Exit(code) => return code,
        }
    };
    // --profile: <out>.topup_profile + "_" + __FILE__ + "_" + __func__ + ".txt"; a file that
    // cannot be opened throws out of main's body
    let base = clp.profile.then(|| clp.profile_fname());
    let mut prof = Profiler::new(base.as_deref(), "_topup.cpp_main.txt")
        .unwrap_or_else(|e| exception::throw(e));
    let total_key = prof.start_entry("Begins topup run");
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
    if let Some(code) = meaningless_settings(&clp, con) {
        return code;
    }
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
    let nthr = work_threads(clp.nthr_given, clp.nthreads, run.threads);
    let prepared = guarded(|| prepare(&clp, &inv, nthr, run, con));
    let (mut cf, mut nlpar) = match prepared {
        Ok(x) => x,
        Err(e) => {
            report(con, "Error occurred when preparing to run topup", &e);
            return 1;
        }
    };
    let level_key = prof.start_entry("Begins topup level 1");
    if let Err(e) = guarded(|| nonlin::nonlin(&mut nlpar, &mut cf).map(|_| ())) {
        report(con, "Error occurred when running first level of topup", &e);
        return 1;
    }
    prof.end_entry(level_key);
    for l in 2..=clp.nlev {
        let level_key = prof.start_entry(&format!("Begins topup level {l}"));
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
        prof.end_entry(level_key);
    }
    if let Err(e) = guarded(|| write_outputs(&clp, &mut cf, &inv, gmean)) {
        report(con, "Error occured while writing output from topup", &e);
        return 1;
    }
    prof.end_entry(total_key);
    // exit(EXIT_SUCCESS): the profiler is never destroyed (no line for open entries)
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
    cf.set_regridding(regridding(clp, size, con)?)?;
    cf.subsample(clp.subsampling(1)?)?;
    cf.smooth(clp.fwhm(1)?);
    cf.set_movements_fixed(!clp.estimate_movements(1)?);
    cf.set_regularisation(clp.lambda(1)?, clp.regtype);
    cf.set_ssq_lambda(clp.ssqlambda);
    cf.set_hessian_precision(clp.numprec);
    let np = new_nonlin(clp, 1, &cf, false)?;
    Ok((cf, np))
}

/// `Regridding(invols)`; FSL-118 (rule A): with subsamplings whose largest does not divide
/// the others (3 and 2), FSL's grid (the image grown by the largest) cannot be subsampled by
/// the others, and topup stops at the first such level; corrected, the grid grows by their
/// least common multiple (the same grid wherever FSL's runs).
fn regridding(clp: &Clp, size: [u32; 3], con: &Console) -> Result<[u32; 3], FslError> {
    let fsl = clp.regridding(size)?;
    if fsl == size {
        return Ok(fsl);
    }
    let mut lcm = 1u32;
    for l in 1..=clp.nlev {
        let s = clp.subsampling(l)?;
        let (mut a, mut b) = (lcm, s);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        lcm = lcm / a * s;
    }
    let maxss = fsl[0] - size[0];
    if lcm == maxss {
        return Ok(fsl);
    }
    if con.reproduce() {
        con.notice(
            &defects::FSL_118,
            "the grid grown by the largest subsampling, which the others do not divide",
        );
        return Ok(fsl);
    }
    con.notice(
        &defects::FSL_118,
        &format!("the grid grown by the subsamplings' least common multiple, {lcm}"),
    );
    Ok(size.map(|s| s + lcm))
}

/// D13 (rule B): settings FSL accepts although they have no meaning, refused by default
/// (`reproduce` runs them as FSL does): a negative number of iterations (FSL-123: no
/// iteration), warp resolution (FSL-124: also refused with `reproduce` where FSL's knot spacing
/// wraps), smoothing width (FSL-125) or regularisation weight (FSL-126).
fn meaningless_settings(clp: &Clp, con: &Console) -> Option<u8> {
    let mut bad: Vec<(&'static defects::Defect, String)> = Vec::new();
    for l in 1..=clp.nlev {
        let (m, w) = (clp.max_iter_signed(l), clp.warp_res(l).unwrap_or(0.0));
        let (f, la) = (clp.fwhm(l).unwrap_or(0.0), clp.lambda(l).unwrap_or(0.0));
        if m < 0 {
            bad.push((
                &defects::FSL_123,
                format!("--miter {m} at level {l} (no iteration)"),
            ));
        }
        if w < 0.0 {
            bad.push((&defects::FSL_124, format!("--warpres {w} at level {l}")));
        }
        if f < 0.0 {
            bad.push((&defects::FSL_125, format!("--fwhm {f} at level {l}")));
        }
        if la < 0.0 {
            bad.push((&defects::FSL_126, format!("--lambda {la} at level {l}")));
        }
    }
    let (d, what) = bad.first()?.clone();
    if con.reproduce() {
        for (d, what) in &bad {
            if d.id != defects::FSL_124.id {
                con.notice(d, &format!("{what}: run as FSL does"));
            }
        }
        return None;
    }
    con.notice(d, &format!("{what}: refused"));
    Some(con.refuse(&format!(
        "{what}: a negative value has no meaning ({})",
        d.id
    )))
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

#[cfg(test)]
mod tests {
    use super::work_threads;

    /// `--nthr` given wins (0: FSL's default, one thread); else the run's thread count.
    #[test]
    fn the_run_threads_unless_nthr_is_given() {
        assert_eq!(work_threads(false, 1, 6), 6);
        assert_eq!(work_threads(false, 1, 0), 1);
        assert_eq!(work_threads(true, 1, 6), 1);
        assert_eq!(work_threads(true, 4, 6), 4);
        assert_eq!(work_threads(true, 0, 6), 1);
    }
}
