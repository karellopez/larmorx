# ANTs' morphology and mask operations: ImageMath MD, ME, MO, MC, GD, GE, GO, GC, FillHoles, PadImage (ANTs v2.6.5, ITK v5.4.5)

What these operations and the ITK filters behind them really compute, found while porting them
(milestone A2, morphology group). Line numbers are at the tags `ANTs-v2.6.5` and
`ITK-v5.4.5`. The parity record is [ants-image-math](../validation/ants-image-math.md)
(categories `binary-morphology`, `grayscale-morphology`, `fill-holes`, `pad-image`). How
ImageMath reads and writes images is in [ants-image-programs.md](ants-image-programs.md).

## `MorphImage` and `ants::Morphological`

**All eight operations go through one function, `ants::Morphological(image, rad, option,
value)`** (`Examples/antsUtilities.h:72-238`), called by `MorphImage`
(`ImageMath_Templates.hxx:8221-8288`) on a `float` image:
- `ImageMath d out op image [rad=1] [value=1]`: both read with `atof`, the radius as a
  `float` and then `static_cast<unsigned long>(rad)` (`antsUtilities.h:133`): `2.9` is 2,
  `-0.5` is 0, and `-1` becomes a radius near 2^64, for which ITK cannot allocate the
  `(2r+1)^D` structuring element (`std::bad_alloc`, abort).
- The structuring element is the same for all eight: `BinaryBallStructuringElement<float, D>`
  with `SetRadius(r)` on every axis (radius in **voxels**, not mm) and
  `CreateStructuringElement()`; `SetRadiusIsParametric` is never called.
- The output is written only for names **longer than 3 characters**
  (`ImageMath_Templates.hxx:8281`), and it is the filter's output: no `descrip`.
- The grayscale operations ignore `value`.
*Read; validated.* *larmorx:* `larmorx_ants::image_math::morphological`; radii above 65535
voxels and the wrapped ones are refused with a message.

## ITK's ball

**`BinaryBallStructuringElement` is `FlatStructuringElement::Ball`**
(`Modules/Filtering/MathematicalMorphology/include/itkBinaryBallStructuringElement.hxx:29-36`,
`itkFlatStructuringElement.hxx:913-1013`): an `EllipsoidInteriorExteriorSpatialFunction` with
axes `2r + 1` (the neighbourhood's size) centred at `r + 0.5`, flood-filled from the centre
with the "center" inclusion strategy, which evaluates each voxel at `index + 0.5`
(`itkFloodFilledSpatialFunctionConditionalConstIterator.hxx`, case 1). So a voxel at offset
`o` from the centre is in the ball when `Σ (o_i / (r + 0.5))² ≤ 1`, computed in double. The
offsets are integers, so this is exactly `Σ o_i² ≤ r(r + 1)`: the two sides of the bound are
at least `0.25 / (r + 0.5)²` apart, far above the rounding of the double computation. The
ball is convex and symmetric, so the flood fill reaches all of it.
- 3D radius 1: the 19 voxels of the 3 × 3 × 3 cube without its corners; 2D radius 1: the
  whole 3 × 3 square; radius 0: the centre only.
- In 4D the ball is four-dimensional: `ImageMath 4 MD` dilates across time too.
*Read; verified* (unit test against the formula for radii up to 200 in 1D, 60 in 2D, 24 in
3D, 8 in 4D, and at the bound for radii up to 65535); *validated* (`MD-dim2`, `MD-dim4`).
*larmorx:* `larmorx_image::morphology::ball_contains`, `ball_squared_radius`.

## Binary dilation and erosion

**`BinaryDilateImageFilter` paints the structuring element along the object's border**
(`itkBinaryDilateImageFilter.hxx:39-457`, `itkBinaryMorphologyImageFilter.hxx`): it splits the
element into connected components, keeps one vector per component, traces the border of the
foreground (voxels with a background voxel among their 3^D neighbours, outside the image
counting as background), paints the element at every border voxel (the whole element at the
first voxel of each border component, then only the "difference set" in the direction it
moves), and finally paints the foreground translated by each component vector. For the
connected ball this is exactly the Minkowski sum: a voxel becomes foreground when a
foreground voxel of the image lies within the ball around it. The values:
- the output starts as the input with **foreground voxels replaced by the background value**
  (`BackgroundValue`, by default `NumericTraits<float>::NonpositiveMin()`, `-FLT_MAX`;
  `.hxx:93-104`), then painted voxels become the foreground. The ball always holds its
  centre, so no voxel keeps `-FLT_MAX`;
- `BoundaryToForeground` is off for dilation: outside the image is not foreground.

**`BinaryErodeImageFilter` does the same on the complement** (`itkBinaryErodeImageFilter.hxx`):
- the output starts as all foreground; voxels within reach of a voxel that is **not** the
  foreground become the background value (`-FLT_MAX`); then every voxel that is the
  background value and was not foreground in the input takes its input value back
  (`.hxx:452-466`);
- `BoundaryToForeground` is on: outside the image counts as foreground, so **the image border
  does not erode** (a mask touching the edge keeps its edge voxels).

**ANTs' `ME` then thresholds**: `1` where the eroded value and the input are both above 0.5,
else `0` (`antsUtilities.h:228-247`). So `ME`'s output is always 0/1, and **a voxel that is
not the foreground value but above 0.5 stays 1**: on a label map `ME 1` erodes label 1 and
leaves labels 2 and 3 untouched (as 1). sMRIPrep's brain extraction calls `ME` on binary
masks (`ME 10` on the CSF mask, `ME 2` and `ME 5` on the brain mask), where this does not
arise.
*Read; validated* (`ME-labels`, `ME-value-2`, `ME-fractional`, `ME-components` with an
object on the border).

**larmorx computes the same sets with an exact distance transform.** A voxel is within the
ball of a foreground voxel exactly when the squared index distance to the nearest foreground
voxel is at most `r(r + 1)`. `larmorx_image::morphology::within_distance` computes that
distance separably in integers (Felzenszwalb and Huttenlocher's lower envelope of parabolas,
the breakpoints compared as exact rationals, capped at `r(r + 1) + 1`), in parallel over
lines. The cost does not grow with the radius: `ME 10` on a 1 mm brain mask takes 0.12 s
against 0.90 s for ANTs.

## Binary opening and closing

**`BinaryMorphologicalOpeningImageFilter`** (`itkBinaryMorphologicalOpeningImageFilter.hxx`)
erodes with the background value set to its own `BackgroundValue`, **0 by default**
(`.hxx:40, 63`), then dilates with the dilation's default background. ANTs sets only the
foreground. A consequence: with `value` 0 (`MO r 0`), eroded foreground voxels become 0, the
foreground itself, so the erosion changes nothing and **the opening is a plain dilation of
the zeros**. *Read; validated* (`MO-value-0`).

**`BinaryMorphologicalClosingImageFilter`** (`itkBinaryMorphologicalClosingImageFilter.hxx`)
has `SafeBorder` on (`.hxx:43`): the input is padded by the radius on every side with 0 (with
`FLT_MAX` if the foreground is 0, `.hxx:58-62`), dilated, eroded with that pad value as
background, and cropped; finally **every voxel that is not the foreground takes its input
value** (`.hxx:142-151`). So an object near the image border closes as if the image went on
with background, and the closing never removes a voxel. *Read; validated* (`MC-*`, including
`MC-value-0` and an object on the border).

*larmorx:* `binary_opening`, `binary_closing`, step by step with ITK's values (the padding
included).

## Grayscale morphology

**`GrayscaleDilateImageFilter` / `GrayscaleErodeImageFilter`** take the maximum / minimum over
the ball, voxels outside the image counting as the boundary value (`NonpositiveMin` for
dilation, `max` for erosion; `itkGrayscaleDilateImageFilter.hxx:36`,
`itkGrayscaleErodeImageFilter.hxx:36`). ITK picks an algorithm in `SetKernel`
(`.hxx:58-89`): the ball is not a decomposable `FlatStructuringElement`, so it is either the
direct scan (`BasicDilateImageFilter`, in 2D and 3D when the kernel is small compared with
the pixels a moving histogram adds per step) or the moving histogram
(`MovingHistogramDilateImageFilter`, a `std::map` per thread). Both give the extreme value.
**The opening and closing pad the image by the radius** (`SafeBorder`): with `max` before
eroding then dilating (opening), with `NonpositiveMin` before dilating then eroding
(closing), and crop (`itkGrayscaleMorphologicalOpeningImageFilter.hxx:139-160`,
`itkGrayscaleMorphologicalClosingImageFilter.hxx:142-163`). *Read; validated*
(`grayscale-morphology`: the phantom, outliers, 2D, 4D, the raw T1w).

**The sign of a zero extreme depends on ITK's algorithm.** The moving histogram is a
`std::map<float, count, std::greater<float>>` (`itkMorphologyHistogram.h`): `-0.0` and `+0.0`
compare equal and share one entry, whose key is whichever was inserted first and stays until
the entry is erased when it empties at the front. So where a neighbourhood holds both signed
zeros and its maximum is zero, the output's sign depends on the order the histogram was
filled in (and so on ITK's thread split). On an image of `-0.0`, `+0.0`, `-1`, `-2`, `GD 1`
returns `-0.0` for about half of those voxels. *Verified* (`GD-signed-zeros`,
`GC-signed-zeros`: up to 820 of 1680 values differ, all in the sign of zero only).
*larmorx:* compares values as the integers of `f32::total_cmp`, so a zero maximum is `+0.0`
and a zero minimum `-0.0`; the parity cases declare a tolerance of 0 ulp with this reason.
Images read from NIfTI rarely hold `-0.0`; it appears after multiplying negative values by 0.

**larmorx's algorithm:** the ball split into runs along the first axis. For every half-width
`w ≤ r`, the running maximum over `[x − w, x + w]` is computed once per row (from the one for
`w − 1`), and each output row takes the maximum over the ball's rows of the matching run
maxima; voxels whose ball reaches outside the image also take the boundary value. Erosion is
the same on complemented keys. Exact, and `GD 2` on a 1 mm T1w takes 0.17 s against 0.97 s.

## `FillHoles`

**`FillHoles`** (`ImageMath_Templates.hxx:8880-9033`):
1. the object is the voxels in `[0.5, 1e9]` (`BinaryThreshold(0.5, 1.e9, 1)`, in float);
2. a Danielsson distance map of it (`InputIsBinaryOff`, unit spacing) thresholded at
   `> 0.001`: **this is exactly the background** (object voxels have distance 0, every other
   voxel at least 1 voxel, and an image with no object gets distances near `2·max size`), so
   larmorx skips the distance map, the slowest part of ANTs' implementation (2.8 s on a 1 mm
   mask, against 0.08 s for larmorx);
3. the background's **face-connected** components (`ConnectedComponentImageFilter`,
   `FullyConnected` off), relabelled by size (`RelabelComponentImageFilter`, minimum size 0):
   label 1 is the largest region, normally the outside; the others are holes;
4. `holeparam` (`atof`, default 2):
   - within `FloatAlmostEqual` of 2 (4 ulps): **every voxel with a label above 1 becomes 1**
     in the image read from the file (which keeps its `descrip`; values other than 0 and 1
     stay);
   - `≤ 1`: for each hole, `objectedge` and `totaledge` count, over the 3^D neighbourhood of
     every voxel of the hole (centre included, with repeats), the neighbours outside the
     hole that are object and that are object or background; the hole is filled when
     `float(objectedge) / float(totaledge) > holeparam` (so `1` fills nothing; `0.99` fills
     holes bounded by the object only);
   - otherwise the ratio is 2: holes are filled if `holeparam < 2`, none above 2.
*Read; validated* (`fill-holes`: every branch, labels, fractional masks, 2D and 4D, real
masks).

**The `holeparam ≤ 1` branch reads outside the image.** It walks the relabelled image with a
`NeighborhoodIterator` (zero-flux boundary for its own values) but reads the binary image at
`GHood.GetIndex(i)`, the unclamped neighbour index (`ImageMath_Templates.hxx:8994`), with
`Image::GetPixel`, which does not check bounds. Indices one row past an edge land on the
neighbouring row (the linear offset wraps); on the first or last slice they land before or
after the image's buffer, and the result depends on whatever memory is there. *Read;
verified* (a hole on the first slice: ANTs exits 0 with some output).
*larmorx:* reproduces the reads that wrap inside the buffer and refuses the others with a
message (`fill-holes/hole-first-slice-0.5`, an expected divergence). fMRIPrep and sMRIPrep
only call `FillHoles 2`.

## `PadImage`

**`PadImage`** (`ImageMath_Templates.hxx:1955-2048`): `ImageMath d out PadImage image pad
[value=0]`, `pad` and `value` read with `atof` as float.
- **The pad argument is required**: without it ANTs passes `argv[5]` (null) to `atof` and
  crashes. Names of 3 characters or fewer are not read (a null image, a crash).
- Each new size is `(unsigned int)(float(size) + pad·2)`, so a fractional `pad` of 2.5 adds 5
  voxels; the image is copied to `(unsigned int)(float(i) + pad)`, a shift of 2. A size of 0
  (`pad = −size/2`) or a negative one (which wraps to about 2^32) crashes ANTs
  (`munmap_chunk(): invalid pointer`, SIGSEGV).
- The new origin is `origin + (P(index) − P'(index2))`, with `index = 0` and
  `index2 = |pad|` (truncated) for a positive pad, the reverse for a negative one, and `P`
  ITK's `TransformIndexToPhysicalPoint` (`origin + M·index`, `M = Direction·diag(spacing)`
  summed as vnl's fixed matrix product). So voxels keep their position in space.
- The output is a new image (`AllocImage`): no `descrip`; the padding value fills it, and a
  4D image is padded along time too.
*Read; validated* (`pad-image`: fractional, negative, oblique, LAS, negative-pixdim, 2D, 4D,
fMRIPrep's ±10 on real images; every header byte identical).
*larmorx:* `larmorx_ants::image_math::pad_image`, with the same float arithmetic for the
sizes, shifts and origin.

## Connected components and relabelling (used by `FillHoles`)

**`ConnectedComponentImageFilter` numbers components in the order of their first voxel in
memory** (`Modules/Segmentation/ConnectedComponents/include/itkConnectedComponentImageFilter.hxx`,
`Modules/Filtering/ImageLabel/include/itkScanlineFilterCommon.h`): each row along the first
axis is run-length encoded, runs get consecutive numbers in memory order (`InitUnion`),
overlapping runs of neighbouring rows are linked to the smaller root (`LinkLabels`), and
`CreateConsecutive` numbers the roots in increasing order; the root of a component is its
first run. Face connectivity joins runs that overlap along x in rows one step apart along one
axis; full connectivity also joins rows that are diagonal neighbours and runs that touch at a
corner (the run is extended by one voxel). The result does not depend on ITK's threads.
*Read; validated* (through `FillHoles`).

**`RelabelComponentImageFilter` sorts by size, largest first, ties by label**
(`itkRelabelComponentImageFilter.hxx`, `GenerateData`: the comparator is
`a.size > b.size || (a.size == b.size && a.label < b.label)`), drops objects smaller than
`MinimumObjectSize` when that is positive, and numbers the rest from 1. **The new labels are
counted in the output pixel type**: with a `float` output (as in ANTs) `outputLabel + 1` stops
growing at 2^24. *Read.* *larmorx:* `larmorx_image::components::{connected_components,
relabel_components}` (union-find over runs; labels by first run), and
`larmorx_ants::image_math::float_labels` for the float labels.
