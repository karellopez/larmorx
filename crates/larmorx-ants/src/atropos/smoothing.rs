// SPDX-License-Identifier: Apache-2.0
//! Atropos' adaptive smoothing: `BSplineScatteredDataPointSetToImageFilter` with several
//! fitting levels and point weights, and `BSplineControlPointImageFilter` (ITK v5.4.5), as
//! `GetSmoothIntensityImageFromPriorImage` runs them.

/// A fitted control-point lattice (`GetPhiLattice`), kept per intensity image and class.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlLattice {
    pub size: Vec<usize>,
    pub data: Vec<f32>,
}
