# SPDX-License-Identifier: Apache-2.0
"""FreeSurfer's installation as an oracle: its identity, and one launcher per program.

The oracle-output cache keys a run by its program's identity (``oracle_cache.key.
program_identity``): an ELF program's SHA-256 and its shared libraries', a script's SHA-256 and
every file of its directory. A FreeSurfer program also reads its installation's data (atlases,
colour tables, ``fsaverage``, the networks' weights) and, for its scripts, the other programs
of ``bin/``; none of that would be in the key. So every FreeSurfer program runs through a
launcher, ``<base>/<tree>/<program>/<program>`` (a two-line shell script that runs the
installation's program), beside a ``PACKAGE.json`` that names the whole installation: its build
stamp, the downloaded package's SHA-256 (when ``download/SHA256SUMS`` is found beside the
unpacked tree, as the reference runs keep it) and the SHA-256 of a manifest of every file of the
tree (each file's path and SHA-256; a link by its text). The launcher's directory is the
"build directory" the key hashes, so a run's key names the installation it ran in, all of it.
The tree is hashed once (about a minute for 8.2.0's 19 GB with four threads) and remembered by
each file's size and modification time.

This module reads no FreeSurfer source: it hashes files and runs programs.
"""

from __future__ import annotations

import functools
import hashlib
import json
import os
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from larmorx_validation.oracle_cache.store import workspace

#: Where the replay tool keeps what it derives (the cache's rule: derived inputs outside it).
SUITE = "freesurfer-replay"


def base() -> Path:
    """``<workspace>/oracles/inputs/freesurfer-replay`` (``LARMORX_FS_REPLAY_BASE`` overrides)."""
    if env := os.environ.get("LARMORX_FS_REPLAY_BASE"):
        return Path(env)
    ws = workspace()
    if ws is None:
        raise RuntimeError("no workspace with oracles/ above this file; set LARMORX_FS_REPLAY_BASE")
    return ws / "oracles" / "inputs" / SUITE


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while block := f.read(4 << 20):
            h.update(block)
    return h.hexdigest()


def manifest(fs_home: Path, *, workers: int = 4, memo: Path | None = None) -> dict[str, str]:
    """Every file of the installation ``fs_home``: relative path -> SHA-256 (a symbolic link:
    ``link:<its text>``). Hashes are remembered in ``memo`` by size and modification time."""
    fs_home = Path(fs_home).resolve()
    memo = (
        memo or base() / f"manifest-{hashlib.sha256(str(fs_home).encode()).hexdigest()[:16]}.json"
    )
    try:
        known = json.loads(memo.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        known = {}
    out: dict[str, str] = {}
    todo: list[tuple[str, Path, list]] = []
    for d, dirs, files in os.walk(fs_home):
        dirs.sort()
        for name in [*sorted(files), *(x for x in dirs if (Path(d) / x).is_symlink())]:
            p = Path(d) / name
            rel = p.relative_to(fs_home).as_posix()
            if p.is_symlink():
                out[rel] = "link:" + os.readlink(p)
                continue
            st = p.stat()
            stamp = [st.st_size, st.st_mtime_ns]
            k = known.get(rel)
            if k and k[:2] == stamp:
                out[rel] = k[2]
            else:
                todo.append((rel, p, stamp))
    with ThreadPoolExecutor(max(1, workers)) as ex:
        for (rel, _p, stamp), digest in zip(
            todo, ex.map(lambda t: _sha256(t[1]), todo), strict=True
        ):
            out[rel] = digest
            known[rel] = [*stamp, digest]
    if todo:
        memo.parent.mkdir(parents=True, exist_ok=True)
        tmp = memo.with_suffix(".part")
        tmp.write_text(json.dumps(known, sort_keys=True), encoding="utf-8")
        os.replace(tmp, memo)
    return dict(sorted(out.items()))


@functools.cache
def identity(fs_home: str, workers: int = 4) -> dict:
    """The installation's identity (``PACKAGE.json``'s content)."""
    home = Path(fs_home).resolve()
    files = manifest(home, workers=workers)
    tree = hashlib.sha256(
        "".join(f"{rel}\t{digest}\n" for rel, digest in files.items()).encode()
    ).hexdigest()
    stamp = home / "build-stamp.txt"
    package = ""
    # the reference runs unpack <workspace>/oracles/freesurfer-<v>/download/<deb> into
    # .../root/usr/local/freesurfer/<v>: the SHA256SUMS sits five levels above the tree
    sums = home.parents[4] / "download" / "SHA256SUMS" if len(home.parents) > 4 else None
    if sums is not None and sums.is_file():
        package = sums.read_text(encoding="utf-8").split()[0]
    return {
        "what": "a FreeSurfer installation (larmorx_validation.freesurfer.package)",
        "build_stamp": stamp.read_text(encoding="utf-8").strip() if stamp.is_file() else "",
        "package_sha256": package,
        "tree_sha256": tree,
        "files": len(files),
        "fs_time": files.get("bin/fs_time", ""),
        "fs_time_orig": files.get("bin/fs_time.orig", ""),
    }


def launcher(program: str, fs_home: str | Path, *, root: Path | None = None) -> Path:
    """The launcher of FreeSurfer's ``program`` (made on first use)."""
    home = Path(fs_home).resolve()
    target = home / "bin" / program
    if not target.is_file():
        raise FileNotFoundError(f"FreeSurfer has no program {program} ({target})")
    ident = identity(str(home))
    d = (root or base() / "launchers") / ident["tree_sha256"][:16] / program
    script = d / program
    text = (
        "#!/bin/sh\n"
        "# larmorx_validation.freesurfer: FreeSurfer's program, the installation in PACKAGE.json\n"
        f"exec '{target}' \"$@\"\n"
    )
    package = json.dumps(ident, indent=1, sort_keys=True) + "\n"

    def holds(path: Path) -> str | None:
        try:
            return path.read_text(encoding="utf-8")
        except OSError:
            return None

    if holds(script) != text or holds(d / "PACKAGE.json") != package:
        d.mkdir(parents=True, exist_ok=True)
        (d / "PACKAGE.json").write_text(package, encoding="utf-8")
        script.write_text(text, encoding="utf-8")
        script.chmod(0o755)
    return script


__all__ = ["SUITE", "base", "identity", "launcher", "manifest"]
