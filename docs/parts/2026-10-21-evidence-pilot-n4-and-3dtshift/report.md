# Final report: Evidence pilot: N4 and 3dTshift

Agent `ae095c3870ac17fe5`, 2026-10-21. The agent's report to the orchestrator, verbatim.

## Part 2a (pilot) is done; I stopped here for the user's review

**Branches to fetch:**
- larmorx: `worktree-agent-ae095c3870ac17fe5`, HEAD `3559db9` (12 commits on 17d9526).
- Site: clone `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-ae095c3870ac17fe5/.claude/larmorx_documentation`, branch `evidence-pilot`, one commit `fc28f55` on 239b0ee. I squashed my WIP commits so no stale image blobs stay in the history.

Nothing pushed. All commits use the no-reply identity, with no AI attribution.

### What was built
**Export (larmorx):** `python -m larmorx_validation evidence <tool>|summary|all --site <site checkout>` (`validation/src/larmorx_validation/evidence/`). It writes into the site's `static/evidence/`:
- `ants-n4.json`, `afni-tshift.json` and `summary.json`;
- the images as blobs named by their SHA-256: raw little-endian values, compressed, lossless and exact.
  - An output close to another is stored as the XOR of its bits with it. N4's corrected images are stored against input ÷ bias field.
  - Total: 25 blobs, 18 MB for both tools.
- A re-export of unchanged results changes nothing in git (checked).
- Every metric is computed in float64 over every voxel. Each verdict is decided in code from those metrics. Every identity the chain of causes shows is asserted.
- `docs/figures.md` is rewritten for the new approach, including how to add a tool in 2b.

**Site components** (vanilla JavaScript, no CDN): `static/js/evidence.js`, `static/css/evidence.css`, `tools/evidence.py`, and `data/verdicts.yml` with the seven verdicts (colour checked for colour blindness, plus a pattern and a name).
- **Image viewer:** labelled panels, plane, slice and volume, zoom and pan, intensity windows (including "each image's own"), a crosshair showing every panel's exact value, difference panels on one stated symmetric scale (linear or log) that say "bit-identical" when zero, side by side, swipe, blink, checkerboard, and "go to the largest difference".
- **Time series** for 4D data, with a per-volume difference strip.
- **Metrics table:** grouped, with a verdict and reason per comparison; the original-against-itself rows are the yardstick.
- **Chain of causes**, plus the agreement, band and speed charts (HTML, with hover; selecting a segment filters the cases).
- **Case explorer:** search, filters, sorting, `#case=` links.
- **Without JavaScript:** still posters and full tables.

**Pages:**
- **N4** uses fMRIPrep's own final N4 node on ds000005 sub-01, run by fMRIPrep, ANTs 2.6.2 in fMRIPrep's image at 1 thread (with and without correctly rounded maths), ANTs 2.6.5 at 1–12 threads (as built and correctly rounded), and larmorx at 1 and 12 threads in both modes.
- **3dTshift** uses fMRIPrep's node on ds005040, run by AFNI, fMRIPrep's own AFNI, and both implementations in both modes at 1 and 12 threads. It has a second example: AFNI-01 on the same run re-stored with a negative scale factor.
- **Validation page:** the method, how to read the evidence, the verdict definitions (each with an anchor), and the matrix of every tool's verdict counts.

### What the evidence shows
- **N4:** fMRIPrep's output is bit-identical to ANTs 2.6.5 at 8 threads, and ANTs 2.6.2 equals 2.6.5 at equal threads and maths. larmorx, at any thread count and in either mode, equals ANTs at 1 thread with CORE-MATH.
  - larmorx − ANTs as built: identical with correctly rounded maths (max |Δ| 0.0312).
  - larmorx − fMRIPrep: within ANTs' own variability (0.0967; it is literally ANTs 8t − ANTs 1t with CORE-MATH).
  - The record's 51 cases: 30 bit-identical, 16 identical with correctly rounded maths, 5 rejected by both.
- **3dTshift:** the replica equals AFNI and fMRIPrep in every value.
  - The clean-room original differs in 189 of 10,321,920 values by one stored unit (its Fourier shift is double precision, AFNI's float32).
  - On the AFNI-01 input, AFNI's output is sign-flipped and scaled, while both default modes give the correct values (marked deliberate correction) and reproduce mode gives AFNI's.

### Checks and gates
- Site build: 0 problems.
- Number check via `shoot.mjs --eval`: 398 of 398 numbers shown agree with the JSON, and 35 of 35 values decoded by the viewers match it exactly, at 1440 and 390 px.
- larmorx: `ruff check` and `ruff format --check` clean; `check_spdx` passes; `pytest tests/python tests/golden` 396 passed.

### Decisions for the user
1. **Taxonomy:** the clean-room original's Fourier last-bit differences (103 cases) fit none of the seven verdicts exactly. I widened *Open difference* to "not accounted for by the others; its cause may be known but it is not closed". They now show as magenta "open".
2. **Matrix:** for the tools not yet redone, cases their records pass "within thresholds" are counted in an eighth column, "not yet classified". That covers antsApplyTransforms 28, ImageMath 5, ITK transform files 1 and head-motion correction 121. Part 2b classifies them.

### Records changed
The record JSONs now keep each case's reported values, so a case's verdict is read rather than inferred. I regenerated `afni-tshift` and `afni-tshift-replica` from clean commits; their results are unchanged (only the date and commit lines differ). Please re-run their standard-tier parity at merge.

### Screenshots
All in `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/evidence/shots/final/`. The most telling:
- `f01-n4-example-1440-light-y0.png`
- `f11-afni01-own-window-1440-dark-y0.png`
- `f07-ts-example-1440-light-y0.png`
- `f02-n4-example-phone-390-dark-y0.png`
- `f16-validation-matrix-1440-light-y0.png`
- `f18-n4-nojs-1440-light-y0.png`

### Left for part 2b
- The other tools' pages.
- Classifying their within-thresholds cases.
- Re-running N4's benchmark against the current oracle; its speed chart still carries that caveat.

### Text to add (I did not edit these files)
- **CLAUDE.md "Where things are":** "Visual reports part 2a done (2026-10-21): `python -m larmorx_validation evidence <tool>|summary|all --site <site>` exports interactive evidence (docs/figures.md); pilot: N4, 3dTshift, the Validation page; next: part 2b (the other tools)."
- **Rule 7:** mention the evidence export and the verdict taxonomy (the site's `data/verdicts.yml`).
- **`docs/next-steps.md`:** add part 2b with the two decisions above.
- **Site CLAUDE.md "Figures" section:** point to README.md's "The validation evidence" section.

Scratch notes: `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/evidence/NOTES.md`.
