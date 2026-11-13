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
`znz.rs`'s SPDX `Apache-2.0 AND Zlib`; altered: translated into Rust, inflate and deflate from
zlib-rs through flate2):

| larmorx | Upstream |
|---|---|
| `niftilib::find_file_extension`, `is_gzfile`, `makebasename`, `validfilename`, `is_complete_filename`, `fileexists`, `findhdrname`, `is_nifti_file`, `NiftiOptions` | `niftilib/nifti1_io.c` (`nifti_find_file_extension`, `nifti_is_gzfile`, `nifti_makebasename`, `nifti_validfilename`, `nifti_is_complete_filename`, `nifti_fileexists`, `nifti_findhdrname`, `is_nifti_file`, `compare_strlist`, `is_mixedcase`, `is_uppercase`, `make_uppercase`, `fileext_compare`, `fileext_n_compare`, `g_opts`), `nifti1.h` (`NIFTI_VERSION`, `NIFTI_ONEFILE`) |
| `znz::ZnzFile` | `znzlib/znzlib.c` (`znzopen`, `Xznzclose`, `znzread`, `znzwrite`, `znzseek`, `znzrewind`, `znztell`, `znzputs`, `znzgets`, `znzflush`, `znzeof`, `znzputc`, `znzgetc`) |
| `znz::GzFile` | zlib 1.2.11 `gzlib.c` (`gz_reset`, `gz_open`, `gzrewind`, `gzseek64`, `gztell64`, `gzeof`, `gz_error`), `gzread.c` (`gz_load`, `gz_avail`, `gz_look`, `gz_decomp`, `gz_fetch`, `gz_skip`, `gz_read`, `gzread`, `gzgetc`), `gzwrite.c` (`gz_init`, `gz_comp`, `gz_zero`, `gz_write`, `gzwrite`, `gzputc`, `gzflush`), `gzclose.c` and `gzclose_r`/`gzclose_w` |
| `znz::StdioFile` | C's `fopen`, `fread`, `fwrite`, `fseek`, `ftell`, `feof`, `fgetc` by their description (ISO C 7.21) and glibc's behaviour (no glibc code) |

The decompressed bytes are zlib's; the compressed bytes a writer makes are zlib-rs's (a deflate
with zlib's own bytes is not ported). Defects FS-76 and FS-77 (nifticlib; FS-77 with its
`reproduce`) and FS-78 (zlib 1.2.11), `larmorx_core::defects`. `NiftiOptions` holds nifticlib's
settings a program can change (`allow_upper_fext` and the compressed extensions); the variants of
other programs' copies are not in this crate (`docs/findings/freesurfer-permissive.md`, closed to
FreeSurfer's clean-room implementers).
