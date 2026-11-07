# 3dvolreg's spec, checked by a model

`specs/3dvolreg.md` (the behaviour of AFNI 26.3.05's `3dvolreg`, the input of its clean-room
original) checked against AFNI's binary by a model written **from the spec and black-box runs
only**: a clean-room implementer may read and run everything here.

| File | What it is |
|---|---|
| `model.py` | the spec's §4–§8 and §10.2 in numpy, statement by statement: AFNI's grid and parameter conventions, the four-shear resampling with every interpolation method (the decomposition by §5.3's stable formulas, numpy's FFT for Fourier), the FIR blurs, the weight, edging, trimming box and derivative images, the weighted least squares and the iterations, `-twopass` with its coarse shift and rotation searches, the storage conversions, `-1Dfile`, `-1Dmatrix_save` and the maximum displacements. The automasks are larmorx's clean-room `lx.afni.automask(implementation="original")`. Not modelled: `-tshift`, `-rotparent`/`-gridparent`, `-savedisp`, `-weight`, `-wtinp`, the FFT blur (`-twoblur` above 14), the messages |
| `check.py` | runs AFNI's 3dvolreg (a black box, a clean environment) and the model on the same input for each case and prints, per case, the identical `-1Dfile` and `-1Dmatrix_save` lines, the largest parameter difference, the maximum displacements that agree, and the registered volumes' largest difference |

Run it in a memory box, with larmorx built into the worktree (`python/larmorx/_core.abi3.so`):

```bash
export LARMORX_WORKSPACE=<workspace> PYTHONPATH=<worktree>/python OPENBLAS_NUM_THREADS=1
scripts/orchestration/boxed.sh 8 python -P validation/investigations/3dvolreg-spec/check.py <work folder> [case ...]
```

The cases (`CASES` in `check.py`): a synthetic moving series made by `check.py` itself (40 × 44 ×
24 × 5, seeded) with every estimation method, `-final NN`, `-noclip -zpad 0`, `-base 2`,
`-twopass`, fMRIPrep's call, AFNI-86's `-nocoarserot`, the shift-only coarse search; the same
array stored RAS as int16; and fMRIPrep's input re-made from ds000005 run 1
(`<workspace>/oracles/inputs/afni-volreg/fmriprep/ds000005-run-01_sliced.nii.gz`, made by the
parity suite with fMRIPrep's own Python) with the default and fMRIPrep's call.

**Results (2026-11-07):** every parameter within 0.0012 degrees or mm of AFNI's (0.0098 with
`-linear`), the registered volumes within 2.7e-5 of their largest value; the remaining difference
is the shear decomposition's numerics in its nearly singular retry (`specs/3dvolreg.md` §5.3,
"Numerics"). The spec writer also ran the model with the decomposition's double values made
AFNI's (a check not kept here, as it needs AFNI's generated formula): then it is bit-identical to
AFNI on all 12 cases without Fourier, and within numpy's FFT differences (1.2e-6) on the four
Fourier cases, whose `-1Dfile` lines are identical (§15 of the spec).
