# NOTES: FAST clean-room original (relays)

Worktree: <workspace>/larmorx/.claude/worktrees/agent-aa48faf7b7fe7f452
Branch: worktree-agent-aa48faf7b7fe7f452 (from main a173af5, FAST's replica merged)

Environment: `bash <scratch>/run.sh <command>` sets env.sh's variables (PATH with the venv and
cargo, FSL off it; PYO3_PYTHON; OPENBLAS_CORETYPE=Haswell; PYTHONPATH=python:validation/src) and
cds into the worktree (the harness refuses `source` and compound shell lines: one plain command
per call; write scripts with the Write tool). The original's binary for the suite and the probes:
`LARMORX_FAST_BIN=<worktree>/target/release/larmorx` (build: `run.sh cargo build --release -p
larmorx-cli -j 14`). The Python extension: `run.sh maturin build --release -j 14 -o
<scratch>/wheel`, then copy the wheel's `larmorx/_core.abi3.so` into `<worktree>/python/larmorx/`
(done at d9fc2fe; never `maturin develop`). The suite: `LARMORX_FAST_BIN=... run.sh
scripts/orchestration/boxed.sh 24 python -m larmorx_validation parity fsl-fast --tier standard
--jobs 3 --out <scratch>/rec1` (about 6 minutes; FSL's runs from the cache, the band's runs of
FSL with the CORE-MATH preload cached too). Summary: `python -I <scratch>/py/summary.py
<scratch>/rec1/fsl-fast.json`.

## State (relay 5, end: sub-task 5 DONE, the part complete; commits c5f3080, 5e9e4c7, df6b055, 0706968)
- The site: clone `<scratch>/site`, branch `fsl-fast-cleanroom-page` (from 9940a19), FAST's page
  with the clean-room original (see HANDOFF.md, "The site page"); exported from 0706968 (clean);
  `tools/blobs.py commit` done in the clone (site commit 886cfc6, evidence-blobs 034431b). Checks: build 0 problems, number check
  1,316/1,316, viewer check 520/520, no "could not be loaded", inventory of all 20 pages after >=
  before (`<scratch>/inv/`), screenshots `<scratch>/shots/before`, `<scratch>/shots/after`.
- Evidence code (worktree): `evidence/fsl_fast.py` (the original's figures-cache entries
  `fsl-fast/original-*` and `fsl-fast/cases-original`, computed once from c5f3080 in a 12 GiB box:
  FSL with the CORE-MATH preload through the oracle cache, the original in both modes), the case
  verdicts classified against FSL and FSL with correctly rounded maths; `evidence/summary.py`
  (FAST's row lists the original); `tables.py` (METRICS `fsl-fast`); `docs/api/fsl-fast.md` (the
  original done, its record).
- Handoff: `<scratch>/HANDOFF.toml` (24 ops, `--check` OK with `--site <scratch>/site`),
  `<scratch>/HANDOFF.md` (reasons, clean-room statements, spec questions, shared code).
- No record, smoke tier or golden re-run (no code of the tool changed after f0dd88b).

## State (relay 4, end: sub-task 4 DONE; commits e60046f..f0dd88b (code, the records' clean commit), e9024ec (records + golden), 92ad973 (README))
- `exp_in_place` randomized check (orchestrator's item 1): the ignored test
  `math::exp::tests::exp_in_place_randomized` (crates/larmorx-core/src/math/exp.rs): 2^32 values
  of nine kinds mixed in every lane, the baseline and the dispatched (AVX2+FMA) batch against
  `exp` and `math::exp` bit for bit (NaN payloads too): **0 differences** (20 s, 14 threads; boxed
  4 GiB; 17,222,731 lanes sent to the accurate path). README of cleanroom-fast: command + counts.
  Golden case `cli.fsl.fast` (and multi, py) go through the batch (PVE likelihoods).
- Difftest descriptor `fsl-fast-original` (validation/src/larmorx_validation/difftest/tools/
  fsl_fast_original.py: the replica's draw + the original's suite; registered in tools/__init__.py).
  `LARMORX_DIFFTEST_CACHE_MODE=use` (runner.py, NEW, default off): a part's trial runs FSL through
  the cache in use mode (the workers forced `off` before). HANDOFF: docs/validation/difftest.md
  "The oracle cache" should mention it (shared doc; text for relay 5).
- The trial (once): seed cleanroom-fast, --budget 0.42 (25 min) + shrinking, 3 jobs, box 12 GiB,
  cache use; `oracles/difftest/2026-11-03-cleanroom-fast/`: **9,005 cases, 8,823 agree, 182
  differ**. Triage (replay_findings.py, 8 at a time, box 24 GiB; probe.py --finding --diff):
  - exception 153 = harness (nibabel zstd 152, NIfTI-2 quaternion 1): suite fixed; all agree.
  - bias/restore 13 (+1) = NaN-variance class: FSL's field NaN outside the mask (b · M with a NaN
    mean): FIXED (Single::bias_outside); 14 agree.
  - stdout 13: k-means line at a singular abort (9) FIXED; FSL-101's -v line (3) FIXED; 1840 OPEN
    (degenerate multi stats: FSL MEAN -nan vs the original 1; images agree; tried "keep" in the
    members/likelihoods: no, reverted).
  - 5605 exit status (rank-deficient covariance: the original's LU exact 0 pivot aborts, FSL
    finishes) EXPLAINED; 2215/2311 (two noise-free channels: ill-conditioned) EXPLAINED.
  - Permanent cases validation/difftest/cases/fsl-fast-original/ (4, standard tier):
    bias-nan-outside-mask, singular-kmeans-line, fsl101-verbose-line, zstd-outputs; the suite
    `fsl-fast` loads them (difftest/permanent.py SUITES: "fsl-fast" -> fsl-fast-original).
  - Draft triage entries for relay 5's handoff (`append` op on validation/difftest/triage.toml):
    <scratch>/triage-fsl-fast-original.toml (5 entries).
  - Findings: docs/findings/fsl-fast-original.md "Random testing: what the short trial showed".
- Thread test (found a bug): the outputs' `Grid::expand` and the negative-value rule ran on
  rayon's GLOBAL pool (32 threads at n_threads 1): FIXED (`Grid::expand_on`, with_threads for the
  rule). tests/parity/test_thread_pools.py: 4 FAST programs (fMRIPrep's call, default -b -B -v,
  two channels, priors -a -A -P) pass.
- Memory test: test_fast_original_needs_no_more_memory_than_fsl (FAST_CASES at 1 thread):
  fMRIPrep's call 152 MiB vs FSL 707; bias 51 vs 139; two channels 62 vs 187 (both modes).
- Golden: 7 cases (cli.fsl.fast [fMRIPrep's call + usage], .bias, .multi, .priors, .reproduce
  [FSL-103 both modes, FSL-99 refusal], py.fsl.fast [+ a 2D plane], cli.fsl.flirt.analyze [the
  shared ANALYZE reader: big-endian, scale 0.5 not applied, pixdim -2 2 2]); new inputs
  tissue_brain(2/_bright), analyze_scaled.hdr, identity.mat; REPLICA_ONLY_FUNCTIONS emptied.
  Each case runs in 0.02-0.15 s.
- Records from the clean commit f0dd88b (once each, --jobs 3, box 24 GiB, cache use):
  **fsl-fast** (NEW docs/validation/fsl-fast.md/.json): 87 cases: 60 pass, 25 both-error, 1
  expected (S2-sizes-larger-first), 1 fail (multi/s5, explained: ill-conditioned 5x5); 77 FSL runs
  served, 23 ran (never-cached policies and signals). **fsl-bet** and **fsl-flirt** re-made:
  compare_records.py against main's: 168/168 and 305/305 cases, 0 status and 0 detail differences,
  summaries equal (the ANALYZE reader change and FLIRT's TR fix move no case).
- Smoke tier once from f0dd88b (parity_smoke.py --workers 12, box 32 GiB, LARMORX_NC_BIN = main's
  crates-nc release binary): **2692 passed, 0 skipped, 0 failed** (parallel 342 s, serial 661 s);
  marker reused by record_golden.py: 208 cases, the 7 new ones added, no existing checksum changed
  (commit e9024ec with the records).
- Gates: gates-rust all PASS (gpl and nc at ae3aaaa, the clippy fix after it touches only a larmorx-core test; root at f0dd88b after the clippy fix: a type
  alias and as_chunks in the exp check); gates-python --no-build PASS (703 passed, 13 skipped;
  ruff, format, SPDX); `tables --check` PASS.
- Spec questions added by relay 4 (for the handoff): §4.5 (the field outside the mask is
  (0 - mean) · 0: NaN over the image when the mean is NaN), §5.2 (the k-means line printed after
  the iteration's inverses: absent at a singular abort), §9.1 FSL-101 (-v's line before the crash:
  already in the spec), §5.5/§5.3 (how FSL's statistics become NaN after a NaN covariance: 1840).
  No new defect (FSL-136..140 unused).

## State (relay 3, end: sub-task 3 DONE; commits 2efa617, 907bd65, b172058, f5b8c6b, 404d147, 5a35878, 94f473b, 88ea041)
- Relays 1 and 2: sub-tasks 1 and 2 (relay 2's state below, kept).
- Relay 3 (sub-task 3, speed; every change bit-identical to fa0a5a1's build at any thread count):
  - `Grid::gauss_seidel` (fast/grid.rs): the in-place sweeps on a pipeline of planes (row (y,z)
    after plane z-1's row y+1; tasks take planes in increasing order; posteriors shared as
    AtomicU32/AtomicU8, Relaxed, the row counters Release/Acquire; serial below 4096 voxels or 1
    thread). Used by the single-channel MRF sweeps, the multi-channel sweeps and pve::icm. Unit
    test `pipelined_sweeps_are_serial_sweeps` (2, 5, 9 threads). No red-black/Jacobi order.
  - MRF update with stack scratch for 2-4 classes (single and multi); random posteriors drawn
    serially, normalised in parallel.
  - `stats::blocked_sums`/`sum_par`: FSL's blocks in parallel, block sums in order (same bits);
    `stats::sorted` par sort with ties by bits; class_stats on it; single bias field's per-voxel
    parts and the volumes in parallel; `Grid::expand` row by row in parallel.
  - multi: class_stats' independent sums one task each (Σq single, Σqy, covariances double, each
    in voxel order), bias fields' per-voxel parts in parallel.
  - `larmorx_core::math::exp_in_place` (NEW, shared crate, additive): exp's fast path 4 lanes
    (vectorised with AVX2+FMA through the audited fma dispatch, `exp` for the rest): exp's bits
    (tests `in_place_is_exp` (baseline build), `exp_in_place_is_exp` (dispatched)); used by the
    PVE likelihoods (single and multi, 64 voxels a batch) and multi's init likelihoods.
    Documented in exp.rs's module doc and larmorx-core's PROVENANCE.
  - Tried and dropped (benchmark report): skipping negligible PVE terms (9,691 of 498 M), 4-voxel
    interleave (4 %). Not done: batched expf (init, ~0.2 s of 0.89 s 1-thread), MRF's 3 exps per
    voxel batched (~6 %); the write phase is I/O-bound (0.49 s uncompressed too).
  - fMRIPrep's call, ds005003: 1 thread 8.86 -> 5.31 s, 14 threads 4.23 -> 0.96 s. Benchmark
    `docs/benchmarks/fsl-fast.md` + `.json` (bench.py; FSL's time from the cache's stored runs,
    never re-run): FSL 21.9/34.0/59.6 s, replica 15.3/23.3/31.3 s (bit-identical to FSL),
    original 1 thread 2.43/3.76/5.21 s, 14 threads 0.39/0.73/0.89 s (ds000005/ds000228/ds005003).
    Memory peak ds005003: 211 MiB (1 thread), 216 MiB (14).
  - `threads_exact.py` (committed): 13 command lines, ref build <scratch>/bin/larmorx-fa0a5a1 on 1
    thread vs the new build on 1, 3, 14: all identical (outputs, stdout, stderr).
  - FLIRT's -applyxfm TR FIXED (orchestrator's item): flirt::engine::apply writes the input's TR as
    FSL reads it: |pixdim[4]|, and 1 for dim[0] >= 4 with TR 0 (black box, applyxfm_tr.py: 11 of
    11 headers FSL's; NIfTI and ANALYZE; after a registration -out keeps the reference's TR, as
    before). Findings: docs/findings/fsl-flirt-original.md "`-applyxfm`'s TR" (table);
    fsl-fast-original.md's section updated. FAST's suite --case 'priors/|fmriprep/ds000005'
    (rec7): 12 pass, 2 both-error (priors/A-a passes now). FLIRT's suite, its 28 apply cases
    (flirt1): 25 pass, 2 both-error, 1 expected divergence: statuses identical to main's record.
  - Docs: API page (speed paragraph, n_threads row), PROVENANCE (grid pipeline, stats, exp_in_place,
    FLIRT TR), cleanroom-fast README (threads_exact.py, bench.py, applyxfm_tr.py,
    applyxfm_tr_inputs.py).
- The extension `python/larmorx/_core.abi3.so` and `target/release/larmorx` are built at
  5a35878's code (the commits after it are docs). Golden tests `-k fsl` and test_fsl_fast.py pass
  (50 + 14 with the extension); `fsl fast` still has no golden case (sub-task 4).
- Expected standard tier now (not re-run whole): rec6's statuses with priors/A-a pass: 58 pass, 23
  both-error, 1 expected, 1 fail (multi/s5, explained).
- Spec questions added by relay 3 (for the handoff): §10: an exact parallel Gauss-Seidel order
  exists (the planes' pipeline above; the spec's "wavefront" can name it); FLIRT §9.3: the TR as
  FSL reads it (absolute value; 1 for a time axis with TR 0), and `-verbose 2`'s "Resampled
  volume" TR for a 3D input (not observed). No new defect (FSL-136..140 unused).

### Relay 2's state as it left it (kept: its spec questions and the two outside cases are still open)
(relay 2, end: sub-task 2 DONE; commits 2d24f59, 9de6099, 4c4d9ed, 835f581, fa0a5a1 and the WIP between)
- Relay 1 (sub-task 1, cf32a87, d9fc2fe): one channel, every single-channel output, -v, the CLI,
  the dispatch, `lx.fsl.fast`, the suite `fsl-fast`, findings, probes.
- Relay 2 (sub-task 2):
  - The 6 open single-channel cases FIXED: the bias kernel's normalised taps stay in double
    (`fast/bias.rs`; black-box: one bias estimate and the whole default run bit-identical to FSL
    with correctly rounded maths; `conv_model.py` + `dump.patch`).
  - Several channels: `crates/larmorx-fsl/src/fast/multi.rs` (mask, log images, kernels
    σ = sqrtf(l/d), single-precision MRF weights, the init with products in DOUBLE (observed),
    members, means/covariances with larmorx's own LU (any S: -S 9+ supported, rule decision:
    nothing to refuse, the replica refuses), G, the bias fields with the weights q/C ROUNDED TO
    SINGLE (observed), MRF sweeps, FSL-98 pairing (reproduce), PVE in the log domain, no tissue
    volumes with --nopve (observed)). `-S 2` at 4 mm and `-S 2 -n 4` at 2 mm bit-identical to
    FSL+CR in the mask.
  - Priors: `cli/fast.rs::register_priors` (FLIRT's clean-room CLI in process: `cli::flirt::main`;
    -v's command lines; FSL-100 (fix: refuses -a without -A when FSLDIR is unset), FSL-101 (fix:
    no priors; reproduce: an empty mask runs on, multi with voxel 0 > 0 aborts "Out of Bounds
    (time index)", else refused), FSL-102 (fix: each name one argument, named; reproduce: the
    shell's split, `sh: 1: Syntax error: word unexpected` without FSLDIR, special characters
    refused)); single.rs: init from priors (`prior_rows`), -P in the sweeps; multi too.
  - `bet::image::read_analyze` (shared with BET's and FLIRT's originals): ANALYZE's scale field
    NOT applied; stored voxel sizes of negative product read radiologically (FSL's standard
    priors: -2 2 2). Black-box (`analyze_scale.py`, the anzmodel in the findings). FAST's
    registered priors and `priors/a-verbose` bit-identical. BET/FLIRT golden fsl tests pass
    (45). Tell the orchestrator (handoff): BET's and FLIRT's originals change on such inputs.
  - FSL-99 reproduce: -H nan's ` TOTAL <T> RHS 0 x15 ` / stderr `0.05 0.05`..`0.7 0.7` / β = 0
    (single); multi: β = 0 named (FSL reads memory never written). FSL-97, FSL-104 (corner reads
    + the percentile abort; refusals), `inv(): matrix is singular` abort, `-S 2 -n 0` outputs,
    `ERROR:: Empty mask image` counted (4n per class statistics, 1 per bias estimate, n per volumes;
    `Single::empty_sums`), `-S n` with too few words refused.
  - `lx.fsl.fast`: the 8-channel limit only for the replica; tests/python/test_fsl_fast.py
    (2 and 9 channels, the singular abort) pass (14 with BET's).
  - The suite: PENDING removed; `no_bias_field` handled (`rep._undefined_bias_field`); LXM_*
    development variables passed through. Standard tier (rec6, after every fix): **57 pass, 23
    both-error, 1 expected (S2-sizes-larger-first), 2 fail, 0 skipped**; fMRIPrep's call passes on
    ds000005 (10 s), ds000228 (13 s), ds005003 (25 s, whole case).
  - The 2 outside: `multi/s5` (an ill-conditioned 5x5 covariance: the original against itself
    with one rounding change moves further, `self_band.py` + `self_band.patch`: the record
    reports it with that reason; following LAPACK/OpenBLAS bit for bit is the replica's job);
    `priors/A-a` (only the `_stdspace` headers' pixdim[4]: FLIRT's -applyxfm writes the input's
    pixdim[4], larmorx's FLIRT the reference's: a FLIRT finding, not changed here).
  - Docs: API page (original section), PROVENANCE (FAST section), findings (7 new sections),
    investigations README (analyze_scale.py, self_band.py/.patch, conv_model.py, dump.patch,
    verbose_cr.py --full).
- Golden: `fsl fast` still has no golden case (tests/golden coverage fails until sub-task 4).
- The extension `python/larmorx/_core.abi3.so` is built at fa0a5a1's code; the binary
  `target/release/larmorx` too.
- Spec questions for the handoff (relay 5): §4.4 (second moment in double), §4.10
  (MRFWeightsTotal after each sweep), §4.8 (the ICM keeps a type; initial type 0), FSL-100 with
  -A, `_restore` 0 at NaN voxels (relay 1); §4.5 (normalised taps in double), §5.2 (the
  assignment's products in double), §5.6 (the bias weights in single; no tissue volumes with
  --nopve), §3.1/bet §3.1 (ANALYZE: no scale field; negative voxel sizes radiological), §6.4
  (FLIRT -applyxfm's pixdim[4]: FLIRT's spec), §8.3 (the empty-mask message count), §9.1 FSL-99
  (the TOTAL/RHS lines; multi's β) (relay 2). Register candidate (rule D): the single-precision
  variance in the log domain (relay 1). No new defect (FSL-136..140 unused).

## Sub-task 5's notes (done by relay 5; kept):
- The handoff must carry: the triage entries (<scratch>/triage-fsl-fast-original.toml, an
  `append` op); validation/tools.toml's fsl-fast original line (record docs/validation/fsl-fast.md:
  60 pass / 25 both-error / 1 expected / 1 fail, status as the orchestrator decides) then
  `python -m larmorx_validation tables`; golden.md (7 new cases, the ANALYZE fixture written byte
  by byte in the test, no committed data); docs/validation/difftest.md (the tool row
  `fsl-fast-original`, the trial, `LARMORX_DIFFTEST_CACHE_MODE=use` in "The oracle cache");
  next-steps item 7 (FAST's original done but the site); the API page / handoff: `-S 9` and more
  supported by the original (the API page says so, line 17); the shared code changed in the
  branch: difftest/runner.py (cache mode), difftest/permanent.py (SUITES), difftest/tools/
  __init__.py, tests/golden/registry.py, tests/parity/test_memory_peak.py, parity/thread_pools.py,
  larmorx-core exp.rs (an ignored test only).
- Tell the orchestrator: the BET and FLIRT records re-made from f0dd88b are identical to main's
  (compare_records: 0 differences), so the shared ANALYZE reader (relay 2) and FLIRT's TR fix
  (relay 3) move no recorded case; the golden case cli.fsl.flirt.analyze pins the reader.
- The trial ran from a dirty tree (e60046f-dirty: the descriptor and the cache mode uncommitted);
  its findings were replayed after the fixes from committed code.
- Site panels: the record's cases, fMRIPrep's three calls, the speed table
  (docs/benchmarks/fsl-fast.md), the trial's numbers; the evidence module evidence/fsl_fast.py.

## What specs/fast.md says that the next relays need (relay 1's summary)
- §2 command line: options read until the first word not starting with `-`; the images are the
  LAST words (`-S n`: channel 1 last, channel 2 before it...); one-letter options joinable; a
  long option's value only after `=`; integers base 0 (`strtol`), floats `strtod` rounded to
  single. Usage text (§2.3) on stderr, exit 1, for no args, `-h`, and before an option error
  (`\n<opt>: Option doesn't exist!` / `Couldn't set_value! valstr="<v> <next>"!` /
  `Missing non-optional argument!`). `-H < 0`: `ERROR: Segmentation smoothness must be
  positive. Exiting`. `-S <= 0`: `At least 2 channels required for Multi Channel Segmentation`
  (no newline), exit 1. Exit 255 + stdout `Exception: Not enough classes detected to init
  KMeans` when two initial means are equal. Aborts (134) -> the original prints FSL's lines
  then `larmorx fsl fast: FSL aborts here (signal 6); exit status 1 (larmorx)` (common::abort).
- §3 images: read as BET's (names, formats, radiological frame, 4D -> first volume + the
  deprecation warning); outputs in FSLOUTPUTTYPE (checked at the first write, after the whole
  computation and the verbose volumes: observed), channel 1's header, cal 0/0; int32 for
  `_seg`, `_seg_<i>`, `_pveseg`, `_mixeltype`, float32 for the rest. Output order: `_seg`,
  `_seg_i` (-g), `_prob_i` (-p), `_pve_i`, `_pveseg`, `_mixeltype` (not --nopve), `_restore`
  / `_restore_<c>` (-B), `_bias` / `_bias_<c>` (-b); with -a also `<out>_{csf,gm,wm}_stdspace`.
- §4 one channel (DONE in relay 1, module `crates/larmorx-fsl/src/fast/single.rs`, `pve.rs`).
- §5 several channels (sub-task 2): mask = every channel > 0 (no FSL-103); kernels
  `σ = sqrt(l / d)` (single); MRF weights' distances in single; init means per channel
  (percentiles, or `-s` with exactly n*S numbers, channel 1's classes first); hard assignment by
  the smallest `D_c = Σ_i Σ_j (y_i - μ_ci)(y_j - μ_cj)` (square of the sum, not sum of squares);
  only `-W` init iterations (not -O + -W); members `q = w p / Σ w p` with w = 1/n drive means,
  covariances (q-weighted, Σq in SINGLE), the bias field (from the CURRENTLY corrected y_b, no
  mean removed, R_s = 0 / W_s = 1 where outside or W_s = 0, each estimate replaces the previous)
  and the stats; likelihood `G = ½ dᵀC⁻¹d + log(sqrt(|det C (2π)ⁿ|))` with `(2π)ⁿ` one factor at
  a time in single; MRF: no random restart, five sweeps from the current posteriors, `expf` of
  `βU - G` in single, sums single; main loop: bias once; -I x (MRF, members, bias, stats); -O x
  (MRF, members, β <- H, stats); then exp(-b_i) per channel, class sort by channel 1's means
  (FSL-98: FSL's inverse-permutation pairing in reproduce), hard seg (largest w p), members,
  PVE on the LOG images with the last stats (§5.8: single-precision fraction steps; pure-class
  likelihood for n=3 from §5.4, for n = 2 and n > 3 a single-precision G'; mixed: μ = δμa +
  (1-δ)μb, C = δ²Ca + (1-δ)²Cb (double), det and inverse per fraction; energy dᵀC⁻¹d + logf(det)
  by solving C z = d; n = 1: every pve 0 and `_pveseg` 1). Verbose: `Starting Multi Image
  Segmentation`, `<i> <c> <μ_ci>`, `KMeans Iteration k`, `Tanaka Iteration ...` (no inner-sweep
  lines), ` CHANNEL i CLASS c MEAN .. STDDEV ..` (double), ` BETA`, `Partial Volume Estimation `
  (no leading space), tissue volumes. `-S 9+`: the replica refuses; the original may support
  them (decide by rules A-D; the thresholds do not need OpenBLAS's bits). Up to 8 x 8 the
  original's own LU (any correct method) is fine.
- §6 priors (sub-task 2): `-A` three names else `$FSLDIR/data/standard/tissuepriors/avg152T1_{csf,
  gray,white}`; `-a` with n not 2 or 3: `Apriori can only be used for 2 or 3-class segmentation`
  (stderr) and no priors; a missing prior: `prior image <name> is not found! priors are not
  used!`; register with FLIRT's `-applyxfm -init <mat>` (trilinear, the prior's data type,
  outputs `<out>_{csf,gm,wm}_stdspace` in FSLOUTPUTTYPE; larmorx in process with
  `crates/larmorx-fsl/src/flirt`, FSL-09's rounding applies by default); `-v` prints each
  `<FSLDIR>/bin/flirt -ref ...` command first; a missing registered prior: `<tissue> prior image
  not transformed correctly! priors are not used!` then FSL crashes (FSL-101). Init from priors:
  p = (π_csf, π_gm, π_wm)/s (two classes: (π_csf, π_gm + π_wm)/s; s <= 0: 1/n); no init
  iterations; then hard seg and stats. `-P`: each MRF update times π_c.
- §7 messages table; `ERROR:: Empty mask image` (stderr) from a sum over an empty mask (an image
  with no voxel above 0 and `-s` means given): NOT DONE in relay 1 (how many times FSL prints it
  needs a black-box run: `-s @t1_means.txt -o seg @zeros.nii.gz`).
- §8: rand (DONE: `fast/rng.rs`, glibc black-box-checked); loop orders storage order in the
  radiological frame; single channel in single precision; blocked sums (DONE: `stats.rs`);
  percentiles (DONE); kernels and the separable convolution (DONE: `bias.rs`, lines/slices in
  parallel); multi-channel matrix arithmetic need not follow OpenBLAS.
- §9 defects: FSL-96 (prob maps outside the mask: 0, named; DONE), FSL-97 (multi -N -b/-B: no
  field), FSL-98 (multi sort pairing), FSL-99 (DONE for the fix refusal and n < 1 in reproduce;
  NOT DONE: reproduce's arithmetic for NaN/inf -H/-f/-R and -l <= 0: `-H nan` makes FSL print
  ` TOTAL ... RHS 0 0 ...` on stdout and `0.05 0.05|0.1 0.1|...` on stderr, observed: the
  "table of zeros" estimate of β), FSL-100 (DONE: reproduce aborts, fix segments and names it;
  with -a and no -A the fix must refuse "name the priors with -A": sub-task 2), FSL-101, FSL-102
  (priors), FSL-103 (DONE: the mask, both modes, named), FSL-104 (multi sizes). §9.2 rule C/D
  items kept as FSL's.
- §10 speed (sub-task 3): MRF sweeps and the mixel ICM are Gauss-Seidel in storage order; the
  per-voxel parts already run in parallel (likelihoods, members, statistics' inputs); the PVE's
  3 x 101 `exp` per voxel is the next hot spot; FSL ~25 s, replica ~17 s single-threaded on
  fMRIPrep's call.
- §11 thresholds (the suite `validation/src/larmorx_validation/parity/fsl_fast.py`): exit,
  messages, file names identical (verbose numbers to 4 significant digits); `_seg`, `_pveseg`,
  `_mixeltype`: per-class Dice >= 0.999 and >= 99.9 % identical; `_pve`: mean |d| <= 1e-3, p99.9
  <= 0.02, totals within 0.1 %; `_prob`: mean <= 1e-3; `_bias`, `_restore`: rel <= 1e-4 at p99.9;
  verbose means and variances rel <= 1e-5. The record should report the last-bit band (§11).

## Clean-room statement (grows relay by relay)
Relay 1 opened: CLAUDE.md (loaded), the brief, `specs/fast.md` (whole), larmorx's own code of
`crates/larmorx-fsl` (lib.rs, cli/mod.rs, cli/common.rs, cli/fmt.rs, cli/options.rs, cli/bet2.rs
parts, bet/image.rs, bet/mod.rs parts, flirt/engine.rs's image writer, PROVENANCE.md's head),
`crates/larmorx-cli/src/{replica.rs,lib.rs}`, `crates/larmorx-py/src/fsl.rs`,
`crates/larmorx-core/src/{defects.rs (FSL-96..104 rows), parallel.rs, math/mod.rs}`,
`crates/larmorx-mri/src/hmc/files.rs` (strip_image_extension), `python/larmorx/fsl/{fast.py,
__init__.py,flirt.py,bet.py parts}`, `python/larmorx/_replica.py` parts, `_core.pyi` parts,
`tests/python/test_fsl_fast.py`, `tests/golden/registry.py` (REPLICA_ONLY_FUNCTIONS),
`tests/parity/test_fsl_fast_replica.py`, `validation/src/larmorx_validation/parity/
{fsl_fast_replica.py (whole: cases and mechanics), fsl_flirt.py (parts), defects.py (parts),
harness.py (parts)}`, `validation/src/larmorx_validation/__main__.py`, `docs/validation/
oracle-cache.md` (the key), `validation/investigations/cleanroom-flirt/{README.md, probe.py
head}`, `scripts/orchestration/{env.sh,lxv.sh,boxed.sh}`. It listed the folder names of
`validation/investigations/` once (`ls`, names only, no file opened). It never opened FSL's
source, `crates-nc/`, the restricted findings, `docs/parts/`, the replica parts' investigations,
`validation/probes/`, triage.toml, the replica's permanent cases or any replica/spec-writer
history.
Relay 1 ran: FSL 6.0.7.23's `fast` as a black box only through the oracle cache in `use` mode
(the replica suite's keys; `validation/investigations/cleanroom-fast/{probe.py,fsl_streams.py}`):
the replica suite's cases (served from the cache) and new command lines on its derived inputs:
`-v -o seg @const.nii.gz`, `--nopve -v -I 1 -O 1 -W 1`, `-v -N -I 1 -O 1 -W 1`, the latter with
`FSLOUTPUTTYPE=BOGUS`, `-v -I 0 -O 0 -W 0 -n 2 -p`, `-gp`, `--class 2`, `-n 0x2 -I 010`
(t1_half / mni4_t1); glibc's `rand` through ctypes (srand(0xFFFFFFFF), srand(0)); numpy models
of the initialisation (`<scratch>/py/initmodel.py`, kept as
`validation/investigations/cleanroom-fast/variance_model.py`). Also through the cache: FSL with
the CORE-MATH preload (`LD_PRELOAD`, keyed by content) on fMRIPrep's calls of ds000005 and
ds005003, opt/bias-outputs, opt/2d, opt/big, opt/type2-verbose, `-v -N -I 1 -O 0 -W 1..3`,
`-v -I 2..4 -O 0..1 -W 1`, `-b -I 0 -O 0 -W 1 --nopve`, `-p -o seg @t1_2d.nii.gz`, and the
suite's band runs; FSL as built on `-B -b -N --nopve @t1_big.nii.gz`, `-v -g @t1_big.nii.gz`,
and the suite's `never` cases (opt/nan, opt/n4-probs: FSL reruns, never cached by the replica
suite's policy). The suite fsl-fast's standard tier ran 4 times (development, not a record).
Memory boxes: at most 24 GiB at once (one box at a time); threads: cargo -j 14, suite --jobs 3 x 4.
Relay 1 also opened `docs/api/fsl-fast.md` (its own page) and `docs/validation/oracle-cache.md`.

Relay 2 opened: the brief, NOTES.md, `specs/fast.md` (whole), larmorx's own code of
`crates/larmorx-fsl` (fast/*, cli/fast.rs, cli/mod.rs (Settings, run_with), cli/common.rs
(abort, image_exception), cli/bet2.rs (read_image, resolve_image), cli/flirt.rs (main's
signature, grep of slope), bet/image.rs (Input, Output, Values, read_first, read_analyze),
flirt/engine.rs (read path 80-130, world() 180-215, output_type/grouped_type/stored_type
2250-2330, -applyxfm's pixdim[4] 2636-2648, 2850-2870), flirt/mod.rs (grep), PROVENANCE.md (FAST
section)), `crates/larmorx-mri/src/hmc/image.rs` (voxel_size, scaling), `crates/larmorx-core/
src/defects.rs` (grep of the FSL_96..104 lines), `python/larmorx/fsl/fast.py`,
`tests/python/test_fsl_fast.py`, `validation/src/larmorx_validation/parity/{fsl_fast.py,
fsl_fast_replica.py (cases, Run, oracle_run, run_oracle)}`, `validation/src/larmorx_validation/
__main__.py` (the --case lines), `validation/investigations/cleanroom-fast/*` (own),
`docs/api/fsl-fast.md`, `docs/findings/fsl-fast-original.md` (own). It imported (not opened)
`fsl_flirt_replica._environment` and `fsldir` in `analyze_scale.py` (as relay 1's suite does). It
ran `git log --oneline -3` on `crates/larmorx-fsl/src/bet/image.rs` (three FLIRT clean-room
commit titles; no replica or spec-writer history). It read the header bytes of FSL's
`data/standard/tissuepriors/avg152T1_csf.hdr` (an image, not source) and listed that folder.
It never opened FSL's source, `crates-nc/`, the restricted findings, `docs/parts/`, the replica
parts' investigations, `validation/probes/`, triage.toml, the replica's permanent cases or any
replica/spec-writer history, and listed or searched no folder as a whole.
Relay 2 ran: FSL 6.0.7.23's `fast` only through the oracle cache in `use` mode: the suite's
standard tier (rec2: 6 cases, rec3: multi/priors/err, rec4, rec5, rec6: all; FSL served from
the cache except the `never` cases, which FSL reruns each time); new FSL runs (cached):
`-H nan -v` (mni4_t1), `-s t1_means.txt` on zeros with 9 option sets, `-S 2` with mni4_t1 twice;
new `never` runs: `-a nothere.mat -v` on zeros (FSL-101), `-S 2 -H nan -v` (memory never
written); FSL with the CORE-MATH preload (cached): t1_half with `-p -N -I 0 -O 0 -W 2/3 --nopve`,
`-b -I 0 -O 0 -W 1 --nopve` (with -n 1, -n 2, -W 0, -l 1/3/5), `-b -B -v`, `-v -g -p`; the MNI
images with `-S 2` (4 mm: -W 0/1/2/15, -I 1, -b -l 1/9/16, -B -b, -n 4, -n 2; 2 mm: -n 4 with
-W 0/1/3/15, -I 0/1, -b, -v -W 0/1, -n 3 -v -W 1), `-S 5` (-W 0/1/15 -p, -B -b, -v), band.py on
multi/s5, s2-n4 (twice), s2-quarter; FSL's `flirt` through the cache (`analyze_scale.py`: the
CSF prior with the identity, default and -datatype float). The original: the binary
`target/release/larmorx` (LARMORX_FAST_BIN) and the wheel's extension. Memory boxes: at most
24 GiB at once (one 16 GiB band box beside one 8 GiB probe box); threads: cargo -j 14, the
suite --jobs 3 x 4.

Relay 3 opened: the brief, NOTES.md, `specs/fast.md` (its headings, §8.2 to §8.7, §10, §11),
`specs/flirt.md` (§9.3, lines 989-1010, and a grep of `applyxfm` in it); larmorx's own code:
`crates/larmorx-fsl/src/fast/*` (all), `cli/fast.rs` (run's threading, write_outputs), `lib.rs`
(head), `Cargo.toml`, `bet/image.rs` (write, 221-332), `flirt/engine.rs` (a grep of pixdim;
2377-2393, 2500-2525, 2625-2660, 2740-2760, 2845-2875), `PROVENANCE.md` (FAST section);
`crates/larmorx-core/src/{parallel.rs, lib.rs head, math/mod.rs (dispatch, fma module, tests),
math/exp.rs (constants, helpers, exp's fast path, tests)}`, `crates/larmorx-core/PROVENANCE.md`
(a grep of exp); `crates/larmorx-io/src/nifti/mod.rs` (640-720), `gzip.rs` (a grep);
`validation/src/larmorx_validation/parity/{fsl_fast_replica.py (fmriprep_calls, fmriprep_input,
fmriprep_args, run_oracle, _fmriprep_case, the multi and priors cases), fsl_fast.py
(original_command, run_original), fsl_flirt.py (original_command, run_original)}`,
`oracle_cache/core.py` (Completed, _replay, _settle, run, _run's serving lines) and the folder's
file names, `__main__.py` (--case); `docs/validation/oracle-cache.md` (a grep, lines 140-150);
`validation/investigations/cleanroom-flirt/` (file names, probe.py whole);
`validation/investigations/cleanroom-fast/*` (own); `docs/benchmarks/fsl-flirt.md` (lines 1-60,
its model) and the folder's file names (grep flirt/bet/fast); `docs/findings/fsl-flirt-original.md`
(its headings and last 30 lines; the clean-room's own findings), `docs/findings/fsl-fast-original.md`
and `docs/api/fsl-fast.md` (own); `docs/validation/fsl-flirt.json` (statuses, read by a script;
the file names of `docs/validation/` matching fsl-flirt); `tests/python` file names (grep fsl).
It ran `fsl_flirt_replica._all_cases()` in Python to list the names of FLIRT's 28 cases with
-applyxfm (names only). It read the header of FSL's `data/standard/tissuepriors/avg152T1_csf.hdr`
(an image). It never opened FSL's source, `crates-nc/`, the restricted findings, `docs/parts/`,
the replica parts' investigations, `validation/probes/`, triage.toml, the replica's permanent
cases or any replica/spec-writer history, and listed or searched no folder as a whole.
Relay 3 ran: FSL 6.0.7.23 only through the oracle cache in `use` mode: new runs of FSL's `flirt`
(cached): 4 command lines of FLIRT's probe.py on applyxfm_tr_inputs.py's images (-applyxfm 3D,
-applyisoxfm 3D, -applyxfm 4D, -dof 6 -out) and applyxfm_tr.py's 11 variants; served from the
cache: FAST's suite --case 'priors/|fmriprep/ds000005' twice (rec7; its 3 `never` cases reran
FSL each time, as their policy says), FLIRT's suite on its 28 apply cases (flirt1: all served),
FAST's probe.py --case priors/a-n2, the benchmark's three FSL calls (bench.py: their stored
times). The replica (the main checkout's `crates-nc/target/release/larmorx-nc`, read only, as a
black box): the benchmark's three calls. The original: `target/release/larmorx` (profiling with
temporary timers never committed, threads_exact.py, the suites, the benchmark) and the wheel's
extension (pytest tests/python/test_fsl_fast.py, tests/golden -k fsl). A scratch crate
(<scratch>/mb) timed `exp` and `exp_in_place`. Memory boxes: one at a time, at most 16 GiB;
threads: cargo -j 14, suites --jobs 3, the original at most 14.

Relay 4 opened: the brief, NOTES.md, `specs/fast.md` (its headings, §3.3, §4.5, §5.3, §5.5,
§6, §9.1, §11, and a grep of `restore`/`members`/`posterior` in it); larmorx's own code:
`crates/larmorx-core/src/math/{exp.rs (fast path, exp_in_place, tests), mod.rs (dispatch, fma)}`,
`crates/larmorx-core/src/parallel.rs` (with_threads), `crates/larmorx-fsl/src/fast/{grid.rs,
single.rs (negative_rule, maximum, bias_field, Single), multi.rs (members, class_stats, invert,
likelihoods, MRF update, k-means loop)}`, `cli/fast.rs` (FSL-101's branch, write_outputs),
`PROVENANCE.md` (FAST section), `stats.rs` (a test), `python/larmorx/fsl/fast.py` (signature,
FastResult); `tests/golden/registry.py` (helpers, inputs, FSL cases), `tests/golden/
test_golden.py`, `tests/parity/{test_thread_pools.py, test_memory_peak.py}`,
`validation/src/larmorx_validation/{difftest/tools/{fsl_fast.py, fsl_flirt.py (its original's
parts), __init__.py}, difftest/{runner.py, cli.py, permanent.py, core.py (Tool)}, parity/
{fsl_fast.py, thread_pools.py, fsl_mcflirt_replica.py (the image readers), fsl_fast_replica.py
(run_oracle's signature), fsl_bet.py and fsl_flirt.py (their binary variables, by grep)},
evidence/difftest.py (the triage matcher), __main__.py (grep)}`, `docs/validation/difftest.md`
(running it, memory, the cache, triage), `docs/findings/fsl-fast-original.md`,
`docs/api/fsl-fast.md` (grep) (own), `validation/investigations/cleanroom-fast/*` (own),
`scripts/orchestration/{gates-rust.sh, compare_records.py, README.md}` (parts), the committed
records' headers (`docs/validation/fsl-bet.json`, `fsl-flirt.json/.md`), the names of
`validation/src/larmorx_validation/difftest/tools/` and `validation/difftest/cases/` (folder
names only; no replica permanent case opened). It never opened FSL's source, `crates-nc/`, the
restricted findings, `docs/parts/`, the replica parts' or spec writers' investigations,
`validation/probes/`, `validation/difftest/triage.toml`, the replica's permanent cases or any
replica/spec-writer history, and listed or searched no folder as a whole.
Relay 4 ran: FSL 6.0.7.23's `fast` and `flirt` only through the oracle cache in `use` mode: the
trial's 9,005 cases (`LARMORX_DIFFTEST_CACHE_MODE=use`; their FSL runs stored), the replays of its
182 findings (four times, at most 8 at a time) and `probe.py --finding` on about 25 of them
(served), the suite's permanent cases (rec8), the memory tests' FSL peaks (served), the records
fsl-fast, fsl-bet and fsl-flirt once each (served but the never-cached policies and signals), the
smoke tier once (12 workers). The replica (main's `crates-nc/target/release/larmorx-nc`, read only,
as a black box) in the BET record and the smoke tier. The original: `target/release/larmorx` and
the wheel's extension. Memory boxes at once: at most 12 (trial) + 8 (thread/memory tests) GiB, then
one at a time (24 for replays and records, 32 for the smoke tier, 16 for the gates); threads: cargo
-j 14, the trial 3 jobs x 4, the records --jobs 3, the smoke tier 12 workers.

Relay 5 opened: the brief, NOTES.md, `docs/figures.md`; in the site clone its CLAUDE.md, README,
FAST's page, `data/verdicts.yml`, the FAST lines of `site.yml`, `content/roadmap.md` and
`data/status.yml`, the evidence calls and facts of the BET and FLIRT pages (grep), `tools/build.py`
and `tools/review/shoot.mjs` (options), `tools/evidence.py` (grep); larmorx's own code:
`evidence/{fsl_fast.py (whole), common.py, core.py (compare), example.py (Panel), summary.py,
__init__.py, fsl_bet.py (its speed, cases and the original's case verdict), ants_n4.py (its
verdict function)}`, `figures/{cache.py, records.py}`, `parity/fsl_fast.py` (own), `parity/
fsl_fast_replica.py` (Run, oracle_run, run_oracle, the fMRIPrep helpers, _prepare,
_clear_unwritten, _undefined_bias_field, the start of the case lists), `parity/defects.py`,
`parity/fsl_mcflirt_replica.py` (replica_version, _files, _is_image), `difftest/{permanent.py,
runner.py}` (parts), `tables.py`, `scripts/orchestration/{apply_handoff.py, README.md (50-75)}`,
`validation/investigations/cleanroom-bet/site_inventory.py` (head) and `cleanroom-flirt/README.md`
(a grep of "inventory"); the records `docs/validation/fsl-fast.md/.json` (own),
`fsl-fast-replica.json` (case ids, by a script), `docs/benchmarks/fsl-fast.md/.json`,
`docs/api/fsl-fast.md`, `docs/findings/fsl-fast-original.md` (own); the shared documents by line
range or a grep of FAST only: the register's summary rows FSL-96 to FSL-104 (lines 40 to 400, the
`| FSL-` rows), `docs/api/upstream-defects.md` (those rows), `docs/validation/golden.md` and
`docs/next-steps.md` (FAST's lines), `docs/validation/difftest.md` (the tools table, "The oracle
cache"), `validation/tools.toml` (FAST's and FLIRT's blocks), `tests/golden/registry.py` (FAST's
cases), `tests/parity/test_memory_peak.py` (FAST_CASES); `git diff --stat a173af5..HEAD` limited to
the branch's shared Apache paths. It never opened FSL's source, `crates-nc/`, the restricted
findings, `docs/parts/`, the replica parts' or spec writers' investigations, `validation/probes/`,
`validation/difftest/triage.toml` (the handoff's `--check` read it in memory, nothing printed), the
replica's permanent cases (the evidence code loads them for the case explorer, as before; never
printed) or any replica/spec-writer history, and listed or searched no folder as a whole.
Relay 5 ran: FSL 6.0.7.23's `fast` only through the oracle cache in `use` mode: for the page's six
examples FSL with the CORE-MATH preload (5 served, 1 run: ds000228's call); for the classification
of the original's 60 passing cases FSL as built (54 served, 6 rerun by their never-cached policy)
and with the preload (20 served, 40 run and stored but the never-cached ones); FSL and the replica
as built not otherwise (their entries existed). The original: `target/release/larmorx` (f0dd88b's
code) in both modes at 14, 4 and 1 threads. Exports four times (no FSL run). Memory boxes: one at
a time, 12 GiB (the entries, 3 workers x 4 threads), 16 GiB (the exports); local servers for the
site checks on ports 8797 and 8798, stopped at the end.

## FROM THE ORCHESTRATOR (2026-11-03 ~01:40; keep this section in every rewrite)
- (2026-11-03 ~02:40) `larmorx_core::math::exp_in_place` (relay 3's AVX2+FMA batch `exp`) is shared core code that
  every platform's bits depend on: the AVX2 path must give the scalar `exp`'s bits on every input, or Linux x64 and
  the platforms without AVX2 (aarch64, which run the scalar path) part. In sub-task 4, before the records: a
  randomized comparison of the batch against the scalar `exp` (at least 10^9 doubles: uniform bit patterns over the
  whole finite range, dense samples over the range FAST uses, the subnormal and overflow thresholds, ±0, ±inf, NaN,
  the values next to every boundary the algorithm branches on), boxed, every difference a failure; committed as a
  probe with its README and counts; and one golden case whose output goes through the batch path.

- The shared reader `bet::image::read_analyze` changed in relay 2 (the scale field not applied; stored voxel sizes
  with a negative product read without the x reversal): sub-task 4 re-makes BET's original record (`fsl-bet`) and
  FLIRT's (`fsl-flirt`) from the final clean commit and compares each with `compare_records.py` against main's
  committed ones (identical expected; any difference is explained in the findings and the handoff), and adds one
  golden or suite case with such an ANALYZE input if none covers it.
- (DONE in relay 3, 5a35878) FLIRT's clean-room original writes `-applyxfm`'s header `pixdim[4]` from the reference where FSL writes the
  input's (relay 2's `priors/A-a`): fix it in FLIRT's original (a clean-room file, yours to edit; black-box evidence
  in `docs/findings/fsl-flirt-original.md`), in sub-task 3 or 4, before the records; the `fsl-flirt` record above
  then shows only that change (explained).
- `-S 9` and more supported by the original (relay 2's decision by the rules: the input makes sense, FSL segments
  it; the replica refuses only because it cannot follow OpenBLAS's bits): say so in the API page and the handoff.
