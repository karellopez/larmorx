// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Voxel arithmetic on the `MRI` structure (`utils/mri.cpp`, 3.1-35's first piece): clones and
//! clears, the size checks, frame copies, value ranges, scaling, thresholds, binarising, the
//! absolute value and signs, sums, differences, products and quotients of volumes, and the
//! extraction of a region (with `MRIcopyFrame` and `MRIcalcCRASforExtractedVolume`, taken early
//! from 3.1-36 and 3.1-37).
//!
//! FreeSurfer's `f(mri_src, mri_dst, ...)` takes a destination or `NULL` (a new volume), and may be
//! given the source itself as the destination: [`Pair`] (and [`Tri`] for two sources) says which.
//! The functions return FreeSurfer's pointer as [`Ret`]: the new volume, the given one, or `NULL`.

// C's comparisons, as written (`if (v > 255) v = 255; else if (v < 0) v = 0;`: the same values
// as `clamp`, NaN kept)
#![allow(clippy::manual_clamp)]

use std::io::Write;

use larmorx_core::defects::{FS_291, FS_292, FS_293, FS_294, FS_295, FS_296, FS_297, FS_461};
use larmorx_freesurfer_core::affine::{AffineMatrix, set_affine_matrix};
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal, NO_ERROR};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add, add64, div64, mul, mul64, sub};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::ctab::ctab_deep_copy;
use larmorx_freesurfer_mri::mri::{
    MRI_BITMAP, MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri, MriRegion,
    mri_alloc, mri_alloc_sequence, mri_copy_header, mri_copy_pulse_parameters,
};
use larmorx_freesurfer_mri::vol_geom::{extract_i_to_r, extract_r_to_i, reinit_cache};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::rows::{self, Num, elem_size, need_voxels};

/// A function's source and destination: a new destination (`NULL`), a given one, or the source
/// itself (`f(mri, mri, ...)`).
pub enum Pair<'a> {
    New(&'a Mri),
    Given(&'a Mri, &'a mut Mri),
    Same(&'a mut Mri),
}

/// Two sources and a destination: a new one, a given one, or one of the sources.
pub enum Tri<'a> {
    New(&'a Mri, &'a Mri),
    Given(&'a Mri, &'a Mri, &'a mut Mri),
    /// The destination is the first source (`f(mri1, mri2, mri1)`).
    First(&'a mut Mri, &'a Mri),
    /// The destination is the second source (`f(mri1, mri2, mri2)`).
    Second(&'a Mri, &'a mut Mri),
}

/// Where a source is: a volume of its own, or the destination.
#[derive(Clone, Copy)]
enum S<'a> {
    Own(&'a Mri),
    Dst,
}

/// The resolved volumes: the sources, a fresh destination when one was made, the given one.
pub(crate) struct Vols<'a> {
    s1: S<'a>,
    s2: S<'a>,
    given: Option<&'a mut Mri>,
    pub(crate) fresh: Option<Mri>,
}

impl<'a> Vols<'a> {
    pub(crate) fn of_pair(p: Pair<'a>) -> (Vols<'a>, Option<&'a Mri>) {
        match p {
            Pair::New(s) => (
                Vols {
                    s1: S::Own(s),
                    s2: S::Own(s),
                    given: None,
                    fresh: None,
                },
                Some(s),
            ),
            Pair::Given(s, d) => (
                Vols {
                    s1: S::Own(s),
                    s2: S::Own(s),
                    given: Some(d),
                    fresh: None,
                },
                Some(s),
            ),
            Pair::Same(d) => (
                Vols {
                    s1: S::Dst,
                    s2: S::Dst,
                    given: Some(d),
                    fresh: None,
                },
                None,
            ),
        }
    }
    pub(crate) fn of_tri(t: Tri<'a>) -> Vols<'a> {
        match t {
            Tri::New(a, b) => Vols {
                s1: S::Own(a),
                s2: S::Own(b),
                given: None,
                fresh: None,
            },
            Tri::Given(a, b, d) => Vols {
                s1: S::Own(a),
                s2: S::Own(b),
                given: Some(d),
                fresh: None,
            },
            Tri::First(d, b) => Vols {
                s1: S::Dst,
                s2: S::Own(b),
                given: Some(d),
                fresh: None,
            },
            Tri::Second(a, d) => Vols {
                s1: S::Own(a),
                s2: S::Dst,
                given: Some(d),
                fresh: None,
            },
        }
    }
    pub(crate) fn has_dst(&self) -> bool {
        self.given.is_some() || self.fresh.is_some()
    }
    pub(crate) fn d(&mut self) -> &mut Mri {
        match &mut self.given {
            Some(d) => d,
            None => self.fresh.as_mut().expect("a destination"),
        }
    }
    pub(crate) fn dref(&self) -> &Mri {
        match &self.given {
            Some(d) => d,
            None => self.fresh.as_ref().expect("a destination"),
        }
    }
    /// The first source (the destination when it is the source).
    pub(crate) fn a(&self) -> &Mri {
        match self.s1 {
            S::Own(m) => m,
            S::Dst => self.dref(),
        }
    }
    pub(crate) fn b(&self) -> &Mri {
        match self.s2 {
            S::Own(m) => m,
            S::Dst => self.dref(),
        }
    }
    pub(crate) fn done(self) -> Ret<Mri> {
        match self.fresh {
            Some(m) => Ret::New(m),
            None => Ret::Given,
        }
    }
    /// `mri_dst = MRIcopy(mri_src, mri_dst)`: a copy of the first source made here, or copied into
    /// the given destination; nothing for the source itself (`MRIcopy` returns at once).
    pub(crate) fn copy_a(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<(), Fatal> {
        match (self.s1, &mut self.given) {
            (S::Dst, _) => {}
            (S::Own(s), Some(d)) => {
                mri_copy(g, out, err, s, Some(&mut **d))?;
            }
            (S::Own(s), None) => {
                self.fresh = mri_copy(g, out, err, s, None)?.into_new();
            }
        }
        Ok(())
    }
    /// The [`Pair`] these volumes were made of, the destination made here given (`f(src, dst)`
    /// after `if (!dst) dst = MRIclone(src, NULL)`): `f`'s result, a made destination returned
    /// as new.
    pub(crate) fn with_pair(
        self,
        f: impl FnOnce(Pair) -> Result<Ret<Mri>, Fatal>,
    ) -> Result<Ret<Mri>, Fatal> {
        match (self.s1, self.given, self.fresh) {
            (S::Dst, Some(d), _) => f(Pair::Same(d)),
            (S::Own(s), Some(d), _) => f(Pair::Given(s, d)),
            (S::Own(s), None, Some(mut m)) => Ok(match f(Pair::Given(s, &mut m))? {
                Ret::Null => Ret::Null,
                _ => Ret::New(m),
            }),
            _ => unreachable!("a pair always has a destination here"),
        }
    }
    /// `NULL` returned: a destination made here is dropped (FreeSurfer leaks it).
    pub(crate) fn null(self) -> Ret<Mri> {
        Ret::Null
    }
}

/// The voxel `(x, y, z, f)` of a source by `MRIgetVoxVal`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn getv(
    g: &mut Globals,
    err: &mut dyn Write,
    v: &Vols,
    first: bool,
    x: i32,
    y: i32,
    z: i32,
    f: i32,
) -> Result<f32, Fatal> {
    let m = if first { v.a() } else { v.b() };
    mri_get_vox_val(g, err, m, x, y, z, f, 0)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn setv(
    g: &mut Globals,
    err: &mut dyn Write,
    v: &mut Vols,
    x: i32,
    y: i32,
    z: i32,
    f: i32,
    val: f32,
) -> Result<(), Fatal> {
    mri_set_vox_val(g, err, v.d(), x, y, z, f, val, 0).map(|_| ())
}

/// `MRIclone(mri_src, NULL)`'s allocation when `v` has no destination: `MRIallocSequence` of the
/// source's extent, type and frames, then (as `MRIclone` does on any destination) the colour
/// table's deep copy and `MRIcopyHeader`.
pub(crate) fn clone_into(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    v: &mut Vols,
) -> Result<(), Fatal> {
    if v.has_dst() {
        return Ok(());
    }
    let s = v.a();
    let fresh = clone_new(g, out, err, s)?;
    v.fresh = Some(fresh);
    Ok(())
}

fn clone_new(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    s: &Mri,
) -> Result<Mri, Fatal> {
    let mut d = mri_alloc_sequence(
        g,
        out,
        err,
        s.vg.width,
        s.vg.height,
        s.vg.depth,
        s.mtype,
        s.nframes,
    )?;
    clone_header(g, out, err, s, &mut d)?;
    Ok(d)
}

fn clone_header(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    s: &Mri,
    d: &mut Mri,
) -> Result<(), Fatal> {
    if s.ct.is_some() {
        d.ct = ctab_deep_copy(g, err, s.ct.as_ref());
    }
    mri_copy_header(g, out, err, s, Some(d))?;
    Ok(())
}

/// `MRIclone(mri_src, mri_dst)`: a new volume of the source's extent, type and frames when none is
/// given (its voxels zero, not copied), the source's colour table deep-copied into it when the
/// source has one, then the source's header (`MRIcopyHeader`).
pub fn mri_clone(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    match dst {
        None => Ok(Ret::New(clone_new(g, out, err, src)?)),
        Some(d) => {
            clone_header(g, out, err, src, d)?;
            Ok(Ret::Given)
        }
    }
}

/// `MRIcloneBySpace(mri_src, type, nframes)`: a new volume of the source's extent, of `type` and
/// `nframes` (the source's when negative), with the source's header and pulse parameters.
pub fn mri_clone_by_space(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    mtype: i32,
    nframes: i32,
) -> Result<Mri, Fatal> {
    let mtype = if mtype < 0 { src.mtype } else { mtype };
    let nframes = if nframes < 0 { src.nframes } else { nframes };
    let mut d = mri_alloc_sequence(
        g,
        out,
        err,
        src.vg.width,
        src.vg.height,
        src.vg.depth,
        mtype,
        nframes,
    )?;
    mri_copy_header(g, out, err, src, Some(&mut d))?;
    mri_copy_pulse_parameters(src, &mut d);
    d.nframes = nframes;
    Ok(d)
}

/// `MRIclear(mri)`: every row of every slice and frame set to zero bytes (`width` elements of the
/// type; `MRI_BITMAP` `width / 8` bytes); `NO_ERROR`, or `ERROR_UNSUPPORTED` with `MRIclear:
/// unsupported input type <type>` for another type.
pub fn mri_clear(g: &mut Globals, err: &mut dyn Write, mri: &mut Mri) -> Result<i32, Fatal> {
    let width = mri.vg.width;
    let bytes: i64 = match mri.mtype {
        MRI_BITMAP => i64::from(width / 8),
        t if elem_size(t) > 0 => i64::from(width) * elem_size(t) as i64,
        t => {
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                &format!("MRIclear: unsupported input type {t}"),
            );
            return Ok(ERROR_UNSUPPORTED);
        }
    };
    let rows = i64::from(mri.nframes) * i64::from(mri.vg.depth) * i64::from(mri.vg.height);
    if rows > 0 && bytes != 0 {
        need_voxels(g, err, "MRIclear", mri)?;
    }
    for frame in 0..mri.nframes {
        for z in 0..mri.vg.depth {
            for y in 0..mri.vg.height {
                let sl = i64::from(z) + i64::from(frame) * i64::from(mri.vg.depth);
                rows::zero_row(mri, sl, i64::from(y), bytes.max(0) as usize);
            }
        }
    }
    Ok(NO_ERROR)
}

/// `MRImatch(mri1, mri2)`: 1 when the extents and types are equal.
pub fn mri_match(a: &Mri, b: &Mri) -> i32 {
    i32::from(
        a.vg.width == b.vg.width
            && a.vg.height == b.vg.height
            && a.vg.depth == b.vg.depth
            && a.mtype == b.mtype,
    )
}

/// `FEQUAL(f1, f2)`: `fabs(f1 - f2) < FLT_EPSILON` (the `float` difference).
pub(crate) fn fequal(a: f32, b: f32) -> bool {
    f64::from(sub(a, b).abs()) < f64::from(f32::EPSILON)
}

/// `MRImatchDimensions(mri1, mri2)`: 1 when the extents are equal and the voxel sizes `FEQUAL`.
pub fn mri_match_dimensions(a: &Mri, b: &Mri) -> i32 {
    i32::from(
        a.vg.width == b.vg.width
            && a.vg.height == b.vg.height
            && a.vg.depth == b.vg.depth
            && fequal(a.vg.xsize, b.vg.xsize)
            && fequal(a.vg.ysize, b.vg.ysize)
            && fequal(a.vg.zsize, b.vg.zsize),
    )
}

/// `MRIcheckSize(mri_src, mri_check, width, height, depth)`: 0 without `mri_check`; 1 when its
/// extent is `width x height x depth` (each 0 taken from the source).
pub fn mri_check_size(src: &Mri, check: Option<&Mri>, width: i32, height: i32, depth: i32) -> i32 {
    let Some(c) = check else {
        return 0;
    };
    let w = if width == 0 { src.vg.width } else { width };
    let h = if height == 0 { src.vg.height } else { height };
    let d = if depth == 0 { src.vg.depth } else { depth };
    i32::from(w == c.vg.width && h == c.vg.height && d == c.vg.depth)
}

/// `MRIcopyFrame(mri_src, mri_dst, src_frame, dst_frame)`: without a destination, a new volume of
/// the source's extent and type with `dst_frame + 1` frames, the source's header and pulse
/// parameters; `NULL` after `MRIcopyFrame: dst frame #<n> out of range (nframes=<n>)` when
/// `dst_frame` is not below the destination's frames; else each voxel of the source's extent (slices,
/// rows, columns) read from `src_frame` and written into `dst_frame` (`MRIgetVoxVal`,
/// `MRIsetVoxVal`).
pub fn mri_copy_frame(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    src_frame: i32,
    dst_frame: i32,
) -> Result<Ret<Mri>, Fatal> {
    let (mut v, _) = Vols::of_pair(p);
    copy_frame(g, out, err, &mut v, src_frame, dst_frame)
        .map(|ok| if ok { v.done() } else { v.null() })
}

fn copy_frame(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    v: &mut Vols,
    src_frame: i32,
    dst_frame: i32,
) -> Result<bool, Fatal> {
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    if !v.has_dst() {
        let s = v.a();
        let mut n = mri_alloc_sequence(g, out, err, w, h, d, s.mtype, dst_frame.wrapping_add(1))?;
        mri_copy_header(g, out, err, s, Some(&mut n))?;
        mri_copy_pulse_parameters(s, &mut n);
        v.fresh = Some(n);
    }
    if dst_frame >= v.dref().nframes {
        let nf = v.dref().nframes;
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("MRIcopyFrame: dst frame #{dst_frame} out of range (nframes={nf})\n"),
        );
        return Ok(false);
    }
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let val = getv(g, err, v, true, x, y, z, src_frame)?;
                setv(g, err, v, x, y, z, dst_frame, val)?;
            }
        }
    }
    Ok(true)
}

/// `MRIcopyFrames(mri_src, mri_dst, src_start_frame, src_end_frame, dst_start_frame)`: without a
/// destination, a new volume of the source's extent and type with `end - start + 1` frames and the
/// source's header; the program ends (`MRIcopyFrames(<s>, <e>, <d>): src (<w>, <h>, <d>) doesn't
/// match destimation (<w>, <h>, <d>)`, `ERROR_BADPARM`) when the two do not `MRImatch`; then
/// [`mri_copy_frame`] of each frame from start to end into frame `+ (dst_start - src_start)`,
/// `NULL` at the first that fails.
pub fn mri_copy_frames(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    s0: i32,
    s1: i32,
    d0: i32,
) -> Result<Ret<Mri>, Fatal> {
    let (mut v, _) = Vols::of_pair(p);
    if !v.has_dst() {
        let s = v.a();
        let mut n = mri_alloc_sequence(
            g,
            out,
            err,
            s.vg.width,
            s.vg.height,
            s.vg.depth,
            s.mtype,
            s1.wrapping_sub(s0).wrapping_add(1),
        )?;
        mri_copy_header(g, out, err, s, Some(&mut n))?;
        v.fresh = Some(n);
    }
    if mri_match(v.a(), v.dref()) == 0 {
        let (a, d) = (v.a(), v.dref());
        let msg = format!(
            "MRIcopyFrames({s0}, {s1}, {d0}): src ({}, {}, {}) doesn't match destimation ({}, {}, {})\n",
            a.vg.width, a.vg.height, a.vg.depth, d.vg.width, d.vg.height, d.vg.depth
        );
        log::error_exit(g, err, ERROR_BADPARM, &msg)?;
    }
    let offset = d0.wrapping_sub(s0);
    let mut fno = s0;
    while fno <= s1 {
        if !copy_frame(g, out, err, &mut v, fno, fno.wrapping_add(offset))? {
            return Ok(v.null());
        }
        fno += 1;
    }
    Ok(v.done())
}

/// `DZERO(d)`: `fabs(d) < 1e-15`.
fn dzero(d: f64) -> bool {
    d.abs() < 1e-15
}

/// `MRIlinearScale(mri_src, mri_dst, scale, offset, only_nonzero)`: a clone without a destination;
/// each voxel of each frame `v * scale + offset` in `float` (only the voxels not `DZERO` when
/// `only_nonzero`), clipped to [0, 255] for a `uchar` destination, written by `MRIsetVoxVal`.
pub fn mri_linear_scale(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    scale: f32,
    offset: f32,
    only_nonzero: bool,
) -> Result<Ret<Mri>, Fatal> {
    let (mut v, _) = Vols::of_pair(p);
    clone_into(g, out, err, &mut v)?;
    let (w, h, d, nf) = (
        v.a().vg.width,
        v.a().vg.height,
        v.a().vg.depth,
        v.a().nframes,
    );
    for f in 0..nf {
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let mut val = getv(g, err, &v, true, x, y, z, f)?;
                    if !only_nonzero || !dzero(f64::from(val)) {
                        val = add(mul(val, scale), offset);
                    }
                    if v.dref().mtype == MRI_UCHAR {
                        if val > 255.0 {
                            val = 255.0;
                        } else if val < 0.0 {
                            val = 0.0;
                        }
                    }
                    setv(g, err, &mut v, x, y, z, f, val)?;
                }
            }
        }
    }
    Ok(v.done())
}

/// `MRIscalarMul(mri_src, mri_dst, scalar)`: a clone without a destination; each voxel of each frame
/// times `scalar` in `float`, written by `MRIsetVoxVal`.
pub fn mri_scalar_mul(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    scalar: f32,
) -> Result<Ret<Mri>, Fatal> {
    voxelwise(g, out, err, p, true, |_, val| mul(val, scalar))
}

/// The functions that map each voxel of each frame (frames outermost when `frames_first`, else
/// innermost) through `op` into a clone of the source (or the given destination).
fn voxelwise(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    frames_first: bool,
    op: impl Fn(&Mri, f32) -> f32,
) -> Result<Ret<Mri>, Fatal> {
    let (mut v, _) = Vols::of_pair(p);
    clone_into(g, out, err, &mut v)?;
    voxelwise_into(g, err, &mut v, frames_first, op)?;
    Ok(v.done())
}

fn voxelwise_into(
    g: &mut Globals,
    err: &mut dyn Write,
    v: &mut Vols,
    frames_first: bool,
    op: impl Fn(&Mri, f32) -> f32,
) -> Result<(), Fatal> {
    let (w, h, d, nf) = (
        v.a().vg.width,
        v.a().vg.height,
        v.a().vg.depth,
        v.a().nframes,
    );
    let mut one = |g: &mut Globals, v: &mut Vols, x, y, z, f| -> Result<(), Fatal> {
        let val = getv(g, err, v, true, x, y, z, f)?;
        let r = op(v.dref(), val);
        setv(g, err, v, x, y, z, f, r)
    };
    if frames_first {
        for f in 0..nf {
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        one(g, v, x, y, z, f)?;
                    }
                }
            }
        }
    } else {
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    for f in 0..nf {
                        one(g, v, x, y, z, f)?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// A volume made with `MRIallocSequence` of the source's extent, type and frames (`nframes`), with
/// the source's header, when `v` has no destination (`MRIabs`, `MRIpos`, `MRIneg`, `MRIsubtract`,
/// `MRIadd`).
fn alloc_like(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    v: &mut Vols,
    nframes: i32,
    mtype: Option<i32>,
    sequence: bool,
) -> Result<(), Fatal> {
    if v.has_dst() {
        return Ok(());
    }
    let s = v.a();
    let t = mtype.unwrap_or(s.mtype);
    let mut n = if sequence {
        mri_alloc_sequence(g, out, err, s.vg.width, s.vg.height, s.vg.depth, t, nframes)?
    } else {
        mri_alloc(g, out, err, s.vg.width, s.vg.height, s.vg.depth, t)?
    };
    mri_copy_header(g, out, err, s, Some(&mut n))?;
    v.fresh = Some(n);
    Ok(())
}

/// `MRIabs(mri_src, mri_dst)`: without a destination a new volume of the source's extent, type and
/// frames with its header; each voxel's absolute value (frames innermost).
pub fn mri_abs(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
) -> Result<Ret<Mri>, Fatal> {
    signs(g, out, err, p, |x| x.abs())
}

/// `MRIpos(mri_src, mri_dst)`: as [`mri_abs`], a negative voxel set to 0.
pub fn mri_pos(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
) -> Result<Ret<Mri>, Fatal> {
    signs(g, out, err, p, |x| if f64::from(x) < 0.0 { 0.0 } else { x })
}

/// `MRIneg(mri_src, mri_dst)`: as [`mri_abs`], a positive voxel set to 0.
pub fn mri_neg(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
) -> Result<Ret<Mri>, Fatal> {
    signs(g, out, err, p, |x| if f64::from(x) > 0.0 { 0.0 } else { x })
}

fn signs(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    op: impl Fn(f32) -> f32,
) -> Result<Ret<Mri>, Fatal> {
    let (mut v, _) = Vols::of_pair(p);
    let nf = v.a().nframes;
    alloc_like(g, out, err, &mut v, nf, None, true)?;
    voxelwise_into(g, err, &mut v, false, |_, x| op(x))?;
    Ok(v.done())
}

/// `MRIthreshold(mri_src, mri_dst, threshold)`: a clone without a destination; each voxel of each
/// frame below `threshold` set to 0.
pub fn mri_threshold(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    threshold: f32,
) -> Result<Ret<Mri>, Fatal> {
    voxelwise(g, out, err, p, true, |_, val| {
        if val < threshold { 0.0 } else { val }
    })
}

/// `MRIupperthresholdAllFrames(mri_src, mri_dst, threshold)`: a clone without a destination; a
/// voxel above `threshold` in any frame is 0 in every frame, another is copied frame by frame.
pub fn mri_upper_threshold_all_frames(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    threshold: f32,
) -> Result<Ret<Mri>, Fatal> {
    let (mut v, _) = Vols::of_pair(p);
    clone_into(g, out, err, &mut v)?;
    let (w, h, d, nf) = (
        v.a().vg.width,
        v.a().vg.height,
        v.a().vg.depth,
        v.a().nframes,
    );
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let mut n = 0;
                for f in 0..nf {
                    if getv(g, err, &v, true, x, y, z, f)? > threshold {
                        n += 1;
                    }
                }
                if n > 0 {
                    for f in 0..nf {
                        setv(g, err, &mut v, x, y, z, f, 0.0)?;
                    }
                } else {
                    for f in 0..nf {
                        let val = getv(g, err, &v, true, x, y, z, f)?;
                        setv(g, err, &mut v, x, y, z, f, val)?;
                    }
                }
            }
        }
    }
    Ok(v.done())
}

/// `MRIbinarize(mri_src, mri_dst, threshold, low_val, hi_val)`: a clone without a destination; each
/// voxel of each frame `low_val` below `threshold` (compared in `double`), else `hi_val` (a NaN
/// voxel too).
pub fn mri_binarize(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    threshold: f32,
    low_val: f32,
    hi_val: f32,
) -> Result<Ret<Mri>, Fatal> {
    voxelwise(g, out, err, p, true, |_, val| {
        if f64::from(val) < f64::from(threshold) {
            low_val
        } else {
            hi_val
        }
    })
}

/// `MRImultiply(mri1, mri2, mri_dst)`: without a destination a new one-frame volume of the first's
/// extent and type with its header; each voxel of frame 0 the product in `float`.
pub fn mri_multiply(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: Tri,
) -> Result<Ret<Mri>, Fatal> {
    let mut v = Vols::of_tri(t);
    alloc_like(g, out, err, &mut v, 1, None, false)?;
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let f1 = getv(g, err, &v, true, x, y, z, 0)?;
                let f2 = getv(g, err, &v, false, x, y, z, 0)?;
                // the official build multiplies f2 by f1 (f2 in the destination register: its NaN wins)
                setv(g, err, &mut v, x, y, z, 0, mul(f2, f1))?;
            }
        }
    }
    Ok(v.done())
}

/// Min and max as `MRIvalRange` & co. keep them: from 10000 and -10000, a smaller (larger) value
/// replacing them (NaNs never do).
struct Range {
    min: f32,
    max: f32,
}

impl Range {
    fn new() -> Range {
        Range {
            min: 10000.0,
            max: -10000.0,
        }
    }
    fn see(&mut self, val: f32) {
        if val < self.min {
            self.min = val;
        }
        if val > self.max {
            self.max = val;
        }
    }
}

/// `MRIvalRange(mri, &min, &max)`: the smallest and largest voxel of every frame (each read as
/// `MRIgetVoxVal` reads it), from 10000 and -10000 (a volume whose voxels are all above 10000 has
/// min 10000; NaNs are passed over); `NO_ERROR`.
pub fn mri_val_range(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
) -> Result<(i32, f32, f32), Fatal> {
    let mut r = Range::new();
    for f in 0..mri.nframes {
        for z in 0..mri.vg.depth {
            for y in 0..mri.vg.height {
                for x in 0..mri.vg.width {
                    r.see(direct_or_get(g, err, mri, x, y, z, f, "MRIvalRange")?);
                }
            }
        }
    }
    Ok((NO_ERROR, r.min, r.max))
}

/// A voxel the way the value-range functions read it: the five direct types through the row
/// pointers, the others by `MRIgetVoxVal`.
#[allow(clippy::too_many_arguments)]
fn direct_or_get(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
    f: i32,
    what: &str,
) -> Result<f32, Fatal> {
    match mri.mtype {
        MRI_FLOAT | MRI_INT | MRI_SHORT | MRI_USHRT | MRI_UCHAR => {
            need_voxels(g, err, what, mri)?;
            let sl = i64::from(z) + i64::from(f) * i64::from(mri.vg.depth);
            Ok(rows::read(g, what, mri, sl, i64::from(y), i64::from(x), mri.mtype).f())
        }
        _ => mri_get_vox_val(g, err, mri, x, y, z, f, 0),
    }
}

/// `MRIvalRangeFrame(mri, &min, &max, frame)`: [`mri_val_range`] of one frame for the five direct
/// types; another type: `MRIvalRange: unsupported type <type>`, `ERROR_UNSUPPORTED`, and the range
/// not set (`None`). A frame outside the volume reads FS-289's and FS-290's places.
pub fn mri_val_range_frame(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    frame: i32,
) -> Result<(i32, Option<(f32, f32)>), Fatal> {
    if !matches!(
        mri.mtype,
        MRI_FLOAT | MRI_INT | MRI_SHORT | MRI_USHRT | MRI_UCHAR
    ) {
        let msg = format!("MRIvalRange: unsupported type {}", mri.mtype);
        log::error_printf(g, err, ERROR_UNSUPPORTED, &msg);
        return Ok((ERROR_UNSUPPORTED, None));
    }
    let mut r = Range::new();
    for z in 0..mri.vg.depth {
        for y in 0..mri.vg.height {
            for x in 0..mri.vg.width {
                r.see(direct_or_get(
                    g,
                    err,
                    mri,
                    x,
                    y,
                    z,
                    frame,
                    "MRIvalRangeFrame",
                )?);
            }
        }
    }
    Ok((NO_ERROR, Some((r.min, r.max))))
}

/// `MRIvalRangeRegion(mri, &min, &max, region)`: the range of frame 0 over the region clipped to
/// the volume (its start raised to 0, its end lowered to the extent); `NO_ERROR`.
pub fn mri_val_range_region(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    region: &MriRegion,
) -> Result<(i32, f32, f32), Fatal> {
    let width = region.x.wrapping_add(region.dx).min(mri.vg.width);
    let height = region.y.wrapping_add(region.dy).min(mri.vg.height);
    let depth = region.z.wrapping_add(region.dz).min(mri.vg.depth);
    let (x0, y0, z0) = (region.x.max(0), region.y.max(0), region.z.max(0));
    let mut r = Range::new();
    for z in z0..depth {
        for y in y0..height {
            // each row's pointer is taken before its voxels (`&MRIFvox(mri, x0, y, z)`)
            need_voxels(g, err, "MRIvalRangeRegion", mri)?;
            rows::touch(g, "MRIvalRangeRegion", mri, i64::from(z), i64::from(y));
            for x in x0..width {
                let val = if mri.mtype == MRI_FLOAT || mri.mtype == MRI_UCHAR {
                    rows::read(
                        g,
                        "MRIvalRangeRegion",
                        mri,
                        i64::from(z),
                        i64::from(y),
                        i64::from(x),
                        mri.mtype,
                    )
                    .f()
                } else {
                    mri_get_vox_val(g, err, mri, x, y, z, 0, 0)?
                };
                r.see(val);
            }
        }
    }
    Ok((NO_ERROR, r.min, r.max))
}

/// `MRIlabelValRange(mri, mri_labeled, label, &min, &max)`: the range of the voxels (every frame)
/// whose label (`nint` of the labelled volume's voxel at the same place and frame) is `label`,
/// starting from the first such voxel (10000 and -10000 when there is none); `NO_ERROR`.
pub fn mri_label_val_range(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    labeled: &Mri,
    label: i32,
) -> Result<(i32, f32, f32), Fatal> {
    let mut r = Range::new();
    let mut first = true;
    for f in 0..mri.nframes {
        for z in 0..mri.vg.depth {
            for y in 0..mri.vg.height {
                for x in 0..mri.vg.width {
                    let val = mri_get_vox_val(g, err, mri, x, y, z, f, 0)?;
                    let lv = mri_get_vox_val(g, err, labeled, x, y, z, f, 0)?;
                    if nint(g, f64::from(lv)) != label {
                        continue;
                    }
                    if first {
                        first = false;
                        r.min = val;
                        r.max = val;
                    }
                    r.see(val);
                }
            }
        }
    }
    Ok((NO_ERROR, r.min, r.max))
}

/// C's conversion of a `float` in [0, 255] or NaN to `unsigned char` (truncation; a NaN is
/// undefined: x86-64 gives 0, FS-296's default 0 as well).
fn float_to_uchar(g: &mut Globals, what: &str, v: f32) -> i64 {
    if v.is_nan() {
        g.notices
            .add(&FS_296, format!("{what}: a NaN stored into a uchar voxel"));
        return 0;
    }
    v as i64
}

/// `MRIscalarMulFrame(mri_src, mri_dst, scalar, frame)`: a clone without a destination; the rows of
/// `frame` through the row pointers by the source's type (`uchar`: `v * scalar` clipped to
/// [0, 255] and truncated; `float`: the product; `short`, `ushort`: `nint((float) v * scalar)` in
/// the type's bits); another type: `MRIscalarMulFrame: unsupported type <type>` and `NULL` (at its
/// first row). A frame outside the volumes: FS-289, FS-290.
pub fn mri_scalar_mul_frame(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    scalar: f32,
    frame: i32,
) -> Result<Ret<Mri>, Fatal> {
    let (mut v, _) = Vols::of_pair(p);
    clone_into(g, out, err, &mut v)?;
    let same = matches!(v.s1, S::Dst);
    let (w, h, d, ty) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth, v.a().mtype);
    const WHAT: &str = "MRIscalarMulFrame";
    for z in 0..d {
        for y in 0..h {
            if !matches!(ty, MRI_UCHAR | MRI_FLOAT | MRI_SHORT | MRI_USHRT) {
                log::error_printf(
                    g,
                    err,
                    ERROR_UNSUPPORTED,
                    &format!("MRIscalarMulFrame: unsupported type {ty}"),
                );
                return Ok(v.null());
            }
            need_voxels(g, err, WHAT, v.a())?;
            need_voxels(g, err, WHAT, v.dref())?;
            let sls = i64::from(z) + i64::from(frame) * i64::from(v.a().vg.depth);
            let sld = i64::from(z) + i64::from(frame) * i64::from(v.dref().vg.depth);
            rows::touch(g, WHAT, v.a(), sls, i64::from(y));
            rows::touch(g, WHAT, v.dref(), sld, i64::from(y));
            for x in 0..w {
                let s = rows::read(g, WHAT, v.a(), sls, i64::from(y), i64::from(x), ty);
                let r = match ty {
                    MRI_UCHAR => {
                        let mut dv = mul(s.f(), scalar);
                        if dv < 0.0 {
                            dv = 0.0;
                        }
                        if dv > 255.0 {
                            dv = 255.0;
                        }
                        Num::I(float_to_uchar(g, WHAT, dv))
                    }
                    // GCC's vectorised loop (rows apart, at least 4 voxels): the third of
                    // three leftover voxels is computed as scalar * v (the scalar's NaN wins)
                    MRI_FLOAT if !same && w >= 4 && w % 4 == 3 && x == w - 1 => {
                        Num::F(mul(scalar, s.f()))
                    }
                    MRI_FLOAT => Num::F(mul(s.f(), scalar)),
                    _ => Num::I(i64::from(nint(g, f64::from(mul(s.f(), scalar))))),
                };
                rows::write(g, WHAT, v.d(), sld, i64::from(y), i64::from(x), ty, r);
            }
        }
    }
    Ok(v.done())
}

/// The `uchar`-only functions given another type (FS-291): FreeSurfer works on the first `width`
/// bytes of each row as `uchar` voxels; by default an error.
fn uchar_only(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    types: &[i32],
) -> Result<(), Fatal> {
    if let Some(&t) = types.iter().find(|&&t| t != MRI_UCHAR) {
        g.notices.add(
            &FS_291,
            format!("{what} of a volume of type {t}: its bytes taken as uchar voxels"),
        );
        if !g.notices.reproduce() {
            log::error_exit(
                g,
                err,
                ERROR_UNSUPPORTED,
                &format!("{what}: unsupported type {t} (uchar volumes only)"),
            )?;
        }
    }
    Ok(())
}

/// `MRIinvertContrast(mri_src, mri_dst, threshold)`: a clone without a destination; each `uchar`
/// voxel of frame 0 above `threshold` replaced by `255 - v`, the others copied. Another type: FS-291.
pub fn mri_invert_contrast(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    threshold: f32,
) -> Result<Ret<Mri>, Fatal> {
    let (mut v, _) = Vols::of_pair(p);
    clone_into(g, out, err, &mut v)?;
    const WHAT: &str = "MRIinvertContrast";
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    let types = [v.a().mtype, v.dref().mtype];
    if w > 0 && h > 0 && d > 0 {
        uchar_only(g, err, WHAT, &types)?;
    }
    for z in 0..d {
        for y in 0..h {
            need_voxels(g, err, WHAT, v.a())?;
            need_voxels(g, err, WHAT, v.dref())?;
            for x in 0..w {
                let Num::I(mut val) = rows::read(
                    g,
                    WHAT,
                    v.a(),
                    i64::from(z),
                    i64::from(y),
                    i64::from(x),
                    MRI_UCHAR,
                ) else {
                    unreachable!()
                };
                if (val as f32) > threshold {
                    val = 255 - val;
                }
                rows::write(
                    g,
                    WHAT,
                    v.d(),
                    i64::from(z),
                    i64::from(y),
                    i64::from(x),
                    MRI_UCHAR,
                    Num::I(val),
                );
            }
        }
    }
    Ok(v.done())
}

/// `MRIscaleAndMultiply(mri1, scale, mri2, mri_dst)`: without a destination a new one-frame volume
/// of the first's extent and type with its header; each `uchar` voxel of frame 0 `nint(v1 * (v2 /
/// scale))` (in `float`, clipped to [0, 255]). Another type: FS-291.
pub fn mri_scale_and_multiply(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: Tri,
    scale: f32,
) -> Result<Ret<Mri>, Fatal> {
    let mut v = Vols::of_tri(t);
    alloc_like(g, out, err, &mut v, 1, None, false)?;
    const WHAT: &str = "MRIscaleAndMultiply";
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    let types = [v.a().mtype, v.b().mtype, v.dref().mtype];
    if w > 0 && h > 0 && d > 0 {
        uchar_only(g, err, WHAT, &types)?;
    }
    for z in 0..d {
        for y in 0..h {
            need_voxels(g, err, WHAT, v.a())?;
            need_voxels(g, err, WHAT, v.b())?;
            need_voxels(g, err, WHAT, v.dref())?;
            for x in 0..w {
                let (zz, yy, xx) = (i64::from(z), i64::from(y), i64::from(x));
                let p1 = rows::read(g, WHAT, v.a(), zz, yy, xx, MRI_UCHAR).f();
                let p2 = rows::read(g, WHAT, v.b(), zz, yy, xx, MRI_UCHAR).f();
                let mut o = mul(p1, div(p2, scale));
                if o > 255.0 {
                    o = 255.0;
                } else if o < 0.0 {
                    o = 0.0;
                }
                let n = nint(g, f64::from(o));
                rows::write(g, WHAT, v.d(), zz, yy, xx, MRI_UCHAR, Num::I(i64::from(n)));
            }
        }
    }
    Ok(v.done())
}

fn div(a: f32, b: f32) -> f32 {
    larmorx_freesurfer_core::sse::div(a, b)
}

/// `MRIdivide(mri1, mri2, mri_dst)`: without a destination a new one-frame `float` volume of the
/// first's extent with the first's header. Unless all three are `uchar`: each voxel of frame 0
/// `v1 / v2` in `double` (0 where `FZERO(v2)`), written by `MRIsetVoxVal`. All `uchar`: the integer
/// quotient, and where `v2` is 0, 0 when `v1` is 0 else 255; FreeSurfer then does not move on in the
/// first volume and the destination (FS-292: the rest of the row is computed from the wrong voxels
/// and written one place early): `reproduce` does that, the default moves on.
pub fn mri_divide(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: Tri,
) -> Result<Ret<Mri>, Fatal> {
    let mut v = Vols::of_tri(t);
    alloc_like(g, out, err, &mut v, 1, Some(MRI_FLOAT), false)?;
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    if v.a().mtype != MRI_UCHAR || v.b().mtype != MRI_UCHAR || v.dref().mtype != MRI_UCHAR {
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let v1 = f64::from(getv(g, err, &v, true, x, y, z, 0)?);
                    let v2 = f64::from(getv(g, err, &v, false, x, y, z, 0)?);
                    let q = if v2.abs() < f64::from(f32::EPSILON) {
                        0.0
                    } else {
                        div64(v1, v2)
                    };
                    setv(g, err, &mut v, x, y, z, 0, q as f32)?;
                }
            }
        }
        return Ok(v.done());
    }
    const WHAT: &str = "MRIdivide";
    let reproduce = g.notices.reproduce();
    for z in 0..d {
        for y in 0..h {
            need_voxels(g, err, WHAT, v.a())?;
            need_voxels(g, err, WHAT, v.b())?;
            need_voxels(g, err, WHAT, v.dref())?;
            let (zz, yy) = (i64::from(z), i64::from(y));
            let fs = divide_walk(g, &v, zz, yy, i64::from(w), true);
            let lagging = fs.iter().enumerate().any(|(i, &(x, _))| x != i as i64);
            let fixed = if lagging {
                divide_walk(g, &v, zz, yy, i64::from(w), false)
            } else {
                fs.clone()
            };
            if fs != fixed {
                g.notices.add(
                    &FS_292,
                    format!("MRIdivide: a uchar row ({y}, {z}) with a zero divisor"),
                );
            }
            for (x, val) in if reproduce { fs } else { fixed } {
                rows::write(g, WHAT, v.d(), zz, yy, x, MRI_UCHAR, Num::I(val));
            }
        }
    }
    Ok(v.done())
}

/// One row of `MRIdivide`'s `uchar` path: the writes `(column, value)` in order, FreeSurfer's walk
/// when `lag` (the first volume's and the destination's places stay where a divisor was 0), the
/// corrected one otherwise. A source that is the destination reads the row's earlier writes.
fn divide_walk(g: &mut Globals, v: &Vols, zz: i64, yy: i64, w: i64, lag: bool) -> Vec<(i64, i64)> {
    const WHAT: &str = "MRIdivide";
    let mut writes: Vec<(i64, i64)> = Vec::new();
    let d = v.dref();
    let rd = |g: &mut Globals, is_dst: bool, m: &Mri, x: i64, writes: &[(i64, i64)]| -> i64 {
        if is_dst
            && matches!(rows::at(d, zz, yy, x, 1), rows::At::In(_))
            && let Some(&(_, val)) = writes.iter().rev().find(|(i, _)| *i == x)
        {
            return val;
        }
        int_of(rows::read(g, WHAT, m, zz, yy, x, MRI_UCHAR))
    };
    let (mut i1, mut id) = (0i64, 0i64);
    for i2 in 0..w {
        let q2 = rd(g, matches!(v.s2, S::Dst), v.b(), i2, &writes);
        let q1 = rd(g, matches!(v.s1, S::Dst), v.a(), i1, &writes);
        if q2 == 0 {
            writes.push((id, if q1 == 0 { 0 } else { 255 }));
            if !lag {
                i1 += 1;
                id += 1;
            }
        } else {
            writes.push((id, q1 / q2));
            i1 += 1;
            id += 1;
        }
    }
    writes
}

fn int_of(n: Num) -> i64 {
    match n {
        Num::I(v) => v,
        Num::F(f) => f as i64,
    }
}

/// The direct sums and differences' arithmetic (`MRIsubtract`, `MRIadd` on two volumes of one
/// type, not `uchar` for `MRIadd`): a source value read through the row pointers.
fn row_val(g: &mut Globals, what: &str, m: &Mri, sl: i64, y: i64, x: i64) -> Option<Num> {
    if elem_size(m.mtype) == 0 {
        return None;
    }
    Some(rows::read(g, what, m, sl, y, x, m.mtype))
}

/// `(type) nint(v)` for an integer destination type: `nint` (FS-12) then C's conversion to the
/// type's width (the low bits; rule C: FreeSurfer's direct paths wrap where `MRIsetVoxVal` clips).
fn nint_to(g: &mut Globals, v: f32) -> Num {
    Num::I(i64::from(nint(g, f64::from(v))))
}

/// `MRIsubtract(mri1, mri2, mri_dst)`: without a destination a new volume of the first's extent,
/// type and frames (1 for a volume of 0 frames) with its header. Volumes of different types: each
/// voxel `v1 - v2` in `float` by `MRIgetVoxVal`/`MRIsetVoxVal`. One type: through the row pointers
/// (the second volume and the destination walked with the first's extent and frames: FS-289,
/// FS-290), `(float) v1 - (float) v2`, stored as `nint` in the destination's integer type's bits
/// (wrapping, not clipping) or as the `float`; a type without a direct path (`RGB`, complex) keeps
/// the previous difference (0 at first), a destination of such a type is not written.
pub fn mri_subtract(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: Tri,
) -> Result<Ret<Mri>, Fatal> {
    let mut v = Vols::of_tri(t);
    let nframes = if v.a().nframes == 0 { 1 } else { v.a().nframes };
    alloc_like(g, out, err, &mut v, nframes, None, true)?;
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    if v.a().mtype != v.b().mtype {
        for f in 0..nframes {
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        let v1 = getv(g, err, &v, true, x, y, z, f)?;
                        let v2 = getv(g, err, &v, false, x, y, z, f)?;
                        setv(g, err, &mut v, x, y, z, f, sub(v1, v2))?;
                    }
                }
            }
        }
        return Ok(v.done());
    }
    const WHAT: &str = "MRIsubtract";
    let mut val = 0.0f32;
    let mut s: i64 = 0;
    for _f in 0..nframes {
        for _z in 0..d {
            for y in 0..h {
                let dty = v.dref().mtype;
                let rows_used = h > 0 && w > 0;
                if rows_used {
                    if elem_size(dty) > 0 {
                        need_voxels(g, err, WHAT, v.dref())?;
                    }
                    if elem_size(v.a().mtype) > 0 {
                        need_voxels(g, err, WHAT, v.a())?;
                        need_voxels(g, err, WHAT, v.b())?;
                    }
                }
                let yy = i64::from(y);
                // the row pointers FreeSurfer takes for this row (each type with a direct path)
                if elem_size(dty) > 0 {
                    rows::touch(g, WHAT, v.dref(), s, yy);
                }
                if elem_size(v.a().mtype) > 0 {
                    rows::touch(g, WHAT, v.a(), s, yy);
                    rows::touch(g, WHAT, v.b(), s, yy);
                }
                for x in 0..i64::from(w) {
                    if let Some(a) = row_val(g, WHAT, v.a(), s, yy, x) {
                        let b = row_val(g, WHAT, v.b(), s, yy, x).expect("one type");
                        val = sub(a.f(), b.f());
                    }
                    let r = match dty {
                        MRI_FLOAT => Some(Num::F(val)),
                        t if elem_size(t) > 0 => Some(nint_to(g, val)),
                        _ => None,
                    };
                    if let Some(r) = r {
                        rows::write(g, WHAT, v.d(), s, yy, x, dty, r);
                    }
                }
            }
            s += 1;
        }
    }
    Ok(v.done())
}

/// `MRIadd(mri1, mri2, mri_dst)`: as [`mri_subtract`] with the sum, and: a first volume of type
/// `uchar` or of a type other than the second's takes the voxel path (`v1 + v2` in `float`,
/// clipped to [0, 255] for a `uchar` destination); one type (not `uchar`) the direct path, where
/// the sum is formed in the source type's C arithmetic (`int` for `short` and `ushort`, wrapping for
/// `int` and `long`, `float` for `float`), then converted to the destination's type (an integer
/// type's low bits; `nint` of a `float` sum for an integer destination; a `float` destination the
/// first value as `float` plus the second converted to `float`).
pub fn mri_add(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: Tri,
) -> Result<Ret<Mri>, Fatal> {
    let mut v = Vols::of_tri(t);
    let nframes = if v.a().nframes == 0 { 1 } else { v.a().nframes };
    alloc_like(g, out, err, &mut v, nframes, None, true)?;
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    if v.a().mtype == MRI_UCHAR || v.a().mtype != v.b().mtype {
        for f in 0..nframes {
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        let v1 = getv(g, err, &v, true, x, y, z, f)?;
                        let v2 = getv(g, err, &v, false, x, y, z, f)?;
                        // the official build adds v1 to v2 (v2 in the destination register)
                        let mut val = add(v2, v1);
                        if v.dref().mtype == MRI_UCHAR && val > 255.0 {
                            val = 255.0;
                        }
                        if v.dref().mtype == MRI_UCHAR && val < 0.0 {
                            val = 0.0;
                        }
                        setv(g, err, &mut v, x, y, z, f, val)?;
                    }
                }
            }
        }
        return Ok(v.done());
    }
    const WHAT: &str = "MRIadd";
    let sty = v.a().mtype;
    let mut s: i64 = 0;
    for _f in 0..nframes {
        for _z in 0..d {
            for y in 0..h {
                let dty = v.dref().mtype;
                if h > 0 && w > 0 {
                    if elem_size(dty) > 0 {
                        need_voxels(g, err, WHAT, v.dref())?;
                    }
                    if elem_size(sty) > 0 {
                        need_voxels(g, err, WHAT, v.a())?;
                        need_voxels(g, err, WHAT, v.b())?;
                    }
                }
                let yy = i64::from(y);
                // the row pointers FreeSurfer takes for this row (each type with a direct path)
                if elem_size(dty) > 0 {
                    rows::touch(g, WHAT, v.dref(), s, yy);
                }
                if elem_size(sty) > 0 {
                    rows::touch(g, WHAT, v.a(), s, yy);
                    rows::touch(g, WHAT, v.b(), s, yy);
                }
                for x in 0..i64::from(w) {
                    if elem_size(dty) == 0 || elem_size(sty) == 0 {
                        continue;
                    }
                    let a = rows::read(g, WHAT, v.a(), s, yy, x, sty);
                    let b = rows::read(g, WHAT, v.b(), s, yy, x, sty);
                    let r = add_cast(g, sty, dty, a, b);
                    rows::write(g, WHAT, v.d(), s, yy, x, dty, r);
                }
            }
            s += 1;
        }
    }
    Ok(v.done())
}

/// One element of `MRIadd`'s direct path: the sum of `a` and `b` (source type `sty`) as the C
/// expression for destination type `dty` computes it.
fn add_cast(g: &mut Globals, sty: i32, dty: i32, a: Num, b: Num) -> Num {
    match (sty, a, b) {
        (MRI_FLOAT, Num::F(x), Num::F(y)) => {
            let s = add(x, y);
            if dty == MRI_FLOAT {
                Num::F(s)
            } else {
                nint_to(g, s)
            }
        }
        (_, Num::I(x), Num::I(y)) => {
            if dty == MRI_FLOAT {
                // (float)x + y: y converted to float, the sum in float
                return Num::F(add(x as f32, y as f32));
            }
            let s = match sty {
                // int arithmetic: the promoted sum (cannot overflow for short and ushort)
                MRI_SHORT | MRI_USHRT => x + y,
                // (int)x + y with dty long: long arithmetic; otherwise int arithmetic (wraps)
                MRI_INT if dty == MRI_LONG => x + y,
                MRI_INT => i64::from((x as i32).wrapping_add(y as i32)),
                _ => x.wrapping_add(y),
            };
            Num::I(s)
        }
        _ => unreachable!("one type"),
    }
}

/// `MRIcalcCRASforExtractedVolume(src, dst, x0, y0, z0, x1, y1, z1)`: the scanner RAS (as `float`s
/// widened to `double`) of the source voxel `(x0 + dst.width / 2.0 - x1, ...)` (in `double`, rounded
/// to `float`) through the source's cached voxel-to-RAS matrix; when the source has none, both its
/// cached matrices are computed and kept first. With `Gdiag & DIAG_SHOW` and `DIAG_VERBOSE_ON`, the
/// line `c_ras for sample volume is (...) compared with the src (...)` on standard error.
#[allow(clippy::too_many_arguments)]
pub fn mri_calc_cras_for_extracted_volume(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &mut Mri,
    dst: &Mri,
    x0: i32,
    y0: i32,
    z0: i32,
    x1: i32,
    y1: i32,
    z1: i32,
) -> Result<(f64, f64, f64), Fatal> {
    let cx = f64::from(x0) + f64::from(dst.vg.width) / 2.0 - f64::from(x1);
    let cy = f64::from(y0) + f64::from(dst.vg.height) / 2.0 - f64::from(y1);
    let cz = f64::from(z0) + f64::from(dst.vg.depth) / 2.0 - f64::from(z1);
    if src.vg.i_to_r.is_none() {
        let tmp = extract_i_to_r(g, out, err, &src.vg)?;
        let mut am = AffineMatrix::default();
        set_affine_matrix(err, &mut am, &tmp)?;
        src.vg.i_to_r = Some(am);
        src.vg.r_to_i = extract_r_to_i(g, out, err, &src.vg)?;
    }
    // AffineMV as the official build inlines it here (read from its object code): each column
    // times the point's coordinate (the coordinate the destination operand), the three products
    // summed in order, then the fourth column added
    let (x, y, z) = (cx as f32, cy as f32, cz as f32);
    let m = src.vg.i_to_r.as_ref().expect("cached").mat;
    let lane = |i: usize| {
        add(
            add(add(mul(x, m[i]), mul(y, m[4 + i])), mul(z, m[8 + i])),
            m[12 + i],
        )
    };
    let (px, py, pz) = (lane(0), lane(1), lane(2));
    let (pr, pa, ps) = (f64::from(px), f64::from(py), f64::from(pz));
    if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
        let f = |x: f64| larmorx_core::cfmt::f(x, 6);
        let _ = writeln!(
            err,
            "c_ras for sample volume is ({}, {}, {}) compared with the src ({}, {}, {})",
            f(pr),
            f(pa),
            f(ps),
            f(f64::from(src.vg.c_r)),
            f(f64::from(src.vg.c_a)),
            f(f64::from(src.vg.c_s))
        );
    }
    Ok((pr, pa, ps))
}

/// `MRIextract(mri_src, mri_dst, x0, y0, z0, dx, dy, dz)`: [`mri_extract_into`] at 0, 0, 0.
#[allow(clippy::too_many_arguments)]
pub fn mri_extract(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &mut Mri,
    dst: Option<&mut Mri>,
    x0: i32,
    y0: i32,
    z0: i32,
    dx: i32,
    dy: i32,
    dz: i32,
) -> Result<Ret<Mri>, Fatal> {
    mri_extract_into(g, out, err, src, dst, [x0, y0, z0], [dx, dy, dz], [0, 0, 0])
}

/// `MRIextractRegion(mri_src, mri_dst, region)`: [`mri_extract_into`] of the region (the whole
/// volume without one) at 0, 0, 0.
pub fn mri_extract_region(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &mut Mri,
    dst: Option<&mut Mri>,
    region: Option<&MriRegion>,
) -> Result<Ret<Mri>, Fatal> {
    let r = region.copied().unwrap_or(MriRegion {
        x: 0,
        y: 0,
        z: 0,
        dx: src.vg.width,
        dy: src.vg.height,
        dz: src.vg.depth,
    });
    mri_extract_into(
        g,
        out,
        err,
        src,
        dst,
        [r.x, r.y, r.z],
        [r.dx, r.dy, r.dz],
        [0, 0, 0],
    )
}

/// `(int)` of a voxel size (C's truncation; NaN and values outside `int` undefined: x86-64's
/// `INT_MIN`, FS-12's rule).
fn size_to_int(g: &mut Globals, x: f32) -> i32 {
    let d = f64::from(x);
    if d.is_nan() || d >= 2147483648.0 || d <= -2147483649.0 {
        g.notices.add(
            &larmorx_core::defects::FS_12,
            format!("MRIextractInto: a voxel size of {d} converted to int"),
        );
        if g.notices.reproduce() {
            return i32::MIN;
        }
        return if d.is_nan() {
            0
        } else if d > 0.0 {
            i32::MAX
        } else {
            i32::MIN
        };
    }
    d.trunc() as i32
}

/// `MRIextractInto(mri_src, mri_dst, x0, y0, z0, dx, dy, dz, x1, y1, z1)`: copies the box of `dx x dy
/// x dz` voxels at `(x0, y0, z0)` of every frame into the destination at `(x1, y1, z1)`.
///
/// - A start at or past the extent prints `MRIextractInto: bad src location (x0, y0, z0)` (an error,
///   `ERROR_BADPARM`) and goes on.
/// - Without a destination: a new volume of `dx x dy x dz`, the source's type and frames, with its
///   header, `imnr0 = z0 + src.imnr0 - z1`, `imnr1 = imnr0 + dz - 1`.
/// - A destination of another type: `MRIextractInto: src and dst types must match` and `NULL`
///   (FreeSurfer frees the caller's volume: FS-293, kept here).
/// - The box is clipped: starts below 0 raised to 0, sizes cut to the source's and then the
///   destination's extent.
/// - A new destination's `xstart` gains `x0 * xsize` and `xend = xstart + dx * xsize` (the same for
///   `y` and `z`), with the voxel sizes truncated to integers (FS-295; by default the sizes as they
///   are, in `float`).
/// - A type without a direct copy ends the program (`MRIextractInto: unsupported source type <t>`).
/// - Rows of `dx` voxels are copied (a negative `dx`, a start past the extent, makes FreeSurfer copy
///   a huge length and crash: FS-294, an error in both modes).
/// - Then the destination takes the source's voxel sizes, `thick`, `ps` and pulse parameters, its
///   centre from [`mri_calc_cras_for_extracted_volume`], its cached matrices recomputed and the
///   source's colour table (a deep copy) when it has one.
#[allow(clippy::too_many_arguments)]
pub fn mri_extract_into(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &mut Mri,
    dst: Option<&mut Mri>,
    p0: [i32; 3],
    size: [i32; 3],
    p1: [i32; 3],
) -> Result<Ret<Mri>, Fatal> {
    let [mut x0, mut y0, mut z0] = p0;
    let [mut dx, mut dy, mut dz] = size;
    let [mut x1, mut y1, mut z1] = p1;
    let (width, height, depth) = (src.vg.width, src.vg.height, src.vg.depth);
    if z0 >= depth || y0 >= height || x0 >= width {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("MRIextractInto: bad src location ({x0}, {y0}, {z0})"),
        );
    }
    let mut fresh = None;
    let dst_alloced = dst.is_none();
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let mut n = mri_alloc_sequence(g, out, err, dx, dy, dz, src.mtype, src.nframes)?;
            mri_copy_header(g, out, err, src, Some(&mut n))?;
            n.imnr0 = z0.wrapping_add(src.imnr0).wrapping_sub(z1);
            n.imnr1 = n.imnr0.wrapping_add(dz).wrapping_sub(1);
            fresh.insert(n)
        }
    };
    if src.mtype != d.mtype {
        g.notices.add(
            &FS_293,
            "MRIextractInto: a destination of another type (FreeSurfer frees the caller's volume)",
        );
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            "MRIextractInto: src and dst types must match",
        );
        return Ok(Ret::Null);
    }
    x0 = x0.max(0);
    y0 = y0.max(0);
    z0 = z0.max(0);
    if x0.wrapping_add(dx) > width {
        dx = width.wrapping_sub(x0);
    }
    if y0.wrapping_add(dy) > height {
        dy = height.wrapping_sub(y0);
    }
    if z0.wrapping_add(dz) > depth {
        dz = depth.wrapping_sub(z0);
    }
    x1 = x1.max(0);
    y1 = y1.max(0);
    z1 = z1.max(0);
    if x1.wrapping_add(dx) > d.vg.width {
        dx = d.vg.width.wrapping_sub(x1);
    }
    if y1.wrapping_add(dy) > d.vg.height {
        dy = d.vg.height.wrapping_sub(y1);
    }
    if z1.wrapping_add(dz) > d.vg.depth {
        dz = d.vg.depth.wrapping_sub(z1);
    }
    let (xs, ys, zs) = (
        size_to_int(g, src.vg.xsize),
        size_to_int(g, src.vg.ysize),
        size_to_int(g, src.vg.zsize),
    );
    if dst_alloced {
        let truncated = [
            xs as f32 != src.vg.xsize,
            ys as f32 != src.vg.ysize,
            zs as f32 != src.vg.zsize,
        ];
        if truncated.iter().any(|&t| t) && [x0, dx, y0, dy, z0, dz].iter().any(|&k| k != 0) {
            g.notices.add(&FS_295, "MRIextractInto: the new volume's start and end with the voxel sizes truncated to integers");
        }
        let reproduce = g.notices.reproduce();
        // an integral voxel size gives FreeSurfer's product in both modes (its zero signs too)
        let step = |k: i32, si: i32, sf: f32| -> f32 {
            if reproduce || si as f32 == sf {
                k.wrapping_mul(si) as f32
            } else {
                mul(k as f32, sf)
            }
        };
        d.xstart = add(d.xstart, step(x0, xs, src.vg.xsize));
        d.xend = add(d.xstart, step(dx, xs, src.vg.xsize));
        d.ystart = add(d.ystart, step(y0, ys, src.vg.ysize));
        d.yend = add(d.ystart, step(dy, ys, src.vg.ysize));
        d.zstart = add(d.zstart, step(z0, zs, src.vg.zsize));
        d.zend = add(d.zstart, step(dz, zs, src.vg.zsize));
    }
    let sz = elem_size(src.mtype);
    if src.mtype == MRI_LONG
        && dx > 0
        && dy > 0
        && dz > 0
        && src.nframes > 0
        && (x0 != 0 || x1 != 0)
    {
        g.notices.add(
            &FS_297,
            "MRIextractInto of a long volume: rows copied from half their column",
        );
    }
    let reproduce_long = g.notices.reproduce();
    if sz == 0 {
        let msg = format!("MRIextractInto: unsupported source type {}", src.mtype);
        log::error_exit(g, err, ERROR_UNSUPPORTED, &msg)?;
    }
    for frame in 0..src.nframes {
        let (mut zd, mut zs) = (z1, z0);
        while zs < z0.wrapping_add(dz) {
            let (mut yd, mut ys) = (y1, y0);
            while ys < y0.wrapping_add(dy) {
                if dx < 0 {
                    g.notices.add(&FS_294, format!("MRIextractInto: a row of {dx} voxels (FreeSurfer copies a huge length and crashes)"));
                    log::error_exit(
                        g,
                        err,
                        ERROR_BADPARM,
                        &format!(
                            "MRIextractInto: a negative row length {dx} (FreeSurfer crashes here)\n"
                        ),
                    )?;
                }
                need_voxels(g, err, "MRIextractInto", src)?;
                need_voxels(g, err, "MRIextractInto", d)?;
                let ssl = i64::from(zs) + i64::from(frame) * i64::from(depth);
                let dsl = i64::from(zd) + i64::from(frame) * i64::from(d.vg.depth);
                let src_row = rows::touch(g, "MRIextractInto", src, ssl, i64::from(ys));
                let dst_row = rows::touch(g, "MRIextractInto", d, dsl, i64::from(yd));
                // memmove of dx voxels' bytes from the source's row at x0 to the destination's at
                // x1; for MRI_LONG FreeSurfer's row pointers step 4 bytes (`long32`): both start at
                // half their column (FS-297; by default at the column)
                let len = dx as usize * sz;
                let step = if src.mtype == MRI_LONG && reproduce_long {
                    4
                } else {
                    sz as i64
                };
                let so = rows::row_bytes_at(src, ssl, i64::from(ys), i64::from(x0) * step, len);
                let dof = rows::row_bytes_at(d, dsl, i64::from(yd), i64::from(x1) * step, len);
                match (so, dof) {
                    (Some(so), Some(dof)) if src_row && dst_row => {
                        let bytes = src.chunk.as_ref().expect("voxels")[so..so + len].to_vec();
                        d.chunk.as_mut().expect("voxels")[dof..dof + len].copy_from_slice(&bytes);
                    }
                    _ if len == 0 => {}
                    _ => {
                        g.notices.add(
                            &larmorx_core::defects::FS_290,
                            "MRIextractInto copies a row outside a volume",
                        );
                    }
                }
                ys += 1;
                yd += 1;
            }
            zs += 1;
            zd += 1;
        }
    }
    d.vg.xsize = src.vg.xsize;
    d.vg.ysize = src.vg.ysize;
    d.vg.zsize = src.vg.zsize;
    d.thick = src.thick;
    d.ps = src.ps;
    mri_copy_pulse_parameters(src, d);
    let (cr, ca, cs) =
        mri_calc_cras_for_extracted_volume(g, out, err, src, d, x0, y0, z0, x1, y1, z1)?;
    d.vg.c_r = cr as f32;
    d.vg.c_a = ca as f32;
    d.vg.c_s = cs as f32;
    reinit_cache(g, out, err, &mut d.vg)?;
    if src.ct.is_some() {
        d.ct = ctab_deep_copy(g, err, src.ct.as_ref());
    }
    Ok(match fresh {
        Some(n) => Ret::New(n),
        None => Ret::Given,
    })
}

/// `MRIsampleVolumeFrameType(mri, x, y, z, frame, SAMPLE_NEAREST, &val)` at a voxel's integer
/// coordinates (the case every caller here takes: whole coordinates always sample the nearest
/// voxel): `outside_val` for a point unambiguously outside, else the voxel of `frame` at the
/// coordinates (clamped to the volume) as a `float`, read through the row pointers (a frame the
/// volume does not have reads memory it does not own: FS-289, FS-290); another type gives 0 and
/// the error `MRIsampleVolumeFrameType: unsupported volume type <t>` (`ERROR_UNSUPPORTED`).
#[allow(clippy::too_many_arguments)]
pub fn sample_volume_frame_nearest(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
    frame: i32,
) -> (i32, f64) {
    crate::sample::sample_volume_frame_nearest(
        g,
        err,
        mri,
        f64::from(x),
        f64::from(y),
        f64::from(z),
        frame,
    )
}

/// `MRIaverage(mri_src, dof, mri_dst)`: without a destination a new one-frame volume of the
/// source's extent and type (`MRIalloc`) with its header; a destination whose extent differs from
/// the source's gives the error `MRIaverage: incompatible volume dimensions` and `NULL`. Then for
/// each frame of the destination and each voxel (x innermost) the running average `(dof * dst +
/// src) / (dof + 1)` in `double` (both voxels sampled nearest, [`sample_volume_frame_nearest`];
/// the product taken `dof` first, as the official build does; `dof + 1` in `int`), stored with
/// `MRIsetVoxVal`; the destination's `dof` incremented.
pub fn mri_average(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    dof: i32,
) -> Result<Ret<Mri>, Fatal> {
    let (mut v, _) = Vols::of_pair(p);
    alloc_like(g, out, err, &mut v, 1, None, false)?;
    if mri_check_size(v.a(), Some(v.dref()), 0, 0, 0) == 0 {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            "MRIaverage: incompatible volume dimensions",
        );
        return Ok(v.null());
    }
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    let n = f64::from(dof);
    let n1 = f64::from(dof.wrapping_add(1));
    for f in 0..v.dref().nframes {
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let (_, s) = sample_volume_frame_nearest(g, err, v.a(), x, y, z, f);
                    let (_, t) = sample_volume_frame_nearest(g, err, v.dref(), x, y, z, f);
                    let val = div64(add64(mul64(n, t), s), n1) as f32;
                    setv(g, err, &mut v, x, y, z, f, val)?;
                }
            }
        }
    }
    v.d().dof = v.d().dof.wrapping_add(1);
    Ok(v.done())
}

// ---- utils/mriset.cpp (3.1-38) ---------------------------------------------------------------

/// `MRIcheckVolDims(mri1, mri2)`: with both volumes, a height, width or depth that differs ends
/// the program (`ERROR: <Progname>-MRIcheckVolDims: volume1 height=<h1> != volume2 height=<h2>.`,
/// then width, then depth).
fn check_vol_dims(g: &mut Globals, err: &mut dyn Write, v: &Vols) -> Result<(), Fatal> {
    if !v.has_dst() {
        return Ok(());
    }
    check_dims(g, err, v.a(), v.dref())
}

/// `MRIcheckVolDims(a, b)` of two volumes (see [`check_vol_dims`]).
pub(crate) fn check_dims(
    g: &mut Globals,
    err: &mut dyn Write,
    a: &Mri,
    b: &Mri,
) -> Result<(), Fatal> {
    for (what, x, y) in [
        ("height", a.vg.height, b.vg.height),
        ("width", a.vg.width, b.vg.width),
        ("depth", a.vg.depth, b.vg.depth),
    ] {
        if x != y {
            let msg = format!(
                "ERROR: {}-MRIcheckVolDims: volume1 {what}={x} != volume2 {what}={y}.\n",
                g.progname
            );
            log::error_exit(g, err, ERROR_BADPARM, &msg)?;
        }
    }
    Ok(())
}

/// `MRIreplaceValues(mri_src, mri_dst, in_val, out_val)`: the source with every voxel `FEQUAL` to
/// `in_val` (`|v - in_val| < FLT_EPSILON` in `float`) replaced by `out_val`, written with
/// `MRIsetVoxVal` (z outermost, then y, x, frames); a byte source and destination go to
/// [`mri_replace_values_uchar`] with `nint` of both values as bytes. Without a destination, a
/// clone of the source (`MRIclone`); a destination of another extent ends the program
/// (`MRIcheckVolDims`). (FreeSurfer runs the slices in parallel; each voxel is its own.)
pub fn mri_replace_values(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    in_val: f32,
    out_val: f32,
) -> Result<Ret<Mri>, Fatal> {
    let (mut v, _) = Vols::of_pair(p);
    replace_values(g, out, err, &mut v, in_val, out_val)?;
    Ok(v.done())
}

/// `MRIreplaceValuesUchar(mri_src, mri_dst, in_val, out_val)`: as [`mri_replace_values`] on byte
/// volumes, through the row pointers; a source or destination of another type goes to
/// [`mri_replace_values`] with the bytes as `float`s. FS-461: FreeSurfer replaces in frame 0
/// only (the other frames of the destination as they were); the default replaces in every frame.
pub fn mri_replace_values_uchar(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    in_val: u8,
    out_val: u8,
) -> Result<Ret<Mri>, Fatal> {
    let (mut v, _) = Vols::of_pair(p);
    replace_values_uchar(g, out, err, &mut v, in_val, out_val)?;
    Ok(v.done())
}

fn replace_values(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    v: &mut Vols,
    in_val: f32,
    out_val: f32,
) -> Result<(), Fatal> {
    check_vol_dims(g, err, v)?;
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    clone_into(g, out, err, v)?;
    if v.a().mtype == MRI_UCHAR && v.dref().mtype == MRI_UCHAR {
        // (BUFTYPE)nint(...): the int's low byte
        let o = nint(g, f64::from(out_val)) as u8;
        let i = nint(g, f64::from(in_val)) as u8;
        return replace_values_uchar(g, out, err, v, i, o);
    }
    let nf = v.a().nframes;
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                for f in 0..nf {
                    let mut val = getv(g, err, v, true, x, y, z, f)?;
                    if fequal(val, in_val) {
                        val = out_val;
                    }
                    setv(g, err, v, x, y, z, f, val)?;
                }
            }
        }
    }
    Ok(())
}

fn replace_values_uchar(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    v: &mut Vols,
    in_val: u8,
    out_val: u8,
) -> Result<(), Fatal> {
    check_vol_dims(g, err, v)?;
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    clone_into(g, out, err, v)?;
    if v.a().mtype != MRI_UCHAR || v.dref().mtype != MRI_UCHAR {
        return replace_values(g, out, err, v, f32::from(in_val), f32::from(out_val));
    }
    let nf = v.a().nframes;
    if nf > 1 {
        g.notices.add(
            &FS_461,
            "MRIreplaceValues of a byte volume replaces in its first frame only",
        );
    }
    let frames = if g.notices.reproduce() { 1 } else { nf };
    const W: &str = "MRIreplaceValuesUchar";
    for f in 0..frames {
        for z in 0..d {
            for y in 0..h {
                let sl = i64::from(z) + i64::from(f) * i64::from(d);
                for x in 0..w {
                    let Num::I(b) =
                        rows::read(g, W, v.a(), sl, i64::from(y), i64::from(x), MRI_UCHAR)
                    else {
                        unreachable!("a byte")
                    };
                    let val = if b == i64::from(in_val) {
                        out_val
                    } else {
                        b as u8
                    };
                    rows::write(
                        g,
                        W,
                        v.d(),
                        sl,
                        i64::from(y),
                        i64::from(x),
                        MRI_UCHAR,
                        Num::I(i64::from(val)),
                    );
                }
            }
        }
    }
    Ok(())
}
