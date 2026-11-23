# Provenance: larmorx-freesurfer-volumes

**Replica: translated from FreeSurfer's source.** This crate holds wave 3.2's programs of the FreeSurfer
replica (`docs/freesurfer/waves.md` section 4): the volume programs both streams run (`mri_convert`, `mri_info`, `mri_normalize`, `mri_segment`, `mri_fill`, ...), fMRIPrep's `mri_coreg`, `mri_robust_register` and `mri_robust_template`, and the scripts around them (`mri_nu_correct.mni`, `seg2cc`, ...). It is distributed under FreeSurfer's
licence (`LicenseRef-FreeSurfer`, `../LICENSE`: the FreeSurfer Software License with its required
preface), as part of the `larmorx-freesurfer` package (`docs/licensing.md`).

The upstream is FreeSurfer 8.2.0 (GitHub's `v8.2.0` archive, SHA-256
`bb1699f7655ebf0f3790db6a78af9532ea7c832c97fe5f71bfd9f0e9526d7e78`; `docs/freesurfer-plan.md`),
built for the official binaries with GCC 8.4.0 (`docs/freesurfer/variation.md`).

## FreeSurfer files and functions translated

Created empty by queue item 3.1-01 (`docs/freesurfer/queue.md`); each item that brings code adds
its files and functions here, with the third-party code it found in them (CLAUDE.md rule 3: each
ported file is audited for third-party code first).

| FreeSurfer 8.2.0 | What | Here |
|---|---|---|
| `mri_normalize/mri_normalize.cpp` 47-160, 1001-1348 (queue item 3.2-24, track D relay 46) | the file statics the options set, `get_option`, `usage_exit`, `main`'s option loop (FS-1016) | `src/mri_normalize/options.rs` |
| `mri_normalize/mri_normalize.help.xml` | the help text `usage_exit` renders (FreeSurfer's file, copied unchanged) | `src/mri_normalize/mri_normalize.help.xml` |
| `mri_normalize/mri_normalize.cpp` 1350-2167 (3.2-24) | `compute_bias`, `MRIremoveWMOutliersAndRetainMedialSurface` and `MRIremoveWMOutliers` (the `#if 1` forms; FS-1014), `add_interior_points`, `remove_surface_outliers` and `remove_outliers_near_surface` (their bodies after the first `return` never run), `remove_nonwm_voxels`, `build_outside_of_brain_mask`, `remove_surface_outliers_T2` (FS-1015), `threshold_control_points`, `remove_controlpoints_outside_range` | `src/mri_normalize/helpers.rs` |
| `mri_normalize/mri_normalize.cpp` 162-994 (queue item 3.2-25, track D relay 47) | `main`: the version options, the options, `-atlas`'s mask, the label's points, `-renorm`, `-surface`, `-conform`, `-mask`, `-r`, `-l`, `-grad`, the float input, `-aseg`, the first pass, the aseg and 1-D normalisations, the gentle and 3-D passes, `-w`'s bias, the byte result, the timing line (FS-1017); the program `mri_normalize` of `larmorx-freesurfer` | `src/mri_normalize/main.rs` |
| `include/csv.h` (queue item 3.2-04, track A relay 46) | `CSVState`, `readCSVRow`, `readCSV` | `src/mri_convert/csv.rs` |
| `utils/stats.cpp` 53-415 (3.2-04, taken from 3.2-07) | `LoadStatTableFS`, `LoadStatTableCSV`, `LoadStatTableTSV`, `LoadStatTable`, `AllocStatTable`, `WriteStatTable`, `PrintStatTable`, `InitStatTableFromMRI` (FS-949, FS-950, FS-1031 to FS-1037), `STAT_TABLE` (`include/stats.h`) | `src/mri_convert/stat_table.rs` |
| `utils/fsgdf.cpp` 868-886 (3.2-04) | `gdfCountItemsInString` (FS-948) | `src/mri_convert/stat_table.rs` |
| `mri_convert/mri_convert.cpp` 57-73, 79-1882, 3889-4001 (3.2-04 and 3.2-05, first half) | the file statics, `main` from its start to the check of a GDF image stem (the option loop, its checks, the parse-only report; FS-1038 to FS-1042), `get_ints`, `get_floats`, `get_string`, `usage_message`, `usage`; `include/DICOMRead.h`'s globals the options set (`DicomSettings`) | `src/mri_convert/options.rs` |
| `mri_convert/mri_convert.help.xml` | the help text `usage` renders (FreeSurfer's file, copied unchanged) | `src/mri_convert/mri_convert.help.xml` |
| `mri_convert/mri_convert.cpp` 1876-3848, 3860-3888 (3.2-05, second half; track A relay 47) | `main` after the GDF stem check (the input reads, the steps on the input, the header overrides, the reports, the transform and template, the transform applied, the type change, the reslice, the steps after it, the colour table, the outputs; FS-1043 to FS-1053), `applyGaussianFilter`; the in-place `MRIhalfCosBias` (`utils/mri.cpp`'s loop, `MRIcopy` of a volume onto itself a no-op) | `src/mri_convert/main.rs` |
| `utils/transform.cpp` 4182-4218, 4239-4277 (3.2-07's lines, taken with 3.2-05) | `LTAtransformTypeName`, `LTAdumpLinearTransform`, `LTAdump` | `src/mri_convert/lta_dump.rs` |
| `include/mri.h` 1931-1934, `utils/mriio.cpp` 194-198, 6521-6914, 7144-7271, `utils/cma.cpp` 48-366, `include/cma.h` 576-625 (3.2-06, track A relay 48) | `READ_OTL_*` flags; `MAX_UNKNOWN_LABELS`, `unknown_labels`, `register_unknown_label`, `clear_unknown_labels`, `print_unknown_labels`, `read_otl_file`, `MRIreadOtl`; `CMAoutlineClaim`, `CMAoutlineField`, `CMAfreeOutlineField`, `CMAoutlineFieldAlloc`, `CMAclearFillField`, `CMAfill` (an explicit stack in the recursion's order), `CMAclaimPoints`, `CMAvalueClaims`, `CMAvalueAllClaims`, `CMAaddWeightedTotals`, `CMAtotalClaims` (the sums kept sparse), `CMAassignLabels`, `CMAzeroOutlines` (FS-1054 to FS-1060, FS-1141 to FS-1145) | `src/mri_convert/otl.rs` |
| `mri_binarize/mri_binarize.cpp` 207-290, 712-1269, 1519-1612 (queue item 3.2-19, track D relay 48) | the file statics (`Options`), `parse_commandline` with its stand-alone options `--ohe`, `--crop-around-ras`, `--dilate-vertex(-sum)` (FS-1023, FS-1024, FS-1026), `usage_exit`, `print_version`, `check_options`, `dump_options` | `src/mri_binarize/options.rs` |
| `mri_binarize/mri_binarize.cpp` 1275-1355, 1357-1517 (3.2-19) | `print_usage`'s and `print_help`'s texts (their `printf` literals, made by `validation/probes/fs-binarize/help_text.py`) | `src/mri_binarize/help.rs` |
| `mri_binarize/mri_binarize.cpp` 292-709 (3.2-19) | `main` (FS-1018 to FS-1022, FS-1025): the in-place `MRIframeSum(InVol, InVol)` and `MRImultiplyConst(InVol, -1, InVol)` as their loops on the input (`utils/fmriutils.cpp` 1745-1765, `utils/mri2.cpp` 1866-1888); the program `mri_binarize` of `larmorx-freesurfer` | `src/mri_binarize/main.rs` |
| `utils/randomfields.cpp` 313-345 (3.2-19: only `mri_binarize --rmin`/`--rmax` reach it) | `RFglobalStats` (without a mask, as the program calls it) | `src/mri_binarize/main.rs` |
| `mri_segstats/mri_segstats.cpp` 102-200, 1500-2273 (queue item 3.2-20, track D relay 49) | the file statics (`Options`), `parse_commandline` with its stand-alone options `--xfm2etiv`, `--rescale-by-seg`, `--sum-in`, `--qa-stats` (FS-1103 to FS-1105, FS-1107, FS-1109, FS-1110), `print_usage`, `usage_exit`, `print_help`, `print_version`, `argnerr`, `check_options`, `dump_options`, `singledash` | `src/mri_segstats/options.rs` |
| `mri_segstats/mri_segstats.help.xml` | the help text `print_usage` renders (FreeSurfer's file, copied unchanged) | `src/mri_segstats/mri_segstats.help.xml` |
| `mri_segstats/mri_segstats.cpp` 203-1494, 2279-2298 (3.2-20) | `main` (FS-1101, FS-1102, FS-1106, FS-1111, FS-1113 to FS-1116, FS-1118; the ids' OpenMP loop on `Settings::threads`, results in id order; `MRIsegCount` of every id in one pass), `MRIsegCount`; the in-place `MRImultiplyConst`, `MRIsquare`, `MRIsquareRoot` of the input as their loops (`utils/mri2.cpp`, `utils/mriset.cpp`); the program `mri_segstats` of `larmorx-freesurfer` | `src/mri_segstats/main.rs` |
| `mri_segstats/mri_segstats.cpp` 2300-2364; `include/stats.h` 24-45; `utils/stats.cpp` 1667-1781 (3.2-20) | `LoadStatSumFile`, `DumpStatSumTable`; `STATSUMENTRY`, `SEGSTAT`; `PrintSegStat`; the table without it (`main` 1287-1409) | `src/mri_segstats/table.rs` |
| `mri_segstats/mri_segstats.cpp` 2376-2714 (3.2-20) | `CountEdits` (FS-1111, FS-1112), `WMAnatStats`, `MRIrescaleBySeg` | `src/mri_segstats/qa.rs` |
| `utils/gtm.cpp` 306-391, 2516-2551 (3.2-20: only `mri_segstats` reaches them so far; they move to the GTM module with `mri_gtmpvc`) | `GTMdefaultSegReplacmentList`, `GTMloadReplacmentList` (FS-1108) | `src/mri_segstats/gtm.rs` |
| `mri_info/mri_info.cpp` 56-118, 177-600 (queue item 3.2-08, track A relay 49) | the file statics (`Options`, `Flags`), `parse_commandline` (FS-1146, FS-1149, FS-1150), `print_usage` and `print_help` (their `printf` literals, copied), `print_version`, `check_options`, `usage_exit` | `src/mri_info/options.rs` |
| `mri_info/mri_info.cpp` 121-174, 603-630, 633-1469 (3.2-08) | `main`, `PrettyMatrixPrint`, `do_file` (FS-1147, FS-1148, FS-1152, FS-1153), `__printTagsToFile` (FS-1150, FS-1151), `__reportWarp`; the program `mri_info` of `larmorx-freesurfer` | `src/mri_info/main.rs` |
| `mris_calc/mris_calc.cpp` 62-638, 1627-1824 (queue item 3.2-21, track D relay 50) | `e_operation`, `fn_mul` to `fn_dev`, `Gfn_sum`, `Gfn_prod`, the counts' and minima's statics, `operation_lookup`, `b_outCurvFile_write`; the operand order and NaNs of the official build's object code (its disassembly: `fn_pctdiff`'s `* 0.5`, `fn_bcor`'s `sign * -log10(p)`, the sums' `a + sum`) | `src/mris_calc/ops.rs` |
| `mris_calc/mris_calc.cpp` 56-60, 72-111, 221-278, 695-1626, 1826-2554, 2556-2628 (3.2-21) | `main`, `options_parse`, `options_print`, `version_print`, `synopsis_show`, `simpleSynopsis_show`, `verbosity_set`, `error_exit` and its callers, `xmalloc`, `output_init`, `fileType_find`, `fileIO_errorHander`, `fileRead`, `fileWrite`, `debuggingInfo_display`, `CURV_set`, `CURV_copy`, the label's `array_compressUsingLabelMask`, `primaryCurvs_backup`, `primaryCurvs_compress`, `label_initialize`, `array_expandUsingLabelMask`, `VOL_fileRead`, `VOL_fileWrite`, `CURV_fileRead`, `ascii_fileWrite`, `CURV_fileWrite`, `CURV_arrayProgress_print`, `CURV_process`, `PrintStatToFile`, `CURV_functionRunABC`, `CURV_functionRunAC` (FS-1119 to FS-1126); the program `mris_calc` of `larmorx-freesurfer` | `src/mris_calc/main.rs` |
| `mris_calc/mris_calc.help.xml` | the help text `synopsis_show` renders (FreeSurfer's file, copied unchanged) | `src/mris_calc/mris_calc.help.xml` |
| `mri_add_xform_to_header/mri_add_xform_to_header.cpp` 45-221 (queue item 3.2-10, track A relay 50) | `main` (FS-1154, FS-1155), `get_option`, `usage_exit`, `print_usage`; the program `mri_add_xform_to_header` of `larmorx-freesurfer` | `src/mri_add_xform_to_header/main.rs` |
| `mri_add_xform_to_header/mri_add_xform_to_header.help.xml` (3.2-10) | the help text, copied unchanged (`print_usage` renders it) | `src/mri_add_xform_to_header/mri_add_xform_to_header.help.xml` |
| `utils/mriio.cpp` 11852-11905 (3.2-10; decided for the program's crate: no other program on the default paths calls it) | `MRIwriteInfo` (FS-1156, FS-1157, FS-1158) | `src/mri_add_xform_to_header/write_info.rs` |
| `mri_convert/mri_make_uchar.cpp` 52-285 (queue item 3.2-22, track D relay 51) | `main` (FS-1127, FS-1129), `get_option` (FS-1128), `usage_exit` (its `printf` literals, copied), `MRIconvertToUchar` (FS-1130, FS-1131; the operand order and NaNs of the official build's object code: `255 * FIRST_PERCENTILE`, `x2 * m`, `val * m + b`, `(-b) / m`); `include/mrinorm.h` `DEFAULT_DESIRED_WHITE_MATTER_VALUE`; `utils/transform.cpp` `LTAtransformPoint` (1496-1503, inline: its static matrix kept by the caller); the program `mri_make_uchar` of `larmorx-freesurfer` | `src/mri_make_uchar/main.rs` |
| `mri_mask/mri_mask.cpp` 89-655 (queue item 3.2-26, track D relays 51 and 52) | `main` (FS-1133, FS-1134, FS-1135), `usage`, `print_version`, `get_option` (FS-1132; its option names in the order it compares them: the transform's `-invert` never reached); `include/cma.h`'s labels `-samseg` clears; the program `mri_mask` of `larmorx-freesurfer` | `src/mri_mask/main.rs` |
| `mri_mask/mri_mask.help.xml` (3.2-26) | the help text `usage` renders (FreeSurfer's file, copied unchanged) | `src/mri_mask/mri_mask.help.xml` |
| `mri_cc/mri_cc.cpp` 162-762, 1432-1555 (queue item 3.2-27, track D relay 53) | `main`'s aseg path (FS-1204; the white-matter path `-a 0` not ported yet), `print_usage`, `get_option` (FS-1203; FS-1201 at the slice table); the program `mri_cc` of `larmorx-freesurfer` | `src/mri_cc/main.rs` |
| `mri_cc/mri_cc.cpp` 1557-2662 (3.2-27, relay 53) | `find_cc_with_aseg` (FS-1201), `cc_cutting_plane_correct` (its debug part), `remove_fornix_new` (FS-1202), `find_voxels_close_to_both_hemis` (the window as masks widened along each axis: the same voxels); `include/cma.h`'s callosum labels, `include/macros.h` `RADIANS`, `DEGREES`, `ISINT` | `src/mri_cc/aseg.rs` |
| `mri_cc/mri_cc.help.xml` (3.2-27) | the help text `print_usage` renders (FreeSurfer's file, copied unchanged) | `src/mri_cc/mri_cc.help.xml` |
| `lta_convert/lta_convert.cpp` 116-1441 (queue item 3.2-09, track A relay 51) | `main` (FS-1164, FS-1168, FS-1169), `parseNextCommand` (FS-1159, FS-1160), `parseCommandLine` (FS-1170), `printUsage`; the program `lta_convert` of `larmorx-freesurfer` | `src/lta_convert/main.rs` |
| `lta_convert/lta_convert.cpp` 196-921 (3.2-09) | `shallowCopyLTA`, `readLTA`, `readFSL`, `readMNI`, `readREG`, `readNIFTYREG`, `readITK`, `readVOX`, `writeFSL`, `writeMNI`, `writeREG`, `writeNIFTYREG`, `writeITK`, `writeVOX` (FS-1161 to FS-1165) | `src/lta_convert/formats.rs` |
| `lta_convert/lta_convert.cpp` 116-193, `utils/transform.cpp` 5665-5794 (3.2-09; `conformGeom` decided for the program's crate: no other program on the default paths calls it) | `upsampleGeom`, `downsampleGeom` (FS-1166), `conformGeom` (FS-1167) | `src/lta_convert/geom.rs` |
| libstdc++'s `std::ifstream` `>>` (`float`, words), `peek`, `getline` and `std::ofstream` `<<` with `precision` in the "C" locale (behaviour as C++ defines it and as measured on the official program; no library code) | the program's NiftyReg, ITK and voxel-space text | `src/lta_convert/cxx.rs` |
| `lta_convert/lta_convert.help.xml` (3.2-09) | the help text, copied unchanged (`printUsage` renders it) | `src/lta_convert/lta_convert.help.xml` |
| `io/fs_temp_file.cpp` 13-31 (queue item 3.2-11, track A relay 52) | `main` (FS-1171); the program `fs_temp_file` of `larmorx-freesurfer` (on `fs.core`'s `ArgumentParser` and `makeTempFile`) | `src/fs_temp_file/main.rs` |
| `io/fs_temp_file.help.xml` (3.2-11) | the help text, copied unchanged (`ArgumentParser`'s help renders it) | `src/fs_temp_file/fs_temp_file.help.xml` |
| `scripts/getfullpath` 21-42 (queue item 3.2-12, track A relay 52) | the script (FS-1173 to FS-1176): the default by the name as given, `reproduce` as tcsh runs it; the program `getfullpath` of `larmorx-freesurfer` | `src/getfullpath/main.rs` |
| tcsh 6.21.00 (Ubuntu 22.04's build) and GNU coreutils 8.32's `dirname`, `basename`, `pwd`: behaviour measured on the official script and test scripts (no tcsh or coreutils code) | the word splitting, braces, `~`, `=N`, patterns, command substitution, `echo`, the options and messages the scripts meet | `src/csh.rs` |
| `mri_robust_register/mri_robust_register.cpp` 71-142, 592-1360, 1424-1827, 1838-2370 (queue item 3.2-13, track C relay 44) | `Parameters`, `main` (FS-1071), `printUsage`, `initRegistration` (FS-1072), `parseNextCommand` (FS-1070), `parseCommandLine`; the robust cost's path, with the entropy images (`--cost ROBENT`, relay 45: fs.robust's `entropy.rs`, FS-1074) ; the Powell registration of `LS`, `NCC`, `TB`, `LNCC`, `SAD` (relay 46: fs.robust's `powell.rs`; `MI`, `NMI`, `ECC`, `SCR`, `SB`, the Powell half-way outputs, `--test`, `--debug`: "not ported yet"); the program `mri_robust_register` of `larmorx-freesurfer` | `src/mri_robust_register/main.rs`, `options.rs` |
| `mri_robust_register/MyMRI.cpp` 580-612 (3.2-13) | `MyMRI::setTypeUCHAR`, `MyMRI::setMaxOutsideVal` | `src/mri_robust_register/main.rs` |
| `mri_robust_register/mri_robust_register.help.xml` (3.2-13) | the help text, copied unchanged (`printUsage` renders it) | `src/mri_robust_register/mri_robust_register.help.xml` |
| `mri_vol2vol/mri_vol2vol.cpp` 492-638, 1316-1693, 2148-2420 (queue item 3.2-38, part 1; track A relay 53) | the file statics (`Options`), `parse_commandline` (`--spm-warp`, `--gcam`/`--gcam0`, `--map-point` read their arguments and end as not ported yet), `check_options` (FS-1177, FS-1178, FS-1180, FS-1183), `dump_options`, `print_version`, `argnerr`, `singledash`, `istringnmatch`, `LoadRtal`, `LoadRfsl` | `src/mri_vol2vol/options.rs` |
| `mri_vol2vol/mri_vol2vol.cpp` 1695-2145 (3.2-38) | `print_usage`'s and `print_help`'s texts (their `printf` literals, unescaped) | `src/mri_vol2vol/usage.txt`, `help.txt` |
| `mri_vol2vol/mri_vol2vol.cpp` 640-1310 (3.2-38, part 1) | `main` (FS-1178, FS-1179, FS-1181, FS-1182; the morphs, `--disp` and `--cost` end as not ported yet: part 2) | `src/mri_vol2vol/main.rs` |

Third-party code: none in `mri_normalize.cpp` (FreeSurfer's own file, MGH's notice). Its library
calls are the other crates' (`larmorx-freesurfer-filter`'s normalisation, `-volume`, `-surf`,
`-gca`; `GCAsourceVoxelToPrior` added to `larmorx-freesurfer-gca` for `-atlas`).

Third-party code: none in `csv.h`, `stats.cpp`, `fsgdf.cpp` and `mri_convert.cpp` (FreeSurfer's own, MGH's notice; `csv.h` carries no notice
and is FreeSurfer's). Their library calls are the other crates' (`larmorx-freesurfer-io`'s TSV
reader, file names, format identification and `MRIwrite`; `-core`'s matrices, version and help;
`-volume`'s orientation check) and `larmorx-core`'s `strtod`/`strtof` (C's conversions, black box).

Third-party code: none in `mri_info.cpp` (FreeSurfer's own file, MGH's notice). Its library calls are
the other crates' (`larmorx-freesurfer-io`'s format identification, header and volume reads, the tag
writer and the colour table's text; `-gca`'s morph reader and `cma_label_to_name`; `-mri`'s
geometry matrices and `vgprint`; `-volume`'s orientation, slice direction, conformed test,
tkregister matrices and histogram; `-core`'s matrices and version) and `larmorx-io`'s znzlib.

Third-party code: none in `lta_convert.cpp` (FreeSurfer's own, MGH's notice; Martin Reuter's) and
`conformGeom` (FreeSurfer's own). Their library calls are the other crates' (`larmorx-freesurfer-volume`'s
`LTAreadEx`, `LTAchangeType`, `LTAprint`, `LTAwrite`, the conformed sizes; `-io`'s `MRIreadHeader`;
`-mri`'s geometry; `-core`'s matrices, version and help; `-gca`'s readers behind the `.lta` reader).

Third-party code: none in `mri_add_xform_to_header.cpp` and `MRIwriteInfo` (FreeSurfer's own,
MGH's notice). Their library calls are the other crates' (`larmorx-freesurfer-io`'s file tests,
`MRIreadInfo`, `MRIread`, `MRIwrite` and FreeSurfer's `.xfm` reader on `larmorx_io::xfm`, MINC's
volume_io: MNI's notice; `-mri`'s transform matrices; `-volume`'s slice direction; `-core`'s
version, help and error messages).

Third-party code: none in `mri_vol2vol.cpp` (FreeSurfer's own, MGH's notice; Doug Greve's). Its
library calls are the other crates' (`larmorx-freesurfer-io`'s `MRIread`, `MRIreadHeader`,
`MRIwrite`, `regio_read_register`, `regio_write_register`, `regio_read_registermat`,
`regio_read_mincxfm`, `CTABreadASCII`, `fio_dirname`; `-volume`'s `MRIvol2Vol`,
`MRIvol2VolTLKernel`, `MRIvol2VolDelta`, `MRIvol2VolFill`, `MRIvol2VolVSM`, `TransformRegDat2LTA`,
`MRIcrop`, `MRIreverseSlices`, `MRIhalfCosBias`, `MRImultiplyConst`, `MRIsetVoxelToRasXform`,
`MRIangles2RotMat`, `DevolveXFM`, `MRIrandn`, `MRIbinarize`, the LTA reader, writer, inverse,
change of type and geometry; `-gca`'s `TransformLTA2RegDat`; `-filter`'s `MRIsoapBubble`; `-mri`'s
`MRItkRegMtx`, `MRIfsl2TkReg`, the voxel-to-RAS matrices, `MRIboundingBox`, `LTAalloc`; `-core`'s
matrices, version options, `PDFtodSeed`, `GetVmPeak`, `nint`) and `larmorx-core`'s `drand48`
(POSIX's definition).

## Changes from the originals

- Each function takes the volumes as `main` passes them: a control volume that is both a
  function's source and its destination is one `&mut Mri` (`main` never passes two).
- `remove_surface_outliers` and `remove_outliers_near_surface` do nothing (FreeSurfer's return at
  their start; the clone `remove_surface_outliers` makes first is never used).
- `main` (relay 47): its file statics are `Options`, `mrinorm.cpp`'s state (`NormState`), `mni` and
  `mri_not_control`, one struct (`Prog`) for the run; `Progname` is the name recon-all runs it by
  (`mri_normalize`); volumes FreeSurfer leaks are dropped when the C code stops using them (a
  lower peak: `MRIcopy`'s copy before `MRInormalize`, the bias field `-l` reads); the timer is the
  monotonic clock (fs.core's `Timer`). `MRIcombineDistanceTransforms(one, dist, dist)` (a source
  that is also the destination) is computed into a new volume of the same geometry, which holds
  the same values (it is voxel by voxel). FS-1017 (a step that gives no volume where FreeSurfer
  goes on with none, a crash: both modes end with an error).
- FS-1014 (the white-matter peak of an empty histogram: 0, as glibc's heap gives it), FS-1015 (a
  walk of `-T2`'s outlier removal past 10,000,000 steps ends the program), FS-1016 (an option
  short of its arguments: an error where FreeSurfer crashes; `(null)` where it reads the
  environment).
- `main`'s variables are one struct (`Options`), the unset ones `None` until an option sets them;
  the DICOM reader's globals the options set are kept in it (`DicomSettings`) until the DICOM
  reader is ported (queue item 3.7-14); `setenv`/`unsetenv` act on the program's environment
  table (`run::Environment`).
- `std::stof`'s `ERANGE` comes from fs.core's scanner on the characters `strtof` read (glibc's
  `strtof` and `%f` share the conversion).
- FS-948 (the item count's read past the line: the count kept, no notice), FS-949 and FS-950 (rule
  A), FS-1031 to FS-1042 (crashes and memory: both modes as `specs/mri_convert.md` says).
- The outline reader (relay 48): FS-1054 (a crash in an error message), FS-1055 (entry 5307's
  never-reset score: rule A, `reproduce` sums from 0 per slice as the official build does,
  observed), FS-1056 (writes outside a never-read array the official build leaves out), FS-1057
  (reads past the outline field), FS-1058 and FS-1059 (kept, rule D), FS-1060 (the inverted
  warning: rule A), FS-1141 to FS-1145 (memory, files, the fill's stack, the field's size);
  `docs/findings/freesurfer-convert.md` Part 4.
- Relay 47: `main`'s volumes are owned values (a step's result replaces the volume; FreeSurfer's
  `MRIfree`s are drops); its NULL checks follow FreeSurfer's, the unchecked ones FS-1052; the
  float arithmetic the output and messages show through fs.core's `sse` helpers (C's operand
  order, x86-64's NaNs), `x *= -1.0` as a sign flip (the object code's `xorps`);
  `applyGaussianFilter`'s constants as the official build has them (`logf(5)` folded,
  `docs/findings/freesurfer-convert.md` Part 3); `run` reads the process's standard error's
  access mode and tracks writes to it (`Globals::stderr`, the C library's `perror`); DICOM inputs
  wait for 3.7-14 (fs.io's not-ported message), outline inputs for relay 48.
- `mri_info` (relay 49): its file statics are `Options` (the input list a growable list: FS-1149);
  `fpout` is standard output or `--o`'s file (buffered, flushed on every exit, as `exit()` flushes
  it). A call FreeSurfer makes with a null pointer where it crashes (FS-1148 with `reproduce`,
  FS-1150, FS-1152) prints what FreeSurfer printed before the crash where that is defined, then an
  error, status 1. `--otags` with a file that cannot be created (FS-1151 with `reproduce`): the tag
  writer's own lines that do not depend on the file (`saving original ras2vox`, the colour table's
  `CTABwriteIntoBinary: fp was NULL`); its debug lines with `--diag-debug` (positions of a file that
  is not there, -1) are not printed. `MRIread` of a type given with `-it` (FS-1147's correction) is
  `MRIread` of `<name>@<type>`, its own way of naming a type.
- `mris_calc` (relay 50): the file statics are one struct (`Calc`), the arrays owned vectors (the
  C's frees are drops; `norm`'s two arrays FreeSurfer leaks are not kept); standard output is held
  until the C program flushes it, so that a crash or an abort in `reproduce` loses what
  FreeSurfer's buffer would (at most whole 4,096-byte blocks written); the elementwise loops run
  on one thread (FreeSurfer's only at ROMP's experimental level); the C library's `powf`,
  `atan2f`, `log10f`, `logf`, `pow`, `log10` through larmorx-core's `Libm` choice with glibc's NaNs
  in both modes, `fmodf` as glibc's measured results (`ops::fmodf_glibc`); `stat` and `strtof`'s
  `errno` modelled (the fatal errors print it); `atof`'s `ERANGE` from fs.core's scanner;
  `docs/findings/freesurfer-mris-calc.md`.
- `mri_make_uchar` (relay 51): standard output is held until the end (FreeSurfer's buffer, never
  flushed before), so that a crash in `reproduce` (FS-1127, FS-1128) loses what it would; the
  per-voxel `MRIvoxelToTalairach` of a coronal volume, whose point is never used, is called only
  while the geometry's matrices are not cached yet (its only other effect; an input that is not
  coronal calls it at every voxel in `reproduce` for its error); `docs/findings/
  freesurfer-mri-make-uchar.md`.
- `mri_mask` (relays 51 and 52): standard output held until the end (`Cout`), what a crash in `reproduce`
  keeps being the part before the last flush (an explicit one, or a message of the C++ error stream: `std::cerr`
  is tied to `std::cout`, fs.core's `StderrState::cxx_messages`) and the full 4,096-byte buffers after it;
  `-no-allow-diff-geom`'s `setenv` changes the run's own copy of the environment, which `MRImask` reads; the C's
  `(int)` of a voxel or of `-transfer`'s value is fs.core's `float_to_int` (FS-17 for NaN and out of range).
  FS-1133 (a `-BBEQ` region `NULL`, `REGIONprint(stdout, NULL)`), FS-1134 (`LTAreduce` `NULL`, then
  `lta->xforms[0]`), FS-1135 (`TransformApplyType` `NULL`: the rest of `main` without a mask modelled in
  `null_mask`, each later step's crash or message as the official's) by default an error with status 1;
  `docs/findings/freesurfer-mri-mask.md`.
- `mri_add_xform_to_header` (relay 50): the transform file is read with
  `input_transform_file_ex` (what C's `transform` argument holds after a failure, and `errno`),
  the volume keeping a partial transform as C's does; a transform never read gives no matrices
  (C's zeroed `General_transform`: type `LINEAR` with null pointers). FS-1154 and FS-1156 with
  `reproduce` print the C library's `*** buffer overflow detected ***: terminated` and end with
  status 134 (the official build's fortified copy aborts); standard output as written (C's buffer,
  lost at the abort when it is not a terminal, is not modelled).

Third-party code: none in `mri_robust_register.cpp` and its `MyMRI` pieces (Martin Reuter's,
MGH's notice). Their library calls are `larmorx-freesurfer-robust`'s (`RegRobust`, `MyMatrix`'s
conversions and polar decomposition, `MyMRI::MRIvalscale`), the other crates' (`-io`'s `MRIread`
and `MRIwrite`, `-gca`'s `TransformRead`, `-volume`'s LTA conversions, writer and resampling,
`MRImask`, change of type, `MRIlinearScale`, C++'s `>> double`; `-filter`'s `MRIsobel`; `-core`'s
version, help and errors) and `larmorx-core`'s VNL ports (`vnl_matlab_print` of a diagonal matrix
added for `Scale`, BSD-3-Clause (VXL)).

- `mri_robust_register` (relay 44): `Parameters` is `Options` (owned strings; the images are the
  values `initRegistration` returns); `RegPowell` is fs.robust's `RegRobust` with `powell` set
  (relay 46; its joint-histogram costs, `SB` and its half-way outputs end with "not ported yet"); the `--iscalein` file is read
  whole and C++'s `>> double` applied (a folder: nothing read, as libstdc++'s stream opens it), the
  `--iscaleout` file written whole (both as their streams write them; `errno` set from a failed
  open); FS-1070 and FS-1071 with `reproduce` print the C++ runtime's and the C library's lines and
  end with status 134 (standard output as flushed: each option's line is). The time taken comes
  from the clock (masked by the probe).

- `mri_vol2vol` (relay 53): its file statics are `Options` (owned strings; `R` an owned matrix:
  FreeSurfer's `R` points into the `--lta` array's matrix, and nothing reads the array's matrix
  after `R` changes); `in`/`mri_template` and `mov`/`targ` are one pair of volumes (FreeSurfer's two
  names point at the same two); the options' `setenv` calls change the program's own copy of the
  environment; the version options keep `argc` (`handle_version_option_whole`, FS-07); the noise
  seed's `unsigned long` is stored as an `int` (its low 32 bits); a `reproduce` abort (FS-1178)
  prints the C library's `*** buffer overflow detected ***: terminated` and ends with status 134,
  the crashes (FS-1179's division by 0: 136; FS-1180, FS-1181: 139) end with their statuses, the
  unflushed standard output FreeSurfer loses there not modelled (the probe counts those cases).
