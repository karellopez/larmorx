// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Coordinate mapping as nitransforms 25.1.0 does it (MIT; notice below and in NOTICE), the
//! transforms under fMRIPrep's one-shot resampler.
//!
//! Points are RAS+ millimetres, transforms map **reference (target) points to moving points**
//! ("image mode"), and a chain applies its steps in list order (`TransformChain.map`). Ported
//! from `nitransforms/base.py` (`ImageGrid.ndcoords`, `_as_homogeneous`, `_apply_affine`),
//! `linear.py` (`Affine.map`), `nonlinear.py` (`DenseFieldTransform.map`) and `manip.py`
//! (`TransformChain.map`). What decides bit-identity:
//!
//! - **float32 inputs.** `_as_homogeneous` converts every point to float32 before each affine
//!   (and before a displacement field's index lookup), so every step starts from float32
//!   coordinates; outputs are float64.
//! - **The product.** numpy evaluates `affine.dot(points)` with BLAS `dgemm`, which on x86-64
//!   (OpenBLAS's Haswell kernel) and aarch64 accumulates with fused multiply-adds:
//!   `fma(a3, 1, fma(a2, z, fma(a1, y, a0·x)))`. [`dot_row`] does exactly that.
//! - **Displacement fields.** A point is looked up when *every* point lies within 1e-3 voxel
//!   of a grid node; otherwise each component is interpolated with SciPy's cubic
//!   `map_coordinates` (mode `constant`, `cval` NaN) and NaN components (outside the field)
//!   fall back to the input point (no displacement).
//!
//! The 4×4 matrices themselves (inverses, products, ITK-to-RAS conversions) are computed by
//! the caller, e.g. with numpy as nitransforms does (`larmorx.transforms`).
//!
//! nitransforms: Copyright (c) 2021 The NiPy developers (MIT License).
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy of this
//! software and associated documentation files (the "Software"), to deal in the Software
//! without restriction, including without limitation the rights to use, copy, modify, merge,
//! publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons
//! to whom the Software is furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all copies or
//! substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
//! INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR
//! PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE
//! FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
//! OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//! DEALINGS IN THE SOFTWARE.

use larmorx_core::linalg::Mat4;
use larmorx_interp::ndimage::{Mode, Spline};
use rayon::prelude::*;

use crate::TransformError;

/// Points per parallel task.
const CHUNK: usize = 8192;

/// Row `r` of `m` times the homogeneous point `(p, 1)`, accumulated as numpy's BLAS does:
/// `fma(m[r][2], p2, fma(m[r][1], p1, m[r][0]·p0)) + m[r][3]` (the last term's product with 1
/// is exact, so the final fused step is a plain addition).
#[inline(always)]
pub fn dot_row(m: &Mat4, r: usize, p: [f64; 3]) -> f64 {
    let s = m[r][0] * p[0];
    let s = m[r][1].mul_add(p[1], s);
    let s = m[r][2].mul_add(p[2], s);
    s + m[r][3]
}

/// `_apply_affine(p, m, 3)` for one point that is already float32-valued.
#[inline(always)]
fn apply(m: &Mat4, p: [f64; 3]) -> [f64; 3] {
    [dot_row(m, 0, p), dot_row(m, 1, p), dot_row(m, 2, p)]
}

/// A point rounded to float32, as `_as_homogeneous` does.
#[inline(always)]
pub fn to_f32(p: [f64; 3]) -> [f64; 3] {
    p.map(|v| f64::from(v as f32))
}

/// `ImageGrid(img).ndcoords`: the RAS+ coordinates of every voxel of a grid with `shape` and
/// `affine`, in **Fortran order** (first index fastest; nitransforms lists them in C order,
/// which changes nothing per point). Indices go through float32, exactly for grids below 2^24
/// voxels per axis.
pub fn ndcoords(shape: [usize; 3], affine: &Mat4) -> Vec<[f64; 3]> {
    let [nx, ny, nz] = shape;
    let mut out = vec![[0.0f64; 3]; nx * ny * nz];
    if out.is_empty() {
        return out;
    }
    out.par_chunks_mut(nx * ny)
        .enumerate()
        .for_each(|(k, plane)| {
            for j in 0..ny {
                for i in 0..nx {
                    let p = to_f32([i as f64, j as f64, k as f64]);
                    plane[i + nx * j] = apply(affine, p);
                }
            }
        });
    out
}

/// `Affine(matrix).map(points)`: every point rounded to float32, then multiplied.
pub fn affine_map(matrix: &Mat4, points: &mut [[f64; 3]]) {
    points.par_chunks_mut(CHUNK).for_each(|chunk| {
        for p in chunk {
            *p = apply(matrix, to_f32(*p));
        }
    });
}

/// `DenseFieldTransform(field)`: a deformation sampled on a grid. `field[c]` holds component
/// `c` of the deformed position (RAS+ mm; the grid's own coordinates plus the displacement)
/// at every node, in Fortran order.
#[derive(Clone, Debug)]
pub struct DenseField {
    shape: [usize; 3],
    /// RAS+ millimetres to grid indices (`ImageGrid.inverse`, numpy's inverse of the affine).
    inverse: Mat4,
    field: [Vec<f64>; 3],
}

impl DenseField {
    /// A field from displacements (`is_deltas=True`, as fMRIPrep's `.h5` warps load):
    /// `deltas[c]` is component `c` of the RAS+ displacement at each node (Fortran order).
    /// The deformation is `ndcoords(shape, affine) + deltas`, in float64.
    pub fn from_deltas(
        shape: [usize; 3],
        affine: &Mat4,
        inverse: &Mat4,
        deltas: [Vec<f64>; 3],
    ) -> Result<Self, TransformError> {
        let n: usize = shape.iter().product();
        if deltas.iter().any(|d| d.len() != n) {
            return Err(TransformError::Invalid(format!(
                "the displacement components must have {n} values ({shape:?})"
            )));
        }
        let coords = ndcoords(shape, affine);
        let mut field = deltas;
        for (c, comp) in field.iter_mut().enumerate() {
            comp.par_iter_mut()
                .zip(coords.par_iter())
                .for_each(|(v, p)| *v += p[c]);
        }
        Ok(DenseField {
            shape,
            inverse: *inverse,
            field,
        })
    }

    /// A field of deformed positions (`is_deltas=False`).
    pub fn from_positions(
        shape: [usize; 3],
        inverse: &Mat4,
        positions: [Vec<f64>; 3],
    ) -> Result<Self, TransformError> {
        let n: usize = shape.iter().product();
        if positions.iter().any(|d| d.len() != n) {
            return Err(TransformError::Invalid(format!(
                "the field components must have {n} values ({shape:?})"
            )));
        }
        Ok(DenseField {
            shape,
            inverse: *inverse,
            field: positions,
        })
    }

    pub fn shape(&self) -> [usize; 3] {
        self.shape
    }

    /// `DenseFieldTransform.map(points)`, in place.
    pub fn map(&self, points: &mut [[f64; 3]]) -> Result<(), TransformError> {
        // ijk = self.reference.index(np.array(x, dtype="float32"))
        let ijk: Vec<[f64; 3]> = points
            .par_iter()
            .map(|p| apply(&self.inverse, to_f32(*p)))
            .collect();
        let on_grid = ijk.par_iter().all(|q| {
            let d = q.map(|v| v - v.round_ties_even());
            (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() < 1e-3
        });
        if on_grid {
            // return self._field[tuple(indexes.T) + (np.s_[:],)]: numpy indexing, where a
            // negative index counts from the end and anything else outside is an IndexError.
            let [nx, ny, nz] = self.shape;
            let wrap = |v: f64, n: usize| -> Option<usize> {
                let i = v.round_ties_even() as i64;
                let i = if i < 0 { i + n as i64 } else { i };
                (0..n as i64).contains(&i).then_some(i as usize)
            };
            let mut bad = None;
            for (p, q) in points.iter_mut().zip(&ijk) {
                match (wrap(q[0], nx), wrap(q[1], ny), wrap(q[2], nz)) {
                    (Some(i), Some(j), Some(k)) => {
                        let n = i + nx * (j + ny * k);
                        *p = [self.field[0][n], self.field[1][n], self.field[2][n]];
                    }
                    _ => {
                        bad = Some(*q);
                        break;
                    }
                }
            }
            return match bad {
                None => Ok(()),
                Some(q) => Err(TransformError::Invalid(format!(
                    "index {q:?} is out of bounds for the field of shape {:?}",
                    self.shape
                ))),
            };
        }
        // Three cubic interpolations, one component at a time, NaN outside the field.
        let mut mapped = vec![[0.0f64; 3]; points.len()];
        for c in 0..3 {
            let spline = Spline::new(
                &self.field[c],
                self.shape,
                3,
                Mode::Constant,
                f64::NAN,
                true,
                true,
            )
            .map_err(|e| TransformError::Invalid(e.to_string()))?;
            mapped
                .par_chunks_mut(CHUNK)
                .zip(ijk.par_chunks(CHUNK))
                .for_each(|(out, q)| {
                    for (o, q) in out.iter_mut().zip(q) {
                        o[c] = spline.sample(*q);
                    }
                });
        }
        // mapped_coords[np.isnan(mapped_coords)] = np.array(x)[np.isnan(mapped_coords)]
        points
            .par_iter_mut()
            .zip(mapped.par_iter())
            .for_each(|(p, m)| {
                for c in 0..3 {
                    if !m[c].is_nan() {
                        p[c] = m[c];
                    }
                }
            });
        Ok(())
    }
}

/// One step of a nitransforms chain.
#[derive(Clone, Debug)]
pub enum Step {
    /// `Affine(matrix)`: reference RAS+ to moving RAS+ (or to voxel indices).
    Affine(Mat4),
    /// `DenseFieldTransform`.
    DenseField(DenseField),
}

/// `TransformChain(steps).map(points)`: each step in list order, in place.
pub fn map_points(steps: &[Step], points: &mut [[f64; 3]]) -> Result<(), TransformError> {
    for step in steps {
        match step {
            Step::Affine(m) => affine_map(m, points),
            Step::DenseField(f) => f.map(points)?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const AFF: Mat4 = [
        [-2.5, 0.1, 0.0, 90.3],
        [0.05, 2.5, -0.2, -126.7],
        [0.0, 0.3, 3.1, -72.25],
        [0.0, 0.0, 0.0, 1.0],
    ];

    #[test]
    fn ndcoords_uses_fused_products() {
        let c = ndcoords([3, 4, 5], &AFF);
        assert_eq!(c.len(), 60);
        let (i, j, k) = (2usize, 3usize, 4usize);
        let p = c[i + 3 * (j + 4 * k)];
        let expected = [
            AFF[0][2].mul_add(4.0, AFF[0][1].mul_add(3.0, AFF[0][0] * 2.0)) + AFF[0][3],
            AFF[1][2].mul_add(4.0, AFF[1][1].mul_add(3.0, AFF[1][0] * 2.0)) + AFF[1][3],
            AFF[2][2].mul_add(4.0, AFF[2][1].mul_add(3.0, AFF[2][0] * 2.0)) + AFF[2][3],
        ];
        assert_eq!(p, expected);
    }

    #[test]
    fn affine_map_rounds_inputs_to_float32() {
        let mut pts = [[0.1f64, 0.2, 0.3]];
        affine_map(&AFF, &mut pts);
        let x = to_f32([0.1, 0.2, 0.3]);
        assert_eq!(pts[0], apply(&AFF, x));
        assert_ne!(x, [0.1, 0.2, 0.3]);
    }

    fn identity() -> Mat4 {
        let mut m = [[0.0; 4]; 4];
        for (i, row) in m.iter_mut().enumerate() {
            row[i] = 1.0;
        }
        m
    }

    #[test]
    fn dense_field_on_grid_is_a_lookup_and_outside_is_identity() {
        let shape = [4, 4, 4];
        let n = 64;
        let deltas = [vec![0.5; n], vec![-0.25; n], vec![0.0; n]];
        let f = DenseField::from_deltas(shape, &identity(), &identity(), deltas).unwrap();
        // All points on the grid: exact lookup.
        let mut pts = vec![[1.0, 2.0, 3.0], [0.0, 0.0, 0.0]];
        f.map(&mut pts).unwrap();
        assert_eq!(pts, vec![[1.5, 1.75, 3.0], [0.5, -0.25, 0.0]]);
        // One point off the grid: interpolation; a point outside keeps its coordinates.
        let mut pts = vec![[1.5, 2.0, 1.0], [10.0, 1.0, 1.0]];
        f.map(&mut pts).unwrap();
        assert!((pts[0][0] - 2.0).abs() < 1e-12 && (pts[0][1] - 1.75).abs() < 1e-12);
        assert_eq!(pts[1], [10.0, 1.0, 1.0]);
        // On the grid but outside it: numpy's IndexError.
        let mut pts = vec![[7.0, 0.0, 0.0]];
        assert!(f.map(&mut pts).is_err());
        // Negative indices wrap, as numpy's do.
        let mut pts = vec![[-1.0, 0.0, 0.0]];
        f.map(&mut pts).unwrap();
        assert_eq!(pts[0], [3.5, -0.25, 0.0]);
    }
}
