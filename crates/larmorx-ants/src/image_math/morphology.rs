// SPDX-License-Identifier: Apache-2.0
//! ImageMath's morphology and mask operations (ANTs v2.6.5):
//! - `MD`, `ME`, `MO`, `MC`, `GD`, `GE`, `GO`, `GC`: [`morphological`], ANTs'
//!   `ants::Morphological` (`Examples/antsUtilities.h:72-250`) on `float` images, with the
//!   ITK filters of [`larmorx_image::morphology`];
//! - `FillHoles`: [`fill_holes`] (`ImageMath_Templates.hxx:8880-9035`), with the connected
//!   components of [`larmorx_image::components`];
//! - `PadImage`: [`pad_image`] (`ImageMath_Templates.hxx:1955-2053`).

use larmorx_core::parallel;
use larmorx_image::components::{connected_components, relabel_components};
use larmorx_image::itk_math::almost_equal_f32;
use larmorx_image::morphology::{
    binary_closing, binary_dilate, binary_erode, binary_opening, grayscale_closing,
    grayscale_closing_owned, grayscale_dilate, grayscale_dilate_owned, grayscale_erode,
    grayscale_erode_owned, grayscale_opening, grayscale_opening_owned,
};
use larmorx_image::{FilterError, VolumeRef, strides};
use larmorx_io::nifti::itk::ItkGeometry;
use rayon::prelude::*;

use crate::image::{AntsImage, x86_to_i64};
use larmorx_core::defects::{self, Notice, Notices, UpstreamDefects};

const CHUNK: usize = 1 << 16;

/// The operations of `ants::Morphological` (its `option` 0 to 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Morphology {
    /// `ME` (0): binary erosion, then `1` where both the eroded and the input value exceed
    /// 0.5, else `0`.
    BinaryErode,
    /// `MD` (1): binary dilation.
    BinaryDilate,
    /// `MO` (2): binary opening.
    BinaryOpen,
    /// `MC` (3): binary closing.
    BinaryClose,
    /// `GE` (4): grayscale erosion.
    GrayscaleErode,
    /// `GD` (5): grayscale dilation.
    GrayscaleDilate,
    /// `GO` (6): grayscale opening.
    GrayscaleOpen,
    /// `GC` (7): grayscale closing.
    GrayscaleClose,
}

impl Morphology {
    /// The operation of ImageMath's name (`MD`, `ME`, `MO`, `MC`, `GD`, `GE`, `GO`, `GC`).
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "ME" => Morphology::BinaryErode,
            "MD" => Morphology::BinaryDilate,
            "MO" => Morphology::BinaryOpen,
            "MC" => Morphology::BinaryClose,
            "GE" => Morphology::GrayscaleErode,
            "GD" => Morphology::GrayscaleDilate,
            "GO" => Morphology::GrayscaleOpen,
            "GC" => Morphology::GrayscaleClose,
            _ => return None,
        })
    }

    /// Whether this is one of the binary operations (which use the foreground value).
    pub fn is_binary(self) -> bool {
        matches!(
            self,
            Morphology::BinaryErode
                | Morphology::BinaryDilate
                | Morphology::BinaryOpen
                | Morphology::BinaryClose
        )
    }
}

/// `ants::Morphological(input, radius, option, value)` on a `float` image: the operation with
/// ITK's `BinaryBallStructuringElement` of `radius` voxels on every axis (in all of the
/// image's dimensions: a 4D image along time too).
///
/// The binary operations use `value` (ImageMath's optional argument, default 1) as the
/// foreground; other values are background. ITK's defaults stay: the dilation's background
/// value is `-f32::MAX` (it never shows), the erosion's too. The grayscale operations ignore
/// `value`. Two defects (ANTS-17, D13), each named where the correction changes the result:
/// - `ME`: ANTs keeps `1` where the eroded value and the input both exceed 0.5, `0`
///   elsewhere: a voxel that is not the foreground but above 0.5 (a label 2 in a mask of 1s)
///   stays 1, and a remaining foreground of 0.5 or less (`ME r 0.3`, `ME r 0`) becomes 0.
///   Corrected ([`UpstreamDefects::Fix`], the default): the eroded foreground as a mask, `1`
///   where the voxel is still the foreground after the erosion, `0` elsewhere. On a 0/1 mask
///   with the foreground 1 (every pipeline's call) both give the same result.
/// - `MO`: ITK's opening writes the voxels its erosion removes as its background value, 0,
///   which ANTs never changes; with the foreground 0 (`MO r 0`) the erosion then removes
///   nothing and the opening is a dilation of the zeros. Corrected: with the foreground 0 the
///   removed voxels become 1 (for a 0/1 mask, the opening of its zeros: by duality, the
///   closing of its ones); any other foreground keeps ITK's 0.
pub fn morphological(
    input: VolumeRef<'_, f32>,
    operation: Morphology,
    radius: usize,
    value: f32,
    n_threads: usize,
    mode: UpstreamDefects,
) -> Result<(Vec<f32>, Vec<Notice>), FilterError> {
    let lowest = -f32::MAX;
    let mut notices = Notices::new(mode);
    let out = match operation {
        Morphology::BinaryDilate => binary_dilate(input, radius, value, lowest, n_threads)?,
        Morphology::BinaryErode => {
            let mut eroded = binary_erode(input, radius, value, lowest, n_threads)?;
            // ANTs' value and the corrected one, counted and the mode's written in place in
            // one pass (random testing, 2026-10-25: both images were made, 1.5 times ANTs'
            // peak memory).
            let fix = mode.fix();
            let (other, lost) = parallel::with_threads(n_threads, || {
                eroded
                    .par_iter_mut()
                    .zip(input.data.par_iter())
                    .with_min_len(CHUNK)
                    .map(|(e, &v)| {
                        let ants = if *e > 0.5 && v > 0.5 { 1.0 } else { 0.0 };
                        let fixed = if *e == value { 1.0 } else { 0.0 };
                        *e = if fix { fixed } else { ants };
                        (usize::from(ants > fixed), usize::from(ants < fixed))
                    })
                    .reduce(|| (0, 0), |(a, b), (c, d)| (a + c, b + d))
            })?;
            if other + lost > 0 {
                let what = format!(
                    "{other} voxel(s) that are not the foreground {value} but above 0.5, and \
                     {lost} voxel(s) of the eroded foreground at or below 0.5"
                );
                notices.add(
                    &defects::ANTS_17,
                    if mode.fix() {
                        format!(
                            "ME gives the eroded foreground as a mask: {what} are 0 and 1 as the \
                             erosion leaves them; ANTs thresholds at 0.5 and makes them 1 and 0"
                        )
                    } else {
                        format!(
                            "ME thresholds at 0.5 as ANTs does: {what} are 1 and 0, not as the \
                             erosion leaves them"
                        )
                    },
                );
            }
            eroded
        }
        Morphology::BinaryOpen if value == 0.0 => {
            // ITK's opening background (0) is the foreground itself (ANTS-17).
            let ants = binary_opening(input, radius, value, 0.0, n_threads)?;
            let fixed = binary_opening(input, radius, value, 1.0, n_threads)?;
            let changed = ants
                .iter()
                .zip(&fixed)
                .filter(|(a, f)| a.to_bits() != f.to_bits())
                .count();
            if changed > 0 {
                notices.add(
                    &defects::ANTS_17,
                    if mode.fix() {
                        format!(
                            "MO with the foreground 0 opens the zeros: the {changed} voxel(s) its \
                             erosion removes and its dilation does not restore become 1; ANTs \
                             sets them to 0, the foreground itself, so it only dilates the zeros"
                        )
                    } else {
                        format!(
                            "MO with the foreground 0 sets the voxels its erosion removes to 0, \
                             the foreground itself, as ANTs does: it dilates the zeros ({changed} \
                             voxel(s) differ from the opening)"
                        )
                    },
                );
            }
            if mode.fix() { fixed } else { ants }
        }
        Morphology::BinaryOpen => binary_opening(input, radius, value, 0.0, n_threads)?,
        Morphology::BinaryClose => binary_closing(input, radius, value, n_threads)?,
        Morphology::GrayscaleErode => grayscale_erode(input, radius, n_threads)?,
        Morphology::GrayscaleDilate => grayscale_dilate(input, radius, n_threads)?,
        Morphology::GrayscaleOpen => grayscale_opening(input, radius, n_threads)?,
        Morphology::GrayscaleClose => grayscale_closing(input, radius, n_threads)?,
    };
    Ok((out, notices.into_vec()))
}

/// A grayscale operation (`GD`, `GE`, `GO`, `GC`) of an image given whole: its buffer becomes
/// the output, so the operation needs two images of memory, as ITK's filters (random testing,
/// 2026-10-25: GE needed 2.4 times ANTs' peak). The same values as [`morphological`].
pub fn grayscale_morphological_owned(
    data: Vec<f32>,
    size: &[usize],
    operation: Morphology,
    radius: usize,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    match operation {
        Morphology::GrayscaleErode => grayscale_erode_owned(data, size, radius, n_threads),
        Morphology::GrayscaleDilate => grayscale_dilate_owned(data, size, radius, n_threads),
        Morphology::GrayscaleOpen => grayscale_opening_owned(data, size, radius, n_threads),
        Morphology::GrayscaleClose => grayscale_closing_owned(data, size, radius, n_threads),
        _ => Err(FilterError::Invalid(
            "a binary operation needs the input image after it".into(),
        )),
    }
}

/// The radius ANTs uses for ImageMath's float argument: `static_cast<unsigned long>(rad)`,
/// truncation toward zero (`-0.5` is 0). `None` where that is not a usable radius (NaN,
/// `≤ −1`, which x86-64 turns into a radius near `2^64`, or above `max`).
pub fn radius_from_f32(rad: f32, max: usize) -> Option<usize> {
    if rad.is_nan() || rad <= -1.0 || f64::from(rad) >= max as f64 + 1.0 {
        return None;
    }
    Some(rad.max(0.0) as usize)
}

/// Why [`fill_holes`] could not reproduce ANTs.
#[derive(Debug, thiserror::Error)]
pub enum FillHolesError {
    #[error(transparent)]
    Filter(#[from] FilterError),
    /// More components than ANTs' float labels tell apart.
    #[error("{0} components: ANTs' float labels no longer tell neighbouring labels apart")]
    TooManyComponents(usize),
}

/// `FillHoles`: `ImageMath d out FillHoles image [holeparam]` on a `float` image; returns the
/// image with the holes filled (set to 1), the other voxels unchanged.
///
/// The object is the voxels in `[0.5, 1e9]`. The background is split into face-connected
/// components (ANTs finds it with a Danielsson distance map thresholded at 0.001, which is
/// exactly the voxels outside the object) and relabelled by size; the largest (label 1,
/// the first in memory among equally large ones) is the outside, the others are holes.
/// - `hole_param` within `FloatAlmostEqual` of 2 (ANTs' default): every hole is filled.
/// - `hole_param ≤ 1`: a hole is filled when the fraction of its neighbours (over the 3^D
///   neighbourhood of each of its voxels, counted with repeats) that are object voxels exceeds
///   `hole_param`. ANTs reads the neighbours of voxels on the image's faces at indices outside
///   the image (ANTS-19): they land on other rows of the image, or outside its memory, where
///   the result depends on whatever is there. With [`UpstreamDefects::Fix`] (the default) a
///   neighbour outside the image is not counted. With [`UpstreamDefects::Reproduce`] the
///   reads that land inside the image's memory are ANTs', and those outside it read as 0
///   (background), with a notice.
/// - otherwise: every hole if `hole_param < 2`, none if above.
pub fn fill_holes(
    input: VolumeRef<'_, f32>,
    hole_param: f32,
    n_threads: usize,
    mode: UpstreamDefects,
) -> Result<(Vec<f32>, Vec<Notice>), FillHolesError> {
    let mut notices = Notices::new(mode);
    let size = input.size;
    let (low, high) = (0.5f32, 1.0e9f32);
    let object: Vec<bool> = parallel::with_threads(n_threads, || {
        input
            .data
            .par_iter()
            .with_min_len(CHUNK)
            .map(|&v| low <= v && v <= high)
            .collect()
    })
    .map_err(FilterError::from)?;
    let background: Vec<bool> = object.iter().map(|&o| !o).collect();
    let cc = connected_components(&background, size, false, n_threads)?;
    let relabel = relabel_components(&cc.labels, cc.count, 0, n_threads)?;
    // The relabelled image as ITK writes it: float labels counted in float.
    let float_label = float_labels(relabel.sizes.len());
    let label_of = |i: usize| -> f32 {
        match relabel.map[cc.labels[i] as usize] {
            0 => 0.0,
            k => float_label[k as usize - 1],
        }
    };
    let mut out = input.data.to_vec();
    if almost_equal_f32(hole_param, 2.0) {
        parallel::with_threads(n_threads, || {
            out.par_iter_mut()
                .enumerate()
                .with_min_len(CHUNK)
                .for_each(|(i, o)| {
                    if label_of(i) > 1.0 {
                        *o = 1.0;
                    }
                });
        })
        .map_err(FilterError::from)?;
        return Ok((out, notices.into_vec()));
    }
    let objects = relabel.sizes.len();
    if objects > 1 << 21 {
        return Err(FillHolesError::TooManyComponents(objects));
    }
    // Which labels to fill (labels 2..=objects).
    let mut fill = vec![false; objects + 1];
    if hole_param <= 1.0 {
        let counts = |mode| edge_counts(&object, size, &relabel.map, &cc.labels, objects, mode);
        let ants = counts(UpstreamDefects::Reproduce);
        let corrected = counts(UpstreamDefects::Fix);
        let decide = |e: &EdgeCounts| -> Vec<bool> {
            (0..=objects)
                .map(|lab| lab >= 2 && e.object[lab] as f32 / e.total[lab] as f32 > hole_param)
                .collect()
        };
        let (ants_fill, fixed_fill) = (decide(&ants), decide(&corrected));
        let changed = (2..=objects)
            .filter(|&l| ants_fill[l] != fixed_fill[l])
            .count();
        // Named where the correction changes a decision, or where ANTs' own decision depends
        // on memory outside the image.
        if changed > 0 || ants.outside_memory > 0 {
            let decisions = if changed > 0 {
                format!("; the correction changes the decision for {changed} hole(s)")
            } else {
                String::new()
            };
            let detail = if mode.fix() {
                format!(
                    "{} neighbour(s) of the holes lie outside the image and are not counted; \
                     ANTs reads them at other rows of the image or outside its memory{decisions}",
                    ants.outside_image
                )
            } else {
                format!(
                    "{} neighbour(s) of the holes lie outside the image and are read as ANTs \
                     reads them; {} of those reads land outside the image's memory, whose \
                     values depend on memory: here 0{decisions}",
                    ants.outside_image, ants.outside_memory
                )
            };
            notices.add(&defects::ANTS_19, detail);
        }
        fill = if mode.fix() { fixed_fill } else { ants_fill };
    } else {
        for f in fill.iter_mut().skip(2) {
            *f = 2.0 > hole_param;
        }
    }
    for (i, o) in out.iter_mut().enumerate() {
        let k = relabel.map[cc.labels[i] as usize] as usize;
        if k >= 2 && fill[k] {
            *o = 1.0;
        }
    }
    Ok((out, notices.into_vec()))
}

/// The labels `RelabelComponentImageFilter<_, float>` writes for `n` objects: ITK counts
/// them in a `float` (`outputLabel + 1`, then `++outputLabel`), exact up to 2^24.
pub fn float_labels(n: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(n);
    let mut label = 0.0f32;
    for _ in 0..n {
        out.push(label + 1.0);
        label += 1.0;
    }
    out
}

/// FillHoles' `objectedge` and `totaledge` for every relabelled component, and how many
/// neighbour reads fall outside the image and outside its memory.
struct EdgeCounts {
    object: Vec<u64>,
    total: Vec<u64>,
    outside_image: u64,
    outside_memory: u64,
}

/// FillHoles' edge counts (the `holeparam ≤ 1` branch, `ImageMath_Templates.hxx:8978-9014`):
/// ANTs' with [`UpstreamDefects::Reproduce`] (neighbours outside the image read at their
/// unclamped offset, 0 beyond the image's memory), larmorx's with [`UpstreamDefects::Fix`]
/// (neighbours outside the image not counted).
fn edge_counts(
    object: &[bool],
    size: &[usize],
    map: &[u32],
    labels: &[u32],
    objects: usize,
    mode: UpstreamDefects,
) -> EdgeCounts {
    let d = size.len();
    let st = strides(size);
    let n = object.len() as isize;
    let mut object_edge = vec![0u64; objects + 1];
    let mut total_edge = vec![0u64; objects + 1];
    let (mut outside_image, mut outside_memory) = (0u64, 0u64);
    let relabelled = |i: usize| map[labels[i] as usize] as usize;
    let count = 3usize.pow(d as u32);
    let mut coord = vec![0usize; d];
    for c in 0..object.len() {
        if c > 0 {
            next_index(&mut coord, size);
        }
        let lab = relabelled(c);
        if lab < 2 {
            continue;
        }
        for mut t in 0..count {
            // The neighbour's index (unclamped) and, for its label, the nearest voxel of the
            // image (`ZeroFluxNeumannBoundaryCondition`).
            let mut clamped = 0usize;
            let mut linear = 0isize;
            let mut inside = true;
            for a in 0..d {
                let o = (t % 3) as isize - 1;
                t /= 3;
                let p = coord[a] as isize + o;
                linear += p * st[a] as isize;
                clamped += p.clamp(0, size[a] as isize - 1) as usize * st[a];
                inside &= p >= 0 && p < size[a] as isize;
            }
            if relabelled(clamped) == lab {
                continue;
            }
            if !inside {
                outside_image += 1;
                if mode.fix() {
                    continue;
                }
            }
            total_edge[lab] += 1;
            if linear < 0 || linear >= n {
                // Outside the image's memory: undefined in ANTs, background here.
                outside_memory += 1;
                continue;
            }
            if object[linear as usize] {
                object_edge[lab] += 1;
            }
        }
    }
    EdgeCounts {
        object: object_edge,
        total: total_edge,
        outside_image,
        outside_memory,
    }
}

/// Why [`pad_image`] could not run.
#[derive(Debug, thiserror::Error)]
pub enum PadImageError {
    /// The new size is 0 or wraps around (ANTs then fails to allocate the image).
    #[error("PadImage: padding by {pad} gives an axis of {size} voxels")]
    BadSize { pad: f32, size: u64 },
}

/// `PadImage`: `ImageMath d out PadImage image pad [value]`: `|pad|` voxels (truncated) added
/// on both sides of every axis (removed for a negative `pad`), the new voxels `value`. The
/// origin moves so that the voxels keep their positions in space, computed as ITK's
/// `TransformIndexToPhysicalPoint` does. The output is a new image (no `descrip`).
///
/// ANTs computes each new size as `(unsigned int)(float(size) + pad·2)` but places voxel `i`
/// at `(unsigned int)(float(i) + pad)`: so a non-integer `pad` (2.5) grows an axis by 5 but
/// shifts it by 2, two voxels before and three after (ANTS-22, [`UpstreamDefects::Reproduce`]).
/// Corrected (the default): a pad is a whole number of voxels on each side, `trunc(pad)`, the
/// same before and after (the shift ANTs applies), so 2.5 adds 2 voxels on each side; named
/// where ANTs' size differs. Returns the padded image and the notices.
pub fn pad_image(
    image: &AntsImage<f32>,
    pad: f32,
    value: f32,
    mode: UpstreamDefects,
) -> Result<(AntsImage<f32>, Vec<Notice>), PadImageError> {
    let g = &image.geometry;
    let d = g.ndim;
    let mut notices = Notices::new(mode);
    // The voxels added on each side (truncated toward zero; a removal if negative).
    let side = x86_to_i64(f64::from(pad));
    let mut new_size = Vec::with_capacity(d);
    let mut uneven: Option<(usize, u64, u64)> = None;
    for &n in &g.size {
        let s = n as f32 + pad * 2.0;
        // GCC converts float to `unsigned int` through a 64-bit truncation.
        let ants = u64::from(x86_to_i64(f64::from(s)) as u32);
        let even = (n as i64)
            .checked_add(side.saturating_mul(2))
            .filter(|&e| e > 0)
            .map_or(0, |e| e as u64);
        if ants != even && uneven.is_none() {
            uneven = Some((n, ants, even));
        }
        let s = if mode.fix() { even } else { ants };
        if s == 0 || s > 1 << 31 {
            return Err(PadImageError::BadSize { pad, size: s });
        }
        new_size.push(s as usize);
    }
    // For a positive pad the sizes alone differ (the image shifts by `side` in both); for a
    // negative fractional pad ANTs also moves the voxels by one more than the origin.
    if let Some((n, ants, even)) = uneven {
        let sides = side.unsigned_abs();
        let verb = if pad < 0.0 { "removes" } else { "adds" };
        let shift = if pad < 0.0 {
            "its voxels move by one more than its origin, so the image also shifts by a voxel \
             in space, and the last row of every axis is the pad value"
        } else {
            "the extra voxel goes at the end"
        };
        notices.add(
            &defects::ANTS_22,
            if mode.fix() {
                format!(
                    "a pad of {pad} {verb} {sides} voxel(s) on each side of every axis (an axis \
                     of {n} voxels has {even}); ANTs makes it {ants} and its padding uneven: \
                     {shift}"
                )
            } else {
                format!(
                    "a pad of {pad} makes an axis of {n} voxels {ants}, as ANTs does: the padding \
                     is uneven ({shift}); {sides} voxel(s) on each side would give {even}"
                )
            },
        );
    }
    // `index` in the input and `index2` in the padded image share a position in space.
    let shift = x86_to_i64(f64::from(pad.abs())) as u32 as i64;
    let (index, index2) = if pad > 0.0 {
        (vec![0i64; d], vec![shift; d])
    } else {
        (vec![shift; d], vec![0i64; d])
    };
    let m = index_to_physical(g);
    let point = |idx: &[i64]| -> Vec<f64> {
        (0..d)
            .map(|i| {
                let mut p = g.origin[i];
                for j in 0..d {
                    p += m[i][j] * idx[j] as f64;
                }
                p
            })
            .collect()
    };
    let (point1, point_pad) = (point(&index), point(&index2));
    let origin2: Vec<f64> = (0..d)
        .map(|i| g.origin[i] + (point1[i] - point_pad[i]))
        .collect();
    // `SetOrigin` changes the origin only if it compares unequal.
    let origin = if origin2.iter().zip(&g.origin).all(|(a, b)| a == b) {
        g.origin.clone()
    } else {
        origin2
    };
    let geometry = ItkGeometry {
        ndim: d,
        size: new_size.clone(),
        spacing: g.spacing.clone(),
        origin,
        direction: g.direction.clone(),
        source: g.source,
        flipped: g.flipped.clone(),
    };
    let total: usize = new_size.iter().product();
    let mut data = vec![value; total];
    let out_strides = strides(&new_size);
    let mut coord = vec![0usize; d];
    'voxels: for (i, &v) in image.data.iter().enumerate() {
        if i > 0 {
            next_index(&mut coord, &g.size);
        }
        let mut target = 0usize;
        for a in 0..d {
            let index = if mode.fix() {
                // `side` voxels on each side: the shift the origin was moved by.
                let shifted = coord[a] as i64 + side;
                if shifted < 0 || shifted >= new_size[a] as i64 {
                    continue 'voxels;
                }
                shifted as usize
            } else {
                let shifted = coord[a] as f32 + pad;
                if shifted < 0.0 || shifted > (new_size[a] - 1) as f32 {
                    continue 'voxels;
                }
                shifted as usize
            };
            target += index * out_strides[a];
        }
        data[target] = v;
    }
    Ok((
        AntsImage {
            data,
            geometry,
            meta: Default::default(),
        },
        notices.into_vec(),
    ))
}

/// The next index in memory order (first axis fastest).
fn next_index(coord: &mut [usize], size: &[usize]) {
    for (c, &n) in coord.iter_mut().zip(size) {
        *c += 1;
        if *c < n {
            return;
        }
        *c = 0;
    }
}

/// ITK's `m_IndexToPhysicalPoint = m_Direction · diag(spacing)`, summed as vnl's fixed
/// matrix product does (`accum = a(i,0)·b(0,j)`, then `+= a(i,k)·b(k,j)`).
fn index_to_physical(g: &ItkGeometry) -> Vec<Vec<f64>> {
    let d = g.ndim;
    (0..d)
        .map(|i| {
            (0..d)
                .map(|j| {
                    let s = |k: usize| if k == j { g.spacing[j] } else { 0.0 };
                    let mut accum = g.direction[i][0] * s(0);
                    for k in 1..d {
                        accum += g.direction[i][k] * s(k);
                    }
                    accum
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_io::nifti::itk::GeometrySource;

    fn image(size: &[usize], data: Vec<f32>) -> AntsImage<f32> {
        let d = size.len();
        AntsImage {
            data,
            geometry: ItkGeometry {
                ndim: d,
                size: size.to_vec(),
                spacing: vec![2.0; d],
                origin: vec![10.0; d],
                direction: (0..d)
                    .map(|i| (0..d).map(|j| if i == j { -1.0 } else { 0.0 }).collect())
                    .collect(),
                source: GeometrySource::Qform,
                flipped: vec![false; d],
            },
            meta: Default::default(),
        }
    }

    #[test]
    fn padding_moves_the_origin_and_keeps_positions() {
        let (r, f) = (UpstreamDefects::Reproduce, UpstreamDefects::Fix);
        let img = image(&[3, 2], (0..6).map(|v| v as f32 + 1.0).collect());
        for mode in [r, f] {
            let (padded, notices) = pad_image(&img, 2.0, -5.0, mode).unwrap();
            assert!(notices.is_empty());
            assert_eq!(padded.size(), [7, 6]);
            // Voxel (0, 0) moved to (2, 2); the origin moved by -2 voxels · direction · spacing.
            assert_eq!(padded.data[2 + 7 * 2], 1.0);
            assert_eq!(padded.data[0], -5.0);
            assert_eq!(padded.geometry.origin, vec![14.0, 14.0]);
            // Cropping back restores the image.
            let (back, _) = pad_image(&padded, -2.0, 0.0, mode).unwrap();
            assert_eq!(back.data, img.data);
            assert_eq!(back.geometry.origin, img.geometry.origin);
            assert!(pad_image(&img, -1.0, 0.0, mode).is_err());
            // A fraction below one half changes nothing: 1.25 grows by 2, shifts by 1.
            let (a, notices) = pad_image(&img, 1.25, 0.0, mode).unwrap();
            assert_eq!(a.size(), [5, 4]);
            assert_eq!(a.data[1 + 5], 1.0);
            assert!(notices.is_empty());
        }
        // ANTS-22: ANTs grows by 2·pad but shifts by the truncated pad (the extra voxel at
        // the end); corrected, one voxel on each side.
        let (odd, notices) = pad_image(&img, 1.5, 0.0, r).unwrap();
        assert_eq!(odd.size(), [6, 5]);
        assert_eq!(odd.data[1 + 6], 1.0);
        assert_eq!(notices[0].defect.id, "ANTS-22");
        let (even, notices) = pad_image(&img, 1.5, 0.0, f).unwrap();
        assert_eq!(even.size(), [5, 4]);
        assert_eq!(even.data[1 + 5], 1.0);
        assert_eq!(even.geometry.origin, odd.geometry.origin);
        assert_eq!(notices[0].defect.id, "ANTS-22");
        // A negative fractional pad: on an axis of 6 ANTs keeps 3 voxels, moves the voxels by
        // 2 while the origin moves by 1, and leaves the last row the pad value; corrected, 1
        // voxel removed on each side and the voxels where the origin says.
        let big = image(&[6, 5], (0..30).map(|v| v as f32).collect());
        let (ants, _) = pad_image(&big, -1.5, 0.0, r).unwrap();
        assert_eq!(ants.size(), [3, 2]);
        assert_eq!(ants.data, [14.0, 15.0, 0.0, 0.0, 0.0, 0.0]);
        let (fixed, notices) = pad_image(&big, -1.5, 0.0, f).unwrap();
        assert_eq!(fixed.size(), [4, 3]);
        assert_eq!(
            fixed.data,
            [
                7.0, 8.0, 9.0, 10.0, 13.0, 14.0, 15.0, 16.0, 19.0, 20.0, 21.0, 22.0
            ]
        );
        assert_eq!(fixed.geometry.origin, ants.geometry.origin);
        assert!(notices[0].detail.contains("one more than its origin"));
    }

    #[test]
    fn fill_holes_fills_enclosed_background() {
        let size = [7usize, 7, 7];
        let at = |x: usize, y: usize, z: usize| x + 7 * (y + 7 * z);
        let mut data = vec![0.0f32; 343];
        for z in 1..6 {
            for y in 1..6 {
                for x in 1..6 {
                    data[at(x, y, z)] = 1.0;
                }
            }
        }
        data[at(3, 3, 3)] = 0.0;
        data[at(2, 2, 2)] = 3.0;
        let spacing = [1.0; 3];
        let v = VolumeRef::new(&data, &size, &spacing);
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let fill = |p: f32| fill_holes(v, p, 1, mode).unwrap();
            let (filled, notices) = fill_holes(v, 2.0, 2, mode).unwrap();
            assert_eq!(filled[at(3, 3, 3)], 1.0);
            assert_eq!(filled[at(2, 2, 2)], 3.0);
            assert_eq!(filled[at(0, 0, 0)], 0.0);
            assert!(notices.is_empty());
            // The hole is surrounded by object voxels only: its edge ratio is 1.
            assert_eq!(fill(0.99).0[at(3, 3, 3)], 1.0);
            assert_eq!(fill(1.0).0[at(3, 3, 3)], 0.0);
            assert_eq!(fill(2.5).0[at(3, 3, 3)], 0.0);
            assert_eq!(fill(1.5).0[at(3, 3, 3)], 1.0);
            assert!(fill(0.99).1.is_empty());
        }
    }

    #[test]
    fn fill_holes_on_the_first_slice() {
        // ANTS-19: a hole on the image's first slice; its neighbours below the slice are
        // outside the image and, in ANTs, outside its memory.
        let size = [7usize, 7, 3];
        let at = |x: usize, y: usize, z: usize| x + 7 * (y + 7 * z);
        let mut data = vec![0.0f32; 7 * 7 * 3];
        for z in 0..3 {
            for y in 1..6 {
                for x in 1..6 {
                    data[at(x, y, z)] = 1.0;
                }
            }
        }
        data[at(3, 3, 0)] = 0.0;
        let spacing = [1.0; 3];
        let v = VolumeRef::new(&data, &size, &spacing);
        // Corrected: the neighbours outside the image are not counted, the 17 others are
        // object voxels (ratio 1): filled.
        let (fixed, notices) = fill_holes(v, 0.9, 1, UpstreamDefects::Fix).unwrap();
        assert_eq!(fixed[at(3, 3, 0)], 1.0);
        assert_eq!(notices[0].defect.id, "ANTS-19");
        assert!(notices[0].detail.contains("the decision for 1 hole"));
        // Reproduced: 8 of them (the one straight below has the hole's own label through
        // ITK's boundary condition) read outside the memory, here as background (17 / 25):
        // not filled.
        let (ants, notices) = fill_holes(v, 0.9, 1, UpstreamDefects::Reproduce).unwrap();
        assert_eq!(ants[at(3, 3, 0)], 0.0);
        assert!(
            notices[0].detail.contains("8 of those reads land outside"),
            "{notices:?}"
        );
        // Where both decide alike, the outputs agree, but ANTs' reads outside its memory are
        // still named.
        let (a, _) = fill_holes(v, 0.5, 1, UpstreamDefects::Fix).unwrap();
        let (b, notices) = fill_holes(v, 0.5, 1, UpstreamDefects::Reproduce).unwrap();
        assert_eq!(a, b);
        assert_eq!(notices.len(), 1);
        // A hole on the middle slice's x face reads only inside the memory (other rows):
        // named only where the decision changes.
        let mut data = vec![0.0f32; 7 * 7 * 3];
        for z in 0..3 {
            for y in 1..6 {
                for x in 0..6 {
                    data[at(x, y, z)] = 1.0;
                }
            }
        }
        data[at(0, 3, 1)] = 0.0;
        let v = VolumeRef::new(&data, &size, &spacing);
        let (a, na) = fill_holes(v, 0.2, 1, UpstreamDefects::Fix).unwrap();
        let (b, nb) = fill_holes(v, 0.2, 1, UpstreamDefects::Reproduce).unwrap();
        assert_eq!(a, b);
        assert!(na.is_empty() && nb.is_empty(), "{na:?} {nb:?}");
    }

    #[test]
    fn erosion_and_opening_with_labels_and_zero() {
        let (r, f) = (UpstreamDefects::Reproduce, UpstreamDefects::Fix);
        // A line: labels 1 (5 voxels), 2 (3 voxels), and a run of 0.3.
        let data = [
            0.0f32, 1., 1., 1., 1., 1., 0., 2., 2., 2., 0., 0.3, 0.3, 0.3, 0.,
        ];
        let size = [15usize];
        let v = VolumeRef::new(&data, &size, &[1.0]);
        // ANTS-17, ME 1 1: ANTs keeps label 2 as 1; corrected, only the eroded label 1.
        let (ants, n) = morphological(v, Morphology::BinaryErode, 1, 1.0, 1, r).unwrap();
        assert_eq!(&ants[..10], &[0., 0., 1., 1., 1., 0., 0., 1., 1., 1.]);
        assert_eq!(n[0].defect.id, "ANTS-17");
        let (fixed, n) = morphological(v, Morphology::BinaryErode, 1, 1.0, 1, f).unwrap();
        assert_eq!(&fixed[..10], &[0., 0., 1., 1., 1., 0., 0., 0., 0., 0.]);
        assert!(
            n[0].detail
                .contains("3 voxel(s) that are not the foreground"),
            "{n:?}"
        );
        // ME 1 0.3: the eroded run of 0.3 is 0 in ANTs (not above 0.5), 1 corrected.
        let (ants, _) = morphological(v, Morphology::BinaryErode, 1, 0.3, 1, r).unwrap();
        assert_eq!(ants[12], 0.0);
        let (fixed, _) = morphological(v, Morphology::BinaryErode, 1, 0.3, 1, f).unwrap();
        assert_eq!(fixed[12], 1.0);
        assert_eq!(fixed.iter().sum::<f32>(), 1.0);
        // A 0/1 mask: both agree and nothing is named.
        let mask = [0.0f32, 1., 1., 1., 0., 1., 0.];
        let m = VolumeRef::new(&mask, &[7], &[1.0]);
        let a = morphological(m, Morphology::BinaryErode, 1, 1.0, 1, r).unwrap();
        let b = morphological(m, Morphology::BinaryErode, 1, 1.0, 1, f).unwrap();
        assert_eq!(a, b);
        assert!(a.1.is_empty());
        // MO 1 0 on a mask with a one-voxel hole: ANTs dilates the zeros (the hole grows), the
        // correction opens them (the hole, smaller than the ball, is filled with 1).
        let holed = [1.0f32, 1., 1., 0., 1., 1., 1., 1., 0., 0., 0., 0.];
        let h = VolumeRef::new(&holed, &[12], &[1.0]);
        let (ants, n) = morphological(h, Morphology::BinaryOpen, 1, 0.0, 1, r).unwrap();
        assert_eq!(ants, [1., 1., 0., 0., 0., 1., 1., 0., 0., 0., 0., 0.]);
        assert_eq!(n[0].defect.id, "ANTS-17");
        let (fixed, _) = morphological(h, Morphology::BinaryOpen, 1, 0.0, 1, f).unwrap();
        assert_eq!(fixed, [1., 1., 1., 1., 1., 1., 1., 1., 0., 0., 0., 0.]);
        // Any other foreground: ITK's opening in both modes.
        let a = morphological(h, Morphology::BinaryOpen, 1, 1.0, 1, r).unwrap();
        assert_eq!(
            a,
            morphological(h, Morphology::BinaryOpen, 1, 1.0, 1, f).unwrap()
        );
        assert!(a.1.is_empty());
    }

    #[test]
    fn morphology_names_and_radii() {
        for name in ["MD", "ME", "MO", "MC", "GD", "GE", "GO", "GC"] {
            assert!(Morphology::from_name(name).is_some());
        }
        assert!(Morphology::from_name("md").is_none());
        assert_eq!(radius_from_f32(2.7, 100), Some(2));
        assert_eq!(radius_from_f32(-0.5, 100), Some(0));
        assert_eq!(radius_from_f32(-1.0, 100), None);
        assert_eq!(radius_from_f32(f32::NAN, 100), None);
        assert_eq!(radius_from_f32(101.0, 100), None);
    }
}
