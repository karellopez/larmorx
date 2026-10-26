# Parity: SmoothImage

**All cases agree.** 5 cases on the `panel` tier: 4 pass, 1 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants SmoothImage` / `lx.ants.smooth_image` (crates larmorx-ants, larmorx-image)
- **Reference:** SmoothImage from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`
- **Test data:** larmorx-testdata `52124bc98f9f`, the data panel (tier `full`, tag `panel`) and the fMRIPrep 25.2.6 oracle runs (`oracles/fmriprep-25.2.6`)
- **Generated:** 2026-10-26 on Linux x86_64, with `python -m larmorx_validation parity ants-smooth-image --tier panel`
- **Original programs:** 5 runs: 0 from the oracle cache, 5 run; cache `<workspace>/oracles/cache`, mode `use`

**Bit-identical: 4 of 4 passing cases** produce exactly the values ANTs writes; 4 of 4 also write exactly its header bytes.
- **D13 fix pass:** no case met a corrected defect; the default output is the reproduced one.

| Category | Cases | Pass | Both error | Expected divergence | Bit-identical | Most values differing |
|---|---|---|---|---|---|---|
| panel-images | 5 | 4 | 1 | 0 | 4 | 0 |

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | both produce their outputs, or neither does |
| Header | every field of the 348-byte header identical |
| Data type and shape | identical |
| Affine (as nibabel reads it) | max \|diff\| ≤ 1e-06 mm |
| Values | bit-identical, unless the case declares a tolerance in ulps with its cause |
| stdout (where compared) | identical text |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| panel-images | 5 | 4 | 1 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `panel/images/gaussian-4d-zero-time/bold-mb4-philips` | sigmas 2x2x2x0 on the same series: a sigma of 0 along time, which ITK's recursive Gaussian refuses (ANTs aborts, larmorx exits 1) (ds002603 sub-01: Philips multiband-4 BOLD: 588 volumes of 80 x 80 x 36 (float32, TR 0.55 s)) | ANTs killed by signal 6: ITK ERROR: RecursiveGaussianImageFilter(0x555b98778c00): Sigma must be greater than zero.; larmorx exit 1: larmorx: SmoothImage: Sigma… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (b7ba0d395cee) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

Both programs get the same arguments and files; ANTs runs as its own binary and larmorx in-process through its Python console entry point, both with `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=4`, larmorx with ANTs' defects reproduced (`LARMORX_UPSTREAM_DEFECTS=reproduce`; its default corrects them, and the D13 fix pass reports what that changes). Outputs are read with nibabel (values as stored, before `scl_slope`) and the headers from their raw bytes. Inputs named `gen:<name>` are built by `larmorx_validation.parity.ants_inputs`. Real images come from `larmorx-testdata` (OpenNeuro ds000005 and its fMRIPrep derivatives).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `panel/images/gaussian-2mm/7t-mp2rage` | pass | 9/9 | sigma 2 mm on the raw 7 T MP2RAGE (ds005454 sub-16: 7 T MP2RAGE uniform image as reconstructed (0.8 mm, 224 x 272 x 288… |
| `panel/images/gaussian-1.5mm/flair` | pass | 9/9 | sigma 1.5 mm on 0.42 x 0.42 x 3 mm voxels (ds003592 sub-280: an 82-year-old's FLAIR (0.42 x 0.42 x 3 mm, 44 slices)) |
| `panel/images/median-1/child-t1w` | pass | 9/9 | a median filter of radius 1 on a child's low-quality T1w (ds000256 sub-CTS203: a 7.5-year-old's low-quality 0.8 mm T1w) |
| `panel/images/gaussian-4d/bold-mb4-philips` | pass | 9/9 | sigmas 2x2x2 mm and 0.55 s (one TR) on a 588-volume series (SmoothImage 4) (ds002603 sub-01: Philips multiband-4 BOLD: … |
| `panel/images/gaussian-4d-zero-time/bold-mb4-philips` | both-error | 1/1 | sigmas 2x2x2x0 on the same series: a sigma of 0 along time, which ITK's recursive Gaussian refuses (ANTs aborts, larmor… |
