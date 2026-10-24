// SPDX-License-Identifier: Apache-2.0
//! Bindings for `larmorx_afni` (`lx.afni`).
//!
//! Images cross as numpy arrays in AFNI's storage types (`uint8`, `int16`, `float32`) and
//! header dicts; the GIL is released while files are read and series are shifted.

use std::path::PathBuf;

use larmorx_afni::automask::{self, AutomaskOptions};
use larmorx_afni::cfmt::g32;
use larmorx_afni::dataset::{
    self, AfniImage, BrickData, Bricks, InterceptPlan, KeptIntercept, OutputTiming, ReadError,
};
use larmorx_afni::geometry::{self, InputGeometry};
use larmorx_afni::input;
use larmorx_afni::msg::Message;
use larmorx_afni::settings::AfniSettings;
use larmorx_afni::timing::{Pattern, mean_time};
use larmorx_afni::tshift::{Method, Restore, TshiftParams, tshift};
use numpy::ndarray::{ArrayD, IxDyn, ShapeBuilder};
use numpy::{PyArray, PyArrayDyn, PyArrayMethods};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

use crate::defects::{PyNotice, mode_from_py as defects_mode, notices_to_py};
use crate::nifti::{header_from_dict, header_to_dict, to_py_err};
use larmorx_core::defects::{Notice, UpstreamDefects};

fn value_err(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}

/// The shift parameters as Python gives them.
struct Request {
    pattern: Option<String>,
    times: Option<Vec<f64>>,
    tr: Option<f64>,
    tzero: Option<f64>,
    slice: Option<usize>,
    ignore: usize,
    method: String,
    restore: String,
    detrend: bool,
    n_threads: usize,
    upstream_defects: UpstreamDefects,
    /// AFNI's variables (`afni_settings`), with the ones `lx.afni.tshift` sets itself.
    settings: AfniSettings,
}

/// AFNI's settings of a Python call: `afni_settings` (checked by the Python wrapper) and the
/// ones the wrapper fixes for its own use (a path is one file; plain messages; no report on
/// oblique inputs), as the replica program receives them (`larmorx/afni/tshift.py`).
fn settings_of(afni_settings: Option<Vec<(String, String)>>) -> AfniSettings {
    let mut settings = AfniSettings::from_pairs(afni_settings.unwrap_or_default());
    for (name, value) in [
        ("AFNI_PATH_SPACES_OK", "YES"),
        ("AFNI_MESSAGE_COLORIZE", "NO"),
        ("AFNI_NO_OBLIQUE_WARNING", "YES"),
    ] {
        settings.set(name, value);
    }
    settings
}

/// The texts of AFNI's warnings among `messages` (the Python wrapper raises them).
fn warning_texts(messages: Vec<Message>) -> impl Iterator<Item = String> {
    messages.into_iter().filter_map(|m| match m {
        Message::Warning(t) => Some(t),
        _ => None,
    })
}

impl Request {
    /// The [`TshiftParams`] for a dataset of `shape` whose header gives `header_tr`, checked
    /// as `3dTshift` checks its options.
    fn params(&self, shape: [usize; 4], header_tr: f32) -> PyResult<TshiftParams> {
        let [_, _, nz, nt] = shape;
        let tr = self.tr.map_or(header_tr, |t| t as f32);
        if tr.is_nan() || tr <= 0.0 || tr.is_infinite() {
            return Err(PyValueError::new_err(format!(
                "the TR must be positive, not {tr}"
            )));
        }
        let slice_times = match (&self.pattern, &self.times) {
            (Some(name), _) => Pattern::from_name(name)
                .ok_or_else(|| PyValueError::new_err(format!("unknown slice pattern {name:?}")))?
                .times(nz, tr),
            (None, Some(times)) => {
                if times.len() != nz {
                    return Err(PyValueError::new_err(format!(
                        "{} slice times for {nz} slices",
                        times.len()
                    )));
                }
                // Parsed as doubles, then stored as float32, as 3dTshift does.
                let times: Vec<f32> = times.iter().map(|&t| t as f32).collect();
                if let Some(bad) = times.iter().find(|&&t| !(0.0..=tr).contains(&t)) {
                    return Err(PyValueError::new_err(format!(
                        "slice time {bad} is outside 0..TR ({tr})"
                    )));
                }
                times
            }
            (None, None) => return Err(PyValueError::new_err("slice_times is required")),
        };
        let tzero = match (self.slice, self.tzero) {
            (Some(k), _) => *slice_times.get(k).ok_or_else(|| {
                PyValueError::new_err(format!("slice {k} is out of range for {nz} slices"))
            })?,
            (None, Some(z)) if z < 0.0 || !z.is_finite() => {
                return Err(PyValueError::new_err(format!(
                    "tzero must be a non-negative number, not {z}"
                )));
            }
            (None, Some(z)) => z as f32,
            (None, None) => mean_time(&slice_times),
        };
        let method = Method::from_name(&self.method)
            .ok_or_else(|| PyValueError::new_err(format!("unknown method {:?}", self.method)))?;
        let restore = match self.restore.as_str() {
            "trend" => Restore::Trend,
            "none" => Restore::None,
            "intercept" => Restore::Intercept,
            other => {
                return Err(PyValueError::new_err(format!(
                    "restore must be 'trend', 'none' or 'intercept', not {other:?}"
                )));
            }
        };
        if nt >= 2 && self.ignore + 5 > nt {
            return Err(PyValueError::new_err(format!(
                "ignore={} is too large for {nt} time points (at most nt - 5)",
                self.ignore
            )));
        }
        Ok(TshiftParams {
            tr,
            slice_times,
            tzero,
            ignore: self.ignore,
            method,
            restore,
            detrend: self.detrend,
            n_threads: self.n_threads,
            upstream_defects: self.upstream_defects,
        })
    }
}

/// `slice_times` is a pattern name or a sequence of numbers.
fn slice_spec(slice_times: &Bound<'_, PyAny>) -> PyResult<(Option<String>, Option<Vec<f64>>)> {
    if let Ok(name) = slice_times.extract::<String>() {
        return Ok((Some(name), None));
    }
    let times: Vec<f64> = slice_times.extract().map_err(|_| {
        PyTypeError::new_err("slice_times must be a pattern name or a sequence of numbers")
    })?;
    Ok((None, Some(times)))
}

fn read_err(e: ReadError) -> PyErr {
    match e {
        ReadError::Nifti(e) => to_py_err(e),
        e => value_err(e),
    }
}

/// The voxels as a Fortran-ordered `(nx, ny, nz, nt)` array of the storage type.
fn bricks_to_py<'py>(py: Python<'py>, bricks: Bricks) -> PyResult<Bound<'py, PyAny>> {
    fn array<'py, T: numpy::Element>(
        py: Python<'py>,
        shape: [usize; 4],
        v: Vec<T>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let a = ArrayD::from_shape_vec(IxDyn(&shape).f(), v).map_err(value_err)?;
        Ok(PyArray::from_owned_array(py, a).into_any())
    }
    match bricks.data {
        BrickData::Byte(v) => array(py, bricks.shape, v),
        BrickData::Short(v) => array(py, bricks.shape, v),
        BrickData::Float(v) => array(py, bricks.shape, v),
    }
}

/// Shifts, then returns `(data, header, warnings, notices)` with the header `3dTshift` would
/// write; `notices` are the upstream defects that changed the result (`larmorx._defects`).
#[allow(clippy::too_many_arguments)]
fn finish<'py>(
    py: Python<'py>,
    mut bricks: Bricks,
    geometry: &InputGeometry,
    kept: Option<KeptIntercept>,
    request: &Request,
    header_tr: f32,
    mut warnings: Vec<String>,
    mut notices: Vec<Notice>,
) -> PyResult<Bound<'py, PyTuple>> {
    let params = request.params(bricks.shape, header_tr)?;
    let defects = request.upstream_defects;
    if params
        .slice_times
        .iter()
        .all(|&t| t == params.slice_times[0])
    {
        warnings.push(format!(
            "input has only 1 time offset, {}",
            g32(params.slice_times[0])
        ));
    }
    // AFNI_NIFTI_SCALE=NO (AFNI-23).
    let mut scaling = None;
    if let Some(kept) = kept {
        let factor = bricks.factors.first().copied().unwrap_or(0.0);
        let (plan, notice) =
            dataset::intercept_plan(&kept, factor, false, params.restore, params.ignore, defects);
        notices.extend(notice);
        match plan {
            InterceptPlan::ReadScaled => dataset::read_scaled(&mut bricks, &kept),
            InterceptPlan::InHeader(s) => scaling = Some(s),
            InterceptPlan::Afni => {}
        }
    }
    notices.extend(
        py.detach(|| tshift(&mut bricks, &params))
            .map_err(value_err)?,
    );
    let timing = OutputTiming {
        tr: params.tr,
        toffset: params.tzero,
        slices: None,
    };
    let (header, geometry_notices, messages) = dataset::output_header(
        geometry,
        &bricks,
        &timing,
        scaling,
        defects,
        &request.settings,
    )
    .map_err(value_err)?;
    notices.extend(geometry_notices);
    warnings.extend(warning_texts(messages));
    let notices: Vec<PyNotice> = notices_to_py(notices);
    PyTuple::new(
        py,
        [
            bricks_to_py(py, bricks)?,
            header_to_dict(py, &header)?.into_any(),
            warnings.into_pyobject(py)?.into_any(),
            notices.into_pyobject(py)?.into_any(),
        ],
    )
}

/// Reads a NIfTI file with AFNI's rules and shifts it (`3dTshift`). Returns the voxels in
/// AFNI's storage type (`uint8`, `int16` or `float32`, before the brick factor, which is the
/// header's `scl_slope`), the output header, warnings and upstream-defect notices.
#[pyfunction]
#[pyo3(signature = (path, slice_times, tr = None, tzero = None, slice = None, ignore = 0, method = "fourier", restore = "trend", detrend = true, n_threads = 1, upstream_defects = "fix", afni_settings = None))]
#[allow(clippy::too_many_arguments)]
fn afni_tshift_file<'py>(
    py: Python<'py>,
    path: PathBuf,
    slice_times: &Bound<'py, PyAny>,
    tr: Option<f64>,
    tzero: Option<f64>,
    slice: Option<usize>,
    ignore: usize,
    method: &str,
    restore: &str,
    detrend: bool,
    n_threads: usize,
    upstream_defects: &str,
    afni_settings: Option<Vec<(String, String)>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let (pattern, times) = slice_spec(slice_times)?;
    let upstream_defects = defects_mode(upstream_defects)?;
    let request = Request {
        pattern,
        times,
        tr,
        tzero,
        slice,
        ignore,
        method: method.to_owned(),
        restore: restore.to_owned(),
        detrend,
        n_threads,
        upstream_defects,
        settings: settings_of(afni_settings),
    };
    let settings = &request.settings;
    let name = path.to_string_lossy().into_owned();
    // The name AFNI shows (its real path, unless AFNI_NOREALPATH) and the file it reads (x.nii
    // from x.nii.gz); a file that cannot be found is read as given, for the reader's error.
    let (mut messages, mut notices) = (Vec::new(), Vec::new());
    let (file, shown) = match input::resolve(
        &name,
        settings,
        upstream_defects,
        &mut messages,
        &mut notices,
    ) {
        Ok(found) => (found.path, found.shown),
        Err(_) => (path.clone(), name.clone()),
    };
    let AfniImage {
        bricks,
        tr: header_tr,
        geometry,
        messages: read_messages,
        notices: read_notices,
        kept_intercept,
        ..
    } = py
        .detach(|| dataset::read(&file, &shown, n_threads, upstream_defects, settings))
        .map_err(read_err)?;
    messages.extend(read_messages);
    notices.extend(read_notices);
    let warnings = warning_texts(messages).collect();
    finish(
        py,
        bricks,
        &geometry,
        kept_intercept,
        &request,
        header_tr,
        warnings,
        notices,
    )
}

/// Copies a 4D array (any memory layout) into Fortran order, zeroing non-finite floats as
/// AFNI does when it reads them.
fn fortran_vec<T: Copy + numpy::Element>(
    a: &Bound<'_, PyArrayDyn<T>>,
    clean: impl Fn(T) -> T,
) -> PyResult<([usize; 4], Vec<T>)> {
    let guard = a.readonly();
    let view = guard.as_array();
    let shape: [usize; 4] = view
        .shape()
        .try_into()
        .map_err(|_| PyValueError::new_err("tshift needs a 4D (x, y, z, t) array"))?;
    Ok((shape, view.t().iter().map(|&x| clean(x)).collect()))
}

/// Shifts a 4D array of `uint8`, `int16` or `float32` values placed by `header` (the header
/// the image would be written with). Returns `(data, header, warnings, notices)` as
/// [`afni_tshift_file`].
#[pyfunction]
#[pyo3(signature = (data, header, slice_times, tr = None, tzero = None, slice = None, ignore = 0, method = "fourier", restore = "trend", detrend = true, n_threads = 1, upstream_defects = "fix", afni_settings = None))]
#[allow(clippy::too_many_arguments)]
fn afni_tshift_array<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyAny>,
    header: &Bound<'py, PyDict>,
    slice_times: &Bound<'py, PyAny>,
    tr: Option<f64>,
    tzero: Option<f64>,
    slice: Option<usize>,
    ignore: usize,
    method: &str,
    restore: &str,
    detrend: bool,
    n_threads: usize,
    upstream_defects: &str,
    afni_settings: Option<Vec<(String, String)>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let (pattern, times) = slice_spec(slice_times)?;
    let request = Request {
        pattern,
        times,
        tr,
        tzero,
        slice,
        ignore,
        method: method.to_owned(),
        restore: restore.to_owned(),
        detrend,
        n_threads,
        upstream_defects: defects_mode(upstream_defects)?,
        settings: settings_of(afni_settings),
    };
    let header = header_from_dict(header)?;
    let (shape, data) = if let Ok(a) = data.cast::<PyArrayDyn<u8>>() {
        let (shape, v) = fortran_vec(a, |x| x)?;
        (shape, BrickData::Byte(v))
    } else if let Ok(a) = data.cast::<PyArrayDyn<i16>>() {
        let (shape, v) = fortran_vec(a, |x| x)?;
        (shape, BrickData::Short(v))
    } else if let Ok(a) = data.cast::<PyArrayDyn<f32>>() {
        let (shape, v) = fortran_vec(a, |x| if x.is_finite() { x } else { 0.0 })?;
        (shape, BrickData::Float(v))
    } else {
        return Err(PyTypeError::new_err(
            "tshift arrays must be uint8, int16 or float32 (native byte order)",
        ));
    };
    let bricks = Bricks::new(shape, data, vec![0.0; shape[3]]).map_err(value_err)?;
    let header_tr = dataset::header_tr(&header);
    let (geometry, messages) = geometry::read_geometry(&header, &request.settings, "<memory>");
    let warnings = warning_texts(messages).collect();
    finish(
        py,
        bricks,
        &geometry,
        None,
        &request,
        header_tr,
        warnings,
        Vec::new(),
    )
}

/// An output dataset as Python receives it: the voxels in their written shape (a plane of one
/// slice is 2D, one volume 3D, a series 4D) and the header.
fn output_to_py<'py>(
    py: Python<'py>,
    header: &larmorx_io::nifti::NiftiHeader,
    bricks: Bricks,
) -> PyResult<Bound<'py, PyAny>> {
    let shape = dataset::written_shape(bricks.shape);
    fn array<'py, T: numpy::Element>(
        py: Python<'py>,
        shape: &[usize],
        v: Vec<T>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let a = ArrayD::from_shape_vec(IxDyn(shape).f(), v).map_err(value_err)?;
        Ok(PyArray::from_owned_array(py, a).into_any())
    }
    let data = match bricks.data {
        BrickData::Byte(v) => array(py, &shape, v)?,
        BrickData::Short(v) => array(py, &shape, v)?,
        BrickData::Float(v) => array(py, &shape, v)?,
    };
    Ok(PyTuple::new(py, [data, header_to_dict(py, header)?.into_any()])?.into_any())
}

/// Reads the input of a Python call of 3dAutomask or 3dUnifize: the file found as AFNI finds
/// it, read with AFNI's rules (a slope of 1 kept as a factor), and AFNI's messages and the
/// notices of reading.
fn read_for_masks(
    py: Python<'_>,
    path: &std::path::Path,
    settings: &AfniSettings,
    mode: UpstreamDefects,
    n_threads: usize,
) -> PyResult<(AfniImage, Vec<Message>, Vec<Notice>)> {
    let name = path.to_string_lossy().into_owned();
    let (mut messages, mut notices) = (Vec::new(), Vec::new());
    let (file, shown) = match input::resolve(&name, settings, mode, &mut messages, &mut notices) {
        Ok(found) => (found.path, found.shown),
        Err(_) => (path.to_path_buf(), name.clone()),
    };
    let mut image = py
        .detach(|| dataset::read_as(&file, &shown, n_threads, mode, settings, true))
        .map_err(read_err)?;
    messages.extend(std::mem::take(&mut image.messages));
    notices.extend(std::mem::take(&mut image.notices));
    Ok((image, messages, notices))
}

/// 3dAutomask's options from Python's arguments, checked as the command line checks them
/// (`-nbhrs` outside 6..26: AFNI-41; `-NN1/2/3` set the peel threshold: AFNI-42).
#[allow(clippy::too_many_arguments)]
fn automask_options(
    clfrac: f64,
    gradual: bool,
    peels: i64,
    neighbours: i64,
    nn: Option<u8>,
    exterior_clip: bool,
    dilate: usize,
    erode: usize,
    si: Option<f64>,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) -> PyResult<AutomaskOptions> {
    use larmorx_afni::automask::morph::Neighbourhood;
    let clfrac = clfrac as f32;
    if !(0.1..=0.9).contains(&clfrac) {
        return Err(PyValueError::new_err(format!(
            "clfrac must be between 0.1 and 0.9, not {clfrac}"
        )));
    }
    let mut o = AutomaskOptions::default();
    o.mask.clfrac = clfrac;
    o.mask.gradual = gradual;
    o.mask.exterior_clip = exterior_clip;
    o.requested_peels = peels;
    o.dilate = dilate;
    o.erode = erode;
    o.si = si.map(|h| h as f32);
    let mut threshold = 17u32;
    if (6..=26).contains(&neighbours) {
        threshold = neighbours as u32;
    } else {
        // As the command line words it (AFNI peels as with 17, or as `nn` says).
        let acts = nn.map_or(17, |level| [6, 18, 26][usize::from(level.clamp(1, 3) - 1)]);
        let detail = match mode {
            UpstreamDefects::Fix => format!(
                "-nbhrs {neighbours} is outside 6..26 (the range the help gives): an error (exit \
                 status 1); AFNI silently peels as with {acts}"
            ),
            UpstreamDefects::Reproduce => format!(
                "-nbhrs {neighbours} is outside 6..26 (the range the help gives): the peeling \
                 acts as with {acts}, as in AFNI; by default it is an error"
            ),
        };
        if mode.fix() {
            return Err(PyValueError::new_err(format!(
                "neighbours={neighbours} is illegal: choose a value between 6 and 26 (AFNI-41; \
                 upstream_defects=\"reproduce\" gives AFNI's result)"
            )));
        }
        notices.push(Notice::new(&larmorx_core::defects::AFNI_41, mode, detail));
    }
    if let Some(level) = nn {
        o.nn = Neighbourhood::from_level(level)
            .ok_or_else(|| PyValueError::new_err(format!("nn must be 1, 2 or 3, not {level}")))?;
        threshold = [6, 18, 26][usize::from(level - 1)];
    }
    o.mask.threshold = threshold;
    Ok(o)
}

/// 3dAutomask on a NIfTI file (the clean-room original): returns `(mask, masked, depth,
/// warnings, notices)`, each output `(data, header)` or `None`, the data in AFNI's storage
/// type before the brick factor (the header's `scl_slope`).
#[pyfunction]
#[pyo3(signature = (path, clfrac = 0.5, gradual = true, peels = 1, neighbours = 17, nn = None, exterior_clip = false, dilate = 0, erode = 0, si = None, apply = false, depth = false, n_threads = 1, upstream_defects = "fix", afni_settings = None))]
#[allow(clippy::too_many_arguments)]
fn afni_automask_file<'py>(
    py: Python<'py>,
    path: PathBuf,
    clfrac: f64,
    gradual: bool,
    peels: i64,
    neighbours: i64,
    nn: Option<u8>,
    exterior_clip: bool,
    dilate: usize,
    erode: usize,
    si: Option<f64>,
    apply: bool,
    depth: bool,
    n_threads: usize,
    upstream_defects: &str,
    afni_settings: Option<Vec<(String, String)>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let mode = defects_mode(upstream_defects)?;
    let settings = settings_of(afni_settings);
    let mut notices = Vec::new();
    let mut options = automask_options(
        clfrac,
        gradual,
        peels,
        neighbours,
        nn,
        exterior_clip,
        dilate,
        erode,
        si,
        mode,
        &mut notices,
    )?;
    options.apply = apply;
    options.depth = depth;
    let (mut image, messages, read_notices) =
        read_for_masks(py, &path, &settings, mode, n_threads)?;
    notices.extend(read_notices);
    let run = py
        .detach(|| automask::run(&mut image, &options, mode, &settings, n_threads))
        .map_err(value_err)?;
    let mut warnings: Vec<String> = warning_texts(messages).collect();
    warnings.extend(warning_texts(run.messages));
    notices.extend(run.notices);
    if let Some(why) = run.failure {
        return Err(PyValueError::new_err(format!("3dAutomask: {why}")));
    }
    let to_py = |o: Option<automask::Output>| -> PyResult<Bound<'py, PyAny>> {
        match o {
            Some((h, b)) => output_to_py(py, &h, b),
            None => Ok(py.None().into_bound(py)),
        }
    };
    let notices: Vec<PyNotice> = notices_to_py(notices);
    PyTuple::new(
        py,
        [
            to_py(run.mask)?,
            to_py(run.masked)?,
            to_py(run.depth)?,
            warnings.into_pyobject(py)?.into_any(),
            notices.into_pyobject(py)?.into_any(),
        ],
    )
}

/// 3dUnifize on a NIfTI file (the clean-room original): returns `(image, scale, automasked,
/// warnings, notices)`, each output `(data, header)` or `None`. The arguments are 3dUnifize's
/// options, checked by the Python wrapper (`t2` is 0, 1 or 2).
#[pyfunction]
#[pyo3(signature = (path, t2 = 0, t2_percentile = 98.5, gm = false, radius = 18.3, bottom = 70.0, top = 80.0, clfrac = 0.2, duplo = true, epi = false, squash = true, save_scale = false, save_automask = false, n_threads = 1, upstream_defects = "fix", afni_settings = None))]
#[allow(clippy::too_many_arguments)]
fn afni_unifize_file<'py>(
    py: Python<'py>,
    path: PathBuf,
    t2: u32,
    t2_percentile: f64,
    gm: bool,
    radius: f64,
    bottom: f64,
    top: f64,
    clfrac: f64,
    duplo: bool,
    epi: bool,
    squash: bool,
    save_scale: bool,
    save_automask: bool,
    n_threads: usize,
    upstream_defects: &str,
    afni_settings: Option<Vec<(String, String)>>,
) -> PyResult<Bound<'py, PyTuple>> {
    use larmorx_afni::unifize::{self, Event, UnifizeOptions};
    let mode = defects_mode(upstream_defects)?;
    if t2 > 2 {
        return Err(PyValueError::new_err(format!(
            "t2 must be 0, 1 or 2, not {t2} (AFNI-48)"
        )));
    }
    let settings = settings_of(afni_settings);
    let options = UnifizeOptions {
        radius: radius as f32,
        bottom: bottom as f32,
        top: top as f32,
        clfrac: clfrac as f32,
        t2,
        t2up: t2_percentile as f32,
        gm,
        duplo,
        epi,
        squash,
        verb: false,
        scale: save_scale,
        automasked: save_automask,
    };
    let (mut image, messages, mut notices) = read_for_masks(py, &path, &settings, mode, n_threads)?;
    let run = py
        .detach(|| unifize::run(&mut image, &options, mode, &settings, n_threads))
        .map_err(value_err)?;
    let mut warnings: Vec<String> = warning_texts(messages).collect();
    let run_messages: Vec<Message> = run
        .events
        .into_iter()
        .filter_map(|e| match e {
            Event::Progress(m) | Event::Always(m) => Some(m),
            Event::Write(_) => None,
        })
        .collect();
    warnings.extend(warning_texts(run_messages));
    notices.extend(run.notices);
    if let Some(why) = run.failure {
        return Err(PyValueError::new_err(format!("3dUnifize: {why}")));
    }
    let to_py = |o: Option<automask::Output>| -> PyResult<Bound<'py, PyAny>> {
        match o {
            Some((h, b)) => output_to_py(py, &h, b),
            None => Ok(py.None().into_bound(py)),
        }
    };
    let notices: Vec<PyNotice> = notices_to_py(notices);
    PyTuple::new(
        py,
        [
            to_py(run.output)?,
            to_py(run.scale)?,
            to_py(run.automasked)?,
            warnings.into_pyobject(py)?.into_any(),
            notices.into_pyobject(py)?.into_any(),
        ],
    )
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(afni_tshift_file, m)?)?;
    m.add_function(wrap_pyfunction!(afni_tshift_array, m)?)?;
    m.add_function(wrap_pyfunction!(afni_automask_file, m)?)?;
    m.add_function(wrap_pyfunction!(afni_unifize_file, m)?)?;
    Ok(())
}
