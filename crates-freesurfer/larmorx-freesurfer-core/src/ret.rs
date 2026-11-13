// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! What a FreeSurfer function that takes an optional output object and returns a pointer gave
//! back (larmorx's own plumbing for `fs.maths`, queue items 3.1-23 and 3.1-24).
//!
//! FreeSurfer's matrix and histogram functions follow one convention: `T *F(..., T *dst)`
//! fills `dst` when the caller gives one, allocates a new object when `dst` is `NULL`, and
//! returns the object it filled, or `NULL` on an error (the caller's `dst` then stays the
//! caller's, unchanged or partly written as the C code leaves it). The translations take the
//! output as `Option<&mut T>` and return a [`Ret`]: the new object, the caller's (`Given`), or
//! `Null`. Where the C code reads an input that is also its output (the same pointer twice), the
//! Rust caller passes a copy of the input (the C functions that allow it copy the input first, or
//! read every element before writing it), or calls the function's `_in_place` form where the
//! aliasing changes the arithmetic (as `HISTOsmooth`'s does).

/// The result of a FreeSurfer function `T *F(..., T *dst)`: see the module's documentation.
#[derive(Clone, Debug, PartialEq)]
pub enum Ret<T> {
    /// A new object (the caller gave no output).
    New(T),
    /// The caller's output, filled.
    Given,
    /// `NULL`.
    Null,
}

impl<T> Ret<T> {
    /// Whether the function returned `NULL`.
    pub fn is_null(&self) -> bool {
        matches!(self, Ret::Null)
    }

    /// The new object, if the function allocated one.
    pub fn into_new(self) -> Option<T> {
        match self {
            Ret::New(t) => Some(t),
            _ => None,
        }
    }

    /// The new object; panics on `Given` and `Null` (for callers that gave no output and know
    /// the function cannot fail).
    pub fn unwrap_new(self) -> T {
        match self {
            Ret::New(t) => t,
            Ret::Given => panic!("Ret::unwrap_new on the caller's own output"),
            Ret::Null => panic!("Ret::unwrap_new on NULL"),
        }
    }
}
