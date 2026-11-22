# freesurfer-mris-calc-probe: `mris_calc` as a black box (open)

An **open probe** for `mris_calc`'s clean room (`specs/mris_calc.md`): it runs FreeSurfer 8.2.0's
official program and a candidate on the same command lines and compares what they give. It holds
no FreeSurfer code: the official program is run, never read. The behaviour it checks is the
spec's.

## What it runs

- The official program: `$FREESURFER_HOME/bin/mris_calc` of the FreeSurfer 8.2.0 installation in
  the workspace (`<workspace>/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/`), found
  on the `PATH` by its name as the scripts run it (its messages print that name), with
  `FREESURFER_HOME` and `FS_LICENSE` set (`larmorx_validation.freesurfer_unit.freesurfer_env`),
  `FS_DISABLE_LANG=1`, `OMP_NUM_THREADS=1`, through the oracle-output cache
  (`docs/validation/oracle-cache.md`). No case sets a diagnostic level.
- The candidate: `--candidate '<command line>'`, the program to check, given the case's arguments
  after its own; `{mode}` in it becomes `reproduce` (FreeSurfer's results, which it must give) and
  then `fix` (the default, which may differ only where it prints a `larmorx: upstream defect`
  notice).

## The cases

`compare.py` writes seeded inputs into `<workspace>/oracles/inputs/fs-calc-prog/<seed>/` (MGZ
volumes of 10 x 12 x 8 in `float`, `uchar`, `short` and `int`, a three-frame one, one of 12 x 10 x 8
(as many values), a 5^3 one, one on another grid, a mask, a NIfTI copy, a 300 x 1 x 1 surface
overlay; curvature files of 200 values with NaNs (signalling and quiet, both signs), infinities,
denormals, zeros, one with other face counts, positive-only, integer-valued ones, one shorter than
its count, one with a negative count, an empty one, a one-value one, a five-value one, one named
`.curv`, one of the old format; labels inside the values, past them, with vertex -1, empty, and a
volume-sized one; a text file, a folder, a file named `2`) and runs 228 command lines: no
arguments, every help and version form, unknown and ignored options (`-x`, `-`, `--`, `--foo`,
`-t`, `--threads`), every option and its one-dash forms (`-o`, `--output`, `-output`, `-O`, `-e`,
`--strictExtensions`, `-l`, `--label`, `-label`, `--verbosity` 0/1/2/-1, `-verbosity` (the version),
`-threads`, `-nthreads`, an unreadable thread count), options given last without their value,
too few and too many arguments, unknown actions (`foo`, `mini`, `maxi`, `MUL`); every action of
two values on two curvature files, twelve with a number, every action of one value, the libm
actions on positive and signed values, the bitwise and logical actions on integers; every
statistic with and without `-o` (and to a folder that does not exist), `ascii`; numbers as input 2
(`nan`, `-inf`, `1e50`, `1e-50`, `0x1p3`, `nan(0x123)`, ` 5`, `5x`, the empty string, `1e400`,
`0.1`, a file named `2`) and as input 1; the output's name (none, no extension, `.mgz` for a
curvature input, `.txt`, `3.5`, with `-e`, the last `-o` winning, an existing volume, in place,
`set` over its input); every mismatch of types and sizes, empty, negative-count, old-format and
unknown inputs, missing files, a folder, a second input missing for an action of two values;
labels (`norm`, `ascii`, statistics, `set` in a label, empty, outside, vertex -1, missing); the
volume actions (`mri_nu_correct.mni`'s rescale, two volumes, `uchar` minus `short`, `int` over
`float`, three frames, a mask, other grids in both orders, other dimensions, other sizes, NIfTI
in and out, an output without an extension, a curvature output, `norm`, `andbw` of `short` and
`int`, `pow`, a surface overlay, statistics, `ascii`, the verbose messages, in place); names of
65,536, 65,532 (with `-e`) and 65,525 bytes.

Compared for each case and mode: the exit status (a process ended by a signal as 128 + its
number); standard output and standard error (the case's folder in messages that name a file by
its absolute name, `--all-info`'s `TimeStamp`, the `#VMPC#` line's figure and the candidate's
`larmorx:` lines masked); the names of the files written and each file (FreeSurfer's provenance
left out: `larmorx_validation.freesurfer.files`). A case whose `reproduce` run notes a defect of
`NOT_COMPARABLE` (FreeSurfer reads memory it never wrote, FS-167 and FS-1124, or writes outside
its mask, FS-1125: their texts are in `crates/larmorx-core/src/defects.rs`) is counted, not
compared.

## Commands

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 12 python -P \
        validation/investigations/freesurfer-mris-calc-probe/compare.py --seed 1 \
        --candidate '<program> --upstream-defects {mode} freesurfer mris_calc' \
        [--only 3,7] [--json out.json]

It exits with 1 when a compared case differs in `reproduce`, or differs in `fix` without a notice.
Run it in a memory box (every case needs less than 100 MiB).

## Results (observed, relay 50 of track D, 2026-11-22)

The replica as the candidate (`validation/probes/fs-calc/prog.py`, which runs this file's cases),
seeds 1 and 2, 228 cases each: 223 identical in `reproduce`, 5 counted (FS-167 twice, FS-1124,
FS-1125 twice), 0 different; `fix` identical or explained by a notice on every compared case.
