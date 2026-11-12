// SPDX-License-Identifier: Apache-2.0
//! A clean-room implementation of tedana 26.0.3's `t2smap`: the T2* and S0 maps of multi-echo
//! fMRI and its optimal combination (Posse et al. 1999; Poser et al. 2006; Kundu et al. 2012),
//! written from `specs/t2smap.md` and black-box runs of tedana's program only
//! (`PROVENANCE.md`).
//!
//! Built for speed (`docs/porting-guide.md`, "Clean-room originals"): the masked data held once,
//! voxel-major; the log-linear fit per voxel in closed form; every stage parallel over voxels in
//! fixed chunks, with no result depending on the thread count. The behaviour of §§2–15 (the
//! command line, the order of the run and its messages, the files, the defects' corrections and
//! `reproduce`) is the spec's; the numbers are held to its §17.3.

#![forbid(unsafe_code)]

pub mod adaptive;
pub mod cli;
pub mod combine;
pub mod curvefit;
pub mod fit;
pub mod formats;
pub mod input;
pub mod log;
pub mod maps;
pub mod options;
pub mod output;
pub mod rmse;
pub mod volumes;

use std::path::{Component, Path, PathBuf};

use larmorx_core::defects::{self, Notices};
use larmorx_core::math;
use larmorx_interp::nilearn::Maths;

use crate::adaptive::Used;
use crate::input::Mask;
use crate::log::{Log, PyError, py_list_int, py_list_str};
use crate::options::{CombMode, FitMode, FitType, Options};
use crate::output::{ImageSpec, Outputs};

/// What a completed run wrote.
#[derive(Clone, Debug, Default)]
pub struct Outcome {
    /// The output directory (absolute).
    pub out_dir: PathBuf,
    /// The registry: (description, file name) in the order written.
    pub files: Vec<(String, String)>,
}

/// Python's `os.path.abspath`: joined to the working directory, `.` and `..` resolved as text.
pub fn abspath(cwd: &Path, p: &str) -> PathBuf {
    let joined = cwd.join(p);
    let mut out = PathBuf::new();
    for c in joined.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !matches!(
                    out.components().next_back(),
                    None | Some(Component::RootDir | Component::Prefix(_))
                ) {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Runs `t2smap` (§3) with `options`: its log on `log`, the defects it met in `notices`.
pub fn t2smap(
    options: &Options,
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Result<Outcome, PyError> {
    let n_threads = options.threads();
    larmorx_core::parallel::with_threads(n_threads, || run(options, n_threads, log, notices))
        .map_err(|e| PyError::new("RuntimeError", e.to_string()))?
}

const NEWSLETTER: &str = "Don't forget to subscribe to the tedana newsletter for updates! This is a very low volume \
     email list.";

fn run(
    o: &Options,
    n_threads: usize,
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Result<Outcome, PyError> {
    let cwd_owned = match &o.cwd {
        Some(d) => d.clone(),
        None => std::env::current_dir().map_err(|e| PyError::new("OSError", e.to_string()))?,
    };
    let cwd = Some(cwd_owned.as_path());
    // 1. the output directory
    let out_dir = abspath(&cwd_owned, &o.out_dir);
    if !out_dir.is_dir() {
        std::fs::create_dir(&out_dir).map_err(|e| {
            let (class, errno) = match e.kind() {
                std::io::ErrorKind::NotFound => {
                    ("FileNotFoundError", "[Errno 2] No such file or directory")
                }
                std::io::ErrorKind::AlreadyExists => ("FileExistsError", "[Errno 17] File exists"),
                std::io::ErrorKind::PermissionDenied => {
                    ("PermissionError", "[Errno 13] Permission denied")
                }
                _ => ("OSError", "[Errno 5] Input/output error"),
            };
            PyError::new(
                class,
                format!(
                    "{errno}: {}",
                    larmorx_core::python::argparse::py_repr_str(&out_dir.to_string_lossy())
                ),
            )
        })?;
    }
    // 2. --exclude's text: its errors stop the run here, the output directory made and left
    // empty (§3, observed with tedana's program run directly)
    let exclude = volumes::parse_exclude(o.exclude.as_deref())?;
    // 3., 4.
    log.info(
        "t2smap:t2smap_workflow:365",
        &format!("Using output directory: {}", out_dir.display()),
    );
    let command = format!("t2smap {}", o.call_args.join(" "));
    std::fs::write(out_dir.join("t2smap_call.sh"), command.as_bytes())
        .map_err(|e| PyError::new("OSError", e.to_string()))?;
    // 5.
    if o.fitmode == FitMode::Ts && !exclude.is_empty() {
        return Err(PyError::value(
            "Excluding volumes is not supported for fitmode='ts'. Please set fitmode='all' or \
             remove the exclude argument.",
        ));
    }
    // 7. the echo times
    let tes = echo_times(&o.tes, log, notices)?;
    // 8. the reference
    let n_zcat = (o.data.len() == 1).then_some(tes.len());
    let reference = input::reference(cwd, &o.data[0], n_zcat)?;
    // 9. the outputs
    let figures = out_dir.join("figures");
    if !figures.exists() {
        log.info(
            "io:__init__:164",
            &format!("Generating figures directory: {}", figures.display()),
        );
        std::fs::create_dir(&figures).map_err(|e| PyError::new("OSError", e.to_string()))?;
    }
    let mut outputs = Outputs::new(out_dir.clone(), &o.prefix, o.convention, o.overwrite);
    if o.overwrite {
        let table = out_dir.join(outputs.name("confounds tsv"));
        if table.exists() {
            let _ = std::fs::remove_file(table);
        }
    }
    // 10. the mask
    let maths = if notices.reproduce() {
        Maths {
            exp: math::exp_glibc,
            log: math::log_glibc,
        }
    } else {
        Maths {
            exp: math::exp,
            log: math::log,
        }
    };
    let mask: Mask = match &o.mask {
        Some(m) => {
            log.info("utils:load_mask:916", "Using user-defined mask");
            input::user_mask(cwd, m)?
        }
        None => {
            log.warning(
                "utils:load_mask:943",
                "Computing EPI mask from first echo using nilearn's compute_epi_mask function. \
                 Most external pipelines include more reliable masking functions. It is \
                 strongly recommended to provide an external mask, and to visually confirm that \
                 mask accurately conforms to data boundaries.",
            );
            let computed =
                input::computed_mask(cwd, &o.data[0], &reference, n_zcat, maths, n_threads);
            let (m, empty) = match computed {
                Ok(x) => x,
                Err(e) if e.message == "attempt to get argmax of an empty sequence" => {
                    notices.add(
                        &defects::TED_03,
                        "nilearn's compute_epi_mask has too few voxels to find a threshold",
                    );
                    return Err(if notices.fix() {
                        PyError::value(
                            "the first echo has too few voxels for nilearn's compute_epi_mask \
                             to find a threshold; give a mask with --mask (tedana fails later \
                             with an unrelated error)",
                        )
                    } else {
                        e
                    });
                }
                Err(e) => return Err(e),
            };
            if empty {
                log.py_warning(
                    "input.rs:240",
                    "MaskWarning",
                    "Computed an empty mask. Are you sure that input data are EPI images not \
                     detrended. ",
                );
                notices.add(
                    &defects::TED_03,
                    "nilearn's compute_epi_mask found an empty mask",
                );
                if notices.fix() {
                    return Err(PyError::value(
                        "nilearn's compute_epi_mask found no voxels in the first echo; give a \
                         mask with --mask (tedana fails later with an unrelated error)",
                    ));
                }
            }
            m
        }
    };
    // 11. the data
    log.info(
        "t2smap:t2smap_workflow:414",
        &format!("Loading input data: {}", py_list_str(&o.data)),
    );
    if o.mask.is_some()
        && !mask.four_d()
        && mask.grid()[..] == reference.shape[..3.min(reference.shape.len())]
        && !mask.affine.allclose(&reference.affine, 1e-5, 1e-8)
    {
        notices.add(
            &defects::TED_04,
            "the mask's affine differs from the data's",
        );
        if notices.fix() {
            return Err(input::nilearn_affine_error(&mask.affine, &reference.affine));
        }
    }
    let mut data = input::read_data(cwd, &o.data, tes.len(), &mask, n_threads, log)?;
    // 12. dummy scans
    let n = o.dummy_scans;
    let t_file = data.n_t as i128;
    if n < 0 || (n > 0 && n >= t_file) {
        notices.add(
            &defects::TED_13,
            format!("--dummy-scans {n} of {t_file} volumes"),
        );
        if notices.fix() {
            return Err(PyError::value(format!(
                "--dummy-scans must be at least 0 and below the number of volumes ({t_file}); \
                 {n} given"
            )));
        }
    }
    if n > 0 {
        log.warning(
            "t2smap:t2smap_workflow:418",
            &format!("Removing the first {n} volumes as dummy scans."),
        );
        drop_first_volumes(&mut data, n.min(t_file) as usize);
    }
    let n_t = data.n_t;
    log.debug(
        "t2smap:t2smap_workflow:422",
        &format!(
            "Resulting data shape: ({}, {}, {})",
            data.n_vox, data.n_echo, n_t
        ),
    );
    // 13. excluded volumes
    let mut used_volumes: Option<Vec<usize>> = None;
    if !exclude.is_empty() {
        log.info(
            "t2smap:t2smap_workflow:426",
            &format!("Excluding volumes: {}", py_list_int(&exclude)),
        );
        let left: Vec<i128> = exclude
            .iter()
            .filter(|&&i| i >= n)
            .map(|&i| i - n)
            .collect();
        if left.is_empty() {
            log.warning(
                "t2smap:t2smap_workflow:433",
                &format!("All exclude indices overlap with dummy scans ({n})."),
            );
        } else {
            let max = *left.iter().max().expect("not empty");
            let t = n_t as i128;
            if max > t || (max == t && notices.fix()) {
                if max == t {
                    notices.add(
                        &defects::TED_02,
                        format!("--exclude index {max} of {t} volumes"),
                    );
                    return Err(PyError::value(format!(
                        "The maximum exclude index ({max}) is not below the number of \
                         timepoints in the data ({t})."
                    )));
                }
                return Err(PyError::value(format!(
                    "The maximum exclude index ({max}) is greater than the number of timepoints \
                     in the data ({t})."
                )));
            }
            log.info(
                "t2smap:t2smap_workflow:441",
                &format!(
                    "Excluding {} volumes from adaptive mask and T2*/S0 estimation",
                    left.len()
                ),
            );
            if max == t {
                notices.add(
                    &defects::TED_02,
                    format!("--exclude index {max} of {t} volumes"),
                );
                return Err(PyError::new(
                    "IndexError",
                    format!("index {t} is out of bounds for axis 0 with size {t}"),
                ));
            }
            let vols: Vec<usize> = (0..n_t).filter(|&v| !left.contains(&(v as i128))).collect();
            if vols.is_empty() {
                notices.add(
                    &defects::TED_16,
                    format!(
                        "--exclude removes all {n_t} volumes from the adaptive mask and the fit"
                    ),
                );
                if notices.fix() {
                    return Err(PyError::value(format!(
                        "--exclude leaves no volume for the adaptive mask and the T2*/S0 fit \
                         (all {n_t} volumes excluded)"
                    )));
                }
            }
            used_volumes = Some(vols);
        }
    }
    // 14. the adaptive mask
    let used = Used {
        data: &data,
        volumes: used_volumes.as_deref(),
    };
    let am = adaptive::adaptive_mask(&used, &o.masktype, o.libraries, log, notices)?;
    let count = am.count;
    if let Some(k) = o.n_independent_echos {
        adaptive::independent_echo_warnings(&count, data.n_echo, k, log, notices)?;
    }
    let n_fit = count.iter().filter(|&&c| c >= 1).count();
    log.debug(
        "t2smap:t2smap_workflow:455",
        &format!("Retaining {n_fit}/{} samples for denoising", data.n_vox),
    );
    let spec = ImageSpec {
        grid: mask.grid(),
        offsets: mask.offsets(),
        affine: mask.affine,
        zooms: reference.zooms,
        version: if o.mask.is_none() {
            reference.version
        } else {
            larmorx_io::nifti::NiftiVersion::V1
        },
        lapack: ImageSpec::lapack_for(o.libraries),
        n_threads,
    };
    let counts32: Vec<i32> = count.iter().map(|&c| i32::from(c)).collect();
    outputs.image("adaptive mask img", &spec, &counts32, 1, false)?;
    // 15. the fit
    log.info("t2smap:t2smap_workflow:458", "Computing T2* map");
    let ts = o.fitmode == FitMode::Ts;
    // `--fitmode ts` checks the echo times in each volume's fit, so with no volume (every one a
    // dummy scan: TED-13 in `reproduce`) neither that check nor TED-31's is reached (§3 step 15,
    // observed)
    let no_volume_fit = ts && n_t == 0;
    if data.n_echo != tes.len() && !no_volume_fit {
        return Err(PyError::value(format!(
            "Second dimension of data ({}) does not match number of echoes provided (tes; {})",
            data.n_echo,
            tes.len()
        )));
    }
    if n_fit == 0 && !no_volume_fit {
        // every voxel removed by the adaptive mask (an empty mask failed at its image, §6.6)
        notices.add(
            &defects::TED_31,
            format!(
                "no voxel is left for the fit (the adaptive mask removed all {})",
                data.n_vox
            ),
        );
        return Err(if notices.fix() {
            PyError::value(format!(
                "no voxel of the mask has good signal (the adaptive mask removed all {}), so \
                 there is nothing to fit (tedana fails with IndexError)",
                data.n_vox
            ))
        } else {
            PyError::new(
                "IndexError",
                "index 0 is out of bounds for axis 1 with size 0",
            )
        });
    }
    if n_fit == 1 {
        notices.add(&defects::TED_17, "a single voxel is left for the fit");
        if notices.reproduce() {
            return Err(PyError::new(
                "IndexError",
                "too many indices for array: array is 0-dimensional, but 1 were indexed",
            ));
        }
    }
    let mut maps = fit::loglin(
        &data,
        &count,
        &tes,
        used.volumes,
        ts,
        ImageSpec::lapack_for(o.libraries),
        n_threads,
        log,
        notices,
    )?;
    let cols = maps.cols;
    if o.fittype == FitType::Curvefit {
        // a voxel with no sample (every volume excluded or a dummy scan: TED-16, TED-13 in
        // `reproduce`): the bound of its smallest sample stops the run before its fit (§7.3)
        let t_used = used.volumes.map_or(n_t, <[usize]>::len);
        if !ts && t_used == 0 && n_fit > 0 {
            return Err(PyError::value(
                "zero-size array to reduction operation minimum which has no identity",
            ));
        }
        let curve = curvefit::run(
            &data,
            &count,
            &tes,
            used.volumes,
            &mut maps,
            notices.reproduce(),
            o.libraries,
        );
        curve_messages(&curve, &count, cols, log);
        // 16. the failures; with --verbose the variances
        curve_images(
            &curve,
            &count,
            cols,
            ts,
            o.verbose,
            &spec,
            &mut outputs,
            notices,
            log,
        )?;
    }
    // 17. floors, limited maps, cap
    let fin = maps::finish(maps, &count, &tes, log, notices);
    // numpy's cast of a float64 image to float32 (§11.3): finite values beyond its range
    // overflow, with a warning per writing place
    let cast = |log: &mut Log<'_>, v: &[f64], scale: f64| {
        if v.iter().any(|&x| {
            x.is_finite() && ((if scale == 1.0 { x } else { x / scale }) as f32).is_infinite()
        }) {
            log.py_warning(
                "output.rs:cast",
                "RuntimeWarning",
                "overflow encountered in cast",
            );
        }
    };
    let as32 = |v: &[f64], scale: f64| -> Vec<f32> {
        v.iter()
            .map(|&x| {
                if scale == 1.0 {
                    x as f32
                } else {
                    (x / scale) as f32
                }
            })
            .collect()
    };
    cast(log, &fin.s0_full, 1.0);
    outputs.image("s0 img", &spec, &as32(&fin.s0_full, 1.0), cols, ts)?;
    // 18. the RMSE and the table
    log.info(
        "t2smap:t2smap_workflow:505",
        "Calculating model fit quality metrics",
    );
    if n_t == 0 {
        // every volume a dummy scan (TED-13 in `reproduce`): tedana's table of no row stops the
        // run before the RMSE image (observed, with two and three echoes)
        return Err(PyError::value(
            "Shape of passed values is (0, 3), indices imply (0, 9)",
        ));
    }
    if rmse::divides_by_zero(&count, &fin) {
        log.py_warning(
            "rmse.rs:40",
            "RuntimeWarning",
            "divide by zero encountered in divide",
        );
    }
    let r = rmse::series(&data, &count, &fin, &tes, notices.reproduce());
    let rmap = rmse::map(&r, data.n_vox, n_t);
    // its voxels without a value are numpy's `nan` (§9 step 2, observed)
    outputs.image_nan("rmse img", &spec, &rmap, 1, false, output::NUMPY_NAN32)?;
    let table = rmse::table(&r, data.n_vox, n_t, log);
    drop(r);
    outputs.table(&rmse::COLUMNS, &table.rows, table.percentiles_f32)?;
    // Python's warnings registry is reset by now: a warning shown before shows again (observed:
    // the image cast's overflow of the S0 maps, the adaptive mask's means in PAID's)
    log.reset_registry();
    cast(log, &fin.s0_limited, 1.0);
    outputs.image(
        "limited s0 img",
        &spec,
        &as32(&fin.s0_limited, 1.0),
        cols,
        ts,
    )?;
    glibc07_notice(&fin, &count, &outputs, notices);
    cast(log, &fin.t2s_limited, 1000.0);
    outputs.image(
        "limited t2star img",
        &spec,
        &as32(&fin.t2s_limited, 1000.0),
        cols,
        ts,
    )?;
    // 19. the combination
    log.info(
        "t2smap:t2smap_workflow:521",
        "Computing optimal combination",
    );
    match o.combmode {
        CombMode::Paid => {
            log.warning(
                "combine:make_optcom:181",
                "Argument 't2s' is not required if 'combmode' is 'paid'. 't2s' array will not \
                 be used.",
            );
            log.info(
                "combine:make_optcom:187",
                "Optimally combining data with parallel-acquired inhomogeneity desensitized \
                 (PAID) method",
            );
            let (reduce, subtract) = combine::paid_warnings(&data, &count);
            if reduce {
                for place in [adaptive::MEAN_SUM, adaptive::VAR_SUM] {
                    log.py_warning(
                        place,
                        "RuntimeWarning",
                        "invalid value encountered in reduce",
                    );
                }
            }
            if subtract {
                log.py_warning(
                    adaptive::VAR_SUBTRACT,
                    "RuntimeWarning",
                    "invalid value encountered in subtract",
                );
            }
        }
        CombMode::T2s => log.info(
            "combine:make_optcom:202",
            if ts {
                "Optimally combining data with voxel- and volume-wise T2* estimates"
            } else {
                "Optimally combining data with voxel-wise T2* estimates"
            },
        ),
    }
    let combined = combine::combine(&data, &count, &fin, &tes, o.combmode, notices)?;
    cast(log, &fin.t2s_full, 1000.0);
    outputs.image("t2star img", &spec, &as32(&fin.t2s_full, 1000.0), cols, ts)?;
    outputs.image("combined img", &spec, &combined, n_t, true)?;
    // 20. the JSON files
    outputs.json(
        "data description json",
        &output::dataset_description(&command, o.libraries),
    )?;
    outputs.registry_json()?;
    // 21.
    log.info("t2smap:t2smap_workflow:566", "Workflow completed");
    log.info("utils:log_newsletter_info:812", NEWSLETTER);
    log.info(
        "utils:log_newsletter_info:816",
        "https://groups.google.com/g/tedana-newsletter",
    );
    Ok(Outcome {
        out_dir,
        files: outputs.registry.clone(),
    })
}

/// Step 7: the echo times' units and errors, TED-10, TED-12 (§3, §15.1).
fn echo_times(
    given: &[f64],
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Result<Vec<f64>, PyError> {
    let tes: Vec<f64> = if given.iter().all(|&t| t > 0.0 && t < 1.0) {
        log.info(
            "utils:check_te_values:792",
            "TE values appear to be in seconds. Converting to milliseconds for internal use.",
        );
        given.iter().map(|&t| t * 1000.0).collect()
    } else if given.iter().all(|&t| t >= 1.0) {
        log.warning(
            "utils:check_te_values:796",
            "TE values appear to be in milliseconds. Per BIDS convention, echo times should be \
             provided in seconds. Support for millisecond TE values is deprecated and will be \
             removed in a future version. Please provide TE values in seconds.",
        );
        given.to_vec()
    } else {
        return Err(PyError::value(
            "TE values must be positive and either all in seconds (values < 1, preferred per \
             BIDS convention) or all in milliseconds (values >= 1, deprecated).",
        ));
    };
    if tes.iter().any(|t| t.is_infinite()) {
        notices.add(&defects::TED_10, "an infinite echo time");
        if notices.fix() {
            return Err(PyError::value(
                "TE values must be finite (tedana fails later in numpy's least squares)",
            ));
        }
    }
    if tes.len() == 1 {
        notices.add(&defects::TED_12, "a single echo time");
        if notices.fix() {
            return Err(PyError::value(
                "t2smap needs two or more echoes (one echo time given; tedana fails later with \
                 an unrelated numpy error)",
            ));
        }
    }
    Ok(tes)
}

/// Removes the first `n` volumes of every echo (step 12), in place.
fn drop_first_volumes(data: &mut input::Data, n: usize) {
    let (t_old, rows) = (data.n_t, data.n_vox * data.n_echo);
    let t_new = t_old - n;
    for r in 0..rows {
        data.x
            .copy_within(r * t_old + n..(r + 1) * t_old, r * t_new);
    }
    data.x.truncate(rows * t_new);
    data.x.shrink_to_fit();
    data.n_t = t_new;
}

/// Step 15's curve-fit messages (§7.3, §12.3), per volume (in `ts`) and group in tedana's
/// order: SciPy's `OptimizeWarning` when a fit of the group warned (Python prints it once), then
/// with `--debug` the group's failure count.
fn curve_messages(curve: &curvefit::Curve, count: &[u8], cols: usize, log: &mut Log<'_>) {
    let groups = fit::groups(count);
    for t in 0..cols {
        for (n, members) in &groups {
            if members.iter().any(|&v| curve.warn[v * cols + t]) {
                log.py_warning(
                    "curvefit.rs:run",
                    "OptimizeWarning",
                    "Covariance of the parameters could not be estimated",
                );
            }
            if log.debugging() {
                let f = members
                    .iter()
                    .filter(|&&v| curve.failed[v * cols + t] == 1)
                    .count();
                if f > 0 {
                    let big_n = members.len();
                    log.debug(
                        "decay:fit_monoexponential:261",
                        &format!(
                            "With {n} echoes, monoexponential fit failed on {f}/{big_n} ({}%) \
                             voxel(s), used log linear estimate instead",
                            larmorx_core::python::number::format_fixed(
                                100.0 * f as f64 / big_n as f64,
                                2
                            )
                        ),
                    );
                }
            }
        }
    }
}

/// Step 16: the failures image and, with `--verbose`, the variance and covariance images
/// (§7.3, §11.1), with TED-07 (each variance in its own map by default) and TED-08 (the
/// count-1 voxels' failures and variances written by default).
#[allow(clippy::too_many_arguments)]
fn curve_images(
    curve: &curvefit::Curve,
    count: &[u8],
    cols: usize,
    ts: bool,
    verbose: bool,
    spec: &ImageSpec,
    outputs: &mut Outputs,
    notices: &mut Notices,
    log: &mut Log<'_>,
) -> Result<(), PyError> {
    let fix = notices.fix();
    let one = |k: usize| count[k / cols] == 1;
    let k_failed_one = (0..curve.failed.len())
        .filter(|&k| one(k) && curve.failed[k] == 1)
        .count();
    let failures: Vec<u8> = (0..curve.failed.len())
        .map(|k| if one(k) && !fix { 0 } else { curve.failed[k] })
        .collect();
    let f32_of = |x: f64| x as f32;
    // the count-1 fits that converged (§15.1: their variances are written by default)
    let k_var_one = if verbose {
        (0..curve.failed.len())
            .filter(|&k| one(k) && curve.failed[k] == 0)
            .count()
    } else {
        0
    };
    // one notice per defect and run (§15, the replica's form, observed): the failures where
    // there are some (summed over the volumes in `ts`), else the variances
    if k_failed_one > 0 {
        notices.add(
            &defects::TED_08,
            format!(
                "{k_failed_one} failed fit(s) of voxels with one good echo marked in the \
                 failures map"
            ),
        );
    } else if k_var_one > 0 {
        notices.add(
            &defects::TED_08,
            format!(
                "the variances of {k_var_one} voxel(s) with one good echo kept in the variance \
                 maps"
            ),
        );
    }
    outputs.image("fit failures img", spec, &failures, cols, ts)?;
    if verbose {
        let swaps = (0..curve.pcov.len()).any(|k| {
            !one(k) && f32_of(curve.pcov[k][0]).to_bits() != f32_of(curve.pcov[k][1]).to_bits()
        });
        if swaps {
            notices.add(
                &defects::TED_07,
                "the T2* and S0 variance maps hold their own variances",
            );
        }
        // tedana writes P[0, 0] (S0's variance) as the T2* variance and P[1, 1] as S0's (TED-07)
        let (t2s_index, s0_index) = if fix { (1, 0) } else { (0, 1) };
        let map = |i: usize| -> Vec<f32> {
            (0..curve.pcov.len())
                .map(|k| {
                    if one(k) && !fix {
                        0.0
                    } else {
                        f32_of(curve.pcov[k][i])
                    }
                })
                .collect()
        };
        // numpy's cast to float32: finite variances beyond its range overflow (§11.3)
        let mut cast = |i: usize| {
            if (0..curve.pcov.len()).any(|k| {
                let x = curve.pcov[k][i];
                !(one(k) && !fix) && x.is_finite() && f32_of(x).is_infinite()
            }) {
                log.py_warning(
                    "output.rs:cast",
                    "RuntimeWarning",
                    "overflow encountered in cast",
                );
            }
        };
        cast(t2s_index);
        outputs.image("t2star variance img", spec, &map(t2s_index), cols, ts)?;
        cast(s0_index);
        outputs.image("s0 variance img", spec, &map(s0_index), cols, ts)?;
        cast(2);
        outputs.image("t2star-s0 covariance img", spec, &map(2), cols, ts)?;
    }
    Ok(())
}

/// GLIBC-07 by §15.2's one-pass rule: the written log-linear S0 values the other exponential
/// would change.
fn glibc07_notice(fin: &maps::Final, count: &[u8], outputs: &Outputs, notices: &mut Notices) {
    let (k1, k2) = maps::glibc07_counts(fin, count);
    if k1 + k2 == 0 {
        return;
    }
    let mut files = Vec::new();
    if k1 > 0 {
        files.push(format!("{} {k1}", outputs.name("s0 img")));
    }
    if k2 > 0 {
        files.push(format!("{} {k2}", outputs.name("limited s0 img")));
    }
    let k = k1 + k2;
    let detail = if notices.fix() {
        format!(
            "the log-linear S0 = exp(b0) is written with the correctly rounded exp; glibc \
             2.35's (tedana's) would write {k} other value(s) ({})",
            files.join(", ")
        )
    } else {
        format!(
            "the log-linear S0 = exp(b0) is written with glibc 2.35's exp, as numpy computes it \
             for tedana; the correctly rounded exp would write {k} other value(s) ({})",
            files.join(", ")
        )
    };
    notices.add(&defects::GLIBC_07, detail);
}
