# SPDX-License-Identifier: Apache-2.0
"""The cache's storage: content-addressed blobs, one JSON entry per key, written atomically.

Layout of ``<cache>/`` (``LARMORX_ORACLE_CACHE``, else ``<workspace>/oracles/cache``)::

    FORMAT                       the layout's version
    blobs/<aa>/<sha256>          every stored file (inputs, outputs, standard output and error),
                                 named by its SHA-256, written once, never changed
    entries/<aa>/<key>.json      one original run: its key's parts in readable form, the result
                                 (exit status, the streams' and outputs' blobs), the time it took,
                                 the date, the larmorx commit that wrote it, its history
    locks/<aa>/<key>.lock        held while an entry is compared and written
    disputes/<key>/<stamp>.json  a run that gave a different result from its stored entry (a
                                 finding: both results are kept)
    findings.jsonl               one line per difference found (append-only)
    hashes/<aa>/<id>             SHA-256 of large files, by path and stat (``hashing``)
    markers/                     the parity smoke tier's markers (``markers``)

Every file is written to a temporary name in the same directory and renamed into place
(``os.replace``), so a reader sees a whole file or none; an entry is written after its blobs.
"""

from __future__ import annotations

import contextlib
import datetime as dt
import hashlib
import json
import os
import shutil
import tempfile
import uuid
from collections.abc import Iterator
from functools import cache
from pathlib import Path
from typing import Any

FORMAT = "1"
ENV_LOCATION = "LARMORX_ORACLE_CACHE"


@cache
def workspace() -> Path | None:
    """The workspace that holds ``oracles/`` (a parent of this file)."""
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles").is_dir():
            return parent
    return None


def location() -> Path:
    """The cache's directory: ``LARMORX_ORACLE_CACHE``, else ``<workspace>/oracles/cache``,
    else ``~/.cache/larmorx-oracle-cache``."""
    if env := os.environ.get(ENV_LOCATION):
        return Path(env).expanduser()
    ws = workspace()
    if ws is not None:
        return ws / "oracles" / "cache"
    return Path.home() / ".cache" / "larmorx-oracle-cache"


def now() -> str:
    return dt.datetime.now(dt.UTC).isoformat(timespec="seconds")


def _fanout(root: Path, name: str, suffix: str = "") -> Path:
    return root / name[:2] / f"{name}{suffix}"


def write_atomic(path: Path, data: bytes) -> None:
    """``data`` into ``path``: a temporary file in the same directory, renamed into place."""
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(data)
        os.replace(tmp, path)
    except BaseException:
        with contextlib.suppress(OSError):
            os.unlink(tmp)
        raise


def dumps(payload: Any) -> bytes:
    return (json.dumps(payload, indent=1, sort_keys=True, ensure_ascii=False) + "\n").encode()


class Store:
    """The cache at ``root``."""

    def __init__(self, root: Path | None = None):
        self.root = Path(root) if root is not None else location()

    # -- layout --------------------------------------------------------------------------------

    def ensure(self) -> None:
        self.root.mkdir(parents=True, exist_ok=True)
        marker = self.root / "FORMAT"
        if not marker.exists():
            write_atomic(marker, f"{FORMAT}\n".encode())
            write_atomic(
                self.root / "README.txt",
                b"The oracle-output cache of larmorx's parity suites (larmorx docs/validation/"
                b"oracle-cache.md): every run of an original program, keyed by the program's "
                b"build, its arguments, settings and inputs. Regenerable; not in git. Never "
                b"edit entries by hand: python -m larmorx_validation oracle-cache --help.\n",
            )
        elif marker.read_text(encoding="utf-8").strip() != FORMAT:
            raise RuntimeError(
                f"{self.root} holds an oracle cache of another format "
                f"({marker.read_text().strip()}, this is {FORMAT}); point LARMORX_ORACLE_CACHE "
                "elsewhere"
            )

    def entry_path(self, key: str) -> Path:
        return _fanout(self.root / "entries", key, ".json")

    def blob_path(self, sha: str) -> Path:
        return _fanout(self.root / "blobs", sha)

    # -- blobs ---------------------------------------------------------------------------------

    def put_bytes(self, data: bytes) -> str:
        sha = hashlib.sha256(data).hexdigest()
        target = self.blob_path(sha)
        if not target.exists():
            write_atomic(target, data)
        return sha

    def put_file(self, path: Path, sha: str | None = None) -> str:
        """Copy ``path`` into the blobs (once per content); its SHA-256."""
        from larmorx_validation.oracle_cache.hashing import sha256_file

        sha = sha or sha256_file(path)
        target = self.blob_path(sha)
        if target.exists():
            return sha
        target.parent.mkdir(parents=True, exist_ok=True)
        tmp = target.parent / f".{sha}.{uuid.uuid4().hex}.tmp"
        try:
            shutil.copyfile(path, tmp)
            from larmorx_validation.oracle_cache.hashing import sha256_file as check

            if check(tmp, memo=False) != sha:  # the file changed while it was copied
                raise RuntimeError(f"{path} changed while it was stored")
            os.replace(tmp, target)
        finally:
            with contextlib.suppress(OSError):
                tmp.unlink()
        return sha

    def get_bytes(self, sha: str) -> bytes:
        return self.blob_path(sha).read_bytes()

    def copy_blob(self, sha: str, target: Path) -> None:
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(self.blob_path(sha), target)

    def has_blob(self, sha: str) -> bool:
        return self.blob_path(sha).is_file()

    # -- entries -------------------------------------------------------------------------------

    def read(self, key: str) -> dict | None:
        path = self.entry_path(key)
        try:
            return json.loads(path.read_text(encoding="utf-8"))
        except FileNotFoundError:
            return None

    def write(self, entry: dict) -> None:
        write_atomic(self.entry_path(entry["key"]), dumps(entry))

    @contextlib.contextmanager
    def lock(self, key: str) -> Iterator[None]:
        """An exclusive lock on ``key`` across processes (``fcntl.flock``; none where the
        platform has no ``fcntl``: the cache runs on Linux, where the originals are)."""
        path = _fanout(self.root / "locks", key, ".lock")
        path.parent.mkdir(parents=True, exist_ok=True)
        try:
            import fcntl
        except ImportError:  # pragma: no cover - not Linux
            yield
            return
        with path.open("a") as f:
            fcntl.flock(f, fcntl.LOCK_EX)
            try:
                yield
            finally:
                fcntl.flock(f, fcntl.LOCK_UN)

    def entries(self) -> Iterator[dict]:
        base = self.root / "entries"
        if not base.is_dir():
            return
        for path in sorted(base.glob("*/*.json")):
            try:
                yield json.loads(path.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError):
                continue

    # -- findings ------------------------------------------------------------------------------

    def dispute(self, key: str, record: dict) -> Path:
        """Keep a run that differs from its stored entry (both results stay); the finding is
        appended to ``findings.jsonl``. The path of the dispute's record."""
        stamp = dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")
        path = self.root / "disputes" / key / f"{stamp}-{os.getpid()}-{uuid.uuid4().hex[:6]}.json"
        write_atomic(path, dumps(record))
        line = json.dumps(
            {"date": now(), "key": key, "dispute": str(path.relative_to(self.root)), **record},
            sort_keys=True,
            ensure_ascii=False,
        )
        self.root.mkdir(parents=True, exist_ok=True)
        with (self.root / "findings.jsonl").open("a", encoding="utf-8") as f:
            f.write(line + "\n")
        return path


__all__ = ["ENV_LOCATION", "FORMAT", "Store", "dumps", "location", "now", "workspace"]
