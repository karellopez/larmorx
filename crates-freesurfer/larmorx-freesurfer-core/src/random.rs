// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's random numbers (`fs.core`, queue item 3.1-22), translated from FreeSurfer
//! 8.2.0's `utils/utils.cpp` (`setRandomSeed`, `getRandomSeed`, `getRandomCalls`,
//! `randomNumber`, `compute_permutation`) with `utils/numerics.cpp`'s `OpenRan1`, the one place
//! they draw from.
//!
//! `OpenRan1` keeps a VXL `vnl_random` (Marsaglia and Zaman's subtract-with-borrow, seeded by a
//! linear congruential generator; ITK 4.13.2's copy in the official build, TP-01) and its last
//! seed in `static` variables: it reseeds the generator whenever it is called with a seed other
//! than the last one, and returns `drand64(0, 1)` rounded to `float`. `vnl_random` is ported into
//! the main package (`larmorx_core::vnl_random`, queue item 3.1-04, the permissive track); the
//! generator is reached here through [`Ran1Source`]: [`Random::new`] installs
//! `larmorx_core::vnl_random::VnlRandom` ([`vnl_source`]); tests may install another with
//! [`Random::set_source`]. `setRandomSeed` also seeds the C library's `rand` and `random` (`srand`,
//! `srandom`); their seed is kept in [`Random::c_seed`] for the generators of glibc's behaviour
//! (queue item 3.1-04's clean room).
//!
//! - **FS-04** (corrected by default): `randomNumber` with no seed set (`idum` 0, also after
//!   `setRandomSeed(0)`) and `FREESURFER_SEED` unset or empty seeds the generator with
//!   `-abs((int) time(NULL))`, after the line `<command> supposed to be reproducible but seed not
//!   set`. The default seeds it with -1 (the seed most FreeSurfer programs set, `setRandomSeed(-1L)`)
//!   and notes FS-04; `reproduce` reads the clock as FreeSurfer does.
//! - **FS-05** (corrected by default): a `FREESURFER_SEED` that `std::stol` cannot read (no
//!   digits, or beyond a 64-bit `long`) throws an exception nobody catches: libstdc++'s
//!   `terminate called after throwing an instance of 'std::invalid_argument'` (or
//!   `'std::out_of_range'`) and `  what():  stol`, then `abort`, exit status 134. The default
//!   ends the program with a clear error and exit status 1; `reproduce` writes libstdc++'s lines
//!   and ends with 134.

use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use larmorx_core::defects::{FS_04, FS_05};
use larmorx_core::vnl_random::VnlRandom;

use crate::cstd;
use crate::globals::Globals;
use crate::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use crate::run::Environment;

/// The generator `OpenRan1` draws from: VXL's `vnl_random` (`larmorx_core::vnl_random`).
pub trait Ran1Source: std::fmt::Debug + Send {
    /// `vnl_random::reseed(seed)` (an `unsigned long`).
    fn reseed(&mut self, seed: u64);
    /// `vnl_random::drand64(0.0, 1.0)`.
    fn drand64_unit(&mut self) -> f64;
    /// A copy with the same state.
    fn box_clone(&self) -> Box<dyn Ran1Source>;
}

/// Makes the generator `vnl_random(seed)` (`OpenRan1`'s `static vnl_random mVnlRandom(mSeed)`).
pub type Ran1Factory = fn(u64) -> Box<dyn Ran1Source>;

impl Ran1Source for VnlRandom {
    fn reseed(&mut self, seed: u64) {
        VnlRandom::reseed(self, seed);
    }

    fn drand64_unit(&mut self) -> f64 {
        self.drand64(0.0, 1.0)
    }

    fn box_clone(&self) -> Box<dyn Ran1Source> {
        Box::new(self.clone())
    }
}

/// `vnl_random(seed)`: the main package's port of ITK 4.13.2's VXL generator, the one the
/// official programs link ([`Random::new`]'s generator).
pub fn vnl_source(seed: u64) -> Box<dyn Ran1Source> {
    Box::new(VnlRandom::new(seed))
}

/// `OpenRan1`'s `static` state: its last seed (`mSeed`, 0 at start) and its generator, made at
/// the first call with seed 0.
#[derive(Debug)]
pub struct OpenRan1 {
    m_seed: i64,
    source: Option<Box<dyn Ran1Source>>,
    factory: Option<Ran1Factory>,
}

impl Clone for OpenRan1 {
    fn clone(&self) -> Self {
        OpenRan1 {
            m_seed: self.m_seed,
            source: self.source.as_ref().map(|s| s.box_clone()),
            factory: self.factory,
        }
    }
}

impl OpenRan1 {
    /// `OpenRan1(&seed)`: reseeds when `seed` is not the last one, then `drand64(0, 1)` as a
    /// `float`. (FreeSurfer stops a program that calls it from an OpenMP thread other than 0; the
    /// replica's callers draw on one thread.)
    pub fn draw(&mut self, seed: i64) -> f32 {
        if self.source.is_none() {
            let factory = self.factory.expect(
                "OpenRan1: no generator installed (Random::set_source; larmorx_core::vnl_random)",
            );
            self.source = Some(factory(0));
        }
        let source = self.source.as_mut().expect("made above");
        if self.m_seed != seed {
            self.m_seed = seed;
            source.reseed(seed as u64);
        }
        source.drand64_unit() as f32
    }
}

/// The random-number state of `utils.cpp` (`seedHasBeenSet`, `idum`, `nrgcalls`, the `static`
/// `laterTime` of `randomNumber`), the seed given to the C library's generators, and
/// [`OpenRan1`].
#[derive(Clone, Debug)]
pub struct Random {
    /// `seedHasBeenSet`.
    pub seed_has_been_set: bool,
    /// `idum`: the seed `randomNumber` passes to `OpenRan1` (0: none set).
    pub idum: i64,
    /// `nrgcalls`: 1 after a seed is set, one more per number drawn.
    pub nrgcalls: i64,
    /// `randomNumber`'s `static int laterTime`: the unseeded message was written.
    later_time: bool,
    /// The seed `srand` and `srandom` were given (`unsigned int`), when `setRandomSeed` ran.
    pub c_seed: Option<u32>,
    /// `OpenRan1`'s state.
    pub ran1: OpenRan1,
}

impl Default for Random {
    fn default() -> Self {
        Self::new()
    }
}

impl Random {
    /// FreeSurfer's state at a program's start, `OpenRan1` drawing from VXL's `vnl_random`
    /// ([`vnl_source`]).
    pub fn new() -> Self {
        Random {
            seed_has_been_set: false,
            idum: 0,
            nrgcalls: 0,
            later_time: false,
            c_seed: None,
            ran1: OpenRan1 {
                m_seed: 0,
                source: None,
                factory: Some(vnl_source),
            },
        }
    }

    /// Installs the generator `OpenRan1` makes at its first call.
    pub fn set_source(&mut self, factory: Ran1Factory) {
        self.ran1.factory = Some(factory);
    }
}

/// `setRandomSeed(seed)`: seeds `srand` and `srandom` with `seed`'s low 32 bits; when `seed` is
/// the current `idum`, draws once with `seed - 1` first (FreeSurfer's way of restarting the
/// sequence: `OpenRan1` reseeds only on a new seed); then `idum = seed`, one draw with it,
/// `nrgcalls = 1`. Returns `NO_ERROR`.
pub fn set_random_seed(g: &mut Globals, seed: i64) -> i32 {
    let r = &mut g.random;
    r.c_seed = Some(seed as u32);
    if r.idum == seed {
        r.ran1.draw(seed.wrapping_sub(1));
    }
    r.idum = seed;
    r.ran1.draw(r.idum);
    r.nrgcalls = 1;
    r.seed_has_been_set = true;
    NO_ERROR
}

/// `getRandomSeed()`.
pub fn get_random_seed(g: &Globals) -> i64 {
    g.random.idum
}

/// `getRandomCalls()`.
pub fn get_random_calls(g: &Globals) -> i64 {
    g.random.nrgcalls
}

/// The name `/proc/self/comm` gives a program: its first 15 bytes, `/` written as `@` (as
/// `randomNumber` and ROMP's statistics file read it).
pub fn comm_of(program: &str) -> String {
    let bytes: Vec<u8> = program
        .bytes()
        .take(15)
        .map(|b| if b == b'/' { b'@' } else { b })
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// What `std::stol(s)` (base 10) gives: the value, or the exception it throws.
fn stol(s: &[u8]) -> Result<i64, &'static str> {
    let mut i = 0;
    while i < s.len() && cstd::is_space(s[i]) {
        i += 1;
    }
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        i += 1;
    }
    if i >= s.len() || !s[i].is_ascii_digit() {
        return Err("std::invalid_argument");
    }
    let v = cstd::strtol10(s);
    let digits = &s[i..];
    let n = digits.iter().take_while(|b| b.is_ascii_digit()).count();
    let neg = s[..i].contains(&b'-');
    // strtol saturates and sets ERANGE beyond the long range; stol throws then.
    let exact: Option<i64> = digits[..n].iter().try_fold(0i64, |acc, &d| {
        let d = i64::from(d - b'0');
        if neg {
            acc.checked_mul(10)?.checked_sub(d)
        } else {
            acc.checked_mul(10)?.checked_add(d)
        }
    });
    match exact {
        Some(x) => {
            debug_assert_eq!(x, v);
            Ok(x)
        }
        None => Err("std::out_of_range"),
    }
}

/// The seed `randomNumber` takes from the clock: `-1L * (long) abs((int) time(NULL))`
/// (`abs(INT_MIN)` stays `INT_MIN`, as x86-64 computes it).
fn clock_seed() -> i64 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    -i64::from((secs as i32).wrapping_abs())
}

/// `randomNumber(low, hi)`: a number between `low` and `hi` (swapped when `low > hi`):
/// `OpenRan1(&idum) * (hi - low) + low`, the `float` draw widened to `double`. With no seed set
/// it first takes `FREESURFER_SEED` (printing `Setting seed for random number genererator to
/// <value>` on standard output, then `setRandomSeed`), or else seeds as FS-04 says. A result
/// outside `[low, hi]` (a draw rounded up to 1.0f) is reported with `ErrorPrintf` and returned.
/// `Err` only for FS-05's ending.
pub fn random_number(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    low: f64,
    hi: f64,
) -> Result<f64, Fatal> {
    let (low, hi) = if low > hi { (hi, low) } else { (low, hi) };
    if g.random.idum == 0 {
        let seed = env
            .get("FREESURFER_SEED")
            .map(|v| v.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !seed.is_empty() {
            let _ = writeln!(out, "Setting seed for random number genererator to {seed}");
            let _ = out.flush();
            match stol(seed.as_bytes()) {
                Ok(s) => {
                    set_random_seed(g, s);
                }
                Err(exception) => {
                    if g.notices.reproduce() {
                        let _ = write!(
                            err,
                            "terminate called after throwing an instance of '{exception}'\n  what():  stol\n"
                        );
                        g.notices
                            .add(&FS_05, "FREESURFER_SEED is not a 64-bit integer: aborted");
                        return Err(Fatal::Exit(134));
                    }
                    log::error(
                        g,
                        err,
                        &format!(
                            "FREESURFER_SEED must be an integer in the range of a 64-bit long, not '{seed}'"
                        ),
                    );
                    g.notices.add(
                        &FS_05,
                        "FREESURFER_SEED is not a 64-bit integer: a clear error, not an abort",
                    );
                    return Err(Fatal::Exit(1));
                }
            }
        } else {
            if !g.random.later_time {
                g.random.later_time = true;
                let _ = writeln!(
                    err,
                    "{} supposed to be reproducible but seed not set",
                    g.comm
                );
            }
            if g.notices.reproduce() {
                g.random.idum = clock_seed();
                g.notices.add(
                    &FS_04,
                    "random numbers seeded from the clock, as FreeSurfer",
                );
            } else {
                g.random.idum = -1;
                g.notices
                    .add(&FS_04, "random numbers seeded with -1, not the clock");
            }
        }
    }
    let range = hi - low;
    let idum = g.random.idum;
    let val = f64::from(g.random.ran1.draw(idum)) * range + low;
    g.random.nrgcalls += 1;
    if val < low || val > hi {
        let f = |x: f64| cstd::fmt_f(f64::from(x as f32), 2, 1);
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("randomNumber({}, {}) - {}\n", f(low), f(hi), f(val)),
        );
    }
    Ok(val)
}

/// `compute_permutation(num, vec)`: `0 .. num-1`, each place `n` in turn swapped with the place
/// `(int) randomNumber(0.0, num - 0.0001)`.
pub fn compute_permutation(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    num: i32,
) -> Result<Vec<i32>, Fatal> {
    let n = usize::try_from(num).unwrap_or(0);
    let mut vec: Vec<i32> = (0..num.max(0)).collect();
    for i in 0..n {
        let index = random_number(g, env, out, err, 0.0, f64::from(num) - 0.0001)? as i32 as usize;
        vec.swap(index, i);
    }
    Ok(vec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    /// A stand-in for `vnl_random` that records what `OpenRan1` asks of it: its draws are
    /// `seed / 2^64 + k / 1000` for the k-th draw after the last reseed.
    #[derive(Clone, Debug)]
    struct Probe {
        seed: u64,
        k: u64,
        reseeds: Vec<u64>,
    }

    impl Ran1Source for Probe {
        fn reseed(&mut self, seed: u64) {
            self.seed = seed;
            self.k = 0;
            self.reseeds.push(seed);
        }
        fn drand64_unit(&mut self) -> f64 {
            self.k += 1;
            (self.seed as f64 / 18446744073709551616.0 + self.k as f64 / 1000.0).fract()
        }
        fn box_clone(&self) -> Box<dyn Ran1Source> {
            Box::new(self.clone())
        }
    }

    fn probe(seed: u64) -> Box<dyn Ran1Source> {
        Box::new(Probe {
            seed,
            k: 0,
            reseeds: vec![],
        })
    }

    fn globals(mode: UpstreamDefects) -> Globals {
        let mut g = Globals::new(mode);
        g.random.set_source(probe);
        g.comm = comm_of("mri_ca_register_long_name/x");
        g
    }

    fn reseeds(g: &Globals) -> Vec<u64> {
        format!("{:?}", g.random.ran1.source)
            .split("reseeds: [")
            .nth(1)
            .map(|s| {
                s.split(']')
                    .next()
                    .unwrap_or("")
                    .split(", ")
                    .filter(|x| !x.is_empty())
                    .map(|x| x.parse().unwrap())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// FreeSurfer 8.2.0's own sequences (the unit oracle, `fsunit fs-random <seed> <n> <low>
    /// <high>`: `setRandomSeed(seed)`, then `randomNumber` n times, each call a fresh process;
    /// 2026-11-12): the SHA-256 of 200 numbers in [0, 1] then 50 in [-3.5, 7.25], as
    /// little-endian doubles, for the seeds the programs set, with the default generator
    /// (`larmorx_core::vnl_random`).
    #[test]
    fn the_official_sequences() {
        use sha2::{Digest, Sha256};
        let want = [
            (-1i64, "417f30724a318bfa4fdceaf8c5d7c5fd790f3cd2dccd994cec78209048a20598"),
            (1234, "26d7d15e40cda364532d58b44d33d46963c3af567b63c6cbcbc2a3509a6282b4"),
            (53, "0e82b68234fe567f8ff8c4ca45c771b58efdbeb95f413708aaaeb0d766ba340f"),
        ];
        let env = Environment::from_pairs(Vec::<(String, String)>::new());
        for (seed, hash) in want {
            let mut h = Sha256::new();
            for (n, low, hi) in [(200, 0.0, 1.0), (50, -3.5, 7.25)] {
                let mut g = Globals::new(UpstreamDefects::Reproduce);
                set_random_seed(&mut g, seed);
                let (mut o, mut e) = (Vec::new(), Vec::new());
                for _ in 0..n {
                    let x = random_number(&mut g, &env, &mut o, &mut e, low, hi).unwrap();
                    h.update(x.to_le_bytes());
                }
            }
            let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(hex, hash, "seed {seed}");
        }
    }

    #[test]
    fn set_random_seed_draws_once_and_restarts_on_the_same_seed() {
        let mut g = globals(UpstreamDefects::Fix);
        assert_eq!(set_random_seed(&mut g, 1234), NO_ERROR);
        assert_eq!((get_random_seed(&g), get_random_calls(&g)), (1234, 1));
        assert_eq!(g.random.c_seed, Some(1234));
        // The same seed again: a draw with 1233 first, so that 1234 restarts.
        set_random_seed(&mut g, 1234);
        assert_eq!(reseeds(&g), vec![1234, 1233, 1234]);
        // -1: an unsigned long of all ones; srand's unsigned int likewise.
        set_random_seed(&mut g, -1);
        assert_eq!(*reseeds(&g).last().unwrap(), u64::MAX);
        assert_eq!(g.random.c_seed, Some(u32::MAX));
    }

    #[test]
    fn random_number_is_the_float_draw_scaled() {
        let mut g = globals(UpstreamDefects::Fix);
        set_random_seed(&mut g, 0x8000_0000_0000_0000u64 as i64);
        let env = Environment::from_pairs::<&str, &str>([]);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        // The second draw after the reseed: 0.5 + 0.002, rounded to float.
        let v = random_number(&mut g, &env, &mut out, &mut err, 10.0, 2.0).unwrap();
        assert_eq!(v, f64::from(0.502f64 as f32) * 8.0 + 2.0);
        assert_eq!(get_random_calls(&g), 2);
        assert!(out.is_empty() && err.is_empty() && g.notices.is_empty());
    }

    #[test]
    fn unseeded_draws_fs04() {
        let env = Environment::from_pairs::<&str, &str>([]);
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let mut g = globals(mode);
            let (mut out, mut err) = (Vec::new(), Vec::new());
            random_number(&mut g, &env, &mut out, &mut err, 0.0, 1.0).unwrap();
            random_number(&mut g, &env, &mut out, &mut err, 0.0, 1.0).unwrap();
            assert_eq!(
                String::from_utf8(err).unwrap(),
                "mri_ca_register supposed to be reproducible but seed not set\n"
            );
            assert!(g.notices.contains(&FS_04));
            if mode.fix() {
                assert_eq!(get_random_seed(&g), -1);
            } else {
                assert!(get_random_seed(&g) < 0);
            }
            // nrgcalls counts from 0 here: no setRandomSeed ran.
            assert_eq!(get_random_calls(&g), 2);
        }
    }

    #[test]
    fn freesurfer_seed_and_fs05() {
        let env = Environment::from_pairs([("FREESURFER_SEED", " 12abc")]);
        let mut g = globals(UpstreamDefects::Fix);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        random_number(&mut g, &env, &mut out, &mut err, 0.0, 1.0).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "Setting seed for random number genererator to  12abc\n"
        );
        assert_eq!((get_random_seed(&g), get_random_calls(&g)), (12, 2));
        for (value, exception) in [
            ("abc", "std::invalid_argument"),
            ("-", "std::invalid_argument"),
            ("99999999999999999999", "std::out_of_range"),
        ] {
            let env = Environment::from_pairs([("FREESURFER_SEED", value)]);
            for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
                let mut g = globals(mode);
                let (mut out, mut err) = (Vec::new(), Vec::new());
                let r = random_number(&mut g, &env, &mut out, &mut err, 0.0, 1.0);
                let err = String::from_utf8(err).unwrap();
                if mode.fix() {
                    assert_eq!(r, Err(Fatal::Exit(1)));
                    assert!(err.starts_with("error: FREESURFER_SEED must be an integer"));
                } else {
                    assert_eq!(r, Err(Fatal::Exit(134)));
                    assert_eq!(
                        err,
                        format!(
                            "terminate called after throwing an instance of '{exception}'\n  what():  stol\n"
                        )
                    );
                }
                assert!(g.notices.contains(&FS_05));
            }
        }
        assert_eq!(stol(b"-9223372036854775808"), Ok(i64::MIN));
        assert_eq!(stol(b"9223372036854775808"), Err("std::out_of_range"));
        assert_eq!(stol(b"-0x10"), Ok(0));
    }

    #[test]
    fn permutation_and_comm() {
        let mut g = globals(UpstreamDefects::Fix);
        set_random_seed(&mut g, 7);
        let env = Environment::from_pairs::<&str, &str>([]);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let p = compute_permutation(&mut g, &env, &mut out, &mut err, 5).unwrap();
        let mut sorted = p.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1, 2, 3, 4]);
        assert_eq!(get_random_calls(&g), 6);
        assert_eq!(comm_of("a/b"), "a@b");
        assert_eq!(comm_of("mris_make_surfaces"), "mris_make_surfa");
    }
}
