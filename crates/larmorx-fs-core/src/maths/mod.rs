// SPDX-License-Identifier: Apache-2.0
//! FreeSurfer 8.2.0's mathematics (`fs.maths`), a clean room of `specs/freesurfer-maths.md`:
//! the float and double matrices (§2, §4, §8, §12, §16), the 4 x 4 affine helpers (§3), the
//! histograms (§5, §21), the decompositions (§8.8, §17), primes (§9), the Gaussian draws (§10),
//! the minimizers (§13), Numerical Recipes' 1-based arrays (§14), the matrix files (§18) and the
//! random-field specifications (§19).
//!
//! Every function is a method of the program's [`Process`] (its mode, messages and notices) or a
//! plain function where it needs none. FreeSurfer's functions take an optional output object and
//! return the object filled or nothing; here the output is an [`Output`] (none given, a separate
//! object, or one of the inputs) and the result a [`Returned`].
//!
//! Where FreeSurfer reads or writes outside an array it allocated, the clean room reads 0 and
//! drops the write, in both modes (§1): the arrays here are FreeSurfer's arrays, with their
//! lengths, and every access outside them goes through those two rules.

pub mod affine;
pub mod dmatrix;
pub mod elements;
pub mod histo;
pub mod matrix;

use larmorx_core::defects::{FS_17, GLIBC_07};
use larmorx_core::drand48::Drand48;

use crate::process::Process;

/// Where a function's output goes: FreeSurfer's optional output pointer.
pub enum Output<'a, T> {
    /// None given: the function makes a new object.
    New,
    /// A separate object of the caller's.
    Given(&'a mut T),
    /// The first input object itself (in place).
    First,
    /// The second input object itself.
    Second,
}

impl<T> Output<'_, T> {
    /// Whether an output was given (any but [`Output::New`]).
    pub fn given(&self) -> bool {
        !matches!(self, Output::New)
    }
}

/// What a function returned: nothing (C's `NULL`), a new object, or its output argument itself
/// (the given object, or the input it named).
#[derive(Clone, Debug, PartialEq)]
pub enum Returned<T> {
    Nothing,
    New(T),
    Output,
}

impl<T> Returned<T> {
    /// Whether nothing was returned.
    pub fn is_nothing(&self) -> bool {
        matches!(self, Returned::Nothing)
    }
}

/// What `fs.maths` keeps for the whole run: the matrices' allocator chosen at the first
/// allocation (§2.2) and the C library's `drand48` state (§10).
#[derive(Clone, Debug, Default)]
pub(crate) struct MathsState {
    pub(crate) old_alloc: Option<bool>,
    pub(crate) drand48: Drand48,
}

impl Process {
    /// C's `(int) x` of a float or double (truncation toward zero, §1): beyond the `int` range
    /// or NaN, FS-17: `-2147483648` with `reproduce`, else saturated (NaN 0).
    pub fn to_int(&mut self, x: f64) -> i32 {
        if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 {
            if self.mode().reproduce() {
                self.note(
                    &FS_17,
                    "a conversion to an integer beyond its range (or of NaN) gave -2147483648",
                );
                i32::MIN
            } else {
                self.note(
                    &FS_17,
                    "a conversion to an integer beyond its range saturated (NaN 0)",
                );
                if x.is_nan() {
                    0
                } else if x > 0.0 {
                    i32::MAX
                } else {
                    i32::MIN
                }
            }
        } else {
            x.trunc() as i32
        }
    }

    /// A double function of the C library (§1, §7): larmorx's correctly rounded one by default,
    /// glibc 2.35's with `reproduce`; GLIBC-07 noted where the two differ.
    pub(crate) fn libm(&mut self, name: &str, x: f64, cr: fn(f64) -> f64, glibc: fn(f64) -> f64) -> f64 {
        let (a, b) = (cr(x), glibc(x));
        self.libm_pick(name, a.to_bits() != b.to_bits() && !(a.is_nan() && b.is_nan()), a, b)
    }

    /// `pow` as [`Process::libm`].
    pub(crate) fn libm_pow(&mut self, x: f64, y: f64) -> f64 {
        let (a, b) = (larmorx_core::math::pow(x, y), larmorx_core::math::pow_glibc(x, y));
        self.libm_pick("pow", a.to_bits() != b.to_bits() && !(a.is_nan() && b.is_nan()), a, b)
    }

    /// A float function of the C library (`expf`, `sinf`, `cosf`) as [`Process::libm`].
    pub(crate) fn libmf(&mut self, name: &str, x: f32, cr: fn(f32) -> f32, glibc: fn(f32) -> f32) -> f32 {
        let (a, b) = (cr(x), glibc(x));
        let differ = a.to_bits() != b.to_bits() && !(a.is_nan() && b.is_nan());
        if differ {
            self.note_glibc07(name);
        }
        if self.mode().reproduce() { b } else { a }
    }

    fn libm_pick(&mut self, name: &str, differ: bool, cr: f64, glibc: f64) -> f64 {
        if differ {
            self.note_glibc07(name);
        }
        if self.mode().reproduce() { glibc } else { cr }
    }

    fn note_glibc07(&mut self, name: &str) {
        if self.mode().reproduce() {
            self.note(&GLIBC_07, format!("glibc's {name}, not correctly rounded, gave this result's last bit"));
        } else {
            self.note(&GLIBC_07, format!("{name} correctly rounded (glibc's differs in the last place)"));
        }
    }

    /// The C library's process-wide `drand48` state (§10).
    pub fn drand48_state(&mut self) -> &mut Drand48 {
        &mut self.maths.drand48
    }
}
