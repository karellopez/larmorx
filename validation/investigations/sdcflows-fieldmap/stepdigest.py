# SPDX-License-Identifier: Apache-2.0
"""The originals' outputs by digest, for the originals' own step of the parity suite
``sdcflows-fieldmap`` (``larmorx_validation.parity.python_versions``: the shared environment's
releases against the newest).

Every check script passes what sdcflows, scikit-learn, SciPy, numpy or nibabel gave on its side
of a comparison through :func:`oracle` (it returns its argument unchanged); the suite takes the
digests gathered since the previous reported line (:func:`take`) when the script prints its
``OK``/``FAIL`` line, records them in the shared environment and compares them in the newest.
Run by themselves, the scripts only gather the digests.

The digest is of the content: arrays by type, shape and values (every NaN counted as one value,
as the suites compare them), files by their decompressed bytes (never their temporary paths),
images by data, affine and header, sparse matrices by their stored arrays, messages and
exceptions by their text with directories and memory addresses removed.

Each batch also has a second digest, the same content with the NIfTI quaternion fields
(``quatern_b``, ``quatern_c``, ``quatern_d``) of every header and NIfTI file zeroed
(:func:`last_masked`): where the full digests of the two releases differ and these are equal,
only the quaternions nibabel wrote changed, the known step of numpy's LAPACK 3.12.0 to 3.12.1
(``python_versions.CHANGES``; ``docs/findings/sdcflows-fieldmap.md``).
"""

from __future__ import annotations

import gzip
import hashlib
import os
import re
from pathlib import Path

import numpy as np


def numpy_lapack() -> str:
    """The LAPACK release numpy's OpenBLAS bundles: "3.12.1" from OpenBLAS 0.3.31 (numpy
    2.5.4's 0.3.34), else "3.12.0" (numpy 2.3.5's 0.3.30)."""
    version = np.show_config(mode="dicts")["Build Dependencies"]["blas"]["version"]
    return "3.12.1" if tuple(int(x) for x in version.split(".")[:3]) >= (0, 3, 31) else "3.12.0"


def _match_numpy_lapack() -> None:
    """larmorx writes nibabel's qforms with numpy 2.5.4's LAPACK 3.12.1 arithmetic; in an
    environment whose numpy bundles 3.12.0 the comparisons use that release
    (``lx.io.nibabel_header.use_lapack``), as nibabel there does."""
    try:
        from larmorx.io.nibabel_header import use_lapack
    except ImportError:  # an older larmorx
        return
    use_lapack(numpy_lapack())


_match_numpy_lapack()

_PENDING: list[bytes] = []
_PENDING_MASKED: list[bytes] = []
_LAST_MASKED: list[str | None] = [None]
#: The directories of absolute paths (temporary directories differ from run to run).
_DIRS = re.compile(r"(?:/[^/\s'\"(),:;\[\]]+)+/")
_ADDRESS = re.compile(r"0x[0-9a-fA-F]+")


def _text(s: str) -> str:
    return _ADDRESS.sub("0x", _DIRS.sub("", s))


def mask_quaternions(data: bytes) -> bytes:
    """``data`` with the quaternion fields of a NIfTI-1 or NIfTI-2 header at its start zeroed
    (anything else unchanged)."""
    sizes = {int.from_bytes(data[:4], "little"), int.from_bytes(data[:4], "big")}
    if 348 in sizes and len(data) >= 348 and data[344:348] in (b"n+1\0", b"ni1\0"):
        return data[:256] + bytes(12) + data[268:]
    if 540 in sizes and len(data) >= 540 and data[4:8] in (b"n+2\0", b"ni2\0"):
        return data[:352] + bytes(24) + data[376:]
    return data


def _file(path: str) -> bytes | None:
    try:
        if len(path) > 4096 or not os.path.isfile(path):
            return None
        data = Path(path).read_bytes()
    except (OSError, ValueError):
        return None
    return gzip.decompress(data) if data[:2] == b"\x1f\x8b" else data


def _array(h, a: np.ndarray, mask: bool = False) -> None:
    if a.dtype == object:
        h.update(b"O" + str(a.shape).encode())
        for x in a.ravel():
            _update(h, x, mask)
        return
    a = np.ascontiguousarray(a)
    if a.dtype.kind in "fc":
        a = np.where(np.isnan(a), np.array(np.nan, dtype=a.dtype), a)
    h.update(f"A{a.dtype.str}{a.shape}".encode())
    h.update(np.ascontiguousarray(a).tobytes())


def _update(h, obj, mask: bool = False) -> None:
    if obj is None:
        h.update(b"N")
    elif isinstance(obj, bool | np.bool_):
        h.update(b"B1" if obj else b"B0")
    elif isinstance(obj, np.ndarray | np.generic):
        _array(h, np.asarray(obj), mask)
    elif isinstance(obj, int):
        h.update(b"I" + str(obj).encode())
    elif isinstance(obj, float):
        h.update(b"F" + ("nan" if obj != obj else obj.hex()).encode())
    elif isinstance(obj, bytes):
        h.update(b"Y" + (mask_quaternions(obj) if mask else obj))
    elif isinstance(obj, str | os.PathLike):
        s = os.fspath(obj)
        content = _file(s)
        if content is None:
            h.update(b"S" + _text(s).encode())
        else:
            h.update(b"P" + os.path.basename(s).encode() + b"\0")
            h.update(hashlib.sha256(mask_quaternions(content) if mask else content).digest())
    elif isinstance(obj, BaseException):
        h.update(f"E{type(obj).__name__}:{_text(str(obj))}".encode())
    elif hasattr(obj, "message") and hasattr(obj, "category"):  # warnings.WarningMessage
        h.update(f"W{obj.category.__name__}:{_text(str(obj.message))}".encode())
    elif hasattr(obj, "getMessage") and hasattr(obj, "levelname"):  # logging.LogRecord
        h.update(f"L{obj.levelname}:{_text(obj.getMessage())}".encode())
    elif hasattr(obj, "dataobj") and hasattr(obj, "affine") and hasattr(obj, "header"):
        h.update(f"M{type(obj).__name__}".encode())  # a nibabel image
        _array(h, np.asanyarray(obj.dataobj))
        _update(h, None if obj.affine is None else np.asarray(obj.affine))
        block = getattr(obj.header, "binaryblock", None)
        if block is not None and mask:
            block = mask_quaternions(block)
        _update(h, block if block is not None else repr(obj.header))
    elif hasattr(obj, "format") and hasattr(obj, "nnz") and hasattr(obj, "shape"):
        h.update(f"X{obj.format}{obj.shape}".encode())  # a SciPy sparse matrix or array
        for name in ("data", "indices", "indptr", "row", "col", "offsets"):
            if hasattr(obj, name):
                _array(h, np.asarray(getattr(obj, name)))
    elif isinstance(obj, dict):
        h.update(b"D" + str(len(obj)).encode())
        for k in sorted(obj, key=str):
            _update(h, str(k), mask)
            _update(h, obj[k], mask)
    elif isinstance(obj, list | tuple):
        h.update((b"T" if isinstance(obj, tuple) else b"Q") + str(len(obj)).encode())
        for x in obj:
            _update(h, x, mask)
    elif hasattr(obj, "trait_get"):  # nipype's outputs (a TraitedSpec)
        traits = obj.trait_get().items()
        _update(h, {k: v for k, v in traits if not k.startswith("trait_")}, mask)
    else:
        h.update(b"R" + _text(repr(obj)).encode())


def digest(obj, mask: bool = False) -> str:
    """The content's SHA-256 (with ``mask``, the NIfTI quaternion fields zeroed)."""
    h = hashlib.sha256()
    _update(h, obj, mask)
    return h.hexdigest()


def oracle(*objs):
    """Gather the digests of an original's outputs; returns the argument (or the arguments)."""
    for obj in objs:
        _PENDING.append(bytes.fromhex(digest(obj)))
        _PENDING_MASKED.append(bytes.fromhex(digest(obj, mask=True)))
    return objs[0] if len(objs) == 1 else objs


def take() -> str | None:
    """The digest of the outputs gathered since the last call (``None`` when there were none)."""
    if not _PENDING:
        _LAST_MASKED[0] = None
        return None
    out = hashlib.sha256(b"".join(_PENDING)).hexdigest()
    _LAST_MASKED[0] = hashlib.sha256(b"".join(_PENDING_MASKED)).hexdigest()
    _PENDING.clear()
    _PENDING_MASKED.clear()
    return out


def last_masked() -> str | None:
    """The masked digest (quaternion fields zeroed) of the batch the last :func:`take` returned."""
    return _LAST_MASKED[0]


def reset() -> None:
    _PENDING.clear()
    _PENDING_MASKED.clear()
    _LAST_MASKED[0] = None
