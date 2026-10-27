# HANDOFF: the ANTs tools on the data panel (re-validation, part a, 2026-10-26 and 27)

Branch: the worktree `agent-ac4ca723afe886719` (larmorx); site clone branch `ants-panel-evidence`
(`<scratch>/site`, its images on the clone's `evidence-blobs`). Exact texts for the shared
documents follow; numbers come from the committed records (`docs/validation/*-panel.md`).

## 1. `validation/tools.toml` (then `python -m larmorx_validation tables`)

Apply `<scratch>/tools_toml_patch.py <checkout> validation/tools.toml` (it appends to each ANTs
tool's `validation` and each ANTs record's `status`; rendered and checked against the committed
records, `--render`), or by hand:

The patched file, rendered against the committed records with `--render`, is
`<scratch>/tools.toml.patched` (25 panel mentions; README row of N4 for example: "...; data panel:
[20 of 69](docs/validation/ants-n4-panel.md) as built, 69 of 69 with correctly rounded maths").
Copy it over `validation/tools.toml`, then `python -m larmorx_validation tables` (and with
`--site <site>` for `data/status.yml`). The script `<scratch>/tools_toml_patch.py` holds every
appended string.

## 2. `docs/decision-log.md`: a new entry before "## Open decisions (PLAN.md §16)"

```markdown
## 2026-10-26: The ANTs tools on the data panel (re-validation, part a)

- **The panel tier** (`--tier panel`, beside smoke, standard and full; never in CI): every ANTs
  suite replays each call fMRIPrep 25.2.6 made of its program in the fourteen finished oracle runs
  (`larmorx_validation.parity.fmriprep_nodes`: each node's own `command.txt`, its inputs from the
  run's working directory, calls repeated word for word compared once) and runs the program on the
  panel's images where fMRIPrep's calls do not reach (`parity.panel_images`: the 7 T MP2RAGE, the
  glioma, the stroke with its lesion mask, the 3.5-year-old, the 82-year-old's FLAIR, the 1.5 T
  MPRAGE, the child's low-quality T1w). The template registration's inputs, which niworkflows
  writes for itself and nipype does not keep, are made again exactly from the node's recorded
  inputs (`parity.niworkflows_inputs`), its initial transform by the ANTs oracle's
  antsAffineInitializer at one thread (fMRIPrep runs it inside that node: it is not a node of its
  own). The records are `docs/validation/<suite>-panel.md`; the standard and full records are
  unchanged.
- **n_threads exact** (the user, 2026-10-26, during the part): some loops ran on rayon's global
  pool (sized by the machine's CPUs) instead of the call's pool: the image metric's samples
  (antsAI, antsAffineInitializer, the linear stages), the BSpline and TimeVaryingBSplineVelocity
  stages' adaptors and time samples, Atropos' k-means and its Parzen-window likelihoods. Fixed in
  this part (results unchanged: golden checksums, the thread-count invariance tests); a probe of 74
  code paths of every registered program (ANTs, AFNI originals and GPL replicas, the head-motion
  correction, the FSL replicas) finds no thread outside the call's pool
  (`tests/parity/test_thread_pools.py`; `docs/findings/ants-cli.md`). The clean-room crates
  needed nothing. Records made before the fix stay valid (results unchanged; the panel records
  made then ran with `RAYON_NUM_THREADS` equal to their thread count).
- **Memory** (the user's rule of 2026-10-25, measured on the panel's kinds of call and on every
  tool's largest panel input): the metric's chunks are sized by what a point keeps (the BSpline
  stage held 329 MiB where ANTs holds 35), the neighbourhood correlation, the gradient images,
  ResampleImage's B-spline coefficients and the separable filters' strided axes no longer hold
  whole-image temporaries; results unchanged. antsAI and antsAffineInitializer stay above ANTs
  (their metric holds the images in double, ANTs in float): a follow-up, their two cases
  strict-xfail in `tests/parity/test_memory_peak.py`.
- **Atropos' input check** (found on the panel): ITK's `VerifyInputInformation` was missing from
  the port; larmorx now throws as ANTs does when an input is on another grid (a mask written from
  the image's affine is 2.9e-6 mm off, beyond ITK's tolerance).
- **antsAI against fMRIPrep's files:** five of fMRIPrep's 36 transforms are up to 6.8 mm from
  ANTs' (and larmorx's) one-thread result; repeated runs show ANTs' own spread above one thread
  (fMRIPrep's ANTs at one thread gives larmorx's transform exactly): within the original's
  variability, nothing to correct.
- **The whole template registrations** (14 cases, about 2.4 hours of ANTs at one thread each, as
  built and with CORE-MATH: some 67 hours of CPU) are left for the next part, on the new
  32-thread machine; this record compares them on every third voxel (the `/small` cases) and runs
  every other registration of the runs whole.
```

## 3. `docs/next-steps.md`

- In "The order from here", the bullet **Meanwhile, here, and then**: replace "every finished
  tool re-validated on the data panel (item 6; the ANTs tools, then AFNI, FSL and I/O)" by
  "every finished tool re-validated on the data panel (item 6; **the ANTs tools: done**, part a,
  2026-10-27; next AFNI, FSL and I/O, part b; the fourteen whole template registrations on the
  new machine)".
- Item 6, after "...a page over its budget gets a raised one with its reason, never less
  content (the user, 2026-10-24).": add "**The ANTs tools: done** (part a, 2026-10-26 and 27):
  panel records for every ANTs suite (`docs/validation/ants-*-panel.md`: ImageMath 374/374,
  ThresholdImage 18/18, MultiplyImages 44/44, SmoothImage 4/4, ResampleImage 4/4,
  ResampleImageBySpacing 3/3 bit-identical; N4 69/69, DenoiseImage 21/21, Atropos 17/17,
  antsApplyTransforms 425/425 bit-identical with CORE-MATH; antsAI 40/40, antsAffineInitializer
  14/14, antsRegistration 71/71 identical to ANTs at one thread), each ANTs page with its panel
  section; n_threads made exact; memory fixes; Atropos' input check."
- "Follow-ups, when they fit": replace the item **Memory against the originals, the remaining
  tools** by: "**Memory, the rest** (the rule as refined 2026-10-27): antsRegistration,
  antsApplyTransforms, Atropos, N4, DenoiseImage and the image programs measured on the panel's
  largest inputs and kept at or below ANTs (`docs/findings/ants-image-programs.md`, "Memory on the
  data panel's largest inputs"); **antsAI and antsAffineInitializer still above** (127 and 110
  MiB against 92 and 91 on a 128 × 128 × 96 head; +40 and +156 MiB on the 7 T): their metric holds
  the images in double, ANTs in float; store them in their pixel type (strict-xfail cases in
  `tests/parity/test_memory_peak.py`); the AFNI programs, mcflirt and `resample_series` remain to
  measure (part b)." Add: "**The fourteen whole template registrations** of the panel (about 67
  hours of CPU) on the new machine: `python -m larmorx_validation parity ants-registration --tier
  panel --case 'register_template.*/registration$'`." Add: "**antsAffineInitializer's speed**:
  larmorx 277 s against ANTs' 73 s at one thread on a 128 × 128 × 96 head, 1212 s against 332 s
  on the 7 T (the D13 notice of ITK-21 runs every local search twice; the rest is the metric's
  speed)." Add: "**ImageMath's panel run in a box**: its 600-volume `m` case needs about 8 GiB in
  the harness (both outputs and larmorx in-process): two jobs in a 20 GiB box."
- "Waiting for the user": nothing new.

## 4. CLAUDE.md, "Where things are"

In the table's ANTs row, after "Atropos (every option; sMRIPrep's call bit-identical with correctly
rounded maths on the five oracle runs)", add: "; **re-validated on the data panel** (2026-10-27:
fMRIPrep's every ANTs node of the fourteen oracle runs and the panel's own images, records
`ants-*-panel`)". In **Next:**, replace "then every finished tool re-validated on the data panel"
by "then the data panel's re-validation of AFNI, FSL and I/O (part b; the ANTs tools' part a is
done)". In "In place", after **Memory:** add: "the thread pools exact (every call's parallel
work in its own pool of `n_threads`: `tests/parity/test_thread_pools.py`, 74 paths)".

## 5. The site: `data/status.yml`, `content/validation.md`, `static/evidence/summary.json`

- `data/status.yml`: regenerate with the patched `tools.toml` (`python -m larmorx_validation
  tables --site <site>`).
- `static/evidence/summary.json`: re-export (`python -m larmorx_validation evidence summary
  --site <site>`); the ANTs pages' JSON are on the branch `ants-panel-evidence`.
- `content/validation.md`: under the data panel's paragraph, one line: "The ANTs tools are
  re-validated on the panel (2026-10-27): every page has a section *On the data panel* with
  fMRIPrep's calls of the fourteen oracle runs and the panel's own images."
- The blobs: `evidence-blobs` in the clone is `d1c812b` (547 blobs, 209 MB): `git push --force
  origin evidence-blobs` before pushing main.

## 6. `docs/porting-guide.md` (if the orchestrator keeps a speed claim's provenance)

The BSpline stage's "20 times faster than ANTs at one thread on a 4D case" (A5 part 2) was
measured when the metric's loop ran on rayon's global pool at any thread count. Re-measured after
the fix, truly at one thread (`bspline/4d`, CPU-pinned): ANTs 668 s, larmorx 27.1 s (24.6 times),
38 MiB against ANTs' 35. The claim holds; no change needed.

## 7. For a clean-room agent

The thread-pool probe (`larmorx_validation.parity.thread_pools`, 74 paths) found no thread outside
the call's pool in the clean-room crates (`crates/larmorx-afni`: 3dTshift, 3dAutomask, 3dUnifize;
`crates/larmorx-mri`: hmc): nothing to hand over.

## 8. For part b (AFNI, FSL, I/O)

- `larmorx_interp::ndimage`'s spline filter (`spline_filter`, used by `lx.transforms.resample_series`
  and `lx.ndimage`) has the pattern fixed here in the 3D decomposition: on its last axis, when
  there are few outer blocks, it copies every tile out before filtering (a second copy of the
  volume). Measure `resample_series` against fMRIPrep on the panel's largest series and fix it the
  same way if it needs more than the original.
- The panel's builders of derived masks should write a mask with its image's own header (see
  `docs/findings/ants-atropos.md`, "Inputs on another grid"): a mask written from the affine can
  be 1e-6 off ITK's tolerance.

## 9. The memory rule, refined (2026-10-27, seen in the main CLAUDE.md)

`tests/parity/test_memory_peak.py` still allows ANTs' peak plus 8 MiB (`OVERHEAD_MIB`); with the
refined rule (10 % plus 8 MiB) its assertion would be `mib <= ants_mib * 1.1 + OVERHEAD_MIB`.
antsAI (127 against 92 MiB) and antsAffineInitializer (110 against 90) exceed the refined rule too,
so their strict xfails stand.
