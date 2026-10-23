// SPDX-License-Identifier: Apache-2.0
//! `antsApplyTransforms`' transform outputs: `-o CompositeTransform[file]`,
//! `-o Linear[file,<invert>]` and `-o DisplacementField[file]` (or `-o [file,1]`).
//!
//! Ported from ANTs v2.6.5 (`Examples/antsApplyTransforms.cxx`, the output section;
//! `Examples/itkantsRegistrationHelper.hxx`, `CollapseLinearTransforms`) and ITK v5.4.5 (the
//! transform classes and `TransformToDisplacementFieldFilter`, see
//! [`larmorx_transform::itk_object`]). The transforms are those of a [`TransformChain`] as
//! ANTs's `CompositeTransform<R>` holds them, `R` the computation type (`--float`): each keeps
//! the class ITK reads it as, an inverted one the class `GetInverseTransform` makes; a chain
//! without parameters gets an identity `MatrixOffsetTransformBase` first
//! ([`with_identity_if_empty`]).
//!
//! - **Composite:** a `CompositeTransform_<R>_3_3` followed by every transform of the chain in
//!   queue order.
//! - **Linear:** the chain collapsed to one `AffineTransform<R>` (ANTs's
//!   `CollapseLinearTransforms`: the centre of the last matrix-offset transform, then
//!   `Compose(nth, true)` with each transform in queue order); inverted, the `AffineTransform`
//!   ANTs builds from it: `SetMatrix(M⁻¹)`, `SetOffset(−(M⁻¹·offset))`, centre 0.
//! - **Displacement field:** `T(p) − p` on the reference grid (ITK's
//!   `TransformToDisplacementFieldFilter`), as a `DisplacementFieldTransform_<R>_3_3`.

use larmorx_core::defects::{Notice, UpstreamDefects};
use larmorx_core::{Grid3, ItkReal, Precision};
use larmorx_io::itk_transform::{ItkTransformParts, Parameters};
use larmorx_transform::itk_object::class_name;
use larmorx_transform::{
    DisplacementField, FieldData, ItkObject, LinearTransform, MatrixOffsetState, ObjectParameters,
    Transform, TransformChain, TransformError,
};

/// What `-o` asks for besides the warped image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformOutput {
    /// `-o CompositeTransform[file]`.
    Composite,
    /// `-o Linear[file,<invert>]`.
    Linear { invert: bool },
    /// `-o DisplacementField[file]` or `-o [file,1]`.
    DisplacementField,
}

/// A transform output could not be made.
#[derive(Debug, thiserror::Error)]
pub enum TransformOutputError {
    /// ANTs fails with this message (exit 1).
    #[error("{0}")]
    Ants(String),
    /// ANTs crashes or aborts here; larmorx refuses (D13 class a, in both modes).
    #[error("larmorx: {0}")]
    Refused(String),
    #[error(transparent)]
    Threads(#[from] larmorx_core::parallel::ThreadPoolError),
}

/// The number of parameters of ANTs's composite (`GetNumberOfParameters`, every transform
/// optimizable).
pub fn chain_parameters(chain: &TransformChain) -> usize {
    chain
        .transforms()
        .iter()
        .map(|t| match t {
            Transform::Linear(l) => l.parameters.len(),
            Transform::DisplacementField(f) => 3 * f.grid.len(),
            Transform::BSpline(b) => b.number_of_parameters(),
            Transform::Composite(_) => 0,
        })
        .sum()
}

/// The chain as `antsApplyTransforms` uses it: with an identity `MatrixOffsetTransformBase`
/// added at the back of the queue when the composite has no parameters ("No transforms found,
/// using identify transform"). The second value says whether it was added.
pub fn with_identity_if_empty(
    chain: TransformChain,
) -> Result<(TransformChain, bool), TransformError> {
    if chain_parameters(&chain) > 0 {
        return Ok((chain, false));
    }
    Ok((
        chain.with_appended(Transform::Linear(LinearTransform::identity_matrix_offset()))?,
        true,
    ))
}

/// Values exact in `precision`, stored as ITK's writer of that precision stores them.
fn values(values: Vec<f64>, precision: Precision) -> Parameters {
    match precision {
        Precision::Double => Parameters::F64(values),
        Precision::Float => Parameters::F32(values.into_iter().map(|v| v as f32).collect()),
    }
}

/// A displacement field's vectors in `precision` (ITK's reader of that precision converts
/// them).
fn field_values(f: &DisplacementField, precision: Precision) -> Parameters {
    match (&f.data, precision) {
        (FieldData::F64(v), Precision::Double) => Parameters::F64(v.clone()),
        (FieldData::F32(v), Precision::Float) => Parameters::F32(v.clone()),
        (FieldData::F32(v), Precision::Double) => {
            Parameters::F64(v.iter().map(|&x| f64::from(x)).collect())
        }
        (FieldData::F64(v), Precision::Float) => {
            Parameters::F32(v.iter().map(|&x| x as f32).collect())
        }
    }
}

/// A chain object as stored parts.
fn object_parts(o: ItkObject<'_>, precision: Precision) -> ItkTransformParts {
    ItkTransformParts {
        name: o.name,
        parameters: match o.parameters {
            ObjectParameters::Values(v) => values(v, precision),
            ObjectParameters::Field(f) => field_values(f, precision),
        },
        fixed_parameters: o.fixed_parameters,
    }
}

/// `-o Linear[...]` in `R`: `GetParameters()` and `GetFixedParameters()` of the collapsed
/// `AffineTransform` or of its inverse.
fn collapsed<R: ItkReal>(
    chain: &TransformChain,
    invert: bool,
    mode: UpstreamDefects,
) -> Result<(Vec<f64>, Vec<f64>), TransformOutputError> {
    let total = chain.collapse_linear::<R>(mode).map_err(|e| match e {
        TransformError::Invalid(m) if m.starts_with("The transform") => {
            TransformOutputError::Ants(m)
        }
        TransformError::Invalid(m) => TransformOutputError::Refused(m),
        e => TransformOutputError::Ants(e.to_string()),
    })?;
    let total = if invert {
        let inv = R::itk_inverse3(&total.matrix).ok_or_else(|| {
            TransformOutputError::Refused(
                "the collapsed affine transform is singular, so it has no inverse; ANTs crashes"
                    .into(),
            )
        })?;
        let mut t = MatrixOffsetState::<R>::identity();
        t.set_matrix(inv);
        let mut offset = [R::ZERO; 3];
        for (row, o) in inv.iter().zip(offset.iter_mut()) {
            let mut sum = R::ZERO;
            for (&m, &v) in row.iter().zip(&total.offset) {
                sum = sum + m * v;
            }
            *o = -sum;
        }
        t.set_offset(offset);
        t
    } else {
        total
    };
    Ok((
        total.parameters(),
        total.center.iter().map(|c| c.to_f64()).collect(),
    ))
}

/// The transforms `kind` makes of `chain` (already passed through
/// [`with_identity_if_empty`]), as ITK's transform writer of `precision` holds them: for a
/// composite, the `CompositeTransform` entry followed by its components; for the others, one
/// transform. A displacement field is computed on `reference` (required) with `n_threads`
/// threads (the result does not depend on it). `mode` corrects or reproduces ITK's defects
/// (ITK-01, ITK-10), whose notices are returned.
pub fn transform_output(
    chain: &TransformChain,
    kind: TransformOutput,
    reference: Option<&Grid3>,
    precision: Precision,
    mode: UpstreamDefects,
    n_threads: usize,
) -> Result<(Vec<ItkTransformParts>, Vec<Notice>), TransformOutputError> {
    match kind {
        TransformOutput::Composite => {
            let objects = chain
                .itk_objects(precision, mode)
                .map_err(|e| TransformOutputError::Ants(e.to_string()))?;
            let mut parts = vec![ItkTransformParts {
                name: class_name("CompositeTransform", precision),
                parameters: Parameters::F64(Vec::new()),
                fixed_parameters: Vec::new(),
            }];
            parts.extend(objects.into_iter().map(|o| object_parts(o, precision)));
            Ok((parts, Vec::new()))
        }
        TransformOutput::Linear { invert } => {
            let (parameters, fixed) = match precision {
                Precision::Double => collapsed::<f64>(chain, invert, mode)?,
                Precision::Float => collapsed::<f32>(chain, invert, mode)?,
            };
            Ok((
                vec![ItkTransformParts {
                    name: class_name("AffineTransform", precision),
                    parameters: values(parameters, precision),
                    fixed_parameters: fixed,
                }],
                Vec::new(),
            ))
        }
        TransformOutput::DisplacementField => {
            let grid = reference.ok_or_else(|| {
                TransformOutputError::Ants("A reference image is required.".into())
            })?;
            let (vectors, notices) =
                larmorx_core::parallel::with_threads(n_threads, || match precision {
                    Precision::Double => chain
                        .displacement_field::<f64>(grid, mode)
                        .map(|(v, n)| (Parameters::F64(v), n)),
                    Precision::Float => chain
                        .displacement_field::<f32>(grid, mode)
                        .map(|(v, n)| (Parameters::F32(v), n)),
                })?
                .map_err(|e| TransformOutputError::Ants(e.to_string()))?;
            let fixed =
                DisplacementField::new(grid.clone(), FieldData::F32(vec![0.0; 3 * grid.len()]), "")
                    .map_err(|e| TransformOutputError::Ants(e.to_string()))?
                    .fixed_parameters();
            Ok((
                vec![ItkTransformParts {
                    name: class_name("DisplacementFieldTransform", precision),
                    parameters: vectors,
                    fixed_parameters: fixed,
                }],
                notices,
            ))
        }
    }
}
