// SPDX-License-Identifier: Apache-2.0
//! The transforms of a chain as ITK objects: the class, parameters and fixed parameters that
//! ANTs's `CompositeTransform<R>` holds for each of them, ITK's `CollapseLinearTransforms`
//! (ANTs) and `TransformToDisplacementFieldFilter`.
//!
//! This is what `antsApplyTransforms -o CompositeTransform[...]`, `-o Linear[...]` and
//! `-o DisplacementField[...]` write. A transform read from a file keeps its class
//! (`TransformFileReaderTemplate<R>` builds it by `SetFixedParameters` and `SetParameters` in
//! `R`). An inverted one (`-t [file,1]`, `GetInverseTransform`) is a new object:
//!
//! - `AffineTransform` inverts to an `AffineTransform`, `TranslationTransform` to a
//!   `TranslationTransform` (offset negated) and `IdentityTransform` to itself;
//! - every other matrix-offset class (`Euler3DTransform`, `VersorRigid3DTransform`,
//!   `Similarity3DTransform`, `Rigid3DTransform`, `MatrixOffsetTransformBase`) inherits
//!   `MatrixOffsetTransformBase::GetInverseTransform`, which instantiates the base class: the
//!   inverse is a `MatrixOffsetTransformBase` (`InvertTransform(*this)` with the base's static
//!   type) with the original's fixed parameters (an `Euler3DTransform`'s four: the centre and
//!   its ZYX flag), the matrix `M⁻¹` (vnl's SVD inverse, in `R`), the offset `−(M⁻¹·offset)`
//!   and the translation `ComputeTranslation` derives from them.
//!
//! Ported from ITK v5.4.5 (`Modules/Core/Transform`: `itkMatrixOffsetTransformBase.hxx`,
//! `itkEuler3DTransform.hxx`, `itkVersorRigid3DTransform.hxx`, `itkSimilarity3DTransform.hxx`,
//! `itkTranslationTransform.hxx`, `itkCompositeTransform.hxx`; `Modules/Filtering/
//! DisplacementField/include/itkTransformToDisplacementFieldFilter.hxx`) and ANTs v2.6.5
//! (`Examples/itkantsRegistrationHelper.hxx`, `CollapseLinearTransforms`;
//! `Examples/antsApplyTransforms.cxx`).

use larmorx_core::defects::{Notice, Notices, UpstreamDefects};
use larmorx_core::{Grid3, IndexFloor, ItkReal, Precision, same_value};
use rayon::prelude::*;

use crate::linear::versor;
use crate::{BSplineTransform, DisplacementField, LinearKind, LinearTransform, Transform};
use crate::{TransformChain, TransformError};

/// The class name a stored transform has in precision `precision`: `<base>_<double|float>_3_3`.
pub fn class_name(base: &str, precision: Precision) -> String {
    let real = match precision {
        Precision::Double => "double",
        Precision::Float => "float",
    };
    format!("{base}_{real}_3_3")
}

/// The base of an ITK class name (`Euler3DTransform` of `Euler3DTransform_double_3_3`).
pub fn class_base(name: &str) -> &str {
    name.split('_').next().unwrap_or(name)
}

/// What an ITK object holds as parameters.
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectParameters<'a> {
    /// `GetParameters()`, each value exact in the object's precision.
    Values(Vec<f64>),
    /// A displacement field's vectors (the field's data, converted to the object's precision
    /// when it is written).
    Field(&'a DisplacementField),
}

/// One transform as ITK holds it: its class name, `GetParameters()` and
/// `GetFixedParameters()`.
#[derive(Clone, Debug, PartialEq)]
pub struct ItkObject<'a> {
    pub name: String,
    pub parameters: ObjectParameters<'a>,
    pub fixed_parameters: Vec<f64>,
}

/// The state of a matrix-offset transform in `R`: what ITK keeps in
/// `MatrixOffsetTransformBase<R>` (`m_Matrix`, `m_Offset`, `m_Center`, `m_Translation`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatrixOffsetState<R> {
    pub matrix: [[R; 3]; 3],
    pub offset: [R; 3],
    pub center: [R; 3],
    pub translation: [R; 3],
}

/// `T sum{}; sum += m(r, c) * v[c]` (`itk::Matrix<R> * Vector<R>`).
fn mat_vec<R: ItkReal>(m: &[[R; 3]; 3], v: &[R; 3]) -> [R; 3] {
    std::array::from_fn(|r| {
        let mut sum = R::ZERO;
        for c in 0..3 {
            sum = sum + m[r][c] * v[c];
        }
        sum
    })
}

/// `vnl_matrix_fixed` product: each entry starts from the first product.
fn mat_mat<R: ItkReal>(a: &[[R; 3]; 3], b: &[[R; 3]; 3]) -> [[R; 3]; 3] {
    std::array::from_fn(|i| {
        std::array::from_fn(|j| {
            let mut acc = a[i][0] * b[0][j];
            for k in 1..3 {
                acc = acc + a[i][k] * b[k][j];
            }
            acc
        })
    })
}

impl<R: ItkReal> MatrixOffsetState<R> {
    /// A new `AffineTransform<R>`: identity matrix, everything else zero.
    pub fn identity() -> Self {
        let (z, o) = (R::ZERO, R::ONE);
        MatrixOffsetState {
            matrix: [[o, z, z], [z, o, z], [z, z, o]],
            offset: [z; 3],
            center: [z; 3],
            translation: [z; 3],
        }
    }

    /// `ComputeOffset`: `offset = translation + center`, then `−= M[i][j]·center[j]` term by
    /// term.
    fn compute_offset(&mut self) {
        for i in 0..3 {
            let mut o = self.translation[i] + self.center[i];
            for j in 0..3 {
                o = o - self.matrix[i][j] * self.center[j];
            }
            self.offset[i] = o;
        }
    }

    /// `ComputeTranslation`: `translation = offset − center`, then `+= M[i][j]·center[j]`.
    fn compute_translation(&mut self) {
        for i in 0..3 {
            let mut t = self.offset[i] - self.center[i];
            for j in 0..3 {
                t = t + self.matrix[i][j] * self.center[j];
            }
            self.translation[i] = t;
        }
    }

    /// `SetCenter`.
    pub fn set_center(&mut self, center: [R; 3]) {
        self.center = center;
        self.compute_offset();
    }

    /// `SetMatrix` (an `AffineTransform`'s `ComputeMatrixParameters` does nothing).
    pub fn set_matrix(&mut self, matrix: [[R; 3]; 3]) {
        self.matrix = matrix;
        self.compute_offset();
    }

    /// `SetTranslation`.
    pub fn set_translation(&mut self, translation: [R; 3]) {
        self.translation = translation;
        self.compute_offset();
    }

    /// `SetOffset`.
    pub fn set_offset(&mut self, offset: [R; 3]) {
        self.offset = offset;
        self.compute_translation();
    }

    /// `Compose(other, pre = true)`: `offset = M·other.offset + offset`, `M = M·other.M`,
    /// then `ComputeTranslation`.
    pub fn compose_pre(&mut self, other: &Self) {
        let mv = mat_vec(&self.matrix, &other.offset);
        self.offset = std::array::from_fn(|i| mv[i] + self.offset[i]);
        self.matrix = mat_mat(&self.matrix, &other.matrix);
        self.compute_translation();
    }

    /// `GetInverse` into a new object of the base class: fixed parameters (the centre)
    /// copied, `M⁻¹` (vnl's SVD inverse in `R`), `offset = −(M⁻¹·offset)`,
    /// `ComputeTranslation`. `None` where ITK finds the matrix singular.
    pub fn inverse(&self) -> Option<Self> {
        let inv = R::itk_inverse3(&self.matrix)?;
        let mv = mat_vec(&inv, &self.offset);
        let mut out = MatrixOffsetState {
            matrix: inv,
            offset: mv.map(|v| -v),
            center: self.center,
            translation: [R::ZERO; 3],
        };
        out.compute_translation();
        Some(out)
    }

    /// `MatrixOffsetTransformBase::GetParameters`: the matrix row by row, then the
    /// translation.
    pub fn parameters(&self) -> Vec<f64> {
        let mut p: Vec<f64> = self.matrix.iter().flatten().map(|v| v.to_f64()).collect();
        p.extend(self.translation.iter().map(|v| v.to_f64()));
        p
    }
}

/// How a chain entry stands in ANTs's composite: the object's state, its class and its
/// stored values.
#[derive(Clone, Debug)]
pub struct LinearObject<R> {
    /// The class base name (`AffineTransform`, `MatrixOffsetTransformBase`, ...).
    pub class: &'static str,
    /// `m_Matrix` and friends; `None` for an `IdentityTransform`.
    pub state: Option<MatrixOffsetState<R>>,
    /// A `TranslationTransform`: `m_Offset` only.
    pub translation_only: bool,
    pub parameters: Vec<f64>,
    pub fixed_parameters: Vec<f64>,
}

/// The class base name ITK reads a linear transform as.
fn read_class(l: &LinearTransform) -> Result<&'static str, TransformError> {
    Ok(match class_base(&l.itk_name) {
        "AffineTransform" => "AffineTransform",
        "MatrixOffsetTransformBase" => "MatrixOffsetTransformBase",
        "Rigid3DTransform" => "Rigid3DTransform",
        "Euler3DTransform" => "Euler3DTransform",
        "VersorRigid3DTransform" => "VersorRigid3DTransform",
        "Similarity3DTransform" => "Similarity3DTransform",
        "QuaternionRigidTransform" => "QuaternionRigidTransform",
        "ScaleVersor3DTransform" => "ScaleVersor3DTransform",
        "ScaleSkewVersor3DTransform" => "ScaleSkewVersor3DTransform",
        "ScaleTransform" => "ScaleTransform",
        "TranslationTransform" => "TranslationTransform",
        "IdentityTransform" => "IdentityTransform",
        other => return Err(TransformError::Unsupported(other.to_owned())),
    })
}

/// `GetFixedParameters()` of a matrix-offset object read from `fixed`: the stored vector
/// (`SetFixedParameters` keeps it) with the centre, as `R`, in its first three values; an
/// `Euler3DTransform`'s is always four values, the fourth its ZYX flag.
fn matrix_offset_fixed<R: ItkReal>(class: &str, fixed: &[f64], center: &[R; 3]) -> Vec<f64> {
    if class == "Euler3DTransform" {
        let zyx = fixed.len() == 4 && fixed[3] != 0.0;
        return vec![
            center[0].to_f64(),
            center[1].to_f64(),
            center[2].to_f64(),
            if zyx { 1.0 } else { 0.0 },
        ];
    }
    let mut out = fixed.to_vec();
    if out.len() < 3 {
        out.resize(3, 0.0);
    }
    for i in 0..3 {
        out[i] = center[i].to_f64();
    }
    out
}

impl<R: ItkReal> LinearObject<R> {
    /// The object `TransformFileReaderTemplate<R>` makes of `l` (`SetFixedParameters`, then
    /// `SetParameters`), with ITK-10 corrected or reproduced as `mode` says.
    pub fn read(l: &LinearTransform, mode: UpstreamDefects) -> Result<Self, TransformError> {
        let class = read_class(l)?;
        let cast = |v: &[f64]| -> Vec<R> { v.iter().map(|&x| R::from_f64(x)).collect() };
        match l.kind {
            LinearKind::Identity => {
                return Ok(LinearObject {
                    class,
                    state: None,
                    translation_only: false,
                    parameters: Vec::new(),
                    fixed_parameters: Vec::new(),
                });
            }
            LinearKind::Translation => {
                let p = cast(&l.parameters);
                let offset = [p[0], p[1], p[2]];
                return Ok(LinearObject {
                    class,
                    state: Some(MatrixOffsetState {
                        matrix: MatrixOffsetState::<R>::identity().matrix,
                        offset,
                        center: [R::ZERO; 3],
                        translation: offset,
                    }),
                    translation_only: true,
                    parameters: offset.iter().map(|v| v.to_f64()).collect(),
                    fixed_parameters: Vec::new(),
                });
            }
            _ => {}
        }
        let (m, _) = l.with_precision_mode::<R>(mode)?;
        let center: [R; 3] = if l.fixed_parameters.len() >= 3 {
            std::array::from_fn(|i| R::from_f64(l.fixed_parameters[i]))
        } else {
            [R::ZERO; 3]
        };
        let p = cast(&l.parameters);
        let (translation, parameters): ([R; 3], Vec<R>) = match l.kind {
            LinearKind::Affine => ([p[9], p[10], p[11]], p.clone()),
            LinearKind::Euler3D => ([p[3], p[4], p[5]], p.clone()),
            LinearKind::VersorRigid3D | LinearKind::Similarity3D => {
                // GetParameters gives the stored versor, scaled down where its axis reached
                // norm 1 (VersorRigid3DTransform / Similarity3DTransform::SetParameters).
                let norm = if l.kind == LinearKind::VersorRigid3D {
                    (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt().to_f64()
                } else {
                    let mut n = (p[0] * p[0]).to_f64();
                    n += (p[1] * p[1]).to_f64();
                    n += (p[2] * p[2]).to_f64();
                    if n > 0.0 { n.sqrt() } else { n }
                };
                let ((x, y, z, _), _) = versor([p[0], p[1], p[2]], norm, mode)?;
                let mut out = vec![x, y, z, p[3], p[4], p[5]];
                if l.kind == LinearKind::Similarity3D {
                    out.push(p[6]);
                }
                ([p[3], p[4], p[5]], out)
            }
            // GetParameters: the quaternion and the translation as stored.
            LinearKind::QuaternionRigid => ([p[4], p[5], p[6]], p.clone()),
            // The scales; no translation.
            LinearKind::Scale => ([R::ZERO; 3], p.clone()),
            LinearKind::ScaleVersor3D | LinearKind::ScaleSkewVersor3D => {
                // The versor's components, then the translation, the scales and the skews.
                let mut norm = (p[0] * p[0]).to_f64();
                norm += (p[1] * p[1]).to_f64();
                norm += (p[2] * p[2]).to_f64();
                if norm > 0.0 {
                    norm = norm.sqrt();
                }
                let ((x, y, z, _), _) = versor([p[0], p[1], p[2]], norm, mode)?;
                let mut out = vec![x, y, z];
                out.extend_from_slice(&p[3..]);
                ([p[3], p[4], p[5]], out)
            }
            LinearKind::Translation | LinearKind::Identity => unreachable!(),
        };
        Ok(LinearObject {
            class,
            state: Some(MatrixOffsetState {
                matrix: m.matrix,
                offset: m.offset,
                center,
                translation,
            }),
            translation_only: false,
            parameters: parameters.iter().map(|v| v.to_f64()).collect(),
            fixed_parameters: matrix_offset_fixed(class, &l.fixed_parameters, &center),
        })
    }

    /// `GetInverseTransform()`: see the module documentation. `None` where ITK finds the
    /// matrix singular (ANTs: "Inverse does not exist").
    pub fn inverse(&self) -> Option<Self> {
        if self.class == "IdentityTransform" {
            return Some(self.clone());
        }
        let state = self.state.as_ref()?;
        if self.translation_only {
            let offset = state.offset.map(|v| -v);
            return Some(LinearObject {
                class: self.class,
                state: Some(MatrixOffsetState {
                    offset,
                    translation: offset,
                    ..*state
                }),
                translation_only: true,
                parameters: offset.iter().map(|v| v.to_f64()).collect(),
                fixed_parameters: Vec::new(),
            });
        }
        let inv = state.inverse()?;
        let class = if self.class == "AffineTransform" {
            "AffineTransform"
        } else {
            "MatrixOffsetTransformBase"
        };
        Some(LinearObject {
            class,
            parameters: inv.parameters(),
            fixed_parameters: self.fixed_parameters.clone(),
            state: Some(inv),
            translation_only: false,
        })
    }
}

/// A chain entry as an ITK object in `R`.
#[derive(Clone, Debug)]
pub enum ChainObject<'a, R> {
    Linear(LinearObject<R>),
    Field(&'a DisplacementField),
    BSpline(&'a BSplineTransform),
}

impl TransformChain {
    /// The chain's transforms as ANTs's `CompositeTransform<R>` holds them, in queue order:
    /// each read as `TransformFileReaderTemplate<R>` reads it and inverted where the chain
    /// inverts it. `mode` corrects or reproduces ITK-10.
    pub fn objects<R: ItkReal>(
        &self,
        mode: UpstreamDefects,
    ) -> Result<Vec<ChainObject<'_, R>>, TransformError> {
        self.transforms
            .iter()
            .zip(&self.inverted_from)
            .map(|(t, from)| {
                Ok(match (t, from) {
                    (Transform::Linear(_), Some(original)) => ChainObject::Linear(
                        LinearObject::<R>::read(original, mode)?
                            .inverse()
                            .ok_or_else(|| {
                                TransformError::Invalid("singular linear transform".into())
                            })?,
                    ),
                    (Transform::Linear(l), None) => {
                        ChainObject::Linear(LinearObject::<R>::read(l, mode)?)
                    }
                    (Transform::DisplacementField(f), _) => ChainObject::Field(f),
                    (Transform::BSpline(b), _) => ChainObject::BSpline(b),
                    (Transform::Composite(_), _) => unreachable!("composites are expanded"),
                })
            })
            .collect()
    }

    /// [`TransformChain::objects`] as stored transforms (`precision` names their class).
    pub fn itk_objects(
        &self,
        precision: Precision,
        mode: UpstreamDefects,
    ) -> Result<Vec<ItkObject<'_>>, TransformError> {
        fn convert<'a, R: ItkReal>(
            objects: Vec<ChainObject<'a, R>>,
            precision: Precision,
        ) -> Vec<ItkObject<'a>> {
            objects
                .into_iter()
                .map(|o| match o {
                    ChainObject::Linear(l) => ItkObject {
                        name: class_name(l.class, precision),
                        parameters: ObjectParameters::Values(l.parameters),
                        fixed_parameters: l.fixed_parameters,
                    },
                    ChainObject::Field(f) => ItkObject {
                        name: class_name(class_base(&f.itk_name), precision),
                        parameters: ObjectParameters::Field(f),
                        fixed_parameters: f.fixed_parameters(),
                    },
                    // `BSplineTransform<R>` holds its coefficients as `R`.
                    ChainObject::BSpline(b) => ItkObject {
                        name: class_name("BSplineTransform", precision),
                        parameters: ObjectParameters::Values(b.parameters_as::<R>()),
                        fixed_parameters: b.fixed_parameters(),
                    },
                })
                .collect()
        }
        Ok(match precision {
            Precision::Double => convert(self.objects::<f64>(mode)?, precision),
            Precision::Float => convert(self.objects::<f32>(mode)?, precision),
        })
    }

    /// ANTs's `CollapseLinearTransforms` in `R`: a new `AffineTransform` whose centre is that
    /// of the last matrix-offset transform of the queue, composed (`Compose(nth, true)`) with
    /// an `AffineTransform` made of each transform in queue order (a translation by its offset;
    /// any other by its centre, matrix and translation). Errors where ANTs fails or crashes:
    /// a transform that is not linear ("The transform or set of transforms is not linear."), an
    /// `IdentityTransform` (ANTs dereferences a null pointer).
    pub fn collapse_linear<R: ItkReal>(
        &self,
        mode: UpstreamDefects,
    ) -> Result<MatrixOffsetState<R>, TransformError> {
        let objects = self.objects::<R>(mode)?;
        let mut linear = Vec::with_capacity(objects.len());
        for o in &objects {
            match o {
                ChainObject::Linear(l) => linear.push(l),
                ChainObject::Field(_) | ChainObject::BSpline(_) => {
                    return Err(TransformError::Invalid(
                        "The transform or set of transforms is not linear.".into(),
                    ));
                }
            }
        }
        let mut total = MatrixOffsetState::<R>::identity();
        if let Some(last) = linear
            .iter()
            .rev()
            .find(|l| l.state.is_some() && !l.translation_only)
        {
            total.set_center(last.state.expect("found").center);
        }
        for l in linear {
            let Some(state) = l.state else {
                return Err(TransformError::Invalid(
                    "an IdentityTransform cannot be collapsed (ANTs crashes: \
                     CollapseLinearTransforms takes it for a matrix-offset transform)"
                        .into(),
                ));
            };
            let mut nth = MatrixOffsetState::<R>::identity();
            if l.translation_only {
                nth.set_offset(state.offset);
            } else {
                nth.set_center(state.center);
                nth.set_matrix(state.matrix);
                nth.set_translation(state.translation);
            }
            total.compose_pre(&nth);
        }
        Ok(total)
    }

    /// ITK's `TransformToDisplacementFieldFilter<Image<Vector<R, 3>, 3>, R>` on `grid` (the
    /// reference image's): for every voxel, `T(p) − p` with `p` its physical point
    /// (`TransformIndexToPhysicalPoint`, in double: the filter's points are the image's), the
    /// transform mapping it as `Point<R>`, the difference taken in double and stored as `R`.
    /// When every transform is linear, each scan line is mapped at its start and one voxel
    /// past its end, and the displacements in between are interpolated, `(1 − α)·start +
    /// α·end` in double with `α = x / nx`. Returns the vectors interleaved (`x, y, z` of each
    /// voxel, voxels `x` fastest) and the notices of ITK-01.
    pub fn displacement_field<R: ItkReal>(
        &self,
        grid: &Grid3,
        mode: UpstreamDefects,
    ) -> Result<(Vec<R>, Vec<Notice>), TransformError> {
        let (chain, prepared) = self.prepare::<R>(mode)?;
        let mut notices = Notices::new(mode);
        notices.extend(prepared);
        // An empty queue gets an identity in ANTs (a MatrixOffsetTransformBase): linear.
        let linear = chain.is_linear();
        let [nx, ny, nz] = grid.size;
        // The filter's points are the image's, `Point<double>` (`TransformIndexToPhysicalPoint`);
        // the transform maps them as `Point<R>` and its result is widened back to double, so
        // the displacement `T(p) − p` is a double difference, stored as `R`.
        let disp = |p: [f64; 3], floor: &IndexFloor| -> [f64; 3] {
            let q = chain.transform_point_with(p.map(R::from_f64), floor);
            std::array::from_fn(|i| q[i].to_f64() - p[i])
        };
        // The floor of displacement-field lookups (ITK-01): a vector whose mapping met a
        // differing floor is computed again with the other floor, and counted if it changed.
        let floor = IndexFloor::new(mode);
        let lines: Vec<Vec<R>> = (0..ny * nz)
            .into_par_iter()
            .map(|line| {
                let (y, z) = (line % ny, line / ny);
                let mut out = Vec::with_capacity(3 * nx);
                if linear {
                    // A linear chain looks nothing up, so takes no floor. The scan line is
                    // mapped at its start and one voxel past its end.
                    let start = disp(grid.voxel_to_point([0, y, z]), &floor);
                    let end = disp(grid.voxel_to_point([nx, y, z]), &floor);
                    for x in 0..nx {
                        let alpha = x as f64 / nx as f64;
                        let one_minus = 1.0 - alpha;
                        for i in 0..3 {
                            out.push(R::from_f64(one_minus * start[i] + alpha * end[i]));
                        }
                    }
                } else {
                    for x in 0..nx {
                        let p = grid.voxel_to_point([x, y, z]);
                        let v = floor.checked(
                            |f| disp(p, f).map(R::from_f64),
                            |a, b| (0..3).all(|i| same_value(&a[i], &b[i])),
                        );
                        out.extend_from_slice(&v);
                    }
                }
                out
            })
            .collect();
        floor.notice(&mut notices, "displacement vectors");
        Ok((lines.into_iter().flatten().collect(), notices.into_vec()))
    }
}

#[cfg(test)]
#[allow(clippy::needless_range_loop)]
mod tests {
    use super::*;

    fn lin(name: &str, p: &[f64], f: &[f64]) -> LinearTransform {
        LinearTransform::from_itk(name, p, f).unwrap()
    }

    /// The classes ITK's `GetInverseTransform` makes, with their fixed parameters.
    #[test]
    fn inverse_classes() {
        let euler = lin(
            "Euler3DTransform_double_3_3",
            &[0.1, -0.2, 0.3, 1.0, 2.0, 3.0],
            &[4.0, 5.0, 6.0],
        );
        let o = LinearObject::<f64>::read(&euler, UpstreamDefects::Fix).unwrap();
        assert_eq!(o.class, "Euler3DTransform");
        assert_eq!(o.fixed_parameters, vec![4.0, 5.0, 6.0, 0.0]);
        let inv = o.inverse().unwrap();
        assert_eq!(inv.class, "MatrixOffsetTransformBase");
        assert_eq!(inv.fixed_parameters, vec![4.0, 5.0, 6.0, 0.0]);
        // The inverse maps the original's image back: M⁻¹(M x + o) − M⁻¹ o = x.
        let s = inv.state.unwrap();
        let fwd = o.state.unwrap();
        let x = [1.5, -2.0, 0.25];
        let y: [f64; 3] = std::array::from_fn(|i| {
            fwd.offset[i] + (0..3).map(|j| fwd.matrix[i][j] * x[j]).sum::<f64>()
        });
        for i in 0..3 {
            let back: f64 = s.offset[i] + (0..3).map(|j| s.matrix[i][j] * y[j]).sum::<f64>();
            assert!((back - x[i]).abs() < 1e-12);
        }
        let affine = lin(
            "AffineTransform_double_3_3",
            &[1.0, 0.1, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 3.0],
            &[1.0, 1.0, 1.0],
        );
        let a = LinearObject::<f64>::read(&affine, UpstreamDefects::Fix).unwrap();
        assert_eq!(a.inverse().unwrap().class, "AffineTransform");
        let t = lin("TranslationTransform_double_3_3", &[1.0, -2.0, 0.5], &[]);
        let ti = LinearObject::<f64>::read(&t, UpstreamDefects::Fix)
            .unwrap()
            .inverse()
            .unwrap();
        assert_eq!(
            (ti.class, ti.parameters, ti.fixed_parameters),
            ("TranslationTransform", vec![-1.0, 2.0, -0.5], vec![])
        );
    }

    /// `CollapseLinearTransforms` of a translation and an affine equals mapping by both.
    #[test]
    fn collapse() {
        let affine = lin(
            "AffineTransform_double_3_3",
            &[1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.5, 1.0, 0.0, 0.0],
            &[2.0, 0.0, 0.0],
        );
        let t = lin("TranslationTransform_double_3_3", &[0.0, 1.0, 0.0], &[]);
        let chain = TransformChain::new([
            (Transform::Linear(affine), false),
            (Transform::Linear(t), false),
        ])
        .unwrap();
        let total = chain.collapse_linear::<f64>(UpstreamDefects::Fix).unwrap();
        // The centre is the affine's (the last matrix-offset transform of the queue).
        assert_eq!(total.center, [2.0, 0.0, 0.0]);
        let x = [3.0, -1.0, 4.0];
        let mapped = chain.transform_point(x);
        for i in 0..3 {
            let v: f64 = total.offset[i] + (0..3).map(|j| total.matrix[i][j] * x[j]).sum::<f64>();
            assert!((v - mapped[i]).abs() < 1e-12);
        }
    }

    /// A linear chain's field is interpolated along scan lines; a chain with a field is mapped
    /// point by point; both give `T(p) − p`.
    #[test]
    fn displacement_fields() {
        let grid = Grid3::new(
            [4, 3, 2],
            [1.0, 2.0, 1.5],
            [1.0, -1.0, 0.5],
            larmorx_core::linalg::IDENTITY3,
        )
        .unwrap();
        let t = lin("TranslationTransform_double_3_3", &[0.5, 1.0, -2.0], &[]);
        let chain = TransformChain::new([(Transform::Linear(t), false)]).unwrap();
        let (v, notices) = chain
            .displacement_field::<f64>(&grid, UpstreamDefects::Fix)
            .unwrap();
        assert!(notices.is_empty());
        assert_eq!(v.len(), 3 * grid.len());
        for c in v.chunks(3) {
            assert_eq!(c, [0.5, 1.0, -2.0]);
        }
    }
}
