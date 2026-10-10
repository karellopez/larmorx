// SPDX-License-Identifier: Apache-2.0
//! ImageMath's Gaussian operations on the command line: `G`, `Laplacian`, `Grad` and
//! `UnsharpMask` (`ImageMath_Templates.hxx`, ANTs v2.6.5: `SmoothImage<DIM>`,
//! `LaplacianImage`, `GradientImage`, `UnsharpMaskImage`).

use super::{Context, OpError, Operation};
use crate::cli::cstd::{atof, convert_vector_f32, stof, stoi};
use crate::image::AntsImage;
use crate::image_math::{
    UnsharpMaskOptions, gradient_magnitude, laplacian, smooth_discrete, unsharp_mask,
};

const ALL: &[usize] = &[2, 3, 4];

/// The operations of this group.
pub const OPERATIONS: &[Operation] = &[
    Operation {
        name: "G",
        dims: ALL,
        usage: "G Image1.ext s    : Smooth with Gaussian of sigma = s",
        run: run_g,
    },
    Operation {
        name: "Grad",
        dims: ALL,
        usage: "Grad Image.ext s normalize? : Gradient magnitude with sigma s (if normalize, then output in range [0, 1])",
        run: run_grad,
    },
    Operation {
        name: "Laplacian",
        dims: ALL,
        usage: "Laplacian Image.ext s normalize? : Laplacian computed with sigma s (if normalize, then output in range [0, 1])",
        run: run_laplacian,
    },
    Operation {
        name: "UnsharpMask",
        dims: ALL,
        usage: "UnsharpMask ImageIn [amount=0.5] [radius=1] [threshold=0] [radius in spacing unit (0)/1] : Apply an Unsharp Mask filter",
        run: run_unsharp_mask,
    },
];

/// `std::stoi` of an argument, or the crash ANTs has when it throws.
fn stoi_arg(op: &str, s: &str) -> Result<i32, OpError> {
    stoi(s).ok_or_else(|| {
        OpError::crash(format!(
            "larmorx: {op}: '{s}' is not a number (ANTs aborts: std::stoi throws)"
        ))
    })
}

/// `std::stof` of an argument, or the crash ANTs has when it throws.
fn stof_arg(op: &str, s: &str) -> Result<f32, OpError> {
    stof(s).ok_or_else(|| {
        OpError::crash(format!(
            "larmorx: {op}: '{s}' is not a number (ANTs aborts: std::stof throws)"
        ))
    })
}

/// `SmoothImage<DIM>`: `ImageMath d out G image [sigma | s1xs2x...]`
/// (`DiscreteGaussianImageFilter`, sigma in physical units).
fn run_g(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let sigma = match ctx.arg(5) {
        None => Vec::new(),
        Some(s) => convert_vector_f32(s).ok_or_else(|| {
            OpError::crash(format!(
                "larmorx: G: the sigma '{s}' starts with an empty element (ANTs reads an uninitialised value)"
            ))
        })?,
    };
    let image: AntsImage<f32> = ctx.read(&fn1)?;
    let sigma = if sigma.len() == 1 || sigma.len() == ctx.dim {
        Some(sigma.as_slice())
    } else {
        let _ = writeln!(
            ctx.err,
            "Incorrect sigma vector size.  Must either be of size 1 or ImageDimension."
        );
        None
    };
    let data = smooth_discrete(image.view(), sigma, ctx.n_threads)?;
    ctx.write(&output, image.derived(data))?;
    Ok(())
}

/// `LaplacianImage`: `ImageMath d out Laplacian image [s]`. ANTs reads **both** sigma and
/// the normalize flag from the same argument (`argv[5]`): sigma with `atof`, the flag with
/// `std::stoi`. So `Laplacian img 1.5 1` normalizes because `1.5` reads as 1, `0.8 1` does
/// not, and the argument after the sigma is ignored.
fn run_laplacian(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let (sigma, normalize) = match ctx.arg(5).map(str::to_owned) {
        None => (1.0f32, false),
        Some(s) => (atof(&s) as f32, stoi_arg("Laplacian", &s)? != 0),
    };
    let image: AntsImage<f32> = ctx.read(&fn1)?;
    let data = laplacian(image.view(), sigma, normalize, ctx.n_threads)?;
    ctx.write(&output, image.derived(data))?;
    Ok(())
}

/// `GradientImage`: `ImageMath d out Grad image [s] [normalize]`.
fn run_grad(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let sigma = ctx.arg(5).map_or(1.0f32, |s| atof(s) as f32);
    let normalize = match ctx.arg(6).map(str::to_owned) {
        None => false,
        Some(s) => stoi_arg("Grad", &s)? != 0,
    };
    let image: AntsImage<f32> = ctx.read(&fn1)?;
    let data = gradient_magnitude(image.view(), sigma, normalize, ctx.n_threads)?;
    ctx.write(&output, image.derived(data))?;
    Ok(())
}

/// `UnsharpMaskImage`: `ImageMath d out UnsharpMask image [amount] [radius] [threshold]
/// [radius-in-spacing-units]`.
fn run_unsharp_mask(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let image: AntsImage<f32> = ctx.read(&fn1)?;
    let mut options = UnsharpMaskOptions::default();
    if let Some(s) = ctx.arg(5) {
        options.amount = stof_arg("UnsharpMask", s)?;
    }
    if let Some(s) = ctx.arg(6) {
        options.radius = stof_arg("UnsharpMask", s)?;
    }
    if let Some(s) = ctx.arg(7) {
        options.threshold = stof_arg("UnsharpMask", s)?;
    }
    if let Some(s) = ctx.arg(8).map(str::to_owned) {
        options.radius_in_spacing_units = stoi_arg("UnsharpMask", &s)? != 0;
    }
    let data = unsharp_mask(image.view(), &options, ctx.n_threads).map_err(|e| {
        OpError::crash(format!(
            "larmorx: UnsharpMask: {e} (ANTs aborts: ITK throws)"
        ))
    })?;
    ctx.write(&output, image.derived(data))?;
    Ok(())
}
