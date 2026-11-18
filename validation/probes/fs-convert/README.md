# fs-convert: `mri_convert`'s stats tables and command line against FreeSurfer 8.2.0's official code

**Closed to FreeSurfer clean-room implementers**: `fsconvert.cpp`, the hooks and `check.py` name
FreeSurfer's functions and variables (running `fsconvert` is a black-box run of FreeSurfer's
code; the behaviour is `specs/mri_convert.md`).

What it shows: the replica `crates-freesurfer/larmorx-freesurfer-volumes` (`src/mri_convert/`)
gives FreeSurfer's results bit for bit with `--upstream-defects reproduce`, case by case, and
differs by default only where a defect it corrects is noted. Queue items 3.2-04 and 3.2-05's first
half (track A, relay 46): the stats tables of `--in_stats_table` and `--out_stats_table`
(`utils/stats.cpp`), `include/csv.h`'s reader, and `mri_convert`'s `main` from its start to the
checks before the input is read (its option loop, the checks, the parse-only report, the help).

## Files

- `fsconvert.cpp`: the official side. It compiles a copy of `mri_convert.cpp` (its `main`
  renamed) with two hooks, and calls the unit oracle's `LoadStatTable`, `InitStatTableFromMRI`
  and `WriteStatTable`, with `csv.h` compiled in a namespace of its own. Each case line runs in a
  child process in `<cases>.d` (emptied first; at most 4 GiB of address space, 60 s); it prints
  `<result> | <text>` (the text, standard output and error unbuffered in their order, C-escaped),
  or `EXIT <status> | <text>` / `CRASH <signal> | <text>`, then the files the case wrote (names not
  starting with `in_`: `F <name> <FNV-1a>`, a `.mgz` read back and printed). Commands: `csv <hex>`
  (`readCSV`: the rows, each row's fields); `lst <name> <hex|->` (`LoadStatTable`, the file
  written first unless `-`: the table's names, numbers as `double` bits and its volume); `ist
  <vol> <name|-|@> <hex|->` (`InitStatTableFromMRI` with a template, none, or an empty name);
  `wst ... <out>` (the same, then `WriteStatTable`); `opt <nfiles> [<name> <hex>]... <nargs>
  <arg>...` (`main` with those arguments, hex: its output, then the dump hook's line of every
  variable the options set).
- `hook_init.inc`, `hook_dump.inc`: the hooks `build.sh` puts into the copy of `mri_convert.cpp`
  (after `main`'s last declaration: main's unset locals get marked values; before the GDF stem
  check: the dump, then the end of the child).
- `build.sh [<workspace>]`: builds it against the unit oracle (`oracles/freesurfer-8.2.0-unit`)
  into `<workspace>/oracles/freesurfer-8.2.0-probes/fs-convert/` with a `BUILD_INFO.txt` (the
  checksums of the binary, the probe's files and the tag's `mri_convert.cpp`).
- `check.py --seed <s> --n <n>`: the port's example `fs_convert_probe` generates the cases (45
  fixed ones, then seeded: tables in FreeSurfer's format, CSV and TSV/TAC, the CSV reader, the
  templates, and command lines, three in four with arguments the options accept); both sides run
  them (the official one through the oracle-output cache at one thread), compared line by line;
  `SKIP`: FS-170, FS-171 (fs.io's TSV reader on memory contents), FS-1031, FS-1032, FS-1034 to
  FS-1042 (FreeSurfer reads memory it does not own or crashes).

## Results (relay 46)

Seed 1, 1,000 cases: 947 identical with `reproduce` (663 command lines, 144 tables read, 48 CSV
texts, 92 templates), 53 `SKIP`, 0 different; `fix` differs on 117, each with a notice (FS-949,
FS-950, FS-1033, FS-01). Seeds 3 (1,000) and 7 (200): 0 different. Recorded:
`tests/fs_convert_official.rs` (seed 1, 1,000 cases, 53 `SKIP`, `0d0e0ee8...`), debug 0.6 s,
release 0.3 s; the same under the AArch64 NaN emulation (`NAN_TRAP_ARM=1`, debug and release) and
on Windows x64. 264 of the 266 option names are drawn in seed 1, 251 of them reach the dump.

    validation/probes/fs-convert/build.sh <workspace>
    scripts/orchestration/boxed.sh 20 python -P validation/probes/fs-convert/check.py --seed 1 --n 1000
