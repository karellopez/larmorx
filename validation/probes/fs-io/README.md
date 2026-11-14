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
- Piece 6 (relay 16, for track D's surfaces): reading volumes and registration files. `mrh` (`MRIreadHeader` with a
  type), `mri` (`MRIreadInfo`), `mrd` (`mri_read` with voxels, called directly: `MRIreadType`'s licence check exits
  without `FREESURFER_HOME`), `mgr` (`mghRead` with or without voxels and a frame), `rrr` (`regio_read_register`), with
  `Gdiag`, the frame-list switch, `FS_SKIP_TAGS` and `FS_MGZIO_USEVOXELBUFREAD`; the MGH files (plain and compressed, whole
  or cut) built by `tests/fs_io/piece6.rs` from the format with every tag kind and `.xfm` files beside them; each volume
  printed as its header fields, transforms, tags' values and an FNV-1a hash of its voxels. Names and messages are printed
  with the fixture's path removed on both sides (`fsio` prints names escaped with an `n` before them; its piece-6
  messages lose the fixture's path), and standard error is oriented first (glibc's `perror` then leaves `errno` alone).
  `SKIP` for formats not ported yet, `.lta` names (fs.volume's `LTAread`), crashes (FS-76, FS-249, FS-253 to FS-258) and
  skips beyond 256 MiB (FS-178); `VARY` for numbers from memory (FS-49, FS-167).
- Piece 7 (relay 17): Bruker ParaVision studies. `bsp` (`splitParameterValue` on one line), `bd3`, `brc`, `bacq`
  (`readBrukerD3proc`, `readBrukerReco`, `readBrukerAcqp` on a generated file, the caller's values at -7), `bvx`
  (`buildVoxToRASTransform` of a given transform on a header of given extents), `bkr` (`brukerRead`, header or voxels)
  and `bmr` (`mri_read` of the study, type 18); the parameter files and studies built by `tests/fs_io/piece7.rs` from
  ParaVision's `##$NAME=value` form (most complete; missing, odd, CR LF, overlong and cut lines; unsupported types; 2dseq
  files of the right length, half, a byte more). `SKIP` for FS-259 and FS-403, `VARY` for FS-401 (values never written).
  Seed 13, 3,815 piece-7 cases: `reproduce` identical on 3,221, 0 different.
- Piece 8 (relay 18): FreeSurfer's 2-D images (`image.cpp`, `filter.cpp`'s pyramid reduction). Each case starts with
  `Gerror`'s value and ends by printing it: `ial` (`ImageAlloc`), `icp` (`ImageCopy`), `ica` (`ImageClearArea`), `ick`
  (`ImageCheckSize`), `iss` (`ImageSetSize`), `icf` (`ImageCopyFrames`), `irs` (`ImageRescale`), `isd`/`isu`
  (`ImageScaleDown`/`ImageScaleUp` called directly), `ird` (`ImageReduce`), `ic1`/`ir1` (`ImageConvolve1d`/
  `ImageReduce1d` with a given kernel); an image is given as `ImageAlloc`'s arguments, its extents in use (-7: as
  allocated), its pixel sizes and its bytes (`N`: none), built by `tests/fs_io/piece8.rs` (up to 13 by 13 pixels and 3
  frames, every format, odd extents, frames and buffers, floats of every kind) and printed as its fields and its bytes
  (hex up to 128, else FNV-1a 64; the probe keeps the length of the buffers it made and forgets an image FreeSurfer is
  about to free). `SKIP` for FS-405 (outside the pixels) and FS-409 (an image used after it is freed). Seed 11, 1,068
  piece-8 cases: `reproduce` identical on 949, 0 different; seed 13, 5,292: identical on 4,605, 0 different.
- Piece 9 (relay 18): fs.core's `ArgumentParser` (`argparse.cpp`, the newer programs' command lines). `apa`: a
  configuration (`A1`/`A2` `addArgument`, `H` `addHelp` with an empty or an XML text; the seven programs' own or
  random ones), a command line (its first word the program's name; made for the configuration half of the time) and
  `exists`/`retrieve` queries; `parsed` and the answers, or `EXIT` with the messages. Built by
  `tests/fs_io/piece9.rs`. `SKIP` where an XML help text is rendered (FreeSurfer's help renderer, `fsPrintHelp.cpp`,
  is not ported yet), `VARY` where the answer depends on the name index's order (libstdc++'s hash order: two names
  match) or on the time (`--all-info`). Seed 13, 2,352 cases: `reproduce` identical on 2,265, 0 different.
- Piece 10 (relay 19): fs.labels' text colour tables and lookups (`colortab.cpp`). `cra` (`CTABreadASCII2` with or
  without the duplicate-name check; `ctabDuplicates` set to -7 before and printed after), `crt`
  (`CTABreadASCIIttHeader`), `ctd` (`CTABreadDefault` with `FREESURFER_HOME`, `SUBJECTS_DIR`, `FS_DESIRED_BVEC_SPACE`
  set or unset: `N` or `x<hex>`), `cal` (`setRandomSeed`, then `CTABalloc`), `cun` (`CTABunique`), `ccr`
  (`CTABcountRepeats`), `cq` (a list of lookups: `v t e h r n a x y i f F g m d`, each answer after the function's
  status, outputs preset to -7), `cpw` (`CTABwriteFileASCII` and the file's bytes, `Gdiag` given), `cps`
  (`CTABprintASCII` to standard output), `cts` (`TissueTypeSchema` of a given table or of the default one), `cme`
  (`CTABmerge`). A table is given as `<version> <nentries> <idbase> <fname> <k>` and `k` entries `<index> <name> <r>
  <g> <b> <a> <tissue type> <count>`; printed as its fields, its non-empty entries (float colours as bits; tissue type
  and count left out for `CTABalloc`'s entries, memory contents in FreeSurfer) and its tissue-type table after `T`;
  `Gerror` last. Table files are built by `tests/fs_io/piece10.rs` (entry lines with every separator, sign, overflow
  and stray character; comments, CR LF, long lines, NUL bytes, `#ctTType` lines whole and broken, lines of vertical
  tabs and form feeds; the default table's first rows). `errno` is set to 0 after each case's files are made (making
  them leaves `EISDIR`). `SKIP` where FreeSurfer crashes or writes outside its tables (FS-413, FS-414's crashes and
  outside stores, FS-416, FS-417, FS-418), where the port refuses a size the machine decides (FS-420), or a name
  overflows `stricmp` (FS-11); `VARY` for FS-414's memory never written. Seed 13, 1,664 piece-10 cases: `reproduce`
  identical on 1,415, 0 different.
  Registration files (3.1-34): `rpr` (`regio_print_register` to standard output), `rwr` (`regio_write_register` and
  the file's bytes), `rrx` (`regio_read_mincxfm` with or without the file information), `rwx` (`regio_write_mincxfm`,
  `FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING` set, `Progname` `fsio`), `rrm` (`regio_read_registermat`); the
  matrices as `4` and 16 float bits. Seed 13, 710 such cases: identical in both modes.
- Piece 11 (relay 20): fs.labels' labels, control points, annotation tables and point sets. `lrd` (`LabelRead` with or
  without a subject; `SUBJECTS_DIR` given, unset or wrong; label files built by `tests/fs_io/piece11.rs` with every
  first-line, count and point-line form, and the subject's `talairach.xfm`), `lwr` (`LabelWrite` of a given label, then
  every file under `w` with its bytes), `lal` (`LabelAlloc`), `lre` (`LabelRealloc`), `lcp` (`LabelCopy`), `ldu`
  (`LabelRemoveDuplicates` with `Gdiag`), `lhv` (`LabelHasVertex`), `crd` (`MRIreadControlPoints` of control-point and
  `.label` files; count and `useRealRAS` preset to -7), `cwr` (`MRIwriteControlPoints`, the creation time fixed by
  `FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING`, the user's name shown as `U`), `cfp` (`MRIfillPoints` on a small
  volume given by its extent, type, voxel sizes, axes, centre and voxels; its voxels as FNV-1a 64), `atr` (the
  annotation table from files, `FREESURFER_HOME` and `annotation_table_file` given, then `R`/`N` reads and the
  lookups `i n a m` and printers `p c`), `ats` (`set_atable_from_ctable` of a piece-10 table, then the same), `pss`
  (an `fsPointSet` built, then `save`, `save_as_ctrlpoint`, `print`, `printCoords`, `writeCoords`,
  `writeCentroidTable`, `get`, `set`, `add`, `remove`, `clear`; the files under `w`). A label is printed as its fields,
  places (FNV-1a 64 beyond 2,000 characters), vertex indices and transforms. `SKIP` for FS-423, FS-424, FS-426,
  FS-427, FS-429, FS-430, FS-431 and FS-11. Seed 13, 2,678 piece-11 cases: `reproduce` identical on 2,594, 0
  different; seed 7, 8,034: identical on 7,783, 0 different.
- Piece 12 (relay 21): `mgi` (`mri_read` of generated GIFTI files, as `mrd` with the volume flag; the volume
  printed as piece 6's with its colour table), `mwr` (`MRIwrite` of one to three volumes given by extent, type,
  frames, geometry, repetition time, name suffix and voxels, with `FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING`
  and `SURFER_SIDEDOOR` set; every file under `w` printed, `.gii` files in any case with fs-surfio's `gii_normal`:
  the user name `U`, each payload Base64-decoded and inflated), `lps` (`loadfsPointSet`; the set's `vox2ras`, points),
  `hlp` (`outputHelpXml` of a text or NULL; the text printed and the status). The GIFTI files, point-set files and
  help texts are built by `tests/fs_io/piece12.rs`. `SKIP` for the writers not ported yet, FS-521, FS-255, the
  GIFTI reader's crashes, FS-434 and help texts that are not well formed; `VARY` for FS-214's address. Seed 7, 1,800
  `mgi`/`mwr`/`lps` cases: `reproduce` identical on 1,589, 0 different; FreeSurfer 8.2.0's 86 `.help.xml` files
  (`hlp x<hex of the file>`): identical; seed 13, 594 `hlp` cases: identical on 465, 0 different.
- Piece 13 (relay 22): `lcv` (`LabelToScannerRAS`, `LabelToSurfaceRAS`, `LabelToVoxel`, `LabelVoxelToSurfaceRAS`
  of a label given as piece 11's, a volume header as piece 11's `cfp`, into a new label, itself or a given label,
  optionally with a surface, `MRISalloc(0, 0)` with its volume geometry read as piece 2's; the result printed as
  piece 11's labels), `mwf` (`MRIwriteFrame` of piece 12's volumes; the status, `errno`, every file under `w` as
  `mwr`). The port's side is in larmorx-freesurfer-volume (its `tests/fs_io13/piece13.rs`, including fs.io's
  `cases.rs`; example `fs_io13_probe`): `check.py --package larmorx-freesurfer-volume --example fs_io13_probe`.
  `SKIP` for the writers not ported yet and FS-429. Seed 7, 1,000 cases: `reproduce` identical on 947, 0 different;
  seed 13, 3,000: 2,823 identical, 0 different. The recorded test `fs_io13_official` (seed 1's 1,200: `--seed 1
  --n 1200 --portable --light`).
- Piece 14 (track A relay 23): `mgw` (`mghWrite` with a chosen frame and an intent, or `MRIwrite` with b-values beside
  the volume, `FS_MGZIO_USEVOXELBUFWRITE` set or not) of a volume given with every field the MGH writer writes (`read_mri14`:
  extent, type, frames, voxels or none, geometry and RAS flag, TR, TE, TI, field of view, flip angle, field strength,
  degrees of freedom, the buffer's bytes, the transform's name, the phase-encoding direction, command lines, the first
  frame's label and threshold, the auto-align and original RAS-to-voxel matrices, a GCA morph's fields, a piece-10
  colour table, b-values); printed: the status, `errno`, the volume's intent, version and voxels after the call, every
  file under `w`, and one case in two the file read back with `mghRead` (piece 6's printing). Every written file whose
  bytes start a gzip stream is printed inflated (`gz:` and the stream's bytes: zlib 1.2.11's compressed bytes here,
  zlib-rs's in the port), in pieces 12, 13 and 14 alike. After the write `fsio` calls `fflush(NULL)`, so a file
  FreeSurfer leaves open (FS-439) shows what the program's end would write. `SKIP` for FS-58, FS-441, FS-444, FS-445,
  FS-446 and the read back of files whose header does not match their data (piece 6's rules); `VARY` for FS-49,
  FS-167, FS-421. Seed 7, 588 `mgw` cases: `reproduce` identical on 495, 0 different; seed 13, 10,294 cases of `mgw`,
  `mwr`, `mgr`, `mrd`, `mrh`, `mri`, `mgi`: identical on 8,056, 0 different. Since relay 23 `mgi` prints the GIFTI
  reader's colour-table entries without their tissue type and count (memory contents in FreeSurfer). NIfTI (`niiWrite`) through
  `MRIwrite` with `.nii`, `.nii.gz`, `.NII` names, the ico7 shape and 32,768 rows, axes permuted with signs: seed 13,
  3,920 `mgw`/`mwr` cases: identical on 3,603, 0 different. Since relay 24 `mgw`'s `how` 2 calls
  `MRIwriteType(name, NIFTI1_FILE, intent)` (`nifti1Write`: the `.hdr`/`.img` pair) with the names `w/o.img`,
  `w/o.hdr`, `w/o`, `w/O.IMG`, `w/a.b.img.gz`, `w/.hdr`, `w/no/o.img`: seed 13, 1,470 `mgw` cases (195 pairs):
  identical on 1,322, 0 different.
- Piece 15 (track A relay 25): `nrd` and `nrh` (`mri_read` with voxels and `MRIreadHeader`, as piece 6's `mrd` and
  `mrh`, after a `Gdiag_no`: `niiPrintHdr` above 0) of NIfTI files built by `tests/fs_io/piece15.rs`: headers field by
  field (both byte orders, every data type scaled or not, extents past `dim[0]`, `glmin` columns, the ico7 shapes,
  units, slice codes, forms with NaN quaternions, odd voxel offsets, texts without NUL bytes), extensions (none,
  another program's, odd codes, FreeSurfer's with random tags, the file ending at the extender), voxels cut or
  doubled; and files the port's `niiWrite` wrote from piece 14's volumes, then changed; `.nii`, `.nii.gz`, NIfTI-1
  pairs by `.hdr` names and stems. Piece 6's environment bits 4 (`TAG_RAS_XFORM_DEBUG`) and 8
  (`VOL_GEOM_THRESH=0.25`). Since relay 25 piece 14's read back uses the writer's reader (`mghRead` after
  `mghWrite`, `mri_read` after `MRIwrite`, `mri_read` with `NIFTI1_FILE` after the pair writer). `SKIP` for FS-644
  (FreeSurfer crashes) and `.img` names (`analyzeRead`, not ported); `VARY` for FS-641 and FS-642 (memory). Seed 13,
  5,769 `nrd`/`nrh`/`mgw` cases: `reproduce` identical on 5,323, 0 different; seed 7, 8,075 with piece 6's readers:
  6,919 identical, 0 different.
- Piece 16 (track A relay 26): `ard` and `arh` (`mri_read` with voxels and `MRIreadHeader`, as piece 6's `mrd` and
  `mrh`, after a `Gdiag_no`, `N_Zero_Pad_Input` and `SPM_START_FRAME`, `N` for unset; piece 6's environment bits 16
  `ATROPHY_SIMULATOR` and 32 `FS_ANALYZE_NO_RESCALE`) of ANALYZE 7.5 pairs and SPM series built by
  `tests/fs_io/piece16.rs` field by field (both byte orders, `sizeof_hdr` 0, 349 or 540 with big-endian fields, every
  data type taken and some refused, extents -1 to 4, odd voxel sizes, offsets, scale factors and orientation codes,
  voxel files cut or longer), `.mat` files (MATLAB version 4 matrices of 4 x 4 and other shapes, piece 5's files),
  NIfTI-1 headers in the `.hdr` (a pair, a single file, a series of pairs, a `.nii` beside the `.img`) and missing
  files; `awr` (piece 14's `mgw` after a `Gdiag_no`, `N_Zero_Pad_Output` and the read back's `N_Zero_Pad_Input`, with
  `how` 3 `MRIwriteType(MRI_ANALYZE_FILE)` and 4 `MRIwriteType(MRI_ANALYZE4D_FILE)`; `N_Zero_Pad_Output` printed after
  the write; the read back with the writer's type). `SKIP` for FS-646, FS-648, FS-650, FS-651 (FreeSurfer crashes or
  aborts), piece 5's classes for the `.mat` reader, piece 14's for the writers (FS-58, FS-53); `VARY` for FS-645 and
  `DIAG_SHOW`'s addresses. Seed 7, 840 piece-16 cases: `reproduce` identical on 758, 0 different; seed 13, 7,569 cases
  with pieces 6, 14 and 15's readers and writers: 6,692 identical, 0 different. Since relay 26 piece 15's `.img` names
  reach `analyzeRead` (compared).
- `nifti1_long_names.py` (relay 24): nine `mgw` cases of the pair writer under names of 1,019 to 1,027 bytes in four
  nested folders (FS-451: the names cut to 1,023 bytes with FreeSurfer's two messages, one file for both from a stem
  of 1,022 bytes, the C library's overflow abort from a name of 1,024 bytes), compared in both modes:
  `python -P validation/probes/fs-io/nifti1_long_names.py` (Linux; prints `PASS` when `reproduce` is identical on
  every case and `fix` differs only with FS-451).
- `fs49_slot_cases.py` (relay 25 of track A): 16 hand-made `tr` and `ctr` cases where the file cuts a number
  read right after another (FS-49: `TAGreadStart`'s tag-30 length, the binary colour tables' runs of numbers on
  C streams), and after an allocation, a name or a print (memory contents), compared with `reproduce`:
  `python -P validation/probes/fs-io/fs49_slot_cases.py` (prints `PASS` when every line is identical or marked
  `SKIP`/`VARY` by the port; 13 identical, 3 memory contents, 2026-11-14).
- `matfile_scipy.py`: 400 matrices written by SciPy's `savemat` (version 4; version 5 plain and
  compressed; doubles, singles, signed and unsigned 8- to 32-bit integers, complex) read by the port's
  `MatlabRead`: the default (`fix`) must read each as SciPy wrote it; FreeSurfer's reading
  (`reproduce`) is counted per format.
- `scanf_lf.c`: glibc's `sscanf("%lf")` as a black box (bits, `errno`, bytes read), the measurements
  behind `larmorx-freesurfer-core::cscan`'s `%lf` test.

Run (inside a memory box; the port and FreeSurfer run on generated inputs):

    validation/probes/fs-io/build.sh <workspace>
    scripts/orchestration/boxed.sh 6 python -P validation/probes/fs-io/check.py --seed 1 --n 1100
    scripts/orchestration/boxed.sh 10 python -P validation/probes/fs-io/check.py --seed 1 --n 4800 --portable
    scripts/orchestration/boxed.sh 10 python -P validation/probes/fs-io/check.py --seed 1 --n 4800 --portable --light
    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-io/check.py --seed 7 --n 86000
    scripts/orchestration/boxed.sh 6 python -P validation/probes/fs-io/matfile_scipy.py

The light run's hash (`official_sha256_skip` with `--light`: the portable cases without the six that
take seconds in a debug build, `cases::HEAVY_SEED1`, measured by `tests/fs_io_heavy.rs`) is
`tests/fs_io_official.rs`'s, checked on every platform; the portable run keeps the six. Results (2026-11-13): `docs/findings/freesurfer-io.md`.
