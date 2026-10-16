// SPDX-License-Identifier: Apache-2.0
//! Bindings for `larmorx-hdf5`: a group's datasets as h5py lists and reads them (for
//! `lx.transforms.chain.load_itk_composite`, which reads ITK composites as nitransforms does),
//! and the replay of an ITK-written transform file that the validation suite `itk-hdf5` uses.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use larmorx_core::Precision;
use larmorx_core::defects::{Notices, UpstreamDefects};
use larmorx_hdf5::{Datatype, File, Layout, ObjectKind};
use larmorx_io::itk_hdf5::{self, WriteOptions};
use numpy::PyArray;
use pyo3::exceptions::{PyKeyError, PyOSError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};

fn open(path: &Path) -> PyResult<File> {
    match File::is_hdf5(path) {
        Ok(true) => {}
        Ok(false) => {
            return Err(PyOSError::new_err(format!(
                "Unable to open file {} (file signature not found)",
                path.display()
            )));
        }
        Err(e) => return Err(PyOSError::new_err(format!("{}: {e}", path.display()))),
    }
    File::open(path).map_err(|e| PyOSError::new_err(format!("{}: {e}", path.display())))
}

fn h5_err(path: &Path, e: larmorx_hdf5::Error) -> PyErr {
    PyValueError::new_err(format!("{}: {e}", path.display()))
}

/// A dataset's values as h5py reads them: strings as a list of `bytes` (`None` for a null
/// variable-length string), floating-point numbers as a float32 array when stored in 4 bytes
/// and a float64 array otherwise (h5py keeps half and extended precision; their values are
/// the same in float64 for what readers do with them).
fn dataset_value<'py>(
    py: Python<'py>,
    file: &File,
    path: &Path,
    info: &larmorx_hdf5::DatasetInfo,
) -> PyResult<Bound<'py, PyAny>> {
    match &info.dtype {
        Datatype::String { .. } | Datatype::VarLen { string: true, .. } => {
            let strings = file.read_strings(info).map_err(|e| h5_err(path, e))?;
            let items = strings
                .into_iter()
                .map(|s| match s {
                    Some(b) => PyBytes::new(py, &b).into_any(),
                    None => py.None().into_bound(py),
                })
                .collect::<Vec<_>>();
            Ok(PyList::new(py, items)?.into_any())
        }
        Datatype::Float(ft) => {
            let single = ft.size == 4;
            let (v, _) = file
                .read_floats(info, !single, 1, false)
                .map_err(|e| h5_err(path, e))?;
            if single {
                let v: Vec<f32> = v.into_iter().map(|x| x as f32).collect();
                Ok(PyArray::from_vec(py, v).into_any())
            } else {
                Ok(PyArray::from_vec(py, v).into_any())
            }
        }
        other => Err(PyTypeError::new_err(format!(
            "{}: datasets of {other:?} are not read by larmorx",
            path.display()
        ))),
    }
}

/// [`dataset_value`] shaped as h5py gives it: float arrays with the dataset's dimensions, and
/// a scalar dataset as a 0-d array (which, as in h5py, cannot be indexed).
fn dataset_value_shaped<'py>(
    py: Python<'py>,
    file: &File,
    path: &Path,
    info: &larmorx_hdf5::DatasetInfo,
) -> PyResult<Bound<'py, PyAny>> {
    let value = dataset_value(py, file, path, info)?;
    let dims: Vec<usize> = match &info.space {
        larmorx_hdf5::Dataspace::Simple { dims, .. } => dims.iter().map(|&d| d as usize).collect(),
        _ => Vec::new(),
    };
    let numpy = py.import("numpy")?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("dtype", py.import("builtins")?.getattr("object")?)?;
    let array = if value.is_instance_of::<PyList>() {
        numpy.call_method("array", (value,), Some(&kwargs))?
    } else {
        value
    };
    array.call_method1("reshape", (pyo3::types::PyTuple::new(py, dims)?,))
}

/// The members of the group at `group`, in h5py's order (by name, byte for byte), as
/// `(name, datasets)`: `datasets` maps each dataset in the member (when it is a group) to its
/// values, as h5py reads them. `KeyError` when the group does not exist.
#[pyfunction]
fn hdf5_group_members<'py>(
    py: Python<'py>,
    path: PathBuf,
    group: &str,
) -> PyResult<Bound<'py, PyList>> {
    let file = open(&path)?;
    let addr = file.resolve(group).map_err(|_| {
        PyKeyError::new_err(format!(
            "Unable to open object (object '{}' doesn't exist)",
            group.trim_start_matches('/')
        ))
    })?;
    let links = file.links(addr).map_err(|e| h5_err(&path, e))?;
    let mut out = Vec::with_capacity(links.len());
    for link in links {
        let name = String::from_utf8_lossy(&link.name).into_owned();
        let member = format!("{}/{name}", group.trim_end_matches('/'));
        let datasets = PyDict::new(py);
        if let Ok(a) = file.resolve(&member)
            && matches!(file.kind(a), Ok(ObjectKind::Group))
        {
            for d in file.links(a).map_err(|e| h5_err(&path, e))? {
                let dname = String::from_utf8_lossy(&d.name).into_owned();
                let Ok(da) = file.resolve(&format!("{member}/{dname}")) else {
                    continue;
                };
                if !matches!(file.kind(da), Ok(ObjectKind::Dataset)) {
                    continue;
                }
                let info = file.dataset(da).map_err(|e| h5_err(&path, e))?;
                datasets.set_item(dname, dataset_value_shaped(py, &file, &path, &info)?)?;
            }
        }
        out.push((name, datasets));
    }
    PyList::new(py, out)
}

/// Every dataset of the file, depth first in name order (as h5py's `visititems` visits them),
/// as `(path, datatype, shape, value)`: the datatype as `"string"`, `"float<bytes>"`,
/// `"integer<bytes>"` or `"class<n>"`, and the value as [`hdf5_group_members`] reads it,
/// integers as their stored bytes (filters undone, no conversion), `None` for other classes.
/// Groups are listed with the datatype `"group"`.
#[pyfunction]
fn hdf5_datasets<'py>(py: Python<'py>, path: PathBuf) -> PyResult<Bound<'py, PyList>> {
    let file = open(&path)?;
    let out = PyList::empty(py);
    let mut stack = vec![(String::new(), file.root())];
    while let Some((name, addr)) = stack.pop() {
        match file.kind(addr).map_err(|e| h5_err(&path, e))? {
            ObjectKind::Group => {
                if !name.is_empty() {
                    out.append((
                        name.trim_start_matches('/').to_string(),
                        "group",
                        Vec::<u64>::new(),
                        py.None(),
                    ))?;
                }
                let links = file.links(addr).map_err(|e| h5_err(&path, e))?;
                for link in links.into_iter().rev() {
                    if let larmorx_hdf5::LinkTarget::Hard(t) = link.target {
                        let child = format!("{name}/{}", String::from_utf8_lossy(&link.name));
                        stack.push((child, t));
                    }
                }
            }
            ObjectKind::Dataset => {
                let info = file.dataset(addr).map_err(|e| h5_err(&path, e))?;
                let shape = match &info.space {
                    larmorx_hdf5::Dataspace::Simple { dims, .. } => dims.clone(),
                    _ => Vec::new(),
                };
                let kind = match &info.dtype {
                    Datatype::String { .. } | Datatype::VarLen { string: true, .. } => {
                        "string".to_string()
                    }
                    Datatype::Float(ft) => format!("float{}", ft.size),
                    Datatype::Integer { size, .. } => format!("integer{size}"),
                    other => format!("class{}", other.class()),
                };
                let value = if kind == "string" || kind.starts_with("float") {
                    dataset_value(py, &file, &path, &info)?
                } else if kind.starts_with("integer") {
                    // The stored bytes, filters undone (no conversion).
                    let raw = file.read_raw(&info, 1).map_err(|e| h5_err(&path, e))?;
                    PyBytes::new(py, &raw).into_any()
                } else {
                    py.None().into_bound(py)
                };
                out.append((name.trim_start_matches('/').to_string(), kind, shape, value))?;
            }
            _ => {}
        }
    }
    Ok(out)
}

fn string(file: &File, path: &Path, p: &str) -> PyResult<String> {
    let a = file.resolve(p).map_err(|e| h5_err(path, e))?;
    let info = file.dataset(a).map_err(|e| h5_err(path, e))?;
    let s = file.read_strings(&info).map_err(|e| h5_err(path, e))?;
    Ok(s.into_iter()
        .next()
        .flatten()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default())
}

/// The file's bytes with every version-2 object header's four times set to `time` and its
/// first chunk's checksum recomputed (objects reachable from the root group).
fn with_times(bytes: Vec<u8>, time: u32) -> larmorx_hdf5::Result<Vec<u8>> {
    let file = File::from_bytes(bytes.clone())?;
    let mut out = bytes;
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![file.root()];
    while let Some(a) = stack.pop() {
        if !seen.insert(a) {
            continue;
        }
        let oh = file.object_header(a)?;
        if oh.times.is_some() {
            let at = a as usize + 6;
            for k in 0..4 {
                out[at + 4 * k..at + 4 * k + 4].copy_from_slice(&time.to_le_bytes());
            }
            let c = oh.chunks[0];
            let (start, end) = (c.addr as usize, (c.addr + c.size) as usize);
            let sum = larmorx_hdf5::checksum::metadata(&out[start..end - 4]);
            out[end - 4..end].copy_from_slice(&sum.to_le_bytes());
        }
        if matches!(file.kind(a)?, ObjectKind::Group) {
            for link in file.links(a)? {
                if let larmorx_hdf5::LinkTarget::Hard(t) = link.target {
                    stack.push(t);
                }
            }
        }
    }
    Ok(out)
}

/// An HDF5 file's bytes with every object header's times set to `time` (validation compares
/// files written at different times).
#[pyfunction]
fn hdf5_set_times<'py>(py: Python<'py>, data: &[u8], time: u32) -> PyResult<Bound<'py, PyBytes>> {
    let out = with_times(data.to_vec(), time).map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(PyBytes::new(py, &out))
}

/// Replays an ITK-written HDF5 transform file through larmorx's writer, with the file's own
/// object time, version strings and compression, and compares the bytes: once with
/// larmorx's deflate, once with the file's own compressed chunks. Returns a dict:
/// `identical` (every byte), `identical_except_times` (the object headers' times set to the
/// root group's on both sides: ITK records the second each object was written),
/// `identical_with_itk_chunks` (likewise, the chunks compressed as in the file),
/// `size`/`size_with_itk_chunks` (ITK's, larmorx's), `first_difference`, `differing_bytes`
/// (times set), `time`, `compression`, and `chunks`, a list of
/// `(dataset, index, itk_bytes, larmorx_bytes, same_stream)`.
#[pyfunction]
fn itk_h5_replay<'py>(py: Python<'py>, path: PathBuf) -> PyResult<Bound<'py, PyDict>> {
    let original =
        std::fs::read(&path).map_err(|e| PyOSError::new_err(format!("{}: {e}", path.display())))?;
    let file = open(&path)?;
    let mut notices = Notices::new(UpstreamDefects::Reproduce);
    let parts = itk_hdf5::read(&path, Precision::Double, &mut notices)
        .map_err(|e| PyValueError::new_err(e.reason))?;
    let root = file
        .object_header(file.root())
        .map_err(|e| h5_err(&path, e))?;
    let time = root.times.map_or(0, |t| t[0]);
    // ITK's compressed chunks, by their decompressed content, and per dataset.
    let mut streams: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
    let mut itk_chunks = Vec::new();
    let mut compression = false;
    for i in 0..parts.len() {
        let p = format!("/TransformGroup/{i}/TransformParameters");
        let Ok(a) = file.resolve(&p) else { continue };
        let info = file.dataset(a).map_err(|e| h5_err(&path, e))?;
        if !matches!(info.layout, Layout::Chunked { .. }) {
            continue;
        }
        compression = true;
        for (k, c) in file
            .chunks(&info)
            .map_err(|e| h5_err(&path, e))?
            .into_iter()
            .enumerate()
        {
            let stored = file.read_chunk_raw(&c).map_err(|e| h5_err(&path, e))?;
            let plain = larmorx_hdf5::filters::unfilter(&info.pipeline, c.mask, stored.clone())
                .map_err(|e| h5_err(&path, e))?;
            itk_chunks.push((p.clone(), k, stored.clone(), plain.clone()));
            streams.insert(plain, stored);
        }
    }
    let options = WriteOptions {
        compression,
        time,
        itk_version: string(&file, &path, "/ITKVersion")?,
        hdf_version: string(&file, &path, "/HDFVersion")?,
        os_name: string(&file, &path, "/OSName")?,
        os_version: string(&file, &path, "/OSVersion")?,
    };
    let write = |deflate: larmorx_hdf5::write::Deflate| {
        let mut notices = Notices::new(UpstreamDefects::Reproduce);
        itk_hdf5::write_with_deflate(&parts, &options, &mut notices, deflate)
            .map_err(|e| PyValueError::new_err(e.reason))
    };
    let ours = write(Box::new(larmorx_hdf5::write::zlib_rs_deflate))?;
    let with_itk = write(Box::new(move |data: &[u8], level: u32| {
        streams
            .get(data)
            .cloned()
            .unwrap_or_else(|| larmorx_hdf5::write::zlib_rs_deflate(data, level))
    }))?;
    let chunks = itk_chunks
        .into_iter()
        .map(|(p, k, stored, plain)| {
            let mine = larmorx_hdf5::write::zlib_rs_deflate(&plain, 5);
            (p, k, stored.len(), mine.len(), mine == stored)
        })
        .collect::<Vec<_>>();
    let identical = ours == original;
    let original = with_times(original, time).map_err(|e| h5_err(&path, e))?;
    let ours = with_times(ours, time).map_err(|e| h5_err(&path, e))?;
    let with_itk = with_times(with_itk, time).map_err(|e| h5_err(&path, e))?;
    let first = ours.iter().zip(&original).position(|(a, b)| a != b);
    let first = match first {
        None if ours.len() != original.len() => Some(ours.len().min(original.len())),
        f => f,
    };
    let differing = ours.iter().zip(&original).filter(|(a, b)| a != b).count()
        + ours.len().abs_diff(original.len());
    let d = PyDict::new(py);
    d.set_item("identical", identical)?;
    d.set_item("identical_except_times", ours == original)?;
    d.set_item("identical_with_itk_chunks", with_itk == original)?;
    d.set_item("size", (original.len(), ours.len()))?;
    d.set_item("size_with_itk_chunks", with_itk.len())?;
    d.set_item("first_difference", first)?;
    d.set_item("differing_bytes", differing)?;
    d.set_item("time", time)?;
    d.set_item("compression", compression)?;
    d.set_item("chunks", chunks)?;
    Ok(d)
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(hdf5_group_members, m)?)?;
    m.add_function(wrap_pyfunction!(itk_h5_replay, m)?)?;
    m.add_function(wrap_pyfunction!(hdf5_set_times, m)?)?;
    m.add_function(wrap_pyfunction!(hdf5_datasets, m)?)?;
    Ok(())
}
