// SPDX-License-Identifier: Apache-2.0
//! Regions (spec §43): `REGIONalloc`, `REGIONclear`, `REGIONcopy`, `REGIONprint` and the bounding
//! boxes of a mask, `REGIONgetBoundingBox`, `REGIONgetBoundingBoxM` (FS-269) and
//! `REGIONgetBoundingBoxEqOdd`.

use larmorx_core::defects::FS_269;

use crate::mri::{Mri, Region};
use crate::process::{Process, Stop};

/// `REGIONalloc()` (§43): a region of zeros.
pub fn region_alloc() -> Region {
    Region::default()
}

/// `REGIONclear(r)` (§43): every field 0.
pub fn region_clear(r: &mut Region) {
    *r = Region::default();
}

/// `REGIONcopy(src, dst)` (§43): a copy (into a new region without one).
pub fn region_copy(src: &Region, dst: Option<&mut Region>) -> Region {
    if let Some(d) = dst {
        *d = *src;
    }
    *src
}

/// `REGIONprint`'s line (§43): `x y z  dx dy dz` and a newline.
pub fn region_line(r: &Region) -> Vec<u8> {
    format!("{} {} {}  {} {} {}\n", r.x, r.y, r.z, r.dx, r.dy, r.dz).into_bytes()
}

impl Process {
    /// `REGIONprint(stdout, r)` (§43).
    pub fn region_print(&mut self, r: &Region) {
        self.put_out(&region_line(r));
    }

    /// The extremes of the columns, rows and slices of the mask's frame-0 voxels whose magnitude
    /// is not below `1e-5` (in `double`; a NaN counts); none: minimum 1000000, maximum 0.
    fn mask_extremes(&mut self, m: &Mri) -> Result<([i32; 3], [i32; 3], bool), Stop> {
        let (w, h, d) = (m.geom.width, m.geom.height, m.geom.depth);
        let mut lo = [1_000_000i32; 3];
        let mut hi = [0i32; 3];
        let mut any = false;
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let v = self.vf(m, (c, r, s), 0)?;
                    if f64::from(v.abs()) < 1e-5 {
                        continue;
                    }
                    any = true;
                    for (k, x) in [c, r, s].into_iter().enumerate() {
                        lo[k] = lo[k].min(x);
                        hi[k] = hi[k].max(x);
                    }
                }
            }
        }
        Ok((lo, hi, any))
    }

    /// `REGIONgetBoundingBoxM(mask, npad)` (§43): the box of the counted voxels with a pad
    /// before (`npad[2k]`) and after (`npad[2k + 1]`) each axis: FreeSurfer's start
    /// `max(min - before, 0)` and size `min(max - min + before + after, extent - start)`
    /// (32-bit); FS-269: by default one voxel more where that box ends before the mask's last
    /// voxel, noted in both modes.
    pub fn region_get_bounding_box_m(&mut self, m: &Mri, npad: [i32; 6]) -> Result<Region, Stop> {
        let (lo, hi, any) = self.mask_extremes(m)?;
        let ext = [m.geom.width, m.geom.height, m.geom.depth];
        let reproduce = self.mode().reproduce();
        let mut start = [0i32; 3];
        let mut size = [0i32; 3];
        let mut short = false;
        for k in 0..3 {
            let (before, after) = (npad[2 * k], npad[2 * k + 1]);
            start[k] = lo[k].wrapping_sub(before).max(0);
            let room = ext[k].wrapping_sub(start[k]);
            let theirs = hi[k]
                .wrapping_sub(lo[k])
                .wrapping_add(before)
                .wrapping_add(after)
                .min(room);
            size[k] = theirs;
            if any && i64::from(start[k]) + i64::from(theirs) - 1 < i64::from(hi[k]) {
                short = true;
                if !reproduce {
                    size[k] = hi[k]
                        .wrapping_sub(lo[k])
                        .wrapping_add(1)
                        .wrapping_add(before)
                        .wrapping_add(after)
                        .min(room);
                }
            }
        }
        if short {
            if reproduce {
                self.note(
                    &FS_269,
                    "REGIONgetBoundingBox: FreeSurfer's box, ending before the mask's last voxel",
                );
            } else {
                self.note(
                    &FS_269,
                    "REGIONgetBoundingBox: one voxel more where FreeSurfer's box ends before the mask's last voxel",
                );
            }
        }
        Ok(Region {
            x: start[0],
            y: start[1],
            z: start[2],
            dx: size[0],
            dy: size[1],
            dz: size[2],
        })
    }

    /// `REGIONgetBoundingBox(mask, npad)` (§43): the same pad before and after every axis.
    pub fn region_get_bounding_box(&mut self, m: &Mri, npad: i32) -> Result<Region, Stop> {
        self.region_get_bounding_box_m(m, [npad; 6])
    }

    /// `REGIONgetBoundingBoxEqOdd(mask, npad)` (§43): the box (the mode's own) with equal sides
    /// in the plane and an odd number of slices, or none (its errors).
    pub fn region_get_bounding_box_eq_odd(
        &mut self,
        m: &Mri,
        npad: i32,
    ) -> Result<Option<Region>, Stop> {
        let mut b = self.region_get_bounding_box(m, npad)?;
        let (w, h, d) = (m.geom.width, m.geom.height, m.geom.depth);
        let head = format!("Volume size: {w} {h} {d}, ");
        let mut out = format!("{head}OrigRegion: ").into_bytes();
        out.extend_from_slice(&region_line(&b));
        self.put_out(&out);
        if b.dx > h {
            self.put_out(b"ERROR: REGIONgetBoundingBoxEqOdd(): dx is bigger than MRI height\n");
            return Ok(None);
        }
        if b.dy > w {
            self.put_out(b"ERROR: REGIONgetBoundingBoxEqOdd(): dy is bigger than MRI width\n");
            return Ok(None);
        }
        if b.dx == b.dy && b.dz % 2 != 0 {
            return Ok(Some(b));
        }
        if b.dz % 2 == 0 {
            if b.dz < d {
                b.dz = b.dz.wrapping_add(1);
            } else if b.z > 0 {
                b.z -= 1;
                b.dz = b.dz.wrapping_add(1);
            } else {
                self.put_out(
                    format!(
                        "ERROR: REGIONgetBoundingBoxEqOdd(): cannot make z odd: z={}, dz={}, nz={d}\n",
                        b.z, b.dz
                    )
                    .as_bytes(),
                );
                return Ok(None);
            }
        }
        if b.dx == b.dy {
            return Ok(Some(b));
        }
        let dxy = b.dx.max(b.dy);
        if b.x.wrapping_add(dxy) <= w && b.y.wrapping_add(dxy) <= h {
            b.dx = dxy;
            b.dy = dxy;
            return Ok(Some(b));
        }
        let delta = b.dx.wrapping_sub(b.dy).wrapping_abs();
        let delta1 = delta / 2;
        let delta2 = delta.wrapping_sub(delta1);
        // the smaller side: (start, size, the other size, the volume's room, its name)
        let x_smaller = b.dx < b.dy;
        let (p, sz, other, room, name) = if x_smaller {
            (b.x, b.dx, b.dy, w, "x")
        } else {
            (b.y, b.dy, b.dx, h, "y")
        };
        let (np, nsz) =
            if p.wrapping_sub(delta1) > 0 && p.wrapping_add(sz).wrapping_add(delta2) <= room {
                (p.wrapping_sub(delta1), sz.wrapping_add(delta))
            } else {
                // the larger side's size, at 0 when the start is below the room after the box,
                // else ending at the edge
                let after = room.wrapping_sub(p.wrapping_add(sz));
                let np = if p < after {
                    0
                } else {
                    room.wrapping_sub(other)
                };
                let (bx, by, bdx, bdy) = if x_smaller {
                    (np, b.y, other, b.dy)
                } else {
                    (b.x, np, b.dx, other)
                };
                let r = Region {
                    x: bx,
                    y: by,
                    z: b.z,
                    dx: bdx,
                    dy: bdy,
                    dz: b.dz,
                };
                let mut out = format!("{head}{name}AsymRegion: ").into_bytes();
                out.extend_from_slice(&region_line(&r));
                self.put_out(&out);
                return Ok(Some(r));
            };
        if x_smaller {
            b.x = np;
            b.dx = nsz;
        } else {
            b.y = np;
            b.dy = nsz;
        }
        Ok(Some(b))
    }
}
