// SPDX-License-Identifier: Apache-2.0
//! Binary volumes (spec §60): `FixSubCortMassHA::FixSCM`, `BinarizeMRI` (`dump`, `qualifies`,
//! `binarize`; FS-803) and `DEMorphBinVol` (`check`, `dump`, `copyFrame` and the three `morph`s;
//! FS-804 to FS-808). The C++ classes are plain structures whose fields the caller sets; their
//! methods are the program's ([`Process`]) taking the structure.

use larmorx_core::defects::{FS_803, FS_804, FS_805, FS_806, FS_807, FS_808};

use super::setops::ext;
use super::{Dst, VolOut};
use crate::clib::printf::sprintf_f64;
use crate::mri::{Mri, types};
use crate::process::{Process, Stop};
use crate::sse;

/// `FixSubCortMassHA`'s fields `FixSCM` reads and the mask it leaves.
#[derive(Debug, Default)]
pub struct FixSubCortMassHa {
    /// `nDilate`: the dilations of the mask.
    pub n_dilate: i32,
    /// The mask `FixSCM` made (`int`, the segmentation's extent).
    pub mask: Option<Mri>,
}

/// `BinarizeMRI`'s fields (§60), with FreeSurfer's defaults.
#[derive(Clone, Debug)]
pub struct BinarizeMri {
    /// 1: thresholds; 2: a match list.
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
    pub nhits: i32,
    pub mritype: i32,
    pub do_abs: i32,
    pub m_debug: i32,
}

impl Default for BinarizeMri {
    fn default() -> BinarizeMri {
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
            m_debug: 0,
        }
    }
}

/// `DEMorphBinVol`'s fields (§60), with FreeSurfer's defaults.
#[derive(Clone, Debug)]
pub struct DeMorphBinVol {
    /// 1: dilation; 2: erosion.
    pub morphtype: i32,
    /// The neighbours within this sum of absolute offsets (1 faces, 2 edges, 3 corners).
    pub topo: i32,
    pub nnbrsthresh: i32,
    pub nmorph: i32,
    pub frame: i32,
    pub nchangestot: i32,
    pub mm_mritype: i32,
    pub mm_debug: i32,
}

impl Default for DeMorphBinVol {
    fn default() -> DeMorphBinVol {
        DeMorphBinVol {
            morphtype: 1,
            topo: 1,
            nnbrsthresh: 0,
            nmorph: 0,
            frame: 0,
            nchangestot: 0,
            mm_mritype: -1,
            mm_debug: 1,
        }
    }
}

/// `%g` of a double.
fn g(v: f64) -> String {
    String::from_utf8_lossy(&sprintf_f64(b"%g", v)).into_owned()
}

/// Whether two volumes differ in width, height or depth.
fn dims_differ(a: &Mri, b: &Mri) -> bool {
    Process::mri_dim_mismatch(a, b, false) != 0
}

impl Process {
    /// `FixSubCortMassHA::FixSCM()` (§60): a mask of white matter, cortex and background (labels
    /// 0, 2, 3, 41, 42) dilated `nDilate` times; the subcortical mass set to 0 at the amygdala
    /// and inferior lateral ventricle voxels, and at the hippocampus voxels outside the mask.
    /// Returns 0; the mask stays in the structure.
    pub fn fix_scm(
        &mut self,
        f: &mut FixSubCortMassHa,
        scm: &mut Mri,
        aseg: &Mri,
    ) -> Result<i32, Stop> {
        let (w, h, d) = ext(aseg);
        let mut mask = self.mri_alloc_sequence(w, h, d, types::INT, 1)?;
        self.mri_copy_header(aseg, Some(&mut mask))?;
        self.copy_pulse_parameters(aseg, &mut mask);
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let v = self.mri_get_vox_val(aseg, c, r, s, 0, 0)?;
                    let l = self.int_fs12(sse::widen(v));
                    if matches!(l, 0 | 2 | 3 | 41 | 42) {
                        self.mri_set_vox_val(&mut mask, c, r, s, 0, 1.0, 0)?;
                    }
                }
            }
        }
        self.put_out(format!("Dilating {} voxels in 3d\n", f.n_dilate).as_bytes());
        for _ in 0..f.n_dilate {
            self.mri_dilate(&mut mask, Dst::Src)?;
        }
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let v = self.mri_get_vox_val(aseg, c, r, s, 0, 0)?;
                    let l = self.int_fs12(sse::widen(v));
                    if matches!(l, 18 | 54 | 5 | 44) {
                        self.mri_set_vox_val(scm, c, r, s, 0, 0.0, 0)?;
                    }
                    if matches!(l, 17 | 53) {
                        let m = self.mri_get_vox_val(&mask, c, r, s, 0, 0)?;
                        if !(m > 0.5) {
                            self.mri_set_vox_val(scm, c, r, s, 0, 0.0, 0)?;
                        }
                    }
                }
            }
        }
        f.mask = Some(mask);
        Ok(0)
    }

    /// `BinarizeMRI::dump(invol, mask, outvol)` (§60): its fields on standard output; returns 0.
    pub fn binarize_dump(&mut self, b: &BinarizeMri, mask: Option<&Mri>) -> i32 {
        let mut s = format!("BinarizeMRI: BinType={} ", b.bin_type);
        if b.bin_type == 1 {
            s.push_str(&format!(" thmin={} thmax={} ", g(b.thmin), g(b.thmax)));
        } else if b.bin_type == 2 {
            s.push_str(&format!("match N={} ", b.matchlist.len()));
            for m in &b.matchlist {
                s.push_str(&format!("{m} "));
            }
        }
        if mask.is_some() {
            s.push_str(&format!(
                "maskthmin={} maskthmax={} ",
                g(b.maskthmin),
                g(b.maskthmax)
            ));
        }
        s.push_str(&format!(
            "zce={} zre={} zse={} ",
            b.zero_col_edges, b.zero_row_edges, b.zero_slice_edges
        ));
        s.push_str(&format!("on={} off={} ", g(b.on_val), g(b.off_val)));
        s.push_str(&format!("fstart={} fend={} ", b.fstart, b.fend));
        s.push_str(&format!("invert={} ", b.invert));
        s.push_str(&format!("DoAbs={} ", b.do_abs));
        s.push_str(&format!("mritype={} ", b.mritype));
        s.push('\n');
        if b.bin_type == 1 && !b.matchlist.is_empty() {
            s.push_str(&format!(
                "WARNING: BinType=1 but matchlist not empty N={}\n",
                b.matchlist.len()
            ));
        }
        self.put_out(s.as_bytes());
        0
    }

    /// `BinarizeMRI::qualifies(invol, mask, c, r, s, f)` (§60): 0 outside the mask or on an
    /// edge being zeroed; 1 for `f < 0`; else whether the voxel's value (of frame `f`, its
    /// absolute value with `DoAbs`) lies within the thresholds (type 1) or equals a value of
    /// the match list.
    #[allow(clippy::too_many_arguments)]
    pub fn binarize_qualifies(
        &mut self,
        b: &BinarizeMri,
        invol: &Mri,
        mask: Option<&Mri>,
        c: i32,
        r: i32,
        s: i32,
        f: i32,
    ) -> Result<i32, Stop> {
        if let Some(m) = mask {
            let mv = f64::from(self.mri_get_vox_val(m, c, r, s, 0, 0)?);
            if mv < b.maskthmin || mv > b.maskthmax {
                return Ok(0);
            }
        }
        let (w, h, d) = ext(invol);
        if b.zero_col_edges != 0 && (c == 0 || c == w.wrapping_sub(1)) {
            return Ok(0);
        }
        if b.zero_row_edges != 0 && (r == 0 || r == h.wrapping_sub(1)) {
            return Ok(0);
        }
        if b.zero_slice_edges != 0 && (s == 0 || s == d.wrapping_sub(1)) {
            return Ok(0);
        }
        if f < 0 {
            return Ok(1);
        }
        let mut v = f64::from(self.mri_get_vox_val(invol, c, r, s, f, 0)?);
        if b.do_abs != 0 {
            v = v.abs();
        }
        if b.bin_type == 1 {
            if v < b.thmin || v > b.thmax {
                return Ok(0);
            }
            return Ok(1);
        }
        Ok(i32::from(b.matchlist.iter().any(|&m| v == f64::from(m))))
    }

    /// `BinarizeMRI::binarize(invol, mask, outvol)` (§60): the messages and refusals, the frame
    /// range stored, then each voxel's frames on or off into the output (FS-803's frames of
    /// the voxels outside the mask or on a zeroed edge); `nhits` stored.
    pub fn binarize(
        &mut self,
        b: &mut BinarizeMri,
        invol: Option<&mut Mri>,
        mask: Option<&Mri>,
        outvol: Dst,
    ) -> Result<VolOut, Stop> {
        if b.m_debug != 0 {
            self.binarize_dump(b, mask);
        }
        let Some(invol) = invol else {
            self.put_out(b"ERROR: BinarizeMRI::binarize(): input is NULL\n");
            return Ok(VolOut::Nothing);
        };
        if b.bin_type == 1 && !b.matchlist.is_empty() {
            self.put_out(
                format!(
                    "WARNING: BinarizeMRI::binarize(): BinType=1 but matchlist not empty N={}\n",
                    b.matchlist.len()
                )
                .as_bytes(),
            );
            self.binarize_dump(b, mask);
        }
        if b.fstart < 0 {
            b.fstart = 0;
        }
        if b.fend < 0 {
            b.fend = invol.nframes;
        }
        if b.fend > invol.nframes {
            self.put_out(
                format!(
                    "ERROR: BinarizeMRI::binarize(): fend={} >= nframes={}\n",
                    b.fend, invol.nframes
                )
                .as_bytes(),
            );
            self.binarize_dump(b, mask);
            return Ok(VolOut::Nothing);
        }
        if let Some(m) = mask
            && dims_differ(invol, m)
        {
            self.put_out(b"ERROR: BinarizeMRI::binarize(): input/mask dim mismatch\n");
            return Ok(VolOut::Nothing);
        }
        let is_self = matches!(outvol, Dst::Src);
        let mut given = match outvol {
            Dst::Given(o) => Some(o),
            _ => None,
        };
        if let Some(o) = &given
            && dims_differ(invol, o)
        {
            self.put_out(b"ERROR: BinarizeMRI::binarize(): input/output dim mismatch\n");
            self.binarize_dump(b, mask);
            return Ok(VolOut::Nothing);
        }
        let mut own = None;
        if given.is_none() && !is_self {
            let (w, h, d) = ext(invol);
            let kind = if b.mritype == -1 {
                types::FLOAT
            } else {
                b.mritype
            };
            let mut o =
                self.mri_alloc_sequence(w, h, d, kind, b.fend.wrapping_sub(b.fstart))?;
            self.mri_copy_header(invol, Some(&mut o))?;
            self.copy_pulse_parameters(invol, &mut o);
            own = Some(o);
        }
        if b.invert != 0 {
            std::mem::swap(&mut b.on_val, &mut b.off_val);
        }
        // the dump again with `m_debug`, the values swapped (observed: also without `invert`,
        // `vrules24`)
        if b.m_debug != 0 {
            self.binarize_dump(b, mask);
        }
        let (on, off) = (b.on_val as f32, b.off_val as f32);
        let (w, h, d) = ext(invol);
        let reproduce = self.mode().reproduce();
        let mut nhits = 0i32;
        // the input itself as the output: read before it is written, voxel by voxel
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let inside = self.binarize_qualifies(b, invol, mask, c, r, s, -1)? != 0;
                    for f in b.fstart..b.fend {
                        let (fo, v) = if !inside {
                            let fo = if reproduce { f } else { f - b.fstart };
                            if b.fstart > 0 {
                                self.note(
                                    &FS_803,
                                    if reproduce {
                                        "BinarizeMRI: the off value of a voxel outside the mask written into the input's frame number"
                                    } else {
                                        "BinarizeMRI: the off value of a voxel outside the mask written into its output frame (FreeSurfer writes the input's frame number)"
                                    },
                                );
                            }
                            (fo, off)
                        } else if self.binarize_qualifies(b, invol, None, c, r, s, f)? != 0 {
                            nhits = nhits.wrapping_add(1);
                            (f - b.fstart, on)
                        } else {
                            (f - b.fstart, off)
                        };
                        let out: &mut Mri = if is_self {
                            &mut *invol
                        } else if let Some(o) = given.as_deref_mut() {
                            o
                        } else {
                            own.as_mut().expect("a new output")
                        };
                        self.mri_set_vox_val(out, c, r, s, fo, v, 0)?;
                    }
                }
            }
        }
        if b.invert != 0 {
            let n = w
                .wrapping_mul(h)
                .wrapping_mul(d)
                .wrapping_mul(b.fend.wrapping_sub(b.fstart));
            nhits = n.wrapping_sub(nhits);
            std::mem::swap(&mut b.on_val, &mut b.off_val);
        }
        b.nhits = nhits;
        if b.m_debug != 0 {
            self.put_out(format!("BinarizeMRI: nhits = {}\n", b.nhits).as_bytes());
        }
        Ok(if is_self {
            VolOut::Src
        } else if given.is_some() {
            VolOut::Given
        } else {
            VolOut::New(own.expect("a new output"))
        })
    }

    /// Whether `frame` is out of range for a volume of `nframes` (§60: below 0 or past the
    /// frames; FS-806: FreeSurfer accepts one equal to the number of frames).
    fn dem_frame_out(&mut self, frame: i32, nframes: i32, what: &str) -> bool {
        if frame < 0 || frame > nframes {
            return true;
        }
        if frame == nframes {
            if self.mode().reproduce() {
                self.note(
                    &FS_806,
                    format!("{what}: a frame equal to the number of frames accepted (read past the volume)"),
                );
                return false;
            }
            self.note(
                &FS_806,
                format!("{what}: a frame equal to the number of frames refused (FreeSurfer accepts it)"),
            );
            return true;
        }
        false
    }

    /// `DEMorphBinVol::check(invol, ubermask, outvol)` (§60): 1 after the first error, else 0.
    pub fn dem_check(
        &mut self,
        d: &DeMorphBinVol,
        invol: Option<&Mri>,
        ubermask: Option<&Mri>,
        outvol: Option<&Mri>,
    ) -> i32 {
        let Some(i) = invol else {
            self.put_out(b"ERROR: DEMorphBinVol::check(): input is NULL\n");
            return 1;
        };
        if let Some(m) = ubermask
            && dims_differ(i, m)
        {
            self.put_out(b"ERROR: DEMorphBinVol(): input/ubermask dim mismatch\n");
            return 1;
        }
        if let Some(o) = outvol
            && dims_differ(i, o)
        {
            self.put_out(b"ERROR: DEMorphBinVol::check(): input/output dim mismatch\n");
            return 1;
        }
        if self.dem_frame_out(d.frame, i.nframes, "DEMorphBinVol::check()") {
            self.put_out(
                format!(
                    "ERROR: DEMorphBinVol::check(): frame={}, out of range {}\n",
                    d.frame, i.nframes
                )
                .as_bytes(),
            );
            return 1;
        }
        if d.topo < 1 || d.topo > 3 {
            self.put_out(
                format!(
                    "ERROR: DEMorphBinVol::check(): topo={}, must be 1, 2, or 3\n",
                    d.topo
                )
                .as_bytes(),
            );
            return 1;
        }
        if d.morphtype != 1 && d.morphtype != 2 {
            self.put_out(
                format!(
                    "ERROR: DEMorphBinVol::check(): morphtype={}, must be 1 or 2\n",
                    d.morphtype
                )
                .as_bytes(),
            );
            return 1;
        }
        0
    }

    /// `DEMorphBinVol::dump()` (§60): its fields on standard output; returns 0.
    pub fn dem_dump(&mut self, d: &DeMorphBinVol) -> i32 {
        self.put_out(
            format!(
                "DEMorphBinVol: topo={} morphtype={} nnbrsthresh={} frame={} nmorph={} mritype={} \n",
                d.topo, d.morphtype, d.nnbrsthresh, d.frame, d.nmorph, d.mm_mritype
            )
            .as_bytes(),
        );
        0
    }

    /// `DEMorphBinVol::copyFrame(invol, frameno)` (§60): a new one-frame volume of `mm_mritype`
    /// (bytes for -1) holding that frame; none after the error of a frame out of range.
    pub fn dem_copy_frame(
        &mut self,
        d: &DeMorphBinVol,
        invol: &Mri,
        frameno: i32,
    ) -> Result<Option<Mri>, Stop> {
        let kind = if d.mm_mritype == -1 {
            types::UCHAR
        } else {
            d.mm_mritype
        };
        self.dem_copy_frame_as(invol, frameno, kind)
    }

    fn dem_copy_frame_as(
        &mut self,
        invol: &Mri,
        frameno: i32,
        kind: i32,
    ) -> Result<Option<Mri>, Stop> {
        if self.dem_frame_out(frameno, invol.nframes, "DEMorphBinVol::copyFrame()") {
            self.put_out(
                format!(
                    "ERROR: DEMorphBinVol::copyFrame(): frameno={frameno}, out of range {}\n",
                    invol.nframes
                )
                .as_bytes(),
            );
            return Ok(None);
        }
        let (w, h, dd) = ext(invol);
        let mut o = self.mri_alloc_sequence(w, h, dd, kind, 1)?;
        self.mri_copy_header(invol, Some(&mut o))?;
        self.copy_pulse_parameters(invol, &mut o);
        for c in 0..w {
            for r in 0..h {
                for s in 0..dd {
                    let v = self.mri_get_vox_val(invol, c, r, s, frameno, 0)?;
                    self.mri_set_vox_val(&mut o, c, r, s, 0, v, 0)?;
                }
            }
        }
        Ok(Some(o))
    }

    /// `DEMorphBinVol::morph(binvol)` (§60): `nmorph` dilations or erosions of frame `frame`
    /// (FS-804's own value, FS-807's invalid settings).
    pub fn dem_morph(&mut self, d: &mut DeMorphBinVol, binvol: &Mri) -> Result<Option<Mri>, Stop> {
        if self.dem_check(d, Some(binvol), None, None) != 0 {
            if !self.mode().reproduce() {
                self.note(
                    &FS_807,
                    "DEMorphBinVol::morph(): no result after the check's error (FreeSurfer goes on)",
                );
                return Ok(None);
            }
            self.note(
                &FS_807,
                "DEMorphBinVol::morph(): went on after the check's error",
            );
        }
        if d.mm_debug != 0 {
            self.dem_dump(d);
        }
        let Some(mut out) = self.dem_copy_frame(d, binvol, d.frame)? else {
            return Ok(None);
        };
        let (w, h, dd) = ext(binvol);
        let reproduce = self.mode().reproduce();
        d.nchangestot = 0;
        for pass in 1..=d.nmorph {
            let Some(copy) = self.dem_copy_frame(d, &out, 0)? else {
                return Ok(None);
            };
            let own_frame = if reproduce { d.frame } else { 0 };
            if d.frame != 0 {
                self.note(
                    &FS_804,
                    if reproduce {
                        "DEMorphBinVol::morph(): each voxel's own value read from its frame of the one-frame copy"
                    } else {
                        "DEMorphBinVol::morph(): each voxel's own value read from frame 0 of the one-frame copy (FreeSurfer reads its frame)"
                    },
                );
            }
            let mut nchanges = 0i32;
            for c in 1..w - 1 {
                for r in 1..h - 1 {
                    for s in 1..dd - 1 {
                        let v = self.mri_get_vox_val(&copy, c, r, s, own_frame, 0)?;
                        let on = self.int_fs12(sse::widen(v)) >= 1;
                        let visit = match d.morphtype {
                            1 => !on,
                            2 => on,
                            _ => true,
                        };
                        if !visit {
                            continue;
                        }
                        let mut n = 0i32;
                        if d.morphtype == 1 || d.morphtype == 2 {
                            'count: for dc in -1..=1i32 {
                                for dr in -1..=1i32 {
                                    for ds in -1..=1i32 {
                                        if dc.abs() + dr.abs() + ds.abs() > d.topo {
                                            continue;
                                        }
                                        let x = self.mri_get_vox_val(
                                            &copy,
                                            c + dc,
                                            r + dr,
                                            s + ds,
                                            0,
                                            0,
                                        )?;
                                        let xon = self.int_fs12(sse::widen(x)) >= 1;
                                        if (d.morphtype == 1) == xon {
                                            n += 1;
                                            if n > d.nnbrsthresh {
                                                break 'count;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if n > d.nnbrsthresh {
                            let val = if d.morphtype == 1 { 1.0 } else { 0.0 };
                            self.mri_set_vox_val(&mut out, c, r, s, 0, val, 0)?;
                            nchanges = nchanges.wrapping_add(1);
                        }
                    }
                }
            }
            d.nchangestot = d.nchangestot.wrapping_add(nchanges);
            if d.mm_debug != 0 {
                self.put_out(
                    format!(
                        "DEMorphBinVol::morph(): pass={pass}/{}, nchanges = {nchanges}, tot = {}\n",
                        d.nmorph, d.nchangestot
                    )
                    .as_bytes(),
                );
            }
        }
        Ok(Some(out))
    }

    /// `DEMorphBinVol::morph(invol, thmin, thmax, mask, outvol)` (`list` none) and
    /// `morph(invol, matchlist, mask, outvol)` (§60): checked; frame `frame` of a volume of
    /// several frames copied (FS-808), binarized within the mask, then [`Process::dem_morph`]
    /// of the result (FS-805).
    #[allow(clippy::too_many_arguments)]
    pub fn dem_morph_binarized(
        &mut self,
        d: &mut DeMorphBinVol,
        invol: &Mri,
        thresholds: (f64, f64),
        list: Option<&[i32]>,
        mask: Option<&Mri>,
        outvol: Option<&Mri>,
    ) -> Result<Option<Mri>, Stop> {
        if self.dem_check(d, Some(invol), mask, outvol) != 0 {
            return Ok(None);
        }
        let reproduce = self.mode().reproduce();
        let mut copied = None;
        if invol.nframes > 1 {
            let kind = if reproduce {
                types::UCHAR
            } else {
                invol.kind
            };
            if invol.kind != types::UCHAR {
                self.note(
                    &FS_808,
                    if reproduce {
                        "DEMorphBinVol::morph(): the frame copied into bytes before it is binarized"
                    } else {
                        "DEMorphBinVol::morph(): the frame copied in the input's type before it is binarized (FreeSurfer copies into bytes)"
                    },
                );
            }
            let Some(c) = self.dem_copy_frame_as(invol, d.frame, kind)? else {
                return Ok(None);
            };
            copied = Some(c);
        }
        let mut b = BinarizeMri {
            m_debug: d.mm_debug,
            ..BinarizeMri::default()
        };
        match list {
            Some(l) => {
                b.bin_type = 2;
                b.matchlist = l.to_vec();
            }
            None => {
                b.bin_type = 1;
                b.thmin = thresholds.0;
                b.thmax = thresholds.1;
            }
        }
        let mut src = match copied {
            Some(c) => c,
            None => invol.clone(),
        };
        let bin = match self.binarize(&mut b, Some(&mut src), mask, Dst::New)? {
            VolOut::New(m) => m,
            _ => return Ok(None),
        };
        if d.frame != 0 {
            self.note(
                &FS_805,
                if reproduce {
                    "DEMorphBinVol::morph(): the frame selected again from the one-frame binarized volume"
                } else {
                    "DEMorphBinVol::morph(): frame 0 of the one-frame binarized volume morphed (FreeSurfer selects the frame again)"
                },
            );
        }
        if reproduce {
            self.dem_morph(d, &bin)
        } else {
            let frame = d.frame;
            d.frame = 0;
            let r = self.dem_morph(d, &bin);
            d.frame = frame;
            r
        }
    }
}
