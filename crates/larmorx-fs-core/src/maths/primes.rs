// SPDX-License-Identifier: Apache-2.0
//! Prime numbers (spec §9): the primes up to a bound, prime factors and the closest factor
//! (FS-32).

use larmorx_core::defects::FS_32;

use crate::process::Process;

impl Process {
    /// `GetPrimes(nmax)`: the primes up to `nmax`, increasing; `None` after its line for
    /// `nmax < 1`.
    pub fn get_primes(&mut self, nmax: i32) -> Option<Vec<i32>> {
        if nmax < 1 {
            self.put_out(format!("ERROR: GetPrimes: cannot compute primes for {nmax}\n").as_bytes());
            return None;
        }
        let n = nmax as usize;
        let mut composite = vec![false; n + 1];
        let mut out = Vec::new();
        for i in 2..=n {
            if composite[i] {
                continue;
            }
            out.push(i as i32);
            let mut j = i * i;
            while j <= n {
                composite[j] = true;
                j += i;
            }
        }
        Some(out)
    }

    /// `GetPrimeFactors(N)`: N's prime factors with multiplicity, increasing (none for 1); `None`
    /// after its line for `N < 1`.
    pub fn get_prime_factors(&mut self, n: i32) -> Option<Vec<i32>> {
        if n < 1 {
            self.put_out(format!("ERROR: cannot compute prime factors of {n}\n").as_bytes());
            return None;
        }
        Some(prime_factors(n))
    }

    /// `GetClosestPrimeFactor(N, P)` (`less`: `GetClosestPrimeFactorLess`); FS-32 for N = 1;
    /// `None` for N < 1 (FreeSurfer crashes after its line).
    pub fn get_closest_prime_factor(&mut self, n: i32, p: i32, less: bool) -> Option<i32> {
        let f = self.get_prime_factors(n)?;
        if f.is_empty() {
            if self.mode().reproduce() {
                self.note(&FS_32, "1 has no prime factor: the empty list's first element read (0)");
                return Some(0);
            }
            self.note(&FS_32, "1 has no prime factor: 1 returned");
            return Some(1);
        }
        let candidates: Vec<i32> = if less { f.iter().copied().filter(|&x| x < p).collect() } else { f.clone() };
        if candidates.is_empty() {
            return Some(f[0]);
        }
        let mut best = candidates[0];
        for &x in &candidates[1..] {
            if (x - p).abs() < (best - p).abs() {
                best = x;
            }
        }
        Some(best)
    }
}

/// The prime factors of `n` ≥ 1, increasing, with multiplicity.
fn prime_factors(mut n: i32) -> Vec<i32> {
    let mut out = Vec::new();
    let mut d = 2i64;
    while d * d <= i64::from(n) {
        while n as i64 % d == 0 {
            out.push(d as i32);
            n /= d as i32;
        }
        d += 1;
    }
    if n > 1 {
        out.push(n);
    }
    out
}
