# Provenance: larmorx-hdf5

A pure-Rust reader of the HDF5 file format and a writer of the subset ITK's transform files
use, laid out byte for byte as HDF5 1.12.1 lays it out (CLAUDE.md rule 5: no HDF5 C library).

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
| `dataset.rs`: compact, contiguous and chunked storage; chunk indices; fill values | `src/H5Dcontig.c`, `src/H5Dcompact.c`, `src/H5Dchunk.c` (`H5D__chunk_lock`), `src/H5Dbtree.c`, `src/H5Dsingle.c`, `src/H5Dnone.c`, `src/H5Dfarray.c`, `src/H5FAcache.c`, `src/H5Dearray.c` (`H5D__earray_idx_iterate`, the swizzled chunk index), `src/H5EA.c` (`H5EA__lookup_elmt`), `src/H5EAhdr.c` (`H5EA__hdr_init`), `src/H5EAcache.c` (the header, index block, super block, data block and page deserializers), `src/H5EAdblock.c` (`H5EA__dblock_sblk_idx`), `src/H5Dbt2.c`; `src/H5Olayout.c` (the chunk index types of `H5D_chunk_index_t`) |
| `filters.rs`: deflate, shuffle, Fletcher-32, n-bit, scale-offset | `src/H5Z.c` (`H5Z_pipeline`), `src/H5Zdeflate.c`, `src/H5Zshuffle.c`, `src/H5Zfletcher32.c`, `src/H5Znbit.c` (`H5Z__filter_nbit`, `H5Z__nbit_decompress` and its atomic, array, compound and no-op cases), `src/H5Zscaleoffset.c` (`H5Z__filter_scaleoffset`, `H5Z__scaleoffset_decompress`, the post-decompression of integers and of floating-point data with decimal scaling; `pow(10, D)` correctly rounded, from `larmorx_core::math`) |
| `file.rs`: path traversal, `H5Lexists`, strip size of conversions | `src/H5Gtraverse.c`, `src/H5L.c`, `src/H5Dio.c` (`H5D__typeinfo_init`: `H5D_TEMP_BUF_SIZE`) |

## The writer (`src/write/`)

Replays what HDF5 1.12.1 does for the calls ITK's transform writer makes
(`Modules/IO/TransformHDF5/src/itkHDF5TransformIO.cxx`: `H5Fcreate` with library-version bounds
1.8, `H5Gcreate2`, `H5Dcreate2` and `H5Dwrite` of variable-length strings and floating-point
datasets, contiguous or chunked with deflate, `H5Fclose`), so that the file's bytes are HDF5's.

| larmorx | HDF5 1.12.1 source it follows |
|---|---|
| `write/alloc.rs`: file space (aggregators, free sections, merging, shrinking, extending, closing) | `src/H5MF.c` (`H5MF_alloc`, `H5MF_xfree`, `H5MF_try_extend`, `H5MF_try_shrink`, `H5MF_close`, `H5MF__close_shrink_eoa`), `src/H5MFaggr.c` (`H5MF__aggr_alloc`, `H5MF__aggr_try_extend`, `H5MF__aggr_reset`, `H5MF_free_aggrs`), `src/H5MFsection.c` (`H5MF__sect_simple_can_merge`, `_merge`, `_can_shrink`, `_shrink`), `src/H5FSsection.c` (`H5FS_sect_add`, `H5FS__sect_merge`, `H5FS__sect_find_node`), `src/H5FDspace.c`, `src/H5Fspace.c` |
| `write/ohdr.rs`: object headers: creation, room for messages, continuation chunks, gaps, message removal and condensing, serialization | `src/H5Oint.c` (`H5O_create`, `H5O_apply_ohdr`), `src/H5Oalloc.c` (`H5O__alloc`, `H5O__alloc_null`, `H5O__alloc_find_best_null`, `H5O__alloc_extend_chunk`, `H5O__alloc_find_best_nonnull`, `H5O__alloc_new_chunk`, `H5O__alloc_chunk`, `H5O__add_gap`, `H5O__eliminate_gap`, `H5O__release_mesg`, `H5O__move_cont`, `H5O__move_msgs_forward`, `H5O__merge_null`, `H5O__remove_empty_chunks`, `H5O__condense_header`), `src/H5Omessage.c` (`H5O__msg_remove_real`, `H5O__msg_iterate_real`, `H5O_msg_flush`), `src/H5Ocont.c`, `src/H5Ochunk.c` (`H5O__chunk_delete`), `src/H5Ocache.c` (`H5O__cache_serialize`, `H5O__chunk_serialize`) |
| `write/encode.rs`: the messages written | `src/H5Olinfo.c`, `src/H5Oginfo.c`, `src/H5Olink.c`, `src/H5Osdspace.c`, `src/H5Odtype.c`, `src/H5Ofill.c`, `src/H5Olayout.c`, `src/H5Opline.c`, `src/H5Ocont.c` (`encode`, `raw_size`) |
| `write/gheap.rs`: global heap collections for variable-length strings | `src/H5HG.c` (`H5HG__create`, `H5HG_insert`, `H5HG__alloc`, `H5HG_extend`), `src/H5Fcwfs.c`, `src/H5Tvlen.c` (`H5T__vlen_disk_write`), `src/H5VLnative_blob.c` |
| `write/btree.rs`: the version-1 B-tree of a chunked dataset's chunks | `src/H5B.c` (`H5B_create`, `H5B_insert`, `H5B__insert_helper`, `H5B__split`, `H5B__insert_child`), `src/H5Bcache.c` (`H5B__cache_serialize`), `src/H5Dbtree.c` (`H5D__btree_cmp3`, `H5D__btree_new_node`, `H5D__btree_insert`), `src/H5VM.c` (`H5VM_vector_cmp_u`) |
| `write/dense.rs`: dense link storage: fractal heap with its root direct block, its persistent free-space manager, the name index's version-2 B-tree | `src/H5Gobj.c` (`H5G_obj_insert`), `src/H5Gdense.c` (`H5G__dense_create`, `H5G__dense_insert`), `src/H5Gbtree2.c` (`H5G__dense_btree2_name_*`), `src/H5HF.c`, `src/H5HFhdr.c` (`H5HF__hdr_create`, `H5HF__hdr_finish_init_phase1/2`), `src/H5HFman.c` (`H5HF__man_insert`), `src/H5HFdblock.c` (`H5HF__man_dblock_new`, `H5HF__man_dblock_create`), `src/H5HFsection.c` (`H5HF__sect_single_reduce`), `src/H5HFspace.c`, `src/H5HFcache.c` (`H5HF__cache_hdr_serialize`, `H5HF__cache_dblock_pre_serialize`), `src/H5FS.c` (`H5FS_create`, `H5FS_close`, `H5FS_delete`), `src/H5FSsection.c` (`H5FS__sect_serialize_size`), `src/H5FScache.c` (`H5FS__cache_hdr_pre_serialize`, `_serialize`, `H5FS__cache_sinfo_serialize`), `src/H5B2.c`, `src/H5B2int.c` (`H5B2__insert`, `H5B2__create_leaf`), `src/H5B2cache.c` (`H5B2__cache_hdr_serialize`, `H5B2__cache_leaf_serialize`), `src/H5Fint.c` (`H5F__dest`, `H5F__flush_phase1`, `H5F__flush_phase2`) |
| `write/mod.rs`: the calls, chunking and the chunk cache, the deflate filter's failure | `src/H5Gobj.c` (`H5G__obj_create_real`), `src/H5Dint.c` (`H5D__update_oh_info`, `H5D_MINHDR_SIZE`), `src/H5Dchunk.c` (`H5D__chunk_flush_entry`, the chunk cache of `H5D_CHUNK_CACHE_NBYTES_DEFAULT`), `src/H5Z.c` (`H5Z_pipeline`, optional filters), `src/H5Zdeflate.c` (`H5Z_DEFLATE_SIZE_ADJUST`), `src/H5Fsuper_cache.c` (superblock version 2) |

Deflate is [zlib-rs](https://github.com/trifectatechfoundation/zlib-rs) (Zlib licence)
through [flate2](https://github.com/rust-lang/flate2-rs) (MIT/Apache-2.0). ITK's HDF5 calls
zlib-ng 2.1.6, whose compressed bytes differ for large chunks (and depend on the CPU); the
streams decompress to the same bytes ([itk-hdf5.md](../../docs/findings/itk-hdf5.md)).
