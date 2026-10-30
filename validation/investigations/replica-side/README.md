# The replica side of 2026-10-30: GLIBC-05, FSL-65/66, SCI-05, the parallel smoke tier

Tools and results of the replica author's part of 2026-10-30 (`docs/parts/` keeps its brief and
report): the AFNI replica's `-EPI` cube root (GLIBC-05), the mcflirt replica's FSL-65 and FSL-66,
SCI-05's cause and the resample-series step, and the parity smoke tier in parallel.

## GLIBC-05: the replica's cube root (`crates-gpl/tools/cbrtf-measure`)

The replica's 3dUnifize `-EPI` takes `larmorx_core::math::cbrtf_glibc` (larmorx's clean-room
cube root with glibc 2.35's bits) with `reproduce`, CORE-MATH's correctly rounded `cbrtf` by
default, and names GLIBC-05 as the clean-room original does (`unifize::epi_cbrtf_notice`).

```bash
cd crates-gpl && cargo build --release --locked -p cbrtf-measure
scripts/orchestration/boxed.sh 4 crates-gpl/target/release/cbrtf-measure --glibc-identity --threads 16
scripts/orchestration/boxed.sh 8 crates-gpl/target/release/cbrtf-measure --threads 16
```

| | Result (2026-10-30, logs `<workspace>/oracles/logs/replica-side-cbrtf-{identity,measure}-2026-10-30.log`) |
|---|---|
| `specs/cbrtf.md` §7.3: `cbrtf_glibc` against the earlier glibc translation, all 2^32 inputs, NaN bits included | 0 differ (16,777,214 NaN inputs among them) |
| §7.1's hash through all 2^32 results | `335d1fb5fb714c95` from §7.1's start `14695981039346656037`; `c679cb92d4402e33` from the 2026-10-23 program's start `1469598103934665603` (FNV's offset basis with its last digit missing: `floatmath-verify/glibc_reference.c`), for both functions: §7.2's number explained, the spec corrected |
| §7.6, every non-negative float32 voxel volume (2^31) | the replica's default radius CORE-MATH's on every input, its `reproduce` radius the glibc translation's on every input (so no radius, message or ball of `reproduce` differs from AFNI's); GLIBC-05 never named where the two radii agree; on a 64³ grid (every offset in reach) the notice present exactly where the ball or the printed radius differs in all 1,358 sampled cases |
| What glibc's last bit changes (unchanged from 2026-10-29) | the radius on 103,849,703 volumes, the message on 65,145,681, the ball on 1,449,010 (from 0.001 mm³: 552 and 8,794) |

## SCI-05: why SciPy's `map_coordinates` varies (`sci05_probe.py`)

```bash
<workspace>/.venv/bin/python -P sci05_probe.py --repeats 20                  # SciPy 1.15.3
<workspace>/oracles/python-newest/bin/python -P sci05_probe.py --repeats 20  # SciPy 1.18.1
```

The resample-series suite's `ndimage/nearest-order<K>` inputs, each call repeated 20 times in one
process (three processes per environment, 1,296 calls each):

| | Calls whose output varies | Where | Values |
|---|---|---|---|
| SciPy 1.15.3 (shared `.venv`) | 2, 7, 7 | orders 2 to 5 only, and only the output at the NaN coordinate (index 120) | NaN in every run; only its sign and payload vary (`7fc00000`, `ffd7ff97`, `fff7fff7`, `ffffffe8`, `7ff8000000000000`, ...) |
| SciPy 1.18.1 (`oracles/python-newest`) | 1, 2, 1 | the same | the same |

**The cause (from SciPy's source, `scipy/ndimage/src/ni_interpolation.c`, `NI_GeometricTransform`,
the same code in 1.15.2 and 1.18.1):** for a NaN coordinate the filter's first index is
`(npy_intp)floor(NaN) - order / 2`; x86-64 converts NaN to `INT64_MIN`, and for orders 2 to 5
subtracting 1 or 2 overflows (undefined in C; it wraps to near `INT64_MAX`). The border test
`start < 0 || start + order >= len` then fails (the second sum overflows too), so the coefficients
are read at the unchecked offset `stride * start`, which wraps to one or two strides before the
array: memory the call never wrote. The spline weights of a NaN coordinate are NaN, so the output
is NaN whatever was read, but when the value read is itself a NaN its sign and payload pass into
the product (`coeff *= weight`). Orders 0 and 1 subtract nothing and take the border path: they
never vary. The suites compare NaN values alike (`resample_series._bits`), so no comparison ever
saw it; only the step's digest of the raw bytes did. The step's digests now count NaN values alike
too, and the shared releases' cases run twice (a comparison whose original still varies is
reported as varying, never as changed).

The resample-series record re-made twice from the same clean commit (bf26b71, 2026-10-30): the
two records are identical case by case (`compare_records.py --against`), the step line the same;
against the record of 2026-10-29 no status changes, and 9 comparisons of `ndimage/nearest-order2`
to `-order5` read "identical" instead of "changed" (their only change was SCI-05's NaN bits; the
cases stay changed by SCI-03).

## The parity smoke tier in parallel (`compare_outcomes.py`)

`scripts/orchestration/parity_smoke.py` runs `tests/parity` on pytest-xdist workers (every file
but `conftest.SERIAL`), then the memory-peak and thread-pool files alone. The proof: the same
commit and build run serially (`--serial`) and in parallel, each part's JUnit XML kept, compared
test by test:

```bash
python -P scripts/orchestration/parity_smoke.py --serial --junit-dir <dir>/serial
python -P scripts/orchestration/parity_smoke.py --workers 16 --junit-dir <dir>/parallel
python -P validation/investigations/replica-side/compare_outcomes.py --a <dir>/serial/whole.xml \
    --b <dir>/parallel/parallel.xml <dir>/parallel/serial.xml --out comparison.json
```

**Result (2026-10-30, commit 7582eae, one build of the extension and of `larmorx-nc`;
`parallel-smoke-2026-10-30.json`):** the 2,600 tests of the smoke tier, every one with the same
outcome in both runs (2,600 passed in each; no test only in one run; skip and xfail reasons
alike). The serial run took 1,574 s (26 min 14 s); the parallel one 951 s (15 min 51 s): 291 s
for the 2,493 tests on 16 workers, then 660 s for the 107 tests that run alone, of which
`test_memory_peak.py` takes about 626 s (it runs each ANTs and FSL program and larmorx's command
under `/usr/bin/time`, twice per D13 mode). The machine was shared with the orchestrator's
verification during both runs. The serial run left the smoke tier's marker for 7582eae, and the
parallel run wrote it again (the same commit and build).

**Safe under concurrency:** each xdist worker is a process with its own temporary directories
(`tempfile.mkdtemp` in the harness, pytest's per-worker `basetemp`); the oracle-output cache writes
every file atomically and locks each key; the derived inputs that suites share across processes
are written atomically (`niworkflows_inputs`) or, for fsl-bet's FSL-made inputs (written in place
by FSL), under a lock on their directory since 2026-10-30 (`fsl_bet._derived_lock`). A pytest-xdist
worker never writes the smoke tier's marker (it sees only its own share of the results).
The whole run stays in the caller's memory box (verify-merge's).

## verify-merge's suites side by side (`scripts/orchestration/run-suites.sh`)

`verify-merge.sh` re-runs its `--suite`s side by side since 2026-10-30
(`scripts/orchestration/run-suites.sh`: each suite in the background with its own log
`<out>/<args>.log`, its record in `<out>/records/`, its `--jobs` as given, so their threads add up;
when all have ended, the `STEP5 <args> exit <status>` lines in the order given, as before; the
`ALLDONE` line unchanged; `--serial-suites` the old order). The proof (2026-10-30, commit 7582eae):
`afni-automask --implementation replica --jobs 3` and `ants-smooth-image --jobs 3`, once one after
the other (13 s) and once side by side (8 s; both served from the oracle cache), compared with
`compare_records.py <side>/records <suites> --against <serial>/records`: 125 of 125 and 39 of 39
cases, no status and no detail different, the summaries equal.

## A note on running proofs and records

Run them in a checkout inside the workspace, and change no tracked file while they run: the
harness finds the workspace as the first parent holding `oracles/` (a copy elsewhere uses
`~/.cache/larmorx-oracle-cache` and finds no recorded runs; a directory of symbolic links to the
workspace finds them under other paths than those the recorded outputs name), and a record is
stamped with the checkout's commit, and whether it is clean, when it ends.
