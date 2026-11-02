// SPDX-License-Identifier: Apache-2.0
//! Linear registration compatible with FSL's `flirt` (`specs/flirt.md`): a multi-resolution
//! schedule of searches and optimisations over 6 to 12 affine parameters (Jenkinson and Smith
//! 2001; Jenkinson et al. 2002), the boundary-based cost (Greve and Fischl 2009), the
//! resampling of the input onto the reference (`-out`, `-applyxfm`), and `convert_xfm`'s matrix
//! algebra (§10).
//!
//! Built on the main package's clean-room registration engine (`larmorx_mri::hmc`: its volumes,
//! parametrisation, sampling, cost sums, line search and kernels), shared with the head-motion
//! correction. Written from the spec, the papers and black-box runs of FSL 6.0.7.23's programs,
//! never from FSL's source or the replica (see `PROVENANCE.md`).

pub mod bbr;
pub mod cost;
pub mod engine;
pub mod mat;
pub mod optimise;
pub mod output;
pub mod pyramid;
pub mod schedule;
pub mod search;
pub mod xfm;

pub use cost::Cost;

/// The final interpolation (`-interp`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Interp {
    #[default]
    Trilinear,
    NearestNeighbour,
    Sinc,
    Spline,
}

impl Interp {
    pub fn from_name(name: &str) -> Option<Interp> {
        Some(match name {
            "trilinear" => Interp::Trilinear,
            "nearestneighbour" => Interp::NearestNeighbour,
            "sinc" => Interp::Sinc,
            "spline" => Interp::Spline,
            _ => return None,
        })
    }
}

/// The sinc window (`-sincwindow`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SincWindow {
    Rectangular,
    #[default]
    Hanning,
    Blackman,
}

impl SincWindow {
    pub fn from_name(name: &str) -> Option<SincWindow> {
        Some(match name {
            "rectangular" => SincWindow::Rectangular,
            "hanning" => SincWindow::Hanning,
            "blackman" => SincWindow::Blackman,
            _ => return None,
        })
    }
}

/// The output data type (`-datatype`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutType {
    Char,
    Short,
    Int,
    Float,
    Double,
}

impl OutType {
    pub fn from_name(name: &str) -> Option<OutType> {
        Some(match name {
            "char" => OutType::Char,
            "short" => OutType::Short,
            "int" => OutType::Int,
            "float" => OutType::Float,
            "double" => OutType::Double,
            _ => return None,
        })
    }
}

/// FLIRT's options (§2), as the command line gives them.
#[derive(Clone, Debug)]
pub struct Options {
    pub input: Option<String>,
    pub reference: Option<String>,
    pub out: Option<String>,
    pub omat: Option<String>,
    pub init: Option<String>,
    pub schedule: Option<String>,
    pub ref_weight: Option<String>,
    pub in_weight: Option<String>,
    pub wmseg: Option<String>,
    pub fieldmap: Option<String>,
    pub fieldmap_mask: Option<String>,
    pub wmcoords: Option<String>,
    pub wmnorms: Option<String>,
    pub bins: i32,
    pub dof: i32,
    pub sinc_width: f32,
    pub padding: f32,
    pub apply_xfm: bool,
    pub apply_iso: Option<f32>,
    pub base_scale: Option<f32>,
    pub min_sampling: f32,
    pub background: Option<f32>,
    /// The coarse and fine search steps, radians (single precision).
    pub coarse: f32,
    pub fine: f32,
    pub echo_spacing: f32,
    pub pedir: i32,
    pub bbr_type: String,
    pub bbr_slope: f32,
    pub verbose: i32,
    pub datatype: Option<OutType>,
    pub cost: Cost,
    pub search_cost: Cost,
    pub interp: Interp,
    pub sinc_window: SincWindow,
    pub quaternion: bool,
    /// The rotation search ranges, radians, `[min, max]` per axis.
    pub search: [[f64; 2]; 3],
    pub no_resample: bool,
    pub force_scaling: bool,
    pub two_d: bool,
    pub no_clamp: bool,
    pub no_resample_blur: bool,
    pub use_sqform: bool,
    pub print_init: bool,
    pub debug_save: bool,
    pub debug: bool,
}

impl Default for Options {
    fn default() -> Self {
        let r = std::f64::consts::FRAC_PI_2;
        Options {
            input: None,
            reference: None,
            out: None,
            omat: None,
            init: None,
            schedule: None,
            ref_weight: None,
            in_weight: None,
            wmseg: None,
            fieldmap: None,
            fieldmap_mask: None,
            wmcoords: None,
            wmnorms: None,
            bins: 256,
            dof: 12,
            sinc_width: 7.0,
            padding: 0.0,
            apply_xfm: false,
            apply_iso: None,
            base_scale: None,
            min_sampling: 1.0,
            background: None,
            coarse: (60.0f64.to_radians()) as f32,
            fine: (18.0f64.to_radians()) as f32,
            echo_spacing: 0.0005,
            pedir: 0,
            bbr_type: "signed".into(),
            bbr_slope: -0.5,
            verbose: 0,
            datatype: None,
            cost: Cost::CorrRatio,
            search_cost: Cost::CorrRatio,
            interp: Interp::Trilinear,
            sinc_window: SincWindow::Hanning,
            quaternion: false,
            search: [[-r, r]; 3],
            no_resample: false,
            force_scaling: false,
            two_d: false,
            no_clamp: false,
            no_resample_blur: false,
            use_sqform: false,
            print_init: false,
            debug_save: false,
            debug: false,
        }
    }
}
