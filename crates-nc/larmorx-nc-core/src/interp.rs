// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.17: fsl-newimage 2203.12 (newimage.cc: interpolate,
// interpolatevalue, kernelinterpolation, splineinterpolate, forcesplinecoefcalculation) and
// fsl-miscmaths 2203.2 (miscmaths.cc: round; splinterpolator.h). Copyright (C) 1999-2018
// University of Oxford (Mark Jenkinson, Jesper Andersson et al.). Rust translation Copyright
// 2026 Karel Lopez Vilaret; modified from the originals. Distributed under the FSL Licence (see
// LICENSE).
//! `volume::interpolate(x, y, z)`: the value of a volume at a voxel position, by the volume's
//! interpolation method (trilinear, nearest neighbour, the windowed sinc kernel, cubic
//! B-splines) and extrapolation method.

use crate::cnum;
use crate::kernel::{Kernel, kernelval};
use crate::sse;
use crate::volume::{Elem, Extrapolation, Interpolation, Volume};

/// `q_tri_interpolation(v000, ..., dx, dy, dz)` (newimage.cc): x, then y, then z.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn q_tri(
    v000: f32,
    v001: f32,
    v010: f32,
    v011: f32,
    v100: f32,
    v101: f32,
    v110: f32,
    v111: f32,
    dx: f32,
    dy: f32,
    dz: f32,
) -> f32 {
    let temp1 = (v100 - v000) * dx + v000;
    let temp2 = (v101 - v001) * dx + v001;
    let temp3 = (v110 - v010) * dx + v010;
    let temp4 = (v111 - v011) * dx + v011;
    let temp5 = (temp3 - temp1) * dy + temp1;
    let temp6 = (temp4 - temp2) * dy + temp2;
    let r = (temp6 - temp5) * dz + temp5;
    if r.is_nan() {
        // A NaN or infinite neighbour: the same steps with SSE's NaNs (crate::sse).
        return q_tri_sse([v000, v001, v010, v011, v100, v101, v110, v111], dx, dy, dz);
    }
    r
}

/// [`q_tri`] with every operation's NaN as SSE gives it (operands in the source's order).
#[cold]
fn q_tri_sse(v: [f32; 8], dx: f32, dy: f32, dz: f32) -> f32 {
    use crate::sse::{add_f32 as add, mul_f32 as mul, sub_f32 as sub};
    let [v000, v001, v010, v011, v100, v101, v110, v111] = v;
    let lerp = |a: f32, b: f32, t: f32| add(mul(sub(b, a), t), a);
    let temp1 = lerp(v000, v100, dx);
    let temp2 = lerp(v001, v101, dx);
    let temp3 = lerp(v010, v110, dx);
    let temp4 = lerp(v011, v111, dx);
    let temp5 = lerp(temp1, temp3, dy);
    let temp6 = lerp(temp2, temp4, dy);
    lerp(temp5, temp6, dz)
}

/// `MISCMATHS::round(float x)`: `(int)(x + 0.5)` above 0, `(int)(x - 0.5)` otherwise (in
/// double, truncated as x86-64 does).
#[inline]
pub fn round_f32(x: f32) -> i32 {
    if x > 0.0 {
        cnum::f64_to_i32(f64::from(x) + 0.5)
    } else {
        cnum::f64_to_i32(f64::from(x) - 0.5)
    }
}

/// `in_neigh_bounds(vol, x, y, z)`.
#[inline]
fn in_neigh_bounds<T: Elem>(v: &Volume<T>, x: i64, y: i64, z: i64) -> bool {
    x >= 0 && y >= 0 && z >= 0 && x < v.xsize() - 1 && y < v.ysize() - 1 && z < v.zsize() - 1
}

/// `getneighbours(x, y, z, ...)`: the eight values of the cell at `(x, y, z)`, as `float`.
#[inline]
pub fn neighbours<T: Elem>(v: &Volume<T>, x: i64, y: i64, z: i64) -> [f32; 8] {
    let base = v.idx(x, y, z);
    let nx = v.xsize() as usize;
    let nxy = nx * v.ysize() as usize;
    let d = &v.data;
    // v000, v001, v010, v011, v100, v101, v110, v111
    [
        d[base].to_f32(),
        d[base + nxy].to_f32(),
        d[base + nx].to_f32(),
        d[base + nx + nxy].to_f32(),
        d[base + 1].to_f32(),
        d[base + 1 + nxy].to_f32(),
        d[base + 1 + nx].to_f32(),
        d[base + 1 + nx + nxy].to_f32(),
    ]
}

/// `floor` of a float (`floorf`) converted to `int64_t` as x86-64 does.
#[inline]
fn floor_i64(x: f32) -> i64 {
    cnum::f32_to_i64(x.floor())
}

/// `volume<T>::interpolate(x, y, z)` with the volume's methods, as `float`. `spline` holds the
/// spline coefficients for [`Interpolation::Spline`] ([`SplineCoefs::new`]); the sinc and
/// spline methods are translated for `volume<float>` only.
pub fn interpolate<T: Elem>(
    v: &Volume<T>,
    x: f32,
    y: f32,
    z: f32,
    spline: Option<&SplineCoefs>,
) -> f32 {
    match v.interp {
        Interpolation::NearestNeighbour => {
            let ix = i64::from(round_f32(x));
            let iy = i64::from(round_f32(y));
            let iz = i64::from(round_f32(z));
            v.get(ix, iy, iz).to_f32()
        }
        Interpolation::Trilinear => {
            let ix = floor_i64(x);
            let iy = floor_i64(y);
            let iz = floor_i64(z);
            let dx = x - ix as f32;
            let dy = y - iy as f32;
            let dz = z - iz as f32;
            let n = if in_neigh_bounds(v, ix, iy, iz) {
                neighbours(v, ix, iy, iz)
            } else {
                [
                    v.get(ix, iy, iz).to_f32(),
                    v.get(ix, iy, iz + 1).to_f32(),
                    v.get(ix, iy + 1, iz).to_f32(),
                    v.get(ix, iy + 1, iz + 1).to_f32(),
                    v.get(ix + 1, iy, iz).to_f32(),
                    v.get(ix + 1, iy, iz + 1).to_f32(),
                    v.get(ix + 1, iy + 1, iz).to_f32(),
                    v.get(ix + 1, iy + 1, iz + 1).to_f32(),
                ]
            };
            q_tri(n[0], n[1], n[2], n[3], n[4], n[5], n[6], n[7], dx, dy, dz)
        }
        Interpolation::Sinc => kernel_interpolation(float_only(v), x, y, z),
        Interpolation::Spline => spline_interpolate(
            float_only(v),
            x,
            y,
            z,
            spline.expect("spline coefficients"),
        ),
    }
}

/// The volume as `volume<float>`: the methods translated for float volumes only.
fn float_only<T: Elem>(v: &Volume<T>) -> &Volume {
    v.as_float()
        .expect("sinc and spline interpolation are translated for float volumes only")
}

impl<T: Elem> Volume<T> {
    /// `interpolate(x, y, z)` with the volume's interpolation and extrapolation (newimage's
    /// defaults: trilinear, zero outside); [`interpolate`] with spline coefficients.
    #[inline]
    pub fn interpolate(&self, x: f32, y: f32, z: f32) -> f32 {
        interpolate(self, x, y, z, None)
    }
}

/// `kernelinterpolation(x, y, z)` with the volume's sinc kernel.
pub fn kernel_interpolation(v: &Volume, x: f32, y: f32, z: f32) -> f32 {
    let Some(k) = v.kernel.as_deref() else {
        // "ERROR: Must set kernel parameters before using interpolation!": not reachable on
        // mcflirt's paths (setinterpolationmethod(sinc) defines the kernel).
        return v.extrapolate(0, 0, 0);
    };
    kernel_interp_with(v, k, x, y, z)
}

fn kernel_interp_with(v: &Volume, k: &Kernel, x: f32, y: f32, z: f32) -> f32 {
    let (wx, wy, wz) = (k.widthx, k.widthy, k.widthz);
    let ix0 = floor_i64(x);
    let iy0 = floor_i64(y);
    let iz0 = floor_i64(z);
    let mut storex = vec![0.0f32; (2 * wx + 1) as usize];
    let mut storey = vec![0.0f32; (2 * wy + 1) as usize];
    let mut storez = vec![0.0f32; (2 * wz + 1) as usize];
    for d in -wz..=wz {
        storez[(d + wz) as usize] = kernelval(z - iz0 as f32 + d as f32, wz, &k.kz);
    }
    for d in -wy..=wy {
        storey[(d + wy) as usize] = kernelval(y - iy0 as f32 + d as f32, wy, &k.ky);
    }
    for d in -wx..=wx {
        storex[(d + wx) as usize] = kernelval(x - ix0 as f32 + d as f32, wx, &k.kx);
    }
    let mut convsum: f32 = 0.0;
    let mut kersum: f32 = 0.0;
    // A NaN coordinate makes the start the most negative int64_t: FSL's loop bounds wrap (as
    // x86-64 integer arithmetic does), so its loops run no step; so do these, in every build.
    let (wx, wy, wz) = (i64::from(wx), i64::from(wy), i64::from(wz));
    for z1 in iz0.wrapping_sub(wz)..=iz0.wrapping_add(wz) {
        let zj = iz0.wrapping_sub(z1).wrapping_add(wz) as usize;
        for y1 in iy0.wrapping_sub(wy)..=iy0.wrapping_add(wy) {
            let yj = iy0.wrapping_sub(y1).wrapping_add(wy) as usize;
            for x1 in ix0.wrapping_sub(wx)..=ix0.wrapping_add(wx) {
                if v.in_bounds(x1, y1, z1) {
                    let xj = ix0.wrapping_sub(x1).wrapping_add(wx) as usize;
                    let kerfac = storex[xj] * storey[yj] * storez[zj];
                    convsum += v.get(x1, y1, z1) * kerfac;
                    kersum += kerfac;
                }
            }
        }
    }
    if convsum.is_nan() || kersum.is_nan() {
        // NaN or infinite voxels: the same sums with SSE's NaNs.
        (convsum, kersum) = (0.0, 0.0);
        for z1 in iz0.wrapping_sub(wz)..=iz0.wrapping_add(wz) {
            let zj = iz0.wrapping_sub(z1).wrapping_add(wz) as usize;
            for y1 in iy0.wrapping_sub(wy)..=iy0.wrapping_add(wy) {
                let yj = iy0.wrapping_sub(y1).wrapping_add(wy) as usize;
                for x1 in ix0.wrapping_sub(wx)..=ix0.wrapping_add(wx) {
                    if v.in_bounds(x1, y1, z1) {
                        let xj = ix0.wrapping_sub(x1).wrapping_add(wx) as usize;
                        let kerfac = sse::mul_f32(sse::mul_f32(storex[xj], storey[yj]), storez[zj]);
                        convsum = sse::add_f32(convsum, sse::mul_f32(v.get(x1, y1, z1), kerfac));
                        kersum = sse::add_f32(kersum, kerfac);
                    }
                }
            }
        }
        if f64::from(kersum.abs()) > 1e-9 {
            return sse::div_f32(convsum, kersum);
        }
        return v.extrapolate(ix0, iy0, iz0);
    }
    if f64::from(kersum.abs()) > 1e-9 {
        convsum / kersum
    } else {
        v.extrapolate(ix0, iy0, iz0)
    }
}

/// `in_bounds(float x, float y, float z)`: `floor` of each coordinate at least 0 and `ceil`
/// below the size.
fn in_bounds_f(v: &Volume, x: f32, y: f32, z: f32) -> bool {
    x.floor() >= 0.0
        && y.floor() >= 0.0
        && z.floor() >= 0.0
        && (x.ceil() as f64) < v.xsize() as f64
        && (y.ceil() as f64) < v.ysize() as f64
        && (z.ceil() as f64) < v.zsize() as f64
}

/// `in_extraslice_bounds(x, y, z)`.
fn in_extraslice_bounds(v: &Volume, x: f32, y: f32, z: f32) -> bool {
    let ix = floor_i64(x);
    let iy = floor_i64(y);
    let iz = floor_i64(z);
    ix >= -1 && iy >= -1 && iz >= -1 && ix < v.xsize() && iy < v.ysize() && iz < v.zsize()
}

/// The cubic B-spline coefficients of volume 0 (`forcesplinecoefcalculation()` with order 3 and
/// the volume's extrapolation, translated to the spline's: `extraslice` is `Constant`), as
/// `Splinterpolator<float>` computes them single-threaded.
#[derive(Clone, Debug)]
pub struct SplineCoefs {
    pub coef: Vec<f32>,
    pub dims: [usize; 3],
    pub constant: bool,
    pub zeros: bool,
}

/// `Splinterpolator` extrapolation types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplineExtrap {
    Zeros,
    Constant,
    Mirror,
    Periodic,
}

fn spline_extrap(e: Extrapolation) -> SplineExtrap {
    match e {
        Extrapolation::Extraslice => SplineExtrap::Constant,
        Extrapolation::Mirror => SplineExtrap::Mirror,
        Extrapolation::Periodic => SplineExtrap::Periodic,
        _ => SplineExtrap::Zeros,
    }
}

impl SplineCoefs {
    /// The coefficients of volume 0 of `v` (order 3).
    pub fn new(v: &Volume) -> SplineCoefs {
        let dims = [v.xsize() as usize, v.ysize() as usize, v.zsize() as usize];
        let n = dims[0] * dims[1] * dims[2];
        let mut coef = v.data[..n].to_vec();
        let et = spline_extrap(v.extrap);
        let steps = [1, dims[0], dims[0] * dims[1]];
        for dim in 0..3 {
            if dims[dim] <= 1 {
                continue;
            }
            // deconv_along(dim): the remaining dimensions in order, x fastest.
            let others: Vec<usize> = (0..3).filter(|&d| d != dim).collect();
            let (r0, r1) = (others[0], others[1]);
            let mut col = vec![0.0f64; dims[dim]];
            for j in 0..dims[r1] {
                for i in 0..dims[r0] {
                    let start = j * steps[r1] + i * steps[r0];
                    for (t, c) in col.iter_mut().enumerate() {
                        *c = f64::from(coef[start + t * steps[dim]]);
                    }
                    let line = col.clone();
                    deconv(&mut col, et);
                    if col.iter().any(|c| c.is_nan()) {
                        // NaN or infinite voxels on the line: SSE's NaNs.
                        col.copy_from_slice(&line);
                        deconv_sse(&mut col, et);
                    }
                    for (t, c) in col.iter().enumerate() {
                        coef[start + t * steps[dim]] = *c as f32;
                    }
                }
            }
        }
        SplineCoefs {
            coef,
            dims,
            constant: et == SplineExtrap::Constant,
            zeros: et == SplineExtrap::Zeros,
        }
    }

    fn et(&self) -> SplineExtrap {
        if self.constant {
            SplineExtrap::Constant
        } else if self.zeros {
            SplineExtrap::Zeros
        } else {
            SplineExtrap::Mirror
        }
    }

    /// `indx2indx(indx, d)`.
    fn indx2indx(&self, mut indx: i32, d: usize) -> usize {
        let dim = self.dims[d] as i32;
        if indx >= 0 && indx < dim {
            return indx as usize;
        }
        match self.et() {
            SplineExtrap::Constant => {
                if indx < 0 {
                    indx = 0;
                } else if indx >= dim {
                    indx = dim - 1;
                }
            }
            SplineExtrap::Zeros | SplineExtrap::Mirror => {
                while indx < 0 {
                    indx = 2 * dim * ((indx + 1) / dim) - 1 - indx;
                }
                while indx >= dim {
                    indx = 2 * dim * (indx / dim) - 1 - indx;
                }
            }
            SplineExtrap::Periodic => {
                while indx < 0 {
                    indx += dim;
                }
                while indx >= dim {
                    indx -= dim;
                }
            }
        }
        indx as usize
    }

    /// `value_at(coord)` for three dimensions, order 3.
    pub fn value_at(&self, coord: [f64; 3]) -> f64 {
        if self.zeros {
            for (i, &c) in coord.iter().enumerate() {
                if c < 0.0 || c > (self.dims[i] as f64 - 1.0) {
                    return 0.0;
                }
            }
        }
        let ni = 4usize;
        let mut inds = [0i32; 3];
        for i in 0..3 {
            let ix = cnum::f64_to_i32(coord[i] + 0.5);
            inds[i] = if f64::from(ix) < coord[i] {
                ix - 1
            } else {
                ix - 2
            };
        }
        let mut wgts = [[0.0f64; 4]; 3];
        for d in 0..3 {
            for (i, w) in wgts[d].iter_mut().enumerate() {
                *w = get_wgt3(coord[d] - f64::from(inds[d] + i as i32));
            }
        }
        let (d0, d1) = (self.dims[0], self.dims[1]);
        let mut val = 0.0f64;
        for k in 0..ni {
            let wgt1 = (1.0 * 1.0) * wgts[2][k];
            let linear1 = d0 * d1 * self.indx2indx(inds[2] + k as i32, 2);
            for j in 0..ni {
                let wgt2 = wgt1 * wgts[1][j];
                let linear2 = linear1 + d0 * self.indx2indx(inds[1] + j as i32, 1);
                for i in 0..ni {
                    let c = f64::from(self.coef[linear2 + self.indx2indx(inds[0] + i as i32, 0)]);
                    val += c * wgts[0][i] * wgt2;
                }
            }
        }
        if val.is_nan() {
            // NaN or infinite coefficients: the same sum with SSE's NaNs.
            use sse::{add_f64 as add, mul_f64 as mul};
            val = 0.0;
            for k in 0..ni {
                let wgt1 = (1.0 * 1.0) * wgts[2][k];
                let linear1 = d0 * d1 * self.indx2indx(inds[2] + k as i32, 2);
                for j in 0..ni {
                    let wgt2 = wgt1 * wgts[1][j];
                    let linear2 = linear1 + d0 * self.indx2indx(inds[1] + j as i32, 1);
                    for i in 0..ni {
                        let c =
                            f64::from(self.coef[linear2 + self.indx2indx(inds[0] + i as i32, 0)]);
                        val = add(val, mul(mul(c, wgts[0][i]), wgt2));
                    }
                }
            }
        }
        val
    }
}

/// `get_wgt(x)` for order 3.
fn get_wgt3(x: f64) -> f64 {
    let mut ax = x.abs();
    if ax < 1.0 {
        2.0 / 3.0 + 0.5 * ax * ax * (ax - 2.0)
    } else if ax < 2.0 {
        ax = 2.0 - ax;
        (1.0 / 6.0) * (ax * ax * ax)
    } else {
        0.0
    }
}

/// `SplineColumn::Deconv(3, et, 1e-8)`.
fn deconv(col: &mut [f64], et: SplineExtrap) {
    let sz = col.len();
    let z = 3.0f64.sqrt() - 2.0;
    let sf = 6u32;
    let prec = 1e-8f64;
    // init_fwd_sweep
    let n = ((larmorx_core::math::log(prec) / larmorx_core::math::log(z.abs())) + 1.5) as u32;
    let n = (n as usize).min(sz);
    let mut iv = col[0];
    let mut z2i = z;
    if et == SplineExtrap::Periodic {
        for i in 1..n {
            iv += z2i * col[sz - i];
            z2i *= z;
        }
    } else {
        for c in col.iter().take(n).skip(1) {
            iv += z2i * c;
            z2i *= z;
        }
    }
    col[0] = iv;
    let lv = col[sz - 1];
    for i in 1..sz {
        col[i] += z * col[i - 1];
    }
    // init_bwd_sweep
    col[sz - 1] = if et == SplineExtrap::Periodic {
        let mut iv = z * col[sz - 1];
        let mut z2i = z * z;
        for i in 1..n {
            iv += z2i * col[i - 1];
            z2i *= z;
        }
        iv / (z2i - 1.0)
    } else {
        -z / (1.0 - z * z) * (2.0 * col[sz - 1] - lv)
    };
    for i in (0..sz.saturating_sub(1)).rev() {
        col[i] = z * (col[i + 1] - col[i]);
    }
    for c in col.iter_mut() {
        *c *= f64::from(sf);
    }
}

/// [`deconv`] with every operation's NaN as SSE gives it (operands in the source's order).
#[cold]
fn deconv_sse(col: &mut [f64], et: SplineExtrap) {
    use sse::{add_f64 as add, div_f64 as div, mul_f64 as mul, sub_f64 as sub};
    let sz = col.len();
    let z = 3.0f64.sqrt() - 2.0;
    let sf = 6u32;
    let prec = 1e-8f64;
    let n = ((larmorx_core::math::log(prec) / larmorx_core::math::log(z.abs())) + 1.5) as u32;
    let n = (n as usize).min(sz);
    let mut iv = col[0];
    let mut z2i = z;
    if et == SplineExtrap::Periodic {
        for i in 1..n {
            iv = add(iv, mul(z2i, col[sz - i]));
            z2i *= z;
        }
    } else {
        for c in col.iter().take(n).skip(1) {
            iv = add(iv, mul(z2i, *c));
            z2i *= z;
        }
    }
    col[0] = iv;
    let lv = col[sz - 1];
    for i in 1..sz {
        col[i] = add(col[i], mul(z, col[i - 1]));
    }
    col[sz - 1] = if et == SplineExtrap::Periodic {
        let mut iv = mul(z, col[sz - 1]);
        let mut z2i = z * z;
        for i in 1..n {
            iv = add(iv, mul(z2i, col[i - 1]));
            z2i *= z;
        }
        div(iv, z2i - 1.0)
    } else {
        mul(-z / (1.0 - z * z), sub(mul(2.0, col[sz - 1]), lv))
    };
    for i in (0..sz.saturating_sub(1)).rev() {
        col[i] = mul(z, sub(col[i + 1], col[i]));
    }
    for c in col.iter_mut() {
        *c = mul(*c, f64::from(sf));
    }
}

/// `splineinterpolate(x, y, z)`.
fn spline_interpolate(v: &Volume, x: f32, y: f32, z: f32, s: &SplineCoefs) -> f32 {
    let ep = v.extrap;
    if !in_bounds_f(v, x, y, z) {
        match ep {
            Extrapolation::BoundsAssert | Extrapolation::Constpad => return v.padvalue,
            Extrapolation::Zeropad => return 0.0,
            _ => {}
        }
    }
    if ep == Extrapolation::Extraslice && !in_extraslice_bounds(v, x, y, z) {
        return v.padvalue;
    }
    s.value_at([f64::from(x), f64::from(y), f64::from(z)]) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An infinite neighbour weighted 0, and the infinities of both signs, give SSE's default
    /// NaN; a NaN neighbour is passed on with its bits, as FSL's x86-64 build does.
    #[test]
    fn trilinear_nans_as_sse() {
        let inf = f32::INFINITY;
        let v = [1.0, 2.0, 3.0, 4.0, inf, 6.0, 7.0, 8.0];
        // dx = 0: (inf - 1) * 0 = NaN made from numbers.
        let r = q_tri(
            v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], 0.0, 0.5, 0.5,
        );
        assert_eq!(r.to_bits(), 0xffc0_0000);
        let w = [1.0, 2.0, 3.0, 4.0, inf, 6.0, -inf, 8.0];
        let r = q_tri(
            w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7], 0.5, 0.5, 0.5,
        );
        assert_eq!(r.to_bits(), 0xffc0_0000);
        let p = f32::from_bits(0x7fc1_2345);
        let r = q_tri(1.0, 2.0, 3.0, 4.0, p, 6.0, 7.0, 8.0, 0.5, 0.5, 0.5);
        assert_eq!(r.to_bits(), 0x7fc1_2345);
        assert_eq!(
            q_tri(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 0.5, 0.5, 0.5),
            4.5
        );
    }
}
