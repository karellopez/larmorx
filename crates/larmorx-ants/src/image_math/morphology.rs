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
    grayscale_dilate, grayscale_erode, grayscale_opening,
};
use larmorx_image::{FilterError, VolumeRef, strides};
use larmorx_io::nifti::itk::ItkGeometry;
use rayon::prelude::*;

use crate::image::{AntsImage, x86_to_i64};

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
/// value is `-f32::MAX` (it never shows), the erosion's too, the opening's 0. `ME`
/// then keeps `1` only where the eroded value and the input both exceed 0.5, `0` elsewhere:
/// a voxel that is not the foreground but above 0.5 (a label 2 in a mask of 1s) stays 1.
/// The grayscale operations ignore `value`.
pub fn morphological(
    input: VolumeRef<'_, f32>,
    operation: Morphology,
    radius: usize,
    value: f32,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let lowest = -f32::MAX;
    match operation {
        Morphology::BinaryDilate => binary_dilate(input, radius, value, lowest, n_threads),
        Morphology::BinaryErode => {
            let eroded = binary_erode(input, radius, value, lowest, n_threads)?;
            Ok(parallel::with_threads(n_threads, || {
                eroded
                    .par_iter()
                    .zip(input.data.par_iter())
                    .with_min_len(CHUNK)
                    .map(|(&e, &v)| if e > 0.5 && v > 0.5 { 1.0 } else { 0.0 })
                    .collect()
            })?)
        }
        Morphology::BinaryOpen => binary_opening(input, radius, value, 0.0, n_threads),
        Morphology::BinaryClose => binary_closing(input, radius, value, n_threads),
        Morphology::GrayscaleErode => grayscale_erode(input, radius, n_threads),
        Morphology::GrayscaleDilate => grayscale_dilate(input, radius, n_threads),
        Morphology::GrayscaleOpen => grayscale_opening(input, radius, n_threads),
        Morphology::GrayscaleClose => grayscale_closing(input, radius, n_threads),
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
    /// ANTs reads outside the image's memory here; its result is undefined.
    #[error(
        "the hole of label {label} touches the image's first or last slice: ANTs reads outside \
         the image's memory there (its result is undefined)"
    )]
    OutOfBounds { label: usize },
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
///   the image, which land on other rows of the image (reproduced) or outside its memory (an
///   error here, [`FillHolesError::OutOfBounds`]).
/// - otherwise: every hole if `hole_param < 2`, none if above.
pub fn fill_holes(
    input: VolumeRef<'_, f32>,
    hole_param: f32,
    n_threads: usize,
) -> Result<Vec<f32>, FillHolesError> {
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
        return Ok(out);
    }
    let objects = relabel.sizes.len();
    if objects > 1 << 21 {
        return Err(FillHolesError::TooManyComponents(objects));
    }
    // Which labels to fill (labels 2..=objects).
    let mut fill = vec![false; objects + 1];
    if hole_param <= 1.0 {
        let (object_edge, total_edge) =
            edge_counts(&object, size, &relabel.map, &cc.labels, objects)?;
        for lab in 2..=objects {
            let erat = object_edge[lab] as f32 / total_edge[lab] as f32;
            fill[lab] = erat > hole_param;
        }
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
    Ok(out)
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

/// FillHoles' `objectedge` and `totaledge` for every relabelled component (the
/// `holeparam ≤ 1` branch, `ImageMath_Templates.hxx:8978-9014`).
fn edge_counts(
    object: &[bool],
    size: &[usize],
    map: &[u32],
    labels: &[u32],
    objects: usize,
) -> Result<(Vec<u64>, Vec<u64>), FillHolesError> {
    let d = size.len();
    let st = strides(size);
    let n = object.len() as isize;
    let mut object_edge = vec![0u64; objects + 1];
    let mut total_edge = vec![0u64; objects + 1];
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
            for a in 0..d {
                let o = (t % 3) as isize - 1;
                t /= 3;
                let p = coord[a] as isize + o;
                linear += p * st[a] as isize;
                clamped += p.clamp(0, size[a] as isize - 1) as usize * st[a];
            }
            if relabelled(clamped) == lab {
                continue;
            }
            if linear < 0 || linear >= n {
                return Err(FillHolesError::OutOfBounds { label: lab });
            }
            total_edge[lab] += 1;
            if object[linear as usize] {
                object_edge[lab] += 1;
            }
        }
    }
    Ok((object_edge, total_edge))
}

/// Why [`pad_image`] could not run.
#[derive(Debug, thiserror::Error)]
pub enum PadImageError {
    /// The new size is 0 or wraps around (ANTs then fails to allocate the image).
    #[error("PadImage: padding by {pad} gives an axis of {size} voxels")]
    BadSize { pad: f32, size: u64 },
}

/// `PadImage`: `ImageMath d out PadImage image pad [value]`: every axis grows by `2·pad`
/// voxels (shrinks for a negative `pad`), the image moves by `|pad|` voxels (truncated), and
/// the new voxels are `value`. The origin moves so that the voxels keep their positions in
/// space, computed as ITK's `TransformIndexToPhysicalPoint` does. The output is a new image
/// (no `descrip`).
///
/// ANTs computes each new size as `(unsigned int)(float(size) + pad·2)` and places voxel `i`
/// at `(unsigned int)(float(i) + pad)` when that is inside: so a non-integer `pad` (2.5)
/// grows an axis by 5 but shifts it by 2.
pub fn pad_image(
    image: &AntsImage<f32>,
    pad: f32,
    value: f32,
) -> Result<AntsImage<f32>, PadImageError> {
    let g = &image.geometry;
    let d = g.ndim;
    let mut new_size = Vec::with_capacity(d);
    for &n in &g.size {
        let s = n as f32 + pad * 2.0;
        // GCC converts float to `unsigned int` through a 64-bit truncation.
        let s = u64::from(x86_to_i64(f64::from(s)) as u32);
        if s == 0 || s > 1 << 31 {
            return Err(PadImageError::BadSize { pad, size: s });
        }
        new_size.push(s as usize);
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
            let shifted = coord[a] as f32 + pad;
            if shifted < 0.0 || shifted > (new_size[a] - 1) as f32 {
                continue 'voxels;
            }
            target += shifted as usize * out_strides[a];
        }
        data[target] = v;
    }
    Ok(AntsImage {
        data,
        geometry,
        meta: Default::default(),
    })
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
        let img = image(&[3, 2], (0..6).map(|v| v as f32 + 1.0).collect());
        let padded = pad_image(&img, 2.0, -5.0).unwrap();
        assert_eq!(padded.size(), [7, 6]);
        // Voxel (0, 0) moved to (2, 2); the origin moved by -2 voxels · direction · spacing.
        assert_eq!(padded.data[2 + 7 * 2], 1.0);
        assert_eq!(padded.data[0], -5.0);
        assert_eq!(padded.geometry.origin, vec![14.0, 14.0]);
        // Cropping back restores the image.
        let back = pad_image(&padded, -2.0, 0.0).unwrap();
        assert_eq!(back.data, img.data);
        assert_eq!(back.geometry.origin, img.geometry.origin);
        // A non-integer pad grows by 2·pad but shifts by the truncated pad.
        let odd = pad_image(&img, 1.5, 0.0).unwrap();
        assert_eq!(odd.size(), [6, 5]);
        assert_eq!(odd.data[1 + 6], 1.0);
        assert!(pad_image(&img, -1.0, 0.0).is_err());
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
        let filled = fill_holes(v, 2.0, 2).unwrap();
        assert_eq!(filled[at(3, 3, 3)], 1.0);
        assert_eq!(filled[at(2, 2, 2)], 3.0);
        assert_eq!(filled[at(0, 0, 0)], 0.0);
        // The hole is surrounded by object voxels only: its edge ratio is 1.
        assert_eq!(fill_holes(v, 0.99, 1).unwrap()[at(3, 3, 3)], 1.0);
        assert_eq!(fill_holes(v, 1.0, 1).unwrap()[at(3, 3, 3)], 0.0);
        assert_eq!(fill_holes(v, 2.5, 1).unwrap()[at(3, 3, 3)], 0.0);
        assert_eq!(fill_holes(v, 1.5, 1).unwrap()[at(3, 3, 3)], 1.0);
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
