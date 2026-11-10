# Reports for the upstream projects

Drafts of bug reports for the tools larmorx replicates. Each one is a defect found while
porting or validating, written up so it can be filed with the upstream project. The full list of
defects, with what larmorx does about each, is the register,
[findings/upstream-defects.md](../findings/upstream-defects.md); a report is written only where
the defect affects that project's users and the evidence is solid enough to file.

A report stays here after it is filed, with the issue's URL added, so the evidence stays with
the project.

Where the project already has an issue for the defect, the draft is a comment for that issue
(its first line says which). Each draft's reproducer is in the repository with the outcomes it
printed (the field maps' part B's: `validation/investigations/sdcflows-fieldmap-b/relay32/`).

| Report | Project | Register | Status |
|---|---|---|---|
| [fmriprep-resampler-openblas-race.md](fmriprep-resampler-openblas-race.md) | fMRIPrep | PIPE-02 | draft, not yet filed |
| [fmriprep-jacobian-wrong-axis.md](fmriprep-jacobian-wrong-axis.md) | fMRIPrep | PIPE-03 | draft, not yet filed; confirmed on a real PEPOLAR run (ds005040) and a phase-difference run with the Jacobian forced (ds005454), 2026-10-12 |
| [sdcflows-shared-b0fieldidentifier.md](sdcflows-shared-b0fieldidentifier.md) | sdcflows, fMRIPrep | PIPE-32, SDC-36 | draft of a comment for the open sdcflows#455 (fMRIPrep#3354 is the same failure), not yet posted: a minimal reproducer and a contained fix, 2026-11-10 |
| [sdcflows-entities-from-parent-folders.md](sdcflows-entities-from-parent-folders.md) | sdcflows | SDC-45 | draft, not yet filed, 2026-11-10 |
| [fmriprep-entities-from-parent-folders.md](fmriprep-entities-from-parent-folders.md) | fMRIPrep | PIPE-36 | draft, not yet filed, 2026-11-10 |
| [pybids-entities-from-parent-folders.md](pybids-entities-from-parent-folders.md) | pybids | PIPE-37 | nothing to file: reported as pybids#1270, fixed on its master by #1273 (2026-08-24), not yet released; measured against that fix, 2026-11-10 |
| [scipy-rank-filter-1d-output-type.md](scipy-rank-filter-1d-output-type.md) | SciPy | SCI-21 (with SCI-22, SCI-23) | draft, not yet filed, 2026-11-10 |
| [scipy-binary-erosion-float-output-mask.md](scipy-binary-erosion-float-output-mask.md) | SciPy | SCI-25 | draft, not yet filed, 2026-11-10 |
| [scipy-label-bool-output.md](scipy-label-bool-output.md) | SciPy | SCI-29 | draft of a comment for the open scipy#24502, not yet posted, 2026-11-10 |
