// SPDX-License-Identifier: Apache-2.0
//! The outputs (`specs/t2smap.md` §11): names, the existence checks, the images, the table,
//! the JSON files.

use std::path::{Path, PathBuf};

use larmorx_core::affine::Affine;
use larmorx_core::element::Element;
use larmorx_core::python::json::{self, Value};
use larmorx_core::python::number::float_repr;
use larmorx_io::nifti::{self, NiftiVersion, WriteOptions};
use larmorx_transform::numpy_linalg::LapackRelease;
use ndarray::{ArrayD, IxDyn, ShapeBuilder};

use crate::log::PyError;
use crate::options::{Convention, Libraries};

/// The registry's descriptions with their `bids` and `orig` names (§11.1).
const NAMES: [(&str, &str, &str); 14] = [
    (
        "adaptive mask img",
        "desc-adaptiveGoodSignal_mask",
        "adaptive_mask",
    ),
    (
        "fit failures img",
        "desc-decayFitFailures_mask",
        "fit_failures",
    ),
    (
        "t2star variance img",
        "stat-variance_desc-t2star_statmap",
        "t2s_var",
    ),
    ("s0 variance img", "stat-variance_desc-s0_statmap", "s0_var"),
    (
        "t2star-s0 covariance img",
        "stat-covariance_desc-t2star+s0_statmap",
        "t2s_s0_covar",
    ),
    ("s0 img", "S0map", "s0vG"),
    ("rmse img", "desc-rmse_statmap", "rmse"),
    ("confounds tsv", "desc-confounds_timeseries", "confounds"),
    ("limited s0 img", "desc-limited_S0map", "s0v"),
    ("limited t2star img", "desc-limited_T2starmap", "t2sv"),
    ("t2star img", "T2starmap", "t2svG"),
    ("combined img", "desc-optcom_bold", "ts_OC"),
    (
        "data description json",
        "dataset_description",
        "dataset_description",
    ),
    ("registry json", "desc-tedana_registry", "registry"),
];

/// x86-64's default float32 NaN, which every NaN the original writes is (§13.1).
const NAN32: u32 = 0xFFC0_0000;

/// The output directory, its file names and what has been written (the registry).
pub struct Outputs {
    pub dir: PathBuf,
    prefix: String,
    convention: Convention,
    overwrite: bool,
    /// (description, file name), in the order written.
    pub registry: Vec<(String, String)>,
}

/// `--prefix` with `_` appended unless empty or already ending with it (§2.2).
pub fn prefix(p: &str) -> String {
    if p.is_empty() || p.ends_with('_') {
        p.to_owned()
    } else {
        format!("{p}_")
    }
}

impl Outputs {
    pub fn new(dir: PathBuf, prefix_text: &str, convention: Convention, overwrite: bool) -> Self {
        Outputs {
            dir,
            prefix: prefix(prefix_text),
            convention,
            overwrite,
            registry: Vec::new(),
        }
    }

    /// `<prefix><name><extension>` of a registry description.
    pub fn name(&self, desc: &str) -> String {
        let (_, bids, orig) = NAMES
            .iter()
            .find(|(d, _, _)| *d == desc)
            .expect("a known description");
        let base = match self.convention {
            Convention::Bids => bids,
            Convention::Orig => orig,
        };
        let ext = if desc.ends_with("tsv") {
            ".tsv"
        } else if desc.ends_with("json") {
            ".json"
        } else {
            ".nii.gz"
        };
        format!("{}{base}{ext}", self.prefix)
    }

    /// The path of `desc`'s file, after the existence check of §11.2.
    pub fn claim(&self, desc: &str) -> Result<PathBuf, PyError> {
        let path = self.dir.join(self.name(desc));
        if path.exists() && !self.overwrite {
            return Err(PyError::new(
                "RuntimeError",
                format!(
                    "File {} already exists. In order to allow overwrite please use the \
                     --overwrite option in the command line or the overwrite parameter in the \
                     Python API.",
                    path.display()
                ),
            ));
        }
        Ok(path)
    }

    fn written(&mut self, desc: &str) {
        let name = self.name(desc);
        self.registry.push((desc.to_owned(), name));
    }

    /// Writes a text file atomically (a temporary file in the directory, renamed over).
    fn write_text(&mut self, desc: &str, text: &str) -> Result<(), PyError> {
        let path = self.claim(desc)?;
        let tmp = temp_path(&path);
        std::fs::write(&tmp, text.as_bytes())
            .and_then(|()| rename(&tmp, &path))
            .map_err(|e| os_error(&path, &e))?;
        self.written(desc);
        Ok(())
    }

    /// The confounds table (§11.4).
    pub fn table(
        &mut self,
        columns: &[&str],
        rows: &[[f64; 9]],
        percentiles_f32: bool,
    ) -> Result<(), PyError> {
        let mut s = columns.join("\t");
        s.push('\n');
        for row in rows {
            let cells: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(i, &v)| {
                    let v = if i >= 2 && percentiles_f32 {
                        v as f32 as f64
                    } else {
                        v
                    };
                    if v.is_nan() {
                        "n/a".to_owned()
                    } else {
                        float_repr(v)
                    }
                })
                .collect();
            s.push_str(&cells.join("\t"));
            s.push('\n');
        }
        self.write_text("confounds tsv", &s)
    }

    /// A JSON file as `json.dump(..., indent=4, sort_keys=True)` writes it, no final newline.
    pub fn json(&mut self, desc: &str, v: &Value) -> Result<(), PyError> {
        let text = json::dumps_indent(v, 4, true);
        self.write_text(desc, &text)
    }

    /// The registry (§11.5): every description written before it.
    pub fn registry_json(&mut self) -> Result<(), PyError> {
        let v = Value::Dict(
            self.registry
                .iter()
                .map(|(d, n)| (d.clone(), Value::str(n.clone())))
                .collect(),
        );
        self.json("registry json", &v)
    }
}

fn temp_path(path: &Path) -> PathBuf {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("out");
    path.with_file_name(format!(".larmorx-{}-{name}", std::process::id()))
}

/// A rename over the target; on Windows, retried briefly while another program holds it.
fn rename(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut last = None;
    for _ in 0..20 {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if cfg!(windows) => {
                last = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                let _ = std::fs::remove_file(from);
                return Err(e);
            }
        }
    }
    let _ = std::fs::remove_file(from);
    Err(last.expect("an error"))
}

fn os_error(path: &Path, e: &std::io::Error) -> PyError {
    PyError::new("OSError", format!("{}: {e}", path.display()))
}

/// How the images are laid out on the grid and their headers (§11.3).
pub struct ImageSpec {
    /// The grid's three sizes.
    pub grid: [usize; 3],
    /// The mask's voxels as Fortran offsets of a grid volume, in voxel order.
    pub offsets: Vec<usize>,
    /// The mask image's affine (the outputs carry it).
    pub affine: Affine,
    /// The reference's four voxel sizes (the 4D images').
    pub zooms: [f64; 4],
    pub version: NiftiVersion,
    pub lapack: LapackRelease,
    pub n_threads: usize,
}

impl ImageSpec {
    /// The LAPACK release of the libraries' numpy (nibabel's qform arithmetic).
    pub fn lapack_for(libraries: Libraries) -> LapackRelease {
        match libraries {
            Libraries::Newest => LapackRelease::V3_12_1,
            Libraries::Shared => LapackRelease::V3_12_0,
        }
    }
}

/// A value the images hold: float32 (NaNs written as x86-64's default NaN), int32, uint8.
pub trait OutValue: Element + Copy {
    fn canonical(self) -> Self {
        self
    }
}

impl OutValue for f32 {
    fn canonical(self) -> Self {
        if self.is_nan() {
            f32::from_bits(NAN32)
        } else {
            self
        }
    }
}

impl OutValue for i32 {}

impl OutValue for u8 {}

impl Outputs {
    /// Writes an image of `values` (`[j * cols + t]` over the mask's voxels, 0 elsewhere): 3D
    /// when `four_d` is false (one column), else `cols` volumes.
    pub fn image<T: OutValue + Default>(
        &mut self,
        desc: &str,
        spec: &ImageSpec,
        values: &[T],
        cols: usize,
        four_d: bool,
    ) -> Result<(), PyError> {
        let path = self.claim(desc)?;
        let [nx, ny, nz] = spec.grid;
        let nvox = nx * ny * nz;
        let mut buf = vec![T::default(); nvox * cols];
        for (j, &f) in spec.offsets.iter().enumerate() {
            for t in 0..cols {
                buf[f + t * nvox] = values[j * cols + t].canonical();
            }
        }
        let shape: Vec<usize> = if four_d {
            vec![nx, ny, nz, cols]
        } else {
            vec![nx, ny, nz]
        };
        let mut hdr = nifti::header_for_image_nibabel(
            None,
            Some(spec.version),
            &shape,
            <T as Element>::DATA_TYPE,
            &spec.affine,
            spec.lapack,
        )
        .map_err(|e| PyError::new("nibabel.spatialimages.HeaderDataError", e.to_string()))?;
        hdr.scl_slope = f64::NAN;
        hdr.scl_inter = f64::NAN;
        hdr.xyzt_units = 0;
        if four_d {
            hdr.pixdim[1..5].copy_from_slice(&spec.zooms);
        }
        let arr = ArrayD::from_shape_vec(IxDyn(&shape).f(), buf).expect("the grid's size");
        let tmp = temp_path(&path);
        nifti::write(
            &tmp,
            &hdr,
            arr.view(),
            &WriteOptions {
                compression_level: 2,
                n_threads: spec.n_threads,
            },
        )
        .map_err(|e| PyError::new("OSError", e.to_string()))?;
        rename(&tmp, &path).map_err(|e| os_error(&path, &e))?;
        self.written(desc);
        Ok(())
    }
}

/// `dataset_description.json` (§11.5), D6's `Python` and `Python_Libraries`.
pub fn dataset_description(command: &str, libraries: Libraries) -> Value {
    let (numpy, scipy) = match libraries {
        Libraries::Newest => ("2.5.4", "1.18.1"),
        Libraries::Shared => ("2.3.5", "1.15.3"),
    };
    let libs = [
        ("bokeh", "3.10.0"),
        ("mapca", "0.0.8"),
        ("matplotlib", "3.11.2"),
        ("nibabel", "5.4.2"),
        ("nilearn", "0.14.1"),
        ("numpy", numpy),
        ("pandas", "3.0.6"),
        ("robustica", "0.1.4"),
        ("scikit-learn", "1.9.1"),
        ("scipy", scipy),
        ("threadpoolctl", "3.7.0"),
        ("tqdm", "4.70.1"),
    ];
    let node = uname();
    Value::Dict(vec![
        ("BIDSVersion".into(), Value::str("1.5.0")),
        ("DatasetType".into(), Value::str("derivative")),
        (
            "GeneratedBy".into(),
            Value::List(vec![Value::Dict(vec![
                (
                    "CodeURL".into(),
                    Value::str("https://github.com/ME-ICA/tedana"),
                ),
                ("Command".into(), Value::str(command)),
                (
                    "Description".into(),
                    Value::str(
                        "A pipeline estimating T2* from multi-echo fMRI data and combining data \
                         across echoes.",
                    ),
                ),
                ("Name".into(), Value::str("t2smap")),
                (
                    "Node".into(),
                    Value::Dict(
                        node.into_iter()
                            .map(|(k, v)| (k.to_owned(), Value::str(v)))
                            .collect(),
                    ),
                ),
                (
                    "Python".into(),
                    Value::str(format!(
                        "larmorx {} (a clean-room implementation of tedana 26.0.3's t2smap, \
                         with the results of Python 3.12.16 and the libraries listed)",
                        larmorx_core::VERSION
                    )),
                ),
                (
                    "Python_Libraries".into(),
                    Value::Dict(
                        libs.iter()
                            .map(|(k, v)| ((*k).to_owned(), Value::str(*v)))
                            .collect(),
                    ),
                ),
                ("Version".into(), Value::str("26.0.3")),
            ])]),
        ),
        ("Name".into(), Value::str("t2smap Outputs")),
    ])
}

/// The host as Python's `platform.uname()` describes it (not compared: §17.3).
fn uname() -> Vec<(&'static str, String)> {
    let read = |p: &str| {
        std::fs::read_to_string(p)
            .map(|s| s.trim().to_owned())
            .unwrap_or_default()
    };
    let system = match std::env::consts::OS {
        "linux" => "Linux",
        "macos" => "Darwin",
        "windows" => "Windows",
        other => other,
    };
    let (node, release, version) = if cfg!(target_os = "linux") {
        (
            read("/proc/sys/kernel/hostname"),
            read("/proc/sys/kernel/osrelease"),
            read("/proc/sys/kernel/version"),
        )
    } else {
        (
            std::env::var("COMPUTERNAME")
                .or_else(|_| std::env::var("HOSTNAME"))
                .unwrap_or_default(),
            String::new(),
            String::new(),
        )
    };
    let machine = match std::env::consts::ARCH {
        "x86_64" if cfg!(windows) => "AMD64",
        "aarch64" if cfg!(windows) => "ARM64",
        "aarch64" if cfg!(target_os = "macos") => "arm64",
        a => a,
    };
    vec![
        ("Machine", machine.to_owned()),
        ("Name", node),
        ("Processor", machine.to_owned()),
        ("Release", release),
        ("System", system.to_owned()),
        ("Version", version),
    ]
}
