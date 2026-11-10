# SPDX-License-Identifier: Apache-2.0
"""The originals' outputs by digest, per comparison, for the originals' own step of the parity
suites ``ndimage-skimage``, ``niworkflows-helpers`` and ``sdcflows-estimation``
(``larmorx_validation.parity.python_versions``: the shared environment's releases, numpy 2.3.5
and SciPy 1.15.3, against the newest, numpy 2.5.4 and SciPy 1.18.1).

A library script passes what SciPy, numpy or scikit-image (or sdcflows' and niworkflows'
functions over them) gave on its side of a comparison through :func:`oracle` (it returns its
argument unchanged), and its tally calls :func:`note` with the comparison's name at each case:
the digests gathered since the previous note go into that name's running digest. At its end the
script calls :func:`lines`, which prints ``STEP <sha256> <name>`` for every name with at least
one gathered output. All of it only when ``LARMORX_STEP_LINES`` is set (the suites set it in
the step's runs); otherwise :func:`oracle` returns at once and nothing is printed, so the
scripts run by themselves as before.

The digest is of the content, as part A's ``stepdigest`` takes it (without its side effect on
larmorx's LAPACK release): arrays by type, shape and values (every NaN counted as one value, as
the suites compare them), files by their decompressed bytes and base name (never their
temporary folders), images by data, affine and header, sequences and mappings element by
element, exceptions, warnings and messages by their text with directories and memory addresses
removed.
"""

from __future__ import annotations

import gzip
import hashlib
import os
import re
from pathlib import Path

import numpy as np

ENV = "LARMORX_STEP_LINES"
#: A file to which every noted case is written, one JSON line each (the comparison's name, the
#: case's number within it, its digest and a short text of the originals' outputs): what
#: ``step_diff.py`` compares between the two environments to find each change's first case.
DUMP = "LARMORX_STEP_DUMP"
ENABLED = bool(os.environ.get(ENV) or os.environ.get(DUMP))

_PENDING: list[bytes] = []
_SHOWN: list[str] = []
_NAMES: dict[str, hashlib._Hash] = {}
_COUNTS: dict[str, int] = {}
#: The directories of absolute paths (temporary directories differ from run to run).
_DIRS = re.compile(r"(?:/[^/\s'\"(),:;\[\]]+)+/")
_ADDRESS = re.compile(r"0x[0-9a-fA-F]+")


def _text(s: str) -> str:
    return _ADDRESS.sub("0x", _DIRS.sub("", s))


def _file(path: str) -> bytes | None:
    try:
        if len(path) > 4096 or not os.path.isfile(path):
            return None
        data = Path(path).read_bytes()
    except (OSError, ValueError):
        return None
    return gzip.decompress(data) if data[:2] == b"\x1f\x8b" else data


def _value_bytes(a: np.ndarray) -> bytes:
    """The array's bytes, without x86-64's long-double padding: an 80-bit extended value is
    stored in 16 bytes whose last 6 are never written (memory's), so only the first 10 of each
    (real or imaginary) part are taken."""
    if a.dtype.kind in "fc" and a.dtype.itemsize in (16, 32) and np.finfo(a.dtype).nmant == 63:
        parts = a.view(np.uint8).reshape(-1, 16)
        return parts[:, :10].tobytes()
    return a.tobytes()


def _update(h, obj) -> None:
    if obj is None:
        h.update(b"N")
    elif isinstance(obj, bool | np.bool_):
        h.update(b"B1" if obj else b"B0")
    elif isinstance(obj, np.ndarray | np.generic):
        a = np.asarray(obj)
        if a.dtype == object:
            h.update(b"O" + str(a.shape).encode())
            for x in a.ravel():
                _update(h, x)
            return
        a = np.ascontiguousarray(a)
        if a.dtype.kind in "fc":
            a = np.where(np.isnan(a), np.array(np.nan, dtype=a.dtype), a)
        h.update(f"A{a.dtype.str}{a.shape}".encode())
        h.update(_value_bytes(np.ascontiguousarray(a)))
    elif isinstance(obj, int):
        h.update(b"I" + str(obj).encode())
    elif isinstance(obj, float):
        h.update(b"F" + ("nan" if obj != obj else obj.hex()).encode())
    elif isinstance(obj, complex):
        h.update(b"C")
        _update(h, obj.real)
        _update(h, obj.imag)
    elif isinstance(obj, bytes):
        h.update(b"Y" + obj)
    elif isinstance(obj, str | os.PathLike):
        s = os.fspath(obj)
        content = _file(s)
        if content is None:
            h.update(b"S" + _text(s).encode())
        else:
            h.update(b"P" + os.path.basename(s).encode() + b"\0")
            h.update(hashlib.sha256(content).digest())
    elif isinstance(obj, BaseException):
        h.update(f"E{type(obj).__name__}:{_text(str(obj))}".encode())
    elif hasattr(obj, "message") and hasattr(obj, "category"):  # warnings.WarningMessage
        h.update(f"W{obj.category.__name__}:{_text(str(obj.message))}".encode())
    elif hasattr(obj, "dataobj") and hasattr(obj, "affine") and hasattr(obj, "header"):
        h.update(b"M")
        _update(h, np.asanyarray(obj.dataobj))
        _update(h, np.asarray(obj.affine))
        _update(h, obj.header.binaryblock if hasattr(obj.header, "binaryblock") else None)
    elif isinstance(obj, dict):
        h.update(b"D" + str(len(obj)).encode())
        for k in sorted(obj, key=repr):
            _update(h, repr(k))
            _update(h, obj[k])
    elif isinstance(obj, list | tuple | set | frozenset):
        items = sorted(obj, key=repr) if isinstance(obj, set | frozenset) else obj
        h.update(f"L{type(obj).__name__}{len(items)}".encode())
        for x in items:
            _update(h, x)
    elif isinstance(obj, type):
        h.update(f"T{obj.__module__}.{obj.__qualname__}".encode())
    else:
        h.update(b"R" + _text(repr(obj)).encode())


def oracle(obj):
    """``obj`` unchanged; its digest is gathered for the next :func:`note` (step runs only)."""
    if ENABLED:
        h = hashlib.sha256()
        _update(h, obj)
        _PENDING.append(h.digest())
        if os.environ.get(DUMP):
            _SHOWN.append(_text(_short(obj))[:400])
    return obj


def _short(obj) -> str:
    if isinstance(obj, np.ndarray):
        flat = np.ravel(obj)[:8]
        return f"array({obj.dtype.str}, {obj.shape}, {flat.tolist()!r}...)"
    if isinstance(obj, list | tuple):
        return "(" + ", ".join(_short(x) for x in obj[:6]) + ")"
    return repr(obj)


def note(name: str) -> None:
    """The comparison ``name`` was counted once: the outputs gathered since the previous note
    go into its digest (none gathered: nothing; the comparison then has no step line)."""
    if not ENABLED or not _PENDING:
        _PENDING.clear()
        _SHOWN.clear()
        return
    h = _NAMES.get(name)
    if h is None:
        h = _NAMES[name] = hashlib.sha256(name.encode())
    for d in _PENDING:
        h.update(d)
    k = _COUNTS[name] = _COUNTS.get(name, 0) + 1
    if dump := os.environ.get(DUMP):
        import json

        case = hashlib.sha256(b"".join(_PENDING)).hexdigest()[:16]
        with open(dump, "a", encoding="utf-8") as f:
            f.write(json.dumps({"name": name, "k": k, "digest": case, "shown": _SHOWN}) + "\n")
    _PENDING.clear()
    _SHOWN.clear()


class Noted:
    """A tally whose every ``add(name, ...)`` also notes ``name`` (for the scripts that use
    ``check_workflows.Tally``)."""

    def __init__(self, tally) -> None:
        self._tally = tally

    def add(self, name: str, *args, **kw):
        note(name)
        return self._tally.add(name, *args, **kw)

    def __getattr__(self, attr):
        return getattr(self._tally, attr)


def lines() -> None:
    """Print ``STEP <sha256> <name>`` per noted comparison (step runs only)."""
    if not os.environ.get(ENV):
        return
    for name in sorted(_NAMES):
        print(f"STEP {_NAMES[name].hexdigest()} {name}")
