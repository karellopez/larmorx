// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-newimage 2601.0 (newimage.h: extrapparams, valid,
// in_bounds; newimage.cc: interpolate, interp1partial, interp3partial, spline_interp1partial,
// spline_interp3partial, splineinterpolate, extrapolate, forcesplinecoefcalculation,
// translate_extrapolation_type). Copyright (C) 1999-2025 University of Oxford (Mark Jenkinson,
// Jesper Andersson et al.). Rust translation Copyright 2026 Karel Lopez Vilaret; modified from
// the originals. Distributed under the FSL Licence (see LICENSE).
//! newimage's interpolation of a `volume<float>` with extrapolation parameters
//! (`volume<T>::extrapparams`) and with partial derivatives (`interp1partial`,
//! `interp3partial`), as fsl-warpfns' resampling calls it, over the general spline interpolator
//! ([`crate::splinterpolator`]: any order, periodic extrapolation).
//!
//! [`Interpolator`] holds what FSL's volume computes on first use: the spline coefficients,
//! from the volume's own extrapolation method and spline order (`forcesplinecoefcalculation`;
//! FSL recomputes them when other extrapolation parameters are passed, with the volume's own
//! method again, so they are always these). The extrapolation parameters decide the rest: the
//! values outside the volume, the early returns of the spline paths, `valid()`.

use crate::interp::{self, q_tri, round_f32};
use crate::splinterpolator::{ExtrapolationType, Splinterpolator};
use crate::sse;
use crate::volume::{Extrapolation, FslError, Interpolation, Volume, periodicclamp};

/// `volume<T>::extrapparams`: each field `None` takes the volume's own setting.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ExtrapParams {
    pub method: Option<Extrapolation>,
    pub padvalue: Option<f32>,
    pub validity: Option<[bool; 3]>,
}

impl ExtrapParams {
    /// `extrapparams()`: the volume's own settings.
    pub const DEFAULT: ExtrapParams = ExtrapParams {
        method: None,
        padvalue: None,
        validity: None,
    };

    /// `extrapparams(meth)`.
    pub fn with_method(method: Extrapolation) -> ExtrapParams {
        ExtrapParams {
            method: Some(method),
            ..ExtrapParams::DEFAULT
        }
    }

    /// `method(vol)`.
    pub fn method<T: crate::volume::Elem>(&self, v: &Volume<T>) -> Extrapolation {
        self.method.unwrap_or(v.extrap)
    }

    /// `padvalue(vol)`.
    pub fn padvalue(&self, v: &Volume) -> f32 {
        self.padvalue.unwrap_or(v.padvalue)
    }

    /// `xvalid(vol)`, `yvalid(vol)`, `zvalid(vol)`.
    pub fn validity<T: crate::volume::Elem>(&self, v: &Volume<T>) -> [bool; 3] {
        self.validity.unwrap_or(v.ep_valid)
    }
}

/// `translate_extrapolation_type(ep)`: newimage's extrapolation as the spline interpolator's
/// (`extraslice` is `Constant`; `constpad` is `Zeros`, its pad value returned before the
/// interpolator is reached).
pub fn translate_extrapolation_type(ep: Extrapolation) -> Result<ExtrapolationType, FslError> {
    Ok(match ep {
        Extrapolation::Zeropad | Extrapolation::Constpad => ExtrapolationType::Zeros,
        Extrapolation::Extraslice => ExtrapolationType::Constant,
        Extrapolation::Mirror => ExtrapolationType::Mirror,
        Extrapolation::Periodic => ExtrapolationType::Periodic,
        Extrapolation::BoundsAssert | Extrapolation::BoundsException => {
            return Err(FslError::imthrow(
                "translate_extrapolation_type: I am lost",
                10,
            ));
        }
    })
}

/// `valid(x, y, z, tol, ep)` (float coordinates): inside the volume, within `tol`, along each
/// direction whose extrapolation is not valid.
pub fn valid_f<T: crate::volume::Elem>(
    v: &Volume<T>,
    x: f32,
    y: f32,
    z: f32,
    tol: f64,
    ep: &ExtrapParams,
) -> bool {
    let [xv, yv, zv] = ep.validity(v);
    let inside = |c: f32, n: i64| {
        let c = f64::from(c);
        c + tol >= 0.0 && c <= (n - 1) as f64 + tol
    };
    (xv || inside(x, v.xsize())) && (yv || inside(y, v.ysize())) && (zv || inside(z, v.zsize()))
}

/// `valid(x, y, z, ep)` (integer coordinates).
pub fn valid_i<T: crate::volume::Elem>(
    v: &Volume<T>,
    x: i64,
    y: i64,
    z: i64,
    ep: &ExtrapParams,
) -> bool {
    let [xv, yv, zv] = ep.validity(v);
    (xv || (x >= 0 && x < v.xsize()))
        && (yv || (y >= 0 && y < v.ysize()))
        && (zv || (z >= 0 && z < v.zsize()))
}

/// `periodicclamp(x, x1, x2)` as newimage's `extrapolate` calls it: miscmaths' `int` version
/// with the `int64_t` index converted to `int` (only indices beyond 2^31 differ from
/// [`periodicclamp`]: those of an infinite or NaN position, which x86-64 converts to
/// `INT64_MIN`, wrap to 0).
pub fn periodicclamp_int(x: i64, x1: i64, x2: i64) -> i64 {
    periodicclamp(i64::from(x as i32), x1, x2)
}

/// newimage's `mirrorclamp(x, x1, x2)` (`int64_t`, its `periodicclamp` miscmaths' `int` one).
pub fn mirrorclamp_int(x: i64, x1: i64, x2: i64) -> i64 {
    if x2 < x1 {
        return mirrorclamp_int(x, x2, x1);
    }
    if x1 == x2 {
        return x1;
    }
    let x3 = 2 * x2 - x1 + 1;
    let mut nx = periodicclamp_int(x, x1, x3);
    if nx > x2 {
        nx = 2 * x2 + 1 - nx;
    }
    nx
}

/// The voxel `(x, y, z)` of volume `t`, or the value `extrapolate(x, y, z, ep)` gives for a
/// voxel outside it (`operator()(x, y, z, [t,] ep)`). `Err` is FSL's error for an extrapolation
/// method `extrapolate` does not handle (`boundsassert`, `boundsexception`).
pub fn value_ep(
    v: &Volume,
    t: usize,
    x: i64,
    y: i64,
    z: i64,
    ep: &ExtrapParams,
) -> Result<f32, FslError> {
    let (nx, ny, nz) = (v.xsize(), v.ysize(), v.zsize());
    let off = t * (nx * ny * nz) as usize;
    let at = |i: i64, j: i64, k: i64| v.data[off + ((k * ny + j) * nx + i) as usize];
    if v.in_bounds(x, y, z) {
        return Ok(at(x, y, z));
    }
    match ep.method(v) {
        Extrapolation::Zeropad => Ok(0.0),
        Extrapolation::Constpad => Ok(ep.padvalue(v)),
        Extrapolation::Periodic => Ok(at(
            periodicclamp_int(x, 0, nx - 1),
            periodicclamp_int(y, 0, ny - 1),
            periodicclamp_int(z, 0, nz - 1),
        )),
        Extrapolation::Mirror => Ok(at(
            mirrorclamp_int(x, 0, nx - 1),
            mirrorclamp_int(y, 0, ny - 1),
            mirrorclamp_int(z, 0, nz - 1),
        )),
        Extrapolation::Extraslice => {
            let edge = |c: i64, n: i64| {
                if c == -1 {
                    0
                } else if c == n {
                    n - 1
                } else {
                    c
                }
            };
            let (ex, ey, ez) = (edge(x, nx), edge(y, ny), edge(z, nz));
            if v.in_bounds(ex, ey, ez) {
                Ok(at(ex, ey, ez))
            } else {
                Ok(ep.padvalue(v))
            }
        }
        Extrapolation::BoundsAssert | Extrapolation::BoundsException => {
            Err(FslError::imthrow("Invalid extrapolation method", 6))
        }
    }
}

/// `in_bounds(x, y, z)` for float coordinates: `floor >= 0` and `ceil < size` along each axis.
fn in_bounds_f(v: &Volume, x: f32, y: f32, z: f32) -> bool {
    x.floor() >= 0.0
        && y.floor() >= 0.0
        && z.floor() >= 0.0
        && x.ceil() < v.xsize() as f32
        && y.ceil() < v.ysize() as f32
        && z.ceil() < v.zsize() as f32
}

/// `in_extraslice_bounds(x, y, z)`.
fn in_extraslice_bounds(v: &Volume, x: f32, y: f32, z: f32) -> bool {
    let ix = floor_i64(x);
    let iy = floor_i64(y);
    let iz = floor_i64(z);
    ix >= -1 && iy >= -1 && iz >= -1 && ix < v.xsize() && iy < v.ysize() && iz < v.zsize()
}

/// `(int64_t) floor(x)` as x86-64 converts it.
#[inline]
fn floor_i64(x: f32) -> i64 {
    crate::cnum::f32_to_i64(x.floor())
}

/// `in_neigh_bounds(vol, x, y, z)`.
#[inline]
fn in_neigh_bounds(v: &Volume, x: i64, y: i64, z: i64) -> bool {
    x >= 0 && y >= 0 && z >= 0 && x < v.xsize() - 1 && y < v.ysize() - 1 && z < v.zsize() - 1
}

/// The image FSL's volume interpolates, with what it computes on first use.
#[derive(Clone, Debug)]
pub struct Interpolator<'a> {
    v: &'a Volume,
    /// Volume `t` of a 4D image (`operator[](t)`), as offset of its first value.
    off: usize,
    /// Whether the image is more than 3D (FSL's 3D-only methods throw).
    not3d: bool,
    splint: Option<Splinterpolator<f32>>,
}

impl<'a> Interpolator<'a> {
    /// The interpolator of `v` (volume 0, or the whole image when it is more than 3D, as a
    /// `volume<float>` that FSL's 3D-only methods refuse). The spline coefficients are
    /// computed now when `v` interpolates by splines (FSL computes them at the first spline
    /// interpolation; the errors are the same).
    pub fn new(v: &'a Volume) -> Result<Interpolator<'a>, FslError> {
        let not3d = v.dimensionality() > 3;
        Interpolator::build(v, 0, not3d)
    }

    /// The interpolator of volume `t` of `v` (`v[t]`, a 3D volume with `v`'s properties).
    pub fn of_volume(v: &'a Volume, t: i64) -> Result<Interpolator<'a>, FslError> {
        if t < 0 || t >= v.tsize() {
            return Err(FslError::imthrow("Invalid t index in [] operator", 61));
        }
        Interpolator::build(v, t as usize, false)
    }

    fn build(v: &'a Volume, t: usize, not3d: bool) -> Result<Interpolator<'a>, FslError> {
        let n = v.nvoxels() as usize;
        let off = t * n;
        let splint = if v.interp == Interpolation::Spline {
            // forcesplinecoefcalculation()
            if not3d {
                return Err(not_3d());
            }
            let et = translate_extrapolation_type(v.extrap)?;
            if v.splineorder > 7 || v.splineorder < 0 {
                return Err(FslError::imthrow(
                    "setsplineorder: Only splines of order up to 7 allowed",
                    10,
                ));
            }
            let dim = [v.xsize() as u32, v.ysize() as u32, v.zsize() as u32];
            Some(Splinterpolator::new(
                &v.data[off..off + n],
                &dim,
                &[et; 3],
                v.splineorder as u32,
                1e-8,
                false,
            )?)
        } else {
            None
        };
        Ok(Interpolator {
            v,
            off,
            not3d,
            splint,
        })
    }

    /// The volume interpolated.
    pub fn volume(&self) -> &Volume {
        self.v
    }

    /// The spline coefficients (spline interpolation only).
    pub fn spline(&self) -> Option<&Splinterpolator<f32>> {
        self.splint.as_ref()
    }

    #[inline]
    fn t(&self) -> usize {
        self.off / (self.v.nvoxels().max(1) as usize)
    }

    /// `operator()(x, y, z, ep)` on this volume.
    #[inline]
    fn get(&self, x: i64, y: i64, z: i64, ep: &ExtrapParams) -> Result<f32, FslError> {
        if self.v.in_bounds(x, y, z) {
            let (nx, ny) = (self.v.xsize(), self.v.ysize());
            return Ok(self.v.data[self.off + ((z * ny + y) * nx + x) as usize]);
        }
        let m = ep.method(self.v);
        if self.not3d && !matches!(m, Extrapolation::Zeropad | Extrapolation::Constpad) {
            return Err(not_3d());
        }
        value_ep(self.v, self.t(), x, y, z, ep)
    }

    /// The eight values of the cell at `(ix, iy, iz)`: v000, v001, v010, v011, v100, v101,
    /// v110, v111 (inside: `getneighbours`; else each through `operator()(..., ep)`).
    fn cell(&self, ix: i64, iy: i64, iz: i64, ep: &ExtrapParams) -> Result<[f32; 8], FslError> {
        if in_neigh_bounds(self.v, ix, iy, iz) {
            let (nx, ny) = (self.v.xsize() as usize, self.v.ysize() as usize);
            let nxy = nx * ny;
            let base = self.off + (iz as usize * ny + iy as usize) * nx + ix as usize;
            let d = &self.v.data;
            return Ok([
                d[base],
                d[base + nxy],
                d[base + nx],
                d[base + nx + nxy],
                d[base + 1],
                d[base + 1 + nxy],
                d[base + 1 + nx],
                d[base + 1 + nx + nxy],
            ]);
        }
        Ok([
            self.get(ix, iy, iz, ep)?,
            self.get(ix, iy, iz + 1, ep)?,
            self.get(ix, iy + 1, iz, ep)?,
            self.get(ix, iy + 1, iz + 1, ep)?,
            self.get(ix + 1, iy, iz, ep)?,
            self.get(ix + 1, iy, iz + 1, ep)?,
            self.get(ix + 1, iy + 1, iz, ep)?,
            self.get(ix + 1, iy + 1, iz + 1, ep)?,
        ])
    }

    /// `interpolate(x, y, z, ep)`: the value by the volume's interpolation method.
    pub fn interpolate(&self, x: f32, y: f32, z: f32, ep: &ExtrapParams) -> Result<f32, FslError> {
        if self.not3d {
            return Err(not_3d());
        }
        match self.v.interp {
            Interpolation::NearestNeighbour => self.get(
                i64::from(round_f32(x)),
                i64::from(round_f32(y)),
                i64::from(round_f32(z)),
                ep,
            ),
            Interpolation::Trilinear => {
                let (ix, iy, iz) = (floor_i64(x), floor_i64(y), floor_i64(z));
                let n = self.cell(ix, iy, iz, ep)?;
                let (dx, dy, dz) = (x - ix as f32, y - iy as f32, z - iz as f32);
                Ok(q_tri(
                    n[0], n[1], n[2], n[3], n[4], n[5], n[6], n[7], dx, dy, dz,
                ))
            }
            Interpolation::Sinc => {
                // kernelinterpolation(x, y, z, ep): ep reaches only its fallback for a lost
                // kernel sum, which interp::kernel_interpolation takes from the volume's own
                // settings; only those are translated (fsl-warpfns passes no others to the
                // image it resamples).
                let own =
                    ep.method(self.v) == self.v.extrap && ep.padvalue(self.v) == self.v.padvalue;
                if !own || self.off != 0 {
                    return Err(FslError {
                        printed: Vec::new(),
                        type_name: "larmorx",
                        what: "sinc interpolation with other extrapolation parameters, or of a \
                               volume of a 4D image, is not translated"
                            .into(),
                    });
                }
                Ok(interp::kernel_interpolation(self.v, x, y, z))
            }
            Interpolation::Spline => self.splineinterpolate(x, y, z, ep),
        }
    }

    /// `splineinterpolate(x, y, z, ep)`.
    fn splineinterpolate(
        &self,
        x: f32,
        y: f32,
        z: f32,
        ep: &ExtrapParams,
    ) -> Result<f32, FslError> {
        let m = ep.method(self.v);
        if !in_bounds_f(self.v, x, y, z) {
            match m {
                Extrapolation::Zeropad => return Ok(0.0),
                Extrapolation::Constpad => return Ok(self.v.padvalue),
                _ => {}
            }
        }
        if m == Extrapolation::Extraslice && !in_extraslice_bounds(self.v, x, y, z) {
            return Ok(self.v.padvalue);
        }
        translate_extrapolation_type(m)?;
        let s = self.splint.as_ref().expect("spline coefficients");
        s.at(f64::from(x), f64::from(y), f64::from(z), 0.0)
    }

    /// `interp1partial(x, y, z, dir, &deriv, ep)`: the value and its derivative along `dir`
    /// (0, 1, 2), per voxel, by trilinear or spline interpolation.
    pub fn interp1partial(
        &self,
        x: f32,
        y: f32,
        z: f32,
        dir: i32,
        ep: &ExtrapParams,
    ) -> Result<(f32, f32), FslError> {
        let method = self.v.interp;
        if method != Interpolation::Trilinear && method != Interpolation::Spline {
            return Err(FslError::imthrow(
                "Derivatives only implemented for tri-linear and spline interpolation",
                10,
            ));
        }
        if !(0..=2).contains(&dir) {
            return Err(FslError::imthrow("Ivalid derivative direction", 11));
        }
        if method == Interpolation::Trilinear {
            let (ix, iy, iz) = (floor_i64(x), floor_i64(y), floor_i64(z));
            let (dx, dy, dz) = (x - ix as f32, y - iy as f32, z - iz as f32);
            let n = self.cell(ix, iy, iz, ep)?;
            let r = tri1::<Plain32>(n, dx, dy, dz, dir);
            if r.0.is_nan() || r.1.is_nan() {
                return Ok(tri1::<Sse32>(n, dx, dy, dz, dir));
            }
            return Ok(r);
        }
        // spline_interp1partial
        let m = ep.method(self.v);
        if !in_bounds_f(self.v, x, y, z) {
            match m {
                Extrapolation::Zeropad => return Ok((0.0, 0.0)),
                Extrapolation::Constpad => return Ok((self.v.padvalue, 0.0)),
                _ => {}
            }
        }
        translate_extrapolation_type(m)?;
        let s = self.splint.as_ref().expect("spline coefficients");
        s.at_deriv(f64::from(x), f64::from(y), f64::from(z), dir as u32)
    }

    /// `interp3partial(x, y, z, &dfdx, &dfdy, &dfdz, ep)`: the value and its three derivatives
    /// (per voxel), by trilinear or spline interpolation.
    pub fn interp3partial(
        &self,
        x: f32,
        y: f32,
        z: f32,
        ep: &ExtrapParams,
    ) -> Result<(f32, [f32; 3]), FslError> {
        let method = self.v.interp;
        if method != Interpolation::Trilinear && method != Interpolation::Spline {
            return Err(FslError::imthrow(
                "interp3partial: Derivatives only implemented for tri-linear and spline \
                 interpolation",
                10,
            ));
        }
        if method == Interpolation::Trilinear {
            let (ix, iy, iz) = (floor_i64(x), floor_i64(y), floor_i64(z));
            let (dx, dy, dz) = (x - ix as f32, y - iy as f32, z - iz as f32);
            let n = self.cell(ix, iy, iz, ep)?;
            let r = tri3::<Plain32>(n, dx, dy, dz);
            if r.0.is_nan() || r.1.iter().any(|v| v.is_nan()) {
                return Ok(tri3::<Sse32>(n, dx, dy, dz));
            }
            return Ok(r);
        }
        // spline_interp3partial
        let m = ep.method(self.v);
        if !in_bounds_f(self.v, x, y, z) {
            match m {
                Extrapolation::Zeropad => return Ok((0.0, [0.0; 3])),
                Extrapolation::Constpad => return Ok((self.v.padvalue, [0.0; 3])),
                _ => {}
            }
        }
        translate_extrapolation_type(m)?;
        let s = self.splint.as_ref().expect("spline coefficients");
        s.val_and_derivs3(f64::from(x), f64::from(y), f64::from(z))
    }
}

fn not_3d() -> FslError {
    FslError::imthrow("3D only method called by higher-dimensional volume.", 75)
}

/// The arithmetic of the trilinear partials: `float` operations, and the `double` ones of
/// `(1.0 - d) * t` (a `double` times a `float`); plain or with SSE's NaNs, operands in the
/// source's order.
trait Arith32 {
    fn add(a: f32, b: f32) -> f32;
    fn sub(a: f32, b: f32) -> f32;
    fn mul(a: f32, b: f32) -> f32;
    fn add64(a: f64, b: f64) -> f64;
    fn mul64(a: f64, b: f64) -> f64;
}

struct Plain32;
struct Sse32;

impl Arith32 for Plain32 {
    #[inline(always)]
    fn add(a: f32, b: f32) -> f32 {
        a + b
    }
    #[inline(always)]
    fn sub(a: f32, b: f32) -> f32 {
        a - b
    }
    #[inline(always)]
    fn mul(a: f32, b: f32) -> f32 {
        a * b
    }
    #[inline(always)]
    fn add64(a: f64, b: f64) -> f64 {
        a + b
    }
    #[inline(always)]
    fn mul64(a: f64, b: f64) -> f64 {
        a * b
    }
}

impl Arith32 for Sse32 {
    #[inline(always)]
    fn add(a: f32, b: f32) -> f32 {
        sse::add_f32(a, b)
    }
    #[inline(always)]
    fn sub(a: f32, b: f32) -> f32 {
        sse::sub_f32(a, b)
    }
    #[inline(always)]
    fn mul(a: f32, b: f32) -> f32 {
        sse::mul_f32(a, b)
    }
    #[inline(always)]
    fn add64(a: f64, b: f64) -> f64 {
        sse::add_f64(a, b)
    }
    #[inline(always)]
    fn mul64(a: f64, b: f64) -> f64 {
        sse::mul_f64(a, b)
    }
}

/// `(1.0 - d)` (a `double`) and `(1.0 - d) * a + d * b`, rounded to `float`.
#[inline(always)]
fn lerp64<A: Arith32>(d: f32, a: f32, b: f32) -> f32 {
    let omd = 1.0 - f64::from(d);
    A::add64(A::mul64(omd, f64::from(a)), f64::from(A::mul(d, b))) as f32
}

/// interp1partial's trilinear branch (newimage.cc): the value and the derivative along `dir`.
#[inline(always)]
fn tri1<A: Arith32>(n: [f32; 8], dx: f32, dy: f32, dz: f32, dir: i32) -> (f32, f32) {
    let [v000, v001, v010, v011, v100, v101, v110, v111] = n;
    // `float onem = 1.0 - d` (computed in double, stored as float)
    let onem = |d: f32| (1.0 - f64::from(d)) as f32;
    let l32 = |o: f32, d: f32, a: f32, b: f32| A::add(A::mul(o, a), A::mul(d, b));
    let (tmp21, tmp22, last) = match dir {
        0 => {
            let o = onem(dz);
            let t11 = l32(o, dz, v000, v001);
            let t12 = l32(o, dz, v010, v011);
            let t13 = l32(o, dz, v100, v101);
            let t14 = l32(o, dz, v110, v111);
            (lerp64::<A>(dy, t11, t12), lerp64::<A>(dy, t13, t14), dx)
        }
        1 => {
            let o = onem(dz);
            let t11 = l32(o, dz, v000, v001);
            let t12 = l32(o, dz, v010, v011);
            let t13 = l32(o, dz, v100, v101);
            let t14 = l32(o, dz, v110, v111);
            (lerp64::<A>(dx, t11, t13), lerp64::<A>(dx, t12, t14), dy)
        }
        _ => {
            let o = onem(dy);
            let t11 = l32(o, dy, v000, v010);
            let t12 = l32(o, dy, v001, v011);
            let t13 = l32(o, dy, v100, v110);
            let t14 = l32(o, dy, v101, v111);
            (lerp64::<A>(dx, t11, t13), lerp64::<A>(dx, t12, t14), dz)
        }
    };
    let deriv = A::sub(tmp22, tmp21);
    // `return((1.0-d)*tmp21 + d*tmp22)`: a double, returned as float
    (lerp64::<A>(last, tmp21, tmp22), deriv)
}

/// interp3partial's trilinear branch (newimage.cc): the value and its three derivatives.
#[inline(always)]
fn tri3<A: Arith32>(n: [f32; 8], dx: f32, dy: f32, dz: f32) -> (f32, [f32; 3]) {
    let [v000, v001, v010, v011, v100, v101, v110, v111] = n;
    let onemdz = (1.0 - f64::from(dz)) as f32;
    let onemdy = (1.0 - f64::from(dy)) as f32;
    let l32 = |o: f32, d: f32, a: f32, b: f32| A::add(A::mul(o, a), A::mul(d, b));
    let tmp11 = l32(onemdz, dz, v000, v001);
    let tmp12 = l32(onemdz, dz, v010, v011);
    let tmp13 = l32(onemdz, dz, v100, v101);
    let tmp14 = l32(onemdz, dz, v110, v111);
    // *dfdx = onemdy*(tmp13-tmp11) + dy*(tmp14-tmp12)
    let dfdx = A::add(
        A::mul(onemdy, A::sub(tmp13, tmp11)),
        A::mul(dy, A::sub(tmp14, tmp12)),
    );
    // *dfdy = (1.0-dx)*(tmp12-tmp11) + dx*(tmp14-tmp13)
    let dfdy = lerp64::<A>(dx, A::sub(tmp12, tmp11), A::sub(tmp14, tmp13));
    let tmp11 = l32(onemdy, dy, v000, v010);
    let tmp12 = l32(onemdy, dy, v001, v011);
    let tmp13 = l32(onemdy, dy, v100, v110);
    let tmp14 = l32(onemdy, dy, v101, v111);
    let tmp21 = lerp64::<A>(dx, tmp11, tmp13);
    let tmp22 = lerp64::<A>(dx, tmp12, tmp14);
    let dfdz = A::sub(tmp22, tmp21);
    // return(onemdz*tmp21 + dz*tmp22): float
    let val = l32(onemdz, dz, tmp21, tmp22);
    (val, [dfdx, dfdy, dfdz])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trilinear interpolation through the interpolator is newimage's `interpolate` (the
    /// existing translation), inside and outside the volume, for every extrapolation.
    #[test]
    fn trilinear_matches_interp() {
        let mut v: Volume = Volume::new(5, 4, 3, 1);
        for (i, d) in v.data.iter_mut().enumerate() {
            *d = ((i * 37) % 23) as f32 * 0.75 - 4.0;
        }
        v.padvalue = 2.5;
        for ext in [
            Extrapolation::Zeropad,
            Extrapolation::Constpad,
            Extrapolation::Extraslice,
            Extrapolation::Mirror,
            Extrapolation::Periodic,
        ] {
            v.extrap = ext;
            let it = Interpolator::new(&v).unwrap();
            for k in -8..32 {
                let x = k as f32 * 0.23 - 1.7;
                let y = k as f32 * 0.17 - 1.1;
                let z = k as f32 * 0.11 - 0.9;
                let a = it.interpolate(x, y, z, &ExtrapParams::DEFAULT).unwrap();
                let b = interp::interpolate(&v, x, y, z, None);
                assert_eq!(a.to_bits(), b.to_bits(), "{ext:?} {x} {y} {z}");
                let (val, _) = it.interp3partial(x, y, z, &ExtrapParams::DEFAULT).unwrap();
                // interp3partial's own formula: equal to q_tri up to its rounding order; both
                // are checked against FSL by validation/probes/fsl-warpfns.
                assert!(
                    (val - a).abs() <= 1e-4 * (1.0 + a.abs()),
                    "{ext:?} {val} {a}"
                );
            }
        }
    }
}
