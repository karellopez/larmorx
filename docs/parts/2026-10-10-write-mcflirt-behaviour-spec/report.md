# Final report: Write mcflirt behaviour spec

Agent `a15a4b15f33b261f2`, 2026-10-10. The agent's report to the orchestrator, verbatim.

## What I did

I wrote the mcflirt behaviour spec and the source-derived findings, added a synthetic test-data generator, and ran FSL 6.0.7.17's `mcflirt` as an oracle in 137 cases (all with the expected exit codes, 58 of them on real BOLD).

**Committed in the larmorx worktree:** commit `fbc165b` on branch `worktree-agent-a15a4b15f33b261f2`, authored as karellopez with the no-reply email, no AI attribution, not pushed. It contains:
- `specs/mcflirt.md` (new): the spec.
- `docs/findings/fsl-mcflirt.md` (new): source-derived notes with file:line references. It says at the top that the clean-room implementer must not open it. Also added a row to `docs/findings/README.md`.
- `scripts/run_mcflirt_oracle.py` (new): the oracle driver. It has the SPDX header and passes ruff.
- `upstream.tsv` and `UPSTREAM.md`: rows for the FSL conda packages (fsl-mcflirt 2111.0, newimage 2203.12, miscmaths 2203.2, flirt 2111.2, newnifti 4.1.0, utils 2203.5, znzlib 2111.0, armawrap 0.6.0) with conda builds and SHA-256s.
- `scripts/bootstrap_workspace.sh`: a one-line change so it skips the new `fsl` location instead of failing on it.

**Not committed in `larmorx-testdata`.** The harness refuses any git command outside my worktree. Generation, `--check`, ruff and pytest (10 passed) are all fine. Files to commit there:
- `generators/hmc_synthetic.py`
- `data/synthetic/hmc/**` (33 files, 7.8 MB, smoke tier)
- `catalog/synthetic-hmc.toml`
- `catalog/index.json` (regenerated)
- `docs/sources/synthetic-hmc.md`
- `docs/sources/README.md` (link added)

## Spec outline (`specs/mcflirt.md`)
Header: versions, the "must not open" list (FSL source, `crates-nc/`, `docs/findings/fsl-mcflirt.md`) and the importance tiers.
1. Purpose.
2. How fMRIPrep uses it (tier 1). The command is `mcflirt -in X -out <stem>_mcf.nii.gz -reffile R -mats`. Only the `.mat` files are used, then converted to ITK and the confounds are recomputed from them.
3. Inputs and parameters.
4. Coordinates: how images are read, the radiological flip, FSL-mm, the meaning of `.mat`, world/ITK formulas, the Euler convention and its decomposition.
5. Schedule: the reference, stages 8/4/4(/4 sinc) mm with tolerance factors 0.8/0.8/0.1, unblurred reference grids, previous-volume initialisation.
6. Cost function: sampling, trilinear interpolation, edge weighting, and normalised correlation as mcflirt actually computes it.
7. Optimiser: a single sweep of coordinate line searches, with the bracketing and refinement rules, parabola vertex and constants.
8. `-meanvol`.
9. Final resampling: edges, background fill, interpolation options, truncation of the output values.
10. Outputs: image type and header, `.mat` text format, `.par`, `.rms`, `-stats`, file naming.
11. Command line and exit codes.
12. Edge cases, including the in-plane mode.
13. Numerics, determinism and speed.
14. Validation plan: metrics, thresholds, variability band.
15. Paper vs implementation.
16. larmorx interfaces.
17. Open questions and the points where exact agreement will be hard.
Appendix A: the other cost functions.

**Open questions:**
- Whether a double-precision implementation stays within the thresholds has to be measured.
- The perturbation runs that would give a variability band have not been made.
- `-spline_final` boundary handling is described from analysis only.
- The histogram costs are described only in outline.
- A second FSL build is needed for the band.
- Whether a better optimiser should become the default (D11).

## Most important findings
- **The "normcorr" cost is effectively an uncentred correlation.** Its weight count is a running sum of running sums, about 10⁴ times too large, so the means it subtracts are negligible. Pearson's correlation is used only with `-smooth 0`.
- **The coarse stages subsample only the reference, without blurring.** The test volume is always used at full resolution.
- **The optimiser is one sweep of six line searches per stage,** despite the function being called `powell_opt`. Results are quantised by the line-search tolerances.
- **The last volume is never initialised from the previous one** (an off-by-one).

## Confirmed by black-box runs
- **Determinism:** three runs of fMRIPrep's command give byte-identical outputs (241 files).
- **Storage doesn't matter:** LAS vs x-reversed RAS, oblique affines and int16/float32/float64/uint16/scaled storage of the same values give bit-identical matrices.
- **Conventions:** the Euler convention, and the `.par` centre being the reference's centre of mass with the minimum subtracted.
- **Formulas and order:** the `.rms` formulas and means; the reference index N/2 with an identity matrix; the visiting order; the stage tolerances and `-rotation`.
- **Initialisation:** six identical displaced volumes give matrices 0 and 5 equal and 1–4 different; with `-fudge` all six are equal.
- **Uncentred correlation (ramp test):** a ramp shifted along x is realigned by default but not with `-smooth 0`.
- **Output values:** the background fill is the 10th percentile of a 2-voxel border; int16 output is truncated, not rounded; the output-type mapping holds.
- **Header:** slice fields are zeroed; `xyzt_units` is forced to mm/s while a TR in milliseconds is copied unchanged; output is little-endian.
- **Thin slabs:** in-plane mode switches on for a 16 mm slab.
- **NaN:** one NaN voxel gives a NaN matrix for that volume only.
- **`-init`:** affects only the resampling, not the matrices.
- **Failures:**
  - a reference with a different grid size aborts after estimation, with nothing written;
  - out-of-range `-refvol` aborts;
  - `mcflirt -help` alone exits 1;
  - `-gdt` with an absolute `-out` path aborts;
  - `-cost woods` fails on BOLD (about 90 mm errors);
  - an unset `FSLOUTPUTTYPE` fails after the `.mat` files are written.

## Oracle data
- **Location:** `<workspace>/oracles/fsl-6.0.7/mcflirt/`, 1.8 GB, outside git. It holds `manifest.json`, one `runs/<case>/run.json` per case (exact argv, environment, exit codes, times, SHA-256s), the median-of-10 references in `refs/`, the analysis scripts and their outputs in `analysis/`, and a README. The log is `oracles/logs/fsl-6.0.7-mcflirt-oracle-real.log`.
- **ds000005 run 1**, with fMRIPrep 21.0.1's published boldref: fMRIPrep's command, two repeats, and one run per option (26 options plus `-refvol 0` and `-meanvol` without a reference).
- **Eight more real runs**, each with fMRIPrep's command, with the reports added, and with defaults: ds000117, ds000122, ds000210 (echo 2, neurological), ds000258 (big-endian), ds003345, ds005040, ds006010 (uint16), ds006736. ds003763 (truncated as published) is an error case.
- **All synthetic hmc series**, plus option, naming and command-line cases.

**Accuracy and spread on that data:**
- mcflirt against the synthetic ground truth: rotation errors up to 0.7°, translation errors up to 0.14 mm.
- FSL 6.0.5.1's published ds000005 confounds against our 6.0.7.17 run: a near-constant offset of up to 0.16 mm plus a per-volume scatter of 0.004–0.012 (standard deviation, mm or degrees). The reference used for that published run is unknown, so this is only an upper bound on the version effect.

## What a clean-room implementer will find hard to match
- **Error propagation:** the stage-1 chain carries early differences along the whole series.
- **Float32 noise:** stage 3 decides between points whose cost differences are at the float32 rounding level. That depends on the three-level float32 sums and on reproducing the inflated count exactly.
- **Sampling positions:** they are built incrementally in float32, and that decides which points enter the sums.
- **Rounding between stages:** angles pass through float32 between stages, and rotations are orthogonal only to about 1e-8.
- **Reference grids:** their sizes and step accumulation are float32-sensitive.
- **Quantised zeros:** very still subjects get exact zero parameters (100 of 240 matrices in ds000005).
- **Parallelism:** stage 1 is sequential within a pass; only stages 2–4 can run in parallel over volumes.

## Other notes
- I deleted my own first timing run (`oracles/fsl-6.0.7/mcflirt/_scratch`), a temporary duplicate of `real/ds000005/fmriprep`.
- `CLAUDE.md`'s status section does not mention the new spec yet; I left it unchanged.
- `docs/api/mri-hmc.md`, named in the spec's interface section, does not exist yet.
