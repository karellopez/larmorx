// SPDX-License-Identifier: Apache-2.0
//! `ImageMath` with its original arguments (ANTs v2.6.5, `Examples/ImageMath.cxx`,
//! `ImageMathHelper{2D,3D,4D}.cxx` and the dispatch tables at the end of
//! `ImageMath_Templates.hxx`).
//!
//! ```text
//! ImageMath <dimension> <output> <operation> <operands...>
//! ```
//!
//! # How ANTs dispatches, and what larmorx reproduces
//!
//! - Fewer than four arguments: the usage on stdout, exit 0 for `--help`/`-h` as the first
//!   argument, else exit 1.
//! - The dimension is read with `std::stoi` (no number aborts ANTs; larmorx exits 1). Only
//!   2, 3 and 4 exist: " Dimension <d> is not supported ", exit 1.
//! - The operation name is matched exactly (case-sensitive) against the tables for that
//!   dimension. An unknown name prints " Operation <op> not found or not supported for
//!   dimension <d>" and exits 1.
//! - **Every known operation exits 0**, whatever happened inside it (the tables discard the
//!   operation's return value; `Mattes` and `ComponentToVector` are the exceptions). An
//!   operation that prints an error and returns therefore still exits 0, and larmorx does
//!   the same ([`OpError::Reported`]). Where ANTs crashes instead (a missing input file
//!   dereferences a null image; `throw` without a handler aborts), larmorx prints a message
//!   and exits 1 ([`OpError::Crash`]).
//!
//! # How to add an ImageMath operation group
//!
//! 1. Port the operations' Rust API (on voxel slices or `AntsImage`s) in
//!    `crates/larmorx-ants/src/image_math/<group>.rs` (or in `larmorx-image` for ITK
//!    filters), with unit tests.
//! 2. Write `crates/larmorx-ants/src/cli/image_math/<group>.rs` with one `fn(&mut Context)`
//!    per operation, reproducing the ANTs function's argument handling (`ctx.arg(i)` is
//!    `argv[i]`, numbered as in `ImageMath_Templates.hxx`: 2 is the output, 3 the operation,
//!    4 the first operand), and a table
//!    `pub const OPERATIONS: &[Operation] = &[Operation { name, dims, usage, run }, ...]`.
//!    Read images with [`Context::read`] (`ReadImage` semantics) and write them with
//!    [`Context::write`] (`ANTs::WriteImage`).
//! 3. Register the group here: `mod <group>;` and `<group>::OPERATIONS` in [`GROUPS`].
//! 4. Add parity cases in `validation/src/larmorx_validation/parity/ants_image_math/` and a
//!    Python wrapper (see `python/larmorx/ants/`).
//!
//! An operation listed in [`KNOWN`] but in no group answers "not supported yet".

mod arithmetic;
mod gaussian;
mod intensity;

use std::io::Write;

use crate::image::{AntsImage, ImageStore, OutputImage, Pixel, ReadError, read_with, write_with};

use super::cstd::stoi;

/// One ImageMath operation.
pub struct Operation {
    /// The name on the command line (`argv[3]`), matched exactly.
    pub name: &'static str,
    /// The dimensions larmorx runs it for (ANTs' tables may allow more).
    pub dims: &'static [usize],
    /// Its usage line, as in ANTs' help.
    pub usage: &'static str,
    /// Runs it.
    pub run: fn(&mut Context<'_>) -> Result<(), OpError>,
}

/// The operation groups larmorx implements. Add new groups here.
pub const GROUPS: &[&[Operation]] = &[
    arithmetic::OPERATIONS,
    intensity::OPERATIONS,
    gaussian::OPERATIONS,
];

/// Every operation name in ANTs v2.6.5's dispatch tables, with the dimensions each table
/// serves (`ImageMathHelperAll`: 2, 3, 4; `2DOnly`; `2DOr3D`; `3DOr4D`; `3DOnly`; `4DOnly`).
pub const KNOWN: &[(&[usize], &[&str])] = &[
    (
        &[2, 3, 4],
        &[
            "m",
            "mresample",
            "+",
            "-",
            "vm",
            "vmresample",
            "v+",
            "v-",
            "/",
            "^",
            "exp",
            "max",
            "abs",
            "addtozero",
            "overadd",
            "total",
            "vtotal",
            "mean",
            "Decision",
            "Neg",
            "G",
            "Convolve",
            "PeronaMalik",
            "InPaint",
            "MD",
            "ME",
            "MO",
            "MC",
            "GD",
            "GE",
            "GO",
            "GC",
            "D",
            "MaurerDistance",
            "Normalize",
            "Grad",
            "Laplacian",
            "Canny",
            "LabelSurfaceArea",
            "PH",
            "CenterImage2inImage1",
            "Byte",
            "ReflectionMatrix",
            "MakeAffineTransform",
            "ClosestSimplifiedHeaderMatrix",
            "LabelStats",
            "ROIStatistics",
            "LabelThickness",
            "DiceAndMinDistSum",
            "Lipschitz",
            "InvId",
            "ShiftImageSlicesInTime",
            "ReplicateImage",
            "ReplicateDisplacement",
            "GetLargestComponent",
            "ExtractVectorComponent",
            "ThresholdAtMean",
            "SetTimeSpacing",
            "SetTimeSpacingWarp",
            "FlattenImage",
            "CorruptImage",
            "Where",
            "Finite",
            "FillHoles",
            "HistogramMatch",
            "RescaleImage",
            "WindowImage",
            "NeighborhoodStats",
            "PadImage",
            "SigmoidImage",
            "CoordinateComponentImages",
            "Sharpen",
            "UnsharpMask",
            "MakeImage",
            "stack",
            "stack2",
            "CompareHeadersAndImages",
            "CountVoxelDifference",
            "RemoveLabelInterfaces",
            "ReplaceVoxelValue",
            "PoissonDiffusion",
            "EnumerateLabelInterfaces",
            "ConvertImageToFile",
            "PValueImage",
            "CorrelationUpdate",
            "ConvertImageSetToMatrix",
            "RandomlySampleImageSetToCSV",
            "ConvertImageSetToEigenvectors",
            "ConvertVectorToImage",
            "ExtractContours",
            "PropagateLabelsThroughMask",
            "FastMarchingExtension",
            "FastMarchingSegmentation",
            "TruncateImageIntensity",
            "ExtractSlice",
            "ClusterThresholdVariate",
            "MajorityVoting",
            "MostLikely",
            "CorrelationVoting",
            "PearsonCorrelation",
            "Translate",
            "NeighborhoodCorrelation",
            "NormalizedCorrelation",
            "Demons",
            "Mattes",
            "MinMaxMean",
            "PureTissueN4WeightMask",
            "BlobDetector",
            "MatchBlobs",
            "Project",
            "ComponentToVector",
        ],
    ),
    (
        &[2],
        &["TileImages", "TimeSeriesRegionCorr", "TimeSeriesRegionSCCA"],
    ),
    (
        &[2, 3],
        &[
            "AverageLabels",
            "Check3TissueLabeling",
            "oldPropagateLabelsThroughMask",
            "SetOrGetPixel",
            "STAPLE",
            "KinematicTensor",
        ],
    ),
    (
        &[3, 4],
        &[
            "ConvertLandmarkFile",
            "PASLQuantifyCBF",
            "PASL",
            "pCASL",
            "TriPlanarView",
        ],
    ),
    (
        &[3],
        &[
            "4DTensorTo3DTensor",
            "ComponentTo3DTensor",
            "FuseNImagesIntoNDVectorField",
            "ExtractComponentFrom3DTensor",
            "FSLTensorToITK",
            "ITKTensorToFSL",
            "MTR",
            "SmoothTensorImage",
            "TensorAxialDiffusion",
            "TensorColor",
            "TensorEigenvalue",
            "TensorFA",
            "TensorFANumerator",
            "TensorFADenominator",
            "TensorIOTest",
            "TensorMask",
            "TensorMeanDiffusion",
            "TensorRadialDiffusion",
            "TensorToLocalSpace",
            "TensorToPhysicalSpace",
            "TensorToVectorComponent",
            "TensorToVector",
            "ValidTensor",
        ],
    ),
    (
        &[4],
        &[
            "AverageOverDimension",
            "CompCorrAuto",
            "ComputeTimeSeriesLeverage",
            "nvols",
            "PCASLQuantifyCBF",
            "SliceTimingCorrection",
            "SplitAlternatingTimeSeries",
            "ThreeTissueConfounds",
            "TimeSeriesMask",
            "TimeSeriesAssemble",
            "TimeSeriesDisassemble",
            "TimeSeriesInterpolationSubtraction",
            "TimeSeriesSimpleSubtraction",
            "TimeSeriesSubset",
            "TimeSeriesToMatrix",
        ],
    ),
];

/// Whether ANTs v2.6.5 knows `op` for `dim`.
pub fn ants_knows(op: &str, dim: usize) -> bool {
    KNOWN
        .iter()
        .any(|(dims, ops)| dims.contains(&dim) && ops.contains(&op))
}

/// The larmorx operation named `op`, if it is implemented.
pub fn find(op: &str) -> Option<&'static Operation> {
    GROUPS.iter().flat_map(|g| g.iter()).find(|o| o.name == op)
}

/// The names of the implemented operations.
pub fn implemented() -> Vec<&'static str> {
    GROUPS
        .iter()
        .flat_map(|g| g.iter())
        .map(|o| o.name)
        .collect()
}

/// How an operation ended other than normally.
#[derive(Debug)]
pub enum OpError {
    /// ANTs prints this (to stdout or stderr as `to_stderr` says) and returns; ImageMath
    /// still exits 0.
    Reported { message: String, to_stderr: bool },
    /// ANTs crashes here (segmentation fault or abort); larmorx prints this and exits 1.
    Crash(String),
    /// ANTs does this, larmorx does not yet; exit 1.
    Unsupported(String),
}

impl OpError {
    pub fn crash(message: impl Into<String>) -> Self {
        OpError::Crash(message.into())
    }
    pub fn unsupported(message: impl Into<String>) -> Self {
        OpError::Unsupported(message.into())
    }
}

impl From<larmorx_image::FilterError> for OpError {
    fn from(e: larmorx_image::FilterError) -> Self {
        OpError::Crash(e.to_string())
    }
}

/// What an operation sees: the arguments, the image store, the output streams.
pub struct Context<'a> {
    /// `argv` as ANTs numbers it: `[ImageMath, dim, output, op, operands...]`.
    pub argv: Vec<String>,
    /// The dimension (`argv[1]`).
    pub dim: usize,
    /// Worker threads (`ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` on the command line).
    pub n_threads: usize,
    pub store: &'a mut dyn ImageStore,
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
}

impl Context<'_> {
    /// `argc`.
    pub fn argc(&self) -> usize {
        self.argv.len()
    }

    /// `argv[i]`, if given.
    pub fn arg(&self, i: usize) -> Option<&str> {
        self.argv.get(i).map(String::as_str)
    }

    /// `argv[i]`, or a crash message: ANTs reads past `argc` there (undefined behaviour).
    pub fn required(&self, i: usize) -> Result<&str, OpError> {
        self.arg(i).ok_or_else(|| {
            OpError::crash(format!(
                "larmorx: ImageMath {} needs at least {} operand(s) (ANTs reads past the arguments here)",
                self.op(),
                i - 3
            ))
        })
    }

    /// The output name (`argv[2]`).
    pub fn output(&self) -> &str {
        &self.argv[2]
    }

    /// The operation (`argv[3]`).
    pub fn op(&self) -> &str {
        &self.argv[3]
    }

    /// ANTs' `ReadImage<itk::Image<T, dim>>`: prints ANTs' message on failure (none for
    /// names shorter than 3 characters) and returns `None`, as `ReadImage` leaves a null
    /// image. Errors larmorx cannot read past (unsupported files) end the operation.
    pub fn try_read<T: Pixel>(&mut self, name: &str) -> Result<Option<AntsImage<T>>, OpError> {
        match read_with::<T, _>(self.store, name, self.dim, self.n_threads) {
            Ok(image) => Ok(Some(image)),
            Err(ReadError::Unsupported { name, message }) => {
                Err(OpError::unsupported(format!("larmorx: {name}: {message}")))
            }
            Err(e) => {
                if let Some(m) = e.ants_message() {
                    let _ = writeln!(self.err, "{m}");
                }
                Ok(None)
            }
        }
    }

    /// [`Context::try_read`] for an image the operation then uses unconditionally: where
    /// ANTs would dereference the null image, larmorx stops with exit 1.
    pub fn read<T: Pixel>(&mut self, name: &str) -> Result<AntsImage<T>, OpError> {
        self.try_read(name)?.ok_or_else(|| {
            OpError::crash(format!(
                "larmorx: cannot read image '{name}' (ANTs crashes here: it uses the null image)"
            ))
        })
    }

    /// `ANTs::WriteImage(image, name)`: nothing for names shorter than 3 characters.
    pub fn write(&mut self, name: &str, image: impl Into<OutputImage>) -> Result<bool, OpError> {
        write_with(self.store, name, image.into(), self.n_threads)
            .map_err(|e| OpError::crash(format!("larmorx: cannot write '{name}': {e}")))
    }
}

/// The usage text: ANTs' synopsis and the operations larmorx implements.
pub fn usage() -> String {
    let mut s = String::from(
        "\nUsage: ImageMath ImageDimension <OutputImage.ext> [operations and inputs] <Image1.ext> <Image2.ext>\n\n\
Usage Information \n\
 ImageDimension: 2 or 3 (for 2 or 3 dimensional operations).\n\
 ImageDimension: 4 (for operations on 4D file, e.g. time-series data).\n\
 Operator: See list of valid operators below.\n\
 The last two arguments can be an image or float value \n\
 NB: Some options output text files\n\n\
Operations ported to larmorx (ANTs v2.6.5); the other ANTs operations are not supported yet:\n",
    );
    for group in GROUPS {
        for op in *group {
            s.push_str("  ");
            s.push_str(op.usage);
            s.push('\n');
        }
    }
    s
}

/// Runs `ImageMath` with `args` (the arguments after the program name) on files.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    run(
        args,
        &mut crate::image::FileStore,
        super::itk_threads(),
        out,
        err,
    )
}

/// Runs `ImageMath` with `args` on images from `store`; returns the exit code.
pub fn run(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut argv = vec!["ImageMath".to_owned()];
    argv.extend(args.iter().cloned());
    if argv.len() < 5 {
        let _ = out.write_all(usage().as_bytes());
        let help = matches!(argv.get(1).map(String::as_str), Some("--help" | "-h"));
        return if help { 0 } else { 1 };
    }
    let op = argv[3].clone();
    let Some(dim) = stoi(&argv[1]) else {
        let _ = writeln!(
            err,
            "larmorx: ImageMath: the dimension '{}' is not a number (ANTs aborts: std::stoi throws)",
            argv[1]
        );
        return 1;
    };
    if !(2..=4).contains(&dim) {
        let _ = writeln!(out, " Dimension {dim} is not supported ");
        return 1;
    }
    let dim = dim as usize;
    if !ants_knows(&op, dim) {
        let _ = writeln!(
            out,
            " Operation {op} not found or not supported for dimension {dim}"
        );
        return 1;
    }
    let Some(operation) = find(&op) else {
        let _ = writeln!(
            err,
            "larmorx: ImageMath operation '{op}' is not supported yet (supported: {})",
            implemented().join(" ")
        );
        return 1;
    };
    if !operation.dims.contains(&dim) {
        let _ = writeln!(
            err,
            "larmorx: ImageMath {op} is supported for dimension {} only, not yet for {dim}",
            operation
                .dims
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
        return 1;
    }
    let mut ctx = Context {
        argv,
        dim,
        n_threads,
        store,
        out,
        err,
    };
    match (operation.run)(&mut ctx) {
        Ok(()) => 0,
        Err(OpError::Reported { message, to_stderr }) => {
            let stream: &mut dyn Write = if to_stderr { ctx.err } else { ctx.out };
            let _ = writeln!(stream, "{message}");
            0
        }
        Err(OpError::Crash(message) | OpError::Unsupported(message)) => {
            let _ = writeln!(ctx.err, "{message}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_capture(args: &[&str]) -> (u8, String, String) {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = main(&args, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn dispatch_errors_match_ants() {
        let (code, out, _) = run_capture(&["--help"]);
        assert_eq!(code, 0);
        assert!(out.contains("Usage: ImageMath"));
        assert_eq!(run_capture(&["3", "o.nii", "m"]).0, 1);
        let (code, out, _) = run_capture(&["5", "o.nii", "m", "a.nii"]);
        assert_eq!(
            (code, out.as_str()),
            (1, " Dimension 5 is not supported \n")
        );
        let (code, out, _) = run_capture(&["3", "o.nii", "Bogus", "a.nii"]);
        assert_eq!(code, 1);
        assert_eq!(
            out,
            " Operation Bogus not found or not supported for dimension 3\n"
        );
        // Known to ANTs, not to larmorx yet.
        let (code, _, err) = run_capture(&["3", "o.nii", "Canny", "a.nii"]);
        assert_eq!(code, 1);
        assert!(err.contains("not supported yet"), "{err}");
        let (code, _, err) = run_capture(&["3", "o.nii", "PeronaMalik", "a.nii"]);
        assert_eq!(code, 1);
        assert!(err.contains("not supported yet"), "{err}");
        // A 2D-only operation in 3D is unknown to ANTs.
        let (code, out, _) = run_capture(&["3", "o.nii", "TileImages", "a.nii"]);
        assert_eq!(code, 1);
        assert!(out.contains("not found"));
    }

    #[test]
    fn every_implemented_operation_is_known_to_ants() {
        for name in implemented() {
            let op = find(name).unwrap();
            for &d in op.dims {
                assert!(ants_knows(name, d), "{name} in {d}D");
            }
        }
    }
}
