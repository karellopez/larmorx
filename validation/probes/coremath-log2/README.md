# coremath-log2

The check of the port of CORE-MATH's `log2` (`crates/larmorx-core/src/math/log2.rs`,
`larmorx_core::math::log2`, the default of `Libm::log2`) against its C, and of the translation of
glibc 2.35's `log2` (`larmorx_core::math::log2_glibc`, the `reproduce` choice: Arm's Optimized
Routines v19.11 `log2.c` without fused multiply-adds) against the system's `log2`, called as a black
box; and how often glibc's `log2` is not correctly rounded. Made for FreeSurfer's joint-histogram
costs (`mri_robust_register --cost MI|NMI|ECC`), track C relay 47, 2026-11-23.

- `src/main.rs` (its own cargo workspace, never part of larmorx's build), the harness of
  `../coremath-log10/` for `log2`: `coremath-log2 <libcrlog2.so> [log2n] [--threads T]
  [--wc <log2.wc>]` compares the bits of `larmorx_core::math::log2` (the FMA copy on this machine; the
  portable one is checked against it by `larmorx-core`'s `fma_copies_are_bit_identical`), of
  CORE-MATH's C `cr_log2` and of glibc's `log2` on 22 special values and every power of two from
  2^-1022 to 2^1023, on every hard-to-round case of CORE-MATH's `log2.wc` at `x` and `-x`, and on
  `2^log2n` arguments of each of eight families (random bit patterns; uniform on (0, 1000); next to 1;
  log-uniform normal numbers; subnormal numbers; the powers of two moved by -3 to 3 units in the last
  place; p-values; integers and dyadic fractions). NaN results equal any NaN for the C (its
  `0.0 / 0.0` for `x < 0` is the processor's default NaN, larmorx's `0x7ff8...`, counted apart);
  `log2_glibc` must equal the system's `log2` in every bit, NaNs included. Prints the hashes of the
  two unit tests `seeded_hash` (from the C and from the system's `log2`) and `VERIFY-PASS` or
  `VERIFY-FAIL` (exit status 1).

```bash
PATH=/usr/bin:$PATH bash validation/probes/coremath-log2/build.sh <scratch>/log2 <workspace>/tags/core-math
scripts/orchestration/boxed.sh 4 <scratch>/log2/coremath-log2 <scratch>/log2/libcrlog2.so 28 \
    --threads 6 --wc <workspace>/tags/core-math/src/binary64/log2/log2.wc
```

How glibc's build was found (measured, not read): Arm's v19.11 `log2.c` built with `HAVE_FAST_FMA`
and `-mfma` differs from the system's `log2` on 175,963 of 402,653,199 arguments; built with
`HAVE_FAST_FMA` 0 and without FMA code (GCC 11.4, `-O2`) it equals it on all of them, NaN bits
included. `log2_glibc` translates that form.

Meaningful on x86-64 Linux with glibc 2.35 (the oracle machine). `build.sh` compiles `log2.c`
(CORE-MATH commit `040ee482a8ca`) with `gcc -O2 -ffp-contract=off -fPIC -shared` (the system's gcc
11.4: `PATH=/usr/bin:$PATH` when another compiler comes first).

## Results (2026-11-23)

`coremath-log2 ... 28 --threads 4 --wc log2.wc` (log
`<workspace>/oracles/logs/fs-found-c-r47-coremath-log2-28.log`): larmorx equals the C on all
2,147,552,192 arguments (NaN bits aside for `x < 0`, as above); `log2_glibc` equals glibc's `log2` on
all of them, NaN bits included; the seeded hashes are `0x7fdf578b8ea688b3` (the C and larmorx) and
`0xd5a3591b5b6bfea5` (glibc and `log2_glibc`). glibc's `log2` is not correctly rounded on:

| family (2^28 arguments each unless stated) | glibc's `log2` not correctly rounded |
|---|---|
| special values and 2^k (2,068) | 0 |
| `log2.wc` at ±x (66,476; half NaN) | 7,637 (11.49 %) |
| random bit patterns (half negative: NaN) | 1,385 (0.0005 %) |
| uniform on (0, 1000) | 32,820 (0.0122 %) |
| next to 1 | 162,729 (0.0606 %) |
| log-uniform normal numbers | 2,741 (0.0010 %) |
| subnormal numbers | 341 (0.0001 %) |
| powers of two ±3 ulp | 0 |
| p-values | 259,438 (0.0966 %) |
| integers and dyadic fractions | 56,392 (0.0210 %) |

## `glibc-form/`: which form of Arm's `log2.c` glibc 2.35 runs (2026-11-23)

`build.sh <out> <optimized-routines-v19.11>/math` builds Arm's `log2.c` and `log2_data.c` (MIT; not
copied here) with this folder's minimal `math_config.h` and runs nothing; then, in `<out>`:
`./cmp 67108864` compares the form without `HAVE_FAST_FMA` and without FMA code against the system's
`log2` on 402,653,199 arguments (six families of 2^26 and special values): 0 differ; `./cmp_fma
67108864` the `HAVE_FAST_FMA` form built with `-mfma -mavx2`: 175,963 differ (all next to 1 or in
the main path's last bit). `search*.c` and `near1.c` tried every combination of fused and unfused
multiply-adds in both paths (`search3`: the non-FMA form with no fusion is the only one with 0 of
200,000 differing). glibc's own code was not read; its `log2` is called as a black box.
