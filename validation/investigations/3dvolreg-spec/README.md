# 3dvolreg's spec, checked by a model

`specs/3dvolreg.md` (the behaviour of AFNI 26.3.05's `3dvolreg`, the input of its clean-room
original) checked against AFNI's binary by a model written **from the spec and black-box runs
only**: a clean-room implementer may read and run everything here.

| File | What it is |
|---|---|
| `model.py` | the spec's §4–§8 and §10.2 in numpy, statement by statement: AFNI's grid and parameter conventions, the four-shear resampling with every interpolation method (the decomposition by §5.3's stable formulas, numpy's FFT for Fourier), the FIR blurs, the weight, edging, trimming box and derivative images, the weighted least squares and the iterations, `-twopass` with its coarse shift and rotation searches, the storage conversions, `-1Dfile`, `-1Dmatrix_save` and the maximum displacements. The automasks are larmorx's clean-room `lx.afni.automask(implementation="original")`. Not modelled: `-tshift`, `-rotparent`/`-gridparent`, `-savedisp`, `-weight`, `-wtinp`, the FFT blur (`-twoblur` above 14), the messages |
| `check.py` | runs AFNI's 3dvolreg (a black box, a clean environment) and the model on the same input for each case and prints, per case, the identical `-1Dfile` and `-1Dmatrix_save` lines, the largest parameter difference, the maximum displacements that agree, and the registered volumes' largest difference |
| `afni_run.py` | AFNI's 3dvolreg as a black box through the oracle-output cache (a clean environment, one thread), for the two scripts below |
| `thresholds.py` | (2026-11-08) holds every requirement of the spec's §14.3 against AFNI for the model taking each side of §5.3's exact ties (`model.TIES`: the smallest computed norm, the first and the last of the tied orders), on 15 cases: the parameters per volume and their median, the matrices, the maximum displacements and their deltas, the registered volumes, fMRIPrep's medians, `-savedisp` and `-SAVEDISP` where well conditioned; it also prints what the earlier fixed thresholds would have failed |
| `afni89.py` | (2026-11-08) runs AFNI on the inputs of the spec's §7.3 (constant and half-constant series of several sizes and values, a base constant along one axis, an all-zero weight; with and without `-twopass`, `-edging`, `-twoblur`, `-delta`) and prints its least-squares errors or the parameters it ends with |

Run it in a memory box, with larmorx built into the worktree (`python/larmorx/_core.abi3.so`):

```bash
export LARMORX_WORKSPACE=<workspace> PYTHONPATH=<worktree>/python:<worktree>/validation/src OPENBLAS_NUM_THREADS=1
scripts/orchestration/boxed.sh 8 python -P validation/investigations/3dvolreg-spec/check.py <work folder> [case ...]
# one case per box, several side by side (each about 1 GiB; the ds* cases take some minutes):
printf '%s\n' s1-heptic s1-linear s1-cubic s1-quintic s1-fourier s1-twopass s1-linear-twopass s1-fmriprep \
  syn-heptic syn-linear ds000005-heptic ds000005-linear ds000005-fmriprep ds003592-linear ds003592-fmriprep |
  xargs -P 10 -I{} scripts/orchestration/boxed.sh 3 python -P validation/investigations/3dvolreg-spec/thresholds.py <work folder> {}
python -P validation/investigations/3dvolreg-spec/thresholds.py <work folder> --summary
scripts/orchestration/boxed.sh 4 python -P validation/investigations/3dvolreg-spec/afni89.py <work folder>
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

**Results (2026-11-08):** the remaining difference is the choice among orders whose norms are
equal in exact arithmetic (two axes with the same voxel size), which an evaluation's last bits
decide at every resampling (spec §5.3, "Numerics"). Taking either side of every such tie moves the
parameters by at most 0.013° with heptic, quintic, cubic and Fourier and by up to 0.051° with
`-linear` (the `s1` series, a roll of 2.4°). `thresholds.py`: every requirement of the revised
§14.3 holds for all three tie choices on all 15 cases (0 failures in 45 runs); the earlier fixed
thresholds failed the matrices (1e-5) in every case but fMRIPrep's calls and the maximum
displacements (0.01 mm) with `-cubic` and `-linear`. `afni89.py`: AFNI-89 is reached by an all-zero
weight and, with `-twopass`, by a base that does not vary along an axis of 30 voxels or more when
the linear interpolation gives its value back exactly (100, 37.3, 1 and 7·10⁵ do; 1000 and 123.456
do not, and AFNI then ends with meaningless parameters), as the spec's §7.3 lists.
