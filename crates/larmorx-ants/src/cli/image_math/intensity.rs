// SPDX-License-Identifier: Apache-2.0
//! ImageMath's intensity operations on the command line: `TruncateImageIntensity`,
//! `Normalize`, `RescaleImage`, `ThresholdAtMean` and `ReplaceVoxelValue`
//! (`ImageMath_Templates.hxx`, ANTs v2.6.5).

use super::{Context, OpError, Operation};
use crate::cli::cstd::{atof, stoi};
use crate::image::AntsImage;
use crate::image_math::{
    Normalization, TruncateOptions, normalize, replace_voxel_value, rescale, threshold_at_mean,
    truncate_image_intensity,
};

const ALL: &[usize] = &[2, 3, 4];

/// The operations of this group.
pub const OPERATIONS: &[Operation] = &[
    Operation {
        name: "TruncateImageIntensity",
        dims: ALL,
        usage: "TruncateImageIntensity InputImage.ext {lowerQuantile=0.025} {upperQuantile=0.975} {numberOfBins=64} {binary-maskImage}",
        run: run_truncate,
    },
    Operation {
        name: "Normalize",
        dims: ALL,
        usage: "Normalize Image.ext opt : Normalize to [0,1]. Option instead divides by average value.  If opt is a mask image, then we normalize by mean intensity in the mask ROI.",
        run: run_normalize,
    },
    Operation {
        name: "RescaleImage",
        dims: ALL,
        usage: "RescaleImage InputImage min max",
        run: run_rescale,
    },
    Operation {
        name: "ThresholdAtMean",
        dims: ALL,
        usage: "ThresholdAtMean Image %ofMean : 1 where the value is at least mean x fraction (and at most the maximum)",
        run: run_threshold_at_mean,
    },
    Operation {
        name: "ReplaceVoxelValue",
        dims: ALL,
        usage: "ReplaceVoxelValue inputImage low high replaceVal : Replace voxels with intensities in [low, high] with replaceVal",
        run: run_replace_voxel_value,
    },
];

/// `TruncateImageIntensity`: `ImageMath d out TruncateImageIntensity image [lo] [hi] [bins]
/// [mask] [copy-space]`; writes the image it read, clipped in place.
fn run_truncate(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let lower_quantile = ctx.arg(5).map_or(0.025f32, |s| atof(s) as f32);
    let upper_quantile = ctx.arg(6).map_or(1.0 - lower_quantile, |s| atof(s) as f32);
    let bins = match ctx.arg(7) {
        None => 64,
        Some(s) => {
            let n = stoi(s).ok_or_else(|| {
                OpError::crash(format!(
                    "larmorx: TruncateImageIntensity: the number of bins '{s}' is not a number (ANTs aborts: std::stoi throws)"
                ))
            })?;
            // `unsigned int numberOfBins = std::stoi(...)`: a negative count wraps.
            usize::try_from(n).map_err(|_| {
                OpError::crash(format!(
                    "larmorx: TruncateImageIntensity: {n} histogram bins (ANTs runs out of memory or crashes)"
                ))
            })?
        }
    };
    let image: AntsImage<f32> = ctx.read(&fn1)?;
    let mask: Option<AntsImage<i32>> = match ctx.arg(8).map(str::to_owned) {
        Some(name) => ctx.try_read(&name)?,
        None => None,
    };
    if let Some(m) = &mask
        && m.size() != image.size()
    {
        return Err(OpError::crash(format!(
            "larmorx: the mask size {:?} differs from the image size {:?} (ANTs reads outside one of them)",
            m.size(),
            image.size()
        )));
    }
    let options = TruncateOptions {
        lower_quantile,
        upper_quantile,
        bins,
    };
    let truncated = truncate_image_intensity(
        &image.data,
        mask.as_ref().map(|m| m.data.as_slice()),
        &options,
        ctx.n_threads,
    )?;
    if output.len() > 3 {
        ctx.write(&output, image.with_data(truncated.data))?;
    }
    Ok(())
}

/// `NormalizeImage`: `ImageMath d out Normalize image [mask | option]`; writes the image it
/// read, changed in place.
fn run_normalize(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let mut option = 0.0f32;
    let mut mask: Option<AntsImage<f32>> = None;
    if let Some(name) = ctx.arg(5).map(str::to_owned) {
        mask = ctx.try_read(&name)?;
        if mask.is_none() {
            option = atof(&name) as f32;
        }
    }
    let image: AntsImage<f32> = ctx.read(&fn1)?;
    let mode = match &mask {
        Some(m) => {
            if m.size() != image.size() {
                return Err(OpError::crash(format!(
                    "larmorx: the mask size {:?} differs from the image size {:?} (ANTs reads outside one of them)",
                    m.size(),
                    image.size()
                )));
            }
            Normalization::MaskMean(&m.data)
        }
        None if larmorx_image::itk_math::almost_equal_f32(option, 0.0) => Normalization::Range,
        None => Normalization::Mean,
    };
    let data = normalize(&image.data, mode, ctx.n_threads)?;
    if output.len() > 3 {
        ctx.write(&output, image.with_data(data))?;
    }
    Ok(())
}

/// `RescaleImage`: `ImageMath d out RescaleImage image min max`
/// (`RescaleIntensityImageFilter`).
fn run_rescale(ctx: &mut Context<'_>) -> Result<(), OpError> {
    if ctx.argc() < 7 {
        return Err(OpError::crash(
            "larmorx: RescaleImage needs an input image, a minimum and a maximum (ANTs aborts: it throws std::exception)",
        ));
    }
    let output = ctx.output().to_owned();
    let input = ctx.required(4)?.to_owned();
    let min = atof(ctx.required(5)?) as f32;
    let max = atof(ctx.required(6)?) as f32;
    let image: AntsImage<f32> = ctx.read(&input)?;
    let data = rescale(&image.data, min, max, ctx.n_threads).map_err(|e| {
        OpError::crash(format!(
            "larmorx: RescaleImage: {e} (ANTs aborts: ITK throws)"
        ))
    })?;
    ctx.write(&output, image.derived(data))?;
    Ok(())
}

/// `ThresholdAtMean`: `ImageMath d out ThresholdAtMean image [fraction=1]`.
fn run_threshold_at_mean(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let fraction = ctx.arg(5).map_or(1.0f32, |s| atof(s) as f32);
    let image: AntsImage<f32> = ctx.read(&fn1)?;
    let data = threshold_at_mean(&image.data, fraction, ctx.n_threads).map_err(|e| {
        OpError::crash(format!(
            "larmorx: ThresholdAtMean: {e} (ANTs aborts: ITK throws)"
        ))
    })?;
    ctx.write(&output, image.derived(data))?;
    Ok(())
}

/// `ReplaceVoxelValue`: `ImageMath d out ReplaceVoxelValue image low high value`; ANTs reads
/// all three numbers whatever the argument count (a missing one crashes it: `atof(NULL)`).
fn run_replace_voxel_value(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let low = atof(ctx.required(5)?) as f32;
    let high = atof(ctx.required(6)?) as f32;
    let value = atof(ctx.required(7)?) as f32;
    let image: AntsImage<f32> = ctx.read(&fn1)?;
    let data = replace_voxel_value(&image.data, low, high, value, ctx.n_threads)?;
    ctx.write(&output, image.derived(data))?;
    Ok(())
}
