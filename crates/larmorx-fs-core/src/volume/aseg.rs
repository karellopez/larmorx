// SPDX-License-Identifier: Apache-2.0
//! Segmentations onto another grid and SPM warps (spec §55): `MRIaseg2vol` with its tissue-type
//! vote (FS-267, FS-509, FS-510), `MRIaseg2volMU` (FS-270), `MRIapplySpmWarp` (FS-511) and the
//! codes of the interpolation and float-to-integer names.

use larmorx_core::defects::{FS_267, FS_270, FS_509, FS_510, FS_511};

use super::ltachain::LtaInto;
use super::sample::cvtt;
use super::{Dst, VolOut};
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix, real_point_f};
use crate::mri::geom::VolGeom;
use crate::mri::lta::lta_type;
use crate::mri::{ColorTable, Lta, Mri, types};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// A volume's extent.
fn extent(m: &Mri) -> (i32, i32, i32) {
    (m.geom.width, m.geom.height, m.geom.depth)
}

/// `(int)round(x)` (half away from zero; NaN and values past `int` the `int` minimum).
fn round_int(x: f32) -> i32 {
    cvtt(f64::from(x).round())
}

/// A label as §55 reads it: `MRIgetVoxVal`'s `float` stored in an `int` (truncated).
fn as_label(v: f32) -> i32 {
    cvtt(f64::from(v))
}

/// The value of a `Returned` matrix, or none.
fn returned(r: Returned<Matrix>) -> Option<Matrix> {
    match r {
        Returned::New(m) => Some(m),
        _ => None,
    }
}

/// One pair of the forward map: the output voxel's index (-1 outside), the label and the
/// segmentation voxel's index.
#[derive(Clone, Copy)]
struct Pair {
    out: i64,
    label: i32,
    seg: i64,
}

/// `float2int_code(name)` (§55): `round` or `rint` 0, `floor` 1 (any case), a name whose first
/// five letters are `tkreg` in any case 2, else -1.
pub fn float2int_code(name: &[u8]) -> i32 {
    let name = name.split(|&b| b == 0).next().unwrap_or(&[]);
    if name.eq_ignore_ascii_case(b"round") || name.eq_ignore_ascii_case(b"rint") {
        0
    } else if name.eq_ignore_ascii_case(b"floor") {
        1
    } else if name.len() >= 5 && name[..5].eq_ignore_ascii_case(b"tkreg") {
        2
    } else {
        -1
    }
}

/// `interpolation_code(name)` (§55): `MRIinterpCode` (fs.mri).
pub fn interpolation_code(name: &[u8]) -> i32 {
    crate::mri::names::interp_code(name)
}

impl Process {
    /// A 4 x 4 matrix times a point, as `MatrixMultiply` into a 4 x 1 matrix (`float`).
    fn aseg_point(&mut self, m: &Matrix, p: [f32; 4]) -> Result<[f32; 4], Stop> {
        if m.rows == 4 && m.cols == 4 && !m.is_complex() {
            return Ok(real_point_f(m, p));
        }
        let mut q = Matrix::zeros(4, 1, MATRIX_REAL);
        for (i, &x) in p.iter().enumerate() {
            q.set(i as i32 + 1, 1, x);
        }
        let mut o = [0f32; 4];
        if let Some(r) = returned(self.matrix_multiply(Some(m), Some(&q), None)?) {
            for (i, x) in o.iter_mut().enumerate().take(r.rows.max(0) as usize) {
                *x = r.at(i as i32 + 1, 1);
            }
        }
        Ok(o)
    }

    /// FS-509's end (§55): the vote reads past FreeSurfer's tables; both modes, status 252, noted.
    fn aseg_fs509(&mut self, why: String) -> Result<(), Stop> {
        self.note(
            &FS_509,
            format!("MRIaseg2vol: {why}: the program ended (FreeSurfer reads past its tables)"),
        );
        self.error_exit(
            codes::BAD_PARM,
            format!("MRIaseg2vol(): {why} (FreeSurfer reads past its tables here)\n").as_bytes(),
        )
    }

    /// The tissue types of a voxel's label runs (label, hits; labels ascending) and their hits
    /// per type (§55), read in every voxel a colour table maps (its warnings for a type of -2;
    /// FS-509 for a label the table does not hold and types of 100 or more).
    fn aseg_types(
        &mut self,
        ct: &ColorTable,
        ntt: usize,
        runs: &[(i32, usize)],
        types: &mut Vec<i32>,
        type_hits: &mut [usize; 100],
    ) -> Result<(), Stop> {
        if ntt > 100 {
            return self.aseg_fs509(format!(
                "a tissue-type table of {ntt} places (more than 100)"
            ));
        }
        if runs.len() > 1000 {
            return self.aseg_fs509(format!(
                "{} labels in one voxel (more than 1000)",
                runs.len()
            ));
        }
        types.clear();
        type_hits.fill(0);
        for &(l, h) in runs {
            let entry = usize::try_from(l)
                .ok()
                .and_then(|i| ct.entries.get(i))
                .and_then(|e| e.as_ref());
            let Some(e) = entry else {
                return self.aseg_fs509(format!("the label {l} is not in the colour table"));
            };
            let t = e.tissue_type;
            if t == -2 {
                let mut b = format!("WARNING: color table entry {l} ").into_bytes();
                b.extend_from_slice(&e.name);
                b.extend_from_slice(b" has not been assigned a tissue type\n");
                self.put_out(&b);
            }
            if t >= 100 {
                return self.aseg_fs509(format!(
                    "the tissue type {t} of the label {l} (100 or more)"
                ));
            }
            if t >= 0 {
                type_hits[t as usize] += h;
            }
            types.push(t);
        }
        Ok(())
    }

    /// The tissue-type vote of a voxel without a clear winner (§55): from the voxel's label runs,
    /// their types and the hits per type, the winning label and its hits (0 when no label has
    /// the winning type).
    fn aseg_vote(
        &mut self,
        ntt: usize,
        runs: &[(i32, usize)],
        types: &[i32],
        type_hits: &[usize; 100],
    ) -> Result<(i32, usize), Stop> {
        let chosen = if ntt == 0 {
            if self.mode().reproduce() {
                self.note(
                    &FS_267,
                    "MRIaseg2vol: a tissue-type table of no places: the vote took its previous vote's type (FreeSurfer's)",
                );
                self.aseg_vote_type
            } else {
                self.note(
                    &FS_267,
                    "MRIaseg2vol: a tissue-type table of no places: the vote took type 0 (FreeSurfer: its previous vote's type)",
                );
                0
            }
        } else {
            let mut best = 0usize;
            for t in 1..ntt {
                if type_hits[t] > type_hits[best] {
                    best = t;
                }
            }
            best as i32
        };
        self.aseg_vote_type = chosen;
        let mut pick: Option<(i32, usize)> = None;
        for (&(l, h), &t) in runs.iter().zip(types) {
            if t == chosen && pick.is_none_or(|(_, bh)| h >= bh) {
                pick = Some((l, h));
            }
        }
        Ok(pick.unwrap_or((runs[0].0, 0)))
    }

    /// `MRIaseg2vol(aseg, tkR, voltemp, fthresh, &volhit, ct)` (§55): the segmentation's labels
    /// onto the template's grid by the most hits (a colour table's tissue-type vote where no
    /// label has half; FS-510's test, FS-267, FS-509), the voxels no label reached mapped back.
    /// Returns the label and hit volumes (`int`, the template's header), or none.
    pub fn mri_aseg2vol(
        &mut self,
        aseg: &Mri,
        tkr: Option<&Matrix>,
        voltemp: &Mri,
        fthresh: f64,
        ct: Option<&ColorTable>,
    ) -> Result<Option<(Mri, Mri)>, Stop> {
        let ntt = match ct {
            Some(c) => match &c.tissue {
                Some(t) => Some(t.entries.len()),
                None => {
                    self.put_out(
                        b"ERROR: MRIaeg2vol: if passing a ct it must have tissue type info\n",
                    );
                    return Ok(None);
                }
            },
            None => None,
        };
        let diag = self.gdiag_no;
        let reproduce = self.mode().reproduce();
        // Va2v = (inv(Kv) tkR) Ka, its inverse Vv2a
        let ka = self.vox2ras_tkreg(&aseg.geom)?;
        let kv = self.vox2ras_tkreg(&voltemp.geom)?;
        let ikv = returned(self.matrix_inverse(Some(&kv), None)?);
        let t = returned(self.matrix_multiply(ikv.as_ref(), tkr, None)?);
        let Some(va2v) = returned(self.matrix_multiply(t.as_ref(), Some(&ka), None)?) else {
            return Ok(None);
        };
        let vv2a = returned(self.matrix_inverse(Some(&va2v), None)?);
        let (wv, hv, dv) = extent(voltemp);
        let mut labels = self.alloc_with_header(voltemp, (wv, hv, dv), types::INT, 1)?;
        let mut hits = self.alloc_with_header(voltemp, (wv, hv, dv), types::INT, 1)?;
        if diag > 0 {
            self.put_out(b"ASeg2Vol: Building LUT\n");
        }
        let (ws, hs, ds) = extent(aseg);
        let mut pairs = Vec::with_capacity((ws.max(0) * hs.max(0) * ds.max(0)) as usize);
        let mut w4 = 0f32;
        let inside = |x: i32, y: i32, z: i32, (w, h, d): (i32, i32, i32)| {
            x >= 0 && x < w && y >= 0 && y < h && z >= 0 && z < d
        };
        for s in 0..ds {
            for r in 0..hs {
                for c in 0..ws {
                    let label = as_label(self.vf(aseg, (c, r, s), 0)?);
                    let q = self.aseg_point(&va2v, [c as f32, r as f32, s as f32, 1.0])?;
                    w4 = q[3];
                    let (oc, or, os) = (round_int(q[0]), round_int(q[1]), round_int(q[2]));
                    let out = if inside(oc, or, os, (wv, hv, dv)) {
                        i64::from(oc)
                            + i64::from(or) * i64::from(wv)
                            + i64::from(os) * i64::from(wv) * i64::from(hv)
                    } else {
                        -1
                    };
                    let seg = i64::from(c)
                        + i64::from(r) * i64::from(ws)
                        + i64::from(s) * i64::from(ws) * i64::from(hs);
                    pairs.push(Pair { out, label, seg });
                }
            }
        }
        if diag > 0 {
            self.put_out(b"ASeg2Vol: Sorting \n");
        }
        // glibc's merge sort on a consistent comparator: a stable order
        pairs.sort_by_key(|a| (a.out, a.label));
        if diag > 0 {
            self.put_out(b"ASeg2Vol: Mapping\n");
        }
        let mut k = 0;
        let mut runs: Vec<(i32, usize)> = Vec::new();
        let mut types: Vec<i32> = Vec::new();
        let mut type_hits = [0usize; 100];
        while k < pairs.len() {
            let idx = pairs[k].out;
            let mut e = k;
            while e < pairs.len() && pairs[e].out == idx {
                e += 1;
            }
            if idx < 0 {
                k = e;
                continue;
            }
            let total = e - k;
            runs.clear();
            for p in &pairs[k..e] {
                match runs.last_mut() {
                    Some(last) if last.0 == p.label => last.1 += 1,
                    _ => runs.push((p.label, 1)),
                }
            }
            let (mut label, mut nhits) = runs[0];
            for &(l, h) in &runs[1..] {
                if h > nhits {
                    (label, nhits) = (l, h);
                }
            }
            if let (Some(ct), Some(ntt)) = (ct, ntt) {
                self.aseg_types(ct, ntt, &runs, &mut types, &mut type_hits)?;
                let below = |h: usize| sse::div32(h as f32, total as f32) < 0.5;
                let highest = runs[runs.len() - 1];
                let (by_winner, by_highest) = (below(nhits), below(highest.1));
                if by_winner != by_highest {
                    if reproduce {
                        self.note(
                            &FS_510,
                            "MRIaseg2vol: the clear winner decided from the highest label's hits (FreeSurfer's)",
                        );
                    } else {
                        self.note(
                            &FS_510,
                            "MRIaseg2vol: the clear winner decided from the winning label's hits (FreeSurfer tests the highest label's)",
                        );
                    }
                }
                if (reproduce && by_highest) || (!reproduce && by_winner) {
                    (label, nhits) = self.aseg_vote(ntt, &runs, &types, &type_hits)?;
                }
            }
            let written = if (nhits as f64 / total as f64) < fthresh {
                0
            } else {
                label
            };
            let (oc, or, os) = (
                (idx % i64::from(wv)) as i32,
                ((idx / i64::from(wv)) % i64::from(hv)) as i32,
                (idx / (i64::from(wv) * i64::from(hv))) as i32,
            );
            self.mri_set_vox_val(&mut labels, oc, or, os, 0, written as f32, 0)?;
            self.mri_set_vox_val(&mut hits, oc, or, os, 0, nhits as f32, 0)?;
            if diag > 1 {
                self.put_out(
                    format!(
                        "{:6} {:6} {:5}    {:6}    {:3} {:3} {:3}     {:3} {:3}\n",
                        k, pairs[k].seg, written, idx, oc, or, os, total, nhits
                    )
                    .as_bytes(),
                );
            }
            k = e;
        }
        if diag > 0 {
            self.put_out(b"ASeg2Vol: Reverse Map\n");
        }
        let (mut nmiss, mut nfilled) = (0i64, 0i64);
        for s in 0..dv {
            for r in 0..hv {
                for c in 0..wv {
                    if self.vf(&hits, (c, r, s), 0)? != 0.0 {
                        continue;
                    }
                    nmiss += 1;
                    let Some(back) = vv2a.as_ref() else {
                        // the reverse map's first product: m1 is null
                        let q = Matrix::zeros(4, 1, MATRIX_REAL);
                        self.matrix_multiply(None, Some(&q), None)?;
                        return Ok(None);
                    };
                    let q = self.aseg_point(back, [c as f32, r as f32, s as f32, w4])?;
                    let (ac, ar, as_) = (round_int(q[0]), round_int(q[1]), round_int(q[2]));
                    if inside(ac, ar, as_, (ws, hs, ds)) {
                        let l = as_label(self.vf(aseg, (ac, ar, as_), 0)?);
                        self.mri_set_vox_val(&mut labels, c, r, s, 0, l as f32, 0)?;
                        self.mri_set_vox_val(&mut hits, c, r, s, 0, 1.0, 0)?;
                        nfilled += 1;
                    } else {
                        self.mri_set_vox_val(&mut labels, c, r, s, 0, 0.0, 0)?;
                    }
                }
            }
        }
        if diag > 0 {
            self.put_out(format!("nmisses = {nmiss} ({nfilled} filled)\n").as_bytes());
            self.put_out(b"ASeg2Vol: done\n");
        }
        Ok(Some((labels, hits)))
    }

    /// `MRIaseg2volMU(aseg, aseg2vol, fthresh, &volhit, USF, ct)` (§55). `USF` -1: the
    /// segmentation itself and a copy of the array (its inverse when the segmentation is not its
    /// source); any other factor: `MRImaskAndUpsample(aseg, none, USF, 2, 0)` (§41) and the
    /// inverse of its array concatenated before the caller's and reduced (FS-270: a segmentation
    /// at neither end ends the program). The result inverted when the segmentation used is its
    /// source, made `REGISTER_DAT`; `MRIaseg2vol` onto a header (`int`, one frame, the
    /// segmentation's pulse parameters) of the caller's first transform's destination when the
    /// segmentation equals its source, else its source.
    pub fn mri_aseg2vol_mu(
        &mut self,
        aseg: &mut Mri,
        lta: &mut Lta,
        fthresh: f64,
        usf: i32,
        ct: Option<&ColorTable>,
    ) -> Result<Option<(Mri, Mri)>, Stop> {
        let (cut, mut l) = if usf == -1 {
            let l = if self.lta_mri_is_source(lta, aseg)? != 0 {
                match self.lta_copy(lta, None)? {
                    Returned::New(x) => x,
                    _ => return Ok(None),
                }
            } else {
                match self.lta_invert(lta, LtaInto::New)? {
                    Returned::New(x) => x,
                    _ => return Ok(None),
                }
            };
            (None, l)
        } else {
            let Some((cut, mut to_cut)) = self.mri_mask_and_upsample(aseg, None, usf, 2, false)?
            else {
                return Ok(None);
            };
            let mut back = match self.lta_invert(&mut to_cut, LtaInto::New)? {
                Returned::New(x) => x,
                _ => return Ok(None),
            };
            let Some(r) = self.lta_concat2(&mut back, lta, true)? else {
                self.note(
                    &FS_270,
                    "MRIaseg2volMU: a segmentation at neither end of the registration: the program ended (FreeSurfer crashes)",
                );
                self.error_exit(
                    codes::BAD_PARM,
                    b"MRIaseg2volMU: the segmentation is at neither end of the registration (FreeSurfer crashes here)\n",
                )?;
                return Ok(None);
            };
            (Some(cut), r)
        };
        let used: &Mri = cut.as_ref().unwrap_or(&*aseg);
        if self.lta_mri_is_source(&l, used)? != 0 {
            self.lta_invert(&mut l, LtaInto::Itself)?;
        }
        self.lta_change_type(&mut l, lta_type::REGISTER_DAT)?;
        let m = l.xforms.first().and_then(|x| x.m_l.clone());
        // the template: the caller's first transform's destination when the segmentation
        // equals its source, else its source
        let at_src = self.lta_mri_is_source(lta, aseg)? != 0;
        let Some(first) = lta.xforms.first() else {
            return Ok(None);
        };
        let g = if at_src {
            first.dst.clone()
        } else {
            first.src.clone()
        };
        let used: &Mri = cut.as_ref().unwrap_or(&*aseg);
        let mut t = self.mri_alloc_header(g.width, g.height, g.depth, types::INT, 1)?;
        self.use_vol_geom(&g, &mut t)?;
        self.copy_pulse_parameters(used, &mut t);
        self.mri_aseg2vol(used, m.as_ref(), &t, fthresh, ct)
    }

    /// `VGgetVoxelToRasXform(vg, base)` (§55): direction cosines times sizes, the translation
    /// `c - M (n/2 + base)` in `float`.
    fn vg_vox2ras_base(&mut self, g: &VolGeom, base: i32) -> Matrix {
        let mut m = Matrix::zeros(4, 4, MATRIX_REAL);
        let p: [f32; 3] = [0, 1, 2].map(|j| sse::add32(g.extent(j) as f32 / 2.0, base as f32));
        for r in 0..3 {
            let mut t = 0f32;
            for (j, &pj) in p.iter().enumerate() {
                let v = sse::mul32(g.cosine(j)[r], g.size(j));
                m.set(r as i32 + 1, j as i32 + 1, v);
                t = sse::add32(t, sse::mul32(v, pj));
            }
            m.set(r as i32 + 1, 4, sse::sub32(g.c[r], t));
        }
        m.set(4, 4, 1.0);
        m
    }

    /// A matrix times a point as `MatrixMultiplyD` into a 4 x 1 matrix.
    fn spm_point(&mut self, m: &Matrix, p: [f32; 4]) -> Result<[f32; 4], Stop> {
        let mut q = Matrix::zeros(4, 1, MATRIX_REAL);
        for (i, &x) in p.iter().enumerate() {
            q.set(i as i32 + 1, 1, x);
        }
        let mut o = [0f32; 4];
        if let Some(r) = returned(self.matrix_multiply_d(Some(m), Some(&q), None)?) {
            for (i, x) in o.iter_mut().enumerate().take(r.rows.max(0) as usize) {
                *x = r.at(i as i32 + 1, 1);
            }
        }
        Ok(o)
    }

    /// `MRIapplySpmWarp(vol, srclta, warp, LRRev, interp, out)` (§55): each inner warp voxel's
    /// frames 0 to 2 an RAS point of SPM's input space, to its voxel (1-based, less 1),
    /// reversed left to right with `lrrev`, through the array's voxel matrix; every frame of
    /// the volume sampled there (nearest 0, trilinear 1) into `out` (given, or new: `float`, the
    /// warp's grid and header, the volume's frames and pulse parameters). FS-511.
    pub fn mri_apply_spm_warp(
        &mut self,
        vol: &Mri,
        srclta: Option<&mut Lta>,
        warp: &Mri,
        lrrev: bool,
        interp: i32,
        out: Dst,
    ) -> Result<VolOut, Stop> {
        if interp != 0 && interp != 1 {
            self.put_out(
                format!("ERROR: MRIapplySpmWarp():  sample type = {interp}, must be 0 or 1\n")
                    .as_bytes(),
            );
            return Ok(VolOut::Nothing);
        }
        let mut own = None;
        let mut given = match out {
            Dst::Given(m) => Some(m),
            _ => {
                let mut m =
                    self.alloc_with_header(warp, extent(warp), types::FLOAT, vol.nframes)?;
                self.copy_pulse_parameters(vol, &mut m);
                own = Some(m);
                None
            }
        };
        let (v2r, ncols, mvox) = match srclta {
            Some(l) => {
                if l.kind != lta_type::LINEAR_VOX_TO_VOX {
                    self.put_out(b"MRIapplySpmWarp(): changing Source LTA type to vox2vox\n");
                    self.lta_change_type(l, lta_type::LINEAR_VOX_TO_VOX)?;
                }
                // SPM's input space: the first transform's end that is not the volume (its
                // destination when the volume is its source, the map then the inverse of its
                // matrix; else its source and its matrix); the array itself untouched
                let inverted = self.lta_mri_is_source(l, vol)? != 0;
                let Some(first) = l.xforms.first() else {
                    return Ok(VolOut::Nothing);
                };
                let g = if inverted {
                    first.dst.clone()
                } else {
                    first.src.clone()
                };
                let fw = first.m_l.clone();
                let m = if inverted {
                    self.put_out(b"MRIapplySpmWarp(): inverting Source LTA\n");
                    returned(self.matrix_inverse(fw.as_ref(), None)?)
                } else {
                    fw
                };
                (self.vg_vox2ras_base(&g, 1), g.width, Some(m))
            }
            None => (
                self.vox2ras_scanner(&vol.geom, 1, false)?,
                vol.geom.width,
                None,
            ),
        };
        let inv = returned(self.matrix_inverse(Some(&v2r), None)?);
        let (ww, hw, dw) = extent(warp);
        let (wf, hf, df) = (
            vol.geom.width as f32,
            vol.geom.height as f32,
            vol.geom.depth as f32,
        );
        // the sampler's value, kept across frames and voxels; whether the call wrote one yet
        let mut val = 0f64;
        let mut written = false;
        let dst: &mut Mri = match (&mut own, &mut given) {
            (Some(m), _) => m,
            (None, Some(m)) => m,
            _ => unreachable!(),
        };
        for c in 1..ww - 1 {
            for r in 1..hw - 1 {
                for s in 1..dw - 1 {
                    let ras = [
                        self.vf(warp, (c, r, s), 0)?,
                        self.vf(warp, (c, r, s), 1)?,
                        self.vf(warp, (c, r, s), 2)?,
                    ];
                    let Some(inv) = inv.as_ref() else {
                        let q = Matrix::zeros(4, 1, MATRIX_REAL);
                        self.matrix_multiply_d(None, Some(&q), None)?;
                        return Ok(VolOut::Nothing);
                    };
                    let q = self.spm_point(inv, [ras[0], ras[1], ras[2], 1.0])?;
                    let mut x = sse::sub32(q[0], 1.0);
                    let (mut y, mut z) = (sse::sub32(q[1], 1.0), sse::sub32(q[2], 1.0));
                    if lrrev {
                        x = sse::sub32((ncols - 1) as f32, x);
                    }
                    match mvox.as_ref() {
                        Some(Some(m)) => {
                            let q = self.spm_point(m, [x, y, z, 1.0])?;
                            (x, y, z) = (q[0], q[1], q[2]);
                        }
                        Some(None) => {
                            // the array's matrix, or its inverse, missing: the product's end
                            let q = Matrix::zeros(4, 1, MATRIX_REAL);
                            self.matrix_multiply_d(None, Some(&q), None)?;
                            return Ok(VolOut::Nothing);
                        }
                        None => {}
                    }
                    // below 0, or not below the extent (a NaN is not below it)
                    let outside = |v: f32, n: f32| {
                        v < 0.0 || v.partial_cmp(&n) != Some(std::cmp::Ordering::Less)
                    };
                    if outside(x, wf) || outside(y, hf) || outside(z, df) {
                        continue;
                    }
                    for f in 0..vol.nframes {
                        // a refused trilinear sample writes nothing (the last value stays); the
                        // nearest sampler's refusal writes 0
                        let wrote = self.frame_type_kept(
                            vol,
                            f64::from(x),
                            f64::from(y),
                            f64::from(z),
                            f,
                            interp,
                            &mut val,
                        )?;
                        if wrote {
                            written = true;
                        } else if !written {
                            self.note(
                                &FS_511,
                                "MRIapplySpmWarp: a sample that failed before any succeeded stored 0 (FreeSurfer: a value never written)",
                            );
                            val = 0.0;
                        }
                        let x = sse::narrow(val);
                        self.mri_set_vox_val(dst, c, r, s, f, x, 0)?;
                    }
                }
            }
        }
        Ok(match own {
            Some(m) => VolOut::New(m),
            None => VolOut::Given,
        })
    }
}
