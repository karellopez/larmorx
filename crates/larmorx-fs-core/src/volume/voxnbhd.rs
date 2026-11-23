// SPDX-License-Identifier: Apache-2.0
//! Voxel neighbourhood measures (spec §44): `MRIvoxelDx`, `MRIvoxelDy`, `MRIvoxelDz` (FS-323),
//! `MRIvoxelMean`, `MRIvoxelDirection` (FS-485) and `MRIvoxelGradientDir2ndDerivative`.

use larmorx_core::defects::FS_485;

use crate::mri::Mri;
use crate::process::{Process, Stop, codes};
use crate::sse;

/// `1.0f / 27000.0f` (§44).
const INV_27000: f32 = f32::from_bits(0x381B_5837);

/// Which derivative: along the columns, the rows or the slices.
#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
    Z,
}

/// One term of `MRIvoxelDirection`'s sum in the official build's operand orders (§44): pattern
/// 0 (the first four of a row), 1 (the fifth) or 2 (the sixth).
fn direction_term(pattern: u8, off: [f32; 3], g: [f32; 3], g0: [f32; 3]) -> f32 {
    use sse::{add32, mul32};
    let (x, y, z) = (off[0], off[1], off[2]);
    let xg = mul32(x, g0[0]);
    let yg = mul32(y, g0[1]);
    let zg = mul32(z, g0[2]);
    let gxg = mul32(g[0], g0[0]);
    let gyg = mul32(g[1], g0[1]);
    let gzg = mul32(g[2], g0[2]);
    match pattern {
        0 => {
            let t1 = add32(add32(yg, xg), zg);
            let t2 = add32(add32(gxg, gyg), gzg);
            mul32(t1, t2)
        }
        1 => {
            let t1 = add32(add32(xg, yg), zg);
            let t2 = add32(add32(gxg, gyg), gzg);
            mul32(t2, t1)
        }
        _ => {
            let t1 = add32(add32(xg, yg), zg);
            mul32(t1, add32(gzg, add32(gxg, gyg)))
        }
    }
}

/// The pattern of the `k`-th element (0-based) of a row of `n` (§44): a row of four or more
/// `0 0 0 0 1 2 0`, a row of three or fewer `1 2 0`.
fn pattern_of(k: usize, n: usize) -> u8 {
    const LONG: [u8; 7] = [0, 0, 0, 0, 1, 2, 0];
    const SHORT: [u8; 3] = [1, 2, 0];
    if n >= 4 {
        LONG.get(k).copied().unwrap_or(0)
    } else {
        SHORT.get(k).copied().unwrap_or(0)
    }
}

impl Process {
    /// The three index-table look-ups of a point (FS-323: an index past the tables ends the
    /// program before any voxel is read).
    fn vox_tables(
        &mut self,
        m: &Mri,
        (x, y, z): (i32, i32, i32),
        name: &str,
    ) -> Result<(i32, i32, i32), Stop> {
        let xi = self.table_index(m.geom.width, x, name)?;
        let yi = self.table_index(m.geom.height, y, name)?;
        let zi = self.table_index(m.geom.depth, z, name)?;
        Ok((xi, yi, zi))
    }

    /// One of `MRIvoxelDx`, `MRIvoxelDy`, `MRIvoxelDz` (§44).
    fn voxel_d(&mut self, m: &Mri, p: (i32, i32, i32), axis: Axis) -> Result<f32, Stop> {
        let name = match axis {
            Axis::X => "MRIvoxelDx",
            Axis::Y => "MRIvoxelDy",
            Axis::Z => "MRIvoxelDz",
        };
        let (x, y, z) = self.vox_tables(m, p, name)?;
        let (w, h, d) = (m.geom.width, m.geom.height, m.geom.depth);
        let xm = self.table_index(w, x - 1, name)?;
        let xp = self.table_index(w, x + 1, name)?;
        let ym = self.table_index(h, y - 1, name)?;
        let yp = self.table_index(h, y + 1, name)?;
        let (zm, zp) = match axis {
            Axis::Z => (
                self.table_index(d, z - 1, name)?,
                self.table_index(d, z + 1, name)?,
            ),
            _ => (z, z),
        };
        // `e2 + (2 f + e1)`, `2 f` as `f + f`
        let side =
            |p: &mut Process, f: (i32, i32, i32), e1: (i32, i32, i32), e2: (i32, i32, i32)| {
                let fv = p.vf(m, f, 0)?;
                let e1v = p.vf(m, e1, 0)?;
                let e2v = p.vf(m, e2, 0)?;
                Ok::<f32, Stop>(sse::add32(e2v, sse::add32(sse::add32(fv, fv), e1v)))
            };
        let (lo, hi) = match axis {
            Axis::X => (
                side(self, (xm, y, z), (xm, ym, z), (xm, yp, z))?,
                side(self, (xp, y, z), (xp, ym, z), (xp, yp, z))?,
            ),
            Axis::Y => (
                side(self, (xm, ym, z), (x, ym, z), (xp, ym, z))?,
                side(self, (xm, yp, z), (x, yp, z), (xp, yp, z))?,
            ),
            Axis::Z => (
                side(self, (xm, y, zm), (x, y, zm), (xp, y, zm))?,
                side(self, (xm, y, zp), (x, y, zp), (xp, y, zp))?,
            ),
        };
        Ok(sse::mul32(sse::sub32(hi, lo), 0.125))
    }

    /// `MRIvoxelDx(mri, x, y, z)` (§44).
    pub fn mri_voxel_dx(&mut self, m: &Mri, x: i32, y: i32, z: i32) -> Result<f32, Stop> {
        self.voxel_d(m, (x, y, z), Axis::X)
    }

    /// `MRIvoxelDy(mri, x, y, z)` (§44).
    pub fn mri_voxel_dy(&mut self, m: &Mri, x: i32, y: i32, z: i32) -> Result<f32, Stop> {
        self.voxel_d(m, (x, y, z), Axis::Y)
    }

    /// `MRIvoxelDz(mri, x, y, z)` (§44).
    pub fn mri_voxel_dz(&mut self, m: &Mri, x: i32, y: i32, z: i32) -> Result<f32, Stop> {
        self.voxel_d(m, (x, y, z), Axis::Z)
    }

    /// The gradient `(Dx, Dy, Dz)` of a voxel.
    fn voxel_gradient(&mut self, m: &Mri, p: (i32, i32, i32)) -> Result<[f32; 3], Stop> {
        Ok([
            self.mri_voxel_dx(m, p.0, p.1, p.2)?,
            self.mri_voxel_dy(m, p.0, p.1, p.2)?,
            self.mri_voxel_dz(m, p.0, p.1, p.2)?,
        ])
    }

    /// `MRIvoxelMean(mri, x0, y0, z0, wsize, frame)` (§44): the `float` mean of the frame's
    /// voxels in the cube of half-width `wsize / 2` cut at the volume's faces; 0 for no voxel.
    pub fn mri_voxel_mean(
        &mut self,
        m: &Mri,
        (x0, y0, z0): (i32, i32, i32),
        wsize: i32,
        frame: i32,
    ) -> Result<f32, Stop> {
        let h = wsize / 2;
        let ext = [m.geom.width, m.geom.height, m.geom.depth];
        let ctr = [x0, y0, z0];
        let lo = [0, 1, 2].map(|k| ctr[k].wrapping_sub(h).max(0));
        let hi = [0, 1, 2].map(|k| ctr[k].wrapping_add(h).min(ext[k] - 1));
        let npix = (0..3)
            .map(|k| hi[k].wrapping_sub(lo[k]).wrapping_add(1))
            .fold(1i32, i32::wrapping_mul);
        let mut total = 0f32;
        for z in lo[2]..=hi[2] {
            for y in lo[1]..=hi[1] {
                for x in lo[0]..=hi[0] {
                    let v = self.vf(m, (x, y, z), frame)?;
                    total = sse::add32(v, total);
                }
            }
        }
        Ok(if npix > 0 {
            sse::div32(total, npix as f32)
        } else {
            0.0
        })
    }

    /// `MRIvoxelDirection(mri, x0, y0, z0, wsize)` (§44): the window's gradients projected on
    /// the voxel's own, weighted by the offsets, times `1 / 27000`. FS-485: by default the window
    /// goes past the far faces through the index tables; with `reproduce` FreeSurfer's cut window
    /// read back as a full cube (the entries never written 0).
    pub fn mri_voxel_direction(
        &mut self,
        m: &Mri,
        (x0, y0, z0): (i32, i32, i32),
        wsize: i32,
    ) -> Result<f32, Stop> {
        if wsize > 7 {
            self.error_printf(
                codes::BAD_PARM,
                format!("MRIvoxelDirection: window size {wsize} too big").as_bytes(),
            );
            return Ok(0.0);
        }
        let h = wsize / 2;
        let ext = [m.geom.width, m.geom.height, m.geom.depth];
        let ctr = [x0, y0, z0];
        let cut =
            h >= 0 && (0..3).any(|k| i64::from(ctr[k]) + i64::from(h) > i64::from(ext[k]) - 1);
        let reproduce = self.mode().reproduce();
        if cut {
            if reproduce {
                self.note(
                    &FS_485,
                    "MRIvoxelDirection: the window cut at the far faces and read back as a full cube, the entries never written 0 (FreeSurfer's)",
                );
            } else {
                self.note(
                    &FS_485,
                    "MRIvoxelDirection: the full window, past the far faces through the index tables (FreeSurfer cuts it and reads it back as a full cube)",
                );
            }
        }
        // the stored gradients, slices outermost
        let far = |k: usize| {
            let e = ctr[k].wrapping_add(h);
            if reproduce { e.min(ext[k] - 1) } else { e }
        };
        let mut stored = Vec::new();
        if h >= 0 {
            for z in z0.wrapping_sub(h)..=far(2) {
                for y in y0.wrapping_sub(h)..=far(1) {
                    for x in x0.wrapping_sub(h)..=far(0) {
                        stored.push(self.voxel_gradient(m, (x, y, z))?);
                    }
                }
            }
        }
        let g0 = self.voxel_gradient(m, (x0, y0, z0))?;
        if h < 0 {
            return Ok(0.0);
        }
        let n = (2 * h + 1) as usize;
        let mut dir = 0f32;
        let mut k = 0usize;
        for z in -h..=h {
            for y in -h..=h {
                for (j, x) in (-h..=h).enumerate() {
                    let g = stored.get(k).copied().unwrap_or([0.0; 3]);
                    k += 1;
                    let term =
                        direction_term(pattern_of(j, n), [x as f32, y as f32, z as f32], g, g0);
                    dir = sse::add32(dir, term);
                }
            }
        }
        Ok(sse::mul32(dir, INV_27000))
    }

    /// `MRIvoxelGradientDir2ndDerivative(mri, x0, y0, z0, wsize)` (§44): the second derivative
    /// along the unit gradient by samples at `p0 ± d u`; 0 where the gradient's length is below
    /// `1e-5`.
    pub fn mri_voxel_gradient_dir_2nd_derivative(
        &mut self,
        m: &Mri,
        (x0, y0, z0): (i32, i32, i32),
        wsize: i32,
    ) -> Result<f32, Stop> {
        let g = self.voxel_gradient(m, (x0, y0, z0))?;
        let len = sse::add32(
            sse::add32(sse::mul32(g[0], g[0]), sse::mul32(g[1], g[1])),
            sse::mul32(g[2], g[2]),
        )
        .sqrt();
        if f64::from(len.abs()) < 1e-5 {
            return Ok(0.0);
        }
        let u = g.map(|x| sse::div32(x, len));
        let h = wsize / 2;
        let (cx, cy, cz) = (x0 as f32, y0 as f32, z0 as f32);
        let mut val = 0f64;
        self.mri_sample_volume(m, f64::from(cx), f64::from(cy), f64::from(cz), &mut val)?;
        let mut d2 = sse::narrow(sse::mul64(f64::from(wsize.wrapping_sub(1)), val));
        let mut d = 1f32;
        while d <= h as f32 {
            for s in [1.0f32, -1.0] {
                let p = [cx, cy, cz]
                    .iter()
                    .zip(u)
                    .map(|(&c, uc)| {
                        let step = sse::mul32(d, uc);
                        f64::from(if s > 0.0 {
                            sse::add32(c, step)
                        } else {
                            sse::sub32(c, step)
                        })
                    })
                    .collect::<Vec<f64>>();
                self.mri_sample_volume(m, p[0], p[1], p[2], &mut val)?;
                d2 = sse::narrow(sse::sub64(f64::from(d2), val));
            }
            d += 1.0;
        }
        Ok(sse::div32(d2, wsize as f32))
    }
}
