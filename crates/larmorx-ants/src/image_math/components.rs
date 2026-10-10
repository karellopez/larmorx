// SPDX-License-Identifier: Apache-2.0
//! ImageMath's component and distance-map operations (ANTs v2.6.5,
//! `Examples/ImageMath_Templates.hxx`):
//! - `GetLargestComponent`: [`largest_component`] (`GetLargestComponent`, lines 427-563);
//! - `D`: [`distance_map`] (`DistanceMap`, lines 8799-8833), ITK's Danielsson map;
//! - `MaurerDistance`: [`maurer_distance`] (`GenerateMaurerDistanceImage`, lines 8836-8876),
//!   ITK's signed Maurer map.

use larmorx_core::parallel;
use larmorx_image::components::{connected_components, relabel_components};
use larmorx_image::distance::{MaurerOptions, danielsson_distance_map, signed_maurer_distance_map};
use larmorx_image::{FilterError, VolumeRef};
use rayon::prelude::*;

use super::morphology::float_labels;

const CHUNK: usize = 1 << 16;

/// `GetLargestComponent`: `ImageMath d out GetLargestComponent image [smallest=50]` on a
/// `float` image; returns the new voxels of the image (1 in the largest component, 0
/// elsewhere).
///
/// The object is the voxels in `[0.25, 1e9]`; its face-connected components are relabelled
/// by size, components smaller than `smallest` (if it is not 0) dropped; every voxel gets the
/// size of its component **as a float** (0 outside the kept components), and the voxels whose
/// size is at least the largest become 1. So:
/// - every component of the largest size is kept (ties keep all of them; sizes above 2^24
///   are compared after rounding to float);
/// - **if no component is kept** (an empty mask, or every component smaller than `smallest`)
///   the largest size is 0 and **every voxel becomes 1**.
pub fn largest_component(
    input: VolumeRef<'_, f32>,
    smallest: u64,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let (low, high) = (0.25f32, 1.0e9f32);
    let object: Vec<bool> = parallel::with_threads(n_threads, || {
        input
            .data
            .par_iter()
            .with_min_len(CHUNK)
            .map(|&v| low <= v && v <= high)
            .collect()
    })?;
    let cc = connected_components(&object, input.size, false, n_threads)?;
    let relabel = relabel_components(&cc.labels, cc.count, smallest, n_threads)?;
    let kept = relabel.sizes.len();
    if kept > 1 << 24 {
        return Err(FilterError::Invalid(format!(
            "{kept} components: ANTs' float labels no longer tell them apart"
        )));
    }
    debug_assert!(
        float_labels(kept)
            .iter()
            .enumerate()
            .all(|(i, &l)| l == (i + 1) as f32)
    );
    // `Clusters`: each kept component's size, `unsigned int` stored as float.
    let size_of: Vec<f32> = std::iter::once(0.0)
        .chain(relabel.sizes.iter().map(|&s| s as u32 as f32))
        .collect();
    let largest = size_of
        .iter()
        .copied()
        .fold(0.0f32, |m, s| if s > m { s } else { m });
    Ok(parallel::with_threads(n_threads, || {
        cc.labels
            .par_iter()
            .with_min_len(CHUNK)
            .map(|&l| {
                let k = relabel.map[l as usize] as usize;
                if size_of[k] >= largest { 1.0 } else { 0.0 }
            })
            .collect()
    })?)
}

/// `D` (`DistanceMap`): ITK's `DanielssonDistanceMapImageFilter` with `InputIsBinaryOff` and
/// `SetUseImageSpacing(true)`: the distance in millimetres from every voxel to the nearest
/// non-zero voxel (0 on them). Sequential, as Danielsson's sweeps are.
pub fn distance_map(input: VolumeRef<'_, f32>) -> Result<Vec<f32>, FilterError> {
    danielsson_distance_map(input, true, false)
}

/// `MaurerDistance` (`GenerateMaurerDistanceImage`): the voxels equal to `foreground` (in
/// float) become 1 and the others 0 (`BinaryThresholdImageFilter`), then ITK's
/// `SignedMaurerDistanceMapImageFilter` with `SquaredDistance` off, `UseImageSpacing` on and
/// `InsideIsPositive` off: the distance in millimetres to the object's inner contour,
/// negative inside (`-0.0` on the contour). Without any object voxel, or with nothing else,
/// every voxel is `±1.8446743e19` (the square root of `FLT_MAX`).
pub fn maurer_distance(
    input: VolumeRef<'_, f32>,
    foreground: f32,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let binary: Vec<f32> = parallel::with_threads(n_threads, || {
        input
            .data
            .par_iter()
            .with_min_len(CHUNK)
            .map(|&v| {
                if foreground <= v && v <= foreground {
                    1.0
                } else {
                    0.0
                }
            })
            .collect()
    })?;
    let options = MaurerOptions {
        background: 0.0,
        inside_is_positive: false,
        squared: false,
        use_spacing: true,
    };
    signed_maurer_distance_map(
        VolumeRef::new(&binary, input.size, input.spacing),
        &options,
        n_threads,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn largest_component_keeps_ties_and_fills_when_nothing_is_kept() {
        // Components of 3, 3 and 2 voxels along a line (0.1 is below 0.25: background).
        let data = [
            1.0f32, 1.0, 1.0, 0.0, 0.3, 0.3, 0.3, 0.0, 2.0, 2.0, 0.0, 0.1,
        ];
        let size = [12usize];
        let sp = [1.0];
        let v = VolumeRef::new(&data, &size, &sp);
        let both = vec![1., 1., 1., 0., 1., 1., 1., 0., 0., 0., 0., 0.];
        assert_eq!(largest_component(v, 0, 1).unwrap(), both);
        assert_eq!(largest_component(v, 3, 2).unwrap(), both);
        // Every component below the minimum size: everything becomes 1.
        let out = largest_component(v, 4, 1).unwrap();
        assert!(out.iter().all(|&o| o == 1.0));
        let tie = [1.0f32, 1.0, 0.0, 1.0, 1.0];
        let out = largest_component(VolumeRef::new(&tie, &[5], &sp), 0, 1).unwrap();
        assert_eq!(out, vec![1., 1., 0., 1., 1.]);
    }

    #[test]
    fn maurer_is_signed_and_zero_on_the_contour() {
        let mut data = vec![0.0f32; 9 * 9];
        for y in 2..7 {
            for x in 2..7 {
                data[x + 9 * y] = 3.0;
            }
        }
        let size = [9usize, 9];
        let sp = [2.0, 1.0];
        let out = maurer_distance(VolumeRef::new(&data, &size, &sp), 3.0, 2).unwrap();
        assert_eq!(out[2 + 9 * 4].to_bits(), (-0.0f32).to_bits());
        assert_eq!(out[4 + 9 * 4], -2.0);
        assert_eq!(out[9 * 4], 4.0);
        let d = distance_map(VolumeRef::new(&data, &size, &sp)).unwrap();
        assert_eq!(d[4 + 9 * 4], 0.0);
        assert_eq!(d[9 * 4], 4.0);
    }
}
