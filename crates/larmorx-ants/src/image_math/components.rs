// SPDX-License-Identifier: Apache-2.0
//! ImageMath's component and distance-map operations (ANTs v2.6.5,
//! `Examples/ImageMath_Templates.hxx`):
//! - `GetLargestComponent`: [`largest_component`] (`GetLargestComponent`, lines 429-563);
//! - `D`: [`distance_map`] (`DistanceMap`, lines 8801-8834), ITK's Danielsson map;
//! - `MaurerDistance`: [`maurer_distance`] (`GenerateMaurerDistanceImage`, lines 8838-8876),
//!   ITK's signed Maurer map;
//! - `ExtractContours`: [`extract_contours`] (`ExtractContours`, lines 8291-8320), ITK's
//!   `LabelContourImageFilter`.

use larmorx_core::defects::{self, Notice, Notices, UpstreamDefects};
use larmorx_core::parallel;
use larmorx_image::components::{connected_components, label_contour, relabel_components};
use larmorx_image::distance::{MaurerOptions, danielsson_distance_map, signed_maurer_distance_map};
use larmorx_image::{FilterError, VolumeRef};
use rayon::prelude::*;

const CHUNK: usize = 1 << 16;

/// `GetLargestComponent`: `ImageMath d out GetLargestComponent image [smallest=50]` on a
/// `float` image; returns the new voxels of the image (1 in the largest component, 0
/// elsewhere) and the notices of the defects it met.
///
/// The object is the voxels in `[0.25, 1e9]`; its face-connected components are relabelled
/// by size, components smaller than `smallest` (if it is not 0) dropped; every voxel gets the
/// size of its component **as a float** (0 outside the kept components), and the voxels whose
/// size is at least the largest become 1. So every component of the largest size is kept
/// (ties keep all of them; sizes above 2^24 are compared after rounding to float). Two
/// defects (D13):
/// - ANTS-20: **if no component is kept** (an empty mask, or every component smaller than
///   `smallest`) the largest size is 0 and ANTs makes **every voxel 1**. Corrected (the
///   default): every voxel 0.
/// - ITK-03: ITK numbers the components in a `float` image, so from the 2^24-th on they all
///   get the label 2^24 and ANTs counts them as one component. Corrected: each its own.
pub fn largest_component(
    input: VolumeRef<'_, f32>,
    smallest: u64,
    n_threads: usize,
    mode: UpstreamDefects,
) -> Result<(Vec<f32>, Vec<Notice>), FilterError> {
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
    let mut notices = Notices::new(mode);
    // `Clusters`: each kept component's size, `unsigned int` stored as float.
    let (sizes, merged) = cluster_sizes(&relabel.sizes, FLOAT_LABELS, mode);
    if merged > 0 {
        notices.add(
            &defects::ITK_03,
            if mode.fix() {
                format!(
                    "{} components are told apart; ITK's float labels count the {merged} from \
                     the 2^24-th on as one",
                    relabel.sizes.len()
                )
            } else {
                format!(
                    "the {merged} components from the 2^24-th on share one float label and are \
                     counted as one, as in ANTs"
                )
            },
        );
    }
    let size_of: Vec<f32> = std::iter::once(0.0).chain(sizes).collect();
    let largest = size_of
        .iter()
        .copied()
        .fold(0.0f32, |m, s| if s > m { s } else { m });
    if largest == 0.0 && !cc.labels.is_empty() {
        let why = if cc.count == 0 {
            "the image has no voxel in [0.25, 1e9]".to_owned()
        } else {
            format!("no component has {smallest} voxels or more")
        };
        if mode.fix() {
            notices.add(
                &defects::ANTS_20,
                format!("{why}: every voxel is 0; ANTs makes every voxel 1"),
            );
            return Ok((vec![0.0; cc.labels.len()], notices.into_vec()));
        }
        notices.add(
            &defects::ANTS_20,
            format!("{why}: every voxel becomes 1, as in ANTs"),
        );
    }
    let out = parallel::with_threads(n_threads, || {
        cc.labels
            .par_iter()
            .with_min_len(CHUNK)
            .map(|&l| {
                let k = relabel.map[l as usize] as usize;
                if size_of[k] >= largest { 1.0 } else { 0.0 }
            })
            .collect()
    })?;
    Ok((out, notices.into_vec()))
}

/// The number of components ITK's float labels tell apart: from the 2^24-th on,
/// `label + 1` no longer grows ([`super::morphology::float_labels`]).
const FLOAT_LABELS: usize = 1 << 24;

/// The size ANTs gives each kept component (`histogram[label]` stored as float): its own
/// size, except with [`UpstreamDefects::Reproduce`] the components from the `saturation`-th
/// on, which share one float label and so the sum of their sizes (ITK-03). Returns the sizes
/// and how many components shared the label (0 when none did).
fn cluster_sizes(sizes: &[u64], saturation: usize, mode: UpstreamDefects) -> (Vec<f32>, usize) {
    let own = |s: u64| s as u32 as f32;
    if sizes.len() <= saturation {
        return (sizes.iter().map(|&s| own(s)).collect(), 0);
    }
    let merged = sizes.len() - saturation + 1;
    if mode.fix() {
        return (sizes.iter().map(|&s| own(s)).collect(), merged);
    }
    // `histogram` is `std::vector<unsigned int>`: the shared count wraps at 2^32.
    let shared = sizes[saturation - 1..]
        .iter()
        .fold(0u32, |acc, &s| acc.wrapping_add(s as u32));
    let out = sizes
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            if i + 1 >= saturation {
                shared as f32
            } else {
                own(s)
            }
        })
        .collect();
    (out, merged)
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

/// x86-64's conversion of a float to `unsigned long` as GCC emits it: below 2^63 a signed
/// 64-bit truncation (so `-1.5` becomes `2^64 - 1`), from 2^63 on the truncation of
/// `v - 2^63` with the top bit set; NaN and values out of range give the integer
/// indefinite (`2^63`, which the second branch turns into 0).
pub fn x86_f32_to_u64(v: f32) -> u64 {
    const TWO63: f32 = 9.223_372e18;
    let cvt = |x: f32| -> i64 {
        if x.is_nan() || x >= TWO63 || x < -TWO63 {
            i64::MIN
        } else {
            x as i64
        }
    };
    if v < TWO63 {
        cvt(v) as u64
    } else {
        (cvt(v - TWO63) as u64) ^ (1 << 63)
    }
}

/// `ExtractContours`: `ImageMath d out ExtractContours image [fullyConnected=1]`: ITK's
/// `LabelContourImageFilter<float, float>` with background 0: voxels whose label is not the
/// background and that touch a voxel with another label (through a face, or anywhere in the
/// `3^D` neighbourhood when fully connected) keep their label, as a float; the others are 0.
///
/// **ANTS-23.** ITK's filter takes each voxel's label as its value converted to `unsigned
/// long` (`static_cast`: 1.7 is 1, 0.6 is 0), so 1.2 and 1.7 are one label with no contour
/// between them, a value below 1 is background, and a contour voxel holds the truncated
/// value ([`UpstreamDefects::Reproduce`]). Corrected (the default): **the labels are the
/// values themselves**, every distinct value its own label (`-0.0` is the background 0, every
/// NaN one label), and a contour voxel keeps its value; named where the two differ.
///
/// A negative or NaN value has no `unsigned long` (ANTS-06): x86-64 builds of ANTs wrap a
/// negative value to near 2^64 ([`x86_f32_to_u64`]), named in both modes; the corrected
/// labels need no conversion.
pub fn extract_contours(
    input: VolumeRef<'_, f32>,
    fully_connected: bool,
    n_threads: usize,
    mode: UpstreamDefects,
) -> Result<(Vec<f32>, Vec<Notice>), FilterError> {
    let undefined = |v: f32| !(v > -1.0 && v < 18_446_744_073_709_551_616.0);
    // ITK's labels: the truncated values (wrapped as x86-64 converts them).
    let (labels, wrapped): (Vec<u64>, usize) = parallel::with_threads(n_threads, || {
        let labels = input
            .data
            .par_iter()
            .with_min_len(CHUNK)
            .map(|&v| x86_f32_to_u64(v))
            .collect();
        let wrapped = input
            .data
            .par_iter()
            .with_min_len(CHUNK)
            .filter(|&&v| undefined(v))
            .count();
        (labels, wrapped)
    })?;
    let contour = label_contour(&labels, input.size, fully_connected, 0, n_threads)?;
    let ants: Vec<f32> = parallel::with_threads(n_threads, || {
        labels
            .par_iter()
            .zip(contour.par_iter())
            .with_min_len(CHUNK)
            .map(|(&l, &c)| if c { l as f32 } else { 0.0 })
            .collect()
    })?;
    drop((labels, contour));
    // The corrected labels: each value its own label, by its bits (0 for ±0, one for NaN).
    let keys: Vec<u64> = parallel::with_threads(n_threads, || {
        input
            .data
            .par_iter()
            .with_min_len(CHUNK)
            .map(|&v| value_label(v))
            .collect()
    })?;
    let contour = label_contour(&keys, input.size, fully_connected, 0, n_threads)?;
    drop(keys);
    let fixed: Vec<f32> = parallel::with_threads(n_threads, || {
        input
            .data
            .par_iter()
            .zip(contour.par_iter())
            .with_min_len(CHUNK)
            .map(|(&v, &c)| if c { v } else { 0.0 })
            .collect()
    })?;
    let changed = ants
        .iter()
        .zip(&fixed)
        .filter(|(a, f)| a.to_bits() != f.to_bits() && !(a.is_nan() && f.is_nan()))
        .count();
    let mut notices = Notices::new(mode);
    if changed > 0 {
        notices.add(
            &defects::ANTS_23,
            if mode.fix() {
                format!(
                    "the labels are the values themselves: {changed} voxel(s) differ from ANTs', \
                     which labels each voxel by its value truncated to an unsigned integer (1.2 \
                     and 1.7 one label, values below 1 background)"
                )
            } else {
                format!(
                    "the labels are the values truncated to unsigned integers, as in ANTs: \
                     {changed} voxel(s) differ from the contours of the values themselves"
                )
            },
        );
    }
    if wrapped > 0 {
        notices.add(
            &defects::ANTS_06,
            if mode.fix() {
                format!(
                    "{wrapped} value(s) are negative, NaN or beyond 2^64: they are labels like \
                     any other value; ANTs on x86-64 wraps their truncated labels around"
                )
            } else {
                format!(
                    "{wrapped} value(s) are negative, NaN or beyond 2^64: their labels wrap \
                     around as ANTs on x86-64 converts them"
                )
            },
        );
    }
    Ok((if mode.fix() { fixed } else { ants }, notices.into_vec()))
}

/// The corrected `ExtractContours` label of a value: 0 for `±0` (the background), one label
/// for every NaN, otherwise the value's bits (plus one, so that no value is the background).
fn value_label(v: f32) -> u64 {
    if v == 0.0 {
        0
    } else if v.is_nan() {
        u64::from(f32::NAN.to_bits()) + 1
    } else {
        u64::from(v.to_bits()) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: UpstreamDefects = UpstreamDefects::Reproduce;
    const F: UpstreamDefects = UpstreamDefects::Fix;

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
        for mode in [R, F] {
            assert_eq!(
                largest_component(v, 0, 1, mode).unwrap(),
                (both.clone(), vec![])
            );
            assert_eq!(largest_component(v, 3, 2, mode).unwrap().0, both);
            let tie = [1.0f32, 1.0, 0.0, 1.0, 1.0];
            let out = largest_component(VolumeRef::new(&tie, &[5], &sp), 0, 1, mode).unwrap();
            assert_eq!(out.0, vec![1., 1., 0., 1., 1.]);
        }
        // Every component below the minimum size (ANTS-20): ANTs makes everything 1,
        // larmorx 0.
        let (out, notices) = largest_component(v, 4, 1, R).unwrap();
        assert!(out.iter().all(|&o| o == 1.0));
        assert_eq!(notices[0].defect.id, "ANTS-20");
        let (out, notices) = largest_component(v, 4, 1, F).unwrap();
        assert!(out.iter().all(|&o| o == 0.0));
        assert_eq!(notices[0].defect.id, "ANTS-20");
        // An empty image too.
        let empty = [0.0f32; 4];
        let (out, _) = largest_component(VolumeRef::new(&empty, &[4], &sp), 0, 1, R).unwrap();
        assert_eq!(out, [1.0; 4]);
        let (out, _) = largest_component(VolumeRef::new(&empty, &[4], &sp), 0, 1, F).unwrap();
        assert_eq!(out, [0.0; 4]);
    }

    #[test]
    fn float_labels_merge_the_last_components() {
        // ITK-03 with a saturation of 3 labels instead of 2^24: the components from the third
        // on share a label, and so their sizes.
        let sizes = [5u64, 3, 2, 2, 2];
        assert_eq!(
            cluster_sizes(&sizes, 3, F),
            (vec![5.0, 3.0, 2.0, 2.0, 2.0], 3)
        );
        assert_eq!(
            cluster_sizes(&sizes, 3, R),
            (vec![5.0, 3.0, 6.0, 6.0, 6.0], 3)
        );
        assert_eq!(
            cluster_sizes(&sizes, 5, R),
            (vec![5.0, 3.0, 2.0, 2.0, 2.0], 0)
        );
    }

    #[test]
    fn contours_use_truncated_labels() {
        assert_eq!(x86_f32_to_u64(1.7), 1);
        assert_eq!(x86_f32_to_u64(-1.5), u64::MAX);
        assert_eq!(x86_f32_to_u64(1.0e19), 9_999_999_980_506_447_872);
        // Integer labels: both modes agree, nothing named.
        let data = [0.0f32, 1.0, 1.0, 1.0, 2.0, 2.0, 0.0];
        for mode in [R, F] {
            let out = extract_contours(VolumeRef::new(&data, &[7], &[1.0]), true, 1, mode);
            assert_eq!(out.unwrap(), (vec![0., 1., 0., 1., 2., 2., 0.], vec![]));
        }
        // ANTS-23: 1.2 and 1.7 are both label 1 in ANTs (no contour between them), 0.6 is
        // background; corrected, each value is its own label and keeps its value.
        let data = [
            0.0f32, 1.2, 1.2, 1.7, 1.7, 2.0, 2.0, 0.0, 0.6, 0.6, 0.6, 0.0,
        ];
        let v = VolumeRef::new(&data, &[12], &[1.0]);
        let (ants, notices) = extract_contours(v, true, 1, R).unwrap();
        assert_eq!(ants, [0., 1., 0., 0., 1., 2., 2., 0., 0., 0., 0., 0.]);
        assert_eq!(notices[0].defect.id, "ANTS-23");
        let (fixed, notices) = extract_contours(v, true, 1, F).unwrap();
        assert_eq!(
            fixed,
            [0., 1.2, 1.2, 1.7, 1.7, 2., 2., 0., 0.6, 0., 0.6, 0.]
        );
        assert_eq!(notices[0].defect.id, "ANTS-23");
        // ANTS-06: a negative value wraps to near 2^64 in ANTs; corrected, it is a label.
        let data = [0.0f32, -1.5, -1.5, 0.0];
        let (out, notices) =
            extract_contours(VolumeRef::new(&data, &[4], &[1.0]), true, 1, R).unwrap();
        assert_eq!(out[1], u64::MAX as f32);
        assert_eq!(notices[1].defect.id, "ANTS-06");
        let (out, notices) =
            extract_contours(VolumeRef::new(&data, &[4], &[1.0]), true, 1, F).unwrap();
        assert_eq!(out, [0.0, -1.5, -1.5, 0.0]);
        assert_eq!(notices[0].defect.id, "ANTS-23");
        assert_eq!(notices[1].defect.id, "ANTS-06");
        // -0.0 is the background, and every NaN one label.
        let data = [-0.0f32, 3.0, f32::NAN, f32::NAN, 0.0];
        let (out, _) = extract_contours(VolumeRef::new(&data, &[5], &[1.0]), true, 1, F).unwrap();
        assert_eq!(out[..2], [0.0, 3.0]);
        assert!(out[2].is_nan() && out[3].is_nan());
        assert_eq!(out[4], 0.0);
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
