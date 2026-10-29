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
//! The descriptions of the AFNI and FSL entries describe observable behaviour only, as the
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
    SCI_02 = "SCI-02", "SciPy ndimage", true,
        "NaN and out-of-range values written to an integer output are converted as x86-64 converts them (C leaves the conversion undefined)";
    SCI_03 = "SCI-03", "SciPy ndimage", true,
        "the spline prefilter's reflect boundary (modes reflect, grid-mirror and nearest) reads a coefficient it has already updated, so the splines do not interpolate on short axes";
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
    FSL_13 = "FSL-13", "mcflirt", true,
        "-rmsrel and -rmsabs create the .mat directory without -mats (holding only the reference's identity)";
    FSL_14 = "FSL-14", "mcflirt", true,
        "one NaN or infinite voxel makes the volume's centre of mass, and so its matrix, NaN";
    FSL_21 = "FSL-21", "mcflirt", true,
        "with a histogram cost (corratio, mutualinfo, normmi, woods) and -bins below 8, the first stage's histogram has no bins (-bins divided by the 8 mm scale): the costs are degenerate and the matrices arbitrary, and corratio writes before its arrays and aborts";
    FSL_20 = "FSL-20", "mcflirt", true,
        "when the .mat directory cannot be created for another reason than its permissions (a missing parent directory), mcflirt prints an error and exits with status 0, writing neither the matrices, the .par and .rms files nor the corrected series";
    FSL_22 = "FSL-22", "FSL", true,
        "the background value (calc_bval) sorts the 2-voxel border with std::sort and operator<: with NaN voxels the order is undefined in C++, so the padding value of a resampled volume is arbitrary";
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
        "bet -R passes bet2's options through bash's echo, which takes leading words made of -n, -e and -E as its own options: a leading -e (bet2's mesh) is dropped, and the other options' backslashes are read as escapes";
    FSL_56 = "FSL-56", "FSL", true,
        "an image saved with two or more NIfTI extensions of different sizes gets the wrong vox_offset (NewNifti adds the first extension's size once per extension): the data start inside the extensions or after a gap";
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
            // Two digits, three once a family passes 99 (ANTs, with Atropos).
            assert!((2..=3).contains(&number.len()), "{}", d.id);
            assert!(number.chars().all(|c| c.is_ascii_digit()), "{}", d.id);
            assert!(DEFECTS[..i].iter().all(|e| e.id != d.id), "{}", d.id);
            assert!(!d.what.ends_with('.'), "{}", d.id);
            assert_eq!(lookup(d.id), Some(*d));
        }
        assert!(lookup("FSL-99").is_none());
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
}
