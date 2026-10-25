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
| `python -m larmorx_validation oracle-cache verify --sample N` | re-runs N entries chosen at random, away from their suites, and checks they give the same result: the drift detector (a changed oracle changes the key; this catches what the key misses). `--program`, `--max-seconds` (only entries whose run took at most that long: a quick check), `--jobs`, `--seed` |
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
| The arguments | as given, with every path replaced: a file that exists is an **input**, `{in0}`, `{in1}`, ...; a directory the run writes in is a **place**, `{cwd}`, `{out0}`, `{d0}`, ..., with the path below it kept | the inputs count by their content, never by their directory or name; the outputs by their role |
| The inputs | each input's content (for `.gz` files the decompressed stream: the gzip header's time and the compression level do not count), its suffix (`.nii.gz`, `.mat`, `+orig.HEAD`: programs choose a reader by it), the files read beside it (`.img` beside `.hdr`, `.BRIK` beside `.HEAD`), and the shape of a path not written normally (`a/sub/../x`, which the program prints) | the result depends on the bytes a program reads |
| The directories | the directories that exist inside the places along the arguments' paths | writing into a missing directory is not writing into one that exists |
| The environment | the variables the program's family reads: ITK and ANTs (`ITK_*`, `ANTS_*`, `NSLOTS`, the variables `ITK_NUMBER_OF_THREADS_ENV_LIST` names), AFNI (`AFNI_*`, `NIML_*`, `OMP_*`, `GOMP_*`: its OpenMP runtime is GNU libgomp; `HOME` by its contents), FSL (`FSL*`, `OMP_*`); for every program `LD_PRELOAD` (by its files' content: the CORE-MATH preload), `LD_LIBRARY_PATH`, `GLIBC_TUNABLES`, `MALLOC_*`, the locale (`LANG`, `LC_*`) and `TZ` | thread counts, seeds, the maths library and the program's own settings change results |
| The machine | the architecture, the CPU's model and flags (glibc picks its maths kernels by them); the number of CPUs when the run sets no thread count | the same binary on another CPU may compute differently |

Left out on purpose: the larmorx commit (the original does not depend on it), directory and
file names, `PATH`, the date, the variables of other families (the user's `FSLDIR` is not part
of an ANTs run's key), and in `HOME` the files AFNI writes and never reads (`.afni.crashlog`,
`.afni.log`). So the orchestrator's re-run of an agent's record, in another checkout, finds the
agent's runs; the replica suites of 3dTshift, 3dAutomask and 3dUnifize find the original
suites' runs of AFNI.

## What is never cached, and why

The rules, applied in this order (`policy.py`; each has a test):

| Rule | Where it is decided | Why |
|---|---|---|
| A case marked so: its `nondeterministic` text, or its description or expected divergence names a defect whose original result varies (`VARYING_DEFECTS`: ANTS-28, -42, -58, -63, -64, -66, -72, -77, -82, -103, -107, ITK-34, AFNI-20, -45, -47) | each suite's `case_never` | the clock, memory the source never defines, a race, a random identifier |
| Values that depend on memory the original never wrote (`AntsCase.undefined_values`, Atropos' `undefined`, AFNI's `Scenario.undefined`) | `case_never` | not determined by the source (D13) |
| A clock seed: `antsRegistration` without `ANTS_RANDOM_SEED` or `--random-seed`; `antsAI` with `Random` sampling or `--random-seed 0` (ANTS-58); `Atropos` without `--use-random-seed 0`; the cases that run ANTs twice to measure its own spread (`expect="random"`) | `policy.ants_seed_never`, `case_never` | ANTs' generator is seeded from the clock |
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
antsRegistration's `DIAGNOSTIC` lines), the set of files written, and every file's bytes (`.gz`
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
  file) counts by the path written there, not its content; `verify` catches a change.
- The cache grows: blobs are shared, nothing is deleted (it is regenerable; archive it rather
  than delete it, CLAUDE.md).
- The cache runs on Linux, where the originals are.
- Another agent's FSL suite keeps its own inputs in `<workspace>/oracles/cache/fsl-bet-inputs`
  (since 2026-10-24), beside the cache's files; the cache reads only its own (`FORMAT`, `blobs/`,
  `entries/`, ...).
