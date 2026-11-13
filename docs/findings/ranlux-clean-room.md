# RANLUX389 and its distributions: the clean room against FreeSurfer (black box)

The clean room `larmorx_core::ranlux` (`crates/larmorx-core/src/ranlux/`, FreeSurfer's foundations,
queue item 3.1-05, 2026-11-13) implements [`specs/ranlux.md`](../../specs/ranlux.md): FreeSurfer
8.2.0's random generator (Lüscher's subtract-with-borrow at luxury 389, James's seeding), its
uniform deviates and the eight distributions FreeSurfer draws from (flat, Gaussian, the unit
Gaussian, exponential, gamma with its three parts, chi-squared, F, t), with FS-100 and FS-101 as
D13 switches. Written from the spec only; checked against FreeSurfer's own compiled functions (the
probe `fs_rng_probe`) and its official `mri_volsynth`, both run as black boxes
(`validation/investigations/freesurfer-cleanroom-random-spline/`, `compare_random.py`; result
`result-2026-11-13-random.json`). **Open to clean-room implementers**: everything here comes from
the spec and from black-box runs.

## The result

From the clean commit `84a2044d` (2026-11-13, the main machine, `result-2026-11-13-random.json`;
the probe's SHA-256 `64b85eb28619…`, `mri_volsynth`'s `6b23b86ea7a7…`):

| What (spec) | Compared | Identical to FreeSurfer | Other |
|---|---|---|---|
| the generator (§10.1): the first 100,000 values of each seed | 2,022 seeds: §2.5's eleven (their hashes as the spec gives them), 0, 2147483564, 2⁶⁴ − 1, 314159265, −5 and 9 times 2147483563, 2³¹ − 1, −2³¹, 2⁵³ ± 1, 2⁶³ ± 1, 1,000 random 64-bit seeds, 1,000 random 32-bit seeds sign-extended | **2,022** | – |
| the all-zero seeds (§2.6, FS-101) | 12 seeds (`m · 2147483563`, m = 1 to 8 and −1 to −4) | FreeSurfer's stream is 0; `reproduce` identical (12) | the default gives §8.2's corrected stream (16777215 ten times, 16777214, 16777215, ...) with the notice (12) |
| the distributions (§10.2): 139 cases × seeds 53, 1 and 4294967296, 20,000 draws each; `mixed` (14,000) × 3; t at 2 + 2⁻⁵¹ (3 draws) | 421 cases, 8,382,003 draws, NaN, infinite and zero parameters included | **8,381,462** | **541 expected**: large-shape gamma draws (shape ≥ 12, chi-squared with ν ≥ 24, F with a half ≥ 12, `mixed`'s `gamma(25)` and `gamma(31.6)`) that met one of the 41,708 tangent arguments where glibc 2.35's `tan` is not correctly rounded (10,613 draws met one; 10,072 of them kept FreeSurfer's bits). 0 unexpected |
| the same with the C library's `tan` (`--system-tan`, a black box) | 8,382,003 draws | **8,382,003** | the tangent is the only cause of a difference |
| FS-100 under `reproduce` (§10.5): shapes 2³² + 0.5, 2³² + 12.25, 2³³ + 8, 10³⁰⁰, NaN, +∞ | 18 cases, 36,000 draws | 35,995 | 5 expected (2³² + 12.25: the truncated shape 12 takes `G_large`); with the system's `tan` all identical |
| FS-100: −1.5 and −∞ | the probe stopped after 5 s (both) | – | the clean room's error in both modes ("FS-100: the gamma shape -1.5 is negative", "... -inf is not a finite number"); the default's notices and errors for the others |
| FS-101 under `reproduce`: the draws that return (seeds ±2147483563) | 20 cases, 2,000 draws (uniform, flat, gamma 0, 12 and 25, `gamma_large(30)`, chi-squared 0 and 24, `fdist(24, 24)`) | **2,000** | – |
| FS-101 under `reproduce`: the draws that never return | 22 cases (positive uniform, Gaussian, unit Gaussian, exponential, gamma 0.4, 3 and 12.5, chi-squared 5, F, t 1.5 and 7) | the probe stopped after 5 s (22) | the clean room's error "FS-101: with seed <S> FreeSurfer's <draw> never returns" (22) |
| the official `mri_volsynth` (§10.4), one thread | 10 fields (`z` seeds 53 and 1234; `chi2` 30, 3, 24; `t` 9, 2, 1; `F` 3/25, 30/50), 8 × 7 × 6 × 3 voxels | **10,080 of 10,080** float32 voxels, with both tangents (21 draws met a differing tangent; the rounding to float32 hid each) | – |
| the tangent's arguments (§10.3) | 16,777,216 | – | 41,708 differ (20,360 with `k < 2²³`), as the spec says |

**Speed** (§9; the main machine, best of 3 runs, other agents running): 20 million integers
printed in decimal, FreeSurfer's probe **2.32 s**, the clean room **0.58 s** (4.0 times faster);
the generation alone 0.21 to 0.23 s (about 11 ns per delivered value, under 1 ns per recurrence
step).

**Status**: every value FreeSurfer's functions give is reproduced bit for bit, except the
large-shape gamma draws that meet one of glibc's 41,708 tangent arguments, where larmorx keeps the
correctly rounded `tan` by the user's decision (GLIBC-07): about 0.65 ‰ of the large-shape draws
here (541 of about 840,000), in their last bits.

## What the black-box runs added to the spec

- **The all-zero stream (FS-101, §2.6 and §8.2): integer gamma shapes of 12 or more return.** The
  spec says every draw that needs a positive uniform or a Gaussian never returns, "gamma with a
  shape above 0" among them. But `G_large` draws plain uniforms only: with `U = 0` the tangent is 0,
  `T = A − 1 > 0`, and `V = 0` passes the test, so `gamma(n, b)` with an integer `n ≥ 12` gives
  `b·(n − 1)` at once. *Verified* with the probe at seed 2147483563: `gamma(12, 1)` = 11,
  `gamma(25, 0.5)` = 12, `gamma_large(30)` = 29, `chisq(24)` = 22, `fdist(24, 24)` = 1, each time
  (`observations.sh`, `compare_random.py`'s `defects`). A non-integer shape of 12 or more still never
  returns (its fraction draws a positive uniform). **larmorx:** `reproduce` gives these values, as
  FreeSurfer does; the error "FS-101: ... never returns" only for the draws that really never return
  (positive uniform, Gaussian, unit Gaussian, exponential, gamma below 12 or with a fraction,
  chi-squared, F and t otherwise).
- **The 64-bit seeds whose state is all zero (§2.6).** The spec lists "2, 3, 4, 5 and 6 times
  [2147483563] and −2, −4, −5 times it". *Verified* for every multiple from −12 to 12: the zero
  seeds are `m · 2147483563` for **m = 1 to 8 and m = −1 to −4**; −5 and 9 are not (the probe and the
  clean room agree on all 25). Why, from §2.2's arithmetic: for `t₀ = m·2147483563` the first step's
  quotient is `m·40014` plus `m·12211 quo 53668`; for `1 ≤ m ≤ 4` that is `m·40014` and `t₁ = 0`; for
  `5 ≤ m ≤ 8` it is one more and `t₁ = −2147483563`, which the one correction brings to 0; from 9 on
  the correction is not enough. For a negative `m` the truncation runs the other way, `t₁` is
  `+2147483563` for `−8 ≤ m ≤ −5`, which no correction touches, so only −1 to −4 give 0.
- **`mixed`'s rotation** (§10.2's check of each draw's consumption) is not in the spec. *Verified*
  from the probe's output by trying every distribution of §10.2 at each position on a copy of the
  clean room's generator (`cleanroom-probe infer-mixed`): 14 draws per round, `uni`, `uni_pos`,
  `flat(−3.5, 7.25)`, `gaussian(2.5)`, `ugauss`, `gamma(3, 2)`, `gamma(0.4, 1.5)`, `gamma(7.3, 0.5)`,
  `gamma(25, 1)`, `gamma(31.6, 1)`, `chisq(5)`, `fdist(4, 9)`, `tdist`, `exponential(0.75)`, the t
  draw's degrees of freedom 1.5 in even rounds and 7 in odd ones (positions 0 and 1 give the same
  bits as a uniform and a positive uniform, position 10 as `gamma(2.5, 2)`).
- **The Gaussian's factor at `R = 1` (§4.2)** cannot be observed: the spec says the factor
  `sqrt(−2·log(R) / R)` is `+0` there, but its own expression gives `−0` (`−2 · (+0) = −0`,
  `−0 / 1 = −0`, `sqrt(−0) = −0`). `R = 1` needs `U₁ = 1/2` with `U₂ = 0`, or `U₁ = 0` with
  `U₂ = 1/2`: about once in 2⁴⁷ attempts, never seen. **larmorx** follows the expression (`−0`),
  which changes only the sign of a zero result; a question for the spec writer.
- **The tangent's arguments (§6, §10.3).** *Verified* without `tan_check.c`: `cleanroom-probe
  tan-list` compares the C library's `tan` (glibc 2.35, through Rust's `f64::tan`, a black box) with
  `larmorx_core::math::tan` on every argument `fl(π_d · k · 2⁻²⁴)`: **41,708** of 16,777,216 differ,
  20,360 of them with `k < 2²³`, as the spec says (list SHA-256 `ad594509f44e…`).
- **The probe's number forms**: it reads parameters as C's `strtod` does (`nan(0x123)` gives the
  payload `0x123`, hexadecimal floats such as `0x1.0000000000001p+1`), and seeds modulo 2⁶⁴
  (`−53` is 2⁶⁴ − 53).

## larmorx's own limits (not FreeSurfer's)

- **FS-100's correction for shapes of 2¹⁰²³ or more**: the default's `G_large(floor(a))` computes
  `sqrt(2a − 1)`, which overflows, and the draw gives +∞ (with `U = 0`, NaN) where the distribution's
  values are finite (its relative spread is below 2⁻⁵⁰⁰, so `a` itself is the right value). Shapes
  that large come from nowhere in FreeSurfer; recorded, not corrected.
- **Speed**: the generator is a tight loop over a fixed buffer (24 values before each group of 389,
  the borrow from the sign of each difference, no branch per value); a group cannot be computed in
  parallel (each step needs the previous borrow).
