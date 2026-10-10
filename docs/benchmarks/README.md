# Benchmarks

Speed of larmorx against the tools it replaces, on real data from `larmorx-testdata` (files tagged `benchmark` plus typical inputs). Each report records the machine, the versions and the method (median of repeated runs after a warm-up).

| Report | Compared with |
|---|---|
| [nifti-io](nifti-io.md) | nibabel, SimpleITK |
| [ants-apply-transforms](ants-apply-transforms.md) | antsApplyTransforms (ANTs 2.6.5 via ANTsPy) |
| [afni-tshift](afni-tshift.md) | 3dTshift (AFNI 25.2.09 binary) |
| [ants-programs](ants-programs.md) | ImageMath, ThresholdImage, MultiplyImages (ANTs 2.6.5 binaries) |

Reproduce a report (build larmorx in release mode first):

```bash
maturin develop --release
python -m larmorx_validation bench nifti-io --tier full --out docs/benchmarks
python -m larmorx_validation bench ants-apply-transforms --repeats 3 --threads 1 4 0 --out docs/benchmarks
python -m larmorx_validation bench afni-tshift --repeats 3 --threads 1 0 --out docs/benchmarks
python -m larmorx_validation bench ants-programs --repeats 5 --threads 1 4 0 --out docs/benchmarks
```
