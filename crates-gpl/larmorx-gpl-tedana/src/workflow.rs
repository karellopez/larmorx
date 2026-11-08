// SPDX-License-Identifier: GPL-3.0-only
// Translated from tedana 26.0.3 (Copyright 2023, The ME-ICA Developers; GNU LGPL 2.1), under the
// GNU GPL version 3 as section 3 of the LGPL 2.1 allows; modified 2026 by Karel Lopez Vilaret.
//! `tedana.workflows.t2smap`: `_main` (the command line) and `t2smap_workflow`, step by step in
//! tedana's order, so that messages, files and failures come in the same order.
//!
//! [`prepare_and_fit`] runs the workflow up to the T2*/S0 fit (the output directory, the call
//! script, the echo times, the reference image, the figures directory, the mask, the data, the
//! dummy scans and excluded volumes, the adaptive mask, the fit). The maps' floors and caps, the
//! RMSE and its table, the optimal combination and the files are the next step's.

use std::io::Write;
use std::path::{Component, Path, PathBuf};

use larmorx_core::defects::{
    Notices, TED_02, TED_03, TED_04, TED_07, TED_08, TED_10, TED_12, TED_13, UpstreamDefects,
};
use larmorx_core::numpy::NpFloat;
use larmorx_core::python::argparse::{formatter_width, py_repr_str};
use larmorx_core::python::json::Value;
use larmorx_core::python::number::parse_int;

use crate::adaptive::make_adaptive_mask;
use crate::combine::{CombMode, make_optcom};
use crate::data::{self, LoadPath, Masked, RefImage, UserMask};
use crate::decay::{Context, Fit, Libraries, fit_decay, fit_decay_ts};
use crate::log::{Log, PyError, Site};
use crate::maps::{modify_t2s_s0_maps, rmse_of_fit_decay_ts, unmask};
use crate::outputs::{ImgData, Outputs, confounds_tsv, uname};
use crate::parser::{self, FitType, Settings};

/// What the program reads from its environment.
#[derive(Clone, Debug, Default)]
pub struct Env {
    /// `COLUMNS` (argparse's width).
    pub columns: Option<String>,
    /// The terminal's width when standard output is one.
    pub terminal_columns: Option<usize>,
    /// `LARMORX_UPSTREAM_DEFECTS`.
    pub mode: UpstreamDefects,
    /// `LARMORX_TEDANA_LIBRARIES` (`newest`, the default, or `shared`).
    pub libraries: Libraries,
}

/// Python's `os.path.abspath`: joined to the working directory and normalised (`.`, `..` and
/// repeated separators removed lexically).
pub fn py_abspath(p: &str) -> String {
    let base = if Path::new(p).is_absolute() {
        PathBuf::from(p)
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(p)
    };
    let mut parts: Vec<String> = Vec::new();
    for c in base.components() {
        match c {
            Component::ParentDir => {
                parts.pop();
            }
            Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            _ => {}
        }
    }
    format!("/{}", parts.join("/"))
}

/// An `OSError` as Python prints it: `[Errno N] <strerror>: '<path>'`, with its subclass.
fn os_error(e: &std::io::Error, path: &str, place: &str) -> PyError {
    let errno = e.raw_os_error().unwrap_or(0);
    let class = match errno {
        2 => "FileNotFoundError",
        13 => "PermissionError",
        17 => "FileExistsError",
        20 => "NotADirectoryError",
        21 => "IsADirectoryError",
        _ => "OSError",
    };
    let text = e.to_string();
    let strerror = text.split(" (os error").next().unwrap_or(&text).to_owned();
    PyError::new(
        class,
        format!("[Errno {errno}] {strerror}: {}", py_repr_str(path)),
        place,
    )
}

/// `tedana.utils.parse_volume_indices(indices_str)`.
pub fn parse_volume_indices(s: Option<&str>) -> Result<Vec<i128>, PyError> {
    let Some(s) = s.filter(|s| !s.is_empty()) else {
        return Ok(Vec::new());
    };
    let place = "tedana/utils.py, in parse_volume_indices";
    let int = |t: &str| {
        parse_int(t).ok_or_else(|| {
            PyError::value_error(
                format!("invalid literal for int() with base 10: {}", py_repr_str(t)),
                place,
            )
        })
    };
    let mut out = std::collections::BTreeSet::new();
    for item in s.split(',') {
        let item = item.trim();
        if item.contains(':') {
            let parts: Vec<&str> = item.split(':').collect();
            if parts.len() > 2 {
                return Err(PyError::value_error(
                    format!("Invalid volume indices string ({s}). Step sizes are not supported."),
                    place,
                ));
            }
            if parts[0].is_empty() || parts[1].is_empty() {
                return Err(PyError::value_error(
                    format!(
                        "Invalid volume indices string ({s}). Open-ended ranges are not \
                         supported."
                    ),
                    place,
                ));
            }
            let (a, b) = (int(parts[0])?, int(parts[1])?);
            if a < 0 || b < 0 {
                return Err(PyError::value_error(
                    format!(
                        "Invalid volume indices string ({s}). Negative indices are not supported."
                    ),
                    place,
                ));
            }
            let mut i = a;
            while i < b {
                out.insert(i);
                i += 1;
            }
        } else {
            let i = int(item)?;
            if i < 0 {
                return Err(PyError::value_error(
                    format!(
                        "Invalid volume indices string ({s}). Negative indices are not supported."
                    ),
                    place,
                ));
            }
            out.insert(i);
        }
    }
    Ok(out.into_iter().collect())
}

/// `_infer_prefix(prefix)`.
fn infer_prefix(p: &str) -> String {
    if !p.is_empty() && !p.ends_with('_') {
        format!("{p}_")
    } else {
        p.to_owned()
    }
}

fn py_list_ints(v: &[i128]) -> String {
    format!(
        "[{}]",
        v.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// The workflow's state after the fit.
pub struct Stages {
    /// `t2smap_command` (`"t2smap " + " ".join(argv)`).
    pub command: String,
    /// The output generator, with the adaptive mask saved.
    pub outputs: Outputs,
    pub settings: Settings,
    pub out_dir: String,
    pub prefix: String,
    /// Echo times in milliseconds.
    pub tes: Vec<f64>,
    pub reference: RefImage,
    pub mask: UserMask,
    pub load_path: LoadPath,
    /// `data_cat` after the dummy scans (all volumes kept for the combination).
    pub masked: Masked,
    /// The volumes the fit used (`use_volumes`), over `masked`'s volumes.
    pub use_volumes: Vec<bool>,
    /// `masksum_denoise` over the mask's voxels.
    pub adaptive: Vec<i64>,
    /// `mask_denoise`.
    pub denoise: Vec<bool>,
    pub fit: Fit,
}

/// `t2smap_workflow(**kwargs)` up to the fit. `argv` is the command line after the program
/// name (for `t2smap_call.sh`).
pub fn prepare_and_fit(
    settings: Settings,
    argv: &[String],
    env: &Env,
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Result<Stages, PyError> {
    let wf = "tedana/workflows/t2smap.py, in t2smap_workflow";
    let out_dir = py_abspath(&settings.out_dir);
    if !Path::new(&out_dir).is_dir() {
        std::fs::create_dir(&out_dir).map_err(|e| os_error(&e, &out_dir, wf))?;
    }
    let mut exclude = parse_volume_indices(settings.exclude.as_deref())?;
    log.setup(settings.quiet, settings.debug);
    log.info(
        Site("t2smap", "t2smap_workflow", 365),
        &format!("Using output directory: {out_dir}"),
    );
    let call = format!("t2smap {}", argv.join(" "));
    let call_path = format!("{out_dir}/t2smap_call.sh");
    std::fs::write(&call_path, call.as_bytes()).map_err(|e| os_error(&e, &call_path, wf))?;
    if settings.fit_ts && !exclude.is_empty() {
        return Err(PyError::value_error(
            "Excluding volumes is not supported for fitmode='ts'. Please set fitmode='all' or \
             remove the exclude argument.",
            wf,
        ));
    }

    // check_te_values
    let te_place = "tedana/utils.py, in check_te_values";
    let tes: Vec<f64> = if settings.tes.iter().all(|&t| t > 0.0 && t < 1.0) {
        log.info(
            Site("utils", "check_te_values", 792),
            "TE values appear to be in seconds. Converting to milliseconds for internal use.",
        );
        settings.tes.iter().map(|&t| t * 1000.0).collect()
    } else if settings.tes.iter().all(|&t| t >= 1.0) {
        log.warning(
            Site("utils", "check_te_values", 796),
            "TE values appear to be in milliseconds. Per BIDS convention, echo times should be \
             provided in seconds. Support for millisecond TE values is deprecated and will be \
             removed in a future version. Please provide TE values in seconds.",
        );
        settings.tes.clone()
    } else {
        return Err(PyError::value_error(
            "TE values must be positive and either all in seconds (values < 1, preferred per \
             BIDS convention) or all in milliseconds (values >= 1, deprecated).",
            te_place,
        ));
    };
    if tes.iter().any(|t| t.is_infinite()) {
        notices.add(&TED_10, "an infinite echo time");
        if notices.fix() {
            return Err(PyError::value_error(
                "TE values must be finite (tedana fails later in numpy's least squares)",
                te_place,
            ));
        }
    }
    let n_echos = tes.len();
    if n_echos == 1 {
        notices.add(&TED_12, "a single echo time");
        if notices.fix() {
            return Err(PyError::value_error(
                "t2smap needs two or more echoes (one echo time given; tedana fails later with \
                 an unrelated numpy error)",
                wf,
            ));
        }
    }

    let reference = data::load_ref(&settings.data, n_echos)?;
    // OutputGenerator(ref_img, convention, out_dir, prefix, ...)
    let prefix = infer_prefix(&settings.prefix);
    let figures = format!("{out_dir}/figures");
    if !Path::new(&figures).is_dir() {
        log.info(
            Site("io", "__init__", 164),
            &format!("Generating figures directory: {figures}"),
        );
        std::fs::create_dir(&figures)
            .map_err(|e| os_error(&e, &figures, "tedana/io.py, in __init__"))?;
    }
    if settings.overwrite {
        let name = if settings.convention == "orig" {
            "confounds"
        } else {
            "desc-confounds_timeseries"
        };
        let path = format!("{out_dir}/{prefix}{name}.tsv");
        if Path::new(&path).exists() {
            std::fs::remove_file(&path)
                .map_err(|e| os_error(&e, &path, "tedana/io.py, in __init__"))?;
        }
    }

    // load_mask
    let threads = threads(&settings);
    let user_mask = settings.mask.is_some();
    let mask = if let Some(mask_path) = settings.mask.clone() {
        log.info(Site("utils", "load_mask", 916), "Using user-defined mask");
        data::load_user_mask(&mask_path, threads)?
    } else {
        log.warning(
            Site("utils", "load_mask", 943),
            "Computing EPI mask from first echo using nilearn's compute_epi_mask function. Most \
             external pipelines include more reliable masking functions. It is strongly \
             recommended to provide an external mask, and to visually confirm that mask \
             accurately conforms to data boundaries.",
        );
        let ted03 = "give a mask with --mask (tedana fails later with an unrelated error)";
        match data::compute_mask(&settings.data, &reference, notices.reproduce(), threads) {
            Ok((m, empty)) => {
                if empty {
                    log.warn(
                        "tedana/utils.py",
                        949,
                        "MaskWarning",
                        "Computed an empty mask. Are you sure that input data are EPI images not \
                         detrended. ",
                        "mask_img = compute_epi_mask(ref_img)",
                    );
                    notices.add(&TED_03, "nilearn's compute_epi_mask found an empty mask");
                    if notices.fix() {
                        return Err(PyError::value_error(
                            format!(
                                "nilearn's compute_epi_mask found no voxels in the first echo; \
                                 {ted03}"
                            ),
                            "tedana/utils.py, in load_mask",
                        ));
                    }
                }
                m
            }
            Err(e) => {
                if e.class == "ValueError" && e.message.starts_with("attempt to get argmax") {
                    notices.add(
                        &TED_03,
                        "nilearn's compute_epi_mask has too few voxels to find a threshold",
                    );
                    if notices.fix() {
                        return Err(PyError::value_error(
                            format!(
                                "the first echo has too few voxels for nilearn's \
                                 compute_epi_mask to find a threshold; {ted03}"
                            ),
                            "tedana/utils.py, in load_mask",
                        ));
                    }
                }
                return Err(e);
            }
        }
    };

    log.info(
        Site("t2smap", "t2smap_workflow", 414),
        &format!(
            "Loading input data: [{}]",
            settings
                .data
                .iter()
                .map(|f| py_repr_str(f))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    );
    if settings.data.len() == 1 {
        log.warning(
            Site("io", "load_data_nilearn", 1104),
            "DEPRECATION WARNING: We are planning to deprecate supplying a single z-concatenated \
             data file at the end of 2026. If you are using this option, please comment on \
             https://github.com/ME-ICA/tedana/issues/1313.",
        );
    }
    // TED-04: the mask's affine against the data's (tedana does not compare them).
    let first = data::load_header(&settings.data[0])?;
    if user_mask && !affines_close(&mask.header.affine, &first.affine) {
        notices.add(&TED_04, "the mask's affine differs from the data's");
        if notices.fix() {
            return Err(PyError::value_error(
                format!(
                    "Mask affine:\n{}\n is different from img affine:\n{}",
                    affine_str(&mask.header.affine),
                    affine_str(&first.affine)
                ),
                "tedana/utils.py, in load_mask",
            ));
        }
    }
    let (mut masked, load_path) = data::load_data(&settings.data, &mask, n_echos, threads)?;

    // Dummy scans.
    let d = settings.dummy_scans;
    if d < 0 || (d > 0 && d >= masked.n_vols as i128) {
        notices.add(
            &TED_13,
            format!("--dummy-scans {d} of {} volumes", masked.n_vols),
        );
        if notices.fix() {
            return Err(PyError::value_error(
                format!(
                    "--dummy-scans must be at least 0 and below the number of volumes ({}); {d} \
                     given",
                    masked.n_vols
                ),
                wf,
            ));
        }
    }
    if d > 0 {
        log.warning(
            Site("t2smap", "t2smap_workflow", 418),
            &format!("Removing the first {d} volumes as dummy scans."),
        );
        let drop = (d as usize).min(masked.n_vols);
        let t = masked.n_vols;
        let keep = t - drop;
        let mut out = Vec::with_capacity(masked.data.len() / t.max(1) * keep);
        for row in masked.data.chunks(t.max(1)) {
            out.extend_from_slice(&row[drop..]);
        }
        masked.data = out;
        masked.n_vols = keep;
    }
    let (n_samp, n_vols) = (masked.voxels.len(), masked.n_vols);
    log.debug(Site("t2smap", "t2smap_workflow", 422), || {
        format!(
            "Resulting data shape: ({n_samp}, {}, {n_vols})",
            masked.n_echos
        )
    });
    if !exclude.is_empty() {
        log.info(
            Site("t2smap", "t2smap_workflow", 426),
            &format!("Excluding volumes: {}", py_list_ints(&exclude)),
        );
        // setdiff1d(exclude, arange(dummy)) - dummy
        exclude = exclude
            .into_iter()
            .filter(|&i| !(0..d.max(0)).contains(&i))
            .map(|i| i - d)
            .collect();
        exclude.sort_unstable();
        exclude.dedup();
        if exclude.is_empty() {
            log.warning(
                Site("t2smap", "t2smap_workflow", 433),
                &format!("All exclude indices overlap with dummy scans ({d})."),
            );
        }
    }
    let mut use_volumes = vec![true; n_vols];
    if let Some(&max) = exclude.last() {
        let nv = n_vols as i128;
        if max > nv {
            return Err(PyError::value_error(
                format!(
                    "The maximum exclude index ({max}) is greater than the number of timepoints \
                     in the data ({n_vols})."
                ),
                wf,
            ));
        }
        if max == nv {
            notices.add(
                &TED_02,
                format!("--exclude index {max} of {n_vols} volumes"),
            );
            return Err(if notices.fix() {
                PyError::value_error(
                    format!(
                        "The maximum exclude index ({max}) is not below the number of \
                         timepoints in the data ({n_vols})."
                    ),
                    wf,
                )
            } else {
                PyError::new(
                    "IndexError",
                    format!("index {max} is out of bounds for axis 0 with size {n_vols}"),
                    wf,
                )
            });
        }
        log.info(
            Site("t2smap", "t2smap_workflow", 441),
            &format!(
                "Excluding {} volumes from adaptive mask and T2*/S0 estimation",
                exclude.len()
            ),
        );
        // Negative indices (a negative --dummy-scans shifts them, TED-13) count from the end.
        for &i in &exclude {
            let k = if i < 0 { i + nv } else { i };
            if k < 0 {
                return Err(PyError::new(
                    "IndexError",
                    format!("index {i} is out of bounds for axis 0 with size {n_vols}"),
                    wf,
                ));
            }
            use_volumes[k as usize] = false;
        }
    }
    let fit_vols: Vec<usize> = (0..n_vols).filter(|&t| use_volumes[t]).collect();
    let nf = fit_vols.len();
    let e = masked.n_echos;
    let mut fit_data = vec![0.0f32; n_samp * e * nf];
    for (k, block) in fit_data.chunks_mut(nf.max(1)).enumerate().take(n_samp * e) {
        let src = &masked.data[k * n_vols..(k + 1) * n_vols];
        for (slot, &t) in block.iter_mut().zip(&fit_vols) {
            *slot = src[t];
        }
    }

    let (denoise, adaptive) = make_adaptive_mask(
        &fit_data,
        n_samp,
        e,
        nf,
        settings.n_independent_echos,
        &settings.masktype,
        nf < n_vols,
        log,
        notices,
    )?;
    log.debug(Site("t2smap", "t2smap_workflow", 455), || {
        format!(
            "Retaining {}/{n_samp} samples for denoising",
            denoise.iter().filter(|&&b| b).count()
        )
    });
    let mut outputs = Outputs {
        out_dir: out_dir.clone(),
        prefix: prefix.clone(),
        convention: settings.convention.clone(),
        overwrite: settings.overwrite,
        shape: masked.spatial,
        affine: mask.header.affine,
        voxels: masked.voxels.clone(),
        ref_zooms: reference.zooms.clone(),
        release: env.libraries.lapack(),
        n_threads: threads,
        registry: Vec::new(),
    };
    let am32: Vec<i32> = adaptive.iter().map(|&a| a as i32).collect();
    outputs.save_img("adaptive mask img", ImgData::I32(&am32), None, None)?;
    drop(am32);
    log.info(Site("t2smap", "t2smap_workflow", 458), "Computing T2* map");
    let kept: Vec<usize> = (0..n_samp).filter(|&v| denoise[v]).collect();
    let mut dm = Vec::with_capacity(kept.len() * e * nf);
    for &v in &kept {
        dm.extend_from_slice(&fit_data[v * e * nf..(v + 1) * e * nf]);
    }
    drop(fit_data);
    let am: Vec<i64> = kept.iter().map(|&v| adaptive[v]).collect();
    if e != tes.len() {
        return Err(PyError::value_error(
            format!(
                "Second dimension of data ({e}) does not match number of echoes provided (tes; \
                 {})",
                tes.len()
            ),
            "tedana/decay.py, in fit_decay",
        ));
    }
    let ctx = Context {
        libraries: env.libraries,
        n_threads: threads,
    };
    let fit = if settings.fit_ts {
        fit_decay_ts(&dm, e, nf, &tes, &am, settings.fittype, &ctx, log, notices)?
    } else {
        fit_decay(&dm, e, nf, &tes, &am, settings.fittype, &ctx, log, notices)?
    };
    Ok(Stages {
        command: call,
        outputs,
        settings,
        out_dir,
        prefix,
        tes,
        reference,
        mask,
        load_path,
        masked,
        use_volumes,
        adaptive,
        denoise,
        fit,
    })
}

/// The libraries' versions of the environment whose results are reproduced, as tedana lists
/// them in `dataset_description.json` (`get_system_version_info`).
fn library_versions(libraries: Libraries) -> Vec<(String, Value)> {
    let (numpy, scipy) = match libraries {
        Libraries::Newest => ("2.5.4", "1.18.1"),
        Libraries::Shared => ("2.3.5", "1.15.3"),
    };
    [
        ("bokeh", "3.10.0"),
        ("matplotlib", "3.11.2"),
        ("mapca", "0.0.8"),
        ("nibabel", "5.4.2"),
        ("nilearn", "0.14.1"),
        ("numpy", numpy),
        ("pandas", "3.0.6"),
        ("robustica", "0.1.4"),
        ("scikit-learn", "1.9.1"),
        ("scipy", scipy),
        ("threadpoolctl", "3.7.0"),
        ("tqdm", "4.70.1"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), Value::str(v)))
    .collect()
}

fn to_f32(v: &[f64]) -> Vec<f32> {
    v.iter().map(|&x| f32::from_f64(x)).collect()
}

fn ms_to_s(v: &[f64]) -> Vec<f32> {
    v.iter().map(|&x| f32::from_f64(x.div(1000.0))).collect()
}

/// `t2smap_workflow` after the fit: the failures and variance maps, the floors and caps, the
/// maps, the RMSE and its table, the optimal combination, the JSON files, the last messages.
pub fn finish(
    st: Stages,
    env: &Env,
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Result<(), PyError> {
    let Stages {
        command,
        mut outputs,
        settings,
        tes,
        masked,
        adaptive,
        denoise,
        fit,
        ..
    } = st;
    let reproduce = notices.reproduce();
    let threads = threads(&settings);
    let (e, n_vols) = (masked.n_echos, masked.n_vols);
    // fit_decay_ts fits every volume (it refuses --exclude): one column per volume.
    let cols = if settings.fit_ts { n_vols } else { 1 };
    let volumes = settings.fit_ts.then_some(n_vols);
    let am_fit: Vec<i64> = adaptive
        .iter()
        .zip(&denoise)
        .filter(|&(_, &d)| d)
        .map(|(&a, _)| a)
        .collect();
    if settings.fittype == FitType::Curvefit {
        let failures = fit.failures.as_ref().expect("curvefit's failures");
        if fit.count1_failed > 0 {
            notices.add(
                &TED_08,
                format!(
                    "{} failed fit(s) of voxels with one good echo marked in the failures map",
                    fit.count1_failed
                ),
            );
        }
        let f8: Vec<u8> = failures.iter().map(|&b| u8::from(b)).collect();
        outputs.save_img(
            "fit failures img",
            ImgData::U8(&f8),
            volumes,
            Some(&denoise),
        )?;
        if settings.verbose {
            notices.add(
                &TED_07,
                "the T2* and S0 variance maps hold their own variances",
            );
            if fit.count1_fitted > 0 {
                notices.add(
                    &TED_08,
                    format!(
                        "the variances of {} voxel(s) with one good echo kept in the variance \
                         maps",
                        fit.count1_fitted
                    ),
                );
            }
            let t2v = fit.t2s_var.as_ref().expect("curvefit");
            let s0v = fit.s0_var.as_ref().expect("curvefit");
            let (first, second) = if reproduce { (s0v, t2v) } else { (t2v, s0v) };
            let cov = fit.covar.as_ref().expect("curvefit");
            outputs.save_img(
                "t2star variance img",
                ImgData::F32(&to_f32(first)),
                volumes,
                Some(&denoise),
            )?;
            outputs.save_img(
                "s0 variance img",
                ImgData::F32(&to_f32(second)),
                volumes,
                Some(&denoise),
            )?;
            outputs.save_img(
                "t2star-s0 covariance img",
                ImgData::F32(&to_f32(cov)),
                volumes,
                Some(&denoise),
            )?;
        }
    }
    let Fit { t2s, s0, .. } = fit;
    let maps = modify_t2s_s0_maps(t2s, s0, &am_fit, cols, &tes, log, notices);
    let t2s_full = unmask(&maps.t2s_full, &denoise, cols);
    let s0_full = unmask(&maps.s0_full, &denoise, cols);
    let t2s_limited = unmask(&maps.t2s_limited, &denoise, cols);
    let s0_limited = unmask(&maps.s0_limited, &denoise, cols);
    drop(maps);
    outputs.save_img("s0 img", ImgData::F32(&to_f32(&s0_full)), volumes, None)?;
    drop(s0_full);
    log.info(
        Site("t2smap", "t2smap_workflow", 505),
        "Calculating model fit quality metrics",
    );
    let rmse = rmse_of_fit_decay_ts(
        &masked.data,
        e,
        n_vols,
        &tes,
        &adaptive,
        &t2s_limited,
        &s0_limited,
        cols,
        reproduce,
        threads,
        log,
    );
    outputs.save_img("rmse img", ImgData::F32(&rmse.map), None, None)?;
    outputs.save_tsv("confounds tsv", &confounds_tsv(&rmse))?;
    drop(rmse);
    outputs.save_img(
        "limited s0 img",
        ImgData::F32(&to_f32(&s0_limited)),
        volumes,
        None,
    )?;
    drop(s0_limited);
    outputs.save_img(
        "limited t2star img",
        ImgData::F32(&ms_to_s(&t2s_limited)),
        volumes,
        None,
    )?;
    drop(t2s_limited);
    log.info(
        Site("t2smap", "t2smap_workflow", 521),
        "Computing optimal combination",
    );
    let mode = if settings.paid {
        CombMode::Paid
    } else {
        CombMode::T2s
    };
    let combined = make_optcom(
        &masked.data,
        e,
        n_vols,
        &tes,
        &adaptive,
        &t2s_full,
        cols,
        mode,
        threads,
        log,
        notices,
    )?;
    drop(masked);
    outputs.save_img(
        "t2star img",
        ImgData::F32(&ms_to_s(&t2s_full)),
        volumes,
        None,
    )?;
    drop(t2s_full);
    outputs.save_img("combined img", ImgData::F32(&combined), Some(n_vols), None)?;
    drop(combined);

    let [system, node, release, version, machine, processor] = uname();
    fn s(v: impl Into<String>) -> Value {
        Value::str(v)
    }
    let generated = Value::Dict(vec![
        ("Name".into(), s("t2smap")),
        ("Version".into(), s(crate::TEDANA_VERSION)),
        (
            "Description".into(),
            s(
                "A pipeline estimating T2* from multi-echo fMRI data and combining data across \
               echoes.",
            ),
        ),
        ("CodeURL".into(), s("https://github.com/ME-ICA/tedana")),
        (
            "Node".into(),
            Value::Dict(vec![
                ("Name".into(), s(node)),
                ("System".into(), s(system)),
                ("Machine".into(), s(machine)),
                ("Processor".into(), s(processor)),
                ("Release".into(), s(release)),
                ("Version".into(), s(version)),
            ]),
        ),
        (
            "Python".into(),
            s(format!(
                "larmorx-gpl {} (tedana {}'s t2smap translated to Rust, with the results of \
                 Python 3.12.16 and the libraries listed)",
                env!("CARGO_PKG_VERSION"),
                crate::TEDANA_VERSION
            )),
        ),
        (
            "Python_Libraries".into(),
            Value::Dict(library_versions(env.libraries)),
        ),
        ("Command".into(), s(command)),
    ]);
    let description = Value::Dict(vec![
        ("Name".into(), s("t2smap Outputs")),
        ("BIDSVersion".into(), s("1.5.0")),
        ("DatasetType".into(), s("derivative")),
        ("GeneratedBy".into(), Value::List(vec![generated])),
    ]);
    outputs.save_json("data description json", &description)?;
    outputs.save_self()?;
    log.info(Site("t2smap", "t2smap_workflow", 566), "Workflow completed");
    log.info(
        Site("utils", "log_newsletter_info", 812),
        "Don't forget to subscribe to the tedana newsletter for updates! This is a very low \
         volume email list.",
    );
    log.info(
        Site("utils", "log_newsletter_info", 816),
        "https://groups.google.com/g/tedana-newsletter",
    );
    log.teardown();
    Ok(())
}

fn affines_close(a: &larmorx_core::Affine, b: &larmorx_core::Affine) -> bool {
    a.rows()
        .iter()
        .flatten()
        .zip(b.rows().iter().flatten())
        .all(|(&x, &y)| (x - y).abs() <= 1e-8 + 1e-5 * y.abs())
}

fn affine_str(a: &larmorx_core::Affine) -> String {
    let v: Vec<f64> = a.rows().iter().flatten().copied().collect();
    larmorx_core::numpy::format::array_str_2d(&v, 4)
}

/// The threads of the fits: `--n-threads` (tedana's joblib workers; 0 or less: all logical
/// CPUs). The results do not depend on it.
fn threads(s: &Settings) -> usize {
    if s.n_threads <= 0 {
        0
    } else {
        s.n_threads.min(1024) as usize
    }
}

/// The `t2smap` program: `args` after the program name; returns the exit status.
pub fn t2smap(args: &[String], env: &Env, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let width = formatter_width(env.columns.as_deref(), env.terminal_columns);
    let settings = match parser::parse(args, width) {
        Ok(s) => s,
        Err(exit) => {
            let _ = out.write_all(exit.stdout.as_bytes());
            let _ = err.write_all(exit.stderr.as_bytes());
            return exit.status as u8;
        }
    };
    let mut notices = Notices::new(env.mode);
    let mut log = Log::new(err);
    let result = prepare_and_fit(settings, args, env, &mut log, &mut notices)
        .and_then(|st| finish(st, env, &mut log, &mut notices));
    let status = match result {
        Ok(()) => 0,
        Err(e) => {
            let _ = log.err.write_all(e.traceback().as_bytes());
            1
        }
    };
    for n in notices.into_vec() {
        let _ = writeln!(log.err, "larmorx-gpl: {}", n.line());
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_indices_as_tedana() {
        assert_eq!(parse_volume_indices(None).unwrap(), Vec::<i128>::new());
        assert_eq!(parse_volume_indices(Some("")).unwrap(), Vec::<i128>::new());
        assert_eq!(
            parse_volume_indices(Some("0, 5:8,3")).unwrap(),
            vec![0, 3, 5, 6, 7]
        );
        let e = parse_volume_indices(Some("0:5:2")).unwrap_err();
        assert!(e.message.ends_with("Step sizes are not supported."));
        let e = parse_volume_indices(Some(":5")).unwrap_err();
        assert!(e.message.ends_with("Open-ended ranges are not supported."));
        let e = parse_volume_indices(Some("-1")).unwrap_err();
        assert!(e.message.ends_with("Negative indices are not supported."));
        let e = parse_volume_indices(Some("1,,2")).unwrap_err();
        assert_eq!(e.message, "invalid literal for int() with base 10: ''");
    }

    #[test]
    fn abspath_normalises() {
        assert_eq!(py_abspath("/a/./b//c/../d"), "/a/b/d");
        assert_eq!(py_abspath("/"), "/");
        assert_eq!(infer_prefix("sub"), "sub_");
        assert_eq!(infer_prefix("sub_"), "sub_");
        assert_eq!(infer_prefix(""), "");
    }
}
