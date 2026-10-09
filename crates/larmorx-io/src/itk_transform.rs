// SPDX-License-Identifier: Apache-2.0
//! ITK transform files: text (`.txt`, `.tfm`), MATLAB v4 (`.mat`), and displacement fields
//! stored as NIfTI vector images (`.nii`, `.nii.gz`).
//!
//! Follows ITK v5.4.5's `TxtTransformIO` and `MatlabTransformIO`: a file holds a list of
//! transforms; when the first is a `CompositeTransform`, the following ones are its components.
//! Like ANTs, [`read_itk_transform`] returns the first transform of the list. HDF5 (`.h5`)
//! composites are read by the Python layer for now (PLAN.md §4.1).

use std::fmt::Write as _;
use std::path::Path;

use larmorx_core::Grid3;
use larmorx_core::linalg::Mat3;
use larmorx_transform::{DisplacementField, FieldData, LinearTransform, Transform, TransformError};

use crate::nifti;

/// A transform file cannot be read or written.
#[derive(Debug, thiserror::Error)]
pub enum TransformFileError {
    #[error("{}: {source}", path.display())]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{}: {message}", path.display())]
    Format {
        path: std::path::PathBuf,
        message: String,
    },
    #[error(transparent)]
    Transform(#[from] TransformError),
    #[error(transparent)]
    Nifti(#[from] nifti::Error),
}

fn format_err(path: &Path, message: impl Into<String>) -> TransformFileError {
    TransformFileError::Format {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

/// One transform as stored by ITK: class name, parameters, fixed parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct ItkTransformParts {
    pub name: String,
    pub parameters: Parameters,
    pub fixed_parameters: Vec<f64>,
}

/// Transform parameters in the precision they were stored in. HDF5 files written by float
/// transforms hold float32 values, which ITK widens to double exactly; keeping them as stored
/// halves the memory of large displacement fields.
#[derive(Clone, Debug, PartialEq)]
pub enum Parameters {
    F64(Vec<f64>),
    F32(Vec<f32>),
}

impl Parameters {
    pub fn len(&self) -> usize {
        match self {
            Parameters::F64(v) => v.len(),
            Parameters::F32(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The values as double, as ITK reads them.
    pub fn to_f64(&self) -> std::borrow::Cow<'_, [f64]> {
        match self {
            Parameters::F64(v) => std::borrow::Cow::Borrowed(v),
            Parameters::F32(v) => {
                std::borrow::Cow::Owned(v.iter().map(|&x| f64::from(x)).collect())
            }
        }
    }
}

impl From<Vec<f64>> for Parameters {
    fn from(v: Vec<f64>) -> Self {
        Parameters::F64(v)
    }
}

/// Builds transforms from ITK's stored parts, nesting the components of a leading composite.
/// Displacement fields need their vectors in `parameters` (e.g. from an `.h5` file).
pub fn transform_from_parts(parts: &[ItkTransformParts]) -> Result<Transform, TransformError> {
    let first = parts
        .first()
        .ok_or_else(|| TransformError::Invalid("no transform in file".into()))?;
    if first.name.starts_with("CompositeTransform") {
        let components = parts[1..]
            .iter()
            .map(single_from_parts)
            .collect::<Result<_, _>>()?;
        return Ok(Transform::Composite(components));
    }
    single_from_parts(first)
}

fn single_from_parts(p: &ItkTransformParts) -> Result<Transform, TransformError> {
    if p.name.starts_with("DisplacementFieldTransform") {
        let data = match &p.parameters {
            Parameters::F32(v) => FieldData::F32(v.clone()),
            Parameters::F64(v) => FieldData::F64(v.clone()),
        };
        return Ok(Transform::DisplacementField(DisplacementField::from_itk(
            &p.name,
            data,
            &p.fixed_parameters,
        )?));
    }
    Ok(Transform::Linear(LinearTransform::from_itk(
        &p.name,
        &p.parameters.to_f64(),
        &p.fixed_parameters,
    )?))
}

/// The stored parts of a transform (the inverse of [`transform_from_parts`]): a composite
/// becomes a `CompositeTransform_double_3_3` entry followed by its components, nested
/// composites flattened.
pub fn transform_to_parts(t: &Transform) -> Vec<ItkTransformParts> {
    fn push(t: &Transform, out: &mut Vec<ItkTransformParts>) {
        match t {
            Transform::Linear(l) => out.push(ItkTransformParts {
                name: l.itk_name.clone(),
                parameters: Parameters::F64(l.parameters.clone()),
                fixed_parameters: l.fixed_parameters.clone(),
            }),
            Transform::DisplacementField(f) => out.push(ItkTransformParts {
                name: f.itk_name.clone(),
                parameters: match &f.data {
                    FieldData::F32(v) => Parameters::F32(v.clone()),
                    FieldData::F64(v) => Parameters::F64(v.clone()),
                },
                fixed_parameters: f.fixed_parameters(),
            }),
            Transform::Composite(ts) => ts.iter().for_each(|t| push(t, out)),
        }
    }
    let mut out = Vec::new();
    if matches!(t, Transform::Composite(_)) {
        out.push(ItkTransformParts {
            name: "CompositeTransform_double_3_3".into(),
            parameters: Parameters::F64(Vec::new()),
            fixed_parameters: Vec::new(),
        });
    }
    push(t, &mut out);
    out
}

/// Parses ITK's text transform format.
pub fn parse_itk_text(text: &str) -> Result<Vec<ItkTransformParts>, String> {
    let mut out: Vec<ItkTransformParts> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or("tags must be delimited by ':'")?;
        let (name, value) = (name.trim(), value.trim());
        match name {
            "Transform" => out.push(ItkTransformParts {
                name: value.to_owned(),
                parameters: Parameters::F64(vec![]),
                fixed_parameters: vec![],
            }),
            "Parameters" | "FixedParameters" => {
                let values = value
                    .split_whitespace()
                    .map(|v| v.parse::<f64>().map_err(|_| format!("bad number {v:?}")))
                    .collect::<Result<Vec<_>, _>>()?;
                let current = out.last_mut().ok_or("parameters before any Transform")?;
                if name == "Parameters" {
                    current.parameters = Parameters::F64(values);
                } else {
                    current.fixed_parameters = values;
                }
            }
            "ComponentTransformFile" => {
                return Err(
                    "composite transforms stored in separate files are not supported".into(),
                );
            }
            _ => {}
        }
    }
    if out.is_empty() {
        return Err("no transform in file".into());
    }
    Ok(out)
}

/// ITK's text representation of a list of linear transforms (with a leading composite header
/// when there is more than one).
pub fn write_itk_text(transforms: &[LinearTransform]) -> String {
    let mut s = String::from("#Insight Transform File V1.0\n");
    let mut index = 0;
    if transforms.len() > 1 {
        s.push_str("#Transform 0\nTransform: CompositeTransform_double_3_3\n");
        index = 1;
    }
    for t in transforms {
        let join = |v: &[f64]| {
            v.iter()
                .map(|x| format!("{x:?}"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let _ = write!(
            s,
            "#Transform {index}\nTransform: {}\nParameters: {}\nFixedParameters: {}\n",
            t.itk_name,
            join(&t.parameters),
            join(&t.fixed_parameters)
        );
        index += 1;
    }
    s
}

// ------------------------------------------------------------------------------------------------
// MATLAB v4 (vnl_matlab): header (type, rows, cols, imag, namlen), name, column-major data.

struct MatVar {
    name: String,
    values: Vec<f64>,
}

fn parse_matlab_vars(bytes: &[u8]) -> Result<Vec<MatVar>, String> {
    let mut vars = Vec::new();
    let mut pos = 0;
    while pos + 20 <= bytes.len() {
        let word = |at: usize, big: bool| {
            let b: [u8; 4] = bytes[at..at + 4].try_into().unwrap();
            if big {
                i32::from_be_bytes(b)
            } else {
                i32::from_le_bytes(b)
            }
        };
        // The type's thousands digit gives the byte order; try little-endian first.
        let big = !matches!(word(pos, false), 0 | 10 | 100 | 110)
            && matches!(word(pos, true), 1000 | 1010 | 1100 | 1110);
        let (ty, rows, cols, imag, namlen) = (
            word(pos, big),
            word(pos + 4, big),
            word(pos + 8, big),
            word(pos + 12, big),
            word(pos + 16, big),
        );
        let precision = (ty % 100) / 10; // 0 double, 1 single
        if !(0..=1).contains(&precision)
            || imag != 0
            || rows < 0
            || cols < 0
            || !(1..=1024).contains(&namlen)
        {
            return Err(format!(
                "unsupported MATLAB variable header (type {ty}, imag {imag}, namlen {namlen})"
            ));
        }
        pos += 20;
        let name_end = pos + namlen as usize;
        let name = bytes.get(pos..name_end).ok_or("truncated variable name")?;
        let name =
            String::from_utf8_lossy(name.split(|&b| b == 0).next().unwrap_or(&[])).into_owned();
        pos = name_end;
        let n = (rows as usize) * (cols as usize);
        let width = if precision == 0 { 8 } else { 4 };
        let data = bytes
            .get(pos..pos + n * width)
            .ok_or("truncated variable data")?;
        let values = data
            .chunks_exact(width)
            .map(|c| match (width, big) {
                (8, false) => f64::from_le_bytes(c.try_into().unwrap()),
                (8, true) => f64::from_be_bytes(c.try_into().unwrap()),
                (_, false) => f64::from(f32::from_le_bytes(c.try_into().unwrap())),
                (_, true) => f64::from(f32::from_be_bytes(c.try_into().unwrap())),
            })
            .collect();
        pos += n * width;
        vars.push(MatVar { name, values });
    }
    Ok(vars)
}

/// Parses ITK's MATLAB transform format: pairs of (parameters named by the transform class,
/// fixed parameters named `fixed`).
pub fn parse_itk_matlab(bytes: &[u8]) -> Result<Vec<ItkTransformParts>, String> {
    let vars = parse_matlab_vars(bytes)?;
    if vars.is_empty() || vars.len() % 2 != 0 {
        return Err(format!(
            "expected (parameters, fixed) pairs, found {} variables",
            vars.len()
        ));
    }
    Ok(vars
        .chunks(2)
        .map(|pair| ItkTransformParts {
            name: pair[0].name.clone(),
            parameters: Parameters::F64(pair[0].values.clone()),
            fixed_parameters: pair[1].values.clone(),
        })
        .collect())
}

/// ITK's MATLAB representation of a linear transform (double precision, little-endian).
pub fn write_itk_matlab(t: &LinearTransform) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, values) in [
        (t.itk_name.as_str(), &t.parameters),
        ("fixed", &t.fixed_parameters),
    ] {
        let name = format!("{name}\0");
        for v in [0i32, values.len() as i32, 1, 0, name.len() as i32] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(name.as_bytes());
        for v in values.iter() {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

// ------------------------------------------------------------------------------------------------
// Displacement fields as NIfTI vector images

/// Reads a displacement field stored as a NIfTI vector image (`x × y × z × 1 × 3`), as ITK's
/// reader does: grid from the header (see [`nifti::itk::itk_geometry`]), vectors in LPS
/// (ANTs writes them so, with intent `NIFTI_INTENT_VECTOR`; fields with intent
/// `NIFTI_INTENT_DISPVECT` are converted from RAS).
pub fn read_displacement_field(path: &Path) -> Result<DisplacementField, TransformFileError> {
    let image = nifti::itk::read_itk_image(path, 1)?;
    let g = &image.geometry;
    if g.ndim != 3 || image.voxels.len() != 3 * g.size.iter().product::<usize>() {
        return Err(format_err(
            path,
            format!(
                "a 3D displacement field has shape (x, y, z, 1, 3); ITK reads {:?} with {} values",
                g.size,
                image.voxels.len()
            ),
        ));
    }
    let n = g.size.iter().product::<usize>();
    // NIfTI stores the components as the slowest axis; ITK interleaves them per voxel.
    fn interleave<T: Copy + Default>(planar: &[T], n: usize) -> Vec<T> {
        let mut out = vec![T::default(); 3 * n];
        for (c, plane) in planar.chunks_exact(n).enumerate() {
            for (v, &value) in plane.iter().enumerate() {
                out[3 * v + c] = value;
            }
        }
        out
    }
    let data = match &image.voxels {
        nifti::itk::ItkVoxels::F32(v) => FieldData::F32(interleave(v, n)),
        nifti::itk::ItkVoxels::F64(v) => FieldData::F64(interleave(v, n)),
    };
    let direction: Mat3 = std::array::from_fn(|i| std::array::from_fn(|j| g.direction[i][j]));
    let grid = Grid3::new(
        [g.size[0], g.size[1], g.size[2]],
        [g.spacing[0], g.spacing[1], g.spacing[2]],
        [g.origin[0], g.origin[1], g.origin[2]],
        direction,
    )
    .ok_or_else(|| format_err(path, "singular grid"))?;
    Ok(DisplacementField::new(
        grid,
        data,
        "DisplacementFieldTransform_double_3_3",
    )?)
}

/// Reads an ITK transform file by its extension, returning its first transform (as ANTs
/// does): `.txt`/`.tfm` text, `.mat` MATLAB, `.nii`/`.nii.gz` displacement field.
pub fn read_itk_transform(path: &Path) -> Result<Transform, TransformFileError> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with(".nii") || name.ends_with(".nii.gz") {
        return Ok(Transform::DisplacementField(read_displacement_field(path)?));
    }
    let io = |source| TransformFileError::Io {
        path: path.to_path_buf(),
        source,
    };
    let parts = if name.ends_with(".mat") {
        parse_itk_matlab(&std::fs::read(path).map_err(io)?).map_err(|m| format_err(path, m))?
    } else if name.ends_with(".txt") || name.ends_with(".tfm") {
        parse_itk_text(&std::fs::read_to_string(path).map_err(io)?)
            .map_err(|m| format_err(path, m))?
    } else if name.ends_with(".h5") || name.ends_with(".hdf5") {
        return Err(format_err(
            path,
            "HDF5 transforms are read by the Python layer (larmorx.transforms.read)",
        ));
    } else {
        return Err(format_err(
            path,
            "unknown transform file type (expected .txt, .tfm, .mat, .nii or .nii.gz)",
        ));
    };
    Ok(transform_from_parts(&parts)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AFFINE: &str = "#Insight Transform File V1.0\n#Transform 0\nTransform: AffineTransform_float_3_3\nParameters: 0.999736 0.00543478 0.0223235 0.00925598 0.794015 -0.607828 -0.0210286 0.607874 0.793755 -5.53715 -45.7572 -48.9496\nFixedParameters: 0 0 0\n";

    #[test]
    fn text_round_trip() {
        let t = transform_from_parts(&parse_itk_text(AFFINE).unwrap()).unwrap();
        let Transform::Linear(lin) = &t else { panic!() };
        assert_eq!(lin.parameters[11], -48.9496);
        let back = transform_from_parts(
            &parse_itk_text(&write_itk_text(std::slice::from_ref(lin))).unwrap(),
        )
        .unwrap();
        assert_eq!(back, t);
        let composite = parse_itk_text(&write_itk_text(&[lin.clone(), lin.clone()])).unwrap();
        assert!(
            matches!(transform_from_parts(&composite).unwrap(), Transform::Composite(ts) if ts.len() == 2)
        );
        assert!(parse_itk_text("Transform AffineTransform").is_err());
    }

    #[test]
    fn matlab_round_trip_and_single_precision() {
        let Transform::Linear(lin) =
            transform_from_parts(&parse_itk_text(AFFINE).unwrap()).unwrap()
        else {
            panic!()
        };
        let bytes = write_itk_matlab(&lin);
        let parts = parse_itk_matlab(&bytes).unwrap();
        assert_eq!(parts[0].name, "AffineTransform_float_3_3");
        assert_eq!(parts[0].parameters, Parameters::F64(lin.parameters.clone()));
        // A float32, big-endian variable (as some writers produce).
        let mut be = Vec::new();
        for v in [1010i32, 2, 1, 0, 2] {
            be.extend_from_slice(&v.to_be_bytes());
        }
        be.extend_from_slice(b"x\0");
        be.extend_from_slice(&1.5f32.to_be_bytes());
        be.extend_from_slice(&(-2.0f32).to_be_bytes());
        let vars = parse_matlab_vars(&be).unwrap();
        assert_eq!(
            (vars[0].name.as_str(), vars[0].values.as_slice()),
            ("x", &[1.5, -2.0][..])
        );
    }
}
