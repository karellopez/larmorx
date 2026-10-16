#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Writes the HDF5 fixtures of the golden tests (``tests/golden/hdf5/*.h5.gz``).

larmorx's HDF5 reader (crate ``larmorx-hdf5``) covers storage features that its own writer
never produces (it replays ITK's calls only): the old superblocks and version-1 object headers,
symbol-table groups, every chunk index of file format 1.10, the shuffle, Fletcher-32, n-bit and
scale-offset filters, floating-point formats other than IEEE single and double, fixed-length
strings, dense groups whose fractal heaps have indirect blocks, huge objects or I/O filters,
creation-order indexes, and the file's shared-message heaps. Golden cases cannot generate such
files on every platform (only the HDF5 C library writes them), so this script writes tiny
synthetic ones, committed to the repository (CLAUDE.md rule 10, the exception for format
fixtures; ``docs/validation/golden.md``, "HDF5 fixtures").

The files hold no image data: small arrays of seeded integers and their quotients. They are
written with h5py's low-level API, with object times off (``track_times``), so the same h5py
and HDF5 versions give the same bytes, and stored gzip-compressed (``mtime=0``, no name),
because some layouts (a paged extensible array, a huge object) allocate kilobytes of mostly
empty space. ``--check`` writes them again into a temporary directory and compares the
decompressed bytes with the committed ones.

Run with the workspace venv (h5py 3.16.0 on HDF5 2.0.0 made the committed files)::

    python scripts/make_hdf5_fixtures.py            # writes tests/golden/hdf5/
    python scripts/make_hdf5_fixtures.py --check    # compares with the committed files
"""

from __future__ import annotations

import argparse
import ctypes
import glob
import gzip
import hashlib
import io
import sys
import tempfile
import time
from collections.abc import Callable
from pathlib import Path

import numpy as np

OUT = Path(__file__).resolve().parents[1] / "tests" / "golden" / "hdf5"


def _values(seed: int, n: int) -> np.ndarray:
    """``n`` integers in ``[-500, 500)`` from SplitMix64 started at ``seed`` (as the golden
    registry draws its inputs)."""
    u = np.uint64
    x = np.arange(1, n + 1, dtype=u) * u(0x9E3779B97F4A7C15) + u(seed)
    x = (x ^ (x >> u(30))) * u(0xBF58476D1CE4E5B9)
    x = (x ^ (x >> u(27))) * u(0x94D049BB133111EB)
    x = x ^ (x >> u(31))
    return (x % u(1000)).astype(np.int64) - 500


def _floats(seed: int, n: int) -> np.ndarray:
    """``n`` quarters in ``[-125, 125)``: exact in every format below."""
    return _values(seed, n) / 4.0


def _lib() -> ctypes.CDLL:
    """h5py's own HDF5 library, for the property-list calls h5py does not wrap."""
    import h5py

    here = Path(h5py.__file__).parent
    found = [
        p
        for p in sorted(glob.glob(str(here.parent / "h5py.libs" / "libhdf5-*")))
        + sorted(glob.glob(str(here / ".dylibs" / "libhdf5.*")))
        if "_hl" not in Path(p).name
    ]
    if not found:
        raise RuntimeError("h5py's HDF5 library not found")
    lib = ctypes.CDLL(found[0])
    i64, uint = ctypes.c_int64, ctypes.c_uint
    lib.H5Pset_link_phase_change.argtypes = [i64, uint, uint]
    lib.H5Pset_shared_mesg_nindexes.argtypes = [i64, uint]
    lib.H5Pset_shared_mesg_index.argtypes = [i64, uint, uint, uint]
    lib.H5Pset_shared_mesg_phase_change.argtypes = [i64, uint, uint]
    lib.H5Pset_deflate.argtypes = [i64, uint]
    lib.H5Pset_filter.argtypes = [i64, ctypes.c_int, uint, ctypes.c_size_t, ctypes.c_void_p]
    lib.H5Pset_istore_k.argtypes = [i64, uint]
    return lib


def _check(status: int, call: str) -> None:
    if status < 0:
        raise RuntimeError(f"{call} failed")


def _create(path: Path, low: int, high: int, fcpl=None):
    """A new file with these format bounds; the root group without object times."""
    import h5py

    fapl = h5py.h5p.create(h5py.h5p.FILE_ACCESS)
    fapl.set_libver_bounds(low, high)
    if fcpl is None:
        fcpl = h5py.h5p.create(h5py.h5p.FILE_CREATE)
    fcpl.set_obj_track_times(False)
    return h5py.h5f.create(str(path).encode(), h5py.h5f.ACC_TRUNC, fcpl=fcpl, fapl=fapl)


def _gcpl():
    import h5py

    gcpl = h5py.h5p.create(h5py.h5p.GROUP_CREATE)
    gcpl.set_obj_track_times(False)
    return gcpl


def _group(parent, name: str, gcpl=None):
    import h5py

    return h5py.h5g.create(parent, name.encode(), gcpl=gcpl or _gcpl())


def _dataset(
    parent,
    name: str,
    data: np.ndarray | None,
    *,
    ftype=None,
    shape: tuple[int, ...] | None = None,
    maxshape: tuple[int | None, ...] | None = None,
    chunks: tuple[int, ...] | None = None,
    filters: tuple[tuple[int, int, tuple[int, ...]], ...] = (),
    fill: np.ndarray | None = None,
    layout: str = "",
    write: tuple[slice, ...] | None = None,
):
    """A dataset written with the low-level API: ``ftype`` in the file (default: ``data``'s
    type), chunked with ``filters`` (``(id, flags, values)``), compact or allocated early as
    ``layout`` says, written whole or only at ``write``."""
    import h5py

    if ftype is None:
        ftype = h5py.h5t.py_create(data.dtype)
    shape = data.shape if shape is None else shape
    dims_max = (
        None
        if maxshape is None
        else tuple(h5py.h5s.UNLIMITED if m is None else m for m in maxshape)
    )
    space = (
        h5py.h5s.create_simple(shape, dims_max) if shape != () else h5py.h5s.create(h5py.h5s.SCALAR)
    )
    dcpl = h5py.h5p.create(h5py.h5p.DATASET_CREATE)
    dcpl.set_obj_track_times(False)
    if chunks is not None:
        dcpl.set_chunk(chunks)
    for fid, flags, values in filters:
        dcpl.set_filter(fid, flags, values)
    if fill is not None:
        dcpl.set_fill_value(fill)
    if layout == "compact":
        dcpl.set_layout(h5py.h5d.COMPACT)
    elif layout == "early":
        dcpl.set_alloc_time(h5py.h5d.ALLOC_TIME_EARLY)
    dset = h5py.h5d.create(parent, name.encode(), ftype, space, dcpl=dcpl)
    if data is not None:
        mtype = h5py.h5t.py_create(data.dtype)
        if write is None:
            dset.write(h5py.h5s.ALL, h5py.h5s.ALL, np.ascontiguousarray(data), mtype=mtype)
        else:
            fspace = dset.get_space()
            start = tuple(s.start for s in write)
            count = tuple(s.stop - s.start for s in write)
            fspace.select_hyperslab(start, count)
            part = np.ascontiguousarray(data[write])
            dset.write(h5py.h5s.create_simple(count), fspace, part, mtype=mtype)
    return dset


def _float_type(
    size: int,
    precision: int,
    fields: tuple[int, int, int, int, int],
    ebias: int,
    norm: int,
    order: int,
    offset: int = 0,
):
    """A floating-point file type of any layout (HDF5's ``H5Tset_fields`` and friends):
    ``fields`` are (sign, exponent, exponent bits, mantissa, mantissa bits) positions."""
    import h5py

    t = (h5py.h5t.IEEE_F32LE if size <= 4 else h5py.h5t.IEEE_F64LE).copy()
    if size <= t.get_size():  # HDF5 checks that the fields fit: shrink them first
        t.set_fields(*fields)
        t.set_offset(offset)
        t.set_precision(precision)
        t.set_size(size)
    else:
        t.set_size(size)
        t.set_precision(precision)
        t.set_offset(offset)
        t.set_fields(*fields)
    t.set_ebias(ebias)
    t.set_norm(norm)
    t.set_order(order)
    return t


def _strings(parent, name: str, values: list[bytes], size: int, pad: int, cset: int) -> None:
    """Fixed-length strings of ``size`` bytes, padded as ``pad`` says."""
    import h5py

    t = h5py.h5t.C_S1.copy()
    t.set_size(size)
    t.set_strpad(pad)
    t.set_cset(cset)
    data = np.array(values, dtype=f"S{size}")
    _dataset(parent, name, data, ftype=t)


def _vlen_strings(parent, name: str, values: list[str], cset: int) -> None:
    import h5py

    t = h5py.h5t.C_S1.copy()
    t.set_size(h5py.h5t.VARIABLE)
    t.set_cset(cset)
    data = np.array(values, dtype=h5py.string_dtype("utf-8" if cset else "ascii"))
    _dataset(parent, name, data, ftype=t)


# ------------------------------------------------------------------------------------------------
# The files


def sb0(path: Path) -> None:
    """Superblock 0 (h5py's default bounds), version-1 object headers, symbol-table groups (a
    group of 12 entries over two B-tree leaves, soft links in the local heap), a dataset whose
    attributes spill into a continuation chunk, a big-endian double, half, x87 extended and
    IEEE quadruple floats, fixed-length strings in each padding, variable-length strings in
    the global heap, a chunked dataset with shuffle, Fletcher-32 and deflate under a version-1
    B-tree with a chunk never written (the fill value), a compact dataset, a scalar, and a
    committed datatype; a soft link to the group."""
    import h5py

    t = h5py.h5t
    fid = _create(path, h5py.h5f.LIBVER_EARLIEST, h5py.h5f.LIBVER_V18)
    try:
        g = _group(fid, "g")
        for i in range(4):
            _dataset(g, f"d{i:02d}", _floats(10 + i, 3))
        for i in range(4, 12):
            g.links.create_soft(f"s{i:02d}".encode(), f"/g/d{i % 4:02d}".encode())
        fid.links.create_soft(b"soft_g", b"/g")
        _dataset(fid, "be", _floats(1, 12).reshape(3, 4), ftype=t.IEEE_F64BE)
        _dataset(
            fid,
            "half",
            _floats(2, 6),
            ftype=_float_type(2, 16, (15, 10, 5, 0, 10), 15, t.NORM_IMPLIED, t.ORDER_LE),
        )
        _dataset(
            fid,
            "x87",
            _floats(3, 5),
            ftype=_float_type(16, 80, (79, 64, 15, 0, 64), 16383, t.NORM_NONE, t.ORDER_LE),
        )
        _dataset(
            fid,
            "quad",
            _floats(4, 5),
            ftype=_float_type(16, 128, (127, 112, 15, 0, 112), 16383, t.NORM_IMPLIED, t.ORDER_LE),
        )
        _dataset(
            fid,
            "quad_be",
            _floats(5, 3),
            ftype=_float_type(16, 128, (127, 112, 15, 0, 112), 16383, t.NORM_IMPLIED, t.ORDER_BE),
        )
        words = [b"Affine", b"Rigid", b"", b"SyN"]
        _strings(fid, "s_nullterm", words, 7, t.STR_NULLTERM, t.CSET_ASCII)
        _strings(fid, "s_nullpad", words, 6, t.STR_NULLPAD, t.CSET_ASCII)
        _strings(fid, "s_spacepad", words, 8, t.STR_SPACEPAD, t.CSET_ASCII)
        _vlen_strings(fid, "s_vlen", ["CompositeTransform_double_3_3", "", "x"], t.CSET_ASCII)
        _dataset(
            fid,
            "chunked",
            _floats(6, 12).astype(np.float32),
            chunks=(5,),
            filters=((2, 0, (4,)), (3, 0, ()), (1, 0, (6,))),
            fill=np.array([-1.5], dtype=np.float32),
            write=(slice(0, 7),),
        )
        _dataset(fid, "compact", _floats(7, 6), layout="compact")
        _dataset(fid, "scalar", np.array(_floats(8, 1)[0]), shape=())
        named = t.IEEE_F32BE.copy()
        named.commit(fid, b"t_f32be")
        _dataset(fid, "named", _floats(9, 4).astype(np.float32), ftype=named)
        d = _dataset(fid, "attrs", _floats(20, 4))
        for i in range(40):
            h5py.h5a.create(
                d, f"attribute_{i:02d}".encode(), t.STD_I32LE, h5py.h5s.create(h5py.h5s.SCALAR)
            ).write(np.array(i, dtype="<i4"))
    finally:
        fid.close()


def sb1(path: Path) -> None:
    """Superblock 1 (a chunk B-tree K of 64 instead of 32) with a chunked dataset."""
    import h5py

    fcpl = h5py.h5p.create(h5py.h5p.FILE_CREATE)
    _check(_lib().H5Pset_istore_k(fcpl.id, 64), "H5Pset_istore_k")
    fid = _create(path, h5py.h5f.LIBVER_EARLIEST, h5py.h5f.LIBVER_V18, fcpl)
    try:
        _dataset(fid, "x", _floats(30, 20), chunks=(3,))
    finally:
        fid.close()


def sb3_datasets(path: Path) -> None:
    """Superblock 3 (file format 1.10), version-2 object headers, and every chunk index of that
    format: a single chunk (filtered or not), implicit, fixed array (unpaged; paged and
    filtered, one page written), extensible array (elements in the index block, in data
    blocks, through a super block with filters, in a page of a paged data block, with the
    unlimited dimension second), version-2 B-tree (unfiltered with an internal node;
    filtered); the n-bit filter on a custom float and on little- and big-endian integers;
    the scale-offset filter on floats (D-scale) and integers (with a fill value); UTF-8
    strings; a chunk never written; a compact dataset; a committed datatype; soft links to a
    group and to a dataset."""
    import h5py

    t = h5py.h5t
    fid = _create(path, h5py.h5f.LIBVER_V110, h5py.h5f.LIBVER_V112)
    gz = ((1, 0, (6,)),)
    try:
        _dataset(fid, "single", _floats(40, 63).reshape(7, 9), chunks=(7, 9))
        _dataset(fid, "single_deflate", _floats(41, 63).reshape(7, 9), chunks=(7, 9), filters=gz)
        _dataset(fid, "implicit", _floats(42, 48).reshape(8, 6), chunks=(2, 3), layout="early")
        _dataset(fid, "farray", _floats(43, 48).reshape(8, 6), chunks=(2, 3))
        paged = (_values(44, 2100) % 200).astype(np.uint8)
        _dataset(
            fid, "farray_paged_deflate", paged, chunks=(1,), filters=gz, write=(slice(2060, 2100),)
        )
        _dataset(fid, "earray_iblock", _floats(45, 3), maxshape=(None,), chunks=(1,))
        ea = (_values(46, 131100) % 200).astype(np.uint8)
        _dataset(
            fid, "earray_dblock", ea[:40], maxshape=(None,), chunks=(1,), write=(slice(10, 40),)
        )
        _dataset(
            fid,
            "earray_sblock_deflate",
            ea[:400],
            maxshape=(None,),
            chunks=(1,),
            filters=gz,
            write=(slice(296, 310),),
        )
        _dataset(
            fid, "earray_paged", ea, maxshape=(None,), chunks=(1,), write=(slice(131064, 131070),)
        )
        _dataset(
            fid, "earray_2d", _floats(47, 36).reshape(3, 12), maxshape=(3, None), chunks=(2, 3)
        )
        _dataset(
            fid,
            "bt2",
            (_values(48, 90) % 200).astype(np.uint8).reshape(90, 1),
            maxshape=(None, None),
            chunks=(1, 1),
        )
        _dataset(
            fid,
            "bt2_deflate",
            _floats(49, 60).reshape(6, 10),
            maxshape=(None, None),
            chunks=(2, 5),
            filters=gz,
        )
        nbit_float = _float_type(4, 20, (26, 20, 6, 7, 13), 31, t.NORM_IMPLIED, t.ORDER_LE, 7)
        _dataset(
            fid,
            "nbit_float",
            (_floats(50, 100) / 4).astype(np.float32),
            ftype=nbit_float,
            chunks=(50,),
            filters=((5, 1, ()),),
        )
        for name, base in (("nbit_int", t.STD_I32LE), ("nbit_int_be", t.STD_I32BE)):
            ft = base.copy()
            ft.set_precision(12)
            ft.set_offset(4)
            _dataset(
                fid,
                name,
                (_values(51, 100) * 3).astype(np.int32),
                ftype=ft,
                chunks=(50,),
                filters=((5, 1, ()),),
            )
        # Scale-offset: (scale type, scale factor); 0 = float D-scale, 2 = integer.
        _dataset(
            fid,
            "so_f32_d2",
            (_floats(52, 100) / 3).astype(np.float32),
            chunks=(40,),
            filters=((6, 1, (0, 2)),),
        )
        _dataset(
            fid,
            "so_f64be_d3",
            _floats(53, 100) / 7,
            ftype=t.IEEE_F64BE,
            chunks=(40,),
            filters=((6, 1, (0, 3)),),
        )
        _dataset(
            fid,
            "so_i16",
            (_values(54, 100) * 5).astype(np.int16),
            chunks=(40,),
            filters=((6, 1, (2, 0)),),
        )
        _dataset(
            fid,
            "so_u64",
            (_values(55, 100) + 500).astype(np.uint64) * np.uint64(1 << 40),
            chunks=(40,),
            filters=((6, 1, (2, 0)),),
        )
        _dataset(
            fid,
            "so_i32_fill",
            (_values(56, 100) * 11).astype(np.int32),
            chunks=(40,),
            filters=((6, 1, (2, 0)),),
            fill=np.array([12345], np.int32),
            write=(slice(20, 70),),
        )
        _dataset(
            fid,
            "shuffle_fletcher",
            _floats(57, 30),
            chunks=(8,),
            filters=((2, 0, (8,)), (3, 0, ())),
        )
        _dataset(
            fid, "fill", _floats(58, 10), chunks=(5,), fill=np.array([7.5]), write=(slice(0, 5),)
        )
        _dataset(fid, "compact", _floats(59, 4), layout="compact")
        _vlen_strings(fid, "utf8", ["Transformación", "変換", ""], t.CSET_UTF8)
        named = t.IEEE_F64BE.copy()
        named.commit(fid, b"t_f64be")
        grp = _group(fid, "grp")
        _dataset(grp, "named", _floats(61, 5), ftype=named)
        grp.links.create_soft(b"soft_single", b"/single")
        fid.links.create_soft(b"soft_grp", b"/grp")
    finally:
        fid.close()


def sb3_groups(path: Path) -> None:
    """Dense groups (links in a fractal heap, a version-2 B-tree of names): 60 links and a
    name of 4,200 characters (more than the 4 KiB a heap block may hold: a huge object) in
    an unfiltered heap whose root became an indirect block, the same with deflate on the heap
    (filtered direct blocks and a filtered huge object), a heap filtered by deflate and
    Fletcher-32, and a group whose links are indexed by creation order too."""
    import h5py

    lib = _lib()
    fid = _create(path, h5py.h5f.LIBVER_V110, h5py.h5f.LIBVER_V112)
    try:
        _dataset(fid, "x", _floats(60, 3))
        for name, filters, n, huge, order in (
            ("plain", (), 60, True, False),
            ("deflate", (1,), 60, True, False),
            ("deflate_fletcher", (1, 3), 12, False, False),
            ("order", (), 12, False, True),
        ):
            gcpl = _gcpl()
            _check(lib.H5Pset_link_phase_change(gcpl.id, 8, 6), "H5Pset_link_phase_change")
            if order:
                gcpl.set_link_creation_order(
                    h5py.h5p.CRT_ORDER_TRACKED | h5py.h5p.CRT_ORDER_INDEXED
                )
            for f in filters:
                if f == 1:
                    _check(lib.H5Pset_deflate(gcpl.id, 6), "H5Pset_deflate")
                else:
                    _check(lib.H5Pset_filter(gcpl.id, f, 0, 0, None), "H5Pset_filter")
            g = _group(fid, name, gcpl)
            for i in range(n):
                # In order 11, 10, ..., so that creation order and name order differ.
                k = n - 1 - i if order else i
                g.links.create_hard(f"l{k:02d}".encode(), fid, b"x")
            if huge:
                g.links.create_hard(b"h" * 4200, fid, b"x")
    finally:
        fid.close()


def sohm(path: Path) -> None:
    """A transform file whose filter pipeline messages (three distinct ones) are shared in a
    list index and whose datatype, dataspace and fill value messages in a B-tree index (more
    than 4 messages turn a list into a B-tree), in the file's shared-message heap
    (``H5Pset_shared_mesg_*``), over 8 transforms of distinct sizes and types."""
    import h5py

    lib = _lib()
    sdspace, dtype, fill, pline = 1 << 1, 1 << 3, 1 << 5, 1 << 11
    fcpl = h5py.h5p.create(h5py.h5p.FILE_CREATE)
    _check(lib.H5Pset_shared_mesg_nindexes(fcpl.id, 2), "H5Pset_shared_mesg_nindexes")
    _check(lib.H5Pset_shared_mesg_index(fcpl.id, 0, pline, 0), "H5Pset_shared_mesg_index")
    _check(
        lib.H5Pset_shared_mesg_index(fcpl.id, 1, dtype | sdspace | fill, 0),
        "H5Pset_shared_mesg_index",
    )
    _check(lib.H5Pset_shared_mesg_phase_change(fcpl.id, 4, 2), "H5Pset_shared_mesg_phase_change")
    fid = _create(path, h5py.h5f.LIBVER_V110, h5py.h5f.LIBVER_V112, fcpl)
    t = h5py.h5t
    try:
        tg = _group(fid, "TransformGroup")
        for i in range(8):
            g = _group(tg, str(i))
            _vlen_strings(g, "TransformType", ["AffineTransform_double_3_3"], t.CSET_ASCII)
            ftype = (t.IEEE_F64LE, t.IEEE_F32BE, t.IEEE_F32LE, t.IEEE_F64BE)[i % 4]
            filters = ((1, 0, (6,)),) if i % 2 else ()
            if i % 3 == 0:
                filters = ((2, 0, (8,)), *filters)
            _dataset(
                g,
                "TransformParameters",
                _floats(70 + i, 12 + i),
                ftype=ftype,
                chunks=(6,),
                filters=filters,
                fill=np.array([float(i)]),
            )
            _dataset(g, "TransformFixedParameters", _floats(90 + i, 3))
    finally:
        fid.close()


FIXTURES: dict[str, Callable[[Path], None]] = {
    "sb0.h5": sb0,
    "sb1.h5": sb1,
    "sb3-datasets.h5": sb3_datasets,
    "sb3-groups.h5": sb3_groups,
    "sohm.h5": sohm,
}


def _gzip(data: bytes) -> bytes:
    buf = io.BytesIO()
    with gzip.GzipFile(filename="", mode="wb", fileobj=buf, mtime=0, compresslevel=9) as z:
        z.write(data)
    return buf.getvalue()


_M = 0xFFFFFFFF


def _rot(x: int, k: int) -> int:
    return ((x << k) | (x >> (32 - k))) & _M


def lookup3(data: bytes, initval: int = 0) -> int:
    """Bob Jenkins' lookup3 ``hashlittle``, as HDF5 checksums its metadata
    (``H5_checksum_lookup3``)."""
    n = len(data)
    a = b = c = (0xDEADBEEF + n + initval) & _M
    i = 0
    while n > 12:
        a = (a + int.from_bytes(data[i : i + 4], "little")) & _M
        b = (b + int.from_bytes(data[i + 4 : i + 8], "little")) & _M
        c = (c + int.from_bytes(data[i + 8 : i + 12], "little")) & _M
        a = (a - c) & _M ^ _rot(c, 4)
        c = (c + b) & _M
        b = (b - a) & _M ^ _rot(a, 6)
        a = (a + c) & _M
        c = (c - b) & _M ^ _rot(b, 8)
        b = (b + a) & _M
        a = (a - c) & _M ^ _rot(c, 16)
        c = (c + b) & _M
        b = (b - a) & _M ^ _rot(a, 19)
        a = (a + c) & _M
        c = (c - b) & _M ^ _rot(b, 4)
        b = (b + a) & _M
        i += 12
        n -= 12
    if n == 0:
        return c
    tail = data[i:] + bytes(12 - n)
    a = (a + int.from_bytes(tail[0:4], "little")) & _M
    b = (b + int.from_bytes(tail[4:8], "little")) & _M
    c = (c + int.from_bytes(tail[8:12], "little")) & _M
    c = (c ^ b) - _rot(b, 14) & _M
    a = (a ^ c) - _rot(c, 11) & _M
    b = (b ^ a) - _rot(a, 25) & _M
    c = (c ^ b) - _rot(b, 16) & _M
    a = (a ^ c) - _rot(c, 4) & _M
    b = (b ^ a) - _rot(a, 14) & _M
    c = (c ^ b) - _rot(b, 24) & _M
    return c


def clear_times(data: bytes) -> bytes:
    """The file with the times of every version-2 object header set to 0 and the header's
    checksum recomputed. HDF5 records them for objects it creates with a default property
    list (the superblock extension of a file with shared messages), whatever
    ``track_times`` says. A header is changed only if its checksum checks, so that bytes of
    data that happen to read ``OHDR`` are never touched."""
    out = bytearray(data)
    at = out.find(b"OHDR")
    while at >= 0:
        flags = out[at + 5] if at + 6 <= len(out) else 0
        if out[at + 4] == 2 and flags & 0x20:
            size_len = 1 << (flags & 3)
            pos = at + 6 + 16 + (4 if flags & 0x10 else 0)
            size = int.from_bytes(out[pos : pos + size_len], "little")
            end = pos + size_len + size
            if end + 4 <= len(out) and lookup3(bytes(out[at:end])) == int.from_bytes(
                out[end : end + 4], "little"
            ):
                out[at + 6 : at + 22] = bytes(16)
                out[end : end + 4] = lookup3(bytes(out[at:end])).to_bytes(4, "little")
        at = out.find(b"OHDR", at + 1)
    return bytes(out)


def build(directory: Path) -> dict[str, bytes]:
    """Every fixture's bytes, written in ``directory``."""
    out = {}
    for name, make in FIXTURES.items():
        path = directory / name
        make(path)
        out[name] = clear_times(path.read_bytes())
    return out


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--check", action="store_true", help="compare with the committed files")
    parser.add_argument("--out", type=Path, default=OUT)
    args = parser.parse_args(argv)
    import h5py

    print(f"h5py {h5py.version.version}, HDF5 {h5py.version.hdf5_version}")
    with tempfile.TemporaryDirectory() as tmp:
        files = build(Path(tmp))
        time.sleep(1.1)  # HDF5 records object times in seconds
        again = build(Path(tmp))
    for name in files:
        if files[name] != again[name]:
            print(f"{name}: two runs give different bytes", file=sys.stderr)
            return 1
    status = 0
    for name, data in files.items():
        target = args.out / f"{name}.gz"
        digest = hashlib.sha256(data).hexdigest()
        if args.check:
            have = gzip.decompress(target.read_bytes()) if target.exists() else None
            same = have == data
            status |= not same
            print(
                f"{name}: {len(data)} bytes, sha256 {digest[:16]}: {'same' if same else 'DIFFERS'}"
            )
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            packed = _gzip(data)
            target.write_bytes(packed)
            print(f"{name}: {len(data)} bytes ({len(packed)} compressed), sha256 {digest[:16]}")
    return status


if __name__ == "__main__":
    raise SystemExit(main())
