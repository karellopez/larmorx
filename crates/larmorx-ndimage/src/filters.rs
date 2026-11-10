// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause AND MIT
//! The minimum, maximum and rank filters, ported from SciPy 1.18.1
//! `scipy/ndimage/src/ni_filters.c` (`NI_MinOrMaxFilter`, `CASE_MIN_OR_MAX_POINT`,
//! `NI_MinOrMaxFilter1D`, `NI_Select`, `NI_RankFilter`, `CASE_RANK_POINT`, `CASE_FILTER_OUT*`)
//! and `ni_support.c` (`NI_ExtendLine`); BSD-3-Clause, the notice is in the crate's root.
//!
//! `rank_filter_1d` is ported from SciPy 1.18.1 `scipy/ndimage/src/_rank_filter_1d.cpp`:
//! Copyright (c) 2011 ashelly.myopenid.com under the MIT licence
//! (<http://www.opensource.org/licenses/mit-license>), modified in 2024 by Gideon Genadi Kogan.
//! Permission is hereby granted, free of charge, to any person obtaining a copy of this software
//! and associated documentation files (the "Software"), to deal in the Software without
//! restriction, including without limitation the rights to use, copy, modify, merge, publish,
//! distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
//! Software is furnished to do so, subject to the following conditions: The above copyright
//! notice and this permission notice shall be included in all copies or substantial portions of
//! the Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
//! PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
//! LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
//! OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//! DEALINGS IN THE SOFTWARE.
//!
//! The Python wrappers' argument handling (`_min_or_max_filter`, `_rank_filter`: the separable
//! case, the rank of a median or percentile, the shortcuts to the minimum and maximum filters)
//! is `lx.ndimage`'s.

use rayon::prelude::*;

use crate::neighbourhood::{BORDER, Neighbourhood};
use crate::{Extend, Footprint, Layout, NdElement, NdimageError, check_origins, unravel};

fn check(input: &[impl Copy], layout: &Layout, fshape: &[usize]) -> Result<(), NdimageError> {
    if input.len() < layout.span() || layout.strides.len() != layout.ndim() {
        return Err(NdimageError::Shape(
            "input buffer and layout disagree".into(),
        ));
    }
    if fshape.len() != layout.ndim() {
        return Err(NdimageError::Shape(format!(
            "footprint.ndim ({}) must match len(axes) ({})",
            fshape.len(),
            layout.ndim()
        )));
    }
    Ok(())
}

/// Runs `point(position, output)` for every element of a C-ordered output of `shape`, the lines
/// along the last axis split over the current rayon pool; `scratch` makes each task's buffer.
fn for_each_point<O, S, MkS, F>(shape: &[usize], out: &mut [O], scratch: MkS, point: F)
where
    O: Send,
    S: Send,
    MkS: Fn() -> S + Sync + Send,
    F: Fn(&[usize], &mut O, &mut S) + Sync + Send,
{
    let ndim = shape.len();
    if out.is_empty() {
        return;
    }
    if ndim == 0 {
        let mut s = scratch();
        point(&[], &mut out[0], &mut s);
        return;
    }
    let line = shape[ndim - 1];
    out.par_chunks_mut(line).enumerate().for_each_init(
        || (scratch(), vec![0usize; ndim]),
        |(s, p), (li, chunk)| {
            unravel(li * line, shape, p);
            for (j, o) in chunk.iter_mut().enumerate() {
                p[ndim - 1] = j;
                point(p, o, s);
            }
        },
    );
}

/// `NI_MinOrMaxFilter`: the minimum (`minimum`) or maximum of the input over `footprint` around
/// every element, with an optional non-flat structuring element (`structure`, C-ordered over the
/// footprint's shape, its values added to the input's in the input's type: subtracted for the
/// minimum), `mode` and `cval` beyond the edges, `origins` per axis. The result is C-ordered.
///
/// SciPy's order is kept (`CASE_MIN_OR_MAX_POINT`, whose running result is the caller's
/// `double`): the first element read starts the result as a double (plus `structure[first]`, in
/// double, not cast back), then each next one (with the structure value cast to the type and
/// added in the type) replaces it when strictly smaller (larger) as a double, so a NaN first stays
/// and a later NaN is skipped; the result is cast to the output type `O` by
/// `CASE_FILTER_OUT_SAFE` (`O = f64` gives SciPy's double itself).
#[allow(clippy::too_many_arguments)]
pub fn min_or_max_filter<T: NdElement, O: NdElement>(
    input: &[T],
    layout: &Layout,
    footprint: &Footprint,
    structure: Option<&[f64]>,
    mode: Extend,
    cval: f64,
    origins: &[isize],
    minimum: bool,
) -> Result<Vec<O>, NdimageError> {
    check(input, layout, &footprint.shape)?;
    check_origins(&footprint.shape, origins)?;
    if footprint.count() == 0 {
        return Err(NdimageError::EmptyFootprint);
    }
    let ss: Option<Vec<f64>> = match structure {
        None => None,
        Some(st) => {
            if st.len() != footprint.mask.len() {
                return Err(NdimageError::Shape(
                    "structure array has incorrect shape".into(),
                ));
            }
            Some(
                st.iter()
                    .zip(&footprint.mask)
                    .filter(|(_, m)| **m)
                    .map(|(&v, _)| if minimum { -v } else { v })
                    .collect(),
            )
        }
    };
    let nb = Neighbourhood::new(layout, footprint, origins, mode);
    let n = nb.len();
    let cv = T::from_f64(cval);
    // Where no NaN can arise (an input without NaN, a finite structure), the interior's
    // values are computed with plain arithmetic and the structure cast to the type once: the
    // same values as `extreme`'s (SSE's results differ from IEEE's only in NaNs).
    let sst: Option<Vec<T>> = ss
        .as_ref()
        .map(|v| v.iter().map(|&d| T::from_f64(d)).collect());
    let plain = !T::any_nan(input)
        && ss.as_ref().is_none_or(|v| v[0].is_finite())
        && sst.as_ref().is_none_or(|v| v.iter().all(|t| t.finite()));
    // near the border, the constant modes' value must not be a NaN either
    let plain_border = plain && !T::any_nan(&[cv]);
    let s0 = ss.as_ref().map(|v| v[0]);
    // One position's value (the module's rules).
    let point = |p: &[usize], offs: &mut Vec<usize>| -> f64 {
        if nb.interior(p) {
            let base = nb.base(p) as isize;
            let rel = nb.relative();
            if plain {
                extreme_plain(input, base, rel, s0, sst.as_deref(), minimum)
            } else {
                extreme(n, ss.as_deref(), minimum, |e| {
                    input[(base + rel[e]) as usize]
                })
            }
        } else if plain_border && p.len() == 3 {
            let (axes, strides) = nb.axes3(p);
            extreme_plain_3d(
                input,
                &footprint.mask,
                axes,
                strides,
                cv,
                s0,
                sst.as_deref(),
                minimum,
            )
        } else {
            nb.offsets(p, offs);
            extreme(n, ss.as_deref(), minimum, |e| {
                let off = offs[e];
                if off == BORDER { cv } else { input[off] }
            })
        }
    };
    let mut out = vec![O::default(); layout.len()];
    let ndim = layout.ndim();
    if plain && ndim > 0 && nb.last_stride() == 1 && !out.is_empty() {
        // A contiguous last axis: each line's interior positions all at once, element by
        // element in the footprint's order (every position sees its elements in that order, as
        // one at a time), the others one at a time.
        let line = layout.shape[ndim - 1];
        let rel = nb.relative();
        out.par_chunks_mut(line).enumerate().for_each_init(
            || (vec![0usize; n], vec![0usize; ndim], Vec::<f64>::new()),
            |(offs, p, acc), (li, chunk)| {
                unravel(li * line, &layout.shape, p);
                let (lo, hi) = nb.line_interior(p);
                for x in (0..lo).chain(hi.max(lo)..line) {
                    p[ndim - 1] = x;
                    chunk[x] = O::from_f64_safe(point(p, offs));
                }
                if lo < hi {
                    p[ndim - 1] = lo;
                    let base = nb.base(p) as isize;
                    let len = hi - lo;
                    lines_plain(input, base, rel, len, s0, sst.as_deref(), minimum, acc);
                    for (o, &a) in chunk[lo..hi].iter_mut().zip(acc.iter()) {
                        *o = O::from_f64_safe(a);
                    }
                }
            },
        );
        return Ok(out);
    }
    for_each_point(
        &layout.shape,
        &mut out,
        || vec![0usize; n],
        |p, o, offs| {
            *o = O::from_f64_safe(point(p, offs));
        },
    );
    Ok(out)
}

/// [`extreme_plain`] at `len` consecutive interior positions of a contiguous last axis (the
/// first at `base`), into `acc`: element by element in the footprint's order, every position
/// updated by each element in turn.
#[allow(clippy::too_many_arguments)]
fn lines_plain<T: NdElement>(
    input: &[T],
    base: isize,
    rel: &[isize],
    len: usize,
    s0: Option<f64>,
    sst: Option<&[T]>,
    minimum: bool,
    acc: &mut Vec<f64>,
) {
    acc.clear();
    let start = (base + rel[0]) as usize;
    acc.extend(input[start..start + len].iter().map(|x| x.to_f64_plain()));
    if let Some(s0) = s0 {
        acc.iter_mut().for_each(|a| *a += s0);
    }
    for e in 1..rel.len() {
        let start = (base + rel[e]) as usize;
        let src = &input[start..start + len];
        match (sst, minimum) {
            (Some(sst), true) => {
                let s = sst[e];
                for (a, &x) in acc.iter_mut().zip(src) {
                    let t = x.add_plain(s).to_f64_plain();
                    *a = if t < *a { t } else { *a };
                }
            }
            (Some(sst), false) => {
                let s = sst[e];
                for (a, &x) in acc.iter_mut().zip(src) {
                    let t = x.add_plain(s).to_f64_plain();
                    *a = if t > *a { t } else { *a };
                }
            }
            (None, true) => {
                for (a, &x) in acc.iter_mut().zip(src) {
                    let t = x.to_f64_plain();
                    *a = if t < *a { t } else { *a };
                }
            }
            (None, false) => {
                for (a, &x) in acc.iter_mut().zip(src) {
                    let t = x.to_f64_plain();
                    *a = if t > *a { t } else { *a };
                }
            }
        }
    }
}

/// [`extreme_plain`] at a 3-D position near the border: the footprint visited in its order
/// (C order of `mask`), each element at its mapped coordinates (`axes`, [`BORDER`] giving
/// `cv`).
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn extreme_plain_3d<T: NdElement>(
    input: &[T],
    mask: &[bool],
    axes: [&[usize]; 3],
    strides: [usize; 3],
    cv: T,
    s0: Option<f64>,
    sst: Option<&[T]>,
    minimum: bool,
) -> f64 {
    let mut res = 0.0f64;
    let mut e = 0usize;
    let mut lin = 0usize;
    for &c0 in axes[0] {
        for &c1 in axes[1] {
            let row = if c0 == BORDER || c1 == BORDER {
                BORDER
            } else {
                c0 * strides[0] + c1 * strides[1]
            };
            for &c2 in axes[2] {
                let on = mask[lin];
                lin += 1;
                if !on {
                    continue;
                }
                let v = if row == BORDER || c2 == BORDER {
                    cv
                } else {
                    input[row + c2 * strides[2]]
                };
                if e == 0 {
                    res = v.to_f64_plain();
                    if let Some(s0) = s0 {
                        res += s0;
                    }
                } else {
                    let t = match sst {
                        Some(sst) => v.add_plain(sst[e]),
                        None => v,
                    }
                    .to_f64_plain();
                    if minimum {
                        if t < res {
                            res = t;
                        }
                    } else if t > res {
                        res = t;
                    }
                }
                e += 1;
            }
        }
    }
    res
}

/// `NI_MinOrMaxFilter`'s value at one position from the footprint's `n` elements (`get(e)`), in
/// the footprint's order (the module's rules: a double running result, the structure's values
/// added).
#[inline(always)]
fn extreme<T: NdElement>(
    n: usize,
    ss: Option<&[f64]>,
    minimum: bool,
    get: impl Fn(usize) -> T,
) -> f64 {
    let mut res = get(0).to_f64();
    if let Some(ss) = ss {
        res = larmorx_core::numpy::NpFloat::add(res, ss[0]);
    }
    for e in 1..n {
        let mut tmp = get(e);
        if let Some(ss) = ss {
            tmp = tmp.add_cast(ss[e]);
        }
        let t = tmp.to_f64();
        if minimum {
            if t < res {
                res = t;
            }
        } else if t > res {
            res = t;
        }
    }
    res
}

/// [`extreme`] at an interior position where no NaN can arise: the first element plus the
/// structure's first value in `f64`, then each element plus the structure's value cast to the
/// type (`sst`), compared in `f64`.
#[inline(always)]
fn extreme_plain<T: NdElement>(
    input: &[T],
    base: isize,
    rel: &[isize],
    s0: Option<f64>,
    sst: Option<&[T]>,
    minimum: bool,
) -> f64 {
    let mut res = input[(base + rel[0]) as usize].to_f64_plain();
    if let Some(s0) = s0 {
        res += s0;
    }
    match sst {
        Some(sst) => {
            for (&r, &s) in rel[1..].iter().zip(&sst[1..]) {
                let t = input[(base + r) as usize].add_plain(s).to_f64_plain();
                if minimum {
                    if t < res {
                        res = t;
                    }
                } else if t > res {
                    res = t;
                }
            }
        }
        None => {
            for &r in &rel[1..] {
                let t = input[(base + r) as usize].to_f64_plain();
                if minimum {
                    if t < res {
                        res = t;
                    }
                } else if t > res {
                    res = t;
                }
            }
        }
    }
    res
}

/// `NI_ExtendLine`: the line's `size_before` and `size_after` extensions in `buffer` (the line
/// itself at `buffer[size_before..size_before + len]`).
fn extend_line(buffer: &mut [f64], len: usize, before: usize, after: usize, mode: Extend, cv: f64) {
    let mode = if len == 1 && mode == Extend::Mirror {
        Extend::Nearest
    } else {
        mode
    };
    let first = before;
    let last = before + len;
    match mode {
        Extend::Nearest => {
            let v = buffer[first];
            buffer[..first].iter_mut().for_each(|x| *x = v);
            let v = buffer[last - 1];
            buffer[last..last + after].iter_mut().for_each(|x| *x = v);
        }
        Extend::Wrap => {
            // backwards from first - 1, reading from last - 1 backwards (through the extension
            // once the line is exhausted)
            let mut src = last as isize - 1;
            let mut dst = first as isize - 1;
            for _ in 0..before {
                buffer[dst as usize] = buffer[src as usize];
                dst -= 1;
                src -= 1;
            }
            let mut src = first;
            let mut dst = last;
            for _ in 0..after {
                buffer[dst] = buffer[src];
                dst += 1;
                src += 1;
            }
        }
        Extend::Reflect | Extend::Mirror => {
            let skip = usize::from(mode == Extend::Mirror);
            let mut size_before = before;
            let mut src = (first + skip) as isize;
            let mut dst = first as isize - 1;
            while size_before > 0 && src < last as isize {
                buffer[dst as usize] = buffer[src as usize];
                dst -= 1;
                src += 1;
                size_before -= 1;
            }
            let mut src = last as isize - 1 - skip as isize;
            while size_before > 0 {
                buffer[dst as usize] = buffer[src as usize];
                dst -= 1;
                src -= 1;
                size_before -= 1;
            }
            let mut size_after = after;
            let mut src = last as isize - 1 - skip as isize;
            let mut dst = last;
            while size_after > 0 && src >= first as isize {
                buffer[dst] = buffer[src as usize];
                dst += 1;
                src -= 1;
                size_after -= 1;
            }
            let mut src = first + skip;
            while size_after > 0 {
                buffer[dst] = buffer[src];
                dst += 1;
                src += 1;
                size_after -= 1;
            }
        }
        Extend::Constant => {
            buffer[..first].iter_mut().for_each(|x| *x = cv);
            buffer[last..last + after].iter_mut().for_each(|x| *x = cv);
        }
    }
}

/// `NI_Correlate1D` (`correlate1d`, and so `gaussian_filter1d`): the correlation of every line
/// along `axis` with `weights` (`size1 = len / 2 + origin` of them before the element), each line
/// read into doubles and extended by `mode` (`NI_ExtendLine`, `cval` for the constant modes).
/// An odd kernel whose mirrored weights differ by at most `DBL_EPSILON` is symmetric:
/// `x[0]·w[0]`, then `+= (x[-j] + x[j])·w[-j]` from the outermost pair in; anti-symmetric
/// (`|w[j] + w[-j]| <= DBL_EPSILON`) the same with `x[-j] - x[j]`; any other: the last term
/// first, then the others from the first. Products and sums round separately (SciPy's C, no
/// FMA). Returns the doubles SciPy casts to the output type (`CASE_COPY_LINE_TO_DATA`), C-ordered.
#[allow(clippy::too_many_arguments)]
pub fn correlate1d<T: NdElement>(
    input: &[T],
    layout: &Layout,
    weights: &[f64],
    axis: usize,
    mode: Extend,
    cval: f64,
    origin: isize,
) -> Result<Vec<f64>, NdimageError> {
    if input.len() < layout.span() || axis >= layout.ndim().max(1) {
        return Err(NdimageError::Shape(
            "input buffer and layout disagree".into(),
        ));
    }
    let filter_size = weights.len();
    if filter_size < 1 {
        return Err(NdimageError::Shape("no filter weights given".into()));
    }
    let size1 = (filter_size / 2) as isize;
    let size2 = filter_size as isize - size1 - 1;
    if size1 + origin < 0 || size2 - origin < 0 {
        return Err(NdimageError::Origin);
    }
    let mut out = vec![0.0f64; layout.len()];
    if layout.ndim() == 0 {
        return Ok(out);
    }
    let len = layout.shape[axis];
    if len == 0 || out.is_empty() {
        return Ok(out);
    }
    let mut symmetric = 0i32;
    if filter_size & 1 == 1 {
        let s1 = size1 as usize;
        symmetric = 1;
        if (1..=s1).any(|i| (weights[s1 + i] - weights[s1 - i]).abs() > f64::EPSILON) {
            symmetric = -1;
            if (1..=s1).any(|i| (weights[s1 + i] + weights[s1 - i]).abs() > f64::EPSILON) {
                symmetric = 0;
            }
        }
    }
    let before = (size1 + origin) as usize;
    let after = (size2 - origin) as usize;
    let s1 = size1 as usize;
    let s2 = size2 as usize;
    let ostrides = crate::c_strides(&layout.shape);
    let other: Vec<usize> = (0..layout.ndim()).filter(|&d| d != axis).collect();
    let other_shape: Vec<usize> = other.iter().map(|&d| layout.shape[d]).collect();
    let n_lines: usize = other_shape.iter().product();
    let lines: Vec<Vec<f64>> = (0..n_lines)
        .into_par_iter()
        .map_init(
            || {
                (
                    vec![0.0f64; before + len + after],
                    vec![0usize; other.len()],
                )
            },
            |(buf, idx), li| {
                unravel(li, &other_shape, idx);
                let base: usize = other
                    .iter()
                    .zip(idx.iter())
                    .map(|(&d, &i)| i * layout.strides[d])
                    .sum();
                for j in 0..len {
                    buf[before + j] = input[base + j * layout.strides[axis]].to_f64();
                }
                extend_line(buf, len, before, after, mode, cval);
                (0..len)
                    .map(|ll| {
                        // iline = line + size1 + ll: buf[c + jj] is iline[jj].
                        let c = s1 + ll;
                        if symmetric > 0 {
                            let mut v = buf[c] * weights[s1];
                            for k in 0..s1 {
                                let d = s1 - k;
                                v += (buf[c - d] + buf[c + d]) * weights[k];
                            }
                            v
                        } else if symmetric < 0 {
                            let mut v = buf[c] * weights[s1];
                            for k in 0..s1 {
                                let d = s1 - k;
                                v += (buf[c - d] - buf[c + d]) * weights[k];
                            }
                            v
                        } else {
                            let mut v = buf[c + s2] * weights[s1 + s2];
                            for (k, w) in weights.iter().enumerate().take(filter_size - 1) {
                                v += buf[c + k - s1] * w;
                            }
                            v
                        }
                    })
                    .collect()
            },
        )
        .collect();
    let mut idx = vec![0usize; other.len()];
    for (li, line) in lines.into_iter().enumerate() {
        unravel(li, &other_shape, &mut idx);
        let base: usize = other
            .iter()
            .zip(idx.iter())
            .map(|(&d, &i)| i * ostrides[d])
            .sum();
        for (j, v) in line.into_iter().enumerate() {
            out[base + j * ostrides[axis]] = v;
        }
    }
    Ok(out)
}

/// `NI_MinOrMaxFilter1D` (`minimum_filter1d`, `maximum_filter1d`): the minimum or maximum over
/// `size` elements along `axis` (`size / 2 + origin` of them before the element), every line
/// read into doubles, extended by `mode` (`NI_ExtendLine`, `cval` for the constant modes),
/// filtered with Richard Harter's running minimum (SciPy's comparisons: `<=` and `>=` replace,
/// so of equal values the latest is kept) and cast to the output type `O` (C's cast,
/// `CASE_COPY_LINE_TO_DATA`). The result is C-ordered.
#[allow(clippy::too_many_arguments)]
pub fn min_or_max_filter1d<T: NdElement, O: NdElement>(
    input: &[T],
    layout: &Layout,
    size: usize,
    axis: usize,
    mode: Extend,
    cval: f64,
    origin: isize,
    minimum: bool,
) -> Result<Vec<O>, NdimageError> {
    if input.len() < layout.span() || axis >= layout.ndim().max(1) {
        return Err(NdimageError::Shape(
            "input buffer and layout disagree".into(),
        ));
    }
    if size < 1 {
        return Err(NdimageError::Shape("incorrect filter size".into()));
    }
    let s2 = (size / 2) as isize;
    if s2 + origin < 0 || s2 + origin >= size as isize {
        return Err(NdimageError::Origin);
    }
    let mut out = vec![O::default(); layout.len()];
    if layout.ndim() == 0 {
        out[0] = O::from_f64(input[0].to_f64());
        return Ok(out);
    }
    let len = layout.shape[axis];
    if len == 0 || out.is_empty() {
        return Ok(out);
    }
    let size1 = (size / 2) as isize;
    let size2 = size as isize - size1 - 1;
    let before = (size1 + origin) as usize;
    let after = (size2 - origin) as usize;
    let ostrides = crate::c_strides(&layout.shape);
    // The lines: every position with index 0 on `axis`, in C order of the other axes.
    let other: Vec<usize> = (0..layout.ndim()).filter(|&d| d != axis).collect();
    let other_shape: Vec<usize> = other.iter().map(|&d| layout.shape[d]).collect();
    let n_lines: usize = other_shape.iter().product();
    let lines: Vec<Vec<O>> = (0..n_lines)
        .into_par_iter()
        .map_init(
            || {
                (
                    vec![0.0f64; before + len + after],
                    vec![0usize; other.len()],
                    Vec::<(f64, usize)>::with_capacity(size),
                )
            },
            |(buf, idx, ring), li| {
                unravel(li, &other_shape, idx);
                let base: usize = other
                    .iter()
                    .zip(idx.iter())
                    .map(|(&d, &i)| i * layout.strides[d])
                    .sum();
                for j in 0..len {
                    buf[before + j] = input[base + j * layout.strides[axis]].to_f64();
                }
                // NI_InitLineBuffer keeps cval as a double (not cast to the type).
                extend_line(buf, len, before, after, mode, cval);
                let mut line = vec![O::default(); len];
                if size == 1 {
                    for j in 0..len {
                        line[j] = O::from_f64(buf[before + j]);
                    }
                    return line;
                }
                harter(buf, size, len, minimum, ring, |j, v| {
                    line[j] = O::from_f64(v)
                });
                line
            },
        )
        .collect();
    // scatter the lines into the C-ordered output
    let mut idx = vec![0usize; other.len()];
    for (li, line) in lines.into_iter().enumerate() {
        unravel(li, &other_shape, &mut idx);
        let base: usize = other
            .iter()
            .zip(idx.iter())
            .map(|(&d, &i)| i * ostrides[d])
            .sum();
        for (j, v) in line.into_iter().enumerate() {
            out[base + j * ostrides[axis]] = v;
        }
    }
    Ok(out)
}

/// Richard Harter's running minimum (maximum) of `NI_MinOrMaxFilter1D` over the extended line
/// `buf` (`size + len - 1` values): a deque of (value, death) pairs in a ring of `size`.
fn harter(
    buf: &[f64],
    size: usize,
    len: usize,
    minimum: bool,
    ring: &mut Vec<(f64, usize)>,
    mut emit: impl FnMut(usize, f64),
) {
    ring.clear();
    ring.resize(size, (0.0, 0));
    let inc = |p: usize| if p + 1 >= size { 0 } else { p + 1 };
    let dec = |p: usize| if p == 0 { size - 1 } else { p - 1 };
    let mut minpair = 0usize;
    ring[0] = (buf[0], size);
    let mut last = 0usize;
    let mut j = 0usize;
    for ll in 1..size + len - 1 {
        let val = buf[ll];
        if ring[minpair].1 == ll {
            minpair = inc(minpair);
        }
        if (minimum && val <= ring[minpair].0) || (!minimum && val >= ring[minpair].0) {
            ring[minpair] = (val, ll + size);
            last = minpair;
        } else {
            while (minimum && ring[last].0 >= val) || (!minimum && ring[last].0 <= val) {
                last = dec(last);
            }
            last = inc(last);
            ring[last] = (val, ll + size);
        }
        if ll >= size - 1 {
            emit(j, ring[minpair].0);
            j += 1;
        }
    }
}

/// `NI_Select`: SciPy's quickselect of the `rank`-th value of `buffer[min..=max]` (Hoare's
/// partition around the first value), the same element SciPy returns, ties, signed zeros and
/// NaNs included.
fn ni_select(buffer: &mut [f64], mut min: usize, mut max: usize, mut rank: usize) -> f64 {
    loop {
        if min == max {
            return buffer[min];
        }
        let x = buffer[min];
        let mut ii = min as isize - 1;
        let mut jj = max as isize + 1;
        loop {
            loop {
                jj -= 1;
                if !(buffer[jj as usize] > x) {
                    break;
                }
            }
            loop {
                ii += 1;
                if !(buffer[ii as usize] < x) {
                    break;
                }
            }
            if ii < jj {
                buffer.swap(ii as usize, jj as usize);
            } else {
                break;
            }
        }
        let jj = jj as usize;
        let count = jj - min + 1;
        if rank < count {
            max = jj;
        } else {
            min = jj + 1;
            rank -= count;
        }
    }
}

/// `NI_RankFilter`: the `rank`-th smallest (0-based) of the input over `footprint` around every
/// element (the footprint's elements read into doubles in C order, `(type)cval` beyond the edges
/// in the constant modes), selected by [`ni_select`], cast to the type and stored in the
/// output type `O` as SciPy's `CASE_FILTER_OUT_SAFE` does. The result is C-ordered. `lx.ndimage`
/// turns the median and the percentiles into ranks and the extreme ranks into
/// [`min_or_max_filter`], and sends 1-D inputs to [`rank_filter_1d`], as SciPy does.
pub fn rank_filter<T: NdElement, O: NdElement>(
    input: &[T],
    layout: &Layout,
    rank: usize,
    footprint: &Footprint,
    mode: Extend,
    cval: f64,
    origins: &[isize],
) -> Result<Vec<O>, NdimageError> {
    check(input, layout, &footprint.shape)?;
    check_origins(&footprint.shape, origins)?;
    let n = footprint.count();
    if rank >= n {
        return Err(NdimageError::Rank);
    }
    let nb = Neighbourhood::new(layout, footprint, origins, mode);
    let cv = T::from_f64(cval).to_f64();
    let mut out = vec![O::default(); layout.len()];
    for_each_point(
        &layout.shape,
        &mut out,
        || (vec![0usize; n], vec![0.0f64; n]),
        |p, o, (offs, buf)| {
            if nb.interior(p) {
                let base = nb.base(p) as isize;
                for (b, &r) in buf.iter_mut().zip(nb.relative()) {
                    *b = input[(base + r) as usize].to_f64();
                }
            } else {
                nb.offsets(p, offs);
                for (b, &off) in buf.iter_mut().zip(offs.iter()) {
                    *b = if off == BORDER {
                        cv
                    } else {
                        input[off].to_f64()
                    };
                }
            }
            let v = T::from_f64(ni_select(buf, 0, n - 1, rank));
            *o = O::from_f64_safe(v.to_f64());
        },
    );
    Ok(out)
}

/// The values `_rank_filter_1d` runs on: `float32`, `float64` and `int64` (SciPy casts the other
/// integer types and booleans to `int64`).
pub trait Rank1d: Copy + PartialOrd + Default + Send + Sync + 'static {}
impl Rank1d for f32 {}
impl Rank1d for f64 {}
impl Rank1d for i64 {}

/// Ashelly's mediator of `_rank_filter_1d.cpp`: a max-heap, the rank element and a min-heap over
/// a circular window, indices into `data` (`heap` stored from index `-rank`).
struct Mediator {
    pos: Vec<i64>,
    heap: Vec<i64>,
    rank: i64,
    n: usize,
    idx: usize,
    min_ct: i64,
    max_ct: i64,
}

impl Mediator {
    fn new(n_items: usize, rank: usize) -> Mediator {
        let r = rank as i64;
        let mut m = Mediator {
            pos: vec![0; n_items],
            heap: vec![0; n_items],
            rank: r,
            n: n_items,
            idx: 0,
            min_ct: n_items as i64 - r - 1,
            max_ct: r,
        };
        for k in (0..n_items).rev() {
            m.pos[k] = k as i64 - r;
            let p = m.pos[k];
            m.set_heap(p, k as i64);
        }
        m
    }
    #[inline]
    fn heap_at(&self, i: i64) -> i64 {
        self.heap[(i + self.rank) as usize]
    }
    #[inline]
    fn set_heap(&mut self, i: i64, v: i64) {
        self.heap[(i + self.rank) as usize] = v;
    }
    #[inline]
    fn less<T: Rank1d>(&self, data: &[T], i: i64, j: i64) -> bool {
        data[self.heap_at(i) as usize] < data[self.heap_at(j) as usize]
    }
    fn exchange(&mut self, i: i64, j: i64) -> bool {
        let t = self.heap_at(i);
        let hj = self.heap_at(j);
        self.set_heap(i, hj);
        self.set_heap(j, t);
        let (hi, hj) = (self.heap_at(i) as usize, self.heap_at(j) as usize);
        self.pos[hi] = i;
        self.pos[hj] = j;
        true
    }
    fn cmp_exch<T: Rank1d>(&mut self, data: &[T], i: i64, j: i64) -> bool {
        self.less(data, i, j) && self.exchange(i, j)
    }
    fn min_sort_down<T: Rank1d>(&mut self, data: &[T], mut i: i64) {
        i *= 2;
        while i <= self.min_ct {
            if i < self.min_ct && self.less(data, i + 1, i) {
                i += 1;
            }
            if !self.cmp_exch(data, i, i / 2) {
                break;
            }
            i *= 2;
        }
    }
    fn max_sort_down<T: Rank1d>(&mut self, data: &[T], mut i: i64) {
        i *= 2;
        while i >= -self.max_ct {
            if i > -self.max_ct && self.less(data, i, i - 1) {
                i -= 1;
            }
            if !self.cmp_exch(data, i / 2, i) {
                break;
            }
            i *= 2;
        }
    }
    fn min_sort_up<T: Rank1d>(&mut self, data: &[T], mut i: i64) -> bool {
        while i > 0 && self.cmp_exch(data, i, i / 2) {
            i /= 2;
        }
        i == 0
    }
    fn max_sort_up<T: Rank1d>(&mut self, data: &[T], mut i: i64) -> bool {
        while i < 0 && self.cmp_exch(data, i / 2, i) {
            i /= 2;
        }
        i == 0
    }
    fn insert<T: Rank1d>(&mut self, data: &mut [T], v: T) {
        let p = self.pos[self.idx];
        let old = data[self.idx];
        data[self.idx] = v;
        self.idx += 1;
        if self.idx == self.n {
            self.idx = 0;
        }
        if p > 0 {
            if v > old {
                self.min_sort_down(data, p);
                return;
            }
            if self.min_sort_up(data, p) && self.cmp_exch(data, 0, -1) {
                self.max_sort_down(data, -1);
            }
        } else if p < 0 {
            if v < old {
                self.max_sort_down(data, p);
                return;
            }
            if self.max_sort_up(data, p) && self.cmp_exch(data, 1, 0) {
                self.min_sort_down(data, 1);
            }
        } else {
            if self.max_sort_up(data, -1) {
                self.max_sort_down(data, -1);
            }
            if self.min_sort_up(data, 1) {
                self.min_sort_down(data, 1);
            }
        }
    }
    fn top<T: Rank1d>(&self, data: &[T]) -> T {
        data[self.heap_at(0) as usize]
    }
}

/// SciPy 1.18.1's `_rank_filter_1d.rank_filter` (`_rank_filter_1d.cpp`, Ashelly's running rank,
/// MIT): the `rank`-th smallest of a window of `win_len` elements (`footprint.size`, the
/// footprint's false elements included, as SciPy passes it) along a 1-D input, the window
/// started and ended by the mode's extension as SciPy writes them (`mode` after
/// `_extend_mode_to_code(mode, is_filter=True)`). SciPy reads before or beyond the input where
/// the window's start or end runs past it (a window much larger than the input, a large origin
/// in `wrap`); larmorx reads the mode's own extension there.
pub fn rank_filter_1d<T: Rank1d>(
    input: &[T],
    rank: usize,
    win_len: usize,
    mode: Extend,
    cval: T,
    origin: isize,
) -> Vec<T> {
    let arr_len = input.len() as isize;
    let mut out = vec![T::default(); input.len()];
    if input.is_empty() || win_len == 0 {
        return out;
    }
    let lim = (win_len as isize - 1) / 2 - origin;
    let lim2 = arr_len - lim;
    if lim2 < 0 && arr_len == 1 {
        out[0] = match mode {
            Extend::Constant if win_len != 1 => cval,
            _ => input[0],
        };
        return out;
    }
    // SciPy indexes `in_arr[i]` directly; an index outside the input reads the extension
    let at = |i: isize| -> T {
        if (0..arr_len).contains(&i) {
            input[i as usize]
        } else {
            match crate::neighbourhood::map_coordinate(i, input.len(), mode) {
                crate::neighbourhood::BORDER => cval,
                j => input[j],
            }
        }
    };
    let mut m = Mediator::new(win_len, rank);
    let mut data = vec![T::default(); win_len];
    let wl = win_len as isize;
    match mode {
        Extend::Reflect => {
            let mut i = wl - lim - 1;
            while i > -1 {
                m.insert(&mut data, at(i));
                i -= 1;
            }
        }
        Extend::Constant => {
            for _ in 0..(wl - lim).max(0) {
                m.insert(&mut data, cval);
            }
        }
        Extend::Nearest => {
            for _ in 0..(wl - lim).max(0) {
                m.insert(&mut data, input[0]);
            }
        }
        Extend::Mirror => {
            let mut i = wl - lim;
            while i > 0 {
                m.insert(&mut data, at(i));
                i -= 1;
            }
        }
        Extend::Wrap => {
            let offset = if win_len.is_multiple_of(2) { 2 } else { 0 };
            let mut i = arr_len - lim - offset - 2 * origin;
            while i < arr_len {
                m.insert(&mut data, at(i));
                i += 1;
            }
        }
    }
    for i in 0..lim {
        m.insert(&mut data, at(i));
    }
    for i in lim.max(0)..arr_len {
        m.insert(&mut data, at(i));
        out[(i - lim) as usize] = m.top(&data);
    }
    for i in 0..lim {
        let v = match mode {
            Extend::Reflect => at(arr_len - 1 - i),
            Extend::Constant => cval,
            Extend::Nearest => input[input.len() - 1],
            Extend::Mirror => at(arr_len - 2 - i),
            Extend::Wrap => at(i),
        };
        m.insert(&mut data, v);
        out[(lim2 + i) as usize] = m.top(&data);
    }
    out
}
