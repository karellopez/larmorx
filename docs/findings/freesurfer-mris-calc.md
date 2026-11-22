# Findings: FreeSurfer 8.2.0's `mris_calc` (replica, queue item 3.2-21)

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source and the official
binary's object code; the clean room's input is `specs/mris_calc.md`).

Track D's relay 50 (2026-11-22). The replica: `crates-freesurfer/larmorx-freesurfer-volumes/src/
mris_calc/` (the program whole: volumes and curvature files). Checks: the open probe
`validation/investigations/freesurfer-mris-calc-probe/` (228 command lines), the closed
`validation/probes/fs-calc/` (`prog.py`, `real.py`, `record.py`), the golden cases
`tests/mris_calc_golden.rs`, the memory entry `MRIS_CALC_CASES`; the record
`docs/validation/freesurfer-mris-calc.md`.

## 1. The official build's arithmetic (object code)

The functions of one and two values are separate functions called through a pointer
(`CURV_functionRunABC` and `CURV_functionRunAC` are not specialised): each receives the arrays'
floats directly (`movss`: a signalling NaN reaches it unquieted) and returns a double that the
loop stores with `cvtsd2ss`. Decoded from the disassembly of `bin/mris_calc` (stripped; the
functions at 0x182a0-0x18cf6 identified by their calls and constants):

- C++'s `<math.h>` overloads pick the float functions for float arguments: `fmodf`, `powf`,
  `atan2f`, `log10f`, `logf`, `sqrtss` (`sqrtf` only called for `errno` on a negative argument);
  `fn_bcor` and `fn_sig2p` call the double `pow` (`pow(10, ...)` with an `int` 10) and `log10`.
- Operand orders (the destination register first, which decides a NaN): `fn_mul` `a * b`;
  `fn_pctdiff` `((a - b) * 100) / (((a + b) + FLT_MIN) * 0.5)` (the `/ 2` is `* 0.5`); `fn_mag`
  `a² + b²`; `fn_sratio` `a / (b + FLT_MIN)` or `(-b) / (a + FLT_MIN)` (`-b` by `xorps`);
  `fn_bcor` `sign(a) * (-log10(p))`; the running sums `(double) a + sum`, `(double)(a * a) +
  sum2`, `(double) a * prod`; `fn_dev` `((sum2 * n) - (sum * sum)) / n`; `fn_mean` the sum
  rounded to a float over the count as a float (`cvtsi2ss`).
- `fn_upl` and `fn_lrl` are branch-free (`cmpless`, `andps`/`andnps`/`orps`): the C's choice,
  `a` for a NaN operand; `fn_masked` returns `b`'s bits when `b == ±0`; `fn_abs` clears the sign
  bit (`andps`); `fn_andbw`/`fn_orbw` convert with `cvttss2si` (INT_MIN for NaN and out of range:
  FS-17's x86-64 value).
- `fn_mini` returns the index as `(float)`, `fn_maxi` as an `int`: only the minimum's index is
  rounded (FS-1126).
- Constants (`.rodata` 0x257a1c-0x257b08): 0, `2 * FLT_MIN` = 2^-125 compared as a float, 100,
  `FLT_MIN`, 0.5, 1; the double 1, -1, 10 and the progress bar's 0.05.

Measured on curvature files with NaN, infinite and denormal values (the open probe's cases and
`ops.rs`'s test `nans_as_the_official_build`): every operation's bits match this model; glibc's
`fmodf` gives a NaN `x` quieted, else a NaN `y` quieted, else the default NaN for an infinite `x`
or a zero `y`; `powf(1, sNaN)` is NaN; `atan2f` of two NaNs gives `x`'s; `pow(1, qNaN)` is 1, so
`bcor` of `-inf` with a NaN `b` gives `-inf` (the `cvtss2sd` before `pow` quiets the NaN).

## 2. The program's quirks

- **The output's name and type** (`fileWrite`): the name is typed by `fileType_find` as an input
  is (`stat`, then `strtof`, then `mri_identify`), so an existing output file's contents decide its
  type (an existing volume named `-o` for a curvature input refuses the run), and a name that is a
  whole number and no file is a number (`-o 3.5`: incompatible). Without an extension `.mgz`/`.crv`
  is added even without `-e`. With `-e` the extension added for a name of known type is that
  type's (`dummy` for a number: `-e -o 3.5` writes `3.5.dummy`), and the type message then still
  names the type found (an existing NIfTI file `x.nii` with `-e`: written as `x.nii.mgz`, MGH by
  its name, the message naming NIfTI's type); a name of no type found takes input 1's type.
  These `strcat`s are not checked by the build (FS-1119's unchecked sites): a name of 65,536 bytes
  or more is written past `G_pch_curvFile3` into the globals after it; no such name can be a file.
- **The output's header** is the last volume read (`Gp_MRI`, replaced by input 2's): two volumes
  of the same dimensions but other geometry give input 2's geometry (rule C, kept); other
  dimensions with as many values are paired in the packing order (FS-1121).
- **`set`** writes to input 1's name, whatever `-o` says (the C's `strcpy(G_pch_curvFile3,
  G_pch_curvFile1)` after the operation): it overwrites its input.
- **`stats -o`**: each statistic opens the file with `"w"`, so it ends holding the product only
  (rule C, kept: no script uses it; correcting would invent a format).
- **Unchecked failures**: input 2's read status is never looked at (a failed read gives size 0,
  which the size check refuses); the output's write status neither (FS-1120): `VOL_fileWrite`'s
  ` [ error ]`, `CURV_fileWrite`'s silent return, `ascii`'s `Write error: <name>` without a new
  line, `PrintStatToFile`'s `ERROR: opening`: all end with status 0. The program's own
  `fileIO_errorHander` has the write-access message for these; larmorx's default uses it.
- **Options**: the `else if` chain compares whole names first (`stricmp`, so `--OUTPUT` works),
  then the first letter: any `-o...` is the output, `-e...` strict, `-v...` the version (so
  `-verbosity` with one dash prints the version), `-l...` the label, `-t...` nothing, any
  `--...` nothing (`--threads 2` leaves `2` as input 1). An option's value read past the last
  argument is `argv[argc]`, NULL: `strcpy`, `sscanf` and `atoi` of it crash (FS-1123).
- `threads` prints `nthreads %d` of an `int` `sscanf` may not write (FS-1124: 0 on every run
  observed; the stack slot).
- `Gppch_operation` lacks a comma after `"square root"` (so `"square root" "set"` is one string
  and every later name is off by one); the table is never used (no effect).
- **`errno`** reaches the fatal errors' first line (`ErrorExit` prints `strerror(errno)` when it is
  not 0): `stat` of a number (`ENOENT`, so `mris_calc a.curv foo 2` prints `error: No such file or
  directory` before `Unknown operation foo failed.`), `strtof`'s `ERANGE` (`1e50` as input 2),
  `malloc`'s `ENOMEM` (a negative curvature count: `error: Cannot allocate memory`). The C
  library's maths functions' `errno` (`EDOM`, `ERANGE`) is not modelled: a later fatal error
  follows only `stat` calls of the output's name, which set their own (gap kept, noted here).
- **Buffers** (FS-1119): `-o`'s and `-l`'s `strcpy` into 65,536-byte globals, `fileType_find`'s copy
  of a name that is no file, the unknown-type message's `sprintf`, `error_exit`'s, and
  `VOL_fileWrite`'s two messages are checked by the build (`__strcpy_chk`, `__sprintf_chk`): past
  them the program aborts (`*** buffer overflow detected ***: terminated`, signal 6) with its
  unflushed standard output lost; measured by the open probe's names of 65,536, 65,532 (strict)
  and 65,525 bytes.
- **Labels** (FS-1125): `G_pf_labelMask[vno] = 1.0` with no bound check; a volume label's points
  have `vno = -1` (four bytes before the array: the heap chunk's size field) and a surface label
  on a smaller input writes past it. The mask's indices are kept as floats (FS-1126).
- The packing order (frame, column, row, slice) makes `ascii`, the minima's indices and a label's
  vertex numbers index volumes with the slice fastest.
- Memory: the inputs are freed before the output volume is allocated; `norm` leaks its two
  arrays. The port's peaks are 0-7 % below FreeSurfer's (`MRIS_CALC_CASES`).
