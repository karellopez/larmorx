// SPDX-License-Identifier: Apache-2.0
//! ImageMath's Gaussian operations on the command line: `G`, `Laplacian`, `Grad` and
//! `UnsharpMask` (`ImageMath_Templates.hxx`, ANTs v2.6.5: `SmoothImage<DIM>`,
//! `LaplacianImage`, `GradientImage`, `UnsharpMaskImage`).

use larmorx_core::defects;

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
        // ANTS-15: ANTs goes on with a variance of 0 and exits 0.
        if ctx.fix() {
            ctx.notices.add(
                &defects::ANTS_15,
                format!(
                    "{} sigma(s) for a {}-dimensional image: an error; ANTs filters with a \
                     variance of 0 (the image unchanged) and exits 0",
                    sigma.len(),
                    ctx.dim
                ),
            );
            return Err(OpError::crash(format!(
                "larmorx: G needs 1 or {} sigmas, not {} ({}=reproduce goes on as ANTs does)",
                ctx.dim,
                sigma.len(),
                defects::ENV
            )));
        }
        ctx.notices.add(
            &defects::ANTS_15,
            format!(
                "{} sigma(s) for a {}-dimensional image: the image is filtered with a variance \
                 of 0 (unchanged), as in ANTs",
                sigma.len(),
                ctx.dim
            ),
        );
        None
    };
    let (data, notices) = smooth_discrete(image.view(), sigma, ctx.n_threads, ctx.notices.mode())?;
    ctx.notices.extend(notices);
    ctx.write(&output, image.derived(data))?;
    Ok(())
}

/// `LaplacianImage`: `ImageMath d out Laplacian image [s] [normalize]`. ANTs reads **both**
/// sigma and the normalize flag from the same argument (`argv[5]`): sigma with `atof`, the
/// flag with `std::stoi` (ANTS-16). So `Laplacian img 1.5 1` normalizes because `1.5` reads as
/// 1, `0.8 1` does not, `.5` aborts, and the argument after the sigma is ignored. larmorx
/// reads the flag from its own argument (`argv[6]`), as `Grad` does, unless reproducing.
fn run_laplacian(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let sigma_arg = ctx.arg(5).map(str::to_owned);
    let flag_arg = ctx.arg(6).map(str::to_owned);
    let sigma = sigma_arg.as_deref().map_or(1.0f32, |s| atof(s) as f32);
    // ANTs' flag: stoi of the sigma's argument (None where std::stoi throws).
    let ants_flag = sigma_arg.as_deref().map(|s| stoi(s).map(|v| v != 0));
    let own_flag = match flag_arg.as_deref() {
        None => false,
        Some(s) => stoi_arg("Laplacian", s)? != 0,
    };
    let normalize = if ctx.fix() {
        match ants_flag {
            Some(Some(f)) if f == own_flag => {}
            Some(ants) => {
                let ants = match ants {
                    Some(true) => "normalizes",
                    Some(false) => "does not normalize",
                    None => "aborts (std::stoi throws)",
                };
                ctx.notices.add(
                    &defects::ANTS_16,
                    format!(
                        "the normalize flag is read from its own argument ({}); ANTs reads it \
                         from the sigma's and {ants}",
                        if own_flag {
                            "normalize"
                        } else {
                            "do not normalize"
                        }
                    ),
                );
            }
            None => {}
        }
        own_flag
    } else {
        match ants_flag {
            None => false,
            Some(Some(f)) => {
                if f != own_flag {
                    ctx.notices.add(
                        &defects::ANTS_16,
                        format!(
                            "the normalize flag is read from the sigma's argument, as in ANTs: \
                             {}",
                            if f { "normalize" } else { "do not normalize" }
                        ),
                    );
                }
                f
            }
            Some(None) => {
                return Err(OpError::crash(format!(
                    "larmorx: Laplacian: '{}' is not a number (ANTs aborts: std::stoi throws)",
                    sigma_arg.as_deref().unwrap_or_default()
                )));
            }
        }
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
