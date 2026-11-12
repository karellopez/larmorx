# FreeSurfer's core services: black-box checks

Closed to FreeSurfer clean-room implementers only in what `docs/findings/freesurfer-core.md`
says of the source; the script itself runs the official program and reads nothing of FreeSurfer's
source.

`blackbox.sh <FREESURFER_HOME>` runs the official FreeSurfer 8.2.0 `mri_info` (from the unpacked
`freesurfer_ubuntu22-8.2.0_amd64.deb`, `docs/freesurfer-plan.md`) on a file that does not exist,
with the diagnostic variables of `specs/freesurfer-core.md` §4, and prints each case's standard
output, its standard error as `od -c` shows it, and its exit status.

What it showed (2026-11-12, the main machine): every case exits 1; standard error is
`error: mghRead(/nonexistent.mgz, -1): could not open file` followed by a NUL byte and a newline
(FS-01); `PROFILE=7.9` prints `turning profiling diagnostics on (7)...`, `PROFILE=1e10` and
`PROFILE=nan` print `(-2147483648)` (FS-02); `DIAG_VERBOSE=` (empty) and `DIAG=0x10000000` print
`verbose diagnostics enabled...` on standard output. The replica's unit tests
(`crates-freesurfer/larmorx-freesurfer-core`, modules `log` and `diag`) hold the same bytes.
