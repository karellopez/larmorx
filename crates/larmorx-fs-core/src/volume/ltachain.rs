// SPDX-License-Identifier: Apache-2.0
//! The operations on a transform array beyond its type (spec §54): `LTAinvert`, `LTAconcat` and
//! `LTAconcat2`, `LTAreduce`, `LTAmriIsSource` and `LTAmriIsTarget`, `LTAprint`; FS-504's ends
//! where FreeSurfer reads a transform an array does not hold.

use super::vgcopy::VG_IS_EQUAL_THRESHOLD;
use crate::clib::printf::sprintf_f64;
use crate::maths::Returned;
use crate::mri::lta::lta_type;
use crate::mri::{LinearTransform, Lta, Mri, VolGeom};
use crate::process::{Process, Stop};
use crate::sse;

/// Where `LTAinvert` puts its result (§54): a new array, another array of the same size (copied
/// into), or the array itself.
pub enum LtaInto<'a> {
    New,
    Given(&'a mut Lta),
    Itself,
}

/// The subject's name up to its first NUL.
fn subject_of(lta: &Lta) -> &[u8] {
    let end = lta
        .subject
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(lta.subject.len());
    &lta.subject[..end]
}

impl Process {
    /// The array's inverses filled (`LTAfillInverse`), the order of the transforms reversed in
    /// both lists and the two lists exchanged: the inverse of the chain (§54).
    fn lta_invert_in_place(&mut self, a: &mut Lta) -> Result<(), Stop> {
        self.lta_fill_inverse(a)?;
        a.xforms.reverse();
        a.inv_xforms.reverse();
        std::mem::swap(&mut a.xforms, &mut a.inv_xforms);
        Ok(())
    }

    /// `LTAinvert(lta, ltainv)` (§54): into a new array (`LTAcopy`), a given array of the same
    /// size (`LTAcopy` into it: another size prints its error and gives nothing, the given array
    /// unchanged) or the array itself; then the inverse of the chain. A transform that cannot be
    /// inverted ends the program in the fill.
    pub fn lta_invert(&mut self, lta: &mut Lta, into: LtaInto<'_>) -> Result<Returned<Lta>, Stop> {
        match into {
            LtaInto::New => match self.lta_copy(lta, None)? {
                Returned::New(mut c) => {
                    self.lta_invert_in_place(&mut c)?;
                    Ok(Returned::New(c))
                }
                _ => Ok(Returned::Nothing),
            },
            LtaInto::Given(d) => match self.lta_copy(lta, Some(d))? {
                Returned::Nothing => Ok(Returned::Nothing),
                _ => {
                    self.lta_invert_in_place(d)?;
                    Ok(Returned::Output)
                }
            },
            LtaInto::Itself => {
                self.lta_invert_in_place(lta)?;
                Ok(Returned::Output)
            }
        }
    }

    /// `LTAprint`'s lines of the mismatch report (`LTA <k> -------------------` and the array).
    fn lta_mismatch_block(&mut self, k: usize, lta: &Lta) -> Result<(), Stop> {
        self.put_out(format!("LTA {k} -------------------\n").as_bytes());
        self.lta_print(lta)?;
        Ok(())
    }

    /// `LTAconcat(ltas, n, Reduce)` (§54): every array's inverses filled first; a new array of all
    /// their transforms (`LTcopy` into it): the first array's, its type, `fscale` and subject
    /// taken; each later array changed to that type when it has another, then its transforms
    /// copied when its first source equals the last copied destination, else its inverse
    /// transforms in reverse order when its last destination does (a warning), else the mismatch
    /// report and none. With `reduce`, `LTAreduce` of the new array. FS-504: no array, or an array
    /// without transforms where a transform is read, ends the program.
    pub fn lta_concat(&mut self, ltas: &mut [Lta], reduce: bool) -> Result<Option<Lta>, Stop> {
        if ltas.is_empty() {
            self.lta_fs504("LTAconcat", "no transform arrays")?;
            return Ok(None);
        }
        for a in ltas.iter_mut() {
            self.lta_fill_inverse(a)?;
        }
        let total: usize = ltas.iter().map(|a| a.xforms.len()).sum();
        let mut out = self.lta_alloc(total as i32, None)?;
        out.kind = ltas[0].kind;
        out.fscale = ltas[0].fscale;
        out.subject = subject_of(&ltas[0]).to_vec();
        let mut n = 0usize;
        for t in &ltas[0].xforms {
            self.lt_copy(t, Some(&mut out.xforms[n]));
            n += 1;
        }
        for k in 1..ltas.len() {
            if ltas[k].kind != out.kind {
                let kind = out.kind;
                self.lta_change_type(&mut ltas[k], kind)?;
            }
            if n == 0 || ltas[k].xforms.is_empty() {
                self.lta_fs504("LTAconcat", "a transform array without transforms")?;
                return Ok(None);
            }
            let last = out.xforms[n - 1].dst.clone();
            let a = &ltas[k];
            let forward = self.vg_is_equal(&a.xforms[0].src, &last, VG_IS_EQUAL_THRESHOLD) != 0;
            let backward = !forward
                && self.vg_is_equal(
                    &a.xforms[a.xforms.len() - 1].dst,
                    &last,
                    VG_IS_EQUAL_THRESHOLD,
                ) != 0;
            if forward {
                for i in 0..a.xforms.len() {
                    self.lt_copy(&ltas[k].xforms[i], Some(&mut out.xforms[n]));
                    n += 1;
                }
            } else if backward {
                self.put_out(
                    format!("WARNING: LTAconcat(): inverting LTA {k} to match geometry\n")
                        .as_bytes(),
                );
                for i in (0..a.inv_xforms.len()).rev() {
                    self.lt_copy(&ltas[k].inv_xforms[i], Some(&mut out.xforms[n]));
                    n += 1;
                }
            } else {
                self.put_out(
                    format!("ERROR: LTAconcat(): LTAs {} and {k} do not match\n", k - 1).as_bytes(),
                );
                let (prev, this) = (ltas[k - 1].clone(), ltas[k].clone());
                self.lta_mismatch_block(k - 1, &prev)?;
                self.lta_mismatch_block(k, &this)?;
                self.put_out(b"--------------------------\n");
                return Ok(None);
            }
        }
        if reduce {
            return self.lta_reduce(&out);
        }
        Ok(Some(out))
    }

    /// `LTAconcat2(lta1, lta2, Reduce)` (§54): [`Process::lta_concat`] of the two arrays (both
    /// changed as it changes them).
    pub fn lta_concat2(
        &mut self,
        a: &mut Lta,
        b: &mut Lta,
        reduce: bool,
    ) -> Result<Option<Lta>, Stop> {
        let mut v = [a.clone(), b.clone()];
        let r = self.lta_concat(&mut v, reduce);
        let [x, y] = v;
        *a = x;
        *b = y;
        r
    }

    /// `LTAreduce(lta)` (§54): on a copy with its inverses filled, a new array of one transform
    /// (the copy's type, `fscale` and subject; its first transform copied); each next transform's
    /// matrix (its inverse's when its destination, not its source, meets the chain: a warning;
    /// neither: an error line and none) multiplied in as `M · reduced` into the reduced matrix
    /// itself; the destination the chain's last. FS-504: an array of no transforms ends the
    /// program, after the copy and its fill.
    pub fn lta_reduce(&mut self, lta: &Lta) -> Result<Option<Lta>, Stop> {
        let mut copy = match self.lta_copy(lta, None)? {
            Returned::New(c) => c,
            _ => return Ok(None),
        };
        self.lta_fill_inverse(&mut copy)?;
        if copy.xforms.is_empty() {
            self.lta_fs504("LTAreduce", "a transform array without transforms")?;
            return Ok(None);
        }
        let mut out = self.lta_alloc(1, None)?;
        out.kind = copy.kind;
        out.fscale = copy.fscale;
        out.subject = subject_of(&copy).to_vec();
        self.lt_copy(&copy.xforms[0], Some(&mut out.xforms[0]));
        // the reduced transform's destination: the last transform's (its source when inverted);
        // each next transform is compared with the previous transform's own destination (§54's
        // text; the probe's groups do not tell the two readings apart)
        let mut end: VolGeom = copy.xforms[0].dst.clone();
        for k in 1..copy.xforms.len() {
            let prev = copy.xforms[k - 1].dst.clone();
            let t: &LinearTransform = &copy.xforms[k];
            let m = if self.vg_is_equal(&t.src, &prev, VG_IS_EQUAL_THRESHOLD) != 0 {
                end = t.dst.clone();
                t.m_l.clone()
            } else if self.vg_is_equal(&t.dst, &prev, VG_IS_EQUAL_THRESHOLD) != 0 {
                self.put_out(
                    format!("WARNING: LTAreduce(): inverting LT {k} to match geometry\n")
                        .as_bytes(),
                );
                end = t.src.clone();
                copy.inv_xforms[k].m_l.clone()
            } else {
                self.put_out(
                    format!("ERROR: LTAreduce(): LTs {} and {k} do not match\n", k - 1).as_bytes(),
                );
                return Ok(None);
            };
            let reduced = &mut out.xforms[0].m_l;
            let b = reduced.clone();
            if let Returned::New(x) =
                self.matrix_multiply(m.as_ref(), b.as_ref(), reduced.as_mut())?
            {
                *reduced = Some(x);
            }
        }
        let d = &mut out.xforms[0].dst;
        self.vg_assign(d, &end);
        Ok(Some(out))
    }

    /// The volume's geometry marked valid (`getVolGeom`, §4).
    fn lta_vol_geom(&mut self, mri: &Mri) -> VolGeom {
        let mut g = VolGeom::default();
        self.get_vol_geom(&mri.geom, &mut g);
        g
    }

    /// `LTAmriIsSource(lta, mri)` (§54): 1 when the volume's geometry (marked valid) equals the
    /// first transform's source, else 0. FS-504 without transforms.
    pub fn lta_mri_is_source(&mut self, lta: &Lta, mri: &Mri) -> Result<i32, Stop> {
        let Some(first) = lta.xforms.first() else {
            self.lta_fs504("LTAmriIsSource", "a transform array without transforms")?;
            return Ok(0);
        };
        let g = self.lta_vol_geom(mri);
        Ok(self.vg_is_equal(&g, &first.src, VG_IS_EQUAL_THRESHOLD))
    }

    /// `LTAmriIsTarget(lta, mri)` (§54): 1 when the volume's geometry (marked valid) equals the
    /// last transform's destination, else 0. FS-504 without transforms.
    pub fn lta_mri_is_target(&mut self, lta: &Lta, mri: &Mri) -> Result<i32, Stop> {
        let Some(last) = lta.xforms.last() else {
            self.lta_fs504("LTAmriIsTarget", "a transform array without transforms")?;
            return Ok(0);
        };
        let g = self.lta_vol_geom(mri);
        Ok(self.vg_is_equal(&g, &last.dst, VG_IS_EQUAL_THRESHOLD))
    }

    /// `LTAprint(fp, lta)` (§54) to standard output: the type (with its name for the voxel,
    /// RAS-to-RAS and REGISTER_DAT types), the count, each transform's mean, sigma and matrix
    /// (`%18.15le `), each transform's two geometry blocks, the subject and the scale. FS-504: a
    /// transform whose matrix is smaller than 4 x 4, or absent, ends the program after its
    /// `1 4 4` line. Returns 0.
    pub fn lta_print(&mut self, lta: &Lta) -> Result<i32, Stop> {
        match self.lta_print_text(lta) {
            Ok(b) => {
                self.put_out(&b);
                Ok(0)
            }
            Err(b) => {
                self.put_out(&b);
                self.flush();
                self.lta_fs504("LTAprint", "a transform without a 4 x 4 matrix")?;
                Ok(0)
            }
        }
    }

    /// `LTAprint`'s text ([`Process::lta_print`]); `Err` with the text up to the `1 4 4` line of
    /// a transform whose matrix is smaller than 4 x 4, or absent (FS-504's end follows).
    pub(crate) fn lta_print_text(&mut self, lta: &Lta) -> Result<Vec<u8>, Vec<u8>> {
        let mut b = format!("type      = {} ", lta.kind).into_bytes();
        b.extend_from_slice(match lta.kind {
            lta_type::LINEAR_VOX_TO_VOX => b"# LINEAR_VOX_TO_VOX\n",
            lta_type::LINEAR_RAS_TO_RAS => b"# LINEAR_RAS_TO_RAS\n",
            lta_type::REGISTER_DAT => b"# REGISTER_DAT\n",
            _ => b"\n",
        });
        b.extend_from_slice(format!("nxforms   = {}\n", lta.num_xforms()).as_bytes());
        for t in &lta.xforms {
            b.extend_from_slice(b"mean      = ");
            for (k, v) in [t.x0, t.y0, t.z0].into_iter().enumerate() {
                if k > 0 {
                    b.push(b' ');
                }
                b.extend_from_slice(&sprintf_f64(b"%6.4f", sse::widen(v)));
            }
            b.extend_from_slice(b"\nsigma     = ");
            b.extend_from_slice(&sprintf_f64(b"%6.4f", sse::widen(t.sigma)));
            b.extend_from_slice(b"\n1 4 4\n");
            let m = match &t.m_l {
                Some(m) if m.rows >= 4 && m.cols >= 4 => m,
                _ => return Err(b),
            };
            for r in 1..=4 {
                for c in 1..=4 {
                    b.extend_from_slice(&sprintf_f64(b"%18.15le ", sse::widen(m.at(r, c))));
                }
                b.push(b'\n');
            }
        }
        for t in &lta.xforms {
            b.extend_from_slice(b"src volume info\n");
            b.extend_from_slice(&self.write_vol_geom(&t.src));
            b.extend_from_slice(b"dst volume info\n");
            b.extend_from_slice(&self.write_vol_geom(&t.dst));
        }
        let s = subject_of(lta);
        if !s.is_empty() {
            b.extend_from_slice(b"subject ");
            b.extend_from_slice(s);
            b.push(b'\n');
        }
        if lta.fscale > 0.0 {
            b.extend_from_slice(b"fscale ");
            b.extend_from_slice(&sprintf_f64(b"%f", sse::widen(lta.fscale)));
            b.push(b'\n');
        }
        Ok(b)
    }
}
