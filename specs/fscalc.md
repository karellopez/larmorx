# `fscalc` (FreeSurfer 8.2.0): behaviour spec

Status: complete but the path of a first input that is a number (it needs `mri_volsynth`, not
ported yet: §5). Queue item 3.5-29, brought into the volumes crates (`larmorx-freesurfer-volumes`
for the replica) because mergeseg runs it. Written by the replica's author (track A, relay 58) as
the spec writer, for the clean room. The shell's behaviours are `specs/seg2cc.md` §2;
**(observed)** marks measurements on the official script.

## 1. What it is for

`fscalc <input> <op> [<input2>] [<op> [<input>]]... -o <out>` applies operations one after another
to a volume (FreeSurfer's front end of `mris_calc`, which takes one operation): mergeseg runs
`fscalc <a> sum <b> -o <out>`, label-cortex `... and ...`.

## 2. The command line

No argument: the usage, status 1. The help test (`specs/seg2cc.md` §2; the help: the usage,
`fscalc 8.2.0`, its text, then `mris_calc -u`'s output). Then `$FREESURFER_HOME/sources.csh`
(unset variable or missing file: as mergeseg, `specs/mergeseg.md` §2).

Options (a value missing: `ERROR: flag <flag> requires one argument`, status 1): `-o`, `--o <out>`;
`-odt`, `--odt <type>` (the output's type, `mri_convert -odt <type> --no_scale 1`); `--float`,
`-float` (the first input converted to floats); `--log <file>` (default `<tmp>/fscalc.log`),
`--nolog`, `--no-log`; `--tmp`, `--tmpdir <dir>` (and no cleanup); `--nocleanup`, `--cleanup`;
`--no-run` (the command lines are printed, nothing is run); `--debug` (not reproduced). **Every
other word** is an input or an operation, in order.

Checks: no output: `ERROR: must spec output`; fewer than two inputs and operations: `ERROR: must
spec an input and an operation` (status 1).

## 3. The run

1. The output's folder (GNU `dirname`), `mkdir -p`; the temporary folder: `--tmpdir`'s, else
   `fs_temp_dir --scratch --base <folder>` (`<folder>/tmp.XXXXXX`, `specs/mergeseg.md` §3.2);
   `mkdir -p` it.
2. The log: unless `/dev/null`, `rm -f`; `Log file for fscalc` `>>` it; through `tee -a`: the date,
   an empty line, `cd <pwd>`, `<$FREESURFER_HOME/bin/fscalc> <the arguments>`, an empty line,
   `build-stamp.txt`'s text, `uname -a` (no version line, no `setenv` line).
3. The first input `<i1>`: a file that exists: `mri_convert <i1> <tmp>/tmp.mgh` (with `--float`:
   `-odt float` after). Otherwise: when its last component ends with `.mgh`, `.mgz`, `.nii`,
   `.nii.gz`, `.img`, `.bhdr`, `.annot`, `.m3z` or `.gii` (and is more than that ending):
   `ERROR: cannot find <i1>`, status 1; else it is a number: the template is the first of the later
   words that is an existing file (none: `ERROR: no inputs are files (that exist)`, status 1), and
   `mri_volsynth --template <template> --pdf const --val-a <i1> --o <tmp>/tmp.mgh` (§5). The command
   line through `tee`; unless `--no-run`, the program's standard output through `tee`, a failure:
   status 1 (no message).
4. While words are left: `----------------------` through `tee`; the next word is the operation
   (a leading `-` removed: `-mul` is `mul`; `sum` is `add`). A binary operation (`mul div mod add
   sub sub0 sqd atan2 mag eq lt lte gt gte upl lrl and or andbw orbw masked pow bcor pctdiff
   pctdiff0 sratio`) takes the next word (none: `ERROR: <op> requires two arguments` through
   `tee`, status 1): `mris_calc -o <tmp>/tmp.mgh <tmp>/tmp.mgh <op> <word>` (a volume or a
   number). A unary one (`sqr sqrt abs log log10 sign norm inv sig2p not`): `mris_calc -o
   <tmp>/tmp.mgh <tmp>/tmp.mgh <op>`. Each: the command line through `tee`, unless `--no-run` the
   output through `tee`, a failure status 1. Neither: `ERROR: operation <op> not recognized`,
   status 1.
5. `----------------------` through `tee`; `mri_convert <tmp>/tmp.mgh <out>` (with `--odt`:
   `-odt <type> --no_scale 1`), as step 3; `rm -rf` the temporary folder unless no cleanup (the
   default log, inside it, goes with it); status 0. No end lines.

## 4. The defects (`reproduce`: FreeSurfer's)

FS-1274, FS-1277, FS-1278 (`specs/seg2cc.md`): the operation's leading `-` is found through the
shell's `echo`, so the operation `-n` (the echo's option) stays `-n` (`ERROR: operation -n not
recognized`); by default `n`.

## 5. Not ported yet

A first input that is a number runs FreeSurfer's `mri_volsynth`, which is not ported: the port
prints the command line, then `larmorx: fscalc: a first input that is a number (<i1>) needs
FreeSurfer's mri_volsynth, which is not ported yet` on standard error, status 1, in both modes.

## 6. Validation plan

As `specs/seg2cc.md` §9: 32^3 float and integer volumes, one with NaNs and infinities, a volume of
another extent (mris_calc's error), every option, operation kind and failure; the output's voxels,
extent, type and geometry, the kept temporary folder and the log compared.
