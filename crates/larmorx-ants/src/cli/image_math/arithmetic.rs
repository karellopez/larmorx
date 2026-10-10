// SPDX-License-Identifier: Apache-2.0
//! ImageMath's voxel-wise arithmetic on the command line: `ImageMath<DIM>` (`m`, `+`, `-`,
//! `/`, `^`, `exp`, `max`, `abs`, `addtozero`, `overadd`, `Decision`, `total`, `mean`,
//! `vtotal`) and `NegativeImage` (`Neg`), from `ImageMath_Templates.hxx` (ANTs v2.6.5).

use super::{Context, OpError, Operation};
use crate::cli::cstd::{format_g, from_string_f32};
use crate::image::AntsImage;
use crate::image_math::{Arithmetic, Operand, arithmetic, negative};

const ALL: &[usize] = &[2, 3, 4];

macro_rules! arith {
    ($name:literal, $usage:literal) => {
        Operation {
            name: $name,
            dims: ALL,
            usage: $usage,
            run: run_arithmetic,
        }
    };
}

/// The operations of this group.
pub const OPERATIONS: &[Operation] = &[
    arith!("m", "m            : Multiply"),
    arith!("+", "+            : Add"),
    arith!("-", "-            : Subtract"),
    arith!(
        "/",
        "/            : Divide (where the divisor is > 0; else the previous voxel's value)"
    ),
    arith!("^", "^            : Power"),
    arith!("exp", "exp          : Take exponent exp(imagevalue*value)"),
    arith!("max", "max          : voxelwise max"),
    arith!("abs", "abs          : absolute value"),
    arith!(
        "addtozero",
        "addtozero    : add image-b to image-a only over points where image-a has zero values"
    ),
    arith!(
        "overadd",
        "overadd      : replace image-a pixel with image-b pixel if image-b pixel is non-zero"
    ),
    arith!(
        "Decision",
        "Decision     : Computes result=1./(1.+exp(-1.0*( pix1-0.25)/pix2))"
    ),
    arith!(
        "total",
        "total        : Sums up values in an image or in image1*image2 (img2 is the probability mask)"
    ),
    arith!(
        "mean",
        "mean         : Average of values in an image or in image1*image2 (img2 is the probability mask)"
    ),
    arith!(
        "vtotal",
        "vtotal       : (writes zeros: ANTs implements nothing for it)"
    ),
    Operation {
        name: "Neg",
        dims: ALL,
        usage: "Neg          : Produce image negative",
        run: run_negative,
    },
];

/// `ImageMath<DIM>`: `ImageMath d out op image1 [image2 | number]`.
fn run_arithmetic(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let op = Arithmetic::from_name(ctx.op()).expect("registered name");
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let fn2 = ctx.arg(5).unwrap_or("").to_owned();
    // `from_string<float>` decides whether the operand is a number; the image is read first.
    let parsed = from_string_f32(&fn2);
    let mut image2: Option<AntsImage<f32>> = None;
    let mut scalar = 1.0f32;
    if parsed.is_number {
        if let Some(v) = parsed.value {
            scalar = v;
        }
    } else {
        image2 = ctx.try_read(&fn2)?;
    }
    let image1: Option<AntsImage<f32>> = ctx.try_read(&fn1)?;
    let Some(image1) = image1 else {
        return Err(OpError::crash(format!(
            "larmorx: cannot read image '{fn1}' (ANTs crashes here: it uses the null image)"
        )));
    };
    let operand = if parsed.is_number {
        Operand::Scalar(scalar)
    } else {
        let Some(image2) = image2.as_ref() else {
            return Err(OpError::crash(format!(
                "larmorx: '{fn2}' is neither a number nor a readable image (ANTs crashes here: it uses the null image)"
            )));
        };
        Operand::Image {
            data: &image2.data,
            size: image2.size(),
        }
    };
    let result = arithmetic(op, &image1.data, image1.size(), operand, ctx.n_threads)?;
    let volume_element = image1
        .geometry
        .spacing
        .iter()
        .fold(1.0f32, |acc, &s| acc * s as f32);
    match op {
        Arithmetic::Total => {
            let _ = writeln!(
                ctx.out,
                "total: {} total-volume: {}",
                format_g(f64::from(result.result)),
                format_g(f64::from(result.result * volume_element))
            );
        }
        Arithmetic::Mean => {
            let _ = writeln!(
                ctx.out,
                "{}",
                format_g(f64::from(result.result / result.count as f32))
            );
        }
        _ => {}
    }
    if output.len() > 3 {
        let out = image1.derived(result.data);
        ctx.write(&output, out)?;
    }
    Ok(())
}

/// `NegativeImage`: `ImageMath d out Neg image`; writes the image it read, changed in place.
fn run_negative(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let image: AntsImage<f32> = ctx.read(&fn1)?;
    let data = negative(&image.data, ctx.n_threads)?;
    let image = image.with_data(data);
    ctx.write(&output, image)?;
    Ok(())
}
