# `mri_convert` (FreeSurfer 8.2.0): behaviour spec

Status: part 1 of 3 (queue items 3.2-04 and 3.2-05, first half: the stats tables, the CSV reader,
and the command line up to the checks before the input volume is read); part 2 (the steps from
reading the input to writing the output, 3.2-05's second half and 3.2-06) and part 3 (the program
assembled, its exit statuses after the checks, 3.2-07) follow. Written by the replica's author
(track A) as the spec writer: behaviour, conventions and messages, for the clean room
(`larmorx-fs-volumes`), which works from this file, the library modules' specs
(`specs/freesurfer-*.md`) and black-box runs of the official program only.

Conventions: messages go to standard output unless "standard error" is said; `%g`, `%f`, `%7.3lf`,
`%-33s` are C's formats (glibc's: `nan`, `-nan`, `inf`, `-inf`; a `NULL` string prints as
`(null)`); `<prog>` is the program's name as it was called, without its folders (`mri_convert`).
"Ends with status n" means the program stops there with that exit status. Numbers read "as
`strtod`" or "as `strtol`" follow the C library (glibc 2.35), read as a black box
(`larmorx_core::cscan` has both); a float option's value is the `double` `strtod` gives, rounded
once to a `float`.

## 1. The stats tables (`--in_stats_table`, `--out_stats_table`)

A stats table (the output of `asegstats2table` or `aparcstats2table`) has a first line of a measure
and the column names, then one line per row: the row's name and one number per column. Its volume
is a `float` volume of `ncols x 1 x 1` voxels and `nrows` frames: voxel `(c, 0, 0)` of frame `r` is
the number of row `r`, column `c` (converted from `double` to `float`); voxel sizes 1; direction
cosines the identity (x along R, y along A, z along S), the centre 0; the volume's cached
voxel-to-RAS matrices are those of a newly allocated volume (FreeSurfer's default orientation),
set before the cosines are replaced. A table with no column or no row gives the allocation's
answer for that size (`MRI::Shape::Shape(): volume size must be greater than 0 in every dimension`
and no volume, for a zero size).

### 1.1 Reading a table

The format is chosen by the name's extension (what follows its last `.`, compared without case):

1. It prints `LoadStatTable(): ext <extension>` (`(null)` for a name with no `.` or with the `.`
   last).
2. `.csv`: `Loading table as CSV`, the CSV reader (§1.3); `.tsv` or `.tac`: `Loading table as TSV
   or TAC`, the TSV reader (§1.4); any other extension: `Loading table as FS Table`, FreeSurfer's
   format (§1.2). A name with no extension is read in FreeSurfer's format (FS-1031, below).
3. A reader that fails prints its message and gives no table.

Defect FS-949: FreeSurfer reads a `.csv` table with the CSV reader and then again in FreeSurfer's
format (`Loading table as FS Table` and that reader's messages follow), keeping the second table: a
real CSV table then fails (its first line is one item) or gives another table. By default only the
CSV reader runs; with `reproduce` both, the second kept.

### 1.2 FreeSurfer's format

- The file cannot be opened: `ERROR: could not open <name>`, no table.
- The columns are the items of the first line less one: an item is a run of characters other than
  blank and tab; a newline or carriage return where an item would start ends the line (inside an
  item it is part of it). Fewer than one column: `ERROR: format:  <name>` (two blanks), no table.
  Then `Found <ncols> data colums`.
- The rows are the following lines: `Found <nrows> data rows`.
- The file is read again as words and numbers, across line ends (C's `fscanf` with `%s` and `%lf`:
  white space is skipped, a word is a run of non-white characters; `%lf` reads glibc's way, see
  `specs/freesurfer-core.md` on the C library's conversions): the first word is the measure, the
  next `ncols` words the column names; then for each row a word (its name) and `ncols` numbers. A
  number that cannot be read (not a number, or the end of the file): `ERROR: format: <name> at row
  <r>, col <c>` (0-based), no table. A word read at the end of the file keeps the previous word.
- FS-1032: an empty file (or one that cannot be read, such as a folder) has no first line;
  FreeSurfer counts the columns of memory contents. Both modes count none (`ERROR: format:`).
- FS-1033: FreeSurfer reads lines in pieces of at most 99,999 bytes: a blank line counts as a row
  (a table with a trailing blank line then fails at its last row: `ERROR: format: <name> at row
  <last>, col 0`), a longer line counts as one row per piece, and only the first piece of the
  first line is counted for the columns. By default a row is a line (read whole) that holds an
  item; with `reproduce` FreeSurfer's count. The words and numbers are read the same way either
  way.
- FS-1034: a word longer than 99,999 bytes overflows FreeSurfer's buffer; both modes read it whole.

### 1.3 The CSV reader (`include/csv.h`)

The file is read line by line (the newline removed; a last line without one is read; a file that
cannot be read gives no lines). An empty line is skipped. A line is split into fields:

- a comma outside quotes ends a field;
- a double quote anywhere opens a quoted part (`a"b,c",d` gives `ab,c` and `d`);
- inside a quoted part every character is kept, `""` gives one `"`, and a closing quote followed
  by anything but a comma or a quote ends the quoted part and drops that character (`"a"b` and
  `a"b,c"d` give `a` and `ab,c`);
- nothing else is removed (blanks and a carriage return stay in their field).

The first line gives the measure (its first field) and the column names (the others); each later
line a row: its name (first field) and its numbers (the next fields; more fields are ignored).
`Found <ncols> data colums`, `Found <nrows> data rows`. Each number is C++'s `std::stof` of its
field (up to a zero byte): the longest prefix `strtof` reads after white space, as a `float` (the
rest ignored: `1.5x` is 1.5, `0x` is 0, `nan(12)` a NaN with payload 12); no number read, or
`ERANGE` (beyond the `float` range, or an inexact result below the smallest normal `float` after
rounding, so `1e-40` and `1e39` fail): `ERROR: format: <name> at row <r>, col <c>`, no table.
Can't open: `ERROR: could not open <name>`.

- FS-1035: a file with no line: FreeSurfer crashes; both modes `ERROR: format:  <name>` and no
  table (and nothing more is read: no second reading of FS-949).
- FS-1036: a row with fewer fields than its numbers need: FreeSurfer reads past the row; both modes
  `ERROR: format: <name> at row <r>, col <c>` for the first missing field and no table (nothing
  more read).

### 1.4 The TSV reader (`.tsv`, `.tac`)

The table is FreeSurfer's TSV reader's (`specs/freesurfer-io.md`, `TSV::read`: column names from
the first line split at tabs, then numbers, the errors and FS-170 to FS-173 there). Then:

- a `.tac` table (extension compared without case) leaves out its first two columns (its times);
  `Found <ncols> data colums` (the columns kept, possibly 0 or less), `Found <nrows> data rows`;
- the measure is `kBq/cc` for a `.tac` table whose second column is named `end[kBq/cc]`, else
  `UnknownUnits`; every row is named `row`;
- FS-1037: a `.tac` table of one column: FreeSurfer reads the second column's name from memory
  contents (a crash, or a comparison of memory); both modes `ERROR: format:  <name>` after the two
  `Found` lines, no table;
- FS-950: FreeSurfer then writes the table's volume to `junk.mgz` in the working directory
  (replacing a file of that name). By default it is not written; with `reproduce` it is.

### 1.5 Writing a table (`--out_stats_table`)

The table is made from a volume and a template (the `--like` name):

1. Its numbers: one row per frame, one column per voxel along x (y and z 0), each the voxel's
   value as a `double`; the volume is copied into the table.
2. No template name, or an empty one: the measure and every name stay unset (printed `(null)`).
3. Otherwise the template is read: can't open: `ERROR: could not open <name>`, no table. Its
   columns as in §1.2 (the first line's items less one; FS-1032 for an empty file), `Found <n>
   data colums`; not the volume's width: `ERROR: Col numbers do not agree in MRI and:  <name>`, no
   table. Its rows as in §1.2 (FS-1033), `Found <n> data rows`; fewer than the frames: `ERROR: Not
   enough row headers for MRI in:  <name>`, no table; more: `WARNING: Too many row headers for MRI
   in:  <name>, will crop ...`. Then its words: the measure, the column names, and for each frame
   the first word of a line (white space and blank lines skipped before it), the rest of that line
   skipped (a word read at the end of the file keeps the buffer's last contents, which may be the
   rest of the previous line: e.g. ` 1 2 3` with its newline). FS-1034 for long words.
4. The table is printed into the output file: the measure with `%-33s ` (padded to 33 bytes, then
   a blank), each column name and a blank, a newline; then each row: its name with `%-33s `, each
   number with `%7.3lf ` (`%7.3lf` then a blank), a newline. A file that cannot be created: `ERROR:
   cannot open <name>` and the program ends with status 1.

## 2. The command line

`mri_convert [options] <in volume> <out volume>`.

### 2.1 The start

In this order: the library's diagnostics are read from the environment (`specs/freesurfer-core.md`:
`DIAG_NO`, `DIAG`, ...); the whole command line is printed, each argument followed by a blank (the
program's name first), then a newline. If any argument (the program's name included) is
`--debug`, the file `debug.gdb` is written in the working directory: `# source this file in gdb
to debug`, `file <program name> ` (a trailing blank), then `run ` followed by every argument but
the program's name and the `--debug` ones, each with a blank after it, and a newline (FS-1042:
where it cannot be created FreeSurfer crashes; both modes write nothing and go on). With no
argument the help is printed (§2.5) and the program ends with status 1. The version options
follow (`specs/freesurfer-core.md`, the version report: any argument starting with `--version` or
`-version`, and `--all-info`, `-all-info`): they are reported and removed from the arguments; when
nothing else is left the program ends with status 0.

### 2.2 How the arguments are read

Each argument is compared exactly (with its case) with the option names below; options may come
in any order and be repeated (the last wins). An argument that matches none: starting with `-`,
`\n<prog>: unknown flag "<argument>"\n` on standard error, the usage hint (§2.5), status 1;
otherwise it is the input volume's name while that is still empty, then the output's, and a third
ends the program: `\n<prog>: extra argument ("<argument>")\n` when it is the last argument, else
`\n<prog>: extra arguments ("<argument>" and following)\n` (standard error), the hint, status 1.
An empty argument fills an empty name with nothing.

The options' arguments are read by four rules (each error on standard error, then the usage hint,
then status 1):

- **integers** (`<n>`): the option needs `k` arguments after it, else `\n<prog>: argument <option>
  expects <k> integers; only <m> arguments after flag\n`; an empty one: `\n<prog>: argument to
  <option> flag is null\n`; each must be read whole by `strtol` in base 10 (white space before it
  and a sign allowed), else `\n<prog>: error converting "<arg>" to an integer for <option> flag,
  incorrect # of args (need <k>)?\n`; the `long` value cut to its low 32 bits.
- **numbers** (`<f>`): the same with `floats` for `integers` in the first message, the same null
  message, and `\n<prog>: error converting "<arg>" to a float for <option> flag: incorrect # of
  args? Need <k>\n`; read whole by `strtod`, the value as a `float`.
- **a name** (`<name>`): the option needs one argument after it, else `\n<prog>: argument <option>
  expects an extra argument; none found\n`. FS-1039: a name of 1,024 bytes or more (2,000 or more
  for `--new-transform-fname`) overflows FreeSurfer's buffer; both modes keep it whole. The same
  for the input and output names given without an option.
- **raw**: `--rescale`, `-sc`, `-osc` take the next argument as C's `atof` reads it (no check:
  `12x` is 12, `x` is 0); `--rescale-voxel` reads three as `sscanf("%d")`; the `-dcm2niix-*`
  options take the next argument as it is. FS-1038: FreeSurfer crashes when the command line ends
  before these arguments (for `--rescale-voxel`, before any of the three; for the `-dcm2niix-*`
  names only `-dcm2niix-opts`: the others then keep no value); both modes print the message of the
  matching rule (`... expects 1 floats; only 0 arguments after flag`, `... expects 3 integers;
  only <m> arguments after flag`, `... expects an extra argument; none found`), the hint, status 1.

### 2.3 The options

Names separated by `,` are the same option. "Sets" names what the conversion later uses (part 2).

| Option | Arguments | Sets / does | Message |
|---|---|---|---|
| `-version2` | | would end the program with status 97, but it starts with `-version` and is a version option (§2.1): never reached | |
| `-r`, `--reorder` | 3 `<n>` | reorder the axes | |
| `-r4`, `--reorder4` | 4 `<n>` | reorder the axes and frames | |
| `--shift` | 3 `<n>` | shift the axes | |
| `-ut`, `--upper_thresh` | `<f>` | an upper threshold | |
| `-oval`, `--outside_val` | `<n>` | the value outside the volume | `setting outside val to <n>` |
| `--no-dwi` | | the environment's `FS_LOAD_DWI=0` | |
| `--debug` | | debugging on | |
| `--left-right-reverse`, `--left-right-reverse-pix`, `--left-right-swap-label`, `--flip-cols`, `--slice-reverse` | | those flips | |
| `--left-right-mirror`, `--left-right-keep` | `<name>` | mirror or keep a half; the name must be `lh` or `rh` | else `ERROR: pass either 'lh' or 'rh' with <option>!` (standard output), status 1 |
| `--left-right-swap-label-table` | `<name> <name>` | two text matrices (read at once, `specs/freesurfer-core.md`'s text matrix reader, with its messages), the label swap on | |
| `--ascii`, `--ascii+crsf`, `--ascii-fcol` | | ASCII output 1, 2, 3; the output's format counts as forced | |
| `--in_stats_table`, `--out_stats_table` | | the input, the output is a stats table (§1) | |
| `--invert_contrast` | `<f>` | the contrast inversion value (default -1: none) | |
| `-i`, `--input_volume` | `<name>` | the input's name | |
| `-o`, `--output_volume` | `<name>` | the output's name | |
| `-c`, `--conform` | | conform | |
| `--conform-dc` | | conform, keeping the direction cosines | |
| `--cw256` | | conform to a width of 256 | |
| `-cm`, `--conform_min` | | conform to the smallest voxel size | |
| `-cs`, `--conform_size` | `<f>` | conform to that voxel size (default 1) | |
| `--delete-cmds` | | delete the command lines | |
| `--new-transform-fname` | `<name>` | the transform's new name | |
| `--sphinx` | | the sphinx position | |
| `--no-rescale-dicom` | | `FS_RESCALE_DICOM=0` | |
| `--bvec-scanner`, `--bvec-voxel` | | `FS_DESIRED_BVEC_SPACE=1`, `=2` | |
| `--no-analyze-rescale` | | `FS_ANALYZE_NO_RESCALE=1` | |
| `--autoalign` | `<name>` | an auto-alignment matrix (text matrix, read at once) | `Auto Align Matrix`, then the matrix printed (`specs/freesurfer-core.md`'s matrix print, 5 decimals; an unread one prints `MatrixPrint: mat = NULL!` as an error) |
| `-nc`, `--nochange` | | no change | |
| `-so`, `--store_orig_ras2vox` | | keep the original RAS-to-voxel matrix | |
| `-po`, `--parse_only` | | the parse-only report (§2.4) | |
| `-ii`, `-oi`, `-ti`, `-is`, `-os`, `-im`, `-om` (and `--in_info`, `--out_info`, `--template_info`, `--in_stats`, `--out_stats`, `--in_matrix`, `--out_matrix`) | | those reports | |
| `-ro`, `--read_only`, `-nw`, `--no_write` | | read only, write nothing | |
| `--force_ras_good`, `--split` | | those | |
| `-at`, `--apply_transform`, `-T` | `<name>` | a transform, not inverted | |
| `-ait`, `--apply_inverse_transform` | `<name>` | a transform, inverted; the file must open for reading (a folder does) | else `ERROR: cannot find transform file <name>` (standard error), status 1 |
| `--like` | `<name>` | the template volume (and the stats table's template) | |
| `--crop`, `--cropsize` | 3 `<n>` | crop on, its centre (default 128 128 128), its size (256 256 256) | |
| `--slice-crop` | `<n> <n>` | crop slices; start above stop | `ERROR: s_start > s_end` (standard error), status 1 |
| `--devolvexfm` | `<name>` | the subject for devolving | |
| `--upsample` | `<n>` | upsample by that factor | |
| `-iis`, `-ijs`, `-iks`, `-ois`, `-ojs`, `-oks` (`--in_i_size`, ...) | `<f>` | an input or output voxel size | |
| `-iid`, `-ijd`, `-ikd`, `-oid`, `-ojd`, `-okd` (`--in_i_direction`, ...) | 3 `<f>` | a direction (normalised, below) | |
| `--ctab` | `<name>` | the colour table (`remove`: remove it) | |
| `--no-ctab`, `--remove-ctab` | | the colour table name `remove` | |
| `-io`, `--in_orientation`, `-oo`, `--out_orientation` | `<name>` | an orientation string, checked (`specs/freesurfer-volume.md`, the orientation string check) | a bad one: `ERROR: with in orientation string <s>` (or `ERROR: with out_orientation string <s>`), the check's text and a newline (standard output), status 1 |
| `-ic`, `-dic`, `-oc` (`--in_center`, `--delta_in_center`, `--out_center`) | 3 `<f>` | those centres | |
| `-oni`, `-oic`, `--out_i_count`, `-onj`, `-ojc`, `--out_j_count`, `-onk`, `-okc`, `--out_k_count` | `<n>` | the output's extent | |
| `-ini`, `-iic`, `--in_i_count`, `-inj`, `-ijc`, `--in_j_count`, `-ink`, `-ikc`, `--in_k_count` | `<n>` | the input's extent | |
| `-vs`, `--voxsize`, `-voxsize` | 3 `<f>` | the output voxel size | |
| `-ds`, `--downsample`, `-downsample`; `-dsold`, `--downsampleold`, `-downsampleold` | 3 `<f>` | the downsampling factors (the same three for both) | |
| `-ds2`, `--downsample2` | | downsample by 2 | |
| `--antialias` | | the anti-aliasing filter | |
| `--nthreads`, `--threads` | `<n>` | the thread count | `\nSet OPEN MP NUM threads to <n>` |
| `--fwhm` | `<f>` | the smoothing FWHM and its sigma, `fwhm / sqrt(log(256))` (in `double`, stored as a `float`) | `fwhm = %g, gstd = %g` |
| `--reduce` | `<n>` | the reductions | `reducing input image <n> times` |
| `-tr`, `-TI`, `-te`, `-flip_angle` | `<f>` | those parameters | |
| `-odt`, `--out_data_type` | `<name>` | lower-cased: `uchar`, `short`, `int`, `float`, `rgb` | another: `\n<prog>: unknown data type "<name as given>"\n` (standard error), the hint, status 1 |
| `--bfile-little-endian` | | `BFILE_LITTLE_ENDIAN=1` | |
| `--rescale`, `-sc`, `--scale`, `-osc`, `--out-scale` | `atof` | the global-mean rescale, the scale, the output scale (defaults 1) | |
| `--rescale-voxel` | 3 `sscanf("%d")` | the voxel whose value rescales | `rescale voxel <c> <r> <s>`; FS-1040: an argument that is not a number leaves the index as memory contents in FreeSurfer; both modes 0 |
| `-rt`, `--resample_type` | `<name>` | lower-cased: `interpolate`, `nearest`, `vote`, `weighted`, `cubic` (default interpolate) | another: `\n<prog>: unknown resample type "<name as given>"\n` (standard error), the hint, status 1 |
| `-it`, `--in_type`; `-ot`, `--out_type`; `-tt`, `--template_type` | `<name>` | a forced format (the format names of `specs/freesurfer-io.md`'s format identification) | |
| `-dicomread3`, `-dcm2niix`, `--dcm2niix`; `-no-dcm2niix`, `--no-dcm2niix` | | the DICOM reader dcm2niix on (the default), off | |
| `-createBIDS`, `--createBIDS`, `-dcm2niix-createBIDS`, `--dcm2niix-createBIDS`; `-dcm2niix-no-ForceStackSameSeries` (and `--`), `-dcm2niix-singlefile` (and `--`) | | those dcm2niix settings | |
| `-dcm2niix-outdir`, `-dcm2niix-info-dump`, `-dcm2niix-dicom-flist`, `-dcm2niix-opts` (and `--`) | raw | those dcm2niix settings | |
| `-siemens-ascii-dump`, `-siemens-ascii-alt-dump`; `-dicomread2`, `-dicomread0` | | the Siemens dumps; the second DICOM reader on (the default), off | |
| `-siemensBVecsCross`, `-no-siemensBVecsCross` | | `FS_dcmGetDWIParamsSiemens_VoxelSpace=1`, unset | |
| `-sn`, `--subject_name` | `<name>` | the subject's name | |
| `-gis`, `--gdf_image_stem` | `<name>` | the GDF image stem; empty: `\n<prog>: zero length GDF image stem given\n` (standard error), the hint, status 1 | |
| `-rl`, `--reslice_like` | `<name>` | the reslicing template | |
| `-f`, `--frame` | `<n>`... | the frames: the next argument and each following one that does not start with `-` and is read whole by `strtol` (an empty one counts, and then fails the integer rule); FS-1041: more than 2,000 overflow FreeSurfer's list, both modes keep them | |
| `--mid-frame` | | the middle frame | |
| `--erode-seg`, `--dil-seg` | `<n>` | erode, dilate the segmentation | |
| `--dil-seg-mask` | `<name>` | the dilation's mask | |
| `--cutends` | `<n>` | cut slices at both ends | |
| `--slice-bias` | `<f>` | the slice bias' alpha (default 1) | |
| `--fsubsample` | 3 `<n>` | frame subsampling: start, delta, end; delta 0: `ERROR: don't use subsample delta = 0` (standard output), status 1 | |
| `-il`, `--in_like` | `<name>` | the input's template | |
| `--first-dicom` | `<name>` | the file the first DICOM's name is written to | |
| `-roi`, `--roi`, `-fp`, `--fill_parcellation`, `-zo`, `--zero_outlines` | | those | |
| `-sp`, `--smooth_parcellation` | `<n>` | the smoothing count, 14 to 26 | else `\n<prog>: clean parcellation count must be between 14 and 26, inclusive\n` (standard error), the hint, status 1 |
| `-cf`, `--color_file` | `<name>` | the colour file | |
| `-nt`, `--no_translate` | | no translation of outline labels | |
| `-ns`, `--no_scale` | `<n>` | no scaling (the number itself) | |
| `-nth`, `--nth_frame` | `<n>` | read only that frame | |
| `-cg`, `--crop_gdf` | | crop GDF images | |
| `-u`, `--usage`, `--help`, `-h` | | the help, status 0 | |
| `--status`, `--statusfile` | name | the Siemens DICOM status file, written `0` and a newline at once; none after it: `ERROR: option --statusfile requires one argument` (standard error), status 1; can't create: `ERROR: could not open <name> for writing` (standard error), status 1 | |
| `--sdcmlist` | name | the Siemens DICOM file list, opened for reading at once; none: `ERROR: option --sdcmlist requires one argument`; can't open: `ERROR: could not open <name> for reading` (standard error), status 1 | |
| `--nslices-override`, `--ncols-override`, `--nrows-override` | `<n>` | `NSLICES_OVERRIDE`, `NCOLS_OVERRIDE`, `NROWS_OVERRIDE` set to `<n>` (`%d`) | `NSlicesOverride <n>`, `NColsOverride <n>`, `NRowsOverride <n>` |
| `--mosaic-fix-noascii` | | `FS_MOSAIC_FIX_NOASCII=1` | |
| `--nspmzeropad`, `--out_nspmzeropad`; `--in_nspmzeropad` | name | the output's, the input's SPM zero padding (`sscanf("%d")`; not a number: unchanged); none: `ERROR: option --out_nspmzeropad requires one argument` (or `--in_nspmzeropad`; standard error), status 1 | |
| `--no-strip-pound` | | a name's `#` part kept | |
| `-zgez`, `--zero_ge_z_offset`; `-nozgez`, `--no_zero_ge_z_offset` | | the GE z offset zeroed or not | |
| `--nskip`, `--ndrop` | `<n>` | frames skipped at the start, dropped at the end; negative: `ERROR: nskip cannot be negative` (or `ndrop`; standard output), status 1 | `nskip = <n>`, `ndrop = <n>` |
| `--diag` | `<n>` | the diagnostic number | |
| `--diag-debug` | | the diagnostics' information bit (0x10000) | |
| `--mra`, `--auto-slice-res` | | the DICOM slice thickness tags | |

The directions: after reading three numbers the length is the square root of `x*x + y*y + z*z`,
all in `float` (left to right); 0: `\n<prog>: directions must have non-zero magnitude;
in_i_direction = (%g, %g, %g)\n` (that option's name, `in_j_direction`, `out_k_direction`, ...),
the hint, status 1; not 1: `normalizing in_i_direction: (%g, %g, %g) -> ` then each divided by the
length (`float`) and `(%g, %g, %g)` and a newline. NaNs as x86-64's SSE makes them (a NaN operand
passes on quieted, of two the left one; a NaN made from numbers, such as infinity over infinity,
is the negative default NaN `0xffc00000`).

After the arguments, in this order (each on standard error, status 1): the dcm2niix options
`-dcm2niix-outdir`, `-createBIDS`, `-dcm2niix-no-ForceStackSameSeries`, `-dcm2niix-info-dump` with
dcm2niix off: `ERROR: option --dcm2niix-outdir <>, -dcm2niix-createBIDS,
-dcm2niix-no-ForceStackSameSeries, --dcm2niix-info-dump <> are only valid with -dcm2niix`;
`-createBIDS` without an output folder: `ERROR: use --dcm2niix-outdir <> to specify output
directory for BIDS`; `--antialias` without `--voxsize`: `ERROR: use --voxsize to specify voxel
size for --antialias`; `--antialias` with a FWHM above 0: three lines, `ERROR: --antialias and
--fwhm are mutually excluded!`, `       use --antialias to automatically calculate FWHM, ` (a
trailing blank), `       or, --fwhm to specify desired FWHM`; `--force_ras_good` with an input
direction: `ERROR: cannot use --force_ras_good and --in_?_direction_flag`; `--downsample` with
`--downsampleold`: `ERROR: --downsample and --downsampleold are mutually exclusive`.

### 2.4 The checks before the input is read

1. A voxel size given (`-iis` ... `-oks`) of 0 or less: `\n<prog>: voxel sizes must be greater
   than zero\n`, then each such size `in i size = %g` (`in j`, ..., `out k`; standard error), the
   hint, status 1.
2. No input name: `\n<prog>: missing input volume name\n`; no output name unless `-ro` or `-nw`:
   `\n<prog>: missing output volume name\n`; `--out_stats_table` without `--like`: `\n<prog>: pass
   example (or empty) stats-table with --like to specify measure, column and row headers\n`
   (standard error, the hint, status 1).
3. The input's name without its format and frame parts (`specs/freesurfer-io.md`: what follows the
   last `@`, and the last `#` unless `--no-strip-pound`). An input forced to `spm` without
   `--in_nspmzeropad` gets a padding of 3.
4. A forced input, template or output format that is unknown (the output's unless an ASCII output):
   `\n<prog>: unknown input volume type <name>\n` (`template`, `output`; standard error), the hint,
   status 1.
5. `-ro` with an output name: `<prog>: warning: read only flag is set; nothing will be written to
   <name>`; `-ro` with `-oi` or `-om`: `<prog>: warning: read only flag is set; no output
   information will be printed` (standard error).
6. Unless `-ro`: the output's format is the forced one (also for an ASCII output or a stats table:
   then unknown), else identified from its name (`specs/freesurfer-io.md`); unknown: `<prog>: can't
   determine type of output volume` (standard error), status 1. A COR output is conformed.
7. `-po`: the parse-only report, then status 0: `input volume name: <name>`, `input name only:
   <name>`, `output volume name: <name>`, `parse_only_flag = 1`, `conform_flag = <0|1>`,
   `conform_size = %f`, `in_info_flag = <n>`, `out_info_flag = <n>`, `in_matrix_flag = <n>`,
   `out_matrix_flag = <n>`; with a forced input `input type is <code>`, a forced output `output type
   is <code>` (the format's number); a subject name `subject name is <name>`; an inversion value of
   0 or more `inversion, value is %g`; `-r`: `reordering, values are <a> <b> <c>`; `-r4`:
   `reordering, values are <a> <b> <c> <d>`; then `translation of otl labels is on` (or `off`).
8. A GDF output without `-gis`: `<prog>: GDF output type, but no GDF image file stem` (standard
   error), status 1.

### 2.5 The help and the usage hint

The help is FreeSurfer's XML help of `mri_convert` (`mri_convert.help.xml`, the 8.2.0 text) as the
help renderer prints it (`specs/freesurfer-core.md`). The usage hint is `\ntype <prog> -u for
usage\n\n` on standard output.

## 3. Validation plan (for the clean room)

Black box, the official `mri_convert` of FreeSurfer 8.2.0 (the recorded runs' install, or the
unit oracle's build): every option of §2.3 with good and bad arguments, each rule of §2.2 (missing,
empty, partial and out-of-range numbers), the refused combinations and checks of §2.4, `-po`'s
report, the help (`-u`, no arguments) and the version options: compare standard output, standard
error and the exit status (a crash: the clean room's error of the defect). The variables the
options set are seen in part 2's outputs (the converted volumes); `-po` shows some at once. Stats
tables: `mri_convert --in_stats_table <table> out.mgz` (the volume of §1) and `mri_convert
--out_stats_table --like <template> <volume> <out>` (the printed table), with the tables of §1's
formats, their edge cases (blank lines, long lines and words, missing fields, CSV quotes, numbers
beyond `float`, `.tac` columns) and each defect's case in both modes (`--upstream-defects fix` and
`reproduce`). The replica's own probe (`validation/probes/fs-convert`, closed to clean-room
implementers) compares the replica with FreeSurfer's code case by case: 1,000 cases of seed 1,
947 identical, 53 where FreeSurfer reads memory it does not own (observed).
