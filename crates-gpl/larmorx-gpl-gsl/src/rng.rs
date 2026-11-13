// SPDX-License-Identifier: GPL-3.0-or-later
// Translated from GSL 1.8's rng/ranlux.c (Copyright (C) 1996, 1997, 1998, 1999, 2000 James
// Theiler, Brian Gough; GNU GPL version 2 or later) as FreeSurfer 8.2.0's utils/numerics.cpp
// transcribes it (Silvester Czanner, 2006; that file carries The General Hospital Corporation's
// notice); translated to Rust in 2026 by Karel Lopez Vilaret, under the GNU GPL version 3 or later.
//! RANLUX389 as FreeSurfer's `numerics.cpp` has it (`sc_status_t`, `sc_inc_status`, `sc_rng_get`,
//! `sc_rng_set`, `sc_rng_alloc`), with its uniform deviates (`sc_uni`, `sc_uni_pos`): Lüscher's
//! subtract-with-borrow generator with lags 24 and 10, of whose every 389 values the first 24 are
//! delivered, seeded by James's method. Statement by statement, with C's integer conversions.
//!
//! D13 (`specs/ranlux.md` §8.2, FS-101): a seed whose 24 seeded values are all 0 leaves the
//! generator at a fixed point (0 for ever). By default the replica starts that one state with a
//! carry of 1 (James's 1994 rule), with a notice; under `reproduce` it keeps FreeSurfer's zero
//! stream, and the draws that would never return report [`DrawError::NeverReturns`].

use std::fmt;

use larmorx_core::UpstreamDefects;
use larmorx_core::defects::{FS_101, Notice, Notices};

/// `SC_MASK_LO` (2²⁴ − 1).
const SC_MASK_LO: u64 = 0x00ff_ffff;
/// `SC_MASK_HI` (`~0x00ffffffUL`).
const SC_MASK_HI: u64 = !0x00ff_ffff;
/// `two_over_24` (2²⁴).
const TWO_OVER_24: u64 = 16_777_216;
/// The luxury level FreeSurfer's `sc_rng_set` uses (`const_lux`).
const CONST_LUX: u32 = 389;

/// `sc_status_t`: the generator's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScStatus {
    /// `i`: the index of the value the next step replaces (lag 24).
    pub i: u32,
    /// `j`: the index of the lag-10 value.
    pub j: u32,
    /// `n`: the values delivered in the current group of 24.
    pub n: u32,
    /// `skip`: the values discarded after each group (`const_lux − 24` = 365).
    pub skip: u32,
    /// `carry`: the borrow of the last step.
    pub carry: u32,
    /// `u[24]` (`unsigned long`): the last 24 values.
    pub u: [u64; 24],
}

/// Why a draw gives no number (the texts of larmorx's clean room, `larmorx_core::ranlux`, so the
/// two implementations print the same lines).
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
    pub(crate) fn in_draw(self, draw: &'static str) -> DrawError {
        match self {
            DrawError::NeverReturns { seed, .. } => DrawError::NeverReturns { seed, draw },
            other => other,
        }
    }
}

/// A shape as the notices and errors write it: Rust's shortest form, in exponent form from 10¹⁶.
pub(crate) fn shape_text(a: f64) -> String {
    if a.is_finite() && a.abs() >= 1e16 {
        format!("{a:e}")
    } else {
        format!("{a}")
    }
}

impl fmt::Display for DrawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DrawError::ShapeNotFinite(a) => write!(
                f,
                "FS-100: the gamma shape {} is not a finite number",
                shape_text(*a)
            ),
            DrawError::ShapeNegative(a) => {
                write!(f, "FS-100: the gamma shape {} is negative", shape_text(*a))
            }
            DrawError::NeverReturns { seed, draw } => {
                write!(
                    f,
                    "FS-101: with seed {seed} FreeSurfer's {draw} never returns"
                )
            }
        }
    }
}

impl std::error::Error for DrawError {}

/// `sc_rng`: a generator (FreeSurfer's one type, `intern_rng_type`, RANLUX389) with D13's mode
/// and its notices. The draws are methods (`randist.rs`).
#[derive(Clone, Debug)]
pub struct ScRng {
    status: ScStatus,
    /// The seed of the last `sc_rng_set`, as given.
    seed: u64,
    /// FS-101 under `reproduce`: the seeded state is all zero, so every value is 0 for ever.
    pub(crate) zero_forever: bool,
    pub(crate) notices: Notices,
    /// The tangent of `sc_gamma_large`: larmorx's correctly rounded one (the user's decision of
    /// 2026-11-04, `specs/ranlux.md` §6), replaced only by validation ([`ScRng::set_tan`]).
    pub(crate) tan: fn(f64) -> f64,
}

/// `sc_inc_status(status)`: one step of the recurrence; returns the new value.
pub fn sc_inc_status(status: &mut ScStatus) -> u64 {
    let mut i = status.i;
    let mut j = status.j;
    // `step = status->u[j] - status->u[i] - status->carry;` in unsigned long, stored in a long
    let mut step = status.u[j as usize]
        .wrapping_sub(status.u[i as usize])
        .wrapping_sub(u64::from(status.carry)) as i64;

    if (step as u64) & SC_MASK_HI != 0 {
        status.carry = 1;
        step = ((step as u64) & SC_MASK_LO) as i64;
    } else {
        status.carry = 0;
    }
    status.u[i as usize] = step as u64;

    if i == 0 {
        i = 23;
    } else {
        i -= 1;
    }
    status.i = i;

    if j == 0 {
        j = 23;
    } else {
        j -= 1;
    }
    status.j = j;

    step as u64
}

impl ScRng {
    /// `sc_rng_alloc(&intern_rng_type)`: a generator set to seed 0 (FreeSurfer's default seed,
    /// 314159265). `mode` decides FS-100 and FS-101.
    #[must_use]
    pub fn alloc(mode: UpstreamDefects) -> ScRng {
        let mut r = ScRng {
            status: ScStatus {
                i: 0,
                j: 0,
                n: 0,
                skip: 0,
                carry: 0,
                u: [0; 24],
            },
            seed: 0,
            zero_forever: false,
            notices: Notices::new(mode),
            tan: larmorx_core::math::tan,
        };
        r.set(0);
        r
    }

    /// `sc_rng_alloc` then `sc_rng_set(r, seed)`: what every FreeSurfer caller does.
    #[must_use]
    pub fn new(seed: u64, mode: UpstreamDefects) -> ScRng {
        let mut r = ScRng::alloc(mode);
        r.set(seed);
        r
    }

    /// `sc_rng_set(r, seed_in)` (`unsigned long`; FreeSurfer's callers convert their own seeds as
    /// C does: `-1` is `u64::MAX`). FS-101 (above) decides the all-zero state.
    pub fn set(&mut self, seed_in: u64) {
        self.seed = seed_in;
        let const_lux = CONST_LUX;
        let status = &mut self.status;

        // `long int seed = seed_in` (two's complement)
        let mut seed: i64 = if seed_in == 0 {
            314_159_265
        } else {
            seed_in as i64
        };

        for i in 0..24 {
            // `tmp` is `unsigned long`: `tmp = seed / 53668` (a signed division, converted), then
            // `seed = 40014 * (seed - tmp * 53668) - tmp * 12211` in unsigned long, stored in a long
            let tmp = (seed / 53_668) as u64;
            seed = 40_014u64
                .wrapping_mul((seed as u64).wrapping_sub(tmp.wrapping_mul(53_668)))
                .wrapping_sub(tmp.wrapping_mul(12_211)) as i64;
            if seed < 0 {
                seed += 2_147_483_563;
            }
            // `seed % two_over_24`: the long converted to unsigned long
            status.u[i] = (seed as u64) % TWO_OVER_24;
        }

        status.i = 23;
        status.j = 9;
        status.n = 0;
        status.skip = const_lux - 24;

        if status.u[23] & SC_MASK_HI != 0 {
            status.carry = 1;
        } else {
            status.carry = 0;
        }

        // FS-101 (larmorx): the all-zero state with carry 0 is a fixed point of the recurrence.
        self.zero_forever = false;
        if status.carry == 0 && status.u.iter().all(|&v| v == 0) {
            if self.notices.fix() {
                status.carry = 1;
                self.notices.add(
                    &FS_101,
                    format!(
                        "seed {seed_in} makes FreeSurfer's random generator give 0 for ever; \
                         larmorx starts it with a carry of 1 (James's 1994 initialisation)"
                    ),
                );
            } else {
                self.zero_forever = true;
                self.notices.add(
                    &FS_101,
                    format!("seed {seed_in} makes FreeSurfer's random generator give 0 for ever"),
                );
            }
        }
    }

    /// `sc_rng_get(r)`: the next integer of the stream, in [0, 2²⁴).
    pub fn get(&mut self) -> u64 {
        let status = &mut self.status;
        let skip = status.skip;
        let ret = sc_inc_status(status);

        status.n += 1;
        if status.n == 24 {
            status.n = 0;
            for _ in 0..skip {
                sc_inc_status(status);
            }
        }

        ret
    }

    /// `sc_uni(r)`: `sc_rng_get(r) / 16777216.0`, in [0, 1).
    #[inline]
    pub fn uni(&mut self) -> f64 {
        self.get() as f64 / 16_777_216.0
    }

    /// `sc_uni_pos(r)`: `sc_uni` until it is not 0, in (0, 1).
    ///
    /// # Errors
    /// [`DrawError::NeverReturns`] when the stream is all zero (FS-101 under `reproduce`), where
    /// FreeSurfer's loop never ends.
    pub fn uni_pos(&mut self) -> Result<f64, DrawError> {
        if self.zero_forever {
            return Err(self.never("positive uniform draw"));
        }
        let mut x;
        loop {
            x = self.get() as f64 / 16_777_216.0;
            if x != 0.0 {
                break;
            }
        }
        Ok(x)
    }

    /// FS-101's error for `draw`.
    pub(crate) fn never(&self, draw: &'static str) -> DrawError {
        DrawError::NeverReturns {
            seed: self.seed,
            draw,
        }
    }

    /// The seed of the last [`ScRng::set`], as given.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The mode for FS-100 and FS-101.
    #[must_use]
    pub fn mode(&self) -> UpstreamDefects {
        self.notices.mode()
    }

    /// The notices of the defects that changed (or, reproduced, would have changed) a result of
    /// this generator so far, one per defect.
    #[must_use]
    pub fn notices(&self) -> &[Notice] {
        self.notices.as_slice()
    }

    /// Takes the notices collected so far.
    pub fn take_notices(&mut self) -> Vec<Notice> {
        let mode = self.notices.mode();
        std::mem::replace(&mut self.notices, Notices::new(mode)).into_vec()
    }

    /// The state (`r->status`).
    #[must_use]
    pub fn status(&self) -> &ScStatus {
        &self.status
    }

    /// The state, to be written as the probe `fs_rng_probe crafted` writes FreeSurfer's (a state
    /// no seed reaches in practice). For validation only.
    #[doc(hidden)]
    pub fn status_mut(&mut self) -> &mut ScStatus {
        &mut self.status
    }

    /// Replaces the tangent `sc_gamma_large` takes by `tan`. For validation only: with the C
    /// library's `tan` every bit of every draw is FreeSurfer's, which shows that the correctly
    /// rounded tangent is the only cause of a difference.
    #[doc(hidden)]
    pub fn set_tan(&mut self, tan: fn(f64) -> f64) {
        self.tan = tan;
    }
}
