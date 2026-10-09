# ANTsPy as an oracle (ANTsPy 0.6.3)

ANTsPy 0.6.3 bundles ITK 5.4.5 (`f51594ad`) and ANTs v2.6.5 plus 7 later commits, all of
them about vector-image I/O. It is our in-process oracle for ANTs.

**`ants.internal.get_lib_fn('antsApplyTransforms')(args)` runs the real ANTs command
line** with file arguments, the same code path as the `antsApplyTransforms` binary. The
parity suite uses this for exact comparisons. *Verified.*

**`ants.image_read` reads as float32 by default** (`pixeltype='float'`). Whatever is stored
on disk, ANTsPy images are float32 unless another type is asked for, so in-memory ANTsPy
results start from float32 data. *Read.*

**`ants.apply_transforms` quirks** (`ants/registration/apply_transforms.py`):
- **`whichtoinvert` defaults to `(True, False)`** when the list is exactly two transforms,
  the first containing `.mat` and the second not. ANTsPy then inverts the affine without
  being asked. Otherwise nothing is inverted. *Read.*
- It passes `--float 0` (double) unless `singleprecision=True`, clones fixed and moving to
  that type, and adds `-z 1`, `-e imagetype` and `-f defaultvalue`. *Read.*
- It **raises** for a 4D moving image with `imagetype=0`. *Read.*

*larmorx:* `lx.ants.apply_transforms` inverts nothing unless asked. This deliberately does
not copy ANTsPy's guess.

**`ants.image_header_info` is imprecise.** It returns the direction transposed and rounded
to 4 decimals, so it cannot serve as a geometry oracle. The ITK-geometry parity uses
`ants.image_read` instead, with `SimpleITK.ImageFileReader.ReadImageInformation` as a
fallback (negative spacing normalised as `ImageFileReader` does). *Verified.*

**`ants.create_ants_transform` rounds parameters to float32, even with
`precision="double"`.** An Euler angle of 0.1 is written as `0.10000000149011612`. Test
transforms written through it carry float32 values in double transforms. That is harmless
for parity, since both tools read the same file, but it matters if exact parameters are
needed. *Verified* (ANTsPy 0.6.3).

**ANTsPy's `create_ants_transform` cannot make** `TranslationTransform` or
`VersorRigid3DTransform`, the types antsRegistration writes for its translation and rigid
stages. The test data write those in ITK's text format directly. *Verified*
(`create_ants_transform(supported_types=True)`).

**ANTsPy wheels** exist for linux-x64, macOS and win-x64 only, for Python 3.8–3.13. They are
missing for linux-aarch64, win-arm64 and Python 3.14, which is one reason for the port.
