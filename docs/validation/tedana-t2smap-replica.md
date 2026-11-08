# Parity: t2smap replica (bit-exact, GNU GPL version 3)

**All cases agree.** 69 cases on the `standard` tier: 44 pass, 25 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx-gpl tedana t2smap` (crate larmorx-gpl-tedana, translated from tedana 26.0.3's source; larmorx-gpl 0.0.1)
- **Reference:** tedana 26.0.3's t2smap in both kept environments: newest (numpy 2.5.4 (OpenBLAS 0.3.34), SciPy 1.18.1) and shared (numpy 2.3.5 (OpenBLAS 0.3.30), SciPy 1.15.3); numpy's OpenBLAS on the Haswell kernels, numpy's AVX-512 kernels off (D2), one BLAS thread; fMRIPrep 25.2.6's own files (tedana 25.0.1) reported
- **Test data:** larmorx-testdata `52e371aab0ac`, tier `standard`
- **Generated:** 2026-11-08 on Linux x86_64, with `python -m larmorx_validation parity tedana-t2smap-replica --tier standard`
- **Original programs:** 155 runs: 104 from the oracle cache, 51 run (16 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - GLIBC-07: 9 case(s) named it, 9 with changed values (largest difference 2.4e+05): `fmriprep/ds000210-sub-02-cuedSGT-curvefit`, `fmriprep/ds000210-sub-02-rest-curvefit`, `fmriprep/ds000258-sub-21262-rest-curvefit`, `fmriprep/ds003592-sub-280-rest-curvefit`, `syn/syn-ts-curve`, `syn/x-verbose-orig-prefix`, `syn/x-verbose-ts`, `difftest/maths-notices-joint` …
  - NP-03: 39 case(s) named it, 38 with changed values (largest difference inf), 7 with other changes (exit N -> N; files ['SNmap.nii.gz', 'TNstarmap.nii.gz', 'dataset_description.json', 'desc-adaptiveGoodSignal_mask.nii.gz']; files ['TNstarmap.nii.gz', 'dataset_description.json', 'desc-optcom_bold.nii.gz', 'desc-tedana_registry.json']; messages): `fmriprep/ds000210-sub-02-cuedSGT-curvefit`, `fmriprep/ds000210-sub-02-rest-curvefit`, `fmriprep/ds000258-sub-21262-rest-curvefit`, `fmriprep/ds003592-sub-280-rest-curvefit`, `fmriprep/ds003592-sub-280-rest-loglin`, `syn/syn-loglin`, `syn/syn-curvefit`, `syn/syn-i16-ms` …
  - TED-01: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit N -> N; files ['SNmap.nii.gz', 'TNstarmap.nii.gz', 'dataset_description.json', 'desc-adaptiveGoodSignal_mask.nii.gz']; messages): `fail/00-ted-01`
  - TED-02: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (messages; the error): `fail/01-ted-02`
  - TED-03: 10 case(s) named it, 0 with changed values (largest difference 0), 10 with other changes (messages; the error): `syn/x-nine-ts`, `syn/x-nine-all`, `syn/x-one-volume`, `syn/x-one-volume-ts`, `syn/x-small-nomask`, `difftest/curvefit-no-volume`, `difftest/dummy-scans-every-volume`, `difftest/empty-mean-no-voxel` …
  - TED-04: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit N -> N; files ['SNmap.nii.gz', 'TNstarmap.nii.gz', 'dataset_description.json', 'desc-adaptiveGoodSignal_mask.nii.gz']; messages): `fail/10-ted-04`
  - TED-05: 10 case(s) named it, 10 with changed values (largest difference 2.4e+05), 1 with other changes (messages): `fmriprep/ds000210-sub-02-cuedSGT-curvefit`, `fmriprep/ds000210-sub-02-rest-curvefit`, `fmriprep/ds000258-sub-21262-rest-curvefit`, `fmriprep/ds003592-sub-280-rest-curvefit`, `fmriprep/ds003592-sub-280-rest-loglin`, `syn/syn-loglin`, `syn/syn-curvefit`, `syn/syn-none` …
  - TED-07: 3 case(s) named it, 3 with changed values (largest difference 1.78e+04): `syn/x-verbose-orig-prefix`, `syn/x-verbose-ts`, `difftest/rmse-infinite-percentiles`
  - TED-08: 5 case(s) named it, 5 with changed values (largest difference inf): `fmriprep/ds000210-sub-02-cuedSGT-curvefit`, `fmriprep/ds000258-sub-21262-rest-curvefit`, `fmriprep/ds003592-sub-280-rest-curvefit`, `syn/syn-inf`, `difftest/rmse-infinite-percentiles`
  - TED-09: 2 case(s) named it, 2 with changed values (largest difference inf), 1 with other changes (messages): `syn/syn-inf`, `difftest/paid-infinite-sample`
  - TED-10: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (files ['desc-adaptiveGoodSignal_mask.nii.gz', 'figures']; messages; the error): `fail/03-ted-10`
  - TED-11: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the error): `fail/09-ted-11`
  - TED-12: 3 case(s) named it, 0 with changed values (largest difference 0), 3 with other changes (files ['desc-adaptiveGoodSignal_mask.nii.gz', 'figures']; files ['figures']; messages; the error): `fail/04-refused`, `fail/05-ted-12`, `fail/06-ted-12`
  - TED-13: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (messages; the error): `fail/08-ted-13`
  - TED-15: 2 case(s) named it, 1 with changed values (largest difference 0.000732), 2 with other changes (exit N -> N; files ['TNstarmap.nii.gz', 'dataset_description.json', 'desc-optcom_bold.nii.gz', 'desc-tedana_registry.json']; messages; the first run's error): `syn/x-paid-const`, `difftest/ted15-first-of-two-runs`
  - TED-17: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit N -> N; files ['SNmap.nii.gz', 'TNstarmap.nii.gz', 'dataset_description.json', 'desc-confounds_timeseries.tsv']; messages): `difftest/ted17-single-voxel`
  - TED-18: 1 case(s) named it, 1 with changed values (largest difference 0.001), 1 with other changes (messages): `difftest/ted18-cap-zero`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status, last error line | the same |
| Messages | tedana's log lines and Python's warnings identical (the run directory masked; the replica's `larmorx-gpl:` lines left out; tqdm's bars not compared) |
| Files written | the same names (an empty directory only the replica's run holds is none: the oracle cache does not replay tedana's empty `figures/`) |
| Every file | bit-identical (images decompressed; `dataset_description.json` but for the host and Python fields, D6) |
| Both environments | each against the replica with its libraries |
| tedana's own step | reported, with its reason (SciPy's trust-region method for a curve fit; LAPACK 3.12.0 -> 3.12.1 in numpy's lstsq for a log-linear fit, now and then) |
| D13 fix pass | a defect named exactly when the default's outputs differ |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| difftest | 18 | 10 | 8 | 0 | 0 |
| fail | 11 | 1 | 10 | 0 | 0 |
| fmriprep | 5 | 5 | 0 | 0 | 0 |
| spread | 3 | 3 | 0 | 0 | 0 |
| syn | 32 | 25 | 7 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `syn/x-nine-ts` | t2smap -d nine-e1 nine-e2 nine-e3 nine-e4 nine-e5 nine-e6 nine-e7 nine-e8 nine-e9 -e 0.006 0.012 0.018000000000000002 0.024 0.03 0.036000000000000004 0.042 0.048 0.054 --fitmode ts --debug | tedana exits 1; the same from the replica. IndexError: index -1 is out of bounds for axis 0 with size 0 |
| `syn/x-nine-all` | t2smap -d nine-e1 nine-e2 nine-e3 nine-e4 nine-e5 nine-e6 nine-e7 nine-e8 nine-e9 -e 0.006 0.012 0.018000000000000002 0.024 0.03 0.036000000000000004 0.042 0.048 0.054 --debug | tedana exits 1; the same from the replica. IndexError: index -1 is out of bounds for axis 0 with size 0 |
| `syn/x-one-volume` | t2smap -d one-e1 one-e2 one-e3 -e 0.013 0.029 0.045 --masktype none | tedana exits 1; the same from the replica. ValueError: The mask is invalid as it is empty: it masks all data. |
| `syn/x-one-volume-ts` | t2smap -d one-e1 one-e2 one-e3 -e 0.013 0.029 0.045 --masktype none --fitmode ts | tedana exits 1; the same from the replica. ValueError: The mask is invalid as it is empty: it masks all data. |
| `syn/x-paid-const` | t2smap -d const-e1 const-e2 const-e3 -e 0.013 0.029 0.045 --mask head-mask --combmode paid | tedana exits 1; the same from the replica. ZeroDivisionError: Weights sum to zero, can't be normalized |
| `syn/x-small-nomask` | t2smap -d small-e1 small-e2 small-e3 -e 0.013 0.029 0.045 | tedana exits 1; the same from the replica. IndexError: index -1 is out of bounds for axis 0 with size 0 |
| `syn/x-exists` | t2smap -d head-e1 head-e2 head-e3 -e 0.013 0.029 0.045 --mask head-mask (run twice) | tedana exits 1; the same from the replica. RuntimeError: File <run>/out/desc-adaptiveGoodSignal_mask.nii.gz already exists. In order to allow overwrite please … |
| `fail/00-ted-01` | TED-01: t2smap -d nan-e1 e2 e3 -e 0.014 0.030 0.046 --mask mask | tedana exits 1; the same from the replica. ValueError: attempt to get argmax of an empty sequence |
| `fail/01-ted-02` | TED-02: t2smap -d e1 e2 e3 -e 0.014 0.030 0.046 --mask mask --exclude 5 | tedana exits 1; the same from the replica. IndexError: index 5 is out of bounds for axis 0 with size 5 |
| `fail/02-refused` | a refused input: t2smap -d e1 e2 e3 -e 0.014 0.030 0.046 --mask mask --exclude 6 | tedana exits 1; the same from the replica. ValueError: The maximum exclude index (6) is greater than the number of timepoints in the data (5). |
| `fail/03-ted-10` | TED-10: t2smap -d e1 e2 e3 -e inf inf inf --mask mask | tedana exits 1; the same from the replica. numpy.linalg.LinAlgError: SVD did not converge in Linear Least Squares |
| `fail/04-refused` | a refused input: t2smap -d e1 e2 -e 0.014 --mask mask | tedana exits 1; the same from the replica. ValueError: Second dimension of data (2) does not match number of echoes provided (tes; 1) |
| `fail/05-ted-12` | TED-12: t2smap -d e1 -e 0.014 --mask mask | tedana exits 1; the same from the replica. IndexError: invalid index to scalar variable. |
| `fail/06-ted-12` | TED-12: t2smap -d e1 -e 0.014 --mask mask --masktype none | tedana exits 1; the same from the replica. ValueError: all the input array dimensions except for the concatenation axis must match exactly, but along dimension… |
| `fail/07-refused` | a refused input: t2smap -d e1 e2 e3 -e 0.014 0.030 --mask mask | tedana exits 1; the same from the replica. ValueError: Second dimension of data (3) does not match number of echoes provided (tes; 2) |
| `fail/08-ted-13` | TED-13: t2smap -d e1 e2 e3 -e 0.014 0.030 0.046 --mask mask --dummy-scans 5 | tedana exits 1; the same from the replica. ValueError: attempt to get argmax of an empty sequence |
| `fail/09-ted-11` | TED-11: t2smap -d e1 e2 e3 -e 0.014 0.030 0.046 --mask mask --n-independent-echos 0 | tedana exits 1; the same from the replica. ZeroDivisionError: division by zero |
| `difftest/curvefit-no-volume` | random testing (2026-11-08): a curve fit of no volume: the first voxel's np.min bound of an empty column raises outside curve_fit's try | tedana exits 1; the same from the replica. IndexError: index -1 is out of bounds for axis 0 with size 0 |
| `difftest/dummy-scans-every-volume` | random testing (2026-11-08): every volume a dummy scan with the decay method: numpy's lstsq of no rows, then pandas' shape error in the confounds table (TED-13 refuses it by default) | tedana exits 1; the same from the replica. IndexError: index -1 is out of bounds for axis 0 with size 0 |
| `difftest/empty-mean-no-voxel` | random testing (2026-11-08): no volume and no voxel: numpy's 'Mean of empty slice' over an empty axis whatever the result's size | tedana exits 1; the same from the replica. IndexError: index -1 is out of bounds for axis 0 with size 0 |
| `difftest/independent-echoes-no-good-voxel` | random testing (2026-11-08): --n-independent-echos with no good voxel: numpy's scalar 0/0 warning | tedana exits 1; the same from the replica. IndexError: index -1 is out of bounds for axis 0 with size 0 |
| `difftest/ted04-other-shape` | random testing (2026-11-08): a mask of another shape: tedana's path raises nilearn's error itself, TED-04 not named | tedana exits 1; the same from the replica. [ 0. 0. 0. 1. ]] |
| `difftest/ted15-first-of-two-runs` | random testing (2026-11-08): a one-volume PAID series run twice without --overwrite: TED-15 is named by the first run, whose error differs from tedana' (the second run then fails alike); the fix pass compares the first runs too | tedana exits 1; the same from the replica. RuntimeError: File <run>/out/sub-01_adaptive_mask.nii.gz already exists. In order to allow overwrite please use the … |
| `difftest/ted16-exclude-every-volume` | random testing (2026-11-08): --exclude removing every volume: tedana fits nothing and writes T2* = 500 ms, S0 = 1 (TED-16, refused by default) | tedana exits 1; the same from the replica. IndexError: index -1 is out of bounds for axis 0 with size 0 |
| `difftest/ted17-single-voxel` | random testing (2026-11-08): a single voxel left for the fit: fit_loglinear's squeezed mask column is 0-dimensional, IndexError (TED-17, fitted by default) | tedana exits 1; the same from the replica. IndexError: too many indices for array: array is 0-dimensional, but 1 were indexed |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (7b9cb9c37eb9) |
| larmorx-testdata | 52e371aab0ac |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

The replica runs as a separate program, the GPL package's (`larmorx-gpl`), compared with tedana in `reproduce` mode (D13); the default `fix` pass is checked against the reproduced run. tedana runs as a program through the oracle-output cache (a launcher per environment and a manifest of its installation).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep/ds000210-sub-02-cuedSGT-curvefit` | pass | 10/10 | fMRIPrep 25.2.6's t2smap call |
| `fmriprep/ds000210-sub-02-rest-curvefit` | pass | 10/10 | fMRIPrep 25.2.6's t2smap call |
| `fmriprep/ds000258-sub-21262-rest-curvefit` | pass | 10/10 | fMRIPrep 25.2.6's t2smap call |
| `fmriprep/ds003592-sub-280-rest-curvefit` | pass | 10/10 | fMRIPrep 25.2.6's t2smap call |
| `fmriprep/ds003592-sub-280-rest-loglin` | pass | 8/8 | fMRIPrep 25.2.6's t2smap call with --fittype loglin |
| `syn/syn-loglin` | pass | 8/8 | t2smap -d f32-e1 f32-e2 f32-e3 f32-e4 -e 0.012 0.028 0.044 0.06 --mask mask |
| `syn/syn-curvefit` | pass | 8/8 | t2smap -d f32-e1 f32-e2 f32-e3 f32-e4 -e 0.012 0.028 0.044 0.06 --mask mask --fittype curvefit |
| `syn/syn-i16-ms` | pass | 8/8 | t2smap -d i16-e1 i16-e2 i16-e3 -e 12 28 44 --mask mask --fittype curvefit |
| `syn/syn-ts-loglin` | pass | 8/8 | t2smap -d f32-e1 f32-e2 f32-e3 -e 0.012 0.028 0.044 --mask mask --fitmode ts |
| `syn/syn-ts-curve` | pass | 8/8 | t2smap -d f32-e1 f32-e2 f32-e3 -e 0.012 0.028 0.044 --mask mask --fitmode ts --fittype curvefit |
| `syn/syn-decay` | pass | 8/8 | t2smap -d f32-e1 f32-e2 f32-e3 f32-e4 -e 0.012 0.028 0.044 0.06 --mask mask --masktype decay |
| `syn/syn-both` | pass | 8/8 | t2smap -d f32-e1 f32-e2 f32-e3 f32-e4 -e 0.012 0.028 0.044 0.06 --mask mask --masktype dropout decay |
| `syn/syn-none` | pass | 8/8 | t2smap -d f32-e1 f32-e2 f32-e3 f32-e4 -e 0.012 0.028 0.044 0.06 --mask mask --masktype none --fittype curvefit |
| `syn/syn-exclude` | pass | 8/8 | t2smap -d f32-e1 f32-e2 f32-e3 f32-e4 -e 0.012 0.028 0.044 0.06 --mask mask --exclude 0,3:5,20 --dummy-scans 2 --fittyp… |
| `syn/syn-nind` | pass | 8/8 | t2smap -d f32-e1 f32-e2 f32-e3 f32-e4 -e 0.012 0.028 0.044 0.06 --mask mask --n-independent-echos 5 --debug |
| `syn/syn-zcat` | pass | 8/8 | t2smap -d zcat -e 0.012 0.028 0.044 --mask mask |
| `syn/syn-mask4d` | pass | 8/8 | t2smap -d f32-e1 f32-e2 f32-e3 -e 0.012 0.028 0.044 --mask mask4d |
| `syn/syn-inf` | pass | 8/8 | t2smap -d inf-e1 nan-e2 f32-e3 -e 0.012 0.028 0.044 --mask mask --fittype curvefit |
| `syn/x-nomask-f32` | pass | 8/8 | t2smap -d head-e1 head-e2 head-e3 -e 0.013 0.029 0.045 |
| `syn/x-nomask-i16-curvefit` | pass | 8/8 | t2smap -d head-i16-e1 head-i16-e2 head-i16-e3 -e 0.013 0.029 0.045 --fittype curvefit |
| `syn/x-nomask-zcat` | pass | 8/8 | t2smap -d head-zcat -e 0.013 0.029 0.045 |
| `syn/x-paid` | pass | 8/8 | t2smap -d head-e1 head-e2 head-e3 -e 0.013 0.029 0.045 --mask head-mask --combmode paid |
| `syn/x-paid-ts` | pass | 8/8 | t2smap -d head-e1 head-e2 head-e3 -e 0.013 0.029 0.045 --mask head-mask --combmode paid --fitmode ts |
| `syn/x-verbose-orig-prefix` | pass | 8/8 | t2smap -d head-e1 head-e2 head-e3 -e 0.013 0.029 0.045 --mask head-mask --fittype curvefit --verbose --convention orig … |
| `syn/x-verbose-ts` | pass | 8/8 | t2smap -d head-e1 head-e2 head-e3 -e 0.013 0.029 0.045 --mask head-mask --fittype curvefit --verbose --fitmode ts |
| `syn/x-nine-ts` | both-error | 10/10 | t2smap -d nine-e1 nine-e2 nine-e3 nine-e4 nine-e5 nine-e6 nine-e7 nine-e8 nine-e9 -e 0.006 0.012 0.018000000000000002 0… |
| `syn/x-nine-all` | both-error | 10/10 | t2smap -d nine-e1 nine-e2 nine-e3 nine-e4 nine-e5 nine-e6 nine-e7 nine-e8 nine-e9 -e 0.006 0.012 0.018000000000000002 0… |
| `syn/x-one-volume` | both-error | 10/10 | t2smap -d one-e1 one-e2 one-e3 -e 0.013 0.029 0.045 --masktype none |
| `syn/x-one-volume-ts` | both-error | 10/10 | t2smap -d one-e1 one-e2 one-e3 -e 0.013 0.029 0.045 --masktype none --fitmode ts |
| `syn/x-paid-const` | both-error | 10/10 | t2smap -d const-e1 const-e2 const-e3 -e 0.013 0.029 0.045 --mask head-mask --combmode paid |
| `syn/x-small-nomask` | both-error | 10/10 | t2smap -d small-e1 small-e2 small-e3 -e 0.013 0.029 0.045 |
| `syn/x-exists` | both-error | 10/10 | t2smap -d head-e1 head-e2 head-e3 -e 0.013 0.029 0.045 --mask head-mask (run twice) |
| `syn/x-overwrite` | pass | 8/8 | t2smap -d head-e1 head-e2 head-e3 -e 0.013 0.029 0.045 --mask head-mask --overwrite (run twice) |
| `syn/x-nine-ts-mask` | pass | 8/8 | t2smap -d nine-e1 nine-e2 nine-e3 nine-e4 nine-e5 nine-e6 nine-e7 nine-e8 nine-e9 -e 0.006 0.012 0.018000000000000002 0… |
| `syn/x-nine-all-mask` | pass | 8/8 | t2smap -d nine-e1 nine-e2 nine-e3 nine-e4 nine-e5 nine-e6 nine-e7 nine-e8 nine-e9 -e 0.006 0.012 0.018000000000000002 0… |
| `syn/x-one-volume-mask` | pass | 8/8 | t2smap -d one-e1 one-e2 one-e3 -e 0.013 0.029 0.045 --masktype none --mask ones-one |
| `syn/x-one-volume-ts-mask` | pass | 8/8 | t2smap -d one-e1 one-e2 one-e3 -e 0.013 0.029 0.045 --masktype none --fitmode ts --mask ones-one |
| `fail/00-ted-01` | both-error | 11/11 | TED-01: t2smap -d nan-e1 e2 e3 -e 0.014 0.030 0.046 --mask mask |
| `fail/01-ted-02` | both-error | 11/11 | TED-02: t2smap -d e1 e2 e3 -e 0.014 0.030 0.046 --mask mask --exclude 5 |
| `fail/02-refused` | both-error | 10/10 | a refused input: t2smap -d e1 e2 e3 -e 0.014 0.030 0.046 --mask mask --exclude 6 |
| `fail/03-ted-10` | both-error | 11/11 | TED-10: t2smap -d e1 e2 e3 -e inf inf inf --mask mask |
| `fail/04-refused` | both-error | 10/10 | a refused input: t2smap -d e1 e2 -e 0.014 --mask mask |
| `fail/05-ted-12` | both-error | 11/11 | TED-12: t2smap -d e1 -e 0.014 --mask mask |
| `fail/06-ted-12` | both-error | 11/11 | TED-12: t2smap -d e1 -e 0.014 --mask mask --masktype none |
| `fail/07-refused` | both-error | 10/10 | a refused input: t2smap -d e1 e2 e3 -e 0.014 0.030 --mask mask |
| `fail/08-ted-13` | both-error | 11/11 | TED-13: t2smap -d e1 e2 e3 -e 0.014 0.030 0.046 --mask mask --dummy-scans 5 |
| `fail/09-ted-11` | both-error | 11/11 | TED-11: t2smap -d e1 e2 e3 -e 0.014 0.030 0.046 --mask mask --n-independent-echos 0 |
| `fail/10-ted-04` | pass | 9/9 | TED-04: t2smap -d e1 e2 e3 -e 0.014 0.030 0.046 --mask mask_aff |
| `spread/ds003592-sub-280-rest-curvefit` | pass | 8/8 | tedana's own spread on fMRIPrep's ds003592 call |
| `spread/syn-curvefit` | pass | 8/8 | tedana's own spread on synthetic echoes (fMRIPrep's form, --fittype curvefit) |
| `spread/syn-loglin` | pass | 8/8 | tedana's own spread on synthetic echoes (fMRIPrep's form, --fittype loglin) |
| `difftest/curvefit-no-volume` | both-error | 10/10 | random testing (2026-11-08): a curve fit of no volume: the first voxel's np.min bound of an empty column raises outside… |
| `difftest/dummy-scans-every-volume` | both-error | 10/10 | random testing (2026-11-08): every volume a dummy scan with the decay method: numpy's lstsq of no rows, then pandas' sh… |
| `difftest/empty-mean-no-voxel` | both-error | 10/10 | random testing (2026-11-08): no volume and no voxel: numpy's 'Mean of empty slice' over an empty axis whatever the resu… |
| `difftest/fresh-header-base-affine` | pass | 8/8 | random testing (2026-11-08): an affine equal to the fresh header's own base affine: nibabel sets the sform and qform of… |
| `difftest/independent-echoes-no-good-voxel` | both-error | 10/10 | random testing (2026-11-08): --n-independent-echos with no good voxel: numpy's scalar 0/0 warning |
| `difftest/maths-notices-joint` | pass | 8/8 | random testing (2026-11-08): a change of one voxel's RMSE that needs both of the other mode's function families: NP-03 … |
| `difftest/nifti2-computed-mask` | pass | 8/8 | random testing (2026-11-08): NIfTI-2 echoes without a mask: the reference stays a Nifti2Image, so nilearn's mask and ev… |
| `difftest/np03-taken-back` | pass | 8/8 | random testing (2026-11-08): numpy's float32 log alone changes values that its float64 exp then takes back: nothing nam… |
| `difftest/paid-infinite-sample` | pass | 8/8 | random testing (2026-11-08): PAID with an infinite sample: numpy's subtract warning in the standard deviation |
| `difftest/qform-signed-zero` | pass | 8/8 | random testing (2026-11-08): a qform-only image: nibabel's get_qform in x87 extended precision sums from +0, so the out… |
| `difftest/rmse-infinite-percentiles` | pass | 8/8 | random testing (2026-11-08): an infinite RMSE: numpy's warnings of nanstd's subtraction and of nanpercentile's interpol… |
| `difftest/shared-percentile-float32` | pass | 8/8 | random testing (2026-11-08): numpy 2.3.5's percentile of float32 data divides q in float32: another dropout rank (the s… |
| `difftest/ted04-other-shape` | both-error | 10/10 | random testing (2026-11-08): a mask of another shape: tedana's path raises nilearn's error itself, TED-04 not named |
| `difftest/ted07-equal-variances` | pass | 8/8 | random testing (2026-11-08): --verbose where both variance maps hold the same values: TED-07 not named |
| `difftest/ted15-first-of-two-runs` | both-error | 10/10 | random testing (2026-11-08): a one-volume PAID series run twice without --overwrite: TED-15 is named by the first run, … |
| `difftest/ted16-exclude-every-volume` | both-error | 10/10 | random testing (2026-11-08): --exclude removing every volume: tedana fits nothing and writes T2* = 500 ms, S0 = 1 (TED-… |
| `difftest/ted17-single-voxel` | both-error | 10/10 | random testing (2026-11-08): a single voxel left for the fit: fit_loglinear's squeezed mask column is 0-dimensional, In… |
| `difftest/ted18-cap-zero` | pass | 8/8 | random testing (2026-11-08): a limited T2* map whose 99.5th percentile is 0: the cap sets every limited value to 0 and … |
