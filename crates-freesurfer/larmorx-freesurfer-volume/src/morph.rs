// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The binary-volume classes of `utils/mri2.cpp` (3.1-37, relay 29): `FixSubCortMassHA::FixSCM`
//! (6534), `BinarizeMRI` (`dump` 6620, `qualifies` 6652, `binarize` 6692) and `DEMorphBinVol`
//! (`morph` 6794, 6846, 6868, `check` 6890, `dump` 6923, `copyFrame` 6937), as structures with
//! FreeSurfer's fields and defaults. Their debugging lines go to standard output (the classes'
//! default stream). FreeSurfer runs the voxel loops in parallel; each voxel is its own, and the
//! counts are sums, so the results are those of one thread.
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §25):
//! - **FS-803** (rule A): `BinarizeMRI::binarize` from a first frame above 0 writes the off value
//!   of masked or edge voxels into the input's frame number of the output. By default the output's
//!   frame; `reproduce` FreeSurfer's (the voxel functions' notices past the output's frames).
//! - **FS-804** (memory): `DEMorphBinVol::morph` reads each voxel's own value from frame `frame`
//!   of its one-frame working copy. By default frame 0 (rule A); `reproduce` FreeSurfer's read.
//! - **FS-805** (rule A): the thresholding and matching `morph`s pass their one-frame binarized
//!   volume to `morph`, which selects frame `frame` of it again. By default its frame 0;
//!   `reproduce` FreeSurfer's selection.
//! - **FS-806** (memory, rule B): `check` and `copyFrame` accept a frame equal to the number of
//!   frames. By default it is out of range (their error); `reproduce` reads it.
//! - **FS-807** (rule B): `morph(binvol)` ignores `check`'s failure. By default it returns `NULL`
//!   after `check`'s error, as the other `morph`s do; `reproduce` goes on.
//! - **FS-808** (rule A): the thresholding and matching `morph`s of a volume of several frames copy
//!   the frame into bytes (`copyFrame`'s default type) before binarizing it. By default the copy
//!   keeps the input's type; `reproduce` bytes.

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_803, FS_804, FS_805, FS_806, FS_807, FS_808};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_UCHAR, Mri, mri_alloc, mri_alloc_sequence, mri_copy_header,
    mri_copy_pulse_parameters,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::arith::Pair;
use crate::cma::{is_amygdala, is_hippo, is_inf_lat_vent};
use crate::combine::mri_dim_mismatch;
use crate::labels::int_of;
use crate::mriset::mri_dilate;

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

/// `%g` of a `double`.
fn gf(x: f64) -> String {
    cfmt::g(x, 6)
}

/// `FixSubCortMassHA::FixSCM()` with `subcorticalmass`, `aseg` and `nDilate`: the mask (a new `int`
/// volume with the segmentation's header and pulse parameters) is 1 where the segmentation's label
/// (its value truncated to an `int`) is 0, 2, 3, 41 or 42, then dilated `nDilate` times in place
/// (`MRIdilate(mask, mask)`, after `Dilating <n> voxels in 3d` on standard output); then the
/// subcortical mass is set to 0 at every amygdala and inferior lateral ventricle voxel, and at
/// every hippocampus voxel outside the mask (its value not above 0.5). Returns 0 and the mask
/// (the structure's `mask`, which callers free).
pub fn fix_scm(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    scm: &mut Mri,
    aseg: &Mri,
    ndilate: i32,
) -> Result<(i32, Mri), Fatal> {
    const WHAT: &str = "FixSubCortMassHA::FixSCM";
    let (w, h, d) = (aseg.vg.width, aseg.vg.height, aseg.vg.depth);
    let mut mask = mri_alloc(g, out, err, w, h, d, MRI_INT)?;
    mri_copy_header(g, out, err, aseg, Some(&mut mask))?;
    mri_copy_pulse_parameters(aseg, &mut mask);
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                let v = mri_get_vox_val(g, err, aseg, c, r, s, 0, 0)?;
                let segid = int_of(g, WHAT, f64::from(v));
                if matches!(segid, 0 | 2 | 3 | 41 | 42) {
                    mri_set_vox_val(g, err, &mut mask, c, r, s, 0, 1.0, 0)?;
                }
            }
        }
    }
    let _ = writeln!(out, "Dilating {ndilate} voxels in 3d");
    for _ in 0..ndilate {
        mri_dilate(g, out, err, Pair::Same(&mut mask))?;
    }
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                let v = mri_get_vox_val(g, err, aseg, c, r, s, 0, 0)?;
                let segid = int_of(g, WHAT, f64::from(v));
                if is_amygdala(segid) || is_inf_lat_vent(segid) {
                    mri_set_vox_val(g, err, scm, c, r, s, 0, 0.0, 0)?;
                    continue;
                }
                if f64::from(mri_get_vox_val(g, err, &mask, c, r, s, 0, 0)?) > 0.5 {
                    continue;
                }
                if is_hippo(segid)
                    && f64::from(mri_get_vox_val(g, err, &mask, c, r, s, 0, 0)?) < 0.5
                {
                    mri_set_vox_val(g, err, scm, c, r, s, 0, 0.0, 0)?;
                    continue;
                }
            }
        }
    }
    Ok((0, mask))
}

/// `class BinarizeMRI`: its fields with FreeSurfer's defaults (`BinType` 1, thresholds 0.5 to
/// infinity, the mask's 0.5 to infinity, no zeroed edges, on 1 and off 0, frames -1 to -1, no
/// inversion, output type -1 (`float`), no absolute value, no debugging).
#[derive(Clone, Debug, PartialEq)]
pub struct BinarizeMri {
    /// `BinType`: 1 thresholds, 2 a match list.
    pub bin_type: i32,
    pub thmin: f64,
    pub thmax: f64,
    pub matchlist: Vec<i32>,
    pub maskthmin: f64,
    pub maskthmax: f64,
    pub zero_col_edges: i32,
    pub zero_row_edges: i32,
    pub zero_slice_edges: i32,
    pub on_val: f64,
    pub off_val: f64,
    pub fstart: i32,
    pub fend: i32,
    pub invert: i32,
    /// `nhits`: the voxels (of every frame) set to the on value by the last `binarize`.
    pub nhits: i32,
    pub mritype: i32,
    pub do_abs: i32,
    /// `m_debug`.
    pub debug: i32,
}

impl Default for BinarizeMri {
    fn default() -> Self {
        BinarizeMri {
            bin_type: 1,
            thmin: 0.5,
            thmax: f64::INFINITY,
            matchlist: Vec::new(),
            maskthmin: 0.5,
            maskthmax: f64::INFINITY,
            zero_col_edges: 0,
            zero_row_edges: 0,
            zero_slice_edges: 0,
            on_val: 1.0,
            off_val: 0.0,
            fstart: -1,
            fend: -1,
            invert: 0,
            nhits: 0,
            mritype: -1,
            do_abs: 0,
            debug: 0,
        }
    }
}

impl BinarizeMri {
    /// `BinarizeMRI::dump(invol, mask, outvol)`: `BinarizeMRI: BinType=<n> `, ` thmin=<g> thmax=<g> `
    /// (type 1) or `match N=<n> ` and each value with a space (type 2), `maskthmin=<g>
    /// maskthmax=<g> ` with a mask, `zce=<n> zre=<n> zse=<n> on=<g> off=<g> fstart=<n> fend=<n>
    /// invert=<n> DoAbs=<n> mritype=<n> ` and a newline; then, for type 1 with a match list,
    /// `WARNING: BinType=1 but matchlist not empty N=<n>`. Returns 0.
    pub fn dump(&self, out: &mut dyn Write, mask: bool) -> i32 {
        let mut s = format!("BinarizeMRI: BinType={} ", self.bin_type);
        if self.bin_type == 1 {
            s += &format!(" thmin={} thmax={} ", gf(self.thmin), gf(self.thmax));
        }
        if self.bin_type == 2 {
            s += &format!("match N={} ", self.matchlist.len() as i32);
            for m in &self.matchlist {
                s += &format!("{m} ");
            }
        }
        if mask {
            s += &format!(
                "maskthmin={} maskthmax={} ",
                gf(self.maskthmin),
                gf(self.maskthmax)
            );
        }
        s += &format!(
            "zce={} zre={} zse={} ",
            self.zero_col_edges, self.zero_row_edges, self.zero_slice_edges
        );
        s += &format!("on={} off={} ", gf(self.on_val), gf(self.off_val));
        s += &format!("fstart={} fend={} ", self.fstart, self.fend);
        s += &format!("invert={} ", self.invert);
        s += &format!("DoAbs={} ", self.do_abs);
        s += &format!("mritype={} ", self.mritype);
        s += "\n";
        if self.bin_type == 1 && !self.matchlist.is_empty() {
            s += &format!(
                "WARNING: BinType=1 but matchlist not empty N={}\n",
                self.matchlist.len() as i32
            );
        }
        let _ = out.write_all(s.as_bytes());
        0
    }

    /// `BinarizeMRI::qualifies(invol, mask, c, r, s, f)`: 0 when the mask's value (frame 0) is
    /// below its minimum or above its maximum, or the voxel is on a zeroed edge; 1 for `f < 0`
    /// (only those tested); else whether the voxel's value of frame `f` (its absolute value with
    /// `DoAbs`) is within the thresholds (type 1, both ends included) or equals a value of the
    /// match list (type 2). NaN values pass both tests.
    #[allow(clippy::too_many_arguments)]
    pub fn qualifies(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        invol: &Mri,
        mask: Option<&Mri>,
        c: i32,
        r: i32,
        s: i32,
        f: i32,
    ) -> Result<i32, Fatal> {
        if let Some(m) = mask {
            let mval = f64::from(mri_get_vox_val(g, err, m, c, r, s, 0, 0)?);
            if mval < self.maskthmin || mval > self.maskthmax {
                return Ok(0);
            }
        }
        if (self.zero_col_edges != 0 && (c == 0 || c == invol.vg.width - 1))
            || (self.zero_row_edges != 0 && (r == 0 || r == invol.vg.height - 1))
            || (self.zero_slice_edges != 0 && (s == 0 || s == invol.vg.depth - 1))
        {
            return Ok(0);
        }
        if f < 0 {
            return Ok(1);
        }
        let mut sval = f64::from(mri_get_vox_val(g, err, invol, c, r, s, f, 0)?);
        if self.do_abs != 0 {
            sval = sval.abs();
        }
        let mut q = 1;
        if self.bin_type == 1 {
            if sval < self.thmin || sval > self.thmax {
                q = 0;
            }
        } else {
            q = 0;
            for &m in &self.matchlist {
                if sval == f64::from(m) {
                    q = 1;
                    break;
                }
            }
        }
        Ok(q)
    }

    /// `BinarizeMRI::binarize(invol, mask, outvol)`: each voxel of the frames `fstart` to `fend - 1`
    /// (-1 meaning 0 and the input's frames; both stored back into the structure) becomes the on
    /// value where it qualifies ([`BinarizeMri::qualifies`]), else the off value (swapped with
    /// `invert`), into frame `f - fstart` of the output: a new volume of the input's extent, the
    /// type `mritype` (`float` for -1) and `fend - fstart` frames with the input's header and pulse
    /// parameters, or the caller's. A voxel outside the mask or on a zeroed edge is off in every
    /// frame. `nhits` is the number of on voxels (with `invert`, the others). Errors on standard
    /// output with `Null`: no input (`ERROR: BinarizeMRI::binarize(): input is NULL`), `fend` past
    /// the frames (`fend=<n> >= nframes=<n>`, then the dump), a mask or output of another extent
    /// (`input/mask dim mismatch`; `input/output dim mismatch` and the dump). With `m_debug` the
    /// dump first and after the swap, and `BinarizeMRI: nhits = <n>`; a type-1 structure with a
    /// match list warns (`WARNING: BinarizeMRI::binarize(): BinType=1 but matchlist not empty
    /// N=<n>` and the dump). FS-803.
    pub fn binarize(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        invol: Option<&Mri>,
        mask: Option<&Mri>,
        outvol: Option<&mut Mri>,
    ) -> Result<Ret<Mri>, Fatal> {
        if self.debug != 0 {
            self.dump(out, mask.is_some());
        }
        let Some(invol) = invol else {
            let _ = writeln!(out, "ERROR: BinarizeMRI::binarize(): input is NULL");
            return Ok(Ret::Null);
        };
        if self.bin_type == 1 && !self.matchlist.is_empty() {
            let _ = writeln!(
                out,
                "WARNING: BinarizeMRI::binarize(): BinType=1 but matchlist not empty N={}",
                self.matchlist.len() as i32
            );
            self.dump(out, mask.is_some());
        }
        if self.fstart < 0 {
            self.fstart = 0;
        }
        if self.fend < 0 {
            self.fend = invol.nframes;
        }
        if self.fend > invol.nframes {
            let _ = writeln!(
                out,
                "ERROR: BinarizeMRI::binarize(): fend={} >= nframes={}",
                self.fend, invol.nframes
            );
            self.dump(out, mask.is_some());
            return Ok(Ret::Null);
        }
        if let Some(m) = mask
            && mri_dim_mismatch(invol, m, false) != 0
        {
            let _ = writeln!(
                out,
                "ERROR: BinarizeMRI::binarize(): input/mask dim mismatch"
            );
            return Ok(Ret::Null);
        }
        let mut fresh = None;
        let o: &mut Mri = match outvol {
            None => {
                let ty = if self.mritype < 0 {
                    MRI_FLOAT
                } else {
                    self.mritype
                };
                let mut m = mri_alloc_sequence(
                    g,
                    out,
                    err,
                    invol.vg.width,
                    invol.vg.height,
                    invol.vg.depth,
                    ty,
                    self.fend - self.fstart,
                )?;
                mri_copy_header(g, out, err, invol, Some(&mut m))?;
                mri_copy_pulse_parameters(invol, &mut m);
                fresh.insert(m)
            }
            Some(d) => {
                if mri_dim_mismatch(invol, d, false) != 0 {
                    let _ = writeln!(
                        out,
                        "ERROR: BinarizeMRI::binarize(): input/output dim mismatch"
                    );
                    self.dump(out, mask.is_some());
                    return Ok(Ret::Null);
                }
                d
            }
        };
        let (on0, off0) = (self.on_val, self.off_val);
        if self.invert != 0 {
            self.on_val = off0;
            self.off_val = on0;
        }
        if self.debug != 0 {
            self.dump(out, mask.is_some());
        }
        let (fstart, fend) = (self.fstart, self.fend);
        let (on, off) = (narrow(self.on_val), narrow(self.off_val));
        let mut nhits = 0i32;
        for c in 0..invol.vg.width {
            for r in 0..invol.vg.height {
                for s in 0..invol.vg.depth {
                    if self.qualifies(g, err, invol, mask, c, r, s, -1)? == 0 {
                        for f in fstart..fend {
                            let fo = if fstart != 0 {
                                g.notices.add(
                                    &FS_803,
                                    format!(
                                        "BinarizeMRI::binarize: frames from {fstart}: a masked or edge voxel's frame {f} written into the output's frame {f}"
                                    ),
                                );
                                if g.notices.reproduce() { f } else { f - fstart }
                            } else {
                                f
                            };
                            mri_set_vox_val(g, err, o, c, r, s, fo, off, 0)?;
                        }
                        continue;
                    }
                    for f in fstart..fend {
                        let v = if self.qualifies(g, err, invol, None, c, r, s, f)? != 0 {
                            nhits = nhits.wrapping_add(1);
                            on
                        } else {
                            off
                        };
                        mri_set_vox_val(g, err, o, c, r, s, f - fstart, v, 0)?;
                    }
                }
            }
        }
        if self.invert != 0 {
            self.on_val = on0;
            self.off_val = off0;
            nhits = invol
                .vg
                .width
                .wrapping_mul(invol.vg.height)
                .wrapping_mul(invol.vg.depth)
                .wrapping_mul(fend - fstart)
                .wrapping_sub(nhits);
        }
        self.nhits = nhits;
        if self.debug != 0 {
            let _ = writeln!(out, "BinarizeMRI: nhits = {nhits}");
        }
        Ok(match fresh {
            Some(m) => Ret::New(m),
            None => Ret::Given,
        })
    }
}

/// `class DEMorphBinVol`: its fields with FreeSurfer's defaults (dilation, face neighbours, a
/// neighbour threshold of 0, no passes, frame 0, output type -1 (bytes), debugging on).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeMorphBinVol {
    /// `morphtype`: 1 dilation, 2 erosion.
    pub morphtype: i32,
    /// `topo`: 1 face, 2 face and edge, 3 every neighbour.
    pub topo: i32,
    pub nnbrsthresh: i32,
    pub nmorph: i32,
    pub frame: i32,
    /// `nchangestot`: the voxels changed by the last `morph`.
    pub nchangestot: i32,
    /// `mm_mritype`.
    pub mritype: i32,
    /// `mm_debug`.
    pub debug: i32,
}

impl Default for DeMorphBinVol {
    fn default() -> Self {
        DeMorphBinVol {
            morphtype: 1,
            topo: 1,
            nnbrsthresh: 0,
            nmorph: 0,
            frame: 0,
            nchangestot: 0,
            mritype: -1,
            debug: 1,
        }
    }
}

/// Whether `frame` is outside the volume's frames as `check` and `copyFrame` test it: below 0 or
/// above the number of frames; FS-806 for one equal to it (out of range by default).
fn frame_out(g: &mut Globals, what: &str, frame: i32, nframes: i32) -> bool {
    if frame < 0 || frame > nframes {
        return true;
    }
    if frame == nframes {
        g.notices.add(
            &FS_806,
            format!("{what}: frame {frame} of a volume of {nframes} frames accepted"),
        );
        return !g.notices.reproduce();
    }
    false
}

impl DeMorphBinVol {
    /// `DEMorphBinVol::check(invol, ubermask, outvol)` with the frame `frame`: 1 after an error on
    /// standard output for no input (`ERROR: DEMorphBinVol::check(): input is NULL`), a mask or
    /// output of another extent (`ERROR: DEMorphBinVol(): input/ubermask dim mismatch`, `ERROR:
    /// DEMorphBinVol::check(): input/output dim mismatch`), a frame out of range (`frame=<n>, out
    /// of range <nframes>`; FS-806), `topo` not 1 to 3 (`topo=<n>, must be 1, 2, or 3`) or
    /// `morphtype` not 1 or 2 (`morphtype=<n>, must be 1 or 2`); else 0.
    fn check_at(
        &self,
        g: &mut Globals,
        out: &mut dyn Write,
        invol: Option<&Mri>,
        ubermask: Option<&Mri>,
        outvol: Option<&Mri>,
        frame: i32,
    ) -> i32 {
        let Some(invol) = invol else {
            let _ = writeln!(out, "ERROR: DEMorphBinVol::check(): input is NULL");
            return 1;
        };
        if let Some(m) = ubermask
            && mri_dim_mismatch(invol, m, false) != 0
        {
            let _ = writeln!(out, "ERROR: DEMorphBinVol(): input/ubermask dim mismatch");
            return 1;
        }
        if let Some(o) = outvol
            && mri_dim_mismatch(invol, o, false) != 0
        {
            let _ = writeln!(
                out,
                "ERROR: DEMorphBinVol::check(): input/output dim mismatch"
            );
            return 1;
        }
        if frame_out(g, "DEMorphBinVol::check", frame, invol.nframes) {
            let _ = writeln!(
                out,
                "ERROR: DEMorphBinVol::check(): frame={frame}, out of range {}",
                invol.nframes
            );
            return 1;
        }
        if self.topo < 1 || self.topo > 3 {
            let _ = writeln!(
                out,
                "ERROR: DEMorphBinVol::check(): topo={}, must be 1, 2, or 3",
                self.topo
            );
            return 1;
        }
        if self.morphtype != 1 && self.morphtype != 2 {
            let _ = writeln!(
                out,
                "ERROR: DEMorphBinVol::check(): morphtype={}, must be 1 or 2",
                self.morphtype
            );
            return 1;
        }
        0
    }

    /// `DEMorphBinVol::check(invol, ubermask, outvol)` (see [`DeMorphBinVol::check_at`]) with the
    /// structure's frame.
    pub fn check(
        &self,
        g: &mut Globals,
        out: &mut dyn Write,
        invol: Option<&Mri>,
        ubermask: Option<&Mri>,
        outvol: Option<&Mri>,
    ) -> i32 {
        self.check_at(g, out, invol, ubermask, outvol, self.frame)
    }

    /// `DEMorphBinVol::dump(binvol)`: `DEMorphBinVol: topo=<n> morphtype=<n> nnbrsthresh=<n>
    /// frame=<n> nmorph=<n> mritype=<n> ` and a newline. Returns 0.
    pub fn dump(&self, out: &mut dyn Write) -> i32 {
        let _ = writeln!(
            out,
            "DEMorphBinVol: topo={} morphtype={} nnbrsthresh={} frame={} nmorph={} mritype={} ",
            self.topo, self.morphtype, self.nnbrsthresh, self.frame, self.nmorph, self.mritype
        );
        0
    }

    /// `copyFrame` into a volume of type `ty`.
    fn copy_frame_as(
        &self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        invol: &Mri,
        frameno: i32,
        ty: i32,
    ) -> Result<Option<Mri>, Fatal> {
        if frame_out(g, "DEMorphBinVol::copyFrame", frameno, invol.nframes) {
            let _ = writeln!(
                out,
                "ERROR: DEMorphBinVol::copyFrame(): frameno={frameno}, out of range {}",
                invol.nframes
            );
            return Ok(None);
        }
        let (w, h, d) = (invol.vg.width, invol.vg.height, invol.vg.depth);
        let mut fr = mri_alloc_sequence(g, out, err, w, h, d, ty, 1)?;
        mri_copy_header(g, out, err, invol, Some(&mut fr))?;
        mri_copy_pulse_parameters(invol, &mut fr);
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    // the value passes as a float (a signalling NaN keeps its bits)
                    let v = mri_get_vox_val(g, err, invol, c, r, s, frameno, 0)?;
                    mri_set_vox_val(g, err, &mut fr, c, r, s, 0, v, 0)?;
                }
            }
        }
        Ok(Some(fr))
    }

    /// `DEMorphBinVol::copyFrame(invol, frameno)`: a new one-frame volume of type `mm_mritype`
    /// (bytes for -1) with the input's header and pulse parameters, holding frame `frameno` of the
    /// input (`MRIsetVoxVal`); `None` after `ERROR: DEMorphBinVol::copyFrame(): frameno=<n>, out of
    /// range <nframes>` on standard output for a frame below 0 or past the frames (FS-806).
    pub fn copy_frame(
        &self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        invol: &Mri,
        frameno: i32,
    ) -> Result<Option<Mri>, Fatal> {
        let ty = if self.mritype < 0 {
            MRI_UCHAR
        } else {
            self.mritype
        };
        self.copy_frame_as(g, out, err, invol, frameno, ty)
    }

    /// `morph(binvol)` with the frame `frame` selected from `binvol`.
    fn morph_at(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        binvol: &Mri,
        frame: i32,
    ) -> Result<Option<Mri>, Fatal> {
        if self.check_at(g, out, Some(binvol), None, None, frame) != 0 {
            g.notices.add(
                &FS_807,
                "DEMorphBinVol::morph: check failed and the morphology goes on",
            );
            if !g.notices.reproduce() {
                return Ok(None);
            }
        }
        if self.debug != 0 {
            self.dump(out);
        }
        let Some(mut outvol) = self.copy_frame(g, out, err, binvol, frame)? else {
            return Ok(None);
        };
        // the voxel's own value: frame `frame` of the one-frame working copy (FS-804)
        let own = if frame != 0 {
            g.notices.add(
                &FS_804,
                format!("DEMorphBinVol::morph: a voxel's own value read from frame {frame} of a one-frame copy"),
            );
            if g.notices.reproduce() { frame } else { 0 }
        } else {
            0
        };
        const WHAT: &str = "DEMorphBinVol::morph";
        let (w, h, d) = (binvol.vg.width, binvol.vg.height, binvol.vg.depth);
        let (dil, ero) = (self.morphtype == 1, self.morphtype == 2);
        self.nchangestot = 0;
        for pass in 0..self.nmorph {
            let mut nchanges = 0i32;
            let Some(binvol0) = self.copy_frame(g, out, err, &outvol, 0)? else {
                return Ok(None);
            };
            for c in 1..w - 1 {
                for r in 1..h - 1 {
                    for s in 1..d - 1 {
                        let v = mri_get_vox_val(g, err, &binvol0, c, r, s, own, 0)?;
                        let binval = f64::from(int_of(g, WHAT, f64::from(v)));
                        if (dil && binval > 0.5) || (ero && binval < 0.5) {
                            continue;
                        }
                        let mut nnbrs = 0;
                        for dc in -1..=1i32 {
                            for dr in -1..=1i32 {
                                for ds in -1..=1i32 {
                                    if dc.abs() + dr.abs() + ds.abs() > self.topo {
                                        continue;
                                    }
                                    let v = mri_get_vox_val(
                                        g,
                                        err,
                                        &binvol0,
                                        c + dc,
                                        r + dr,
                                        s + ds,
                                        0,
                                        0,
                                    )?;
                                    let bv = f64::from(int_of(g, WHAT, f64::from(v)));
                                    if (dil && bv > 0.5) || (ero && bv < 0.5) {
                                        nnbrs += 1;
                                    }
                                    if nnbrs > self.nnbrsthresh {
                                        break;
                                    }
                                }
                                if nnbrs > self.nnbrsthresh {
                                    break;
                                }
                            }
                            if nnbrs > self.nnbrsthresh {
                                break;
                            }
                        }
                        if nnbrs > self.nnbrsthresh {
                            let v = if dil { 1.0 } else { 0.0 };
                            mri_set_vox_val(g, err, &mut outvol, c, r, s, 0, v, 0)?;
                            nchanges = nchanges.wrapping_add(1);
                        }
                    }
                }
            }
            self.nchangestot = self.nchangestot.wrapping_add(nchanges);
            if self.debug != 0 {
                let _ = writeln!(
                    out,
                    "DEMorphBinVol::morph(): pass={}/{}, nchanges = {}, tot = {}",
                    pass + 1,
                    self.nmorph,
                    nchanges,
                    self.nchangestot
                );
            }
        }
        Ok(Some(outvol))
    }

    /// `DEMorphBinVol::morph(binvol)`: `check` (FS-807: its failure stops by default), the dump
    /// with `mm_debug`, the output a copy of frame `frame` ([`DeMorphBinVol::copy_frame`]), then
    /// `nmorph` passes over the interior voxels (1 to extent - 2 on each axis), each on a copy of
    /// the output as the previous pass left it: a voxel that is off (dilation; on for erosion: its
    /// value truncated to an `int` above 0.5) with more than `nnbrsthresh` neighbours on (off) within
    /// `topo` (the sum of the absolute offsets) is set to 1 (0). `nchangestot` counts the changes;
    /// with `mm_debug` `DEMorphBinVol::morph(): pass=<k>/<n>, nchanges = <n>, tot = <n>` after each
    /// pass. FS-804, FS-806.
    pub fn morph(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        binvol: &Mri,
    ) -> Result<Option<Mri>, Fatal> {
        let frame = self.frame;
        self.morph_at(g, out, err, binvol, frame)
    }

    /// The thresholding and matching `morph`s after `bm`'s settings.
    #[allow(clippy::too_many_arguments)]
    fn morph_bin(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        invol: &Mri,
        mask: Option<&Mri>,
        outvol: Option<&Mri>,
        mut bm: BinarizeMri,
    ) -> Result<Option<Mri>, Fatal> {
        if self.check(g, out, Some(invol), mask, outvol) != 0 {
            return Ok(None);
        }
        let copy;
        let involuse: Option<&Mri> = if invol.nframes > 1 {
            let ty = if self.mritype < 0 {
                if invol.mtype != MRI_UCHAR {
                    g.notices.add(
                        &FS_808,
                        "DEMorphBinVol::morph: a frame of a volume of several frames copied into bytes before it is binarized",
                    );
                }
                if g.notices.reproduce() {
                    MRI_UCHAR
                } else {
                    invol.mtype
                }
            } else {
                self.mritype
            };
            copy = self.copy_frame_as(g, out, err, invol, self.frame, ty)?;
            copy.as_ref()
        } else {
            Some(invol)
        };
        bm.debug = self.debug;
        let Some(binvol) = bm.binarize(g, out, err, involuse, mask, None)?.into_new() else {
            return Ok(None);
        };
        // the binarized volume holds the chosen frame only (FS-805)
        let frame = if self.frame != 0 {
            g.notices.add(
                &FS_805,
                format!(
                    "DEMorphBinVol::morph: frame {} selected again from the one-frame binarized volume",
                    self.frame
                ),
            );
            if g.notices.reproduce() { self.frame } else { 0 }
        } else {
            0
        };
        self.morph_at(g, out, err, &binvol, frame)
    }

    /// `DEMorphBinVol::morph(invol, thmin, thmax, mask, outvol)`: `check(invol, mask, outvol)` (its
    /// failure gives `None`), the chosen frame (copied first when the input has several frames,
    /// FS-808) binarized by thresholds ([`BinarizeMri`] type 1 within the mask, the structure's
    /// debugging), then [`DeMorphBinVol::morph`] of the binarized volume (FS-805). `outvol` is
    /// only checked: the result is a new volume.
    #[allow(clippy::too_many_arguments)]
    pub fn morph_thresh(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        invol: &Mri,
        thmin: f64,
        thmax: f64,
        mask: Option<&Mri>,
        outvol: Option<&Mri>,
    ) -> Result<Option<Mri>, Fatal> {
        let bm = BinarizeMri {
            bin_type: 1,
            thmin,
            thmax,
            ..BinarizeMri::default()
        };
        self.morph_bin(g, out, err, invol, mask, outvol, bm)
    }

    /// `DEMorphBinVol::morph(invol, matchlist, mask, outvol)`: as [`DeMorphBinVol::morph_thresh`]
    /// with a match list ([`BinarizeMri`] type 2).
    #[allow(clippy::too_many_arguments)]
    pub fn morph_match(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        invol: &Mri,
        matchlist: &[i32],
        mask: Option<&Mri>,
        outvol: Option<&Mri>,
    ) -> Result<Option<Mri>, Fatal> {
        let bm = BinarizeMri {
            bin_type: 2,
            matchlist: matchlist.to_vec(),
            ..BinarizeMri::default()
        };
        self.morph_bin(g, out, err, invol, mask, outvol, bm)
    }
}
