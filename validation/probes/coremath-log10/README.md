# coremath-log10

The check of the port of CORE-MATH's `log10` (`crates/larmorx-core/src/math/log10.rs`,
`larmorx_core::math::log10`, the default of `Libm::log10`) against its C, an independent check of
the C's rounding against mpmath, and how often glibc 2.35's `log10` (the `reproduce` choice,
`larmorx_core::math::log10_glibc`) is not correctly rounded. glibc's `log10` is called as a black
box.

- `src/main.rs` (its own cargo workspace, never part of larmorx's build):
  `coremath-log10 <libcrlog10.so> [log2n] [--threads T] [--wc <log10.wc>]` compares the bits of
  `larmorx_core::math::log10` (the FMA copy on this machine; the portable one is checked against it
  by `larmorx-core`'s `fma_copies_are_bit_identical`), of CORE-MATH's C `cr_log10` and of glibc's
  `log10` on 45 special values and exact powers of ten, on every hard-to-round case of CORE-MATH's
  `log10.wc` at `x` and `-x`, and on `2^log2n` arguments of each of eight families: random bit
  patterns; uniform on (0, 1000); next to 1 (`1 + t`, `|t|` from 2^-53 to 2^-1: the accurate path);
  log-uniform normal numbers; subnormal numbers; the powers of ten from 1e-323 to 1e308 moved by -3
  to 3 units in the last place; p-values (uniform on (0, 1), scaled down by up to 2^-1000);
  integers and dyadic fractions. NaN results equal any NaN, but the C's `0.0 / 0.0` for `x < 0` (the
  processor's default NaN, `0xfff8...` on x86-64) against larmorx's `0x7ff8...` (glibc's too) is
  counted apart. It also checks `log10_glibc` against glibc's `log10` bit for bit on every argument,
  and prints the hash of `larmorx-core`'s unit test `math::log10::tests::seeded_hash` from the C.
  Prints `VERIFY-PASS` or `VERIFY-FAIL` (exit status 1).
- `mpmath_check.py`: the C's `cr_log10` against mpmath's `log10` at 400 bits rounded to nearest on
  every positive case of `log10.wc` and on seeded arguments (random bits, next to 1, p-values).
  Prints `MPMATH-PASS` or `MPMATH-FAIL`.
- `coremath_tables.py`: transcribes log10.c's fast-path tables `_INVERSE` and `_LOG_INV` into the
  bit patterns of `log10.rs`; `--check-shared <core-math>/src/binary64` shows that the polynomial `P`
  and dint.h's accurate-path tables are log.c's (`SHARED-SAME`), which `log10.rs` therefore takes
  from `log.rs`.

```bash
PATH=/usr/bin:$PATH bash validation/probes/coremath-log10/build.sh <scratch>/log10 <workspace>/tags/core-math
scripts/orchestration/boxed.sh 4 <scratch>/log10/coremath-log10 <scratch>/log10/libcrlog10.so 28 \
    --threads 6 --wc <workspace>/tags/core-math/src/binary64/log10/log10.wc
scripts/orchestration/boxed.sh 4 <workspace>/.venv/bin/python -I validation/probes/coremath-log10/mpmath_check.py \
    <scratch>/log10/libcrlog10.so <workspace>/tags/core-math/src/binary64/log10/log10.wc 100000
python -P validation/probes/coremath-log10/coremath_tables.py --check-shared <workspace>/tags/core-math/src/binary64
```

Meaningful on x86-64 Linux with glibc 2.35 (the oracle machine). `build.sh` compiles `log10.c`
(CORE-MATH commit `040ee482a8ca`) with `gcc -O2 -ffp-contract=off -fPIC -shared` (the system's gcc
11.4: `PATH=/usr/bin:$PATH` when another compiler comes first).

## Results (2026-11-14)

`coremath-log10 ... 28 --threads 6 --wc log10.wc` (log
`<workspace>/oracles/logs/fs-found-c-r22-coremath-log10-28.log`, about 2 minutes): larmorx equals
the C on all 2,147,614,729 arguments (NaN bits aside for `x < 0`, as above); `log10_glibc` equals
glibc's `log10` on all of them, NaN bits included; the seeded hash is `0xb615169b046f3d5a` from both.
glibc's `log10` is not correctly rounded on:

| family (2^28 arguments each unless stated) | glibc's `log10` not correctly rounded |
|---|---|
| special values and 10^n (45) | 1 |
| `log10.wc` at ±x (131,036; half NaN) | 16,271 (12.42 %) |
| random bit patterns (half negative: NaN) | 301,139 (0.112 %) |
| uniform on (0, 1000) | 10,292,969 (3.83 %) |
| next to 1 | 74,379,698 (27.7 %) |
| log-uniform normal numbers | 601,061 (0.224 %) |
| subnormal numbers | 58,258 (0.0217 %) |
| powers of ten ±3 ulp | 242,938 (0.0905 %) |
| p-values | 27,440,583 (10.2 %) |
| integers and dyadic fractions | 10,534,623 (3.92 %) |

`mpmath_check.py ... 100000` (log `<workspace>/oracles/logs/fs-found-c-r22-coremath-log10-mpmath.log`):
the C equals mpmath on all 65,508 positive cases of `log10.wc` and on 99,950 arguments of each of
the three seeded families.
