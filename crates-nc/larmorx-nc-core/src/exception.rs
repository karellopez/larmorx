// SPDX-License-Identifier: LicenseRef-FSL
// Rust translation Copyright 2026 Karel Lopez Vilaret. Distributed under the FSL Licence (see
// LICENSE).
//! C++ exceptions thrown deep inside a computation whose Rust signature returns a value (a cost
//! function, a resampling): FSL's code throws where Armadillo's `inv()` finds a singular matrix
//! or newimage cannot allocate a volume, and the exception unwinds to whoever catches it (FLIRT's
//! `main`, or nobody: `std::terminate`). [`throw`] unwinds with the exception as the payload
//! (`std::panic::resume_unwind`, which prints nothing); [`catch`] stops it where FSL's code
//! catches it. A program's entry point catches what nothing else does and prints the C++
//! runtime's text for an uncaught exception.

use crate::volume::{Elem, FslError, Volume};

/// Throws `e` (unwinds to the nearest [`catch`]).
pub fn throw(e: FslError) -> ! {
    std::panic::resume_unwind(Box::new(e))
}

/// Runs `f`, returning the exception it throws ([`throw`]); any other panic goes on unwinding.
pub fn catch<R>(f: impl FnOnce() -> R) -> Result<R, FslError> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(r) => Ok(r),
        Err(payload) => match payload.downcast::<FslError>() {
            Ok(e) => Err(*e),
            Err(other) => std::panic::resume_unwind(other),
        },
    }
}

/// `volume<T>(x, y, z)` (`initialize`): a volume of zeros, or newimage's `Out of memory`
/// exception (number 99) where its allocation fails (FSL catches `std::bad_alloc` there).
pub fn try_volume<T: Elem>(x: i64, y: i64, z: i64, t: i64) -> Result<Volume<T>, FslError> {
    let n = [x, y, z, t]
        .iter()
        .try_fold(1usize, |acc, &d| acc.checked_mul(d.max(0) as usize));
    let ok = n.is_some_and(|n| {
        let mut probe: Vec<T> = Vec::new();
        probe.try_reserve_exact(n).is_ok()
    });
    if !ok {
        return Err(FslError::imthrow("Out of memory", 99));
    }
    Ok(Volume::new(x, y, z, t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thrown_and_caught() {
        let r: Result<u8, FslError> = catch(|| throw(FslError::imthrow("Out of memory", 99)));
        let e = r.unwrap_err();
        assert_eq!(e.what, "Out of memory");
        assert_eq!(catch(|| 7).unwrap(), 7);
    }

    #[test]
    fn impossible_volume() {
        let e = try_volume::<f32>(i64::MAX / 2, 4, 4, 1).unwrap_err();
        assert_eq!(
            e.printed,
            vec!["Image Exception : #99 :: Out of memory".to_owned()]
        );
    }
}
