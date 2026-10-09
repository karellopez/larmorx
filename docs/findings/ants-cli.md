# ANTs command lines (ANTs v2.6.5)

## The argument parser (`Utilities/antsCommandLineParser.cxx`, `antsCommandLineOption.cxx`)

**Repeated options come back last first.** `AddFunction` pushes each value to the *front*
of the option's list (`antsCommandLineOption.cxx:42,64`). `GetFunction(0)` is therefore the
last occurrence on the command line, and that is what every single-valued option reads.
*Read.*
*larmorx:* `cli::parser` keeps values in command-line order. `Parsed::last()` is
`GetFunction(0)`.

**Brackets are rewritten only at the ends of a word.** `RegroupCommandLineArguments`
(`antsCommandLineParser.cxx:201`) replaces `{ ( <` only as a word's first character and
`} ) >` only as its last. So `Gaussian(1x2x3,2)` becomes `Gaussian(1x2x3,2]`, which ANTs
rejects ("Incorrect command line specification"). `[a.mat, 1]` split across two words is
joined back. A word with text after its first `]` is an error. *Read.*
*larmorx:* reproduced, including the errors (`parser.rs` tests).

**Short options take two characters.** `-x...` gives the name `argument.substr(1, 2)`, so
`-tf` is looked up as the long option `tf`. *Read.* *larmorx:* reproduced.

**Negative numbers are values, not options.** A following word ends an option's values when
it starts with `-` and `atof(word) == 0`. So `-f -0.5` passes `-0.5` to `-f`, while `-0`
would end it. An option with no value gets `"1"` (`-v` alone means verbose on).
*Read.* *larmorx:* reproduced (`parser::atof` mirrors C `atof`).

**Values are `name[p1,p2,...]`.** The parameters are split at every comma up to the first
`]`, without nesting. *Read.*

**`ConvertVector` splits on `x`** (`antsCommandLineParser.h:142`): `1x2x3` gives
`[1, 2, 3]`. `Convert<bool>` reads an integer, so `true` is not true. *Read.*

## `antsApplyTransforms` (`Examples/antsApplyTransforms.cxx`)

**The `-t` order.** For points of the output grid, the *first* `-t` acts first:
`-t A -t B` maps an output point `x` to `B(A(x))`. Seen as moving an image, the order
reverses: the image moves by `B` first, then `A`. That is the "last transform is applied
first" of the ANTs documentation, and why `-t warp.nii.gz -t affine.mat` is the usual order.
The mechanism: the parser returns the options last first, `GetCompositeTransformFromParserOption`
(`itkantsRegistrationHelper.h:1281-1440`) adds them to an ITK `CompositeTransform` in that
order, and the composite applies its queue back to front.
*Verified:* with `-t scale2.mat -t translate+1.mat` on a ramp image, output voxel 3 reads
input position 7 = 2·3 + 1.
*larmorx:* `TransformChain` stores the same queue. An earlier draft had the order reversed;
the oracle run caught it.

**Composite files keep their own order.** A `CompositeTransform` read from a file (`.h5`) is
expanded in place, component by component, in the file's queue order
(`itkantsRegistrationHelper.h:1438-1450`). *Read.* *larmorx:* `TransformChain::new`
flattens the same way, and a unit test pins the resulting mapping.

**Inverting** (`-t [file,1]`) calls `GetInverseTransform()` (`itkantsRegistrationHelper.h:1427`). For a displacement field
without an inverse field that returns null, and the run fails with "Inverse does not exist".
*Read.* *larmorx:* `TransformError::NotInvertible`.

**`-t identity`** (or `Identity`) adds an identity `MatrixOffsetTransformBase`. With no
`-t` at all, ANTs adds one too. *Read.* *larmorx:* an empty chain, which maps points
exactly the same way.

**Initializer transforms** `-t [fixed,moving,feature]` (three or more parameters) build a
rigid transform from the images. *Read.* *larmorx:* not supported yet (error).

**Pixel type.** Everything is computed in `double`, or in `float` with `--float 1`. The
input is read straight into that type, so integer data are converted on reading. The output
type is the computation type unless `-u` says otherwise. *Read.*
*larmorx:* double throughout. `--float` rounds the input to float32 and writes float32, but
the arithmetic stays double, so float results can differ in the last float bits.

**`-u` casts with `CastImageFilter`, a plain `static_cast`.** Integers truncate toward zero.
An unknown `-u` name silently falls back to the default type
(`antsApplyTransforms.cxx:1453-1600`). *Read.*

**Out-of-range values wrap on x86-64.** C++ leaves the cast of an out-of-range double to a
small integer undefined. x86-64 builds convert through a 32-bit integer (`cvttsd2si`: NaN and
values beyond ±2³¹ give `INT_MIN`) and keep the low bits. So `-u uchar` turns 400.7 into 144
and −3.7 into 253, and `-u char` wraps too. ARM builds (`fcvtzs`) saturate instead, so ANTs
gives different results on different hardware. *Verified* on x86-64: an `-u uchar` and an
`-u char` output both match the int32 wrap exactly, and neither matches saturation.
*larmorx:* the CLI reproduces x86-64 on every platform. `lx.ants.apply_transforms(dtype=...)`
saturates instead, which is safer, and says so.

**`-e 0` with a 4D file fails.** v2.6.5 (`antsApplyTransforms.cxx:355-375`) checks the file's number of dimensions against
`-d` and exits ("Input image dimension does not match"). It also rejects non-scalar pixel
types. *Read.* *larmorx:* same errors.

**`-e 3` (time series).** The 4D image is cut into 3D volumes with
`ExtractImageFilter::SetDirectionCollapseToSubmatrix` (`antsApplyTransforms.cxx:526`), so each volume takes the top-left 3×3
of the 4D direction. Each volume is resampled with the same transforms. The output takes
its first three axes from the reference and the fourth axis from the input (spacing, origin
and the rest via `CopyInformation`). `--time-index t` extracts a single volume instead and
writes a 3D image. *Read.*
*larmorx:* the same. The continuous indices are computed once and reused for every volume,
which gives the same numbers ITK computes per volume.

**Gaussian and MultiLabel sigmas default to the input image's spacing**
(`make_interpolator_snip.tmpl`). A sigma parameter of `s` applies to every axis; `sxsxs`
gives one per axis. **MultiLabel ignores its alpha parameter.** Alpha is always 4.0: the
snippet has no branch for a second parameter (`make_interpolator_snip.tmpl:127`). **GenericLabel ignores its parameter.** It
always uses linear interpolation. Gaussian alpha defaults to 1.0. `BlackmanWindowedSinc` is
accepted although the help does not list it. *Read.* *larmorx:* reproduced.

**`-o` forms.**
- `file` or `[file,0]` writes the warped image.
- `[file,1]` or `DisplacementField[file]` writes the composed displacement field.
- `Linear[file,<invert>]` writes the collapsed affine.
- `CompositeTransform[file]` writes the composite.
- `[file]` with a single parameter gives an empty file name (`antsApplyTransforms.cxx:762-771`).

*Read.* *larmorx:* only the image outputs so far. The others report "not supported yet".

**Threads.** ITK uses all cores unless `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` is set.
*larmorx's CLI:* the same environment variable. The result does not depend on the thread
count, in ANTs or in larmorx.
