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
| `case_search.py`, `edge_forms.py` | the black-box searches behind the `reproduce` paths' suite and golden cases, and the `-edge` arithmetic probe (relay 2b, below) |
| `record_moves.py`, `record-moves-2026-11-04.jsonl` | the records' moved fits: FSL against `reproduce`, FSL against the default, `reproduce` against the default, FSL's own band (relay 2c, below) |

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

## The records with glibc's functions in `reproduce`: what moved and why (relay 2c, 2026-11-04)

The eight clean-room records the wiring reaches were made again from `be3db91` (standard tier,
the oracle cache in `use` mode) and compared with main's, case by case
(`scripts/orchestration/compare_records.py`). Only the `reproduce` comparisons and the fix passes
moved; fslstats', BET's, fslmaths', 3dTshift's and topup's existing cases did not move at all
(fslmaths' and 3dTshift's new cases, above, all pass). The records also hold cases main's
suites gained after main's records were made (FLIRT's `programs/applyxfm4D-uint8`, `-uint8-3d-spline`
and `pointflirt-no-points`, FAST's `opt/nan-first`, topup's `opt/fsl145-*`, `opt/fsl146-four-axes`,
and topup's `at/jac-z` fix pass): run with main's own code (`2e1a062`), every one of them gives
the same status and the same numbers. FAST's `reproduce` is now
FSL's values bit for bit in fMRIPrep's three calls and every single-channel case (Dice and
identical voxels 1.00000, partial volumes and probabilities 0, were 0.998 to 0.99998). FLIRT's
matrices are now FSL's text in most registrations (`real/dof6`, `real/default`, the schedules,
the verbose levels, `-cost mutualinfo`, `woods`, `normmi`), `real/cost-normmi` (1.60 mm) and
`real/weights-normmi` (10.15 mm) pass, fMRIPrep's three calls are within 0.0065 mm (were 0.0101).

A few iterative fits moved away from FSL instead, each within its threshold. `record_moves.py`
runs such a case as its suite does and also measures, for the same call, FSL against
`reproduce` (the record's number), FSL against the default (correctly rounded functions) and
`reproduce` against the default: the same program and arithmetic, only the last bits of the C
library's functions differ, so this last number is how far the fit moves with those last bits
alone. For FLIRT also FSL's own variability (the suite's band: FSL on its input with uniform
noise of ±0.5).

| Case | Threshold (mm) | FSL – `reproduce` (main) | FSL – default | `reproduce` – default | FSL's own |
|---|---|---|---|---|---|
| FLIRT `bbr/default` (and `bbr/debug`, `bbr/fmriprep-form`, `bbr/type-global_abs`) | 0.1 | 0.0693 (0.0197) | 0.0197 | 0.0651 | 0.0148 |
| FLIRT `bbr/coords` | 0.1 | 0.0040 (0.0016) | 0.0016 | 0.0033 | 0.0014 |
| FLIRT `bbr/fieldmap-mask` | 0.1 | 0.0518 (0.0386) | 0.0386 | 0.0415 | 0.0063 |
| FLIRT `bbr/fieldmap` | 0.1 | 0.0797 (0.0902) | 0.0902 | 0.0168 | 0.0785 |
| FLIRT `bbr/type-local_abs` | 0.1 | 0.0268 (0.3647) | 0.3647 | 0.3694 | 0.9038 |
| FLIRT `bbr/slope` | 0.1 | 0.0063 (0.0183) | 0.0183 | 0.0211 | 0.0132 |
| FLIRT `bbr/weights` | 0.1 | 0.0035 (0.0060) | 0.0060 | 0.0029 | 0.0042 |
| FLIRT `real/bins64` | 0.5 | 0.0692 (0.0601) | 0.0601 | 0.0984 | 0.0896 |
| FLIRT `real/searchr` | 0.5 | 0.1349 (0.0330) | 0.0330 | 0.1463 | 0.0182 |
| FLIRT `real/weights` | 0.5 | 0.3059 (0.2100) | 0.2100 | 0.4251 | 17.6579 |
| FLIRT `real/cost-normcorr` | 0.5 | 0.0118 (0.0126) | 0.0126 | 0.0034 | 0.0123 |
| FLIRT `real/cost-labeldiff` | 0.5 | 1.2259 (1.2260) | 1.2260 | 0.0003 | 54.5916 |
| FLIRT `real/weights-normcorr` (fails, as on main) | 0.1 | 0.5083 (0.5785) | 0.5785 | 0.1273 | 0.1879 |
| FLIRT `real/cost-leastsq` (fails, as on main) | 0.5 | 82.0821 (81.8069) | 81.8069 | 0.5746 | 46.7560 |
| mcflirt `real/ds000005/opt-cost-normmi` | 0.1 | 0.0126 (0.0108) | 0.0108 | 0.0109 | |
| mcflirt `real/ds000005/opt-cost-mutualinfo` | 0.1 | 0.0047 (0.0048) | 0.0048 | 0.0048 | |

(FLIRT: the 95th-percentile displacement over the reference's brain; mcflirt: the 95th percentile
over the volumes of the RMS deviation, radius 80 mm.) In every row the default is main's
`reproduce` to the last digit, as it should be: the default is untouched. And in every row where
the record moved away from FSL, `reproduce` – default is of the size of the move: these fits end
where the last bits of their arithmetic put them. With glibc's functions the C library's last bits
are now FSL's, but the rest of the clean-room arithmetic (BBR's boundary points and normals and
the order of its sums, the costs' accumulations, the interpolation) is not FSL's bit for bit, so
the paths still part and their end points land on either side, within the cost's flat minimum.
The default, what users get, stays where it was (FSL – default = main's numbers).

```bash
python -P validation/investigations/cleanroom-glibc-dbl/record_moves.py flirt bbr/default bbr/coords \
    bbr/fieldmap-mask bbr/fieldmap bbr/type-local_abs bbr/slope bbr/weights real/bins64 real/searchr \
    real/weights real/cost-leastsq real/weights-normcorr real/cost-labeldiff real/cost-normcorr
python -P validation/investigations/cleanroom-glibc-dbl/record_moves.py hmc \
    real/ds000005/opt-cost-normmi real/ds000005/opt-cost-mutualinfo
```

(inside `scripts/orchestration/boxed.sh 24`, with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`
and `LARMORX_ORACLE_CACHE_MODE=use`: about 9 minutes for FLIRT's, whose bands run FSL on noisy
inputs, 1.5 for mcflirt's; the output, one JSON line per case, is `record-moves-2026-11-04.jsonl`.)

## The site's evidence after the change (relay 3, 2026-11-04)

The documentation site's pages of FLIRT, FAST, topup, fslmaths, fslstats, 3dTshift, the head-motion
correction and BET were exported again from this branch. What was computed again, and the two checks
behind it:

- `refresh_original` (in `larmorx_validation.evidence.fsl_flirt`, `fsl_fast`, `fsl_topup`;
  `fsl_maths_original.refresh` for fslmaths and fslstats; run as `python -m
  larmorx_validation.evidence.<module> refresh-original "<why>"`): the clean-room original's runs of
  every example entry of the figures cache made again in their working directories, FSL's runs kept
  (FSL with correctly rounded maths, FSL on a perturbed input), and the original's case entries
  (`cases-original`, `original-cases-default-mode`) computed again with FSL served by the
  oracle-output cache; each entry records why (`rerun`, `recomputed`). fslmaths' and fslstats'
  entries only where glibc's functions reach their calls (`GLIBC_REACHED`: `-s`, `-bptf`, `-tfce`,
  the double functions, `-e`, `-E`); combine_masks and fslstats' goodvoxels calls use none.
- `evidence_unchanged.py`: the entries not computed again (3dTshift's two examples, Fourier
  interpolation; the head-motion correction's two, the normalised correlation; BET's three) checked
  instead: each original run made again from its recorded command by this build, into a scratch
  directory, its outputs compared with the entry's byte for byte after decompression. Result
  (`evidence-unchanged-2026-11-04.json`): 19 of 19 runs give the same bytes.
- `site_inventory.py`: the site's inventory page by page (every built page's words, headings,
  paragraphs, tables and rows, images, figures, links, viewers and numbers carrying a JSON pointer;
  every evidence file's lists and mappings, examples, cases, tables, leaves and blobs), before and
  after an export, and the counts that fell (`--compare`), for the rule that the site's evidence only
  ever improves.
