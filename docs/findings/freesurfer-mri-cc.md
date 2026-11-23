# Findings: FreeSurfer 8.2.0's `mri_cc` (replica, queue item 3.2-27)

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source and the official
binary's runs; the clean room's input will be `specs/mri_cc.md`).

Track D's relay 53 (2026-11-23): the path recon-all's `seg2cc` runs. The replica:
`crates-freesurfer/larmorx-freesurfer-volumes/src/mri_cc/` (`main.rs`: `main`'s aseg path,
`get_option`, `print_usage`; `aseg.rs`: `find_cc_with_aseg`, `cc_cutting_plane_correct`,
`remove_fornix_new`, `find_voxels_close_to_both_hemis`), on fs.volume (`VLSTcreateInRegion` added to
its `voxlist.rs` for it), fs.filter's segments and fs.core's matrices. Not ported yet (relay 54): the
white-matter path (`-a 0`: `find_corpus_callosum`, `find_cc_slice`, `remove_fornix`,
`edge_detection`, `mri/wm` and `talairach.xfm` through `MRItoTalairachEx`), which recon-all never
runs; the probe, golden cases, spec and record.

## 1. The recorded steps

recon-all runs `mri_cc` only inside `seg2cc` (both streams, the volume conformed: `mri_cc -aseg
aseg.auto_noCCseg.mgz -o aseg.auto.mgz -lta transforms/cc_up.lta <subject>` in the subject's `mri`
folder; it reads `aseg.auto_noCCseg.mgz` and `norm.mgz`, writes `aseg.auto.mgz` and the transform).
The replay (`validation/investigations/freesurfer-mri-cc-replay/`): each recorded `seg2cc` step run
again by the replay tool with FreeSurfer's own `seg2cc` script and a `PATH` on which `mri_cc` is the
port (`larmorx-freesurfer --upstream-defects <mode> freesurfer mri_cc`). Every file compared
(FreeSurfer's provenance aside: the transform's and the volume's file names and dates, the script's
log of dates, times and memory figures).

## 2. What the translation keeps

- The rotation search's loop conditions read `zrot_best` and `yrot_best` again at each step, and a
  better score moves them: the window of angles follows the best as it changes (both loops), and the
  inner loop's start uses the best of the moment. The port evaluates the bounds as the C does.
- `x0` is a `double` loop variable over whole numbers; `y0` and `z0` are `int`s set from the
  midline's mean (truncated), so the plane search's centre has an integer y and z and a fractional x
  only where no rotation beats the principal axes' plane (`x0_best = means[0]`).
- `MatrixReallocRotation` takes a `float` angle (the `double` angle converted), and the matrices are
  `float`: the plane's normal is `float` arithmetic turned to `double`.
- The centre's search (`*pyc`, `*pzc`) `break`s out of the slice loop only: the row loop goes on, so
  the last row with a qualifying voxel wins. Only printed (`cc center is found at`); kept.
- The dark white matter added back after the closing reads the aseg as read (`mri_aseg_orig`, the
  head's frame) at the upright frame's voxels: FreeSurfer's mix of frames, kept.
- `find_voxels_close_to_both_hemis`'s 5 x 5 x 5 window through the index tables (coordinates
  clamped to the volume) holds a label exactly when its part inside the volume does: the port widens
  the two labels' masks along z, y and x (a sliding count), the same voxels; volumes whose tables are
  not the clamping ones take the window loop itself. 19 s to 0.9 s on a 256^3 head.
- FreeSurfer prints `WARNING: min_x_fornix not set` for a sagittal slice in which `remove_fornix_new`
  finds no fornix voxel; on the recorded heads this happens in 0 to 5 of the 5 slices (ds000005's v8
  run: all five; most classic runs: none).

## 3. The defects (rules A to D)

- **FS-1201** (class a, memory): `slice_voxels[MAX_CENTRAL_SLICES]` (500 `int`s on the stack) is
  indexed by `x0 - xmin` over the hemispheres' x range in the upright frame; a range of more than 500
  columns (a volume about 500 voxels wide) writes past it. An error in both modes.
- **FS-1202** (class a, rule A): `remove_fornix_new` divides the summed thicknesses of the fornix
  columns by their count `num`; with none (`WARNING: min_x_fornix not set`) the mean is `0/0`, a NaN,
  and so is the deviation (or the variance rounds below 0 and its square root is a NaN), and
  `max_thickness = (int)ceil(NaN)` is `INT_MIN` (x86-64's `cvttsd2si`). The last four columns
  (`min_x_fornix - 3` to `min_x_fornix`, the slice's far edge without fornix) then meet `thickness >=
  INT_MIN` and their callosum voxels are erased from `ystart`, computed as `last_on + INT_MIN - 1`
  (a signed overflow: undefined, in practice wrapped). By default nothing is shaved where the
  thickness is not a number, noted when FreeSurfer's would erase a voxel; `reproduce` FreeSurfer's
  (`INT_MIN`, 32-bit wrapping). On the recorded heads those columns hold no callosum: both modes
  agree (§4).
- **FS-1203** (class a, crash): every option that takes a value reads `argv[2]` (and `argv[3]`,
  `argv[4]` for `-DEBUG_VOXEL`) with `strcpy`, `atoi` or `atof`: given last it reads the NULL after
  the arguments and crashes (signal 11). By default `mri_cc: option <option> needs <n> value(s)`, the
  help, status 1; `reproduce` status 139.
- **FS-1204** (rule A): `MRIwrite` of the aseg (also `-lh`/`-rh`'s copy) and `LTAwrite` of `-lta`'s
  transform are not checked: an output that cannot be written still ends with status 0. By default
  status 1 after the writer's messages (and the end lines); `reproduce` 0.
- To be checked against the official program by the probe (relay 54): `-d` above 50 (`ErrorExit`
  with a `%s` given an `int`: most likely a crash), `-d` of 0 or less (`volume[-1]`, a write before
  the table), the names' 1,024-byte buffers (`strcpy` of a longer option value), the diagnostic
  volume file's divisions below 5 (`volume[4]` unwritten).

## 4. Validation (relay 53)

The 12 recorded `seg2cc` steps (6 heads x 2 streams) with the port's `mri_cc`: `reproduce` 12 of 12
reproduced, `fix` 12 of 12 (`aseg.auto.mgz` identical on every run, or only its provenance differs:
the subjects folder's name; `cc_up.lta` and `seg2cc.log` only in provenance). Time, the whole
`seg2cc` step at one thread (it runs `mri_info` too): the port's 17.7 to 35.8 s (`reproduce`) and 18.4
to 36.5 s (`fix`) against the recorded 14.4 to 95.3 s (362 s recorded for the twelve, 266 s now; the
two pixar008 runs 95.3 and 70.4 s recorded, 35.8 and 33.9 s now); `mri_cc` alone on ds000005's v8
head 18.2 s against FreeSurfer's 17.7 s. What remains
is in fs.volume's library: `MRIclose` of the whole volume 5.4 s, the four `MRIdilate`s 2 s, each
`MRIlinearTransformInterp` (nearest) about 1.6 s (three calls): speed items for that module.
Memory (`tests/parity/test_memory_peak.py`, `MRI_CC_CASES`: seg2cc's call on ds000005's v8 head, one
thread): the port 328.7 MiB (`reproduce`) and 329.6 MiB (`fix`) against FreeSurfer's 344.5 MiB, the
aseg and the transform identical.
