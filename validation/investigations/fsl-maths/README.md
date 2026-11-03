# fslmaths' replica: investigation helpers

The tools behind the findings in `docs/findings/fsl-maths.md` (restricted: source-derived) and
the parity suite `fsl-maths-replica`. Run them from the repository with
`PYTHONPATH=python:validation/src` and FSL 6.0.7.23 installed; FSL runs only through the
oracle-output cache, and every run goes in a memory box
(`scripts/orchestration/boxed.sh 4 python -P ...`).

- `one_case.py <case> [fix|reproduce] [--out DIR]`: one case of the suite into `DIR/fsl` and
  `DIR/rep` (FSL's run from the cache, the replica's in the given mode), with the header fields
  and the values that differ (bits included: NaN signs, last bits).
- `tan_correctly_rounded.py <DIR>`: whether glibc's double `tan` (FSL's) is correctly rounded on
  the inputs of `fn/double-tan`: 3 of 210 values are not (2026-11-03), so neither a correctly
  rounded `tan` nor `libm`'s (11 of 210 off by one ulp) reproduces FSL in `double`; the float
  path (`fn/tan`) is identical.
- `probe_fsl.py <file>` with `probe_filters.txt`: FSL's behaviour on the edge cases of the
  kernels and spatial filters (2026-11-03): `-s 0` and `-kernel gauss 0` make every voxel NaN
  (FSL-160); kernels without voxels or of an unknown type filter with an empty kernel and print
  `ERROR:: Empty mask image` (FSL-161); `-fmedian` with NaN voxels stops with `sort(): detected
  NaN` (FSL-162); `-dilF` and `-eroF` leave NaN voxels out; `-subsamp2` of a 4D image stops with
  `3D only method called by higher-dimensional volume.` (FSL-163); `-dt int ... -kernel sphere 5
  -fmean` traps (FSL-156); `-inm` of an image of zeros prints its error and writes NaN.
- `arm_glibc_powf.py <work dir>`: whether glibc 2.35's `powf` tables (`__powf_log2_data`,
  `__exp2f_data`) are Arm's Optimized Routines v19.11 (MIT), the code `glibc_float::powf` is
  translated from, and where the C library's `powf` starts (`objdump -d` there shows the nine
  multiply-adds of the FMA build): both tables found as one block each in Ubuntu 22.04's
  libm.so.6 (2026-11-03). The function itself: `crates-nc/larmorx-nc-core/tools/libm-verify`
  (0 of 10 million differ from `powf@GLIBC_2.2.5`).

## fslstats in both D13 modes

- `stats_modes.py [pattern] [v]`: for the `fsl-stats-replica` cases whose name holds `pattern`
  (default `d13/`), runs the replica in `reproduce` and `fix` mode and prints the exit statuses,
  whether the output changes and the defects named (`v`: both outputs). It showed which cases
  reach FSL-181 to FSL-189 (`docs/findings/fsl-maths.md`).
- `goodvoxels_stats_modes.py`: fMRIPrep's four goodvoxels fslstats calls (`-M`, `-S`) on FSL's
  own chain, in both modes: the same numbers, no defect named.

Run boxed, with the replica built and the suite's inputs made:
`scripts/orchestration/boxed.sh 4 python -P validation/investigations/fsl-maths/stats_modes.py`.

## The robust range's bins (FSL-187, rule D's measurement)

- `robust_range_inputs.py`: the 272 images whose robust range FSL's programs take through
  `find_thresholds` (fMRIPrep's `bet` and `flirt -bbr` inputs of the oracle runs, BET's real
  images from larmorx-testdata, every input of the fslmaths, fslstats, FLIRT and FAST replica
  suites), one path per line. Then
  `LARMORX_ROBUST_RANGE_INPUTS=<list> cargo test --release -p larmorx-nc-core --test
  robust_range_probe -- --ignored --nocapture` (in `crates-nc`, boxed) prints each image whose
  values leave `int`'s range in a pass, whether its range moves, and the smallest synthetic
  triggers. Result (2026-11-03): 2 of 272 images reach it, both the fslstats suite's deliberate
  cases (`outl` moves, `bigcount` does not); none of fMRIPrep's; the smallest trigger is 99
  values on [0, 1) and one at 1e12 (`docs/findings/fsl-maths.md`, FSL-187).

`tan_correctly_rounded.py` is superseded by `validation/probes/glibc-double-maths` (GLIBC-07's
measurement on 10^8 arguments per function, and the check of `larmorx_core::math::tan`).

## The Python wrappers (sub-task 5b)

- `wrappers_check.py`: every typed wrapper of `lx.fsl` for fslmaths and fslstats (and the
  generic `fslmaths` and `fslstats`) against the replica's command line it stands for, on small
  seeded images: the images written, the numbers printed, `tensor_decomp`'s ten images and
  `roc`'s curve and areas, the error text of a failure. Result (2026-11-03): 56 checks, 0 differ.
  Run boxed with the replica built: `LARMORX_NC_BIN=crates-nc/target/release/larmorx-nc
  scripts/orchestration/boxed.sh 4 python -P validation/investigations/fsl-maths/wrappers_check.py`.

## Random testing (sub-task 6) and the FFT's twiddle factors (relay 6b)

- `finding_values.py <finding.json> [drawn]`: a random-testing finding's shrunk case run by FSL
  (through the cache) and by the replica in `reproduce` mode: the voxels that differ, with both
  values and their bits.
- `edge_pow_probe.py <image>`: whether FSL's `-edge` squares with glibc's `pow` or with `x * x`
  on a double image: `x * x` (GCC folds `pow(x, 2.0)`; 4,608 of 4,608 voxels against 4,606).
- `fft_twiddles.py <N> ...`: per transform size, how many of the twiddle factors glibc gives
  other than the correctly rounded value, through `sincos` (what FSL's `cexp` takes) and through
  the separate `cos` and `sin`. Result (2026-11-03): 60 points: `sin(8 k)` and `sin(57 k)` (both
  directions) through `sincos`, none through the separate functions (the FMA build on this
  machine), so the first reading, which used only the latter, missed the cause. The full
  comparison of Armadillo's own factors and transforms: `validation/probes/fsl-avwutils-lib`
  (`check_fft.py`, `check_fft_case.py`).

## The specs (sub-task 5c)

- `spec_cases.py`: every parity-suite case that `specs/fslmaths.md` and `specs/fslstats.md` cite
  in brackets (`[d13/bin-char]`, `[opt/p*]` as a prefix) exists in `fsl-maths-replica` or
  `fsl-stats-replica`, so each "observed" citation names a case that runs FSL. Result
  (2026-11-03): 38 and 15 citations, none missing. Run from the repository's root:
  `PYTHONPATH=python:validation/src python -P validation/investigations/fsl-maths/spec_cases.py .`
  (it only lists the cases; nothing runs).
