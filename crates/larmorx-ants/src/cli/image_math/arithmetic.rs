// SPDX-License-Identifier: Apache-2.0
//! ImageMath's voxel-wise arithmetic on the command line: `ImageMath<DIM>` (`m`, `+`, `-`,
//! `/`, `^`, `exp`, `max`, `abs`, `addtozero`, `overadd`, `Decision`, `total`, `mean`,
//! `vtotal`) and `NegativeImage` (`Neg`), from `ImageMath_Templates.hxx` (ANTs v2.6.5).

use larmorx_core::defects;

use super::{Context, OpError, Operation};
use crate::cli::cstd::{StrictNumber, format_g, from_string_f32, strict_f32};
use crate::image::AntsImage;
use crate::image_math::arithmetic::operand_is_smaller;
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
    arith!("/", "/            : Divide"),
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
        "vtotal       : (not implemented: ANTs writes zeros, LARMORX_UPSTREAM_DEFECTS=reproduce too)"
    ),
    Operation {
        name: "Neg",
        dims: ALL,
        usage: "Neg          : Produce image negative",
        run: run_negative,
    },
];

/// The operations whose operand larmorx may leave out when it corrects ANTS-05: those for
/// which ANTs' implicit 1 means "the image alone" (`abs` ignores it; `exp` is `exp(v · 1)`;
/// `total` and `mean` sum the image itself).
const OPTIONAL_OPERAND: &[Arithmetic] = &[
    Arithmetic::Abs,
    Arithmetic::Exp,
    Arithmetic::Total,
    Arithmetic::Mean,
    Arithmetic::VTotal,
];

/// How the operand `argv[5]` is read.
enum Reading {
    /// A number.
    Number(f32),
    /// An image file.
    Image(String),
    /// An error (larmorx's reading only).
    Error(String),
}

/// ANTs' reading of the operand: `from_string<float>` decides whether it is a number (no
/// operand at all is the number 1).
fn ants_reading(fn2: Option<&str>) -> Reading {
    let text = fn2.unwrap_or("");
    let parsed = from_string_f32(text);
    if parsed.is_number {
        Reading::Number(parsed.value.unwrap_or(1.0))
    } else {
        Reading::Image(text.to_owned())
    }
}

/// larmorx's reading (ANTS-05 corrected): a number when the text is exactly a number, an
/// error when it is malformed or out of `float`'s range, an image otherwise; no operand is
/// an error unless the operation can go without one ([`OPTIONAL_OPERAND`]).
fn strict_reading(op: Arithmetic, fn2: Option<&str>) -> Reading {
    let Some(text) = fn2 else {
        return if OPTIONAL_OPERAND.contains(&op) {
            Reading::Number(1.0)
        } else {
            Reading::Error(format!(
                "ImageMath {} needs a second operand (an image or a number)",
                op.name()
            ))
        };
    };
    match strict_f32(text) {
        StrictNumber::Number(v) => Reading::Number(v),
        StrictNumber::OutOfRange => {
            Reading::Error(format!("the operand '{text}' is out of float's range"))
        }
        StrictNumber::NotANumber if from_string_f32(text).is_number => {
            Reading::Error(format!("the operand '{text}' is not a number"))
        }
        StrictNumber::NotANumber => Reading::Image(text.to_owned()),
    }
}

/// What a reading says, for the notice.
fn describe(r: &Reading) -> String {
    match r {
        Reading::Number(v) => format!("the number {}", format_g(f64::from(*v))),
        Reading::Image(name) => format!("the image '{name}'"),
        Reading::Error(message) => format!("an error ({message})"),
    }
}

/// `ImageMath<DIM>`: `ImageMath d out op image1 [image2 | number]`.
fn run_arithmetic(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let op = Arithmetic::from_name(ctx.op()).expect("registered name");
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let fn2 = ctx.arg(5).map(str::to_owned);
    // ANTS-05: ANTs' reading of the operand, and larmorx's.
    let ants = ants_reading(fn2.as_deref());
    let strict = strict_reading(op, fn2.as_deref());
    let same = match (&ants, &strict) {
        (Reading::Number(a), Reading::Number(b)) => a.to_bits() == b.to_bits(),
        (Reading::Image(a), Reading::Image(b)) => a == b,
        _ => false,
    };
    if !same {
        let given = fn2
            .as_deref()
            .map_or("no operand".to_owned(), |t| format!("the operand '{t}'"));
        let detail = if ctx.fix() {
            format!(
                "{given} is {}; ANTs reads it as {}",
                describe(&strict),
                describe(&ants)
            )
        } else {
            format!("{given} is read as {}, as ANTs reads it", describe(&ants))
        };
        ctx.notices.add(&defects::ANTS_05, detail);
    }
    let reading = if ctx.fix() { strict } else { ants };
    if let Reading::Error(message) = &reading {
        return Err(OpError::crash(format!(
            "larmorx: {message} (ANTS-05; {}=reproduce reads it as ANTs does)",
            defects::ENV
        )));
    }
    // The second image is read first, as in ANTs.
    let mut image2: Option<AntsImage<f32>> = None;
    if let Reading::Image(name) = &reading {
        image2 = ctx.try_read(name)?;
    }
    let image1: Option<AntsImage<f32>> = ctx.try_read(&fn1)?;
    let Some(image1) = image1 else {
        return Err(OpError::crash(format!(
            "larmorx: cannot read image '{fn1}' (ANTs crashes here: it uses the null image)"
        )));
    };
    let operand = match &reading {
        Reading::Number(v) => Operand::Scalar(*v),
        Reading::Image(name) => {
            let Some(image2) = image2.as_ref() else {
                return Err(OpError::crash(format!(
                    "larmorx: '{name}' is neither a number nor a readable image (ANTs crashes here: it uses the null image)"
                )));
            };
            Operand::Image {
                data: &image2.data,
                size: image2.size(),
            }
        }
        Reading::Error(_) => unreachable!("returned above"),
    };
    let mode = ctx.notices.mode();
    let result = match arithmetic(
        op,
        &image1.data,
        image1.size(),
        operand,
        ctx.n_threads,
        mode,
    ) {
        Ok(r) => r,
        Err(e) => {
            // The corrected refusals (ANTS-07, ANTS-10) name their defect.
            if let Operand::Image { size, .. } = operand
                && operand_is_smaller(size, image1.size())
            {
                ctx.notices.add(
                    &defects::ANTS_07,
                    "the second image is smaller than the first: an error; ANTs reads outside it",
                );
            }
            if op == Arithmetic::VTotal {
                ctx.notices.add(
                    &defects::ANTS_10,
                    "vtotal is an error; ANTs writes an image of zeros",
                );
            }
            return Err(OpError::crash(format!(
                "larmorx: ImageMath {}: {e}",
                op.name()
            )));
        }
    };
    ctx.notices.extend(result.notices);
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
    let out = image1.derived(result.data);
    ctx.write_if_long(&output, out)?;
    Ok(())
}

/// `NegativeImage`: `ImageMath d out Neg image`; writes the image it read, changed in place.
fn run_negative(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let image: AntsImage<f32> = ctx.read(&fn1)?;
    let (data, notices) = negative(&image.data, ctx.n_threads, ctx.notices.mode())?;
    ctx.notices.extend(notices);
    let image = image.with_data(data);
    ctx.write(&output, image)?;
    Ok(())
}
