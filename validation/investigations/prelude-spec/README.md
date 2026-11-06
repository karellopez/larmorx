# prelude-spec: the black-box facts and the model behind `specs/prelude.md`

The spec writer's tools for PRELUDE's spec (2026-11-06). Both are written from the spec's text
and from runs of FSL's program, never from FSL's source: a clean-room implementer may read and run
them. Run them with the workspace's `.venv` (numpy, scipy, nibabel), inside
`scripts/orchestration/boxed.sh` (4 GiB for the probe, 12 GiB for the model on fMRIPrep's calls),
with FSL off the `PATH`.

| File | What it shows | How to run |
|---|---|---|
| `blackbox.py` | 58 cases of a `prelude` program on inputs it generates (seeded numpy: a wrapped quadratic phase over an ellipsoid, its magnitude and mask, a three-volume series, a complex64 image, a phase of range 12.6 rad, NaN first voxels, an image wider than 64 voxels, headers with intent, display range, `aux_file` and another sform): every message of the command line (the usage, each parser error, the checks), the image layer's aborts, the range error, `FSLOUTPUTTYPE` unset and unknown, a missing output directory, output names with extensions, every `-v` line (3D, `-s`, `--labelslices`, ramps, series), the files written and their header fields. The source of §2 to §4's **(observed)** facts | `python -I blackbox.py <work dir> $FSLDIR/bin/prelude > fsl.txt` (FSLDIR set); the same with `larmorx fsl prelude` (the original) or `larmorx-nc fsl prelude` (the replica), then `diff` the reports |
| `model.py` | §4 and §6 of the spec modelled in Python (numpy, scipy; the merging a plain heap): the extrema and rewrap, the default threshold, the mask's largest component and its ties, the ramps in FSL's domain or every voxel (`--fix`), the intervals, the region numbers in the three modes, the constraints' single-precision sums in the visiting order, the choice rule, the folds, the reference region, the slices' joining; compared bit for bit with FSL's files and `-v` lines | `python -I model.py blackbox <work dir of blackbox.py run against FSL> --report fsl.txt`; `python -I model.py blackbox <work dir against larmorx's default> --fix --report rep.txt`; `python -I model.py fmriprep <workspace>/oracles/fmriprep-25.2.6` |

What they gave (2026-11-06, FSL 6.0.7.23; the replica built from this branch):

- `blackbox.py` against FSL and against the replica's default: the reports differ only where
  larmorx differs by design: the aborts (FSL's signal 6, larmorx's exit 1 with its line), FSL-261
  (the ramps over every voxel), FSL-262 (the series of masks and labels, so with an unknown
  `FSLOUTPUTTYPE` the default prints every volume's `-v` lines before the message), FSL-266 (no
  `grot`), FSL-270 (refused).
- `model.py blackbox` (FSL): **22 of 22** cases that write an unwrapped phase from real-valued
  inputs bit-identical in every file (`-o`, `--savemask`, `-l`, `-r`) and in every `-v` line
  (3D, 2D labels automatic and asked for, `-s`, ramps, series with `--start`, a mask series, a
  one-voxel mask, an empty mask, `-n` in hexadecimal, a NaN first phase voxel with a 0-to-2π
  phase and with a 12.6 rad range, a wide image with and without `-f`).
- `model.py blackbox --fix` (the replica's default): 22 of 22 (FSL-261's full domain, FSL-262's
  series).
- `model.py fmriprep`: fMRIPrep 25.2.6's **7 of 7** prelude calls (ds001600 three, ds002422 two,
  ds002603, ds005454: 14,678 regions) bit-identical to fMRIPrep's own unwrapped phase.

Not modelled: complex input (glibc's `atan2f`, §6.4, is not in numpy), NaN values inside the
mask (FSL-268, FSL-269: §9), FSL-263 to FSL-265's readers, the size mismatch of FSL-271.
