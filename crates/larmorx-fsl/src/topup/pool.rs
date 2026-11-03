// SPDX-License-Identifier: Apache-2.0
//! The large arrays one level of topup's estimation keeps for reuse (`specs/topup.md` §11).

use rayon::prelude::*;

use super::spline::Op1;

/// Large arrays kept for reuse within one level (§11): the arrays a cost evaluation, its
/// gradient and the regulariser free are taken by the next ones, instead of new memory from the
/// system (whose first touch costs a page fault per 4 KiB page, and the faults of several
/// threads queue in the kernel). An array is taken back only at its exact length and zeroed
/// before use, so the arithmetic is unchanged; the pool holds only arrays that were in use
/// before, so it does not raise the peak memory much (at most [`Pool::KEPT`] arrays).
#[derive(Debug, Default)]
pub struct Pool {
    free: std::sync::Mutex<Vec<Vec<f64>>>,
}

impl Clone for Pool {
    fn clone(&self) -> Pool {
        Pool::default()
    }
}

impl Pool {
    /// Arrays shorter than this are not kept (the allocator reuses them without page faults).
    const MIN: usize = 1 << 15;
    /// At most this many arrays are kept.
    const KEPT: usize = 48;

    /// A zeroed array of `n` values.
    pub fn zeros(&self, n: usize) -> Vec<f64> {
        if n >= Self::MIN {
            let kept = {
                let mut f = self.free.lock().unwrap_or_else(|e| e.into_inner());
                f.iter()
                    .rposition(|v| v.len() == n)
                    .map(|i| f.swap_remove(i))
            };
            if let Some(mut v) = kept {
                v.par_chunks_mut(1 << 14).for_each(|c| c.fill(0.0));
                return v;
            }
        }
        vec![0.0; n]
    }

    /// An array of `n` values whose contents are left as they are: for a caller that writes
    /// every value before reading any.
    pub fn take(&self, n: usize) -> Vec<f64> {
        if n >= Self::MIN {
            let mut f = self.free.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(i) = f.iter().rposition(|v| v.len() == n) {
                return f.swap_remove(i);
            }
        }
        vec![0.0; n]
    }

    /// Gives `v` back for reuse.
    pub fn give(&self, v: Vec<f64>) {
        if v.len() >= Self::MIN {
            let mut f = self.free.lock().unwrap_or_else(|e| e.into_inner());
            if f.len() < Self::KEPT {
                f.push(v);
            }
        }
    }

    /// [`Op1::apply_axis`] with its output from the pool.
    pub fn apply_axis(
        &self,
        op: &Op1,
        data: &[f64],
        dims: [usize; 3],
        axis: usize,
    ) -> (Vec<f64>, [usize; 3]) {
        let mut od = dims;
        od[axis] = op.out;
        op.apply_axis_with(data, dims, axis, self.zeros(od[0] * od[1] * od[2]))
    }
}
