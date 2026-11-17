// SPDX-License-Identifier: Apache-2.0
//! The geometry's copies, matrices, comparison and printer (spec §4).

use crate::clib::printf::sprintf_f64;
use crate::maths::matrix::Matrix;
use crate::mri::VolGeom;
use crate::mri::geom::fixed_name;
use crate::process::{Process, Stop};
use crate::sse;

/// `FLT_EPSILON`, the threshold of `vg_isEqual` unless a program changes it.
pub const VG_IS_EQUAL_THRESHOLD: f64 = f32::EPSILON as f64;

/// The fields `getVolGeom` and `copyVolGeom` copy (§4), `valid` aside.
fn copy_fields(src: &VolGeom, dst: &mut VolGeom) {
    dst.width = src.width;
    dst.height = src.height;
    dst.depth = src.depth;
    dst.xsize = src.xsize;
    dst.ysize = src.ysize;
    dst.zsize = src.zsize;
    dst.x = src.x;
    dst.y = src.y;
    dst.z = src.z;
    dst.c = src.c;
    dst.fname = fixed_name(&src.fname, 1024);
}

impl Process {
    /// `getVolGeom(src, dst)` (§4): `valid` 1, the extent, sizes, cosines, centre and name.
    pub fn get_vol_geom(&mut self, src: &VolGeom, dst: &mut VolGeom) {
        dst.valid = 1;
        copy_fields(src, dst);
    }

    /// `copyVolGeom(src, dst)` (§4): as [`Process::get_vol_geom`] with the source's `valid`.
    pub fn copy_vol_geom(&mut self, src: &VolGeom, dst: &mut VolGeom) {
        dst.valid = src.valid;
        copy_fields(src, dst);
    }

    /// `vg_i_to_r(vg)` (§4): the scanner voxel-to-RAS matrix (base 0, no shear).
    pub fn vg_i_to_r(&mut self, vg: &VolGeom) -> Result<Matrix, Stop> {
        self.extract_i_to_r(vg)
    }

    /// `vg_r_to_i(vg)` (§4): its inverse, none when the inverse fails.
    pub fn vg_r_to_i(&mut self, vg: &VolGeom) -> Result<Option<Matrix>, Stop> {
        self.extract_r_to_i(vg)
    }

    /// `TkrVox2RASfromVolGeom(vg)` (§4): the tkregister voxel-to-RAS matrix.
    pub fn tkr_vox2ras_from_vol_geom(&mut self, vg: &VolGeom) -> Result<Matrix, Stop> {
        self.vox2ras_tkreg(vg)
    }

    /// `vg_isNotEqualThresh(vg1, vg2, thresh)` (§4): 0 when they agree, else the code of the
    /// first field that differs (1 `valid`, 2-4 the extent, 5-7 the sizes, 8-16 the cosines,
    /// 17-19 the centre); the shears are not compared.
    pub fn vg_is_not_equal_thresh(&mut self, a: &VolGeom, b: &VolGeom, thresh: f64) -> i32 {
        let code = VolGeom::not_equal_code(a, b, thresh);
        if code > 19 { 0 } else { code }
    }

    /// `vg_isEqual(vg1, vg2)` (§4) at the program's threshold (`threshold`,
    /// [`VG_IS_EQUAL_THRESHOLD`] unless the program changes it): 1 when equal, else 0.
    pub fn vg_is_equal(&mut self, a: &VolGeom, b: &VolGeom, threshold: f64) -> i32 {
        i32::from(self.vg_is_not_equal_thresh(a, b, threshold) == 0)
    }

    /// `vg_print(vg)` (§4, obsolete): the geometry on standard error, then standard error flushed.
    pub fn vg_print_obsolete(&mut self, g: &VolGeom) {
        if g.valid != 1 {
            self.put_err(b"volume geometry: info is either not contained or not valid.\n");
            self.flush();
            return;
        }
        let mut b = b"volume geometry:\n".to_vec();
        b.extend_from_slice(
            format!("extent  : ({}, {}, {})\n", g.width, g.height, g.depth).as_bytes(),
        );
        let rows: [(&str, [f32; 3]); 5] = [
            ("voxel   ", [g.xsize, g.ysize, g.zsize]),
            ("x_(ras) ", g.x),
            ("y_(ras) ", g.y),
            ("z_(ras) ", g.z),
            ("c_(ras) ", g.c),
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
        let end = g
            .fname
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(g.fname.len());
        b.extend_from_slice(&g.fname[..end]);
        b.push(b'\n');
        self.put_err(&b);
        self.flush();
    }
}
