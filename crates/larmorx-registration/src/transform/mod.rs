// SPDX-License-Identifier: Apache-2.0
//! The transforms of a registration and their derivatives (ITK v5.4.5,
//! `Modules/Core/Transform/include`; ANTs v2.6.5 `Utilities/itkANTSAffine3DTransform.hxx`,
//! `itkANTSCenteredAffine2DTransform.hxx`).
//!
//! Every transform here is of ITK's matrix-offset family (`y = M·x + offset`) or a
//! translation; they differ in their parameters, how a parameter update changes them, and
//! their Jacobian with respect to the parameters. [`Xfm`] holds one of them in the
//! computation type `R`; [`Composite`] is ITK's `CompositeTransform`, whose last transform is
//! the one a stage optimises (`SetOnlyMostRecentTransformToOptimizeOn`).
//!
//! Arithmetic follows each class: points and matrices in `R`; Jacobians computed in double
//! where the class declares double temporaries and stored in `R` (`JacobianType` is an
//! array of `R`).

mod linear;
mod set_matrix;

pub use linear::{Kind, LinearXfm, collapse_linear, inverse_r};

use crate::field::FieldXfm;
use crate::{ItkException, Matrix, Real};

/// One transform of a composite.
#[derive(Clone, Debug, PartialEq)]
pub enum Xfm<R, const D: usize> {
    /// `IdentityTransform`.
    Identity,
    /// `TranslationTransform`: `y = x + offset`; its parameters are the offset.
    Translation { offset: [R; D] },
    /// A transform of the matrix-offset family.
    Linear(LinearXfm<R, D>),
    /// `DisplacementFieldTransform` (SyN's output, or a field among the initial transforms).
    Field(FieldXfm<R, D>),
}

/// The exception of operations a displacement field does not support here (a linear stage's
/// optimizer never sees one).
fn field_unsupported(what: &str) -> ItkException {
    ItkException::new(
        "Modules/Filtering/DisplacementField/include/itkDisplacementFieldTransform.hxx",
        0,
        "DisplacementFieldTransform",
        format!("{what}: a displacement field is not optimised by a linear stage"),
    )
}

/// ITK's `Matrix · Point` in `R` (each row summed from 0) plus the offset.
#[inline]
pub fn matrix_point<R: Real, const D: usize>(
    m: &Matrix<R, D>,
    p: &[R; D],
    offset: &[R; D],
) -> [R; D] {
    std::array::from_fn(|r| {
        let mut s = R::ZERO;
        for c in 0..D {
            s = s + m[r][c] * p[c];
        }
        s + offset[r]
    })
}

impl<R: Real, const D: usize> Xfm<R, D> {
    /// `GetNameOfClass()`.
    pub fn class(&self) -> &'static str {
        match self {
            Xfm::Identity => "IdentityTransform",
            Xfm::Translation { .. } => "TranslationTransform",
            Xfm::Linear(l) => l.kind.class(),
            Xfm::Field(_) => "DisplacementFieldTransform",
        }
    }

    /// `TransformPoint`.
    #[inline]
    pub fn transform_point(&self, p: &[R; D]) -> [R; D] {
        match self {
            Xfm::Identity => *p,
            Xfm::Translation { offset } => std::array::from_fn(|i| p[i] + offset[i]),
            Xfm::Linear(l) => matrix_point(&l.matrix, p, &l.offset),
            Xfm::Field(f) => f.transform_point(p),
        }
    }

    /// `TransformCovariantVector(vector, point)`: unchanged by the identity and a translation;
    /// a matrix transform's inverse matrix transposed (vnl's inverse, [`linear::inverse_r`]);
    /// a field's approximate inverse Jacobian at the voxel nearest `p`, transposed (ITK's
    /// `Transform::TransformCovariantVector`). Each entry summed in `R` from 0.
    pub fn transform_covariant(&self, v: &[R; D], p: &[R; D]) -> [R; D] {
        let transposed = |m: &Matrix<R, D>| -> [R; D] {
            std::array::from_fn(|i| {
                let mut s = R::ZERO;
                for j in 0..D {
                    s = s + m[j][i] * v[j];
                }
                s
            })
        };
        match self {
            Xfm::Identity | Xfm::Translation { .. } => *v,
            Xfm::Linear(l) => match linear::inverse_r(&l.matrix) {
                Some(inv) => transposed(&inv),
                // ITK keeps the last inverse it could compute; a singular matrix has none.
                None => *v,
            },
            Xfm::Field(f) => transposed(&f.inverse_jacobian_position(p)),
        }
    }

    /// `ComputeJacobianWithRespectToPosition(p)`: the matrix (identity for a translation), a
    /// field's local Jacobian at the voxel nearest `p`.
    pub fn jacobian_position(&self, p: &[R; D]) -> Matrix<R, D> {
        match self {
            Xfm::Identity | Xfm::Translation { .. } => std::array::from_fn(|i| {
                std::array::from_fn(|j| if i == j { R::ONE } else { R::ZERO })
            }),
            Xfm::Linear(l) => l.matrix,
            Xfm::Field(f) => f.jacobian_position(p),
        }
    }

    /// `GetNumberOfParameters()`.
    pub fn number_of_parameters(&self) -> usize {
        match self {
            Xfm::Identity => 0,
            Xfm::Translation { .. } => D,
            Xfm::Linear(l) => l.kind.number_of_parameters(D),
            Xfm::Field(f) => D * f.field.space.len(),
        }
    }

    /// `Clone()` (`Transform::InternalClone`): a new object given this one's fixed parameters,
    /// then its parameters, which recomputes what they define. An inverse's offset, set
    /// directly by `GetInverse`, becomes the one `ComputeOffset` derives from its translation.
    pub fn itk_clone(&self) -> Result<Self, ItkException> {
        let mut out = self.clone();
        if let Xfm::Linear(l) = &mut out {
            let p = l.parameters();
            l.set_parameters(&p)?;
        }
        Ok(out)
    }

    /// `GetParameters()`.
    pub fn parameters(&self) -> Vec<R> {
        match self {
            Xfm::Identity => Vec::new(),
            Xfm::Translation { offset } => offset.to_vec(),
            Xfm::Linear(l) => l.parameters(),
            Xfm::Field(f) => f.field.data.iter().flatten().copied().collect(),
        }
    }

    /// `SetParameters`.
    pub fn set_parameters(&mut self, p: &[R]) -> Result<(), ItkException> {
        match self {
            Xfm::Identity => Ok(()),
            Xfm::Translation { offset } => {
                offset[..D].copy_from_slice(&p[..D]);
                Ok(())
            }
            Xfm::Linear(l) => l.set_parameters(p),
            Xfm::Field(_) => Err(field_unsupported("SetParameters")),
        }
    }

    /// `GetFixedParameters()` (as double).
    pub fn fixed_parameters(&self) -> Vec<f64> {
        match self {
            Xfm::Identity => Vec::new(),
            Xfm::Translation { .. } => vec![0.0; 0],
            Xfm::Linear(l) => l.fixed_parameters(),
            Xfm::Field(f) => f.fixed_parameters(),
        }
    }

    /// `UpdateTransformParameters(update, factor)`.
    pub fn update(&mut self, update: &[R], factor: R) -> Result<(), ItkException> {
        match self {
            Xfm::Identity => Ok(()),
            Xfm::Translation { offset } => {
                // Transform::UpdateTransformParameters: GetParameters, += update·factor
                // (or += update when factor is 1), SetParameters.
                for i in 0..D {
                    offset[i] = if factor == R::ONE {
                        offset[i] + update[i]
                    } else {
                        offset[i] + update[i] * factor
                    };
                }
                Ok(())
            }
            Xfm::Linear(l) => l.update(update, factor),
            Xfm::Field(_) => Err(field_unsupported("UpdateTransformParameters")),
        }
    }

    /// `ComputeJacobianWithRespectToParameters(p, J)`: `J` is `D` rows of
    /// [`Xfm::number_of_parameters`] values, row by row.
    pub fn jacobian_parameters(&self, p: &[R; D], out: &mut [R]) {
        match self {
            Xfm::Identity => {}
            // A displacement field's Jacobian is local: the identity for its D parameters at
            // the point.
            Xfm::Translation { .. } | Xfm::Field(_) => {
                out.fill(R::ZERO);
                for i in 0..D {
                    out[i * D + i] = R::ONE;
                }
            }
            Xfm::Linear(l) => l.jacobian_parameters(p, out),
        }
    }

    /// `IsLinear()`.
    pub fn is_linear(&self) -> bool {
        !matches!(self, Xfm::Field(_))
    }
}

/// ITK's `CompositeTransform` with only its last transform optimised.
#[derive(Clone, Debug, PartialEq)]
pub struct Composite<R, const D: usize> {
    /// The queue: the last transform is applied first.
    pub transforms: Vec<Xfm<R, D>>,
}

impl<R: Real, const D: usize> Composite<R, D> {
    pub fn new(transforms: Vec<Xfm<R, D>>) -> Self {
        Composite { transforms }
    }

    /// `TransformPoint`: the queue in reverse order.
    #[inline]
    pub fn transform_point(&self, p: &[R; D]) -> [R; D] {
        let mut q = *p;
        for t in self.transforms.iter().rev() {
            q = t.transform_point(&q);
        }
        q
    }

    /// `TransformCovariantVector(vector, point)`: the queue in reverse order, each transform
    /// at the point as the transforms before it moved it.
    pub fn transform_covariant(&self, v: &[R; D], p: &[R; D]) -> [R; D] {
        let mut out = *v;
        let mut q = *p;
        for t in self.transforms.iter().rev() {
            out = t.transform_covariant(&out, &q);
            q = t.transform_point(&q);
        }
        out
    }

    /// The optimised transform.
    pub fn active(&self) -> &Xfm<R, D> {
        self.transforms.last().expect("an empty composite")
    }

    pub fn active_mut(&mut self) -> &mut Xfm<R, D> {
        self.transforms.last_mut().expect("an empty composite")
    }

    /// `GetNumberOfParameters()` of the optimised part.
    pub fn number_of_parameters(&self) -> usize {
        self.active().number_of_parameters()
    }

    /// `ComputeJacobianWithRespectToParametersCachedTemporaries`: the active transform's
    /// Jacobian at `p`, then, for each transform applied after it, left-multiplied by that
    /// transform's Jacobian with respect to position (each product in `R`, summed in a
    /// double, stored in `R`).
    pub fn jacobian(&self, p: &[R; D], out: &mut [R]) {
        let n = self.transforms.len();
        let np = self.number_of_parameters();
        if n == 1 {
            self.transforms[0].jacobian_parameters(p, out);
            return;
        }
        // The last transform (applied first) is the only one optimised; each transform applied
        // after it multiplies by its Jacobian with respect to position at the point mapped so
        // far (a linear transform's does not depend on the point).
        self.transforms[n - 1].jacobian_parameters(p, out);
        let nonlinear = self.transforms.iter().any(|t| !t.is_linear());
        let mut q = if nonlinear {
            self.transforms[n - 1].transform_point(p)
        } else {
            *p
        };
        for t in self.transforms[..n - 1].iter().rev() {
            let jp = t.jacobian_position(&q);
            if nonlinear {
                q = t.transform_point(&q);
            }
            for c in 0..np {
                let mut temp = [0.0f64; D];
                for r in 0..D {
                    for k in 0..D {
                        temp[r] += (jp[r][k] * out[k * np + c]).to_f64();
                    }
                }
                for r in 0..D {
                    out[r * np + c] = R::from_f64(temp[r]);
                }
            }
        }
    }
}

/// ITK's transform categories as ANTs' collapse distinguishes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Category {
    Linear,
    DisplacementField,
}

fn category<R: Real, const D: usize>(t: &Xfm<R, D>) -> Category {
    if t.is_linear() {
        Category::Linear
    } else {
        Category::DisplacementField
    }
}

/// ANTs' `CollapseDisplacementFieldTransforms`: neighbouring fields that are both invertible
/// or both not are composed into one ([`crate::field::compose`], the later transform of the
/// queue warping the earlier one's field; their inverses composed the other way); a change of
/// invertibility starts a new field.
pub fn collapse_fields<R: Real, const D: usize>(
    group: &[crate::field::FieldXfm<R, D>],
    floor: &larmorx_core::IndexFloor,
) -> Vec<crate::field::FieldXfm<R, D>> {
    use crate::field::{FieldXfm, compose};
    use std::sync::Arc;
    let mut out = Vec::new();
    let Some(first) = group.first() else {
        return out;
    };
    let mut current = first.clone();
    let mut invertible = current.inverse.is_some();
    for nth in &group[1..] {
        let same = invertible == nth.inverse.is_some();
        if same {
            let total = compose(&current.field, &nth.field, floor);
            let total_inverse = if invertible {
                Some(compose(
                    nth.inverse.as_deref().expect("invertible"),
                    current.inverse.as_deref().expect("invertible"),
                    floor,
                ))
            } else {
                None
            };
            current = FieldXfm {
                field: Arc::new(total),
                inverse: total_inverse.map(Arc::new),
                exact_floor: current.exact_floor,
            };
        } else {
            out.push(FieldXfm {
                field: Arc::clone(&current.field),
                inverse: if invertible {
                    current.inverse.clone()
                } else {
                    None
                },
                exact_floor: current.exact_floor,
            });
            current = nth.clone();
            invertible = current.inverse.is_some();
        }
    }
    out.push(current);
    out
}

/// ANTs' `CollapseCompositeTransform`: a queue of linear transforms becomes one
/// `AffineTransform` ([`collapse_linear`]), a queue of displacement fields as few fields as
/// [`collapse_fields`] makes; a mixed queue is split into runs of one category, each collapsed,
/// except that the queue's last transform, when it starts a new run, is kept as it is. `None`
/// where ANTs crashes (an `IdentityTransform` in a linear run, ANTS-48).
pub fn collapse_composite<R: Real, const D: usize>(
    queue: &[Xfm<R, D>],
    floor: &larmorx_core::IndexFloor,
) -> Option<Vec<Xfm<R, D>>> {
    let collapse = |group: &[Xfm<R, D>], c: Category| -> Option<Vec<Xfm<R, D>>> {
        match c {
            Category::Linear => Some(vec![Xfm::Linear(collapse_linear(group)?)]),
            Category::DisplacementField => {
                let fields: Vec<_> = group
                    .iter()
                    .map(|t| match t {
                        Xfm::Field(f) => f.clone(),
                        _ => unreachable!("a run of fields"),
                    })
                    .collect();
                Some(
                    collapse_fields(&fields, floor)
                        .into_iter()
                        .map(Xfm::Field)
                        .collect(),
                )
            }
        }
    };
    if queue.iter().all(|t| t.is_linear()) {
        return collapse(queue, Category::Linear);
    }
    if queue.iter().all(|t| !t.is_linear()) {
        return collapse(queue, Category::DisplacementField);
    }
    let last = queue.len() - 1;
    let mut out = Vec::new();
    let mut current = category(&queue[0]);
    let mut group = vec![queue[0].clone()];
    for (n, t) in queue.iter().enumerate().skip(1) {
        let c = category(t);
        if c == current {
            group.push(t.clone());
            if n == last {
                out.extend(collapse(&group, current)?);
            }
        } else {
            out.extend(collapse(&group, current)?);
            group.clear();
            current = c;
            if n < last {
                group.push(t.clone());
            } else {
                out.push(t.clone());
            }
        }
    }
    Some(out)
}
