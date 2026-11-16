// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Voxel lists (`utils/voxlist.cpp`, 3.1-40): `VLSTcreate` (98), `VLSTsort` (217) with its
//! comparison, `VLSTfree` (254), `VLSTcomputeStats` (419), `VLSTalloc` (588), `VLSTcopy` (610),
//! `VLSTcopyInto` (649), `VLSTaddUnique` (688), `VLSTaddWithValue` (695), `VLSTadd` (709) and
//! `VLSTinList` (723).
//!
//! A list's nine arrays are vectors of the length FreeSurfer allocated (`calloc`); `nvox` and
//! `max_vox` are kept as FreeSurfer keeps them, apart from the arrays' lengths. The volume a list
//! was made from is a reference (`vl->mri`).
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-volume.md` §27):
//! - **FS-888** (rule A): `VLSTcreate` with `border_only` counts the voxels in the range whose six
//!   neighbours all are (the interior), and then stores the border voxels: the list's count is
//!   the interior's, and its arrays are too short when the border is larger. By default the
//!   border voxels are counted; `reproduce` keeps FreeSurfer's count (an error where it writes past
//!   the arrays).
//! - **FS-889** (a hang, memory): `VLSTcreate` with a negative `skip` never ends (-1) or reads
//!   frames before the first: an error in both modes.
//! - **FS-890** (memory): a list read or written past its arrays (an index or count beyond what was
//!   allocated, a sort of more voxels than its source holds): an error in both modes.
//! - **FS-891** (rule A): `VLSTsort` moves every field of the sorted voxels but their frame indices:
//!   by default the frames move with their voxels; `reproduce` leaves them.
//! - **FS-892** (rule C): `VLSTcreate` never sets `max_vox` (0 for a new list): `VLSTadd` then
//!   refuses to add. Kept (no program adds to such a list).
//! - **FS-893** (rule C): `VLSTcopy` and `VLSTcopyInto` give the copy new spline slope arrays of
//!   zeros (the source's slopes are not copied) and end the program when the source has slopes but
//!   no parameterisation. Kept (no program copies a spline list).

use std::io::Write;

use larmorx_core::defects::{FS_888, FS_889, FS_890, FS_891};
use larmorx_core::glibc_qsort::qsort;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_NOMEMORY, Fatal, NO_ERROR};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

use crate::deriv::neighbors_in_range;

/// `errno`'s `ENOMEM` on Linux (a failed `calloc`).
const ENOMEM: i32 = 12;

/// `VOXEL_LIST`.
#[derive(Clone, Debug, Default)]
pub struct VoxelList<'a> {
    /// `type`: `VOXLIST_NORMAL` (0) or `VOXLIST_SPLINE` (1).
    pub vtype: i32,
    pub xi: Vec<i32>,
    pub yi: Vec<i32>,
    pub zi: Vec<i32>,
    /// `fi`: the frame index.
    pub fi: Vec<i32>,
    /// `xd`, `yd`, `zd`: transformed coordinates.
    pub xd: Vec<f32>,
    pub yd: Vec<f32>,
    pub zd: Vec<f32>,
    pub vsrc: Vec<f32>,
    pub vdst: Vec<f32>,
    /// `mri`: the volume the list was made from (`NULL`: `None`).
    pub mri: Option<&'a Mri>,
    pub nvox: i32,
    pub max_vox: i32,
    pub mean: f64,
    pub std: f64,
    /// `mx`, `my`, `mz`: a spline's slopes; `t` its parameterisation (`NULL`: `None`).
    pub mx: Option<Vec<f32>>,
    pub my: Option<Vec<f32>>,
    pub mz: Option<Vec<f32>>,
    pub t: Option<Vec<f32>>,
}

/// FS-890: FreeSurfer reads or writes past the list's arrays here; an error in both modes.
fn past(g: &mut Globals, err: &mut dyn Write, what: &str, detail: String) -> Result<(), Fatal> {
    g.notices.add(&FS_890, format!("{what}: {detail} (past the list's arrays)"));
    log::error_exit(
        g,
        err,
        ERROR_BADPARM,
        &format!("{what}: {detail} (FreeSurfer reads or writes past the list's arrays here)\n"),
    )
}

/// Whether `i` indexes every one of the lengths.
fn within(i: i64, lens: &[usize]) -> bool {
    i >= 0 && lens.iter().all(|&n| (i as u64) < n as u64)
}

impl VoxelList<'_> {
    /// The lengths of the nine arrays.
    fn lens(&self) -> [usize; 9] {
        [
            self.xi.len(),
            self.yi.len(),
            self.zi.len(),
            self.fi.len(),
            self.xd.len(),
            self.yd.len(),
            self.zd.len(),
            self.vsrc.len(),
            self.vdst.len(),
        ]
    }

    /// The nine arrays of `n` zeros each (`calloc`).
    fn allocate(&mut self, n: usize) {
        self.xd = vec![0.0; n];
        self.yd = vec![0.0; n];
        self.zd = vec![0.0; n];
        self.vsrc = vec![0.0; n];
        self.vdst = vec![0.0; n];
        self.xi = vec![0; n];
        self.yi = vec![0; n];
        self.zi = vec![0; n];
        self.fi = vec![0; n];
    }
}

/// `VLSTcreate(mri, low_val, hi_val, vl, skip, border_only)`: the voxels whose value lies in
/// `[low_val, hi_val]` (every `skip + 1`-th frame, slice, row and column), with `border_only` only
/// those with fewer than six face neighbours in the range (`MRIneighborsInRange`): their indices,
/// frames and values (`vsrc`) in the order frame, slice, row, column; the list's `mri` the volume.
/// A list is allocated (arrays of the count, at least 1) when none is given or the given one's
/// `max_vox` is below the count (the given one's arrays replaced); `max_vox` is never set
/// (FS-892). `allocating <n> voxel indices...` on standard output under `DIAG_SHOW` and
/// `DIAG_VERBOSE`, and `voxel (<x>, <y>, <z>) = <v> added to voxlist at <i>` for `Gx Gy Gz`.
/// FS-888, FS-889, FS-890.
#[allow(clippy::too_many_arguments)]
pub fn vlst_create<'a>(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &'a Mri,
    low_val: f32,
    hi_val: f32,
    vl: Option<VoxelList<'a>>,
    skip: i32,
    border_only: bool,
) -> Result<VoxelList<'a>, Fatal> {
    if skip < 0 {
        g.notices.add(
            &FS_889,
            format!("VLSTcreate with skip {skip} (FreeSurfer never ends or reads before frame 0)"),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!("VLSTcreate: skip {skip} is negative (FreeSurfer never ends here)\n"),
        )?;
    }
    let step = (skip + 1) as usize;
    let lo = f64::from(low_val);
    let hi = f64::from(hi_val);
    let in_range = |v: f64| v >= lo && v <= hi;
    let (w, h, d, nf) = (mri.vg.width, mri.vg.height, mri.vg.depth, mri.nframes);
    let (mut fs_count, mut border_count) = (0i32, 0i32);
    for f in (0..nf.max(0)).step_by(step) {
        for z in (0..d.max(0)).step_by(step) {
            for y in (0..h.max(0)).step_by(step) {
                for x in (0..w.max(0)).step_by(step) {
                    let val = f64::from(mri_get_vox_val(g, err, mri, x, y, z, f, 0)?);
                    let nb = if border_only {
                        neighbors_in_range(g, err, mri, x, y, z, f, low_val, hi_val)?
                    } else {
                        6
                    };
                    if !(border_only && nb < 6) && in_range(val) {
                        fs_count += 1;
                    }
                    if in_range(val) && (!border_only || nb < 6) {
                        border_count += 1;
                    }
                }
            }
        }
    }
    let nvox = if border_only && fs_count != border_count {
        g.notices.add(
            &FS_888,
            format!(
                "VLSTcreate with border_only: {border_count} border voxels, FreeSurfer's count {fs_count}"
            ),
        );
        if g.notices.reproduce() {
            fs_count
        } else {
            border_count
        }
    } else {
        border_count
    };
    if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
        let _ = writeln!(out, "allocating {nvox} voxel indices...");
    }
    let given = vl.is_some();
    let mut vl = vl.unwrap_or_default();
    if !given || vl.max_vox < nvox {
        vl.allocate(nvox.max(1) as usize);
    }
    vl.nvox = nvox;
    let mut n = 0i64;
    for f in (0..nf.max(0)).step_by(step) {
        for z in (0..d.max(0)).step_by(step) {
            for y in (0..h.max(0)).step_by(step) {
                for x in (0..w.max(0)).step_by(step) {
                    let val = f64::from(mri_get_vox_val(g, err, mri, x, y, z, f, 0)?);
                    if !in_range(val) {
                        continue;
                    }
                    if border_only
                        && neighbors_in_range(g, err, mri, x, y, z, f, low_val, hi_val)? >= 6
                    {
                        continue;
                    }
                    let i = n;
                    n += 1;
                    if x == g.gx && y == g.gy && z == g.gz {
                        let _ = writeln!(
                            out,
                            "voxel ({x}, {y}, {z}) = {} added to voxlist at {i}",
                            fmt_f(val, 2, 1)
                        );
                    }
                    if !within(i, &[vl.xi.len(), vl.yi.len(), vl.zi.len(), vl.fi.len(), vl.vsrc.len()]) {
                        past(g, err, "VLSTcreate", format!("voxel {i} of an array of {}", vl.xi.len()))?;
                    }
                    let i = i as usize;
                    vl.xi[i] = x;
                    vl.yi[i] = y;
                    vl.zi[i] = z;
                    vl.fi[i] = f;
                    vl.vsrc[i] = val as f32;
                }
            }
        }
    }
    vl.mri = Some(mri);
    Ok(vl)
}

/// `SORT_VOXEL` (and the frame index, which FreeSurfer's lacks: FS-891).
#[derive(Clone, Copy)]
struct SortVoxel {
    vsrc: f32,
    vdst: f32,
    xd: f32,
    yd: f32,
    zd: f32,
    xi: i32,
    yi: i32,
    zi: i32,
    fi: i32,
}

/// `compare_sort`: by `vsrc` descending, then `xi`, `yi`, `zi` descending (`vsrc` neither below nor
/// above, a NaN included, passes to the indices).
fn compare_sort(a: &SortVoxel, b: &SortVoxel) -> i32 {
    if a.vsrc < b.vsrc {
        1
    } else if a.vsrc > b.vsrc {
        -1
    } else if a.xi < b.xi {
        1
    } else if a.xi > b.xi {
        -1
    } else if a.yi < b.yi {
        1
    } else if a.yi > b.yi {
        -1
    } else if a.zi < b.zi {
        1
    } else if a.zi > b.zi {
        -1
    } else {
        0
    }
}

/// Where `VLSTsort` writes.
pub enum SortInto<'b, 'a> {
    /// `VLSTsort(src, NULL)`: a copy of `src` (`VLSTcopy(src, NULL, 0, src->nvox)`), sorted.
    New(&'b VoxelList<'a>),
    /// `VLSTsort(src, dst)`: `dst`'s own voxels sorted (`src` only sizes the sort table).
    Given(&'b VoxelList<'a>, &'b mut VoxelList<'a>),
    /// `VLSTsort(vl, vl)`.
    Same(&'b mut VoxelList<'a>),
}

/// `VLSTsort(vl_src, vl_dst)`: the destination's `nvox` voxels sorted by [`compare_sort`] (the C
/// library's `qsort`); every field moves but the frame index (FS-891). The sort table holds the
/// source's `nvox` voxels: a negative count ends the program (`MRIorderIndices: could not
/// allocate sort table`), a destination of more voxels FS-890. Returns the new list for
/// [`SortInto::New`].
pub fn vlst_sort<'a>(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    into: SortInto<'_, 'a>,
) -> Result<Option<VoxelList<'a>>, Fatal> {
    let mut fresh = None;
    let (src_nvox, dst): (i32, &mut VoxelList<'a>) = match into {
        SortInto::New(s) => {
            let c = vlst_copy(g, out, err, s, None, 0, s.nvox)?.unwrap_new();
            (s.nvox, fresh.insert(c))
        }
        SortInto::Given(s, d) => (s.nvox, d),
        SortInto::Same(d) => (d.nvox, d),
    };
    if src_nvox < 0 {
        g.errno = ENOMEM;
        log::error_exit(
            g,
            err,
            ERROR_NOMEMORY,
            "MRIorderIndices: could not allocate sort table",
        )?;
    }
    let n = dst.nvox.max(0);
    if n > src_nvox {
        past(
            g,
            err,
            "VLSTsort",
            format!("{n} voxels sorted in a table of {src_nvox}"),
        )?;
    }
    if !within(i64::from(n) - 1, &dst.lens()) && n > 0 {
        past(g, err, "VLSTsort", format!("{n} voxels of arrays of {}", dst.xi.len()))?;
    }
    let n = n as usize;
    let mut v: Vec<SortVoxel> = (0..n)
        .map(|i| SortVoxel {
            xi: dst.xi[i],
            yi: dst.yi[i],
            zi: dst.zi[i],
            vsrc: dst.vsrc[i],
            vdst: dst.vdst[i],
            xd: dst.xd[i],
            yd: dst.yd[i],
            zd: dst.zd[i],
            fi: dst.fi[i],
        })
        .collect();
    qsort(&mut v, compare_sort);
    let moved = v.iter().enumerate().any(|(i, s)| s.fi != dst.fi[i]);
    if moved {
        g.notices.add(
            &FS_891,
            "VLSTsort of voxels of several frames: the frame indices stay in place",
        );
    }
    let keep_fi = g.notices.reproduce();
    for (i, s) in v.iter().enumerate() {
        dst.xi[i] = s.xi;
        dst.yi[i] = s.yi;
        dst.zi[i] = s.zi;
        dst.vsrc[i] = s.vsrc;
        dst.vdst[i] = s.vdst;
        dst.xd[i] = s.xd;
        dst.yd[i] = s.yd;
        dst.zd[i] = s.zd;
        if !keep_fi {
            dst.fi[i] = s.fi;
        }
    }
    Ok(fresh)
}

/// `VLSTfree(&vl)`: `NO_ERROR`, the list `None`; `ERROR_BADPARM` (no message) for none.
pub fn vlst_free(vl: &mut Option<VoxelList<'_>>) -> i32 {
    if vl.take().is_none() {
        ERROR_BADPARM
    } else {
        NO_ERROR
    }
}

/// `VLSTcomputeStats(vl)`: the mean and standard deviation (`sqrt(E[v²] - E[v]²)`, in `double`)
/// of the list's voxels in frame 0 of its volume; both 0 without a volume or voxels. FS-890.
pub fn vlst_compute_stats(
    g: &mut Globals,
    err: &mut dyn Write,
    vl: &mut VoxelList<'_>,
) -> Result<(), Fatal> {
    let Some(mri) = vl.mri.filter(|_| vl.nvox > 0) else {
        vl.mean = 0.0;
        vl.std = 0.0;
        return Ok(());
    };
    let n = vl.nvox;
    if !within(i64::from(n) - 1, &[vl.xi.len(), vl.yi.len(), vl.zi.len()]) {
        past(g, err, "VLSTcomputeStats", format!("{n} voxels of arrays of {}", vl.xi.len()))?;
    }
    let (mut mean, mut std) = (0.0f64, 0.0f64);
    for i in 0..n as usize {
        let val = f64::from(mri_get_vox_val(g, err, mri, vl.xi[i], vl.yi[i], vl.zi[i], 0, 0)?);
        mean = sse::add64(mean, val);
        std = sse::add64(std, sse::mul64(val, val));
    }
    mean = sse::div64(mean, f64::from(n));
    std = sse::div64(std, f64::from(n));
    std = sse::sqrt64(sse::sub64(std, sse::mul64(mean, mean)));
    vl.mean = mean;
    vl.std = std;
    Ok(())
}

/// `VLSTalloc(nvox)`: a list of `nvox` voxels (every array `nvox` zeros), `nvox` and `max_vox`
/// both `nvox`; a negative count (or one beyond memory) ends the program (`VLSTalloc(<n>): could
/// not allocate VL struct`).
pub fn vlst_alloc<'a>(
    g: &mut Globals,
    err: &mut dyn Write,
    nvox: i32,
) -> Result<VoxelList<'a>, Fatal> {
    let mut vl = VoxelList::default();
    let mut probe: Vec<i32> = Vec::new();
    if nvox < 0 || probe.try_reserve_exact(nvox as usize).is_err() {
        g.errno = ENOMEM;
        log::error_exit(
            g,
            err,
            ERROR_NOMEMORY,
            &format!("VLSTalloc({nvox}): could not allocate VL struct"),
        )?;
        return Ok(vl);
    }
    vl.allocate(nvox as usize);
    vl.nvox = nvox;
    vl.max_vox = nvox;
    Ok(vl)
}

/// The slope arrays `VLSTcopy` and `VLSTcopyInto` give a copy of a list with slopes: `num` zeros
/// each (`calloc`; none for a negative `num`), for each array the source has; the program ends
/// (`VLSTcopy: could not allocate <num>-len slope arrays`) when the source has slopes (`mx`) and
/// the copy lacks one of the four (FS-893).
fn copy_slopes(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &VoxelList<'_>,
    dst: &mut VoxelList<'_>,
    num: i32,
) -> Result<(), Fatal> {
    let fresh = || (num >= 0).then(|| vec![0.0f32; num as usize]);
    let any = src.mx.is_some() || src.my.is_some() || src.mz.is_some() || src.t.is_some();
    if num < 0 && any {
        // calloc of a negative count fails
        g.errno = ENOMEM;
    }
    if src.mx.is_some() {
        dst.mx = fresh();
    }
    if src.my.is_some() {
        dst.my = fresh();
    }
    if src.mz.is_some() {
        dst.mz = fresh();
    }
    if src.t.is_some() {
        dst.t = fresh();
    }
    if src.mx.is_some()
        && (dst.t.is_none() || dst.mx.is_none() || dst.my.is_none() || dst.mz.is_none())
    {
        log::error_exit(
            g,
            err,
            ERROR_NOMEMORY,
            &format!("VLSTcopy: could not allocate {num}-len slope arrays"),
        )?;
    }
    Ok(())
}

/// One voxel's nine fields from `src[i]` into `dst[j]`, FS-890 past either's arrays.
fn copy_voxel(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    src: &VoxelList<'_>,
    i: i64,
    dst: &mut VoxelList<'_>,
    j: i64,
) -> Result<(), Fatal> {
    if !within(i, &src.lens()) {
        past(g, err, what, format!("source voxel {i} of arrays of {}", src.xi.len()))?;
    }
    if !within(j, &dst.lens()) {
        past(g, err, what, format!("destination voxel {j} of arrays of {}", dst.xi.len()))?;
    }
    let (i, j) = (i as usize, j as usize);
    dst.xi[j] = src.xi[i];
    dst.yi[j] = src.yi[i];
    dst.zi[j] = src.zi[i];
    dst.fi[j] = src.fi[i];
    dst.xd[j] = src.xd[i];
    dst.yd[j] = src.yd[i];
    dst.zd[j] = src.zd[i];
    dst.vsrc[j] = src.vsrc[i];
    dst.vdst[j] = src.vdst[i];
    Ok(())
}

/// `VLSTcopy(vl_src, vl_dst, start_src_index, num)`: `num` voxels from `start_src_index` on into
/// the destination from 0 (a new [`vlst_alloc`] of `num` when none is given); the destination's
/// type the source's, its slope arrays [`copy_slopes`]'s, its `nvox` raised to `num`. A
/// destination whose `max_vox` is below `num` ends the program (`VLSTcopy: destination voxlist
/// not big enough (<max_vox>, but must be <start + num>)`). FS-890, FS-893.
pub fn vlst_copy<'a>(
    g: &mut Globals,
    _out: &mut dyn Write,
    err: &mut dyn Write,
    src: &VoxelList<'a>,
    dst: Option<&mut VoxelList<'a>>,
    start_src_index: i32,
    num: i32,
) -> Result<Ret<VoxelList<'a>>, Fatal> {
    let mut fresh = None;
    let given = dst.is_some();
    let d: &mut VoxelList<'a> = match dst {
        Some(d) => d,
        None => fresh.insert(vlst_alloc(g, err, num)?),
    };
    if d.max_vox < num {
        log::error_exit(
            g,
            err,
            ERROR_NOMEMORY,
            &format!(
                "VLSTcopy: destination voxlist not big enough ({}, but must be {})",
                d.max_vox,
                start_src_index.wrapping_add(num)
            ),
        )?;
    }
    d.vtype = src.vtype;
    copy_slopes(g, err, src, d, num)?;
    for i in 0..num.max(0) {
        copy_voxel(
            g,
            err,
            "VLSTcopy",
            src,
            i64::from(i) + i64::from(start_src_index),
            d,
            i64::from(i),
        )?;
    }
    if d.nvox < num {
        d.nvox = num;
    }
    Ok(match fresh {
        Some(f) => Ret::New(f),
        None if given => Ret::Given,
        None => Ret::Null,
    })
}

/// `VLSTcopyInto(vl_src, vl_dst, start_dst_index, num)`: the source's first `num` voxels into the
/// destination from `start_dst_index` on (a new [`vlst_alloc`] of `start + num` when none is
/// given); as [`vlst_copy`] otherwise (its `max_vox` must reach `start + num`, its `nvox` is raised
/// to it). FS-890, FS-893.
pub fn vlst_copy_into<'a>(
    g: &mut Globals,
    _out: &mut dyn Write,
    err: &mut dyn Write,
    src: &VoxelList<'a>,
    dst: Option<&mut VoxelList<'a>>,
    start_dst_index: i32,
    num: i32,
) -> Result<Ret<VoxelList<'a>>, Fatal> {
    let end = start_dst_index.wrapping_add(num);
    let mut fresh = None;
    let given = dst.is_some();
    let d: &mut VoxelList<'a> = match dst {
        Some(d) => d,
        None => fresh.insert(vlst_alloc(g, err, end)?),
    };
    if d.max_vox < end {
        log::error_exit(
            g,
            err,
            ERROR_NOMEMORY,
            &format!(
                "VLSTcopy: destination voxlist not big enough ({}, but must be {end})",
                d.max_vox
            ),
        )?;
    }
    d.vtype = src.vtype;
    copy_slopes(g, err, src, d, num)?;
    for i in 0..num.max(0) {
        copy_voxel(
            g,
            err,
            "VLSTcopyInto",
            src,
            i64::from(i),
            d,
            i64::from(start_dst_index) + i64::from(i),
        )?;
    }
    if d.nvox < end {
        d.nvox = end;
    }
    Ok(match fresh {
        Some(f) => Ret::New(f),
        None if given => Ret::Given,
        None => Ret::Null,
    })
}

/// `VLSTadd(vl, x, y, z, xd, yd, zd)`: the voxel's indices and coordinates at `nvox` (its values
/// and frame untouched), `nvox` raised by one: `NO_ERROR`; a full list (`nvox >= max_vox`):
/// `VLSTadd: too many voxels (<max_vox>)` as an error and `ERROR_NOMEMORY`. FS-890.
#[allow(clippy::too_many_arguments)]
pub fn vlst_add(
    g: &mut Globals,
    err: &mut dyn Write,
    vl: &mut VoxelList<'_>,
    x: i32,
    y: i32,
    z: i32,
    xd: f32,
    yd: f32,
    zd: f32,
) -> Result<i32, Fatal> {
    if vl.nvox >= vl.max_vox {
        log::error_printf(
            g,
            err,
            ERROR_NOMEMORY,
            &format!("VLSTadd: too many voxels ({})", vl.max_vox),
        );
        return Ok(ERROR_NOMEMORY);
    }
    let i = i64::from(vl.nvox);
    if !within(i, &[vl.xi.len(), vl.yi.len(), vl.zi.len(), vl.xd.len(), vl.yd.len(), vl.zd.len()]) {
        past(g, err, "VLSTadd", format!("voxel {i} of arrays of {}", vl.xi.len()))?;
    }
    let i = i as usize;
    vl.xi[i] = x;
    vl.yi[i] = y;
    vl.zi[i] = z;
    vl.xd[i] = xd;
    vl.yd[i] = yd;
    vl.zd[i] = zd;
    vl.nvox += 1;
    Ok(NO_ERROR)
}

/// `VLSTinList(vl, x, y, z)`: 1 when one of the first `nvox` voxels has these indices, else 0.
/// FS-890.
pub fn vlst_in_list(
    g: &mut Globals,
    err: &mut dyn Write,
    vl: &VoxelList<'_>,
    x: i32,
    y: i32,
    z: i32,
) -> Result<i32, Fatal> {
    for n in 0..vl.nvox.max(0) as usize {
        if n >= vl.xi.len() {
            past(g, err, "VLSTinList", format!("voxel {n} of arrays of {}", vl.xi.len()))?;
        }
        if vl.xi[n] != x {
            continue;
        }
        if n >= vl.yi.len() {
            past(g, err, "VLSTinList", format!("voxel {n} of arrays of {}", vl.yi.len()))?;
        }
        if vl.yi[n] != y {
            continue;
        }
        if n >= vl.zi.len() {
            past(g, err, "VLSTinList", format!("voxel {n} of arrays of {}", vl.zi.len()))?;
        }
        if vl.zi[n] == z {
            return Ok(1);
        }
    }
    Ok(0)
}

/// `VLSTaddUnique(vl, x, y, z, xd, yd, zd)`: 0 when the voxel is in the list, else [`vlst_add`]
/// (its status dropped) and 1.
#[allow(clippy::too_many_arguments)]
pub fn vlst_add_unique(
    g: &mut Globals,
    err: &mut dyn Write,
    vl: &mut VoxelList<'_>,
    x: i32,
    y: i32,
    z: i32,
    xd: f32,
    yd: f32,
    zd: f32,
) -> Result<i32, Fatal> {
    if vlst_in_list(g, err, vl, x, y, z)? != 0 {
        return Ok(0);
    }
    vlst_add(g, err, vl, x, y, z, xd, yd, zd)?;
    Ok(1)
}

/// `VLSTaddWithValue(vl, x, y, z, xd, yd, zd, vsrc, vdst)`: [`vlst_add`], then the new voxel's
/// values: `NO_ERROR`; when it fails, `Gerror` (the error it set).
#[allow(clippy::too_many_arguments)]
pub fn vlst_add_with_value(
    g: &mut Globals,
    err: &mut dyn Write,
    vl: &mut VoxelList<'_>,
    x: i32,
    y: i32,
    z: i32,
    xd: f32,
    yd: f32,
    zd: f32,
    vsrc: f32,
    vdst: f32,
) -> Result<i32, Fatal> {
    if vlst_add(g, err, vl, x, y, z, xd, yd, zd)? != NO_ERROR {
        return Ok(g.gerror);
    }
    let ind = i64::from(vl.nvox) - 1;
    if !within(ind, &[vl.vsrc.len(), vl.vdst.len()]) {
        past(g, err, "VLSTaddWithValue", format!("voxel {ind} of arrays of {}", vl.vsrc.len()))?;
    }
    vl.vdst[ind as usize] = vdst;
    vl.vsrc[ind as usize] = vsrc;
    Ok(NO_ERROR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;
    use larmorx_freesurfer_mri::mri::{MRI_UCHAR, mri_alloc_sequence};
    use larmorx_freesurfer_mri::voxel::mri_set_vox_val;

    /// A 3 x 3 x 3 cube of ones in a 5 x 5 x 5 volume: 27 voxels, 26 on its border.
    fn cube(g: &mut Globals) -> Mri {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut m = mri_alloc_sequence(g, &mut out, &mut err, 5, 5, 5, MRI_UCHAR, 1).unwrap();
        for z in 1..4 {
            for y in 1..4 {
                for x in 1..4 {
                    mri_set_vox_val(g, &mut err, &mut m, x, y, z, 0, 1.0, 0).unwrap();
                }
            }
        }
        m
    }

    #[test]
    fn create_counts_the_border_by_default() {
        for (mode, n) in [(UpstreamDefects::Fix, 26), (UpstreamDefects::Reproduce, 1)] {
            let mut g = Globals::new(mode);
            let m = cube(&mut g);
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let all = vlst_create(&mut g, &mut out, &mut err, &m, 1.0, 1.0, None, 0, false).unwrap();
            assert_eq!(all.nvox, 27);
            assert_eq!(all.max_vox, 0);
            let r = vlst_create(&mut g, &mut out, &mut err, &m, 1.0, 1.0, None, 0, true);
            if mode.reproduce() {
                // FreeSurfer's count is the one interior voxel: the border writes past the arrays
                assert!(r.is_err());
            } else {
                assert_eq!(r.unwrap().nvox, n);
            }
            assert!(g.notices.as_slice().iter().any(|x| x.defect.id == "FS-888"));
        }
    }

    #[test]
    fn sort_moves_frames_by_default() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let mut g = Globals::new(mode);
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let mut vl = vlst_alloc(&mut g, &mut err, 3).unwrap();
            vl.vsrc = vec![1.0, 3.0, 2.0];
            vl.xi = vec![0, 1, 2];
            vl.fi = vec![0, 1, 2];
            vlst_sort(&mut g, &mut out, &mut err, SortInto::Same(&mut vl)).unwrap();
            assert_eq!(vl.vsrc, [3.0, 2.0, 1.0]);
            assert_eq!(vl.xi, [1, 2, 0]);
            let fi = if mode.reproduce() { [0, 1, 2] } else { [1, 2, 0] };
            assert_eq!(vl.fi, fi);
        }
    }

    #[test]
    fn add_until_full_and_free() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut err = Vec::new();
        let mut vl = vlst_alloc(&mut g, &mut err, 2).unwrap();
        vl.nvox = 0;
        assert_eq!(vlst_add_unique(&mut g, &mut err, &mut vl, 1, 2, 3, 0.0, 0.0, 0.0).unwrap(), 1);
        assert_eq!(vlst_add_unique(&mut g, &mut err, &mut vl, 1, 2, 3, 0.0, 0.0, 0.0).unwrap(), 0);
        assert_eq!(vlst_add(&mut g, &mut err, &mut vl, 4, 5, 6, 0.0, 0.0, 0.0).unwrap(), NO_ERROR);
        assert_eq!(
            vlst_add(&mut g, &mut err, &mut vl, 7, 8, 9, 0.0, 0.0, 0.0).unwrap(),
            ERROR_NOMEMORY
        );
        assert_eq!(vlst_in_list(&mut g, &mut err, &vl, 4, 5, 6).unwrap(), 1);
        let mut p = Some(vl);
        assert_eq!(vlst_free(&mut p), NO_ERROR);
        assert_eq!(vlst_free(&mut p), ERROR_BADPARM);
    }
}
