# SPDX-License-Identifier: Apache-2.0
"""SHA-256 of files, remembered: in this process by path and stat, and for large files (the
fMRIPrep image, real BOLD series) across processes, in the cache's ``hashes/``.

A file's identity for the memo is its path, device, inode, size and modification time in
nanoseconds: rewriting a file changes at least its time (or its inode, when it is replaced).
"""

from __future__ import annotations

import contextlib
import hashlib
import os
import zlib
from pathlib import Path

#: Files at least this large are remembered across processes.
LARGE = 8 << 20
_CHUNK = 4 << 20
_memo: dict[tuple, str] = {}


def _stat_id(path: Path) -> tuple:
    st = path.stat()
    return (str(path), st.st_dev, st.st_ino, st.st_size, st.st_mtime_ns)


def _digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while block := f.read(_CHUNK):
            h.update(block)
    return h.hexdigest()


def _gunzip_digest(path: Path) -> str:
    """The SHA-256 of a gzip file's decompressed stream (every member); ``""`` if it is not a
    whole gzip file (then its bytes are what a program reads)."""
    h = hashlib.sha256()
    try:
        with path.open("rb") as f:
            d = zlib.decompressobj(wbits=31)
            fed = False  # whether the current member has received bytes
            while block := f.read(_CHUNK):
                while block:
                    fed = True
                    h.update(d.decompress(block))
                    if d.eof:  # a member ended: the rest of the block is the next one's
                        block, d, fed = d.unused_data, zlib.decompressobj(wbits=31), False
                    else:
                        block = b""
    except zlib.error:
        return ""
    return "" if fed else h.hexdigest()  # a member that never ended: a truncated file


def sha256_content(path: Path | str) -> str:
    """What a program reads from ``path``: for a ``.gz`` file the SHA-256 of its decompressed
    stream (``gunzip:<hex>``: the header's time and the compression do not count), else of its
    bytes."""
    path = Path(path)
    if path.name.endswith(".gz"):
        digest = sha256_file(path, kind="gunzip")
        if digest:
            return f"gunzip:{digest}"
    return sha256_file(path)


def sha256_file(path: Path | str, *, memo: bool = True, kind: str = "bytes") -> str:
    """The SHA-256 of ``path``'s bytes (hex); with ``kind="gunzip"``, of its decompressed
    stream (``""`` if it is not a whole gzip file)."""
    path = Path(path)
    digest = _gunzip_digest if kind == "gunzip" else _digest
    if not memo:
        return digest(path)
    ident = (*_stat_id(path), kind)
    if (hit := _memo.get(ident)) is not None:
        return hit
    persistent = None
    if ident[3] >= LARGE:
        from larmorx_validation.oracle_cache.store import location, write_atomic

        name = hashlib.sha256(repr(ident).encode()).hexdigest()
        persistent = location() / "hashes" / name[:2] / name
        try:
            value = persistent.read_text(encoding="ascii").strip()
            if len(value) == 64:
                _memo[ident] = value
                return value
        except OSError:
            pass
    value = digest(path)
    if (*_stat_id(path), kind) != ident:  # changed while it was read: do not remember it
        return value
    _memo[ident] = value
    if persistent is not None:
        with contextlib.suppress(OSError):
            write_atomic(persistent, value.encode("ascii"))
    return value


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def directory_digest(path: Path, *, max_files: int = 64, max_bytes: int = 1 << 20) -> str | None:
    """A digest of a small directory's tree (names and contents); ``None`` when it holds more
    than ``max_files`` files or ``max_bytes`` bytes (a real home directory)."""
    items = []
    total = 0
    for root, dirs, files in os.walk(path):
        dirs.sort()
        for name in sorted(files):
            p = Path(root) / name
            rel = p.relative_to(path).as_posix()
            try:
                size = p.stat().st_size
            except OSError:
                continue
            total += size
            if len(items) >= max_files or total > max_bytes:
                return None
            items.append(f"{rel}\0{sha256_file(p, memo=False)}")
    return sha256_bytes("\n".join(items).encode())


def clear_memo() -> None:
    """Forget this process's memo (tests)."""
    _memo.clear()


__all__ = [
    "LARGE",
    "clear_memo",
    "directory_digest",
    "sha256_bytes",
    "sha256_content",
    "sha256_file",
]
