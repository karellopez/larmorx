# Provenance: larmorx-nc-core

**Replica: translated from FSL's source.** This crate is a step-by-step Rust translation of the
FSL libraries FSL's registration and segmentation tools share, from FSL 6.0.7.17 (the conda
packages fsl-newimage 2203.12, fsl-miscmaths 2203.2, fsl-newnifti 4.1.0, fsl-znzlib 2111.0,
fsl-armawrap 0.6.0), written to give FSL's results bit for bit. It is distributed under the FSL
Licence (`../LICENSE`), as part of the `larmorx-nc` package (`docs/licensing.md`). The FSL
files it was translated from are in `../fsl-source/`, unchanged. No Apache-2.0 crate uses it,
and it uses no GPL crate.

It is the shared core of the FSL replicas (mcflirt now; FLIRT, BET, FAST and topup next): NIfTI
I/O as FSL does it, `volume<float>`, interpolation, the cost functions, the optimiser, the
transform algebra and the matrix rules of Armadillo as armawrap evaluates them.

## Licences of the translated sources

- **FSL** (fsl-newimage, fsl-miscmaths, fsl-newnifti, fsl-znzlib): Copyright (C) 1999-2023
  University of Oxford, the FSL Licence. `nifti1.h`, `znzlib.{h,c}` and the code
  `legacyFunctions.cc` takes from `nifti1_io.c` were released to the public domain by their
  authors.
- **Armadillo 10.5.1** as bundled by **fsl-armawrap 0.6.0** (`arma.rs`): Apache-2.0
  (`../LICENSES/Apache-2.0.txt`; notices in `../NOTICE`).
- **fdlibm**, as the GNU C Library 2.35 ships `atan2f` and `atanf` (`fdlibm.rs`): only Sun's
  permissive notice (`SunPro`), reproduced in the file and in `../NOTICE`. Source: glibc
  2.35's `sysdeps/ieee754/flt-32/e_atan2f.c` and `s_atanf.c` (fdlibm code, no LGPL notice).
- **Arm Optimized Routines v19.11** (`glibc_float.rs`): MIT (`../LICENSES/MIT-arm-optimized-routines.txt`).
  glibc 2.35's `sinf`, `cosf`, `sincosf` and `logf` are these routines with the same
  arithmetic; the translation is made from Arm's MIT files, not from glibc's LGPL copies
  (compared line by line: identical expressions and constants).
- `cnum.rs` and `settings.rs` hold no FSL code (C/C++ number semantics of FSL's x86-64
  binaries; FSL's environment variables as a table, porting guide §9.1); `x87.rs` emulates the
  x87 extended-precision arithmetic that FSL's NIfTI code uses through `long double`.

Every source file starts with an SPDX header naming its licence and the files it translates.

## Files and functions translated

| Rust module | Upstream file | Functions, classes |
|---|---|---|
| `nifti` | fsl-newnifti `NewNifti.{h,cc}`, `legacyFunctions.{h,cc}`, `nifti1.h`, `nifti2.h`; fsl-znzlib `znzlib.c` | `NiftiHeader` (fields, `initialise`, `sformCode`/`qformCode`, `setNiftiVersion`, `niftiVersion`, `isAnalyze`, `singleFile`, `bpvOfDatatype`, `nominalVoxOffset`, origin), `NiftiExtension`, `LegacyFields`, `NiftiIO::loadImage`/`loadHeader`/`saveImage` (NIfTI-1, NIfTI-2 and ANALYZE 7.5, byte swapping, `.hdr`/`.img` pairs, gzip), `nifti_quatern_to_mat44`, `nifti_mat44_to_quatern`, `nifti_mat33_determ`, `nifti_mat33_inverse`, `nifti_mat44_inverse`, `nifti_mat33_polar`, `nifti_mat33_rownorm`/`colnorm`, the `znzread`/`znzwrite` semantics; FSL's messages |
| `volume` | fsl-newimage `newimage.{h,cc}`, `newimageio.h`, `generalio.cc` | `volume<float>`/`volume4D<float>` (sizes, voxel sizes, sform/qform, interpolation, extrapolation, pad value, `operator()` with `extrapolate`, `copyproperties`, `operator[]`, `setDisplayMaximumMinimum`), `calc_cog`/`cog("scaled_mm")`, `backgroundval` (`calc_bval`), `left_right_order`, `swapmat`, `swapdimensions`, `makeradiological`/`makeneurological`, `periodicclamp`, `mirrorclamp`, `read_volume`, `read_volume4D` (`read_general`, `ConvertAndScaleNewNiftiBuffer`, `set_volume_properties`), `dtype`, `save_volume`, `save_volume_dtype` (`set_fsl_hdr`, `convertbuffer`), `make_basename`, `return_validimagefilename`, `find_pathname`, `FSLOUTPUTTYPE`; `imthrow` and FSL's exception texts |
| `interp` | fsl-newimage `newimage.cc`; fsl-miscmaths `miscmaths.cc`, `splinterpolator.h` | `interpolate`, `interpolatevalue` (nearest neighbour, trilinear `q_tri`, sinc, spline), `kernelinterpolation`, `getneighbours`, `round`, `Splinterpolator<float>` (order 3: `calc_coef`, `deconv_along`, `get_wgt`, `value_at`, the boundary conditions) |
| `kernel` | fsl-miscmaths `kernel.{h,cc}` | `sincfn`, `hanning`, `blackman`, `rectangular`, `sinckernel1D`, `kernelval`, `kernelstorage`; `definesincinterpolation` (newimage) |
| `costfns` | fsl-newimage `costfns.{h,cc}` | `Costfn` (`cost`, `set_no_bins`, `set_costfn`, smoothing), `normcorr`, `normcorr_smoothed`, `normcorr_smoothed_sinc`, `leastsquares`, `leastsquares_smoothed`, `corr_ratio`, `corr_ratio_smoothed`, `woods_fn`, `mutual_info_smoothed`, `normalised_mutual_info_smoothed` (`calc_smoothed_entropy`), `findrangex`, `q_tri_interpolation`, `q_sinc_interpolation` (`q_setupkernel`, `q_kernelval`, `q_sinc`, `q_hanning`), `in_interp_bounds` |
| `optimise` | fsl-miscmaths `optimise.{h,cc}` | `optimise`, `optimise1d`, `findinitialbound`, `estquadmin`, `extrapolatept`, `nextpt`, `along` |
| `miscmaths` | fsl-miscmaths `miscmaths.{h,cc}` | `make_rot`, `construct_rotmat_euler`, `compose_aff`, `decompose_aff`, `rotmat2euler`, `rms_deviation`, `read_ascii_matrix` (`skip_alpha`, `isNumber`), `norm2`, `dot`, `cross` |
| `newimagefns` | fsl-newimage `newimagefns.{h,cc}` | `isotropic_resample`, `affine_transform`, `raw_affine_transform`, `affine_transform_mask`, `make_grad_masks`, `gradient` |
| `arma` | Armadillo 10.5.1 (fsl-armawrap 0.6.0) | the evaluation of `*` chains (`glue_times` with `inv` and `solve` rewriting), `gemv`/`gemm_emul_tinysq`, `gemm_emul_large`, `op_dot::direct_dot_arma`, `accu`, `max`, `op_inv::apply_tiny`, `op_det`, `auxlib::solve_square_tiny`, `trace` |
| `fdlibm` | glibc 2.35 `e_atan2f.c`, `s_atanf.c` (fdlibm) | `atan2f`, `atanf`, with the constants as the compiler rounds fdlibm's decimal literals (`aT[0]` is `0x3eaaaaab`, not its comment's `0x3eaaaaaa`; corrected 2026-10-25) (`tools/glibc-float-verify`: `atanf` bit-identical to the C library on all 2^32 inputs, `atan2f` on every pair of 38 special values and 3 × 2^30 seeded random pairs) |
| `glibc_float` | Arm Optimized Routines v19.11 `math/sincosf.h`, `sincosf.c`, `sinf.c`, `cosf.c`, `sincosf_data.c`, `logf.c`, `logf_data.c` | `sinf`, `cosf`, `sincosf`, `logf`, as glibc 2.35's x86-64 FMA build computes them (`tools/glibc-float-verify`: bit-identical to the C library on all 2^32 inputs of each) |

## Changes from the originals

- **Only what FSL's tools in larmorx-nc call is translated**, for `float` images (the type
  mcflirt reads every image into); other template types and functions are left out. The
  data layout is a flat `Vec<f32>` with FSL's x-fastest order.
- **Errors are values.** FSL's exceptions (`imthrow`, `NiftiException`, Armadillo's
  `logic_error`) become `FslError` with FSL's message and exception type; the programs print
  what the C++ runtime prints for an uncaught exception and end as described in their own
  crates.
- **Maths as FSL's Linux build computes it.** The C library's float functions are glibc
  2.35's (`fdlibm`, `glibc_float`); double `cos` and `log` are correctly rounded
  (`larmorx_core::math`), which glibc's agree with on every argument the replicas use
  (`tools/glibc-float-verify kernel`); x87 `long double` arithmetic is emulated (`x87`);
  Armadillo's BLAS-backed paths (matrices larger than 4 x 4) are not reached by mcflirt and
  panic if they ever are.
- **Thread-independent.** Nothing here is parallel; the programs parallelize over volumes
  where FSL's results do not depend on the order (see their crates).
- **D13 hooks.** `Costfn::skip_nonfinite` (FSL-14's non-finite samples) and the corrected
  centre of gravity are options the programs set for `fix`; with `reproduce` everything is
  FSL's code path.
