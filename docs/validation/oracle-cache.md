# The oracle-output cache

An original program's output for the same build, inputs, arguments and settings never changes,
so the parity suites compute it once (decision log 2026-10-23, "speed at the same quality",
point 1). Every run of an original program the suites make goes through a content-addressed
cache: a run seen before is not run again, its outputs are put back and its exit status and
messages returned; only larmorx runs. Runs whose result the original itself varies are never
cached, because that variation is what the records measure. The code is
`validation/src/larmorx_validation/oracle_cache/`; its tests are `validation/tests/`.

## Using it

Nothing to do: every parity run uses it (`python -m larmorx_validation parity ...`,
`pytest tests/parity`, `scripts/orchestration/verify-merge.sh`), in the mode `use`.

| | |
|---|---|
| Where | `LARMORX_ORACLE_CACHE`, else `<workspace>/oracles/cache` (outside git; never commit it) |
| `--oracle-cache use` | the default: a run in the cache is served; a run that is not runs and is stored |
| `--oracle-cache refresh` | every run that may be cached runs again and is compared with its entry; a difference is reported loudly, kept, and fails the command |
| `--oracle-cache off` | the cache is neither read nor written (the records' "before") |
| `LARMORX_ORACLE_CACHE_MODE` | the same for `pytest tests/parity` and any other caller (the option sets it for the `--jobs` workers) |
| `python -m larmorx_validation oracle-cache verify --sample N` | re-runs N entries chosen at random, away from their suites, and checks they give the same result: the drift detector (a changed oracle changes the key; this catches what the key misses). `--program`, `--max-seconds` (only entries whose run took at most that long: a quick check), `--jobs`, `--seed`; `--keys KEY...` (or `@file`, one per line: a verify log works) re-runs those entries, disputed ones included; `oracle-cache --cache DIR` works on another cache (`validation/investigations/oracle-cache/copy_entries.py` copies chosen entries into one, so a verify writes nothing in the workspace's) |
| `... oracle-cache confirm` | runs the repetitions the runs above one thread still need before they are served (below) |
| `... oracle-cache stats`, `... disputes` | what the cache holds; the entries a run disagreed with |
| `... oracle-cache resolve KEY... --check --reason TEXT` | a dispute that was no difference of the result (something that changes from run to run the comparison did not mask, masked since): with `--check` the entry runs again and is served again only if it gives the same result; the reason goes into its history and `findings.jsonl`, the dispute's record stays |

Each record's header says how many original runs came from the cache and how many ran, and
where the cache is, for example

> - **Original programs:** 657 runs: 509 from the oracle cache, 148 run (124 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

(`ants-registration`, standard tier, warm: the 124 are its thread band, its clock-seeded
cases and those that read memory ANTs never wrote; the others ended by a signal.)

and its JSON has the same in `oracle_cache` (with why each run that ran was not served). Nothing
else in a record depends on the cache: a record made from the cache is byte-identical, apart from
that line and the timings, to one made without it (below, "The proof").

## The key

A SHA-256 over the parts below, kept readable in every entry (`parts`).

| Part | What | Why |
|---|---|---|
| The oracle | the program's SHA-256 and the SHA-256 of every shared library it loads (`ldd`: glibc's `libm` and `libc` decide the unmodified programs' maths); for a script, every file of its build directory; for a container, the image's SHA-256, the program's name and Apptainer's version | a rebuilt oracle, an updated glibc or another image is another oracle |
| The arguments | as given, with every path replaced: a file that exists is an **input**, `{in0}`, `{in1}`, ...; a directory the run writes in is a **place**, `{cwd}`, `{out0}`, `{d0}`, ..., with the path below it kept. A path with blanks in it is one path (`/tmp/x/with blank/in.nii`); files named by a printf pattern (Atropos' `prior%02d.nii.gz`) are inputs, by their content and number (`series`); the text a program reads in a path (`policy.path_text`) is kept where a replaced path holds it (`text`; below) | the inputs count by their content, never by their directory or name; the outputs by their role |
| The inputs | each input's content (for `.gz` files the decompressed stream: the gzip header's time and the compression level do not count), its suffix (`.nii.gz`, `.mat`, `+orig.HEAD`: programs choose a reader by it), the files read beside it (`.img` beside `.hdr`, `.BRIK` beside `.HEAD`), and the shape of a path not written normally (`a/sub/../x`, which the program prints); files a program reads that its arguments do not name as files, when the suite names them (`inputs=`: FSL's images named without their extension, `bet in out` reading `in.nii.gz`), count by their content alike | the result depends on the bytes a program reads |
| The directories | the directories that exist inside the places along the arguments' paths | writing into a missing directory is not writing into one that exists |
| The environment | the variables the program's family reads: ITK and ANTs (`ITK_*`, `ANTS_*`, `NSLOTS`, the variables `ITK_NUMBER_OF_THREADS_ENV_LIST` names), AFNI (`AFNI_*`, `NIML_*`, `OMP_*`, `GOMP_*`: its OpenMP runtime is GNU libgomp; `HOME` by its contents), FSL (`FSL*`, `OMP_*`); for every program `LD_PRELOAD` (by its files' content: the CORE-MATH preload), `LD_LIBRARY_PATH`, `GLIBC_TUNABLES`, `MALLOC_*`, the locale (`LANG`, `LC_*`) and `TZ` | thread counts, seeds, the maths library and the program's own settings change results |
| The machine | the architecture, the CPU's model and flags (glibc picks its maths kernels by them); the number of CPUs when the run sets no thread count | the same binary on another CPU may compute differently |

Left out on purpose: the larmorx commit (the original does not depend on it), directory and
file names (unless the program reads their text, below), `PATH`, the date, the variables of other families (the user's `FSLDIR` is not part
of an ANTs run's key), and in `HOME` the files AFNI writes and never reads (`.afni.crashlog`,
`.afni.log`). So the orchestrator's re-run of an agent's record, in another checkout, finds the
agent's runs; the replica suites of 3dTshift, 3dAutomask and 3dUnifize find the original
suites' runs of AFNI.

## Keyed since 2026-10-28: what the move's proof found the key missed

The move to the new main machine re-ran every entry (`oracle-cache verify`,
[machine-move-proof.md](../findings/machine-move-proof.md)): 89 did not replay identically on
either machine. Each kind is now reproduced or never cached; an entry stores `rules: 2` (outside
its key), so `verify` knows the entries stored before.

| What | Since 2026-10-28 | Entries re-keyed or retired (of 9,415) |
|---|---|---|
| **A path's text the program reads**: `bet2/one-argument` failed at random (its temporary folder `lx-fsl-gen-<random>` sometimes held `-h`, and bet2 with one argument containing `-h` prints its usage, while the cache served FSL's run keyed by content) | `policy.path_text(program, args)` names the text a run may read in its arguments' paths: whitespace for every program (a shell script expands `$1` unquoted; AFNI splits at a blank), `-h` and `--help` for bet2 with one argument (`bet2.cpp:811-818`). Where a path the key replaces holds it, the key keeps it, `parts["text"]` (`{"0": ["-h"]}`), and only then: every other run keeps its key. The suites' temporary folders end their prefix in `_` (`lx-fsl-gen_`, `lx-fsl_`, `lx-difftest-inputs_`), so mkdtemp's random part (letters, digits, `_`) never follows a dash; BET's suite refuses a temporary root holding `-h` with a clear error | none (bet2's one-argument entries stored its error, without `-h`) |
| **A path with a blank** (`afni-tshift/environment/path-spaces-ok`): taken apart at its blank, the file was no input (its content not in the key) | the pieces joined by blanks are one path where they name an existing file or folder, or a file in an existing folder; the blank is text the program reads (above); `verify` makes a place whose name holds a blank with its name | 14 re-keyed (3dTshift 12, 3dAutomask 1, 3dUnifize 1) |
| **Files named by a pattern** (Atropos' `PriorProbabilityImages[3,{d0}/prior%02d.nii.gz,0.5]`): no inputs, so their content was not in the key and `verify` did not make them | the existing files a `%d` or `%0Nd` pattern names are inputs, by their content and number; `verify` puts them back by their names | 68 re-keyed (Atropos) |
| **A file in the working directory the arguments do not name** (mcflirt's `edge/mat-exists`: a file `out.mat` sends the matrices to `out.mat+`) | mcflirt's runner passes every file of its working directory as `inputs=` | 1 re-keyed |
| **Apptainer's variables** (fMRIPrep's `bet`, 38 entries: the suite's `--env HOME=...` makes Apptainer warn) | `verify` started the container with the key's variables only; an entry keeps every variable the suite set inside (`launcher.env`; for older entries they are read from the command), `HOME` as `{home}` | none (the key is unchanged) |
| **AFNI's history note in a NIfTI-2 extension** (3dTshift with `AFNI_NIFTI_WRITE_TYPE=2`, 6 entries): its length follows the run's paths | the comparison leaves NIfTI-2 extensions out as it does NIfTI-1 ones, where `Policy.nifti_extensions` says so | none |
| **A shell's report of a crash names the process ID** (`bet: line 399: 1915072 Segmentation fault ...`) | masked in the comparison | none |
| **Runs that vary on their own machine** (random testing's runs of 2026-10-25: antsRegistration's velocity-field stages, ImageMath and MultiplyImages with a smaller second image, ResampleImageBySpacing smoothing a thin axis) | never cached, by ANTs' program-level rules (below), which the cache now applies to every run of an ITK program whichever suite runs it | 23 never served again: 19 antsRegistration (ANTS-82: 18 random-testing runs and the permanent case `mattes-nan-bin-svd-warning`), 3 ANTS-07, 1 ANTS-28 |

A re-keyed entry stays where it is (nothing is deleted); its suite's next run stores the new one.
`verify` skips an older entry these corrections re-keyed, and a run the current rules never
cache, each with its reason (`rerun.stale`, `rerun.never_now`; the one entry whose re-keying the
entry itself cannot show is named in `rerun.SUPERSEDED`). The counts come from
`validation/investigations/oracle-cache/rekey_count.py`. On a copy of the 89 entries
(`copy_entries.py`): 45 identical (the 38 Apptainer runs, the 6 NIfTI-2 runs, BET's crash), 44
skipped with their reason (22 Atropos priors, 3 paths with a blank, mcflirt's `out.mat`, 15
ANTS-82, 2 ANTS-07, 1 ANTS-28).

## The address-space limit

Every original program the harness runs (served or not: the cache's runner, `verify`, random
testing) runs under `RLIMIT_AS` = 64 GiB (`LARMORX_ADDRESS_SPACE_LIMIT`, e.g. `96G`, `off`),
inherited by everything it starts; larmorx's in-process runs of the ANTs image programs run with
the harness's soft limit lowered to it (`larmorx_validation.address_space`). Linux refuses an
allocation only above about RAM plus swap: the cases that expect ANTs to fail to allocate (96 to
111 GiB: ImageMath's `MD` with radius -1, `difftest/allocation-refused` of ResampleImage and
ResampleImageBySpacing) failed as recorded on the old laptop (94 GiB) and were granted on the new
machine (323 GiB), where the box killed them; under the limit they fail alike on every machine.
The largest real call measured, mcflirt on the 600-volume multiband-8 BOLD series, peaks at 9.4
GiB of address space; the harness with larmorx in-process at 19.3 GiB (ImageMath on that series,
its comparison included) (`validation/investigations/address-space/`). `difftest --heavy` raises
the limit to twice its box.

## What is never cached, and why

The rules, applied in this order (`policy.py`; each has a test):

| Rule | Where it is decided | Why |
|---|---|---|
| A case marked so: its `nondeterministic` text, or its description or expected divergence names a defect whose original result varies (`VARYING_DEFECTS`: ANTS-07, -28, -42, -58, -63, -64, -66, -72, -77, -82, -103, -107, ITK-34, AFNI-20, -45, -47) | each suite's `case_never` | the clock, memory the source never defines, a race, a random identifier |
| Values that depend on memory the original never wrote (`AntsCase.undefined_values`, Atropos' `undefined`, AFNI's `Scenario.undefined`) | `case_never` | not determined by the source (D13) |
| A clock seed: `antsRegistration` without `ANTS_RANDOM_SEED` or `--random-seed`, and whatever the seed its `Exponential`, `BSplineExponential` and `TimeVaryingVelocityField` stages with `Regular` or `Random` sampling (ANTS-82); `antsAI` with `Random` sampling or `--random-seed 0` (ANTS-58); `Atropos` without `--use-random-seed 0`; the cases that run ANTs twice to measure its own spread (`expect="random"`) | `policy.ants_seed_never` (since 2026-10-28 applied by the cache to every run of the `itk` family), `case_never` | ANTs' generator is seeded from the clock |
| Memory ANTs never wrote, decided from the arguments and the images' headers: ImageMath's voxel-wise operations and MultiplyImages with a second image read past its end (ANTS-07); ResampleImageBySpacing smoothing an axis of fewer than 4 voxels (ANTS-28) | `policy.ants_memory_never` (with the clock rules, `policy.ants_never`: every run of the `itk` family) | the heap's contents (random testing's runs met them without naming them) |
| A hang (`expect="hang"`) | `case_never` | the outcome is the timeout's |
| The references the corrections record runs five times (ANTS-72) | `ants_registration_corrections.VARIES` | the record measures their spread |
| Above one thread, programs known to vary from run to run: `antsRegistration`, `antsAI`, `antsAffineInitializer`, `DenoiseImage` (`VARIES_ABOVE_ONE_THREAD`) | `policy.decide` | their thread bands are measured afresh |
| Any other run above one thread (ImageMath at four threads, N4's thread band, AFNI's OpenMP loops), or with no thread count set | `policy.decide` | stored, but **served only once three runs gave the same result** (the porting guide: a race shows only in repeated runs, at least three at every thread count above one); until then it runs and each run is compared with the stored one; a single difference and it is never served |
| A run that ends by a signal, or by its timeout | `core._settle` | a crash is memory the source does not define; a timeout is the machine's clock |
| A run whose outputs or messages hold its temporary paths in a form the cache cannot map back, or that writes outside the places it knows | `core._capture` | the replay would not be what a run gives (AFNI's own `+orig.HEAD` datasets, which name their path; 3dTshift's NIfTI extension is the exception below) |
| A command a case runs a second time | `core._run` | it runs and is compared with the first (a case that runs the same command twice may be measuring the original's own spread); it is served to such a case only once three runs agreed, and one difference is a finding |

**3dTshift's NIfTI extension.** Without `AFNI_NIFTI_NOEXT`, AFNI writes its own header extension:
a random identifier, the date and the output's own path. The 3dTshift suite compares the header's
fields and the data, and whole files only with `AFNI_NIFTI_NOEXT=YES`, so its runs are cached
with `Policy.nifti_extensions` set: the extension is left out when runs are compared, and may
hold the run's path.

## The runners

| Suite(s) | Runner | Threads | Rules |
|---|---|---|---|
| `ants-image-math`, `-threshold-image`, `-multiply-images`, `-smooth-image`, `-resample-image(-by-spacing)` | `ants_programs._run_ants` | 4 (served after 3 identical runs) | `case_never` (undefined values, varying defects) |
| `ants-n4`, `ants-denoise` | `ants_threaded.run` | 1; the band at 2 to 12 (N4: after 3 runs; DenoiseImage: never) | as above |
| `ants-registration`, `-corrections` | `ants_registration._run_ants`, `_laplacian`, `corrections._apply` | 1; the band never | seeds, hangs, ANTS-72 / -82 / -64 cases, the five ITK-35 references |
| `ants-ai`, `ants-affine-initializer` | `ants_ai._run_ants` | 1; the band never | ANTS-58, hangs, `expect="random"` |
| `ants-atropos` | `ants_atropos._run_ants` | 1; the band after 3 runs | the clock seed, ANTS-107, `expect="random"` |
| `ants-apply-transforms`, `itk-hdf5` | `ants_apply_transforms._run_ants`, `ants_transform_output._run_ants`, `itk_hdf5._run` | 4 (after 3 runs) | crashes; HDF5 outputs compared by their datasets (ITK-09) |
| `afni-tshift` (both implementations) | `afni_tshift._run_afni_streams` | 1 (3dTshift has no OpenMP loop) | AFNI-20; the NIfTI extension |
| `afni-automask`, `afni-unifize` (both) | `afni_replica.run_afni`, `run_fmriprep_afni` (fMRIPrep's image) | `OMP_NUM_THREADS` (2: after 3 runs) | AFNI's undefined reads |
| `mri-hmc` | none | – | mcflirt's runs are recorded once already (`oracles/fsl-6.0.7/mcflirt`) |
| `fsl-mcflirt-replica` | `fsl_mcflirt_replica.run_oracle` (FSL in `~/fsl`) | 1 (mcflirt is single-threaded) | crashes (FSL-21's `corratio` abort); every file it writes is in its working directory (`outputs=[cwd]`), and every file there before the run is an input (`inputs=`, since 2026-10-28) |
| `fsl-bet` | `fsl_bet.oracle_run` (FSL in `~/fsl`; fMRIPrep's FSL through its image) | 1 (FSL's BET is single-threaded) | FSL-43 and FSL-53 (memory FSL never wrote), FSL-46 (the cache replays no removal), crashes; images named without their extension through `inputs=` |
| `nifti-io`, `resample-series`, `numpy-linalg`, `itk-geometry` | none | – | their references run in-process (nibabel, fMRIPrep's Python, numpy, ANTsPy) |

Not through the cache: `pipeline-stages` (it keeps its own runs in `oracles/compare` and reuses
them), the benchmarks (they measure time) and the evidence exports (the figures cache).

## How a run is stored and replayed

An entry (`entries/<aa>/<key>.json`) holds the key's parts, the result (exit status, both
streams and every output as blobs), the inputs (blobs, so `verify` can run it again), the places,
the command as it ran, the time it took, the date, the larmorx commit that wrote it, how many
identical runs confirmed it and its history. Blobs (`blobs/<aa>/<sha256>`) are content-addressed
and written once. Every file is written to a temporary name and renamed into place, an entry
after its blobs, under a lock per key (`fcntl.flock`): two `--jobs` workers storing the same run
store one entry, and the second confirms (or disputes) the first.

The outputs are the files the run created or changed in its places, the directories its
arguments name (an input's directory too); a file it wrote that is one of its inputs (AFNI's
`-prefix` over an existing file, a transform's field written again) is stored by its input's
role and replayed onto this run's input. The replay copies them back. In the streams, every
place, every input and every input's name is stored as a placeholder and replayed with this
run's: a replayed message names this run's files, as a fresh run would. A file written anywhere
else, or bytes that still hold the run's temporary paths, and the run is not stored.

## Differences: findings, never overwritten

When a fresh run of a stored key gives another result (`refresh`, `verify`, a confirmation, two
workers), the entry is marked disputed and never served again, both results are kept
(`disputes/<key>/<stamp>.json`, their blobs), a line goes to `findings.jsonl`, and the parity
command prints it (`refresh` and `verify` exit with status 1); `pytest tests/parity` lists them
at its end. Either the original varies (a case to mark, a finding for `docs/findings/`) or the
key misses something (a fix here). What is compared: the exit status, the streams with memory
addresses and elapsed times masked (`Elapsed time`, AFNI's `Elapsed = ... sec`, the times in
antsRegistration's `DIAGNOSTIC` lines, a shell's process ID in its report of a crash), the set of files written, and every file's bytes (`.gz`
files decompressed; HDF5 files by their datasets and attributes, as ITK stamps objects with
times, ITK-09; variable-length strings by their contents). What is served is always the stored
bytes.

## No duplicate runs

`pytest tests/parity` that passes, whole, at the smoke tier, from a clean commit, writes a marker
(`<cache>/markers/parity-smoke-<commit>.json`: the commit, the time, the result, the counts, the
SHA-256 of the extension it tested). `scripts/record_golden.py` reuses it for the same commit and
build instead of running the smoke tier again (`--rerun-parity` runs it anyway), and says so in
`golden.json`.

## The proof, and the gain

Measured on 2026-10-24 and 25 from clean commits (090cfc6, then 3e6ac7f after the fixes below)
with `validation/investigations/oracle-cache/measure.sh` and `remeasure.sh`, `--jobs 6`, on the
Linux x86-64 workstation (12 logical CPUs) **while other agents worked**: the load average was
8 to 31 (the table gives it). The times are therefore a busy machine's; a quiet machine's are a
follow-up. "Before" is `--oracle-cache off`, the behaviour before the cache; "cold" an empty
cache; "warm" the same run again.

| What | Before | Cold | Warm | Original runs (warm): from the cache, run | Load (mean) |
|---|---|---|---|---|---|
| Smoke tier, every suite as records (26 records) | 2021 s | 1793 s | 694 s | 2141, 522 | 13.4 / 11.3 / 14.3 |
| `oracle-cache confirm` after the cold run (the runs above one thread) | | 841 s | | | 13.6 |
| `pytest tests/parity` (the smoke tier as tests, one process) | 5654 s | | 1614 s | | 8.1 / 9.2 |
| `ants-registration`, standard tier | 7197 s | 7200 s | 2958 s | 509, 148 | 17.5 / 18.0 / 30.9 |
| `ants-image-math`, standard | 36.8 s | 42.8 s | 21.6 s | 314, 20 | as above |
| `afni-tshift`, standard | 494 s | 512 s | 288 s | 312, 2 | as above |
| `afni-tshift --implementation replica`, standard | 453 s | 277 s | 295 s | 312, 2 | as above |
| `mri-hmc`, standard | 853 s | 884 s | 952 s | 0, 0 | as above |
| `ants-registration-corrections`, smoke | | 113 s | 87 s, 104 s | 190, 250 | 9.2 |

- **The gain** is the originals' share: on the smoke tier the warm run served 2141 runs that had
  taken 5326 s when they ran (in the parallel cold run), and `pytest tests/parity`, which
  `record_golden.py` and the merge checklist run, went from 94 to 27 minutes. What is left warm
  is mostly larmorx: its GPL replica of 3dUnifize is as slow as AFNI (30 to 70 s a case), and
  the registration's warm run is larmorx's registrations plus ANTs' thread band at 2, 4 and 8
  threads, which is never cached (108 runs). The cold run costs what a run without the cache
  costs (7197 s and 7200 s for the registration): the hashing and storing are negligible.
- **Across suites:** the replica suites run AFNI with the same arguments as the original
  suites, so `afni-tshift --implementation replica` found 312 of its 314 AFNI runs in a cold
  cache that only the original suite had filled.
- **`mri-hmc`** runs no original program (it replays mcflirt's runs, recorded once in
  `oracles/fsl-6.0.7/mcflirt`): no gain, by construction. It exits with status 1 with and
  without the cache: its one failing case (`real/ds003345/default`, FD correlation) is the
  record's open case.
- **What cannot be served:** the corrections record's ANTS-72 references draw from the clock in
  every run, and about 120 later runs apply the transforms they give: their inputs change in
  every run, so they are new keys every time.

**The proof** (`same_records.py`; the cache's line, the timings, memory addresses, temporary
paths and the date are masked, as they change between any two runs, cached or not):

- Standard tier: `afni-tshift`, its replica, `ants-image-math` and `mri-hmc` are byte-identical
  warm and without the cache; `ants-registration` differs in one line, the thread band's summary
  (antsRegistration above one thread, never cached), which differs as much between the run
  without the cache and the cold run.
- Smoke tier: 17 of the 23 records and the 3 replica records are byte-identical; the other six
  (`ants-ai`, `ants-atropos`, `ants-denoise`, `ants-registration`,
  `ants-registration-corrections`, `ants-resample-image`) differ only in lines that report runs
  the cache never serves (the thread bands of antsAI, antsRegistration and DenoiseImage, Atropos'
  clock-seeded runs, the ANTS-72 references, a crash that prints uninitialised memory), and the
  same six differ the same way between the run without the cache and the cold run.
- `oracle-cache verify`: 160 entries of the workspace's cache run again (three samples), 155 the
  same at once; the five others were false alarms of the comparison or of `verify`'s rebuilding,
  fixed (below), then the same.

**What the measurements found in the cache itself** (fixed, each with a test, before the second
measurement): AFNI's key held Intel's `KMP_*` variables, which libraries the harness imports set
in its own process, so under pytest no AFNI run was found (AFNI's runtime is libgomp, which
never reads them); a file a run wrote over one of its inputs was stored under its name, not its
input's role (two disputes in the corrections suite, whose derived files were named by Python's
`hash()`, different in every process: named stably since); antsRegistration's `DIAGNOSTIC`
lines hold iteration times the comparison did not mask (four false alarms of `verify`, resolved
with `--check`); `verify` rebuilt inputs named relative to the working directory, or by a path
with `..`, as plain absolute paths. And one about a record: Atropos' clock-seeded cases run ANTs
twice, and two runs that start in the same second get the same seed from ITK's clock, so
"ANTs run 1 vs run 2" is sometimes "identical" (never cached; a follow-up for the suite).

## Limits

- An input a program reads without its path in the arguments (a file named inside another
  file) counts by the path written there, not its content, unless the suite names it
  (`inputs=`) or a printf pattern in the arguments names it; `verify` catches a change.
- A path's text counts only where `policy.path_text` says a program reads it; a blank above the
  folder that holds an input (`/tmp/a b/x/in.nii`) is not made again by `verify`.
- The cache grows: blobs are shared, nothing is deleted (it is regenerable; archive it rather
  than delete it, CLAUDE.md).
- The cache runs on Linux, where the originals are.
- BET's suite keeps the inputs FSL derives (bet2's meshes and FLIRT's matrices of the real
  images) in `<workspace>/oracles/inputs/fsl-bet/` (since 2026-10-25), outside the cache's
  directory, as every suite keeps its inputs.
