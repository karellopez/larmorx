# Parity: tedana t2smap: the clean-room original against tedana 26.0.3 and the replica

**All cases agree.** 119 cases on the `full` tier: 114 pass, 0 rejected by both, 5 expected divergences, 0 failures.

- **Validated:** tedana t2smap (clean-room original)
- **Reference:** tedana 26.0.3's t2smap (oracle cache); larmorx-gpl's replica by default
- **Test data:** larmorx-testdata `52e371aab0ac`, tier `full`
- **Generated:** 2026-11-11 on Linux x86_64, with `python -m larmorx_validation parity tedana-t2smap-original --tier full`
- **Original programs:** 119 runs: 116 from the oracle cache, 3 run (2 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

`ds003592-loglin`: (A) 11 rows pass [11299199/11299200 values identical]; (B) 11 rows pass [11299200/11299200 values identical]
`ds003592-curvefit`: (A) 13 rows pass [11381033/11406278 values identical]; (b)/(c) share 0.305%; (B) 13 rows pass [11380746/11406278 values identical]; (b)/(c) share 0.259%
`ds003592-curvefit-verbose`: (A) 16 rows pass [11486292/11569034 values identical]; (b)/(c) share 0.305%; (B) 16 rows pass [11485187/11569034 values identical]; (b)/(c) share 0.259%
`ds000210-cuedSGT-curvefit`: (A) 13 rows pass [9430950/9436299 values identical]; (b)/(c) share 0.000%; (B) 13 rows pass [9430980/9436299 values identical]; (b)/(c) share 0.000%
`ds000210-rest-curvefit`: (A) 13 rows pass [13935546/13946060 values identical]; (b)/(c) share 0.000%; (B) 13 rows pass [13935632/13946060 values identical]; (b)/(c) share 0.002%
`ds000258-curvefit`: (A) 13 rows pass [7311090/7342822 values identical]; (b)/(c) share 0.124%; (B) 13 rows pass [7312250/7342822 values identical]; (b)/(c) share 0.076%

## Thresholds

| Quantity | Requirement |
|---|---|
| exit status, standard output, files | identical |
| messages | identical but Python warnings' location lines (§12.3) and the cap's DEBUG digits |
| notices (B) | the same defects named in the same words and counts (NP-03, GLIBC-07 aside; the cap's decimals by §17.3's cap row) |
| values | specs/t2smap.md §17.3 by its check.py; file by file where its model cannot read the inputs |
| threads | every output identical at 1, 4 and 8 threads |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| fmriprep | 6 | 6 | 0 | 0 | 0 |
| probe | 111 | 106 | 0 | 5 | 0 |
| threads | 2 | 2 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `three-d` | the replica prints only the last of tedana's two chained tracebacks for a 3D echo file (the original prints both, as tedana) |
| `x-mask-grid` | the replica prints only nilearn's error where tedana chains it after its own 'Image/mask shape mismatch' (the original prints both, as tedana) |
| `x-underflow-debug` | the replica does not show numpy's cast overflow again for the limited S0 map after the confounds table, where tedana's warnings registry has been reset (observed; the original shows it, as tedana) |
| `x-underflow-debug-curvefit` | SciPy's own floating-point warnings inside its trust-region and finite-difference steps (overflow in a scalar multiply and a dot product, invalid subtractions) are not reproduced: the original's solver is its own (§12.3 lists the curve fit's OptimizeWarning only); one voxel whose SciPy fit overflow… |
| `x-inf-both-paid` | the replica does not show numpy's 'invalid value encountered in reduce' of the adaptive mask's means of a series holding both infinities (tedana does: observed; the original does too) |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.1.0rc1 (9c34af60d378) |
| larmorx-testdata | 52e371aab0ac |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| tedana | 26.0.3 |
| scipy | 1.15.3 |
| nibabel | 5.4.2 |
| nilearn | 0.14.1 |

## Notes

(A) is the original's reproduce against tedana, (B) its default against the replica's default (a black box). The fitted numbers of reproduce are the original's own solvers' within §17.3, not bit for bit (decision of 2026-11-11).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `help` | pass | 9/9 | -h |
| `version` | pass | 9/9 | -v |
| `no-arguments` | pass | 9/9 | (no arguments) |
| `bad-choice` | pass | 9/9 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --fittype exp |
| `missing-file` | pass | 9/9 | -d <I>/none.nii.gz -e 0.012 |
| `bad-int` | pass | 9/9 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --dummy-scans 1.5 |
| `loglin` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz |
| `loglin-ms` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 12 28 44 --mask <I>/mask.nii.gz |
| `te-mixed` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 28 44 --mask <I>/mask.nii.gz |
| `te-negative` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e -0.012 0.028 0.044 --mask <I>/mask.nii.gz |
| `te-inf` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 12 28 inf --mask <I>/mask.nii.gz |
| `te-count` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 --mask <I>/mask.nii.gz |
| `curvefit` | pass | 15/15 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --fittype curvefit |
| `curvefit-verbose-orig-prefix` | pass | 15/15 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --fittype curvefit --verbose -… |
| `ts-loglin` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --fitmode ts |
| `ts-curvefit-verbose` | pass | 15/15 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --fitmode ts --fittype curvefi… |
| `paid` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --combmode paid |
| `paid-ts` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --combmode paid --fitmode ts |
| `no-mask` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 |
| `masktype-decay` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --masktype decay |
| `masktype-none` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --masktype none |
| `masktype-both` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --masktype dropout decay |
| `dummy-scans` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --dummy-scans 2 |
| `dummy-scans-all` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --dummy-scans 6 |
| `exclude` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --exclude 0,2:4 |
| `exclude-dummy` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --exclude 0,1,4 --dummy-scans 2 |
| `exclude-dummy-all` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --exclude 0,1 --dummy-scans 2 |
| `exclude-step` | pass | 9/9 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --exclude 0:4:2 |
| `exclude-open` | pass | 9/9 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --exclude 2: |
| `exclude-negative` | pass | 9/9 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --exclude -1 |
| `exclude-negative-range` | pass | 9/9 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --exclude -2:1 |
| `exclude-text` | pass | 9/9 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --exclude a |
| `exclude-large` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --exclude 7 |
| `exclude-equal` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --exclude 6 |
| `exclude-all` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --exclude 0:6 |
| `exclude-ts` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --exclude 1 --fitmode ts |
| `zcat` | pass | 13/13 | -d <I>/zcat.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz |
| `int16` | pass | 13/13 | -d <I>/i16-e1.nii.gz <I>/i16-e2.nii.gz <I>/i16-e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz |
| `n-independent-3` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --n-independent-echos 3 |
| `n-independent-4` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --n-independent-echos 4 |
| `n-independent-0` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --n-independent-echos 0 |
| `debug-curvefit` | pass | 15/15 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --fittype curvefit --debug |
| `quiet` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --quiet |
| `exists` | pass | 26/26 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz |
| `overwrite` | pass | 26/26 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz -f |
| `nan-sample` | pass | 13/13 | -d <I>/nan-e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz |
| `nan-sample-decay` | pass | 13/13 | -d <I>/nan-e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --masktype decay |
| `inf-sample` | pass | 13/13 | -d <I>/e1.nii.gz <I>/inf-e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz |
| `one-echo` | pass | 13/13 | -d <I>/e1.nii.gz -e 0.012 --mask <I>/mask.nii.gz |
| `two-echoes` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz -e 0.012 0.028 --mask <I>/mask.nii.gz |
| `three-d` | expected-divergence | 13/13 | -d <I>/e1-3d.nii.gz <I>/e1-3d.nii.gz <I>/e1-3d.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz |
| `mask-shifted` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask-shifted.nii.gz |
| `abbreviations` | pass | 15/15 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask=<I>/mask.nii.gz --fitt curvefit --comb=paid |
| `threads` | pass | 15/15 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --fittype curvefit --n-threads… |
| `x-nan-echo2` | pass | 13/13 | -d <I>/e1.nii.gz <I>/nan-e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz |
| `x-inf-echo1` | pass | 13/13 | -d <I>/inf-e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz |
| `x-cap-zero` | pass | 13/13 | -d <I>/e1.nii.gz <I>/neg-e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz |
| `x-one-volume` | pass | 13/13 | -d <I>/one-e1.nii.gz <I>/one-e2.nii.gz <I>/one-e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz |
| `x-paid-one-volume` | pass | 13/13 | -d <I>/one-e1.nii.gz <I>/one-e2.nii.gz <I>/one-e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --combmode paid |
| `x-paid-constant` | pass | 13/13 | -d <I>/const-e1.nii.gz <I>/const-e2.nii.gz <I>/const-e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --combmode p… |
| `x-single-voxel` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask-one.nii.gz |
| `x-mask-4d` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask-4d.nii.gz |
| `x-mask-grid` | expected-divergence | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask-grid.nii.gz |
| `x-n-independent-negative` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --n-independent-echos -1 |
| `x-dummy-negative` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --dummy-scans -1 --exclude 0 |
| `x-te-single-inf` | pass | 13/13 | -d <I>/e1.nii.gz -e inf --mask <I>/mask.nii.gz |
| `x-debug-ts` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --fitmode ts --debug |
| `x-help-40` | pass | 9/9 | -h |
| `x-curvefit-decay` | pass | 15/15 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --masktype decay --fittype cur… |
| `x-ts-curvefit-decay` | pass | 15/15 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --masktype decay --fittype cur… |
| `x-one-volume-curvefit` | pass | 15/15 | -d <I>/one-e1.nii.gz <I>/one-e2.nii.gz <I>/one-e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --fittype curvefit… |
| `x-inf-sample-curvefit` | pass | 15/15 | -d <I>/e1.nii.gz <I>/inf-e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --fittype curvefit --debug… |
| `x-two-echoes-ts-curvefit` | pass | 15/15 | -d <I>/e1.nii.gz <I>/e2.nii.gz -e 0.012 0.028 --mask <I>/mask.nii.gz --fittype curvefit --fitmode ts --verbose |
| `x-curvefit-used-volumes` | pass | 15/15 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --fittype curvefit --dummy-sca… |
| `x-curvefit-no-mask` | pass | 15/15 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --fittype curvefit --debug |
| `x-curvefit-one-echo-voxels` | pass | 15/15 | -d <I>/e1.nii.gz <I>/mixed4-e2.nii.gz <I>/mixed4-e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --masktype dropo… |
| `x-ts-curvefit-one-echo-voxels` | pass | 15/15 | -d <I>/e1.nii.gz <I>/mixed4-e2.nii.gz <I>/mixed4-e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --masktype dropo… |
| `x-cap-zero-curvefit` | pass | 15/15 | -d <I>/e1.nii.gz <I>/neg-e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --fittype curvefit --verbo… |
| `x-ts-curvefit-paid` | pass | 15/15 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --fittype curvefit --fitmode t… |
| `x-exclude-all-none` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --masktype none --exclude 0:6 |
| `x-exclude-all-none-curvefit` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --masktype none --exclude 0:6 … |
| `x-exclude-all-decay` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --masktype decay --exclude 0:6 |
| `x-dummy-all-none` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --masktype none --dummy-scans 6 |
| `x-dummy-all-none-curvefit` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --masktype none --dummy-scans … |
| `x-dummy-all-none-ts` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --masktype none --dummy-scans … |
| `x-epi-board` | pass | 13/13 | -d <I>/board-e1.nii.gz <I>/board-e2.nii.gz <I>/board-e3.nii.gz -e 0.012 0.028 0.044 |
| `x-epi-empty` | pass | 13/13 | -d <I>/slab-e1.nii.gz <I>/slab-e2.nii.gz <I>/slab-e3.nii.gz -e 0.012 0.028 0.044 |
| `x-dummy-all-none-2e` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz -e 0.012 0.028 --mask <I>/mask.nii.gz --masktype none --dummy-scans 6 |
| `x-underflow-debug` | expected-divergence | 13/13 | -d <I>/e1.nii.gz <I>/fast-e2.nii.gz <I>/fast-e3.nii.gz -e 0.012 0.01205 0.0121 --mask <I>/mask.nii.gz --masktype none -… |
| `x-underflow-debug-curvefit` | expected-divergence | 13/13 | -d <I>/e1.nii.gz <I>/fast-e2.nii.gz <I>/fast-e3.nii.gz -e 0.012 0.01205 0.0121 --mask <I>/mask.nii.gz --masktype none -… |
| `x-inf-both-paid` | expected-divergence | 13/13 | -d <I>/e1.nii.gz <I>/both-e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --combmode paid |
| `x-inf-echo3-decay-paid` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/inf-e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --masktype decay --combmod… |
| `x-epi-tiny` | pass | 13/13 | -d <I>/tiny-e1.nii.gz <I>/tiny-e2.nii.gz <I>/tiny-e3.nii.gz -e 0.012 0.028 0.044 |
| `x-cap-zero-one` | pass | 13/13 | -d <I>/e1.nii.gz <I>/negmost-e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz |
| `x-inf-sample-ts` | pass | 13/13 | -d <I>/e1.nii.gz <I>/inf-e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --fitmode ts |
| `x-inf-sample-paid` | pass | 13/13 | -d <I>/e1.nii.gz <I>/inf-e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz --combmode paid |
| `x-te-count-independent` | pass | 13/13 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 --mask <I>/mask.nii.gz --n-independent-echos 3 |
| `x-pair-i16` | pass | 13/13 | -d <I>/p16-e1.hdr <I>/p16-e2.hdr <I>/p16-e3.hdr -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz |
| `x-pair-i16-img` | pass | 13/13 | -d <I>/p16-e1.img <I>/p16-e2.img <I>/p16-e3.img -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz |
| `x-analyze` | pass | 8/8 | -d <I>/a32-e1.hdr <I>/a32-e2.hdr <I>/a32-e3.hdr -e 0.012 0.028 0.044 --mask <I>/a32-mask.hdr |
| `x-analyze-nomask` | pass | 8/8 | -d <I>/a32-e1.hdr <I>/a32-e2.hdr <I>/a32-e3.hdr -e 0.012 0.028 0.044 |
| `x-analyze-mask-4d` | pass | 8/8 | -d <I>/a32-e1.img <I>/a32-e2.img <I>/a32-e3.img -e 0.012 0.028 0.044 --mask <I>/a32-mask4d.hdr |
| `x-analyze-be` | pass | 8/8 | -d <I>/b32-e1.hdr <I>/b32-e2.hdr <I>/b32-e3.hdr -e 0.012 0.028 0.044 --mask <I>/a32-mask.hdr |
| `x-analyze-spm` | pass | 8/8 | -d <I>/s16-e1.hdr <I>/s16-e2.hdr <I>/s16-e3.hdr -e 0.012 0.028 0.044 --mask <I>/s16-mask.hdr |
| `x-analyze-cal` | pass | 8/8 | -d <I>/c16-e1.hdr <I>/c16-e2.hdr <I>/c16-e3.hdr -e 0.012 0.028 0.044 --mask <I>/a32-mask.hdr |
| `x-analyze-mat` | pass | 8/8 | -d <I>/mat-e1.img <I>/mat-e2.img <I>/mat-e3.img -e 0.012 0.028 0.044 --mask <I>/mask.nii.gz |
| `x-mgh` | pass | 8/8 | -d <I>/m-e1.mgz <I>/m-e2.mgz <I>/m-e3.mgz -e 0.012 0.028 0.044 --mask <I>/m-mask.mgz |
| `x-mgh-nomask` | pass | 8/8 | -d <I>/m-e1.mgz <I>/m-e2.mgz <I>/m-e3.mgz -e 0.012 0.028 0.044 |
| `x-mgh-oblique-i16` | pass | 8/8 | -d <I>/mo-e1.mgh <I>/mo-e2.mgh <I>/mo-e3.mgh -e 0.012 0.028 0.044 --mask <I>/mo-mask.mgz |
| `x-mgh-oblique-nomask` | pass | 8/8 | -d <I>/mo-e1.mgh <I>/mo-e2.mgh <I>/mo-e3.mgh -e 0.012 0.028 0.044 |
| `x-mgh-mask-on-nifti` | pass | 8/8 | -d <I>/e1.nii.gz <I>/e2.nii.gz <I>/e3.nii.gz -e 0.012 0.028 0.044 --mask <I>/mn-mask.mgz |
| `threads-loglin` | pass | 5/5 | the default at 1, 4 and 8 threads |
| `threads-curvefit-ts` | pass | 5/5 | the default at 1, 4 and 8 threads |
| `ds003592-loglin` | pass | 13/13 | fMRIPrep 25.2.6's ds003592 call ['--fittype', 'loglin'] |
| `ds003592-curvefit` | pass | 15/15 | fMRIPrep 25.2.6's ds003592 call ['--fittype', 'curvefit'] |
| `ds003592-curvefit-verbose` | pass | 15/15 | fMRIPrep 25.2.6's ds003592 call ['--fittype', 'curvefit', '--verbose'] |
| `ds000210-cuedSGT-curvefit` | pass | 15/15 | fMRIPrep 25.2.6's ds000210-cuedSGT call ['--fittype', 'curvefit'] |
| `ds000210-rest-curvefit` | pass | 15/15 | fMRIPrep 25.2.6's ds000210-rest call ['--fittype', 'curvefit'] |
| `ds000258-curvefit` | pass | 15/15 | fMRIPrep 25.2.6's ds000258 call ['--fittype', 'curvefit'] |
