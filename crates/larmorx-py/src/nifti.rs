//! Bindings for `larmorx_io::nifti`.
//!
//! Headers cross the boundary as dicts whose keys are the fields of the Python
//! `larmorx.io.NiftiHeader` dataclass; arrays cross as numpy arrays without copying (data read
//! from files keep their Fortran order). The GIL is released while files are read or written.

use larmorx_core::element::DataType;
use larmorx_core::{Affine, Complex};
use larmorx_io::nifti::{
    self, ByteOrder, Extension, NiftiHeader, NiftiVersion, ReadOptions, Scaling, WriteOptions,
    header::AnalyzeFields, header::Nifti2Fields,
};
use numpy::{PyArray, PyArrayDyn, PyArrayMethods, PyReadonlyArray2};
use pyo3::exceptions::{PyFileNotFoundError, PyOSError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyTuple};

pyo3::create_exception!(
    _core,
    NiftiError,
    PyValueError,
    "A NIfTI file is malformed or unsupported."
);

fn to_py_err(err: nifti::Error) -> PyErr {
    match &err {
        nifti::Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
            PyFileNotFoundError::new_err(err.to_string())
        }
        nifti::Error::Io { .. } => PyOSError::new_err(err.to_string()),
        nifti::Error::InvalidArgument(_) => PyValueError::new_err(err.to_string()),
        _ => NiftiError::new_err(err.to_string()),
    }
}

fn header_err(err: nifti::HeaderError) -> PyErr {
    NiftiError::new_err(err.to_string())
}

// ------------------------------------------------------------------------------------------------
// Header <-> dict

fn bytes_field<const N: usize>(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<[u8; N]> {
    let value: Vec<u8> = get(dict, key)?;
    let mut out = [0u8; N];
    if value.len() > N {
        return Err(PyValueError::new_err(format!(
            "{key} is longer than {N} bytes"
        )));
    }
    out[..value.len()].copy_from_slice(&value);
    Ok(out)
}

fn get<'py, T: FromPyObjectOwned<'py>>(dict: &Bound<'py, PyDict>, key: &str) -> PyResult<T> {
    let item = dict
        .get_item(key)?
        .ok_or_else(|| PyValueError::new_err(format!("header field '{key}' is missing")))?;
    let value: PyResult<T> = item.extract::<T>().map_err(Into::into);
    value.map_err(|e| PyTypeError::new_err(format!("header field '{key}': {e}")))
}

fn array_field<const N: usize, T: Copy + Default + for<'py> FromPyObjectOwned<'py>>(
    dict: &Bound<'_, PyDict>,
    key: &str,
) -> PyResult<[T; N]> {
    let values: Vec<T> = get(dict, key)?;
    values.try_into().map_err(|v: Vec<T>| {
        PyValueError::new_err(format!("{key} must have {N} values, not {}", v.len()))
    })
}

fn header_to_dict<'py>(py: Python<'py>, h: &NiftiHeader) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item(
        "version",
        match h.version {
            NiftiVersion::V1 => 1,
            NiftiVersion::V2 => 2,
        },
    )?;
    d.set_item(
        "byte_order",
        match h.byte_order {
            ByteOrder::Little => "<",
            ByteOrder::Big => ">",
        },
    )?;
    d.set_item("magic", PyBytes::new(py, &h.magic))?;
    d.set_item("dim_info", h.dim_info)?;
    d.set_item("dim", PyTuple::new(py, h.dim)?)?;
    d.set_item("intent_p1", h.intent_p1)?;
    d.set_item("intent_p2", h.intent_p2)?;
    d.set_item("intent_p3", h.intent_p3)?;
    d.set_item("intent_code", h.intent_code)?;
    d.set_item("datatype", h.datatype)?;
    d.set_item("bitpix", h.bitpix)?;
    d.set_item("slice_start", h.slice_start)?;
    d.set_item("pixdim", PyTuple::new(py, h.pixdim)?)?;
    d.set_item("vox_offset", h.vox_offset)?;
    d.set_item("scl_slope", h.scl_slope)?;
    d.set_item("scl_inter", h.scl_inter)?;
    d.set_item("slice_end", h.slice_end)?;
    d.set_item("slice_code", h.slice_code)?;
    d.set_item("xyzt_units", h.xyzt_units)?;
    d.set_item("cal_max", h.cal_max)?;
    d.set_item("cal_min", h.cal_min)?;
    d.set_item("slice_duration", h.slice_duration)?;
    d.set_item("toffset", h.toffset)?;
    d.set_item("descrip", PyBytes::new(py, &h.descrip))?;
    d.set_item("aux_file", PyBytes::new(py, &h.aux_file))?;
    d.set_item("qform_code", h.qform_code)?;
    d.set_item("sform_code", h.sform_code)?;
    d.set_item("quatern_b", h.quatern_b)?;
    d.set_item("quatern_c", h.quatern_c)?;
    d.set_item("quatern_d", h.quatern_d)?;
    d.set_item("qoffset_x", h.qoffset_x)?;
    d.set_item("qoffset_y", h.qoffset_y)?;
    d.set_item("qoffset_z", h.qoffset_z)?;
    d.set_item("srow_x", PyTuple::new(py, h.srow_x)?)?;
    d.set_item("srow_y", PyTuple::new(py, h.srow_y)?)?;
    d.set_item("srow_z", PyTuple::new(py, h.srow_z)?)?;
    d.set_item("intent_name", PyBytes::new(py, &h.intent_name))?;
    d.set_item("data_type", PyBytes::new(py, &h.analyze.data_type))?;
    d.set_item("db_name", PyBytes::new(py, &h.analyze.db_name))?;
    d.set_item("extents", h.analyze.extents)?;
    d.set_item("session_error", h.analyze.session_error)?;
    d.set_item("regular", h.analyze.regular)?;
    d.set_item("glmax", h.analyze.glmax)?;
    d.set_item("glmin", h.analyze.glmin)?;
    d.set_item("eol_check", PyBytes::new(py, &h.nifti2.eol_check))?;
    d.set_item("unused_str", PyBytes::new(py, &h.nifti2.unused_str))?;
    let exts = PyList::empty(py);
    for e in &h.extensions {
        exts.append((e.code, PyBytes::new(py, &e.content)))?;
    }
    d.set_item("extensions", exts)?;
    Ok(d)
}

fn header_from_dict(d: &Bound<'_, PyDict>) -> PyResult<NiftiHeader> {
    let version = match get::<i64>(d, "version")? {
        1 => NiftiVersion::V1,
        2 => NiftiVersion::V2,
        v => {
            return Err(PyValueError::new_err(format!(
                "version must be 1 or 2, not {v}"
            )));
        }
    };
    let byte_order = match get::<String>(d, "byte_order")?.as_str() {
        "<" => ByteOrder::Little,
        ">" => ByteOrder::Big,
        o => {
            return Err(PyValueError::new_err(format!(
                "byte_order must be '<' or '>', not {o:?}"
            )));
        }
    };
    let extensions = get::<Vec<(i32, Vec<u8>)>>(d, "extensions")?
        .into_iter()
        .map(|(code, content)| Extension::new(code, content))
        .collect();
    Ok(NiftiHeader {
        version,
        byte_order,
        magic: bytes_field(d, "magic")?,
        dim_info: get(d, "dim_info")?,
        dim: array_field(d, "dim")?,
        intent_p1: get(d, "intent_p1")?,
        intent_p2: get(d, "intent_p2")?,
        intent_p3: get(d, "intent_p3")?,
        intent_code: get(d, "intent_code")?,
        datatype: get(d, "datatype")?,
        bitpix: get(d, "bitpix")?,
        slice_start: get(d, "slice_start")?,
        pixdim: array_field(d, "pixdim")?,
        vox_offset: get(d, "vox_offset")?,
        scl_slope: get(d, "scl_slope")?,
        scl_inter: get(d, "scl_inter")?,
        slice_end: get(d, "slice_end")?,
        slice_code: get(d, "slice_code")?,
        xyzt_units: get(d, "xyzt_units")?,
        cal_max: get(d, "cal_max")?,
        cal_min: get(d, "cal_min")?,
        slice_duration: get(d, "slice_duration")?,
        toffset: get(d, "toffset")?,
        descrip: bytes_field(d, "descrip")?,
        aux_file: bytes_field(d, "aux_file")?,
        qform_code: get(d, "qform_code")?,
        sform_code: get(d, "sform_code")?,
        quatern_b: get(d, "quatern_b")?,
        quatern_c: get(d, "quatern_c")?,
        quatern_d: get(d, "quatern_d")?,
        qoffset_x: get(d, "qoffset_x")?,
        qoffset_y: get(d, "qoffset_y")?,
        qoffset_z: get(d, "qoffset_z")?,
        srow_x: array_field(d, "srow_x")?,
        srow_y: array_field(d, "srow_y")?,
        srow_z: array_field(d, "srow_z")?,
        intent_name: bytes_field(d, "intent_name")?,
        analyze: AnalyzeFields {
            data_type: bytes_field(d, "data_type")?,
            db_name: bytes_field(d, "db_name")?,
            extents: get(d, "extents")?,
            session_error: get(d, "session_error")?,
            regular: get(d, "regular")?,
            glmax: get(d, "glmax")?,
            glmin: get(d, "glmin")?,
        },
        nifti2: Nifti2Fields {
            eol_check: bytes_field(d, "eol_check")?,
            unused_str: bytes_field(d, "unused_str")?,
        },
        extensions,
    })
}

fn affine_to_py<'py>(py: Python<'py>, a: &Affine) -> Bound<'py, PyArray<f64, numpy::Ix2>> {
    let rows = a.rows();
    let arr = numpy::ndarray::Array2::from_shape_fn((4, 4), |(i, j)| rows[i][j]);
    PyArray::from_owned_array(py, arr)
}

fn affine_from_py(a: &PyReadonlyArray2<'_, f64>) -> PyResult<Affine> {
    let view = a.as_array();
    if view.shape() != [4, 4] {
        return Err(PyValueError::new_err(format!(
            "affine must be 4x4, not {:?}",
            view.shape()
        )));
    }
    Ok(Affine::from_rows(std::array::from_fn(|i| {
        std::array::from_fn(|j| view[[i, j]])
    })))
}

fn data_type_from_name(name: &str) -> PyResult<DataType> {
    DataType::from_name(name)
        .ok_or_else(|| PyValueError::new_err(format!("unsupported data type {name:?}")))
}

// ------------------------------------------------------------------------------------------------
// Functions

/// Reads a NIfTI file. `scaling` is "auto", "raw", "float32" or "float64". Returns
/// `(data, affine, header, fixes, scaled)`.
#[pyfunction]
#[pyo3(signature = (path, scaling = "auto", n_threads = 1))]
pub fn nifti_read<'py>(
    py: Python<'py>,
    path: std::path::PathBuf,
    scaling: &str,
    n_threads: usize,
) -> PyResult<Bound<'py, PyTuple>> {
    let scaling = match scaling {
        "auto" => Scaling::Auto,
        "raw" => Scaling::Raw,
        "float32" => Scaling::F32,
        "float64" => Scaling::F64,
        s => return Err(PyValueError::new_err(format!("unknown scaling {s:?}"))),
    };
    let image = py
        .detach(|| nifti::read(&path, &ReadOptions { scaling, n_threads }))
        .map_err(to_py_err)?;
    let affine = affine_to_py(py, &image.affine().map_err(header_err)?).into_any();
    let data: Bound<'py, PyAny> = larmorx_core::dispatch_dyn_array!(image.data, a => PyArray::from_owned_array(py, a).into_any());
    let fixes: Vec<&str> = image.fixes.iter().map(|f| f.0).collect();
    PyTuple::new(
        py,
        [
            data,
            affine,
            header_to_dict(py, &image.header)?.into_any(),
            fixes.into_pyobject(py)?.into_any(),
            image.scaled.into_pyobject(py)?.to_owned().into_any(),
        ],
    )
}

/// Parses a header block (348 or 540 bytes, e.g. nibabel's `header.binaryblock`) without
/// extensions and without fixes.
#[pyfunction]
pub fn nifti_parse_header<'py>(py: Python<'py>, block: &[u8]) -> PyResult<Bound<'py, PyDict>> {
    header_to_dict(py, &NiftiHeader::parse(block).map_err(header_err)?)
}

/// Reads a header: exactly as stored, or with nibabel's load-time fixes applied (`fix`).
/// Returns the header dict, and the list of fixes when `fix` is set.
#[pyfunction]
#[pyo3(signature = (path, fix = false))]
pub fn nifti_read_header(
    py: Python<'_>,
    path: std::path::PathBuf,
    fix: bool,
) -> PyResult<Bound<'_, PyAny>> {
    let mut header = py.detach(|| nifti::read_header(&path)).map_err(to_py_err)?;
    if !fix {
        return Ok(header_to_dict(py, &header)?.into_any());
    }
    let fixes: Vec<&str> = header
        .apply_load_fixes()
        .map_err(header_err)?
        .iter()
        .map(|f| f.0)
        .collect();
    Ok(PyTuple::new(
        py,
        [
            header_to_dict(py, &header)?.into_any(),
            fixes.into_pyobject(py)?.into_any(),
        ],
    )?
    .into_any())
}

macro_rules! write_typed {
    ($py:expr, $array:expr, $path:expr, $header:expr, $options:expr; $($t:ty),*) => {{
        $(
            if let Ok(a) = $array.cast::<PyArrayDyn<$t>>() {
                let guard = a.readonly();
                let view = guard.as_array();
                return $py.detach(|| nifti::write(&$path, &$header, view, &$options)).map_err(to_py_err);
            }
        )*
    }};
}

/// Writes `data` with `header` (a header dict). Data are stored in their own type.
#[pyfunction]
#[pyo3(signature = (path, data, header, compression_level = 2, n_threads = 1))]
pub fn nifti_write(
    py: Python<'_>,
    path: std::path::PathBuf,
    data: &Bound<'_, PyAny>,
    header: &Bound<'_, PyDict>,
    compression_level: u32,
    n_threads: usize,
) -> PyResult<()> {
    let header = header_from_dict(header)?;
    let options = WriteOptions {
        compression_level,
        n_threads,
    };
    write_typed!(py, data, path, header, options;
        u8, i8, u16, i16, u32, i32, u64, i64, f32, f64, Complex<f32>, Complex<f64>);
    let dtype = data
        .getattr("dtype")
        .map_or_else(|_| "?".to_owned(), |d| d.to_string());
    Err(PyTypeError::new_err(format!(
        "cannot write an array of dtype {dtype}: expected a native-byte-order numpy array of \
         (u)int8/16/32/64, float32/64 or complex64/128"
    )))
}

/// The header nibabel would write for an image (see `larmorx_io::nifti::header_for_image`).
#[pyfunction]
#[pyo3(signature = (shape, dtype, affine, template = None, version = None))]
pub fn nifti_header_for_image<'py>(
    py: Python<'py>,
    shape: Vec<usize>,
    dtype: &str,
    affine: PyReadonlyArray2<'_, f64>,
    template: Option<&Bound<'_, PyDict>>,
    version: Option<u8>,
) -> PyResult<Bound<'py, PyDict>> {
    let template = template.map(header_from_dict).transpose()?;
    let version = match version {
        None => None,
        Some(1) => Some(NiftiVersion::V1),
        Some(2) => Some(NiftiVersion::V2),
        Some(v) => {
            return Err(PyValueError::new_err(format!(
                "version must be 1 or 2, not {v}"
            )));
        }
    };
    let header = nifti::header_for_image(
        template.as_ref(),
        version,
        &shape,
        data_type_from_name(dtype)?,
        &affine_from_py(&affine)?,
    )
    .map_err(header_err)?;
    header_to_dict(py, &header)
}

/// Quantities derived from a header dict: shape, data type, zooms, the affines and scaling.
#[pyfunction]
pub fn nifti_header_info<'py>(
    py: Python<'py>,
    header: &Bound<'_, PyDict>,
) -> PyResult<Bound<'py, PyDict>> {
    let h = header_from_dict(header)?;
    let out = PyDict::new(py);
    let shape = h.shape().map_err(header_err)?;
    out.set_item("shape", PyTuple::new(py, shape)?)?;
    out.set_item("dtype", h.data_type().map(DataType::name).ok())?;
    out.set_item("zooms", PyTuple::new(py, h.zooms().map_err(header_err)?)?)?;
    out.set_item(
        "affine",
        affine_to_py(py, &h.best_affine().map_err(header_err)?),
    )?;
    out.set_item("qform", h.qform().ok().map(|a| affine_to_py(py, &a)))?;
    out.set_item("sform", affine_to_py(py, &h.sform()))?;
    out.set_item(
        "base_affine",
        affine_to_py(py, &h.base_affine().map_err(header_err)?),
    )?;
    out.set_item("slope_inter", h.slope_inter().map_err(header_err)?)?;
    out.set_item("header_bytes", PyBytes::new(py, &h.to_bytes()))?;
    Ok(out)
}

/// Registers the NIfTI functions and exception on the extension module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("NiftiError", m.py().get_type::<NiftiError>())?;
    m.add_function(wrap_pyfunction!(nifti_read, m)?)?;
    m.add_function(wrap_pyfunction!(nifti_read_header, m)?)?;
    m.add_function(wrap_pyfunction!(nifti_parse_header, m)?)?;
    m.add_function(wrap_pyfunction!(nifti_write, m)?)?;
    m.add_function(wrap_pyfunction!(nifti_header_for_image, m)?)?;
    m.add_function(wrap_pyfunction!(nifti_header_info, m)?)?;
    Ok(())
}
