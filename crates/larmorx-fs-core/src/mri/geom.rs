// SPDX-License-Identifier: Apache-2.0
//! The geometry (`VOL_GEOM`, spec §2), its scanner voxel-to-RAS matrix and cache (§3), its
//! printing and comparisons (§11) and the sphinx position (§9).

use larmorx_core::defects::FS_59;

use crate::clib::printf::sprintf_f64;
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::process::{Process, Stop};
use crate::sse;

/// The diagnostic bit `DIAG_INFO` (§3's shear print).
pub const DIAG_INFO: u64 = 0x1_0000;
/// The diagnostic bit `DIAG_SHOW` (§6's colour-table line, with the verbose bit).
pub const DIAG_SHOW: u64 = 0x40;

/// A volume's geometry (§2): its extent, voxel sizes, direction cosines, centre, shears, file
/// name, and the cached matrices and registration matrix it may carry.
#[derive(Clone, Debug, PartialEq)]
pub struct VolGeom {
    pub valid: i32,
    pub width: i32,
    pub height: i32,
    pub depth: i32,
    pub xsize: f32,
    pub ysize: f32,
    pub zsize: f32,
    pub ras_good_flag: i32,
    /// The direction cosines of columns, rows and slices, `[x_r, x_a, x_s]` and so on.
    pub x: [f32; 3],
    pub y: [f32; 3],
    pub z: [f32; 3],
    /// The centre (`c_r`, `c_a`, `c_s`).
    pub c: [f32; 3],
    /// The shears (`s_r`, `s_a`, `s_s`).
    pub s: [f32; 3],
    /// The file name (at most 1,023 bytes).
    pub fname: Vec<u8>,
    /// The cached voxel-to-RAS matrix, 16 floats column by column.
    pub i_to_r: Option<[f32; 16]>,
    /// The cached RAS-to-voxel matrix (absent when its inverse failed).
    pub r_to_i: Option<Matrix>,
    /// The registration matrix.
    pub register_mat: Option<Matrix>,
}

impl Default for VolGeom {
    /// A new geometry (§2): the extent 0 (FreeSurfer leaves it unspecified), voxel sizes 1, the
    /// coronal direction cosines, centre and shears 0, no name, no matrices.
    fn default() -> VolGeom {
        VolGeom {
            valid: 0,
            width: 0,
            height: 0,
            depth: 0,
            xsize: 1.0,
            ysize: 1.0,
            zsize: 1.0,
            ras_good_flag: 0,
            x: [-1.0, 0.0, 0.0],
            y: [0.0, 0.0, -1.0],
            z: [0.0, 1.0, 0.0],
            c: [0.0; 3],
            s: [0.0; 3],
            fname: Vec::new(),
            i_to_r: None,
            r_to_i: None,
            register_mat: None,
        }
    }
}

/// The geometry's file name as FreeSurfer's fixed field keeps it (at most 1,023 bytes).
pub fn fixed_name(name: &[u8], size: usize) -> Vec<u8> {
    let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
    name[..end.min(size - 1)].to_vec()
}

impl VolGeom {
    /// The copy of a geometry into a new one (§2): every field and the cached matrices, never
    /// the registration matrix.
    pub fn copied(&self) -> VolGeom {
        VolGeom {
            register_mat: None,
            ..self.clone()
        }
    }

    /// The direction cosine of axis `j` (0 columns, 1 rows, 2 slices).
    pub fn cosine(&self, j: usize) -> [f32; 3] {
        match j {
            0 => self.x,
            1 => self.y,
            _ => self.z,
        }
    }

    /// The voxel size of axis `j`.
    pub fn size(&self, j: usize) -> f32 {
        match j {
            0 => self.xsize,
            1 => self.ysize,
            _ => self.zsize,
        }
    }

    /// The extent of axis `j`.
    pub fn extent(&self, j: usize) -> i32 {
        match j {
            0 => self.width,
            1 => self.height,
            _ => self.depth,
        }
    }

    /// The cache's 16 floats of a 4 x 4 matrix, column by column.
    pub fn cache_of(m: &Matrix) -> [f32; 16] {
        let mut a = [0.0f32; 16];
        for c in 0..4 {
            for r in 0..4 {
                a[r + 4 * c] = m.at(r as i32 + 1, c as i32 + 1);
            }
        }
        a
    }

    /// The cached voxel-to-RAS matrix as a 4 x 4 matrix.
    pub fn cache_matrix(a: &[f32; 16]) -> Matrix {
        let mut m = Matrix::zeros(4, 4, MATRIX_REAL);
        for c in 0..4 {
            for r in 0..4 {
                m.set(r as i32 + 1, c as i32 + 1, a[r + 4 * c]);
            }
        }
        m
    }

    /// `MRIhfs2Sphinx` (§9): the R components of the cosines and the centre negated (the sign
    /// bit flipped), the A and S components swapped; returns 0.
    pub fn hfs_to_sphinx(&mut self) -> i32 {
        for v in [&mut self.x, &mut self.y, &mut self.z, &mut self.c] {
            v[0] = f32::from_bits(v[0].to_bits() ^ 0x8000_0000);
            v.swap(1, 2);
        }
        0
    }

    /// `isNotEqualThresh(a, b, t)` (§11): 0 when equal, else the code of the first field that
    /// differs (1 `valid`, 2-4 the extent, 5-7 the sizes, 8-16 the cosines, 17-19 the centre,
    /// 20-22 the shears); a float field differs unless `|a - b| < t` (NaN always differs).
    pub fn not_equal_code(a: &VolGeom, b: &VolGeom, t: f64) -> i32 {
        if a.valid != b.valid {
            return 1;
        }
        for (k, (x, y)) in [(a.width, b.width), (a.height, b.height), (a.depth, b.depth)]
            .into_iter()
            .enumerate()
        {
            if x != y {
                return 2 + k as i32;
            }
        }
        let fa = a.float_fields();
        let fb = b.float_fields();
        for k in 0..fa.len() {
            let d = sse::sub32(fa[k], fb[k]).abs();
            if !(f64::from(d) < t) {
                return 5 + k as i32;
            }
        }
        0
    }

    /// The float fields in the order of the inequality codes 5 to 22.
    fn float_fields(&self) -> [f32; 18] {
        [
            self.xsize, self.ysize, self.zsize, self.x[0], self.x[1], self.x[2], self.y[0],
            self.y[1], self.y[2], self.z[0], self.z[1], self.z[2], self.c[0], self.c[1],
            self.c[2], self.s[0], self.s[1], self.s[2],
        ]
    }

    /// Whether any shear is above 1e-5 (compared in double).
    pub fn has_shear(&self) -> bool {
        self.s.iter().any(|&s| f64::from(s) > 1e-5)
    }
}

impl Process {
    /// A geometry assigned onto `dst` (§2): every field and the source's cached matrices, the
    /// destination's registration matrix untouched. FS-59: where the source has no cached
    /// matrix, by default the destination then has none either; `reproduce` keeps its old one.
    pub fn vg_assign(&mut self, dst: &mut VolGeom, src: &VolGeom) {
        let reg = dst.register_mat.take();
        let (old_i, old_r) = (dst.i_to_r.take(), dst.r_to_i.take());
        let stale = (src.i_to_r.is_none() && old_i.is_some())
            || (src.r_to_i.is_none() && old_r.is_some());
        *dst = VolGeom {
            register_mat: reg,
            ..src.clone()
        };
        if stale {
            if self.mode().reproduce() {
                self.note(
                    &FS_59,
                    "the assigned geometry kept its old cached matrices (the source had none)",
                );
                if src.i_to_r.is_none() {
                    dst.i_to_r = old_i;
                }
                if src.r_to_i.is_none() {
                    dst.r_to_i = old_r;
                }
            } else {
                self.note(
                    &FS_59,
                    "the assigned geometry's stale cached matrices dropped (as its source, none)",
                );
            }
        }
    }

    /// `MRIxfmCRS2XYZ(mri, base, shear)` (§3): the scanner voxel-to-RAS matrix of a geometry.
    pub fn vox2ras_scanner(
        &mut self,
        g: &VolGeom,
        base: i32,
        shear: bool,
    ) -> Result<Matrix, Stop> {
        let mut m = Matrix::zeros(4, 4, MATRIX_REAL);
        for j in 0..3 {
            let cos = g.cosine(j);
            let size = sse::widen(g.size(j));
            for r in 0..3 {
                let v = sse::narrow(sse::mul64(sse::widen(cos[r]), size));
                m.set(r as i32 + 1, j as i32 + 1, v);
            }
        }
        m.set(4, 4, 1.0);
        if shear && g.has_shear() {
            let mut sh = Matrix::zeros(4, 4, MATRIX_REAL);
            for k in 1..=4 {
                sh.set(k, k, 1.0);
            }
            sh.set(1, 2, g.s[0]);
            sh.set(1, 3, g.s[1]);
            sh.set(2, 3, g.s[2]);
            if self.gdiag & DIAG_INFO != 0 {
                self.put_out(b"[DEBUG] MRIxfmCRS2XYZ() matshear:\n");
                self.matrix_print(Some(false), Some(&sh), 5);
            }
            if let Returned::New(p) = self.matrix_multiply_d(Some(&m), Some(&sh), None)? {
                m = p;
            }
        }
        let mut p = Matrix::zeros(4, 1, MATRIX_REAL);
        for j in 0..3 {
            let v = f64::from(g.extent(j)) / 2.0 + f64::from(base);
            p.set(j as i32 + 1, 1, sse::narrow(v));
        }
        p.set(4, 1, 1.0);
        if let Returned::New(mp) = self.matrix_multiply_d(Some(&m), Some(&p), None)? {
            for r in 1..=3 {
                let v = sse::sub64(sse::widen(g.c[r as usize - 1]), sse::widen(mp.at(r, 1)));
                m.set(r, 4, sse::narrow(v));
            }
        }
        Ok(m)
    }

    /// `extract_i_to_r` (§3): the scanner matrix, base 0, no shear.
    pub fn extract_i_to_r(&mut self, g: &VolGeom) -> Result<Matrix, Stop> {
        self.vox2ras_scanner(g, 0, false)
    }

    /// `extract_r_to_i` (§3): its inverse, absent when the inverse fails.
    pub fn extract_r_to_i(&mut self, g: &VolGeom) -> Result<Option<Matrix>, Stop> {
        let m = self.extract_i_to_r(g)?;
        self.inverse_of(&m)
    }

    /// The float matrix inverse (`specs/freesurfer-maths.md` §8), absent when it fails.
    pub fn inverse_of(&mut self, m: &Matrix) -> Result<Option<Matrix>, Stop> {
        Ok(match self.matrix_inverse(Some(m), None)? {
            Returned::New(i) => Some(i),
            _ => None,
        })
    }

    /// `MRIreInitCache` (§3): the cached voxel-to-RAS and RAS-to-voxel matrices recomputed from
    /// the geometry; returns 0.
    pub fn reinit_cache(&mut self, g: &mut VolGeom) -> Result<i32, Stop> {
        let m = self.extract_i_to_r(g)?;
        let inv = self.inverse_of(&m)?;
        g.i_to_r = Some(VolGeom::cache_of(&m));
        g.r_to_i = inv;
        Ok(0)
    }

    // ---- §11: printing and comparisons

    /// `vgprint(nocheck)` (§11), on standard output.
    pub fn vg_print(&mut self, g: &VolGeom, nocheck: bool) {
        if g.valid != 1 && !nocheck {
            self.put_out(b"volume geometry: info is either not contained or not valid.\n");
            return;
        }
        let mut b = b"volume geometry:\n".to_vec();
        if nocheck {
            b.extend_from_slice(format!("valid   : {}\n", g.valid).as_bytes());
        }
        b.extend_from_slice(
            format!("extent  : ({}, {}, {})\n", g.width, g.height, g.depth).as_bytes(),
        );
        let rows: [(&str, [f32; 3]); 6] = [
            ("voxel   ", [g.xsize, g.ysize, g.zsize]),
            ("x_(ras) ", g.x),
            ("y_(ras) ", g.y),
            ("z_(ras) ", g.z),
            ("c_(ras) ", g.c),
            ("shears  ", g.s),
        ];
        for (name, v) in rows {
            b.extend_from_slice(name.as_bytes());
            b.extend_from_slice(b": (");
            for (k, x) in v.iter().enumerate() {
                if k > 0 {
                    b.extend_from_slice(b", ");
                }
                b.extend_from_slice(&sprintf_f64(b"%7.4f", sse::widen(*x)));
            }
            b.extend_from_slice(b")\n");
        }
        b.extend_from_slice(b"file    : ");
        b.extend_from_slice(&g.fname);
        b.push(b'\n');
        self.put_out(&b);
    }

    /// `geomprint(fmt, ...)` (§11): the caller's text (already formatted, when given), then the
    /// dimensions, voxel sizes and the R, A and S rows, on standard output.
    pub fn geom_print(&mut self, g: &VolGeom, text: Option<&[u8]>) {
        let mut b = text.map(<[u8]>::to_vec).unwrap_or_default();
        b.extend_from_slice(
            format!(
                "        : Dimensions: {} x {} x {}\n",
                g.width, g.height, g.depth
            )
            .as_bytes(),
        );
        b.extend_from_slice(b"        : voxel size: ");
        for (k, x) in [g.xsize, g.ysize, g.zsize].iter().enumerate() {
            if k > 0 {
                b.extend_from_slice(b", ");
            }
            b.extend_from_slice(&sprintf_f64(b"%6.6f", sse::widen(*x)));
        }
        b.push(b'\n');
        for (i, axis) in ["r", "a", "s"].iter().enumerate() {
            b.extend_from_slice(format!("        : x_{axis} = ").as_bytes());
            b.extend_from_slice(&sprintf_f64(b"%8.10f", sse::widen(g.x[i])));
            b.extend_from_slice(format!(", y_{axis} = ").as_bytes());
            b.extend_from_slice(&sprintf_f64(b"%8.10f", sse::widen(g.y[i])));
            b.extend_from_slice(format!(", z_{axis} = ").as_bytes());
            b.extend_from_slice(&sprintf_f64(b"%8.10f", sse::widen(g.z[i])));
            b.extend_from_slice(format!(", c_{axis} = ").as_bytes());
            b.extend_from_slice(&sprintf_f64(b"%10.10f", sse::widen(g.c[i])));
            b.push(b'\n');
        }
        self.put_out(&b);
    }

    /// `toMatrix` (§11): the cosines as columns 1-3, the centre as column 4, (0, 0, 0, 1) below,
    /// into a given matrix (at least 4 x 4: its top-left 4 x 4, the rest kept; smaller: FS-264)
    /// or a new 4 x 4.
    pub fn vg_to_matrix(
        &mut self,
        g: &VolGeom,
        given: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        let fill = |m: &mut Matrix| {
            for (j, v) in [g.x, g.y, g.z, g.c].iter().enumerate() {
                for r in 0..3 {
                    m.set(r as i32 + 1, j as i32 + 1, v[r]);
                }
            }
            for j in 1..=3 {
                m.set(4, j, 0.0);
            }
            m.set(4, 4, 1.0);
        };
        if let Some(m) = given {
            self.not_4x4("toMatrix", m)?;
            fill(m);
            return Ok(Returned::Output);
        }
        let mut m = Matrix::zeros(4, 4, MATRIX_REAL);
        fill(&mut m);
        Ok(Returned::New(m))
    }

    /// The geometry's `==` (§11): 1 when [`VolGeom::not_equal_code`] is 0 at the threshold
    /// `vg_isEqual_Threshold` (`threshold`).
    pub fn vg_equal(&mut self, a: &VolGeom, b: &VolGeom, threshold: f64) -> i32 {
        i32::from(VolGeom::not_equal_code(a, b, threshold) == 0)
    }

    /// `checkgeom(a, b, t, debug)` (§11): whether an element of the two geometry matrices
    /// differs by more than `t`; each such element printed.
    pub fn vg_check_geom(
        &mut self,
        a: &VolGeom,
        b: &VolGeom,
        t: f64,
        debug: bool,
    ) -> Result<bool, Stop> {
        let ma = self.vg_to_matrix(a, None)?;
        let mb = self.vg_to_matrix(b, None)?;
        let (Returned::New(ma), Returned::New(mb)) = (ma, mb) else {
            return Ok(false);
        };
        let mut differ = false;
        for r in 1..=4 {
            for c in 1..=4 {
                let d = (sse::widen(ma.at(r, c)) - sse::widen(mb.at(r, c))).abs();
                if d > t {
                    differ = true;
                    let mut b = Vec::new();
                    if debug {
                        b.extend_from_slice(b"[DEBUG] ");
                    }
                    b.extend_from_slice(
                        format!(
                            "VOL_GEOM::checkgeom(): Volumes differ in geometry row={r} col={c} diff="
                        )
                        .as_bytes(),
                    );
                    b.extend_from_slice(&sprintf_f64(b"%.10lf", d));
                    b.extend_from_slice(b" (thresh=");
                    b.extend_from_slice(&sprintf_f64(b"%g", t));
                    b.extend_from_slice(b")\n");
                    self.put_out(&b);
                }
            }
        }
        Ok(differ)
    }

    /// `VOL_GEOM::diffvox2ras(m1, m2, t, msg, debug)` (§11): whether an element of the two 4 x 4
    /// matrices differs by more than `t`, with the debug lines.
    pub fn vg_diff_vox2ras(
        &mut self,
        m1: &Matrix,
        m2: &Matrix,
        t: f64,
        msg: &[u8],
        debug: bool,
    ) -> bool {
        const PAD: &[u8] = b"                              : ";
        if debug {
            let mut b = b"[DEBUG]VOL_GEOM::diffvox2ras(): ".to_vec();
            b.extend_from_slice(msg);
            b.push(b'\n');
            self.put_out(&b);
        }
        let mut differ = false;
        for r in 1..=4 {
            for c in 1..=4 {
                let (v1, v2) = (sse::widen(m1.at(r, c)), sse::widen(m2.at(r, c)));
                let d = (v1 - v2).abs();
                if d > t {
                    differ = true;
                    if debug {
                        let mut b = PAD.to_vec();
                        b.extend_from_slice(
                            format!("vox2ras differs at row={r} col={c} diff=").as_bytes(),
                        );
                        b.extend_from_slice(&sprintf_f64(b"%.10lf", d));
                        b.extend_from_slice(b" (thresh=");
                        b.extend_from_slice(&sprintf_f64(b"%g", t));
                        b.extend_from_slice(b"), val1=");
                        b.extend_from_slice(&sprintf_f64(b"%.10lf", v1));
                        b.extend_from_slice(b" val2=");
                        b.extend_from_slice(&sprintf_f64(b"%.10lf", v2));
                        b.push(b'\n');
                        self.put_out(&b);
                    }
                }
            }
        }
        if debug && !differ {
            let mut b = PAD.to_vec();
            b.extend_from_slice(b"vox2ras same\n");
            self.put_out(&b);
        }
        differ
    }

    /// `checkvox2ras(a, b, t, debug, sform)` (§11): whether the two scanner vox2ras (base 0)
    /// differ, with the debug lines and matrices.
    pub fn vg_check_vox2ras(
        &mut self,
        a: &VolGeom,
        b: &VolGeom,
        t: f64,
        debug: bool,
        sform: Option<&Matrix>,
    ) -> Result<bool, Stop> {
        let m1 = self.vox2ras_scanner(a, 0, false)?;
        let m2 = self.vox2ras_scanner(b, 0, false)?;
        let differ = self.vg_diff_vox2ras(&m1, &m2, t, b"TAG_RAS_XFORM vs MRI_XFORM", debug);
        if differ && debug {
            if let Some(sf) = sform {
                self.vg_diff_vox2ras(sf, &m2, t, b"NII_SFORM vs MRI_XFORM", debug);
                self.vg_diff_vox2ras(&m1, sf, t, b"TAG_RAS_XFORM vs NII_SFORM", debug);
            }
            self.put_out(b"VOL_GEOM::checkvox2ras(): vox2ras1 (TAG_RAS_XFROM):\n");
            self.matrix_print(Some(false), Some(&m1), 10);
            self.put_out(b"VOL_GEOM::checkvox2ras(): vox2ras2 (MRI_XFORM):\n");
            self.matrix_print(Some(false), Some(&m2), 10);
            if let Some(sf) = sform {
                self.put_out(b"VOL_GEOM::checkvox2ras(): nii_sform:\n");
                self.matrix_print(Some(false), Some(sf), 10);
            }
        }
        Ok(differ)
    }
}
