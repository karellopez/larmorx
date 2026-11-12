// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's random draws of `utils/pdf.cpp` that its programs reach (`fs.maths`, queue item
//! 3.1-24), translated from FreeSurfer 8.2.0: `PDFtodSeed` (a seed from the time of day) and
//! `PDFgaussian` (a standard normal number by Marsaglia's polar method on the C library's
//! `drand48`, [`Globals::drand48`]).
//!
//! `log` is larmorx's correctly rounded one by default and glibc 2.35's with `reproduce`
//! (`larmorx_core::math::Libm`).
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-maths.md`):
//! - **FS-29** (a fixed seed by default): `PDFtodSeed` returns the clock's seconds plus
//!   microseconds. By default it returns [`FIXED_SEED`]; `reproduce` reads the clock.
//! - **FS-30** (corrected by default): `PDFgaussian`'s loop accepts the radius 0 (both draws
//!   exactly 0.5) and returns `0 * sqrt(inf)`, NaN. By default it draws again; `reproduce`
//!   returns the NaN.

use std::time::{SystemTime, UNIX_EPOCH};

use larmorx_core::defects::{FS_29, FS_30};
use larmorx_core::math::Libm;

use crate::globals::Globals;

/// The seed [`pdf_tod_seed`] returns by default (FS-29).
pub const FIXED_SEED: u64 = 1;

/// `PDFtodSeed()`: `tv_sec + tv_usec` of `gettimeofday` as an `unsigned long` with `reproduce`;
/// [`FIXED_SEED`] by default (FS-29, noted in both modes).
pub fn pdf_tod_seed(g: &mut Globals) -> u64 {
    if g.notices.reproduce() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        g.notices
            .add(&FS_29, "PDFtodSeed: a seed from the clock, as FreeSurfer");
        return now.as_secs().wrapping_add(u64::from(now.subsec_micros()));
    }
    g.notices.add(
        &FS_29,
        format!("PDFtodSeed: the fixed seed {FIXED_SEED}, not the clock"),
    );
    FIXED_SEED
}

/// `PDFgaussian()`: `v1 = 2 drand48() - 1`, `v2 = 2 drand48() - 1` until `r2 = v1² + v2²` is at
/// most 1, then `v1 sqrt(-2 log(r2) / r2)`, all in `double` (FS-30 for `r2 = 0`).
pub fn pdf_gaussian(g: &mut Globals) -> f64 {
    let libm = Libm::quiet(g.mode());
    loop {
        let v1 = 2.0 * g.drand48.drand48() - 1.0;
        let v2 = 2.0 * g.drand48.drand48() - 1.0;
        let r2 = v1 * v1 + v2 * v2;
        if r2 > 1.0 {
            continue;
        }
        if r2 == 0.0 {
            if !g.notices.reproduce() {
                g.notices
                    .add(&FS_30, "PDFgaussian: a radius of 0 drawn again");
                continue;
            }
            g.notices
                .add(&FS_30, "PDFgaussian: a radius of 0 gives NaN, as FreeSurfer");
        }
        return v1 * (-2.0 * libm.log(r2) / r2).sqrt();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;
    use larmorx_core::drand48::Drand48;

    #[test]
    fn gaussian_is_marsaglia_on_drand48() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        g.drand48 = Drand48::seeded(1234);
        let mut d = Drand48::seeded(1234);
        let x = pdf_gaussian(&mut g);
        let want = loop {
            let v1 = 2.0 * d.drand48() - 1.0;
            let v2 = 2.0 * d.drand48() - 1.0;
            let r2 = v1 * v1 + v2 * v2;
            if r2 <= 1.0 {
                break v1 * (-2.0 * larmorx_core::math::log(r2) / r2).sqrt();
            }
        };
        assert_eq!(x.to_bits(), want.to_bits());
        assert_eq!(pdf_tod_seed(&mut g), FIXED_SEED);
        assert!(g.notices.contains(&FS_29));
    }
}
