// SPDX-License-Identifier: Apache-2.0
//! The fast arctangent (spec §12): a table of 100,001 floats, `T[i] = (float) atan2(i, 100000)`,
//! looked up by the ratio of the smaller magnitude to the larger, then brought into the right
//! quadrant; the C library's `atan2f` under `FREESURFER_fastApproxAtan2f_use_atanf2`. FS-06: the
//! pairs FreeSurfer's lookup reads outside its table for (a NaN, two infinities, an overflowing
//! product) get defined values in both modes.

use std::sync::OnceLock;

use larmorx_core::defects::FS_06;
use larmorx_core::math;

use crate::process::{BUILD_ROOT, Process};
use crate::sse;

/// The table's size.
pub const TABLE_LEN: usize = 100_001;

/// The table, built once (correctly rounded `atan2` in double, rounded to float).
pub fn table() -> &'static [f32] {
    static TABLE: OnceLock<Box<[f32]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        (0..TABLE_LEN)
            .map(|i| math::atan2(i as f64, 100_000.0) as f32)
            .collect()
    })
}

/// What one lookup met: a pair inside FreeSurfer's table, or one of FS-06's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lookup {
    Table,
    /// FS-06: a NaN or two infinite arguments (`atan2f`'s value).
    NanOrInfinities,
    /// FS-06: `100000·min` overflowed a float (computed on the scaled magnitudes).
    Overflow,
}

/// The table's index for the ratio `small / large` (`large ≥ small`, both finite or `large`
/// infinite): `(int)((100000·small)/large)` in float, an overflowing product computed on both
/// magnitudes scaled by 2^-64.
#[inline]
fn index(small: f32, large: f32) -> (usize, bool) {
    if large == 0.0 {
        return (0, false);
    }
    let p = 100_000.0f32 * small;
    if p.is_infinite() {
        let scale = f32::from_bits(0x1f80_0000); // 2^-64
        let q = (100_000.0f32 * (small * scale)) / (large * scale);
        return (q as usize, true);
    }
    ((p / large) as usize, false)
}

/// `fastApproxAtan2f(y, x)` (the table's path), and what the lookup met.
pub fn fast_atan2f(y: f32, x: f32) -> (f32, Lookup) {
    if y.is_nan() || x.is_nan() {
        // atan2f's value for a NaN: the NaN operand, quieted (SSE's rule for x + y)
        return (sse::add32(x, y), Lookup::NanOrInfinities);
    }
    if y.is_infinite() && x.is_infinite() {
        return (
            math::atan2(f64::from(y), f64::from(x)) as f32,
            Lookup::NanOrInfinities,
        );
    }
    let t = table();
    let (ax, ay) = (x.abs(), y.abs());
    let (r, over) = if ax >= ay {
        let (i, o) = index(ay, ax);
        (t[i], o)
    } else {
        let (i, o) = index(ax, ay);
        (std::f32::consts::FRAC_PI_2 - t[i], o)
    };
    let pi = f64::from(std::f32::consts::PI);
    let r = f64::from(r);
    let v = match (x >= 0.0, y >= 0.0) {
        (true, true) => r,
        (true, false) => -r,
        (false, true) => pi - r,
        (false, false) => r - pi,
    };
    (
        v as f32,
        if over {
            Lookup::Overflow
        } else {
            Lookup::Table
        },
    )
}

impl Process {
    /// `fastApproxAtan2f(y, x)`: the table's arctangent, or the C library's `atan2f` when
    /// `FREESURFER_fastApproxAtan2f_use_atanf2` is set (read once; the first call then writes its
    /// line on standard output). The C library's: correctly rounded by default, glibc 2.35's bits
    /// with `reproduce`.
    pub fn fast_approx_atan2f(&mut self, y: f32, x: f32) -> f32 {
        let slow = match self.atan_slow {
            Some(s) => s,
            None => {
                let s = self.env.is_set("FREESURFER_fastApproxAtan2f_use_atanf2");
                self.atan_slow = Some(s);
                if s {
                    self.put_out(
                        format!("{BUILD_ROOT}/utils/utils.cpp:1727 using slow atan2f\n").as_bytes(),
                    );
                }
                s
            }
        };
        if slow {
            return if self.mode.reproduce() {
                math::atan2f_glibc(y, x)
            } else {
                math::atan2(f64::from(y), f64::from(x)) as f32
            };
        }
        let (v, what) = fast_atan2f(y, x);
        match what {
            Lookup::Table => {}
            Lookup::NanOrInfinities => self.note(
                &FS_06,
                "the fast arctangent of a NaN or two infinite arguments gives atan2f's value",
            ),
            Lookup::Overflow => self.note(
                &FS_06,
                "the fast arctangent's overflowing ratio computed on scaled magnitudes",
            ),
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn table_hash() {
        let mut h = Sha256::new();
        for v in table() {
            h.update(v.to_le_bytes());
        }
        let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "4bb06d9e2f3a1b01bf4256f7befbd5f9842036095048023174106d0ef7f8addf"
        );
    }

    #[test]
    fn quadrants_and_defects() {
        assert_eq!(fast_atan2f(0.0, -0.0).0.to_bits(), 0);
        assert_eq!(fast_atan2f(1.0, f32::INFINITY), (0.0, Lookup::Table));
        assert_eq!(
            fast_atan2f(f32::INFINITY, 1.0).0,
            std::f32::consts::FRAC_PI_2 - table()[0]
        );
        assert_eq!(fast_atan2f(f32::NAN, 1.0).1, Lookup::NanOrInfinities);
        let (v, w) = fast_atan2f(f32::from_bits(0x7f00_0000), f32::MAX);
        assert_eq!(w, Lookup::Overflow);
        assert!(v.is_finite());
        assert_eq!(
            fast_atan2f(f32::INFINITY, f32::NEG_INFINITY).0,
            (3.0 * std::f64::consts::FRAC_PI_4) as f32
        );
    }
}
