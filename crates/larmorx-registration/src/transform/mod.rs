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

pub mod bspline;
mod linear;
mod set_matrix;

pub use bspline::BSplineXfm;
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
    /// `BSplineTransform<R, D, 3>` (a `BSpline` stage's output, or among the initial
    /// transforms).
    BSpline(BSplineXfm<R, D>),
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
            Xfm::Field(f) => f.class(),
            Xfm::BSpline(_) => "BSplineTransform",
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
            Xfm::BSpline(b) => b.transform_point(p),
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
            // ITK throws (no Jacobian with respect to position). Covariant vectors are mapped
            // only through inverted fixed transforms, and a B-spline transform has no inverse,
            // so this is never reached.
            Xfm::BSpline(_) => *v,
        }
    }

    /// `ComputeJacobianWithRespectToPosition(p)`: the matrix (identity for a translation), a
    /// field's local Jacobian at the voxel nearest `p`. A B-spline transform throws in ITK
    /// ([`Composite::position_jacobian_error`]); the identity stands in here.
    pub fn jacobian_position(&self, p: &[R; D]) -> Matrix<R, D> {
        match self {
            Xfm::Identity | Xfm::Translation { .. } | Xfm::BSpline(_) => std::array::from_fn(|i| {
                std::array::from_fn(|j| if i == j { R::ONE } else { R::ZERO })
            }),
            Xfm::Linear(l) => l.matrix,
            Xfm::Field(f) => f.jacobian_position(p),
        }
    }

    /// `GetNumberOfLocalParameters()`: `D` for a displacement field, else every parameter.
    pub fn number_of_local_parameters(&self) -> usize {
        match self {
            Xfm::Field(_) => D,
            _ => self.number_of_parameters(),
        }
    }

    /// `GetNumberOfParameters()`.
    pub fn number_of_parameters(&self) -> usize {
        match self {
            Xfm::Identity => 0,
            Xfm::Translation { .. } => D,
            Xfm::Linear(l) => l.kind.number_of_parameters(D),
            Xfm::Field(f) => D * f.field.space.len(),
            Xfm::BSpline(b) => b.number_of_parameters(),
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
            Xfm::BSpline(b) => b.coefficients.clone(),
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
            Xfm::Field(f) => f.set_parameters(p),
            Xfm::BSpline(b) => b.set_parameters(p),
        }
    }

    /// `GetFixedParameters()` (as double).
    pub fn fixed_parameters(&self) -> Vec<f64> {
        match self {
            Xfm::Identity => Vec::new(),
            Xfm::Translation { .. } => vec![0.0; 0],
            Xfm::Linear(l) => l.fixed_parameters(),
            Xfm::Field(f) => f.fixed_parameters(),
            Xfm::BSpline(b) => b.fixed.clone(),
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
            Xfm::Field(f) => {
                let mut copy = update.to_vec();
                f.update(&mut copy, factor)
            }
            Xfm::BSpline(b) => b.update(update, factor),
        }
    }

    /// `UpdateTransformParameters(update, factor)` with ITK's side effect on the caller's
    /// array: a Gaussian-smoothing displacement field smooths `update` in place.
    pub fn update_in_place(&mut self, update: &mut [R], factor: R) -> Result<(), ItkException> {
        match self {
            Xfm::Field(f) => f.update(update, factor),
            other => other.update(update, factor),
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
            Xfm::BSpline(b) => b.jacobian_parameters(p, out),
        }
    }

    /// Whether the scales estimator treats the transform as one of local support
    /// (`TransformHasLocalSupportForScalesEstimation`: a displacement field).
    pub fn has_local_support(&self) -> bool {
        matches!(self, Xfm::Field(_))
    }

    /// `IsLinear()`.
    pub fn is_linear(&self) -> bool {
        !matches!(self, Xfm::Field(_) | Xfm::BSpline(_))
    }

    /// `GetTransformCategory() == BSpline`.
    pub fn is_bspline(&self) -> bool {
        matches!(self, Xfm::BSpline(_))
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

    /// `GetNumberOfLocalParameters()` of the optimised part.
    pub fn number_of_local_parameters(&self) -> usize {
        self.active().number_of_local_parameters()
    }

    /// The exception `ComputeJacobianWithRespectToParametersCachedTemporaries` throws: a
    /// B-spline transform applied after the active one has no Jacobian with respect to
    /// position (`BSplineBaseTransform::ComputeJacobianWithRespectToPosition`).
    pub fn position_jacobian_error(&self) -> Option<ItkException> {
        let n = self.transforms.len();
        (n > 1 && self.transforms[..n - 1].iter().any(Xfm::is_bspline))
            .then(BSplineXfm::<R, D>::no_position_jacobian)
    }

    /// `ComputeJacobianWithRespectToParametersCachedTemporaries`: the active transform's
    /// Jacobian at `p`, then, for each transform applied after it, left-multiplied by that
    /// transform's Jacobian with respect to position (each product in `R`, summed in a
    /// double, stored in `R`).
    pub fn jacobian(&self, p: &[R; D], out: &mut [R]) {
        let n = self.transforms.len();
        let np = self.number_of_local_parameters();
        if n == 1 {
            self.transforms[0].jacobian_parameters(p, out);
            return;
        }
        // The last transform (applied first) is the only one optimised; each transform applied
        // after it multiplies by its Jacobian with respect to position at the point mapped so
        // far (a linear transform's does not depend on the point).
        self.transforms[n - 1].jacobian_parameters(p, out);
        self.position_products(p, out, np);
    }

    /// [`Composite::jacobian`] for a B-spline transform being optimised: only the columns of
    /// the parameters whose control points support `p` (every other column is zero), as
    /// `(parameters, D × k values row by row)`; `None` for another active transform.
    pub fn sparse_jacobian(&self, p: &[R; D]) -> Option<(Vec<usize>, Vec<R>)> {
        let Xfm::BSpline(b) = self.active() else {
            return None;
        };
        let Some(support) = b.support(p) else {
            return Some((Vec::new(), Vec::new()));
        };
        let offsets = b.support_offsets(&support);
        let per = b.parameters_per_dimension();
        let s = offsets.len();
        let k = D * s;
        let mut cols = Vec::with_capacity(k);
        let mut out = vec![R::ZERO; D * k];
        for d in 0..D {
            for (i, &o) in offsets.iter().enumerate() {
                cols.push(d * per + o);
                out[d * k + d * s + i] = R::from_f64(support.weights[i]);
            }
        }
        if self.transforms.len() > 1 {
            self.position_products(p, &mut out, k);
        }
        Some((cols, out))
    }

    /// The active transform's Jacobian `out` (`D` rows of `np` columns) left-multiplied by the
    /// Jacobian with respect to position of each transform applied after it.
    fn position_products(&self, p: &[R; D], out: &mut [R], np: usize) {
        let n = self.transforms.len();
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
    BSpline,
}

fn category<R: Real, const D: usize>(t: &Xfm<R, D>) -> Category {
    match t {
        Xfm::Field(_) => Category::DisplacementField,
        Xfm::BSpline(_) => Category::BSpline,
        _ => Category::Linear,
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
                smoothing: current.smoothing.clone(),
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
                smoothing: None,
            });
            current = nth.clone();
            invertible = current.inverse.is_some();
        }
    }
    out.push(current);
    out
}

/// One run of a category collapsed (`CollapseLinearTransforms`,
/// `CollapseDisplacementFieldTransforms`); `None` where ANTs crashes: an `IdentityTransform` in
/// a linear run (ANTS-48), a transform of another category in the run (ANTs' collapse throws
/// outside a try block, ANTS-76).
fn collapse_run<R: Real, const D: usize>(
    group: &[Xfm<R, D>],
    c: Category,
    floor: &larmorx_core::IndexFloor,
) -> Option<Vec<Xfm<R, D>>> {
    if group.iter().any(|t| category(t) != c) {
        return None;
    }
    match c {
        Category::Linear => Some(vec![Xfm::Linear(collapse_linear(group)?)]),
        Category::DisplacementField => {
            let fields: Vec<_> = group
                .iter()
                .filter_map(|t| match t {
                    Xfm::Field(f) => Some(f.clone()),
                    _ => None,
                })
                .collect();
            Some(
                collapse_fields(&fields, floor)
                    .into_iter()
                    .map(Xfm::Field)
                    .collect(),
            )
        }
        Category::BSpline => Some(group.to_vec()),
    }
}

/// ANTs' `CollapseCompositeTransform`, as it is: a queue of linear transforms becomes one
/// `AffineTransform` ([`collapse_linear`]), a queue of displacement fields as few fields as
/// [`collapse_fields`] makes; otherwise the transforms before the first linear transform or
/// field are kept, and from there runs of one category are collapsed, except that the queue's
/// last transform, when it starts a new run, is kept as it is. B-spline transforms are kept
/// as they come, but the run logic does not expect them (ANTS-76): a transform after leading
/// B-spline transforms only is lost, a B-spline transform after another (after a linear or
/// field run) is lost at the end of the queue and joins the next run otherwise, where ANTs'
/// collapse throws. `None` where ANTs crashes (also ANTS-48).
pub fn collapse_composite_ants<R: Real, const D: usize>(
    queue: &[Xfm<R, D>],
    floor: &larmorx_core::IndexFloor,
) -> Option<Vec<Xfm<R, D>>> {
    if queue.iter().all(|t| t.is_linear()) {
        return collapse_run(queue, Category::Linear, floor);
    }
    if queue
        .iter()
        .all(|t| category(t) == Category::DisplacementField)
    {
        return collapse_run(queue, Category::DisplacementField, floor);
    }
    let last = queue.len() - 1;
    let mut out = Vec::new();
    let Some(start) = queue.iter().position(|t| category(t) != Category::BSpline) else {
        return Some(queue.to_vec());
    };
    out.extend(queue[..start].iter().cloned());
    let mut current = category(&queue[start]);
    let mut group = vec![queue[start].clone()];
    for (n, t) in queue.iter().enumerate().skip(start + 1) {
        let c = category(t);
        if c == current {
            group.push(t.clone());
            if n == last && current != Category::BSpline {
                out.extend(collapse_run(&group, current, floor)?);
            }
        } else {
            if current != Category::BSpline {
                out.extend(collapse_run(&group, current, floor)?);
                group.clear();
            }
            current = c;
            if c != Category::BSpline && n < last {
                group.push(t.clone());
            } else {
                out.push(t.clone());
            }
        }
    }
    Some(out)
}

/// [`collapse_composite_ants`] with ANTS-76 corrected: every B-spline transform is kept as it
/// is and ends the run before it; the linear and field runs around them are collapsed as ANTs
/// collapses runs (the queue's last transform, when it starts a new run, kept as it is).
/// Without B-spline transforms, ANTs' collapse. `None` where ANTs crashes (ANTS-48).
pub fn collapse_composite<R: Real, const D: usize>(
    queue: &[Xfm<R, D>],
    floor: &larmorx_core::IndexFloor,
) -> Option<Vec<Xfm<R, D>>> {
    if !queue.iter().any(Xfm::is_bspline) {
        return collapse_composite_ants(queue, floor);
    }
    let last = queue.len() - 1;
    let mut out = Vec::new();
    let mut group: Vec<Xfm<R, D>> = Vec::new();
    let mut current: Option<Category> = None;
    for (n, t) in queue.iter().enumerate() {
        let c = category(t);
        if c == Category::BSpline {
            if let Some(k) = current.take() {
                out.extend(collapse_run(&group, k, floor)?);
            }
            group.clear();
            out.push(t.clone());
            continue;
        }
        if current == Some(c) {
            group.push(t.clone());
            continue;
        }
        if let Some(k) = current.take() {
            out.extend(collapse_run(&group, k, floor)?);
        }
        group.clear();
        if n == last && n > 0 {
            out.push(t.clone());
        } else {
            group.push(t.clone());
            current = Some(c);
        }
    }
    if let Some(k) = current {
        out.extend(collapse_run(&group, k, floor)?);
    }
    Some(out)
}
