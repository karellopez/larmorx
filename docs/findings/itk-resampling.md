# ITK resampling and interpolation (ITK v5.4.5), as `antsApplyTransforms` uses it

## `ResampleImageFilter` (`Modules/Filtering/ImageGrid/include/itkResampleImageFilter.hxx`)

**Two code paths.** When the transform is linear (`TransformCategory::Linear`, including a
composite of linear transforms), `LinearThreadedGenerateData` (line 432) maps only the two
ends of each output scan line. Its continuous index at `x` is
`start + (x / size_x)·(end − start)`, where `start` is the mapped index of voxel 0 and `end`
that of index `size_x`, one past the last voxel. Otherwise
`NonlinearThreadedGenerateData` (line 370) maps every voxel. The two paths round
differently, so a port must take the same path to match bit for bit. *Read.*
*larmorx:* `IndexMapper::row` takes the same paths.

**Output voxel → physical point** (`ImageBase::TransformIndexToPhysicalPoint`,
`itkImageBase.h:589`): for an integer index ITK starts from the origin and adds
`M[i][j]·index[j]` term by term. The continuous-index version sums the products first and
adds the origin last. *Read.* *larmorx:* `Grid3::voxel_to_point` (integer path) and
`Grid3::index_to_point`.

**Physical point → continuous index** (`itkImageBase.h:512`): `(p − origin)`, multiplied by
`m_PhysicalPointToIndex`, the SVD inverse of `direction·diag(spacing)`
(`itkImageBase.hxx:171-181`). See the inverse finding in
[itk-transforms.md](itk-transforms.md). *Read.*

**Inside test.** `ImageFunction::IsInsideBuffer` (`itkImageFunction.h:156`) accepts
`-0.5 ≤ index < size − 0.5` on every axis, written so that NaN is outside. None of the
interpolators ANTs uses override it. *Read.* *larmorx:* `larmorx_interp::is_inside`.

**Outside points get the default value** (`-f`), not the cast value. Inside points go
through `CastPixelWithBoundsChecking`, a clamp to the output pixel range. For ANTs it changes
nothing, because the resampler's output type is the computation type and the `-u` cast
happens later. *Read.*

## Interpolators

| ANTs `-n` | ITK class | Notes |
|---|---|---|
| `Linear` | `LinearInterpolateImageFunction` | 3D optimized path: an axis is skipped when its fractional distance is ≤ 0 or its upper neighbour is past the last voxel; interpolates x, then y, then z, as `a + (b − a)·d`. |
| `NearestNeighbor` | `NearestNeighborInterpolateImageFunction` | Rounds half up (`floor(x + 0.5)`). |
| `BSpline[order]` | `BSplineInterpolateImageFunction` + `BSplineDecompositionImageFilter` | Unser's prefilter with tolerance 1e-10 and mirror boundaries. The support starts at `floor(float(x) + ½·even)`: **the index is rounded to float first**. The weights follow ITK's expressions exactly (e.g. `z2n *= z2n * iz`), so they round the same way. |
| `Gaussian[sigma,alpha]` | `GaussianInterpolateImageFunction` | Separable erf weights over `±alpha·sigma` (sigma in mm, divided by the spacing), normalised by their sum. **erf is `vnl_erf`**, from `vnl_gamma_p` with relative error 3e-7, not the C library's. |
| `MultiLabel[sigma]` | `LabelImageGaussianInterpolateImageFunction` | Gaussian weights summed per label. Scanning with x fastest, the label whose sum first exceeds the running maximum wins. Labels are `std::map` keys, so they compare by value (`-0.0 == 0.0`). |
| `GenericLabel` | `LabelImageGenericInterpolateImageFunction<…, Linear>` (remote module ITKGenericLabelInterpolator) | Linear interpolation of each label's indicator, labels in ascending order. The strictly highest value wins, starting from label 0 at value 0. **ITK interpolates every distinct value in the image at every voxel**, and a float intensity image has as many labels as distinct values: it practically never finishes. *Verified:* antsApplyTransforms was still running after 9 minutes on a boldref → 1 mm T1w grid and was stopped. larmorx only evaluates the labels among the 8 neighbours (the others are 0 and cannot win), with the same result. |
| `*WindowedSinc` | `WindowedSincInterpolateImageFunction<…, 3, Window, ConstantBoundaryCondition>` | Radius 3: 6 taps per axis (offsets −2…3 around `floor(x)`). Zero outside the image (ANTs' `ResampleImage` keeps ITK's default boundary instead, the nearest edge voxel: [ants-gaussian-filters.md](ants-gaussian-filters.md#resampleimage-examplesresampleimagecxx)). A delta when the distance is exactly 0. Windows use `π/3` scaling, e.g. Lanczos `sin(z)/z` with `z = π·x/3`. |

*Read; ported in `larmorx-interp`. Parity per interpolator is in the antsApplyTransforms
validation record.*
