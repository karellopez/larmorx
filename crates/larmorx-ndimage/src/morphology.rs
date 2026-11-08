// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Binary morphology, ported from SciPy 1.18.1 `scipy/ndimage/src/ni_morphology.c`
//! (`NI_BinaryErosion`, `CASE_NI_ERODE_POINT`, `NI_BinaryErosion2`, `CASE_ERODE_POINT2`) and
//! `scipy/ndimage/_morphology.py` (`_binary_erosion`'s iteration strategies, `binary_dilation`'s
//! reversed structure, `generate_binary_structure`, `iterate_structure`, `_center_is_true`);
//! BSD-3-Clause, the notice is in the crate's root.
//!
//! One pass is SciPy's point rule: where the mask is false the input's truth is kept; where the
//! structure's centre is true and the input already has the eroded value, it is kept; otherwise
//! the result is the "true" value unless an element of the structure falls on a "false" input or,
//! with a false `border_value`, outside the array (a dilation is the erosion of the complement:
//! `invert`). More iterations follow SciPy's two strategies, which give the same result: with
//! the centre true (and `brute_force` false) only the elements changed by the previous pass
//! spread to their structure neighbours (`NI_BinaryErosion2`, in place, the mask applied as SciPy
//! applies it: cast to `int8`); otherwise whole passes repeat.

use rayon::prelude::*;

use crate::neighbourhood::{BORDER, Neighbourhood};
use crate::{Extend, Footprint, Layout, NdElement, NdimageError, c_strides, unravel};

/// The truth of a mask element as `NI_BinaryErosion2` reads it: the mask cast to `int8`
/// (`np.asarray(mask, dtype=np.int8)`), so `0.5` or `256` are false there although true in the
/// first pass.
pub trait MaskElement: NdElement {
    /// `numpy.int8(value) != 0` (C's cast: floats truncated through a 32-bit integer, integers
    /// wrapped).
    fn int8_nonzero(self) -> bool;
}

macro_rules! mask_int {
    ($($t:ty),*) => {$(
        impl MaskElement for $t {
            #[inline]
            fn int8_nonzero(self) -> bool {
                (self as i8) != 0
            }
        }
    )*};
}
mask_int!(u8, i8, u16, i16, u32, i32, u64, i64);

impl MaskElement for f32 {
    #[inline]
    fn int8_nonzero(self) -> bool {
        (crate::cvtt_i32(f64::from(self)) as i8) != 0
    }
}

impl MaskElement for f64 {
    #[inline]
    fn int8_nonzero(self) -> bool {
        (crate::cvtt_i32(self) as i8) != 0
    }
}

/// The arguments of [`binary_erosion`] besides the input (`_binary_erosion`'s, after the Python
/// wrapper's checks and, for a dilation, the reversed structure and adjusted origins).
#[derive(Clone, Copy, Debug)]
pub struct BinaryErosion<'a> {
    /// The structuring element.
    pub structure: &'a Footprint,
    /// The origin on every axis.
    pub origins: &'a [isize],
    /// The number of passes; below 1, until nothing changes.
    pub iterations: i64,
    /// The value beyond the edges.
    pub border_value: bool,
    /// A dilation (`invert = 1`): the erosion of the complement.
    pub invert: bool,
    /// `brute_force`: whole passes even when the structure's centre is true.
    pub brute_force: bool,
}

/// `_center_is_true(structure, origin)`: `structure[s // 2 + o for each axis]` with numpy's
/// indexing (a negative index counts from the end); SciPy checks no origin of the binary
/// operations, so an index outside the structure is numpy's `IndexError`
/// ([`NdimageError::CentreIndex`]).
pub fn center_is_true(structure: &Footprint, origins: &[isize]) -> Result<bool, NdimageError> {
    let strides = c_strides(&structure.shape);
    let mut lin = 0usize;
    for (d, (&s, &o)) in structure.shape.iter().zip(origins).enumerate() {
        let mut c = (s / 2) as isize + o;
        if c < -(s as isize) || c >= s as isize {
            return Err(NdimageError::CentreIndex(format!(
                "index {c} is out of bounds for axis {d} with size {s}"
            )));
        }
        if c < 0 {
            c += s as isize;
        }
        lin += c as usize * strides[d];
    }
    Ok(structure.mask[lin])
}

/// One pass of `NI_BinaryErosion` from `read(offset)` (the input's truth at an input offset)
/// into a C-ordered 0/1 output; returns whether any element changed and, when `list`, the
/// C-order positions of those that did.
#[allow(clippy::too_many_arguments)]
fn pass(
    layout: &Layout,
    nb: &Neighbourhood,
    read: &(dyn Fn(usize) -> bool + Sync),
    mask: Option<&(dyn Fn(&[usize]) -> bool + Sync)>,
    border_value: bool,
    invert: bool,
    cit: bool,
    out: &mut [u8],
    list: bool,
) -> (bool, Vec<usize>) {
    let (t, f) = if invert { (0u8, 1u8) } else { (1u8, 0u8) };
    let bdr = if invert { !border_value } else { border_value };
    let ndim = layout.ndim();
    let n = nb.len();
    let shape = &layout.shape;
    let line = if ndim == 0 { 1 } else { shape[ndim - 1] };
    let results: Vec<(bool, Vec<usize>)> = out
        .par_chunks_mut(line.max(1))
        .enumerate()
        .map_init(
            || (vec![0usize; n], vec![0usize; ndim]),
            |(offs, p), (li, chunk)| {
                let mut changed = false;
                let mut changes = Vec::new();
                if ndim > 0 {
                    unravel(li * line, shape, p);
                }
                for (j, o) in chunk.iter_mut().enumerate() {
                    if ndim > 0 {
                        p[ndim - 1] = j;
                    }
                    let here = layout.offset(p);
                    let inp = u8::from(read(here));
                    let msk = mask.is_none_or(|m| m(p));
                    let (value, ch) = if !msk {
                        (inp, false)
                    } else if cit && inp == f {
                        (inp, false)
                    } else {
                        nb.offsets(p, offs);
                        let mut v = t;
                        for &off in offs.iter() {
                            if off == BORDER {
                                if !bdr {
                                    v = f;
                                    break;
                                }
                            } else {
                                let nn = if read(off) { t } else { f };
                                if nn == 0 {
                                    v = f;
                                    break;
                                }
                            }
                        }
                        (v, v != inp)
                    };
                    *o = value;
                    if ch {
                        changed = true;
                        if list {
                            changes.push(li * line + j);
                        }
                    }
                }
                (changed, changes)
            },
        )
        .collect();
    let changed = results.iter().any(|r| r.0);
    let list = results.into_iter().flat_map(|r| r.1).collect();
    (changed, list)
}

/// `_binary_erosion` (`binary_erosion`, and `binary_dilation` with `invert` after the wrapper
/// reversed the structure): the result as C-ordered 0/1 values (SciPy's default `bool` output).
///
/// `mask`: where false, elements keep the input's truth (the first pass reads its truth; the
/// later passes of the propagation strategy read it cast to `int8`, as SciPy does).
pub fn binary_erosion<T: NdElement, M: MaskElement>(
    input: &[T],
    layout: &Layout,
    args: BinaryErosion<'_>,
    mask: Option<(&[M], &Layout)>,
) -> Result<Vec<u8>, NdimageError> {
    let structure = args.structure;
    if input.len() < layout.span() {
        return Err(NdimageError::Shape(
            "input buffer and layout disagree".into(),
        ));
    }
    if structure.shape.len() != layout.ndim() {
        return Err(NdimageError::Shape(
            "structure and input must have same dimensionality".into(),
        ));
    }
    if structure.mask.is_empty() {
        return Err(NdimageError::EmptyStructure);
    }
    if let Some((m, ml)) = mask
        && (ml.shape != layout.shape || m.len() < ml.span())
    {
        return Err(NdimageError::Shape(
            "mask and input must have equal sizes".into(),
        ));
    }
    if args.origins.len() != layout.ndim() {
        return Err(NdimageError::Shape(
            "sequence argument must have length equal to input rank".into(),
        ));
    }
    let cit = center_is_true(structure, args.origins)?;
    // A centre before the structure (an index numpy's `_center_is_true` wraps around) makes
    // SciPy's C code read and write outside its arrays (a segmentation fault or garbage,
    // `validation/investigations/sdcflows-fieldmap-b/probe_origin_crash.py`): a clear error.
    if structure
        .shape
        .iter()
        .zip(args.origins)
        .any(|(&s, &o)| (s / 2) as isize + o < 0)
    {
        return Err(NdimageError::Origin);
    }
    let nb = Neighbourhood::new(layout, structure, args.origins, Extend::Constant);
    // the mask's truth at a position (multi-index), in the mask's own layout
    let out_layout = Layout::c(&layout.shape);
    let mask_at: Option<Box<dyn Fn(&[usize]) -> bool + Sync>> = mask.map(|(m, ml)| {
        let ml = ml.clone();
        let f: Box<dyn Fn(&[usize]) -> bool + Sync> =
            Box::new(move |p: &[usize]| m[ml.offset(p)].nonzero());
        f
    });
    let read_in = |off: usize| input[off].nonzero();
    let mut out = vec![0u8; layout.len()];
    let iterations = args.iterations;
    if iterations == 1 {
        pass(
            layout,
            &nb,
            &read_in,
            mask_at.as_deref(),
            args.border_value,
            args.invert,
            cit,
            &mut out,
            false,
        );
        return Ok(out);
    }
    if cit && !args.brute_force {
        let (_, list) = pass(
            layout,
            &nb,
            &read_in,
            mask_at.as_deref(),
            args.border_value,
            args.invert,
            cit,
            &mut out,
            true,
        );
        let mask_c: Option<Vec<bool>> = mask.map(|(m, ml)| {
            let mut idx = vec![0usize; ml.ndim()];
            (0..out.len())
                .map(|lin| {
                    unravel(lin, &ml.shape, &mut idx);
                    m[ml.offset(&idx)].int8_nonzero()
                })
                .collect()
        });
        propagate(
            &mut out,
            &out_layout,
            structure,
            args,
            mask_c.as_deref(),
            list,
        );
        return Ok(out);
    }
    // whole passes, alternating between two buffers
    let (mut changed, _) = pass(
        layout,
        &nb,
        &read_in,
        mask_at.as_deref(),
        args.border_value,
        args.invert,
        cit,
        &mut out,
        false,
    );
    let nb_out = Neighbourhood::new(&out_layout, structure, args.origins, Extend::Constant);
    let mut ii = 1i64;
    let mut before: Option<Vec<u8>> = None;
    while ii < iterations || (iterations < 1 && changed) {
        let prev = out.clone();
        let read_prev = |off: usize| prev[off] != 0;
        let (ch, _) = pass(
            &out_layout,
            &nb_out,
            &read_prev,
            mask_at.as_deref(),
            args.border_value,
            args.invert,
            cit,
            &mut out,
            false,
        );
        changed = ch;
        if iterations < 1 && changed {
            if before.as_deref() == Some(&out[..]) {
                return Err(NdimageError::Oscillation);
            }
            before = Some(prev);
        }
        ii += 1;
    }
    Ok(out)
}

/// `NI_BinaryErosion2`: from the elements changed by the first pass, the eroded value spreads to
/// every structure neighbour still holding the other value (each newly changed element spreading
/// in the next round), for `iterations - 1` rounds (all rounds until none changes when
/// `iterations < 1`), never into elements whose `int8` mask is zero. `out` is C-ordered.
fn propagate(
    out: &mut [u8],
    layout: &Layout,
    structure: &Footprint,
    args: BinaryErosion<'_>,
    mask: Option<&[bool]>,
    mut list: Vec<usize>,
) {
    let (t, f) = if args.invert { (0u8, 1u8) } else { (1u8, 0u8) };
    let ndim = layout.ndim();
    // the neighbours of a changed element y: x = y - (o - c) for the structure's elements o
    let centre: Vec<isize> = structure
        .shape
        .iter()
        .zip(args.origins)
        .map(|(&s, &o)| (s / 2) as isize + o)
        .collect();
    let mut deltas: Vec<Vec<isize>> = Vec::new();
    let mut k = vec![0usize; ndim];
    for (lin, &on) in structure.mask.iter().enumerate() {
        if on {
            unravel(lin, &structure.shape, &mut k);
            deltas.push((0..ndim).map(|d| centre[d] - k[d] as isize).collect());
        }
    }
    let niter = args.iterations - 1;
    let mut round = 0i64;
    let mut p = vec![0usize; ndim];
    while !list.is_empty() {
        if !(niter <= 0 || round < niter) {
            break;
        }
        round += 1;
        let mklist = niter <= 0 || round < niter;
        let mut next = Vec::new();
        for &y in &list {
            unravel(y, &layout.shape, &mut p);
            'nb: for dlt in &deltas {
                let mut off = 0usize;
                for d in 0..ndim {
                    let c = p[d] as isize + dlt[d];
                    if c < 0 || c >= layout.shape[d] as isize {
                        continue 'nb;
                    }
                    off += c as usize * layout.strides[d];
                }
                if mask.is_some_and(|m| !m[off]) {
                    continue;
                }
                if out[off] == t {
                    out[off] = f;
                    if mklist {
                        next.push(off);
                    }
                }
            }
        }
        list = next;
    }
}

/// `binary_dilation`'s structure and origins: the structure reversed on every axis, every
/// origin negated (minus one more on an axis of even length); then [`binary_erosion`] with
/// `invert`.
pub fn dilation_arguments(structure: &Footprint, origins: &[isize]) -> (Footprint, Vec<isize>) {
    let reversed = structure.reversed();
    let origins = origins
        .iter()
        .zip(&structure.shape)
        .map(|(&o, &s)| if s & 1 == 0 { -o - 1 } else { -o })
        .collect();
    (reversed, origins)
}

/// `generate_binary_structure(rank, connectivity)`: the C-ordered `3^rank` structure whose
/// elements lie within `connectivity` steps of the centre (city-block distance; connectivity
/// below 1 is 1). Rank 0 is a single true element.
pub fn generate_binary_structure(rank: usize, connectivity: i64) -> Footprint {
    let connectivity = connectivity.max(1);
    let shape = vec![3usize; rank];
    let n = 3usize.pow(rank as u32);
    let mut idx = vec![0usize; rank];
    let mask = (0..n)
        .map(|lin| {
            unravel(lin, &shape, &mut idx);
            let dist: i64 = idx.iter().map(|&i| (i as i64 - 1).abs()).sum();
            dist <= connectivity
        })
        .collect();
    Footprint { shape, mask }
}

/// `iterate_structure(structure, iterations)`: the structure dilated with itself `iterations -
/// 1` times (a copy for `iterations < 2`); the new origin (when asked) is `iterations ·
/// origin`.
pub fn iterate_structure(structure: &Footprint, iterations: i64) -> Footprint {
    if iterations < 2 {
        return structure.clone();
    }
    let ni = (iterations - 1) as usize;
    let shape: Vec<usize> = structure.shape.iter().map(|&s| s + ni * (s - 1)).collect();
    let pos: Vec<usize> = structure.shape.iter().map(|&s| ni * (s / 2)).collect();
    let layout = Layout::c(&shape);
    let mut start = vec![0u8; layout.len()];
    let mut k = vec![0usize; shape.len()];
    for (lin, &on) in structure.mask.iter().enumerate() {
        unravel(lin, &structure.shape, &mut k);
        let idx: Vec<usize> = k.iter().zip(&pos).map(|(a, b)| a + b).collect();
        start[layout.offset(&idx)] = u8::from(on);
    }
    let zeros = vec![0isize; shape.len()];
    let (rev, origins) = dilation_arguments(structure, &zeros);
    let out = binary_erosion::<u8, u8>(
        &start,
        &layout,
        BinaryErosion {
            structure: &rev,
            origins: &origins,
            iterations: ni as i64,
            border_value: false,
            invert: true,
            brute_force: false,
        },
        None,
    )
    .expect("a valid structure");
    Footprint {
        shape,
        mask: out.into_iter().map(|v| v != 0).collect(),
    }
}
