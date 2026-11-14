// SPDX-License-Identifier: Apache-2.0
//! Which last bits an original program's C library functions give: the correctly rounded
//! functions of this module (larmorx's default, rule 5) or glibc 2.35's (`reproduce`, GLIBC-07;
//! the user's decision of 2026-11-04: glibc's exact bits for `reproduce` only).
//!
//! A clean-room original (FSL's, `crates/larmorx-fsl`; the head-motion correction,
//! `crates/larmorx-mri`) calls its C library functions through a [`Libm`] made from its call's
//! [`UpstreamDefects`] mode: by default the correctly rounded value, with `reproduce` the
//! `*_glibc` function where the tool's spec names glibc's. So the default is untouched, and
//! `reproduce` gives the original program's bits wherever its result depends on glibc's last
//! bit.
//!
//! **The witness.** A [`Libm::new`] also records whether the other choice would have given
//! another value (bit for bit) on an argument it evaluated ([`Libm::differed`]). GLIBC-07 is
//! named only where a tool can tell exactly that its output changed (the user's decision of
//! 2026-11-04, "name exact cases only"): a pointwise computation (fslmaths' voxelwise
//! functions, fslstats' entropies, avscale, makerot, `convert_xfm -fixscaleskew`, 3dTshift's
//! weighted sinc) compares the values it stores itself, evaluating both with
//! [`Libm::is_glibc`] and calling [`Libm::note`], and names GLIBC-07 in both modes where they
//! differ. It looks until the first difference only, so after that each call costs one
//! function; it is deterministic whatever the threads (the OR over every argument evaluated).
//!
//! **Quiet.** An iterative fit (FLIRT, FAST, mcflirt's mutual information, topup, bet2,
//! betsurf) and a filter that combines many function values (fslmaths' kernels, `-bptf`,
//! `-tfce`, `-roc`) cannot tell whether a last bit moved its outputs without running twice,
//! so it names nothing and takes a [`Libm::quiet`]: the same functions, without the witness
//! (one function per call). Their API pages say that `reproduce` gives glibc's (the
//! original's) maths and that the default's correctly rounded functions can move a result's
//! last bits.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::defects::UpstreamDefects;

/// The C library functions of one call of an original program, with the last bits its mode
/// asks for (see the module's documentation).
#[derive(Debug)]
pub struct Libm {
    glibc: bool,
    /// The witness looks ([`Libm::new`]); not for a [`Libm::quiet`].
    watch: bool,
    differed: AtomicBool,
}

impl Default for Libm {
    /// [`Libm::quiet`] of [`UpstreamDefects::Fix`]: correctly rounded, without the witness.
    fn default() -> Libm {
        Libm::quiet(UpstreamDefects::Fix)
    }
}

impl Libm {
    /// Correctly rounded for [`UpstreamDefects::Fix`], glibc 2.35's for
    /// [`UpstreamDefects::Reproduce`].
    /// The witness looks: [`Libm::differed`] tells whether the other choice gave another
    /// value on an argument evaluated.
    pub fn new(mode: UpstreamDefects) -> Libm {
        Libm {
            glibc: mode.reproduce(),
            watch: true,
            differed: AtomicBool::new(false),
        }
    }

    /// The same functions as [`Libm::new`] of `mode`, without the witness: for a program that
    /// names nothing (an iterative fit, a filter; see the module's documentation), each call
    /// evaluates one function and [`Libm::differed`] stays `false`.
    pub fn quiet(mode: UpstreamDefects) -> Libm {
        Libm {
            glibc: mode.reproduce(),
            watch: false,
            differed: AtomicBool::new(false),
        }
    }

    /// Whether the functions are glibc's (`reproduce`).
    #[inline]
    pub fn is_glibc(&self) -> bool {
        self.glibc
    }

    /// The mode this [`Libm`] was made for.
    pub fn mode(&self) -> UpstreamDefects {
        if self.glibc {
            UpstreamDefects::Reproduce
        } else {
            UpstreamDefects::Fix
        }
    }

    /// Whether the other choice gave another value on an argument evaluated so far (always
    /// `false` for a [`Libm::quiet`]).
    #[inline]
    pub fn differed(&self) -> bool {
        self.differed.load(Ordering::Relaxed)
    }

    /// Records that the other choice gives another result (`differs`), for sites that compare
    /// what they store themselves.
    #[inline]
    pub fn note(&self, differs: bool) {
        if differs && self.watch {
            self.differed.store(true, Ordering::Relaxed);
        }
    }

    /// Whether the witness still looks (no difference seen yet; never for a [`Libm::quiet`]):
    /// a site that compares itself may skip the other evaluation once this is false.
    #[inline]
    pub fn watching(&self) -> bool {
        self.watch && !self.differed()
    }

    /// `cr()` by default, `glibc()` with `reproduce`; the other evaluated too while the witness
    /// looks (for a site whose correctly rounded form is its own expression).
    #[inline]
    pub fn choose64(&self, cr: impl Fn() -> f64, glibc: impl Fn() -> f64) -> f64 {
        let v = if self.glibc { glibc() } else { cr() };
        if self.watching() {
            let o = if self.glibc { cr() } else { glibc() };
            self.note(o.to_bits() != v.to_bits());
        }
        v
    }

    /// [`Libm::choose64`] for `f32`.
    #[inline]
    pub fn choose32(&self, cr: impl Fn() -> f32, glibc: impl Fn() -> f32) -> f32 {
        let v = if self.glibc { glibc() } else { cr() };
        if self.watching() {
            let o = if self.glibc { cr() } else { glibc() };
            self.note(o.to_bits() != v.to_bits());
        }
        v
    }

    /// `e^x` ([`super::exp`] or [`super::exp_glibc`]).
    #[inline]
    pub fn exp(&self, x: f64) -> f64 {
        self.choose64(|| super::exp(x), || super::exp_glibc(x))
    }

    /// The natural logarithm ([`super::log`] or [`super::log_glibc`]).
    #[inline]
    pub fn log(&self, x: f64) -> f64 {
        self.choose64(|| super::log(x), || super::log_glibc(x))
    }

    /// The base-10 logarithm ([`super::log10`] or [`super::log10_glibc`]).
    #[inline]
    pub fn log10(&self, x: f64) -> f64 {
        self.choose64(|| super::log10(x), || super::log10_glibc(x))
    }

    /// `x^y` ([`super::pow`] or [`super::pow_glibc`]).
    #[inline]
    pub fn pow(&self, x: f64, y: f64) -> f64 {
        self.choose64(|| super::pow(x, y), || super::pow_glibc(x, y))
    }

    /// The sine ([`super::sin`] or [`super::sin_glibc`]).
    #[inline]
    pub fn sin(&self, x: f64) -> f64 {
        self.choose64(|| super::sin(x), || super::sin_glibc(x))
    }

    /// The cosine ([`super::cos`] or [`super::cos_glibc`]).
    #[inline]
    pub fn cos(&self, x: f64) -> f64 {
        self.choose64(|| super::cos(x), || super::cos_glibc(x))
    }

    /// `e^x` for `f32` ([`super::expf`] or [`super::expf_glibc`]).
    #[inline]
    pub fn expf(&self, x: f32) -> f32 {
        self.choose32(|| super::expf(x), || super::expf_glibc(x))
    }

    /// The natural logarithm for `f32` ([`super::logf`] or [`super::logf_glibc`]).
    #[inline]
    pub fn logf(&self, x: f32) -> f32 {
        self.choose32(|| super::logf(x), || super::logf_glibc(x))
    }

    /// `x^y` for `f32` ([`super::powf`] or [`super::powf_glibc`]).
    #[inline]
    pub fn powf(&self, x: f32, y: f32) -> f32 {
        self.choose32(|| super::powf(x, y), || super::powf_glibc(x, y))
    }

    /// The sine for `f32` ([`super::sinf`] or [`super::sinf_glibc`]).
    #[inline]
    pub fn sinf(&self, x: f32) -> f32 {
        self.choose32(|| super::sinf(x), || super::sinf_glibc(x))
    }

    /// The cosine for `f32` ([`super::cosf`] or [`super::cosf_glibc`]).
    #[inline]
    pub fn cosf(&self, x: f32) -> f32 {
        self.choose32(|| super::cosf(x), || super::cosf_glibc(x))
    }

    /// The arc cosine for `f32` ([`super::acosf`] or [`super::acosf_glibc`]).
    #[inline]
    pub fn acosf(&self, x: f32) -> f32 {
        self.choose32(|| super::acosf(x), || super::acosf_glibc(x))
    }

    /// `atan2(y, x)` for `f32`: by default the correctly rounded double [`super::atan2`] of the
    /// two rounded to `f32`; with `reproduce` [`super::atan2f_glibc`] (fdlibm's).
    #[inline]
    pub fn atan2f(&self, y: f32, x: f32) -> f32 {
        self.choose32(
            || super::atan2(f64::from(y), f64::from(x)) as f32,
            || super::atan2f_glibc(y, x),
        )
    }

    /// [`Libm::exp`] of every value of `x`, in place: by default [`super::exp_in_place`] (the
    /// same bits as [`super::exp`], faster), with `reproduce` [`super::exp_glibc`] of each.
    pub fn exp_in_place(&self, x: &mut [f64]) {
        // the witness looks at the arguments in order until the first difference
        if self.watching() {
            for &a in x.iter() {
                if super::exp(a).to_bits() != super::exp_glibc(a).to_bits() {
                    self.note(true);
                    break;
                }
            }
        }
        if self.glibc {
            for v in x.iter_mut() {
                *v = super::exp_glibc(*v);
            }
        } else {
            super::exp_in_place(x);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_correctly_rounded_and_reproduce_glibc() {
        let (fix, rep) = (
            Libm::new(UpstreamDefects::Fix),
            Libm::new(UpstreamDefects::Reproduce),
        );
        assert!(!fix.is_glibc() && rep.is_glibc());
        assert_eq!(fix.mode(), UpstreamDefects::Fix);
        assert_eq!(rep.mode(), UpstreamDefects::Reproduce);
        // seeded arguments: every result the chosen function's, and the witness set exactly
        // when the two differ on one of them
        let mut s = 0x243f_6a88_85a3_08d3u64;
        let mut any = [false; 13];
        for _ in 0..20_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let u = (s >> 11) as f64 / (1u64 << 53) as f64;
            let x = u * 40.0 - 20.0;
            let f = x as f32;
            let p = (u * 3.0) as f32;
            let pairs64: [(f64, f64, f64, f64); 6] = [
                (
                    fix.exp(x),
                    rep.exp(x),
                    super::super::exp(x),
                    super::super::exp_glibc(x),
                ),
                (
                    fix.log(u),
                    rep.log(u),
                    super::super::log(u),
                    super::super::log_glibc(u),
                ),
                (
                    fix.log10(u),
                    rep.log10(u),
                    super::super::log10(u),
                    super::super::log10_glibc(u),
                ),
                (
                    fix.pow(u, x),
                    rep.pow(u, x),
                    super::super::pow(u, x),
                    super::super::pow_glibc(u, x),
                ),
                (
                    fix.sin(x),
                    rep.sin(x),
                    super::super::sin(x),
                    super::super::sin_glibc(x),
                ),
                (
                    fix.cos(x),
                    rep.cos(x),
                    super::super::cos(x),
                    super::super::cos_glibc(x),
                ),
            ];
            for (k, (a, b, c, g)) in pairs64.into_iter().enumerate() {
                assert_eq!((a.to_bits(), b.to_bits()), (c.to_bits(), g.to_bits()));
                any[k] |= c.to_bits() != g.to_bits();
            }
            let pairs32: [(f32, f32, f32, f32); 7] = [
                (
                    fix.expf(f),
                    rep.expf(f),
                    super::super::expf(f),
                    super::super::expf_glibc(f),
                ),
                (
                    fix.logf(p),
                    rep.logf(p),
                    super::super::logf(p),
                    super::super::logf_glibc(p),
                ),
                (
                    fix.powf(p, f / 4.0),
                    rep.powf(p, f / 4.0),
                    super::super::powf(p, f / 4.0),
                    super::super::powf_glibc(p, f / 4.0),
                ),
                (
                    fix.sinf(f),
                    rep.sinf(f),
                    super::super::sinf(f),
                    super::super::sinf_glibc(f),
                ),
                (
                    fix.cosf(f),
                    rep.cosf(f),
                    super::super::cosf(f),
                    super::super::cosf_glibc(f),
                ),
                (
                    fix.acosf(p - 1.5),
                    rep.acosf(p - 1.5),
                    super::super::acosf(p - 1.5),
                    super::super::acosf_glibc(p - 1.5),
                ),
                (
                    fix.atan2f(f, p),
                    rep.atan2f(f, p),
                    super::super::atan2(f64::from(f), f64::from(p)) as f32,
                    super::super::atan2f_glibc(f, p),
                ),
            ];
            for (k, (a, b, c, g)) in pairs32.into_iter().enumerate() {
                assert_eq!((a.to_bits(), b.to_bits()), (c.to_bits(), g.to_bits()));
                any[6 + k] |= c.to_bits() != g.to_bits();
            }
        }
        // glibc's differ somewhere on 20,000 arguments for each, so both witnesses saw it
        assert!(any.iter().filter(|&&a| a).count() >= 10, "{any:?}");
        assert!(fix.differed() && rep.differed());
    }

    #[test]
    fn the_witness_stays_unset_where_the_two_agree() {
        let l = Libm::new(UpstreamDefects::Fix);
        for k in 0..100 {
            // exact cases: e^0, log 1, small integer powers, sin 0
            let _ = (
                l.exp(0.0),
                l.log(1.0),
                l.log10(1000.0),
                l.pow(2.0, f64::from(k % 10)),
                l.sin(0.0),
            );
        }
        assert!(!l.differed());
        l.note(false);
        assert!(!l.differed());
        l.note(true);
        assert!(l.differed() && !l.watching());
    }

    #[test]
    fn a_quiet_libm_gives_the_same_bits_without_the_witness() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (w, q) = (Libm::new(mode), Libm::quiet(mode));
            assert_eq!(q.is_glibc(), mode.reproduce());
            for i in 0..5000 {
                let x = (f64::from(i) - 2500.0) * 0.0137;
                assert_eq!(w.exp(x).to_bits(), q.exp(x).to_bits());
                assert_eq!(w.sin(x).to_bits(), q.sin(x).to_bits());
                let f = x as f32;
                assert_eq!(w.cosf(f).to_bits(), q.cosf(f).to_bits());
            }
            let mut v: Vec<f64> = (0..5000).map(|i| f64::from(i) * 0.011 - 27.0).collect();
            q.exp_in_place(&mut v);
            q.note(true);
            assert!(w.differed(), "{mode}");
            assert!(!q.differed() && !q.watching(), "{mode}");
        }
        assert!(!Libm::default().watching() && !Libm::default().is_glibc());
    }

    #[test]
    fn exp_in_place_gives_each_functions_bits() {
        let xs: Vec<f64> = (0..5000).map(|i| (i as f64 - 2500.0) * 0.0137).collect();
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let l = Libm::new(mode);
            let mut v = xs.clone();
            l.exp_in_place(&mut v);
            for (a, &x) in v.iter().zip(&xs) {
                let want = if mode.fix() {
                    super::super::exp(x)
                } else {
                    super::super::exp_glibc(x)
                };
                assert_eq!(a.to_bits(), want.to_bits());
            }
            let differs = xs
                .iter()
                .any(|&x| super::super::exp(x).to_bits() != super::super::exp_glibc(x).to_bits());
            assert_eq!(l.differed(), differs);
        }
    }
}
