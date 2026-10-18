// SPDX-License-Identifier: Apache-2.0
//! Point sets and the point-set metrics of `antsRegistration` (ITK v5.4.5 and ANTs v2.6.5):
//!
//! - [`kdtree`], ITK's k-d tree (`PointsLocator`, `KdTreeGenerator`, `KdTree`);
//! - [`gaussian`], `GaussianMembershipFunction` and `ManifoldParzenWindowsPointSetFunction`;
//! - [`metric`], `PointSetToPointSetMetricWithIndexv4` and the metrics ICP, PSE, JHCT (labelled)
//!   and IGDM.

pub mod gaussian;
pub mod kdtree;
pub mod metric;

pub use metric::{
    FixedInverse, IntensityPointSet, LabeledPointSet, PointSetCorrections, PointSetInputs,
    PointSetMetric, PointSetMetricKind, PointSetValue,
};
