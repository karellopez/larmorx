# FreeSurfer 8.2.0's `fs_temp_file`: findings of the replica (queue item 3.2-11)

**Restricted** (source-derived): closed to FreeSurfer clean-room implementers, who work from
`specs/fs_temp_file.md`. Track A, relay 52 (2026-11-22). The replica:
`crates-freesurfer/larmorx-freesurfer-volumes/src/fs_temp_file/`; the probe:
`validation/probes/fs-temp-file/`; the record: `docs/validation/freesurfer-fs-temp-file.md`.

## The program

`io/fs_temp_file.cpp` (31 lines): an `ArgumentParser` with `addHelp`, `addArgument("-s",
"--suffix", 1)`, `addArgument("-b", "--base", 1)`, `addArgument("--scratch")` (the type left
`Unknown`: a string for the two with a value, a flag for `--scratch`); `parse`; then
`setenv("FS_TMPDIR", "/scratch", 0)` for `--scratch` (no overwrite: an `FS_TMPDIR` set to the empty
string is kept), `setenv("FS_TMPDIR", basedir.c_str(), 1)` for `--base`, `makeTempFile(suffix)`
(`utils/utils.cpp` 1942-1951) and `std::cout << tempfile << "\n"`. `main` returns 0 by falling off.
Every message comes from the parser (`utils/argparse.cpp`, ported as `fs.core`'s `argparse.rs`,
queue item 3.1-21) and from `getBaseTempDir`/`makeTempFile` (`fs.core`'s `utils.rs`).

## The defect (rules A to D; `larmorx_core::defects`)

1. **FS-1171** (class e: a command-line quirk; rule B). `--base` only sets `FS_TMPDIR`, which
   `getBaseTempDir` (`utils/utils.cpp` 1921-1937) checks like any variable: a value that is empty
   or not a writable folder is passed over without a message, and the loop goes on to `TMPDIR`,
   `TMP`, `TEMP` and `TEMPDIR`, ending with `TEMPDIR`'s value or `/tmp`. A base the caller named
   explicitly that cannot be used makes no sense to replace silently (the file lands elsewhere and
   a script then writes there): by default the program ends with FreeSurfer's own message for an
   unusable folder, `Can not write to temporary directory <base>`, status 1, before anything is
   made; `reproduce` goes on as FreeSurfer does. Measured: six probe cases (missing with and
   without `TMPDIR`, empty, a file, read-only, missing with an unusable `TEMPDIR`). No FreeSurfer
   script passes `--base` (every call in `scripts/` gives `--suffix` or nothing); fMRIPrep never
   runs the program: no impact.

## Kept (rule C)

- `getBaseTempDir`'s loop leaves `basedir` at the last variable's value (`TEMPDIR`) when no
  variable was usable, so a set but unusable `TEMPDIR` ends the program (`Can not write to
  temporary directory <TEMPDIR>`) where an unusable `TMPDIR` alone falls through to `/tmp`. The
  library's behaviour, shared with every program that makes temporary files (`mri_segstats`'s
  colour table): either answer is defensible, no recorded or fMRIPrep call sets these variables to
  unusable folders, and a correction would change one of the two; kept in both modes, documented
  in the spec (§2.3). (A register row: the handoff.)

## The port's own corrections (no defect of FreeSurfer)

- **The modes of the temporary file and folder.** glibc's `mkstemps` makes the file with mode 0600
  and `mkdtemp` the folder with 0700 (less the umask); the port's `make_unique` (`fs.core`'s
  `utils.rs`) made them with Rust's defaults (0666 and 0777 less the umask: 0644 and 0755 here).
  Now 0600 and 0700 on Unix (`OpenOptionsExt::mode`, `DirBuilderExt::mode`; Windows has no modes).
  Measured by the probe's listings (`F t/tmp.XXXXXX 600 0` for the official program). `mri_segstats`'s
  temporary colour table and `makeTempDir`'s callers get the same modes; their outputs do not
  change.

## recon-all's call

recon-all runs `fs_temp_file` with no option before its version check (`scripts/recon-all` 8566):
every one of the 18 reference runs' `recon-all.out` shows `fs-check-version --s <subject> --o
/tmp/tmp.<six>`, no variable set; the probe's case without variables gives the same line shape.
