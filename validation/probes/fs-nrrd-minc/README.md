# fs-nrrd-minc: NrrdIO's NRRD reader and MINC's volume input and output

The check behind `larmorx_io::nrrd` (NrrdIO r3991's `nrrdLoad`, relay 26) and `larmorx_io::minc` (MINC
1.5.1's volume input over netCDF 3.6.0-p1, relay 27, and its volume output, relay 28) for FreeSurfer's
foundations, track C. Findings: `docs/findings/nrrdio.md`
(the upstream) and `docs/findings/freesurfer-permissive.md` (FreeSurfer's copies; this probe
compiles against FreeSurfer's `packages/` headers and is closed to FreeSurfer's clean-room
implementers as that file is).

```
bash build.sh <out>    # needs the unit oracle (oracles/freesurfer-8.2.0-unit), tags/NrrdIO-r3991,
                       # and for MINC tags/minc-1.5.1 and tags/netcdf-3.6.0-p1 (build_minc.sh);
                       # GCC 8.4.0 from the unit oracle
<out>/fs-nrrd-minc <out> [count] [seed] [--only nrrd|minc|write]
<out>/fs-nrrd-minc <out> 1 1 --only minc --files <dir>   # MINC 1.5.1's dumps of <dir>'s files hashed
LXPROBE_ONE=<file> <out>/fs-nrrd-minc <out> 1 1 --only minc   # one file, every output printed
<out>/fs-nrrd-minc <out> 1 1 --only write --write-cases <dir>  # MINC 1.5.1's runs of exported write cases hashed
```

- `libnrrd_up.so`: NrrdIO r3991's files built as FreeSurfer builds its copy (`TEEM_DIO=0
  TEEM_QNANHIBIT=1 TEEM_32BIT=0 TEEM_ENDIAN=1234`, no `TEEM_ZLIB`, no `TEEM_BZIP2`; GCC 8.4.0,
  `-O3 -DNDEBUG -fopenmp -w -fPIC`), with `shim_nrrd.c` and an empty `up-include/itk_NrrdIO_mangle.h`
  (NrrdIO r3991 includes ITK's mangling header; FreeSurfer's copy drops the include).
- `libnrrd_fs.so`: FreeSurfer 8.2.0's `libnrrdio.a` from the unit oracle's build, with `shim_nrrd.c`
  compiled against its header.
- `shim_nrrd.c` runs `nrrdLoad` in a forked child and writes the whole `Nrrd` (every field, every
  double's bits, the data, comments, key/values) and biff's messages as a dump on a pipe, with the
  child's standard error; the parent classifies a child that exits, dies of a signal or runs past
  the time limit (300 ms for a case the port says never returns, 5 s otherwise).
- The driver (`src/nrrd.rs`) generates files of three kinds: random headers (every field in valid
  and broken spellings and values, the five encodings, attached and detached data, templates,
  lists, skips, `\r\n`, NUL bytes), well-formed files (every type, every encoding NrrdIO has, space
  information, kinds sized to their axes, detached pieces), and cases aimed at the undefined paths
  (long messages, 17 sizes, template conversions, long ASCII values, sizes past 2^64, long lines).
  The port reads each with `reproduce`: its dump must equal both originals'; a case where the port
  notes a defect whose original behaviour is undefined (FS-761, FS-762, FS-765 to FS-768, FS-771) is
  not compared, and the originals' outcomes there are counted. Generated sizes stay small: NrrdIO
  clears its whole array before reading, so a header announcing gigabytes makes the original's
  memory, not its result, the measure.
- `LXPROBE_SHOW=<n>` prints up to n differences; a differing case's files are kept in
  `<out>/work-<seed>/keep-<seed>-<case>`.

Prints the counts and `VERIFY-PASS` or `VERIFY-FAIL` (exit status 1). Run inside a memory box
(`scripts/orchestration/boxed.sh 8 ...`); single-threaded, each case two forks per original.

## Results (2026-11-14, relay 26)

| seed | cases | comparisons identical | of them successful reads | differ | not compared (undefined) |
|---|---|---|---|---|---|
| 7 | 1,500 (first generator) | 2,880 | | 0 | 120 |
| 13 | 4,000 (with the undefined cases) | 7,352 | 196 | 0 | 648 |
| 17 | 4,000 (with the well-formed files) | 7,402 | 1,138 | 0 | 598 |
| 101 | 20,000 | 36,926 | 6,070 | 0 | 3,074 |
| 202 | 20,000 | 36,992 | 6,250 | 0 | 3,008 |
| 303 | 20,000 | 36,918 | 6,266 | 0 | 3,082 |

(Each case is compared with both originals; seeds 101 to 303 ran three at a time in a 12 GiB box,
logs `<workspace>/oracles/logs/fs-found-c-r26-nrrd-seed*.log`.)

On the undefined cases both originals behaved alike: FS-761 never returned; FS-762, FS-768 and all
but one FS-767 case aborted (signal 6: FreeSurfer's build has `_FORTIFY_SOURCE` and the stack
protector); FS-771 crashed (signal 11); FS-765 and FS-766 returned.

## MINC (relay 27, 2026-11-15)

- `libminc_up.so` (`build_minc.sh`): MINC 1.5.1's `libsrc` and `volume_io` (`<workspace>/tags/minc-1.5.1`)
  over netCDF 3.6.0-p1's `libsrc` (`<workspace>/tags/netcdf-3.6.0-p1`), with `up-include-minc/config.h`
  and `up-include-netcdf/ncconfig.h` (the two configure scripts' answers for Linux x86-64), GCC 8.4.0 and
  FreeSurfer's flags; `libminc_fs.so`: FreeSurfer 8.2.0's `libminc.a` and `libnetcdf.a` from the unit
  oracle, with `shim_minc.c` compiled against its `packages/minc` header.
- `shim_minc.c` runs FreeSurfer's `mincRead` sequence in a forked child (`start_volume_input` with the
  names `xspace, yspace, zspace, time`, the file's own type, no voxel range; `input_more_of_volume` until
  done; every `get_volume_voxel_value`) and dumps every field of the volume (type, sign, names, sizes,
  separations, starts, cosines, spatial axes, voxel and real ranges, scale, space, the voxel-to-world
  matrix, two world points, the reading fractions, every voxel's double bits); its child closes its
  descriptors from 3 up so that netCDF's ids (descriptors, printed in its messages) start at 3, and can
  set `VOLUME_CACHE_THRESHOLD`.
- The driver (`src/minc.rs`) writes netCDF classic files (its own writer): well-formed MINC files of
  every type, sign, valid range form, `image-max`/`image-min` layout (absent, scalar, per slice, per two
  dimensions, over a later dimension; NaN, equal and reversed ranges), geometry (zero and negative steps,
  short and zero cosines, space types), dimension orders, time as a record dimension (none and irregular
  spacing included), versions 1 and 2, truncated files; the same with header bytes changed or cut; and
  the special cases (empty, not netCDF, HDF5, gzip, a directory, a missing file, char and unknown image
  types, long and empty names, attributes of unknown types, global attributes standing for missing
  dimension variables, `image-max` over a foreign dimension, numeric strings). A header announcing more
  than 2^22 values is not run (the originals would allocate and read it whole). The port reads each with
  `reproduce` under MINC 1.5.1's dialect (`Mni`; one case in ten with the cache forced on, as the
  original's child) and under `FreeSurferDialect` (FreeSurfer's copy as measured:
  `docs/findings/freesurfer-permissive.md`); dumps, standard output, standard error and the exit status
  must equal each original's. A case where the port notes a defect of undefined behaviour (FS-774,
  FS-776 to FS-779, FS-782 to FS-785, FS-789, FS-790, FS-792, FS-795, FS-796; FS-781 for MINC 1.5.1) is
  counted, not compared; so are, for FreeSurfer, a voxel-to-world matrix with entries that are not finite
  (the probe's stand-in for FreeSurfer's 4 x 4 inverse does not model them) and, for MINC 1.5.1 with the
  cache, a short file's reads past its end (the cache reads blocks in the caller's order).

| seed | cases | comparisons identical | of them successful reads | differ | not compared (undefined) |
|---|---|---|---|---|---|
| 4 | 3,000 | 4,214 | 1,898 | 0 | 1,758 |
| 5 | 4,000 | 6,204 | 3,110 | 1 (FreeSurfer's line 1328, since added) | 1,759 |
| 6 | 4,000 | 6,209 | 3,121 | 2 (FS-794, since added) | 1,771 |
| 7 | 4,000 | 6,221 | 3,119 | 2 (FS-794, the cache with FS-793) | 1,745 |
| 8 | 4,000 | 6,220 | 3,012 | 2 (the shim's limit of 100,000 slabs: since counted apart) | 1,752 |
| 9 | 4,000 | 6,168 | 3,098 | 0 | 1,812 |
| 10 | 4,000 | 6,170 | 3,063 | 0 | 1,796 |
| 8, final code | 4,000 | 6,220 | 3,012 | 0 | 1,754 |
| 11, final code | 4,000 | 6,198 | 3,101 | 0 | 1,754 |

(Seeds 1 to 3, the first runs, found the differences fixed since: the shared cosine buffer, `nctypelen`'s
messages, the cache, raw-byte names, FreeSurfer's dialect. Logs
`<workspace>/oracles/logs/fs-found-c-r27-minc-seed*.log`, three seeds at a time in an 18 GiB box.)

The unit test `larmorx_io::minc::tests::seeded_files_as_minc` hashes the port's dumps of 120 seeded files;
`--files` on its exported files (`LARMORX_MINC_EXPORT=<dir>`) gives MINC 1.5.1's hash of its own dumps:
`c96915507a07c679e24d29c32397d68ce1ddcc485b7997b6605062d72286de47`, the test's constant.

## MINC output (relay 28, 2026-11-15)

- `shim_minc.c`'s `fm_minc_write` runs a write case in a forked child (`WriteCase`, the same layout as
  `src/ffi.rs`'s `WriteCaseC`): FreeSurfer 8.2.0's `mincWrite` sequence with every argument given
  (`create_volume`, optionally `set_volume_space_type` and `set_volume_real_range`, `set_volume_sizes`,
  `alloc_volume_data`, `set_volume_separations`, `set_volume_direction_cosine` or `_unit_cosine` per listed
  axis, `set_volume_translation`, `set_volume_voxel_value` of every voxel in C order, `output_volume`,
  `delete_volume`), its dump written line by line on the pipe (a later exit keeps the steps done). Its
  `fm_invert` calls the library's own `compute_transform_inverse` in the probe's process (its standard output
  captured): the FreeSurfer side's 4 x 4 inverse is FreeSurfer's own (the read probe uses it too since this
  relay; relay 27's stand-in modelled only its failure messages and left non-finite matrices uncompared).
- The driver (`src/minc_write.rs`) draws a case of mincWrite's shape six times in ten (the names `xspace,
  yspace, zspace[, time]`, its four types, the file's type and range the volume's, history ""), compared with
  MINC 1.5.1 and FreeSurfer's copy (`FreeSurferDialect`, as measured: FS-797, its exit after a zero-length
  cosine, its inverse), else any call (MINC 1.5.1 only): 0 to 6 dimensions, names from the standard ones,
  `foo`, empty and `NULL`, every type and sign (with `MI_ORIGINAL_TYPE` and char), voxel and file ranges
  ordinary, empty, reversed, not a number and huge, steps 0, negative, infinite, tiny, huge and not a number,
  cosines unit, oblique, zero, NaN and huge (and axes out of range), sizes up to 300,000 voxels, voxel values out
  of range, NaN and infinite, file names in a missing directory, a directory, under a file, empty, `.gz`,
  `$LXPROBE_W`, with blanks, over an older file; histories `NULL`, "" and a line; space types. Each case is run by
  the original, then by the port with `reproduce` (and with the default, for the notices' counts), each from the
  same setting; the step dumps, standard output, standard error, exit status and the file's bytes must be the
  original's. A case where the port notes a defect of undefined behaviour (FS-781, FS-789, FS-798 to FS-801) is
  counted, not compared.

| seed | cases | comparisons identical | of them files written | differ | not compared (undefined) |
|---|---|---|---|---|---|
| 3 | 2,000 | 2,912 | 1,843 | 0 | 281 |
| 4 | 2,000 | 2,862 | 1,796 | 0 | 314 |
| 5 | 2,000 | 2,878 | 1,778 | 0 | 309 |
| 6 | 2,000 | 2,941 | 1,844 | 0 | 294 |
| 7 | 2,000 | 2,925 | 1,816 | 0 | 274 |
| 9 | 2,000 | 2,915 | 1,864 | 0 | 266 |

(The final code. Seeds 1 and 2, the first runs, found what was fixed since: FreeSurfer's exit after a zero-length
cosine, its inverse's values (now called itself), the NaN bits of volume_io's object code, FS-797 to FS-801.
Logs `<workspace>/oracles/logs/fs-found-c-r28-write-seed*.log`. The read probe with FreeSurfer's own inverse:
seeds 8 and 11, 2,000 files each, 3,107 and 3,077 comparisons identical, 0 different,
`fs-found-c-r28-read-seed{8,11}.log`.)

The unit test `larmorx_io::minc::tests::seeded_volumes_written` writes 80 seeded cases with the port and hashes
the runs; `LARMORX_MINC_EXPORT_WRITE=<dir>` exports their descriptions and `--write-cases <dir>` gives MINC
1.5.1's hash of the same runs: `42ded1ed2f8e044f72ae866c86936f9aa2dc84014ccf97a4545418f664fae198`, the test's
constant.

