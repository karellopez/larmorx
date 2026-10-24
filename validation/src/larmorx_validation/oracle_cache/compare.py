# SPDX-License-Identifier: Apache-2.0
"""Whether two runs of an original program gave the same result.

Used when a run is compared with its stored entry (``refresh``, ``verify``, the confirmations of
runs above one thread, two workers storing the same key). What is served from the cache is
always the stored bytes; this only decides whether a fresh run differs, ignoring what changes
from run to run without being part of the result:

- in the messages, memory addresses (``0x...``) and the numbers of elapsed and CPU times
  (``Elapsed time: ...``, ``Elapsed = ... sec``), plus a run's own ``volatile`` patterns;
- in the outputs, the compression of ``.gz`` files (their decompressed bytes are compared) and
  the bookkeeping of HDF5 files (their datasets and attributes are compared, not the bytes:
  HDF5 stamps objects with times).

Everything else must be the same bytes: the exit status, the messages, the set of files written
and every file's contents.
"""

from __future__ import annotations

import gzip
import re
from collections.abc import Sequence
from pathlib import Path

import numpy as np

VOLATILE = (
    r"\b0x[0-9a-fA-F]+\b",
    r"(?i)(?<=elapsed time)(\s*\(stage \d+\))?\s*:\s*[0-9.eE+-]+(\s*s(ec)?)?",
    r"(?i)(?<=total elapsed time)\s*:\s*[0-9.eE+-]+",
    r"(?<=Elapsed = )[0-9.]+(?= sec)",
    r"(?i)(?<=cpu time)\s*[=:]\s*[0-9.eE+-]+",
    r"(?i)(?<=clock time)\s*[=:]\s*[0-9.eE+-]+",
)


def mask(text: bytes, volatile: Sequence[str] = ()) -> bytes:
    """``text`` with what changes from run to run replaced by ``…``."""
    s = text.decode("utf-8", "replace")
    for pattern in (*VOLATILE, *volatile):
        s = re.sub(pattern, "…", s)
    return s.encode()


def first_line_difference(a: bytes, b: bytes) -> str:
    la, lb = a.decode("utf-8", "replace").splitlines(), b.decode("utf-8", "replace").splitlines()
    for i in range(max(len(la), len(lb))):
        x = la[i] if i < len(la) else "<none>"
        y = lb[i] if i < len(lb) else "<none>"
        if x != y:
            return f"line {i + 1}: stored {x[:160]!r}, now {y[:160]!r}"
    return "the same lines"


def _h5_content(path: Path) -> list:
    import h5py

    items: list = []

    def attrs(obj) -> list:
        out = []
        for k in sorted(obj.attrs):
            v = obj.attrs[k]
            out.append((k, np.asarray(v).tobytes() if not isinstance(v, (str, bytes)) else v))
        return out

    with h5py.File(path, "r") as f:
        items.append(("/", attrs(f)))

        def visit(name, obj):
            if isinstance(obj, h5py.Dataset):
                value = obj[()]
                data = value if isinstance(value, (bytes, str)) else np.asarray(value).tobytes()
                items.append((name, str(obj.dtype), obj.shape, data, attrs(obj)))
            else:
                items.append((name, "group", attrs(obj)))

        f.visititems(visit)
    return items


def without_extensions(data: bytes) -> bytes | None:
    """A NIfTI-1 file's header (without ``vox_offset``) and data, its extensions left out;
    ``None`` if ``data`` is not a single-file NIfTI-1."""
    if len(data) < 352 or data[344:348] != b"n+1\x00":
        return None
    order = "<" if int.from_bytes(data[:4], "little") == 348 else ">"
    offset = int(np.frombuffer(data[108:112], dtype=f"{order}f4")[0])
    return data[:108] + data[112:348] + data[offset:]


def same_file(stored: Path, fresh: Path, *, nifti_extensions: bool = True) -> tuple[bool, str]:
    """Whether two output files hold the same result (see the module's documentation);
    ``nifti_extensions=False`` leaves NIfTI extensions out (``Policy.nifti_extensions``)."""
    a, b = stored.read_bytes(), fresh.read_bytes()
    if a == b:
        return True, ""
    name = fresh.name
    if name.endswith(".gz"):
        try:
            a, b = gzip.decompress(a), gzip.decompress(b)
        except (OSError, EOFError):
            return False, "the compressed files differ and do not decompress"
        if a == b:
            return True, "the same after decompression"
    if not nifti_extensions and name.endswith((".nii", ".nii.gz")):
        ca, cb = without_extensions(a), without_extensions(b)
        if ca is not None and ca == cb:
            return True, "the same header and data (the extensions are not compared)"
    if name.endswith(".gz"):
        return False, f"{_bytes_detail(a, b)} (decompressed)"
    if name.endswith((".h5", ".hdf5")):
        try:
            same = _h5_content(stored) == _h5_content(fresh)
        except Exception as e:  # pragma: no cover - h5py missing or a broken file
            return False, f"the HDF5 files differ ({type(e).__name__}: {e})"
        return same, "the same datasets and attributes" if same else "the datasets differ"
    return False, _bytes_detail(a, b)


def _bytes_detail(a: bytes, b: bytes) -> str:
    if len(a) != len(b):
        return f"{len(a)} bytes stored, {len(b)} now"
    diff = np.frombuffer(a, np.uint8) != np.frombuffer(b, np.uint8)
    where = np.flatnonzero(diff)
    return f"{where.size} of {len(a)} bytes differ, the first at offset {int(where[0])}"


__all__ = ["VOLATILE", "first_line_difference", "mask", "same_file", "without_extensions"]
