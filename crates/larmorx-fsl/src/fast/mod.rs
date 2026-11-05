// SPDX-License-Identifier: Apache-2.0
//! FAST's tissue segmentation (FSL 6.0.7.23's `fast`, FAST4), a clean-room original written
//! from `specs/fast.md`, Y. Zhang, M. Brady and S. Smith's paper ("Segmentation of brain MR
//! images through a hidden Markov random field model and the expectation-maximization
//! algorithm", IEEE TMI 20(1):45-57, 2001), FSL's FAST documentation and black-box runs of
//! FSL's program (see `PROVENANCE.md`).
//!
//! - [`single`]: one channel (§4): the mask, the log image, the initialisation, the MRF
//!   iterations, the bias field, the class order and the hard segmentation; the priors (§6);
//! - [`multi`]: several channels (§5);
//! - [`pve`]: the partial volumes (§4.8);
//! - [`bias`]: the bias field's kernels and separable smoothing (§4.5, §8.5);
//! - [`grid`]: the mask's voxels and their MRF neighbourhood (§4.2);
//! - [`stats`]: FSL's blocked sums, the percentiles, the class statistics and the Gaussian
//!   (§4.4, §8.2, §8.3);
//! - [`rng`]: the C library's random numbers FAST draws (§8.1).
//!
//! The command line is [`crate::cli::fast`].

pub mod bias;
pub mod grid;
pub mod multi;
pub mod pve;
pub mod rng;
pub mod single;
pub mod stats;

pub use single::{Params, Single, Stop, segment, zero_classes};

/// Standard output as FAST's C library buffers it when it is not a terminal (`specs/fast.md`
/// §7): the lines FAST flushes (written with [`std::io::Write::flush`] after them) survive an
/// abort, the others written since the last flush are lost. A run that ends normally (exit 0,
/// 1 or 255) writes [`Buffered::all`], one that aborts where FAST does [`Buffered::flushed`].
#[derive(Clone, Debug, Default)]
pub struct Buffered {
    buf: Vec<u8>,
    flushed: usize,
}

impl Buffered {
    /// Everything written.
    pub fn all(&self) -> &[u8] {
        &self.buf
    }

    /// What was written up to the last flush.
    pub fn flushed(&self) -> &[u8] {
        &self.buf[..self.flushed]
    }
}

impl std::io::Write for Buffered {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(b);
        Ok(b.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flushed = self.buf.len();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    #[test]
    fn buffered_keeps_the_flushed_lines() {
        let mut b = super::Buffered::default();
        let _ = writeln!(b, "flushed");
        let _ = b.flush();
        let _ = writeln!(b, "KMeans Iteration 0");
        assert_eq!(b.flushed(), b"flushed\n");
        assert_eq!(b.all(), b"flushed\nKMeans Iteration 0\n");
    }
}
