# FreeSurfer 8.2.0's `mri_info`: findings of the replica (queue item 3.2-08)

**Restricted** (source-derived): closed to FreeSurfer clean-room implementers, who work from
`specs/mri_info.md`. Track A, relay 49 (2026-11-22). The replica: `crates-freesurfer/
larmorx-freesurfer-volumes/src/mri_info/`; the probe: `validation/probes/fs-info/`; the record:
`docs/validation/freesurfer-mri-info.md`.

## The defects (rules A to D; `larmorx_core::defects`)

1. **FS-1146** (class e; rule A). `parse_commandline` sets `PrintCRes` for `--xsize`, and also for
   `--ysize` and `--zsize` (`mri_info.cpp` 261-272): all three print `xsize`. The usage lists
   `--cres`, `--rres` and `--sres` as the column, row and slice sizes; `--ysize` and `--zsize` are
   their aliases by name. Corrected: `--ysize` sets the row flag, `--zsize` the slice flag. No
   script of FreeSurfer 8.2.0 passes them (`scripts/`, `python/`).
2. **FS-1147** (class e; rule A). `-it`/`--in_type` sets `intype` with `string_to_type`, but
   `do_file` starts with `intype = mri_identify(fname)` (646): the option never reaches the read.
   Corrected: the given type (when it names one) replaces the identified one for the read:
   `MRIreadHeader(fname, type)`, or `MRIread` of `<fname>@<type>` for the options that read the
   voxels (`MRIread`'s own way of naming the type, which keeps its NaN removal and wildcards). The
   `type:` line and `--format` still print the identified type (`mri_identify` called again, as
   FreeSurfer does). With `reproduce` the option only goes through `string_to_type` (its buffer
   check included).
3. **FS-1148** (class a: crash; rule A). The morph test of `do_file` (664) is `strstr` of `m3d`,
   `m3z`, `M3D` or `M3Z` in the extension, while `mri_identify` lower-cases it (`string_to_type`):
   a name like `warp.M3z` is identified as `MGH_MORPH`, so no header is read (`mri` stays `NULL`),
   and the morph report is skipped. `--format` still works (it prints `unknown`:
   `type_to_string(MGH_MORPH)` has no case for it); every other path dereferences `NULL`. The
   default report prints its first line (`fpout`) and, from the measurement with `strace`, nothing
   else reaches standard output when it is a pipe or file: `mri_identify` flushes standard output
   before the `type:` line, which is then lost with the crash. Corrected: a morph by its type is
   reported as a morph. `reproduce`: the lines printed before the crash (the default report's
   first two; the tag report's three and its empty file), then an error and status 1.
4. **FS-1149** (class a: memory; both modes every input). `inputlist[100]` has no bound check
   (491). Measured on the official binary (relay 49): 101 to 129 inputs give the right answers;
   at 130 the program also loads each talairach transform (`INFO: loaded talairach xform ...` on
   standard error: an overwritten flag of the library); at 200 `Gdiag` has `DIAG_INFO` set (the
   tag reader's `[DEBUG]` lines, nine output lines per input); at 1,000 it crashes (status 139).
   larmorx reports every input in both modes and notes the case.
5. **FS-1150** (class a: crash). `--otags` and `-it` read `pargv[0]`, `argv[argc]` (a null
   pointer) when they are the last argument: `-it` crashes at once in `string_to_type`'s
   `strlen(NULL)`; `--otags` crashes when the tag report is reached (`strlen(outtagfile)`), after
   printing `save TAG data as (null)` (glibc's `%s` of a null pointer). `--o` as the last argument
   gives `fopen(NULL, "w")`, which glibc fails (`EFAULT`): `ERROR: could not open (null)`, status 1,
   reproduced in both modes (no defect). Both modes: an error, status 1, noted.
6. **FS-1151** (class f; rule B). `__printTagsToFile` never checks `znzopen`'s result: with a file
   that cannot be created `MRITAGwrite(mri, NULL)` writes nothing (every znzlib call returns 0 on a
   null file), the colour table's writer prints `CTABwriteIntoBinary: fp was NULL` (its
   `ErrorReturn`, after `error: <errno of the failed open>`), and the report follows. Corrected:
   `ERROR: could not open <file>`, status 1. `reproduce`: FreeSurfer's lines (the writer's
   `saving original ras2vox` and the colour table's error); the tag writer's `[DEBUG]` lines with
   `--diag-debug` would print the positions of a file that is not there (`znztell` of `NULL`, -1):
   not printed (the probe never combines the two).
7. **FS-1152** (class a: crash). `--ras2vox` (`MatrixInverse(m, NULL)`) and `--ras2vox-tkr`
   (`MatrixInverse(m, m)`, whose `NULL` result replaces `m`) print the inverse's elements without a
   check: a matrix that cannot be inverted (a voxel size of 0) crashes the program. `--tkr2scanner`
   and `--scanner2tkr` end in `MatrixMultiply`'s own error instead (`m2 is null`, reproduced); the
   report prints `mat = NULL!` for its inverse (reproduced). Both modes: an error, status 1.
8. **FS-1153** (class c; rule A). `--stats` divides each frame's sum by `d*h*w` in `int`, which
   wraps for more than 2,147,483,647 voxels a frame (GCC's `imul`). Corrected: the count in 64
   bits. Not reachable by the probe (a 2 GiB volume); the code's two paths are the same expression
   otherwise.

## Behaviour worth knowing

- **The locale and `errno`.** `DiagInit` calls `FSinit`, whose `setLocale` tries
  `setlocale(LC_NUMERIC, "en_US")` first: on a machine whose `en_US` locale exists only as
  `en_US.utf8` (this one: glibc opens `/usr/lib/locale/en_US/LC_NUMERIC` and `en/LC_NUMERIC`, both
  missing), the first call fails with `ENOENT` and the second succeeds without resetting `errno`.
  Every later `ErrorExit`/`ErrorPrintf` that finds `errno` still 0 in larmorx then prints
  `error: No such file or directory` first in FreeSurfer (for `mri_info noext`: before `could not
  parse extension from noext`). This is a fact of the machine (its installed locales), as the
  locale warning `docs/findings/freesurfer-core.md` describes; not reproduced. The probe runs the
  official program with `FS_DISABLE_LANG=1`, which skips `setLocale` (numbers are formatted the
  same: the C locale's `LC_NUMERIC`).
- **The licence check prints the licence file under `DIAG_NO`.** `chklc`, which `MRIread` calls,
  prints the licence file's lines when `Gdiag_no` is above 1 (`DIAG_NO=2`). The probe never runs
  a volume read with `DIAG_NO` set (its one `DIAG_NO` case ends before any read; `check.py`
  refuses any other). Relay 49's first run held three such cases: their cache entries and the
  blobs of their output were removed the same day, before anything else read them.
- **Standard output and `--o`'s file.** The print flags write to `fpout`; the full report writes
  only its first line there (the rest with `printf`), and so do the morph report's first block,
  `--cmds` (`printf`), the tag report (`printf`, its matrices with `MatrixPrintFmt(fpout, ...)`),
  and the warp report (`printf`). `vgprint` writes to standard output and flushes it.
- **The order of the print flags** is `do_file`'s: `--format` (after the morph test), `--zero-cras`
  (applied), `--dump`, then `--tr`, `--te`, `--fa`, `--fad` (each printed, no return), `--is-1mm-iso`,
  `--conformed`, `--conformed-to-min`, `--type` (each returns), `--ti` (printed), a return when any
  of `--tr`, `--te`, `--fa`, `--fad`, `--type`, `--ti` was given; then the single-answer flags in
  source order. `--version` is never reached as an option (`handleVersionOption` takes it first)
  unless its case differs (`--Version`: the version and status 1).
- **The report's `talairach xfm` line** is printed for a folder (COR) or a name whose extension
  is exactly `mgz` or `mgh` (case-sensitive `strcmp`): not for `.nii`, `.img`, nor `.MGZ`.
- **`--dump`'s `Precision`** has no line end for a type outside its list (`float complex`,
  `bitmap`, `rgb`); `--type` prints nothing for those but `float complex`.
- **A COR volume's transform name** holds its folder's absolute name (`mriio`'s COR reader), so
  its tag file (`--otags`) depends on the folder's path length; the probe leaves that pair out.
- **The oracle-output cache and a recorded working directory.** The replay (`replay.py`) runs the
  official program in the recording's `mri/` folder; the cache stores the output with that folder
  replaced by its working-directory token, and the report's `talairach xfm` line holds the folder
  (the header's absolute name). A later run of the same command from another folder (the memory
  test, `tests/parity/test_memory_peak.py::test_mri_info_replica_needs_no_more_memory_than_freesurfer`)
  was recorded as a dispute (`oracles/cache/disputes/0516b1f6.../`), a false alarm of the token:
  reported to the orchestrator.
