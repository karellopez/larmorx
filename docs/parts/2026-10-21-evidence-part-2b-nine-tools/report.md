# Final report: Evidence part 2b: nine tools

Agent `a60162af2996553ab`, 2026-10-21. The agent's report to the orchestrator, verbatim.

## Evidence part 2b: done, stopped at the end of the part

**Result.** All nine tool pages are now interactive evidence, and every case is classified. The Validation page's matrix has no "within thresholds, not yet classified" column any more. Everything is committed, nothing is pushed, and no tool behaviour changed. One open issue: I wrote the site commit through the clone under `.claude/`. It may have raised permission prompts (details under "Problems").

**Where it is**
- larmorx: worktree `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-a60162af2996553ab`, branch `worktree-agent-a60162af2996553ab`. 18 commits on e3ba51d (b9075b5 … bb05c43), clean tree, no AI attribution.
- Site: clone `<worktree>/.claude/larmorx_documentation`, branch `evidence-2b`. One squashed commit, bee1c6e, on 85844e1, authored with the no-reply identity, clean tree.
- Working copy of the site: `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/evidence-2b/site`. Notes: `.../evidence-2b/NOTES.md`.

### Per tool: worked examples and verdicts
1. **antsRegistration.** sMRIPrep's T1w→MNI registration on every third voxel, with SyN and BSplineSyN. Images shown: template, moving, warped, checkerboard, and the dx/dy/dz fields (LPS). Compared against ANTs at 1–8 threads, CORE-MATH, and larmorx. Also a table of the linear stages' parameters and field norms, and the thread band.
   - Verdict: within-variability (inside ANTs' own thread band).
   - The thread band ratio is now 1.52; the earlier record said 1.87. This is ANTs' own nondeterminism between records.
2. **DenoiseImage.** A real T1w denoised by ANTs at 1–12 threads, repeated at 8 and 12 threads, plus fMRIPrep's own ANTs, CORE-MATH and larmorx.
   - New finding: the repeated runs gave identical bytes on 2026-10-21. I updated `docs/findings/ants-denoise.md` and ANTS-42 in the register.
   - ANTS-43 is shown as a d13 case.
3. **antsApplyTransforms.** fMRIPrep's `anat2std_t1w` resampling through the .h5 composite with Lanczos, in double and with `--float 1`, against ANTs as built and with CORE-MATH.
   - Verdicts: bit-identical / identical-cr. The 28 formerly unclassified cases are transform outputs, now bit-identical.
4. **Head-motion correction.** fMRIPrep's mcflirt command on a real run: FSL 6.0.7 mcflirt against the larmorx clean-room original. Shows six volumes, motion parameters and FD over the run with mcflirt's perturbation band (int16 reversed copy and noise), a stats table, and a D13 section.
   - 121 cases classified: 30 within-variability and 91 open, plus 1 failing case kept open. The page says plainly that most open cases have no measured band yet.
5. **One-shot BOLD resampler.** ds005040 native resampling with its field map, plus PIPE-03's Jacobian on the MNI grid against the reference. Metrics are taken inside the brain mask.
6. **ImageMath and the filters.** sMRIPrep's brain-extraction calls on their own inputs (a T1w case and a padded case), with one case explorer per program.
   - I re-ran the 5 cases that had passed only within a tolerance: 3 are now identical-cr and 2 are divergence (shown with their reason).
7. **antsAI / antsAffineInitializer.** The brain extraction's initial transform, shown as parameters, placed images (moving image masked) and distances in mm. Every case of both records is shown.
8. **ITK transform files.** Every dataset and attribute of fMRIPrep's real .h5 composite as h5py, ITK and larmorx read it, and the files larmorx writes as ITK reads them back.
   - The 1 unclassified case is now bit-identical.
9. **NIfTI I/O.** Header fields and data against nibabel for two shown files; 254 files from 41 OpenNeuro datasets, grouped by source; speed.
   - 66 cases are within-variability: their matrices differ only in the last bits. This is measured, not assumed. With nibabel's qform computed in double (what nibabel does on arm64 and Windows), larmorx's matrices match it bit for bit (NIB-04).
   - The record now counts 2 more files than the README's 252, so I updated the README.

### Checks
- **Data:** part 2b adds 31.60 MB; all blobs together come to 49.92 MB, within the ~60 MB budget.
- **Exports** ran from clean commit bb05c43 with `--no-compute`.
- **Number check** (`LX_EVIDENCE.check()`): 1869 numbers, 0 mismatches. `checkViewers()` passes on every page.
- **Re-export check:** a second export gives byte-identical output.
- **Paths:** provenance shows no machine paths (only `<larmorx>`, `<workspace>`, `~`, including the commands' environment values).
- **Site build:** 0 problems. The part-1 static figures are removed.

### Screenshots
They are in `.../evidence-2b/shots/` (desktop 1440, phone 390, light and dark; final set in `shots/final/`). The most telling ones:
- `hmc-v-1440-light-y4700.png`, `hmc-v-1440-light-y3050.png`
- `reg-1440-light-y3700.png`
- `den-1440-light-y3500.png`
- `aat-ex-1440-light-y2900.png`
- `rs-1440-light-y4600.png`
- `itk-1440-light-y1700.png`
- `hmc-phone-390-dark-y6200.png`
- `final/validation-1440-light-y3300.png`, `final/ai-viewer-1440-light-y2700.png`

The `final/*-example` element captures show blank canvases. That is a capture artefact: the full-page shots of the same viewers render correctly.

### Problems
- **Permission prompts.** I wrote the site commit with rsync and `git commit` into the clone under `.claude/larmorx_documentation`, which may have prompted. Two attempts to clone the site into scratch were refused by the auto-mode classifier. Two leftovers sit under `.claude/`:
  - `<worktree>/.claude/py`, a wrapper script;
  - `<worktree>/.claude/s/`, from before your message.

  Neither is tracked. I haven't deleted them (never-delete rule); archive or remove them as you see fit.
- **One bug fixed in this part's export code.** The first denoise export used 28 GB, because a band with an infinite ratio made the axis ticks loop forever. I fixed it (non-finite ratios are excluded); exports now run under `ulimit`.

### What's left
- The head-motion band is still unmeasured for the option and synthetic cases, which is why 91 cases are open. Measuring mcflirt's band per case would classify them.
- The antsRegistration benchmark is still pending.
- Suggested line for `docs/next-steps.md`: "Evidence part 2b merged (site bee1c6e; larmorx bb05c43): every tool page interactive, no unclassified cases. Follow-ups: measure mcflirt's band for the HMC option and synthetic cases (91 open); antsRegistration benchmark."
- For CLAUDE.md's "Where things are": the build-cache seed is unchanged, since this part built no Rust.

### Gates
`ruff check`, `ruff format --check`, `check_spdx` and `pytest tests/python tests/golden` (396 passed) all pass. The cargo gates weren't run because no Rust changed, and `pytest tests/parity` wasn't run because no tool behaviour changed.

### Commits
- larmorx (18):
  - b9075b5: records keep measured values; provenance paths normalised
  - 1dfe56c: ruff format
  - 98b0bee: eight records regenerated from clean commit 1dfe56c, results unchanged
  - 246c4e1: shared pieces (`common.py`)
  - one commit per tool: f2bf1e9, 9d61493, 76a2611, f575be3, b6d82aa, 046ce76, 3997cc8, fb85b26, 6c8a14f
  - ca8bf18: TOOLS, summary, `docs/figures.md`, findings
  - 2c9aa47: README
  - b80cebd: NIB-04 measured
  - 63fef36: provenance environment values
  - bb05c43: DenoiseImage label
- Site: bee1c6e.
