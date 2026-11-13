// SPDX-License-Identifier: Apache-2.0
//! RANLUX389 and the random distributions FreeSurfer 8.2.0 draws from: a **clean-room
//! implementation** of `specs/ranlux.md` (FreeSurfer's foundations, queue item 3.1-05), written
//! from that spec, the papers it cites and black-box runs of FreeSurfer's own functions only.
//!
//! - [`Ranlux`]: Lüscher's subtract-with-borrow generator with lags 24 and 10 (James's seeding,
//!   §2.2), decimated at luxury 389 (of every 389 values the first 24 are delivered, §2.4). It
//!   delivers integers in [0, 2²⁴) ([`Ranlux::next_u24`], FreeSurfer's `sc_rng_get`).
//! - Its uniform deviates ([`Ranlux::uniform`], [`Ranlux::uniform_pos`], §3) and the eight
//!   distributions (§4): [`Ranlux::flat`], [`Ranlux::gaussian`], [`Ranlux::ugaussian`],
//!   [`Ranlux::exponential`], [`Ranlux::gamma`] (with its parts [`Ranlux::gamma_int`],
//!   [`Ranlux::gamma_large`], [`Ranlux::gamma_frac`]), [`Ranlux::chisq`], [`Ranlux::fdist`],
//!   [`Ranlux::tdist`]. Each consumes the stream exactly as the spec lists it, and its result
//!   has FreeSurfer's bits: binary64 operations in the spec's order, glibc 2.35's `log` and `exp`
//!   ([`crate::math::log_glibc`], [`crate::math::exp_glibc`]), x86-64's NaNs on every platform.
//!   The one exception is `tan` (§6): larmorx keeps it correctly rounded ([`crate::math::tan`]),
//!   also under `reproduce` (the user's decision of 2026-11-04, GLIBC-07), so a gamma draw of
//!   shape 12 or more may differ from FreeSurfer's in its last bits where its tangent's argument
//!   is one of the 41,708 where glibc's `tan` is not correctly rounded.
//! - The two defects of the spec's §8, by D13: FS-100 (gamma shapes outside [0, 2³²)) and FS-101
//!   (seeds whose state is all zero), corrected by default with a notice, reproduced bit for bit
//!   with [`UpstreamDefects::Reproduce`]; draws that cannot give a number are [`DrawError`]s.
//!
//! No global or thread-local state: each caller owns its generators (FreeSurfer's `mri_coreg`
//! makes three, each seeded alike). Clone a generator to fork its stream.

mod dist;
#[cfg(test)]
pub(crate) mod sha256;
#[cfg(test)]
mod tests;

use crate::defects::{FS_101, Notice, Notices, UpstreamDefects};
use std::fmt;

/// The long lag of the recurrence: the number of values of its state.
const LONG_LAG: usize = 24;
/// The short lag.
const SHORT_LAG: usize = 10;
/// The values computed per group at luxury 389.
const GROUP: usize = 389;
/// The values delivered per group (the first ones).
const DELIVERED: usize = 24;
/// The buffer: the 24 values before a group, then the group.
const BUF: usize = LONG_LAG + GROUP;
/// The integers' range, 2²⁴.
const RANGE: i32 = 1 << 24;
/// The seed that replaces 0 (§2.2 step 1).
pub const DEFAULT_SEED: u64 = 314_159_265;

/// Why a draw gives no number (the spec's §8; the messages are its texts).
#[derive(Clone, Debug, PartialEq)]
pub enum DrawError {
    /// FS-100: a gamma shape that is NaN or infinite (by default; under `reproduce` only −∞,
    /// where FreeSurfer's draw never returns).
    ShapeNotFinite(f64),
    /// FS-100: a negative gamma shape (both modes: FreeSurfer's draw does not return in
    /// practice).
    ShapeNegative(f64),
    /// FS-101 under `reproduce`: the seed makes the stream all zero, and this draw would never
    /// return.
    NeverReturns {
        /// The generator's seed, as given.
        seed: u64,
        /// The draw, in words ("Gaussian draw").
        draw: &'static str,
    },
}

impl DrawError {
    /// The same error, naming `draw` as the one that never returns.
    fn in_draw(self, draw: &'static str) -> DrawError {
        match self {
            DrawError::NeverReturns { seed, .. } => DrawError::NeverReturns { seed, draw },
            other => other,
        }
    }
}

impl fmt::Display for DrawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DrawError::ShapeNotFinite(a) => {
                write!(f, "FS-100: the gamma shape {a} is not a finite number")
            }
            DrawError::ShapeNegative(a) => write!(f, "FS-100: the gamma shape {a} is negative"),
            DrawError::NeverReturns { seed, draw } => {
                write!(f, "FS-101: with seed {seed} FreeSurfer's {draw} never returns")
            }
        }
    }
}

impl std::error::Error for DrawError {}

/// RANLUX389 (`specs/ranlux.md` §2), seeded by James's method, delivering integers in [0, 2²⁴),
/// with the draws of §3 and §4 as methods.
///
/// The buffer holds, in `buf[0..24]`, the 24 values before the current group and, in
/// `buf[24..413]`, the group: its first 24 values are the ones delivered, the other 365 are
/// computed and discarded. A new group starts from the last 24 values of the previous one.
#[derive(Clone, Debug)]
pub struct Ranlux {
    buf: [i32; BUF],
    /// The carry after the last value computed.
    carry: i32,
    /// The index in `buf` of the next value to deliver; `LONG_LAG + DELIVERED` when the group
    /// is used up.
    next: usize,
    /// The seed, as given.
    seed: u64,
    /// The stream is constant 0 for ever (FS-101 under `reproduce`): draws that would wait for
    /// a non-zero value report [`DrawError::NeverReturns`].
    zero_forever: bool,
    notices: Notices,
    /// The tangent of the large-shape gamma draw: [`crate::math::tan`] (§6), replaced only by
    /// validation ([`Ranlux::set_tan`]).
    tan: fn(f64) -> f64,
}

impl Ranlux {
    /// A generator seeded with `seed` (§2.2; FreeSurfer's callers convert their own seeds to an
    /// unsigned 64-bit integer as C does: `-1` is `u64::MAX`). `mode` decides FS-100 and FS-101.
    #[must_use]
    pub fn new(seed: u64, mode: UpstreamDefects) -> Ranlux {
        let mut r = Ranlux {
            buf: [0; BUF],
            carry: 0,
            next: LONG_LAG + DELIVERED,
            seed,
            zero_forever: false,
            notices: Notices::new(mode),
            tan: crate::math::tan,
        };
        r.reseed(seed);
        r
    }

    /// Seeds again (§2.2): everything drawn before is discarded; the mode and the notices
    /// collected so far are kept.
    pub fn reseed(&mut self, seed: u64) {
        self.seed = seed;
        let s = if seed == 0 { DEFAULT_SEED } else { seed };
        // Schrage's evaluation of t <- 40014 t mod 2147483563, on the seed read as signed.
        let mut t = s as i64;
        // The history goes into the buffer's last 24 places, where a group's refill takes it
        // from: x_(24-k) = v_k, so x_0 = v_24 ... x_23 = v_1.
        let mut all_zero = true;
        for k in 1..=LONG_LAG {
            let q = t / 53_668;
            let r = t % 53_668;
            t = 40_014 * r - 12_211 * q;
            if t < 0 {
                t += 2_147_483_563;
            }
            let v = (t & (RANGE as i64 - 1)) as i32;
            all_zero &= v == 0;
            self.buf[BUF - k] = v;
        }
        self.carry = 0;
        self.zero_forever = false;
        self.next = LONG_LAG + DELIVERED;
        if all_zero {
            // FS-101 (§8.2): a fixed point of the recurrence.
            if self.notices.fix() {
                self.carry = 1;
                self.notices.add(
                    &FS_101,
                    format!(
                        "seed {seed} makes FreeSurfer's random generator give 0 for ever; \
                         larmorx starts it with a carry of 1 (James's 1994 initialisation)"
                    ),
                );
            } else {
                self.zero_forever = true;
                self.notices.add(
                    &FS_101,
                    format!("seed {seed} makes FreeSurfer's random generator give 0 for ever"),
                );
            }
        }
    }

    /// Replaces the tangent the large-shape gamma draw takes (§6) by `tan`. For validation only:
    /// with the C library's `tan`, run as a black box, every bit of every draw is FreeSurfer's,
    /// which shows that the correctly rounded tangent is the only cause of a difference.
    #[doc(hidden)]
    pub fn set_tan(&mut self, tan: fn(f64) -> f64) {
        self.tan = tan;
    }

    /// The seed the generator was last seeded with, as given.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The mode for FS-100 and FS-101.
    #[must_use]
    pub fn mode(&self) -> UpstreamDefects {
        self.notices.mode()
    }

    /// The notices of the defects that changed (or, reproduced, would have changed) a result
    /// of this generator so far, one per defect.
    #[must_use]
    pub fn notices(&self) -> &[Notice] {
        self.notices.as_slice()
    }

    /// Takes the notices collected so far.
    pub fn take_notices(&mut self) -> Vec<Notice> {
        let mode = self.notices.mode();
        std::mem::replace(&mut self.notices, Notices::new(mode)).into_vec()
    }

    /// Computes the next group: the last 24 values of the previous group (or the seeded history)
    /// become the buffer's first 24, then 389 steps of the recurrence (§2.3).
    #[inline(never)]
    fn refill(&mut self) {
        let buf = &mut self.buf;
        buf.copy_within(GROUP..BUF, 0);
        let mut c = self.carry;
        for n in LONG_LAG..BUF {
            let d = buf[n - SHORT_LAG] - buf[n - LONG_LAG] - c;
            // d in [-2^24, 2^24): its sign is the borrow, its low 24 bits the value.
            c = ((d as u32) >> 31) as i32;
            buf[n] = d & (RANGE - 1);
        }
        self.carry = c;
        self.next = LONG_LAG;
    }

    /// The next integer of the stream, in [0, 2²⁴) (FreeSurfer's `sc_rng_get`).
    #[inline]
    pub fn next_u24(&mut self) -> u32 {
        if self.next == LONG_LAG + DELIVERED {
            self.refill();
        }
        let v = self.buf[self.next];
        self.next += 1;
        v as u32
    }

    /// Fills `out` with the next integers of the stream (as many calls of [`Ranlux::next_u24`]).
    pub fn fill_u24(&mut self, out: &mut [u32]) {
        let mut i = 0;
        while i < out.len() {
            if self.next == LONG_LAG + DELIVERED {
                self.refill();
            }
            let take = (LONG_LAG + DELIVERED - self.next).min(out.len() - i);
            for (o, &v) in out[i..i + take]
                .iter_mut()
                .zip(&self.buf[self.next..self.next + take])
            {
                *o = v as u32;
            }
            self.next += take;
            i += take;
        }
    }

    /// A uniform deviate in [0, 1): one integer times 2⁻²⁴, exact (§3, `sc_uni`).
    #[inline]
    pub fn uniform(&mut self) -> f64 {
        f64::from(self.next_u24()) * (1.0 / 16_777_216.0)
    }

    /// A uniform deviate in (0, 1): uniforms are drawn until one is not 0 (§3, `sc_uni_pos`).
    ///
    /// # Errors
    /// [`DrawError::NeverReturns`] when the stream is all zero (FS-101 under `reproduce`).
    #[inline]
    pub fn uniform_pos(&mut self) -> Result<f64, DrawError> {
        if self.zero_forever {
            return Err(self.never("positive uniform draw"));
        }
        loop {
            let u = self.uniform();
            if u != 0.0 {
                return Ok(u);
            }
        }
    }

    /// FS-101's error for `draw`.
    fn never(&self, draw: &'static str) -> DrawError {
        DrawError::NeverReturns {
            seed: self.seed,
            draw,
        }
    }
}
