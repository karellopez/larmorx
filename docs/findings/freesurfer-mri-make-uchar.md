# Findings: FreeSurfer 8.2.0's `mri_make_uchar` (replica, queue item 3.2-22)

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source and the official
binary's object code; the clean room's input is `specs/mri_make_uchar.md`).

Track D's relay 51 (2026-11-22). The replica: `crates-freesurfer/larmorx-freesurfer-volumes/src/
mri_make_uchar/` (the whole program, `mri_convert/mri_make_uchar.cpp`). Checks: the open probe
`validation/investigations/freesurfer-mri-make-uchar-probe/` (103 command lines), the closed
`validation/probes/fs-make-uchar/` (`prog.py`, `real.py`, `record.py`), the golden cases
`tests/mri_make_uchar_golden.rs`, the memory entry `MRI_MAKE_UCHAR_CASES`; the record
`docs/validation/freesurfer-mri-make-uchar.md`.

## 1. The official build's arithmetic (object code)

`MRIconvertToUchar` decoded from the disassembly of `bin/mri_make_uchar` (stripped; the function
at 0x16f00, found by the mapping line's format string at 0x215240):

- The ball's loop calls `MRIvoxelToTalairach` with `x, y, z` as doubles, then computes
  `sqrt((xt² + yt²) + zt²)` of its outputs and discards it (the `sqrt` call stays: it may set
  `errno`, which it cannot for a sum of squares); `MRIvoxelToWorld`, three `cvtsd2ss` into `v_X`,
  `LTAtransformPoint`, three `cvtss2sd` of `v_Y`, `sqrt((x² + y²) + z²)`, `comisd MAX_R, r` (`r <
  MAX_R`, false for a NaN).
- The line: `y1 = 255.0 * FIRST_PERCENTILE` (`mulsd` with 255 the destination), `m = (110 - y1) /
  (x2 - x1)`, `b = 110 - x2 * m` (`mulsd m` into `x2`). `bin_size` is optimised away.
- The mapping: `(val * m) + b` (`val` the destination of both), `comisd val, 0` then `comisd 255,
  val`: a NaN is neither clamped nor counted and reaches `MRIsetVoxVal` as `cvtsd2ss` makes it.
- The printed `thresh` and `maxsat`: `(-b) / m` (`xorpd` of `b`'s sign, then `divsd m`) and `(255 -
  b) / m`.

Only the printed values can show a NaN's sign or payload (the output is `uchar`); the replica
follows these orders with fs.core's `sse` helpers.

## 2. `errno` in the ball's loop

`MRIvoxelToTalairach` of a volume that is not coronal returns its error (`ErrorPrintf`) at every
voxel, and `ErrorPrintf` prints `errno`'s message first when `errno` is not 0, never clearing it.
In the same loop `LTAtransformAtPoint` calls `exp(-dsq / (2 sigma²))` for the transform's weight:
an `.xfm` read by `ltaMNIread` has `sigma = 1` and centre 0, so the weight underflows for every
point farther than about 38.6 mm from the origin, and glibc's `exp` then sets `ERANGE` (measured:
`validation/investigations/freesurfer-make-uchar-errno/`, `ERANGE` exactly when a finite argument
gives 0 or an infinity). The official program's stderr for an axial 32^3 volume: the first voxel's
error with whatever `errno` the start-up and the reads left (observed: `No such file or
directory` for the probe's volumes written by nibabel, nothing for the golden cases' volumes
written by the replica; the replica gives the same in both), every later one with `Numerical
result out of range` first. The replica models `ERANGE` in fs.volume's `LTAtransformAtPoint`
(track C's file; the weights normalise to 1 for one transform, so no result changes).

The point `MRIvoxelToTalairach` computes is never used: with a coronal volume its only effect is
the geometry's matrix cache, which `MRIvoxelToWorld` fills alike right after; the replica calls it
only while that cache is empty (FS-1130 for the other case).

## 3. The defects

- **FS-1127** (class a, rule B): `argc < 3` is the only check, but the output is `argv[3]`: two
  arguments pass it, and `MRIwrite(mri_out, NULL)` crashes in `MRIgetVolumeName`'s `strcpy` after
  the whole computation (signal 11). Standard output is a pipe's or a file's buffer, never flushed
  before: lost. The default prints the usage and ends with status 1 before reading.
- **FS-1128** (class a, rule B): `get_option` reads `argv[2]` for `-f`, `-w`, `-r` with `sscanf`:
  given last, `sscanf(NULL, ...)` crashes. `-h` and `-v` given last store the NULL and the
  argument count then stops at the usage; the default gives the same for the three.
- **FS-1129** (class c, rule A): both `MRIwrite` calls' statuses and `HISTOwriteTxt`'s are
  ignored; the default ends with status 1 after the writer's messages (the timing line left out).
- **FS-1130** (class f, rule A): the per-voxel error of a volume that is not coronal, though the
  Talairach point is never used. 2 lines per voxel: 33.5 million lines (about 1.8 GB of standard
  error) for a 256^3 axial volume. The default prints none.
- **FS-1131** (class c, rule B): `m = (y2 - y1) / (x2 - x1)` with `x1 = x2` (both percentiles in
  bins of one value: an empty ball, `-r 0`, a transform that moves the head away, an all-zero
  volume, a NaN percentile) is infinite or NaN; every mapped voxel is then NaN, which `MRIsetVoxVal`
  stores as `nint(NaN)`'s low byte, 0 (FS-12): a volume of zeros, status 0. The default stops with
  an error after the `i1` line.

## 4. Speed

Uncached, on the recorded heads' 18 calls (`real.py`): FreeSurfer 43 s, the replica 60 s. On one
256^3 `float` volume (2026-11-22, the main machine): FreeSurfer 2.6 s, the replica 3.9 s, of which
the ball's loop 2.2 s: per voxel `MRIvoxelToWorld`, `LTAtransformAtPoint` (about 50 ns: both
`exp`s of larmorx-core's `Libm` witness, the matrix copy) and `MatrixMultiply` (about 40 ns:
fs.core's generic element access; a direct-index variant measured no faster). Bit for bit speed
work on those library functions is a speed relay's item.

## 5. Memory

FreeSurfer's peak is about 550 MiB even for a `uchar` input (599 MiB for a 256^3 `float` one); the
replica's is its volumes and the reads' buffers (38 and 85 MiB).

## 6. The real call's input

`mri_nu_correct.mni`'s `mri_make_uchar nu.mgz transforms/talairach.xfm nu.mgz` reads the `float`
volume that `mri_convert ... --like orig.mgz` made from the rescaled N3 output; the reference runs
keep only the step's output (the `uchar` `nu.mgz`). The validation uses stand-ins (`real.py`):
the run's `orig.mgz` times the run's own recorded rescale ratio as `float`.
