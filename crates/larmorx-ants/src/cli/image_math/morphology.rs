// SPDX-License-Identifier: Apache-2.0
//! ImageMath's morphology and mask operations on the command line (`ImageMath_Templates.hxx`,
//! ANTs v2.6.5): `MD`, `ME`, `MO`, `MC`, `GD`, `GE`, `GO`, `GC` (`MorphImage`), `FillHoles`
//! and `PadImage`.

use super::{Context, OpError, Operation};
use crate::cli::cstd::atof;
use crate::image::AntsImage;
use crate::image_math::{
    FillHolesError, Morphology, fill_holes, morphological, pad_image, radius_from_f32,
};
use larmorx_image::morphology::MAX_RADIUS;

const ALL: &[usize] = &[2, 3, 4];

macro_rules! morph {
    ($name:literal, $usage:literal) => {
        Operation {
            name: $name,
            dims: ALL,
            usage: $usage,
            run: run_morph,
        }
    };
}

/// The operations of this group.
pub const OPERATIONS: &[Operation] = &[
    morph!(
        "MD",
        "MD Image1.ext s [value=1] : Morphological Dilation with radius s (binary: value is the foreground)"
    ),
    morph!(
        "ME",
        "ME Image1.ext s [value=1] : Morphological Erosion with radius s (binary; output 0/1)"
    ),
    morph!(
        "MO",
        "MO Image1.ext s [value=1] : Morphological Opening with radius s (binary)"
    ),
    morph!(
        "MC",
        "MC Image1.ext s [value=1] : Morphological Closing with radius s (binary)"
    ),
    morph!("GD", "GD Image1.ext s : Grayscale Dilation with radius s"),
    morph!("GE", "GE Image1.ext s : Grayscale Erosion with radius s"),
    morph!("GO", "GO Image1.ext s : Grayscale Opening with radius s"),
    morph!("GC", "GC Image1.ext s : Grayscale Closing with radius s"),
    Operation {
        name: "FillHoles",
        dims: ALL,
        usage: "FillHoles Image parameter : Parameter = ratio of edge at object to edge at background;  --  Parameter = 1 is a definite hole bounded by object only, 0.99 is close -- Default of parameter > 1 will fill all holes",
        run: run_fill_holes,
    },
    Operation {
        name: "PadImage",
        dims: ALL,
        usage: "PadImage ImageIn Pad-Number [value] : If Pad-Number is negative, de-Padding occurs",
        run: run_pad_image,
    },
];

/// `MorphImage`: `ImageMath d out <MD|ME|MO|MC|GD|GE|GO|GC> image [radius=1] [value=1]`.
/// The radius is read with `atof` and truncated (`static_cast<unsigned long>`); the output is
/// written only for names longer than 3 characters.
fn run_morph(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let operation = Morphology::from_name(ctx.op()).expect("registered name");
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let rad = ctx.arg(5).map_or(1.0f32, |s| atof(s) as f32);
    let value = ctx.arg(6).map_or(1.0f32, |s| atof(s) as f32);
    let image: AntsImage<f32> = ctx.read(&fn1)?;
    let Some(radius) = radius_from_f32(rad, MAX_RADIUS) else {
        let what = if rad.is_nan() || rad <= -1.0 {
            "ANTs converts it to a radius near 2^64 and cannot allocate its structuring element"
        } else {
            "larmorx accepts radii up to 65535 voxels"
        };
        return Err(OpError::unsupported(format!(
            "larmorx: {}: a radius of {rad} is not supported ({what})",
            ctx.op()
        )));
    };
    let data = morphological(image.view(), operation, radius, value, ctx.n_threads)?;
    if output.len() > 3 {
        ctx.write(&output, image.derived(data))?;
    }
    Ok(())
}

/// `FillHoles`: `ImageMath d out FillHoles image [holeparam=2]`; writes the image it read with
/// the holes set to 1 (keeping its `descrip`).
fn run_fill_holes(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let hole_param = ctx.arg(5).map_or(2.0f32, |s| atof(s) as f32);
    let image: AntsImage<f32> = ctx.read(&fn1)?;
    let data = fill_holes(image.view(), hole_param, ctx.n_threads).map_err(|e| match e {
        FillHolesError::Filter(e) => OpError::from(e),
        other => OpError::unsupported(format!("larmorx: FillHoles: {other}")),
    })?;
    ctx.write(&output, image.with_data(data))?;
    Ok(())
}

/// `PadImage`: `ImageMath d out PadImage image pad [value=0]`.
fn run_pad_image(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let pad = atof(ctx.required(5)?) as f32;
    let value = ctx.arg(6).map_or(0.0f32, |s| atof(s) as f32);
    if fn1.len() <= 3 {
        return Err(OpError::crash(format!(
            "larmorx: PadImage: ANTs reads no image for a name of 3 characters or fewer ('{fn1}') and then uses the null image"
        )));
    }
    let image: AntsImage<f32> = ctx.read(&fn1)?;
    let padded = pad_image(&image, pad, value).map_err(|e| {
        OpError::crash(format!(
            "larmorx: {e} (ANTs cannot allocate or write such an image)"
        ))
    })?;
    ctx.write(&output, padded)?;
    Ok(())
}
