# Benchmarks

Speed of larmorx against the tools it replaces, on real data from `larmorx-testdata` (files tagged `benchmark` plus typical inputs). Each report records the machine, the versions and the method (median of repeated runs after a warm-up).

| Report | Compared with |
|---|---|
| [nifti-io](nifti-io.md) | nibabel, SimpleITK |

Reproduce a report (build larmorx in release mode first):

```bash
maturin develop --release
python -m larmorx_validation bench nifti-io --tier full --out docs/benchmarks
```
