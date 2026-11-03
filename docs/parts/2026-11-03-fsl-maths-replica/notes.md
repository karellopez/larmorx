# NOTES: fslmaths and fslstats replica (relay notes)

Worktree: <workspace>/larmorx/.claude/worktrees/agent-a0c92dea6a2eec1a2
(branch `worktree-agent-a0c92dea6a2eec1a2`, from main 3a15db3 with topup's replica merged)
Every command runs there (`cd` first); never touch the main checkout.

## How to work here (relay 1's setup, reuse it)
- The sandbox refuses `source`, `cd x && ...` chains and multi-heredoc commands: run things
  through `<scratch>/run.sh <cmd...>` (env.sh's settings, literal; `RUN_IN=crates-nc` to run
  in a subdirectory), and write helper files with the Write tool.
- Build: `RUN_IN=crates-nc bash <scratch>/run.sh cargo build -j 14 --release -p larmorx-nc-cli`
  (the build-cache seed is in place: `crates-nc/target` copied from the main checkout).
  `python/larmorx/_core.abi3.so` is the main checkout's copy (main's build; rebuild with
  maturin into `<scratch>/wheel` only if the root workspace's Rust changes matter).
- Suite: `bash <scratch>/parity.sh <tier> [case-substring]` runs `fsl-maths-replica` boxed
  (8 GiB, `--jobs 8`, cache `use`) and prints the summary and every non-pass
  (`<scratch>/py/failures.py`). One case into `<scratch>/case/{fsl,rep}` with the differing
  header fields and values: `bash <scratch>/run.sh scripts/orchestration/boxed.sh 4 python -P
  <scratch>/py/onecase.py <case> [fix|reproduce]` (FSL through the cache).
- Other replicas' smoke tiers (FSL from the cache): `<scratch>/others.sh` (BET's is
  `parity fsl-bet --implementation replica`); log in `oracles/logs/fsl-maths-1-*.log`.

## Summary for the next relays (porting guide §1, D13, §11; the models)
- Role: replica author and spec writer (FSL 6.0.7.23's source: `~/fsl-6.0.7.23/src/`). Never
  edit `crates/larmorx-fsl`, `-mri`, `-afni`. Specs, `docs/api/`, `larmorx_core::defects`
  texts, next-steps, triage, the site and the register's summary rows describe behaviour only.
- D13: every defect gets a `reproduce` switch (one switch: `LARMORX_UPSTREAM_DEFECTS`);
  (a) crash/UB: a defined result or a refusal (crashes refused in both modes; what FSL defines,
  e.g. an abort with its text, reproduced); (c) deterministic bug: fixed by default, notice
  only where the result differs (`Notices`, printed as `larmorx-nc: upstream defect ...`);
  undefined float-to-int conversions clamp, NaN to 0. Rules A-D decide questionable ones.
  The parity suite runs `reproduce` bit for bit, then a `fix` pass (`fix_pass`: a defect named
  exactly when the outputs differ; `silent_ok` for notices without a visible change).
- Contract (§11, sub-task 5): spec, Rust API, Python wrapper + `.pyi`, CLI, API page with the
  option table and D13 section, findings + register rows (handoff), parity suite + record +
  `validation/tools.toml` line (handoff), PROVENANCE, golden cases (`crates-nc/larmorx-nc-cli/
  tests/golden.rs`: `BetCase` entries, recorded by `LARMORX_GOLDEN_RECORD=1` from a clean
  commit; `every_tool_has_a_golden_case` FAILS now: fslmaths has none yet), memory test
  (`tests/parity/test_memory_peak.py`), difftest descriptor, figures/site page.
- Models: `crates-nc/larmorx-nc-topup` (crate layout, `Console`, cli), the FLIRT replica suite
  (`parity/fsl_flirt_replica.py`: `compare`, `fix_pass`, `run_replica` are reused by the new
  suite), `docs/parts/2026-11-03-fsl-topup-replica/notes.md`.

## Relay 1 (sub-task 1: fslmaths' core and fMRIPrep's call) - DONE
Commits: 1b0fff9 (WIP: crate, core modules, defects), 7190ee8 (suite, FSL-152/153 attribution,
the core's -mas on volops, findings, provenance, unit tests, helpers).
- **Core** (`larmorx-nc-core`): `volops.rs` (newimage's arithmetic for char/short/int/float/
  double: `Arith`; `op_scalar`, `op_volume` with `samesize SUBSET` and cyclic sources,
  `binarise`, `threshold`, `extrema_masked`, `extrema_without_nan`, `no_mask_voxels`,
  `robust_limits_masked`, generic `find_thresholds` (with mask); `newimagevox2mm_mat`,
  `samesize_subset`, `abs_value`); `cprob.rs` (`Drand` drand/sdrand, `ndtri`, `mtherr`).
  `Volume::robust_limits` now delegates to the generic `find_thresholds` (identical to the old
  float version on 400 random images before the switch); `avwutils::fslmaths_mas` now uses
  `volops` (`binarise`, `op_volume`): no duplicate of `-mas`. BET, FLIRT and FAST replica smoke
  tiers re-run after the change: no failure (BET 27 pass, 24 both-error, 3 expected-divergence;
  FLIRT 18 + 4; FAST 2) — the BET re-run after the `-mas` refactor: see below.
  `fsl-source/fsl-cprob/` added (drand.cc, ndtri.cc, polevl.cc, mtherr.cc, const.cc, mconf.h,
  cprob.h; `scripts/fsl_source.py` VERSIONS/FILES; MANIFEST regenerated).
- **Crate** `crates-nc/larmorx-nc-avwutils` (`fslmaths.rs`, `cli.rs`, `usage.rs`, `lib.rs`;
  registered in `crates-nc/Cargo.toml`, `larmorx-nc-cli` (`fsl fslmaths`), libm added to the
  workspace deps for `tan`). Implemented: main (usage, -h/--help, -dt/-datatype incl. `input`,
  -odt/-output_datatype, messages, statuses), inputParser<T> for the five types, loadNewImage
  (3D cloned over 4D, orientation warning), `-add -sub -mul -div -rem -mas -max -min` (number
  and image), `-thr -uthr -thrp -thrP -uthrp -uthrP`, `-exp -log -sin -cos -tan -asin -acos
  -atan -sqr -sqrt -recip -abs -bin -binv -nan -nanm -index -grid -rand -randn -seed -range
  -save -restart`, the hidden `-pow` and `-invbin`; the output rounding and `save_volume_dtype`.
  Not yet (refused with "is not translated yet in this build"): everything of sub-tasks 2-3
  (list `NOT_YET` in fslmaths.rs, and every `-<Uppercase>` dimension op).
- **Defects** in `larmorx_core::defects` (code): FSL-151, 152, 153, 155, 156, 158, 159, all
  with fix + reproduce. FSL-154 (-dt char/-dt input on uint8: 128-255 read as negative) and
  FSL-157 (-odt input on uint16/int8...: truncated not rounded; int8 written as uint8) are
  DECIDED (rule A) but NOT yet implemented (no entry in defects.rs yet): do them in sub-task 5
  or 6 (FSL-154 needs an unsigned 8-bit element type, or u8 handling at read time).
  Details and cases: `docs/findings/fsl-maths.md` (restricted).
- **Suite** `fsl-maths-replica` (`validation/src/larmorx_validation/parity/fsl_maths_replica.py`,
  registered in `__main__.py`'s PARITY_MODULES; NOT yet in `tables.py`, `tools.toml`,
  `tests/parity/test_fsl_maths_replica.py`: sub-task 5/6). Inputs made once in
  `<workspace>/oracles/inputs/fsl-maths-replica/` (seeded; `_save` writes only missing files,
  marker `v2.done`: add new inputs at the end of `_make_inputs`). Standard tier: 141 cases
  (4 fMRIPrep combine_masks calls: ds000005, ds005040 x2 kinds, ds003592; the rest synthetic):
  RESULT at the end of relay 1: see "State" below.
- **Known difference**: `fn/double-tan` (11 of 210 values 1 ulp off in double; glibc's tan is
  not correctly rounded, the replica uses libm's). Float `-tan` identical. Decide in sub-task 5
  or 6 (port glibc's tan — LGPL, which package? — or document a double-only tolerance).

## Relay 2 (sub-task 2: kernels, morphology and spatial filters) - DONE
Commits: d427936 (WIP: filters module, operations, FSL-160..165), 2b1eeb8 (-tensor_decomp,
FSL-166, suite's `fixed` check), f1188b5 (FFT path, findings, PROVENANCE, NOTICE, fsl-source),
85d0483 (probe helper in `validation/investigations/fsl-maths/`).
- **Core** (`larmorx-nc-core`): new `filters.rs` (generic over `Arith`: `Kern` (kernel with FSL's
  sizes, empty loops for sizes <= 0), `box_kernel_vox/_mm`, `gaussian_kernel3d_mm` (reports
  zero width), `spherical_kernel`, `gaussian_kernel2d`; `morphfilter` + `morph_messages` +
  `MorphIssues`; `dilall`; `generic_convolve` (separable masked passes, or normalised kernel by
  `convolve_plain` or the FFT, renormalised by a volume of ones; `ConvStop::{Trap,Exception}`),
  `uses_fft`, `fsllog2`; `subsample_by_2` (any T, any volume t), `subsample_reads_outside`;
  `edge_strengthen`; `log_edge_detect` (`every_volume` switch); `connected_components` (exact
  two-pass with the float-rounded parent map; sub-task 3's -tfce can reuse it), `fill_holes`
  (`planar` switch), `fill_holes_traps`; `masked_mean` (calculateSums' blocks, generic);
  Armadillo's `arma_max`/`arma_min` (two partials), `arma_sort` (NaN refused)); new `kissfft.rs`
  (Armadillo 14.2.1's Kiss FFT engine, `fft3`, `fft_convolve`; twiddles from correctly rounded
  cos/sin, GLIBC-07 territory, all 8 FFT cases identical). `avwutils::dilate_f` (BET's -dilF) now
  runs `filters::morphfilter` (Armadillo's max: NaN left out; the old code kept a first NaN):
  BET replica smoke tier unchanged (27 pass, 24 both-error, 3 expected-divergence).
- **fslmaths.rs**: kernel state (`kernel`, `separable`, `zero_width`), `-kernel` (all types),
  `-s`, `-fmean`, `-fmeanu`, `-dilM -dilD -dilF -ero -eroF -fmedian`, `-dilall`, `-subsamp2`,
  `-subsamp2offc`, `-edge`, `-dog_edge`, `-fillh`, `-fillh26`, `-inm`, `-ing`, `-tensor_decomp`
  (arma::eig_sym = dsyevd; ten outputs `<out>_L1..._V3`, then FA). Helpers `no_voxels` (FSL-161
  rule-B error), `convolve`, `morph`, `subsample`, `tensor_decomp`. `NOT_YET` now holds only
  sub-task 3's names (`-roc -pval -pval0 -cpval -ztop -ptoz -rank -ranknorm -tfce_minT
  -tfce_delta -tfce -tfceS -roi -bptf`) plus every `-<Uppercase>` op.
- **Defects** (in `larmorx_core::defects`, fix + reproduce, all checked by the suite):
  FSL-160 (zero-width Gaussian -> NaN; rule A: impulse in the convolutions only), FSL-161 (kernel
  without voxels / unknown type / -s<0 / -dog_edge radius<0; rule B: error), FSL-162 (-fmedian,
  -dilD sort a NaN -> logic_error; rule A: NaN left out), FSL-163 (-subsamp2(offc) of 4D ->
  exception #75 or first volume only; rule A: every volume), FSL-164 (-dog_edge 4D filters
  volume 0 only; rule A), FSL-165 (-fillh of a size-1 axis -> SIGFPE; rule A: planar edges;
  reproduce refuses), FSL-166 (-tensor_decomp of a NaN/inf tensor -> out_of_range; rule A: voxel
  0). Register rows: through the handoff in sub-task 7 (texts: `docs/findings/fsl-maths.md`).
  IDs used so far: FSL-151..166 (FSL-154, 157 decided, not implemented: see relay 1).
- **Suite**: inputs marker now `v5.done` (new inputs appended at the end of `_make_inputs`:
  sparse3, sparsenan3, lab3, blob3, odd3, two2d, kern3, kern_even, kern_inf, kern4d, kern_inf2,
  even4, holes3, blob2d, tensor, tensornan; add new ones at the end and bump the marker).
  `_filter_cases()` holds sub-task 2's 110 cases (morph/, dilall/, conv/, sub/, edge/, dog/,
  fillh/, norm/, tensor/, d13/). New `Run.fixed` field: where FSL stops on a defect the default
  corrects, the default must exit 0, write an output and name the defect.
- **Not defects** (documented): off-centre warnings; -inm of an all-zero image (FSL's error, NaN);
  a +inf kernel value (Armadillo's out_of_range, both modes).
- **For the orchestrator / later**: `arma::direct_max` in arma.rs is one running max; Armadillo
  14.2.1 uses two partials (differs only in the sign of a zero max). Not changed (other
  replicas use it); `filters::arma_max` follows 14.2.1. Noted in the findings.

## Relay 3 (sub-task 3 split: 3a DONE here; 3b next)
Commits: 786fb15 (arma direct_max/direct_min, cprob ndtr/erf/erfc/expx2, ndtri Q2 fix, the
library probe), a5f45e2 (the operations, vstats, bptf, FSL-167..172, suite cases, goodvoxels),
4463472 (findings, PROVENANCE, docs).
- **Orchestrator's direct_max item: DONE.** `arma::direct_max` now two partials as Armadillo
  14.2.1 (`op_max_meat.hpp`), new `arma::direct_min`; `filters::arma_max/arma_min` removed,
  `morphfilter` calls the arma ones. Checked vs Armadillo 14.2.1 (`arma::max(vec)`, `Mat::max()`,
  min likewise) in `validation/probes/fsl-avwutils-lib` (534 vectors: signed zeros both orders,
  NaN, inf, ties): 0 of 2,136 lines differ. Callers: `Mat::max_abs` (FLIRT `run.rs:1057`, mcflirt
  `run.rs:264`, warpfns `warpfns.rs:197,799,800` -> applywarp/topup/applytopup) and
  `filters::morphfilter` (fslmaths -dilF/-eroF/-ero, BET's -dilF). max_abs works on |x| (zeros
  all +0), so those results cannot move; morphfilter's body is unchanged from relay 2's
  arma_max. Sub-task 6's re-made records confirm (compare_records).
- **Core**: `cprob.rs` + `ndtr`, `erf`, `erfc`, `erfce`, `expx2`, `MAXLOG`, `SQRTH` (UNK
  constants verified by script against ndtr.cc; NaN of `y*sqrt(z)` in FSL's register order via
  `sse::mul_f64`); **relay 1's `ndtri` had a mistyped digit in `Q2[1]` (fixed: far tails only,
  `sqrt(-2 log y) >= 8`)**. New `vstats.rs` (newimage's whole-volume statistics for every T:
  `sums` (calculateSums' blocks), `mean`, `variance`/`stddev` (`corrected` = FSL-167's masked
  form), `variance_defect`, `extrema` / `extrema_without_nan` with indices, `percentile`,
  `percentile_index`) - **sub-task 4 (fslstats) should build on it** (it has no mask yet: add
  the masked forms there; `stats.rs` has FAST's float-only masked ones). `filters.rs` +
  `bandpass_temporal_filter`, `bptf_half_widths`, `bptf_stop` (weights computed lazily for the
  offsets reached; wrapping int arithmetic as C).
  `fsl-source/fsl-cprob/ndtr.cc`, `expx2.cc` added (MANIFEST 145 files).
- **Probe** `validation/probes/fsl-avwutils-lib/` (probe.cc, build.sh, check.py, README;
  Rust side `crates-nc/larmorx-nc-core/tests/avwutils_probe.rs`, ignored test): Armadillo max/min
  and fsl-cprob's ndtr/ndtri/erf/erfc vs `libfsl-cprob.so`: 180,890 arguments, 0 differ. Build:
  `PATH=/usr/bin:$PATH bash validation/probes/fsl-avwutils-lib/build.sh <scratch>/avw_probe`
  (FSL's g++ is on the normal PATH; /usr/bin/g++ is the system one), then boxed check.py.
- **fslmaths** (`src/fslmaths/stats.rs`, child module of fslmaths.rs: `use super::*`):
  reductions `-T/-X/-Y/-Z` x `max min mean std maxn median perc ar1` (other capitals: nsize 1;
  unknown name: message + exit 1), `-pval -pval0 -cpval -ztop -ptoz -rank -ranknorm -roi -bptf`.
  `NOT_YET` now: `-roc -tfce_minT -tfce_delta -tfce -tfceS`.
- **Defects** (in `larmorx_core::defects`, all with fix + reproduce, checked by the suite):
  FSL-153 text extended (NaN first value along a dimension: -Tmax/-Tmin/-Tmaxn), FSL-162
  extended (-rank/-ranknorm NaN -> sort exception; fix: rank the non-NaN), FSL-167 (unmasked
  variance: no clamp, n=1 -> NaN; fix = masked form), FSL-168 (percentiles with NaN: std::sort
  arbitrary / outside -> refuse; fix: NaN left out), FSL-169 (NaN stats in -pval/-pval0/-cpval;
  fix: NaN p, NaN perms left out), FSL-170 (-ztop of +-inf -> NaN; fix 0/1), FSL-171 (-bptf
  sigma beyond int: segfault / bad_array_new_length; rule B error by default), FSL-172 (-ptoz
  p < ~1.1e-16: inf + "ndtri domain error"; fix -ndtri(p)). **IDs used: FSL-151..172** (154 and
  157 decided, not implemented: see relay 1). Not defects: -Tar1 int overflow (every term
  truncates to 0 either way), -bptf nan (skips), weights' t*t wrap (only series > 46,341).
- **Suite**: inputs marker `v6.done` (ts, tsc, tsd, tss, tsi, tsvar, stat, stat0, statnan,
  statd, stats16, zmap, zinf, pmap, ptiny appended); `_dim_cases()` (dim/, stat/, roi/, bptf/,
  d13/ new ones); **goodvoxels**: `GOODVOXELS_STEPS` (fMRIPrep's 14 fslmaths + 4 fslstats calls
  in workflow order, nipype's formats), `_goodvoxels_chain(run)` makes FSL's chain once
  (locked; `oracles/inputs/fsl-maths-replica/goodvoxels/<run>/`, `values.json` = fslstats'
  numbers: cov_ribbon_mean 0.010577, mod_ribbon_mean 0.95208, mod_ribbon_std 3.6842,
  upper_thresh 2.79418), ribbon stand-in = dseg label 1 (GM) at each BOLD voxel centre on
  ds005040's space-T1w BOLD (50x62x44x63); cases `fmriprep-goodvoxels/<run>/<node>` (first one
  smoke): **14/14 bit-identical**. The 4 fslstats calls are run (FSL, cache) but not compared:
  **sub-task 4 adds their comparison** (reuse `_stats_run`, `GOODVOXELS_STEPS`).

## State at the end of relay 3 (2026-11-03, commit 4463472, tree clean)
- `fsl-maths-replica` standard tier: **358 cases: 312 pass, 45 both-error, 1 fail**
  (`fn/double-tan`, the known double last bit; the orchestrator's GLIBC-07 plan applies).
- Gates run on the changed crates: `cargo fmt --all --check` (crates-nc and root) clean;
  `cargo clippy -p larmorx-nc-core -p larmorx-nc-avwutils -p larmorx-nc-cli --all-targets
  --locked -D warnings` clean; `cargo test -p larmorx-nc-core -p larmorx-nc-avwutils` pass;
  `cargo test -p larmorx-core defects` pass; ruff check/format, check_spdx,
  `scripts/fsl_source.py --check` clean. Not run: full gates, smoke tier, records (sub-task 6);
  other replicas' smoke tiers were not re-run (the direct_max change cannot move their results;
  sub-task 6's records check it). `every_tool_has_a_golden_case` still fails (until sub-task 5).
- `python/larmorx/_core.abi3.so` is still main's copy (rebuild before the Python wrapper tests:
  `larmorx_core::defects` has new entries FSL-167..172).

## Relay 4 (sub-task 3b: -tfce, -tfceS, -tfce_minT, -tfce_delta, -roc) - DONE
Commits: 44897c8 (code, defects, suite cases, powf probe script), b773812 (findings, PROVENANCE,
READMEs). Tree clean.
- **Core**: `glibc_float::powf` (Arm's Optimized Routines v19.11 `powf.c` + `powf_log2_data.c`,
  translated; the nine FMAs of `__powf_fma` read from libm's machine code; tables found in
  libm.so.6 by `validation/investigations/fsl-maths/arm_glibc_powf.py`; `tools/libm-verify` now
  has a `powf` family: **0 of 10 million differ** from `powf@GLIBC_2.2.5`; the compat wrapper
  changes no value). `libm-verify/glibc_reference.c` now binds the `GLIBC_2.2.5` symbols itself
  (`.symver`): FSL's conda gcc on this machine bound `exp@GLIBC_2.29`, `powf@GLIBC_2.27` ...
  (the README's earlier claim no longer held); all older families still 0 differ.
  `miscmaths::mean_col`, `var_col`, `interp1`, `percentile_col`, `quantile_col` (miscmaths.cc
  2412.6; `-roc`; sub-task 4's fslstats may not need them).
- **fslmaths**: new child module `src/fslmaths/tfce.rs` (`TfceParams`, `tfce`, `tfce_support`,
  `roc`); `stats::nifti_to_newimage_vox` factored out of `-roi` (shared with `-tfceS`);
  `NOT_YET`/`not_yet` removed: **every fslmaths operation is translated now**.
  -tfce's runtime_errors are caught by fslmaths (`Exception` = NEWMAT's = std::runtime_error):
  "ERROR: TFCE failed, ...values." + what + "\n\nExiting", exit 1. armawrap index errors are
  `std::out_of_range` ("Mat::operator(): index out of bounds"); an index into an EMPTY
  ColumnVector divides by zero (`(i-1)/Ncols()`, SIGFPE: refused).
- **Defects** (in `larmorx_core::defects`; fix + reproduce; findings in `docs/findings/fsl-maths.md`):
  FSL-153 extended (-tfce max, -roc min/max), FSL-173 (tfce's read past its sorted list: no
  effect, never named), FSL-174 (a step that cannot advance: rule B error; reproduce: -tfce
  refused (hang), -tfceS FSL's out_of_range), FSL-175 (-tfce auto step of an image <= 0: zeros,
  rule A), FSL-176 (connectivity not 6/18/26: rule B), FSL-177 (-tfceS's arrays: rule A; empty
  array SIGFPE refused), FSL-178 (-tfceS voxel outside: rule B; reproduce simulates in-volume
  offsets, refuses beyond), FSL-179 (-roc noise of another size: rule B; reproduce in-buffer
  simulation, refuses smaller), FSL-180 (-roc without true/noise voxels: rule B; reproduce FSL's
  NaN curve + sort exception). **IDs used: FSL-151..180** (154, 157 decided, not implemented).
- **Suite**: inputs marker `v7.done` (tfz, tfz4, tfd, tfs, tfzero, tfneg, tfnan, tfinf, tfsmall,
  roctruth, rocim, rocimd, rocims, rocim3, rocnan, rocnoise, rocnoise4, rocnoise2, roctruthneg,
  roctruthones, roctruthsmall appended); `_tfce_cases()` (48 cases: tfce/, tfceS/, roc/, d13/).

## State at the end of relay 4 (2026-11-03, commit b773812, tree clean) [superseded: see relay 5]
- `fsl-maths-replica` standard tier (run before the last lint-only change; the 48 new cases
  re-run after it, same result): **406 cases: 348 pass, 57 both-error, 1 fail** (`fn/double-tan`,
  the known double last bit; GLIBC-07 plan below). New cases: 36 pass, 12 both-error.
- Gates run on the changed crates: `cargo fmt --all --check` (crates-nc and root) clean; clippy
  `-p larmorx-nc-core -p larmorx-nc-avwutils -p larmorx-nc-cli -p libm-verify --all-targets
  --locked -D warnings` and `-p larmorx-core` clean; `cargo test -p larmorx-nc-core -p
  larmorx-nc-avwutils` and `-p larmorx-core defects` pass; ruff check/format, check_spdx,
  `fsl_source.py --check` (145 files) clean. Not run: full gates, smoke tier, records (sub-task 6).
- `python/larmorx/_core.abi3.so` is still main's copy (rebuild before the Python wrapper tests:
  `larmorx_core::defects` has new entries FSL-167..180).
- **For the orchestrator (relay 4):** `bfmatrix.rs:44` and `spmat.rs:922` (topup's) name
  Armadillo's index error `std::logic_error`; Armadillo 14.2.1's `arma_stop_bounds_error` throws
  `std::out_of_range` (relay 2's FSL-166 and this relay's -tfceS cases, checked against FSL's
  stderr). If those paths ever reach an uncaught terminate message, the type name would differ.
  Not changed here (topup's code).

## Relay 5 (sub-task 4: fslstats) - DONE
Commits: b51141c (program, json, vstats masked forms, defects, suite, probe), 99d1de3 (findings,
PROVENANCE, JSON compared as printed, 2 cases), 502f1d6 + 2856e5d (investigation helpers). Tree
clean.
- **Orchestrator's item DONE**: `bfmatrix.rs` and `spmat.rs` `index_error()` now
  `std::out_of_range` (Armadillo 14.2.1's `arma_stop_bounds_error`), unit tests in both
  (`an_index_out_of_range_is_armadillos_out_of_range`), PROVENANCE's "Errors are values" line.
  sub-task 6 re-makes `fsl-topup-replica` (expected identical: no suite case reached it).
- **Program in the core**: `larmorx_nc_core::fslstats` (`main(argv, &Run, out, err) -> u8`,
  `USAGE`), because BET runs it: `avwutils::fslstats_cog` now calls `fslstats::main(["fslstats",
  in, "-l", t, "-C"])` (BET replica smoke tier unchanged: 27 pass, 24 both-error, 3
  expected-divergence). The avwutils crate's `cli::fslstats` wraps it; `larmorx-nc fsl fslstats`
  registered (larmorx-nc-cli, its tool list text too). Every option of fslstats.cc incl. hidden
  `-D`; -t/-K/-json; JSON notices deferred per key (a replaced key names nothing).
- **Core additions**: `json.rs` (nlohmann 3.12.0 Grisu2 `to_chars` + `Value` + `dump(4)`; MIT
  header; probe `validation/probes/nlohmann-to-chars` + ignored test `tests/json_probe.rs`:
  0 of 1,897,170 differ); `vstats` masked forms (`in_mask`, `count_masked`, `sums_where`/`_masked`,
  `mean_*`, `variance_*`, `extrema_masked`, `percentile_values`, `image_histogram` with
  `clamp_bins`/`skip_nan`).
- **Defects FSL-181..189** (in `larmorx_core::defects`, tool "fslstats"; texts in
  `docs/findings/fsl-maths.md`, now "fslmaths and fslstats"): 181 NaN in order stats (A),
  182 -s/-S without mask (A), 183 -t with 4D -k/-d (A), 184 -x/-X empty mask SIGSEGV (refused
  both modes), 185 -e/-E empty robust range SIGFPE (A: entropy 0; reproduce refuses), 186 -H
  max<=min (B: error), 187 histogram bin beyond int (A; find_thresholds left as FSL's: rule D
  noted for the orchestrator, shared with BET/FAST/fslmaths), 188 -w empty 4D time range (A),
  189 -json -v/-V count as float (A). IDs used: FSL-151..189 (FSL-190 left; 191-200 allowed).
- **Suite `fsl-stats-replica`** (`parity/fsl_stats_replica.py`, in `__main__.py`; NOT yet in
  tables.py/tools.toml/tests/parity: sub-task 5/6). Inputs `oracles/inputs/fsl-stats-replica/`
  (marker `v2.done`; append new inputs at the end of `_make_inputs` and bump the marker).
  Cases: FSL's own `feedsRun` (options parsed from the script; text + JSON against FSL's
  expected files: all match), goodvoxels' 4 calls (FSL's chain from fsl-maths-replica; identical
  in both modes, no defect named), BET's `-l 0.001 -C` on ds005040's T1w, ~140 synthetic.
  **Standard tier: 205 cases: 170 pass, 35 both-error, 0 fail.** Smoke 3. Runner:
  `SUITE=fsl-stats-replica bash <scratch>/parity2.sh <tier>`. Killed-FSL cases (184, 185) are
  checked by `_crash_fix` (the default refuses or corrects, naming the defect).
- Helpers: `validation/investigations/fsl-maths/stats_modes.py` (both modes per case),
  `goodvoxels_stats_modes.py`.
- Gates run: fmt (nc, root), clippy (-p larmorx-nc-core -p larmorx-nc-avwutils -p larmorx-nc-cli,
  and larmorx-core) clean; `cargo test` core/avwutils/bet and larmorx-core defects pass; ruff,
  check_spdx clean; fsl-maths-replica smoke 18 pass + 1 both-error. Not run: full gates, smoke
  tier, records. `python/larmorx/_core.abi3.so` still main's (defects FSL-167..189 new).

## Relay 6 (sub-task 5 split: 5a DONE here; 5b next, then 5c)
Commits: d83abf2 (tan, GLIBC-07, FSL-187 in the robust range), 1b5a617 (merge of main: ccd50d0,
3dd05c7), d4e9935 (Windows-aware separator in the core), ed31e33 (FSL-154, FSL-157). Tree clean.
- **Double maths (orchestrator's ~09:10 item): DONE.** CORE-MATH's `tan` ported:
  `crates/larmorx-core/src/math/tan.rs` (uses cos.rs's tables and dint helpers, now `pub(super)`;
  `math::tan` dispatched with an FMA copy; unit tests incl. the two exceptions). Verified vs
  CORE-MATH's C: `validation/probes/glibc-double-maths` (own cargo workspace; `build.sh <out>
  <workspace>/tags/core-math`; `coremath` mode): all 1,077,353 tan.wc cases at ±x + 2^30 random,
  and atan2(x,1) vs cr_atan on atan.wc: 0 differ. **Measured** glibc 2.35 vs correctly rounded
  (`measure --log2n 26`, 6.7e7 per family): double results differ sin 0.12 %, cos 0.085 %, tan
  0.21 %, atan 0.083 %, asin 0.16 %, acos 0.071 %, exp 0.076 %, log 0.021 %, pow 0.083 %;
  **0 float results differ** (default -dt float unaffected). Table in the probe README and
  `docs/findings/fsl-maths.md` "Known differences". fslmaths' `-sin -cos -tan -asin -acos -atan
  -exp -log -pow`: correctly rounded by default; `reproduce` = glibc's bits for exp/log/pow
  (glibc_double), CR for the six IBM ones in both modes; GLIBC-07 named where glibc's bits change
  a stored value (exp/log/pow only). `GLIBC_07` added to `larmorx_core::defects`. libm crate
  dropped from crates-nc. Suite: `Run.glibc` (max ulps) -> `fn/double-*` report
  `expected-divergence` (GLIBC-07 with counts) when FSL differs only in the last place;
  `fn/double-tan`: 3 of 210 voxels, 1 ulp. Internal uses (kernels' exp, cprob, -e's log) keep
  glibc's bits (unchanged).
- **find_thresholds bin range (orchestrator's ~15:50 item): DONE.** Measured first (rule D):
  `crates-nc/larmorx-nc-core/tests/robust_range_probe.rs` (ignored test) on 272 images listed by
  `validation/investigations/fsl-maths/robust_range_inputs.py` (66 fMRIPrep bet/flirt -bbr
  inputs, 13 BET real images, all fslmaths/fslstats/FLIRT/FAST suite inputs): only the fslstats
  suite's deliberate `outl` (range moves) and `bigcount` (overflow, no move) reach it; none of
  fMRIPrep's. Smallest trigger: 99 values on [0,1) + one at 1e12; with 1000 values one value from
  ~3.2e6 moves it. **NOTE: the callers are BET (bet2) and FLIRT, not FAST** (FAST has its own
  percentiles; it never calls find_thresholds). Decided rule A as FSL-187: `volops::robust_range`
  -> `RobustRange {fsl, fixed, overflow}` (corrected pass only when a bin overflowed);
  `Volume::robust_limits` returns it; callers pick by mode and name FSL-187 where the result
  changes: bet2 (`robust_limits` helper, `adjust_initial_mesh` got a `notices` param), FLIRT
  (`Run::robust_limits`, -in/-ref), fslmaths -thrp/-thrP/-uthrp/-uthrP (`select`'s `causes`),
  fslstats -r, -e, -E. FSL-187's defects.rs entry: tool "FSL", text extended. Cases:
  `d13/thrp-far`, `d13/uthrP-far` (new input `far3`, marker v8.done), fslstats `d13/e-far`.
  BET replica smoke 27 pass/24 both-error/3 expected-divergence and FLIRT replica smoke 18/4:
  unchanged.
- **FSL-154, FSL-157: DONE** (they were decided, not implemented). FSL-154: uint8 image in the
  char template (-dt char / -dt input) computes in `u8` (new `Elem`/`Arith` for u8 in the core;
  `read4d` names FSL-154 where an image holds 128..255; FSL-154 is in the suite's `_SILENT`).
  FSL-157: -odt input on int8/uint16 rounded (named where rounding != truncation); int8 written
  as DT_INT8 (`Volume::save_volume_coded`, clamp -128..127; always named for an int8 output);
  uint16 stays int32 (FSL's closest template, rule C). Cases `d13/char-uint8-thr`,
  `d13/input-uint8-div`, `d13/odt-input-int8`, `d13/odt-input-uint16-round`.
- **Windows-aware helper: DONE.** Main merged (ccd50d0, 3dd05c7).
  `larmorx_nc_core::volume::last_separator` (/ ; on Windows \ too), used by `find_pathname` and
  re-exported as topup's `options::last_separator`. fslmaths/fslstats and `remove_ext` /
  `make_basename` parse no name at / or . (suffix-based), so nothing else needed it.

## State at the end of relay 6 (2026-11-03, commit ed31e33) [superseded: see relay 7]
- fsl-maths-replica standard 412 cases (350 pass, 57 both-error, 1 expected-divergence +4 new);
  fsl-stats-replica standard 205 (170 pass, 35 both-error).

## Relay 7 (sub-task 5 split: 5b DONE here; 5c next)
Commits: 784d587 (wrappers, dispatch, registry), 4c6c362 (golden cases, Python tests, memory
cases, parity wrappers), a5ac54a (the core's streaming reader, reader suite cases, the wrappers'
check), b178831 (CLI test example), 4ff0d47 (findings' memory section, PROVENANCE). Tree clean.
- **Python** (`python/larmorx/fsl/maths.py`, `stats.py`; exported in `lx.fsl`, docstring updated):
  generic `fslmaths(image, *operations, dt=, odt=)` -> Image and `fslstats(image, *options,
  timeseries=, index_mask=, json=)` -> str (stdout), both run the replica **in the caller's working
  directory** (relative names as in a shell; `-save`/`-roc` files land there; generated inputs and
  the output are absolute paths in a temp dir; FSLOUTPUTTYPE=NIFTI; FSL* variables excluded).
  In-memory images keep their data type (int8/uint16/... too; bool -> uint8; others float32).
  Typed fslmaths wrappers (one per family): `arithmetic` (BINARY names), `apply_mask`, `threshold`
  (of= value/robust_range/positive_robust_range), `unary` (UNARY), `binarise`, `add_noise`,
  `grid`, `dog_edge`, `normalise_intensity`, `spatial_filter` (SPATIAL_FILTERS, kernel= "3D" |
  ("gauss", s) | ("file", img) ...), `smooth`, `subsample2`, `collapse` (COLLAPSE, axis, percentile),
  `statistical` (STATISTICAL), `roi`, `bandpass_temporal_filter`, `tfce`, `tfce_support`, `roc`
  (-> RocResult: aroc, quartiles, curve, image), `tensor_decomp` (-> TensorDecomposition). Typed
  fslstats wrappers (text numbers, six decimals, as nipype reads them; shape per volume/label,
  NaN for a missing label): `image_mean`, `image_std`, `image_percentile`, `image_range`,
  `image_entropy`, `image_volume`, `centre_of_gravity`, `extremum_position`, `bounding_roi`,
  `image_histogram`; common keywords mask/lower/upper (both mask and thresholds: ValueError),
  absolute, nan_as_zero, difference, per_volume (-t), labels (-K). Errors: ReplicaError with
  fslmaths'/fslstats' message without the usage text (`_replica.ReplicaError` gained `stdout`).
  No `_core.pyi` change (no new core functions; the modules are typed, `py.typed`).
- **Dispatch**: `larmorx fsl fslmaths|fslstats` (crates/larmorx-cli: FSL_TOOLS + REPLICAS
  `original: false`; tests updated: replica_only lists, the unknown-tool example is now melodic).
  NOTE for the merge: the topup clean-room agent edits the same lists (topup -> original).
- **Golden** (`crates-nc/larmorx-nc-cli/tests/golden.rs`): 29 cases in BET_CASES (fslmaths.* one
  per family incl. FFT, -save chains, GLIBC-07 double fix+reproduce, FSL-157 fix+reproduce with
  FSL-151, FSL-160 fix+reproduce, FSL-162; fslstats.options/-t -K/-json/FSL-182 fix+reproduce) +
  AVW_REFUSALS (FSL-161, FSL-186); new inputs in write_bet_inputs (mask8, labels, sparse, i8, zmap,
  zmap2, truth, kern, tensor 8x8x6x6); PRINTING gains fslmaths/fslstats (stdout.txt). All run,
  named defects as expected, no path in any output; **UNRECORDED** (sub-task 6:
  `LARMORX_GOLDEN_RECORD=1` from the clean commit). They add ~1-2 s to the 39 s debug golden run.
  `every_tool_has_a_golden_case` passes; registry `REPLICA_ONLY_FUNCTIONS` lists every new
  function (tests/golden pass without FSL: 602 passed).
- **Tests**: `tests/python/test_fsl_maths.py` (stand-in program: 50 tests); `tests/parity/
  test_fsl_maths_replica.py`, `test_fsl_stats_replica.py` (smoke 19 + 3 pass);
  `tests/parity/test_memory_peak.py` AVW_CASES (6 calls, both modes, outputs and stdout compared;
  `_peak_run`/`_original_run` helpers added): **all pass** after the reader fix. No thread test
  (one thread).
- **Core reader (memory finding, fixed)**: raw/gzip read as a stream (`nifti::Source`),
  `load_header` reads 540 bytes only, data converted piece by piece (`open_image`,
  `ImageData::read`, `convert_buffer` appends); zstd/bzip2 still whole. Before: -Tstd 116 MiB vs
  FSL 75, fslstats -t 69 vs 43; after: 37.5 vs 75, 37.4 vs 43 (BET's calls fell too). Checked:
  fsl-maths-replica standard 417 (356 pass, 60 both-error, 1 expected-divergence: +5 `io/`
  reader cases, identical to FSL), fsl-stats-replica 205 (170/35), smoke tiers mcflirt 131
  (108/23), FLIRT 22 (18/4), FAST 2/2, topup 5 (4/1), BET 54 (27/24/3): unchanged; mcflirt's
  `rgb-input` regression (empty data for an unread datatype) found and fixed before the commit.
  `fsl_mcflirt_replica._same_streams` compares a copied truncated input by its bytes.
  Every FSL replica record whose inputs are gz/raw goes through the new reader: sub-task 6's
  re-made records must stay identical (add to its list).
- Helper committed: `validation/investigations/fsl-maths/wrappers_check.py` (56 checks, 0 differ).
- Gates run: fmt (root, nc), clippy -p larmorx-nc-core -p larmorx-nc-cli, -p larmorx-cli; cargo
  test larmorx-nc-core, -avwutils, larmorx-cli, the nc golden (only "not recorded"); ruff
  check/format; pytest tests/python tests/golden (no FSLDIR, LARMORX_NC_BIN). Not run: full gates,
  smoke tier, records (sub-task 6). `python/larmorx/_core.abi3.so`: rebuilt in this relay
  (wheel in `<scratch>/wheel`).

## Relay 8 (sub-task 5c: specs and API pages) - DONE
Commit: ab99259. Tree clean.
- **`specs/fslmaths.md`** (21 sections, ~990 lines with the usage text): purpose and fMRIPrep's
  calls (combine_masks; the 14 goodvoxels calls in order, `-s 5 ... -dilD` with the Gaussian kernel
  `-s` leaves), the command line (the chain, `-dt` only first, `-odt` only second-to-last, the
  output name last, numbers by strtod/atof/atoi), data types, the usage text (byte-identical to
  `usage.rs`, checked), messages; images (reading in the computing type, operands' sizes and
  cycling, the orientation warning's RMS formula, writing: rounding, display range, conversion,
  header, `-save`); the arithmetic conventions; every operation family (binary, thresholds and the
  robust range as specs/bet.md §4.2 generalised, functions and GLIBC-07, Wichmann-Hill and `-seed`
  via glibc's rand as specs/fast.md §8.1, `-index`, `-grid`, kernels, morphology with Armadillo's
  two-partial max/sum, separable and direct/FFT convolution, subsampling, edge, DoG (voxels,
  in-plane), fillh, inm/ing, reductions, p-values/ranks/z<->p, roi, bptf, TFCE (no step factor),
  -tfceS, ROC, tensor decomposition with FA/MO formulas); status table; bit-deciding conventions;
  the defects table FSL-151..180 + 187 + GLIBC-07 with fix/reproduce; rule C list; not replicated;
  built for speed; validation plan (bit-identical list, sums 1e-5 of max + 99 % within 4 ulp,
  double functions 1 ulp, binary-after-sum 0.01 %, TFCE 1e-5, ROC 1e-5, tensor 1e-5 / sign-free
  eigenvectors; memory rule).
- **`specs/fslstats.md`** (10 sections): preoptions, options table, usage text (byte-identical to
  `fslstats::USAGE`), messages and statuses table; images; the pass state and the options that
  change it (thresholds replace masks, `-k`, `-d`, `-a`, `-n`); every statistic (blocks, unmasked
  forms, extremes, robust range, percentiles, histograms and entropy, `-w`, cog); text and JSON
  output (nlohmann's number forms); conventions; defects FSL-181..189; rule C; speed; validation
  plan (identical text for exact statistics, 1e-6 relative for sums).
- Both specs cite suite cases in brackets; every citation checked to exist
  (`validation/investigations/fsl-maths/spec_cases.py`: 38 + 15, none missing). No routine names
  (only printed messages and public libraries: Cephes, Armadillo, glibc, nlohmann, Kiss FFT).
- **`docs/api/fsl-maths.md`, `docs/api/fsl-stats.md`** (topup's model): two implementations,
  status with the standard tier's numbers (fslmaths 356 of 357 + 60 both-error, `fn/double-tan`
  GLIBC-07; fslstats 170 of 170 + 35), the ribbon stand-in sentence, quick start, the generic
  functions (caller's working directory; six decimals vs `-json`), the option tables to the typed
  wrappers, data types, GLIBC-07, Windows names, D13 lists, Not yet.
- **Findings** (`docs/findings/fsl-maths.md`): header points to the specs; the functions bullet
  now says GLIBC-07; the "Not yet translated" section became "Completeness, and the specs" (5
  points: dog_edge sigmas in voxels, -grid sets, -log leaves <= 0, -s's kernel stays, TFCE has no
  step factor).
- **PROVENANCE**: avwutils (fslstats' command line and its core program, all defects incl. 154,
  157, 187 and fslstats', the maths now CORE-MATH + GLIBC-07, fslstats' copyright); core (the tool
  crates list, three new "Changes" bullets: the streaming reader, fslmaths'/fslstats' maths, the
  D13 hooks RobustRange/u8/save_volume_coded/NaN-free statistics).
- **Docstrings fixed** in `python/larmorx/fsl/maths.py`: `dog_edge` (sigmas in voxels, in-plane),
  `grid` (sets the grid's voxels). tests/python/test_fsl_maths.py: 50 pass.
- Gates run: ruff check/format (the changed Python), check_spdx. Nothing else changed.

## Relay 9 (sub-task 6: random testing and the records)
Commits: 0427fad (descriptors, @dt: inputs, permanent cases, first fixes), 6ac018f (trial triage,
probes, findings section), 030b5d7 + 56af74b (lint).
- **Descriptors** `validation/src/larmorx_validation/difftest/tools/fsl_avwutils.py`: `fsl-maths`
  (MATHS: six image kinds general/series/mask/zmap/pmap/tensor, scan-like grids 6-32 per axis,
  chains of 1-4 operations drawn for the current image, every family; `-dt`/`-odt`, output names,
  FSLOUTPUTTYPE) and `fsl-stats` (STATS_TOOL: -t/-K/-json preoptions, 1-5 options or fMRIPrep's
  and BET's forms); registered in `tools/__init__.py`; `permanent.SUITES` gains both suites; the
  suites' `_prepare` read `@dt:` inputs; fslstats' `run_case` has a `direct` kind; `tables.py`
  METRICS fsl-maths-replica / fsl-stats-replica = _bet. Not drawn: see the module docstring.
- **Check run** `<workspace>/oracles/difftest/2026-11-03-fsl-maths-check` (seed check1, 1 min,
  5,186 cases: 3 differences) and **the trial** `<workspace>/oracles/difftest/2026-11-03-fsl-
  maths-replica` (seed fsl-maths-trial, budget 0.4 h = 24 min, 13 jobs at 1 thread, 1.5 GiB per
  case, box 16 GiB, LARMORX_DIFFTEST_CACHE_MODE=use, larmorx 0427fad; log oracles/logs/2026-11-03-
  fsl-maths-replica.log): **183,983 cases: fsl-maths 114,906 (114,310 agree, 595 notice, 1 differ),
  fsl-stats 69,077 (69,068 agree, 9 differ)**; no memory, timeout, crash.
- **Fixed** (replica; every finding replayed boxed: agree): -edge squares with x*x (FSL's build;
  glibc's pow differs: `validation/investigations/fsl-maths/edge_pow_probe.py`); -subsamp2 of a
  3-wide image names FSL-75 in both modes (FSL writes past calc_bval's buffer and aborts; value
  never used; 1x1x1 refused by reproduce; FSL-75's defects.rs text extended to fslmaths; FSL-75 in
  the suite's `_SILENT`); FSL-154 named for every image the u8 template reads; FSL-164 named only
  where every-volume filtering changes a voxel (`quiet_conv`); calc_cog's NaN signs (`cog_sums_sse`:
  block flushes `tot + total`, `vx + v_cog`, from libfsl-newimage.so's machine code; 8 fslstats
  findings -c/-C); -json's deferred notices flushed on an error (FSL-185 before FSL-184's refusal).
  Permanent cases: `validation/difftest/cases/fsl-maths/` (edge-double-square, subsamp2-three-wide,
  dt-input-uint8-dog-edge) and `fsl-stats/` (cog-nan-sign-voxels, cog-nan-sign-mm,
  json-notices-before-refusal), tier standard, status fixed; each run in its suite (pass/both-error).
- **[RESOLVED by relay 6b: the twiddle factors, glibc's sincos, GLIBC-07; see "Relay 6b"]**
  **OPEN (for the next step; the orchestrator decides when):** the Kiss FFT engine
  (`larmorx-nc-core::kissfft`) differs from Armadillo 14.2.1 for lengths 15, 30, 60 (radix-3 stage
  with m >= 5): probe `validation/probes/fsl-avwutils-lib/fft_probe.cc` (build: `PATH=$HOME/fsl-
  6.0.7.23/bin:/usr/bin:$PATH g++ -O2 -std=c++17 -DARMA_DONT_USE_WRAPPER -I $HOME/fsl-6.0.7.23/
  include fft_probe.cc -o <scratch>/fft_probe`; `<scratch>/fft_probe 60 38 15` vs `LARMORX_FFT_
  SIZES="60 38 15" cargo test --release -p larmorx-nc-core --test fft_probe kiss_fft_lines --
  --ignored --nocapture`): lengths 2-25 identical except 15. Twiddles identical to glibc
  (`fft_twiddles.py`, the `twiddles` ignored test), GCC's complex multiply = the formula, and a
  literal Python transcription (`<scratch>/py/kiss_emul.py`) gives the replica's values, not
  Armadillo's: so the compiled engine does something the reading missed (next idea: print
  Armadillo's intermediate butterfly values for N=15 from a copy of the header with prints, or
  compare `arma::fft` of a `Col<cx_double>` against `fft_engine_kissfft` called directly). Trial
  finding `fsl-maths-trial-61795` (`-kernel gauss 2.25 -fmeanu` of 30x1x3: 1 of 90 voxels 1 ulp);
  NOT promoted (it would fail). Triage status for the handoff: open (a larmorx mistake to fix).
- **Notices without a visible change** (595 fslmaths cases): chains whose later operation removes
  an intermediate correction's effect, and FSL-151's output rounding of values both modes store
  alike (counts by defect: FSL-151 328, GLIBC-07 73, FSL-164 49, FSL-152 36, FSL-154 combos 61,
  FSL-168 22, FSL-167 9, FSL-153 7, FSL-169 5, FSL-170 3, FSL-163 1). Triage: explained.
- Findings: `docs/findings/fsl-maths.md` gains "Random testing (sub-task 6, 2026-11-03)".
- **Gates** (from 56af74b, wheel rebuilt into <scratch>/wheel and `_core.abi3.so` copied;
  logs oracles/logs/fsl-maths-1-gates.log, fsl-maths-1-nctests.log): root OK, crates-gpl OK,
  crates-nc: fmt and clippy clean, 151 tests pass, `golden` fails only on the 31 unrecorded
  fslmaths/fslstats cases (167 outputs "not recorded"; no checksum differs) after 6342d27
  (fslmaths.char names FSL-154); gates-python --no-build: 761 passed + 602 passed without FSLDIR,
  collection, ruff, SPDX OK; `tables --check` OK.
- **Records** (from 6342d27, clean; `<scratch>/records.sh`, log oracles/logs/fsl-maths-1-records.log):
  `docs/validation/fsl-maths-replica.{json,md}` **420 cases: 358 pass (18 fMRIPrep: combine_masks
  and the 14 goodvoxels steps), 61 both-error, 1 expected-divergence (fn/double-tan, GLIBC-07)**;
  FSL 376 runs from the cache, 39 run (signals, never stored). `docs/validation/fsl-stats-replica.
  {json,md}` **208 cases: 172 pass (4 goodvoxels), 36 both-error**; FSL 203 from the cache, 5 run.
  Committed f56c282. **The five FSL replica records re-made** into <scratch>/rec/records (box 30,
  run-suites.sh, jobs 3+3+3+2+3): **compare_records: mcflirt 237/237, BET 178/178, FLIRT 302/302,
  FAST 86/86, topup 173/173: status differs 0, details differ 0, summaries equal** (the streaming
  reader, direct_max, the FSL-187 robust range, the index error type and calc_cog's flush order
  move none of them).
- **Parity smoke tier** from f56c282 (boxed 32, `parity_smoke.py --workers 14`; log
  oracles/logs/fsl-maths-1-smoke.log): **2,731 passed, 0 failed** (parallel 266 s, then the memory
  and thread files serially 789 s: every replica's memory calls again); marker
  parity-smoke-f56c282... .
- **Golden**: `record_golden.py` from f56c282 (marker reused): 208 cases, 1,283 outputs, only the
  header changed (b19b5b9). **larmorx-nc golden** recorded from b19b5b9 (3f06ecb): the 31
  fslmaths/fslstats cases (167 outputs) added, every earlier checksum unchanged; the check passes
  after recording.
- **API pages** (7d9ead6): status lines from the records and the trial.
- Tree clean at 7d9ead6. Nothing of this relay still runs.

## Relay 6b (the FFT engine's open case, the orchestrator's item) - DONE
Commits: a51517a (cause, probes, permanent case, docs), 397485a (fsl-maths-replica record from
a51517a, API status line). Tree clean.
- **Cause: the twiddle factors (step 1 settled it; GLIBC-07, step 2).** FSL's
  `libfsl-newimage.so` (its machine code: `fft_engine_kissfft<..>::fft_engine_kissfft` calls
  `cexp@GLIBC_2.2.5`) computes each factor as `cexp(0 + i (n k))`; glibc's `cexp` takes glibc's
  `sincos`. glibc 2.35's `sincos` is ONE build on x86-64 (a plain FUNC, no IFUNC), while `sin` and
  `cos` choose an FMA build on CPUs with FMA (this one): the two disagree on 0.04-0.07 % of
  arguments (0 with `GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA`). Relay 9's `fft_twiddles.py`
  compared the separate `cos`/`sin` (the FMA build, which agrees with correctly rounded there),
  hence "twiddles identical": the wrong glibc function. N=15: `sin(2k)` (`...bec2` vs CR `...bec1`);
  N=60: `sin(8k)`, `sin(57k)`.
- **Proof** (`validation/probes/fsl-avwutils-lib`, README "The FFT"): `fft_twiddles.cc` prints the
  engine's own `coeffs_array` beside glibc's cexp/sincos/cos/sin; `check_fft.py` (lengths 1-256):
  65,792 factors, Armadillo = cexp = sincos on all, vs separate cos/sin 106 differ, vs the
  replica (CR) 220 differ in 80 lengths (15 20 30 39 40 49 51 59 60 61 ...); angles identical (pi,
  the product, the inverse sign); transforms 197,376 values: replica 5,426 differ in 60 lengths;
  **replica engine given Armadillo's factors (`KissFft::with_twiddles`): 0 differ**.
  `fft_reads.py`: a butterfly reads a differing factor in 64 of the 80 lengths. End to end
  (`check_fft_case.py`, probe-only cargo feature `probe-twiddles` of larmorx-nc-core, built into a
  scratch target dir, reading `LARMORX_PROBE_FFT_TWIDDLES`): the trial's case with Armadillo's
  factors = FSL's output bit for bit; with the replica's own: 1 of 90 voxels differs. The case
  uses transforms of 60 and 26 points (not 38).
- **Measured** (`validation/probes/glibc-double-maths`, new `sincos.sin`, `sincos.cos`,
  `sincos.sin-vs-sin`, `sincos.cos-vs-cos`, family "twiddle angles" [-2pi, 2pi], `--only`): sincos
  vs CR on the angles 0.148 % (sin), 0.148 % (cos); on intensities 0.128 / 0.092 %. **Every** glibc
  double function fslmaths calls (sin cos tan atan asin acos exp log pow) has an FMA build on
  x86-64: FSL's own double results depend on whether the CPU has FMA (both builds' counts in the
  README); the replica's `reproduce` exp/log/pow bits are the FMA build's (this machine's).
- **Decision (step 2):** correctly rounded in both modes (unchanged behaviour); no notice (as for
  the six IBM functions). Permanent case `validation/difftest/cases/fsl-maths/fft-twiddles-glibc-
  sincos.json` (tier standard, status register, defect GLIBC-07, expect divergence); the suite's
  `Run.glibc_where` + `_glibc_divergence` now accept float outputs when a `where` is given;
  `case_maths` maps a permanent case's `expect = "divergence"` to `glibc=1, glibc_where=reason`.
  The suite reports it `expected-divergence` ("GLIBC-07: 1 of 90 float voxels 1 unit(s) ...").
- **Docs**: findings (efficient_convolve bullet; Known differences: new FFT bullet; Random testing:
  OPEN -> explained), `specs/fslmaths.md` §10 (factors = glibc's sincos, GLIBC-07; the D13 table
  row; the validation plan), `docs/api/fsl-maths.md` (status line, maths section, GLIBC-07
  bullet), core PROVENANCE, kissfft.rs module docs, `larmorx_core::defects` GLIBC-07 text extended
  (sincos/FFT), investigations README (relay 9's three helpers listed; `fft_twiddles.py` now also
  calls `sincos`).
- **Re-run** (only what the change reaches: the core FFT's only caller is `filters::generic_convolve`
  <- fslmaths' `-s/-fmean/-fmeanu`; no other replica calls it): fsl-maths-replica record from
  a51517a: **421 cases: 358 pass, 61 both-error, 2 expected-divergence** (fn/double-tan,
  difftest/fft-twiddles-glibc-sincos); `compare_records` vs relay 9's: 420 earlier cases, status
  differs 0, details differ 0 (log oracles/logs/fsl-maths-1-records-6b.log). nc golden check
  (debug, boxed): 2 passed (every FFT golden checksum unchanged). cargo test core + avwutils pass
  (new unit test `a_twiddle_table_gives_the_engine_its_factors`); clippy core/avwutils/cli, core
  with `--features probe-twiddles`, larmorx-core: clean; fmt (root, nc, the glibc probe) clean;
  `cargo test -p larmorx-core defects` pass; ruff check/format, check_spdx clean;
  `pytest validation/tests` 159 passed (unboxed and without LARMORX_ORACLE_CACHE_MODE: inside a box
  three tests fail on their own nested box / cache mode); `tests/parity/test_fsl_maths_replica.py`
  19 passed. Not re-run: the smoke tier, the other records, record_golden (nothing they cover
  moved). `python/larmorx/_core.abi3.so` lacks the GLIBC-07 text extension (rebuild before any
  Python gate that prints defect texts).
- Pre-existing (not this relay's): the glibc-double-maths probe's clippy `-D warnings` fails on
  relay 6's `coremath` `type_complexity` (line ~388); the probe workspace is not in the gates.
- Scratch: `<scratch>/b6/` (probe binaries, `work256/arma_twiddles.txt`, `target-probe/` probe
  build, `check_fft_1_256.txt`), `<scratch>/gdm/measure26-*.txt`, `records6b.sh`, `rec6b/`.

## (Done by relay 7, see below) Plan for sub-task 7 (the site pages and the handoff), then the final report
- Site pages `/tools/fsl-maths/`, `/tools/fsl-stats/` (brief's sub-task 7; topup's page the model),
  from the records `docs/validation/fsl-maths-replica.json`, `fsl-stats-replica.json`, the trial
  (`<workspace>/oracles/difftest/2026-11-03-fsl-maths-replica/summary.json`, report.md) and
  fMRIPrep's calls (combine_masks on ds000005/ds005040/ds003592; goodvoxels on ds005040 with the
  ribbon stand-in, say so). Evidence module in `validation/src/larmorx_validation/evidence/`.
- **HANDOFF.toml / HANDOFF.md**, collecting: relay 6's and relay 8's lists ("For the handoff
  (sub-task 7)" in "Next" of relay 8's notes above, kept below), and from this relay:
  - `validation/difftest/triage.toml`: the draft `<scratch>/triage-draft.toml` (7 entries: the
    FFT case now `register` GLIBC-07, FIRST of the fsl-maths "every file bit-identical" entries so
    the site's trial panel matches it, not edge-double-square's; check that the 595-notices entry's
    status "explained" is a status the site/triage accepts: the file's header lists fixed, register,
    open, harness, unclear);
  - `docs/validation/difftest.md`: two table rows (fsl-maths, fsl-stats: parity runner
    `parity.fsl_maths_replica` / `fsl_stats_replica`, implementations replica vs fslmaths /
    fslstats, what is drawn: the descriptor's docstring) and a sentence on the trial in the oracle
    cache paragraph (183,983 cases, 24 min, 13 jobs, 1.5 GiB per case, seed fsl-maths-trial);
  - `validation/tools.toml` entries for fsl-maths and fsl-stats (model fsl-topup's), then
    `python -m larmorx_validation tables` (tables.py METRICS already registered in-branch);
  - the register (`docs/findings/upstream-defects.md`, behaviour only): FSL-151..189 rows,
    GLIBC-07 (naming the functions with their frequencies AND glibc's `sincos` in the FFT
    convolution's twiddle factors: 80 of the lengths 1-256 hold a factor one bit away, 0.148 % of
    twiddle angles; the trial met it once in 114,906 fslmaths cases; and that glibc picks an FMA
    build of each double function at run time, so FSL's own double results depend on the CPU;
    `sincos` has one build), and FSL-75's row extended to fslmaths' -subsamp2/-subsamp2offc (defects.rs text
    already extended in-branch: tool still "flirt"; the register's tool column may say FLIRT,
    mcflirt, fslmaths);
  - next-steps follow-ups: (the Kiss FFT engine's difference is explained: relay 6b; no follow-up
    of its own) GLIBC-07's cbrtf-route part must cover `sincos` too (the FFT's twiddle factors; one
    build) and decide which build of `sin` ... `pow` "glibc's bits" means (FMA or not: they differ;
    the oracle machine and the old laptop have FMA); D13 notice precision in chains and
    FSL-151's output rounding (595 notices: explained); plus relay 8's list (zstd/bzip2 read whole,
    the clean-room originals from the specs, goodvoxels with a real ribbon, GLIBC-07's six IBM
    functions by the cbrtf route, the restricted-findings lists gain fsl-maths.md).
- The final report (the brief's "The final report"): what was run in relay 9: the check run
  (5,186 cases) and the trial (183,983), each record once (the two new + five re-made), the smoke
  tier once (f56c282), record_golden once (marker reused), the nc golden once (+1 check); relay
  6b: the fsl-maths-replica record once more (a51517a, +1 case), the nc golden check once.
- **Site page `/tools/fsl-maths/` must show for the FFT case**: the record's second expected
  divergence (`difftest/fft-twiddles-glibc-sincos`, GLIBC-07, 1 of 90 float voxels, one unit in the
  last place), with its reason in plain words (FSL's FFT takes its twiddle factors from glibc's
  sincos, not correctly rounded; larmorx's are; with FSL's own factors larmorx's output is FSL's bit
  for bit: the probe's 0 of 197,376 and the end-to-end case), a verdict from `data/verdicts.yml`
  (expected / not important: the last bit of a float, never on fMRIPrep's calls); the trial panel's
  one fslmaths difference should read this triage entry (see above).

### Kept from relay 8's "Next" (for the handoff)
- the reader change as one line in docs/status or the register? (no defect: a larmorx memory fix);
  next-steps: zstd/bzip2 inputs are still decompressed whole (memory 2x for those formats); the
  clean-room originals of fslmaths and fslstats from the new specs; the goodvoxels re-check with a
  real ribbon; GLIBC-07's six IBM functions by the cbrtf route (frequencies in the findings'
  table); **the porting guide's §1 and CLAUDE.md rule 1's lists of restricted findings gain
  `fsl-maths.md`** (`docs/findings/fsl-maths.md` covers fslmaths and fslstats).

## Relay 7 (sub-task 7: the site pages, the handoff, the final report) - DONE; THE PART IS COMPLETE
Commits: 6a1e3b8 (glibc-double-maths probe: clippy type alias; fsl-avwutils-lib README: hand lint of
probe-twiddles), b4b23e2 (evidence modules fsl_maths.py, fsl_stats.py; TOOLS; summary.py PAGES),
70d05f3 (combine table: one row per record case), a6fe0ef (GLIBC-07's defects.rs text: tan 0.21 %,
not 0.25 %; findings: "glibc's bits" = the FMA build), fb25f03 (fft_real_kernels.py + README).
Tree clean at fb25f03. `_core.abi3.so` and crates-nc/target/release/larmorx-nc rebuilt at a6fe0ef.
- **Figures-cache entries** (`<workspace>/oracles/figures-cache/fsl-maths/{fmriprep-combine-masks,
  fmriprep-goodvoxels,families,glibc-double,cases-default-mode}`, `fsl-stats/{fmriprep-goodvoxels,
  options-real,cases-default-mode}`) from clean b4b23e2 (log oracles/logs/fsl-maths-1-entries7.log):
  FSL served 1 + 14 + 4 (+1 BET) runs, ran through the cache 15 + 8 + 28. Everything bit-identical
  (families 15/15 both modes; fslstats 29/29 reproduce; default differs only on -S, -d, -t: FSL-182).
  GLIBC-07 real image (-dt double on the BOLD mean): sin 211, cos 84, tan 313, asin 209, acos 20,
  atan 200 of 136,400 differ by 1 ulp; exp/log reproduce = FSL, default corrects 110/15.
- **Export** from 70d05f3 into the site clone (`<scratch>/site`, branch fsl-maths-page, 8074099;
  evidence-blobs 3e03069, 897 blobs = 851 + 46). Pages content/tools/fsl-maths.md, fsl-stats.md.
  Checks: build 0 problems; number check 146/146 and 51/51; viewers 61/61; no "could not be
  loaded"; blobs verify OK; inventory `<scratch>/inventory.md` (all pages after >= before; only the
  manifest flagged, +46 -0); screenshots `<scratch>/shots/after/` (32). Server stopped.
- **Handoff**: `<scratch>/HANDOFF.toml` (32 ops, 14 files; written against main 05454bc and the
  site's main 0d1dbdc: --check OK there) and `<scratch>/HANDOFF.md` (reasons; register counts for
  the orchestrator: 376 on the worktree + handoff, main adds FSL-144..146; triage.toml header lacks
  `explained`).
- Gates this relay: ruff check/format (validation), check_spdx, pytest validation/tests 159 passed
  (unboxed), tables --check, cargo test -p larmorx-core defects, cargo fmt (root), the probe's
  clippy -D warnings and fmt.

## FROM THE ORCHESTRATOR (keep this section in every rewrite)
- (2026-11-03) **For sub-task 7 (relay 7), from relay 6b's report.**
  - **GLIBC-07 is settled (relay 6b):** the FFT twiddles come from glibc's `cexp`, so from its `sincos`. Correctly
    rounded in both modes, as for the six IBM functions; the permanent case is an expected divergence. The handoff's
    GLIBC-07 texts (register row, next-steps follow-up for the cbrtf-route part) name `sincos` as a seventh function,
    with its measured frequency (0.148 % of twiddle angles).
  - **"glibc's bits" means the FMA build** (the orchestrator's decision, recorded at the merge in the decision log).
    glibc 2.35 picks the FMA builds at run time on every x86-64 CPU since 2013, and the oracle machines run them. The
    other build is a variant to note, not a target. Say so in the findings' GLIBC-07 section and in the handoff's
    GLIBC-07 texts. The existing `reproduce` for exp/log/pow already matches it.
  - **Triage status `explained` is valid**: `docs/validation/difftest.md` lists it (fixed | register | open | harness |
    explained | unclear), and `triage.toml` already holds 8 such entries. Keep it for the 595 notices. If
    `triage.toml`'s header comment lacks it, add one line to HANDOFF.md, and the orchestrator fixes the header at the
    merge. Keep the FFT entry before the other "every file bit-identical" entries, as relay 6b ordered them.
  - **Rebuild `_core.abi3.so`** (maturin into `<scratch>/wheel`, copy into the worktree's `python/larmorx/`) before any
    Python step that prints defect texts or exports the page's evidence. The worktree's copy lacks the extended
    GLIBC-07 text.
  - **Small, your own probe code:** if the `glibc-double-maths` probe's clippy `type_complexity` warning is a
    one-line type alias, fix it. In the handoff, as next-steps follow-ups: the three `validation/tests` that fail
    inside `boxed.sh` or with `LARMORX_ORACLE_CACHE_MODE` set (they should not depend on the caller's
    environment), and the probe-only feature `probe-twiddles`, linted by hand only (write the command in the
    probe's README).
  - **The site:** your clone comes from the site's `main` as it is now (171a8ae) and its `evidence-blobs` from
    72d1f2d (851 blobs). topup's clean-room agent is adding to topup's page beside you, in its own clone. Touch only
    your new pages and their evidence data; shared site files go through the handoff. The orchestrator joins the two
    parts' blobs at the merges.
- (2026-11-03) **DONE by relay 6b (a51517a, 397485a; see "Relay 6b": step 1 found the cause, step 2 applied).**
  **The FFT engine's open case (relay 6's report) is relay 6b's whole task, before sub-task 7.** A replica
  is bit-exact (rule 8: the source is the specification), so the 1-bit difference against Armadillo 14.2.1's
  `fft_engine_kissfft` for lengths 15, 30 and 60 is a cause to find, not a tolerance.
  1. **Test the twiddles first.** Armadillo computes each twiddle with `std::exp` of a complex number, which goes to
     glibc's `cexp` and so to glibc's double `sincos` (IBM's dbl-64 code, LGPL, never translated). If our engine
     takes CORE-MATH's correctly rounded `sin`/`cos`, the twiddles differ wherever glibc's are not correctly rounded
     (relay 5a measured 0.02-0.21 % per function). Compare the twiddles bit for bit for N = 15, 30, 60 (and the
     lengths the trial met) in the committed probe; then check π's constant, the phase product's order of operations,
     and the inverse sign.
  2. **If the twiddles are the cause:** this is GLIBC-07 (the same decision as `fn/double-tan`): correctly rounded in
     both modes for now; the case becomes a permanent case marked as an expected divergence naming GLIBC-07; the
     findings, the API page and GLIBC-07's handoff text name the FFT twiddles as one more place glibc's `sin`/`cos`
     reach, with how often the trial meets it. Exact bits come later by the cbrtf route (the orchestrator schedules it).
  3. **If not:** compare butterfly by butterfly (the factor order Armadillo chooses, the radix-3 and radix-5 butterflies'
     order of operations and constants, the generic butterfly). Fix what differs, make the trial's case a permanent
     case that passes, and re-run only what the change reaches: list the core FFT's callers first; at least the
     `fsl-maths-replica` record from the new clean commit (`compare_records` against relay 6's: only FFT cases may
     move, toward FSL), the smoke tier and the nc golden check; any other replica that calls the FFT gets its record re-made too.
  4. **Bounded:** if neither settles it within this relay, keep the probe, keep the case out of the permanent set,
     write the open finding with what was ruled out, and put a follow-up in the handoff draft. Then stop.
- (2026-11-03) **Sub-task 6's re-made records are all five FSL replicas'**, because the shared NIfTI reader now
  streams (relay 5b): `fsl-mcflirt-replica`, `fsl-bet --implementation replica`, `fsl-flirt-replica`,
  `fsl-fast-replica` and `fsl-topup-replica`. They come from the final clean commit, once each, side by side within
  the memory budget; `compare_records` must find each identical, and any difference is a finding to explain before
  the handoff. The memory test's measured calls of the other replicas run again too.
- (2026-11-03) **DONE by relay 6 (d83abf2; see Relay 6).** **The bin-range defect in the shared robust range** (`find_thresholds`, used by BET, FAST and
  fslmaths; relay 4 left it as FSL has it). Decided by the rules, one behaviour for one cause:
  - **Measure first (rule D).** Find which inputs reach it, the smallest synthetic trigger included, and check
    whether fMRIPrep's real calls through `find_thresholds` reach it. Those are BET's and FAST's calls in the
    oracle runs; the suites' cases are enough, through the oracle cache.
  - **Then decide as FSL-187** (the input makes sense and the result is wrong: rule A). Correct it by default in
    `find_thresholds`, with `reproduce` giving FSL's result; extend FSL-187's entry, defect texts and register row to
    the tools it reaches (fslstats, fslmaths, BET, FAST).
  - **If the trigger is an input that makes no sense**, decide it as rule B (a clear error) instead.
  - **Check with sub-task 6's re-made records:** BET's and FAST's replica records must stay identical.
  - **For the clean-room side,** add a next-steps follow-up in the handoff: check BET's and FAST's originals for the
    same behaviour.
- **DONE by relay 6 (d83abf2).** **Double maths (the ~09:10 item) is due in this sub-task 5:** CORE-MATH's `tan` ported, and glibc's double functions
  measured.
- (2026-11-03) **DONE by relay 5 (b51141c).** **Armadillo's index error type** (relay 3b: `larmorx-nc-core`'s `bfmatrix.rs:44` and
  `spmat.rs:922` name `std::logic_error`, where Armadillo 14.2.1 throws `std::out_of_range`).
  - The source is the specification (rule 8): fix both in this branch, with a unit test of the message where one
    is reachable, and a line in the findings.
  - Add `fsl-topup-replica` to sub-task 6's re-made records (`compare_records`: identical expected).
- **Defect IDs:** this part's range is FSL-151 to FSL-190. If fslstats and the sweep need more, continue with FSL-191
  to FSL-200 and say so in the handoff.
- (2026-11-03) **The goodvoxels ribbon stand-in** (relay 3a: grey matter from the segmentation, since the
  no-reconall oracle runs have no FreeSurfer ribbon) is acceptable for this part.
  - Say so wherever the case appears: the record's text, the site page and the API page.
  - Put a next-steps follow-up in the handoff: re-check goodvoxels with a real ribbon once a FreeSurfer (recon-all)
    oracle run exists.
- (2026-11-03) **DONE by relay 6 (1b5a617, d4e9935).** **Windows paths** (CI on main 3a15db3 failed on Windows in topup's replica test
  `a_short_estimation_from_a_configuration_file`). FSL finds a name's directory and extension at the last `/`, and
  Windows' temporary folders are named `...\.tmpXXXX\`, so the dot in the folder's name was read as an
  extension. Fixed on main in ccd50d0: `larmorx_nc_topup::options::last_separator` treats `\` as a separator
  only on Windows; Linux and macOS are unchanged; `docs/api/fsl-topup.md` says so.
  - Wherever fslmaths or fslstats (and `larmorx-nc-core`'s name helpers, e.g. `remove_ext` and `make_basename`)
    parse a file name at `/`, use one Windows-aware helper. Put it in `larmorx-nc-core` and have topup's use it
    once you merge main.
  - The tests must pass under a temporary folder whose name holds a dot: rule 10, "Portable tests".
- (2026-11-03) **`arma::direct_max`** (relay 2's question). **DONE by relay 3** (786fb15;
  see "Relay 3" above: one translation, probe 0 differ, callers listed). The source is the specification (rule 8), and
  the FSL replicas follow Armadillo 14.2.1 (CLAUDE.md rule 2).
  - Make the core's `arma::direct_max` follow 14.2.1 (two running maxima), and have `filters::arma_max` use that
    one function: one translation, not two. Do it first in the next relay. Check it against Armadillo 14.2.1 on
    small cases (the sign of a zero maximum included).
  - List every replica that calls it. Sub-task 6's re-made records (`compare_records`) then show whether any
    result moved; any change is a finding to explain.
  - BET's `-dilF` NaN change (relay 2) is covered the same way: BET's replica record is re-made in sub-task 6.
- (2026-11-03) **DONE by relay 6 (d83abf2).** **Double-precision maths functions (`fn/double-tan`)**, decided by the rules and the user's
  precedents: `cbrtf` (2026-10-30) is a pointwise function, so its correctly rounded value is the default and glibc's
  bits come with `reproduce`. BET's `tanh` (2026-10-31) is different, because BET amplifies the last bit.
  1. **The default.** Use correctly rounded functions from `larmorx_core::math` (CORE-MATH). Port CORE-MATH's
     double `tan` (MIT) into `larmorx_core::math`, with its own tests, as `sin` and `cos` were ported; no `libm`
     crate for `tan`.
  2. **Measure first (rule D).** Write a boxed probe that runs against the C library (glibc 2.35, the one the
     oracle uses), committed under `validation/probes/`.
     - Functions: every double maths function fslmaths and fslstats call (`sin`, `cos`, `tan`, `asin`, `acos`,
       `atan`, and the others).
     - What it counts: how often glibc's result differs from the correctly rounded one, on about 10^8 random
       arguments in the ranges fslmaths meets (image intensities; [-1, 1] for `asin` and `acos`). Counts only.
  3. **`reproduce`.** It gives glibc's bits wherever permissive code allows it (Arm's Optimized Routines or fdlibm,
     as `exp` and `log` already do).
     - For functions whose glibc 2.35 code is LGPL (IBM's Accurate Mathematical Library in dbl-64: `sin`, `cos`,
       `tan`, `asin`, `acos`, `atan`), never copy or translate that code.
     - For those, the replica gives the correctly rounded value in both modes for now. The record names each such
       case as an expected divergence, with the measured frequency.
     - Register one entry through the handoff: **GLIBC-07** (class d, glibc 2.35's double functions that are not
       correctly rounded; one entry naming each function with its frequency).
     - Exact glibc bits by the `cbrtf` route (a behaviour spec, then a clean-room implementation in
       `larmorx_core::math`) are a later small part the orchestrator schedules from your measurement. In the
       handoff, say which functions need it and how often each differs.
  - **When:** in sub-task 5 (the contract) at the latest; steps 1 and 2 may come earlier, when convenient.
- The golden test `every_tool_has_a_golden_case` failing until sub-task 5 is expected; the full gates come in sub-task 6.
