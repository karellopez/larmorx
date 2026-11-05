// SPDX-License-Identifier: Apache-2.0
//! Volumes as the estimation sees them (`specs/mcflirt.md` §4.1): float32 values, x fastest,
//! in "radiological" order (x reversed in memory for images whose affine has a positive
//! determinant), with the voxel sizes of the header's `pixdim`.

/// A 3D float32 volume. Voxel `(x, y, z)` is `data[x + nx·(y + ny·z)]`; its FSL-mm position
/// is `(x·dx, y·dy, z·dz)` (§4.2).
#[derive(Clone, Debug, PartialEq)]
pub struct Volume {
    pub shape: [usize; 3],
    pub voxel_size: [f32; 3],
    pub data: Vec<f32>,
}

impl Volume {
    /// A volume of `shape` holding `data` (x fastest).
    ///
    /// # Panics
    /// If `data` does not have `shape`'s number of voxels.
    pub fn new(shape: [usize; 3], voxel_size: [f32; 3], data: Vec<f32>) -> Volume {
        assert_eq!(
            data.len(),
            shape.iter().product::<usize>(),
            "volume data do not match the shape {shape:?}"
        );
        Volume {
            shape,
            voxel_size,
            data,
        }
    }

    /// The number of voxels.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether the volume has no voxels.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// The value at voxel `(x, y, z)`.
    #[inline]
    pub fn at(&self, x: usize, y: usize, z: usize) -> f32 {
        self.data[x + self.shape[0] * (y + self.shape[1] * z)]
    }

    /// The smallest value, found the way a running `if v < min` loop finds it from the first
    /// voxel: NaN values are passed over, unless the first voxel is NaN.
    pub fn min(&self) -> f32 {
        let mut it = self.data.iter();
        let Some(&first) = it.next() else {
            return 0.0;
        };
        it.fold(first, |m, &v| if v < m { v } else { m })
    }

    /// Whether some voxel is NaN or infinite.
    pub fn has_non_finite(&self) -> bool {
        self.data.iter().any(|v| !v.is_finite())
    }

    /// The intensity-weighted centre in FSL-mm (§6.1): `d ⊙ Σ (v − v_min)·idx / Σ (v − v_min)`,
    /// as every centre of mass of FLIRT (`specs/flirt.md` §8.1 step 3; `specs/mcflirt.md` §6.1,
    /// spec writer 2026-11-03): `v − v_min` in float32, converted to double; double running sums
    /// over the voxels in z, y, x order, flushed into the totals after every
    /// `max(1000, trunc(√N)) + 1` voxels (each block's sums restart at 0 and are added to the
    /// totals, the last block's at the end); each coordinate its total over the total, times
    /// the voxel size. A total below 1e−5 in magnitude counts as 1. One NaN voxel makes the
    /// centre NaN, as in mcflirt (FSL-14).
    pub fn centre_of_mass(&self) -> [f64; 3] {
        self.centre(false).0
    }

    /// [`Volume::centre_of_mass`] with NaN and infinite voxels left out (of the minimum and of
    /// the sums; they still count in the blocks' lengths): larmorx's correction of FSL-14. For
    /// a volume without such voxels it is the same computation, so the same bits.
    pub fn centre_of_mass_finite(&self) -> [f64; 3] {
        self.centre(true).0
    }

    /// The centre ([`Volume::centre_of_mass`], or with `finite_only` the finite one) and
    /// whether its total was below 1e−5 in magnitude (counted as 1; mcflirt prints
    /// `WARNING::in calculating COG, total = 0.0` there, §6.1; a NaN total is not below).
    pub fn centre_counted(&self, finite_only: bool) -> ([f64; 3], bool) {
        self.centre(finite_only)
    }

    fn centre(&self, finite_only: bool) -> ([f64; 3], bool) {
        let [nx, ny, nz] = self.shape;
        let vmin = if finite_only {
            let mut it = self.data.iter().copied().filter(|v| v.is_finite());
            it.next().map_or(0.0, |first| {
                it.fold(first, |m, v| if v < m { v } else { m })
            })
        } else {
            self.min()
        };
        let block = ((self.data.len() as f64).sqrt() as usize).max(1000) + 1;
        // [Σ w·x, Σ w·y, Σ w·z, Σ w]: the totals and the current block's sums
        let mut tot = [0.0f64; 4];
        let mut part = [0.0f64; 4];
        let mut count = 0usize;
        for z in 0..nz {
            for y in 0..ny {
                let row = &self.data[nx * (y + ny * z)..nx * (y + ny * z + 1)];
                for (x, &v) in row.iter().enumerate() {
                    if !(finite_only && !v.is_finite()) {
                        let w = f64::from(v - vmin);
                        part[0] += w * x as f64;
                        part[1] += w * y as f64;
                        part[2] += w * z as f64;
                        part[3] += w;
                    }
                    count += 1;
                    if count == block {
                        for k in 0..4 {
                            tot[k] += part[k];
                            part[k] = 0.0;
                        }
                        count = 0;
                    }
                }
            }
        }
        for k in 0..4 {
            tot[k] += part[k];
        }
        let mut total = tot[3];
        let zero = total.abs() < 1e-5;
        if zero {
            total = 1.0;
        }
        let d = self.voxel_size.map(f64::from);
        (
            [
                d[0] * (tot[0] / total),
                d[1] * (tot[1] / total),
                d[2] * (tot[2] / total),
            ],
            zero,
        )
    }

    /// The background value (§9.2): the value at index `⌊count / 10⌋` of the sorted voxels of
    /// the outer shell two voxels thick.
    pub fn background(&self) -> f32 {
        let [nx, ny, nz] = self.shape;
        let edge = |i: usize, n: usize| i < 2 || i + 2 >= n;
        let mut shell = Vec::new();
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    if edge(x, nx) || edge(y, ny) || edge(z, nz) {
                        shell.push(self.at(x, y, z));
                    }
                }
            }
        }
        if shell.is_empty() {
            return 0.0;
        }
        shell.sort_unstable_by(f32::total_cmp);
        shell[shell.len() / 10]
    }

    /// The volume with its x axis reversed (the in-memory order of a positive-determinant
    /// image, and back).
    /// [`Volume::flipped_x`] in place (each row reversed; no copy of the volume).
    pub fn flip_x_in_place(&mut self) {
        let nx = self.shape[0];
        for row in self.data.chunks_exact_mut(nx.max(1)) {
            row.reverse();
        }
    }

    pub fn flipped_x(&self) -> Volume {
        let nx = self.shape[0];
        let mut data = Vec::with_capacity(self.data.len());
        for row in self.data.chunks_exact(nx.max(1)) {
            data.extend(row.iter().rev());
        }
        Volume::new(self.shape, self.voxel_size, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centre_of_mass_subtracts_the_minimum() {
        // A constant background of 100 with one bright voxel: the centre is that voxel.
        let mut data = vec![100.0f32; 4 * 3 * 2];
        data[1 + 4 * (2 + 3)] = 300.0;
        let v = Volume::new([4, 3, 2], [2.0, 3.0, 4.0], data);
        assert_eq!(v.centre_of_mass(), [2.0, 6.0, 4.0]);
        // A constant volume: the denominator is 0, replaced by 1, so the centre is 0.
        let c = Volume::new([2, 2, 2], [1.0; 3], vec![5.0; 8]).centre_of_mass();
        assert_eq!(c, [0.0; 3]);
    }

    /// The sums run in blocks of max(1000, trunc(√N)) + 1 voxels (§6.1): on a volume of more
    /// than one block the centre is the blocked sums' bits, which a single running sum misses.
    #[test]
    fn the_centre_sums_in_blocks() {
        let shape = [37usize, 29, 23];
        let n: usize = shape.iter().product();
        let mut s: u64 = 99;
        let data: Vec<f32> = (0..n)
            .map(|_| {
                s = s
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                // a wide range of magnitudes, so that the double sums round
                ((s >> 40) % 4096) as f32
                    * 0.37
                    * [1e-3f32, 1.0, 1e3, 1e5][((s >> 20) % 4) as usize]
                    + 11.0
            })
            .collect();
        let v = Volume::new(shape, [1.1, 0.9, 2.3], data.clone());
        let vmin = v.min();
        let block = ((n as f64).sqrt() as usize).max(1000) + 1;
        let (mut tot, mut part, mut count) = ([0.0f64; 4], [0.0f64; 4], 0usize);
        let (mut single, mut single_w) = ([0.0f64; 3], 0.0f64);
        for (i, &x) in data.iter().enumerate() {
            let w = f64::from(x - vmin);
            let idx = [i % 37, (i / 37) % 29, i / (37 * 29)].map(|c| c as f64);
            for a in 0..3 {
                part[a] += w * idx[a];
                single[a] += w * idx[a];
            }
            part[3] += w;
            single_w += w;
            count += 1;
            if count == block {
                for k in 0..4 {
                    tot[k] += part[k];
                    part[k] = 0.0;
                }
                count = 0;
            }
        }
        for k in 0..4 {
            tot[k] += part[k];
        }
        let d = [1.1f32, 0.9, 2.3].map(f64::from);
        let want = [0, 1, 2].map(|a| d[a] * (tot[a] / tot[3]));
        assert_eq!(v.centre_of_mass(), want);
        let one_sum = [0, 1, 2].map(|a| d[a] * (single[a] / single_w));
        assert_ne!(want, one_sum, "the blocks must matter on this volume");
        assert_eq!(v.centre_counted(false), (want, false));
        // a constant volume: total 0, counted as 1 and reported
        let c = Volume::new([2, 2, 2], [1.0; 3], vec![5.0; 8]);
        assert_eq!(c.centre_counted(false), ([0.0; 3], true));
    }

    #[test]
    fn nan_voxels_make_the_centre_nan() {
        let mut data = vec![1.0f32; 8];
        data[3] = f32::NAN;
        let v = Volume::new([2, 2, 2], [1.0; 3], data);
        assert!(v.centre_of_mass().iter().all(|c| c.is_nan()));
        assert!(v.has_non_finite());
    }

    /// FSL-14 corrected: non-finite voxels are left out; without them the two centres are the
    /// same bits.
    #[test]
    fn the_finite_centre_leaves_non_finite_voxels_out() {
        let mut data = vec![100.0f32; 4 * 3 * 2];
        data[1 + 4 * (2 + 3)] = 300.0;
        let clean = Volume::new([4, 3, 2], [2.0, 3.0, 4.0], data.clone());
        assert_eq!(clean.centre_of_mass_finite(), clean.centre_of_mass());
        assert!(!clean.has_non_finite());
        data[0] = f32::NAN;
        data[5] = f32::NEG_INFINITY;
        let dirty = Volume::new([4, 3, 2], [2.0, 3.0, 4.0], data);
        assert_eq!(dirty.centre_of_mass_finite(), [2.0, 6.0, 4.0]);
    }

    #[test]
    fn background_is_the_tenth_percentile_of_the_shell() {
        // 5 x 5 x 5: every voxel but the centre is in the shell (124 voxels).
        let data: Vec<f32> = (0..125).map(|i| i as f32).collect();
        let v = Volume::new([5, 5, 5], [1.0; 3], data);
        let mut shell: Vec<f32> = (0..125).filter(|&i| i != 62).map(|i| i as f32).collect();
        shell.sort_by(f32::total_cmp);
        assert_eq!(v.background(), shell[12]);
    }

    #[test]
    fn flipping_twice_is_the_identity() {
        let data: Vec<f32> = (0..24).map(|i| i as f32).collect();
        let v = Volume::new([4, 3, 2], [1.0; 3], data);
        let f = v.flipped_x();
        assert_eq!(f.at(0, 1, 1), v.at(3, 1, 1));
        assert_eq!(f.flipped_x(), v);
    }
}
