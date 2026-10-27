// SPDX-License-Identifier: Apache-2.0
//! Running a 1D filter along every line of an image, as ITK's separable filters do
//! (`RecursiveSeparableImageFilter`, the directional passes of `DiscreteGaussianImageFilter`).
//!
//! Every line is read from `float` into `double`, filtered, and written back as `float`, so a
//! pass is `float → float` with the arithmetic in double, as in ITK's `float` instantiations.
//! Lines are independent, so the result never depends on how they are shared among threads.
//!
//! Lines are filtered in groups of up to [`LANES`] neighbouring lines, laid out as `n` rows of
//! `w` values (position along the line × line), so that the same operation runs on contiguous
//! values and the lines' recursions proceed side by side. Lines along the first axis are
//! contiguous in the image and are transposed into that layout; lines along another axis are
//! strided and are gathered row by row. Each line still sees exactly the operations of a
//! scalar loop.

use larmorx_core::parallel;
use rayon::prelude::*;

use crate::FilterError;

/// How many neighbouring lines are filtered together along the strided axes.
pub(crate) const LANES: usize = 32;
/// The lines one task of a strided axis owns (filtered [`LANES`] at a time).
const GROUP: usize = 8 * LANES;

/// A filter of a group of `w` lines of `n` values: `input[i * w + l]` is value `i` of line
/// `l`; the filter writes `output` the same way (`scratch` has the same size, for its own
/// use).
pub(crate) trait LineFilter: Sync {
    fn filter(&self, input: &[f64], output: &mut [f64], scratch: &mut [f64], n: usize, w: usize);
}

struct Buffers {
    input: Vec<f64>,
    output: Vec<f64>,
    scratch: Vec<f64>,
}

impl Buffers {
    fn new(len: usize) -> Self {
        Buffers {
            input: vec![0.0; len],
            output: vec![0.0; len],
            scratch: vec![0.0; len],
        }
    }
}

/// A voxel type lines are filtered in: read into double, written back with one rounding
/// (`float`), or as is (`double`).
pub(crate) trait LinePixel: Copy + Send + Sync {
    fn to_f64(self) -> f64;
    fn from_f64(v: f64) -> Self;
}

impl LinePixel for f32 {
    #[inline]
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    #[inline]
    fn from_f64(v: f64) -> f32 {
        v as f32
    }
}

impl LinePixel for f64 {
    #[inline]
    fn to_f64(self) -> f64 {
        self
    }
    #[inline]
    fn from_f64(v: f64) -> f64 {
        v
    }
}

/// Filters every line of `data` (an image of `size`, Fortran order) along `axis` with
/// `filter`, in place.
pub(crate) fn filter_lines<T: LinePixel, F: LineFilter>(
    data: &mut [T],
    size: &[usize],
    axis: usize,
    filter: &F,
    n_threads: usize,
) -> Result<(), FilterError> {
    let n = size[axis];
    if n == 0 || data.is_empty() {
        return Ok(());
    }
    let inner: usize = size[..axis].iter().product();
    parallel::with_threads(n_threads, || {
        if inner == 1 {
            // Contiguous lines: groups of up to LANES consecutive lines, transposed into the
            // `n × w` layout.
            data.par_chunks_mut(n * LANES).for_each_init(
                || Buffers::new(n * LANES),
                |b, lines| {
                    let w = lines.len() / n;
                    let len = n * w;
                    for (l, line) in lines.chunks_exact(n).enumerate() {
                        for (i, &v) in line.iter().enumerate() {
                            b.input[i * w + l] = v.to_f64();
                        }
                    }
                    filter.filter(
                        &b.input[..len],
                        &mut b.output[..len],
                        &mut b.scratch[..len],
                        n,
                        w,
                    );
                    for (l, line) in lines.chunks_exact_mut(n).enumerate() {
                        for (i, v) in line.iter_mut().enumerate() {
                            *v = T::from_f64(b.output[i * w + l]);
                        }
                    }
                },
            );
        } else {
            // Strided lines: each task owns the pieces of up to `GROUP` neighbouring lines, one
            // piece per position along the axis, and filters them `LANES` lines at a time (each
            // line is filtered alone, so the grouping changes no value). The tasks are made and
            // run a batch at a time: a reference (16 bytes) per piece for the whole image was
            // up to 0.2 byte per voxel where the rows are shorter than `GROUP` (27 MB on a
            // 588-volume series: the data panel, 2026-10-26, SmoothImage's peak against ANTs').
            let groups = inner.div_ceil(GROUP);
            let budget = (rayon::current_num_threads() * 16).max(16);
            let mut tasks: Vec<Vec<&mut [T]>> = Vec::new();
            for block in data.chunks_mut(inner * n) {
                if groups <= budget {
                    let first = tasks.len();
                    tasks.extend((0..groups).map(|_| Vec::with_capacity(n)));
                    for row in block.chunks_mut(inner) {
                        for (c, piece) in row.chunks_mut(GROUP).enumerate() {
                            tasks[first + c].push(piece);
                        }
                    }
                    if tasks.len() >= budget {
                        run_strided(std::mem::take(&mut tasks), n, filter);
                    }
                } else {
                    // A block of many columns (the last axis of a large image): its rows'
                    // regions of `budget` groups, one batch each.
                    let mut g0 = 0;
                    while g0 < groups {
                        let g1 = (g0 + budget).min(groups);
                        let (lo, hi) = (g0 * GROUP, (g1 * GROUP).min(inner));
                        let mut batch: Vec<Vec<&mut [T]>> =
                            (g0..g1).map(|_| Vec::with_capacity(n)).collect();
                        for row in block.chunks_mut(inner) {
                            for (c, piece) in row[lo..hi].chunks_mut(GROUP).enumerate() {
                                batch[c].push(piece);
                            }
                        }
                        run_strided(batch, n, filter);
                        g0 = g1;
                    }
                }
            }
            run_strided(tasks, n, filter);
        }
    })?;
    Ok(())
}

/// One batch of a strided axis's tasks (see [`filter_lines`]), in parallel on the current pool.
fn run_strided<T: LinePixel, F: LineFilter>(tasks: Vec<Vec<&mut [T]>>, n: usize, filter: &F) {
    tasks.into_par_iter().for_each_init(
        || Buffers::new(n * LANES),
        |b, mut rows| {
            let width = rows[0].len();
            for start in (0..width).step_by(LANES) {
                let w = LANES.min(width - start);
                let len = n * w;
                for (i, row) in rows.iter().enumerate() {
                    let src = &row[start..start + w];
                    for (x, &v) in b.input[i * w..(i + 1) * w].iter_mut().zip(src) {
                        *x = v.to_f64();
                    }
                }
                filter.filter(
                    &b.input[..len],
                    &mut b.output[..len],
                    &mut b.scratch[..len],
                    n,
                    w,
                );
                for (i, row) in rows.iter_mut().enumerate() {
                    let dst = &mut row[start..start + w];
                    for (v, &x) in dst.iter_mut().zip(b.output[i * w..(i + 1) * w].iter()) {
                        *v = T::from_f64(x);
                    }
                }
            }
        },
    );
}

/// Where a line lies in the image: its first voxel's index and the distance in memory between
/// its voxels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Line {
    pub start: usize,
    pub stride: usize,
}

/// Calls `f(state, line, where)` on every line of `data` (an image of `size`, Fortran order)
/// along `axis`, with the line's values in a contiguous slice that `f` may change in place.
/// Lines along the first axis are passed as they lie in memory; lines along another axis are
/// gathered in groups of up to [`LANES`] neighbouring lines (tasks of up to `GROUP`) and
/// scattered back afterwards.
/// Each line is seen once, so the result never depends on the thread count (`init` makes
/// each worker's scratch state).
pub(crate) fn for_each_line<T, S, I, F>(
    data: &mut [T],
    size: &[usize],
    axis: usize,
    n_threads: usize,
    init: I,
    f: F,
) -> Result<(), FilterError>
where
    T: Copy + Send + Sync + Default,
    I: Fn() -> S + Sync + Send,
    F: Fn(&mut S, &mut [T], Line) + Sync + Send,
{
    let n = size[axis];
    if n == 0 || data.is_empty() {
        return Ok(());
    }
    let inner: usize = size[..axis].iter().product();
    parallel::with_threads(n_threads, || {
        if inner == 1 {
            // Contiguous lines, a few per task.
            let per_task = (4096 / n).max(1);
            data.par_chunks_mut(n * per_task).enumerate().for_each_init(
                &init,
                |state, (c, lines)| {
                    for (l, line) in lines.chunks_exact_mut(n).enumerate() {
                        let start = (c * per_task + l) * n;
                        f(state, line, Line { start, stride: 1 });
                    }
                },
            );
        } else {
            // Strided lines: each task owns the pieces of up to `GROUP` neighbouring lines, one
            // piece per position along the axis, and gathers them `LANES` at a time (as in
            // `filter_lines`: one reference per `GROUP` voxels of a row, not per `LANES`).
            let mut tasks: Vec<(usize, Vec<&mut [T]>)> = Vec::new();
            for (b, block) in data.chunks_mut(inner * n).enumerate() {
                let first = tasks.len();
                tasks.extend(
                    (0..inner.div_ceil(GROUP))
                        .map(|c| (b * inner * n + c * GROUP, Vec::with_capacity(n))),
                );
                for row in block.chunks_mut(inner) {
                    for (c, piece) in row.chunks_mut(GROUP).enumerate() {
                        tasks[first + c].1.push(piece);
                    }
                }
            }
            tasks.into_par_iter().for_each_init(
                || (init(), vec![T::default(); n * LANES]),
                |(state, buf), (start, mut rows)| {
                    let width = rows[0].len();
                    for s0 in (0..width).step_by(LANES) {
                        let w = LANES.min(width - s0);
                        for (i, row) in rows.iter().enumerate() {
                            for (l, &v) in row[s0..s0 + w].iter().enumerate() {
                                buf[l * n + i] = v;
                            }
                        }
                        for l in 0..w {
                            let line = Line {
                                start: start + s0 + l,
                                stride: inner,
                            };
                            f(state, &mut buf[l * n..(l + 1) * n], line);
                        }
                        for (i, row) in rows.iter_mut().enumerate() {
                            for (l, v) in row[s0..s0 + w].iter_mut().enumerate() {
                                *v = buf[l * n + i];
                            }
                        }
                    }
                },
            );
        }
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_each_line_sees_every_line_once_with_its_position() {
        for size in [
            vec![7usize, 5, 3],
            vec![70, 3, 2, 4],
            vec![1, 9, 40],
            vec![6],
        ] {
            let len: usize = size.iter().product();
            let data: Vec<u32> = (0..len as u32).collect();
            for axis in 0..size.len() {
                for threads in [1, 3] {
                    let mut d = data.clone();
                    for_each_line(
                        &mut d,
                        &size,
                        axis,
                        threads,
                        || (),
                        |_, line, at| {
                            for (i, v) in line.iter_mut().enumerate() {
                                // Each value is its own index: check it, then mark it.
                                assert_eq!(*v as usize, at.start + i * at.stride);
                                *v += 1_000_000;
                            }
                        },
                    )
                    .unwrap();
                    assert!(d.iter().zip(&data).all(|(a, b)| *a == b + 1_000_000));
                }
            }
        }
    }

    /// A filter whose output at `i` is a sum of the line's values weighted by position, so
    /// that every value's line and position matter.
    struct Weighted;

    impl LineFilter for Weighted {
        fn filter(&self, input: &[f64], output: &mut [f64], _: &mut [f64], n: usize, w: usize) {
            for l in 0..w {
                let mut acc = 0.0;
                for i in 0..n {
                    acc += input[i * w + l] * (i + 1) as f64;
                    output[i * w + l] = acc;
                }
            }
        }
    }

    fn reference(data: &[f32], size: &[usize], axis: usize) -> Vec<f32> {
        let strides = crate::volume::strides(size);
        let mut out = data.to_vec();
        let n = size[axis];
        for start in 0..data.len() {
            if !(start / strides[axis]).is_multiple_of(n) {
                continue;
            }
            let mut acc = 0.0f64;
            for i in 0..n {
                let k = start + i * strides[axis];
                acc += f64::from(data[k]) * (i + 1) as f64;
                out[k] = acc as f32;
            }
        }
        out
    }

    #[test]
    fn every_axis_matches_a_scalar_loop_for_any_thread_count() {
        for size in [
            vec![7usize, 5, 3],
            vec![70, 3, 2, 4],
            vec![1, 9, 40],
            vec![6],
        ] {
            let len: usize = size.iter().product();
            let data: Vec<f32> = (0..len).map(|i| ((i * 37) % 101) as f32 * 0.25).collect();
            for axis in 0..size.len() {
                let expected = reference(&data, &size, axis);
                for threads in [1, 3] {
                    let mut d = data.clone();
                    filter_lines(&mut d, &size, axis, &Weighted, threads).unwrap();
                    assert_eq!(d, expected, "size {size:?} axis {axis} threads {threads}");
                }
            }
        }
    }
}
