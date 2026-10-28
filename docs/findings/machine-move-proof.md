# The move to the new main machine: results do not depend on it

The proof of the move's phase 3 ([machine-move.md](../machine-move.md)), run 2026-10-28 at commit
`faa4f31`. The new main machine: AMD (Alienware m18 R1), 32 threads, 124 GiB, AVX-512. The old
laptop: Intel i7-8750H, 12 threads, 31 GiB, AVX2 and FMA. Both run Ubuntu 22.04.4 with glibc
2.35-0ubuntu3.15 (`libm`, `libc`, `libstdc++`, `libgomp` identical by checksum), Rust 1.99.0,
Apptainer 1.4.5 and the same oracle builds (SHA-256 against `MOVE-MANIFEST.json`); Python
3.12.16 here against 3.12.10 there (a patch release: no difference below traces to it).

**Verdict.** Neither larmorx nor any original program computes differently on the new machine.
The one machine-dependent computation is numpy's: its OpenBLAS picks other kernels on AVX-512
(below, and [numpy-blas.md](numpy-blas.md)), so the validation now pins numpy to the Haswell
kernels larmorx reproduces. Every other difference is a test that depends on the machine's
memory, a value measured afresh from an original's clock-seeded runs, or a gap in the oracle
cache's replay, each explained below. *Verified*.

## Build, gates and golden checksums

The three Rust workspaces (`fmt`, `clippy -D warnings`, `test --locked`), `ruff`, the SPDX check,
`pytest tests/python tests/golden validation/tests`: all pass; the golden checksums are
unchanged (log `oracles/logs/move-build-gates.log`).

## The originals: the oracle cache re-run

The cache's key includes the CPU, so the old laptop's entries are never hits here.
`oracle-cache verify` re-ran its 4,953 entries of at most 300 s on the new machine (seed
20261028): 4,541 identical, 89 different, 323 skipped (programs that were in agents' scratch
directories). The 89 were then re-run on the old laptop itself: 88 differ there as well, and the
last varies on both machines (in 6 of 8 runs there, 7 of 8 here). A sample of 17 of the 89
entries longer than 300 s: all identical. So no original program computes differently on the
new CPU. The 89 that do not replay identically on any machine:

| Entries | Why the replay differs from the stored run |
|---|---|
| 38 | Apptainer's warning about `HOME`, which depends on the replay's environment |
| 22 | Atropos priors named by a `%02d` pattern: the replay does not restore the inputs the pattern names |
| 6 | AFNI's history note, which holds the command line: its length follows the replay's paths |
| 3 | an input path with a blank in it |
| 1 | mcflirt's `out.mat+` folder name |
| 1 | BET's crash message, which names the process ID |
| 15 | random-testing runs of antsRegistration with `Random` sampling (clock-seeded: they vary on their own machine) |
| 3 | random-testing runs of ImageMath, MultiplyImages and ResampleImageBySpacing that vary on their own machine |

Logs: `oracles/logs/move-cache-verify.log`, `move-cache-differ-oldlaptop.log`,
`move-cache-verify-long.log`.

## larmorx: every record's standard tier, and both machines at the same commit

On the new machine: the smoke tier (`pytest tests/parity`), every suite's standard tier, the
AFNI and FSL replicas, mcflirt's full tier. Against the committed records no status changed except
numpy-linalg's (below). The records were made at older commits, so their details mix code changes
with the machine; to separate the two, the old laptop ran 22 of the suites at the same commit
`faa4f31` (2,012 cases in common). Statuses: identical in every case but numpy-linalg's 28.
Details: 20 cases differ, all explained:

| Cases | What differs | Why |
|---|---|---|
| numpy-linalg, all 28 | skipped here | numpy's OpenBLAS runs its SkylakeX kernels on AVX-512 and the suite skips any kernel larmorx does not reproduce. With `OPENBLAS_CORETYPE=Haswell` every case is identical to the record |
| resample-series 5, nifti-io 5 | the last bits of nitransforms' mapped points and resampled data (2.4e-17, 2.9e-14), nibabel's qform (1.6e-16 against 0) | the same kernels: under SkylakeX numpy's 4×4 inverses differ from Haswell's for most matrices (measured, [numpy-blas.md](numpy-blas.md)). With `OPENBLAS_CORETYPE=Haswell` all 10 are identical to the old laptop's |
| ants-registration-corrections 4, ants-ai 2, ants-atropos 2 | ANTs' own runs and the distances to them | references drawn from a generator seeded by the clock (ANTS-72, Atropos's clock seed, antsAI's `Random` sampling), measured afresh at every run, never cached |
| itk-hdf5 1 | a report-only count of differing bytes (23,777 against 23,774) | ITK's file stores its write time in each object header; its compressed chunks are identical on both machines. Where larmorx's own deflate shifts the layout, the count depends on that time |
| ants-image-math 1 | `MD` with radius -1: ANTs killed by signal 9 here, by signal 6 (`std::bad_alloc`) there; both-error on both | ANTs asks for an impossible amount of memory: refused on the old laptop, granted here and the process killed by its memory box (next section) |

Thread counts: "all threads" means 32 here against 12 there; the thread-pool tests pass, and no
result depends on the count. Logs: `oracles/logs/move-proof.log`, `move-proof2.log`, and the old
laptop's `oracles/logs/move-proof-oldlaptop.log`.

## Tests that depend on the machine or fail at random

- **`difftest/allocation-refused`** of ResampleImage and ResampleImageBySpacing: the case expects
  ANTs to fail to allocate 117 to 119 GB. Linux refuses such an allocation only above RAM plus
  swap: on the old laptop (31 + 63 GiB) ANTs fails as recorded, here (124 + 199 GiB) the memory
  is granted, the process grows until its memory box kills it, and the smoke tier's whole box
  went with it once. ImageMath's `MD` with radius -1 is the same mechanism. To fix: run these
  originals under a fixed address-space limit (`RLIMIT_AS`), so they fail alike on every
  machine.
- **`bet2/one-argument`** fails at random on any machine: the suite's temporary folder
  `lx-fsl-gen-<random>` sometimes contains `-h`, and `bet2` then prints its help
  ([fsl-bet.md](fsl-bet.md)), while the oracle cache serves FSL's run keyed by content. To fix: a
  prefix without a dash run (`lx-fsl-gen_`), and a cache key that keeps a path's text where the
  program reads it.
- **3dAutomask's clean-room original** fails 5 cases added after its record (fMRIPrep's calls
  on the data panel's runs) on both machines: AFNI's obliquity message prints `20.646870` degrees,
  the clean-room original `20.646889`; the masks agree. Code, not the machine: it goes to the
  clean-room fixes part as a behaviour-only description.

## Fixed (2026-10-28)

The tests that depended on the machine or failed at random, and the cache's replay gaps, fixed in
one part (commits 831c754, e682a7e, efb8536, 224bbac, 051d7ef and this document's; validation
code only, nothing larmorx computes changed):

- **Allocations refused alike on every machine.** Every original program the harness runs
  (through the oracle cache's runner: the parity suites, `verify`, random testing) runs under
  `RLIMIT_AS` = 64 GiB, inherited by what it starts; larmorx's in-process runs of the ANTs image
  programs run with the harness's soft limit lowered to it
  (`validation/src/larmorx_validation/address_space.py`, `LARMORX_ADDRESS_SPACE_LIMIT`). The
  refused requests (`strace`): ImageMath's `MD -1` 96.0 GiB, ResampleImage's and
  ResampleImageBySpacing's `allocation-refused` 109.2 and 110.9 GiB, N4's 3.2·10¹⁶ bytes; the
  largest real call, mcflirt on the 600-volume multiband-8 BOLD series, 9.4 GiB of address space
  (antsApplyTransforms on it 6.7, ImageMath 3.6, 3dTshift at 32 OpenMP threads 2.6, sMRIPrep's
  registrations 2.6, fMRIPrep's `bet` through Apptainer 1.8 GiB), every one the same result under
  the limit; the harness with larmorx in-process at most 19.3 GiB
  (`validation/investigations/address-space/`). The four cases fail as recorded here
  (*both-error*: ANTs aborts on `std::bad_alloc` or ITK's `MemoryAllocationError`, larmorx
  refuses), as on the old laptop. Random testing
  derives the limit from each case's memory budget (twice it, never below 64 GiB), so
  `difftest --heavy` runs under twice its box and a heavy case is never refused what its box
  allows.
- **`bet2/one-argument`.** BET's temporary folders are `lx-fsl-gen_<random>` and
  `lx-fsl_<random>` (random testing's inputs `lx-difftest-inputs_<random>`): mkdtemp's random part
  never follows a dash, and the suite refuses a temporary root holding `-h`. The cache key keeps
  the text a program reads in a path where a replaced path holds it (`policy.path_text`:
  whitespace for every program, `-h` and `--help` for bet2 with one argument), and only there:
  no stored entry changes key for it (bet2's one-argument entries stored its error).
- **The replay gaps**, each reproduced or never cached ([oracle-cache.md](../validation/oracle-cache.md),
  "Keyed since 2026-10-28"): Atropos' priors named by a pattern are inputs (68 entries re-keyed),
  a path with a blank is one path (14), mcflirt's working directory's files are inputs (1),
  `verify` starts a container with every variable the suite set (the 38 Apptainer runs), compares
  NIfTI-2 files without their extensions (AFNI's history note, 6) and masks a shell's process ID
  (BET's crash, 1); the cache applies ANTs' program-level rules to every ITK run, so the
  random-testing runs that varied are never cached (ANTS-82's velocity-field stages, ANTS-07's
  smaller operand, ANTS-28's smoothed thin axis: 23 entries never served again). `oracle-cache
  verify --keys` on a copy of the 89 entries: **45 identical, 44 skipped with their reason**
  (log `oracles/logs/machine-tests-verify89b.log`).
- **The smoke tier passes in full here**, without `-k "not allocation-refused"`: `pytest
  tests/parity` from the clean commit 224bbac, twice (in a 28 GiB box): 2547 passed and the 2
  expected failures of `test_memory_peak.py` both times (26 and 24 minutes), no run differing from
  its cache entry, the smoke marker written. The standard tiers of the suites the part touched
  (fsl-bet, fsl-mcflirt-replica, ants-atropos, afni-tshift, afni-automask, afni-unifize,
  ants-image-math, ants-multiply-images, ants-resample-image, ants-resample-image-by-spacing,
  ants-n4; 1,515 cases at 051d7ef): no status changed against the committed records
  (`compare_records.py`); the new cases since those records are the permanent random-testing cases
  and the panel's fMRIPrep calls, among them 3dAutomask's 5 known failures (above). The 24 entries
  the new rules stored verify identical.

## What follows

- `scripts/orchestration/env.sh` sets `OPENBLAS_CORETYPE=Haswell` (unless already set), so the
  suites that compare with numpy, nibabel and nitransforms reproduce their records on any x86-64
  machine. fMRIPrep's oracle runs use `apptainer --cleanenv`, which drops it: a new fMRIPrep run
  on an AVX-512 machine inverts its affines with the SkylakeX kernels, unlike the fourteen runs
  made on the old laptop (Haswell). Pass `APPTAINERENV_OPENBLAS_CORETYPE=Haswell` when a run must
  match them.
- The oracle cache across CPUs (next-steps): the originals computed identically on both CPUs in
  every entry that replays; the key's machine part can be narrowed to what the programs'
  dispatchers read. numpy's OpenBLAS shows that this differs per program.
