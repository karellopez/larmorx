# The clean-room `sin_glibc`, `cos_glibc`, `sincos_glibc`: the proofs of glibc's bits

`larmorx_core::math`'s `sin_glibc`, `cos_glibc` and `sincos_glibc` (and `sin_glibc_sse2`,
`cos_glibc_sse2`, glibc's build for CPUs without FMA) are a **clean-room implementation**
(Apache-2.0) of [`specs/glibc-dbl.md`](../../../specs/glibc-dbl.md): glibc 2.35's double `sin`,
`cos` and `sincos` on x86-64, bit for bit, for `reproduce` only (the default everywhere stays
correctly rounded; the user's decisions of 2026-11-04, decision log "glibc's exact maths for
`reproduce`" and "GLIBC-07's tables"). The implementer worked from the spec's text, the black-box
record of `../glibc-dbl/` (its `families.rs` and the two record files) and runs of the system's
functions as a black box. glibc's source (IBM's Accurate Mathematical Library, LGPL-2.1-or-later),
the spec writer's model and `../glibc-dbl/tables.py` were never opened. Everything here is
Apache-2.0 and holds no number or line of glibc's.

| File | What |
|---|---|
| `cleanroom_tables.py` | the numbers of the spec from their definitions (mpmath): the 75 digits of 2/π, table `R` (sin and cos of N/128, N = 0 to 109, as correctly rounded double-doubles, spec §5.1, computed with 400 and with 1,000 bits: they agree), and a check of the spec's constants that have a definition (`H0`, `H1`, `G`, `M1` to `M4`, `M2L`, `T3`, `V2`, `τ`: all agree with the spec's bits); `--write` writes `crates/larmorx-core/src/math/sincos_glibc_tables.rs`, without it the file is checked (`TABLES-OK`) |
| `verify/` | the verification tool (its own cargo workspace; it includes `../glibc-dbl/families.rs` verbatim and calls the system's `libm.so.6` through `dlopen`): `record` (spec §12.1, the record's block hashes and special arguments), `fresh` (against the system's functions on fresh arguments), `hashes` (the unit tests' expected hashes, from glibc), `bench`, `list` |
| `record-2026-11-04.json` | `record` against `../glibc-dbl/glibc-dbl-2.35-x86_64-blocks.json` |
| `record-sse2-2026-11-04.json` | `record --sse2` against `../glibc-dbl/glibc-dbl-2.35-x86_64-sse2-blocks.json` |
| `fresh-2026-11-04.json` | `fresh --log2n 30` against the system's FMA builds of `sin`, `cos` and `sincos`'s single build |
| `fresh-sse2-2026-11-04.json` | `fresh --sse2 --log2n 29` with `GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA` (glibc's builds without FMA) |

## Results (2026-11-04, larmorx `d0c84e8`, main machine: glibc 2.35, x86-64 with FMA and AVX2, 28 threads)

- **The record** (spec §12.1): `sin`, `cos`, `sincos` (both outputs): **768 of 768 blocks**
  (3 × 8 families × 2^27 = 3.2 × 10^9 arguments) and **184 of 184 special arguments** per
  function agree, NaN bits included; the function hashes are the spec's (`f56e61144bf7fe10`,
  `40834730f664179a`, `0c685a3d03758470`). `RECORD-PASS` in 4.4 s.
- **Both readings of `FMA`** (spec §12.2): `sin_glibc_sse2`, `cos_glibc_sse2` (every `FMA` a
  rounded product then a rounded sum) against the record of glibc's build without FMA: 512 of 512
  blocks and 184 of 184 special arguments (hashes `1e528d6d7a721e72`, `ab86b037da025ab2`). The
  record tells the readings apart: the build without FMA against the FMA record agrees on only 18
  (`sin`) and 21 (`cos`) of 256 blocks.
- **Fresh arguments against the system's functions**: the record's 8 families at the indices
  after the record's (2^27 to 2^27 + 2^30) and three of the tool's own (±(N/128 + u) next to every
  row of table `R`, the band [0.85546875, L1) near π/2, magnitudes 2^1 to 2^27 for the medium
  reduction and its limit `L2`), 2^30 arguments each: **11,811,160,064 arguments per function,
  0 differ** (`sin`, `cos`, `sincos` both outputs; 75.8 s). With glibc's builds without FMA
  (`GLIBC_TUNABLES`), `sin_glibc_sse2` and `cos_glibc_sse2` on 2^29 per family:
  5,905,580,032 arguments each, 0 differ.
- **The unit tests** (`math::sincos_glibc::tests`, every platform in CI): the hashes of glibc's
  own results on 65,536 seeded arguments (bit patterns, [-8, 8), [-2^28, 2^28), magnitudes
  2^-30 to 2^33) for `sin`, `cos`, `sincos` and the builds without FMA; §4.1's special values; table
  `R`'s high parts against the correctly rounded `sin` and `cos` of N/128; `sincos` equal to the
  build without FMA outside [0.85546875, L1) (spec §2); and (`math::tests`) the FMA-compiled copies
  of `sin_glibc` and `cos_glibc` against the portable ones (software `mul_add`).
- **Speed** (`bench`, one thread, ns per call; 2^22 arguments per range, best of 5):

  | Range | `sin_glibc` | `cos_glibc` | `sincos_glibc` | `sin` (CORE-MATH) | `cos` (CORE-MATH) | glibc `sin` | glibc `cos` | glibc `sincos` |
  |---|---|---|---|---|---|---|---|---|
  | uniform on [-π, π) | 10.0 | 10.3 | 13.3 | 6.2 | 16.2 | 11.2 | 12.4 | 17.1 |
  | uniform on [-10^5, 10^5) | 13.4 | 13.8 | 18.2 | 6.5 | 21.2 | 13.5 | 14.2 | 21.0 |
  | log-uniform 2^27 to 2^1023 | 34.5 | 35.0 | 41.2 | 12.1 | 22.0 | 41.1 | 41.2 | 47.7 |

  Within 1.6 to 2.9 times CORE-MATH's `sin` (faster than its `cos` below 10^5) and as fast as
  glibc's own functions or faster.

What the spec's §5.3 leaves open stays open: table `R` comes from its definition, and 18 of its
low parts are not glibc's, so a result could differ from glibc's where the exact correction of the
table-driven step lies within about 2^-107 of a rounding boundary (an estimated one evaluation in
about 2^53 of those reading these rows). None was found in the 3.2 × 10^9 record arguments, the
3.5 × 10^10 fresh ones (1.2 × 10^10 per function, 2^30 per function next to the table's rows) or
the SSE2 checks. A difference found later is reported with its argument (spec §12).

## How to run it

```bash
python -P validation/investigations/cleanroom-glibc-dbl/cleanroom_tables.py   # TABLES-OK
cd validation/investigations/cleanroom-glibc-dbl/verify
CARGO_TARGET_DIR=<scratch>/verify-target cargo build --release --offline
V=<scratch>/verify-target/release/cleanroom-glibc-dbl-verify
R=../../glibc-dbl
LARMORX_COMMIT=$(git rev-parse --short HEAD) <workspace>/larmorx/scripts/orchestration/boxed.sh 4 \
    $V record --record $R/glibc-dbl-2.35-x86_64-blocks.json --threads 28 --out ../record-2026-11-04.json
LARMORX_COMMIT=$(git rev-parse --short HEAD) <workspace>/larmorx/scripts/orchestration/boxed.sh 4 \
    $V record --sse2 --record $R/glibc-dbl-2.35-x86_64-sse2-blocks.json --threads 28 --out ../record-sse2-2026-11-04.json
LARMORX_COMMIT=$(git rev-parse --short HEAD) <workspace>/larmorx/scripts/orchestration/boxed.sh 4 \
    $V fresh --log2n 30 --threads 28 --out ../fresh-2026-11-04.json
GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA LARMORX_COMMIT=$(git rev-parse --short HEAD) \
    <workspace>/larmorx/scripts/orchestration/boxed.sh 4 $V fresh --sse2 --log2n 29 --threads 28 --out ../fresh-sse2-2026-11-04.json
$V hashes; GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA $V hashes --sse2
<workspace>/larmorx/scripts/orchestration/boxed.sh 2 $V bench --log2n 22
$V list sin 3 0 10      # compare with ../glibc-dbl/blackbox's `list sin 3 0 10`
```

The comparisons with the system's functions are meaningful on x86-64 Linux with glibc 2.35 and a
CPU with FMA and AVX2 (the oracle machine); the record comparison and the unit tests on any
platform. Memory: well under 1 GiB.

## The tools' `reproduce` paths: cases where the two choices differ (relay 2b, 2026-11-04)

The clean-room originals take glibc's functions with `reproduce` and the correctly rounded ones
by default (`larmorx_core::math::Libm`). To check those paths against FSL and AFNI, the suites
need inputs where the two choices give other outputs; the existing cases had none.
`case_search.py` finds them by black-box runs of larmorx's command line in both modes:

| Search | Found (outputs that differ between the modes) | Used by |
|---|---|---|
| `fslmaths <dir>` (the suite's own inputs, `fsl_maths.own_inputs`) | `-s` / `-kernel gauss` 3.95 (1,217 voxels), 4.8 (32), 5.2 (3,030) of 110 sigmas; `-bptf 1.73 -1` (11,785) and `5.72 -1` (3,980) of a double image, `1.7` none; `-tfce_delta 0.01 -tfce 1.7 0.5 6` (2) of 18 settings; none of them named (filters) | `fsl_maths.py`: `orig/glibc-s`, `orig/glibc-kernel-gauss`, `orig/glibc-bptf`, `orig/glibc-tfce` |
| `bptf-weights` (CPython's `math.exp`, the system's glibc, against mpmath) | 62 of 1,940 sigmas (0.60 to 19.99) have a weight whose glibc `exp` is not correctly rounded, the first 1.73 at Δ = 1 | the `-bptf` sigma above |
| `tshift <dir> [n]` (the suite's float32 series, 4 slices) | about one in three random sets of slice times gives other `-wsinc5`/`-wsinc9` values (1 to 14 of 3,600), each named GLIBC-07 in both modes | `afni_tshift.py`: `methods/glibc-sinf-wsinc5`, `-wsinc9` |

The double-precision voxelwise functions need no search: on a 32³ double image uniform in
[-20, 20) (`-sin` 49 voxels, `-cos` 40, `-exp` 34) or in [0.001, 1000) (`-log` 3, `-pow 1.7` 29)
the default and `reproduce` differ, each named exactly (`orig/glibc-double-*`). The golden cases
of these paths (`cli.fsl.fslmaths.reproduce`, `cli.afni.3dTshift.wsinc`, `cli.fsl.flirt.reproduce`,
`cli.fsl.mcflirt.reproduce`, `cli.fsl.makerot.reproduce`) use the registry's own inputs, each
call checked to differ between the modes.

Not found, and why: `-dog_edge` (its output is 0 or 1: a last bit of a Gaussian practically never
moves a zero crossing; 50 settings), `-roc` (its rates are printed with six digits), fslstats'
`-e` and `-E` (six digits). topup's taps are rounded to single precision, where the two `exp`s
practically never differ.

**`-edge` calls no glibc function** (`edge_forms.py`): FSL's double `-edge` on the case
`orig/edge-double` is, in all 27,000 interior voxels, the exact squares `d·d / (v·v)` added from
z to x; glibc's `pow(d, 2)` in the spec's order gives 24,352. `specs/fslmaths.md` §10.4's
`pow(x, 2)` is compiled as `x·x` (GCC folds it), and the terms are added from the last axis. The
clean-room original follows (both modes).

```bash
python -P validation/investigations/cleanroom-glibc-dbl/case_search.py fslmaths <scratch>/cs-fm
python -P validation/investigations/cleanroom-glibc-dbl/case_search.py bptf-weights
python -P validation/investigations/cleanroom-glibc-dbl/case_search.py tshift <scratch>/cs-ts 15
python -P validation/investigations/cleanroom-glibc-dbl/edge_forms.py
```

(each inside `scripts/orchestration/boxed.sh 24`, with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`
and `LARMORX_ORACLE_CACHE_MODE=use`; a few minutes each.)
