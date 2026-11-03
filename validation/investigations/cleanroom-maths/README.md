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
| `probe_f187.py <dir> <larmorx>` | FSL-187 in BET's and FLIRT's clean-room originals, by black-box runs: `bet2 -v`'s thresholds and FLIRT's costs at the identity (`measurecost1.sch`; `corratio`, `normmi`, `mutualinfo`) on images with and without a value far above their range, by FSL and by the originals in both modes (findings 22) |
| `fit_roc.py <dir> [variant]` | `-roc`'s arithmetic on FSL's curve files and printed lines (the noise counts over every voxel, the candidates not above 0, the means as double sums rounded to single, a double image compared in single precision, each volume's area on the common rate, the last rectangle, the quartiles before the division by `\|L\|`), and the alternatives' differences (findings 19; the probes' command lines in its docstring) |
| `bench.py <outdir> <larmorx> --nc <larmorx-nc> [--impl ...] [--case ...] [--fsl-from <dir>]` | the benchmarks of [`docs/benchmarks/fsl-maths.md`](../../../docs/benchmarks/fsl-maths.md): FSL, the replica and the original at 1 and 14 threads on fMRIPrep's calls (combine_masks, the goodvoxels chain, BET's fslstats call) and the heavy operations on ds005040's BOLD series and T1w, one run each; wall time, peak memory, the original's outputs at both thread counts compared bit for bit and against FSL's (FSL timed directly: benchmarks are the oracle-output cache's exception; in a 24 GiB box) |
| `probe_gauss.py <dir> [<larmorx>]` | FSL's `-s 5` impulse response along y at voxel sizes 0.8333333, 0.85 and 1 mm against forms of the Gaussian's exponent put through §10.2's separable filter (glibc's `expf`/`exp` through `ctypes`), and `-kernel gauss 1 -fmeanu` on a 27³ impulse at 0.85 × 0.8333333 × 0.83 mm against the orders of the three axes' terms; with `<larmorx>`, the original's responses too: FSL's kernel is `i² (d d)` per axis, added x, y, z (findings 23) |
| `probe_geometry.py <dir> <larmorx>` | the output geometry of `-subsamp2`, `-subsamp2offc` and the reductions along x, y, z on 11 header variants (either orientation, oblique and sheared, a matrix unset) and three sizes, FSL against the original field by field and against the rule of findings 24 (159 of 165 calls identical, 6 by one unit in the last place of one offset) |
| `probe_kernel_file.py <dir> <larmorx>` | the morphological filters with a kernel image (zeros, values below 0.5, even and odd sizes and extents) on images with and without zeros: FSL's off-centre warning by the kernel's sizes and no empty-mask message (findings 25; 84 of 84 calls identical) |
| `show_finding.py <finding.json> <outdir> [--fix] [--full]` | a random-testing finding made concrete: its inputs written, FSL (through the cache) and the original run, the streams and the images' headers and first values printed (the triage of relay 6) |
| `compare_call.py <dir> <larmorx> <program> <args...>` | one command line on files in a folder, FSL through the cache against the original, the first differing values; the outputs kept beside (bisecting a finding's chain: findings 29's char arithmetic, 26's TFCE of a double image) |
| `recheck_trial.py <night> <out.jsonl> [--jobs N]` | every finding of the trial replayed with the current build, the verdicts and the remaining differences counted |
| `triage_counts.py <night> <recheck.jsonl> [--json out.json]` | every finding in one triage category (fixed by which change, or explained, expected, registered, open) with counts and one reproducer each: `trial_triage.json` |
| `fit_tfce.py <dir>` | TFCE's arithmetic (`E` in single precision) and `-tfceS`'s output with and without a threshold reached, on FSL's outputs of the replica suite's TFCE cases (findings 17) |

Run from the repository with the validation package on the path, inside a memory box:

```
PYTHONPATH=python:validation/src scripts/orchestration/boxed.sh 4 \
  python -P validation/investigations/cleanroom-maths/probe_thresholds.py
```
