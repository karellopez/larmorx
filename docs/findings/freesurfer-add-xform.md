# FreeSurfer 8.2.0's `mri_add_xform_to_header`: findings of the replica (queue item 3.2-10)

**Restricted** (source-derived): closed to FreeSurfer clean-room implementers, who work from
`specs/mri_add_xform_to_header.md`. Track A, relay 50 (2026-11-22). The replica:
`crates-freesurfer/larmorx-freesurfer-volumes/src/mri_add_xform_to_header/`; the probe:
`validation/probes/fs-add-xform/`; the record: `docs/validation/freesurfer-mri-add-xform-to-header.md`.

## The defects (rules A to D; `larmorx_core::defects`)

1. **FS-1154** (class a: crash; both modes noted, `reproduce` aborts). `main` copies the
   transform's name with `strcpy(mri->transform_fname, xform_fname)` (`mri_add_xform_to_header.cpp`
   144) into the `MRI`'s `char transform_fname[STRLEN]` (1,024 bytes). The official build has
   `_FORTIFY_SOURCE`: the copy is `__strcpy_chk`, which aborts for a name of 1,024 bytes or more
   (`*** buffer overflow detected ***: terminated`, `SIGABRT`, status 134 from a shell; measured
   with a 1,104-byte name, with and without `-c`, MGZ and MGH outputs: nothing written). Standard
   output, a pipe in the measurement, loses `INFO: extension is mgz` with `-c` (nothing flushed
   it); without `-c` the read's error message flushed it first. By default larmorx writes the whole
   name (the MGH tag holds any length); `reproduce` prints the line and ends with 134 (standard
   output as written: whether C's buffer survives depends on where it goes).
2. **FS-1155** (class c; rule A). `MRIwrite`'s and `MRIwriteInfo`'s results are ignored (151-157):
   `exit(0)` after a write that failed (an unknown extension: `unknown file type for file (<f>)`; a
   missing folder: the MGH writer's messages; `MRIwriteInfo`'s open). recon-all would go on with
   the volume unchanged. Corrected: status 1 after the writer's messages; `reproduce` 0.
3. **FS-1156** (class a: crash; as FS-1154). `MRIwriteInfo` (`utils/mriio.cpp` 11852) builds
   `sprintf(fname, "%s/%s", fpref, INFO_FNAME)` in `char fname[STRLEN]`: `__sprintf_chk` aborts
   when the folder's name has 1,014 bytes or more (measured: a folder 1,059 bytes deep aborts, one
   1,009 bytes deep writes its file). The second `sprintf` (the `xform` line, 11890) copies a name
   FS-1154 already bounds.
4. **FS-1157** (class c; rule A). `MRIwriteInfo` writes `thick` as `mri->ps / MM_PER_METER` (11871),
   the pixel size, where `corRead` reads `thick` into `mri->thick` and `zsize` (1662-1666:
   `mri->zsize = mri->thick`). A COR folder whose slices are 2 mm thick and pixels 1 mm
   (`thick 0.002`, `psiz 0.001`) is rewritten with `thick 0.001`: read back, its slices are 1 mm.
   Corrected: `mri->thick / 1000` (the field the line names, which `corRead` set from the folder);
   `reproduce` the pixel size. Measured on the official program (the probe's `cor2` cases: 59
   notices in seed 3, all identical with `reproduce`).
5. **FS-1158** (class c; rule A). `MRIwriteInfo` writes the `xform` line only `if
   (mri->linear_transform)` (11883), the pointer set when a transform was loaded. With `-c` the
   program never loads one, so the new name reaches the file only when the folder's own
   `COR-.info` named a transform file that `corRead` loaded (the comment above the line: "this
   won't work for relative paths ..."). Corrected: the line written whenever the name is not empty
   (as `corWrite` does: `strlen(mri->transform_fname) > 0`, 1879); a loaded transform with an empty
   name (`mri_add_xform_to_header "" cor out`) gives no line by default, FreeSurfer's `xform ` with
   `reproduce`.

## Behaviours kept (no defect)

- **A failed read goes on.** `input_transform_file` failing prints `ErrorPrintf(ERROR_NO_MEMORY,
  "%s: could not read xform file '%s'\n", ...)` (134) and the program writes the output anyway: the
  transform is only checked ("my guess is just to verify the validity of the transform?"), the
  name is what the program adds. Rule C (a design scripts may rely on: recon-all passes `-c` for a
  transform that does not exist yet). The message's own `\n` gives the line `\0\n` after it (FS-01).
- **The partial transform.** volume_io's `input_transform` assigns `*transform = next` at its first
  transform and the concatenation at each later one (`packages/minc/minc_volume_io.c` 1440-1452),
  and returns `ERROR` after a later failure: the caller's transform keeps what was read. The port's
  main-package reader returned nothing on a failure; `larmorx_io::xfm::input_transform_file_ex`
  now gives what C's argument holds and `errno` (relay 50: a change to the main package's MINC
  port, behaviour of MINC 1.5.1's own `gen_xf_io.c` as much as FreeSurfer's copy), and fs.io's
  `mghRead` tag reader and `corRead` keep that partial transform in the volume (their matrices
  cleared, as in C, 12304-12307 and 1722-1725). Visible only through `MRIwriteInfo`'s
  `linear_transform` test: `mri_add_xform_to_header` without `-c` takes the matrices from whatever
  transform the volume holds (`get_linear_transform_ptr(&mri->transform)`, 139-141).
- **A transform never read.** The `MRI`'s `General_transform` is zeroed by its allocation: type 0
  is `LINEAR`, its matrix pointers `NULL`, so `get_linear_transform_ptr` returns `NULL` without a
  message and `MRIwriteInfo` writes no `xform` line.
- **Concatenations stay linear.** FreeSurfer's reader reads only `Linear` transforms and
  `concat_general_transforms` folds two linear ones into one (`minc_volume_io.c` 1010-1030), so
  `get_linear_transform_ptr`'s `exit(1)` for other types is never reached from a file.
- **`errno`.** `ErrorPrintf` prints `strerror(errno)` first when it is not 0. After
  `input_transform_file`, `errno` is what volume_io's `fopen`s left: `file_exists` is an `fopen`
  (`minc_files.c` 129-143), `open_file_with_default_suffix` tries the name and, without an
  extension, the name with `.xfm`; `open_file` tries `.z`, `.Z`, `.gz` for a missing name; a
  folder opens and its first `fgetc` fails with `EISDIR`. Measured: a missing file prints `No such
  file or directory`; a folder `Is a directory`; a malformed file nothing (no call failed).
- **The extension.** `fio_extension` (`utils/fio.cpp` 728) takes the text after the last `.` of the
  whole name, folders included: `a.b/c` gives `b/c` (`currently only .mgz or .mgh ...`), `trail.`
  none (`no extension found`); `V.MGZ` is refused (`strcmp` with `mgz`).
- **`-n`.** `get_option`'s `N` returns 1 (an argument skipped, unused): an option without effect
  kept from older versions. Last on the line it leaves `argc` below 2, and the program says `no
  transform name specified` (no read past the arguments: the loop's `argc > 1` test comes first).
- **`ErrorInit`** is `FSinit()` (`include/log.h` 80), as `DiagInit` calls it: nothing visible
  twice.

## The oracle-output cache (relay 50)

The cache keys an input file by its content and suffix (`oracle_cache/key.py`'s `Input`), not by
its name. Two inputs of the same bytes named `a.b/c` and `trail.` (suffix `""` both) had one key:
the probe's seed 3 was served the `a.b/c` run (`INFO: extension is b/c`) for `trail.`, whose own
run says `no extension found` (case 463, run again by hand). `oracle_cache.policy.path_text` now
keeps, for the programs of `EXTENSION_FROM_WHOLE_PATH` (`mri_add_xform_to_header`), the text from
each argument's last dot; seeds 1 and 3 were run again with the new keys (seed 1's hash unchanged,
seed 3 0 different). Other programs that read a format from a whole path (`mri_info`'s
`fio_extension` of its inputs) may want the same.

Relay 51 widened it: every program of larmorx-freesurfer reads its volumes through `MRIread`
(`mri_identify`: `strrchr(name, '.')` of the whole name, and for the extension `gz` a `strstr` of
`.mgh.gz` and `.nii.gz` anywhere in it) or its transforms through `LTAread`
(`TransformFileNameType`: the last `@` or `.`, the name cut at `#`, a name ending in `nii.gz`), so
`EXTENSION_FROM_WHOLE_PATH` lists them all (with track D's `mri_make_uchar` and the unit oracle's
`fsunit`), and `policy.whole_path_texts` keeps only what the key's suffix of the file's own name
does not already hold (the text after the last dot where that suffix does not end with it; a gzip
name the suffix does not hold): a run whose names the suffix decides keeps its old key. A name
without a dot (`noext`) keeps its key too, and its entry may have been stored by a run of another
name: relay 51's re-check ran every run with such an input fresh (`refresh`) and compared each
run's exit status, streams and files with the run served before
(`validation/investigations/oracle-cache/whole_path_recheck.py`; logs
`<scratch>/A/rl51/recheck/`): fs-add-xform seeds 1, 3 and the extra cases (27, 39 and 3 confusable
runs: `noext`, `a.b/c`, `trail.`) and fs-calc seeds 1 and 2 (one each: an input `2`) give the
same summaries and official hashes (`127e7633...`, `98de724e...`, `b5eebbd1...`; 223 of 228
identical); the only raw differences, 24 runs' standard error, are FS-167 cases (FreeSurfer
prints memory never written: `SKIP`). The other probes (fs-info seeds 1, 3, golden and its
replay; fs-binarize, fs-segstats, fs-normalize seeds 1, 2 and golden; fs-convert's golden; the
golden cases of fs-calc and fs-add-xform) have no confusable run: their keys do not change.
