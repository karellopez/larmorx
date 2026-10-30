## Report: the replica side of 2026-10-30 (GLIBC-05, FSL-65/66, SCI-05, the parallel smoke tier, verify-merge's suites)

Branch `worktree-agent-a54c42b251eb08a4d`, 17 commits from e1fb75a to 7dd14ca; the handoff is
`handoff.md` here. No new defect IDs.

### 1. GLIBC-05 in the AFNI replica
With `reproduce` the replica's `-EPI` radius uses `larmorx_core::math::cbrtf_glibc`; by default
the correctly rounded cube root with the clean-room original's GLIBC-05 notice word for word (the
old last-bit warning gone). The `glibc-05/epi-*` cases run for the replica. `specs/cbrtf.md` §7.3:
`cbrtf-measure --glibc-identity`: 0 of 2^32 inputs differ from the glibc translation, NaN bits
included; §7.6: on every float32 voxel volume the replica's `reproduce` radius equals glibc's and
its default CORE-MATH's, GLIBC-05 never named where the radii agree, 1,358 sampled notices exact.
§7.2 corrected (as spec writer): the 2026-10-23 program started its hash from FNV's offset basis
with its last digit missing (`335d1fb5fb714c95` with §7.1's start, `c679cb92d4402e33` with the
program's). Record `afni-unifize-replica` (from fb587fa): 127 cases, 106 pass, 19 rejected by both,
2 expected, 0 failures; compare_records: only 11 new cases (GLIBC-05's 5 and AFNI-75's 6 guard
cases). Golden `3dUnifize.EPI.glibc05` and `.reproduce`; a Python test.

### 2. FSL-65 and FSL-66 in the mcflirt replica
By default the `.mat` directory, `.par` and `.rms` files opened before anything is written (an
existing file kept until written; what the step made removed on failure), an unopenable file an
error naming it and FSL-65; `-bins` below 0 refused before any work, naming FSL-66. `reproduce`:
FSL-65 in mcflirt's order (an unopenable `.par` ends after `MAT_0000` and the `.rms` files with
mcflirt's message, status 0; an unopenable `.rms` not written); FSL-66 checked where mcflirt
prepares the bins (−1 bins as SIGFPE, −2 or fewer as `bad_array_new_length`, each an error, exit
1, nothing written; −3 to −1 with a cost using no bins run as mcflirt, FSL-66 named); FSL-21 now
named in `reproduce` too. The 33 cases of §14.1 in the replica's suite against mcflirt 6.0.7.23
live: 24 pass, 9 rejected by both, the matrices mcflirt's as text wherever both run to the end. A
command-line test each; 6 golden cases (20 rows). Record `fsl-mcflirt-replica` (from 7582eae): 236
cases, 205 of 205 compared bit-identical, 31 rejected by both; compare_records: only the 33 new.

### 3. SCI-05 and the resample-series step
Cause (from SciPy's source): a NaN coordinate's filter start index overflows for orders 2 to 5, so
SciPy reads memory just before the array; the output is NaN every run, only its bits vary. SciPy
1.18.1 has it too (1, 2 and 1 varying calls of 1,296 per process; 1.15.3: 2, 7 and 7). The step's
digests count NaN values alike, and the shared releases' cases run twice (a comparison varying
between them is reported as varying). Record `resample-series` made twice from bf26b71: identical
case by case, the same step line; against 2026-10-29's, no status change, 9 comparisons of
`nearest-order2` to `-order5` changed from "changed" to "identical".

### 4. The parity smoke tier in parallel
`scripts/orchestration/parity_smoke.py`: every test file on 16 pytest-xdist workers, then the
memory-peak and thread-pool files alone (`conftest.SERIAL`); the same marker as a serial run;
`--serial` the fallback. fsl-bet's FSL-made inputs created under a lock. pytest-xdist 3.8.0 and
execnet 2.1.2 in the shared `.venv` and the lock file. **Proof** (same commit 7582eae, same build):
serial 2,600 passed in 1,574 s, parallel 2,600 passed in 951 s, every test id with the same
outcome (291 s on 16 workers, 660 s for the 107 serial tests, of which the memory peaks about 626 s;
`compare_outcomes.py`, `parallel-smoke-2026-10-30.json`). Used by `verify-merge.sh` step 4 and
`record_golden.py` (`--serial-parity`, `--parity-workers`).

### 5. verify-merge's suites side by side
`run-suites.sh`: the `--suite`s concurrently, each with its own log, STEP5 lines in the order
given, ALLDONE and exit lines unchanged; `--serial-suites` the fallback; `compare_records.py
--against DIR`. Proof on two suites: the same records (0 differences in 125 and 39 cases).

### Gates
Rust (three workspaces) at 7582eae; Python 637 passed, ruff, SPDX at 7dd14ca; `tables --check`;
the parity smoke tier passed twice at 7582eae, serially and in parallel.

### Notes from the agent
Records stamp the commit at the end of a run (no commits in that tree while a record runs);
snapshots outside the workspace use another cache (a stray 92 MB cache left in `~/.cache`); a
resample-series run at `--jobs 4` in a 10 GiB box lost a worker to the OOM killer (`--jobs 2` in 16
GiB fine); running the memory-peak file beside the parallel part would bring the smoke tier to
about 11 minutes (kept isolated, as briefed).

### At the merge (orchestrator)
The handoff integrated (the register: SCI-05, GLIBC-05, FSL-65, FSL-66 rows and sections, history;
the public defect page; next-steps; golden.md; the orchestration README; CLAUDE.md's gates, merge
step 4 and the FSL row; `specs/mcflirt.md`'s two outdated notes, behaviour only). The stray cache
moved to `<workspace>/archive/2026-10-30-stray-oracle-cache/` (not deleted); the agent's leftover log
watcher (`tail -f`) stopped.
