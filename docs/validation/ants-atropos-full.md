# Parity: Atropos

**All cases agree.** 102 cases on the `full` tier: 66 pass, 28 rejected by both, 8 expected divergences, 0 failures.

- **Validated:** `larmorx ants Atropos` / `lx.ants.atropos` (crates larmorx-ants, larmorx-image)
- **Reference:** Atropos from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`, run with one thread
- **Test data:** larmorx-testdata `00b862132e9c`, tier `full`
- **Generated:** 2026-10-23 on Linux x86_64, with `python -m larmorx_validation parity ants-atropos --tier full`

**Bit-identical to the unmodified ANTs program (glibc's math, one thread): 27 of 62 compared cases; the classified image identical in 62 of 62.** The others' posteriors differ in the last bits where glibc's `expf`/`logf`/`exp`/`log`/`pow` are not correctly rounded for a value the run meets (their largest differences and NCC are in the table).
**The cause, confirmed: with those functions correctly rounded (CORE-MATH preloaded into the same ANTs program), 62 of 62 are bit-identical.**
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - ANTS-100: 1 case(s) named it, 1 with changed values (largest difference 2): `priors-vector`
  - ANTS-101: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit N -> N; outputs not written): `unknown-formulation`
  - ANTS-102: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit N -> N; outputs written): `icm-brackets`
  - ANTS-103: 1 case(s) named it, 1 with changed values (largest difference 2): `joint-shape-orientation`
  - ANTS-104: 1 case(s) named it, 1 with changed values (largest difference 0.996): `manifold-parzen-covariance`
  - ANTS-105: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit N -> N; outputs written): `priors-minimize-memory`
  - ANTS-106: 5 case(s) named it, 1 with changed values (largest difference 3), 4 with other changes (exit N -> N; outputs written): `partial-volume-with-mrf`, `partial-volume-phantom-mrf-0.1`, `partial-volume-phantom-mrf-0.3`, `real/ds000005-sub-01/smriprep-partial-volume`, `real/ds005040-sub-002/smriprep-partial-volume`
  - ANTS-90: 4 case(s) named it, 3 with changed values (largest difference 2), 1 with other changes (exit N -> N; outputs written): `kmeans-multivariate`, `log-euclidean`, `joint-shape-orientation`, `grubbs-rosner-multivariate`
  - ANTS-92: 3 case(s) named it, 3 with changed values (largest difference 2): `random`, `clock-seed-random-labels`, `real/ds000005/random`
  - ANTS-94: 5 case(s) named it, 4 with changed values (largest difference 2), 1 with other changes (exit N -> N; outputs written): `box-plot`, `grubbs-rosner`, `grubbs-rosner-outliers`, `grubbs-rosner-multivariate`, `verbose-box-plot`
  - ANTS-95: 2 case(s) named it, 2 with changed values (largest difference 2): `manifold-parzen`, `manifold-parzen-covariance`
  - ANTS-96: 7 case(s) named it, 3 with changed values (largest difference 3), 4 with other changes (exit N -> N; outputs written): `partial-volume-likelihoods`, `partial-volume-with-mrf`, `partial-volume-phantom-mrf-0`, `partial-volume-phantom-mrf-0.1`, `partial-volume-phantom-mrf-0.3`, `real/ds000005-sub-01/smriprep-partial-volume`, `real/ds005040-sub-002/smriprep-partial-volume`
  - ANTS-98: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit N -> N; outputs written): `mask-255`
  - ITK-50: 48 case(s) named it, 36 with changed values (largest difference 3), 7 with other changes (exit N -> N; outputs written): `kmeans-3`, `kmeans-4`, `kmeans-centres`, `kmeans-multivariate`, `histogram-parzen`, `manifold-parzen`, `manifold-parzen-covariance`, `log-euclidean` …
  - ITK-51: 1 case(s) named it, 1 with changed values (largest difference 2): `grubbs-rosner-outliers`

| Case | Output | Difference from ANTs as built |
|---|---|---|
| `kmeans-3` | post02 | 1 of 63360 differ; max \|diff\| 1.82e-12, 1 - NCC 0 |
| `kmeans-4` | post01 | 42 of 63360 differ; max \|diff\| 5.82e-11, 1 - NCC 0 |
| `kmeans-4` | post02 | 36 of 63360 differ; max \|diff\| 2.33e-10, 1 - NCC 1.1e-16 |
| `kmeans-4` | post03 | 242 of 63360 differ; max \|diff\| 5.96e-08, 1 - NCC 1.2e-15 |
| `kmeans-4` | post04 | 16 of 63360 differ; max \|diff\| 3.73e-09, 1 - NCC 2.4e-15 |
| `random` | post01 | 24 of 63360 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `random` | post02 | 32 of 63360 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `random` | post03 | 25 of 63360 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `priors-socrates` | post01 | 38 of 63360 differ; max \|diff\| 3.73e-09, 1 - NCC 0 |
| `priors-socrates` | post02 | 72 of 63360 differ; max \|diff\| 5.96e-08, 1 - NCC 2.2e-16 |
| `priors-socrates` | post03 | 28 of 63360 differ; max \|diff\| 1.16e-10, 1 - NCC 1.1e-16 |
| `priors-plato` | post01 | 41 of 63360 differ; max \|diff\| 3.73e-09, 1 - NCC 2.2e-16 |
| `priors-plato` | post02 | 94 of 63360 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `priors-plato` | post03 | 28 of 63360 differ; max \|diff\| 1.16e-10, 1 - NCC 1.1e-16 |
| `priors-aristotle` | post01 | 55 of 63360 differ; max \|diff\| 2.27e-13, 1 - NCC 1.1e-16 |
| `priors-aristotle` | post02 | 48 of 63360 differ; max \|diff\| 3.55e-15, 1 - NCC 0 |
| `priors-aristotle` | post03 | 44 of 63360 differ; max \|diff\| 8.88e-16, 1 - NCC 0 |
| `priors-sigmoid` | post01 | 8 of 63360 differ; max \|diff\| 1.16e-10, 1 - NCC 0 |
| `priors-sigmoid` | post02 | 17 of 63360 differ; max \|diff\| 7.28e-12, 1 - NCC 0 |
| `priors-sigmoid` | post03 | 25 of 63360 differ; max \|diff\| 1.14e-13, 1 - NCC 0 |
| `priors-euclidean` | post01 | 33 of 63360 differ; max \|diff\| 1.86e-09, 1 - NCC 1.7e-15 |
| `priors-euclidean` | post02 | 33 of 63360 differ; max \|diff\| 5.82e-11, 1 - NCC 0 |
| `priors-euclidean` | post03 | 28 of 63360 differ; max \|diff\| 2.33e-10, 1 - NCC 1.1e-16 |
| `priors-threshold` | post01 | 30 of 63360 differ; max \|diff\| 1.49e-08, 1 - NCC 0 |
| `priors-threshold` | post02 | 40 of 63360 differ; max \|diff\| 5.82e-11, 1 - NCC 0 |
| `priors-threshold` | post03 | 39 of 63360 differ; max \|diff\| 1.16e-10, 1 - NCC 1.1e-15 |
| `priors-vector` | post01 | 23 of 63360 differ; max \|diff\| 1.86e-09, 1 - NCC 3.3e-16 |
| `priors-vector` | post02 | 35 of 63360 differ; max \|diff\| 5.96e-08, 1 - NCC 3.3e-16 |
| `priors-vector` | post03 | 38 of 63360 differ; max \|diff\| 9.31e-10, 1 - NCC 0 |
| `priors-all-outputs` | post01 | 38 of 63360 differ; max \|diff\| 3.73e-09, 1 - NCC 0 |
| `priors-all-outputs` | post02 | 72 of 63360 differ; max \|diff\| 5.96e-08, 1 - NCC 2.2e-16 |
| `priors-all-outputs` | post03 | 28 of 63360 differ; max \|diff\| 1.16e-10, 1 - NCC 1.1e-16 |
| `priors-all-outputs` | lik01 | 5 of 63360 differ; max \|diff\| 4.66e-10, 1 - NCC 0 |
| `priors-all-outputs` | lik02 | 140 of 63360 differ; max \|diff\| 4.66e-10, 1 - NCC 2.4e-15 |
| `priors-all-outputs` | lik03 | 4 of 63360 differ; max \|diff\| 5.29e-23, 1 - NCC 0 |
| `priors-adaptive-smoothing` | post01 | 28 of 63360 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `priors-adaptive-smoothing` | post02 | 49 of 63360 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `priors-adaptive-smoothing` | post03 | 44 of 63360 differ; max \|diff\| 3.73e-09, 1 - NCC 0 |
| `priors-adaptive-smoothing` | lik01 | 2 of 63360 differ; max \|diff\| 4.66e-10, 1 - NCC 3.4e-15 |
| `priors-adaptive-smoothing` | lik02 | 46 of 63360 differ; max \|diff\| 1.86e-09, 1 - NCC 0 |
| `priors-adaptive-smoothing` | lik03 | 16 of 63360 differ; max \|diff\| 5.82e-11, 1 - NCC 0 |
| `label-fast-marching` | post01 | 8 of 63360 differ; max \|diff\| 4.66e-10, 1 - NCC 0 |
| `label-fast-marching` | post02 | 28 of 63360 differ; max \|diff\| 3.73e-09, 1 - NCC 1.1e-15 |
| `label-fast-marching` | post03 | 34 of 63360 differ; max \|diff\| 1.16e-10, 1 - NCC 7.8e-16 |
| `label-euclidean` | post01 | 8 of 63360 differ; max \|diff\| 4.66e-10, 1 - NCC 0 |
| `label-euclidean` | post02 | 28 of 63360 differ; max \|diff\| 3.73e-09, 1 - NCC 1.1e-15 |
| `label-euclidean` | post03 | 34 of 63360 differ; max \|diff\| 1.16e-10, 1 - NCC 7.8e-16 |
| `label-propagation` | post01 | 32 of 63360 differ; max \|diff\| 3.73e-09, 1 - NCC 2.2e-16 |
| `label-propagation` | post02 | 62 of 63360 differ; max \|diff\| 2.98e-08, 1 - NCC 2.3e-15 |
| `label-propagation` | post03 | 57 of 63360 differ; max \|diff\| 1.49e-08, 1 - NCC 0 |
| `label-propagation-per-label` | post01 | 31 of 63360 differ; max \|diff\| 5.96e-08, 1 - NCC 1e-15 |
| `label-propagation-per-label` | post02 | 36 of 63360 differ; max \|diff\| 5.96e-08, 1 - NCC 1.2e-15 |
| `label-propagation-per-label` | post03 | 77 of 63360 differ; max \|diff\| 7.45e-09, 1 - NCC 4.4e-16 |
| `manifold-parzen-covariance` | post01 | 1 of 63360 differ; max \|diff\| 1.86e-09, 1 - NCC 0 |
| `manifold-parzen-covariance` | post02 | 1 of 63360 differ; max \|diff\| 1.12e-08, 1 - NCC 1.3e-15 |
| `log-euclidean` | post01 | 187 of 63360 differ; max \|diff\| 3.54e-08, 1 - NCC 0 |
| `log-euclidean` | post02 | 145 of 63360 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `log-euclidean` | post03 | 232 of 63360 differ; max \|diff\| 5e-16, 1 - NCC 0 |
| `mrf-radius` | post01 | 5 of 63360 differ; max \|diff\| 2.91e-11, 1 - NCC 3.3e-16 |
| `mrf-radius` | post02 | 23 of 63360 differ; max \|diff\| 3.64e-12, 1 - NCC 0 |
| `mrf-radius` | post03 | 7 of 63360 differ; max \|diff\| 1.78e-15, 1 - NCC 0 |
| `mrf-coefficient-image` | post01 | 38 of 63360 differ; max \|diff\| 5.96e-08, 1 - NCC 8.9e-16 |
| `mrf-coefficient-image` | post02 | 83 of 63360 differ; max \|diff\| 2.33e-10, 1 - NCC 0 |
| `mrf-coefficient-image` | post03 | 95 of 63360 differ; max \|diff\| 3.73e-09, 1 - NCC 3.3e-16 |
| `annealing` | post01 | 23 of 63360 differ; max \|diff\| 4.55e-13, 1 - NCC 0 |
| `annealing` | post02 | 13 of 63360 differ; max \|diff\| 3.16e-30, 1 - NCC 0 |
| `annealing` | post03 | 12 of 63360 differ; max \|diff\| 3.94e-31, 1 - NCC 0 |
| `verbose-priors` | post01 | 34 of 63360 differ; max \|diff\| 2.98e-08, 1 - NCC 2.2e-15 |
| `verbose-priors` | post02 | 111 of 63360 differ; max \|diff\| 5.96e-08, 1 - NCC 1.8e-15 |
| `verbose-priors` | post03 | 60 of 63360 differ; max \|diff\| 1.16e-10, 1 - NCC 7.8e-16 |
| `verbose-parzen` | post01 | 1 of 63360 differ; max \|diff\| 1.73e-18, 1 - NCC 0 |
| `real/ds000005-sub-01/smriprep` | post01 | 89 of 5898240 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `real/ds000005-sub-01/smriprep` | post02 | 53 of 5898240 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `real/ds000005-sub-01/smriprep` | post03 | 23 of 5898240 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `real/ds005040-sub-002/smriprep` | post01 | 80 of 15842304 differ; max \|diff\| 5.96e-08, 1 - NCC 4.8e-15 |
| `real/ds005040-sub-002/smriprep` | post02 | 60 of 15842304 differ; max \|diff\| 8.94e-08, 1 - NCC 0 |
| `real/ds005040-sub-002/smriprep` | post03 | 67 of 15842304 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `real/ds005454-sub-16/smriprep` | post01 | 4175 of 17547264 differ; max \|diff\| 1.19e-07, 1 - NCC 3.6e-14 |
| `real/ds005454-sub-16/smriprep` | post02 | 3655 of 17547264 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `real/ds005454-sub-16/smriprep` | post03 | 3805 of 17547264 differ; max \|diff\| 1.19e-07, 1 - NCC 3.3e-16 |
| `real/ds005752-sub-ON01016/smriprep` | post01 | 11 of 12845056 differ; max \|diff\| 5.96e-08, 1 - NCC 2e-15 |
| `real/ds005752-sub-ON01016/smriprep` | post02 | 23 of 12845056 differ; max \|diff\| 5.96e-08, 1 - NCC 7.9e-15 |
| `real/ds005752-sub-ON01016/smriprep` | post03 | 21 of 12845056 differ; max \|diff\| 2.98e-08, 1 - NCC 8.9e-15 |
| `real/ds000005-sub-01/priors-3` | post01 | 1101 of 5898240 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `real/ds000005-sub-01/priors-3` | post02 | 1079 of 5898240 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `real/ds000005-sub-01/priors-3` | post03 | 934 of 5898240 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `real/ds005040-sub-002/priors-3` | post01 | 3731 of 15842304 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `real/ds005040-sub-002/priors-3` | post02 | 3056 of 15842304 differ; max \|diff\| 1.19e-07, 1 - NCC 7.5e-15 |
| `real/ds005040-sub-002/priors-3` | post03 | 2971 of 15842304 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `real/ds001600-sub-1/priors-3` | post01 | 1945 of 11534336 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `real/ds001600-sub-1/priors-3` | post02 | 1693 of 11534336 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `real/ds001600-sub-1/priors-3` | post03 | 1760 of 11534336 differ; max \|diff\| 1.19e-07, 1 - NCC 1e-14 |
| `real/ds005454-sub-16/priors-3` | post01 | 4100 of 17547264 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `real/ds005454-sub-16/priors-3` | post02 | 4583 of 17547264 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `real/ds005454-sub-16/priors-3` | post03 | 4448 of 17547264 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `real/ds005752-sub-ON01016/priors-3` | post01 | 1560 of 12845056 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `real/ds005752-sub-ON01016/priors-3` | post02 | 1403 of 12845056 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `real/ds005752-sub-ON01016/priors-3` | post03 | 1423 of 12845056 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `real/ds000005-sub-01/priors-6` | post01 | 1180 of 5898240 differ; max \|diff\| 1.19e-07, 1 - NCC 1.5e-14 |
| `real/ds000005-sub-01/priors-6` | post02 | 1075 of 5898240 differ; max \|diff\| 1.19e-07, 1 - NCC 3e-14 |
| `real/ds000005-sub-01/priors-6` | post03 | 1198 of 5898240 differ; max \|diff\| 1.19e-07, 1 - NCC 1.8e-14 |
| `real/ds000005-sub-01/priors-6` | post04 | 1464 of 5898240 differ; max \|diff\| 5.96e-08, 1 - NCC 1.2e-15 |
| `real/ds000005-sub-01/priors-6` | post05 | 1279 of 5898240 differ; max \|diff\| 5.96e-08, 1 - NCC 1.8e-15 |
| `real/ds000005-sub-01/priors-6` | post06 | 1111 of 5898240 differ; max \|diff\| 1.19e-07, 1 - NCC 3.3e-16 |
| `real/ds000005/otsu` | post01 | 23 of 5898240 differ; max \|diff\| 5.96e-08, 1 - NCC 8.9e-16 |
| `real/ds000005/otsu` | post02 | 76 of 5898240 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `real/ds000005/otsu` | post03 | 37 of 5898240 differ; max \|diff\| 1.49e-08, 1 - NCC 1.1e-16 |
| `real/ds000005/random` | post01 | 39 of 5898240 differ; max \|diff\| 5.96e-08, 1 - NCC 2.1e-14 |
| `real/ds000005/random` | post02 | 30 of 5898240 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `real/ds000005/random` | post03 | 23 of 5898240 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
- `clock-seed` seg: ANTs run 1 vs run 2: identical; larmorx vs ANTs: identical / identical
- `clock-seed` post01: ANTs run 1 vs run 2: 14184 of 63360 differ; max |diff| 4.17e-06, 1 - NCC 6.5e-14; larmorx vs ANTs: identical / 14184 of 63360 differ; max |diff| 4.17e-06, 1 - NCC 6.5e-14
- `clock-seed` post02: ANTs run 1 vs run 2: 27756 of 63360 differ; max |diff| 4.17e-06, 1 - NCC 1.4e-13; larmorx vs ANTs: 1 of 63360 differ; max |diff| 9.09e-13, 1 - NCC 2.2e-16 / 27756 of 63360 differ; max |diff| 4.17e-06, 1 - NCC 1.4e-13
- `clock-seed` post03: ANTs run 1 vs run 2: 30641 of 63360 differ; max |diff| 3.64e-06, 1 - NCC 7.4e-14; larmorx vs ANTs: identical / 30641 of 63360 differ; max |diff| 3.64e-06, 1 - NCC 7.4e-14
- `clock-seed-random-labels` seg: ANTs run 1 vs run 2: 24062 of 63360 differ; smallest Dice 0.285677; larmorx vs ANTs: 23020 of 63360 differ; smallest Dice 0.271868 / 23787 of 63360 differ; smallest Dice 0.313139
- `clock-seed-random-labels` post01: ANTs run 1 vs run 2: 35551 of 63360 differ; max |diff| 0.992, 1 - NCC 0.73; larmorx vs ANTs: 35551 of 63360 differ; max |diff| 0.988, 1 - NCC 0.63 / 35551 of 63360 differ; max |diff| 0.999, 1 - NCC 0.81
- `clock-seed-random-labels` post02: ANTs run 1 vs run 2: 35551 of 63360 differ; max |diff| 0.989, 1 - NCC 0.82; larmorx vs ANTs: 35551 of 63360 differ; max |diff| 0.983, 1 - NCC 0.67 / 35551 of 63360 differ; max |diff| 0.989, 1 - NCC 0.79
- `clock-seed-random-labels` post03: ANTs run 1 vs run 2: 35551 of 63360 differ; max |diff| 0.983, 1 - NCC 0.83; larmorx vs ANTs: 35551 of 63360 differ; max |diff| 0.987, 1 - NCC 0.85 / 35551 of 63360 differ; max |diff| 0.99, 1 - NCC 0.76
- `real/ds000005/smriprep-clock-seed` seg: ANTs run 1 vs run 2: 300 of 5898240 differ; smallest Dice 0.999452; larmorx vs ANTs: 225 of 5898240 differ; smallest Dice 0.999652 / 279 of 5898240 differ; smallest Dice 0.999487
- `real/ds000005/smriprep-clock-seed` post01: ANTs run 1 vs run 2: 758096 of 5898240 differ; max |diff| 0.154, 1 - NCC 7.5e-06; larmorx vs ANTs: 721653 of 5898240 differ; max |diff| 0.104, 1 - NCC 4.2e-06 / 757470 of 5898240 differ; max |diff| 0.113, 1 - NCC 6.4e-06
- `real/ds000005/smriprep-clock-seed` post02: ANTs run 1 vs run 2: 795219 of 5898240 differ; max |diff| 0.154, 1 - NCC 6.3e-06; larmorx vs ANTs: 783857 of 5898240 differ; max |diff| 0.104, 1 - NCC 4.2e-06 / 795303 of 5898240 differ; max |diff| 0.139, 1 - NCC 5.8e-06
- `real/ds000005/smriprep-clock-seed` post03: ANTs run 1 vs run 2: 702527 of 5898240 differ; max |diff| 0.139, 1 - NCC 1.7e-06; larmorx vs ANTs: 672334 of 5898240 differ; max |diff| 0.0995, 1 - NCC 1.6e-06 / 658124 of 5898240 differ; max |diff| 0.139, 1 - NCC 1.9e-06

**ANTS-106 on the partial-volume phantom** (known fractions; Dice per class, 4 = 1x2 and 5 = 2x3, and the share of voxels labelled as the truth). larmorx's default corrects ANTS-106 and, as everywhere, ITK-50 and ANTS-96; without the MRF only those two apply:

| Case | ANTs | larmorx's default |
|---|---|---|
| `partial-volume-phantom-mrf-0` | Dice 0.975 0.944 0.975 0.728 0.578 (classes 1-5), 0.937 of the voxels as the truth | Dice 0.971 0.948 0.971 0.711 0.594 (classes 1-5), 0.934 of the voxels as the truth |
| `partial-volume-phantom-mrf-0.1` | Dice 0.980 0.951 0.964 0.295 0.405 (classes 1-5), 0.940 of the voxels as the truth | Dice 0.991 0.983 0.985 0.862 0.805 (classes 1-5), 0.977 of the voxels as the truth |
| `partial-volume-phantom-mrf-0.3` | no labels (ANTs stopped) | Dice 0.991 0.983 0.986 0.835 0.801 (classes 1-5), 0.978 of the voxels as the truth |

**sMRIPrep's call with partial-volume classes on fMRIPrep's T1w** (ANTS-106): ANTs stops; larmorx's default labels

| Case | ANTs | larmorx's default: voxels per class |
|---|---|---|
| `real/ds000005-sub-01/smriprep-partial-volume` | stops (exit 1) | classes 1-3 (CSF, GM, WM) and 4 = 1x2, 5 = 2x3: 1: 110433, 2: 224560, 3: 214642, 4: 109362, 5: 137019 |
| `real/ds005040-sub-002/smriprep-partial-volume` | stops (exit 1) | classes 1-3 (CSF, GM, WM) and 4 = 1x2, 5 = 2x3: 1: 277931, 2: 822401, 3: 769812, 4: 262474, 5: 395475 |

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | both produce their outputs, or neither does (same status) |
| Messages | identical text where compared (stdout; `-v` without the ITK base classes' lines) |
| Header | every field of the 348-byte header identical |
| Data type, shape, affine | identical (affine within 1e-06 mm) |
| larmorx's threads | 4 threads and 1 thread give the same bits |
| Values | bit-identical to ANTs with correctly rounded math (CORE-MATH preloaded); with ANTs as built, bit-identity reported and NCC > 0.995 where posteriors differ |
| Reported | ANTs' thread band (2, 4, 6 threads vs 1), ANTs' seed band (two clock-seeded runs), the distance to fMRIPrep's own files |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| dimensions | 2 | 2 | 0 | 0 | 0 |
| initialization | 6 | 6 | 0 | 0 | 0 |
| inputs | 3 | 2 | 1 | 0 | 0 |
| likelihood | 5 | 5 | 0 | 0 | 0 |
| messages | 31 | 5 | 22 | 4 | 0 |
| mrf | 5 | 5 | 0 | 0 | 0 |
| outliers | 5 | 4 | 1 | 0 | 0 |
| partial volume | 6 | 4 | 2 | 0 | 0 |
| posteriors | 16 | 15 | 0 | 1 | 0 |
| random seeds | 2 | 0 | 0 | 2 | 0 |
| real (other) | 3 | 2 | 0 | 1 | 0 |
| real (partial volume) | 2 | 0 | 2 | 0 | 0 |
| real (priors) | 6 | 6 | 0 | 0 | 0 |
| real (sMRIPrep) | 5 | 5 | 0 | 0 | 0 |
| verbose | 5 | 5 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `priors-minimize-memory` | ANTs aborts; larmorx exits 1 with its text |
| `bad-number` | ANTs aborts; larmorx exits 1 with its text |
| `icm-brackets` | ANTs aborts; larmorx exits 1 with its text |
| `label-propagation-without-parameters` | ANTs aborts; larmorx exits 1 with its text |
| `output-extension` | ANTs aborts; larmorx exits 1 with its text |
| `clock-seed` | ANTs draws its seed from the clock; larmorx uses 121212 |
| `clock-seed-random-labels` | ANTs draws its seed from the clock; larmorx uses 121212 |
| `real/ds000005/smriprep-clock-seed` | ANTs draws its seed from the clock; larmorx uses 121212 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `grubbs-rosner-multivariate` | Grubbs-Rosner on two images: the class covariance becomes singular in both | ANTs exit 1: '\nSLATEC/DGAMMA: X SO BIG GAMMA OVERFLOWS\nSLATEC/DGAMMA: X SO BIG GAMMA OVERFLOWS\nSLATEC/DGAMMA: X SO BIG GAMMA OVERFLOWS\nSLATEC/DGAMMA: X SO … |
| `partial-volume-with-mrf` | partial-volume classes with the MRF (ANTS-106: they can never win a voxel; both stop) | ANTs exit 1: ''; larmorx exit 1: 'rrects it]\nlarmorx: upstream defect ANTS-96 (Atropos) reproduced: the partial-volume classes keep their initial means [LARMO… |
| `partial-volume-phantom-mrf-0.3` | the partial-volume phantom, classes 1x2 and 2x3 with their likelihoods, MRF 0.3 (ANTS-106), scored against the known fractions | ANTs exit 1: ''; larmorx exit 1: 'rrects it]\nlarmorx: upstream defect ANTS-96 (Atropos) reproduced: the partial-volume classes keep their initial means [LARMO… |
| `mask-255` | a mask of 255s (one LabelMap label, ITK's error) | ANTs exit 1: 'nListSampleFunction.hxx, line 105\nGaussianListSampleFunction (0x57ccfd938f10): The input list sample has <= 1 element. Function evaluations will… |
| `no-arguments` | no arguments: the short help | ANTs exit 1: ' ). \n\n -v, --verbose (0)/1\n Verbose output. \n\n -h \n Print the help menu (short version). \n\n --help \n Print the help menu.'; larmorx exit… |
| `no-initialization` | no -i | ANTs exit 1: 'No initialization option specified.'; larmorx exit 1: 'No initialization option specified.' |
| `initialization-without-classes` | -i KMeans (no classes) | ANTs exit 1: 'usage option (see -m). With that option one can specify a prior probability threshold in which only those pixels exceeding that threshold are sto… |
| `unknown-initialization` | -i Foo[3] | ANTs exit 1: 'Unrecognized initialization strategy request.'; larmorx exit 1: 'Unrecognized initialization strategy request.' |
| `wrong-centres` | two centres for three classes | ANTs exit 1: 'The cluster center vector size does not equal the specified number of classes.'; larmorx exit 1: 'The cluster center vector size does not equal t… |
| `priors-without-weight` | PriorProbabilityImages without a weight | ANTs exit 1: 'usage option (see -m). With that option one can specify a prior probability threshold in which only those pixels exceeding that threshold are sto… |
| `priors-missing` | prior images that do not exist | ANTs exit 1: 'file <tmp>/nope01.nii.gz does not exist .'; larmorx exit 1: 'file <tmp>/nope01.nii.gz does not exist .' |
| `prior-label-missing` | a prior label image that does not exist | ANTs exit 1: 'file <tmp>/nope.nii.gz does not exist .'; larmorx exit 1: 'file <tmp>/nope.nii.gz does not exist .' |
| `no-mask` | no -x | ANTs exit 1: 'An image mask is required. Specify a mask image with the -x option.'; larmorx exit 1: 'An image mask is required. Specify a mask image with the -… |
| `mask-missing` | a mask that does not exist | ANTs exit 1: 'file <tmp>/nope.nii.gz does not exist .'; larmorx exit 1: 'file <tmp>/nope.nii.gz does not exist .' |
| `no-image` | no -a | ANTs exit 1: 'No input images were specified. Specify an input image with the -a option.'; larmorx exit 1: 'No input images were specified. Specify an input im… |
| `image-missing` | an image that does not exist | ANTs exit 1: 'file <tmp>/nope.nii.gz does not exist .'; larmorx exit 1: 'file <tmp>/nope.nii.gz does not exist .' |
| `dimension-5` | -d 5 | ANTs exit 1: 'Unsupported dimension'; larmorx exit 1: 'Unsupported dimension' |
| `negative-temperature` | a negative annealing temperature | ANTs exit 1: 'Annealing temperature must be positive.'; larmorx exit 1: 'Annealing temperature must be positive.' |
| `rate-above-one` | an annealing rate of 2 | ANTs exit 1: 'Annealing rate must be in the range [0, 1].'; larmorx exit 1: 'Annealing rate must be in the range [0, 1].' |
| `wrong-levels` | -b with two levels in 3D | ANTs exit 1: 'Incorrect number of levels'; larmorx exit 1: 'Incorrect number of levels' |
| `wrong-mesh` | -b with a 2D mesh in 3D | ANTs exit 1: 'Incorrect mesh resolution'; larmorx exit 1: 'Incorrect mesh resolution' |
| `wrong-radius` | -m with a 2D radius in 3D | ANTs exit 1: 'MRF radius size needs to be equal to the image dimension.'; larmorx exit 1: 'MRF radius size needs to be equal to the image dimension.' |
| `unknown-likelihood` | -k Foo | ANTs exit 1: 'Unrecognized likelihood model request.'; larmorx exit 1: 'Unrecognized likelihood model request.' |
| `tensor-image-count` | a tensor model with one image | ANTs exit 1: 'Expect images in upper triangular order \n xx xy xz yy yz zz \nIncorrect number of intensity images specified.'; larmorx exit 1: 'Expect images i… |
| `partial-volume-labels` | -s 1x2x3 (three labels) | ANTs exit 1: 'Error: Currently Atropos only supports partial volume label sets of size equal to 2.'; larmorx exit 1: 'Error: Currently Atropos only supports pa… |
| `unknown-outliers` | -w Foo | ANTs exit 1: 'Unrecognized outlier handling strategy request.'; larmorx exit 1: 'Unrecognized outlier handling strategy request.' |
| `real/ds000005-sub-01/smriprep-partial-volume` | sMRIPrep's options on ds000005-sub-01's N4-corrected T1w and brain mask with partial-volume classes 1x2 and 2x3 (ANTS-106: ANTs stops) | ANTs exit 1: ''; larmorx exit 1: 'rrects it]\nlarmorx: upstream defect ANTS-96 (Atropos) reproduced: the partial-volume classes keep their initial means [LARMO… |
| `real/ds005040-sub-002/smriprep-partial-volume` | sMRIPrep's options on ds005040-sub-002's N4-corrected T1w and brain mask with partial-volume classes 1x2 and 2x3 (ANTS-106: ANTs stops) | ANTs exit 1: ''; larmorx exit 1: 'rrects it]\nlarmorx: upstream defect ANTS-96 (Atropos) reproduced: the partial-volume classes keep their initial means [LARMO… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (708bb99389fe) |
| larmorx-testdata | 00b862132e9c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

ANTs runs as its own binary, larmorx in-process (`larmorx._core.cli_main`), with ANTs' defects reproduced (`LARMORX_UPSTREAM_DEFECTS=reproduce`). Outputs are read with nibabel (values as stored) and the headers from their raw bytes. Inputs named `gen:atropos-*` are built by this suite (seeded); the real cases read the fMRIPrep oracle runs and the OASIS30ANTs template in `oracles/`.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `kmeans-3` | pass | 29/29 | K-means, 3 classes, MRF 0.1 (sMRIPrep's options on the phantom) |
| `kmeans-4` | pass | 33/33 | K-means, 4 classes, MRF 0.3 |
| `kmeans-centres` | pass | 24/24 | K-means from given centres (50, 100, 150) |
| `otsu` | pass | 24/24 | Otsu thresholds, 3 classes |
| `random` | pass | 27/27 | random initial labels (fixed seed) |
| `kmeans-multivariate` | pass | 24/24 | K-means on two images (ANTS-90: only the first decides the initial labels) |
| `priors-socrates` | pass | 27/27 | prior probability images (weight 0.5), Socrates |
| `priors-plato` | pass | 27/27 | prior probability images, Plato |
| `priors-aristotle` | pass | 27/27 | prior probability images, Aristotle |
| `priors-sigmoid` | pass | 27/27 | prior probability images, Sigmoid |
| `priors-euclidean` | pass | 27/27 | prior probability images (weight 0.25), Euclidean distance priors |
| `priors-threshold` | pass | 27/27 | prior probability images with a probability threshold of 0.1 |
| `priors-vector` | pass | 27/27 | the priors as one vector image (ANTS-100: every class reads the last component's normalized copy) |
| `priors-minimize-memory` | expected-divergence | 4/4 | prior probability images with --minimize-memory-usage and a mask (ANTS-105: ANTs crashes) |
| `priors-all-outputs` | pass | 60/60 | prior probability images: posteriors, likelihoods and distance priors written |
| `priors-adaptive-smoothing` | pass | 75/75 | prior probability images with adaptive smoothing (0.5) of the intensities: B-spline fits per class, 3 levels |
| `label-fast-marching` | pass | 27/27 | a prior label image (holes filled by fast marching) |
| `label-euclidean` | pass | 27/27 | a prior label image, Euclidean distances |
| `label-propagation` | pass | 27/27 | a prior label image with label propagation for every class ([0.5,0.8]) |
| `label-propagation-per-label` | pass | 27/27 | label propagation for labels 1 and 3 |
| `histogram-parzen` | pass | 24/24 | histogram Parzen windows (sigma 1, 32 bins) |
| `manifold-parzen` | pass | 24/24 | manifold Parzen windows (k = 10; ANTS-95: the weights ignored) |
| `manifold-parzen-covariance` | pass | 26/26 | manifold Parzen windows with local covariances (5 neighbours; ANTS-104) |
| `log-euclidean` | pass | 27/27 | log-Euclidean Gaussians of six tensor components |
| `joint-shape-orientation` | pass | 24/24 | joint shape and orientation histograms of six tensor components (ANTS-103: an unset coordinate) |
| `mrf-synchronous` | pass | 24/24 | synchronous updating (no ICM) |
| `mrf-radius` | pass | 27/27 | an anisotropic MRF radius (2x1x2), smoothing 0.2 |
| `mrf-coefficient-image` | pass | 27/27 | an MRF coefficient image |
| `mrf-none` | pass | 24/24 | no MRF (smoothing 0) |
| `mrf-icm-codes` | pass | 29/29 | two ICM iterations and the ICM code image |
| `no-proportions` | pass | 24/24 | no mixture proportions (Socrates[0]) |
| `annealing` | pass | 27/27 | annealing from 2 at a rate of 0.5 down to 0.1 |
| `box-plot` | pass | 24/24 | box-plot winsorizing (ANTS-94: the filter re-runs on its own output) |
| `box-plot-wide` | pass | 24/24 | box-plot winsorizing with a whisker of 100 (nothing beyond) |
| `grubbs-rosner` | pass | 20/20 | Grubbs-Rosner winsorizing, three iterations (ANTS-94; ANTS-107: from the second iteration on, ANTs weighs the grown sam… |
| `grubbs-rosner-outliers` | pass | 20/20 | Grubbs-Rosner on an image with six far outliers (ITK-51: ITK's Student quantiles are NaN, no outlier is found) |
| `grubbs-rosner-multivariate` | both-error | 4/4 | Grubbs-Rosner on two images: the class covariance becomes singular in both |
| `partial-volume-mrf-only` | pass | 24/24 | a partial-volume label set used by the MRF only |
| `partial-volume-likelihoods` | pass | 24/24 | partial-volume classes 1x2 and 2x3 with their likelihoods, no MRF |
| `partial-volume-with-mrf` | both-error | 4/4 | partial-volume classes with the MRF (ANTS-106: they can never win a voxel; both stop) |
| `partial-volume-phantom-mrf-0` | pass | 26/26 | the partial-volume phantom, classes 1x2 and 2x3 with their likelihoods, MRF 0 (ANTS-106), scored against the known frac… |
| `partial-volume-phantom-mrf-0.1` | pass | 26/26 | the partial-volume phantom, classes 1x2 and 2x3 with their likelihoods, MRF 0.1 (ANTS-106), scored against the known fr… |
| `partial-volume-phantom-mrf-0.3` | both-error | 6/6 | the partial-volume phantom, classes 1x2 and 2x3 with their likelihoods, MRF 0.3 (ANTS-106), scored against the known fr… |
| `2d` | pass | 24/24 | a 2D slice |
| `4d` | pass | 24/24 | a 4D series (two volumes) |
| `mask-255` | both-error | 4/4 | a mask of 255s (one LabelMap label, ITK's error) |
| `mask-float` | pass | 24/24 | a float32 mask with 0.6 at its rim (read as 0) |
| `no-dimension` | pass | 24/24 | no -d: Atropos assumes 3D (ANTS-99) |
| `no-arguments` | both-error | 4/4 | no arguments: the short help |
| `help` | pass | 3/3 | --help |
| `short-help` | pass | 3/3 | -h |
| `no-initialization` | both-error | 4/4 | no -i |
| `initialization-without-classes` | both-error | 4/4 | -i KMeans (no classes) |
| `unknown-initialization` | both-error | 4/4 | -i Foo[3] |
| `bad-number` | expected-divergence | 4/4 | -i KMeans[3x] (the parser cannot convert) |
| `wrong-centres` | both-error | 4/4 | two centres for three classes |
| `priors-without-weight` | both-error | 4/4 | PriorProbabilityImages without a weight |
| `priors-missing` | both-error | 4/4 | prior images that do not exist |
| `prior-label-missing` | both-error | 4/4 | a prior label image that does not exist |
| `no-mask` | both-error | 4/4 | no -x |
| `mask-missing` | both-error | 4/4 | a mask that does not exist |
| `no-image` | both-error | 4/4 | no -a |
| `image-missing` | both-error | 4/4 | an image that does not exist |
| `dimension-5` | both-error | 4/4 | -d 5 |
| `negative-temperature` | both-error | 4/4 | a negative annealing temperature |
| `rate-above-one` | both-error | 4/4 | an annealing rate of 2 |
| `wrong-levels` | both-error | 4/4 | -b with two levels in 3D |
| `wrong-mesh` | both-error | 4/4 | -b with a 2D mesh in 3D |
| `wrong-radius` | both-error | 4/4 | -m with a 2D radius in 3D |
| `unknown-likelihood` | both-error | 4/4 | -k Foo |
| `tensor-image-count` | both-error | 4/4 | a tensor model with one image |
| `partial-volume-labels` | both-error | 4/4 | -s 1x2x3 (three labels) |
| `partial-volume-label-range` | pass | 9/9 | -s 1x4 with three classes |
| `unknown-outliers` | both-error | 4/4 | -w Foo |
| `icm-brackets` | expected-divergence | 4/4 | --icm [1,2], the documented form (ANTS-102: ANTs aborts) |
| `label-propagation-without-parameters` | expected-divergence | 4/4 | -l 2 (the parser cannot convert the missing parameters: ANTs aborts) |
| `unknown-formulation` | pass | 9/9 | -p Zeno (Socrates kept, ANTS-101) |
| `output-extension` | expected-divergence | 3/4 | a classified image with an unknown extension |
| `posterior-without-percent` | pass | 14/14 | a posterior name without %d (every class writes the same file) |
| `verbose` | pass | 26/26 | -v 1: progress, the filter's settings and its models |
| `verbose-grubbs` | pass | 26/26 | -v 1 with Grubbs-Rosner (its outlier listing; one iteration, before ANTS-107) |
| `verbose-box-plot` | pass | 26/26 | -v 1 with box-plot winsorizing |
| `verbose-priors` | pass | 29/29 | -v 1 with prior probability images and label propagation |
| `verbose-parzen` | pass | 27/27 | -v 1 with histogram Parzen windows |
| `clock-seed` | expected-divergence | 7/7 | --use-random-seed 1: ANTs seeds from the clock (two runs); larmorx uses the seed 121212 |
| `clock-seed-random-labels` | expected-divergence | 7/7 | Random[3] with the clock's seed |
| `real/ds000005-sub-01/smriprep` | pass | 35/35 | sMRIPrep's Atropos call in ds000005-sub-01 (fMRIPrep 25.2.6's command line, its N4-corrected T1w and mask) |
| `real/ds001600-sub-1/smriprep` | pass | 28/28 | sMRIPrep's Atropos call in ds001600-sub-1 (fMRIPrep 25.2.6's command line, its N4-corrected T1w and mask) |
| `real/ds005040-sub-002/smriprep` | pass | 31/31 | sMRIPrep's Atropos call in ds005040-sub-002 (fMRIPrep 25.2.6's command line, its N4-corrected T1w and mask) |
| `real/ds005454-sub-16/smriprep` | pass | 31/31 | sMRIPrep's Atropos call in ds005454-sub-16 (fMRIPrep 25.2.6's command line, its N4-corrected T1w and mask) |
| `real/ds005752-sub-ON01016/smriprep` | pass | 31/31 | sMRIPrep's Atropos call in ds005752-sub-ON01016 (fMRIPrep 25.2.6's command line, its N4-corrected T1w and mask) |
| `real/ds000005-sub-01/priors-3` | pass | 27/27 | antsCorticalThickness-style segmentation in ds000005-sub-01: the OASIS30ANTs priors (CSF, CGM, WM) in T1w space, weight… |
| `real/ds005040-sub-002/priors-3` | pass | 27/27 | antsCorticalThickness-style segmentation in ds005040-sub-002: the OASIS30ANTs priors (CSF, CGM, WM) in T1w space, weigh… |
| `real/ds001600-sub-1/priors-3` | pass | 27/27 | antsCorticalThickness-style segmentation in ds001600-sub-1: the OASIS30ANTs priors (CSF, CGM, WM) in T1w space, weight … |
| `real/ds005454-sub-16/priors-3` | pass | 27/27 | antsCorticalThickness-style segmentation in ds005454-sub-16: the OASIS30ANTs priors (CSF, CGM, WM) in T1w space, weight… |
| `real/ds005752-sub-ON01016/priors-3` | pass | 27/27 | antsCorticalThickness-style segmentation in ds005752-sub-ON01016: the OASIS30ANTs priors (CSF, CGM, WM) in T1w space, w… |
| `real/ds000005-sub-01/priors-6` | pass | 45/45 | antsCorticalThickness-style segmentation in ds000005-sub-01: the OASIS30ANTs priors (CSF, CGM, WM, SCGM, BS, CBM) in T1… |
| `real/ds000005-sub-01/smriprep-partial-volume` | both-error | 5/5 | sMRIPrep's options on ds000005-sub-01's N4-corrected T1w and brain mask with partial-volume classes 1x2 and 2x3 (ANTS-1… |
| `real/ds005040-sub-002/smriprep-partial-volume` | both-error | 5/5 | sMRIPrep's options on ds005040-sub-002's N4-corrected T1w and brain mask with partial-volume classes 1x2 and 2x3 (ANTS-… |
| `real/ds000005/otsu` | pass | 27/27 | Otsu[3] on ds000005's N4-corrected T1w |
| `real/ds000005/random` | pass | 27/27 | Random[3] on ds000005's T1w (fixed seed) |
| `real/ds000005/smriprep-clock-seed` | expected-divergence | 7/7 | sMRIPrep's call with --use-random-seed 1: ANTs seeds from the clock (two ANTs runs, ANTs' seed band) |
