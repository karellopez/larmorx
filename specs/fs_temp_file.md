# `fs_temp_file` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-11: the command line, the folder chosen, the file made, every
message, the exit statuses, the defect with its switch, and the validation plan, §6). Written by the
replica's author (track A, relay 52) as the spec writer: behaviour, conventions and messages, for
the clean room (`larmorx-fs-volumes`), which works from this file, the library modules' specs
(`specs/freesurfer-argparse.md` for the command-line parser, its messages, the help and the version
options; `specs/freesurfer-core.md` §18 for the temporary folder and the file's name) and black-box
runs of the official program only.

Conventions: "standard output" unless said otherwise. An **error** is FreeSurfer's error message
(`specs/freesurfer-core.md`: `error: ` and the text, then a new line) and the end of the program
with the status given.

## 1. The command line

`fs_temp_file [-s|--suffix <str>] [-b|--base <dir>] [--scratch]`, read by FreeSurfer's parser of
its newer programs (`specs/freesurfer-argparse.md`) configured, in this order, with: the help text
(`-h`, `--help`), `-s`/`--suffix` taking one value, `-b`/`--base` taking one value, `--scratch`
taking none (and the parser's own `--all-info` and `--version`). So:

- a value-taking option at the end, or followed only by another known option: error `not enough
  inputs supplied to '<option as written>'`, status 2 (`-s -b`: about `-s`); a value may start with
  a dash when it is not a known option (`-s -x`: the suffix `-x`);
- any other word (an unknown option `--foo`, `-s=.mgz`, `-sb`, `-version`, an empty argument, a word
  after an option's value or after `--scratch`): error `unexpected argument '<word>'`, status 2,
  at the first such word (before a later `--help`);
- an option given twice: the last value wins (`-s .mgz -s .lta`: `.lta`);
- `--version`: `fs_temp_file freesurfer 8.2.0` on standard output (once, however often given); then
  `--all-info`'s line (`specs/freesurfer-core.md`, the version report); when the arguments are
  exactly those printed (`--version`, `--all-info`, or both, each once) the program ends, status 0;
  otherwise it goes on (`--version --version` prints one line and makes a file);
- `-h` or `--help` (after the version lines): the help (§1.1), status 0, no file made.

### 1.1 The help

FreeSurfer's XML help renderer (`specs/freesurfer-core.md`) on this text (`fs_temp_file.help.xml`):
name `fs_temp_file`; usage `fs_temp_file [options]`; description "Generates and creates an empty
temporary file. The resulting path will be printed to stdout. The FS_TMPDIR environment variable can
be used to override the default temporary directory."; optional arguments `-b, --base [dir]`
("Manually specify base temporary directory."), `-s, --suffix [str]` ("Optional file suffix."),
`--scratch` ("Use /scratch directory if available, but FS_TMPDIR takes priority."), `-h, --help`
("Print this help text and exit.").

## 2. What it does

1. `--scratch`: when `FS_TMPDIR` is not set (set and empty counts as set), the program's own
   environment gets `FS_TMPDIR=/scratch` (whether or not `/scratch` exists).
2. `--base <dir>`: the program's environment gets `FS_TMPDIR=<dir>` (replacing any value, `--scratch`'s
   included), even when `<dir>` is empty.
3. The base folder is chosen as `specs/freesurfer-core.md` §18 says: the first of `FS_TMPDIR`,
   `TMPDIR`, `TMP`, `TEMP`, `TEMPDIR` that is set, not empty, and an existing folder in which a new
   file can be made; when none is: the value of `TEMPDIR` when it is set and not empty, else `/tmp`;
   that choice not writable: error `Can not write to temporary directory <dir>`, status 1. A name is
   used as written (a relative name stays relative, `rel/tmp.XXXXXX`; a final `/` stays,
   `<dir>//tmp.XXXXXX`; a link to a folder is used through the link).
4. The file `<base>/tmp.XXXXXX<suffix>` is made new and empty (the suffix empty without
   `--suffix`): `XXXXXX` six letters or digits, unpredictable; **(observed)** its mode is 0600 (read
   and write for the owner only, less the process's umask). It cannot be made (a suffix holding a
   `/`, a name longer than the system allows): error `Could not create temporary file in <base>`,
   status 1. A suffix holding `XXXXXX` keeps it (`tmp.<six>XXXXXX`).
5. Its name, then a new line, on standard output; status 0.

## 3. The defect (`larmorx_core::defects`)

- **FS-1171**: a `--base` folder that does not exist, cannot be written, is a file, or is empty, is
  not used and nothing says so: the file is made in the folder of the next variable that is set and
  writable, else in `TEMPDIR`'s (an error when that is set and not writable) or `/tmp`. By default:
  error `Can not write to temporary directory <dir as given>`, status 1, before anything is made;
  `reproduce`: FreeSurfer's behaviour. A notice in both modes when it happens.

The folder rule of §2.3 (a set but unusable `TEMPDIR` ends the program where an unusable earlier
variable falls through to `/tmp`) is FreeSurfer's library behaviour, kept in both modes.

## 4. Exit statuses

0 a file made (or the help, or only version options); 1 no folder or no file (§2.3, §2.4, FS-1171's
default); 2 a command-line error (§1).

## 5. Where it runs

FreeSurfer's scripts take its standard output as a file's name (`` set f = `fs_temp_file --suffix
.txt` ``); recon-all calls it with no option before its version check (every reference run:
`/tmp/tmp.XXXXXX`, no variable set).

## 6. Validation plan (for the clean room)

Black-box runs of the official program (`$FREESURFER_HOME/bin/fs_temp_file`, `FS_DISABLE_LANG=1`, no
`DIAG_NO`), each case in a folder of its own with the variables set as the case says and no others,
compared line by line with the clean room's: the exit status, standard output and error, the
listing of the case's folder (each folder and file with its mode and size) and of a file made in
`/tmp` (then removed); the case's folder written as `<DIR>`, each `tmp.` with six letters or digits
after it as `tmp.XXXXXX`, `--all-info`'s `TimeStamp`, `User`, `Machine` and `PlatformVersion` values
as `*`. The cases: each variable alone and before the next; a variable naming a folder that does
not exist, a file, a read-only folder, a link, a folder with a final `/`, a blank, a relative name;
`TEMPDIR` unusable alone; no variable (`/tmp`); every suffix form of §2.4; `--base` absolute,
relative, with a final `/`, missing, empty, a file, read-only, twice, with `FS_TMPDIR` set; every
`--scratch` combination (on a machine without `/scratch`); every parser message of §1; `--help`,
`--version`, `--all-info` alone and together. The defect's default (§3) is checked against its
notice: the clean room's `reproduce` must give every official line.
