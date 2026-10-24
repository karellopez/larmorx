// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.17: fsl-miscmaths 2203.2 (kernel.{h,cc}). Copyright (C)
// 2001-2018 University of Oxford (Mark Jenkinson). Rust translation Copyright 2026 Karel Lopez
// Vilaret; modified from the original. Distributed under the FSL Licence (see LICENSE).
//! Interpolation kernels: the windowed sinc of `setinterpolationmethod(sinc)`
//! (`sinckernel1D`) and its lookup (`kernelval`).

use larmorx_core::math;

use crate::glibc_float;

/// The sine in single precision: C++'s `sin(float)`, i.e. the C library's `sinf`, as FSL's Linux
/// build computes it (glibc 2.35, FMA build; [`glibc_float::sinf`]). The double `cos` of the
/// windows is correctly rounded: glibc's agrees with it on every argument the kernels use
/// (`tools/glibc-float-verify kernel`).
#[inline]
pub fn sinf(x: f32) -> f32 {
    glibc_float::sinf(x)
}

/// A kernel of half-widths `(wx, wy, wz)` sampled at `kx.len()` points over `[-w, w]`
/// (`kernelstorage`).
#[derive(Clone, Debug, PartialEq)]
pub struct Kernel {
    pub widthx: i32,
    pub widthy: i32,
    pub widthz: i32,
    pub kx: Vec<f64>,
    pub ky: Vec<f64>,
    pub kz: Vec<f64>,
}

/// `sincfn(x)`.
pub fn sincfn(x: f32) -> f32 {
    if f64::from(x.abs()) < 1e-7 {
        return (1.0 - f64::from(x.abs())) as f32;
    }
    let y = (std::f64::consts::PI * f64::from(x)) as f32;
    sinf(y) / y
}

/// `hanning(x, w)` (`w` the half-width).
pub fn hanning(x: f32, w: i32) -> f32 {
    if x.abs() > w as f32 {
        0.0
    } else {
        (0.5 + 0.5 * math::cos(std::f64::consts::PI * f64::from(x) / f64::from(w))) as f32
    }
}

/// `blackman(x, w)` (`w` the half-width).
pub fn blackman(x: f32, w: i32) -> f32 {
    if x.abs() > w as f32 {
        0.0
    } else {
        let a = std::f64::consts::PI * f64::from(x) / f64::from(w);
        let b = 2.0 * std::f64::consts::PI * f64::from(x) / f64::from(w);
        (0.42 + 0.5 * math::cos(a) + 0.08 * math::cos(b)) as f32
    }
}

/// `rectangular(x, w)`.
pub fn rectangular(x: f32, w: i32) -> f32 {
    if x.abs() > w as f32 { 0.0 } else { 1.0 }
}

/// `sinckernel1D(type, w, n)` (`w` the full width): `n` samples of the windowed sinc. An
/// unknown window prints FSL's message and uses Blackman's.
pub fn sinckernel1d(window: &str, w: i32, n: i32, messages: &mut Vec<String>) -> Vec<f64> {
    let nstore = n.max(1);
    let hw = (w - 1) / 2;
    let halfnk = ((f64::from(nstore) - 1.0) / 2.0) as f32;
    let mut ker = vec![0.0f64; nstore as usize];
    for k in 1..=nstore {
        // (n-halfnk-1)/halfnk*hw: int - float - int, / float, * int, all float.
        let x = ((k as f32 - halfnk - 1.0) / halfnk) * hw as f32;
        let v = match window {
            "hanning" | "h" => sincfn(x) * hanning(x, hw),
            "blackman" | "b" => sincfn(x) * blackman(x, hw),
            "rectangular" | "r" => sincfn(x) * rectangular(x, hw),
            _ => {
                messages.push("ERROR: Unrecognised sinc window type - using Blackman".to_owned());
                return sinckernel1d("b", w, nstore, messages);
            }
        };
        ker[(k - 1) as usize] = f64::from(v);
    }
    ker
}

/// `definesincinterpolation(type, w, nstore)`: the same kernel along the three axes, with
/// half-widths `(w-1)/2`.
pub fn sinc_kernel(window: &str, w: i32, nstore: i32, messages: &mut Vec<String>) -> Kernel {
    let k = sinckernel1d(window, w, nstore, messages);
    let hw = (w - 1) / 2;
    Kernel {
        widthx: hw,
        widthy: hw,
        widthz: hw,
        kx: k.clone(),
        ky: k.clone(),
        kz: k,
    }
}

/// `kernelval(x, w, kernel)`: linear interpolation of the sampled kernel at `x` (half-width
/// `w`).
pub fn kernelval(x: f32, w: i32, kernel: &[f64]) -> f32 {
    if x.abs() > w as f32 {
        return 0.0;
    }
    let nrows = kernel.len() as i32;
    let halfnk = ((f64::from(nrows) - 1.0) / 2.0) as f32;
    let mut dn = (f64::from(x / w as f32 * halfnk + halfnk) + 1.0) as f32;
    let n = crate::cnum::f32_to_i32(dn.floor());
    dn -= n as f32;
    if n > nrows - 1 {
        return 0.0;
    }
    if n < 1 {
        return 0.0;
    }
    let (a, b) = (kernel[(n - 1) as usize], kernel[n as usize]);
    (a * (1.0 - f64::from(dn)) + b * f64::from(dn)) as f32
}
