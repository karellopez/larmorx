# Final report: Visual reports part 1: figures

Agent `a23fe03519502c885`, 2026-10-21. The agent's report to the orchestrator, verbatim.

## Visual reports, part 1: finished. All three parts of the brief are done; nothing is left running.

**Framework (larmorx, `validation/src/larmorx_validation/figures/`)**
- **Command:** `python -m larmorx_validation figures <tool>|all --out <site>/static/figures [--only …] [--cache …] [--no-compute]`.
- **Inputs:**
  - The committed records: the JSON and the Markdown tables, which are read by column name and fail loudly if missing.
  - The benchmark reports.
  - A figures cache, `oracles/figures-cache/<tool>/<case>/`, with a `RUN_INFO.json` per case (commands, environment, threads, seed, versions, input SHA-256, commit). A missing case is recomputed.
- **Outputs:** every figure as SVG in light and dark, at 720 px and a 360 px phone width. Text is drawn as the site's fonts' outlines. The fonts are fetched once from google/fonts at a pinned commit with SHA-256 checks. Brain images are embedded as lossless WebP. Beside each figure, `<figure>.json` records provenance: records with SHA-256, cache commands, data and licences, the MNI notice, commit, command and versions.
- **Style:** the site's palette. The state and program colours passed the dataviz colour-vision validator in both themes. Brain slices are masked to a dilated brain mask, so no face can appear. Difference maps use a symmetric scale with its range stated, and a zero map reads "bit-identical".
- **Packaging:** a `figures` extra in `validation/pyproject.toml`. ruff now allows "−" as a confusable character.
- **Guide:** `docs/figures.md` (run, cache, provenance, style, data rules, how to add a tool, check, publish). It is linked from row 13 of the porting guide's §11 table and from `validation/README.md`.

**Figures (37), per tool**
- **antsRegistration:** agreement (273 identical to ANTs + 36 with CORE-MATH, as the first bar), ANTs' thread band (30 cases), and sMRIPrep's T1w→MNI registration on every third voxel (ds000005). The image figure has a contour overlay, a checkerboard, and displacement-field difference maps: larmorx vs ANTs as built, ANTs at 4 vs 1 thread, larmorx vs ANTs with CORE-MATH.
- **N4:** agreement, differences against ANTs' thread band, the bias field on the ds000005 T1w with three difference maps, speed.
- **DenoiseImage:** the thread-dependence chart (ANTs differs from its one-thread result in 21.3 / 49.5 / 81.7 / 85.3 % of voxels at 2 / 4 / 8 / 12 threads; larmorx in 0.09 %), coronal maps of where the differences fall, agreement, band, speed.
- **antsApplyTransforms:** agreement with double and `--float 1`, fMRIPrep's T1w→MNI `.h5` with Lanczos (largest difference from ANTs as built 7e-13; float and CORE-MATH bit-identical), speed.
- **3dTshift:** agreement for the clean-room original and for the replica, the time series of a ds005040 run through fMRIPrep's command (original: 175 int16 values off by 1 unit; replica: 0), speed.
- **One-shot resampler:** agreement, and the PIPE-03 Jacobian on ds005040's own field map on the MNI grid. It reproduces the record's numbers: fMRIPrep's median error 0.0124 against larmorx's 0.0051.
- **Head-motion correction (marked experimental):** agreement; the six motion parameters and FD against the recorded mcflirt run on ds000005; the deviation against mcflirt's own perturbation band, with the D13 default visible at the last volume; speed.
- **ImageMath and the filters:** an overview of the six programs, ImageMath by category, speed on the pipelines' calls.
- **antsAI and antsAffineInitializer:** two agreement charts and the thread band.
- **NIfTI I/O:** agreement by source of the files, read and write speed against nibabel and SimpleITK.
- **ITK and HDF5 transform files:** agreement and the per-file checks.
- **No benchmark report:** antsRegistration, antsAI and the HDF5 files get no speed chart; their pages say the benchmark is pending.
- **Data:** OpenNeuro ds000005 (PDDL-1.0) and ds005040 (CC0-1.0), and MNI152NLin2009cAsym with its notice in the captions.
- **Clean provenance:** all real-data cases were recomputed from a clean commit; every figure's provenance names 1568b5c, except ImageMath and the filters (07f43af).

**Site (`larmorx_documentation` clone, branch `figures-part1`)**
- **Figure component:** `{{ figure('<tool>/<name>', caption) }}` in `tools/build.py`. It shows the version of the current theme (switch and system setting), the narrow version below 560 px, lazy loading, numbered captions and a Provenance link. Styles are in `site.css`.
- **Tools section:** `content/tools/` holds an index and 11 pages: what each tool is, how it is implemented, how to call it, numbers quoted from its record with links, figures, the original's variability, speed or pending, D13 corrections, limits.
- **Navigation and links:** Tools is in the header and the sidebar (`site.yml`, now with summaries). The home status table links each tool's name (`page:` in `status.yml`). The Validation page points to Tools. The home page's ANTs card now says "registration (linear and deformable stages)".
- **Landing fix (a):** on phones mid-scroll, the rings and the brain rise into the free space while neither text is shown (`hero.js`, `shiftNarrow`).
- **Landing fix (b):** Greek letters and subscripts now come from small `unicode-range` subsets. These are Inter's and JetBrains Mono's own Greek, plus Noto Serif Display (italic and upright) beside Instrument Serif, which has no Greek. They are made by `tools/fonts/make_greek.py` (pinned, SHA-256 checked). The OFL text is in `static/fonts/`, and the footer credits the font.
- **Checks:** `tools/build.py` builds 15 pages with 0 problems. I looked at screenshots on desktop and phone in light and dark (home hero at three scroll points, tool pages) and fixed what was wrong.

**What the figures showed (no new defects; everything is consistent with the records)**
- On the template registration, glibc's maths moves ANTs' SyN field by up to 1.1 mm, about as much as ANTs' own threads do. With CORE-MATH, larmorx and ANTs are bit-identical.
- On the N4 case, larmorx is bit-identical to ANTs at one thread, while ANTs at 4 threads differs from itself by up to 0.11.
- On ds000005, the hmc step change at volumes 100–110 comes about 10 volumes later in larmorx than in mcflirt (RMS 0.06 mm), inside mcflirt's own band. This is the known agreement level, not a new finding.

**Gates and commits**
- **larmorx gates:** ruff check and format, `check_spdx`, and `pytest tests/python tests/golden` (396 passed). No Rust was touched.
- **larmorx commits** (worktree branch `worktree-agent-a23fe03519502c885`): 498069f, d7e8cf3, 712a0b0, 663683d, 1568b5c, 07f43af.
- **Site commits** (`figures-part1`): d554c51 (landing), 8b8be88 (Tools pages and component), b8fec84 (final figures).
  - The new `site.css` rules (Greek `@font-face`, `--math`/`--serif`, figure styles) landed in 8b8be88, not with the font files in d554c51.
  - The clone had no local git identity, so it would have used the global (personal) address. I set it to the no-reply address before committing.

**Things to know**
- `static/figures` is about 12 MB in the site repository (37 figures × 4 SVGs, 60–200 KB each, much smaller when GitHub Pages serves them compressed). Making it smaller would need per-SVG embedded font subsets instead of outlines.
- Six earlier cache entries, computed while the figure code was uncommitted, are in `<workspace>/archive/2026-10-21-figures-cache-dirty/`.
- This worktree holds a 25 GB `target/` that can seed the next build. `crates-gpl/target` was built too.

**Suggested text you could add**
- **CLAUDE.md, "Where things are":** "Visual reports done (2026-10-21): `larmorx_validation.figures`, guide `docs/figures.md`, figures cache `oracles/figures-cache`, one site page per tool under Tools."
- **docs/next-steps.md:** mark item 1 done, and add as follow-ups: benchmarks for antsRegistration, antsAI and HDF5 (their pages say pending); smaller figure files (per-SVG font subsets).
- **Site CLAUDE.md:** "brain images as lossless WebP embedded in the SVGs."
