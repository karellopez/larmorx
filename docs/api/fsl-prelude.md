# prelude (phase unwrapping)

`lx.fsl.prelude` (command line `larmorx fsl prelude`): FSL 6.0.7.23's `prelude` (fsl-fugue
2603.0), PRELUDE, the Phase Region Expanding Labeller for Unwrapping Discrete Estimates
(Jenkinson 2003, "Fast, automated, N-dimensional phase-unwrapping algorithm", Magnetic Resonance
in Medicine 49:193-197). It unwraps a phase image (radians) inside a mask: the masked phase is
split into regions of similar wrapped phase, which are merged, the best-agreeing pair first,
each moved by the whole number of 2 pi turns that best joins it to its neighbour. The result is
the unwrapped phase in radians, 0 outside the mask. sdcflows (fMRIPrep) runs it on the phase
difference of a phase-difference field map before converting it to a field in Hz:
`prelude --abs=<magnitude> --mask=<mask> --phase=<phase difference> --unwrap=<out>`.

**Implementations.** FSL is distributed under the FSL Licence (non-commercial use only), so
larmorx keeps its FSL tools in two implementations ([licensing](../licensing.md)):
- the bit-exact **replica** (2026-11-06), `larmorx-nc fsl prelude` (package `larmorx-nc`, under
  the FSL Licence's terms: [PROVENANCE](../../crates-nc/larmorx-nc-fugue/PROVENANCE.md)), run as
  a separate program;
- the **clean-room original** is to be written from `specs/prelude.md` (the next part).

Until the original exists, `implementation="auto"` and `"replica"` run the replica and need
`larmorx-nc` installed (`pip install larmorx-nc`); without it they raise
`ReplicaNotFoundError`, and `"original"` raises `NotImplementedError`. The command line follows
`LARMORX_IMPLEMENTATION` the same way (exit 127 when the replica's program is not found).

**Speed.** The replica merges the regions with FSL's arithmetic in FSL's order, but keeps each
region's neighbours and the candidate merges ordered, instead of scanning every region for every
merge: fMRIPrep's call on ds005454-sub-16 (14,678 regions) takes 0.15 s against FSL's 6.5 s,
with the same bits.

**Status: `experimental`** (the record `fsl-prelude-replica` comes with this part's tests).

## Usage

```python
import larmorx as lx

# fMRIPrep's call: a phase difference (radians) with its magnitude and brain mask
res = lx.fsl.prelude("phasediff_rad.nii.gz", "magnitude.nii.gz", mask="mask.nii.gz")
res.unwrapped            # lx.Image, float32, radians

# a complex image, the masks and labels returned, the linear ramps removed during unwrapping
res = lx.fsl.prelude(complex_image="complex.nii.gz", remove_ramps=True,
                     save_mask=True, save_labels=True)
res.mask, res.labels
```

Inputs are paths or in-memory images (an `lx.Image`, a nibabel image, an `(array, affine)`
pair); in-memory images are written to a temporary directory in their own type (uint8, int16,
int32, float32, float64; other types as float32; a complex array as complex64). The result is a
`PreludeResult`: `unwrapped`, and `mask`, `labels`, `raw_phase` (or `None`), `stdout` (the
messages of `verbose=True`). A run that fails raises `ReplicaError` (a `RuntimeError`) with
prelude's messages.

## Options

| `prelude` | Python (`lx.fsl.prelude(...)`) | What |
|---|---|---|
| `-p,--phase <image>` | `phase` (first argument) | the wrapped phase, radians; a range (maximum minus minimum) above 3 pi is refused (exit 255), values beyond [-pi, pi] are rewrapped first |
| `-a,--abs <image>` | `magnitude` (second argument) | the magnitude, for the mask; with `-p` only |
| `-c,--complex <image>` | `complex_image=` | a complex image instead of `-p` and `-a`: its phase and magnitude |
| `-o,--out,-u,--unwrap <name>` | (the result's `unwrapped`) | the unwrapped phase (compulsory) |
| `-m,--mask <image>` | `mask=` | the mask (thresholded at 0.5 unless `-t` is given); its last volume serves the volumes beyond it |
| `-t,--thresh <value>` | `threshold=` | the mask's threshold on the magnitude (default: 10 % of the way from the 2nd to the 98th percentile); the largest connected part above it is kept |
| `-n,--numphasesplit <n>` | `num_phase_split=8` | the number of phase intervals the regions are first split into (at least 2) |
| `-s,--slices` | `slices=True` | labels and unwraps slice by slice, then joins the slices |
| `--labelslices` | `label_slices=True` | labels slice by slice (automatic when the image is larger than 64 voxels along x or y), unwraps in 3D |
| `-f,--force3D` | `force_3d=True` | everything in 3D (overrides `-s`, `--labelslices` and the automatic 2D labels) |
| `--start=<n>` | `start=` | the first volume processed (from 0) |
| `--end=<n>` | `end=` | the last volume processed (default: the last) |
| `--removeramps` | `remove_ramps=True` | removes the linear phase ramps before unwrapping and restores them after |
| `--savemask=<name>` | `save_mask=True` (the result's `mask`) | the mask used |
| `-l,--labels <name>` | `save_labels=True` (the result's `labels`) | the regions' labels (int32) |
| `-r,--rawphase <name>` | `save_raw_phase=True` (the result's `raw_phase`) | the wrapped phase as prelude used it (rewrapped where it was outside [-pi, pi]) |
| `-v,--verbose` | `verbose=True` (the result's `stdout`) | diagnostic messages |
| `-h,--help` | - | the usage (exit 1) |
| `FSLOUTPUTTYPE` | - (NIfTI, read back) | the output files' type; the wrapper sets it and never reads the caller's FSL variables |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects="fix"` | `"reproduce"` gives FSL's result bit for bit |
| - | `implementation="auto"` | `"auto"` or `"replica"` (no original yet) |
| - | `n_threads=1` | threads for writing and reading the images (prelude itself runs on one thread, as FSL's) |

A long option takes its value after `=` (`--start=1`), as in FSL. The Python wrapper checks its
arguments before running anything (`ValueError`): `complex_image` or `phase` and `magnitude`
together, `num_phase_split` of at least 2, `start` and `end` not negative.

## Upstream defects

Corrected by default (D13; [the register](upstream-defects.md)), each with a notice where the
correction changed the result, and reproduced with `upstream_defects="reproduce"`
(`LARMORX_UPSTREAM_DEFECTS=reproduce`):

| ID | FSL 6.0.7.23 | larmorx by default |
|---|---|---|
| FSL-261 | `--removeramps` leaves the image's last column, row and slice out of the ramps (one slice: no ramp removed) | every column, row and slice |
| FSL-262 | `--savemask` and `-l` with several volumes keep only the last volume's | every processed volume's, as a series |
| FSL-263 | `-c` with an image that is not complex reads its bytes as floats and its imaginary part from memory never written | refused (exit 1) |
| FSL-264 | `-c` with a big-endian complex image exchanges the real and imaginary parts | read as stored |
| FSL-265 | `-c` with a complex128 image reads none of its values | read, converted to float |
| FSL-266 | `-v` with `--removeramps` writes an image `grot` in the working directory | not written |
| FSL-268 | NaN magnitudes enter the default threshold, and a NaN first voxel empties the mask | NaN left out of the threshold and the maximum |
| FSL-269 | NaN phases inside the mask enter the regions: a region joined to a NaN moves by about 2^31 turns | NaN voxels left out of the mask, NaN in the output |
| FSL-270 | `--end` before `--start` writes no output and exits 0 | refused (exit 1) |
| FSL-271 | a magnitude or mask of another size is read by the phase's voxel indices | refused (exit 1) |
| FSL-272 | a NaN first phase value skips the phase range check and the rewrap to [-pi, pi] (FSL's extrema are both NaN) | the extrema of the other values: rewrapped, or FSL's range error (exit 255), as without the NaN |

FSL-267 (the range message says 6.283 radians where the check is 3 pi) is a message only: kept
as FSL prints it.

## Validation

- **Replica against FSL 6.0.7.23** (the parity suite `fsl-prelude-replica`): bit-identical exit
  status, messages and files on every option, output type and failure case; fMRIPrep 25.2.6's
  seven prelude calls on four oracle runs (ds005454, ds002603, ds002422, ds001600) bit-identical
  to FSL and to fMRIPrep's own outputs.
- **Golden**: larmorx-nc's 15 cases and 4 failures (`crates-nc/larmorx-nc-cli/tests/golden.rs`)
  on every platform.
