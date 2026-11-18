// SPDX-License-Identifier: Apache-2.0
//! Extracting a region (spec §11): `MRIextract`, `MRIextractInto`, `MRIextractRegion` and
//! `MRIcalcCRASforExtractedVolume`; FS-293 to FS-295 and FS-297.

use larmorx_core::defects::{FS_293, FS_294, FS_295, FS_297};

use super::{Dst, VolOut, Vols};
use crate::clib::printf::sprintf_f64;
use crate::mri::{Mri, Region, types};
use crate::process::{DIAG_VERBOSE, Process, Stop, codes};
use crate::sse;

/// The diagnostic bit `DIAG_SHOW`.
const DIAG_SHOW: u64 = 0x40;

/// C's `(int)` of a float (x86-64: `INT_MIN` for a NaN or a value outside `int`).
fn trunc_int(x: f32) -> i32 {
    if x.is_nan() || !(-2147483648.0..2147483648.0).contains(&x) {
        i32::MIN
    } else {
        x as i32
    }
}

impl Process {
    /// `MRIextract(src, dst, x0, y0, z0, dx, dy, dz)` (§11): `MRIextractInto` at (0, 0, 0).
    #[allow(clippy::too_many_arguments)]
    pub fn mri_extract(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        x0: i32,
        y0: i32,
        z0: i32,
        dx: i32,
        dy: i32,
        dz: i32,
    ) -> Result<VolOut, Stop> {
        self.mri_extract_into(src, dst, [x0, y0, z0], [dx, dy, dz], [0, 0, 0])
    }

    /// `MRIextractRegion(src, dst, region)` (§11): the region's box (the whole volume without
    /// one).
    pub fn mri_extract_region(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        region: Option<&Region>,
    ) -> Result<VolOut, Stop> {
        let g = &src.geom;
        let r = region.copied().unwrap_or(Region {
            x: 0,
            y: 0,
            z: 0,
            dx: g.width,
            dy: g.height,
            dz: g.depth,
        });
        self.mri_extract_into(src, dst, [r.x, r.y, r.z], [r.dx, r.dy, r.dz], [0, 0, 0])
    }

    /// `MRIextractInto(src, dst, x0, y0, z0, dx, dy, dz, x1, y1, z1)` (§11): the box at `p0`
    /// of size `size` copied to `p1` of the destination.
    pub fn mri_extract_into(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        p0: [i32; 3],
        size: [i32; 3],
        p1: [i32; 3],
    ) -> Result<VolOut, Stop> {
        let reproduce = self.mode().reproduce();
        let mut v = Vols::new(src, None, dst);
        let [mut x0, mut y0, mut z0] = p0;
        let [mut dx, mut dy, mut dz] = size;
        let [x1, y1, z1] = p1;
        // 1
        {
            let g = &v.a().geom;
            if x0 >= g.width || y0 >= g.height || z0 >= g.depth {
                self.error_printf(
                    codes::BAD_PARM,
                    format!("MRIextractInto: bad src location ({x0}, {y0}, {z0})").as_bytes(),
                );
            }
        }
        // 2
        let fresh = !v.has_dst();
        if fresh {
            let (kind, nf) = (v.a().kind, v.a().nframes);
            let mut d = self.mri_alloc_sequence(dx, dy, dz, kind, nf)?;
            self.copy_header_into(v.a(), &mut d)?;
            d.imnr0 = z0.wrapping_add(v.a().imnr0).wrapping_sub(z1);
            d.imnr1 = d.imnr0.wrapping_add(dz).wrapping_sub(1);
            v.set_dst(d);
        }
        // 3
        if v.d().kind != v.a().kind {
            self.error_printf(
                codes::BAD_PARM,
                b"MRIextractInto: src and dst types must match",
            );
            if !fresh {
                self.note(
                    &FS_293,
                    "the caller's destination of another type kept (FreeSurfer frees it)",
                );
            }
            return Ok(VolOut::Nothing);
        }
        // 4
        x0 = x0.max(0);
        y0 = y0.max(0);
        z0 = z0.max(0);
        {
            let (g, h) = (&v.a().geom, &v.d().geom);
            dx = dx.min(g.width - x0).min(h.width - x1);
            dy = dy.min(g.height - y0).min(h.height - y1);
            dz = dz.min(g.depth - z0).min(h.depth - z1);
        }
        // 5
        if fresh {
            let (xs, ys, zs) = (v.a().geom.xsize, v.a().geom.ysize, v.a().geom.zsize);
            let whole = |s: f32| s == s.trunc() && s.is_finite();
            if !(whole(xs) && whole(ys) && whole(zs)) {
                if reproduce {
                    self.note(
                        &FS_295,
                        "the new volume's start and end set with the voxel sizes truncated to integers",
                    );
                } else {
                    self.note(
                        &FS_295,
                        "the new volume's start and end set with the voxel sizes",
                    );
                }
            }
            let d = v.d_mut();
            for (k, s) in [xs, ys, zs].into_iter().enumerate() {
                let (p, n) = [(x0, dx), (y0, dy), (z0, dz)][k];
                let (start, end) = match k {
                    0 => (&mut d.xstart, &mut d.xend),
                    1 => (&mut d.ystart, &mut d.yend),
                    _ => (&mut d.zstart, &mut d.zend),
                };
                if reproduce || whole(s) {
                    let i = trunc_int(s);
                    *start = sse::add32(*start, p.wrapping_mul(i) as f32);
                    *end = sse::add32(*start, n.wrapping_mul(i) as f32);
                } else {
                    *start = sse::add32(*start, sse::mul32(p as f32, s));
                    *end = sse::add32(*start, sse::mul32(n as f32, s));
                }
            }
        }
        // 6
        let kind = v.a().kind;
        if !matches!(
            kind,
            types::UCHAR | types::SHORT | types::USHRT | types::INT | types::LONG | types::FLOAT
        ) {
            self.error_exit(
                codes::UNSUPPORTED,
                format!("MRIextractInto: unsupported source type {kind}").as_bytes(),
            )?;
        }
        // 7
        let bpv = v.a().bytes_per_vox as i64;
        let nf = v.a().nframes;
        let half = kind == types::LONG;
        if half && (x0 != 0 || x1 != 0) && dx > 0 && dy > 0 && dz > 0 && nf > 0 {
            if reproduce {
                self.note(
                    &FS_297,
                    "a box of 64-bit voxels copied from half the first column (FreeSurfer's byte offsets)",
                );
            } else {
                self.note(&FS_297, "a box of 64-bit voxels copied whole");
            }
        }
        let (sb, db) = if half && reproduce {
            (4 * i64::from(x0), 4 * i64::from(x1))
        } else {
            (bpv * i64::from(x0), bpv * i64::from(x1))
        };
        let what = "MRIextractInto";
        for f in 0..i64::from(nf) {
            for z in 0..i64::from(dz) {
                for y in 0..i64::from(dy) {
                    if dx < 0 {
                        self.note(
                            &FS_294,
                            "a box row of a negative length: the program ends (FreeSurfer crashes)",
                        );
                        self.error_exit(
                            codes::BAD_PARM,
                            b"MRIextractInto: a box row of a negative length (FreeSurfer crashes here)",
                        )?;
                        return Ok(VolOut::Nothing);
                    }
                    self.direct_needs_voxels(v.a(), what)?;
                    self.direct_needs_voxels(v.d(), what)?;
                    let (sy, sz) = (i64::from(y0) + y, i64::from(z0) + z);
                    let (dy1, dz1) = (i64::from(y1) + y, i64::from(z1) + z);
                    for i in 0..i64::from(dx) {
                        let sp = self.row_place(v.a(), sb + i * bpv, sy, sz, f, bpv as u64);
                        let b = self.direct_read(v.a(), sp, bpv as usize, what);
                        let dp = self.row_place(v.d(), db + i * bpv, dy1, dz1, f, bpv as u64);
                        self.direct_write(v.d_mut(), dp, &b, what);
                    }
                }
            }
        }
        // 8
        let s = v.a().clone();
        let cras =
            self.mri_calc_cras_for_extracted_volume_of(&mut v, [x0, y0, z0], [x1, y1, z1])?;
        let ct = s.ct.as_ref().and_then(|c| self.ctab_deep_copy(Some(c)));
        let d = v.d_mut();
        d.geom.xsize = s.geom.xsize;
        d.geom.ysize = s.geom.ysize;
        d.geom.zsize = s.geom.zsize;
        d.thick = s.thick;
        d.ps = s.ps;
        d.flip_angle = s.flip_angle;
        d.tr = s.tr;
        d.te = s.te;
        d.ti = s.ti;
        d.geom.c = [
            sse::narrow(cras[0]),
            sse::narrow(cras[1]),
            sse::narrow(cras[2]),
        ];
        let mut g = d.geom.clone();
        self.reinit_cache(&mut g)?;
        let d = v.d_mut();
        d.geom = g;
        if ct.is_some() {
            d.ct = ct;
        }
        Ok(v.out())
    }

    /// The centre of the extracted volume for the call's volumes (the source's cache filled).
    fn mri_calc_cras_for_extracted_volume_of(
        &mut self,
        v: &mut Vols,
        p0: [i32; 3],
        p1: [i32; 3],
    ) -> Result<[f64; 3], Stop> {
        let dst = v.d().clone();
        let src = v.vol_mut(0);
        self.mri_calc_cras_for_extracted_volume(src, &dst, p0, p1)
    }

    /// `MRIcalcCRASforExtractedVolume(src, dst, x0, y0, z0, x1, y1, z1)` (§11): the point `p0 +
    /// dst's extent / 2 - p1` through the source's cached voxel-to-RAS matrix (computed and kept
    /// when absent).
    pub fn mri_calc_cras_for_extracted_volume(
        &mut self,
        src: &mut Mri,
        dst: &Mri,
        p0: [i32; 3],
        p1: [i32; 3],
    ) -> Result<[f64; 3], Stop> {
        let e = [dst.geom.width, dst.geom.height, dst.geom.depth];
        let q: Vec<f64> = (0..3)
            .map(|k| f64::from(p0[k]) + f64::from(e[k]) / 2.0 - f64::from(p1[k]))
            .collect();
        let r = self.voxel_to_world(&mut src.geom, q[0], q[1], q[2])?;
        if self.gdiag & DIAG_SHOW != 0 && self.gdiag & DIAG_VERBOSE != 0 {
            let mut b = b"c_ras for sample volume is (".to_vec();
            for (k, x) in r.iter().enumerate() {
                if k > 0 {
                    b.extend_from_slice(b", ");
                }
                b.extend_from_slice(&sprintf_f64(b"%f", *x));
            }
            b.extend_from_slice(b") compared with the src (");
            for (k, x) in src.geom.c.iter().enumerate() {
                if k > 0 {
                    b.extend_from_slice(b", ");
                }
                b.extend_from_slice(&sprintf_f64(b"%f", sse::widen(*x)));
            }
            b.extend_from_slice(b")\n");
            self.put_err(&b);
        }
        Ok(r)
    }
}
