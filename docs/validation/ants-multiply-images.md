# Parity: MultiplyImages

**All cases agree.** 15 cases on the `standard` tier: 12 pass, 3 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants MultiplyImages` / `lx.ants.multiply_images` (crate larmorx-ants)
- **Reference:** MultiplyImages from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`
- **Test data:** larmorx-testdata `00b862132e9c`, tier `standard`
- **Generated:** 2026-10-12 on Linux x86_64, with `python -m larmorx_validation parity ants-multiply-images --tier standard`

**Bit-identical: 12 of 12 passing cases** produce exactly the values ANTs writes; 12 of 12 also write exactly its header bytes.
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - ANTS-27: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (exit N -> N; outputs not written): `product/scalar-text`, `product/missing-file`

| Category | Cases | Pass | Both error | Expected divergence | Bit-identical | Most values differing |
|---|---|---|---|---|---|---|
| errors | 3 | 0 | 3 | 0 | 0 | – |
| product | 12 | 12 | 0 | 0 | 12 | 0 |

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
| errors | 3 | 0 | 3 | 0 | 0 |
| product | 12 | 12 | 0 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `errors/no-output-name` | no output name: ANTs prints 'missing output filename' and aborts | ANTs killed by signal 6: terminate called without an active exception; larmorx exit 1: larmorx: MultiplyImages: missing output filename (ANTs aborts: throw wit… |
| `errors/missing-first` | a first image that does not exist | ANTs killed by signal 11: ; larmorx exit 1: larmorx: MultiplyImages: cannot read image '<tmp>/none.nii.gz' (ANTs crashes here) |
| `errors/dimension-5` | dimension 5 | ANTs exit 1: not supported 5; larmorx exit 1: not supported 5 |

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

## Notes

Both programs get the same arguments and files; ANTs runs as its own binary and larmorx in-process through its Python console entry point, both with `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=4`, larmorx with ANTs' defects reproduced (`LARMORX_UPSTREAM_DEFECTS=reproduce`; its default corrects them, and the D13 fix pass reports what that changes). Outputs are read with nibabel (values as stored, before `scl_slope`) and the headers from their raw bytes. Inputs named `gen:<name>` are built by `larmorx_validation.parity.ants_inputs`.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `product/image-image` | pass | 9/9 | the phantom x an operand with zeros and negatives |
| `product/image-mask` | pass | 9/9 | the phantom x a uint8 mask |
| `product/image-scalar` | pass | 9/9 | the phantom x 2.5 |
| `product/image-negative-scalar` | pass | 9/9 | the phantom x -0.5 |
| `product/scalar-text` | pass | 9/9 | a second argument '3abc' that is not a file: atof gives 3 |
| `product/missing-file` | pass | 9/9 | a second file that does not exist: read as a number, atof gives 0 |
| `product/larger-operand` | pass | 9/9 | a larger second image (read at the same indices) |
| `product/int16-scaled` | pass | 9/9 | scaled int16 x the phantom |
| `product/dim2` | pass | 9/9 | a 2D image |
| `product/dim4` | pass | 9/9 | a 4D series |
| `product/real-T1w-mask` | pass | 9/9 | fMRIPrep's T1w x its brain mask |
| `product/real-GM-scalar` | pass | 9/9 | a GM probability map x 0.5 |
| `errors/no-output-name` | both-error | 1/1 | no output name: ANTs prints 'missing output filename' and aborts |
| `errors/missing-first` | both-error | 1/1 | a first image that does not exist |
| `errors/dimension-5` | both-error | 1/1 | dimension 5 |
