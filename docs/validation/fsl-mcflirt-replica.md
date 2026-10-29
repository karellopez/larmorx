# Parity: mcflirt replica (bit-exact, FSL Licence)

**All cases agree.** 203 cases on the `standard` tier: 181 pass, 22 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx-nc fsl mcflirt` (crate larmorx-nc-mcflirt, translated from FSL 6.0.7.23's source; `lx.mri.hmc(implementation="replica")`)
- **Reference:** mcflirt from FSL 6.0.7.23 (fsl-mcflirt 2111.0, the conda build at `~/fsl-6.0.7.23`, as built: glibc 2.35's maths): the recorded runs in `oracles/fsl-6.0.7.23/mcflirt-haswell/` and runs made here
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-10-29 on Linux x86_64, with `python -m larmorx_validation parity fsl-mcflirt-replica --tier standard`
- **Original programs:** 132 runs: 115 from the oracle cache, 17 run; cache `<workspace>/oracles/cache`, mode `use`

- **Bit-identical to mcflirt as built** (every file, the exit status and both streams; the replica in `reproduce` mode): 181 of 181 compared cases; 22 where both end with the same error.

The three columns of the head-motion validation (the clean-room original's numbers from its record, `docs/validation/mri-hmc.md`, not computed again):

| Recorded run | Replica vs mcflirt | Clean-room original vs mcflirt |
|---|---|---|
| `real/ds000005/default` | pass (246/246 files) | pass: p95 RMS 0.0212 mm, MAT identical 1/240 |
| `real/ds000005/fmriprep` | pass (241/241 files) | pass: p95 RMS 0.0262 mm, MAT identical 0/240 |
| `real/ds000005/fmriprep-repeat2` | pass (241/241 files) | pass: p95 RMS 0.0262 mm, MAT identical 0/240 |
| `real/ds000005/fmriprep-repeat3` | pass (241/241 files) | pass: p95 RMS 0.0262 mm, MAT identical 0/240 |
| `real/ds000005/fmriprep-reports` | pass (246/246 files) | pass: p95 RMS 0.0262 mm, MAT identical 0/240 |
| `real/ds000005/opt-2d` | pass (246/246 files) | pass: p95 RMS 0.0133 mm, MAT identical 0/240 |
| `real/ds000005/opt-bins64` | pass (246/246 files) | pass: p95 RMS 0.0262 mm, MAT identical 0/240 |
| `real/ds000005/opt-cost-corratio` | pass (246/246 files) | pass: p95 RMS 0.0662 mm, MAT identical 0/240 |
| `real/ds000005/opt-cost-leastsquares` | pass (246/246 files) | pass: p95 RMS 0.0220 mm, MAT identical 0/240 |
| `real/ds000005/opt-cost-mutualinfo` | pass (246/246 files) | pass: p95 RMS 0.0048 mm, MAT identical 1/240 |
| `real/ds000005/opt-cost-normcorr` | pass (246/246 files) | pass: p95 RMS 0.0262 mm, MAT identical 0/240 |
| `real/ds000005/opt-cost-normmi` | pass (246/246 files) | pass: p95 RMS 0.0108 mm, MAT identical 0/240 |
| `real/ds000005/opt-cost-woods` | pass (246/246 files) | pass: p95 RMS – mm, MAT identical – |
| `real/ds000005/opt-dof12` | pass (246/246 files) | pass: p95 RMS 0.0220 mm, MAT identical 0/240 |
| `real/ds000005/opt-dof7` | pass (246/246 files) | pass: p95 RMS 0.0208 mm, MAT identical 0/240 |
| `real/ds000005/opt-dof9` | pass (246/246 files) | pass: p95 RMS 0.0206 mm, MAT identical 0/240 |
| `real/ds000005/opt-fudge` | pass (246/246 files) | pass: p95 RMS 0.0147 mm, MAT identical 0/240 |
| `real/ds000005/opt-gdt` | both-error (0/0 files) | both-error: p95 RMS – mm, MAT identical – |
| `real/ds000005/opt-meanvol` | pass (247/247 files) | pass: p95 RMS 0.0205 mm, MAT identical 0/240 |
| `real/ds000005/opt-meanvol-noref` | pass (247/247 files) | pass: p95 RMS 0.0226 mm, MAT identical 0/240 |
| `real/ds000005/opt-nn_final` | pass (246/246 files) | pass: p95 RMS 0.0262 mm, MAT identical 0/240 |
| `real/ds000005/opt-refvol0` | pass (246/246 files) | pass: p95 RMS 0.0204 mm, MAT identical 1/240 |
| `real/ds000005/opt-rotation2` | pass (246/246 files) | pass: p95 RMS 0.0234 mm, MAT identical 0/240 |
| `real/ds000005/opt-scaling3` | pass (246/246 files) | pass: p95 RMS 0.0262 mm, MAT identical 0/240 |
| `real/ds000005/opt-smooth0` | pass (246/246 files) | pass: p95 RMS 0.0247 mm, MAT identical 3/240 |
| `real/ds000005/opt-smooth2` | pass (246/246 files) | pass: p95 RMS 0.0247 mm, MAT identical 0/240 |
| `real/ds000005/opt-spline_final` | pass (246/246 files) | pass: p95 RMS 0.0262 mm, MAT identical 0/240 |
| `real/ds000005/opt-stages1` | pass (246/246 files) | pass: p95 RMS 0.0271 mm, MAT identical 3/240 |
| `real/ds000005/opt-stages2` | pass (246/246 files) | pass: p95 RMS 0.0308 mm, MAT identical 0/240 |
| `real/ds000005/opt-stats` | pass (249/249 files) | pass: p95 RMS 0.0262 mm, MAT identical 0/240 |
| `real/ds000117/default` | pass (214/214 files) | pass: p95 RMS 0.0220 mm, MAT identical 1/208 |
| `real/ds000117/fmriprep` | pass (209/209 files) | pass: p95 RMS 0.0191 mm, MAT identical 0/208 |
| `real/ds000117/fmriprep-reports` | pass (214/214 files) | pass: p95 RMS 0.0191 mm, MAT identical 0/208 |
| `real/ds000122/default` | pass (264/264 files) | pass: p95 RMS 0.0288 mm, MAT identical 1/258 |
| `real/ds000122/fmriprep` | pass (259/259 files) | pass: p95 RMS 0.0299 mm, MAT identical 0/258 |
| `real/ds000122/fmriprep-reports` | pass (264/264 files) | pass: p95 RMS 0.0299 mm, MAT identical 0/258 |
| `real/ds000210-rest-echo2/default` | pass (210/210 files) | pass: p95 RMS 0.0210 mm, MAT identical 1/204 |
| `real/ds000210-rest-echo2/fmriprep` | pass (205/205 files) | pass: p95 RMS 0.0226 mm, MAT identical 0/204 |
| `real/ds000210-rest-echo2/fmriprep-reports` | pass (210/210 files) | pass: p95 RMS 0.0226 mm, MAT identical 0/204 |
| `real/ds000258-bigendian/default` | pass (245/245 files) | pass: p95 RMS 0.0178 mm, MAT identical 1/239 |
| `real/ds000258-bigendian/fmriprep` | pass (240/240 files) | pass: p95 RMS 0.0181 mm, MAT identical 0/239 |
| `real/ds000258-bigendian/fmriprep-reports` | pass (245/245 files) | pass: p95 RMS 0.0181 mm, MAT identical 0/239 |
| `real/ds003345/default` | pass (222/222 files) | fail: p95 RMS 0.0279 mm, MAT identical 1/216 |
| `real/ds003345/fmriprep` | pass (217/217 files) | pass: p95 RMS 0.0259 mm, MAT identical 0/216 |
| `real/ds003345/fmriprep-reports` | pass (222/222 files) | pass: p95 RMS 0.0259 mm, MAT identical 0/216 |
| `real/ds003763-truncated/default` | both-error (0/0 files) | both-error: p95 RMS – mm, MAT identical – |
| `real/ds005040/default` | pass (69/69 files) | pass: p95 RMS 0.0260 mm, MAT identical 1/63 |
| `real/ds005040/fmriprep` | pass (64/64 files) | pass: p95 RMS 0.0252 mm, MAT identical 0/63 |
| `real/ds005040/fmriprep-reports` | pass (69/69 files) | pass: p95 RMS 0.0252 mm, MAT identical 0/63 |
| `real/ds006010/default` | pass (119/119 files) | pass: p95 RMS 0.0130 mm, MAT identical 1/113 |
| `real/ds006010/fmriprep` | pass (114/114 files) | pass: p95 RMS 0.0142 mm, MAT identical 0/113 |
| `real/ds006010/fmriprep-reports` | pass (119/119 files) | pass: p95 RMS 0.0142 mm, MAT identical 0/113 |
| `real/ds006736/default` | pass (75/75 files) | pass: p95 RMS 0.0189 mm, MAT identical 1/69 |
| `real/ds006736/fmriprep` | pass (70/70 files) | pass: p95 RMS 0.0252 mm, MAT identical 0/69 |
| `real/ds006736/fmriprep-reports` | pass (75/75 files) | pass: p95 RMS 0.0252 mm, MAT identical 0/69 |
| `syn/cli/bare-input` | pass (13/13 files) | pass: p95 RMS 0.0488 mm, MAT identical 1/12 |
| `syn/cli/dof5` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/cli/help` | both-error (0/0 files) | both-error: p95 RMS – mm, MAT identical – |
| `syn/cli/help-with-input` | pass (0/0 files) | pass: p95 RMS – mm, MAT identical – |
| `syn/cli/no-arguments` | both-error (0/0 files) | both-error: p95 RMS – mm, MAT identical – |
| `syn/cli/plots-only` | pass (2/2 files) | pass: p95 RMS – mm, MAT identical – |
| `syn/cli/refvol-minus1` | pass (18/18 files) | pass: p95 RMS 0.0488 mm, MAT identical 1/12 |
| `syn/cli/refvol-minus2` | both-error (0/0 files) | both-error: p95 RMS – mm, MAT identical – |
| `syn/cli/rmsrel-only` | pass (4/4 files) | pass: p95 RMS 0.0000 mm, MAT identical 1/1 |
| `syn/cli/stages0` | both-error (0/0 files) | both-error: p95 RMS – mm, MAT identical – |
| `syn/cli/unknown-option-middle` | both-error (0/0 files) | both-error: p95 RMS – mm, MAT identical – |
| `syn/content/background/default` | pass (12/12 files) | pass: p95 RMS 0.0864 mm, MAT identical 1/6 |
| `syn/content/background/reffile` | pass (12/12 files) | pass: p95 RMS 0.0870 mm, MAT identical 1/6 |
| `syn/content/nan/default` | pass (18/18 files) | pass: p95 RMS 0.0567 mm, MAT identical 2/12 |
| `syn/content/nan/reffile` | pass (18/18 files) | pass: p95 RMS 0.0585 mm, MAT identical 1/12 |
| `syn/content/ramp/reffile` | pass (11/11 files) | pass: p95 RMS – mm, MAT identical – |
| `syn/content/ramp/reffile-smooth0` | pass (11/11 files) | pass: p95 RMS – mm, MAT identical – |
| `syn/dtypes/float32/default` | pass (18/18 files) | pass: p95 RMS 0.0488 mm, MAT identical 1/12 |
| `syn/dtypes/float32/reffile` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/dtypes/float64/default` | pass (18/18 files) | pass: p95 RMS 0.0488 mm, MAT identical 1/12 |
| `syn/dtypes/float64/reffile` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/dtypes/int16-scaled/default` | pass (18/18 files) | pass: p95 RMS 0.0488 mm, MAT identical 1/12 |
| `syn/dtypes/int16-scaled/reffile` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/dtypes/uint16/default` | pass (18/18 files) | pass: p95 RMS 0.0488 mm, MAT identical 1/12 |
| `syn/dtypes/uint16/reffile` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/dtypes/uint8/default` | pass (18/18 files) | pass: p95 RMS 0.0395 mm, MAT identical 1/12 |
| `syn/dtypes/uint8/reffile` | pass (18/18 files) | pass: p95 RMS 0.0961 mm, MAT identical 0/12 |
| `syn/geometry/single-volume-3d/default` | pass (7/7 files) | pass: p95 RMS 0.0000 mm, MAT identical 1/1 |
| `syn/geometry/single-volume-3d/reffile` | pass (7/7 files) | pass: p95 RMS 0.0104 mm, MAT identical 0/1 |
| `syn/geometry/single-volume/default` | pass (7/7 files) | pass: p95 RMS 0.0000 mm, MAT identical 1/1 |
| `syn/geometry/single-volume/reffile` | pass (7/7 files) | pass: p95 RMS 0.0104 mm, MAT identical 0/1 |
| `syn/geometry/thin-slab/default` | pass (14/14 files) | pass: p95 RMS 0.0407 mm, MAT identical 1/8 |
| `syn/geometry/thin-slab/fov10` | pass (14/14 files) | pass: p95 RMS 0.0407 mm, MAT identical 1/8 |
| `syn/geometry/thin-slab/rotation2` | pass (14/14 files) | pass: p95 RMS 0.0308 mm, MAT identical 2/8 |
| `syn/geometry/tiny/default` | pass (12/12 files) | pass: p95 RMS 0.1130 mm, MAT identical 1/6 |
| `syn/geometry/two-volumes/default` | pass (8/8 files) | pass: p95 RMS 0.0514 mm, MAT identical 1/2 |
| `syn/geometry/two-volumes/reffile` | pass (8/8 files) | pass: p95 RMS 0.0578 mm, MAT identical 0/2 |
| `syn/motion/identical-copies/default` | pass (12/12 files) | pass: p95 RMS 0.0000 mm, MAT identical 6/6 |
| `syn/motion/identical-copies/reffile` | pass (12/12 files) | pass: p95 RMS 0.0575 mm, MAT identical 0/6 |
| `syn/motion/identical-copies/reffile-fudge` | pass (12/12 files) | pass: p95 RMS 0.0228 mm, MAT identical 0/6 |
| `syn/motion/large/default` | pass (18/18 files) | pass: p95 RMS 0.0463 mm, MAT identical 1/12 |
| `syn/motion/large/reffile` | pass (18/18 files) | pass: p95 RMS 0.0646 mm, MAT identical 0/12 |
| `syn/motion/small/2d` | pass (18/18 files) | pass: p95 RMS 0.0360 mm, MAT identical 0/12 |
| `syn/motion/small/default` | pass (18/18 files) | pass: p95 RMS 0.0488 mm, MAT identical 1/12 |
| `syn/motion/small/default-report` | pass (18/18 files) | pass: p95 RMS 0.0488 mm, MAT identical 1/12 |
| `syn/motion/small/dof12` | pass (18/18 files) | pass: p95 RMS 0.1252 mm, MAT identical 0/12 |
| `syn/motion/small/dof7` | pass (18/18 files) | pass: p95 RMS 0.0707 mm, MAT identical 0/12 |
| `syn/motion/small/fsloutputtype-nifti` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/motion/small/fudge` | pass (18/18 files) | pass: p95 RMS 0.0486 mm, MAT identical 0/12 |
| `syn/motion/small/init` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/motion/small/meanvol` | pass (19/19 files) | pass: p95 RMS 0.2811 mm, MAT identical 0/12 |
| `syn/motion/small/nifti2` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/motion/small/nn_final` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/motion/small/no-fsloutputtype` | both-error (12/12 files) | expected-divergence: p95 RMS – mm, MAT identical – |
| `syn/motion/small/no-input` | both-error (0/0 files) | both-error: p95 RMS – mm, MAT identical – |
| `syn/motion/small/no-out` | pass (14/14 files) | pass: p95 RMS 0.0488 mm, MAT identical 1/12 |
| `syn/motion/small/out-no-ext` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/motion/small/out-twice` | pass (30/30 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/motion/small/reffile` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/motion/small/reffile-4d` | pass (18/18 files) | pass: p95 RMS 0.0560 mm, MAT identical 0/12 |
| `syn/motion/small/reffile-grid2` | both-error (0/0 files) | both-error: p95 RMS – mm, MAT identical – |
| `syn/motion/small/reffile-ras` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/motion/small/refvol-0` | pass (18/18 files) | pass: p95 RMS 0.0560 mm, MAT identical 1/12 |
| `syn/motion/small/refvol-out-of-range` | both-error (0/0 files) | both-error: p95 RMS – mm, MAT identical – |
| `syn/motion/small/report` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/motion/small/sinc_final` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/motion/small/smooth0` | pass (18/18 files) | pass: p95 RMS 0.0585 mm, MAT identical 0/12 |
| `syn/motion/small/spline_final` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/motion/small/stages1` | pass (18/18 files) | pass: p95 RMS 0.0694 mm, MAT identical 0/12 |
| `syn/motion/small/stages4` | pass (18/18 files) | pass: p95 RMS 0.0539 mm, MAT identical 0/12 |
| `syn/motion/small/stats` | pass (21/21 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/motion/small/unknown-cost` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/motion/small/unknown-option` | both-error (0/0 files) | both-error: p95 RMS – mm, MAT identical – |
| `syn/orientation/oblique/default` | pass (18/18 files) | pass: p95 RMS 0.0488 mm, MAT identical 1/12 |
| `syn/orientation/oblique/reffile` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |
| `syn/orientation/ras-unflipped/default` | pass (18/18 files) | pass: p95 RMS 0.0515 mm, MAT identical 1/12 |
| `syn/orientation/ras-unflipped/reffile` | pass (18/18 files) | pass: p95 RMS 0.0660 mm, MAT identical 0/12 |
| `syn/orientation/ras/default` | pass (18/18 files) | pass: p95 RMS 0.0488 mm, MAT identical 1/12 |
| `syn/orientation/ras/reffile` | pass (18/18 files) | pass: p95 RMS 0.0713 mm, MAT identical 0/12 |

- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - FSL-01: 163 case(s) named it, 162 with changed values (largest difference inf), 28 with other changes (exit N -> N; files ['a.hdr.d/out.nii.gz', 'a.img.d/out.nii.gz']; files ['out.nii.gz_mean_reg.nii.gz', 'out.nii.gz_meanvol.nii.gz', 'out.nii.gz_sigma.nii.gz', 'out.nii.gz_variance.nii.gz']; files ['out_mcf.nii.gz.mat/MAT_N']; files ['out_mcf.nii.gz_mean_reg.nii.gz', 'out_mcf_mean_reg.nii.gz']; files ['out_mcf.nii.gz_meanvol.nii.gz', 'out_mcf.nii.gz_sigma.nii.gz', 'out_mcf.nii.gz_variance.nii.gz', 'out_mcf_meanvol.nii.gz']; out.nii.gz header; out_mcf.nii.gz header): `recorded/real/ds000005/default`, `recorded/real/ds000005/fmriprep`, `recorded/real/ds000005/fmriprep-repeat2`, `recorded/real/ds000005/fmriprep-repeat3`, `recorded/real/ds000005/fmriprep-reports`, `recorded/real/ds000005/opt-2d`, `recorded/real/ds000005/opt-bins64`, `recorded/real/ds000005/opt-cost-corratio` …
  - FSL-03: 7 case(s) named it, 7 with changed values (largest difference 371), 6 with other changes (out.nii.gz header; out_mcf.nii.gz header): `recorded/real/ds000005/opt-2d`, `recorded/syn/geometry/thin-slab/default`, `recorded/syn/geometry/thin-slab/fov10`, `recorded/syn/geometry/thin-slab/rotation2`, `recorded/syn/motion/small/2d`, `edge/fov0-2d`, `difftest/atanf-printed-params`
  - FSL-04: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit N -> N; files ['grefvol_out.nii.gz', 'out.nii.gz', 'out.par']): `edge/gdt`
  - FSL-05: 2 case(s) named it, 2 with changed values (largest difference 15): `recorded/real/ds000005/opt-scaling3`, `edge/scaling-hist`
  - FSL-06: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit N -> N; files ['out_mcf.nii.gz', 'out_mcf.nii.gz.mat/MAT_N', 'out_mcf.nii.gz.mat/MAT_N', 'out_mcf.nii.gz.mat/MAT_N']): `recorded/syn/motion/small/unknown-cost`
  - FSL-08: 10 case(s) named it, 9 with changed values (largest difference 992), 9 with other changes (files ['out.nii.gz_mean_reg.nii.gz', 'out.nii.gz_meanvol.nii.gz', 'out.nii.gz_sigma.nii.gz', 'out.nii.gz_variance.nii.gz']; files ['out_mcf.nii.gz_mean_reg.nii.gz', 'out_mcf_mean_reg.nii.gz']; files ['out_mcf.nii.gz_meanvol.nii.gz', 'out_mcf.nii.gz_sigma.nii.gz', 'out_mcf.nii.gz_variance.nii.gz', 'out_mcf_meanvol.nii.gz']; out.nii.gz header; out_mcf.nii.gz header): `recorded/real/ds000005/opt-meanvol`, `recorded/real/ds000005/opt-meanvol-noref`, `recorded/real/ds000005/opt-stats`, `recorded/syn/motion/small/meanvol`, `recorded/syn/motion/small/stats`, `edge/fsloutputtype-zst`, `edge/meanvol-reffile-sinc-stats`, `difftest/atanf-matrices-from-0` …
  - FSL-09: 143 case(s) named it, 143 with changed values (largest difference 1.07e+03), 23 with other changes (files ['a.hdr.d/out.nii.gz', 'a.img.d/out.nii.gz']; files ['out.nii.gz_mean_reg.nii.gz', 'out.nii.gz_meanvol.nii.gz', 'out.nii.gz_sigma.nii.gz', 'out.nii.gz_variance.nii.gz']; files ['out_mcf.nii.gz.mat/MAT_N']; files ['out_mcf.nii.gz_mean_reg.nii.gz', 'out_mcf_mean_reg.nii.gz']; files ['out_mcf.nii.gz_meanvol.nii.gz', 'out_mcf.nii.gz_sigma.nii.gz', 'out_mcf.nii.gz_variance.nii.gz', 'out_mcf_meanvol.nii.gz']; out.nii.gz header; out_mcf.nii.gz header): `recorded/real/ds000005/default`, `recorded/real/ds000005/fmriprep`, `recorded/real/ds000005/fmriprep-repeat2`, `recorded/real/ds000005/fmriprep-repeat3`, `recorded/real/ds000005/fmriprep-reports`, `recorded/real/ds000005/opt-2d`, `recorded/real/ds000005/opt-bins64`, `recorded/real/ds000005/opt-cost-corratio` …
  - FSL-10: 3 case(s) named it, 3 with changed values (largest difference 33), 3 with other changes (out_mcf.nii.gz header): `recorded/real/ds003345/default`, `recorded/real/ds003345/fmriprep`, `recorded/real/ds003345/fmriprep-reports`
  - FSL-13: 1 case(s) named it, 1 with changed values (largest difference 3), 1 with other changes (files ['out_mcf.nii.gz.mat/MAT_N']): `recorded/syn/cli/rmsrel-only`
  - FSL-14: 3 case(s) named it, 3 with changed values (largest difference inf), 1 with other changes (out.nii.gz header): `recorded/syn/content/nan/default`, `recorded/syn/content/nan/reffile`, `edge/nan-reffile`
  - FSL-20: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit N -> N): `edge/out-dir-missing`
  - FSL-21: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (exit N -> N; files ['out.nii.gz']): `edge/bins4-mutualinfo`, `edge/bins1-normmi`
  - FSL-59: 4 case(s) named it, 4 with changed values (largest difference 391), 4 with other changes (files ['out.nii.gz_mean_reg.nii.gz', 'out.nii.gz_meanvol.nii.gz', 'out.nii.gz_sigma.nii.gz', 'out.nii.gz_variance.nii.gz']; out.nii.gz header; out_mcf.nii.gz header): `recorded/real/ds000005/opt-nn_final`, `recorded/syn/motion/small/nn_final`, `recorded/syn/motion/small/sinc_final`, `edge/meanvol-reffile-sinc-stats`
  - FSL-60: 1 case(s) named it, 1 with changed values (largest difference 0.0133), 1 with other changes (files ['a.hdr.d/out.nii.gz', 'a.img.d/out.nii.gz']): `edge/img-in-output-directory`
- **FSL's own step, FSL 6.0.7.17 → 6.0.7.23** (both installations on the same arguments; the previous one's runs from the oracle-output cache or its recording where an earlier record made them): 179 of 197 cases identical (exit status, messages, every output); 18 changed: 4 by FSL-59 (FSL 6.0.7.23's mcflirt applies -sinc_final and -nn_final with trilinear interpolation (newimage 2601.0 keeps each volume's interpolation method from its first access); 6.0.7.17 used the method asked for): `recorded/real/ds000005/opt-nn_final`, `recorded/syn/motion/small/nn_final`, `recorded/syn/motion/small/sinc_final`, `edge/meanvol-reffile-sinc-stats`; 2 by FSL-60 (FSL 6.0.7.23 opens an image whose path holds '.img' or '.hdr' before its extension under another name (NewNifti 5.0.0's replaceSuffix)): `edge/img-in-directory`, `edge/img-in-output-directory`; 5 by arma-lapack (FSL 6.0.7.23's Armadillo 14.2.1 inverts 4 x 4 matrices and solves with them through LAPACK (OpenBLAS), where Armadillo 10.5.1 used its closed-form inverse: the last bits of values computed from nearly equal matrices change, and the sign of NaN parameters): `recorded/real/ds000005/opt-stages1`, `recorded/syn/content/nan/default`, `recorded/syn/content/nan/reffile`, `recorded/syn/motion/identical-copies/default`, `recorded/syn/motion/identical-copies/reffile-fudge`; 2 by compressed-input (FSL 6.0.7.23 reads zstd and bzip2 compressed images): `edge/input-zst`, `edge/input-bz2-plain-name`; 5 by output-types (FSL 6.0.7.23 adds the zstd and bzip2 output types (FSLOUTPUTTYPE NIFTI_ZST, NIFTI_BZ2, ...) and lists every type in the message for an unknown one): `edge/fsloutputtype-analyze`, `edge/fsloutputtype-zst`, `edge/fsloutputtype-pair-bz2`, `edge/fsloutputtype-nifti2-zst`, `edge/fsloutputtype-bogus`.

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same (an abort: the replica exits 1 after the same text) |
| Standard output and error | identical (paths of the run directories replaced; the replica's `larmorx-nc:` lines left out) |
| Files written | the same names |
| Every file | bit-identical (images decompressed; `descrip` left out) |
| D13 fix pass | a defect named exactly when the default's outputs differ |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| difftest | 6 | 6 | 0 | 0 | 0 |
| edge cases | 42 | 32 | 10 | 0 | 0 |
| fMRIPrep's calls | 21 | 21 | 0 | 0 | 0 |
| recorded: options (ds000005) | 25 | 24 | 1 | 0 | 0 |
| recorded: real | 30 | 29 | 1 | 0 | 0 |
| recorded: synthetic/cli | 11 | 6 | 5 | 0 | 0 |
| recorded: synthetic/content | 6 | 6 | 0 | 0 | 0 |
| recorded: synthetic/dtypes | 10 | 10 | 0 | 0 | 0 |
| recorded: synthetic/geometry | 10 | 10 | 0 | 0 | 0 |
| recorded: synthetic/motion | 36 | 31 | 5 | 0 | 0 |
| recorded: synthetic/orientation | 6 | 6 | 0 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `recorded/real/ds000005/opt-gdt` | real/ds000005/opt-gdt | mcflirt exits [-6]; the same from the replica. |
| `recorded/real/ds003763-truncated/default` | real/ds003763-truncated/default | mcflirt exits [-6]; the same from the replica. |
| `recorded/syn/cli/help` | syn/cli/help | mcflirt exits [1]; the same from the replica. |
| `recorded/syn/cli/no-arguments` | syn/cli/no-arguments | mcflirt exits [1]; the same from the replica. |
| `recorded/syn/cli/refvol-minus2` | syn/cli/refvol-minus2 | mcflirt exits [-6]; the same from the replica. |
| `recorded/syn/cli/stages0` | syn/cli/stages0 | mcflirt exits [-6]; the same from the replica. |
| `recorded/syn/cli/unknown-option-middle` | syn/cli/unknown-option-middle | mcflirt exits [255]; the same from the replica. |
| `recorded/syn/motion/small/no-fsloutputtype` | syn/motion/small/no-fsloutputtype | mcflirt exits [1]; the same from the replica. |
| `recorded/syn/motion/small/no-input` | syn/motion/small/no-input | mcflirt exits [2]; the same from the replica. |
| `recorded/syn/motion/small/reffile-grid2` | syn/motion/small/reffile-grid2 | mcflirt exits [-6]; the same from the replica. |
| `recorded/syn/motion/small/refvol-out-of-range` | syn/motion/small/refvol-out-of-range | mcflirt exits [-6]; the same from the replica. |
| `recorded/syn/motion/small/unknown-option` | syn/motion/small/unknown-option | mcflirt exits [255]; the same from the replica. |
| `edge/bins2-corratio` | the correlation ratio with 2 bins | mcflirt exits [-6]; the same from the replica. double free or corruption (out) |
| `edge/in-missing` | -in names no file | mcflirt exits [-6]; the same from the replica. Image Exception : #63 :: No image files match: nosuch Image Exception : #22 :: Failed to read volume nosuch Erro… |
| `edge/init-missing` | -init names no file | mcflirt exits [-6]; the same from the replica. Could not open matrix file nosuch.mat terminate called after throwing an instance of 'std::logic_error' what(): … |
| `edge/init-short` | -init with 3 numbers | mcflirt exits [-6]; the same from the replica. terminate called after throwing an instance of 'std::logic_error' what(): matrix multiplication: incompatible ma… |
| `edge/init-3x3` | -init with a 3 x 3 matrix | mcflirt exits [-6]; the same from the replica. terminate called after throwing an instance of 'std::logic_error' what(): matrix multiplication: incompatible ma… |
| `edge/out-dir-readonly` | -out in a directory without write permission | mcflirt exits [-6]; the same from the replica. Error! Could not create directory: <larmorx-testdata>/data/synthetic/hmc/motion/ro/out.mat. No write permission … |
| `edge/fsloutputtype-analyze` | FSLOUTPUTTYPE=ANALYZE | mcflirt exits [1]; the same from the replica. ERROR:: Unrecognised value (ANALYZE) of environment variable FSLOUTPUTTYPE Legal values are: NIFTI, NIFTI_PAIR, N… |
| `edge/img-in-directory` | an input in a directory named a.img.d: FSL 6.0.7.23 opens a.hdr.d/in.nii.gz (FSL-60) | mcflirt exits [-6]; the same from the replica. Image Exception : #22 :: Failed to read volume a.img.d/in.nii.gz Error : Error: cant open file a.hdr.d/in.nii.gz… |
| `edge/fsloutputtype-bogus` | an unknown FSLOUTPUTTYPE | mcflirt exits [1]; the same from the replica. ERROR:: Unrecognised value (BOGUS) of environment variable FSLOUTPUTTYPE Legal values are: NIFTI, NIFTI_PAIR, NIF… |
| `edge/rgb-input` | RGB input (a type FSL does not read) | mcflirt exits [-6]; the same from the replica. Image Exception : #8 :: Fslread: DT 128 not supported terminate called after throwing an instance of 'std::runti… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (1281f57a806f-dirty) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

The replica runs as a separate program, the FSL replica package's (`larmorx-nc`). It is compared with mcflirt in `reproduce` mode (D13); the default `fix` pass is checked against the reproduced run. fMRIPrep's own matrices come from its container's FSL (fsl-mcflirt 2111.0 with newer fsl-newimage and fsl-miscmaths) and are reported only. FSL version: 6.0.7.23 (`--fsl-version`, `LARMORX_FSL_VERSION`; installations: 6.0.7.23 `~/fsl-6.0.7.23`, 6.0.7.17 `~/fsl`). Each case also ran FSL 6.0.7.17 on the same arguments (from the oracle-output cache or its recording where an earlier record ran it): the check 'FSL's own step: 6.0.7.17 → 6.0.7.23' says whether FSL's own exit status, messages and outputs changed, and why.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `recorded/real/ds000005/default` | pass | 7/7 | real/ds000005/default |
| `recorded/real/ds000005/fmriprep` | pass | 7/7 | real/ds000005/fmriprep |
| `recorded/real/ds000005/fmriprep-repeat2` | pass | 7/7 | real/ds000005/fmriprep-repeat2 |
| `recorded/real/ds000005/fmriprep-repeat3` | pass | 7/7 | real/ds000005/fmriprep-repeat3 |
| `recorded/real/ds000005/fmriprep-reports` | pass | 7/7 | real/ds000005/fmriprep-reports |
| `recorded/real/ds000005/opt-2d` | pass | 7/7 | real/ds000005/opt-2d |
| `recorded/real/ds000005/opt-bins64` | pass | 7/7 | real/ds000005/opt-bins64 |
| `recorded/real/ds000005/opt-cost-corratio` | pass | 7/7 | real/ds000005/opt-cost-corratio |
| `recorded/real/ds000005/opt-cost-leastsquares` | pass | 7/7 | real/ds000005/opt-cost-leastsquares |
| `recorded/real/ds000005/opt-cost-mutualinfo` | pass | 7/7 | real/ds000005/opt-cost-mutualinfo |
| `recorded/real/ds000005/opt-cost-normcorr` | pass | 7/7 | real/ds000005/opt-cost-normcorr |
| `recorded/real/ds000005/opt-cost-normmi` | pass | 7/7 | real/ds000005/opt-cost-normmi |
| `recorded/real/ds000005/opt-cost-woods` | pass | 7/7 | real/ds000005/opt-cost-woods |
| `recorded/real/ds000005/opt-dof12` | pass | 7/7 | real/ds000005/opt-dof12 |
| `recorded/real/ds000005/opt-dof7` | pass | 7/7 | real/ds000005/opt-dof7 |
| `recorded/real/ds000005/opt-dof9` | pass | 7/7 | real/ds000005/opt-dof9 |
| `recorded/real/ds000005/opt-fudge` | pass | 7/7 | real/ds000005/opt-fudge |
| `recorded/real/ds000005/opt-gdt` | both-error | 6/6 | real/ds000005/opt-gdt |
| `recorded/real/ds000005/opt-meanvol` | pass | 7/7 | real/ds000005/opt-meanvol |
| `recorded/real/ds000005/opt-meanvol-noref` | pass | 7/7 | real/ds000005/opt-meanvol-noref |
| `recorded/real/ds000005/opt-nn_final` | pass | 7/7 | real/ds000005/opt-nn_final |
| `recorded/real/ds000005/opt-refvol0` | pass | 7/7 | real/ds000005/opt-refvol0 |
| `recorded/real/ds000005/opt-rotation2` | pass | 7/7 | real/ds000005/opt-rotation2 |
| `recorded/real/ds000005/opt-scaling3` | pass | 7/7 | real/ds000005/opt-scaling3 |
| `recorded/real/ds000005/opt-smooth0` | pass | 7/7 | real/ds000005/opt-smooth0 |
| `recorded/real/ds000005/opt-smooth2` | pass | 7/7 | real/ds000005/opt-smooth2 |
| `recorded/real/ds000005/opt-spline_final` | pass | 7/7 | real/ds000005/opt-spline_final |
| `recorded/real/ds000005/opt-stages1` | pass | 7/7 | real/ds000005/opt-stages1 |
| `recorded/real/ds000005/opt-stages2` | pass | 7/7 | real/ds000005/opt-stages2 |
| `recorded/real/ds000005/opt-stats` | pass | 7/7 | real/ds000005/opt-stats |
| `recorded/real/ds000117/default` | pass | 7/7 | real/ds000117/default |
| `recorded/real/ds000117/fmriprep` | pass | 7/7 | real/ds000117/fmriprep |
| `recorded/real/ds000117/fmriprep-reports` | pass | 7/7 | real/ds000117/fmriprep-reports |
| `recorded/real/ds000122/default` | pass | 7/7 | real/ds000122/default |
| `recorded/real/ds000122/fmriprep` | pass | 7/7 | real/ds000122/fmriprep |
| `recorded/real/ds000122/fmriprep-reports` | pass | 7/7 | real/ds000122/fmriprep-reports |
| `recorded/real/ds000210-rest-echo2/default` | pass | 7/7 | real/ds000210-rest-echo2/default |
| `recorded/real/ds000210-rest-echo2/fmriprep` | pass | 7/7 | real/ds000210-rest-echo2/fmriprep |
| `recorded/real/ds000210-rest-echo2/fmriprep-reports` | pass | 7/7 | real/ds000210-rest-echo2/fmriprep-reports |
| `recorded/real/ds000258-bigendian/default` | pass | 7/7 | real/ds000258-bigendian/default |
| `recorded/real/ds000258-bigendian/fmriprep` | pass | 7/7 | real/ds000258-bigendian/fmriprep |
| `recorded/real/ds000258-bigendian/fmriprep-reports` | pass | 7/7 | real/ds000258-bigendian/fmriprep-reports |
| `recorded/real/ds003345/default` | pass | 7/7 | real/ds003345/default |
| `recorded/real/ds003345/fmriprep` | pass | 7/7 | real/ds003345/fmriprep |
| `recorded/real/ds003345/fmriprep-reports` | pass | 7/7 | real/ds003345/fmriprep-reports |
| `recorded/real/ds003763-truncated/default` | both-error | 6/6 | real/ds003763-truncated/default |
| `recorded/real/ds005040/default` | pass | 7/7 | real/ds005040/default |
| `recorded/real/ds005040/fmriprep` | pass | 7/7 | real/ds005040/fmriprep |
| `recorded/real/ds005040/fmriprep-reports` | pass | 7/7 | real/ds005040/fmriprep-reports |
| `recorded/real/ds006010/default` | pass | 7/7 | real/ds006010/default |
| `recorded/real/ds006010/fmriprep` | pass | 7/7 | real/ds006010/fmriprep |
| `recorded/real/ds006010/fmriprep-reports` | pass | 7/7 | real/ds006010/fmriprep-reports |
| `recorded/real/ds006736/default` | pass | 7/7 | real/ds006736/default |
| `recorded/real/ds006736/fmriprep` | pass | 7/7 | real/ds006736/fmriprep |
| `recorded/real/ds006736/fmriprep-reports` | pass | 7/7 | real/ds006736/fmriprep-reports |
| `recorded/syn/cli/bare-input` | pass | 7/7 | syn/cli/bare-input |
| `recorded/syn/cli/dof5` | pass | 7/7 | syn/cli/dof5 |
| `recorded/syn/cli/help` | both-error | 6/6 | syn/cli/help |
| `recorded/syn/cli/help-with-input` | pass | 7/7 | syn/cli/help-with-input |
| `recorded/syn/cli/no-arguments` | both-error | 6/6 | syn/cli/no-arguments |
| `recorded/syn/cli/plots-only` | pass | 7/7 | syn/cli/plots-only |
| `recorded/syn/cli/refvol-minus1` | pass | 7/7 | syn/cli/refvol-minus1 |
| `recorded/syn/cli/refvol-minus2` | both-error | 6/6 | syn/cli/refvol-minus2 |
| `recorded/syn/cli/rmsrel-only` | pass | 7/7 | syn/cli/rmsrel-only |
| `recorded/syn/cli/stages0` | both-error | 6/6 | syn/cli/stages0 |
| `recorded/syn/cli/unknown-option-middle` | both-error | 6/6 | syn/cli/unknown-option-middle |
| `recorded/syn/content/background/default` | pass | 7/7 | syn/content/background/default |
| `recorded/syn/content/background/reffile` | pass | 7/7 | syn/content/background/reffile |
| `recorded/syn/content/nan/default` | pass | 7/7 | syn/content/nan/default |
| `recorded/syn/content/nan/reffile` | pass | 7/7 | syn/content/nan/reffile |
| `recorded/syn/content/ramp/reffile` | pass | 7/7 | syn/content/ramp/reffile |
| `recorded/syn/content/ramp/reffile-smooth0` | pass | 7/7 | syn/content/ramp/reffile-smooth0 |
| `recorded/syn/dtypes/float32/default` | pass | 7/7 | syn/dtypes/float32/default |
| `recorded/syn/dtypes/float32/reffile` | pass | 7/7 | syn/dtypes/float32/reffile |
| `recorded/syn/dtypes/float64/default` | pass | 7/7 | syn/dtypes/float64/default |
| `recorded/syn/dtypes/float64/reffile` | pass | 7/7 | syn/dtypes/float64/reffile |
| `recorded/syn/dtypes/int16-scaled/default` | pass | 7/7 | syn/dtypes/int16-scaled/default |
| `recorded/syn/dtypes/int16-scaled/reffile` | pass | 7/7 | syn/dtypes/int16-scaled/reffile |
| `recorded/syn/dtypes/uint16/default` | pass | 7/7 | syn/dtypes/uint16/default |
| `recorded/syn/dtypes/uint16/reffile` | pass | 7/7 | syn/dtypes/uint16/reffile |
| `recorded/syn/dtypes/uint8/default` | pass | 7/7 | syn/dtypes/uint8/default |
| `recorded/syn/dtypes/uint8/reffile` | pass | 7/7 | syn/dtypes/uint8/reffile |
| `recorded/syn/geometry/single-volume-3d/default` | pass | 7/7 | syn/geometry/single-volume-3d/default |
| `recorded/syn/geometry/single-volume-3d/reffile` | pass | 7/7 | syn/geometry/single-volume-3d/reffile |
| `recorded/syn/geometry/single-volume/default` | pass | 7/7 | syn/geometry/single-volume/default |
| `recorded/syn/geometry/single-volume/reffile` | pass | 7/7 | syn/geometry/single-volume/reffile |
| `recorded/syn/geometry/thin-slab/default` | pass | 7/7 | syn/geometry/thin-slab/default |
| `recorded/syn/geometry/thin-slab/fov10` | pass | 7/7 | syn/geometry/thin-slab/fov10 |
| `recorded/syn/geometry/thin-slab/rotation2` | pass | 7/7 | syn/geometry/thin-slab/rotation2 |
| `recorded/syn/geometry/tiny/default` | pass | 7/7 | syn/geometry/tiny/default |
| `recorded/syn/geometry/two-volumes/default` | pass | 7/7 | syn/geometry/two-volumes/default |
| `recorded/syn/geometry/two-volumes/reffile` | pass | 7/7 | syn/geometry/two-volumes/reffile |
| `recorded/syn/motion/identical-copies/default` | pass | 7/7 | syn/motion/identical-copies/default |
| `recorded/syn/motion/identical-copies/reffile` | pass | 7/7 | syn/motion/identical-copies/reffile |
| `recorded/syn/motion/identical-copies/reffile-fudge` | pass | 7/7 | syn/motion/identical-copies/reffile-fudge |
| `recorded/syn/motion/large/default` | pass | 7/7 | syn/motion/large/default |
| `recorded/syn/motion/large/reffile` | pass | 7/7 | syn/motion/large/reffile |
| `recorded/syn/motion/small/2d` | pass | 7/7 | syn/motion/small/2d |
| `recorded/syn/motion/small/default` | pass | 7/7 | syn/motion/small/default |
| `recorded/syn/motion/small/default-report` | pass | 7/7 | syn/motion/small/default-report |
| `recorded/syn/motion/small/dof12` | pass | 7/7 | syn/motion/small/dof12 |
| `recorded/syn/motion/small/dof7` | pass | 7/7 | syn/motion/small/dof7 |
| `recorded/syn/motion/small/fsloutputtype-nifti` | pass | 7/7 | syn/motion/small/fsloutputtype-nifti |
| `recorded/syn/motion/small/fudge` | pass | 7/7 | syn/motion/small/fudge |
| `recorded/syn/motion/small/init` | pass | 7/7 | syn/motion/small/init |
| `recorded/syn/motion/small/meanvol` | pass | 7/7 | syn/motion/small/meanvol |
| `recorded/syn/motion/small/nifti2` | pass | 7/7 | syn/motion/small/nifti2 |
| `recorded/syn/motion/small/nn_final` | pass | 7/7 | syn/motion/small/nn_final |
| `recorded/syn/motion/small/no-fsloutputtype` | both-error | 6/6 | syn/motion/small/no-fsloutputtype |
| `recorded/syn/motion/small/no-input` | both-error | 6/6 | syn/motion/small/no-input |
| `recorded/syn/motion/small/no-out` | pass | 7/7 | syn/motion/small/no-out |
| `recorded/syn/motion/small/out-no-ext` | pass | 7/7 | syn/motion/small/out-no-ext |
| `recorded/syn/motion/small/out-twice` | pass | 7/7 | syn/motion/small/out-twice |
| `recorded/syn/motion/small/reffile` | pass | 7/7 | syn/motion/small/reffile |
| `recorded/syn/motion/small/reffile-4d` | pass | 7/7 | syn/motion/small/reffile-4d |
| `recorded/syn/motion/small/reffile-grid2` | both-error | 6/6 | syn/motion/small/reffile-grid2 |
| `recorded/syn/motion/small/reffile-ras` | pass | 7/7 | syn/motion/small/reffile-ras |
| `recorded/syn/motion/small/refvol-0` | pass | 7/7 | syn/motion/small/refvol-0 |
| `recorded/syn/motion/small/refvol-out-of-range` | both-error | 6/6 | syn/motion/small/refvol-out-of-range |
| `recorded/syn/motion/small/report` | pass | 7/7 | syn/motion/small/report |
| `recorded/syn/motion/small/sinc_final` | pass | 7/7 | syn/motion/small/sinc_final |
| `recorded/syn/motion/small/smooth0` | pass | 7/7 | syn/motion/small/smooth0 |
| `recorded/syn/motion/small/spline_final` | pass | 7/7 | syn/motion/small/spline_final |
| `recorded/syn/motion/small/stages1` | pass | 7/7 | syn/motion/small/stages1 |
| `recorded/syn/motion/small/stages4` | pass | 7/7 | syn/motion/small/stages4 |
| `recorded/syn/motion/small/stats` | pass | 7/7 | syn/motion/small/stats |
| `recorded/syn/motion/small/unknown-cost` | pass | 7/7 | syn/motion/small/unknown-cost |
| `recorded/syn/motion/small/unknown-option` | both-error | 6/6 | syn/motion/small/unknown-option |
| `recorded/syn/orientation/oblique/default` | pass | 7/7 | syn/orientation/oblique/default |
| `recorded/syn/orientation/oblique/reffile` | pass | 7/7 | syn/orientation/oblique/reffile |
| `recorded/syn/orientation/ras-unflipped/default` | pass | 7/7 | syn/orientation/ras-unflipped/default |
| `recorded/syn/orientation/ras-unflipped/reffile` | pass | 7/7 | syn/orientation/ras-unflipped/reffile |
| `recorded/syn/orientation/ras/default` | pass | 7/7 | syn/orientation/ras/default |
| `recorded/syn/orientation/ras/reffile` | pass | 7/7 | syn/orientation/ras/reffile |
| `edge/gdt` | pass | 7/7 | -gdt with a relative -out: the gradient images are registered (FSL-04) |
| `edge/bins4-mutualinfo` | pass | 7/7 | mutual information with 4 bins |
| `edge/bins1-normmi` | pass | 7/7 | normalised mutual information with 1 bin |
| `edge/bins2-corratio` | both-error | 6/6 | the correlation ratio with 2 bins |
| `edge/in-missing` | both-error | 6/6 | -in names no file |
| `edge/init-missing` | both-error | 6/6 | -init names no file |
| `edge/init-short` | both-error | 6/6 | -init with 3 numbers |
| `edge/init-3x3` | both-error | 6/6 | -init with a 3 x 3 matrix |
| `edge/verbose20` | pass | 7/7 | -verbose 20: every cost evaluation's matrix printed |
| `edge/verbose20-report` | pass | 7/7 | -verbose 20 with -report (fixed notation after a matrix) |
| `edge/out-dir-missing` | pass | 7/7 | -out in a directory that does not exist (FSL-20) |
| `edge/out-dir-readonly` | both-error | 6/6 | -out in a directory without write permission |
| `edge/mat-exists` | pass | 7/7 | a file named out.mat exists: the matrices go to out.mat+ |
| `edge/fsloutputtype-pair` | pass | 7/7 | FSLOUTPUTTYPE=NIFTI_PAIR |
| `edge/fsloutputtype-pair-gz` | pass | 7/7 | FSLOUTPUTTYPE=NIFTI_PAIR_GZ |
| `edge/fsloutputtype-nifti2-gz` | pass | 7/7 | FSLOUTPUTTYPE=NIFTI2_GZ |
| `edge/fsloutputtype-analyze` | both-error | 6/6 | FSLOUTPUTTYPE=ANALYZE |
| `edge/fsloutputtype-zst` | pass | 7/7 | FSLOUTPUTTYPE=NIFTI_ZST: zstd outputs (FSL 6.0.7.23) |
| `edge/fsloutputtype-pair-bz2` | pass | 7/7 | FSLOUTPUTTYPE=NIFTI_PAIR_BZ2: a bzip2 pair (FSL 6.0.7.23) |
| `edge/fsloutputtype-nifti2-zst` | pass | 7/7 | FSLOUTPUTTYPE=NIFTI2_ZST (FSL 6.0.7.23) |
| `edge/input-zst` | pass | 7/7 | a zstd input, found by FSL's extension search too (FSL 6.0.7.23) |
| `edge/input-bz2-plain-name` | pass | 7/7 | a bzip2 file named .nii: read by its content (FSL 6.0.7.23) |
| `edge/img-in-directory` | both-error | 6/6 | an input in a directory named a.img.d: FSL 6.0.7.23 opens a.hdr.d/in.nii.gz (FSL-60) |
| `edge/img-in-output-directory` | pass | 7/7 | an output in a.img.d beside a directory a.hdr.d: FSL 6.0.7.23 writes it there (FSL-60) |
| `edge/fsloutputtype-bogus` | both-error | 6/6 | an unknown FSLOUTPUTTYPE |
| `edge/stages0-meanvol` | pass | 7/7 | -stages 0 with -meanvol |
| `edge/stages0-refvol` | pass | 7/7 | -stages 0 with a volume of the series as the reference |
| `edge/dof13` | pass | 7/7 | -dof 13: 12 used, with a message per volume |
| `edge/int8` | pass | 7/7 | int8 input (written as uint8: FSL-09) |
| `edge/int32` | pass | 7/7 | int32 input |
| `edge/analyze-input` | pass | 7/7 | ANALYZE 7.5 input (.hdr/.img) |
| `edge/rgb-input` | both-error | 6/6 | RGB input (a type FSL does not read) |
| `edge/nan-reffile` | pass | 7/7 | a reference with a NaN voxel (FSL-14) |
| `edge/refvol-last` | pass | 7/7 | the last volume as the reference |
| `edge/meanvol-reffile-sinc-stats` | pass | 7/7 | -meanvol from a reference file, sinc, -stats, -out with its extension |
| `edge/atanf-phantom-2` | pass | 7/7 | a moving phantom (seed 2) reaching atanf's aT[0] (random testing) |
| `edge/atanf-phantom-11` | pass | 7/7 | a moving phantom (seed 11) reaching atanf's aT[0] (random testing) |
| `edge/atanf-phantom-27` | pass | 7/7 | a moving phantom (seed 27) reaching atanf's aT[0] (random testing) |
| `edge/rotation0` | pass | 7/7 | -rotation 0 (infinite rotation tolerances) |
| `edge/smooth-negative` | pass | 7/7 | a negative -smooth |
| `edge/scaling-hist` | pass | 7/7 | -scaling and -hist (no effect: FSL-05) |
| `edge/fov0-2d` | pass | 7/7 | -2d with -fov 0 |
| `fmriprep/ds000005-sub-01-noreconall/task_mixedgamblestask_run_01` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds000005-sub-01-noreconall, task_mixedgamblestask_run_01) |
| `fmriprep/ds000005-sub-01-noreconall/task_mixedgamblestask_run_02` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds000005-sub-01-noreconall, task_mixedgamblestask_run_02) |
| `fmriprep/ds000005-sub-01-noreconall/task_mixedgamblestask_run_03` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds000005-sub-01-noreconall, task_mixedgamblestask_run_03) |
| `fmriprep/ds000228-sub-pixar008-noreconall/task_pixar` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds000228-sub-pixar008-noreconall, task_pixar) |
| `fmriprep/ds000256-sub-CTS203-noreconall/task_restbaseline` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds000256-sub-CTS203-noreconall, task_restbaseline) |
| `fmriprep/ds000256-sub-CTS203-noreconall/task_restfixationfixed` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds000256-sub-CTS203-noreconall, task_restfixationfixed) |
| `fmriprep/ds000256-sub-CTS203-noreconall/task_restfixationnofeedback` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds000256-sub-CTS203-noreconall, task_restfixationnofeedback) |
| `fmriprep/ds002422-sub-02-noreconall/task_arithm_run_01` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds002422-sub-02-noreconall, task_arithm_run_01) |
| `fmriprep/ds002422-sub-02-noreconall/task_arithm_run_02` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds002422-sub-02-noreconall, task_arithm_run_02) |
| `fmriprep/ds002422-sub-02-noreconall/task_rest` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds002422-sub-02-noreconall, task_rest) |
| `fmriprep/ds002603-sub-01-noreconall/task_wm_acq_Mb4Mm3Tr550` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds002603-sub-01-noreconall, task_wm_acq_Mb4Mm3Tr550) |
| `fmriprep/ds002603-sub-01-noreconall/task_wm_acq_SeqMm3Tr2000` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds002603-sub-01-noreconall, task_wm_acq_SeqMm3Tr2000) |
| `fmriprep/ds003592-sub-280-noreconall/ses_1_task_rest_echo_1` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds003592-sub-280-noreconall, ses_1_task_rest_echo_1) |
| `fmriprep/ds004884-sub-M2079-noreconall/ses_217_task_naming40_acq_epfid2_dir_AP_run_2` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds004884-sub-M2079-noreconall, ses_217_task_naming40_acq_epfid2_dir_AP_run_2) |
| `fmriprep/ds004884-sub-M2079-noreconall/ses_217_task_rest_acq_epfidp2p2_dir_AP_run_9` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds004884-sub-M2079-noreconall, ses_217_task_rest_acq_epfidp2p2_dir_AP_run_9) |
| `fmriprep/ds004958-sub-fsm64ps-noreconall/task_rest` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds004958-sub-fsm64ps-noreconall, task_rest) |
| `fmriprep/ds005003-sub-RS001-noreconall/ses_1_task_motor` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds005003-sub-RS001-noreconall, ses_1_task_motor) |
| `fmriprep/ds005003-sub-RS001-noreconall/ses_1_task_rest` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds005003-sub-RS001-noreconall, ses_1_task_rest) |
| `fmriprep/ds005040-sub-002-noreconall/task_clip_run_1` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds005040-sub-002-noreconall, task_clip_run_1) |
| `fmriprep/ds005752-sub-ON01016-noreconall/ses_01_task_rest_dir_forward` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds005752-sub-ON01016-noreconall, ses_01_task_rest_dir_forward) |
| `fmriprep/ds005752-sub-ON01016-noreconall/ses_01_task_rest_dir_reverse` | pass | 8/8 | fMRIPrep 25.2.6's mcflirt call (ds005752-sub-ON01016-noreconall, ses_01_task_rest_dir_reverse) |
| `difftest/atanf-default-call` | pass | 6/6 | the replica with fdlibm's comment value of atanf's aT[0] (0x3eaaaaaa, before 2026-10-25) differed from mcflirt (the def… |
| `difftest/atanf-matrices-from-0` | pass | 6/6 | the replica with fdlibm's comment value of atanf's aT[0] (0x3eaaaaaa, before 2026-10-25) differed from mcflirt (matrice… |
| `difftest/atanf-matrices-from-4` | pass | 6/6 | the replica with fdlibm's comment value of atanf's aT[0] (0x3eaaaaaa, before 2026-10-25) differed from mcflirt (matrice… |
| `difftest/atanf-par-only` | pass | 6/6 | the replica with fdlibm's comment value of atanf's aT[0] (0x3eaaaaaa, before 2026-10-25) differed from mcflirt (the .pa… |
| `difftest/atanf-printed-params` | pass | 6/6 | the replica with fdlibm's comment value of atanf's aT[0] (0x3eaaaaaa, before 2026-10-25) differed from mcflirt (a print… |
| `difftest/meanvol-name-notice` | pass | 6/6 | -meanvol with an -out extension: the corrected name of the -meanvol image (<stem>_mean_reg) is named in FSL-08's notice… |
