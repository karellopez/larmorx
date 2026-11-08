// SPDX-License-Identifier: Apache-2.0
//! Bindings of scikit-image's pieces (`larmorx_ndimage::skimage`: `ball`, `threshold_otsu`,
//! `random_walker`), sdcflows' `brain_masker` (`larmorx_sdcflows::masker`), and two numpy
//! helpers the checks use (`histogram`, `dot`), for `larmorx.skimage` and `larmorx.sdcflows`.

use larmorx_core::numpy::reduce::{SumOrder, std_ordered};
use larmorx_core::numpy::{self as np, NpFloat, NumpyRelease};
use larmorx_ndimage::skimage::{
    self as sk, Mode, Numbering, Preprocessed, RandomWalker, RwWarning, Spacing, SparseOrder,
};
use larmorx_sdcflows::masker::{MaskerOptions, brain_masker};
use numpy::ndarray::{ArrayD, IxDyn};
use numpy::{Element, PyArray, PyArrayDyn, PyReadonlyArrayDyn, PyUntypedArrayMethods};
use pyo3::exceptions::{PyIndexError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

fn to_py<'py, T: Element>(
    py: Python<'py>,
    shape: &[usize],
    v: Vec<T>,
) -> PyResult<Bound<'py, PyArrayDyn<T>>> {
    let a = ArrayD::from_shape_vec(IxDyn(shape), v)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(PyArray::from_owned_array(py, a))
}

fn values<T: Copy + Element>(a: &PyReadonlyArrayDyn<'_, T>) -> Vec<T> {
    a.as_array().iter().copied().collect()
}

fn release(name: &str) -> PyResult<NumpyRelease> {
    NumpyRelease::from_version(name)
        .ok_or_else(|| PyValueError::new_err(format!("not a numpy 2 version: {name:?}")))
}

fn sum_order(name: &str) -> PyResult<SumOrder> {
    SumOrder::from_version(name)
        .ok_or_else(|| PyValueError::new_err(format!("not a numpy version: {name:?}")))
}

/// SciPy's release name to the order of the random walker's Laplacian rows: 1.15 (and older)
/// puts the diagonal last; 1.16 onwards sorts.
fn scipy_order(name: &str) -> PyResult<SparseOrder> {
    let mut it = name.split('.');
    let major: u32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor: u32 = it
        .next()
        .map(|s| s.trim_end_matches(|c: char| !c.is_ascii_digit()))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| PyValueError::new_err(format!("not a SciPy version: {name:?}")))?;
    Ok(if major > 1 || minor >= 16 {
        SparseOrder::Scipy118
    } else {
        SparseOrder::Scipy115
    })
}

fn otsu_err(e: sk::OtsuError) -> PyErr {
    match e {
        sk::OtsuError::Empty => PyIndexError::new_err(e.to_string()),
        _ => PyValueError::new_err(e.to_string()),
    }
}

/// `skimage.morphology.ball(radius, strict_radius=...)` as a C-ordered bool array.
#[pyfunction]
fn skimage_ball(py: Python<'_>, radius: f64, strict_radius: bool) -> PyResult<Bound<'_, PyAny>> {
    let b = sk::ball(radius, strict_radius);
    Ok(to_py(py, &b.shape, b.mask)?.into_any())
}

/// `threshold_otsu(image, nbins)` of a float32 or float64 image (the threshold as a Python
/// float in the image's type).
#[pyfunction]
fn skimage_threshold_otsu(py: Python<'_>, image: &Bound<'_, PyAny>, nbins: usize) -> PyResult<f64> {
    if let Ok(x) = image.extract::<PyReadonlyArrayDyn<'_, f32>>() {
        let v = values(&x);
        return py
            .detach(|| sk::threshold_otsu(&v, nbins))
            .map(f64::from)
            .map_err(otsu_err);
    }
    let x: PyReadonlyArrayDyn<'_, f64> = image.extract()?;
    let v = values(&x);
    py.detach(|| sk::threshold_otsu(&v, nbins))
        .map_err(otsu_err)
}

/// `threshold_otsu(image)` of an integer image given as int64 values.
#[pyfunction]
fn skimage_threshold_otsu_int(py: Python<'_>, image: PyReadonlyArrayDyn<'_, i64>) -> PyResult<i64> {
    let v = values(&image);
    py.detach(|| sk::threshold_otsu_int(&v)).map_err(otsu_err)
}

/// `threshold_otsu(hist=(counts, centres))`: counts as float32, centres as float32 or float64
/// (the type numpy promotes `float32 * centres` to); the index of the chosen centre.
#[pyfunction]
fn skimage_threshold_otsu_hist(
    counts: PyReadonlyArrayDyn<'_, f32>,
    centers: &Bound<'_, PyAny>,
) -> PyResult<usize> {
    let c = values(&counts);
    if let Ok(x) = centers.extract::<PyReadonlyArrayDyn<'_, f32>>() {
        return sk::threshold_otsu_hist(&c, &values(&x)).map_err(otsu_err);
    }
    let x: PyReadonlyArrayDyn<'_, f64> = centers.extract()?;
    sk::threshold_otsu_hist(&c, &values(&x)).map_err(otsu_err)
}

fn warning_text(w: RwWarning) -> &'static str {
    match w {
        RwWarning::NotConverged => {
            "Conjugate gradient convergence to tolerance not achieved. Consider decreasing beta \
             to improve system conditionning."
        }
        RwWarning::ProbabilityRange => {
            "The probability range is outside [0, 1] given the tolerance `prob_tol`. Consider \
             decreasing `beta` and/or decreasing `tol`."
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn random_walker_t<'py, D: NpFloat + Element>(
    py: Python<'py>,
    data: &PyReadonlyArrayDyn<'py, D>,
    labels: &[i64],
    shape: [usize; 3],
    channels: usize,
    multichannel: bool,
    spacing: Spacing,
    std: Option<f64>,
    order_of_sums: Option<SumOrder>,
    beta: f64,
    mode: Mode,
    tol: f64,
    prob_tol: f64,
    order: SparseOrder,
    numbering: Numbering,
    return_full_prob: bool,
    n_threads: usize,
) -> PyResult<Bound<'py, PyDict>> {
    let v = values(data);
    let out = PyDict::new(py);
    let result = py.detach(|| {
        larmorx_core::parallel::with_threads(n_threads, || {
            let prep = sk::preprocess(labels, shape)?;
            let Preprocessed::Ready(prep) = prep else {
                return Ok((prep, None));
            };
            let rw = RandomWalker {
                data: &v,
                shape,
                channels,
                std: match (std, order_of_sums) {
                    (Some(s), _) => Some(D::from_f64(s)),
                    (None, Some(o)) => Some(std_ordered(&v, 0, o)),
                    (None, None) => None,
                },
                multichannel,
                spacing,
                beta,
                mode,
                tol,
                prob_tol,
                order,
                numbering,
            };
            let res = sk::random_walker(&rw, &prep)?;
            Ok((Preprocessed::Ready(prep), Some(res)))
        })
    });
    let (prep, res) = result
        .map_err(|e| PyValueError::new_err(e.to_string()))?
        .map_err(|e: sk::RwError| PyValueError::new_err(e.to_string()))?;
    match (prep, res) {
        (Preprocessed::NoZero, _) => out.set_item("kind", "nozero")?,
        (Preprocessed::AllIsolated(l), _) => {
            out.set_item("kind", "isolated")?;
            out.set_item("labels", to_py(py, &shape, l)?)?;
        }
        (Preprocessed::Ready(prep), Some(res)) => {
            let warnings: Vec<&str> = res.warnings.iter().map(|&w| warning_text(w)).collect();
            out.set_item("warnings", warnings)?;
            out.set_item("iterations", res.iterations.clone())?;
            out.set_item("float32", res.float32)?;
            out.set_item("nlabels", prep.nlabels)?;
            out.set_item("numbering_differs", res.numbering_differs)?;
            if return_full_prob {
                out.set_item("kind", "full")?;
                let full = res.full_prob(&prep);
                out.set_item(
                    "prob",
                    to_py(py, &[prep.nlabels, shape[0], shape[1], shape[2]], full)?,
                )?;
            } else {
                out.set_item("kind", "labels")?;
                out.set_item("labels", to_py(py, &shape, res.labels(&prep))?)?;
            }
        }
        (Preprocessed::Ready(_), None) => unreachable!("a system is always solved"),
    }
    Ok(out)
}

/// `random_walker` after scikit-image's argument checks: C-ordered data `(x, y, z, channels)`
/// (float32 or float64), C-ordered int64 labels `(x, y, z)`, the spacing in its numpy type
/// (float32 or float64), numpy's `data.std()` of the caller's array (`None`: computed here in C
/// order). Returns a dict: `kind` (`nozero`, `isolated` with `labels`, `full` with `prob`, or
/// `labels` with `labels`), `warnings`, `iterations`, `float32`, `nlabels`.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (data, labels, multichannel, spacing, std, beta, mode, tol, prob_tol, scipy_version, numbering, return_full_prob, n_threads, numpy_version=None))]
fn skimage_random_walker<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyAny>,
    labels: PyReadonlyArrayDyn<'py, i64>,
    multichannel: bool,
    spacing: &Bound<'py, PyAny>,
    std: Option<f64>,
    beta: f64,
    mode: &str,
    tol: f64,
    prob_tol: f64,
    scipy_version: &str,
    numbering: &str,
    return_full_prob: bool,
    n_threads: usize,
    numpy_version: Option<&str>,
) -> PyResult<Bound<'py, PyDict>> {
    let order_of_sums = numpy_version.map(sum_order).transpose()?;
    let lshape = labels.shape().to_vec();
    if lshape.len() != 3 {
        return Err(PyValueError::new_err("labels must be 3-D here"));
    }
    let shape = [lshape[0], lshape[1], lshape[2]];
    let lab = values(&labels);
    let spacing = if let Ok(s) = spacing.extract::<PyReadonlyArrayDyn<'_, f32>>() {
        let v = values(&s);
        Spacing::F32([v[0], v[1], v[2]])
    } else {
        let s: PyReadonlyArrayDyn<'_, f64> = spacing.extract()?;
        let v = values(&s);
        Spacing::F64([v[0], v[1], v[2]])
    };
    let mode = match mode {
        "cg" => Mode::Cg,
        "cg_j" => Mode::CgJ,
        other => {
            return Err(PyValueError::new_err(format!(
                "mode {other:?} is not ported"
            )));
        }
    };
    let numbering = match numbering {
        "mask" => Numbering::Mask,
        "edges" => Numbering::Edges,
        other => return Err(PyValueError::new_err(format!("numbering {other:?}"))),
    };
    let order = scipy_order(scipy_version)?;
    if let Ok(x) = data.extract::<PyReadonlyArrayDyn<'py, f32>>() {
        let channels = *x.shape().last().unwrap_or(&1);
        return random_walker_t(
            py,
            &x,
            &lab,
            shape,
            channels,
            multichannel,
            spacing,
            std,
            order_of_sums,
            beta,
            mode,
            tol,
            prob_tol,
            order,
            numbering,
            return_full_prob,
            n_threads,
        );
    }
    let x: PyReadonlyArrayDyn<'py, f64> = data.extract()?;
    let channels = *x.shape().last().unwrap_or(&1);
    random_walker_t(
        py,
        &x,
        &lab,
        shape,
        channels,
        multichannel,
        spacing,
        std,
        order_of_sums,
        beta,
        mode,
        tol,
        prob_tol,
        order,
        numbering,
        return_full_prob,
        n_threads,
    )
}

/// sdcflows' `brain_masker` of a C-ordered float32 3-D array (`img.get_fdata(dtype='float32')`)
/// with the header's first three zooms: `(probseg float64, mask uint8, threshold, iterations,
/// warnings)`.
#[pyfunction]
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sdcflows_brain_masker<'py>(
    py: Python<'py>,
    data: PyReadonlyArrayDyn<'py, f32>,
    zooms: [f32; 3],
    padding: usize,
    numpy_version: &str,
    scipy_version: &str,
    reproduce: bool,
    n_threads: usize,
) -> PyResult<(
    Bound<'py, PyArrayDyn<f64>>,
    Bound<'py, PyArrayDyn<u8>>,
    f64,
    Vec<usize>,
    Vec<String>,
)> {
    let sh = data.shape().to_vec();
    if sh.len() != 3 {
        return Err(PyValueError::new_err("brain_masker needs a 3-D image"));
    }
    let shape = [sh[0], sh[1], sh[2]];
    let v = values(&data);
    let opts = MaskerOptions {
        padding,
        numpy: release(numpy_version)?,
        sum_order: sum_order(numpy_version)?,
        scipy: scipy_order(scipy_version)?,
        reproduce,
    };
    let out = py
        .detach(|| {
            larmorx_core::parallel::with_threads(n_threads, || brain_masker(&v, shape, zooms, opts))
        })
        .map_err(|e| PyValueError::new_err(e.to_string()))?
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok((
        to_py(py, &sh, out.probseg)?,
        to_py(py, &sh, out.mask)?,
        f64::from(out.threshold),
        out.iterations,
        out.warnings,
    ))
}

/// `numpy.histogram(a, bins=nbins)` of a float32 or float64 array: `(counts int64, edges)`.
#[pyfunction]
fn numpy_histogram<'py>(
    py: Python<'py>,
    a: &Bound<'py, PyAny>,
    nbins: usize,
) -> PyResult<(Bound<'py, PyArrayDyn<i64>>, Bound<'py, PyAny>)> {
    fn run<'py, T: NpFloat + Element>(
        py: Python<'py>,
        v: Vec<T>,
        nbins: usize,
    ) -> PyResult<(Bound<'py, PyArrayDyn<i64>>, Bound<'py, PyAny>)> {
        let (c, e) = np::histograms::histogram(&v, nbins)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        let c: Vec<i64> = c.into_iter().map(|x| x as i64).collect();
        let ne = e.len();
        Ok((to_py(py, &[nbins], c)?, to_py(py, &[ne], e)?.into_any()))
    }
    if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, f32>>() {
        return run(py, values(&x), nbins);
    }
    let x: PyReadonlyArrayDyn<'_, f64> = a.extract()?;
    run(py, values(&x), nbins)
}

/// `np.dot(x, y)` of two float32 or float64 vectors (OpenBLAS's Haswell kernels).
#[pyfunction]
fn numpy_dot(x: &Bound<'_, PyAny>, y: &Bound<'_, PyAny>) -> PyResult<f64> {
    if let (Ok(a), Ok(b)) = (
        x.extract::<PyReadonlyArrayDyn<'_, f32>>(),
        y.extract::<PyReadonlyArrayDyn<'_, f32>>(),
    ) {
        return Ok(f64::from(np::dot::dot_f32(&values(&a), &values(&b))));
    }
    let a: PyReadonlyArrayDyn<'_, f64> = x.extract()?;
    let b: PyReadonlyArrayDyn<'_, f64> = y.extract()?;
    Ok(np::dot::dot_f64(&values(&a), &values(&b)))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(skimage_ball, m)?)?;
    m.add_function(wrap_pyfunction!(skimage_threshold_otsu, m)?)?;
    m.add_function(wrap_pyfunction!(skimage_threshold_otsu_int, m)?)?;
    m.add_function(wrap_pyfunction!(skimage_threshold_otsu_hist, m)?)?;
    m.add_function(wrap_pyfunction!(skimage_random_walker, m)?)?;
    m.add_function(wrap_pyfunction!(sdcflows_brain_masker, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_histogram, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_dot, m)?)?;
    Ok(())
}
