// SPDX-License-Identifier: Apache-2.0
//! ANTs' `MultiScaleLaplacianBlobDetectorImageFilter` (ANTs v2.6.5,
//! `Utilities/itkMultiScaleLaplacianBlobDetectorImageFilter.{h,hxx}`, by Bradley Lowekamp, a
//! public-domain work of the U.S. National Library of Medicine): the strongest local maxima
//! of the scale-normalised Laplacian of Gaussian over positions and scales, as `antsAI -b`
//! finds its blobs.
//!
//! As the filter computes them:
//! - **the scales**: `k = 2^(1/steps)`, `σ₀ = √t_start / k`, `n = ⌈ln(√t_end / σ₀) / ln k⌉ + 1`
//!   scales `σ_i = σ₀ k^(n − i − 1)`, from the largest down (one more than the range at each
//!   end, so that every scale of the range is compared with both neighbours);
//! - **the Laplacian** of each scale: ITK's `LaplacianRecursiveGaussianImageFilter<float,
//!   float>` normalised across scale ([`crate::gaussian::laplacian_recursive_gaussian`]);
//! - **the maxima**: each voxel of the middle scale of three consecutive ones whose value is
//!   at least every value of its `3^D` neighbourhood in the three (the boundary faces read the
//!   nearest voxel inside, ITK's zero-flux Neumann condition), visited face by face as
//!   `ImageBoundaryFacesCalculator` lists them (the interior first) and in raster order in
//!   each face;
//! - **the heaps**: a min-heap of at most `N` blobs per work unit, with libstdc++'s
//!   `push_heap`, `pop_heap` and `sort_heap` (the order of equal values depends on them), then
//!   `std::merge` into the list kept so far (cut to `N`), whose smallest value becomes the
//!   threshold of the next scale.
//!
//! **Threads.** ANTs sizes its vector of heaps by ITK's global default thread count but runs
//! the scale on the multi-threader's work units, four times as many when there is more than
//! one thread: the work units past the thread count index the vector past its end (ANTS-60,
//! undefined behaviour). larmorx computes the one-work-unit result, which is what ANTs
//! defines at one thread, on any thread count (the Laplacians and the maxima test run in
//! parallel; the heaps take the candidates in ANTs' order).
//!
//! **The thresholds (ANTS-112).** The filter means to keep the `N` strongest maxima, but (1)
//! after each scale it takes the smallest kept value as the threshold for the next scales even
//! while fewer than `N` blobs are kept, so a weaker maximum found later is dropped although
//! there is room for it; and (2) when a work unit's heap has just filled, the next candidate is
//! compared with that threshold instead of the heap's smallest value, so a candidate weaker
//! than every blob in the heap can push the weakest one out. [`Thresholds::Strongest`] keeps
//! the `N` strongest (the threshold applies only to a full list, a full heap compares with its
//! smallest); [`Thresholds::Ants`] reproduces ANTs. [`detect`] computes both and returns which
//! differ.

#![allow(clippy::needless_range_loop)]

use larmorx_core::math;
use larmorx_core::parallel;
use rayon::prelude::*;

use crate::FilterError;
use crate::gaussian::laplacian_recursive_gaussian;
use crate::volume::{VolumeRef, strides};

/// The filter's settings (`SetStartT`, `SetEndT`, `SetStepsPerOctave`, `SetNumberOfBlobs`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlobSettings {
    /// The first scale as a variance, `t = σ²`.
    pub start_t: f64,
    /// The last scale as a variance.
    pub end_t: f64,
    /// Scales per doubling of σ (an `unsigned int` in the filter).
    pub steps_per_octave: u32,
    /// The most blobs kept.
    pub number_of_blobs: usize,
}

/// A blob: the filter's private `Blob` (centre index, the Laplacian's σ, the value).
#[derive(Clone, Debug, PartialEq)]
pub struct Blob {
    /// The voxel index of the maximum.
    pub center: Vec<i64>,
    /// The σ of the Laplacian at the maximum (`m_Sigma`, physical units).
    pub sigma: f64,
    /// The normalised Laplacian's value there.
    pub value: f32,
}

impl Blob {
    /// `ScaleSpaceBlobSpatialObject::GetObjectRadius()`: `√(D/2)·σ` (the Gaussian's
    /// `SigmaInObjectSpace`) times `√2`.
    pub fn object_radius(&self) -> f64 {
        let d = self.center.len() as f64;
        ((d / 2.0).sqrt() * self.sigma) * std::f64::consts::SQRT_2
    }
}

/// How the thresholds work (ANTS-112).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Thresholds {
    /// ANTs' code.
    Ants,
    /// The `N` strongest maxima.
    Strongest,
}

/// What [`detect`] found: the blobs with ANTs' thresholds and with the corrected ones, in the
/// filter's order (descending values), and the number of scales.
#[derive(Clone, Debug, PartialEq)]
pub struct Detection {
    pub ants: Vec<Blob>,
    pub strongest: Vec<Blob>,
    pub scales: usize,
}

impl Detection {
    /// The blobs of `thresholds`.
    pub fn blobs(&self, thresholds: Thresholds) -> &[Blob] {
        match thresholds {
            Thresholds::Ants => &self.ants,
            Thresholds::Strongest => &self.strongest,
        }
    }
}

/// The number of scales and the σ of each, as `GenerateData` computes them.
pub fn scales(settings: &BlobSettings) -> Vec<f64> {
    let k = math::pow(2.0, 1.0 / f64::from(settings.steps_per_octave));
    let initial_sigma = settings.start_t.sqrt() * 1.0 / k;
    let n = (math::log(settings.end_t.sqrt() / initial_sigma) / math::log(k)).ceil() + 1.0;
    // `const unsigned int numberOfScales = std::ceil(...) + 1.0;`
    let n = if n.is_nan() || n <= 0.0 {
        0
    } else if n >= 4_294_967_296.0 {
        u32::MAX as usize
    } else {
        n as u32 as usize
    };
    (0..n)
        .map(|i| initial_sigma * math::pow(k, (n - i - 1) as f64))
        .collect()
}

/// `ImageBoundaryFacesCalculator::Compute` for the whole image and radius 1: the faces as
/// `(start, size)` per axis, the non-boundary region first.
pub(crate) fn boundary_faces(size: &[usize], radius: usize) -> Vec<(Vec<i64>, Vec<usize>)> {
    let d = size.len();
    let r = radius as i64;
    let b_start = vec![0i64; d];
    let b_size: Vec<i64> = size.iter().map(|&s| s as i64).collect();
    let r_start = b_start.clone();
    let r_size = b_size.clone();
    let mut nb_size = r_size.clone();
    let mut nb_start = r_start.clone();
    let mut vr_start = r_start.clone();
    let mut vr_size = r_size.clone();
    let mut faces = Vec::new();
    for i in 0..d {
        let mut overlap_low = (r_start[i] - r) - b_start[i];
        let mut overlap_high = if b_size[i] > 2 * r {
            (b_start[i] + b_size[i]) - (r_start[i] + r_size[i] + r)
        } else {
            (b_start[i] + r) - (r_start[i] + r_size[i])
        };
        if overlap_low < 0 {
            let mut f_start = vec![0i64; d];
            let mut f_size = vec![0i64; d];
            for j in 0..d {
                f_start[j] = vr_start[j];
                if j == i {
                    if -overlap_low > r_size[i] {
                        overlap_low = -r_size[i];
                    }
                    f_size[j] = -overlap_low;
                    vr_size[j] += overlap_low;
                    vr_start[j] -= overlap_low;
                } else {
                    f_size[j] = vr_size[j];
                }
                f_size[j] = f_size[j].min(r_size[j]);
            }
            if f_size[i] > nb_size[i] {
                nb_size[i] = 0;
            } else {
                nb_size[i] -= f_size[i];
            }
            nb_start[i] += -overlap_low;
            faces.push((f_start, f_size));
        }
        if overlap_high < 0 {
            let mut f_start = vec![0i64; d];
            let mut f_size = vec![0i64; d];
            for j in 0..d {
                if j == i {
                    if -overlap_high > r_size[i] {
                        overlap_high = -r_size[i];
                    }
                    f_start[j] = r_start[j] + r_size[j] + overlap_high;
                    f_size[j] = -overlap_high;
                    vr_size[j] += overlap_high;
                } else {
                    f_start[j] = vr_start[j];
                    f_size[j] = vr_size[j];
                }
            }
            if f_size[i] > nb_size[i] {
                nb_size[i] = 0;
            } else {
                nb_size[i] -= f_size[i];
            }
            faces.push((f_start, f_size));
        }
    }
    let mut out = Vec::with_capacity(faces.len() + 1);
    out.push((nb_start, nb_size));
    out.extend(faces);
    out.into_iter()
        .map(|(s, z)| (s, z.into_iter().map(|v| v.max(0) as usize).collect()))
        .collect()
}

/// libstdc++'s heap algorithms (GCC 14's `bits/stl_heap.h`) with the filter's comparison
/// `BlobValueGreaterCompare`, `a.value > b.value` (a min-heap: the first element is the
/// smallest).
mod heap {
    use super::Blob;

    #[inline]
    fn comp(a: &Blob, b: &Blob) -> bool {
        a.value > b.value
    }

    /// `__push_heap(first, holeIndex, topIndex, value)`.
    fn push_hole(h: &mut [Blob], mut hole: usize, top: usize, value: Blob) {
        // `_Distance __parent = (__holeIndex - 1) / 2;` (signed: 0 for hole 0).
        let parent_of = |i: usize| if i == 0 { 0 } else { (i - 1) / 2 };
        let mut parent = parent_of(hole);
        while hole > top && comp(&h[parent], &value) {
            h[hole] = h[parent].clone();
            hole = parent;
            parent = parent_of(hole);
        }
        h[hole] = value;
    }

    /// `push_heap(first, last)`: the last element sifted up.
    pub fn push(h: &mut [Blob]) {
        let n = h.len();
        let value = h[n - 1].clone();
        push_hole(h, n - 1, 0, value);
    }

    /// `__adjust_heap(first, holeIndex, len, value)`.
    fn adjust(h: &mut [Blob], mut hole: usize, len: usize, value: Blob) {
        let top = hole;
        let mut second = hole;
        // `while (__secondChild < (__len - 1) / 2)` with signed lengths (len ≥ 1 here).
        let half = if len == 0 { 0 } else { (len - 1) / 2 };
        while second < half {
            second = 2 * (second + 1);
            if comp(&h[second], &h[second - 1]) {
                second -= 1;
            }
            h[hole] = h[second].clone();
            hole = second;
        }
        if len.is_multiple_of(2) && len >= 2 && second == (len - 2) / 2 {
            second = 2 * (second + 1);
            h[hole] = h[second - 1].clone();
            hole = second - 1;
        }
        push_hole(h, hole, top, value);
    }

    /// `__pop_heap(first, last, result)` with `result = last` (the range `[first, last)`
    /// being the heap without its last slot): the front moves to `result`.
    fn pop_to(h: &mut [Blob], last: usize) {
        let value = h[last].clone();
        h[last] = h[0].clone();
        adjust(h, 0, last, value);
    }

    /// `pop_heap(first, last)`: the smallest moves to the end, the rest stays a heap.
    pub fn pop(h: &mut [Blob]) {
        let n = h.len();
        if n > 1 {
            pop_to(h, n - 1);
        }
    }

    /// `sort_heap(first, last)`: descending values (the comparison's ascending order).
    pub fn sort(h: &mut [Blob]) {
        let mut last = h.len();
        while last > 1 {
            last -= 1;
            pop_to(h, last);
        }
    }

    /// `std::merge(a, b)` with the comparison: an element of `b` goes first only if its value
    /// is greater.
    pub fn merge(a: &[Blob], b: &[Blob]) -> Vec<Blob> {
        let mut out = Vec::with_capacity(a.len() + b.len());
        let (mut i, mut j) = (0, 0);
        while i < a.len() && j < b.len() {
            if comp(&b[j], &a[i]) {
                out.push(b[j].clone());
                j += 1;
            } else {
                out.push(a[i].clone());
                i += 1;
            }
        }
        out.extend_from_slice(&a[i..]);
        out.extend_from_slice(&b[j..]);
        out
    }
}

/// One threshold variant's state across scales.
struct Keeper {
    thresholds: Thresholds,
    n: usize,
    blobs: Vec<Blob>,
    global_min: f32,
}

impl Keeper {
    fn new(thresholds: Thresholds, n: usize) -> Self {
        Keeper {
            thresholds,
            n,
            blobs: Vec::new(),
            global_min: f32::MIN,
        }
    }

    /// The threshold the next scale starts from (the work unit's `localMinimalBestBlobValue`).
    fn start_threshold(&self) -> f32 {
        self.global_min
    }

    /// `ThreadedGenerateData`'s heap over the scale's candidates (in visiting order), then
    /// the merge of `GenerateData`.
    fn scale(
        &mut self,
        candidates: &[(usize, f32)],
        index_of: impl Fn(usize) -> Vec<i64>,
        sigma: f64,
    ) {
        let mut heap: Vec<Blob> = Vec::with_capacity(self.n.min(candidates.len()));
        let mut local_min = self.global_min;
        for &(offset, center) in candidates {
            if center < local_min {
                continue;
            }
            if heap.len() < self.n {
                heap.push(Blob {
                    center: index_of(offset),
                    sigma,
                    value: center,
                });
                heap::push(&mut heap);
                if self.thresholds == Thresholds::Strongest && heap.len() == self.n {
                    local_min = heap[0].value;
                }
            } else if center > local_min {
                heap::pop(&mut heap);
                let last = heap.len() - 1;
                heap[last] = Blob {
                    center: index_of(offset),
                    sigma,
                    value: center,
                };
                heap::push(&mut heap);
                local_min = heap[0].value;
            }
        }
        heap::sort(&mut heap);
        if !heap.is_empty() {
            let mut merged = heap::merge(&self.blobs, &heap);
            merged.truncate(self.n);
            self.blobs = merged;
        }
        if !self.blobs.is_empty() {
            self.global_min = match self.thresholds {
                Thresholds::Ants => self.blobs[self.blobs.len() - 1].value,
                Thresholds::Strongest if self.blobs.len() == self.n => {
                    self.blobs[self.blobs.len() - 1].value
                }
                Thresholds::Strongest => f32::MIN,
            };
        }
    }
}

/// The voxels of the middle image `n1` that are maxima of their `3^D` neighbourhoods in `n0`,
/// `n1` and `n2` (each test `center < neighbour` false, NaN included), in the filter's
/// visiting order, with their values; voxels below `floor` are skipped (they could not pass
/// the heap's threshold, which only grows).
fn maxima(
    n0: &[f32],
    n1: &[f32],
    n2: &[f32],
    size: &[usize],
    faces: &[(Vec<i64>, Vec<usize>)],
    floor: f32,
    n_threads: usize,
) -> Result<Vec<(usize, f32)>, FilterError> {
    let d = size.len();
    let st = strides(size);
    let count: usize = 3usize.pow(d as u32);
    // The neighbourhood's offsets per axis, the first axis fastest (ITK's order).
    let offsets: Vec<Vec<i64>> = (0..count)
        .map(|h| {
            let mut v = Vec::with_capacity(d);
            let mut r = h;
            for _ in 0..d {
                v.push((r % 3) as i64 - 1);
                r /= 3;
            }
            v
        })
        .collect();
    let linear: Vec<isize> = offsets
        .iter()
        .map(|o| (0..d).map(|k| o[k] as isize * st[k] as isize).sum())
        .collect();
    let images = [n0, n1, n2];
    let test = |idx: &[i64], o: usize, interior: bool| -> Option<f32> {
        let center = n1[o];
        if center < floor {
            return None;
        }
        for img in images {
            for h in 0..count {
                let v = if interior {
                    img[(o as isize + linear[h]) as usize]
                } else {
                    let mut lin = 0usize;
                    for k in 0..d {
                        let c = (idx[k] + offsets[h][k]).clamp(0, size[k] as i64 - 1);
                        lin += c as usize * st[k];
                    }
                    img[lin]
                };
                if center < v {
                    return None;
                }
            }
        }
        Some(center)
    };
    let mut out = Vec::new();
    for (fi, (start, fsize)) in faces.iter().enumerate() {
        let n: usize = fsize.iter().product();
        if n == 0 {
            continue;
        }
        let interior = fi == 0;
        // Rows along the first axis, in raster order; each row's maxima in order.
        let row_len = fsize[0];
        let rows = n / row_len;
        let per_row: Vec<Vec<(usize, f32)>> = parallel::with_threads(n_threads, || {
            (0..rows)
                .into_par_iter()
                .map(|row| {
                    let mut idx = vec![0i64; d];
                    let mut r = row;
                    for k in 1..d {
                        idx[k] = start[k] + (r % fsize[k]) as i64;
                        r /= fsize[k];
                    }
                    let mut found = Vec::new();
                    for x in 0..row_len {
                        idx[0] = start[0] + x as i64;
                        let o: usize = (0..d).map(|k| idx[k] as usize * st[k]).sum();
                        if let Some(v) = test(&idx, o, interior) {
                            found.push((o, v));
                        }
                    }
                    found
                })
                .collect()
        })?;
        for row in per_row {
            out.extend(row);
        }
    }
    Ok(out)
}

/// The blob detector on `input` (a `float` image; spacing in physical units): the blobs with
/// ANTs' thresholds and with the corrected ones (ANTS-112). Fails where ITK's Laplacian throws
/// (an axis of fewer than 4 voxels, ITK's message).
pub fn detect(
    input: VolumeRef<'_, f32>,
    settings: &BlobSettings,
    n_threads: usize,
) -> Result<Detection, FilterError> {
    let size = input.size;
    let d = size.len();
    let sigmas = scales(settings);
    let faces = boundary_faces(size, 1);
    let index_of = |mut o: usize| -> Vec<i64> {
        let mut idx = vec![0i64; d];
        for k in 0..d {
            idx[k] = (o % size[k]) as i64;
            o /= size[k];
        }
        idx
    };
    let mut ants = Keeper::new(Thresholds::Ants, settings.number_of_blobs);
    let mut strongest = Keeper::new(Thresholds::Strongest, settings.number_of_blobs);
    let mut window: Vec<Vec<f32>> = Vec::with_capacity(3);
    for (i, &sigma) in sigmas.iter().enumerate() {
        let lap = laplacian_recursive_gaussian(input, sigma, true, n_threads)?;
        window.push(lap);
        if window.len() > 3 {
            window.remove(0);
        }
        if i > 1 {
            let floor = ants.start_threshold().min(strongest.start_threshold());
            let candidates = maxima(
                &window[0], &window[1], &window[2], size, &faces, floor, n_threads,
            )?;
            // m_CurrentSigma: the previous scale's σ (the middle image).
            let center_sigma = sigmas[i - 1];
            ants.scale(&candidates, index_of, center_sigma);
            strongest.scale(&candidates, index_of, center_sigma);
        }
    }
    Ok(Detection {
        ants: ants.blobs,
        strongest: strongest.blobs,
        scales: sigmas.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob(v: f32) -> Blob {
        Blob {
            center: vec![0],
            sigma: 1.0,
            value: v,
        }
    }

    #[test]
    fn antsai_scales() {
        let s = BlobSettings {
            start_t: 1.0,
            end_t: 1024.0,
            steps_per_octave: 10,
            number_of_blobs: 10,
        };
        let sig = scales(&s);
        assert_eq!(sig.len(), 53);
        assert!(sig.windows(2).all(|w| w[0] > w[1]));
        assert!((sig[52] - 0.933_032_991_536_807_4).abs() < 1e-15);
    }

    #[test]
    fn faces_of_a_small_image() {
        let f = boundary_faces(&[5, 4, 3], 1);
        // The interior, then the two faces along each axis.
        assert_eq!(f[0], (vec![1, 1, 1], vec![3, 2, 1]));
        assert_eq!(f.len(), 7);
        let total: usize = f.iter().map(|(_, s)| s.iter().product::<usize>()).sum();
        assert_eq!(total, 60);
    }

    #[test]
    fn heap_keeps_the_largest_and_sorts_them_down() {
        let mut h: Vec<Blob> = Vec::new();
        for v in [3.0, 1.0, 4.0, 1.5, 9.0, 2.0] {
            h.push(blob(v));
            heap::push(&mut h);
        }
        assert_eq!(h[0].value, 1.0);
        heap::pop(&mut h);
        assert_eq!(h.last().unwrap().value, 1.0);
        h.pop();
        heap::sort(&mut h);
        let v: Vec<f32> = h.iter().map(|b| b.value).collect();
        assert_eq!(v, vec![9.0, 4.0, 3.0, 2.0, 1.5]);
    }

    fn values(k: &Keeper) -> Vec<f32> {
        k.blobs.iter().map(|b| b.value).collect()
    }

    /// ANTS-112 (1): ANTs' threshold after a scale is the weakest kept blob even while the
    /// list has room, so a weaker maximum of a later scale is dropped.
    #[test]
    fn ants_threshold_before_the_list_is_full() {
        let idx = |o: usize| vec![o as i64];
        let mut ants = Keeper::new(Thresholds::Ants, 3);
        let mut strongest = Keeper::new(Thresholds::Strongest, 3);
        for k in [&mut ants, &mut strongest] {
            k.scale(&[(0, 10.0)], idx, 4.0);
            k.scale(&[(1, 5.0), (2, 7.0)], idx, 2.0);
        }
        assert_eq!(values(&ants), vec![10.0]);
        assert_eq!(values(&strongest), vec![10.0, 7.0, 5.0]);
    }

    /// ANTS-112 (2): when a heap has just filled, ANTs compares the next candidate with the
    /// scale's starting threshold instead of the heap's weakest blob.
    #[test]
    fn ants_heap_replacement_after_filling() {
        let idx = |o: usize| vec![o as i64];
        let mut ants = Keeper::new(Thresholds::Ants, 2);
        let mut strongest = Keeper::new(Thresholds::Strongest, 2);
        for k in [&mut ants, &mut strongest] {
            k.scale(&[(0, 8.0), (1, 9.0), (2, 1.0)], idx, 4.0);
        }
        assert_eq!(values(&ants), vec![9.0, 1.0]);
        assert_eq!(values(&strongest), vec![9.0, 8.0]);
        // Merging keeps the earlier scale's blob first among equal values.
        let mut k = Keeper::new(Thresholds::Strongest, 3);
        k.scale(&[(0, 5.0)], idx, 4.0);
        k.scale(&[(1, 5.0), (2, 6.0)], idx, 2.0);
        let order: Vec<(f32, f64)> = k.blobs.iter().map(|b| (b.value, b.sigma)).collect();
        assert_eq!(order, vec![(6.0, 2.0), (5.0, 4.0), (5.0, 2.0)]);
    }

    #[test]
    fn finds_a_bright_spot_at_its_scale() {
        // A Gaussian-like spot of variance about 4 (σ 2) in a 24³ image.
        let n = 24usize;
        let mut data = vec![0.0f32; n * n * n];
        for z in 0..n {
            for y in 0..n {
                for x in 0..n {
                    let r2 = ((x as f32 - 11.0).powi(2)
                        + (y as f32 - 12.0).powi(2)
                        + (z as f32 - 12.0).powi(2))
                        / 8.0;
                    data[x + n * (y + n * z)] = 100.0 / (1.0 + r2 + r2 * r2 / 2.0);
                }
            }
        }
        // The Laplacian's minimum is at a bright spot: invert so it is a maximum.
        let data: Vec<f32> = data.iter().map(|v| -v).collect();
        let size = [n, n, n];
        let spacing = [1.0, 1.0, 1.0];
        let s = BlobSettings {
            start_t: 1.0,
            end_t: 64.0,
            steps_per_octave: 10,
            number_of_blobs: 3,
        };
        let det = detect(VolumeRef::new(&data, &size, &spacing), &s, 2).unwrap();
        assert_eq!(det.strongest[0].center, vec![11, 12, 12]);
        let single = detect(VolumeRef::new(&data, &size, &spacing), &s, 1).unwrap();
        assert_eq!(det, single);
    }
}
