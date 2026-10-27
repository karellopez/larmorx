## Data panel re-validation, part a (the ANTs and ITK tools): report

All 13 ANTs and ITK tools now have a panel record. Every compared case agrees with ANTs 2.6.5 at one thread. Three things are left: antsAI and antsAffineInitializer still use more memory than ANTs, the 14 whole-size template registrations were not run, and antsAffineInitializer is slow at one thread.

### Panel records (`docs/validation/*-panel.md` and `.json`, each from a clean commit)
Each suite gained a `--tier panel` (never in CI). It replays every ANTs node of the 14 finished fMRIPrep oracle runs from the node's own `command.txt`, and runs the program on panel images that fMRIPrep never gives it.

| Tool | Cases | Result | Commit |
|---|---|---|---|
| ImageMath | 374 | 374/374 bit-identical, headers included (378 nodes, 365 distinct calls) | 6e810bc |
| ThresholdImage | 18 | 18/18 bit-identical | b7ba0d3 |
| MultiplyImages | 44 | 44/44 bit-identical | b7ba0d3 |
| SmoothImage | 5 | 4/4 bit-identical, 1 rejected by both | b7ba0d3 |
| ResampleImage | 4 | 4/4 bit-identical | b7ba0d3 |
| ResampleImageBySpacing | 3 | 3/3 bit-identical | b7ba0d3 |
| N4BiasFieldCorrection | 70 | 69/69 with CORE-MATH (20 as built), 1 rejected by both | 6e810bc |
| DenoiseImage | 21 | 21/21 with CORE-MATH (0 as built) | b7ba0d3 |
| Atropos | 18 | 17/17 with CORE-MATH, classes identical in all, 1 rejected by both | b7ba0d3 |
| antsApplyTransforms | 425 | 425/425 with CORE-MATH; 329 as built, including all 167 `--float 1` calls | 349d3f7 |
| antsAI | 40 | 40/40 identical to ANTs at one thread | 31f7554 |
| antsAffineInitializer | 14 | 14/14 identical as built | 31f7554 |
| antsRegistration | 71 | 71/71: 16 as built, 55 more with CORE-MATH | 349d3f7 |

- **Node inventory:** 1,023 ANTs nodes in the 14 runs:
  - antsApplyTransforms 421
  - ImageMath 378
  - N4 64
  - antsRegistration 57
  - MultiplyImages 42
  - antsAI 36
  - DenoiseImage 17
  - Atropos 14
  - ThresholdImage 14
  - antsAffineInitializer: 14 calls inside the template-registration node; it is not a node of its own.

  Every node is compared except the 14 whole template registrations. The records' inventory tables name them, with the reason.
- **The 14 whole template registrations** (about 67 h of CPU) are not run. They are compared on every third voxel (the 14 `/small` cases) and proposed for the next part on the new machine. Their inputs are remade exactly from each node's recorded inputs (`parity.niworkflows_inputs`), because nipype did not keep them. Every other registration ran at full size: the 14 brain extractions, 22 BOLD and 7 field-map coregistrations.
- **antsAI against fMRIPrep's own files:**
  - 31 of 36 transforms are within 1e-8.
  - Five are up to 6.8 mm off (displacement over a 100 mm cube).
  - `ai_variability.py` explains them: fMRIPrep's own ANTs at one thread gives exactly larmorx's transform in all 7 calls studied, and ANTs at 6–8 threads lands, run to run, on fMRIPrep's file, on the one-thread result, or elsewhere.
  - This is ANTs' own spread above one thread, so nothing needs correcting (`docs/findings/ants-ai.md`).
- **Atropos against fMRIPrep's own files:** the same segmentation in every run; posteriors differ in their last bits in 0 to 4,175 voxels.

### Fixes in larmorx (results unchanged unless stated; each has a permanent test or case)
- **n_threads exact (the user's request):**
  - Before the fix, some work ran on rayon's global pool instead of the call's own pool: the image metric's samples, the linear stages, the BSpline and TimeVaryingBSplineVelocityField stages, antsAI and antsAffineInitializer, and Atropos' k-means and Parzen likelihoods.
  - The probe (`parity/thread_pools.py`) now covers 74 code paths of every registered program — ANTs, the AFNI clean-room originals and GPL replicas, hmc, and the FSL replicas. It finds no thread outside the call's pool.
  - The clean-room crates needed nothing, so there is nothing to hand to a clean-room agent.
  - Tests: `tests/parity/test_thread_pools.py`, a Rust test, and a Python thread-invariance test for the linear stages and initializers.
  - No record is made misleading by the fix.
  - The porting guide's "BSpline stage 20 times faster than ANTs at one thread" was re-measured truly at one thread: 24.6 times (668 s against 27 s). The claim holds.
- **Memory** (findings in ants-registration.md, ants-ai.md, ants-image-programs.md):

  | Call | ANTs | larmorx before | larmorx now |
  |---|---|---|---|
  | BSpline stage, 4D | 35 MiB | 329 MiB | 38 MiB |
  | ResampleImage B-spline, 7 T | 256 MiB | 384 MiB | 257 MiB |
  | SmoothImage, 588-volume series | 1052 MiB | 1081 MiB | 1058 MiB |

  - The neighbourhood correlation and the gradient images no longer hold whole-image copies.
  - New `test_memory_peak.py` cases cover Atropos, apply `-e 3`, linear+SyN, BSpline, B-spline resampling and strided smoothing.
- **Atropos lacked ITK's input-geometry check.** The panel found it: a FLAIR mask written from the image's affine is 2.9e-6 mm off the grid, so ANTs exits 1 while larmorx segmented. larmorx now throws as ANTs does. Six standard-tier cases and one panel error case cover it.
- **Suite and evidence fixes** found on the way:
  - The panel builders now write masks with the image's own header.
  - N4 panel cases follow the suite's notice rules (ANTS-36, and ANTS-38 met in one mode only beside ANTS-40's correction).
  - The `oracles` symlink path no longer breaks the template inputs (`fmriprep_nodes.root`).
  - The apply record's float summary counts the panel's `--float 1` calls.
  - Two evidence-viewer bugs fixed: the volume view of a box-shaped region, and the D13 verdict reading of notices.
- **New defect IDs:** none. Every difference was a larmorx omission, a suite issue, or ANTs' own variability.

### Memory and speed on the largest panel inputs, one thread (`docs/findings/ants-image-programs.md` table)
- **Below or at ANTs' peak:**
  - N4 7 T: 814 MiB against 1814, 38 s against 233
  - antsApplyTransforms, 600-volume series: 4710 MiB against 6501
  - antsRegistration brain extraction, 7 T, whole: 2394 MiB against 2884, 1165 s against 2431, four files identical
  - the rest: within ANTs' peak or at most +6 MiB
- **Still above ANTs (follow-up):**
  - antsAI: +40 MiB on the 7 T; 127 against 92 MiB on the test head
  - antsAffineInitializer: +156 MiB on the 7 T; 110 against 90 MiB on the test head
  - Cause: their metric holds the images in double where ANTs uses float.
  - Both test cases are strict `xfail`. They also exceed the refined 10 % + 8 MiB rule now in the main CLAUDE.md; the test still uses the old allowance (HANDOFF §9).
- **antsAffineInitializer is 3.7–3.8 times slower than ANTs at one thread.** The global pool used to hide this. Part of it is the ITK-21 notice running every local search twice. Follow-up.

### Site (`<scratch>/site`, branch `ants-panel-evidence`: 3864e97, 79cc24c; `evidence-blobs` d1c812b)
- **Content:**
  - All 7 ANTs pages have an "On the data panel" section and a facts line linking the panel records.
  - 24 new worked examples, each with a verdict.
  - Whole volumes only on small grids: N4's multiband-8 BOLD reference, the lesion in 2 mm MNI space, the antsAI tumour example, and the 7 T BOLD-reference registration.
  - Large anatomies are shown as three full-resolution slices.
  - **Deviation from the brief:** I did not show 2–3 whole volumes per tool. A raw 7 T head with its panels is 245 MB, half the site's budget.
- **Size:**
  - The site is 222 MB of 500.
  - N4's page budget was raised from 60 to 75 MB, with its reason. Every other page is within 40 MB.
- **No-loss and checks:**
  - The before/after inventory (`site_inventory.py`) shows no loss on all 7 pages.
  - Build: 0 problems, including the links into the repository.
  - Number check: every number agrees. On the five pages whose counts I read: programs 408, n4 724, denoise 440, atropos 451, ai 532, apply 240 checked, all agreeing; registration 1174 checked, all agreeing. 0 viewer problems, 0 failed examples.
- **Screenshots:** 28 before and 28 after (1440 and 390 px, light and dark) in `<scratch>/shots/before` and `<scratch>/shots/after`. I looked at one; the rest are checked by script.
- **Push order:** push `evidence-blobs` before main.

### Gates
All passed on the current tree:
- Rust, all three workspaces
- pytest tests/python, tests/golden and validation/tests: 604 passed; golden checksums unchanged
- pytest tests/parity: 2547 passed, 2 expected failures (the strict xfails)
- ruff, SPDX, `tables --check`

### Commits
- **Worktree:** head da3d963, clean. Main ones:
  - b6334f4 n_threads
  - 3118e09, 92b7540 memory
  - b7ba0d3 Atropos geometry check
  - 95dde4a, 557627e, 349d3f7, 6e810bc suite fixes
  - e05a5a7, 042d70d, 97720ad evidence
  - record commits 33014da, 7133a34, 1f2ae09, 8b50cfb, ddafea3, da3d963
- **Site clone:** 3864e97, 79cc24c.
- No AI attribution anywhere. Nothing pushed.

### HANDOFF (`<scratch>/HANDOFF.md`)
- tools.toml: the patched file is ready and rendered, at `<scratch>/tools.toml.patched`.
- Exact texts for: the decision-log entry, next-steps (order, item 6, follow-ups), CLAUDE.md "Where things are", and the site's status, summary and validation page.
- Porting-guide note: the 20× claim holds.
- For part b: `larmorx_interp::ndimage`'s spline filter holds the same whole-volume copy on its last axis (used by `resample_series`).
- The memory-rule note (§9).

### What's left
- The 14 whole template registrations, on a larger machine.
- antsAI and antsAffineInitializer memory (store the metric's images in float) and the initializer's speed.

### Unsettled
Nothing. The rules settled every case.

Scratch directory: `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/7214b6a0-d66a-4953-afa8-24cf02d0691a/scratchpad/panel-ants/` (NOTES.md, HANDOFF.md, shots/, final-chain.log).
