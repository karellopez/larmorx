// SPDX-License-Identifier: Apache-2.0
//! The original ANTs command lines (`larmorx ants <tool> ...`).
//!
//! Each program has a module with `main(args, out, err) -> exit code` (on files) and, for
//! the image programs, `run(args, store, n_threads, out, err)` on any
//! [`ImageStore`](crate::image::ImageStore). Programs that use ANTs' command-line parser
//! (`antsApplyTransforms`) go through [`parser`]; the others read `argv` by position, with
//! the C conversions in [`cstd`]. To add a program: write its module, add it to [`TOOLS`]
//! and to [`run`].

pub mod affine_initializer;
pub mod ai;
pub mod apply_transforms;
pub mod atropos;
pub mod cstd;
pub mod denoise_image;
pub mod image_math;
pub mod menu;
pub mod multiply_images;
pub mod n4_bias_field_correction;
pub mod parser;
pub mod registration;
pub mod resample_image;
pub mod resample_image_by_spacing;
pub mod smooth_image;
pub mod threshold_image;

use std::io::Write;
use std::path::Path;

use larmorx_core::Precision;
use larmorx_core::defects::Notices;
use larmorx_io::itk_transform::TransformFileError;
use larmorx_transform::Transform;

/// Reads the transform files named on a command line.
///
/// [`FileLoader`] reads every format larmorx supports (ITK text, MATLAB, HDF5, NIfTI
/// displacement fields); tests may substitute their own.
pub trait TransformLoader: Sync {
    /// Reads `path` as ITK's reader for the computation type `precision` reads it, with
    /// D13's mode and notices in `notices`.
    fn load_with(
        &self,
        path: &Path,
        precision: Precision,
        notices: &mut Notices,
    ) -> Result<Transform, TransformFileError>;

    /// [`TransformLoader::load_with`] in double precision, defects corrected, notices dropped.
    fn load(&self, path: &Path) -> Result<Transform, TransformFileError> {
        let mut notices = Notices::new(larmorx_core::defects::UpstreamDefects::Fix);
        self.load_with(path, Precision::Double, &mut notices)
    }
}

/// [`TransformLoader`] for the files larmorx reads (`larmorx_io::itk_transform`).
pub struct FileLoader;

impl TransformLoader for FileLoader {
    fn load_with(
        &self,
        path: &Path,
        precision: Precision,
        notices: &mut Notices,
    ) -> Result<Transform, TransformFileError> {
        larmorx_io::itk_transform::read_itk_transform_notices(path, precision, notices)
    }
}

/// The ANTs tools on the command line.
pub const TOOLS: &[&str] = &[
    "antsAffineInitializer",
    "antsAI",
    "antsApplyTransforms",
    "antsRegistration",
    "Atropos",
    "DenoiseImage",
    "ImageMath",
    "MultiplyImages",
    "N4BiasFieldCorrection",
    "ResampleImage",
    "ResampleImageBySpacing",
    "SmoothImage",
    "ThresholdImage",
];

/// Runs ANTs tool `tool` with `args` (the arguments after the tool name). `None` if there is
/// no such tool.
pub fn run(
    tool: &str,
    args: &[String],
    loader: &dyn TransformLoader,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Option<u8> {
    // ITK's output window: the warnings its file IO prints while the program runs, written at
    // the program's end (`larmorx_io::itk_output`).
    let _ = larmorx_io::itk_output::take();
    let code = run_tool(tool, args, loader, out, err);
    for w in larmorx_io::itk_output::take() {
        let _ = err.write_all(w.as_bytes());
    }
    code
}

fn run_tool(
    tool: &str,
    args: &[String],
    loader: &dyn TransformLoader,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Option<u8> {
    match tool {
        "antsAffineInitializer" => Some(affine_initializer::main(args, out, err)),
        "antsAI" => Some(ai::main(args, out, err)),
        "antsApplyTransforms" => Some(apply_transforms::main(args, loader, out, err)),
        "antsRegistration" => Some(registration::main(args, loader, out, err)),
        "Atropos" => Some(atropos::main(args, out, err)),
        "DenoiseImage" => Some(denoise_image::main(args, out, err)),
        "ImageMath" => Some(image_math::main(args, out, err)),
        "MultiplyImages" => Some(multiply_images::main(args, out, err)),
        "N4BiasFieldCorrection" => Some(n4_bias_field_correction::main(args, out, err)),
        "ResampleImage" => Some(resample_image::main(args, out, err)),
        "ResampleImageBySpacing" => Some(resample_image_by_spacing::main(args, out, err)),
        "SmoothImage" => Some(smooth_image::main(args, out, err)),
        "ThresholdImage" => Some(threshold_image::main(args, out, err)),
        _ => None,
    }
}

/// The number of threads ANTs would use, read from the environment as ITK reads it; 0 means
/// all logical CPUs. See [`itk_threads_from`].
pub(crate) fn itk_threads() -> usize {
    itk_threads_from(|name| std::env::var(name).ok())
}

/// ITK's default thread count from environment variables (`var` looks one up), as
/// `MultiThreaderBase::GetGlobalDefaultNumberOfThreads` reads them
/// (`itkMultiThreaderBase.cxx:266-330`, ITK v5.4.5): the variables named in
/// `ITK_NUMBER_OF_THREADS_ENV_LIST` (separated by `:`), or `NSLOTS` when that list is not set,
/// and always `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` last; each one that is set replaces the
/// count with its value read by C's `atoi`, so the last one set wins. 0 (none set, or a value
/// that is not a positive number) means all logical CPUs.
///
/// nipype passes a node's `num_threads` to ANTs as `NSLOTS` (fMRIPrep's working directories
/// show `NSLOTS=8` for DenoiseImage and the anatomical N4s). larmorx's results do not depend on
/// the count, so this only decides how many CPUs it uses. ITK takes a negative value as a huge
/// unsigned one and so uses its maximum; larmorx then uses all logical CPUs.
pub(crate) fn itk_threads_from(var: impl Fn(&str) -> Option<String>) -> usize {
    let list = match var("ITK_NUMBER_OF_THREADS_ENV_LIST") {
        Some(names) => format!("{names}:ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"),
        None => "NSLOTS:ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS".to_string(),
    };
    let mut threads = 0;
    for name in list.split(':').filter(|n| !n.is_empty()) {
        if let Some(value) = var(name) {
            // atoi: leading digits, 0 when there are none
            threads = cstd::stoi(&value).map_or(0, |v| usize::try_from(v).unwrap_or(0));
        }
    }
    threads
}

#[cfg(test)]
mod tests {
    use super::itk_threads_from;

    fn threads(vars: &[(&str, &str)]) -> usize {
        itk_threads_from(|name| {
            vars.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| (*v).to_string())
        })
    }

    #[test]
    fn itk_thread_variables() {
        assert_eq!(threads(&[]), 0);
        // nipype's num_threads
        assert_eq!(threads(&[("NSLOTS", "8")]), 8);
        assert_eq!(threads(&[("ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS", "3")]), 3);
        // the last one set wins, even when empty or not a number
        assert_eq!(
            threads(&[
                ("NSLOTS", "8"),
                ("ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS", "2")
            ]),
            2
        );
        assert_eq!(
            threads(&[
                ("NSLOTS", "8"),
                ("ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS", "")
            ]),
            0
        );
        // atoi
        assert_eq!(threads(&[("NSLOTS", " 6x")]), 6);
        assert_eq!(threads(&[("NSLOTS", "-4")]), 0);
        // a list replaces NSLOTS
        assert_eq!(
            threads(&[
                ("ITK_NUMBER_OF_THREADS_ENV_LIST", "PBS_NP:MY_THREADS"),
                ("NSLOTS", "8"),
                ("MY_THREADS", "5"),
            ]),
            5
        );
        assert_eq!(
            threads(&[
                ("ITK_NUMBER_OF_THREADS_ENV_LIST", "MY_THREADS"),
                ("MY_THREADS", "5"),
                ("ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS", "7"),
            ]),
            7
        );
    }
}
