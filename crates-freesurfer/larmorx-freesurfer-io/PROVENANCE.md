# Provenance: larmorx-freesurfer-io

**Replica: translated from FreeSurfer's source.** This crate holds `fs.io` (3.1-25 to 3.1-32) and `fs.labels` (3.1-33, 3.1-34) of the FreeSurfer
replica (`docs/freesurfer/waves.md` section 4): the volume formats (`MRIread`/`MRIwrite`: MGH/MGZ, NIfTI, ANALYZE, the format probe, the other readers; FreeSurfer's tags, `.mat`, `.npy`, GIFTI volumes), FreeSurfer's changes to MINC's files, and colour tables, annotations, labels, `register.dat`, control points, point sets and CSV (`fs.labels`). It is distributed under FreeSurfer's
licence (`LicenseRef-FreeSurfer`, `../LICENSE`: the FreeSurfer Software License with its required
preface), as part of the `larmorx-freesurfer` package (`docs/licensing.md`).

The upstream is FreeSurfer 8.2.0 (GitHub's `v8.2.0` archive, SHA-256
`bb1699f7655ebf0f3790db6a78af9532ea7c832c97fe5f71bfd9f0e9526d7e78`; `docs/freesurfer-plan.md`),
built for the official binaries with GCC 8.4.0 (`docs/freesurfer/variation.md`).

## FreeSurfer files and functions translated

Queue item 3.1-25, piece 1 (FreeSurfer's foundations, track A, relay 10). Audited for third-party
code: none in these files (FreeSurfer's own: Bruce Fischl, Douglas Greve).

| file (FreeSurfer 8.2.0) | functions (lines) | module |
|---|---|---|
| `utils/fio.cpp` | `putf` (52), `getf` (60), `fread1` (73), `fread2` (83), `fread3` (96), `fread4` (109), `fwrite1` (122), `fwrite2` (129), `fwrite3` (144), `fwrite4` (154), `fwriteShort` (162), `freadDouble` (169), `freadInt` (182), `freadLong` (194), `freadShort` (207), `freadFloatEx` (229), `freadDoubleEx` (239), `freadIntEx` (249), `freadShortEx` (259), `fwriteInt` (270), `fwriteLong` (278), `freadFloat` (287), `fwriteFloat` (306), `fwriteDouble` (320), `znzread1` (334), `znzread2` (344), `znzread3` (357), `znzread4` (370), `znzwrite1` (383), `znzwrite2` (390), `znzwrite3` (405), `znzwrite4` (415), `znzwriteShort` (423), `znzwriteUShort` (431), `znzreadDouble` (439), `znzreadInt` (455), `znzreadLong` (469), `znzreadShort` (484), `znzreadFloatEx` (512), `znzreadDoubleEx` (522), `znzreadIntEx` (532), `znzreadShortEx` (542), `znzwriteInt` (553), `znzwriteLong` (561), `znzreadFloat` (570), `znzwriteFloat` (589), `znzwriteDouble` (603), `fio_dirname` (616), `fio_basename` (674), `fio_extension` (728), `fio_DirIsWritable` (762), `fio_FileExistsReadable` (783), `fio_IsDirectory` (797), `TSV::read` (1034), `TSV::print` (1103), `TSV::write` (1119) | `fio` |
| `packages/nifti/znzlib.c` (FreeSurfer's copy of znzlib, public domain) | `znzread`, `znzwrite` (the compressed branch: one `gzread`/`gzwrite`, the count over the member size) | `fio::fs_znzread`, `fs_znzwrite`, on `larmorx_io::znz`'s zlib layer |
| `utils/bfileio.cpp` | `byteswapbufdouble` (51), `byteswapbuffloat` (81), `byteswapbufshort` (103) | `bfileio` |
| `utils/MRIio_old.cpp` | `MGHprint_error` (29), `lcalloc` (44) | `mriio_old` |
| (larmorx's own) | C's `FILE` as these functions use it: `fopen`'s modes and `errno`, `fread`/`fwrite` of members, `getc`, `fgets`, `feof`, `ferror`, `fclose`, `fseek`, `rewind`, `ftell`, `scanf`'s input; written from ISO C 7.21 and glibc 2.35's measured behaviour (`validation/investigations/glibc-stdio-probe/` for the seeks) | `cfile` |

Queue item 3.1-25, piece 2 (track A, relay 11), with tags.cpp (3.1-30) and colortab.cpp's binary
tables (3.1-33) taken early (the orchestrator's decision of 2026-11-13). Audited for third-party
code: none (FreeSurfer's own: tags.cpp and mri.h by Bruce Fischl, colortab.cpp by Kevin Teich and Bruce Fischl, fstagsio.cpp without an author line; the General Hospital Corporation's copyright).

| file (FreeSurfer 8.2.0) | functions (lines) | module |
|---|---|---|
| `utils/tags.cpp` | `TAGskip` (35), `TAGreadStart` (51), `TAGread` (75), `TAGwriteStart` (86), `TAGwrite` (99), `TAGwriteEnd` (109), `TAGmakeCommandLineString` (124), `TAGreadMatrix` (179), `TAGwriteMatrix` (195), `znzTAGreadStart` (250), `znzTAGwriteStart` (298), `znzTAGwriteEnd` (325), `znzWriteMatrix` (349), `znzReadMatrix` (405); `include/tags.h`'s tag numbers and `MATRIX_STRLEN` | `tags` |
| `utils/fstagsio.cpp` | `FStagsIO`'s 43 functions (22-1195: the `getlen_*`, `write_*`, `read_*`, `skip_tag`, the NIfTI extension's matrix and frame forms) | `fstagsio` |
| `include/mri.h` | `VOL_GEOM::write` (512), `VOL_GEOM::read` (562) | `fstagsio::vol_geom_write`, `vol_geom_read` |
| `utils/colortab.cpp` | `CTABreadFromBinary` (352), `CTABwriteIntoBinary` (390), `CTABreadFromBinaryV1` (573), `CTABwriteIntoBinaryV1` (668), `CTABreadFromBinaryV2` (707), `CTABwriteIntoBinaryV2` (825), `znzCTABreadFromBinary` (889), `znzCTABwriteIntoBinary` (927), `znzCTABreadFromBinaryV1` (951), `znzCTABwriteIntoBinaryV1` (1042), `znzCTABreadFromBinaryV2` (1081), `znzCTABwriteIntoBinaryV2` (1195) | `colortab_bin` |

Not translated from tags.cpp (unreached): `TAGwriteCommandLine`, `TAGwriteAutoAlign`, `znzTAGskip`
(`FStagsIO::skip_tag` is its reached copy), `znzTAGwrite`, `znzTAGwriteCommandLine`, `znzTAGreadFloat`,
`znzWriteAutoAlignMatrix`, `znzReadAutoAlignMatrix`.

The one-number readers and writers the inventory does not list as reached (`putf`, `getf`, `fread4`,
`fwrite1`, `fwrite4`, `fwriteShort`, `freadShort`, the other `Ex` readers, `fwriteDouble`, `znzread2/3/4`,
`znzwrite1/2/3/4`, `znzreadDoubleEx`, `znzreadShortEx`) are translated all the same (relay 12): the
surface files' readers and writers (`fs.surfio`, `mrisurf_io.cpp`) and `vlabels.cpp` call them.

Queue item 3.1-25, piece 3 (track A, relay 13). Audited for third-party code: none (fscnpy.cpp and
fscnpy.h without an author line; gifti.cpp by Kevin Teich and Nick Schmansky; dti.cpp by Douglas
Greve; the General Hospital Corporation's copyright). gifti.cpp's helpers work on gifticlib's
structures through `larmorx_io::gifti` (track C's port of gifticlib 1.12, public domain).

| file (FreeSurfer 8.2.0) | functions (lines) | module |
|---|---|---|
| `utils/fscnpy.cpp`, `include/fscnpy.h` | `fscnpy::fscnpy` (8), `~fscnpy` (29: Rust's drop), `npy2mri` (67), `__parse_header_dictionary` (215), `__getmritype` (371), `__getarchendian` (416: `<`, every supported platform), `__contiguousreorder` (470) | `fscnpy` |
| `utils/gifti.cpp` | `gifti_alloc_and_add_darray` (47), `gifti_get_DA_value_2D` (69-212) | `gifti` |
| `utils/dti.cpp` | `DTIwriteBValues` (848), `DTIwriteBVectors` (931), `DTIparsePulseSeqName` (980), `DTIbvecChangeSpace` (1083) | `dti` |
| (larmorx's own) | `fscnpy::ShapeVec`: libstdc++'s `std::vector<int>` as `npy2mri` uses it (`push_back`'s growth by doubling with new places unwritten, `pop_back`, `erase(begin())`), from the C++ standard's and libstdc++'s documented behaviour, for FS-206 | `fscnpy` |

Not translated: `fscnpy::read` (no caller), `fscnpy::write` (empty), `exec_progress_callback` (freeview's
progress bar: nothing without its callback); the rest of gifti.cpp (3.1-26) and of dti.cpp (the items
that reach it).

Not reached by any default-path program, so not translated: `MGHopen_file`, `fio_NLines`, `fio_pushd`, `fio_popd`, `fio_fullpath`, `fio_mkdirp`,
`fio_FileHasCarriageReturn`, `makelocallink`, `TSV::IsTSVExt` (`mri_glmfit` only); `lmalloc`,
`file_name`, `buffer_to_image`, `image_to_buffer`; `bfileio.cpp`'s bfile readers.

## Changes from the originals

- D13 (`docs/findings/freesurfer-io.md`, `larmorx_core::defects`): FS-49 and FS-167 (missing bytes
  0, both modes), FS-168 (`.`, both modes), FS-169 (the whole name, both modes), FS-170 (no names,
  both modes), FS-171 (the byte before the line never read), FS-172 (an error by default; FreeSurfer's
  stale numbers where it moves on, an error where it reads for ever), FS-173 (formats refused, both
  modes), FS-174 (status 1 by default), FS-175 (a group past the buffer left, both modes), FS-176
  (exact by default; `larmorx-freesurfer-core::machine::swap_long64`).
- Memory as x86-64 holds it: numbers are built from their big-endian bytes on every platform; the
  `Ex` readers overwrite the caller's value's little-endian bytes, as FreeSurfer does on x86-64.
- `TSV`'s column names are byte strings (`larmorx-freesurfer-core::fio::Tsv`), as C++'s
  `std::string`s hold them.
- `TSV::print`'s formats are printed by `larmorx-freesurfer-core::cstd::format_double` (`%f`, `%e`,
  `%g` and their flags, as glibc; `%a` refused).
- Piece 2: FS-177 to FS-194 (`docs/findings/freesurfer-io.md` Part 2). The skippers and the colour
  table readers never allocate a length or a count before reading: they read what the file holds
  (FS-178, FS-191), with the impossible lengths' exit kept and counts above 2^20 (version 1) or 2^24
  (version 2) refused. `TAGread`, `read_data` take the caller's buffer as a slice: bytes beyond it are
  read and dropped. `FStagsIO` holds the znz file it writes (`FsTagsIo { fp, niftiheaderext }`); the
  frames' matrix and the labels are Rust values (`Matrix`, `labels[(x * y0 + y) * z0 + z]`).
- `CFile::tell` (`ftell`) for `TAGwriteStart`'s position.
- `lcalloc` returns a vector of the caller's element type (`nmemb` elements; the C `size` is checked
  for the product's overflow).
- Piece 3: FS-199 to FS-220 (`docs/findings/freesurfer-io.md` Part 3; FS-206 in
  `larmorx-freesurfer-mri`). `npy2mri` reads the header into a heap buffer of the bytes the file gives
  (zeros after them), whatever its length; its parse runs on that buffer with FreeSurfer's NUL writes
  and stops at its end. Its messages go to the caller's standard output; its exits are
  `Fatal::Exit(1)`. The volume's voxels are copied into `Mri::chunk`. `gifti_get_DA_value_2D` reads
  the data's little-endian bytes (zeros past them: gifticlib's arrays always hold their values).
  `DTIbvecChangeSpace` copies `bvec` before the in-place product. The b-value writers write the
  file's text at once.

Queue item 3.1-27, piece 1 (track A, relay 14): the volume format probe, with the pieces it needs
taken early (mri_identify.cpp's lines 697-1195 from 3.1-28, `MRIgetVolumeName` from mriio.cpp
(3.1-28), signa.cpp's header test from 3.1-30, DICOMRead.cpp's format-probe functions, which stay in
fs.io by the orchestrator's decision of 2026-11-13). Audited for third-party code: mri_identify.cpp
(Christian Haselgrove), signa.cpp (GE's `mvtsunf` conversion inside `ghf`, as FreeSurfer carries it),
Bruker.cpp, DICOMRead.cpp (Sebastien Gicquel, Douglas Greve): FreeSurfer's own, the General Hospital
Corporation's copyright; the DICOM reader they call is CTN 3.0.6's (`larmorx_io::dicom`, Apache-2.0
AND LicenseRef-CTN, the main package's crate, used as a dependency).

| file (FreeSurfer 8.2.0) | functions (lines) | module |
|---|---|---|
| `utils/mri_identify.cpp`, `include/mri_identify.h` | `type_to_string` (47), `string_to_type` (146), `mri_identify` (201), `IDtypeFromStem` (394), `IDstemFromName` (423), `IDextensionFromName` (486), `IDnameFromStem` (537), `is_cor` (571), `is_brik` (594), `is_siemens` (608), `is_genesis` (664), `is_ge_lx` (697), `is_analyze` (723), `is_mnc` (764), `is_mgh` (802), `is_bshort` (817), `is_bfloat` (829), `is_bhdr` (840), `bhdr_stem` (854), `bhdr_firstslicefname` (878), `bhdr_precisionstring` (909), `bhdr_precision` (942), `is_sdt` (967), `is_gdf` (998), `is_otl` (1011), `is_ximg` (1024), `is_nifti1` (1026), `is_nii` (1071), `is_nrrd` (1112), `IDisCurv` (1195); the magic numbers; `include/mri.h`'s format numbers (1516-1553) | `identify` |
| `utils/mriio.cpp` | `MRIgetVolumeName` (472); `include/mri.h`'s `MRIIO_Strip_Pound` (2033) | `identify::mri_get_volume_name`; `larmorx_freesurfer_core::globals::Globals::mriio_strip_pound` |
| `utils/Bruker.cpp` | `checkBrukerFiles` (201), `is_bruker` (834); since relay 17 `brukerRead` (146), `splitParameterValue` (261), `readBrukerD3proc` (290), `buildVoxToRASTransform` (346), `readBrukerAcqp` (532), `readBrukerReco` (657), `readBrukerVolume` (771), `include/Bruker.h`'s `BrukerTransform` | `bruker` |
| `utils/signa.cpp`, `include/signa.h` | `ghc` (69), `ghi` (77), `ghf` (93), `get_signa_header_info` (144), `is_signa` (179); the header's offsets and `HINFO` | `signa` |
| `utils/DICOMRead.cpp` | `IsDICOM` (5410), `IsSiemensDICOM` (1479), `GetObjectFromFile` (1207), `GetElementFromFile` (1164), `AllocElementData` (1252) and `FreeElementData` (1408) for the Manufacturer element, `DCMPrintCond` (5523); `IsDICOM`'s static answer | `dicom_probe`; the state in `Globals::dicom` |
| `packages/dicom/dcm.c` (FreeSurfer's copy of CTN 3.0.6) | `DCM_GetElement` (2699) with `locateElement` (6601), `DCM_GetElementValue` (950) for one element of the object's top level read into memory; FreeSurfer's two changes to CTN's behaviour (the "Data Element ... longer" message, the 4-byte swap in pairs) as `dicom_probe::FsDialect` | `dicom_probe` |
| (larmorx's own) | `COND_DumpConditions`'s output byte for byte (`dicom_probe::dump_conditions`: larmorx-io's `CondStack::dump` goes through text streams); `CFile`'s reads of a folder failing with `EISDIR` on every platform and the failed read's `errno` | `dicom_probe`, `cfile` |

Queue item 3.1-27, piece 2 (track A, relay 15): mosaics and MATLAB files. Audited for third-party
code: mosaic.cpp (Douglas Greve) and matfile.cpp (Bruce Fischl): FreeSurfer's own, the General
Hospital Corporation's copyright. matfile.cpp calls the system zlib's `uncompress` (zlib 1.2.11 on
the official build's Ubuntu): larmorx decodes with `flate2` on `zlib-rs` (Zlib licence, a dependency
already in the lock file), its results mapped to `uncompress`'s (`Z_OK`, `Z_BUF_ERROR`,
`Z_DATA_ERROR`).

| file (FreeSurfer 8.2.0) | functions (lines) | module |
|---|---|---|
| `utils/mosaic.cpp`, `include/mosaic.h` | `VolSS2MosSS` (72), `MosSS2VolSS` (129), `CheckMosaic` (205) | `mosaic` |
| `utils/matfile.cpp`, `include/matfile.h` | `Matlab_Install_printf` (55), `MatlabWrite` (61), `MatlabRead` (68), `MatFileRead` (141), `MatFileWrite` (206), `matFree` (276), `matAlloc` (286), `readMatFile` (310), `MatReadHeader0` (458), `swapBytes` (503), `ReadMatlabFileVariable` (516), `ReadMatlabFileContents` (545), `MLFCprint` (628), `MLFCfree` (645), `znzreadMatFile` (671), `MatlabRead2` (765), `MatReadHeader` (863), `znzMatReadHeader` (1061); the `MAT_*`, `MATFILE_*` constants, `MATFILE`, `MLFC` | `matfile` |

Changes for piece 2: the official build's integer arithmetic for mosaics (GCC 8.4.0 at `-O3`: the
bounds as `x >= n`, the remainder from the division); C's `ungetc` modelled by a push-back stack over
`CFile` (`matfile::Pb`), a relative seek counting the bytes given back; `matAlloc`'s rows held flat;
`MATFILE`'s fields never written tracked by a mask (0 in their place, noted); the decoded buffer of a
compressed variable read with glibc's allocation sizes (zero within, memory contents beyond, noted);
`MatlabRead2`'s and `MatFileRead`'s files closed (FS-242). The corrected readers (`fix`) are larmorx's
own: MATLAB's version 4 and 5 formats read by their definition (MATLAB's MAT-file format document).

## FreeSurfer files and functions translated (part 6, relay 16: reading volumes and registration files)

| FreeSurfer file | What | Here |
|---|---|---|
| `utils/mriio.cpp` 204-257, 488-1105 | `setDirectionCosine`, `mri_read`, `nan_inf_check`, `MRIreadType`, `MRIreadInfo`, `MRIreadHeader` (taken early from 3.1-28) | `mriio.rs` |
| `utils/mriio.cpp` 10853-11245, 12195-12420 | `local_buffer_to_image`, `mghRead`, `MRITAGread` (taken early from 3.1-30) | `mriio.rs` |
| `utils/registerio.cpp` 43-162 | `regio_read_register` (taken early from 3.1-34; its `.lta` branch through `RegisterLta`) | `registerio.rs` |
| `utils/resample.cpp` 109-118 | `float2int_code` | `registerio.rs` |
| `packages/minc/minc_gaussian.c` 157-290, `minc_volume_io.c` (its changes to MINC 1.5.1's reader: messages, transform types, tokens, `~user`) | `invert_4x4_matrix`, FreeSurfer's volume_io dialect of `larmorx_io::xfm` (MNI's notice: `LicenseRef-MNI`) | `minc.rs` |

Third-party code: `packages/minc` is MNI's volume_io reworked by FreeSurfer (MNI's permissive notice); `minc.rs`
carries both licence names. No other third-party code in these lines.

Changes for part 7 (relay 17, Bruker's readers): `BrukerTransform`'s and the readers' outputs held as
`Option`s (`None`: never written, FS-401, 0 in both modes); the line buffer as the C string `fgets`
gives (its first NUL ends it); the `sscanf` calls through `larmorx_freesurfer_core::cscan::Scanf`;
`stuff_four_by_four`'s float rounding and `MatrixMultiply` through fs.core's `matrix_multiply` (the
in-place products as copies, as `MatrixMultiplyD` does); the voxel size and offset in the official
object code's operation order (`fov * 10 / size`; `-vox * (ft_size - 1) * 0.5`, the halving equal
to the source's `/ 2.`); the files closed where FreeSurfer leaves them open (FS-260);
`exec_progress_callback` a no-op (no FreeSurfer program sets a progress callback). The defaults of
FS-402 and FS-404 are larmorx's own: the voxels read by their type's size and in `$RECO_byte_order`'s
order (ParaVision's documented parameter).


## FreeSurfer files and functions translated (part 8, relay 18: FreeSurfer's 2-D images)

| FreeSurfer file | What | Here |
|---|---|---|
| `utils/image.cpp` 65-145, 384-468, 526-569, 977-1500, 3748-3822; `include/image.h`, `include/hips.h` (the `IMAGE` header, the formats, the pixel macros) | `ImageAlloc` (65, 3.1-26's line), `ImageAllocBuffer`, `ImageFree`, `ImageCopy`, `ImageClearArea`, `ImageCheckSize`, `ImageSetSize`, `ImageCopyFrames`, `ImageRescale`, `ImageScaleDown`, `ImageScaleUp`, `init_header`, `h_copy`, `free_header` (the reached 14 of the file's 73 functions) | `image.rs` |
| `utils/hips.cpp` 4-5, 42-47 | `hips_cplxtor`, `hips_rtocplx` (fs.core's globals), the conversion stubs `h_tob` ... `h_toi` | `image.rs` |
| `utils/filter.cpp` 713-1041 | `ImageConvolve1d`, `ImageReduce` with its kernel, `ImageReduce1d` (fs.filter's, here because `ImageScaleDown` calls them and fs.filter's crate sits above fs.io) | `image.rs` |
| `include/macros.h` 35, 118 | `exp2` as `pow(2.0, f)`, `ISPOW2` (decided exactly: `ispow2`, `nint_log2`) | `image.rs` |

Changes for part 8: the pixels one `Vec<u8>` of the length FreeSurfer allocates, the pointer walks
(`image += sizeimage` per frame, restored at the end) as byte offsets, every pixel access checked
(FS-405: both modes skip the write and read 0); floats as their little-endian bytes; the sums in the
official object's operand order through `larmorx_freesurfer_core::sse`; `ImageReduce`'s `static`
work image and `ImageConvolve1d`'s `static` index tables made each call (only memory differs);
`ISPOW2` and `nint(log2(1 / scale))` decided without `log2f`/`log2`/`pow` (exact for the values they
meet: a power of two's logarithm is exact in glibc, and the count is computed only next to one).
The defaults of FS-406 (clamped conversions), FS-407 (the destination frame), FS-408 (every frame
reduced), FS-409 (the copy returned) and FS-410 (`ERROR_NO_MEMORY`) are larmorx's own.


## FreeSurfer files and functions translated (part 9, relay 19: fs.labels' text colour tables and lookups)

| FreeSurfer file | What | Here |
|---|---|---|
| `utils/colortab.cpp` 55-262, 420-571, 1245-1765, 1779-2167, 2480-2496; `include/colortab.h` (`CTAB_VERSION_TO_WRITE`, `ctabDuplicates`, `AnnotToRGB`) | `CTABreadASCII`, `CTABreadASCII2` (with `ctabDuplicates`), `CTABreadASCIIttHeader`, `CTABalloc`, `CTABunique`, `CTABcountRepeats`, `CTABrandom`, `CTABreadDefault`, `CTABgetNumberOfValidEntries`, `CTABgetNumberOfTotalEntries`, `CTABisEntryValid`, `CTABhasEntry`, `CTABrgbAtIndexi`, `CTABcopyName`, `CTABrgb2Annotation`, `CTABentryNameToIndex`, `CTABentryNameToAnnotation`, `CTABannotationAtIndex`, `CTABfindAnnotation`, `CTABfindDuplicateNames`, `CTABfindIndexFromAnnotation`, `CTABfindRGBi`, `CTABfindName`, `CTABprintASCII`, `CTABwriteFileASCII`, `TissueTypeSchema`, `TissueTypeSchemaDefault`, `TissueTypeSchemaDefaultHead`, `CTABmerge` | `colortab.rs` |
| `include/cma.h` | the structure numbers the two schemas name (`Left_Cerebral_Cortex` 3 ... `Head_ExtraCerebral` 258) | `colortab.rs` (`tt_default`, `tt_default_head`) |
| `utils/registerio.cpp` 161-350, 523-534 | `regio_print_register`, `regio_write_register`, `regio_read_mincxfm`, `regio_write_mincxfm`, `regio_read_registermat` (3.1-34) | `registerio.rs` |
| `utils/mriio.cpp` 12278-12310 (part 6's `MRITAGread`, changed) | the transform name's buffer: its bytes never written tracked (FS-421) | `mriio.rs` |

Changes for part 9: the table's places a `Vec<Option<Box<ColorTableEntry>>>` (fs.mri's structure);
lines read through `CFile::fgets` with FreeSurfer's 1,024-byte buffer and `fgetl`'s rules, numbers
and names through fs.core's `cscan::Scanf` (glibc's `sscanf`, `errno` kept in `Globals`);
`CTABreadASCIIttHeader`'s two passes over the lines read once (the last line twice, as `fgets`
leaves its buffer at the end); `stricmp` on bytes (FS-11 noted for 1,024 bytes or more);
`CTABprintASCII`'s `%3d` and `%-30s` written byte by byte; `ctabDuplicates` returned by
`ctab_read_ascii2_counted` (no global); `CTABprintASCII(ct, stdout)` as `ctab_print_ascii_stdout`;
`FSENVgetenv`'s colour-table reader given the program's globals and output (fs.core's
`fsenv_getenv`). The defaults of FS-413 (a negative structure an error, a larger one kept), FS-414
(a bad `#ctTType` line an error), FS-417 (empty places skipped), FS-418 (the table grown) and the
caps of FS-420 (2^24 places, 2^20 random entries) are larmorx's own; FS-415 closes the file (no
notice), FS-416 returns no table.


## FreeSurfer files and functions translated (part 10, relay 20: fs.labels' labels, control points, annotation tables, point sets)

| FreeSurfer file | What | Here |
|---|---|---|
| `utils/label.cpp` 54-214, 534-620, 804-845, 1121-1132, 1246-1305, 2543-2575; `include/label.h` 35-63 (`LABEL_VERTEX`, `LABEL`, `LABEL_COORDS_*`); `include/mri.h` 86-89 | `LabelReadFrom`, `LabelRead`, `LabelFree`, `LabelWriteInto`, `LabelWrite`, `LabelAlloc`, `labelLoadTransform`, `LabelCopy`, `LabelRemoveDuplicates`, `LabelHasVertex`, `LabelRealloc` | `label.rs` |
| `utils/ctrpoints.cpp` 37-170, 495-558; `include/ctrpoints.h` (`MPoint`) | `MRIreadControlPoints`, `MRIwriteControlPoints`, `MRIfillPoints` | `ctrpoints.rs` |
| `utils/annotation.cpp` 44-89, 115-316, 524-555; `include/annotation.h` (`annotation_table_file`) | `ATABLE_ELT`, `read_named_annotation_table`, `read_annotation_table`, `annotation_to_index`, `index_to_name`, `index_to_annotation`, `annotation_to_name`, `set_atable_from_ctable`, `print_annotation_table`, `print_annotation_colortable` | `annotation.rs` |
| `include/pointset.h`; `utils/pointset.cpp` 17-71, 103-154 | `fsPointSet` (`Point`, `add`, `set`, `get`, `remove`, `clear`, `npoints`, `print`, `printCoords`, `writeCoords`), `save`, `save_as_ctrlpoint`, `writeCentroidTable` | `pointset.rs` |
| `utils/utils.cpp` 284-304 (`fgetl`) | with the caller's buffer size (199 for control points and annotation tables) | `label.rs` (`fgetl`) |

Changes for part 10: a label's places a `Vec<LabelVertex>` of `max_points` places, its names and
space `Vec<u8>` C strings (FS-423, FS-427: longer ones kept whole), its subject's transform a
`larmorx_io::xfm::GeneralTransform` with its forward and inverse matrices (`None`: the zeroed
`General_transform` of a label without one), its surface, hash and template pointers the caller's
handles (`Option<usize>`); `getenv` through the program's `Environment`; files through `CFile` (the
program's directory for relative names); `sscanf` through fs.core's `cscan::Scanf`; numbers through
`cstd::fmt_f` and `format_double` (`%g`, the streams' default); FreeSurfer's process-wide
annotation table (`atable`, `num_entries`, `annotation_table_file`) the caller's
`AnnotationTables`; `fsPointSet`'s `ofstream`s as whole-file writes (a file that does not open is
not written, as a failed stream); `MRIfillPoints`' sums and differences in the official object's
operand order through `larmorx_freesurfer_core::sse`. The defaults of FS-424 (no points stored),
FS-425 (only the points read), FS-428 (closed once), FS-429 (the copy's places made), FS-430 (whole
names), FS-431 (nothing removed) and the cap of FS-426 (2^26 places) are larmorx's own.
`loadfsPointSet` and the label coordinate conversions are not ported yet (see the findings' Part 10).

## FreeSurfer files and functions translated (track D's GIFTI volumes, moved here in its relay 23)

Written by track D (the surface track) in its relays 21 and 22 in `larmorx-freesurfer-surf` and
moved here in its relay 23, so that `MRIread` and `MRIwrite` (this crate) reach the GIFTI volume
functions without the surface crate (the orchestrator's decision: they need no surface type).
Behaviour: `specs/freesurfer-surfio.md` §107-111; mechanisms: `docs/findings/freesurfer-surfio.md`
§23-24.

| FreeSurfer file | What | Here |
|---|---|---|
| `utils/gifti.cpp` 1231-1411, 1506-1609; `include/mri.h` 677-683 (`MGZ_INTENT_*`) | `MRISreadGiftiAsMRI`, `mriWriteGifti` (FS-521, FS-522, FS-395) | `gifti_vol.rs` |
| `utils/gifti.cpp` 213-336, 1416-1424, 2857-3006 | `gifti_set_DA_value_2D` (FS-17), `insertCommonMetaData` (the user name from `/proc` and `/etc/passwd` as `getpwuid`, else `USER`/`USERNAME`; `Date` by `currentDateTime`), `readLabelTable` (FS-389, FS-80) with the colour entries it builds | `gifti_common.rs` |
| `utils/gifti.cpp` (the steps its readers and writers repeat) | `gifti_read_image` with a version "1" made "1.0" and gifticlib's check (FS-395), the writers' end (`gifti_valid_gifti_image`, `gifti_write_image`, `gifti_free_image`), `gifti_get_meta_value`, `gifti_DA_rows_cols` as the readers count vertices, the arrays' attributes and sizes | `gifti_common.rs` |
| `packages/gifti/gifti_io.c` 4232-4250, `gifti_xml.c` 1236-1252 | FreeSurfer's two changes to gifticlib 1.12 (`FsGifti`, over `larmorx_io::gifti`) | `gifti_common.rs` |
| (larmorx's own) | `streams::Io`: standard output, standard error and the environment passed together (moved from the surface crate's `coords`) | `streams.rs` |

Changes for this part: gifticlib's settings and persistent state (its globals `G` and `GXD`) are
fs.core's `Globals::gifti`, one per run as in FreeSurfer, which the volume and the surface functions
share (before relay 23 they were the surface crate's `SurfGlobals::gifti`).

## FreeSurfer files and functions translated (part 11, relay 21 of track A: GIFTI branches, MRIwrite, point-set files)

| FreeSurfer file | What | Here |
|---|---|---|
| `utils/mriio.cpp` 783-784 | `mri_read`'s GIFTI branch (`MRISreadGiftiAsMRI`) | `mriio.rs` |
| `utils/mriio.cpp` 1109-1295, 1309-1360; `include/mri.h` 1906-1908 | `MRIwriteType` (the directory check, the refusals, the GIFTI branch, the b-values tail), `MRIwrite` (the several volumes' names, FS-432) | `mriio.rs` |
| `utils/pointset.cpp` 74-92; `include/json.h` 14249 | `loadfsPointSet` (FS-433), FreeSurfer's `type_error.305` wording | `pointset.rs` |

Changes for part 11: the writers not ported yet (COR, MINC, image, `.bhdr`, bshort, bfloat, ANALYZE,
BRIK, the morphs, MGH, GDF, NIfTI, NRRD) answer with a message and -1; `chklc` passes (the port needs
no key); `stat` as `std::fs::metadata`; the several volumes as a slice; `loadfsPointSet`'s nlohmann
values through `larmorx_core::json` (track C's port), its uncaught exceptions as FS-433's clear error
(status 1) or, with `reproduce`, the abort's lines and status 134. `errno` set as FreeSurfer leaves it
after gifticlib's failed `fopen` (read and write) and after `getpwuid` (0) in `gifti_common.rs` and
`gifti_vol.rs`.

## FreeSurfer files and functions translated (part 13, relay 23 of track A: MGH and MGZ written)

| FreeSurfer file | What | Here |
|---|---|---|
| `utils/mriio.cpp` 11247-11518; `include/mri.h` 2027 | `mghWrite` (FS-439 to FS-446, FS-58) | `mriio_write.rs` |
| `utils/mriio.cpp` 12421-12534 | `MRITAGwrite` (FS-441) | `mriio_write.rs` |
| `utils/mriio.cpp` 1197-1198 | `MRIwriteType`'s MGH branch | `mriio.rs` |
| `utils/mriio.cpp` 9522-9760, 10229-10398, 12538-12731, 12865-12899; `utils/mri2.cpp` 653-735 | `niiWrite` (FS-447, FS-448, FS-449, FS-58), `mriToNiftiSform`, `mriToNiftiQform`, `__getMRITAGlength`, `__writeFSniiextensionHeader`, `mri_reshape` as `niiWrite` calls it (the ico7 surface) | `mriio_write.rs` |
| `utils/mriio.cpp` 1206-1210 | `MRIwriteType`'s NIfTI branch | `mriio.rs` |
| `utils/mriio.cpp` 8177-8363 | `nifti1Write` (FS-447, FS-449, FS-451, FS-452, FS-58; relay 24) | `mriio_write.rs` |
| `utils/mriio.cpp` 1124-1135, 1203-1205 | `MRIwriteType`'s `stat` (its `errno`) and NIfTI-1 branch (relay 24) | `mriio.rs` |
| `utils/mriio.cpp` 8365-8979, 10107-10227, 10400-10441, 12149-12198, 12717-12859; `include/mri.h` 310-326; `utils/mri2.cpp` 653-735 | `niiRead`, `niftiSformToMri`, `niftiQformToMri`, `swap_nifti_1_header`, `niiPrintHdr`, `__niiReadHeaderextension`, `__niiReadSetVox2ras`, `__readFSniiextensionHeader`, `MRI::update_ras_xform`, `mri_reshape` as the readers call it (FS-454 to FS-460, FS-641 to FS-643; relay 25) | `mriio_nifti.rs` |
| `utils/mriio.cpp` 7680-8175 | `nifti1Read` (FS-453, FS-454, FS-457, FS-458, FS-643, FS-644; relay 25) | `mriio_nifti.rs` |
| `utils/mriio.cpp` 772-777 | `mri_read`'s NIfTI branches (relay 25) | `mriio.rs` |
| `utils/utils.cpp` 1551-1556 | `exec_progress_callback` (no callback in the port's programs: nothing) | (not translated) |

Changes for part 13: FreeSurfer's one `znzwrite*` per number is one write per slice (the header and
the scan parameters one write each): the same bytes, whose counts FreeSurfer does not check (a byte
volume's rows stay one write each, as their count is checked); a compressed file's bytes are
zlib-rs's (`larmorx_io::znz`), the stream they inflate to FreeSurfer's. `MRIwriteType` and `MRIwrite`
take the volume mutably (FreeSurfer's `mghWrite` changes its intent and version). FS-440's check of
the writes and the close is larmorx's own (FreeSurfer has none). The tag readers' start
(`znzTAGreadStart`, `FStagsIO::read_tagid_len`) no longer notes FS-167 for a number cut by the end of
the file, which they do not use; `mghRead`'s skip before the file's start sets `errno` EINVAL (glibc's
`fseek`); `utils::resolve` keeps an empty name empty (`FileExists("")` is false in FreeSurfer: the
folder opened instead); `minc::input_transform_file` gives volume_io the program's folder with `/`
on Windows (volume_io's escape, FS-79's `reproduce`, dropped its backslashes). `niiWrite`'s header is built as
the bytes of `struct nifti_1_header` in x86-64's memory; its data are written one slice per write (each
row's count was checked; a slice's count is); the warp map's format change (`Warpfield::changeFormat`)
is not ported yet (a message, -1).

Changes for relay 24: `nifti1Write`'s two files are C streams (`cfile::CFile`, an 8,192-byte buffer
where glibc's is the file system's block: only the row at which a full disk is found can differ);
FS-451's names are built from the whole name by default, and cut to 1,023 bytes or the program
ended with the C library's overflow message (status 134) under `reproduce`, as measured
(`validation/probes/fs-io/nifti1_long_names.py`); FS-452's check of the closes is larmorx's own.
The niiWrite helpers shared with it: the size checks (`nifti_size_checks`), a row's bytes and
FS-447's clamp (`nifti_row`, `long_rows_as_ints`). `matfile`'s size products saturate (a header's
sizes overflowed 64 bits: a debug build's panic; the refusal, FS-243, as before for the sizes that fit).

Changes for relay 25: the NIfTI header is kept as the 348 bytes of `struct nifti_1_header` in x86-64's
memory and byte-swapped in place field by field (as `swap_nifti_1_header`), its fields read from their
offsets. The arithmetic follows the official build's object code (the unit oracle's `mriio.cpp.o`,
`objdump`): `niftiQformToMri`'s quaternion with GCC's folding of `1.0 - s` and `1.0 / sqrt(s)` into
`float` operations (exact, the same values) and its operand order (which NaN payload a result keeps:
`b * (2a)` as `B * A2`, `2.0*a*c` as `C * A2`, `r13` as `2ac + 2bd`, the centre's sums `y + x + z`,
`z + (x + y)`, `x + y + z`), the scaled loops' `value * slope + intercept` (`slope * value` for
`float` data), the units' products with the field first (the factor first for `c_s` and TR), the
field of view and axis ends as `(extent x size) x 0.5` with the sign flipped first for the start;
`(long)vox_offset` as `cvttss2si`; `float`/`double` conversions as `cvtss2sd`/`cvtsd2ss` written
out (`widen`, `narrow`: NaN payloads kept, quieted). The voxel loops of both readers are one
function (`read_voxels`) with their differences as parameters (the count line, the swaps of
`double`s and complex values); `mri_reshape`'s copy is `mriio_write::reshape` (the writer's ico7
reshape generalised); `nifti1Read`'s cut names (FS-453) are larmorx's own under `reproduce`. The
fixes of FS-454 to FS-460, FS-643 and FS-644 are larmorx's own.
