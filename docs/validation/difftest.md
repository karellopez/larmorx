# Differential random testing

The parity suites test the cases people chose; random testing finds the ones nobody thought of
(decision log 2026-10-23, point 4; 2026-10-24, "random testing's differences"). Every night,
random valid inputs and option combinations run through each original program and larmorx; a
difference is shrunk to the smallest input and fewest options that still show it, and either
fixed at once (a clear larmorx mistake: its shrunk input becomes a permanent parity case) or
written into the defect register by rules A-D. Never in CI (rule 10): it needs the originals.

The code is `validation/src/larmorx_validation/difftest/`; its tests are
`validation/tests/test_difftest.py`.

## Running it

```bash
python -m larmorx_validation difftest --tools all --budget 6 --jobs 4 --seed 20261025
scripts/orchestration/difftest-nightly.sh            # the same, for cron or by hand; dated directory
python -m larmorx_validation difftest --case ants-image-math:20261025:17 --show    # a case's recipe
python -m larmorx_validation difftest --case ants-image-math:20261025:17           # run it again
python -m larmorx_validation difftest --replay <finding or permanent case .json>   # a reproducer
python -m larmorx_validation difftest --report oracles/difftest/2026-10-25         # report again
python -m larmorx_validation difftest --promote <finding .json> --id <name> [--tier smoke] \
    [--expect pass|divergence|error|guard --reason TEXT] [--status fixed|register|open --defect ID --rule A-D]
python -m larmorx_validation difftest --list                                       # the tools here
```

| Option | Effect |
|---|---|
| `--tools` | `all`, or a comma-separated list (`--list` names them, and says why one cannot run here) |
| `--budget` | hours of random cases; tools take turns by their weight until it is spent |
| `--jobs`, `--threads` | cases at a time, and the threads of each original and larmorx run (default 1: the comparison at one thread; the thread bands stay the parity suites' job) |
| `--seed` | the night's seed (default: the date); every case is drawn from (tool, seed, case number) |
| `--shrink-budget` | seconds to shrink one difference (default 600) |
| `--out` | the night's directory (default `<workspace>/oracles/difftest/<date>/`) |
| `--memory-gib`, `--box-gib` | the hard memory limit of each worker (3 GiB, no swap) and the box the command makes for itself when it runs in none (8 GiB); `--memory-gib 0` runs without limits (only where cgroup v2 is not writable) |
| `--heavy <night>`, `--heavy-hours`, `--heavy-only` | run a night's heavy cases to their end (below), the quickest first by their prediction; `--heavy-only` names the cases to run (`tool:seed:n`, comma-separated or a file) |

A night writes `oracles/difftest/<date>/` (outside git, like every oracle output):

| File | Contents |
|---|---|
| `run.json` | the command, seed, budget, jobs, threads, larmorx commit, the tools run and those that could not run |
| `results.jsonl` | one line per case as it ends: tool, case number, verdict per implementation, the options and input features it covered, seconds, its findings' files |
| `findings/<tool>/<seed>-<n>-<implementation>.json` | each difference: the drawn recipe, the shrunk recipe, the moves that shrank it, the checks that failed (both), how to replay it |
| `report.md`, `summary.json` | the morning report and its numbers (the site's exporter reads the newest night's `summary.json`: a folder named by its date alone; a part's short trial, `--out <date>-<topic>`, never replaces it) |
| `heavy.jsonl`, `heavy-results.jsonl`, `heavy-prediction.json` | the cases over the memory limit, their results once `--heavy` ran them, their predicted memory and time |

## What a case is

**A descriptor per tool** (`difftest/tools/<module>.py`, registered in `difftest/tools/__init__.py`)
says how to draw a case: the program's option grammar with valid ranges and combinations (the
forms its parity cases use, and more), the input generators, which implementations run, which
of the suite's reported checks must be exact. It turns the drawn recipe into a case of the tool's
own parity suite, so **the same runners and comparators** decide it as decide the records
(header, values bit for bit, messages, exit status, files, the D13 fix pass). Adding a tool is one
short module; FSL's BET is one (`tools/fsl_bet.py`, 2026-10-26).

**Inputs** (`difftest/gen.py`) come from seeded random numbers and plain arithmetic only (the golden
tests' rule: no transcendental function, so a recipe gives the same bytes on every machine),
written field by field: shapes up to 32³ voxels (odd, one voxel thick, 2D to 7D for the NIfTI
tests, 4 to 8 volumes in a series), geometry (rational-quaternion qforms, so flips, axis
permutations and obliques; anisotropic spacing; the qform and sform codes; an sform equal to the
qform, absent, or sheared; the spatial units), data types and scale factors (a negative slope
included), value ranges (constant, zeros, negative, huge, tiny, integer steps, NaN, infinities,
signed zeros, outliers), masks, 4D timing (TR, its unit, slice timing), displacement fields, ITK
transforms (text in many spellings, MATLAB, HDF5), and small crops of real templates from
`larmorx-testdata` where a tool needs anatomy. Phantoms are functions of world coordinates, so a
moved copy of one (registration, head motion) is exact and its answer known.

**Realistic cases; an extreme share only on request** (user, 2026-10-25: "reasonable size
tests"; 2026-11-02: realistic cases only while the tools are ported; `gen.EXTREME_SHARE`,
`REALISTIC_SPACINGS`, `EXTREME_SPACINGS`). Every case is realistic, with headers in millimetres
(or without units) and voxels of 0.5 to 4 mm (within the 0.2 to 10 mm of real scanners), matrices
of at most 32 voxels per axis and the tools' typical options, unless a share of *extreme* cases is
asked for (`difftest --extreme-share 0.1`, or `LARMORX_DIFFTEST_EXTREME_SHARE`; the case's feature
`extreme`), which may also have metres or micrometres and voxels of 0.01 to 50 units. From
2026-10-25 to 2026-11-02 every night drew one case in ten as extreme; the extreme cases alone made
every memory finding (FLIRT's three trials of 2026-11-01: 60 of 60, none among some 200 000
realistic cases), and re-running them without the memory box took the machine down
(2026-11-02, CLAUDE.md). The draw is consumed either way, so a realistic case is the same case at
any share; a night's `run.json` records its share (`extreme_share`; a night without it drew 0.1),
and `--heavy` draws its queue again with it. A realistic header does not make every case small:
odd option combinations can still be heavy (an N4 call in 4D with order-5 splines on a 1.5 mm
image needs 22 GiB in ANTs), and the sizing laws below catch those before they run. Robustness to
nonsense headers is checked instead by a few fixed cases per tool in its standard tier, kept only
where the original and larmorx finish within a small box; the extreme share returns, if at all,
in the final validation phase. The report gives each tool's count of extreme cases (column
*Extreme*), and the site's panels too.

**Predicted before run** (`difftest/sizing.py`). For the tools whose memory grows with what their
options and header ask for (N4BiasFieldCorrection's lattice and padding, the resamplers' output
grids, 3dUnifize `-EPI`'s ball of neighbours), each case's need is predicted from the original's
sizing law and the unit costs measured on smaller variants of real cases (the original's and
larmorx's GiB and hours). A case predicted over the case's memory limit is not compared: larmorx's
default runs alone, with the limit as `LARMORX_MEMORY_LIMIT`, and must stop at once with its
guard's clear error (ANTS-162, AFNI-75), the verdict *guard* (a finding, `guard missing`, if it
does not); where larmorx's own need fits, or the implementation has no guard, nothing runs
(*memory*). Either way the prediction is kept in the case's line (`predicted`) and the case is
queued in the night's `heavy.jsonl`, for the heavy box (`difftest --heavy`), and on a larger
machine when it is beyond this one ("pending: larger machine",
`validation/investigations/difftest/heavy_pending.py`, with the command line that runs it there).
The tests: `validation/tests/test_difftest.py::test_cases_are_realistic_and_an_extreme_share_is_labelled`, `::test_a_nights_extreme_share`
and `::test_the_sizing_laws_and_a_case_predicted_over_the_limit`.

| Tool | Parity runner | Implementations | Drawn |
|---|---|---|---|
| `nifti-io` | `parity.nifti_io` | larmorx vs nibabel | every NIfTI form, type, scaling, geometry, extension, NIfTI-2, byte order |
| `itk-transforms` | `parity.itk_hdf5` (`read`) | larmorx vs ITK, h5py, nitransforms | transform lists in text, MATLAB and HDF5 files (every linear type, 2D, float, composites, displacement fields) |
| `ants-apply-transforms` | `parity.ants_apply_transforms` | port | images, reference grids, chains of linear transforms and fields, inversions, every interpolator, `--float`, `-u`, `-f`, `-e 3` |
| `ants-image-math` | `parity.ants_programs` | port | every operation larmorx ports with its operand forms, dimensions 2 to 4 |
| `ants-threshold-image`, `-multiply-images`, `-smooth-image`, `-resample-image`, `-resample-image-by-spacing` | `parity.ants_programs` | port | their argument forms |
| `ants-n4`, `ants-denoise` | `parity.ants_threaded` (no thread band) | port | masks, weights, shrink factors, convergence, B-spline meshes and orders, histograms, noise models, patches, searches |
| `ants-registration` | `parity.ants_registration` | port | one to three stages of every family with every image metric, sampling, levels, masks, initial transforms, global options (never a linear stage after a deformable one, ANTS-72's clock) |
| `ants-ai`, `ants-affine-initializer` | `parity.ants_ai` | port | metrics, transforms, principal axes, searches, grids, masks, the blob alignment, outputs |
| `ants-atropos` | `parity.ants_atropos` | port | initializations, likelihoods, posteriors, MRF, ICM, outliers, partial volumes (seed fixed) |
| `afni-tshift` | `parity.afni_tshift` | clean-room original, replica | series, slice timing in the header and as `-tpattern` (names, files, `@1D:`), every method and option |
| `afni-automask`, `afni-unifize` | `parity.afni_replica` | clean-room original, replica | every option of each |
| `mri-hmc` | `parity.mri_hmc` (a run recorded live) | clean-room original vs mcflirt | moved phantoms, mcflirt's options |
| `fsl-mcflirt` | `parity.fsl_mcflirt_replica` | replica vs mcflirt | the same |
| `fsl-bet` | `parity.fsl_bet` (`expect="random"`: a case where FSL crashes and `reproduce` fills is judged as the record's crash-filled cases) | replica vs bet2, bet, betsurf, fslroi, standard_space_roi | bet2's options, bet's variations (`-R`, `-F` on a series, `-Z` on a slab; `-S`, `-B`, `-A`, `-A2` with a T2 on the same grid since 2026-11-04, FSL's standard images from `$FSLDIR/data`) with them, betsurf with and without a T2 on a closed surface made by the descriptor; fslroi's three argument forms with regions outside the image, sizes of -1 and 0, int8 inputs (FSL-217); standard_space_roi's options with drawn masks, `-ssref` mostly, FLIRT options after them (since 2026-11-04); every stored type, FSL's output types |
| `fsl-flirt` | `parity.fsl_flirt_replica` (a whole `Run` as the case's payload: inputs as `@dt:` names, FSL's schedules as `@fslsch:`) | replica vs flirt, convert_xfm | registrations with every cost and search cost, degrees of freedom, bins, the search options, `-init`, weights, `-usesqform`, the resampling options; BBR with FSL's `bbr.sch`, a one-label segmentation, its types and slope; `-applyxfm` and `-applyisoxfm` on 3D and 4D inputs with every interpolation, `-paddingsize`, `-setbackground`, `-datatype`; `convert_xfm` `-inverse`, `-concat`, `-fixscaleskew`; every stored type, FSL's output types (not drawn: what the replica refuses as not yet replicated, and what makes FSL loop for ever or abort, FSL-21, FSL-66, FSL-71, FSL-72) |
| `fsl-flirt-programs` | `parity.fsl_flirt_replica` (the same payload; registered for part 2's short trials of the new paths) | replica vs flirt, avscale, img2imgcoord, std2imgcoord, img2stdcoord, rmsdiff, makerot | `-anglerep quaternion` registrations; avscale's matrices (`--allparams`, `--inverteddies`, a volume's centre of gravity); the coordinate programs' voxel and mm coordinates from files and standard input; rmsdiff with and without a mask; makerot's axes, centres and angles (since 2026-11-01) |
| `fsl-flirt-part3` | `parity.fsl_flirt_replica` (the same payload; registered for part 3's trial of the new paths) | replica vs flirt, midtrans, pointflirt, applyxfm4D, extracttxt | midtrans of 2 to 4 drawn transforms (`-o`, `-v`, `--debug`, `--separate`, `--template`); pointflirt of 1 to 20 points and their drawn affine images (`-o`, `-v`, `--vox`); applyxfm4D on 3D and 4D images of every stored type with every interpolation (`-3D`, `-singlematrix`); flirt schedules with `alignpaxes` or a drawn parameter-subset mask; extracttxt with drawn words, counts and relative starts, negative ones included (since 2026-11-01) |
| `fsl-fast` | `parity.fsl_fast_replica` (a whole `Run` as the case's payload: inputs as `@dt:` names; `-p` and multi-channel `-N` cases never cached, FSL's never-written voxels set to the defined values as in the suite) | replica vs fast | fMRIPrep's call; one to four channels of one brain-extracted phantom (T1-, T2- and PD-like contrasts, 2D slices, whole heads, crops of a real T1w), every option (short and long names), FSL's tissue priors with a drawn standard-to-input matrix (`-a`, `-P`), `-s`, every stored type, FSL's output types (since 2026-11-02) |
| `fsl-fast-original` | `parity.fsl_fast` (the replica's draws, `difftest/tools/fsl_fast_original.py`; its `Run` payloads and treatment of FSL's never-written voxels; the permanent cases in `validation/difftest/cases/fsl-fast-original/`) | clean-room original vs fast | the same as `fsl-fast` (since 2026-11-03; its trial with `LARMORX_DIFFTEST_CACHE_MODE=use`) |
| `fsl-topup` | `parity.fsl_topup_replica` (`difftest_case`: a topup `Run`, or FSL's topup run of the case's own pair through the oracle cache and then an applytopup `Run` on its coefficients and movements; inputs as `@dt:` names) | replica vs topup and applytopup | EPI-like phantom heads distorted along their phase encodings by one off-resonance field (gen.py's `distortion`: the intensity times the displacement's Jacobian), small movements; y or x pairs, three or four volumes; topup's per-level options (one to three levels), its global options, `--datain` or `--acqp`, configuration files (the case's own, FSL's shipped ones), every output, `-v`, `--debug`, `--trace`; fMRIPrep's call (sdcflows' b02b0.cnf); applytopup's five methods, `--interp`, `--datatype`, short and long names; FSL's output types (not drawn: phase encodings along z, sizes the subsamplings do not divide, the record's refusals and failures, `--profile`, more than one thread: FSL-116; since 2026-11-03) |
| `fsl-maths` | `parity.fsl_maths_replica` (a `Run` of fslmaths' arguments; inputs as `@dt:` names) | replica vs fslmaths | six kinds of images (scan-like heads, series, masks, z and p maps, tensors) on grids of 6 to 32 voxels per axis; chains of one to four operations drawn for the current image from every family (arithmetic with numbers and images, thresholds, functions, kernels and filters, morphology, subsampling, edges, reductions along each dimension, statistical transforms, `-roi`, `-bptf`, TFCE, ROC, the tensor's decomposition), `-dt`, `-odt`, the output's name and `FSLOUTPUTTYPE` (not drawn: the record's refusals and crashes, `-rand`, `-randn` without `-seed`; since 2026-11-03) |
| `fsl-stats` | `parity.fsl_stats_replica` (a `Run` of fslstats' arguments, the `direct` kind) | replica vs fslstats | the same kinds of images and masks; one to five options in order, or fMRIPrep's (goodvoxels' `-M`, `-S`) and BET's (`-l <t> -C`) forms, with `-t`, `-K` and `-json` (since 2026-11-03) |
| `fsl-topup-original` | `parity.fsl_topup` (`difftest_case`; the replica's draws, `difftest/tools/fsl_topup_original.py`, from the `fsl-topup` stream, so the replica trial's seed serves every FSL run from the oracle cache; a random case's band: FSL with its read-out times changed in their last float bit, through the cache; the permanent cases in `validation/difftest/cases/fsl-topup-original/` (7)) | clean-room original vs topup and applytopup | the same as `fsl-topup` (since 2026-11-03; its trial with `LARMORX_DIFFTEST_CACHE_MODE=use`: 14,400 cases in 11 minutes, 7,401 agree, the 6,999 others triaged by category in `triage.toml`) |
| `fsl-maths-original` | `parity.fsl_maths` (`difftest/tools/fsl_avwutils_original.py`: the replica's draws from the `fsl-maths` stream, so the replica trial's seed serves its FSL runs from the oracle cache; the suite's `direct` payloads; the permanent cases in `validation/difftest/cases/fsl-maths-original/` (17)) | clean-room original vs fslmaths | the same as `fsl-maths` (since 2026-11-03; its trial with `LARMORX_DIFFTEST_CACHE_MODE=use`: 150,000 cases, 143,191 agree, 804 differ only by a named correction, the 6,005 others re-run with the final build and triaged by category in `triage.toml`) |
| `fsl-stats-original` | `parity.fsl_stats` (the same module, the `fsl-stats` stream; the permanent cases in `validation/difftest/cases/fsl-stats-original/` (4)) | clean-room original vs fslstats | the same as `fsl-stats` (since 2026-11-03; the same trial: 150,000 cases, 149,214 agree, 7 differ only by a named correction, the 779 others triaged in `triage.toml`) |
| `fsl-prelude` | `parity.fsl_prelude_replica` (a `Run` of prelude's arguments, the `direct` kind; inputs as `@dt:` names; a replica's refusal in `reproduce` where FSL's NaN sort would read memory it does not own, FSL-268 or FSL-269, named to the runner: *undefined*; the permanent cases in `validation/difftest/cases/fsl-prelude/`) | replica vs prelude | fMRIPrep's call (sdcflows' `--abs --mask --phase --unwrap`, the phase difference from 0 to 2 pi); a phase and a magnitude, or a complex64 image (big-endian ones included, FSL-264), over a head phantom with a random background: phases wrapped into [-pi, pi), [0, 2 pi) or not (FSL's range error), flat to many cycles with a bump, stored as floats, scaled int16 or uint16 and a scanner's unscaled values; every option (`-n`, `-t`, `-m`, `--labelslices`, `-s`, `-f`, `--removeramps`, `-v`, `--savemask`, `-l`, `-r`, the output's four names, short and long names, grouped flags), series with `--start` and `--end` (FSL-262, FSL-270), images of another size (FSL-271), NaN phases and magnitudes (FSL-268, FSL-269), one-slice and thin images, FSL's output types (not drawn: `-c` with other types, FSL-263 and FSL-265; images wider than 64 voxels; volume indices outside the series; since 2026-11-06; its trial `oracles/difftest/2026-11-06-prelude-replica`, seed `prelude-trial`, 30 minutes, 12 jobs at one thread, 1.5 GiB a case, FSL through the oracle cache: 50,711 cases, 50,702 bit-identical, the 9 others the descriptor's own draw, fixed and bit-identical drawn again; a two-minute check run before it found two imprecise D13 notices of the replica, fixed) |
| `resample-series` | `parity.resample_series` | port vs fMRIPrep's `ResampleSeries` | series, head motion, coregistration, targets, field maps, orders, modes |

**Verdicts** (`difftest/core.py`): *agree* (the parity runner passed, or both programs rejected the
input alike; for a replica or port every reported bit-identity check is identical); *explained*
(the values differ from the original as built only through glibc's maths: identical with
CORE-MATH preloaded, by the suite's own second run or by a re-run of the case with the preload);
*notice* (the only failed check is the fix pass naming a correction that changed nothing this
input shows: D13 notices are meant to fire exactly where the result changes, so these are kept
and counted per defect, not shrunk); *undefined* (the original's result depends on memory it
never wrote or on its clock, and larmorx names the defect while reproducing it, refuses with
the defect's ID, or the descriptor knows the drawn case meets it, `extra["undefined"]`: e.g.
ANTS-82's clock-seeded sampling of the velocity-field stages; not comparable, never cached);
*differ* (a finding); *memory* (a run exceeded the case's memory limit: queued as a heavy case,
below); *timeout*, *crash* (unclear, for the orchestrator). Replicas and ports must
be bit-identical (compared in `reproduce` mode; the fix pass checks that a D13 notice is named
exactly where the default changes the result); clean-room originals are held to their records'
thresholds.

**Crashes:** a case that kills its worker (larmorx aborting, a Rust allocation failure) is run
again in a child process of its own, and shrunk there, each candidate in a child.

## Memory and time limits; heavy cases

On 2026-10-25 one random N4BiasFieldCorrection case of the first night (a 4D image whose header
gives its voxels in metres, with a spline distance of 20 mm: ANTS-162) grew to 28 GB resident;
the machine swapped until the editor and every agent running in it went down. Since then
(`validation/src/larmorx_validation/difftest/limits.py`):

- **the night runs in a memory box**: `scripts/orchestration/difftest-nightly.sh` runs inside
  `scripts/orchestration/boxed.sh 8 ...` or boxes itself (a systemd user scope of 8 GiB,
  `LARMORX_DIFFTEST_BOX_GIB`, and no swap); `python -m larmorx_validation difftest` (a night,
  `--case`, `--replay`) re-runs itself in such a box when it runs in none (`--box-gib`);
- **each worker has a hard limit**: a cgroup v2 leaf of the box with `memory.max` of
  `--memory-gib` (3 GiB by default, `LARMORX_DIFFTEST_CASE_GIB` for the nightly script, many
  times what an input of at most 32^3 voxels needs; a worker's peak is about 0.3 to 1.5 GiB) and
  `memory.swap.max` 0. The worker enters its leaf before it runs anything, so every original
  program and larmorx run of its cases is held there;
- **a run over the limit is an outcome, not a crash of the harness**: the kernel stops the
  largest process of the leaf; the verdict is *memory*, "the original <program> exceeds 3 GiB"
  (a child process stopped, the worker alive: read from the leaf's `memory.events` and the
  exits the worker saw) or "larmorx exceeds 3 GiB" (the worker, which runs larmorx in-process,
  stopped: each implementation then runs again in a child process of its own, as a crash
  does; that process also compares the outputs, whose arrays count toward the limit, so a large
  output can stop it where larmorx itself fits); a fresh worker takes the leaf and the night goes on (the self-test tool
  `selftest-memory` and the test `test_a_case_over_the_memory_limit_is_recorded_and_the_night_goes_on`
  prove it on a real boxed night);
- **time**: each run has its alarm (the tool's timeout) and each case its deadline, past which
  its worker's process group is killed (*timeout*); then every other process of the worker's
  leaf is stopped as well (`Leaf.stop_others`, 2026-10-29: killing a program kills that process
  only, and a script's own children, FSL's `bet` running `bet2`, had lived on for hours);
- **address space** (2026-10-28): every original program runs under `RLIMIT_AS` = 64 GiB
  (`larmorx_validation.address_space`, `LARMORX_ADDRESS_SPACE_LIMIT`), inherited by what it
  starts, and larmorx's in-process runs of the ANTs image programs with the worker's soft limit
  lowered to it: an allocation beyond it is refused at once on every machine, whatever its RAM
  and swap (Linux refuses one only above about RAM plus swap: the old laptop refused the 96 to
  111 GiB the permanent cases `allocation-refused` and ImageMath's `MD -1` ask for, the new
  machine granted them, machine-move-proof.md). Random testing derives the limit from the
  case's own memory budget, keeping the measured margin of address space over resident memory
  (threads' and OpenMP's stacks): twice the budget, never below 64 GiB. A night (3 GiB a case)
  runs under 64 GiB, so a case asking for more address space is refused by both programs alike
  (*agree*, as the permanent cases record); `--heavy` runs under twice its box (memory and swap:
  144 GiB for 24 + 48 GiB, about 2.2 TiB for the largest pending prediction, 1140 GiB, on the
  1 TB server), so the limit never refuses what a heavy case's box allows and its cases run to
  their end. An explicit `LARMORX_ADDRESS_SPACE_LIMIT` is kept.

**Heavy cases are run, not avoided** (user, 2026-10-25). Every case with a *memory* verdict is
queued in the night's `heavy.jsonl`. `python -m larmorx_validation difftest --heavy
oracles/difftest/<date>` runs the queue later, one case at a time, each implementation in a
child process in a leaf of 24 GiB with 48 GiB of swap (`--heavy-hours`, 2 per run), only when the
machine is quiet (the user says when); the results, with each run's peak memory, go to
`heavy-results.jsonl`, and the report's "Heavy cases" section lists every queued case with its
result (or "waiting"). Each case runs with the recipe its night drew: a seed and case number
name a case only for the generator of the night's commit (`run.json`), and the generator
changes (the realistic share, 2026-10-25, draws every case of the first night's queue
differently), so the queue line keeps the recipe itself, and for a night queued before that the
night's finding of the case does (`runner.night_recipe`; a case with neither is skipped with
the reason). Before a queue is run, `validation/investigations/difftest/heavy_predict.py`
predicts each case's memory and time from the original's sizing law and the unit costs measured
on smaller variants of the same case (`n4_memory.py`, `scaled_memory.py`; 3dUnifize's ball of
neighbours from AFNI's own formula). A case predicted beyond the heavy box (24 GiB and 48 GiB of
swap) is not run: `heavy_predict.py --record-beyond` writes its result, *beyond this machine* or,
above 1 TiB, *beyond any machine*, with the prediction, and `--heavy` skips it. Where such a size
grows from a header's units (ANTS-162, AFNI-75), larmorx's default checks the memory before it
allocates and stops at once with the GiB needed and the likely cause (`LARMORX_MEMORY_LIMIT` sets
another limit than the machine's memory; `reproduce` attempts the computation); a permanent case
per kind (`--expect guard`) runs only that check.

## Shrinking

`difftest/shrink.py`, greedy and deterministic: from the case that differs, try each move in turn
and keep the first that still shows the difference (it differs, and one of the checks that failed
still fails), then start again, until no move keeps it or the time budget is spent. The moves, the
tool's own first: drop an optional option group (and the inputs only it named); halve the longest
axis, keep fewer volumes; a plumb orientation, unit spacing, the origin at 0, an sform equal to the
qform, codes 1, millimetres; float32, no scale factor, plain values, no slice timing; a simpler
content (a constant, a ramp, a blob). The finding keeps both recipes and the moves.

## Permanent cases

A difference worth keeping becomes `validation/difftest/cases/<tool>/<id>.json`
(`difftest --promote`): the shrunk recipe, where it was found, what was done (`fixed` in larmorx,
`register` with its defect ID and rule, `open` for a clean-room original's agent), its tier and
expectation. The tool's parity suite loads it as a regular case, `difftest/<id>`
(`difftest.permanent.cases`), in the tiers it names: its inputs are rebuilt from the recipe, the
original's runs replay from the oracle cache.

## Triage

Every difference is decided by the user's rule (2026-10-24): a clear larmorx mistake in a port
or replica is fixed at once (bit-identical again, with a unit test, a permanent case and the
gates); a clean-room original's difference is reported for its agent with the black-box
reproducer (an agent that has read the upstream source never edits a clean-room crate); a defect
of the original goes into the register, decided by rules A-D with the D13 switch where one is
required; anything else is listed for the orchestrator. What was done about each is written in
`validation/difftest/triage.toml` (tool, implementation, the failed checks, `status` fixed |
register | open | harness | explained (an expected difference, measured and explained: no mistake
and no defect) | unclear, the explanation, the permanent case or finding it links to);
the site's "Random testing" panels show it beside each night's differences.

The tools behind the findings are in `validation/investigations/difftest/` (its README): replaying
a finding by hand against ANTs, AFNI or mcflirt, bisecting a registration's options, probing an
original's behaviour on edge cases.

## The oracle cache

Random inputs must not make the cache grow without bound, and every finding must replay exactly
(`docs/validation/oracle-cache.md`):

- **random cases run the originals with the cache off** (`LARMORX_ORACLE_CACHE_MODE=off` in the
  workers): nothing is read or stored; random inputs are new keys every night, and storing them
  would only grow the cache;
- **a finding's shrunk reproducer is run once more with the cache on** (`use`): its original
  runs are stored (one entry per run, a few kilobytes of tiny inputs and outputs), so its replay
  and the permanent case built from it are served from the cache;
- so the cache grows only by the findings, a few per night at most; nothing is ever deleted.
- **a part's short trial** may run the originals through the cache in `use` mode instead
  (`LARMORX_DIFFTEST_CACHE_MODE=use`, `runner.explore_mode`; `off` stays the default for nights):
  its original runs are stored, so its replays, re-checks and permanent cases are served and no
  original result is computed twice (every FSL run of a part through the cache, the briefs since
  2026-11-02; FAST's clean-room trial of 2026-11-03 was the first: 9,005 cases, a few hundred MB;
  topup's replica trial of the same day the second: `oracles/difftest/2026-11-03-topup-replica`, seed
  `topup-trial`, 27 minutes, 12 jobs at one thread, 1.5 GiB per case: 14,460 cases, every one
  bit-identical, nothing to triage; fslmaths' and fslstats' replica trial of the same day the third:
  `oracles/difftest/2026-11-03-fsl-maths-replica`, seed `fsl-maths-trial`, 24 minutes, 13 jobs at
  one thread, 1.5 GiB per case: 183,983 cases, fslmaths 114,906 (one difference: GLIBC-07, the FFT's
  twiddle factors) and fslstats 69,077 (nine: the replica's, fixed), after a one-minute check run
  of 5,186 cases (three of the replica's, fixed); their clean-room originals' trial of the same
  day the fourth: `oracles/difftest/2026-11-03-fsl-maths-original`, seed `fsl-maths-trial` (the
  replica trial's cases drawn again, then more), 20 minutes, 13 jobs at one thread, 1.5 GiB per
  case: 300,000 cases; fslmaths 150,000 (143,191 agree, 804 a named correction only, 6,005
  differ) and fslstats 150,000 (149,214, 7 and 779); every finding re-run with the final build
  and put in one category (`validation/investigations/cleanroom-maths/trial_triage.json`): 5,168
  the original's mistakes, fixed with 21 permanent cases, and 58 chains those fixes reach, 170
  the suite's, 1,338 explained, 33
  expected (`-subsamp2offc`'s means), 7 FSL-75 (registered), 10 open (8 display ranges after
  `-range` in their last bits, 2 `-roc` curve numbers 2.4e-7 and 5e-7 from FSL's)).
- a run whose original varies on its own machine is never cached, whatever the case's texts say:
  the cache applies ANTs' program-level rules to every ITK run (`policy.ants_never`: the
  velocity-field stages' clock-seeded sampling, ANTS-82; a smaller second image read past its
  end, ANTS-07; a thin axis smoothed, ANTS-28). The trial runs of 2026-10-25 had stored 22 such
  runs (18 of them differed when the move's verify ran them again, machine-move-proof.md); they
  are never served again.
- the inputs' folder is `lx-difftest-inputs_<random>`: its prefix ends in `_`, so its random
  part never makes option-like text (`-h`) that a program reading its argument's text sees
  (bet2 with one argument; `policy.path_text`).

## Before the first night: three trial runs

Three short runs (half an hour each at two jobs, 2026-10-25, seeds `trial1` to `trial3`; 642,
1563 and 1678 cases) tested the harness and found, before the first night, what the
triage file lists as fixed: in the ports, ITK's sform/qform choice made in vnl's float SVD
(larmorx decided in double), allocations ITK refuses (larmorx aborted), singular directions ITK
refuses, composite transform names ITK cannot create, a float transform's point indexed in
float, vnl's SVD warning on a NaN Mattes histogram, ImageFileWriter's origin (the sign of a
zero) and its refusal of an empty image, Atropos' k-means on an empty mask, the CC metric's
Jacobian (ITK-35 thrown where ITK does not); in the GPL replicas, AFNI-03's notice in 3dAutomask
and 3dUnifize; and in the harness its own mistakes (an infinite value compared with itself, the
fix pass of options the program never reaches, ANTS-82's clock). Three defects of the originals
went into the register: ANTS-160, ANTS-161 and ITK-95.

## The first night

The first night began on the morning of 2026-10-25 and took the machine down after 670 cases:
by 08:35 one N4 case (ANTS-162) had grown to 28 GB (its results are kept in
`oracles/difftest/2026-10-25-oom/`). The
memory box and the per-case limits above were built that morning, and the night ran again at
11:03, boxed (6.2 h, 3 jobs at one thread, seed `20261025`, the same
seed, so its first cases are the crashed night's): 34 880 cases of 21 tools; 31790 agree, 3 explained, 1350 notice, 394 undefined, 1227 differ, 76 memory, 38 timeout; 58 groups of differences: 34 fixed in larmorx, 6 defects of the originals in the register, 17 open for the clean-room originals' agents, 1 the harness's own; 76 heavy cases queued
(`oracles/difftest/2026-10-25/report.md`). Its harness was commit `fa9dfcf`, its larmorx build and
replica programs those of `1d971f1` (the same Rust).

What it found, by kind (each difference's triage in `validation/difftest/triage.toml`):

- **larmorx mistakes, fixed the same day** (each with a unit test and, where the original's
  result is defined, a permanent case): ITK's shortest numbers at a tie (a float B-spline
  parameter written one digit off); four transform classes antsApplyTransforms could not apply
  (QuaternionRigid, ScaleVersor3D, ScaleSkewVersor3D, Scale); a float versor's cosine squared in
  double; the empty sample set's exception (line and class); antsAI's warnings in ITK's order;
  the median's zero of a box holding both zeros (libstdc++'s introselect); DenoiseImage's and
  N4's dimension without `-d` (ANTS-99) and the Rician bias's smoothing kernel (ANTS-14); the NIfTI
  writer's warning for direction cosines orthogonal in double but not in float; a field written
  under a linear stage's `.mat` name keeping its class; and in `reproduce`, ITK's aliased
  derivatives of sample points outside a shrunk region (ITK-97, displacement-field, SyN and
  B-spline SyN stages) and a later level's zero Mattes bin size (ITK-100). The 2D B-spline fit
  of N4 (three times ANTs' memory per control point and twelve times its time, found by a heavy
  case) now splits its work by lattice rows; ResampleImage's B-spline on a 2-voxel axis no
  longer panics (ITK-98).
- **defects of the originals, in the register by rules A-D:** ANTS-162 (rule C for the method,
  with a guard by default: lattices, paddings and grids sized from the physical extent), ANTS-163
  (a mask of another size read outside its buffer), ITK-97 (rule A), ITK-98, ITK-99, ITK-100
  (rule B), AFNI-75 (3dUnifize `-EPI`'s ball of neighbours on micrometre voxels: rule C with a
  guard by default, as ANTS-162); ANTS-161, ITK-95 and ITK-96 met again (their `reproduce`
  switches are follow-ups).
- **the replicas of other parts:** the mcflirt replica differed in 107 cases; its own part's
  fixes (an atanf constant, and FSL-08's notice of the `-meanvol` name) make every one of them
  bit-identical.
- **the clean-room originals, reported for their agents** (black-box reproducers): 3dTshift's
  header under AFNI-07's correction and `toffset`'s last bits; 3dAutomask's "[from ?]" and its
  fix pass's messages; 3dUnifize's header and AFNI-08 notice; the head-motion correction beyond
  its record's thresholds on small phantoms (782 cases), whose alignment is as good as
  mcflirt's (`hmc_residuals.py`: the corrected series' temporal deviation within 1 % of mcflirt's
  in 22 of 40 sampled cases, lower in 8, higher in 10).
- **heavy cases:** 76 cases exceeded the 3 GiB limit of a case. Their predictions
  (`heavy_predict.py`) put 11 beyond any machine (the N4 lattices of ANTS-162, the 3dUnifize
  balls of AFNI-75: never run; permanent cases expect the guards' errors). The heavy box ran 40
  on the evening of 2026-10-25 (batch 1, 2.67 h): 26 agree, 14 notices that changed no value, no
  difference. 25 wait for a larger machine, each with its prediction and command line: 13 N4
  cases too long here (51.5 h in all), 8 beyond the heavy box, and 4 of batch 1 to run again
  with the harness's fixes (two comparisons that held both outputs of billions of voxels, one
  that started before a fix of that comparison, one original past the suites' 30-minute run
  limit).
- **open:** ITK-01's notice in antsRegistration's field lookups (the default's exact floor
  changes a value in its last bit with no notice); one voxel of `resample_series` one float ulp
  from fMRIPrep's; the timeouts (38: afni-unifize 1, ants-ai 1, ants-n4 14, ants-registration 10, ants-resample-image 8, ants-resample-image-by-spacing 3, ants-smooth-image 1).
