// SPDX-License-Identifier: Apache-2.0
//! ImageMath's component and distance-map operations on the command line
//! (`ImageMath_Templates.hxx`, ANTs v2.6.5): `GetLargestComponent`, `D` (`DistanceMap`) and
//! `MaurerDistance` (`GenerateMaurerDistanceImage`).

use super::{Context, OpError, Operation};
use crate::cli::cstd::{atof, stoi};
use crate::image::AntsImage;
use crate::image_math::{distance_map, largest_component, maurer_distance};

const ALL: &[usize] = &[2, 3, 4];

/// The operations of this group.
pub const OPERATIONS: &[Operation] = &[
    Operation {
        name: "GetLargestComponent",
        dims: ALL,
        usage: "GetLargestComponent InputImage {MinObjectSize} : Get the largest object in an image",
        run: run_largest_component,
    },
    Operation {
        name: "D",
        dims: ALL,
        usage: "D Image.ext : Danielson Distance Transform",
        run: run_distance_map,
    },
    Operation {
        name: "MaurerDistance",
        dims: ALL,
        usage: "MaurerDistance inputImage {foreground=1} : Calculate the signed Maurer distance transform",
        run: run_maurer_distance,
    },
];

/// `GetLargestComponent`: `ImageMath d out GetLargestComponent image [smallest=50]`; writes
/// the image it read (keeping its `descrip`), only for output names longer than 3
/// characters.
fn run_largest_component(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let smallest = match ctx.arg(5) {
        None => 50u64,
        Some(s) => {
            let n = stoi(s).ok_or_else(|| {
                OpError::crash(format!(
                    "larmorx: GetLargestComponent: the minimum size '{s}' is not a number (ANTs aborts: std::stoi throws)"
                ))
            })?;
            // `unsigned long smallest = std::stoi(...)`: a negative size wraps to near 2^64.
            i64::from(n) as u64
        }
    };
    let image: AntsImage<f32> = ctx.read(&fn1)?;
    let data = largest_component(image.view(), smallest, ctx.n_threads)?;
    if output.len() > 3 {
        ctx.write(&output, image.with_data(data))?;
    }
    Ok(())
}

/// `DistanceMap`: `ImageMath d out D image`, written only for output names longer than 3
/// characters.
fn run_distance_map(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let image: AntsImage<f32> = ctx.read(&fn1)?;
    let data = distance_map(image.view())?;
    if output.len() > 3 {
        ctx.write(&output, image.derived(data))?;
    }
    Ok(())
}

/// `GenerateMaurerDistanceImage`: `ImageMath d out MaurerDistance image [foreground=1]`.
fn run_maurer_distance(ctx: &mut Context<'_>) -> Result<(), OpError> {
    let output = ctx.output().to_owned();
    let fn1 = ctx.required(4)?.to_owned();
    let image: AntsImage<f32> = ctx.read(&fn1)?;
    let foreground = ctx.arg(5).map_or(1.0f32, |s| atof(s) as f32);
    let data = maurer_distance(image.view(), foreground, ctx.n_threads)?;
    ctx.write(&output, image.derived(data))?;
    Ok(())
}
