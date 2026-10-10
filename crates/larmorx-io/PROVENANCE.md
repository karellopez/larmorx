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
- `Modules/IO/TransformInsightLegacy/src/itkTxtTransformIO.cxx` (text format);
- `Modules/IO/TransformMatlab/src/itkMatlabTransformIO.cxx` (MATLAB v4 format);
- the displacement-field reading path of ANTs v2.6.5's `itk::ants::ReadTransform`
  (`Utilities/itkantsReadWriteTransform.h`).

HDF5 (`.h5`) files follow ITK's `itkHDF5TransformIO` layout and are read and written by the
Python layer with h5py (`python/larmorx/transforms.py`).
