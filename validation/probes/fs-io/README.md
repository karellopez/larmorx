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
- Piece 3 (relay 13): the commands `npy` (`fscnpy::npy2mri` on generated `.npy` files: NumPy's headers of
  every version, alignment, key order and value FreeSurfer meets, then cut or changed; the volume's extents,
  type, sizes and voxel bytes), `gda` and `gdv` (gifti.cpp's `static` helpers `gifti_alloc_and_add_darray`
  and `gifti_get_DA_value_2D`, reached through `gifti_statics.cpp`, which compiles FreeSurfer's gifti.cpp
  itself into `fsio` with gifti.cpp's own compile command from the unit oracle's `compile_commands.json`),
  `bvw` (`DTIwriteBValues`, `DTIwriteBVectors`), `psn` (`DTIparsePulseSeqName`), `bcs`
  (`DTIbvecChangeSpace` on a header with direction cosines and gradients); the port's side in
  `tests/fs_io/piece3.rs`. `SKIP` where FreeSurfer crashes or reads past what it was given (FS-199,
  FS-200, FS-201, FS-203, FS-204, FS-205, FS-207 without a shape, FS-211, FS-217, FS-219), `VARY` where its
  result is memory contents or an address (FS-202, FS-206 past the list's storage, FS-207, FS-209, FS-214,
  FS-218 and FS-220 past the matrix).
- Piece 4 (queue item 3.1-27, relay 14): the volume format probe. `sty` (`string_to_type`), `tts`
  (`type_to_string`), `mgvn` (`MRIgetVolumeName` with `MRIIO_Strip_Pound` on and off), `mid`
  (`mri_identify`), `isx` (each format's test: `is_cor` ... `is_nrrd`, `IDisCurv`, `is_signa`,
  `is_bruker`), `idf` (`IDstemFromName`, `IDextensionFromName`, `IDnameFromStem`, `IDtypeFromStem`, the
  `.bhdr` helpers, `checkBrukerFiles` with its messages), `dcm` (`IsDICOM`, `IsSiemensDICOM` and
  `mri_identify` in ten sequences, with and without `FS_DICOM_DEBUG`). A file case first makes its
  files in the fixture's folder `w` (`setup`: folders, and files of zero bytes with patches: the
  formats' headers built by `tests/fs_io/piece4.rs` from their layouts, right and broken, DICOM part 10
  and bare data sets in three transfer syntaxes with a Manufacturer element of many values and
  representations); `mid` and `dcm` end with CTN's condition stack dumped (`COND_DumpConditions`).
  CTN's messages name the file as the port opened it (joined to the fixture's path): the port's lines
  show it relative to the fixture, as `fsio` (which runs in the fixture) does. `SKIP` where FreeSurfer
  crashes or overflows (FS-221, FS-227, FS-228). `build.sh` adds CTN's include folder.
- Piece 5 (queue item 3.1-27's second piece, relay 15): mosaics and MATLAB files. `mvm` (`VolSS2MosSS`),
  `mmv` (`MosSS2VolSS`), each printing the return, the three outputs (-7 where untouched) and the
  out-of-bounds flag (7 where untouched, or not given), `mck` (`CheckMosaic`), `mlw` (`MatlabWrite` of a
  real or complex matrix of any float bits, then the file's bytes), `mlr` (`MatlabRead`), `mfr`
  (`MatFileRead` with a type; the structure's fields and data), `mrc` (`ReadMatlabFileContents`,
  `MLFCprint` to standard output, `MLFCfree`, then `ReadMatlabFileVariable`), the readers with `Gdiag`
  and `Gdiag_no` set; the `.mat` files built by `tests/fs_io/piece5.rs` from MATLAB's version 4 and 5
  formats (both byte orders, every numbers type, complex, small and normal names and numbers, 1 to 3
  dimensions, zlib-compressed, one compressing more than 100 to 1), cut or changed. A matrix of more
  than 65,536 numbers prints as `fnv<FNV-1a 64 of its floats' bytes>`. `SKIP` where FreeSurfer crashes,
  its outcome depends on the machine's memory or it passes the 5 seconds printing (FS-13, FS-229,
  FS-243, FS-248); `VARY` where it reads memory (FS-245, the addresses printed with `DIAG_SHOW` and
  `DIAG_VERBOSE`).
- `matfile_scipy.py`: 400 matrices written by SciPy's `savemat` (version 4; version 5 plain and
  compressed; doubles, singles, signed and unsigned 8- to 32-bit integers, complex) read by the port's
  `MatlabRead`: the default (`fix`) must read each as SciPy wrote it; FreeSurfer's reading
  (`reproduce`) is counted per format.
- `scanf_lf.c`: glibc's `sscanf("%lf")` as a black box (bits, `errno`, bytes read), the measurements
  behind `larmorx-freesurfer-core::cscan`'s `%lf` test.

Run (inside a memory box; the port and FreeSurfer run on generated inputs):

    validation/probes/fs-io/build.sh <workspace>
    scripts/orchestration/boxed.sh 6 python -P validation/probes/fs-io/check.py --seed 1 --n 1100
    scripts/orchestration/boxed.sh 10 python -P validation/probes/fs-io/check.py --seed 1 --n 4300 --portable
    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-io/check.py --seed 7 --n 86000
    scripts/orchestration/boxed.sh 6 python -P validation/probes/fs-io/matfile_scipy.py

The portable run's hash (`official_sha256_skip`) is `tests/fs_io_official.rs`'s, checked on every
platform. Results (2026-11-13): `docs/findings/freesurfer-io.md`.
