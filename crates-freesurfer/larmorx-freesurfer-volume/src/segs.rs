// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Segment helpers of `utils/mri2.cpp` (3.1-38, taken first: fs.filter needs them):
//! `MRImatchSegs` (5687-5708), `MRIsegStats` (3826-3877), `HISTOseg` (5717-5740).
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §18):
//! - **FS-489** (c): `MRImatchSegs` into the segmentation itself clears each voxel before it reads
//!   it, so every voxel is matched as label 0. By default each voxel is read first (rule A);
//!   `reproduce` as FreeSurfer; noted where the two differ.
//! - **FS-490** (a, memory): `HISTOseg` with a bin count of 0 (`round((bmax - bmin) / bdelta)` of
//!   -1) counts a matching voxel in the bin before the first; both modes count it nowhere.

use std::io::Write;

use larmorx_core::defects::{FS_489, FS_490};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::histo::{Histogram, histo_init};
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add, add64, div64, mul, mul64, sqrt64, sub, sub64};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::arith::Pair;
use crate::labels::int_of;

/// `(double)` of a `float` as `cvtss2sd` makes it (a signalling NaN quieted).
fn widen(x: f32) -> f64 {
    if x.is_nan() {
        f64::from(f32::from_bits(x.to_bits() | 0x0040_0000))
    } else {
        f64::from(x)
    }
}

/// A `double` stored in a `float` (`cvtsd2ss`): rounded to nearest; a NaN keeps its sign and the
/// top 22 bits of its payload, quieted.
fn narrow(x: f64) -> f32 {
    if x.is_nan() {
        let b = x.to_bits();
        f32::from_bits(
            (((b >> 32) as u32) & 0x8000_0000) | 0x7fc0_0000 | (((b >> 29) as u32) & 0x003f_ffff),
        )
    } else {
        x as f32
    }
}

/// `MRImatchSegs(seg, seglist, nsegs, MaskId, mask)`: frame 0 of the mask (a copy of the
/// segmentation, `MRIcopy`, when none is given) over the segmentation's extent: `MaskId` (as a
/// `float`) where the voxel's label (`(int)` of its value) is in the list, else 0 (written with
/// `MRIsetVoxVal`). The segmentation itself as the mask: FS-489 (module documentation).
pub fn mri_match_segs(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    seglist: &[i32],
    mask_id: i32,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRImatchSegs";
    let listed = |id: i32| seglist.contains(&id);
    let maskval = mask_id as f32;
    match p {
        Pair::Same(m) => {
            let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
            for c in 0..w {
                for r in 0..h {
                    for s in 0..d {
                        let own = mri_get_vox_val(g, err, m, c, r, s, 0, 0)?;
                        mri_set_vox_val(g, err, m, c, r, s, 0, 0.0, 0)?;
                        // FreeSurfer reads the voxel it has just cleared
                        let seen = mri_get_vox_val(g, err, m, c, r, s, 0, 0)?;
                        let id_fs = int_of(g, WHAT, f64::from(seen));
                        let id = int_of(g, WHAT, f64::from(own));
                        if listed(id) != listed(id_fs) {
                            g.notices.add(
                                &FS_489,
                                "a segmentation matched into itself read its voxels after clearing them",
                            );
                        }
                        let id = if g.notices.reproduce() { id_fs } else { id };
                        if listed(id) {
                            mri_set_vox_val(g, err, m, c, r, s, 0, maskval, 0)?;
                        }
                    }
                }
            }
            Ok(Ret::Given)
        }
        Pair::New(seg) | Pair::Given(seg, _) => {
            let (seg, given) = match p {
                Pair::Given(seg, mask) => (seg, Some(mask)),
                _ => (seg, None),
            };
            let mut fresh = None;
            let mask: &mut Mri = match given {
                Some(m) => m,
                None => fresh.insert(
                    mri_copy(g, out, err, seg, None)?
                        .into_new()
                        .expect("MRIcopy without a destination"),
                ),
            };
            let (w, h, d) = (seg.vg.width, seg.vg.height, seg.vg.depth);
            for c in 0..w {
                for r in 0..h {
                    for s in 0..d {
                        mri_set_vox_val(g, err, mask, c, r, s, 0, 0.0, 0)?;
                        let v = mri_get_vox_val(g, err, seg, c, r, s, 0, 0)?;
                        if listed(int_of(g, WHAT, f64::from(v))) {
                            mri_set_vox_val(g, err, mask, c, r, s, 0, maskval, 0)?;
                        }
                    }
                }
            }
            Ok(match fresh {
                Some(m) => Ret::New(m),
                None => Ret::Given,
            })
        }
    }
}

/// What [`mri_seg_stats`] writes back (`min`, `max`, `range`, `mean`, `std`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SegStats {
    pub min: f32,
    pub max: f32,
    pub range: f32,
    pub mean: f32,
    pub std: f32,
}

/// `MRIsegStats(seg, segid, mri, frame, &min, &max, &range, &mean, &std)`: over the segmentation's
/// extent (columns outermost), the voxels whose label (`(int)` of frame 0's value) is `segid`:
/// their count (returned), and of `mri`'s values in `frame` the minimum and maximum (`float`
/// comparisons from the first value; 0 and 0 when there is none), `range = max - min`, `mean =
/// sum / n` (`double` sums: `sum + v`, `v v + sum2`; 0 when there is none) and, for more than one
/// voxel, `std = sqrt((n mean mean - 2 mean sum + sum2) / (n - 1))` with `n mean mean` and `2 mean`
/// (as `mean + mean`) in `float`, the rest in `double` (0 otherwise).
pub fn mri_seg_stats(
    g: &mut Globals,
    err: &mut dyn Write,
    seg: &Mri,
    segid: i32,
    mri: &Mri,
    frame: i32,
) -> Result<(i32, SegStats), Fatal> {
    const WHAT: &str = "MRIsegStats";
    let mut st = SegStats::default();
    let (mut sum, mut sum2) = (0.0f64, 0.0f64);
    let mut n = 0i32;
    for c in 0..seg.vg.width {
        for r in 0..seg.vg.height {
            for s in 0..seg.vg.depth {
                let lab = mri_get_vox_val(g, err, seg, c, r, s, 0, 0)?;
                let id = int_of(g, WHAT, f64::from(lab));
                if id != segid {
                    continue;
                }
                let val = mri_get_vox_val(g, err, mri, c, r, s, frame, 0)?;
                n = n.wrapping_add(1);
                if n == 1 {
                    st.min = val;
                    st.max = val;
                }
                if st.min > val {
                    st.min = val;
                }
                if st.max < val {
                    st.max = val;
                }
                let v = widen(val);
                sum = add64(sum, v);
                sum2 = add64(mul64(v, v), sum2);
            }
        }
    }
    st.range = sub(st.max, st.min);
    st.mean = if n != 0 {
        narrow(div64(sum, f64::from(n)))
    } else {
        0.0
    };
    st.std = if n > 1 {
        let a = mul(mul(n as f32, st.mean), st.mean);
        let b = mul64(widen(add(st.mean, st.mean)), sum);
        let q = add64(sub64(widen(a), b), sum2);
        narrow(sqrt64(div64(q, f64::from(n - 1))))
    } else {
        0.0
    };
    Ok((n, st))
}

/// `HISTOseg(seg, segid, vol, bmin, bmax, bdelta)`: a histogram of `round((bmax - bmin) /
/// bdelta) + 1` bins (`HISTOinit` with `bmin`, `bmax`) of frame 0 of `vol` over the voxels whose
/// label (frame 0 of `seg`, as a `double`) is `segid` (columns outermost): each voxel counts
/// one in bin `(int)(round(v - bmin) / bdelta)`, clamped to the first and the last bin. FS-490:
/// with no bin (module documentation).
#[allow(clippy::too_many_arguments)]
pub fn histo_seg(
    g: &mut Globals,
    err: &mut dyn Write,
    seg: &Mri,
    segid: i32,
    vol: &Mri,
    bmin: f64,
    bmax: f64,
    bdelta: f64,
) -> Result<Histogram, Fatal> {
    const WHAT: &str = "HISTOseg";
    let nbins = int_of(g, WHAT, div64(sub64(bmax, bmin), bdelta).round() + 1.0);
    if nbins < 0 {
        // HISTOalloc's calloc of a negative count fails and sets errno (ENOMEM), which its error
        // message prints first (`error: Cannot allocate memory`); the port's histo_alloc does not
        // set it (reported to fs.maths' track)
        g.errno = 12;
    }
    let mut h = histo_init(g, err, None, nbins, bmin, bmax)?
        .into_new()
        .expect("HISTOinit without a histogram");
    for c in 0..seg.vg.width {
        for r in 0..seg.vg.height {
            for s in 0..seg.vg.depth {
                let vsegid = widen(mri_get_vox_val(g, err, seg, c, r, s, 0, 0)?);
                if vsegid != f64::from(segid) {
                    continue;
                }
                let v = widen(mri_get_vox_val(g, err, vol, c, r, s, 0, 0)?);
                let mut binno = int_of(g, WHAT, div64(sub64(v, bmin).round(), bdelta));
                if binno < 0 {
                    binno = 0;
                }
                if binno >= nbins {
                    binno = nbins - 1;
                }
                match usize::try_from(binno).ok().filter(|&b| b < h.counts.len()) {
                    Some(b) => h.counts[b] += 1.0,
                    None => g.notices.add(
                        &FS_490,
                        format!("HISTOseg: a voxel of segment {segid} with no bin to count it in"),
                    ),
                }
            }
        }
    }
    Ok(h)
}
