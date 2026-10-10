# ITK's NIfTI reader and writer (ITK v5.4.5, `Modules/IO/NIFTI/src/itkNiftiImageIO.cxx`)

ANTs reads and writes every image through this code, so ANTs results depend on it.

## Geometry

ITK chooses between the qform and the sform with its own rules, which differ from nibabel's.
*Read; validated* on 252 catalog files (`docs/validation/` → itk-geometry).
*larmorx:* `nifti::itk::itk_geometry`.

- **Neither code set:** origin 0 and identity direction. With the default Analyze flavour
  (`ITK4Warning`), the Analyze orientation code is ignored.
- **The sform wins** when it is orthonormal and one of these holds: the qform code is 0, the
  sform code is `SCANNER_ANAT`, or both are set and describe nearly the same space (within
  1e-4). Otherwise the qform is used.
- **A non-orthonormal sform without a qform is an error.** `ITK_NIFTI_SFORM_PERMISSIVE` is
  off by default and in ANTs builds.
- **Matrices are built in single precision**, as nifti_clib stores them (`float mat44`), and
  converted to LPS.
- **Units:** metres scale spacing and origin by 1000, microns by 1/1000. Milliseconds and
  microseconds scale the fourth axis to seconds.
- **Negative `pixdim`** stays negative in the ImageIO. `ImageFileReader` then makes the
  spacing positive and flips the direction column.
- **NIfTI-2 is not readable** by ITK 5.4. Neither are `NIFTI_INTENT_GENMATRIX` images.
- **Number of dimensions:** trailing size-1 dimensions beyond the third are dropped. Vector
  intents (`VECTOR`, `DISPVECT`, `SYMMATRIX`) use the fifth dimension as components.

## Voxel values

**Scaling is applied in float32 for integer data** (`itkNiftiImageIO.cxx:600-640, 745-795,
1275-1310`). When the header asks for scaling (`MustRescale`, line 466), the reported
component type becomes `float` for integer data. Each value is cast to float, then
`tmp = double(v)·slope + inter`, and the result is stored as float again. nibabel scales in
float64, so **a scaled int16 BOLD differs between nibabel and ITK by float32 rounding**.
Float32 data are scaled in double and rounded to float32. Float64 data are scaled in double.
*Read.* *larmorx:* `nifti::itk::read_itk_image` reproduces it.

**Non-finite float values are read as 0.** nifti_clib's `nifti_read_buffer`
(`Modules/ThirdParty/NIFTI/src/nifti/niftilib/nifti1_io.c:5013-5049`, compiled in because
`isfinite` is a macro) replaces every NaN and ±inf of float32 and float64 data by 0 as it
reads, before any scaling. So ITK, and every ANTs program, never sees a NaN from a NIfTI
file: `ImageMath abs` turns `-inf` into 0. *Read; verified* (found by the ImageMath parity
suite: `head-outliers` has a NaN and a `-inf`). *larmorx:* `read_itk_image` does the same
(it did not before 2026-10-10; no earlier test file had non-finite values).

**When ITK scales.** Only if `|slope| > ε` and (`|slope − 1| > ε` or `|inter| > ε`), with
`ε` = double epsilon. Analyze files (no NIfTI magic) are never scaled. A slope of 0 is
replaced by 1. nifti_clib first replaces non-finite slope and intercept by 0
(`FIXED_FLOAT`). *Read.*

**Displacement vectors: RAS→LPS only for `NIFTI_INTENT_DISPVECT`.** With intent `DISPVECT`
(1006), ITK negates the x and y components on reading (`m_ConvertRASDisplacementVectors`,
default on; `itkNiftiImageIO.cxx:1221-1233`, `ConvertRASToFromLPS_CXYZT` at line 501). With
`NIFTI_INTENT_VECTOR` (1007) the vectors are taken as LPS. ITK writes vector images, ANTs
warps included, with intent `VECTOR` (line 1590), so ANTs' own warps round-trip unchanged.
A `DISPVECT` field written in RAS by another tool is converted. The conversion is only
implemented for float data: unscaled integer vectors throw. *Read.*
*larmorx:* reproduced in `read_itk_image`. ANTsPy 0.6.3 carries 7 commits after ANTs v2.6.5
that touch vector I/O, which the parity runs must keep in mind.

## Writing

**ITK writes** both qform and sform with code `SCANNER_ANAT` (1), and `xyzt_units` mm + s.
*Verified* on antsApplyTransforms output. *larmorx's CLI:* writes the same codes and units.

**The header ITK writes, field by field** (`NiftiImageIO::WriteImageInformation` and
`SetNIfTIOrientationFromImageIO`, `itkNiftiImageIO.cxx:1429-1790, 2236-2363`; nifti_clib's
`nifti_simple_init_nim` and `nifti_convert_nim2nhdr`). *Read; validated:* every output of the
ImageMath, ThresholdImage and MultiplyImages suites (167 cases) has the same 348 header bytes
as ANTs'.
- `dim[0]` is the image dimension, `dim[1..7]` its sizes then 1; `pixdim[1..3]` the spacing
  (float), and **`pixdim[4..7]` are 0** for a 3D image (nifti_clib's initial header has
  `dim[0] = 3`, so they are never fixed to 1); a 2D image keeps `pixdim[3] = 1`;
- the direction becomes a rotation through `nifti_make_orthog_mat44` (rows normalised in
  float, then nifti_clib's single-precision polar decomposition) and `nifti_mat44_to_quatern`;
  the sform is that rotation with each column multiplied by the float spacing; qoffset and
  srow translations are the origin in float, x and y negated (LPS → RAS);
- `pixdim[0]` is the quaternion's `qfac`, `scl_slope` 1, `scl_inter` 0, `regular` `'r'`,
  `vox_offset` 352, `toffset` the 4th axis origin (else 0), `cal_*`, `slice_*`, `intent_*`,
  `dim_info` and `glmax/glmin` 0;
- **`descrip` and `aux_file` come from the image's metadata dictionary**: the input file's
  when the written image is the object a reader produced (ITK's reader stores `descrip` as
  `ITK_FileNotes`), empty for images made by filters, `AllocImage` or `CopyInformation`
  (which does not copy the dictionary).
*larmorx:* `nifti::itk::itk_header` / `write_itk_image`, with `ItkMeta` for the dictionary.

**Signed zeros in the direction: ITK's setters keep `+0.0`.** The reader computes the LPS
direction as `−1 · m[i][j]` for the first two rows, which gives `−0.0` where the matrix has
`0`. But `ImageBase::SetDirection` copies only the elements that compare unequal to the
current ones (`itkImageBase.hxx:149-160`), and a new image starts with the identity, so those
`−0.0` never replace its `+0.0`. `SetOrigin` (`itkSetMacro`) replaces the whole point only
if it differs. The sign survives into the written header (a `−0.0` in an srow), so it decides
whether the header bytes match. *Read; verified* (a header byte differed until this was
modelled). *larmorx:* `ItkGeometry::stored_in_new_image`, applied to every geometry read.

## nifti_clib file naming

**Given `x.nii.gz`, a reader built on nifti_clib opens `x.nii` instead if both exist.** It
tries the uncompressed name first. This silently corrupted an early benchmark that had both
files in one directory. *Verified* with SimpleITK. *larmorx:* opens exactly the path given.
Benchmarks keep compressed and uncompressed copies in separate directories.
