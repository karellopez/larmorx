# fslmaths' clean-room original: black-box probes

Helpers of the clean-room implementer of `crates/larmorx-fsl`'s `maths` (fslmaths' original),
behind [`docs/findings/fsl-maths-original.md`](../../../docs/findings/fsl-maths-original.md). They
run FSL 6.0.7.23's `fslmaths` as a black box, every run through the oracle-output cache (the
replica suite's runner, so a run made once is served after), and print what it writes.

| Script | What it shows |
|---|---|
| `probe_thresholds.py` | the thresholds' far side per computing type (FSL-152: `-thr`/`-uthr` inclusive, `-bin` exclusive, `short` wrapping too), `-save`'s type and display range, `-div`/`-rem`/`-max`/`-min` with an image of other sizes (FSL-159) (findings 1 to 3) |
| `show_fsl.py <case> ...` | FSL's exit status, standard output and error on cases of the replica suite (`fsl-maths-replica`), by name: the messages of findings 4 |
| `fsl_outputs.py <outdir> <case or args:...> ...` | FSL's output files, kept for analysis, for cases of the replica suite or ad-hoc command lines with its own inputs (an impulse, a ramp with a NaN or an infinity, asymmetric kernels, `-roc`'s ramp and truths): the Gaussian kernel's normalising sum, the FFT path's orientation and NaN, the subsampling's last bits, `-roc` (findings 7-9, 13, 14, 19, 20); `impulse<n>` an impulse of n³ voxels |
| `fit_orientation.py <dir>` | where FSL lands an asymmetric kernel on an impulse, as stored (its FFT path) or mirrored (its direct sum), for the same kernel on images of two sizes: FSL-192 (findings 14; the probes' command lines in its docstring) |
| `fit_bptf.py <dir>` | `-bptf`'s arithmetic on FSL's double outputs: the spec's form against FSL's order of the high-pass products (findings 15; the probes' command lines in its docstring) |
| `show_fslstats.py <case or args:...> ...` | FSL's fslstats on the replica suite's synthetic cases (never its `fsltest/` ones, FSL's own tests) or ad-hoc command lines with its inputs or the clean-room suite's (`@orig:`): its exit status, standard output and error (findings 21) |
| `fit_roc.py <dir> [variant]` | `-roc`'s arithmetic on FSL's curve files and printed lines (the noise counts over every voxel, the candidates not above 0, the means as double sums rounded to single, a double image compared in single precision, each volume's area on the common rate, the last rectangle, the quartiles before the division by `\|L\|`), and the alternatives' differences (findings 19; the probes' command lines in its docstring) |
| `fit_tfce.py <dir>` | TFCE's arithmetic (`E` in single precision) and `-tfceS`'s output with and without a threshold reached, on FSL's outputs of the replica suite's TFCE cases (findings 17) |

Run from the repository with the validation package on the path, inside a memory box:

```
PYTHONPATH=python:validation/src scripts/orchestration/boxed.sh 4 \
  python -P validation/investigations/cleanroom-maths/probe_thresholds.py
```
