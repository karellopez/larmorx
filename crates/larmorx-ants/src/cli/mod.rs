//! The original ANTs command lines (`larmorx ants <tool> ...`).

pub mod apply_transforms;
pub mod parser;

use std::io::Write;
use std::path::Path;

use larmorx_transform::Transform;

/// Reads the transform files named on a command line.
///
/// The standalone binary reads ITK text, MATLAB and NIfTI files ([`FileLoader`]); the Python
/// entry point also reads HDF5 (`.h5`) files, through h5py.
pub trait TransformLoader: Sync {
    fn load(&self, path: &Path) -> Result<Transform, String>;
}

/// [`TransformLoader`] for the formats the Rust reader supports (`.txt`, `.tfm`, `.mat`,
/// `.nii`, `.nii.gz`).
pub struct FileLoader;

impl TransformLoader for FileLoader {
    fn load(&self, path: &Path) -> Result<Transform, String> {
        larmorx_io::itk_transform::read_itk_transform(path).map_err(|e| e.to_string())
    }
}

/// The ANTs tools on the command line.
pub const TOOLS: &[&str] = &["antsApplyTransforms"];

/// Runs ANTs tool `tool` with `args` (the arguments after the tool name). `None` if there is
/// no such tool.
pub fn run(
    tool: &str,
    args: &[String],
    loader: &dyn TransformLoader,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Option<u8> {
    match tool {
        "antsApplyTransforms" => Some(apply_transforms::main(args, loader, out, err)),
        _ => None,
    }
}

/// The number of threads ANTs would use: `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` if set, else
/// all logical CPUs (0).
pub(crate) fn itk_threads() -> usize {
    std::env::var("ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}
