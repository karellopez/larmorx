# fs-io: FreeSurfer 8.2.0's file helpers against the replica, case by case

**Closed to FreeSurfer clean-room implementers** (it names FreeSurfer's functions; they may run the
probe as a black box). Queue item 3.1-25 (`fs.io`, piece 1): `fio.cpp`'s readers and writers of one
number on C streams and znz files, `fio_dirname`, `fio_basename`, `fio_extension`,
`fio_DirIsWritable`, `fio_FileExistsReadable`, `fio_IsDirectory`, `TSV::read` and `TSV::write`;
`bfileio.cpp`'s byte swaps; `MRIio_old.cpp`'s `lcalloc`.

- `fsio.cpp`: the official side. It calls FreeSurfer's own functions, linked with the unit oracle's
  `libutils.a` and packages as the official programs are (`build.sh`, Ubuntu's gcc-8 8.4.0). Each
  case runs in a forked child (1 GiB of address space, 5 seconds) inside a fixture directory made
  beside the cases (`f`, `d/i`, an unreadable `nr`, a read-only `ro`, links `ln`, `dln`, `bad`).
- `check.py`: generates the cases with the port's example `fs_io_probe` (`crates-freesurfer/
  larmorx-freesurfer-io/examples/`, cases in `tests/fs_io/cases.rs`), runs `fsio` through the
  oracle-output cache at one thread, runs the port with `reproduce` and `fix`, compares. A `?` digit
  in the port's line is a byte FreeSurfer takes from memory contents (FS-49, FS-167): it matches any
  digit. Lines the port marks `SKIP` (FreeSurfer crashes or reads for ever, or the port refuses:
  FS-169, FS-172, FS-173, FS-175) are counted, `VARY` (FS-168, FS-170) compared apart.
- Piece 2 (queue item 3.1-25's second piece, relay 11): the commands `vgw`/`vgr` (`VOL_GEOM::write`/`read`,
  names copied over earlier names), `fsw`/`fsr`/`fsl` (`FStagsIO`'s writers, a reader script over a tag stream,
  the `getlen_*` lengths; `fsl sizeof` prints `sizeof(MRI_FRAME)` and `sizeof(CTE)`), `ctw`/`ctr` (the binary
  colour tables on C streams and znz files), `tw`/`tr`/`ztw`/`ztr` (tags.cpp's tag functions), `cmdl`
  (`TAGmakeCommandLineString`); the port's side in `tests/fs_io/piece2.rs` (the cases' files built from the
  formats' rules, then cut or changed). The port marks `SKIP` where FreeSurfer crashes or its result depends on
  the machine (FS-179, FS-181, FS-183, FS-185, FS-188 past the entry, FS-189, FS-191, skips beyond 256 MiB) and
  `VARY` where it reads memory contents (FS-49 and FS-167 in these commands, FS-180, FS-187, FS-192).
- `scanf_lf.c`: glibc's `sscanf("%lf")` as a black box (bits, `errno`, bytes read), the measurements
  behind `larmorx-freesurfer-core::cscan`'s `%lf` test.

Run (inside a memory box; the port and FreeSurfer run on generated inputs):

    validation/probes/fs-io/build.sh <workspace>
    scripts/orchestration/boxed.sh 6 python -P validation/probes/fs-io/check.py --seed 1 --n 1100
    scripts/orchestration/boxed.sh 6 python -P validation/probes/fs-io/check.py --seed 1 --n 1000 --portable
    scripts/orchestration/boxed.sh 8 python -P validation/probes/fs-io/check.py --seed 7 --n 22000

The portable run's hash (`official_sha256_skip`) is `tests/fs_io_official.rs`'s, checked on every
platform. Results (2026-11-13): `docs/findings/freesurfer-io.md`.
