# Final report: Differential random testing, generated tables, memory against the originals

Agent `ab7de196daece3c4e`, 2026-10-25. Its final report to the orchestrator, verbatim, preceded
by the orchestrator's summary of the part's course (the interim messages are in the session).

## The part's course

- **The first night crashed the machine** (08:35): a random N4 case on a header in metres made
  ANTs grow to 28 GB; the machine swapped until VS Code went down. Fixes: `boxed.sh` (memory boxes,
  CLAUDE.md), per-worker 3 GiB caps, a heavy queue. The runaway case was explained: a 52 m image
  with a 20 mm spline distance (ANTS-162; a guard by default, the method kept).
- **The user's decisions that day:** heavy cases are run, not avoided ("we cannot avoid the test.
  We should do it"); random tests of reasonable size ("I thought we were supposed to use reasonable
  size tests"); and "we should fix the memory problem": larmorx never needs more memory than the
  original for the same call.
- **Random testing found** the mcflirt replica's `atanf` constant (61 findings, all bit-identical
  after the fix by the mcflirt agent) and dozens of larmorx and original defects (triage below).
- **Heavy batch 1** (44 cases, 2.67 h): 26 agree, 14 notices; the rest pending on a larger machine.

## Report: differential random testing, the generated tables and memory against the originals (agent B)

The part is finished and committed. Every image program I measured now needs at most ANTs' peak memory (plus the Python interpreter's few MiB), with outputs unchanged bit for bit. Gates pass except one known smoke failure that main already fixes. Nothing was pushed or merged.

**Memory (the user's rule: never more than the original for the same call)**

Peak resident memory per process, one thread, larmorx's command against ANTs 2.6.5:

| Call | ANTs | larmorx before | larmorx now |
|---|---|---|---|
| ResampleImageBySpacing, 67 M output voxels | 279 MiB | 2.2× ANTs | 283 MiB |
| N4BiasFieldCorrection, -s 2 | 79 MiB | 104 MiB | 41 MiB |
| DenoiseImage | 40 MiB | 85 MiB | 44 MiB |
| ImageMath GE | 89 MiB | 213 MiB | 85 MiB |
| ImageMath GD | 88 MiB | 181 MiB | 85 MiB |
| ImageMath ME | 97 MiB | 150 MiB | 94 MiB |
| TruncateImageIntensity | 89 MiB | 117 MiB | 85 MiB |
| SmoothImage, ThresholdImage | 83 MiB | 90 MiB | 88 MiB |
| a call on 16 voxels (the interpreter) | 18 MiB | 32 MiB | 20 MiB |

- The other ImageMath operations measured (MD, MO, MC, GO, GC, GetLargestComponent, FillHoles, Normalize, PadImage, m, G) are all at or below ANTs.
- Causes fixed: ResampleImageBySpacing's second pass to count ANTS-32's voxels; the gzip writer holding whole batches; the command line importing everything at start; line-filter task lists 8 times finer than needed; N4's B-spline factor cache (256 bytes per point) and copies of the input, mask and result (a new uncached fit keeps N4 at 5.5 s against 4.3 s with the cache; ANTs 21 s); DenoiseImage's 28 MiB window of contributions, two extra padded images and the separate count image; grayscale morphology's up to five full-size images (now in place); ME building both modes' outputs to count ANTS-17's voxels (now one pass); TruncateImageIntensity's label image.
- The resamplers' memory guard counts the measured 7.5 bytes per output voxel instead of 16.
- `tests/parity/test_memory_peak.py`: 13 mid-size calls held to ANTs' peak + 8 MiB, larmorx bit for bit in `reproduce` (N4 and DenoiseImage against ANTs with CORE-MATH): 13 of 13 pass.
- Re-ran the standard parity tiers of ImageMath, SmoothImage, ThresholdImage, MultiplyImages, both resamplers, DenoiseImage, N4 and Atropos: every case's status matches the committed records.
- Not measured yet: antsRegistration, antsApplyTransforms, Atropos, antsAI, the AFNI programs and mcflirt (a follow-up).

**Realistic random tests:** nine cases in ten use realistic voxel sizes (about 0.5–4 mm, normal matrix sizes and options); one in ten is extreme and labelled. A case's peak memory is predicted before it runs; one predicted over the case limit is checked as a guard case instead. `docs/validation/difftest.md`, with unit tests. `--heavy` runs the recipe the night drew (from its findings).

**Heavy queue:** batch 1 ran 18:18–20:58 (2.67 h against 2.86 h predicted) with the fixed build: 26 agree, 14 notices that changed no value, no difference. 25 are "pending: larger machine" (13 long N4 cases, 51.5 h; 8 beyond this machine's heavy box; 4 reruns that hit harness limits here, now fixed: chunked comparison of outputs over 2²⁸ values, efe03b6; the case's own limit for the original, cd2fc27). 11 stay "beyond any machine", covered by the guards (ANTS-162, AFNI-75).

**Triage:** all 58 groups of differences from the first night: 34 fixed in larmorx, 6 in the register, 17 reported for the clean-room agents, 1 harness.

**Gates:** Rust fmt, clippy and tests in all three workspaces; ruff, format, SPDX; `pytest tests/python tests/golden validation/tests` 579 passed (golden unchanged); `pytest tests/parity` 2399 passed plus the known ds003592 stage failure that main fixes; the site built against its current main, every page at least as before, 312 evidence files byte-identical, `difftest.json` added, all panel checks agree.

**Follow-ups:** the remaining tools' memory measured the same way; ResampleImage nearest-neighbour to `unsigned char` 2.4× ANTs' time; AFNI-75's guard for the clean-room 3dUnifize (spec, then a clean-room agent); ITK-01's missing notice in antsRegistration's field lookups; the `reproduce` gaps of ITK-95, ITK-96 and ANTS-161; one voxel of `resample_series` one ulp from fMRIPrep's; the 25 pending heavy cases on the lab's server.

## At the merge (orchestrator)

Conflicts resolved (evidence `OTHER` gains `difftest`; README and the validation index regenerated
by `tables`; `validation/tools.toml`'s mcflirt template keeps the details the hand-written rows
had). The handoff was integrated by a helper agent and reviewed (ITK-96 kept as its own entry: it
is a separate switch in the code; the register recounted: 254 entries).
