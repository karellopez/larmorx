// SPDX-License-Identifier: Apache-2.0
//! Spatial transforms for larmorx (PLAN.md §5), in ITK's conventions.
//!
//! Transforms map **LPS** physical points, as ITK and ANTs do. [`LinearTransform`] covers
//! ITK's matrix-offset family (affine, Euler, versor rigid, similarity, translation),
//! [`DisplacementField`] dense deformations, and [`Transform::Composite`] ITK's composite
//! transforms. [`TransformChain`] is what `antsApplyTransforms` builds from its `-t` options.
//!
//! Two modules follow other conventions: [`nitransforms`] maps **RAS** points as nitransforms
//! does (fMRIPrep's transforms), and [`resample_series`] is fMRIPrep's one-shot BOLD resampler
//! built on it and on SciPy-compatible interpolation (`larmorx_interp::ndimage`).
//! [`openblas`] computes the 4×4 matrix products and inverses they need as numpy does with
//! OpenBLAS's Haswell kernels, in a fixed order, so they are the same on every platform.
#![forbid(unsafe_code)]

use larmorx_core::defects::{self, Notice, Notices, UpstreamDefects};
use larmorx_core::{IndexFloor, ItkReal};

pub mod field;
pub mod linear;
pub mod nitransforms;
pub mod openblas;
pub mod resample_series;

pub use field::{DisplacementField, FieldData};
pub use linear::{LinearKind, LinearTransform, MatrixOffset};

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
    /// For each entry of `transforms` that is an inverted linear transform, the transform
    /// before inversion: another precision inverts it again in its own arithmetic.
    inverted_from: Vec<Option<LinearTransform>>,
}

/// Expands `t` (inverted if `invert`) into ITK's queue order, as
/// `t.inverse()?.flatten_into(...)` does, and records the original of every inverted linear
/// transform.
fn push_flattened(
    t: Transform,
    invert: bool,
    out: &mut Vec<Transform>,
    from: &mut Vec<Option<LinearTransform>>,
) -> Result<(), TransformError> {
    match (t, invert) {
        (Transform::Composite(ts), false) => {
            for c in ts {
                push_flattened(c, false, out, from)?;
            }
        }
        // A composite's inverse is its components inverted, in reverse order.
        (Transform::Composite(ts), true) => {
            for c in ts.into_iter().rev() {
                push_flattened(c, true, out, from)?;
            }
        }
        (Transform::Linear(l), true) => {
            out.push(Transform::Linear(l.inverse()?));
            from.push(Some(l));
        }
        (Transform::DisplacementField(_), true) => return Err(TransformError::NotInvertible),
        (t, false) => {
            out.push(t);
            from.push(None);
        }
    }
    Ok(())
}

impl TransformChain {
    /// A chain from `(transform, invert)` pairs in command-line order.
    pub fn new(items: impl IntoIterator<Item = (Transform, bool)>) -> Result<Self, TransformError> {
        let items: Vec<(Transform, bool)> = items.into_iter().collect();
        let mut transforms = Vec::new();
        let mut inverted_from = Vec::new();
        for (t, invert) in items.into_iter().rev() {
            push_flattened(t, invert, &mut transforms, &mut inverted_from)?;
        }
        Ok(TransformChain {
            transforms,
            inverted_from,
        })
    }

    /// The chain instantiated with the computation type `R`, as ANTs builds its
    /// `CompositeTransform<R>`: each linear transform from its stored parameters in `R`
    /// (inverted in `R` where the chain inverts it), displacement fields looked up in `R`.
    /// For `R = f64` it maps points exactly as [`TransformChain::transform_point`].
    pub fn with_precision<R: ItkReal>(&self) -> Result<PrecisionChain<'_, R>, TransformError> {
        self.prepare::<R>(UpstreamDefects::Reproduce)
            .map(|(chain, _)| chain)
    }

    /// [`TransformChain::with_precision`] with ITK's defects corrected
    /// ([`UpstreamDefects::Fix`]) or reproduced: ITK-10 (a versor that fails to load in
    /// float) and ITK-01 (the floor of displacement-field lookups,
    /// [`PrecisionChain::index_floor`]). Returns the chain and the notices of the defects met
    /// while building it (ITK-01's are counted while the chain maps points).
    pub fn prepare<R: ItkReal>(
        &self,
        mode: UpstreamDefects,
    ) -> Result<(PrecisionChain<'_, R>, Vec<Notice>), TransformError> {
        let mut notices = Notices::new(mode);
        let mut steps = Vec::with_capacity(self.transforms.len());
        let mut instantiate = |l: &LinearTransform| -> Result<MatrixOffset<R>, TransformError> {
            let (m, corrected) = l.with_precision_mode::<R>(mode)?;
            if corrected {
                notices.add(
                    &defects::ITK_10,
                    format!(
                        "the {} is loaded in single precision from its versor computed in double; \
                         ITK fails to load it (the versor's axis, rounded to float, is longer \
                         than 1)",
                        l.itk_name
                    ),
                );
            }
            Ok(m)
        };
        for (t, from) in self.transforms.iter().zip(&self.inverted_from) {
            steps.push(match (t, from) {
                (Transform::Linear(_), Some(original)) => {
                    Step::Linear(instantiate(original)?.inverse().ok_or_else(|| {
                        TransformError::Invalid("singular linear transform".into())
                    })?)
                }
                (Transform::Linear(l), None) => Step::Linear(instantiate(l)?),
                (Transform::DisplacementField(f), _) => Step::Field(f),
                (Transform::Composite(_), _) => unreachable!("composites are expanded"),
            });
        }
        Ok((
            PrecisionChain {
                steps,
                floor: IndexFloor::new(mode),
            },
            notices.into_vec(),
        ))
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

/// A [`TransformChain`] in the computation type `R` ([`TransformChain::with_precision`]).
#[derive(Clone, Debug)]
pub struct PrecisionChain<'a, R> {
    /// ITK's composite queue, applied from the back.
    steps: Vec<Step<'a, R>>,
    /// The floor of displacement-field lookups (ITK-01).
    floor: IndexFloor,
}

#[derive(Clone, Debug)]
enum Step<'a, R> {
    Linear(MatrixOffset<R>),
    Field(&'a DisplacementField),
}

impl<R: ItkReal> PrecisionChain<'_, R> {
    /// Maps a point (LPS) through the chain in `R` (`CompositeTransform<R>::TransformPoint`:
    /// the queue from the back, each transform taking and returning `Point<R>`).
    #[inline]
    pub fn transform_point(&self, p: [R; 3]) -> [R; 3] {
        self.transform_point_with(p, &self.floor)
    }

    /// [`PrecisionChain::transform_point`] with the displacement-field lookups' floor chosen
    /// by `floor` (ITK-01) instead of the chain's own.
    #[inline]
    pub fn transform_point_with(&self, p: [R; 3], floor: &IndexFloor) -> [R; 3] {
        self.steps.iter().rev().fold(p, |q, step| match step {
            Step::Linear(m) => m.transform_point(q),
            Step::Field(f) => f.transform_point_with(q, floor),
        })
    }

    /// The floor of the displacement-field lookups, with the count of lookups where ITK's
    /// floor differed from the exact one (ITK-01).
    pub fn index_floor(&self) -> &IndexFloor {
        &self.floor
    }

    /// Whether every transform is linear (ITK's `TransformCategory::Linear`).
    pub fn is_linear(&self) -> bool {
        self.steps.iter().all(|s| matches!(s, Step::Linear(_)))
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
    #[test]
    fn precision_chains() {
        let euler = LinearTransform::from_itk(
            "Euler3DTransform_double_3_3",
            &[0.1, -0.2, 0.3, 1.0, 2.0, 3.0],
            &[4.0, 5.0, 6.0],
        )
        .unwrap();
        let chain = TransformChain::new([
            (Transform::Linear(euler.clone()), true),
            (translation([1.0, 0.0, 0.5]), false),
            (scaling(2.0), true),
        ])
        .unwrap();
        let p = [3.0, -1.0, 2.5];
        // f64: the same mapping as the chain itself.
        assert_eq!(
            chain.with_precision::<f64>().unwrap().transform_point(p),
            chain.transform_point(p)
        );
        // f32: close to it, and computed from the original parameters in float.
        let single = chain.with_precision::<f32>().unwrap();
        let q = single.transform_point(p.map(|x| x as f32));
        for (a, b) in q.iter().zip(chain.transform_point(p)) {
            assert!((f64::from(*a) - b).abs() < 1e-5);
        }
        let inverted = euler.with_precision::<f32>().unwrap().inverse().unwrap();
        let first = single.steps.last().unwrap();
        assert!(matches!(first, Step::Linear(m) if *m == inverted));
        // Inverting a composite inverts its components in reverse order.
        let composite = Transform::Composite(vec![scaling(2.0), translation([0.0, 1.0, 0.0])]);
        let a = TransformChain::new([(composite.clone(), true)]).unwrap();
        let b = TransformChain::new([(composite.inverse().unwrap(), false)]).unwrap();
        assert_eq!(a.transforms(), b.transforms());
        assert_eq!(a.transform_point(p), b.transform_point(p));
    }
}
