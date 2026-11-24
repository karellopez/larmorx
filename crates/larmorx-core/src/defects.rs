// SPDX-License-Identifier: Apache-2.0
//! Upstream defects: D13, "correct by default, faithful on request" (`docs/api/upstream-defects.md`).
//!
//! When an original program (AFNI, FSL, ANTs, SciPy, fMRIPrep, ...) has a deterministic defect
//! that changes its results, larmorx corrects it by default and says so when it changed a
//! result; a compatibility switch reproduces the original bit for bit. The defects are listed
//! in `docs/findings/upstream-defects.md`, each with an ID (`FSL-01`, `PIPE-03`, ...).
//!
//! - [`UpstreamDefects`] is the switch: [`UpstreamDefects::Fix`] (the default) or
//!   [`UpstreamDefects::Reproduce`]. Every affected tool takes it in its options; on the
//!   command line it comes from the environment variable [`ENV`]
//!   (`LARMORX_UPSTREAM_DEFECTS=fix|reproduce`), so that no flag can clash with the original's
//!   arguments.
//! - [`Defect`] is a register entry; [`DEFECTS`] lists every entry larmorx has a switch for.
//! - [`Notice`] says that a defect changed this result (when fixed) or would have (when
//!   reproduced). Tools return notices; the command lines print [`Notice::line`] on standard
//!   error and the Python wrappers raise `larmorx.UpstreamDefectWarning`.
//!
//! The descriptions of the AFNI, FSL and FreeSurfer entries describe observable behaviour only, as the
//! register's summary does, so that the clean-room implementations may use them.

use std::fmt;

/// The environment variable that sets the [`UpstreamDefects`] mode of the command line.
pub const ENV: &str = "LARMORX_UPSTREAM_DEFECTS";

/// D13's compatibility switch: correct an upstream defect (the default) or reproduce it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UpstreamDefects {
    /// larmorx's corrected behaviour (the default).
    #[default]
    Fix,
    /// The original program's behaviour, bit for bit (the parity suites use it).
    Reproduce,
}

impl UpstreamDefects {
    /// `fix` or `reproduce`.
    pub fn parse(name: &str) -> Option<UpstreamDefects> {
        match name {
            "fix" => Some(UpstreamDefects::Fix),
            "reproduce" => Some(UpstreamDefects::Reproduce),
            _ => None,
        }
    }

    /// The name [`UpstreamDefects::parse`] reads.
    pub fn name(self) -> &'static str {
        match self {
            UpstreamDefects::Fix => "fix",
            UpstreamDefects::Reproduce => "reproduce",
        }
    }

    /// Whether defects are corrected.
    pub fn fix(self) -> bool {
        self == UpstreamDefects::Fix
    }

    /// Whether defects are reproduced.
    pub fn reproduce(self) -> bool {
        self == UpstreamDefects::Reproduce
    }

    /// The mode of the command line: [`ENV`], `fix` when unset or empty.
    pub fn from_env() -> Result<UpstreamDefects, BadMode> {
        Self::from_value(std::env::var_os(ENV).as_deref())
    }

    /// [`UpstreamDefects::from_env`] for a given value of [`ENV`].
    pub fn from_value(value: Option<&std::ffi::OsStr>) -> Result<UpstreamDefects, BadMode> {
        match value {
            None => Ok(UpstreamDefects::Fix),
            Some(v) if v.is_empty() => Ok(UpstreamDefects::Fix),
            Some(v) => v
                .to_str()
                .and_then(UpstreamDefects::parse)
                .ok_or_else(|| BadMode(v.to_string_lossy().into_owned())),
        }
    }
}

impl fmt::Display for UpstreamDefects {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// [`ENV`] holds something other than `fix` or `reproduce`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadMode(pub String);

impl fmt::Display for BadMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{ENV} must be 'fix' or 'reproduce', not '{}'", self.0)
    }
}

impl std::error::Error for BadMode {}

/// The exit code of a command line whose [`ENV`] is invalid (a usage error).
pub const EXIT_BAD_MODE: u8 = 2;

/// An entry of the defect register (`docs/findings/upstream-defects.md`).
#[derive(Debug, PartialEq, Eq)]
pub struct Defect {
    /// The register's ID (`FSL-01`).
    pub id: &'static str,
    /// The original program.
    pub tool: &'static str,
    /// What the original does, by its observable behaviour.
    pub what: &'static str,
    /// Whether [`UpstreamDefects::Fix`] corrects it. `false` for a defect whose correction is
    /// available but not yet the default (a decision pending with the user; FSL-02).
    pub fixed_by_default: bool,
}

impl Defect {
    /// This entry as the program `tool` meets it: a defect several programs share (FSL-09,
    /// FSL-11), its notices naming the program that ran. Not in [`DEFECTS`]: the same ID.
    pub const fn of(&self, tool: &'static str) -> Defect {
        Defect {
            id: self.id,
            tool,
            what: self.what,
            fixed_by_default: self.fixed_by_default,
        }
    }
}

macro_rules! defects {
    ($($name:ident = $id:literal, $tool:literal, $default:literal, $what:literal;)*) => {
        $(
            #[doc = concat!("`", $id, "` (", $tool, "): ", $what, ".")]
            pub const $name: Defect = Defect {
                id: $id,
                tool: $tool,
                what: $what,
                fixed_by_default: $default,
            };
        )*
        /// Every register entry larmorx has a switch for, in register order.
        pub const DEFECTS: &[&Defect] = &[$(&$name),*];
    };
}

defects! {
    ANTS_03 = "ANTS-03", "ANTs", true,
        "file names of fewer than 3 characters are neither read nor written (ImageMath writes only names longer than 3), and the program exits 0";
    ANTS_04 = "ANTS-04", "ImageMath", true,
        "the program exits 0 after an operation that failed and wrote nothing";
    ANTS_05 = "ANTS-05", "ImageMath", true,
        "an operand that is not exactly a number is read as a file (7 with a trailing blank), as 0 (1e, -) or as FLT_MAX (1e40), and a missing operand is the number 1";
    ANTS_06 = "ANTS-06", "ANTs", true,
        "values out of an integer pixel type's range, and NaN, are converted as x86-64 converts them: they wrap around (C++ leaves the conversion undefined)";
    ANTS_07 = "ANTS-07", "ImageMath, MultiplyImages", true,
        "a second image smaller than the first is read at the first image's indices, outside its own voxels";
    ANTS_08 = "ANTS-08", "ImageMath", true,
        "/ divides only where the divisor is positive; every other voxel gets the previous voxel's result";
    ANTS_09 = "ANTS-09", "ImageMath", true,
        "total and mean write the running sum into each voxel";
    ANTS_10 = "ANTS-10", "ImageMath", true,
        "vtotal writes an image of zeros: ANTs implements nothing for it";
    ANTS_11 = "ANTS-11", "ImageMath", true,
        "Neg finds the range with else-if, so a voxel that raises the maximum is never compared with the minimum";
    ANTS_12 = "ANTS-12", "ImageMath", true,
        "TruncateImageIntensity finds the histogram's range with else-if, so the brightest voxel can fall outside the histogram";
    ANTS_13 = "ANTS-13", "ImageMath", true,
        "Normalize finds the range from a maximum of 0 and a minimum of 1e9";
    ANTS_14 = "ANTS-14", "ImageMath", true,
        "G with a variance above about 709 voxels squared gives NaN everywhere (the Gaussian kernel's Bessel function overflows)";
    ANTS_15 = "ANTS-15", "ImageMath, SmoothImage", true,
        "a sigma list of the wrong length is not an error: G filters with variance 0 and SmoothImage with 1 mm, exit 0";
    ANTS_16 = "ANTS-16", "ImageMath", true,
        "Laplacian reads its normalize flag from the sigma's argument";
    ANTS_17 = "ANTS-17", "ImageMath", true,
        "MO with the foreground 0 sets the voxels its erosion removes to 0, the foreground itself, so it only dilates the zeros; ME makes 1 every voxel above 0.5 that its erosion keeps, other labels included, and 0 a remaining foreground of 0.5 or less";
    ANTS_19 = "ANTS-19", "ImageMath", true,
        "FillHoles with holeparam at most 1 reads neighbours outside the image";
    ANTS_20 = "ANTS-20", "ImageMath", true,
        "GetLargestComponent turns every voxel into 1 when no component is kept";
    ANTS_22 = "ANTS-22", "ImageMath", true,
        "PadImage with a fractional pad p grows each axis by the truncated 2p but shifts the image by the truncated p, so the padding (or cropping) is uneven: 2.5 adds 2 voxels before and 3 after";
    ANTS_23 = "ANTS-23", "ImageMath", true,
        "ExtractContours takes each voxel's value truncated to an unsigned integer as its label: 1.2 and 1.7 are one label with no contour between them, values below 1 are background, and contour voxels hold the truncated value";
    ANTS_24 = "ANTS-24", "ThresholdImage", true,
        "a mode other than Otsu or Kmeans is a range threshold whose lower bound is the mode's name read with atof";
    ANTS_25 = "ANTS-25", "ThresholdImage", true,
        "Otsu with a mask finds the range from FLT_MIN with else-if";
    ANTS_27 = "ANTS-27", "MultiplyImages", true,
        "a second argument that cannot be read as an image is read as a number with atof, so a missing file multiplies by 0";
    ANTS_28 = "ANTS-28", "ResampleImageBySpacing, SmoothImage", true,
        "a smoothed axis of fewer than 4 voxels makes the smoothing fail, and the program resamples the smoothing's buffer, which was never written";
    ANTS_29 = "ANTS-29", "ResampleImage", true,
        "a size of 1 gives an infinite spacing and an image of zeros";
    ANTS_30 = "ANTS-30", "ResampleImage", true,
        "the seventh argument is read both as the pixel type and as the interpolator's parameter";
    ANTS_31 = "ANTS-31", "ResampleImageBySpacing", true,
        "the nn flag is read from the addvox argument in 2D and whenever there are more than 9 arguments in 4D, and dimensions other than 2 to 4 do nothing";
    ANTS_32 = "ANTS-32", "ResampleImageBySpacing", true,
        "the smoothing sigma out/in - 1, meant in voxels, is taken in millimetres (half a voxel for 2 to 4 mm), and output voxels beyond the input get the input's value at index (1, 1, ...) instead of 0";
    ANTS_33 = "ANTS-33", "antsApplyTransforms", true,
        "an unknown -u type keeps the default type, MultiLabel ignores its alpha and GenericLabel its interpolator";
    ANTS_35 = "ANTS-35", "N4BiasFieldCorrection", true,
        "-c, -b and -t without brackets set nothing";
    ANTS_36 = "ANTS-36", "N4BiasFieldCorrection", true,
        "a mask that cannot be read becomes an image of ones, and -r does nothing without a mask";
    ANTS_38 = "ANTS-38", "N4BiasFieldCorrection", true,
        "the histogram's range is found with else-if, so a first included voxel that is the unique minimum is left out of the histogram";
    ANTS_39 = "ANTS-39", "N4BiasFieldCorrection", true,
        "256 histogram bins give a 1024-point FFT instead of 512 (a float logarithm divided by a double one)";
    ANTS_40 = "ANTS-40", "N4BiasFieldCorrection", true,
        "the voxels N4 uses whose value is 0 or negative stay in its log image as they are (0, the log of an intensity of 1), inside the histogram it sharpens and the bias field's fit: without a mask, a zero background or zero-filled voxels pull the field";
    ANTS_43 = "ANTS-43", "DenoiseImage", true,
        "the Rician correction factor overflows at high signal-to-noise ratios and the bias becomes 0";
    ANTS_44 = "ANTS-44", "DenoiseImage", true,
        "with a mask and the Rician model, voxels outside the mask that patches reach are corrected with the raw noise estimate of a centre in the mask, not with the smoothed and corrected bias every other voxel gets";
    ANTS_47 = "ANTS-47", "DenoiseImage", true,
        "a mask that cannot be read is no mask: the whole image is denoised, exit 0";
    ANTS_50 = "ANTS-50", "antsRegistration", true,
        "--save-state after linear stages only: no state, exit 1 without a message, no transform written";
    ANTS_51 = "ANTS-51", "antsRegistration", true,
        "transform files without collapsing: the initial transforms skipped by counting the -r options (not the transforms they hold, nor --restore-state's), each file named after the next stage in order: a transform written under another's name, or the names run out (a read past the end of a list, then a crash)";
    ANTS_49 = "ANTS-49", "antsRegistration", true,
        "a linear stage that throws (an ITK exception) is dropped: the next stages run without it, the outputs are written and the program exits 0";
    ANTS_52 = "ANTS-52", "antsAI", true,
        "the translation search grid is ignored for Rigid and Similarity in 2D and for Similarity in 3D: every translation gives the same start point";
    ANTS_53 = "ANTS-53", "antsAI", true,
        "AlignGeometricCenters and AlignCentersOfMass with iterations above 0 run a search that lists no start point and write no transform: an empty .mat file, exit 0 (a .txt or .h5 output crashes)";
    ANTS_54 = "ANTS-54", "antsAffineInitializer", true,
        "the local-search argument is never read: every start point runs 20 iterations of the local optimizer, whatever it says";
    ANTS_55 = "ANTS-55", "antsAI", true,
        "a translation search grid with fewer extents than dimensions is read past the end of its vector";
    ANTS_56 = "ANTS-56", "antsAffineInitializer", true,
        "a dimension other than 2, 3 or 4 runs as 2D";
    ANTS_61 = "ANTS-61", "antsRegistration", true,
        "a stage that combines IGDM with ICP, PSE or JHCT, or lists it after an image metric, registers only one kind of point set: ITK finds fewer inputs than metrics and throws (the stage is dropped), or crashes";
    ANTS_62 = "ANTS-62", "antsRegistration", true,
        "a text point set that ends with a newline gets one more point from uninitialised variables (in the oracle's build, a copy of the last point)";
    ANTS_63 = "ANTS-63", "antsRegistration", true,
        "a binary VTK point set's labels are read only as far as one point's components: the other labels are uninitialised memory";
    ANTS_65 = "ANTS-65", "antsRegistration", true,
        "IGDM's estimated intensity sigma ignores both point sets' minima (they are never updated) and subtracts the moving maximum: it is the difference of the two maxima, or the larger maximum";
    ANTS_66 = "ANTS-66", "antsRegistration", true,
        "IGDM's value (the line search's) computes the neighbourhood's centre from a negative number for neighbourhoods of more than one voxel and reads memory before the intensity arrays";
    ANTS_71 = "ANTS-71", "antsRegistration", true,
        "--print-similarity-measure-interval in a SyN stage crashes unless the registration has an initial fixed transform (-q), a moving transform before the stage and a single image metric: the full-scale similarity's composites get a null transform, or the stage's metric is cast to an image metric and read through a null pointer";
    ITK_01 = "ITK-01", "ITK", true,
        "Math::Floor gives 0 instead of -1 for negative values closer to 0 than 2^-26 (float) or 2^-55 (double)";
    ITK_02 = "ITK-02", "ITK", true,
        "a file with more dimensions than the program reads is read with an identity direction";
    ITK_18 = "ITK-18", "ITK", true,
        "the joint-histogram mutual information (ANTs' MI) swaps its marginal densities: each image's marginal is the other's";
    ITK_19 = "ITK-19", "ITK", true,
        "MultiStartOptimizerv4 counts a start point that threw with the previous one's metric value (0 for the first), so a first start point that throws stays the best unless another one's value is below 0";
    ITK_20 = "ITK-20", "ITK", true,
        "the Mattes metric refuses its joint histogram's spacing when the analysis region has one intensity or none, and is then left without the histogram's buffer: the next evaluation of the metric crashes";
    ITK_21 = "ITK-21", "ITK", true,
        "MultiStartOptimizerv4 without a scales estimator (antsAffineInitializer) starts each start point's local optimization at the learning rate the previous start point's line search left, so each start point's result depends on the one before it";
    ITK_22 = "ITK-22", "ITK", true,
        "LabeledPointSetToPointSetMetricv4 throws 'Label not found in common label set' for a fixed point whose label the moving set lacks (after warning that the label sets are not bijective)";
    ITK_23 = "ITK-23", "ITK", true,
        "GaussianMembershipFunction keeps the univariate prefactor 1/sqrt(2 pi) when its covariance is set to the identity, in any dimension (JHCT with a point-set sigma of 1)";
    ITK_25 = "ITK-25", "ITK", true,
        "ManifoldParzenWindowsPointSetFunction reads past the end of its list of neighbours when a point set has fewer points than the covariance neighbourhood";
    ITK_26 = "ITK-26", "ITK", true,
        "ManifoldParzenWindowsPointSetFunction adds each product to a local covariance's diagonal twice";
    ITK_28 = "ITK-28", "ITK", true,
        "JHCT tests alpha = 1 with != for its prefactor and with AlmostEquals for its value: an alpha within 4 ulps of 1 takes the logarithm and divides by alpha - 1";
    ANTS_73 = "ANTS-73", "ANTs", true,
        "an image given as an initial transform whose voxels are not vectors of the registration's dimension (a scalar image, a 2D field in 3D) is read as a displacement field anyway: ITK converts the pixels as gray, complex, RGB or RGBA values and ANTs registers with the result";
    ANTS_74 = "ANTS-74", "ANTs", true,
        "--save-state after linear stages that follow a SyN stage (or a restored state) saves that SyN stage's state, without the later stages, although the option's description says the state is the composite transform unless the last transform is SyN's";
    ANTS_76 = "ANTS-76", "ANTs", true,
        "collapsing the output transforms (-z 1, the default) around B-spline transforms (BSpline stages) loses transforms: one that follows only B-spline transforms, and a B-spline transform that follows another after a linear or field run (at the end of the queue; before more linear transforms or fields it joins their run and ANTs crashes)";
    ANTS_77 = "ANTS-77", "ANTs", true,
        "a BSpline stage with a mesh size of 0, or fewer mesh sizes than dimensions (read past the end of the list), divides by zero: NaN grid geometry, vnl SVD warnings and an exception on garbage indices";
    ANTS_75 = "ANTS-75", "ANTs", true,
        "--print-similarity-measure-interval and --write-interval-volumes copy the stage's transforms by setting each copy's parameters before its fixed parameters, which a displacement field refuses, and a B-spline transform whose number of parameters is not a new one's (a mesh of 1): with such a transform among the stage's (an initial transform, or a GaussianDisplacementField, BSplineDisplacementField or BSpline stage) the stage fails at its first iteration";
    ANTS_67 = "ANTS-67", "antsRegistration", true,
        "IGDM's kNeighborhood and gradientSigma are read and never used: one nearest point, central-difference gradients";
    ANTS_68 = "ANTS-68", "antsRegistration", true,
        "IGDM's derivative lacks the Gaussians' 1/sigma^2 factors, so it is not the gradient of IGDM's value";
    ANTS_70 = "ANTS-70", "antsRegistration", true,
        "IGDM's estimated distance sigma is the standard deviation of the nearest distances, 0 when they are all equal (an image registered to itself), and the parameters become NaN";
    ANTS_78 = "ANTS-78", "antsRegistration", true,
        "BSplineDisplacementField and BSplineExponential take their meshes 'at the base level' but double them already at the base level (mesh << (level + 1))";
    ANTS_79 = "ANTS-79", "antsRegistration", true,
        "TimeVaryingVelocityField's number of time indices and TimeVaryingBSplineVelocityField's mesh are read from the last -t option on the command line, not the stage's own";
    ANTS_80 = "ANTS-80", "antsRegistration", true,
        "Exponential and BSplineExponential stages take their smoothing sigmas as millimetres even when they are given in voxels (the stage never passes the sigmas' units to its registration method, whose default is millimetres)";
    ANTS_81 = "ANTS-81", "antsRegistration", true,
        "without collapsing, an Exponential stage given the short name Exp writes no InverseWarp file: the list of transforms with an inverse has only the long name";
    ITK_24 = "ITK-24", "ITK", true,
        "JHCT's total number of points leaves out the fixed points (counted only after it is used): the prefactors use the moving points alone";
    ITK_29 = "ITK-29", "ITK", true,
        "ManifoldParzenWindowsPointSetFunction uses its kernel sigma as a variance (identity times sigma), where the regularisation sigma is squared";
    ITK_35 = "ITK-35", "ITK", true,
        "BSplineTransform has no Jacobian with respect to position: a stage after a BSpline stage, or with a B-spline initial transform, fails at its first derivative unless its metric works on identity fields";
    ITK_36 = "ITK-36", "ITK", true,
        "TimeVaryingBSplineVelocityFieldImageRegistrationMethod sets each metric's transforms on the whole multi-metric, so every metric is evaluated through the transforms the last metric set: point-set metrics listed before an image metric see identity fields (never the deformation), image metrics after a point-set metric are warped twice (their resampled images through the flows again)";
    ITK_37 = "ITK-37", "ITK", true,
        "TimeVaryingBSplineVelocityFieldImageRegistrationMethod zeroes the flow's displacement field at the first time sample (and at the last) but keeps the previous integration's inverse field, which the point-set metrics use to map their points";
    ITK_38 = "ITK-38", "ITK", true,
        "TimeVaryingBSplineVelocityFieldImageRegistrationMethod with a single point-set metric fits the points (and the domain's boundary points) at their physical positions in the B-spline's parametric domain, which leaves out the domain's direction: with any direction but the identity (an image stored in RAS order, as NIfTI files are) the points fall outside it and the fit throws";
    ITK_39 = "ITK-39", "ITK", true,
        "TimeVaryingVelocityFieldImageRegistrationMethodv4 resamples the fixed images through the flow alone: the fixed initial transforms (antsRegistration's -q) are ignored";
    ITK_40 = "ITK-40", "ITK", true,
        "the time-varying velocity field registration methods resample the fixed and moving images through the flows but give the metric the stage's masks unchanged (with identity transforms), so the masks no longer cover the images they belong to";
    ITK_41 = "ITK-41", "ITK", true,
        "a velocity field transform that never integrated its velocity field (a stage of no iteration and no resampling) is incomplete: a time-varying one's displacement field has no interpolator input (the next point it maps crashes), an exponential one keeps its velocity image as its field and has no inverse field (its inverse throws 'No displacement field is specified.')";
    ITK_42 = "ITK-42", "ITK", true,
        "the scales estimator probes a transform of local support with a unit change of the central voxel's parameters; where a B-spline-smoothed update field pins every control point to its stationary boundary (a coarse mesh at a shrunken level), the probe vanishes and the scales, all below epsilon (equal but for rounding), make the optimizer throw 'm_Scales values must be > epsilon', although equal scales cancel in the estimated learning rate";
    ITK_32 = "ITK-32", "ITK", true,
        "the labelled point-set metric (ANTs' ICP, PSE, JHCT) appends new per-label metrics at every Initialize and keeps using the first ones: in SyN, which initialises the metric twice per iteration, every evaluation compares with the moving points as the stage's first evaluation mapped them (in the moving-to-middle update, with the moving points themselves)";
    ITK_33 = "ITK-33", "ITK", true,
        "SyN with several metrics gives every metric identity displacement field transforms after setting the point-set metrics' transforms, so a point-set metric among several metrics never sees the initial transforms or the deformation: its points are compared where they were read";
    ITK_31 = "ITK-31", "ITK", true,
        "SyN sets the metric's resampled masks only for the roles that have one: with a fixed mask and no moving mask (or the reverse), the role without one keeps the mask of the previous evaluation, which from the second iteration of a level is the previous iteration's resampled mask";
    ITK_30 = "ITK-30", "ITK", true,
        "the global correlation metric (ANTs' GC) with a displacement field transform (SyN) adds every point's derivative into the first voxel's entries, so only that voxel's update is not zero (the check meant to refuse such transforms is in the constructor and never fires)";
    ITK_03 = "ITK-03", "ITK", true,
        "components relabelled in a float image stop getting new labels at 2^24";
    ITK_08 = "ITK-08", "nifti_clib", true,
        "asked for x.nii.gz, the reader takes the header from x.nii.gz but the voxels from x.nii when that file exists";
    ITK_10 = "ITK-10", "ITK", true,
        "a versor whose axis has a norm of at least 1 can fail to load in single precision";
    ITK_12 = "ITK-12", "ITK", true,
        "an HDF5 transform file whose TransformType is a fixed-length string (as h5py writes numpy bytes) cannot be read: ITK reads only variable-length strings";
    ITK_14 = "ITK-14", "ITK", true,
        "HDF5 transform parameters stored in a floating-point type wider than 8 bytes (long double, quadruple precision) are read through float, losing their precision";
    ITK_15 = "ITK-15", "ITK", true,
        "a transform without parameters (IdentityTransform) cannot be written to HDF5 with compression, which ANTs always asks for: the zero chunk size fails (H5Pset_chunk failed), leaving a file without that transform's parameters that ITK cannot read";
    HDF_01 = "HDF-01", "HDF5", true,
        "converting floating-point data of a non-native format (half, quadruple, VAX, custom), HDF5 1.12 keeps state from one element to the next: after a denormalized value every later value of the same conversion buffer rounds as a denormal, an unnormalized zero takes the previous value's mantissa width, and a denormal that rounds up to the smallest normal loses the carry";
    PIPE_03 = "PIPE-03", "fMRIPrep ResampleSeries", true,
        "the Jacobian of the field map is taken along the target grid's axis with the source's phase-encoding index, in target voxels";
    PIPE_04 = "PIPE-04", "nitransforms", true,
        "the transforms of an ITK composite .h5 file are read in name order, so a composite of 10 or more transforms is misordered";
    PIPE_17 = "PIPE-17", "nitransforms", true,
        "the first group of an ITK .h5 file is skipped as if it were a composite: a file of one transform cannot be loaded (IndexError), a file of several transforms without a composite loses its first, and an empty composite fails";
    PIPE_18 = "PIPE-18", "niworkflows IntraModalMerge", true,
        "grand_mean_scaling indexes the series with a mask of the wrong shape, so every series of two or more volumes fails with numpy's IndexError";
    PIPE_19 = "PIPE-19", "niworkflows IntraModalMerge", true,
        "a 6- or 7-dimensional image passes the check for more than four dimensions and is averaged over its fourth axis";
    PIPE_20 = "PIPE-20", "fMRIPrep DistortionParameters", true,
        "a missing readout time or phase-encoding direction becomes undefined outputs, so the BOLD run is resampled without its field map although one is assigned to it (and the report still names the correction)";
    PIPE_21 = "PIPE-21", "fMRIPrep ReconstructFieldmap", true,
        "the field evaluated directly on a target is returned on the target projected into the field map's space and made positive (its voxels reversed along an axis that runs against its world axis), while the resampler reads it voxel for voxel on the target's own grid: a mirrored field";
    PIPE_23 = "PIPE-23", "nipype is_outlier", true,
        "a median absolute deviation of 0 (more than half of the values equal) makes the values at the median 0/0, NaN, which fail the threshold test and count as outliers, so NonsteadyStatesDetector calls every leading volume non-steady";
    PIPE_24 = "PIPE-24", "niworkflows ValidateImage", true,
        "the report names the translation's unit with nibabel's get_xyzt_units, which raises KeyError for a spatial unit code 4 to 7";
    PIPE_25 = "PIPE-25", "fMRIPrep", true,
        "a BOLD run whose B0FieldSource names an estimator by another of its sources' B0FieldIdentifier values (aliases) gets no field map, or not, as a Python set orders them";
    PIPE_26 = "PIPE-26", "niworkflows DerivativesDataSink", true,
        "with two or more list-valued entity inputs the files are matched to the names in the order of a Python set of the entity names, which changes between processes";
    PIPE_27 = "PIPE-27", "niworkflows DerivativesDataSink", true,
        "its warning about a changed data type names the original type twice";
    PIPE_28 = "PIPE-28", "niworkflows DerivativesDataSink", true,
        "compress with fewer values than files (but more than one) fails with Python's IndexError";
    PIPE_29 = "PIPE-29", "niworkflows DerivativesDataSink", true,
        "the data type coercion casts values out of the integer type's range and NaN with numpy's cast (undefined in C; on x86-64 a wrapped or arbitrary value)";
    PIPE_30 = "PIPE-30", "niworkflows DerivativesDataSink", true,
        "a RepetitionTime input for an image that is not 4-D makes the header fix set four voxel sizes on it, and nibabel's HeaderDataError stops the sink";
    PIPE_31 = "PIPE-31", "fMRIPrep", true,
        "a precomputed field map whose derivatives lack its field map, reference or coefficients file stops the graph's construction with a bare KeyError";
    PIPE_32 = "PIPE-32", "fMRIPrep", true,
        "every participant's field-map estimators go into sdcflows' one registry, so a second participant with a B0FieldIdentifier an earlier one used (BIDS asks it to be unique only within a participant) stops the run with KeyError";
    PIPE_33 = "PIPE-33", "niworkflows IntensityClip", true,
        "an image constant between its percentiles is divided by its maximum, 0: a NaN image, converted to int16 or uint8 by an undefined conversion (0 on x86-64)";
    PIPE_34 = "PIPE-34", "niworkflows RobustAverage", true,
        "a gathered volume whose mean is 0 after the clipping is divided by it: NaN or infinite voxels, registered and averaged";
    PIPE_37 = "PIPE-37", "pybids BIDSLayout", true,
        "the index matches the entity patterns against each file's absolute path, so the folders above the dataset give its files entities: below a folder named func, sub-pilot or run-2 every file gets datatype func, subject pilot or run 2, and with the derivatives configuration (whose den-, res-, desc-, ... patterns have no separator before them) below golden-x or andres-lab every file gets den or res";
    PIPE_40 = "PIPE-40", "niworkflows IntraModalMerge", true,
        "with to_ras the series is reoriented to RAS but in_mask is not, so grand_mean_scaling indexes the RAS series with the mask on the input's grid: numpy's IndexError when the shapes differ, a mirrored or permuted mask when they agree";
    PIPE_41 = "PIPE-41", "fMRIPrep init_bold_confs_wf", true,
        "merge_confound_metadata's third input is set to the tissue signals' metadata (Method: Mean for global_signal, csf, white_matter, csf_wm and tcompcor) and then connected to the crown's metadata, which replaces it: the confounds' JSON never describes those five columns";
    PIPE_43 = "PIPE-43", "niworkflows parse_formula", true,
        "the variables of each group of a model formula, and the columns others stands for, are ordered as a Python set of strings, whose order changes between processes (the per-process hash seed), so ExpandModel writes the expansion's columns of a variable in another order from one run to the next (x_power2 before or after x_derivative1_power2); never reproduced";
    PIPE_44 = "PIPE-44", "nipype compute_noise_components", true,
        "a mask that gives no component (an empty mask with components_criterion 'all') ends the loop over the masks (break): the masks after it are skipped, their components and metadata lost, and when no component was found failure_mode='NaN' stops with ValueError: need at least one array to concatenate instead of returning";
    PIPE_45 = "PIPE-45", "nipype compute_noise_components", true,
        "only the voxels whose float sum over time is NaN are zeroed: a voxel holding an infinity (and no NaN) reaches the pre-filter and the SVD, which then fails, so every component of the mask is NaN with failure_mode='NaN' (or ValueError: array must not contain infs or NaNs), while a finite voxel whose float32 sum overflows both ways is zeroed";
    PIPE_46 = "PIPE-46", "nipype compute_dvars", true,
        "the voxel-wise standardisation, which runs whatever DVARS the caller asked for, turns numpy's floating-point warnings into errors: a voxel whose predicted difference SD is 0 (a robust SD of 0, kept by remove_zerovariance=False, or an AR(1) estimate of exactly 1) stops the whole computation with RuntimeWarning: divide by zero (or invalid value) encountered in divide, and every DVARS series is lost";
    PIPE_47 = "PIPE-47", "nipype compute_dvars", true,
        "an AR(1) estimate above 1, which the float32 FFT's rounding gives for a series whose lag-1 autocorrelation is just below 1, makes the predicted difference SD the square root of a negative number: NaN, so every standardised and voxel-wise DVARS value is NaN, without an error";
    PIPE_50 = "PIPE-50", "nipype compute_dvars", true,
        "a voxel whose demeaned series is all zeros (a constant voxel, kept by remove_zerovariance=False) has the AR(1) Yule-Walker equation 0 a = 0: with SciPy 1.15 nitime's solve returns NaN, which makes every standardised and voxel-wise DVARS value NaN without an error; with SciPy 1.18 it stops with LinAlgError (\"A singular matrix detected.\")";
    PIPE_51 = "PIPE-51", "nipype TSNR", true,
        "the maps of a series stored as integers (tSNR, mean, standard deviation, the detrended series) are written in that integer type, quantized by nibabel's scaling (256 levels for uint8, 65,536 for int16): the check meant to switch the header to float32 for integer data never fires, as the data are float32 by then";
    PIPE_52 = "PIPE-52", "nipype regress_poly", true,
        "the non-constant regressors it returns (CompCor's Legendre pre-filter file) are X[:, :-1], the constant and every polynomial but the highest, not the polynomials of degree 1 to d";
    PIPE_53 = "PIPE-53", "nipype regress_poly, cosine_filter", true,
        "an axis other than the last takes that axis' length as the series' length but reshapes the data in memory order to rows of that length, so the series regressed are runs of consecutive values across voxels and volumes, without an error";
    PIPE_54 = "PIPE-54", "nipype compute_noise_components", true,
        "a components_criterion of 0 ends the loop over the masks at the first one, and a negative one other than -1 ('all') slices columns off the end of each mask's components (u[:, :-2]) while marking none retained in the metadata; a count given as a float (6.0) stops with TypeError when an SVD fails with failure_mode='NaN'";
    PIPE_55 = "PIPE-55", "nipype compute_noise_components", true,
        "an empty mask with a failure_mode other than 'error' returns an empty pre-filter basis, so when it is the last mask the cosine or polynomial basis is lost (CompCor's pre-filter file then holds no basis column)";
    PIPE_56 = "PIPE-56", "nipype CompCor", true,
        "the repetition time read from the header is converted from milliseconds but not from microseconds (xyzt_units usec): it is taken as that many seconds, which gives a cosine basis of millions of columns";
    PIPE_57 = "PIPE-57", "nipype compute_noise_components", true,
        "a filter_type other than 'cosine', 'polynomial' and False silently filters nothing, not even the mean, and a float32 series is then decomposed in single precision";
    PIPE_58 = "PIPE-58", "nipype compute_noise_components", true,
        "mask_names shorter than the masks silently drops the masks without a name (zip): their components and metadata are not computed";
    PIPE_59 = "PIPE-59", "nipype compute_noise_components", true,
        "an SVD that fails with failure_mode='NaN' is replaced by T NaN singular values whatever the decomposition's size min(T, N), and with components_criterion 'all' by one NaN component while all T values are marked retained (CompCor's metadata then names every one after the first component)";
    PIPE_60 = "PIPE-60", "nipy rotation_vec2mat", true,
        "the angle of a rotation vector given as integers is the square root of its sum of squares computed in the vector's own type, which wraps around silently (a component of an int8 vector from 12, int16 from 182, int32 from 46,341; an int64 vector when the sum passes 2^63, from about 1.75e9 per component), so nipype's normalize_mc_params(params, 'NIPY') of such a row is another rotation or numpy's LinAlgError";
    PIPE_61 = "PIPE-61", "niworkflows AddTSVHeader", true,
        "numpy's loadtxt squeezes a table of one row to a vector, which savetxt writes as one column (the row's values one under the other), and a table of one value to a scalar, which savetxt refuses (ValueError: Expected 1D or 2D array, got 0D array instead)";
    PIPE_62 = "PIPE-62", "niworkflows AddTSVHeader", true,
        "a header with another number of names than the table's columns is written as given, so the file's header and rows disagree";
    PIPE_63 = "PIPE-63", "fMRIPrep GatherConfounds", true,
        "a shorter table is aligned with the last rows (its missing rows first), but pandas' concat joins the row labels without sorting them: when an earlier table is the shorter one (no signals table, a DVARS table first), the first volume's row is written last and every other row one volume early";
    PIPE_64 = "PIPE-64", "niworkflows parse_formula", true,
        "the shorthands (wm, gsr, rps, fd, acc, tcc, dv, dvall, nss, spikes, others) are replaced as text, anywhere and again inside every group: a name that holds one is rewritten (std_dvars becomes std_std_dvarsars), dvall becomes std_dvarsall (dv is replaced first), a shorthand that matches no column leaves an empty term (IndexError: string index out of range), and the names others leaves out lose every d<digits> and ^<digits> in them";
    PIPE_65 = "PIPE-65", "niworkflows parse_formula", true,
        "a term's operators are read with regular expressions over the whole term: text around a group other than a derivative before it and an exponent after it is ignored (foo(a), (a)b and d1x(a) give a's columns), an exponent after a group that holds another single-caret exponent fails with TypeError (((a)^2)^3), and a name that starts like a derivative or ends like an exponent (d1a, a^2) is taken for an operator without its group (TypeError)";
    PIPE_66 = "PIPE-66", "niworkflows parse_formula", true,
        "a variable reached by two terms of the formula (a + (a)^^2) is repeated: twice in parse_formula's table and four times in ExpandModel's file (pandas selects every column of a repeated name each time it is named), and a group that repeats one fails with pandas' ValueError (Shape of passed values ...)";
    PIPE_67 = "PIPE-67", "niworkflows spike_regressors", true,
        "each lag is added to the volumes the earlier lags flagged, so lags [1, 2] (or [0, 1, 2]) flag the third volume after a spike too";
    PIPE_68 = "PIPE-68", "niworkflows spike_regressors", true,
        "minimum_contiguous counts the last run of unflagged volumes one volume longer than it is (its end is taken one past the table), so a final run one volume shorter than the minimum is not flagged";
    PIPE_69 = "PIPE-69", "niworkflows spike_regressors", true,
        "a criterion other than '<' and '>' is skipped without an error (when every criterion is: TypeError: reduce() of empty iterable with no initial value)";
    PIPE_70 = "PIPE-70", "niworkflows NormalizeMotionParams", true,
        "numpy's loadtxt squeezes a table of one row to a vector, which numpy's apply_along_axis refuses (AxisError: axis 1 is out of bounds for array of dimension 1)";
    PIPE_71 = "PIPE-71", "nitransforms load", true,
        "a transform file of one transform is loaded as one Affine, whose 4 x 4 matrix fMRIPrep's FSLMotionParams and FSLRMSDeviation then take as a series of four (its rows): a one-volume series stops with numpy's AxisError (axis2: axis 1 is out of bounds for array of dimension 1)";
    PIPE_72 = "PIPE-72", "fMRIPrep FSLMotionParams", true,
        "a reference image whose centre of gravity is undefined (a constant image under 25.2.7, whose weights value - min are all zero; a sum of zero under 25.2.6; a NaN or infinite voxel) gives NaN or infinite translations in every row without an error, which FramewiseDisplacement turns into n/a everywhere";
    PIPE_73 = "PIPE-73", "nitransforms ITKLinearTransform", true,
        "every ITK text transform is read in single precision (numpy's genfromtxt with dtype f4), the double-precision ones (AffineTransform_double_3_3) too, so their parameters are rounded to float32";
    PIPE_74 = "PIPE-74", "fMRIPrep _binary_union", true,
        "the masks are cast to int64 with numpy's cast, undefined in C for NaN, infinities and values beyond int64's range: on x86-64 they become the most negative int64, so a voxel of +inf or 1e19 (or a uint64 value from 2**63) is outside the union";
    PIPE_75 = "PIPE-75", "fMRIPrep _binary_union", true,
        "masks of different shapes are broadcast without an error (a mask of shape (X, Y, 1) is taken for every slice of an (X, Y, Z) one)";
    PIPE_76 = "PIPE-76", "fMRIPrep acompcor_masks", true,
        "with is_aseg the grey-matter map is cast to uint8 with numpy's cast before the test > 0: integers wrap (-1 is 255, inside; 256 is 0, outside) and floats out of int32's range or NaN are undefined in C (on x86-64 0, outside; a float in range keeps its truncated int32's low byte: 300.0 is 44)";
    PIPE_77 = "PIPE-77", "niworkflows BinaryDilation", true,
        "a negative radius (traits.Int takes it) dilates with scikit-image's ball of that radius, an arbitrary footprint: -1 is one false element, so the dilated mask is empty; -2 the cross of radius 1";
    PIPE_78 = "PIPE-78", "niworkflows SignalExtraction", true,
        "a class label holding a tab or a line break is written as it is, which breaks the table's columns or rows";
    PIPE_80 = "PIPE-80", "nipype command lines", true,
        "some file names are written into the command line unquoted (the files inside ANTs' bracketed arguments: ApplyTransforms' --transform [ f, 1 ] and --output, Registration's --metric, --masks, --initial-moving-transform and --output, N4BiasFieldCorrection's and DenoiseImage's [ out, extra ]; ApplyTransforms' --transform f, MultiImageMaths' operands, TOPUP's --datain file), and the line runs through the shell, so a file name with a blank is split into several words (one with a quote breaks the line); BET's input name is made relative after it is quoted, so a name that needs quotes is taken relative to the process's folder: the program gets other file names and fails";
    PIPE_81 = "PIPE-81", "nipype command line", true,
        "nipypecli run gives an input of a compound trait (traits.Either) the argparse type of the input before it in name order, a variable left from the loop's previous iteration (CompCor's num_components reads text, so a number of components is refused; save_pre_filter and save_metadata read a float, so a file name or True is refused), and cannot build the parser of an interface whose first input is compound (UnboundLocalError: niworkflows' TSV2JSON) or that has a list of compound items (AttributeError: niworkflows' DictMerge, fMRIPrep's FMRISummary)";
    PIPE_82 = "PIPE-82", "nipype command line", true,
        "nipypecli run reads an input's text by the type of its default value, not by its trait: a float input whose default is an integer reads only integers (CompCor's high_pass_cutoff, FramewiseDisplacement's radius), and the texts of a tuple (aCompCorMasks' bold_zooms, RegridToZooms' zooms, the plots' figsize), an optional dictionary or an enumeration's values that are not text (CompCor's pre_filter False, GenerateSamplingReference's xform_code) reach the trait unconverted and are refused whatever is given; a list of booleans reads every text but the empty one as true";
    PD_01 = "PD-01", "pandas read_csv", true,
        "the default float parser (precise_xstrtod) keeps at most 17 digit characters, the leading zeros of 0.00... included, accumulates them in a double and divides by one power of ten, so most values to_csv writes (their shortest repr, up to 17 significant digits) are read back as another double, one or more units in the last place off";
    PD_02 = "PD-02", "pandas to_json", true,
        "the 10-decimal formatting of doubles (ujson) multiplies the fraction by 1e10 in floating point, truncates it and rounds the remainder, ties of an even last digit down and of a last digit 0 up, so some values are written with another 10th decimal than the correctly rounded one";
    PD_03 = "PD-03", "pandas read_csv", true,
        "an integer column with missing values marks them with int64's minimum before turning them into NaN, so a value equal to -9223372036854775808 in it is read as NaN";
    SCI_02 = "SCI-02", "SciPy ndimage", true,
        "NaN and out-of-range values written to an integer output are converted as x86-64 converts them (C leaves the conversion undefined)";
    SCI_03 = "SCI-03", "SciPy ndimage", true,
        "the spline prefilter's reflect boundary (modes reflect, grid-mirror and nearest) reads a coefficient it has already updated, so the splines do not interpolate on short axes";
    SCI_09 = "SCI-09", "SciPy ndimage", true,
        "on a 1-D input the rank, median and percentile filters take the footprint's whole window, its false elements included (the rank counted from the true ones), so a footprint with holes filters another neighbourhood";
    SCI_10 = "SCI-10", "SciPy ndimage", true,
        "a binary erosion, dilation or propagation of more than one pass reads the mask's truth in the first pass and the mask cast to int8 in the later ones (0.5 and 256 are false there)";
    SCI_13 = "SCI-13", "SciPy watershed_ift", true,
        "neighbours are found by adding the structure's offsets to the C-order index, checked only against the array's size, so the last element of a row neighbours the first of the next and an axis of length 1 has neighbours along it";
    SCI_14 = "SCI-14", "SciPy watershed_ift", true,
        "an element whose cost drops is taken out of its queue only when it has a neighbour there, so an element alone in its queue stays: queues join, elements are cut off every queue (never processed) or processed at the wrong level";
    SCI_15 = "SCI-15", "SciPy watershed_ift", true,
        "markers and labels are read as 32-bit C ints: an int64 marker 2**32 + 5 is 5, 2**31 a background marker";
    SCI_16 = "SCI-16", "SciPy watershed_ift", true,
        "the byte offsets are 32-bit C ints: above 2 GiB of input or output it reads and writes outside its arrays";
    SCI_18 = "SCI-18", "SciPy linalg.inv", true,
        "SciPy 1.18's inverse of a general matrix whose norm overflows (its largest row sum is infinite) returns the LU factors: the condition estimate (LAPACK gecon) stops with info -5 and the inverse (getri) is skipped without an error";
    SCI_21 = "SCI-21", "SciPy ndimage", true,
        "the 1-D rank, median and percentile filters of a float32, float64 or int64 input write the input type's values into an output of another type or byte order (its bytes reinterpreted), past the output's end when its items are smaller (the heap corrupted: a crash or a hang)";
    SCI_22 = "SCI-22", "SciPy ndimage", true,
        "the 1-D rank, median and percentile filters refuse a big-endian float32 or float64 input (RuntimeError), numpy's longlong and, where it has 8 bytes (Windows, macOS on Apple silicon), numpy's long double (TypeError), which the n-dimensional filters take";
    SCI_23 = "SCI-23", "SciPy ndimage", true,
        "the 1-D rank, median and percentile filters into a non-contiguous output of the input's type never resolve numpy's write-back copy: numpy warns (RuntimeWarning) when the copy is freed";
    SCI_25 = "SCI-25", "SciPy ndimage", true,
        "a binary erosion of more than one pass (the propagation strategy: the structure's centre true, no brute_force) with a mask into a float32 or float64 output array erodes elements outside the mask in its later passes, as it marks them by the first byte of each output element, which is 0 for both 0.0 and 1.0 (binary_closing too)";
    SCI_26 = "SCI-26", "SciPy ndimage", true,
        "a binary erosion or dilation of more than one pass (the propagation strategy; binary_propagation and binary_fill_holes too) into a byte-swapped or misaligned output array never resolves numpy's write-back copy of it: numpy warns (RuntimeWarning) when the copy is freed";
    SCI_27 = "SCI-27", "SciPy ndimage", true,
        "label refuses a big-endian input or output (Cython's ValueError: Big-endian buffer not supported on little-endian compiler), which every other function of ndimage takes";
    SCI_28 = "SCI-28", "SciPy ndimage", true,
        "label writes the labels into a read-only output array";
    SCI_29 = "SCI-29", "SciPy ndimage", true,
        "label into a bool output writes the labels as their bytes (2, 3, ... up to 255): an invalid bool array, where labels that do not fit any other type are refused (RuntimeError)";
    SCI_30 = "SCI-30", "SciPy ndimage", true,
        "sum_labels (sum, center_of_mass) with a sequence index fails on a 0-d index array (TypeError: numpy.int64 does not support item assignment) and on an empty input (ValueError of labels.min(), or IndexError for uint64 labels)";
    SCI_31 = "SCI-31", "SciPy ndimage", true,
        "a binary erosion or dilation of more than one pass (the propagation strategy: the structure's centre true, no brute_force; binary_opening, binary_closing and binary_propagation too) with an int8 mask array uses the caller's mask as its scratch, as SciPy's cast to int8 does not copy it: the mask is left -1 where it was true and the first pass's output where it was false, and binary_closing's erosion then reads that overwritten mask, so its result differs";
    SCI_41 = "SCI-41", "SciPy linalg.solve", true,
        "SciPy 1.15 solves a diagonal system (every 1 x 1 system) by dividing, so a singular one gives inf or NaN with RuntimeWarnings only (its general path raises LinAlgError: Matrix is singular.), and checks only the matrix for infinities and NaNs, not the right-hand side; SciPy 1.18 raises LinAlgError: A singular matrix detected. and checks both";
    NP_03 = "NP-03", "numpy", true,
        "numpy's float32 log on x86-64 CPUs with AVX2 and FMA3 or AVX-512 is numpy's own kernel, which is not correctly rounded (10,947,841 of the 201,326,593 floats in [1, 2^24] get a result up to 3 units in the last place from the exact value; 78.4 million of all positive floats, up to 4), and the C library's logf on other CPUs: tedana's t2smap takes it for its log-linear T2* and S0 fit (the logarithm of float32 data plus 1), so its maps' last bits, and the curve fit started from them, depend on the CPU";
    SDC_01 = "SDC-01", "sdcflows", true,
        "grid_bspline_weights gives a sampling position beyond the first or the last control point (within distance 2 of it) the end interval's polynomial continued (SciPy's BSpline extrapolation) instead of the cubic B-spline's value: at 1.5 control spacings beyond the grid the weight is -3.27 instead of 0.0208, so a field evaluated there (B0FieldTransform.fit's grid of an oblique target, a target larger than the field map) is wrong by up to the field's own size";
    SDC_02 = "SDC-02", "sdcflows", true,
        "grid_bspline_weights on a sampling grid rotated relative to the control grid warns that the grids are not aligned and still multiplies per-axis weights, each taken from one index of the voxel, which is not the B-spline field at the voxel (measured on a real field rotated 0.5 degrees: 2.3 Hz RMS, 40 Hz at most; at 5 degrees: 20 Hz RMS, 345 Hz); B0FieldTransform.fit meets it on the projected grid of any rotating registration (fMRIPrep's registrations of the oracle runs, about 1 degree: 1.5 to 4.6 Hz RMS, up to 49 Hz, in the brain)";
    SDC_04 = "SDC-04", "sdcflows", true,
        "au2rads (PhaseMap2rads) rescales a constant phase map to NaN everywhere (0 / 0) without an error";
    SDC_05 = "SDC-05", "sdcflows", true,
        "B0FieldTransform.apply accepts a readout time only as a Python float or a list: an integer or a numpy float32 readout time fails with a TypeError";
    SDC_06 = "SDC-06", "sdcflows", true,
        "bspline_grid (and BSplineApprox) divide the affine by every header zoom, so a field map stored as a 4D image with one volume (as fMRIPrep writes its desc-preproc_fieldmap) fails with numpy's broadcasting ValueError";
    SDC_07 = "SDC-07", "sdcflows", true,
        "TOPUPCoeffReorient negates TOPUP's field coefficients (a field in Hz) when the phase-encoding direction, reoriented to RAS, is written with BIDS' letter i, which inverts the field (measured r = -0.999 with the true field; FSL's letter x, which fMRIPrep passes, is not negated)";
    SDC_08 = "SDC-08", "sdcflows", true,
        "B0FieldTransform.apply (and ApplyCoeffsField) fails with an AttributeError on a series read from a file whose axes already run to R, A and S (no reorientation): numpy's rollaxis is given nibabel's ArrayProxy, which has no transpose";
    SDC_09 = "SDC-09", "sdcflows", true,
        "SubtractPhases fails with Python's TypeError of a comparison with None when a phase map's metadata have no EchoTime";
    SDC_10 = "SDC-10", "sdcflows", true,
        "B0FieldTransform.fit with approx (the default of ApplyCoeffsField and of sdcflows' unwarping workflow), on a target not aligned with the coefficients, builds its intermediate grid around the target as given and resamples it at the target's own coordinates, so the data-to-field-map transform is ignored: the field is taken where the target lies before the registration (measured on ds002603's BOLD references, registrations of 2.5 to 3.7 mm: 7.7 to 8.5 Hz RMS, up to 87 Hz in the brain)";
    SDC_11 = "SDC-11", "sdcflows", true,
        "resample_to_zooms (BSplineApprox's resize to zooms_min) onto a grid of a single voxel fails with nitransforms' bare AssertionError (assert targets.ndim == 2 after numpy's squeeze of the one target point), so a field map whose whole extent is below zooms_min cannot be fitted";
    TED_01 = "TED-01", "tedana", true,
        "t2smap's adaptive mask (dropout) takes the 33rd percentile of the first echo's non-zero temporal means, which is NaN when one of them is NaN (a NaN sample, or no volume left), so no voxel equals it and t2smap fails with ValueError: attempt to get argmax of an empty sequence";
    TED_02 = "TED-02", "tedana", true,
        "t2smap --exclude accepts an index equal to the number of volumes (its check is greater than, not greater than or equal), then fails with numpy's IndexError";
    TED_03 = "TED-03", "tedana", true,
        "t2smap without --mask computes one with nilearn's compute_epi_mask; when that finds no voxel (nilearn's MaskWarning: Computed an empty mask) t2smap goes on and fails in the adaptive mask with IndexError: index -1 is out of bounds for axis 0 with size 0, and an image with too few voxels for its threshold fails with ValueError: attempt to get argmax of an empty sequence";
    TED_04 = "TED-04", "tedana", true,
        "t2smap applies a user mask by voxel index without comparing its affine with the data's, so a mask of the same shape in another space is used silently";
    TED_05 = "TED-05", "tedana", true,
        "t2smap caps only the limited T2* map (values above ten times its 99.5th percentile set to it); the full map, from which the optimal combination is weighted and which fMRIPrep keeps as its T2* map, holds the fit's largest values (up to 239,794 s on fMRIPrep's ds003592 call, in 2 to 34 voxels of its four multi-echo calls), where tedana 25.0.1's t2smap capped the full map too";
    TED_07 = "TED-07", "tedana", true,
        "t2smap --fittype curvefit --verbose writes the variance of S0 as the T2* variance map and the variance of T2* as the S0 variance map (curve_fit's covariance diagonal unpacked in the wrong order)";
    TED_08 = "TED-08", "tedana", true,
        "t2smap --fittype curvefit fits a voxel with one good echo with two echoes and keeps its estimate in the full maps, but a failure of that fit is not marked in the failures map, nor are its variances in the --verbose maps";
    TED_09 = "TED-09", "tedana", true,
        "t2smap's log-linear fit (also the start of curvefit) solves every voxel with the same number of good echoes as one least-squares problem, so one infinite sample (an infinite value, or an integer scaled beyond float32) makes the T2* and S0 of all those voxels NaN";
    TED_10 = "TED-10", "tedana", true,
        "t2smap accepts an infinite echo time (-e inf ...: all values at least 1, so taken as milliseconds) and fails in numpy's least squares with LinAlgError: SVD did not converge in Linear Least Squares";
    TED_11 = "TED-11", "tedana", true,
        "t2smap --n-independent-echos 0 fails with Python's ZeroDivisionError";
    TED_12 = "TED-12", "tedana", true,
        "t2smap accepts a single echo (one echo time) and fails later with an unrelated numpy error: IndexError: invalid index to scalar variable (dropout) or a ValueError of numpy's column_stack (log-linear fit)";
    TED_13 = "TED-13", "tedana", true,
        "t2smap --dummy-scans accepts a count that removes every volume (then fails with ValueError: attempt to get argmax of an empty sequence after numpy's 'Mean of empty slice' warnings) and a negative count (which shifts --exclude's indices up instead of removing volumes)";
    TED_14 = "TED-14", "tedana", true,
        "t2smap --debug's message on the T2* floor counts echoes times voxels as voxels (T2* values for N/E·V voxels), so its count and percentage are wrong for more than one echo";
    TED_15 = "TED-15", "tedana", true,
        "t2smap --combmode paid weights each echo by its temporal signal-to-noise ratio, 0 where the echo's standard deviation over time is 0, so a voxel whose good echoes are constant (and every voxel of a one-volume series) has weights summing to zero and t2smap fails with numpy's ZeroDivisionError: Weights sum to zero, can't be normalized";
    TED_16 = "TED-16", "tedana", true,
        "t2smap --exclude accepts a list that removes every volume (with or without --dummy-scans): the adaptive mask is then made from no volume and the log-linear fit solves no rows, so every voxel's T2* is 1/0 (infinite, then set to 500 ms) and S0 is 1, and t2smap writes these maps and the combination they weight with exit status 0 (the curve fit fails with numpy's ValueError of an empty minimum, the dropout method with the argmax of an empty sequence)";
    TED_17 = "TED-17", "tedana", true,
        "t2smap fails with IndexError: too many indices for array: array is 0-dimensional, but 1 were indexed when a single voxel is left for the fit (a one-voxel mask, or one voxel with good signal), with both fit types and both fit modes";
    TED_18 = "TED-18", "tedana", true,
        "t2smap caps the limited T2* map at its 99.5th percentile, which is 0 when more than 99.5 % of its values are 0 (the voxels with one good echo, which the limited map sets to 0): every positive limited T2* is then set to 0, so the limited map holds no estimate and the RMSE is computed with T2* = 0 (numpy's divide-by-zero warning)";
    TED_31 = "TED-31", "tedana", true,
        "t2smap fails with IndexError: index 0 is out of bounds for axis 1 with size 0 when the adaptive mask removes every voxel of the mask (no voxel left for the fit), with both fit types and both fit modes, after writing the adaptive mask image";
    TED_46 = "TED-46", "tedana", true,
        "t2smap's log-linear fit (also the start of curvefit) solves each echo-count group as one batched least-squares problem, so a voxel whose observations have no slope (all equal: no decay, e.g. saturated samples) gets the batch's rounding noise as its slope instead of 0: T2* 1 ms where the noise is negative, 1e16 ms and more where it is positive (the cap can move with them), decided by the voxel's place in the batch and the LAPACK and BLAS builds, never the 500 ms an exact zero slope gives";
    SDC_12 = "SDC-12", "sdcflows", true,
        "ReorientImageAndMetadata checks the upper-cased orientation code but reorients to the code as given, so a lower-case one fails in nibabel";
    SDC_14 = "SDC-14", "sdcflows", true,
        "_norm_lap of an image with no positive (or no negative) value after the median's subtraction fails with numpy's IndexError";
    SDC_16 = "SDC-16", "sdcflows", true,
        "the fieldmap-less registration's restriction puts the weight 1.0 at the position of the phase-encoding axis among the fixed image's voxel axes, while ANTs weighs the physical axes: a sagittal or coronal grid frees another axis and holds the phase encoding at 0.1";
    SDC_17 = "SDC-17", "sdcflows", true,
        "2.17.1 lacks three corrections of 2.15.2's estimator search: bids_filters also restrict the fieldmap-less estimation's anatomical reference and targets, no sessionless anatomical reference serves sessioned runs, and the acquisition and ceagent filters never reach the EPI field maps";
    SDC_20 = "SDC-20", "sdcflows", true,
        "get_identifier's KeyError for an unknown index table is the literal text '{by}'";
    SDC_21 = "SDC-21", "sdcflows", true,
        "the files whose B0FieldIdentifier is a list are found by a case-insensitive regular-expression search of every filter, so other subjects' (01 matches 010) or identifiers' files join the estimator";
    SDC_22 = "SDC-22", "sdcflows", true,
        "get_workflow and init_fmap_preproc_wf pass every keyword argument on to the estimator's workflow, which ignores the ones it does not take: a misspelt option is ignored without a word";
    SDC_24 = "SDC-24", "sdcflows", true,
        "_qwarp2ants squeezes every axis of length one before it adds the vector axis, so the warp of a one-slice image loses that spatial axis and is not a displacement field ANTs reads";
    SDC_26 = "SDC-26", "sdcflows", true,
        "init_unwarp_wf given both fmap2data_xfm and data2fmap_xfm, which ApplyCoeffsField declares exclusive, uses data2fmap_xfm and ignores the other without a word";
    SDC_27 = "SDC-27", "sdcflows", true,
        "match_histogram assigns the matched values into an integer image with numpy's cast: values its type cannot hold and NaN have an undefined conversion (on x86-64 a wrapped value)";
    SDC_28 = "SDC-28", "sdcflows", true,
        "epi_mask casts its watershed levels to uint16: a flat residual (0 / 0), a NaN or an infinity, or an overflow, gives an undefined conversion (0 on x86-64)";
    SDC_29 = "SDC-29", "sdcflows", true,
        "its BIDS app builds the fieldmap-less estimators of a participant without field maps without their inputs: they fail when run, after the other estimators' work has started";
    SDC_30 = "SDC-30", "sdcflows", true,
        "its BIDS app records --session-label and never uses it: every session is estimated";
    SDC_31 = "SDC-31", "sdcflows", true,
        "its BIDS app reads --bids-filter-file and never uses it";
    SDC_32 = "SDC-32", "sdcflows", true,
        "its BIDS app records --sloppy and --debug and builds every estimator without them";
    SDC_33 = "SDC-33", "sdcflows", true,
        "its BIDS app accepts option values it then fails on with a Python traceback: a --mem-gb unit it cannot look up, a --bids-filter-file that does not exist or is not JSON, a --use-plugin file that does not exist";
    SDC_35 = "SDC-35", "sdcflows", true,
        "its BIDS app reads --use-plugin's file with PyYAML's load without the Loader argument PyYAML 6 requires, so the option always stops the app with TypeError";
    SDC_36 = "SDC-36", "sdcflows", true,
        "its BIDS app records every participant's estimators in one registry, so two participants that share a B0FieldIdentifier stop it with KeyError";
    SDC_37 = "SDC-37", "sdcflows", true,
        "brain_masker's rough mask without a connected component makes the background the largest component, so the whole image is taken as the brain (a constant image divides 0 by 0)";
    SDC_38 = "SDC-38", "sdcflows", true,
        "brain_masker with no brain seed left after the rough mask's erosion takes the background's probability as the brain's";
    SDC_45 = "SDC-45", "sdcflows", true,
        "FieldmapFile takes its entities from the file's absolute path, where pybids' derivatives patterns (den-, res-, desc-, label-, ...) have no separator before them: a dataset below a folder such as golden-clone or andres-lab gives every file den or res, and the fieldmap-less search queries the layout with them and stops (ValueError: 'den' is not a recognized entity)";
    SDC_46 = "SDC-46", "sdcflows", true,
        "brain_masker with padding=0 crops its random walker's probabilities with [..., 0:-0, 0:-0, 0:-0], an empty slice: the probability and the mask are written as empty (0 x 0 x 0) images and the brain is written unmasked (an empty boolean index changes nothing)";
    SKI_01 = "SKI-01", "scikit-image random_walker", true,
        "with a mask (negative labels or isolated seeds) the Laplacian's nodes are numbered by the remaining edges' ends but the labels by the mask, so a mask pixel without a neighbour in it shifts every later node and the system pairs labels with the wrong pixels";
    SKI_02 = "SKI-02", "scikit-image match_histograms", true,
        "an unsigned-integer image's reference values are counted with numpy's bincount, which refuses a reference with non-integer or negative values, and a uint64 image";
    SKI_03 = "SKI-03", "scikit-image random_walker", true,
        "labels that are not consecutive come back as their consecutive renumbering from 1, not the labels given";
    SKI_04 = "SKI-04", "scikit-image random_walker", true,
        "return_full_prob with no pixel to segment returns the seeds' boolean masks concatenated along the last axis, not the (nlabels, ...) probabilities";
    GLIBC_05 = "GLIBC-05", "glibc 2.35", true,
        "cbrtf is not correctly rounded: about one float in ten (10.6 %) gets a result one unit in the last place from the exact cube root, and 3dUnifize -EPI takes its radius from it (18.3 mm over the cube root of the voxel volume), so on some voxel sizes its ball of neighbours or the radius it prints is another than the exact cube root gives";
    GLIBC_06 = "GLIBC-06", "glibc 2.35", true,
        "tanh (double precision) is not correctly rounded: about one argument in five gets a result one unit in the last place from the exact value, and FSL's bet2 takes its surface's smoothness weight from it at every step, so where such a last bit survives (most of all where a self-intersecting surface is rerun, which amplifies it) the surface and the brain mask are other than the exact tanh gives";
    GLIBC_07 = "GLIBC-07", "glibc 2.35", true,
        "the mathematical functions FSL's and AFNI's programs call are not correctly rounded: the double-precision sin, cos, tan, asin, acos, atan, exp, log and pow (on image intensities, between 0.02 % (log) and 0.21 % (tan) of the arguments get a result one unit in the last place from the exact value) and the single-precision sinf, cosf, atan2f, expf, logf and powf; so fslmaths' -sin, -cos, -tan, -asin, -acos, -atan, -exp, -log and -pow of a double image (-dt double) differ in that last place on those voxels (a float image, the default, keeps the same values), its kernels, -bptf, -tfce and -roc, fslstats' -e and -E and 3dTshift's -wsinc5 and -wsinc9 weights (about 1 % of them) can take another last bit, and the iterative programs (FLIRT and its programs, mcflirt's mutual-information costs, FAST, topup's smoothing, bet2's powers, betsurf's blurring) another path; nor is sincos, which gives the twiddle factors of fslmaths' FFT convolution (through cexp): of the transform lengths 1 to 256, 80 hold a factor one unit in the last place from the exact one, so a few voxels of such a convolution differ in their last place; and numpy's float64 exp and log are the C library's on x86-64 CPUs without AVX-512 (and SciPy's powers C's pow), which tedana's t2smap takes for its S0, its curve fit's model, the T2* floor, the RMSE and the combination's weights; and FreeSurfer's matrices and histograms call cosf and sinf (rotations), expf (Gaussian matrices), exp, pow and log (the histograms' smoothing, robust fit and entropy), log (Gaussian draws) and log2 (the Powell registration's mutual-information costs)";
    AFNI_01 = "AFNI-01", "AFNI", true,
        "a negative scl_slope is dropped where a voxel's series is read (THD_extract_array) but applied elsewhere: 3dTshift's shifted slices come out sign-flipped and scaled, 3dAutomask averages several sub-bricks without the factor, 3dUnifize -EPI's median volume has the wrong sign";
    AFNI_02 = "AFNI-02", "AFNI", true,
        "float32 data with a slope and an intercept are scaled when read and keep the slope in the output header, so they are scaled twice";
    AFNI_03 = "AFNI-03", "AFNI", true,
        "header slice code 2 (sequential decreasing) gives every slice time 0: 3dTshift shifts nothing, and 3dAutomask and 3dUnifize write no slice timing";
    AFNI_05 = "AFNI-05", "3dTshift", true,
        "-no_detrend removes and restores the mean of only the first voxel of each pair; the second is shifted as raw values";
    AFNI_07 = "AFNI-07", "AFNI", true,
        "metre and micron units: the voxel sizes and directions are converted to millimetres but the origin is not (and an oblique matrix is not converted at all)";
    AFNI_08 = "AFNI-08", "AFNI", true,
        "a copy of the input whose slice times are all within 0.003 of 0 is written with slice_end 0";
    AFNI_11 = "AFNI-11", "AFNI", true,
        "an output file that exists already is not written, and the program exits with status 0";
    AFNI_12 = "AFNI-12", "AFNI", true,
        "a -prefix with blanks, shell metacharacters or non-ASCII characters anywhere in its path is refused";
    AFNI_16 = "AFNI-16", "AFNI", true,
        "a voxel-to-world matrix within about 0.01 to 0.024 degrees of plumb (a single-precision test; the angle depends on the voxel size) is written as the exactly plumb matrix of its axes, its tilt discarded";
    AFNI_18 = "AFNI-18", "AFNI", true,
        "an input name with a blank is read as a list of datasets to join in time (unless AFNI_PATH_SPACES_OK is YES), so an input in a directory whose name has a blank cannot be opened";
    AFNI_19 = "AFNI-19", "3dTshift", true,
        "with AFNI_ALLOW_MILLISECONDS set and -TR in milliseconds, the time offset (toffset) is written in milliseconds while the TR is written in seconds, both labelled seconds";
    AFNI_20 = "AFNI-20", "AFNI", true,
        "a dataset of one row (one voxel along y and z) is written as a 1D text file under the NIfTI output name (unless AFNI_WRITE_1D_AS_PREFIX is YES)";
    AFNI_22 = "AFNI-22", "AFNI", true,
        "a -Dname=value global option whose value has blanks, or a blank around its '=', sets another value (-DX=a b sets X to 'a=b'), and one longer than 255 or 1023 characters is cut";
    AFNI_23 = "AFNI-23", "AFNI", true,
        "with AFNI_NIFTI_SCALE=NO, uint8 and int16 data with both a slope and an intercept stay integers with the slope as their scale factor and lose the intercept, so the values written are off by it";
    AFNI_26 = "AFNI-26", "AFNI", true,
        "an output that cannot be written (its directory is missing or cannot be written) is not written, and the program exits with status 0";
    AFNI_41 = "AFNI-41", "3dAutomask", true,
        "-nbhrs outside 6..26 (the range its help gives) is silently replaced: the peeling acts as with 17";
    AFNI_43 = "AFNI-43", "3dAutomask", true,
        "-peels 0 peels once: the help says that 0 turns the peeling off, but the peel count is set to 1 for any value below 1";
    AFNI_45 = "AFNI-45", "3dUnifize", true,
        "with -T2up at or near 100, the contrast is inverted about the value one past the sorted mask values (whatever the memory there holds; usually 0, which zeroes the output)";
    AFNI_47 = "AFNI-47", "3dUnifize", true,
        "a volume of more than a million voxels with an axis of 2 or 3 voxels: its half-size scale image expands back to a grid of another size, which AFNI reads as if it were the input's, past its end";
    AFNI_48 = "AFNI-48", "3dUnifize", true,
        "-T2 given three or more times (the help documents once and twice) leaves the output inverted: neither inverted back nor edge-clipped";
    AFNI_75 = "AFNI-75", "3dUnifize", true,
        "-EPI sets the neighbourhood's radius to 18.3 mm over the voxels' geometric mean size, in voxels, with no upper bound: on micrometre (or otherwise tiny) voxels the ball of neighbours has trillions of points, built before any is used, which fills the machine until the process is stopped";
    AFNI_86 = "AFNI-86", "3dvolreg", true,
        "-nocoarserot matches -noclip's first four letters, which the option scan tests first: it turns the clipping of the registered bricks off and leaves the coarse rotation search on";
    AFNI_87 = "AFNI-87", "3dvolreg", true,
        "a byte dataset's registered sub-bricks are written through mri_to_byte, which stretches each to 0..255.9 from its own minimum and maximum";
    AFNI_88 = "AFNI-88", "3dvolreg", true,
        "an option that needs a value, given last on the command line, reads past the arguments (a crash; -savedisp's check is off by one)";
    AFNI_89 = "AFNI-89", "3dvolreg", true,
        "a base whose least squares cannot be set up (all-zero weights, or a rotated or shifted copy that equals the base under the weight: a constant volume, a dimension below 2) prints lsqfit's errors and crashes at the first registered sub-brick";
    AFNI_90 = "AFNI-90", "3dvolreg", true,
        "-prefix, -dfile and -1Dfile longer than 255 bytes, and a -weight dataset name longer than 255, overflow fixed buffers; -maxdisp1D names are cut to their first 997 bytes";
    AFNI_92 = "AFNI-92", "3dvolreg", true,
        "-maxdisp (the default) sets the automask's clip fraction to 0.333 for the rest of the run, so -nomaxdisp changes -twopass's coarse rotation search (its mask with 0.5): an output-only option changes the registration";
    AFNI_93 = "AFNI-93", "3dvolreg", true,
        "the coarse search of -twopass on a volume too thin for its edge (n - edge <= edge on an axis) sums over no voxel, so every shift scores 0 and the first candidate wins: a start shifted by -2 steps along z and rotated by -9, -18, -9 degrees";
    AFNI_94 = "AFNI-94", "3dvolreg", true,
        "-twopass with -coarse 1 on a volume of 120 or more voxels along every axis halves the coarse step to 0 for the subsampled search, which never ends";
    AFNI_95 = "AFNI-95", "3dvolreg", true,
        "a second -base is refused only when the first was a dataset or an index above 0: -base 0 -base 3 registers to brick 3";
    AFNI_96 = "AFNI-96", "3dvolreg", true,
        "a -rotparent dataset with VOLREG_MATVEC_000000 but without VOLREG_CENTER_OLD or VOLREG_CENTER_BASE (or with fewer than 3 numbers in one) is read through a NULL pointer or past the attribute (a crash or garbage)";
    AFNI_97 = "AFNI-97", "3dvolreg", true,
        "a Fourier rotation whose shear shifts the last of an odd count of rows, and the previous pair's second row, beyond the row length clears the missing second row through a NULL pointer (fft_shift2): a crash";
    AFNI_98 = "AFNI-98", "3dvolreg", true,
        "with -input, the messages about a base index beyond the volumes and a weight of another size print the argument after the options instead of the dataset: (null) when there is none";
    AFNI_99 = "AFNI-99", "3dvolreg", true,
        "-maxdisp and -savedisp convert the matrix to the dataset's axes with their signs but leave the voxel positions unsigned: for an axis stored L2R, P2A or S2I the displacements are those of the mirrored position";
    FSL_01 = "FSL-01", "mcflirt", true,
        "the last volume of the series starts its first stage from the identity, not from the previous volume's result";
    FSL_02 = "FSL-02", "mcflirt", false,
        "the normalised correlation's count N is a running sum of running sums of the weights, so the correlation is effectively uncentred";
    FSL_03 = "FSL-03", "mcflirt", true,
        "the in-plane mode pads the first stage's reference grid twice";
    FSL_04 = "FSL-04", "mcflirt", true,
        "-gdt registers each volume's gradient image to the reference's plain image (the reference's gradient is computed and discarded), and aborts when -out names a directory";
    FSL_05 = "FSL-05", "mcflirt", true,
        "-scaling and -hist are accepted and ignored";
    FSL_06 = "FSL-06", "mcflirt", true,
        "an unknown -cost silently keeps normcorr";
    FSL_07 = "FSL-07", "mcflirt", true,
        "numbers are read with C's atoi and atof (text reads as 0, 1.5 as 1); -help alone exits 1";
    FSL_08 = "FSL-08", "mcflirt", true,
        "the -stats images leave the first and last slices at 0, and their names keep the -out extension inside";
    FSL_09 = "FSL-09", "mcflirt", true,
        "int8 input is written as uint8, and integer outputs are truncated toward zero instead of rounded";
    FSL_10 = "FSL-10", "mcflirt", true,
        "output units are always millimetres and seconds, so a TR in milliseconds is labelled seconds";
    FSL_11 = "FSL-11", "FSL", true,
        "with FSLOUTPUTTYPE unset the programs stop with an error at their first image (exit 1), after the outputs written before it";
    FSL_13 = "FSL-13", "mcflirt", true,
        "-rmsrel and -rmsabs create the .mat directory without -mats (holding only the reference's identity)";
    FSL_14 = "FSL-14", "mcflirt", true,
        "one NaN or infinite voxel makes the volume's centre of mass, and so its matrix, NaN";
    FSL_21 = "FSL-21", "mcflirt", true,
        "with a histogram cost (corratio, mutualinfo, normmi, woods) and -bins below 8, the first stage's histogram has no bins (-bins divided by the 8 mm scale): the costs are degenerate and the matrices arbitrary, and corratio writes before its arrays and aborts";
    FSL_20 = "FSL-20", "mcflirt", true,
        "when the .mat directory cannot be created for another reason than its permissions (a missing parent directory), mcflirt prints an error and exits with status 0, writing neither the matrices, the .par and .rms files nor the corrected series";
    FSL_22 = "FSL-22", "FSL", true,
        "the background value sorts the 2-voxel border with std::sort and operator<: with NaN voxels the order is undefined in C++, so the padding value of a resampled volume is arbitrary";
    FSL_40 = "FSL-40", "bet2", true,
        "the input and output names lose their first '.hdr' wherever it stands (a directory named x.hdr is read as x), and a name with '.img' but no '.hdr' aborts the program";
    FSL_41 = "FSL-41", "bet2", true,
        "with no voxel strictly between the 2nd and 98th percentiles inside the initial sphere (a constant or binary image), the median brain intensity is read from the end of an empty list: FSL crashes";
    FSL_42 = "FSL-42", "bet2", true,
        "numeric options without meaning are used as given: a NaN threshold (-f, -g), a negative, NaN or infinite radius (-r), a smoothness of 0 or less (-w), a centre (-c) or search depth (--parameter_d1/d2) that is not finite";
    FSL_43 = "FSL-43", "bet2, betsurf", true,
        "the fill of a surface's mask starts at the vertices' centroid without a bounds check: a centroid outside the image (a single slice, a surface leaving the grid) writes and reads outside the volume";
    FSL_44 = "FSL-44", "bet", true,
        "the script expands its arguments unquoted: a name with a blank is split into words (an input with a blank is reported not valid, an output with a blank writes other files), and an empty argument ends the options";
    FSL_45 = "FSL-45", "bet", true,
        "-Z crops its outputs back with imglob, called without ${FSLDIR}/bin: with FSL's programs off the PATH the outputs stay padded, and with them on every other image whose name starts with the output's is cropped too";
    FSL_46 = "FSL-46", "bet", true,
        "the cleanup removes every file whose name starts with <output>_tmp, also files the run did not create";
    FSL_47 = "FSL-47", "betsurf", true,
        "--t1only fills the scalp mask from the centre of the outer skull surface, taken in the padded working grid, instead of the scalp surface's";
    FSL_48 = "FSL-48", "betsurf", true,
        "a T2 image on another grid than the T1 (other dimensions or voxel sizes) is sampled at the T1's voxel coordinates";
    FSL_50 = "FSL-50", "betsurf", true,
        "one skull profile whose search range is empty ends the program with status 255 and no message, before any output is written";
    FSL_51 = "FSL-51", "betsurf", true,
        "a profile search can read past the profile's last sample, into memory FSL never set, and use it";
    FSL_52 = "FSL-52", "betsurf", true,
        "mesh files are read without checks: a number that does not read leaves values never set, and a face naming a vertex the file does not have reads past the vertices";
    FSL_53 = "FSL-53", "betsurf", true,
        "a matrix file with fewer than the 12 numbers betsurf uses leaves the missing entries at values never set (the first that does not read is 0)";
    FSL_54 = "FSL-54", "betsurf", true,
        "--t1only exits with status 0 also when it fails (too few arguments, a matrix file it cannot open)";
    FSL_55 = "FSL-55", "betsurf", true,
        "a NaN in the T1 inside the brain surface makes the 98th percentile (std::nth_element) undefined";
    FSL_58 = "FSL-58", "betsurf", true,
        "with no voxel of the T1 inside the brain surface, the 98th percentile of the brain's intensities is read from an empty list (std::nth_element, then the element before the list's start): FSL crashes or uses memory it never wrote";
    FSL_59 = "FSL-59", "mcflirt", true,
        "-sinc_final and -nn_final resample the series with trilinear interpolation (FSL 6.0.7.23: newimage 2601.0 keeps the interpolation method each volume of a series had when it was first accessed, so the method set for the final resampling never reaches it)";
    FSL_60 = "FSL-60", "FSL", true,
        "an image name holding '.img' before its extension (in a directory or in the file's own stem) is opened under another name (FSL 6.0.7.23: NewNifti 5.0.0 replaces the name's last '.img' with '.hdr'): another file is read or written, or the program aborts";
    FSL_61 = "FSL-61", "bet2", true,
        "--binaryMask with an input on another grid than the mask: the brain mask made on the mask's grid is applied over the input's dimensions, reading and writing outside it (FSL 6.0.7.23)";
    FSL_62 = "FSL-62", "bet2", true,
        "--ct with --binaryMask converts the mask instead of the CT image and writes the mask, masked, as the brain image (FSL 6.0.7.23)";
    FSL_63 = "FSL-63", "bet", true,
        "bet -R (and -B with -s) passes bet2's options through bash's echo, which takes leading words made of -n, -e and -E as its own options: a leading -e (bet2's mesh) is dropped, and the other options' backslashes are read as escapes";
    FSL_56 = "FSL-56", "FSL", true,
        "an image saved with two or more NIfTI extensions of different sizes gets the wrong vox_offset (FSL counts the first extension's size once per extension): the data start inside the extensions or after a gap";
    FSL_65 = "FSL-65", "mcflirt", true,
        "an output text file that cannot be opened (its name a directory, its directory missing or not writable): -plots' .par file ends the run at volume 0's line with \"error: unable to open output file!\" and exit status 0, after the first matrix and the .rms files (no other matrix, no corrected series); an .rms file is silently not written";
    FSL_66 = "FSL-66", "mcflirt", true,
        "a negative -bins: each stage has -bins / scale bins (truncated toward zero), prepared for every cost function; with -1 bins the process is killed by SIGFPE (an integer division by zero), with -2 or fewer it aborts with std::bad_array_new_length; nothing is written";
    FSL_68 = "FSL-68", "flirt", true,
        "every error after the command line (an image or matrix file that cannot be read, a matrix of the wrong size, an unknown -bbrtype) ends the program with exit status 0: its main catches every exception and returns 0 after printing the message, with no output written";
    FSL_78 = "FSL-78", "midtrans", true,
        "an error after the transforms are read (a matrix that is not 4 x 4, a singular product, an image that cannot be read) ends the program with exit status 0: its main catches the exception, prints its message and returns 0, with no output written";
    FSL_79 = "FSL-79", "applyxfm4D", true,
        "spline interpolation of an integer image (char, short or int) keeps the spline coefficients in the image's integer type (each filtering pass's results rounded half up, toward zero below -0.5) and converts each interpolated value to the type before the output's own conversion: the values of a coarser, shifted spline";
    FSL_80 = "FSL-80", "applyxfm4D", true,
        "a sinc kernel asked for after another one was defined is replaced by the earlier one whatever their windows and widths: applyxfm4D -3D's rectangular sinc kernel of width 9 becomes the default blackman kernel of width 7 defined just before it";
    FSL_81 = "FSL-81", "aff2rigid", true,
        "aff2rigid stops with NameError: name 'mat' is not defined (FSL 6.0.7.23 runs it on numpy 2.4.6, and numpy 2 removed the mat alias): it never writes its matrix";
    FSL_76 = "FSL-76", "makerot", true,
        "the axis is divided by the square root of its length instead of its length: for an axis that is not of unit length the rotation is by the angle times the square root of the axis's length (30 degrees about 0,0,4 give 60)";
    FSL_77 = "FSL-77", "makerot", true,
        "an axis or centre of fewer than three values reads past the vector's values (what its buffer still holds: the default's zeros or an earlier value of the same option), and values beyond the third are ignored";
    FSL_75 = "FSL-75", "flirt", true,
        "the background value (the 10th percentile of the 2-voxel border) sizes its buffer too small for a volume 3 voxels wide along x (2 or more along y or z) or along y (2 or more along z, x not 2 or 4), as FLIRT's pyramid levels can be: it writes the rest past the buffer (heap corruption: FLIRT may abort or crash later, or finish) and takes the percentile of the values that fit; a 1 x 1 x 1 volume reads an empty buffer; fslmaths' -subsamp2 and -subsamp2offc compute the same value for such sizes (an image 3 voxels wide, as halving a 6-voxel axis makes it), which they never use: fslmaths may abort with glibc's double free or corruption";
    FSL_90 = "FSL-90", "flirt", true,
        "with -anglerep quaternion the search's starting points, which hold Euler angles, are composed as quaternions: the three angles are read as a quaternion's vector part, so a starting point whose angles' squares sum above 1 loses its rotation (FLIRT prints Parameters do not form a valid axis - greater than unity) and the others start from another rotation than the search found";
    FSL_91 = "FSL-91", "flirt", true,
        "a -refweight or -inweight with no voxel of at least 0.01 (an empty weighting mask, such as an image of zeros) is accepted: the weighted images are filtered to zeros and every weight is 0, so every cost is constant (NaN for bbr), and FLIRT writes the matrix its search happens to end at (an arbitrary rotation of its grid)";
    FSL_92 = "FSL-92", "flirt", true,
        "an input or reference of one voxel (1 x 1 x 1) is registered: one value cannot measure an alignment, and FLIRT writes the matrix its search happens to end at (an arbitrary rotation of its grid)";
    FSL_93 = "FSL-93", "flirt", true,
        "a voxel size of 0 in the input's or reference's header (which reading the image counts as 1) makes the base scale 0: every voxel size divided by it is infinite, the initial matrix NaN, and FLIRT never finishes; -basescale 0 the same";
    FSL_96 = "FSL-96", "fast", true,
        "-p's probability maps are never written outside the brain mask (the voxels of value 0 or below), nor anywhere with --nopve when FAST runs no k-means iteration (-W 0 -O 0) or initialises from priors (-a): FAST saves whatever its memory held there, zeros or other values depending on how the memory was reused";
    FSL_97 = "FSL-97", "fast", true,
        "with several channels and -N, the bias field -b saves and -B applies is never computed: FAST saves whatever its memory held (infinite fields and zero images were seen)";
    FSL_98 = "FSL-98", "fast", true,
        "with several channels, sorting the classes by channel 1's means gives each sorted class the covariance, posteriors and other channels' means of another class (the inverse of the sorting permutation): right only when the permutation is its own inverse (no change, a swap, -t 2's reversal), wrong when classes move in a cycle (for example -s means given in a cyclic order)";
    FSL_99 = "FSL-99", "fast", true,
        "numeric options without meaning are used as given: a smoothness (-H, -f, -R) that is not a finite number (a NaN -H passes the check for a negative one and estimates the smoothness against a table of zeros, or with several channels of memory never written), a bias field smoothing extent (-l) of 0 or less or not finite (every voxel ends in class 1), fewer than one class (-n; FAST aborts)";
    FSL_100 = "FSL-100", "fast", true,
        "with FSLDIR unset FAST aborts (std::logic_error) before it segments, even when no priors are asked for (-A avoids it)";
    FSL_101 = "FSL-101", "fast", true,
        "when FLIRT cannot register the priors (-a naming a matrix FLIRT cannot read, a prior it cannot resample), FAST prints that the priors are not used, then initialises from priors it never allocated and crashes";
    FSL_102 = "FSL-102", "fast", true,
        "-a registers the priors by running FLIRT through the shell with the names unquoted: a name with a blank is split into words and the shell's special characters in names are interpreted; with FSLDIR unset and -A the command starts with the text (null), a syntax error of the shell's";
    FSL_103 = "FSL-103", "fast", true,
        "the brain mask (every voxel above 0) leaves out the voxels equal to the image's maximum when the maximum is 2^24 or more (the upper bound maximum + 1 rounds to the maximum in single precision)";
    FSL_104 = "FSL-104", "fast", true,
        "-S n with images of different sizes reads and writes the smaller images past their ends (over channel 1's grid)";
    FSL_111 = "FSL-111", "applyxfm4D", true,
        "a uint8 image is read into a char image, signed on x86-64: its values 128 to 255 are read as v - 256 (255 as -1), so interpolating across 127 and 128, the background value (the border's 10th percentile) and the spline's coefficients are those of the negative values; the output's own conversion wraps a signed char back to its byte";
    FSL_117 = "FSL-117", "topup", true,
        "with --regmod=membrane_energy, Levenberg-Marquardt's steps use a Hessian of the membrane energy measured in mm (each first derivative divided by the voxel size) while the cost and its gradient measure it in voxels: the regularisation's curvature is underestimated by the squared voxel size, so the steps do not follow the cost being minimised";
    FSL_118 = "FSL-118", "topup", true,
        "with --regrid=1 (the default), subsamplings whose largest does not divide the others (3 and 2) stop topup at the first level with the smaller one with \"invalid subsampling factor\", although each divides the image and the command line's check accepted them: the grid is the image grown by the largest subsampling, which the smaller one does not divide";
    FSL_119 = "FSL-119", "topup", true,
        "--fout and --featout's field maps carry the input's intent parameters shifted by one place: the first is written as 0, the second and third as the input's first and second (its third lost)";
    FSL_120 = "FSL-120", "topup", true,
        "with --acqp, an acquisition-parameter file whose number of rows differs from the number of volumes is reported as a mismatch between the image and an empty name (the --datain value, which is not set)";
    FSL_121 = "FSL-121", "topup", true,
        "with FSLDIR unset, a --config name that is not found where topup runs stops topup with the C++ library's text basic_string: construction from null is not valid, which does not say what is missing";
    FSL_122 = "FSL-122", "topup", true,
        "a line of a --config file whose text after its first word is longer than 1023 characters (a long comment) ends the reading of the file there: every setting after it is silently ignored and its defaults used";
    FSL_123 = "FSL-123", "topup", true,
        "a negative --miter at a level runs no iteration there (the level's estimation is skipped silently): the number of iterations is compared as a signed number below an unsigned maximum";
    FSL_124 = "FSL-124", "topup", true,
        "a negative --warpres gives a knot spacing of about four billion voxels (a negative number converted to unsigned): the spline field's kernels need more memory than any machine has, so topup is killed or stops with an allocation failure";
    FSL_125 = "FSL-125", "topup", true,
        "a negative --fwhm is accepted and acts as no smoothing (a kernel of a width that is not positive is the identity), which is not what a smoothing width asks for";
    FSL_126 = "FSL-126", "topup", true,
        "a negative --lambda is accepted: the regularisation rewards a rough field, so the minimisation drives the field to large values and the cost below zero";
    FSL_132 = "FSL-132", "applytopup", true,
        "--interp=spline (the default) is not applied to the first volume of the first --imain image: that volume is resampled by trilinear interpolation (the image's interpolation as read) and every other volume by splines, so with --method=jac the first image's first volume, and with --method=lsr its movement correction, differ from the method asked for";
    FSL_133 = "FSL-133", "applytopup", true,
        "--method=lsr reads the field's displacement just before a line's first voxel from the wrong place when the line's length (the image's size along the phase encoding) does not divide 2^32 (30, 96, 100, ... voxels; powers of two are right): the line is not wrapped around circularly, so the first rows of each line's restoration matrix use the displacement of a voxel in the middle of the line instead of its last voxel";
    FSL_134 = "FSL-134", "applytopup", true,
        "a --topup field other than topup's coefficients on the images' grid: with a field image (topup's --fout or another field map in Hz) --method=jac computes the Jacobian determinant with the y displacements in units of the x voxel size (wrong when the phase encoding is along y and the voxels are not square in x and y); and a field on another grid is not refused: with --method=jac a field with as many voxels as the images but another shape is laid on the images' grid as it is stored (its displacement, and its Jacobian determinant at the same voxel indices, 0 beyond the field's own grid), one with another number of voxels stops applytopup with an internal message; with --method=lsr the field's lines are read past their end";
    FSL_135 = "FSL-135", "applytopup", true,
        "an unknown --method writes no output and exits with status 0 without a message; --method=lsr (and the undocumented vb2D, vb3D and vb4D) with a --topup that gives no movement parameters (a field image, or coefficients without their _movpar.txt) aborts with an internal index error";
    FSL_141 = "FSL-141", "applytopup", true,
        "the undocumented --method=vb2D, vb3D and vb4D count the data points of the noise and regularisation priors (half the number of values) in 32-bit integers: beyond about two billion values (very large 4D images with vb4D) the counts wrap around to wrong, even negative, numbers";
    FSL_142 = "FSL-142", "applytopup", true,
        "the undocumented --method=vb3D and vb4D count two images' data points for the noise prior whatever the number of images restored together: with three or more images of one phase-encoding axis the noise precision is estimated from too few data points (vb2D counts them right)";
    FSL_143 = "FSL-143", "applytopup", true,
        "the undocumented --method=vb3D and vb4D regularise with a smoothness prior whose coupling between a voxel and the voxels next to its neighbours in the adjacent slices (one step along the phase encoding and one slice away) is half what the squared second-difference operator it is built from gives (the coupling within a slice is right)";
    FSL_144 = "FSL-144", "applytopup", true,
        "--method=jac with a phase encoding that has a z component (an acquisition row whose third element is not 0, which topup refuses: it estimates no field for one) silently leaves that component out: the image is displaced along x and y only, so an image encoded along z alone is written uncorrected";
    FSL_145 = "FSL-145", "topup", true,
        "with --splineorder=2, an axis of n voxels shorter than one knot spacing k gets one B-spline coefficient, and one shorter than two spacings two when k is 3 voxels or more, where the general count is ceil((2n + k - 4) / (2k)) + 2 (every other axis): the field's support ends inside the axis, so the field is forced towards 0 at its far end (exactly 0 from 1.5 spacings on) whatever the images show, and a level that refits a field of a single coefficient along an axis stops topup with an internal index error";
    FSL_146 = "FSL-146", "topup", true,
        "with volumes whose phase encodings run along different axes (for example AP, PA, LR and RL), the cost's gradient leaves out of each axis' Jacobian term the volumes not encoded along that axis (their share of the mean prediction's derivative): the gradient is not the cost's, so the minimisers step in a wrong direction (a first Levenberg-Marquardt step can raise the cost) and end where this false gradient vanishes rather than at the cost's minimum";
    FSL_151 = "FSL-151", "fslmaths", true,
        "a value converted to an integer type outside its range, or NaN, wraps around or becomes the most negative integer: values of an integer output type (-odt char, short, int), -rem's integer remainder, numbers and results under -dt char, short or int";
    FSL_152 = "FSL-152", "fslmaths", true,
        "-thr, -thrp, -thrP, -uthr, -uthrp, -uthrP, -bin, -binv and -mas bound their range by the image's maximum plus 1 (or minimum minus 1), which wraps around under -dt char and int and rounds back to the maximum for values from 2^24 (and infinity): the largest (or smallest) voxels are zeroed";
    FSL_153 = "FSL-153", "fslmaths", true,
        "a NaN first voxel makes the image's minimum and maximum NaN: -thr, -uthr, -bin, -binv and -mas then zero every voxel, and -range writes a NaN display range; NaN voxels also count in the lowest bin of the robust range of -thrp, -thrP, -uthrp and -uthrP; along a dimension (-Tmax, -Tmin, -Tmaxn and their X, Y and Z forms) a NaN first value makes the maximum and minimum NaN and the index of the maximum 0, while NaN values elsewhere are left out";
    FSL_154 = "FSL-154", "fslmaths", true,
        "-dt char and -dt input on a uint8 image compute in signed 8-bit values, so the voxels from 128 to 255 are read as negative: -thr, -bin, the comparisons, divisions and conversions to other types treat them as negative (additions keep the right low byte)";
    FSL_155 = "FSL-155", "fslmaths", true,
        "-dt input on an image stored as int8, uint16, uint32, int64, uint64 or float128 computes nothing, writes no output and exits with status 0";
    FSL_156 = "FSL-156", "fslmaths", true,
        "an integer division by zero crashes fslmaths (a floating-point exception): -rem by a number or an image value whose integer part is 0, -div by such a number under -dt char, short or int, -grid with a spacing of 0, and the most negative int divided by -1 under -dt int";
    FSL_157 = "FSL-157", "fslmaths", true,
        "-odt input on an int8 or uint16 image truncates the result toward zero instead of rounding it as the other integer outputs are, and writes an int8 output as uint8: a negative value becomes another (its byte read as unsigned)";
    FSL_158 = "FSL-158", "fslmaths", true,
        "an operation that takes more arguments than remain before the output name reads past the end of the command line: it crashes, or reads the environment";
    FSL_159 = "FSL-159", "fslmaths", true,
        "-div, -rem, -max and -min with an image of another size are not refused: -div and -rem take the values of the image's own corner of the same voxel indices (0 outside it), -max and -min read the second image's values at misplaced positions, or past its end when it is smaller";
    FSL_160 = "FSL-160", "fslmaths", true,
        "a Gaussian kernel of zero width (-s 0, -kernel gauss 0, or a sigma so small that twice its square is 0 in single precision) divides 0 by 0 at its centre: -s, -fmean and -fmeanu then make every voxel NaN instead of leaving the image as it is";
    FSL_161 = "FSL-161", "fslmaths", true,
        "-kernel and -s accept a kernel without voxels (a size of 0 or below along an axis, a negative sigma or radius, or an unknown kernel type) and filter with it: the dilations, erosions and median leave the image as it is, -fmean and -fmeanu make it NaN or 0, -s makes it 0, with the message ERROR:: Empty mask image";
    FSL_162 = "FSL-162", "fslmaths", true,
        "-fmedian, and -dilD at a zero voxel, stop the program (an uncaught exception: sort(): detected NaN) when a NaN lies under the kernel, instead of filtering the other values; -rank and -ranknorm stop it the same way at a voxel whose time series holds a NaN, instead of ranking the other values";
    FSL_163 = "FSL-163", "fslmaths", true,
        "-subsamp2 and -subsamp2offc of a 4D image stop the program (an uncaught exception: 3D only method called by higher-dimensional volume), except -subsamp2offc of even sizes, which keeps the first volume only";
    FSL_164 = "FSL-164", "fslmaths", true,
        "-dog_edge of a 4D image filters the first volume only: the zero crossings of the other volumes are taken from their unfiltered values";
    FSL_165 = "FSL-165", "fslmaths", true,
        "-fillh and -fillh26 of an image with a size of 1 along an axis (a single slice) crash (an integer division by zero in the test for the image's edge)";
    FSL_166 = "FSL-166", "fslmaths", true,
        "-tensor_decomp stops the program (an uncaught exception: Mat::operator(): index out of bounds) at a voxel whose tensor holds a NaN or an infinite value, instead of leaving that voxel at 0";
    FSL_167 = "FSL-167", "fslmaths", true,
        "the standard deviation along a dimension (-Tstd, -Xstd, -Ystd, -Zstd) and -ranknorm's take the variance as (n / (n - 1)) (mean of squares - squared mean) without the clamp at 0 and the guard for one value that the masked variance has: a nearly constant series whose variance rounds below 0 gives NaN, and a dimension of size 1 (-Tstd of a 3D image) gives NaN instead of 0";
    FSL_168 = "FSL-168", "fslmaths", true,
        "the percentiles along a dimension (-Tmedian, -Tperc and their X, Y and Z forms) sort values holding NaN with a comparison NaN breaks: the order is not sorted, so the value taken is arbitrary, and the sort can read and write outside the values (a crash)";
    FSL_169 = "FSL-169", "fslmaths", true,
        "-pval, -pval0 and -cpval with NaN statistics: a NaN statistic gets a p-value (the smallest, 1/T, from -pval; 0 from -cpval), a NaN permutation value counts as smaller than the statistic, and -cpval's maximum of each permutation volume is the maximum of the voxels after its last NaN in storage order";
    FSL_170 = "FSL-170", "fslmaths", true,
        "-ztop of an infinite z gives NaN instead of 0 (for +inf) or 1 (for -inf)";
    FSL_171 = "FSL-171", "fslmaths", true,
        "-bptf with a sigma whose filter half-width is not a valid int (+inf, or a sigma above about 3.6e8 volumes for the high-pass, 5.4e7 for the low-pass): either filter crashes or stops the program (an uncaught exception: std::bad_array_new_length)";
    FSL_172 = "FSL-172", "fslmaths", true,
        "-ptoz of a p-value so small that 1 - p rounds to 1 (below about 1.1e-16) gives an infinite z and prints ndtri domain error for each such voxel, instead of the z of that p-value";
    FSL_173 = "FSL-173", "fslmaths", true,
        "-tfce reads one element past the end of its list of voxels above the lowest threshold in both of its loops over that list (each tests the element before the bound); the element read never changes the result, but the read may fall outside the program's memory when the list fills its allocation to the end of a page";
    FSL_174 = "FSL-174", "fslmaths", true,
        "-tfce and -tfceS with a threshold step that cannot advance the thresholds (a step below the precision of the float threshold it is added to, a NaN step, and for -tfceS a negative one): -tfce never ends, or with a NaN step returns zeros; -tfceS stops (an uncaught exception: std::out_of_range) once its arrays are full";
    FSL_175 = "FSL-175", "fslmaths", true,
        "-tfce of a volume whose maximum is 0 or below (an all-zero statistic image, for example) stops with tfce requires a positive deltaT input when the step is automatic, where no voxel is above the lowest threshold and the enhancement is 0 everywhere";
    FSL_176 = "FSL-176", "fslmaths", true,
        "-tfce and -tfceS accept any connectivity: -tfce joins the neighbours of the largest of 6, 18 and 26 not above it (none below 6), -tfceS none at all unless it is exactly 6, 18 or 26";
    FSL_177 = "FSL-177", "fslmaths", true,
        "-tfceS sizes its arrays of cluster sizes and thresholds from the voxel's value over the step, leaving out the lowest threshold (-tfce_minT) and the rounding of the accumulated float thresholds: with a negative lowest threshold, or one step more than planned, the program stops (an uncaught exception: std::out_of_range), and where the arrays have no element (the voxel's value over the step below 0) or the voxel's volume has no cluster at all it divides by zero (SIGFPE)";
    FSL_178 = "FSL-178", "fslmaths", true,
        "-tfceS with a voxel outside the image reads the image and the cluster labels at the voxel's offset from their start (another voxel, or memory outside the image)";
    FSL_179 = "FSL-179", "fslmaths", true,
        "-roc with a noise image of another size than the input indexes the noise image with the input's sizes: its log transform misses volumes (whose raw values then set the thresholds' range) or reaches voxels twice, and a smaller noise image is read and written outside its memory";
    FSL_180 = "FSL-180", "fslmaths", true,
        "-roc with a truth image without true voxels (or, without a noise image, without zero voxels) divides 0 by 0 for the true (false) positive rates, writes them as NaN to the curve and stops in its quartiles' sort (an uncaught exception: sort(): detected NaN)";
    FSL_181 = "FSL-181", "fslstats", true,
        "NaN values among the voxels counted: -R takes the minimum and maximum from the first value, so a NaN first value makes both NaN (and -x and -X give the first voxel's coordinates); -p and -P sort values holding NaN with a comparison NaN breaks, so the value taken is arbitrary and the sort can read and write outside the values (a crash); -r, -h, -H, -e and -E count NaN values in the lowest bin of their histograms";
    FSL_182 = "FSL-182", "fslstats", true,
        "without a mask, -s takes the variance as (n / (n - 1)) (mean of squares - squared mean) without the clamp at 0 and the guard for one value that the masked variance has (a constant image whose variance rounds below 0 gives NaN, a single voxel NaN), and -S squares each nonzero value in single precision before adding it and does not clamp at 0: a nearly constant image gets a standard deviation of a few parts in ten thousand of its values instead of 0, and a variance rounded below 0 gives NaN";
    FSL_183 = "FSL-183", "fslstats", true,
        "with -t, a 4D -k mask is refused (Mask and image must be the same size) and a -d image stops fslstats at the second time point when it is 3D (an uncaught index error: Invalid t index) or at the first when it is 4D (Image must be the same size as the base image): the volume for the time point is picked but never used";
    FSL_184 = "FSL-184", "fslstats", true,
        "-x and -X with a mask (or thresholds) that selects no voxel read the coordinates of an empty list: fslstats crashes";
    FSL_185 = "FSL-185", "fslstats", true,
        "-e and -E of an image whose robust range is empty (a constant image, after the thresholds and masks) crash fslstats (an integer division by zero indexing the empty histogram) instead of giving the entropy 0";
    FSL_186 = "FSL-186", "fslstats", true,
        "-H with a maximum that is not above the minimum prints no histogram, without a message";
    FSL_187 = "FSL-187", "FSL", true,
        "a value more than about two billion bin widths above a histogram's range is counted in its first bin instead of its last (its bin number does not fit in an int): in fslstats' histograms (-H, -e, -E), and in the robust range of fslstats (-r, -e, -E), fslmaths (-thrp, -thrP, -uthrp, -uthrP), bet2 and flirt, whose 2 % and 98 % limits move when most values are packed far below one value above them";
    FSL_188 = "FSL-188", "fslstats", true,
        "-w of a 4D image without nonzero voxels gives the time range as a first volume of tsize - 1 with a size of 2 - tsize (negative for three or more volumes), while the x, y and z ranges cover the whole image";
    FSL_189 = "FSL-189", "fslstats", true,
        "with -json, -v and -V write the number of voxels in single precision: above 16,777,216 voxels (a 4D series, a large 3D image) the count is rounded";
    FSL_191 = "FSL-191", "fslmaths", true,
        "-fmean and -fmeanu with a kernel large enough for FSL's FFT convolution (nvox * nker > 44 * 2 nvox * log2(2 nvox)) make every voxel of a volume NaN when one of its voxels is NaN or infinite, where the direct sum FSL uses for smaller kernels leaves NaN only within the kernel's reach of that voxel";
    FSL_192 = "FSL-192", "fslmaths", true,
        "-fmean and -fmeanu with a kernel large enough for FSL's FFT convolution (nvox * nker > 44 * 2 nvox * log2(2 nvox)) apply the kernel mirrored along every axis (a true convolution), where the direct sum FSL uses for smaller kernels or larger images applies it as stored (a correlation): for a kernel that is not point-symmetric, or of an even size, the result depends on the kernel's and the image's sizes";
    FSL_193 = "FSL-193", "fslmaths", true,
        "-roc with a curve file that cannot be written (a folder that does not exist, no permission) writes no curve and says nothing: the run goes on and exits 0";
    FSL_201 = "FSL-201", "topup", true,
        "with --estmov=1, the derivatives of the predicted volumes with respect to the rotations are forward differences (a step of 1e-5 rad) of rotation matrices built with a single-precision angle, cosine and sine and an axis entry slightly different from 1: their rounding, divided by the small step, adds a false scaling along the rotation's axis and errors of about 1 % to the matrix' derivative, so the gradient and the Gauss-Newton Hessian of the movements are not the cost's (the rotations' entries 0.4 to 40 % off on real images) and the minimisers end where this false gradient vanishes rather than at the cost's minimum";
    FSL_202 = "FSL-202", "fast", true,
        "with one channel, each class's variance (in the log domain) subtracts the single-precision square of the single-precision mean from the mean of the squares: the two nearly cancel, so the variances of classes with a small spread (white matter) are off by up to about 0.3 % (a few % for intensities above 2^24), which moves the labels and partial volumes (up to hundreds of voxels from those of the exactly computed statistics)";
    FSL_203 = "FSL-203", "fast", true,
        "an input whose first voxel is NaN has a NaN minimum and maximum (they start from the first voxel and NaN never replaces them): the negative values are not cleared and, with one channel, the brain mask (every voxel between 0 and the maximum + 1) is empty, so FAST stops with an exception (exit 255) or segments nothing";
    FSL_204 = "FSL-204", "pointflirt", true,
        "with two coordinate files that are both empty or missing (no points), pointflirt crashes (a segmentation fault) after its messages instead of saying that there are no points to fit";
    FSL_211 = "FSL-211", "standard_space_roi", true,
        "the script expands its arguments unquoted: a name with a blank is split into words (an input, reference or mask with a blank is reported not valid, an output with a blank writes and removes other files, an option with a blank ends the options with a message of the shell), and FLIRT's options given after the script's are split again";
    FSL_212 = "FSL-212", "standard_space_roi", true,
        "the cleanup removes every file whose name starts with <output>_tmp, also files the run did not create (a missing one makes rm fail and the script exit 1)";
    FSL_213 = "FSL-213", "standard_space_roi", true,
        "a variable maskthr in the environment is applied, as fslmaths options, to the standard-space mask transformed into the input's space (the script sets it only for -b)";
    FSL_214 = "FSL-214", "standard_space_roi", true,
        "a program that fails (FLIRT refusing an option, an image FSL cannot read or write) does not stop the script: the next steps run on what is missing, and the script exits with the status of its cleanup, 0 when nothing else went wrong, mostly without an output";
    FSL_215 = "FSL-215", "fslroi", true,
        "fslroi exits with status 0 after the errors it reports (an image that cannot be read, an invalid region, a time index outside the series), without writing an output";
    FSL_216 = "FSL-216", "bet", true,
        "bet -S computes its eye threshold (3.5 standard deviations above the brain's mean) with dc from the numbers fslstats prints, which dc misreads when a number is printed with an exponent (its e and sign are taken as commands), with a minus sign (a subtraction), or as nan or inf (commands): the threshold is wrong or empty";
    FSL_217 = "FSL-217", "fslroi", true,
        "fslroi and fslmerge (and bet -F and -Z, which run them) write an int8 image as uint8 with the same bytes: every negative value becomes another (128 to 255), and bet2 then runs on those values";
    FSL_241 = "FSL-241", "midtrans", true,
        "transforms whose values reach beyond double precision in its averages (a scale of 1e200, a translation of 1e300) make midtrans write rows of NaN, or report its failed singular value decomposition as a matrix-size error, and exit 0, instead of saying that the transforms cannot be averaged";
    FSL_242 = "FSL-242", "topup", true,
        "with --minmet=1 (scaled conjugate gradients), a level whose last iteration's step is rejected (its cost above the current point's) ends at that rejected point, not at the lowest cost the level found: the next level, or the outputs, start from a worse field (and movements) than the level had";
    FSL_261 = "FSL-261", "prelude", true,
        "--removeramps estimates, removes and restores the linear phase ramps without the voxels of the image's last column, row and slice: their differences do not count in the ramps and their phase keeps the ramp while the rest loses it; an image of one slice gets no ramp removed at all";
    FSL_262 = "FSL-262", "prelude", true,
        "with several volumes, --savemask and --labels write each volume's mask and labels to the same file in turn: only the last volume's are kept";
    FSL_263 = "FSL-263", "prelude", true,
        "-c with an image that is not complex takes the file's bytes as single-precision floats (right only for float32 data, without its scaling; other types give other values or are read past their end) and takes every imaginary value from memory that was never written";
    FSL_264 = "FSL-264", "prelude", true,
        "-c with a complex image stored in the other byte order (big-endian) exchanges its real and imaginary parts (each value's 8 bytes are swapped as one), so the phase is wrong";
    FSL_265 = "FSL-265", "prelude", true,
        "-c with a complex128 (double-precision) image reads none of its values: every value comes from memory past the empty data buffer";
    FSL_266 = "FSL-266", "prelude", true,
        "-v with --removeramps writes the ramp-free phase to an image named grot in the working directory, replacing any file of that name";
    FSL_268 = "FSL-268", "prelude", true,
        "NaN values in the magnitude (or mask) image enter the sort of the percentiles that set the default mask threshold (their place undefined), and a NaN first voxel makes the image's maximum NaN, so the mask is empty and the unwrapped phase all zeros";
    FSL_269 = "FSL-269", "prelude", true,
        "NaN phase values inside the mask enter the labels: their constraints are NaN, merged last from the most negative integer as their offset, so a region labelled together with a NaN voxel, or merged into one through a NaN constraint, is moved by about 2^31 cycles (the phase written is of the order of 1e10), and with -s NaN differences between slices are sorted in an undefined order";
    FSL_270 = "FSL-270", "prelude", true,
        "--end before --start processes no volume and writes no unwrapped output, yet exits 0 (the raw phase output, if asked for, is written)";
    FSL_271 = "FSL-271", "prelude", true,
        "a magnitude or mask image of another size than the phase image is not refused: it is read by the phase image's voxel indices, outside its own extent by its extrapolation, so the mask does not correspond to the phase";
    FSL_272 = "FSL-272", "prelude", true,
        "the phase range check (more than 3 pi refused) and the rewrap to [-pi, pi] (beyond 1.001 pi) take the image's maximum and minimum from its first value on: a NaN first value (the first voxel of a masked or divided phase map) makes both NaN, so a phase from 0 to 2 pi is not rewrapped and is unwrapped with whole cycles wrong in some regions, and a phase of any range is not refused";
    FSL_273 = "FSL-273", "applytopup", true,
        "--method=jac divides the field's derivatives by the field's own voxel sizes (a coefficient field's intent parameters, a field image's header) while the displacement it applies is laid at the images' voxel indices in the images' voxel sizes: a field of the images' shape whose voxel sizes differ from the images' gets a Jacobian determinant scaled along each phase-encoded axis by the ratio of the voxel sizes, not the Jacobian of the displacement applied, so the intensities are modulated wrongly";
    ITK_50 = "ITK-50", "ITK", true,
        "KdTreeBasedKmeansEstimator never counts the point a k-d tree node is split at when it divides the node between centres: those points are left out of the pass's new centres";
    ITK_51 = "ITK-51", "ITK", true,
        "TDistribution's density divides Gamma((n+1)/2) by Gamma(n/2), both infinite above about 340 degrees of freedom: the density and InverseCDF are NaN, so GrubbsRosnerListSampleFilter finds no outlier in a sample of more than about 342 values";
    ANTS_90 = "ANTS-90", "Atropos", true,
        "KMeans with several intensity images starts its second k-means (on every image) from the mask's mean instead of the first k-means' class means, and then keeps the first k-means' labels: only the first image decides the initial labelling";
    ANTS_91 = "ANTS-91", "Atropos", true,
        "Otsu finds the first image's range from FLT_MIN with else-if, so the histogram can miss the brightest voxel and, for negative images, extends to 0";
    ANTS_92 = "ANTS-92", "Atropos", true,
        "Random labels every voxel, outside the mask too, and those voxels enter the first estimates of the classes";
    ANTS_93 = "ANTS-93", "Atropos", true,
        "the adaptive smoothing weights of several -a options are applied in reverse order (the first image gets the last option's weight)";
    ANTS_94 = "ANTS-94", "Atropos", true,
        "the outlier filters keep their previous output: BoxPlot computes its replacement values from a histogram that also holds the previous call's values, and GrubbsRosner gives the second and later images the first image's values";
    ANTS_95 = "ANTS-95", "Atropos", true,
        "ManifoldParzenWindows evaluates every class without its posterior weights, so all classes have the same likelihood up to a constant";
    ANTS_96 = "ANTS-96", "Atropos", true,
        "partial-volume classes are never re-estimated: their Gaussians keep the initial labelling's means";
    ANTS_97 = "ANTS-97", "Atropos", true,
        "with --minimize-memory-usage the prior probability images are stored with their voxel numbers in float, so in images of more than 2^24 voxels prior values land on rounded voxel numbers";
    ANTS_98 = "ANTS-98", "Atropos", true,
        "KMeans takes the first image's range from the voxels labelled 1 in the mask only: a mask with other values gives infinite initial centres";
    ANTS_99 = "ANTS-99", "ANTs", true,
        "without -d every image is taken as 3D (Atropos, N4BiasFieldCorrection, DenoiseImage): the program asks the image reader for the dimension before the reader has read the header, and NIfTI's reader answers 3";
    ANTS_100 = "ANTS-100", "Atropos", true,
        "prior probability images given as one vector image (without --minimize-memory-usage) all hold the last component: every class gets the last class's prior";
    ANTS_101 = "ANTS-101", "Atropos", true,
        "-p with a name other than Socrates, Plato, Aristotle or Sigmoid silently keeps Socrates";
    ANTS_102 = "ANTS-102", "Atropos", true,
        "--icm in its documented bracketed form ([async,iterations,codeImage]) aborts: the option's empty name is converted to a bool";
    ANTS_103 = "ANTS-103", "Atropos", true,
        "JointShapeAndOrientationProbability evaluates its histograms at the raw tensor components, at a point whose second coordinate is never set (uninitialized memory)";
    ANTS_104 = "ANTS-104", "Atropos", true,
        "ManifoldParzenWindows' local covariances (a covariance neighbourhood above 0) double the running diagonal sums at every neighbour and add the running upper values to the lower triangle";
    ANTS_105 = "ANTS-105", "Atropos", true,
        "--minimize-memory-usage with prior probability images and a mask crashes: the check of the priors against the mask rebuilds a prior on the first intensity image before any is read";
    ANTS_106 = "ANTS-106", "Atropos", true,
        "partial-volume classes with their likelihoods and an MRF never win a voxel: the MRF compares the tissue labels of each partial-volume set but never the partial-volume label itself, so a partial-volume label costs +1 against every neighbour and Atropos stops";
    ANTS_107 = "ANTS-107", "Atropos", true,
        "with GrubbsRosner, whose output keeps growing (ANTS-94), Atropos weighs the samples beyond the masked voxels with weights it never sets (uninitialized memory): the class parameters depend on what the memory held";
    ITK_60 = "ITK-60", "ITK", true,
        "LandmarkBasedTransformInitializer's 2D rigid fit takes the angle -pi/2 whenever the landmarks' summed dot product is within 5e-5 of 0, whatever the sign of their cross product (a quarter turn the other way gives the opposite rotation)";
    ITK_96 = "ITK-96", "ITK", true,
        "the HDF5 transform writer (with compression) cannot write a transform without parameters (a chunk of 0 values): an IdentityTransform, or the inverse a displacement field without an inverse field gives, stops the file part-written with HDF5's error";
    ITK_97 = "ITK-97", "ITK", true,
        "a sampled metric's derivative at a sample point outside the virtual region (a shrink factor: the samples are drawn on the fixed image) is added at the region's linear offset of its index, another voxel's parameters (StorePointDerivativeResult checks nothing)";
    ITK_100 = "ITK-100", "ITK", true,
        "MattesMutualInformationImageToImageMetricv4 keeps its joint histogram images from a stage's first level, so a later level whose analysis region has one intensity (a bin size of 0) never reaches ImageBase::SetSpacing's exception, as the first level does: the bin size is used silently, the bin indices are NaN or infinite and clamped, the metric is 0";
    ITK_99 = "ITK-99", "ITK", true,
        "a sampled metric's derivative at a sample point outside the virtual region whose linear offset falls outside the derivative array is written past its end: memory it does not own";
    ITK_98 = "ITK-98", "ITK", true,
        "B-spline interpolation mirrors a support index only once at each end of an axis: on an axis too short for the spline's support (2 voxels for orders 3 and 4, 2 or 3 for order 5) it reads the coefficient image outside its buffer, memory it never wrote";
    ITK_103 = "ITK-103", "VXL", false,
        "VXL's L-BFGS (netlib's lbfgs translated by f2c) steps its line search's first stage from the unmodified function values where Nocedal's Fortran passes the modified ones (FXM, FYM), and then rebuilds them from stale values";
    ITK_104 = "ITK-104", "VXL", true,
        "VXL's L-BFGS resets ys to 0 on every call (a local of the C function, saved in the Fortran), so a caller that supplies the diagonal (DIAGCO) gets an infinite 1/ys";
    ITK_105 = "ITK-105", "VXL", true,
        "VXL's L-BFGS prints its vectors (IPRINT(2) of 1 and above) scaled from the signed value that last exceeded the running maximum in magnitude rather than from the largest magnitude, and divides a vector of zeros by 0";
    ITK_106 = "ITK-106", "VXL", true,
        "VXL's L-BFGS prints its vectors X and G (IPRINT(2) of 1 and above) from the double before each array, memory it does not own, and drops their last element (lb1 passes write50 the pointers it decremented for Fortran's indexing)";
    ITK_107 = "ITK-107", "VXL", true,
        "vnl_inverse of a 2 x 2, 3 x 3 or 4 x 4 matrix whose determinant is 0 returns an uninitialised vnl_matrix_fixed (its assertion is compiled out): memory contents";
    ITK_108 = "ITK-108", "VXL", true,
        "vnl_cholesky never initialises rcond_ and LINPACK's dpoco leaves it unset for a matrix that is not positive definite: rcond(), and the estimate_condition mode's message, give memory contents";
    ITK_109 = "ITK-109", "VXL", true,
        "vnl_cholesky::determinant (LINPACK's dpodi) keeps its mantissa in [1, 10) with loops that never end when it is not finite: a factor with an infinite or NaN diagonal entry, or one whose square overflows the mantissa";
    ITK_110 = "ITK-110", "VXL", true,
        "vnl_real_eigensystem never writes Vreal's columns of a complex eigenvalue pair (the matrix is allocated uninitialised): those columns are memory contents";
    ITK_111 = "ITK-111", "VXL", true,
        "when EISPACK's hqr2 fails (30 n iterations), vnl_real_eigensystem reads the eigenvalues it had not found from uninitialised vectors: D and the columns built from them depend on memory contents";
    ITK_112 = "ITK-112", "VXL", true,
        "EISPACK's balanc (vnl_real_eigensystem) never returns on a matrix with an infinite or NaN entry, or entries so large that a row's or column's scaled sum overflows, in the part it balances";
    ITK_113 = "ITK-113", "VXL", true,
        "vnl_qr::solve (and inverse) of a matrix with one row whose entry is 0 never writes the solution (LINPACK's info is 1 and VNL's vector is uninitialised): memory contents";
    ITK_114 = "ITK-114", "VXL", true,
        "vnl_brent_minimizer (and vnl_brent and vnl_powell, which use it) can loop for ever, coming back to the same points: on a bracket with an infinite end where the cost is NaN or not lower than at the best point (the golden-section step lands on that end again and again), or whose ends are so large that their sum overflows (the midpoint is infinite, the stopping test never passes, two points take turns)";
    ITK_115 = "ITK-115", "VXL", true,
        "vnl_symmetric_eigensystem never returns on many matrices with an infinite or NaN entry: EISPACK's pythag loops for ever when its second argument is NaN or both are infinite (its iteration starts from a NaN ratio and never meets its stopping test), and tql2 makes such calls, once after reading and writing one element past its arrays (when a NaN is left at the end of the subdiagonal)";
    ITK_116 = "ITK-116", "VXL", true,
        "vnl_svd (LINPACK's svdc) can loop for ever on a matrix with an infinite or NaN entry: its deflation and splitting steps do not count towards the iteration limit, and they can come back to the same singular values and superdiagonal again and again";
    ITK_117 = "ITK-117", "VXL", true,
        "vnl_real_eigensystem takes an eigenvalue whose imaginary part is not 0 for the first of a complex pair; when the last eigenvalue's imaginary part is NaN it reads the eigenvector's imaginary part from one row past its eigenvector matrix and writes the conjugate one element past its eigenvalues: memory contents, and the program can crash";
    ANTS_110 = "ANTS-110", "antsAI", true,
        "the blob alignment keeps the blob pairs whose displacements have about the same length as the others' (the squared distance between a pair's fixed and moving blob over another pair's), not those whose distances to the other blobs are preserved between the images, as its comment and ImageMath's MatchBlobs say";
    ANTS_111 = "ANTS-111", "antsAI", true,
        "once no blob pair with a positive patch correlation is left, the pairing takes the first fixed and the first moving blob again at every step (an uncorrelated and repeated pair) until it has as many pairs as asked or has tried every fixed blob";
    ANTS_112 = "ANTS-112", "antsAI", true,
        "the blob detector does not keep the N strongest maxima: after each scale it drops every later maximum below the weakest one kept even while fewer than N are kept, and when a heap has just filled it lets a weaker maximum push out the heap's weakest one";
    ANTS_114 = "ANTS-114", "antsAI", true,
        "a rigid or similarity blob alignment that keeps no landmark writes a transform of NaNs (ITK's rigid landmark fits divide the landmark sums by their count, 0)";
    ANTS_116 = "ANTS-116", "antsAI", true,
        "the blob alignment compares two blobs' patches after turning the moving one with a rotation built from the patches' principal axes, whose signs are arbitrary, without checking its determinant: identical patches can be compared reflected or turned by 180 degrees";
    ANTS_160 = "ANTS-160", "ResampleImage", true,
        "Gaussian interpolation given fewer sigmas than dimensions reads the missing sigmas past the end of their list: the result depends on memory the program never wrote";
    ANTS_162 = "ANTS-162", "ANTs", true,
        "N4BiasFieldCorrection's B-spline lattice and padding, and ResampleImage's and ResampleImageBySpacing's output grids, are sized from the image's physical extent with no check: a header in metres or micrometres, or a 4D image's time axis, can make a tiny input need more memory than any machine has, which fills the machine until the process is stopped";
    FS_01 = "FS-01", "FreeSurfer", true,
        "the error messages most of FreeSurfer's library prints (its C-style errors: ErrorPrintf, ErrorExit, ErrorReturn) end with a NUL byte before their newline, on standard error and in the error log, so recon-all.log and other logs that capture them read as binary files to grep and text tools";
    FS_02 = "FS-02", "FreeSurfer", true,
        "a PROFILE environment variable beyond the 32-bit integer range, or NaN, sets the profiling level -2147483648";
    FS_03 = "FS-03", "FreeSurfer", true,
        "a failed internal assertion ends the program with a segmentation fault (exit status 139) after its message; with FREESURFER_HANG_ON_ASSERT set it first waits for ever";
    FS_04 = "FS-04", "FreeSurfer", true,
        "a program that draws random numbers before setting a seed (or after setting seed 0), with FREESURFER_SEED unset, takes its seed from the clock: no two runs agree";
    FS_05 = "FS-05", "FreeSurfer", true,
        "a FREESURFER_SEED that is not an integer, or is beyond a 64-bit integer, ends the program with an abort (exit status 134, after the C++ library's terminate message) at its first random number";
    FS_06 = "FS-06", "FreeSurfer", true,
        "the fast arctangent (fastApproxAtan2f) returns memory contents, or crashes, when an argument is NaN, both are infinite, or 100,000 times the smaller magnitude overflows a float";
    FS_07 = "FS-07", "FreeSurfer", true,
        "the --version and --all-info handling skips the argument that follows each option it removes and examines the last arguments again: a second version option can be missed, another handled twice";
    FS_08 = "FS-08", "FreeSurfer", true,
        "FSENVgetenv returns memory contents for the b-vector space when FS_DESIRED_BVEC_SPACE holds no number, and for the working directory when it cannot be read";
    FS_09 = "FS-09", "FreeSurfer", true,
        "the median (and quick_select) of an empty list returns memory contents";
    FS_10 = "FS-10", "FreeSurfer", true,
        "the unique-integer helpers (count, list and most frequent value of a list) given an empty list report one item, whose value is memory contents";
    FS_11 = "FS-11", "FreeSurfer", true,
        "the case-insensitive string comparison (stricmp) and the file-name extension helper crash or corrupt memory when given a string of more than 1,023 bytes";
    FS_12 = "FS-12", "FreeSurfer", true,
        "nint and nint2 (FreeSurfer's rounding to an integer) give -2147483648 for values beyond the 32-bit integer range and for NaN";
    FS_13 = "FS-13", "FreeSurfer", true,
        "MatrixAlloc of a negative size, or of more numbers than an int holds: for some negative sizes an empty matrix (whose stored numbers can show the matrix's own record, two memory addresses included), for the others a crash, or (with the old allocator) a message that it uses an mmap'd file and an exit";
    FS_14 = "FS-14", "FreeSurfer", true,
        "MatrixCopy of a complex matrix copies, of each row, only as many floats as the row has columns, starting one float before the row's first pair, into the same places of the copy: half the values are missing and the rest are misplaced";
    FS_15 = "FS-15", "FreeSurfer", true,
        "MatrixMultiply and MatrixMultiplyD of complex matrices add the product onto the output's earlier contents: a given output, or one that is also an input, gives a wrong product";
    FS_16 = "FS-16", "FreeSurfer", true,
        "matrix functions given an output whose type or size differs from the result's (a complex product into a real matrix) write outside the output: memory the program does not own";
    FS_17 = "FS-17", "FreeSurfer", true,
        "FreeSurfer's matrices and histograms give -2147483648 where a float or double beyond the 32-bit integer range, or NaN, becomes an integer (bin numbers, counts compared as integers, the floor of a vector)";
    FS_18 = "FS-18", "FreeSurfer", true,
        "HISTOclearCounts resets only the first count of the histogram (and, making a new histogram, copies only the first bin's value): the counts it is meant to reset keep accumulating";
    FS_19 = "FS-19", "FreeSurfer", true,
        "HISTOaddSample places a sample outside the histogram's range by the range's lower intensity: below a negative lower intensity the sample is dropped, above the range with a lower intensity beyond the bin count it is counted in the first bin; a NaN sample is counted in the first bin";
    FS_20 = "FS-20", "FreeSurfer", true,
        "HISTOaddSample gives a sample exactly at a bin's value the weight 0 in both neighbouring bins, so it is not counted at all (with a bin size of 1 and integer intensities, no sample is counted)";
    FS_21 = "FS-21", "FreeSurfer", true,
        "HISTOcopy, HISTOadd, HISTOsubtract and HISTOcumsum given an output histogram with fewer bins allocated than the result write past its arrays: memory the program does not own";
    FS_22 = "FS-22", "FreeSurfer", true,
        "FreeSurfer's histogram searches and helpers read and write outside the histogram's bins when given a start bin outside it (-1 included), when a peak window reaches past an end, and for histograms of 0 or 1 bins: memory the program does not own";
    FS_23 = "FS-23", "FreeSurfer", true,
        "HISTOmakePDF given an output histogram other than its source normalises the output's own counts instead of the source's";
    FS_24 = "FS-24", "FreeSurfer", true,
        "HISTOrobustGaussianFit of a histogram of one bin never returns";
    FS_25 = "FS-25", "FreeSurfer", true,
        "FreeSurfer's real-only matrix operations (identity, transpose, sums, differences, scalar products, squares, diagonals, constants, reshapes, traces, element sums, symmetry and zero tests, column norms, vector lengths and products) give a complex matrix the results of a real matrix of its numbers, and MatrixClear clears only part of a complex matrix: wrong values, no message";
    FS_26 = "FS-26", "FreeSurfer", true,
        "MatrixCopyRealRegion into a complex destination copies the source's imaginary parts instead of its real parts";
    FS_27 = "FS-27", "FreeSurfer", true,
        "MatrixCopyRealRegion and MatrixCopyImagRegion of a real source take each value from the next element (the last one from beyond the array: memory contents), and MatrixCopyRegion between a real and a complex matrix copies the wrong numbers";
    FS_28 = "FS-28", "FreeSurfer", true,
        "matrix functions that fill a caller's output do not check that it fits (the region copies' destination region, MatrixTranspose's and MatrixRealToComplex's output, MatrixIdentity's given matrix, MatrixDiag's and MatrixNormalizeColScale's): elements land in other rows or outside the array";
    FS_29 = "FS-29", "FreeSurfer", true,
        "PDFtodSeed's seed comes from the clock (seconds plus microseconds): a run that takes its seed from it when no seed is given never repeats";
    FS_30 = "FS-30", "FreeSurfer", true,
        "PDFgaussian returns NaN (zero divided by zero) when its two uniform draws are both exactly 0.5";
    FS_32 = "FS-32", "FreeSurfer", true,
        "GetClosestPrimeFactor and GetClosestPrimeFactorLess of 1, which has no prime factor, return memory contents (0 in practice) as the factor";
    FS_33 = "FS-33", "FreeSurfer", true,
        "MatrixInverse and MatrixDeterminant of a 0 x 0 matrix read memory that was never allocated (the inverse also writes 16 numbers past its output; the determinant returns memory contents), and MatrixInverse of a singular complex matrix crashes";
    FS_31 = "FS-31", "FreeSurfer", true,
        "FreeSurfer's vector functions do not check their vectors' lengths (VectorCrossProduct and VectorTripleProduct accept column vectors of one or two elements and never check the other vectors; VectorDot and VectorDistance take the second vector to the first's length): values are read from outside the vectors";
    FS_34 = "FS-34", "FreeSurfer", true,
        "OpenPowell and OpenPowell2 called with no initial directions crash at the end of the minimization";
    FS_35 = "FS-35", "FreeSurfer", false,
        "OpenPowell and OpenPowell2 never report too many iterations: at the iteration limit OpenPowell does not end the program, and OpenPowell2 neither prints its message nor returns 1 (kept: FreeSurfer's change keeps programs from exiting at the limit)";
    FS_36 = "FS-36", "FreeSurfer", false,
        "OpenPowell's tolerance argument has no effect: every call stops at a relative tolerance of 1e-10 on the cost (kept: the programs' results depend on it)";
    FS_37 = "FS-37", "FreeSurfer", true,
        "OpenDFPMin's L-BFGS passes never take the starting point as the best one: when no evaluation improves on it, a successful pass gives empty parameters (OpenDFPMin then crashes) and a failed one the line search's last trial point with the previous pass's end value; and the end value a pass reports is the last one evaluated, which can be a worse point's";
    FS_38 = "FS-38", "FreeSurfer", true,
        "OpenDFPMin repeats its minimization while the cost is better than the caller's starting value by more than the tolerance, never comparing with the previous pass's, so it repeats until a pass fails or crashes (FS-37)";
    FS_39 = "FS-39", "FreeSurfer", true,
        "OpenDFPMin returns FAILED_USER_REQUEST without writing the parameters, the iteration count or the final value when an L-BFGS pass reaches its evaluation limit: the progress made is lost and the caller's count is left unset";
    FS_40 = "FS-40", "FreeSurfer", true,
        "MatrixSVDEigenValues and MatrixConditionNumber of a matrix with more rows than columns read values past the singular values (memory the program does not own), so the eigenvalues and the condition number (1e8 where that memory holds 0) are arbitrary";
    FS_41 = "FS-41", "FreeSurfer", true,
        "OpenNonSymmetricEigenSystem of a non-symmetric matrix of about a thousand rows or more crashes under the default 8 MiB stack limit";
    FS_42 = "FS-42", "FreeSurfer", true,
        "OpenEigenSystem reports success with eigenvectors that are memory contents when the eigensystem does not converge (as for a matrix with NaN entries)";
    FS_43 = "FS-43", "FreeSurfer", true,
        "MatrixSVD gives V as many rows and columns as the matrix has rows where the decomposition's V has as many as its columns (with more rows than columns V's numbers land in the wrong places), and a failed decomposition (fewer rows than columns, or no convergence) leaves V unchanged (zeros for a new one), with no message";
    FS_44 = "FS-44", "FreeSurfer", true,
        "MatrixSVDInverse of a matrix with more rows than columns crashes or ends the program";
    FS_45 = "FS-45", "FreeSurfer", true,
        "MatrixPseudoInverse of a matrix with fewer rows than columns crashes for a zero matrix and for a caller's output it cannot fill (one of the other shape), and given an output of the transposed shape it fills it, frees it and returns a new matrix";
    FS_46 = "FS-46", "FreeSurfer", true,
        "MatrixSVDPseudoInverse never writes a given output (it returns a new matrix), returns a matrix of NaNs when the decomposition fails, and crashes for a zero matrix with fewer rows than columns";
    FS_47 = "FS-47", "FreeSurfer", true,
        "MatrixEigenSystem of a symmetric matrix with infinite entries, and MatrixCalculateEigenSystemHelper's symmetric case of a matrix with infinite or NaN entries in its upper triangle, never return on many such matrices (VNL's ITK-115), and on the others return the values VNL computes, though such a matrix has no eigensystem";
    FS_48 = "FS-48", "FreeSurfer", true,
        "the SVD functions (MatrixSVD, the SVD inverses and pseudo-inverses, the condition numbers) never return for some matrices with infinite or NaN entries";
    FS_49 = "FS-49", "FreeSurfer", true,
        "freadInt and freadDouble at the end of a file keep, for the bytes they could not read, bytes of the number read before them (memory contents where none was read just before; freadInt prints nothing): MatrixReadFrom of a file that ends early fills the matrix's type, sizes or elements with them, and so do an old-style transform tag's length and a binary colour table's numbers cut by the end of the file";
    FS_50 = "FS-50", "FreeSurfer", true,
        "HISTOabs of a histogram whose absolute bin values span an infinite range, or whose bin size is 0 or NaN, ends the program with 'could not allocate histogram' (the bin count it needs is beyond the int range)";
    FS_51 = "FS-51", "FreeSurfer", true,
        "HISTOrobustGaussianFit never returns for an infinite bin size, or one so small that a tenth of it rounds to 0";
    FS_52 = "FS-52", "FreeSurfer", true,
        "MRIgetVoxVal and MRIsetVoxVal do not check that a voxel index lies in the volume (but for negative columns, rows and slices when reading): a column, row, slice or frame beyond the volume reads or writes another voxel of it (a column past the end of a row is the next row's first voxel), and writing at a negative index writes an earlier voxel";
    FS_53 = "FS-53", "FreeSurfer", true,
        "MRIgetVoxVal and MRIsetVoxVal at an index beyond the volume's whole buffer read memory contents or write outside the volume (a crash or another object's corruption), and so do writes and complex-voxel accesses at a row or slice index outside the volume";
    FS_54 = "FS-54", "FreeSurfer", true,
        "MRIsetVoxVal of an MRI_LONG volume rounds the value to an int (-2147483648 beyond the int range) and also stores it as a 4-byte int into half of another voxel of the row (the voxel at half its column), corrupting that voxel";
    FS_55 = "FS-55", "FreeSurfer", true,
        "a volume of a type MRIsizeof does not size (MRI_BITMAP, MRI_DOUBLE_COMPLEX, unknown type codes) is not refused as an unsupported type: its allocation fails with 'could not allocate memory for slice 1 out of N' (or, with no voxels, succeeds with impossible sizes)";
    FS_56 = "FS-56", "FreeSurfer", true,
        "MRIcopyHeader onto a volume without a registration matrix does not copy the source's: the copy has none";
    FS_57 = "FS-57", "FreeSurfer", true,
        "CTABdeepCopy does not copy a colour table's idbase (the copy has 0) nor its entries' voxel counts (memory contents)";
    FS_58 = "FS-58", "FreeSurfer", true,
        "reading or writing a voxel of a volume made without voxels (MRIallocHeader, a header read alone) crashes, but for reads at negative indices (outside_val) and the types without a value (tensor, bitmap, double complex: -1e10 read, nothing written, as with voxels)";
    FS_59 = "FS-59", "FreeSurfer", true,
        "assigning a geometry (VOL_GEOM) that has no cached voxel-to-RAS matrices onto one that has them keeps the destination's old matrices, computed from its old geometry";
    FS_60 = "FS-60", "FreeSurfer", true,
        "MRIcopyHeader onto a volume that owns a transform, from a volume without one, frees that transform yet the destination still shows it as its own: its matrices are then freed memory (using them, or the volume's end, crashes or reads garbage)";
    FS_71 = "FS-71", "MINC volume_io", true,
        "a compressed input (.gz, .Z, .z, or a missing name whose compressed file exists) is decompressed by running gunzip -c, then bunzip2 -c, through the shell with the file name unquoted: a name with a blank or shell syntax fails, reads other files or runs commands";
    FS_72 = "FS-72", "MINC volume_io", true,
        "a thin-plate spline's Points or Displacements list that reaches the end of the .xfm file after white space without its ';' is read forever (the same pushed-back character each time), the list growing until memory runs out";
    FS_73 = "FS-73", "MINC volume_io", true,
        "a thin-plate spline's Number_Dimensions of 0 divides by zero (the program crashes) and a negative one sizes its arrays negatively";
    FS_74 = "FS-74", "MINC volume_io", true,
        "a thin-plate spline with an empty Points list frees a pointer that was never set (a crash or heap damage), and the writer writes such a list with no ';', so the file cannot be read back";
    FS_75 = "FS-75", "MINC volume_io", true,
        "a file name that starts with a $VAR (or ~) whose value is empty writes through a null pointer (expand_filename has not allocated its result yet): the program crashes";
    FS_76 = "FS-76", "nifticlib", true,
        "znzread or znzwrite on a compressed file with a member size of 0 divides by zero (nmemb - remain/size): a crash, or whatever the compiled code makes of it";
    FS_77 = "FS-77", "nifticlib", true,
        "znzread on a compressed file that ends inside a member counts that member as read (nmemb - remain/size rounds the missing members down): a caller checking the count takes a short read for a complete one";
    FS_78 = "FS-78", "zlib 1.2.11", true,
        "gzseek forward (or gzflush or gzclose after it) before the first gzwrite fills the gap with memory zlib never set instead of zeros: gz_zero clears its buffer once, before allocating it";
    FS_79 = "FS-79", "MINC volume_io", true,
        "a backslash in a file name is an escape that expand_filename drops, and only '/' separates directories: a Windows path (C:\\Users\\...) loses its separators, its directory, base name and extension are misread, and a file next to it is never found";
    FS_80 = "FS-80", "gifticlib", true,
        "a label table whose Labels carry colours for some labels only leaves holes in its colour array (or makes it shorter than the table), and a Label whose Key attribute is missing or wrong is given no key: the writer, the validity check and the display read memory never written";
    FS_81 = "FS-81", "gifticlib", true,
        "Base64 data whose character data hold characters outside the alphabet (skipped and counted, the default) take the bytes carried to the next piece from the wrong place when a group of four is cut at the piece's end: the values after it are decoded from earlier text or from memory never written";
    FS_82 = "FS-82", "gifticlib", true,
        "NIFTI_TYPE_FLOAT128 is 6 bytes per value in gifticlib's type table (a long double takes 16): its arrays are allocated, decoded and written with 6 bytes per value, and the ASCII writer reads 16 per value, past the data";
    FS_83 = "FS-83", "gifticlib", true,
        "the ASCII decoder's count of a value's pieces (COMPLEX64, COMPLEX128, RGB24) persists from one array to the next, even of another file: after an array whose last value is incomplete the next array of that type starts in the middle of a value";
    FS_84 = "FS-84", "gifticlib", true,
        "a metadata entry without a name (an MD whose Name has no text) is compared through a null pointer by gifti_get_meta_value and, at verbosity 2 and below, gifti_add_to_meta: the program crashes";
    FS_85 = "FS-85", "gifticlib", true,
        "a GIFTI element outside the element it belongs to (Data, MatrixData, a coordinate system outside a DataArray, a Name or Value outside an MD, a dimension past Dim7 when writing) is read or written through a null or out-of-range pointer: the program crashes or reads memory it never wrote";
    FS_86 = "FS-86", "gifticlib", true,
        "character data (white space included) after a CDATA section or a child element inside a GIFTI Name, Value, Label, DataSpace or TransformedSpace, or inside one of them outside its parent, is written through a null pointer: the program crashes";
    FS_88 = "FS-88", "expat", true,
        "a UTF-16 document of an odd length whose last token is cut by its odd byte (white space, a name or a ']' at the end of the prolog or the content) is converted past its end: the program crashes or never returns";
    FS_89 = "FS-89", "expat", true,
        "an error reported inside a multibyte character (a non-ASCII character in an XML declaration's pseudo-attribute) makes the line count step over the error position and read past the document: the program crashes";
    FS_87 = "FS-87", "gifticlib", true,
        "at verbosity 6 and 7 the GIFTI reader's messages about a MatrixData buffer show the Data element's buffer and length instead, including memory never written";
    FS_90 = "FS-90", "CTN", true,
        "a condition message of 1,024 bytes or more (a long file name in it) overflows the buffer it is formatted in, and a file name of 1,024 bytes or more overflows the object's copy of it: the program crashes or its memory is corrupted";
    FS_91 = "FS-91", "CTN", true,
        "an element of undefined length that is neither a sequence nor pixel data (a sequence the 2002 dictionary types otherwise, a stray item) is read with a count of -1, which fails, and the failure is taken for a complete read: the element is kept with a value never written, the remaining length grows by one and the value's bytes are read as the next elements (an item's delimiter there ends the data set early)";
    FS_92 = "FS-92", "CTN", true,
        "Bits Allocated, Pixel Representation, the file meta group's length, the transfer syntax and the Waveform Data Representation are read from their values whatever their length and representation: a shorter value, a sequence or a transfer syntax without its zero byte is read past its end, from memory never written";
    FS_93 = "FS-93", "CTN", true,
        "a group length element shorter than 4 bytes is given the group's 4-byte length, and a Waveform Data Representation longer than 16 bytes is copied into a 17-byte field: both write past their memory";
    FS_94 = "FS-94", "CTN (FreeSurfer's copy)", true,
        "FreeSurfer's copy swaps the 4-byte integers (UL, SL) of big-endian data in 2-byte pairs instead of reversing them: every such value read from a big-endian file is wrong";
    FS_95 = "FS-95", "CTN", true,
        "a UL or SL value of big-endian data whose length is not a multiple of 4 is swapped 4 bytes at a time past its end, its 32-bit count wrapping: the program crashes";
    FS_96 = "FS-96", "CTN", true,
        "FD and FL values (64- and 32-bit floating point) of big-endian data are left in the file's byte order: every such value read from a big-endian file is wrong";
    FS_97 = "FS-97", "CTN", true,
        "sequences are read by recursion, one level per nesting: a file nesting them deeply enough exhausts the stack and the program crashes";
    FS_98 = "FS-98", "CTN", true,
        "a fragment of encapsulated pixel data whose length is odd (or over the remaining count) closes the object, and the reader then writes through the closed object: the program crashes";
    FS_99 = "FS-99", "nlohmann/json", true,
        "the pretty printer's indent string (512 characters) is doubled once, with spaces, when an indent passes it (past 512 characters a tab indent becomes spaces; an indent more than twice the string's length is read past its memory), and the writer and the value's destructor recurse once per nesting level: a value nested deeply enough exhausts the stack";
    FS_281 = "FS-281", "miniz 1.15", true,
        "a deflate stream that ends early, read with no more input to come (the zlib stream API's MZ_FINISH, mz_uncompress), is padded with zero bytes, which are decoded as if they were the stream's: a truncated stream can be accepted with bytes it never held";
    FS_282 = "FS-282", "miniz 1.15", true,
        "the length symbols 286 and 287 and the distance symbols 30 and 31, which deflate does not define, are accepted: such a length copies 3 bytes and moves the output by 3 (past the end of the buffer when fewer than 3 bytes are left), such a distance copies the byte under the cursor";
    FS_283 = "FS-283", "miniz 1.15", true,
        "a Huffman code with no codes or a single code is accepted whatever its length, and a bit pattern it gives no symbol to is decoded as symbol 0 (from the lookup table without taking any bits)";
    FS_284 = "FS-284", "miniz 1.15", true,
        "in the zlib-style stream API, a match farther back than the output so far (or of distance 0 before 32 KiB were output) copies from the 32 KiB dictionary where nothing was written yet: memory never written";
    FS_100 = "FS-100", "FreeSurfer", true,
        "the gamma draw converts its shape's integer part to a 32-bit unsigned integer, which C leaves undefined outside [0, 2^32): a shape of 2^32 or more gives a value of the wrong size, NaN and +inf the value of a shape below 1, and a negative shape or -inf a draw that does not return in practice";
    FS_101 = "FS-101", "FreeSurfer", true,
        "a seed whose 24 seeded values are all 0 (2147483563, -2147483563 and other multiples of 2147483563) makes the random generator give 0 for ever (its initial carry is 0): flat draws are constant and most other draws never return";
    FS_103 = "FS-103", "FreeSurfer", true,
        "the cubic spline's check that the knots increase strictly is false for a NaN knot, so a NaN knot is accepted and the spline's values become NaN";
    FS_111 = "FS-111", "FreeSurfer", true,
        "setting the inflated surface's name (mris_register -inflated) builds the name of its curvature file in a buffer of 1,024 bytes: a name of 1,022 bytes or more ends the program with a buffer-overflow abort (signal 6)";
    FS_112 = "FS-112", "FreeSurfer", true,
        "adding a command line to a surface that already holds 1,000 prints an error and then stores the line past the end of the list, over the count of lines, which becomes a memory address";
    FS_113 = "FS-113", "FreeSurfer", true,
        "scaling a surface by a negative factor (and the scale-then-translate operation with a negative factor) leaves its bounding box upside down: the low end above the high end";
    FS_114 = "FS-114", "FreeSurfer", true,
        "centring a spherical surface that has no vertex never returns: the search for its centre keeps halving its step without end";
    FS_115 = "FS-115", "FreeSurfer", true,
        "restoring the target or layer-IV coordinates of a spherical patch sets its canonical coordinates to the temporary coordinates, not to the restored ones";
    FS_116 = "FS-116", "FreeSurfer", true,
        "reading a face's original area while it is still marked to be computed later prints NYI and crashes the program on purpose (a segmentation fault)";
    FS_117 = "FS-117", "FreeSurfer", true,
        "reallocating a surface to no faces (also a surface that already has none) fails an assertion and ends the program, and reallocating to no vertices a surface with room for none leaves it without its vertex array (or, the next time, without its other per-vertex arrays) and reports a failure";
    FS_118 = "FS-118", "FreeSurfer", true,
        "the progress bar over a surface's vertices (discrete curvatures, the faces' geometric order) ends the program with an arithmetic exception (signal 8) for a surface of fewer than 20 vertices";
    FS_119 = "FS-119", "FreeSurfer", true,
        "column printing of a status text of 16,378 bytes or more (mris_curvature_stats' and mris_calc's messages, which show file and surface names) ends the program with a buffer-overflow abort (signal 6) before anything is printed";
    FS_120 = "FS-120", "FreeSurfer", false,
        "after the default computation of a surface's metric properties (at every surface read and every deformation step) the faces' angles are not updated: they keep the values they had before (0 for a new surface), while the older and the faster computations update them (kept until its effect on mris_sphere's and mris_register's results is measured; the correction is on request)";
    FS_121 = "FS-121", "FreeSurfer", true,
        "the default computation of a surface's metric properties gives a vertex whose normal cannot be computed (its faces' normals sum to no length even after five small random moves, as with NaN or infinite coordinates) a normal of memory contents, and for a plane adds memory contents to the area of every vertex next to a ripped face";
    FS_122 = "FS-122", "FreeSurfer", true,
        "computing the metric properties of a surface with a NaN white or pial coordinate, a NaN white normal or a NaN radius prints, every time, up to 11 lines such as 'no:<vertex>  v->whitex  nan != nan  during count_MRIScomputeMetricProperties_calls:<n>' (and once 'b <file>:335  needed'), as if the value had changed";
    FS_123 = "FS-123", "FreeSurfer", true,
        "the faster computation of the metric properties (FS_FASTER_MP set) adds memory contents to the areas and normals of the vertices next to a ripped face, and takes the bounding box of a surface with no vertex from memory it does not own";
    FS_124 = "FS-124", "FreeSurfer", false,
        "the faster computation of the metric properties (FS_FASTER_MP set) gives a vertex whose normal cannot be computed in five rounds an area of -1 (and an original area of 1 when it had none) and a normal of no length";
    FS_125 = "FS-125", "FreeSurfer", true,
        "the faster computation of the metric properties (FS_FASTER_MP set) with several threads sums the faces' areas in an order that depends on the threads' timing, so the total area can change from run to run in its last bits";
    FS_126 = "FS-126", "FreeSurfer", true,
        "the surface library's distance, area-error, triangle-property, curvature, float and area files take their names in 1,024-byte buffers: a name of 1,024 bytes or more (1,020 or more for the GIFTI alternatives of FS_GII) ends the program with a buffer-overflow abort (signal 6), or is cut to 1,023 bytes after the message '<function>: Truncation on line <n>', so another file is written or read";
    FS_127 = "FS-127", "FreeSurfer", true,
        "MRISreadTriangleProperties reads the vertex and face counts at the start of a triangle-property file as floats where MRISwriteTriangleProperties writes them as integers, so it refuses every file the writer wrote (but one of a surface without vertices) with 'incompatible vertex number in file <file>'";
    FS_128 = "FS-128", "FreeSurfer", true,
        "MRISwriteTriangleProperties writes, for every face, the three angles of the place after the surface's last face (a spare place's, or memory past the faces) instead of each face's own angles";
    FS_129 = "FS-129", "FreeSurfer", true,
        "MRISreadTriangleProperties and MRISwriteTriangleProperties with a surface name whose last dot is its first or second character take the two letters of the hemisphere from memory before the name";
    FS_130 = "FS-130", "FreeSurfer", true,
        "MRISwriteAngleError divides a vertex's running sum of angle differences by three times its face count after every face, so a vertex of more than one face gets less than the mean difference of its angles";
    FS_131 = "FS-131", "FreeSurfer", true,
        "MRISwriteAngleError writes an angle error that is NaN or beyond the 32-bit integer range (in hundredths of a degree) as -32767, as if it were the most negative error";
    FS_132 = "FS-132", "FreeSurfer", true,
        "MRISmaxMarked starts from vertex 0's mark even when vertex 0 is ripped, and reads it for a surface without vertices (a value from memory)";
    FS_133 = "FS-133", "FreeSurfer", true,
        "MRISreadTriangleProperties reads as many faces as a triangle-property file says, even more than the surface has: they go into its spare face places or past its arrays (memory corruption or a crash)";
    FS_134 = "FS-134", "FreeSurfer", true,
        "MRISreadTriangleProperties and MRISwriteTriangleProperties with a surface name of 1,024 bytes or more write its folder's name past a 1,024-byte buffer (the program can end with '*** stack smashing detected ***' or go on with corrupted names)";
    FS_136 = "FS-136", "FreeSurfer", true,
        "mrisReadAsciiCurvatureFile, when it cannot open its file, prints 25 bytes of binary data in place of its own name in the message '<name> could not open file <file>.'";
    FS_137 = "FS-137", "FreeSurfer", true,
        "mrisReadAsciiCurvatureFile with a file of fewer curvature lines than the surface has unripped vertices stops at the file's end and reports success, the remaining vertices keeping the curvatures they had";
    FS_138 = "FS-138", "FreeSurfer", true,
        "MRISreadNewCurvatureIntoArray and MRISreadNewCurvatureVector, when they cannot open their file, end the program with a segmentation fault (signal 11) instead of the message 'MRISreadNewCurvatureIntoArray(<file>): fopen failed'";
    FS_139 = "FS-139", "FreeSurfer", true,
        "MRISreadDecimation with a file that ends, or whose numbers stop, before every vertex has its value takes the missing values as decimated (a binary file) or as the last number read (a text file) and reports success";
    FS_140 = "FS-140", "FreeSurfer", true,
        "MRISreadDecimation with a text file whose vertex count cannot be read compares memory contents with the surface's vertex count";
    FS_141 = "FS-141", "FreeSurfer", true,
        "MRISwriteGeo and MRISwriteICO write an unripped face that has a ripped vertex: MRISwriteGeo writes a vertex number from memory contents for it (and, for a surface of more than 300,000 vertices, writes past a 300,000-entry buffer), MRISwriteICO a number that names another vertex or none";
    FS_142 = "FS-142", "FreeSurfer", true,
        "MRISwriteICO numbers the vertices it writes without the ripped ones but writes each face's vertices by their numbers on the surface plus one, so once a vertex is ripped the faces after it name the wrong vertices";
    FS_143 = "FS-143", "FreeSurfer", true,
        "MRISreadVTK with a vertex whose three coordinates cannot be read gives it the coordinates read before (those of the vertex before it) and goes on, reporting success when the rest of the file can be read";
    FS_144 = "FS-144", "FreeSurfer", true,
        "MRISreadVTK with a first vertex whose coordinates cannot be read gives it coordinates from memory contents";
    FS_145 = "FS-145", "FreeSurfer", true,
        "MRISreadVTK with a file that ends after its POINT_DATA line or before all the lines its FIELD line announces ends the program with a segmentation fault (signal 11)";
    FS_146 = "FS-146", "FreeSurfer", true,
        "MRISreadVTK with a file that ends before a field's values takes the missing values as 0 and reports success";
    FS_147 = "FS-147", "FreeSurfer", true,
        "MRISreadVTK and the ASCII, movie.byu and GIFTI surface readers count a vertex's faces in one byte: a vertex of more than 255 faces gets its count modulo 256";
    FS_148 = "FS-148", "FreeSurfer", true,
        "MRISbuildFileName_read with a name whose first dot is its first character, or its second after an h, and the curvature reader's file-name choice with a name whose first dot is its first or second character, read one or two bytes before the name to decide whether it names a hemisphere";
    FS_149 = "FS-149", "FreeSurfer", true,
        "MRISbuildFileName_read (a name with a slash, or without a hemisphere when FS_POSIX is unset) and __MRISapplyFSGIIwrite (FS_GII set to .gii) write a file name of 1,024 bytes or more past their caller's 1,024-byte buffer";
    FS_150 = "FS-150", "FreeSurfer", true,
        "the ASCII and movie.byu surface readers (and the movie.byu positions reader) given a file with no line that is not blank or a comment end the program with a segmentation fault (signal 11)";
    FS_151 = "FS-151", "FreeSurfer", true,
        "the ASCII surface reader given a first line whose two counts cannot be read, the ASCII and movie.byu surface and position readers given a first vertex whose numbers cannot be read, and the text STL reader given a first vertex or facet line whose numbers cannot be read, take the missing values from memory contents";
    FS_152 = "FS-152", "FreeSurfer", true,
        "the ASCII and movie.byu surface and position readers and the text STL reader given a vertex, face or facet whose numbers cannot be read (the file ends early, or holds something else there) give it the values read before it (the previous vertex's coordinates, the last rip flag read, an ASCII face's vertex 0; in an STL file the numbers of the line before, vertex or facet) and report success";
    FS_153 = "FS-153", "FreeSurfer", true,
        "the movie.byu surface and position readers given a first line with a face count of 0 end the program with an arithmetic exception (signal 8)";
    FS_154 = "FS-154", "FreeSurfer", true,
        "the movie.byu surface reader given a face whose vertex number is not one of the surface's (below 1 or above the vertex count, a number it cannot read, or a last vertex not written negative) changes memory outside the surface's vertex arrays (a crash or corrupted memory later)";
    FS_155 = "FS-155", "FreeSurfer", true,
        "the movie.byu positions reader takes coordinates from a file of another surface (other vertex and face counts) without a message: the first vertices of a larger surface, or of a smaller one the last coordinates repeated";
    FS_156 = "FS-156", "FreeSurfer", true,
        "the vertex-positions reader of a binary triangle file given a negative vertex count returns a surface of that count with no vertices, as if it had read it";
    FS_157 = "FS-157", "FreeSurfer", true,
        "the STL reader given a vertex of more than 255 faces writes its face lists past their allocation (memory corruption or a crash)";
    FS_158 = "FS-158", "FreeSurfer", true,
        "the STL reader given a vertex with a NaN or infinite coordinate (one or three of them) reads memory it does not own to find it: a crash, or a wrong vertex";
    FS_159 = "FS-159", "FreeSurfer", true,
        "the STL reader given a facet of fewer than three vertices ends the program with a segmentation fault (signal 11)";
    FS_160 = "FS-160", "FreeSurfer", true,
        "the STL reader takes a file for binary when its triangle count times 50, wrapped around 2^32, plus 84 is its size, and then reads the triangles past the file's end from memory contents";
    FS_161 = "FS-161", "FreeSurfer", true,
        "MRIcopy of a tensor or complex volume into one of the same type copies only the first width bytes of each row (a quarter or an eighth of its voxels' bytes), leaving the rest of the destination as it was";
    FS_162 = "FS-162", "FreeSurfer", true,
        "MRIcopy from float to short, unsigned short or unsigned char, and from short to unsigned char, wraps values outside the destination type's range around (40000 becomes -25536 in short, -5 becomes 251 in unsigned char) where the rest of FreeSurfer clips them";
    FS_163 = "FS-163", "FreeSurfer", true,
        "MRIcopy into a given volume of another extent or frame count puts the source's rows in other places, or writes past the destination (a crash or another object's corruption)";
    FS_164 = "FS-164", "FreeSurfer", true,
        "MRItkReg2Native of a registration matrix that cannot be inverted (singular) returns the uninverted native product, or a matrix of infinities, with no message";
    FS_165 = "FS-165", "FreeSurfer", true,
        "MRIfixTkReg of a registration matrix with four columns and another number of rows crashes";
    FS_166 = "FS-166", "FreeSurfer", true,
        "MRIcopy of a tensor volume copies every row from the source's first voxel onto the destination's first voxel: only the destination's first row changes, and it becomes the source's first row";
    FS_261 = "FS-261", "FreeSurfer", true,
        "the byte-order swap of a buffer of n items of 2, 4 or 8 bytes (ByteSwapBuf) reverses only the first n/2, n/4 or n/8 items (rounded up) and leaves the rest as they were";
    FS_263 = "FS-263", "FreeSurfer", true,
        "with exceptions switched on, a fatal error with a nonzero code aborts the program (exit status 134, after the C++ library's terminate message) instead of passing the code to the program's handler";
    FS_264 = "FS-264", "FreeSurfer", true,
        "the 4 x 4 matrix helpers (MRImatrixOfDirectionCosines, MRImatrixOfVoxelSizes, MRImatrixOfTranslations, a geometry's toMatrix and RASFromSurfaceRAS_ given a matrix to fill; MRIsetVox2RASFromMatrix given its matrix) given a matrix with fewer than 4 rows or columns write or read past its elements: a crash, memory contents, or other elements of the matrix changed";
    FS_265 = "FS-265", "FreeSurfer", true,
        "MRIvalRangeRegion given no region crashes, where MRIextractRegion takes a missing region as the whole volume";
    FS_266 = "FS-266", "FreeSurfer", true,
        "raising each frame of a volume to its power (mri_framepower) given fewer powers than the volume has frames reads past the powers given: the later frames are raised to whatever memory holds there";
    FS_267 = "FS-267", "FreeSurfer", true,
        "mapping a segmentation onto another grid with a tissue-type vote (MRIaseg2vol with a colour table) whose tissue-type table has no places takes, as the vote's tissue type, the one an earlier vote of the same program chose (0 in its first): a later call's labels depend on the earlier calls";
    FS_268 = "FS-268", "FreeSurfer", true,
        "cropping around an RAS point (MRIcropAroundRAS) gives the fields of view, which it documents in RAS order (left-right, front-back, up-down), to the voxel axes the other way round: for an orientation that moves all three axes (SRA, ASR, PIL, ASL and the like) each axis gets another axis's field of view";
    FS_269 = "FS-269", "FreeSurfer", true,
        "the bounding box of a mask (REGIONgetBoundingBox, REGIONgetBoundingBoxM) is one voxel smaller on each axis than the voxels it should bound: without a pad after the mask (or a negative one) it leaves out the mask's last voxels on that axis, corrected by default for REGIONgetBoundingBox (mri_binarize --bb, mri_mask -BBEQ); with a pad of 1 or more only the far side's last padding voxel, outside the mask, is missing, kept in both modes; REGIONgetBoundingBoxM (mri_mask -bb, -crop, -crop-crs, -BBM) keeps the original's box in both modes for every pad (pipelines rely on it: recon-all's SynthMorph crop and its venous-sinus network's crop, which the corrected box moves by a voxel)";
    FS_270 = "FS-270", "FreeSurfer", true,
        "mapping a segmentation onto another grid with upsampling (MRIaseg2volMU with an upsampling factor) when the segmentation is at neither end of the registration crashes the program after the concatenation's refusal";
    FS_331 = "FS-331", "FreeSurfer", true,
        "RASFromSurfaceRAS_ (and VGtkreg2RAS) given a matrix larger than 4 x 4 to fill computes the tkregister matrix's inverse into it as if it were 4 x 4 and the product into it again: a 4 x N matrix (N > 4) gets the numbers laid out across its rows, the program ends when one of the matrix's first 4 x 4 numbers left unwritten is not finite, and one of more than 4 rows gives nothing after a size-mismatch error";
    FS_332 = "FS-332", "FreeSurfer", true,
        "MRIclone given the source itself as its destination frees the source's colour table before copying it: the volume loses its colour table after an error that no table could be copied";
    FS_167 = "FS-167", "FreeSurfer", true,
        "the binary readers of one number (fread1, fread2, freadLong, freadFloat, and znzread1, znzreadShort, znzreadInt, znzreadLong, znzreadFloat, znzreadDouble) at the end of a file return a number whose missing bytes are memory contents; znzreadInt and znzreadLong keep there the bytes of the number read just before them, as freadInt does (FS-49): a tag's length cut by the end of a compressed or plain volume file, and a binary colour table's numbers in such a file";
    FS_168 = "FS-168", "FreeSurfer", true,
        "fio_dirname of a name of one character (a, or a/ with trailing slashes) can return / instead of ., depending on what the memory allocator left in memory it had just been given back";
    FS_169 = "FS-169", "FreeSurfer", true,
        "fio_DirIsWritable of a directory name of 1,982 bytes or more (1,983 when it names a file) overflows a 2,000-byte buffer: the program aborts with *** buffer overflow detected ***";
    FS_170 = "FS-170", "FreeSurfer", true,
        "TSV::read of an empty file takes its column names from memory contents";
    FS_171 = "FS-171", "FreeSurfer", true,
        "TSV::read of a file whose first line is empty reads, and can change, a byte of memory outside the line it read";
    FS_172 = "FS-172", "FreeSurfer", true,
        "TSV::read of a table with an entry that is not a number stores the previous number (memory contents for the first) in its place and, unless the characters it passes over lead to a number, never returns: it reads empty rows until memory runs out";
    FS_173 = "FS-173", "FreeSurfer", true,
        "TSV::print and TSV::write with a format that does not print exactly one double (a conversion of another type, or more than one) print register or memory contents or crash, and with a format of more than 998 bytes the program aborts with *** buffer overflow detected ***";
    FS_174 = "FS-174", "FreeSurfer", true,
        "an allocation that fails in lcalloc ends the program with exit status 0, as if it had succeeded, after printing Cannot calloc()";
    FS_176 = "FS-176", "FreeSurfer", true,
        "the byte-order swap of a 64-bit integer (swapLong64) and the readers and writers of a 64-bit integer (freadLong, fwriteLong, znzreadLong, znzwriteLong) round it to a double's 53 significant bits: a number swapped or read comes back rounded (-2^63 where the rounding reaches 2^63), and a number written is stored with the low bits of its first bytes changed, whenever its byte-reversed form needs more than 53 bits (most negative numbers, and numbers whose last bytes are not zero)";
    FS_175 = "FS-175", "FreeSurfer", true,
        "byteswapbufshort, byteswapbuffloat and byteswapbufdouble given a byte count that is not a multiple of the number's size reverse the bytes of a whole last number past the count: past the end of a buffer that ends at the count (memory the program does not own)";
    FS_177 = "FS-177", "FreeSurfer", true,
        "TAGskip, znzTAGskip and FStagsIO::skip_tag, when they cannot allocate a tag's data length (a negative length always), end the program with a message that shows the length's low 32 bits as an unsigned number (-1 shows as 4294967295)";
    FS_178 = "FS-178", "FreeSurfer", true,
        "the readers that skip a tag's data (TAGskip, znzTAGskip, FStagsIO::skip_tag, and FStagsIO::read_mri_frames' bytes after the frames) given a tag length beyond the machine's memory end the program, or crash the frames' reader, even where the file holds only a few bytes: the outcome depends on the machine's memory";
    FS_179 = "FS-179", "FreeSurfer", true,
        "the readers of a matrix tag (TAGreadMatrix, znzReadMatrix, FStagsIO::read_matrix) given a text whose first word has 1,600 bytes or more (TAGreadMatrix) or 100 bytes or more (the other two) corrupt the program's memory: a crash or wrong results";
    FS_180 = "FS-180", "FreeSurfer", true,
        "znzReadMatrix and FStagsIO::read_matrix, when the 1,600 bytes of a matrix tag hold no NUL byte (or the file ends inside them and the bytes it gives hold none), read the matrix's text on past them: numbers that reach the end of those bytes continue with memory contents";
    FS_181 = "FS-181", "FreeSurfer", true,
        "the matrix tag writers (TAGwriteMatrix, znzWriteMatrix, FStagsIO::write_matrix in both forms) given a matrix smaller than 4 x 4 write numbers read from outside it";
    FS_182 = "FS-182", "FreeSurfer", true,
        "TAGmakeCommandLineString given a command line longer than the space its caller has for it (4,096 bytes in mri_segment) corrupts the program's memory";
    FS_183 = "FS-183", "FreeSurfer", true,
        "FStagsIO::read_scan_parameters given a tag length below the parameters' 20 bytes, or one beyond the machine's memory, crashes, overwrites the program's memory with the rest of the file, or gives no phase-encoding direction";
    FS_184 = "FS-184", "FreeSurfer", true,
        "the length FStagsIO gives a GCA morph's label tag wraps at 2^31: a volume whose voxel count times 4 reaches 2^31 gets a wrong length";
    FS_185 = "FS-185", "FreeSurfer", true,
        "VOL_GEOM::read of a geometry in FreeSurfer's NIfTI extension given a name length beyond about 8 MiB (Linux's default stack size) crashes the program";
    FS_186 = "FS-186", "FreeSurfer", true,
        "VOL_GEOM::read does not end the name it reads: in a geometry that held a longer name, a name read from FreeSurfer's NIfTI extension (or the end of a file inside it) keeps the earlier name's rest after it, a name of length 0 leaves the earlier name, and a name of 512 bytes or more read from an .mgz continues with the earlier name's bytes after its first 512";
    FS_187 = "FS-187", "FreeSurfer", true,
        "the binary colour table readers take a name whose bytes in the file hold no NUL byte (a name length of 0, a name the file ends inside, or 1,023 bytes or more copied into an entry) with memory contents after it";
    FS_188 = "FS-188", "FreeSurfer", true,
        "the first version's binary colour table readers given a structure name length of 1,024 or more write a zero byte past the name: no visible effect below 1,064, another object's memory corrupted beyond";
    FS_189 = "FS-189", "FreeSurfer", true,
        "the second version's binary colour table readers given a structure number at or beyond the table's entry count read and write the table's list of entries outside it (a crash or another object's corruption)";
    FS_190 = "FS-190", "FreeSurfer", true,
        "znzCTABreadFromBinaryV2 reads at most 1,023 bytes of a name (the table's file name or an entry's) and does not skip the rest: the other bytes of a longer name are read as the numbers that follow";
    FS_191 = "FS-191", "FreeSurfer", true,
        "the binary colour table readers given an entry count or a name length beyond the machine's memory give an error message, a crash (the first version's), or a table, depending on the machine's memory";
    FS_192 = "FS-192", "FreeSurfer", true,
        "the binary colour table readers given a file that ends inside the table take the missing numbers from memory contents (the first version reads as many entries as its count says) and return a table";
    FS_194 = "FS-194", "FreeSurfer", false,
        "the first version's binary colour table writers store the table's entry count, empty places included, but only its non-empty entries: a table with empty places is read back with its last entries taken from what follows it (kept: the first version's format, which FreeSurfer writes only for tables read from first-version files, has no empty places)";
    FS_193 = "FS-193", "FreeSurfer", true,
        "the length FStagsIO gives a GCA morph's geometry tag in FreeSurfer's NIfTI extension counts the two file names whole, while the geometry's writer stores at most 512 bytes of each: with a longer name the extension's lengths are wrong";
    FS_195 = "FS-195", "FreeSurfer", true,
        "copying a linear transform array (LTAcopy) does not copy each transform's own type and label: a new copy's transforms have type -1 (unknown) and label 0, a given destination keeps its own";
    FS_196 = "FS-196", "FreeSurfer", true,
        "copying a linear transform array onto one whose transforms lack a matrix, or have smaller ones, loses that matrix's copy: the destination keeps no matrix, or its old numbers";
    FS_197 = "FS-197", "FreeSurfer", true,
        "freeing no linear transform array (LTAfree of a null array) crashes";
    FS_198 = "FS-198", "FreeSurfer", true,
        "copying a linear transform array from one whose transforms lack a matrix onto one that has it leaves the destination's matrix unusable: using or freeing the copy later crashes or reads garbage";
    FS_199 = "FS-199", "FreeSurfer", true,
        "npy2mri (reading a NumPy .npy file) reads the header length of a file of format version 3.0 from 3 bytes instead of NumPy's 4: the header text starts a byte early and lacks its last byte, and the reading runs on past it (a crash or a header from memory contents); versions 3.1 and later crash";
    FS_200 = "FS-200", "FreeSurfer", true,
        "npy2mri of a .npy file of format version 0 or 4 to 255 reads its header length with a size taken from memory contents: a crash or a header read from memory contents";
    FS_201 = "FS-201", "FreeSurfer", true,
        "npy2mri of a .npy file whose header length is about 8 MiB or more (the program's default stack size; version 2 headers may give up to 4 GiB) crashes";
    FS_202 = "FS-202", "FreeSurfer", true,
        "npy2mri of a .npy file that ends inside its header takes the header's missing bytes from memory contents";
    FS_203 = "FS-203", "FreeSurfer", true,
        "npy2mri reads past a .npy header that has no newline, or whose descr, fortran_order or shape value or a key without a colon runs to its end: memory contents beyond the header (a crash, or a type, order or shape read from memory)";
    FS_204 = "FS-204", "FreeSurfer", true,
        "npy2mri crashes on a shape with an empty element: every one-dimensional array as NumPy writes it, (3,), and shapes such as (3, ) or (3,,4)";
    FS_205 = "FS-205", "FreeSurfer", true,
        "npy2mri crashes on an array whose extents are all 1 (a single value of any number of dimensions) or that has none";
    FS_206 = "FS-206", "FreeSurfer", true,
        "a volume made from fewer than 3 extents (a two- or one-dimensional .npy array, or one whose leading or trailing extents of 1 npy2mri drops) gets its missing extents from memory: for an array whose extents of 1 were dropped, a dropped or earlier extent (an array of shape (1, 3, 4, 1) becomes a volume of 3 x 4 x 4 voxels, (1, 1, 2) one of 2 x 2 x 2), otherwise memory contents (often 0: a volume without voxels)";
    FS_207 = "FS-207", "FreeSurfer", true,
        "npy2mri of a .npy header without a descr or a fortran_order entry takes the data type or the order from memory contents, and of one without a shape entry crashes";
    FS_208 = "FS-208", "FreeSurfer", true,
        "npy2mri reverses the bytes of a big-endian array of 8-byte integers in 4-byte halves: every value is wrong";
    FS_209 = "FS-209", "FreeSurfer", true,
        "npy2mri's verbose output for a C-order array prints memory addresses, which differ from run to run";
    FS_210 = "FS-210", "FreeSurfer", true,
        "npy2mri takes any fortran_order value other than False (such as 0, false or a misspelling) as True: the voxels are placed transposed";
    FS_211 = "FS-211", "FreeSurfer", true,
        "npy2mri reads a slice of 2 GiB or more wrongly or past the volume's memory (its byte count does not fit 32 bits)";
    FS_212 = "FS-212", "FreeSurfer", true,
        "npy2mri of a .npy file shorter than its array leaves the missing voxels 0 without a message";
    FS_213 = "FS-213", "FreeSurfer", true,
        "npy2mri of a .npy header whose length does not make the data start at a multiple of 16 bytes skips the bytes up to the next multiple: the voxels are shifted by those bytes (NumPy reads the data right after the header)";
    FS_214 = "FS-214", "FreeSurfer", true,
        "gifti_get_DA_value_2D's message for a GIFTI data array without data prints the array's memory address as data= (it differs from run to run), not the data's (nil)";
    FS_215 = "FS-215", "FreeSurfer", true,
        "gifti_get_DA_value_2D reads a COMPLEX64 array's values as 8-byte doubles: each value is a meaningless number made of a value's real and imaginary parts";
    FS_216 = "FS-216", "FreeSurfer", true,
        "gifti_get_DA_value_2D reads the elements of an array of more than 2^31 values beyond the 2^31st from the wrong place";
    FS_217 = "FS-217", "FreeSurfer", true,
        "DTIwriteBValues and DTIwriteBVectors given no matrix create (or empty) the file and crash";
    FS_218 = "FS-218", "FreeSurfer", true,
        "DTIwriteBVectors of a matrix with fewer than 3 columns, and DTIwriteBValues of one without columns, write the next rows' numbers in each row's missing columns and, past the matrix's last number, memory contents";
    FS_219 = "FS-219", "FreeSurfer", true,
        "DTIparsePulseSeqName of a name with 100 characters or more before or after its # overflows a buffer: a crash or wrong values";
    FS_220 = "FS-220", "FreeSurfer", true,
        "DTIbvecChangeSpace of gradient directions with fewer than 3 columns reads and writes past each row: it changes the next rows' numbers, and past the matrix's last number it reads memory contents and writes outside the matrix";
    FS_346 = "FS-346", "FreeSurfer", true,
        "the binary triangle and quadrangle surface readers given a command-line tag whose length is -1 read the rest of the file into a block of no bytes: memory corruption or a crash when more than a few bytes follow";
    FS_347 = "FS-347", "FreeSurfer", true,
        "the binary triangle and quadrangle surface readers given more than 1,000 command-line tags store the 1,001st past the surface's list of command lines, over its command-line count and group-average area (values from a memory address, then wrong command lines or a crash)";
    FS_348 = "FS-348", "FreeSurfer", true,
        "the quadrangle surface reader given a command-line tag whose length cannot be allocated (below -1, or 2^63 - 1) writes through the failed allocation: a segmentation fault";
    FS_349 = "FS-349", "FreeSurfer", true,
        "the quadrangle surface reader given room for fewer faces than its quadrangles make (a size multiplier below 1) writes the others past its face array before its own check ends the program";
    FS_350 = "FS-350", "FreeSurfer", true,
        "the binary triangle and quadrangle surface writers given a surface whose command-line count is larger than the command lines it holds end the program with a segmentation fault (signal 11), leaving a partial file";
    FS_351 = "FS-351", "FreeSurfer", true,
        "the quadrangle surface reader accepts a quadrangle vertex number at or above the surface's vertex count (within its room for vertices): the surface's faces then refer to a vertex it does not have";
    FS_352 = "FS-352", "FreeSurfer", true,
        "the quadrangle surface reader given a quadrangle vertex number beyond the surface's room for vertices counts the face in memory outside its vertex arrays (memory corruption or a crash)";
    FS_353 = "FS-353", "FreeSurfer", true,
        "the quadrangle surface reader drops both triangles of a quadrangle whose first triangle (by the split it chooses) has two equal vertices: a single triangle the quadrangle writer stored as v0 v1 v1 v2 is lost on reading for about half of the vertex pairs";
    FS_354 = "FS-354", "FreeSurfer", true,
        "the binary triangle surface readers given a creator's line of more than 199 bytes (a long user name or creation date in the file's first line) read its remaining bytes as the vertex and face counts: the file's surface cannot be read";
    FS_355 = "FS-355", "FreeSurfer", true,
        "the surface library's annotation reader, given an annotation name without a folder that starts with the surface's hemisphere letters (lh.aparc.annot for a left surface), adds the hemisphere again (label/lh.lh.aparc.annot), so it does not find the file the annotation writer wrote for the same name; it takes a name as naming the hemisphere only when the name is the two letters alone, in any case";
    FS_356 = "FS-356", "FreeSurfer", true,
        "the surface library's file-type test and the curvature and annotation name builders, given a name (or a surface name) whose folder or last part is 1,024 bytes or more, write past a 1,024-byte buffer (the program can end with '*** stack smashing detected ***' or go on with corrupted names); the surface reader (MRISread), looking for the subject's Talairach transform beside a surface name of 300 bytes or more, writes past a 300-byte buffer; the GIFTI surface reader copies a file name of 1,024 bytes or more past the surface's 1,024-byte name";
    FS_357 = "FS-357", "FreeSurfer", true,
        "the surface library's neighbourhood search (the 2- and 3-hop neighbours MRISread and others compute), given a vertex with a ripped vertex among its known neighbours, reads its neighbour list past the part it filled: the vertex's neighbourhood then depends on memory contents (MRISread then ends with a failed assertion at the ripped vertex)";
    FS_358 = "FS-358", "FreeSurfer", true,
        "the surface library's older neighbourhood search (taken when MRISfindNeighborsAtVertex_old is set), given a vertex with more than 1,024 vertices within the hops asked for, writes past its caller's 1,024-entry list";
    FS_359 = "FS-359", "FreeSurfer", true,
        "the surface reader (MRISread) given a quadrangle surface file in the old format (no magic number) reads each vertex's face list past its end (its count is the file's count plus the faces found): the surface's neighbours depend on memory contents";
    FS_360 = "FS-360", "FreeSurfer", true,
        "the surface reader (MRISread), given a surface file with a vertex of more than 255 faces whose one-byte count wrapped around (FS-147 reproduced), writes the vertex's face list past its end";
    FS_361 = "FS-361", "FreeSurfer", true,
        "the icosahedron reader (ICOread, and MRISread of a .tri or .ico file), given a surface that is not closed (a vertex with more neighbours than faces) or a vertex of more than 255 faces, writes past the vertex's neighbour or face list (memory corruption or a crash)";
    FS_362 = "FS-362", "FreeSurfer", true,
        "the icosahedron reader (ICOread, and MRISread of a .tri or .ico file), given a vertex or face count it cannot read, a vertex or face number outside the counts, or a face whose vertices are not all among the file's vertices (a face line missing, or holding 0), writes or reads outside its arrays (memory corruption or a crash)";
    FS_363 = "FS-363", "FreeSurfer", true,
        "the surface library's plane fit (mrisComputeOptimalPlane) fits its plane through the origin instead of the vertices' mean, includes ripped vertices, and returns the centre (0, 0, 0)";
    FS_364 = "FS-364", "FreeSurfer", true,
        "MRIShistoThresholdCurvature (mris_curvature's outlier removal) of a surface whose unripped curvatures are all equal, or one of which is not finite (NaN, infinite), writes far outside its 1,000-bin histogram: a segmentation fault";
    FS_365 = "FS-365", "FreeSurfer", true,
        "MRIShistoThresholdCurvature of a surface with no unripped vertex reads memory before its histogram (no effect on the result)";
    FS_366 = "FS-366", "FreeSurfer", true,
        "the barycentric coordinates of a point on a surface face (face_barycentric_coords; MRISsampleFace's value) are 0, 0, 0 for a face whose projected triangle is degenerate (twice its area below 1e-15) instead of a third each, as the function gives for a face whose edges have no length";
    FS_367 = "FS-367", "FreeSurfer", true,
        "the brute-force distance from a point to a triangle (MinDistToTriangleBF) with a step below 2^-52 (zero or negative included) can loop for ever";
    FS_368 = "FS-368", "FreeSurfer", false,
        "the averaged surface normals of a coordinate set other than the current one (MRIScomputeSurfaceNormals of the white, pial or original positions) leave the surface's current positions at that set's positions instead of the positions it had (kept until the effect on its callers is measured; the correction is on request)";
    FS_369 = "FS-369", "FreeSurfer", true,
        "the averaged surface normals (MRIScomputeSurfaceNormals) give a vertex whose faces are all ripped the normal of the vertex processed before it (0 for the first)";
    FS_371 = "FS-371", "FreeSurfer", true,
        "the topology fixer's point-in-spherical-triangle test (containsAnotherVertexOnSphere) lists the triangle's neighbours in 1,000 places: more than 1,000 write past the list (memory corruption or a crash)";
    FS_370 = "FS-370", "FreeSurfer", true,
        "the closest-vertex searches across the cortex and the cortical thickness (MRISfindClosestOrigVertices, MRISfindClosestPialVerticesCanonicalCoords, MRISmeasureCorticalThickness) with a neighbourhood size of 100 or more, or a search reaching more than 100,000 vertices, read and write outside their arrays (wrong counts or a crash)";
    FS_372 = "FS-372", "FreeSurfer", true,
        "the discrete curvatures (MRIScomputeSecondFundamentalFormDiscrete, mris_curvature's), at a vertex whose faces do not form one fan around it (a vertex on the surface's border, two fans meeting at one vertex), read before the vertex's face list and can write past their own list (an undefined result or a crash) before reporting the vertex";
    FS_373 = "FS-373", "FreeSurfer", true,
        "the discrete curvatures' sign of two faces (FACES_Hcurvature_determineSign), given a face that names one vertex twice, read the vertex before the surface's first (an undefined sign)";
    FS_374 = "FS-374", "FreeSurfer", true,
        "the area errors as a surface's curvature (MRISuseAreaErrors) divide each vertex's area change by zero instead of its face count: every vertex's value is infinite or not a number";
    FS_375 = "FS-375", "FreeSurfer", true,
        "the principal curvatures and their functions as a surface's curvature (MRISuseK1Curvature, MRISuseK2Curvature, MRISusePrincipalCurvatureFunction; mris_curvature_stats) give a range that includes the first vertex's previous curvature value, which no vertex has any more (a range wider than the values, or not a number)";
    FS_376 = "FS-376", "FreeSurfer", true,
        "the principal curvatures and their functions as a surface's curvature (MRISuseK1Curvature, MRISuseK2Curvature, MRISusePrincipalCurvatureFunction), on a surface without vertices, read memory outside the surface (an undefined range)";
    FS_377 = "FS-377", "FreeSurfer", true,
        "the principal curvatures' outlier removal (MRIScomputeSecondFundamentalFormThresholded with a threshold of 0 or more; mris_curvature -thresh) of a surface whose unripped principal curvatures k1 (or k2) are all equal, or one of which is not finite (NaN, infinite), writes far outside its 1,000-bin histogram: a segmentation fault";
    FS_378 = "FS-378", "FreeSurfer", true,
        "the principal curvatures' outlier removal (MRIScomputeSecondFundamentalFormThresholded with a threshold of 0 or more) of a surface with no unripped vertex reads memory before its histograms (no effect on the result)";
    FS_379 = "FS-379", "FreeSurfer", true,
        "the averaging of a surface's gradients (MRISaverageGradients), on a surface whose neighbour lists are not symmetric (an unripped neighbour whose own neighbours are all ripped), averages a vertex with other vertices' neighbours and reads past its list (an undefined result)";
    FS_380 = "FS-380", "FreeSurfer", true,
        "the vertex report (MRISprintVertexInfo) lists a vertex's neighbours by its face count, so for a vertex with more faces than room in its neighbour list it reads past the list (undefined numbers or a crash)";
    FS_381 = "FS-381", "FreeSurfer", true,
        "the GIFTI surface reader (mrisReadGIFTIfile; MRISread of a .gii surface) copies the coordinates' VolGeomFname metadata into the surface's 1,024-byte volume-geometry name as 1,024 bytes whatever its length: past a shorter name it copies memory outside the metadata (bytes that the surface's writers later write), and a name of 1,024 bytes or more is left without its end";
    FS_382 = "FS-382", "FreeSurfer", true,
        "the GIFTI surface reader, given a NUM_TAG_CMDLINE above 1,000 in the coordinates' metadata, says it keeps the last 1,000 command lines and then never returns";
    FS_383 = "FS-383", "FreeSurfer", true,
        "the GIFTI surface reader takes the face array's vertex numbers unchecked: a face naming a vertex the surface does not have (negative, or past the coordinates) is counted in memory outside the surface (an undefined result or a crash, then or later)";
    FS_384 = "FS-384", "FreeSurfer", true,
        "the GIFTI reader's sparse storage (a NIFTI_INTENT_NODE_INDEX array) takes its vertex numbers unchecked: a number the surface does not have stores the value in memory outside the surface";
    FS_385 = "FS-385", "FreeSurfer", true,
        "the GIFTI reader reads a NIFTI_INTENT_LABEL array's values as 32-bit unsigned integers whatever the array's data type: the keys of a FLOAT32 array are its numbers' bit patterns, and an array of 8- or 16-bit integers is read four bytes per value, past its data";
    FS_386 = "FS-386", "FreeSurfer", true,
        "the GIFTI reader looks a label array's keys up in the surface's colour table by key: a key the table has no entry for (a table the caller gives, or one made from a colour array read before the labels) is read outside the table or through an empty entry (a crash)";
    FS_387 = "FS-387", "FreeSurfer", true,
        "the GIFTI surface reader compares a coordinate system's DataSpace (and, for NIFTI_XFORM_UNKNOWN, its TransformedSpace) without checking that the file gave one: a coordinate system without it crashes the program";
    FS_388 = "FS-388", "FreeSurfer", true,
        "the GIFTI reader, asked for one data array by number (an annotation's data array), does not check the number against the file's arrays: a number past them, or negative, reads outside the image's array list (a crash)";
    FS_389 = "FS-389", "FreeSurfer", true,
        "the GIFTI label-table reader uses each label's key as its place in the colour table without checking its sign: a negative key writes its entry before the table (the table then holds fewer entries than labels and the reader refuses the file, or the program crashes)";
    FS_390 = "FS-390", "FreeSurfer", false,
        "the GIFTI reader's sparse storage (a NIFTI_INTENT_NODE_INDEX array) is refused unless it lists every vertex: the data arrays that follow it must hold one value per vertex of the surface, not one per listed vertex as the GIFTI format's sparse storage has them";
    FS_391 = "FS-391", "FreeSurfer", true,
        "the GIFTI reader's shape overlays (NIFTI_INTENT_SHAPE arrays read as overlays) take their data type from the array's ShapeDataType metadata: an array without it gets a data type of memory contents, and one of 256 bytes or more is written past the overlay's 256-byte field";
    FS_392 = "FS-392", "FreeSurfer", true,
        "the GIFTI reader's shape overlays copy the array's Name metadata as 1,024 bytes: a name of 1,024 bytes or more is left without its end and read past";
    FS_393 = "FS-393", "FreeSurfer", true,
        "the colour table the GIFTI reader makes from a NIFTI_INTENT_RGB_VECTOR or RGBA_VECTOR array takes each colour's alpha from memory past the three components it stored (an undefined alpha)";
    FS_394 = "FS-394", "FreeSurfer", false,
        "the GIFTI reader's statistics overlays (data arrays of the other intents read as overlays) keep only the first array of each run of one intent, never add the last run to the overlay list, and write the third and later arrays of a run outside their one-frame volume: a file of statistics arrays gives no statistics overlay, or one with only its first array";
    FS_395 = "FS-395", "FreeSurfer", true,
        "the GIFTI readers (of surfaces, of volumes, and the count of a file's overlays) compare the file's Version with \"1\" without checking that the file gave one: a GIFTI file without a Version attribute crashes the program (gifticlib itself accepts it)";
    FS_396 = "FS-396", "FreeSurfer", true,
        "the GIFTI reader reads a NIFTI_INTENT_LABEL array's values without checking that gifticlib gave it any (an array gifticlib could not decode, such as ASCII UINT32 values): the program crashes";
    FS_397 = "FS-397", "FreeSurfer", true,
        "the GIFTI label writer sizes its label table by the colour table's entries but fills it only with the entries that have a name: a colour table with an unnamed entry gives a label table with unnamed places, the file is found invalid and is not written";
    FS_398 = "FS-398", "FreeSurfer", true,
        "the GIFTI label writer reads the surface's colour table without checking that it has one: writing labels of a surface without a colour table crashes the program";
    FS_399 = "FS-399", "FreeSurfer", true,
        "the GIFTI shape and statistics writers that take their values from a volume place each voxel by its column only: a volume of more than one row or slice has its values overwritten by the last row and slice (and columns past the surface's vertices are refused with a message)";
    FS_400 = "FS-400", "FreeSurfer", true,
        "the GIFTI statistics writer that takes its values from a volume writes a single data array whatever the volume's frames: a volume of several frames gives only its last frame, one of no frames an array without attributes (the file is refused as invalid)";
    FS_521 = "FS-521", "FreeSurfer", true,
        "the GIFTI reader of volumes (MRIread of a .gii file) chooses the data arrays to read correctly only when every array of the file has the same intent: in a file that mixes intents it counts the arrays of every intent it knows and then reads an intent from memory it never wrote, so the volume's frames, type and values are undefined (usually every value 0)";
    FS_522 = "FS-522", "FreeSurfer", true,
        "the GIFTI writer of volumes (MRIwrite to a .gii file) writes the first row of the first slice of each frame only: the other rows and slices of a volume are lost";
    FS_523 = "FS-523", "FreeSurfer", true,
        "the combined GIFTI writer (a surface with its overlays) reads each shape or statistics overlay's volume without checking that it has one: an overlay without its values crashes the program";
    FS_524 = "FS-524", "FreeSurfer", true,
        "the combined GIFTI writer looks an overlay's GIFTI intent up by its overlay type, and the generic statistics type (the one the GIFTI reader gives its statistics overlays) has none: such overlays are dropped from the file without a message";
    FS_525 = "FS-525", "FreeSurfer", true,
        "the surface autocorrelation and kernel smoothing functions (MRISar1, MRISarN, MRISsmoothKernel) read their input volume's voxels, and write an output volume the caller gives, as 32-bit floats whatever the volume's type: a volume of another type gives values made of other voxels' bytes (or of memory past the volume)";
    FS_526 = "FS-526", "FreeSurfer", true,
        "MRISsmoothKernel given a per-vertex kernel volume weighs every neighbour by the global kernel instead, which is then all zeros: every smoothed value is 0/0 (NaN)";
    FS_527 = "FS-527", "FreeSurfer", true,
        "MRISsmoothKernel's closing message prints the output's value at column 1031 of its first row whatever the output's size: for a surface of fewer than 1,032 vertices it prints another voxel's value or memory outside the volume";
    FS_528 = "FS-528", "FreeSurfer", true,
        "MRIScrsLUT, for a surface whose vertex count differs from the volume's voxels, does not free the table it began (a small memory leak; the result is not changed)";
    FS_529 = "FS-529", "FreeSurfer", false,
        "MRISsmoothKernel given an output volume does not clear it: the vertices it skips (ripped, masked, or without neighbours) keep the caller's values";
    FS_530 = "FS-530", "FreeSurfer", true,
        "the surface hop lists (SetSurfHopList) and their users MRISarN and MRISsmoothKernel accept fewer than one hop and then write into empty lists: the program crashes at the first vertex they handle";
    FS_531 = "FS-531", "FreeSurfer", true,
        "mriSurfaceRASToVoxel with FREESURFER_mriSurfaceRASToVoxel_both set checks its two computations the wrong way round: it reports \"not same answer\" and crashes the program whenever they agree (in any coordinate)";
    FS_532 = "FS-532", "FreeSurfer", true,
        "mriSurfaceRASToVoxel uses a surface-RAS-to-voxel matrix that nothing in FreeSurfer sets: called without one, or with one whose product with a point is refused (a matrix of other than four columns) or has fewer than three rows, it crashes the program or reads memory outside the product";
    FS_533 = "FS-533", "FreeSurfer", true,
        "MRISsmoothKernel given hop lists made by an earlier call for fewer hops than its kernel has reads past the lists (memory contents)";
    FS_534 = "FS-534", "FreeSurfer", true,
        "with FS-525, a volume of a type smaller than a float has its later voxels read and written past the end of its memory: the values read are memory contents, and the writes corrupt the program's memory (it can crash later)";
    FS_535 = "FS-535", "FreeSurfer", true,
        "the surface's corner table (used by the corner-angle statistics and the surface-quality summaries) on a surface with an edge not shared by exactly two triangles (an open or non-manifold surface): the edge table stops at that edge with an error, the later edges are left as zero-length edges of vertex 0, and building the corners then crashes the program";
    FS_536 = "FS-536", "FreeSurfer", true,
        "the surface-quality summaries (one line, or the Area/Corner/Edge/Hinge table) crash the program when a statistic has nothing to summarise (every vertex ripped, or no triangle, edge or corner left)";
    FS_537 = "FS-537", "FreeSurfer", true,
        "the corner-angle statistics use the edges' directions from the last edge measurement, not from the surface's current coordinates: after the vertices move they describe the old surface, and before any edge measurement every corner reads 90 degrees";
    FS_538 = "FS-538", "FreeSurfer", true,
        "the edge statistics (lengths, hinge dot products and angles) and the one-line surface-quality summary use the triangles' normals from their last computation: after the vertices move the hinge values describe the old surface, and with no normals computed yet the program crashes";
    FS_539 = "FS-539", "FreeSurfer", true,
        "the fast-marching distance maps (MRIextractDistanceMap, MRIdistanceTransform) of a volume one voxel wide take the voxel that follows in memory (the first voxel of the next row) as each voxel's neighbour beyond the edge, so the distances depend on the next row's labels";
    FS_540 = "FS-540", "FreeSurfer", true,
        "the fast-marching distance maps of a volume one voxel high or deep, and of the last voxel of a volume one voxel wide, read memory outside the volume as a neighbour beyond the edge: the distances depend on memory contents, or the program crashes";
    FS_541 = "FS-541", "FreeSurfer", true,
        "with a mask, the fast-marching distance maps first set the masked voxels from a distance limit not yet set (memory never written): the values are replaced before the map is returned, but in a volume one voxel wide (FS-539) a neighbour's distance can be read from them";
    FS_542 = "FS-542", "FreeSurfer", true,
        "the fast-marching distance maps turn a label-volume or mask value that is NaN or beyond the 32-bit integer range into -2147483648: such voxels never match the label, and the mask never excludes them";
    FS_543 = "FS-543", "FreeSurfer", true,
        "MRIdistanceTransform with DTRANS_MODE_INSIDE computes the unsigned distances on both sides of the label (DTRANS_MODE_UNSIGNED's map) instead of the distances inside it";
    FS_544 = "FS-544", "FreeSurfer", true,
        "the fast-marching distance maps given a mode they do not know (MRIextractDistanceMap's 1 to 4, MRIdistanceTransform's DTRANS_MODE_* 1 to 4, mris_distance_to_label's -mode) return a map of zeros without a message";
    FS_545 = "FS-545", "FreeSurfer", true,
        "the one-dimensional convolution (MRIconvolve1d) of a volume of a type other than uchar or float into a float volume takes the volume's outside value before the first voxel of a row and the next row's voxels past its last, where uchar and float volumes (and the integer-typed convolutions) repeat the edge voxel";
    FS_546 = "FS-546", "FreeSurfer", true,
        "the one-dimensional convolutions (MRIconvolve1d and its uchar, short and int forms) given a destination smaller than the source, or a source or destination frame the volume does not have, read and write other voxels or memory outside the volumes (a crash, memory corruption or memory contents)";
    FS_547 = "FS-547", "FreeSurfer", true,
        "the one-dimensional convolution into a uchar volume reads a float source's memory byte by byte as uchar voxels, and into a short or int volume reads a source of another type as shorts or ints: the values are other voxels' bytes, and for a narrower source type memory past the volume";
    FS_548 = "FS-548", "FreeSurfer", true,
        "the separable Gaussian convolution (MRIconvolveGaussian) given a kernel volume that is not float reads its memory as floats: wrong weights, or memory past the kernel";
    FS_549 = "FS-549", "FreeSurfer", true,
        "the one-dimensional convolutions with a kernel longer than 1,001 taps read past the volume's boundary index tables near the edges: memory contents as voxel indices (a crash or wrong voxels)";
    FS_550 = "FS-550", "FreeSurfer", true,
        "the separable Gaussian convolution (MRIconvolveGaussian) of a volume without frames crashes (an integer division by zero)";
    FS_551 = "FS-551", "FreeSurfer", true,
        "the Gaussian smoothing (MRIgaussianSmoothNI, MRIgaussianSmooth, MRImaskedGaussianSmooth) of a volume with an extent of 0 along a smoothed axis writes outside a matrix (memory corruption)";
    FS_552 = "FS-552", "FreeSurfer", true,
        "the masked Gaussian smoothing (MRImaskedGaussianSmooth) given a target whose extent or frames differ from the source's prints the smoother's dimension-mismatch error and then crashes";
    FS_553 = "FS-553", "FreeSurfer", true,
        "the pyramid reduction (MRIreduce) of a volume of one slice, without a destination, returns a volume of depth 0 (no voxels) instead of the reduced slice";
    FS_554 = "FS-554", "FreeSurfer", true,
        "the six-neighbour Laplacian (MRIlaplacian) of a volume of several frames takes the centre voxel of every frame from the first frame";
    FS_555 = "FS-555", "FreeSurfer", true,
        "the Sobel derivatives (MRIxSobel, MRIySobel, MRIzSobel, MRIsobel and its gradient magnitude) given a destination smaller than the source or without the frame they write, or, for a source that is not uchar, a destination that is not float, write floats into other voxels' bytes or into memory outside it (memory corruption, a crash), and read a gradient volume without its three frames outside its memory";
    FS_556 = "FS-556", "FreeSurfer", true,
        "the divergence (MRIdivergence) ignores the destination it is given: it returns a new volume and leaves the given one unchanged";
    FS_557 = "FS-557", "FreeSurfer", true,
        "the white-matter border classification (MRImarkBorderVoxels, MRIborderClassifyVoxel, MRIreclassify, MRIcomputeClassStatistics) turns a voxel value that is NaN or beyond the 32-bit integer range into -2147483648, and squares the neighbourhood's integer values in 32 bits (wrapping beyond 46,340)";
    FS_558 = "FS-558", "FreeSurfer", false,
        "the border voxel classifier (MRIborderClassifyVoxel, used by MRIreclassify) computes the gray-matter likelihood with the white matter's variance, and when both likelihoods are 0 divides the white one by 0 (its probability NaN, the voxel taken as white)";
    FS_559 = "FS-559", "FreeSurfer", true,
        "MRImarkBorderVoxels of a uchar volume given a destination that is not uchar, or smaller than the source, writes its labels as bytes into other voxels or outside the destination (memory corruption)";
    FS_561 = "FS-561", "FreeSurfer", true,
        "the region growing of the connected components (GrassFire6, GrassFire18) given a label that its label volume stores as 0 (0 itself, or a value the volume's type clamps or rounds to 0) never ends when two object voxels of the region touch: the program hangs while its memory grows";
    FS_560 = "FS-560", "FreeSurfer", true,
        "the white and gray class statistics (MRIcomputeClassStatistics) of an intensity volume with negative values or NaNs count them in a histogram bin before its start (memory corruption)";
    FS_562 = "FS-562", "FreeSurfer", true,
        "the temporal covariance and AR(1) maps (fMRIcovariance, fMRItemporalAR1) given an output volume that is not float, or smaller than the input, write floats into other voxels' bytes or outside the output (memory corruption)";
    FS_563 = "FS-563", "FreeSurfer", true,
        "the temporal AR(1) map (fMRItemporalAR1) of a voxel whose variance is 0 is its lag-1 covariance divided by 0 (NaN, or plus or minus infinity) instead of 0";
    FS_564 = "FS-564", "FreeSurfer", true,
        "the temporal AR(1) map and the time-course normalisation (fMRItemporalAR1, MRIframeNorm) of a series whose covariance cannot be computed (one frame, or a degrees-of-freedom adjustment that leaves none) print the covariance's error and then crash, unless every voxel is masked out";
    FS_565 = "FS-565", "FreeSurfer", true,
        "the temporal covariance (fMRIcovariance) with a degrees-of-freedom adjustment that is NaN or leaves a count outside the 32-bit integer range, and the frame subsampling (fMRIsubSample) with a frame count outside that range, convert the number undefinedly (x86-64: -2147483648; the covariance then divides by about 2^31 for a lag of 1 or more)";
    FS_566 = "FS-566", "FreeSurfer", true,
        "the ASL control-interpolation matrix (ASLinterpMatrix) of one time point writes one element past the matrix (memory outside it; the returned matrix is not changed)";
    FS_567 = "FS-567", "FreeSurfer", true,
        "the volume-to-matrix conversion (fMRItoMatrix) given a matrix with fewer rows than frames or fewer columns than voxels writes into other elements and outside the matrix (memory corruption)";
    FS_568 = "FS-568", "FreeSurfer", true,
        "the matrix-to-volume conversion (fMRIfromMatrix) with a mask that selects no voxel and a matrix with columns crashes (an integer division by zero)";
    FS_569 = "FS-569", "FreeSurfer", true,
        "the frame subsampling (fMRIsubSample) with a negative step from a start below the stop, or a step that carries the frame number past the 32-bit integer range, reads frames before the volume's memory without end (a crash)";
    FS_570 = "FS-570", "FreeSurfer", true,
        "the volume clusters' member coordinates (x, y, z) are left unset (memory contents) unless a transform is given when the clusters are found; pruning the clusters by distance then compares memory contents";
    FS_571 = "FS-571", "FreeSurfer", true,
        "the volume clusters' coordinates computed with a transform that is not 4 x 4 crash after the matrix product's error message (fewer than 3 rows: memory outside the product is read)";
    FS_572 = "FS-572", "FreeSurfer", false,
        "the non-parametric white matter and cortex classification (MRIreclassifyWMCtxNonPar, MRIclassifyAmbiguousNonPar, used by mri_segment with a segmentation) ends both class histograms at the smaller of the two classes' maxima (leaving the brighter white matter out of its distribution), and searches the upper ambiguity threshold from the first bin rather than from the classes' crossing";
    FS_573 = "FS-573", "FreeSurfer", true,
        "the ambiguity thresholds of MRIclassifyAmbiguousNonPar, when no histogram bin satisfies the probability ratio, are read from one bin before or after the histogram (memory contents)";
    FS_574 = "FS-574", "FreeSurfer", false,
        "MRIclassifyAmbiguousNonPar dilates the white matter and cortex masks NdilWM and NdilCtx times, but each dilation starts again from the undilated mask, so any count above 1 dilates by one voxel only";
    FS_575 = "FS-575", "FreeSurfer", true,
        "MRIclassifyAmbiguousNonPar with a dilation count of 0 or less crashes (it reads a dilated mask it never made)";
    FS_577 = "FS-577", "FreeSurfer", true,
        "the connected segments (MRIsegment, MRImaxsegment) record each scanned voxel's segment number in a 16-bit map: a segment numbered above 32,767 is not seen by the voxels next to it, which start or join other segments, so one connected region can be split";
    FS_578 = "FS-578", "FreeSurfer", true,
        "MRImaxsegment (mri_watershed), when it removes small segments during its scan, renumbers the segments it keeps but not the voxels already scanned: the voxels scanned next join the wrong segment or start a new one, so a connected region can be split or joined to another";
    FS_579 = "FS-579", "FreeSurfer", true,
        "MRIsetSegmentValue with a segment number outside the segment table reads memory outside it (memory contents, or a crash)";
    FS_581 = "FS-581", "FreeSurfer", true,
        "the connected segments of a volume with more than 32,768 columns, rows or slices keep voxel coordinates in 16 bits: they wrap, and the label map is then written outside the volume (memory corruption)";
    FS_582 = "FS-582", "FreeSurfer", true,
        "the segment dilations (MRIsegmentDilate, MRIsegmentDilateThreshold) of a segment voxel more than 500 voxels outside the volume read outside the volume's index tables (memory contents, or a crash)";
    FS_583 = "FS-583", "FreeSurfer", true,
        "the Voronoi diagram of the control points (MRIbuildVoronoiDiagram) of a 16-bit volume without a destination volume crashes (it reads the type of a destination it has not made yet)";
    FS_584 = "FS-584", "FreeSurfer", true,
        "the float soap bubble (MRIsoapBubble of a float volume, mri_normalize's bias field) runs its passes on several threads that update the largest change without synchronisation, so the pass after which it stops can change from run to run";
    FS_585 = "FS-585", "FreeSurfer", true,
        "the Voronoi diagram and the soap bubble read a control or destination volume of a wider type (an int control volume for a 16-bit source, a 16-bit or int volume where bytes are expected) byte by byte, so the control points and the averaged values are not the volume's values";
    FS_586 = "FS-586", "FreeSurfer", true,
        "the Voronoi diagram and the soap bubble given a destination or control volume smaller than the source (or with fewer frames, or a destination of another type for a float or 16-bit source) read and write outside that volume (memory corruption)";
    FS_587 = "FS-587", "FreeSurfer", true,
        "the intensity normalisation given a control volume (or a bias picture, or a 16-bit spline normalisation's destination) of another type than bytes (16-bit numbers) reads and writes that volume's memory byte by byte (16 bits at a time), so the control points and the written values are not the volume's values";
    FS_588 = "FS-588", "FreeSurfer", true,
        "the intensity normalisation given a control volume (or a bias picture, or a 16-bit spline normalisation's destination) smaller than the source reads and writes outside that volume (memory contents or corruption)";
    FS_589 = "FS-589", "FreeSurfer", true,
        "the intensity normalisation's windows (the control-point searches, the CSF search) of a volume whose voxel size is tiny, zero or not a number reach past the volume's index tables (memory contents, or a crash)";
    FS_590 = "FS-590", "FreeSurfer", true,
        "the one-dimensional normalisation's peak check (MRInormCheckPeaks) chooses its starting peak among the windows above the Talairach origin even when fewer peaks were found: it then reads entries that are not peaks (in mri_normalize, memory never written) and can drop good peaks";
    FS_591 = "FS-591", "FreeSurfer", true,
        "the one-dimensional normalisation with more than 80 windows writes past its window and histogram arrays, and its peak check with fewer than 7 windows above the Talairach origin reads before its peak arrays (memory corruption or contents)";
    FS_592 = "FS-592", "FreeSurfer", true,
        "the gentle control-point search (MRInormGentlyFindControlPoints, mri_normalize -gentle, mri_ca_normalize) wraps its intensity thresholds below 0 or above 255 around modulo 256 (a high threshold of 260 becomes 4), so the search finds other control points or none";
    FS_593 = "FS-593", "FreeSurfer", true,
        "MRI3dUseFileControlPoints with an FS_THRESH_CTRL that is not a number uses a threshold never written (memory contents)";
    FS_594 = "FS-594", "FreeSurfer", true,
        "MRI3dUseFileControlPoints with FS_THRESH_CTRL removes the control points below the threshold but keeps each one that directly follows a removed one";
    FS_595 = "FS-595", "FreeSurfer", true,
        "MRInormAddFileControlPoints' message on rejected control points prints a number that is not the number of rejected points (whatever the machine holds at that moment)";
    FS_596 = "FS-596", "FreeSurfer", true,
        "the intensity normalisation uses values never written: the number in the rejected control points' message, the bias of a bias field whose type the sampler refuses (memory contents)";
    FS_597 = "FS-597", "FreeSurfer", true,
        "the tissue intensities of the three-dimensional normalisation (MRInormFindControlPoints, MRI3dNormalize: mri_normalize's white matter, grey matter and CSF levels) are read from a smoothed histogram bin outside the histogram when a peak or valley is not found (a white matter peak without voxels near the labelled one, a grey matter valley or peak searched below the first bin): the levels, the thresholds made from them and the printed values are memory contents; a search started below the 32-bit integer range crashes";
    FS_598 = "FS-598", "FreeSurfer", true,
        "the three-dimensional normalisation's second control-point search (MRInormFindControlPoints) widens its intensity window until one pass finds 10 new control points, without end when fewer than 10 voxels can still become control points (rare: the first search, which must have succeeded, rounds its window differently, so only a white matter target with a fraction can leave the second short)";
    FS_600 = "FS-600", "FreeSurfer", true,
        "the one-dimensional normalisation (MRInormalize, mri_normalize without an aseg) checks its peaks against entries of its peak arrays past the peaks found, which it never wrote (memory contents): it can keep or drop peaks, or find none, by what the memory holds";
    FS_599 = "FS-599", "FreeSurfer", true,
        "the three-dimensional normalisation's first control-point search (MRInormFindControlPoints) gives up after its eleventh pass even when that pass found enough control points (the error 'could not find enough control points')";
    FS_601 = "FS-601", "FreeSurfer", true,
        "the high-signal control-point search of mri_normalize's -snr normalisation (MRInormFindHighSignalLowStdControlPoints, MRInormalizeHighSignalLowStd) prints, when fewer than two regions pass its signal-to-noise threshold, the missing region's voxel count and centroid from memory outside its table in the message 'using segments ...' (memory contents); with no region at all FreeSurfer also writes into that memory and restores it";
    FS_602 = "FS-602", "FreeSurfer", true,
        "the high-signal control-point search (MRInormFindHighSignalLowStdControlPoints) prints the largest region's voxel count rounded to single precision in its message 'using segments ...': a count above 16,777,216 voxels can be printed wrong (its control points are not changed)";
    FS_603 = "FS-603", "FreeSurfer", true,
        "the high-signal control-point search (MRInormFindHighSignalLowStdControlPoints) sizes its window of about 7 mm in voxels as an int that overflows for a slice thickness below about 3.3e-9 mm: the window wraps to no voxels or a negative size (its local means are not computed and its spreads are 0)";
    FS_604 = "FS-604", "FreeSurfer", true,
        "the surface library's cached conversion from a surface's tkregister RAS to a volume's voxels (MRISsurfaceRASToVoxelCached and the maps MRISfillInterior uses; the ribbon and shell volumes, mris_volmask, mris_place_surface, mri_surf2volseg) keeps the matrix it made for an earlier call while the new volume's header compares equal: a second surface whose volume geometry differs, a volume of another extent whose header was copied from the first (its centre voxel elsewhere) or one differing by less than 1.2e-7 in a header value gets the earlier matrix, so its points land in other voxels";
    FS_605 = "FS-605", "FreeSurfer", true,
        "the cached conversion from a volume's voxels to a surface's tkregister RAS (MRISsurfaceRASFromVoxelCached; mri_mc) makes its matrix at its first call and keeps it for every later surface and volume, so later conversions use the first pair's geometry";
    FS_606 = "FS-606", "FreeSurfer", false,
        "the surface shell and interior volumes (MRISshell, MRISfillInterior; mris_volmask's ribbon, mris_place_surface, mri_surf2volseg) draw nothing for a face whose first two corners are at one point (the face is a segment, or a point when all three coincide); its edges are drawn by the neighbouring faces of a closed surface";
    FS_607 = "FS-607", "FreeSurfer", true,
        "the bounding volume (MRISmakeBoundingVolume; mris_place_surface's T2 and FLAIR steps) of a surface whose bounding box spans more than 2^31 voxels of the surface's volume geometry gets a garbage extent (an integer overflow)";
    FS_608 = "FS-608", "FreeSurfer", true,
        "the interior of a surface (MRISfillInterior; mris_place_surface, mri_surf2volseg, mris_volmask's callers) in a given volume that the surface does not cross (no voxel of its shell inside the volume) ends the program with the misleading messages 'volume size must be greater than 0 in every dimension' and 'could not allocate memory for -999996 slices'";
    FS_609 = "FS-609", "FreeSurfer", true,
        "the outside distance map of a surface (MRISextractOutsideDistanceMap) reads its label volume as bytes: a label volume of another type is compared by one byte of each voxel's value, and a point of the map outside the label volume reads another voxel's byte while it stays in the volume's memory";
    FS_614 = "FS-614", "FreeSurfer", true,
        "the outside distance map of a surface (MRISextractOutsideDistanceMap) reads memory outside its label volume for a point of the map beyond the volume's memory (a crash or another program's data)";
    FS_610 = "FS-610", "FreeSurfer", true,
        "the outside distance map of a surface (MRISextractOutsideDistanceMap) whose label volume has no voxel of the label ends the program with 'No labels 0 could be found in the volume', naming 0 instead of the label";
    FS_611 = "FS-611", "FreeSurfer", true,
        "masking a surface's values by a label (MRISmaskLabel, MRISmaskNotLabel) writes outside the surface's vertices for a label point that names no vertex (a volume label's points, vertex -1) or a vertex beyond the surface's";
    FS_612 = "FS-612", "FreeSurfer", false,
        "masking a surface's values outside a label (MRISmaskNotLabel) leaves the values of the vertices that were already marked before the call, and clears the marks of the label's vertices";
    FS_613 = "FS-613", "FreeSurfer", true,
        "the laminar volume fractions of a surface pair (MRIcomputeLaminarVolumeFractions) compute the white surface's interior at the program's first call only and reuse it for every later call, whatever its surface, volume or resolution";
    FS_615 = "FS-615", "FreeSurfer", true,
        "the T2 mask of mri_em_register, mri_ca_register and mri_ca_normalize (MRImask_with_T2_and_aparc_aseg) given a T2 volume without an aparc+aseg (their option -T2MASK) crashes the program in its step that erases T2-bright voxels next to the dark exterior, the step that uses the aparc+aseg to stay out of the brain (as soon as one voxel lies outside the largest dark region)";
    FS_616 = "FS-616", "FreeSurfer", true,
        "the T2 mask (MRImask_with_T2_and_aparc_aseg) given a destination other than its source masks the destination's own voxels and ignores the source (every FreeSurfer caller passes one volume as both)";
    FS_617 = "FS-617", "FreeSurfer", true,
        "the T2 mask (MRImask_with_T2_and_aparc_aseg) asked to erase no layer of bright exterior voxels (a distance of 0 or less) prints the error 'MRIfree: null pointer' at its end (its result is not changed; every FreeSurfer caller asks for 5 layers)";
    FS_618 = "FS-618", "FreeSurfer", true,
        "the T2 mask (MRImask_with_T2_and_aparc_aseg) without a destination crashes the program at its end, after its messages (every FreeSurfer caller gives one)";
    FS_619 = "FS-619", "FreeSurfer", true,
        "writing one field of a surface's vertices into a new volume (MRISwriteIntoVolume given no volume) makes a volume of 1 x 1 x <vertices>, so a surface of two vertices or more is refused with 'nvertices <n> doesn't match' and no volume is given (the diagnostic dx, dy, dz volumes of the surface deformations then crash the program)";
    FS_620 = "FS-620", "FreeSurfer", true,
        "reading a surface's values from a volume (MRISreadValues) at a frame the volume does not have crashes the program (signal 11) instead of printing its 'attempted to read frame' error";
    FS_621 = "FS-621", "FreeSurfer", true,
        "reading a surface's values from a w file (MRISreadValues) with a vertex number outside the surface skips that entry without reading its value, so every later entry is read three bytes out of step (other vertex numbers and values), and its 'vertex index out of range' line prints the previous entry's value";
    FS_622 = "FS-622", "FreeSurfer", true,
        "reading a surface's values (MRISreadValues) from a file name of 1,023 bytes or more: FreeSurfer's copy of the name has no end, so the volume format test reads memory past it (an undefined answer)";
    FS_623 = "FS-623", "FreeSurfer", true,
        "reading a surface's values from a volume (MRISreadValues) whose width x height x depth overflows a 32-bit int to the vertex count writes past the values (a crash or memory corruption)";
    FS_624 = "FS-624", "FreeSurfer", true,
        "reading a surface segmentation (an MGH or NIfTI file of one value per vertex) as an annotation (MRISreadAnnotation) maps its numbers through the first colour table the program used for that, not through the file's own, when an earlier one was used (a second segmentation with another colour table gets the first table's colours)";
    FS_625 = "FS-625", "FreeSurfer", true,
        "reading a surface segmentation as an annotation (MRISreadAnnotation of an MGH or NIfTI file) with a colour table given by the caller frees that table, so a later use of it (another read with it, or the surface's table when it came from it) reads freed memory: a crash or garbage";
    FS_626 = "FS-626", "FreeSurfer", true,
        "the normal-dot and principal-curvature distributions mris_fix_topology learns its retessellation from (mrisComputeNormalDotDistribution, mrisComputePrincipalCurvatureDistributions) accept histograms of too few bins: the normal-dot one divides its bin size by the bin count minus 2 (zero or negative below 3 bins; with one bin its largest dots are counted one past the last bin, outside its counts), a curvature histogram without bins counts every vertex one before its first bin (memory corrupted); every caller passes 100 bins";
    FS_627 = "FS-627", "FreeSurfer", false,
        "the normal-dot distribution of mris_fix_topology (mrisComputeNormalDotDistribution) takes each edge's dot product with the vertex normal as dx nx + dy ny dz nz, a product where the sum dx nx + dy ny + dz nz is meant, so the histogram is not that of the edges' normal components";
    FS_628 = "FS-628", "FreeSurfer", true,
        "the principal-curvature distribution of mris_fix_topology (mrisComputePrincipalCurvatureDistributions) floors and normalises the first 100 x 100 voxels of its k1-k2 volume whatever the volume's size: a larger volume keeps its other voxels as raw counts outside the normalisation, and a smaller one, or one not of floats, is read and written outside its voxels (every caller passes 100 x 100 floats)";
    FS_629 = "FS-629", "FreeSurfer", true,
        "moving a surface to Talairach space (MRIStalairachTransform) crashes the program for a surface that has a Talairach transform but no surface-RAS-to-Talairach matrix, and writes past the destination's vertices when the destination surface has fewer vertices than the source (every caller moves one surface in place)";
    FS_630 = "FS-630", "FreeSurfer", true,
        "the label functions on a surface (LabelMark, LabelMark2, LabelUnmark, LabelArea, LabelFillMarked, and through them LabelErode and LabelDilate) read or write outside the surface's vertices for a label point that names no vertex (-1, a volume label's points) or a vertex beyond the surface's, and outside the label's vertex index table when it is shorter than the surface";
    FS_631 = "FS-631", "FreeSurfer", true,
        "dilating a label on a surface (LabelDilate) writes its new points past the label's places when the label is full and its growth by half does not enlarge it: a label without places (half of nothing is nothing), which gains points when a ripped vertex keeps a mark (the marks' clearing skips ripped vertices), or a growth beyond the 2^26 places larmorx allocates (FS-426)";
    FS_632 = "FS-632", "FreeSurfer", true,
        "inverting a label on a surface (MRISlabelInvert, and LabelRemoveHolesSurf through it) crashes the program when the label has a point beyond the surface's vertices (its mask is refused and then read)";
    FS_633 = "FS-633", "FreeSurfer", true,
        "writing a surface's area error as a value file (MRISwriteAreaErrorToValFile) takes each vertex's original area from the surface's first faces (as many as the vertex has) instead of from the vertex's own faces, so the values written are not the vertices' area errors";
    FS_634 = "FS-634", "FreeSurfer", true,
        "writing a surface's area error as a value file (MRISwriteAreaErrorToValFile) reads the face-normal cache past the surface's faces for a vertex that lists more faces than the surface has (a face listed twice): a crash or another array's data";
    FS_901 = "FS-901", "FreeSurfer", true,
        "growing a cluster on a surface (sclustMapSurfClusters, sclustGrowSurfCluster, sclustGrowByDist; mri_surfcluster, mri_glmfit's simulations, label hole and island removal, mris_remove_intersection's hole filling, mri_fieldsign) crashes the program (a stack overflow, signal 11) when the cluster is large enough: its growth's path through the vertices passes what the program's 8 MiB stack holds (about 104,000 vertices; a whole hemisphere's cluster on a surface of 120,000 to 150,000 vertices goes 71,000 to 87,000 deep, so high-resolution surfaces of more than about 175,000 vertices crash)";
    FS_902 = "FS-902", "FreeSurfer", true,
        "removing a surface label's islands (LabelRemoveIslandsSurf; also its hole removal, LabelRemoveHolesSurf, and both, LabelRemoveHolesAndIslandsSurf; mri_label2label --annot-fill-holes) crashes the program for a label without points, and so the hole removal for a label that covers every vertex of the surface";
    FS_903 = "FS-903", "FreeSurfer", true,
        "the surface cluster functions (the cluster summary, the renumbering after sorting, the clusterwise-p pruning and its map of values, the cluster annotation, the cluster growths' seeds) read or write outside their arrays for a vertex whose cluster number is negative or beyond the cluster list, or for a seed that is not one of the surface's vertices; every caller gives the numbers the cluster growth made";
    FS_904 = "FS-904", "FreeSurfer", true,
        "pruning surface clusters by their clusterwise p-value (sclustPruneByCWPval; mri_surfcluster --cwpvalthresh) writes past its renumbering table for more than 10,000 clusters: memory corrupted";
    FS_905 = "FS-905", "FreeSurfer", true,
        "pruning surface clusters by their clusterwise p-value (sclustPruneByCWPval) neither keeps nor removes a cluster whose p-value is NaN: the cluster is left out of the list, but its vertices keep their values and get a cluster number from memory never written";
    FS_906 = "FS-906", "FreeSurfer", true,
        "saving surface clusters as a point set (sclustSaveAsPointSet; mri_surfcluster --pointset) crashes the program when the file cannot be created";
    FS_907 = "FS-907", "FreeSurfer", true,
        "growing a cluster by distance (sclustGrowByDist) continued by a later call reads the earlier call's seed vertex, which may belong to a surface already freed or with fewer vertices; every caller starts each growth anew";
    FS_908 = "FS-908", "FreeSurfer", true,
        "importing a surface's values or curvatures from a vector or a matrix column (MRISimportValVector, MRISimportCurvatureVector, MRISimportValFromMatrixColumn) reads outside the vector or the matrix when it holds fewer values than the surface has vertices, or for a column the matrix does not have; every caller passes one value per vertex";
    FS_909 = "FS-909", "FreeSurfer", true,
        "the surface hash table of faces (mrisMarkIntersections, the intersection removal of mris_fix_topology, mris_place_surface, mris_remesh and mris_remove_intersection, the self-intersection tests) given a resolution that is not above 0, or a surface coordinate that is not finite or so far from the origin that its voxel number passes the int range, stores the faces in the table's first bucket and looks them up nowhere: intersections are missed without a word; a closest-vertex or closest-face search from a point that is not finite or that far finds nothing";
    FS_910 = "FS-910", "FreeSurfer", true,
        "the surface hash table of faces misses every intersection of faces more than 1,000 voxels from the origin along an axis (1,000 mm at its default resolution of 1 mm): such faces are stored at the table's edge but looked for beyond it";
    FS_911 = "FS-911", "FreeSurfer", true,
        "the surface hash table of faces lists a face that touches more than 10,000 of its voxels in the first 10,000 only, with two error messages (complete list too big!) for each voxel left out, so intersections with the rest of the face are missed";
    FS_912 = "FS-912", "FreeSurfer", true,
        "the surface hash table's intersection test ends the program (MHT_MAX_FACES exceeded!, exit status 1) when more than 10,000 faces lie in the voxels of the face tested";
    FS_913 = "FS-913", "FreeSurfer", true,
        "the surface hash table of faces runs for hours on a face whose sampling takes hundreds of millions of steps (a corner that is not finite next to finite ones, a face kilometres across), printing two error messages for each voxel past the 10,000th";
    FS_914 = "FS-914", "FreeSurfer", false,
        "the hole counts printed when holes are filled are not the number of holes: marking a surface's intersections with hole filling (mrisMarkIntersections, mris_remove_intersection --fill-holes) prints Found N holes counting the unmarked region around the holes as one, and MRISfillHoles prints found -1 holes when no vertex is below its threshold";
    FS_915 = "FS-915", "FreeSurfer", true,
        "filling the holes of a surface map (MRISfillHoles, and the hinge removal's hole filling through MRISmarkEdge) treats a vertex whose value equals the threshold, or is NaN, as neither in the map nor in a hole: it stays out of the map even inside a hole, and it separates the holes around it";
    FS_916 = "FS-916", "FreeSurfer", false,
        "marking a surface's intersections (mrisMarkIntersections) leaves the marks of ripped vertices as they were, and its hole filling counts a ripped vertex as marked or not by that old mark; the programs' surfaces have no ripped vertices there";
    FS_917 = "FS-917", "FreeSurfer", true,
        "ripping a surface's vertices by a label (MRISripLabel, MRISripNotLabel: mris_place_surface's label option) writes outside the surface's vertices for a label point that names no vertex (-1, a volume label's points) or a vertex beyond the surface's";
    FS_978 = "FS-978", "FreeSurfer", true,
        "the mode filter of a surface's annotations (MRISmodeFilterAnnotations, mris_ca_label) filters through the program's annotation table, which it sets from the surface's colour table only when the program has none and only after it has sized its counts: a surface whose annotations come from its own colour table is filtered through another table (the default table file's, or one the program used before), and when the program has no table and the default file cannot be read the filter changes nothing but the annotations it does not know (mris_ca_label sets the table from the same colour table first, so its results are not changed)";
    FS_979 = "FS-979", "FreeSurfer", true,
        "in FS-978's second case (no annotation table and no default table file), the mode filter of a surface's annotations counts every annotation of the surface's colour table outside its counts of one entry: memory is corrupted and the program may crash";
    FS_980 = "FS-980", "FreeSurfer", true,
        "the vector registration's value setters (MRISsetCurvaturesToValues, MRISsetCurvaturesToOrigValues, MRISsetOrigValuesToCurvatures, MRISsetOrigValuesToValues, MRISsetValuesToCurvatures) read or write outside a vertex's value arrays for a field number outside them";
    FS_981 = "FS-981", "FreeSurfer", true,
        "the vector registration's value setters crash the program for an unripped vertex that has no value arrays";
    FS_982 = "FS-982", "FreeSurfer", true,
        "dilating a vertex to a target sum (MRISdilateVertexToSum, mri_binarize --dilate-vertex-sum) counts the vertices of all its passes together, so it reports the target as not possible once its passes together have counted as many vertices as the surface has, though the last pass's disk holds fewer (a disk of a few hundred vertices on a whole hemisphere is still found; on a small surface the dilation stops early); when the vertex's part of the surface is used up before the target is reached it goes on with a pass for every vertex it counted, each pass over the whole part";
    FS_983 = "FS-983", "FreeSurfer", true,
        "dilating a vertex to a target sum (MRISdilateVertexToSum) with a target that needs no pass (not above 0 with a measure given, or not a number) builds its mask from hop lists of no hops: the program crashes";
    FS_984 = "FS-984", "FreeSurfer", true,
        "ripping a surface's unknown vertices (MRISripUnknown: mri_gtmpvc's surfaces) and the confined dilation of a surface mask with an annotation to stay in (MRISdilateConfined), for a surface without a colour table, decide each vertex by a colour-table index the program never computed (memory contents): the vertices ripped or dilated are arbitrary";
    FS_985 = "FS-985", "FreeSurfer", true,
        "the confined dilation of a surface mask with a new annotation (MRISdilateConfined) and the label of an annotation (MRISannotation_to_label: mris_hausdorff_dist) take the annotation of a colour-table index the table does not hold (no table, an index outside it or an empty place) from memory never written: the dilated vertices get an arbitrary annotation, and the label holds the vertices of an arbitrary one (or the program crashes)";
    FS_986 = "FS-986", "FreeSurfer", true,
        "removing a surface's ripped vertices from its lists (MRISremoveRipped: MRISreadPatch, mris_flatten, the label rips of mri_surf2surf and others) never updates the neighbour lists of the vertices that are kept: they keep the ripped vertices, and their distances, among their neighbours";
    FS_987 = "FS-987", "FreeSurfer", true,
        "renumbering a surface without its ripped faces and vertices (MRISrenumberRemovingRippedFacesAndVertices: mris_inflate's tearing) pairs each vertex's kept faces after a removed one with the vertex's place in the face before: the vertex's place in its faces is wrong from there";
    FS_988 = "FS-988", "FreeSurfer", true,
        "renumbering a surface without its ripped faces and vertices gives each kept vertex that moves the neighbour and face lists of the vertex that had its new number before (filtered by the rip flags of the moved vertices, a neighbour ripped with a flag other than 1 kept as -1): the renumbered surface's topology is wrong";
    FS_989 = "FS-989", "FreeSurfer", false,
        "removing an edge or a triangle link of a surface (MRISremoveTriangleLinks for quadrangle surfaces, the edge and link removals) leaves the counts of the 2- and 3-hop neighbours as they were, and removing an edge also the count of neighbours in use, so the last immediate neighbour is counted again after them; FreeSurfer's programs remove the links of quadrangle surfaces at one hop, where only the immediate counts are read";
    FS_990 = "FS-990", "FreeSurfer", false,
        "removing a link between two vertices (the tearing of mris_inflate and mris_flatten) rips the faces that hold both but removes no other link of those faces, so a link no face holds any more stays; removing a face removes a link that a face of the other direction still holds";
    FS_991 = "FS-991", "FreeSurfer", true,
        "the surface check of mris_topo_fixer (MRISisSurfaceValid) starts each vertex's corner check from its first neighbour, also for a vertex without neighbours: it marks a vertex named by memory never written, or crashes for a vertex without a neighbour list";
    FS_992 = "FS-992", "FreeSurfer", true,
        "removing a neighbour from a vertex's lists when the vertex has fewer neighbours in use than the neighbour's place (a neighbourhood size of 0) moves a negative count of entries: the memory around the vertex's list is overwritten (other vertices' lists corrupted) or the program crashes";
    FS_993 = "FS-993", "FreeSurfer", true,
        "the neighbours of a vertex in order around it (MRISgetSpatOrderedNeighbors: mris_skeletonize) read memory never written for a vertex without faces, and past a face's three vertices for a vertex its first face does not hold";
    FS_994 = "FS-994", "FreeSurfer", true,
        "renumbering a surface without its ripped faces and vertices copies each moved face's normal matrices and each moved vertex's user data without moving them: two places share them, and releasing the surface releases them twice";
    FS_995 = "FS-995", "FreeSurfer", true,
        "renumbering a surface without its ripped faces and vertices, when unripped faces hold ripped vertices or vertices have neighbours ripped with a flag other than 1, writes the vertex number -1 into those faces and neighbour lists: the program's later reads of them go outside the surface's vertices (memory contents or a crash)";
    FS_996 = "FS-996", "FreeSurfer", true,
        "the soap-bubble smoothing of a surface's positions (MRISsoapBubbleVertexPositions: mris_fix_topology, mris_make_surfaces, mris_smooth; the weighted one) reads the diagnostic vertex (Gdiag_no) before each pass without checking that the surface has it: a diagnostic vertex beyond the surface's vertices reads memory outside them (its report printed from there, or a crash)";
    FS_997 = "FS-997", "FreeSurfer", false,
        "the closest-vertex search of the surface hash table within one voxel (MHTfindClosestVertexNoXYZ: mri_surf2volseg, mri_vol2surf, mri_label2label, mris_ca_label, mris_place_surface, mris_register) returns no vertex (-1, at 1000 mm) for a point with no vertex within one voxel of the table, although its description promises a full search then; the programs take that answer as far from the surface, so it is kept";
    FS_998 = "FS-998", "FreeSurfer", true,
        "a closest-vertex search of the surface hash table on a table of faces (MHTfindClosestVertexGeneric, MHTfindClosestVertexNo2, MHTfindClosestVertex2, MHTfindClosestVertexSet2, MHTfindClosestVertexInTable), or a closest-face search on a table of vertices (MHTfindClosestFaceGeneric), is not refused: the face numbers are read as vertex numbers (or the other way round), and those beyond the surface's vertices (faces) read memory outside them";
    FS_576 = "FS-576", "FreeSurfer", true,
        "a closest search of the surface hash table on a table of the other kind (FS-998) reads, for a face number beyond the surface's vertices (or a vertex number beyond its faces), memory outside the surface's vertices (faces): its answer depends on memory contents, or the program crashes";
    FS_999 = "FS-999", "FreeSurfer", true,
        "the closest-vertex and closest-face searches of the surface hash table look for nothing beyond the table (1,000 of its voxels from the origin), although a vertex or face beyond the table is stored at its edge: a point at such a vertex does not find it";
    FS_1001 = "FS-1001", "FreeSurfer", true,
        "projecting a surface of the pial status back onto its pial positions (the surface deformation's projection step) crashes the program when the surface carries no face table, or when a vertex has no face within 1,000 mm (a vertex at NaN, a table without faces)";
    FS_1002 = "FS-1002", "FreeSurfer", true,
        "flattening a patch (MRISflattenPatch: mris_flatten) with DIAG_SHOW reports the diagnostic vertex (Gdiag_no) without checking that the surface has it: beyond the surface's vertices it reads memory outside them (a report printed from there, or a crash)";
    FS_1003 = "FS-1003", "FreeSurfer", true,
        "the flattening's report of the diagnostic vertex prints that vertex's original area on every neighbour's line instead of the neighbour's own";
    FS_1004 = "FS-1004", "FreeSurfer", true,
        "flattening a patch (MRISflattenPatch: mris_flatten) whose vertex normals sum to zero turns every unripped vertex's position into NaN";
    FS_1005 = "FS-1005", "FreeSurfer", true,
        "flattening a patch (MRISflattenPatch: mris_flatten) with DIAG_SHOW and a diagnostic vertex (Gdiag_no) that has neighbours crashes the program when the surface holds no vertex distances";
    FS_1006 = "FS-1006", "FreeSurfer", true,
        "reading a patch from a GIFTI file name (MRISreadPatch, MRISreadPatchNoRemove: mris_convert, mri_surf2surf, mris_curvature and other programs given a patch) reads the file as a new surface that it never returns: the program's surface is left whole (not a patch) and the read reports success; a GIFTI file that cannot be read crashes the program";
    FS_1007 = "FS-1007", "FreeSurfer", true,
        "reading an ASCII patch file (MRISreadPatch, MRISreadPatchNoRemove) that holds no line but comments and blank lines crashes the program";
    FS_1008 = "FS-1008", "FreeSurfer", true,
        "reading an ASCII patch file (MRISreadPatch, MRISreadPatchNoRemove) whose point line holds no vertex number, or whose coordinate line holds fewer than three numbers, silently takes the previous point's vertex number or coordinates for the missing values";
    FS_1009 = "FS-1009", "FreeSurfer", false,
        "the patch readers' messages (MRISreadPatch, MRISreadPatchNoRemove) print a bad vertex number of a patch file one below the file's (zero-based), and a label or volume vertex number equal to the surface's vertex count as greater than it; the messages are kept";
    FS_1010 = "FS-1010", "FreeSurfer", true,
        "reading an ASCII patch file (MRISreadPatch, MRISreadPatchNoRemove) whose first line holds no vertex count, or whose first point holds no vertex number or fewer than three coordinates, uses memory the program never wrote for the missing values";
    FS_1011 = "FS-1011", "FreeSurfer", true,
        "sampling a surface's coordinates at a point through its canonical (spherical) positions (MRISsampleFaceCoordsCanonical and the sampled energies of the surface deformation: mris_expand, mris_inflate, mris_place_surface, mris_register, mris_sphere) crashes the program when the point has no face within 1,000 mm (a point that is not finite, a face table without faces)";
    FS_1012 = "FS-1012", "FreeSurfer", true,
        "the sampled spring and parallel energies of the surface deformation (mris_expand, mris_register, mris_sphere, mris_place_surface) sample a face whose number is outside the surface's faces when no face is found within 1,000 mm, or when a vertex's assigned face is unset (-1) or beyond the faces; the walks through a face table's buckets (the ray along a vertex's normal: mris_expand, mris_sphere; the faces overlapping a face: mris_fix_topology, mris_topo_fixer) given a table of vertices read its vertex numbers as faces: they read memory outside the faces (memory contents or a crash)";
    FS_1013 = "FS-1013", "FreeSurfer", true,
        "finding the faces that overlap a face (mris_fix_topology, mris_topo_fixer) walks the face's bounding box in half-millimetre steps: a face kilometres across, or with a coordinate that is infinite, runs for hours or never ends";
    FS_1014 = "FS-1014", "FreeSurfer", true,
        "mri_normalize's removal of white-matter outliers (with -aseg, and with -L's longitudinal control points) reads the white-matter intensity peak from memory outside its histogram when the control volume holds no control point: the threshold is memory contents (0 with the C library's usual heap)";
    FS_1015 = "FS-1015", "FreeSurfer", true,
        "mri_normalize -T2 with -surface walks from each control point along the distance map's gradient in steps of half a voxel's size: an input volume whose voxel size is 0 or not a number never ends that walk when it meets no edge";
    FS_1016 = "FS-1016", "FreeSurfer", true,
        "mri_normalize given an option with fewer arguments than it takes (the option last on the command line) crashes when the missing argument is a number or a file it opens (-sigma, -seed, -atlas, -label, ...), and for an option of two or three arguments given none takes the missing ones after the first from the process's environment (printed, or read as numbers)";
    FS_1017 = "FS-1017", "FreeSurfer", true,
        "mri_normalize crashes when a step it relies on gives no volume: -conform's conforming or a resampling of -aseg's or -renorm's volume that fails, -mask_orig's volume that cannot be read, -no1d with -surface_interior after a one-dimensional normalisation that failed, or -r and -l without their volume names";
    FS_1018 = "FS-1018", "FreeSurfer", true,
        "mri_binarize --match (and its preset lists: --wm, --gm, --ventricles, ...) on more than one thread (--threads above 1, or OMP_NUM_THREADS above 1 or unset) shares one counter of the match values between its threads: voxels can be matched or missed at random";
    FS_1019 = "FS-1019", "FreeSurfer", true,
        "mri_binarize --rmax prints `Setting max thresh to` with the minimum threshold instead of the maximum it sets";
    FS_1020 = "FS-1020", "FreeSurfer", true,
        "mri_binarize --fdr goes on when no false-discovery-rate threshold can be computed (an input that is not float, no voxel in the mask or of the sign asked for): it prints and thresholds with a number it never set (memory contents)";
    FS_1021 = "FS-1021", "FreeSurfer", true,
        "mri_binarize --fix-vol-topo crashes when the binarised volume is already topologically correct (its surface's Euler number is 2): the volume it then writes has been freed";
    FS_1022 = "FS-1022", "FreeSurfer", true,
        "mri_binarize --fix-vol-topo can remove the binarised volume's first voxel (in column, row, slice order) while it erodes the filled volume back: every other voxel of the binarisation is kept, as the fix intends";
    FS_1023 = "FS-1023", "FreeSurfer", true,
        "mri_binarize keeps at most 1,000 match values, replacements and nearest-neighbour replacements: more (--match-ctab with FreeSurfer's whole colour table, long --match lists) are written over its other settings: other options change, or the program crashes";
    FS_1024 = "FS-1024", "FreeSurfer", true,
        "mri_binarize --dilate-vertex with fewer than four arguments prints the usage line of --dilate-vertex-sum";
    FS_1025 = "FS-1025", "FreeSurfer", true,
        "mri_binarize crashes when a step gives no volume and the program goes on with it: --remove-islands or --fill-holes with a binary value of 0 (--inv, --gm, --binval 0), --fix-vol-topo after more dilations than it allows (a binarisation without any voxel)";
    FS_1026 = "FS-1026", "FreeSurfer", true,
        "mri_binarize --crop-around-ras, --dilate-vertex(-sum) and --match-ctab take a number that cannot be read (the centre, the field of view, a value kept or left out, the vertex, the radius or target) from memory contents";
    FS_1027 = "FS-1027", "FreeSurfer", true,
        "mri_binarize --surf and --fix-vol-topo keep about 1.25 GB of address space per tessellation of the volume that they never release: --fix-vol-topo, which tessellates at each of its steps, ends after some tens to hundreds of steps (the address space or the memory limit decides) with `Cannot calloc()` and exit status 0 without writing anything, or with a crash";
    FS_1101 = "FS-1101", "FreeSurfer", true,
        "mri_segstats cuts a file name it builds of 1,000 bytes or more (eTIV's talairach.xfm or --talxfm's transform, --euler's lh.orig.nofix) at 999 bytes and prints `main: Truncation on line <n>` on standard error: the shorter name is then read; larmorx reads the whole name";
    FS_1102 = "FS-1102", "FreeSurfer", true,
        "mri_segstats writes a file name of 1,000 bytes or more that it builds from the subjects' folder or FREESURFER_HOME (--annot, --slabel, --euler's rh.orig.nofix, the ribbon, --ctab-default; --qa-stats's names of 4,000 bytes or more) past the memory it sets aside for it (undefined behaviour); larmorx uses the whole name";
    FS_1103 = "FS-1103", "FreeSurfer", true,
        "mri_segstats --mul, --div and --rescale-by-seg (its target mean or an id) take a number that cannot be read from memory contents";
    FS_1104 = "FS-1104", "FreeSurfer", true,
        "mri_segstats --vox with one or two numbers reads past the command line and crashes";
    FS_1105 = "FS-1105", "FreeSurfer", true,
        "mri_segstats --annot and --xfm2etiv with too few arguments say that they need 1 argument (they need 3 and 2)";
    FS_1106 = "FS-1106", "FreeSurfer", true,
        "mri_segstats --mask writes the mask's frame (--maskframe) on the summary file's MaskInvert line instead of whether the mask was inverted (--maskinvert)";
    FS_1107 = "FS-1107", "FreeSurfer", true,
        "mri_segstats --gtm-default-seg-merge-choroid leaves out the default merge list's last two replacements, 62 and 80 into 0 (the right vessel and the non-white-matter hypointensities), as if they were the choroid plexus's, which the list no longer holds: those two labels stay unmerged";
    FS_1108 = "FS-1108", "FreeSurfer", true,
        "mri_segstats --replace-file after other replacements (--replace, --gtm-default-seg-merge) counts the earlier ones twice: it prints their total as the number it read, accepts a file of none, and adds as many replacements of 0 into 0 (or of what --gtm-default-seg-merge-choroid left out)";
    FS_1109 = "FS-1109", "FreeSurfer", true,
        "mri_segstats keeps at most 1,000 ids (--id), excluded ids (--exclude, --excl-ctxgmwm) and replacements (--replace, --replace-file, --gtm-default-seg-merge): more are written over its other settings (undefined behaviour)";
    FS_1110 = "FS-1110", "FreeSurfer", true,
        "mri_segstats --slabel reports segmentation 0 (the surface's vertices outside the label) though it is meant to exclude it: the summary lists it and its ExcludeSegId line names no id";
    FS_1111 = "FS-1111", "FreeSurfer", true,
        "mri_segstats crashes when a file it writes cannot be opened: --ctab-out, --sfavg and --qa-stats's file";
    FS_1112 = "FS-1112", "FreeSurfer", true,
        "mri_segstats --qa-stats crashes when the subject's aparc+aseg.mgz or norm.mgz cannot be read";
    FS_1113 = "FS-1113", "FreeSurfer", true,
        "mri_segstats aborts (std::logic_error) where it reads the brain volume statistics without a subject's name or SUBJECTS_DIR: the surface volumes or --brain-vol-from-seg without --subject (when (null)/(null)/mri/ribbon.mgz exists), --qa-stats without SUBJECTS_DIR";
    FS_1114 = "FS-1114", "FreeSurfer", true,
        "mri_segstats --annot of an annotation without a colour table crashes";
    FS_1115 = "FS-1115", "FreeSurfer", true,
        "mri_segstats crashes when a step gives no volume and the program goes on with it: --seg-from-input with an input whose header cannot be read";
    FS_1116 = "FS-1116", "FreeSurfer", true,
        "mri_segstats --id with a colour table: an id outside the table given first is accepted or refused by memory contents; larmorx refuses it (ERROR: cannot find seg id)";
    FS_1117 = "FS-1117", "FreeSurfer", true,
        "mri_segstats --ctab-gca writes a GCA label of 1,000 or more past the list it marks labels in (undefined behaviour), and crashes on a label that FreeSurferColorLUT.txt does not hold; larmorx keeps every label and ends with an error for one the table lacks";
    FS_1118 = "FS-1118", "FreeSurfer", true,
        "mri_segstats --id with a colour table: an id outside the table after other ids is accepted or refused by the answer for the id before it (an accepted one is reported with an empty name and colour); larmorx refuses it (ERROR: cannot find seg id)";
    FS_1119 = "FS-1119", "FreeSurfer", true,
        "mris_calc ends with a buffer-overflow abort (signal 6) where a name or a message built from one passes 65,535 bytes (the output or label name, an input name that is no file, the type and write messages); larmorx uses the name whole";
    FS_1120 = "FS-1120", "FreeSurfer", true,
        "mris_calc ends with status 0 when its output (volume, curvature, ascii or statistic file) cannot be written, after a [ error ] mark, an ERROR or Write error line, or nothing at all; larmorx ends with could not establish write access (status 252)";
    FS_1121 = "FS-1121", "FreeSurfer", true,
        "mris_calc combines two volumes of other dimensions holding as many values value by value in its packing order and writes the second volume's dimensions; larmorx refuses them with the size mismatch (status 10)";
    FS_1122 = "FS-1122", "FreeSurfer", true,
        "mris_calc with an operation of two values (add, mul, set, masked, ...) and no second input crashes (signal 11); larmorx ends with a message (status 10)";
    FS_1123 = "FS-1123", "FreeSurfer", true,
        "mris_calc with an option that takes a value (-o, -l, --verbosity, threads) given last crashes (signal 11); larmorx ends with a message (status 1)";
    FS_1124 = "FS-1124", "FreeSurfer", true,
        "mris_calc threads (nthreads) with a value that is no number prints a thread count from memory never written (measured 0); larmorx prints 0";
    FS_1125 = "FS-1125", "FreeSurfer", true,
        "mris_calc --label with a point whose vertex number is outside input 1's values writes outside its mask (memory corruption; such points never count); larmorx refuses the label (status 10)";
    FS_1126 = "FS-1126", "FreeSurfer", true,
        "mris_calc prints the index of the minimum and the negative, zero and positive counts through a float, and keeps a label's indices as floats: beyond 16,777,216 values they are rounded (a label's values then land at a rounded index); larmorx keeps them exact";
    FS_1127 = "FS-1127", "FreeSurfer", true,
        "mri_make_uchar given only an input volume and a transform (no output name) converts the whole volume, then crashes writing it (signal 11; what it printed to a file or pipe is lost): larmorx prints the usage and ends with status 1 before reading anything";
    FS_1128 = "FS-1128", "FreeSurfer", true,
        "mri_make_uchar's -f, -w or -r given as the last argument, without its value, crashes (signal 11): larmorx prints the usage and ends with status 1, as for -h and -v given last";
    FS_1129 = "FS-1129", "FreeSurfer", true,
        "mri_make_uchar ends with status 0 when its output volume, the ball volume of -v or the histogram file of -h cannot be written: larmorx ends with status 1 after the writer's messages";
    FS_1130 = "FS-1130", "FreeSurfer", true,
        "mri_make_uchar given a volume whose slices are not coronal prints the error 'MRIvoxelToTalairachVoxel: unsupported slice direction' (after the last system error's message) once for every voxel, although the point it fails to compute is never used and the result is right: larmorx prints none";
    FS_1131 = "FS-1131", "FreeSurfer", true,
        "mri_make_uchar whose ball around the Talairach origin gives both percentiles in histogram bins of one value (no voxel in the ball, one intensity in it) or whose percentile is not a number divides by zero: its intensity line is not finite and it writes a volume of zeros with status 0: larmorx ends with an error (status 1) and writes no output";
    FS_1132 = "FS-1132", "FreeSurfer", true,
        "mri_mask given an option that takes numbers or a file (-dilate, -erode, -T, -threshold, -crop-to-fov, -crop-to-fov-mm, -BB, -crop-crs, -BBM, -BBMM, -BBEQ, -transfer, -oval, -lta_src, -lta_dst, -DEBUG_VOXEL, -bgnoise) as its last arguments, without all its values, reads past its arguments and crashes (signal 11): larmorx prints that the option needs its values and the help, status 1";
    FS_1133 = "FS-1133", "FreeSurfer", true,
        "mri_mask -BBEQ whose bounding box cannot be made square with an odd number of slices (its columns more than the volume's rows, its rows more than the volume's columns, or an even number of slices spanning the whole depth from the first) prints its ERROR line and crashes (signal 11; what it printed to a file or pipe is lost): larmorx ends with an error, status 1";
    FS_1134 = "FS-1134", "FreeSurfer", true,
        "mri_mask -xform with a transform file of several transforms that do not chain (each one's source or destination the previous one's destination) prints 'ERROR: LTAreduce(): LTs <n> and <n+1> do not match' and crashes (signal 11): larmorx ends with an error, status 1";
    FS_1135 = "FS-1135", "FreeSurfer", true,
        "mri_mask -xform with a transform that inverts but whose voxel-to-voxel form for the two volumes is singular in single precision prints 'MRIlinearTransform: xform is singular' and goes on without a mask: it crashes (signal 11) with -T, -threshold, -invert, -samseg, -lh, -rh, the crops or -dilate, else ends with 'must have a valid MRI (src)' (status 252): larmorx ends with an error at once, status 1";
    FS_1201 = "FS-1201", "FreeSurfer", true,
        "mri_cc on an aseg whose hemispheres' white matter spans more than 500 columns between them (a volume about 500 voxels wide or more) writes past its table of slices: larmorx ends with an error in both modes";
    FS_1202 = "FS-1202", "FreeSurfer", true,
        "mri_cc removing the fornix from a sagittal slice in which it finds no fornix (it prints 'WARNING: min_x_fornix not set') computes the callosum's maximum thickness from no measurement and can shave callosum voxels of the slice's last columns: larmorx shaves nothing there";
    FS_1203 = "FS-1203", "FreeSurfer", true,
        "mri_cc given an option that takes a value (-sdir, -DEBUG_VOXEL, -aseg, -norm, -lta, -m, -d, -s, -a, -o, -t) as its last argument, without its value, reads past its arguments and crashes (signal 11): larmorx prints that the option needs its values and the help, status 1";
    FS_1204 = "FS-1204", "FreeSurfer", true,
        "mri_cc ends with status 0 when its output aseg, the transform of -lta or (with -a 0) cc_org.mgz cannot be written: larmorx ends with status 1 after the writer's messages";
    FS_1205 = "FS-1205", "FreeSurfer", true,
        "mri_cc -a 0 (the white-matter path) whose mri/wm does not read (it is not there, or it is a file whose format is not found without a suffix: recon-all writes wm.mgz) goes on and crashes (signal 11) once its Talairach transform is read: larmorx ends with 'could not read white matter volume from <file>', the status of a missing input";
    FS_1206 = "FS-1206", "FreeSurfer", true,
        "mri_cc -a 0 on voxels smaller than 0.5 mm makes more sagittal slices than its tables hold (42) and writes past them: larmorx ends with an error in both modes";
    FS_1207 = "FS-1207", "FreeSurfer", true,
        "mri_cc -a 0 on voxels larger than 1.3125 mm makes fewer sagittal slices (fewer than 17) than its choice of the midline reads, and reads areas it never computed (memory contents): larmorx reads them as 0 in both modes, with a notice";
    FS_1208 = "FS-1208", "FreeSurfer", true,
        "mri_cc -a 0's cut of the fornix compares the width of its first row with that of the row before it, which it never measured (memory contents): larmorx takes that width as 0 in both modes, with a notice";
    FS_1209 = "FS-1209", "FreeSurfer", true,
        "mri_cc -a 0 on a volume taller than 258 voxels or of more than 257 slices never ends (the cut of the fornix searches its sagittal slices only up to voxel 256): larmorx ends with an error in both modes";
    FS_1211 = "FS-1211", "FreeSurfer", true,
        "mri_cc -d with more than 50 divisions crashes (signal 11) in its own error message, which reads the count as text, and loses the messages before it: larmorx prints 'too many CC divisions specified (max = 50)', the status of an unsupported request";
    FS_1212 = "FS-1212", "FreeSurfer", true,
        "mri_cc given a name of 1,024 bytes or more (-sdir, -aseg, -norm, -lta, -o, or SUBJECTS_DIR) aborts in its checked copy into a 1,024-byte buffer (*** buffer overflow detected ***, signal 6) and loses its messages: larmorx keeps the name whole";
    FS_1213 = "FS-1213", "FreeSurfer", true,
        "mri_cc -d 0 or a negative count counts each callosum voxel into the element before its table of divisions (memory outside it) and labels the whole callosum with one label: larmorx refuses such a count by default";
    FS_1214 = "FS-1214", "FreeSurfer", true,
        "mri_cc's volume measurement file (written only with the diagnostic flags DIAG_WRITE and DIAG_VERBOSE) prints five divisions; with fewer than five (-d 1 to 4) it prints counts it never made (memory contents): larmorx prints 0 for them in both modes, with a notice";
    FS_1215 = "FS-1215", "FreeSurfer", true,
        "mri_cc -t with a half thickness that takes the callosum's slab past the volume's last column reads other voxels of the volume as the slab's (the next rows') and, further out, memory past the volume, where it crashes (a large -t: signal 11): larmorx refuses such a thickness by default";
    FS_1216 = "FS-1216", "FreeSurfer", true,
        "mri_segment -polvlen with more than 200 positions along a voxel's normal (a length above 24.75; a position every 0.25) writes past FreeSurfer's tables at the first ambiguous voxel it does not decide at the centre (memory corruption): larmorx refuses such a length by default and, with reproduce, ends with an error where FreeSurfer writes past them";
    FS_1217 = "FS-1217", "FreeSurfer", true,
        "mri_segment -polvwsize even or negative (from -16 to 16) takes each plane's median from more values than it samples, the rest memory contents: larmorx refuses such a window by default; with reproduce the values never sampled are read as 0";
    FS_1218 = "FS-1218", "FreeSurfer", true,
        "mri_segment -polvwsize of 17 or more (or -17 or less) writes or reads past FreeSurfer's table of a plane's 256 values at the first ambiguous voxel: larmorx refuses such a window by default and, with reproduce, ends with an error there";
    FS_1219 = "FS-1219", "FreeSurfer", true,
        "mri_segment given an option that takes values (-slope, -pslope, -nslope, -debug_voxel, -ghi, -glo, -wlo, -wm_low_factor, -whi, -nseg, -thicken, -seg, -polvwsize, -min-wm-mask, -polvlen, -wsizemm, -diagno, -n, -t, -p, -x, -w and the other options of those first letters) as its last arguments, without all its values, reads past its arguments and crashes (signal 11): larmorx prints that the option needs its values and the help, status 1";
    FS_1220 = "FS-1220", "FreeSurfer", true,
        "mri_segment -b <sigma> (any option starting with b) takes the sigma of the smoothing before the removal of voxels of the wrong direction from the option's own name (0) instead of its value: larmorx uses the value";
    FS_1221 = "FS-1221", "FreeSurfer", true,
        "mri_segment -seg <aseg> Qwm Qctx NdilWM NdilCtx reads aseg.presurf.mgz of the working folder whatever name it is given (it prints the name given): larmorx reads the named file";
    FS_1222 = "FS-1222", "FreeSurfer", true,
        "mri_segment -seg with an aseg whose dimensions differ from the input's reads the two volumes at each other's voxels (other voxels, memory outside them): larmorx ends with an error in both modes";
    FS_1223 = "FS-1223", "FreeSurfer", true,
        "mri_segment -min-wm-mask with a mask whose dimensions differ from the input's labels the input's voxels in the mask (past its voxels when it is smaller: memory corruption): larmorx ends with an error in both modes";
    FS_1224 = "FS-1224", "FreeSurfer", true,
        "mri_segment -min-wm-mask does not change the segmentation: FreeSurfer sets every voxel of the mask to 1 and then writes its first labels over it, so the mask's contents are never used (kept: the option has no effect, as in FreeSurfer)";
    FS_1225 = "FS-1225", "FreeSurfer", true,
        "mri_segment -min-wm-mask with a mask that is not a volume of bytes reads and writes the labels written into it byte by byte (other voxels' bytes): larmorx does not use the mask by default (it has no effect, FS-1224); with reproduce the labels are read and written as FreeSurfer does";
    FS_1226 = "FS-1226", "FreeSurfer", true,
        "mri_segment ends with status 0 when its output cannot be written: larmorx ends with status 1 after the writer's messages";
    FS_1227 = "FS-1227", "FreeSurfer", true,
        "mri_segment -fillv on a volume in which it finds no ventricle (no segment of 100 voxels or more over 7 slices or more) crashes (signal 11): larmorx adds no ventricle";
    FS_1228 = "FS-1228", "FreeSurfer", true,
        "mri_segment -fillbg or -fillv on a volume whose slices are not coronal uses Talairach coordinates it never computed (memory contents), after the message MRIvoxelToTalairachVoxel: unsupported slice direction: larmorx refuses such a volume by default; with reproduce the coordinates are 0";
    FS_1229 = "FS-1229", "FreeSurfer", true,
        "mri_segment --VERSION (its own version option, in any case other than --version) prints the placeholder Version: ##version## on standard error and ends: kept";
    FS_1230 = "FS-1230", "FreeSurfer", true,
        "mri_segment -thickenonly (or -thicken with a value above 1) does not thicken: it writes the input with its one-dimensional strands removed (the thickening is switched off in FreeSurfer): kept";
    FS_1231 = "FS-1231", "FreeSurfer", true,
        "mri_segment -seg whose relabelled aseg holds no white matter (it prints ERROR: could not find any WM voxels) goes on with the white matter's statistics 0 and the cortex's taken from memory it never set: larmorx ends with an error by default; with reproduce the cortex's are 0 too";
    FS_1232 = "FS-1232", "FreeSurfer", true,
        "mri_segment -wm_low_factor f sets the bottom of the white matter range to (1 - f) gray_mean + f white_mean without -seg but to f gray_mean + (1 - f) white_mean (its help's formula) with -seg: kept";
    FS_1233 = "FS-1233", "FreeSurfer", true,
        "finding bright voxels outside the white matter (MRIfindBrightNonWM) with a T1 or white-matter volume that is not of bytes (mris_make_surfaces' T1, mris_place_surface's --invol and --wm) reads both, and writes and dilates its labels, as the bytes of the volumes' rows: the labels land on other voxels or are lost (a float T1 never shows a bright voxel); larmorx takes the T1's values as bytes (FS-802) and the white matter's values, and writes the labels (100, 130) in the T1's type";
    FS_1234 = "FS-1234", "FreeSurfer", false,
        "mri_edit_wm_with_aseg's search for paths from the hippocampus, amygdala and inferior lateral ventricle to cortex marks its working volumes, made like the segmentation, through their bytes, and reads them by their bytes on the right and by their values on the left (its region's lateral border is also taken anew for each column, so that trim removes nothing): with a segmentation that is not of bytes (recon-all's aseg.presurf.mgz: int in the classic stream, float in v8) the search finds no path and adds no voxel ('0 voxels added to wm to prevent paths from MTL structures to cortex' on every recorded head); kept: with the segmentation as bytes (the search then runs as written) it adds no voxel on two recorded heads either";
    FS_1236 = "FS-1236", "FreeSurfer", true,
        "mri_edit_wm_with_aseg reads and writes voxels outside the white-matter and T1 volumes (the slice before the first or after the last, a column past a row's end) when the structures it edits touch the volume's border, and walks past its index tables for volumes wider than 500 voxels: memory it does not own; larmorx reads 0 there and writes nowhere (a column past a row's end: FreeSurfer's next or previous row with reproduce, the row's border voxel by default)";
    FS_1237 = "FS-1237", "FreeSurfer", true,
        "mri_edit_wm_with_aseg's second spackling of the hippocampus, for a voxel whose lateral-inferior neighbour at x + 1 is off and brighter, turns on the voxel at x - 1 instead (its other three neighbours are handled as intended); larmorx turns on the x + 1 neighbour";
    FS_1238 = "FS-1238", "FreeSurfer", true,
        "mri_edit_wm_with_aseg -keep-hailvcp lh rh reads its two values but does not take them from the arguments, so they become the input file names (or, when negative, unknown options): the option cannot be used; larmorx takes the two values";
    FS_1239 = "FS-1239", "FreeSurfer", true,
        "mri_edit_wm_with_aseg with an option given without all its values (-fillven, -fix-scm-ha, -fix-scm-ha-only, -fix-ento-wm, -sa-fix-ento-wm, -label-acj, -fix-acj, -sa-fix-acj, -keep-hailvcp, -debug_voxel) reads past its arguments and crashes (exit status 139); larmorx prints a clear error and the help, exit status 1";
    FS_1240 = "FS-1240", "FreeSurfer", true,
        "mri_edit_wm_with_aseg ends with exit status 0 when its output cannot be written (a folder that does not exist): larmorx ends with 1";
    FS_1210 = "FS-1210", "FreeSurfer", true,
        "mri_cc -a 0 whose search for the callosum's seed fails (no column of the callosum's thickness near Talairach's origin, or 'could not find valid seed for the cc') uses the seed's Talairach millimetres as a voxel position (only the printed centre and the division counts depend on it): larmorx uses the seed's voxel";
    FS_974 = "FS-974", "FreeSurfer", true,
        "sampling a surface's long-range neighbours (MRISsampleDistances: mris_register, mris_sphere, mris_curvature) with the progress report on (Gdiag's heartbeat bit) divides by zero on a surface of fewer than 10 vertices: the program crashes (signal 8)";
    FS_975 = "FS-975", "FreeSurfer", true,
        "sampling long-range neighbours at each distance (MRISsampleAtEachDistance) writes its counts past its array of 200 for a neighbourhood of 200 or more (MRISsampleDistances then stops at 100)";
    FS_976 = "FS-976", "FreeSurfer", true,
        "sampling long-range neighbours (MRISsampleDistances) with counts that sum below 0, or more than 32,767 neighbours for one vertex, reads and writes outside the vertex's neighbour list: the program crashes or computes garbage";
    FS_977 = "FS-977", "FreeSurfer", true,
        "sampling long-range neighbours (MRISsampleDistances) with a diagnostic vertex (Gdiag_no) beyond the surface's vertices, or a ripped one without original distances, reads outside them for its log file";
    FS_971 = "FS-971", "FreeSurfer", true,
        "the median filters of a surface's values (MRISmedianFilterVals, MRISmedianFilterVal2s, MRISmedianFilterVal2baks, MRISmedianFilterCurvature, MRISmedianFilterD) write past their list of 1,024 values for a vertex with more than 1,023 unripped neighbours in use";
    FS_972 = "FS-972", "FreeSurfer", true,
        "the mode filters of a surface's values (MRISmodeFilterVals, MRISmodeFilterZeroVals) count a neighbour whose value rounds above the largest value truncated (2.6 next to a largest value of 2.6: 3 above 2) outside their table, so that value is never chosen, and the program may crash";
    FS_973 = "FS-973", "FreeSurfer", true,
        "the mode filter of a surface's zero values (MRISmodeFilterZeroVals) never ends when a vertex's value is not a number and a vertex of value 0 next to it keeps being processed: the value changes at every pass, as NaN is never equal to itself, and the program prints a line per pass for ever";
    FS_221 = "FS-221", "FreeSurfer", true,
        "the volume format probe (mri_identify, the format tests and the volume-name helpers) misbehaves for a file name of 1,024 bytes or more (a Bruker study's file names of 512 or more, the stem helpers' names of 2,000 or more): its answer is undefined and the program can crash";
    FS_222 = "FS-222", "FreeSurfer", true,
        "is_mnc, for a name whose only dot is its first character followed by gz (.gz), reads one byte of memory outside the name (its answer is not changed)";
    FS_223 = "FS-223", "FreeSurfer", true,
        "is_mgh compares the whole file name with .mgh, .mgz and .mgh.gz instead of the name's end, so no ordinary name is recognised as MGH (no program calls it)";
    FS_224 = "FS-224", "FreeSurfer", true,
        "is_signa leaves a file shorter than a GE Signa header (14,336 bytes) open: a program that probes many such files runs out of file descriptors";
    FS_225 = "FS-225", "FreeSurfer", true,
        "a GE Signa header's 16-bit integer whose low byte is 0x80 or more reads as that byte minus 256 whatever its high byte (a matrix of 128 reads as -128), so such Signa files are not recognised and their fields are wrong";
    FS_226 = "FS-226", "FreeSurfer", true,
        "IDextensionFromName takes a name's last letters as its volume extension without the dot before them: a name ending in nii, mgz, mgh, img, bhdr or nii.gz without a dot (brainnii) is taken for a volume of that format, so a surface file so named is read or written as a volume";
    FS_227 = "FS-227", "FreeSurfer", true,
        "IsSiemensDICOM, for a DICOM file whose Manufacturer element (0008,0070) is stored with a numeric representation (US, SS, UL, SL, FL, FD, AT) and holds more bytes than one such number, corrupts memory: the answer depends on memory contents and the program can crash";
    FS_228 = "FS-228", "FreeSurfer", true,
        "IsSiemensDICOM crashes on a DICOM file whose Manufacturer element (0008,0070) has a representation it does not handle (OT, UT, DD, a retired, context or delimiter representation) or an undefined length";
    FS_249 = "FS-249", "FreeSurfer", true,
        "reading a volume (MRIread, MRIreadEx, mri_read, MRIreadHeader) whose name is 1,024 bytes or more (for mri_read and MRIreadHeader, once the working directory is put before it) writes past FreeSurfer's name buffers: the result is undefined and the program can crash";
    FS_253 = "FS-253", "FreeSurfer", true,
        "reading an MGH or MGZ volume that holds the NIfTI header extension's RAS-transform tag (tag 9) crashes";
    FS_254 = "FS-254", "FreeSurfer", true,
        "reading an MGH or MGZ volume whose talairach-transform tag holds 1,024 bytes or more, or whose folder's name is 975 bytes or more, writes past FreeSurfer's name buffers: the transform's name and the volume after it are undefined and the program can crash";
    FS_255 = "FS-255", "FreeSurfer", true,
        "reading only the header of a float volume (MRIreadHeader, MRIreadInfo) with a frame list (name#0) crashes";
    FS_256 = "FS-256", "FreeSurfer", true,
        "with FS_MGZIO_USEVOXELBUFREAD set, reading one frame after the first of an MGH volume writes past the one-frame volume FreeSurfer makes for it, and reading a volume without slices (no frames, or a depth of 0) crashes";
    FS_257 = "FS-257", "FreeSurfer", true,
        "an MGH volume's command-line or phase-encoding tag whose length is negative or larger than the machine can allocate crashes FreeSurfer or corrupts its memory";
    FS_258 = "FS-258", "FreeSurfer", true,
        "regio_read_register writes past its buffer for a subject or method name of 1,000 bytes or more in a register.dat, and gives sizes from memory contents for an .lta without transforms";
    FS_259 = "FS-259", "FreeSurfer", true,
        "reading a Bruker study, a parameter line (##name=value in acqp, reco or d3proc) whose name is 256 bytes or more or whose value is 128 bytes or more writes past FreeSurfer's buffers: memory corruption, a crash or wrong values";
    FS_260 = "FS-260", "FreeSurfer", true,
        "reading a Bruker study leaves its reco file open (and d3proc or reco on an unsupported data type, 2dseq on an unknown one): a program that reads many studies runs out of file descriptors";
    FS_401 = "FS-401", "FreeSurfer", true,
        "reading a Bruker study whose files lack a value FreeSurfer uses (the sizes in d3proc; the transposition, field of view, sizes, data type in reco; TR, TE, TI, the flip angle, the offsets or the gradient matrix in acqp) takes it from memory contents: the volume's size, type, times or geometry are undefined";
    FS_402 = "FS-402", "FreeSurfer", true,
        "reading a Bruker study's voxels takes 2 bytes per voxel whatever the data type: a 32-bit volume (RECO_wordtype _32BIT_SGN_INT) gets only the first half of each row from the file and zeros after it";
    FS_403 = "FS-403", "FreeSurfer", true,
        "reading a Bruker study whose 2dseq file is shorter than the volume crashes (the volume is freed twice)";
    FS_404 = "FS-404", "FreeSurfer", true,
        "reading a Bruker study ignores RECO_byte_order: a big-endian 2dseq is read in the machine's byte order, so its values are wrong";
    FS_405 = "FS-405", "FreeSurfer", true,
        "FreeSurfer's 2-D image functions (allocating, clearing an area, copying frames, scaling, the pyramid reduction) read and write outside an image's pixels when given a frame, an area, a size or a buffer the image does not have: memory corruption, a crash or memory contents";
    FS_406 = "FS-406", "FreeSurfer", true,
        "FreeSurfer's 2-D image functions turn a float beyond the target type's range, or NaN, into a pixel byte or a pixel coordinate without a check: a fill value or a float pixel beyond the byte range becomes the low byte of -2147483648 or of its integer part, a coordinate -2147483648";
    FS_407 = "FS-407", "FreeSurfer", true,
        "ImageCopyFrames, converting frames (bytes into floats or integers, integers into anything), writes each frame into the output frame of the same number, not into the destination frame it was given";
    FS_408 = "FS-408", "FreeSurfer", true,
        "ImageScaleDown of a float image of more than one frame by a power of two (0.5, 0.25, ...) reduces only the first frame: the output's other frames are left as they were";
    FS_409 = "FS-409", "FreeSurfer", true,
        "ImageRescale by 1 with a given output smaller than its input returns that output after freeing it, without the copy: memory contents or a crash";
    FS_410 = "FS-410", "FreeSurfer", true,
        "ImageAlloc, when the pixels cannot be allocated, ends the program with the code of FreeSurfer's previous error, 0 when there was none: a program that ran out of memory exits with status 0 (success)";
    FS_411 = "FS-411", "FreeSurfer", true,
        "a newer-style FreeSurfer program without a help text (mri_brainvol_stats) run without a required input prints, before its error, a help line chosen by memory never written: No xml string passed. when that memory holds 0, otherwise an XML parser's message";
    FS_412 = "FS-412", "FreeSurfer", true,
        "ImageScaleUp, ImageScaleDown and ImageRescale with a scale that makes the image's rows or columns beyond the 32-bit integer range (or NaN) go on with extents of -2147483648: nothing is scaled, or the image cannot be allocated, with no message about the scale";
    FS_413 = "FS-413", "FreeSurfer", true,
        "reading a text colour table, an entry line whose structure number is negative, or is the table's largest while its name holds a #, is stored outside the table: memory corruption, a crash or another object's values (a smaller number with a # in its name is read as an entry)";
    FS_414 = "FS-414", "FreeSurfer", true,
        "reading a text colour table's tissue-type lines (#ctTType): a line of vertical tabs, form feeds or carriage returns only, or a #ctTType line with nothing after the keyword, crashes FreeSurfer; a #ctTType line whose structure number, name, colour or transparency does not read takes that value from an earlier such line or from memory never written, and one whose structure number is negative or does not read can be stored outside the table";
    FS_415 = "FS-415", "FreeSurfer", true,
        "reading a text colour table without tissue-type lines leaves the file open: a program that reads many tables runs out of file descriptors";
    FS_416 = "FS-416", "FreeSurfer", true,
        "reading the default colour table (CTABreadDefault), and adding a tissue-type schema without a table, crash when FreeSurfer's environment cannot be read: FREESURFER_HOME not set, its FreeSurferColorLUT.txt missing or unreadable, or a wrong FS_DESIRED_BVEC_SPACE";
    FS_417 = "FS-417", "FreeSurfer", true,
        "a table of random colours of no entries (CTABalloc of 0), and drawing new colours for or counting the repeated colours of a table with empty places, crash FreeSurfer";
    FS_418 = "FS-418", "FreeSurfer", true,
        "merging a colour table with more places into a smaller one (CTABmerge) writes past the smaller table: memory corruption or a crash";
    FS_419 = "FS-419", "FreeSurfer", false,
        "the tissue-type schemas' table of tissue classes keeps the random float colours it was made with: its float colours do not match its integer ones (kept: no FreeSurfer program reads them)";
    FS_420 = "FS-420", "FreeSurfer", true,
        "a text colour table whose largest structure number (or tissue-type number) is beyond the machine's memory, or a table of random colours of such a count, gives an error message, a crash or a table depending on the machine's memory";
    FS_421 = "FS-421", "FreeSurfer", true,
        "reading an MGH volume whose talairach tag's text has no NUL byte (and fewer than 1,024 bytes) takes the transform's name on past the text from memory never written: a garbled name, so the transform is not found or another file is read";
    FS_422 = "FS-422", "FreeSurfer", true,
        "reading an MNI transform file (regio_read_mincxfm) whose first line is not MNI Transform File leaves the file open: a program that reads many runs out of file descriptors";
    FS_423 = "FS-423", "FreeSurfer", true,
        "reading a label file whose first line's text after vox2ras= is 100 bytes or more writes the text past the label's coordinate-space field: memory corruption, wrong label fields or a crash when the label is freed";
    FS_424 = "FS-424", "FreeSurfer", true,
        "reading a label file whose point count is 0 while point lines follow writes those points outside the label's memory: memory corruption or a crash (the label returned has no points)";
    FS_425 = "FS-425", "FreeSurfer", true,
        "reading a label file whose point count is larger than its point lines gives a label whose missing points are vertex 0 at (0, 0, 0) with statistic 0, without a message";
    FS_426 = "FS-426", "FreeSurfer", true,
        "a label of more points than the machine's memory holds (a point count read from a file, or a label allocated or enlarged to it) gives an error message, a crash or a label depending on the machine's memory";
    FS_427 = "FS-427", "FreeSurfer", true,
        "reading, writing or making a label, a label name, subject name or subjects directory longer than FreeSurfer's 1,024-byte name buffers is written past them, or read on past its end: memory corruption, a garbled name or a crash";
    FS_428 = "FS-428", "FreeSurfer", true,
        "writing a label, a write error (a full disk) closes the file twice: a crash or memory corruption after the error message";
    FS_429 = "FS-429", "FreeSurfer", true,
        "copying a label into a given label with fewer places than the source's points writes the points past the given label's memory: memory corruption or a crash";
    FS_430 = "FS-430", "FreeSurfer", true,
        "the annotation tables (a table file, or one set from a colour table): a name of 100 bytes or more, or a table path of 1,024 bytes or more, is written past its buffer or read on past its end: memory corruption, a garbled name or a crash";
    FS_431 = "FS-431", "FreeSurfer", true,
        "removing a point outside a point set (fsPointSet::remove) is undefined behaviour: memory corruption or a crash";
    FS_433 = "FS-433", "FreeSurfer", true,
        "loading a point set (a JSON file) that is not JSON, or whose vox2ras, points, coordinates or values have other types, ends the program with an uncaught exception: an abort (status 134) instead of an error message";
    FS_434 = "FS-434", "FreeSurfer", true,
        "printing a program's help, an element of the help text without text (an empty section or argument), or a comment, processing instruction or CDATA section between its sections or arguments, crashes the program (a null pointer)";
    FS_435 = "FS-435", "FreeSurfer", true,
        "printing a program's help, a word longer than a line (70 characters, 62 in an explanation) is cut at the line's end and its next character is dropped";
    FS_436 = "FS-436", "FreeSurfer", true,
        "printing a program's help, a comment or processing instruction among the kinds of arguments is printed as a section of that name in capitals followed by ARGUMENTS";
    FS_437 = "FS-437", "FreeSurfer", true,
        "converting a label of a surface from voxel coordinates to tkregister (surface) RAS converts each point the other way, as a tkregister RAS point to a voxel: wrong coordinates";
    FS_438 = "FS-438", "FreeSurfer", true,
        "writing one frame of a volume to a file reports success when the file could not be written (an unknown format, a missing folder)";
    FS_439 = "FS-439", "FreeSurfer", true,
        "writing an MGH volume of a type the MGH writer does not handle (long, RGB, tensor, bitmap, double complex), or whose byte rows fail to write, ends with an error but leaves the file open: a compressed .mgz file stays empty and an uncompressed .mgh file holds only what was written before the error";
    FS_440 = "FS-440", "FreeSurfer", true,
        "writing an MGH volume reports success when the file could not be written in full (a full disk, a failed compression): only a byte volume's rows are checked";
    FS_441 = "FS-441", "FreeSurfer", true,
        "writing an MGH volume (or FreeSurfer's NIfTI header extension) of a volume without frames reads a first frame the volume does not have: memory contents decide whether a frames tag is written";
    FS_442 = "FS-442", "FreeSurfer", true,
        "writing one chosen frame of a volume of several frames to an MGH file writes the volume's whole frame count in the header: the file holds fewer frames than its header says and cannot be read back";
    FS_443 = "FS-443", "FreeSurfer", true,
        "writing an MGH volume with FS_MGZIO_USEVOXELBUFWRITE set leaves the volume's own voxels byte-swapped in memory: the program's volume is wrong after the write";
    FS_444 = "FS-444", "FreeSurfer", true,
        "writing one chosen frame that the volume does not have to an MGH file reads memory outside the volume";
    FS_445 = "FS-445", "FreeSurfer", true,
        "writing an MGH volume with FS_MGZIO_USEVOXELBUFWRITE set crashes for a volume without slices (a depth or a frame count of 0)";
    FS_446 = "FS-446", "FreeSurfer", true,
        "a volume made without voxels (a header read alone) leaves its contiguous-buffer flag unset (memory contents): writing it as an MGH file with FS_MGZIO_USEVOXELBUFWRITE set then crashes or not depending on memory";
    FS_447 = "FS-447", "FreeSurfer", true,
        "writing a volume of 64-bit integers (long) as NIfTI declares 32-bit integers and stores the first half of each row's bytes: wrong values";
    FS_448 = "FS-448", "FreeSurfer", true,
        "writing a NIfTI volume ignores the file's close: a compressed file whose last bytes could not be written (a full disk) is reported as written";
    FS_449 = "FS-449", "FreeSurfer", true,
        "writing a NIfTI volume of exactly 32,768 rows, slices or frames passes the size check and stores -32,768 in the header's 16-bit field: a header readers reject or misread";
    FS_450 = "FS-450", "FreeSurfer", true,
        "the singular value decomposition of a matrix without columns (MatrixSVD, MatrixSVDEigenValues; any row count) crashes";
    FS_451 = "FS-451", "FreeSurfer", true,
        "writing a NIfTI pair (.hdr and .img) under a name of 1,020 to 1,023 bytes without the .img or .hdr extension writes the files under names cut to 1,023 bytes, with a message (from 1,022 bytes the header and the image share one name and the header is lost), and under a name of 1,024 bytes or more the program aborts (buffer overflow detected)";
    FS_452 = "FS-452", "FreeSurfer", true,
        "writing a NIfTI pair (.hdr and .img) ignores both files' close: files whose last bytes could not be written (a full disk) are reported as written";
    FS_453 = "FS-453", "FreeSurfer", true,
        "reading a NIfTI pair (.hdr and .img) under a name of 1,020 to 1,023 bytes without the .img or .hdr extension opens files under names cut to 1,023 bytes, with a message";
    FS_454 = "FS-454", "FreeSurfer", true,
        "reading a scaled NIfTI volume of unsigned 32-bit values (a nonzero slope other than slope 1 with intercept 0) fails with 'error reading' unless the file holds the data twice, and then gives the second copy's values";
    FS_455 = "FS-455", "FreeSurfer", true,
        "reading an unscaled big-endian NIfTI volume of doubles swaps each value's two 4-byte halves separately instead of its 8 bytes: wrong values";
    FS_456 = "FS-456", "FreeSurfer", true,
        "reading an unscaled big-endian NIfTI volume of complex values leaves the values unswapped: wrong values";
    FS_457 = "FS-457", "FreeSurfer", false,
        "reading an unscaled NIfTI volume, or an ANALYZE volume, of unsigned 32-bit values gives an int volume: values above 2,147,483,647 become negative (kept: FreeSurfer's volume type)";
    FS_458 = "FS-458", "FreeSurfer", true,
        "reading a NIfTI volume of 1 or 2 dimensions, the header's extents past its number of dimensions are used as the volume's rows or slices instead of 1: an extent of 0 gives an empty volume, a larger one a read past the data";
    FS_459 = "FS-459", "FreeSurfer", true,
        "reading a big-endian NIfTI file never reads its header extensions, FreeSurfer's own included, and prints a warning that no valid extension was read";
    FS_460 = "FS-460", "FreeSurfer", true,
        "reading a NIfTI file whose first header extension is another program's does not find FreeSurfer's extension after it, and may take other bytes of the file for one";
    FS_641 = "FS-641", "FreeSurfer", true,
        "reading a NIfTI file that ends within 4 bytes after its 348-byte header decides from memory contents whether a header extension follows";
    FS_642 = "FS-642", "FreeSurfer", true,
        "printing a NIfTI header (debug output) prints its text fields as C strings: a field without a NUL byte runs on into the following fields, and past the header's end into memory";
    FS_643 = "FS-643", "FreeSurfer", true,
        "reading a big-endian NIfTI volume whose column count is not positive takes the columns from the header's glmin field without swapping its bytes: a wrong extent";
    FS_644 = "FS-644", "FreeSurfer", true,
        "reading a NIfTI pair (.hdr and .img) of the ico7 surface's shape (163,842 voxels) whose header gives 0 frames prints that the number of elements cannot change and then crashes";
    FS_645 = "FS-645", "FreeSurfer", true,
        "reading an ANALYZE file or SPM series with SPM_START_FRAME set to a text that does not start with a number uses memory contents as the first frame's number (the default is 1)";
    FS_646 = "FS-646", "FreeSurfer", true,
        "reading or writing ANALYZE files (.img and .hdr, SPM series) under names too long for FreeSurfer's buffers (about 1,000 bytes when reading, 1,024 when writing) ends the program (buffer overflow detected)";
    FS_647 = "FS-647", "FreeSurfer", true,
        "reading an ANALYZE header of an unsupported data type prints the type after FreeSurfer has released the header's memory (the number printed is still the header's)";
    FS_648 = "FS-648", "FreeSurfer", true,
        "reading an ANALYZE volume without a usable .mat file whose header's orientation code is not one of -1 to 5 prints a line with an unreadable orientation name and then crashes";
    FS_649 = "FS-649", "FreeSurfer", true,
        "reading an ANALYZE volume of double values with several frames puts every frame's values into the first frame: the first frame holds the last frame's values and the others stay 0";
    FS_650 = "FS-650", "FreeSurfer", true,
        "reading an ANALYZE volume whose .mat file holds a matrix that is not 4 columns wide, or has fewer than 3 rows, prints a matrix error and crashes";
    FS_651 = "FS-651", "FreeSurfer", true,
        "reading an SPM series of NIfTI pairs (zero-padded frame names) whose first frame's pair cannot be read as NIfTI crashes after the reader's message";
    FS_652 = "FS-652", "FreeSurfer", true,
        "reading a big-endian ANALYZE header, its extents field is byte-swapped as a 16-bit number and dim[5] to dim[7] are not swapped: the header dump prints wrong values";
    FS_653 = "FS-653", "FreeSurfer", true,
        "writing an ANALYZE volume with exactly 32,768 columns, rows, slices or frames writes that extent as -32,768 (larger extents are refused)";
    FS_654 = "FS-654", "FreeSurfer", true,
        "writing an ANALYZE volume of long (64-bit) values writes 32-bit ints, but from the first half of each row's bytes: the values come out as pieces of the longs";
    FS_655 = "FS-655", "FreeSurfer", true,
        "writing ANALYZE files ignores the files' close: files whose last bytes could not be written (a full disk) are reported as written";
    FS_656 = "FS-656", "FreeSurfer", true,
        "an ANALYZE voxel offset, or a written volume's smallest and largest value or origin, outside the integer range it is converted to (a NaN included) becomes the lowest int (the offset is then ignored), or wraps around 16 bits for the origin";
    FS_657 = "FS-657", "FreeSurfer", true,
        "a COR volume's header (COR-.info) whose required field (imnr0, imnr1, ptype, x, y, thick, psiz, strtx to endz) has no number after its name counts the field as given and uses memory contents for it";
    FS_658 = "FS-658", "FreeSurfer", true,
        "a COR header's flip-angle line is read eleven characters in, where the writer's 'flip angle ' ends: a shorter line gives a number from an earlier, longer line of the file or from memory contents";
    FS_659 = "FS-659", "FreeSurfer", true,
        "writing a volume that is not 8-bit as COR writes the first bytes of each row (a quarter of a float row) as the slice's voxels, after only a warning";
    FS_660 = "FS-660", "FreeSurfer", true,
        "writing COR slices from imnr0 to imnr1 when that range is longer than the volume's depth writes the next frames' slices as further slices, then crashes past the voxels";
    FS_661 = "FS-661", "FreeSurfer", true,
        "a bshort or bfloat volume's geometry file (<stem>.bhdr) that opens but cannot be read (a folder of that name) is read again for ever, printing an error each time";
    FS_662 = "FS-662", "FreeSurfer", true,
        "a bshort or bfloat slice header (<stem>_000.hdr) without its four numbers leaves the rows, columns, frames or byte order it lacks as memory contents (an error is printed, the reading goes on)";
    FS_663 = "FS-663", "FreeSurfer", true,
        "writing a bvolume for a subject warns 'overwriting file register.dat' whether or not register.dat exists";
    FS_664 = "FS-664", "FreeSurfer", true,
        "writing a bshort or bfloat volume writes its geometry to <stem>.hdr, while reading looks for <stem>.bhdr: a written volume read back loses its orientation, sizes and timing (the coronal 1 mm default)";
    FS_665 = "FS-665", "FreeSurfer", true,
        "writing as bshort or bfloat a volume of another type that names a subject, with SUBJECTS_DIR unset, crashes after the slices are written";
    FS_666 = "FS-666", "FreeSurfer", true,
        "writing a volume with the bhdr type to a name that does not end in .bhdr writes the files (null)_000.bshort or .bfloat and their headers in the working folder";
    FS_667 = "FS-667", "FreeSurfer", false,
        "writing a bvolume for a subject builds both volumes' direction matrices for register.dat from the R components alone (their A and S rows repeat them), so the matrices are singular and register.dat is written only when rounding hides that (kept: the geometry file is written instead, as always)";
    FS_668 = "FS-668", "FreeSurfer", true,
        "a bshort or bfloat geometry file's last line is ignored when it does not end with a newline";
    FS_669 = "FS-669", "FreeSurfer", true,
        "a bshort or bfloat geometry file's line that starts with a NUL byte makes the reader read, and possibly change, the byte before its line buffer";
    FS_670 = "FS-670", "FreeSurfer", true,
        "a bshort or bfloat volume in the root folder (/<stem>_000.bshort) is read from the working folder instead";
    FS_671 = "FS-671", "FreeSurfer", true,
        "writing as bshort or bfloat a volume of another type with two or more frames ends by freeing the converted copy as if it had a frame table for every frame (it has one): the program may crash after the files are written";
    FS_672 = "FS-672", "FreeSurfer", true,
        "a volume whose Talairach transform (named by a COR header or an MGH tag) fails to load keeps it marked as the volume's own; freeing the volume then frees a transform that was never written: memory contents, usually ending the program with 'delete_general_transform' and status 1";
    FS_673 = "FS-673", "FreeSurfer", true,
        "a Varian SDT volume's header (.spr) whose last line does not end with a newline has that line ignored";
    FS_674 = "FS-674", "FreeSurfer", true,
        "an SDT header (.spr) that opens but cannot be read (a folder of that name) is read again for ever, printing an error each time";
    FS_675 = "FS-675", "FreeSurfer", true,
        "an SDT volume whose name has no extension but sits in a folder whose name has a '.' (x.d/vol) gets its header's name from the folder's ('x.spr'), not 'x.d/vol.spr'";
    FS_676 = "FS-676", "FreeSurfer", true,
        "a GDF header line without its value (IMAGE_FILE_PATH, IP_RES, SL_THICK, UNITS, SIZE, the FS_?_RAS axes, ORIENTATION, MIN_CROP, MAX_CROP alone) counts the field as given and uses memory contents for it";
    FS_677 = "FS-677", "FreeSurfer", true,
        "a GDF DATA_TYPE line shorter than 'DATA_TYPE ' takes its type from the characters past its end: an earlier, longer line's or memory contents";
    FS_678 = "FS-678", "FreeSurfer", true,
        "a GDF header's units 'cm' leave the voxel sizes as they are and 'm' divides them by 100, where millimetres need them multiplied by 10 and 1,000";
    FS_679 = "FS-679", "FreeSurfer", true,
        "a GDF crop box (MIN_CROP, MAX_CROP, with mri_convert's cropping) outside the integer range, or NaN, becomes the lowest integer";
    FS_680 = "FS-680", "FreeSurfer", true,
        "reading GE Genesis files (I.001, or names ending NNN.MR) and GE Signa LX files (names like e1s2i3) always ends with a crash (signal 11) once the series' file names are found: no such volume can be read";
    FS_681 = "FS-681", "FreeSurfer", false,
        "reading GE Genesis files named I.<number> prints a second 'fname_format2 :' line from memory never written, or none (kept: no such line)";
    FS_682 = "FS-682", "FreeSurfer", true,
        "reading GE Genesis, GE LX or GE XIMG files, a % in the folder's or the series' name is taken as a C conversion when the other files' names are made: wrong names, a crash or an abort";
    FS_683 = "FS-683", "FreeSurfer", true,
        "a GE Genesis series of several echoes (their count in the file's header) is read as a volume with one slice per file in every echo: the slices past the file count divided by the echoes stay 0, and the centre is computed for the larger depth";
    FS_684 = "FS-684", "FreeSurfer", false,
        "reading GE LX or GE XIMG files the error messages name genesisRead, and reading GE Signa files is_signa and read_signa (kept)";
    FS_685 = "FS-685", "FreeSurfer", true,
        "reading a GE Genesis series of several echoes, FreeSurfer frees a header whose frame count it raised after allocating one frame: it reads past the frame table, and depending on memory contents crashes (signal 11 or an abort) or goes on";
    FS_686 = "FS-686", "FreeSurfer", true,
        "a GE Signa series of several echoes is read as a volume of (files - 1) / 2 + 1 slices whatever the echo count, each file's slice advanced as for two echoes: with three or more echoes files land in the wrong slices and frames and past the volume (memory corruption or a crash); with GE_ODD or GE_EVEN the volume has one frame while the files are read as echoes (the same, or every file into the first slice)";
    FS_687 = "FS-687", "FreeSurfer", true,
        "a GE Signa slice file shorter than its slice gives a slice of the bytes it holds and zeros (or an earlier file's bytes), byte-swapped, without a message";
    FS_688 = "FS-688", "FreeSurfer", true,
        "reading an old Siemens .ima header shorter than 4,998 bytes, the header reader frees a stack address when a field read runs past the end of the file and aborts the program (signal 6)";
    FS_689 = "FS-689", "FreeSurfer", true,
        "reading an old Siemens .ima header between 4,998 and 5,016 bytes, the pixel-size fields are read past the end of the file and keep whatever the stack held (memory contents)";
    FS_691 = "FS-691", "FreeSurfer", true,
        "an old Siemens .ima volume's field of view is read from the height's value (the width read overwrites the same variable) and byte-swapped a second time, so the width is ignored and only pathological bytes change the result from the height";
    FS_692 = "FS-692", "FreeSurfer", true,
        "reading an old Siemens .ima mosaic whose file has more rows than columns, the raw volume is allocated with the columns and rows exchanged, so the pixel read writes past the voxels (memory corruption or a crash)";
    FS_693 = "FS-693", "FreeSurfer", true,
        "an AFNI header (.HEAD) that opens but cannot be read (a folder of that name) is read again for ever, printing an error each time";
    FS_694 = "FS-694", "FreeSurfer", true,
        "an AFNI header (.HEAD) whose last line does not end with a newline has that line ignored: a count line there is lost, a line of numbers ends the reading with 'hit EOF while reading'";
    FS_695 = "FS-695", "FreeSurfer", true,
        "an AFNI header attribute whose count line comes before any type line or name line takes the missing type or name from memory contents";
    FS_696 = "FS-696", "FreeSurfer", true,
        "an AFNI header line 'count' with no number takes its count from the characters past the line's end: an earlier, longer line's or memory contents";
    FS_697 = "FS-697", "FreeSurfer", true,
        "an AFNI header number that does not read as one (a word, a decimal number in an integer attribute, a comma) makes FreeSurfer write numbers past its buffer until it crashes";
    FS_698 = "FS-698", "FreeSurfer", true,
        "an AFNI header attribute with more numbers than its count keeps the extra numbers past the count (read where the reader reads past the count: DATASET_RANK, SCENE_DATA) while they fit its allocation";
    FS_699 = "FS-699", "FreeSurfer", true,
        "an AFNI header attribute with a negative count fails without a message (numbers; errno ENOMEM), or crashes (a string of count -2 and below, or 2,147,483,647: signal 11)";
    FS_700 = "FS-700", "FreeSurfer", true,
        "an AFNI header attribute DATASET_RANK, TYPESTRING, SCENE_DATA, IDCODE_STRING, BRICK_STATS, BRICK_TYPES or BRICK_FLOAT_FACS given with another type (a string for numbers, numbers of the other kind) crashes the reader (signal 11) instead of an error";
    FS_701 = "FS-701", "FreeSurfer", true,
        "an AFNI header attribute with fewer values than the reader uses (DATASET_RANK under 2, SCENE_DATA under 3, BRICK_TYPES empty, an odd BRICK_STATS count, frames beyond BRICK_STATS's pairs or BRICK_FLOAT_FACS's values) gives memory contents for the missing ones";
    FS_702 = "FS-702", "FreeSurfer", true,
        "an AFNI TYPESTRING of 16 characters or more, or a BYTEORDER_STRING of 10 or more, is copied over the header's next fields (the scene data, orientation codes, origin and voxel sizes, then the header's other values: wrong values, memory contents or a crash)";
    FS_703 = "FS-703", "FreeSurfer", true,
        "an AFNI IDCODE_STRING whose last character is not '~' is copied one byte past its buffer, within its allocation (nothing visible)";
    FS_704 = "FS-704", "FreeSurfer", true,
        "the AFNI header's printout names the brick types wrongly: type 2 (int) as 'float', type 3 (float) as 'complex'";
    FS_705 = "FS-705", "FreeSurfer", true,
        "reading the voxels of an AFNI dataset whose header has no BRICK_FLOAT_FACS crashes (signal 11) before the first row; without BRICK_STATS it crashes (signal 11) after the first frame";
    FS_706 = "FS-706", "FreeSurfer", true,
        "an AFNI dataset whose frames have different scale factors (BRICK_FLOAT_FACS) takes the volume's type from the first frame's factor alone: a scaled frame after an unscaled first is written as floats into a byte or short volume, an unscaled frame (or one of factor 1) after a scaled first as bytes or shorts into a float volume: wrong values";
    FS_707 = "FS-707", "FreeSurfer", true,
        "an AFNI dataset with a dimension or frame count of 0 (or whose voxel count wraps to 0) crashes the reader with a division by zero (signal 8)";
    FS_708 = "FS-708", "FreeSurfer", true,
        "the AFNI reader's messages name the header when the BRIK cannot be opened, and cut the BRIK's length and voxel count to 32 bits";
    FS_709 = "FS-709", "FreeSurfer", true,
        "an AFNI dataset with a folder in the BRIK's place takes the folder's length from the file system (tmpfs, ext4 and others answer differently): an error or a read failure as the file system decides";
    FS_710 = "FS-710", "FreeSurfer", true,
        "an AFNI header attribute with more numbers than its allocation holds has them written past it: memory corruption or a crash";
    FS_711 = "FS-711", "FreeSurfer", true,
        "the AFNI header's printout names brick types 4 and above (and negative ones) with bytes from past its table of names: memory contents";
    FS_712 = "FS-712", "FreeSurfer", true,
        "an AFNI dataset whose later frame is scaled while the first is not writes that frame's floats past the end of the byte or short volume: memory corruption or a crash";
    FS_713 = "FS-713", "FreeSurfer", true,
        "the AFNI reader's message when the BRIK's size does not match the brick type prints a number from memory contents in place of the voxel count";
    FS_714 = "FS-714", "FreeSurfer", true,
        "an AFNI header string attribute of count -1 gives a string of memory contents (and an IDCODE_STRING of count -1 crashes the reader)";
    FS_715 = "FS-715", "FreeSurfer", true,
        "an AFNI header attribute with a count beyond what the machine's memory holds (above about 67 million numbers or 268 million characters) fails or crashes as the machine decides";
    FS_716 = "FS-716", "FreeSurfer", true,
        "an AFNI IDCODE_STRING of 24, 40, 56 ... characters whose last is not '~' is copied one byte past its allocation, over the memory allocator's records: memory corruption";
    FS_717 = "FS-717", "FreeSurfer", true,
        "reading the header of a volume of several frames with a frame list (name#n, without the voxels), as of an AFNI dataset, frees a copy whose frame table has one entry as if it had one per frame: memory contents, a crash or not";
    FS_718 = "FS-718", "FreeSurfer", true,
        "reading an image file as a volume (a .mat, .rgb, .pgm, .ppm, .pbm name, or the image type) whose reader fails (the file is missing or unreadable, a MATLAB file that cannot be read, a frame of a HIPS image, any PGM, PPM or PBM file, an SGI image of more than three planes) crashes (signal 11) after the reader's message";
    FS_719 = "FS-719", "FreeSurfer", true,
        "reading a complex MATLAB matrix as an image volume writes its numbers past the image's memory (8 bytes a pixel into 1) before the program ends on the complex format: memory corruption or a crash";
    FS_720 = "FS-720", "FreeSurfer", true,
        "reading an SGI image (.rgb) whose header is in the big-endian order (SGI's standard and FreeSurfer's own files) converts 8 bytes past the header's memory";
    FS_721 = "FS-721", "FreeSurfer", true,
        "reading a run-length SGI image in the little-endian order takes its tables' entries as 8 bytes: the row lengths come from the image data, so rows are wrong, cut or read from elsewhere";
    FS_722 = "FS-722", "FreeSurfer", true,
        "writing an SGI image (.rgb) stores the program's file descriptor number and a memory address in the file's header: bytes that change from run to run";
    FS_723 = "FS-723", "FreeSurfer", true,
        "reading an SGI image of 2-byte pixels whose rows have 16,384 to 32,767 pixels fails on the first row (getrow: error on read of row, status 1) after reading the rest of the file into the row's buffer";
    FS_724 = "FS-724", "FreeSurfer", true,
        "reading an SGI image of 2-byte pixels whose rows have 16,384 to 32,767 pixels, with more of the file left than the row holds, writes the rest of the file past the row's buffer: memory corruption or a crash before the read fails";
    FS_725 = "FS-725", "FreeSurfer", true,
        "writing an SGI image (.rgb) that cannot be created prints iopen: can't open output file (null), without the file's name";
    FS_726 = "FS-726", "FreeSurfer", true,
        "reading a run-length SGI image whose row is longer than the reader's buffer, or whose runs go past the row's width or the buffer's end, reads and writes past the reader's memory: memory corruption, wrong pixels or a crash";
    FS_727 = "FS-727", "FreeSurfer", true,
        "reading an SGI image can leave pixels that no read gave (a run-length row of length -1 or shorter than the width, a row of 32,768 or more 2-byte pixels, runs expanded from bytes no read gave): their values are memory contents";
    FS_728 = "FS-728", "FreeSurfer", true,
        "reading an SGI image of 2-byte pixels whose rows have 32,768 pixels or more reads only part of each row; the rest of the row is what the reader's buffer held before";
    FS_729 = "FS-729", "FreeSurfer", true,
        "reading a run-length SGI image in the big-endian order (SGI's standard files) converts its tables past their memory: memory corruption, a crash or a hang";
    FS_730 = "FS-730", "FreeSurfer", true,
        "writing an image volume as an SGI image (.rgb) wider or taller than 65,535 pixels stores its sizes modulo 65,536: a wrong file, or img_seek: row number out of range (status 1)";
    FS_731 = "FS-731", "FreeSurfer", true,
        "writing a volume as an image file reports success (0) when the image is not written: the file cannot be opened, the format refuses the image (PGM, PPM, a JPEG or MATLAB name of a volume that is not of bytes), or a HIPS name, which writes nothing";
    FS_732 = "FS-732", "FreeSurfer", true,
        "writing a volume of a type image files do not take (16-bit, 64-bit, RGB and other types, with at least one row) as an image file goes on after MRItoImage's error: a crash (signal 11), or for PGM and PPM names the format's refusal and ImageFree: null pointer";
    FS_733 = "FS-733", "FreeSurfer", true,
        "writing a volume 32,768 voxels wide or wider as an ITK warp (a NIfTI displacement field) stores its width in the header's 16-bit field modulo 65,536 (negative, or a smaller width): a file ITK misreads";
    FS_734 = "FS-734", "FreeSurfer", true,
        "reading a NRRD volume of signed 8-bit, unsigned 16-bit or unsigned 32-bit samples stores them in a voxel type that cannot hold every value (unsigned bytes, signed 16-bit, signed 32-bit): negative 8-bit samples read as 256 plus the value, unsigned samples past the signed range read as negative numbers";
    FS_735 = "FS-735", "FreeSurfer", true,
        "reading a NRRD volume of unsigned 64-bit samples stores them as signed 64-bit voxels: samples past 2^63 - 1 read as negative numbers";
    FS_736 = "FS-736", "FreeSurfer", true,
        "reading a NRRD volume takes the file's space origin (the first voxel's position) as the volume's centre, the voxel sizes from the diagonal of the space directions alone and the axes' directions from the space's name alone (RAS, LAS or LPS; another space keeps the default coronal axes), and keeps LPS and LAS coordinates unconverted: every NRRD volume lies half its extent off, and oblique or flipped volumes get wrong axes and zero or negative voxel sizes";
    FS_737 = "FS-737", "FreeSurfer", true,
        "reading a NRRD volume without a space origin gives the volume a NaN centre";
    FS_738 = "FS-738", "FreeSurfer", true,
        "reading a NRRD volume one of whose first three axes has no space direction (none, or no space directions at all) gives NaN voxel sizes";
    FS_739 = "FS-739", "FreeSurfer", true,
        "reading a NRRD volume of 2^31 samples or more, or with an axis of 2^31 samples or more, indexes the samples in 32-bit arithmetic (and cuts the axis to 32 bits): reads outside the data, a crash";
    FS_740 = "FS-740", "FreeSurfer", true,
        "refusing a NRRD file that is not a volume (other than three or four axes in a three-dimensional space), the element count and element size it prints are cut to 32 bits (wrong, or negative, from 2^31 on)";
    FS_741 = "FS-741", "FreeSurfer", true,
        "reading a NRRD volume of signed or unsigned 64-bit samples stores each sample's low 32 bits as a 32-bit number, one after another from the start of its row, in a volume of 64-bit voxels: the voxels hold pairs of other samples' low halves, and the second half of every row zeros";
    FS_742 = "FS-742", "FreeSurfer", true,
        "reading a MINC volume of signed bytes stores its values as unsigned bytes: a negative value reads as 256 plus the value";
    FS_743 = "FS-743", "FreeSurfer", true,
        "reading a MINC volume of unsigned 16-bit integers stores its values as signed 16-bit voxels: values above 32,767 read as the value minus 65,536";
    FS_744 = "FS-744", "FreeSurfer", true,
        "reading a MINC volume of 32-bit integers stores each value as a 32-bit number, one after another from the start of its row, in a volume of 64-bit voxels: the voxels hold pairs of other values, the second half of every row zeros, and unsigned values above 2^31 - 1 negative";
    FS_745 = "FS-745", "FreeSurfer", true,
        "reading a MINC file of five or more dimensions reads the names of its dimensions past the four the reader gives (memory contents): a crash, or the file refused after a read of what memory names";
    FS_746 = "FS-746", "FreeSurfer", true,
        "writing a volume of 64-bit integers (long) as MINC writes the 32-bit numbers in the first half of each row's bytes, one to a voxel: the halves of other voxels, wrong values";
    FS_747 = "FS-747", "FreeSurfer", true,
        "writing a volume of 32-bit integers as MINC takes the file's valid range from the values rounded to floats: past 2^24 the values lie outside the file's valid range, and with values near the 32-bit limits the range passes them and every voxel is written as -2147483648";
    FS_748 = "FS-748", "FreeSurfer", true,
        "writing a float volume as MINC whose first voxel is NaN gives the file a valid range of NaN (with volume_io's message: Volume has invalid min and max voxel value), whatever the other voxels hold";
    FS_749 = "FS-749", "FreeSurfer", true,
        "reading a GCA atlas (.gca, .gcz) whose node grid or prior grid sizes, as the file stores them, differ from the sizes its spacings give (a spacing that is not a whole number, a training volume whose width is not a multiple of the node spacing) reads the file's nodes and priors with the sizes the spacings give: every value after the first difference is read from the wrong place";
    FS_750 = "FS-750", "FreeSurfer", true,
        "reading a GCA atlas that ends inside its nodes, priors or tagged parameters takes the missing numbers from memory contents (label counts among them, so the reading can go on far past the end)";
    FS_751 = "FS-751", "FreeSurfer", true,
        "reading a GCA atlas keeps a prior's label count, a classifier's neighbour label counts and every label in 16 bits: a count or label of 32,768 or more (65,536 or more for a label) becomes another number, and a count read so reads the rest of the file from the wrong place";
    FS_752 = "FS-752", "FreeSurfer", true,
        "reading a GCA atlas of 101 to 109 inputs whose file holds the MR parameters (TR, flip angle, TE per input) writes them past the room for 100: other inputs' parameters and the atlas's direction cosines, centre, sizes and voxel sizes are overwritten (unless the file gives the direction cosines after the parameters)";
    FS_753 = "FS-753", "FreeSurfer", true,
        "reading a GCA atlas of more than 46,340 inputs computes the number of covariance values per class in 32 bits, which overflows: an allocation error that prints the wrong count, or a smaller allocation the reading writes past (memory corruption, a crash)";
    FS_754 = "FS-754", "FreeSurfer", true,
        "a GCA atlas whose prior grid has a voxel-to-RAS matrix with no inverse (a prior spacing or voxel size of 0, direction cosines of length 0 or linearly dependent, NaN among them) crashes the program that reads it";
    FS_755 = "FS-755", "FreeSurfer", true,
        "reading a GCA atlas with a negative neighbour label count ends the program with an allocation error whose count is part of a memory address, not the count";
    FS_756 = "FS-756", "FreeSurfer", true,
        "reading a GCA atlas allocates a node's classes before reading them: a class count, or a count of inputs, beyond the machine's memory gives an allocation error or the reading of a file that ends (memory contents), depending on the machine";
    FS_757 = "FS-757", "FreeSurfer", true,
        "reading a GCA atlas whose node grid reaches past its prior grid (a prior spacing larger than its sizes allow, or the sizes FS-749 reads) reads priors outside the prior grid for the class weights: memory contents or a crash";
    FS_759 = "FS-759", "FreeSurfer", true,
        "reading a GCA atlas of more than 109 inputs whose file holds the MR parameters overwrites the atlas's internal references with them: the program crashes";
    FS_760 = "FS-760", "FreeSurfer", true,
        "reading a volume whose header asks for millions of frames (or making a volume of that many): FreeSurfer keeps and fills about 1.6 kB of memory for each frame before it reads a voxel (3.2 GB for 2,000,000 frames of one voxel each), so past the machine's memory it ends at once (MRIalloc: could not allocate <n> frame, status 254; 449,657,401 frames) and below it fills the memory (12,000,000 frames: more than 12 GiB): the outcome depends on the machine";
    FS_841 = "FS-841", "FreeSurfer", true,
        "reading a float volume with non-finite voxels (NaN or infinite) through MRIread, MRIreadEx or MRIremoveNaNs on several threads: the voxel its 'NaN found at voxel' line names, and how many such lines it prints, depend on the threads' timing when those voxels lie in more than one column of x (the count and the voxels' new values do not)";
    FS_842 = "FS-842", "FreeSurfer", true,
        "adding a command line to a volume that already records 1,000 (MRIaddCommandLine, which every program calls on the volume it writes): after its error message FreeSurfer writes past the volume's table of command lines, so the count of lines becomes memory contents and a later line or the volume's writing can crash or write garbage";
    FS_843 = "FS-843", "FreeSurfer", true,
        "reordering a volume's axes (MRIreorder, mri_convert -r): dimension numbers other than 1, 2 and 3 whose sizes' product and sum both come to 6 in 32-bit arithmetic pass FreeSurfer's check and crash the program";
    FS_844 = "FS-844", "FreeSurfer", true,
        "reordering a volume's axes into a given volume with fewer frames than the source reads memory past that volume's slices: a crash or memory corruption";
    FS_845 = "FS-845", "FreeSurfer", true,
        "reordering a volume's axes into a given volume of another voxel type writes the source's voxels with the source's voxel size at the destination's rows: the values misplaced, and past the destination's voxels for a smaller type";
    FS_846 = "FS-846", "FreeSurfer", true,
        "a selective-average volume (FS-FAST's selxavg) whose name with .dat takes 1,000 bytes or more: FreeSurfer writes the header's name past its buffer: memory corruption or a crash";
    FS_847 = "FS-847", "FreeSurfer", true,
        "reading a selective-average header with more than 10 gamma functions: FreeSurfer writes their parameters over the header's other values (the null condition, the noise order, the arrays' places) and past the header beyond 20";
    FS_848 = "FS-848", "FreeSurfer", true,
        "a selective-average header whose counts ask for more than 2^31 numbers (its matrices of estimates, or its frames' powers): FreeSurfer's sizes wrap around in 32 bits, its numbers are written outside its arrays (a crash), and reading tries billions of numbers";
    FS_849 = "FS-849", "FreeSurfer", true,
        "writing a selective-average header whose lists hold fewer numbers than its counts reads past them: memory contents written, or a crash";
    FS_850 = "FS-850", "FreeSurfer", false,
        "removing a label's overlap with another (LabelRemoveOverlap, mri_label2label and mri_cor2label --ring): points without a surface vertex match each other, so every such point is removed when the other label has one (kept: the programs that call it pass surface labels, whose points all have vertices)";
    FS_851 = "FS-851", "FreeSurfer", true,
        "reading a GCA morph (.m3z, .m3d) or warp volume whose header asks for millions of nodes: FreeSurfer keeps and fills 264 bytes of memory for each node before it reads one (550 MB for 128 x 128 x 128 nodes), so past the machine's memory it ends (GCAMalloc: could not allocate ...) or the system ends it, and below it fills the memory: the outcome depends on the machine";
    FS_852 = "FS-852", "FreeSurfer", true,
        "reading a GCA morph with SHOW_NEG set on several threads: its lines naming each node whose volume turned negative come in the threads' order when those nodes lie in more than one column of x (which lines are printed does not depend on the threads)";
    FS_853 = "FS-853", "FreeSurfer", true,
        "reading a GCA morph whose spacing is 1,291 or more (or -1,291 or less): the cube of the spacing, which sets every node's original volume, is computed in 32 bits and wraps around: wrong volumes, Jacobians and negative-node counts";
    FS_854 = "FS-854", "FreeSurfer", true,
        "reading a compressed GCA morph (.m3z) whose compressed data is damaged after its nodes: the reading of its tags never ends, or ends as memory contents decide";
    FS_855 = "FS-855", "FreeSurfer", true,
        "reading an MGH or NIfTI warp volume as a GCA morph when the volume has no morph geometries (its geometry tag missing): the geometries' validity and sizes are memory contents, which the RAS warp formats and the node-coordinate frames use and which writing the morph carries into its outputs (the sizes printed, an .m3z's geometry tag, a warp volume's tags); and those frames of a morph whose image geometry has a size of 0 or less crash the program";
    FS_856 = "FS-856", "FreeSurfer", true,
        "writing a three-frame volume as a GCA morph (an .m3z or .m3d name, or the morph type asked for): success is reported whatever the morph's writer did: a file that cannot be made, a warp volume whose write fails, or a name of another format, for which nothing is written and nothing is said";
    FS_857 = "FS-857", "FreeSurfer", true,
        "the inverse sample of a GCA morph whose spacing is 0 (an atlas voxel to the image, as the atlas programs ask it) divides by the spacing in integers: the program crashes";
    FS_858 = "FS-858", "FreeSurfer", true,
        "reading a linear transform file (.lta) whose type is 11, the GCA morph's type, as a transform: every later use treats the linear transform as a morph: memory contents or a crash";
    FS_859 = "FS-859", "FreeSurfer", true,
        "using a linear transform file that holds no transform (nxforms = 0) as a transform (sampling a point, its geometries): the first transform is read past the array: memory contents or a crash";
    FS_860 = "FS-860", "FreeSurfer", true,
        "freeing a transform that was never read (a transform file that could not be read): the program crashes";
    FS_861 = "FS-861", "FreeSurfer", true,
        "the inverse sample of a GCA morph with no nodes (a grid of size 0) reads a node past the grid: memory contents or a crash";
    FS_862 = "FS-862", "FreeSurfer", true,
        "the nearest-voxel sample of a GCA morph's inverse (an image voxel to the atlas, as the atlas programs ask it) at a point within half a voxel of the image's far face reads one voxel past the inverse: another voxel's value, or memory contents past the last slice";
    FS_863 = "FS-863", "FreeSurfer", true,
        "reading a TIFF image whose last page's link to the next cannot be read, or asking for pages past those read: FreeSurfer writes and reads its memory past the pages it counted (a crash or wrong frames)";
    FS_864 = "FS-864", "FreeSurfer", true,
        "reading a TIFF image with frames outside its pages (a frame number past the last page, or a range ending past it): FreeSurfer reads memory contents for the missing pages, or makes an image of no frames or of a negative number of frames (the program then ends on its allocation)";
    FS_865 = "FS-865", "FreeSurfer", true,
        "reading or writing a TIFF image whose pages do not fit the image FreeSurfer makes for them (a later page larger than the first, rows longer than the image's at its last row, 1-bit rows at the image's end, grey tiles, 4-sample rows of fewer than 8 bits a sample, a width of 0): FreeSurfer reads or writes past the image's memory (a crash or memory corruption)";
    FS_866 = "FS-866", "FreeSurfer", true,
        "reading a 1-bit TIFF image: each row is unpacked to a whole number of bytes, so up to 7 pixels past the width spill into the next row (for top-left images the row already read), and a fill order of low bit first is applied a second time (the pixels of each byte mirrored)";
    FS_867 = "FS-867", "FreeSurfer", true,
        "reading a TIFF image: whether a page of separate planes is refused, and whether a 1-bit image's fill order of low bit first is taken into account, depends on memory contents (the same file is refused or read as contiguous planes from one call to another)";
    FS_868 = "FS-868", "FreeSurfer", true,
        "reading a TIFF image with a page of separate planes: after its message (PLANARCONFIG_SEPARATE unsupported) the program ends on an allocation of 2^64 - 3 bytes with that error's code (status 253), or goes on with an image of no pixels";
    FS_869 = "FS-869", "FreeSurfer", true,
        "reading several pages of a TIFF image: each page is placed after width x height x samples x (bits / 8) bytes for each page before it, so 1-bit pages all land on the first frame and 4-sample pages a third past their frames; the check that the pages have the same size misses the last page and pages of the same size but another shape";
    FS_870 = "FS-870", "FreeSurfer", true,
        "reading a tiled TIFF image: the tiles of a row are counted as width / tile width + 1, one too many when the width is a multiple of the tile width (every tile after the first row misplaced or dropped); every tile is placed as 3-byte pixels whatever its samples and bits, and a bottom-left orientation is read as top-left";
    FS_871 = "FS-871", "FreeSurfer", true,
        "reading a tiled TIFF image whose tile cannot be read, whose tiles have a width or length of 0, or whose tiles cannot be held in memory: FreeSurfer copies memory contents, divides by zero, never ends, or reads into no memory (a crash)";
    FS_872 = "FS-872", "FreeSurfer", true,
        "reading a TIFF image of samples FreeSurfer's image cannot hold (grey samples of other than 1, 8, 16, 32 or 64 bits, colour samples of other than 8 bits): the rows are stored as they come in an image of bytes or of 3-byte pixels, packed or past the rows: wrong pixels";
    FS_873 = "FS-873", "FreeSurfer", true,
        "reading a TIFF image: a field the page does not hold and libtiff gives no default for (the photometric interpretation, the compression, the size of a page that could not be read) is printed (Gdiag's verbose lines) or used as the image's size from memory contents";
    FS_874 = "FS-874", "FreeSurfer", true,
        "writing a TIFF image: images of bytes are labelled signed (SampleFormat 2) and floating-point images are labelled as signed integers (a second SampleFormat replaces the first): other readers take bytes above 127 as negative and floats as integers";
    FS_875 = "FS-875", "FreeSurfer", true,
        "copying a GCA morph into a new one (a transform copied, as concatenating transforms does): the copy keeps neither the morph's inverse (its index volumes) nor its affine matrix, so the copy has no inverse and is written without its affine";
    FS_876 = "FS-876", "FreeSurfer", true,
        "copying a transform into one of the other kind (a linear transform array into a GCA morph, or the reverse): FreeSurfer copies into the other structure's memory (a crash or memory corruption)";
    FS_877 = "FS-877", "FreeSurfer", true,
        "reading a stacked autoencoder (.ae, .sae, .csae) whose first line does not hold its five numbers (window half-width, layer count, scale, type, levels): after a warning the reading goes on with memory contents for the numbers missing";
    FS_878 = "FS-878", "FreeSurfer", true,
        "reading a stacked autoencoder whose file ends before a layer it announces, whose layer matrices cannot be read or made, or whose check of a one-voxel window has no first layer: FreeSurfer crashes";
    FS_879 = "FS-879", "FreeSurfer", true,
        "reading a stacked autoencoder whose window, input count, frame count or later layer's kernel does not fit a 32-bit integer: the sizes wrap around";
    FS_880 = "FS-880", "FreeSurfer", true,
        "reading a stacked autoencoder as a volume when a layer's weight matrix has fewer inputs than the window (or the kernel through the layer before) needs: the voxels take the next hidden units' weights";
    FS_921 = "FS-921", "FreeSurfer", true,
        "reading a later layer of a stacked autoencoder as a volume (frame #1 and on): the volume's header takes the extent of the autoencoder's training volume while its voxels are the kernel's, so the header and the voxels disagree (reads past the voxels, and the next layer's kernel sized from that extent)";
    FS_922 = "FS-922", "FreeSurfer", true,
        "reading a stacked autoencoder whose window is one voxel and whose levels are 0 or fewer: the check run on reading writes its first input outside the layer's memory (memory corruption or a crash)";
    FS_923 = "FS-923", "FreeSurfer", true,
        "reading a stacked autoencoder whose window is one voxel, with one level and no hidden units: the check run on reading reads the first weight of a matrix that has none (a crash)";
    FS_924 = "FS-924", "FreeSurfer", true,
        "reading a binary matrix (MatrixReadFrom: the layers of an autoencoder file) from a file that ends early, when the matrix's size asks for a million numbers or more past the end (a size read from misaligned bytes asks for up to two billion): an error line for each number and a matrix of that size filled from memory contents (billions of lines, gigabytes of memory)";
    FS_925 = "FS-925", "FreeSurfer", true,
        "reading a stacked autoencoder as a volume when a layer's weight matrix has fewer inputs than the window (or the kernel through the layer before) needs: the last hidden unit's voxels take memory contents past the matrix";
    FS_926 = "FS-926", "FreeSurfer", true,
        "allocating a matrix of many rows (a size read from a damaged or misaligned file, such as a billion rows of no numbers): FreeSurfer asks for 8 bytes per row before any number (gigabytes), so whether it goes on or crashes depends on the machine's memory";
    FS_927 = "FS-927", "FreeSurfer", true,
        "reading a later layer of a stacked autoencoder as a volume (frame #1 and on) when the autoencoder's stored extent is larger than the layer's kernel (as for any autoencoder trained on a volume): the volume's header names more voxels than its memory holds, so the reading goes on with memory contents or crashes";
    FS_928 = "FS-928", "FreeSurfer", true,
        "converting a linear transform from RAS to voxels without a source volume, composing a linear transform into a destination that holds a GCA morph, or making the density map of a linear transform that cannot be applied (a singular matrix): FreeSurfer dereferences a null pointer or writes into the morph's memory (a crash or memory corruption)";
    FS_929 = "FS-929", "FreeSurfer", true,
        "writing a warp volume (MGH or NIfTI) whose morph geometries were never given: one read without its geometry tag (an MGH warp volume, or a NIfTI displacement field without FreeSurfer's header extension), or a GCA morph read from such a volume and written as a warp volume: the geometries' validity and sizes written into the file's tags, and a NIfTI output's displacements when FreeSurfer converts them to RAS through those geometries, are memory contents";
    FS_930 = "FS-930", "FreeSurfer", true,
        "resampling a volume through a GCA morph (onto the atlas, or back onto the image) with an interpolation the sampler does not support (sinc), or a volume type it does not read: each voxel whose sample fails (one error line each) takes the value of the voxel before it instead of a value of its own";
    FS_931 = "FS-931", "FreeSurfer", true,
        "resampling a volume through a GCA morph when the first voxel sampled fails (an unsupported interpolation such as sinc, or an unsupported volume type): that voxel, and those after it until one is sampled, take memory contents";
    FS_932 = "FS-932", "FreeSurfer", true,
        "resampling a volume of more than one slice onto a GCA morph's atlas: every atlas voxel whose point falls at or below the volume's first slice (between -1 and 0, the first slice itself included) is 0, while points down to -1 along the other two axes take the edge voxels' values (an identity morph loses the first slice)";
    FS_933 = "FS-933", "FreeSurfer", true,
        "resampling a volume through a GCA morph of spacing 0 (or sampling it at a point that is not a number) whose grid has no nodes along one of its axes: FreeSurfer reads outside the morph's nodes (a crash)";
    FS_934 = "FS-934", "FreeSurfer", true,
        "composing a GCA morph with a linear transform array whose transforms do not chain (one's destination is not the next one's source or destination): after the error line FreeSurfer reads the reduced array it did not get (a crash)";
    FS_935 = "FS-935", "FreeSurfer", true,
        "composing a GCA morph of spacing 0 with linear transforms: FreeSurfer divides the atlas extent by the spacing as integers (a division by zero: a crash)";
    FS_936 = "FS-936", "FreeSurfer", true,
        "replacing a GCA morph by its inverse when the morph has no inverse yet and the volume its image geometry names cannot be read: after the error line FreeSurfer keeps no morph in the transform, and its next use crashes";
    FS_937 = "FS-937", "FreeSurfer", true,
        "reordering all four dimensions of a volume (mri_convert --reorder4) with an order value outside 1 to 4: FreeSurfer reads memory outside its arrays for the result's extents and the voxels' places (undefined behaviour: memory contents or a crash)";
    FS_938 = "FS-938", "FreeSurfer", true,
        "reordering all four dimensions of a volume (mri_convert --reorder4) with an order that repeats a dimension: FreeSurfer silently makes a volume with that dimension twice, each voxel at the place its repeated index gives (a place reached several times keeps the last value, the others stay 0)";
    FS_939 = "FS-939", "FreeSurfer", true,
        "moving a surface through a GCA morph (mri_normalize -surface with a morph): a vertex outside the morph's grid, or next to a node of invalid position, silently gets the position of the last vertex the morph moved";
    FS_940 = "FS-940", "FreeSurfer", true,
        "moving a surface through a GCA morph: a vertex outside the morph's grid, or next to a node of invalid position, before any vertex the morph could move gets a position from memory contents";
    FS_941 = "FS-941", "FreeSurfer", true,
        "moving a surface through a RAS-to-RAS linear transform with a volume given (mris_sphere's transform option): the warning that the volume's centre differs from the centre of the volume the surface was made on never appears for different centres (only for a centre that is not a finite number), though the surface is then shifted";
    FS_942 = "FS-942", "FreeSurfer", true,
        "moving a surface through a linear transform of a type other than voxel-to-voxel or RAS-to-RAS (a physical-voxel or coronal transform file): the program crashes at the first vertex that is not ripped";
    FS_943 = "FS-943", "FreeSurfer", false,
        "moving a surface through a RAS-to-RAS linear transform with a volume given: the warning that the volume's centre differs from the transform's source volume never appears for different centres (only for a centre that is not a finite number); kept, as a RAS-to-RAS transform does not depend on its source's centre (the warning would be wrong for the usual transform from the raw volume to the conformed one)";
    FS_944 = "FS-944", "FreeSurfer", true,
        "reading a GE ROI series (mri_convert --roi): a named file that ends before the image's width or height (shorter than 16 bytes) gives a width or height from memory contents";
    FS_945 = "FS-945", "FreeSurfer", true,
        "reading a GE ROI series: the first slice file read that ends before its pixel data offset (shorter than 8 bytes) reads its pixels from an offset made of memory contents";
    FS_946 = "FS-946", "FreeSurfer", true,
        "reading a GE ROI series: a slice file that ends before its pixel data offset after another slice was read takes the offset's missing bytes from the earlier slice's offset, in reversed byte order: its pixels are read from an unrelated place";
    FS_947 = "FS-947", "FreeSurfer", true,
        "reading a GE ROI series whose named file asks for a volume of more than 16 GiB: FreeSurfer clears the whole volume before reading a slice, so it goes on or exhausts the machine's memory depending on the machine";
    FS_948 = "FS-948", "FreeSurfer", false,
        "counting the items of a stats table's first line reads past the end of the line, through memory the program does not use, up to the next blank or tab: the count is the same";
    FS_949 = "FS-949", "FreeSurfer", true,
        "mri_convert --in_stats_table reads a .csv table twice, as CSV and then in FreeSurfer's own table format, and keeps the second: a CSV table fails to load or gives another table";
    FS_950 = "FS-950", "FreeSurfer", true,
        "mri_convert --in_stats_table of a .tsv or .tac table writes the table's volume to junk.mgz in the working directory (replacing a file of that name)";
    FS_1031 = "FS-1031", "FreeSurfer", true,
        "mri_convert --in_stats_table of a file name without an extension crashes";
    FS_1032 = "FS-1032", "FreeSurfer", true,
        "a stats table in FreeSurfer's format, or the template of --out_stats_table, that is empty (or cannot be read) has its columns counted from memory contents";
    FS_1033 = "FS-1033", "FreeSurfer", true,
        "the stats tables in FreeSurfer's format, and the template of --out_stats_table, count as rows the pieces of at most 99,999 bytes their lines are read in: a blank line counts as a row (a table with a trailing blank line fails to load), a longer line counts as several, and the columns are counted from the first line's first 99,999 bytes only";
    FS_1034 = "FS-1034", "FreeSurfer", true,
        "a word of more than 99,999 bytes in a stats table in FreeSurfer's format, or in the template of --out_stats_table, is written past the reader's buffer (a crash or corrupted memory)";
    FS_1035 = "FS-1035", "FreeSurfer", true,
        "mri_convert --in_stats_table of an empty .csv table (no line) crashes";
    FS_1036 = "FS-1036", "FreeSurfer", true,
        "mri_convert --in_stats_table of a .csv table with a row of fewer fields than its first line reads past the row (a crash or memory contents read as numbers)";
    FS_1037 = "FS-1037", "FreeSurfer", true,
        "mri_convert --in_stats_table of a .tac table of one column reads a second column's name from memory contents (a crash or a comparison of memory)";
    FS_1038 = "FS-1038", "FreeSurfer", true,
        "mri_convert's --rescale, -sc (--scale), -osc (--out-scale), --rescale-voxel and -dcm2niix-opts crash when the command line ends before their arguments";
    FS_1039 = "FS-1039", "FreeSurfer", true,
        "mri_convert copies a name argument of 1,024 bytes or more (2,000 or more for --new-transform-fname) past the end of its buffer (a crash or corrupted memory)";
    FS_1040 = "FS-1040", "FreeSurfer", true,
        "mri_convert --rescale-voxel with an index that is not a number uses memory contents for that index";
    FS_1041 = "FS-1041", "FreeSurfer", true,
        "mri_convert -f (--frame) with more than 2,000 frame numbers writes them past the end of its list (a crash or corrupted memory)";
    FS_1042 = "FS-1042", "FreeSurfer", true,
        "mri_convert --debug crashes when debug.gdb cannot be created in the working directory";
    FS_1043 = "FS-1043", "FreeSurfer", true,
        "mri_convert --left-right-reverse crashes when no axis of the input volume points left or right";
    FS_1044 = "FS-1044", "FreeSurfer", true,
        "mri_convert --new-transform-fname with a name of 1,024 bytes or more writes it past the volume's transform-name field (corrupted memory or a crash)";
    FS_1045 = "FS-1045", "FreeSurfer", true,
        "mri_convert's warning that a volume's axes are not orthogonal never checks the second against the third axis (it checks the first against the third twice): a volume whose only non-orthogonal pair is the second and third gets no warning";
    FS_1046 = "FS-1046", "FreeSurfer", true,
        "mri_convert --invert_transform of a linear transform without a source geometry and with a name of 505 bytes or more writes the name of the volume it then looks for past its buffer (corrupted memory or a crash)";
    FS_1047 = "FS-1047", "FreeSurfer", true,
        "mri_convert --like (--out_like) crashes when the volume it names cannot be read, where the input is resampled into it (with or without a transform)";
    FS_1048 = "FS-1048", "FreeSurfer", true,
        "mri_convert applying a nonlinear morph inverted (-ait) writes the result also to t.mgz in the working directory (replacing a file of that name)";
    FS_1049 = "FS-1049", "FreeSurfer", true,
        "mri_convert --ascii, --ascii+crsf and --ascii-fcol crash when the output file cannot be created";
    FS_1050 = "FS-1050", "FreeSurfer", true,
        "mri_convert --out_stats_table crashes after creating the output file when no table can be made from the volume (its --like template cannot be read or does not fit the volume)";
    FS_1051 = "FS-1051", "FreeSurfer", true,
        "mri_convert --split writes a frame's file name of 1,024 bytes or more past its buffer (corrupted memory or a crash)";
    FS_1052 = "FS-1052", "FreeSurfer", true,
        "mri_convert crashes when a step gives no volume and the program goes on with it (an upper threshold, upsampling, a label swap, a reduction, a crop, a downsampled template, a morph that cannot be applied, a frame copy)";
    FS_1053 = "FS-1053", "FreeSurfer", true,
        "mri_convert --upsample, --conform and --downsample (-ds, -dsold) print the new volume's geometry with a validity flag (valid) that FreeSurfer never sets: the printed value is memory contents";
    FS_1054 = "FS-1054", "FreeSurfer", true,
        "mri_convert reading an outline (.otl) series whose slice file does not start with GDF FILE VERSION (an empty file or a folder included) crashes while it prints its error message (a segmentation fault); corrected: the message otl slice <n> does not appear to be a GDF file and status 1";
    FS_1055 = "FS-1055", "FreeSurfer", true,
        "mri_convert reading an outline (.otl) series with an outline of the colour table's entry 5307 (rh-unc in FreeSurferColorLUT.txt): that entry's score is never reset from one voxel to the next, so it adds up over the slice (scanned column by column) and the label takes every voxel scanned after its sum passes their own scores; its value before the first voxel is memory never written (0 under reproduce); corrected: each voxel scored afresh";
    FS_1056 = "FS-1056", "FreeSurfer", true,
        "mri_convert reading an outline (.otl) series: an outline point of a known label with a coordinate outside 0 to 511 is written outside a 512 x 512 array of FreeSurfer's (undefined behaviour; the array is never read, and the official build leaves it and its writes out, so its results do not change); not written in larmorx";
    FS_1057 = "FS-1057", "FreeSurfer", true,
        "mri_convert reading an outline (.otl) series into a volume less than 171 voxels wide or high (its size from --in_like) reads the labels of the voxels whose place in the outlines' 512 x 512 space falls outside the outline field (twice the volume's width by twice its height) from memory FreeSurfer does not own (arbitrary labels or a crash); 0 there in larmorx";
    FS_1058 = "FS-1058", "FreeSurfer", false,
        "mri_convert reading an outline (.otl) series into a volume whose width (height) does not divide 512 samples the outlines' 512 x 512 space at whole steps of 512 divided by the width, rounded down: a width of 300 samples the first 300 columns only, and a width above 512 samples the first column for every voxel; kept until measured on real outline files (rule D)";
    FS_1059 = "FS-1059", "FreeSurfer", false,
        "mri_convert reading an outline (.otl) series weighs a voxel's diagonal neighbours as its side neighbours (0.5) when it chooses the voxel's label from the outlines' claims, though FreeSurfer defines a diagonal weight of 0.707 for them; kept until measured on real outline files (rule D)";
    FS_1060 = "FS-1060", "FreeSurfer", true,
        "mri_convert reading an outline (.otl) series checks for an exterior or cortex label that follows other labels with an inverted condition: its warning (label ... following non-exterior labels in slice <n>) is printed for a label that follows one holding all four words Exterior, exterior, Cortex and cortex, never where it is meant; corrected: printed when a label holding Exterior, exterior, Cortex or cortex follows another label";
    FS_1141 = "FS-1141", "FreeSurfer", true,
        "mri_convert reading an outline (.otl) series of ascii points: a point line that does not hold two numbers leaves the coordinates it lacks as memory never written; 0 in larmorx";
    FS_1142 = "FS-1142", "FreeSurfer", true,
        "mri_convert reading an outline (.otl) series with a slice count below 1 (--in_k_count 0) prints its error couldn't find any file between <first> and <last> with a first name never written (memory contents); printed empty in larmorx";
    FS_1143 = "FS-1143", "FreeSurfer", true,
        "mri_convert reading an outline (.otl) series keeps every slice file open: past the process's limit of open files (1,024 by default) the later slices are left empty, without a message; every slice read in larmorx";
    FS_1144 = "FS-1144", "FreeSurfer", true,
        "mri_convert reading an outline (.otl) series fills each outline's region with a recursion as deep as the region is large: a region of more than about 170,000 places of the outline field (twice the volume's width by twice its height; an outline whose seed lies outside it, or an outline of no points, in a volume 256 voxels wide or more) overflows a default 8 MiB stack and the program crashes; filled in larmorx";
    FS_1145 = "FS-1145", "FreeSurfer", true,
        "mri_convert reading an outline (.otl) series into a volume whose outline field (twice its width by twice its height, 75 bytes a place) passes the machine's memory: FreeSurfer's outcome depends on the machine (an error, or the program killed); refused in larmorx as by a machine without that memory (error allocating claim field)";
    FS_1146 = "FS-1146", "FreeSurfer", true,
        "mri_info --ysize and --zsize print the column voxel size (the same as --xsize and --cres), not the row and slice sizes; corrected: --ysize prints the row size as --rres does, --zsize the slice size as --sres does";
    FS_1147 = "FS-1147", "FreeSurfer", true,
        "mri_info -it (--in_type) has no effect: each volume is read as the type its name gives; corrected: read as the type given";
    FS_1148 = "FS-1148", "FreeSurfer", true,
        "mri_info crashes on a morph whose extension mixes upper and lower case (.M3z, .m3D): its type is a morph, but its report is that of a volume never read (the --format report aside); corrected: reported as a morph, as .m3z and .M3Z are";
    FS_1149 = "FS-1149", "FreeSurfer", true,
        "mri_info with more than 100 input volumes writes the names past the 100th over other settings of the program: its result then depends on its build (other options turned on, more messages, or a crash past about a thousand); larmorx reports every input";
    FS_1150 = "FS-1150", "FreeSurfer", true,
        "mri_info crashes when -it (--in_type) is the last argument, and when --otags is the last argument and the tag report is reached (no file name follows them)";
    FS_1151 = "FS-1151", "FreeSurfer", true,
        "mri_info --otags with a file that cannot be created writes nothing and says nothing: the report follows as if the tags were saved; corrected: ERROR: could not open <file> and status 1";
    FS_1152 = "FS-1152", "FreeSurfer", true,
        "mri_info --ras2vox and --ras2vox-tkr crash on a volume whose voxel-to-RAS matrix (or tkregister matrix) cannot be inverted, as with a voxel size of 0";
    FS_1153 = "FS-1153", "FreeSurfer", true,
        "mri_info --stats divides each frame's sum by the voxel count of a frame in a 32-bit integer, which wraps past 2,147,483,647 voxels: a wrong (or negative, or infinite) mean; corrected: the count in 64 bits";
    FS_1154 = "FS-1154", "FreeSurfer", true,
        "mri_add_xform_to_header with a transform name of 1,024 bytes or more passes the room the volume's header has for it: the official build stops with *** buffer overflow detected ***: terminated (status 134, nothing written); corrected: the whole name written";
    FS_1155 = "FS-1155", "FreeSurfer", true,
        "mri_add_xform_to_header ends with exit status 0 when its output cannot be written (a folder that does not exist, a file it may not write, an unknown format): only the writer's messages tell; corrected: status 1 after them";
    FS_1156 = "FS-1156", "FreeSurfer", true,
        "mri_add_xform_to_header writing a COR folder's COR-.info whose path (the folder's name with /COR-.info) has 1,024 bytes or more passes FreeSurfer's buffer: the official build stops with *** buffer overflow detected ***: terminated (status 134, nothing written); corrected: the file written";
    FS_1157 = "FS-1157", "FreeSurfer", true,
        "mri_add_xform_to_header writing a COR folder's COR-.info writes the pixel size as the slice thickness (thick): a volume whose slices are thicker or thinner than its pixels reads back with the pixel size as its thickness; corrected: the slice thickness";
    FS_1158 = "FS-1158", "FreeSurfer", true,
        "mri_add_xform_to_header writing a COR folder's COR-.info writes the transform's name (its xform line) only when a transform was loaded: with -c (copy the name only) the name is lost unless the folder's own transform had loaded, and an empty name gives the line xform with nothing after it; corrected: the line is written whenever the name is not empty";
    FS_1159 = "FS-1159", "FreeSurfer", true,
        "lta_convert with an option that takes a value given last aborts (an option naming a file or the subject: basic_string::_M_construct null not valid, status 134) or crashes (--srcupsample, --srcdownsample, --trgupsample, --trgdownsample, --srcconform-size, --trgconform-size: status 139); corrected: ERROR: Option: <option> needs a value, status 1";
    FS_1160 = "FS-1160", "FreeSurfer", true,
        "lta_convert --srcconform-size and --trgconform-size read their value but do not step over it: the reading of the options stops at the value, and every option after it is ignored without a message (an --outlta after it writes nothing, and the program says successful); corrected: the value is stepped over";
    FS_1161 = "FS-1161", "FreeSurfer", true,
        "lta_convert --initk (ITK text transforms) takes the end of a file that ends with white space (the usual new line after the last line) as a repeat of its last line: a last FixedParameters line with a centre other than 0 applies the centre twice (a wrong translation), a last Parameters line rewrites the matrix with the last three numbers, a last Transform line or comment ends the program as an unknown line; corrected: the reading ends at the end of the file";
    FS_1162 = "FS-1162", "FreeSurfer", true,
        "lta_convert --initk never ends when a number of a Parameters or FixedParameters line is not a number and is not at the end of the file: the program repeats the line for ever; larmorx ends with ERROR readITK: cannot read <file>, status 1";
    FS_1163 = "FS-1163", "FreeSurfer", true,
        "lta_convert --inniftyreg, --inniftyreg2d, --initk and --invox given a file with fewer numbers than they need, or with text that is not a number, go on: a number not read keeps the value read before it (a row repeats the row before it), one before any number read is memory contents; corrected: ERROR <reader>: cannot read <file>, status 1";
    FS_1164 = "FS-1164", "FreeSurfer", true,
        "lta_convert with a transform that cannot be inverted (a singular NiftyReg, ITK or voxel-space input, --invert, --outniftyreg, --outitk, --outvox) goes on without its matrix and crashes where it writes it (status 139), or prints MatrixPrint: mat = NULL! and says successful when nothing is written; a singular voxel-space input gives the inverse's leftovers silently; corrected: ERROR <step>: the matrix cannot be inverted, status 1";
    FS_1165 = "FS-1165", "FreeSurfer", true,
        "lta_convert --infsl and --inreg given a file that does not read, and an .lta without a transform, go on and crash (status 139) after reading both volumes; corrected: ERROR <reader>: cannot read <file>, status 1";
    FS_1166 = "FS-1166", "FreeSurfer", true,
        "lta_convert's --srcupsample, --srcdownsample, --trgupsample and --trgdownsample accept factors that make no sense: a downsampling factor of 0 divides by 0 (status 136), a factor below 1 or one that makes a size pass 2,147,483,647 writes a geometry of sizes 0 or below, and a downsampling factor that does not divide every size prints ERROR: MRIdownsampleN: ... and goes on with the geometry unchanged (status 0); corrected: ERROR, status 1";
    FS_1167 = "FS-1167", "FreeSurfer", true,
        "lta_convert --srcconform-dc and --trgconform-dc on a geometry whose orientation does not give each axis its own main direction (an unknown orientation ???, as a transform without its volume's geometry has, or two axes along the same direction) read memory past the geometry's three axes: the conformed geometry holds memory contents; corrected: ERROR, status 1";
    FS_1168 = "FS-1168", "FreeSurfer", true,
        "lta_convert --subject with a name of 1,024 bytes or more copies it past the room the transform has for it (memory corruption: names of a few thousand bytes crash the program); larmorx keeps the whole name";
    FS_1169 = "FS-1169", "FreeSurfer", true,
        "lta_convert --sqrt (which prints the matrix's singular value decomposition, MATRIX SQRT not supported yet) puts the decomposition's left factor in place of the transform: every output written after the .lta (--outfsl, --outmni, --outreg, --outniftyreg, --outitk, --outvox) holds that factor instead of the transform; corrected: the outputs keep the transform";
    FS_1170 = "FS-1170", "FreeSurfer", true,
        "lta_convert stops reading its options at the first argument that does not start with a dash, without a message: that argument and every option after it are ignored, and the program goes on with what it read; corrected: ERROR: Option: <argument> unknown, status 1";
    FS_1171 = "FS-1171", "FreeSurfer", true,
        "fs_temp_file --base with a folder that does not exist, cannot be written or is empty makes its file elsewhere without a message: in the folder of the next of TMPDIR, TMP, TEMP and TEMPDIR that is set and writable, else in /tmp; corrected: Can not write to temporary directory <base>, status 1";
    FS_1173 = "FS-1173", "FreeSurfer", true,
        "getfullpath reads its name again as shell words: blanks split it (the first word is used, the next ones are taken as variable assignments or end the program with set: Variable name must begin with a letter. or ... must contain alphanumeric characters.), patterns (*, ?, [...], {...}) are matched against the files (set: No match., or if: Expression Syntax. for several), a leading ~ or =N is a home folder or the directory stack, a folder named +N is taken as the directory stack (pushd: Directory stack not that deep.), and an empty name ends with dirname's and if's errors, status 1; corrected: the name as given";
    FS_1174 = "FS-1174", "FreeSurfer", true,
        "getfullpath prints its result through tcsh's echo, which reads backslash sequences in it: a name or folder holding \\t, \\n, \\c, \\0nnn and the like prints a tab, a new line, nothing after it, a byte; corrected: the path as it is";
    FS_1175 = "FS-1175", "FreeSurfer", true,
        "getfullpath given a name that starts with a dash (other than - alone) passes it to dirname as options: dirname: invalid option, unrecognized option or missing operand, then if: Missing file name., status 1; corrected: the name as given";
    FS_1176 = "FS-1176", "FreeSurfer", true,
        "getfullpath reads the physical path of the name's folder again as shell words: several blanks or a tab in it print as one space, and pattern characters in it are matched against the files (echo: No match., status 1, when nothing matches); corrected: the path as it is";
    FS_1177 = "FS-1177", "FreeSurfer", true,
        "mri_vol2vol --crop without a target volume, an LTA or --downsample stops with ERROR: No target volume supplied. (status 1), although --crop makes its own target; corrected: the crop is made";
    FS_1178 = "FS-1178", "FreeSurfer", true,
        "mri_vol2vol: a target or registration name built from SUBJECTS_DIR, FSLDIR or FREESURFER_HOME (--fstarg, --tal, --mni152reg) of 2,000 bytes or more, or a --save-reg output name of 996 bytes or more: the official build aborts (*** buffer overflow detected ***: terminated, status 134); corrected: the whole name is used";
    FS_1179 = "FS-1179", "FreeSurfer", true,
        "mri_vol2vol --downsample with a factor of 0 for the second or third axis ends with a floating-point exception (status 136); a factor below 0 makes a grid of negative size, refused by the allocation's messages (status 1); corrected: ERROR: --downsample's factors must be 1 or more, status 1";
    FS_1180 = "FS-1180", "FreeSurfer", true,
        "mri_vol2vol --inv with an LTA and no --mov crashes (status 139) where it reads the moving volume's header (with --keep-precision, or with --targ); corrected: an error naming the missing --mov, status 1";
    FS_1181 = "FS-1181", "FreeSurfer", true,
        "mri_vol2vol --crop cuts the moving volume to one voxel more than the box of its voxels above 0 on each axis (the output one voxel per axis larger, times the scale), and crashes (status 139) when the box reaches the volume's last voxel on any axis; corrected: the box itself";
    FS_1183 = "FS-1183", "FreeSurfer", true,
        "mri_vol2vol --reg with a tkregister register.dat (not an .lta) and a target (--targ or --fstarg) always stops with LTAchangeType: src geometry must be valid (status 252): the register.dat's matrix is never applied; corrected: the register.dat's matrix maps the target to the moving volume";
    FS_1182 = "FS-1182", "FreeSurfer", true,
        "mri_vol2vol --crop with a scale that leaves no voxel (0, or so small that a size rounds to 0) writes an empty volume with status 0, and a negative scale stops with the allocation's messages (status 1); corrected: ERROR: --crop's scale <s> leaves no voxel, status 1";
    FS_1184 = "FS-1184", "FreeSurfer", true,
        "mri_vol2vol --map-point with a coordinate or a coordinate kind that is not a number maps memory contents (the number is never read); corrected: ERROR: --map-point: <argument> is not a number, status 1";
    FS_1185 = "FS-1185", "FreeSurfer", true,
        "mri_vol2vol --cost with a regseg segmentation smaller than the resampled volume reads past the segmentation (memory contents); corrected: ERROR: SegRegCost(): regseg and f differ in size, and the cost line is not written";
    FS_1186 = "FS-1186", "FreeSurfer", true,
        "mri_vol2vol --cost with a cost file that cannot be opened for appending (a folder that does not exist or cannot be written) crashes (status 139) after the resampling; corrected: ERROR: could not open <file>, status 1";
    FS_1187 = "FS-1187", "FreeSurfer", true,
        "mri_vol2vol --cost when the cost is refused (ERROR: SegRegCost(): regseg type must be int, uchar, or float, or f type must be float, as with --precision other than float) still appends a line of eight numbers that were never computed (memory contents); corrected: no line is written and the program ends with status 1";
    FS_1188 = "FS-1188", "FreeSurfer", true,
        "mri_vol2vol --gcam with a source LTA and no morph (gcam 0) stops with ERROR: MRIvol2volGCAM(): neither src nor dst of Dest LTA matches the src vol geom of the Source LTA when the destination LTA's destination (not its source) is the source LTA's source, the case the check is meant to accept; corrected: the destination LTA is used as it is";
    FS_1189 = "FS-1189", "FreeSurfer", true,
        "mri_vol2vol --gcam with a voxel shift map and a phase-encoding direction (--vsm-pedir before --gcam) other than 1, 2 or 3: a negative direction squares the shift and writes outside the point's coordinates (a crash or memory corruption), 0 crashes, 4 changes the point's homogeneous coordinate (no effect), others write past it; corrected: -1, -2, -3 shift along that axis in the negative direction, others end with ERROR: MRIvol2volGCAM: pedir=<p>, must be +/-1, +/-2, +/-3, status 1";
    FS_1190 = "FS-1190", "FreeSurfer", true,
        "mri_vol2vol --gcam0 (and --gcam with MY_MORPHS_DO_NOT_CONFORM_DEAL_WITH_IT set) without a destination LTA (0, or a file that does not read) crashes (status 139); corrected: ERROR: MRIvol2volGCAM0(): a destination LTA is needed, status 1";
    FS_1191 = "FS-1191", "FreeSurfer", true,
        "mri_vol2vol --morph and --inv-morph with a morph name of 1,000 bytes or more (SUBJECTS_DIR/<subject>/mri/transforms/<m3z>, or the --m3z name with --noDefM3zPath): the official build aborts (*** buffer overflow detected ***: terminated, status 134); corrected: the whole name is used";
    FS_1192 = "FS-1192", "FreeSurfer", true,
        "mri_vol2vol --morph --filter (the mode filter of the morphed volume) on values below -0.5, or not a number, counts them outside its histogram (memory contents or a crash); corrected: they are counted as values of their own";
    FS_1193 = "FS-1193", "FreeSurfer", true,
        "mri_vol2vol --cost without a segmentation SUBJECTS_DIR/<subject>/mri/regseg (.nii, .nii.gz, .mgh, .mgz, .img and the like) crashes (status 139) after the resampling; corrected: ERROR: --cost: cannot find the segmentation <name>, status 1";
    FS_1194 = "FS-1194", "FreeSurfer", true,
        "mri_concat --f: a word of the list file of 2,000 bytes or more names an input whose name runs on into the next words (its first 2,000 bytes followed by the next input's name); corrected: each word is its own input";
    FS_1195 = "FS-1195", "FreeSurfer", true,
        "mri_concat --f with a list file name holding white space opens only the name's first word; corrected: the whole name is opened";
    FS_1196 = "FS-1196", "FreeSurfer", true,
        "mri_concat --f with an empty argument (or only white space) opens a name taken from memory contents; corrected: the empty name is opened (ERROR: opening , status 1)";
    FS_1197 = "FS-1197", "FreeSurfer", true,
        "mri_concat --f with a list file name of 2,009 bytes or more that opens (a name past its buffer of 2,000 bytes): the official build stops when the command line has been read (*** stack smashing detected ***: terminated, status 134); corrected: the whole name is used";
    FS_1198 = "FS-1198", "FreeSurfer", true,
        "mri_concat --f lists that bring the inputs past 400,000 are stored past the program's tables: it crashes (status 139); corrected: ERROR: ninputs > 400000, status 1, as for the command line's inputs";
    FS_1200 = "FS-1200", "FreeSurfer", true,
        "mri_concat --rms with an input whose header reads but whose voxels do not crashes (status 139) after the reader's messages; corrected: status 1";
    FS_1241 = "FS-1241", "FreeSurfer", true,
        "mri_concat --no-check with an input of other dimensions than the first, or with more frames in all than inputs, reads and writes past the volumes (other voxels, memory contents, a crash); corrected: ERROR: dimension mismatch between <first> and <input> (or ERROR: <input> has more frames than counted), status 1";
    FS_1242 = "FS-1242", "FreeSurfer", true,
        "mri_concat frame counts past 2,147,483,647 wrap around (the inputs' frames in all, --mean-div-n's frames squared from 46,341 frames, --rep's frames times replications): wrong scale factors and wrong output sizes; corrected: ERROR: <what> passes 2147483647 frames, status 1";
    FS_1243 = "FS-1243", "FreeSurfer", true,
        "mri_concat --no-check --w with more frames than inputs weights the frames past the weight file's rows with memory contents; corrected: refused with FS-1241's error";
    FS_1244 = "FS-1244", "FreeSurfer", true,
        "mri_concat --pca with an output stem of 1,990 bytes or more: the official build aborts naming its matrix files (*** buffer overflow detected ***: terminated, status 134); corrected: the whole name is used";
    FS_1245 = "FS-1245", "FreeSurfer", true,
        "mri_concat --pca with an output name without a volume extension (.mgz, .mgh, .nii, .nii.gz, .img, ...) writes its matrices as (null).u.mtx and (null).stats.dat (before the output itself is refused: unknown file type); corrected: <output>.u.mtx and <output>.stats.dat";
    FS_1246 = "FS-1246", "FreeSurfer", true,
        "mri_concat --mtx with a matrix whose columns are not the frame count prints the matrix's row count in ERROR: dimension mismatch between inputs (<n>) and matrix (<rows>); corrected: its column count, the one compared";
    FS_1248 = "FS-1248", "FreeSurfer", true,
        "mri_concat --mask of other dimensions than the inputs is read at the inputs' voxels by --sort, --vote, --first-non-zero and --cumsum (other voxels of the mask, memory contents, a crash); corrected: ERROR: --mask's dimensions differ from the volume's, status 1";
    FS_1247 = "FS-1247", "FreeSurfer", true,
        "mri_concat --zconcat with a slice count to skip that is not a number skips a count taken from memory contents; corrected: ERROR: --zconcat: nskip <x> is not a number, status 1";
    FS_1249 = "FS-1249", "FreeSurfer", true,
        "mri_label2vol --defects with a negative offset names colour-table entries before the table's first (a write through memory contents, a crash); corrected: ERROR: the offset is negative, status 1";
    FS_1250 = "FS-1250", "FreeSurfer", true,
        "mri_label2vol --defects with defect numbers and an offset whose sum passes 2147483647: the colour table's size wraps below 0 and the program crashes; corrected: ERROR: the defect numbers plus the offset pass 2147483647, status 1";
    FS_1251 = "FS-1251", "FreeSurfer", true,
        "mri_label2vol --defects takes its seventh word as the label whenever the surface's name is not an option, even when that word is the next option (which then fails to read as a label), and drops the label when the surface's name starts with --; corrected: the seventh word is the label when it is not an option";
    FS_1252 = "FS-1252", "FreeSurfer", true,
        "mri_label2vol --defects with an offset or merge flag that is not a number uses a value from memory contents; corrected: ERROR: --defects: the offset (or merge flag) is not a number, status 1";
    FS_1253 = "FS-1253", "FreeSurfer", true,
        "mri_label2vol --defects with a label whose points name vertices the surface does not have: the label's mask is refused and the program then crashes; corrected: ERROR: the label does not fit the surface, status 1";
    FS_1254 = "FS-1254", "FreeSurfer", true,
        "mri_label2vol --pvf with labels or an annotation writes the partial-volume file, then ends with exit status 1 without writing the output volume; corrected: the output written too, status 0";
    FS_1255 = "FS-1255", "FreeSurfer", true,
        "mri_label2vol counts a label's hits in a voxel in a 16-bit integer: from the 32,768th label point in one voxel the count wraps to -32,768 and the voxel loses the label; corrected: the count stops at 32,767";
    FS_1256 = "FS-1256", "FreeSurfer", true,
        "mri_label2vol --proj none never sets the projected points: every point lands at a position taken from memory contents; corrected: ERROR: --proj none projects nothing, status 1";
    FS_1257 = "FS-1257", "FreeSurfer", true,
        "mri_label2vol --proj whose stop is infinite, or whose step is too small for the depth ever to pass the stop, never ends; both modes: ERROR: --proj never reaches its end, status 1";
    FS_1258 = "FS-1258", "FreeSurfer", true,
        "mri_label2vol --proj with a label point at a vertex the surface does not have (other than -1) reads the point's position from outside the surface's vertices (memory contents; measured: the point lands nowhere); corrected: ERROR: the label point is not one of the surface's vertices, status 1";
    FS_1259 = "FS-1259", "FreeSurfer", true,
        "mri_label2vol with more than 100 --label options writes the extra labels past its table, into memory the program uses for something else (measured: no visible effect); both modes take every label";
    FS_1260 = "FS-1260", "FreeSurfer", true,
        "mri_label2vol with a segmentation (--seg or --aparc+aseg) and --fill-ribbon but no --temp maps the segmentation onto a template it never read and crashes; corrected: ERROR: a segmentation needs a template volume, status 1";
    FS_1262 = "FS-1262", "FreeSurfer", true,
        "mri_label2vol --offset large enough that a label's code (its number plus the offset) passes the int range writes a wrapped code; corrected: ERROR: the label codes pass the range of an int, status 1";
    FS_1261 = "FS-1261", "FreeSurfer", true,
        "mri_label2vol --fill-ribbon with a label that is not in tkregister coordinates (and --tkr-template given) crashes after 'Converting to TkRegister RAS'; corrected: the label is converted and mapped (only its vertices and stats are used)";
    FS_1263 = "FS-1263", "FreeSurfer", true,
        "mri_label2vol --new-aseg2vol with a segmentation holding labels below 0 keeps their frames in memory it does not own (memory contents: a wrong frame, damaged memory or a crash); corrected: each such label has its own frame, as every other label";
    FS_1264 = "FS-1264", "FreeSurfer", true,
        "mri_label2vol --new-aseg2vol --ttype (or --ttype+head) with a segmentation label that the colour table of $FREESURFER_HOME does not hold crashes at the first output voxel whose label no fraction of 0.5 or more decides; corrected: ERROR: MRIseg2SegPVF(): the segmentation's label <n> is not in the colour table, status 1";
    FS_1265 = "FS-1265", "FreeSurfer", true,
        "mri_label2vol --new-aseg2vol --ttype with a segmentation label that has no tissue type in the schema adds its fraction outside the tissue types' votes (memory contents) at output voxels no fraction of 0.5 or more decides; corrected: such a label does not vote";
    FS_1266 = "FS-1266", "FreeSurfer", true,
        "mri_label2vol --new-aseg2vol with an upsampling factor that rounds to 0 (a --resmm between -0.5 and 0, or by default a segmentation whose column size is below 0.5 mm) samples no point of an output voxel: the fractions are 0 (with --ttype every voxel takes the first label); corrected: one sample per voxel";
    FS_1267 = "FS-1267", "FreeSurfer", true,
        "mri_label2vol --new-aseg2vol with a --resmm whose samples per voxel side, upsampling factor or samples per voxel (the three sides' counts multiplied) pass 2,147,483,647 (--resmm -inf, 1e-300, ...) counts no sample (an int's lowest value) or divides by a wrapped number; corrected: ERROR: MRIseg2SegPVF(): resmm <r> asks for <the count>, status 1";
    FS_1268 = "FS-1268", "FreeSurfer", true,
        "mri_label2vol --fill-ribbon --annot with an annotation index (below the largest) that no vertex has crashes after 'Mapping'; corrected: the index maps nothing";
    FS_1269 = "FS-1269", "FreeSurfer", true,
        "mri_label2vol --fill-ribbon with a label point at a vertex the surface does not have (-1 for a volume label, or past its vertices) writes outside the surface's values (memory the program does not own); corrected: ERROR: label point <n> is at vertex <v>, not one of the surface's <n> vertices, status 1";
    FS_1270 = "FS-1270", "FreeSurfer", true,
        "mri_label2vol --fill-ribbon --lta with a transform whose source and destination are both other than the ribbon prints 'ERROR: MRIsurf2VolOpt(): ribbon is neither source nor target' and crashes; corrected: status 1 after the message";
    FS_1271 = "FS-1271", "FreeSurfer", true,
        "mri_label2vol --fill-ribbon with a ribbon voxel (3 or 42) farther from the surface than its quick search reaches (a search of the neighbouring cells of a 16 mm grid) prints 'ERROR: MRIsurf2VolOpt(): No Match', the search's details and crashes; corrected: the voxel takes the value of the surface's closest vertex (a full search), as every other ribbon voxel";
    FS_1272 = "FS-1272", "FreeSurfer", true,
        "mri_label2vol --fill-ribbon without FREESURFER_HOME (or without its FreeSurferColorLUT.txt) prints the environment's message and crashes; corrected: status 1 after the message";
    FS_1273 = "FS-1273", "FreeSurfer", true,
        "mri_label2vol --fill-ribbon reads the ribbon from the environment's SUBJECTS_DIR even when --sd names another subjects folder (the surface comes from --sd's): the ribbon of another folder, or none ('(null)' in its name when SUBJECTS_DIR is not set); corrected: the ribbon from the subjects folder the program uses (--sd's, else the environment's)";
    FS_1274 = "FS-1274", "FreeSurfer", true,
        "seg2cc, mergeseg and fscalc print their usage (status 1) when any argument holds the text -help, and their version (status 0, nothing done) when any holds -version: a subject or file name such as sub-helper; corrected: only the arguments -help and --help (-version and --version) do";
    FS_1275 = "FS-1275", "FreeSurfer", true,
        "seg2cc --expert always fails: with an expert-options file that passes the check it ends with 'XOptsFiles: Undefined variable.', with one that fails it after the check's message with 'cmd: Undefined variable.' (status 1); corrected: the file is checked (the check's message and status 1 when it fails) and seg2cc goes on (the file is not used, as the option's comment says)";
    FS_1276 = "FS-1276", "FreeSurfer", true,
        "seg2cc --ctab is accepted and ignored: for a segmentation not conformed to its smallest voxel, the merged corpus callosum labels always get $FREESURFER_HOME/FreeSurferColorLUT.txt; corrected: the colour table --ctab names";
    FS_1277 = "FS-1277", "FreeSurfer", true,
        "seg2cc, mergeseg and fscalc read their arguments and some names (SUBJECTS_DIR, FREESURFER_HOME, the output's folder) again as shell words: blanks split a name, patterns (*, ?, [...]) are matched against files, braces, a leading ~ or =N expand, an empty argument vanishes: other files and folders, or the shell's errors; corrected: every name as given (D13 gap: the shell's reading is not reproduced, both modes take the names as given, with a notice)";
    FS_1278 = "FS-1278", "FreeSurfer", true,
        "seg2cc, mergeseg and fscalc print their lines through the shell's echo: a first word -n drops the line's end (an unrecognized flag -n, an operation -n), backslash sequences in a name (\\t, \\c, \\0nnn) become other characters or end the line; corrected: the lines as written";
    FS_1279 = "FS-1279", "FreeSurfer", true,
        "seg2cc with a relative --log or --tmpdir (or a relative SUBJECTS_DIR, which its default names come from) reads the name from two folders: the folder it starts in and the subject's mri folder it then enters: the log is split between two files (or lines are lost with 'tee: ... No such file or directory'), and the conformation check's answer is not found, which takes the path for a volume not conformed; corrected: the names from the folder seg2cc starts in";
    FS_1280 = "FS-1280", "FreeSurfer", true,
        "seg2cc with a relative SUBJECTS_DIR runs mri_cc (and for a segmentation not conformed to its smallest voxel also mri_label2vol's merge steps) inside the subject's mri folder, where the relative name does not lead to the subject: they fail; corrected: the programs get the subjects folder's absolute name";
    FS_1341 = "FS-1341", "FreeSurfer", true,
        "mergeseg stops with 'SUBJECTS_DIR: Undefined variable.' (status 1) when SUBJECTS_DIR is not set, after making its temporary folder, although it never uses the subjects folder; corrected: its log line 'setenv SUBJECTS_DIR' is left empty and the merge is made";
    FS_758 = "FS-758", "FreeSurfer", true,
        "reading a compressed GCA atlas (.gcz) whose compressed data is damaged: damaged at its start, the version is taken from memory contents; damaged after the nodes and priors, the reading never ends";
    FS_432 = "FS-432", "FreeSurfer", true,
        "writing several volumes at once (the echoes of a multi-echo series) under one name, a name of 1,024 bytes or more with its suffix is written past FreeSurfer's buffer: memory corruption or a crash";
    FS_285 = "FS-285", "FreeSurfer", true,
        "reading a volume-geometry block (the text block of linear transform and surface files) whose line starts with a word of 64 bytes or more writes past the memory FreeSurfer sets aside for that word (undefined behaviour; no effect on the official build's results)";
    FS_286 = "FS-286", "FreeSurfer", true,
        "reading a volume-geometry block whose line has a second word of 2 bytes or more (not the usual =) writes past the memory FreeSurfer sets aside for that word: the official build then takes the line's key from that word's third byte on instead of the line's first word (a line 'valid == 1' is not counted)";
    FS_287 = "FS-287", "FreeSurfer", true,
        "reading a volume-geometry block, a blank line counts as a repeat of the line before it and takes one of the block's eight places: the block's last line (the centre, cras) is then not read and the line after the block is";
    FS_288 = "FS-288", "FreeSurfer", true,
        "reading a volume-geometry block whose first line is blank compares memory contents with the block's keys";
    FS_289 = "FS-289", "FreeSurfer", true,
        "the volume operations that walk a second volume (or the destination, or a frame they are given) with the first volume's extent and frames read and write other voxels of a smaller volume";
    FS_290 = "FS-290", "FreeSurfer", true,
        "the same operations read and write memory outside a volume whose extent or frames are smaller than the first's, or outside a frame it does not have";
    FS_291 = "FS-291", "FreeSurfer", true,
        "MRIinvertContrast and MRIscaleAndMultiply, written for volumes of bytes, take the first bytes of each row of a volume of another type as byte voxels: meaningless values";
    FS_292 = "FS-292", "FreeSurfer", true,
        "MRIdivide of three byte volumes does not move on in the first volume and the destination after a voxel whose divisor is 0: the rest of the row is divided with the wrong voxels and written one place early";
    FS_293 = "FS-293", "FreeSurfer", true,
        "MRIextractInto given a destination of another type than the source frees the caller's volume, which the caller then still uses";
    FS_294 = "FS-294", "FreeSurfer", true,
        "MRIextractInto with a start at or past the source's extent copies rows of a negative length (a huge size): the program crashes";
    FS_295 = "FS-295", "FreeSurfer", true,
        "MRIextractInto sets a new volume's start and end positions (xstart, xend and the others) with the voxel sizes truncated to integers: sub-millimetre voxels give 0, 1.5 mm voxels 1";
    FS_296 = "FS-296", "FreeSurfer", true,
        "MRIscalarMulFrame stores a NaN product into a byte volume as a value the C language leaves undefined (0 in the official build)";
    FS_297 = "FS-297", "FreeSurfer", true,
        "MRIextractInto of a volume of 64-bit integers (long) copies each row of the box from half its first column in the source to half its first column in the destination: the box's voxels come from the wrong place, half voxels included";
    FS_298 = "FS-298", "FreeSurfer", true,
        "the trilinear, cubic and sinc samplers read a volume of 64-bit integers (long) as 32-bit integers at the voxel's index: they sample halves of other voxels";
    FS_299 = "FS-299", "FreeSurfer", true,
        "MRIcubicSampleVolume (cubic sampling) divides its weighted sum by the number of points it added (64 inside the volume) where its weights sum to 8: a constant volume samples to an eighth of its value, other volumes likewise too dark";
    FS_300 = "FS-300", "FreeSurfer", true,
        "MRIcubicSampleVolumeFrame (cubic sampling of a frame) near the volume's edge adds window points it never read: memory contents";
    FS_301 = "FS-301", "FreeSurfer", true,
        "MRItranslate and MRIrotate_I (and the rotations about an axis) write each sample as a byte at the voxel's byte place whatever the volume's type: a volume of another type gets bytes inside its voxels";
    FS_303 = "FS-303", "FreeSurfer", true,
        "MRItranslate and MRIrotate_I on a volume of a type the trilinear sampler refuses store a value never set (memory contents) or the sample before";
    FS_304 = "FS-304", "FreeSurfer", true,
        "MRIrotateX_I, MRIrotateY_I and MRIrotateZ_I with an angle whose rotation cannot be inverted (NaN, infinite) crash after the error message";
    FS_305 = "FS-305", "FreeSurfer", true,
        "MRIfindApproximateSkullBoundingBox of a volume whose centre of mass is not a number (infinite voxels) walks about two billion voxels outside the volume and, when its first box is narrow, never returns";
    FS_306 = "FS-306", "FreeSurfer", true,
        "reading a transform whose file name has 500 bytes or more writes past the memory FreeSurfer sets aside for classifying the name (memory corruption)";
    FS_307 = "FS-307", "FreeSurfer", true,
        "reading a MINC transform (.xfm) whose last line starts with % after a second % comment line never returns";
    FS_308 = "FS-308", "FreeSurfer", true,
        "reading an .lta whose first two lines are not type = <n> and nxforms = <n> takes the type or the number of transforms from memory contents";
    FS_309 = "FS-309", "FreeSurfer", true,
        "reading an .lta of a single line crashes";
    FS_310 = "FS-310", "FreeSurfer", true,
        "reading a MINC transform (.xfm) whose %Generated by comment ends with a word containing src or dst crashes";
    FS_302 = "FS-302", "FreeSurfer", true,
        "the sinc samplers with a half-width whose window holds more than 128 voxels on an axis write past the memory FreeSurfer sets aside for an axis's 128 weights: memory corruption or a crash";
    FS_229 = "FS-229", "FreeSurfer", true,
        "VolSS2MosSS and MosSS2VolSS (a volume's voxel to a mosaic's pixel and back) crash with a division by zero for an image of no columns or rows, VolSS2MosSS also for a mosaic of no columns, and both for sizes of -2147483648 with -1";
    FS_230 = "FS-230", "FreeSurfer", true,
        "the .mat readers take every version 4 matrix that is not of little-endian doubles (the type code not 0) for the other byte order: its sizes and numbers are read byte-swapped (wrong sizes, a failed read or an exit)";
    FS_231 = "FS-231", "FreeSurfer", true,
        "the .mat readers fail on every matrix of 16-bit integers (could not read val[0, 0]); a file ending one byte into such a number reads it with its other byte from memory";
    FS_232 = "FS-232", "FreeSurfer", true,
        "the .mat readers read 8 bytes for each 32-bit integer: the numbers combine two of the file's (or one byte-swapped), and the read fails half-way";
    FS_233 = "FS-233", "FreeSurfer", true,
        "the .mat readers read unsigned 8-, 16- and 32-bit integers as signed: values of 128, 32768 or 2147483648 and above come out negative";
    FS_234 = "FS-234", "FreeSurfer", true,
        "the .mat readers convert single-precision numbers of the other byte order to integers before swapping their bytes: the values are wrong";
    FS_235 = "FS-235", "FreeSurfer", true,
        "the .mat readers read the imaginary part of a complex matrix only for doubles: for other types it is memory contents (or 1) instead of the file's numbers";
    FS_236 = "FS-236", "FreeSurfer", true,
        "MatlabRead never sees the complex flag of a version 5 matrix (it tests another bit): a complex matrix comes back real, and its imaginary part is read without skipping its element's tag";
    FS_237 = "FS-237", "FreeSurfer", true,
        "MatlabRead does not recognise numbers MATLAB stores in a version 5 small element (4 bytes or fewer: one single-precision number, one 32-bit integer, up to four bytes): unsupported matlab format and zeros";
    FS_238 = "FS-238", "FreeSurfer", true,
        "MatlabRead skips 8 bytes too many after a version 5 variable name whose length is a multiple of 8 (an empty name included): the matrix's type and numbers are read from the wrong place";
    FS_239 = "FS-239", "FreeSurfer", true,
        "MatlabRead's verbose messages show a version 5 variable name of 4 characters or fewer cut to its first character, and a longer one followed by memory contents";
    FS_240 = "FS-240", "FreeSurfer", true,
        "MatlabRead reads compressed (MATLAB 7, the default save format) variables wrongly: the rows, type and complex flag come from the wrong bytes, the byte order from memory never written, numbers from a fixed place and past the variable, and the decoding stops at 100 times the compressed size";
    FS_241 = "FS-241", "FreeSurfer", true,
        "MatlabRead, given a version 5 file that ends inside a variable's header, goes on with sizes, type and flags from memory never written";
    FS_242 = "FS-242", "FreeSurfer", true,
        "MatlabRead and MatFileRead leave the .mat file open (a compressed one twice): a program reading many files runs out of file descriptors";
    FS_243 = "FS-243", "FreeSurfer", true,
        "the .mat readers allocate whatever sizes a file states before reading it: a negative or huge size crashes the program or depends on the machine's memory";
    FS_244 = "FS-244", "FreeSurfer", true,
        "MatFileRead's bytes of numbers outside -128 to 127 wrap (300 gives 44), and of numbers beyond the 32-bit range or NaN are 0";
    FS_245 = "FS-245", "FreeSurfer", true,
        "the .mat readers' results depend on memory contents for some files (a 16-bit number cut by the file's end, the imaginary part of a matrix that is not of doubles, a compressed variable, a version 5 header cut by the file's end, a name printed past its end): values taken from memory never written or read past their buffers";
    FS_246 = "FS-246", "FreeSurfer", true,
        "MatlabRead, given a version 5 array of three or more dimensions, reads its name and type from the third dimension's place: wrong results";
    FS_247 = "FS-247", "FreeSurfer", true,
        "MatlabWrite of a complex matrix writes its real and imaginary parts interleaved as the first half of a real matrix";
    FS_248 = "FS-248", "FreeSurfer", true,
        "ReadMatlabFileContents of a file of more than 1,000 variables writes past its lists (a crash or corruption)";
    FS_250 = "FS-250", "FreeSurfer", true,
        "MatFileRead goes on after a header it could not read, with the sizes read so far (or 0)";
    FS_251 = "FS-251", "FreeSurfer", true,
        "the .mat readers take a byte 0xff for the end of the file: a version 5 file with 0xff in its first four bytes is refused, a version 4 variable whose first byte is 0xff ends the reading";
    FS_252 = "FS-252", "FreeSurfer", true,
        "MRIcopyHeader crashes when the source volume's voxel-to-RAS matrix has not been computed (FreeSurfer's own volumes always have it)";
    FS_311 = "FS-311", "FreeSurfer", true,
        "the segmentation functions (left-right label swaps, the ribbon fix, one-hot encoding, the brain volume corrections) turn a voxel value or a label list entry that is NaN or beyond the 32-bit integer range into the label -2147483648";
    FS_312 = "FS-312", "FreeSurfer", true,
        "the supratentorial and cortical gray matter volume corrections count each thalamus voxel twice";
    FS_313 = "FS-313", "FreeSurfer", true,
        "the cortical gray matter volume correction subtracts the left vessel label's voxels in the ribbon but not the right vessel label's";
    FS_314 = "FS-314", "FreeSurfer", true,
        "reading the cached brain volume statistics (stats/brainvol.stats) aborts on a line that names a measure, holds no number and ends in white space";
    FS_315 = "FS-315", "FreeSurfer", true,
        "the left-right swap of a segmentation by label lists reads the lists past their end when they hold fewer numbers than rows";
    FS_316 = "FS-316", "FreeSurfer", true,
        "the flood fill of a byte volume from a seed clamps a seed row or slice at or past the volume's height or depth to the volume's width minus one";
    FS_317 = "FS-317", "FreeSurfer", true,
        "downsampling by factors and the change of a volume's type with rescaling convert a number beyond the 32-bit integer range, or NaN, to the integer -2147483648 (the extent, the windows, the histogram's bins and counts)";
    FS_318 = "FS-318", "FreeSurfer", true,
        "the mean of a frame's voxels not below a threshold is the mean of frame 0's whatever the frame asked for";
    FS_319 = "FS-319", "FreeSurfer", true,
        "upsampling with the sum conserved goes on with no volume when its change to floats fails, and would crash; no input makes that change fail (its clipping fractions are 0), so both implementations keep only a guard";
    FS_320 = "FS-320", "FreeSurfer", true,
        "trilinear splatting of a value into an integer volume (MRIinterpolateIntoVolume) adds each corner's rounded share to the voxel in the voxel's type: a sum beyond the type's range wraps around (bytes, shorts, unsigned shorts, 32-bit integers)";
    FS_321 = "FS-321", "FreeSurfer", true,
        "trilinear splatting of a value into a volume of 64-bit integers (long) adds into 32-bit integers at the voxel's index: it changes halves of other voxels";
    FS_322 = "FS-322", "FreeSurfer", true,
        "the directional derivatives and gradients of a volume of a type the trilinear sampler refuses (RGB, complex) use sample values never written: memory contents";
    FS_323 = "FS-323", "FreeSurfer", true,
        "the neighbour counts of a voxel more than 500 voxels outside the volume read its boundary index tables past their ends: memory contents";
    FS_324 = "FS-324", "FreeSurfer", true,
        "the directional derivative at a scale (MRIsampleVolumeDerivativeScale) with an infinite scale, or one so large that twice it overflows (2^1023 or more), never returns";
    FS_325 = "FS-325", "FreeSurfer", true,
        "the linear transform of a volume (MRIlinearTransformInterp) stores, where a sample fails (sinc sampling, a type the trilinear sampler refuses), the last sample that succeeded, and before the first a value never written: memory contents";
    FS_326 = "FS-326", "FreeSurfer", true,
        "resampling a volume by voting (MRIresample with SAMPLE_VOTE) counts each label's votes for a voxel in a byte, which stops at 255: a label with more votes ties with one of 255 and the lower label wins";
    FS_327 = "FS-327", "FreeSurfer", true,
        "resampling a volume by voting onto a template whose orientation is unknown (no good RAS and no principal slice direction) or whose voxel-to-RAS matrix does not invert crashes";
    FS_328 = "FS-328", "FreeSurfer", true,
        "resampling a volume of 64-bit integers (long) onto a template writes each voxel as a 32-bit integer at the voxel's index: it changes halves of other voxels";
    FS_329 = "FS-329", "FreeSurfer", true,
        "the statistics of a volume of 64-bit integers (MRIstats, MRIprintStats) read 32-bit integers at the voxels' indices: halves of the first voxels instead of the voxels";
    FS_330 = "FS-330", "FreeSurfer", true,
        "the printed statistics (MRIprintStats) of a volume without voxels, or of a type the statistics do not read (bitmap, tensor, RGB, complex), are values never written: memory contents";
    FS_461 = "FS-461", "FreeSurfer", true,
        "replacing a value in a byte volume (MRIreplaceValues, MRIreplaceValuesUchar) replaces it in the first frame only; the other frames of a new or other destination are not written";
    FS_462 = "FS-462", "FreeSurfer", true,
        "the centre of a volume sampled by a whole factor (MRIcalcCRASforSampledVolume) divides by the sampled volume's extent: an extent of 0 ends the program";
    FS_463 = "FS-463", "FreeSurfer", true,
        "the check of an orientation string (MRIcheckOrientationString, MRIorientationStringToDircos) reads three characters, past the end of a string of fewer than two: memory contents";
    FS_464 = "FS-464", "FreeSurfer", true,
        "setting a volume's geometry from a voxel-to-RAS matrix (MRIsetVoxelToRasXform) reads rows 1 to 3 and columns 1 to 4, past the elements of a smaller matrix: memory contents";
    FS_465 = "FS-465", "FreeSurfer", true,
        "the conversions between a volume's slice and a 2-D image (MRItoImage, MRItoImageView, ImageToMRI) write or read past the image's pixels when a given image is smaller than the slice or of a narrower pixel format, or when the image holds fewer frames or pixels than its header counts: memory contents";
    FS_466 = "FS-466", "FreeSurfer", true,
        "an RGB image's frames after the first are read into a volume (ImageToMRI) one byte per pixel from the start instead of three: they overlap the first frame";
    FS_467 = "FS-467", "FreeSurfer", true,
        "the smoothing of a parcellation (MRIsmoothParcellation) counts its neighbours' labels in an array indexed by the label: a negative label is counted outside the array: memory contents";
    FS_468 = "FS-468", "FreeSurfer", true,
        "the clipping of bright white matter (MRIclipBrightWM) stores every voxel through a byte: a value below 0 or above 255, or NaN, becomes the low byte of its 32-bit truncation (0 for NaN and beyond the 32-bit range)";
    FS_469 = "FS-469", "FreeSurfer", true,
        "the partial-volume count of a label (MRIvoxelsInLabelWithPartialVolumeEffects) weighs a voxel on the label's border whose own label is negative or not below the label limit with a mean taken from memory contents";
    FS_470 = "FS-470", "FreeSurfer", true,
        "the partial-volume count of a label (MRIvoxelsInLabelWithPartialVolumeEffects) given no intensity volume reads the first border voxel's intensity from no volume: the program crashes";
    FS_471 = "FS-471", "FreeSurfer", true,
        "the partial-volume count of a label (MRIvoxelsInLabelWithPartialVolumeEffects) crashes the program for a negative label limit (at a voxel on the label's border, and from -4 down at any voxel it counts; -1 to -3 count the voxels off the border as usual), and for a very large one (from about 699,000 labels with Linux's default 8 MiB stack)";
    FS_472 = "FS-472", "FreeSurfer", true,
        "a constant fill (MRIconst) or a normalisation of each voxel's frames (MRInormalizeSequence) of a byte, short or unsigned short volume stores a value past the type's range as the low bits of its integer: it wraps";
    FS_473 = "FS-473", "FreeSurfer", true,
        "the reversal of a volume's slices (MRIreverseSliceOrder) copies one b-value and b-vector per frame from tables of fewer rows than frames past their rows: a crash or memory contents";
    FS_474 = "FS-474", "FreeSurfer", true,
        "the order of a volume's voxels by value (MRIorderIndices) keeps each voxel's coordinates and value in bytes: a coordinate past 255 wraps, and a value outside 0 to 255 or NaN becomes the low byte of its 32-bit truncation";
    FS_475 = "FS-475", "FreeSurfer", true,
        "the partial-volume fractions of a segmentation (MRIcomputeVolumeFractions) count a voxel of label 0 into the frame before the first (labels are frame plus 1): FreeSurfer reads and writes outside the volume: a crash or memory contents";
    FS_476 = "FS-476", "FreeSurfer", true,
        "the partial-volume fractions of a segmentation (MRIcomputeVolumeFractions) never mark the last label in their second pass (voxels no segmentation voxel reached), which accepts labels below the frame count only";
    FS_477 = "FS-477", "FreeSurfer", true,
        "the application of a list of voxel differences (MRIapplyDiffVect) accepts a column, row or slice equal to the volume's extent and writes past the volume";
    FS_478 = "FS-478", "FreeSurfer", true,
        "a list of voxel differences applied in place (MRIapplyDiffVect) frees the volume's colour table before copying it: the volume loses it, with the error 'CTABdeepCopy: ct was NULL'";
    FS_479 = "FS-479", "FreeSurfer", true,
        "the intracranial volume estimate from a transform file (MRIestimateTIV) reads the first linear transform of a file that holds none: the program crashes";
    FS_480 = "FS-480", "FreeSurfer", true,
        "the vote over each voxel's sorted frames (MRIvote) told to exclude zeros still takes a last run of zeros when it is the longest";
    FS_481 = "FS-481", "FreeSurfer", true,
        "the reshape of a volume of 64-bit integers (mri_reshape) copies 32-bit integers at the voxels' indices: halves of the first voxels, the rest left 0";
    FS_482 = "FS-482", "FreeSurfer", true,
        "the resampling with a voxel-shift map (MRIvol2VolVSM) shifts by the last value the map's sampler wrote; for a map of a type the sampler refuses, at the first voxel a value never written: memory contents";
    FS_483 = "FS-483", "FreeSurfer", true,
        "the frame covariance and PCA of a masked volume (MRIcovarianceMatrix, MRIpca) count the mask voxels above 0.5 but sum every voxel not below 0.5: a mask voxel of exactly 0.5 (or NaN) enters the sums and not the count";
    FS_484 = "FS-484", "FreeSurfer", true,
        "the frame PCA of a volume without frames (MRIpca) crashes in the singular value decomposition of its empty covariance";
    FS_485 = "FS-485", "FreeSurfer", true,
        "the gradient-direction measure of a voxel (MRIvoxelDirection) cuts its window at the volume's far faces but reads it back as a full cube: the gradients pair with the wrong offsets and the last entries were never written (memory contents)";
    FS_487 = "FS-487", "FreeSurfer", true,
        "the Talairach point conversions (MRIvoxelToTalairachEx and its siblings) crash on the point when their matrix could not be made without ending the program (a transform matrix that is not 4 x 4); and the Talairach-to-voxel matrix of a transform array that holds no transform is read past the array: memory contents";
    FS_488 = "FS-488", "FreeSurfer", true,
        "the resampling of a volume out of Talairach space (MRIfromTalairachEx) without a target volume uses the target before its own check and crashes";
    FS_489 = "FS-489", "FreeSurfer", true,
        "the mask of listed labels (MRImatchSegs) made in the segmentation itself clears each voxel before reading its label: every voxel is matched as label 0";
    FS_490 = "FS-490", "FreeSurfer", true,
        "the histogram of a segment's intensities (HISTOseg) with a bin count of 0 counts each matching voxel in the bin before the first: memory it does not own";
    FS_491 = "FS-491", "FreeSurfer", true,
        "the conformed template that keeps a volume's direction cosines, for a volume whose orientation has no left-right, inferior-superior or anterior-posterior axis (cosines not good, or two axes along one direction), reads a voxel size and a field of view that are not the volume's: memory contents";
    FS_492 = "FS-492", "FreeSurfer", true,
        "the conformed template that keeps a volume's direction cosines converts a field of view beyond the 32-bit integer range, or NaN, to the integer -2147483648, and the difference of two fields of view wraps around";
    FS_493 = "FS-493", "FreeSurfer", true,
        "the erosion of a segmentation (MRIerodeSegmentation) with a count of erosions of 0 or below, and its dilation (MRIdilateSegmentation) with a count of 0 or below -1, never end (the count is lowered until it reaches 1)";
    FS_494 = "FS-494", "FreeSurfer", true,
        "the threshold mask of byte volumes (MRIthresholdMask) stores a replacement value beyond 0 to 255, or NaN, as the low byte of its truncation to a 32-bit integer";
    FS_495 = "FS-495", "FreeSurfer", true,
        "the percentile threshold of a volume (MRIpercentThresh) for a percentage that rounds to the end of the sorted voxels or past it returns 0 from the unused part of its list, not the largest value";
    FS_496 = "FS-496", "FreeSurfer", true,
        "the percentile threshold of a volume (MRIpercentThresh) for a percentage that rounds past the list's allocation, below 0, or NaN reads memory it does not own";
    FS_497 = "FS-497", "FreeSurfer", true,
        "the dilation of a segmentation until it stops changing (MRIdilateSegmentation with -1) never ends when a step's new labels do not reach the next step's copy (a complex volume, of whose rows FreeSurfer's copy holds only the first width bytes, an eighth of a float complex row): the same changes are counted forever";
    FS_498 = "FS-498", "FreeSurfer", true,
        "the histograms of a whole volume (MRIhistogram), of a region of bytes and of a label in a region (MRIhistogramLabelRegion) count a voxel whose bin falls outside the histogram (a NaN voxel or bin size, a rounding past the last bin) in memory they do not own, or crash";
    FS_499 = "FS-499", "FreeSurfer", true,
        "the histogram equalisation (MRIgetEqualizeHisto, MRIhistoEqualize) indexes the histograms by intensity from the low to the high limit, outside their bins when the low limit is below 0 or an intensity is beyond the last bin: memory contents read and memory it does not own written";
    FS_500 = "FS-500", "FreeSurfer", true,
        "the cumulative histogram for equalisation (MRIgetEqualizeHisto) leaves the values of its bins from the low to the high limit unset (0, or the given histogram's)";
    FS_501 = "FS-501", "FreeSurfer", true,
        "the histogram segmentation of white matter (MRIhistoSegment) crashes on a volume of integers or longs (no region histogram to smooth) and on a region whose range needs 2^31 bins or more";
    FS_502 = "FS-502", "FreeSurfer", true,
        "the histogram segmentation of white matter (MRIhistoSegment) never ends when its search for the gray-matter peak comes back to a peak it has found (a gray-matter limit below -1 once both peaks are found)";
    FS_503 = "FS-503", "FreeSurfer", true,
        "the thresholded region histogram (MRIhistogramRegionWithThreshold) with several threads adds per-thread histograms of fractional counts, whose sums depend on the number of threads";
    FS_504 = "FS-504", "FreeSurfer", true,
        "the transform-array functions (LTAconcat, LTAreduce, LTAmriIsSource, LTAmriIsTarget, LTAprint, LTAchangeType's diagnostic print, the writers LTAwrite and LTAwriteEx, LTAtransformAtPoint, LTAvoxelTransformToCoronalRasTransform, DevolveXFM) given no array, an array without transforms or a transform without a 4 x 4 matrix read memory they do not own or crash";
    FS_505 = "FS-505", "FreeSurfer", true,
        "filling a transform array's inverses (LTAfillInverse, and every function that calls it) computes the inverse of a transform whose inverse has no matrix and loses it: the inverse stays without a matrix";
    FS_506 = "FS-506", "FreeSurfer", true,
        "changing a transform array from physical-voxel to RAS-to-RAS (LTAchangeType, and every change that passes through it) converts the matrix as if it were RAS-to-RAS before scaling it: the result is not the RAS-to-RAS transform";
    FS_507 = "FS-507", "FreeSurfer", true,
        "changing a transform array of several transforms from a tkregister registration to RAS-to-RAS (LTAchangeType) converts the first transform once for every transform and leaves the others unconverted";
    FS_508 = "FS-508", "FreeSurfer", true,
        "changing a transform array to FSL's type (LTAchangeType, directly or through RAS-to-RAS) when FSL's conversion gives no matrix (a registration that cannot be inverted) frees the transform's matrix and goes on using it: memory it no longer owns read";
    FS_509 = "FS-509", "FreeSurfer", true,
        "mapping a segmentation onto another grid with a tissue-type vote (MRIaseg2vol with a colour table) reads the table at a label it does not hold (outside it or an empty place), and counts tissue types of 100 or more and more than 1,000 labels in one voxel past its arrays";
    FS_510 = "FS-510", "FreeSurfer", true,
        "mapping a segmentation onto another grid with a tissue-type vote (MRIaseg2vol with a colour table) decides whether a voxel has a clear winner from the hits of its highest label instead of the label with the most hits, so a label with half or more of the voxel can lose to the tissue-type vote";
    FS_511 = "FS-511", "FreeSurfer", true,
        "applying an SPM warp field (MRIapplySpmWarp) stores, where a trilinear sample of a volume type it cannot read fails before any sample of the call gave a value, a value never written";
    FS_512 = "FS-512", "FreeSurfer", true,
        "concatenating two volumes along the slices (MRIzconcat) copies as many rows as the volumes have columns: rows past the width stay 0, and where the width is larger it reads and writes rows the volumes do not have";
    FS_513 = "FS-513", "FreeSurfer", true,
        "shifting a volume along its rows or slices (MRIshiftDim) wraps and drops the shifted indices by the number of columns instead of the number of rows or slices";
    FS_514 = "FS-514", "FreeSurfer", true,
        "labelling the amygdala-cortex junction (LabelAmygalaCortalJunction) with several threads writes the left and the right label into a cortex voxel next to both amygdalae in an order that depends on the threads";
    FS_515 = "FS-515", "FreeSurfer", true,
        "cropping around an RAS point (MRIcropAroundRAS) of a volume whose orientation names an axis twice, or is unknown, crops to a field of view that is never set: memory contents";
    FS_516 = "FS-516", "FreeSurfer", true,
        "cropping around an RAS point through an LTA (MRIcropAroundRAS) decides the LTA's direction backwards (it inverts the LTA when the volume is its destination, goes on when the volume is neither end, refuses when it is both) and compares the volume's validity flag, which no volume constructor sets";
    FS_517 = "FS-517", "FreeSurfer", true,
        "cropping around a voxel (MRIcropAroundCRS) whose last voxel lies beyond the int range or at its maximum (a centre too far away, or one that is not a number) overflows the crop's limits or its loop counter: as the official build compiles it, the crop crashes the program or returns a volume holding none of the volume's voxels, depending on the axis";
    FS_518 = "FS-518", "NrrdIO", true,
        "a NRRD space named 3D-left-handed or 3Drighthanded is not recognised (the read fails), 3D-right-handed-time is read as 3D-left-handed and 3D-left-handed-time as 3D-right-handed-time (two names of its table run together)";
    FS_519 = "FS-519", "NrrdIO", true,
        "an axis kind given as none resets that axis's centering (the centers field) to unknown instead of the kind";
    FS_520 = "FS-520", "NrrdIO", true,
        "a NRRD file with an axis of kind 3-gradient ends the program after reading its data (nrrdKindSize: PANIC: nrrdKind 20 not implemented!, exit status 1)";
    FS_761 = "FS-761", "NrrdIO", true,
        "a NRRD comment line holding a character that is neither printable nor white space (a control character, or any byte of 0x80 or above, such as UTF-8 text) makes the reader loop forever";
    FS_762 = "FS-762", "NrrdIO", true,
        "an error message longer than 256 bytes (a long file name, a long header line or value) overflows the reader's message buffer: the program aborts instead of printing it";
    FS_763 = "FS-763", "NrrdIO", true,
        "the last line of a NRRD header, a data file list or a skipped line that ends at the end of the file without a newline is dropped (a detached header ending in its data file line reads as having none)";
    FS_764 = "FS-764", "NrrdIO", true,
        "the message for a space dimension beyond 8 shows the array's dimension instead of the space dimension";
    FS_765 = "FS-765", "NrrdIO", true,
        "a sizes, spacings, thicknesses, axis mins or axis maxs line with more than 16 values for a 16-dimensional array is parsed one value past the reader's array (memory beside it is written) before the read fails";
    FS_766 = "FS-766", "NrrdIO", true,
        "a data file name template (data file: name%03d.raw ...) holding a second conversion (another %d, %s) formats it from no argument, and one wider than ten digits writes past the name's buffer";
    FS_767 = "FS-767", "NrrdIO", true,
        "an ASCII-encoded value longer than 1,024 bytes overflows the reader's token buffer";
    FS_768 = "FS-768", "NrrdIO", true,
        "a NRRD whose sample count times sample size passes 2^64 bytes allocates the wrapped size and reads its data past it";
    FS_769 = "FS-769", "NrrdIO", true,
        "a byte of 0x80 or above in hex-encoded data is read as the character of its low 7 bits (0xb1 as 1, 0xa0 as a space) instead of ending the read as an invalid character";
    FS_770 = "FS-770", "NrrdIO", true,
        "a detached NRRD header in the root directory (/name.nhdr) whose data file name is relative fails (need nio->path for header-relative datafiles)";
    FS_771 = "FS-771", "NrrdIO", true,
        "a NRRD header that gives its data file before its dimension reads an axis far outside its array (the program crashes)";
    FS_772 = "FS-772", "NrrdIO", true,
        "the message for a data file list that fails its check shows the reader's line buffer at the field's offset: bytes of earlier header lines, or memory never written when the buffer grew";
    FS_773 = "FS-773", "NrrdIO", true,
        "on Windows a header name whose directories are separated by backslashes, and data file names that start with a drive or a backslash, are taken as relative to the current directory: detached data files are not found";
    FS_774 = "FS-774", "MINC", true,
        "a compressed MINC file (.gz, .bz, .bz2, .Z, .z, .zip, or a missing name whose compressed file exists) is decompressed by running gunzip -c (or bunzip2 -c, zcat, pcat) through the shell with the file name unquoted: a name with a blank or shell syntax fails, reads other files or runs commands";
    FS_775 = "FS-775", "FreeSurfer", true,
        "a MINC image of type double cannot be read: the program ends with bad data type (exit status 1)";
    FS_776 = "FS-776", "netCDF", true,
        "a variable of a version 1 netCDF file whose data offset is 2 GiB or more is read at a negative offset: memory beside the reader's buffer, or an Invalid argument error";
    FS_777 = "FS-777", "netCDF", true,
        "a netCDF variable whose dimension sizes multiply to a multiple of 2^64 crashes the reader (a division by zero)";
    FS_778 = "FS-778", "netCDF", true,
        "a netCDF header whose dimension, attribute or variable list (or an attribute's values) announces more entries than the file holds is read on through zeros until memory runs out";
    FS_779 = "FS-779", "MINC volume_io", true,
        "a MINC file without an image variable, or with more dimensions than the volume can have, makes the volume input use a dimension count it never set and go on with no volume (a crash or an error, by memory contents)";
    FS_780 = "FS-780", "MINC volume_io", true,
        "a time dimension whose variable has no spacing attribute is read as regular or irregular by memory contents";
    FS_781 = "FS-781", "MINC volume_io", true,
        "a MINC image of type char (or of a type netCDF does not know) leaves the volume's type unset: the read fails or goes on by memory contents (an allocation message, then an ICV is not attached to an image variable error that ends the program)";
    FS_782 = "FS-782", "MINC volume_io", true,
        "a MINC dimension name longer than 64 bytes overflows the volume input's name buffer";
    FS_783 = "FS-783", "MINC volume_io", true,
        "a MINC image of more than 512 dimensions overflows the volume input's and MINC's dimension arrays";
    FS_784 = "FS-784", "MINC", true,
        "an empty string attribute (signtype, spacetype, spacing) is checked one byte before MINC's buffer and read as an empty string or as what the buffer held";
    FS_785 = "FS-785", "MINC", true,
        "an image-max or image-min variable over a dimension the image does not have is read at an index never set (memory contents)";
    FS_786 = "FS-786", "MINC", true,
        "a value that is not a number (a slice range of NaN) converted to an integer voxel gives x86's undefined conversion (INT_MIN for an int image, 0 otherwise)";
    FS_787 = "FS-787", "MINC", true,
        "a valid range outside its integer type's range (or not a number) is converted to the type with C's undefined conversion (x86's value)";
    FS_788 = "FS-788", "FreeSurfer", true,
        "a compressed MINC file (.mnc.gz and the other compressed endings) cannot be read: the program ends with Cannot uncompress the file (exit status 1)";
    FS_789 = "FS-789", "MINC volume_io", true,
        "a MINC volume of 2^32 voxels or more counts its voxels in 32 bits: its data are allocated too small and read past their end";
    FS_790 = "FS-790", "MINC volume_io", true,
        "a MINC volume with a dimension of size 0 (a time dimension without records) is read into no data";
    FS_791 = "FS-791", "MINC volume_io", true,
        "a direction_cosines attribute of fewer than three values takes its missing values from the dimension read before it";
    FS_793 = "FS-793", "netCDF", true,
        "values of a netCDF file shorter than its header are read past its end as bytes of an earlier block of the reader's buffer where netCDF means zeros";
    FS_794 = "FS-794", "MINC volume_io", true,
        "a MINC volume with a dimension of size 1 whose order differs from the volume's (one x slice of a zspace, yspace, xspace file read as x, y, z) is read with each slab written as one run from its first voxel: slabs overwrite each other and the rest of the volume is never written";
    FS_795 = "FS-795", "MINC volume_io", true,
        "... such a run that passes the end of the volume's array writes memory beside it";
    FS_796 = "FS-796", "MINC volume_io", true,
        "... and the voxels no slab wrote keep the memory the volume's array was given";
    FS_792 = "FS-792", "MINC volume_io", true,
        "a direction_cosines attribute of fewer than three values on the first dimension read with direction cosines takes its missing values from memory never written";
    FS_798 = "FS-798", "MINC volume_io", true,
        "a volume with a dimension of size 0 is allocated with its pointers set up into an allocation of no bytes: allocating it or writing it as MINC crashes or reads memory";
    FS_799 = "FS-799", "MINC volume_io", true,
        "a volume whose voxels were never allocated, written as MINC, is read through a null or unset pointer (a crash) unless its conversion fails first";
    FS_800 = "FS-800", "MINC volume_io", true,
        "a volume without spatial axes whose geometry was changed gets its voxel-to-world transform from memory never written (compute_world_transform returns without one): written as MINC, its output reports a transform that is not linear and deleting it an internal error";
    FS_801 = "FS-801", "MINC volume_io", true,
        "a volume created without a type (MI_ORIGINAL_TYPE) has a voxel range and sign that were never written: written as MINC, its valid range, image range and conversion come from memory contents";
    FS_797 = "FS-797", "FreeSurfer", true,
        "a MINC volume with a time dimension (more than one frame) cannot be written: the program ends with convert_spatial_axis_to_dim_name, bad axis (exit status 1) before the file is created";
    FS_802 = "FS-802", "FreeSurfer", true,
        "finding bright voxels outside the white matter (MRIfindBrightNonWM) takes each T1 value as the low byte of its truncation to an integer: a value above 255, below 0 or not a number wraps (300 is not bright, 400 is)";
    FS_803 = "FS-803", "FreeSurfer", true,
        "binarizing a frame range that does not start at frame 0 (BinarizeMRI with a first frame above 0) writes the off value of the voxels outside the mask or on a zeroed edge into the input's frame number instead of the output's: the output's first frames keep 0 there, and frames past its end are written outside it";
    FS_804 = "FS-804", "FreeSurfer", true,
        "dilating or eroding a binary volume (DEMorphBinVol) with a frame other than 0 reads each voxel's own value from that frame of its one-frame working copy (memory outside it), its neighbours from frame 0";
    FS_805 = "FS-805", "FreeSurfer", true,
        "dilating or eroding a thresholded or label-matched volume (DEMorphBinVol's thresholds and match lists) with a frame other than 0 selects that frame again from the one-frame binarized volume: frame 1 reads past it, a later frame gives no output";
    FS_806 = "FS-806", "FreeSurfer", true,
        "DEMorphBinVol accepts a frame equal to the number of frames (its checks allow one past the last) and reads that frame past the volume";
    FS_807 = "FS-807", "FreeSurfer", true,
        "dilating or eroding a binary volume (DEMorphBinVol) with a neighbourhood other than 1 to 3, a morphology type other than 1 or 2, or a frame out of range prints its error and goes on: a result is made with those settings";
    FS_808 = "FS-808", "FreeSurfer", true,
        "dilating or eroding a thresholded or label-matched volume of several frames (DEMorphBinVol's thresholds and match lists) copies the chosen frame into bytes first (values rounded and clamped to 0 to 255), so its thresholds and labels compare with the bytes; a one-frame volume is compared as it is";
    FS_809 = "FS-809", "zlib 1.2.11", true,
        "deflatePrime called while compressed bytes wait to be handed out behind some already handed out (a deflate call that ran out of output space) puts its bits at the position given by the number of waiting bytes instead of after them: it overwrites bytes already handed out, and the next deflate call hands out bytes it never wrote (memory contents)";
    FS_810 = "FS-810", "zlib 1.2.11", true,
        "deflatePrime does not check that its bits fit in the output buffer behind the waiting bytes: repeated calls while bytes wait write past the buffer, and the next deflate call reads past it";
    FS_812 = "FS-812", "zlib 1.2.11", true,
        "deflateGetDictionary after a switch to storing (deflateParams to level 0 right after deflateResetKeep, which does not flush, keeping bytes read ahead) copies from past the end of its window: the dictionary's last bytes are memory contents";
    FS_811 = "FS-811", "zlib 1.2.11", true,
        "storing (level 0) can slide the window down while more bytes wait to be hashed than lie before the current position; after a switch to a compressing level (deflateParams) the next fill of the window reads far before its start and the program crashes (zlib 1.2.12 limits the count to the position)";
    FS_813 = "FS-813", "IJG libjpeg 6b", true,
        "a JPEG file whose scan names one component twice (two frame components with one identifier, or an identifier repeated in the scan's list) is decoded without a complaint: in a file of one scan the component left out keeps the memory contents of the decoder's sample buffer, so its colour comes from whatever that memory held; in a file of several scans the second copy decides that component's coefficients (libjpeg 9 refuses such a scan)";
    FS_814 = "FS-814", "libtiff 3.6.1", true,
        "counting the directories of a TIFF file whose chain of directories loops back (a directory's link points to an earlier one) never returns; corrected, the count stops at the first directory seen again";
    FS_815 = "FS-815", "libtiff 3.6.1", true,
        "a directory entry or a strip count whose size in bytes does not fit 32 bits is allocated with its size cut to 32 bits and then written past (a crash); refused as an allocation failure, with libtiff's No space message";
    FS_816 = "FS-816", "libtiff 3.6.1", true,
        "a directory with a second Compression field that names another codec with private tags (LZW, Deflate, the CCITT codecs, PixarLog, SGILog) keeps the first codec's tag methods after freeing it: the next TIFFSetField or TIFFGetField runs them on freed or another codec's state (a crash or an endless recursion); the current codec's methods are used";
    FS_817 = "FS-817", "libtiff 3.6.1", true,
        "decoders read past a strip's data: a PackBits run or literal at the end of the data takes bytes an earlier strip left in libtiff's buffer, and the test for old-style LZW codes looks at two bytes of a one-byte strip; corrected as libtiff 4 does, decoding stops at the end of the data and a strip under two bytes is never old-style";
    FS_818 = "FS-818", "libtiff 3.6.1", true,
        "LZW decoding of damaged data uses table entries the decoder never writes (codes 256 and 257 right after a Clear code: memory contents), the decoder for old-style LZW codes follows null links (a crash) and writes before the start of its row, and a table entry linked to itself is followed for ever (a hang); read as zeros, decoding stopped, nothing written there";
    FS_819 = "FS-819", "libtiff 3.6.1", true,
        "a decoder that reads past a strip's data reads bytes of libtiff's raw buffer that no read has written since the buffer was allocated (memory contents); read as 0";
    FS_820 = "FS-820", "libtiff 3.6.1", true,
        "the type check of a directory entry whose type matches none of its field's entries, when that field is the last of its table, reads the memory after the table, and checks no later entry of the directory: those are read without their type and count checked, and an unknown tag among them crashes the reader; corrected, every entry is checked";
    FS_821 = "FS-821", "libtiff 3.6.1", true,
        "a directory of no strips (an image length of 0) without StripByteCounts: estimating the counts divides by zero (a crash) or trims the strip at index 65535 of empty arrays (memory outside them); nothing is estimated";
    FS_822 = "FS-822", "libtiff 3.6.1", true,
        "per-sample fields (BitsPerSample, Compression, MinSampleValue, MaxSampleValue, SampleFormat) given as LONG values for several samples are read as the 16-bit halves of the values, so they differ (Cannot handle different per-sample values), and past five samples the reader writes past its buffer; corrected, they are read as LONG values";
    FS_823 = "FS-823", "libtiff 3.6.1", true,
        "an SMinSampleValue or SMaxSampleValue of type ASCII, UNDEFINED or an unknown type: libtiff's error message passes no type, so its number and its field name come from memory never given (garbage or a crash); the message is written with the type";
    FS_824 = "FS-824", "libtiff 3.6.1", true,
        "ExtraSamples of more than four BYTE values are read as half as many 16-bit values, the rest of the array being memory never written, so the samples are refused or set from that memory; corrected, the bytes are the values";
    FS_825 = "FS-825", "libtiff 3.6.1", true,
        "a one-character ASCII or UNDEFINED value of a field that passes its count (an unknown tag, InkNames) is passed with the string where the count goes: the count is an address (an allocation of any size) and the copy reads a pointer never given (a crash or a failure); set as one value";
    FS_826 = "FS-826", "libtiff 3.6.1", true,
        "reading a strip or tile of a directory whose strip arrays were never read (a TIFFReadDirectory that failed) or past their end reads a null pointer or memory outside the arrays (a crash); refused";
    FS_827 = "FS-827", "libtiff 3.6.1", true,
        "writing a rational value that is NaN, infinite or above 4294967295 converts it to an unsigned 32-bit integer out of range (undefined; x86-64 gives 0 or its low 32 bits); clamped to 0 or 4294967295";
    FS_828 = "FS-828", "libtiff 3.6.1", true,
        "a directory that counts more entries than it fills writes the rest as memory never written; written as zeros";
    FS_829 = "FS-829", "libtiff 3.6.1", true,
        "the PixarLog decoder sizes its row buffer by the rows per strip in 32 bits and never checks the allocation: where it fails (the default of 2^32 - 1 rows per strip asks for about 8 GiB), zlib is given a null output and every row fails with a zlib error; corrected, the buffer is what a row needs";
    FS_830 = "FS-830", "libtiff 3.6.1", true,
        "after a TIFFReadDirectory that fails, the scanline and tile sizes of the previous directory are kept: TIFFReadScanline and TIFFReadTile write that many bytes into a buffer sized for the new directory (memory past it); only the buffer is written";
    FS_831 = "FS-831", "libtiff 3.6.1", true,
        "when a strip's start fails in the codec's setup (an unsupported predictor), reading the next row of that strip calls the LZW or Deflate decoder without a start: its input pointers were never set (a crash); nothing is decoded";
    FS_832 = "FS-832", "libtiff 3.6.1", true,
        "the predictor's horizontal differencing works by whole pixels while bytes are left, so a row whose byte count is not a multiple of the pixel's (a scanline size left from another directory) is processed past its end; past the caller's buffer nothing is touched";
    FS_833 = "FS-833", "libtiff 3.6.1", true,
        "a negative strip or tile size (TIFFTileSize of a stripped image overflows 32 bits; kept after a TIFFReadDirectory that failed) makes the None decoder copy, the Deflate decoder inflate into, and the byte swap of 16-, 32- and 64-bit samples swap, about 2^64 bytes (memory past the buffers); nothing is written";
    FS_834 = "FS-834", "libtiff 3.6.1", true,
        "a SamplesPerPixel of several values, of a floating-point type or of a string type (and such an entry left unchecked by FS-820) is passed as an array's or a string's address, or as a floating-point argument, where one integer goes: the field takes an address's low bits or a register never set (nondeterministic); ignored with libtiff's incorrect-count or wrong-type warning";
    FS_835 = "FS-835", "libtiff 3.6.1", true,
        "the NeXT, ThunderScan and CCITT decoders write past the buffer they are given when the data asks for more pixels than a row holds (a span or a run past the row, a row size larger than the buffer): memory after the buffer; only the buffer is written";
    FS_836 = "FS-836", "libtiff 3.6.1", true,
        "the NeXT decoder reads a row's code byte and a literal span's four-byte header without checking that the strip still has them, does not check a span against its row, lets a run of pixels go past the image width, and counts a tile's pixels by the image's width; corrected as libtiff 4.0.10 does: the reads and spans checked, runs stopped at the row's end, a tile's width used";
    FS_837 = "FS-837", "libtiff 3.6.1", true,
        "the ThunderScan decoder writes a run of pixels, and the deltas of a byte, in full when they go past the row's pixel count: the pixels land in the next row or past the buffer; corrected, nothing is written past the row (the pixel count and its error message unchanged)";
    FS_838 = "FS-838", "libtiff 3.6.1", true,
        "the CCITT decoders keep the runs of the current and the reference line in one block that damaged data makes them read or write outside (a crash or memory contents) or read where nothing was written (memory contents), and a row size of 0 bytes never returns; corrected as libtiff 4 does: the block cleared, the row stopped at the first access outside it with a Buffer overflow error, nothing decoded for a row size of 0";
    FS_839 = "FS-839", "libtiff 3.6.1", true,
        "the CCITT RLE/W decoder aligns each row to a 16-bit word by the bits it holds, not by the row's position: a row that ends with the decoder holding bits read from the next word loses them, and the next row starts a word late (Premature EOF or wrong rows of a valid image); corrected, each row starts at the next 16-bit boundary of the strip";
    FS_840 = "FS-840", "libtiff 3.6.1", true,
        "the PixarLog decoder inflates past its row buffer when a call needs more than the rows per strip gave it, looks the first codes of a row up through its tables without masking them to 11 bits (memory past the tables), and for 11- and 12-bit samples, whose scanline holds fewer codes than a row, reads codes never inflated (memory contents) and writes past the caller's buffer; corrected as libtiff 4.0.10 does: the buffer what a row needs, the first codes masked, the codes cut to whole rows with its warning";
    FS_881 = "FS-881", "libtiff 3.6.1", true,
        "the PixarLog decoder for 12-bit samples (PICIO's) with one, two or more than four samples per pixel gives each pixel the next pixel's value, so the second pixel's value is lost and the row's last values come from past the row; corrected, each pixel gets its own value, as the 8-bit, 16-bit and floating-point decoders give it";
    FS_882 = "FS-882", "libtiff 3.6.1", true,
        "the SGILog decoders size their translation buffer by the image width times the rows per strip in an int: the default of 2^32 - 1 rows per strip makes it negative, so every LogL or LogLuv image without RowsPerStrip (and every tiled one) fails with No space for SGILog translation buffer, and a product past 2^31 gives a buffer smaller than a row; corrected as libtiff 4.0.10 does: a strip's (the rows per strip at most the image length) or a tile's pixels";
    FS_883 = "FS-883", "libtiff 3.6.1", true,
        "the SGILog decoders' run and literal loops do not stop at the row's pixel count (they write on past it and the row fails as short by a negative count), a run reads its two bytes and a 24-bit LogLuv pixel its three when fewer are left, and rows of 0 bytes never end; corrected as libtiff 4.0.10 does: the loops stop at the row's end and at the data's";
    FS_884 = "FS-884", "libtiff 3.6.1", true,
        "the SGILog decoders write pixels past their translation buffer when it is smaller than a row (FS-882's sizes) or a run goes past the row (FS-883): memory past the buffer; not written";
    FS_885 = "FS-885", "libtiff 3.6.1", true,
        "a CCITT file whose FaxSubAddress entry comes before its Compression entry: the reader registers the tag as an unknown one that passes its count before the codec registers its own, and then hands the codec's tag method the count where the string's address goes (a crash); set from the string";
    FS_886 = "FS-886", "FreeSurfer", true,
        "a transform array's weighted transform at a point (LTAtransformAtPoint, and LTAtransformInterp of several transforms) keeps the weights of at most 4,096 transforms: an array of more writes past them";
    FS_887 = "FS-887", "FreeSurfer", true,
        "making a subject's transform work with tkregister's coordinates (DevolveXFM, DevolveXFMWithSubjectsDir) builds the paths of the subject's folder, original volume and transform in 2,000 bytes: a longer path writes past them";
    FS_888 = "FS-888", "FreeSurfer", true,
        "a voxel list of a range's border voxels (VLSTcreate with border_only) counts the voxels whose six neighbours all lie in the range and then stores the border voxels: the list's count is the interior's, its last voxels zeros, or its arrays too short for the border's (writes past them); corrected: the border voxels are counted";
    FS_889 = "FS-889", "FreeSurfer", true,
        "a voxel list made with a negative skip (VLSTcreate) never ends (skip -1) or reads frames before the first";
    FS_890 = "FS-890", "FreeSurfer", true,
        "the voxel-list functions (VLSTcreate, VLSTsort, VLSTcopy, VLSTcopyInto, VLSTadd, VLSTinList, VLSTcomputeStats, VLSTaddWithValue) read or write past a list's arrays when a count or an index goes beyond what was allocated, and a sort of a destination list of more voxels than its source writes past its sort table";
    FS_891 = "FS-891", "FreeSurfer", true,
        "sorting a voxel list (VLSTsort) moves each voxel's indices, coordinates and values but not its frame index: after the sort the voxels of a list of several frames carry other voxels' frames; corrected: the frames move with their voxels";
    FS_892 = "FS-892", "FreeSurfer", false,
        "a voxel list made from a volume (VLSTcreate) keeps its maximum count at 0 (a new list) or the given list's: adding a voxel to it (VLSTadd) is refused as too many voxels; kept (no program adds to such a list)";
    FS_893 = "FS-893", "FreeSurfer", false,
        "copying a voxel list with spline slopes (VLSTcopy, VLSTcopyInto) gives the copy slope arrays of zeros instead of the source's slopes, and ends the program when the source has its first slope array and the copy then lacks one of the three slope arrays or the parameterisation (the source without it and the destination too); kept (no program copies a spline list)";
    FS_894 = "FS-894", "CTN", false,
        "the messages of an element that is not found name the routine DCM_GetElementVM for a request of DCM_GetElement, and DCM_GetElementValue's message for a group that holds no element (a group whose only element was its group length, read with DCM_NOGROUPLENGTH) shows the group's number where the element's goes; kept (messages only)";
    FS_951 = "FS-951", "CTN", true,
        "the value of a sequence asked for with another representation (DCM_GetElementValue refuses only a request that itself says SQ) is copied from the memory that holds the sequence's list of items: memory contents, or a crash past it; refused as for a sequence (Cannot retrieve value of element with SQ representation)";
    FS_952 = "FS-952", "CTN", true,
        "a request of 4,294,967,295 bytes of pixel data left in the file (an encapsulated value, whose length is undefined, asked for whole) reads nothing, the read failing, and the failure is taken for a complete read: the request succeeds with nothing written into the caller's buffer (memory never written); corrected: the read is reported as failed (DCM failed to access file)";
    FS_953 = "FS-953", "CTN", true,
        "an odd number of bytes of 16-bit pixel data in the reverse byte order (big endian) asked for (a request of an odd length) is swapped two bytes at a time past its end until the 32-bit count wraps: the program crashes; the whole pairs are swapped and the last byte is kept";
    FS_954 = "FS-954", "FreeSurfer", true,
        "converting a transform array from RAS to voxel coordinates (LTArasToVoxelXform) stores every converted matrix in the first transform: an array of several transforms ends with the first holding the last one's matrix converted and the others unconverted; corrected: each transform converted in place";
    FS_955 = "FS-955", "FreeSurfer", false,
        "converting a transform array from voxel to RAS coordinates without volumes (LTAvoxelToRasXform from the transforms' own geometries) applies the conversion from RAS to voxel coordinates (the destination's RAS-to-voxel matrix times the matrix times the source's voxel-to-RAS matrix), as before FreeSurfer's 2018 change, which kept this output on purpose; with volumes the conversion is the right one; kept (a deliberate choice of the original)";
    FS_956 = "FS-956", "FreeSurfer", true,
        "composing a transform with a matrix on the left and one on the right into another array (LTAcompose) gives only the right product: the left one is computed and then overwritten (composing in place gives both); corrected: the right matrix multiplies the left product";
    FS_957 = "FS-957", "FreeSurfer", true,
        "converting a transform array from RAS to voxel coordinates with no destination volume (LTArasToVoxelXform) converts the matrices with the source as the destination, then copies the geometry of the missing volume: the program crashes; corrected: the source's geometry for both";
    FS_958 = "FS-958", "libtiff 3.6.1", true,
        "a scanline or tile of more than 0 bytes read into no buffer (TIFFReadScanline or TIFFReadTile given a null pointer): the codecs other than Deflate write the decoded bytes through the null pointer (a crash) once they decode any (Deflate's zlib refuses it: `zlib error`, -1), and the byte swap of 16-, 32- and 64-bit samples of a file of the other byte order swaps them there (also after a codec that is not implemented, whose -1 is taken as success); decoded and dropped";
    FS_959 = "FS-959", "libtiff 3.6.1", true,
        "starting a strip or tile divides by a count of 0 (the strips per image, from a strip count that overflows 32 bits; the tiles across or down, from a tile width or length near 2^32 whose rounding overflows): the program ends with SIGFPE; corrected: row, column and sample 0";
    FS_960 = "FS-960", "FreeSurfer", true,
        "the volume operations that walk a volume's rows directly (MRIinvertContrast and MRIscaleAndMultiply on another type than bytes, MRItranslate and MRIrotate_I into another type, mri_framepower, the byte walks of the set and label operations) find every row of a tensor volume, of every slice and frame, at the volume's first voxel: each row reads the same first values, and each row written replaces them, the rest of the volume unchanged; corrected: each row at its own place";
    FS_961 = "FS-961", "FreeSurfer", true,
        "downsampling a volume by a negative or infinite factor goes on with extents of 0 or below: the new volume's allocation then fails with memory errors, an infinite factor makes a volume 0 voxels wide, and a given destination is refused as a dimension mismatch; refused with a clear error by default";
    FS_962 = "FS-962", "FreeSurfer", true,
        "doubling a volume's resolution with cubic B-splines (MRIupsample2BSpline) on a volume one row high or one slice deep copies the source's own voxels over the rows (slices) the earlier steps doubled: each output row (slice) gets the source's voxels at the doubled columns, read past the source's extent (other voxels of the source or memory it does not own); corrected: the doubled rows (slices) are kept";
    FS_963 = "FS-963", "FreeSurfer", false,
        "halving or doubling a volume's resolution with cubic B-splines (MRIdownsample2BSpline, MRIupsample2BSpline) changes the voxel size of an axis of one voxel, which neither resamples: halving doubles it and moves the voxel's centre by half a source voxel, doubling halves it; kept until measured with the two-dimensional registration that reaches it (rule D)";
    FS_964 = "FS-964", "FreeSurfer", true,
        "sampling a B-spline (one allocated and never computed, as the computation refuses degrees other than 2 to 9) whose degree is above 9, or below 0 on a volume of one voxel, writes interpolation indices past their arrays or reads indices never set: memory contents or a crash; refused with a clear error (degrees 0 and 1 on a volume of one voxel read its coefficient, as FreeSurfer does); under `reproduce`, a degree above 9 on coefficients with an axis longer than one voxel ends as FreeSurfer does after its stray writes (`Invalid spline degree` on standard output, status 1; a crash where the writes reach past the stack's top, at degrees 30,000 to 1,000,000 in the probe, not at 2,147,483,647)";
    FS_965 = "FS-965", "FreeSurfer", true,
        "a histogram whose bin count is taken from the values' range (one bin per unit, asked for with 0 bins or fewer) of values spanning more than 67,108,864 asks for that many bins: gigabytes of memory allocated and filled, the program slow or killed; refused with a clear error";
    FS_966 = "FS-966", "FreeSurfer", true,
        "the principal axes of an image's intensities (the robust registration's initial orientation) are sorted by finding the largest and the smallest eigenvalue; when none is above another (all equal, or NaN) both are the first and the middle axis is read from a fourth column of the 3 x 3 eigenvector matrix: two numbers of the first column and one past the matrix (memory it does not own); corrected: the eigensystem's own order";
    FS_967 = "FS-967", "FreeSurfer", true,
        "the principal axes of an image with an infinite or NaN voxel have no covariance: the eigensystem then never returns (the program hangs) or stops without eigenvectors, which are read all the same (memory never written); refused with a clear error by default";
    FS_968 = "FS-968", "FreeSurfer", true,
        "the distance between two rigid transforms (the robust registration's convergence test) inverts the first with a singular value decomposition; on a matrix with an infinite or NaN entry the decomposition can stop without converging (a warning on standard error), and the distance is computed from the decomposition reached all the same (an arbitrary number); refused with a clear error by default";
    FS_969 = "FS-969", "FreeSurfer", true,
        "the matrix square root of the robust registration (and the complex Schur form it is computed from) of a matrix with an infinite or NaN element: the Schur form's QR iteration does not converge (`info = 2, something went wrong` and `the QR iteration failed` on standard error) or the result is NaN, and the NaN matrix is returned all the same, the checks of the result never failing on NaNs; refused with a clear error by default";
    FS_970 = "FS-970", "FreeSurfer", true,
        "the matrix square root of the robust registration of an empty (0 x 0) matrix: ` ** On entry to ZGEES parameter number  6 had an illegal value` on standard output and the end of the program with status 0; refused with a clear error and status 1 by default";
    FS_1061 = "FS-1061", "FreeSurfer", false,
        "a robust registration step of an image with more than 2,147,483,647 voxels and frames gives some voxels the weights of others; unreachable in practice (it needs more than 60 GB of memory), noted, computed as FreeSurfer computes it";
    FS_1062 = "FS-1062", "FreeSurfer", false,
        "a subsampled robust registration step (`--subsample`) estimates its rotation, scale and shear parameters as if every voxel position were halved; kept until measured with the registrations that reach it (rule D)";
    FS_1063 = "FS-1063", "FreeSurfer", true,
        "the robust registration in double precision with an intensity scale (`--doubleprec --iscale`) builds each step's transformation from memory contents instead of the step's solution, and the Powell registration (`--cost` other than ROB) with an intensity scale (`--iscale`) builds each evaluation's transformation of some models (translation 3-D, rigid 2-D, isotropic scale 3-D) from memory contents instead of its parameters; larmorx uses the solution, and the parameters, in both modes";
    FS_1064 = "FS-1064", "FreeSurfer", true,
        "the robust registration's median and median absolute deviation of an empty list return memory contents; NaN in both modes";
    FS_1065 = "FS-1065", "FreeSurfer", true,
        "the robust location estimate of a set of values (Tukey's biweight, `mri_robust_template --average 2`) crashes unless the values are all almost equal; larmorx computes it in both modes";
    FS_1066 = "FS-1066", "FreeSurfer", true,
        "the least-squares estimate of a single location returns memory contents instead of the mean; larmorx returns the mean in both modes (no program reaches it)";
    FS_1067 = "FS-1067", "FreeSurfer", true,
        "a robust registration step with fewer usable voxels than the transformation has parameters gives memory contents (the robust solve) or an arbitrary solution (least squares); refused with a clear error and status 1 by default";
    FS_1068 = "FS-1068", "FreeSurfer", true,
        "the half-way maps of a robust registration that ran no iteration (the finest level skipped with no coarser one) are read from empty matrices: a crash or memory contents; an error in both modes";
    FS_1069 = "FS-1069", "FreeSurfer", true,
        "a robust registration step of two images of different sizes (intensity scaling only, without symmetry, of images of different extents) reads past the smaller one: a crash or memory contents; an error in both modes";
    FS_1070 = "FS-1070", "FreeSurfer", true,
        "mri_robust_register given an option that takes an argument as its last argument crashes: it aborts (an uncaught std::logic_error, status 134) for a name and dies of a segmentation fault for a number; by default an error naming the option, status 1";
    FS_1071 = "FS-1071", "FreeSurfer", true,
        "mri_robust_register given an --lta name of 1,024 bytes or more aborts (*** buffer overflow detected ***, status 134) after printing its results, without writing the transform; by default the name is used";
    FS_1073 = "FS-1073", "FreeSurfer", true,
        "the robust registration in double precision with an intensity scale (`--doubleprec --iscale`), and the Powell registration (`--cost` other than ROB) with an intensity scale (`--iscale`), of a rigid 3-D, affine 2-D or 3-D, isotropic-scale 2-D or translation 2-D model builds each step's (each evaluation's) transformation with its first two parameters replaced by memory contents (with glibc's allocator: a tiny positive number and 0: the translation along x and y lost); by default the step's solution (the evaluation's parameters)";
    FS_1072 = "FS-1072", "FreeSurfer", true,
        "mri_robust_register given a non-linear initial transform (--ixform of a morph) reads the morph as a linear transform array: memory it does not own; an error in both modes";
    FS_1074 = "FS-1074", "FreeSurfer", true,
        "mri_robust_register's entropy images (--cost ROBENT) with an --entradius below 0 abort (an uncaught std::bad_array_new_length, status 134) for a 3-D image and give an image of zeros for a 2-D one; a radius whose box of weights holds 2^31 or more entries aborts or writes past its weights; by default an error, status 1";
    FS_1075 = "FS-1075", "FreeSurfer", true,
        "mri_robust_register's half-way outputs (--halfmov, --halfdst, --halfmovlta, --halfdstlta, --halfweights) after a registration with a cost other than ROB and ROBENT take the half-way space's geometry from memory the program has already released: the half-way transforms' target geometry and the half-way images' grids are memory contents (garbage sizes, voxel sizes and directions, which may change from run to run; a size that cannot be allocated, status 1, or that exhausts the memory); larmorx uses the registration's target (the destination as the registration resampled it, at the last resolution it registered), in both modes";
    FS_1076 = "FS-1076", "FreeSurfer", true,
        "mri_robust_register's test mode (--test) on an image of 16 to 31 voxels along its smallest axis reads past the end of its one-level image pyramid (it takes the level it dropped, smaller than 16 voxels); by default an error, status 1";
    FS_1077 = "FS-1077", "FreeSurfer", true,
        "mri_robust_register's test mode (--test) on an image that does not read prints that it was read successfully, then dies of a segmentation fault; an error in both modes";
    FS_1078 = "FS-1078", "FreeSurfer", true,
        "mri_robust_register's test modes 5 and 6 (--test 5, --test 6) write their pattern of integers as bytes into an image of floating-point numbers, so the bytes of the first voxels of each row change instead of the pattern's voxels; by default the pattern's values are written as voxel values";
    FS_1079 = "FS-1079", "FreeSurfer", true,
        "mri_robust_register --debug with a half-way image (--halfmov, --halfdst) after a registration with a cost other than ROB and ROBENT writes the image, then dies of a segmentation fault where it would print the image's means over the registration's weights (that registration has no weights); by default the means are not printed";
    FS_1080 = "FS-1080", "FreeSurfer", true,
        "mri_robust_template with an option that takes an argument or a list given last dies of a segmentation fault (--template aborts instead); by default an error naming the option";
    FS_1081 = "FS-1081", "FreeSurfer", true,
        "mri_robust_template runs the inputs' registrations on several threads when it may: its printed lines and the sums of the inputs' centres then depend on the run; larmorx always gives the one-thread order";
    FS_1082 = "FS-1082", "FreeSurfer", true,
        "mri_robust_template's seed from the images' centres (no --seed and no --inittp) is converted to an unsigned integer without a range check: a sum of 2^32 or more gives an arbitrary seed; by default the largest unsigned integer";
    FS_1083 = "FS-1083", "FreeSurfer", true,
        "mri_robust_template takes the clock as the random target's seed when the images' centres are all 0 (no --seed and no --inittp), so the target and the printed seed change from run to run; by default the seed stays 0";
    FS_1084 = "FS-1084", "FreeSurfer", true,
        "mri_robust_template with fewer --masks than inputs reads the missing masks' names from memory past the list; larmorx ends with an error at the first input without a mask in both modes";
    FS_1085 = "FS-1085", "FreeSurfer", true,
        "mri_robust_template with one 4-D input and a mask that does not read checks the input instead of the mask and dies of a segmentation fault; by default the error of an input that does not read, naming the mask";
    FS_1086 = "FS-1086", "FreeSurfer", true,
        "mri_robust_template with one 4-D input and more than one mask reads none of them and dies of a segmentation fault on the first frame; by default an error";
    FS_1087 = "FS-1087", "FreeSurfer", true,
        "mri_robust_template with an --ixforms file that does not read dies of a segmentation fault; by default the error 'could not read transform file'";
    FS_1088 = "FS-1088", "FreeSurfer", true,
        "mri_robust_template with a morph (.m3z) as --ixforms reads the morph as a linear transform (memory contents); larmorx ends with an error in both modes";
    FS_1089 = "FS-1089", "FreeSurfer", true,
        "mri_robust_template with one 4-D input and fewer --ixforms than frames applies the missing transforms through no transform and dies of a segmentation fault; by default an error";
    FS_1090 = "FS-1090", "FreeSurfer", true,
        "mri_robust_template --mapmov with one name writes the mapped inputs as one 4-D image whose header is a default one (1 mm voxels, a default orientation and centre), not the template's; by default the template's header";
    FS_1091 = "FS-1091", "FreeSurfer", false,
        "mri_robust_template --conform does not write a conformed template (256^3, 1 mm): it prints 'not implemented, yet' for each input and writes the template resampled trilinearly in its own space; kept, noted";
    FS_1092 = "FS-1092", "FreeSurfer", true,
        "mri_robust_template --inittp with a number larger than the number of inputs reads past its list of images (memory contents); larmorx ends with an error in both modes";
    FS_1093 = "FS-1093", "FreeSurfer", true,
        "mri_robust_template --inittp 0 without --ixforms, with --noit or with two inputs, maps the inputs with no transform and dies of a segmentation fault; by default an error";
    FS_1094 = "FS-1094", "FreeSurfer", true,
        "mri_robust_template --weights with --leastsquares and the global iterations stores each input's weights past the end of a list it never sized (heap memory); larmorx keeps no weights in both modes";
    FS_1095 = "FS-1095", "FreeSurfer", true,
        "mri_robust_template --average 3 builds a matrix of (inputs x voxels) rows and voxels columns, which no image of a useful size fits in memory; larmorx ends with an error above 2,048 voxels in both modes";
    FS_1096 = "FS-1096", "FreeSurfer", true,
        "mri_robust_template ends with status 0 when the template or one of its outputs (transforms, intensity factors, mapped inputs, weights) could not be written; by default status 1 after the program's messages";
    FS_1097 = "FS-1097", "FreeSurfer", false,
        "mri_robust_template with one 4-D input and no --inittp draws its random initial target among one name, so it always starts from frame 1; kept, noted";
    FS_1098 = "FS-1098", "FreeSurfer", true,
        "mri_robust_template --debug writes its debug files into the template's folder name cut two bytes short (out/t.mgz: into the working folder with the prefix 'ou'); by default into the template's folder";
    FS_1099 = "FS-1099", "FreeSurfer", true,
        "the matrix logarithm of mri_robust_template's affine mean space (--affine without --fixtp), of a transform with an eigenvalue on the nonpositive real axis (a reflection, or a zero eigenvalue): FreeSurfer prints that the principal logarithm is not defined and goes on to a logarithm that is not the principal one, which nearly always ends in its 'Result too imaginary to ignore!' error; refused with the message and status 1 by default";
    FS_1100 = "FS-1100", "FreeSurfer", true,
        "the matrix logarithm of mri_robust_template's affine mean space, of a transform whose eigenvalues fall into separate groups (more than 0.1 apart) one of which holds two or more eigenvalues: the triangular Sylvester equation between two groups leaves one term out of every column after the first, so the logarithm, and the mean space, are wrong; computed correctly by default";
    FS_1311 = "FS-1311", "FreeSurfer", true,
        "the matrix exponential of mri_robust_template's affine mean space (--affine without --fixtp) takes its Pade approximant's coefficients from memory it no longer owns (uninitialized stack memory in the official build): its result is NaN or arbitrary, and so is the mean space (memory contents); the coefficients are Higham's in both modes, noted";
    FS_1312 = "FS-1312", "FreeSurfer", true,
        "the matrix logarithm of mri_robust_template's affine mean space ends the program ('Result too imaginary to ignore!') when the imaginary part of its complex intermediate result passes 10 n 2^-52 times the result's one-norm, a bound below the rounding of its own complex arithmetic: about one ordinary near-identity transform in twenty is refused; by default refused only above sqrt(2^-52) times the norm";
}

/// The register entry with ID `id`.
pub fn lookup(id: &str) -> Option<&'static Defect> {
    DEFECTS.iter().copied().find(|d| d.id == id)
}

/// A defect that changed a result ([`UpstreamDefects::Fix`]: larmorx corrected it), or would
/// have ([`UpstreamDefects::Reproduce`]: larmorx reproduced the original).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub defect: &'static Defect,
    pub mode: UpstreamDefects,
    /// What it did to this result, in a few words.
    pub detail: String,
}

impl Notice {
    pub fn new(
        defect: &'static Defect,
        mode: UpstreamDefects,
        detail: impl Into<String>,
    ) -> Notice {
        Notice {
            defect,
            mode,
            detail: detail.into(),
        }
    }

    /// `corrected` or `reproduced`.
    pub fn verb(&self) -> &'static str {
        match self.mode {
            UpstreamDefects::Fix => "corrected",
            UpstreamDefects::Reproduce => "reproduced",
        }
    }

    /// `<ID> (<tool>) corrected: <detail>` (or `reproduced`).
    pub fn message(&self) -> String {
        format!(
            "{} ({}) {}: {}",
            self.defect.id,
            self.defect.tool,
            self.verb(),
            self.detail
        )
    }

    /// The line the command lines print on standard error:
    /// `upstream defect <ID> (<tool>) corrected: <detail> [<ENV>=reproduce gives <tool>'s
    /// result]`. The Python wrappers recognise it in a replica's output (`larmorx._defects`).
    pub fn line(&self) -> String {
        let hint = match self.mode {
            UpstreamDefects::Fix => format!("{ENV}=reproduce gives {}'s result", self.defect.tool),
            UpstreamDefects::Reproduce => format!("{ENV}=fix corrects it"),
        };
        format!("upstream defect {} [{hint}]", self.message())
    }
}

impl fmt::Display for Notice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

/// FSL-11's notice for a run of the FSL program `tool` with `FSLOUTPUTTYPE` unset whose images
/// were written as `NIFTI_GZ` (the default; with `reproduce` the run does what FSL's does). The
/// one text both implementations print: the clean-room originals' (`larmorx_mri::hmc::image::
/// fsl11_notice` re-exports it) and the `larmorx-nc` replicas'. It says what FSL's `tool` does
/// with the variable unset ([`fsl11_what_fsl_does`]).
pub fn fsl11_notice(tool: &str) -> Notice {
    Notice::new(
        &FSL_11,
        UpstreamDefects::Fix,
        format!(
            "FSLOUTPUTTYPE is not set: the images are written as NIFTI_GZ (.nii.gz); {}",
            fsl11_what_fsl_does(tool)
        ),
    )
}

/// What FSL 6.0.7.23's `tool` does when `FSLOUTPUTTYPE` is not set (observed, black box:
/// `validation/investigations/easy-cli/fsl11_probe.py`): its programs stop with an error at
/// their first image (exit 1); `standard_space_roi`, a script, goes on after its steps stop
/// there and exits 0 without writing an image.
pub fn fsl11_what_fsl_does(tool: &str) -> String {
    match tool {
        "standard_space_roi" => "FSL's standard_space_roi writes no image (its steps stop with \
                                 an error at their first image) and exits 0"
            .to_owned(),
        _ => format!("FSL's {tool} stops with an error at its first image (exit 1)"),
    }
}

/// Adds `notice` to `notices` unless the same defect is already there (one notice per defect
/// and result).
pub fn push_once(notices: &mut Vec<Notice>, notice: Notice) {
    if !notices.iter().any(|n| n.defect.id == notice.defect.id) {
        notices.push(notice);
    }
}

/// The mode of one call and the notices it collects: what a tool with many code paths (an
/// ANTs program reading, converting and writing several images) passes along.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notices {
    mode: UpstreamDefects,
    list: Vec<Notice>,
}

impl Notices {
    pub fn new(mode: UpstreamDefects) -> Notices {
        Notices {
            mode,
            list: Vec::new(),
        }
    }

    /// The mode.
    pub fn mode(&self) -> UpstreamDefects {
        self.mode
    }

    /// Whether defects are corrected.
    pub fn fix(&self) -> bool {
        self.mode.fix()
    }

    /// Whether defects are reproduced.
    pub fn reproduce(&self) -> bool {
        self.mode.reproduce()
    }

    /// Notes that `defect` changed this result (or, reproduced, would have): one notice per
    /// defect, the first detail kept ([`push_once`]).
    pub fn add(&mut self, defect: &'static Defect, detail: impl Into<String>) {
        push_once(&mut self.list, Notice::new(defect, self.mode, detail));
    }

    /// Adds notices collected elsewhere (one per defect).
    pub fn extend(&mut self, notices: impl IntoIterator<Item = Notice>) {
        for n in notices {
            push_once(&mut self.list, n);
        }
    }

    /// The notices, in the order they were added.
    pub fn as_slice(&self) -> &[Notice] {
        &self.list
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Whether `defect` was noted.
    pub fn contains(&self, defect: &Defect) -> bool {
        self.list.iter().any(|n| n.defect.id == defect.id)
    }

    pub fn into_vec(self) -> Vec<Notice> {
        self.list
    }

    /// Prints each notice as the command lines do: `larmorx: <Notice::line>`.
    pub fn print(&self, err: &mut dyn std::io::Write) {
        for n in &self.list {
            let _ = writeln!(err, "larmorx: {}", n.line());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn modes() {
        for m in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            assert_eq!(UpstreamDefects::parse(m.name()), Some(m));
        }
        assert_eq!(UpstreamDefects::default(), UpstreamDefects::Fix);
        assert_eq!(UpstreamDefects::parse("Fix"), None);
        assert_eq!(UpstreamDefects::from_value(None), Ok(UpstreamDefects::Fix));
        assert_eq!(
            UpstreamDefects::from_value(Some(OsStr::new(""))),
            Ok(UpstreamDefects::Fix)
        );
        assert_eq!(
            UpstreamDefects::from_value(Some(OsStr::new("reproduce"))),
            Ok(UpstreamDefects::Reproduce)
        );
        let e = UpstreamDefects::from_value(Some(OsStr::new("faithful"))).unwrap_err();
        assert!(e.to_string().contains("LARMORX_UPSTREAM_DEFECTS"), "{e}");
    }

    #[test]
    fn the_registry_has_unique_well_formed_ids() {
        for (i, d) in DEFECTS.iter().enumerate() {
            let (family, number) = d.id.split_once('-').unwrap();
            assert!(family.chars().all(|c| c.is_ascii_uppercase()), "{}", d.id);
            // Two digits, three once a family passes 99 (ANTs, with Atropos),
            // four once it passes 999 (FreeSurfer's FS-1001 onwards).
            assert!((2..=4).contains(&number.len()), "{}", d.id);
            assert!(number.chars().all(|c| c.is_ascii_digit()), "{}", d.id);
            assert!(DEFECTS[..i].iter().all(|e| e.id != d.id), "{}", d.id);
            assert!(!d.what.ends_with('.'), "{}", d.id);
            assert_eq!(lookup(d.id), Some(*d));
        }
        assert!(lookup("NONE-01").is_none());
    }

    #[test]
    fn notice_lines() {
        let n = Notice::new(
            &FSL_01,
            UpstreamDefects::Fix,
            "volume 9 starts from volume 8",
        );
        assert_eq!(
            n.line(),
            "upstream defect FSL-01 (mcflirt) corrected: volume 9 starts from volume 8 \
             [LARMORX_UPSTREAM_DEFECTS=reproduce gives mcflirt's result]"
        );
        let r = Notice::new(&FSL_01, UpstreamDefects::Reproduce, "x");
        assert!(
            r.line()
                .ends_with("[LARMORX_UPSTREAM_DEFECTS=fix corrects it]")
        );
        let mut all = vec![n.clone()];
        push_once(&mut all, r);
        assert_eq!(all, vec![n]);
    }

    #[test]
    fn collected_notices() {
        let mut notices = Notices::new(UpstreamDefects::Reproduce);
        assert!(notices.reproduce() && !notices.fix() && notices.is_empty());
        notices.add(&ANTS_08, "first");
        notices.add(&ANTS_08, "second");
        notices.add(&ITK_02, "x");
        assert_eq!(notices.as_slice().len(), 2);
        assert_eq!(notices.as_slice()[0].detail, "first");
        assert_eq!(notices.as_slice()[0].mode, UpstreamDefects::Reproduce);
        assert!(notices.contains(&ITK_02) && !notices.contains(&ANTS_09));
        let mut printed = Vec::new();
        notices.print(&mut printed);
        let text = String::from_utf8(printed).unwrap();
        assert!(
            text.starts_with("larmorx: upstream defect ANTS-08 (ImageMath) reproduced: first"),
            "{text}"
        );
        assert_eq!(text.lines().count(), 2);
    }

    #[test]
    fn fsl11_says_what_fsl_does() {
        // FSL's programs stop at their first image; standard_space_roi's script goes on
        assert_eq!(
            fsl11_notice("fslmaths").line(),
            "upstream defect FSL-11 (FSL) corrected: FSLOUTPUTTYPE is not set: the images are \
             written as NIFTI_GZ (.nii.gz); FSL's fslmaths stops with an error at its first image \
             (exit 1) [LARMORX_UPSTREAM_DEFECTS=reproduce gives FSL's result]"
        );
        assert_eq!(
            fsl11_notice("standard_space_roi").line(),
            "upstream defect FSL-11 (FSL) corrected: FSLOUTPUTTYPE is not set: the images are \
             written as NIFTI_GZ (.nii.gz); FSL's standard_space_roi writes no image (its steps \
             stop with an error at their first image) and exits 0 \
             [LARMORX_UPSTREAM_DEFECTS=reproduce gives FSL's result]"
        );
    }
}
