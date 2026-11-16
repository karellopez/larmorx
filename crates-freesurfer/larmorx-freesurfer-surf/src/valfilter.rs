// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surf's `utils/mrisurf_vals.cpp`: the neighbourhood filters of a surface's per-vertex values
//! (queue item 3.4-03, taken early; relay 40 of track D): `MRISminFilterCurvatures` (line 1872),
//! `MRISmaxFilterCurvatures` (1911), `MRISaverageMarkedCurvatures` (1991),
//! `MRISaverageMarkedVals` (2029), `MRISaverageVals` (2066), `MRISaverageD` (2120),
//! `MRISmedianFilterVals` (2157), `MRISgaussianFilterD` (2286), `MRISmedianFilterD` (2324),
//! `MRISmedianFilterCurvature` (2372), `MRISmedianFilterVal2s` (2415),
//! `MRISmedianFilterVal2baks` (2458), `MRISaverageVal2s` (2501), `MRISaverageVal2baks` (2693),
//! `MRISmodeFilterVals` (2731), `MRISmodeFilterZeroVals` (2814), `MRISsoapBubbleVals` (2977),
//! `MRISsoapBubbleD` (3029) and `MRISaverageMarkedStats` (3224). `MRISaverageCurvatures` is
//! [`crate::props10`]'s.
//!
//! Each step reads the previous step's values (the new ones in the vertices' `tdx` first), but
//! the soap bubbles', which update in place in vertex order. The official build's arithmetic: the
//! averages in `float`, the running sum the first operand (its NaN passed on), a vertex without
//! neighbours keeping its value undivided (GCC skips the division by 1, so a signalling NaN stays
//! signalling); the medians' middle pair `(a[n/2 - 1] + a[n/2]) * 0.5` with the lower one first,
//! sorted by glibc 2.35's `qsort` ([`larmorx_core::glibc_qsort`], `mris_sort_compare_float`: a NaN
//! compares equal to everything); the Gaussian filter and the soap bubbles in `double`, stored in
//! `float` as `cvtsd2ss` does ([`larmorx_freesurfer_core::sse`] for every NaN). `MRISaverageVals`'
//! OpenMP loops change nothing: each vertex's value depends only on the previous step's.
//!
//! The mode filters count, for each vertex, its 1-hop neighbours' values rounded (`nint`) in a
//! table of `max + 1` entries (`max` the largest value over every vertex, truncated as x86-64
//! does); here only the values met are counted (the same answer: the most frequent value, the
//! smallest of a tie, from 0 for `MRISmodeFilterVals` and from 1 for `MRISmodeFilterZeroVals`),
//! so a large `max` costs neither memory nor time. A table of a negative size (a value of 2^31 or
//! more, or not a number, truncated to `INT_MIN`, then nothing larger) is `calloc`'s failure:
//! `Cannot allocate memory` and `MRISmodeFilterVals: could not allocate histo array of <n> ints`
//! (`ErrorExit`, `ERROR_NOMEMORY`). Defects (`larmorx_core::defects`):
//! - **FS-971** (out of bounds): the median filters write past their list of 1,024 values for a
//!   vertex with more than 1,023 unripped neighbours in use: both modes sort them all (noted).
//! - **FS-972** (out of bounds): the mode filters count a neighbour whose rounded value is above
//!   the table's last entry (a value that rounds up past the largest one truncated, as 2.6 next to
//!   a largest value of 2.6, or a table of no entries) outside the table: `reproduce` drops it as
//!   FreeSurfer's results show; by default the table covers every rounded value (noted).
//! - **FS-973** (hang): `MRISmodeFilterZeroVals` never ends when a vertex's value is not a number
//!   (`NaN != NaN`: it changes at every pass) and a vertex of value 0 next to it keeps being
//!   processed; both modes stop at the first pass that leaves the surface as the previous one did
//!   (noted).

use std::io::Write;

use larmorx_core::defects::{FS_971, FS_972, FS_973};
use larmorx_core::glibc_qsort::qsort;
use larmorx_freesurfer_core::cstd::double_to_int_x86;
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_NOMEMORY, Fatal, NO_ERROR};
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::utils::nint;

use crate::aaa::mris_sort_compare_float;
use crate::coords::Io;
use crate::metric::compute_curvature_min_max;
use crate::mris::{Mris, Vertex};

/// `MAX_NEIGHBORS` (`include/mrisurf_aaa.h`): the medians' list.
pub const MAX_NEIGHBORS: usize = 1024;
/// `ENOMEM`.
const ENOMEM: i32 = 12;

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// `(float) x` of a `double` as x86-64 converts it (`cvtsd2ss`): a NaN keeps its sign and the top
/// of its payload, quieted.
fn narrow(x: f64) -> f32 {
    if x.is_nan() {
        let b = x.to_bits();
        f32::from_bits(
            ((b >> 32) as u32 & 0x8000_0000) | 0x7fc0_0000 | ((b >> 29) as u32 & 0x003f_ffff),
        )
    } else {
        x as f32
    }
}

/// `(double) x` of a `float` (`cvtss2sd`): a NaN keeps its sign and payload, quieted.
pub(crate) fn widen(x: f32) -> f64 {
    if x.is_nan() {
        let b = x.to_bits();
        f64::from_bits(
            (u64::from(b & 0x8000_0000) << 32)
                | 0x7ff8_0000_0000_0000
                | (u64::from(b & 0x003f_ffff) << 29),
        )
    } else {
        f64::from(x)
    }
}

/// The per-vertex field a filter works on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Val,
    Val2,
    Val2bak,
    Curv,
    D,
    Stat,
}

fn get(v: &Vertex, f: Field) -> f32 {
    match f {
        Field::Val => v.val,
        Field::Val2 => v.val2,
        Field::Val2bak => v.val2bak,
        Field::Curv => v.curv,
        Field::D => v.d,
        Field::Stat => v.stat,
    }
}

fn set(v: &mut Vertex, f: Field, x: f32) {
    match f {
        Field::Val => v.val = x,
        Field::Val2 => v.val2 = x,
        Field::Val2bak => v.val2bak = x,
        Field::Curv => v.curv = x,
        Field::D => v.d = x,
        Field::Stat => v.stat = x,
    }
}

/// The first `n` entries of vertex `vno`'s neighbour list (`vnum` or `vtotal`).
fn nbrs(mris: &Mris, vno: usize, n: i16) -> Vec<i32> {
    let vt = mris.topology(vno);
    let n = usize::try_from(n).unwrap_or(0);
    match vt.v.as_deref() {
        Some(v) if n > 0 => v[..n.min(v.len())].to_vec(),
        _ => Vec::new(),
    }
}

/// One averaging step's new value of vertex `vno` (the official build's loop): its own value plus
/// its 1-hop neighbours' that `skip` does not exclude, in list order (`float`, the sum first), over
/// their count plus one; a vertex whose `vnum` is not above 0 keeps its value undivided.
fn mean_of(mris: &Mris, vno: usize, f: Field, skip: impl Fn(&Vertex) -> bool) -> f32 {
    let vt = mris.topology(vno);
    let mut val = get(mris.vertex(vno), f);
    if vt.vnum <= 0 {
        return val;
    }
    let mut num = 0.0f32;
    for n in nbrs(mris, vno, vt.vnum) {
        let vn = mris.vertex(n as usize);
        if skip(vn) {
            continue;
        }
        num = sse::add(num, 1.0);
        val = sse::add(val, get(vn, f));
    }
    num = sse::add(num, 1.0);
    sse::div(val, num)
}

/// The averaging shared by `MRISaverageVals` and its kin: `navgs` steps; each vertex `center`
/// does not exclude gets [`mean_of`] (neighbours excluded by `nb`) in `tdx`, then each vertex
/// `write` does not exclude takes it.
fn average(
    mris: &mut Mris,
    navgs: i32,
    f: Field,
    center: impl Fn(&Vertex) -> bool,
    nb: impl Fn(&Vertex) -> bool + Copy,
    write: impl Fn(&Vertex) -> bool,
) {
    for _ in 0..navgs {
        for vno in 0..nverts(mris) {
            if center(mris.vertex(vno)) {
                continue;
            }
            let m = mean_of(mris, vno, f, nb);
            mris.vertex_mut(vno).tdx = m;
        }
        for vno in 0..nverts(mris) {
            let v = mris.vertex_mut(vno);
            if write(v) {
                continue;
            }
            let t = v.tdx;
            set(v, f, t);
        }
    }
}

fn ripped(v: &Vertex) -> bool {
    v.ripflag != 0
}

fn ripped_or_unmarked(v: &Vertex) -> bool {
    v.ripflag != 0 || v.marked == 0
}

/// `MRISaverageVals(mris, navgs)`: `navgs` steps of each unripped vertex's `val` replaced by the
/// mean of it and its unripped 1-hop neighbours'. `NO_ERROR`.
pub fn average_vals(mris: &mut Mris, navgs: i32) -> i32 {
    average(mris, navgs, Field::Val, ripped, ripped, ripped);
    NO_ERROR
}

/// `MRISaverageVal2s(mris, navgs)`: as [`average_vals`] for `val2`.
pub fn average_val2s(mris: &mut Mris, navgs: i32) -> i32 {
    average(mris, navgs, Field::Val2, ripped, ripped, ripped);
    NO_ERROR
}

/// `MRISaverageVal2baks(mris, navgs)`: as [`average_vals`] for `val2bak`.
pub fn average_val2baks(mris: &mut Mris, navgs: i32) -> i32 {
    average(mris, navgs, Field::Val2bak, ripped, ripped, ripped);
    NO_ERROR
}

/// `MRISaverageD(mris, navgs)`: as [`average_vals`] for `d`.
pub fn average_d(mris: &mut Mris, navgs: i32) -> i32 {
    average(mris, navgs, Field::D, ripped, ripped, ripped);
    NO_ERROR
}

/// `MRISaverageMarkedVals(mris, navgs)`: as [`average_vals`] over the marked vertices only (an
/// unmarked one neither changes nor counts as a neighbour).
pub fn average_marked_vals(mris: &mut Mris, navgs: i32) -> i32 {
    average(
        mris,
        navgs,
        Field::Val,
        ripped_or_unmarked,
        ripped_or_unmarked,
        ripped_or_unmarked,
    );
    NO_ERROR
}

/// `MRISaverageCurvatures`' steps ([`crate::props10::mris_average_curvatures`] adds the extremes):
/// as [`average_vals`] for `curv`.
pub(crate) fn average_curvatures(mris: &mut Mris, navgs: i32) {
    average(mris, navgs, Field::Curv, ripped, ripped, ripped);
}

/// `MRISaverageMarkedCurvatures(mris, navgs)`: each marked unripped vertex's `curv` the mean of
/// it and its unripped 1-hop neighbours' (marked or not); then the curvatures' extremes
/// (`mrisComputeCurvatureMinMax`). `NO_ERROR`.
pub fn average_marked_curvatures(mris: &mut Mris, navgs: i32) -> i32 {
    average(
        mris,
        navgs,
        Field::Curv,
        ripped_or_unmarked,
        ripped,
        ripped_or_unmarked,
    );
    compute_curvature_min_max(mris);
    NO_ERROR
}

/// `FZERO(f)` of a `float`: `|f| < FLT_EPSILON` (NaN: no).
fn fzero(f: f32) -> bool {
    f.abs() < f32::EPSILON
}

/// `MRISaverageMarkedStats(mris, navgs)`: as [`average_vals`] for `stat`, each step then marking
/// 2 every unripped unmarked vertex whose new `stat` is not within `FLT_EPSILON` of 0 (NaN
/// included). `NO_ERROR`.
pub fn average_marked_stats(mris: &mut Mris, navgs: i32) -> i32 {
    for _ in 0..navgs {
        for vno in 0..nverts(mris) {
            if ripped(mris.vertex(vno)) {
                continue;
            }
            let m = mean_of(mris, vno, Field::Stat, ripped);
            mris.vertex_mut(vno).tdx = m;
        }
        for vno in 0..nverts(mris) {
            let v = mris.vertex_mut(vno);
            if v.ripflag != 0 {
                continue;
            }
            v.stat = v.tdx;
            if v.marked == 0 && !fzero(v.stat) {
                v.marked = 2;
            }
        }
    }
    NO_ERROR
}

/// `MRISminFilterCurvatures` (`max` false) and `MRISmaxFilterCurvatures` (`max` true): `niter`
/// steps of each unripped vertex's `curv` replaced by the smallest (largest) of it and its
/// unripped 1-hop neighbours' (by `<` or `>`, in list order: a NaN neighbour never wins and a
/// vertex's own NaN is kept); then the curvatures' extremes. `NO_ERROR`.
pub fn extreme_filter_curvatures(mris: &mut Mris, niter: i32, max: bool) -> i32 {
    for _ in 0..niter {
        for vno in 0..nverts(mris) {
            if ripped(mris.vertex(vno)) {
                continue;
            }
            let mut curv = mris.vertex(vno).curv;
            for n in nbrs(mris, vno, mris.topology(vno).vnum) {
                let vn = mris.vertex(n as usize);
                if vn.ripflag != 0 {
                    continue;
                }
                if (max && vn.curv > curv) || (!max && vn.curv < curv) {
                    curv = vn.curv;
                }
            }
            mris.vertex_mut(vno).tdx = curv;
        }
        for vno in 0..nverts(mris) {
            let v = mris.vertex_mut(vno);
            if v.ripflag == 0 {
                v.curv = v.tdx;
            }
        }
    }
    compute_curvature_min_max(mris);
    NO_ERROR
}

/// The median filters (`MRISmedianFilterVals`, `...Val2s`, `...Val2baks`, `...Curvature`,
/// `...D`): `nmedians` steps of each unripped vertex's field replaced by the median of it and its
/// unripped neighbours' (the first `vtotal` entries of its list when `vtotal`, else `vnum`):
/// sorted by `qsort`, the middle one of an odd count, else the mean of the middle two (`(lower +
/// upper) * 0.5`, `float`). FS-971: more than 1,023 unripped neighbours are all sorted (FreeSurfer
/// writes past its list of 1,024).
pub fn median_filter(g: &mut Globals, mris: &mut Mris, nmedians: i32, f: Field, vtotal: bool) {
    for _ in 0..nmedians {
        for vno in 0..nverts(mris) {
            if ripped(mris.vertex(vno)) {
                continue;
            }
            let vt = mris.topology(vno);
            let count = if vtotal { vt.vtotal } else { vt.vnum };
            let mut list = vec![get(mris.vertex(vno), f)];
            for n in nbrs(mris, vno, count) {
                let vn = mris.vertex(n as usize);
                if vn.ripflag != 0 {
                    continue;
                }
                list.push(get(vn, f));
            }
            if list.len() > MAX_NEIGHBORS {
                g.notices.add(
                    &FS_971,
                    format!(
                        "a median filter at vertex {vno}: {} values sorted (FreeSurfer writes past its list of {MAX_NEIGHBORS})",
                        list.len()
                    ),
                );
            }
            qsort(&mut list, |a: &f32, b: &f32| {
                mris_sort_compare_float(*a, *b)
            });
            let num = list.len();
            let t = if num % 2 == 1 {
                list[(num - 1) / 2]
            } else {
                sse::mul(sse::add(list[num / 2 - 1], list[num / 2]), 0.5)
            };
            mris.vertex_mut(vno).tdx = t;
        }
        for vno in 0..nverts(mris) {
            let v = mris.vertex_mut(vno);
            if v.ripflag == 0 {
                let t = v.tdx;
                set(v, f, t);
            }
        }
    }
}

/// `MRISmedianFilterVals(mris, nmedians)`: [`median_filter`] of `val` over the 1-hop neighbours.
/// `NO_ERROR`.
pub fn median_filter_vals(g: &mut Globals, mris: &mut Mris, nmedians: i32) -> i32 {
    median_filter(g, mris, nmedians, Field::Val, false);
    NO_ERROR
}

/// `MRISmedianFilterVal2s(mris, nmedians)`: of `val2`.
pub fn median_filter_val2s(g: &mut Globals, mris: &mut Mris, nmedians: i32) -> i32 {
    median_filter(g, mris, nmedians, Field::Val2, false);
    NO_ERROR
}

/// `MRISmedianFilterVal2baks(mris, nmedians)`: of `val2bak`.
pub fn median_filter_val2baks(g: &mut Globals, mris: &mut Mris, nmedians: i32) -> i32 {
    median_filter(g, mris, nmedians, Field::Val2bak, false);
    NO_ERROR
}

/// `MRISmedianFilterCurvature(mris, nmedians)`: of `curv`.
pub fn median_filter_curvature(g: &mut Globals, mris: &mut Mris, nmedians: i32) -> i32 {
    median_filter(g, mris, nmedians, Field::Curv, false);
    NO_ERROR
}

/// `MRISmedianFilterD(mris, nmedians, vtotal)`: of `d`, over the neighbours in use when `vtotal`
/// is not 0.
pub fn median_filter_d(g: &mut Globals, mris: &mut Mris, nmedians: i32, vtotal: i32) -> i32 {
    median_filter(g, mris, nmedians, Field::D, vtotal != 0);
    NO_ERROR
}

/// `MRISgaussianFilterD(mris, wt)`: one step of each unripped vertex's `d` replaced by `(d wt +
/// sum of its unripped 1-hop neighbours' d (1 - wt)) / (wt + their count (1 - wt))`, in `double`
/// (each product the value first). `NO_ERROR`.
pub fn gaussian_filter_d(mris: &mut Mris, wt: f64) -> i32 {
    let nbr_wt = sse::sub64(1.0, wt);
    for vno in 0..nverts(mris) {
        if ripped(mris.vertex(vno)) {
            continue;
        }
        let mut val = sse::mul64(widen(mris.vertex(vno).d), wt);
        let mut norm = wt;
        for n in nbrs(mris, vno, mris.topology(vno).vnum) {
            let vn = mris.vertex(n as usize);
            if vn.ripflag != 0 {
                continue;
            }
            norm = sse::add64(norm, nbr_wt);
            val = sse::add64(val, sse::mul64(widen(vn.d), nbr_wt));
        }
        mris.vertex_mut(vno).tdx = narrow(sse::div64(val, norm));
    }
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 {
            v.d = v.tdx;
        }
    }
    NO_ERROR
}

/// The soap bubbles (`MRISsoapBubbleVals` of `val`, `MRISsoapBubbleD` of `d`): with `Gdiag`'s
/// show and verbose bits, `performing soap bubble smoothing of <what> for <navgs> iterations.` on
/// standard output; then `navgs` passes in vertex order, in place: each unripped vertex not
/// marked 1 whose 1-hop neighbours include marked ones (any mark but 0, ripped ones too) takes
/// their mean (`double`) and the mark 2.
fn soap_bubble(g: &Globals, out: &mut dyn Write, mris: &mut Mris, navgs: i32, d: bool) -> i32 {
    if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
        let what = if d { "D vals" } else { "vals" };
        let _ = writeln!(
            out,
            "performing soap bubble smoothing of {what} for {navgs} iterations."
        );
    }
    let f = if d { Field::D } else { Field::Val };
    for _ in 0..navgs {
        for vno in 0..nverts(mris) {
            let v = mris.vertex(vno);
            if v.ripflag != 0 || v.marked == 1 {
                continue;
            }
            let mut mean = 0.0f64;
            let mut cmpt = 0i32;
            for n in nbrs(mris, vno, mris.topology(vno).vnum) {
                let vn = mris.vertex(n as usize);
                if vn.marked != 0 {
                    mean = sse::add64(mean, widen(get(vn, f)));
                    cmpt += 1;
                }
            }
            if cmpt > 0 {
                let x = narrow(sse::div64(mean, f64::from(cmpt)));
                let v = mris.vertex_mut(vno);
                set(v, f, x);
                v.marked = 2;
            }
        }
    }
    NO_ERROR
}

/// `MRISsoapBubbleVals(mris, navgs)`: the soap bubble of `val`. `NO_ERROR`.
pub fn soap_bubble_vals(g: &Globals, out: &mut dyn Write, mris: &mut Mris, navgs: i32) -> i32 {
    soap_bubble(g, out, mris, navgs, false)
}

/// `MRISsoapBubbleD(mris, navgs)`: the soap bubble of `d`. `NO_ERROR`.
pub fn soap_bubble_d(g: &Globals, out: &mut dyn Write, mris: &mut Mris, navgs: i32) -> i32 {
    soap_bubble(g, out, mris, navgs, true)
}

/// The mode filters' largest value: `max` from 0, raised by every vertex's `val` above it (the
/// `int` as a `float`), truncated as x86-64 does (`INT_MIN` for NaN and beyond the range).
fn mode_max(mris: &Mris) -> i32 {
    let mut max_val = 0i32;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno).val;
        if v > max_val as f32 {
            max_val = double_to_int_x86(f64::from(v));
        }
    }
    max_val
}

/// The mode filters' table: `calloc(max + 1, sizeof(int))`'s failure for a negative size, as
/// `ErrorExit` reports it.
fn mode_table(g: &mut Globals, err: &mut dyn Write, max_val: i32) -> Result<(), Fatal> {
    let n = max_val.wrapping_add(1);
    if n < 0 {
        g.errno = ENOMEM;
        log::error_exit(
            g,
            err,
            ERROR_NOMEMORY,
            &format!("MRISmodeFilterVals: could not allocate histo array of {n} ints"),
        )?;
    }
    Ok(())
}

/// A vertex's mode: its 1-hop neighbours' values rounded by `nint` (those below 0 skipped), each
/// counted; from `from` (0 or 1) upwards the value of the largest count, the first of a tie, a
/// count of 0 at `from` 1 never winning; 0 when none wins. FS-972: a value above `max_val` is
/// FreeSurfer's write outside its table: dropped with `reproduce`, counted otherwise.
fn mode_of(g: &mut Globals, mris: &Mris, vno: usize, max_val: i32, from: i32) -> i32 {
    let mut idx = Vec::new();
    for n in nbrs(mris, vno, mris.topology(vno).vnum) {
        let index = nint(g, f64::from(mris.vertex(n as usize).val));
        if index < 0 {
            continue;
        }
        if index > max_val {
            g.notices.add(
                &FS_972,
                format!(
                    "a mode filter at vertex {vno}: a neighbour's value rounds to {index}, past the table's last entry {max_val} (FreeSurfer counts it outside the table)"
                ),
            );
            if g.notices.reproduce() {
                continue;
            }
        }
        idx.push(index);
    }
    idx.sort_unstable();
    let count = |i: i32| idx.iter().filter(|&&x| x == i).count();
    let (mut max_histo, mut max_index) = if from == 0 { (count(0), 0) } else { (0, 0) };
    let mut k = 0;
    while k < idx.len() {
        let i = idx[k];
        let mut e = k;
        while e < idx.len() && idx[e] == i {
            e += 1;
        }
        if i >= 1 && e - k > max_histo {
            max_histo = e - k;
            max_index = i;
        }
        k = e;
    }
    max_index
}

/// The state a mode-filter pass leaves: each vertex's `val` and `valbak` bits and mark.
fn mode_state(mris: &Mris) -> Vec<(u32, u32, i16)> {
    (0..nverts(mris))
        .map(|vno| {
            let v = mris.vertex(vno);
            (v.val.to_bits(), v.valbak.to_bits(), v.marked)
        })
        .collect()
}

/// The two passes after a mode filter's counting: each unripped vertex marked 0 (to be processed
/// again) when its `val` differs from its new value `valbak` (`!=`: a NaN always differs), else 1,
/// and given it; then each unripped vertex marked 0 unmarks its 1-hop neighbours. Returns the
/// vertices changed.
fn mode_settle(mris: &mut Mris) -> i32 {
    let mut nchanged = 0;
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        #[allow(clippy::float_cmp)]
        if v.val != v.valbak {
            v.marked = 0;
            nchanged += 1;
        } else {
            v.marked = 1;
        }
        v.val = v.valbak;
    }
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 || v.marked == 1 {
            continue;
        }
        for n in nbrs(mris, vno, mris.topology(vno).vnum) {
            mris.vertex_mut(n as usize).marked = 0;
        }
    }
    nchanged
}

/// `MRISmodeFilterVals(mris, niter)`: the marks cleared (`MRISclearMarks`); up to `niter` passes:
/// each unripped vertex not yet settled (mark 0) counted as zero when its `val` rounds to 0, and
/// given its neighbours' mode (from 0) in `valbak`; then [`mode_settle`]; `iter <k>: <changed>
/// changed, <zeros> zero` on standard output; no change ends the passes. The marks cleared again.
/// `NO_ERROR`; `ErrorExit` for a table of a negative size.
pub fn mode_filter_vals(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    niter: i32,
) -> Result<i32, Fatal> {
    let max_val = mode_max(mris);
    mode_table(g, io.err, max_val)?;
    crate::discrete::clear_marks(mris);
    for ino in 0..niter {
        let mut nzero = 0;
        for vno in 0..nverts(mris) {
            let v = mris.vertex(vno);
            if v.ripflag != 0 || v.marked != 0 {
                continue;
            }
            let val = v.val;
            if nint(g, f64::from(val)) == 0 {
                nzero += 1;
            }
            let m = mode_of(g, mris, vno, max_val, 0);
            mris.vertex_mut(vno).valbak = m as f32;
        }
        let nchanged = mode_settle(mris);
        let _ = writeln!(io.out, "iter {ino}: {nchanged} changed, {nzero} zero");
        if nchanged == 0 {
            break;
        }
    }
    crate::discrete::clear_marks(mris);
    Ok(NO_ERROR)
}

/// `MRISmodeFilterZeroVals(mris)`: as [`mode_filter_vals`], with no bound on the passes, only the
/// vertices whose `val` rounds to 0 given their neighbours' mode (from 1: 0 when no neighbour is
/// above 0), the others' `valbak` their `val`; the passes go on while some vertex changed and some
/// zero was processed. FS-973: a pass that leaves the surface as the previous one did ends the
/// passes (FreeSurfer goes on for ever).
pub fn mode_filter_zero_vals(g: &mut Globals, io: &mut Io, mris: &mut Mris) -> Result<i32, Fatal> {
    let max_val = mode_max(mris);
    mode_table(g, io.err, max_val)?;
    crate::discrete::clear_marks(mris);
    let mut ino = 0;
    let mut before = mode_state(mris);
    loop {
        let mut nzero = 0;
        for vno in 0..nverts(mris) {
            let v = mris.vertex(vno);
            if v.ripflag != 0 || v.marked != 0 {
                continue;
            }
            let val = v.val;
            if nint(g, f64::from(val)) == 0 {
                nzero += 1;
            } else {
                mris.vertex_mut(vno).valbak = val;
                continue;
            }
            let m = mode_of(g, mris, vno, max_val, 1);
            mris.vertex_mut(vno).valbak = m as f32;
        }
        let nchanged = mode_settle(mris);
        let _ = writeln!(io.out, "iter {ino}: {nchanged} changed, {nzero} zero");
        ino += 1;
        if nchanged == 0 || nzero == 0 {
            break;
        }
        let after = mode_state(mris);
        if after == before {
            g.notices.add(
                &FS_973,
                format!(
                    "MRISmodeFilterZeroVals: pass {} left the surface as the one before ({nchanged} changed: values that are not numbers): stopped (FreeSurfer goes on for ever)",
                    ino - 1
                ),
            );
            break;
        }
        before = after;
    }
    crate::discrete::clear_marks(mris);
    Ok(NO_ERROR)
}
