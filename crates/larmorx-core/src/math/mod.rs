//! Correctly rounded transcendental functions (CLAUDE.md rule 5, decided 2026-10-09).
//!
//! Each function returns the exact result rounded to the nearest `f64` (ties to even). So
//! results are the same on every platform, and agree with glibc, and so with ITK, ANTs and
//! AFNI on Linux, except where glibc itself is not correctly rounded (about 0.1 % of
//! calls). They are pure-Rust ports of CORE-MATH (MIT, commit `040ee482a8ca`, see
//! `UPSTREAM.md`), verified against mpmath on CORE-MATH's worst-case inputs and millions of
//! random inputs. Background: `docs/findings/platform-math.md`.

mod exp;

pub use exp::exp;
