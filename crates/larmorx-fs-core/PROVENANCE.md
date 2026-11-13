# Provenance: larmorx-fs-core

**Clean-room implementation. No FreeSurfer code was used.**

Written from [`specs/freesurfer-core.md`](../../specs/freesurfer-core.md) (FreeSurfer 8.2.0's core
services, `fs.core`, queue item C3.1-01) by clean-room implementers (FreeSurfer's second clean room,
from 2026-11-13) who never opened FreeSurfer's source (`<workspace>/tags/freesurfer-8.2.0`,
`<workspace>/freesurfer/`, GitHub's repository, the installed package's scripts, Python and
libraries), the unit oracle's build tree (`<workspace>/oracles/freesurfer-8.2.0-unit/`, apart from
running its prebuilt probe), glibc's or libstdc++'s source, the code FreeSurfer borrows (GSL,
Burkardt's and EPFL's code, Washington University's 4dfp tools), the replica crates
(`crates-freesurfer/`, `crates-gpl/`, `crates-nc/`, `crates/larmorx-afni-replica`), the documents and
tools written from those sources (`docs/freesurfer/`, `docs/findings/freesurfer-*.md`, the
preparation's and the replicas' investigations and probes, the replay tool's and the unit oracle's
Python), or `docs/parts/`. Its only inputs were:

| Input | What it gave |
|---|---|
| [`specs/freesurfer-core.md`](../../specs/freesurfer-core.md) | the behaviour of every function: the process state, the messages and their bytes, the exit statuses, the diagnostics read from the environment, the byte swaps, the command-line helpers, the random numbers' seeding and draws, the string, file-name, number and list helpers, the selections, the fast arctangent's table, the version options and stamps, times and dates, the environment record, the memory and usage reports, the logic-problem reports, the temporary files, threads, the reproducible sums, the debugging stack, HIPS and the licence check; the defects FS-01 to FS-12 and FS-261 to FS-263 with their corrections and `reproduce` |
| The standards and papers of the spec's §21 (C11, C++11, POSIX.1-2017, RFC 8536, glibc's reference manual, IEEE 754, Intel's manual, Wirth 1976, Devillard 1998, *Numerical Recipes in C* §8.5) | the C library's documented behaviour where the spec names it |
| The open probe folder [`validation/investigations/freesurfer-core-probe/`](../../validation/investigations/freesurfer-core-probe/README.md) (calls only) and black-box runs of the prebuilt probe `fs_core_probe` (FreeSurfer 8.2.0's compiled core functions and the C library), of the C library (glibc 2.35) and of FreeSurfer 8.2.0's official programs | the comparisons of the spec's §20 (`validation/investigations/freesurfer-cleanroom-core/`) |
| Black-box runs of the C library through the probe's `gmtime`, `localtime`, `strftimec` and `atan2f` commands (`tz_cases.py`) | the `TZ` readings the documents leave open (§14's notes in `clib/time.rs`), and `atan2f`'s NaN of two |
| RFC 8536's format and the system's zone files (read as data) | the zone-file reader |
| larmorx's own Apache-2.0 code: `larmorx_core::vnl_random` (VXL's generator, a permissive port), `larmorx_core::glibc_random` (a clean room), `larmorx_core::x87`, `larmorx_core::cscan`, `larmorx_core::cfmt`, `larmorx_core::math`, `larmorx_core::defects` | the generators, the extended arithmetic, the C library's readings and number formats, the correctly rounded `atan2`, the defect register's entries |
