// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-newimage 2601.0: newimage.cc's arithmetic operators,
// calculateSums). Copyright (C) 1999-2025 University of Oxford. Rust translation Copyright 2026
// Karel Lopez Vilaret; modified from the originals. Distributed under the FSL Licence (see
// LICENSE).
//! newimage's arithmetic on `volume<float>` as topup uses it: each operator works in `float`
//! (a `double` scalar is first converted to `float`, as the operators take `T`), and a binary
//! operator's result has its left operand's properties (both operands have the same
//! dimensionality here).

use larmorx_nc_core::volume::{Elem, FslError, Volume};

/// `imthrow("Attempted to <what> images of different sizes", 3)` when `b` has more values.
fn same(a: &Volume, b: &Volume, what: &str) -> Result<(), FslError> {
    if a.data.len() < b.data.len() || a.data.len() % b.data.len().max(1) != 0 {
        return Err(FslError::imthrow(
            format!("Attempted to {what} images of different sizes"),
            3,
        ));
    }
    Ok(())
}

/// `a * b` (element by element).
pub fn mul(a: &Volume, b: &Volume) -> Result<Volume, FslError> {
    let mut r = a.clone();
    mul_assign(&mut r, b)?;
    Ok(r)
}

/// `a - b`.
pub fn sub(a: &Volume, b: &Volume) -> Result<Volume, FslError> {
    let mut r = a.clone();
    same(&r, b, "subtract")?;
    let n = b.data.len();
    for (k, x) in r.data.iter_mut().enumerate() {
        *x -= b.data[k % n];
    }
    Ok(r)
}

/// `a + b`.
pub fn add(a: &Volume, b: &Volume) -> Result<Volume, FslError> {
    let mut r = a.clone();
    add_assign(&mut r, b)?;
    Ok(r)
}

/// `a += b`.
pub fn add_assign(a: &mut Volume, b: &Volume) -> Result<(), FslError> {
    same(a, b, "add")?;
    let n = b.data.len();
    for (k, x) in a.data.iter_mut().enumerate() {
        *x += b.data[k % n];
    }
    Ok(())
}

/// `a *= b`.
pub fn mul_assign(a: &mut Volume, b: &Volume) -> Result<(), FslError> {
    same(a, b, "multiply")?;
    let n = b.data.len();
    for (k, x) in a.data.iter_mut().enumerate() {
        *x *= b.data[k % n];
    }
    Ok(())
}

/// `vol * num` (and `num * vol`): `num` as `float`.
pub fn scaled(a: &Volume, num: f32) -> Volume {
    let mut r = a.clone();
    scale(&mut r, num);
    r
}

/// `vol *= num`.
pub fn scale(a: &mut Volume, num: f32) {
    for x in a.data.iter_mut() {
        *x *= num;
    }
}

/// `vol /= num`.
pub fn div_scalar(a: &mut Volume, num: f32) {
    for x in a.data.iter_mut() {
        *x /= num;
    }
}

/// `vol / num`.
pub fn divided(a: &Volume, num: f32) -> Volume {
    let mut r = a.clone();
    div_scalar(&mut r, num);
    r
}

/// `vol = num` (every value).
pub fn fill(a: &mut Volume, num: f32) {
    a.data.iter_mut().for_each(|x| *x = num);
}

/// `calculateSums(vol, mask)[0]` without a mask (`sum()`): the values as `double`, summed in
/// blocks of `max(sqrt(n), 10000) + 1`, each block's sum added to the total.
pub fn sum<T: Elem>(v: &Volume<T>) -> f64 {
    let n_all = v.data.len();
    let nlim = ((n_all as f64).sqrt() as i64).max(10000);
    let (mut total, mut s, mut n) = (0.0f64, 0.0f64, 0i64);
    for &x in &v.data {
        s += x.to_f64();
        n += 1;
        if n > nlim {
            total += s;
            s = 0.0;
            n = 0;
        }
    }
    total + s
}

/// `mean()`: `sum() / max(1, totalElements())`.
pub fn mean(v: &Volume) -> f64 {
    sum(v) / (v.data.len() as f64).max(1.0)
}

/// A 3D volume of `x × y × z` with default properties and the voxel sizes `dims`
/// (`volume(x, y, z)` then `setdims`), its values 0.
pub fn grid(x: i64, y: i64, z: i64, dims: [f32; 3]) -> Volume {
    let mut v = Volume::new(x, y, z, 1);
    v.setdims(dims[0], dims[1], dims[2]);
    v
}
