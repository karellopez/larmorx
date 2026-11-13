# MINC 1.5.1's `.xfm` reader and writer

**Source-derived from MINC 1.5.1's volume_io** (MNI's permissive notice, `LICENSES/LicenseRef-MNI.txt`):
closed to the implementer of a future clean room of these routines (porting guide §1). Found while
porting item 3.1-14 of larmorx's FreeSurfer foundations into `larmorx-io` (`xfm`) and `larmorx-core`
(`cscan`), 2026-11-12/13. The check: `validation/probes/fs-minc-nifti`. FreeSurfer's copy of these
routines (what it reaches, how it differs, measured as a black box):
`docs/findings/freesurfer-permissive.md` (closed to FreeSurfer's clean-room implementers; this file
describes MINC 1.5.1 only, 2026-11-13).

## The upstream

The port follows MINC 1.5.1's release tarball (`<workspace>/tags/minc-1.5.1`, SHA-256
`0b41d5bb...`): `MNI_formats/gen_xf_io.c`, `gen_xfs.c`, `mni_io.c`, `Geometry/transforms.c`,
`inverse.c`, `gaussian.c`, `Prog_utils/files.c`, `string.c`, `print.c` (libminc 2.1.12's volume_io
is the same code). The writer is ported for larmorx's own use and checked against MINC 1.5.1.

## How volume_io reads an `.xfm` file (what the port keeps)

- Numbers are tokens ended by a space, `;` or a newline, **not a tab**: `1\t0 0 1` is one token,
  read by `sscanf("%lf")` as 1, and every later element shifts by one.
- `%` and `#` start a comment only where white space is skipped (before a token).
- A carriage return inside a token is dropped; a NUL byte adds nothing and the token goes on.
- `sscanf("%lf")` is glibc's: `1e` and `1e+` read 1, `0x` alone reads nothing, `0x.` reads 0,
  `nan(...)` reads the default NaN (`larmorx_core::cscan`, 400,000 strings of each kind against
  glibc: 0 differences).
- A singular matrix gives the identity for the transform and its inverse, silently
  (`create_linear_transform`).
- Two linear transforms in a row are multiplied into one (`concat_general_transforms` "crunches"
  them), their inverses multiplied in the other order (not the inverse of the product).
- `Invert_Flag = True` swaps the matrix and its inverse; the result's flag is clear.
- `print_error` formats into a C string, so a message ends at a NUL byte (`Expected ';', found '`
  for a NUL).
- A name without an extension that does not exist is tried with `.xfm`; `~`, `~user` and `$VAR`
  are expanded (twice and three times over: `open_file_with_default_suffix` expands, `open_file`
  expands again, `has_no_extension` once more).
- On x86-64 `char` is signed, so `ungetc` of the byte 0xFF is `ungetc(EOF)`, which pushes nothing.

## Defects (D13; register entries FS-71 to FS-75, FS-79)

| ID | What volume_io does | larmorx |
|---|---|---|
| FS-71 | a compressed input (`.gz`, `.Z`, `.z`, or a missing name whose compressed file exists) is decompressed by `system("gunzip -c <name> > <tmp>")`, then `bunzip2 -c`, the name unquoted: a blank or shell syntax fails, reads other files or runs commands | gzip and bzip2 decompressed in memory; `compress`'s `.Z` and `pack`'s `.z` are not decoded (an error, as an undecodable file); a notice for a name the shell would split |
| FS-72 | a thin-plate spline's `Points` or `Displacements` list that reaches the end of the file after white space without its `;` is read forever (the same pushed-back character), the list growing until memory runs out | stops; `Expected ';', found end of file.` |
| FS-73 | `Number_Dimensions = 0` divides by zero (a crash); a negative one sizes arrays negatively | an error message |
| FS-74 | an empty `Points` list frees a pointer never set; the writer writes such a list without its `;` | a spline with no points (its affine part); the writer writes the `;` |
| FS-75 | a name that starts with a `$VAR` (or `~`) whose value is empty writes through a null pointer | expands to nothing |
| FS-79 | `expand_filename` takes a backslash as an escape (it drops it and keeps the next character) and only `/` separates directories (`remove_directories_from_filename`, `extract_directory`, the default `.xfm` suffix's extension test, a grid volume's name, the writer's grid file name): a Windows path (`C:\Users\RUNNER~1\...`) loses every separator, so its directory, base name and extension are misread and the file next to it is not found | on Windows, a backslash separates directories as `/` does (never an escape; a `$NAME` also ends at it), and a name starting with `\` or a drive letter and `:` is absolute; elsewhere volume_io's rules; `reproduce` keeps the escape on Windows too |

FS-71 to FS-75 are not reproduced (crashes, hangs and memory contents never are, CLAUDE.md D13);
each gives a notice. FS-79 (rule A: a Windows path is a sensible input whose result is wrong)
gives a notice whenever a Windows name holds a backslash, in both modes. A `~` that does not start
the name (`RUNNER~1`, Windows' short names) is kept as written by volume_io and the port alike
(only a leading `~` names a home directory); blanks are ordinary characters. Found by CI's Windows
jobs on main f43acf5b (GitHub's runners' temporary folders are under `C:\Users\RUNNER~1\`); the
test `files_compressed_defaults_and_dialects` now reads and writes in a folder named
`RUNNER~1 a b.*`, and `expansion_of_names` checks the Windows rules on every platform. In the probe: 390 of 20,000 generated files and 327 of 40,000 names reach them (MINC is
not run there).

## The numbers (the probe at scale 1, 2026-11-13, from 78828906's code; the later commits change
only formatting, documents and lint forms)

| Family | Cases | Against MINC 1.5.1 |
|---|---|---|
| `scan` (`cscan` against glibc) | 400,000 strings each for `%lf`, `strtod`, `%d` | 0 differences (glibc) |
| `xfm` (generated and mutated files) | 20,000 | 0 differences (390 not run: FS-72 to FS-74) |
| `ops`: `compute_transform_inverse` | 50,000 matrices | 0 |
| `ops`: `concat_transforms`, `transform_point` | 100,000 each | 0 |
| `ops`: `concat_general_transforms` of two files (inverse flags) | 4,000 | 0 |
| `ops`: `expand_filename` | 39,673 names | 0 (327 not run: FS-75) |
| `write` (`output_transform_file`), `general_transform_point` | 10,000, 9,845 | 0 |

A scale-5 run was stopped after its `xfm` family (the machine was saturated by other parts' runs:
three hours for what takes about 30 minutes alone); its log, `oracles/logs/fs-found-c-minc-nifti-
probe-5.log`, has no counts.

## Not ported (gaps, each reported)

The displacement volume of a `Grid_Transform` (a MINC file: reading `Dialect::read_grid_volume`,
writing `Dialect::write_grid_volume` fail with a message), applying thin-plate splines and grids
(`general_transform_point` returns the type), user transforms (no file form), and `abort_if_allowed`'s
wait for an answer when standard input has ended (larmorx takes the end as `n`).
