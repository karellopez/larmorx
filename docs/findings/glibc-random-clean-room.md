# glibc 2.35's rand, srand, random and srandom: the clean room against the C library (black box)

The clean room `larmorx_core::glibc_random` (`crates/larmorx-core/src/glibc_random/`, FreeSurfer's
foundations, queue item 3.1-04's glibc part, 2026-11-13) implements
[`specs/glibc-random.md`](../../specs/glibc-random.md): the C library's default generator as the GNU
C Library 2.35 (Ubuntu 22.04, x86-64) gives it, which FreeSurfer 8.2.0's `mri_robust_template`
(its random first time point) and `setRandomSeed` call. Written from the spec only; checked against
the C library's own functions (the probe `glibc_random_probe`, a program that calls `srand`, `rand`,
`srandom` and `random`) and against FreeSurfer's official `mri_robust_template`, both run as black
boxes (`validation/investigations/freesurfer-cleanroom-random-spline/`, `compare_glibc_random.py`;
result `result-2026-11-13-glibc-random.json`). **Open to clean-room implementers**: everything here
comes from the spec and from black-box runs.

## The result

From the clean commit `1071fcd1` (2026-11-13, the main machine, glibc 2.35-0ubuntu3.15,
`result-2026-11-13-glibc-random.json`; the probe built with Ubuntu's `cc` 11.4 at `-O2`, SHA-256
`ad890bf551cc…`; `mri_robust_template`'s `9c5672e1fe4a…`, the clean room's probe `59ba5444b2d8…`):

| What (spec) | Compared | Identical to the C library |
|---|---|---|
| the stream before any seeding (§4, §10 item 1) | `default 100000` | **100,000 values** |
| the edge seeds (§10 item 2), through `srand` and `rand` and through `srandom` and `random` | 21 seeds (0 to 3, 53, 1234, 2836, 16807, 127772 to 127774, 2³¹ − 2 to 2³¹ + 1, 2³¹ − 1 + 127773, 2³² − 2, 2³² − 1, 2 · (2³¹ − 1), 2³² − 1 − 127773, 16807 · 127773 mod 2³²) and 12 `long` seeds converted as C converts them (−1, −2, −53, −(2³¹ − 1), −2³¹, −2³¹ − 1, 2³², 2³² + 5, 2⁴⁰ + 7, −2⁴⁰ − 7, 2⁶³ − 1, −2⁶³), 10,000 values each | **66 of 66 streams** (660,000 values) |
| random seeds (§10 item 3) | 150 32-bit and 30 64-bit seeds (fixed seed 20261113), 10,000 values each through `rand`, every third also through `random` | **240 of 240 streams** (2,400,000 values) |
| one shared state (§3, §10 item 4) | `shared <seed> 1000` on 20 seeds (0, 1, 53, 2³¹ − 1, 2³¹, 2³² − 1, −1 and 13 random): `rand` and `random` alternating after `srand`; `srand` then `srandom` as `setRandomSeed` calls them; `srandom` alone | **20 of 20**; in the C library's own output the three blocks of 1,000 are the same for all 20 |
| long streams and blocks of seeds (§6, §10 item 5) | the hashes of 10⁶ values of seeds 1, 53, 1234 and 2³¹; seeds 0 to 2²⁰ − 1 (8 values each) and 2³¹ − 512 to 2³¹ + 511 (64 each) | **6 of 6**, and equal to the spec's numbers |
| every seed (§10 item 6) | all 2³² seeds, the first 4 values of each, compared as 256 blocks of 2²⁴ seeds (the hash of each block) | **256 of 256 blocks**: all 2³² seeds (17,179,869,184 values) |
| the official `mri_robust_template` (§7, §10 item 7) | "Will use TP k as random initial target" for `N` = 2, 3, 5, 7 inputs (8 × 8 × 8 volumes from the official `mri_volsynth`) and seeds 1, 2, 53, 1234, 99991, 2³¹ − 1, −1, against `(o₀ mod N) + 1` from the clean room | **28 of 28** (also in the cache's `refresh` mode: 35 fresh runs of the official programs, each equal to its stored entry) |
| the golden cases (§10 item 8) | seeds 0, 1, 2³¹, 2³² − 1, the `long` −1 and the unseeded state: the first 4 values and the 1,000th | their SHA-256 recorded from the probe's output; the clean room's unit tests check them on every platform |

No difference anywhere. The spec registers no defect (§8), and none was found. The run made 644
runs of the original programs, 366 of them served by the oracle-output cache (earlier runs of the
same commands); the 256 blocks of every seed ran fresh, and a run of the `mri_robust_template`
section in the cache's `refresh` mode ran all 35 of its programs again, each equal to its entry.

## Speed

Both programs on the same machine, the best of five runs each (`compare_glibc_random.py`'s `speed`;
the machine was shared with other work, so the numbers vary by a few tens of per cent between runs):

| | The C library (the probe) | The clean room | Faster |
|---|---|---|---|
| drawing: `long 53 100000000` (10⁸ values and their FNV-1a hash) | 0.435 s | 0.094 s | 4.6× |
| seeding: `hash 0 1048576 8` (2²⁰ seedings, 8 values each) | 1.231 s | 0.110 s | 11.2× |
| every seed (§10 item 6): 2³² seedings, 4 values each, 4 threads (4 processes for the probe) | 1,420 s | 131 s | 10.9× |

How: a draw is one addition of two words of a 31-word ring and a shift, inlined, with no lock (the
caller owns the state; the C library's functions share one process-wide state and are
thread-safe, as their manual pages say). Seeding computes Park and
Miller's 30 words as `(16807ⁱ mod (2³¹ − 1)) · (r₀ mod (2³¹ − 1)) mod (2³¹ − 1)` from a table of the
powers, so they do not wait for each other (the same residues as the spec's step-by-step congruence,
which the unit tests check on 20,011 seeds term by term), and the 310 discarded terms in a flat
buffer as three interleaved chains of additions kept in registers (the recurrence's short lag is 3).
The state is 31 words and two indices; no allocation.

## What the black-box runs added to the spec

1. `mri_robust_template` with the 8 × 8 × 8 inputs of §10 item 7 ends at once with exit status 1,
   after its "Will use TP k" line: "Input image is smaller than min dimension: 8 < 16" (the random
   time point is drawn before the inputs' sizes are checked). So the 30-second stop of
   `robust_template_tp.sh` is never reached, and the oracle-output cache stores the runs (they end
   by themselves).
2. The program prints the converted seed: `--seed -1` gives "(seed 4294967295 )", C's conversion of
   §7.
3. The probe reads its seeds with C's `strtoll(s, NULL, 0)`: a leading `0` makes a seed octal and
   `0x` hexadecimal. The comparisons pass decimal seeds without leading zeros; the clean room's probe
   (`cleanroom-probe glibc`) reads decimal only and saturates an out-of-range seed as `strtoll` does.

## larmorx's own limits

None found. `GlibcRandom` is a value: `new()` (the unseeded state, seed 1's), `seeded(s)` and
`seed(s)` (`srand` and `srandom`), `seed_c_long(v)` (C's conversion of a wider signed seed, modulo
2³²), `next_u31()` (the stream), `rand()` and `random()` (the same stream in C's `int` and `long`)
and `fill`. Only the default state of 31 words is implemented; `initstate`, `setstate`, the
reentrant `_r` functions and `rand_r` are outside the spec (§5) and are not provided.
