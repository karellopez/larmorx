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
