# FAST replica: relay notes

Worktree: <workspace>/larmorx/.claude/worktrees/agent-ac6eb2fb8c3621cf1
Branch: worktree-agent-ac6eb2fb8c3621cf1 (fast-forwarded by relay 1 from 2ae6873 to main 0bed438)
Commits: f747b28 (relay 1), 137e648 (2a), cbe9f06 + d269d58 + 22c4ec8 (2b), 887ef5b + cb7d5d1 (3)
Scratch (<scratch>): <scratch>/

## State
- Relay 1 (sub-task 1, the core and fMRIPrep's call): DONE, commit f747b28.
- Relay 2a (the multi-channel segmentation): DONE, commit 137e648.
- Relay 2b (every other option, the priors, the D13 decisions, -S 5 to 8): DONE, commits
  cbe9f06, d269d58, 22c4ec8 (details below, "Relay 2b").
- Relay 3 (sub-task 3, the contract): DONE, commits 887ef5b, cb7d5d1 (see "Relay 3"). Gates run
  (log oracles/logs/fast-3-gates.log): crates-nc fmt, clippy -D warnings, tests (all pass but
  golden: only "not recorded" fast.* lines); root fmt, clippy + tests of larmorx-cli and
  larmorx-core; wheel rebuilt; ruff, format, SPDX; pytest tests/python tests/golden
  validation/tests (671 passed, 12 skipped). Not run: smoke tier, records (relay 4).
- Relay 4 (sub-task 4, random testing and the records): DONE, commits f217c6a, b42aa20, bb068f1,
  d1be4c9, 85e98c7 (details: "Relay 4: what was done" below). Records from bb068f1 (clean):
  fsl-fast-replica 86 cases: 59 pass, 26 both-error, 1 expected (FSL-104), 0 fail; FSL 64 of 86 runs
  from the cache, 22 run (10 never cached) -> committed in d1be4c9 with the generated tables;
  mcflirt 237/237, BET 178/178, FLIRT 302/302 re-made into <scratch>/records: compare_records
  identical (status and details) to the committed ones (not committed, as FLIRT part 3 did for
  BET). Smoke tier from bb068f1 (boxed 32, 12 workers): 2660 passed (parallel 345 s + serial
  651 s), marker oracles/cache/markers/parity-smoke-bb068f1...json. Logs:
  oracles/logs/fast-4-{trial,record-*,smoke,priors,clippy,nctest2}.log. Nothing left running.
- Relay 5 (sub-task 5, the site page and the handoff): DONE. larmorx commits 584017e, 7051a1c,
  ef91a76, 190cbac, da6ce74 (the evidence module `evidence/fsl_fast.py` + TOOLS + summary PAGES;
  the API page's status). Site clone <scratch>/site, branch fsl-fast-page 97101ab (page + JSON +
  manifest), local evidence-blobs 6977211 (689 blobs). Figures cache: oracles/figures-cache/fsl-fast/
  (7 entries; the 6 FSL runs served from the oracle cache). Checks: build 0 problems, numbers
  136/136, viewers 195/195, inventory 0 problems (<scratch>/inventory-{before,after}.json), shots
  <scratch>/shots/final/. Gates: ruff, format, SPDX, tables --check, pytest 672 passed.
  Handoff: <scratch>/HANDOFF.toml (23 ops, --check ok at da6ce74), <scratch>/HANDOFF.md. The final
  report returned to the orchestrator (REPORT.md refused by the harness: report text only).
  Nothing left running (servers stopped; the main worktree of the site removed).
## Relay 3: what was built (887ef5b, cb7d5d1)
- `specs/fast.md` (new, 750 lines): command line (syntax, every option, the usage text verbatim,
  option errors, exit statuses), images and outputs, the single-channel algorithm (mask/log, MRF
  neighbourhood, k-means initialisation, means/variances, bias field, main loop with random
  restarts and beta's schedule, class sort, hard segmentation, partial volumes with mixel types,
  ICM and fractions, members/-p, verbose texts), the multi-channel differences, the priors,
  messages table, numeric conventions (glibc rand per Selinger, single vs double per call, FSL's
  blocked sums and percentiles, separable convolution, Armadillo/OpenBLAS, the fraction
  sequences), D13 table FSL-96..104, rule C / rule D lists, -S 9+ not replicated, speed, the
  validation plan (thresholds to calibrate against the replica with correctly rounded maths).
  Facts probed on FSL: `validation/probes/fsl-fast-cli/` (probe.sh + README; run boxed).
- `lx.fsl.fast` (`python/larmorx/fsl/fast.py`: `FastResult`, every option, channels in channel
  order (written last-first on the command line), priors (matrix, -A images, fsldir for the
  standard priors), manual_means, n_threads, implementation, upstream_defects; FSLDIR set to the
  call's tmp dir when no fsldir: no FSL-100 notice); exported in `lx.fsl`;
  `tests/python/test_fsl_fast.py` (stub replica: 5 tests). No `_core.pyi` change (no new
  `_core` function).
- `larmorx fsl fast`: `crates/larmorx-cli` REPLICAS entry (original: false) + FSL_TOOLS +
  tests; `tests/golden/registry.py` REPLICA_ONLY_FUNCTIONS `lx.fsl.fast`.
- larmorx-nc golden (`crates-nc/larmorx-nc-cli/tests/golden.rs`): 9 FAST BetCases (fast.fmriprep,
  .bias, .t2-n2-nopve-verbose, .n4-manualseg, .multi, .priors (inputs copied in: `@name` token),
  .fsldir-unset, .fsl103, .fsl103.reproduce) on a new `brain` input (3 tissues, x gradient), a
  `big` one (x 2^15, float) and `means4.txt`; FAST_REFUSALS (fast.fsl99.nan, fast.fsl104.sizes)
  through the shared refusal loop; "fsl fast" in PRINTING (stdout.txt kept); run_bet sets FSLDIR
  to the run dir for fast (except fast.fsldir-unset). `every_tool_has_a_golden_case` passes;
  `golden` FAILS only with "not recorded" for the fast.* cases: RECORD them (relay 4).
- `tests/parity/test_memory_peak.py` FAST_CASES (fmriprep on ds000005, bias on mni_t1, two
  channels): larmorx 538/117/127 MiB vs FSL 707/139/187, reproduce outputs identical (but
  `_prob_`); `thread_pools.py`: fsl fast (bias) and two channels: pass. Log
  `<workspace>/oracles/logs/fast-3-memthreads.log`.
- Suite: STANDARD_RUNS + ds000228-sub-pixar008, ds005003-sub-RS001: both pass (bit-identical);
  log `oracles/logs/fast-3-fmriprep.log` (2 FSL runs now in the oracle cache).
- Replica changes: `-S 9+` refusal now `larmorx-nc: error: fast: -S <n>: more than 8 channels
  are not replicated (...)` exit 1 in both modes (crate test in tests/fast.rs); FSL-96 also named
  when `members` was never written at all (single channel, --nopve with priors or -W 0 -O 0:
  `Segmentation::members_written`); `larmorx_core::defects` FSL-96 text extended accordingly.
- Docs: `docs/api/fsl-fast.md` (links the record docs/validation/fsl-fast-replica.md, which
  relay 4 writes), `docs/findings/fsl-fast.md` (restricted), PROVENANCE (-S 9+ decision).

## For relay 4
- Build: the root `target/` is now seeded (copied from the main checkout); the wheel's
  `_core.abi3.so` is in `python/larmorx/` (rebuilt by relay 3's gates). Rebuild it with
  `maturin build --release -j 14 -o <scratch>/wheel` after any root change.
- Difftest descriptor (realistic cases only), trial <= 30 min boxed, triage, replays boxed.
- Then from the final clean commit, once each: the standard-tier record (`--out docs/validation`
  -> fsl-fast-replica.md/.json), the parity smoke tier, `record_golden.py` (reuses the smoke
  marker), the larmorx-nc golden record (`LARMORX_GOLDEN_RECORD=1 cargo test --release -p
  larmorx-nc-cli --test golden` in crates-nc, as the earlier nc records were made: check
  docs/validation/golden.md), and mcflirt's, BET's and FLIRT's replica records (core changed:
  compare_records identical). `validation/tools.toml` entry for fsl-fast-replica (handoff or
  branch? tools.toml is a generated-table source: an agent edits it in its branch).
- Keep the lapack_probe numbers (388/420, NaN payloads only) in the record or findings
  (already in docs/findings/fsl-fast.md).

## Relay 2b: what was built (cbe9f06, d269d58)
- Suite `fsl-fast-replica` standard tier (final run, from 22c4ec8's code): 81 cases, 57 pass,
  23 both-error (FSL and the replica exit alike, every file identical), 1 expected divergence
  (err/S2-sizes-larger-first: FSL-104, refused in both modes), 0 fail. Oracle cache: 61
  served, 20 run (the never-cached: aborts, crashes, `-p`'s unwritten memory). Log:
  `<workspace>/oracles/logs/fast-2b-run4.log`. Derived inputs v9 (v8's plus mni_sum, mni_sqrt,
  mni_prod, mni_mix for -S 5 to 8); earlier: v8 (`oracles/inputs/fsl-fast-replica/`): t1_2d, t1_4d, t1_offset, t1_negvox,
  t1_nan, t1_int16, t1_big (max 2^25), const, zeros, t1_means{,_cycle,_two}.txt, std2mni.mat
  (avg152T1 -> MNI152NLin2009cAsym 2 mm: translation 5.5 6.5 6.5, world-aligned), ident.mat,
  tpl_{csf,gm,wm} (ds000005's fMRIPrep `space-MNI152NLin2009cAsym_res-2` probsegs: the
  templateflow probsegs are empty placeholders here), mni_means_cycle.txt (T1w + ratio),
  phantom_t1/t2 (opposite contrasts).
- Kinds of cases: `opt/` (single-channel options and odd inputs, t1_half), `err/` (messages,
  exit statuses, failure cases, rule-B refusals), `priors/` (-a/-A/-P single and multi),
  `multi/s2-opposite-contrasts`, `multi/s2-manualseg-cycle` (FSL-98 exercised: the fix changes
  1.1 M values). New `Run` fields: `fix_finishes` (the default run finishes, naming an entry),
  `fix_refuses` (the default refuses), `refused` (both modes refuse; FSL's run never cached,
  reported). `SILENT = ("FSL-96", "FSL-97")` passed to the FLIRT suite's `fix_pass` (its new
  `silent_ok` keyword; default unchanged).
- `fast.rs`: `main` collects `Notices` (printed last, `larmorx-nc: upstream defect ...`, also
  when an exception unwinds); `prior_registration` complete: names (FSLDIR or -A), the
  2/3-class message, FslFileExists (`return_validimagefilename(..).is_ok()`), the three FLIRT
  runs (`run_flirt`: FLIRT's replica `larmorx_nc_flirt::flirt` in-process, its exceptions
  caught and printed as FLIRT's process would, verbose prints the command), the registered
  priors read back, -P -> 2, -1 when FLIRT's outputs are missing. Crate dependency added:
  larmorx-nc-flirt (crates-nc/Cargo.lock refreshed). The >4-channel refusal now after the
  channels are read (FAST's read errors first: `-S 9 a b` reads "9" and aborts, as FSL).
  Negative `-n`: single channel throws std::length_error `vector::_M_fill_insert` (-1) /
  `vector::_M_default_append` (<= -2) in TanakaMain; multi throws std::logic_error
  `Mat::init(): requested size is too large` (TanakaCreate's first ReSize). `-n 0` multi exits 0
  (compared).
- Defects decided (entries in `crates/larmorx-core/src/defects.rs`, behaviour-only texts; the
  test sample `lookup("FSL-99")` changed to `NONE-01`):
  - FSL-96 `-p`'s maps outside the mask never written: 0 in both modes, named in both (silent).
  - FSL-97 multi `-N` with -b/-B: bias field never written: 1 / the image, named in both (silent).
  - FSL-98 multi qsort inverse permutation (rule A): fixed by default; named when the sorting
    permutation is not its own inverse (`MultiSegmentation::sort_not_involution`).
  - FSL-99 rule B: -H/-f/-R not finite, -l <= 0 or not finite, -n < 1: refused by default
    (`larmorx-nc: error: fast: ...`, exit 1); reproduce runs FAST's arithmetic (multi `-H nan`'s
    rhs zeros) and names it. FSL measured: -H inf, -l 0/-5/nan, -f nan -> every voxel class 1;
    -R nan/inf change the pve; -H nan estimates beta against zeros; -n 0/-1/-2 abort.
  - FSL-100 FSLDIR unset: reproduce aborts (std::logic_error); fix: segments when no -a, a
    refusal with -a and no -A (name the priors with -A).
  - FSL-101 FLIRT's registered priors missing (bad matrix, a name split by the shell, FSLDIR
    unset with -A): FAST prints "... priors are not used!" then crashes (SIGSEGV): fix segments
    without priors; reproduce refuses where FAST crashes.
  - FSL-102 the shell: fix passes names whole and needs no FSLDIR for FLIRT (named when a name
    has a blank/special character or FSLDIR is unset); reproduce splits at blanks, prints dash's
    "sh: 1: Syntax error: word unexpected" for FSLDIR unset, refuses names with shell specials.
  - FSL-103 mask leaves out the maximum's voxels when max >= 2^24 (rule A): fixed (mask: v > 0
    and finite); named when it changed (`Segmentation::max_left_out`).
  - FSL-104 channels of different sizes: when channel 1 fits inside the others and -s does not
    give every mean, FAST reads their corners and aborts at the percentiles ("mask and vol have
    different sizes in calc_percentiles", newimage #3): reproduce crops and throws the same
    (`MultiParams::size_error`); otherwise refused in both modes; the default always refuses.
  - Not registered (no ID used, FSL-105..110 free): relay 1's "input is the last word / parsing
    stops at the first non-option" (rule C: as documented, `fast [options] file(s)`; an option
    after the input is read as the image and aborts); `-S n` with fewer images reads the option
    words and argv[0] as images and aborts at the first (defined: FSL's own path is no image),
    UB only if a file named like argv[0] exists (refused); missing input -> abort (convention);
    `-P` or `-A` without `-a` -> no effect (rule C, documented); the 2-class InitSimple `==`
    (no effect); the multi-channel k-means pairing of opposite contrasts (relay 2a item 7:
    every voxel class 1, NaN bias field; FSL's result reproduced in both modes): rule D
    candidate (measure a channel-1-conditioned initialisation on real T1w+T2w before deciding;
    pinned by `multi/s2-opposite-contrasts`); the FLIRT replica's own defaults apply inside
    `-a` (FSL-09: the uint8 avg152 priors' resampled values rounded instead of truncated by
    default; FSL-68 for a bad matrix).
- Gates run: crates-nc fmt, clippy -D warnings (workspace), cargo test --workspace (all pass
  except the known `every_tool_has_a_golden_case`: relay 3); root: cargo fmt --check,
  clippy -p larmorx-core, cargo test -p larmorx-core defects; ruff check/format, SPDX. Not run:
  pytest, the smoke tier, records (relay 4).
- `PROVENANCE.md` updated (prior_registration with FLIRT's replica, the D13 summary, pending:
  `-S 5+`).
- Probe scripts used (scratch, exploratory, results above): `<scratch>/probe/{run,look,neg}.sh`
  (FSL's behaviour on the nonsense options and negative class counts). Relay 3 may move them
  into `validation/investigations/fsl-fast/` as one probe if a finding cites them.

## -S 5 to 8 (22c4ec8) and -S 9+
- `larmorx-nc-core::lapack::MAX_N = 8`: getrf/getrs (nrhs 1 above 4)/gesv/getri/trtri accept up
  to 8; for n >= 5 getf2_k's DOTU by `blas::ddot` (strided), GEMV_N and dgetri's DGEMV by
  `blas::dgemv_n`, IAMAX by `iamax_short` (iamax_sse2.S `.L50`, 5 to 8 elements: maxpd lanes,
  the cmpeqpd block of 8, comisd groups, the next index unchecked), the dtrtri zero check as
  "first exact zero" (NaN hiding a zero not modelled: unreachable in FAST). The n <= 4 code
  paths are untouched (the existing nc golden checksums pass). `arma::inv`/`det`/`solve` up to
  8 (solve: one column above 4). Probe: `validation/investigations/version-numerics/
  lapack_probe.py` OPS for 5..8 and a `covariance`/`covariance-collinear` family; the
  `lapack-probe` tool's ipiv sized MAX_N. Run (2,000 per family): 388 of 420 (op, family) pairs
  identical; the 32 others only in the `nonfinite`/`triu-nan` families and only in NaN payload
  bits ("beyond NaN bits 0"): `blas.rs` does not model NaN operand order. Log:
  `<scratch>/lp58.log`. (Relay 4 should keep this probe's numbers in the record or findings.)
- `multi_mriseg.rs`: MAX_CHANNELS 8; `quad` for n >= 5 = BLAS dgemv('T', n, n) then
  `dgemv_t_col` (its unit tests now cover n up to 8 against `arma::mul3` and `blas::dgemv_t`).
- Suite: `multi/s5` (5 x 5; -B -b) 21 of 21 files and `multi/s8` (-n 2 -g) 15 of 15
  bit-identical.
- `-S 9+` still refused ("not replicated yet", after the images are read): it needs
  iamax_sse2.S's aligned path (`.L05`, chosen by the column's 16-byte alignment, which follows
  from Armadillo's 16-byte-aligned buffers and `j * (n + 1)`'s parity) for 9 to 16, the 8-way
  ddot/daxpy kernels beyond 16, getrf's blocked path beyond 19. Practically unused; leave it
  unless the orchestrator asks.
- NaN payloads: a 5-to-8-channel run whose covariances turn NaN (empty classes) may write NaNs
  with other payload bits than FSL's (values and NaN-ness identical): for the findings.

## Relay 2a: what was built (137e648)
- `crates-nc/larmorx-nc-fast/src/multi_mriseg.rs` (new): `MultiSegmentation`, the translation of
  `ZMRIMULTISegmentation` (TanakaCreate/TanakaMain and every function they call; InitSimple and
  the `bapused >= 2` prior product translated but UNTESTED: `-a` still refused by
  prior_registration). Matrix maths on Armadillo 14.2.1's paths, verified on FSL: `row*Sinv*col`
  = `(row Sinv) col` = tinysq-transposed gemv then OpenBLAS `dgemv_t` (m = n channels, one
  column: its tail rows are FMAs: `fma(a0,x0,a1*x1)`), local copies `tinysq_t`/`dgemv_t_col`
  unit-tested bit-equal to the core's `arma::mul3` and `lapack::blas::dgemv_t`; `row*S.i()*col`
  (PVEnergy) = `row * solve(S, col)` (`dgesv` = getrf + getrs; LU cached per mixel type and
  fraction); `inv()`/`det()` = core `arma::inv`/`arma::det`. Types: logGaussian's normalisation
  in double, logpveGaussian's in float (`logf(sqrtf(fabsf(det*M_2PI)))`), PVEnergy `+ logf(det)`;
  the MRF `exp` is `expf` (all-float argument), Initclass/PV likelihoods double `exp`; the delta
  loops `delta += step` in float. Limited to 2..=4 channels (core LAPACK up to 4 x 4); `-S 5+`
  is refused "not replicated yet" (would need OpenBLAS getf2/getri/getrs and dgemv beyond 4 x 4).
- `fast.rs`: `segment_multi_channel` (images = the last nch words, images[0] the LAST word:
  channel 1 names the outputs, orders the classes, gives every output's header except
  `_restore_<n>`, which keeps channel n's own header), outputs in FSL's order; `clear_negative`
  shared with the single channel; refusals: more channels than words after the program
  ("fewer images than channels", FSL would read argv[0] or before it), channels of different
  sizes (FSL reads/writes past the smaller), more than 4 channels.
- `mriseg.rs`: `read_floats` made `pub(crate)` (shared). `tests/fast.rs`: the run helper catches
  thrown FslErrors (prints them), a three-slab two-channel test.
- Suite `fsl_fast_replica.py`: derived inputs v3 (`oracles/inputs/fsl-fast-replica/`: `mni_t1`,
  `mni_t2`, `mni_bold`, `mni_ratio` (T1w/T2w) at 2 mm from `<workspace>/oracles/templateflow`'s
  MNI152NLin2009cAsym res-02, masked by its brain mask; `mni4_t1/t2` at 4 mm; `mni_means.txt`
  for `-s`); kind `multi`: 11 cases (`multi/s2-outputs`, `s2-type2-verbose`, `s3`, `s4-n2`,
  `s2-n4`, `s2-nopve-default-name`, `s2-nobias-outputs`, `s2-manualseg`, `s2-options`, smoke
  `s2-quarter`, error `s0-error`). `Run.never` (oracle cache `but_never`), `Run.unwritten`
  (FSL's `_prob_*` voxels outside its `_seg` mask set to 0 before comparing, reported) and
  `Run.no_bias_field` (FSL's `_bias_<n>`/`_restore_<n>` with -N replaced by the defined 1 and
  channel image, headers kept, reported).
- Results: standard tier 13 cases: 12 pass + 1 both-error (s0-error: same exit, same message);
  every compared file bit-identical, stdout/stderr identical incl. `-v`; fix pass OK (no defect
  named yet). FSL ~25 s per 2 mm case, replica ~8 s (one thread for the MRF, threads in the bias
  convolutions). Oracle cache: 10 served, 3 never cached (the `-p` and `-N` cases).
- Gates: crates-nc fmt, clippy -D warnings clean; tests all pass except the known
  `every_tool_has_a_golden_case` (relay 3); ruff, SPDX clean. Not run: root/crates-gpl, pytest,
  smoke tier, records (relay 4).

## Defects found by relay 2a (for 2b to decide by rules A-D; source fast_two.cc /
## multi_mriseg_two.cc of fsl-fast4 2111.3)
1. `members` (both segmentations: mriseg_two.cc:112, multi_mriseg_two.cc:123) is `new float[]`,
   never written outside the mask: `-p`'s `_prob_<n>` outside the mask holds heap memory (seen in
   `multi/s2-outputs`: 8 voxels of `_prob_0` from (89,0,0) hold malloc's free-list pointers,
   changing with ASLR; relay 1's single-channel run happened to get fresh zero pages, so
   fMRIPrep's `-p` call is exposed too). Replica writes 0. D13 "memory never written": defined
   parts reproduced, the rest filled deterministically (0) with a warning (decide whether the
   warning prints on every `-p` run, e.g. only when the comparison could see it: never).
2. Multi-channel `-N` (multi_mriseg_two.cc:199, 251): `p_bias` is never written without a bias
   field, yet `m_Finalbias=p_bias` and takeexpo/`-B`/`-b` use it: FSL saved `_bias_<n>` = inf and
   `_restore_<n>` = 0 inside the brain (heap contents), 1/0 and 0/NaN in 2 voxels outside.
   Replica: no bias field (bias 1, restore = the image). Rule A (input sensible, result wrong) or
   the memory rule: default the defined result + notice; reproduce: the same defined fill + a
   warning (FSL's values depend on memory). The single channel sets its field to 0 itself.
3. Multi-channel `qsort` (multi_mriseg_two.cc:1659): class c's channel-1 mean matched with the
   m-th smallest, then slot c gets class m's other-channel means, covariance, inverse and
   posteriors while its channel-1 mean becomes the c-th smallest: the INVERSE permutation.
   Correct only for permutations that are their own inverse (identity, swaps; `-t 2`'s reversal);
   a 3-cycle mixes classes (channel-1 mean of one class, covariance/posteriors of another). Not
   seen in the suite's cases (all identity or swaps). Rule A candidate: fix = apply the sorting
   permutation (slot m <- class c); reproduce = FSL's; notice when the permutation is not an
   involution. The replica implements FSL's now (`MultiSegmentation::qsort`).
4. `rhs` (multi_mriseg_two.cc:204) `new float[100]` is never written (TanakaPriorHyper only for
   `-H` < 0, refused by main) but TanakaHyper reads it when `-H nan` (NaN passes `Hyp<0`):
   heap memory. Replica: zeros. Same rule-B candidate as relay 1's item 2 (refuse a NaN `-H`).
5. More channels than words (fast_two.cc:407/415, `argv[argc-c-1]`): `-S n` with n >= argc
   reads argv[0] (FSL's own path) then argv[-1] (UB). Replica refuses ("fewer images than
   channels"); `-S 4 a b` reads the word "4" as an image (FSL's "Failed to read volume 4"; the
   replica the same). Rule B.
6. Channels of different sizes: TanakaMain reads/writes `m_Mricopy[i].value(x,y,z)` over channel
   1's grid: out of bounds for a smaller channel (heap corruption); replica refuses. Class (a).
7. Not a defect, a finding for the spec: the multi-channel k-means starts each channel's class
   means from that channel's own percentiles in ascending order, so channels of opposite contrast
   (T1 and T2 slabs: dark/bright swapped) pair the wrong means; a class can start empty, then
   `normtemp` = 0 and `cov/=tot` (multi_mriseg_two.cc:1792) give NaN means and covariances and
   every later value NaN (FSL exits 0: all voxels class 1, NaN bias field). Replica identical to
   FSL there (checked by hand on a phantom: stdout and every file but item 1's garbage). On the
   real template's T1w+T2w the init works. Rule C (keep, document) or D; a suite case for 2b.
8. InitSimple with 2 classes (multi_mriseg_two.cc:1243; also check mriseg_two.cc):
   `talpriors(0)=talpriors(1)=talpriors(2)==0.0f` (a comparison for an assignment): talpriors(1)
   is overwritten just below, talpriors(0) never read: no effect (document only).
- Untested multi-channel paths: `-a`/`-P` (InitSimple, the prior product), `-n 1` (no mixel
  types: pve all 0, pveseg 1 in the mask), a singular covariance (two identical channels:
  `inv(): matrix is singular` thrown, check FSL's exact stderr/exit, arma may print "error:"),
  4D channels, NaN voxels, `-S 5+` (refused).

## What relay 1 built (f747b28)
- `crates-nc/larmorx-nc-fast/` (new crate; workspace member, `larmorx-nc fsl fast` in
  `larmorx-nc-cli/src/main.rs` FSL_TOOLS): `src/fast.rs` (main: options in `options.add` order,
  usage/messages/exit codes, input + negative-value handling, prior_registration (FSLDIR, the
  names; `-a` refuses "not replicated yet"), outputs, multi-channel: only the `<2 channels`
  error, `>=2` refuses "not replicated yet"), `src/mriseg.rs` (ZMRISegmentation, whole single
  channel incl. BiasRemoval, verbose prints, TanakaHyper, InitSimple, ICMPV for 2/3/>3 classes;
  TanakaPriorHyper not translated: unreachable since main refuses Hyp<0), `src/cli.rs`,
  `tests/fast.rs` (usage/errors, a phantom run), `PROVENANCE.md`.
- `larmorx-nc-core` (additive; existing golden checksums unchanged): `glibc_float::expf`
  (__expf_fma: 5 fused multiply-adds read from libm's machine code), `logf_compat`;
  `glibc_double::log` (__log_fma), `log_compat`; `crand.rs` (glibc rand/srand); `stats.rs`
  (calculateSums blocks, sum/mean with mask, percentile, threshold, binarise); `FSLDIR` added to
  `settings::FSL_VARIABLES`; PROVENANCE rows; tools: `glibc-float-verify expf` (all 2^32: 0 differ),
  `libm-verify` + `glibc_reference.c` gain log/logf/expf (0 differ; README: build the reference
  with FSL's gcc, on PATH as ~/fsl/bin/gcc, so it binds GLIBC_2.2.5 symbols like FSL).
- `crates-nc/fsl-source/fsl-fast4/` (5 files; scripts/fsl_source.py FILES + VERSIONS; README row).
- Suite `validation/src/larmorx_validation/parity/fsl_fast_replica.py` (registered in
  `__main__.py`; `tests/parity/test_fsl_fast_replica.py`): kinds `fmriprep` (every
  `anat_fit_wf/fast` node; input found by name in the run's work dir and copied into the run dir;
  STANDARD_RUNS = ds000005 only; fMRIPrep's own pve outputs reported) and `real` (`Run(args)`,
  `@name` = derived input in `oracles/inputs/fsl-fast-replica/`; `real/fmriprep-call-half` smoke).
  Reuses fsl_flirt_replica's `compare`, `fix_pass`, `run_replica`, `_environment`.
- `validation/investigations/fsl-fast/` (README, `arm_glibc_expf_log.py`: Arm tables found in
  libm, the IFUNC offsets).

## Results (relay 1)
- standard tier: 2 of 2 pass. fmriprep/ds000005: 13 of 13 files bit-identical (seg, seg_0..2,
  prob_0..2, pve_0..2, pveseg, mixeltype + the input copy), stdout/stderr identical, D13 fix pass
  OK, fMRIPrep's own outputs (its container's FSL) identical 4/4. Smoke `real/fmriprep-call-half`
  pass. Oracle cache: 2 entries (FSL 25 s and 3 s); replica ~17 s per full run, one thread.
- crates-nc: fmt, clippy -D warnings clean; tests all pass EXCEPT
  `larmorx-nc-cli/tests/golden.rs::every_tool_has_a_golden_case` (no FAST golden case yet;
  relay 3). `golden.rs`'s `run_bet` removes FSLDIR: FAST then aborts (FSL's behaviour, below)
  unless FSL-96's default no longer needs FSLDIR, or the case sets it.
- Not run: the root and crates-gpl workspaces (untouched), pytest, the smoke tier, records.

## How to work here (lessons)
- The worktree guard refuses `source <relative>`, `..` paths, `git -C`, and long compound
  commands/heredocs with relative paths: use absolute paths, the Edit/Write tools, and
  `<scratch>/dev.sh <dir> <command...>` (env.sh's environment + PYTHONPATH for this worktree).
  URLs containing "github" in a command trip it too (put them in a script file).
- Build: `<scratch>/dev.sh <wt>/crates-nc cargo build --release -j 14 -p larmorx-nc-cli`
  (crates-nc/target was seeded from the main checkout; the root target/ was NOT copied, and no
  larmorx wheel is built here, so `--out` records fail: environment.describe imports
  larmorx._core; relay 4: `maturin build --release -o <scratch>/wheel`, copy `_core.abi3.so`).
- Suite: `<scratch>/dev.sh <wt> <wt>/scripts/orchestration/boxed.sh 16 python -m
  larmorx_validation parity fsl-fast-replica --tier standard [--case X] [--jobs 2]`; per-check
  detail: `python -P <scratch>/py/show_case.py <id substring> [tier]`.
- Limits: 14 threads, boxes <= 32 GiB together (another agent works beside).

## What the documents say (for the next relays)
- Roles (porting guide §1): we are replica author AND spec writer; `specs/fast.md` (relay 3)
  describes behaviour, maths, conventions, command line, messages, failure cases, validation
  plan; never code or routine names; mark black-box facts "(observed)". Never edit clean-room
  crates (`crates/larmorx-fsl`, `-mri`, `-afni`).
- D13 (§8): classes (a) crash/UB -> defined result or refusal by default, reproduce switch for
  what FSL defines; (b) never; (c) fixed by default + notice when it changed the result; (e)
  parsed as documented, switch; one switch `LARMORX_UPSTREAM_DEFECTS=fix|reproduce`; replica's
  notices `larmorx-nc: upstream defect FSL-xx (...)` via `larmorx_core::defects` (add FSL_96...
  entries there; see how BET's Notices work: `larmorx-nc-bet/src/bet2.rs` print_notices,
  `larmorx_core::defects::Notices`); rules A-D decide without asking. The suite's fix pass checks
  "named iff changed" (`silent_ok` for notices whose effect the comparison cannot see).
- Contract (§11): API page `docs/api/fsl-fast.md`, findings `docs/findings/fsl-fast.md`
  (restricted: source-derived), validation record `docs/validation/fsl-fast-replica.md`,
  `validation/tools.toml` entry, golden cases, memory test (`tests/parity/test_memory_peak.py`,
  oracle peaks through the cache `run(..., peak=True)`), difftest descriptor, site page. Shared
  docs only through `<scratch>/HANDOFF.toml` (+ HANDOFF.md).
- Models: FLIRT replica suite and crate (`crates-nc/larmorx-nc-flirt`, `fsl_flirt_replica.py`),
  BET's (`larmorx-nc-bet`), parts' notes `docs/parts/2026-11-01-fsl-flirt-part*/notes.md`.

## FAST facts found (relay 1; findings for docs/findings/fsl-fast.md, restricted)
- fast binds `exp, expf, log, logf, sqrt, sqrtf, rand, srand` @GLIBC_2.2.5 (SVID wrappers; the
  log wrappers return +NAN for x<0, -inf for 0). No FMA instructions in `fast`; float maths as
  written; `-1.0*x` folded to negation by GCC.
- RNG: `srand(-1)` then `rand()/(float)RAND_MAX` per masked voxel and class at each
  TanakaIterations; glibc TYPE_3 generator (crand.rs).
- C++ overloads: `log(float)`/`exp(float)` are logf/expf (Initclass, takeexpo, exp of m_Mri,
  ICMPV, PVEnergy, -s means); `log(x+1.0)`, logGaussian's log/sqrt, the posteriors' and PV
  likelihoods' exp are double.
- The MRF sweep updates m_post in place (Gauss-Seidel); 27 neighbours in n,m,l order incl. self
  and corners with weight 0; ICMPV also in place; mask outside the image is 0 (zeropad).
- MeansVariances: float products (post*mri, (post*mri)*mri), sums in calculateSums' blocks
  (nlim = max(sqrt(nvox), 10000), flush when n > nlim); `mean*mean` rounded to float before the
  double subtraction; "ERROR:: Empty mask image" per empty sum (4 per class).
- `--pvestep` is declared but never added (no such option). `-p`'s help starts with a tab.
- Candidate defects for relay 2 (decide by rules A-D, IDs FSL-96..110):
  1. FSLDIR unset: `string(getenv("FSLDIR"))` throws std::logic_error "basic_string:
     construction from null is not valid" -> abort, even without -a (only -A avoids it). Class
     (a); replica currently reproduces the abort in both modes (fast.rs prior_registration).
  2. `-H nan` passes main's `Hyp<0` check -> TanakaHyper against an all-zero rhs (prints
     " TOTAL ... RHS 0 ..." and 14 stderr lines, beta chosen against zeros). Rule B candidate.
  3. The input is `argv[argc-1]` (the LAST word) and parsing stops at the first non-option:
     `fast in.nii.gz -n 2` reads "2" as the image. Class (e).
  4. Mask = binarise(0, max+1, exclusive): voxels equal to the maximum are left out when
     max+1 == max in float (max >= 2^24). Rule A candidate.
  5. A missing input: read_volume throws (uncaught -> abort); the "Image cannot be found" branch
     is dead code.
  6. `-n 1` (no mixel types), `-n 0`, `-n` > 3, `-t 2` (reversed order), equal initial means
     ("Exception: Not enough classes detected to init KMeans", exit 255), NaN/inf voxels (sort
     with NaN: stdsort Outside handled NaN-last + flag, not yet a notice), all-zero image.
  7. `-a` runs `$FSLDIR/bin/flirt -ref <in> -in <prior> -out <out>_{csf,gm,wm}_stdspace -applyxfm
     -init <mat>` with system() (files left behind; verbose prints the commands): replicate with
     the FLIRT replica in-process (add larmorx-nc-flirt as a dependency), priors from
     `~/fsl-6.0.7.23/data/standard/tissuepriors/avg152T1_{csf,gray,white}`; InitSimple and
     bapused=2 (`-P`) are translated but untested; prior volume sizes are not checked by FSL
     (`.value()`), mine returns "Out of Bounds" if smaller (fix when wiring).
- Untested against FSL so far: bias removal (default without -N; uses core convolve_separable,
  divide), `-b`/`-B` outputs, verbose texts, `-s`, `--nopve`, `-t 2/3`, `-n`, `-R`, `-H`, `-f`, `-l`,
  `-O`, `-W`, `-I` > 0, 2D images, 4D input (core read_volume truncates with FSL's warning).
- NaN bit patterns: plain Rust arithmetic (= SSE on x86); for aarch64 golden cases, NaN outputs
  may need `larmorx_nc_core::sse` helpers.
- Memory: FSL allocates meanP/varianceP (2 x 4D) per MeansVariances, PVprob (6 vols), qsort
  copies; the replica avoids meanP/varianceP but still copies post/prob in qsort.
- Threads: FSL is single-threaded; the replica uses threads only in the bias convolutions
  (core convolve_separable, result independent of the count).
- The core changed (additively): relay 4 re-runs mcflirt's, BET's and FLIRT's records and
  compares them (the existing nc golden checksums already pass unchanged).

## For the handoff (relay 5)
- (relay 3) next-steps follow-ups: `-S 9` and more (refused in both modes, the orchestrator's
  decision 2026-11-02: needs OpenBLAS's aligned iamax path 9-16, 8-way ddot/daxpy beyond 16,
  dgetrf's blocked path beyond 19); the rule-D measurement of the multi-channel initialisation
  (per-channel ascending means + the (sum of differences)^2 "distance") AND of the multi-channel
  bias field (estimated from the corrected image, replacing not refining, no mean removed,
  sqrt(l/d) kernels) on real T1w+T2w pairs (specs/fast.md 9.2). Register: FSL-96's text was
  extended (also never written at all with --nopve and priors or -W 0 -O 0). README / tools /
  validation index: lx.fsl.fast (replica only). docs/api/upstream-defects.md rows FSL-96..104.
  docs/licensing.md / overview: FAST in larmorx-nc. specs/README? (none exists).
- No shared document edited yet. To hand off: the defect register entries, docs/api/
  upstream-defects.md, validation index/tools.toml, golden.md, next-steps; the core's new
  FSL_VARIABLES entry (FSLDIR) concerns every nc tool's settings table (harmless).
- (relay 2b) For relay 4: `larmorx-nc-core` changed again (lapack.rs: the 5-to-8 paths beside
  the untouched n <= 4 ones; arma.rs: the asserts and pivot arrays): re-run mcflirt's, BET's
  and FLIRT's records and compare (expected identical). The root workspace changed too
  (`crates/larmorx-core/src/defects.rs`: FSL-96..FSL-104, the test sample): its gates and
  pytest (`tests/python/test_defects.py` reads the table) in relay 4's gates.
  `fsl_flirt_replica.fix_pass` gained `silent_ok=` (default unchanged).
- (relay 2b) For the handoff: the register entries FSL-96..FSL-104 (texts in defects.rs; rule
  and decision per entry in "Relay 2b" above) and the not-registered decisions (rule C/D list
  there); docs/api/upstream-defects.md rows; next-steps: `-S 9+` open, the rule-D measurement
  of the multi-channel k-means initialisation.

## Relay 4: what was done (f217c6a, b42aa20, bb068f1, d1be4c9, 85e98c7)
- Descriptor `validation/src/larmorx_validation/difftest/tools/fsl_fast.py` (`fsl-fast`, registered in
  tools/__init__.py; permanent cases attached to the suite: difftest/permanent.py SUITES
  "fsl-fast-replica"). Realistic only: kinds fmriprep (2) / single (6) / multi 2-4 channels (2);
  brain-extracted phantom heads (gen.py's new optional content flag "brain": zero outside the CSF
  ellipsoid; 10 % whole heads; 6 % one-slice 2D; crops of the MNI T1w for single channel), the image
  centred on the world origin, a field of view of 24-64 mm with 12-32 voxels per axis, a softened
  bias field; every option with short and long spellings; -a with a standard-to-input matrix that
  scales FSL's priors onto the brain (-P), -s from the phantom's tissue levels (in order, cycled,
  swapped); FSL's output types (NIFTI_GZ forced with -p and multi -N: the suite's FSL-96/97 handling,
  never cached). The suite's `_prepare` reads `@dt:` inputs.
- Trial: `oracles/difftest/2026-11-02-fsl-fast` (seed fast-trial, 0.5 h, 8 jobs at 1 thread, 2 GiB
  per case, box 20 GiB, oracle cache `use`; log oracles/logs/fast-4-trial.log): 17,497 cases (0
  extreme): 16,768 pass, 725 rejected by both alike (371 k-means cannot start (255), 159 singular
  covariance with several channels (abort), 178 FSLOUTPUTTYPE=ANALYZE that FSL 6.0.7.23 refuses
  (157 exit 1, 21 crash with -a: FSL-101), 17 -a with -n 4 "Apriori can only be used for 2 or 3
  classes"), 4 differ, 0 memory, 0 timeout, 0 notice. Pre-check before it: 40 cases (<scratch>/precheck).
- The 4 differences: FSL-101 (FLIRT fails on the output type, FAST initialises from unallocated
  priors). Fixed in the replica's reproduce (fast.rs `unregistered_priors`, `Segmentation::mask_is_empty`):
  -v's "Beginning prior-based initialisation" printed before the refusal; one channel with an empty
  mask runs on as FAST does (no voxel reads the priors); several channels of one size whose voxel
  (0,0,0) is above 0 in every channel throw newimage's "Out of Bounds (time index)" (#5) and abort
  (FAST's bounds-checked access); otherwise refused where FAST writes through a null pointer.
  Permanent cases (standard tier): fsl101-empty-mask-runs-on, fsl101-verbose-before-crash,
  fsl101-multi-exception. Replays: all 4 agree; the trial's 27 cases with -a and ANALYZE re-run with
  the new binary: 27 agree. Suite priors/ + err/fsldir + difftest/: 20 cases, 12 pass, 8 both-error.
- larmorx-nc golden recorded (bb068f1, from b42aa20): FAST 11 cases (9 runs + the 2 refusals), 124
  outputs; golden.tsv now 79 cases, 440 outputs; every other checksum unchanged. Main golden: no
  checksum due (pytest tests/golden: 179 passed, nothing unrecorded): record_golden.py not run.
- Gates (relay 4): crates-nc fmt, clippy -D warnings, cargo test --workspace --no-fail-fast (all pass
  but golden "not recorded" before bb068f1; golden passes after recording); ruff check/format, SPDX;
  pytest tests/python tests/golden validation/tests: 670 passed, 12 skipped, 2 failed (the tables
  tests: the FAST record did not exist yet; after d1be4c9: 672 passed, 12 skipped).
- d1be4c9: the record docs/validation/fsl-fast-replica.{md,json} + `tables` (README.md and
  docs/validation/README.md: one generated row each, from validation/tools.toml's new fsl-fast tool
  and fsl-fast-replica record entries; tables.py METRICS "fsl-fast-replica": _bet for the fMRIPrep
  count). The site's status table is generated from the same tools.toml (relay 5: run `tables` with
  the site clone, as the earlier parts did, or hand it off).
- 85e98c7: validation/investigations/fsl-fast/difftest_finding.py (+ README section).

## For relay 5 (the handoff texts relay 4 owes; put them in HANDOFF.toml/.md)
- triage.toml (shared): four [[difference]] entries, tool "fsl-fast", implementation "replica",
  status "fixed":
  - signature ["stdout identical"], link validation/difftest/cases/fsl-fast/fsl101-verbose-before-crash.json:
    "FSL-101 with -v: FAST prints 'Beginning prior-based initialisation' before it crashes on the
    priors it never allocated; the replica's reproduce stopped before that line (now printed)"
  - signature ["stdout identical", "stderr identical"], link .../fsl101-empty-mask-runs-on.json:
    "FSL-101 on an image whose mask is empty: FAST runs on (no voxel reads the priors; every class
    statistic NaN); the replica's reproduce refused (now runs on as FAST)"
  - signature ["stderr identical"], link .../fsl101-multi-exception.json: "FSL-101 with several
    channels whose first voxel is in the mask: FAST's bounds-checked access to the unallocated
    priors throws 'Out of Bounds (time index)' and aborts; the replica's reproduce refused (now
    throws the same)"
  (the trial's 4 findings: #3230, #3415 (the first two kinds), #4273 (verbose), #4762 (multi))
- docs/validation/difftest.md (shared): a table row: `fsl-fast` | `parity.fsl_fast_replica` (a whole
  `Run` as the case's payload: inputs as `@dt:` names; `-p` and multi-channel `-N` cases never cached,
  FSL's never-written voxels set to the defined values as in the suite) | replica vs fast | fMRIPrep's
  call; one to four channels of one brain-extracted phantom (T1-, T2- and PD-like contrasts, 2D
  slices, whole heads, crops of a real T1w), every option (short and long names), FSL's tissue
  priors with a drawn standard-to-input matrix (-a, -P), -s, every stored type, FSL's output types.
- golden.md (shared): crates-nc row "79 cases, 440 outputs" (was "65 cases, 313 outputs", stale);
  a FAST paragraph: eleven cases in crates-nc/larmorx-nc-cli/tests/golden.rs (fast.fmriprep, .bias,
  .t2-n2-nopve-verbose, .n4-manualseg, .multi, .priors, .fsldir-unset, .fsl103, .fsl103.reproduce,
  and the refusals fast.fsl99.nan, fast.fsl104.sizes).
- The register's FSL-101 text (shared; larmorx_core::defects not changed by relay 4) may add: "or,
  with several channels whose first voxel is in the mask, aborts on an exception; an image with an
  empty mask runs on" (behaviour only).
