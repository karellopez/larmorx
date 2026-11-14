// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrisegment.cpp`'s connected segments (queue item 3.1-43): `MRImaxSegmentArea` (67),
//! `MRImaxsegment` (83), `MRIsegment` (313), `MRIsegmentFree` (477), `MRIsegmentAlloc` (499),
//! `mriSegmentMerge` (529), `mriSegmentReallocateVoxels` (573), `mriSegmentReallocateSegments`
//! (638), `MRIsegmentToImage` (672), `MRIsetSegmentValue` (703), `mriSegmentNew` (727),
//! `MRIcompactSegments` (757), `MRIsegmentDilate` (829), `MRIremoveSmallSegments` (883),
//! `MRIeraseSmallSegments` (900), `mriComputeSegmentStatistics` (924),
//! `MRIsegmentDilateThreshold` (958), `MRIsegmentMax` (1014), `MRIsegmentClearIgnoreFlags`
//! (1034), `MRIfindMaxSegmentNumber` (1046), `MRIsegmentFill` (1061); and `utils/rfa.cpp`'s
//! `csf_labels` (680).
//!
//! A segment's voxel list is a vector whose length is `nvoxels`; FreeSurfer's spare capacity
//! (`max_voxels`, grown by 10 % with `realloc`) only decides memory, and its allocation failures
//! (never on a machine that can run its programs) have no counterpart. The segment table keeps
//! FreeSurfer's length (`max_segments`) and its slots' fields as FreeSurfer moves them: compaction
//! moves a segment's area and voxels into an empty slot and leaves that slot's other fields (label,
//! flags, centroid, bounds) as they were. The label map of the scan is FreeSurfer's 16-bit volume
//! (FS-577). FS-577, FS-578, FS-579, FS-581, FS-582.

use std::io::Write;

use larmorx_core::defects::{FS_577, FS_578, FS_579, FS_581, FS_582};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_NOMEMORY, Fatal, NO_ERROR};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_mri::mri::{MAX_INDEX, MRI_SHORT, Mri, mri_alloc};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::mri_clone;
use larmorx_freesurfer_volume::cma::{
    CSF, Fourth_Ventricle, Left_Inf_Lat_Vent, Left_Lateral_Ventricle, Right_Inf_Lat_Vent,
    Right_Lateral_Ventricle, Third_Ventricle, Unknown,
};

/// `MAX_SEGMENTS`: the segment table's first length.
pub const MAX_SEGMENTS: i32 = 8000;
/// `VOX_INCREASE`: the growth factor of the voxel lists and the segment table.
const VOX_INCREASE: f64 = 1.1;
/// `ENOMEM`: what a failed `calloc` leaves in `errno` (the error message prints it).
const ENOMEM: i32 = 12;
/// `NBR_VOX`: the neighbour slots of a voxel (3 x 3 x 3).
const NBR_VOX: usize = 27;

/// `csf_labels` (`utils/rfa.cpp`): the labels `MRIcountCSFInNbhd` counts as CSF.
pub const CSF_LABELS: [i32; 8] = [
    CSF,
    Unknown,
    Left_Lateral_Ventricle,
    Right_Lateral_Ventricle,
    Left_Inf_Lat_Vent,
    Right_Inf_Lat_Vent,
    Third_Ventricle,
    Fourth_Ventricle,
];

/// `MRI_SEGMENT_VOXEL` (`MSV`): a voxel's column, row and slice as 16-bit integers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Msv {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

/// `MRI_SEGMENT`: the area (the voxels' volumes summed in `float`), the voxels, the label and the
/// scan's flag (`found`), the centroid and the bounds (`mriComputeSegmentStatistics`), the
/// `ignore` flag of `MRIsegmentMax`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MriSegment {
    pub area: f32,
    pub voxels: Vec<Msv>,
    pub label: i32,
    pub found: i32,
    pub cx: f32,
    pub cy: f32,
    pub cz: f32,
    pub x0: i16,
    pub x1: i16,
    pub y0: i16,
    pub y1: i16,
    pub z0: i16,
    pub z1: i16,
    pub ignore: i16,
}

impl MriSegment {
    /// `nvoxels`.
    pub fn nvoxels(&self) -> i32 {
        self.voxels.len() as i32
    }
}

/// `MRI_SEGMENTATION`: the segment table (its length is `max_segments`) and the number of
/// segments (`nsegments`). FreeSurfer's `mri` (the volume segmented) is the caller's: the
/// functions that read it take it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MriSegmentation {
    pub segments: Vec<MriSegment>,
    pub nsegments: i32,
}

impl MriSegmentation {
    /// `max_segments`.
    pub fn max_segments(&self) -> i32 {
        self.segments.len() as i32
    }
}

/// `MRIsegmentAlloc(max_segments, max_voxels)`: a table of `max_segments` empty segments.
/// FreeSurfer's failures: a negative `max_segments` (`MRIsegmentAlloc: could not alloc
/// mrisegments`), a negative `max_voxels` with segments (`MRIsegmentAlloc: could not alloc 0th
/// voxel list`), both ending the program with `ERROR_NOMEMORY`.
pub fn mri_segment_alloc(
    g: &mut Globals,
    err: &mut dyn Write,
    max_segments: i32,
    max_voxels: i32,
) -> Result<MriSegmentation, Fatal> {
    if max_segments < 0 {
        g.errno = ENOMEM;
        log::error_exit(
            g,
            err,
            ERROR_NOMEMORY,
            "MRIsegmentAlloc: could not alloc mrisegments",
        )?;
    }
    if max_voxels < 0 && max_segments > 0 {
        g.errno = ENOMEM;
        log::error_exit(
            g,
            err,
            ERROR_NOMEMORY,
            "MRIsegmentAlloc: could not alloc 0th voxel list",
        )?;
    }
    Ok(MriSegmentation {
        segments: vec![MriSegment::default(); max_segments.max(0) as usize],
        nsegments: 0,
    })
}

/// `MRIsegmentFree(&mriseg)`: the segmentation dropped (`NO_ERROR`).
pub fn mri_segment_free(mriseg: &mut Option<MriSegmentation>) -> i32 {
    *mriseg = None;
    NO_ERROR
}

/// The scan's label map: FreeSurfer's `MRI_SHORT` volume (labels stored as 16-bit integers,
/// FS-577), widened to 32 bits by default when a label does not fit.
struct LabelMap {
    w: usize,
    h: usize,
    narrow: Vec<i16>,
    wide: Option<Vec<i32>>,
}

impl LabelMap {
    fn new(w: i32, h: i32, d: i32) -> LabelMap {
        let n = (w.max(0) as usize) * (h.max(0) as usize) * (d.max(0) as usize);
        LabelMap {
            w: w.max(0) as usize,
            h: h.max(0) as usize,
            narrow: vec![-1; n],
            wide: None,
        }
    }
    fn idx(&self, x: i32, y: i32, z: i32) -> usize {
        (z as usize * self.h + y as usize) * self.w + x as usize
    }
    fn get(&self, x: i32, y: i32, z: i32) -> i32 {
        let i = self.idx(x, y, z);
        match &self.wide {
            Some(v) => v[i],
            None => i32::from(self.narrow[i]),
        }
    }
    fn set(&mut self, g: &mut Globals, x: i32, y: i32, z: i32, label: i32) {
        let i = self.idx(x, y, z);
        if let Some(v) = &mut self.wide {
            v[i] = label;
            return;
        }
        if i16::try_from(label).is_err() {
            g.notices.add(
                &FS_577,
                format!("segment number {label} stored in the 16-bit label map of the scan"),
            );
            if !g.notices.reproduce() {
                let mut v: Vec<i32> = self.narrow.iter().map(|&l| i32::from(l)).collect();
                self.narrow = Vec::new();
                v[i] = label;
                self.wide = Some(v);
                return;
            }
        }
        self.narrow[i] = label as i16;
    }
    fn len(&self) -> usize {
        match &self.wide {
            Some(v) => v.len(),
            None => self.narrow.len(),
        }
    }
    fn get_i(&self, i: usize) -> i32 {
        match &self.wide {
            Some(v) => v[i],
            None => i32::from(self.narrow[i]),
        }
    }
    fn set_i(&mut self, i: usize, label: i32) {
        match &mut self.wide {
            Some(v) => v[i] = label,
            None => self.narrow[i] = label as i16,
        }
    }
}

/// `MRImaxSegmentArea(mriseg)`: the largest `nvoxels` of the whole table (-1 for none).
pub fn mri_max_segment_area(mriseg: &MriSegmentation) -> i32 {
    let mut maxarea = -1;
    for s in &mriseg.segments {
        if s.nvoxels() > maxarea {
            maxarea = s.nvoxels();
        }
    }
    maxarea
}

/// `mriSegmentReallocateSegments(mriseg, max_segments)`: a table of `max_segments` slots; the
/// first `nsegments` keep their area and voxels (their other fields are cleared), the others are
/// empty. Every segment of the scan is among the first `nsegments` when it is called.
fn reallocate_segments(mriseg: &mut MriSegmentation, max_segments: i32) {
    let mut old = std::mem::take(&mut mriseg.segments);
    let mut segs = vec![MriSegment::default(); max_segments.max(0) as usize];
    for (s, slot) in segs
        .iter_mut()
        .enumerate()
        .take(mriseg.nsegments.max(0) as usize)
    {
        slot.area = old[s].area;
        slot.voxels = std::mem::take(&mut old[s].voxels);
    }
    mriseg.segments = segs;
}

/// `mriSegmentNew(mriseg)`: the table grown by 10 % when it is full (to 4,000 slots, or by 1,024,
/// when 10 % adds none), `nsegments` incremented, then the first empty slot of the first
/// `nsegments` (searched from `low`, below which the scan knows every slot holds voxels; the slot
/// returned is filled next).
fn segment_new(
    g: &mut Globals,
    err: &mut dyn Write,
    mriseg: &mut MriSegmentation,
    low: &mut usize,
) -> Result<usize, Fatal> {
    if mriseg.nsegments >= mriseg.max_segments() {
        let max = mriseg.max_segments();
        let mut newlen = (f64::from(max) * VOX_INCREASE) as i32;
        if newlen == max {
            newlen = if max < MAX_SEGMENTS / 2 {
                MAX_SEGMENTS / 2
            } else {
                max + 1024
            };
        }
        reallocate_segments(mriseg, newlen);
    }
    mriseg.nsegments += 1;
    let n = mriseg.nsegments as usize;
    // the scan's hint: every slot below `low` holds voxels (the search FreeSurfer starts at 0)
    let s = (*low..n)
        .find(|&s| mriseg.segments[s].voxels.is_empty())
        .unwrap_or(n);
    *low = s + 1;
    if s >= mriseg.segments.len() {
        // never: the scan keeps nsegments equal to its live segments
        log::error_exit(
            g,
            err,
            ERROR_NOMEMORY,
            "mriSegmentNew: the segment table is full",
        )?;
    }
    Ok(s)
}

/// `mriSegmentMerge(mriseg, s0, s1, mri_labeled)`: segment `s1`'s voxels appended to `s0`'s (and
/// labelled `s0`), its area added; `s1` emptied (area 0), `nsegments` decremented.
fn segment_merge(
    g: &mut Globals,
    mriseg: &mut MriSegmentation,
    s0: usize,
    s1: usize,
    labels: &mut LabelMap,
) {
    let moved = std::mem::take(&mut mriseg.segments[s1].voxels);
    let area1 = mriseg.segments[s1].area;
    for v in &moved {
        labels.set(g, i32::from(v.x), i32::from(v.y), i32::from(v.z), s0 as i32);
    }
    let m0 = &mut mriseg.segments[s0];
    m0.voxels.extend_from_slice(&moved);
    m0.area += area1;
    mriseg.nsegments -= 1;
    mriseg.segments[s1].area = 0.0;
}

/// Moves each segment with voxels into the first empty slot before it, in order; true when one
/// moved. The slot receives the area and voxels only.
fn compact_moves(mriseg: &mut MriSegmentation) -> bool {
    let n = mriseg.segments.len();
    let mut moved = false;
    // FreeSurfer searches from s + 1 each time; the slots between are empty, so the search resumes
    let mut j = 0;
    for s in 0..n {
        if mriseg.segments[s].voxels.is_empty() {
            j = j.max(s + 1);
            while j < n && mriseg.segments[j].voxels.is_empty() {
                j += 1;
            }
            if j < n {
                let vox = std::mem::take(&mut mriseg.segments[j].voxels);
                let area = mriseg.segments[j].area;
                mriseg.segments[j].area = 0.0;
                mriseg.segments[s].voxels = vox;
                mriseg.segments[s].area = area;
                moved = true;
                j += 1;
            }
        }
    }
    moved
}

/// `MRIcompactSegments(mriseg)` with whether a segment moved: nothing when `nsegments` is 0;
/// otherwise the segments with voxels moved to the front in order (a slot receives a segment's
/// area and voxels and keeps its other fields), the table cut at the first empty slot,
/// `nsegments` its length.
fn compact(mriseg: &mut MriSegmentation) -> bool {
    if mriseg.nsegments == 0 {
        return false;
    }
    let moved = compact_moves(mriseg);
    let s = mriseg
        .segments
        .iter()
        .position(|m| m.voxels.is_empty())
        .unwrap_or(mriseg.segments.len());
    mriseg.segments.truncate(s);
    mriseg.nsegments = s as i32;
    moved
}

/// `MRIcompactSegments(mriseg)`: see [`compact`] (`NO_ERROR`).
pub fn mri_compact_segments(mriseg: &mut MriSegmentation) -> i32 {
    compact(mriseg);
    NO_ERROR
}

/// `MRIremoveSmallSegments(mriseg, min_voxels)` with whether compaction moved a segment: every
/// slot of fewer than `min_voxels` voxels emptied (its area kept), then compacted.
fn remove_small(mriseg: &mut MriSegmentation, min_voxels: i32) -> bool {
    for s in &mut mriseg.segments {
        if s.nvoxels() < min_voxels {
            s.voxels = Vec::new();
        }
    }
    compact(mriseg)
}

/// `MRIremoveSmallSegments(mriseg, min_voxels)`: the segments of fewer than `min_voxels` voxels
/// emptied, then [`mri_compact_segments`] (`NO_ERROR`).
pub fn mri_remove_small_segments(mriseg: &mut MriSegmentation, min_voxels: i32) -> i32 {
    remove_small(mriseg, min_voxels);
    NO_ERROR
}

/// `mriComputeSegmentStatistics(mriseg)`: for each of the first `nsegments` segments, bounds
/// from 10000 and -10000 and the centroid (the coordinates summed in `float`, divided by
/// `(float) nvoxels`; left 0 for an empty segment).
fn compute_statistics(mriseg: &mut MriSegmentation) {
    let n = (mriseg.nsegments.max(0) as usize).min(mriseg.segments.len());
    for m in &mut mriseg.segments[..n] {
        m.x0 = 10000;
        m.y0 = 10000;
        m.z0 = 10000;
        m.x1 = -10000;
        m.y1 = -10000;
        m.z1 = -10000;
        m.cx = 0.0;
        m.cy = 0.0;
        m.cz = 0.0;
        if m.voxels.is_empty() {
            continue;
        }
        for v in &m.voxels {
            m.cx += f32::from(v.x);
            m.cy += f32::from(v.y);
            m.cz += f32::from(v.z);
            m.x0 = m.x0.min(v.x);
            m.y0 = m.y0.min(v.y);
            m.z0 = m.z0.min(v.z);
            m.x1 = m.x1.max(v.x);
            m.y1 = m.y1.max(v.y);
            m.z1 = m.z1.max(v.z);
        }
        let n = m.nvoxels() as f32;
        m.cx /= n;
        m.cy /= n;
        m.cz /= n;
    }
}

/// How a scan reads a voxel: as a `float` (`MRIsegment`) or truncated to `int` first
/// (`MRImaxsegment`, FS-17 beyond `int`).
fn in_range(g: &mut Globals, v: f32, low: f32, hi: f32, int_val: bool) -> bool {
    let v = if int_val {
        float_to_int(g, f64::from(v), "MRImaxsegment") as f32
    } else {
        v
    };
    v >= low && v <= hi
}

/// The volume's extent checked against the 16-bit voxel coordinates (FS-581).
fn check_extent(g: &mut Globals, err: &mut dyn Write, mri: &Mri, what: &str) -> Result<(), Fatal> {
    let v = &mri.vg;
    if v.width > 32768 || v.height > 32768 || v.depth > 32768 {
        g.notices.add(
            &FS_581,
            format!(
                "{what} of a {} x {} x {} volume",
                v.width, v.height, v.depth
            ),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "{what}: a volume of more than 32768 columns, rows or slices (FreeSurfer's 16-bit \
                 voxel coordinates wrap and it writes outside its label map)"
            ),
        )?;
    }
    Ok(())
}

/// `getMemoryUsed()`: the process's heap usage in kB (its error text written to the stream).
pub type HeapUsage = dyn FnMut(&mut dyn Write) -> i32;

/// The heap usage message of `MRImaxsegment` (`getMemoryUsed()`, printed when above 0).
fn print_heap(out: &mut dyn Write, mem: i32) {
    if mem > 0 {
        let _ = write!(out, "\n             heap usage = {mem} Kbytes.");
    }
}

/// The scan of `MRIsegment` and `MRImaxsegment` (`heap` given for the latter: it reads the voxel
/// values as `int`, prints the heap usage and removes small segments as the table grows).
fn scan(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    low_val: f32,
    hi_val: f32,
    mut heap: Option<&mut HeapUsage>,
) -> Result<MriSegmentation, Fatal> {
    let maxseg = heap.is_some();
    let what = if maxseg {
        "MRImaxsegment"
    } else {
        "MRIsegment"
    };
    check_extent(g, err, mri, what)?;
    let voxel_size = mri.vg.xsize * mri.vg.ysize * mri.vg.zsize;
    let (width, height, depth) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let mut mriseg = mri_segment_alloc(g, err, MAX_SEGMENTS, 5)?;
    let mut totcount = 0i32;
    if maxseg {
        for z in 0..depth {
            for y in 0..height {
                for x in 0..width {
                    let v = mri_get_vox_val(g, err, mri, x, y, z, 0, 0)?;
                    if in_range(g, v, low_val, hi_val, true) {
                        totcount += 1;
                    }
                }
            }
        }
    }
    // FreeSurfer's MRI_SHORT label volume (its allocation's failures and messages)
    drop(mri_alloc(g, out, err, width, height, depth, MRI_SHORT)?);
    let mut labels = LabelMap::new(width, height, depth);
    let mut count = 0i32;
    let mut low = 0usize;
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                let v = mri_get_vox_val(g, err, mri, x, y, z, 0, 0)?;
                if !in_range(g, v, low_val, hi_val, maxseg) {
                    continue;
                }
                count += 1;
                let mut border = [-1i32; NBR_VOX];
                let mut nvox = 0usize;
                for zk in -1..=1i32 {
                    let zi = z + zk;
                    if zi < 0 || zi >= depth {
                        continue;
                    }
                    for yk in -1..=1i32 {
                        let yi = y + yk;
                        if yi < 0 || yi >= height {
                            continue;
                        }
                        for xk in -1..=1i32 {
                            let slot = nvox;
                            nvox += 1;
                            if xk.abs() + yk.abs() + zk.abs() > 1 {
                                continue;
                            }
                            let xi = x + xk;
                            if xi < 0 || xi >= width {
                                continue;
                            }
                            let l = labels.get(xi, yi, zi);
                            if l >= 0 {
                                border[slot] = l;
                            }
                        }
                    }
                }
                let mut nlabels = 0;
                for &l in &border {
                    if l >= 0 && mriseg.segments[l as usize].found == 0 {
                        mriseg.segments[l as usize].found = 1;
                        nlabels += 1;
                    }
                }
                for &l in &border {
                    if l >= 0 {
                        mriseg.segments[l as usize].found = 0;
                    }
                }
                let label: usize = match nlabels {
                    0 => {
                        if let Some(heap) = heap.as_deref_mut()
                            && mriseg.nsegments >= mriseg.max_segments()
                        {
                            let mem = heap(err);
                            if f64::from(mriseg.max_segments())
                                > f64::from(MAX_SEGMENTS) * VOX_INCREASE
                            {
                                print_heap(out, mem);
                                let maxarea = mri_max_segment_area(&mriseg);
                                let _ = write!(
                                    out,
                                    "\n             current max segment has {maxarea} voxels"
                                );
                                if f64::from(totcount - count) < f64::from(maxarea) * 0.99 {
                                    let _ = write!(
                                        out,
                                        "\n             removing small segments (less than 1 \
                                         percent of maxarea)."
                                    );
                                    let moved = remove_small(
                                        &mut mriseg,
                                        (f64::from(maxarea) * 0.01) as i32,
                                    );
                                    relabel_after_removal(g, &mriseg, &mut labels, moved);
                                    low = 0;
                                }
                            }
                        }
                        segment_new(g, err, &mut mriseg, &mut low)?
                    }
                    1 => border.iter().find(|&&l| l >= 0).copied().unwrap_or(0) as usize,
                    _ => {
                        let mut first: Option<usize> = None;
                        for &l in &border {
                            if l < 0 {
                                continue;
                            }
                            match first {
                                None => {
                                    first = Some(l as usize);
                                    mriseg.segments[l as usize].found = 1;
                                }
                                Some(s0) => {
                                    if mriseg.segments[l as usize].found == 0 {
                                        mriseg.segments[l as usize].found = 1;
                                        segment_merge(g, &mut mriseg, s0, l as usize, &mut labels);
                                        low = low.min(l as usize);
                                    }
                                }
                            }
                        }
                        for &l in &border {
                            if l >= 0 {
                                mriseg.segments[l as usize].found = 0;
                            }
                        }
                        first.unwrap_or(0)
                    }
                };
                let m = &mut mriseg.segments[label];
                m.voxels.push(Msv {
                    x: x as i16,
                    y: y as i16,
                    z: z as i16,
                });
                m.area += voxel_size;
                labels.set(g, x, y, z, label as i32);
            }
        }
    }
    if let Some(heap) = heap {
        let mem = heap(err);
        print_heap(out, mem);
        let maxarea = mri_max_segment_area(&mriseg);
        let _ = write!(
            out,
            "\n             removing small segments (less than 1 percent of maxarea)."
        );
        remove_small(&mut mriseg, (0.01 * f64::from(maxarea)) as i32);
    } else {
        compact(&mut mriseg);
    }
    compute_statistics(&mut mriseg);
    Ok(mriseg)
}

/// `MRImaxsegment`'s label map after small segments were removed during the scan: FreeSurfer
/// clears the labels of `nsegments` and above but keeps the old numbers of the segments that
/// compaction moved (FS-578); by default the map is rebuilt from the segments.
fn relabel_after_removal(
    g: &mut Globals,
    mriseg: &MriSegmentation,
    labels: &mut LabelMap,
    moved: bool,
) {
    if moved {
        g.notices.add(
            &FS_578,
            "MRImaxsegment removed small segments during its scan and renumbered the others",
        );
    }
    if moved && !g.notices.reproduce() {
        for i in 0..labels.len() {
            labels.set_i(i, -1);
        }
        for (s, m) in mriseg.segments.iter().enumerate() {
            for v in &m.voxels {
                labels.set(g, i32::from(v.x), i32::from(v.y), i32::from(v.z), s as i32);
            }
        }
        return;
    }
    for i in 0..labels.len() {
        if labels.get_i(i) >= mriseg.nsegments {
            labels.set_i(i, -1);
        }
    }
}

/// `MRIsegment(mri, low_val, hi_val)`: the 6-connected segments of the voxels of frame 0 whose
/// value is within `[low_val, hi_val]`, in scan order (slice, row, column): a voxel joins the
/// segment of its first labelled neighbour (in the order slice before, row before, column before,
/// column after, row after, slice after); a voxel between several segments merges the others into
/// the first; the table compacted, then the statistics.
pub fn mri_segment(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    low_val: f32,
    hi_val: f32,
) -> Result<MriSegmentation, Fatal> {
    scan(g, out, err, mri, low_val, hi_val, None)
}

/// `MRImaxsegment(mri, low_val, hi_val)`: as [`mri_segment`] with the values truncated to `int`,
/// and, whenever the table is full past 8,800 slots, the heap usage, the largest segment and, when
/// the voxels left to scan cannot outgrow it, the segments under 1 % of it removed (FS-578); at
/// the end the heap usage and the segments under 1 % of the largest removed. `heap` is
/// `getMemoryUsed` (`larmorx_freesurfer_core::utils::get_memory_used`).
pub fn mri_max_segment(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    low_val: f32,
    hi_val: f32,
    heap: &mut HeapUsage,
) -> Result<MriSegmentation, Fatal> {
    scan(g, out, err, mri, low_val, hi_val, Some(heap))
}

/// `MRIsegmentToImage(mri_src, mri_dst, mriseg, s)`: the source's frame-0 values copied at the
/// voxels of segment `s` (every segment for `s` below 0) into the destination (a clone of the
/// source when none is given); `s` at or above `nsegments` returns the destination after
/// `MRIsegmentToImage: invalid segment #<s>`.
pub fn mri_segment_to_image(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    mriseg: &MriSegmentation,
    s: i32,
) -> Result<Ret<Mri>, Fatal> {
    let mut new = None;
    let given = dst.is_some();
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let Ret::New(m) = mri_clone(g, out, err, src, None)? else {
                unreachable!("a new clone")
            };
            new.insert(m)
        }
    };
    if s >= mriseg.nsegments {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("MRIsegmentToImage: invalid segment #{s}"),
        );
    } else {
        let (smin, smax) = if s < 0 {
            (0, mriseg.nsegments - 1)
        } else {
            (s, s)
        };
        for s in smin..=smax {
            copy_segment(g, err, src, d, &mriseg.segments[s as usize])?;
        }
    }
    Ok(match new {
        Some(m) if !given => Ret::New(m),
        _ => Ret::Given,
    })
}

fn copy_segment(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    dst: &mut Mri,
    m: &MriSegment,
) -> Result<(), Fatal> {
    for v in &m.voxels {
        let (x, y, z) = (i32::from(v.x), i32::from(v.y), i32::from(v.z));
        let val = mri_get_vox_val(g, err, src, x, y, z, 0, 0)?;
        mri_set_vox_val(g, err, dst, x, y, z, 0, val, 0)?;
    }
    Ok(())
}

/// `MRIsetSegmentValue(mri, mriseg, s, val)`: frame 0 of the voxels of segment `s` set to `val`
/// (`NO_ERROR`). A segment number outside the table reads memory outside it (FS-579): refused.
pub fn mri_set_segment_value(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
    mriseg: &MriSegmentation,
    s: i32,
    val: f32,
) -> Result<i32, Fatal> {
    if s < 0 || s >= mriseg.max_segments() {
        g.notices.add(
            &FS_579,
            format!(
                "MRIsetSegmentValue of segment {s} of a table of {}",
                mriseg.max_segments()
            ),
        );
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "MRIsetSegmentValue: invalid segment #{s} (FreeSurfer reads outside its segment table)"
            ),
        );
        return Ok(ERROR_BADPARM);
    }
    for v in &mriseg.segments[s as usize].voxels {
        mri_set_vox_val(
            g,
            err,
            mri,
            i32::from(v.x),
            i32::from(v.y),
            i32::from(v.z),
            0,
            val,
            0,
        )?;
    }
    Ok(NO_ERROR)
}

/// `mri->xi[i]` (and `yi`, `zi`): the boundary index table's entry, or FS-582 beyond its margin.
fn table(
    g: &mut Globals,
    err: &mut dyn Write,
    t: Option<&Vec<i32>>,
    i: i32,
    what: &str,
) -> Result<i32, Fatal> {
    let Some(t) = t else {
        return Ok(0);
    };
    if i < -MAX_INDEX || i + MAX_INDEX >= t.len() as i32 {
        g.notices.add(
            &FS_582,
            format!("{what}: a segment voxel next to index {i}, outside the volume's index table"),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "{what}: a segment voxel far outside the volume (FreeSurfer reads outside its \
                 index tables)"
            ),
        )?;
        return Ok(0);
    }
    Ok(Mri::index(t, i))
}

/// `MRIsegmentDilate(mriseg, mri)` and `MRIsegmentDilateThreshold(mriseg, mri_binary,
/// mri_thresh, low, hi)` (`thresh` given).
fn dilate(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mriseg: &mut MriSegmentation,
    mri: &Mri,
    thresh: Option<(&Mri, i32, i32)>,
) -> Result<i32, Fatal> {
    let what = if thresh.is_some() {
        "MRIsegmentDilateThreshold"
    } else {
        "MRIsegmentDilate"
    };
    let voxel_size = mri.vg.xsize * mri.vg.ysize * mri.vg.zsize;
    let Ret::New(mut segs) = mri_clone(g, out, err, mri, None)? else {
        unreachable!("a new clone")
    };
    let n = mriseg.nsegments.max(0) as usize;
    for m in &mriseg.segments[..n.min(mriseg.segments.len())] {
        copy_segment(g, err, mri, &mut segs, m)?;
    }
    for segno in 0..n.min(mriseg.segments.len()) {
        let nvox = mriseg.segments[segno].voxels.len();
        for v in 0..nvox {
            let p = mriseg.segments[segno].voxels[v];
            let (x, y, z) = (i32::from(p.x), i32::from(p.y), i32::from(p.z));
            for xk in -1..=1i32 {
                let xi = table(g, err, mri.xi.as_ref(), x + xk, what)?;
                for yk in -1..=1i32 {
                    let yi = table(g, err, mri.yi.as_ref(), y + yk, what)?;
                    for zk in -1..=1i32 {
                        if xk.abs() + yk.abs() + zk.abs() != 1 {
                            continue;
                        }
                        let zi = table(g, err, mri.zi.as_ref(), z + zk, what)?;
                        let tval = match thresh {
                            Some((t, _, _)) => {
                                let f = mri_get_vox_val(g, err, t, xi, yi, zi, 0, 0)?;
                                float_to_int(g, f64::from(f), what)
                            }
                            None => 0,
                        };
                        let a = mri_get_vox_val(g, err, mri, xi, yi, zi, 0, 0)?;
                        if a == 0.0 {
                            continue;
                        }
                        if mri_get_vox_val(g, err, &segs, xi, yi, zi, 0, 0)? != 0.0 {
                            continue;
                        }
                        if let Some((_, lo, hi)) = thresh
                            && !(tval >= lo && tval <= hi)
                        {
                            continue;
                        }
                        let m = &mut mriseg.segments[segno];
                        m.voxels.push(Msv {
                            x: xi as i16,
                            y: yi as i16,
                            z: zi as i16,
                        });
                        let val = mri_get_vox_val(g, err, mri, xi, yi, zi, 0, 0)?;
                        mri_set_vox_val(g, err, &mut segs, xi, yi, zi, 0, val, 0)?;
                        mriseg.segments[segno].area += voxel_size;
                    }
                }
            }
        }
    }
    Ok(NO_ERROR)
}

/// `MRIsegmentDilate(mriseg, mri)`: each segment (in order, its voxels as they were before it)
/// grows by its 6-neighbours (column, then row, then slice offsets, -1 before +1; positions
/// clamped to the volume) whose frame-0 value is not 0 and that no segment holds yet; the
/// image of the segments starts as the volume's values at their voxels (`NO_ERROR`).
pub fn mri_segment_dilate(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mriseg: &mut MriSegmentation,
    mri: &Mri,
) -> Result<i32, Fatal> {
    dilate(g, out, err, mriseg, mri, None)
}

/// `MRIsegmentDilateThreshold(mriseg, mri_binary, mri_thresh, low_thresh, hi_thresh)`: as
/// [`mri_segment_dilate`] on `mri_binary`, a neighbour taken only when `mri_thresh`'s frame-0 value
/// truncated to `int` (FS-17 beyond `int`) is within `[low_thresh, hi_thresh]` (`NO_ERROR`).
#[allow(clippy::too_many_arguments)]
pub fn mri_segment_dilate_threshold(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mriseg: &mut MriSegmentation,
    mri_binary: &Mri,
    mri_thresh: &Mri,
    low_thresh: i32,
    hi_thresh: i32,
) -> Result<i32, Fatal> {
    dilate(
        g,
        out,
        err,
        mriseg,
        mri_binary,
        Some((mri_thresh, low_thresh, hi_thresh)),
    )
}

/// `MRIeraseSmallSegments(mriseg, mri_seg, min_voxels)`: every frame of the voxels of the slots
/// with fewer than `min_voxels` voxels set to 0 (`MRIeraseSmallSegments: erasing (<x>, <y> <z>)`
/// at the diagnostic voxel); the number of voxels erased.
pub fn mri_erase_small_segments(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mriseg: &MriSegmentation,
    mri_seg: &mut Mri,
    min_voxels: i32,
) -> Result<i32, Fatal> {
    let mut nerased = 0;
    for m in &mriseg.segments {
        if m.nvoxels() < min_voxels {
            for v in &m.voxels {
                let (x, y, z) = (i32::from(v.x), i32::from(v.y), i32::from(v.z));
                if x == g.gx && y == g.gy && z == g.gz {
                    let _ = writeln!(out, "MRIeraseSmallSegments: erasing ({x}, {y} {z})");
                }
                for f in 0..mri_seg.nframes {
                    mri_set_vox_val(g, err, mri_seg, x, y, z, f, 0.0, 0)?;
                }
                nerased += 1;
            }
        }
    }
    Ok(nerased)
}

/// `MRIsegmentMax(mriseg)`: the first of the first `nsegments` segments not ignored with the most
/// voxels (above 0), or -1.
pub fn mri_segment_max(mriseg: &MriSegmentation) -> i32 {
    let mut max_segno = -1;
    let mut max_voxels = 0;
    let n = (mriseg.nsegments.max(0) as usize).min(mriseg.segments.len());
    for (s, m) in mriseg.segments[..n].iter().enumerate() {
        if m.ignore != 0 {
            continue;
        }
        if m.nvoxels() > max_voxels {
            max_segno = s as i32;
            max_voxels = m.nvoxels();
        }
    }
    max_segno
}

/// `MRIsegmentClearIgnoreFlags(mriseg)`: the first `nsegments` segments' `ignore` cleared.
pub fn mri_segment_clear_ignore_flags(mriseg: &mut MriSegmentation) -> i32 {
    let n = (mriseg.nsegments.max(0) as usize).min(mriseg.segments.len());
    for m in &mut mriseg.segments[..n] {
        m.ignore = 0;
    }
    NO_ERROR
}

/// `MRIfindMaxSegmentNumber(mriseg)`: the last of the first `nsegments` segments with the most
/// voxels (0 for none).
pub fn mri_find_max_segment_number(mriseg: &MriSegmentation) -> i32 {
    let mut max_segno = 0;
    let mut max_voxels = 0;
    let n = (mriseg.nsegments.max(0) as usize).min(mriseg.segments.len());
    for (s, m) in mriseg.segments[..n].iter().enumerate() {
        if m.nvoxels() >= max_voxels {
            max_voxels = m.nvoxels();
            max_segno = s as i32;
        }
    }
    max_segno
}

/// `MRIsegmentFill(mriseg, s, mri, fillval)`: frame 0 of segment `s`'s voxels set to `fillval` in
/// `mri` (a clone of `seg_mri`, the volume segmented, when none is given); `NULL` after
/// `MRIsegmentFill: invalid segment #<s>` for `s` outside `[0, nsegments)`.
#[allow(clippy::too_many_arguments)]
pub fn mri_segment_fill(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mriseg: &MriSegmentation,
    seg_mri: &Mri,
    s: i32,
    mri: Option<&mut Mri>,
    fillval: f32,
) -> Result<Ret<Mri>, Fatal> {
    if s < 0 || s >= mriseg.nsegments {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("MRIsegmentFill: invalid segment #{s}"),
        );
        return Ok(Ret::Null);
    }
    let m = &mriseg.segments[s as usize];
    let set = |g: &mut Globals, err: &mut dyn Write, d: &mut Mri| -> Result<(), Fatal> {
        for v in &m.voxels {
            mri_set_vox_val(
                g,
                err,
                d,
                i32::from(v.x),
                i32::from(v.y),
                i32::from(v.z),
                0,
                fillval,
                0,
            )?;
        }
        Ok(())
    };
    match mri {
        Some(d) => {
            set(g, err, d)?;
            Ok(Ret::Given)
        }
        None => {
            let Ret::New(mut d) = mri_clone(g, out, err, seg_mri, None)? else {
                unreachable!("a new clone")
            };
            set(g, err, &mut d)?;
            Ok(Ret::New(d))
        }
    }
}
