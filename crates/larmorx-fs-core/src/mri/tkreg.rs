// SPDX-License-Identifier: Apache-2.0
//! The tkregister and FSL matrices (spec §9): the tkregister and FSL vox2ras, a geometry's six
//! matrices, the surface RAS conversions, the registrations between tkregister, native, vox2vox
//! and FSL forms (FS-164, FS-165, FS-331), one-based voxels.

use larmorx_core::defects::{FS_164, FS_165, FS_331};

use super::geom::VolGeom;
use super::types;
use crate::clib::printf::sprintf_f64;
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// Which direction an FSL registration is converted.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FslWay {
    FslToTk,
    TkToFsl,
}

impl Process {
    /// `MRIxfmCRS2XYZtkreg` (§9): the scanner matrix (base 0, no shear) of a geometry of the
    /// same extent and voxel sizes with the coronal cosines and centre 0, computed through a
    /// header of that extent (its shape messages).
    pub fn vox2ras_tkreg(&mut self, g: &VolGeom) -> Result<Matrix, Stop> {
        self.shape_messages(&[g.width, g.height, g.depth]);
        let h = VolGeom {
            width: g.width,
            height: g.height,
            depth: g.depth,
            xsize: g.xsize,
            ysize: g.ysize,
            zsize: g.zsize,
            ..VolGeom::default()
        };
        self.vox2ras_scanner(&h, 0, false)
    }

    /// `MRIxfmCRS2XYZfsl` (§9): the voxel sizes on the diagonal, 1 at (4, 4).
    pub fn vox2ras_fsl(&mut self, g: &VolGeom) -> Matrix {
        let mut m = Matrix::zeros(4, 4, MATRIX_REAL);
        m.set(1, 1, g.xsize);
        m.set(2, 2, g.ysize);
        m.set(3, 3, g.zsize);
        m.set(4, 4, 1.0);
        m
    }

    /// The float product of `specs/freesurfer-maths.md` §8.1 into a new matrix.
    fn chain(&mut self, a: Option<&Matrix>, b: Option<&Matrix>) -> Result<Option<Matrix>, Stop> {
        Ok(match self.matrix_multiply(a, b, None)? {
            Returned::New(m) => Some(m),
            _ => None,
        })
    }

    /// The float product into its running result `d` (a step that does not fit leaves `d`).
    fn chain_into(&mut self, d: &mut Matrix, b: Option<&Matrix>) -> Result<(), Stop> {
        let a = d.clone();
        self.matrix_multiply(Some(&a), b, Some(d))?;
        Ok(())
    }

    /// `RASFromSurfaceRAS_(mri, m)` (§9): tkregister RAS to scanner RAS, `T inv(K)`, into a
    /// given matrix or a new one. FreeSurfer computes `inv(K)` into the given matrix and the
    /// product into it again: a given matrix of another size than 4 x 4 is FS-264 (fewer than 4
    /// rows or columns) or FS-331 (larger: by default its top-left 4 x 4).
    pub fn tkreg_to_scanner(
        &mut self,
        g: &VolGeom,
        given: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        let t = self.vox2ras_scanner(g, 0, false)?;
        let k = self.vox2ras_tkreg(g)?;
        let Some(m) = given else {
            let inv = self.inverse_of(&k)?;
            let r = self.chain(Some(&t), inv.as_ref())?;
            return Ok(r.map_or(Returned::Nothing, Returned::New));
        };
        self.not_4x4("RASFromSurfaceRAS_", m)?;
        let larger = m.rows != 4 || m.cols != 4;
        if larger && !self.mode().reproduce() {
            self.note(
                &FS_331,
                format!(
                    "RASFromSurfaceRAS_ given a {} x {} matrix: the result written into its top-left 4 x 4",
                    m.rows, m.cols
                ),
            );
            let inv = self.inverse_of(&k)?;
            let r = self.chain(Some(&t), inv.as_ref())?;
            let Some(r) = r else {
                return Ok(Returned::Nothing);
            };
            for row in 1..=4 {
                for col in 1..=4 {
                    m.set(row, col, r.at(row, col));
                }
            }
            return Ok(Returned::Output);
        }
        if larger {
            self.note(
                &FS_331,
                format!(
                    "RASFromSurfaceRAS_ given a {} x {} matrix: FreeSurfer's inverse and product into it",
                    m.rows, m.cols
                ),
            );
        }
        let inv = match self.matrix_inverse(Some(&k), Some(m))? {
            Returned::Nothing => None,
            _ => Some(m.clone()),
        };
        let Some(inv) = inv else {
            self.matrix_multiply(Some(&t), None, None)?;
            return Ok(Returned::Nothing);
        };
        Ok(match self.matrix_multiply(Some(&t), Some(&inv), Some(m))? {
            Returned::Nothing => Returned::Nothing,
            _ => Returned::Output,
        })
    }

    /// `surfaceRASFromRAS_(mri)` (§9): scanner RAS to tkregister RAS as `K inv(T)` (measured:
    /// not the inverse of [`Process::tkreg_to_scanner`], whose last bits and signed zeros
    /// differ).
    pub fn scanner_to_tkreg(&mut self, g: &VolGeom) -> Result<Option<Matrix>, Stop> {
        let t = self.vox2ras_scanner(g, 0, false)?;
        let k = self.vox2ras_tkreg(g)?;
        let it = self.inverse_of(&t)?;
        self.chain(Some(&k), it.as_ref())
    }

    /// The volume header a geometry value makes (its extent, sizes, cosines, centre).
    fn header_of(&mut self, g: &VolGeom) -> Result<VolGeom, Stop> {
        let h = self.mri_from_vol_geom(g, types::UCHAR, 1, true)?;
        Ok(h.geom)
    }

    /// `VGtkreg2RAS(vg)` (§9): tkregister RAS to scanner RAS on a header made from the geometry.
    pub fn vg_tkreg_to_ras(&mut self, g: &VolGeom) -> Result<Option<Matrix>, Stop> {
        let h = self.header_of(g)?;
        Ok(match self.tkreg_to_scanner(&h, None)? {
            Returned::New(m) => Some(m),
            _ => None,
        })
    }

    /// `VGras2tkreg(vg, m)` (§9): its inverse, a new matrix (the output argument ignored).
    pub fn vg_ras_to_tkreg(&mut self, g: &VolGeom) -> Result<Option<Matrix>, Stop> {
        let Some(m) = self.vg_tkreg_to_ras(g)? else {
            return Ok(None);
        };
        self.inverse_of(&m)
    }

    /// `MRItkReg2Native(ref, mov, R)` (§9): `inv(T mov inv(K mov) R K ref inv(T ref))`. FS-164
    /// when that inverse fails.
    pub fn tkreg_to_native(
        &mut self,
        rf: &VolGeom,
        mv: &VolGeom,
        r: Option<&Matrix>,
    ) -> Result<Option<Matrix>, Stop> {
        let t_ref = self.vox2ras_scanner(rf, 0, false)?;
        let t_mov = self.vox2ras_scanner(mv, 0, false)?;
        let k_ref = self.vox2ras_tkreg(rf)?;
        let k_mov = self.vox2ras_tkreg(mv)?;
        let ik_mov = self.inverse_of(&k_mov)?;
        let it_ref = self.inverse_of(&t_ref)?;
        let Some(mut d) = self.chain(Some(&t_mov), ik_mov.as_ref())? else {
            return Ok(None);
        };
        if let Some(r) = r {
            self.chain_into(&mut d, Some(r))?;
        }
        self.chain_into(&mut d, Some(&k_ref))?;
        self.chain_into(&mut d, it_ref.as_ref())?;
        let a = d.clone();
        if let Returned::Nothing = self.matrix_inverse(Some(&a), Some(&mut d))? {
            if self.mode().reproduce() {
                self.note(
                    &FS_164,
                    "the registration that cannot be inverted returned uninverted (or as the inverse's infinities)",
                );
                return Ok(Some(d));
            }
            self.note(&FS_164, "the registration that cannot be inverted: refused");
            self.error_exit(
                codes::BAD_PARM,
                b"MRItkReg2Native: the registration cannot be inverted",
            )?;
            return Ok(None);
        }
        Ok(Some(d))
    }

    /// `MRItkRegMtx(ref, mov, D)` (§9): `K mov inv(T mov) inv(D) T ref inv(K ref)`.
    pub fn tkreg_from_native(
        &mut self,
        rf: &VolGeom,
        mv: &VolGeom,
        d: Option<&Matrix>,
    ) -> Result<Option<Matrix>, Stop> {
        let t_ref = self.vox2ras_scanner(rf, 0, false)?;
        let t_mov = self.vox2ras_scanner(mv, 0, false)?;
        let k_ref = self.vox2ras_tkreg(rf)?;
        let k_mov = self.vox2ras_tkreg(mv)?;
        let it_mov = self.inverse_of(&t_mov)?;
        let ik_ref = self.inverse_of(&k_ref)?;
        let Some(mut r) = self.chain(Some(&k_mov), it_mov.as_ref())? else {
            return Ok(None);
        };
        if let Some(d) = d {
            let id = match self.matrix_inverse(Some(d), None)? {
                Returned::New(m) => Some(m),
                _ => None,
            };
            self.chain_into(&mut r, id.as_ref())?;
        }
        self.chain_into(&mut r, Some(&t_ref))?;
        self.chain_into(&mut r, ik_ref.as_ref())?;
        Ok(Some(r))
    }

    /// `MRItkRegMtxFromVox2Vox(ref, mov, V)` (§9): `K mov V inv(K ref)`, or without V
    /// [`Process::tkreg_from_native`] with no D.
    pub fn tkreg_from_vox2vox(
        &mut self,
        rf: &VolGeom,
        mv: &VolGeom,
        v: Option<&Matrix>,
    ) -> Result<Option<Matrix>, Stop> {
        let Some(v) = v else {
            return self.tkreg_from_native(rf, mv, None);
        };
        let k_ref = self.vox2ras_tkreg(rf)?;
        let k_mov = self.vox2ras_tkreg(mv)?;
        let ik_ref = self.inverse_of(&k_ref)?;
        let r = self.chain(Some(&k_mov), Some(v))?;
        let Some(mut r) = r else {
            self.matrix_multiply(None, ik_ref.as_ref(), None)?;
            return Ok(None);
        };
        self.chain_into(&mut r, ik_ref.as_ref())?;
        Ok(Some(r))
    }

    /// `MRIvoxToVoxFromTkRegMtx(mov, targ, R)` (§9): `inv(K mov) R K targ`, or
    /// `inv(K mov) K targ` without R.
    pub fn vox2vox_from_tkreg(
        &mut self,
        mv: &VolGeom,
        targ: &VolGeom,
        r: Option<&Matrix>,
    ) -> Result<Option<Matrix>, Stop> {
        let k_mov = self.vox2ras_tkreg(mv)?;
        let ik_mov = self.inverse_of(&k_mov)?;
        let k_targ = self.vox2ras_tkreg(targ)?;
        match r {
            Some(r) => {
                let Some(mut x) = self.chain(ik_mov.as_ref(), Some(r))? else {
                    self.matrix_multiply(None, Some(&k_targ), None)?;
                    return Ok(None);
                };
                self.chain_into(&mut x, Some(&k_targ))?;
                Ok(Some(x))
            }
            None => self.chain(ik_mov.as_ref(), Some(&k_targ)),
        }
    }

    /// `MRIfixTkReg(mov, R)` (§9): the old tkregister's truncation fitted back over 1,000 points
    /// drawn from the process's `drand48` sequence. FS-165 (both modes) for a registration of
    /// four columns and another row count.
    pub fn fix_tkreg(&mut self, mv: &VolGeom, r: Option<&Matrix>) -> Result<Option<Matrix>, Stop> {
        let k_mov = self.vox2ras_tkreg(mv)?;
        let ik_mov = self.inverse_of(&k_mov)?;
        let n = 1000;
        let mut p = Matrix::zeros(4, n, MATRIX_REAL);
        for j in 1..=n {
            for i in 1..=3 {
                let u = self.drand48_state().drand48();
                p.set(i, j, sse::narrow(256.0 * (u - 0.5)));
            }
            p.set(4, j, 1.0);
        }
        let rp = self.chain(r, Some(&p))?;
        let Some(c) = self.chain(ik_mov.as_ref(), rp.as_ref())? else {
            // a registration of four columns and another row count: FreeSurfer crashes on the
            // product that failed (FS-165, refused in both modes)
            self.note(
                &FS_165,
                "MRIfixTkReg given a registration of four columns and another row count: refused (FreeSurfer crashes)",
            );
            self.error_exit(
                codes::BAD_PARM,
                b"MRIfixTkReg: the registration matrix is not 4 x 4",
            )?;
            return Ok(None);
        };
        let mut t = Matrix::zeros(4, n, MATRIX_REAL);
        for j in 1..=n {
            t.set(1, j, c.at(1, j).floor());
            t.set(2, j, c.at(2, j).ceil());
            t.set(3, j, c.at(3, j).floor());
            t.set(4, j, 1.0);
        }
        let mut pt = Matrix::zeros(n, 4, MATRIX_REAL);
        for j in 1..=n {
            for i in 1..=4 {
                pt.set(j, i, p.at(i, j));
            }
        }
        let tpt = self.chain(Some(&t), Some(&pt))?;
        let ktpt = self.chain(Some(&k_mov), tpt.as_ref())?;
        let ppt = self.chain(Some(&p), Some(&pt))?;
        let ippt = match ppt {
            Some(m) => self.inverse_of(&m)?,
            None => None,
        };
        self.chain(ktpt.as_ref(), ippt.as_ref())
    }

    /// The FSL registrations' checks and left-right flips (§9): the voxel-size diagonals
    /// (`D ref` and `inv(D mov)` from FSL, `D mov` and `D ref` to FSL), flipped for each volume
    /// whose scanner vox2ras has a positive determinant when `FSLOUTPUTTYPE` is NIFTI.
    fn fsl_diagonals(
        &mut self,
        way: FslWay,
        rf: &VolGeom,
        mv: &VolGeom,
    ) -> Result<(Matrix, Matrix), Stop> {
        let diag = |g: &VolGeom, inverse: bool| {
            let mut m = Matrix::zeros(4, 4, MATRIX_REAL);
            for (k, s) in [g.xsize, g.ysize, g.zsize].into_iter().enumerate() {
                let v = if inverse { sse::div32(1.0, s) } else { s };
                m.set(k as i32 + 1, k as i32 + 1, v);
            }
            m.set(4, 4, 1.0);
            m
        };
        let (mut dm, mut dr) = match way {
            FslWay::FslToTk => (diag(mv, true), diag(rf, false)),
            FslWay::TkToFsl => (diag(mv, false), diag(rf, false)),
        };
        let Some(kind) = self.env.get("FSLOUTPUTTYPE").map(<[u8]>::to_vec) else {
            let what = match way {
                FslWay::FslToTk => "FSL registration matrix to FreeSurfer",
                FslWay::TkToFsl => "FreeSurfer registration matrix to FSL",
            };
            self.put_out(
                format!(
                    "ERROR: trying to convert {what},\n       but FSLOUTPUTTYPE variable is not set.\n"
                )
                .as_bytes(),
            );
            self.flush();
            return Err(Stop::Exit(1));
        };
        let mut b = b"FSLOUTPUTTYPE ".to_vec();
        b.extend_from_slice(&kind);
        b.extend_from_slice(b" \n");
        self.put_out(&b);
        if matches!(&kind[..], b"NIFTI" | b"NIFTI_PAIR" | b"NIFTI_GZ") {
            let tm = self.vox2ras_scanner(mv, 0, false)?;
            let tr = self.vox2ras_scanner(rf, 0, false)?;
            let det_m = self.matrix_determinant(&tm);
            let det_r = self.matrix_determinant(&tr);
            let mut b = match way {
                FslWay::FslToTk => b"fsl2TkReg: mov det = ".to_vec(),
                FslWay::TkToFsl => b"tkreg2FSL: mov det = ".to_vec(),
            };
            b.extend_from_slice(&sprintf_f64(b"%g", sse::widen(det_m)));
            b.extend_from_slice(b", ref det = ");
            b.extend_from_slice(&sprintf_f64(b"%g", sse::widen(det_r)));
            b.push(b'\n');
            self.put_out(&b);
            for (which, det, g) in [("Mov", det_m, mv), ("Ref", det_r, rf)] {
                if det.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
                    continue;
                }
                self.put_out(
                    format!(
                        "INFO: FSL2FreeSurfer: {which} volume is NIFTI with positive det,\n      applying LR flip to registration matrix.\n"
                    )
                    .as_bytes(),
                );
                let wm1 = (g.width - 1) as f32;
                let inverse_mov = which == "Mov" && way == FslWay::FslToTk;
                let d = if which == "Mov" { &mut dm } else { &mut dr };
                let v = d.at(1, 1);
                d.set(1, 1, -v);
                d.set(
                    1,
                    4,
                    if inverse_mov {
                        wm1
                    } else {
                        sse::mul32(g.xsize, wm1)
                    },
                );
            }
        }
        Ok((dm, dr))
    }

    /// `MRIfsl2TkReg(ref, mov, M)` (§9): `K mov inv(D mov) inv(M) D ref inv(K ref)`.
    pub fn fsl_to_tkreg(
        &mut self,
        rf: &VolGeom,
        mv: &VolGeom,
        m: Option<&Matrix>,
    ) -> Result<Option<Matrix>, Stop> {
        let (id_mov, d_ref) = self.fsl_diagonals(FslWay::FslToTk, rf, mv)?;
        let k_mov = self.vox2ras_tkreg(mv)?;
        let k_ref = self.vox2ras_tkreg(rf)?;
        let ik_ref = self.inverse_of(&k_ref)?;
        let im = match self.matrix_inverse(m, None)? {
            Returned::New(x) => Some(x),
            _ => None,
        };
        let x = self.chain(Some(&k_mov), Some(&id_mov))?;
        let x = self.chain(x.as_ref(), im.as_ref())?;
        let x = self.chain(x.as_ref(), Some(&d_ref))?;
        self.chain(x.as_ref(), ik_ref.as_ref())
    }

    /// `MRItkreg2FSL(ref, mov, R)` (§9): `inv(D mov inv(K mov) R K ref inv(D ref))`, absent
    /// when that inverse fails.
    pub fn tkreg_to_fsl(
        &mut self,
        rf: &VolGeom,
        mv: &VolGeom,
        r: Option<&Matrix>,
    ) -> Result<Option<Matrix>, Stop> {
        let (d_mov, d_ref) = self.fsl_diagonals(FslWay::TkToFsl, rf, mv)?;
        let id_ref = self.inverse_of(&d_ref)?;
        let k_mov = self.vox2ras_tkreg(mv)?;
        let ik_mov = self.inverse_of(&k_mov)?;
        let k_ref = self.vox2ras_tkreg(rf)?;
        // each product into the running result, which takes its value (none after a step that
        // does not fit)
        let mut x = self.chain(Some(&d_mov), ik_mov.as_ref())?;
        for b in [r, Some(&k_ref), id_ref.as_ref()] {
            x = match x {
                Some(mut a) => {
                    let a0 = a.clone();
                    match self.matrix_multiply(Some(&a0), b, Some(&mut a))? {
                        Returned::Nothing => None,
                        _ => Some(a),
                    }
                }
                None => self.chain(None, b)?,
            };
        }
        let Some(mut x) = x else {
            return Ok(None);
        };
        let a = x.clone();
        Ok(match self.matrix_inverse(Some(&a), Some(&mut x))? {
            Returned::Nothing => None,
            _ => Some(x),
        })
    }

    /// `MtxCRS1toCRS0(Q)` (§9): the identity with 1 in the fourth column's first three rows,
    /// into Q (a new matrix when none); Q not 4 x 4: the error line and nothing.
    pub fn crs1_to_crs0(&mut self, q: Option<&mut Matrix>) -> Returned<Matrix> {
        let fill = |m: &mut Matrix| {
            for r in 1..=4 {
                for c in 1..=4 {
                    m.set(
                        r,
                        c,
                        if r == c || (c == 4 && r < 4) {
                            1.0
                        } else {
                            0.0
                        },
                    );
                }
            }
        };
        match q {
            None => {
                let mut m = Matrix::zeros(4, 4, MATRIX_REAL);
                fill(&mut m);
                Returned::New(m)
            }
            Some(q) if q.rows == 4 && q.cols == 4 => {
                fill(q);
                Returned::Output
            }
            Some(_) => {
                self.put_out(b"ERROR: MtxCRS1toCRS0(): input matrix is not 4x4\n");
                Returned::Nothing
            }
        }
    }
}
