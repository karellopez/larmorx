// SPDX-License-Identifier: Apache-2.0
//! Mathematical morphology with ITK v5.4.5's ball structuring element, as ANTs v2.6.5 uses it
//! (`ants::Morphological`, `Examples/antsUtilities.h`): `BinaryBallStructuringElement`,
//! `Binary{Erode,Dilate}ImageFilter`, `BinaryMorphological{Opening,Closing}ImageFilter`,
//! `Grayscale{Erode,Dilate}ImageFilter` and `GrayscaleMorphological{Opening,Closing}ImageFilter`.
//!
//! # The ball
//!
//! `BinaryBallStructuringElement::CreateStructuringElement` (`FlatStructuringElement::Ball`,
//! `itkFlatStructuringElement.hxx:913`) fills a `(2r+1)^D` neighbourhood with the voxels whose
//! centre lies in an ellipsoid of diameter `2r + 1` along every axis, centred on the middle
//! voxel: `Σ ((o_i) / (r + 0.5))² ≤ 1` in double, for the offset `o` from the centre
//! ([`ball_contains`]). The offsets are integers, so this is `Σ o_i² ≤ r(r + 1)`: the two
//! sides differ by at least `0.25 / (r + 0.5)²`, far more than the rounding of the double
//! computation for any radius larmorx accepts ([`MAX_RADIUS`]). The ball always holds its
//! centre and the points `±r` along every axis. In 3D, radius 1 is the 19 voxels of the
//! 3 × 3 × 3 cube without its corners; in 2D, the whole 3 × 3 square.
//!
//! # Binary filters
//!
//! ITK computes binary dilation by tracing the object's border and painting the structuring
//! element along it (`itkBinaryDilateImageFilter.hxx`): for a connected element `B` that is
//! exactly the Minkowski sum `X ⊕ B` of the voxels `X` equal to the foreground value. larmorx
//! gets the same set from an exact Euclidean distance transform in integers: a voxel is in
//! `X ⊕ B` exactly when some voxel of `X` lies within squared index distance `r(r + 1)`.
//! The values around it follow ITK's filters step by step:
//! - [`binary_dilate`] (`BoundaryToForeground` off: outside the image is not foreground):
//!   foreground in `X ⊕ B`; elsewhere the input, a foreground voxel becoming the background
//!   value (never happens: the ball holds its centre);
//! - [`binary_erode`] (`BoundaryToForeground` on: outside the image counts as foreground):
//!   voxels within reach of a non-foreground voxel become the input value if that is not the
//!   foreground, else the background value; the others the foreground;
//! - [`binary_opening`]: erosion (background `background`, ITK's default 0), then dilation;
//! - [`binary_closing`] (`SafeBorder` on): the image padded by `r` with 0 (or the type's
//!   maximum if the foreground is 0), dilated, eroded, cropped; voxels that did not end as
//!   foreground take the input value.
//!
//! # Grayscale filters
//!
//! The maximum (dilation) or minimum (erosion) over the ball, outside voxels counting as the
//! filter's boundary value (`NonpositiveMin` for dilation, `max` for erosion). ITK chooses
//! between a direct neighbourhood scan and a moving histogram; both give that extreme value.
//! Opening and closing pad the image by `r` (with `max` for opening, `NonpositiveMin` for
//! closing), filter twice and crop (`SafeBorder`). larmorx splits the ball into runs along
//! the first axis: the maximum over a run of half-width `w` is computed once per row for
//! every `w`, then combined over the ball's rows. Values are compared as the integers of
//! `f32::total_cmp`, so the result is exact; the one case where ITK's result depends on its
//! algorithm (a neighbourhood holding both `-0.0` and `+0.0` whose extreme is zero) gives
//! `+0.0` for the maximum and `-0.0` for the minimum here.

use larmorx_core::parallel;
use rayon::prelude::*;

use crate::FilterError;
use crate::lines::{Line, for_each_line};
use crate::volume::{VolumeRef, strides};

/// The largest radius accepted (in voxels): squared distances up to `r(r + 1)` fit in 32
/// bits. ANTs' own limit is the memory for its `(2r+1)^D` structuring element.
pub const MAX_RADIUS: usize = 65_535;

/// The largest number of voxels of an intermediate (padded) image.
const MAX_VOXELS: usize = 1 << 34;

const CHUNK: usize = 1 << 15;

/// Whether `offset` (from the centre) is in ITK's `BinaryBallStructuringElement` of `radius`
/// (`FlatStructuringElement::Ball` with `radiusIsParametric` off): the voxel centre lies in
/// the ellipsoid of axes `2r + 1` centred on the middle voxel, computed in double as
/// `EllipsoidInteriorExteriorSpatialFunction::Evaluate` does.
pub fn ball_contains(radius: usize, offset: &[isize]) -> bool {
    let half = 0.5 * (2 * radius + 1) as f64;
    let mut distance = 0.0f64;
    for &o in offset {
        let x = o as f64 / half;
        distance += x * x;
    }
    distance <= 1.0
}

/// The squared length bound of the ball of `radius`: an offset is in it exactly when
/// `Σ o_i² ≤ r(r + 1)` (see the module documentation).
pub fn ball_squared_radius(radius: usize) -> u64 {
    radius as u64 * (radius as u64 + 1)
}

fn check_radius(radius: usize) -> Result<(), FilterError> {
    if radius > MAX_RADIUS {
        return Err(FilterError::invalid(format!(
            "a radius of {radius} voxels is too large (at most {MAX_RADIUS})"
        )));
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// Distances

/// Scratch space of [`envelope_line`].
#[derive(Default)]
struct Envelope {
    /// The sites of the lower envelope.
    v: Vec<usize>,
    /// Where each site's parabola becomes the lowest: `zn / zd`.
    zn: Vec<i64>,
    zd: Vec<i64>,
    /// The line's input values.
    f: Vec<u32>,
}

/// The squared distance `min_j (x − j)² + f(j)` for every `x` of a line, capped at `cap`
/// (values `≥ cap` stand for "farther than `cap − 1`" and are not sites). The lower envelope
/// of parabolas of Felzenszwalb and Huttenlocher (2012), its breakpoints compared exactly as
/// rationals.
fn envelope_line(line: &mut [u32], cap: u32, s: &mut Envelope) {
    let n = line.len();
    let Some(first) = line.iter().position(|&f| f < cap) else {
        return;
    };
    if s.v.len() < n + 1 {
        s.v.resize(n + 1, 0);
        s.zn.resize(n + 1, 0);
        s.zd.resize(n + 1, 0);
    }
    s.f.clear();
    s.f.extend_from_slice(line);
    let Envelope { v, zn, zd, f } = s;
    let height = |q: usize| i64::from(f[q]) + (q as i64) * (q as i64);
    let mut k = 0usize;
    v[0] = first;
    for (q, &fq) in f.iter().enumerate().skip(first + 1) {
        if fq >= cap {
            continue;
        }
        let hq = height(q);
        loop {
            let p = v[k];
            // The parabola of `q` is the lower one right of `num / den`; `v[k]` goes if that
            // is not right of where `v[k]` itself became the lowest.
            let num = hq - height(p);
            let den = 2 * (q - p) as i64;
            if k > 0 && i128::from(num) * i128::from(zd[k]) <= i128::from(zn[k]) * i128::from(den) {
                k -= 1;
                continue;
            }
            k += 1;
            v[k] = q;
            zn[k] = num;
            zd[k] = den;
            break;
        }
    }
    let mut j = 0usize;
    for (x, out) in line.iter_mut().enumerate() {
        while j < k && i128::from(zn[j + 1]) < (x as i128) * i128::from(zd[j + 1]) {
            j += 1;
        }
        let p = v[j];
        let dx = x as i64 - p as i64;
        *out = (dx * dx + i64::from(f[p])).min(i64::from(cap)) as u32;
    }
}

/// For every voxel of an image of `size`, whether a voxel with `site[i]` lies within squared
/// index distance `t` (`Σ Δ_i² ≤ t`). Exact (integers); the result does not depend on
/// `n_threads`.
pub(crate) fn within_distance(
    site: &[bool],
    size: &[usize],
    t: u64,
    n_threads: usize,
) -> Result<Vec<bool>, FilterError> {
    let cap = u32::try_from(t + 1)
        .map_err(|_| FilterError::invalid("the squared radius does not fit in 32 bits"))?;
    if !site.iter().any(|&s| s) {
        return Ok(vec![false; site.len()]);
    }
    let mut dist: Vec<u32> = parallel::with_threads(n_threads, || {
        site.par_iter()
            .with_min_len(CHUNK)
            .map(|&s| if s { 0 } else { cap })
            .collect()
    })?;
    for axis in 0..size.len() {
        if size[axis] <= 1 {
            continue;
        }
        for_each_line(
            &mut dist,
            size,
            axis,
            n_threads,
            Envelope::default,
            |scratch, line, _: Line| envelope_line(line, cap, scratch),
        )?;
    }
    Ok(parallel::with_threads(n_threads, || {
        dist.par_iter()
            .with_min_len(CHUNK)
            .map(|&d| u64::from(d) <= t)
            .collect()
    })?)
}

// ------------------------------------------------------------------------------------------------
// Binary filters

fn check_input(input: &VolumeRef<'_, f32>, radius: usize) -> Result<(), FilterError> {
    check_radius(radius)?;
    if input.dim() == 0 {
        return Err(FilterError::invalid(
            "an image needs at least one dimension",
        ));
    }
    Ok(())
}

/// `BinaryDilateImageFilter<float, float, BinaryBallStructuringElement>` with
/// `SetDilateValue(foreground)` and `SetBackgroundValue(background)` (ITK's default background
/// is `NonpositiveMin`, `-f32::MAX`): `foreground` where a voxel equal to `foreground` lies
/// within the ball (inside the image), else the input value (a foreground voxel would become
/// `background`, but the ball always reaches its own centre).
pub fn binary_dilate(
    input: VolumeRef<'_, f32>,
    radius: usize,
    foreground: f32,
    background: f32,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    check_input(&input, radius)?;
    let site: Vec<bool> = parallel::with_threads(n_threads, || {
        input
            .data
            .par_iter()
            .with_min_len(CHUNK)
            .map(|&v| v == foreground)
            .collect()
    })?;
    let near = within_distance(&site, input.size, ball_squared_radius(radius), n_threads)?;
    Ok(parallel::with_threads(n_threads, || {
        input
            .data
            .par_iter()
            .zip(near.par_iter())
            .with_min_len(CHUNK)
            .map(|(&v, &near)| {
                if near {
                    foreground
                } else if v == foreground {
                    background
                } else {
                    v
                }
            })
            .collect()
    })?)
}

/// `BinaryErodeImageFilter<float, float, BinaryBallStructuringElement>` with
/// `SetErodeValue(foreground)` and `SetBackgroundValue(background)` (ITK's default background
/// is `NonpositiveMin`): voxels with a non-foreground voxel of the image within the ball
/// become `background`, then every voxel that is `background` and was not foreground takes
/// its input value back; the others are `foreground`. Outside the image counts as
/// foreground, so the image border does not erode.
pub fn binary_erode(
    input: VolumeRef<'_, f32>,
    radius: usize,
    foreground: f32,
    background: f32,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    check_input(&input, radius)?;
    let site: Vec<bool> = parallel::with_threads(n_threads, || {
        input
            .data
            .par_iter()
            .with_min_len(CHUNK)
            .map(|&v| v != foreground)
            .collect()
    })?;
    let near = within_distance(&site, input.size, ball_squared_radius(radius), n_threads)?;
    Ok(parallel::with_threads(n_threads, || {
        input
            .data
            .par_iter()
            .zip(near.par_iter())
            .with_min_len(CHUNK)
            .map(|(&v, &near)| {
                let out = if near { background } else { foreground };
                if out == background && v != foreground {
                    v
                } else {
                    out
                }
            })
            .collect()
    })?)
}

/// `BinaryMorphologicalOpeningImageFilter` with `SetForegroundValue(foreground)` and
/// `SetBackgroundValue(background)` (ITK's default 0): [`binary_erode`] with `background`,
/// then [`binary_dilate`] with ITK's default background.
pub fn binary_opening(
    input: VolumeRef<'_, f32>,
    radius: usize,
    foreground: f32,
    background: f32,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let eroded = binary_erode(input, radius, foreground, background, n_threads)?;
    let v = VolumeRef::new(&eroded, input.size, input.spacing);
    binary_dilate(v, radius, foreground, -f32::MAX, n_threads)
}

/// `BinaryMorphologicalClosingImageFilter` with `SetForegroundValue(foreground)` and
/// `SafeBorder` on (ITK's default): the image padded by `radius` voxels with 0 (with
/// `f32::MAX` if `foreground` is 0), [`binary_dilate`], [`binary_erode`] with that pad value
/// as background, cropped back; voxels that are not `foreground` then take the input value.
pub fn binary_closing(
    input: VolumeRef<'_, f32>,
    radius: usize,
    foreground: f32,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    check_input(&input, radius)?;
    let background = if foreground == 0.0 { f32::MAX } else { 0.0 };
    let (padded, size) = pad(input.data, input.size, radius, background, n_threads)?;
    let v = VolumeRef::new(&padded, &size, input.spacing);
    let dilated = binary_dilate(v, radius, foreground, -f32::MAX, n_threads)?;
    drop(padded);
    let v = VolumeRef::new(&dilated, &size, input.spacing);
    let eroded = binary_erode(v, radius, foreground, background, n_threads)?;
    drop(dilated);
    let cropped = crop(&eroded, &size, radius, n_threads)?;
    Ok(parallel::with_threads(n_threads, || {
        cropped
            .par_iter()
            .zip(input.data.par_iter())
            .with_min_len(CHUNK)
            .map(|(&c, &v)| if c != foreground { v } else { c })
            .collect()
    })?)
}

// ------------------------------------------------------------------------------------------------
// Padding

/// `data` (an image of `size`) with `r` voxels of `value` added on both sides of every axis
/// (`ConstantPadImageFilter`), and the new size.
pub(crate) fn pad<T: Copy + Send + Sync>(
    data: &[T],
    size: &[usize],
    r: usize,
    value: T,
    n_threads: usize,
) -> Result<(Vec<T>, Vec<usize>), FilterError> {
    let new_size: Vec<usize> = size.iter().map(|&n| n + 2 * r).collect();
    let total = new_size
        .iter()
        .try_fold(1usize, |acc, &n| acc.checked_mul(n))
        .filter(|&t| t <= MAX_VOXELS)
        .ok_or_else(|| FilterError::invalid("the padded image is too large"))?;
    let nx = new_size[0];
    let in_strides = strides(size);
    let mut out = vec![value; total];
    parallel::with_threads(n_threads, || {
        out.par_chunks_mut(nx)
            .enumerate()
            .with_min_len(64)
            .for_each(|(row, out_row)| {
                // The row's coordinates in the padded image, then in the input.
                let mut rest = row;
                let mut base = 0usize;
                for a in 1..new_size.len() {
                    let c = rest % new_size[a];
                    rest /= new_size[a];
                    if c < r || c >= r + size[a] {
                        return;
                    }
                    base += (c - r) * in_strides[a];
                }
                out_row[r..r + size[0]].copy_from_slice(&data[base..base + size[0]]);
            });
    })?;
    Ok((out, new_size))
}

/// The inverse of [`pad`]: `data` (an image of `size`) without `r` voxels on both sides of
/// every axis (`CropImageFilter`).
pub(crate) fn crop<T: Copy + Send + Sync + Default>(
    data: &[T],
    size: &[usize],
    r: usize,
    n_threads: usize,
) -> Result<Vec<T>, FilterError> {
    let new_size: Vec<usize> = size.iter().map(|&n| n - 2 * r).collect();
    let total: usize = new_size.iter().product();
    let nx = new_size[0];
    let in_strides = strides(size);
    let mut out = vec![T::default(); total];
    if total == 0 {
        return Ok(out);
    }
    parallel::with_threads(n_threads, || {
        out.par_chunks_mut(nx)
            .enumerate()
            .with_min_len(64)
            .for_each(|(row, out_row)| {
                let mut rest = row;
                let mut base = r;
                for a in 1..new_size.len() {
                    let c = rest % new_size[a];
                    rest /= new_size[a];
                    base += (c + r) * in_strides[a];
                }
                out_row.copy_from_slice(&data[base..base + nx]);
            });
    })?;
    Ok(out)
}

// ------------------------------------------------------------------------------------------------
// Grayscale filters

/// `v` as an integer in the order of `f32::total_cmp` (`-0.0 < +0.0`).
#[inline]
fn key(v: f32) -> u32 {
    let b = v.to_bits();
    if b >> 31 == 1 { !b } else { b | 0x8000_0000 }
}

#[inline]
fn value(k: u32) -> f32 {
    f32::from_bits(if k >> 31 == 1 { k & 0x7fff_ffff } else { !k })
}

/// The ball's runs along the first axis: for each offset on the other axes that the ball
/// reaches, the half-width `w` of its run `[-w, w]` along the first axis.
struct Runs {
    /// `(offsets on axes 1.., w)`.
    runs: Vec<(Vec<isize>, usize)>,
    radius: usize,
}

impl Runs {
    fn new(dim: usize, radius: usize) -> Self {
        let r = radius as isize;
        let mut runs = Vec::new();
        let mut t = vec![-r; dim.saturating_sub(1)];
        loop {
            let mut offset = vec![0isize; dim];
            offset[1..].copy_from_slice(&t);
            if ball_contains(radius, &offset) {
                let mut w = 0usize;
                while w < radius && {
                    offset[0] = w as isize + 1;
                    ball_contains(radius, &offset)
                } {
                    w += 1;
                }
                runs.push((t.clone(), w));
            }
            // Next offset tuple (first of the other axes fastest).
            let mut a = 0;
            loop {
                if a == t.len() {
                    return Runs { runs, radius };
                }
                if t[a] < r {
                    t[a] += 1;
                    break;
                }
                t[a] = -r;
                a += 1;
            }
        }
    }
}

/// The maximum over the ball (keys), outside voxels counting as `boundary`.
fn dilate_keys(
    keys: &[u32],
    size: &[usize],
    runs: &Runs,
    boundary: u32,
    n_threads: usize,
) -> Result<Vec<u32>, FilterError> {
    let n = keys.len();
    let nx = size[0];
    if n == 0 {
        return Ok(Vec::new());
    }
    let d = size.len();
    let row_stride = strides(&size[1..]);
    let mut out = vec![0u32; n];
    let mut current: Vec<u32> = keys.to_vec();
    let mut next = vec![0u32; n];
    for w in 0..=runs.radius {
        if w > 0 {
            // The maximum over [x - w, x + w] from the one over [x - w + 1, x + w - 1].
            parallel::with_threads(n_threads, || {
                next.par_chunks_mut(nx)
                    .zip(current.par_chunks(nx))
                    .with_min_len(64)
                    .for_each(|(o, c)| {
                        for x in 0..nx {
                            let mut m = c[x];
                            if x > 0 {
                                m = m.max(c[x - 1]);
                            }
                            if x + 1 < nx {
                                m = m.max(c[x + 1]);
                            }
                            o[x] = m;
                        }
                    });
            })?;
            std::mem::swap(&mut current, &mut next);
        }
        let these: Vec<&Vec<isize>> = runs
            .runs
            .iter()
            .filter(|(_, rw)| *rw == w)
            .map(|(t, _)| t)
            .collect();
        if these.is_empty() {
            continue;
        }
        let current = &current;
        parallel::with_threads(n_threads, || {
            out.par_chunks_mut(nx)
                .enumerate()
                .with_min_len(16)
                .for_each(|(row, out_row)| {
                    // The row's coordinates on the other axes.
                    let mut coord = [0usize; 8];
                    let mut rest = row;
                    for a in 1..d {
                        coord[a] = rest % size[a];
                        rest /= size[a];
                    }
                    'runs: for t in &these {
                        let mut source = 0usize;
                        for a in 1..d {
                            let c = coord[a] as isize + t[a - 1];
                            if c < 0 || c >= size[a] as isize {
                                continue 'runs;
                            }
                            source += c as usize * row_stride[a - 1];
                        }
                        let src = &current[source * nx..(source + 1) * nx];
                        for (o, &s) in out_row.iter_mut().zip(src) {
                            *o = (*o).max(s);
                        }
                    }
                });
        })?;
    }
    // Voxels whose ball reaches outside the image (within `radius` of a face) see the
    // boundary value too.
    let r = runs.radius;
    parallel::with_threads(n_threads, || {
        out.par_chunks_mut(nx)
            .enumerate()
            .with_min_len(64)
            .for_each(|(row, out_row)| {
                let mut rest = row;
                let mut near_face = false;
                for &na in &size[1..] {
                    let c = rest % na;
                    rest /= na;
                    if c < r || c + r >= na {
                        near_face = true;
                    }
                }
                for (x, o) in out_row.iter_mut().enumerate() {
                    if near_face || x < r || x + r >= nx {
                        *o = (*o).max(boundary);
                    }
                }
            });
    })?;
    Ok(out)
}

/// The minimum over the ball (keys), outside voxels counting as `boundary`: the complement of
/// the maximum of the complements.
fn erode_keys(
    keys: &[u32],
    size: &[usize],
    runs: &Runs,
    boundary: u32,
    n_threads: usize,
) -> Result<Vec<u32>, FilterError> {
    let complement: Vec<u32> = keys.iter().map(|&k| !k).collect();
    let mut out = dilate_keys(&complement, size, runs, !boundary, n_threads)?;
    out.iter_mut().for_each(|k| *k = !*k);
    Ok(out)
}

fn keys_of(input: &VolumeRef<'_, f32>, n_threads: usize) -> Result<Vec<u32>, FilterError> {
    Ok(parallel::with_threads(n_threads, || {
        input
            .data
            .par_iter()
            .with_min_len(CHUNK)
            .map(|&v| key(v))
            .collect()
    })?)
}

fn values_of(keys: &[u32], n_threads: usize) -> Result<Vec<f32>, FilterError> {
    Ok(parallel::with_threads(n_threads, || {
        keys.par_iter()
            .with_min_len(CHUNK)
            .map(|&k| value(k))
            .collect()
    })?)
}

fn check_dims(input: &VolumeRef<'_, f32>, radius: usize) -> Result<(), FilterError> {
    check_input(input, radius)?;
    if input.dim() > 8 {
        return Err(FilterError::invalid("at most 8 dimensions"));
    }
    Ok(())
}

/// `GrayscaleDilateImageFilter<float, float, BinaryBallStructuringElement>`: the maximum over
/// the ball, outside voxels counting as `-f32::MAX`.
pub fn grayscale_dilate(
    input: VolumeRef<'_, f32>,
    radius: usize,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    check_dims(&input, radius)?;
    let runs = Runs::new(input.dim(), radius);
    let keys = keys_of(&input, n_threads)?;
    let out = dilate_keys(&keys, input.size, &runs, key(-f32::MAX), n_threads)?;
    values_of(&out, n_threads)
}

/// `GrayscaleErodeImageFilter<float, float, BinaryBallStructuringElement>`: the minimum over
/// the ball, outside voxels counting as `f32::MAX`.
pub fn grayscale_erode(
    input: VolumeRef<'_, f32>,
    radius: usize,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    check_dims(&input, radius)?;
    let runs = Runs::new(input.dim(), radius);
    let keys = keys_of(&input, n_threads)?;
    let out = erode_keys(&keys, input.size, &runs, key(f32::MAX), n_threads)?;
    values_of(&out, n_threads)
}

/// `GrayscaleMorphologicalOpeningImageFilter` (`SafeBorder` on): the image padded by `radius`
/// voxels of `f32::MAX`, eroded, dilated and cropped.
pub fn grayscale_opening(
    input: VolumeRef<'_, f32>,
    radius: usize,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    check_dims(&input, radius)?;
    let runs = Runs::new(input.dim(), radius);
    let keys = keys_of(&input, n_threads)?;
    let (padded, size) = pad(&keys, input.size, radius, key(f32::MAX), n_threads)?;
    drop(keys);
    let eroded = erode_keys(&padded, &size, &runs, key(f32::MAX), n_threads)?;
    drop(padded);
    let dilated = dilate_keys(&eroded, &size, &runs, key(-f32::MAX), n_threads)?;
    drop(eroded);
    values_of(&crop(&dilated, &size, radius, n_threads)?, n_threads)
}

/// `GrayscaleMorphologicalClosingImageFilter` (`SafeBorder` on): the image padded by `radius`
/// voxels of `-f32::MAX`, dilated, eroded and cropped.
pub fn grayscale_closing(
    input: VolumeRef<'_, f32>,
    radius: usize,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    check_dims(&input, radius)?;
    let runs = Runs::new(input.dim(), radius);
    let keys = keys_of(&input, n_threads)?;
    let (padded, size) = pad(&keys, input.size, radius, key(-f32::MAX), n_threads)?;
    drop(keys);
    let dilated = dilate_keys(&padded, &size, &runs, key(-f32::MAX), n_threads)?;
    drop(padded);
    let eroded = erode_keys(&dilated, &size, &runs, key(f32::MAX), n_threads)?;
    drop(dilated);
    values_of(&crop(&eroded, &size, radius, n_threads)?, n_threads)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every offset of the `(2r+1)^D` neighbourhood, first axis fastest.
    fn offsets(dim: usize, r: usize) -> Vec<Vec<isize>> {
        let mut out: Vec<Vec<isize>> = vec![Vec::new()];
        for _ in 0..dim {
            out = (-(r as isize)..=r as isize)
                .flat_map(|o| {
                    out.iter().map(move |p| {
                        let mut v = p.clone();
                        v.push(o);
                        v
                    })
                })
                .collect();
        }
        out
    }

    #[test]
    fn the_ball_is_a_squared_length_bound() {
        for (dim, max_r) in [(1usize, 200usize), (2, 60), (3, 24), (4, 8)] {
            for r in 0..=max_r {
                let t = ball_squared_radius(r) as i64;
                for o in offsets(dim, r) {
                    let s: i64 = o.iter().map(|&v| (v * v) as i64).sum();
                    assert_eq!(ball_contains(r, &o), s <= t, "{dim}D r {r} {o:?}");
                }
            }
        }
        // ITK's 3D ball of radius 1 has 19 voxels; the 2D one is the full 3 x 3.
        assert_eq!(
            offsets(3, 1).iter().filter(|o| ball_contains(1, o)).count(),
            19
        );
        assert_eq!(
            offsets(2, 1).iter().filter(|o| ball_contains(1, o)).count(),
            9
        );
        // Large radii: offsets just inside and just outside the bound fall on the right side.
        for r in [1000usize, 30_000, MAX_RADIUS] {
            let t = ball_squared_radius(r) as i64;
            for a in (r as i64 - 300)..=(r as i64) {
                let mut b = ((t - a * a) as f64).sqrt() as i64;
                while b * b > t - a * a {
                    b -= 1;
                }
                while (b + 1) * (b + 1) <= t - a * a {
                    b += 1;
                }
                assert!(
                    ball_contains(r, &[a as isize, b as isize]),
                    "r {r} ({a}, {b})"
                );
                assert!(
                    !ball_contains(r, &[a as isize, b as isize + 1]),
                    "r {r} ({a}, {})",
                    b + 1
                );
                assert!(ball_contains(r, &[a as isize, 0, b as isize]));
            }
        }
    }

    /// The brute-force definitions.
    fn brute_dilate_set(site: &[bool], size: &[usize], r: usize) -> Vec<bool> {
        let st = strides(size);
        let balls: Vec<Vec<isize>> = offsets(size.len(), r)
            .into_iter()
            .filter(|o| ball_contains(r, o))
            .collect();
        (0..site.len())
            .map(|i| {
                let c: Vec<isize> = (0..size.len())
                    .map(|a| ((i / st[a]) % size[a]) as isize)
                    .collect();
                balls.iter().any(|o| {
                    let mut j = 0usize;
                    for a in 0..size.len() {
                        let p = c[a] + o[a];
                        if p < 0 || p >= size[a] as isize {
                            return false;
                        }
                        j += p as usize * st[a];
                    }
                    site[j]
                })
            })
            .collect()
    }

    fn brute_extreme(data: &[f32], size: &[usize], r: usize, max: bool, boundary: f32) -> Vec<f32> {
        let st = strides(size);
        let balls: Vec<Vec<isize>> = offsets(size.len(), r)
            .into_iter()
            .filter(|o| ball_contains(r, o))
            .collect();
        (0..data.len())
            .map(|i| {
                let c: Vec<isize> = (0..size.len())
                    .map(|a| ((i / st[a]) % size[a]) as isize)
                    .collect();
                let mut best = key(data[i]);
                for o in &balls {
                    let mut j = 0usize;
                    let mut inside = true;
                    for a in 0..size.len() {
                        let p = c[a] + o[a];
                        if p < 0 || p >= size[a] as isize {
                            inside = false;
                            break;
                        }
                        j += p as usize * st[a];
                    }
                    let k = if inside { key(data[j]) } else { key(boundary) };
                    best = if max { best.max(k) } else { best.min(k) };
                }
                value(best)
            })
            .collect()
    }

    fn pattern(size: &[usize], seed: usize) -> Vec<f32> {
        let n: usize = size.iter().product();
        (0..n)
            .map(|i| (((i * 7919 + seed * 104_729) % 97) as f32) - 30.0)
            .collect()
    }

    #[test]
    fn distances_match_brute_force() {
        for (size, r) in [
            (vec![13usize, 11, 9], 1usize),
            (vec![13, 11, 9], 2),
            (vec![17, 9], 3),
            (vec![9, 7, 5, 4], 1),
            (vec![30], 4),
            (vec![12, 10, 8], 5),
        ] {
            let data = pattern(&size, r);
            let site: Vec<bool> = data.iter().map(|&v| v > 55.0).collect();
            let expected = brute_dilate_set(&site, &size, r);
            for threads in [1, 3] {
                let got = within_distance(&site, &size, ball_squared_radius(r), threads).unwrap();
                assert_eq!(got, expected, "{size:?} r {r}");
            }
        }
    }

    #[test]
    fn binary_filters_follow_itk() {
        let size = [12usize, 10, 7];
        let spacing = [1.0, 1.0, 1.0];
        // A mask with values 0, 1 and 2 (2 is not the foreground).
        let data: Vec<f32> = pattern(&size, 3)
            .iter()
            .map(|&v| {
                if v > 20.0 {
                    1.0
                } else if v < -25.0 {
                    2.0
                } else {
                    0.0
                }
            })
            .collect();
        let v = VolumeRef::new(&data, &size, &spacing);
        let fg: Vec<bool> = data.iter().map(|&x| x == 1.0).collect();
        let not_fg: Vec<bool> = data.iter().map(|&x| x != 1.0).collect();
        for r in [0usize, 1, 2] {
            let dil = binary_dilate(v, r, 1.0, -f32::MAX, 2).unwrap();
            let set = brute_dilate_set(&fg, &size, r);
            for i in 0..data.len() {
                assert_eq!(dil[i], if set[i] { 1.0 } else { data[i] });
            }
            let ero = binary_erode(v, r, 1.0, -f32::MAX, 2).unwrap();
            let set = brute_dilate_set(&not_fg, &size, r);
            for i in 0..data.len() {
                let expected = if !set[i] {
                    1.0
                } else if data[i] != 1.0 {
                    data[i]
                } else {
                    -f32::MAX
                };
                assert_eq!(ero[i], expected);
            }
            // Thread counts do not matter.
            assert_eq!(
                binary_opening(v, r, 1.0, 0.0, 1).unwrap(),
                binary_opening(v, r, 1.0, 0.0, 4).unwrap()
            );
            assert_eq!(
                binary_closing(v, r, 1.0, 1).unwrap(),
                binary_closing(v, r, 1.0, 4).unwrap()
            );
        }
        // Closing fills a one-voxel hole; opening removes a one-voxel object.
        let mut cube = vec![0.0f32; 9 * 9 * 9];
        let at = |x: usize, y: usize, z: usize| x + 9 * (y + 9 * z);
        for z in 2..7 {
            for y in 2..7 {
                for x in 2..7 {
                    cube[at(x, y, z)] = 1.0;
                }
            }
        }
        cube[at(4, 4, 4)] = 0.0;
        cube[at(0, 0, 0)] = 1.0;
        let s = [9usize, 9, 9];
        let sp = [1.0; 3];
        let closed = binary_closing(VolumeRef::new(&cube, &s, &sp), 1, 1.0, 1).unwrap();
        assert_eq!(closed[at(4, 4, 4)], 1.0);
        let opened = binary_opening(VolumeRef::new(&cube, &s, &sp), 1, 1.0, 0.0, 1).unwrap();
        assert_eq!(opened[at(0, 0, 0)], 0.0);
        assert_eq!(opened[at(4, 3, 4)], 1.0);
    }

    #[test]
    fn grayscale_filters_match_brute_force() {
        for (size, r) in [
            (vec![11usize, 9, 7], 1usize),
            (vec![11, 9, 7], 2),
            (vec![15, 8], 3),
            (vec![6, 5, 4, 4], 1),
            (vec![20], 2),
        ] {
            let data = pattern(&size, r + 7);
            let spacing = vec![1.0; size.len()];
            let v = VolumeRef::new(&data, &size, &spacing);
            for threads in [1, 3] {
                assert_eq!(
                    grayscale_dilate(v, r, threads).unwrap(),
                    brute_extreme(&data, &size, r, true, -f32::MAX),
                    "{size:?} r {r}"
                );
                assert_eq!(
                    grayscale_erode(v, r, threads).unwrap(),
                    brute_extreme(&data, &size, r, false, f32::MAX),
                    "{size:?} r {r}"
                );
            }
            // Opening and closing: pad, filter twice, crop.
            let padded_size: Vec<usize> = size.iter().map(|&n| n + 2 * r).collect();
            let (p, _) = pad(&data, &size, r, f32::MAX, 1).unwrap();
            let e = brute_extreme(&p, &padded_size, r, false, f32::MAX);
            let d = brute_extreme(&e, &padded_size, r, true, -f32::MAX);
            assert_eq!(
                grayscale_opening(v, r, 2).unwrap(),
                crop(&d, &padded_size, r, 1).unwrap()
            );
            let (p, _) = pad(&data, &size, r, -f32::MAX, 1).unwrap();
            let d = brute_extreme(&p, &padded_size, r, true, -f32::MAX);
            let e = brute_extreme(&d, &padded_size, r, false, f32::MAX);
            assert_eq!(
                grayscale_closing(v, r, 2).unwrap(),
                crop(&e, &padded_size, r, 1).unwrap()
            );
        }
    }

    #[test]
    fn keys_order_like_total_cmp() {
        let vals = [-f32::MAX, -1.0, -0.0, 0.0, 1e-30, 3.0, f32::MAX];
        for w in vals.windows(2) {
            assert!(key(w[0]) < key(w[1]));
            assert_eq!(value(key(w[0])).to_bits(), w[0].to_bits());
        }
    }
}
