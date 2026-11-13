# MINC 1.5.1's `.xfm` reader and writer, and FreeSurfer 8.2.0's copy

**Source-derived from MINC 1.5.1's volume_io** (MNI's permissive notice, `LICENSES/LicenseRef-MNI.txt`):
closed to the implementer of a future clean room of these routines (porting guide §1). The
**FreeSurfer** sections describe FreeSurfer's copy by its behaviour only (the probe's black-box
runs of the unit oracle's `libminc.a`; FreeSurfer's `packages/minc` was not read). Found while
porting the FreeSurfer foundations' item 3.1-14 (`docs/freesurfer/queue.md`) into `larmorx-io`
(`xfm`) and `larmorx-core` (`cscan`), 2026-11-12/13. The check: `validation/probes/fs-minc-nifti`.

## The upstream

FreeSurfer's `packages/minc` (15 files) is MNI's volume_io reworked: its `minc_gaussian.c` says
"BASED ON minc-1.5.1"; 66 % of `minc_volume_io.c`'s and 75 % of `minc_files.c`'s non-blank lines
are lines of MINC 1.5.1's `volume_io` (the same share for libminc 2.1.12, whose volume_io is the
same code); 8 % of `minc_gaussian.c`'s (its 4 x 4 inverse is FreeSurfer's own). Counted without
reading FreeSurfer's files (numbers only). The port follows MINC 1.5.1's release tarball
(`<workspace>/tags/minc-1.5.1`, SHA-256 `0b41d5bb...`): `MNI_formats/gen_xf_io.c`, `gen_xfs.c`,
`mni_io.c`, `Geometry/transforms.c`, `inverse.c`, `gaussian.c`, `Prog_utils/files.c`, `string.c`,
`print.c`. What FreeSurfer reaches (the call graph's core view): `input_transform_file` and what it
calls, `get_linear_transform_ptr`, `get_inverse_linear_transform_ptr`, `copy_general_transform`,
`delete_general_transform`, `transform_point` (`utils/mriio.cpp`'s `talairach.xfm`, `utils/mri.cpp`,
`utils/label.cpp`, `utils/stats.cpp`, `mri_add_xform_to_header`); FreeSurfer writes `.xfm` files with
its own code (`utils/transform.cpp`, `registerio.cpp`: track A), so MINC's writer is ported for
larmorx's own use and checked against MINC 1.5.1 only.

## How volume_io reads an `.xfm` file (what the port keeps)

- Numbers are tokens ended by a space, `;` or a newline, **not a tab**: `1\t0 0 1` is one token,
  read by `sscanf("%lf")` as 1, and every later element shifts by one (FreeSurfer the same).
- `%` and `#` start a comment only where white space is skipped (before a token).
- A carriage return inside a token is dropped; a NUL byte adds nothing and the token goes on
  (FreeSurfer: a NUL ends the token, the rest of it is read and lost).
- `sscanf("%lf")` is glibc's: `1e` and `1e+` read 1, `0x` alone reads nothing, `0x.` reads 0,
  `nan(...)` reads the default NaN (`larmorx_core::cscan`, 400,000 strings of each kind against
  glibc: 0 differences).
- A singular matrix gives the identity for the transform and its inverse, silently
  (`create_linear_transform`); FreeSurfer prints `invert_4x4_matrix failed, Some col is all zeros`.
- Two linear transforms in a row are multiplied into one (`concat_general_transforms` "crunches"
  them), their inverses multiplied in the other order (not the inverse of the product).
- `Invert_Flag = True` swaps the matrix and its inverse; the result's flag is clear.
- `print_error` formats into a C string, so a message ends at a NUL byte (`Expected ';', found '`
  for a NUL); FreeSurfer prints the NUL.
- A name without an extension that does not exist is tried with `.xfm`; `~`, `~user` and `$VAR`
  are expanded (twice and three times over: `open_file_with_default_suffix` expands, `open_file`
  expands again, `has_no_extension` once more).
- On x86-64 `char` is signed, so `ungetc` of the byte 0xFF is `ungetc(EOF)`, which pushes nothing.

## Defects (D13; register entries FS-71 to FS-75)

| ID | What volume_io does | larmorx |
|---|---|---|
| FS-71 | a compressed input (`.gz`, `.Z`, `.z`, or a missing name whose compressed file exists) is decompressed by `system("gunzip -c <name> > <tmp>")`, then `bunzip2 -c`, the name unquoted: a blank or shell syntax fails, reads other files or runs commands | gzip and bzip2 decompressed in memory; `compress`'s `.Z` and `pack`'s `.z` are not decoded (an error, as an undecodable file); a notice for a name the shell would split |
| FS-72 | a thin-plate spline's `Points` or `Displacements` list that reaches the end of the file after white space without its `;` is read forever (the same pushed-back character), the list growing until memory runs out | stops; `Expected ';', found end of file.` |
| FS-73 | `Number_Dimensions = 0` divides by zero (a crash); a negative one sizes arrays negatively | an error message |
| FS-74 | an empty `Points` list frees a pointer never set; the writer writes such a list without its `;` | a spline with no points (its affine part); the writer writes the `;` |
| FS-75 | a name that starts with a `$VAR` (or `~`) whose value is empty writes through a null pointer | expands to nothing |

None is reproduced (crashes, hangs and memory contents never are, CLAUDE.md D13); each gives a
notice. In the probe: 390 of 20,000 generated files and 327 of 40,000 names reach them (MINC is
not run there).

## FreeSurfer 8.2.0's changes (by behaviour; for `larmorx-freesurfer`, track A)

Measured with `ProbeFs` (`validation/probes/fs-minc-nifti/src/dialect.rs`), which makes the port
give FreeSurfer's results exactly (20,000 files, 40,000 names: 0 differences):

1. Only `Linear` transforms: any other type is `input_transform(): invalid transform type <name>.`
   (MINC: `input_transform(): invalid transform type.` for an unknown one; thin-plate splines and
   grids read).
2. A number that does not convert prints `<token> is not a valid double` first.
3. A mismatched keyword prints `Expected "<keyword> =" got str:<token>` (`(null)` at the end of
   the file) wherever MINC prints `Expected "<keyword> ="` or nothing.
4. A NUL byte ends a token; messages are printed with their NUL bytes.
5. `~user` is always `.` (volume_io built without `getpwnam`; MINC: the user's home directory).
6. Its own 4 x 4 inverse (`invert_4x4_matrix`; singular: its message and the identity). It gives
   MINC's Gaussian elimination's bits for all 16 elements in 1,045 of 12,406 affine matrices
   (last row 0 0 0 1), 6,095 of 11,205 small-integer ones, 640 of 21,950 general ones: a replica
   of FreeSurfer's results needs FreeSurfer's routine.
7. A compressed `.xfm` crashes (a double free after `gunzip`), and with `TMPDIR` unset its
   temporary file goes to `/` (`Unable to create temporary file /minc-XXXXXX`).
8. An empty `$VAR` at the start of a name expands to nothing (no crash: FS-75 is MINC's).

## The numbers (the probe at scale 1, 2026-11-13, from 78828906's code; the later commits change
only formatting, documents and lint forms)

| Family | Cases | Against MINC 1.5.1 | Against FreeSurfer 8.2.0 |
|---|---|---|---|
| `scan` (`cscan` against glibc) | 400,000 strings each for `%lf`, `strtod`, `%d` | 0 differences (glibc) | – |
| `xfm` (generated and mutated files) | 20,000 | 0 differences (390 not run: FS-72 to FS-74) | 0 differences (`ProbeFs`) |
| `ops`: `compute_transform_inverse` | 50,000 matrices | 0 | the measurement above (FreeSurfer's own inverse) |
| `ops`: `concat_transforms`, `transform_point` | 100,000 each | 0 | 0 |
| `ops`: `concat_general_transforms` of two files (inverse flags) | 4,000 | 0 | – |
| `ops`: `expand_filename` | 39,673 names | 0 (327 not run: FS-75) | 0 |
| `write` (`output_transform_file`), `general_transform_point` | 10,000, 9,845 | 0 | – (FreeSurfer has no MINC writer) |

A scale-5 run was stopped after its `xfm` family (the machine was saturated by other parts' runs:
three hours for what takes about 30 minutes alone); its log, `oracles/logs/fs-found-c-minc-nifti-
probe-5.log`, has no counts.

## Not ported (gaps, each reported)

The displacement volume of a `Grid_Transform` (a MINC file: reading `Dialect::read_grid_volume`,
writing `Dialect::write_grid_volume` fail with a message), applying thin-plate splines and grids
(`general_transform_point` returns the type), user transforms (no file form), and `abort_if_allowed`'s
wait for an answer when standard input has ended (larmorx takes the end as `n`).
