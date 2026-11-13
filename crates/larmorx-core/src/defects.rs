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
        "the mathematical functions FSL's and AFNI's programs call are not correctly rounded: the double-precision sin, cos, tan, asin, acos, atan, exp, log and pow (on image intensities, between 0.02 % (log) and 0.21 % (tan) of the arguments get a result one unit in the last place from the exact value) and the single-precision sinf, cosf, atan2f, expf, logf and powf; so fslmaths' -sin, -cos, -tan, -asin, -acos, -atan, -exp, -log and -pow of a double image (-dt double) differ in that last place on those voxels (a float image, the default, keeps the same values), its kernels, -bptf, -tfce and -roc, fslstats' -e and -E and 3dTshift's -wsinc5 and -wsinc9 weights (about 1 % of them) can take another last bit, and the iterative programs (FLIRT and its programs, mcflirt's mutual-information costs, FAST, topup's smoothing, bet2's powers, betsurf's blurring) another path; nor is sincos, which gives the twiddle factors of fslmaths' FFT convolution (through cexp): of the transform lengths 1 to 256, 80 hold a factor one unit in the last place from the exact one, so a few voxels of such a convolution differ in their last place; and numpy's float64 exp and log are the C library's on x86-64 CPUs without AVX-512 (and SciPy's powers C's pow), which tedana's t2smap takes for its S0, its curve fit's model, the T2* floor, the RMSE and the combination's weights; and FreeSurfer's matrices and histograms call cosf and sinf (rotations), expf (Gaussian matrices), exp, pow and log (the histograms' smoothing, robust fit and entropy) and log (Gaussian draws)";
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
        "MatrixAlloc of a negative size, or of more numbers than an int holds, crashes or corrupts memory after printing that it uses an mmap'd file, or exits";
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
        "MatrixSVDPseudoInverse never writes a given output (it returns a new matrix), returns a pseudo-inverse of infinities when the decomposition fails, and crashes for a zero matrix with fewer rows than columns";
    FS_47 = "FS-47", "FreeSurfer", true,
        "MatrixEigenSystem of a symmetric matrix with infinite entries, and MatrixCalculateEigenSystemHelper's symmetric case of a matrix with infinite or NaN entries in its upper triangle, never return on many such matrices (VNL's ITK-115), and on the others return the values VNL computes, though such a matrix has no eigensystem";
    FS_48 = "FS-48", "FreeSurfer", true,
        "the SVD functions (MatrixSVD, the SVD inverses and pseudo-inverses, the condition numbers) never return for some matrices with infinite or NaN entries";
    FS_49 = "FS-49", "FreeSurfer", true,
        "freadInt and freadDouble at the end of a file keep, for the bytes they could not read, bytes of the number read before them (memory contents where none was read; freadInt prints nothing), and MatrixReadFrom of a file that ends early fills the matrix's type, sizes or elements with them";
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
        "reading or writing a voxel of a volume made without voxels (MRIallocHeader, a header read alone) crashes";
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
        "MRISreadVTK and the ASCII and movie.byu surface readers count a vertex's faces in one byte: a vertex of more than 255 faces gets its count modulo 256";
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
    FS_167 = "FS-167", "FreeSurfer", true,
        "the binary readers of one number (fread1, fread2, freadLong, freadFloat, and znzread1, znzreadShort, znzreadInt, znzreadLong, znzreadFloat, znzreadDouble) at the end of a file return a number whose missing bytes are memory contents";
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
        "the surface library's file-type test and the curvature and annotation name builders, given a name (or a surface name) whose folder or last part is 1,024 bytes or more, write past a 1,024-byte buffer (the program can end with '*** stack smashing detected ***' or go on with corrupted names); the surface reader (MRISread), looking for the subject's Talairach transform beside a surface name of 300 bytes or more, writes past a 300-byte buffer";
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
    FS_285 = "FS-285", "FreeSurfer", true,
        "reading a volume-geometry block (the text block of linear transform and surface files) whose line starts with a word of 64 bytes or more writes the word past its buffer: memory corruption, a crash or wrong values";
    FS_286 = "FS-286", "FreeSurfer", true,
        "reading a volume-geometry block whose line has a second word of 2 bytes or more (not the usual =) writes the word past its 2-byte buffer: memory corruption, a crash or wrong values";
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
        "MRIscalarMulFrame stores a NaN product into a byte volume by C's conversion, which is undefined for NaN";
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
        "reading a transform whose file name has 500 bytes or more overflows the buffer the name is classified in (memory corruption)";
    FS_307 = "FS-307", "FreeSurfer", true,
        "reading a MINC transform (.xfm) whose last line starts with % after a second % comment line never returns";
    FS_308 = "FS-308", "FreeSurfer", true,
        "reading an .lta whose first two lines are not type = <n> and nxforms = <n> takes the type or the number of transforms from memory contents";
    FS_309 = "FS-309", "FreeSurfer", true,
        "reading an .lta of a single line crashes";
    FS_310 = "FS-310", "FreeSurfer", true,
        "reading a MINC transform (.xfm) whose %Generated by comment ends with a word containing src or dst crashes";
    FS_302 = "FS-302", "FreeSurfer", true,
        "the sinc samplers with a half-width whose window holds more than 128 voxels on an axis write past their weight arrays: memory corruption or a crash";
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
            // Two digits, three once a family passes 99 (ANTs, with Atropos).
            assert!((2..=3).contains(&number.len()), "{}", d.id);
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
