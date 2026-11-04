# Parity: topup (clean-room original)

**1 case(s) fail.** 183 cases on the `standard` tier: 94 pass, 66 rejected by both, 22 expected divergences, 1 failures.

- **Validated:** `larmorx fsl topup` (larmorx 0.0.1, crate larmorx-fsl: the clean-room original written from specs/topup.md, Apache-2.0)
- **Reference:** topup from FSL 6.0.7.23 (fsl-topup 2203.5), the conda build at `~/fsl-6.0.7.23`, one thread
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-04 on Linux x86_64, with `python -m larmorx_validation parity fsl-topup --tier standard`
- **Original programs:** 185 runs: 164 from the oracle cache, 21 run (2 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same |
| Files written | the same names |
| Standard output and error | the same lines, numbers to 4 significant digits; -v's costs: the same levels, the first to 4 digits, the final within 0.1 % |
| Headers, the log | identical |
| Field (Hz) inside the brain | p99 \|d\| <= 0.5, mean \|d\| <= 0.1, correlation >= 0.999 |
| Movements | translations within 0.05 mm, rotations within 0.001 rad |
| Corrected images inside the brain | relative \|d\| <= 0.001 at p99, mean \|d\| <= 0.001 of the mean |
| Jacobians, displacement fields | the field's bounds times t (and the voxel size) |
| FSL's own band | where measured (FSL at --nthr=4 against 1, from the cache): a metric beyond its threshold passes within that band, and says so |
| D13 fix pass | a defect named exactly when the default's outputs differ |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| at | 74 | 38 | 25 | 11 | 0 |
| cfg | 16 | 8 | 4 | 4 | 0 |
| debug | 5 | 5 | 0 | 0 | 0 |
| difftest | 7 | 6 | 0 | 1 | 0 |
| err | 39 | 3 | 36 | 0 | 0 |
| fmriprep | 2 | 2 | 0 | 0 | 0 |
| opt | 37 | 29 | 1 | 6 | 1 |
| thr | 3 | 3 | 0 | 0 | 0 |

## Failures

### `opt/fsl145-one-coefficient`

--splineorder=2 at --subsamp=2 with z shorter than one knot spacing (FSL-145: one coefficient, whose refit stops FSL; the default runs)

- **Exit status**: FSL 1, the original 0
- **Files written**: FSL only [], original only ['f.nii.gz', 'o_fieldcoef.nii.gz', 'o_movpar.txt']
- **Standard error (the original's own lines left out)**: 3 lines, the original 1; line 1: 'Error occurred when preparing to run topup' / ''

## Expected divergences

| Case | Reason |
|---|---|
| `opt/levels` | the movements estimated, not converged: FSL's rotation derivatives are not its cost's (findings 14). Outside the thresholds: c: relative difference inside the brain, 99th percentile 0.001247 (threshold 0.001) |
| `opt/scg-movements` | the movements estimated, not converged: FSL's rotation derivatives are not its cost's (findings 14). Outside the thresholds: o_movpar.txt: translations 0.2498 (threshold 0.05); o_movpar.txt: rotations 0.004863 (threshold 0.001); r_02.mat: translations 1.033 (threshold 0.20095694083360355) |
| `opt/four-volumes` | the movements estimated, not converged: FSL's rotation derivatives are not its cost's (findings 14). Outside the thresholds: c: relative difference inside the brain, 99th percentile 0.03348 (threshold 0.001); c: mean \|d\| / mean intensity inside the brain 0.001622 (threshold 0.001); c_mean: relative… |
| `opt/zero-readout` | the movements estimated, not converged: FSL's rotation derivatives are not its cost's (findings 14). Outside the thresholds: c: relative difference inside the brain, 99th percentile 0.06429 (threshold 0.001); c: mean \|d\| / mean intensity inside the brain 0.004861 (threshold 0.001); c_mean: relative… |
| `opt/lambda-negative` | FSL-126 reproduced: a negative weight rewards a rough field, both minimisations diverge. Outside the thresholds: --fout (Hz): \|d\| inside the brain, 99th percentile 200.1 (threshold 0.5); --fout (Hz): \|d\| inside the brain, mean 53.49 (threshold 0.1); --fout (Hz): correlation inside the brain 0.7821 … |
| `opt/warpres-negative` | FSL-124: refused; FSL is not run: its knot spacing of about 4e9 voxels needs more memory than the machine holds (2026-11-03: killed in a 24 GiB memory box) |
| `cfg/b02b0` | the movements estimated, not converged: FSL's rotation derivatives are not its cost's (findings 14). Outside the thresholds: c: relative difference inside the brain, 99th percentile 0.002251 (threshold 0.001); c_mean: relative difference inside the brain, 99th percentile 0.0015 (threshold 0.001) |
| `cfg/b02b0_2` | the movements estimated, not converged: FSL's rotation derivatives are not its cost's (findings 14). Outside the thresholds: c: relative difference inside the brain, 99th percentile 0.002251 (threshold 0.001); c_mean: relative difference inside the brain, 99th percentile 0.0015 (threshold 0.001) |
| `cfg/b02b0_4` | the movements estimated, not converged: FSL's rotation derivatives are not its cost's (findings 14). Outside the thresholds: c: relative difference inside the brain, 99th percentile 0.001684 (threshold 0.001); c_mean: relative difference inside the brain, 99th percentile 0.001108 (threshold 0.001) |
| `cfg/b02b0_7T` | the movements estimated, not converged: FSL's rotation derivatives are not its cost's (findings 14). Outside the thresholds: c: relative difference inside the brain, 99th percentile 0.006691 (threshold 0.001); c_mean: relative difference inside the brain, 99th percentile 0.004121 (threshold 0.001) |
| `at/lsr-long` | FSL-133 is not reproduced by the original: FSL's wrap-around of lines whose length does not divide 2^32 (96 voxels here; at/lsr-crop's lines of 30 differ at their ends, outside the brain) is not known from the spec and the black-box probes (findings 21); the original wraps circularly in both modes … |
| `at/vb2d-long` | FSL-133 is not reproduced by the original: FSL's wrap-around of lines whose length does not divide 2^32 (96 voxels here; at/lsr-crop's lines of 30 differ at their ends, outside the brain) is not known from the spec and the black-box probes (findings 21); the original wraps circularly in both modes … |
| `at/vb3d-long` | FSL-133 is not reproduced by the original: FSL's wrap-around of lines whose length does not divide 2^32 (96 voxels here; at/lsr-crop's lines of 30 differ at their ends, outside the brain) is not known from the spec and the black-box probes (findings 21); the original wraps circularly in both modes … |
| `at/vb4d-long` | FSL-133 is not reproduced by the original: FSL's wrap-around of lines whose length does not divide 2^32 (96 voxels here; at/lsr-crop's lines of 30 differ at their ends, outside the brain) is not known from the spec and the black-box probes (findings 21); the original wraps circularly in both modes … |
| `at/vb3d-aniso` | FSL-143 reproduced: FSL's smoothness prior is indefinite, so its iterations diverge (lambda below 0 in the first iterations, then phi and lambda fall towards 1e-23 to 1e-206, the image following their ratio): the divergence amplifies the moved images' float-level differences in the printed values (… |
| `at/vb2d-aniso` | the printed phi and lambda of the top slices (few voxels in the brain) part from FSL's in the 4th digit during the iterations (8e-4) and meet again (their final values within 1e-5): the iterations amplify the moved images' float-level differences there; the image within 1e-5 (p99), and the same aga… |
| `at/vb4d-four` | FSL-143 reproduced: FSL's smoothness prior is indefinite, so its iterations diverge (lambda below 0 in the first iterations, then phi and lambda fall towards 1e-23 to 1e-206, the image following their ratio): the divergence amplifies the moved images' float-level differences in the printed values (… |
| `at/vb3d-three` | FSL-143 reproduced: FSL's smoothness prior is indefinite, so its iterations diverge (lambda below 0 in the first iterations, then phi and lambda fall towards 1e-23 to 1e-206, the image following their ratio): the divergence amplifies the moved images' float-level differences in the printed values (… |
| `at/vb-other-grid` | FSL-134: refused; FSL is not run: FSL reads the field's lines past their end (undefined behaviour) |
| `at/jac-other-shape` | FSL-134: refused by the original in both modes: FSL lays a field of as many voxels as the images in another shape on the images' grid; how it does is not known from the spec and the black-box runs (findings 22) |
| `at/lsr-other-grid` | FSL-134: refused; FSL is not run: FSL reads the field's lines past their end (undefined behaviour) |
| `difftest/fsl145-short-axis` | the movements' rotations differ (finding 14: FSL's movement derivatives); the coefficients' layout and every exact check agree. Outside the thresholds: o_movpar.txt: rotations 0.002174 (threshold 0.001) |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `opt/z-phase-encoding` | phase encoding along z | topup exits 1; the same from the original. Error occurred when preparing to run topup Exception thrown with message: Topup::TopupScan::TopupScan: third element… |
| `cfg/not-found` | a configuration that does not exist | topup exits 1; the same from the original. Cannot find config-file nothere |
| `cfg/not-found-path` | a configuration with a path that does not exist | topup exits 1; the same from the original. Cannot find config-file ./nothere |
| `cfg/fsldir-unset` | FSLDIR unset and a name to look up there | topup exits 1; the same from the original. basic_string: construction from null is not valid |
| `cfg/bad-option` | an unknown option in the configuration | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/no-arguments` | no arguments: the usage | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/help` | -h: the usage | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/help-long` | --help with the compulsory options set | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/unknown-option` | an unknown option | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/missing-value` | an option without its value | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/no-imain` | no --imain | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/imain-missing` | an --imain that does not exist | topup exits 1; the same from the original. Image Exception : #63 :: No image files match: nothere Image Exception : #22 :: Failed to read volume nothere.nii.gz… |
| `err/imain-text` | an --imain that is not an image | topup exits 1; the same from the original. Image Exception : #63 :: No image files match: acq.txt Image Exception : #22 :: Failed to read volume acq.txt Error … |
| `err/datain-and-acqp` | both --datain and --acqp | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/no-datain` | neither --datain nor --acqp | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/datain-missing` | a --datain that does not exist | topup exits 1; the same from the original. Could not open matrix file nothere.txt Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --d… |
| `err/datain-five-columns` | a --datain of five columns | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/datain-text` | a --datain of words | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/datain-norm` | a phase-encoding vector not of unit length | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/datain-rows` | three rows for two volumes | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/acqp-rows` | three rows for two volumes, given with --acqp | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/readout-long` | a read-out time above 0.2 s | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/readout-two-zero` | two read-out times of 0 | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/levels-subsamp` | --subsamp of three levels against two | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/levels-minmet` | --minmet of three levels against two | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/minmet-2` | --minmet=2 | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/subsamp-incompatible` | --subsamp=3 on a 32-voxel axis | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/subsamp-0` | --subsamp=0 | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/subsamp-mixed` | subsamplings 3 and 2 on a 30 x 30 x 18 image (each divides it; the regridded grid of 33 does not divide by 2) | topup exits 1; the same from the original. Error occurred when preparing to run level 2 of topup Exception thrown with message: Topup::TopupScan::SubSample: in… |
| `err/numprec` | an unknown --numprec | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/regmod` | an unknown --regmod | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/interp` | an unknown --interp | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/splineorder-4` | --splineorder=4 | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/splineorder-negative` | --splineorder=-1 | topup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file w… |
| `err/debug-4` | --debug=4 | topup exits 1; the same from the original. Error occurred when preparing to run topup Exception thrown with message: Topup::TopupCF::SetDebug: Debug level out … |
| `err/debug-negative` | --debug=-1 | topup exits 1; the same from the original. Error occurred when preparing to run topup Exception thrown with message: Topup::TopupCF::SetDebug: Debug level out … |
| `err/out-no-directory` | --out in a directory that does not exist | topup exits 1; the same from the original. Error occured while writing output from topup Exception thrown with message: Error: cant open file nodir/o_fieldcoef… |
| `err/iout-no-directory` | --iout in a directory that does not exist | topup exits 1; the same from the original. Error occured while writing output from topup Exception thrown with message: Error: cant open file nodir/c.nii.gz |
| `err/profile-no-directory` | --profile with --out in a directory that does not exist | topup exits 1; the same from the original. |
| `err/outputtype-invalid` | FSLOUTPUTTYPE=BOGUS | topup exits 1; the same from the original. ERROR:: Unrecognised value (BOGUS) of environment variable FSLOUTPUTTYPE Legal values are: NIFTI, NIFTI_PAIR, NIFTI2… |
| `err/outputtype-unset` | FSLOUTPUTTYPE unset | topup exits 1; the same from the original. ERROR:: Environment variable FSLOUTPUTTYPE is not set! Please make sure that the appropriate configuration file is s… |
| `at/vb-one-direction` | --method=vb3D of two images with the same phase encoding (FSL's message) | applytopup exits -6; the same from the original. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Invalid combin… |
| `at/vb-no-movements` | --method=vb4D with a field image, which has no movements (FSL-135) | applytopup exits -6; the same from the original. terminate called after throwing an instance of 'TOPUP::TopupFileIOException' what(): TopupFileIO::TopupFileRea… |
| `at/no-arguments` | no arguments: the compulsory options and the usage | applytopup exits 1; the same from the original. *************************************************** The following COMPULSORY options have not been set: -i,--im… |
| `at/help` | -h: the usage | applytopup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) applytopup (Version 1.0) Copyright(c) 2009, University of Oxford (Jesper Anderss… |
| `at/unknown-option` | an unknown option | applytopup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) applytopup (Version 1.0) Copyright(c) 2009, University of Oxford (Jesper Anderss… |
| `at/invert` | --invert (declared, never added to the parser) | applytopup exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) applytopup (Version 1.0) Copyright(c) 2009, University of Oxford (Jesper Anderss… |
| `at/missing-out` | no --out | applytopup exits 1; the same from the original. *************************************************** The following COMPULSORY options have not been set: -o,--ou… |
| `at/interp-unknown` | an unknown --interp | applytopup exits -6; the same from the original. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Unknown interp… |
| `at/lists-mismatch` | two images and one index | applytopup exits -6; the same from the original. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Mismatched --i… |
| `at/datatype-unknown` | an unknown --datatype | applytopup exits -6; the same from the original. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Unknown data t… |
| `at/datatype-preserve-missing` | --datatype=preserve of an image that does not exist | applytopup exits -6; the same from the original. Image Exception : #63 :: No image files match: nothere terminate called after throwing an instance of 'std::ru… |
| `at/imain-missing` | an image that does not exist | applytopup exits -6; the same from the original. Image Exception : #63 :: No image files match: nothere Image Exception : #22 :: Failed to read volume nothere.… |
| `at/images-mismatch` | images of different sizes | applytopup exits -6; the same from the original. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Mismatched inp… |
| `at/datain-missing` | a --datain that does not exist | applytopup exits -6; the same from the original. Could not open matrix file nothere.txt terminate called after throwing an instance of 'TOPUP::TopupFileIOExcep… |
| `at/datain-norm` | a phase-encoding vector not of unit length | applytopup exits -6; the same from the original. terminate called after throwing an instance of 'TOPUP::TopupFileIOException' what(): TopupFileIO::TopupDatafil… |
| `at/inindex-zero` | an index of 0 | applytopup exits -6; the same from the original. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Invalid entry … |
| `at/inindex-beyond` | an index beyond --datain's rows | applytopup exits -6; the same from the original. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Invalid entry … |
| `at/topup-extension` | --topup with an extension | applytopup exits -6; the same from the original. terminate called after throwing an instance of 'TOPUP::TopupFileIOException' what(): TopupFileIO::TopupFileRea… |
| `at/topup-missing` | a --topup that does not exist | applytopup exits -6; the same from the original. terminate called after throwing an instance of 'TOPUP::TopupFileIOException' what(): TopupFileIO::TopupFileRea… |
| `at/lsr-oblique` | --method=lsr with an oblique phase encoding | applytopup exits -6; the same from the original. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Invalid phase-… |
| `at/lsr-same` | --method=lsr of two images with the same phase encoding | applytopup exits -6; the same from the original. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Invalid combin… |
| `at/lsr-one` | --method=lsr of one image | applytopup exits -6; the same from the original. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Invalid combin… |
| `at/outputtype-invalid` | FSLOUTPUTTYPE=BOGUS | applytopup exits 1; the same from the original. ERROR:: Unrecognised value (BOGUS) of environment variable FSLOUTPUTTYPE Legal values are: NIFTI, NIFTI_PAIR, N… |
| `at/lsr-no-movements` | --method=lsr with a field image, which has no movements (FSL-135) | applytopup exits -6; the same from the original. terminate called after throwing an instance of 'TOPUP::TopupFileIOException' what(): TopupFileIO::TopupFileRea… |
| `at/jac-other-grid` | --method=jac with a field on another grid (FSL-134: another number of voxels) | applytopup exits -6; the same from the original. Image Exception : #2 :: Attempted to copydata with non-matching sizes terminate called after throwing an insta… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (be3db916dce3) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

The cases are the replica suite's (`fsl-topup-replica`); FSL's runs are served from the oracle cache under that suite's keys. The original is compared in `reproduce` mode (D13) and its default `fix` pass against the reproduced run. The options the original does not take yet are skipped until its next sub-tasks.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep/ds001600-sub-1-noreconall` | pass | 38/38 | fMRIPrep's field-map estimation call (sdcflows' b02b0.cnf) |
| `fmriprep/ds005040-sub-002-noreconall` | pass | 38/38 | fMRIPrep's field-map estimation call (sdcflows' b02b0.cnf) |
| `debug/one-level` | pass | 25/25 | one level (subsampling 2, 3 Levenberg-Marquardt iterations) with --debug=2 and -v: every cost-function evaluation's deb… |
| `debug/movements` | pass | 25/25 | one level with the movements estimated (five parameters of the reversed scan: the translation along the phase encoding … |
| `debug/hessian` | pass | 12/12 | --debug=3 on the half-resolution pair: one iteration's every debug file, the Hessian's text included |
| `debug/trace` | pass | 13/13 | --trace: every routine's entry and exit printed, two levels |
| `debug/profile` | pass | 11/11 | --profile: the levels' durations in <out>.topup_profile_topup.cpp_main.txt (the clock's numbers masked) |
| `opt/defaults` | pass | 12/12 | every default (warpres 10, fwhm 8, five LM iterations, movements) |
| `opt/levels` | expected-divergence | 46/47 | two levels: subsampling 2 with Levenberg-Marquardt and the movements, then full resolution with scaled conjugate gradie… |
| `opt/numprec-float` | pass | 16/16 | --numprec=float |
| `opt/interp-linear` | pass | 18/18 | --interp=linear |
| `opt/membrane-lm` | pass | 16/16 | --regmod=membrane_energy with Levenberg-Marquardt (the energy's Hessian) |
| `opt/membrane-scg` | pass | 16/16 | --regmod=membrane_energy with scaled conjugate gradients (the energy's gradient) |
| `opt/splineorder-2` | pass | 16/16 | --splineorder=2 |
| `opt/fsl145-short-axis` | pass | 16/16 | --splineorder=2 with z shorter than two knot spacings (FSL-145: 2 coefficients) |
| `opt/fsl145-one-coefficient` | fail | 4/7 | --splineorder=2 at --subsamp=2 with z shorter than one knot spacing (FSL-145: one coefficient, whose refit stops FSL; t… |
| `opt/scale` | pass | 18/18 | --scale=1: each volume scaled to the mean |
| `opt/ssqlambda-0` | pass | 16/16 | --ssqlambda=0: lambda as given |
| `opt/regrid-0` | pass | 16/16 | --regrid=0: the image's own grid, two levels |
| `opt/scg` | pass | 12/12 | --minmet=1 without movements |
| `opt/scg-movements` | expected-divergence | 12/16 | --minmet=1 with the movements: FSL's warning |
| `opt/acqp` | pass | 12/12 | --acqp instead of --datain |
| `opt/datain-transposed` | pass | 12/12 | an acquisition-parameter file of four rows |
| `opt/x-phase-encoding` | pass | 34/34 | phase encoding along x (the translation along x left out), the movements |
| `opt/four-volumes` | expected-divergence | 41/50 | four volumes along x and y: six movement parameters for each moving scan |
| `opt/fsl146-four-axes` | pass | 16/16 | four volumes along y and x without the movements (FSL-146: each axis' Jacobian term of the gradient over the volumes en… |
| `opt/oblique-phase-encoding` | pass | 24/24 | a phase-encoding vector along x and y (the Jacobian of both displacements) |
| `opt/readout-times` | pass | 16/16 | different read-out times |
| `opt/zero-readout` | expected-divergence | 12/22 | one read-out time of 0 (a synthesised undistorted image) |
| `opt/same-direction` | pass | 16/16 | both volumes with the same phase encoding |
| `opt/z-phase-encoding` | both-error | 5/5 | phase encoding along z |
| `opt/int16` | pass | 18/18 | a 16-bit integer image |
| `opt/one-volume` | pass | 12/12 | a single 3D volume |
| `opt/no-out` | pass | 12/12 | no --out: the base name from --imain |
| `opt/intent` | pass | 22/22 | an image with intent parameters: the field's header |
| `opt/nifti` | pass | 16/16 | FSLOUTPUTTYPE=NIFTI |
| `opt/nifti2` | pass | 22/22 | FSLOUTPUTTYPE=NIFTI2_GZ |
| `opt/nifti-pair` | pass | 24/24 | FSLOUTPUTTYPE=NIFTI_PAIR |
| `opt/iout-extension` | pass | 18/18 | --iout with .nii.gz: the mean as <name>.nii.gz_mean |
| `opt/nthr-0` | pass | 12/12 | --nthr=0: the default thread count (FSL_NUM_THREADS unset: one) |
| `opt/warpres-0` | pass | 12/12 | --warpres=0: a knot every voxel |
| `opt/lambda-negative` | expected-divergence | 9/17 | a negative --lambda |
| `opt/fwhm-negative` | pass | 17/17 | a negative --fwhm |
| `opt/warpres-negative` | expected-divergence | 2/2 | a negative --warpres |
| `cfg/b02b0` | expected-divergence | 20/22 | FSL's b02b0.cnf by name, one iteration per level |
| `cfg/b02b0_1` | pass | 22/22 | FSL's b02b0_1.cnf by name, one iteration per level |
| `cfg/b02b0_2` | expected-divergence | 20/22 | FSL's b02b0_2.cnf by name, one iteration per level |
| `cfg/b02b0_4` | expected-divergence | 20/22 | FSL's b02b0_4.cnf by name, one iteration per level |
| `cfg/b02b0_7T` | expected-divergence | 20/22 | FSL's b02b0_7T.cnf by name, one iteration per level |
| `cfg/test_acqp` | pass | 22/22 | FSL's test_acqp.cnf by name, one iteration per level |
| `cfg/b02b0-extension` | pass | 12/12 | FSL's b02b0.cnf by name with its extension |
| `cfg/local` | pass | 16/16 | a local configuration file |
| `cfg/local-no-extension` | pass | 16/16 | a local configuration named without .cnf |
| `cfg/command-line-wins` | pass | 16/16 | options on the command line take precedence |
| `cfg/directory` | pass | 12/12 | a directory as the configuration (read as empty) |
| `cfg/not-found` | both-error | 4/4 | a configuration that does not exist |
| `cfg/not-found-path` | both-error | 4/4 | a configuration with a path that does not exist |
| `cfg/fsldir-unset` | both-error | 5/5 | FSLDIR unset and a name to look up there |
| `cfg/bad-option` | both-error | 4/4 | an unknown option in the configuration |
| `cfg/long-line` | pass | 16/16 | a comment line of more than 1023 characters before two settings |
| `err/no-arguments` | both-error | 4/4 | no arguments: the usage |
| `err/help` | both-error | 4/4 | -h: the usage |
| `err/help-long` | both-error | 4/4 | --help with the compulsory options set |
| `err/unknown-option` | both-error | 4/4 | an unknown option |
| `err/missing-value` | both-error | 4/4 | an option without its value |
| `err/no-imain` | both-error | 4/4 | no --imain |
| `err/imain-missing` | both-error | 4/4 | an --imain that does not exist |
| `err/imain-text` | both-error | 4/4 | an --imain that is not an image |
| `err/datain-and-acqp` | both-error | 4/4 | both --datain and --acqp |
| `err/no-datain` | both-error | 4/4 | neither --datain nor --acqp |
| `err/datain-missing` | both-error | 4/4 | a --datain that does not exist |
| `err/datain-five-columns` | both-error | 4/4 | a --datain of five columns |
| `err/datain-text` | both-error | 4/4 | a --datain of words |
| `err/datain-norm` | both-error | 4/4 | a phase-encoding vector not of unit length |
| `err/datain-rows` | both-error | 4/4 | three rows for two volumes |
| `err/acqp-rows` | both-error | 5/5 | three rows for two volumes, given with --acqp |
| `err/readout-long` | both-error | 4/4 | a read-out time above 0.2 s |
| `err/readout-two-zero` | both-error | 4/4 | two read-out times of 0 |
| `err/levels-subsamp` | both-error | 4/4 | --subsamp of three levels against two |
| `err/levels-minmet` | both-error | 4/4 | --minmet of three levels against two |
| `err/minmet-2` | both-error | 4/4 | --minmet=2 |
| `err/subsamp-incompatible` | both-error | 4/4 | --subsamp=3 on a 32-voxel axis |
| `err/subsamp-0` | both-error | 4/4 | --subsamp=0 |
| `err/subsamp-mixed` | both-error | 6/6 | subsamplings 3 and 2 on a 30 x 30 x 18 image (each divides it; the regridded grid of 33 does not divide by 2) |
| `err/numprec` | both-error | 4/4 | an unknown --numprec |
| `err/regmod` | both-error | 4/4 | an unknown --regmod |
| `err/interp` | both-error | 4/4 | an unknown --interp |
| `err/splineorder-4` | both-error | 4/4 | --splineorder=4 |
| `err/splineorder-negative` | both-error | 4/4 | --splineorder=-1 |
| `err/debug-4` | both-error | 5/5 | --debug=4 |
| `err/debug-negative` | both-error | 5/5 | --debug=-1 |
| `err/miter-negative` | pass | 13/13 | --miter=-1 (about 4e9 iterations) |
| `err/out-no-directory` | both-error | 5/5 | --out in a directory that does not exist |
| `err/iout-no-directory` | both-error | 11/11 | --iout in a directory that does not exist |
| `err/logout-no-directory` | pass | 11/11 | --logout in a directory that does not exist |
| `err/rbmout-no-directory` | pass | 12/12 | --rbmout in a directory that does not exist |
| `err/profile-no-directory` | both-error | 5/5 | --profile with --out in a directory that does not exist |
| `err/outputtype-invalid` | both-error | 5/5 | FSLOUTPUTTYPE=BOGUS |
| `err/outputtype-unset` | both-error | 5/5 | FSLOUTPUTTYPE unset |
| `thr/nthr-4` | pass | 22/22 | --nthr=4 (the spline field's threaded products and the Hessians' threaded sparse products) against --nthr=1 |
| `thr/fsl-num-threads` | pass | 22/22 | FSL_NUM_THREADS=4 with --nthr=1 (the default thread count of the Hessian's sparse matrices) against it unset |
| `thr/openblas` | pass | 22/22 | FSL_SKIP_GLOBAL=1 and OPENBLAS_NUM_THREADS=4 (OpenBLAS's own threads: its dot product of more than 10,000 values) again… |
| `at/lsr-y` | pass | 8/8 | least-squares restoration of the pair along y (the default method, spline interpolation, the movements) |
| `at/lsr-4d` | pass | 8/8 | least-squares restoration of two volumes per phase-encoding direction |
| `at/lsr-x` | pass | 8/8 | least-squares restoration along x |
| `at/lsr-four` | pass | 7/7 | least-squares restoration of two collections (along y and along x) |
| `at/lsr-three` | pass | 8/8 | least-squares restoration of three images in one collection |
| `at/lsr-trilinear` | pass | 8/8 | --interp=trilinear |
| `at/lsr-crop` | pass | 8/8 | least-squares restoration of lines of 30 voxels (a length that does not divide 2^32) |
| `at/lsr-zero-field` | pass | 8/8 | least-squares restoration with a field of zeros (band systems: Armadillo's band solver) |
| `at/vb2d-y` | pass | 9/9 | --method=vb2D: phi and lambda per slice |
| `at/vb3d-y` | pass | 9/9 | --method=vb3D: phi and lambda per volume |
| `at/vb4d-y` | pass | 9/9 | --method=vb4D: one phi and lambda |
| `at/lsr-long` | expected-divergence | 7/8 | --method=lsr on lines of 96 voxels along y (the long-line pair) |
| `at/vb2d-long` | expected-divergence | 7/9 | --method=vb2D on lines of 96 voxels along y (the long-line pair) |
| `at/vb3d-long` | expected-divergence | 7/9 | --method=vb3D on lines of 96 voxels along y (the long-line pair) |
| `at/vb4d-long` | expected-divergence | 7/9 | --method=vb4D on lines of 96 voxels along y (the long-line pair) |
| `at/vb3d-4d` | pass | 9/9 | --method=vb3D of two volumes (each on its own) |
| `at/vb4d-4d` | pass | 9/9 | --method=vb4D of two volumes (together) |
| `at/vb2d-x` | pass | 9/9 | --method=vb2D along x |
| `at/vb3d-x` | pass | 9/9 | --method=vb3D along x |
| `at/vb3d-aniso` | expected-divergence | 8/9 | --method=vb3D with voxels of 6 x 4 mm in plane (the prior's scale factors) |
| `at/vb2d-aniso` | expected-divergence | 8/9 | --method=vb2D with voxels of 6 x 4 mm in plane |
| `at/vb4d-four` | expected-divergence | 6/7 | --method=vb4D of two collections (along y and along x) |
| `at/vb3d-three` | expected-divergence | 8/9 | --method=vb3D of three images in one collection (FSL-142: counted as two) |
| `at/vb2d-three` | pass | 9/9 | --method=vb2D of three images in one collection |
| `at/vb-one-direction` | both-error | 4/4 | --method=vb3D of two images with the same phase encoding (FSL's message) |
| `at/vb-no-movements` | both-error | 5/5 | --method=vb4D with a field image, which has no movements (FSL-135) |
| `at/vb-other-grid` | expected-divergence | 2/2 | --method=vb2D with a field on another grid (FSL-134) |
| `at/jac-y` | pass | 8/8 | Jacobian modulation of one image |
| `at/jac-pair` | pass | 8/8 | Jacobian modulation of the pair, averaged |
| `at/jac-4d` | pass | 8/8 | Jacobian modulation of two volumes |
| `at/jac-trilinear` | pass | 8/8 | --method=jac --interp=trilinear |
| `at/jac-x` | pass | 8/8 | Jacobian modulation along x |
| `at/jac-quadratic` | pass | 8/8 | a field of quadratic splines |
| `at/jac-field` | pass | 8/8 | --topup naming a field image (topup's --fout): no movements |
| `at/jac-field-aniso` | pass | 8/8 | a field image and voxels of 6 x 4 mm in plane (the Jacobian of the y displacements) |
| `at/jac-no-movpar` | pass | 8/8 | coefficients without their _movpar.txt (no movements) |
| `at/jac-oblique` | pass | 8/8 | a phase-encoding vector along x and y |
| `at/jac-z` | pass | 8/8 | phase encoding along z (FSL-144: FSL leaves the z component out silently; the default refuses) |
| `at/datatype-preserve` | pass | 8/8 | --datatype=preserve |
| `at/datatype-char` | pass | 8/8 | --datatype=char |
| `at/datatype-short` | pass | 8/8 | --datatype=short |
| `at/datatype-int` | pass | 8/8 | --datatype=int |
| `at/datatype-float` | pass | 8/8 | --datatype=float |
| `at/datatype-double` | pass | 8/8 | --datatype=double |
| `at/datatype-preserve-int16` | pass | 8/8 | --datatype=preserve of a 16-bit image |
| `at/datatype-preserve-int8` | pass | 8/8 | --datatype=preserve of an 8-bit signed image |
| `at/short-options` | pass | 8/8 | the short options |
| `at/nifti` | pass | 8/8 | FSLOUTPUTTYPE=NIFTI |
| `at/no-arguments` | both-error | 4/4 | no arguments: the compulsory options and the usage |
| `at/help` | both-error | 4/4 | -h: the usage |
| `at/unknown-option` | both-error | 4/4 | an unknown option |
| `at/invert` | both-error | 4/4 | --invert (declared, never added to the parser) |
| `at/missing-out` | both-error | 4/4 | no --out |
| `at/interp-unknown` | both-error | 4/4 | an unknown --interp |
| `at/lists-mismatch` | both-error | 4/4 | two images and one index |
| `at/datatype-unknown` | both-error | 4/4 | an unknown --datatype |
| `at/datatype-preserve-missing` | both-error | 4/4 | --datatype=preserve of an image that does not exist |
| `at/imain-missing` | both-error | 4/4 | an image that does not exist |
| `at/images-mismatch` | both-error | 4/4 | images of different sizes |
| `at/datain-missing` | both-error | 4/4 | a --datain that does not exist |
| `at/datain-norm` | both-error | 4/4 | a phase-encoding vector not of unit length |
| `at/inindex-zero` | both-error | 4/4 | an index of 0 |
| `at/inindex-beyond` | both-error | 4/4 | an index beyond --datain's rows |
| `at/topup-extension` | both-error | 4/4 | --topup with an extension |
| `at/topup-missing` | both-error | 4/4 | a --topup that does not exist |
| `at/lsr-oblique` | both-error | 4/4 | --method=lsr with an oblique phase encoding |
| `at/lsr-same` | both-error | 4/4 | --method=lsr of two images with the same phase encoding |
| `at/lsr-one` | both-error | 4/4 | --method=lsr of one image |
| `at/outputtype-invalid` | both-error | 4/4 | FSLOUTPUTTYPE=BOGUS |
| `at/method-unknown` | pass | 6/6 | an unknown --method (FSL-135: FSL writes nothing and exits 0) |
| `at/lsr-no-movements` | both-error | 5/5 | --method=lsr with a field image, which has no movements (FSL-135) |
| `at/jac-other-grid` | both-error | 5/5 | --method=jac with a field on another grid (FSL-134: another number of voxels) |
| `at/jac-other-shape` | expected-divergence | 2/2 | --method=jac with a field of as many voxels as the images in another shape (FSL-134) |
| `at/lsr-other-grid` | expected-divergence | 2/2 | --method=lsr with a field on another grid (FSL-134) |
| `difftest/acquisition-transposed` | pass | 18/18 | an acquisition file of four rows and one column per volume (read transposed): the suite's own reading fixed (the drawn … |
| `difftest/configuration-outputs` | pass | 22/22 | outputs named by a configuration file (FSL's test_acqp names --fout and --iout): the suite compares them under their co… |
| `difftest/fsl145-short-axis` | expected-divergence | 15/16 | FSL-145: --splineorder=2 on an axis shorter than two knot spacings (two coefficients; reproduce FSL's, the default the … |
| `difftest/fsl146-mixed-axes` | pass | 24/24 | FSL-146: volumes encoded along y and x (FSL's gradient leaves the volumes not encoded along an axis out of its Jacobian… |
| `difftest/quadratic-count` | pass | 21/21 | topup --splineorder=2: FSL's quadratic B-spline count ceil((2n + k - 4) / (2k)) + 2 (the original had taken the cubic's… |
| `difftest/scg-regulariser-weight` | pass | 28/28 | scaled conjugate gradients at a level whose start has Reg > 0: the gradient at x + sigma d takes the current point's re… |
| `difftest/trace-verbose` | pass | 22/22 | --trace with -v: FSL prints a stage's Entering line inside a cost line; the suite takes the stages out |
