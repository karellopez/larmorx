// SPDX-License-Identifier: Apache-2.0
//! Distance maps, as ITK v5.4.5 computes them:
//! - `DanielssonDistanceMapImageFilter` ([`danielsson_distance_map`];
//!   `Modules/Filtering/DistanceMap/include/itkDanielssonDistanceMapImageFilter.hxx`):
//!   Danielsson's vector propagation. Every voxel holds the offset to its nearest object
//!   voxel (non-zero voxels are the objects, offset 0; the others start at
//!   `2·max(size)` on every axis). A `ReflectiveImageRegionConstIterator` sweeps the image
//!   back and forth along every axis (`2^D` visits per voxel), and at each background voxel
//!   compares, axis by axis, its offset with the neighbour's offset plus one step towards
//!   it, keeping the shorter (in physical units with `UseImageSpacing`). The propagation is
//!   **not exact** (it can miss the nearest object in some configurations) and its result
//!   depends on the order of the sweeps, so larmorx replays the same sweeps in the same
//!   order, on one thread, with the same double arithmetic.
//! - `SignedMaurerDistanceMapImageFilter` ([`signed_maurer_distance_map`];
//!   `itkSignedMaurerDistanceMapImageFilter.hxx`): the object's inner contour
//!   (`BinaryContourImageFilter`, fully connected: object voxels with a background voxel in
//!   their `3^D` neighbourhood) is the set of sites; Maurer's separable Voronoi pass runs
//!   along each axis in turn, in **float** (`OutputPixelType`), with two different
//!   computations of a voxel's position (`float(i)·float(spacing)` when building the
//!   envelope, `float(i·spacing)` when querying it). Lines are independent, so the result
//!   does not depend on `n_threads`. The output is the square root (in double) of the
//!   squared distance, negative inside the object unless `inside_is_positive`; contour
//!   voxels get `-0.0`.

use larmorx_core::parallel;
use rayon::prelude::*;

use crate::FilterError;
use crate::lines::{Line, for_each_line};
use crate::volume::{VolumeRef, strides};

const CHUNK: usize = 1 << 16;

// ------------------------------------------------------------------------------------------------
// Danielsson

/// `DanielssonDistanceMapImageFilter<float, float>` (`InputIsBinary` does not change the
/// distance map): the distance from every voxel to the nearest non-zero voxel, in physical
/// units with `use_spacing` (else in voxels), squared if `squared`. An image without any
/// non-zero voxel gets `‖2·max(size)·(1, …, 1)‖` (with spacing) everywhere but along the
/// sweeps' drift. Sequential: Danielsson's sweeps cannot be split without changing the
/// result.
pub fn danielsson_distance_map(
    input: VolumeRef<'_, f32>,
    use_spacing: bool,
    squared: bool,
) -> Result<Vec<f32>, FilterError> {
    match input.dim() {
        1 => Ok(danielsson::<1>(input, use_spacing, squared)),
        2 => Ok(danielsson::<2>(input, use_spacing, squared)),
        3 => Ok(danielsson::<3>(input, use_spacing, squared)),
        4 => Ok(danielsson::<4>(input, use_spacing, squared)),
        d => Err(FilterError::invalid(format!(
            "Danielsson distance maps in {d} dimensions are not supported"
        ))),
    }
}

/// `Σ (c_i · s_i)²` summed in axis order, as `UpdateLocalDistance` and `ComputeVoronoiMap`
/// compute it (without spacing, `Σ c_i²`).
#[inline]
fn norm<const D: usize>(c: &[i32; D], spacing: &[f64; D], use_spacing: bool) -> f64 {
    let mut n = 0.0f64;
    for i in 0..D {
        let mut v = f64::from(c[i]);
        if use_spacing {
            v *= spacing[i];
        }
        n += v * v;
    }
    n
}

fn danielsson<const D: usize>(
    input: VolumeRef<'_, f32>,
    use_spacing: bool,
    squared: bool,
) -> Vec<f32> {
    let size: [usize; D] = std::array::from_fn(|i| input.size[i]);
    let spacing: [f64; D] = std::array::from_fn(|i| input.spacing[i]);
    let st: [usize; D] = {
        let s = strides(&size);
        std::array::from_fn(|i| s[i])
    };
    let n = input.len();
    if n == 0 {
        return Vec::new();
    }
    // `PrepareData`: objects 0, the rest 2·maxLength on every axis.
    let max_length = size.iter().copied().max().unwrap_or(1) as i32;
    let far = [2 * max_length; D];
    let mut comp: Vec<[i32; D]> = input
        .data
        .iter()
        .map(|&v| if v != 0.0 { [0; D] } else { far })
        .collect();
    // Each voxel's current squared length (what `UpdateLocalDistance` recomputes as
    // `norm1`: the same expression on the same offset, so the same bits).
    let mut norms: Vec<f64> = comp
        .iter()
        .map(|c| norm(c, &spacing, use_spacing))
        .collect();
    // The reflective iterator: begin and end offsets 1 on axes longer than one voxel.
    let off: [usize; D] = std::array::from_fn(|i| usize::from(size[i] > 1));
    let mut pos = off;
    let mut forward = [true; D];
    let mut here: usize = (0..D).map(|i| pos[i] * st[i]).sum();
    loop {
        // `if (!inputIt.Get())`: zero (of either sign) is background.
        if input.data[here] == 0.0 {
            for dim in 0..D {
                if size[dim] <= 1 {
                    continue;
                }
                let (there, step) = if forward[dim] {
                    (here - st[dim], -1)
                } else {
                    (here + st[dim], 1)
                };
                let mut candidate = comp[there];
                candidate[dim] += step;
                let n2 = norm(&candidate, &spacing, use_spacing);
                if norms[here] > n2 {
                    comp[here] = candidate;
                    norms[here] = n2;
                }
            }
        }
        // `ReflectiveImageRegionConstIterator::operator++`.
        let mut remaining = false;
        for i in 0..D {
            if forward[i] {
                if pos[i] + 1 < size[i] {
                    pos[i] += 1;
                    here += st[i];
                } else {
                    let p = size[i] - off[i] - 1;
                    here = here - pos[i] * st[i] + p * st[i];
                    pos[i] = p;
                    forward[i] = false;
                }
                remaining = true;
                break;
            }
            if pos[i] >= 1 {
                pos[i] -= 1;
                here -= st[i];
                remaining = true;
                break;
            }
            let p = off[i];
            here = here - pos[i] * st[i] + p * st[i];
            pos[i] = p;
            forward[i] = true;
        }
        if !remaining {
            break;
        }
    }
    // `ComputeVoronoiMap`: the distance in double, stored as float.
    comp.iter()
        .map(|c| {
            let mut d = 0.0f64;
            for i in 0..D {
                if use_spacing {
                    let v = f64::from(c[i]) * spacing[i];
                    d += v * v;
                } else {
                    d += (i64::from(c[i]) * i64::from(c[i])) as f64;
                }
            }
            if squared { d as f32 } else { d.sqrt() as f32 }
        })
        .collect()
}

// ------------------------------------------------------------------------------------------------
// Signed Maurer

/// The options of [`signed_maurer_distance_map`], with ITK's defaults in `Default`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaurerOptions {
    /// The background value; every other value is the object (default 0).
    pub background: f32,
    /// Positive distances inside the object (default false: negative inside).
    pub inside_is_positive: bool,
    /// Squared distances (default true in ITK; ANTs sets false).
    pub squared: bool,
    /// Physical units (default false in ITK; ANTs sets true).
    pub use_spacing: bool,
}

impl Default for MaurerOptions {
    fn default() -> Self {
        MaurerOptions {
            background: 0.0,
            inside_is_positive: false,
            squared: true,
            use_spacing: false,
        }
    }
}

/// The object's inner contour (`BinaryContourImageFilter`, fully connected): object voxels
/// with a background voxel among their `3^D` neighbours inside the image.
fn contour(object: &[bool], size: &[usize], n_threads: usize) -> Result<Vec<bool>, FilterError> {
    let nx = size[0];
    let rows = &size[1..];
    let row_strides = strides(rows);
    let d = rows.len();
    let mut out = vec![false; object.len()];
    parallel::with_threads(n_threads, || {
        out.par_chunks_mut(nx)
            .enumerate()
            .with_min_len(16)
            .for_each(|(row, out_row)| {
                let obj = &object[row * nx..(row + 1) * nx];
                if !obj.iter().any(|&o| o) {
                    return;
                }
                let coord: Vec<usize> = (0..d).map(|a| (row / row_strides[a]) % rows[a]).collect();
                // Background within one voxel along x, in any neighbouring row (this one too).
                let mut near = vec![false; nx];
                for mut t in 0..3usize.pow(d as u32) {
                    let mut other = 0usize;
                    let mut inside = true;
                    for a in 0..d {
                        let c = coord[a] as isize + (t % 3) as isize - 1;
                        t /= 3;
                        if c < 0 || c >= rows[a] as isize {
                            inside = false;
                            break;
                        }
                        other += c as usize * row_strides[a];
                    }
                    if !inside {
                        continue;
                    }
                    let o = &object[other * nx..(other + 1) * nx];
                    for x in 0..nx {
                        if !o[x] {
                            near[x.saturating_sub(1)] = true;
                            near[x] = true;
                            if x + 1 < nx {
                                near[x + 1] = true;
                            }
                        }
                    }
                }
                for x in 0..nx {
                    out_row[x] = obj[x] && near[x];
                }
            });
    })?;
    Ok(out)
}

/// `Remove` of Maurer's algorithm (float arithmetic, left to right).
#[inline]
fn remove(d1: f32, d2: f32, df: f32, x1: f32, x2: f32, xf: f32) -> bool {
    let a = x2 - x1;
    let b = xf - x2;
    let c = xf - x1;
    let value = c * d2.abs() - b * d1.abs() - a * df.abs() - a * b * c;
    value > 0.0
}

/// Scratch space of [`voronoi`].
#[derive(Default)]
struct Envelope {
    g: Vec<f32>,
    h: Vec<f32>,
}

/// `SignedMaurerDistanceMapImageFilter::Voronoi` on one line: `line` holds the values from
/// the previous axes (sites are the values other than `f32::MAX`), `inside(i)` says whether
/// voxel `i` of the line is in the object.
fn voronoi(
    line: &mut [f32],
    inside: impl Fn(usize) -> bool,
    spacing: f64,
    options: &MaurerOptions,
    s: &mut Envelope,
) {
    let nd = line.len();
    if s.g.len() < nd {
        s.g.resize(nd, 0.0);
        s.h.resize(nd, 0.0);
    }
    let (g, h) = (&mut s.g, &mut s.h);
    let mut l: isize = -1;
    for (i, &di) in line.iter().enumerate() {
        let iw = if options.use_spacing {
            i as f32 * spacing as f32
        } else {
            i as f32
        };
        if di != f32::MAX {
            if l >= 1 {
                while l >= 1 {
                    let k = l as usize;
                    if !remove(g[k - 1], g[k], di, h[k - 1], h[k], iw) {
                        break;
                    }
                    l -= 1;
                }
            }
            l += 1;
            g[l as usize] = di;
            h[l as usize] = iw;
        }
    }
    if l == -1 {
        return;
    }
    let ns = l as usize;
    let mut l = 0usize;
    for (i, out) in line.iter_mut().enumerate() {
        let iw = if options.use_spacing {
            (i as f64 * spacing) as f32
        } else {
            i as f32
        };
        let mut d1 = g[l].abs() + (h[l] - iw) * (h[l] - iw);
        while l < ns {
            let d2 = g[l + 1].abs() + (h[l + 1] - iw) * (h[l + 1] - iw);
            if d1 <= d2 {
                break;
            }
            l += 1;
            d1 = d2;
        }
        *out = if inside(i) != options.inside_is_positive {
            -d1
        } else {
            d1
        };
    }
}

/// `SignedMaurerDistanceMapImageFilter<float, float>`: the distance from every voxel to the
/// object's inner contour (the object being the voxels other than `options.background`),
/// negative inside the object unless `inside_is_positive`, squared or not, in physical units
/// with `use_spacing`. Voxels whose lines never meet a site (an image that is all object or
/// all background) keep `f32::MAX` (its square root, `1.8446743e19`, unless `squared`).
pub fn signed_maurer_distance_map(
    input: VolumeRef<'_, f32>,
    options: &MaurerOptions,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let size = input.size;
    let bg = options.background;
    let inside: Vec<bool> = parallel::with_threads(n_threads, || {
        input
            .data
            .par_iter()
            .with_min_len(CHUNK)
            .map(|&v| v != bg)
            .collect()
    })?;
    // The binary image (`BinaryThresholdImageFilter`: FLT_MAX for the background, 0 for the
    // object) and its contour (`BinaryContourImageFilter`): 0 on the object's inner contour,
    // FLT_MAX elsewhere. The thresholder's test `bg ≤ v ≤ bg` is `v == bg` (NaN is
    // object), so the object is exactly the voxels `inside`.
    let sites = contour(&inside, size, n_threads)?;
    let mut out: Vec<f32> = parallel::with_threads(n_threads, || {
        sites
            .par_iter()
            .with_min_len(CHUNK)
            .map(|&s| if s { 0.0 } else { f32::MAX })
            .collect()
    })?;
    // Maurer's Voronoi pass along each axis in turn.
    for (axis, &spacing) in input.spacing.iter().enumerate() {
        let inside = &inside;
        for_each_line(
            &mut out,
            size,
            axis,
            n_threads,
            Envelope::default,
            |s, line, at: Line| {
                voronoi(
                    line,
                    |i| inside[at.start + i * at.stride],
                    spacing,
                    options,
                    s,
                )
            },
        )?;
    }
    if !options.squared {
        parallel::with_threads(n_threads, || {
            out.par_iter_mut()
                .zip(inside.par_iter())
                .with_min_len(CHUNK)
                .for_each(|(v, &inside)| {
                    let d = f64::from(v.abs()).sqrt() as f32;
                    *v = if inside != options.inside_is_positive {
                        -d
                    } else {
                        d
                    };
                });
        })?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(size: &[usize], density: usize) -> Vec<f32> {
        let n: usize = size.iter().product();
        (0..n)
            .map(|i| {
                if (i * 7919 + i / 5) % density == 0 {
                    1.0
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// The exact Euclidean distance to the nearest object voxel.
    fn exact(
        data: &[f32],
        size: &[usize],
        spacing: &[f64],
        to: impl Fn(usize) -> bool,
    ) -> Vec<f64> {
        let st = strides(size);
        let d = size.len();
        let coord =
            |i: usize| -> Vec<f64> { (0..d).map(|a| ((i / st[a]) % size[a]) as f64).collect() };
        (0..data.len())
            .map(|i| {
                let ci = coord(i);
                (0..data.len())
                    .filter(|&j| to(j))
                    .map(|j| {
                        let cj = coord(j);
                        (0..d)
                            .map(|a| ((ci[a] - cj[a]) * spacing[a]).powi(2))
                            .sum::<f64>()
                    })
                    .fold(f64::INFINITY, f64::min)
                    .sqrt()
            })
            .collect()
    }

    #[test]
    fn danielsson_is_close_to_exact_and_zero_on_objects() {
        for (size, spacing) in [
            (vec![13usize, 11, 9], vec![1.0, 1.5, 2.0]),
            (vec![17, 15], vec![1.0, 1.0]),
            (vec![6, 5, 4, 3], vec![1.0, 1.0, 1.0, 2.0]),
        ] {
            let data = pattern(&size, 37);
            let v = VolumeRef::new(&data, &size, &spacing);
            let got = danielsson_distance_map(v, true, false).unwrap();
            let want = exact(&data, &size, &spacing, |j| data[j] != 0.0);
            for i in 0..data.len() {
                if data[i] != 0.0 {
                    assert_eq!(got[i], 0.0);
                }
                // Danielsson's 4-neighbour propagation is within a fraction of a voxel.
                assert!((f64::from(got[i]) - want[i]).abs() < 1.0, "{size:?} {i}");
            }
            let sq = danielsson_distance_map(v, true, true).unwrap();
            for i in 0..data.len() {
                assert!((f64::from(sq[i]).sqrt() - f64::from(got[i])).abs() < 1e-3);
            }
        }
    }

    fn brute_contour(object: &[bool], size: &[usize]) -> Vec<bool> {
        let st = strides(size);
        let d = size.len();
        (0..object.len())
            .map(|i| {
                object[i]
                    && (0..3usize.pow(d as u32)).any(|mut t| {
                        let mut j = 0usize;
                        for a in 0..d {
                            let c = ((i / st[a]) % size[a]) as isize + (t % 3) as isize - 1;
                            t /= 3;
                            if c < 0 || c >= size[a] as isize {
                                return false;
                            }
                            j += c as usize * st[a];
                        }
                        !object[j]
                    })
            })
            .collect()
    }

    #[test]
    fn maurer_matches_the_exact_distance_to_the_contour() {
        for (size, spacing) in [
            (vec![12usize, 10, 8], vec![1.0, 1.5, 2.0]),
            (vec![16, 13], vec![0.5, 1.0]),
            (vec![6, 5, 4, 4], vec![1.0, 1.0, 1.0, 3.0]),
        ] {
            let n: usize = size.iter().product();
            let st = strides(&size);
            // A blob.
            let data: Vec<f32> = (0..n)
                .map(|i| {
                    let r: f64 = (0..size.len())
                        .map(|a| {
                            let c = ((i / st[a]) % size[a]) as f64 - size[a] as f64 / 2.0;
                            c * c / (size[a] as f64 * size[a] as f64 / 9.0)
                        })
                        .sum();
                    if r < 1.0 { 1.0 } else { 0.0 }
                })
                .collect();
            let v = VolumeRef::new(&data, &size, &spacing);
            let object: Vec<bool> = data.iter().map(|&x| x != 0.0).collect();
            let contour = brute_contour(&object, &size);
            assert_eq!(super::contour(&object, &size, 2).unwrap(), contour);
            let want = exact(&data, &size, &spacing, |j| contour[j]);
            let options = MaurerOptions {
                squared: false,
                use_spacing: true,
                ..Default::default()
            };
            for threads in [1, 3] {
                let got = signed_maurer_distance_map(v, &options, threads).unwrap();
                for i in 0..n {
                    let expected = if object[i] { -want[i] } else { want[i] };
                    assert!((f64::from(got[i]) - expected).abs() < 1e-4, "{size:?} {i}");
                    if contour[i] {
                        assert_eq!(got[i].to_bits(), (-0.0f32).to_bits());
                    }
                }
            }
        }
        // All background: nothing to measure from.
        let zeros = vec![0.0f32; 24];
        let options = MaurerOptions {
            squared: false,
            ..Default::default()
        };
        let got =
            signed_maurer_distance_map(VolumeRef::new(&zeros, &[4, 3, 2], &[1.0; 3]), &options, 1)
                .unwrap();
        assert!(got.iter().all(|&g| g == f64::from(f32::MAX).sqrt() as f32));
    }
}
