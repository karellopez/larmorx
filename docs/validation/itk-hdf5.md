# Parity: ITK's HDF5 transform files

**All cases agree.** 149 cases on the `standard` tier: 128 pass, 16 rejected by both, 5 expected divergences, 0 failures.

- **Validated:** `lx.transforms.read`/`write`, `lx.transforms.load_itk_composite`, `-t x.h5` of `larmorx ants antsApplyTransforms` (crates larmorx-hdf5, larmorx-io)
- **Reference:** ITK 5.4.5 on HDF5 1.12.1 through ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0 (antsApplyTransforms, CompositeTransformUtil), h5py 3.16.0 on HDF5 2.0.0, nitransforms 25.1.0
- **Test data:** larmorx-testdata `00b862132e9c`, tier `standard`
- **Generated:** 2026-10-21 on Linux x86_64, with `python -m larmorx_validation parity itk-hdf5 --tier standard`

Datasets as h5py reads them, bit for bit: 73 of 73 files
HDF5 storage features (every chunk index, the n-bit and scale-offset filters, dense groups' fractal heaps, filtered or not, shared messages) as h5py reads them, bit for bit: 54 of 54 files
Transforms as ITK reads them (double), bit for bit: 57 of 57 files
Transforms as ITK reads them (float), bit for bit: 57 of 57 files
nitransforms' chain, bit for bit: 57 of 57 files
Files ITK refuses: 16 of 16 refused alike (exit status and messages)
Files ITK reads and larmorx refuses (expected divergences): 4 (ITK-17's undefined cast; data stored in other files)
Files written: 16 of 17 byte-identical to ITK's given ITK's compressed chunks and object times
Files written with larmorx's deflate: 12 of 17 byte-identical (object times set alike)

## Thresholds

| Quantity | Requirement |
|---|---|
| Datasets | every group and dataset h5py reads: names, types, shapes, values bit for bit |
| Transforms | as ITK reads them (written back by ANTs), bit for bit, in double and float |
| nitransforms | the chain it loads: matrices and fields bit for bit |
| Refused files | the same exit status and messages (addresses, build paths aside) |
| Written files | every byte ITK's, given ITK's compressed chunks and object times; read back by ANTs (identical output image), h5py and nitransforms as ITK's file |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| errors | 20 | 0 | 16 | 4 | 0 |
| hdf5 | 54 | 54 | 0 | 0 | 0 |
| read/catalog | 4 | 4 | 0 | 0 | 0 |
| read/fmriprep-25.2.6 | 10 | 10 | 0 | 0 | 0 |
| read/h5py | 26 | 26 | 0 | 0 | 0 |
| read/itk | 17 | 17 | 0 | 0 | 0 |
| write | 18 | 17 | 0 | 1 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `errors/h5py/external-link` | a transform behind an external link to another HDF5 file: ITK's HDF5 follows it; larmorx refuses the file |
| `errors/h5py/external-storage` | parameters in an external raw file (H5Pset_external): ITK's HDF5 follows it; larmorx refuses the file |
| `errors/h5py/mixed-dimension` | ITK-17: ITK casts the 2-D component to a 3-D transform (undefined behaviour) and antsApplyTransforms exits 0; larmorx refuses the file |
| `errors/h5py/virtual` | parameters in a virtual dataset (mapping another dataset of the same file): ITK's HDF5 follows it; larmorx refuses the file |
| `write/composite-38-affines` | 39 links do not fit the first block of TransformGroup's fractal heap: HDF5 adds an indirect block, which larmorx's writer does not cover; larmorx keeps the links as link messages (a group info storing 39 compact links), a layout HDF5 itself writes for such a group property list. Every reader reads … |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `errors/h5py/2d-fixed` | written by h5py, refused by ITK: 2d-fixed | Description: ITK ERROR: HDF5TransformIOTemplate(0x0): Wrong # of dims for TransformType in HDF5 File |
| `errors/h5py/2d-parameters` | written by h5py, refused by ITK: 2d-parameters | Description: ITK ERROR: HDF5TransformIOTemplate(0x0): Wrong # of dims for TransformType in HDF5 File |
| `errors/h5py/fixed-string` | written by h5py, refused by ITK: fixed-string | Description: ITK ERROR: HDF5TransformIOTemplate(0x0): H5Dread failed for variable length string |
| `errors/h5py/gap-in-names` | written by h5py, refused by ITK: gap-in-names | Description: ITK ERROR: HDF5TransformIOTemplate(0x0): H5Gopen2 failed |
| `errors/h5py/int-fixed` | written by h5py, refused by ITK: int-fixed | Description: ITK ERROR: HDF5TransformIOTemplate(0x0): Wrong data type for /TransformGroup/1/TransformFixedParametersin HDF5 File |
| `errors/h5py/int-parameters` | written by h5py, refused by ITK: int-parameters | Description: ITK ERROR: HDF5TransformIOTemplate(0x0): Wrong data type for /TransformGroup/1/TransformParametersin HDF5 File |
| `errors/h5py/lzf` | written by h5py, refused by ITK: lzf | Description: ITK ERROR: HDF5TransformIOTemplate(0x0): H5Dread failed |
| `errors/h5py/missing-parameters` | written by h5py, refused by ITK: missing-parameters | Description: ITK ERROR: HDF5TransformIOTemplate(0x0): H5Dopen2 failed |
| `errors/h5py/no-precision-in-name` | written by h5py, refused by ITK: no-precision-in-name | basic_string::replace: __pos (which is 18446744073709551615) > this->size() (which is 15) |
| `errors/h5py/no-transform-group` | written by h5py, refused by ITK: no-transform-group | failed to read file: /tmp/lx-h5-sdd15ydg/no-transform-group.h5 |
| `errors/h5py/not-hdf5` | written by h5py, refused by ITK: not-hdf5 | set the suffix to an unsupported type. |
| `errors/h5py/scalar-parameters` | written by h5py, refused by ITK: scalar-parameters | Description: ITK ERROR: HDF5TransformIOTemplate(0x0): Wrong # of dims for TransformType in HDF5 File |
| `errors/h5py/superblock3-chunked` | written by h5py, refused by ITK: superblock3-chunked | Description: ITK ERROR: HDF5TransformIOTemplate(0x0): H5Dopen2 failed |
| `errors/h5py/truncated` | written by h5py, refused by ITK: truncated | set the suffix to an unsupported type. |
| `errors/h5py/two-strings` | written by h5py, refused by ITK: two-strings | Description: ITK ERROR: HDF5TransformIOTemplate(0x0): H5Dread failed for variable length string |
| `errors/h5py/unknown-type` | written by h5py, refused by ITK: unknown-type | "VersorTransform_float_3_3" |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (1dfe56c0dc60) |
| larmorx-testdata | 00b862132e9c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| h5py | 3.16.0 |
| nitransforms | 25.1.0 |
| nibabel | 5.4.2 |

## Notes

larmorx reads in `reproduce` mode for the comparisons (ITK's defects reproduced, D13), and a fix pass reports what the default changes. **The bytes in which larmorx's own files differ from ITK's, and no others:** (1) the four times of every version-2 object header (access, modification, change and birth time: ITK's HDF5 records the second of writing; larmorx writes the fixed value 0, so its files do not depend on when they were written, `Hdf5WriteOptions.time` sets another) and with them each object header's checksum (Jenkins' lookup3, its last four bytes); (2) the compressed chunk streams, where larmorx's deflate (zlib-rs) chooses other matches than ITK's (zlib-ng 2.1.6, level 5): a differing stream also changes its size, recorded in the chunk B-tree's key, and moves everything allocated after it by the difference, so every address of something allocated after it changes too (links to later groups and datasets, later B-tree nodes and chunks in layout messages and B-tree nodes, continuation blocks, global heap collections), with the checksums of the object headers that hold them, and the superblock's end-of-file address with the superblock's checksum. The byte comparisons therefore give larmorx ITK's time and set each object's times alike on both sides, and compare once with ITK's own compressed chunks (every byte must then be ITK's) and once with larmorx's deflate (the chunks that differ are listed with their sizes). Decompressed, every chunk is identical. zlib-ng 2.1.6 hashes with CRC-32C on x86-64 CPUs with SSE4.2, CRC-32 on Arm with the CRC extension, and multiplicatively elsewhere, so ITK's own compressed bytes depend on the CPU; larmorx's deflate gives the same bytes on every platform (golden case `py.transforms.write_h5`, which also fingerprints a compressed file's bytes).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `errors/h5py/2d-fixed` | both-error | 5/5 | written by h5py, refused by ITK: 2d-fixed |
| `errors/h5py/2d-parameters` | both-error | 5/5 | written by h5py, refused by ITK: 2d-parameters |
| `errors/h5py/external-link` | expected-divergence | 5/5 | written by h5py, read by ITK, refused by larmorx: a transform behind an external link to another HDF5 file |
| `errors/h5py/external-storage` | expected-divergence | 5/5 | written by h5py, read by ITK, refused by larmorx: parameters in an external raw file (H5Pset_external) |
| `errors/h5py/fixed-string` | both-error | 5/5 | written by h5py, refused by ITK: fixed-string |
| `errors/h5py/gap-in-names` | both-error | 5/5 | written by h5py, refused by ITK: gap-in-names |
| `errors/h5py/int-fixed` | both-error | 5/5 | written by h5py, refused by ITK: int-fixed |
| `errors/h5py/int-parameters` | both-error | 5/5 | written by h5py, refused by ITK: int-parameters |
| `errors/h5py/lzf` | both-error | 5/5 | written by h5py, refused by ITK: lzf |
| `errors/h5py/missing-parameters` | both-error | 5/5 | written by h5py, refused by ITK: missing-parameters |
| `errors/h5py/mixed-dimension` | expected-divergence | 3/3 | written by h5py, refused by ITK: mixed-dimension |
| `errors/h5py/no-precision-in-name` | both-error | 5/5 | written by h5py, refused by ITK: no-precision-in-name |
| `errors/h5py/no-transform-group` | both-error | 5/5 | written by h5py, refused by ITK: no-transform-group |
| `errors/h5py/not-hdf5` | both-error | 5/5 | written by h5py, refused by ITK: not-hdf5 |
| `errors/h5py/scalar-parameters` | both-error | 5/5 | written by h5py, refused by ITK: scalar-parameters |
| `errors/h5py/superblock3-chunked` | both-error | 5/5 | written by h5py, refused by ITK: superblock3-chunked |
| `errors/h5py/truncated` | both-error | 5/5 | written by h5py, refused by ITK: truncated |
| `errors/h5py/two-strings` | both-error | 5/5 | written by h5py, refused by ITK: two-strings |
| `errors/h5py/unknown-type` | both-error | 5/5 | written by h5py, refused by ITK: unknown-type |
| `errors/h5py/virtual` | expected-divergence | 5/5 | written by h5py, read by ITK, refused by larmorx: parameters in a virtual dataset (mapping another dataset of the same … |
| `hdf5/fixture/sb0` | pass | 2/2 | a golden-test fixture (tests/golden/hdf5/sb0.h5.gz) |
| `hdf5/fixture/sb1` | pass | 2/2 | a golden-test fixture (tests/golden/hdf5/sb1.h5.gz) |
| `hdf5/fixture/sb3-datasets` | pass | 2/2 | a golden-test fixture (tests/golden/hdf5/sb3-datasets.h5.gz) |
| `hdf5/fixture/sb3-groups` | pass | 2/2 | a golden-test fixture (tests/golden/hdf5/sb3-groups.h5.gz) |
| `hdf5/fixture/sohm` | pass | 4/4 | a golden-test fixture (tests/golden/hdf5/sohm.h5.gz) |
| `hdf5/group/deflate-direct` | pass | 1/1 | an HDF5 storage feature written by h5py: group:deflate-direct |
| `hdf5/group/deflate-fletcher32-direct` | pass | 1/1 | an HDF5 storage feature written by h5py: group:deflate-fletcher32-direct |
| `hdf5/group/deflate-fletcher32-huge` | pass | 1/1 | an HDF5 storage feature written by h5py: group:deflate-fletcher32-huge |
| `hdf5/group/deflate-fletcher32-indirect` | pass | 1/1 | an HDF5 storage feature written by h5py: group:deflate-fletcher32-indirect |
| `hdf5/group/deflate-huge` | pass | 1/1 | an HDF5 storage feature written by h5py: group:deflate-huge |
| `hdf5/group/deflate-indirect` | pass | 1/1 | an HDF5 storage feature written by h5py: group:deflate-indirect |
| `hdf5/group/order-indirect` | pass | 1/1 | an HDF5 storage feature written by h5py: group:order-indirect |
| `hdf5/group/plain-direct` | pass | 1/1 | an HDF5 storage feature written by h5py: group:plain-direct |
| `hdf5/group/plain-huge` | pass | 1/1 | an HDF5 storage feature written by h5py: group:plain-huge |
| `hdf5/group/plain-indirect` | pass | 1/1 | an HDF5 storage feature written by h5py: group:plain-indirect |
| `hdf5/index/bt2` | pass | 1/1 | an HDF5 storage feature written by h5py: index:bt2 |
| `hdf5/index/bt2-filtered` | pass | 1/1 | an HDF5 storage feature written by h5py: index:bt2-filtered |
| `hdf5/index/earray-2d-swizzled` | pass | 1/1 | an HDF5 storage feature written by h5py: index:earray-2d-swizzled |
| `hdf5/index/earray-3d-swizzled` | pass | 1/1 | an HDF5 storage feature written by h5py: index:earray-3d-swizzled |
| `hdf5/index/earray-dblocks` | pass | 1/1 | an HDF5 storage feature written by h5py: index:earray-dblocks |
| `hdf5/index/earray-filtered` | pass | 1/1 | an HDF5 storage feature written by h5py: index:earray-filtered |
| `hdf5/index/earray-paged` | pass | 1/1 | an HDF5 storage feature written by h5py: index:earray-paged |
| `hdf5/index/earray-sblocks` | pass | 1/1 | an HDF5 storage feature written by h5py: index:earray-sblocks |
| `hdf5/index/earray-small` | pass | 1/1 | an HDF5 storage feature written by h5py: index:earray-small |
| `hdf5/index/farray` | pass | 1/1 | an HDF5 storage feature written by h5py: index:farray |
| `hdf5/index/farray-paged` | pass | 1/1 | an HDF5 storage feature written by h5py: index:farray-paged |
| `hdf5/index/farray-paged-filtered` | pass | 1/1 | an HDF5 storage feature written by h5py: index:farray-paged-filtered |
| `hdf5/index/implicit` | pass | 1/1 | an HDF5 storage feature written by h5py: index:implicit |
| `hdf5/index/single` | pass | 1/1 | an HDF5 storage feature written by h5py: index:single |
| `hdf5/index/single-filtered` | pass | 1/1 | an HDF5 storage feature written by h5py: index:single-filtered |
| `hdf5/nbit/float` | pass | 1/1 | an HDF5 storage feature written by h5py: nbit:float |
| `hdf5/nbit/int` | pass | 1/1 | an HDF5 storage feature written by h5py: nbit:int |
| `hdf5/nbit/int-be` | pass | 1/1 | an HDF5 storage feature written by h5py: nbit:int-be |
| `hdf5/scaleoffset/>f8,3,` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:>f8,3, |
| `hdf5/scaleoffset/f4,0,` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:f4,0, |
| `hdf5/scaleoffset/f4,2,` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:f4,2, |
| `hdf5/scaleoffset/f4,2,-7.5` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:f4,2,-7.5 |
| `hdf5/scaleoffset/f4,6,` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:f4,6, |
| `hdf5/scaleoffset/f8,0,` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:f8,0, |
| `hdf5/scaleoffset/f8,2,` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:f8,2, |
| `hdf5/scaleoffset/f8,6,` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:f8,6, |
| `hdf5/scaleoffset/i1,0,` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:i1,0, |
| `hdf5/scaleoffset/i2,0,` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:i2,0, |
| `hdf5/scaleoffset/i4,0,` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:i4,0, |
| `hdf5/scaleoffset/i4,0,12345` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:i4,0,12345 |
| `hdf5/scaleoffset/i8,0,` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:i8,0, |
| `hdf5/scaleoffset/u1,0,` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:u1,0, |
| `hdf5/scaleoffset/u2,0,` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:u2,0, |
| `hdf5/scaleoffset/u4,0,` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:u4,0, |
| `hdf5/scaleoffset/u8,0,` | pass | 1/1 | an HDF5 storage feature written by h5py: scaleoffset:u8,0, |
| `hdf5/sohm/btree` | pass | 1/1 | an HDF5 storage feature written by h5py: sohm:btree |
| `hdf5/sohm/one-list` | pass | 1/1 | an HDF5 storage feature written by h5py: sohm:one-list |
| `hdf5/sohm/superblock2` | pass | 1/1 | an HDF5 storage feature written by h5py: sohm:superblock2 |
| `hdf5/sohm/two-lists` | pass | 1/1 | an HDF5 storage feature written by h5py: sohm:two-lists |
| `read/catalog/openneuro-derivatives/ds000005-fmriprep/sub-01/anat/sub-01_from-MNI152NLin2009cAsym_to-T1w_mode-image_xfm.h5` | pass | 5/5 | Inverse composite transform MNI152NLin2009cAsym -> T1w (HDF5), used to bring template masks/priors into T1w space. |
| `read/catalog/openneuro-derivatives/ds000005-fmriprep/sub-01/anat/sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5` | pass | 5/5 | ANTs composite transform (HDF5: affine + SyN displacement field) T1w -> MNI152NLin2009cAsym, as written by antsRegistra… |
| `read/catalog/synthetic/resampling/transforms/composite-affine-warp.h5` | pass | 5/5 | ITK HDF5 composite, as antsApplyTransforms -o CompositeTransform[...] writes it from -t warp-lps-vector.nii.gz -t affin… |
| `read/catalog/synthetic/series-resampling/transforms/anat2std-composite.h5` | pass | 5/5 | Anatomical-to-standard ITK composite in ITK's HDF5 layout: an AffineTransform_double_3_3 (with a centre) then a Displac… |
| `read/fmriprep-25.2.6/ds000005-sub-01-noreconall/sub-01_from-MNI152NLin2009cAsym_to-T1w_mode-image_xfm.h5` | pass | 5/5 | found: sub-01_from-MNI152NLin2009cAsym_to-T1w_mode-image_xfm.h5 |
| `read/fmriprep-25.2.6/ds000005-sub-01-noreconall/sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5` | pass | 5/5 | found: sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5 |
| `read/fmriprep-25.2.6/ds001600-sub-1-noreconall/sub-1_from-MNI152NLin2009cAsym_to-T1w_mode-image_xfm.h5` | pass | 5/5 | found: sub-1_from-MNI152NLin2009cAsym_to-T1w_mode-image_xfm.h5 |
| `read/fmriprep-25.2.6/ds001600-sub-1-noreconall/sub-1_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5` | pass | 5/5 | found: sub-1_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5 |
| `read/fmriprep-25.2.6/ds005040-sub-002-noreconall/sub-002_from-MNI152NLin2009cAsym_to-T1w_mode-image_xfm.h5` | pass | 5/5 | found: sub-002_from-MNI152NLin2009cAsym_to-T1w_mode-image_xfm.h5 |
| `read/fmriprep-25.2.6/ds005040-sub-002-noreconall/sub-002_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5` | pass | 5/5 | found: sub-002_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5 |
| `read/fmriprep-25.2.6/ds005454-sub-16-noreconall/sub-16_from-MNI152NLin2009cAsym_to-T1w_mode-image_xfm.h5` | pass | 5/5 | found: sub-16_from-MNI152NLin2009cAsym_to-T1w_mode-image_xfm.h5 |
| `read/fmriprep-25.2.6/ds005454-sub-16-noreconall/sub-16_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5` | pass | 5/5 | found: sub-16_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5 |
| `read/fmriprep-25.2.6/ds005752-sub-ON01016-noreconall/sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_from-MNI152NLin2009cAsym_to-T1w_mode-image_xfm.h5` | pass | 5/5 | found: sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_from-MNI152NLin2009cAsym_to-T1w_mode-image_xfm.h5 |
| `read/fmriprep-25.2.6/ds005752-sub-ON01016-noreconall/sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5` | pass | 5/5 | found: sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5 |
| `read/h5py/big-endian-f4` | pass | 5/5 | written by h5py: big-endian-f4 |
| `read/h5py/big-endian-f8` | pass | 5/5 | written by h5py: big-endian-f8 |
| `read/h5py/compact` | pass | 5/5 | written by h5py: compact |
| `read/h5py/composite` | pass | 5/5 | written by h5py: composite |
| `read/h5py/composite-second` | pass | 7/7 | written by h5py: composite-second |
| `read/h5py/dense-11` | pass | 5/5 | written by h5py: dense-11 |
| `read/h5py/dense-11-superblock3` | pass | 5/5 | written by h5py: dense-11-superblock3 |
| `read/h5py/earray` | pass | 5/5 | written by h5py: earray |
| `read/h5py/empty-composite` | pass | 5/5 | written by h5py: empty-composite |
| `read/h5py/farray` | pass | 5/5 | written by h5py: farray |
| `read/h5py/farray-paged` | pass | 5/5 | written by h5py: farray-paged |
| `read/h5py/float32` | pass | 5/5 | written by h5py: float32 |
| `read/h5py/half` | pass | 5/5 | written by h5py: half |
| `read/h5py/implicit` | pass | 5/5 | written by h5py: implicit |
| `read/h5py/long-double` | pass | 5/5 | written by h5py: long-double |
| `read/h5py/misspelt-itk4` | pass | 5/5 | written by h5py: misspelt-itk4 |
| `read/h5py/nbit` | pass | 5/5 | written by h5py: nbit |
| `read/h5py/no-version-strings` | pass | 5/5 | written by h5py: no-version-strings |
| `read/h5py/scalar-type` | pass | 5/5 | written by h5py: scalar-type |
| `read/h5py/scaleoffset` | pass | 5/5 | written by h5py: scaleoffset |
| `read/h5py/shuffle-fletcher32` | pass | 5/5 | written by h5py: shuffle-fletcher32 |
| `read/h5py/single` | pass | 5/5 | written by h5py: single |
| `read/h5py/single-chunk` | pass | 5/5 | written by h5py: single-chunk |
| `read/h5py/superblock3` | pass | 5/5 | written by h5py: superblock3 |
| `read/h5py/two-linear` | pass | 5/5 | written by h5py: two-linear |
| `read/h5py/utf8-strings` | pass | 5/5 | written by h5py: utf8-strings |
| `read/itk/assemble` | pass | 5/5 | written by ITK: CompositeTransformUtil --assemble (itk::ants::WriteTransform) |
| `read/itk/assemble-field-first` | pass | 5/5 | written by ITK: CompositeTransformUtil --assemble: a field, then two linear transforms (objects allocated after the com… |
| `read/itk/composite-11-affines` | pass | 5/5 | written by ITK: -o CompositeTransform[...] of 11 affines: 12 links (dense: a fractal heap and a v2 B-tree) |
| `read/itk/composite-37-affines` | pass | 5/5 | written by ITK: -o CompositeTransform[...] of 37 affines: 38 links (dense: a fractal heap and a v2 B-tree) |
| `read/itk/composite-38-affines` | pass | 5/5 | written by ITK: -o CompositeTransform[...] of 38 affines: 39 links (dense: a fractal heap and a v2 B-tree) |
| `read/itk/composite-8-affines` | pass | 5/5 | written by ITK: -o CompositeTransform[...] of 8 affines: 9 links (dense: a fractal heap and a v2 B-tree) |
| `read/itk/composite-9-affines` | pass | 5/5 | written by ITK: -o CompositeTransform[...] of 9 affines: 10 links (dense: a fractal heap and a v2 B-tree) |
| `read/itk/composite-affine` | pass | 5/5 | written by ITK: -o CompositeTransform[...]: one affine |
| `read/itk/composite-float` | pass | 5/5 | written by ITK: --float 1 -o CompositeTransform[...]: float parameters |
| `read/itk/composite-identity` | pass | 5/5 | written by ITK: -o CompositeTransform[...] -t identity |
| `read/itk/composite-types` | pass | 5/5 | written by ITK: -o CompositeTransform[...]: six transform types (seven links, compact) |
| `read/itk/composite-warp-affine` | pass | 5/5 | written by ITK: -o CompositeTransform[...]: a displacement field and an affine (chunk of 190 KB) |
| `read/itk/displacement-field` | pass | 5/5 | written by ITK: -o DisplacementField[...]: a collapsed field on the reference grid |
| `read/itk/fmriprep-rewritten` | pass | 5/5 | written by ITK: fMRIPrep's T1w→MNI composite rewritten by ITK 5.4.5: a field of several 1 Mi-value chunks |
| `read/itk/linear` | pass | 5/5 | written by ITK: -o Linear[...]: two transforms collapsed into one affine |
| `read/itk/linear-float` | pass | 5/5 | written by ITK: --float 1 -o Linear[...]: float parameters |
| `read/itk/linear-inverse` | pass | 5/5 | written by ITK: -o Linear[...,1]: the inverse |
| `write/assemble` | pass | 7/7 | CompositeTransformUtil --assemble (itk::ants::WriteTransform) |
| `write/assemble-field-first` | pass | 7/7 | CompositeTransformUtil --assemble: a field, then two linear transforms (objects allocated after the compressed chunk) |
| `write/composite-11-affines` | pass | 6/6 | -o CompositeTransform[...] of 11 affines: 12 links (dense: a fractal heap and a v2 B-tree) |
| `write/composite-37-affines` | pass | 6/6 | -o CompositeTransform[...] of 37 affines: 38 links (dense: a fractal heap and a v2 B-tree) |
| `write/composite-38-affines` | expected-divergence | 6/6 | -o CompositeTransform[...] of 38 affines: 39 links (dense: a fractal heap and a v2 B-tree) |
| `write/composite-8-affines` | pass | 6/6 | -o CompositeTransform[...] of 8 affines: 9 links (dense: a fractal heap and a v2 B-tree) |
| `write/composite-9-affines` | pass | 6/6 | -o CompositeTransform[...] of 9 affines: 10 links (dense: a fractal heap and a v2 B-tree) |
| `write/composite-affine` | pass | 7/7 | -o CompositeTransform[...]: one affine |
| `write/composite-float` | pass | 6/6 | --float 1 -o CompositeTransform[...]: float parameters |
| `write/composite-identity` | pass | 6/6 | -o CompositeTransform[...] -t identity |
| `write/composite-types` | pass | 7/7 | -o CompositeTransform[...]: six transform types (seven links, compact) |
| `write/composite-warp-affine` | pass | 7/7 | -o CompositeTransform[...]: a displacement field and an affine (chunk of 190 KB) |
| `write/displacement-field` | pass | 6/6 | -o DisplacementField[...]: a collapsed field on the reference grid |
| `write/fmriprep-rewritten` | pass | 6/6 | fMRIPrep's T1w→MNI composite rewritten by ITK 5.4.5: a field of several 1 Mi-value chunks |
| `write/itk15-identity-compressed` | pass | 7/7 | CompositeTransformUtil --assemble with an IdentityTransform: ITK fails (ITK-15) and leaves a partial file |
| `write/linear` | pass | 6/6 | -o Linear[...]: two transforms collapsed into one affine |
| `write/linear-float` | pass | 6/6 | --float 1 -o Linear[...]: float parameters |
| `write/linear-inverse` | pass | 6/6 | -o Linear[...,1]: the inverse |
