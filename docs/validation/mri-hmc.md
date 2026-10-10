# Parity: Head-motion correction (mcflirt-compatible)

**1 case(s) fail.** 134 cases on the `standard` tier: 121 pass, 11 rejected by both, 1 expected divergences, 1 failures.

- **Validated:** `larmorx mri hmc` / `lx.mri.hmc` (crate larmorx-mri, clean-room from `specs/mcflirt.md`, Apache-2.0)
- **Reference:** mcflirt from FSL 6.0.7.17 (fsl-mcflirt 2111.0), the recorded runs in `oracles/fsl-6.0.7/mcflirt/`
- **Test data:** larmorx-testdata `28167dea9be5`, tier `standard`
- **Generated:** 2026-10-10 on Linux x86_64, with `python -m larmorx_validation parity mri-hmc --tier standard`

**fMRIPrep's command on real BOLD runs** (`-reffile <HMC reference> -mats`): larmorx's matrices against mcflirt's, next to mcflirt's own deviation when its input is perturbed slightly (the band, worst of noise and reverse order):

| Run | Volumes | RMS dev. median (mm) | 95th pct (mm) | mcflirt band 95th pct (mm) | Median Δparam (mm / °) | FD r | mcflirt band FD r |
|---|---|---|---|---|---|---|---|
| ds000005 | 240 | 0.0094 | **0.0262** | 0.1014 | 0.0017 / 0.0005 | 0.8869 | 0.7941 |
| ds000117 | 208 | 0.0111 | **0.0191** | 0.0203 | 0.0023 / 0.0031 | 0.8991 | 0.8720 |
| ds000122 | 258 | 0.0159 | **0.0299** | 0.0312 | 0.0023 / 0.0048 | 0.9985 | 0.9977 |
| ds000210-rest-echo2 | 204 | 0.0130 | **0.0226** | 0.0229 | 0.0024 / 0.0039 | 0.9820 | 0.9799 |
| ds000258-bigendian | 239 | 0.0103 | **0.0181** | 0.0193 | 0.0014 / 0.0021 | 0.9949 | 0.9949 |
| ds003345 | 216 | 0.0141 | **0.0259** | 0.0299 | 0.0023 / 0.0040 | 0.9867 | 0.9840 |
| ds005040 | 63 | 0.0135 | **0.0252** | 0.0254 | 0.0028 / 0.0032 | 0.8539 | 0.8238 |
| ds006010 | 113 | 0.0073 | **0.0142** | 0.0161 | 0.0010 / 0.0010 | 0.9263 | 0.8857 |
| ds006736 | 69 | 0.0121 | **0.0252** | 0.0259 | 0.0021 / 0.0038 | 0.9987 | 0.9974 |

**All runs with `-mats`** (116): the worst 95th-percentile RMS deviation is 0.2811 mm and the worst median parameter difference 0.0103 mm / 0.0278° (thresholds per run: 0.1 mm and 0.05 mm / 0.05°, or mcflirt's band where wider; 24 runs use the band). 49 of 11271 matrix files are identical to mcflirt's as text (6 decimals).

| Category | Runs | RMS dev. median (mm) | Worst 95th pct (mm) | Worst median Δparam (mm / °) | Max Δparam (mm / °) | Lowest FD r | Identical .mat |
|---|---|---|---|---|---|---|---|
| options (ds000005) | 23 | 0.0094 | 0.0662 | 0.0103 / 0.0083 | 0.063 / 0.067 | 0.9991 | 7/5520 |
| real | 28 | 0.0113 | 0.0299 | 0.0029 / 0.0053 | 0.047 / 0.046 | 0.9820 | 8/5094 |
| synthetic/cli | 4 | 0.0213 | 0.0713 | 0.0027 / 0.0164 | 0.016 / 0.080 | 0.9971 | 3/37 |
| synthetic/content | 4 | 0.0293 | 0.0870 | 0.0036 / 0.0126 | 0.027 / 0.094 | 0.9988 | 5/36 |
| synthetic/dtypes | 10 | 0.0265 | 0.0961 | 0.0032 / 0.0164 | 0.017 / 0.099 | 0.9971 | 5/120 |
| synthetic/geometry | 10 | 0.0122 | 0.1130 | 0.0029 / 0.0278 | 0.026 / 0.114 | 0.9995 | 7/38 |
| synthetic/motion | 31 | 0.0365 | 0.2811 | 0.0099 / 0.0267 | 0.071 / 0.624 | 0.9737 | 11/354 |
| synthetic/orientation | 6 | 0.0293 | 0.0713 | 0.0027 / 0.0164 | 0.019 / 0.080 | 0.9971 | 3/72 |

**Against the synthetic ground truth** (mean RMS deviation from the true motion, 27 runs): larmorx 0.4090 mm, mcflirt 0.4143 mm.

## mcflirt's own variability band

mcflirt is deterministic, so its variability is measured by perturbing its input slightly and comparing its matrices with its own unperturbed ones (`python -m larmorx_validation.parity.mri_hmc_band`): uniform noise of ±0.5 intensity units, the volumes in reverse order (runs with a separate reference), the top slice cropped. Noise and reverse order set the widened thresholds; cropping changes the problem more (it breaks thin images) and is only reported.

| Run | Perturbation | RMS dev. median (mm) | 95th pct (mm) | Median Δparam (mm / °) | FD r |
|---|---|---|---|---|---|
| `real/ds000005/default` | crop | 0.0119 | 0.0223 | 0.0028 / 0.0028 | 0.8700 |
| `real/ds000005/default` | noise | 0.0113 | 0.0210 | 0.0020 / 0.0027 | 0.8749 |
| `real/ds000005/fmriprep` | crop | 0.0114 | 0.0811 | 0.0025 / 0.0014 | 0.8979 |
| `real/ds000005/fmriprep` | noise | 0.0097 | 0.0265 | 0.0017 / 0.0020 | 0.8941 |
| `real/ds000005/fmriprep` | reverse | 0.0221 | 0.1014 | 0.0051 / 0.0068 | 0.7941 |
| `real/ds000005/opt-meanvol` | crop | 0.0172 | 0.0328 | 0.0036 / 0.0051 | 0.7985 |
| `real/ds000005/opt-meanvol` | noise | 0.0123 | 0.0233 | 0.0021 / 0.0036 | 0.8544 |
| `real/ds000005/opt-meanvol` | reverse | 0.0253 | 0.0537 | 0.0051 / 0.0067 | 0.6737 |
| `real/ds000117/default` | crop | 0.0143 | 0.0252 | 0.0033 / 0.0042 | 0.8357 |
| `real/ds000117/default` | noise | 0.0131 | 0.0231 | 0.0026 / 0.0038 | 0.8592 |
| `real/ds000117/fmriprep` | crop | 0.0128 | 0.0207 | 0.0035 / 0.0038 | 0.9046 |
| `real/ds000117/fmriprep` | noise | 0.0118 | 0.0194 | 0.0026 / 0.0034 | 0.9024 |
| `real/ds000117/fmriprep` | reverse | 0.0138 | 0.0203 | 0.0036 / 0.0041 | 0.8720 |
| `real/ds000122/default` | crop | 0.0172 | 0.0321 | 0.0029 / 0.0055 | 0.9983 |
| `real/ds000122/default` | noise | 0.0156 | 0.0284 | 0.0023 / 0.0050 | 0.9990 |
| `real/ds000122/fmriprep` | crop | 0.0193 | 0.0343 | 0.0043 / 0.0062 | 0.9982 |
| `real/ds000122/fmriprep` | noise | 0.0168 | 0.0278 | 0.0025 / 0.0051 | 0.9983 |
| `real/ds000122/fmriprep` | reverse | 0.0187 | 0.0312 | 0.0042 / 0.0073 | 0.9977 |
| `real/ds000210-rest-echo2/default` | crop | 0.0125 | 0.0224 | 0.0030 / 0.0037 | 0.9413 |
| `real/ds000210-rest-echo2/default` | noise | 0.0105 | 0.0216 | 0.0022 / 0.0031 | 0.9935 |
| `real/ds000210-rest-echo2/fmriprep` | crop | 0.0128 | 0.0238 | 0.0025 / 0.0047 | 0.9854 |
| `real/ds000210-rest-echo2/fmriprep` | noise | 0.0122 | 0.0226 | 0.0022 / 0.0037 | 0.9820 |
| `real/ds000210-rest-echo2/fmriprep` | reverse | 0.0151 | 0.0229 | 0.0036 / 0.0051 | 0.9799 |
| `real/ds000258-bigendian/default` | crop | 0.0129 | 0.0210 | 0.0030 / 0.0042 | 0.9938 |
| `real/ds000258-bigendian/default` | noise | 0.0095 | 0.0181 | 0.0012 / 0.0026 | 0.9956 |
| `real/ds000258-bigendian/fmriprep` | crop | 0.0147 | 0.0271 | 0.0034 / 0.0052 | 0.9932 |
| `real/ds000258-bigendian/fmriprep` | noise | 0.0099 | 0.0188 | 0.0015 / 0.0029 | 0.9953 |
| `real/ds000258-bigendian/fmriprep` | reverse | 0.0118 | 0.0193 | 0.0025 / 0.0040 | 0.9949 |
| `real/ds003345/default` | crop | 0.0145 | 0.0293 | 0.0030 / 0.0044 | 0.9888 |
| `real/ds003345/default` | noise | 0.0138 | 0.0249 | 0.0022 / 0.0041 | 0.9893 |
| `real/ds003345/fmriprep` | crop | 0.0152 | 0.0278 | 0.0031 / 0.0046 | 0.9860 |
| `real/ds003345/fmriprep` | noise | 0.0144 | 0.0254 | 0.0024 / 0.0037 | 0.9872 |
| `real/ds003345/fmriprep` | reverse | 0.0174 | 0.0299 | 0.0039 / 0.0058 | 0.9840 |
| `real/ds005040/default` | crop | 0.0140 | 0.0232 | 0.0029 / 0.0043 | 0.8423 |
| `real/ds005040/default` | noise | 0.0150 | 0.0259 | 0.0031 / 0.0044 | 0.8091 |
| `real/ds005040/fmriprep` | crop | 0.0124 | 0.0227 | 0.0020 / 0.0028 | 0.8663 |
| `real/ds005040/fmriprep` | noise | 0.0124 | 0.0213 | 0.0022 / 0.0038 | 0.8529 |
| `real/ds005040/fmriprep` | reverse | 0.0160 | 0.0254 | 0.0038 / 0.0047 | 0.8238 |
| `real/ds006010/default` | crop | 0.0059 | 0.0147 | 0.0009 / 0.0002 | 0.9208 |
| `real/ds006010/default` | noise | 0.0063 | 0.0135 | 0.0008 / 0.0002 | 0.9191 |
| `real/ds006010/fmriprep` | crop | 0.0073 | 0.0131 | 0.0008 / 0.0007 | 0.9320 |
| `real/ds006010/fmriprep` | noise | 0.0086 | 0.0161 | 0.0019 / 0.0021 | 0.8857 |
| `real/ds006010/fmriprep` | reverse | 0.0087 | 0.0151 | 0.0019 / 0.0030 | 0.9004 |
| `real/ds006736/default` | crop | 0.0106 | 0.0210 | 0.0020 / 0.0032 | 0.9989 |
| `real/ds006736/default` | noise | 0.0097 | 0.0233 | 0.0018 / 0.0018 | 0.9989 |
| `real/ds006736/fmriprep` | crop | 0.0128 | 0.0225 | 0.0020 / 0.0030 | 0.9991 |
| `real/ds006736/fmriprep` | noise | 0.0117 | 0.0231 | 0.0019 / 0.0036 | 0.9990 |
| `real/ds006736/fmriprep` | reverse | 0.0154 | 0.0259 | 0.0035 / 0.0059 | 0.9974 |
| `syn/content/background/default` | crop | 38.4359 | 98.9725 | 0.0421 / 0.2072 | -0.1239 |
| `syn/content/background/default` | noise | 0.0520 | 0.0879 | 0.0057 / 0.0142 | 1.0000 |
| `syn/geometry/tiny/default` | crop | 3.4466 | 7.5313 | 0.1260 / 2.0623 | 0.2370 |
| `syn/geometry/tiny/default` | noise | 0.0926 | 0.2606 | 0.0057 / 0.0397 | 0.9963 |
| `syn/motion/identical-copies/reffile` | crop | 0.0321 | 0.0579 | 0.0033 / 0.0167 | 0.6482 |
| `syn/motion/identical-copies/reffile` | noise | 0.0383 | 0.0549 | 0.0040 / 0.0163 | 0.2677 |
| `syn/motion/identical-copies/reffile` | reverse | 0.0299 | 0.0597 | 0.0023 / 0.0104 | 0.8523 |
| `syn/motion/large/reffile` | crop | 0.0285 | 0.0873 | 0.0035 / 0.0110 | 0.9992 |
| `syn/motion/large/reffile` | noise | 0.0283 | 0.0434 | 0.0022 / 0.0157 | 0.9996 |
| `syn/motion/large/reffile` | reverse | 0.0258 | 0.0887 | 0.0040 / 0.0078 | 0.9991 |
| `syn/motion/small/default` | crop | 0.0294 | 0.0550 | 0.0037 / 0.0133 | 0.9983 |
| `syn/motion/small/default` | noise | 0.0232 | 0.0404 | 0.0013 / 0.0094 | 0.9994 |
| `syn/motion/small/dof12` | crop | 0.0699 | 0.1686 | 0.0087 / 0.0368 | 0.9946 |
| `syn/motion/small/dof12` | noise | 0.0818 | 0.2561 | 0.0070 / 0.0412 | 0.9932 |
| `syn/motion/small/dof12` | reverse | 0.4105 | 1.1565 | 0.0278 / 0.0734 | 0.8346 |
| `syn/motion/small/dof7` | crop | 0.0345 | 0.0663 | 0.0033 / 0.0151 | 0.9982 |
| `syn/motion/small/dof7` | noise | 0.0303 | 0.0715 | 0.0033 / 0.0160 | 0.9973 |
| `syn/motion/small/dof7` | reverse | 0.0426 | 0.0599 | 0.0036 / 0.0200 | 0.9968 |
| `syn/motion/small/meanvol` | crop | 0.0280 | 0.6967 | 0.0044 / 0.0106 | 0.8475 |
| `syn/motion/small/meanvol` | noise | 0.0249 | 0.4161 | 0.0036 / 0.0107 | 0.9015 |
| `syn/motion/small/reffile` | crop | 0.0317 | 0.0606 | 0.0023 / 0.0143 | 0.9984 |
| `syn/motion/small/reffile` | noise | 0.0291 | 0.0749 | 0.0033 / 0.0148 | 0.9944 |
| `syn/motion/small/reffile` | reverse | 0.0289 | 0.1068 | 0.0041 / 0.0125 | 0.9942 |

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status and files written | the same (errors: both reject) |
| Matrix RMS deviation (radius 80 mm), 95th percentile per run | ≤ 0.1 mm |
| Median parameter difference per run | ≤ 0.05 mm and ≤ 0.05° |
| FD (Power, 50 mm) correlation | ≥ 0.99 where the largest FD is ≥ 0.5 mm (band: mcflirt's own, if lower) |
| Thresholds marked *band* | widened to mcflirt's own deviation under small perturbations of that run's input (the variability band), where that is larger |
| Ramp series (only x-translation identifiable) | x translation within 0.3 mm; the rest reported |
| `.par` median differences; `.rms` values | ≤ 0.05 mm / 0.05°; ≤ 0.1 mm |
| Images: header (type, shape, voxel sizes, units, xforms); values | identical; NCC ≥ 0.999 |
| Synthetic truth: mean RMS deviation | larmorx ≤ mcflirt + 0.05 mm |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| options (ds000005) | 25 | 24 | 1 | 0 | 0 |
| real | 30 | 28 | 1 | 0 | 1 |
| synthetic/cli | 11 | 6 | 5 | 0 | 0 |
| synthetic/content | 6 | 6 | 0 | 0 | 0 |
| synthetic/dtypes | 10 | 10 | 0 | 0 | 0 |
| synthetic/geometry | 10 | 10 | 0 | 0 | 0 |
| synthetic/motion | 36 | 31 | 4 | 1 | 0 |
| synthetic/orientation | 6 | 6 | 0 | 0 | 0 |

## Failures

### `real/ds003345/default`

mcflirt's defaults (middle volume as reference)

- **FD (Power, 50 mm) correlation** (r = 0.9885, threshold ≥ 0.9893 (band)): max |ΔFD| 0.0851 mm, max FD 1.353 mm

## Expected divergences

| Case | Reason |
|---|---|
| `syn/motion/small/no-fsloutputtype` | Without FSLOUTPUTTYPE, mcflirt writes the .mat files and fails (exit 1); larmorx writes every output as .nii.gz (its default). |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `real/ds000005/opt-gdt` | option sweep: -gdt (writes grefvol_<out>, impossible with an absolute -out) | mcflirt exit [-6]: terminate called after throwing an instance of 'NiftiIO::NiftiException' what(): Error: cant open file grefvol_~/VScode_projects/super_fmrip… |
| `real/ds003763-truncated/default` | a .nii.gz truncated as published (65,536 bytes) | mcflirt exit [-6]: Image Exception : #22 :: Failed to read volume <testdata-cache>/openneuro/ds003763/sub-16111/func/sub-16111_task-heart_bold.nii.gz Erro; lar… |
| `syn/cli/help` | -help alone | mcflirt exit [1]: ; larmorx exit [1]: |
| `syn/cli/no-arguments` | no arguments | mcflirt exit [1]: ; larmorx exit [1]: |
| `syn/cli/refvol-minus2` | -refvol -2 | mcflirt exit [-6]: Image Exception : #61 :: Invalid t index in [] operator terminate called after throwing an instance of 'std::runtime_error' what(): Invalid … |
| `syn/cli/stages0` | -stages 0 with -reffile: the reference is never read | mcflirt exit [-6]: Image Exception : #8 :: Attempted to use affine transform with no voxels in vout terminate called after throwing an instance of 'std::runtim… |
| `syn/cli/unknown-option-middle` | an unknown option followed by others | mcflirt exit [255]: Unrecognised option -edge; larmorx exit [255]: Unrecognised option -edge |
| `syn/motion/small/no-input` | no -in | mcflirt exit [2]: Input filename not found; larmorx exit [2]: Input filename not found |
| `syn/motion/small/reffile-grid2` | reference on a different grid | mcflirt exit [-6]: Image Exception : #2 :: Attempted to copydata with non-matching sizes terminate called after throwing an instance of 'std::runtime_error' wh… |
| `syn/motion/small/refvol-out-of-range` | -refvol 12 with 12 volumes | mcflirt exit [-6]: Image Exception : #61 :: Invalid t index in [] operator terminate called after throwing an instance of 'std::runtime_error' what(): Invalid … |
| `syn/motion/small/unknown-option` | -edge (offered by nipype) is not an mcflirt option | mcflirt exit [255]: Lacking argument to option -edge; larmorx exit [255]: Lacking argument to option -edge |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (fc392f579ce7) |
| larmorx-testdata | 28167dea9be5 |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

Each recorded mcflirt run is replayed with the same arguments and FSLOUTPUTTYPE; outputs are read with nibabel. Parameters are decomposed about the centre of the reference's field of view (FSL-mm) for the comparison; the `.par` files are compared as written (about the reference's intensity-weighted centre). The `descrip` header field differs by design (larmorx writes its own name). larmorx uses all logical CPUs; its results do not depend on the thread count.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `real/ds000005/default` | pass | 15/15 | mcflirt's defaults (middle volume as reference) |
| `real/ds000005/fmriprep` | pass | 10/10 | fMRIPrep's command (nipype MCFLIRT(save_mats=True), ref_file) |
| `real/ds000005/fmriprep-repeat2` | pass | 10/10 | repeat of real/ds000005/fmriprep (run-to-run determinism) |
| `real/ds000005/fmriprep-repeat3` | pass | 10/10 | repeat of real/ds000005/fmriprep (run-to-run determinism) |
| `real/ds000005/fmriprep-reports` | pass | 15/15 | fMRIPrep's command plus -plots -rmsrel -rmsabs |
| `real/ds000005/opt-2d` | pass | 15/15 | option sweep: -2d |
| `real/ds000005/opt-bins64` | pass | 15/15 | option sweep: -bins 64 |
| `real/ds000005/opt-cost-corratio` | pass | 15/15 | option sweep: -cost corratio |
| `real/ds000005/opt-cost-leastsquares` | pass | 15/15 | option sweep: -cost leastsquares |
| `real/ds000005/opt-cost-mutualinfo` | pass | 15/15 | option sweep: -cost mutualinfo |
| `real/ds000005/opt-cost-normcorr` | pass | 15/15 | option sweep: -cost normcorr |
| `real/ds000005/opt-cost-normmi` | pass | 15/15 | option sweep: -cost normmi |
| `real/ds000005/opt-cost-woods` | pass | 4/4 | option sweep: -cost woods |
| `real/ds000005/opt-dof12` | pass | 15/15 | option sweep: -dof 12 |
| `real/ds000005/opt-dof7` | pass | 15/15 | option sweep: -dof 7 |
| `real/ds000005/opt-dof9` | pass | 15/15 | option sweep: -dof 9 |
| `real/ds000005/opt-fudge` | pass | 15/15 | option sweep: -fudge |
| `real/ds000005/opt-gdt` | both-error | – | option sweep: -gdt (writes grefvol_<out>, impossible with an absolute -out) |
| `real/ds000005/opt-meanvol` | pass | 17/17 | option sweep: -meanvol |
| `real/ds000005/opt-meanvol-noref` | pass | 17/17 | -meanvol without -reffile |
| `real/ds000005/opt-nn_final` | pass | 15/15 | option sweep: -nn_final |
| `real/ds000005/opt-refvol0` | pass | 15/15 | default with -refvol 0 |
| `real/ds000005/opt-rotation2` | pass | 15/15 | option sweep: -rotation 2 |
| `real/ds000005/opt-scaling3` | pass | 15/15 | option sweep: -scaling 3 |
| `real/ds000005/opt-smooth0` | pass | 15/15 | option sweep: -smooth 0 |
| `real/ds000005/opt-smooth2` | pass | 15/15 | option sweep: -smooth 2 |
| `real/ds000005/opt-spline_final` | pass | 15/15 | option sweep: -spline_final |
| `real/ds000005/opt-stages1` | pass | 15/15 | option sweep: -stages 1 |
| `real/ds000005/opt-stages2` | pass | 15/15 | option sweep: -stages 2 |
| `real/ds000005/opt-stats` | pass | 21/21 | option sweep: -stats |
| `real/ds000117/default` | pass | 15/15 | mcflirt's defaults (middle volume as reference) |
| `real/ds000117/fmriprep` | pass | 10/10 | fMRIPrep's command (nipype MCFLIRT(save_mats=True), ref_file) |
| `real/ds000117/fmriprep-reports` | pass | 15/15 | fMRIPrep's command plus -plots -rmsrel -rmsabs |
| `real/ds000122/default` | pass | 15/15 | mcflirt's defaults (middle volume as reference) |
| `real/ds000122/fmriprep` | pass | 10/10 | fMRIPrep's command (nipype MCFLIRT(save_mats=True), ref_file) |
| `real/ds000122/fmriprep-reports` | pass | 15/15 | fMRIPrep's command plus -plots -rmsrel -rmsabs |
| `real/ds000210-rest-echo2/default` | pass | 15/15 | mcflirt's defaults (middle volume as reference) |
| `real/ds000210-rest-echo2/fmriprep` | pass | 10/10 | fMRIPrep's command (nipype MCFLIRT(save_mats=True), ref_file) |
| `real/ds000210-rest-echo2/fmriprep-reports` | pass | 15/15 | fMRIPrep's command plus -plots -rmsrel -rmsabs |
| `real/ds000258-bigendian/default` | pass | 15/15 | mcflirt's defaults (middle volume as reference) |
| `real/ds000258-bigendian/fmriprep` | pass | 10/10 | fMRIPrep's command (nipype MCFLIRT(save_mats=True), ref_file) |
| `real/ds000258-bigendian/fmriprep-reports` | pass | 15/15 | fMRIPrep's command plus -plots -rmsrel -rmsabs |
| `real/ds003345/default` | fail | 14/15 | mcflirt's defaults (middle volume as reference) |
| `real/ds003345/fmriprep` | pass | 10/10 | fMRIPrep's command (nipype MCFLIRT(save_mats=True), ref_file) |
| `real/ds003345/fmriprep-reports` | pass | 15/15 | fMRIPrep's command plus -plots -rmsrel -rmsabs |
| `real/ds003763-truncated/default` | both-error | – | a .nii.gz truncated as published (65,536 bytes) |
| `real/ds005040/default` | pass | 15/15 | mcflirt's defaults (middle volume as reference) |
| `real/ds005040/fmriprep` | pass | 10/10 | fMRIPrep's command (nipype MCFLIRT(save_mats=True), ref_file) |
| `real/ds005040/fmriprep-reports` | pass | 15/15 | fMRIPrep's command plus -plots -rmsrel -rmsabs |
| `real/ds006010/default` | pass | 15/15 | mcflirt's defaults (middle volume as reference) |
| `real/ds006010/fmriprep` | pass | 10/10 | fMRIPrep's command (nipype MCFLIRT(save_mats=True), ref_file) |
| `real/ds006010/fmriprep-reports` | pass | 15/15 | fMRIPrep's command plus -plots -rmsrel -rmsabs |
| `real/ds006736/default` | pass | 15/15 | mcflirt's defaults (middle volume as reference) |
| `real/ds006736/fmriprep` | pass | 10/10 | fMRIPrep's command (nipype MCFLIRT(save_mats=True), ref_file) |
| `real/ds006736/fmriprep-reports` | pass | 15/15 | fMRIPrep's command plus -plots -rmsrel -rmsabs |
| `syn/cli/bare-input` | pass | 10/10 | input given without -in |
| `syn/cli/dof5` | pass | 16/16 | -dof 5 (below 6) |
| `syn/cli/help` | both-error | – | -help alone |
| `syn/cli/help-with-input` | pass | 2/2 | -help after -in |
| `syn/cli/no-arguments` | both-error | – | no arguments |
| `syn/cli/plots-only` | pass | 5/5 | -plots without -mats: is a .mat directory created |
| `syn/cli/refvol-minus1` | pass | 15/15 | -refvol -1 |
| `syn/cli/refvol-minus2` | both-error | – | -refvol -2 |
| `syn/cli/rmsrel-only` | pass | 11/11 | -rmsrel without -mats: which files appear in the .mat directory |
| `syn/cli/stages0` | both-error | – | -stages 0 with -reffile: the reference is never read |
| `syn/cli/unknown-option-middle` | both-error | – | an unknown option followed by others |
| `syn/content/background/default` | pass | 15/15 | defaults |
| `syn/content/background/reffile` | pass | 16/16 | -reffile phantom/ref |
| `syn/content/nan/default` | pass | 15/15 | defaults |
| `syn/content/nan/reffile` | pass | 15/15 | -reffile phantom/ref |
| `syn/content/ramp/reffile` | pass | 4/4 | ramp moved along x: the default cost finds the shifts |
| `syn/content/ramp/reffile-smooth0` | pass | 4/4 | ramp with -smooth 0 (the unweighted cost) |
| `syn/dtypes/float32/default` | pass | 15/15 | defaults |
| `syn/dtypes/float32/reffile` | pass | 15/15 | -reffile phantom/ref |
| `syn/dtypes/float64/default` | pass | 15/15 | defaults |
| `syn/dtypes/float64/reffile` | pass | 15/15 | -reffile phantom/ref |
| `syn/dtypes/int16-scaled/default` | pass | 15/15 | defaults |
| `syn/dtypes/int16-scaled/reffile` | pass | 15/15 | -reffile phantom/ref |
| `syn/dtypes/uint16/default` | pass | 15/15 | defaults |
| `syn/dtypes/uint16/reffile` | pass | 15/15 | -reffile phantom/ref |
| `syn/dtypes/uint8/default` | pass | 15/15 | defaults |
| `syn/dtypes/uint8/reffile` | pass | 15/15 | -reffile phantom/ref |
| `syn/geometry/single-volume-3d/default` | pass | 14/14 | defaults |
| `syn/geometry/single-volume-3d/reffile` | pass | 14/14 | -reffile phantom/ref |
| `syn/geometry/single-volume/default` | pass | 14/14 | defaults |
| `syn/geometry/single-volume/reffile` | pass | 15/15 | -reffile phantom/ref |
| `syn/geometry/thin-slab/default` | pass | 16/16 | 16 mm slab: in-plane registration |
| `syn/geometry/thin-slab/fov10` | pass | 15/15 | -fov 10: the 8 mm reference grid still has fewer than 3 slices |
| `syn/geometry/thin-slab/rotation2` | pass | 15/15 | in-plane mode prints the tolerances |
| `syn/geometry/tiny/default` | pass | 15/15 | syn/geometry/tiny/default |
| `syn/geometry/two-volumes/default` | pass | 14/14 | defaults |
| `syn/geometry/two-volumes/reffile` | pass | 15/15 | -reffile phantom/ref |
| `syn/motion/identical-copies/default` | pass | 15/15 | defaults |
| `syn/motion/identical-copies/reffile` | pass | 16/16 | -reffile phantom/ref |
| `syn/motion/identical-copies/reffile-fudge` | pass | 16/16 | -fudge on identical copies: every volume starts from the identity |
| `syn/motion/large/default` | pass | 15/15 | defaults |
| `syn/motion/large/reffile` | pass | 16/16 | -reffile phantom/ref |
| `syn/motion/small/2d` | pass | 16/16 | -2d forces in-plane registration |
| `syn/motion/small/default` | pass | 15/15 | defaults |
| `syn/motion/small/default-report` | pass | 16/16 | progress messages, middle-volume reference |
| `syn/motion/small/dof12` | pass | 16/16 | syn/motion/small/dof12 |
| `syn/motion/small/dof7` | pass | 16/16 | syn/motion/small/dof7 |
| `syn/motion/small/fsloutputtype-nifti` | pass | 16/16 | FSLOUTPUTTYPE=NIFTI with -out ending in .nii.gz |
| `syn/motion/small/fudge` | pass | 16/16 | -fudge: no initialisation from the previous volume |
| `syn/motion/small/init` | pass | 16/16 | -init with a 5 mm x translation |
| `syn/motion/small/meanvol` | pass | 17/17 | syn/motion/small/meanvol |
| `syn/motion/small/nifti2` | pass | 16/16 | FSLOUTPUTTYPE=NIFTI2_GZ |
| `syn/motion/small/nn_final` | pass | 16/16 | syn/motion/small/nn_final |
| `syn/motion/small/no-fsloutputtype` | expected-divergence | 1/1 | FSLOUTPUTTYPE unset |
| `syn/motion/small/no-input` | both-error | – | no -in |
| `syn/motion/small/no-out` | pass | 11/11 | no -out: the output name is <input>_mcf |
| `syn/motion/small/out-no-ext` | pass | 16/16 | -out without an extension |
| `syn/motion/small/out-twice` | pass | 23/23 | the same -out twice: the second .mat directory gets a '+' |
| `syn/motion/small/reffile` | pass | 16/16 | -reffile phantom/ref |
| `syn/motion/small/reffile-4d` | pass | 15/15 | a 4D file as -reffile (its first volume is used) |
| `syn/motion/small/reffile-grid2` | both-error | – | reference on a different grid |
| `syn/motion/small/reffile-ras` | pass | 15/15 | reference stored RAS (x-flipped data): same physical image as phantom/ref |
| `syn/motion/small/refvol-0` | pass | 15/15 | syn/motion/small/refvol-0 |
| `syn/motion/small/refvol-out-of-range` | both-error | – | -refvol 12 with 12 volumes |
| `syn/motion/small/report` | pass | 16/16 | progress messages |
| `syn/motion/small/sinc_final` | pass | 16/16 | syn/motion/small/sinc_final |
| `syn/motion/small/smooth0` | pass | 16/16 | syn/motion/small/smooth0 |
| `syn/motion/small/spline_final` | pass | 16/16 | syn/motion/small/spline_final |
| `syn/motion/small/stages1` | pass | 16/16 | syn/motion/small/stages1 |
| `syn/motion/small/stages4` | pass | 16/16 | syn/motion/small/stages4 |
| `syn/motion/small/stats` | pass | 22/22 | syn/motion/small/stats |
| `syn/motion/small/unknown-cost` | pass | 16/16 | an unknown cost name |
| `syn/motion/small/unknown-option` | both-error | – | -edge (offered by nipype) is not an mcflirt option |
| `syn/orientation/oblique/default` | pass | 15/15 | defaults |
| `syn/orientation/oblique/reffile` | pass | 15/15 | -reffile phantom/ref |
| `syn/orientation/ras-unflipped/default` | pass | 15/15 | defaults |
| `syn/orientation/ras-unflipped/reffile` | pass | 15/15 | -reffile phantom/ref |
| `syn/orientation/ras/default` | pass | 15/15 | defaults |
| `syn/orientation/ras/reffile` | pass | 16/16 | -reffile phantom/ref |
