// SPDX-License-Identifier: Apache-2.0
//! ITK transform files: text (`.txt`, `.tfm`), MATLAB v4 (`.mat`), HDF5 (`.h5`, through
//! [`crate::itk_hdf5`]), and displacement fields stored as NIfTI vector images (`.nii`,
//! `.nii.gz`).
//!
//! Follows ITK v5.4.5's `TxtTransformIO`, `MatlabTransformIO` and `HDF5TransformIO`: a file
//! holds a list of transforms; when the first is a `CompositeTransform`, the following ones
//! are its components. As ANTs's `itk::ants::ReadTransform` does, [`read_itk_transform`]
//! returns that composite, the one transform of a list of one, or a composite of all the
//! transforms of a longer list. As ITK's reader does, a file in HDF5 with a `/TransformGroup`
//! is read as HDF5 whatever its name ends with (ITK asks its HDF5 transform reader first).

use std::fmt::Write as _;
use std::path::Path;

use larmorx_core::defects::{Notices, UpstreamDefects};
use larmorx_core::linalg::Mat3;
use larmorx_core::{Grid3, Precision};
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
    /// The file does not exist (ANTs: `Transform file does not exist: <path>`).
    #[error("Transform file does not exist: {}", .0.display())]
    Missing(std::path::PathBuf),
    /// ITK cannot read the file: why, and what ANTs prints.
    #[error("{0}")]
    Unreadable(crate::itk_hdf5::ReadFailure),
    /// ITK cannot write the transforms: why, and what ANTs prints.
    #[error("{}: {failure}", path.display())]
    Unwritable {
        path: std::path::PathBuf,
        failure: crate::itk_hdf5::WriteFailure,
    },
}

impl TransformFileError {
    /// What ANTs's `itk::ants::ReadTransform` prints on standard error for this error, where
    /// larmorx reproduces it (missing files, HDF5 files).
    pub fn ants_report(&self) -> Option<String> {
        match self {
            TransformFileError::Missing(p) => {
                Some(format!("Transform file does not exist: {}\n", p.display()))
            }
            TransformFileError::Unreadable(f) => Some(f.ants_report.clone()),
            TransformFileError::Unwritable { path, failure } if !failure.ants_report.is_empty() => {
                Some(
                    failure
                        .ants_report
                        .replace("<file>", &path.display().to_string()),
                )
            }
            _ => None,
        }
    }
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

/// Builds the transform ANTs's `itk::ants::ReadTransform` makes of a file's stored parts: a
/// leading composite takes the following parts as its components
/// (`TransformFileReaderTemplate::Update`); otherwise one part is itself and several make a
/// composite of all of them. A composite further down the list is an empty composite (ITK
/// creates it without components): the identity. Displacement fields need their vectors in
/// `parameters` (e.g. from an `.h5` file).
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
    if parts.len() > 1 {
        return Ok(Transform::Composite(
            parts
                .iter()
                .map(single_from_parts)
                .collect::<Result<_, _>>()?,
        ));
    }
    single_from_parts(first)
}

fn single_from_parts(p: &ItkTransformParts) -> Result<Transform, TransformError> {
    if p.name.starts_with("CompositeTransform") {
        return Ok(Transform::Composite(Vec::new()));
    }
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

/// Parses ITK's text transform format, as a double-precision reader does.
pub fn parse_itk_text(text: &str) -> Result<Vec<ItkTransformParts>, String> {
    parse_itk_text_with(text, Precision::Double)
}

/// Parses ITK's text transform format as `TxtTransformIOTemplate<T>` reads it for the
/// computation type `precision`: every number of a `Parameters` **and** a `FixedParameters`
/// line is read by `>> T` (`OptimizerParameters<T>`, `itkTxtTransformIO.cxx`), so in float it
/// is rounded once from its decimal text to `f32` (as `strtof`, not `strtod` then a cast); the
/// fixed parameters are then widened to double. The values are returned as `f64` either way.
pub fn parse_itk_text_with(
    text: &str,
    precision: Precision,
) -> Result<Vec<ItkTransformParts>, String> {
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
                let number = |v: &str| match precision {
                    Precision::Double => v.parse::<f64>(),
                    Precision::Float => v.parse::<f32>().map(f64::from),
                };
                let values = value
                    .split_whitespace()
                    .map(|v| number(v).map_err(|_| format!("bad number {v:?}")))
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

/// The extensions ITK's HDF5 transform writer takes (`HDF5TransformIOTemplate::CanWriteFile`:
/// the file name's last extension, compared case for case).
pub const HDF5_EXTENSIONS: &[&str] = &[
    ".hdf", ".h4", ".hdf4", ".h5", ".hdf5", ".he4", ".he5", ".hd5",
];

/// The file name's last extension, dot included (`SystemTools::GetFilenameLastExtension`).
fn last_extension(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.rfind('.')
        .map(|i| name[i..].to_string())
        .unwrap_or_default()
}

/// Whether ITK writes `path` as an HDF5 transform file ([`HDF5_EXTENSIONS`]).
pub fn is_hdf5_name(path: &Path) -> bool {
    HDF5_EXTENSIONS.contains(&last_extension(path).as_str())
}

/// Writes transforms in the ITK format of the file name's extension: HDF5
/// ([`HDF5_EXTENSIONS`]; any transforms, written by [`crate::itk_hdf5::write`] with
/// `options`), ITK text (`.txt`, `.tfm`; linear transforms) or MATLAB (`.mat`; one linear
/// transform). A list of more than one transform is written as ITK writes a composite's: the
/// `CompositeTransform` entry first (text files write it themselves).
///
/// When ITK's HDF5 writer fails with `notices` in `reproduce` mode and leaves a file behind
/// (ITK-15), that file is written before the error is returned.
pub fn write_itk_transform_parts(
    path: &Path,
    parts: &[ItkTransformParts],
    options: &crate::itk_hdf5::WriteOptions,
    notices: &mut Notices,
) -> Result<(), TransformFileError> {
    let io = |source| TransformFileError::Io {
        path: path.to_path_buf(),
        source,
    };
    let bytes = if is_hdf5_name(path) {
        match crate::itk_hdf5::write(parts, options, notices) {
            Ok(bytes) => bytes,
            Err(mut failure) => {
                if let Some(partial) = failure.partial.take() {
                    std::fs::write(path, partial).map_err(io)?;
                }
                return Err(TransformFileError::Unwritable {
                    path: path.to_path_buf(),
                    failure,
                });
            }
        }
    } else {
        let lower = last_extension(path).to_ascii_lowercase();
        let components = match parts.first() {
            Some(first) if first.name.starts_with("CompositeTransform") => &parts[1..],
            _ => parts,
        };
        let linear = components
            .iter()
            .map(|p| {
                LinearTransform::from_itk(&p.name, &p.parameters.to_f64(), &p.fixed_parameters)
            })
            .collect::<Result<Vec<_>, _>>()?;
        match lower.as_str() {
            ".mat" => match linear.as_slice() {
                [one] => write_itk_matlab(one),
                _ => {
                    return Err(format_err(
                        path,
                        "a .mat file holds exactly one linear transform",
                    ));
                }
            },
            ".txt" | ".tfm" => write_itk_text(&linear).into_bytes(),
            _ => {
                return Err(format_err(
                    path,
                    "unknown transform file type (expected .h5 or another HDF5 extension, .txt, \
                     .tfm or .mat)",
                ));
            }
        }
    };
    std::fs::write(path, bytes).map_err(io)
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

/// Reads an ITK transform file as ANTs does ([`transform_from_parts`] of what the file
/// stores), in double precision and with upstream defects corrected (notices are dropped; see
/// [`read_itk_transform_parts`]).
pub fn read_itk_transform(path: &Path) -> Result<Transform, TransformFileError> {
    read_itk_transform_with(path, Precision::Double)
}

/// [`read_itk_transform`] as ITK's reader for the computation type `precision` reads the
/// file. Text files' numbers are read in that precision ([`parse_itk_text_with`]); MATLAB,
/// HDF5 and NIfTI files are read as stored (ITK casts their values to the transform's type
/// afterwards, as
/// [`LinearTransform::with_precision`](larmorx_transform::LinearTransform::with_precision)
/// and the displacement field lookup do).
pub fn read_itk_transform_with(
    path: &Path,
    precision: Precision,
) -> Result<Transform, TransformFileError> {
    let mut notices = Notices::new(UpstreamDefects::Fix);
    let parts = read_itk_transform_parts(path, precision, &mut notices)?;
    Ok(transform_from_parts(&parts)?)
}

/// The parts a transform file stores, read as ANTs's `itk::ants::ReadTransform` reads them
/// for the computation type `precision`, with D13's mode and notices in `notices` (HDF5 files:
/// [`crate::itk_hdf5`]). Which reader applies:
/// - a missing file: [`TransformFileError::Missing`];
/// - `.nii`, `.nii.gz`: a displacement field (ANTs tries files without a transform extension
///   as images first);
/// - an HDF5 file with a `/TransformGroup`, whatever its name ends with;
/// - `.mat`: MATLAB v4; `.txt`, `.tfm`: ITK text;
/// - `.h5`, `.hdf5`, `.hdf4` files that are not HDF5 transform files: ITK's "Could not create
///   Transform IO object" error.
pub fn read_itk_transform_parts(
    path: &Path,
    precision: Precision,
    notices: &mut Notices,
) -> Result<Vec<ItkTransformParts>, TransformFileError> {
    if !path.exists() {
        return Err(TransformFileError::Missing(path.to_path_buf()));
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with(".nii") || name.ends_with(".nii.gz") {
        return Ok(transform_to_parts(&Transform::DisplacementField(
            read_displacement_field(path)?,
        )));
    }
    let io = |source| TransformFileError::Io {
        path: path.to_path_buf(),
        source,
    };
    // ITK's transform IO factory asks the HDF5 reader first: an HDF5 file with a transform
    // group is read as HDF5 whatever its extension.
    match crate::itk_hdf5::can_read(path) {
        Ok(_) => {
            return crate::itk_hdf5::read(path, precision, notices)
                .map_err(TransformFileError::Unreadable);
        }
        Err((why, h5)) => {
            if name.ends_with(".h5") || name.ends_with(".hdf5") || name.ends_with(".hdf4") {
                return Err(TransformFileError::Unreadable(
                    crate::itk_hdf5::no_transform_io(
                        &path.display().to_string(),
                        h5.as_ref(),
                        &why,
                    ),
                ));
            }
        }
    }
    if name.ends_with(".mat") {
        parse_itk_matlab(&std::fs::read(path).map_err(io)?).map_err(|m| format_err(path, m))
    } else if name.ends_with(".txt") || name.ends_with(".tfm") {
        parse_itk_text_with(&std::fs::read_to_string(path).map_err(io)?, precision)
            .map_err(|m| format_err(path, m))
    } else {
        Err(format_err(
            path,
            "unknown transform file type (expected .txt, .tfm, .mat, .h5, .nii or .nii.gz)",
        ))
    }
}

/// [`read_itk_transform_parts`] made into a transform ([`transform_from_parts`]).
pub fn read_itk_transform_notices(
    path: &Path,
    precision: Precision,
    notices: &mut Notices,
) -> Result<Transform, TransformFileError> {
    let parts = read_itk_transform_parts(path, precision, notices)?;
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
    #[test]
    fn float_text_numbers_are_rounded_once() {
        // Just below the midpoint 26843547·2⁻²⁸ between the floats 0x3dcccccd and 0x3dccccce:
        // strtod rounds it onto the midpoint, which a cast to float then rounds to even (up);
        // read as float (strtof, ITK's `>> float`), it rounds down.
        let text = "#Insight Transform File V1.0\nTransform: TranslationTransform_double_3_3\nParameters: 0.1000000052154064178466796874999999999999 0 0\nFixedParameters: 0.1000000052154064178466796874999999999999 0 0\n";
        let double = parse_itk_text(text).unwrap();
        let single = parse_itk_text_with(text, Precision::Float).unwrap();
        let d = double[0].parameters.to_f64()[0];
        let f = single[0].parameters.to_f64()[0];
        assert_eq!((d as f32).to_bits(), 0x3dcc_ccce);
        assert_eq!(f, f64::from(f32::from_bits(0x3dcc_cccd)));
        assert_eq!(single[0].fixed_parameters[0], f);
    }
}
