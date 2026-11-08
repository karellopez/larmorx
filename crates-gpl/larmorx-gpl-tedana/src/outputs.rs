// SPDX-License-Identifier: GPL-3.0-only
// Translated from tedana 26.0.3 (Copyright 2023, The ME-ICA Developers; GNU LGPL 2.1), under the
// GNU GPL version 3 as section 3 of the LGPL 2.1 allows; modified 2026 by Karel Lopez Vilaret.
//! The files t2smap writes, as `tedana.io.OutputGenerator` names and saves them:
//!
//! - names from tedana's `outputs.json` for the convention (`bids`, which tedana calls
//!   `bidsv1.5.0`, or `orig`), the prefix, and `.nii.gz`, `.tsv` or `.json`; an existing file
//!   without `--overwrite` is tedana's `RuntimeError` before anything is written;
//! - images through nilearn's `masking.unmask` and `new_img_like(..., copy_header=False)`: the
//!   values put back at the mask's voxels (zeros elsewhere) in a **fresh** nibabel header with
//!   the mask's affine (sform code 2, qform code 0 with nibabel's quaternion; an affine nibabel
//!   finds close to the fresh header's own keeps both codes 0), float64 stored as float32 and
//!   int64 as int32, a 4D image given the reference's zooms (`set_zooms`, the repetition time
//!   included), gzip-compressed;
//! - the confounds table as pandas' `to_csv(sep="\t", lineterminator="\n", na_rep="n/a",
//!   index=False)` writes it (numpy's shortest digits of each value's type);
//! - JSON as `json.dump(indent=4, sort_keys=True)`: `dataset_description.json` and the
//!   registry of the files written (description to file name).

use std::path::Path;

use larmorx_core::Affine;
use larmorx_core::DataType;
use larmorx_core::ndarray::{ArrayD, IxDyn, ShapeBuilder};
use larmorx_core::numpy::format::scalar_str;
use larmorx_core::python::json::{Value, dumps_indent};
use larmorx_io::nifti::{self, NiftiHeader, NiftiVersion, WriteOptions};
use larmorx_transform::numpy_linalg::LapackRelease;

use crate::log::PyError;
use crate::maps::Rmse;

/// The descriptions of t2smap's files and their names in tedana's `outputs.json`:
/// (description, `orig` name, `bidsv1.5.0` name).
const NAMES: &[(&str, &str, &str)] = &[
    (
        "adaptive mask img",
        "adaptive_mask",
        "desc-adaptiveGoodSignal_mask",
    ),
    (
        "fit failures img",
        "fit_failures",
        "desc-decayFitFailures_mask",
    ),
    (
        "t2star variance img",
        "t2s_var",
        "stat-variance_desc-t2star_statmap",
    ),
    ("s0 variance img", "s0_var", "stat-variance_desc-s0_statmap"),
    (
        "t2star-s0 covariance img",
        "t2s_s0_covar",
        "stat-covariance_desc-t2star+s0_statmap",
    ),
    ("s0 img", "s0vG", "S0map"),
    ("rmse img", "rmse", "desc-rmse_statmap"),
    ("confounds tsv", "confounds", "desc-confounds_timeseries"),
    ("limited s0 img", "s0v", "desc-limited_S0map"),
    ("limited t2star img", "t2sv", "desc-limited_T2starmap"),
    ("t2star img", "t2svG", "T2starmap"),
    ("combined img", "ts_OC", "desc-optcom_bold"),
    (
        "data description json",
        "dataset_description",
        "dataset_description",
    ),
    ("registry json", "registry", "desc-tedana_registry"),
];

/// `OutputGenerator.get_name(description)` without the folder: prefix, name, extension.
pub fn file_name(description: &str, convention: &str, prefix: &str) -> String {
    let (_, orig, bids) = NAMES
        .iter()
        .find(|(d, _, _)| *d == description)
        .expect("a description of t2smap's outputs");
    let name = if convention == "orig" { orig } else { bids };
    let ext = if description.ends_with("img") {
        if name.ends_with(".nii") || name.ends_with(".nii.gz") {
            ""
        } else {
            ".nii.gz"
        }
    } else if description.ends_with("json") {
        ".json"
    } else {
        ".tsv"
    };
    format!("{prefix}{name}{ext}")
}

/// The image values, as tedana passes them to `save_img` (after its 64-to-32-bit cast).
pub enum ImgData<'a> {
    F32(&'a [f32]),
    I32(&'a [i32]),
    U8(&'a [u8]),
}

impl ImgData<'_> {
    /// The stored values' bits.
    fn bits(&self) -> Vec<u32> {
        match self {
            ImgData::F32(v) => v.iter().map(|x| x.to_bits()).collect(),
            ImgData::I32(v) => v.iter().map(|&x| x as u32).collect(),
            ImgData::U8(v) => v.iter().map(|&x| u32::from(x)).collect(),
        }
    }
}

/// An image's values as a pass saved them: the voxels (Fortran-order indices), the volumes
/// per voxel and each value's bits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeptImg {
    pub description: String,
    pub voxels: Vec<usize>,
    pub cols: usize,
    pub bits: Vec<u32>,
}

impl KeptImg {
    /// The values on the whole grid of `nxyz` voxels (zeros elsewhere, as written).
    fn grid(&self, nxyz: usize, cols: usize) -> Vec<u32> {
        let mut out = vec![0u32; nxyz * cols];
        for (k, &vox) in self.voxels.iter().enumerate() {
            for c in 0..self.cols.min(cols) {
                out[vox + c * nxyz] = self.bits[k * self.cols + c];
            }
        }
        out
    }
}

/// What the first pass saved of the files that depend on the maths: the images' values and
/// the confounds table's text (the maths' comparison, `crate::maths`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Kept {
    pub images: Vec<KeptImg>,
    pub tables: Vec<(String, String)>,
}

/// Where the saved values go.
#[derive(Clone, Debug, Default)]
pub enum Sink {
    /// Write the files (the run).
    #[default]
    Files,
    /// Write the files and keep their values ([`Outputs::kept`]).
    Keep,
    /// Write nothing and count, per file, the values that differ from the first pass's
    /// ([`Outputs::diffs`]).
    Compare(std::sync::Arc<Kept>),
}

/// One file's values that differ from the first pass's: (file name, values that differ).
pub type Diff = (String, usize);

/// `OutputGenerator` with the mask registered.
pub struct Outputs {
    pub out_dir: String,
    pub prefix: String,
    pub convention: String,
    pub overwrite: bool,
    /// The mask image's spatial shape and affine (`masking.unmask`'s output space).
    pub shape: [usize; 3],
    pub affine: Affine,
    /// The mask's voxels (Fortran-order indices), in the order of the masked rows.
    pub voxels: Vec<usize>,
    /// `reference_img.header.get_zooms()`.
    pub ref_zooms: Vec<f64>,
    /// nibabel's qform arithmetic (numpy's LAPACK).
    pub release: LapackRelease,
    pub n_threads: usize,
    /// `self.registry`: description and file name, in the order saved.
    pub registry: Vec<(String, String)>,
    /// Where the values go.
    pub sink: Sink,
    /// The values kept ([`Sink::Keep`]).
    pub kept: Kept,
    /// The values that differ from the first pass's ([`Sink::Compare`]), in the order saved.
    pub diffs: Vec<Diff>,
}

impl Outputs {
    /// `get_name(description)`: the full path.
    pub fn path(&self, description: &str) -> String {
        format!(
            "{}/{}",
            self.out_dir,
            file_name(description, &self.convention, &self.prefix)
        )
    }

    /// `save_file`'s first step: the name, or tedana's `RuntimeError` for an existing file.
    fn claim(&self, description: &str) -> Result<String, PyError> {
        let name = self.path(description);
        if Path::new(&name).exists() && !self.overwrite {
            return Err(PyError::new(
                "RuntimeError",
                format!(
                    "File {name} already exists. In order to allow overwrite please use the \
                     --overwrite option in the command line or the overwrite parameter in the \
                     Python API."
                ),
                "tedana/io.py, in save_file",
            ));
        }
        Ok(name)
    }

    fn register(&mut self, description: &str, name: &str) {
        let base = Path::new(name)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Some(e) = self.registry.iter_mut().find(|(d, _)| d == description) {
            e.1 = base;
        } else {
            self.registry.push((description.to_owned(), base));
        }
    }

    /// The header nibabel writes for a fresh `Nifti1Image(data, affine)`, after `set_zooms`
    /// for a 4D image and the update at writing.
    fn header(&self, shape: &[usize], dtype: DataType) -> Result<NiftiHeader, PyError> {
        let herr = |e: larmorx_io::nifti::HeaderError| {
            PyError::new(
                "nibabel.spatialimages.HeaderDataError",
                e.to_string(),
                "nibabel/nifti1.py",
            )
        };
        let fresh = NiftiHeader::new(NiftiVersion::V1, shape, dtype).map_err(herr)?;
        let close = |h: &NiftiHeader| {
            h.best_affine_nibabel()
                .is_ok_and(|b| self.affine.allclose(&b, 1e-5, 1e-8))
        };
        let mut hdr = fresh.clone();
        if !close(&fresh) {
            hdr.set_affine_nibabel(&self.affine, self.release)
                .map_err(herr)?;
        }
        if shape.len() == 4 {
            if self.ref_zooms.len() != 4 {
                return Err(PyError::new(
                    "nibabel.spatialimages.HeaderDataError",
                    "Expecting 4 zoom values for ndim 4",
                    "nibabel/analyze.py, in set_zooms",
                ));
            }
            if self.ref_zooms.iter().any(|&z| z < 0.0) {
                return Err(PyError::new(
                    "nibabel.spatialimages.HeaderDataError",
                    "zooms must be positive",
                    "nibabel/analyze.py, in set_zooms",
                ));
            }
            for (k, &z) in self.ref_zooms.iter().enumerate() {
                hdr.pixdim[k + 1] = f64::from(z as f32);
            }
            if !close(&hdr) {
                hdr.set_affine_nibabel(&self.affine, self.release)
                    .map_err(herr)?;
            }
        }
        hdr.scl_slope = 1.0;
        hdr.scl_inter = 0.0;
        Ok(hdr)
    }

    /// `save_file(data, description[, mask=subset])` of an image: `data` holds one value per
    /// voxel of the mask (or of `subset` of the mask's voxels) for a 3D image (`volumes`
    /// `None`), or `volumes` values per voxel for a 4D one (tedana's 2D arrays, even of one
    /// column).
    pub fn save_img(
        &mut self,
        description: &str,
        data: ImgData<'_>,
        volumes: Option<usize>,
        subset: Option<&[bool]>,
    ) -> Result<(), PyError> {
        let cols = volumes.unwrap_or(1);
        let compare = matches!(self.sink, Sink::Compare(_));
        let name = if compare {
            self.path(description)
        } else {
            self.claim(description)?
        };
        let voxels: Vec<usize> = match subset {
            None => self.voxels.clone(),
            Some(s) => self
                .voxels
                .iter()
                .zip(s)
                .filter(|&(_, &b)| b)
                .map(|(&v, _)| v)
                .collect(),
        };
        if voxels.is_empty() {
            // nilearn's load_mask_img of the (sub)mask in masking.unmask.
            return Err(PyError::value_error(
                "The mask is invalid as it is empty: it masks all data.",
                "nilearn/masking.py, in load_mask_img",
            ));
        }
        let nxyz: usize = self.shape.iter().product();
        if let Sink::Compare(first) = &self.sink {
            let bits = data.bits();
            let n = match first.images.iter().find(|k| k.description == description) {
                None => bits.len(),
                Some(k) if k.voxels == voxels && k.cols == cols => {
                    k.bits.iter().zip(&bits).filter(|(a, b)| a != b).count()
                }
                Some(k) => {
                    let mine = KeptImg {
                        description: description.to_owned(),
                        voxels,
                        cols,
                        bits,
                    };
                    let c = cols.max(k.cols);
                    let (a, b) = (k.grid(nxyz, c), mine.grid(nxyz, c));
                    a.iter().zip(&b).filter(|(x, y)| x != y).count()
                }
            };
            let file = file_name(description, &self.convention, &self.prefix);
            self.diffs.push((file, n));
            self.register(description, &name);
            return Ok(());
        }
        if matches!(self.sink, Sink::Keep) {
            self.kept.images.push(KeptImg {
                description: description.to_owned(),
                voxels: voxels.clone(),
                cols,
                bits: data.bits(),
            });
        }
        let mut shape = self.shape.to_vec();
        if let Some(t) = volumes {
            shape.push(t);
        }
        fn scatter<T: Copy + Default>(
            v: &[T],
            voxels: &[usize],
            nxyz: usize,
            cols: usize,
        ) -> Vec<T> {
            let mut out = vec![T::default(); nxyz * cols];
            for (k, &vox) in voxels.iter().enumerate() {
                for c in 0..cols {
                    out[vox + c * nxyz] = v[k * cols + c];
                }
            }
            out
        }
        let write_err = |e: nifti::Error| PyError::new("OSError", e.to_string(), "nibabel");
        let opts = WriteOptions {
            compression_level: 2,
            n_threads: self.n_threads,
        };
        let dims = IxDyn(&shape).f();
        match data {
            ImgData::F32(v) => {
                let hdr = self.header(&shape, DataType::F32)?;
                let a =
                    ArrayD::from_shape_vec(dims, scatter(v, &voxels, nxyz, cols)).expect("shape");
                nifti::write(&name, &hdr, a.view(), &opts).map_err(write_err)?;
            }
            ImgData::I32(v) => {
                let hdr = self.header(&shape, DataType::I32)?;
                let a =
                    ArrayD::from_shape_vec(dims, scatter(v, &voxels, nxyz, cols)).expect("shape");
                nifti::write(&name, &hdr, a.view(), &opts).map_err(write_err)?;
            }
            ImgData::U8(v) => {
                let hdr = self.header(&shape, DataType::U8)?;
                let a =
                    ArrayD::from_shape_vec(dims, scatter(v, &voxels, nxyz, cols)).expect("shape");
                nifti::write(&name, &hdr, a.view(), &opts).map_err(write_err)?;
            }
        }
        self.register(description, &name);
        Ok(())
    }

    /// `save_file(rmse_df, "confounds tsv")`.
    pub fn save_tsv(&mut self, description: &str, text: &str) -> Result<(), PyError> {
        if let Sink::Compare(first) = &self.sink {
            let n = match first.tables.iter().find(|(d, _)| d == description) {
                None => text.lines().map(|l| l.split('\t').count()).sum(),
                Some((_, t)) => table_cells_differing(t, text),
            };
            let name = self.path(description);
            let file = file_name(description, &self.convention, &self.prefix);
            self.diffs.push((file, n));
            self.register(description, &name);
            return Ok(());
        }
        if matches!(self.sink, Sink::Keep) {
            self.kept
                .tables
                .push((description.to_owned(), text.to_owned()));
        }
        let name = self.claim(description)?;
        std::fs::write(&name, text.as_bytes())
            .map_err(|e| PyError::new("OSError", e.to_string(), "pandas"))?;
        self.register(description, &name);
        Ok(())
    }

    /// `save_file(dict, description)` of a JSON file.
    pub fn save_json(&mut self, description: &str, v: &Value) -> Result<(), PyError> {
        let name = self.claim(description)?;
        std::fs::write(&name, dumps_indent(v, 4, true).as_bytes())
            .map_err(|e| PyError::new("OSError", e.to_string(), "tedana/io.py, in save_json"))?;
        self.register(description, &name);
        Ok(())
    }

    /// `save_self()`: the registry as it is before its own entry.
    pub fn save_self(&mut self) -> Result<(), PyError> {
        let v = Value::Dict(
            self.registry
                .iter()
                .map(|(d, n)| (d.clone(), Value::str(n.clone())))
                .collect(),
        );
        self.save_json("registry json", &v)
    }
}

/// The cells of two tab-separated tables that differ (a missing cell counts as differing).
fn table_cells_differing(a: &str, b: &str) -> usize {
    let rows = |t: &str| -> Vec<Vec<String>> {
        t.lines()
            .map(|l| l.split('\t').map(str::to_owned).collect())
            .collect()
    };
    let (ra, rb) = (rows(a), rows(b));
    let mut n = 0;
    for i in 0..ra.len().max(rb.len()) {
        let (x, y) = (ra.get(i), rb.get(i));
        let len = x.map_or(0, Vec::len).max(y.map_or(0, Vec::len));
        for j in 0..len {
            if x.and_then(|r| r.get(j)) != y.and_then(|r| r.get(j)) {
                n += 1;
            }
        }
    }
    n
}

/// The confounds table (`rmse_df`) as `to_csv` writes it: float64 columns (numpy's shortest
/// float64 digits; the float32 mean and standard deviation widened), or all float32 when the
/// percentiles are (`Percentiles::float32`), NaN as `n/a`.
pub fn confounds_tsv(r: &Rmse) -> String {
    let names = [
        "rmse_mean",
        "rmse_std",
        "rmse_min",
        "rmse_percentile02",
        "rmse_percentile25",
        "rmse_median",
        "rmse_percentile75",
        "rmse_percentile98",
        "rmse_max",
    ];
    let t = r.mean.len();
    let f32s = r.percentiles.float32;
    let cell = |v: f64| -> String {
        if v.is_nan() {
            "n/a".to_owned()
        } else if f32s {
            scalar_str(v as f32)
        } else {
            scalar_str(v)
        }
    };
    let mut out = names.join("\t");
    out.push('\n');
    for i in 0..t {
        let mut row = vec![cell(f64::from(r.mean[i])), cell(f64::from(r.std[i]))];
        for q in 0..7 {
            row.push(cell(r.percentiles.values[q * t + i]));
        }
        out.push_str(&row.join("\t"));
        out.push('\n');
    }
    out
}

/// `platform.uname()`'s fields tedana records: (system, node, release, version, machine,
/// processor), as Python finds them (`uname` on Unix, the environment on Windows).
pub fn uname() -> [String; 6] {
    if cfg!(windows) {
        let var = |k: &str| std::env::var(k).unwrap_or_default();
        let ver = std::process::Command::new("cmd")
            .args(["/c", "ver"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        // "Microsoft Windows [Version 10.0.22631.4602]" -> "10.0.22631"
        let version = ver
            .split("Version ")
            .nth(1)
            .and_then(|s| s.split(']').next())
            .map(|v| v.split('.').take(3).collect::<Vec<_>>().join("."))
            .unwrap_or_default();
        let build: u32 = version
            .split('.')
            .nth(2)
            .and_then(|b| b.parse().ok())
            .unwrap_or(0);
        let release = if version.is_empty() {
            String::new()
        } else if build >= 22000 {
            "11".to_owned()
        } else {
            "10".to_owned()
        };
        [
            "Windows".to_owned(),
            var("COMPUTERNAME"),
            release,
            version,
            var("PROCESSOR_ARCHITECTURE"),
            var("PROCESSOR_IDENTIFIER"),
        ]
    } else {
        let field = |flag: &str| {
            std::process::Command::new("uname")
                .arg(flag)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| {
                    let s = String::from_utf8_lossy(&o.stdout).into_owned();
                    s.strip_suffix('\n').map(str::to_owned).unwrap_or(s)
                })
                .unwrap_or_default()
        };
        // Python strips `uname -p`'s output; the other fields are os.uname()'s as they are.
        let processor = field("-p").trim().to_owned();
        let processor = if processor == "unknown" {
            String::new()
        } else {
            processor
        };
        [
            field("-s"),
            field("-n"),
            field("-r"),
            field("-v"),
            field("-m"),
            processor,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_as_tedana() {
        assert_eq!(
            file_name("t2star img", "bidsv1.5.0", ""),
            "T2starmap.nii.gz"
        );
        assert_eq!(file_name("t2star img", "orig", "sub_"), "sub_t2svG.nii.gz");
        assert_eq!(
            file_name("confounds tsv", "bidsv1.5.0", ""),
            "desc-confounds_timeseries.tsv"
        );
        assert_eq!(
            file_name("registry json", "bidsv1.5.0", "x_"),
            "x_desc-tedana_registry.json"
        );
    }

    #[test]
    fn differing_cells() {
        assert_eq!(table_cells_differing("a\tb\n1\t2\n", "a\tb\n1\t2\n"), 0);
        assert_eq!(table_cells_differing("a\tb\n1\t2\n", "a\tb\n1\t3\n"), 1);
        assert_eq!(table_cells_differing("a\tb\n1\t2\n", "a\tb\n"), 2);
        let k = KeptImg {
            description: String::new(),
            voxels: vec![3, 1],
            cols: 2,
            bits: vec![1, 2, 3, 4],
        };
        assert_eq!(k.grid(4, 2), vec![0, 3, 0, 1, 0, 4, 0, 2]);
    }
}
