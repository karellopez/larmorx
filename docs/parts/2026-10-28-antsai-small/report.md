## antsAI and antsAffineInitializer: memory and speed, done. Not a single result bit changed.

Worktree branch `agent-a876beb406db40b5c`, 17 commits from `e50fcf9` to `4e286e4`. Not pushed, not merged; the tree is clean. Handoff: `<scratch>/antsai-small/HANDOFF.md`. Notes: `<scratch>/antsai-small/NOTES.md`.

**Outcome:**
- **Memory:** at or below ANTs on every input; the two strict xfails are removed.
- **Speed:** antsAI is about twice as fast as ANTs at one thread. antsAffineInitializer in `reproduce` at one thread matches ANTs on the 7 T but is still 1.2× slower on the head (gap explained below). Everything else is faster than ANTs.
- **Bit-identity:** shown by comparing the old and new builds case by case, by the re-made records, by unchanged golden checksums and by the smoke tier.

### Memory
Peak at one thread; the rule is at most 1.10 × ANTs + 8 MiB.

| Call | ANTs | larmorx before | larmorx now |
|---|---|---|---|
| antsAI, the memory test's head | 92–96 MiB | 126 MiB | 95 MiB |
| antsAffineInitializer, the head | 89 MiB | 109 MiB | 91 MiB |
| antsAffineInitializer, 7 T MP2RAGE + MNI 1 mm | 688 MiB | 844 MiB (old laptop) | 676 MiB |
| antsAI, same 7 T pair | 754 MiB | not measured | 717 MiB |

- **What changed:** the metric now holds the images as read, in float (`MetricImage::Float` in the registration crate's metric module). antsRegistration still uses `Real`, so its arithmetic is unchanged. The moments, the centred initializer (new `initialize_pixels`) and the search all read the float images, so the double copies are gone.
- **Why the bits stayed:** each float value is read as a double exactly, which is what the old double copies held; every operation after that is the same.
- **Memory test:** `test_memory_peak.py` now passes for both programs (antsAI 96 vs 91 MiB, antsAffineInitializer 90 vs 90).

### Speed
**ANTs really runs on one thread.** It uses 99 % of one CPU, with 3 voluntary context switches in 35 s on the head and 59 in 252 s on the 7 T. Neither program sets a thread count; ITK takes it from `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`.

**Profile.** `perf` is blocked for users here (`perf_event_paranoid` is 4), and py-spy cannot see the rayon threads. I wrote a gdb stack sampler (`gdb_profile.py`) and added timing to each metric evaluation. Nearly all the time is the Mattes loop over the sampled points: about half in interpolation, including ITK's index rounding, which took five dependent operations. A search makes about 2,200 value-only and 300 derivative evaluations of 157k points.

**Steps** (antsAffineInitializer, head, `reproduce`, one thread; ANTs takes 35 s). Each step kept the same operations in the same order.

| Step | Time |
|---|---|
| Before | 133 s |
| Images in float | 116 s |
| Sampled path: each point's fixed bin computed once (ITK recomputes the same value every time), points added to the histogram in their original order | 66 s |
| Batches of 64 points processed stage by stage; corners read first; gradient computed in the value's cell, all three components in one pass | 48 s |
| ITK's rounding done in two operations instead of five (`round_to_i64`, tested equal to the old method on 600k values) | (included above) |
| Inlining, no `memcpy` | 42 s |
| ITK-21 check made cheaper | 41 s |
| Fused value-only accumulation | 40 s |

The ITK-21 check works like this:
- At one thread, the second search reuses the first search's evaluations wherever the two ask the same question: the whole first start point and each later start point's first evaluation. They are matched by the exact bits of the parameters.
- At more threads, the two searches run at the same time in the call's thread pool.
- The notice text is unchanged in every case.
- Stopping the second search where the two agree to the bit cannot help: once the learning rates differ, no later evaluation repeats.

**Final numbers** (median of 3, `docs/benchmarks/ants-ai.md`; shared machine, so timings are informal):

| Call | ANTs, 1 thread | larmorx `reproduce`, 1 | larmorx default, 1 | `reproduce`, 8 | default, 8 |
|---|---|---|---|---|---|
| antsAffineInitializer, head | 33.5 s | 40.0 s | 20.9 s | 8.7 s | 4.8 s |
| antsAI, head | 31.2 s | 17.5 s | 17.8 s | 5.0 s | 4.8 s |
| antsAffineInitializer, 7 T | 252 s | 254.5 s | 124.7 s | 51.6 s | 26.1 s |
| antsAI, 7 T | 156 s | 79.0 s | 79.9 s | 21.8 s | 20.9 s |

**Remaining gap:** antsAffineInitializer in `reproduce` at one thread is 1.2× ANTs on the head.
- One larmorx search now takes about 0.6 of ANTs' time: a value-only evaluation costs about 43 ns per point, one with the derivative about 116 ns.
- The ITK-21 notice needs a second search, and only 1/16 of it can be shared. On the head that is about 20 s of search plus 19 s for the check, against ANTs' 33.5 s.
- Closing it further would need SIMD across the points of a batch. I did not attempt it.
- In the default mode (10 iterations, as the call asks, versus ANTs' 20) larmorx takes half of ANTs' time.

### Proof that nothing changed
- **Old build vs new build:** `compare_builds.py` ran all 172 standard cases of both suites, in both modes, with the new build at 1 and 4 threads. 0 differences in exit status, both output streams (every notice included) and every file. One earlier run showed a difference that turned out to be a race in the harness's input generation, not larmorx; the script now generates inputs first and retries a difference once.
- **Records:** both re-made from the clean commit `9b9fe9aa5232`, with ANTs' runs served from the oracle cache.
  - ants-ai: 132 cases, 76 pass, 52 rejected by both, 4 expected divergences, 0 failures.
  - ants-affine-initializer: 26 pass, 13 rejected by both, 2 expected, 0 failures.
  - `compare_records` found no status change. My `record_fields.py` checked every field; the only differences are timings, ANTs' own clock-seeded spread, fields the 2026-10-17 record format did not have yet, and one permanent difftest case added since.
- **Golden checksums:** unchanged.
- **Smoke tier** (`pytest tests/parity`): 2,549 passed.

### Gates
- Rust, all three workspaces: OK.
- `pytest tests/python tests/golden validation/tests`: 623 passed.
- ruff, SPDX, `tables --check`: all OK.
- No commit has AI attribution.

### For the merge (all in the handoff)
- **Generated tables already committed:** I regenerated `README.md` and `docs/validation/README.md` with `tables` because `validation/tests/test_tables.py` fails otherwise. Only the ants-ai counts change. The site's `status.yml` still needs `tables --site`.
- **Shared core function changed:** `round_to_i64` (ITK's floor and round for every tool) now uses a faster method. Results are identical; golden checksums and the smoke tier confirm it.
- **Possible follow-up:** antsRegistration's sampled Mattes stages could use the same fast path, but they would need their own records re-made.

The handoff has the exact text for:
- next-steps: two items marked done, plus a new follow-up for antsRegistration's sampled Mattes stages;
- CLAUDE.md, "Where things are": the "Next:" paragraph and the "Memory:" bullet;
- the site's antsAI "Speed" sentences;
- the decision log: no entry needed.

No new defects, and no register or golden.md changes.

### Files
Everything is under the agent's worktree.
- Report: `docs/benchmarks/ants-ai.md`
- Findings: `docs/findings/ants-ai.md`
- Code: `crates/larmorx-registration/src/metric/mattes.rs`
- Tools: `validation/investigations/ants-ai-speed/`
- Benchmark script: `validation/src/larmorx_validation/bench/ants_ai.py`, run with `python -m larmorx_validation bench ants-ai --repeats 3 --threads 1 8 --out docs/benchmarks`

Nothing the rules could not settle.
