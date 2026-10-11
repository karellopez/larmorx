// SPDX-License-Identifier: Apache-2.0
//! Displacement fields (ITK's `DisplacementFieldTransform`).
//!
//! A point `x` (LPS) maps to `x + d(x)`, where `d` is the field linearly interpolated at `x`.
//! Outside the field (more than half a voxel beyond its grid) the point is returned unchanged.
//! Ported from ITK v5.4.5: `itkDisplacementFieldTransform.hxx` (`TransformPoint`) and
//! `itkVectorLinearInterpolateImageFunction.hxx`.

use larmorx_core::linalg::Mat3;
use larmorx_core::{Grid3, IndexFloor, ItkReal};

use crate::TransformError;

/// The displacement vectors, in single or double precision as stored.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldData {
    F32(Vec<f32>),
    F64(Vec<f64>),
}

impl FieldData {
    fn len(&self) -> usize {
        match self {
            FieldData::F32(v) => v.len(),
            FieldData::F64(v) => v.len(),
        }
    }

    /// Component `i` as the field image of a `DisplacementFieldTransform<R>` holds it:
    /// `static_cast<R>` of the stored value (exact for float values, and for any value in
    /// double).
    #[inline]
    fn get_as<R: ItkReal>(&self, i: usize) -> R {
        match self {
            FieldData::F32(v) => R::from_f64(f64::from(v[i])),
            FieldData::F64(v) => R::from_f64(v[i]),
        }
    }
}

/// A dense displacement field on a grid: 3 components (LPS mm) per voxel, voxel `x` fastest
/// (ITK's layout: `[d_x, d_y, d_z]` of voxel (0,0,0), then of voxel (1,0,0), ...).
#[derive(Clone, Debug, PartialEq)]
pub struct DisplacementField {
    pub grid: Grid3,
    pub data: FieldData,
    /// The ITK class name it was read as.
    pub itk_name: String,
}

impl DisplacementField {
    pub fn new(
        grid: Grid3,
        data: FieldData,
        itk_name: impl Into<String>,
    ) -> Result<Self, TransformError> {
        if data.len() != 3 * grid.len() {
            return Err(TransformError::Invalid(format!(
                "a displacement field on a {:?} grid needs {} values, got {}",
                grid.size,
                3 * grid.len(),
                data.len()
            )));
        }
        Ok(DisplacementField {
            grid,
            data,
            itk_name: itk_name.into(),
        })
    }

    /// A field from ITK's representation: the parameters are the vectors, and the 18 fixed
    /// parameters are size (3), origin (3), spacing (3) and direction (9, row-major).
    pub fn from_itk(
        itk_name: &str,
        data: FieldData,
        fixed: &[f64],
    ) -> Result<Self, TransformError> {
        if fixed.len() != 18 {
            return Err(TransformError::Invalid(format!(
                "{itk_name} needs 18 fixed parameters, got {}",
                fixed.len()
            )));
        }
        let size = [fixed[0], fixed[1], fixed[2]].map(|v| v as usize);
        let direction: Mat3 =
            std::array::from_fn(|i| std::array::from_fn(|j| fixed[9 + 3 * i + j]));
        let grid = Grid3::new(
            size,
            [fixed[6], fixed[7], fixed[8]],
            [fixed[3], fixed[4], fixed[5]],
            direction,
        )
        .ok_or_else(|| TransformError::Invalid("singular displacement-field grid".into()))?;
        Self::new(grid, data, itk_name)
    }

    /// ITK's fixed parameters of this field.
    pub fn fixed_parameters(&self) -> Vec<f64> {
        let g = &self.grid;
        let mut out: Vec<f64> = g.size.iter().map(|&n| n as f64).collect();
        out.extend_from_slice(&g.origin);
        out.extend_from_slice(&g.spacing);
        out.extend(g.direction.iter().flatten());
        out
    }

    /// The displacement at a continuous index, as ITK's
    /// `VectorLinearInterpolateImageFunction<…, R>` computes it (`R`, the coordinate type, is
    /// its `InternalComputationType`): base index `Math::Floor`, distances and overlaps in `R`
    /// (`overlap *= 1.0 − d` is a double product stored as `R`), each `overlap · value`
    /// product in `R` (the field's components are `R`), summed in double; the 8 neighbours in
    /// a fixed order, indices clamped to the grid, stopping once the overlaps sum to 1.
    #[inline]
    fn displacement<R: ItkReal>(&self, cidx: [R; 3], floor: &IndexFloor) -> [f64; 3] {
        let size = self.grid.size;
        let base: [i64; 3] = cidx.map(|c| floor.pick(c.itk_floor(), c.to_f64()));
        let distance: [R; 3] = std::array::from_fn(|d| cidx[d] - R::from_f64(base[d] as f64));
        let end = size.map(|n| n as i64 - 1);
        let mut out = [0.0f64; 3];
        let mut total = 0.0f64;
        for counter in 0..8u32 {
            let mut overlap = R::ONE;
            let mut neighbour = [0usize; 3];
            for d in 0..3 {
                let idx = if counter >> d & 1 == 1 {
                    overlap = overlap * distance[d];
                    (base[d] + 1).min(end[d])
                } else {
                    overlap = R::from_f64(overlap.to_f64() * (1.0 - distance[d].to_f64()));
                    base[d].max(0)
                };
                neighbour[d] = idx as usize;
            }
            if overlap != R::ZERO {
                let v = 3 * (neighbour[0] + size[0] * (neighbour[1] + size[1] * neighbour[2]));
                for (k, o) in out.iter_mut().enumerate() {
                    *o += (overlap * self.data.get_as::<R>(v + k)).to_f64();
                }
                total += overlap.to_f64();
            }
            if total == 1.0 {
                break;
            }
        }
        out
    }

    /// Maps a point (LPS): `x + d(x)` inside the field, `x` outside.
    pub fn transform_point(&self, p: [f64; 3]) -> [f64; 3] {
        self.transform_point_as(p)
    }

    /// [`DisplacementField::transform_point`] in the computation type `R`
    /// (`DisplacementFieldTransform<R>::TransformPoint`): the continuous index of the `R`
    /// point is computed in double from the field's geometry and stored as `R`; outside the
    /// buffer (ITK's `IsInsideBuffer` on that index) the point is returned unchanged; inside,
    /// each coordinate is `x + d` computed in double and stored as `R`.
    #[inline]
    pub fn transform_point_as<R: ItkReal>(&self, p: [R; 3]) -> [R; 3] {
        self.transform_point_with(p, &IndexFloor::itk())
    }

    /// [`DisplacementField::transform_point_as`] with the floor of the base index chosen by
    /// `floor` (ITK's is 0 just below 0, defect ITK-01).
    #[inline]
    pub fn transform_point_with<R: ItkReal>(&self, p: [R; 3], floor: &IndexFloor) -> [R; 3] {
        let cidx: [R; 3] = self
            .grid
            .point_to_index(p.map(|x| x.to_f64()))
            .map(R::from_f64);
        if !self.grid.is_inside(cidx.map(|c| c.to_f64())) {
            return p;
        }
        let d = self.displacement(cidx, floor);
        std::array::from_fn(|i| R::from_f64(p[i].to_f64() + d[i]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::linalg::IDENTITY3;

    fn field(f: impl Fn(usize, usize, usize) -> [f64; 3]) -> DisplacementField {
        let grid = Grid3::new([4, 3, 2], [2.0, 2.0, 2.0], [10.0, 0.0, 0.0], IDENTITY3).unwrap();
        let mut data = Vec::new();
        for k in 0..2 {
            for j in 0..3 {
                for i in 0..4 {
                    data.extend(f(i, j, k));
                }
            }
        }
        DisplacementField::new(
            grid,
            FieldData::F64(data),
            "DisplacementFieldTransform_double_3_3",
        )
        .unwrap()
    }

    #[test]
    fn linear_inside_identity_outside() {
        let f = field(|i, _, _| [i as f64, 0.0, -1.0]);
        // Voxel centres: exact values.
        assert_eq!(f.transform_point([14.0, 2.0, 2.0]), [16.0, 2.0, 1.0]);
        // Half way between i = 1 and i = 2: d_x = 1.5.
        assert_eq!(f.transform_point([13.0, 0.0, 0.0]), [14.5, 0.0, -1.0]);
        // Beyond the last voxel but within half a voxel: clamped to the edge value.
        assert_eq!(f.transform_point([16.9, 0.0, 0.0]), [19.9, 0.0, -1.0]);
        // Outside: unchanged.
        assert_eq!(f.transform_point([17.1, 0.0, 0.0]), [17.1, 0.0, 0.0]);
        assert_eq!(f.transform_point([8.9, 0.0, 0.0]), [8.9, 0.0, 0.0]);
    }

    #[test]
    fn itk_fixed_parameters_round_trip() {
        let f = field(|_, _, _| [0.0; 3]);
        let g = DisplacementField::from_itk(
            "DisplacementFieldTransform_float_3_3",
            f.data.clone(),
            &f.fixed_parameters(),
        )
        .unwrap();
        assert_eq!(g.grid, f.grid);
        assert!(
            DisplacementField::from_itk("x", FieldData::F32(vec![0.0; 5]), &f.fixed_parameters())
                .is_err()
        );
    }
}
