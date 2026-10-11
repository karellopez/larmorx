# Parity: DenoiseImage

**All cases agree.** 51 cases on the `standard` tier: 42 pass, 9 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants DenoiseImage` / `lx.ants.denoise_image` (crates larmorx-image, larmorx-ants)
- **Reference:** DenoiseImage from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), g++ (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0, the binary built by `scripts/build_ants_oracle.sh` (with ITK's remote module AdaptiveDenoising at `24825c8d`), run with one thread
- **Test data:** larmorx-testdata `00b862132e9c`, tier `standard`
- **Generated:** 2026-10-11 on Linux x86_64, with `python -m larmorx_validation parity ants-denoise --tier standard`

**Bit-identical to the unmodified ANTs program (glibc's math, one thread): 7 of 40 compared cases.** The others differ where glibc's `expf` (or `exp`) is not correctly rounded for a value the run meets; the table compares each difference with ANTs' own difference between one thread and 2, 4 and 12 threads (a ratio below 1 is inside ANTs' thread band).
**The cause, confirmed: with those glibc functions replaced by correctly rounded ones (CORE-MATH preloaded into the same ANTs program, one thread), 40 of 40 are bit-identical.**

| Case | Output | Difference from ANTs as built | max |diff| / band | (1 - NCC) / band |
|---|---|---|---|---|
| `fmriprep/smriprep-t1w` | denoised | 5522 of 5898240 differ; max \|diff\| 0.000198, max rel 8.3e-05, 1 - NCC 0 | 7.3e-07 | 0 |
| `fmriprep/smriprep-t1w-ds000210` | denoised | 5389 of 11534336 differ; max \|diff\| 0.00214, max rel 0.00079, 1 - NCC 2.2e-13 | 3.6e-06 | 3.5e-09 |
| `fmriprep/smriprep-t1w-ds000210` | noise | 5389 of 11534336 differ; max \|diff\| 0.00214, max rel 0.011, 1 - NCC 0 | 3.6e-06 | 0 |
| `fmriprep/t1w-preproc-mask` | denoised | 495 of 5898240 differ; max \|diff\| 0.000122, max rel 7.1e-07, 1 - NCC 0 | 8.1e-07 | 0 |
| `fmriprep/t1w-preproc-mask` | noise | 495 of 5898240 differ; max \|diff\| 0.000122, max rel 0.00056, 1 - NCC 0 | 8.1e-07 | 0 |
| `fmriprep/mni-t1w` | denoised | 766 of 1082035 differ; max \|diff\| 0.000977, max rel 5.9e-07, 1 - NCC 0 | 9.7e-07 | 0 |
| `fmriprep/boldref` | denoised | 44 of 139264 differ; max \|diff\| 0.000122, max rel 1.6e-06, 1 - NCC 0 | 6.9e-07 | 0 |
| `fmriprep/boldref` | noise | 44 of 139264 differ; max \|diff\| 0.000122, max rel 3.7e-05, 1 - NCC 0 | 6.9e-07 | 0 |
| `fmriprep/magnitude` | denoised | 112 of 147456 differ; max \|diff\| 0.00016, max rel 2e-06, 1 - NCC 3.3e-16 | 3.9e-07 | 8.8e-13 |
| `options/defaults` | denoised | 44 of 35840 differ; max \|diff\| 3.05e-05, max rel 2.2e-07, 1 - NCC 0 | 4e-05 | 0 |
| `options/rician` | denoised | 25 of 35840 differ; max \|diff\| 3.05e-05, max rel 6.6e-07, 1 - NCC 2.2e-16 | 4.2e-07 | 2.5e-14 |
| `options/rician` | noise | 25 of 35840 differ; max \|diff\| 3.05e-05, max rel 0.00014, 1 - NCC 2.2e-16 | 4.2e-07 | 1.9e-15 |
| `options/gaussian` | denoised | 44 of 35840 differ; max \|diff\| 3.05e-05, max rel 2.2e-07, 1 - NCC 0 | 2.4e-05 | 0 |
| `options/gaussian` | noise | 45 of 35840 differ; max \|diff\| 3.05e-05, max rel 3.1e-05, 1 - NCC 4.4e-16 | 2.4e-05 | 1.3e-09 |
| `options/model-case` | denoised | 25 of 35840 differ; max \|diff\| 3.05e-05, max rel 6.6e-07, 1 - NCC 2.2e-16 | 4.2e-07 | 3.4e-14 |
| `options/mask-rician` | denoised | 10 of 35840 differ; max \|diff\| 2.29e-05, max rel 5.2e-07, 1 - NCC 2.2e-16 | 3.8e-07 | 1.6e-13 |
| `options/mask-rician` | noise | 10 of 35840 differ; max \|diff\| 2.29e-05, max rel 7.2e-06, 1 - NCC 6.7e-16 | 3.8e-07 | 8e-15 |
| `options/mask-gaussian` | denoised | 10 of 35840 differ; max \|diff\| 1.53e-05, max rel 1.4e-07, 1 - NCC 0 | 2.4e-06 | 0 |
| `options/mask-fractional` | denoised | 2 of 35840 differ; max \|diff\| 7.63e-06, max rel 6.8e-08, 1 - NCC 0 | 1.6e-07 | 0 |
| `options/mask-values` | denoised | 1 of 35840 differ; max \|diff\| 1.53e-05, max rel 9.6e-08, 1 - NCC 1.1e-16 | 2.8e-07 | 7.2e-14 |
| `options/mask-larger` | denoised | 10 of 35840 differ; max \|diff\| 2.29e-05, max rel 5.2e-07, 1 - NCC 2.2e-16 | 4.4e-07 | 1.7e-13 |
| `options/missing-mask` | denoised | 25 of 35840 differ; max \|diff\| 3.05e-05, max rel 6.6e-07, 1 - NCC 2.2e-16 | 4.2e-07 | 2.3e-14 |
| `options/shrink-2` | denoised | 93 of 35840 differ; max \|diff\| 1.91e-05, max rel 2.7e-06, 1 - NCC 2.2e-16 | 2.9e-07 | 1.2e-14 |
| `options/shrink-2` | noise | 160 of 35840 differ; max \|diff\| 1.72e-05, max rel 4.4e-06, 1 - NCC 0 | 2.6e-07 | 0 |
| `options/shrink-0` | denoised | 44 of 35840 differ; max \|diff\| 3.05e-05, max rel 2.2e-07, 1 - NCC 0 | 6.9e-06 | 0 |
| `options/patch-2` | denoised | 25 of 35840 differ; max \|diff\| 3.05e-05, max rel 8.2e-07, 1 - NCC 1.1e-16 | 4.1e-07 | 5.8e-15 |
| `options/patch-0` | denoised | 11 of 35840 differ; max \|diff\| 3.05e-05, max rel 8.8e-07, 1 - NCC 7.8e-16 | inf | inf |
| `options/search-1` | denoised | 27 of 35840 differ; max \|diff\| 2.29e-05, max rel 2.2e-07, 1 - NCC 1.1e-16 | 3.7e-06 | 1.5e-10 |
| `options/search-3` | denoised | 23 of 35840 differ; max \|diff\| 7.44e-05, max rel 3e-06, 1 - NCC 2.2e-16 | 1e-06 | 2.4e-14 |
| `options/radii-per-axis` | denoised | 29 of 35840 differ; max \|diff\| 4.2e-05, max rel 1.5e-06, 1 - NCC 0 | 5.5e-07 | 0 |
| `options/output-one-parameter` | denoised | 44 of 35840 differ; max \|diff\| 3.05e-05, max rel 2.2e-07, 1 - NCC 0 | 0.00013 | 0 |
| `options/verbose` | denoised | 44 of 35840 differ; max \|diff\| 3.05e-05, max rel 2.2e-07, 1 - NCC 0 | 4.1e-05 | 0 |
| `inputs/int16-scaled` | denoised | 21 of 35840 differ; max \|diff\| 3.05e-05, max rel 2.6e-07, 1 - NCC 0 | 4.2e-07 | 0 |
| `inputs/outliers` | denoised | 21 of 35840 differ; max \|diff\| 0.0117, max rel 9.1e-07, 1 - NCC 0 | 1e-07 | 0 |
| `inputs/outliers` | noise | 21 of 35840 differ; max \|diff\| 0.0117, max rel 4.6e-06, 1 - NCC 0 | 1e-07 | 0 |
| `inputs/outliers-gaussian` | denoised | 37 of 35840 differ; max \|diff\| 0.000977, max rel 2.7e-07, 1 - NCC 0 | 0.00022 | 0 |
| `inputs/negative-pixdim` | denoised | 22 of 35840 differ; max \|diff\| 9.16e-05, max rel 5.6e-06, 1 - NCC 2.2e-16 | 1.3e-06 | 2.4e-14 |
| `inputs/dim2` | denoised | 1 of 1280 differ; max \|diff\| 1.53e-05, max rel 1.1e-07, 1 - NCC 0 | 2.3e-07 | 0 |
| `inputs/dim2` | noise | 1 of 1280 differ; max \|diff\| 1.53e-05, max rel 8.8e-07, 1 - NCC 5.6e-16 | 2.3e-07 | 7.1e-15 |
| `inputs/dim4` | denoised | 43 of 22400 differ; max \|diff\| 1.91e-05, max rel 7e-07, 1 - NCC 1.1e-15 | 1.1e-06 | 1.6e-13 |
| `inputs/dim4` | noise | 46 of 22400 differ; max \|diff\| 1.91e-05, max rel 2.9e-06, 1 - NCC 4.4e-16 | 1.1e-06 | 6.8e-13 |
| `inputs/dim3-of-4d` | denoised | 6 of 4480 differ; max \|diff\| 1.14e-05, max rel 3.3e-07, 1 - NCC 2.2e-16 | 5.9e-07 | 3.5e-14 |
| `inputs/dimension-from-file` | denoised | 25 of 35840 differ; max \|diff\| 3.05e-05, max rel 6.6e-07, 1 - NCC 2.2e-16 | 4.2e-07 | 2.3e-14 |

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | both produce their outputs, or neither does |
| Header | every field of the 348-byte header identical |
| Data type, shape, affine | identical (affine within 1e-06 mm) |
| larmorx's threads | 4 threads and 1 thread give the same bits |
| Values | with ANTs as built (glibc's math), bit-identity reported and NCC > 0.995 where they differ; bit-identical to ANTs at one thread with correctly rounded math (CORE-MATH preloaded) |
| Reported | the difference from ANTs as built next to ANTs' thread band (ANTs at 2, 4, 12 threads vs 1 thread) |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| errors | 10 | 1 | 9 | 0 | 0 |
| fmriprep | 6 | 6 | 0 | 0 | 0 |
| inputs | 13 | 13 | 0 | 0 | 0 |
| options | 22 | 22 | 0 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `errors/shrink-not-integer` | -s 1.5: the parser's Convert<int> throws (ANTs aborts) | ANTs exit -6: "s/antsCommandLineParser.h:133:\nITK ERROR: CommandLineParser(0x647a03dc8ab0): ERROR: Parse error occured during command line argument processing… |
| `errors/dimension-5` | -d 5: 'Unsupported dimension' | ANTs exit 1: 'Unsupported dimension'; larmorx exit 1: 'Unsupported dimension' |
| `errors/missing-input` | an input that does not exist (ANTs crashes) | ANTs exit -11: 'file <tmp>/none.nii.gz does not exist .'; larmorx exit 1: 'file <tmp>/none.nii.gz does not exist . \nlarmorx: DenoiseImage: cannot read the inp… |
| `errors/unknown-model` | -n Poisson: exit 1 | ANTs exit 1: ''; larmorx exit 1: '' |
| `errors/patch-wrong-length` | -p 1x1 in 3D: exit 1 | ANTs exit 1: ''; larmorx exit 1: '' |
| `errors/search-bracketed` | -r [2]: the option's name is empty, so no radius is read: exit 1 | ANTs exit 1: ''; larmorx exit 1: '' |
| `errors/mask-other-grid` | a mask on another grid: ITK throws, exit 1 | ANTs exit 1: ''; larmorx exit 1: '' |
| `errors/mask-full-resolution-shrink-2` | -s 2 with a full-resolution mask: not on the shrunk grid, ITK throws, exit 1 | ANTs exit 1: ''; larmorx exit 1: '' |
| `errors/mask-smaller` | a mask that does not cover the image: ITK's requested region is outside it, exit 1 | ANTs exit 1: ''; larmorx exit 1: '' |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (0c7c7a49882c-dirty) |
| larmorx-testdata | 00b862132e9c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

ANTs runs as its own binary, larmorx in-process through its Python console entry point. Outputs are read with nibabel (values as stored) and the headers from their raw bytes. Inputs named `gen:<name>` are built by `larmorx_validation.parity.ants_inputs` and this suite's builders; real images come from `larmorx-testdata`.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep/smriprep-t1w` | pass | 11/11 | sMRIPrep's denoising of each T1w (anatomical.py:1461), as nipype writes it: -i <T1w> -n Rician -o <out> -s 1, no -d; ds… |
| `fmriprep/smriprep-t1w-ds000210` | pass | 19/19 | the same command on ds000210's T1w (256 x 256 x 176, float64 on disk), the noise saved as nipype writes it with save_no… |
| `fmriprep/t1w-preproc-mask` | pass | 19/19 | fMRIPrep's preprocessed T1w (ds000005) with -n Rician and its brain mask |
| `fmriprep/mni-t1w` | pass | 11/11 | sMRIPrep's options on MNI152NLin2009cAsym's T1w template (res-02, 97 x 115 x 97) |
| `fmriprep/boldref` | pass | 19/19 | sMRIPrep's options on ds000005's BOLD reference (64 x 64 x 34, int16) |
| `fmriprep/magnitude` | pass | 11/11 | sMRIPrep's options on ds003012's fieldmap magnitude (64 x 64 x 36, int32) |
| `options/defaults` | pass | 11/11 | no option but input and output: the Gaussian noise model |
| `options/rician` | pass | 19/19 | -n Rician, with the noise image |
| `options/gaussian` | pass | 19/19 | -n Gaussian, with the noise image |
| `options/model-case` | pass | 11/11 | -n RICIAN: the model name is case-insensitive |
| `options/mask-rician` | pass | 19/19 | -x a binary mask (uint8), Rician: only voxels in the mask are centres; the bias correction only in the mask |
| `options/mask-gaussian` | pass | 11/11 | -x a binary mask, Gaussian |
| `options/mask-fractional` | pass | 11/11 | a float mask (0, 0.3, 0.6, 0.999, 1, 1.7, 2) read as unsigned char: below 1 is 0 |
| `options/mask-values` | pass | 11/11 | a float mask with 0.4, 255, 256, 300.5 and -1: the C++ casts to unsigned char (256 is 0, -1 is 255) |
| `options/mask-larger` | pass | 11/11 | a mask larger than the image on the same grid: read by the image's indices |
| `options/missing-mask` | pass | 11/11 | a mask that does not exist: ANTs prints a message and denoises without a mask |
| `options/shrink-2` | pass | 19/19 | -s 2: the filter runs on every other voxel; the noise is resampled linearly (float coordinates) onto the full grid |
| `options/shrink-3` | pass | 13/13 | -s 3, Gaussian |
| `options/shrink-0` | pass | 11/11 | -s 0: ITK makes it 1 |
| `options/shrink-2-mask` | pass | 8/8 | -s 2 with a mask on the shrunk grid |
| `options/patch-2` | pass | 11/11 | -p 2: patches of 5 x 5 x 5 |
| `options/patch-0` | pass | 11/11 | -p 0: patches of one voxel |
| `options/search-1` | pass | 11/11 | -r 1 |
| `options/search-3` | pass | 11/11 | -r 3, Rician |
| `options/radii-per-axis` | pass | 11/11 | -p 1x2x1 -r 2x1x3: one radius per axis |
| `options/output-one-parameter` | pass | 11/11 | -o [out]: one bracketed name |
| `options/no-output` | pass | 1/1 | no -o: the filter runs, nothing is written, exit 0 |
| `options/verbose` | pass | 11/11 | -v 1 (stdout is not compared: it holds timings and ITK's object dump) |
| `inputs/int16-scaled` | pass | 11/11 | scaled int16 input |
| `inputs/outliers` | pass | 19/19 | hot, negative and zero voxels, a NaN and -inf: they spread to their patches |
| `inputs/outliers-gaussian` | pass | 11/11 | the same, Gaussian |
| `inputs/constant` | pass | 8/8 | a constant image (every local variance 0: no centre takes part) |
| `inputs/las-sform-only` | pass | 8/8 | an sform-only LAS image, -s 2 (the noise resampled through the oblique grid) |
| `inputs/negative-pixdim` | pass | 11/11 | a qform-only image with a negative pixdim |
| `inputs/dim2` | pass | 19/19 | -d 2 on a 2D image, with the noise |
| `inputs/dim2-mask-shrink` | pass | 8/8 | -d 2 with a 2D mask and -p 2, Gaussian |
| `inputs/dim2-shrink-2` | pass | 8/8 | -d 2 with -s 2 |
| `inputs/dim4` | pass | 19/19 | -d 4: 4D patches and search boxes (time is an axis like the others) |
| `inputs/dim4-shrink-2` | pass | 13/13 | -d 4 -s 2: time is shrunk too; the noise resampled in 4D |
| `inputs/dim3-of-4d` | pass | 11/11 | -d 3 on a 4D file: its first volume, with an identity direction |
| `inputs/dimension-from-file` | pass | 11/11 | no -d: the dimension is the file's |
| `errors/help` | pass | 1/1 | --help: the usage, exit 0 |
| `errors/shrink-not-integer` | both-error | – | -s 1.5: the parser's Convert<int> throws (ANTs aborts) |
| `errors/dimension-5` | both-error | – | -d 5: 'Unsupported dimension' |
| `errors/missing-input` | both-error | – | an input that does not exist (ANTs crashes) |
| `errors/unknown-model` | both-error | – | -n Poisson: exit 1 |
| `errors/patch-wrong-length` | both-error | – | -p 1x1 in 3D: exit 1 |
| `errors/search-bracketed` | both-error | – | -r [2]: the option's name is empty, so no radius is read: exit 1 |
| `errors/mask-other-grid` | both-error | – | a mask on another grid: ITK throws, exit 1 |
| `errors/mask-full-resolution-shrink-2` | both-error | – | -s 2 with a full-resolution mask: not on the shrunk grid, ITK throws, exit 1 |
| `errors/mask-smaller` | both-error | – | a mask that does not cover the image: ITK's requested region is outside it, exit 1 |
