## FAST's clean-room original: final report (relays 1 to 5, 2026-11-03)

The last relay's report as returned to the orchestrator (subagents cannot write report files), verbatim but
for paths.

**What was built.** Over relays 1 to 4, `crates/larmorx-fsl` gained a `fast` module (Apache-2.0, written only
from `specs/fast.md`, the paper and black-box runs of FSL). It covers every option, message, exit status and
failure case. It handles one channel or any number (`-S 9` and more supported, decided by rules A-D; the
replica refuses them). It computes the covariances with its own LU and registers the priors (`-a`, `-A`, `-P`)
with FLIRT's clean-room original in the same process. FSL-96 to FSL-104 are corrected by default and
reproduced with `reproduce`. Shared with FLIRT's original: the ANALYZE reader (`bet::image::read_analyze`: the
scale field not applied; negative voxel-size products read radiologically) and `-applyxfm`'s TR (the input's,
as FSL reads it). Wired into `lx.fsl.fast(implementation="original")` (and `"auto"` without `larmorx-nc`) and
`larmorx fsl fast`.

**Record `fsl-fast`** (87 cases, from the clean commit f0dd88b): 60 within the thresholds, 25 rejected by both,
1 expected divergence (FSL-104, refused), 1 outside the thresholds (`multi/s5`: five channels give nearly
singular 5 x 5 covariances; the original against itself with one rounding changed moves further than it lands
from FSL). On the site each passing case was run again against FSL with correctly rounded maths (the CORE-MATH
preload): of the 60, 13 bit-identical to FSL, 36 bit-identical to FSL with correctly rounded maths, 10 within
FSL's own difference between the two builds, 1 open (`multi/s2-options`: 18 partial-volume values one step from
FSL, where FSL's own difference is 15). fMRIPrep's three calls (ds000005, ds000228, ds005003) are bit for bit
what FSL computes with correctly rounded maths.

**Other checks.** Golden: 7 cases (`cli.fsl.fast` through the batched `exp`, `cli.fsl.flirt.analyze`); no
existing checksum changed. Memory: fMRIPrep's call 152 MiB against FSL's 707. Threads: four programs; the test
found the outputs running on rayon's global pool (fixed, `Grid::expand_on`). `exp_in_place`: 2^32 values
against the scalar `exp`, 0 differences.

**Speed** (`docs/benchmarks/fsl-fast.md`; informal, one run each, shared machine):

| fMRIPrep's call | FSL | replica | original, 1 thread | original, 14 threads |
|---|---|---|---|---|
| ds000005 | 21.9 s | 15.3 s | 2.43 s (9x) | 0.39 s (56x) |
| ds000228 | 34.0 s | 23.3 s | 3.76 s (9x) | 0.73 s (47x) |
| ds005003 | 59.6 s | 31.3 s | 5.21 s (11x) | 0.89 s (67x) |

**Random-testing trial:** 25 minutes, 9,005 realistic cases, 8,823 agree; of the 182 differences, 153 the
suite's own image reader, 13 a NaN bias field outside the mask (fixed), 12 messages (fixed; four permanent
cases), 3 explained, 1 message open (case 1840: FSL prints `MEAN -nan`, the original `MEAN 1`; the images
agree). No new defect (FSL-136 to FSL-140 unused).

**What was run.** FSL only through the oracle cache in `use` mode (the never-cached policies re-ran by design).
Records `fsl-fast`, `fsl-bet` and `fsl-flirt` once each from f0dd88b; BET's and FLIRT's identical to main's
(168/168, 305/305). Smoke tier once (2,692 passed); `record_golden.py` reused its marker. Relay 5 (the site):
FSL with the preload 1 new run for the examples, 5 served; the classification of the 60 passing cases: FSL as
built 54 served, 6 re-run; with the preload 20 served, 40 new; four exports; boxes one at a time.

**Gates.** Relay 4: `gates-rust` (three workspaces), `gates-python --no-build`, `tables --check`. Relay 5 changed
Python only: ruff, format, `test_tables`, `tables --check`.

**The site page.** All six examples show the original beside FSL, FSL with correctly rounded maths and the
replica, verdicts decided by code; the original's agreement chart, 90 cases with both implementations, a Speed
section; stale text corrected. Build 0 problems; number check 1,316/1,316; viewer check 520/520; inventory all
20 pages after >= before; page 29.6 MB of 40 MB.

**Not done (what this kind of part leaves):** the full tier, the data panel and long random testing (final
validation phase); the spec writer's answers to the questions; the rule-D measurement of the single-channel
variance; two speed steps (batched `expf`, the MRF's `exp`); the open case 1840.

### Merge notes (the orchestrator, 2026-11-03)
- Five relay agents; each report read before the next relay was launched, with the orchestrator's items
  written into NOTES.md ("FROM THE ORCHESTRATOR"): the shared ANALYZE reader's change checked by re-made
  `fsl-bet` and `fsl-flirt` records (identical), FLIRT's `-applyxfm` TR fix, and a randomized check of the batch
  `exp` (2^32 values, 0 differences) because every platform's bits depend on it.
- Scans: 32 commits, all by the no-reply identity, no AI attribution; no shared document edited in the branch.
  One permanent random-testing case (`validation/difftest/cases/fsl-fast-original/zstd-outputs.json`) recorded
  a traceback with the checkout's absolute path in `found.detail`: replaced by `<workspace>` at the merge; the
  writer's fix is a next-steps follow-up.
- The 24 handoff operations applied (`handoff.toml` here is the applied copy: the roadmap row says "topup's
  replica under way"); register counts unchanged (313; no new entry); tables, the site's status and summary
  regenerated; `docs/status.md`, CLAUDE.md's "Next".
- Clean-room: relay 1 listed the names of `validation/investigations/`' folders once (no file opened); the
  later prompts forbade it and no relay repeated it.
- The random-testing runner turns the oracle cache off for random cases by design (random inputs never make it
  grow); relay 4 added the opt-in `LARMORX_DIFFTEST_CACHE_MODE=use` for a part's short trial. FLIRT's
  clean-room relay 5's "trial with the cache off" (reported at that merge as a broken rule) was at least in
  part this default.
- The agent's 36 scratch scripts not in the branch (its findings' tools are committed under
  `validation/investigations/cleanroom-fast/`) archived in
  `<workspace>/archive/2026-11-03-cleanroom-fast-scratch-scripts/`.
