# SPDX-License-Identifier: Apache-2.0
"""FreeSurfer 8.2.0's unit oracle from Python: one library function of the official build's
``utils`` per call, through the oracle-output cache.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (its commands name FreeSurfer's internal functions;
running the harness is a black-box run of FreeSurfer's code, like running one of its programs).

The harness ``fsunit`` is built by ``validation/investigations/freesurfer-unit-oracle/build.py``
(FreeSurfer's ``utils`` and packages from the 8.2.0 tag, GCC 8.4.0 as the official build ran it,
ITK 4.13.2) into ``<workspace>/oracles/freesurfer-8.2.0-unit/bin/fsunit`` (``LARMORX_FS_UNIT``
names another). Each call runs ``fsunit <command> <arguments>`` through
:func:`larmorx_validation.oracle_cache.run` (one thread, FreeSurfer's family of variables, its
files compared with their provenance left out), so a call seen before is served from the cache;
the harness prints one JSON object, every floating-point value as ``{"hex": "%a", "dec":
"%.17g"}`` (:func:`value` reads it exactly).

    from larmorx_validation import freesurfer_unit as fu
    fu.vnl_random(1234, 5, "drand64")          # [0.33..., ...]
    fu.matrix_inverse([[2.0, 0, 0, 1], ...])   # FreeSurfer's MatrixInverse on float32 values
    fu.mri_read_write("in.mgz", "out.mgz")     # MRIread then MRIwrite; the header as read

The commands, the build and the proof that it is the official build:
``validation/investigations/freesurfer-unit-oracle/README.md``.
"""

from __future__ import annotations

import json
import os
import struct
import tempfile
from collections.abc import Sequence
from dataclasses import dataclass, field
from pathlib import Path

#: Where the harness is, unless ``LARMORX_FS_UNIT`` says otherwise (relative to the workspace).
DEFAULT = Path("oracles") / "freesurfer-8.2.0-unit" / "bin" / "fsunit"
ENV_PATH = "LARMORX_FS_UNIT"

#: The methods of ``vnl_random`` the harness draws with.
VNL_METHODS = ("lrand32", "drand32", "drand64", "normal", "normal64")


class UnitError(RuntimeError):
    """The harness failed (its exit status and messages in the text)."""


def harness() -> Path | None:
    """The ``fsunit`` program (``LARMORX_FS_UNIT``, else the workspace's build; ``None``: no
    workspace holds ``oracles/``, as on CI)."""
    from larmorx_validation.oracle_cache.store import workspace

    env = os.environ.get(ENV_PATH)
    if env:
        return Path(env)
    ws = workspace()
    return ws / DEFAULT if ws is not None else None


#: FreeSurfer's installation the harness reads (its data; ``MRIread`` checks the licence there
#: unless ``FS_LICENSE`` names the file), relative to the workspace.
FS_HOME = Path("oracles") / "freesurfer-8.2.0" / "root" / "usr" / "local" / "freesurfer" / "8.2.0"
#: The licence file read in place when neither ``LARMORX_FS_LICENSE`` nor ``FS_LICENSE`` names one
#: (CLAUDE.md: never copied, never in results).
LICENCE = Path("/usr/local/freesurfer/license.txt")


def freesurfer_env() -> dict[str, str]:
    """``FREESURFER_HOME`` and ``FS_LICENSE`` for FreeSurfer's code: ``MRIread`` and the other
    readers call FreeSurfer's licence check (``chklc``), which stops a program without them."""
    from larmorx_validation.oracle_cache.store import workspace

    out = {}
    ws = workspace()
    home = os.environ.get("LARMORX_FS_HOME") or (str(ws / FS_HOME) if ws is not None else "")
    if home:
        out["FREESURFER_HOME"] = home
    lic = os.environ.get("LARMORX_FS_LICENSE") or os.environ.get("FS_LICENSE")
    if not lic and LICENCE.is_file():
        lic = str(LICENCE)
    if lic:
        out["FS_LICENSE"] = lic
    return out


def available() -> bool:
    exe = harness()
    return exe is not None and exe.is_file()


def value(v: dict | int | str) -> float | int:
    """A value the harness printed, exactly: ``{"hex": ...}`` as a float, an integer as itself."""
    if isinstance(v, dict):
        return float.fromhex(v["hex"])
    if isinstance(v, str):
        return float.fromhex(v)
    return v


def values(vs: Sequence) -> list:
    return [values(v) if isinstance(v, list) else value(v) for v in vs]


def f32(x: float) -> float:
    """``x`` rounded to float32 (FreeSurfer's MATRIX holds float), as a Python float."""
    return struct.unpack("<f", struct.pack("<f", x))[0]


@dataclass
class Result:
    """One call: the harness's JSON, its exit status and messages, whether it was served."""

    argv: list[str]
    returncode: int | None
    data: dict
    stderr: str
    served: bool
    outputs: list[Path] = field(default_factory=list)


def call(
    command: str,
    *args: str | os.PathLike,
    outputs: Sequence[Path] = (),
    env: dict[str, str] | None = None,
    threads: int = 1,
    never: str = "",
    timeout: float | None = 600,
    check: bool = True,
) -> Result:
    """``fsunit <command> <args>`` through the oracle-output cache. ``outputs``: directories (or
    files) it writes beyond those its arguments name; ``env``: variables added to its controlled
    environment (``PATH`` of the system only, ``OMP_NUM_THREADS`` = ``threads``, the C locale,
    :func:`freesurfer_env`);
    ``never``: why this call must never be cached (the policy's rule)."""
    from larmorx_validation import oracle_cache

    exe = harness()
    if exe is None or not exe.is_file():
        raise UnitError(f"the unit oracle is not built: {exe} (build.py all)")
    run_env = {
        "PATH": "/usr/bin:/bin",
        "OMP_NUM_THREADS": str(threads),
        "LC_ALL": "C",
        "LANG": "C",
        "HOME": os.environ.get("HOME", "/tmp"),
        **freesurfer_env(),
    }
    run_env.update(env or {})
    policy = oracle_cache.Policy(
        threads=threads, never=never, family="freesurfer", formats="freesurfer"
    )
    argv = [str(exe), command, *map(str, args)]
    done = oracle_cache.run(
        argv,
        env=run_env,
        outputs=list(outputs),
        policy=policy,
        timeout=timeout,
        runner="freesurfer-unit",
    )
    text = done.stdout.strip()
    try:
        data = json.loads(text.splitlines()[-1]) if text else {}
    except json.JSONDecodeError:
        data = {}
    res = Result(argv, done.returncode, data, done.stderr, done.cached)
    if check and (done.returncode != 0 or not data):
        raise UnitError(
            f"fsunit {command} failed ({done.returncode}): {done.stderr.strip()[-2000:]}"
        )
    return res


def version() -> dict:
    return call("version").data


def vnl_random(seed: int, count: int, method: str = "drand64") -> list:
    """``vnl_random(seed)`` (ITK 4.13.2's VXL, as the official binaries link it), then ``count``
    draws of ``method`` (:data:`VNL_METHODS`): integers for ``lrand32``, floats otherwise."""
    if method not in VNL_METHODS:
        raise ValueError(f"method {method!r} not in {VNL_METHODS}")
    return values(call("vnl-random", str(seed), str(count), method).data["values"])


def fs_random(seed: int, count: int, low: float = 0.0, high: float = 1.0) -> list[float]:
    """FreeSurfer's ``setRandomSeed(seed)`` then ``count`` calls of ``randomNumber(low, high)``."""
    res = call("fs-random", str(seed), str(count), low.hex(), high.hex())
    return values(res.data["values"])


def open_ran1(seed: int, count: int) -> list[float]:
    """FreeSurfer's ``OpenRan1(&seed)`` (``utils/numerics.cpp``) ``count`` times from a fresh
    process: ``vnl_random(seed)``'s ``drand64(0, 1)`` rounded to float, the values
    ``setRandomSeed`` and ``randomNumber`` draw from."""
    return values(call("open-ran1", str(seed), str(count)).data["values"])


def _write_matrix(path: Path, m: Sequence[Sequence[float]]) -> None:
    rows, cols = len(m), len(m[0])
    lines = [f"{rows} {cols}"] + [" ".join(f32(float(x)).hex() for x in row) for row in m]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def _matrix_call(command: str, mats: Sequence[Sequence[Sequence[float]]], out: bool):
    with tempfile.TemporaryDirectory(prefix="lx-fsunit_") as d:
        names = []
        for i, m in enumerate(mats):
            p = Path(d) / f"m{i}.txt"
            _write_matrix(p, m)
            names.append(p)
        extra = [Path(d) / "out.txt"] if out else []
        res = call(command, *names, *extra, check=False)
    if res.returncode not in (0, 1) or not res.data:
        raise UnitError(f"fsunit {command} failed ({res.returncode}): {res.stderr.strip()}")
    r = res.data.get("result")
    if r is None:
        return None
    return values(r) if isinstance(r, list) else value(r)


def matrix_inverse(m: Sequence[Sequence[float]]) -> list[list[float]] | None:
    """FreeSurfer's ``MatrixInverse`` (``utils/matrix.cpp``) on ``m`` rounded to float32;
    ``None`` where it returns NULL (singular)."""
    return _matrix_call("matrix-inverse", [m], out=True)


def matrix_multiply(
    a: Sequence[Sequence[float]], b: Sequence[Sequence[float]]
) -> list[list[float]]:
    return _matrix_call("matrix-multiply", [a, b], out=True)


def matrix_determinant(m: Sequence[Sequence[float]]) -> float:
    return _matrix_call("matrix-determinant", [m], out=False)


def mri_header(path: Path) -> dict:
    """``MRIread``: the header as FreeSurfer reads it (type, sizes, direction cosines, centre,
    TR/TE/TI/flip angle, vox2ras), every float exact."""
    return call("mri-header", Path(path)).data["header"]


def mri_read_write(src: Path, dst: Path, cmdlines_from: Path | None = None) -> dict:
    """``MRIread(src)`` then ``MRIwrite(dst)`` (the writer chosen by ``dst``'s suffix, as
    FreeSurfer chooses it); the header as read. ``cmdlines_from``: a volume whose command-line
    provenance (as ``MRIread`` reads it) replaces ``src``'s before the write."""
    extra = [Path(cmdlines_from)] if cmdlines_from is not None else []
    res = call("mri-read-write", Path(src), Path(dst), *extra)
    return res.data["header"]
