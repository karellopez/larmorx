// SPDX-License-Identifier: Apache-2.0
//! The mask and the MRF neighbourhood (`specs/fast.md` §4.2), laid out for speed: the mask's
//! voxels are numbered in storage order and every per-voxel quantity is kept for them only
//! (compact arrays); a map over the image padded by one voxel on every side gives each voxel's
//! number, voxels outside the mask (and the padding) the number of an extra row of zeros. So a
//! neighbour's posteriors are read without a bounds check or a mask test: a neighbour outside the
//! image or the mask contributes `w · 0 = 0`, exactly what leaving it out gives.

/// The 18 neighbours with a weight (the 6 face and 12 edge neighbours of the 3 × 3 × 3 block),
/// in §4.2's order: z offset outermost, then y, then x; the 8 corners and the voxel itself have
/// weight 0 and are left out.
pub const NEIGHBOURS: [[i8; 3]; 18] = {
    let mut out = [[0i8; 3]; 18];
    let mut k = 0;
    let mut dz = -1i8;
    while dz <= 1 {
        let mut dy = -1i8;
        while dy <= 1 {
            let mut dx = -1i8;
            while dx <= 1 {
                let nonzero = (dx != 0) as u8 + (dy != 0) as u8 + (dz != 0) as u8;
                if nonzero == 1 || nonzero == 2 {
                    out[k] = [dx, dy, dz];
                    k += 1;
                }
                dx += 1;
            }
            dy += 1;
        }
        dz += 1;
    }
    out
};

/// The mask's voxels and their neighbourhood.
#[derive(Clone, Debug)]
pub struct Grid {
    pub shape: [usize; 3],
    pub voxel: [f32; 3],
    /// The padded grid's sizes.
    pub padded_shape: [usize; 3],
    /// The padded grid: each voxel's number in the mask, [`Grid::len`] outside it.
    pub map: Vec<u32>,
    /// The padded index of each mask voxel, in storage order.
    pub padded: Vec<u32>,
    /// The image index of each mask voxel.
    pub index: Vec<u32>,
    /// The padded-grid offsets of [`NEIGHBOURS`].
    pub offsets: [isize; 18],
    /// Their weights (§4.2: the inverse distance in mm, double precision rounded to single).
    pub weights: [f32; 18],
}

/// §4.2's weight of a neighbour at offset `o`, voxel sizes `d`.
pub fn weight(o: [i8; 3], d: [f32; 3]) -> f32 {
    let [dx, dy, dz] = d.map(f64::from);
    match (o[0] != 0, o[1] != 0, o[2] != 0) {
        (true, false, false) => (1.0 / dx) as f32,
        (false, true, false) => (1.0 / dy) as f32,
        (false, false, true) => (1.0 / dz) as f32,
        (false, true, true) => (1.0 / (dz * dz + dy * dy).sqrt()) as f32,
        (true, false, true) => (1.0 / (dz * dz + dx * dx).sqrt()) as f32,
        (true, true, false) => (1.0 / (dx * dx + dy * dy).sqrt()) as f32,
        _ => 0.0,
    }
}

impl Grid {
    /// The grid of an image of `shape` and voxel sizes `voxel`, its mask the voxels for which
    /// `in_mask(index)` holds. `None` when the padded grid has 2^32 voxels or more.
    pub fn new(
        shape: [usize; 3],
        voxel: [f32; 3],
        in_mask: impl Fn(usize) -> bool,
    ) -> Option<Grid> {
        let [nx, ny, nz] = shape;
        let [px, py, pz] = [nx + 2, ny + 2, nz + 2];
        let total = px.checked_mul(py)?.checked_mul(pz)?;
        if total >= u32::MAX as usize {
            return None;
        }
        let mut padded = Vec::new();
        let mut index = Vec::new();
        for z in 0..nz {
            for y in 0..ny {
                let row = (z * ny + y) * nx;
                let prow = ((z + 1) * py + y + 1) * px + 1;
                for x in 0..nx {
                    if in_mask(row + x) {
                        padded.push((prow + x) as u32);
                        index.push((row + x) as u32);
                    }
                }
            }
        }
        let n = padded.len() as u32;
        let mut map = vec![n; total];
        for (k, &p) in padded.iter().enumerate() {
            map[p as usize] = k as u32;
        }
        let mut offsets = [0isize; 18];
        let mut weights = [0f32; 18];
        for (k, o) in NEIGHBOURS.iter().enumerate() {
            offsets[k] = (isize::from(o[2]) * py as isize + isize::from(o[1])) * px as isize
                + isize::from(o[0]);
            weights[k] = weight(*o, voxel);
        }
        Some(Grid {
            shape,
            voxel,
            padded_shape: [px, py, pz],
            map,
            padded,
            index,
            offsets,
            weights,
        })
    }

    /// The number of mask voxels.
    pub fn len(&self) -> usize {
        self.padded.len()
    }

    pub fn is_empty(&self) -> bool {
        self.padded.is_empty()
    }

    /// The image's voxel count.
    pub fn voxels(&self) -> usize {
        self.shape.iter().product()
    }

    /// The mask numbers of mask voxel `i`'s weighted neighbours ([`Grid::len`] for those
    /// outside the mask or the image).
    #[inline(always)]
    pub fn neighbours(&self, i: usize) -> [usize; 18] {
        let p = self.padded[i] as isize;
        let mut out = [0usize; 18];
        for k in 0..18 {
            out[k] = self.map[(p + self.offsets[k]) as usize] as usize;
        }
        out
    }

    /// A per-voxel quantity over the whole image: `compact[i]` at mask voxel `i`, `outside`
    /// elsewhere.
    pub fn expand<T: Copy>(&self, compact: impl Fn(usize) -> T, outside: T) -> Vec<T> {
        let mut out = vec![outside; self.voxels()];
        for (i, &v) in self.index.iter().enumerate() {
            out[v as usize] = compact(i);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbourhood() {
        assert_eq!(NEIGHBOURS[0], [0, -1, -1]);
        assert_eq!(NEIGHBOURS[2], [0, 0, -1]);
        assert_eq!(NEIGHBOURS[17], [0, 1, 1]);
        let g = Grid::new([3, 3, 3], [1.0, 2.0, 2.0], |i| i != 0).unwrap();
        assert_eq!(g.len(), 26);
        // the centre voxel (13 in the image, 12 in the mask) sees its 18 neighbours, one of them
        // (the corner, index 0) is outside the mask but has weight 0 anyway
        let nb = g.neighbours(12);
        assert!(nb.iter().all(|&j| j < 26));
        assert_eq!(weight([1, 0, 0], [1.0, 2.0, 2.0]), 1.0);
        assert_eq!(
            weight([0, 1, 1], [1.0, 2.0, 2.0]),
            (1.0 / 8f64.sqrt()) as f32
        );
        // a voxel at the image's edge: its outside neighbours read the row of zeros
        assert!(g.neighbours(0).contains(&26));
    }
}
