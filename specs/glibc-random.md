# Spec: glibc 2.35's `rand`, `srand`, `random` and `srandom` (x86-64), as FreeSurfer 8.2.0's programs get them (`glibc-random`)

(spec writer, track D of FreeSurfer's foundations, relay 2, 2026-11-12) The input for a clean-room
implementation, in the main package (`larmorx-core`, Apache-2.0), of the C library's default random
generator: the values `rand()` and `random()` return after `srand(s)` or `srandom(s)`, and before any
seeding, **bit for bit**, for every 32-bit seed, as the GNU C Library 2.35 (Ubuntu 22.04, x86-64, the
machine of the official FreeSurfer 8.2.0 build and of the reference runs) delivers them.

**Why.** FreeSurfer's programs call the C library's generator (TP-15 of FreeSurfer's third-party
audit); glibc is LGPL-2.1-or-later, so its code is never copied or translated (CLAUDE.md rule 2): this
spec describes the behaviour as mathematics, from which another agent writes the function, as for
`specs/glibc-dbl.md` and `specs/cbrtf.md`. The generator itself is published (the BSD `random(3)`
additive generator with the Park–Miller seeding congruence; P. Selinger's description of glibc's, §11);
this spec adds the exact conventions (the seed's conversion, the zero seed, the negative first word,
the discarded values, the shared state) that give glibc's numbers.

**Where FreeSurfer draws from it** (§7): `mri_robust_template` picks its random first time point with
one `srand(seed)` and one `rand()` (only without `--inittp`; sMRIPrep passes `--inittp 1`), and
FreeSurfer's `setRandomSeed` seeds `srand` and `srandom` in 67 programs. No other function of the
programs on FreeSurfer's default paths calls `rand`, `random`, `rand_r`, `random_r`, `initstate` or
`setstate` (the call graph's full view, 2026-11-12). The other C-library generator FreeSurfer uses,
`drand48` and its family, is defined exactly by POSIX and is already in `larmorx-core` (track C).

**Whoever implements it must not open:** glibc's source (`<workspace>/tags/glibc-2.35/`, any other
release or copy; among others `stdlib/random.c`, `stdlib/random_r.c`, `stdlib/rand.c`,
`stdlib/rand_r.c`), its disassembly, the spec writer's model
(`validation/investigations/glibc-random-spec-check/`), `crates-gpl/` and `crates-nc/`, `docs/parts/`
and the scratch folders of FreeSurfer's parts. It may read this spec, the publications of §11, the C
standard's and POSIX's text on `rand`, `srand`, `random`, `srandom`, and build and run the probe in
`validation/investigations/glibc-random-probe/` (calls of the C library's functions only) and the
official FreeSurfer programs as black boxes (§10). This spec holds the method as mathematics, no code;
what black-box runs confirmed is marked **(observed)**.

---

## 1. Notation

- **Integers** are exact mathematical integers. `a mod m` (for `m > 0`) is the **non-negative**
  residue, in `[0, m)`, also for a negative `a` (`−5 mod 3 = 1`). `⌊v / 2⌋` is the integer part of
  `v / 2` for `v ≥ 0` (the shift right by one bit).
- **A 32-bit word** is an integer in `[0, 2³²)`; "`v` as a word" is `v mod 2³²` (for a negative `v`,
  its 32-bit two's-complement pattern).
- **The seed** `s` is an integer in `[0, 2³²)` (C's `unsigned int`). A caller that passes a wider or
  signed integer has it converted by C's rule, modulo 2³² (FreeSurfer's `setRandomSeed(long)`, §7:
  `−1` becomes 4294967295, `2⁴⁰ + 7` becomes 7).
- `m = 2³¹ − 1 = 2147483647`, `a = 16807 = 7⁵`.

## 2. Seeding: `srand(s)` and `srandom(s)` (one function)

`srand(s)` and `srandom(s)` do the same thing **(observed**, §10 group 4**)**: they replace the
process's one state by the state of seed `s`:

1. If `s = 0`, use `s = 1` instead (so `srand(0)` and `srand(1)` give the same stream **(observed)**).
2. `r₀ = s` if `s < 2³¹`, else `r₀ = s − 2³²` (the seed read as a signed 32-bit integer: seeds from
   2³¹ give a negative `r₀`).
3. `rᵢ = (a · rᵢ₋₁) mod m` for `i = 1, …, 30` (Park and Miller's "minimal standard" congruence, the
   non-negative residue; for a negative `r₀`, `r₁` is the non-negative residue of the negative product:
   `r₀ = −1` gives `r₁ = 2147466840`). Every `rᵢ` with `i ≥ 1` is in `[0, m)`.
4. `r₀, …, r₃₀` are taken as words (§1: a negative `r₀` becomes `r₀ + 2³²`).
5. `r₃₁ = r₀`, `r₃₂ = r₁`, `r₃₃ = r₂` (copies), and for every `n ≥ 34`

   `rₙ = (rₙ₋₃₁ + rₙ₋₃) mod 2³²`

   (an additive lagged-Fibonacci recurrence with lags 31 and 3, the trinomial `x³¹ + x³ + 1`).
6. The seeding itself makes the terms `r₃₄, …, r₃₄₃` (310 terms, ten times the 31 words) and
   delivers none of them.

For `s = 1`: `r₁ = 16807`, `r₂ = 282475249`, `r₃ = 1622650073`, …, `r₃₀ = 1505795335`.

## 3. The stream: `rand()` and `random()` (one stream)

After the seeding, the `k`-th call (`k = 0, 1, 2, …`) of `rand()` or `random()` returns

  `oₖ = ⌊r₃₄₄₊ₖ / 2⌋`,

an integer in `[0, 2³¹ − 1]` (the top 31 bits of the word; `RAND_MAX = 2147483647`). `rand` returns it
as an `int`, `random` as a `long`: the same value.

- **One state.** `rand` and `random` advance the same stream: any interleaving of the two calls
  consumes `o₀, o₁, …` in call order; `srand` and `srandom` set that one state **(observed**, §10
  group 4: `srand(s)` then alternate `rand()`, `random()` calls give `o₀, o₁, …` of seed `s`; and
  `srand(s)` followed by `srandom(s)`, as FreeSurfer's `setRandomSeed` calls them, gives the stream of
  `srandom(s)` alone, the second call restarting it**)**.
- Only the last 31 terms are ever needed to continue the recurrence.
- The state is never all zero (`r₀ ≠ 0` for every seed), so no seed gives a constant stream.

## 4. The state before any seeding

A process that calls `rand()` or `random()` before any `srand` or `srandom` gets the stream of seed 1
(the C standard requires this of `rand`) **(observed**, 100,000 values**)**: `1804289383, 846930886,
1681692777, 1714636915, 1957747793, …`.

## 5. Not specified here

The other entry points of the same generator family are not reached by FreeSurfer's programs and are
not specified: `initstate` and `setstate` (other state sizes, which change the lags and the number of
words; a state of fewer than 32 bytes switches to a linear congruential generator), the reentrant `random_r`,
`srandom_r`, `initstate_r`, `setstate_r`, and `rand_r` (a different generator). If a later program
needs one of them, this spec gains a section from a new black-box record.

## 6. Reference numbers (observed)

`glibc_random_probe` (§10) on the reference machine (glibc 2.35, Ubuntu 22.04, x86-64), 2026-11-12:

| Seed `s` | `o₀ … o₄` |
|---|---|
| 1 (also 0, and no seeding) | 1804289383, 846930886, 1681692777, 1714636915, 1957747793 |
| 53 | 2099602078, 1877112656, 1115506446, 957877911, 2107065092 |
| 1234 | 479142414, 465566339, 961126155, 1057886067, 1222702060 |
| 2147483647 (`r₁ = … = r₃₀ = 0`) | 1065668062, 2142264300, 1066566375, 1064012770, 2141034222 |
| 2147483648 (`r₀ = −2³¹`) | 1336741213, 1210407648, 1447044896, 337392383, 82502902 |
| 4294967295 (`r₀ = −1`; a `long` −1) | 254925627, 1205188300, 366127624, 1401405153, 76053476 |

**Hashes.** FNV-1a 64-bit over the values as 32-bit integers (`h₀ = 14695981039346656037`, then
`h = (h XOR v) · 1099511628211 mod 2⁶⁴` for each value `v` in order):

| Probe command | What it hashes | Hash |
|---|---|---|
| `long 1 1000000` | `o₀ … o₉₉₉₉₉₉` of seed 1 | `8b12006d0a30178c` |
| `long 53 1000000` | of seed 53 | `8f2bb11c5dd49223` |
| `long 1234 1000000` | of seed 1234 | `54ec7c5ad4bf1bfe` |
| `long 2147483648 1000000` | of seed 2³¹ | `98464c3c54ef732f` |
| `hash 0 1048576 8` | seeds 0 to 2²⁰ − 1 in order, `o₀ … o₇` of each | `5d031cec3a78334a` |
| `hash 2147483136 1024 64` | seeds 2³¹ − 512 to 2³¹ + 511, `o₀ … o₆₃` of each | `d3acc423130f2161` |

## 7. FreeSurfer's calls (what the replicas pass and use)

- **`mri_robust_template`**, without `--inittp`: with `seed` its `--seed` value (C's `atoi`, converted
  to `unsigned int`) or, when that is 0, a seed computed from the input images (and the clock if that is
  0 again), it calls `srand(seed)` and takes the time point `(o₀ mod N) + 1` of its `N` inputs; it
  prints "Will use TP k as random initial target (seed s )." **(observed**: the official program
  agrees with `(o₀ mod N) + 1` from the probe on 28 of 28 cases, `N` = 2, 3, 5, 7, seeds 1, 2, 53,
  1234, 99991, 2147483647 and −1, §10 group 7**)**.
- **`setRandomSeed(seed)`** (a `long`) calls `srand(seed)` then `srandom(seed)`, both converting the
  seed modulo 2³²; it leaves the state of `srandom(seed mod 2³²)` (§3). On the default paths nothing
  draws from `rand` or `random` after it, so the state it sets is not observable there; a replica keeps
  it anyway, for any later caller.

## 8. Defects and the `reproduce` switch

None. Every behaviour above is deterministic, defined for every seed, and is the documented behaviour
of the C library (`srand(0)` meaning seed 1 is glibc's documented rule: kept, rule C, identical in
`fix` and `reproduce`). Two properties of FreeSurfer's *use* belong to `mri_robust_template`'s item,
not to this generator: the small bias of `o₀ mod N` (at most `N / 2³¹`), and the clock seed when the
image-derived seed is 0 (nondeterminism, never reproduced by default, D13).

## 9. What the clean room provides

In `larmorx-core` (a clean-room file of the GNU family, listed in the clean-room audit's tables):

- a generator **state value** (never a process-global variable: the caller, for instance
  `larmorx-freesurfer`'s replica of FreeSurfer's process-wide state, holds it), created as the state
  before any seeding (§4);
- **seed** with a 32-bit unsigned seed (§2), replacing the state; the conversion of a wider signed
  integer (§1) at the call site or as a second entry point;
- **next**, returning `oₖ` (§3) as an unsigned 32-bit integer in `[0, 2³¹ − 1]` and advancing the
  state by one;
- no threads, no locks (one state, one caller at a time), no allocation after creation, the same
  results on every platform (integer arithmetic only).

## 10. Validation plan (black box)

The probe `validation/investigations/glibc-random-probe/glibc_random_probe.c` (calls of the system's
`srand`, `rand`, `srandom`, `random` only; build with any C compiler on a machine with glibc 2.35:
`cc -O2 -o glibc_random_probe glibc_random_probe.c`) prints the C library's values; the clean room
passes when it gives the same values:

1. **The default stream:** `default 100000` against the clean room's unseeded state.
2. **Edge seeds,** 10,000 values each (`rand <seed> 10000`): 0, 1, 2, 3, 53, 1234, 2836, 16807, 127772,
   127773, 127774, 2147483646, 2147483647, 2147483648, 2147483649, 2147483647 + 127773, 4294967294,
   4294967295, 2 · 2147483647, 4294967295 − 127773, (16807 · 127773) mod 2³²; and C's conversion of a
   `long`: −1, −2, −53, −2147483647, −2147483648, −2147483649, 2³², 2³² + 5, 2⁴⁰ + 7, −2⁴⁰ − 7,
   2⁶³ − 1, −2⁶³.
3. **Random seeds:** 150 32-bit and 30 64-bit seeds (any generator), 10,000 values each; `random`
   against `srandom` for a subset.
4. **One state:** `shared <seed> 1000` (alternating `rand` and `random`; `srand` then `srandom`;
   `srandom` alone): three copies of the same 1,000 values.
5. **Long streams:** §6's `long` hashes (10⁶ values).
6. **Every seed:** all 2³² seeds, the first 4 values each (`raw <seed0> <count> 4` writes them as
   binary; or the `hash` command over blocks of seeds, compared with the clean room's hash of the same
   block). The spec writer's check ran this in two processes in about an hour.
7. **The official program:** `robust_template_tp.sh` (the probe folder) runs FreeSurfer's
   `mri_robust_template` for `N` inputs and a seed and compares its "Will use TP k" line with
   `(o₀ mod N) + 1` (28 cases); a replica of `mri_robust_template` passes the same cases.
8. **Golden cases** (CLAUDE.md rule 10), once the function is reachable from a registered tool: seeds
   0, 1, 2³¹ and 2³² − 1, the first 4 values of each and the 1,000th.

The spec writer's check (`validation/investigations/glibc-random-spec-check/`, closed) ran groups 1 to
6 against a model written from this text: all identical (its README).

## 11. References

- P. Selinger, "The GLIBC pseudo-random number generator" (2007,
  `https://www.mathstat.dal.ca/~selinger/random/`): the recurrence of §2 and §3 as glibc's.
- S. K. Park and K. W. Miller, "Random number generators: good ones are hard to find", Comm. ACM 31
  (1988) 1192–1201: the seeding congruence (multiplier 16807, modulus 2³¹ − 1).
- D. E. Knuth, *The Art of Computer Programming*, vol. 2, §3.2.2: additive (lagged-Fibonacci)
  generators and primitive trinomials.
- 4.3BSD's `random(3)` manual page: the interface, the 31-word default state, `RAND_MAX`.
- ISO/IEC 9899 (C), §7.22.2: `rand`, `srand`, the unseeded stream equal to seed 1's; POSIX.1-2017:
  `random`, `srandom`.
