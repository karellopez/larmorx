//! gzip compression in parallel, with output independent of the number of threads.
//!
//! The input is cut into fixed-size blocks. Each block is deflated independently, primed with
//! the 32 KiB that precede it as a dictionary, and ended on a byte boundary with a sync flush;
//! the last one is finished. The concatenation is one ordinary gzip member that any reader
//! can decompress (the method of pigz). Because block boundaries do not depend on the thread
//! count, neither do the output bytes.

use std::io::{self, Write};

use flate2::{Compress, Compression, FlushCompress, Status};
use rayon::prelude::*;

/// Input bytes per independently compressed block.
pub const BLOCK_SIZE: usize = 256 * 1024;
/// Deflate's window: the dictionary handed to each block.
const DICT_SIZE: usize = 32 * 1024;
/// Blocks compressed together per batch (bounds the memory held by the encoder).
const BATCH_BLOCKS: usize = 64;

/// A gzip encoder that compresses blocks in parallel on the current rayon pool.
///
/// Call [`ParallelGzEncoder::finish`] to write the last block and the trailer.
pub struct ParallelGzEncoder<W: Write> {
    inner: W,
    level: u32,
    pending: Vec<u8>,
    dictionary: Vec<u8>,
    crc: crc32fast::Hasher,
    total_in: u64,
}

impl<W: Write> ParallelGzEncoder<W> {
    /// A new encoder at `level` (0 = store, 1 = fastest, 9 = smallest). Writes the gzip header
    /// at once: no file name, modification time 0, OS "unknown" (as Python's `gzip`).
    pub fn new(mut inner: W, level: u32) -> io::Result<Self> {
        let level = level.min(9);
        let xfl = match level {
            1 => 4,
            9 => 2,
            _ => 0,
        };
        inner.write_all(&[0x1f, 0x8b, 8, 0, 0, 0, 0, 0, xfl, 255])?;
        Ok(ParallelGzEncoder {
            inner,
            level,
            pending: Vec::with_capacity(BLOCK_SIZE * BATCH_BLOCKS),
            dictionary: Vec::new(),
            crc: crc32fast::Hasher::new(),
            total_in: 0,
        })
    }

    /// Compresses the complete blocks held in `pending`, keeping any partial block.
    fn flush_blocks(&mut self) -> io::Result<()> {
        let n_blocks = self.pending.len() / BLOCK_SIZE;
        if n_blocks == 0 {
            return Ok(());
        }
        let data = &self.pending[..n_blocks * BLOCK_SIZE];
        let dictionary = &self.dictionary;
        let level = self.level;
        let results: Vec<io::Result<(Vec<u8>, crc32fast::Hasher)>> = (0..n_blocks)
            .into_par_iter()
            .map(|i| {
                let block = &data[i * BLOCK_SIZE..(i + 1) * BLOCK_SIZE];
                let dict = if i == 0 {
                    dictionary.as_slice()
                } else {
                    &data[i * BLOCK_SIZE - DICT_SIZE..i * BLOCK_SIZE]
                };
                let mut crc = crc32fast::Hasher::new();
                crc.update(block);
                Ok((deflate_block(dict, block, level, false)?, crc))
            })
            .collect();
        for result in results {
            let (bytes, crc) = result?;
            self.inner.write_all(&bytes)?;
            self.crc.combine(&crc);
        }
        self.total_in += (n_blocks * BLOCK_SIZE) as u64;
        self.dictionary = data[data.len() - DICT_SIZE..].to_vec();
        self.pending.drain(..n_blocks * BLOCK_SIZE);
        Ok(())
    }

    /// Compresses the remaining input, writes the trailer and returns the inner writer.
    pub fn finish(mut self) -> io::Result<W> {
        self.flush_blocks()?;
        let last = std::mem::take(&mut self.pending);
        self.inner
            .write_all(&deflate_block(&self.dictionary, &last, self.level, true)?)?;
        self.crc.update(&last);
        self.total_in += last.len() as u64;
        self.inner
            .write_all(&self.crc.clone().finalize().to_le_bytes())?;
        self.inner
            .write_all(&(self.total_in as u32).to_le_bytes())?; // ISIZE: length mod 2^32
        self.inner.flush()?;
        Ok(self.inner)
    }
}

impl<W: Write> Write for ParallelGzEncoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(buf);
        if self.pending.len() >= BLOCK_SIZE * BATCH_BLOCKS {
            self.flush_blocks()?;
        }
        Ok(buf.len())
    }

    /// Does nothing: blocks are only emitted when complete, to keep the output independent of
    /// how the input was split into writes.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Raw-deflates `input` primed with `dict`; ends with a sync flush, or with the final block.
fn deflate_block(dict: &[u8], input: &[u8], level: u32, last: bool) -> io::Result<Vec<u8>> {
    let mut c = Compress::new(Compression::new(level), false);
    if !dict.is_empty() {
        c.set_dictionary(dict).map_err(io::Error::other)?;
    }
    let flush = if last {
        FlushCompress::Finish
    } else {
        FlushCompress::Sync
    };
    let mut out = Vec::with_capacity(input.len() + input.len() / 1000 + 64);
    loop {
        let consumed = usize::try_from(c.total_in()).unwrap_or(usize::MAX);
        if out.capacity() - out.len() < 64 {
            out.reserve(out.capacity().max(4096));
        }
        let status = c
            .compress_vec(&input[consumed..], &mut out, flush)
            .map_err(io::Error::other)?;
        let done_input = usize::try_from(c.total_in()).unwrap_or(usize::MAX) == input.len();
        match status {
            Status::StreamEnd => break,
            // A sync flush is complete once all input is consumed and output space remains.
            Status::Ok | Status::BufError if !last && done_input && out.len() < out.capacity() => {
                break;
            }
            Status::Ok | Status::BufError => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::MultiGzDecoder;
    use std::io::Read;

    fn sample(n: usize) -> Vec<u8> {
        // Compressible but not trivial: a slow ramp with pseudo-random noise.
        let mut x = 0x2545_f491_4f6c_dd1du64;
        (0..n)
            .map(|i| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                ((i / 512) as u8).wrapping_add((x % 7) as u8)
            })
            .collect()
    }

    fn compress(data: &[u8], level: u32, threads: usize, chunk: usize) -> Vec<u8> {
        larmorx_core::parallel::with_threads(threads, || {
            let mut enc = ParallelGzEncoder::new(Vec::new(), level).unwrap();
            for piece in data.chunks(chunk.max(1)) {
                enc.write_all(piece).unwrap();
            }
            enc.finish().unwrap()
        })
        .unwrap()
    }

    #[test]
    fn round_trips_and_is_independent_of_threads_and_write_sizes() {
        for n in [
            0,
            1,
            1000,
            BLOCK_SIZE,
            BLOCK_SIZE + 1,
            3 * BLOCK_SIZE + 12345,
            BATCH_BLOCKS * BLOCK_SIZE + 7,
        ] {
            let data = sample(n);
            let reference = compress(&data, 1, 1, usize::MAX);
            for (threads, chunk) in [(2, 4096), (4, 1_000_003), (8, 1)] {
                if chunk == 1 && n > BLOCK_SIZE {
                    continue; // byte-by-byte writes are slow; covered for small inputs
                }
                assert_eq!(
                    compress(&data, 1, threads, chunk),
                    reference,
                    "n={n} threads={threads}"
                );
            }
            let mut back = Vec::new();
            MultiGzDecoder::new(reference.as_slice())
                .read_to_end(&mut back)
                .unwrap();
            assert_eq!(back, data, "n={n}");
        }
    }

    #[test]
    fn all_levels_decompress() {
        let data = sample(2 * BLOCK_SIZE + 99);
        for level in 0..=9 {
            let gz = compress(&data, level, 3, 65536);
            let mut back = Vec::new();
            MultiGzDecoder::new(gz.as_slice())
                .read_to_end(&mut back)
                .unwrap();
            assert_eq!(back, data, "level {level}");
        }
    }
}
