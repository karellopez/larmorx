# glibc 2.35's double `sin`, `cos`, `sincos`, `tan`, `asin`, `acos`, `atan`: the spec's evidence

The tools behind `specs/glibc-dbl.md` (GLIBC-07, the `cbrtf` route of 2026-10-30): glibc's
functions are IBM's Accurate Mathematical Library (LGPL-2.1-or-later), so larmorx gets glibc's
bits for `reproduce` from a behaviour spec, never from glibc's code. Everything here is
Apache-2.0 and holds no number or line of glibc's: the black-box tool calls the system's
functions, and `tables.py` reads glibc's tables only at run time, to compare them with
definitions.

| File | What |
|---|---|
| `families.rs` | the seeded arguments of the record (spec §10.1): families per function, the special arguments; included verbatim by `blackbox/` and by any implementation's test |
| `blackbox/` | the black-box tool (its own cargo workspace): `record` hashes glibc's results in blocks of 2^22 per family and counts where they differ from `larmorx_core::math`'s correctly rounded functions; `list` prints arguments and results inside a block |
| `glibc-dbl-2.35-x86_64-blocks.json` | the record: glibc 2.35, x86-64 with FMA and AVX2 (glibc's FMA builds of `sin`, `cos`, `tan`, `asin`, `acos`, `atan`; `sincos`'s single build); 7 functions, 1,600 block hashes, 6.7 × 10^9 arguments, 184 special arguments each |
| `glibc-dbl-2.35-x86_64-sse2-blocks.json` | the same families of `sin` and `cos` with glibc's FMA builds switched off (`GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA`): its build for CPUs without FMA |
| `tables.py` | regenerates the spec's numbers from their definitions (the digits of 2/π, the constants, table `R` of sines and cosines of N/128) and compares them with glibc's, bit for bit; surveys tan's, asin's/acos's and atan's tables against their natural definitions; `--emit` writes the definitions' table `R` and digits for an implementation; `--search` the other rules tried for table `R`; `--print` lists glibc's numbers where they differ (not for a clean-room implementer) |

The spec writer's model of `sin`, `cos` and `sincos`, written from the spec's text, is
`crates-gpl/tools/glibc-dbl-model` (in the GPL workspace: its author had read glibc's source; a
clean-room implementer must not open it, but may run it as a black box).

## Results (2026-11-04, main machine: glibc 2.35, x86-64 with FMA and AVX2, 28 threads)

- **The record** (`blackbox record`, 14.8 s; the SSE2 one 4.6 s): the shares that differ from the
  correctly rounded value are in the spec, §10.3 (`sin` 0.100 %, `cos` 0.092 %, `sincos`
  0.108 % and 0.099 %, `tan` 0.188 %, `asin` 0.077 %, `acos` 0.053 %, `atan` 0.065 % over all
  families; 0.12 to 0.25 % on image intensities and uniform angles).
- **The numbers** (`tables.py`): the 75 digits of 2/π and every constant of the method from their
  definitions, 0 differ; table `R` (sin and cos of N/128, N = 0 to 109, as correctly rounded
  double-doubles): 422 of 440 numbers, the 18 others are low parts one or two units in their last
  place away (one 39), in 17 rows; 111 other rules miss at least 17 (`--search`). glibc's table in
  `libm.so.6` equals its source text.
- **The other tables have no definition**: tan's 186 points are searched points (their `F`, `FF`,
  `G` columns are correctly rounded functions of the points, all 186); asin's and acos's 216
  subintervals hold fitted polynomials; atan's 241 rows searched points and fitted coefficients.
  So `tan`, `asin`, `acos`, `atan` are not specified: a licence question for the user.
- **The model** (`glibc-dbl-model check`, from the spec's text with `--emit`'s table): `sin`,
  `cos`, `sincos`: 768 of 768 blocks (3.2 × 10^9 arguments) and 184 of 184 special arguments per
  function agree with the record (`MODEL-PASS`, 6.6 s); with `--sse2`, 512 of 512 blocks of the
  SSE2 record. With glibc's own table in place of the definition's: the same results on the
  record's arguments and on 17 × 2^30 arguments per function next to the 17 rows that differ
  (`glibc-dbl-model tables --log2n 30`, 108 s).

## How to run it

```bash
cd validation/investigations/glibc-dbl
python -P tables.py                       # the definitions against glibc's tables (prints TABLES: ...)
python -P tables.py --emit <scratch>/R.txt
(cd blackbox && CARGO_TARGET_DIR=<scratch>/target cargo build --release --offline)
<workspace>/larmorx/scripts/orchestration/boxed.sh 8 <scratch>/target/release/glibc-dbl-blackbox \
    record --threads 28 --out glibc-dbl-2.35-x86_64-blocks.json
GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA <workspace>/larmorx/scripts/orchestration/boxed.sh 8 \
    <scratch>/target/release/glibc-dbl-blackbox record --threads 28 --only sin,cos \
    --out glibc-dbl-2.35-x86_64-sse2-blocks.json
<scratch>/target/release/glibc-dbl-blackbox list sin 3 0 10
# the spec writer's model (GPL workspace):
cd ../../../crates-gpl && cargo build --release -p glibc-dbl-model
<workspace>/larmorx/scripts/orchestration/boxed.sh 8 target/release/glibc-dbl-model check \
    --record ../validation/investigations/glibc-dbl/glibc-dbl-2.35-x86_64-blocks.json --table <scratch>/R.txt --threads 28
<workspace>/larmorx/scripts/orchestration/boxed.sh 8 target/release/glibc-dbl-model tables --table <scratch>/R.txt \
    --glibc-table <workspace>/tags/glibc-2.35/sysdeps/ieee754/dbl-64/sincostab.c --log2n 30 --threads 28
```

The record and the model are meaningful on x86-64 Linux with glibc 2.35 and a CPU with FMA and
AVX2 (the oracle machine). Memory: well under 1 GiB. `tables.py` needs `mpmath` and glibc 2.35's
source (`<workspace>/tags/glibc-2.35`); the comparison with `libm.so.6`'s bytes needs the C
library's debug symbols (`libc6-dbg`), and is skipped without them.
