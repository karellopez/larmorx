# Parity: N4BiasFieldCorrection

**All cases agree.** 51 cases on the `standard` tier: 46 pass, 5 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants N4BiasFieldCorrection` / `lx.ants.n4_bias_field_correction` (crates larmorx-image, larmorx-ants)
- **Reference:** N4BiasFieldCorrection from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`, run with one thread
- **Test data:** larmorx-testdata `00b862132e9c`, tier `standard`
- **Generated:** 2026-10-12 on Linux x86_64, with `python -m larmorx_validation parity ants-n4 --tier standard`

**Bit-identical to the unmodified ANTs program (glibc's math, one thread): 29 of 45 compared cases.** The others differ where glibc's `expf`, `logf`, `exp` or `sincos` is not correctly rounded for a value the run meets; the table compares each difference with ANTs' own difference between one thread and 2, 3, 4, 8 and 12 threads (a ratio below 1 is inside ANTs' thread band).
**The cause, confirmed: with those glibc functions replaced by correctly rounded ones (CORE-MATH preloaded into the same ANTs program, one thread), 45 of 45 are bit-identical.**
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - ANTS-35: 1 case(s) named it, 1 with changed values (largest difference 54): `options/no-brackets`
  - ANTS-36: 5 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit N -> N; outputs not written): `fmriprep/boldref-coregistration`, `fmriprep/boldref-template`, `fmriprep/brain-extraction-final`, `fmriprep/n4-only`, `options/missing-mask`
  - ANTS-38: 4 case(s) named it, 4 with changed values (largest difference 3.9e+03): `options/mask`, `options/mask-no-rescale`, `options/mask-and-weight`, `inputs/ramp`
  - ITK-02: 2 case(s) named it, 1 with changed values (largest difference 10.9), 2 with other changes (header quatern_b, quatern_c, quatern_d, srow_x, srow_y, srow_z; header quatern_d, srow_x, srow_y): `inputs/dim3-of-4d`, `inputs/bold-first-volume`

| Case | Output | Difference from ANTs as built | max |diff| / band | (1 - NCC) / band |
|---|---|---|---|---|
| `fmriprep/boldref-coregistration` | corrected | 69747 of 139264 differ; max \|diff\| 0.00061, max rel 5.6e-06, 1 - NCC 5.4e-14 | 0.43 | 0.45 |
| `fmriprep/boldref-template` | corrected | 467120 of 1082035 differ; max \|diff\| 0.00439, max rel 1.7e-06, 1 - NCC 8.8e-15 | 0.25 | 0.16 |
| `fmriprep/smriprep-pre-template` | corrected | 5413547 of 5898240 differ; max \|diff\| 0.241, max rel 0.00034, 1 - NCC 4.8e-10 | 1.3 | 0.24 |
| `fmriprep/n4-only` | corrected | 694333 of 5898240 differ; max \|diff\| 0.0501, max rel 8.3e-05, 1 - NCC 1.3e-10 | 0.25 | 0.01 |
| `fmriprep/n4-only` | bias field | 5898130 of 5898240 differ; max \|diff\| 7.86e-05, max rel 8.3e-05, 1 - NCC 2.1e-08 | 0.15 | 0.0053 |
| `fmriprep/t1w-ds000210-final` | corrected | 6707886 of 11534336 differ; max \|diff\| 0.452, max rel 0.00017, 1 - NCC 6.9e-11 | 0.21 | 0.064 |
| `fmriprep/t1w-ds000210-final` | bias field | 11508335 of 11534336 differ; max \|diff\| 0.000274, max rel 0.00017, 1 - NCC 4.8e-09 | 0.21 | 0.17 |
| `fmriprep/t1w-preproc-mask` | corrected | 517729 of 5898240 differ; max \|diff\| 0.000244, max rel 3.4e-07, 1 - NCC 4.7e-14 | 0.023 | 0.012 |
| `options/mask-and-weight` | corrected | 10838 of 35840 differ; max \|diff\| 0.0612, max rel 0.00056, 1 - NCC 1.6e-08 | 0.87 | 0.79 |
| `options/shrink-1` | corrected | 33414 of 35840 differ; max \|diff\| 0.000122, max rel 8e-07, 1 - NCC 4.8e-15 | 0.36 | 0.28 |
| `options/shrink-3` | corrected | 35840 of 35840 differ; max \|diff\| 0.133, max rel 0.0063, 1 - NCC 3.5e-09 | 1.7 | 4.2 |
| `options/shrink-0` | corrected | 33414 of 35840 differ; max \|diff\| 0.000122, max rel 8e-07, 1 - NCC 4.8e-15 | 0.36 | 0.28 |
| `options/distance-30` | corrected | 35824 of 35840 differ; max \|diff\| 0.0275, max rel 0.0039, 1 - NCC 3.8e-10 | 0.4 | 0.058 |
| `options/distance-30` | bias field | 35827 of 35840 differ; max \|diff\| 0.00427, max rel 0.0039, 1 - NCC 2e-07 | 0.4 | 0.092 |
| `options/distance-order-2` | corrected | 35838 of 35840 differ; max \|diff\| 1.49, max rel 0.072, 1 - NCC 5.4e-08 | 0.22 | 0.0028 |
| `inputs/int16-scaled` | corrected | 35836 of 35840 differ; max \|diff\| 0.11, max rel 0.00096, 1 - NCC 5.5e-09 | 0.072 | 0.0031 |
| `inputs/las-sform-only` | corrected | 35840 of 35840 differ; max \|diff\| 0.0402, max rel 0.00024, 1 - NCC 5.7e-11 | 0.46 | 0.31 |
| `inputs/dim2-distance` | corrected | 1280 of 1280 differ; max \|diff\| 0.2, max rel 0.0014, 1 - NCC 1.6e-08 | 0.33 | 0.56 |
| `inputs/bold-first-volume` | corrected | 53416 of 94276 differ; max \|diff\| 0.00165, max rel 2.4e-06, 1 - NCC 2.4e-13 | 0.39 | 0.74 |

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | both produce their outputs, or neither does |
| Header | every field of the 348-byte header identical |
| Data type, shape, affine | identical (affine within 1e-06 mm) |
| larmorx's threads | 4 threads and 1 thread give the same bits |
| Values | bit-identical to ANTs at one thread with correctly rounded math (CORE-MATH preloaded); with ANTs as built (glibc's math), bit-identity reported and NCC > 0.995 where they differ |
| Reported | the difference from ANTs as built next to ANTs' thread band (ANTs at 2, 3, 4, 8, 12 threads vs 1 thread) |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| errors | 6 | 1 | 5 | 0 | 0 |
| fmriprep | 11 | 11 | 0 | 0 | 0 |
| inputs | 12 | 12 | 0 | 0 | 0 |
| options | 22 | 22 | 0 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `errors/shrink-not-integer` | -s 2.5: the parser's Convert<int> throws (ANTs aborts) | ANTs exit -6: "s/antsCommandLineParser.h:133:\nITK ERROR: CommandLineParser(0x5924ed7c9da0): ERROR: Parse error occured during command line argument processing… |
| `errors/dimension-5` | -d 5: 'Unsupported dimension' | ANTs exit 1: 'Unsupported dimension'; larmorx exit 1: 'Unsupported dimension' |
| `errors/missing-input` | an input that does not exist (ANTs crashes) | ANTs exit -11: 'file <tmp>/none.nii.gz does not exist .'; larmorx exit 1: 'file <tmp>/none.nii.gz does not exist . \nlarmorx: N4BiasFieldCorrection: cannot rea… |
| `errors/mesh-wrong-length` | -b [2x2] in 3D: 'Incorrect mesh resolution', exit 1 | ANTs exit 1: ''; larmorx exit 1: '' |
| `errors/mask-other-grid` | a mask on another grid: ITK throws (ANTs aborts) | ANTs exit -6: 't occupy the same physical space! \nInputImage Origin: [2.1000000e+01, 3.0000000e+01, -2.0000000e+01], InputImagePrimary Origin: [2.4000000e+01,… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (50509e30b3e8) |
| larmorx-testdata | 00b862132e9c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
| scipy | 1.15.3 |

## Notes

ANTs runs as its own binary, larmorx in-process through its Python console entry point. Outputs are read with nibabel (values as stored) and the headers from their raw bytes. Inputs named `gen:<name>` are built by `larmorx_validation.parity.ants_inputs` and this suite's builders; real images come from `larmorx-testdata`.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep/boldref-coregistration` | pass | 12/12 | fMRIPrep's BOLD reference (niworkflows func/util.py:487): -d 3, b-spline distance 200, rescale (no effect without a mas… |
| `fmriprep/boldref-template` | pass | 12/12 | the same command on MNI152NLin2009cAsym's fMRIPrep boldref (res-02), weight = the brain probability map resampled onto … |
| `fmriprep/smriprep-pre-template` | pass | 12/12 | sMRIPrep's N4 before the anatomical template (anatomical.py:1502): -d 3, ANTs' defaults (4 levels of 50 iterations, thr… |
| `fmriprep/brain-extraction-pass1` | pass | 9/9 | sMRIPrep's brain extraction, first N4 (niworkflows anat/ants.py:243): -b [200] -c [50x50x50x50,1e-7] -s 4, on the trunc… |
| `fmriprep/brain-extraction-final` | pass | 14/14 | sMRIPrep's final N4 (anat/ants.py:704): -b [200] -c [50x50x50x50x50,1e-7] -s 4 -r, weight = the WM probability map, bia… |
| `fmriprep/n4-only` | pass | 20/20 | sMRIPrep's N4 for skull-stripped T1w images (anat/ants.py:927): the final N4's options without a weight image, on a bra… |
| `fmriprep/sdcflows-magnitude` | pass | 9/9 | sdcflows' brain extraction of a fieldmap magnitude (workflows/ancillary.py:87): -c [50x50x50x50x50,1e-7] -s 4; ds003012… |
| `fmriprep/sdcflows-magnitude-ds001600` | pass | 9/9 | the same command on ds001600's magnitude1 (acq-v4) |
| `fmriprep/t1w-ds000210-final` | pass | 20/20 | the final N4's options (without weights) on ds000210's T1w |
| `fmriprep/boldref-mask` | pass | 14/14 | ds000005's boldref with its brain mask: outside-mask voxels restored, the rest rescaled to the input's range in the mas… |
| `fmriprep/t1w-preproc-mask` | pass | 12/12 | fMRIPrep's preprocessed T1w with -x the brain mask, -r 0, shrink 2 |
| `options/defaults` | pass | 9/9 | no option but input and output |
| `options/mask` | pass | 14/14 | a binary mask: restoration outside, rescaling inside; bias field |
| `options/mask-no-rescale` | pass | 9/9 | a mask with -r 0 |
| `options/fractional-mask` | pass | 9/9 | a float mask (0, 0.3, 0.6, 0.999, 1, 1.7, 2): N4 uses every non-zero voxel; only voxels within 4 ulps of 1 are rescaled |
| `options/weight` | pass | 14/14 | a probability map as weight image (exact 0, 0.5 and 1) |
| `options/mask-and-weight` | pass | 12/12 | mask and weight image together |
| `options/shrink-1` | pass | 12/12 | -s 1: no subsampling |
| `options/shrink-3` | pass | 12/12 | -s 3 |
| `options/shrink-0` | pass | 12/12 | -s 0: ITK makes it 1 |
| `options/distance-30` | pass | 20/20 | -b [30]: padding to whole spans of 30 mm, 2 or 3 spans per axis |
| `options/distance-order-2` | pass | 12/12 | -b [25,2]: quadratic splines |
| `options/mesh` | pass | 9/9 | -b [2x3x1]: a mesh of 2 x 3 x 1 elements |
| `options/order-5` | pass | 9/9 | -b [1x1x1,5]: quintic splines (Cox-de Boor kernels) |
| `options/convergence-stop` | pass | 9/9 | -c [30x30,0.001]: levels end as soon as the convergence value is not above 0.001 |
| `options/one-level` | pass | 9/9 | -c [15]: one fitting level, no refinement |
| `options/no-brackets` | pass | 9/9 | -c 50x50 and -b 200 without brackets set nothing: ITK's one level of 50 iterations with threshold 0.001, 4 control poin… |
| `options/histogram-100` | pass | 9/9 | -t [0.3,0.02,100]: 100 bins (FFT of 256) |
| `options/histogram-256` | pass | 9/9 | -t [0.15,0.01,256]: 256 bins; log(256)/log(2) rounds above 8, so the FFT has 1024 points |
| `options/histogram-600` | pass | 9/9 | -t [0.15,0.01,600]: an FFT of 2048 points, where glibc's sincos is not correctly rounded for 4 twiddle factors |
| `options/output-one-parameter` | pass | 9/9 | -o [out]: one bracketed name |
| `options/missing-mask` | pass | 9/9 | a mask that does not exist: ones are used, but the mask counts as given (restoration and rescaling run) |
| `options/verbose` | pass | 9/9 | -v 1 (stdout is not compared: it holds timings and ITK's object dump) |
| `inputs/int16-scaled` | pass | 12/12 | scaled int16 input |
| `inputs/outliers` | pass | 9/9 | hot, negative and zero voxels (NaN and -inf read as 0): non-positive values keep their value in the log image |
| `inputs/constant` | pass | 9/9 | a constant image (the histogram has no width) |
| `inputs/ramp` | pass | 9/9 | values increasing in memory order: ITK's histogram minimum is never set |
| `inputs/las-sform-only` | pass | 12/12 | an sform-only LAS image |
| `inputs/negative-pixdim` | pass | 9/9 | a qform-only image with a negative pixdim |
| `inputs/dim2` | pass | 14/14 | -d 2 on a 2D image |
| `inputs/dim2-distance` | pass | 12/12 | -d 2 with -b [20] and -s 2 |
| `inputs/dim4` | pass | 14/14 | -d 4: the series shrunk along space only, a spline along time too |
| `inputs/dim3-of-4d` | pass | 9/9 | -d 3 on a 4D file: its first volume, with an identity direction |
| `inputs/dimension-from-file` | pass | 9/9 | no -d: the dimension is the file's |
| `inputs/bold-first-volume` | pass | 12/12 | -d 3 on fMRIPrep's preprocessed BOLD series: its first volume |
| `errors/help` | pass | 1/1 | --help: the usage, exit 0 |
| `errors/shrink-not-integer` | both-error | 1/1 | -s 2.5: the parser's Convert<int> throws (ANTs aborts) |
| `errors/dimension-5` | both-error | 1/1 | -d 5: 'Unsupported dimension' |
| `errors/missing-input` | both-error | 1/1 | an input that does not exist (ANTs crashes) |
| `errors/mesh-wrong-length` | both-error | 1/1 | -b [2x2] in 3D: 'Incorrect mesh resolution', exit 1 |
| `errors/mask-other-grid` | both-error | 1/1 | a mask on another grid: ITK throws (ANTs aborts) |
