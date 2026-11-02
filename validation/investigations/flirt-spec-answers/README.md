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
| `fsl_runs.py` | FSL's runs, each through the oracle-output cache (computed once): the boundary-based cost of `bbr/default`'s initial matrix (`bbrcost*`: `bbrstep` 1, 2, 200, 201; default, `-noclamp`, `local_abs`, `global_abs`), `real/dof6` with `-verbose 7` and with `-debug`, `corratio_costs.sch` on the BOLD reference and on the BOLD reference minus 30000 (`woods-neg`, a derived input in `<workspace>/oracles/inputs/flirt-spec-answers/`) | `fsl_runs.py <runs dir> [name ...]` |
| `bbr_model.py` | questions 4 and 5 (§6.5): the boundary points, the Gaussian of σ = 2 mm, the gradient's 3, 1, 1/3 weights, the normals, the clamping (§4.1), the cost and `-bbrtype`: FSL's 16 printed costs | `bbr_model.py <runs dir>` |
| `corratio_model.py` | questions 1 to 3 (§6.2, §8.1, `specs/mcflirt.md` appendix A): the correlation ratio's per-bin sums, the sparse bins, the two final roundings, Woods' terms, the centres of mass; the search's first sweep at 8 mm on `real/dof6` (23 evaluations, FSL's printed costs and parameters) on the 8 mm pair that `-debug` saved, and six `corratio` and `woods` values on each of two images. `--variant single-final`, `double-sums`, `appendix-a` and `--woods n-sigma-mu` show where each other reading parts from FSL | `corratio_model.py <runs dir> [--variant ...] [--woods ...]` |
| `corratio_costs.sch` | the schedule of the `corratio-mc` and `woods-neg` runs (an identity and a translation, given to `setrow` as columns) | passed to FLIRT by `fsl_runs.py` |

What the models gave (2026-11-02): `bbr_model.py` 16 of 16 printed costs equal (0.537036,
0.540203, 0.566266, 0.554291 for the default); `corratio_model.py` 23 of 23 evaluations of the
first sweep and 12 of 12 cost values equal. The other readings part from FSL: at the 5th
evaluation (`single-final`: the cost without its two roundings), at the 2nd (`double-sums`), on
the cost values (`appendix-a`, sparse bins out of the bins' sum only: 0.982248 for 0.984415;
`n-sigma-mu`, `n_b·σ_b` for a bin with a non-positive sum: 20844 for 37174512).
