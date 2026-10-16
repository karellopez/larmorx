# Provenance: larmorx-hdf5

A pure-Rust reader of the HDF5 file format and a writer of the subset ITK's transform files
use (CLAUDE.md rule 5: no HDF5 C library).

**The format** follows The HDF Group's *HDF5 File Format Specification*, version 3.0.
**The behaviour and the layout** follow HDF5 **1.12.1**, the version ITK v5.4.5 bundles
(`<workspace>/tags/ITK-v5.4.5/Modules/ThirdParty/HDF5/src/itkhdf5`, tag `hdf5-1_12_1` per its
`UpdateFromUpstream.sh`). HDF5 is distributed under a BSD-style licence (The HDF Group and the
University of Illinois; reproduced in `NOTICE`); files that re-express its logic carry
`SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause` (listed in `scripts/check_spdx.py`).
No code was copied verbatim; the logic is re-expressed in Rust.

| larmorx | HDF5 1.12.1 source it follows |
|---|---|
| `checksum.rs`: `lookup3`, `fletcher32` | `src/H5checksum.c`: `H5_checksum_lookup3` (Bob Jenkins' public-domain lookup3), `H5_checksum_fletcher32` |
| `superblock.rs`: signature search, versions 0-3, base address, truncation check | `src/H5FDint.c` (`H5FD_locate_signature`), `src/H5Fsuper.c` (`H5F__super_read`), `src/H5Fsuper_cache.c` (`H5F__superblock_prefix_decode`, `H5F__cache_superblock_deserialize`) |
| `object.rs`: object headers 1 and 2, continuation chunks, message flags | `src/H5Ocache.c` (`H5O__prefix_deserialize`, `H5O__chunk_deserialize`), `src/H5Opkg.h` |
| `message.rs`: dataspace, fill value (old and new), layout 1-4, filter pipeline 1-2, link, link info, group info, symbol table, shared messages | `src/H5Osdspace.c`, `src/H5Ofill.c`, `src/H5Olayout.c`, `src/H5Opline.c`, `src/H5Olink.c`, `src/H5Olinfo.c`, `src/H5Oginfo.c`, `src/H5Ostab.c`, `src/H5Oshared.c` (`decode` callbacks) |
| `datatype.rs`: datatype message; conversion of floating-point data to native `double`/`float` | `src/H5Odtype.c` (`H5O__dtype_decode_helper`); `src/H5T.c` (path search: hard functions, then soft functions last registered first); `src/H5Tconv.c` (`H5T__conv_order_opt`, `H5T__conv_f_f`, the hard `long double` conversions `H5T_CONV_Ff`); `src/H5Tbit.c` (bit fields) |
| `btree.rs`: version-1 and version-2 B-trees | `src/H5Bcache.c`, `src/H5B2cache.c`, `src/H5B2hdr.c` (`H5B2__hdr_init`), `src/H5B2pkg.h` |
| `heap.rs`: local, global and fractal heaps | `src/H5HLcache.c`, `src/H5HGcache.c` (`H5HG_read`), `src/H5HFcache.c`, `src/H5HFdtable.c`, `src/H5HFdblock.c` (`H5HF__man_dblock_locate`), `src/H5HFhuge.c`, `src/H5HFtiny.c` |
| `group.rs`: symbol-table, compact and dense groups | `src/H5Gstab.c`, `src/H5Gnode.c`, `src/H5Gcompact.c`, `src/H5Gdense.c`, `src/H5Gobj.c`, `src/H5Oint.c` (`H5O__obj_class_real`) |
| `dataset.rs`: compact, contiguous and chunked storage; chunk indices; fill values | `src/H5Dcontig.c`, `src/H5Dcompact.c`, `src/H5Dchunk.c` (`H5D__chunk_lock`), `src/H5Dbtree.c`, `src/H5Dsingle.c`, `src/H5Dnone.c`, `src/H5Dfarray.c`, `src/H5FAcache.c`, `src/H5Dbt2.c` |
| `filters.rs`: deflate, shuffle, Fletcher-32 | `src/H5Z.c` (`H5Z_pipeline`), `src/H5Zdeflate.c`, `src/H5Zshuffle.c`, `src/H5Zfletcher32.c` |
| `file.rs`: path traversal, `H5Lexists`, strip size of conversions | `src/H5Gtraverse.c`, `src/H5L.c`, `src/H5Dio.c` (`H5D__typeinfo_init`: `H5D_TEMP_BUF_SIZE`) |

Deflate is [zlib-rs](https://github.com/trifectatechfoundation/zlib-rs) (Zlib licence)
through [flate2](https://github.com/rust-lang/flate2-rs) (MIT/Apache-2.0).
