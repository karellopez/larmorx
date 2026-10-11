# `antsApplyTransforms --float 1`: the single-precision pipeline (ANTs v2.6.5, ITK v5.4.5)

With `--float 1`, `antsApplyTransforms` instantiates its whole template with `T = float`
(`Examples/antsApplyTransforms.cxx:1534-1546`): `Image<float, 3>`, `CompositeTransform<float, 3>`
read by `TransformFileReaderTemplate<float>`, `ResampleImageFilter<Image<float>, Image<float>,
float>` (both the interpolator's and the transform's precision are float) and the interpolators
of `make_interpolator_snip.tmpl` with `TCoordRep = float`. fMRIPrep 25.2.6 passes `--float 1` in
14 calls of the ds000005 oracle run: the anatomical outputs and template priors to MNI, the
boldrefs to T1w and MNI, and the report's T1w on the boldref ([fmriprep-oracle.md](fmriprep-oracle.md)).

Below, "computed in double, stored as float" means what C++'s types make of it: an expression
with a `double` operand (a `double` literal, an image's geometry, `alpha`) is evaluated in
double and rounded once when stored into a `float` variable, array element or `Point<float>`.
Everything here is *read* from the pinned tags and *validated* by the parity suite (all 81
`--float 1` cases bit-identical to the unmodified ANTs, [validation](../validation/ants-apply-transforms.md))
and by the stage comparison (all 14 of fMRIPrep's `--float 1` outputs bit-identical,
[fmriprep-stages](../validation/fmriprep-stages.md)).

*larmorx:* `ApplyTransformsOptions::precision` / `precision="float32"` / `--float 1`; ITK's
computation type is a type parameter (`larmorx_core::ItkReal`, implemented for `f32` and
`f64`), so the float and double pipelines are the same code
(`larmorx_ants::apply_transforms`, `larmorx_transform::{MatrixOffset, PrecisionChain}`,
`larmorx_interp::Interpolator::with_precision`). Before 2026-10-11 larmorx computed `--float 1`
in double and rounded the input and output to float32: equal to ANTs in double precision
rounded to float32, and 1 to 8 × 10⁵ voxels apart from ANTs in each of the 14 fMRIPrep outputs.

## What is float, what stays double

| Quantity | Type with `--float 1` | Where |
|---|---|---|
| Input voxels | float (`ImageFileReader<Image<float>>`: a `static_cast` of the file's values) | `ReadImage<ImageType>` |
| Image geometry (origin, spacing, direction, `m_PhysicalPointToIndex`) | **double** (`SpacePrecisionType`) | `itkImageBase.h` |
| An output voxel's physical point | computed in double, **stored as `Point<float>`** before the first transform (`Point`'s converting constructor) | `itkResampleImageFilter.hxx:395-398, 461-464` |
| Transform parameters | float (the file's values cast; text parsed as float, below) | `itkTxtTransformIO.cxx`, `itkMatlabTransformIO.cxx:52-73`, `itkHDF5TransformIO.cxx:164-204` |
| Fixed parameters | double (`FixedParametersValueType`), but a centre is stored as `Point<float>` | `itkTransformBase.h:83`, `itkMatrixOffsetTransformBase.hxx:472-487` |
| Every transform's `TransformPoint` | float in, float out, float arithmetic | below |
| Continuous index | `(p − origin)` and `M⁻¹·v` in double from the float point, **stored as float** | `itkImageBase.h:510-525` |
| Scan line (linear transforms) | ends as float indices; `vector = end − start` in float; each index `start + α·vector` computed in double, stored as float | `itkResampleImageFilter.hxx:470-493` |
| Inside test | float index against float bounds (`start − 0.5`, `end + 0.5`) | `itkImageFunction.h` |
| Interpolated value | **double** (`NumericTraits<float>::RealType`) | `itkInterpolateImageFunction.h` |
| Output pixel | float: the double value clamped to `[−FLT_MAX, FLT_MAX]`, then rounded (`CastComponentWithBoundsChecking`; ±∞ become ±`FLT_MAX`, NaN passes) | `itkResampleImageFilter.hxx:302-322` |
| Default value (`-f`) | float, parsed by `istringstream >> float` | `antsCommandLineParser.h:122-138` |

## Transforms

- **Matrix-offset transforms** (`MatrixOffsetTransformBase<float>`, `AffineTransform`,
  `Rigid3DTransform`): `TransformPoint` is `M·p + offset`, each row summed from 0 in float
  (`itkMatrix.hxx:48-62`), then the float offset added. `ComputeOffset` subtracts the terms of
  `M·c` one at a time in float (`itkMatrixOffsetTransformBase.hxx:627-643`).
- **Inverses** (`-t [file,1]`): `itk::Matrix<float>::GetInverse` checks vnl's 3×3 determinant
  (six products, in float, `vnl_determinant.hxx`) against 0 and inverts by `vnl_svd<float>`
  (LINPACK's `ssvdc`; `V · W⁻¹ · Uᵀ` with `W⁻¹ = 1.0f / w` and float products); the offset is
  `−(M⁻¹·offset)` in float. As in double, the inverse of every 3D matrix-offset class is a plain
  `MatrixOffsetTransformBase` (`InvertTransform(*this)` with the base's static type), so
  Euler3DTransform's `ComputeMatrixParameters`, which rebuilds the matrix from angles, is never
  applied to it. *larmorx:* `MatrixOffset::<f32>::inverse`, `larmorx_core::vnl_svd_f32::vnl_inverse3_f32`.
- **Euler3DTransform<float>**: `std::cos(float)` and `std::sin(float)`, which are glibc's
  `cosf`/`sinf`, or `sincosf` where GCC pairs them (the oracle binary imports all three), and
  the rotation matrices multiplied by `vnl_matrix_fixed`'s product in float (each entry starts
  from the first product). glibc 2.35's `sinf` and `cosf` are not correctly rounded for 0.7 % of
  all floats (29.4 and 28.2 million of 2^32; GLIBC-03 in the [register](upstream-defects.md)).
  *larmorx:* correctly rounded `sinf`/`cosf` ported from CORE-MATH (`larmorx_core::math`), and
  `scripts/build_crmath_preload.sh` now replaces glibc's `sinf`, `cosf` and `sincosf` too.
- **VersorRigid3DTransform<float>**: the norm of the axis parameters is summed and rooted in
  float (`itkVersorRigid3DTransform.hxx`, `SetParameters`); an axis whose norm reaches
  `1 − 1e-10` is divided by `norm·(1 + 1e-10)` in double and stored as float. `Versor<float>::Set`
  then takes `sin = |axis|` in double (`Vector::GetNorm` accumulates in double), stored as float,
  and `w = √(1 − sin²)` in double, stored as float; `GetMatrix` multiplies pairs of components
  in float and computes each entry in double (`RealType`), stored as float (`itkVersor.hxx`).
  **Similarity3DTransform<float>** sums the squares in double instead (`double norm =
  p[0]*p[0]; norm += ...`), roots in double, and multiplies the matrix by the float scale.
- **A versor can fail to load in float** (*verified*): the axis scaled down to norm 1 can round
  back above 1 in float, and `Versor::Set` throws ("Trying to initialize a Versor with a vector
  whose magnitude is greater than 1"). With the parameters `-0.7041559 0.63925344 0.3665738`,
  ANTs 2.6.5 resamples with `--float 0` and exits 1 with `--float 1`. *larmorx:* refuses the same
  way (`invalid transform: versor with magnitude greater than 1`). Register: ITK-10.
- **TranslationTransform<float>**: `p + offset` in float, inverse `−offset`.
- **DisplacementFieldTransform<float>** (`itkDisplacementFieldTransform.hxx:68-98`): the field
  is `Image<Vector<float, 3>>` with double geometry. The float point's continuous index in the
  field is computed in double and stored as float; outside the buffer the point is returned
  unchanged. Inside, `VectorLinearInterpolateImageFunction<…, float>`
  (`itkVectorLinearInterpolateImageFunction.hxx`) has `InternalComputationType = float`: the
  base index is ITK's `Math::Floor` of the float index, distances and overlaps are float
  (`overlap *= 1.0 − d` is a double product stored as float), each `overlap · value` product is a
  float product, the sum and the total overlap are double; then `point[i] += d[i]` is computed in
  double and stored as float.
- **Composite transforms** apply their queue back to front, each component on `Point<float>`.

## Reading transform files in float

- **Text** (`TxtTransformIOTemplate<float>::Read`, `itkTxtTransformIO.cxx`): both the
  `Parameters` and the `FixedParameters` line are parsed into the same
  `OptimizerParameters<float>` buffer by `>> float` (`vnl_vector<float>::read_ascii`, so
  `strtof`), and the fixed parameters are then widened to double. A decimal is therefore rounded
  **once**, to float; parsing it as double and casting (what larmorx did before) differs when
  the double lands exactly on a midpoint between two floats. *Validated*
  (`float/parse/text-transform`, a text affine built of such numbers). *larmorx:*
  `larmorx_io::itk_transform::parse_itk_text_with(text, Precision::Float)`.
- **MATLAB** (`.mat`): parameters read as stored (float or double) and cast to float; fixed
  parameters read as double.
- **HDF5** (`.h5`): the same: `ReadParameters` reads as stored, `static_cast` to float;
  `ReadFixedParameters` gives double.
- **NIfTI displacement fields**: the vector image is read into float (a cast of float64 data).
- **`-f`**: `Convert<float>` reads the value with `>> float`, rounded once (*validated*,
  `float/parse/default-value`).

## Interpolators with `TCoordRep = float`

The interpolated values are double in every case; what changes is how the float index is
rounded.

| `-n` | What float changes |
|---|---|
| `Linear` | nothing beyond the float index: the 3D path's distances `index − floor` are exact in float, and the interpolation is in double |
| `NearestNeighbor` | `Math::RoundHalfIntegerUp<long>(float)`: `round_half_to_even(2x + 0.5f) >> 1` with `2x + 0.5f` **rounded in float**, so the largest float below a half-integer rounds up (`itkMathDetail.h:400-411`); in double this equals `floor(x + 0.5)` |
| `BSpline` | nothing beyond the float index: the support starts at `floor(float(x) + ½·even)` in both precisions, the weights and the coefficients (`BSplineDecompositionImageFilter`, `CoeffType` double) are double |
| `Gaussian`, `MultiLabel` | the region's ends `c + 0.5 ∓ cutoff` are computed in double and **stored as float** before `floor`/`ceil` (`ComputeInterpolationRegion`); the erf arrays take the index as double |
| `GenericLabel` | nothing beyond the float index (`LinearInterpolateImageFunction<LabelSelectionAdaptor, float>`) |
| `*WindowedSinc` | the base index is `Math::Floor<long>(float)`; the window function is `XWindowFunction<3, float, float>`: its argument is the double offset **stored as float**, its value is computed in double (`m_Factor` is double, `std::cos` and `std::sin` of a double) and **stored as float**, then multiplied by the double `Sinc(x)` |

*larmorx:* `larmorx_interp::Interpolator::with_precision`; nearest neighbour through
`nearest_as::<_, f32>`, the Gaussian regions and sinc windows generic over `ItkReal`.

## ITK's `Math::Floor` in the field and sinc lookups (both precisions)

`VectorLinearInterpolateImageFunction` and `WindowedSincInterpolateImageFunction` take their
base index with `itk::Math::Floor`, which on x86-64 is `round_half_to_even(2x − 0.5) >> 1`
(`itkMathDetail.h:414-422`): 0, not −1, for indices in `[−2⁻⁵⁵, 0)` (double) or `[−2⁻²⁶, 0)`
(float). larmorx used `floor` there before 2026-10-11; it now uses ITK's in both precisions
(ITK-01 in the [register](upstream-defects.md); the D13 decision about it applies here too).
No golden or parity case changed.

## Known difference not reproduced

- **`Rigid3DTransform`'s orthogonality check** (`itkRigid3DTransform.hxx`, `SetParameters`:
  a matrix that is not orthogonal within `MatrixOrthogonalityTolerance<T>` throws) is not
  reproduced, in either precision: larmorx reads such a file as an affine.
- **`GenericLabel` with both `−0.0` and `+0.0` in an image**: ITK's label set is a
  `std::set<PixelType>`, in which the two zeros are one label (the first one in the image's
  raster order); larmorx's label lookup tells them apart. Not seen in any image; noted for a
  later fix.
