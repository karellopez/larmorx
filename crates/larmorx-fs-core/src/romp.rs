// SPDX-License-Identifier: Apache-2.0
//! Threads and FreeSurfer's parallel loops (spec §19): the 128-thread limit, the loops' levels
//! (only the reproducible levels run on several threads) and the reproducible sums, whose pieces
//! make any thread count give the same bits.

/// The most threads a FreeSurfer program runs on.
pub const MAX_THREADS: usize = 128;

/// The threads a program's parallel loops use: `OMP_NUM_THREADS` (C's `atoi`), at most 128;
/// one when it is unset or not a positive number (FreeSurfer's start-up default).
pub fn thread_count(omp_num_threads: Option<&[u8]>) -> usize {
    match omp_num_threads.map(crate::clib::atoi) {
        Some(n) if n > 0 => (n as usize).min(MAX_THREADS),
        _ => 1,
    }
}

/// A parallel loop's level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Serial = 0,
    Experimental = 1,
    Fast = 2,
    AssumeReproducible = 3,
    ShownReproducible = 4,
}

impl Level {
    /// Every level, in order.
    pub const ALL: [Level; 5] = [
        Level::Serial,
        Level::Experimental,
        Level::Fast,
        Level::AssumeReproducible,
        Level::ShownReproducible,
    ];

    /// Whether a loop of this level runs on several threads in the official build
    /// (`ROMP_if_parallel1`): only the two reproducible levels.
    pub fn parallel(self) -> bool {
        matches!(self, Level::AssumeReproducible | Level::ShownReproducible)
    }
}

/// The pieces of a reproducible loop over `lo ≤ i < hi` (`ROMP_Distributor_begin`):
/// `min(hi - lo, 128)` consecutive pieces, piece j holding ⌊(hi - start)/(p - j)⌋ members.
pub fn pieces(lo: i64, hi: i64) -> Vec<(i64, i64)> {
    if hi <= lo {
        return Vec::new();
    }
    let p = (hi - lo).min(MAX_THREADS as i64);
    let mut out = Vec::with_capacity(p as usize);
    let mut start = lo;
    for j in 0..p {
        let len = (hi - start) / (p - j);
        out.push((start, start + len));
        start += len;
    }
    out
}

/// A reproducible sum: `initial` plus the pieces' sums (each from 0.0, its terms in loop order)
/// in piece order, in double; the pieces summed on `n_threads` threads, the same bits on any
/// count.
pub fn reproducible_sum(initial: f64, terms: &[f64], n_threads: usize) -> f64 {
    let parts = pieces(0, terms.len() as i64);
    let piece_sum = |&(a, b): &(i64, i64)| {
        terms[a as usize..b as usize]
            .iter()
            .fold(0.0f64, |s, &t| crate::sse::add64(s, t))
    };
    let sums: Vec<f64> = if n_threads > 1 && parts.len() > 1 {
        larmorx_core::parallel::with_threads(n_threads, || {
            use rayon::prelude::*;
            parts.par_iter().map(piece_sum).collect()
        })
        .unwrap_or_else(|_| parts.iter().map(piece_sum).collect())
    } else {
        parts.iter().map(piece_sum).collect()
    };
    sums.into_iter().fold(initial, crate::sse::add64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_of_300() {
        let p = pieces(0, 300);
        assert_eq!(p.len(), 128);
        assert_eq!(p.iter().filter(|(a, b)| b - a == 2).count(), 84);
        assert_eq!(p.iter().filter(|(a, b)| b - a == 3).count(), 44);
        assert!(pieces(5, 5).is_empty());
        assert_eq!(pieces(-5, -3), vec![(-5, -4), (-4, -3)]);
    }

    #[test]
    fn threads_and_sums() {
        assert_eq!(thread_count(None), 1);
        assert_eq!(thread_count(Some(b"1000")), 128);
        let t: Vec<f64> = (0..1000).map(|i| (i as f64 - 500.5) * 1e-3).collect();
        assert_eq!(
            reproducible_sum(1.0, &t, 1).to_bits(),
            reproducible_sum(1.0, &t, 8).to_bits()
        );
    }
}
