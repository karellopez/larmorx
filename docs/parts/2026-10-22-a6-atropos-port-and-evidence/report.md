# Final report: A6: Atropos port and evidence

Agent `a6259dedbd25d703c`, 2026-10-22. The agent's report to the orchestrator, verbatim.

## A6 (Atropos): done

**The port is complete.** ANTs 2.6.5 / ITK 5.4.5 `Atropos` is in the main package as `larmorx ants Atropos`, `lx.ants.atropos` and `larmorx_ants::atropos::Atropos`. On the standard tier the classified images are identical to the unmodified ANTs binary in every compared case (56 of 56). Every compared output is bit-identical once that binary uses CORE-MATH's correctly rounded maths.

### What was built
- **Filter:** all five initializations (KMeans with and without centres, Otsu, Random, prior probability images from a `%02d` series or a vector image, prior label image) and all six likelihood models (Gaussian, partial volume, histogram Parzen, manifold Parzen with and without local covariances, log-Euclidean, joint shape and orientation).
  - All four posterior formulations, annealing, the MRF (factor, radius or coefficient image), ICM with code images, convergence, label propagation (fast marching or Euclidean), `-u`, both outlier filters, partial-volume sets, adaptive B-spline smoothing.
  - Every output, `-v` output and message; 2D, 3D and 4D.
- **Supporting code:** ITK's k-d tree and k-means (`larmorx_image::kdtree`), SLATEC's Student t distribution, and fast marching with libstdc++'s heap order.
- **Determinism:** results do not depend on the thread count (checked at 1 and 6 threads), and larmorx never reads the clock: `-r 1` uses seed 121212, `-r 0` uses ANTs' 19650218.
- **Python:** the wrapper uses typed dataclasses (`GaussianLikelihood`, `HistogramParzenWindows`, `BoxPlot`, …, `AtroposResult`). `_core.ants_program` now accepts file aliases (needed for prior series given as paths).
- **Tests:** 22 Python tests, 40 Rust unit tests, and 4 golden cases on a 24×20×16 three-tissue phantom (CLI and Python, fix and reproduce).
- **Docs:** API page `docs/api/ants-atropos.md`, findings `docs/findings/ants-atropos.md`, register entries, PROVENANCE rows (`larmorx-ants`, `-image`, `-interp`), README and validation-index rows, `docs/api/upstream-defects.md` rows.

### Parity (`ants-atropos`)
| Tier | Cases | Pass | Both reject | Expected divergence | Failures | Compared, bit-identical with CORE-MATH | Compared, bit-identical as built |
|---|---|---|---|---|---|---|---|
| Standard | 93 | 60 | 25 | 8 | 0 | 56 of 56 | 25 of 56 |
| Full | 97 | 64 | 25 | 8 | 0 | 60 of 60 | – |

- **Against the unmodified binary:** the classified images are always identical. Posteriors differ in their last bits where glibc's `expf`/`logf` are not correctly rounded.
- **sMRIPrep's call on the five oracle runs:**
  - bit-identical with CORE-MATH on all five;
  - against ANTs as built, 11 to 4,175 of 5.9 to 17.5 million values of a posterior image differ, by at most 1.2e-7 (ds001600: none);
  - fMRIPrep's own output files equal the repository's ANTs build.
- **Prior-based segmentations** (antsCorticalThickness-style, OASIS30ANTs CSF/GM/WM priors, and all six in the full tier) are bit-identical with CORE-MATH.
- **ANTs' own variability:**
  - its thread band is zero (1, 2, 4 and 8 threads give the same bytes);
  - with the clock's seed, two runs on ds000005 differ in 343 labels (Dice 0.99942) and in posteriors by up to 0.28;
  - larmorx's `-r 1` runs sit as far from them (294 and 327 labels).
- **Expected divergences:** ANTs aborts or crashes (unconvertible values, `--icm [1,2]`, an unknown output extension, ANTS-105's crash) and its clock-seeded runs.

### Findings (register: ITK-50, ITK-51, ANTS-90 to ANTS-107)
- **ITK-50:** ITK's k-means drops the k-d tree's split points. It is met on every sMRIPrep run; the correction changes 1 to 25 of 0.8 to 2.5 million brain-voxel labels and posteriors by at most 0.14, less than ANTs' clock-seed spread.
- **ITK-51:** the Student distribution is NaN above about 340 degrees of freedom, so Grubbs-Rosner never finds an outlier in any image.
- **ANTS-107:** with Grubbs-Rosner from the second iteration, ANTs weighs part of its sample with uninitialized memory. Its result changes when only the output path length changes; larmorx fills those weights with 0 when reproducing.
- **Others:** a multivariate K-means whose second pass is thrown away, Otsu's range bug, outlier filters that keep their previous output, manifold Parzen ignoring weights and doubling covariances, partial-volume classes never re-estimated, vector priors all holding the last component, `-d` always 3D, an unknown `-p` silently running Socrates, `--icm [..]` aborting, an unset coordinate in the joint shape model, and a crash with `-u 1` plus priors plus mask.
- **nipype:** its `Atropos` interface writes the `--icm [..]` form that aborts, and its `icm_use_synchronous_update=True` actually selects asynchronous updating (ICM). Both are in the findings; sMRIPrep sets neither.
- All are corrected by default except ANTS-106.

### Site page (`larmorx_documentation`)
- The clone is at `<scratch>/a6-atropos/site`, branch `a6-atropos-evidence`, squashed to commit `58199ac`, with the no-reply identity. The evidence module is `validation/src/larmorx_validation/evidence/ants_atropos.py`.
- **Example 1, sMRIPrep's Atropos on ds000005:** fMRIPrep's output, ANTs at 1/2/4/8 threads, with CORE-MATH, with the clock's seed (3 runs), fMRIPrep's own ANTs 2.6.2 at 1 thread, and larmorx at 1 and 6 threads in both modes. The chain of causes and every verdict are computed from the outputs; the ITK-50 correction is shown against ANTs' seed band.
- **Example 2:** the OASIS-priors segmentation (classes, grey-matter posterior and prior).
- **Rest of the page:** agreement chart, all 93 cases with verdicts, a status row and a sidebar entry. `evidence.js` gained `u4` (uint32) arrays for the classified images.
- **Checks:**
  - the site builds with 0 problems (62 repository links checked);
  - number check 252/252 agree, viewer check 48/48;
  - re-export changes nothing;
  - page blobs are 26.3 MB, about the budget.
- Screenshots (1440 and 390, light and dark) were reviewed and are in `<scratch>/a6-atropos/shots/final-*`.

### Gates
- **Rust:** both workspaces pass fmt, clippy `-D warnings` and `test --locked`.
- **Python:** ruff, SPDX, and `pytest tests/python tests/golden` (422 passed).
- **Parity smoke:** passed inside `record_golden.py`.
- **Records:** produced from the clean commit `8b74732`.

### Commits
- **larmorx:** branch `worktree-agent-a6259dedbd25d703c`, 15 commits from `c675645` to `d065bb4`, tree clean, no AI attribution.

### For you to merge and decide
- **Shared files I touched** (merge after A5 part 3):
  - `crates/larmorx-py/src/ants_filters/mod.rs`, `_core.pyi`, `python/larmorx/ants/__init__.py`;
  - `crates/larmorx-core/src/defects.rs`: the registry test now allows 3-digit IDs;
  - `crates/larmorx-cli/src/lib.rs`: its "unknown tool" test now uses `KellyKapowski`;
  - `tests/golden/registry.py` and `golden.json`: re-record after both merges;
  - `validation/__main__.py`, `tests/parity/test_ants_programs.py`, `evidence/__init__.py`, `evidence/summary.py`;
  - both upstream-defects documents and the README;
  - on the site: `site.yml`, `data/status.yml`, `summary.json`, `evidence.js`.
- **Left alone to avoid conflicts:** the README and site roadmap line still says "then Atropos". Atropos also needs moving from "Next/Running" in CLAUDE.md and `docs/next-steps.md`. A suggested CLAUDE.md table entry: "Atropos (every option; sMRIPrep's call bit-identical with CORE-MATH on the five oracle runs) | `ants-atropos`".
- **Decision needed: ANTS-106.** Partial-volume classes combined with an MRF never win a voxel, and both modes stop with an error. No correction is defined; one option is to count a partial-volume label as a member of its own set.
- **Approvals:** none were needed.
- **Environment notes:**
  - the missing OASIS CSF, CGM, SCGM and CBM priors (empty placeholders in `oracles/templateflow`) are fetched from TemplateFlow by the suite and checked against recorded SHA-256 sums;
  - `tools/review/shoot.mjs` only works in this sandbox with `--chrome` pointing to a `--no-sandbox` wrapper and a short `TMPDIR` (`/tmp/claude-1000/a6s`).
- **No benchmark was run**, as the brief said.
