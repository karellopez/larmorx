# FLIRT's spec answers (2026-11-02): the models and their FSL runs

The spec writer's answers to the questions of FLIRT's clean-room original, part 1
(`specs/flirt.md` §14, "The clean-room questions of 2026-11-02"). Each answer that states a
precision is proved by a model written from the spec's new text alone, which reproduces what FSL
6.0.7.23 printed. The models read the inputs of the `fsl-flirt-replica` suite
(`<workspace>/oracles/inputs/fsl-flirt-replica/`, made from ds005040's fMRIPrep run) and FSL's
outputs of `fsl_runs.py`. Run everything inside `scripts/orchestration/boxed.sh` (8 GiB is
plenty), with `PYTHONPATH=python:validation/src` and `python -P`.

| File | What it shows | How to run |
|---|---|---|
| `fsl_runs.py` | FSL's runs, each through the oracle-output cache (computed once). Sub-task 3 (the refusals and leads): `default12`, `zero-refweight` (and `-6`, `-bbr`), `zero-inweight` (derived all-zero weights), `onevox-in`, `-ref`, `-both`, `zero-voxsize` (a derived BOLD reference with a z voxel size of 0 in its header: FSL never finishes, so it ends at the cache's 60-minute timeout and is never stored). Sub-task 2 (questions 6 to 20): `quat`, `usesqform`, `isoxfm-setbg` (the suite's cases), `sinc`, `isoxfm-oblique` (a derived T1w with an oblique qform and no sform), `fixscaleskew`, `inverse-noz`, `rotation` (see its docstring). Sub-task 1: the boundary-based cost of `bbr/default`'s initial matrix (`bbrcost*`: `bbrstep` 1, 2, 200, 201; default, `-noclamp`, `local_abs`, `global_abs`), `real/dof6` with `-verbose 7` and with `-debug`, `corratio_costs.sch` on the BOLD reference and on the BOLD reference minus 30000 (`woods-neg`, a derived input in `<workspace>/oracles/inputs/flirt-spec-answers/`) | `fsl_runs.py <runs dir> [name ...]` |
| `bbr_model.py` | questions 4 and 5 (§6.5): the boundary points, the Gaussian of σ = 2 mm, the gradient's 3, 1, 1/3 weights, the normals, the clamping (§4.1), the cost and `-bbrtype`: FSL's 16 printed costs | `bbr_model.py <runs dir>` |
| `corratio_model.py` | questions 1 to 3 (§6.2, §8.1, `specs/mcflirt.md` appendix A): the correlation ratio's per-bin sums, the sparse bins, the two final roundings, Woods' terms, the centres of mass; the search's first sweep at 8 mm on `real/dof6` (23 evaluations, FSL's printed costs and parameters) on the 8 mm pair that `-debug` saved, and six `corratio` and `woods` values on each of two images. `--variant single-final`, `double-sums`, `appendix-a` and `--woods n-sigma-mu` show where each other reading parts from FSL | `corratio_model.py <runs dir> [--variant ...] [--woods ...]` |
| `matrices_model.py` | questions 9, 11, 15, 16 (§3's composition and decomposition, §4.1's left-right swap of a neurological file with its single-precision field of view, §9.3's written header, §12's 4 × 4 inverse): FSL's text of `convert_xfm -inverse` (its `-0`s) and `-fixscaleskew`, `-usesqform`'s matrix, the quaternion, offset and sform fields of `-applyisoxfm`'s output, and the linear part of a composition of double angles. `--variant double-fov`, `plain-rotation` and `divide` show where each other reading parts | `matrices_model.py <runs dir> [--variant ...]` |
| `resample_model.py` | questions 13 and 14 (§9.2's windowed sinc, §9.3's blur and spline with `-setbackground`): 40000 sampled voxels of `-applyxfm -interp sinc` and all 172800 values of `real/applyisoxfm-4d-blur-setbackground`, bit for bit; `--variant double-sums`, `kernel-order`, `divide-table` (the sinc) and the two other readings of the spline (printed with `--spline`) part from FSL | `resample_model.py <runs dir> [--variant ...]`, `resample_model.py <runs dir> --spline` |
| `replica_runs.py` | sub-task 3 (FSL-90 to FSL-93, `specs/flirt.md` §11): the FLIRT replica in both D13 modes on `fsl_runs.py`'s named runs (`reproduce`: FSL's matrix and standard error; the default: its refusal or its matrix's RMS deviation from FSL's and, with `--against`, from another run's) | `replica_runs.py <runs dir> name ... [--against name]` (after `fsl_runs.py`) |
| `corratio_costs.sch` | the schedule of the `corratio-mc` and `woods-neg` runs (an identity and a translation, given to `setrow` as columns) | passed to FLIRT by `fsl_runs.py` |

What the models gave (2026-11-02): `bbr_model.py` 16 of 16 printed costs equal (0.537036,
0.540203, 0.566266, 0.554291 for the default); `corratio_model.py` 23 of 23 evaluations of the
first sweep and 12 of 12 cost values equal. The other readings part from FSL: at the 5th
evaluation (`single-final`: the cost without its two roundings), at the 2nd (`double-sums`), on
the cost values (`appendix-a`, sparse bins out of the bins' sum only: 0.982248 for 0.984415;
`n-sigma-mu`, `n_b·σ_b` for a bin with a non-positive sum: 20844 for 37174512).

What the models of sub-task 2 gave (2026-11-02): `matrices_model.py` 5 of 5 FSL outputs equal (the
inverse's text with its two `-0`s, `-fixscaleskew`'s ten digits, `-usesqform`'s `-13.74984741`, the
twelve header fields of `-applyisoxfm` on an oblique neurological reference, a composition of the
double angles 0.1, -0.25, 0.3); `double-fov` parts on `-usesqform` (`-13.74984288`) and the sform
(1.803835 for 1.803831), `plain-rotation` on the composition (1.018201155 for 1.018201124).
`resample_model.py`: 40000 of 40000 sinc voxels and 172800 of 172800 spline values bit-identical;
`double-sums` 3767, `kernel-order` 20447 and `divide-table` 24688 of the 40000; the blur reading
zeros 129359 and the edge coefficients repeated 167146 of the 172800.

What sub-task 3 measured (2026-11-02; FSL 6.0.7.23, the replica identical in `reproduce` on every
run): `-anglerep quaternion`'s matrix 0.3685 mm (RMS deviation, 80 mm) from `-anglerep euler`'s,
the corrected search's 0.2333 mm (0.192 mm from FSL's); an all-zero `-refweight` gives a rotation
of 95.5 degrees (identical at 6 and 12 dof), an all-zero `-inweight` 142.6 degrees, with `bbr` 2339
NaN warnings; a 1 × 1 × 1 input, reference or both 130.9, 95.5 and 142.6 degrees; a header voxel
size of 0: FSL still running after 60 minutes (the replica's trace: infinite voxel sizes, a NaN
initial matrix, a reference level of −2147483648 voxels per axis).
