# Final report: The oracle-output cache

Agent `a5fad26d6bd042746`, 2026-10-24 to 2026-10-25. The agent's report to the orchestrator, verbatim, followed by its two interim notes on the measurements.

The oracle-output cache is built and wired into every parity suite that runs an original program; all gates pass from the last commit, 208fa6c. A warm parity run now re-runs mostly larmorx, and its records are byte-identical to records made without the cache, except in lines from runs the cache never serves. Branch `worktree-agent-a5fad26d6bd042746` (from 5317323), 13 commits, not pushed. No Rust, site, golden or defect-register change.

**Design**
- **Code:** a new package, `validation/src/larmorx_validation/oracle_cache/`. Suites call `oracle_cache.run(argv, env=, policy=)` instead of `subprocess.run`. There is also `run_apptainer` for fMRIPrep's image.
- **Wired in:** the ANTs programs, N4 and DenoiseImage, registration and its corrections suite, antsAI and the affine initializer, Atropos, antsApplyTransforms and the transform outputs, the HDF5 suite, 3dTshift, and 3dAutomask/3dUnifize (both builds, fMRIPrep's AFNI included).
- **The key** is a SHA-256 over:
  - the program's SHA-256 and every shared library it loads (glibc's libm decides the maths); for a container, the image's SHA-256;
  - the arguments with paths replaced: existing files by their content (decompressed for `.gz`), suffix and companion files; output directories by their role;
  - the environment variables of the program's family (ITK/ANTs, AFNI or FSL), plus `LD_PRELOAD` by content, the locale and similar;
  - the CPU model and flags.
- Not in the key: the larmorx commit, file names, `PATH`.
- **Store:** `<workspace>/oracles/cache` (or `LARMORX_ORACLE_CACHE`). Content-addressed blobs, one JSON entry per run (readable key parts, result, run time, date, larmorx commit, history), atomic writes, a lock per key. A replay restores the outputs and rewrites the stored paths in the messages to this run's.
- **Never cached:**
  - **Clock seeds:** registration without a seed, antsAI's `Random` sampling or `--random-seed 0`, Atropos without `--use-random-seed 0`.
  - **Case markings:** cases marked nondeterministic, or that depend on memory the original never wrote, or whose text names a defect that varies run to run (ANTS-72, -82, -107, AFNI-20 and others).
  - **Hangs, crashes and timeouts.**
  - **Programs that vary above one thread** (antsRegistration, antsAI, antsAffineInitializer, DenoiseImage), at those thread counts.
- **Other multi-thread runs** are served only after 3 identical runs (`oracle-cache confirm` runs the missing repeats); so is a command a case runs twice, which may be measuring the original's own spread.
- **Modes:** `--oracle-cache use|refresh|off` (default `use`). `oracle-cache verify --sample N` is the drift detector; there are also `confirm`, `stats`, `disputes`, and `resolve --check` for false alarms.
- **A difference between a fresh run and its entry is a finding:** both results are kept, the entry is never served again, it is reported loudly, and `refresh` and `verify` exit 1.
- **Records** get one header line, "Original programs: N runs: X from the oracle cache, Y run …; cache …, mode …", and an `oracle_cache` field in the JSON.

**No duplicate runs:** a passing whole `pytest tests/parity` from a clean commit writes a marker in the cache. `scripts/record_golden.py` reuses it for the same commit and extension build (`--rerun-parity` overrides). The marker exists for 208fa6c, and `smoke_marker()` finds it. `verify-merge.sh` gained `--oracle-cache`.

**Proof** (records compared with the cache line, timings, memory addresses, `/tmp` paths and the "Generated" date masked; those change between any two runs):
- **Standard tier:**
  - `afni-tshift`, its replica, `ants-image-math` and `mri-hmc` are byte-identical without the cache and warm.
  - `ants-registration` differs in one line, its thread-band summary (never cached), which differs the same way between the uncached and the cold run.
- **Smoke tier:**
  - 17 of 23 records and all 3 replica records are identical.
  - The other 6 differ only in lines from runs that are never cached: thread bands, Atropos' clock-seeded runs, the ANTS-72 references, and one crash that prints uninitialised memory. The same 6 differ between the uncached and cold runs.
- **verify:** 160 workspace entries re-run. Five false alarms were fixed (below) and the affected entries then matched.

**Gain.** Measured with jobs 6 beside other agents' runs, so these are busy-machine times; the load average is given per step, and quiet-machine timings are a follow-up.

| Run | Off | Cold | Warm | Load (mean) |
|---|---|---|---|---|
| Smoke tier, all records | 2021 s | 1793 s | 694 s (2141 runs from the cache, 522 run) | 13.4 / 11.3 / 14.3 |
| `oracle-cache confirm` after cold | | 841 s | | 13.6 |
| `pytest tests/parity` | 5654 s | | 1614 s | 8.1 / 9.2 |
| `ants-registration` standard | 7197 s | 7200 s | 2958 s (509 of 657 runs from the cache) | 17.5 / 18.0 / 30.9 |
| `ants-image-math` standard | 36.8 s | 42.8 s | 21.6 s | as above |
| `afni-tshift` standard | 494 s | 512 s | 288 s | as above |
| `afni-tshift` replica standard | 453 s | 277 s | 295 s | as above |
| `mri-hmc` standard | 853 s | 884 s | 952 s | as above |
| `ants-registration-corrections` smoke | | 113 s | 87 s, then 104 s | 9.2 |

- The registration warm run (load 30.9) ran beside fMRIPrep, so its time understates the gain.
- The cold run costs the same as an uncached one.
- The replica suite reuses the original suite's AFNI runs: 312 of its 314 runs came from the cache even cold.
- `mri-hmc` runs no original program (it replays recorded mcflirt runs), so it gains nothing; its exit status 1 is its known open case `real/ds003345/default`, the same with and without the cache.
- About 120 corrections-suite runs can never be served: they apply transforms produced by clock-seeded ANTS-72 runs, so their inputs change every run.

**Bugs the measurements found, fixed with tests**
- Libraries imported by pytest set Intel's `KMP_*` variables, which were in AFNI's key, so under pytest no AFNI run was ever found. AFNI uses GNU libgomp, which never reads them, so they were dropped from the key.
- A file a run wrote over one of its inputs was stored under its old name; it is now replayed onto the current input. The corrections suite named its derived files with Python's per-process `hash()`; they now have stable names.
- antsRegistration's `DIAGNOSTIC` lines print iteration times, which the comparison did not mask (4 false alarms, resolved after a re-check).
- `verify` rebuilt relative or `..` input paths as plain absolute paths; it now keeps them as written, and a path not written normally counts in the key.

**Finding, not fixed:** Atropos' clock-seeded cases run ANTs twice. When both runs start in the same second they get the same ITK seed, so the record's "run 1 vs run 2" is sometimes "identical". This happens with or without the cache; the handoff lists it as a follow-up.

**Tests:** 59 in `validation/tests/test_oracle_cache.py`, now part of `gates-python.sh`. They cover:
- the key: stable, sensitive to each of its parts, blind to directory and file names;
- atomic writes, six concurrent processes, and `--jobs` workers through the harness;
- every never-cached rule, the three-run confirmation, repeats within a case;
- `refresh` and `verify` reporting differences, `resolve`, and the `record_golden` marker reuse.

**Gates, at 208fa6c (clean):** `gates-python.sh --no-build` exits 0 (526 passed, ruff, SPDX), and `pytest tests/parity` passes 2247 tests in 1797 s on the warm workspace cache. No Rust gates, since no Rust changed.

**For you at the merge**
- The workspace cache is filled: 2777 entries, 3.5 GiB, covering all smoke suites and the standard tiers of registration, image-math and tshift (both builds). The merge re-runs will be served. One stale disputed entry remains, harmlessly.
- An FSL agent keeps `fsl-bet-inputs` inside `oracles/cache` (since 2026-10-24 18:55). Both work, but a convention for where FSL suites keep their inputs is yours to set.

**Documentation:** `docs/validation/oracle-cache.md` (new: design, rules, runners, measurements, proof), `validation/README.md`, `scripts/orchestration/README.md`, and `validation/investigations/oracle-cache/` with its README and the measurement scripts. The shared-document text is in the handoff: CLAUDE.md (where things are, the speed bullet, the gates, a new working rule, the merge steps, the repo map), `docs/validation/README.md`, next-steps item 6 and follow-ups, a decision-log entry, and the porting guide §10.

## Interim notes on the measurements

On `mri-hmc`'s exit status 1: "yes, exit status 1, and it was 1 in std-off and std-cold too. It is the suite's own open case, not the cache. All three runs report '134 cases: 121 pass, 11 both-error, 1 expected-divergence, 1 fail'. The fail is real/ds003345/default, FD (Power, 50 mm) correlation: max |dFD| 0.0851 mm, max FD 1.353 mm. That is the record's known status: experimental, 1 of 122 compared cases misses its threshold, as in docs/validation/mri-hmc.md. None of its runs came from the cache because the suite runs no original program: it replays mcflirt runs recorded once in oracles/fsl-6.0.7/mcflirt. Evidence the cache changed nothing: the mri-hmc records from std-off and std-warm are byte-identical apart from the cache line, the timings and the 'Generated' date. Same for std-off vs std-cold. For the other standard suites: afni-tshift, afni-tshift replica and ants-image-math are identical too. ants-registration differs in exactly one line, the thread-band summary (antsRegistration above one thread, never cached), and off vs cold differ in that same line."

The orchestrator checked the `KMP_*` fix: the AFNI oracle's 3dTshift and 3dAutomask link no OpenMP runtime, and 3dUnifize links GNU `libgomp.so.1`, which reads only `OMP_*` and `GOMP_*` variables.
