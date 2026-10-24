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
| `python -m larmorx_validation oracle-cache verify --sample N` | re-runs N entries chosen at random, away from their suites, and checks they give the same result: the drift detector (a changed oracle changes the key; this catches what the key misses). `--program`, `--jobs`, `--seed` |
| `... oracle-cache confirm` | runs the repetitions the runs above one thread still need before they are served (below) |
| `... oracle-cache stats`, `... disputes` | what the cache holds; the entries a run disagreed with |

Each record's header says how many original runs came from the cache and how many ran, and
where the cache is, for example

> - **Original programs:** 1406 runs: 1287 from the oracle cache, 119 run (40 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

and its JSON has the same in `oracle_cache` (with why each run that ran was not served). Nothing
else in a record depends on the cache: a record made from the cache is byte-identical, apart from
that line and the timings, to one made without it (below, "The proof").

## The key

A SHA-256 over the parts below, kept readable in every entry (`parts`).

| Part | What | Why |
|---|---|---|
| The oracle | the program's SHA-256 and the SHA-256 of every shared library it loads (`ldd`: glibc's `libm` and `libc` decide the unmodified programs' maths); for a script, every file of its build directory; for a container, the image's SHA-256, the program's name and Apptainer's version | a rebuilt oracle, an updated glibc or another image is another oracle |
| The arguments | as given, with every path replaced: a file that exists is an **input**, `{in0}`, `{in1}`, ...; a directory the run writes in is a **place**, `{cwd}`, `{out0}`, `{d0}`, ..., with the path below it kept | the inputs count by their content, never by their directory or name; the outputs by their role |
| The inputs | each input's content (for `.gz` files the decompressed stream: the gzip header's time and the compression level do not count), its suffix (`.nii.gz`, `.mat`, `+orig.HEAD`: programs choose a reader by it) and the files read beside it (`.img` beside `.hdr`, `.BRIK` beside `.HEAD`) | the result depends on the bytes a program reads |
| The directories | the directories that exist inside the places along the arguments' paths | writing into a missing directory is not writing into one that exists |
| The environment | the variables the program's family reads: ITK and ANTs (`ITK_*`, `ANTS_*`, `NSLOTS`, the variables `ITK_NUMBER_OF_THREADS_ENV_LIST` names), AFNI (`AFNI_*`, `NIML_*`, `OMP_*`, `GOMP_*`, `KMP_*`, `HOME` by its contents), FSL (`FSL*`, `OMP_*`); for every program `LD_PRELOAD` (by its files' content: the CORE-MATH preload), `LD_LIBRARY_PATH`, `GLIBC_TUNABLES`, `MALLOC_*`, the locale (`LANG`, `LC_*`) and `TZ` | thread counts, seeds, the maths library and the program's own settings change results |
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
| A command a case runs a second time | `core._run` | it runs and is compared with the first: a case that runs the same command twice measures the original's own spread |

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

The outputs are the files the run created or changed in its places (and the files named in the
arguments, which it may write); the replay copies them back into this run's places. In the
streams, every place, every input and every input's name is stored as a placeholder and
replayed with this run's: a replayed message names this run's files, as a fresh run would.

## Differences: findings, never overwritten

When a fresh run of a stored key gives another result (`refresh`, `verify`, a confirmation, two
workers), the entry is marked disputed and never served again, both results are kept
(`disputes/<key>/<stamp>.json`, their blobs), a line goes to `findings.jsonl`, and the parity
command prints it (`refresh` and `verify` exit with status 1); `pytest tests/parity` lists them
at its end. Either the original varies (a case to mark, a finding for `docs/findings/`) or the
key misses something (a fix here). What is compared: the exit status, the streams with memory
addresses and elapsed times masked, the set of files written, and every file's bytes (`.gz` files
decompressed; HDF5 files by their datasets and attributes, as ITK stamps objects with times,
ITK-09). What is served is always the stored bytes.

## No duplicate runs

`pytest tests/parity` that passes, whole, at the smoke tier, from a clean commit, writes a marker
(`<cache>/markers/parity-smoke-<commit>.json`: the commit, the time, the result, the counts, the
SHA-256 of the extension it tested). `scripts/record_golden.py` reuses it for the same commit and
build instead of running the smoke tier again (`--rerun-parity` runs it anyway), and says so in
`golden.json`.

## The proof, and the gain

MEASUREMENTS

## Limits

- An input a program reads without its path in the arguments (a file named inside another
  file) counts by the path written there, not its content; `verify` catches a change.
- The cache grows: blobs are shared, nothing is deleted (it is regenerable; archive it rather
  than delete it, CLAUDE.md).
- The cache runs on Linux, where the originals are.
