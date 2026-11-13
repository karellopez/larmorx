# FreeSurfer 8.2.0's side of the permissive pieces (track C of FreeSurfer's foundations)

**Closed to FreeSurfer's clean-room implementers** (CLAUDE.md rule 1: `docs/findings/freesurfer-*.md`).
The open files of these pieces (`larmorx-core`'s and `larmorx-io`'s code, comments and
`PROVENANCE.md`, `docs/findings/x87-extended.md`, `vnl-random.md`, `vnl-netlib.md`, `minc-xfm.md`,
`nifti-znz.md`, the register's open sections) describe only what the upstreams (x87, POSIX, glibc as a
black box, VXL in ITK 4.13.2, MINC 1.5.1, nifticlib 2.0.0, zlib 1.2.11) do, and the checks' numbers
(the rule "open files carry behaviour only", 2026-11-13). What those files said about FreeSurfer
itself (which of its functions call what, its seeds, settings, casts, build flags, its copies'
changes, the paths of `docs/freesurfer/`) is here, moved on 2026-11-13 by track C's relay 4. Sources:
`docs/freesurfer/` (the preparation's inventory, cards and variation study), the unit oracle
(`oracles/freesurfer-8.2.0-unit`, its `compile_commands.json`), and black-box runs of FreeSurfer's
libraries through the probes.

## The x87's `long double` (`larmorx_core::x87::F80`, item 3.1-03)

- FreeSurfer 8.2.0 computes in `long double` in 12 reached functions (`docs/freesurfer/variation.md`
  §1); GCC 8.4.0 compiles them into x87 instructions.
- The six arithmetic patterns of `validation/probes/x87-extended` (sums of `double` products, a mean,
  variance and standard deviation, cross products of `float` edge vectors and their length, a unit
  vector and its gradient, `float` kernel weights over their sum, squared length differences) are
  those functions' patterns, compiled by GCC 8.4.0 with FreeSurfer's C++ flags (`-std=c++11 -O3
  -DNDEBUG -fopenmp -fno-builtin -fPIC`, the unit oracle's `compile_commands.json`). `F80` composed
  in the source's order gives their bits; in `double` 1.8 % to 71 % of the outputs change.
- The project's official FreeSurfer runs ran on the AMD machine whose x87 the probe measured.

## `vnl_random` and `drand48` (item 3.1-04)

- `OpenRan1` (the generator of FreeSurfer's `-seed` options) is a static `vnl_random`, constructed
  from a zero-initialised static `long` (seed 0) before its first reseed; FreeSurfer passes its seed
  as a `long` (only its low 32 bits matter) and draws `(float) drand64(0.0, 1.0)`. Seeds the
  programs use: 1234 (`mri_normalize`, `mris_smooth`, `mris_ca_label -seed 1234`) and 53
  (`AntsN4BiasFieldCorrectionFs`'s `drand48` seed); the probe `validation/probes/fs-random` includes
  both.
- Every official binary that holds `drand64` (65 of the 72 default-path programs, GCC 8.4.0) divides
  the *first* `lrand32` by `0xffffffff` (`docs/freesurfer/variation.md` §3,
  `validation/investigations/freesurfer-inventory/drand64_order.sh`); GCC 8.4.0 compiling ITK
  4.13.2's `vnl_random.cxx` with FreeSurfer's flags does the same (`fs-random`).
- ITK-101 (a copy loses the cached normal) and ITK-102 (`lrand32(lower, upper)`'s overflow) are not
  reached: FreeSurfer's one generator is never copied and never draws a range.
- The replays that use these generators (`mri_normalize`, `mris_smooth`, `mris_ca_label -seed
  1234`) wait for those programs' replicas.

## VNL and netlib (items 3.1-07 and later)

Where FreeSurfer calls them (the call sites the cards name):

- `utils/numerics.cpp`: `OpenLUMatrixInverse` (`vnl_det` and `vnl_inverse` of
  `vnl_matrix_fixed<float, 2..4>`, `vnl_qr<float>` above 4 × 4, its determinant compared with 0
  first), `OpenMatrixDeterminant` (`vnl_det` of `float` 2 × 2 to 4 × 4, `vnl_determinant<float>`
  above), `OpenQRdecomposition` (`vnl_qr<float>`'s `Q()`, `R()`), `OpenSvdcmp` (`vnl_svd<float>`'s
  `U`, `W`, `V`), `OpenEigenSystem` (`vnl_symmetric_eigensystem<float>`'s `V` and eigenvalues),
  `OpenNonSymmetricEigenSystem` (`vnl_real_eigensystem`'s `Vreal`, rounded to float, and the real
  parts of `D`), `MatrixSqrt` (`vnl_inverse` of `vnl_matrix_fixed<double, 3, 3>`, Denman and
  Beavers' iteration), `sc_linalg_cholesky_decomp` (`vnl_cholesky` in its default `verbose` mode,
  `rank_deficiency()`, `upper_triangle()`), `OpenPowell`/`OpenPowell2` (FreeSurfer's `fs_powell`,
  which steps with the deprecated `vnl_brent`: `bracket_minimum(&ax, &xx, &bx)` from `(0, step, 0)`,
  then `minimize_given_bounds(bx, xx, ax, linmin_xtol, &xx)`), `OpenDFPMin` (FreeSurfer's `fs_lbfgs`,
  which drives `v3p_netlib_lbfgs_` with `memory = 7`, line-search accuracy 0.1, `xtol` 1e-16,
  `diagco` false, `IPRINT = {-1, 0}`, or `{1, 1}` under `DIAG_VERBOSE_ON`; `mri_em_register`,
  `mri_ca_register`).
- `mri_robust_register/`: `vnl_powell` (`RegPowell.cpp`), `vnl_matrix_inverse<double>`,
  `vnl_determinant<double>`, `vnl_qr<T>`, `vnl_svd<double>` and, on option paths, complex
  eigensystems and complex SVDs (not ported in 3.1-07).
- `AntsN4BiasFieldCorrectionFs`: `vnl_fft_1d<double>` of N4's histogram, 200 bins padded to 512.
- ITK-103's measurement used `OpenDFPMin`'s settings (memory 7, accuracy 0.1, `xtol` 1e-16) beside
  VNL's defaults; ITK-104's diagonal is never supplied by FreeSurfer (`diagco` false); ITK-105 and
  ITK-106 print only under `DIAG_VERBOSE_ON`; ITK-110's unwritten columns are copied by
  `OpenNonSymmetricEigenSystem` into its float output; ITK-107's zero-determinant inverse:
  `OpenLUMatrixInverse` tests the determinant first, `MatrixSqrt` does not.
- `fs_vnl`'s own `uninit` (`p[i] += xx · dx[i]`) and line function (track A's
  `larmorx-freesurfer-core`) compute with plain arithmetic: on a cost that falls without end their
  NaNs would take AArch64's sign; FreeSurfer's costs are bounded.
- ITK-114 to ITK-116 (relay 4): none is reached on FreeSurfer's default paths; FS-47 and FS-48 (track
  A) are FreeSurfer's side of ITK-115 and ITK-116 (`OpenEigenSystem`, `OpenSvdcmp` on infinite or
  NaN entries): with `symmetric_eigensystem_f32_full` and `vnl_svd_f32_full` they can follow
  FreeSurfer on the matrices it finishes. LINPACK's `info` on a failed SVD, which track A measured as
  `min(rows + 1, cols)` on every failing case, is the `m` of `ssvdc`'s loop when it stops.
- The unit oracle's VNL (`validation/probes/fs-vnl`) is ITK 4.13.2 built with ITK's Release flags
  (`-O3 -DNDEBUG -msse2 -mfpmath=sse`, FreeSurfer's ITK configuration); `shim.cxx` is compiled with
  FreeSurfer's own flags for `utils/numerics.cpp`.

## MINC's `.xfm` (item 3.1-14)

- FreeSurfer's `packages/minc` (15 files) is MNI's volume_io reworked: its `minc_gaussian.c` says
  "BASED ON minc-1.5.1"; 66 % of `minc_volume_io.c`'s and 75 % of `minc_files.c`'s non-blank lines
  are lines of MINC 1.5.1's `volume_io` (the same share for libminc 2.1.12); 8 % of
  `minc_gaussian.c`'s (its 4 × 4 inverse is FreeSurfer's own). Counted without reading FreeSurfer's
  files (numbers only).
- What FreeSurfer reaches (the call graph's core view): `input_transform_file` and what it calls,
  `get_linear_transform_ptr`, `get_inverse_linear_transform_ptr`, `copy_general_transform`,
  `delete_general_transform`, `transform_point` (`utils/mriio.cpp`'s `talairach.xfm`,
  `utils/mri.cpp`, `utils/label.cpp`, `utils/stats.cpp`, `mri_add_xform_to_header`); FreeSurfer
  writes `.xfm` files with its own code (`utils/transform.cpp`, `registerio.cpp`: track A), so
  MINC's writer is ported for larmorx's own use and checked against MINC 1.5.1 only.
- FreeSurfer's copy, by behaviour (the probe's `ProbeFs`, `validation/probes/fs-minc-nifti/src/
  dialect.rs`, makes the port give its results exactly: 20,000 files, 40,000 names, 0
  differences), for `larmorx-freesurfer`'s own dialect (track A):
  1. Only `Linear` transforms: any other type is `input_transform(): invalid transform type <name>.`
     (MINC: `input_transform(): invalid transform type.` for an unknown one; thin-plate splines and
     grids read).
  2. A number that does not convert prints `<token> is not a valid double` first.
  3. A mismatched keyword prints `Expected "<keyword> =" got str:<token>` (`(null)` at the end of
     the file) wherever MINC prints `Expected "<keyword> ="` or nothing.
  4. A NUL byte ends a token (the rest of it is read and lost); messages are printed with their NUL
     bytes.
  5. `~user` is always `.` (volume_io built without `getpwnam`; MINC: the user's home directory).
  6. Its own 4 × 4 inverse (`invert_4x4_matrix`; singular: `invert_4x4_matrix failed, Some col is
     all zeros` and the identity). It gives MINC's Gaussian elimination's bits for all 16 elements
     in 1,045 of 12,406 affine matrices (last row 0 0 0 1), 6,095 of 11,205 small-integer ones, 640
     of 21,950 general ones: a replica of FreeSurfer's results needs FreeSurfer's routine.
  7. A compressed `.xfm` crashes (a double free after `gunzip`), and with `TMPDIR` unset its
     temporary file goes to `/` (`Unable to create temporary file /minc-XXXXXX`).
  8. An empty `$VAR` at the start of a name expands to nothing (no crash: FS-75 is MINC's).
  9. Tabs end no token, as in MINC (`1\t0 0 1` is one token).
- The probe's numbers against FreeSurfer's `libminc.a`: `xfm` 20,000 files 0 differences, `ops`
  `concat_transforms` and `transform_point` 100,000 each 0, `expand_filename` 39,673 names 0;
  `compute_transform_inverse`: the measurement in item 6. MINC 1.5.1's files in the probe are built
  with GCC 8.4.0 and FreeSurfer's flags.
- FS-71 to FS-75 in FreeSurfer: none on its default paths (`talairach.xfm` is never compressed; it
  reads linear transforms only; its copy expands an empty `$VAR` to nothing).

## nifticlib and znzlib (item 3.1-15)

- FreeSurfer's `nifti1_io.c` matches nifticlib 2.0.0 in 95.1 % of its tokens (94.6 % for nifti_clib
  3.0.1, 94.9 % for ITK 4.13.2's copy), its `znzlib.c` in 97.2 % (the same as ITK 4.13.2's; 97.0 %
  for 3.0.1): counted without reading FreeSurfer's files.
- What FreeSurfer reaches (the core view): `nifti_findhdrname`, `is_nifti_file` and their helpers,
  and every `znz*` function its MGZ and NIfTI code uses (`znzopen`, `znzread`, `znzwrite`, `znzseek`,
  `znztell`, `znzeof`, `znzgetc`, `znzclose`); its official binaries load zlib 1.2.11 at run time
  (`libz.so.1`, Ubuntu 22.04's).
- FreeSurfer's copy, by behaviour (the probe's FreeSurfer model, `Flavour::FreeSurfer`: 0
  differences on 6,000 name cases, 3,000 read scripts, 2,000 write scripts at scale 1):
  1. No upper-case extensions (`allow_upper_fext` off), `nifti_is_gzfile` takes `.gz` only (2.0.0
     also `.GZ`), and `.nia.gz` is a compressed extension (`NiftiOptions { allow_upper_fext: false,
     upper_case_gz: false, nia_gz: true }`).
  2. Its `znzread` on a compressed file is one `gzread` of `size * nmemb` bytes divided by `size`:
     the complete members, no "short" warning, and zlib's -1 becomes `SIZE_MAX / size`
     (2305843009213693951 for 8-byte members); `znzwrite` likewise; `znzrewind` is `gzrewind`
     (2.0.0: `gzseek(0, SEEK_SET)`, which keeps `gzeof`'s flag after the end of an empty file).
     FS-77's correction (the complete members) is what FreeSurfer's copy does.
- FS-76 to FS-78 in FreeSurfer: none on its default paths (its MGZ and NIfTI writers in `mriio.cpp`
  never seek before writing). Byte-identical MGZ outputs need zlib 1.2.11's deflate (the
  preparation's item P-17).

## C's `sscanf` and `strtod` (`larmorx_core::cscan`)

- FreeSurfer reads `.xfm` numbers through MINC's `sscanf("%lf")` and its matrix files through
  `MatrixReadTxt`, `MatrixAsciiReadFrom` (track A's `matrix_io`, on `cscan`).

## GIFTI: gifticlib and expat (items 3.1-09 to 3.1-12)

- FreeSurfer 8.2.0's `packages/gifti` is gifticlib 1.12 (github.com/NIFTI-Imaging/gifti_clib at
  8ec2a95, 2012-06-15) reformatted, with two changes that alter behaviour (token comparison,
  `<scratch>` tools of relay 5; confirmed by the probe below):
  1. `gifti_valid_gifti_image`'s version check reads the image's `Version` and "1.0" with
     `sscanf("%f")` and compares the two `float`s ("NJS: original code checks string, not value,
     so '1' fails"); when they differ it prints `** invalid: gim version = %f, expected %f` and
     counts an error. An image without a `Version` reads 0 on both sides: valid. A version of
     " 1.0", "1", "1.00" is valid; "1.0x" is too (the number read stops at the 'x'); "nan" and
     "2.0" are not. `larmorx_io::gifti::Dialect::check_version`.
  2. `push_cstm` accepts a coordinate system under a `NIFTI_INTENT_TIME_SERIES` array as well as
     a `NIFTI_INTENT_POINTSET` one; the message names both: `** DA[%d] has coordsys with intent %s
     (should be %s or %s)`. `Dialect::coordsys_intent_message`.
  Track A's replica of FreeSurfer's GIFTI calls supplies this dialect (the probe's `ProbeFs` is a
  model of it; `larmorx_core::cscan::sscanf_f32` reads the `float`s as glibc does).
- FreeSurfer's `packages/expat` is expat 2.0.1 (libexpat tag R_2_0_1) built with `XML_DTD`,
  `XML_NS` and `XML_CONTEXT_BYTES` 1024 (its `expat_config.h`), without one `memmove` guard; no
  difference in behaviour was found (the probe below: every event and error equal).
- `validation/probes/fs-gifti` compares the port with gifticlib 1.12 + expat 2.0.1 + nifticlib
  2.0.0 built from their pinned sources and with FreeSurfer's copies as the unit oracle built them
  (`libgifti.a`, `libexpat.a`, `libnifti.a`; GCC 8.4.0, FreeSurfer's flags); its README has the
  numbers. FreeSurfer reads and writes GIFTI through `gifti_read_image`, `gifti_write_image` and
  `gifti_valid_gifti_image` (its `utils/gifti.cpp`, track A).

## CTN's DICOM library (item 3.1-16)

- FreeSurfer 8.2.0's `packages/dicom` is CTN 3.0.6's object library (`dcm.c`, `dcmdict.c`,
  `dcmcond.c`, `dcmsupport.c`, `condition.c`, `lst.c`, `ctnthread.c`) reformatted, with casts, a
  note "Modified Dec 15 2014 by DNG to handle double and float" (`DCM_DumpElements` and the
  element union's `fl`/`fd`: nothing the reading reaches) and three changes the reading reaches,
  measured by the probe: (1) `swapInPlace` swaps UL and SL of big-endian data in 2-byte pairs (the
  4-byte reversal kept under `#if 0`, with a comment on UL values whose length is not a multiple
  of 4): FS-94, which also makes FS-95's crash unreachable in FreeSurfer; (2) the
  `DCM_ELEMENTLENGTHERROR` message's continuation line carries 35 spaces ("... of" + 36 blanks +
  "data in stream or file in %s", the line splice of a reformatted source); (3) `newElementItem`
  computes the room with a `long`, so an undefined length (FS-91) allocates 4 GiB + 152 bytes
  where CTN 3.0.6 allocates 152 (neither is ever written). The dictionary is CTN 3.0.6's, entry for
  entry (`gen_dict.sh` compares them); the platform header defines `LONGSIZE 32` with `U32` an
  `unsigned int` (the same types as CTN on Linux x86-64), no threads.
- `IsDICOM` (its `utils/DICOMRead.cpp`, track A's `fs.io`) opens a file four times, as
  `mri_identify` asks for every volume it cannot tell by name: `DCM_PART10FILE`,
  `DCM_ORDERLITTLEENDIAN`, `DCM_ORDERBIGENDIAN`, `DCM_FORMATCONVERSION`, each with
  `DCM_ACCEPTVRMISMATCH`, closing the object after each failure and clearing the condition stack
  before and after (`COND_PopCondition(1)`). The fourth attempt can never succeed (no byte order:
  "DCM illegal stream option: Byte order"), and after a failed attempt the close finds the handle
  empty when the reader had closed the object ("NULL object passed to routine DCM_CloseObject").
  With `FS_DICOM_DEBUG` set it prints the conditions (`DCMPrintCond`, and `COND_DumpConditions`
  after the first attempt, which empties the stack). `GetObjectFromFile` repeats the four opens
  with the caller's options and dumps the stack when all fail; `mri_probedicom` opens three ways.
  None of FreeSurfer's calls passes another option the port does not follow.
- A file that is DICOM to CTN in its implicit little-endian attempt is any file whose first bytes
  read as ascending tags with lengths that fit: a NIfTI-1 header is not (its 348 makes
  (015c,0000) of length 0, then (0000,0000) out of order), nor is an MGH header (its version and
  width, read as a tag and a length, run past the file). FreeSurfer asks `IsDICOM` only for names
  `mri_identify` cannot place by their extension.
- The probe `validation/probes/fs-dicom` compares the port with the unit oracle's `libdicom.a`
  through the probe's `FsDialect` (the two changes above): its README has the numbers.

## nlohmann/json (items 3.1-19 and 3.1-20)

- FreeSurfer 8.2.0's `include/json.h` is nlohmann/json 3.2.0's single header (its version macros
  say 3.2.0) with later type traits backported (`void_t`, `is_detected`, `is_basic_json` in the
  conversions) and one change of behaviour: `operator[]`'s `type_error.305` reads "cannot use
  operator[] with a string argument with <type>" for a key and "... with a numeric argument with
  <type>" for an index (3.2.0: "cannot use operator[] with <type>"); the lexer, parser,
  serializer and Grisu2 are 3.2.0's line for line. The probe's `FsDialect`
  (`validation/probes/fs-json/src/main.rs`) is the model for track A's dialect.
- Callers: `utils/pointset.cpp` (`loadfsPointSet`: `is >> j`, `j["vox2ras"]` to a string, a
  range-for over `j["points"]`, `coord["x"]` to `double`, `point["legacy_stat"]`);
  `mris_reposition_surface` and `utils/targetpointset.cpp`, `utils/mrisurf_sseTerms.cpp`
  (`MRISpointSetLocationError`), `mris_place_surface` (`istream >> j`, `["points"].size()`,
  `js_pts[i]["coordinates"]["x"].get<float>()`, `js["vox2ras"].get<std::string>()`): reading
  Freeview's point sets, always through `operator>>` (not strict: what follows the value is
  ignored). `mri_seg_overlap` writes its report with `out << std::setw(4) << json` (pretty, indent
  4, `std::map` order: "inputs", "measures", "names"; numbers by Grisu2). `fsPointSet::save`
  writes its JSON by hand with `std::ofstream` (not this library).

## miniz, libtiff, NrrdIO, Cephes and the FNV hash (item 3.1-18); argparse (item 3.1-21)

What the queue's reach numbers (`docs/freesurfer/code.toml`, the call graph's foundations) count for
these files, checked against the build (relay 8, 2026-11-13; the call graph:
`validation/investigations/freesurfer-inventory/callgraph.py --core --why`):

- **miniz** (`packages/dcm2niix/miniz.c`, 65 programs by the call graph): the call graph resolves
  `compress2` in `gifti_xml.c` (`ewrite_data`) and `uncompress` in `utils/matfile.cpp`
  (`znzMatReadHeader`) to miniz's `#define compress2 mz_compress2` and `#define uncompress
  mz_uncompress`; but those macros live only inside `nii_dicom_batch.cpp`, the one file that
  includes `miniz.c` (with `MiniZ` defined, as FreeSurfer's build does: `BUILD_SCRIBBLE` off), and
  the official programs import `compress2`, `deflate`, `inflate` from the system's zlib (`nm -D
  mri_cc`). miniz's code is linked into every program with dcm2niix (the wrapper reaches
  `nii_dicom_batch.cpp`), and runs on one path: `geProtocolBlock` (GE's gzip-compressed protocol
  block, read when dcm2niix converts a GE DICOM series: `mz_inflateInit2(-15)`, one
  `mz_inflate(MZ_SYNC_FLUSH)` with the whole block after its gzip header and before its 8-byte
  trailer, `avail_out` the trailer's ISIZE, success only on `MZ_STREAM_END`, the CRC never
  checked). miniz's compressor runs only in dcm2niix's writers (`writeNiiGz`, the MGH and JNIfTI
  writers) when `opts.isGz` is set, which `dcm2niix_fswrapper` sets false: never in FreeSurfer.
  So only the inflater is ported (`larmorx_core::miniz`). FreeSurfer's copy is miniz 1.15 r4
  (GitHub richgel999/miniz commit 28f5066e, 2013-10-14) with a 5-line note on top, three comment
  spellings and three `#pragma unused` lines in the ZIP functions (`diff`).
- **libtiff** (`tif_error.c`, `tif_unix.c`, `tif_fax3.c`, 7 programs): the call graph matches
  `extension(...)` in `mri_cc.cpp`'s `find_corpus_callosum` to `tif_fax3.c`'s file-local macro
  `#define extension(a0) Fax3Extension(module, tif, a0)`: not a call. libtiff runs only when a
  program reads a TIFF volume (`mri_read`'s TIFF type), an unused format. FreeSurfer's three files
  are libtiff 3.6.1's (`diff`: identical); its `tiffvers.h` says 3.6.1 (2003-12-26).
- **NrrdIO** (`754.c`'s `airFloatQNaN`, 65 programs): reached through `AIR_NAN` in
  `simple.c`'s `ERROR` macro, which the call graph matches to MINC's `ERROR` in
  `minc_volume_io.c`: not a call. NrrdIO runs only reading NRRD files (unused). FreeSurfer
  builds it with `TEEM_QNANHIBIT=1` (its CMake file); its NrrdIO is Teem 1.9.0's.
- **Cephes** (`const.c`'s `PI`, 65 programs): `utils/mri.cpp` (`ham_sinc`) writes `PI`, which
  FreeSurfer's `include/const.h` defines as `M_PI`; the call graph matched it to Cephes' global
  `double PI`. No Cephes function is on a default path. The global's bits are `M_PI`'s
  (0x400921fb54442d18, the probe): nothing to port.
- **FNV** (`include/fnv_hash.h`, 39 programs; `include/fnvhash.h`, 65): both are FreeSurfer's
  files, not a third party's: `fnv_hash.h` carries MGH's FreeSurfer notice ("Original Author:
  Bevin Brett", 2021), `fnvhash.h` no notice. They implement FNV-1a with the 32-bit offset basis
  and prime in an `unsigned long` (64 bits on Linux: the multiplication wraps modulo 2^64, the
  high bits carry on). Callers: `utils/mrisurf.cpp`'s surface hashes (`fnv_init`, `fnv_add` over
  the vertices', faces' and surface's members), `utils/mrisurf_metricProperties_slow.h`'s
  `fnv_hash(trial, k, &random_counter, -RAN, RAN)` (FreeSurfer's own mapping of the hash of three
  `unsigned int`s to a float in [LO, HI]: `(double)(hash & 0xffffffff) / (double)0xffffffff`, then
  `(float)((HI-LO)*f + LO)`), and `utils/gcamorph.cpp`'s `__m3zWrite` (`FnvHash::add` over the
  morph's data; `FnvHash::write` writes the decimal value and `std::endl`). larmorx-core has the
  algorithm (`larmorx_core::fnv::fnv1a_32_wide`, from FNV's public description); `fnv_hash`'s
  float mapping and `FnvHash::write`'s file are FreeSurfer's: track A's, in crates-freesurfer.
  The audit's TP-13 calls both files the FNV hash (public domain): the algorithm is; the files
  are FreeSurfer's.
- **argparse** (`include/argparse.h`, `utils/argparse.cpp`, 7 programs): "contains code that was
  derived from the ArgumentParser project by Hilton Bristow" (BSD-3-Clause). Measured against
  Bristow's last commit (github.com/hbristow/argparse 43edb8ac, `argparse.hpp`, 434 lines):
  923 of FreeSurfer's 3,968 tokens (23 %; comments and layout ignored; the tool
  `<scratch>/C/tools/toksim.py` of relays 3 and 6) are in the same order as Bristow's 2,904. The
  shared part is the class's shape (the `Any` holder, `insertArgument`'s index map, `retrieve`);
  the behaviour a caller sees (argument types and their checks, `stob`, the XML help, the error
  messages through `log.h`'s `fs::fatal`, `exists`, `strip`/`unstrip`) is FreeSurfer's. So item
  3.1-21 is FreeSurfer's code: its replica belongs in crates-freesurfer (track A); a port of
  Bristow's upstream alone would serve no program. Relay 8 ported nothing of it (handoff).

## The probes that name FreeSurfer's internals (for the clean-room audit's restriction table)

`validation/probes/fs-vnl` (its shim's comments: FreeSurfer's call sites and flags),
`validation/probes/x87-extended` (`patterns.cpp`: FreeSurfer's `long double` patterns),
`validation/probes/fs-random` (FreeSurfer's seeds and its `drand64` cast),
`validation/probes/fs-minc-nifti` (`ProbeFs`, `Flavour::FreeSurfer`: FreeSurfer's copies' changes)
`validation/probes/fs-gifti` (`dialect.rs`: FreeSurfer's two GIFTI changes) and
`validation/probes/fs-dicom` (`port.rs`'s `FsDialect`: FreeSurfer's changes to CTN) and
`validation/probes/fs-json` (`FsDialect`: its `json.h`'s messages) and
`validation/probes/fs-misc` (`facts.cpp` includes FreeSurfer's FNV headers) are closed to
FreeSurfer's clean-room implementers, as this file is.
