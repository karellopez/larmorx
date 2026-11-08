# Parity: 3dvolreg: the clean-room original against AFNI 26.3.05 and the replica

**All cases agree.** 251 cases on the `standard` tier: 185 pass, 63 rejected by both, 3 expected divergences, 0 failures.

- **Validated:** afni-volreg-original
- **Reference:** AFNI 26.3.05 3dvolreg (oracles/afni-26.3.05, through the oracle cache); the replica's default as a black box
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-08 on Linux x86_64, with `python -m larmorx_validation parity afni-volreg-original --tier standard`
- **Original programs:** 247 runs: 243 from the oracle cache, 4 run; cache `<workspace>/oracles/cache`, mode `use`

- **Cases by category** (passed or agreeing / all): base 7/7, datatypes 12/12, defects 30/30, edge 25/25, environment 9/9, errors 34/34, fmriprep 10/10, global options 8/8, interpolation 22/22, messages 13/13, outputs 15/15, rotparent 24/24, settings 6/6, threads 3/3, tshift 9/9, twodup 4/4, twopass 12/12, weights 8/8
- **-1Dfile lines identical with AFNI's** (bit-identity is reported, not required, §14.3): 417 of 1096
- **Largest share of a threshold used** (1.0 = at the threshold): -1Dfile parameters 0.611 (`iso-ras-fourier`); -1Dmatrix_save rotations 0.947 (`four-slices`); -1Dmatrix_save translations 0.689 (`params`); -maxdisp1D deltas 0.626 (`negative`); -maxdisp1D values 0.659 (`negative`); -savedisp DX 0.671 (`afni99-savedisp`); -savedisp DY 0.723 (`afni99-savedisp`); -savedisp DZ 0.163 (`afni99-savedisp`); normalised DX, DY, DZ where well conditioned 0.052 (`SAVEDISP`); raw displacements (-savedisp) 0.538 (`SAVEDISP`); registered volumes 0.681 (`fmriprep-ds000256-nofeedback`)
- **fMRIPrep's medians**: the original's at most 0.115 of its limit (`fmriprep-ds000256-nofeedback`; 1.0 = the limit its volumes imply, §14.3)
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - AFNI-01: 1 case(s) named it, 1 with changed values (largest difference 6e+03), 1 with other changes (d.ND's text; m.affND's text; md.ND's text; md_delt.ND's text; p.ND's text; the messages): `afni01-tshift`
  - AFNI-11: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit status N -> N; the files written; the messages): `existing-prefix`
  - AFNI-12: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the files written; the messages): `prefix-illegal`
  - AFNI-26: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the messages): `afni90-prefix`
  - AFNI-86: 2 case(s) named it, 2 with changed values (largest difference 34.4), 2 with other changes (d.ND's text; m.affND's text): `afni86`, `afni86-alone`
  - AFNI-87: 3 case(s) named it, 3 with changed values (largest difference 10): `uint8`, `uint8-fourier`, `verbose-byte`
  - AFNI-88: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the messages): `afni88-base`
  - AFNI-89: 4 case(s) named it, 0 with changed values (largest difference 0), 4 with other changes (the messages): `afni89-const`, `afni89-half`, `afni89-weight`, `afni89-weight-twopass`
  - AFNI-90: 4 case(s) named it, 0 with changed values (largest difference 0), 4 with other changes (exit status N -> N; the files written; the messages): `afni90-prefix`, `afni90-dfile`, `afni90-1dfile`, `afni90-weight`
  - AFNI-92: 1 case(s) named it, 0 with changed values (largest difference 0): `afni92`
  - AFNI-93: 2 case(s) named it, 1 with changed values (largest difference 16.4), 1 with other changes (d.ND's text; m.affND's text; md.ND's text; md_delt.ND's text; p.ND's text): `afni93`, `afni93-one-slice`
  - AFNI-94: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the messages): `afni94`
  - AFNI-95: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit status N -> N; the files written; the messages): `afni95`
  - AFNI-96: 3 case(s) named it, 0 with changed values (largest difference 0), 3 with other changes (the messages): `afni96-centre`, `afni96-base`, `afni96-short`
  - AFNI-98: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (the messages): `afni98-base`, `afni98-weight`
  - AFNI-99: 17 case(s) named it, 1 with changed values (largest difference 10.7), 17 with other changes (dsN-baseline_volreg_md.ND's text; dsN-baseline_volreg_md_delt.ND's text; dsN-echo-N_volreg_md.ND's text; dsN-echo-N_volreg_md_delt.ND's text; dsN-mbN_volreg_md.ND's text; dsN-mbN_volreg_md_delt.ND's text; dsN-nofeedback_volreg_md.ND's text; dsN-nofeedback_volreg_md_delt.ND's text; dsN-run-N_volreg_md.ND's text; dsN-run-N_volreg_md_delt.ND's text; dsN_volreg_md.ND's text; dsN_volreg_md_delt.ND's text; md.ND's text; md_delt.ND's text; pepolar-dsN_volreg_md.ND's text; pepolar-dsN_volreg_md_delt.ND's text; the messages): `int16-ras`, `iso-ras-fourier`, `int16-float`, `int16-fmriprep`, `no-transform`, `afni99-ras`, `afni99-savedisp`, `fmriprep-ds000005-run-01` …
  - GLIBC-07 (not named): 33 case(s) named it, 0 with changed values (largest difference 0), 33 with other changes (the messages): `base-4d`, `emptyish`, `emptyish-twopass`, `oblique`, `oblique-fmriprep`, `afni88-prefix`, `afni88-input`, `afni88-matrix` …

## Thresholds

| Quantity | Requirement |
|---|---|
| exit status, files written, headers, types, geometry | identical |
| messages | byte for byte; the CPU times masked; the numbers that follow from the parameters within their derived bounds (+0.01); -verbose -verbose's derivative ranges within 10 %, its fit numbers masked (§14.3) |
| parameters (-1Dfile, -dfile, -rotcom) | per volume \|d angle\| <= phi_t + k\|angle\|, \|d shift\| <= x_t delta + k\|shift\|; median <= m; k = 0.005, m = 0.001 (k = 0.03, m = 0.005 for -linear) |
| -dfile | rmsold identical; rmsnew within the registered volumes' rms difference + 1e-3 rmsnew (+1 for int16) |
| -1Dmatrix_save, -maxdisp1D, -savedisp | bounds derived from each volume's own parameter differences (§14.3, Derived quantities) |
| -SAVEDISP | the normalisation and products bit for bit on AFNI's raw displacements; the normalised values where well conditioned |
| registered volumes | each volume v: mean \|diff\| <= L_v max and NCC >= 1 - 50 L_v^2, L_v = max(2e-3, 1e-3 theta_v), theta_v = \|roll\| + \|pitch\| + \|yaw\| of the reference (degrees); the largest difference reported |
| registered volumes, -final NN | every differing voxel's value from an input voxel within two voxels of the reference's source voxel (the input 0 outside its grid); checked where the input's values are unique |
| fMRIPrep's medians | mean \|diff\| <= L_med max + q and NCC >= 1 - 50 L_med^2 against fMRIPrep's own output (L_med the run's largest L_v, q the stored median's step); the largest difference reported |
| the default against the replica's default | the same thresholds; the defects named the same |
| threads | every byte the same at 1, 2, 5 and 14 threads |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| base | 7 | 7 | 0 | 0 | 0 |
| datatypes | 12 | 12 | 0 | 0 | 0 |
| defects | 30 | 10 | 19 | 1 | 0 |
| edge | 25 | 24 | 1 | 0 | 0 |
| environment | 9 | 6 | 3 | 0 | 0 |
| errors | 34 | 3 | 29 | 2 | 0 |
| fmriprep | 10 | 10 | 0 | 0 | 0 |
| global options | 8 | 7 | 1 | 0 | 0 |
| interpolation | 22 | 22 | 0 | 0 | 0 |
| messages | 13 | 13 | 0 | 0 | 0 |
| outputs | 15 | 15 | 0 | 0 | 0 |
| rotparent | 24 | 14 | 10 | 0 | 0 |
| settings | 6 | 6 | 0 | 0 | 0 |
| threads | 3 | 3 | 0 | 0 | 0 |
| tshift | 9 | 9 | 0 | 0 | 0 |
| twodup | 4 | 4 | 0 | 0 | 0 |
| twopass | 12 | 12 | 0 | 0 | 0 |
| weights | 8 | 8 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `afni96-short` | AFNI-96 (rule B, an error in both modes, spec §12): AFNI reads the third number past the attribute's two and goes on with whatever it finds there; larmorx stops |
| `prefix-afni-format` | larmorx writes no .HEAD/.BRIK dataset: it stops with an error where AFNI writes its own format (all the larmorx AFNI tools' rule, spec §9.4) |
| `prefix-default` | larmorx writes no .HEAD/.BRIK dataset: the default prefix 'volreg' is refused (spec §9.4) |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `error-unknown` | an unknown option | both refuse the input with the same messages |
| `one-slice` | one slice | both refuse the input with the same messages |
| `afni88-prefix` | -prefix without a value (AFNI-88) | both refuse the input with the same messages |
| `afni88-input` | -input without a value (AFNI-88) | both refuse the input with the same messages |
| `afni88-rotparent` | -rotparent without a value (AFNI-88) | both refuse the input with the same messages |
| `afni88-matrix` | -1Dmatrix_save without a value (AFNI-88) | both refuse the input with the same messages |
| `afni88-base` | -base without a value (AFNI-88) | AFNI crashes and the original stops with AFNI's messages and an error |
| `afni89-const` | a constant 40 x 44 x 24 series with -twopass (AFNI-89) | AFNI crashes and the original stops with AFNI's messages and an error |
| `afni89-half` | a half-constant series with -twopass (AFNI-89) | AFNI crashes and the original stops with AFNI's messages and an error |
| `afni89-weight` | an all-zero -weight (AFNI-89) | AFNI crashes and the original stops with AFNI's messages and an error |
| `afni89-weight-twopass` | an all-zero -weight with -twopass (AFNI-89) | AFNI crashes and the original stops with AFNI's messages and an error |
| `afni90-prefix` | a -prefix of 256 bytes (AFNI-90) | AFNI crashes and the original stops with AFNI's messages and an error |
| `afni90-dfile` | a -dfile of 300 bytes (AFNI-90) | AFNI crashes and the original stops with AFNI's messages and an error |
| `afni90-1dfile` | a -1Dfile of 256 bytes (AFNI-90) | AFNI crashes and the original stops with AFNI's messages and an error |
| `afni90-weight` | a -weight name of 300 bytes (AFNI-90) | AFNI crashes and the original stops with AFNI's messages and an error |
| `afni93-one-slice` | one slice with -twopass (AFNI-93) | both refuse the input with the same messages |
| `afni94` | -coarse 1 2 with -twopass on 120^3 voxels (AFNI-94; against the replica: AFNI never ends) | AFNI never ends (AFNI-94) |
| `afni96-centre` | a -rotparent without VOLREG_CENTER_OLD (AFNI-96) | AFNI crashes and the original stops with AFNI's messages and an error |
| `afni96-base` | a -rotparent without VOLREG_CENTER_BASE (AFNI-96) | AFNI crashes and the original stops with AFNI's messages and an error |
| `afni98-base` | -input with a base index too large: (null) (AFNI-98) | both refuse the input with the same messages |
| `afni98-weight` | -input with a -weight of another size (AFNI-98) | both refuse the input with the same messages |
| `too-few` | no dataset | both refuse the input with the same messages |
| `too-many` | two datasets | both refuse the input with the same messages |
| `final-bad` | -final spline | both refuse the input with the same messages |
| `twoblur-small` | -twoblur 1.5 | both refuse the input with the same messages |
| `zpad-negative` | -zpad -1 | both refuse the input with the same messages |
| `edging-negative` | -edging -1 | both refuse the input with the same messages |
| `edging-percent` | -edging 30% | both refuse the input with the same messages |
| `tshift-negative` | -tshift -1 | both refuse the input with the same messages |
| `base-negative` | -base -1 | both refuse the input with the same messages |
| `base-too-large` | -base 9 | both refuse the input with the same messages |
| `base-missing` | -base of a missing file | both refuse the input with the same messages |
| `base-twice` | -base 1 -base 2 | both refuse the input with the same messages |
| `input-missing` | -input of a missing file | both refuse the input with the same messages |
| `dataset-missing` | a missing dataset | both refuse the input with the same messages |
| `weight-missing` | -weight of a missing file | both refuse the input with the same messages |
| `weight-no-bracket` | -weight without [n] | both refuse the input with the same messages |
| `weight-no-index` | -weight with [x] | both refuse the input with the same messages |
| `weight-bad-index` | -weight with [3] | both refuse the input with the same messages |
| `weight-twice` | two -weight | both refuse the input with the same messages |
| `weight-dims` | a -weight of another size | both refuse the input with the same messages |
| `matrix-twice` | two -1Dmatrix_save | both refuse the input with the same messages |
| `matrix-badname` | -1Dmatrix_save of a refused name | both refuse the input with the same messages |
| `input-twice` | two -input | both refuse the input with the same messages |
| `one-brick-itself` | one volume and an internal base | both refuse the input with the same messages |
| `base-dims` | an external base of another size | both refuse the input with the same messages |
| `base-spacing` | an external base of other steps | both refuse the input with the same messages |
| `base-orient` | an external base of another orientation | both refuse the input with the same messages |
| `base-sparse` | a base of fewer than 100 non-zero voxels | both refuse the input with the same messages |
| `rp-gp27m` | a -gridparent with S2I slices | both refuse the input with the same messages |
| `rp-gp22` | a -gridparent with fewer slices (error) | both refuse the input with the same messages |
| `rp-gpsp` | a -gridparent of other steps (error) | both refuse the input with the same messages |
| `rp-gpx` | a -gridparent of another nx (error) | both refuse the input with the same messages |
| `rp-noattr` | a -rotparent without VOLREG attributes | both refuse the input with the same messages |
| `rp-missing` | a missing -rotparent | both refuse the input with the same messages |
| `rp-twice` | two -rotparent | both refuse the input with the same messages |
| `gp-missing` | a missing -gridparent | both refuse the input with the same messages |
| `gp-twice` | two -gridparent | both refuse the input with the same messages |
| `rp-skew` | a -rotparent matrix not orthogonal | both refuse the input with the same messages |
| `env-edging-percent` | AFNI_VOLREG_EDGING=40% (error) | both refuse the input with the same messages |
| `env-edging-bad` | AFNI_VOLREG_EDGING=-2 (error) | both refuse the input with the same messages |
| `env-deconflict-no` | AFNI_DECONFLICT=NO with an existing output | both refuse the input with the same messages |
| `stdin-empty` | -@ with nothing on standard input (against the replica only) | the original and the replica refuse the input alike |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (88ecca255b5d) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

The clean-room original is held to `specs/3dvolreg.md` §14.3's thresholds, not to AFNI's bits: the shear decomposition's exact ties are decided by the evaluation's last bits, the spec does not give AFNI's (§5.3 'Numerics', §16). Bit-identity is reported per check.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `default` | pass | 46/46 | the default heptic method on the moving series s1 |
| `fmriprep-s1` | pass | 46/46 | fMRIPrep's options on s1 (motions of 3 degrees and 5 mm) |
| `int16-ras` | pass | 48/48 | int16 stored L2R, P2A, I2S (AFNI-99 in -maxdisp) |
| `savedisp` | pass | 60/60 | -savedisp's three datasets |
| `verbose-twopass` | pass | 46/46 | -verbose with -twopass: the coarse searches' lines |
| `error-unknown` | both-error | 10/10 | an unknown option |
| `method-linear` | pass | 46/46 | estimation and output by linear |
| `method-cubic` | pass | 46/46 | estimation and output by cubic |
| `method-quintic` | pass | 46/46 | estimation and output by quintic |
| `method-heptic` | pass | 46/46 | estimation and output by heptic |
| `method-Fourier` | pass | 46/46 | estimation and output by Fourier |
| `final-NN` | pass | 44/44 | heptic estimation, -final NN |
| `final-linear` | pass | 46/46 | heptic estimation, -final linear |
| `final-cubic` | pass | 46/46 | heptic estimation, -final cubic |
| `final-quintic` | pass | 46/46 | heptic estimation, -final quintic |
| `final-Fourier` | pass | 46/46 | heptic estimation, -final Fourier |
| `final-heptic-linear` | pass | 46/46 | -linear estimation, -final heptic |
| `noclip-cubic` | pass | 46/46 | -noclip with cubic |
| `noclip-fourier` | pass | 46/46 | -noclip with Fourier |
| `clipit` | pass | 46/46 | -clipit (a notice: the default) |
| `zpad-0` | pass | 46/46 | -zpad 0 |
| `zpad-2` | pass | 46/46 | -zpad 2 |
| `zpad-9` | pass | 46/46 | -zpad 9 |
| `fourier-zpad0-noclip` | pass | 46/46 | Fourier without padding or clipping |
| `spellings` | pass | 46/46 | -Linear, -Quintic: the methods' other spellings |
| `linear-noclip-final-nn` | pass | 44/44 | -linear -noclip -final NN |
| `iso-ras-fourier` | pass | 46/46 | isotropic 2 mm voxels stored RAS, Fourier |
| `base-2` | pass | 46/46 | -base 2 |
| `base-last` | pass | 46/46 | -base 5 (the last volume) |
| `base-dset` | pass | 46/46 | an external base (volume 0 of a 3D dataset) |
| `base-dset-verbose` | pass | 46/46 | an external base elsewhere: the centres' distance |
| `base-4d` | pass | 46/46 | a 4D -base dataset: its warning, volume 0 |
| `base-dset-twopass` | pass | 46/46 | an external base with -twopass |
| `base-abbrev` | pass | 46/46 | -bas 1 (four letters) |
| `scaled` | pass | 46/46 | int16 with scale factors: stored as float32 |
| `scaled-float` | pass | 46/46 | int16 with scale factors, -float |
| `int16-float` | pass | 46/46 | int16 with -float |
| `uint8` | pass | 48/48 | uint8 (AFNI-87) |
| `uint8-fourier` | pass | 48/48 | uint8 with fMRIPrep's options (AFNI-87) |
| `float64` | pass | 46/46 | float64 stored (converted to float32) |
| `uint16` | pass | 46/46 | uint16 stored |
| `bigendian` | pass | 46/46 | a big-endian float32 file |
| `nifti2` | pass | 46/46 | a NIfTI-2 file |
| `plain-nii` | pass | 28/28 | an uncompressed .nii, output .nii.gz |
| `int16-fmriprep` | pass | 48/48 | int16 RAS with fMRIPrep's options |
| `one-volume-base` | pass | 46/46 | one volume with an external base |
| `still` | pass | 46/46 | no motion at all |
| `big-motion` | pass | 46/46 | rotations of 6 to 12 degrees and 8 mm without -twopass |
| `big-motion-twopass` | pass | 46/46 | the same with -twopass: the coarse searches find them |
| `big-motion-fmriprep` | pass | 46/46 | the same with fMRIPrep's options |
| `two-volumes` | pass | 46/46 | two volumes |
| `emptyish` | pass | 46/46 | a volume with fewer than 66 non-zero voxels: not registered |
| `emptyish-twopass` | pass | 46/46 | the same with -twopass |
| `const` | pass | 46/46 | a constant series without -twopass: registers |
| `const29-twopass` | pass | 46/46 | a constant 29^3 series with -twopass: AFNI-89 not reached |
| `one-slice` | both-error | 10/10 | one slice |
| `four-slices` | pass | 46/46 | four slices |
| `odd-sizes` | pass | 46/46 | 37 x 41 x 19 voxels of 3.2 x 2.9 x 4 mm |
| `odd-fourier` | pass | 46/46 | the odd grid with Fourier (odd FFT lengths) and -twopass |
| `oblique` | pass | 46/46 | an oblique grid (the obliquity report) |
| `oblique-fmriprep` | pass | 46/46 | the oblique grid with fMRIPrep's options |
| `sagittal` | pass | 46/46 | sagittal slices (axes y, z, x) |
| `sagittal-twopass` | pass | 46/46 | sagittal slices with -twopass |
| `nan` | pass | 46/46 | NaN voxels |
| `negative` | pass | 46/46 | negative values (clipping to the input's range) |
| `qform-only` | pass | 46/46 | a qform only |
| `sform-only` | pass | 46/46 | an sform only |
| `no-transform` | pass | 46/46 | neither code: the NO spatial transform warning |
| `code2-verbose` | pass | 46/46 | codes 2 (./ in AFNI's names) with -verbose |
| `msec` | pass | 46/46 | a TR in milliseconds |
| `afni86` | pass | 46/46 | -nocoarserot read as -noclip (AFNI-86) |
| `afni86-alone` | pass | 46/46 | -nocoarserot without -twopass (AFNI-86) |
| `afni88-prefix` | both-error | 10/10 | -prefix without a value (AFNI-88) |
| `afni88-input` | both-error | 10/10 | -input without a value (AFNI-88) |
| `afni88-rotparent` | both-error | 10/10 | -rotparent without a value (AFNI-88) |
| `afni88-matrix` | both-error | 10/10 | -1Dmatrix_save without a value (AFNI-88) |
| `afni88-base` | both-error | 11/11 | -base without a value (AFNI-88) |
| `afni89-const` | both-error | 11/11 | a constant 40 x 44 x 24 series with -twopass (AFNI-89) |
| `afni89-half` | both-error | 11/11 | a half-constant series with -twopass (AFNI-89) |
| `afni89-weight` | both-error | 11/11 | an all-zero -weight (AFNI-89) |
| `afni89-weight-twopass` | both-error | 11/11 | an all-zero -weight with -twopass (AFNI-89) |
| `afni90-prefix` | both-error | 11/11 | a -prefix of 256 bytes (AFNI-90) |
| `afni90-dfile` | both-error | 17/17 | a -dfile of 300 bytes (AFNI-90) |
| `afni90-1dfile` | both-error | 17/17 | a -1Dfile of 256 bytes (AFNI-90) |
| `afni90-weight` | both-error | 11/11 | a -weight name of 300 bytes (AFNI-90) |
| `afni90-maxdisp` | pass | 22/22 | a -maxdisp1D name of 254 bytes (written) |
| `afni92` | pass | 38/38 | -nomaxdisp with -twopass: the coarse automask's clip fraction (AFNI-92) |
| `afni93` | pass | 46/46 | four slices with -twopass: an empty search region (AFNI-93) |
| `afni93-one-slice` | both-error | 10/10 | one slice with -twopass (AFNI-93) |
| `afni94` | both-error | 10/10 | -coarse 1 2 with -twopass on 120^3 voxels (AFNI-94; against the replica: AFNI never ends) |
| `afni95` | pass | 28/28 | -base 0 then another -base (AFNI-95) |
| `afni96-centre` | both-error | 11/11 | a -rotparent without VOLREG_CENTER_OLD (AFNI-96) |
| `afni96-base` | both-error | 11/11 | a -rotparent without VOLREG_CENTER_BASE (AFNI-96) |
| `afni96-short` | expected-divergence | 10/10 | a -rotparent centre of two numbers (AFNI-96) |
| `afni98-base` | both-error | 10/10 | -input with a base index too large: (null) (AFNI-98) |
| `afni98-weight` | both-error | 10/10 | -input with a -weight of another size (AFNI-98) |
| `afni99-lps` | pass | 46/46 | the same data stored R2L, A2P, I2S: the reference of AFNI-99 |
| `afni99-ras` | pass | 48/48 | stored L2R, P2A, I2S with -verbose (AFNI-99) |
| `afni99-savedisp` | pass | 62/62 | -savedisp on a RAS dataset (AFNI-99) |
| `afni01-tshift` | pass | 46/46 | -tshift on int16 with a negative scale factor (AFNI-01) |
| `too-few` | both-error | 10/10 | no dataset |
| `too-many` | both-error | 10/10 | two datasets |
| `final-bad` | both-error | 10/10 | -final spline |
| `twoblur-small` | both-error | 10/10 | -twoblur 1.5 |
| `zpad-negative` | both-error | 10/10 | -zpad -1 |
| `edging-negative` | both-error | 10/10 | -edging -1 |
| `edging-percent` | both-error | 10/10 | -edging 30% |
| `tshift-negative` | both-error | 10/10 | -tshift -1 |
| `base-negative` | both-error | 10/10 | -base -1 |
| `base-too-large` | both-error | 10/10 | -base 9 |
| `base-missing` | both-error | 10/10 | -base of a missing file |
| `base-twice` | both-error | 10/10 | -base 1 -base 2 |
| `input-missing` | both-error | 10/10 | -input of a missing file |
| `dataset-missing` | both-error | 10/10 | a missing dataset |
| `weight-missing` | both-error | 10/10 | -weight of a missing file |
| `weight-no-bracket` | both-error | 10/10 | -weight without [n] |
| `weight-no-index` | both-error | 10/10 | -weight with [x] |
| `weight-bad-index` | both-error | 10/10 | -weight with [3] |
| `weight-twice` | both-error | 10/10 | two -weight |
| `weight-dims` | both-error | 10/10 | a -weight of another size |
| `matrix-twice` | both-error | 10/10 | two -1Dmatrix_save |
| `matrix-badname` | both-error | 10/10 | -1Dmatrix_save of a refused name |
| `input-twice` | both-error | 10/10 | two -input |
| `input-extra` | pass | 28/28 | -input with an argument left (a warning) |
| `one-brick-itself` | both-error | 10/10 | one volume and an internal base |
| `base-dims` | both-error | 10/10 | an external base of another size |
| `base-spacing` | both-error | 10/10 | an external base of other steps |
| `base-orient` | both-error | 10/10 | an external base of another orientation |
| `base-sparse` | both-error | 10/10 | a base of fewer than 100 non-zero voxels |
| `savedisp-badname` | pass | 60/60 | -savedisp x.nii (too short) |
| `prefix-afni-format` | expected-divergence | 10/10 | a -prefix in AFNI's own format (larmorx refuses it) |
| `prefix-default` | expected-divergence | 10/10 | no -prefix (AFNI's default 'volreg', its own format) |
| `prefix-illegal` | pass | 22/22 | a -prefix NIfTI name AFNI refuses when writing |
| `rotcom` | pass | 46/46 | -rotcom: 3drotate's commands |
| `rotcom-noclip-zpad0` | pass | 46/46 | -rotcom with -noclip -zpad 0 -final Fourier |
| `prefix-null` | pass | 26/26 | -prefix NULL: parameters only |
| `maxdisp-stdout` | pass | 28/28 | -maxdisp1D - (standard output, and -_delt) |
| `maxdisp-noext` | pass | 36/36 | -maxdisp1D md (md_delt) |
| `matrix-noext` | pass | 34/34 | -1Dmatrix_save ff (ff.aff12.1D) |
| `overwrite-text` | pass | 22/22 | existing text outputs: their warnings |
| `nomaxdisp` | pass | 38/38 | -nomaxdisp |
| `SAVEDISP` | pass | 93/93 | -SAVEDISP's nineteen datasets |
| `SAVEDISP-still` | pass | 93/93 | -SAVEDISP without motion (zeros left alone) |
| `savedisp-null` | pass | 30/30 | -savedisp with -prefix NULL |
| `savedisp-oblique` | pass | 60/60 | -savedisp on the oblique grid |
| `existing-prefix` | pass | 22/22 | an existing -prefix (AFNI-11) |
| `verbose-outputs` | pass | 60/60 | -verbose with every output (the order of writing) |
| `verbose` | pass | 46/46 | -verbose |
| `verb-abbrev` | pass | 46/46 | -verb (five letters) |
| `vv-heptic` | pass | 50/50 | -verbose -verbose: the basis and the fits |
| `vv-linear` | pass | 50/50 | -verbose -verbose -linear |
| `vv-fourier` | pass | 50/50 | -verbose -verbose -Fourier |
| `vv-twopass` | pass | 50/50 | -verbose -verbose -twopass |
| `vv-wtinp` | pass | 50/50 | -verbose -verbose -wtinp |
| `vv-weight` | pass | 50/50 | -verbose -verbose -weight |
| `verbose-byte` | pass | 48/48 | -verbose on a byte input |
| `verbose-scaled` | pass | 46/46 | -verbose on a scaled input |
| `notices` | pass | 46/46 | -clipit -coarserot -wtrim: their notices |
| `zpad-twice` | pass | 46/46 | -zpad 2 -zpad 3: the second -zpad's warning |
| `rp` | pass | 46/46 | -rotparent (AFNI's own output) |
| `rp-verbose` | pass | 46/46 | -rotparent -verbose |
| `rp-fourier` | pass | 46/46 | -rotparent -Fourier |
| `rp-linear-noclip` | pass | 46/46 | -rotparent -linear -noclip |
| `rp-twopass` | pass | 46/46 | -rotparent -twopass (its warning) |
| `rp-gp27` | pass | 46/46 | a -gridparent of 27 slices: padding |
| `rp-gp29` | pass | 46/46 | a -gridparent of 29 slices: padding 2 and 3 |
| `rp-gp24` | pass | 46/46 | a -gridparent of as many slices: its origin |
| `rp-gp27m` | both-error | 10/10 | a -gridparent with S2I slices |
| `rp-gp22` | both-error | 10/10 | a -gridparent with fewer slices (error) |
| `rp-gpsp` | both-error | 10/10 | a -gridparent of other steps (error) |
| `rp-gpx` | both-error | 10/10 | a -gridparent of another nx (error) |
| `gp-alone` | pass | 46/46 | -gridparent without -rotparent (a warning) |
| `rp-noattr` | both-error | 10/10 | a -rotparent without VOLREG attributes |
| `rp-missing` | both-error | 10/10 | a missing -rotparent |
| `rp-twice` | both-error | 10/10 | two -rotparent |
| `gp-missing` | both-error | 10/10 | a missing -gridparent |
| `gp-twice` | both-error | 10/10 | two -gridparent |
| `rp-skew` | both-error | 10/10 | a -rotparent matrix not orthogonal |
| `rp-st` | pass | 46/46 | -rotparent on a series with slice offsets (moved) |
| `rp-st-gp27` | pass | 46/46 | slice offsets padded by a -gridparent |
| `rp-oblique` | pass | 46/46 | an oblique series and -rotparent |
| `rp-twodup` | pass | 46/46 | -gridparent overrides -twodup |
| `rp-savedisp` | pass | 60/60 | -rotparent with -savedisp |
| `tshift` | pass | 46/46 | -tshift 0 (alt+z offsets in the header) |
| `tshift-verbose-ignore` | pass | 46/46 | -tshift 1 -verbose |
| `tshift-seq` | pass | 46/46 | -tshift 0 with seq+z offsets |
| `tshift-none` | pass | 46/46 | -tshift on a series without offsets (a warning) |
| `tshift-large-ignore` | pass | 46/46 | -tshift 3 on 6 volumes: too large (a warning) |
| `tshift-base-prior` | pass | 46/46 | -tshift 1 with -base 0 (a warning) |
| `tshift-one-brick` | pass | 46/46 | -tshift on one volume (a warning) |
| `tshift-fails` | pass | 46/46 | -tshift with offsets beyond the TR: it fails (a warning) |
| `tshift-fmriprep` | pass | 46/46 | -tshift 0 with fMRIPrep's options |
| `twodup` | pass | 46/46 | -twodup with -twopass and a moved base |
| `twodup-onepass` | pass | 46/46 | -twodup without -twopass (ignored) |
| `twodup-internal` | pass | 46/46 | -twodup with an internal base (ignored) |
| `twodup-st` | pass | 46/46 | -twodup on a series with slice offsets |
| `twopass` | pass | 46/46 | -twopass |
| `twoblur-3` | pass | 46/46 | -twopass -twoblur 3 |
| `twoblur-fft` | pass | 46/46 | -twopass -twoblur 16: the FFT blur |
| `coarse-shift-only` | pass | 46/46 | -coarse 2 1 |
| `coarse-4-3` | pass | 46/46 | -coarse 4 3 |
| `coarserot` | pass | 46/46 | -twopass -coarserot (a notice) |
| `sinit` | pass | 46/46 | -twopass -sinit (no scale initialisation) |
| `maxite-raised` | pass | 46/46 | -twopass -maxite 5 (raised to 66) |
| `twopass-linear` | pass | 46/46 | -twopass -linear |
| `twopass-final-nn` | pass | 44/44 | -twopass -Fourier -final NN |
| `twopass-delta` | pass | 46/46 | -twopass -delta 0.3 |
| `weight` | pass | 46/46 | -weight vol[0] |
| `weight-4d` | pass | 46/46 | -weight 4D[1] |
| `wtinp` | pass | 46/46 | -wtinp |
| `weight-wtinp` | pass | 46/46 | -weight with -wtinp (overridden) |
| `edging-3` | pass | 46/46 | -edging 3 |
| `edging-10pc` | pass | 46/46 | -edging 10% |
| `edging-0` | pass | 46/46 | -edging 0 |
| `weight-twopass` | pass | 46/46 | -weight with -twopass |
| `params` | pass | 46/46 | -params 5 0.02 0.05 0.9 |
| `maxite-3` | pass | 46/46 | -maxite 3 |
| `maxd` | pass | 46/46 | -maxd 3 (read as -maxite) |
| `x-thresh` | pass | 46/46 | -x_thresh 0.05 |
| `rot-thresh` | pass | 46/46 | -rot_thresh 0.1 |
| `delta-0.5` | pass | 46/46 | -delta 0.5 |
| `env-edging` | pass | 46/46 | AFNI_VOLREG_EDGING=10 |
| `env-edging-percent` | both-error | 10/10 | AFNI_VOLREG_EDGING=40% (error) |
| `env-edging-bad` | both-error | 10/10 | AFNI_VOLREG_EDGING=-2 (error) |
| `env-quiet` | pass | 46/46 | AFNI_QUIET_STARTUP=YES |
| `env-type-warn` | pass | 46/46 | AFNI_NIFTI_TYPE_WARN=YES on uint16 and a float64 base |
| `type-notice-once` | pass | 46/46 | the type conversion notice once (uint16 input, float64 base) |
| `env-deconflict-no` | both-error | 10/10 | AFNI_DECONFLICT=NO with an existing output |
| `env-firfac` | pass | 46/46 | AFNI_BLUR_FIRFAC=3 with -twopass |
| `env-norealpath` | pass | 46/46 | AFNI_NOREALPATH=YES with -verbose |
| `global-overwrite` | pass | 34/34 | -overwrite with an existing output |
| `global-D-edging` | pass | 46/46 | -DAFNI_VOLREG_EDGING=3 |
| `global-deconflict` | pass | 34/34 | -DAFNI_DECONFLICT=YES with an existing output |
| `global-nifti2` | pass | 46/46 | -DAFNI_NIFTI_WRITE_TYPE=2 |
| `global-skip-afnirc` | pass | 46/46 | -skip_afnirc |
| `global-after` | pass | 10/10 | -overwrite after the dataset |
| `stdin-words` | pass | 10/10 | -@: the options read from standard input (against the replica only) |
| `stdin-empty` | both-error | 10/10 | -@ with nothing on standard input (against the replica only) |
| `threads-heptic` | pass | 4/4 | the default at 1, 2, 5, 14 threads: every byte the same |
| `threads-fmriprep` | pass | 4/4 | the default at 1, 2, 5, 14 threads: every byte the same |
| `threads-odd-fourier` | pass | 4/4 | the default at 1, 2, 5, 14 threads: every byte the same |
| `fmriprep-ds000005-run-01` | pass | 49/49 | fMRIPrep's RobustAverage call: single-band, int16 stored, 64 x 64 x 34, 20 volumes |
| `fmriprep-ds000005-run-03` | pass | 49/49 | fMRIPrep's RobustAverage call: two volumes |
| `fmriprep-ds005454` | pass | 49/49 | fMRIPrep's RobustAverage call: multiband, 160 x 160 x 96, oblique |
| `fmriprep-ds003592-echo-1` | pass | 49/49 | fMRIPrep's RobustAverage call: multi-echo (echo 1), oblique |
| `fmriprep-ds004958` | pass | 49/49 | fMRIPrep's RobustAverage call: multiband 8, uint16 stored, three volumes, 24 degrees oblique |
| `fmriprep-ds002603-mb4` | pass | 49/49 | fMRIPrep's RobustAverage call: Philips multiband 4, float32 stored |
| `fmriprep-pepolar-ds005040` | pass | 49/49 | fMRIPrep's RobustAverage call: sdcflows' PEPOLAR reference average, two topup-corrected volumes |
| `fmriprep-pepolar-ds001600` | pass | 49/49 | fMRIPrep's RobustAverage call: the same, 80 x 80 x 60 |
| `fmriprep-ds000256-baseline` | pass | 49/49 | fMRIPrep's RobustAverage call: a child's run (the motion levels' baseline) |
| `fmriprep-ds000256-nofeedback` | pass | 49/49 | fMRIPrep's RobustAverage call: a child's run, the most motion (rotations to 4.7 degrees) |
