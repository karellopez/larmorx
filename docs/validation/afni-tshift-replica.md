# Parity: 3dTshift (replica)

**All cases agree.** 308 cases on the `standard` tier: 276 pass, 29 rejected by both, 3 expected divergences, 0 failures.

- **Validated:** `larmorx-gpl afni 3dTshift` (larmorx-gpl 0.0.1; crate larmorx-gpl-afni, the GPL-3.0-or-later replica translated from AFNI 25.2.09's source), run by larmorx as a separate process: `larmorx afni 3dTshift` with `LARMORX_IMPLEMENTATION=replica` and `lx.afni.tshift(..., implementation="replica")`
- **Reference:** 3dTshift from AFNI (AFNI_25.2.09), the binary built by `scripts/build_afni_oracle.sh`
- **Test data:** larmorx-testdata `00b862132e9c`, tier `standard`
- **Generated:** 2026-10-12 on Linux x86_64, with `python -m larmorx_validation parity afni-tshift --tier standard --implementation replica`

**Bit-identical: 276 of 276 passing cases** produce exactly the bytes of AFNI's output data.
- Fourier: 145 of 145 bit-identical.
- Lagrange and weighted-sinc methods and copies: 131 of 131 bit-identical.
- The clean-room original (`larmorx afni 3dTshift`, Apache-2.0) has its own record: [afni-tshift.md](afni-tshift.md).
- Python API (`lx.afni.tshift(..., implementation="replica")`): run on the 194 passing cases it can express; 194 give the command line's output bit for bit (data and header). From an image in memory: 149 of 149 give the same values.
- **AFNI's global options and environment variables** (categories `global` and `environment`, 57 cases): exit status and messages compared too, standard output and error byte for byte (colour codes included; AFNI's version banner and compile date, and larmorx's own lines, set aside): 57 of 57 identical.
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - AFNI-01: 7 case(s) named it, 7 with changed values (largest difference 8.54e+03): `synthetic/dtypes/int16-negative-slope`, `methods/dtypes/int16-negative-slope-linear`, `methods/dtypes/int16-negative-slope-cubic`, `methods/dtypes/int16-negative-slope-quintic`, `methods/dtypes/int16-negative-slope-heptic`, `methods/dtypes/int16-negative-slope-wsinc5`, `methods/dtypes/int16-negative-slope-wsinc9`
  - AFNI-02: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (header scl_slope): `defects/f32-slope-inter`, `defects/f32-slope-inter-copy`
  - AFNI-03: 2 case(s) named it, 1 with changed values (largest difference 27.8), 1 with other changes (header slice_code): `header-timing/code2-seq-dec`, `header-timing/code2-beyond-tr`
  - AFNI-05: 4 case(s) named it, 4 with changed values (largest difference 122): `options/float32-no-detrend`, `options/float32-no-detrend-fourier`, `options/int16-no-detrend`, `options/int16-no-detrend-fourier`
  - AFNI-07: 5 case(s) named it, 0 with changed values (largest difference 0), 5 with other changes (header qoffset_x, srow_x; header srow_x): `defects/metre`, `defects/micron-copy`, `defects/oblique-metre`, `geometry/slightly-oblique-metre`, `defects/oblique-metre-zero-origin`
  - AFNI-08: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (header slice_end): `defects/tiny-offsets-copy`
  - AFNI-16: 3 case(s) named it, 0 with changed values (largest difference 0), 3 with other changes (header qoffset_x, srow_x; header srow_x): `geometry/slightly-oblique`, `geometry/slightly-oblique-qform`, `geometry/slightly-oblique-metre`
  - AFNI-19: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (toffset): `environment/allow-milliseconds`
  - AFNI-22: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (messages): `global/D-message-prefix-with-blank`
- **D13 defaults, replica against the clean-room original** (`upstream_defects="fix"`, the 24 cases where a correction applies or that test one): 16 bit-identical, 7 within the original's thresholds (Fourier and other last bits, as in the original's own record), 1 where the original is known to diverge from AFNI, 0 disagreeing.
  - `defects/tiny-offsets-copy`: known (the original diverges from AFNI here): differ: header fields (scaling, timing, slice fields, units, xform codes): slice_code: replica 0, original 1; slice_duration: replica 0.0, original 0.0005000000237487257

| Category | Compared | Bit-identical | Worst float diff (× max AFNI) | Worst integer diff | Most values differing (fraction) |
|---|---|---|---|---|---|
| copy | 8 | 8 | 0.0e+00 | 0 | 0.0e+00 |
| defects | 8 | 8 | 0.0e+00 | 0 | 0.0e+00 |
| environment | 26 | 26 | 0.0e+00 | 0 | 0.0e+00 |
| geometry | 3 | 3 | 0.0e+00 | – | 0.0e+00 |
| global | 19 | 19 | 0.0e+00 | 0 | 0.0e+00 |
| header-timing | 13 | 13 | 0.0e+00 | – | 0.0e+00 |
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
| copy | 8 | 8 | 0 | 0 | 0 |
| defects | 8 | 8 | 0 | 0 | 0 |
| divergence | 2 | 0 | 0 | 2 | 0 |
| environment | 31 | 26 | 4 | 1 | 0 |
| errors | 18 | 0 | 18 | 0 | 0 |
| geometry | 3 | 3 | 0 | 0 | 0 |
| global | 26 | 19 | 7 | 0 | 0 |
| header-timing | 13 | 13 | 0 | 0 | 0 |
| methods | 84 | 84 | 0 | 0 | 0 |
| options | 42 | 42 | 0 | 0 | 0 |
| real | 20 | 20 | 0 | 0 | 0 |
| synthetic | 33 | 33 | 0 | 0 | 0 |
| tpattern | 20 | 20 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `environment/one-row-dataset` | AFNI writes a dataset of one row as a 1D text file under the NIfTI name, with a random identifier (AFNI-20, unless AFNI_WRITE_1D_AS_PREFIX is YES); larmorx writes NIfTI in both modes, as AFNI does with that variable set (environment/write-1D-as-prefix) |
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
| `errors/existing-output` | an output that exists already (AFNI exits 0 without writing; larmorx exits 1, AFNI-11) | AFNI: ** ERROR: dataset NOT written to disk!; larmorx: ** ERROR: dataset NOT written to disk! |
| `errors/prefix-with-blank` | a -prefix with a blank (AFNI refuses it; larmorx writes it, AFNI-12) | AFNI: ** FATAL ERROR: illegal value '<tmp>/afni-out put.nii' after -prefix; larmorx: ** FATAL ERROR: illegal value '<tmp>/larmorx-out put.nii' after -prefix |
| `global/D-deconflict-no` | -DAFNI_DECONFLICT=NO: an existing output stops the shift before the work (exit 1) | AFNI exit 1: ** FATAL ERROR: output dataset already exists './out.nii'; larmorx exit 1: ** FATAL ERROR: output dataset already exists './out.nii' (the same mes… |
| `global/D-blank-before-value` | -DAFNI_ALLOW_ARBITRARY_FILENAMES= YES: AFNI sets '=YES', not YES: refused | AFNI exit 1: ** FATAL ERROR: illegal value 'out put.nii' after -prefix; larmorx exit 1: ** FATAL ERROR: illegal value 'out put.nii' after -prefix (the same mes… |
| `global/D-empty-value` | -DAFNI_ALLOW_ARBITRARY_FILENAMES= (no value) sets nothing: refused | AFNI exit 1: ** FATAL ERROR: illegal value 'out put.nii' after -prefix; larmorx exit 1: ** FATAL ERROR: illegal value 'out put.nii' after -prefix (the same mes… |
| `global/D-empty-name` | -D=YES (no name) sets nothing | AFNI exit 1: ** FATAL ERROR: illegal value 'out put.nii' after -prefix; larmorx exit 1: ** FATAL ERROR: illegal value 'out put.nii' after -prefix (the same mes… |
| `global/D-without-equals` | -DAFNI_ALLOW_ARBITRARY_FILENAMES (no '='): an unknown option | AFNI exit 1: ** FATAL ERROR: Unknown option: -DAFNI_ALLOW_ARBITRARY_FILENAMES; larmorx exit 1: ** FATAL ERROR: Unknown option: -DAFNI_ALLOW_ARBITRARY_FILENAMES… |
| `global/D-alone` | -D alone: an unknown option | AFNI exit 1: ** FATAL ERROR: Unknown option: -D; larmorx exit 1: ** FATAL ERROR: Unknown option: -D (the same messages) |
| `global/D-lowercase` | -d...=YES: not a global option, an unknown option | AFNI exit 1: ** FATAL ERROR: Unknown option: -dAFNI_ALLOW_ARBITRARY_FILENAMES=YES; larmorx exit 1: ** FATAL ERROR: Unknown option: -dAFNI_ALLOW_ARBITRARY_FILEN… |
| `environment/deconflict-no-copy` | AFNI_DECONFLICT=NO on a copy: AFNI finds the output after the work, exit 0 (AFNI-11) | AFNI exit 0: ** ERROR: dataset NOT written to disk!; larmorx exit 0: ** ERROR: dataset NOT written to disk! (the same messages) |
| `environment/input-with-blank` | an input in a directory with a blank: AFNI reads a list of datasets and fails (AFNI-18) | AFNI exit 1: ** FATAL ERROR: Can't open input dataset '<generated>/with blank/in.nii'; larmorx exit 1: ** FATAL ERROR: Can't open input dataset '<generated>/wi… |
| `environment/1D-text-error` | text in the -tpattern file: AFNI's parse error | AFNI exit 1: ** FATAL ERROR: Can't read tpattern file st.1D; larmorx exit 1: ** FATAL ERROR: Can't read tpattern file st.1D (the same messages) |
| `environment/use-fgets` | AFNI_USE_FGETS=YES: a file with CR line ends is one row, of 8 values (9 > TR) | AFNI exit 1: ** FATAL ERROR: Illegal value 9 in tpattern file st.1D; larmorx exit 1: ** FATAL ERROR: Illegal value 9 in tpattern file st.1D (the same messages) |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (4aaad72f285c) |
| larmorx-testdata | 00b862132e9c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

Both programs read the same files with the same arguments; outputs are read with nibabel and compared as stored (before `scl_slope`). Real runs pass the BIDS `SliceTiming` as fMRIPrep does: a tab-separated `-tpattern` file of `str(float)` values (reversed when `SliceEncodingDirection` ends in `-`) and `-tzero round(min + 0.5 * (max - min), 3)`. Header-timing inputs are generated from the synthetic float32 file by setting `slice_code`, `slice_start`, `slice_end`, `slice_duration`, `dim_info` and the time unit. larmorx-gpl uses all logical CPUs (`OMP_NUM_THREADS` unset) on the command line and 1 thread through the Python API (its default); its results do not depend on the thread count. The Python API runs every passing case it can express (a `-tpattern` name or fMRIPrep's slice timing, options in seconds; not the header's timing, AFNI 1D files or copies of the input) and must give the command line's output: the same values and the same header bytes (`vox_offset` aside); for unscaled `uint8`, `int16` and `float32` inputs it also runs on the image loaded in memory. Both programs run without AFNI's environment variables of this process and with `HOME` an empty directory (no `.afnirc`), `AFNI_DONT_LOGFILE=YES` and `AFNI_NIFTI_TYPE_WARN=NO`, plus each case's variables. The cases of AFNI's global options and environment variables run each program in a directory of its own and compare exit statuses and messages too.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `synthetic/content/clipping` | pass | 14/14 | default Fourier, -tpattern alt+z: Series swinging between -32760 and 32760: shifted values clip to ±32767 (AFNI never w… |
| `synthetic/content/constant` | pass | 14/14 | default Fourier, -tpattern alt+z: Constant series (every voxel 500): detrending leaves zeros, the range is a point. |
| `synthetic/content/half-integers` | pass | 14/14 | default Fourier, -tpattern alt+z: float32 series made of k + 0.5 values (exact rounding ties when written as int16 by o… |
| `synthetic/content/steps` | pass | 14/14 | default Fourier, -tpattern alt+z: Step functions: the Fourier shift rings (Gibbs), so the clipping to the input range m… |
| `synthetic/dtypes/float32-nonfinite` | pass | 14/14 | default Fourier, -tpattern alt+z: float32 with NaN, +inf and -inf samples: AFNI reads them as 0. |
| `synthetic/dtypes/float32-slope` | pass | 13/13 | default Fourier, -tpattern alt+z: float32 with scl_slope 2 and no intercept: AFNI keeps it as a brick factor even for f… |
| `synthetic/dtypes/float32` | pass | 14/14 | default Fourier, -tpattern alt+z: float32: shifted values are stored as computed. |
| `synthetic/dtypes/float64` | pass | 13/13 | default Fourier, -tpattern alt+z: float64: AFNI converts to float32 on read and writes float32. |
| `synthetic/dtypes/int16-negative-slope` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 with scl_slope -0.5: AFNI applies a negative brick factor on output but not on … |
| `synthetic/dtypes/int16-slope-inter` | pass | 13/13 | default Fourier, -tpattern alt+z: int16 with scl_slope 0.37 and scl_inter 12.5: AFNI converts to float32. |
| `synthetic/dtypes/int16-slope` | pass | 13/13 | default Fourier, -tpattern alt+z: int16 with scl_slope 0.25 and no intercept: a brick factor (output stays int16). |
| `synthetic/dtypes/int16` | pass | 14/14 | default Fourier, -tpattern alt+z: int16, no scaling: AFNI keeps short and rounds the output with SHORTIZE. |
| `synthetic/dtypes/int32` | pass | 13/13 | default Fourier, -tpattern alt+z: int32: AFNI converts to float32 on read and writes float32. |
| `synthetic/dtypes/uint8` | pass | 14/14 | default Fourier, -tpattern alt+z: uint8: AFNI keeps byte and rounds the output with BYTEIZE. |
| `synthetic/lengths/nt0012-fft16` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 12 time points: AFNI's FFT length is 16 (fft16). |
| `synthetic/lengths/nt0016-fft20` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 16 time points: AFNI's FFT length is 20 (radix-5). |
| `synthetic/lengths/nt0020-fft24` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 20 time points: AFNI's FFT length is 24 (radix-3). |
| `synthetic/lengths/nt0028-fft32` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 28 time points: AFNI's FFT length is 32 (fft32). |
| `synthetic/lengths/nt0056-fft60` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 56 time points: AFNI's FFT length is 60 (radix-15). |
| `synthetic/lengths/nt0060-fft64` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 60 time points: AFNI's FFT length is 64 (fft64). |
| `synthetic/lengths/nt0116-fft120` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 116 time points: AFNI's FFT length is 120 (radix-15). |
| `synthetic/lengths/nt0124-fft128` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 124 time points: AFNI's FFT length is 128 (fft128). |
| `synthetic/lengths/nt0156-fft160` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 156 time points: AFNI's FFT length is 160 (radix-5). |
| `synthetic/lengths/nt0188-fft192` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 188 time points: AFNI's FFT length is 192 (radix-3). |
| `synthetic/lengths/nt0236-fft240` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 236 time points: AFNI's FFT length is 240 (radix-15). |
| `synthetic/lengths/nt0252-fft256` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 252 time points: AFNI's FFT length is 256 (fft256). |
| `synthetic/lengths/nt0476-fft480` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 476 time points: AFNI's FFT length is 480 (radix-15). |
| `synthetic/lengths/nt0508-fft512` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 508 time points: AFNI's FFT length is 512 (fft512). |
| `synthetic/lengths/nt0764-fft768` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 764 time points: AFNI's FFT length is 768 (radix-3). |
| `synthetic/lengths/nt1020-fft1024` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 1020 time points: AFNI's FFT length is 1024 (fft1024). |
| `synthetic/lengths/nt1276-fft1280` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 1276 time points: AFNI's FFT length is 1280 (radix-5). |
| `synthetic/lengths/nt2044-fft2048` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 2044 time points: AFNI's FFT length is 2048 (fft2048). |
| `synthetic/lengths/nt4092-fft4096` | pass | 14/14 | default Fourier, -tpattern alt+z: int16 series of 4092 time points: AFNI's FFT length is 4096 (fft_4dec). |
| `methods/content/clipping-linear` | pass | 14/14 | -linear, -tpattern alt-z, on content/clipping |
| `methods/content/clipping-cubic` | pass | 14/14 | -cubic, -tpattern alt-z, on content/clipping |
| `methods/content/clipping-quintic` | pass | 14/14 | -quintic, -tpattern alt-z, on content/clipping |
| `methods/content/clipping-heptic` | pass | 14/14 | -heptic, -tpattern alt-z, on content/clipping |
| `methods/content/clipping-wsinc5` | pass | 14/14 | -wsinc5, -tpattern alt-z, on content/clipping |
| `methods/content/clipping-wsinc9` | pass | 14/14 | -wsinc9, -tpattern alt-z, on content/clipping |
| `methods/content/constant-linear` | pass | 14/14 | -linear, -tpattern alt-z, on content/constant |
| `methods/content/constant-cubic` | pass | 14/14 | -cubic, -tpattern alt-z, on content/constant |
| `methods/content/constant-quintic` | pass | 14/14 | -quintic, -tpattern alt-z, on content/constant |
| `methods/content/constant-heptic` | pass | 14/14 | -heptic, -tpattern alt-z, on content/constant |
| `methods/content/constant-wsinc5` | pass | 14/14 | -wsinc5, -tpattern alt-z, on content/constant |
| `methods/content/constant-wsinc9` | pass | 14/14 | -wsinc9, -tpattern alt-z, on content/constant |
| `methods/content/half-integers-linear` | pass | 14/14 | -linear, -tpattern alt-z, on content/half-integers |
| `methods/content/half-integers-cubic` | pass | 14/14 | -cubic, -tpattern alt-z, on content/half-integers |
| `methods/content/half-integers-quintic` | pass | 14/14 | -quintic, -tpattern alt-z, on content/half-integers |
| `methods/content/half-integers-heptic` | pass | 14/14 | -heptic, -tpattern alt-z, on content/half-integers |
| `methods/content/half-integers-wsinc5` | pass | 14/14 | -wsinc5, -tpattern alt-z, on content/half-integers |
| `methods/content/half-integers-wsinc9` | pass | 14/14 | -wsinc9, -tpattern alt-z, on content/half-integers |
| `methods/content/steps-linear` | pass | 14/14 | -linear, -tpattern alt-z, on content/steps |
| `methods/content/steps-cubic` | pass | 14/14 | -cubic, -tpattern alt-z, on content/steps |
| `methods/content/steps-quintic` | pass | 14/14 | -quintic, -tpattern alt-z, on content/steps |
| `methods/content/steps-heptic` | pass | 14/14 | -heptic, -tpattern alt-z, on content/steps |
| `methods/content/steps-wsinc5` | pass | 14/14 | -wsinc5, -tpattern alt-z, on content/steps |
| `methods/content/steps-wsinc9` | pass | 14/14 | -wsinc9, -tpattern alt-z, on content/steps |
| `methods/dtypes/float32-nonfinite-linear` | pass | 14/14 | -linear, -tpattern alt-z, on dtypes/float32-nonfinite |
| `methods/dtypes/float32-nonfinite-cubic` | pass | 14/14 | -cubic, -tpattern alt-z, on dtypes/float32-nonfinite |
| `methods/dtypes/float32-nonfinite-quintic` | pass | 14/14 | -quintic, -tpattern alt-z, on dtypes/float32-nonfinite |
| `methods/dtypes/float32-nonfinite-heptic` | pass | 14/14 | -heptic, -tpattern alt-z, on dtypes/float32-nonfinite |
| `methods/dtypes/float32-nonfinite-wsinc5` | pass | 14/14 | -wsinc5, -tpattern alt-z, on dtypes/float32-nonfinite |
| `methods/dtypes/float32-nonfinite-wsinc9` | pass | 14/14 | -wsinc9, -tpattern alt-z, on dtypes/float32-nonfinite |
| `methods/dtypes/float32-slope-linear` | pass | 13/13 | -linear, -tpattern alt-z, on dtypes/float32-slope |
| `methods/dtypes/float32-slope-cubic` | pass | 13/13 | -cubic, -tpattern alt-z, on dtypes/float32-slope |
| `methods/dtypes/float32-slope-quintic` | pass | 13/13 | -quintic, -tpattern alt-z, on dtypes/float32-slope |
| `methods/dtypes/float32-slope-heptic` | pass | 13/13 | -heptic, -tpattern alt-z, on dtypes/float32-slope |
| `methods/dtypes/float32-slope-wsinc5` | pass | 13/13 | -wsinc5, -tpattern alt-z, on dtypes/float32-slope |
| `methods/dtypes/float32-slope-wsinc9` | pass | 13/13 | -wsinc9, -tpattern alt-z, on dtypes/float32-slope |
| `methods/dtypes/float32-linear` | pass | 14/14 | -linear, -tpattern alt-z, on dtypes/float32 |
| `methods/dtypes/float32-cubic` | pass | 14/14 | -cubic, -tpattern alt-z, on dtypes/float32 |
| `methods/dtypes/float32-quintic` | pass | 14/14 | -quintic, -tpattern alt-z, on dtypes/float32 |
| `methods/dtypes/float32-heptic` | pass | 14/14 | -heptic, -tpattern alt-z, on dtypes/float32 |
| `methods/dtypes/float32-wsinc5` | pass | 14/14 | -wsinc5, -tpattern alt-z, on dtypes/float32 |
| `methods/dtypes/float32-wsinc9` | pass | 14/14 | -wsinc9, -tpattern alt-z, on dtypes/float32 |
| `methods/dtypes/float64-linear` | pass | 13/13 | -linear, -tpattern alt-z, on dtypes/float64 |
| `methods/dtypes/float64-cubic` | pass | 13/13 | -cubic, -tpattern alt-z, on dtypes/float64 |
| `methods/dtypes/float64-quintic` | pass | 13/13 | -quintic, -tpattern alt-z, on dtypes/float64 |
| `methods/dtypes/float64-heptic` | pass | 13/13 | -heptic, -tpattern alt-z, on dtypes/float64 |
| `methods/dtypes/float64-wsinc5` | pass | 13/13 | -wsinc5, -tpattern alt-z, on dtypes/float64 |
| `methods/dtypes/float64-wsinc9` | pass | 13/13 | -wsinc9, -tpattern alt-z, on dtypes/float64 |
| `methods/dtypes/int16-negative-slope-linear` | pass | 14/14 | -linear, -tpattern alt-z, on dtypes/int16-negative-slope |
| `methods/dtypes/int16-negative-slope-cubic` | pass | 14/14 | -cubic, -tpattern alt-z, on dtypes/int16-negative-slope |
| `methods/dtypes/int16-negative-slope-quintic` | pass | 14/14 | -quintic, -tpattern alt-z, on dtypes/int16-negative-slope |
| `methods/dtypes/int16-negative-slope-heptic` | pass | 14/14 | -heptic, -tpattern alt-z, on dtypes/int16-negative-slope |
| `methods/dtypes/int16-negative-slope-wsinc5` | pass | 14/14 | -wsinc5, -tpattern alt-z, on dtypes/int16-negative-slope |
| `methods/dtypes/int16-negative-slope-wsinc9` | pass | 14/14 | -wsinc9, -tpattern alt-z, on dtypes/int16-negative-slope |
| `methods/dtypes/int16-slope-inter-linear` | pass | 13/13 | -linear, -tpattern alt-z, on dtypes/int16-slope-inter |
| `methods/dtypes/int16-slope-inter-cubic` | pass | 13/13 | -cubic, -tpattern alt-z, on dtypes/int16-slope-inter |
| `methods/dtypes/int16-slope-inter-quintic` | pass | 13/13 | -quintic, -tpattern alt-z, on dtypes/int16-slope-inter |
| `methods/dtypes/int16-slope-inter-heptic` | pass | 13/13 | -heptic, -tpattern alt-z, on dtypes/int16-slope-inter |
| `methods/dtypes/int16-slope-inter-wsinc5` | pass | 13/13 | -wsinc5, -tpattern alt-z, on dtypes/int16-slope-inter |
| `methods/dtypes/int16-slope-inter-wsinc9` | pass | 13/13 | -wsinc9, -tpattern alt-z, on dtypes/int16-slope-inter |
| `methods/dtypes/int16-slope-linear` | pass | 13/13 | -linear, -tpattern alt-z, on dtypes/int16-slope |
| `methods/dtypes/int16-slope-cubic` | pass | 13/13 | -cubic, -tpattern alt-z, on dtypes/int16-slope |
| `methods/dtypes/int16-slope-quintic` | pass | 13/13 | -quintic, -tpattern alt-z, on dtypes/int16-slope |
| `methods/dtypes/int16-slope-heptic` | pass | 13/13 | -heptic, -tpattern alt-z, on dtypes/int16-slope |
| `methods/dtypes/int16-slope-wsinc5` | pass | 13/13 | -wsinc5, -tpattern alt-z, on dtypes/int16-slope |
| `methods/dtypes/int16-slope-wsinc9` | pass | 13/13 | -wsinc9, -tpattern alt-z, on dtypes/int16-slope |
| `methods/dtypes/int16-linear` | pass | 14/14 | -linear, -tpattern alt-z, on dtypes/int16 |
| `methods/dtypes/int16-cubic` | pass | 14/14 | -cubic, -tpattern alt-z, on dtypes/int16 |
| `methods/dtypes/int16-quintic` | pass | 14/14 | -quintic, -tpattern alt-z, on dtypes/int16 |
| `methods/dtypes/int16-heptic` | pass | 14/14 | -heptic, -tpattern alt-z, on dtypes/int16 |
| `methods/dtypes/int16-wsinc5` | pass | 14/14 | -wsinc5, -tpattern alt-z, on dtypes/int16 |
| `methods/dtypes/int16-wsinc9` | pass | 14/14 | -wsinc9, -tpattern alt-z, on dtypes/int16 |
| `methods/dtypes/int32-linear` | pass | 13/13 | -linear, -tpattern alt-z, on dtypes/int32 |
| `methods/dtypes/int32-cubic` | pass | 13/13 | -cubic, -tpattern alt-z, on dtypes/int32 |
| `methods/dtypes/int32-quintic` | pass | 13/13 | -quintic, -tpattern alt-z, on dtypes/int32 |
| `methods/dtypes/int32-heptic` | pass | 13/13 | -heptic, -tpattern alt-z, on dtypes/int32 |
| `methods/dtypes/int32-wsinc5` | pass | 13/13 | -wsinc5, -tpattern alt-z, on dtypes/int32 |
| `methods/dtypes/int32-wsinc9` | pass | 13/13 | -wsinc9, -tpattern alt-z, on dtypes/int32 |
| `methods/dtypes/uint8-linear` | pass | 14/14 | -linear, -tpattern alt-z, on dtypes/uint8 |
| `methods/dtypes/uint8-cubic` | pass | 14/14 | -cubic, -tpattern alt-z, on dtypes/uint8 |
| `methods/dtypes/uint8-quintic` | pass | 14/14 | -quintic, -tpattern alt-z, on dtypes/uint8 |
| `methods/dtypes/uint8-heptic` | pass | 14/14 | -heptic, -tpattern alt-z, on dtypes/uint8 |
| `methods/dtypes/uint8-wsinc5` | pass | 14/14 | -wsinc5, -tpattern alt-z, on dtypes/uint8 |
| `methods/dtypes/uint8-wsinc9` | pass | 14/14 | -wsinc9, -tpattern alt-z, on dtypes/uint8 |
| `options/float32-ignore` | pass | 14/14 | -ignore 3: three leading points kept out of fit and shift (float32) |
| `options/float32-tzero-0` | pass | 14/14 | -tzero 0: every slice moved to the start of the TR (float32) |
| `options/float32-tzero-0.5` | pass | 14/14 | -tzero 0.5 (float32) |
| `options/float32-tzero-beyond-tr` | pass | 14/14 | -tzero 5: beyond the TR (allowed) (float32) |
| `options/float32-slice-2` | pass | 14/14 | -slice 2: align to slice 2's time (float32) |
| `options/float32-slice-wins` | pass | 14/14 | -tzero and -slice: -slice wins (float32) |
| `options/float32-rlt` | pass | 14/14 | -rlt: no trend added back (float32) |
| `options/float32-rlt+` | pass | 14/14 | -rlt+: only the intercept added back (float32) |
| `options/float32-rlt-last-wins` | pass | 14/14 | -rlt then -rlt+: the last wins (float32) |
| `options/float32-no-detrend` | pass | 15/15 | -heptic -no_detrend: mean removed only (float32) |
| `options/float32-no-detrend-fourier` | pass | 15/15 | -no_detrend then -Fourier (allowed, with a warning) (float32) |
| `options/float32-TR-seconds` | pass | 14/14 | -TR 2.5s (float32) |
| `options/float32-TR-plain` | pass | 14/14 | -TR 1.7 (no unit) (float32) |
| `options/float32-TR-ms` | pass | 12/12 | -TR 2000ms: every time in milliseconds (float32) |
| `options/float32-TR-msec` | pass | 12/12 | -TR 2000msec -tzero 500 (float32) |
| `options/float32-verbose` | pass | 14/14 | -verbose (float32) |
| `options/float32-abbreviations` | pass | 14/14 | -cub and -verb: AFNI's abbreviations (float32) |
| `options/float32-wsinc-case` | pass | 14/14 | -WSINC5: case-insensitive (float32) |
| `options/float32-ignore-heptic` | pass | 14/14 | -ignore 5 with -heptic (float32) |
| `options/float32-rlt-wsinc9` | pass | 14/14 | -rlt with -wsinc9 (float32) |
| `options/float32-rlt+-linear` | pass | 14/14 | -rlt+ with -linear (float32) |
| `options/int16-ignore` | pass | 14/14 | -ignore 3: three leading points kept out of fit and shift (int16) |
| `options/int16-tzero-0` | pass | 14/14 | -tzero 0: every slice moved to the start of the TR (int16) |
| `options/int16-tzero-0.5` | pass | 14/14 | -tzero 0.5 (int16) |
| `options/int16-tzero-beyond-tr` | pass | 14/14 | -tzero 5: beyond the TR (allowed) (int16) |
| `options/int16-slice-2` | pass | 14/14 | -slice 2: align to slice 2's time (int16) |
| `options/int16-slice-wins` | pass | 14/14 | -tzero and -slice: -slice wins (int16) |
| `options/int16-rlt` | pass | 14/14 | -rlt: no trend added back (int16) |
| `options/int16-rlt+` | pass | 14/14 | -rlt+: only the intercept added back (int16) |
| `options/int16-rlt-last-wins` | pass | 14/14 | -rlt then -rlt+: the last wins (int16) |
| `options/int16-no-detrend` | pass | 15/15 | -heptic -no_detrend: mean removed only (int16) |
| `options/int16-no-detrend-fourier` | pass | 15/15 | -no_detrend then -Fourier (allowed, with a warning) (int16) |
| `options/int16-TR-seconds` | pass | 14/14 | -TR 2.5s (int16) |
| `options/int16-TR-plain` | pass | 14/14 | -TR 1.7 (no unit) (int16) |
| `options/int16-TR-ms` | pass | 12/12 | -TR 2000ms: every time in milliseconds (int16) |
| `options/int16-TR-msec` | pass | 12/12 | -TR 2000msec -tzero 500 (int16) |
| `options/int16-verbose` | pass | 14/14 | -verbose (int16) |
| `options/int16-abbreviations` | pass | 14/14 | -cub and -verb: AFNI's abbreviations (int16) |
| `options/int16-wsinc-case` | pass | 14/14 | -WSINC5: case-insensitive (int16) |
| `options/int16-ignore-heptic` | pass | 14/14 | -ignore 5 with -heptic (int16) |
| `options/int16-rlt-wsinc9` | pass | 14/14 | -rlt with -wsinc9 (int16) |
| `options/int16-rlt+-linear` | pass | 14/14 | -rlt+ with -linear (int16) |
| `tpattern/alt+z` | pass | 14/14 | -tpattern alt+z |
| `tpattern/altplus` | pass | 14/14 | -tpattern altplus |
| `tpattern/alt+z2` | pass | 14/14 | -tpattern alt+z2 |
| `tpattern/alt-z` | pass | 14/14 | -tpattern alt-z |
| `tpattern/altminus` | pass | 14/14 | -tpattern altminus |
| `tpattern/alt-z2` | pass | 14/14 | -tpattern alt-z2 |
| `tpattern/seq+z` | pass | 14/14 | -tpattern seq+z |
| `tpattern/seqplus` | pass | 14/14 | -tpattern seqplus |
| `tpattern/seq-z` | pass | 14/14 | -tpattern seq-z |
| `tpattern/seqminus` | pass | 14/14 | -tpattern seqminus |
| `tpattern/zero` | pass | 14/14 | -tpattern zero |
| `tpattern/simult` | pass | 14/14 | -tpattern simult |
| `tpattern/file-tabs` | pass | 12/12 | -tpattern @file: one line, tab-separated (as fMRIPrep writes it) |
| `tpattern/file-lines` | pass | 12/12 | -tpattern @file: one value per line |
| `tpattern/file-matrix` | pass | 12/12 | -tpattern @file: two columns: read column by column |
| `tpattern/file-comments` | pass | 12/12 | -tpattern @file: comments and commas |
| `tpattern/file-extra` | pass | 12/12 | -tpattern @file: more values than slices |
| `tpattern/file-repeat` | pass | 12/12 | -tpattern @file: N@value repetition |
| `tpattern/file-tr` | pass | 12/12 | -tpattern @file: a value equal to the TR |
| `tpattern/inline-1D` | pass | 12/12 | -tpattern '@1D: 0 1 0.5 1.5' |
| `header-timing/code1-seq-inc` | pass | 12/12 | no -tpattern, slice_code 1 (sequential increasing) |
| `header-timing/code2-seq-dec` | pass | 13/13 | no -tpattern, slice_code 2: AFNI gives every slice time 0 |
| `header-timing/code3-alt-inc` | pass | 12/12 | no -tpattern, slice_code 3 (alternating increasing) |
| `header-timing/code4-alt-dec` | pass | 12/12 | no -tpattern, slice_code 4 (alternating decreasing) |
| `header-timing/code5-alt-inc2` | pass | 12/12 | no -tpattern, slice_code 5 (alternating increasing, from slice 1) |
| `header-timing/code6-alt-dec2` | pass | 12/12 | no -tpattern, slice_code 6 (alternating decreasing, from nz-2) |
| `header-timing/code3-partial` | pass | 12/12 | no -tpattern, slice_code 3 over slices 1..2 only |
| `header-timing/code3-ms` | pass | 12/12 | no -tpattern, slice_code 3 with the TR and duration in milliseconds |
| `header-timing/code3-beyond-tr` | pass | 12/12 | no -tpattern, times beyond the TR: a copy of the input, timing kept |
| `header-timing/code2-beyond-tr` | pass | 13/13 | no -tpattern, slice_code 2: AFNI gives every slice time 0 and shifts nothing; corrected, the times exceed the TR and th… |
| `header-timing/code3-no-slice-dim` | pass | 12/12 | no -tpattern, no slice axis in dim_info: no timing, a copy |
| `header-timing/code3-zero-duration` | pass | 12/12 | no -tpattern, slice_duration 0: no timing, a copy |
| `header-timing/tpattern-overrides` | pass | 14/14 | -tpattern overrides the header's timing |
| `defects/f32-slope-inter` | pass | 14/14 | float32 with scl_slope 2 and scl_inter 10, shifted (AFNI-02) |
| `defects/f32-slope-inter-copy` | pass | 13/13 | float32 with scl_slope 2 and scl_inter 10, a copy (AFNI-02) |
| `defects/metre` | pass | 15/15 | units in metres with an origin (AFNI-07) |
| `defects/micron-copy` | pass | 13/13 | units in micrometres with an origin, a copy (AFNI-07) |
| `defects/negative-slope-zero` | pass | 14/14 | int16 with scl_slope -0.5 and -tpattern zero: nothing is shifted, so AFNI-01 changes nothing |
| `defects/oblique-metre` | pass | 15/15 | units in metres, an oblique sform with an origin (AFNI-07) |
| `defects/tiny-offsets-copy` | pass | 13/13 | header slice times 0.5 ms apart beyond a 1 ms TR: a copy of the input that keeps them, all within 3 ms of 0 (AFNI-08) |
| `geometry/slightly-oblique` | pass | 15/15 | an sform 0.006° from plumb (AFNI writes the plumb matrix below about 0.014°, AFNI-16) |
| `geometry/slightly-oblique-qform` | pass | 15/15 | a qform only, 0.006° from plumb: AFNI writes the plumb matrix with the header's voxel sizes as steps (AFNI-16) |
| `geometry/slightly-oblique-metre` | pass | 15/15 | an sform in metres 0.006° from plumb: AFNI writes the plumb matrix, its steps in millimetres and its origin in metres (… |
| `defects/oblique-metre-zero-origin` | pass | 15/15 | units in metres, an oblique sform at the origin: AFNI keeps the whole sform in metres (AFNI-07) |
| `copy/short-slice-spacing` | pass | 12/12 | header slice times 2 ms apart beyond a 5 ms TR: a copy, whose slice fields AFNI derives from the times within 3 ms (sli… |
| `copy/alt-two-slices` | pass | 12/12 | header slice code 3 over slices 1..2, beyond the TR: a copy, written with slice code 1 |
| `copy/no-timing-int16` | pass | 12/12 | no slice timing anywhere: the output is a copy of the input (int16) |
| `copy/no-timing-int16-slope` | pass | 12/12 | no slice timing anywhere: the output is a copy of the input (int16 with a brick factor) |
| `copy/no-timing-int16-slope-inter` | pass | 12/12 | no slice timing anywhere: the output is a copy of the input (int16 scaled to float32) |
| `copy/no-timing-float64` | pass | 12/12 | no slice timing anywhere: the output is a copy of the input (float64 converted to float32) |
| `copy/no-timing-float32-nonfinite` | pass | 12/12 | no slice timing anywhere: the output is a copy of the input (non-finite floats zeroed) |
| `copy/3d-input` | pass | 12/12 | a 3D image: one value per voxel, written back as 3D |
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
| `errors/existing-output` | both-error | – | an output that exists already (AFNI exits 0 without writing; larmorx exits 1, AFNI-11) |
| `errors/prefix-with-blank` | both-error | – | a -prefix with a blank (AFNI refuses it; larmorx writes it, AFNI-12) |
| `global/overwrite-after-dataset` | pass | 15/15 | -overwrite after the dataset replaces an existing output |
| `global/overwrite-as-option-value` | pass | 15/15 | -overwrite where -tpattern's value stands: taken out first, -tpattern reads alt+z |
| `global/overwrite-copy` | pass | 15/15 | -overwrite on a copy of the input (no slice timing) |
| `global/overwrite-no-existing-file` | pass | 15/15 | -overwrite with no output to replace |
| `global/overwrite-then-D` | pass | 15/15 | -overwrite, then -DAFNI_DECONFLICT=YES: the later one wins (out_AA1.nii) |
| `global/D-deconflict-yes` | pass | 15/15 | -DAFNI_DECONFLICT=YES: an existing output makes the run write out_AA1.nii |
| `global/D-deconflict-no` | both-error | 2/2 | -DAFNI_DECONFLICT=NO: an existing output stops the shift before the work (exit 1) |
| `global/D-wins-over-environment` | pass | 15/15 | -DAFNI_DECONFLICT=OVERWRITE over AFNI_DECONFLICT=YES in the environment |
| `global/D-arbitrary-filenames` | pass | 15/15 | -DAFNI_ALLOW_ARBITRARY_FILENAMES=YES: a -prefix with a blank is written |
| `global/D-after-dataset` | pass | 15/15 | -D after the dataset applies too |
| `global/D-as-option-value` | pass | 15/15 | -D where -tpattern's value stands: taken out first |
| `global/D-value-with-blank` | pass | 15/15 | -DAFNI_ALLOW_ARBITRARY_FILENAMES=Y ES: AFNI sets 'Y=ES', still YES |
| `global/D-blank-before-value` | both-error | 2/2 | -DAFNI_ALLOW_ARBITRARY_FILENAMES= YES: AFNI sets '=YES', not YES: refused |
| `global/D-empty-value` | both-error | 2/2 | -DAFNI_ALLOW_ARBITRARY_FILENAMES= (no value) sets nothing: refused |
| `global/D-empty-name` | both-error | 2/2 | -D=YES (no name) sets nothing |
| `global/D-without-equals` | both-error | 2/2 | -DAFNI_ALLOW_ARBITRARY_FILENAMES (no '='): an unknown option |
| `global/D-alone` | both-error | 2/2 | -D alone: an unknown option |
| `global/D-lowercase` | both-error | 2/2 | -d...=YES: not a global option, an unknown option |
| `global/D-message-prefix-with-blank` | pass | 15/15 | -DAFNI_MESSAGE_PREFIX=a b: AFNI's messages start with 'a=b::' (AFNI-22) |
| `global/D-unknown-variable` | pass | 15/15 | -D of a variable 3dTshift does not read: no effect |
| `global/ok-1D-text` | pass | 15/15 | -ok_1D_text: text in the -tpattern file is read as 0 |
| `global/skip-afnirc` | pass | 15/15 | -skip_afnirc: accepted |
| `environment/deconflict-overwrite` | pass | 15/15 | AFNI_DECONFLICT=OVERWRITE: an existing output is replaced, with AFNI's warning |
| `environment/deconflict-yes` | pass | 15/15 | AFNI_DECONFLICT=YES: the output goes to out_AA1.nii |
| `environment/deconflict-no-copy` | both-error | 2/2 | AFNI_DECONFLICT=NO on a copy: AFNI finds the output after the work, exit 0 (AFNI-11) |
| `environment/arbitrary-filenames` | pass | 15/15 | AFNI_ALLOW_ARBITRARY_FILENAMES=YES: a -prefix with a blank is written |
| `environment/input-with-blank` | both-error | 2/2 | an input in a directory with a blank: AFNI reads a list of datasets and fails (AFNI-18) |
| `environment/path-spaces-ok` | pass | 15/15 | AFNI_PATH_SPACES_OK=YES: an input in a directory with a blank is one file |
| `environment/nifti-priority-Q` | pass | 15/15 | AFNI_NIFTI_PRIORITY=Q: the qform rather than the sform (they differ) |
| `environment/nifti-priority-illegal` | pass | 15/15 | AFNI_NIFTI_PRIORITY=X: AFNI's warning, the sform |
| `environment/nifti-scale-no` | pass | 15/15 | AFNI_NIFTI_SCALE=NO: int16 with a slope and an intercept stays int16 (slope kept) |
| `environment/type-warn-default` | pass | 15/15 | AFNI_NIFTI_TYPE_WARN unset: AFNI's conversion message, with the muting lines |
| `environment/type-warn-yes` | pass | 15/15 | AFNI_NIFTI_TYPE_WARN=YES: the conversion message alone |
| `environment/nifti-view-tlrc` | pass | 15/15 | AFNI_NIFTI_VIEW=TLRC: sform code 2 is the tlrc view, so code 3 |
| `environment/default-std-space` | pass | 15/15 | AFNI_DEFAULT_STD_SPACE=MNI: sform code 5 gives code 4 |
| `environment/sform-code-7` | pass | 15/15 | sform code 7: AFNI's warning; AFNI_NIFTI_VIEW=tlrc makes it code 3 |
| `environment/write-type-2` | pass | 15/15 | AFNI_NIFTI_WRITE_TYPE=2: a NIfTI-2 output |
| `environment/allow-milliseconds` | pass | 15/15 | AFNI_ALLOW_MILLISECONDS=YES with -TR 2000ms: toffset written in ms (AFNI-19) |
| `environment/1D-zero-text` | pass | 15/15 | AFNI_1D_ZERO_TEXT=YES: text in the -tpattern file is read as 0 |
| `environment/1D-text-error` | both-error | 2/2 | text in the -tpattern file: AFNI's parse error |
| `environment/use-fgets` | both-error | 2/2 | AFNI_USE_FGETS=YES: a file with CR line ends is one row, of 8 values (9 > TR) |
| `environment/cr-line-ends` | pass | 15/15 | a file with CR line ends: four rows of two columns, read column by column |
| `environment/realpath` | pass | 15/15 | a conversion message names the input by its real path ('..' resolved) |
| `environment/norealpath` | pass | 15/15 | AFNI_NOREALPATH=YES: the message names the input as given |
| `environment/oblique-warning` | pass | 15/15 | an oblique input: AFNI's obliquity report |
| `environment/no-oblique-warning` | pass | 15/15 | AFNI_NO_OBLIQUE_WARNING=YES: no obliquity report |
| `environment/message-prefix` | pass | 15/15 | AFNI_MESSAGE_PREFIX: on every labelled message |
| `environment/message-colorize-no` | pass | 15/15 | AFNI_MESSAGE_COLORIZE=NO: labels without terminal escape codes |
| `environment/echo-commandline` | pass | 15/15 | AFNI_ECHO_COMMANDLINE=YES (and a log file): the command line is echoed |
| `environment/quiet-startup` | pass | 15/15 | AFNI_QUIET_STARTUP=YES: no version banner (larmorx prints none) |
| `environment/nifti-noext` | pass | 16/16 | AFNI_NIFTI_NOEXT=YES: AFNI writes no header extension, as larmorx: the whole file |
| `environment/one-row-dataset` | expected-divergence | 3/4 | a dataset of one row: AFNI writes a 1D text file under the NIfTI name (AFNI-20) |
| `environment/write-1D-as-prefix` | pass | 15/15 | AFNI_WRITE_1D_AS_PREFIX=YES: a dataset of one row is written as NIfTI |
| `global/D-nifti-priority-Q` | pass | 15/15 | -DAFNI_NIFTI_PRIORITY=Q: as the environment variable |
| `global/D-nifti-view-tlrc` | pass | 15/15 | -DAFNI_NIFTI_VIEW=TLRC: as the environment variable |
| `global/D-write-type-2` | pass | 15/15 | -DAFNI_NIFTI_WRITE_TYPE=2: as the environment variable |
| `global/D-nifti-scale-no` | pass | 15/15 | -DAFNI_NIFTI_SCALE=NO: as the environment variable |
| `divergence/afni-format-output` | expected-divergence | 1/1 | -prefix without .nii: AFNI writes its own BRIK/HEAD format |
| `divergence/voxshift` | expected-divergence | 1/1 | -voxshift: per-voxel shifts from a dataset |
| `real/ds001600-acq-v4` | pass | 14/14 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-1_task-rest_acq-v4_bold |
| `real/ds001600-acq-v1` | pass | 14/14 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-1_task-rest_acq-v1_bold |
| `real/ds001600-acq-v2` | pass | 14/14 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-1_task-rest_acq-v2_bold |
| `real/ds001600-acq-PA` | pass | 14/14 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-1_task-rest_acq-PA_bold |
| `real/ds000210-echo-1` | pass | 14/14 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-02_task-cuedSGT_run-01_echo-1_bold |
| `real/ds000210-echo-2` | pass | 14/14 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-02_task-cuedSGT_run-01_echo-2_bold |
| `real/ds000210-echo-3` | pass | 14/14 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-02_task-cuedSGT_run-01_echo-3_bold |
| `real/ds006736` | pass | 14/14 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-004_task-freeRecall_bold |
| `real/ds006010-uint16` | pass | 13/13 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-206_task-category_run-01_bold |
| `real/ds003345-ms-header` | pass | 14/14 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-22973_task-PenaltyKik_run-02_bold |
| `real/ds005454-mb4-96-slices` | pass | 14/14 | fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on sub-16_task-rest_bold |
| `real/ds003345-linear` | pass | 14/14 | fMRIPrep's call with -linear |
| `real/ds003345-cubic` | pass | 14/14 | fMRIPrep's call with -cubic |
| `real/ds003345-quintic` | pass | 14/14 | fMRIPrep's call with -quintic |
| `real/ds003345-heptic` | pass | 14/14 | fMRIPrep's call with -heptic |
| `real/ds003345-wsinc5` | pass | 14/14 | fMRIPrep's call with -wsinc5 |
| `real/ds003345-wsinc9` | pass | 14/14 | fMRIPrep's call with -wsinc9 |
| `real/ds003345-ignore-4` | pass | 14/14 | fMRIPrep's call with -ignore 4 (non-steady-state volumes) |
| `real/ds000210-header-timing` | pass | 12/12 | no -tpattern: the slice timing in the header (slice_code 3, ALT_INC) |
| `real/ds001600-header-zero-duration` | pass | 12/12 | no -tpattern: slice_code 5 with slice_duration 0, so a copy of the input |
