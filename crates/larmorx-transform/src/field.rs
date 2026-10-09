//! Displacement fields (ITK's `DisplacementFieldTransform`).
//!
//! A point `x` (LPS) maps to `x + d(x)`, where `d` is the field linearly interpolated at `x`.
//! Outside the field (more than half a voxel beyond its grid) the point is returned unchanged.
//! Ported from ITK v5.4.5: `itkDisplacementFieldTransform.hxx` (`TransformPoint`) and
//! `itkVectorLinearInterpolateImageFunction.hxx`.

use larmorx_core::Grid3;
use larmorx_core::linalg::Mat3;

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

    #[inline]
    fn get(&self, i: usize) -> f64 {
        match self {
            FieldData::F32(v) => f64::from(v[i]),
            FieldData::F64(v) => v[i],
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

    /// The displacement at a continuous index, as ITK's `VectorLinearInterpolateImageFunction`:
    /// a weighted sum over the 8 neighbours (indices clamped to the grid), in a fixed order.
    fn displacement(&self, cidx: [f64; 3]) -> [f64; 3] {
        let size = self.grid.size;
        let base = cidx.map(|c| c.floor());
        let distance = [cidx[0] - base[0], cidx[1] - base[1], cidx[2] - base[2]];
        let base = base.map(|b| b as i64);
        let end = size.map(|n| n as i64 - 1);
        let mut out = [0.0f64; 3];
        let mut total = 0.0f64;
        for counter in 0..8u32 {
            let mut overlap = 1.0f64;
            let mut neighbour = [0usize; 3];
            for d in 0..3 {
                let idx = if counter >> d & 1 == 1 {
                    overlap *= distance[d];
                    (base[d] + 1).min(end[d])
                } else {
                    overlap *= 1.0 - distance[d];
                    base[d].max(0)
                };
                neighbour[d] = idx as usize;
            }
            if overlap != 0.0 {
                let v = 3 * (neighbour[0] + size[0] * (neighbour[1] + size[1] * neighbour[2]));
                for (k, o) in out.iter_mut().enumerate() {
                    *o += overlap * self.data.get(v + k);
                }
                total += overlap;
            }
            if total == 1.0 {
                break;
            }
        }
        out
    }

    /// Maps a point (LPS): `x + d(x)` inside the field, `x` outside.
    pub fn transform_point(&self, p: [f64; 3]) -> [f64; 3] {
        let cidx = self.grid.point_to_index(p);
        if !self.grid.is_inside(cidx) {
            return p;
        }
        let d = self.displacement(cidx);
        [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
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
