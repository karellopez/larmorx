## FLIRT replica part 3: final report

Everything is committed on worktree branch `worktree-agent-a0ef3a192c69be27a` (from 04662aa, 22 commits, last one d2300bb). The tree is clean and nothing I started is still running: no memory boxes, scopes or site server.
- HANDOFF: <scratch>/HANDOFF.md
- NOTES: <scratch>/NOTES.md
- Site clone: `<scratch>/site`, branch `fsl-flirt-part3`, commit 1f5773b.

### Your two messages, done first
1. **Re-check script made safe (26ccc60).** `validation/investigations/fsl-flirt/recheck_findings.py` now:
   - refuses to run outside a memory box, and its message names `scripts/orchestration/boxed.sh`;
   - puts each spawn worker in its own leaf with the trial's per-case memory limit (taken from `run.json`, 3 GiB) and sets the matching address-space limit;
   - skips memory findings unless `--memory` is given, and then runs them one at a time;
   - uses at most 8 workers.

   Since then, every run of FSL or the replica was boxed, with all boxes together at most 80 GiB.
2. **Realistic cases only.** The extreme findings are triaged as deferred to the final validation phase, with status `open` (a status `triage.toml` already uses). No further trial was run. One boxed re-check had already included the 4 extreme non-memory findings once, before your message arrived.
3. **Robustness cases (68ad3fd).** One nonsense-header case each for applyxfm4D (metres), midtrans (template with 50 mm voxels), pointflirt (`--vox` with 0.01 and 50 mm voxels) and pairreg (metres). All four finished inside `boxed.sh 4` and are identical to FSL, so none was dropped. Adding them showed that the suite and the descriptor had passed midtrans' `--template` and `--separate` as two words; FSL takes them only as `--opt=value`. That is fixed in both, so the template path is now really tested and identical. I did not touch `gen.py`, `core.py`, `runner.py` or `cli.py`.

### Built, per task
| Task | Status | Where | Suite cases |
|---|---|---|---|
| 1. alignpaxes | done | spec §8.7 | 5 |
| 2. Every parameter-subset mask | done | spec §8.3 | 3 |
| 3. midtrans, pointflirt | done, with CLI, `lx.fsl` wrapper and golden case each | spec §15.5, §15.6 | 26 |
| 4. applyxfm4D | done: every type (double included), every interpolation and option | spec §15.7 | 20 |
| 5. `-debug` volumes, BBR's included | done | spec §9.4 | 3 |
| 6. mcflirt FSL-75 | done: named in both modes, whole border by default | — | 1 |
| 7. makerot NaN signs | done | spec §15.4 | — |
| 8. Scripts pairreg, extracttxt, aff2rigid | done; `-warp` refusal kept | spec §15.8 | 17 |
| 9. Site page | done | — | — |

- **Task 1–2:** FSL's own LAPACK is translated into `larmorx-nc-core` (`dsyevd`, `dgesdd` on OpenBLAS's Haswell kernels). A probe compared every output word with FSL's `liblapack.so.3`: 400 records, all identical.
- **Task 7:** the 3 part-2 probe calls with NaN or infinite input are now identical; part 2's makerot probe gives 50 of 50, the new NaN probe 10 of 10.
- **Task 8:** the extracttxt probe gives 390 of 390 identical; aff2rigid with FSL's script repaired gives 22 of 22 identical.
- **Spec §14** now lists only what is left by decision: `-warp`, the NaN signs below, and `standard_space_roi` / `flirt_average` / `epi_reg`.

### New defects (FSL-78 to FSL-81; FSL-82 to 89 and PIPE-18 to 22 unused)
- **FSL-78**, rule B: midtrans exits 0 after an error. Now exit 1 by default.
- **FSL-79**, rule A: applyxfm4D's spline on integer images keeps coefficients and values in the integer type.
- **FSL-80**, rule A: applyxfm4D `-3D` sinc gets FSL's cached kernel instead of the one it asks for.
- **FSL-81**, rule A: aff2rigid crashes with `NameError` under NumPy 2. The matrix is computed by default.
- Kept and documented (rule C): pointflirt reads a 3-point file with points as columns.
- Also extended: FSL-22 and FSL-75 now cover applyxfm4D, and FSL-75 covers mcflirt.

### Records, golden, memory
- **FLIRT record (8826d3b):** 296 cases, 205 of 205 compared bit-identical, 87 rejected by both, 4 expected divergences. Its header names every program. Part 2's 217 cases are unchanged in status and details (compare_records); 79 cases are new.
- **mcflirt record (bc119ff):** 237 cases. The 236 old ones are identical; the new `edge/fsl75-narrow` is rejected by both (FSL aborts with a double free).
- **BET** (re-run because the core changed): 178 of 178 identical to its record. Not re-committed.
- **Golden:** `golden.json` (222e226) changed only in its header; no value moved. larmorx-nc golden (d2300bb) now has 65 cases and 313 outputs (+6 cases, +8 outputs).
- **Memory:** applyxfm4D on fMRIPrep's 63-volume BOLD uses 85 MiB against FSL's 128 MiB (trilinear) and 166 MiB (spline), with identical outputs.
- **Thread pools:** no new parallel paths.

### Random testing (one trial, as asked)
- Pre-check: 2,805 cases.
- Trial: half an hour, 209,657 cases; 209,455 agree, 136 notices, 19 differ, 47 over the memory limit.
- **Fixed, each with a suite or permanent case (5 promoted), re-checked boxed with the final build:**
  - applyxfm4D's double template;
  - FSL-79 made exact;
  - FSL-22 in applyxfm4D;
  - FSL's exceptions thrown inside computations: a singular matrix in a cost evaluation, and "Out of memory". A new `exception` module in the core unwinds them to FLIRT's `main`;
  - extracttxt's dc and sed messages;
  - the notices, now made only where the written output changes. 131 of 136 now agree; the other 5 are FSL-79 or FSL-80 cancelling FSL-09 in the same voxel, and stay as notices by design.
- **Left open, 7 findings:** the sign of NaN outputs when inputs have NaN or infinite voxels in reproduce mode. This is 3 all-NaN matrices (the path exists since part 2) and 3 applyxfm4D spline images, plus the pre-check's #496. Every value is NaN on both sides; only the sign bit differs.
- **Deferred to the final phase (extreme cases):** the 47 memory cases. FSL needs as much memory itself: one measured at 8,966 MiB for FSL against 8,342 MiB for the replica.

### What I ran
- Smoke tier once, from d6be1e4, boxed: 2644 passed (parallel 344 s, serial 906 s).
- Records: FLIRT once from bc119ff (296 cases: 277 from the oracle cache, 19 run). A first attempt was stopped before writing anything, to add the robustness cases. mcflirt and BET once each.
- Re-checks, all boxed: 38 runs of the trial's realistic findings, 330 of the pre-check's, 136 notice cases.

### Gates
- Rust: fmt and clippy pass in all three workspaces. Tests pass in root and crates-gpl; in crates-nc everything passed except the golden test (outputs not yet recorded at that run), and after recording that golden test passes.
- pytest (tests/python, tests/golden, validation/tests): 651 passed.
- ruff, SPDX, `fsl_source --check` and `tables --check` all pass.

### Site page (1f5773b)
- Page text updated and evidence re-exported from the new record. Cases went from 217 to 296; images unchanged (39 blobs), so no evidence-blobs push is needed.
- Build: 0 problems. Number check 125 of 125, viewer check 18 of 18. Inventory after ≥ before.
- Screenshots before and after (1440 and 390, light and dark): <scratch>/shots/
- Note: the local `larmorx_documentation` repository's `evidence-blobs` branch is at 79a1750, behind GitHub's 2a38d50. The clone fetched GitHub's.

### Not done
- The NaN-sign class above.
- The coordinate programs' `-warp`: waits for FSL's warp-field libraries.
- `standard_space_roi`, `flirt_average` and `epi_reg`: wait for FSL's other programs.
- The extreme-case and heavy findings: final validation phase.
- No speed figure was taken.
- The clean-room original is the next part.

---

**Merge note (orchestrator, 2026-11-02).** The part was interrupted at 00:08 on 2026-11-02:
its unboxed `recheck_findings.py ... all 24` (24 workers, memory findings included) filled memory
and systemd-oomd killed VS Code with the agent and the orchestrator's session (decision log
2026-11-02). Resumed by message with two instructions: make the re-check script safe (done,
26ccc60) and, after the user's decision of the same morning, re-check realistic findings only
(the extreme ones deferred to the final validation phase) and add fixed nonsense-header cases
(four, all within a 4 GiB box). At the merge FSL-81's class was corrected from (b) to (a) (a
deterministic crash, not a race). Smart testing checked: one trial (before the decision), the
smoke tier once, the FLIRT record once (277 of 296 cases from the oracle cache), every re-check
boxed.
