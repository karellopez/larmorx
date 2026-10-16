# ITK's HDF5 transform files (`.h5`)

`lx.transforms.read`, `lx.transforms.write` with `lx.transforms.Hdf5WriteOptions`,
`lx.transforms.load_itk_composite`, and `-t x.h5` / `-t [x.h5,1]` of
`larmorx ants antsApplyTransforms` (Rust crates `larmorx-hdf5`, a pure-Rust reader and writer
of the HDF5 that ITK uses, and `larmorx-io`, module `larmorx_io::itk_hdf5`). Ported from
ITK 5.4.5's `HDF5TransformIOTemplate` and HDF5 1.12.1, the HDF5 that ITK bundles; no HDF5 C
library and no h5py.

**Status: `validated`** against ITK itself (the ANTs 2.6.5 oracle), h5py and nitransforms (see
the [validation record](../validation/itk-hdf5.md)):
- **reading:** every `.h5` transform file at hand, written by ITK 4.11 to 5.4.5 on HDF5 1.8.17 to
  1.14.6 (fMRIPrep 25.2.6's runs, the test data, TemplateFlow, the oracle) and by h5py in every
  way HDF5 allows: every dataset as h5py reads it, the transforms as ITK reads them (in double
  and float), the chain nitransforms loads, all bit for bit; files ITK refuses are refused
  with ITK's messages;
- **writing:** every file the oracle writes (composites, `-o Linear[...]`, float parameters,
  displacement fields, dense groups, `CompositeTransformUtil`) is written by larmorx byte for
  byte, given ITK's object time and ITK's compressed chunks; ANTs, h5py and nitransforms read
  larmorx's files as ITK's.

How ITK reads and writes these files, and what HDF5 makes of it byte by byte:
[itk-hdf5.md](../findings/itk-hdf5.md).

## Quick start

```python
import larmorx as lx

xfms = lx.transforms.read("sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5")
[t.name for t in xfms]   # ['CompositeTransform_double_3_3', 'AffineTransform_float_3_3',
                         #  'DisplacementFieldTransform_float_3_3']

lx.transforms.write("copy.h5", xfms)                       # as ANTs writes: compressed, time 0
lx.transforms.write("plain.h5", xfms, hdf5=lx.transforms.Hdf5WriteOptions(compression=False))

chain = lx.transforms.load_itk_composite("copy.h5")         # as fMRIPrep (nitransforms) loads it
```

From a shell, `.h5` files are read wherever ANTs takes a transform:

```bash
larmorx ants antsApplyTransforms -d 3 -i T1w.nii.gz -r MNI.nii.gz \
    -t sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5 -o T1w_MNI.nii.gz
larmorx ants antsApplyTransforms -d 3 -i bold_ref.nii.gz -r T1w.nii.gz -t [linear.h5,1] -o out.nii.gz
```

## Reading: `lx.transforms.read(path, *, upstream_defects="fix")`

Returns the transforms stored in the file as `ItkTransform(name, parameters, fixed_parameters)`,
in the file's order (a composite first, then its components). Any HDF5 file with a
`/TransformGroup` is read as one, whatever its name ends with, as ITK does; a `.h5` file that
is not gets ITK's "Could not create Transform IO object" error. Parameters keep their stored
precision: float32 parameters (as float transforms write them, fMRIPrep's warps among them) are
not widened in memory; ITK widens them exactly.

What the reader covers: superblocks 0 to 3; version-1 and version-2 object headers,
continuation chunks; symbol-table, compact and dense groups (fractal heaps with direct,
indirect, huge and tiny objects; version-1 and version-2 B-trees); soft links; compact,
contiguous and chunked datasets with every chunk index of HDF5 1.12 but the extensible array
(version-1 and version-2 B-trees, single chunk, implicit, fixed array); the deflate, shuffle
and Fletcher-32 filters; floating-point data of any size and byte order (half, single, double,
x87 extended, quadruple; converted as HDF5 1.12.1's `H5Dread` converts them to the reader's
precision); fixed- and variable-length strings; fill values. Refused, as ITK's HDF5 refuses them: the
szip filter and filter plugins (LZF), and HDF5 2.0's layout message version 5. **Not covered
yet, though ITK reads them:** the n-bit and scale-offset filters, extensible-array chunk
indices (chunked datasets with an unlimited dimension in the newest file format) and virtual
datasets; no transform file written by ITK, ANTs or fMRIPrep uses them. Chunks are decompressed
in parallel; the result does not depend on the thread count.

On the command line and with `precision="float32"` (`lx.ants.apply_transforms`), the file is
read as ITK's reader of that precision reads it: the precision in a transform's name is
corrected (`AffineTransform_double_3_3` becomes `AffineTransform_float_3_3`), parameters stored
in 8 bytes are read as double and anything else as float.

## Writing: `lx.transforms.write(path, transforms, *, hdf5=None, upstream_defects="fix")`

The format follows the extension, as ITK chooses its writer: HDF5 for `.h5`, `.hdf5`, `.hdf`,
`.h4`, `.hdf4`, `.he4`, `.he5`, `.hd5` (compared case for case), ITK text for `.txt` and
`.tfm`, MATLAB for `.mat`. An HDF5 file takes any transforms: a composite first (its components
follow), linear transforms, displacement fields.

The file is laid out exactly as ITK 5.4.5 on HDF5 1.12.1 lays it out: `/ITKVersion`,
`/HDFVersion`, `/OSName`, `/OSVersion`, then `/TransformGroup/0`, `/1`, … each with
`TransformType` (a variable-length ASCII string), and for every transform but a composite
`TransformFixedParameters` (float64, contiguous) and `TransformParameters` (float32 or float64
as stored). The bytes are HDF5 1.12.1's for the same calls, superblock, object headers,
global heap, chunk B-tree, dense groups and free-space included. Two things differ from a file
ITK writes for the same transforms:

- **Object times.** ITK records the time of writing in every object header (access,
  modification, change and birth); larmorx records `Hdf5WriteOptions.time`, 0 by default, so
  the same transforms always give the same bytes (ITK-09).
- **Compressed chunks.** Both decompress to the same bytes, but larmorx compresses with zlib-rs
  where ITK's HDF5 calls zlib-ng 2.1.6, whose output itself depends on the CPU (its hash is
  CRC-32C with SSE4.2, CRC-32 on Arm, multiplicative elsewhere). Small chunks, such as an
  affine's 96 bytes, come out identical; a large field's chunk can differ by a few bytes, and
  then so do the addresses after it. larmorx's own output is the same on every platform
  (golden case `py.transforms.write_h5`).

A group of 38 or more transforms is the one layout larmorx does not replay: HDF5 gives the
group's fractal heap an indirect block there; larmorx stores the links as link messages,
recording that number of compact links in the group info (`H5Pset_link_phase_change`), which
every reader reads.

### `lx.transforms.Hdf5WriteOptions`

| Field | Default | ITK |
|---|---|---|
| `compression` | `True` | `TransformFileWriter::SetUseCompression`: parameters in chunks of at most 1 Mi values, deflate level 5. ANTs always sets it; ITK's default is `False` (contiguous) |
| `time` | `0` | the time of writing, in seconds since 1970-01-01 UTC |
| `itk_version` | `"5.4.5"` | `/ITKVersion`: ITK's version |
| `hdf_version` | `"HDF5 library version: 1.12.1"` | `/HDFVersion`: `H5_VERS_INFO` |
| `os_name` | `"larmorx"` | `/OSName`: the system's name (`"Linux"`) |
| `os_version` | larmorx's version | `/OSVersion`: the system's release |

### Option mapping

| ITK / ANTs | larmorx |
|---|---|
| `itk::TransformFileReaderTemplate<double>` on `x.h5` | `lx.transforms.read("x.h5")` |
| `itk::ants::ReadTransform` (`-t x.h5`, `-t [x.h5,1]`) | `larmorx ants antsApplyTransforms -t ...`; `lx.ants.apply_transforms(..., ["x.h5"])` |
| `TransformFileReaderTemplate<float>` (`--float 1`) | `precision="float32"`, `--float 1` |
| `TransformFileWriterTemplate<T>` + `SetUseCompression(true)` (`itk::ants::WriteTransform`) | `lx.transforms.write(path, transforms)` |
| the same without compression (ITK's default) | `hdf5=Hdf5WriteOptions(compression=False)` |
| `nitransforms.manip.load(path, fmt="h5")` (fMRIPrep) | `lx.transforms.load_itk_composite(path)` |
| `h5py.File(path)["TransformGroup"]` | `larmorx._core.hdf5_group_members(path, "/TransformGroup")` (internal) |

Rust: `larmorx_io::itk_hdf5::{read, write, WriteOptions}` (ITK's semantics, ANTs' messages in
`ReadFailure::ants_report` / `WriteFailure::ants_report`), `larmorx_io::itk_transform::{
read_itk_transform_parts, write_itk_transform_parts}`; the HDF5 layer `larmorx_hdf5::{File,
write::Writer}`.

## Errors

Files ITK refuses are refused with ITK's reasons, and the command line prints what ANTs prints:
HDF5's error stack (abridged to the call and its cause), then ANTs' "Transform reader for <file>
caught an ITK exception:" and the exception (its addresses as `0x0`, ITK's source tree as
`<ITK source>`). For instance: `TransformType` not a variable-length string ("H5Dread failed
for variable length string"), a gap in the group names ("H5Gopen2 failed"), parameters that
are not floating point or not one-dimensional ("Wrong data type for …", "Wrong # of dims for
TransformType in HDF5 File"), an unknown transform class (the list of the classes ITK knows), a
missing file ("Transform file does not exist: …").

## Upstream defects (D13)

larmorx corrects these by default and warns (`lx.UpstreamDefectWarning`, `upstream_defects=
"reproduce"` or `LARMORX_UPSTREAM_DEFECTS=reproduce` for ITK's behaviour;
[upstream-defects.md](upstream-defects.md)):

| Entry | ITK / nitransforms | larmorx by default | `reproduce` |
|---|---|---|---|
| ITK-12 | a `TransformType` stored as a fixed-length string (h5py's `numpy.bytes_`) cannot be read | reads it | refuses, as ITK |
| ITK-13 | the messages for a parameter dataset of the wrong class or rank name `TransformType` and run the path into "in HDF5 File" | names the dataset | ITK's text |
| ITK-14 | parameters wider than 8 bytes (long double, quadruple) are read through float | reads them in the reader's precision | through float |
| ITK-15 | a transform without parameters (`IdentityTransform`) cannot be written with compression: "H5Pset_chunk failed", and a file ITK cannot read is left | stores its empty parameters uncompressed | fails as ITK, writing the file ITK leaves |
| HDF-01 | HDF5 1.12 converts non-native floats with state carried from one element to the next | converts each element on its own | HDF5's conversion |
| PIPE-04 | nitransforms reads a composite's transforms in name order (`10` before `2`) | numeric order | name order |
| PIPE-17 | nitransforms skips the first group as if it were the composite: a file of one transform fails, a file of several without a composite loses its first, an empty composite fails | reads the first group when it is not a composite; an empty composite is the identity | as nitransforms |

ITK-16 (a null `TransformType` string, which crashes ITK) and ITK-17 (a component of another
dimension, undefined behaviour in ANTs) are refused in both modes.
