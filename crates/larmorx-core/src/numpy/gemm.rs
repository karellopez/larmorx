// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! The arithmetic of OpenBLAS's Haswell `dgemm` micro-kernel (`kernel/x86_64/
//! dgemm_kernel_4x8_haswell.S`, OpenBLAS 0.3.34 and 0.3.30: the same file) on packed panels:
//! the hot loop of `larmorx_transform::openblas::level3`, which ports the drivers around it
//! (blocking, the kernel's paths and their classes, `alpha`, `beta`, `dsyrk`, `dtrmm`).
//!
//! In every path of the kernel each element of the `C` block is one accumulator, cleared to +0
//! (`vxorpd`) or started by a product (`vmulpd`, the first step of the 4-row paths when the
//! block's depth is 8 or more), and the products `a[i][k]·b[k][j]` of the block are added to it
//! in `k`'s order with fused multiply-adds (`vfmadd231pd`/`sd`), except in the path of four rows
//! by one column, where the depth is spread over four accumulators (below). Starting from the
//! product instead of +0 changes only the sign of an exactly zero result (when every product
//! is -0), which the caller restores; so the tiles here always start from +0.
//!
//! - [`tile_4x8`]: the chains of a 4 × 8 tile (32 independent accumulators).
//! - [`tile_4x1_split`]: the 4-row, 1-column path (`KERNEL4x1`, `KERNEL4x1_SUB`, `SAVE4x1`):
//!   the first `8·⌊l/8⌋` steps go to four accumulators by `k mod 4`, the remaining `l mod 8` to
//!   the first one, and the four are added as `(acc0 + acc1) + (acc2 + acc3)`.
//!
//! `f64::mul_add` is a correctly rounded fused multiply-add on every target, so the bits are
//! the hardware's `vfmadd` on every platform; on x86-64 CPUs with AVX2 and FMA the loops are
//! compiled with those features (runtime dispatch, the same bits) so the tile runs in vector
//! registers.
//!
//! Upstream file (BSD-3-Clause, notice below): OpenBLAS 0.3.34 and 0.3.30
//! `kernel/x86_64/dgemm_kernel_4x8_haswell.S`. The logic is re-expressed in Rust; no code was
//! copied.
//!
//! OpenBLAS: Copyright (c) 2011-2014, The OpenBLAS Project. All rights reserved.
//! Redistribution and use in source and binary forms, with or without modification, are
//! permitted provided that the following conditions are met: 1. Redistributions of source code
//! must retain the above copyright notice, this list of conditions and the following
//! disclaimer. 2. Redistributions in binary form must reproduce the above copyright notice,
//! this list of conditions and the following disclaimer in the documentation and/or other
//! materials provided with the distribution. 3. Neither the name of the OpenBLAS project nor the
//! names of its contributors may be used to endorse or promote products derived from this
//! software without specific prior written permission. THIS SOFTWARE IS PROVIDED BY THE
//! COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING,
//! BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
//! PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR
//! ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
//! BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
//! PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
//! CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
//! OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

/// Rows of a packed tile.
pub const MR: usize = 4;
/// Columns of a packed tile.
pub const NR: usize = 8;

/// The accumulators of a 4 × 8 tile, `acc[c][r]` (column-major, as `C`).
pub type Tile = [[f64; MR]; NR];

#[inline(always)]
fn tile_4x8_body(ap: &[f64], bp: &[f64], l: usize) -> Tile {
    let mut acc = [[0.0f64; MR]; NR];
    for (a, b) in ap[..l * MR]
        .as_chunks::<MR>()
        .0
        .iter()
        .zip(bp[..l * NR].as_chunks::<NR>().0)
    {
        for (col, &bj) in acc.iter_mut().zip(b) {
            for (x, &ai) in col.iter_mut().zip(a) {
                *x = ai.mul_add(bj, *x);
            }
        }
    }
    acc
}

#[inline(always)]
fn tile_4x1_split_body(ap: &[f64], b: &[f64], l: usize) -> [f64; MR] {
    let mut acc = [[0.0f64; MR]; 4];
    let main = l & !7;
    for k in 0..main {
        let lane = &mut acc[k % 4];
        for (x, &ai) in lane.iter_mut().zip(&ap[k * MR..k * MR + MR]) {
            *x = ai.mul_add(b[k], *x);
        }
    }
    for k in main..l {
        for (x, &ai) in acc[0].iter_mut().zip(&ap[k * MR..k * MR + MR]) {
            *x = ai.mul_add(b[k], *x);
        }
    }
    // SAVE4x1: ymm4 = ymm5 + ymm4, ymm6 = ymm7 + ymm6, ymm4 = ymm6 + ymm4.
    std::array::from_fn(|r| (acc[0][r] + acc[1][r]) + (acc[2][r] + acc[3][r]))
}

/// The fused multiply-add chains of a 4 × 8 tile over `l` steps, each from +0: `acc[c][r] =
/// fma(a[l-1][r], b[l-1][c], … fma(a[0][r], b[0][c], +0))`, with the panels packed as
/// `ap[k·4 + r]` and `bp[k·8 + c]`.
///
/// # Panics
/// When a panel holds fewer than `l` steps.
pub fn tile_4x8(ap: &[f64], bp: &[f64], l: usize) -> Tile {
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma") {
        return fma::tile_4x8(ap, bp, l);
    }
    tile_4x8_body(ap, bp, l)
}

/// The 4-row, 1-column path of the kernel over `l` steps (module documentation), with the row
/// panel packed as `ap[k·4 + r]` and the column as `b[k]`.
///
/// # Panics
/// When a panel holds fewer than `l` steps.
pub fn tile_4x1_split(ap: &[f64], b: &[f64], l: usize) -> [f64; MR] {
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma") {
        return fma::tile_4x1_split(ap, b, l);
    }
    tile_4x1_split_body(ap, b, l)
}

/// The kernels compiled with AVX2 and FMA. AUDIT (unsafe): each call is guarded by
/// `is_x86_feature_detected!("avx2")` and `("fma")` in [`tile_4x8`] and [`tile_4x1_split`];
/// calling a `#[target_feature]` function requires `unsafe` only because the features must be
/// present, and the guard ensures it. The bodies are the portable ones (`mul_add` is the same
/// fused multiply-add either way), so the bits do not depend on the path.
#[cfg(target_arch = "x86_64")]
#[allow(unsafe_code)]
mod fma {
    use super::{MR, Tile};

    #[inline]
    pub(super) fn tile_4x8(ap: &[f64], bp: &[f64], l: usize) -> Tile {
        #[target_feature(enable = "avx2,fma")]
        fn kernel(ap: &[f64], bp: &[f64], l: usize) -> Tile {
            super::tile_4x8_body(ap, bp, l)
        }
        // SAFETY: only called after `is_x86_feature_detected!("avx2")` and `("fma")` returned
        // true.
        unsafe { kernel(ap, bp, l) }
    }

    #[inline]
    pub(super) fn tile_4x1_split(ap: &[f64], b: &[f64], l: usize) -> [f64; MR] {
        #[target_feature(enable = "avx2,fma")]
        fn kernel(ap: &[f64], b: &[f64], l: usize) -> [f64; MR] {
            super::tile_4x1_split_body(ap, b, l)
        }
        // SAFETY: as `tile_4x8`.
        unsafe { kernel(ap, b, l) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(n: usize, seed: u64) -> Vec<f64> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                s = s
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((s >> 11) as f64 / (1u64 << 53) as f64) - 0.5
            })
            .collect()
    }

    #[test]
    fn tile_is_the_chain_in_order() {
        for l in [0usize, 1, 7, 8, 9, 31] {
            let ap = values(l * MR, 1);
            let bp = values(l * NR, 2);
            let t = tile_4x8(&ap, &bp, l);
            let p = tile_4x8_body(&ap, &bp, l);
            for c in 0..NR {
                for r in 0..MR {
                    let mut want = 0.0f64;
                    for k in 0..l {
                        want = ap[k * MR + r].mul_add(bp[k * NR + c], want);
                    }
                    assert_eq!(t[c][r].to_bits(), want.to_bits());
                    assert_eq!(p[c][r].to_bits(), want.to_bits());
                }
            }
        }
    }

    #[test]
    fn split_path_spreads_the_depth() {
        for l in [0usize, 1, 7, 8, 9, 17, 40] {
            let ap = values(l * MR, 3);
            let b = values(l, 4);
            let t = tile_4x1_split(&ap, &b, l);
            for r in 0..MR {
                let mut lanes = [0.0f64; 4];
                for k in 0..l {
                    let lane = if k < (l & !7) { k % 4 } else { 0 };
                    lanes[lane] = ap[k * MR + r].mul_add(b[k], lanes[lane]);
                }
                let want = (lanes[0] + lanes[1]) + (lanes[2] + lanes[3]);
                assert_eq!(t[r].to_bits(), want.to_bits());
            }
        }
    }
}
