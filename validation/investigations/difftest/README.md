# Differential random testing: the tools behind its findings

The nightly random testing (`python -m larmorx_validation difftest`,
[docs/validation/difftest.md](../../../docs/validation/difftest.md)) writes each difference as a
finding (`oracles/difftest/<night>/findings/<tool>/<seed>-<n>-<implementation>.json`) with its
shrunk recipe. `difftest --replay <finding>` runs it again through the parity runner; the scripts
here look inside one, by hand. Run them with the repository's `python/` and `validation/src` on
`PYTHONPATH`, with `python -P` (some are named like modules).

| Script | What it shows | Findings it explained |
|---|---|---|
| `inspect_finding.py <finding> [dir]` | an ANTs image program's finding: ANTs twice (does it repeat itself?), larmorx in both D13 modes, exit statuses, messages, values and header fields that differ | the sform/qform choice in vnl's float SVD; allocation refusals; the singular direction |
| `run_pair.py <finding> <dir> <program>` | any ANTs program on a finding's inputs: ANTs at one thread and larmorx in both modes, each run's full standard error, the standard output lines that differ | Atropos on an empty mask (k-means throws before any Gaussian) |
| `inspect_registration.py <finding> <dir> [--crmath]` | an antsRegistration finding: ANTs (as built or with CORE-MATH) and larmorx in `reproduce`, their standard error and the first differing standard output line | the vnl SVD warning of Mattes' NaN bin sizes |
| `bisect_registration.py <finding> <dir> ["old=>new" ...]` | where an antsRegistration finding starts to differ: the first differing `DIAGNOSTIC` line (time columns removed) and every output file's differing values, with edits of the argument line (`re:` for a regular expression) | the float transform's point indexed in float (an Exponential stage after a deformable one, `--float 1`); the CC metric's flat neighbourhoods and ITK-35 |
| `morphology_radius0.py <ANTs bin> <dir> [radii]` | ImageMath's GO, GC, GD and GE with a radius of 0 on small images: the voxels ITK's moving histogram leaves at ±FLT_MAX | ITK-95 |
| `probe_resample_sigma.py <dir>` | ResampleImage's Gaussian interpolation with too few, zero or negative sigmas, ANTs against larmorx in both modes | ANTS-160, ANTS-161 |
| `signed_zero_origin.py <dir> [x,y,z ...]` | the sign of a zero origin component through ANTs' reading and writing (ImageFileWriter's `TransformIndexToPhysicalPoint`) | the writer's origin |
| `inspect_tshift.py <finding> <dir>` | a 3dTshift finding: AFNI, the GPL replica and the clean-room original in both modes, the header fields that differ (black-box) | the clean-room original's AFNI-07 header |
| `mcflirt_pair.py <finding> <dir> [--threads N] [--preload so] ["old=>new" ...]` | an mcflirt finding: FSL's mcflirt (optionally with a CORE-MATH preload) and the replica in `reproduce`, the first differing line of standard error, the `.par` rows and the voxels that differ | the replica's open differences (below) |
| `build_crmath_double_preload.sh <out>` | a CORE-MATH preload with only the double-precision functions (glibc's float ones kept, as the mcflirt replica computes) | ruled glibc's double rounding out for the mcflirt replica's differences |
