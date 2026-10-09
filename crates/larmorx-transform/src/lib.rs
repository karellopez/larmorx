// SPDX-License-Identifier: Apache-2.0
//! Spatial transforms for larmorx (PLAN.md §5), in ITK's conventions.
//!
//! Transforms map **LPS** physical points, as ITK and ANTs do. [`LinearTransform`] covers
//! ITK's matrix-offset family (affine, Euler, versor rigid, similarity, translation),
//! [`DisplacementField`] dense deformations, and [`Transform::Composite`] ITK's composite
//! transforms. [`TransformChain`] is what `antsApplyTransforms` builds from its `-t` options.
#![forbid(unsafe_code)]

pub mod field;
pub mod linear;

pub use field::{DisplacementField, FieldData};
pub use linear::{LinearKind, LinearTransform};

/// A transform cannot be built or used.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TransformError {
    #[error("unsupported transform type {0}")]
    Unsupported(String),
    #[error("invalid transform: {0}")]
    Invalid(String),
    #[error("a displacement field cannot be inverted; use its inverse field instead")]
    NotInvertible,
}

/// One ITK transform.
#[derive(Clone, Debug, PartialEq)]
pub enum Transform {
    Linear(LinearTransform),
    DisplacementField(DisplacementField),
    /// ITK's `CompositeTransform`: transforms in queue order, applied **last first**.
    Composite(Vec<Transform>),
}

impl Transform {
    /// Maps a point (LPS).
    pub fn transform_point(&self, p: [f64; 3]) -> [f64; 3] {
        match self {
            Transform::Linear(t) => t.transform_point(p),
            Transform::DisplacementField(t) => t.transform_point(p),
            Transform::Composite(ts) => ts.iter().rev().fold(p, |q, t| t.transform_point(q)),
        }
    }

    /// The inverse: linear transforms invert exactly; composites invert each component in
    /// reverse order; displacement fields cannot be inverted.
    pub fn inverse(&self) -> Result<Transform, TransformError> {
        match self {
            Transform::Linear(t) => Ok(Transform::Linear(t.inverse()?)),
            Transform::DisplacementField(_) => Err(TransformError::NotInvertible),
            Transform::Composite(ts) => Ok(Transform::Composite(
                ts.iter()
                    .rev()
                    .map(Transform::inverse)
                    .collect::<Result<_, _>>()?,
            )),
        }
    }

    /// The transforms in queue order, with nested composites expanded (same mapping).
    fn flatten_into(self, out: &mut Vec<Transform>) {
        match self {
            Transform::Composite(ts) => ts.into_iter().for_each(|t| t.flatten_into(out)),
            t => out.push(t),
        }
    }
}

/// The transform `antsApplyTransforms` applies: its `-t` options, each optionally inverted.
///
/// Mapping an output-grid point, the **first** `-t` acts first: `-t A -t B` maps `x` to
/// `B(A(x))`. (For images this reads the other way: the moving image is moved by `B` first.
/// So `-t warp -t affine` moves an image by the affine, then the warp.) This is how ANTs
/// builds it. Its parser hands the options back last first, ITK's `CompositeTransform`
/// queues them in that order, and the queue is applied back to front. The chain stores that
/// same queue.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransformChain {
    /// ITK's composite queue (composites expanded): applied from the back.
    transforms: Vec<Transform>,
}

impl TransformChain {
    /// A chain from `(transform, invert)` pairs in command-line order.
    pub fn new(items: impl IntoIterator<Item = (Transform, bool)>) -> Result<Self, TransformError> {
        let items: Vec<(Transform, bool)> = items.into_iter().collect();
        let mut transforms = Vec::new();
        for (t, invert) in items.into_iter().rev() {
            let t = if invert { t.inverse()? } else { t };
            t.flatten_into(&mut transforms);
        }
        Ok(TransformChain { transforms })
    }

    /// The transforms in ITK's composite-queue order (the last one acts first on a point).
    pub fn transforms(&self) -> &[Transform] {
        &self.transforms
    }

    pub fn is_empty(&self) -> bool {
        self.transforms.is_empty()
    }

    /// Maps a point (LPS) through the whole chain.
    pub fn transform_point(&self, p: [f64; 3]) -> [f64; 3] {
        self.transforms
            .iter()
            .rev()
            .fold(p, |q, t| t.transform_point(q))
    }

    /// Whether every transform is linear (so the chain is one affine map).
    pub fn is_linear(&self) -> bool {
        self.transforms
            .iter()
            .all(|t| matches!(t, Transform::Linear(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translation(t: [f64; 3]) -> Transform {
        Transform::Linear(
            LinearTransform::from_itk("TranslationTransform_double_3_3", &t, &[]).unwrap(),
        )
    }

    fn scaling(s: f64) -> Transform {
        Transform::Linear(LinearTransform::from_matrix(
            [[s, 0.0, 0.0], [0.0, s, 0.0], [0.0, 0.0, s]],
            [0.0; 3],
        ))
    }

    #[test]
    fn chains_map_points_first_option_first_like_ants() {
        // -t scale(2) -t translate(+1): x -> 2x + 1 (checked against antsApplyTransforms).
        let chain =
            TransformChain::new([(scaling(2.0), false), (translation([1.0, 0.0, 0.0]), false)])
                .unwrap();
        assert_eq!(chain.transform_point([3.0, 0.0, 0.0]), [7.0, 0.0, 0.0]);
        assert!(chain.is_linear());
        // Inverting the scale: x -> x / 2 + 1
        let chain =
            TransformChain::new([(scaling(2.0), true), (translation([1.0, 0.0, 0.0]), false)])
                .unwrap();
        assert_eq!(chain.transform_point([3.0, 0.0, 0.0]), [2.5, 0.0, 0.0]);
        // A composite option keeps its own order: -t C -t Composite[A, B] maps x to A(B(C(x))).
        let composite = Transform::Composite(vec![scaling(2.0), translation([1.0, 0.0, 0.0])]);
        let chain =
            TransformChain::new([(translation([0.0, 0.0, 1.0]), false), (composite, false)])
                .unwrap();
        assert_eq!(chain.transform_point([3.0, 0.0, 0.0]), [8.0, 0.0, 2.0]);
    }

    #[test]
    fn composites_flatten_without_changing_the_mapping() {
        let composite = Transform::Composite(vec![scaling(2.0), translation([0.0, 1.0, 0.0])]);
        let p = [1.0, 1.0, 1.0];
        let direct = composite.transform_point(p);
        let chain = TransformChain::new([(composite.clone(), false)]).unwrap();
        assert_eq!(chain.transforms().len(), 2);
        assert_eq!(chain.transform_point(p), direct);
        let back = composite.inverse().unwrap().transform_point(direct);
        assert!(p.iter().zip(back).all(|(a, b)| (a - b).abs() < 1e-12));
    }
}
