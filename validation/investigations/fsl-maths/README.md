# fslmaths' replica: investigation helpers

The tools behind the findings in `docs/findings/fsl-maths.md` (restricted: source-derived) and
the parity suite `fsl-maths-replica`. Run them from the repository with
`PYTHONPATH=python:validation/src` and FSL 6.0.7.23 installed; FSL runs only through the
oracle-output cache, and every run goes in a memory box
(`scripts/orchestration/boxed.sh 4 python -P ...`).

- `one_case.py <case> [fix|reproduce] [--out DIR]`: one case of the suite into `DIR/fsl` and
  `DIR/rep` (FSL's run from the cache, the replica's in the given mode), with the header fields
  and the values that differ (bits included: NaN signs, last bits).
- `tan_correctly_rounded.py <DIR>`: whether glibc's double `tan` (FSL's) is correctly rounded on
  the inputs of `fn/double-tan`: 3 of 210 values are not (2026-11-03), so neither a correctly
  rounded `tan` nor `libm`'s (11 of 210 off by one ulp) reproduces FSL in `double`; the float
  path (`fn/tan`) is identical.
- `probe_fsl.py <file>` with `probe_filters.txt`: FSL's behaviour on the edge cases of the
  kernels and spatial filters (2026-11-03): `-s 0` and `-kernel gauss 0` make every voxel NaN
  (FSL-160); kernels without voxels or of an unknown type filter with an empty kernel and print
  `ERROR:: Empty mask image` (FSL-161); `-fmedian` with NaN voxels stops with `sort(): detected
  NaN` (FSL-162); `-dilF` and `-eroF` leave NaN voxels out; `-subsamp2` of a 4D image stops with
  `3D only method called by higher-dimensional volume.` (FSL-163); `-dt int ... -kernel sphere 5
  -fmean` traps (FSL-156); `-inm` of an image of zeros prints its error and writes NaN.
