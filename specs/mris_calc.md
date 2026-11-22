# `mris_calc` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-21, the whole program: its volume operations and its curvature
files; queue item 3.5-37 adds only the checks through `vertexvol`'s recorded steps). Written by the
replica's author (track D) as the spec writer: behaviour, maths, conventions and messages, for the
clean room (`larmorx-fs-volumes`, queue item C3.2-15), which works from this file, the library
modules' specs (`specs/freesurfer-*.md`) and black-box runs of the official program only.

Conventions: volumes are FreeSurfer volumes (`specs/freesurfer-mri.md`, `specs/freesurfer-io.md`:
read as FreeSurfer's general volume reader reads them, which turns every voxel that is not finite
into 0 unless `FS_LEAVE_NANS` is set; written in the format their name gives). "Standard error"
is said where a message goes there; everything else goes to standard output. `%f` and `%lf` are
C's formats (6 decimals; `nan`, `-nan`, `inf`, `-inf` by the value's sign). **The column
printer**: `<left>` right-aligned in 50 bytes (a longer text as it is), then ` [ <right> ]` and a
new line right-aligned in 30 bytes, on standard error; with an empty right part only the left part
is printed (no new line); with an empty left part only the right part (`specs/freesurfer-surf.md`,
its column texts: a right part of 16,378 bytes or more aborts, FS-119). "A float" is IEEE binary32,
"a double" binary64; the arithmetic is SSE2's (no fused operations), NaN results as §4.1 says.

## 1. The command line

`mris_calc [options] <input1> <action> [<input2> | <number>]`.

1. The version options first (`specs/freesurfer-core.md`, the version report): with only version
   options the program ends with status 0.
2. Then every argument that begins with `-` is an option, until the first that does not; the
   options are read in order, each with the value after it where it takes one. An option is the
   argument without its first `-`; it is compared without case:
   - `-output` (that is `--output`), or any option whose first letter is `o`/`O` (`-o`, `-out`,
     `-output`): the output's name is the next argument (the last one given wins).
   - `threads` or `nthreads` (`-threads`, `-nthreads`; `--threads` is no such option): the next
     argument read as C's `%d`; prints `nthreads <n>` (a value that is no number: FS-1124). The
     thread count changes nothing else.
   - `-strictExtensions`, or any option whose first letter is `e`/`E`: strict extensions (§5).
   - `-version`, or any option whose first letter is `v`/`V` (so `-verbosity` with one dash too):
     prints `8.2.0` and a new line on standard error, status 1.
   - `-verbosity` (`--verbosity`): the next argument read as C's `atoi`; any value but 0 turns on
     the verbose messages (§6).
   - `-label`, or any option whose first letter is `l`/`L`: the label file is the next argument.
   - `-help`, `-usage` (`--help`, `--usage`), or any option whose first letter is `u`, `U`, `h`,
     `H` or `?`: the help (§1.1), status 1.
   - an option whose first letter is `t`/`T` (`-t`, `-tx`; not `threads`), or whose first letter is
     `-` (`--`, `--anything` not named above): ignored.
   - anything else (`-x`, `-` alone): `Unknown option '<argument>'. Looking for help? Use '-u'
     instead.` on standard error, status 1.
   An option that takes a value given as the last argument: FS-1123.
3. After the options two or three arguments must be left; otherwise the error (FreeSurfer's fatal
   error, `specs/freesurfer-core.md`: `error: <text>` on standard error, after `error: <errno's
   text>` when `errno` is not 0) `mris_calc: Insufficient number of arguments.` + new line + `You
   should specify '<input1> <ACTION> [<input2> | <floatNumber>]'` + new line + `Use a '-u' for full
   usage instructions.`, status 10.

### 1.1 The help

The help is the text of FreeSurfer's `mris_calc.help.xml` rendered as FreeSurfer renders its help
files (`specs/freesurfer-core.md`, the help texts), on standard output, then status 1.

## 2. The inputs

Each input name is typed (§2.1) and read (§2.2, §2.3). Input 1 is read first; then input 2 if
given. A failure to read input 1 ends the program (§2.4); a failure to read input 2 is not looked
at (its size is then 0, which §3's size check refuses).

### 2.1 What a name is

1. A name that is no file (`stat` fails: `errno` keeps the failure's code) and that C's `strtof`
   reads whole (leading white space allowed, nothing left over: `2`, ` 5`, `nan`, `-inf`, `1e50`,
   `0x1p3`, `nan(0x123)`, and the empty string) is a **number** (`strtof` sets `errno` to `ERANGE`
   for an overflow or a tiny inexact result, which a later fatal error prints). `5x`, or a number
   that names an existing file (a file called `2`), is not.
2. Otherwise FreeSurfer's file-type identification (`specs/freesurfer-io.md`, `mri_identify`)
   decides: the curvature type is a **curvature file** (the new format's magic number in its first
   3 bytes); any other known type is a **volume**; unknown: the error of §2.4's third kind.

### 2.2 A volume

The volume is read whole (§ conventions). Its values are **packed** into one array of `width *
height * depth * frames` floats: frame outermost, then column, row and slice (the slice index
fastest). Before the packing `Packing` and after it `Done packing` are printed (each flushed). The
volume's header is kept (geometry, voxel sizes, the pulse parameters TR, TE, TI and flip angle,
the transform, the names): the header of the **last volume read** is the output's (§5).

### 2.3 A curvature file

The file is read with FreeSurfer's binary readers (`specs/freesurfer-io.md`, the big-endian
numbers): the 3-byte magic number (16,777,215; another: §2.4's second kind), the value count (a
32-bit integer), the face count and the values per vertex (kept for the output, from the last
curvature file read), then `count` floats. A count beyond the file's values: each missing value
prints `error: freadFloat: fread failed` (FreeSurfer's error message, `specs/freesurfer-io.md`)
and is memory contents in FreeSurfer (FS-167; 0 in larmorx). A negative count asks for more memory
than exists: `error: Cannot allocate memory` and then `error: mris_calc: virtual memory exhausted.`
(status 10). With the verbose messages on, a progress bar (§6).

### 2.4 Read failures of input 1

- the file does not open (a volume the reader refuses, after the reader's own messages, or a
  curvature file that does not open): fatal error `\nmris_calc: could not establish read access to
  '<name>'.\n`, status 252;
- the magic number is not the curvature format's: fatal error `\nmris_calc: curvature file
  '<name>' has wrong magic number.\n`, status 252;
- a name of unknown type (for either input): on standard error `\nmris_calc:\n\tSorry, but I seem
  to have encountered an error.\n\tWhile accessing file '<name>',\n\tI was unable to identify this
  file type. Does it exist?\n` and one more new line, status 10 (no `error: ` prefix: the
  program's own errors are printed so, §7's list).

A number as input 1 has no values: §3's empty-input error.

### 2.5 A number as input 2

Its value is C's `atof` of the name (a double, `ERANGE` as `strtod` sets it), rounded to a float;
input 2 is then that value repeated as many times as input 1 has values.

## 3. The checks

In this order, each the program's own error (§7) with status 10:

1. input 1 has no values: `While checking input 1,` / `it seems that the input file has no
   contents.`;
2. three arguments and input 2's size is not input 1's: `While checking on input filetype sizes,` /
   `I found a size mismatch, i.e. len(input1)!=len(input2)`;
3. three arguments, input 2 not a number and its kind (volume, curvature) not input 1's: `While
   checking on input filetypes,` / `it seems that you specified incompatible types.`.

Two volumes with as many values but other dimensions: FS-1121.

Then the output array (input 1's size, zeros) is made, and without `-o` the output's name is
`out.mgz` (input 1 a volume) or `out.crv` (a curvature file).

## 4. The action

The action is one of these names, exactly (case matters): `mul div mod add pow sub sub0 sratio
pctdiff pctdiff0 sqd sqr sqrt not set atan2 bcor sig2p mag log10 abs inv sign eq lt lte gt gte
masked and or andbw orbw upperlimit lowerlimit stats size min max mean std sum prod norm log
ascii`. Any other (`mini`, `maxi`, `MUL`): after §4.3's label steps, fatal error `Unknown operation
<action> failed.\n`, status 252 (with the verbose messages on, a new line on standard output
before it).

### 4.1 The values' arithmetic

`a` is input 1's value at an index, `b` input 2's. Every result is a double, stored rounded to a
float. Where the result is computed in float and returned as a double, a signalling NaN is quieted
(its quiet bit set, sign and payload kept) on the way. **NaN results** follow SSE: an operation of
numbers that has no value (`inf - inf`, `0 * inf`, `0 / 0`, `inf / inf`, the square root of a
negative number) gives the default NaN `0xffc00000` (double `0xfff8000000000000`); an operation
with a NaN operand gives that operand quieted, and of two NaN operands the **first** as written
below (`x op y` gives `x`'s). Comparisons with a NaN are false. Of a float to a double and back
every NaN keeps its sign and the top of its payload.

Two values (each needs input 2: FS-1122):

| action | result |
|---|---|
| `mul` | `a * b` (float) |
| `div` | 0 when `b == 0` (also `-0`); else `a / b` (float) |
| `mod` | the C library's `fmodf(a, b)`: a NaN `a` quieted; else a NaN `b` quieted; else the default NaN for an infinite `a` or a zero `b`; else the exact remainder (`a` for an infinite `b`) |
| `add`, `sub` | `a + b`, `a - b` (float) |
| `pow` | the C library's `powf(a, b)` (§4.2) |
| `sub0` | 0 when `|a|` or `|b|` is below `2 * FLT_MIN` (2^-125); else `a - b` |
| `sratio` | 0 as `sub0`; else `a > b`: `a / (b + FLT_MIN)`; else `(-b) / (a + FLT_MIN)` (`-b` flips the sign bit, also of a NaN) |
| `pctdiff` | `((a - b) * 100) / (((a + b) + FLT_MIN) * 0.5)` in float |
| `pctdiff0` | 0 as `sub0`; else `pctdiff` |
| `sqd` | `d * d` with `d = a - b` |
| `set` | `b`; and the output's name becomes input 1's name (it overwrites input 1, `-o` ignored) |
| `atan2` | the C library's `atan2f(a, b)` (`a` the ordinate; §4.2) |
| `bcor` | doubles: `p = pow(10, -|a|)` (`-|a|` on the float's bits, then widened), `p = 1 - pow(1 - p, b)`, then `sign(a) * (-log10(p))`, `sign` as `sign` below, the product with `sign` first |
| `mag` | `sqrt((a * a) + (b * b))` in float |
| `eq`, `lt`, `lte`, `gt`, `gte` | `a` when `a == b` (`<`, `<=`, `>`, `>=`), else 0 |
| `masked` | `b` when `b == 0` (its sign kept), else `a` (a NaN `b` is not 0) |
| `and`, `or` | 1 when `a != 0` and (or) `b != 0` (a NaN is not 0), else 0 |
| `andbw`, `orbw` | the bitwise and (or) of `a` and `b` truncated to 32-bit integers, as a double: x86-64 gives -2147483648 for a NaN or a value beyond the range (FS-17) |
| `upperlimit` | `b` when `b <= a`, else `a` |
| `lowerlimit` | `b` when `a <= b`, else `a` |

One value (input 2, if given, is not used):

| action | result |
|---|---|
| `sqr` | `a * a` (float) |
| `sqrt` | `sqrt(a)` in float (the default NaN for a negative `a`) |
| `not` | 0 when `a > 0.5`, else 1 (1 for a NaN) |
| `sig2p` | `pow(10, -|a|)` in double |
| `log10`, `log` | 0 when `a <= 0`; else the C library's `log10f(a)`, `logf(a)` (§4.2; a NaN `a` is not `<= 0`) |
| `abs` | `a` with its sign bit cleared (also a NaN's) |
| `inv` | `1 / a` in double |
| `sign` | -1 when `a < 0`, 1 when `a > 0`, else 0 (0 for a NaN) |
| `norm` | the minimum `m` and maximum `M` of input 1 (§4.4's running ones, as floats), `r = M - m` (float); then each value `(a - m)` (float), then `div` of that by `r` (0 everywhere when `r == 0`) |

### 4.2 The C library's functions

`powf`, `atan2f`, `log10f`, `logf`, and the double `pow` and `log10`: by default larmorx's
correctly rounded functions (`atan2f` as the correctly rounded double `atan2` rounded to a float);
with `reproduce` glibc 2.35's (larmorx-core's `*_glibc`, x86-64's build); a run whose values differ
between the two choices notes GLIBC-07. Their NaN results are glibc's in both modes (a NaN argument
gives a NaN as glibc's x86-64 build gives it: `powf(1, sNaN)` and `pow(x, qNaN)` are NaN, `pow(1,
qNaN)` is 1; `atan2f` of two NaNs gives its second argument's quieted; `log10` of a negative
number a positive quiet NaN).

### 4.3 With `--label`

Before the action: input 1's, input 2's and (for an action that writes, §5) the output's values are
kept; the label is read (`specs/freesurfer-io.md`, `LabelRead` with no subject; with the verbose
messages, `Reading label...` before and `ok` after in the column printer; a label that cannot be
read: after the reader's messages, fatal error `mris_calc: could not read label file <name>`,
status 255); its points' vertex numbers mark input 1's indices (a vertex number outside input 1's
values: FS-1125); the inputs and the output become their values at the marked indices, in index
order. The action runs on those. For an action that writes, the output written is the zeros of
§3 with the results at the marked indices.

### 4.4 The statistics

`size`, `min`, `max`, `mean`, `std`, `sum`, `prod` print one statistic, `stats` all of them, in
this order, through the column printer (standard error), over input 1's values (inside the label
with `--label`). With `-o` each statistic printed is also written to the output's name as `%lf`
and a new line, each one replacing the file (with `stats` the file ends holding the product: rule
C, kept). An action that only prints writes no other output.

- `Size`: the count as a 32-bit integer (`%d`).
- `Min@(index)`: `%f (%d)`: the running minimum: the first value, then every value `<=` it (a NaN
  never; a NaN first stays), and the index of the last value that set it (the first value's index
  0), the index rounded through a float (FS-1126). `Max@(index)`: the same with `>=`, the index not
  rounded.
- `Mean`: the values summed in double (`a + sum`), the sum rounded to a float, divided in float by
  the count as a float.
- `Std`: in double, `s` the sum (`a + s`) and `q` the sum of the squares (`a * a` in float, then `a²
  + q`), then `((q * n) - (s * s)) / n` with `n` the count; rounded to a float `v`; printed:
  `sqrt(v / (n - 1))` in float (`n - 1` as an unsigned 64-bit count converted to a float: 1.8e19
  for no values; one value gives `-nan`).
- `Sum` (`a + sum` in double, printed as a float; the file gets the double), then `negCount`,
  `zeroCount`, `posCount` (values `< 0`, `== 0`, `> 0`; printed `%f` of the count as a float:
  FS-1126).
- `Prod`: the product in double (`a * prod` from 1), printed as a float, the double in the file.
- With no values (an empty label): 0 for each, the indices 0.

`ascii`: input 1's values (inside the label) one per line, `%f`, to the output's name (without `-o`:
`out.mgz.ascii` or `out.crv.ascii`); the column printer's `Saving result to '<name>' (type =
ASCII)` before (left part only) and `ok` after; a file that does not open: `Write error: <name>`
with no new line (FS-1120).

## 5. The output

The actions of §4.1 write the output. Its name: with an extension (what follows the last `.`;
`fio_extension`, `specs/freesurfer-io.md`) of `crv` it is a curvature file; otherwise the name is
typed as §2.1 types an input (an existing file by its contents; a name that is no file and a
whole number is a number); without an extension `.mgz` or `.crv` (input 1's) is added. With strict
extensions a name of unknown type gets input 1's extension added (`.mgz`, `.crv`), and a name whose
type's extension (`mgz` for every volume type, `crv`, `dummy` for a number) is not its own gets
that extension added and is then taken as input 1's kind. A name of no type found is input 1's
kind. A kind that is not input 1's: the program's own error `While checking on output filetype,`
/ `it seems there is a type mismatch to the input type.`, status 10 (a curvature input with `-o
x.mgz`, a volume with `-o x.crv`, a number-like `-o 3.5`).

- **A volume**: a float volume of the last volume read's dimensions and header (§2.2: with two
  volumes, input 2's), its values unpacked in the packing order; before it `Unpacking` (flushed),
  then the column printer's `Saving result to '<name>' (type = <type> )` (left part only; `<type>`
  the name of the output's file type as FreeSurfer names it: `MGH`, `nii`, ...: the type found for
  the name, or input 1's when none was), the volume written (FreeSurfer's writer, by the name's
  format, with its own messages), then `ok`, or `error` when the writer failed (FS-1120).
- **A curvature file**: the new format (magic number, the count, the last curvature file's face
  count and values per vertex, the values as big-endian floats); the column printer's `Saving
  result to '<name>' (type = MRI_CURV_FILE)` (left part) before the values and `ok` after; a file
  that does not open: nothing printed (FS-1120).

Then `#VMPC# mris_calc VmPeak  <kB>` (two blanks; the process's peak virtual memory in kB, which
varies) and status 0.

## 6. The verbose messages

With `--verbosity` other than 0: after reading the arguments, the column printer's `Input file 1:`
/ input 1's name, `ACTION:` / the action, `Input file 2:` / input 2's name (when given), `Output
file:` / the output's name as it then is (`out` without `-o`); `Reading...` (left) and `ok`
(right) around each volume read; after the inputs `Size of input1: ` / its size (`%d`), `Type of
input1: ` / `Volume`, `Curvature` or `FloatArg`, `ACTION:` / the action, `Size of input2: `, `Type
of input2: ` (`Unknown` without one). Progress bars on standard output while a curvature file is
read or written and while `ascii` writes: at the first value the message right-aligned in 50 bytes
(`Reading <name>`, or the saving message) and ` [` (flushed); at every value whose index `i` has
`i % f == f - 1` for `f` the count times 0.05 truncated (when `f > 0`) a `#` (flushed); at the last
value `] ` and `[ ok ]` with a new line (not flushed). A new line on standard output after the
action.

## 7. Exit statuses and the program's own errors

The program's own errors print on standard error `\nmris_calc:\n\tSorry, but I seem to have
encountered an error.\n\tWhile <action>,\n\t<error>\n` and one more new line, and end with the
status given. FreeSurfer's fatal errors (`error: ...`) end with their code's low 8 bits (252 for
-4, 255 for -1). Statuses: 0 (done, also after a failed write: FS-1120), 1 (help, version, an
unknown option), 10, 252, 255; FreeSurfer's crashes 139 and aborts 134 (FS-1119, FS-1122,
FS-1123). Standard output is the C library's buffer: what was printed since the last flush is lost
at a crash or an abort.

## 8. Defects

Each with `reproduce` giving FreeSurfer's behaviour (`crates/larmorx-core/src/defects.rs`):

- **FS-1119**: a text past the program's 65,536-byte buffers: the output's or the label's name
  (65,536 bytes or more), an input name that is no file of that length, the unknown-type message
  of a name of 65,519 bytes or more, the volume output's messages: FreeSurfer aborts (`*** buffer
  overflow detected ***: terminated` on standard error, status 134); adding an extension to such
  an output name is not checked (FreeSurfer writes past its buffer; the name cannot be written).
  larmorx uses the name whole.
- **FS-1120**: an output that cannot be written ends with status 0 (after ` [ error ]`, `ERROR:
  opening <name>` for a statistic's file, `Write error: <name>`, or nothing for a curvature file);
  larmorx ends, after those, with the fatal error `\nmris_calc: could not establish write access to
  '<name>'.\n`, status 252.
- **FS-1121**: two volumes of other dimensions with as many values are combined in the packing
  order and the output has input 2's dimensions; larmorx refuses them with §3's size mismatch.
- **FS-1122**: an action of two values without input 2 crashes FreeSurfer (status 139); larmorx:
  the program's error `While checking inputs,` / `this operation needs a second input.`, status 10.
- **FS-1123**: an option that takes a value given last crashes FreeSurfer (139); larmorx prints
  `mris_calc: option '<option>' needs a value` on standard error, status 1.
- **FS-1124**: `threads` with a value that is no number prints memory never written (observed 0);
  larmorx prints `nthreads 0` in both modes.
- **FS-1125**: a label point whose vertex number is outside input 1's values (`-1` for a volume
  label's points, or past the end) is written outside FreeSurfer's mask (such points never count);
  larmorx refuses the label (the program's error `While reading the label,` / `a label point is
  outside input 1's values.`, status 10); `reproduce` skips those points.
- **FS-1126**: the minimum's index, the three counts and a label's indices pass through a float:
  past 16,777,216 they are rounded (a label's values then land at a rounded index); larmorx keeps
  them exact.
- Kept (rule C, no switch): with `stats -o` only the product stays in the file; the output's
  header from input 2's volume; `--anything` ignored; the standard deviation's one-pass formula.

## 9. Validation plan

The open probe `validation/investigations/freesurfer-mris-calc-probe/` (its README) runs the
official program and a candidate on 228 command lines over seeded synthetic inputs (every option
and its one-dash forms, every action on curvature files with NaN, infinite, denormal and zero
values and on numbers, the statistics with and without `-o`, `ascii`, the output's names and
strict extensions, labels inside, outside and empty, every input type and failure, the volume
types, names past the buffers) and compares exit status (a signal as 128 + its number), messages
(the case's folder, the peak memory and the version report's time masked) and every file written;
`reproduce` must give FreeSurfer's on every case but those noting FS-167, FS-1124 or FS-1125
(counted); `fix` may differ only with a notice. The replica (observed, 2026-11-22): seed 1, 223 of
226 identical in `reproduce`, 3 counted, 0 different. A clean room is also checked against the
real calls of `mri_nu_correct.mni`, the vertex areas and `label-cortex` on the recorded heads (the
replica's record, `docs/validation/freesurfer-mris-calc.md`: the areas equal the runs' own
`?h.area.mid`), and its golden cases (32^3 volumes and 1,000-value curvature files, both modes).
