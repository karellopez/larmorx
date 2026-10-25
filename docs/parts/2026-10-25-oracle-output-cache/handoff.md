# HANDOFF: the oracle-output cache (shared-document text for the orchestrator)

Part: the evidence infrastructure, first item: the oracle-output cache (decision log 2026-10-23,
"speed at the same quality", points 1 and 2). Branch: the worktree's
(`worktree-agent-a5fad26d6bd042746`, from main 5317323). No Rust change, no site change, no defect
IDs used, no golden checksum changed.

Every section gives the exact text to integrate. Numbers come from
`<scratch>/measure/` (commits 090cfc6 and 3e6ac7f, clean; `validation/investigations/oracle-cache/measure.sh`, `remeasure.sh`).

---------------------------------------------------------------------------------------------

## 1. `CLAUDE.md`

### 1a. "Where things are": after the paragraph "**The clean-room originals of 3dAutomask and 3dUnifize merged** ...", add

```
**The oracle-output cache merged** (2026-10-24; the evidence infrastructure's first item): every
run of an original program the parity suites make goes through a content-addressed cache
(`<workspace>/oracles/cache`, `LARMORX_ORACLE_CACHE`; `validation/src/larmorx_validation/oracle_cache/`,
[docs/validation/oracle-cache.md](docs/validation/oracle-cache.md)), so re-running a record runs
only larmorx; runs the original varies (the clock, races, thread bands of programs that vary,
memory it never wrote) always run, and a run above one thread is served once three runs agreed.
`--oracle-cache use|refresh|off`; `python -m larmorx_validation oracle-cache verify --sample N`
re-runs entries to catch drift. Each record's header names its runs from the cache. The parity
smoke tier leaves a marker that `scripts/record_golden.py` reuses (no second smoke run).
```

### 1b. "Where things are", the **Next:** line: replace "**the evidence infrastructure** (decision log 2026-10-23): the data panel, ..." by

```
**the evidence infrastructure** (decision log 2026-10-23), after the oracle-output cache: the data panel, differential random testing, generated tables, applied to every finished tool.
```

(and drop "the clean-room originals of 3dAutomask and 3dUnifize (from their specs, ...), and" from
that line: merged on 2026-10-23, as the paragraph above it says.)

### 1c. "The user's direction", the bullet **Speed at the same quality**: replace its first sentence by

```
- **Speed at the same quality** (user, 2026-10-23): oracle outputs are cached by content and never computed twice (the oracle-output cache, since 2026-10-24: verification re-runs only larmorx; variation is still measured fresh), no duplicate runs (the golden recording reuses the parity smoke result through its marker; the long tier once, at the end), small questions decided in batches.
```

### 1d. "Agent working rules", **Gates before the final commits**: replace `pytest tests/python tests/golden` by

```
`pytest tests/python tests/golden validation/tests`
```

and **Records only from a clean commit**: replace "(it runs the parity smoke tier first, about 10 minutes)" by

```
(it runs the parity smoke tier first, about 10 minutes, unless that tier passed for the same clean commit and build: then it reuses its marker)
```

and add a bullet after **Records only from a clean commit**:

```
- **The oracle-output cache** ([docs/validation/oracle-cache.md](docs/validation/oracle-cache.md)): new suites run original programs through `oracle_cache.run` (never `subprocess.run`), with a `Policy` naming the thread count, the family of environment variables and, from the case, why a run must never be cached (`case_never`: a clock seed, memory never written, a defect of `VARYING_DEFECTS`, a case that measures the original's own spread). A difference between a fresh run and its entry is a finding (`<cache>/findings.jsonl`), never overwritten.
```

### 1e. "Merging an agent's work", step 2: replace `pytest tests/python tests/golden` by `pytest tests/python tests/golden validation/tests`; step 5: append

```
The re-run serves the agent's runs of the originals from the oracle-output cache (the record's header says how many); `--oracle-cache refresh` re-runs and compares them.
```

### 1f. Repo map: after the row `validation/`, add

```
| `validation/src/larmorx_validation/oracle_cache/`, `validation/tests/` | the oracle-output cache (key, store, policy, replays, `verify`) and the validation package's own tests ([docs/validation/oracle-cache.md](docs/validation/oracle-cache.md)) |
```

---------------------------------------------------------------------------------------------

## 2. `docs/validation/README.md`

### 2a. The table: after the row `oracle-runs`, add

```
| [oracle-cache](oracle-cache.md) | how the parity suites run the original programs once: the key, what is never cached and why, the modes, `verify`, the measured gain | — | reference |
```

### 2b. After the paragraph "These records compare larmorx with the original tools on Linux x86-64. ...", add

```
**The original programs run once.** Every parity suite runs the originals through the
[oracle-output cache](oracle-cache.md): a run seen before (the same oracle build, inputs,
arguments and settings) is served from `<workspace>/oracles/cache`, so re-making a record runs
only larmorx; each record's header says how many original runs came from the cache. Runs whose
result the original itself varies (its clock, races, the thread bands of programs that vary,
memory it never wrote) always run. A record made from the cache is byte-identical, apart from
that line and the timings, to one made without it. Measured on a busy machine (other agents ran beside it; load average 8 to 31): the smoke tier's records took 2021 s without the cache and 694 s warm, `pytest tests/parity` 94 minutes and 27, `ants-registration`'s standard record 2.0 hours and 49 minutes (509 of its 657 original runs from the cache); details and the proof in [oracle-cache.md](oracle-cache.md).
```

### 2c. "Reproduce a record": after the code block, add

```
The commands use the oracle cache (`--oracle-cache use`, the default); `--oracle-cache off` runs
every original program, `--oracle-cache refresh` runs them and compares them with the cache.
```

---------------------------------------------------------------------------------------------

## 3. `docs/next-steps.md`

### 3a. Item 6: replace "**First, the oracle-output cache** (decision log 2026-10-23, ...): every output ... the golden recording reusing the parity smoke result. Then:" by

```
**The oracle-output cache: done** (2026-10-24; [validation/oracle-cache.md](validation/oracle-cache.md)):
every run of an original program computed once, keyed by the oracle build, inputs, arguments and
settings; the runs the original varies always run; the golden recording reuses the parity smoke
result. Measured on a busy machine (other agents ran beside it; load average 8 to 31): the smoke tier's records took 2021 s without the cache and 694 s warm, `pytest tests/parity` 94 minutes and 27, `ants-registration`'s standard record 2.0 hours and 49 minutes (509 of its 657 original runs from the cache); details and the proof in [oracle-cache.md](validation/oracle-cache.md). Then:
```

### 3b. "Follow-ups, when they fit": add

```
- **The oracle cache, further:** `pipeline-stages` still keeps its own runs (`oracles/compare`)
  and could use the cache; mcflirt's variability band (`parity/mri_hmc_band.py`) and the recorded
  mcflirt runs could go through it; `oracle-cache confirm --jobs 6` overnight serves the thread
  bands of N4 and the AFNI OpenMP runs from their third agreeing run; `oracle-cache verify
  --sample 50` at each merge or weekly; an archive policy when the cache grows (it is
  regenerable; archive, do not delete).
- **Atropos' clock-seeded cases** (`ants-atropos`, `expect="random"`): the two ANTs runs that
  measure ANTs' own spread can start within the same second and then get the same seed from
  ITK's clock, so the record's "ANTs run 1 vs run 2" is sometimes "identical" (seen 2026-10-24 in
  runs with and without the cache; never cached). Wait a second between the two runs, or check
  that their outputs differ.
- **Benchmarks of the oracle cache on a quiet machine** (the measurements of 2026-10-24 ran beside
  other agents, load average 8 to 31).
```

---------------------------------------------------------------------------------------------

## 4. `docs/decision-log.md`: a new entry after "2026-10-24: ANTS-40 and ITK-06 measured (rule D)"

```
## 2026-10-24: The oracle-output cache, how it decides

Built from the decision of 2026-10-23 ("speed at the same quality", points 1 and 2), these
choices made while building it ([validation/oracle-cache.md](validation/oracle-cache.md)):
- **The key** is the program's SHA-256 and its shared libraries' (glibc's `libm` decides the
  unmodified programs' maths), or a container image's; the arguments with inputs named by their
  content (a `.gz` input by its decompressed stream) and suffix and the run's directories by
  their roles; the environment variables of the program's family; the CPU's model and flags.
  Not the larmorx commit, file names or `PATH`.
- **Runs above one thread** are served only once three runs agreed (the porting guide's rule for
  races), the programs known to vary there (`antsRegistration`, `antsAI`,
  `antsAffineInitializer`, `DenoiseImage`) never; clock seeds, memory never written, hangs,
  crashes and the cases that measure the original's own spread always run.
- **A difference** between a fresh run and its entry is a finding: both kept, the entry never
  served again, reported loudly (`refresh` and `verify` fail).
- **No duplicate smoke runs:** a passing `pytest tests/parity` of a clean commit leaves a marker
  that `scripts/record_golden.py` reuses for the same commit and build.
```

---------------------------------------------------------------------------------------------

## 5. `docs/porting-guide.md`, §10 "Working safely": add a bullet after **Records only from a clean commit.**

```
- **Run the originals through the oracle cache** ([validation/oracle-cache.md](validation/oracle-cache.md)):
  a suite calls `oracle_cache.run` with a `Policy` (thread count, environment family, `never`
  from its case), never `subprocess.run`, so a record re-made at a merge runs only larmorx. Mark
  what the original varies (a clock seed, memory it never wrote, a case that runs a command twice
  to measure its spread: `nondeterministic=` or a defect of `VARYING_DEFECTS`); otherwise the
  cache freezes one run of it. A run that differs from its entry is a finding, kept in the
  cache's `disputes/`.
```

---------------------------------------------------------------------------------------------

## 6. For the orchestrator (no document text)

- **The workspace's cache is filled**: `<workspace>/oracles/cache` holds 2777 entries (3.5 GiB of
  blobs; the original runs took 5.0 h) from the measurements: every smoke-tier suite, and the
  standard tier of `ants-registration`, `ants-image-math`, `afni-tshift` (both implementations).
  So re-running those records at the merge serves their original runs (`verify-merge.sh
  --suite ...`). Left out of it: the entries made before the fixes of 2026-10-25 that those fixes
  concern (`merge_caches.py`). One entry is disputed (an AFNI run whose input path had `..`, keyed
  before the key learnt that spelling; never found again); `oracle-cache disputes` lists it.
- **Another agent's directory in the cache's place:** an FSL agent keeps its suite's inputs in
  `<workspace>/oracles/cache/fsl-bet-inputs` (since 2026-10-24 18:55). The cache reads only its own
  files, so both work; a convention for the FSL suites (through `oracle_cache.run`, or their own
  directory elsewhere) is the orchestrator's to set.
- **Gates:** no Rust change (no Rust gates needed). `gates-python.sh --no-build` and
  `pytest tests/parity` from the final commit: see the report.
- **No golden checksum, no site page, no defect ID** in this part.
