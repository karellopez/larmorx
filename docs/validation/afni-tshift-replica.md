# Parity: 3dTshift (replica)

**All cases agree.** 236 cases on the `standard` tier: 217 pass, 17 rejected by both, 2 expected divergences, 0 failures.

- **Validated:** `larmorx-gpl afni 3dTshift` (crate larmorx-gpl-afni, the GPL-3.0-or-later replica translated from AFNI 25.2.09's source), run as a separate process
- **Reference:** 3dTshift from AFNI (AFNI_25.2.09), the binary built by `scripts/build_afni_oracle.sh`
- **Test data:** larmorx-testdata `5434909ddf09`, tier `standard`
- **Generated:** 2026-10-10 on Linux x86_64, with `python -m larmorx_validation parity afni-tshift --tier standard --implementation replica`

**Bit-identical: 217 of 217 passing cases** produce exactly the bytes of AFNI's output data.
- Fourier: 97 of 97 bit-identical.
- Lagrange and weighted-sinc methods and copies: 120 of 120 bit-identical.
- The clean-room original (`larmorx afni 3dTshift`, Apache-2.0) has its own record: [afni-tshift.md](afni-tshift.md).

| Category | Compared | Bit-identical | Worst float diff (× max AFNI) | Worst integer diff | Most values differing (fraction) |
|---|---|---|---|---|---|
| copy | 6 | 6 | 0.0e+00 | 0 | 0.0e+00 |
| header-timing | 12 | 12 | 0.0e+00 | – | 0.0e+00 |
| methods | 84 | 84 | 0.0e+00 | 0 | 0.0e+00 |
| options | 42 | 42 | 0.0e+00 | 0 | 0.0e+00 |
| real | 20 | 20 | 0.0e+00 | 0 | 0.0e+00 |
| synthetic | 33 | 33 | 0.0e+00 | 0 | 0.0e+00 |
| tpattern | 20 | 20 | 0.0e+00 | – | 0.0e+00 |

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | both succeed, or both reject the arguments |
| Output data type, shape, scaling (`scl_slope`), TR, `toffset`, units, slice fields, xform codes | identical |
| Geometry: sform and qform (read by nibabel) | max \|diff\| ≤ 1e-06 mm |
| float32 outputs | max \|diff\| ≤ 1e-05 × max \|AFNI output\| (specs/3dTshift.md §9); bit-identity reported |
| Integer outputs | max \|diff\| ≤ 1, with the fraction of differing values |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| copy | 6 | 6 | 0 | 0 | 0 |
| divergence | 2 | 0 | 0 | 2 | 0 |
| errors | 17 | 0 | 17 | 0 | 0 |
| header-timing | 12 | 12 | 0 | 0 | 0 |
| methods | 84 | 84 | 0 | 0 | 0 |
| options | 42 | 42 | 0 | 0 | 0 |
| real | 20 | 20 | 0 | 0 | 0 |
| synthetic | 33 | 33 | 0 | 0 | 0 |
| tpattern | 20 | 20 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `divergence/afni-format-output` | larmorx writes NIfTI only; it rejects a -prefix that does not end in .nii or .nii.gz, where AFNI writes its own format. |
| `divergence/voxshift` | -voxshift is not supported by larmorx (fMRIPrep does not use it). |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `errors/unknown-option` | an unknown option | AFNI: ** FATAL ERROR: Unknown option: -bogus; larmorx: ** FATAL ERROR: Unknown option: -bogus |
| `errors/no-detrend-fourier` | -no_detrend while the method is Fourier | AFNI: ** FATAL ERROR: found -no_detrend, changing default to -heptic; larmorx: ** FATAL ERROR: found -no_detrend, changing default to -heptic |
| `errors/rlt-no-detrend` | -rlt with -no_detrend | AFNI: ** FATAL ERROR: cannot use both -rlt and -no_detrend; larmorx: ** FATAL ERROR: cannot use both -rlt and -no_detrend |
| `errors/unknown-pattern` | pattern names are case-sensitive | AFNI: ** ERROR: Unknown tpattern = ALT+Z; larmorx: ** ERROR: Unknown tpattern = ALT+Z |
| `errors/slice-too-large` | -slice beyond the last | AFNI: ** FATAL ERROR: -slice value is too large (4 >= 4); larmorx: ** FATAL ERROR: -slice value is too large (4 >= 4) |
| `errors/ignore-too-large` | -ignore > nt - 5 | AFNI: ** FATAL ERROR: -ignore value 96 is too large; larmorx: ** FATAL ERROR: -ignore value 96 is too large |
| `errors/negative-tzero` | -tzero < 0 | AFNI: ** FATAL ERROR: illegal value '-1' after -tzero!; larmorx: ** FATAL ERROR: illegal value '-1' after -tzero! |
| `errors/negative-ignore` | -ignore < 0 | AFNI: ** FATAL ERROR: -ignore value -1 is negative!; larmorx: ** FATAL ERROR: -ignore value -1 is negative! |
| `errors/zero-TR` | -TR 0 | AFNI: ** FATAL ERROR: illegal value '0' after -TR!; larmorx: ** FATAL ERROR: illegal value '0' after -TR! |
| `errors/bad-TR` | -TR abc | AFNI: ** FATAL ERROR: illegal value 'abc' after -TR!; larmorx: ** FATAL ERROR: illegal value 'abc' after -TR! |
| `errors/missing-file` | a missing -tpattern file | AFNI: ** FATAL ERROR: Can't read tpattern file <tmp>/none.1D; larmorx: ** FATAL ERROR: Can't read tpattern file <tmp>/none.1D |
| `errors/file-too-short` | a -tpattern file with fewer values than slices | AFNI: ** FATAL ERROR: tpattern file <tmp>/st.1D has 3 values but have 4 slices; larmorx: ** FATAL ERROR: tpattern file <tmp>/st.1D has 3 values but have 4 slic… |
| `errors/file-beyond-tr` | a -tpattern value beyond the TR | AFNI: ** FATAL ERROR: Illegal value 2.5 in tpattern file <tmp>/st.1D; larmorx: ** FATAL ERROR: Illegal value 2.5 in tpattern file <tmp>/st.1D |
| `errors/file-negative` | a negative -tpattern value | AFNI: ** FATAL ERROR: Illegal value -0.5 in tpattern file <tmp>/st.1D; larmorx: ** FATAL ERROR: Illegal value -0.5 in tpattern file <tmp>/st.1D |
| `errors/file-text` | a -tpattern file with text | AFNI: ** FATAL ERROR: Can't read tpattern file <tmp>/st.1D; larmorx: ** FATAL ERROR: Can't read tpattern file <tmp>/st.1D |
| `errors/missing-dataset` | an input file that does not exist | AFNI: ** FATAL ERROR: Can't open input dataset '<generated>/does-not-exist.nii'; larmorx: ** FATAL ERROR: Can't open input dataset '<generated>/does-not-exist.… |
| `errors/existing-output` | an output that exists already (AFNI exits 0 without writing; larmorx exits 1) | AFNI: ** ERROR: dataset NOT written to disk!; larmorx: ** FATAL ERROR: output dataset name '<tmp>/out.nii' conflicts with existing file |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (6d1ca26b1633-dirty) |
| larmorx-testdata | 5434909ddf09 |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

Both programs read the same files with the same arguments; outputs are read with nibabel and compared as stored (before `scl_slope`). Real runs pass the BIDS `SliceTiming` as fMRIPrep does: a tab-separated `-tpattern` file of `str(float)` values (reversed when `SliceEncodingDirection` ends in `-`) and `-tzero round(min + 0.5 * (max - min), 3)`. Header-timing inputs are generated from the synthetic float32 file by setting `slice_code`, `slice_start`, `slice_end`, `slice_duration`, `dim_info` and the time unit. larmorx-gpl uses all logical CPUs (`OMP_NUM_THREADS` unset); its results do not depend on the thread count.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `synthetic/content/clipping` | pass | 11/11 | default Fourier, -tpattern alt+z: Series swinging between -32760 and 32760: shifted values clip to ±32767 (AFNI never w… |
| `synthetic/content/constant` | pass | 11/11 | default Fourier, -tpattern alt+z: Constant series (every voxel 500): detrending leaves zeros, the range is a point. |
| `synthetic/content/half-integers` | pass | 11/11 | default Fourier, -tpattern alt+z: float32 series made of k + 0.5 values (exact rounding ties when written as int16 by o… |
| `synthetic/content/steps` | pass | 11/11 | default Fourier, -tpattern alt+z: Step functions: the Fourier shift rings (Gibbs), so the clipping to the input range m… |
| `synthetic/dtypes/float32-nonfinite` | pass | 11/11 | default Fourier, -tpattern alt+z: float32 with NaN, +inf and -inf samples: AFNI reads them as 0. |
| `synthetic/dtypes/float32-slope` | pass | 11/11 | default Fourier, -tpattern alt+z: float32 with scl_slope 2 and no intercept: AFNI keeps it as a brick factor even for f… |
| `synthetic/dtypes/float32` | pass | 11/11 | default Fourier, -tpattern alt+z: float32: shifted values are stored as computed. |
| `synthetic/dtypes/float64` | pass | 11/11 | default Fourier, -tpattern alt+z: float64: AFNI converts to float32 on read and writes float32. |
| `synthetic/dtypes/int16-negative-slope` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 with scl_slope -0.5: AFNI applies a negative brick factor on output but not on … |
| `synthetic/dtypes/int16-slope-inter` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 with scl_slope 0.37 and scl_inter 12.5: AFNI converts to float32. |
| `synthetic/dtypes/int16-slope` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 with scl_slope 0.25 and no intercept: a brick factor (output stays int16). |
| `synthetic/dtypes/int16` | pass | 11/11 | default Fourier, -tpattern alt+z: int16, no scaling: AFNI keeps short and rounds the output with SHORTIZE. |
| `synthetic/dtypes/int32` | pass | 11/11 | default Fourier, -tpattern alt+z: int32: AFNI converts to float32 on read and writes float32. |
| `synthetic/dtypes/uint8` | pass | 11/11 | default Fourier, -tpattern alt+z: uint8: AFNI keeps byte and rounds the output with BYTEIZE. |
| `synthetic/lengths/nt0012-fft16` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 12 time points: AFNI's FFT length is 16 (fft16). |
| `synthetic/lengths/nt0016-fft20` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 16 time points: AFNI's FFT length is 20 (radix-5). |
| `synthetic/lengths/nt0020-fft24` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 20 time points: AFNI's FFT length is 24 (radix-3). |
| `synthetic/lengths/nt0028-fft32` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 28 time points: AFNI's FFT length is 32 (fft32). |
| `synthetic/lengths/nt0056-fft60` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 56 time points: AFNI's FFT length is 60 (radix-15). |
| `synthetic/lengths/nt0060-fft64` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 60 time points: AFNI's FFT length is 64 (fft64). |
| `synthetic/lengths/nt0116-fft120` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 116 time points: AFNI's FFT length is 120 (radix-15). |
| `synthetic/lengths/nt0124-fft128` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 124 time points: AFNI's FFT length is 128 (fft128). |
| `synthetic/lengths/nt0156-fft160` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 156 time points: AFNI's FFT length is 160 (radix-5). |
| `synthetic/lengths/nt0188-fft192` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 188 time points: AFNI's FFT length is 192 (radix-3). |
| `synthetic/lengths/nt0236-fft240` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 236 time points: AFNI's FFT length is 240 (radix-15). |
| `synthetic/lengths/nt0252-fft256` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 252 time points: AFNI's FFT length is 256 (fft256). |
| `synthetic/lengths/nt0476-fft480` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 476 time points: AFNI's FFT length is 480 (radix-15). |
| `synthetic/lengths/nt0508-fft512` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 508 time points: AFNI's FFT length is 512 (fft512). |
| `synthetic/lengths/nt0764-fft768` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 764 time points: AFNI's FFT length is 768 (radix-3). |
| `synthetic/lengths/nt1020-fft1024` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 1020 time points: AFNI's FFT length is 1024 (fft1024). |
| `synthetic/lengths/nt1276-fft1280` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 1276 time points: AFNI's FFT length is 1280 (radix-5). |
| `synthetic/lengths/nt2044-fft2048` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 2044 time points: AFNI's FFT length is 2048 (fft2048). |
| `synthetic/lengths/nt4092-fft4096` | pass | 11/11 | default Fourier, -tpattern alt+z: int16 series of 4092 time points: AFNI's FFT length is 4096 (fft_4dec). |
| `methods/content/clipping-linear` | pass | 11/11 | -linear, -tpattern alt-z, on content/clipping |
| `methods/content/clipping-cubic` | pass | 11/11 | -cubic, -tpattern alt-z, on content/clipping |
| `methods/content/clipping-quintic` | pass | 11/11 | -quintic, -tpattern alt-z, on content/clipping |
| `methods/content/clipping-heptic` | pass | 11/11 | -heptic, -tpattern alt-z, on content/clipping |
| `methods/content/clipping-wsinc5` | pass | 11/11 | -wsinc5, -tpattern alt-z, on content/clipping |
| `methods/content/clipping-wsinc9` | pass | 11/11 | -wsinc9, -tpattern alt-z, on content/clipping |
| `methods/content/constant-linear` | pass | 11/11 | -linear, -tpattern alt-z, on content/constant |
| `methods/content/constant-cubic` | pass | 11/11 | -cubic, -tpattern alt-z, on content/constant |
| `methods/content/constant-quintic` | pass | 11/11 | -quintic, -tpattern alt-z, on content/constant |
| `methods/content/constant-heptic` | pass | 11/11 | -heptic, -tpattern alt-z, on content/constant |
| `methods/content/constant-wsinc5` | pass | 11/11 | -wsinc5, -tpattern alt-z, on content/constant |
| `methods/content/constant-wsinc9` | pass | 11/11 | -wsinc9, -tpattern alt-z, on content/constant |
| `methods/content/half-integers-linear` | pass | 11/11 | -linear, -tpattern alt-z, on content/half-integers |
| `methods/content/half-integers-cubic` | pass | 11/11 | -cubic, -tpattern alt-z, on content/half-integers |
| `methods/content/half-integers-quintic` | pass | 11/11 | -quintic, -tpattern alt-z, on content/half-integers |
| `methods/content/half-integers-heptic` | pass | 11/11 | -heptic, -tpattern alt-z, on content/half-integers |
| `methods/content/half-integers-wsinc5` | pass | 11/11 | -wsinc5, -tpattern alt-z, on content/half-integers |
| `methods/content/half-integers-wsinc9` | pass | 11/11 | -wsinc9, -tpattern alt-z, on content/half-integers |
| `methods/content/steps-linear` | pass | 11/11 | -linear, -tpattern alt-z, on content/steps |
| `methods/content/steps-cubic` | pass | 11/11 | -cubic, -tpattern alt-z, on content/steps |
| `methods/content/steps-quintic` | pass | 11/11 | -quintic, -tpattern alt-z, on content/steps |
| `methods/content/steps-heptic` | pass | 11/11 | -heptic, -tpattern alt-z, on content/steps |
| `methods/content/steps-wsinc5` | pass | 11/11 | -wsinc5, -tpattern alt-z, on content/steps |
| `methods/content/steps-wsinc9` | pass | 11/11 | -wsinc9, -tpattern alt-z, on content/steps |
| `methods/dtypes/float32-nonfinite-linear` | pass | 11/11 | -linear, -tpattern alt-z, on dtypes/float32-nonfinite |
| `methods/dtypes/float32-nonfinite-cubic` | pass | 11/11 | -cubic, -tpattern alt-z, on dtypes/float32-nonfinite |
| `methods/dtypes/float32-nonfinite-quintic` | pass | 11/11 | -quintic, -tpattern alt-z, on dtypes/float32-nonfinite |
| `methods/dtypes/float32-nonfinite-heptic` | pass | 11/11 | -heptic, -tpattern alt-z, on dtypes/float32-nonfinite |
| `methods/dtypes/float32-nonfinite-wsinc5` | pass | 11/11 | -wsinc5, -tpattern alt-z, on dtypes/float32-nonfinite |
| `methods/dtypes/float32-nonfinite-wsinc9` | pass | 11/11 | -wsinc9, -tpattern alt-z, on dtypes/float32-nonfinite |
| `methods/dtypes/float32-slope-linear` | pass | 11/11 | -linear, -tpattern alt-z, on dtypes/float32-slope |
| `methods/dtypes/float32-slope-cubic` | pass | 11/11 | -cubic, -tpattern alt-z, on dtypes/float32-slope |
| `methods/dtypes/float32-slope-quintic` | pass | 11/11 | -quintic, -tpattern alt-z, on dtypes/float32-slope |
| `methods/dtypes/float32-slope-heptic` | pass | 11/11 | -heptic, -tpattern alt-z, on dtypes/float32-slope |
| `methods/dtypes/float32-slope-wsinc5` | pass | 11/11 | -wsinc5, -tpattern alt-z, on dtypes/float32-slope |
| `methods/dtypes/float32-slope-wsinc9` | pass | 11/11 | -wsinc9, -tpattern alt-z, on dtypes/float32-slope |
| `methods/dtypes/float32-linear` | pass | 11/11 | -linear, -tpattern alt-z, on dtypes/float32 |
| `methods/dtypes/float32-cubic` | pass | 11/11 | -cubic, -tpattern alt-z, on dtypes/float32 |
| `methods/dtypes/float32-quintic` | pass | 11/11 | -quintic, -tpattern alt-z, on dtypes/float32 |
| `methods/dtypes/float32-heptic` | pass | 11/11 | -heptic, -tpattern alt-z, on dtypes/float32 |
| `methods/dtypes/float32-wsinc5` | pass | 11/11 | -wsinc5, -tpattern alt-z, on dtypes/float32 |
| `methods/dtypes/float32-wsinc9` | pass | 11/11 | -wsinc9, -tpattern alt-z, on dtypes/float32 |
| `methods/dtypes/float64-linear` | pass | 11/11 | -linear, -tpattern alt-z, on dtypes/float64 |
| `methods/dtypes/float64-cubic` | pass | 11/11 | -cubic, -tpattern alt-z, on dtypes/float64 |
| `methods/dtypes/float64-quintic` | pass | 11/11 | -quintic, -tpattern alt-z, on dtypes/float64 |
| `methods/dtypes/float64-heptic` | pass | 11/11 | -heptic, -tpattern alt-z, on dtypes/float64 |
| `methods/dtypes/float64-wsinc5` | pass | 11/11 | -wsinc5, -tpattern alt-z, on dtypes/float64 |
| `methods/dtypes/float64-wsinc9` | pass | 11/11 | -wsinc9, -tpattern alt-z, on dtypes/float64 |
| `methods/dtypes/int16-negative-slope-linear` | pass | 11/11 | -linear, -tpattern alt-z, on dtypes/int16-negative-slope |
| `methods/dtypes/int16-negative-slope-cubic` | pass | 11/11 | -cubic, -tpattern alt-z, on dtypes/int16-negative-slope |
| `methods/dtypes/int16-negative-slope-quintic` | pass | 11/11 | -quintic, -tpattern alt-z, on dtypes/int16-negative-slope |
| `methods/dtypes/int16-negative-slope-heptic` | pass | 11/11 | -heptic, -tpattern alt-z, on dtypes/int16-negative-slope |
| `methods/dtypes/int16-negative-slope-wsinc5` | pass | 11/11 | -wsinc5, -tpattern alt-z, on dtypes/int16-negative-slope |
| `methods/dtypes/int16-negative-slope-wsinc9` | pass | 11/11 | -wsinc9, -tpattern alt-z, on dtypes/int16-negative-slope |
| `methods/dtypes/int16-slope-inter-linear` | pass | 11/11 | -linear, -tpattern alt-z, on dtypes/int16-slope-inter |
| `methods/dtypes/int16-slope-inter-cubic` | pass | 11/11 | -cubic, -tpattern alt-z, on dtypes/int16-slope-inter |
| `methods/dtypes/int16-slope-inter-quintic` | pass | 11/11 | -quintic, -tpattern alt-z, on dtypes/int16-slope-inter |
| `methods/dtypes/int16-slope-inter-heptic` | pass | 11/11 | -heptic, -tpattern alt-z, on dtypes/int16-slope-inter |
| `methods/dtypes/int16-slope-inter-wsinc5` | pass | 11/11 | -wsinc5, -tpattern alt-z, on dtypes/int16-slope-inter |
| `methods/dtypes/int16-slope-inter-wsinc9` | pass | 11/11 | -wsinc9, -tpattern alt-z, on dtypes/int16-slope-inter |
| `methods/dtypes/int16-slope-linear` | pass | 11/11 | -linear, -tpattern alt-z, on dtypes/int16-slope |
| `methods/dtypes/int16-slope-cubic` | pass | 11/11 | -cubic, -tpattern alt-z, on dtypes/int16-slope |
| `methods/dtypes/int16-slope-quintic` | pass | 11/11 | -quintic, -tpattern alt-z, on dtypes/int16-slope |
| `methods/dtypes/int16-slope-heptic` | pass | 11/11 | -heptic, -tpattern alt-z, on dtypes/int16-slope |
| `methods/dtypes/int16-slope-wsinc5` | pass | 11/11 | -wsinc5, -tpattern alt-z, on dtypes/int16-slope |
| `methods/dtypes/int16-slope-wsinc9` | pass | 11/11 | -wsinc9, -tpattern alt-z, on dtypes/int16-slope |
| `methods/dtypes/int16-linear` | pass | 11/11 | -linear, -tpattern alt-z, on dtypes/int16 |
| `methods/dtypes/int16-cubic` | pass | 11/11 | -cubic, -tpattern alt-z, on dtypes/int16 |
| `methods/dtypes/int16-quintic` | pass | 11/11 | -quintic, -tpattern alt-z, on dtypes/int16 |
| `methods/dtypes/int16-heptic` | pass | 11/11 | -heptic, -tpattern alt-z, on dtypes/int16 |
| `methods/dtypes/int16-wsinc5` | pass | 11/11 | -wsinc5, -tpattern alt-z, on dtypes/int16 |
| `methods/dtypes/int16-wsinc9` | pass | 11/11 | -wsinc9, -tpattern alt-z, on dtypes/int16 |
| `methods/dtypes/int32-linear` | pass | 11/11 | -linear, -tpattern alt-z, on dtypes/int32 |
| `methods/dtypes/int32-cubic` | pass | 11/11 | -cubic, -tpattern alt-z, on dtypes/int32 |
| `methods/dtypes/int32-quintic` | pass | 11/11 | -quintic, -tpattern alt-z, on dtypes/int32 |
| `methods/dtypes/int32-heptic` | pass | 11/11 | -heptic, -tpattern alt-z, on dtypes/int32 |
| `methods/dtypes/int32-wsinc5` | pass | 11/11 | -wsinc5, -tpattern alt-z, on dtypes/int32 |
| `methods/dtypes/int32-wsinc9` | pass | 11/11 | -wsinc9, -tpattern alt-z, on dtypes/int32 |
| `methods/dtypes/uint8-linear` | pass | 11/11 | -linear, -tpattern alt-z, on dtypes/uint8 |
| `methods/dtypes/uint8-cubic` | pass | 11/11 | -cubic, -tpattern alt-z, on dtypes/uint8 |
| `methods/dtypes/uint8-quintic` | pass | 11/11 | -quintic, -tpattern alt-z, on dtypes/uint8 |
| `methods/dtypes/uint8-heptic` | pass | 11/11 | -heptic, -tpattern alt-z, on dtypes/uint8 |
| `methods/dtypes/uint8-wsinc5` | pass | 11/11 | -wsinc5, -tpattern alt-z, on dtypes/uint8 |
| `methods/dtypes/uint8-wsinc9` | pass | 11/11 | -wsinc9, -tpattern alt-z, on dtypes/uint8 |
| `options/float32-ignore` | pass | 11/11 | -ignore 3: three leading points kept out of fit and shift (float32) |
| `options/float32-tzero-0` | pass | 11/11 | -tzero 0: every slice moved to the start of the TR (float32) |
| `options/float32-tzero-0.5` | pass | 11/11 | -tzero 0.5 (float32) |
| `options/float32-tzero-beyond-tr` | pass | 11/11 | -tzero 5: beyond the TR (allowed) (float32) |
| `options/float32-slice-2` | pass | 11/11 | -slice 2: align to slice 2's time (float32) |
| `options/float32-slice-wins` | pass | 11/11 | -tzero and -slice: -slice wins (float32) |
| `options/float32-rlt` | pass | 11/11 | -rlt: no trend added back (float32) |
| `options/float32-rlt+` | pass | 11/11 | -rlt+: only the intercept added back (float32) |
| `options/float32-rlt-last-wins` | pass | 11/11 | -rlt then -rlt+: the last wins (float32) |
| `options/float32-no-detrend` | pass | 11/11 | -heptic -no_detrend: mean removed only (float32) |
| `options/float32-no-detrend-fourier` | pass | 11/11 | -no_detrend then -Fourier (allowed, with a warning) (float32) |
| `options/float32-TR-seconds` | pass | 11/11 | -TR 2.5s (float32) |
| `options/float32-TR-plain` | pass | 11/11 | -TR 1.7 (no unit) (float32) |
| `options/float32-TR-ms` | pass | 11/11 | -TR 2000ms: every time in milliseconds (float32) |
| `options/float32-TR-msec` | pass | 11/11 | -TR 2000msec -tzero 500 (float32) |
| `options/float32-verbose` | pass | 11/11 | -verbose (float32) |
| `options/float32-abbreviations` | pass | 11/11 | -cub and -verb: AFNI's abbreviations (float32) |
| `options/float32-wsinc-case` | pass | 11/11 | -WSINC5: case-insensitive (float32) |
| `options/float32-ignore-heptic` | pass | 11/11 | -ignore 5 with -heptic (float32) |
| `options/float32-rlt-wsinc9` | pass | 11/11 | -rlt with -wsinc9 (float32) |
| `options/float32-rlt+-linear` | pass | 11/11 | -rlt+ with -linear (float32) |
| `options/int16-ignore` | pass | 11/11 | -ignore 3: three leading points kept out of fit and shift (int16) |
| `options/int16-tzero-0` | pass | 11/11 | -tzero 0: every slice moved to the start of the TR (int16) |
| `options/int16-tzero-0.5` | pass | 11/11 | -tzero 0.5 (int16) |
| `options/int16-tzero-beyond-tr` | pass | 11/11 | -tzero 5: beyond the TR (allowed) (int16) |
| `options/int16-slice-2` | pass | 11/11 | -slice 2: align to slice 2's time (int16) |
| `options/int16-slice-wins` | pass | 11/11 | -tzero and -slice: -slice wins (int16) |
| `options/int16-rlt` | pass | 11/11 | -rlt: no trend added back (int16) |
| `options/int16-rlt+` | pass | 11/11 | -rlt+: only the intercept added back (int16) |
| `options/int16-rlt-last-wins` | pass | 11/11 | -rlt then -rlt+: the last wins (int16) |
| `options/int16-no-detrend` | pass | 11/11 | -heptic -no_detrend: mean removed only (int16) |
| `options/int16-no-detrend-fourier` | pass | 11/11 | -no_detrend then -Fourier (allowed, with a warning) (int16) |
| `options/int16-TR-seconds` | pass | 11/11 | -TR 2.5s (int16) |
| `options/int16-TR-plain` | pass | 11/11 | -TR 1.7 (no unit) (int16) |
| `options/int16-TR-ms` | pass | 11/11 | -TR 2000ms: every time in milliseconds (int16) |
| `options/int16-TR-msec` | pass | 11/11 | -TR 2000msec -tzero 500 (int16) |
| `options/int16-verbose` | pass | 11/11 | -verbose (int16) |
| `options/int16-abbreviations` | pass | 11/11 | -cub and -verb: AFNI's abbreviations (int16) |
| `options/int16-wsinc-case` | pass | 11/11 | -WSINC5: case-insensitive (int16) |
| `options/int16-ignore-heptic` | pass | 11/11 | -ignore 5 with -heptic (int16) |
| `options/int16-rlt-wsinc9` | pass | 11/11 | -rlt with -wsinc9 (int16) |
| `options/int16-rlt+-linear` | pass | 11/11 | -rlt+ with -linear (int16) |
| `tpattern/alt+z` | pass | 11/11 | -tpattern alt+z |
| `tpattern/altplus` | pass | 11/11 | -tpattern altplus |
| `tpattern/alt+z2` | pass | 11/11 | -tpattern alt+z2 |
| `tpattern/alt-z` | pass | 11/11 | -tpattern alt-z |
| `tpattern/altminus` | pass | 11/11 | -tpattern altminus |
| `tpattern/alt-z2` | pass | 11/11 | -tpattern alt-z2 |
| `tpattern/seq+z` | pass | 11/11 | -tpattern seq+z |
| `tpattern/seqplus` | pass | 11/11 | -tpattern seqplus |
| `tpattern/seq-z` | pass | 11/11 | -tpattern seq-z |
| `tpattern/seqminus` | pass | 11/11 | -tpattern seqminus |
| `tpattern/zero` | pass | 11/11 | -tpattern zero |
| `tpattern/simult` | pass | 11/11 | -tpattern simult |
| `tpattern/file-tabs` | pass | 11/11 | -tpattern @file: one line, tab-separated (as fMRIPrep writes it) |
| `tpattern/file-lines` | pass | 11/11 | -tpattern @file: one value per line |
| `tpattern/file-matrix` | pass | 11/11 | -tpattern @file: two columns: read column by column |
| `tpattern/file-comments` | pass | 11/11 | -tpattern @file: comments and commas |
| `tpattern/file-extra` | pass | 11/11 | -tpattern @file: more values than slices |
| `tpattern/file-repeat` | pass | 11/11 | -tpattern @file: N@value repetition |
| `tpattern/file-tr` | pass | 11/11 | -tpattern @file: a value equal to the TR |
| `tpattern/inline-1D` | pass | 11/11 | -tpattern '@1D: 0 1 0.5 1.5' |
| `header-timing/code1-seq-inc` | pass | 11/11 | no -tpattern, slice_code 1 (sequential increasing) |
| `header-timing/code2-seq-dec` | pass | 11/11 | no -tpattern, slice_code 2: AFNI gives every slice time 0 |
| `header-timing/code3-alt-inc` | pass | 11/11 | no -tpattern, slice_code 3 (alternating increasing) |
| `header-timing/code4-alt-dec` | pass | 11/11 | no -tpattern, slice_code 4 (alternating decreasing) |
| `header-timing/code5-alt-inc2` | pass | 11/11 | no -tpattern, slice_code 5 (alternating increasing, from slice 1) |
| `header-timing/code6-alt-dec2` | pass | 11/11 | no -tpattern, slice_code 6 (alternating decreasing, from nz-2) |
| `header-timing/code3-partial` | pass | 11/11 | no -tpattern, slice_code 3 over slices 1..2 only |
| `header-timing/code3-ms` | pass | 11/11 | no -tpattern, slice_code 3 with the TR and duration in milliseconds |
| `header-timing/code3-beyond-tr` | pass | 11/11 | no -tpattern, times beyond the TR: a copy of the input, timing kept |
| `header-timing/code3-no-slice-dim` | pass | 11/11 | no -tpattern, no slice axis in dim_info: no timing, a copy |
| `header-timing/code3-zero-duration` | pass | 11/11 | no -tpattern, slice_duration 0: no timing, a copy |
| `header-timing/tpattern-overrides` | pass | 11/11 | -tpattern overrides the header's timing |
| `copy/no-timing-int16` | pass | 11/11 | no slice timing anywhere: the output is a copy of the input (int16) |
| `copy/no-timing-int16-slope` | pass | 11/11 | no slice timing anywhere: the output is a copy of the input (int16 with a brick factor) |
| `copy/no-timing-int16-slope-inter` | pass | 11/11 | no slice timing anywhere: the output is a copy of the input (int16 scaled to float32) |
| `copy/no-timing-float64` | pass | 11/11 | no slice timing anywhere: the output is a copy of the input (float64 converted to float32) |
| `copy/no-timing-float32-nonfinite` | pass | 11/11 | no slice timing anywhere: the output is a copy of the input (non-finite floats zeroed) |
| `copy/3d-input` | pass | 11/11 | a 3D image: one value per voxel, written back as 3D |
| `errors/unknown-option` | both-error | – | an unknown option |
| `errors/no-detrend-fourier` | both-error | – | -no_detrend while the method is Fourier |
| `errors/rlt-no-detrend` | both-error | – | -rlt with -no_detrend |
| `errors/unknown-pattern` | both-error | – | pattern names are case-sensitive |
| `errors/slice-too-large` | both-error | – | -slice beyond the last |
| `errors/ignore-too-large` | both-error | – | -ignore > nt - 5 |
| `errors/negative-tzero` | both-error | – | -tzero < 0 |
| `errors/negative-ignore` | both-error | – | -ignore < 0 |
| `errors/zero-TR` | both-error | – | -TR 0 |
| `errors/bad-TR` | both-error | – | -TR abc |
| `errors/missing-file` | both-error | – | a missing -tpattern file |
| `errors/file-too-short` | both-error | – | a -tpattern file with fewer values than slices |
| `errors/file-beyond-tr` | both-error | – | a -tpattern value beyond the TR |
| `errors/file-negative` | both-error | – | a negative -tpattern value |
| `errors/file-text` | both-error | – | a -tpattern file with text |
| `errors/missing-dataset` | both-error | – | an input file that does not exist |
| `errors/existing-output` | both-error | – | an output that exists already (AFNI exits 0 without writing; larmorx exits 1) |
| `divergence/afni-format-output` | expected-divergence | 1/1 | -prefix without .nii: AFNI writes its own BRIK/HEAD format |
| `divergence/voxshift` | expected-divergence | 1/1 | -voxshift: per-voxel shifts from a dataset |
| `real/ds001600-acq-v4` | pass | 11/11 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-1_task-rest_acq-v4_bold |
| `real/ds001600-acq-v1` | pass | 11/11 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-1_task-rest_acq-v1_bold |
| `real/ds001600-acq-v2` | pass | 11/11 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-1_task-rest_acq-v2_bold |
| `real/ds001600-acq-PA` | pass | 11/11 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-1_task-rest_acq-PA_bold |
| `real/ds000210-echo-1` | pass | 11/11 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-02_task-cuedSGT_run-01_echo-1_bold |
| `real/ds000210-echo-2` | pass | 11/11 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-02_task-cuedSGT_run-01_echo-2_bold |
| `real/ds000210-echo-3` | pass | 11/11 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-02_task-cuedSGT_run-01_echo-3_bold |
| `real/ds006736` | pass | 11/11 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-004_task-freeRecall_bold |
| `real/ds006010-uint16` | pass | 11/11 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-206_task-category_run-01_bold |
| `real/ds003345-ms-header` | pass | 11/11 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-22973_task-PenaltyKik_run-02_bold |
| `real/ds005454-mb4-96-slices` | pass | 11/11 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-16_task-rest_bold |
| `real/ds003345-linear` | pass | 11/11 | fMRIPrep's call with -linear |
| `real/ds003345-cubic` | pass | 11/11 | fMRIPrep's call with -cubic |
| `real/ds003345-quintic` | pass | 11/11 | fMRIPrep's call with -quintic |
| `real/ds003345-heptic` | pass | 11/11 | fMRIPrep's call with -heptic |
| `real/ds003345-wsinc5` | pass | 11/11 | fMRIPrep's call with -wsinc5 |
| `real/ds003345-wsinc9` | pass | 11/11 | fMRIPrep's call with -wsinc9 |
| `real/ds003345-ignore-4` | pass | 11/11 | fMRIPrep's call with -ignore 4 (non-steady-state volumes) |
| `real/ds000210-header-timing` | pass | 11/11 | no -tpattern: the slice timing in the header (slice_code 3, ALT_INC) |
| `real/ds001600-header-zero-duration` | pass | 11/11 | no -tpattern: slice_code 5 with slice_duration 0, so a copy of the input |
