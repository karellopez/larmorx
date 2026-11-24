// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_robust_template`'s `main` and `getRandomNumber` (`mri_robust_template.cpp` 135-404): the
//! program's entry point [`run`] (registered as `mri_robust_template` in the crate's `PROGRAMS`)
//! and [`main`].
//!
//! The version line (`8.2.0`) and an empty line first; the version options (`handleVersionOption`:
//! the program ends with status 0 when nothing else is given, recon-all's `-all-info`); the
//! command line ([`super::options`]). Then the [`MultiRegistration`] of the settings: its folder for
//! the debug files (from the template's name, FS-1098), the inputs read (`Could not load
//! movables!` when fewer than two), the initial transforms and intensities (`Error: num LTA files
//! != num movables!`, `Error: num intensity files != num movables!`, both status 1); without
//! `--inittp` and `--ixforms` the initial target drawn at random (`Will use TP <n> as random initial
//! target (seed <s> ).`, glibc's `srand(seed)` and `rand() % n + 1`, the seed from the images'
//! centres unless `--seed`); then:
//!
//! - `--noit`, or two inputs: each input registered to the initial target (`initialXforms(tp,
//!   fixtp, 0, 5, 0.01)`, unless `--ixforms`), the template made with the final sample type;
//! - more inputs: the initial transforms the same way, then `computeTemplate` (the global
//!   iterations: `--maxit` (`-1`: 6) and `--epsit` (0 or less: 0.03), the pairwise registrations'
//!   `--pairmaxit` and `--pairepsit`), and the template made again when `--finalnearest` changed
//!   the sample type.
//!
//! Then `Writing final template: <name>` and the template, the `--conform` template, `Writing final
//! transforms (warps etc.)...`, the transforms (`--lta`, `--vox2vox`), the mapped inputs
//! (`--mapmov`), the inputs with moved headers (`--mapmovhdr`), the intensity factors
//! (`--iscaleout`), the weights (`--weights`, not with `--leastsquares`); the time taken
//! (`registration took <m> minutes and <s> seconds.`, from the clock) and the citation. Status 0.
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-robust-template.md`), besides
//! [`super::options`]' and [`super::multireg`]'s:
//! - **FS-1083** (nondeterminism): when the images' centres are all 0 the seed is 0 and
//!   `getRandomNumber` takes the clock's (`time(NULL)`): the target and the printed seed change
//!   from run to run. By default the seed stays 0 (glibc's `srand(0)`, the stream of seed 1),
//!   noted; `reproduce` takes the clock.
//! - **FS-1096** (results): a write that fails (the template, a transform, an intensity file, a
//!   mapped input, weights) ends the program with status 0, after FreeSurfer's messages. By
//!   default status 1 at the end (rule A), noted; `reproduce` 0.
//! - **FS-1097** (rule C, kept): with one 4-D input the random target is drawn among
//!   `--mov`'s names (one), so it is always frame 1; noted.
//! - **FS-1098** (results, `--debug` only): the debug files' folder is the template's name up to
//!   two bytes before its last `/` (`substr(0, pos - 1)`: `out/t.mgz` puts them in the working
//!   folder with the prefix `ou`; a name whose only `/` is its first takes the whole name). By
//!   default the folder itself (`out/`; rule A), noted when the two differ; `reproduce`
//!   FreeSurfer's.

use std::io::Write;

use larmorx_core::defects::{FS_1083, FS_1096, FS_1097, FS_1098};
use larmorx_core::glibc_random::GlibcRandom;
use larmorx_freesurfer_core::ctime;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, Fatal};
use larmorx_freesurfer_core::run::Settings;
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::timer::Timer;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_core::version::{get_version, handle_version_option};

use super::multireg::MultiRegistration;
use super::options::{Options, Run, parse_command_line};

/// The program's name as `argv[0]` gives it (fMRIPrep and recon-all run it by name).
pub const NAME: &str = "mri_robust_template";

/// `mri_robust_template`'s entry point: its original arguments (without the program's name), the
/// settings, the two streams; the exit status. The notices of the run follow on standard error.
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut g = Globals::new(settings.mode);
    g.stderr.read_write = system::stderr_read_write();
    let status = {
        let mut terr = log::TrackedErr::new(&g, &mut *err);
        let mut r = Run {
            out: &mut *out,
            err: &mut terr,
            env: &settings.env,
            dir: settings.dir.as_deref(),
            threads: settings.threads,
            input: None,
        };
        match main(&mut g, &mut r, args) {
            Ok(()) => 0,
            Err(f) => f.status(),
        }
    };
    let _ = out.flush();
    g.notices.print(err);
    status
}

/// `getRandomNumber(start, end, seed)`: a seed of 0 replaced by the clock's (FS-1083), then
/// `srand(seed)` and `rand() % (end - start + 1) + start`.
fn get_random_number(g: &mut Globals, start: i32, end: i32, seed: &mut u32) -> i32 {
    if *seed == 0 {
        g.notices.add(
            &FS_1083,
            "the random target's seed is 0 (the images' centres are empty): FreeSurfer takes the clock's",
        );
        if g.notices.reproduce() {
            *seed = ctime::now() as u32;
        }
    }
    let mut rnd = GlibcRandom::new();
    rnd.seed(*seed);
    let range = end - start + 1;
    rnd.rand() % range + start
}

/// The debug files' folder (`MR.setOutdir`): FreeSurfer's `P.mean.substr(0, pos - 1)` for the
/// last `/` at `pos` (FS-1098, noted with `--debug`, the only output it reaches), `./` without one.
fn outdir(g: &mut Globals, mean: &str, debug: bool) -> String {
    let Some(pos) = mean.rfind('/') else {
        return "./".to_owned();
    };
    // substr(0, pos - 1): pos 0 wraps to npos (the whole name)
    let official = if pos == 0 {
        mean.to_owned()
    } else {
        mean[..pos - 1].to_owned()
    };
    let folder = mean[..pos + 1].to_owned();
    if official != folder && debug {
        g.notices.add(
            &FS_1098,
            format!(
                "the debug files' folder of --template {mean}: FreeSurfer's '{official}' (two bytes short of the folder)"
            ),
        );
    }
    if g.notices.reproduce() {
        official
    } else {
        folder
    }
}

/// `main(argc, argv)` on the arguments after the program's name. `Ok` is status 0.
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    let _ = write!(r.out, "{}\n\n", get_version());
    let _ = r.out.flush();
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let (nargs, left) = handle_version_option(g, r.env, r.out, &argv, NAME);
    if nargs != 0 && left.len() == 1 {
        return Err(Fatal::Exit(0));
    }
    g.progname = NAME.to_owned();
    let mut p = Options::default();
    parse_command_line(g, r, &mut p, &left[1..])?;
    let mut start = Timer::new();
    start.reset();

    let mut mr = MultiRegistration::new();
    mr.outdir = outdir(g, &p.mean, p.debug != 0);
    mr.debug = p.debug;
    mr.rigid = !p.affine;
    mr.transonly = p.transonly;
    if p.iscale {
        let _ = writeln!(r.out, "Setting iscale ...");
    }
    mr.iscale = p.iscale;
    if p.iscaleonly {
        let _ = writeln!(r.out, "Setting iscaleonly ...");
    }
    mr.iscaleonly = p.iscaleonly;
    mr.nomulti = p.nomulti;
    mr.robust = !p.leastsquares;
    mr.sat = p.sat;
    mr.satit = p.satit;
    mr.fixvoxel = p.fixvoxel;
    mr.keeptype = !p.floattype;
    mr.average = p.average;
    mr.doubleprec = p.doubleprec;
    mr.subsamplesize = p.subsamplesize;
    mr.highit = p.highit;
    if !p.nweights.is_empty() {
        mr.backupweights = true;
    }
    mr.crascenter = p.crascenter;
    mr.resthresh = p.resthresh;
    mr.frobnormthresh = p.frobnormthresh;

    let nin = mr.load_movables(g, r, &p.mov, &p.masks, p.allow_diff_vox_size)?;
    if nin <= 1 {
        let _ = r.out.flush();
        let _ = writeln!(r.err, "Could not load movables!");
        return Err(Fatal::Exit(1));
    }
    if !p.iltas.is_empty() {
        let loaded = mr.load_ltas(g, r, &p.iltas)?;
        if loaded != nin {
            let _ = r.out.flush();
            let _ = writeln!(r.err, "Error: num LTA files != num movables!");
            return Err(Fatal::Exit(1));
        }
    }
    if !p.iscalein.is_empty() {
        let loaded = mr.load_intensities(g, r, &p.iscalein)?;
        if loaded != nin {
            let _ = r.out.flush();
            let _ = writeln!(r.err, "Error: num intensity files != num movables!");
            return Err(Fatal::Exit(1));
        }
    }
    if p.inittp < 0 && p.iltas.is_empty() {
        if p.seed == 0 {
            p.seed = mr.get_seed(g, r)?;
        }
        if p.mov.len() == 1 {
            g.notices.add(
                &FS_1097,
                "the random target of a 4-D input is drawn among one name: always frame 1 (kept)",
            );
        }
        p.inittp = get_random_number(g, 1, p.mov.len() as i32, &mut p.seed);
        let _ = writeln!(
            r.out,
            "Will use TP {} as random initial target (seed {} ).",
            p.inittp, p.seed
        );
    }

    if p.noit || nin == 2 {
        if p.iltas.is_empty() && p.inittp > 0 {
            mr.initial_xforms(g, r, p.inittp, p.fixtp, 0, 5, 0.01)?;
        }
        mr.set_sample_type(r.out, p.finalinterp)?;
        mr.map_and_average_mov(g, r, 0)?;
    } else {
        if p.iltas.is_empty() && p.inittp > 0 {
            mr.initial_xforms(g, r, p.inittp, p.fixtp, 0, 5, 0.01)?;
        }
        if p.iterate == -1 {
            p.iterate = 6;
        }
        if p.epsit <= 0.0 {
            p.epsit = 0.03;
        }
        mr.compute_template(g, r, p.iterate, p.epsit, p.pairiterate, p.pairepsit)?;
        if p.finalinterp != mr.sampletype {
            mr.set_sample_type(r.out, p.finalinterp)?;
            mr.map_and_average_mov(g, r, 0)?;
        }
    }

    let _ = writeln!(r.out, "Writing final template: {}", p.mean);
    mr.write_mean(g, r, &p.mean)?;
    if !p.conform.is_empty() {
        mr.write_conform_mean(g, r, &p.conform)?;
    }
    let _ = writeln!(r.out, "Writing final transforms (warps etc.)...");
    if !p.nltas.is_empty() {
        mr.write_ltas(g, r, &p.nltas, p.lta_vox2vox, &p.mean)?;
    }
    if !p.nwarps.is_empty() {
        mr.write_warps(g, r, &p.nwarps)?;
    }
    if !p.mapmovhdr.is_empty() {
        mr.write_map_mov_hdr(g, r, &p.mapmovhdr)?;
    }
    if !p.iscaleout.is_empty() {
        mr.write_intensities(g, r, &p.iscaleout);
    }
    if !p.leastsquares && !p.nweights.is_empty() {
        mr.write_weights(g, r, &p.nweights, p.oneminusweights)?;
    }
    let failed = mr.write_failed;
    drop(mr);

    let msec = start.milliseconds();
    let seconds = nint(g, (msec as f32 / 1000.0) as f64);
    let (minutes, seconds) = (seconds / 60, seconds % 60);
    let _ = writeln!(
        r.out,
        "registration took {minutes} minutes and {seconds} seconds."
    );
    let _ = write!(r.out, "\n Thank you for using RobustTemplate! \n");
    let _ = write!(
        r.out,
        " If you find it useful and use it for a publication, please cite: \n\n"
    );
    let _ = writeln!(
        r.out,
        " Within-Subject Template Estimation for Unbiased Longitudinal Image Analysis"
    );
    let _ = writeln!(
        r.out,
        " M. Reuter, N.J. Schmansky, H.D. Rosas, B. Fischl. NeuroImage 2012."
    );
    let _ = writeln!(r.out, " http://dx.doi.org/10.1016/j.neuroimage.2012.02.084");
    let _ = write!(r.out, " http://reuter.mit.edu/papers/reuter-long12.pdf\n\n");
    if failed {
        g.notices.add(
            &FS_1096,
            "an output was not written: FreeSurfer ends with status 0 all the same",
        );
        if !g.notices.reproduce() {
            let _ = r.out.flush();
            let _ = writeln!(
                r.err,
                "mri_robust_template: an output could not be written (FreeSurfer's status would be 0)"
            );
            return Err(Fatal::Exit(1));
        }
    }
    Ok(())
}
