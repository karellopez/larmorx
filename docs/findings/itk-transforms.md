# ITK transforms (ITK v5.4.5)

**`CompositeTransform` applies its queue back to front.** `AddTransform` appends to the
queue, and `TransformPoint` walks it from `rbegin()` (`itkCompositeTransform.hxx:61-72`).
So after `AddTransform(A); AddTransform(B)` a point maps to `A(B(x))`. *Read; verified
through antsApplyTransforms (see [ants-cli.md](ants-cli.md)).*
*larmorx:* `Transform::Composite` and `TransformChain` hold the queue and apply it the same
way.

**A composite counts as linear only when every component is linear**
(`CompositeTransform::GetTransformCategory`, `itkCompositeTransform.hxx:27`). That is what
switches `ResampleImageFilter` to its fast scan-line path. *Read.*
*larmorx:* `TransformChain::is_linear`.

**Matrix-offset transforms** map `y = M·x + offset`. `TransformPoint` computes `M·x` as a
row sum starting from 0, then adds the offset. *Read.* *larmorx:* the same, without fused
multiply-add.

**The offset subtracts the centre terms one at a time.** `ComputeOffset`
(`itkMatrixOffsetTransformBase.hxx:627-643`) computes
`offset[i] = ((t[i] + c[i]) − M[i][0]·c[0]) − M[i][1]·c[1] − M[i][2]·c[2]`. Summing `M·c` first
and subtracting once rounds differently whenever the centre is non-zero, as in every affine
antsRegistration writes. *Verified:* with the summed form, fMRIPrep's T1w → MNI `.h5`
resampling differed from ANTs by up to 3e-11 in 24 % of voxels. With ITK's order the output
is bit-identical. Text affines with a zero centre had matched either way.

**Euler angles compose as `Rz·Rx·Ry`, or `Rz·Ry·Rx` when the fourth fixed parameter is
non-zero (`ComputeZYX`).** *Read.*

**Inverses keep the class and set the matrix directly.** `MatrixOffsetTransformBase::GetInverse`
stores `M⁻¹` and `−M⁻¹·offset` without going back through the transform's parameters
(Euler angles, versor), so the inverse is exactly the inverted matrix. *Read.*

**`itk::Matrix::GetInverse` is an SVD inverse** (`vnl_matrix_inverse`, `itkMatrix.h:277`).
For a signed permutation of a diagonal matrix, which is the index-to-point matrix of every
axis-aligned image, the SVD is exact and each entry `m` becomes exactly `1/m`. A cofactor
inverse can be one ulp off. That ulp decides nearest-neighbour ties when a resampling grid
lands exactly half way between voxels, which is common when downsampling by 2. *Read.*
*larmorx:* `linalg::inverse3_itk` is exact for those matrices and uses the cofactor inverse
otherwise. Oblique matrices can still differ from vnl by a few ulps; the parity reports
measure the effect.

**Displacement fields** (`DisplacementFieldTransform::TransformPoint`) map `x` to `x + d(x)`,
with `d` read by `VectorLinearInterpolateImageFunction`. That is a weighted sum over the 8
neighbours with indices clamped to the grid, in a fixed order, stopping once the weights
reach 1. Outside the field's buffer, more than half a voxel beyond the grid, the point is
returned unchanged. *Read.* *larmorx:* `field.rs`, unit-tested at the edges.

**Field fixed parameters** are 18 numbers: size (3), origin (3), spacing (3), then the
direction (9, row-major). *Read; seen in fMRIPrep `.h5` files.*

## Transform files

**ITK's text format** has `#Insight Transform File V1.0`, then a `Transform:`,
`Parameters:` and `FixedParameters:` line for each transform. The **MATLAB v4 format** pairs
each transform's parameters (variable named after the class) with a variable named `fixed`,
little or big endian, single or double. *Read; round-trip tests.*

**HDF5 (`.h5`) composites** written by ITK, such as fMRIPrep's `*_xfm.h5`, contain
`/TransformGroup/0` with `TransformType = CompositeTransform_float_3_3`, then one group per
component, each with `TransformType`, `TransformParameters` and `TransformFixedParameters`.
Float transforms store their parameters as **float32**, chunked and gzip-compressed. One
1 mm MNI warp is 25.6 M values (≈100 MB as float32). Fixed parameters are float64. *Seen in
the testdata (`ds000005-fmriprep`, written by ITK 5.1.0).* ITK reads float32 parameters
into a double transform exactly.
*larmorx:* `ItkTransformParts::parameters` keeps float32 data as float32
(`Parameters::F32`), so a field is not doubled in memory. Values parsed from text are
double, as ANTs reads them in double mode; an earlier draft wrongly narrowed text
`_float_` fields to float32.
