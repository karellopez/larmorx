// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! The C library's `pow` as FSL's programs call it (BET: the initial radius, `pow(x, 1/3)`;
//! the threshold's exponent, `pow(f, 0.275)`).
//!
//! Provisional: the correctly rounded `pow` of `larmorx_core::math` (CORE-MATH). glibc 2.35's
//! `pow` (ARM's algorithm, in its FMA variant on CPUs with FMA) is not correctly rounded in
//! about 0.09 % of these calls (measured on 20 million arguments); the replica's exact port of
//! it replaces this function.

/// `pow(x, y)`.
pub fn pow(x: f64, y: f64) -> f64 {
    larmorx_core::math::pow(x, y)
}
