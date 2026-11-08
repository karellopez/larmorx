// SPDX-License-Identifier: Apache-2.0
//! AFNI's grid of a NIfTI dataset and 3dvolreg's conventions on it (`specs/3dvolreg.md` §4.1,
//! §4.4): each voxel axis's orientation, signed step and origin in AFNI's DICOM coordinates, the
//! handedness, the axis codes, the conversion matrix `P` and the two centres.

use larmorx_core::defects::UpstreamDefects;

use crate::automask::program::orientation;
use crate::geometry::{InputGeometry, Source};

/// The direction a voxel axis's index increases toward (AFNI's orientation codes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orient {
    R2L,
    L2R,
    P2A,
    A2P,
    I2S,
    S2I,
}

impl Orient {
    /// The DICOM coordinate the axis runs along (0: x, 1: y, 2: z).
    pub fn dicom_axis(self) -> usize {
        match self {
            Orient::R2L | Orient::L2R => 0,
            Orient::P2A | Orient::A2P => 1,
            Orient::I2S | Orient::S2I => 2,
        }
    }

    /// +1 when the index increases toward the positive DICOM end (R2L, A2P, I2S), else −1: the
    /// sign of the step and of `P`'s entry.
    pub fn sign(self) -> f32 {
        match self {
            Orient::R2L | Orient::A2P | Orient::I2S => 1.0,
            _ => -1.0,
        }
    }

    /// The first and second letter (`R2L`: R, L).
    pub fn letters(self) -> (char, char) {
        match self {
            Orient::R2L => ('R', 'L'),
            Orient::L2R => ('L', 'R'),
            Orient::P2A => ('P', 'A'),
            Orient::A2P => ('A', 'P'),
            Orient::I2S => ('I', 'S'),
            Orient::S2I => ('S', 'I'),
        }
    }

    /// The name in AFNI's messages (`R-L`).
    pub fn name(self) -> String {
        let (a, b) = self.letters();
        format!("{a}-{b}")
    }

    /// From the RAS axis (0, 1, 2) and whether the index runs toward its positive end.
    fn from_ras(axis: usize, positive: bool) -> Orient {
        match (axis, positive) {
            (0, true) => Orient::L2R,
            (0, false) => Orient::R2L,
            (1, true) => Orient::P2A,
            (1, false) => Orient::A2P,
            (_, true) => Orient::I2S,
            (_, false) => Orient::S2I,
        }
    }
}

/// A 3 × 3 matrix in double precision, rows first.
pub type M3 = [[f64; 3]; 3];

/// AFNI's grid of a dataset (§4.1).
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    /// Voxels along each axis.
    pub n: [usize; 3],
    pub orient: [Orient; 3],
    /// The signed steps Δ (mm).
    pub delta: [f32; 3],
    /// The coordinate of voxel 0 along each axis, in DICOM coordinates (mm).
    pub origin: [f32; 3],
}

impl Grid {
    /// The grid of a dataset of `n` voxels read as `g`. A matrix whose orientation cannot be
    /// found (a zero column) is taken as L2R, P2A, I2S (NIfTI's default).
    pub fn from_geometry(g: &InputGeometry, n: [usize; 3], mode: UpstreamDefects) -> Grid {
        let mm = g.mm_per_unit;
        let codes = orientation(&g.m).unwrap_or([(0, true), (1, true), (2, true)]);
        let orient = codes.map(|(axis, positive)| Orient::from_ras(axis, positive));
        let size = |j: usize| -> f32 {
            let s = match g.source {
                Source::Sform => {
                    let (x, y, z) = (
                        f64::from(g.m[0][j]),
                        f64::from(g.m[1][j]),
                        f64::from(g.m[2][j]),
                    );
                    ((x * x + y * y) + z * z).sqrt() as f32
                }
                Source::Qform | Source::Pixdim => g.pixdim[j],
            };
            if mm != 1.0 {
                (f64::from(s) * mm) as f32
            } else {
                s
            }
        };
        // The offsets: converted to millimetres by the default (AFNI-07), as the output's.
        let offset = |row: usize| -> f32 {
            if mm != 1.0 && mode.fix() {
                match &g.qform_mm {
                    Some(q) => q[row][3],
                    None => (f64::from(g.m[row][3]) * mm) as f32,
                }
            } else {
                g.m[row][3]
            }
        };
        let mut delta = [0f32; 3];
        let mut origin = [0f32; 3];
        for j in 0..3 {
            let o = orient[j];
            delta[j] = o.sign() * size(j);
            let row = o.dicom_axis();
            // NIfTI's x and y are DICOM's negated.
            origin[j] = if row < 2 { -offset(row) } else { offset(row) };
        }
        Grid {
            n,
            orient,
            delta,
            origin,
        }
    }

    /// The voxel sizes `|Δ|`.
    pub fn sizes(&self) -> [f32; 3] {
        self.delta.map(f32::abs)
    }

    /// The handedness h (§4.1): +1 when the matrix of the axes' DICOM unit vectors has a
    /// positive determinant.
    pub fn handedness(&self) -> f32 {
        let p = self.p(true);
        if det(&p) > 0.0 { 1.0 } else { -1.0 }
    }

    /// `code(L)` (§4.1): `+n` (1-based) when axis n runs from `letter`, `−n` when it runs to it.
    pub fn code(&self, letter: char) -> i32 {
        for (n, o) in self.orient.iter().enumerate() {
            let (a, b) = o.letters();
            if a == letter {
                return n as i32 + 1;
            }
            if b == letter {
                return -(n as i32 + 1);
            }
        }
        0
    }

    /// The internal axes `a1, a2, a3` of the angles (roll about I–S, pitch about R–L, yaw
    /// about A–P) and their signs `σ1, σ2, σ3` (§4.4).
    pub fn angle_axes(&self) -> ([usize; 3], [f32; 3]) {
        let h = self.handedness();
        let mut axes = [0usize; 3];
        let mut signs = [0f32; 3];
        for (m, letter) in ['I', 'R', 'A'].into_iter().enumerate() {
            let c = self.code(letter);
            axes[m] = (c.unsigned_abs() as usize).saturating_sub(1);
            signs[m] = if c < 0 { -h } else { h };
        }
        (axes, signs)
    }

    /// The matrix `P` from dataset axes to DICOM order (§4.1): in column n, ±1 in the row of axis
    /// n's DICOM coordinate; with `signed` false the permutation alone (`|P|`).
    pub fn p(&self, signed: bool) -> M3 {
        let mut p = [[0.0; 3]; 3];
        for (n, o) in self.orient.iter().enumerate() {
            p[o.dicom_axis()][n] = if signed { f64::from(o.sign()) } else { 1.0 };
        }
        p
    }

    /// The centre of the dataset in DICOM order (§4.1, float32).
    pub fn centre(&self) -> [f32; 3] {
        let mut c = [0f32; 3];
        for (n, o) in self.orient.iter().enumerate() {
            let a = self.origin[n];
            let b = self.origin[n] + (self.n[n].saturating_sub(1) as f32) * self.delta[n];
            c[o.dicom_axis()] = 0.5f32 * (a + b);
        }
        c
    }

    /// The axis centre of the displacements (§4.1): `o + 0.5·(n − 1)·Δ` in double, rounded to
    /// float32, per axis in dataset order.
    pub fn axis_centre(&self) -> [f32; 3] {
        [0, 1, 2].map(|n| {
            (f64::from(self.origin[n])
                + 0.5 * (self.n[n].saturating_sub(1) as f64) * f64::from(self.delta[n]))
                as f32
        })
    }

    /// The reported shifts `(dS, dL, dP)` of internal shifts along the dataset axes (§4.4).
    pub fn reported_shifts(&self, d: [f32; 3]) -> [f32; 3] {
        let mut out = [0f32; 3];
        for (n, o) in self.orient.iter().enumerate() {
            let (slot, v) = match o {
                Orient::R2L => (1, d[n]),
                Orient::L2R => (1, -d[n]),
                Orient::P2A => (2, -d[n]),
                Orient::A2P => (2, d[n]),
                Orient::I2S => (0, d[n]),
                Orient::S2I => (0, -d[n]),
            };
            out[slot] = v;
        }
        out
    }
}

impl Grid {
    /// The internal shifts along the dataset axes of reported shifts `(dS, dL, dP)` (the
    /// inverse of [`Grid::reported_shifts`]).
    pub fn internal_shifts(&self, r: [f32; 3]) -> [f32; 3] {
        let [ds, dl, dp] = r;
        self.orient.map(|o| match o {
            Orient::R2L => dl,
            Orient::L2R => -dl,
            Orient::P2A => -dp,
            Orient::A2P => dp,
            Orient::I2S => ds,
            Orient::S2I => -ds,
        })
    }
}

/// The determinant, by the rule of Sarrus in a fixed order.
pub fn det(a: &M3) -> f64 {
    a[0][0] * a[1][1] * a[2][2] - a[0][0] * a[1][2] * a[2][1] - a[1][0] * a[0][1] * a[2][2]
        + a[1][0] * a[0][2] * a[2][1]
        + a[2][0] * a[0][1] * a[1][2]
        - a[2][0] * a[0][2] * a[1][1]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(orient: [Orient; 3]) -> Grid {
        Grid {
            n: [4, 5, 6],
            orient,
            delta: orient.map(|o| o.sign() * 2.0),
            origin: [10.0, -20.0, 30.0],
        }
    }

    #[test]
    fn codes_axes_and_signs() {
        use Orient::*;
        // RAS storage: L2R, P2A, I2S. The DICOM unit vectors -x, -y, +z: determinant +1.
        let ras = grid([L2R, P2A, I2S]);
        assert_eq!(ras.handedness(), 1.0);
        assert_eq!((ras.code('I'), ras.code('R'), ras.code('A')), (3, -1, -2));
        assert_eq!(ras.angle_axes(), ([2, 0, 1], [1.0, -1.0, -1.0]));
        // LPS storage: R2L, A2P, I2S: +x, +y, +z.
        let lps = grid([R2L, A2P, I2S]);
        assert_eq!(lps.angle_axes(), ([2, 0, 1], [1.0, 1.0, 1.0]));
        assert_eq!(ras.reported_shifts([1.0, 2.0, 3.0]), [3.0, -1.0, -2.0]);
        assert_eq!(lps.reported_shifts([1.0, 2.0, 3.0]), [3.0, 1.0, 2.0]);
        assert_eq!(ras.p(true)[0][0], -1.0);
        assert_eq!(ras.p(false)[0][0], 1.0);
    }
}
