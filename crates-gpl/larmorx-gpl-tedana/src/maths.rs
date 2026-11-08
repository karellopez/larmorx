// SPDX-License-Identifier: GPL-3.0-only
// Translated from tedana 26.0.3 (Copyright 2023, The ME-ICA Developers; GNU LGPL 2.1), under the
// GNU GPL version 3 as section 3 of the LGPL 2.1 allows; modified 2026 by Karel Lopez Vilaret.
//! The transcendental functions t2smap's arithmetic takes, in two families:
//!
//! - [`Family::Logf`]: the float32 logarithm of the log-linear fit (`np.log(np.abs(data) + 1)`
//!   of float32 data): numpy's own kernel (`larmorx_core::numpy::umath::log_f32`; NP-03: not
//!   correctly rounded) or the correctly rounded `logf`;
//! - [`Family::ExpLog`]: numpy's float64 `exp` and `log` (the C library's, glibc 2.35's, on a CPU
//!   without AVX-512; `umath::exp_f64`, `log_f64`) of S0, the curve fit's model, the T2* floor,
//!   the RMSE's predictions, the combination's weights and nilearn's smoothing kernel, and
//!   SciPy's powers of the curve fit (C's `pow`) (GLIBC-07), or the correctly rounded ones.
//!
//! By default both are correctly rounded (rule 5), with `reproduce` tedana's (numpy's). A
//! [`Maths::of_mode`] keeps a witness per family, as `larmorx_core::math::Libm` does: whether the
//! other choice gave another value on an argument evaluated (deterministic whatever the threads:
//! the OR over every argument; once a family has differed, each call evaluates one function).
//! The program then runs the stages that depend on them again with the other choice of that
//! family only ([`Maths::with_other`], no witness), compares every value it writes and every
//! line it prints, and names NP-03 or GLIBC-07 with the count of values that differ: nothing
//! when none does (the workflow's `t2smap`).

use std::sync::atomic::{AtomicBool, Ordering};

use larmorx_core::defects::UpstreamDefects;
use larmorx_core::math::{self, Libm};
use larmorx_core::numpy::umath;

/// A family of functions whose two choices the program compares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// The float32 logarithm (NP-03).
    Logf,
    /// The float64 exponential and logarithm, and the curve fit's powers (GLIBC-07).
    ExpLog,
}

/// The functions of one pass (see the module's documentation).
#[derive(Debug)]
pub struct Maths {
    /// numpy's float32 log kernel (else the correctly rounded `logf`).
    numpy_logf: bool,
    /// numpy's (glibc's) float64 `exp`, `log` and C's `pow` (else the correctly rounded ones).
    numpy_explog: bool,
    watch: bool,
    logf_differed: AtomicBool,
    explog_differed: AtomicBool,
}

impl Maths {
    /// The functions of a mode (correctly rounded for `fix`, numpy's for `reproduce`), with the
    /// witness.
    pub fn of_mode(mode: UpstreamDefects) -> Maths {
        Maths {
            numpy_logf: mode.reproduce(),
            numpy_explog: mode.reproduce(),
            watch: true,
            logf_differed: AtomicBool::new(false),
            explog_differed: AtomicBool::new(false),
        }
    }

    /// These functions with the other choice for `family`, without the witness.
    pub fn with_other(&self, family: Family) -> Maths {
        self.with_others(&[family])
    }

    /// These functions with the other choice for each of `families`, without the witness.
    pub fn with_others(&self, families: &[Family]) -> Maths {
        let (mut l, mut e) = (self.numpy_logf, self.numpy_explog);
        for family in families {
            match family {
                Family::Logf => l = !l,
                Family::ExpLog => e = !e,
            }
        }
        Maths {
            numpy_logf: l,
            numpy_explog: e,
            watch: false,
            logf_differed: AtomicBool::new(false),
            explog_differed: AtomicBool::new(false),
        }
    }

    fn flag(&self, family: Family) -> &AtomicBool {
        match family {
            Family::Logf => &self.logf_differed,
            Family::ExpLog => &self.explog_differed,
        }
    }

    /// Whether the other choice of `family` gave another value on an argument evaluated.
    pub fn differed(&self, family: Family) -> bool {
        self.flag(family).load(Ordering::Relaxed)
    }

    /// Records that the other choice of `family` gives another result.
    pub fn note(&self, family: Family, differs: bool) {
        if differs && self.watch {
            self.flag(family).store(true, Ordering::Relaxed);
        }
    }

    /// Whether the witness of `family` still looks.
    pub fn watching(&self, family: Family) -> bool {
        self.watch && !self.differed(family)
    }

    /// Whether the float64 functions are numpy's (glibc's).
    pub fn numpy_explog(&self) -> bool {
        self.numpy_explog
    }

    /// `np.log` of a float32.
    #[inline]
    pub fn log32(&self, x: f32) -> f32 {
        let pick = |numpy: bool| {
            if numpy {
                umath::log_f32(x)
            } else {
                math::logf(x)
            }
        };
        let v = pick(self.numpy_logf);
        if self.watching(Family::Logf) {
            self.note(
                Family::Logf,
                pick(!self.numpy_logf).to_bits() != v.to_bits(),
            );
        }
        v
    }

    /// `np.exp` of a float64.
    #[inline]
    pub fn exp(&self, x: f64) -> f64 {
        let pick = |numpy: bool| {
            if numpy {
                umath::exp_f64(x)
            } else {
                math::exp(x)
            }
        };
        let v = pick(self.numpy_explog);
        if self.watching(Family::ExpLog) {
            self.note(
                Family::ExpLog,
                pick(!self.numpy_explog).to_bits() != v.to_bits(),
            );
        }
        v
    }

    /// `np.log` of a float64.
    #[inline]
    pub fn log(&self, x: f64) -> f64 {
        let pick = |numpy: bool| {
            if numpy {
                umath::log_f64(x)
            } else {
                math::log(x)
            }
        };
        let v = pick(self.numpy_explog);
        if self.watching(Family::ExpLog) {
            self.note(
                Family::ExpLog,
                pick(!self.numpy_explog).to_bits() != v.to_bits(),
            );
        }
        v
    }

    /// The C library functions of SciPy's curve fit (its powers): glibc's when the float64
    /// functions are numpy's, with a witness while this family's looks; [`Maths::note_libm`]
    /// takes its finding back.
    pub fn libm(&self) -> Libm {
        let mode = if self.numpy_explog {
            UpstreamDefects::Reproduce
        } else {
            UpstreamDefects::Fix
        };
        if self.watching(Family::ExpLog) {
            Libm::new(mode)
        } else {
            Libm::quiet(mode)
        }
    }

    /// Adds what a [`Maths::libm`]'s witness saw to [`Family::ExpLog`]'s.
    pub fn note_libm(&self, libm: &Libm) {
        self.note(Family::ExpLog, libm.differed());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn witness_and_other() {
        let m = Maths::of_mode(UpstreamDefects::Fix);
        assert!(!m.numpy_explog());
        // Exact arguments: both choices agree.
        assert_eq!(m.exp(0.0), 1.0);
        assert_eq!(m.log(1.0), 0.0);
        assert_eq!(m.log32(1.0), 0.0);
        assert!(!m.differed(Family::Logf) && !m.differed(Family::ExpLog));
        // Find a float32 log argument where numpy's kernel and the correctly rounded differ.
        let x = (1..1_000_000u32)
            .map(|k| 1.0f32 + k as f32 * 1e-4)
            .find(|&x| umath::log_f32(x).to_bits() != math::logf(x).to_bits())
            .expect("an argument where numpy's logf is not correctly rounded");
        assert_eq!(m.log32(x).to_bits(), math::logf(x).to_bits());
        assert!(m.differed(Family::Logf) && !m.differed(Family::ExpLog));
        let o = m.with_other(Family::Logf);
        assert_eq!(o.log32(x).to_bits(), umath::log_f32(x).to_bits());
        assert!(!o.differed(Family::Logf), "no witness in the other pass");
        assert!(!o.numpy_explog());
        let r = Maths::of_mode(UpstreamDefects::Reproduce);
        assert_eq!(r.log32(x).to_bits(), umath::log_f32(x).to_bits());
        assert!(r.differed(Family::Logf));
        assert!(!r.with_other(Family::ExpLog).numpy_explog());
    }
}
