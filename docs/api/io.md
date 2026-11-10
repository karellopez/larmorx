# Images and NIfTI I/O

`lx.Image`, `lx.load` and `lx.save` (module `larmorx.io`; Rust crates `larmorx-core` and `larmorx-io`).

**Status: `validated`** against nibabel 5.4.2 (see the [validation record](../validation/nifti-io.md)): bit-identical data in every read mode (including memory-mapped), identical header fields and extensions, affines within 1e-9 mm, and the same header on write, on every file of the test-data catalog.

## Quick start

```python
import larmorx as lx

img = lx.load("sub-01_bold.nii.gz")          # lx.Image(data, affine, header)
img.data.shape, img.affine, img.tr            # (64, 64, 36, 240), 4x4 RAS+ mm, 2.0
mean = img.with_data(img.data.mean(axis=3))   # same affine and header metadata
lx.save(mean, "mean.nii.gz", n_threads=8)     # parallel gzip; identical bytes for any n_threads
```

## `lx.Image`

A frozen dataclass:

| Field | Meaning |
|---|---|
| `data` | numpy array, indexed `[i, j, k, ...]` (arrays read from files are Fortran-ordered, as in nibabel) |
| `affine` | 4×4 float64, voxel indices → RAS+ millimetres (read-only array) |
| `header` | the `NiftiHeader` the image was read with, or `None`; carries units, TR, slice timing, intent, descriptions and extensions |

Properties: `shape`, `ndim`, `dtype`, `voxel_sizes`, `tr`. Methods: `with_data(data)`, `replace(**fields)`, `save(path, **options)`, `from_nibabel(img)`, `to_nibabel()`. `lx.as_image(x)` accepts an `Image`, a path, a nibabel image or an `(array, affine)` pair.

## `lx.load(path, *, dtype=None, scaled=True, mmap=False, n_threads=1)`

Reads `.nii`, `.nii.gz`, `.hdr`/`.img` pairs (either name), `.hdr.gz`/`.img.gz`; NIfTI-1 and NIfTI-2; either byte order.

| `dtype`, `scaled` | Result | nibabel equivalent |
|---|---|---|
| `None`, `True` (default) | stored type if the header does not scale the data; float64 (complex128) if it does | `np.asanyarray(img.dataobj)` |
| `np.float64` | float64, scaled | `img.get_fdata()` |
| `np.float32` | float32, scaled | `img.get_fdata(dtype=np.float32)` |
| `None`, `False` | stored values, `scl_slope`/`scl_inter` ignored | `img.dataobj.get_unscaled()` |

`mmap=True` memory-maps the voxel data instead of reading it (copy-on-write, like nibabel's default `mmap='c'`) when the file allows it: uncompressed, native byte order, and no scaling (or `scaled=False`). Other files are read normally. Useful for large uncompressed images of which only part is needed.

Header fields that nibabel fixes when it reads a file (zero or negative voxel sizes, a `qfac` other than ±1, unknown xform codes, a wrong `bitpix`) are fixed the same way and logged on the `larmorx.io` logger. `lx.io.read_header(path)` returns the header exactly as stored, without fixes.

## `lx.save(image, path, *, dtype=None, version=None, compression_level=2, n_threads=1)`

| Option | Meaning |
|---|---|
| `dtype` | store as this type (`data.astype(dtype)`); by default the array's own type, never rescaled |
| `version` | 1 or 2; by default the header's version, or NIfTI-1 when the shape fits |
| `compression_level` | gzip level for `.gz` names, 0–9. The default 2 gives the size nibabel's level 1 gives (see below) |
| `n_threads` | compression threads (0 = all CPUs); the output bytes do not depend on it |

The header is the image's header, updated as nibabel updates it: the qform/sform and their codes are kept when the image affine is still `allclose` to the header's, and replaced by the affine (sform code 2 "aligned", qform code 0 "unknown") otherwise. Without a header, the result is the header nibabel writes for a new `Nifti1Image(data, affine)`.

## `lx.io.save_nibabel(image, path, *, dtype=None, version=None, compression_level=2, scale=True, n_threads=1)`

Writes the image as nibabel's `img.to_filename(path)` writes it, for ports of Python programs
that write through nibabel (sdcflows' interfaces, fMRIPrep's own steps): the voxel data are
stored in the **header's** data type (`dtype` overrides it; without a header, the array's
type), and float data that do not fit an integer type are scaled by nibabel's rules, with the
`scl_slope` and `scl_inter` nibabel writes. The header is the one `lx.save` writes (nibabel's
`update_header`). The same options as `lx.save`. `scale=False` is nibabel's path for a header
whose `scl_slope` and `scl_inter` are set (a loaded image's are NaN; nilearn's `new_img_like`
sets them to 0): no scaling is computed, the values are cast as `ArrayWriter(check_scaling=False)`
casts them, and the header's `scl_slope` and `scl_inter` are written as they are.

| nibabel 5.4.2 | larmorx |
|---|---|
| `nb.Nifti1Image(data, affine, header).to_filename(p)` | `lx.io.save_nibabel(lx.Image(data, affine, header), p)` |
| `ArrayWriter` / `SlopeArrayWriter` / `SlopeInterArrayWriter` (`arraywriters.make_array_writer`): the slope and intercept of a float-to-integer scaling, NaN to 0, infinities clipped | the same rules, the same values |
| `volumeutils.array_to_file` (the stored values: `rint((x - inter) / slope)` clipped) | `lx.io.nibabel_stored(data, dtype)` gives `(stored, slope, intercept)` without writing |
| `WriterError`, `ScalingError`, `HeaderDataError` | `lx.io.WriterError`, `lx.io.ScalingError`, `lx.io.HeaderDataError` (raised where nibabel raises them) |

nibabel computes the slope and intercept in `np.longdouble` (`best_float()`), which is x87's
80-bit extended type on x86-64 Linux and float64 on the other platforms, so nibabel itself
writes other scale factors on macOS, Windows and Linux aarch64. larmorx computes those few
scalar operations in exact rational arithmetic rounded to a 64-bit significand, so every
platform writes the scale factors nibabel writes on x86-64 Linux. Checked against nibabel
5.4.2 on 60,000 extended-precision operations and 782 writes (every integer type, NaN,
infinities, constant and empty arrays: `validation/investigations/sdcflows-fieldmap/
check_nibabel_write.py`), byte for byte after decompression.

## `lx.io.load_nibabel(path, *, dtype=None, scaled=True, n_threads=1)`, `lx.io.nibabel_affine(header)`

`lx.load` with the affine **nibabel** reads from the header (`nb.load(path).affine`,
`header.get_best_affine()`), for ports of Python programs that read through nibabel: the sform
when `sform_code > 0`; the qform when `qform_code > 0`, which nibabel builds in `np.longdouble`
(`quaternions.fillpositive` and `quat2mat`, then `R @ diag(pixdim)`): x87's extended format on
x86-64 Linux, emulated exactly (rational arithmetic rounded to 64-bit significands, then once
to float64), so every platform gets Linux's bits; else nibabel's base affine
(`shape_zoom_affine`, `-0.0` translations of the singleton axes included).
`lx.io.nibabel_affine(header)` is that affine for any header. `lx.load`'s own qform is float64
arithmetic, the same on every platform but not nibabel's in the last bit or a zero's sign.

The headers these ports write store the qform as nibabel's `set_qform` does
(`lx.io.save_nibabel`, and `lx.io.nibabel_header.header_for_image` / `set_xform`): the voxel
sizes `√((a0² + a1²) + a2²)`, `qfac` from `numpy.linalg.det`, the closest rotation from
`numpy.linalg.svd` and the quaternion from `numpy.linalg.eigh` (`mat2quat`), with numpy's
linear algebra replicated operation for operation (`larmorx_transform::numpy_linalg`: LAPACK's
`dgesdd` and `dsyevd` paths for these sizes on OpenBLAS's Haswell kernels; checked on 4,048
matrices against numpy 2.5.4 and 2.3.5, `validation/investigations/sdcflows-fieldmap/
check_numpy_linalg.py`). numpy 2.5.4's OpenBLAS 0.3.34 bundles LAPACK 3.12.1 and numpy 2.3.5's
0.3.30 LAPACK 3.12.0, which apply a Householder reflector differently (`DLARF1F`, `DLARF`): the
float32 quaternion of about 2 % of oblique affines differs between the two in its last bit.
larmorx follows 3.12.1; `lx.io.nibabel_header.use_lapack("3.12.0")` selects the older release
(for validation against numpy 2.3.5). `lx.io.nibabel_header.io_orientation` is nibabel's
`io_orientation` with the same SVD (the axis a rotation with two equal candidates picks).
`lx.save` and `lx.load` keep larmorx's own arithmetic.

## From nibabel to larmorx

| nibabel | larmorx |
|---|---|
| `nib.load(p)` then `img.dataobj` / `img.get_fdata()` | `lx.load(p)` / `lx.load(p, dtype=np.float64)` |
| `img.affine`, `img.header` | `img.affine`, `img.header` (a typed dataclass of every header field) |
| `nib.Nifti1Image(data, affine, header)` | `lx.Image(data, affine, header)` or `img.with_data(data)` |
| `nib.save(img, p)` | `lx.save(img, p)`; `lx.io.save_nibabel(img, p)` for nibabel's scaling into the header's type |
| `img.header.get_zooms()`, `get_xyzt_units()` | `img.header.zooms`, `spatial_unit` / `time_unit`, `img.tr` |
| `img.header.get_qform()`, `get_sform()`, `get_slope_inter()` | `img.header.qform`, `sform`, `slope_inter` |

## Deliberate differences from nibabel

| Situation | nibabel | larmorx | Why |
|---|---|---|---|
| Byte order of returned arrays | the file's | native | native arrays are faster in numpy and required by the Rust kernels; values are identical |
| Saving a float array with an integer header type | rescales into the header's type | `lx.save` stores the array's type (use `dtype=` to convert); `lx.io.save_nibabel` rescales as nibabel does | no silent precision loss by default |
| Default gzip level | 1 (zlib) | 2 (zlib-rs) | zlib-rs's level 1 is a much faster strategy that compresses ~15% less; its level 2 matches zlib's level-1 size and is still faster |
| Gzip detection | by file name | by the file's first bytes | a gzipped `.nii` (or a plain `.nii.gz`) is read instead of failing |
| Single file with `vox_offset` 0 | reads the header bytes as voxel data | error | the data would be garbage |
| `get_fdata` on complex data | drops the imaginary part | error | no silent data loss |
| RGB24/RGBA32 data | structured arrays | not supported yet | planned |

`lx.load` keeps NaN and infinities as they are, as nibabel does. The ANTs programs' reader
(`larmorx ants ...` and the `lx.ants` functions given a file name) reads them as 0, as ITK's
NIfTI reader (nifti_clib) does for every ANTs user: an image that marks missing data with NaN is
read with 0 there. This is kept in both modes by decision (ITK-06, rule C, 2026-10-23: no fMRIPrep
input has such a value, and keeping them would spread one NaN through every smoothing step;
[register](../findings/upstream-defects.md),
[investigation](../../validation/investigations/itk-06-nonfinite/README.md)).

## Performance

From the [benchmark report](../benchmarks/nifti-io.md) (real data from 2 MB templates to a 1.4 GB 7 T BOLD run; median times, warm OS cache):

| Operation | vs nibabel | vs SimpleITK |
|---|---|---|
| Read `.nii.gz` | 1.1–2.4× faster | 1.1–1.6× faster |
| Read `.nii` | 1.1–1.5× faster | 2–3.8× faster |
| Write `.nii.gz`, 1 thread | 1.3–1.9× faster | – |
| Write `.nii.gz`, all threads | 4.4–7.6× faster | 4–9× faster |
| Write `.nii.gz` at gzip level 6 | 5.8–12.4× faster | – |
| Write `.nii` | about the same (disk-bound) | about the same |

Reading a gzip stream is inherently sequential; the gains come from zlib-rs's inflate and from decompressing straight into the final array. Writing compresses in parallel, with output bytes that do not depend on the thread count.
