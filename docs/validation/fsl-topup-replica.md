# Parity: topup and applytopup replicas (bit-exact, FSL Licence)

**All cases agree.** 176 cases on the `standard` tier: 106 pass, 67 rejected by both, 3 expected divergences, 0 failures.

- **Validated:** `larmorx-nc fsl topup` and `fsl applytopup` (crate larmorx-nc-topup, translated from FSL 6.0.7.23's source)
- **Reference:** topup from FSL 6.0.7.23 (fsl-topup 2203.5, fsl-basisfield 2203.4, fsl-warpfns 2501.0, fsl-newimage 2601.0, fsl-miscmaths 2412.6, fsl-utils 2412.6, OpenBLAS 0.3.33 on one thread; the conda build at `~/fsl-6.0.7.23`, as built)
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-03 on Linux x86_64, with `python -m larmorx_validation parity fsl-topup-replica --tier standard`
- **Original programs:** 176 runs: 155 from the oracle cache, 21 run (2 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same |
| Standard output and error | identical (the run directory and FSL's program path replaced; the replica's `larmorx-nc:` lines left out) |
| Files written | the same names |
| Every file | bit-identical (images decompressed; `descrip` left out) |
| D13 fix pass | a defect named exactly when the default's outputs differ |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| at | 74 | 47 | 25 | 2 | 0 |
| cfg | 16 | 12 | 4 | 0 | 0 |
| debug | 5 | 5 | 0 | 0 | 0 |
| err | 39 | 3 | 36 | 0 | 0 |
| fmriprep | 2 | 2 | 0 | 0 | 0 |
| opt | 37 | 34 | 2 | 1 | 0 |
| thr | 3 | 3 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `opt/warpres-negative` | FSL-124: refused; FSL is not run: its knot spacing of about 4e9 voxels needs more memory than the machine holds (2026-11-03: killed in a 24 GiB memory box) |
| `at/vb-other-grid` | FSL-134: refused; FSL is not run: FSL reads the field's lines past their end (undefined behaviour) |
| `at/lsr-other-grid` | FSL-134: refused; FSL is not run: FSL reads the field's lines past their end (undefined behaviour) |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `opt/fsl145-one-coefficient` | --splineorder=2 at --subsamp=2 with z shorter than one knot spacing (FSL-145: one coefficient, whose refit stops FSL; the default runs) | topup exits 1; the same from the replica. Error occurred when preparing to run topup Exception thrown with message: Mat::operator(): index out of bounds |
| `opt/z-phase-encoding` | phase encoding along z | topup exits 1; the same from the replica. Error occurred when preparing to run topup Exception thrown with message: Topup::TopupScan::TopupScan: third element … |
| `cfg/not-found` | a configuration that does not exist | topup exits 1; the same from the replica. Cannot find config-file nothere |
| `cfg/not-found-path` | a configuration with a path that does not exist | topup exits 1; the same from the replica. Cannot find config-file ./nothere |
| `cfg/fsldir-unset` | FSLDIR unset and a name to look up there | topup exits 1; the same from the replica. basic_string: construction from null is not valid |
| `cfg/bad-option` | an unknown option in the configuration | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/no-arguments` | no arguments: the usage | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/help` | -h: the usage | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/help-long` | --help with the compulsory options set | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/unknown-option` | an unknown option | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/missing-value` | an option without its value | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/no-imain` | no --imain | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/imain-missing` | an --imain that does not exist | topup exits 1; the same from the replica. Image Exception : #63 :: No image files match: nothere Image Exception : #22 :: Failed to read volume nothere.nii.gz … |
| `err/imain-text` | an --imain that is not an image | topup exits 1; the same from the replica. Image Exception : #63 :: No image files match: acq.txt Image Exception : #22 :: Failed to read volume acq.txt Error :… |
| `err/datain-and-acqp` | both --datain and --acqp | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/no-datain` | neither --datain nor --acqp | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/datain-missing` | a --datain that does not exist | topup exits 1; the same from the replica. Could not open matrix file nothere.txt Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --da… |
| `err/datain-five-columns` | a --datain of five columns | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/datain-text` | a --datain of words | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/datain-norm` | a phase-encoding vector not of unit length | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/datain-rows` | three rows for two volumes | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/acqp-rows` | three rows for two volumes, given with --acqp | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/readout-long` | a read-out time above 0.2 s | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/readout-two-zero` | two read-out times of 0 | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/levels-subsamp` | --subsamp of three levels against two | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/levels-minmet` | --minmet of three levels against two | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/minmet-2` | --minmet=2 | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/subsamp-incompatible` | --subsamp=3 on a 32-voxel axis | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/subsamp-0` | --subsamp=0 | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/subsamp-mixed` | subsamplings 3 and 2 on a 30 x 30 x 18 image (each divides it; the regridded grid of 33 does not divide by 2) | topup exits 1; the same from the replica. Error occurred when preparing to run level 2 of topup Exception thrown with message: Topup::TopupScan::SubSample: inv… |
| `err/numprec` | an unknown --numprec | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/regmod` | an unknown --regmod | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/interp` | an unknown --interp | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/splineorder-4` | --splineorder=4 | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/splineorder-negative` | --splineorder=-1 | topup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) topup Usage: topup --imain=<some 4D image> --datain=<text file> --config=<text file wi… |
| `err/debug-4` | --debug=4 | topup exits 1; the same from the replica. Error occurred when preparing to run topup Exception thrown with message: Topup::TopupCF::SetDebug: Debug level out o… |
| `err/debug-negative` | --debug=-1 | topup exits 1; the same from the replica. Error occurred when preparing to run topup Exception thrown with message: Topup::TopupCF::SetDebug: Debug level out o… |
| `err/out-no-directory` | --out in a directory that does not exist | topup exits 1; the same from the replica. Error occured while writing output from topup Exception thrown with message: Error: cant open file nodir/o_fieldcoef.… |
| `err/iout-no-directory` | --iout in a directory that does not exist | topup exits 1; the same from the replica. Error occured while writing output from topup Exception thrown with message: Error: cant open file nodir/c.nii.gz |
| `err/profile-no-directory` | --profile with --out in a directory that does not exist | topup exits 1; the same from the replica. |
| `err/outputtype-invalid` | FSLOUTPUTTYPE=BOGUS | topup exits 1; the same from the replica. ERROR:: Unrecognised value (BOGUS) of environment variable FSLOUTPUTTYPE Legal values are: NIFTI, NIFTI_PAIR, NIFTI2,… |
| `err/outputtype-unset` | FSLOUTPUTTYPE unset | topup exits 1; the same from the replica. ERROR:: Environment variable FSLOUTPUTTYPE is not set! Please make sure that the appropriate configuration file is so… |
| `at/vb-one-direction` | --method=vb3D of two images with the same phase encoding (FSL's message) | applytopup exits -6; the same from the replica. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Invalid combina… |
| `at/vb-no-movements` | --method=vb4D with a field image, which has no movements (FSL-135) | applytopup exits -6; the same from the replica. terminate called after throwing an instance of 'TOPUP::TopupFileIOException' what(): TopupFileIO::TopupFileRead… |
| `at/no-arguments` | no arguments: the compulsory options and the usage | applytopup exits 1; the same from the replica. *************************************************** The following COMPULSORY options have not been set: -i,--ima… |
| `at/help` | -h: the usage | applytopup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) applytopup (Version 1.0) Copyright(c) 2009, University of Oxford (Jesper Andersso… |
| `at/unknown-option` | an unknown option | applytopup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) applytopup (Version 1.0) Copyright(c) 2009, University of Oxford (Jesper Andersso… |
| `at/invert` | --invert (declared, never added to the parser) | applytopup exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) applytopup (Version 1.0) Copyright(c) 2009, University of Oxford (Jesper Andersso… |
| `at/missing-out` | no --out | applytopup exits 1; the same from the replica. *************************************************** The following COMPULSORY options have not been set: -o,--out… |
| `at/interp-unknown` | an unknown --interp | applytopup exits -6; the same from the replica. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Unknown interpo… |
| `at/lists-mismatch` | two images and one index | applytopup exits -6; the same from the replica. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Mismatched --in… |
| `at/datatype-unknown` | an unknown --datatype | applytopup exits -6; the same from the replica. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Unknown data ty… |
| `at/datatype-preserve-missing` | --datatype=preserve of an image that does not exist | applytopup exits -6; the same from the replica. Image Exception : #63 :: No image files match: nothere terminate called after throwing an instance of 'std::run… |
| `at/imain-missing` | an image that does not exist | applytopup exits -6; the same from the replica. Image Exception : #63 :: No image files match: nothere Image Exception : #22 :: Failed to read volume nothere.n… |
| `at/images-mismatch` | images of different sizes | applytopup exits -6; the same from the replica. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Mismatched inpu… |
| `at/datain-missing` | a --datain that does not exist | applytopup exits -6; the same from the replica. Could not open matrix file nothere.txt terminate called after throwing an instance of 'TOPUP::TopupFileIOExcept… |
| `at/datain-norm` | a phase-encoding vector not of unit length | applytopup exits -6; the same from the replica. terminate called after throwing an instance of 'TOPUP::TopupFileIOException' what(): TopupFileIO::TopupDatafile… |
| `at/inindex-zero` | an index of 0 | applytopup exits -6; the same from the replica. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Invalid entry i… |
| `at/inindex-beyond` | an index beyond --datain's rows | applytopup exits -6; the same from the replica. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Invalid entry i… |
| `at/topup-extension` | --topup with an extension | applytopup exits -6; the same from the replica. terminate called after throwing an instance of 'TOPUP::TopupFileIOException' what(): TopupFileIO::TopupFileRead… |
| `at/topup-missing` | a --topup that does not exist | applytopup exits -6; the same from the replica. terminate called after throwing an instance of 'TOPUP::TopupFileIOException' what(): TopupFileIO::TopupFileRead… |
| `at/lsr-oblique` | --method=lsr with an oblique phase encoding | applytopup exits -6; the same from the replica. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Invalid phase-e… |
| `at/lsr-same` | --method=lsr of two images with the same phase encoding | applytopup exits -6; the same from the replica. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Invalid combina… |
| `at/lsr-one` | --method=lsr of one image | applytopup exits -6; the same from the replica. terminate called after throwing an instance of 'TOPUP::ApplyTopupException' what(): ApplyTopup::Invalid combina… |
| `at/outputtype-invalid` | FSLOUTPUTTYPE=BOGUS | applytopup exits 1; the same from the replica. ERROR:: Unrecognised value (BOGUS) of environment variable FSLOUTPUTTYPE Legal values are: NIFTI, NIFTI_PAIR, NI… |
| `at/lsr-no-movements` | --method=lsr with a field image, which has no movements (FSL-135) | applytopup exits -6; the same from the replica. terminate called after throwing an instance of 'TOPUP::TopupFileIOException' what(): TopupFileIO::TopupFileRead… |
| `at/jac-other-grid` | --method=jac with a field on another grid (FSL-134: another number of voxels) | applytopup exits -6; the same from the replica. Image Exception : #2 :: Attempted to copydata with non-matching sizes terminate called after throwing an instan… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (53796ce0a606) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

The replica runs as a separate program, the FSL replica package's (`larmorx-nc`), compared with FSL in `reproduce` mode (D13); the default `fix` pass is checked against the reproduced run. fMRIPrep's own outputs come from its container's FSL (an older topup) and are reported only. The inputs are fMRIPrep's: the merged images of its working directory, sdcflows' configuration and the encoding file nipype writes.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep/ds001600-sub-1-noreconall` | pass | 7/7 | fMRIPrep's field-map estimation call (sdcflows' b02b0.cnf) |
| `fmriprep/ds005040-sub-002-noreconall` | pass | 7/7 | fMRIPrep's field-map estimation call (sdcflows' b02b0.cnf) |
| `debug/one-level` | pass | 6/6 | one level (subsampling 2, 3 Levenberg-Marquardt iterations) with --debug=2 and -v: every cost-function evaluation's deb… |
| `debug/movements` | pass | 6/6 | one level with the movements estimated (five parameters of the reversed scan: the translation along the phase encoding … |
| `debug/hessian` | pass | 6/6 | --debug=3 on the half-resolution pair: one iteration's every debug file, the Hessian's text included |
| `debug/trace` | pass | 6/6 | --trace: every routine's entry and exit printed, two levels |
| `debug/profile` | pass | 7/7 | --profile: the levels' durations in <out>.topup_profile_topup.cpp_main.txt (the clock's numbers masked) |
| `opt/defaults` | pass | 6/6 | every default (warpres 10, fwhm 8, five LM iterations, movements) |
| `opt/levels` | pass | 6/6 | two levels: subsampling 2 with Levenberg-Marquardt and the movements, then full resolution with scaled conjugate gradie… |
| `opt/numprec-float` | pass | 6/6 | --numprec=float |
| `opt/interp-linear` | pass | 6/6 | --interp=linear |
| `opt/membrane-lm` | pass | 6/6 | --regmod=membrane_energy with Levenberg-Marquardt (the energy's Hessian) |
| `opt/membrane-scg` | pass | 6/6 | --regmod=membrane_energy with scaled conjugate gradients (the energy's gradient) |
| `opt/splineorder-2` | pass | 6/6 | --splineorder=2 |
| `opt/fsl145-short-axis` | pass | 6/6 | --splineorder=2 with z shorter than two knot spacings (FSL-145: 2 coefficients) |
| `opt/fsl145-one-coefficient` | both-error | 6/6 | --splineorder=2 at --subsamp=2 with z shorter than one knot spacing (FSL-145: one coefficient, whose refit stops FSL; t… |
| `opt/scale` | pass | 6/6 | --scale=1: each volume scaled to the mean |
| `opt/ssqlambda-0` | pass | 6/6 | --ssqlambda=0: lambda as given |
| `opt/regrid-0` | pass | 6/6 | --regrid=0: the image's own grid, two levels |
| `opt/scg` | pass | 6/6 | --minmet=1 without movements |
| `opt/scg-movements` | pass | 6/6 | --minmet=1 with the movements: FSL's warning |
| `opt/acqp` | pass | 6/6 | --acqp instead of --datain |
| `opt/datain-transposed` | pass | 6/6 | an acquisition-parameter file of four rows |
| `opt/x-phase-encoding` | pass | 6/6 | phase encoding along x (the translation along x left out), the movements |
| `opt/four-volumes` | pass | 6/6 | four volumes along x and y: six movement parameters for each moving scan |
| `opt/fsl146-four-axes` | pass | 6/6 | four volumes along y and x without the movements (FSL-146: each axis' Jacobian term of the gradient over the volumes en… |
| `opt/oblique-phase-encoding` | pass | 6/6 | a phase-encoding vector along x and y (the Jacobian of both displacements) |
| `opt/readout-times` | pass | 6/6 | different read-out times |
| `opt/zero-readout` | pass | 6/6 | one read-out time of 0 (a synthesised undistorted image) |
| `opt/same-direction` | pass | 6/6 | both volumes with the same phase encoding |
| `opt/z-phase-encoding` | both-error | 5/5 | phase encoding along z |
| `opt/int16` | pass | 6/6 | a 16-bit integer image |
| `opt/one-volume` | pass | 6/6 | a single 3D volume |
| `opt/no-out` | pass | 6/6 | no --out: the base name from --imain |
| `opt/intent` | pass | 6/6 | an image with intent parameters: the field's header |
| `opt/nifti` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI |
| `opt/nifti2` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI2_GZ |
| `opt/nifti-pair` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI_PAIR |
| `opt/iout-extension` | pass | 6/6 | --iout with .nii.gz: the mean as <name>.nii.gz_mean |
| `opt/nthr-0` | pass | 6/6 | --nthr=0: the default thread count (FSL_NUM_THREADS unset: one) |
| `opt/warpres-0` | pass | 6/6 | --warpres=0: a knot every voxel |
| `opt/lambda-negative` | pass | 7/7 | a negative --lambda |
| `opt/fwhm-negative` | pass | 7/7 | a negative --fwhm |
| `opt/warpres-negative` | expected-divergence | 2/2 | a negative --warpres |
| `cfg/b02b0` | pass | 6/6 | FSL's b02b0.cnf by name, one iteration per level |
| `cfg/b02b0_1` | pass | 6/6 | FSL's b02b0_1.cnf by name, one iteration per level |
| `cfg/b02b0_2` | pass | 6/6 | FSL's b02b0_2.cnf by name, one iteration per level |
| `cfg/b02b0_4` | pass | 6/6 | FSL's b02b0_4.cnf by name, one iteration per level |
| `cfg/b02b0_7T` | pass | 6/6 | FSL's b02b0_7T.cnf by name, one iteration per level |
| `cfg/test_acqp` | pass | 6/6 | FSL's test_acqp.cnf by name, one iteration per level |
| `cfg/b02b0-extension` | pass | 6/6 | FSL's b02b0.cnf by name with its extension |
| `cfg/local` | pass | 6/6 | a local configuration file |
| `cfg/local-no-extension` | pass | 6/6 | a local configuration named without .cnf |
| `cfg/command-line-wins` | pass | 6/6 | options on the command line take precedence |
| `cfg/directory` | pass | 6/6 | a directory as the configuration (read as empty) |
| `cfg/not-found` | both-error | 5/5 | a configuration that does not exist |
| `cfg/not-found-path` | both-error | 5/5 | a configuration with a path that does not exist |
| `cfg/fsldir-unset` | both-error | 6/6 | FSLDIR unset and a name to look up there |
| `cfg/bad-option` | both-error | 5/5 | an unknown option in the configuration |
| `cfg/long-line` | pass | 6/6 | a comment line of more than 1023 characters before two settings |
| `err/no-arguments` | both-error | 5/5 | no arguments: the usage |
| `err/help` | both-error | 5/5 | -h: the usage |
| `err/help-long` | both-error | 5/5 | --help with the compulsory options set |
| `err/unknown-option` | both-error | 5/5 | an unknown option |
| `err/missing-value` | both-error | 5/5 | an option without its value |
| `err/no-imain` | both-error | 5/5 | no --imain |
| `err/imain-missing` | both-error | 5/5 | an --imain that does not exist |
| `err/imain-text` | both-error | 5/5 | an --imain that is not an image |
| `err/datain-and-acqp` | both-error | 5/5 | both --datain and --acqp |
| `err/no-datain` | both-error | 5/5 | neither --datain nor --acqp |
| `err/datain-missing` | both-error | 5/5 | a --datain that does not exist |
| `err/datain-five-columns` | both-error | 5/5 | a --datain of five columns |
| `err/datain-text` | both-error | 5/5 | a --datain of words |
| `err/datain-norm` | both-error | 5/5 | a phase-encoding vector not of unit length |
| `err/datain-rows` | both-error | 5/5 | three rows for two volumes |
| `err/acqp-rows` | both-error | 6/6 | three rows for two volumes, given with --acqp |
| `err/readout-long` | both-error | 5/5 | a read-out time above 0.2 s |
| `err/readout-two-zero` | both-error | 5/5 | two read-out times of 0 |
| `err/levels-subsamp` | both-error | 5/5 | --subsamp of three levels against two |
| `err/levels-minmet` | both-error | 5/5 | --minmet of three levels against two |
| `err/minmet-2` | both-error | 5/5 | --minmet=2 |
| `err/subsamp-incompatible` | both-error | 5/5 | --subsamp=3 on a 32-voxel axis |
| `err/subsamp-0` | both-error | 5/5 | --subsamp=0 |
| `err/subsamp-mixed` | both-error | 6/6 | subsamplings 3 and 2 on a 30 x 30 x 18 image (each divides it; the regridded grid of 33 does not divide by 2) |
| `err/numprec` | both-error | 5/5 | an unknown --numprec |
| `err/regmod` | both-error | 5/5 | an unknown --regmod |
| `err/interp` | both-error | 5/5 | an unknown --interp |
| `err/splineorder-4` | both-error | 5/5 | --splineorder=4 |
| `err/splineorder-negative` | both-error | 5/5 | --splineorder=-1 |
| `err/debug-4` | both-error | 5/5 | --debug=4 |
| `err/debug-negative` | both-error | 5/5 | --debug=-1 |
| `err/miter-negative` | pass | 7/7 | --miter=-1 (about 4e9 iterations) |
| `err/out-no-directory` | both-error | 5/5 | --out in a directory that does not exist |
| `err/iout-no-directory` | both-error | 5/5 | --iout in a directory that does not exist |
| `err/logout-no-directory` | pass | 6/6 | --logout in a directory that does not exist |
| `err/rbmout-no-directory` | pass | 6/6 | --rbmout in a directory that does not exist |
| `err/profile-no-directory` | both-error | 5/5 | --profile with --out in a directory that does not exist |
| `err/outputtype-invalid` | both-error | 5/5 | FSLOUTPUTTYPE=BOGUS |
| `err/outputtype-unset` | both-error | 5/5 | FSLOUTPUTTYPE unset |
| `thr/nthr-4` | pass | 7/7 | --nthr=4 (the spline field's threaded products and the Hessians' threaded sparse products) against --nthr=1 |
| `thr/fsl-num-threads` | pass | 7/7 | FSL_NUM_THREADS=4 with --nthr=1 (the default thread count of the Hessian's sparse matrices) against it unset |
| `thr/openblas` | pass | 7/7 | FSL_SKIP_GLOBAL=1 and OPENBLAS_NUM_THREADS=4 (OpenBLAS's own threads: its dot product of more than 10,000 values) again… |
| `at/lsr-y` | pass | 6/6 | least-squares restoration of the pair along y (the default method, spline interpolation, the movements) |
| `at/lsr-4d` | pass | 6/6 | least-squares restoration of two volumes per phase-encoding direction |
| `at/lsr-x` | pass | 6/6 | least-squares restoration along x |
| `at/lsr-four` | pass | 6/6 | least-squares restoration of two collections (along y and along x) |
| `at/lsr-three` | pass | 6/6 | least-squares restoration of three images in one collection |
| `at/lsr-trilinear` | pass | 6/6 | --interp=trilinear |
| `at/lsr-crop` | pass | 6/6 | least-squares restoration of lines of 30 voxels (a length that does not divide 2^32) |
| `at/lsr-zero-field` | pass | 6/6 | least-squares restoration with a field of zeros (band systems: Armadillo's band solver) |
| `at/vb2d-y` | pass | 6/6 | --method=vb2D: phi and lambda per slice |
| `at/vb3d-y` | pass | 6/6 | --method=vb3D: phi and lambda per volume |
| `at/vb4d-y` | pass | 6/6 | --method=vb4D: one phi and lambda |
| `at/lsr-long` | pass | 6/6 | --method=lsr on lines of 96 voxels along y (the long-line pair) |
| `at/vb2d-long` | pass | 6/6 | --method=vb2D on lines of 96 voxels along y (the long-line pair) |
| `at/vb3d-long` | pass | 6/6 | --method=vb3D on lines of 96 voxels along y (the long-line pair) |
| `at/vb4d-long` | pass | 6/6 | --method=vb4D on lines of 96 voxels along y (the long-line pair) |
| `at/vb3d-4d` | pass | 6/6 | --method=vb3D of two volumes (each on its own) |
| `at/vb4d-4d` | pass | 6/6 | --method=vb4D of two volumes (together) |
| `at/vb2d-x` | pass | 6/6 | --method=vb2D along x |
| `at/vb3d-x` | pass | 6/6 | --method=vb3D along x |
| `at/vb3d-aniso` | pass | 6/6 | --method=vb3D with voxels of 6 x 4 mm in plane (the prior's scale factors) |
| `at/vb2d-aniso` | pass | 6/6 | --method=vb2D with voxels of 6 x 4 mm in plane |
| `at/vb4d-four` | pass | 6/6 | --method=vb4D of two collections (along y and along x) |
| `at/vb3d-three` | pass | 6/6 | --method=vb3D of three images in one collection (FSL-142: counted as two) |
| `at/vb2d-three` | pass | 6/6 | --method=vb2D of three images in one collection |
| `at/vb-one-direction` | both-error | 5/5 | --method=vb3D of two images with the same phase encoding (FSL's message) |
| `at/vb-no-movements` | both-error | 6/6 | --method=vb4D with a field image, which has no movements (FSL-135) |
| `at/vb-other-grid` | expected-divergence | 2/2 | --method=vb2D with a field on another grid (FSL-134) |
| `at/jac-y` | pass | 6/6 | Jacobian modulation of one image |
| `at/jac-pair` | pass | 6/6 | Jacobian modulation of the pair, averaged |
| `at/jac-4d` | pass | 6/6 | Jacobian modulation of two volumes |
| `at/jac-trilinear` | pass | 6/6 | --method=jac --interp=trilinear |
| `at/jac-x` | pass | 6/6 | Jacobian modulation along x |
| `at/jac-quadratic` | pass | 6/6 | a field of quadratic splines |
| `at/jac-field` | pass | 6/6 | --topup naming a field image (topup's --fout): no movements |
| `at/jac-field-aniso` | pass | 6/6 | a field image and voxels of 6 x 4 mm in plane (the Jacobian of the y displacements) |
| `at/jac-no-movpar` | pass | 6/6 | coefficients without their _movpar.txt (no movements) |
| `at/jac-oblique` | pass | 6/6 | a phase-encoding vector along x and y |
| `at/jac-z` | pass | 6/6 | phase encoding along z (FSL-144: FSL leaves the z component out silently; the default refuses) |
| `at/datatype-preserve` | pass | 6/6 | --datatype=preserve |
| `at/datatype-char` | pass | 6/6 | --datatype=char |
| `at/datatype-short` | pass | 6/6 | --datatype=short |
| `at/datatype-int` | pass | 6/6 | --datatype=int |
| `at/datatype-float` | pass | 6/6 | --datatype=float |
| `at/datatype-double` | pass | 6/6 | --datatype=double |
| `at/datatype-preserve-int16` | pass | 6/6 | --datatype=preserve of a 16-bit image |
| `at/datatype-preserve-int8` | pass | 6/6 | --datatype=preserve of an 8-bit signed image |
| `at/short-options` | pass | 6/6 | the short options |
| `at/nifti` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI |
| `at/no-arguments` | both-error | 5/5 | no arguments: the compulsory options and the usage |
| `at/help` | both-error | 5/5 | -h: the usage |
| `at/unknown-option` | both-error | 5/5 | an unknown option |
| `at/invert` | both-error | 5/5 | --invert (declared, never added to the parser) |
| `at/missing-out` | both-error | 5/5 | no --out |
| `at/interp-unknown` | both-error | 5/5 | an unknown --interp |
| `at/lists-mismatch` | both-error | 5/5 | two images and one index |
| `at/datatype-unknown` | both-error | 5/5 | an unknown --datatype |
| `at/datatype-preserve-missing` | both-error | 5/5 | --datatype=preserve of an image that does not exist |
| `at/imain-missing` | both-error | 5/5 | an image that does not exist |
| `at/images-mismatch` | both-error | 5/5 | images of different sizes |
| `at/datain-missing` | both-error | 5/5 | a --datain that does not exist |
| `at/datain-norm` | both-error | 5/5 | a phase-encoding vector not of unit length |
| `at/inindex-zero` | both-error | 5/5 | an index of 0 |
| `at/inindex-beyond` | both-error | 5/5 | an index beyond --datain's rows |
| `at/topup-extension` | both-error | 5/5 | --topup with an extension |
| `at/topup-missing` | both-error | 5/5 | a --topup that does not exist |
| `at/lsr-oblique` | both-error | 5/5 | --method=lsr with an oblique phase encoding |
| `at/lsr-same` | both-error | 5/5 | --method=lsr of two images with the same phase encoding |
| `at/lsr-one` | both-error | 5/5 | --method=lsr of one image |
| `at/outputtype-invalid` | both-error | 5/5 | FSLOUTPUTTYPE=BOGUS |
| `at/method-unknown` | pass | 7/7 | an unknown --method (FSL-135: FSL writes nothing and exits 0) |
| `at/lsr-no-movements` | both-error | 6/6 | --method=lsr with a field image, which has no movements (FSL-135) |
| `at/jac-other-grid` | both-error | 6/6 | --method=jac with a field on another grid (FSL-134: another number of voxels) |
| `at/jac-other-shape` | pass | 7/7 | --method=jac with a field of as many voxels as the images in another shape (FSL-134) |
| `at/lsr-other-grid` | expected-divergence | 2/2 | --method=lsr with a field on another grid (FSL-134) |
