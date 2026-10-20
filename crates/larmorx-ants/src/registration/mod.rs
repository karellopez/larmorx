// SPDX-License-Identifier: Apache-2.0
//! `antsRegistration`: ANTs's registration of a moving image (or images) onto a fixed image in
//! stages, each a transform optimised over a multi-resolution pyramid against one or more
//! metrics.
//!
//! Ported from ANTs v2.6.5 (`Examples/antsRegistration.cxx`,
//! `Examples/antsRegistrationTemplateHeader.h`, `Examples/itkantsRegistrationHelper.h/.hxx`,
//! the iteration observers) on ITK v5.4.5's v4 registration framework, which is in the crate
//! `larmorx-registration`. This module holds the registration's configuration as ANTs builds it
//! from its command line ([`RegistrationConfig`]), the stages ANTs runs, and the outputs it
//! writes.
//!
//! **Scope (A4 part 2).** The linear stages (`Rigid`, `Affine`, `CompositeAffine`,
//! `Similarity`, `Translation`) are ported. The deformable ones (`BSpline`,
//! `GaussianDisplacementField`, `BSplineDisplacementField`, `TimeVaryingVelocityField`,
//! `TimeVaryingBSplineVelocityField`, `SyN`, `BSplineSyN`, `Exponential`,
//! `BSplineExponential`) are parsed and checked as ANTs parses them, and refused when a stage
//! would run them: they are A5's.

use larmorx_core::Precision;

pub mod config;
pub mod helper;
pub mod pointsets;
pub mod syn;

pub use config::*;

/// What a stage's transform is (`RegistrationHelper::XfrmMethod`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XfrmMethod {
    Rigid,
    Affine,
    CompositeAffine,
    Similarity,
    Translation,
    BSpline,
    GaussianDisplacementField,
    BSplineDisplacementField,
    TimeVaryingVelocityField,
    TimeVaryingBSplineVelocityField,
    SyN,
    BSplineSyN,
    Exponential,
    BSplineExponential,
}

impl XfrmMethod {
    /// `StringToXfrmMethod` of a lower-cased name.
    pub fn from_name(name: &str) -> Option<XfrmMethod> {
        Some(match name {
            "rigid" => XfrmMethod::Rigid,
            "affine" => XfrmMethod::Affine,
            "compositeaffine" | "compaff" => XfrmMethod::CompositeAffine,
            "similarity" => XfrmMethod::Similarity,
            "translation" => XfrmMethod::Translation,
            "bspline" | "ffd" => XfrmMethod::BSpline,
            "gaussiandisplacementfield" | "gdf" => XfrmMethod::GaussianDisplacementField,
            "bsplinedisplacementfield" | "dmffd" => XfrmMethod::BSplineDisplacementField,
            "timevaryingvelocityfield" | "tvf" => XfrmMethod::TimeVaryingVelocityField,
            "timevaryingbsplinevelocityfield" | "tvdmffd" => {
                XfrmMethod::TimeVaryingBSplineVelocityField
            }
            "syn" | "symmetricnormalization" => XfrmMethod::SyN,
            "bsplinesyn" => XfrmMethod::BSplineSyN,
            "exp" | "exponential" => XfrmMethod::Exponential,
            "bsplineexponential" => XfrmMethod::BSplineExponential,
            _ => return None,
        })
    }

    /// `TransformMethod::XfrmMethodAsString`.
    pub fn as_str(self) -> &'static str {
        match self {
            XfrmMethod::Rigid => "Rigid",
            XfrmMethod::Affine => "Affine",
            XfrmMethod::CompositeAffine => "CompositeAffine",
            XfrmMethod::Similarity => "Similarity",
            XfrmMethod::Translation => "Translation",
            XfrmMethod::BSpline => "BSpline",
            XfrmMethod::GaussianDisplacementField => "GaussianDisplacementField",
            XfrmMethod::BSplineDisplacementField => "BSplineDisplacementField",
            XfrmMethod::TimeVaryingVelocityField => "TimeVaryingVelocityField",
            XfrmMethod::TimeVaryingBSplineVelocityField => "TimeVaryingBSplineVelocityField",
            XfrmMethod::SyN => "SyN",
            XfrmMethod::BSplineSyN => "BSplineSyN",
            XfrmMethod::Exponential => "Exponential",
            XfrmMethod::BSplineExponential => "BSplineExponential",
        }
    }

    /// Whether the transform is one of the linear ones this part ports.
    pub fn is_linear(self) -> bool {
        matches!(
            self,
            XfrmMethod::Rigid
                | XfrmMethod::Affine
                | XfrmMethod::CompositeAffine
                | XfrmMethod::Similarity
                | XfrmMethod::Translation
        )
    }

    /// The ITK class ANTs optimises for this method in `dimension` (the `*TransformTraits`:
    /// in 4-D, Rigid, Similarity and CompositeAffine fall back to `AffineTransform`).
    pub fn itk_class(self, dimension: usize) -> &'static str {
        match (self, dimension) {
            (XfrmMethod::Rigid, 2) => "Euler2DTransform",
            (XfrmMethod::Rigid, 3) => "Euler3DTransform",
            (XfrmMethod::Similarity, 2) => "Similarity2DTransform",
            (XfrmMethod::Similarity, 3) => "Similarity3DTransform",
            (XfrmMethod::CompositeAffine, 2) => "ANTSCenteredAffine2DTransform",
            (XfrmMethod::CompositeAffine, 3) => "ANTSAffine3DTransform",
            (XfrmMethod::Translation, _) => "TranslationTransform",
            (XfrmMethod::Rigid | XfrmMethod::Similarity | XfrmMethod::CompositeAffine, _)
            | (XfrmMethod::Affine, _) => "AffineTransform",
            _ => "",
        }
    }
}

/// What a metric is (`RegistrationHelper::MetricEnumeration`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricKind {
    /// ANTs's neighbourhood cross correlation (`CC`).
    Cc,
    /// Joint histogram mutual information (`MI2`).
    Mi,
    /// Mattes mutual information (`Mattes`, `MI`).
    Mattes,
    /// `MeanSquares` (`MSQ`, `SSD`).
    MeanSquares,
    Demons,
    /// Global correlation.
    Gc,
    Icp,
    Pse,
    Jhct,
    Igdm,
}

impl MetricKind {
    /// `StringToMetricType` of a lower-cased name.
    pub fn from_name(name: &str) -> Option<MetricKind> {
        Some(match name {
            "cc" => MetricKind::Cc,
            "mi2" => MetricKind::Mi,
            "mattes" | "mi" => MetricKind::Mattes,
            "meansquares" | "msq" | "ssd" => MetricKind::MeanSquares,
            "demons" => MetricKind::Demons,
            "gc" => MetricKind::Gc,
            "icp" => MetricKind::Icp,
            "pse" => MetricKind::Pse,
            "jhct" => MetricKind::Jhct,
            "igdm" => MetricKind::Igdm,
            _ => return None,
        })
    }

    /// `Metric::GetMetricAsString`.
    pub fn as_str(self) -> &'static str {
        match self {
            MetricKind::Cc => "CC",
            MetricKind::Mi => "JointHistogramMI",
            MetricKind::Mattes => "MattesMI",
            MetricKind::MeanSquares => "MeanSquares",
            MetricKind::Demons => "Demons",
            MetricKind::Gc => "GC",
            MetricKind::Icp => "ICP",
            MetricKind::Pse => "PSE",
            MetricKind::Jhct => "JHCT",
            MetricKind::Igdm => "IGDM",
        }
    }

    /// `IsPointSetMetric`.
    pub fn is_point_set(self) -> bool {
        matches!(
            self,
            MetricKind::Icp | MetricKind::Pse | MetricKind::Jhct | MetricKind::Igdm
        )
    }
}

/// The metric's sampling (`SamplingStrategy`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sampling {
    None,
    Regular,
    Random,
}

/// The computation type's name as ANTs prints it ("Using double precision for
/// computations.").
pub fn precision_word(p: Precision) -> &'static str {
    match p {
        Precision::Double => "double",
        Precision::Float => "single",
    }
}
