// SPDX-License-Identifier: Apache-2.0
//! `antsAI`'s blob alignment (`larmorx_ants::ai_blobs`) with everything it computed: the
//! blobs of both images, the correspondence matrix, the pairs, the landmarks and the landmark
//! transform (`lx.ants.blob_alignment`).

use std::collections::HashMap;

use larmorx_ants::ai_blobs::{self, BlobAlignment, BlobFailure, LandmarkKind};
use larmorx_ants::cli::ai::terminate_text;
use larmorx_ants::cli::registration::to_image;
use larmorx_ants::image::read_with;
use larmorx_core::defects::Notices;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use super::{MemoryStore, itk_image_from_tuple};
use crate::defects::tuple_with_notices;

fn blobs_to_py<'py>(py: Python<'py>, blobs: &[ai_blobs::Blob]) -> PyResult<Bound<'py, PyList>> {
    let items = blobs
        .iter()
        .map(|b| {
            (
                b.center.clone(),
                b.sigma,
                b.object_radius(),
                f64::from(b.value),
            )
        })
        .collect::<Vec<_>>();
    PyList::new(py, items)
}

fn alignment_to_py<'py, const D: usize>(
    py: Python<'py>,
    a: &BlobAlignment<D>,
) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("fixed_blobs", blobs_to_py(py, &a.fixed_blobs)?)?;
    d.set_item("moving_blobs", blobs_to_py(py, &a.moving_blobs)?)?;
    d.set_item("correspondence", a.correspondence.clone())?;
    d.set_item("pairs", a.pairs.clone())?;
    d.set_item("landmarks", a.landmarks.clone())?;
    d.set_item(
        "fixed_landmarks",
        a.fixed_landmarks
            .iter()
            .map(|p| p.to_vec())
            .collect::<Vec<_>>(),
    )?;
    d.set_item(
        "moving_landmarks",
        a.moving_landmarks
            .iter()
            .map(|p| p.to_vec())
            .collect::<Vec<_>>(),
    )?;
    d.set_item(
        "matrix",
        a.matrix.iter().map(|r| r.to_vec()).collect::<Vec<_>>(),
    )?;
    d.set_item("offset", a.offset.to_vec())?;
    d.set_item("messages", a.messages.clone())?;
    Ok(d)
}

/// What failed, as the command line would say it.
fn failure_text(f: &BlobFailure) -> String {
    match f {
        BlobFailure::Uncaught(e) => terminate_text(e),
        BlobFailure::EmptyBlobList => "The moving image or fixed image blobs list is empty.\nno blob was found in one of the images: ANTs then reads the offset of a null transform and crashes\n".into(),
        BlobFailure::NoLandmark => "no blob pair kept its distances to 3 others: there is no landmark for the rigid fit (ANTs writes a transform of NaNs, ANTS-114)\n".into(),
    }
}

fn run<const D: usize>(
    store: &mut MemoryStore,
    extract: u32,
    to_match: u32,
    kind: LandmarkKind,
    n_threads: usize,
    notices: &mut Notices,
) -> Result<BlobAlignment<D>, String> {
    let mut read = |name: &str, notices: &mut Notices| -> Result<_, String> {
        let img = read_with::<f32, _>(store, name, D, n_threads, notices)
            .map_err(|e| e.ants_message().unwrap_or_else(|| e.to_string()))?;
        to_image::<f32, D>(img).map_err(|e| e.description)
    };
    let fixed = read("fixed", notices)?;
    let moving = read("moving", notices)?;
    ai_blobs::align(&fixed, &moving, extract, to_match, kind, notices, n_threads)
        .map_err(|f| failure_text(&f))
}

/// `(result dict or None, error text or None, notices)`.
#[pyfunction]
#[pyo3(signature = (fixed, moving, dim, blobs_to_extract, blobs_to_match, affine, n_threads, upstream_defects))]
#[allow(clippy::too_many_arguments)]
fn ants_blob_alignment<'py>(
    py: Python<'py>,
    fixed: &Bound<'py, PyAny>,
    moving: &Bound<'py, PyAny>,
    dim: usize,
    blobs_to_extract: u32,
    blobs_to_match: u32,
    affine: bool,
    n_threads: usize,
    upstream_defects: &str,
) -> PyResult<Bound<'py, pyo3::types::PyTuple>> {
    let mode = crate::defects::mode_from_py(upstream_defects)?;
    let mut store = MemoryStore {
        inputs: HashMap::new(),
        aliases: HashMap::new(),
        outputs: HashMap::new(),
        capture: Vec::new(),
    };
    for (name, value) in [("fixed", fixed), ("moving", moving)] {
        if let Ok(path) = value.extract::<String>() {
            store.aliases.insert(name.to_owned(), path);
        } else {
            store
                .inputs
                .insert(name.to_owned(), itk_image_from_tuple(value)?);
        }
    }
    let kind = if affine {
        LandmarkKind::Affine
    } else {
        LandmarkKind::Rigid
    };
    let mut notices = Notices::new(mode);
    let none = || py.None().into_bound(py);
    let (value, error) = match dim {
        2 => match py.detach(|| {
            run::<2>(
                &mut store,
                blobs_to_extract,
                blobs_to_match,
                kind,
                n_threads,
                &mut notices,
            )
        }) {
            Ok(a) => (alignment_to_py(py, &a)?.into_any(), none()),
            Err(e) => (none(), e.into_pyobject(py)?.into_any()),
        },
        3 => match py.detach(|| {
            run::<3>(
                &mut store,
                blobs_to_extract,
                blobs_to_match,
                kind,
                n_threads,
                &mut notices,
            )
        }) {
            Ok(a) => (alignment_to_py(py, &a)?.into_any(), none()),
            Err(e) => (none(), e.into_pyobject(py)?.into_any()),
        },
        _ => {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "the blob alignment is 2D or 3D, not {dim}D"
            )));
        }
    };
    tuple_with_notices(py, vec![value, error], notices.into_vec())
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ants_blob_alignment, m)?)?;
    Ok(())
}
