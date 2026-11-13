// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's prime numbers (`fs.maths`, queue item 3.1-24), translated from FreeSurfer 8.2.0's
//! `utils/prime.cpp` (`GetPrimes`, `GetPrimeFactors`, `CompareFactors`, `GetClosestPrimeFactor`,
//! `GetClosestPrimeFactorLess`): mri_surf2surf's and mri_vol2surf's reshaping of a surface's
//! values.
//!
//! `GetPrimeFactors` sorts its factors with the C library's `qsort` and `CompareFactors`, a total
//! order of integers, so any sort gives the same list.
//!
//! Defect (`larmorx_core::defects`; the finding in `docs/findings/freesurfer-maths.md`):
//! - **FS-32** (corrected by default): the closest prime factor of 1, which has none, is read from
//!   the empty list (glibc's `calloc` gives 0 there). By default it is 1 (no reshaping);
//!   `reproduce` gives 0.

use std::io::Write;

use larmorx_core::defects::FS_32;

use crate::globals::Globals;

/// `GetPrimes(Nmax, &Nprimes)`: the primes up to `Nmax` (the sieve of Eratosthenes from each
/// prime's square, up to `(int) sqrt(Nmax)`), in increasing order; `None` after `ERROR:
/// GetPrimes: cannot compute primes for <Nmax>` on standard output when `Nmax < 1`. (FreeSurfer's
/// callers pass at most 46340, the square root of the largest `int`; the sieve's index is kept
/// in 64 bits, where FreeSurfer's `int` would wrap beyond `INT_MAX - k`.)
pub fn get_primes(out: &mut dyn Write, nmax: i32) -> Option<Vec<i32>> {
    if nmax < 1 {
        let _ = writeln!(out, "ERROR: GetPrimes: cannot compute primes for {nmax}");
        return None;
    }
    let n = nmax as usize;
    let mut numlist: Vec<i32> = (0..=nmax).collect();
    numlist[1] = 0;
    let kmax = f64::from(nmax).sqrt() as i64;
    for k in 2..=kmax {
        if numlist[k as usize] != 0 {
            let mut m = k * k;
            while m <= nmax as i64 {
                numlist[m as usize] = 0;
                m += k;
            }
        }
    }
    let _ = n;
    Some(numlist.into_iter().filter(|&v| v != 0).collect())
}

/// `GetPrimeFactors(N, &Nfactors)`: `N`'s prime factors with their multiplicities, from lowest to
/// highest (the primes up to `floor(sqrt(N))` divided out in passes over the list, the remaining
/// cofactor last); 1 has none. `None` after `ERROR: cannot compute prime factors of <N>` on
/// standard output when `N < 1`.
pub fn get_prime_factors(out: &mut dyn Write, n: i32) -> Option<Vec<i32>> {
    if n < 1 {
        let _ = writeln!(out, "ERROR: cannot compute prime factors of {n}");
        return None;
    }
    let all = get_primes(out, f64::from(n).sqrt().floor() as i32)?;
    let mut factors = Vec::new();
    let mut nn = n;
    let mut changed = true;
    while changed {
        changed = false;
        for &p in &all {
            if nn % p == 0 {
                factors.push(p);
                nn /= p;
                changed = true;
            }
        }
    }
    if nn != 1 {
        factors.push(nn);
    }
    // qsort with CompareFactors
    factors.sort_unstable();
    Some(factors)
}

/// `GetClosestPrimeFactor(N, P)` (`less` false) and `GetClosestPrimeFactorLess(N, P)` (`less`
/// true): the prime factor of `N` nearest `P` (the first of equally near ones, from the lowest;
/// with `less`, the nearest below `P`, or the lowest factor when none is below). FS-32 for `N = 1`;
/// `N < 1`: FreeSurfer reads the factor list it did not make and crashes: `None` after
/// `GetPrimeFactors`' message (never reproduced).
pub fn get_closest_prime_factor(
    g: &mut Globals,
    out: &mut dyn Write,
    n: i32,
    p: i32,
    less: bool,
) -> Option<i32> {
    let factors = get_prime_factors(out, n)?;
    let name = if less {
        "GetClosestPrimeFactorLess"
    } else {
        "GetClosestPrimeFactor"
    };
    if factors.is_empty() {
        if g.notices.reproduce() {
            g.notices.add(
                &FS_32,
                format!("{name}({n}, {p}): the empty list's first element read, 0 as glibc's calloc gives it"),
            );
            return Some(0);
        }
        g.notices.add(
            &FS_32,
            format!("{name}({n}, {p}): 1, which has no prime factor"),
        );
        return Some(1);
    }
    let mut nmin = 0usize;
    let mut dmin = factors[nmin].wrapping_sub(p).wrapping_abs();
    for (i, &f) in factors.iter().enumerate() {
        let d = f.wrapping_sub(p).wrapping_abs();
        if dmin > d && (!less || f < p) {
            dmin = d;
            nmin = i;
        }
    }
    Some(factors[nmin])
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn primes_and_factors() {
        let mut o = Vec::new();
        assert_eq!(
            get_primes(&mut o, 30).unwrap(),
            [2, 3, 5, 7, 11, 13, 17, 19, 23, 29]
        );
        assert_eq!(get_primes(&mut o, 1).unwrap(), Vec::<i32>::new());
        assert!(get_primes(&mut o, 0).is_none());
        assert_eq!(o, b"ERROR: GetPrimes: cannot compute primes for 0\n");
        assert_eq!(get_prime_factors(&mut o, 360).unwrap(), [2, 2, 2, 3, 3, 5]);
        assert_eq!(get_prime_factors(&mut o, 97).unwrap(), [97]);
        assert_eq!(get_prime_factors(&mut o, 1).unwrap(), Vec::<i32>::new());
        assert_eq!(
            get_prime_factors(&mut o, 163842).unwrap(),
            [2, 3, 7, 47, 83]
        );
    }

    #[test]
    fn closest_factors_fs32() {
        let mut o = Vec::new();
        let mut g = Globals::new(UpstreamDefects::Fix);
        assert_eq!(
            get_closest_prime_factor(&mut g, &mut o, 163842, 50, false),
            Some(47)
        );
        assert_eq!(
            get_closest_prime_factor(&mut g, &mut o, 163842, 47, true),
            Some(7)
        );
        assert_eq!(
            get_closest_prime_factor(&mut g, &mut o, 8, 1, true),
            Some(2)
        );
        assert!(!g.notices.contains(&FS_32));
        assert_eq!(
            get_closest_prime_factor(&mut g, &mut o, 1, 5, false),
            Some(1)
        );
        assert!(g.notices.contains(&FS_32));
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        assert_eq!(
            get_closest_prime_factor(&mut g, &mut o, 1, 5, true),
            Some(0)
        );
    }
}
