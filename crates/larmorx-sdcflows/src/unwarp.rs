// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! `B0FieldTransform.apply`'s unwarping of one volume (sdcflows 2.17.1 `sdcflows/transform.py`,
//! `_sdc_unwarp`; Apache-2.0): every voxel moved along the phase-encoding axis by the field
//! (Hz) times the readout time, the volume interpolated there with SciPy's
//! `ndimage.map_coordinates` (`larmorx-interp`'s replica), and optionally multiplied by the
//! Jacobian determinant of the shift, `1 + np.gradient(vsm, axis=pe)`.
//!
//! numpy's type rules decide the bits (numpy 2.5.4 `numpy/lib/_function_base_impl.py`,
//! `gradient`; NEP 50; BSD-3-Clause, Copyright NumPy Developers): the field is float32
//! (`mapped.get_fdata(dtype='float32')`) and the voxel coordinates are float32
//! (nitransforms' `ndindex`, cast). A readout time given as a Python float is a "weak" scalar,
//! so the voxel-shift map is float32 (`fmap * float32(ro)`) and so are the shifted coordinate
//! (`float32 + float32`), the gradient and the Jacobian product; a numpy float64 readout time
//! makes the shift map float64, its sum with the float32 coordinate is rounded back into the
//! float32 coordinate array, and the gradient and the product are float64 (rounded into the
//! float32 output). The gradient is numpy's: central differences `(f[i+1] - f[i-1]) / 2.0` inside,
//! one-sided `(f[1] - f[0]) / 1.0` and `(f[n-1] - f[n-2]) / 1.0` at the two ends
//! (`edge_order=1`, unit spacing).

use larmorx_core::defects::{Notice, UpstreamDefects};
use larmorx_interp::ndimage::{self, Mode, Sample};
use rayon::prelude::*;

use crate::SdcError;

/// numpy's message when an axis is too short for `np.gradient` (`edge_order` 1).
pub const GRADIENT_TOO_SMALL: &str = "Shape of array too small to calculate a numerical \
     gradient, at least (edge_order + 1) elements are required.";

/// The readout time of a volume, with the numpy type sdcflows multiplies the field by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReadoutTime {
    /// A Python `float` (NEP 50's weak scalar): `fmap * ro` is float32.
    Weak(f64),
    /// A numpy `float64` scalar: `fmap * ro` is float64.
    Float64(f64),
}

/// The interpolation and the Jacobian of [`unwarp_volume`] (`B0FieldTransform.apply`'s
/// `order`, `mode`, `cval`, `prefilter`, `jacobian`).
#[derive(Clone, Copy, Debug)]
pub struct UnwarpOptions {
    /// Spline order, 0 to 5 (`order=3`).
    pub order: u32,
    /// SciPy's boundary mode (`mode='constant'`).
    pub mode: Mode,
    /// The value outside the volume for the constant modes (`cval=0.0`).
    pub cval: f64,
    /// Spline prefilter (`prefilter=True`).
    pub prefilter: bool,
    /// Multiply by `1 + np.gradient(vsm, axis=pe)` (`jacobian=True`).
    pub jacobian: bool,
    /// SciPy's `reflect` prefilter boundary (SCI-03; the fixed SciPy 1.18 behaviour either way).
    pub defects: UpstreamDefects,
}

impl Default for UnwarpOptions {
    fn default() -> Self {
        UnwarpOptions {
            order: 3,
            mode: Mode::Constant,
            cval: 0.0,
            prefilter: true,
            jacobian: true,
            defects: UpstreamDefects::Fix,
        }
    }
}

/// The voxel-shift map, in the type numpy computes it in.
enum Shift {
    F32(Vec<f32>),
    F64(Vec<f64>),
}

/// `_sdc_unwarp` for one volume (no head-motion transform: `apply` ignores them).
///
/// `volume` holds the volume's values in Fortran order (`shape`, any [`Sample`] type: the
/// moving image's in-memory type, as `map_coordinates` receives it); `fmap_hz` the field in
/// Hz on the same grid, in C order (float32); `pe_axis` the array axis of the phase encoding
/// (0, 1 or 2), and `ro` the signed readout time (negative when the displacements are
/// reversed). Returns the unwarped volume (float32, C order) and the notices of the
/// interpolation. Runs on the current rayon pool; the result does not depend on it.
pub fn unwarp_volume<T: Sample>(
    volume: &[T],
    shape: [usize; 3],
    fmap_hz: &[f32],
    pe_axis: usize,
    ro: ReadoutTime,
    options: &UnwarpOptions,
) -> Result<(Vec<f32>, Vec<Notice>), SdcError> {
    let n: usize = shape.iter().product();
    if volume.len() != n || fmap_hz.len() != n {
        return Err(SdcError::Invalid(format!(
            "the volume ({}) and the field ({}) must both have {n} voxels",
            volume.len(),
            fmap_hz.len()
        )));
    }
    if pe_axis > 2 {
        return Err(SdcError::Invalid(format!(
            "phase-encoding axis {pe_axis} is not 0, 1 or 2"
        )));
    }
    if options.jacobian && shape[pe_axis] < 2 {
        return Err(SdcError::Invalid(GRADIENT_TOO_SMALL.to_owned()));
    }
    let shift = match ro {
        ReadoutTime::Weak(t) => {
            let t32 = t as f32;
            Shift::F32(fmap_hz.par_iter().map(|&f| f * t32).collect())
        }
        ReadoutTime::Float64(t) => {
            Shift::F64(fmap_hz.par_iter().map(|&f| f64::from(f) * t).collect())
        }
    };
    let [_, nj, nk] = shape;
    let coordinates: Vec<[f64; 3]> = (0..n)
        .into_par_iter()
        .map(|m| {
            let idx = [m / (nj * nk), (m / nk) % nj, m % nk];
            let mut c = idx.map(|v| f64::from(v as f32));
            let base = idx[pe_axis] as f32;
            c[pe_axis] = match &shift {
                Shift::F32(v) => f64::from(base + v[m]),
                Shift::F64(v) => f64::from((f64::from(base) + v[m]) as f32),
            };
            c
        })
        .collect();
    let mut out = vec![0f32; n];
    let notices = ndimage::map_coordinates(
        volume,
        shape,
        &coordinates,
        &mut out,
        options.order,
        options.mode,
        options.cval,
        options.prefilter,
        options.defects,
    )
    .map_err(|e| SdcError::Runtime(e.to_string()))?;
    drop(coordinates);
    if options.jacobian {
        let len = shape[pe_axis];
        let stride = [nj * nk, nk, 1][pe_axis];
        let along = |m: usize| (m / stride) % len;
        match &shift {
            Shift::F32(v) => out.par_iter_mut().enumerate().for_each(|(m, o)| {
                let p = along(m);
                let g = if p == 0 {
                    (v[m + stride] - v[m]) / 1.0
                } else if p == len - 1 {
                    (v[m] - v[m - stride]) / 1.0
                } else {
                    (v[m + stride] - v[m - stride]) / 2.0
                };
                *o *= 1.0 + g;
            }),
            Shift::F64(v) => out.par_iter_mut().enumerate().for_each(|(m, o)| {
                let p = along(m);
                let g = if p == 0 {
                    (v[m + stride] - v[m]) / 1.0
                } else if p == len - 1 {
                    (v[m] - v[m - stride]) / 1.0
                } else {
                    (v[m + stride] - v[m - stride]) / 2.0
                };
                *o = (f64::from(*o) * (1.0 + g)) as f32;
            }),
        }
    }
    Ok((out, notices))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(shape: [usize; 3]) -> Vec<f64> {
        // Fortran order: value = i + 10 j + 100 k
        let mut v = Vec::new();
        for k in 0..shape[2] {
            for j in 0..shape[1] {
                for i in 0..shape[0] {
                    v.push((i + 10 * j + 100 * k) as f64);
                }
            }
        }
        v
    }

    #[test]
    fn zero_field_is_the_identity_at_order_1() {
        let shape = [4, 3, 2];
        let vol = ramp(shape);
        let fmap = vec![0f32; 24];
        let opts = UnwarpOptions {
            order: 1,
            jacobian: false,
            ..Default::default()
        };
        let (out, _) =
            unwarp_volume(&vol, shape, &fmap, 1, ReadoutTime::Weak(0.05), &opts).unwrap();
        // C order output
        for i in 0..4 {
            for j in 0..3 {
                for k in 0..2 {
                    assert_eq!(out[i * 6 + j * 2 + k], (i + 10 * j + 100 * k) as f32);
                }
            }
        }
    }

    #[test]
    fn a_uniform_shift_of_one_voxel_moves_the_samples() {
        let shape = [5, 3, 2];
        let vol = ramp(shape);
        // 20 Hz * 0.05 s = 1 voxel along i
        let fmap = vec![20f32; 30];
        let opts = UnwarpOptions {
            order: 1,
            jacobian: true,
            ..Default::default()
        };
        let (out, _) =
            unwarp_volume(&vol, shape, &fmap, 0, ReadoutTime::Weak(0.05), &opts).unwrap();
        // i = 0 samples i = 1 (value 1); the Jacobian of a uniform shift is 1
        assert_eq!(out[0], 1.0);
        assert_eq!(out[2 * 6], 3.0);
    }

    #[test]
    fn the_jacobian_needs_two_samples_along_the_axis() {
        let shape = [3, 1, 2];
        let vol = vec![0.0f64; 6];
        let fmap = vec![0f32; 6];
        let e = unwarp_volume(
            &vol,
            shape,
            &fmap,
            1,
            ReadoutTime::Weak(0.05),
            &Default::default(),
        )
        .unwrap_err();
        assert_eq!(e, SdcError::Invalid(GRADIENT_TOO_SMALL.to_owned()));
    }
}
