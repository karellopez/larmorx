// SPDX-License-Identifier: Apache-2.0
//! Family `order`: the closed-form determinants and inverses of 2 x 2 to 4 x 4 matrices with
//! non-finite and extreme entries (`larmorx_core::vnl_fixed::cases`), every value compared bit
//! for bit, NaNs included (their signs and payloads come from the operand order of each `+` and
//! `*`, which `order/orders.py` read from the object code): `vnl_det<T>` and `vnl_determinant<T>`
//! of rows (libitkvnl's and libitkvnl_algo's instances), `vnl_determinant<T>(vnl_matrix)` of 1 to
//! 4 rows, and `vnl_inverse<T>` (the stand-alone instances, libutils' byte for byte where it has
//! one), for `float` and `double`. A matrix whose determinant is 0 has no inverse to compare
//! (VNL's is uninitialised memory, ITK-107): counted.

use larmorx_core::vnl_fixed::{self, cases};
use larmorx_core::x87::cases::SplitMix64;

use crate::{Lib, Tally};

#[derive(Default)]
struct Nans(u64, u64);

impl Nans {
    fn f32s(&mut self, v: &[f32]) {
        for x in v.iter().filter(|x| x.is_nan()) {
            self.0 += 1;
            self.1 += u64::from(x.to_bits() != 0xffc0_0000);
        }
    }
    fn f64s(&mut self, v: &[f64]) {
        for x in v.iter().filter(|x| x.is_nan()) {
            self.0 += 1;
            self.1 += u64::from(x.to_bits() != 0xfff8_0000_0000_0000);
        }
    }
}

pub fn run(lib: &Lib, scale: u64, t: &mut Tally) {
    let mut rng = SplitMix64::new(0x0de7_0bad_5eed);
    let mut singular = 0u64;
    // NaN results of the original: all, and those that are not x86-64's default NaN (an input's
    // NaN passed on, where the operand order decides which)
    let mut c = Nans::default();
    for n in 1..=4usize {
        for k in 0..(25_000 * scale) {
            t.cases += 1;
            let m = cases::matrix_f32(&mut rng, n);
            let what = |f: &str| format!("{f} f32 n={n} case {k} {:08x?}", m.iter().map(|x| x.to_bits()).collect::<Vec<_>>());
            let ours = vnl_fixed::determinant(&m, n);
            // SAFETY: n x n floats in, one float out (the shim's contract).
            let theirs = unsafe { (lib.determinant_f32)(n as i32, m.as_ptr(), 0) };
            t.f32s(&|| what("vnl_determinant(matrix)"), &[ours], &[theirs]);
            c.f32s(&[theirs]);
            if n == 1 {
                continue;
            }
            // SAFETY: as above.
            let (det, rows) = unsafe {
                ((lib.det_fixed_f32)(n as i32, m.as_ptr()), (lib.order_determinant_rows_f32)(n as i32, m.as_ptr()))
            };
            t.f32s(&|| what("vnl_det"), &[ours], &[det]);
            t.f32s(&|| what("vnl_determinant(rows)"), &[ours], &[rows]);
            if ours == 0.0 {
                singular += 1;
                continue;
            }
            let mut inv = vec![0f32; n * n];
            // SAFETY: n x n floats in and out.
            unsafe { (lib.order_inverse_f32)(n as i32, m.as_ptr(), inv.as_mut_ptr()) };
            t.f32s(&|| what("vnl_inverse"), &vnl_fixed::inverse(&m, n), &inv);
            c.f32s(&inv);
        }
        for k in 0..(25_000 * scale) {
            t.cases += 1;
            let m = cases::matrix_f64(&mut rng, n);
            let what = |f: &str| format!("{f} f64 n={n} case {k} {:016x?}", m.iter().map(|x| x.to_bits()).collect::<Vec<_>>());
            let ours = vnl_fixed::determinant(&m, n);
            // SAFETY: n x n doubles in, one double out.
            let theirs = unsafe { (lib.determinant_f64)(n as i32, m.as_ptr(), 0) };
            t.f64s(&|| what("vnl_determinant(matrix)"), &[ours], &[theirs]);
            c.f64s(&[theirs]);
            if n == 1 {
                continue;
            }
            // SAFETY: as above.
            let (det, rows) = unsafe {
                ((lib.det_fixed_f64)(n as i32, m.as_ptr()), (lib.order_determinant_rows_f64)(n as i32, m.as_ptr()))
            };
            t.f64s(&|| what("vnl_det"), &[ours], &[det]);
            t.f64s(&|| what("vnl_determinant(rows)"), &[ours], &[rows]);
            if ours == 0.0 {
                singular += 1;
                continue;
            }
            let mut inv = vec![0f64; n * n];
            // SAFETY: n x n doubles in and out.
            unsafe { (lib.order_inverse_f64)(n as i32, m.as_ptr(), inv.as_mut_ptr()) };
            t.f64s(&|| what("vnl_inverse"), &vnl_fixed::inverse(&m, n), &inv);
            c.f64s(&inv);
        }
    }
    t.notes.push(format!(
        "{} NaN results of the original among them (determinants of matrices and inverses), {} of them an input's NaN passed on (not the default NaN)",
        c.0, c.1
    ));
    t.notes.push(format!("{singular} matrices with a zero determinant: no inverse compared (ITK-107)"));
}
