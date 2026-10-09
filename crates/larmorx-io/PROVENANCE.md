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
