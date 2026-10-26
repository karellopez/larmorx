# Provenance: larmorx-nc-core

**Replica: translated from FSL's source.** This crate is a step-by-step Rust translation of the
FSL libraries FSL's tools share, from FSL 6.0.7.17 (the conda packages fsl-newimage 2203.12,
fsl-miscmaths 2203.2, fsl-newnifti 4.1.0, fsl-znzlib 2111.0, fsl-meshclass 2111.0, fsl-utils
2203.5, fsl-avwutils 2209.3, fsl-armawrap 0.6.0), with the parts of the C and C++ libraries FSL's
Linux binaries rely on, written to give FSL's results bit for bit. It is distributed under the
FSL Licence (`../LICENSE`), as part of the `larmorx-nc` package (`docs/licensing.md`). The FSL
files it was translated from are in `../fsl-source/`, unchanged (`MANIFEST.tsv`). No Apache-2.0
crate uses it, and it uses no GPL crate.

It is the shared core of the FSL replicas: each FSL library is translated once, here, and the
tool crates build on it (`larmorx-nc-mcflirt`, `larmorx-nc-bet`; FLIRT, FAST and topup next).
mcflirt's and BET's translations of the same libraries were joined into one at BET's merge
(2026-10-25); every case of both replicas' records and golden tests gives the same bytes with
the joined version.

## The API, for the next replicas

| Module | What a tool uses |
|---|---|
| `volume` | `Volume<T>` (`volume<T>`; `Volume` alone is `volume<float>`) for `T` in [`Elem`]: `i8` (`char`), `i16`, `i32`, `f32`, `f64`. Construction (`new`, `new7`, `copyconvert` between types), access (`value`, `get` with the volume's extrapolation, `set`, `idx`), properties (`pixdims`, `sform`/`qform` and their codes, `copy_properties`, `sub_volume`, `extrema`, `left_right_order`, `swap_lr_order`, `make_radiological`/`make_neurological`, `sampling_mat`, `niftivox2newimagevox_mat`), float statistics (`calc_cog`, `cog_scaled_mm`, `backgroundval`/`background_value` with FSL-22's `NanOrder` and `Background`, `robust_limits`). Files: `read_volume::<T>` (a 4D file cut to its first volume with FSL's warning), `read_volume4d::<T>` (the volume, the file's datatype and units), `file_dtype`, `closest_templated_type`; `save_volume` (the volume's own type) and, for float volumes, `save`/`save_typed` (FSL's casts or the FSL-09 correction), with `SaveOptions` (`descrip`, units, FSL-56's offset reproduced or not); `BUILDSTRING` (newimage's `descrip`); `env_output_type`, `output_extension`, `offset_defect_applies`; file names (`make_basename`, `return_validimagefilename`, `find_pathname`, `is_regular_file`); `round_f64` (`MISCMATHS::round(double)`). Errors: `FslError` (FSL's printed lines, the exception's type and `what()`), `SaveError` |
| `nifti` | `NiftiHeader` (every field of NewNifti's header, `get_sform`/`set_sform`, `get_qform`/`set_qform`, `set_nifti_version`), `NiftiExtension`, `load_header`, `load_image`, `save_image` (with FSL-56's switch), the legacy matrix routines (`quatern_to_mat44`, `mat44_to_quatern`, `mat33_determ`, `mat33_inverse`, `mat44_inverse`, `mat33_polar`) |
| `interp`, `kernel` | `interpolate` (the volume's method: nearest, trilinear for every element type, sinc and splines for float volumes; `Volume::interpolate` for the defaults), `q_tri`, `round_f32`, `SplineCoefs`, the sinc kernels |
| `newimagefns` | `isotropic_resample`, `affine_transform`, `raw_affine_transform`, `affine_transform_mask`, `gradient`, `smooth` (separable Gaussian, threads without changing the result), `gaussian_kernel_1d` |
| `costfns`, `optimise`, `miscmaths` | FLIRT's cost functions, the line-search optimiser, the transform algebra, `read_ascii_matrix` |
| `arma` | `Mat` and the evaluation paths of Armadillo's small products, inverses, determinants and solves (`mul`, `mul3`, `mul4`, `gemv_tinysq`, `inv`, `det`, `solve`, `dot`, `accu`, `trace`) |
| `mesh` | meshclass: `Mesh` (`icosa`, `update`, `translation`, `rescale`, `retessellate`, `self_intersection`, `real_self_intersection`, `save_vtk`, ...), `MPoint`, `V3`, `mesh::profile::Profile`, `mesh::load` (`load`, OFF, VTK, FreeSurfer surfaces) |
| `options` | fsl-utils' `OptionParser`, `Opt`, the parsed values and `usage` |
| `avwutils` | what FSL's scripts run of `fslval`, `fslroi`, `fslmerge -z`, `fslmaths -dilF`/`-mas`, `fslstats -l -C`, and fslpy's `imtest`, `remove_ext`, `imglob`; `ToolFailure` (the program's messages and status) |
| `settings` | `Settings`: FSL's environment variables as a table, read once (`from_env(FSL_VARIABLES)`) |
| `cnum`, `x87`, `sse` | x86-64's float-to-integer conversions, `strtod`/`strtof`/`atof`/`atoi`/`strtol`, `printf`'s `%f`/`%e`/`%g` and C++ streams' default (`stream`); x87 `long double`; SSE's NaN rules (`add_f32`, ..., `S32`/`S64`) |
| `fdlibm`, `glibc_float`, `glibc_double` | glibc 2.35's maths as FSL's binaries call it: `atan2f`, `atanf`, `tanh`, `expm1`; `sinf`, `cosf`, `sincosf`, `logf`; `pow`, `exp` |
| `istream`, `stdsort` | libstdc++'s formatted input (`>>`, `getline`), `std::sort` and `std::nth_element` |

## Licences of the translated sources

- **FSL** (fsl-newimage, fsl-miscmaths, fsl-newnifti, fsl-znzlib, fsl-meshclass, fsl-utils,
  fsl-avwutils): Copyright (C) 1999-2023 University of Oxford, the FSL Licence. `nifti1.h`,
  `znzlib.{h,c}` and the code `legacyFunctions.cc` takes from `nifti1_io.c` were released to the
  public domain by their authors.
- **fslpy 3.21.1** (`avwutils.rs`: the file-name rules of `imtest`, `remove_ext`, `imglob`, which
  FSL's scripts run): Apache-2.0, Copyright 2016-2024 University of Oxford (Paul McCarthy).
- **Armadillo 10.5.1** as bundled by **fsl-armawrap 0.6.0** (`arma.rs`): Apache-2.0
  (`../LICENSES/Apache-2.0.txt`; notices in `../NOTICE`).
- **fdlibm**, as the GNU C Library 2.35 ships `atan2f`, `atanf`, `tanh` and `expm1` (`fdlibm.rs`):
  only Sun's permissive notice (`SunPro`), reproduced in the file and in `../NOTICE`. Source:
  glibc 2.35's `sysdeps/ieee754/flt-32/e_atan2f.c`, `s_atanf.c`, `sysdeps/ieee754/dbl-64/s_tanh.c`,
  `s_expm1.c` (fdlibm code, no LGPL notice).
- **Arm Optimized Routines v19.11** (`glibc_float.rs`, `glibc_double.rs`): MIT
  (`../LICENSES/MIT-arm-optimized-routines.txt`). glibc 2.35's `sinf`, `cosf`, `sincosf`,
  `logf`, `pow` and `exp` are these routines with the same arithmetic; the translations are
  made from Arm's MIT files, not from glibc's LGPL copies (compared line by line: identical
  expressions and constants; for `pow` and `exp` every number of the tables with Arm's x86-64
  configuration, `validation/investigations/fsl-bet/arm_glibc_pow_exp.py`). The fused
  multiply-adds follow the machine code of Ubuntu 22.04's `libm.so.6` (`__pow_fma`,
  `__exp_fma`; `tools/libm-verify`).
- **libstdc++ behaviour** (`stdsort.rs`, `istream.rs`): written from the algorithms' published
  descriptions (the SGI STL's introsort and introselect, the C++ standard's `num_get`), not copied
  from libstdc++. The SGI STL is Copyright (c) 1994 Hewlett-Packard Company and Copyright (c)
  1996 Silicon Graphics Computer Systems, Inc., under their permission notice ("Permission to
  use, copy, modify, distribute and sell this software and its documentation for any purpose is
  hereby granted without fee, provided that the above copyright notice appear in all copies and
  that both that copyright notice and this permission notice appear in supporting
  documentation. [Hewlett-Packard Company | Silicon Graphics] makes no representations about the
  suitability of this software for any purpose. It is provided "as is" without express or
  implied warranty."; both in full in `../NOTICE`). `stdsort.rs` differs from the SGI STL where
  libstdc++ does: the pivot step moves the median of the second, middle and last elements to
  the first place and partitions the rest (the SGI STL partitions around a copy of the median of
  the first, middle and last). `sort_f32_checked` (FSL-22's `reproduce`, `calc_bval`) checks
  every index against the vector's ends; its results equal libstdc++'s `std::sort` on 768
  vectors with and without NaN values (GCC 11.4 and 14.3;
  `validation/probes/libstdcxx-sort/sort_probe.cc`, the unit test
  `checked_sort_gives_libstdcxx_order_with_nan`).
- `cnum.rs`, `settings.rs` and `sse.rs` hold no FSL code (C/C++ number semantics of FSL's x86-64
  binaries; FSL's environment variables as a table, porting guide §9.1; SSE's NaN rules, made
  explicit so that NaN results have FSL's x86-64 bits on every platform); `x87.rs` emulates the
  x87 extended-precision arithmetic that FSL's NIfTI code uses through `long double`.

Every source file starts with an SPDX header naming its licence and the files it translates.

## Files and functions translated

| Rust module | Upstream file | Functions, classes |
|---|---|---|
| `nifti` | fsl-newnifti `NewNifti.{h,cc}`, `legacyFunctions.{h,cc}`, `nifti1.h`, `nifti2.h`; fsl-znzlib `znzlib.c` | `NiftiHeader` (fields, `initialise`, `sformCode`/`qformCode`, `setNiftiVersion`, `niftiVersion`, `isAnalyze`, `singleFile`, `bpvOfDatatype`, `nominalVoxOffset`, origin), `NiftiExtension`, `LegacyFields`, `NiftiIO::loadImage`/`loadHeader`/`saveImage` (NIfTI-1, NIfTI-2 and ANALYZE 7.5, byte swapping, `.hdr`/`.img` pairs, extensions, gzip with zlib's header), `nifti_quatern_to_mat44`, `nifti_mat44_to_quatern`, `nifti_mat33_determ`, `nifti_mat33_inverse`, `nifti_mat44_inverse`, `nifti_mat33_polar`, `nifti_mat33_rownorm`/`colnorm`, the `znzread`/`znzwrite` semantics; FSL's messages |
| `volume` | fsl-newimage `newimage.{h,cc}`, `newimageio.h`, `generalio.cc` | `volume<T>`/`volume4D<T>` (sizes, voxel sizes, sform/qform, interpolation, extrapolation, pad value, `operator()` with `extrapolate`, `copyproperties`, `copyconvert`, `operator[]`, `setDisplayMaximumMinimum`), `calc_cog`/`cog("scaled_mm")`, `backgroundval` (`calc_bval`), `robustmin`/`robustmax` (`find_thresholds`, `imageHistogram`), `left_right_order`, `swapmat`, `swapdimensions`, `makeradiological`/`makeneurological`, `niftivox2newimagevox_mat`, `periodicclamp`, `mirrorclamp`, `read_volume`, `read_volume4D` (`read_general`, `ConvertAndScaleNewNiftiBuffer`, `convertbuffer` into each element type, `set_volume_properties`), `dtype`, `save_volume`, `save_volume_dtype` (`set_fsl_hdr`, `convertbuffer`), `make_basename`, `return_validimagefilename`, `find_pathname`, `FSLOUTPUTTYPE`; `imthrow` and FSL's exception texts; fsl-miscmaths `round(double)` |
| `interp` | fsl-newimage `newimage.cc`; fsl-miscmaths `miscmaths.cc`, `splinterpolator.h` | `interpolate`, `interpolatevalue` (nearest neighbour, trilinear `q_tri`, sinc, spline), `kernelinterpolation`, `getneighbours`, `round`, `Splinterpolator<float>` (order 3: `calc_coef`, `deconv_along`, `get_wgt`, `value_at`, the boundary conditions) |
| `kernel` | fsl-miscmaths `kernel.{h,cc}` | `sincfn`, `hanning`, `blackman`, `rectangular`, `sinckernel1D`, `kernelval`, `kernelstorage`; `definesincinterpolation` (newimage) |
| `costfns` | fsl-newimage `costfns.{h,cc}` | `Costfn` (`cost`, `set_no_bins`, `set_costfn`, smoothing), `normcorr`, `normcorr_smoothed`, `normcorr_smoothed_sinc`, `leastsquares`, `leastsquares_smoothed`, `corr_ratio`, `corr_ratio_smoothed`, `woods_fn`, `mutual_info_smoothed`, `normalised_mutual_info_smoothed` (`calc_smoothed_entropy`), `findrangex`, `q_tri_interpolation`, `q_sinc_interpolation` (`q_setupkernel`, `q_kernelval`, `q_sinc`, `q_hanning`), `in_interp_bounds` |
| `optimise` | fsl-miscmaths `optimise.{h,cc}` | `optimise`, `optimise1d`, `findinitialbound`, `estquadmin`, `extrapolatept`, `nextpt`, `along` |
| `miscmaths` | fsl-miscmaths `miscmaths.{h,cc}` | `make_rot`, `construct_rotmat_euler`, `compose_aff`, `decompose_aff`, `rotmat2euler`, `rms_deviation`, `read_ascii_matrix` (`skip_alpha`, `isNumber`), `norm2`, `dot`, `cross` |
| `newimagefns` | fsl-newimage `newimagefns.{h,cc}` | `isotropic_resample`, `affine_transform`, `raw_affine_transform`, `affine_transform_mask`, `make_grad_masks`, `gradient`, `smooth`, `convolve_separable`, `convolve`, `gaussian_kernel1D` |
| `arma` | Armadillo 10.5.1 (fsl-armawrap 0.6.0) | the evaluation of `*` chains (`glue_times` with `inv` and `solve` rewriting), `gemv`/`gemm_emul_tinysq`, `gemm_emul_large`, `op_dot::direct_dot_arma`, `accu`, `max`, `op_inv::apply_tiny`, `op_det`, `auxlib::solve_square_tiny`, `trace` |
| `mesh` | fsl-meshclass `point.{cpp,h}`, `mpoint.{cpp,h}`, `triangle.{cpp,h}`, `mesh.{cpp,h}`, `pt_special.{cpp,h}`, `selfintersection.cpp`, `meshclass.h` | `Pt`, `Vec` and their operators, `Mpoint`, `Triangle` (the constructor's neighbour bookkeeping, `normal`, `centroid`, `intersect`), `Mesh` (copy constructor, `operator=`, `update`, `translation`, `rescale`, `retessellate`, `self_intersection`, `real_self_intersection`, `stream_mesh` type 3, `save`), `make_mesh_from_icosa` |
| `mesh::load` | fsl-meshclass `mesh.cpp` | `Mesh::load`, `load_off`, `load_vtk_ASCII`, `load_fs` |
| `mesh::profile` | fsl-meshclass `profile.{cpp,h}` | `Profile`, `pro_pair` |
| `options` | fsl-utils `options.{h,cc}`, `parse.cc`, `usage.cc`, `functions.cc`, `matches.cc` | `OptionParser`, `Option<T>`, `parse_command_line`, `parse_option`, `parse_long_option`, `usage`, `string_to_T` |
| `avwutils` | fsl-avwutils `fslstats.cc` (`-C`, `-l`), `fslroi.cc`, `fslmerge.cc` (`-z`), `fslmaths.cc` (`-dilF`, `-mas`), `fslval`; fslpy `fsl/utils/path.py`, `fsl/scripts/imtest.py`, `remove_ext.py`, `imglob.py` | what FSL's scripts run of them |
| `fdlibm` | glibc 2.35 `e_atan2f.c`, `s_atanf.c`, `s_tanh.c`, `s_expm1.c` (fdlibm) | `atan2f`, `atanf`, with the constants as the compiler rounds fdlibm's decimal literals (`aT[0]` is `0x3eaaaaab`, not its comment's `0x3eaaaaaa`; corrected 2026-10-25) (`tools/glibc-float-verify`: `atanf` bit-identical to the C library on all 2^32 inputs, `atan2f` on every pair of 38 special values and 3 × 2^30 seeded random pairs); `__tanh`, `__expm1` (`tools/libm-verify`) |
| `glibc_float` | Arm Optimized Routines v19.11 `math/sincosf.h`, `sincosf.c`, `sinf.c`, `cosf.c`, `sincosf_data.c`, `logf.c`, `logf_data.c` | `sinf`, `cosf`, `sincosf`, `logf`, as glibc 2.35's x86-64 FMA build computes them (`tools/glibc-float-verify`: bit-identical to the C library on all 2^32 inputs of each) |
| `glibc_double` | Arm Optimized Routines v19.11 `math/pow.c`, `pow_log_data.c`, `exp.c`, `exp_data.c` (glibc 2.35's `e_pow.c`, `e_pow_log_data.c`, `e_exp.c`, `e_exp_data.c`) | `__pow`, `log_inline`, `exp_inline`, `specialcase`, `checkint`; `__exp` and its `specialcase`, as `__pow_fma` and `__exp_fma` (`tools/libm-verify`: 0 of 4 million `pow` and 0 of 4 million `exp` differ from the C library) |
| `cnum` | (glibc behaviour) | `strtod`, `strtof`, `strtol`, `atof`, `atoi`, `printf`'s `%g`, `%f`, `%e`; x86-64's conversions |
| `x87` | (x87 behaviour) | `long double` sums, products and quotients rounded to 64 bits, then stored (`tools/libm-verify`: 0 of 5 million differ from the x87 unit) |
| `stdsort` | (libstdc++ behaviour; the SGI STL, under the HP/SGI notice) | `std::sort` (`sort_by`; `sort_f32_checked` for floats that may be NaN), `std::nth_element` on doubles |
| `istream` | (libstdc++ behaviour) | `operator>>` for `int`, `float`, `double`, `std::string`; `std::getline` |

## Changes from the originals

- **Only what FSL's tools in larmorx-nc call is translated**, for the element types they use;
  sinc and spline interpolation for float volumes only. The data layout is a flat `Vec<T>` with
  FSL's x-fastest order.
- **Errors are values.** FSL's exceptions (`imthrow`, `NiftiException`, Armadillo's
  `logic_error`) become `FslError` with FSL's message and exception type; the programs print
  what the C++ runtime prints for an uncaught exception and end as described in their own
  crates.
- **Maths as FSL's Linux build computes it.** The C library's functions are glibc 2.35's
  (`fdlibm`, `glibc_float`, `glibc_double`); double `cos` and `log` are correctly rounded
  (`larmorx_core::math`), which glibc's agree with on every argument the replicas use
  (`tools/glibc-float-verify kernel`); x87 `long double` arithmetic is emulated (`x87`);
  Armadillo's BLAS-backed paths (matrices larger than 4 x 4) are not reached by the replicas and
  panic if they ever are. Where NaN or infinite voxels can reach a result, it is computed with
  SSE's NaN rules (`sse`): the centre of gravity, the trilinear interpolation, the gradient, the
  Gaussian smoothing, `fslmaths -mas` and `fslstats`'s threshold, newimage's robust range
  (`find_thresholds`: the histogram's range and the 2 and 98 % thresholds) and meshclass's
  points and vectors (BET's merge, from BET's NaN matrix, `validation/probes/fsl-bet-nan`).
- **Thread-independent.** The Gaussian smoothing shares its z slices between threads; nothing
  else here is parallel; the results never depend on the thread count.
- **D13 hooks.** `Costfn::skip_nonfinite` (FSL-14's non-finite samples), the corrected centre of
  gravity, NewNifti's data offset of several extensions (`SaveOptions::reproduce_offset`,
  FSL-56), and a volume's `nan_order` (FSL-22: a shell with NaN sorted with NaNs last, or in
  libstdc++'s order; `background_value` says which) are options the programs set from their
  mode; with `reproduce` everything is FSL's code path.
- **Gzip files with znzlib's settings.** Compressed outputs carry the gzip header of a Unix zlib
  at its default level, as znzlib's `gzopen` writes it; their decompressed bytes are FSL's. The
  compressed stream is zlib-rs's (pure Rust), whose blocks need not be zlib's: compressed files are
  compared decompressed (decision log, 2026-10-16). Saving converts and writes a volume's values
  chunk by chunk, with no copy of the image, so that it needs no more memory than FSL's.
