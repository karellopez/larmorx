# Provenance: larmorx-io

## NIfTI-1 and NIfTI-2 (`src/nifti/`)

**Original Rust implementation; no source code was copied.**

- **File format:** the NIfTI-1 (`nifti1.h`) and NIfTI-2 (`nifti2.h`) headers published by the NIfTI Data Format Working Group (public domain). Field offsets were cross-checked against nibabel's `header_dtype` definitions.
- **Semantics:** how fields are interpreted, fixed and written reproduces **nibabel 5.4.2** (MIT licence, © the nibabel developers), the library fMRIPrep uses for I/O. The parity suite checks every behaviour below against nibabel itself ([docs/validation/nifti-io.md](../../docs/validation/nifti-io.md)).

| larmorx | nibabel 5.4.2 behaviour it reproduces |
|---|---|
| `NiftiHeader::sniff` | `AnalyzeHeader.guessed_endian` (byte order from `dim[0]`, then `sizeof_hdr`) |
| `NiftiHeader::apply_load_fixes` | the header checks run on load: `_chk_datatype`, `_chk_bitpix`, `_chk_pixdims`, `_chk_qfac`, `_chk_magic`, `_chk_offset`, `_chk_qform_code`, `_chk_sform_code` (error level 40) |
| `NiftiHeader::shape`, `set_shape` | `Nifti1Header.get_data_shape` / `set_data_shape`, including FreeSurfer's long-vector (`dim[1] = -1`, `glmin`) and ico7 (`27307 × 1 × 6`) conventions |
| `NiftiHeader::qform` | `Nifti1Header.get_qform`, `quaternions.fillpositive` (threshold 3·eps of the header precision) and `quaternions.quat2mat` (same operation order) |
| `NiftiHeader::set_qform` | `Nifti1Header.set_qform` (zooms, `qfac`, polar decomposition to strip shears, `w ≥ 0`) |
| `NiftiHeader::base_affine`, `best_affine` | `get_base_affine` (`shape_zoom_affine` with x flip) and `get_best_affine` |
| `NiftiHeader::slope_inter` | `Nifti1Header.get_slope_inter` |
| `nifti::read`, `Scaling` | `ArrayProxy._get_scaled` and `volumeutils.apply_read_scaling` (result types; multiply only if slope ≠ 1, add only if intercept ≠ 0; numpy's complex arithmetic) |
| `nifti::read_first_volumes` (the confounds' part, 2026-11-12) | `ArrayProxy.__getitem__` with `[:, :, :, :k]` (`fileslice`: only the first `k` volumes of each index after the fourth axis are read, the values scaled as `_get_scaled` scales them) |
| `nifti::header_for_image` | `SpatialImage.update_header` and `Nifti1Pair._affine2header` (keep the header's xforms when the affine is `allclose` to them; else sform code 2, qform code 0) |
| `Extension::read_all` | `Nifti1Extensions.from_fileobj` |

The numerical algorithms for the polar decomposition (scaled Newton iteration, Higham 1986) and the rotation-to-quaternion conversion (Shepperd 1978) are standard methods implemented from their publications, in `larmorx-core`.

## nibabel's scaled writer (`python/larmorx/io/nibabel_writer.py`)

**Ported** from nibabel 5.4.2 (MIT licence, © the NiBabel developers; the release in
`upstream.tsv`), in Python with numpy's own arithmetic (the SPDX header names both licences):

| larmorx | nibabel 5.4.2 |
|---|---|
| `_SlopeInterWriter` (`calc_scale`, `_do_scaling`, `_iu2iu`, `_range_scale`, `_writing_range`, `needs_nan2zero`) | `arraywriters.py`: `make_array_writer`, `ArrayWriter`, `SlopeArrayWriter`, `SlopeInterArrayWriter`, `get_slope_inter` |
| `_array_to_stored`, `_write_data`, `_finite_range`, `_best_write_scale_ftype`, `_ftype4scaled_finite`, `_working_type`, `_dt_min_max`, `_better_float_of`, `_matching_float` | `volumeutils.py`: `array_to_file`, `_write_data`, `finite_range`, `best_write_scale_ftype`, `_ftype4scaled_finite`, `working_type`, `_dt_min_max`, `better_float_of`, `_matching_float` |
| `_shared_range`, `_floor_exact`, `_ceil_exact`, `_floor_log2`, `_int_abs` | `casting.py`: `shared_range`, `floor_exact`, `ceil_exact`, `floor_log2`, `int_abs` |
| `nibabel_stored` (the scaling computed when the header's `scl_slope`/`scl_inter` are unset), its slope and intercept checks | `analyze.py` `AnalyzeImage.to_file_map`; `nifti1.py` `Nifti1Header.set_slope_inter` |
| `_Extended` | `casting.best_float()`'s `np.longdouble` on x86-64 Linux (x87 extended precision), emulated in exact rational arithmetic rounded to 64 bits, so every platform gives x86-64 Linux's slope and intercept |

The voxel data and the header are written by this crate's NIfTI writer
(`_core.nifti_header_for_image`, `_core.nifti_write`). Moved here from `lx.sdcflows`, where the
sdcflows port (field maps, part A) first needed it.

## nibabel's header arithmetic (`src/nifti/header.rs`: `nibabel_qform`, `set_qform_nibabel`; `src/nifti/mod.rs`: `header_for_image_nibabel`; `python/larmorx/io/nibabel_header.py`)

**Ported** from nibabel 5.4.2 (MIT licence, © the NiBabel developers) and numpy 2.5.4
(BSD-3-Clause):

| larmorx | Upstream |
|---|---|
| `nibabel_qform`, `NibabelQform`, `NiftiHeader::set_qform_nibabel`, `set_affine_nibabel`, `header_for_image_nibabel`; `nibabel_header.header_for_image`, `set_xform` | nibabel `nifti1.py` (`Nifti1Header.set_qform`, `Nifti1Pair._affine2header`), `quaternions.py` (`mat2quat`); numpy `linalg/_linalg.py` (`svd`, `eigh`, `det`) through `larmorx_transform::numpy_linalg` (its provenance there) |
| `nibabel_header.nibabel_affine`, `load_nibabel`, `_get_qform`, `_base_affine`, `_ext_sqrt`, `_ext_to_float64` | nibabel `nifti1.py` (`get_best_affine`, `get_qform`, `get_qform_quaternion`, `quaternion_threshold`), `nifti2.py` (`quaternion_threshold`), `analyze.py` (`get_base_affine`), `affines.py` (`shape_zoom_affine`), `quaternions.py` (`fillpositive`, `quat2mat`, `MAX_FLOAT`); numpy's `longdouble` `matmul` inner loop and `LONGDOUBLE_dot` (`arraytypes.c.src`), emulated exactly with the writer's `_Extended` (x87's 64-bit significand) |
| `nibabel_header.io_orientation`, `ensure_positive_cosines` | nibabel `orientations.py` (`io_orientation`); sdcflows `utils/tools.py` (`ensure_positive_cosines`, Apache-2.0); the polar factor through `numpy_linalg::io_orientation_polar` |

## nibabel's affine of a header, in Rust (`src/nifti/nibabel_affine.rs`)

**Ported** from nibabel 5.4.2 (MIT licence, © the NiBabel developers) and numpy 2.5.4
(BSD-3-Clause), for tedana's t2smap replica (2026-11-08, `larmorx-gpl`), the Rust counterpart of
`nibabel_header.nibabel_affine`:

| larmorx | Upstream |
|---|---|
| `NiftiHeader::qform_nibabel`, `best_affine_nibabel`; `X87` (x87 extended precision: add, multiply, divide, square root, rounding to double, exactly with integer significands) | nibabel `nifti1.py` (`get_best_affine`, `get_qform`, `quaternion_threshold`), `nifti2.py` (`quaternion_threshold`), `quaternions.py` (`fillpositive`, `quat2mat`); numpy's `longdouble` `matmul` inner loop and `LONGDOUBLE_dot` (`arraytypes.c.src`) |

## Parallel gzip (`src/gzip.rs`)

**Original implementation** of the parallel-deflate method popularised by pigz (Mark Adler): fixed-size blocks, each compressed independently with the preceding 32 KiB as a dictionary and ended with a sync flush. No pigz code was used. Deflate itself is [zlib-rs](https://github.com/trifectatechfoundation/zlib-rs) (Zlib licence), through [flate2](https://github.com/rust-lang/flate2-rs) (MIT/Apache-2.0).

## ITK's reading of NIfTI geometry (`src/nifti/itk.rs`)

**Ported** (logic re-expressed in Rust, no code copied verbatim) from the release tags in [UPSTREAM.md](../../UPSTREAM.md):

| larmorx | Upstream (ITK v5.4.5, `f51594ad`) | Licence |
|---|---|---|
| `itk_geometry`: qform/sform choice, origin and direction (LPS), units, number of dimensions | `Modules/IO/NIFTI/src/itkNiftiImageIO.cxx`: `ReadImageInformation`, `SetImageIOOrientationFromNIfTI`, `IsAffine` | Apache-2.0 |
| `ItkGeometry::with_positive_spacing` | `Modules/IO/ImageBase/include/itkImageFileReader.hxx`: `GenerateOutputInformation` (negative spacing) | Apache-2.0 |
| dim/pixdim fixes, `qto_xyz`/`sto_xyz` construction, `quatern_to_mat44` | `Modules/ThirdParty/NIFTI/src/nifti/niftilib/nifti1_io.c`: `nifti_convert_nhdr2nim`, `nifti_quatern_to_mat44`, `NIFTI_VERSION` | public domain |

Single-precision steps are kept where ITK computes in `float`. The SVD comparisons use larmorx's own symmetric eigen-solver instead of vnl's LINPACK SVD; they only decide whether two matrices are within 1e-4, so the choice of algorithm does not change the outcome in practice. The parity suite `itk-geometry` checks the result against ITK 5.4.5 itself (through ANTsPy) on every catalog file.

## ITK's reading of NIfTI voxel values (`src/nifti/itk.rs`, `read_itk_image`)

**Ported** (logic re-expressed in Rust) from ITK v5.4.5 `Modules/IO/NIFTI/src/itkNiftiImageIO.cxx`
(Apache-2.0):
- `MustRescale`;
- `RescaleFunction`;
- `CastCopy`, which promotes scaled integers to float32;
- the slope and intercept rules, and nifti_clib's `FIXED_FLOAT`;
- `ConvertRASToFromLPS_CXYZT` for `NIFTI_INTENT_DISPVECT` vectors (`m_ConvertRASDisplacementVectors`);
- nifti_clib's `nifti_read_buffer`: non-finite float values read as 0 (public domain).
- nifti_clib's `nifti_findimgname` (`nifti1_io.c`, public domain): the order in which it looks
  for the image file of a name (`.nii`, `.nii.gz`, `.img`, `.img.gz` for a single file),
  `NiftiPaths::nifti_clib_image_path` and `read_with_data_from`, used only to reproduce
  defect ITK-08 (the voxels of `x.nii` read for `x.nii.gz`).

## ITK's writing of NIfTI headers (`src/nifti/itk/write.rs`)

**Ported** (logic re-expressed in Rust) from ITK v5.4.5 (Apache-2.0) and its bundled
nifti_clib (public domain):
- `Modules/IO/NIFTI/src/itkNiftiImageIO.cxx`: `WriteImageInformation`,
  `SetNIfTIOrientationFromImageIO` (the header fields, both xforms, the dictionary's
  `descrip` and `aux_file`);
- `Modules/ThirdParty/NIFTI/src/nifti/niftilib/nifti1_io.c`: `nifti_simple_init_nim`,
  `nifti_convert_nim2nhdr`, `nifti_make_orthog_mat44`, `nifti_mat44_to_quatern`,
  `nifti_mat33_polar`, `nifti_mat33_inverse`, `nifti_mat33_determ`, `nifti_mat33_rownorm`,
  `nifti_mat33_colnorm` (single precision kept where nifti_clib uses `float`);
- `Modules/Core/Common/include/itkImageBase.hxx`: `SetDirection` (element-wise copy) and
  `itkSetMacro` for `SetOrigin`, modelled by `ItkGeometry::stored_in_new_image`.

Every output header of the ImageMath, ThresholdImage and MultiplyImages parity suites matches
ITK's byte for byte.

## ITK transform files (`src/itk_transform.rs`)

**Ported** (logic re-expressed in Rust) from ITK v5.4.5 (Apache-2.0):
- `Modules/IO/TransformInsightLegacy/src/itkTxtTransformIO.cxx` (text format; with
  `parse_itk_text_with(_, Precision::Float)`, the numbers of both parameter lines read as
  float, as `TxtTransformIOTemplate<float>` reads them into `OptimizerParameters<float>`);
- `Modules/IO/TransformMatlab/src/itkMatlabTransformIO.cxx` (MATLAB v4 format; a composite as
  one variable, a float transform's fixed parameters as floats) and VXL's
  `core/vnl/vnl_matlab_write.cxx`, `vnl_matlab_header.h` (the variable header);
- `Modules/IO/TransformInsightLegacy/src/itkTxtTransformIO.cxx` (`Write`, `print_vector`) and
  `Modules/Core/Common/src/itkNumberToString.cxx` (`itk_number`, `itk_number_f32`: the
  double-conversion library's `DoubleToStringConverter::EcmaScriptConverter().ToShortest`,
  BSD-3-Clause, re-expressed from its documented settings on Rust's shortest digits);
- `nifti::itk::write_itk_vector_image`, `itk_vector_header`: `Modules/IO/NIFTI/src/itkNiftiImageIO.cxx`
  (`WriteImageInformation` for vector pixels, `Write`'s component-major data);
- the displacement-field reading path of ANTs v2.6.5's `itk::ants::ReadTransform`
  (`Utilities/itkantsReadWriteTransform.h`).

## ITK's HDF5 transform files (`src/itk_hdf5.rs`, `src/itk_transform_names.rs`)

**Ported** (logic re-expressed in Rust) from ITK v5.4.5 (Apache-2.0), on the HDF5 layer of
crate `larmorx-hdf5`:
- `Modules/IO/TransformHDF5/src/itkHDF5TransformIO.cxx` (`CanReadFile`, `CanWriteFile`, `Read`,
  `ReadParameters`, `ReadFixedParameters`, `Write`, `WriteOneTransform`, `WriteParameters`,
  `WriteFixedParameters`, `WriteString`; the exception lines quoted in messages);
- `Modules/IO/TransformBase/include/itkTransformIOBase.h` (`CorrectTransformPrecisionType`),
  `src/itkTransformIOBase.cxx` (`CreateTransform`), `src/itkTransformFileReader.cxx`
  (`Update`: the IO search and its messages), `src/itkCompositeTransformIOHelper.cxx`;
- `Modules/Core/Common/src/itkExceptionObject.cxx` (`Print`, the report's layout);
- ANTs v2.6.5 `Utilities/itkantsReadWriteTransform.h` (`ReadTransform`, `WriteTransform`: the
  messages and the composite flattening) and `Examples/antsApplyTransforms.cxx:536-539` (the
  `MatrixOffsetTransformBase` it registers);
- for `unregistered_class` (the factory's message for the MATLAB and text readers as well):
  `Modules/IO/TransformMatlab/src/itkMatlabTransformIO.cxx` and
  `Modules/IO/TransformInsightLegacy/src/itkTxtTransformIO.cxx` (`CreateTransform` of the
  precision-corrected name), and ANTs v2.6.5 `Examples/itkantsRegistrationHelper.h:1388-1396`
  (the `MatrixOffsetTransformBase` antsRegistration registers for its dimension).

`itk_transform_names.rs` lists the transform classes registered with ITK's factory in the ANTs
2.6.5 build, copied from the oracle's own message for an unknown class.

## MNI transform files (`src/xfm/`; FreeSurfer's foundations, item 3.1-14, 2026-11-12/13)

**Ported** from MINC 1.5.1's volume_io (the release tarball `minc-1.5.1.tar.gz`, SHA-256
`0b41d5bb1b178c62792c9e4fbd8ddd971d1a90aad7c257ff00bcbdddc3998afe`, pinned at
`<workspace>/tags/minc-1.5.1`; © 1993-1995 David MacDonald and 1993-2000 Peter Neelin, McConnell
Brain Imaging Centre, Montreal Neurological Institute, McGill University; MNI's permissive notice,
`LICENSES/LicenseRef-MNI.txt`; the files' SPDX `Apache-2.0 AND LicenseRef-MNI`):

| larmorx | MINC 1.5.1 `volume_io/` |
|---|---|
| `xfm/read.rs`: `CStream`, `get_nonwhite`, `skip_expected`, `input_string`, `keyword_and_equal`, `input_real`, `input_int`, `input_reals`, `unget_string`, `input_one_transform`, `input_transform`, `input_transform_file` | `MNI_formats/mni_io.c` (`mni_get_nonwhite_character`, `mni_skip_expected_character`, `mni_input_string`, `mni_input_keyword_and_equal_sign`, `mni_input_real`, `mni_input_int`, `mni_input_reals`, `unget_string`), `MNI_formats/gen_xf_io.c` (`input_one_transform`, `input_transform`, `input_transform_file`), `Prog_utils/files.c` (`input_character`, `unget_character`) |
| `xfm/write.rs` | `MNI_formats/gen_xf_io.c` (`output_one_transform`, `output_transform`, `output_transform_file`), `MNI_formats/mni_io.c` (`output_comments`) |
| `xfm/mod.rs`: `GeneralTransform` and its methods, `general_transform_point`, `handle_internal_error` | `MNI_formats/gen_xfs.c` (`create_linear_transform`, `get_transform_type`, `get_n_concated_transforms`, `get_nth_general_transform`, `get_linear_transform_ptr`, `get_inverse_linear_transform_ptr`, `transform_or_invert_point`, `copy_and_invert_transform`, `create_inverse_general_transform`, `invert_general_transform`, `concat_general_transforms`), `Prog_utils/print.c` (`handle_internal_error`, `abort_if_allowed`) |
| `xfm/mod.rs`: `make_identity_transform`, `close_to_identity`, `concat_transforms`, `homogenous_transform_point`, `transform_point`, `compute_transform_inverse` | `Geometry/transforms.c`, `Geometry/inverse.c` |
| `xfm/gaussian.rs` | `Geometry/gaussian.c` (`scaled_maximal_pivoting_gaussian_elimination`, `..._real`, `invert_square_matrix`) |
| `xfm/files.rs` | `Prog_utils/files.c` (`expand_filename`, `get_user_home_directory`, `file_exists`, `remove_directories_from_filename`, `has_no_extension`, `extract_directory`, `file_exists_as_compressed`, `open_file`, `open_file_with_default_suffix`, `print_system_error`), `Prog_utils/string.c` (`equal_strings`, `string_ends_in`, `concat_char_to_string`) |

The numbers are read with `larmorx_core::cscan` (C's `sscanf("%lf")`, from the C standard and
glibc's behaviour) and written with `larmorx_core::cfmt` (`%.15g`). Decompression is in memory
(flate2 for gzip, the `bzip2` crate's pure-Rust backend for bzip2) where volume_io runs `gunzip`
and `bunzip2` (FS-71). A program's own variant of volume_io plugs in through `Dialect` (no such
variant is in this crate; FreeSurfer's copy was not read, and what it changes is described in a
file closed to FreeSurfer's clean-room implementers, `docs/findings/freesurfer-permissive.md`).
Defects FS-71 to FS-75 (`larmorx_core::defects`).

**Identical to MINC 1.5.1** (its files built from the tarball with GCC 8.4.0 at `-O3`):
`validation/probes/fs-minc-nifti` (families `xfm`, `ops`, `write`), numbers in
`docs/findings/minc-xfm.md`.

## nifticlib's file names and znzlib (`src/nifti/niftilib.rs`, `src/znz.rs`; item 3.1-15, 2026-11-13)

**Ported** from nifticlib 2.0.0 (`nifticlib-2.0.0.tar.gz`, SHA-256
`a3e988e6a32ec57833056f6b09f940c69e79829028da121ff2c5c6f7f94a7f88`, pinned at
`<workspace>/tags/nifticlib-2.0.0`; public domain: Robert W Cox's `nifti1_io.c`, Mark Jenkinson's
`znzlib.c`) and zlib 1.2.11's file layer (`zlib-1.2.11.tar.gz`, SHA-256
`c3e5e9fdd5004dcb542feda5ee4f0ff0744628baf8ed2dd5d66f8ca1197cb1a1`, pinned at
`<workspace>/tags/zlib-1.2.11`; © 1995-2017 Jean-loup Gailly and Mark Adler, the zlib licence;
`znz.rs`'s SPDX `Apache-2.0 AND Zlib`; altered: translated into Rust, inflate from zlib-rs
through flate2, deflate zlib 1.2.11's own from `larmorx_core::zlib` since 2026-11-15):

| larmorx | Upstream |
|---|---|
| `niftilib::find_file_extension`, `is_gzfile`, `makebasename`, `validfilename`, `is_complete_filename`, `fileexists`, `findhdrname`, `is_nifti_file`, `NiftiOptions` | `niftilib/nifti1_io.c` (`nifti_find_file_extension`, `nifti_is_gzfile`, `nifti_makebasename`, `nifti_validfilename`, `nifti_is_complete_filename`, `nifti_fileexists`, `nifti_findhdrname`, `is_nifti_file`, `compare_strlist`, `is_mixedcase`, `is_uppercase`, `make_uppercase`, `fileext_compare`, `fileext_n_compare`, `g_opts`), `nifti1.h` (`NIFTI_VERSION`, `NIFTI_ONEFILE`) |
| `znz::ZnzFile` | `znzlib/znzlib.c` (`znzopen`, `Xznzclose`, `znzread`, `znzwrite`, `znzseek`, `znzrewind`, `znztell`, `znzputs`, `znzgets`, `znzflush`, `znzeof`, `znzputc`, `znzgetc`) |
| `znz::GzFile` | zlib 1.2.11 `gzlib.c` (`gz_reset`, `gz_open`, `gzrewind`, `gzseek64`, `gztell64`, `gzeof`, `gz_error`), `gzread.c` (`gz_load`, `gz_avail`, `gz_look`, `gz_decomp`, `gz_fetch`, `gz_skip`, `gz_read`, `gzread`, `gzgetc`), `gzwrite.c` (`gz_init`, `gz_comp`, `gz_zero`, `gz_write`, `gzwrite`, `gzputc`, `gzputs`, `gzflush`, `gzsetparams`), `gzclose.c` and `gzclose_r`/`gzclose_w` |
| `znz::StdioFile` | C's `fopen`, `fread`, `fwrite`, `fseek`, `ftell`, `feof`, `fgetc` by their description (ISO C 7.21) and glibc's behaviour (no glibc code) |

The decompressed bytes are zlib's; since 2026-11-15 (track C relay 30) the compressed bytes a
writer makes are zlib 1.2.11's too: `gz_init`'s buffers (twice `want` bytes of input, `want` of
output) and `gz_comp`'s loop as zlib keeps them, the mode's strategy letters (`f`, `h`, `R`,
`F`), deflate from `larmorx_core::zlib` (the probe `validation/probes/zlib-deflate`: files
identical to Ubuntu 22.04's `libz.so.1`, but FS-78's). Defects FS-76 and FS-77 (nifticlib; FS-77 with its
`reproduce`) and FS-78 (zlib 1.2.11), `larmorx_core::defects`. `NiftiOptions` holds nifticlib's
settings a program can change (`allow_upper_fext` and the compressed extensions); the variants of
other programs' copies are not in this crate (`docs/findings/freesurfer-permissive.md`, closed to
FreeSurfer's clean-room implementers).

## GIFTI: gifticlib 1.12 over a translation of expat 2.0.1

| larmorx file | Upstream | Licence |
|---|---|---|
| `src/gifti/mod.rs` | gifticlib 1.12 (`gifti_io.c`, `gifti_io.h`, `gifti_xml.h`: the structures, type and intent tables, checks, sizes, metadata, copies, the free functions' messages, displays) | public domain (Richard Reynolds, NIH/NIMH) |
| `src/gifti/read.rs` | gifticlib 1.12 (`gifti_xml.c`: `gxml_read_image`, the element stack and callbacks, the ASCII, Base64Binary and GZipBase64Binary decoders, `gxml_data`'s state between calls; `gifti_io.c`: `gifti_read_extern_DA_data`, `gifti_check_swap`); zlib 1.2.11's `uncompress2` loop (zlib licence) over zlib-rs | public domain; Zlib |
| `src/gifti/write.rs` | gifticlib 1.12 (`gifti_xml.c`: `gxml_write_image`, `gxml_write_gifti`, the `ewrite_*` functions, `gxml_disp_b64_data`; `gifti_io.c`: `gifti_write_image`, `gifti_write_extern_DA_data`); GZipBase64Binary data by zlib 1.2.11's `compress2` (`larmorx_core::zlib`, since 2026-11-15) into gifticlib's buffer of `1.01 n + 12` bytes, its failure messages | public domain (the compressor: `larmorx_core::zlib`, Zlib) |
| `src/xml/tok.rs` | expat 2.0.1 (`xmltok.c`, `xmltok_impl.c`, `xmltok_ns.c`, `asciitab.h`, `iasciitab.h`, `latin1tab.h`, `utf8tab.h`: the built-in encodings, byte types, name checks, UTF-8 conversions, every tokenizer, `getAtts`, `charRefNumber`, `updatePosition`) | MIT (expat's COPYING) |
| `src/xml/role.rs` | expat 2.0.1 (`xmlrole.c`, `xmlrole.h`: the prolog's state machine for the document entity) | MIT |
| `src/xml/mod.rs` | expat 2.0.1 (`xmlparse.c`: `XML_Parse`, `XML_ParseBuffer`, `XML_GetCurrentLineNumber`, `XML_ErrorString`, the prolog, content, CDATA and epilog processors, `initScan`, `processXmlDecl`, `doParseXmlDecl`, `storeAtts`, `storeAttributeValue`, `appendAttributeValue`, `storeEntityValue`, `defineAttribute`, the content-model scaffold, `reportDefault`, `normalizeLines`, `normalizePublicId`) for the configuration gifticlib uses | MIT |
| `src/xml/nametab.rs` | expat 2.0.1 (`nametab.h`, generated from it) | MIT |

- gifticlib 1.12 is the history of github.com/NIFTI-Imaging/gifti_clib at commit
  8ec2a95bce6360c3f7f655b36767e4d04c61d77c (2012-06-15, "gifti library version 1.12, 15 June,
  2012"; the only commit touching the library's files between 1.12 and 1.13's 7d94df8), pinned in
  `<workspace>/tags/gifticlib-1.12` (GitHub's archive of the commit, SHA-256
  965406c74a48a0bf1a58d6d556189da120de5e5fbb6209b145805325be9c0416). expat 2.0.1 is libexpat's
  tag `R_2_0_1`, pinned in `<workspace>/tags/expat-2.0.1` (GitHub's archive, SHA-256
  6a26c3fe91d2a05d05fac187c0a092e1602c335ee12b008d9fe4da30788402b9), configured as the C build
  this follows: `XML_DTD`, `XML_NS` (compiled, unused: no namespace processing),
  `XML_CONTEXT_BYTES` 1024.
- The parser covers what a parser made by `XML_ParserCreate(NULL)` reaches with gifticlib's
  handlers: no external entities are read, parameter entities are never expanded (parameter
  entity parsing is off), internal entities are expanded in attribute values only (references in
  content reach the default handler). An earlier parser written from expat's behaviour was
  replaced by this translation (the probe found 49 % of generated documents parsed differently;
  archived in `<workspace>/archive/2026-11-13-xml-adhoc-parser/`).
- D13: FS-80 to FS-88 (`gifti::defects`; FS-88 is expat's); `Dialect` holds the two points where
  a program built on gifticlib changes it.
- Checks: `src/gifti/tests.rs`, `src/xml/tests.rs`; the differential probe
  `validation/probes/fs-gifti` against gifticlib 1.12 with expat 2.0.1 and against FreeSurfer
  8.2.0's copies (its README has the numbers).

## DICOM files: CTN 3.0.6's `DCM_OpenFile` (`src/dicom/`; FreeSurfer's foundations, item 3.1-16, 2026-11-13)

| larmorx file | Upstream | Licence |
|---|---|---|
| `src/dicom/mod.rs` | CTN 3.0.6 (`dicom_objects.h`, `dcmprivate.h`: the value representations, options, tags, the object, group and element structures; `dcm.c`: `vrMap`, `DCM_CreateObject`'s fields) | CTN's notice (`LICENSES/LicenseRef-CTN.txt`) |
| `src/dicom/cond.rs` | CTN 3.0.6 (`dicom.h`'s `FORM_COND`; `dicom_objects.h`'s condition codes; `dcmcond.c`: the message table, `DCM_Message`; `condition.c`: `COND_PushCondition`, `COND_PopCondition`, `COND_TopCondition`, `COND_ExtractConditions`'s order, `COND_DumpConditions`, the stack's size and overflow) | CTN's notice |
| `src/dicom/dict.rs`, `src/dicom/dict_table.rs` | CTN 3.0.6 (`dcmdict.c`: `DCM_LookupElement`; the tables, generated by `validation/probes/fs-dicom/gen_dict.sh`) | CTN's notice |
| `src/dicom/read.rs` | CTN 3.0.6 (`dcm.c`: `DCM_OpenFile`, `DCM_CloseObject`, `checkObject`, `readFile1`, `readPreamble`, `readGroupElement`, `readVRLength`, `readSequence`, `scanCompressedPixels`, `readData`, `newElementItem`, `checkAttributeOrder`, `handleGroupItem`, `updateObjectType`, `updateSpecialElements`, `computeVM`, `ctxSensitiveLookup`, `swapInPlace`, `swapATGroupElement`, `fileSize`) for files on a little-endian machine | CTN's notice |
| `src/dicom/element.rs` (item 3.1-56, 2026-11-16) | CTN 3.0.6 (`dcm.c`: `DCM_GetElement`, `DCM_GetElementValue`, `locateElement`, `checkObject`; `dicom_objects.h`'s `DCM_ELEMENT`) | CTN's notice |

- CTN 3.0.6 is the DICOM Central Test Node's release as Debian packaged it
  (`ctn_3.0.6.orig.tar.gz`, snapshot.debian.org, SHA-256
  2e21b5dbe5d1f85ceaaf43b117d22316b48d3ffeb7e8d2e828530dadb8c30861), pinned in
  `<workspace>/tags/ctn-3.0.6`; of the releases available (3.0.6, 3.2.0) it is the closest to the
  copy the probe also compares with (99.98 % of `dcmdict.c`'s tokens, 95.8 % of `dcm.c`'s, the
  rest layout and casts), built as on Linux x86-64 (`LITTLE_ENDIAN_ARCHITECTURE`, no threads).
- What it covers: `DCM_OpenFile` on files with the byte orders, explicit or implicit VR, part 10,
  `DCM_NOGROUPLENGTH` and `DCM_ACCEPTVRMISMATCH`; `DCM_FORMATCONVERSION` without a byte order
  (CTN refuses it: "illegal stream option"). The options no caller reaches (`DCM_DELETEONCLOSE`,
  `DCM_USELENGTHTOEND`, `DCM_ALLOWREPEATELEMENTS` / `DCM_TRYFILENAMECHANGE`, `DCM_EFILM`, a format
  conversion with a byte order) return `read::Unsupported`; `open_file_named` opens one path and
  shows another name (the bytes the C caller passed) in the messages and the object, for programs
  run in process with their own working directory; streams (`DCM_ReadStream`) are not ported.
- Elements (item 3.1-56): `element::get_element` and `element::get_element_value`, the two calls
  FreeSurfer's DICOM reader makes for every element it uses (`DCM_GetElement` by tag, then
  `DCM_GetElementValue` whole or in pieces through its context): the value from memory, or pixel
  data read through the object's descriptor (`lseek`, then `read` with the count as an `int`), with
  CTN's conditions and messages (the routine name `DCM_GetElementVM`, FS-894) and the swaps of
  data in the reverse byte order; the value is returned as the bytes CTN writes at the start of the
  caller's buffer (`ValueRead`), so a caller sees what a too small C buffer would receive. The
  context is the offset it holds (CTN 3.0.6 casts it to 32 bits, the copy to a `long`: the same for
  every offset a value has). D13: FS-951 (a sequence asked for as another representation), FS-952
  (a failed read of 4 GiB - 1 taken for complete), FS-953 (an odd count of big-endian 16-bit
  pixels).
- CTN's global state is a value (`CondStack`): the condition stack, and `errno` and the
  descriptors as CTN's calls leave them (a read with a negative count, a descriptor closed twice,
  glibc's first `perror` on a write-only standard error; the open file behind each descriptor,
  which `get_element_value` reads through, so a descriptor a failed open closed reads nothing,
  EBADF, as in C), so `perror`'s texts are CTN's; errno
  texts are glibc's on every platform. The condition stack keeps the bytes, and its dumps
  (`CondStack::dump`, the overflow dump in `push`) print them unchanged through
  `Streams::err_bytes` on a byte stream (`larmorx_core::cfmt::Writers`); a text-only stream gets
  U+FFFD for bytes that are not UTF-8.
- The C arithmetic is kept: 32-bit lengths that wrap, `fileSize`'s `int`, the group lengths
  recomputed, the transfer syntax applied when the object's size reaches the meta group's end, a
  delimiter at any level ending the data set, the explicit VR code compared as a C string (the
  delimiter's code is CTN's one byte 0x1E).
- D13: FS-90 to FS-98 and FS-951 to FS-953 (`larmorx_core::defects`; FS-894 kept); `Dialect`
  holds the two points where a program built on CTN changes it (a message's text, FS-94's swap).
- Checks: `src/dicom/tests.rs` (the stack's rules, a part 10 file, the format probe's four opens
  of a NIfTI header, FS-91, FS-94, FS-96, the nesting limit; a digest of 300 generated files
  recorded on Linux x86-64, also under the AArch64 NaN trap, debug and release);
  `src/dicom/element_tests.rs` (every condition and message, pieces, contexts, big-endian and
  encapsulated pixel data, FS-91, FS-894, FS-951 to FS-953 in both modes, a closed descriptor; a
  digest of the requests on 240 generated files, `0x131978a09fff2d6e`, Linux x86-64 = Windows x64,
  under the NaN trap in debug and release); the differential probe `validation/probes/fs-dicom`
  (its open script and its element script) against CTN 3.0.6 built from its source and against the
  copy in the unit oracle (README there).

## NRRD: NrrdIO's reader (`src/nrrd/`; FreeSurfer's foundations, track C relay 26, 2026-11-14)

Ported from Teem's NrrdIO, SVN `NrrdIO/trunk` revision 3991 (2008-11-18, `TEEM_VERSION` 1.9.0;
`<workspace>/tags/NrrdIO-r3991`, its `MANIFEST.sha256`), Gordon Kindlmann and the University of
Utah, zlib-style terms (the notice in `NOTICE`); SPDX `Apache-2.0 AND Zlib` in each file. As
FreeSurfer 8.2.0's programs behave: the quiet NaN's high bit set, no zlib or bzip2 (the `gzip` and
`bzip2` encodings unavailable), no direct I/O.

| larmorx | NrrdIO r3991 |
|---|---|
| `nrrd::load`, `load_with`, `LoadError` | `read.c`: `nrrdLoad`, `nrrdRead`, `_nrrdRead`, `_nrrdSplitName`, `_nrrdOneLine`, `nrrdLineSkip`, `nrrdByteSkip`, `_nrrdCalloc` (see below); `miscAir.c`: `airFopen`; `string.c`: `airOneLine` |
| `Reader::format_read`, `header_check`, `data_file_next`, `sprintf_d` | `formatNRRD.c`: `_nrrdFormatNRRD_read`, `_nrrdFormatNRRD_contentStartsLike`, `_nrrdHeaderCheck`, `nrrdIoStateDataFileIterNext` |
| `Reader::parse_field`, `parse_info`, `space_vector`, `quoted`, `contains_percent_d`, `parse_data_file`, `data_fn_check` | `parseNrrd.c`: `_nrrdReadNrrdParseField`, every `_nrrdReadNrrdParse_*`, `_nrrdSpaceVectorParse`, `_nrrdGetQuotedString`, `_nrrdContainsPercentDAndMore`, `_nrrdDataFNNumber`, `_nrrdDataFNCheck` |
| `Reader::field_check`, `check_space_info`, `check`; `Nrrd::element_size`, `element_number`; `size_check`, `space_dimension`, `kind_size` | `simple.c`: `_nrrdFieldCheck*`, `_nrrdFieldCheckSpaceInfo`, `_nrrdCheck`, `nrrdElementSize`, `nrrdElementNumber`, `nrrdSpaceDimension`, `nrrdSpaceSet`; `axis.c`: `_nrrdSizeCheck`, `nrrdKindSize`, `_nrrdAxisInfoInit`, `nrrdAxisInfoSet_nva` |
| `Reader::read_data`, `read_ascii`, `read_hex`, `fscanf_s`, `HEX_TABLE`; the swap in `format_read` | `encodingRaw.c`, `encodingAscii.c`, `encodingHex.c` (`_nrrdReadHexTable`), `encodingGzip.c`/`encodingBzip2.c` (their unavailable branches), `encoding.c`; `endianNrrd.c`: `nrrdSwapEndian`, `_nrrdBlockEndian` |
| `Biff` | `biffbiff.c`: `biffAdd`, `_biffAddErr`, `_biffAddKey`, `biffGet`, `biffDone`, `biffGetDone`, `biffCheck` |
| `AirEnum`, the enum tables, `type_name`, `space_name` | `enum.c`: `airEnumVal`, `airEnumStr`, `airEnumValCheck`, `_airEnumIndex`; `enumsNrrd.c` (type, encoding, centering, kind, field, space), `endianAir.c` (`airEndian`) |
| `Strtok`, `strntok`, `unescape`, `to_lower`, `is_print`, `Reader::one_linify`, `parse_str_d`, `parse_str_z`, `single_sscanf_*` | `string.c`: `airStrtok`, `airStrntok`, `airUnescape`, `airToLower`, `airOneLinify`; `parseAir.c`: `airParseStrD`, `airParseStrZ`, `airSingleSscanf` |
| `Nrrd::key_value_add`; the comment rules in `parse_info` | `keyvalue.c`: `nrrdKeyValueAdd`; `comment.c`: `nrrdCommentAdd` |
| `air_nan` | `754.c`: `airFloatQNaN` (through `larmorx_core::nrrdio`), as a `double` |

- C's stdio is emulated (`CFile`: `getc`, one `ungetc`, `fread`, `fseek(SEEK_END)`; a directory
  opens and reads as empty, as on Linux; errno texts are glibc's, through `dicom::cond`), and the
  line buffer byte for byte (`CBuf`: what `airOneLine` writes, never-written bytes known).
- NrrdIO clears its whole array before reading; the port grows it as it reads, bounded by what
  the data files hold (the values are the same; `docs/findings/nrrdio.md`, "Memory").
- D13: FS-518 to FS-520 and FS-761 to FS-773 (`larmorx_core::defects`); `docs/findings/nrrdio.md`.
- Checks: `src/nrrd/tests.rs`; the differential probe `validation/probes/fs-nrrd-minc` against
  NrrdIO r3991 built from its source and against FreeSurfer's copy (README there).

## MINC 1 volumes: volume_io's input over MINC's library and netCDF (`src/minc/`; FreeSurfer's foundations, track C relay 27, 2026-11-15)

**Ported** from two release tarballs:

- MINC 1.5.1 (`minc-1.5.1.tar.gz`, SHA-256 `0b41d5bb1b178c62792c9e4fbd8ddd971d1a90aad7c257ff00bcbdddc3998afe`,
  `<workspace>/tags/minc-1.5.1`; © 1993-1995 David MacDonald and 1993-2000 Peter Neelin, McConnell Brain
  Imaging Centre, Montreal Neurological Institute, McGill University; MNI's permissive notice,
  `LICENSES/LicenseRef-MNI.txt`);
- netCDF 3.6.0-p1 (`netcdf-3.6.0-p1.tar.gz`, SHA-256
  `a9e6105ed89ce6cfd2edc78e8184232a2f950e674c5241c28cae361a1fe1adfe`, from a mirror of Unidata's archive,
  `https://www.gfd-dennou.org/arch/netcdf/unidata-mirror/old/`, `<workspace>/tags/netcdf-3.6.0-p1`;
  © 1993-2004 University Corporation for Atmospheric Research/Unidata, `LICENSES/NetCDF.txt`).

| larmorx | the original |
|---|---|
| `minc/netcdf.rs`: `Px`, `Stream`, `read_header`, `var_shape`, `get_vara`, `iocount`, `get_run`, `var_offset`, `coord_check`, `edge_check`, `strerror`, `advise`, `ncopen`, `ncclose`, `ncvarid`, `ncvarinq`, `ncdiminq`, `ncattinq`, `ncattget`, `ncvarget`, `nctypelen` | netCDF 3.6.0-p1 `src/libsrc/`: `posixio.c` (`blksize`, `px_pgin`, `px_get`, `ncio_px_sync`, `ncio_open`), `v1hpg.c` (`v1hs`, `fault_v1hs`, `check_v1hs`, the `v1h_get_*` functions, `NC_computeshapes`, `nc_get_NC`), `nc.c` (`nc__open_mp`, `NC_check_id`, `nc_close`), `var.c` (`NC_var_shape`, `NC_findvar`, `nc_inq_varid`, `nc_inq_var`), `dim.c` (`nc_inq_dim`), `attr.c` (`ncx_len_NC_attrV`, `NC_findattr`, `NC_lookupattr`, `nc_inq_att`, `nc_get_att`), `putget.c` (`NCcoordck`, `NCedgeck`, `NC_varoffset`, `getNCvx_<type>_<type>`, `NCiocount`, `odo1`, `nc_get_vara`), `ncx.c` (the big-endian getters), `error.c` (`nc_strerror`), `v2i.c` (`nc_advise`, `ncopen`, `ncclose`, `ncvarid`, `ncvarinq`, `ncdiminq`, `ncattinq`, `ncattget`, `ncvarget`, `nctypelen`) |
| `minc/libsrc.rs` | MINC 1.5.1 `libsrc/`: `minc_error.c` (`MI_save_routine_name`, `MI_return`, `MI_return_error`, `MI_log_pkg_error2`, `MI_log_sys_error1`), `netcdf_convenience.c` (`miexpand_file`, `miopen`, `miclose`, `miattget`, `miattget_with_sign`, `miattget1`, `miattgetstr`, `mivarget`, `mivarget1`, `mitranslate_coords`), `minc_convenience.c` (`miget_datatype`, `miget_default_range`, `miget_valid_range`, `miget_image_range`), `value_conversion.c` (`MI_varaccess`, `MI_var_loop`, `MI_get_sign_from_string`, `MI_get_sign`, `MI_convert_type`), `minc_basic.h` (`MI_TO_DOUBLE`, `MI_FROM_DOUBLE`), `minc_useful.h` (`ROUND`, `MAX`, `MIN`) |
| `minc/icv.rs` | MINC 1.5.1 `libsrc/image_conversion.c` (`miicv_create`, `miicv_free`, `miicv_setdbl`, `miicv_setint`, `miicv_setstr`, `miicv_inqdbl`, `miicv_ndattach`, `MI_icv_get_type`, `MI_icv_get_vrange`, `MI_icv_get_norm`, `miicv_detach`, `miicv_get`, `MI_icv_access`, `MI_icv_calc_scale`, `MI_icv_chkid`), `dim_conversion.c` (`miicv_attach`, `MI_icv_get_dim`, `MI_get_dim_flip`, `MI_get_dim_scale`) |
| `minc/volume.rs` | MINC 1.5.1 `volume_io/Volumes/volumes.c` (`get_default_dim_names`, `convert_dim_name_to_spatial_axis`, `create_volume`, `set_volume_type`, `alloc_volume_data`, `set_volume_voxel_range`, `get_volume_real_range`, `set_volume_real_range`, `set_volume_separations`, `set_volume_starts`, `set_volume_direction_unit_cosine`, `set_volume_space_type`, `get_volume_total_n_voxels`, `compute_world_transform`, `check_recompute_world_transform`, `convert_voxel_to_world`), `multidim_arrays.c` (`get_type_size`, `alloc_multidim_array`), `evaluate.c` (`get_volume_voxel_value`) |
| `minc/input.rs` | MINC 1.5.1 `volume_io/Volumes/input_volume.c` (`start_volume_input`, `input_more_of_volume`, `delete_volume_input`), `input_mnc.c` (`get_minc_file_n_dimensions`, `initialize_minc_input`, `initialize_minc_input_from_minc_id`, `match_dimension_names`, `input_more_minc_file`, `input_slab`, `input_minc_hyperslab`, `close_minc_input`, `set_default_minc_input_options`), `volume_cache.c` (`get_n_bytes_cache_threshold`, `open_cache_volume_input_file`), `Prog_utils/files.c` (`filename_extension_matches`) |

The voxel-to-world matrix is made with `larmorx_io::xfm`'s `create_linear_transform` (a singular matrix
gives the identity) through the dialect's `.xfm` dialect. Decompression is in memory (flate2, the
`bzip2` crate) where MINC runs `gunzip` or `bunzip2` through the shell (FS-774). A program's own copy of
MINC plugs in through `minc::Dialect` (FreeSurfer's, measured as a black box by
`validation/probes/fs-nrrd-minc`, is described in a file closed to FreeSurfer's clean-room implementers,
`docs/findings/freesurfer-permissive.md`). Defects FS-774 to FS-796 (`larmorx_core::defects`;
`docs/findings/minc-volumes.md`).

## MINC 1 volumes written: volume_io's output over MINC's library and netCDF's classic writer (`src/minc/`; FreeSurfer's foundations, track C relay 28, 2026-11-15)

**Ported** from the same two release tarballs as the volume input (above: MINC 1.5.1, netCDF 3.6.0-p1).

| larmorx | the original |
|---|---|
| `minc/ncwrite.rs`: `nc_create`, `WStream`, `put_header_items`, `header_len`, `write_header`, `write_numrecs`, `sync`, `fill_var`, `check_vlen`, `check_vlens`, `begins`, `endef`, `def_dim`, `def_var`, `put_att`, `vnrecs`, `put_run`, `put_vara`, `put_var1`, `close_written`, `check_name`, `nccreate`, `ncdimdef`, `ncdimid`, `ncvardef`, `ncattput`, `ncendef`, `ncvarput`, `ncvarput1`, `ncclose_written` | netCDF 3.6.0-p1 `src/libsrc/`: `nc.c` (`new_NC`, `nc__create_mp`, `nc_create`, `NC_begins`, `NC_check_vlens`, `NC_endef`, `nc_enddef`, `write_NC`, `write_numrecs`, `NC_sync`, `fillerup`, `nc_close`, `nc_abort`), `posixio.c` (`fgrow`, `px_pgout`, `px_get`'s write paths, `px_rel`, `ncio_px_rel`, `ncio_px_sync`, `ncio_px_init2`, `ncio_create`, `ncio_close`), `v1hpg.c` (`rel_v1hs`, `fault_v1hs`, `check_v1hs`, the `v1h_put_*` functions, `ncx_len_NC`, `ncx_put_NC`), `dim.c` (`nc_def_dim`, `nc_inq_dimid`), `var.c` (`nc_def_var`, `NC_check_vlen`), `attr.c` (`ncx_len_NC_attrV`, `nc_put_att_<type>`, `nc_put_att`), `string.c` (`NC_check_name`), `putget.c` (`NC_fill_<type>`, `fill_NC_var`, `NCfillrecord`, `NCvnrecs`, `putNCvx_<type>_<type>`, `nc_put_vara_<type>`, `nc_put_var1_<type>`, `nc_put_vara`, `nc_put_var1`), `ncx.c` (the big-endian putters, `ncx_put_size_t`, `ncx_put_off_t`, `ncx_pad_putn_text`), `v2i.c` (`nccreate`, `ncdimdef`, `ncdimid`, `ncvardef`, `ncattput`, `ncendef`, `ncvarput`, `ncvarput1`, `ncclose`) |
| `minc/netcdf.rs` (additions): `Px::new_file`, `Px::pgout`, `Px::get_rgn`, `Px::rel`, `Px::put`, `Px::sync`, `Px::write_raw`, `Source::write_at` | netCDF 3.6.0-p1 `posixio.c` (`px_get`'s page-out branches, `px_pgout`, `ncio_px_init2` of a new file, `ncio_px_rel`, `ncio_px_sync`) |
| `minc/mwrite.rs` | MINC 1.5.1 `libsrc/`: `netcdf_convenience.c` (`micreate`, `miattputint`, `miattputdbl`, `miattputstr`, `mivarput`, `mivarput1`), `minc_convenience.c` (`miset_valid_range`, `miattput_pointer`, `miadd_child`, `micreate_std_variable`, `MI_create_dim_variable`, `MI_create_dimwidth_variable`, `MI_create_image_variable`, `MI_create_imaxmin_variable`, `MI_verify_maxmin_dims`, `MI_create_root_variable`, `MI_create_simple_variable`, `MI_add_stdgroup`), `value_conversion.c` (`MI_varaccess`, `MI_var_action` and `MI_convert_type` for a put), `minc_varlists.h` (`dimvarlist`, `dimwidthlist`, `varlist`), `minc.h` (the standard names and values) |
| `minc/icv.rs` (additions): `miicv_put`, `calc_scale`'s put | MINC 1.5.1 `libsrc/image_conversion.c` (`miicv_put`, `MI_icv_access` and `MI_icv_calc_scale` for a put, `MI_icv_coords_tovar` without a dimension conversion) |
| `minc/output.rs` | MINC 1.5.1 `volume_io/Volumes/output_volume.c` (`create_output_dim_names`, `copy_volume_auxiliary_and_history` without an original file, `output_modified_volume` without one, `output_volume`), `output_mnc.c` (`is_default_direction_cosine`, `output_world_transform`, `create_image_variable`, `end_file_def`, `initialize_minc_output`, `add_minc_history`, `get_dimension_ordering`, `check_minc_output_variables`, `output_minc_hyperslab`, `output_slab`, `output_the_volume`, `output_minc_volume`, `close_minc_output`, `set_default_minc_output_options`, `set_minc_output_dimensions_order`, `set_minc_output_real_range`, `set_minc_output_use_volume_starts_and_steps_flag`), `volumes.c` (`convert_transform_origin_to_starts`, `convert_transform_to_starts_and_steps`, `get_volume_dimension_names`), `multidim_arrays.c` (`copy_multidim_data_reordered`, as a gather in the file's order) |
| `minc/volume.rs` (additions): `create_volume`, `alloc_volume_data`, `set_direction_cosine`, `set_translation`, `set_voxel_value`, `dimension_names_canonical` | MINC 1.5.1 `volume_io/Volumes/volumes.c` (`create_volume`, `alloc_volume_data`, `set_volume_direction_cosine`, `set_volume_translation`, `get_volume_dimension_names`), `evaluate.c` (`set_volume_voxel_value`, `SET_VOXEL`'s C casts as GCC 8.4.0 compiles them for x86-64) |
| `xfm/mod.rs` (`transform_point`), `xfm/gaussian.rs` (`solve_linear_system`) | MINC 1.5.1 `volume_io/Geometry/transforms.c` (`transform_point`), `gaussian.c` (`solve_linear_system`) |

The arithmetic whose NaNs reach the file (`set_volume_direction_cosine`, `set_volume_translation`'s three-axis
path, `transform_point`) takes the operand order of volume_io's x86-64 object code (GCC 8.4.0 at `-O3`; read from
the probe's `libminc_up.so`) through `larmorx_core::ItkReal`'s `sse_*` operations, so the NaN bits are x86-64's on
every platform. Defects FS-797 to FS-801 (`larmorx_core::defects`; `docs/findings/minc-volumes.md`, FreeSurfer's
copy's in `docs/findings/freesurfer-permissive.md`).

## JPEG: IJG libjpeg 6b's decompressor and grey compressor (`src/jpeg/`; FreeSurfer's foundations, track C relay 31, 2026-11-15)

Translated from the Independent JPEG Group's JPEG software, release 6b of 27-Mar-1998
(`jpegsrc.v6b.tar.gz`, SHA-256 75c3ec241e9996504fe02a9ed4d12f16b74ade713972f3db9e65ce95cd27e35d;
`<workspace>/tags/jpeg-6b`, `upstream.tsv`); copyright (C) 1991-1998, Thomas G. Lane; IJG's
conditions (its README, `LICENSES/IJG.txt`; the acknowledgement in `NOTICE`); SPDX `Apache-2.0 AND
IJG` in each translated file (`cases.rs`, the test encoder, is larmorx's own). The configuration:
IJG's defaults (`jmorecfg.h`: 8-bit samples, `INT32` a 64-bit `long` on LP64, no arithmetic
coding, multi-scan and progressive decoding, block smoothing, `jmemnobs.c`'s memory), as FreeSurfer
8.2.0 builds its `packages/jpeg` (IJG 6b file for file but one `#include` in `jerror.c`).

| larmorx | IJG 6b |
|---|---|
| `msg::TABLE`, `msg::format`, `ErrMgr`, `Fatal` | `jerror.h` (the message table, generated from it), `jerror.c`: `format_message`, `emit_message`, `output_message`, `error_exit`'s message, `reset_error_mgr` |
| `D::fill`, `byte`, `two_bytes`, `skip` (`Src`) | `jdatasrc.c`: `init_source`, `fill_input_buffer` (the fake EOI), `skip_input_data`; `jdmarker.c`'s `INPUT_BYTE`, `INPUT_2BYTES` |
| `D::get_soi`, `get_sof`, `get_sos`, `get_dht`, `get_dqt`, `get_dri`, `get_interesting_appn`, `examine_app0`, `examine_app14`, `skip_variable`, `next_marker`, `first_marker`, `read_markers`, `read_restart_marker`, `resync_to_restart` | `jdmarker.c` (its trace messages not kept: trace level 0) |
| `D::initial_setup`, `per_scan_setup`, `latch_quant_tables`, `start_input_pass`, `consume_markers`, `consume_input` | `jdinput.c` |
| `D::default_decompress_parms`, `read_header`; `decompress`, `decode_rgb`, `read_header` | `jdapimin.c`: `default_decompress_parms`, `jpeg_read_header`, `jpeg_consume_input`, `jpeg_finish_decompress`; `jdapistd.c`: `jpeg_start_decompress`, `jpeg_read_scanlines`; `jdmaster.c`: `master_selection`'s order of checks, `use_merged_upsample` (never chosen: fancy upsampling), `jpeg_calc_output_dimensions` (scale 1/1), `prepare_range_limit_table` (`range_limit`) |
| `D::make_derived`, `start_pass_huff`, `fill_bit_buffer`, `huff_decode`, `huff_decode_slow`, `get_bits`, `process_restart`, `decode_mcu`, `decode_block`, `huff_extend` | `jdhuff.c`, `jdhuff.h` (`HUFF_DECODE`, `CHECK_BIT_BUFFER`, `GET_BITS`) |
| `D::start_pass_phuff`, `decode_mcu_prog`, `dc_first`, `ac_first`, `dc_refine`, `ac_refine` | `jdphuff.c` |
| `D::start_imcu_row`, `consume_data`; `decompress_onepass`, `idct_row`, `smoothing_ok`, `idct_smooth_row` | `jdcoefct.c` |
| `build_dct_tables`, `idct_islow` | `jddctmgr.c` (`start_pass`, the slow integer method only), `jidctint.c` |
| `Up`, `h2v1_fancy`, `h2v2_fancy`, `Output::emit` | `jdsample.c` (`jinit_upsampler`'s choice, every method); `jdmainct.c`'s context rows as their effect (the row above the first and below the last real row repeat it); `jdpostct.c` (pass-through) |
| `YccTables`, `Convert` | `jdcolor.c`: `build_ycc_rgb_table`, `ycc_rgb_convert`, `gray_rgb_convert`, `null_convert`, `jinit_color_deconverter`'s checks (for `JCS_RGB` output) |
| `NATURAL_ORDER` | `jutils.c`: `jpeg_natural_order` with its 16 extra entries |
| `encode_gray`, `encode_one_block`, `emit_code`, `emit_bits`, `fdct_islow`, `c_derived`, `Dest` | `jcparam.c` (`jpeg_set_defaults`, `jpeg_set_quality`, `jpeg_quality_scaling`, `jpeg_add_quant_table`, the standard tables), `jcmaster.c` (`initial_setup`'s checks), `jcprepct.c` (`expand_bottom_edge`), `jcsample.c` (`fullsize_downsample`, `expand_right_edge`), `jccolor.c` (`grayscale_convert`), `jccoefct.c`, `jcdctmgr.c` (`forward_DCT`, the slow integer divisors), `jfdctint.c`, `jchuff.c` (`jpeg_make_c_derived_tbl`, `emit_bits`, `flush_bits`, `encode_one_block`), `jcmarker.c` (the markers of a baseline grey file), `jdatadst.c` (4,096-byte blocks, `term_destination`) |

- Not translated (no caller here sets them): IDCT scaling, colour quantization, the fast and float
  DCTs, raw data, buffered-image mode, suspending sources, other compressor parameters.
- The sample rows of three iMCU rows are kept per component; the coefficients of the whole image
  only for files of several scans, as IJG keeps them (`Options::max_alloc` models a failing
  `malloc`: `Insufficient memory (case 4)`).
- D13: FS-813 (`larmorx_core::defects`); `docs/findings/jpeg-6b.md`.
- Checks: `src/jpeg/tests.rs` (a digest of 400 generated files decoded in both modes and 40 grey
  files written); the differential probe `validation/probes/fs-jpeg` against IJG 6b built from its
  release and FreeSurfer 8.2.0's `libjpeg.a` (README there).

## TIFF: libtiff 3.6.1's reader and writer (`src/tiff/`; FreeSurfer's foundations, track C relay 32, 2026-11-16)

Translated from libtiff 3.6.1 (`tiff-v3.6.1.tar.gz`, SHA-256
538c6302bb2426a69d25e45dcb5e5b11d7235cf497368651b2d6c92612dbf83f; `<workspace>/tags/libtiff-3.6.1`,
`upstream.tsv`); Copyright (c) 1988-1997 Sam Leffler, Copyright (c) 1991-1997 Silicon Graphics,
Inc.; libtiff's notice (`LICENSES/libtiff.txt`, its acknowledgement in `NOTICE`); SPDX
`Apache-2.0 AND libtiff` in each translated file (`cases.rs`, `codecgen.rs` and `tests.rs` are larmorx's own). The
configuration is FreeSurfer 8.2.0's build of its `packages/tiff` (libtiff 3.6.1 but two `$Id`
lines and a cast in `tif_print.c`, which is not translated): `ZIP_SUPPORT`, `PIXARLOG_SUPPORT`,
`tiffconf.h`'s codecs, `STRIPCHOP_DEFAULT`, `SUBIFD_SUPPORT`, `port.h` without `HAVE_MMAP`,
`HOST_FILLORDER` MSB2LSB, `HOST_BIGENDIAN` 0, `NDEBUG` (its asserts compiled out). Deflate over
`larmorx_core::zlib` (zlib 1.2.11 as Ubuntu 22.04 builds it, the library FreeSurfer's official
programs load).

| larmorx | libtiff 3.6.1 |
|---|---|
| `mod.rs`: `Tiff::open`, `client_open`, `init_order`, `get_mode`, `number_of_directories`, `advance_directory`, `set_directory`, `close`, `header_bytes` | `tif_open.c` (`TIFFClientOpen`, `TIFFInitOrder`, `_TIFFgetMode`, the queries), `tif_unix.c` (`TIFFOpen`), `tif_dir.c` (`TIFFNumberOfDirectories`, `TIFFAdvanceDirectory` unmapped, `TIFFSetDirectory`), `tif_close.c` |
| `io.rs`: `Fd` | `tif_unix.c`'s `_tiffReadProc`, `_tiffWriteProc`, `_tiffSeekProc`, `_tiffSizeProc` (with Linux's `read`, `lseek`, `fstat` and `open` results on every platform) |
| `fields.rs`: `MAIN`, `PREDICT`, `ZIP`, `FAX`, `FAX3`, `FAX4`, `PIXARLOG`, `LOGLUV`, `Fields::merge`, `merge_anon`, `setup`, `find`, `data_width`, the tags | `tif_dirinfo.c` (`tiffFieldInfo`, `_TIFFSetupFieldInfo`, `tagCompare`, `_TIFFMergeFieldInfo`, `_TIFFFindFieldInfo` with its cache, `_TIFFCreateAnonFieldInfo`, `TIFFDataWidth`), `tif_predict.c`, `tif_zip.c`, `tif_fax3.c`, `tif_pixarlog.c`, `tif_luv.c` (their field tables), `tiff.h` |
| `dir.rs`: `Dir`, `default_directory`, `free_directory`, `set_field`, `set_field_arg`, `vsetfield`, `check_ink_names`, `get_field`, `vgetfield`, `get_field_defaulted` | `tif_dir.h`, `tif_dir.c` (`TIFFDefaultDirectory`, `TIFFFreeDirectory`, `TIFFVSetField` with `OkToChangeTag`, `_TIFFVSetField`, `setExtraSamples`, `checkInkNamesString`, `TIFFVGetField`, `_TIFFVGetField`), `tif_aux.c` (`TIFFVGetFieldDefaulted`); the codecs' `vsetfield`/`vgetfield` hooks (`PredictorVSetField`, `ZIPVSetField` and their get side) |
| `dirread.rs`: `read_directory`, `check_malloc`, `extract_data`, `check_dir_count`, `fetch_data`, `fetch_string`, `cvt_rational`, `fetch_rational`, `fetch_byte_array`, `fetch_short_array`, `fetch_long_array`, `fetch_rational_array`, `fetch_float_array`, `fetch_double_array`, `fetch_any_array`, `fetch_normal_tag`, `fetch_per_sample_shorts`, `fetch_per_sample_anys`, `fetch_strip_thing`, `fetch_extra_samples`, `fetch_colormap`, `fetch_short_pair`, `fetch_ref_black_white`, `estimate_strip_byte_counts`, `missing_required`, `chop_up_single_uncompressed_strip` | `tif_dirread.c` (every function of the unmapped path), `tif_dir.h`'s `TIFFExtractData` |
| `read.rs`: `scanline_size`, `number_of_strips`, `number_of_tiles`, `vstrip_size`, `tile_row_size`, `vtile_size`, `tile_size`, `compute_tile`, `check_tile`, `check_read`, `read_scanline`, `seek_row`, `read_raw1`, `read_buffer_setup`, `fill_strip`, `start_strip`, `start_tile`, `read_tile`, `read_encoded_tile` | `tif_strip.c`, `tif_tile.c`, `tif_read.c` (unmapped), `tif_swab.c`'s `TIFFReverseBits` |
| `codec.rs`: the method enums, `set_default_compression_state`, `set_compression_scheme`, `predictor_init`, `cleanup`, `setup_decode`, `predictor_setup`, `predictor_setup_decode`, `pre_decode`, `decode`, `no_decode`, `codec_seek`, `post_decode`, `dump_decode`, `packbits_decode`, `lzw_pre_decode`, `lzw_decode_with`, `lzw_next_code`, `code_loop`, `zip_decode`, `zip_set_quality`, `pfunc` | `tif_compress.c`, `tif_codec.c` (`_TIFFBuiltinCODECS`, `NotConfigured`), `tif_dumpmode.c`, `tif_packbits.c`, `tif_lzw.c` (`LZWSetupDecode`, `LZWPreDecode`, `LZWDecode`, `LZWDecodeCompat`, `LZWCleanup`, `LZWSetupEncode`), `tif_zip.c`, `tif_predict.c` (`horAcc8`, `horAcc16`, `swabHorAcc16`, `horDiff8`, `horDiff16`, the row and tile wrappers), `tif_read.c`'s `_TIFFSwab*BitData` |
| `fax3.rs`: `tables` (`fill_table` and the code lists), `Fax`, `fax_init`, `fax_vsetfield`, `fax_vgetfield`, `fax_setup`, `fax_pre_decode`, `fax_decode` and the decoder's `Dec` (`sync_eol`, `expand1d`, `expand2d`, `check_b1`, `cleanup_runs`, `setval`, `fill`, `need8`, `need16`) (2026-11-16, relay 33) | `mkg3states.c` (its code lists and `FillTable`: the tables equal FreeSurfer's `tif_fax3sm.c`, `validation/probes/fs-tiff/fax3sm_hash.py`), `tif_fax3.h` (the decoder's macros), `tif_fax3.c` (`InitCCITTFax3`, `TIFFInitCCITTFax3`, `TIFFInitCCITTFax4`, `TIFFInitCCITTRLE`, `TIFFInitCCITTRLEW`, `Fax3VSetField`, `Fax3VGetField`, `Fax3SetupState`, `Fax3PreDecode`, `Fax3Decode1D`, `Fax3Decode2D`, `Fax4Decode`, `Fax3DecodeRLE`, `_TIFFFax3fillruns`, the messages), `tif_swab.c`'s `TIFFGetBitRevTable` |
| `next.rs`: `next_decode` (`Out`, the decoders' output) | `tif_next.c` (3.6.1's `NeXTDecode`; by default libtiff 4.0.10's checks, FS-836) |
| `thunder.rs`: `thunder_decode_row` | `tif_thunder.c` (`ThunderDecodeRow`, `ThunderDecode`) |
| `pixarlog.rs`: `tables`, `PixarLog`, `pixarlog_init`, `pixarlog_vsetfield`, `pixarlog_vgetfield`, `pixarlog_setup`, `pixarlog_pre_decode`, `pixarlog_decode`, `accumulate` | `tif_pixarlog.c` (`PixarLogMakeTables`'s decoding tables, `PixarLogGuessDataFmt`, `TIFFInitPixarLog`, `PixarLogVSetField`, `PixarLogVGetField`, `PixarLogSetupDecode`, `PixarLogPreDecode`, `PixarLogDecode`, `horizontalAccumulateF`, `...12`, `...16`, `...11`, `...8`) |
| `luv.rs`: `LogLuv`, `logluv_init`, `logluv_vsetfield`, `logluv_vgetfield`, `logluv_setup`, `logluv_decode`, `logluv_decode_rows`, `logluv_row`, the conversions | `tif_luv.c` (`TIFFInitSGILog`, `LogLuvVSetField`, `LogLuvVGetField`, `LogLuvSetupDecode`, `LogL16InitState`, `LogLuvInitState`, the two guesses, `LogL16Decode`, `LogLuvDecode24`, `LogLuvDecode32`, `LogLuvDecodeStrip`, `LogLuvDecodeTile`, `LogL16toY`, `LogL10toY`, `uv_decode`, `LogLuv24toXYZ`, `LogLuv32toXYZ`, `XYZtoRGB24`, `L16toY`, `L16toGry`, `Luv24to*`, `Luv32to*`), `uvcode.h` (`uv_row`); Copyright (c) 1997 Greg Ward Larson and Silicon Graphics, Inc. (its notice in `LICENSES/libtiff.txt`) |
| `write.rs`: `write_check`, `setup_strips`, `write_buffer_setup`, `write_scanline`, `grow_strips`, `append_to_strip`, `flush_data1`, `flush_data`, `flush`, `setup_encode`, `pre_encode`, `post_encode`, `encode`, `no_encode`, `dump_encode`, `packbits_encode`, `zip_encode`, `zip_post_encode`, `write_directory`, `setup_short_long`, `setup_short`, `write_data`, `write_short_array`, `write_long_array`, `write_rational_array`, `write_any_array`, `write_float_array`, `write_byte_array`, `write_normal_tag`, `link_directory` | `tif_write.c`, `tif_flush.c`, `tif_dirwrite.c` (`_TIFFWriteDirectory` with `done`, its writers of the fields the port keeps, `TIFFLinkDirectory`), `tif_dumpmode.c`, `tif_packbits.c`, `tif_zip.c` and `tif_predict.c`'s encoders |

Not translated: memory mapping (the build has none), `TIFFClientOpen`'s client procedures, tag
extenders and `TIFFReassignTagToIgnore` (empty), `TIFFPrintDirectory` (`tif_print.c`), the RGBA
interface (`tif_getimage.c`, `tif_color.c`), `TIFFWriteEncodedStrip`/`Tile`, `TIFFRewriteDirectory`,
`TIFFUnlinkDirectory`; the encoders of the CCITT, PixarLog and SGILog codecs (no caller writes
them). Validated by `validation/probes/fs-tiff` (`docs/findings/libtiff-3.6.1.md`).
