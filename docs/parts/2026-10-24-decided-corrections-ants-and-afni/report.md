# Final report: Decided corrections: ANTs and AFNI

Agent `af2255d42fc4f41d1`, 2026-10-24. The agent's report to the orchestrator, verbatim.

## Small fixes: report

All ten entries are implemented. In reproduce mode every result is still bit-identical to the original, and every gate passes.
- Worktree: `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-af2255d42fc4f41d1`, branch `worktree-agent-af2255d42fc4f41d1`, 25 commits from 30c50b0 to 441bc80. The tree is clean and no commit carries AI attribution.
- Site: `<scratch>/site`, branch `small-fixes-evidence`, squashed into one commit, f99b354, on a53145d.
- Handoff: `<scratch>/HANDOFF.md` (`<scratch>` = `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/small-fixes/`). Its section 9 and the AFNI rows of 1a are marked restricted (†).
- Nothing was pushed or merged.

### Each entry
Every corrected path has a notice wherever the default differs from the original, a `reproduce` switch and unit tests.

| Entry | Rule | What the default does now | Evidence |
|---|---|---|---|
| ANTS-17 | A | `ME r v` gives the eroded mask of `v`; `MO r 0` opens the zeros | 3 known-result cases match SciPy bit for bit; fix pass 8 cases. On fMRIPrep's segmentation (1 GM, 2 WM, 3 CSF), eroding the WM: ANTs returns 452,471 extra GM and CSF voxels as 1 |
| ANTS-22 | A | the whole part of the pad on each side | NumPy, bit for bit, origin within 1e-6 mm; fix pass 5 cases |
| ANTS-23 | A | every value is its own label | NumPy, bit for bit; fix pass 6 cases |
| ANTS-32 | A | sigma in input voxels; points beyond the input are 0 | ANTs' own output at 1 mm, and ANTs' output with its outside voxels set to 0, both bit for bit; fix pass 24 cases |
| ANTS-44 | A | the smoothed, corrected bias wherever a patch of a centre in the mask reaches | fix pass 6 cases; on a real T1w, 70,886 voxels outside the mask change |
| ANTS-116 | A | proper rotations, every sign combination of the axes, best correlation kept | identical blob pairs at correlation 1: 13/13 against ANTs' 4/13 (phantoms: 19/19 vs 5/19, 28/28 vs 14/28). Translations now exact; rotated 3D phantom 0.50 mm off (ANTs 34 mm), 2D 0.85 mm (ANTs 6.2 mm), rotated T1w 1.08 mm (ANTs: no landmark). Fix pass 18 cases |
| AFNI-41 | B | refused in the GPL replica: a fatal error naming the entry | 5 refusal cases; reproduce gives AFNI's mask bit for bit |
| AFNI-48 | B | refused in the GPL replica; `-EPI` still runs | 2 refusal cases; reproduce gives AFNI's output bit for bit |

On ANTS-44: the corrected voxels outside the mask now behave like the mask's own inner edge. They move from the input by 8.5 on average (ANTs 7.0; the inner edge 16.8). 812 of them become 0 (ANTs: 3), which the Rician correction also does to 1,299 voxels of the inner edge in both modes. This is recorded in the findings and on the page.

### The two measurements (rule D)
- **ANTS-40 → corrected by default (rule A).** I measured all 25 N4 calls of the five oracle runs; 16 include zero or negative voxels.
  - Leaving them out changes the field's shape by 0.6–36 % at the 99th percentile. ANTs' own 1-vs-6-thread variability is 2e-6 to 3e-3.
  - On phantoms with a known field, the shape error falls from 0.026 to 0.0097. The white matter is more uniform after all five final N4s.
  - On ds005454 sub-16, ANTs' final N4 corrected nothing: the field is 1.0000–1.0002 because of 43 negative voxels. Corrected, the white matter's CV falls from 0.0762 to 0.0689.
  - The tools are in `validation/investigations/ants-40-n4-nonpositive`.
- **ITK-06 → kept (rule C).** The oracle runs pass 509 images to ANTs programs and none holds a NaN or infinity; one file of 149 in the test catalog does. Keeping such values would turn a smoothed image entirely to NaN. It is nifti_clib's deliberate behaviour. The API pages say so; the tools are in `validation/investigations/itk-06-nonfinite`.

### Records, golden, gates
- **Records** (standard tier, each from a clean commit, 0 failures):

| Record | Cases | Commit |
|---|---|---|
| ImageMath | 334 | f312142 |
| ResampleImageBySpacing | 37 | f312142 |
| N4 | 52 | 6bf2860 |
| DenoiseImage | 51 | 6bf2860 |
| antsAI | 131 | 6bf2860 |
| 3dAutomask | 109 | 6bf2860 |
| 3dUnifize | 99 | 6bf2860 |
| antsAI, full tier (re-run because ANTS-116 touches its cases) | 144 | bbe77cc |

  Reproduce-mode agreement with the originals is unchanged; the counts grew only by the new known-result and refusal cases.
- **Golden:** re-recorded from bbe77cc after the parity smoke tier passed (2169 cases). The new reproduce outputs equal the old default checksums, output by output.
- **Gates:** Rust in both workspaces, the Python gates (450 passed, ruff, spdx) and `pytest tests/parity` all pass.

### Site
The N4, DenoiseImage, ImageMath and filters, antsAI, 3dAutomask and 3dUnifize pages now show each correction as a deliberate correction. New examples cover ANTS-40 (with the 25-call and phantom tables), ANTS-44, ANTS-17 and the ANTS-116 table; the refusals appear in the case explorers.
- The build reports 0 problems, with the main-repository links checked.
- The number check agrees everywhere (464, 261, 201, 348, 53 and 52 values), and the viewer check reports no failures.
- A second export changes nothing.
- I reviewed screenshots at 1440 and 390 px, light and dark, in `<scratch>/shots/`.

### New defects
No new IDs were used. ANTs' negative fractional `PadImage` also misplaces the data by one voxel; I folded that into ANTS-22.

### What needs your attention
- **Thread limit:**
  - Early on, the ANTS-40 measurement ran larmorx briefly at 12 threads, before my helper capped threads.
  - Re-running the N4 example's existing 12-thread runs, I pinned them to 6 CPUs with `taskset`; the pinning is recorded in the cache.
  - The parity suites' own ANTs thread-band runs go up to 12 threads.
- **Interrupted run:** the first antsAI full-tier run crashed because I overwrote a library file a running worker was using. The copy is now done safely and the run was repeated.
- **Archived:** one wrong cache entry (it eroded the CSF label under a white-matter title) is in `<workspace>/archive/2026-10-24-small-fixes-cache/`.
- **Spec writer:** the clean-room specs need the AFNI-41 and AFNI-48 refusals; handoff section 8 has the text, observable behaviour only.
- **Register:** with the handoff rows applied, the recount shows no decide and no yes left.
- Nothing needed approval, and every case was settled by the rules.
