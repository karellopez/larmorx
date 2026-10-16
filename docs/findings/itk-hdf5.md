# ITK's HDF5 transform files (ITK v5.4.5, HDF5 1.12.1)

How ITK reads and writes `.h5` transform files (`Modules/IO/TransformHDF5/src/itkHDF5TransformIO.cxx`),
what HDF5 1.12.1 (ITK's bundled copy, `Modules/ThirdParty/HDF5`) makes of the writer's calls,
byte for byte, and what the other readers of these files (h5py, nitransforms) do. Line numbers
are at the pinned tags (`tags/ITK-v5.4.5`, `tags/ANTs-v2.6.5`). *larmorx:* crate
`larmorx-hdf5` (reader and writer), `larmorx_io::itk_hdf5` (ITK's semantics),
`lx.transforms.read`/`write`/`load_itk_composite`; API page
[transforms-hdf5.md](../api/transforms-hdf5.md); validation [itk-hdf5](../validation/itk-hdf5.md).

## Which files exist

| Written by | HDF5 | Layout seen | Where |
|---|---|---|---|
| ITK 5.4.5 (the ANTs 2.6.5 oracle) | 1.12.1 | superblock 2, version-2 object headers with times, compact groups (dense from the ninth link), contiguous fixed parameters, parameters chunked + deflate 5 | `antsApplyTransforms -o CompositeTransform[...]`, `-o Linear[...]`, `-o DisplacementField[x.h5]`, `CompositeTransformUtil --assemble` |
| ITK 5.4.4 (fMRIPrep 25.2.6's image) | 1.14.6 | as above | `*_xfm.h5`, `ants_t1_to_mni*Composite.h5` of the oracle runs |
| ITK 5.1.0 | 1.10.4 | as above | ds000005's fMRIPrep derivatives (test-data catalog) |
| ITK 5.4.0 | 1.14.3 | as above | TemplateFlow's `tpl-MNIInfant_cohort-1_from-MNI152NLin6Asym_mode-image_xfm.h5` |
| ITK 4.11 | 1.8.17 | superblock 0, version-1 object headers, symbol-table groups, misspelt `Tranform…Parameters`, one contiguous float64 field of 25.6 M values (205 MB) | TemplateFlow's MNI152NLin6Asym ↔ MNI152NLin2009cAsym |
| h5py | any | anything HDF5 allows | the validation's own files |

*Verified* (the survey of every `.h5` in the workspace, 2026-10-16).

## Reading (`HDF5TransformIOTemplate<T>::Read`, lines 285-367)

- **Which reader.** ITK's transform-IO factory asks its IOs in registration order: HDF5 (double,
  float), MATLAB, text. The HDF5 IO says yes to any file `H5Fis_hdf5` recognises that has a
  `/TransformGroup` link (`CanReadFile`, lines 40-75), **whatever its name ends with**; a
  `.h5` file it cannot read then fails with "Could not create Transform IO object for reading
  file" (`itkTransformFileReader.cxx:128`), listing the six IOs of the ANTs build. *Read;
  verified.* *larmorx:* the same order (`read_itk_transform_parts`).
- **The transforms are `/TransformGroup/0`, `/1`, … up to the group's number of links**
  (`getNumObjs`, line 299): each is opened by its index, not by listing the group, so a gap in
  the names (`0`, `2`) fails with HDF5's "object '1' doesn't exist". *Verified.*
- **`TransformType` is read as a variable-length string into a one-element dataspace**
  (lines 307-316). A fixed-length string (as h5py writes `numpy.bytes_`) fails: HDF5 has no
  conversion from it (**ITK-12**). Two strings fail with "src and dest dataspaces have
  different number of elements selected". A scalar dataspace is read. *Verified.*
- **The precision in the name is corrected to the reader's** (`CorrectTransformPrecisionType`,
  `itkTransformIOBase.h:173-195`): the double reader replaces the first `float` with `double`
  unless `double` occurs, and the other way round. A name with neither throws
  `std::out_of_range` from `std::string::replace` (position `npos`), which ANTs prints as
  "basic_string::replace: __pos (which is 18446744073709551615) > this->size() (which is N)".
  *Verified.* *larmorx:* the same, and in `fix` mode a plain message.
- **Any `CompositeTransform…` entry is created empty and has no parameters** (line 324). The
  reader (`TransformFileReaderTemplate::Update`) then puts the following transforms into a
  leading composite; ANTs (`itk::ants::ReadTransform`) uses that composite, a single
  transform, or a composite of all the transforms of a longer list. A composite further down
  stays an empty composite. *Read; verified.*
- **ITK 4's misspelt names** `TranformFixedParameters`, `TranformParameters` are read when the
  correct name does not exist (lines 327-353). *Verified* on TemplateFlow's files.
- **Parameters must be of the floating-point class and of rank 1** (`ReadParameters`, lines
  164-206; `ReadFixedParameters`, 210-252): "Wrong data type for <path>in HDF5 File" (no blank
  before "in") and "Wrong # of dims for TransformType in HDF5 File" (names `TransformType`
  whatever the dataset): **ITK-13**, messages only. *Verified.*
- **Parameters stored in 8 bytes are read as `double`, anything else as `float`**
  (`ParamType.getSize() == sizeof(double)`): half precision is widened exactly, but `long
  double` and IEEE quadruple precision lose their precision through `float` (**ITK-14**).
  *Verified* with h5py-written files.
- **HDF5's own conversions.** Big-endian floats are swapped; floats of other formats go through
  `H5T__conv_f_f`, which in HDF5 1.12 keeps state from one element to the next (**HDF-01**).
  Conversions run in strips of `1 MiB / max(source, destination size)` elements, per chunk.
  *Read* (`src/H5Tconv.c`); larmorx's port computes both results and warns where they differ.
- **Failures** inside the `try` become `itkExceptionMacro(<< error.getCDetailMsg())` at line
  365 ("H5Gopen2 failed", "H5Dopen2 failed", "H5Dread failed for variable length string",
  "H5Dread failed"), after HDF5 prints its error stack on standard error (`HDF5-DIAG: Error
  detected in HDF5 (1.12.1) thread 0:` and one frame per call). ANTs prints "Transform reader
  for <file> caught an ITK exception:" and the exception (`itkantsReadWriteTransform.h`).
  *Verified.* *larmorx:* the same text, the exception's addresses as `0x0`, ITK's source tree
  as `<ITK source>`, HDF5's stack abridged to the API call and the cause.
- **A null variable-length `TransformType`** (an HDF5 nil string) makes ITK's C++ binding
  build a `std::string` from a null pointer (`c++/src/H5DataSet.cpp:761`): undefined
  behaviour (**ITK-16**). *Read.* *larmorx:* refuses the file.
- **A composite whose components have another dimension** (a 2-D transform in a 3-D file): each
  component is `static_cast` to the composite's dimension
  (`itkCompositeTransformIOHelper.cxx:205`), undefined behaviour; antsApplyTransforms runs on
  and exits 0 (**ITK-17**). *Read; verified.* *larmorx:* refuses a transform whose name gives
  another dimension (`LinearTransform::from_itk`).

## Writing (`HDF5TransformIOTemplate<T>::Write`, lines 409-465)

- **The file:** created with library-version bounds 1.8 (`setLibverBounds(H5F_LIBVER_V18,
  H5F_LIBVER_V18)`, line 420): superblock version 2, version-2 object headers, new-style
  groups. Then four one-element variable-length `C_S1` strings: `/ITKVersion` (`5.4.5`),
  `/HDFVersion` (`H5_VERS_INFO`, "HDF5 library version: 1.12.1"), `/OSName` and `/OSVersion`
  (itksys `SystemInformation`: "Linux", the kernel release), then the group `/TransformGroup`.
  *Verified.*
- **The transforms:** when the first is a composite, the composite and its direct components
  (`CompositeTransformIOHelper`); otherwise the writer's list. Each gets a group `i` with
  `TransformType` and, unless it is a composite, `TransformFixedParameters` (always
  `NATIVE_DOUBLE`, contiguous; line 151) and `TransformParameters` (`NATIVE_DOUBLE` or
  `NATIVE_FLOAT` as the writer's template type; lines 117-148). A composite after the first
  transform throws "Composite Transform can only be 1st transform in a file" (line 390) after
  its group and type are written. *Read; verified.*
- **Compression** (`SetUseCompression(true)`): parameters in chunks of `min(n, 1 Mi)` values,
  deflate at level 5 (`H5Pset_deflate`, an *optional* filter); without it, contiguous.
  **ANTs always asks for it** (`itk::ants::WriteTransform`; antsApplyTransforms'
  `-o Linear[...]`, `-o CompositeTransform[...]`, `-o DisplacementField[x.h5]`); ITK's default
  is no compression. *Read; verified.*
- **A transform without parameters cannot be written with compression** (**ITK-15**): its
  chunk size is 0 and `H5Pset_chunk` fails ("all chunk dimensions must be positive"), caught at
  line 462 ("H5Pset_chunk failed"). The file keeps what was written before, the transform's
  fixed parameters included, and ITK cannot read it back. *Verified* (`CompositeTransformUtil
  --assemble` with an `IdentityTransform`, 2026-10-16). antsApplyTransforms never meets it:
  its `-t identity` is a `MatrixOffsetTransformBase` with 12 parameters.
- **Every object header records the time of writing** (access, modification, change, birth),
  so two writes of the same transform differ (ITK-09), and objects written across a second
  boundary carry different times within one file. *Verified.* *larmorx:* writes 0 unless told a
  time (`Hdf5WriteOptions.time`).
- **A float pipeline's linear transform keeps its centre as a float point**
  (`MatrixOffsetTransformBase<float>::m_Center`): what `--float 1 -o CompositeTransform[...]`
  writes back as fixed parameters is the centre rounded to float. Displacement fields keep
  their grid in double. *Verified* (itk-hdf5's float reading checks).

## What HDF5 1.12.1 makes of it, byte for byte

larmorx's writer (`larmorx_hdf5::write`) replays HDF5's own allocation and object-header
code for ITK's calls; with the same transforms, object time, strings and compressed chunks,
every byte is ITK's (*validated*: itk-hdf5's `write/` cases). What it replays:

- **File space** (`H5MF`, `src/H5MF*.c`): the `sec2` driver, strategy `FSM_AGGR` without
  persistent free space, a metadata and a small-data aggregator of 2048 bytes (global heaps
  count as raw data), free sections merged and absorbed, the end of the file shrunk at close.
- **Object headers** (`src/H5Oalloc.c`): groups get 120 bytes of messages (`H5G__obj_create_real`:
  link info, group info and four links of 8-character names), datasets 256
  (`H5D_MINHDR_SIZE`); messages go into the best-fitting null message, else the chunk grows in
  place when the file allows, else a continuation chunk is made, moving the smallest message
  that frees room for the continuation message (the group's link info moves this way).
  Message order in a dataset: dataspace, datatype, fill value, pipeline, layout.
- **Strings** go to global heap collections of at least 4096 bytes (`src/H5HG.c`), reused
  while they have room.
- **Chunks** are indexed by a version-1 B-tree (nodes of 2096 bytes for 1-D data); chunks of at
  most 1 MiB stay in the chunk cache until the dataset is closed, larger ones are written as
  they come.
- **The ninth link makes a group dense** (`H5G_obj_insert`, `src/H5Gobj.c`; `H5G__dense_create`,
  `src/H5Gdense.c`): a fractal heap header (146 bytes) and a version-2 B-tree header (38
  bytes); each link message, in header order, goes into the heap (its 512-byte root direct
  block, created at a temporary address) and its name hash (lookup3) into the B-tree's root
  leaf (512 bytes); the heap's free space is tracked by a persistent free-space manager (82-byte
  header). The link messages are then removed and the header condensed (`H5O__condense_header`:
  null messages to the end of their chunks, merged; emptied continuation chunks freed; the link
  info, which had moved to a continuation chunk, back into the first). At close the section
  info (26 bytes) and the direct block get real space, in that order, after the aggregators
  are released (`H5F__flush_phase1`, `H5FS__cache_hdr_pre_serialize`,
  `H5HF__cache_dblock_pre_serialize`). *Validated:* composites of 8, 9, 11, 36 and 37
  transforms, byte for byte.
- **The 39th link** of a dense group no longer fits the heap's first direct block: HDF5 makes
  the root an indirect block. larmorx's writer does not replay that; for a composite of 38 or
  more transforms it keeps the links as link messages, with a group info message that stores
  that many compact links (`H5Pset_link_phase_change`), a layout HDF5 writes for such a group
  creation property list and every reader reads. *Verified* (ITK's 38-transform file has an
  indirect root).

## Compression: zlib-ng 2.1.6's bytes depend on the CPU

ITK's HDF5 calls `compress2` of its bundled zlib, **zlib-ng 2.1.6** (`Modules/ThirdParty/ZLIB`),
at level 5: `deflate_medium` with a hash chosen at run time (`functable.c`): CRC-32C
(`_mm_crc32_u32`) on x86-64 CPUs with SSE4.2, CRC-32 (`__crc32w`) on Arm CPUs with the CRC
extension, a multiplicative hash otherwise. Different hashes find different matches, so **ITK's
compressed chunks, and every address after them, depend on the CPU it runs on**; they
decompress to the same bytes. *Read; verified on this machine (SSE4.2).* larmorx compresses
with zlib-rs 0.6.8 (flate2), which follows zlib-ng 2.2's multiplicative hash: small chunks (an
affine's 96 bytes) come out identical to ITK's; a 190 KB displacement-field chunk came out 3
bytes longer (first difference at byte 22,228 of the chunk). HDF5's deflate filter fails when
the stream would exceed `ceil(1.001 n) + 12` bytes, and the optional filter is then skipped
for that chunk (its filter-mask bit set); larmorx does the same. Decided 2026-10-16 (the
orchestrator): larmorx keeps zlib-rs; the bytes of compressed chunks may differ from ITK's,
nothing else does.

## Other readers

- **h5py** (3.16 on HDF5 2.0.0 here) reads every file larmorx and ITK write. With
  `libver="latest"` HDF5 2.0 writes chunked datasets with layout message version 5, which
  HDF5 1.12 (ITK) and larmorx refuse. *Verified.*
- **nitransforms 25.1.0** (`io/itk.py:366-430`, `ITKCompositeH5.from_h5obj`), which fMRIPrep uses,
  takes `list(h5group.values())[1:]`: h5py's **name order** ("10" before "2", **PIPE-04**), and
  **always skips the first group as if it were the composite**: a file of one transform
  (`-o Linear[x.h5]`) fails with `IndexError` in `TransformChain`, a file of several
  transforms without a composite loses its first, an empty composite fails (**PIPE-17**). Only
  `AffineTransform…` and `DisplacementFieldTransform…` are read; the misspelt names are
  detected on group `1` only. *Verified.*
