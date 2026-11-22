# Findings: FreeSurfer 8.2.0's `mri_mask` (replica, queue item 3.2-26)

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source and the official
binary's runs; the clean room's input is `specs/mri_mask.md`).

Track D's relays 51 and 52 (2026-11-22). The replica: `crates-freesurfer/larmorx-freesurfer-volumes/
src/mri_mask/` (the whole program, `mri_mask/mri_mask.cpp` 89-655). Checks: the open probe
`validation/investigations/freesurfer-mri-mask-probe/` (193 command lines), the closed
`validation/probes/fs-mask/` (`prog.py`, `real.py`, `record.py`), the golden cases
`tests/mri_mask_golden.rs`, the memory entry `MRI_MASK_CASES`; the record
`docs/validation/freesurfer-mri-mask.md`.

## 1. The options

`get_option` compares the option's name with `stricmp` in a chain of `else if`s; `invert` appears
twice, first for the mask (`InvertMask`), then for the transform (`invert = 1`, `Inversely apply
the given transform` on standard error): the second is never reached, and the variable `invert` is
never read anyway. The replica keeps the names in the source's order (`OPTION_NAMES`), the first
equal one deciding.

FS-1132 (relay 51): every option that reads `argv[2]` and beyond with `atoi`, `atof`, `sscanf` or
`MRIreadHeader` reads the NULL after the last argument (or the next option's text when too few
values precede the three arguments: no crash then, but wrong values; the replica gives the same
values). `-xform` given last stores the NULL as the file name (`(null)` printed, no transform
read): no crash, kept.

## 2. The three crashes of relay 52 (FS-1133, FS-1134, FS-1135)

Each checked against the official program (relay 52's black-box folder, then the open probe):

- **FS-1133.** `REGIONgetBoundingBoxEqOdd` returns NULL after printing its `ERROR` line when the
  box's `dx > mask->height`, `dy > mask->width`, or its `dz` is even, not below the depth, and
  `z == 0` (`cannot make z odd`). Its branch "remove 1 from z and add 1 to dz" is unreachable: with
  `z > 0`, `dz <= depth - z < depth`, so the earlier branch (add 1 to dz) always takes it. Its last
  "cannot make x=y" is unreachable too (`dx == dy` returned before). `mri_mask` passes the NULL to
  `REGIONprint(stdout, region)`, which reads `r->x`: signal 11. Three inputs (a 32 x 8 x 9 volume,
  an 8 x 32 x 9 one, a 16 x 16 x 32 one, each with a full mask): status 139, nothing on standard
  output (held in the buffer; no error message flushed it).
- **FS-1134.** `LTAreduce` returns NULL after `ERROR: LTAreduce(): LTs <k-1> and <k> do not match`;
  `mri_mask` frees the read array (`LTAfree(&tmp)`) and reads `lta->xforms[0].src.valid`: signal 11.
  A two-transform `.lta` (every matrix first, then the geometries, as `ltaReadFileEx` reads them)
  whose second transform's source and destination are neither the first's destination: status 139.
- **FS-1135.** `TransformApplyType` returns NULL only from `MRIlinearTransformInterp`'s
  `MatrixInverse` of the voxel-to-voxel matrix (`ErrorReturn(NULL, ... "MRIlinearTransform: xform
  is singular")`; the morph path never returns NULL). The transform's own matrix passed
  `LTAfillInverse` (in `LTAreduce`, and again in `LTAchangeType`) before: `vnl_det` of the 4 x 4
  `float` matrix is not 0. The voxel-to-voxel matrix is that matrix through the two volumes'
  `i2r`/`r2i` products in `float`; its determinant can round to exactly 0 where the original's did
  not. A search (`validation/investigations/freesurfer-mri-mask-singular/search.py`; two of its
  cases are the probe's `SINGULAR_V2V` and `SINGULAR_RAS`): 400 random rank-2 3 x 3 matrices moved one to three ulps in one element, on an
  oblique 8^3 grid of voxels 1.1 x 0.7 x 1.3 mm: 25 of 400 (voxel to voxel) and 19 of 400 (scanner
  to scanner) reach the NULL, the rest invert or fail at `LTAfillInverse` (`TransformInvert: xform
  noninvertible`, status 252). Diagonal matrices near `vnl_det`'s underflow never reach it (both
  determinants round alike). After the NULL, `mri_mask` goes on with `mri_mask = NULL`:
  `MRIcopy(NULL, NULL)` returns NULL at once (`mri_src == mri_dst`); the threshold and `samseg`
  loops read `mri_mask->depth`, `MRIclone(NULL)` (`-lh`, `-rh`), `REGIONgetBoundingBoxM(NULL)`,
  `REGIONgetBoundingBox(NULL)` (`-BBEQ`), `MRIcropToFoV(NULL)`, `mri_mask->xsize` (`-mm`) and
  `MRIdilate(NULL)` (`mri_src->type`) crash; `MRIerodeNN(NULL, NULL)` prints `cannot be done
  in-place` and returns NULL, `MRIcopy(NULL, NULL)` NULL, `MRIfree(&NULL)` the error `MRIfree: null
  pointer\n`; `MRImask`'s `getVolGeom(NULL)` (`VOL_GEOM::copyFromMRI`) ends with `ErrorExit
  (ERROR_BADPARM, "must have a valid MRI (src)")`: status 252. Measured on each option.

Rule B for all three (an input that makes no sense for the program; FreeSurfer crashes or ends
with an unrelated error): by default a clear error with status 1 at the point of failure;
`reproduce` the official's status and output.

## 3. Standard output at a crash: the C++ error stream flushes it

FreeSurfer's `ErrorPrintf` and `ErrorExit` write through `fs::error()`, a `std::cerr` stream;
`std::cerr` is tied to `std::cout`, and with `sync_with_stdio` (the default) `std::cout` writes
through C's `stdout`: every error message first flushes C's standard output buffer. Observed with
FS-1135's crashing cases: the official program's `DoAbs = 0` and `Applying transform to the mask
volume` survive the crash (the singular error flushed them), while FS-1133's and FS-1134's crashes,
with no error message before, lose everything. The replica's standard output (`Cout` in
`main.rs`) keeps what a crash keeps: the part before the last flush (an explicit
`fflush(stdout)`, i.e. the writer's `flush`, or a message of the C++ error stream, counted by
`larmorx-freesurfer-core`'s `StderrState::cxx_messages`, which `log::error`'s writer raises) plus
the full 4,096-byte buffers after it. `fs::warning()` (also `std::cerr`) is not counted: no
warning precedes a crash `mri_mask` reaches.

## 4. Transforms the library refuses in both modes (FS-307, FS-309, FS-310)

A text file named as an `.lta` of one line (`-xform notes.lta`): FreeSurfer's `ltaReadFileEx`
scans its missing second line and crashes (status 139); the library (`larmorx-freesurfer-volume`'s
`ltaread.rs`, track C) ends with an error in both modes (`specs/freesurfer-volume.md`'s table:
"the same" in both). The probe counts such cases (`NOT_COMPARABLE`), as the other programs'
probes do.

## 5. A morph inverted needs more than 4 GiB

`mri_mask -xform talairach.m3z -T 5 norm.mgz <the atlas-space volume> out.mgz` (the mask in the
atlas, the input in the head: `Inverting transform to match MRI geometries`, `GCAMinvert` on the
256^3 head) was ended by a 4 GiB memory box in both programs (relay 52, exit 137 for each). Not run
again in a larger box (the rule); `real.py` runs the forward direction only (6 calls on 3 classic
runs, identical in both modes; the first, uncached run: official 20.8 s, the port 26.7 s for the
six: a speed item for later).

## 6. The recorded calls

recon-all's `mri_mask` steps (24: `-T 5` in both streams, the v8 stream's two `-oval 1 -invert`
calls; relay 51's replays reproduced them in both modes, relay 52's record redoes them) and, from
the v8 stream's scripts, the calls whose inputs the runs keep (`T1.mgz` masked by `synthstrip.mgz`,
`-bb 3` on `orig.mgz` into NIfTI: `real.py`, identical in both modes on 3 runs). The scripts' other
calls (`-T .001 -crop 0` and `-T .001 -crop-to-fov-mm 80 80 80` on priors in temporary folders)
read files the runs do not keep; the probe covers their options.
