// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-topup 2203.5: topupfns.{h,cpp}). Copyright (C) 2012
// University of Oxford (Jesper Andersson). Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the originals. Distributed under the FSL Licence (see LICENSE).
//! topup's command line (`parse_topup_command_line`, `topup_clp`): the options, the
//! configuration file (`--config`: a name, a name with `.cnf`, or either in
//! `$FSLDIR/etc/flirtsch/`, then in larmorx-nc's copy of FSL's settings files; options given on
//! the command line take precedence), the
//! acquisition-parameter file, the checks of every setting, the per-level settings (a single
//! value applies to every level) and the log file of the parsed settings.

use std::io::Write;

use larmorx_core::defects;
use larmorx_nc_core::bfmatrix::Precision;
use larmorx_nc_core::nonlin::NlMethod;
use larmorx_nc_core::options::{ArgFlag, Opt, OptionParser, Parsed, Value};
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volume::{self, FslError};

use crate::Console;
use crate::costfn::Regularisation;
use crate::fileio::Datafile;
use crate::scan::{InterpType, topup_error};

const TITLE: &str = "topup";
const EXAMPLES: &str = "topup --imain=<some 4D image> --datain=<text file> --config=<text file with parameters> --out=my_topup_results\n";

fn opt(key: &'static str, default: Value, help: &'static str, flag: ArgFlag, visible: bool) -> Opt {
    Opt {
        key,
        default,
        help,
        compulsory: false,
        flag,
        visible,
    }
}

/// The options in the order `parse_topup_command_line` adds them (`-h,--help` twice).
fn parser() -> OptionParser {
    use ArgFlag::{NoArgument as N, Requires};
    let r = Requires(1);
    let s = |v: &str| Value::Str(v.to_owned());
    let mut imain = opt("--imain", s(""), "\tname of 4D file with images", r, true);
    imain.compulsory = true;
    let help = opt(
        "-h,--help",
        Value::Bool(false),
        "display help info",
        N,
        true,
    );
    let opts = vec![
        imain,
        opt(
            "--datain",
            s(""),
            "name of text file with PE directions/times",
            r,
            true,
        ),
        opt(
            "--acqp",
            s(""),
            "\talternative way to specify text file with PE directions/times",
            r,
            true,
        ),
        opt(
            "--out",
            s(""),
            "\tbase-name of output files (spline coefficients (Hz) and movement parameters)",
            r,
            true,
        ),
        opt(
            "--fout",
            s(""),
            "\tname of image file with field (Hz)",
            r,
            true,
        ),
        opt(
            "--iout",
            s(""),
            "\tname of 4D image file with unwarped images",
            r,
            true,
        ),
        opt(
            "--featout",
            s(""),
            "\tbase-name of output for export to FEAT",
            r,
            true,
        ),
        opt(
            "--dfout",
            s(""),
            "\tbase-name of 4D image files with displacement fields",
            r,
            false,
        ),
        opt(
            "--rbmout",
            s(""),
            "\tbase-name of ascii files with RB matrices",
            r,
            false,
        ),
        opt(
            "--jacout",
            s(""),
            "\tbase-name of image files with jacobian determinants",
            r,
            false,
        ),
        opt("--logout", s(""), "Name of log-file", r, true),
        opt(
            "--warpres",
            Value::FloatVec(vec![10.0]),
            "(approximate) resolution (in mm) of warp basis for the different sub-sampling levels, default 10",
            r,
            true,
        ),
        opt(
            "--subsamp",
            Value::IntVec(vec![1]),
            "sub-sampling scheme, default 1",
            r,
            true,
        ),
        opt(
            "--fwhm",
            Value::FloatVec(vec![8.0]),
            "\tFWHM (in mm) of gaussian smoothing kernel, default 8",
            r,
            true,
        ),
        opt(
            "--config",
            s(""),
            "Name of config file specifying command line arguments",
            r,
            true,
        ),
        opt(
            "--miter",
            Value::IntVec(vec![5]),
            "\tMax # of non-linear iterations, default 5",
            r,
            true,
        ),
        opt(
            "--lambda",
            Value::FloatVec(vec![0.0]),
            "Weight of regularisation, default depending on --ssqlambda and --regmod switches. See user documetation.",
            r,
            true,
        ),
        opt(
            "--ssqlambda",
            Value::Int(1),
            "If set (=1), lambda is weighted by current ssq, default 1",
            r,
            true,
        ),
        opt(
            "--regmod",
            s("bending_energy"),
            "Model for regularisation of warp-field [membrane_energy bending_energy], default bending_energy",
            r,
            true,
        ),
        opt(
            "--estmov",
            Value::IntVec(vec![100]),
            "Estimate movements if set, default 1 (true)",
            r,
            true,
        ),
        opt(
            "--minmet",
            Value::IntVec(vec![0]),
            "Minimisation method 0=Levenberg-Marquardt, 1=Scaled Conjugate Gradient, default 0 (LM)",
            r,
            true,
        ),
        opt(
            "--splineorder",
            Value::Int(3),
            "Order of spline, 2->Qadratic spline, 3->Cubic spline. Default=3",
            r,
            true,
        ),
        opt(
            "--numprec",
            s("double"),
            "Precision for representing Hessian, double or float. Default double",
            r,
            true,
        ),
        opt(
            "--interp",
            s("spline"),
            "Image interpolation model, linear or spline. Default spline",
            r,
            true,
        ),
        opt(
            "--scale",
            Value::Int(0),
            "\tIf set (=1), the images are individually scaled to a common mean, default 0 (false)",
            r,
            true,
        ),
        opt(
            "--regrid",
            Value::Int(1),
            "\tIf set (=1), the calculations are done in a different grid, default 1 (true)",
            r,
            true,
        ),
        opt(
            "--nthr",
            Value::Int(1),
            "\tNumber of threads to use (cannot be greater than numbers of hardware cores), default 1",
            r,
            true,
        ),
        help.clone(),
        opt(
            "-v,--verbose",
            Value::Bool(false),
            "Print diagonostic information while running",
            N,
            true,
        ),
        opt(
            "--debug",
            Value::Int(0),
            "Save debug information while running, levels 0 (no info), 1 (some info), 2 (little more info) or 3 (LOTS of info)",
            r,
            false,
        ),
        opt("--trace", Value::Bool(false), "", N, false),
        opt("--profile", Value::Bool(false), "", N, false),
        help,
    ];
    OptionParser::new(TITLE, EXAMPLES, opts)
}

/// `topup_clp`: the settings of a run.
#[derive(Clone, Debug)]
pub struct Clp {
    pub nlev: usize,
    pub imain: String,
    out: String,
    pub fout: String,
    pub iout: String,
    pub featout: String,
    pub dfout: String,
    pub rbmout: String,
    pub jacout: String,
    lout: String,
    pub datafile: Datafile,
    warpres: Vec<f32>,
    subsamp: Vec<i32>,
    fwhm: Vec<f32>,
    miter: Vec<i32>,
    lambda: Vec<f32>,
    pub ssqlambda: bool,
    pub indscale: bool,
    regrid: bool,
    pub regtype: Regularisation,
    pub interp: InterpType,
    pub numprec: Precision,
    optmet: Vec<NlMethod>,
    estmov: Vec<i32>,
    pub sporder: u32,
    pub nthreads: u32,
    /// Whether `--nthr` was given (on the command line or in the configuration file): when it
    /// was not, the run's thread count applies (larmorx's `--n-threads`, else
    /// `OMP_NUM_THREADS`; `topup::main`).
    pub nthr_given: bool,
    pub verbose: bool,
    pub trace: bool,
    pub debug: u32,
    pub profile: bool,
}

/// The last directory separator (`larmorx_nc_core::volume::last_separator`): `/`, as in FSL; on
/// Windows, where FSL does not run, `\` as well.
pub use larmorx_nc_core::volume::last_separator;

/// TOPUP's `path`: up to and including the last `/`.
pub fn path(fullname: &str) -> String {
    match last_separator(fullname) {
        Some(i) => fullname[..=i].to_owned(),
        None => String::new(),
    }
}

/// TOPUP's `filename`: after the last `/`, up to its first `.`.
pub fn filename(fullname: &str) -> String {
    let f = match last_separator(fullname) {
        Some(i) => &fullname[i + 1..],
        None => fullname,
    };
    match f.find('.') {
        Some(i) => f[..i].to_owned(),
        None => f.to_owned(),
    }
}

/// TOPUP's `extension`: from the last `.` (after the last `/`).
pub fn extension(fullname: &str) -> String {
    match fullname.rfind('.') {
        Some(d) => match last_separator(fullname) {
            Some(e) if d <= e => String::new(),
            _ => fullname[d..].to_owned(),
        },
        None => String::new(),
    }
}

/// `check_exist(fname)`: whether an `ifstream` opens it (a directory opens).
fn check_exist(fname: &str) -> bool {
    let p = std::path::Path::new(fname);
    p.is_dir() || std::fs::File::open(p).is_ok()
}

/// `existing_conf_file(cfname)`: `Err` is the exception of `std::string(getenv("FSLDIR"))`
/// when `FSLDIR` is not set (libstdc++'s text). After FSL's own places, a name without a
/// directory is looked for in larmorx-nc's copy of FSL's settings files
/// ([`larmorx_nc_core::fsldata::bundled`]`/etc/flirtsch/`, as named, then with `.cnf`; the user's
/// decision of 2026-11-09), so that `--config=b02b0` works without an FSL installation.
fn existing_conf_file(cfname: &str, fsl: &Settings) -> Result<String, String> {
    existing_conf_file_in(cfname, fsl, larmorx_nc_core::fsldata::bundled())
}

/// [`existing_conf_file`] with larmorx-nc's copy given (tests).
fn existing_conf_file_in(
    cfname: &str,
    fsl: &Settings,
    bundled: Option<std::path::PathBuf>,
) -> Result<String, String> {
    if check_exist(cfname) {
        return Ok(cfname.to_owned());
    }
    if extension(cfname).is_empty() {
        let e = format!("{cfname}.cnf");
        if check_exist(&e) {
            return Ok(e);
        }
    }
    if path(cfname).is_empty() {
        // larmorx-nc's copy: as named, then with `.cnf`
        let in_copy = || {
            let dir = bundled.as_ref()?.join("etc").join("flirtsch");
            let e = dir.join(cfname);
            if e.is_file() {
                return Some(e.to_string_lossy().into_owned());
            }
            let e2 = dir.join(format!("{cfname}.cnf"));
            (extension(cfname).is_empty() && e2.is_file())
                .then(|| e2.to_string_lossy().into_owned())
        };
        let Some(fsldir) = fsl.get("FSLDIR") else {
            return in_copy()
                .ok_or_else(|| "basic_string: construction from null is not valid".to_owned());
        };
        let e = format!("{fsldir}/etc/flirtsch/{cfname}");
        if check_exist(&e) {
            return Ok(e);
        }
        if extension(&e).is_empty() {
            let e2 = format!("{e}.cnf");
            if check_exist(&e2) {
                return Ok(e2);
            }
        }
        if let Some(e) = in_copy() {
            return Ok(e);
        }
    }
    Ok(String::new())
}

/// How the parse ends when it does not give settings: the exit status (FSL's `exit`).
pub type Exit = u8;

/// `std::thread::hardware_concurrency()`.
fn hardware_concurrency() -> u32 {
    std::thread::available_parallelism().map_or(0, |n| n.get() as u32)
}

impl Clp {
    /// `topup_clp(...)`: the checks of every setting.
    fn new(
        p: &OptionParser,
        r: &Parsed,
        fsl: &Settings,
        cerr: &mut Vec<String>,
        warnings: &mut Vec<String>,
        con: &Console,
    ) -> Result<Clp, FslError> {
        let st = |k: &str| r.get(p, k);
        let imain = st("--imain").str().to_owned();
        let invol = volume::read_volume_hdr_only(&imain, fsl)?;
        let nvols = invol.tsize();
        let (datain, acqp) = (st("--datain"), st("--acqp"));
        if datain.set && acqp.set {
            return Err(topup_error(
                "topup_clp::topup_clp: Only one of --datain and --acqp can be set.",
            ));
        } else if !datain.set && !acqp.set {
            return Err(topup_error(
                "topup_clp::topup_clp: Exactly one of --datain and --acqp must be set.",
            ));
        }
        let dname = if datain.set { datain.str() } else { acqp.str() };
        let datafile = Datafile::read(dname, cerr)?;
        if datafile.n() as i64 != nvols {
            // FSL-120: FSL names --datain's value, empty when --acqp gave the file
            let named = if acqp.set && !con.reproduce() {
                con.notice(
                    &defects::FSL_120,
                    "the mismatch names the --acqp file, not --datain's empty value",
                );
                acqp.str()
            } else {
                if acqp.set {
                    con.notice(
                        &defects::FSL_120,
                        "the mismatch names --datain's empty value, not the --acqp file",
                    );
                }
                datain.str()
            };
            return Err(topup_error(&format!(
                "topup_clp::topup_clp: Mismatch between {imain} and {named}"
            )));
        }
        let mut zero_i = 9999usize;
        for i in 0..datafile.n() {
            if datafile.rt[i] < 1e-6 {
                zero_i = i;
            }
        }
        for i in 0..datafile.n() {
            let t = datafile.rt[i];
            if i != zero_i && (t < 0.01 || t > 0.2) {
                return Err(topup_error(
                    "topup_clp::topup_clp: Unrealistic read-out time, should be in range 0.01-0.2 s",
                ));
            }
        }
        let mut warpres = st("--warpres").floats().to_vec();
        let mut subsamp = st("--subsamp").ints().to_vec();
        let mut fwhm = st("--fwhm").floats().to_vec();
        let mut miter = st("--miter").ints().to_vec();
        let mut lambda = st("--lambda").floats().to_vec();
        let mut estmov = st("--estmov").ints().to_vec();
        let minmet = st("--minmet").ints().to_vec();
        let nlev = [
            subsamp.len(),
            warpres.len(),
            fwhm.len(),
            miter.len(),
            lambda.len(),
            estmov.len(),
            minmet.len(),
        ]
        .into_iter()
        .max()
        .unwrap_or(1);
        if nlev > 1 {
            fn level<T: Copy>(v: &mut Vec<T>, nlev: usize, name: &str) -> Result<(), FslError> {
                if v.len() == 1 {
                    v.resize(nlev, v[0]);
                    Ok(())
                } else if v.len() != nlev {
                    Err(topup_error(&format!(
                        "topup_clp::topup_clp: Mismatch between {name} and other parameters "
                    )))
                } else {
                    Ok(())
                }
            }
            level(&mut subsamp, nlev, "--subsamp")?;
            level(&mut warpres, nlev, "--warpres")?;
            level(&mut fwhm, nlev, "--fwhm")?;
            level(&mut miter, nlev, "--miter")?;
            level(&mut lambda, nlev, "--lambda")?;
            level(&mut estmov, nlev, "--estmov")?;
            if minmet.len() != nlev && minmet.len() != 1 {
                return Err(topup_error(
                    "topup_clp::topup_clp: Mismatch between --minmet and other parameters ",
                ));
            }
        }
        for &s in subsamp.iter().take(nlev) {
            let bad = |n: i64| n % i64::from(s) != 0;
            if s < 1 || bad(invol.xsize()) || bad(invol.ysize()) || bad(invol.zsize()) {
                return Err(topup_error(
                    "topup_clp::topup_clp: Subsampling levels incompatible with image data.",
                ));
            }
        }
        let mut optmet = vec![NlMethod::Lm; nlev];
        for (i, m) in optmet.iter_mut().enumerate() {
            let t = if minmet.len() == 1 {
                minmet[0]
            } else {
                minmet[i]
            };
            *m = match t {
                0 => NlMethod::Lm,
                1 => NlMethod::Scg,
                _ => {
                    return Err(topup_error(
                        "topup_clp::topup_clp: Unrecognized minimization method code.",
                    ));
                }
            };
        }
        for i in 0..nlev {
            if estmov[i] != 0 && optmet[i] == NlMethod::Scg {
                warnings.push(
                    "topup_clp::topup_clp: WARNING, it is generally NOT a good idea to include movements when using the Scaled Conjugate Gradient method.".to_owned(),
                );
            }
        }
        let numprec = match st("--numprec").str() {
            "float" => Precision::Float,
            "double" => Precision::Double,
            other => {
                return Err(topup_error(&format!(
                    "topup_clp::topup_clp: Unrecognized value {other} as argument to --numprec."
                )));
            }
        };
        let regtype = match st("--regmod").str() {
            "membrane_energy" => Regularisation::MembraneEnergy,
            "bending_energy" => Regularisation::BendingEnergy,
            other => {
                return Err(topup_error(&format!(
                    "topup_clp::topup_clp: Unrecognized value {other} as argument to --regmod."
                )));
            }
        };
        let sporder = st("--splineorder").int() as u32;
        if !(2..=3).contains(&sporder) {
            return Err(topup_error(
                "topup_clp::topup_clp: Only quadratic or cubic splines allowed.",
            ));
        }
        let interp = match st("--interp").str() {
            "linear" => InterpType::Linear,
            "spline" => InterpType::Spline,
            other => {
                return Err(topup_error(&format!(
                    "topup_clp::topup_clp: Unrecognized value {other} as argument to --interp."
                )));
            }
        };
        let nthr = st("--nthr").int();
        let nthreads = if nthr != 1 {
            let ncores = hardware_concurrency();
            if ncores == 0 || (nthr as u32) <= ncores {
                nthr as u32
            } else {
                ncores
            }
        } else {
            1
        };
        Ok(Clp {
            nlev,
            imain,
            out: st("--out").str().to_owned(),
            fout: st("--fout").str().to_owned(),
            iout: st("--iout").str().to_owned(),
            featout: st("--featout").str().to_owned(),
            dfout: st("--dfout").str().to_owned(),
            rbmout: st("--rbmout").str().to_owned(),
            jacout: st("--jacout").str().to_owned(),
            lout: st("--logout").str().to_owned(),
            datafile,
            warpres,
            subsamp,
            fwhm,
            miter,
            lambda,
            ssqlambda: st("--ssqlambda").int() != 0,
            indscale: st("--scale").int() != 0,
            regrid: st("--regrid").int() != 0,
            regtype,
            interp,
            numprec,
            optmet,
            estmov,
            sporder,
            nthreads,
            nthr_given: st("--nthr").set,
            verbose: st("-v,--verbose").bool(),
            trace: st("--trace").bool(),
            debug: st("--debug").int() as u32,
            profile: st("--profile").bool(),
        })
    }

    fn base(&self) -> String {
        if self.out.is_empty() {
            format!("{}{}", path(&self.imain), filename(&self.imain))
        } else {
            self.out.clone()
        }
    }

    /// `CoefFname()`.
    pub fn coef_fname(&self) -> String {
        format!("{}_fieldcoef", self.base())
    }

    /// `MovParFname()`.
    pub fn movpar_fname(&self) -> String {
        format!("{}_movpar.txt", self.base())
    }

    /// `ProfileFname()`.
    pub fn profile_fname(&self) -> String {
        format!("{}.topup_profile", self.base())
    }

    /// `LogFname()`.
    pub fn log_fname(&self) -> String {
        if self.lout.is_empty() {
            format!("{}{}.topup_log", path(&self.imain), filename(&self.imain))
        } else {
            self.lout.clone()
        }
    }

    fn check(&self, level: usize, name: &str) -> Result<usize, FslError> {
        if level < 1 || level > self.nlev {
            return Err(topup_error(&format!(
                "topup_clp::{name}: Out-of-range value of level"
            )));
        }
        Ok(level - 1)
    }

    /// `SubSampling(level)` (1-based).
    pub fn subsampling(&self, level: usize) -> Result<u32, FslError> {
        Ok(self.subsamp[self.check(level, "SubSampling")?] as u32)
    }

    pub fn warp_res(&self, level: usize) -> Result<f64, FslError> {
        Ok(f64::from(self.warpres[self.check(level, "WarpRes")?]))
    }

    pub fn fwhm(&self, level: usize) -> Result<f64, FslError> {
        Ok(f64::from(self.fwhm[self.check(level, "FWHM")?]))
    }

    pub fn lambda(&self, level: usize) -> Result<f64, FslError> {
        Ok(f64::from(self.lambda[self.check(level, "Lambda")?]))
    }

    /// `--miter` at `level` as given (FSL-123: a negative value).
    pub fn max_iter_signed(&self, level: usize) -> i32 {
        self.miter.get(level.wrapping_sub(1)).copied().unwrap_or(0)
    }

    pub fn max_iter(&self, level: usize) -> Result<u32, FslError> {
        Ok(self.miter[self.check(level, "MaxIter")?] as u32)
    }

    pub fn estimate_movements(&self, level: usize) -> Result<bool, FslError> {
        Ok(self.estmov[self.check(level, "EstimateMovements")?] != 0)
    }

    pub fn optimisation_method(&self, level: usize) -> Result<NlMethod, FslError> {
        Ok(self.optmet[self.check(level, "OptimisationMethod")?])
    }

    /// `Regridding(invols)`: the image's size, or with `--regrid` that plus the largest
    /// subsampling.
    pub fn regridding(&self, size: [u32; 3]) -> Result<[u32; 3], FslError> {
        if !self.regrid {
            return Ok(size);
        }
        let mut maxss = 1u32;
        for i in 0..self.nlev {
            maxss = maxss.max(self.subsampling(i + 1)?);
        }
        Ok(size.map(|s| s + maxss))
    }
}

/// What `parse_topup_command_line` gives: the settings, or the exit status after FSL's
/// messages (written already).
pub enum ParseEnd {
    Run(Box<Clp>),
    Exit(Exit),
}

/// `parse_topup_command_line(argc, argv)`, with its messages on `cerr` and `cout` and the log
/// file written.
pub fn parse(
    argv: &[String],
    fsl: &Settings,
    cout: &mut dyn Write,
    cerr: &mut dyn Write,
    con: &Console,
) -> ParseEnd {
    let p = parser();
    let usage = |cerr: &mut dyn Write| {
        let _ = cerr.write_all(p.usage().as_bytes());
    };
    let mut parsed = match p.parse_command_line(argv, 0, false) {
        Ok(r) => r,
        Err((_, msg)) => {
            usage(cerr);
            let _ = write!(cerr, "\n{msg}\n");
            return ParseEnd::Exit(1);
        }
    };
    let config = parsed.get(&p, "--config").clone();
    if config.set {
        let fname = match existing_conf_file(config.str(), fsl) {
            Ok(f) => f,
            Err(what) => {
                // FSL-121 (rule B): string(getenv("FSLDIR")) of a null pointer
                if con.reproduce() {
                    con.notice(
                        &defects::FSL_121,
                        "FSLDIR unset: the C++ library's text instead of what is missing",
                    );
                    let _ = write!(cerr, "\n{what}\n");
                    return ParseEnd::Exit(1);
                }
                con.notice(&defects::FSL_121, "FSLDIR unset: a clear message");
                let _ = writeln!(
                    cerr,
                    "larmorx-nc: error: topup: --config={}: no such file here, and FSLDIR is \
                     unset, so FSL's configurations ($FSLDIR/etc/flirtsch) cannot be searched \
                     (FSL-121)",
                    config.str()
                );
                return ParseEnd::Exit(1);
            }
        };
        if fname.is_empty() {
            let _ = writeln!(cerr, "Cannot find config-file {}", config.str());
            return ParseEnd::Exit(1);
        }
        // FSL-122 (rule A): FSL's reading ends at a line longer than 1023 characters
        match p.parse_config_file_lines(&mut parsed, &fname, !con.reproduce()) {
            Err(msg) => {
                usage(cerr);
                let _ = write!(cerr, "\n{msg}\n");
                return ParseEnd::Exit(1);
            }
            Ok(true) => con.notice(
                &defects::FSL_122,
                if con.reproduce() {
                    "the settings after a configuration line of more than 1023 characters ignored"
                } else {
                    "the settings after a configuration line of more than 1023 characters read"
                },
            ),
            Ok(false) => {}
        }
    }
    if parsed.get(&p, "-h,--help").bool() || p.check_compulsory(&parsed).is_some() {
        usage(cerr);
        return ParseEnd::Exit(1);
    }
    let (mut lines, mut warnings) = (Vec::new(), Vec::new());
    let made = Clp::new(&p, &parsed, fsl, &mut lines, &mut warnings, con);
    for w in &warnings {
        let _ = writeln!(cout, "{w}");
    }
    let clp = match made {
        Ok(c) => c,
        Err(e) => {
            for l in &lines {
                let _ = writeln!(cerr, "{l}");
            }
            let _ = cerr.write_all(e.printed_text().as_bytes());
            usage(cerr);
            let _ = write!(cerr, "\n{}\n", e.what);
            return ParseEnd::Exit(1);
        }
    };
    for l in &lines {
        let _ = writeln!(cerr, "{l}");
    }
    // the log of the parsed settings (an ofstream that cannot be opened writes nothing)
    let mut log = String::new();
    for key in ["--imain", "--datain", "--acqp", "--out"] {
        log_opt(&p, &parsed, key, &mut log);
    }
    for key in ["--fout", "--iout", "--featout"] {
        if parsed.get(&p, key).set {
            log_opt(&p, &parsed, key, &mut log);
        }
    }
    for key in [
        "--warpres",
        "--subsamp",
        "--fwhm",
        "--miter",
        "--lambda",
        "--ssqlambda",
        "--estmov",
        "--minmet",
        "--regmod",
        "--splineorder",
        "--numprec",
        "--interp",
        "--scale",
        "--regrid",
        "--nthr",
    ] {
        log_opt(&p, &parsed, key, &mut log);
    }
    let _ = std::fs::write(clp.log_fname(), log);
    ParseEnd::Run(Box::new(clp))
}

fn log_opt(p: &OptionParser, r: &Parsed, key: &str, log: &mut String) {
    let i = p
        .opts
        .iter()
        .position(|o| o.key == key)
        .expect("a topup option");
    log.push_str(&p.opts[i].print(&r.states[i]));
    log.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FSL's places first (as named, `.cnf`, `$FSLDIR/etc/flirtsch/`), then larmorx-nc's copy
    /// for a name without a directory; FSLDIR unset with nothing in the copy keeps FSL's text.
    #[test]
    fn the_settings_file_fsl_then_larmorx_ncs_copy() {
        let root = tempfile::tempdir().unwrap();
        let (fsldir, copy) = (root.path().join("fsl ~1"), root.path().join("share"));
        for (dir, name) in [(&fsldir, "a.cnf"), (&copy, "a.cnf"), (&copy, "b.cnf")] {
            let d = dir.join("etc").join("flirtsch");
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join(name), "--miter=1\n").unwrap();
        }
        let path_of = |dir: &std::path::Path, name: &str| {
            dir.join("etc")
                .join("flirtsch")
                .join(name)
                .to_string_lossy()
                .into_owned()
        };
        let set = Settings::new().with("FSLDIR", &fsldir.to_string_lossy());
        let unset = Settings::new().without("FSLDIR");
        let fsl_a = format!("{}/etc/flirtsch/a.cnf", fsldir.to_string_lossy());
        let got = |name: &str, s: &Settings, copy: Option<&std::path::Path>| {
            existing_conf_file_in(name, s, copy.map(|p| p.to_path_buf()))
        };
        assert_eq!(got("a", &set, Some(&copy)), Ok(fsl_a.clone()));
        assert_eq!(got("a.cnf", &set, Some(&copy)), Ok(fsl_a));
        assert_eq!(got("b", &set, Some(&copy)), Ok(path_of(&copy, "b.cnf")));
        assert_eq!(got("b.cnf", &set, Some(&copy)), Ok(path_of(&copy, "b.cnf")));
        assert_eq!(got("b", &set, None), Ok(String::new()));
        assert_eq!(got("c", &set, Some(&copy)), Ok(String::new()));
        assert_eq!(got("a", &unset, Some(&copy)), Ok(path_of(&copy, "a.cnf")));
        let fsl_text = Err("basic_string: construction from null is not valid".to_owned());
        assert_eq!(got("c", &unset, Some(&copy)), fsl_text);
        assert_eq!(got("a", &unset, None), fsl_text);
        // a name with a directory is never looked for in the copy
        assert_eq!(got("x/b", &set, Some(&copy)), Ok(String::new()));
    }
}
