// SPDX-License-Identifier: Apache-2.0
//! Random numbers (spec §9): one generator per program (VXL's `vnl_random`, through a draw
//! function that reseeds it only for a new seed and returns `drand64(0, 1)` rounded to float),
//! the seeding, the numbers between two bounds, `FREESURFER_SEED` (FS-04, FS-05) and the
//! permutations.

use larmorx_core::defects::{FS_04, FS_05};
use larmorx_core::glibc_random::GlibcRandom;
use larmorx_core::vnl_random::VnlRandom;

use crate::clib::{self, Stol};
use crate::process::{Process, Stop, codes};

/// A program's random state: the current seed and the draw count, the draw function's last seed
/// and its generator, the C library's `rand`/`random` state and whether the "seed not set" line
/// was written.
#[derive(Clone, Debug)]
pub(crate) struct RandomState {
    seed: i64,
    calls: i64,
    last_seed: i64,
    generator: VnlRandom,
    pub(crate) c_random: GlibcRandom,
    warned: bool,
}

impl Default for RandomState {
    fn default() -> Self {
        RandomState {
            seed: 0,
            calls: 0,
            last_seed: 0,
            generator: VnlRandom::new(0),
            c_random: GlibcRandom::new(),
            warned: false,
        }
    }
}

impl RandomState {
    /// The draw function (`OpenRan1(seed)`): reseeds for a seed other than the last one (its 64
    /// bits read as an unsigned number), then `drand64(0, 1)` rounded to float.
    fn draw(&mut self, seed: i64) -> f32 {
        if seed != self.last_seed {
            self.generator.reseed(seed as u64);
            self.last_seed = seed;
        }
        self.generator.drand64(0.0, 1.0) as f32
    }
}

/// The process's name as Linux keeps it (`/proc/self/comm`), empty without `/proc`.
pub fn process_comm() -> Vec<u8> {
    match std::fs::read("/proc/self/comm") {
        Ok(mut b) => {
            if b.last() == Some(&b'\n') {
                b.pop();
            }
            b
        }
        Err(_) => Vec::new(),
    }
}

/// The clock's seconds, for FS-04's `reproduce`.
fn clock_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Process {
    /// `setRandomSeed(seed)`.
    pub fn set_random_seed(&mut self, seed: i64) {
        let r = &mut self.random;
        r.c_random.seed_c_long(seed);
        if seed == r.seed {
            r.draw(seed.wrapping_sub(1));
        }
        r.seed = seed;
        r.draw(seed);
        r.calls = 1;
    }

    /// `getRandomSeed`.
    pub fn random_seed(&self) -> i64 {
        self.random.seed
    }

    /// `getRandomCalls`.
    pub fn random_calls(&self) -> i64 {
        self.random.calls
    }

    /// The C library's `rand`/`random` state the seeding sets (for programs that call them).
    pub fn c_random(&mut self) -> &mut GlibcRandom {
        &mut self.random.c_random
    }

    /// The "no seed yet" check of every draw (current seed 0): `FREESURFER_SEED` when set and
    /// not empty, else the "seed not set" line and FS-04's seed.
    fn seed_if_unseeded(&mut self) -> Result<(), Stop> {
        if self.random.seed != 0 {
            return Ok(());
        }
        if let Some(v) = self.env.non_empty("FREESURFER_SEED").map(<[u8]>::to_vec) {
            let mut line = b"Setting seed for random number genererator to ".to_vec();
            line.extend_from_slice(&v);
            line.push(b'\n');
            self.put_out(&line);
            self.flush();
            let seed = match clib::stol(&v) {
                Stol::Value(s) => s,
                bad => return Err(self.bad_seed(&v, bad)),
            };
            self.set_random_seed(seed);
            return Ok(());
        }
        if !self.random.warned {
            self.random.warned = true;
            let mut line = process_comm();
            line.extend_from_slice(b" supposed to be reproducible but seed not set\n");
            self.put_err(&line);
        }
        let seed = if self.mode.reproduce() {
            self.note(&FS_04, "the random numbers seeded from the clock");
            -((clock_seconds() as i32).unsigned_abs() as i64)
        } else {
            self.note(
                &FS_04,
                "the random numbers seeded with -1 instead of the clock",
            );
            -1
        };
        self.random.seed = seed;
        Ok(())
    }

    /// FS-05: a `FREESURFER_SEED` that is no 64-bit integer.
    fn bad_seed(&mut self, value: &[u8], why: Stol) -> Stop {
        if self.mode.reproduce() {
            self.note(
                &FS_05,
                "the program aborted (status 134) on a FREESURFER_SEED that is no 64-bit integer",
            );
            let kind = if why == Stol::OutOfRange {
                "std::out_of_range"
            } else {
                "std::invalid_argument"
            };
            self.put_err(
                format!(
                    "terminate called after throwing an instance of '{kind}'\n  what():  stol\n"
                )
                .as_bytes(),
            );
            self.flush();
            return Stop::Exit(134);
        }
        self.note(&FS_05, "a clear error and status 1 instead of an abort");
        let mut line =
            b"error: FREESURFER_SEED must be an integer in the range of a 64-bit long, not '"
                .to_vec();
        line.extend_from_slice(value);
        line.extend_from_slice(b"'\n");
        self.put_err(&line);
        Stop::Exit(1)
    }

    /// `randomNumber(low, hi)`: the float draw times `hi - low`, plus `low` (double; the bounds
    /// swapped when low > hi); a result outside [low, hi] is reported as a C-style error and
    /// returned anyway.
    pub fn random_number(&mut self, low: f64, hi: f64) -> Result<f64, Stop> {
        let (low, hi) = if low > hi { (hi, low) } else { (low, hi) };
        self.seed_if_unseeded()?;
        let seed = self.random.seed;
        let f = self.random.draw(seed);
        self.random.calls += 1;
        let v = f64::from(f) * (hi - low) + low;
        if v < low || v > hi {
            let p =
                |x: f64| larmorx_core::cfmt::pad(&larmorx_core::cfmt::f(f64::from(x as f32), 1), 2);
            let m = format!("randomNumber({}, {}) - {}\n", p(low), p(hi), p(v));
            self.error_printf(codes::BAD_PARM, m.as_bytes());
        }
        Ok(v)
    }

    /// `compute_permutation(n)`: from the identity, each place in order swapped with the place
    /// a number between 0 and `n - 0.0001` gives, truncated.
    pub fn compute_permutation(&mut self, n: i32) -> Result<Vec<i32>, Stop> {
        let n = n.max(0);
        let mut p: Vec<i32> = (0..n).collect();
        for i in 0..n as usize {
            let j = self.random_number(0.0, f64::from(n) - 0.0001)? as i32;
            p.swap(i, j as usize);
        }
        Ok(p)
    }
}
