# SPDX-License-Identifier: Apache-2.0
"""Which Python environment the suites whose originals are Python libraries (numpy, SciPy,
nitransforms, nibabel, fMRIPrep's resampler) run in: the newest releases or the shared
environment's (decision log 2026-10-27, "Newest upstream versions, updated in steps"; the
models are :mod:`afni_versions` and :mod:`fsl_versions`).

Two environments are kept side by side, neither modified by the harness:

- ``newest``: ``<workspace>/oracles/python-newest`` (numpy 2.5.4 with OpenBLAS 0.3.34, SciPy
  1.18.1; every other package as the shared one: made by
  ``validation/investigations/version-numerics/make_python_newest.sh``, which records its
  contents in ``ENVIRONMENT.txt``), the releases the ports follow since 2026-10-29;
- ``shared``: ``<workspace>/.venv`` (numpy 2.3.5, SciPy 1.15.3: ``validation/requirements-lock.txt``),
  which every other suite runs in.

These originals run in the harness's own process, so the environment is chosen by the
interpreter: ``--python`` (``parity``, ``evidence``, ``difftest``) or ``LARMORX_PYTHON`` names
it, and the command starts again under that environment's interpreter when it is not the one
running (:func:`reexec`). Without either, the suites of :data:`SUITES` (and random testing's
tools of :data:`TOOLS`) use ``newest`` when it exists; any other command keeps the running
interpreter.

The **step**: a run in ``newest`` first runs the same cases in ``shared``, recording the
originals' outputs by digest (``LARMORX_PYTHON_STEP_RECORD``), then reports per case whether
the originals' own results changed from the shared releases to the newest
(``LARMORX_PYTHON_STEP_FROM``), each change with its reason (:data:`CHANGES`); in ``shared``
the cases those changes reach are expected divergences, with the reason.
"""

from __future__ import annotations

import contextlib
import contextvars
import functools
import hashlib
import json
import os
import re
import sys
from collections.abc import Iterator
from pathlib import Path

import numpy as np

#: The kept environments: name -> directory under the workspace.
ENVS = {"newest": "oracles/python-newest", "shared": ".venv"}
NEWEST = "newest"
SHARED = "shared"
ENV = "LARMORX_PYTHON"
#: The parity suites whose originals are Python libraries (they default to ``newest``).
SUITES = ("resample-series", "resample-series-jacobian", "numpy-linalg")
#: The suites that report the originals' own step (their comparisons record digests).
STEP_SUITES = ("resample-series",)
#: Random testing's tools of the same originals.
TOOLS = ("resample-series", "nifti-io")
STEP_RECORD = "LARMORX_PYTHON_STEP_RECORD"
STEP_FROM = "LARMORX_PYTHON_STEP_FROM"
#: The releases each environment holds (what the step names; the records read them live).
RELEASES = {
    NEWEST: "numpy 2.5.4 (OpenBLAS 0.3.34), SciPy 1.18.1",
    SHARED: "numpy 2.3.5 (OpenBLAS 0.3.30), SciPy 1.15.3",
}

#: What changed between the shared releases and the newest in the code the ports follow (the
#: source diffs and the libraries: ``validation/investigations/version-numerics/``). Keys are
#: the register's entries, or a short name where the change is not a defect.
CHANGES = {
    "SCI-03": "SciPy 1.18 sums the reflect boundary's causal initialisation into a variable of "
    "its own (scipy/ndimage/src/ni_splines.c), so its last term reads the original first "
    "coefficient: SCI-03 fixed upstream, with the scaling's rounding of its own "
    "(modes nearest, reflect and grid-mirror, orders 2 to 5)",
}

#: Spline modes whose prefilter uses the reflect initialisation.
REFLECT_MODES = ("nearest", "reflect", "grid-mirror")


def workspace() -> Path | None:
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles").is_dir():
            return parent
    return None


def prefix(name: str) -> Path | None:
    ws = workspace()
    if ws is None:
        return None
    d = ws / ENVS[name]
    return d if (d / "bin" / "python").exists() else None


def running() -> str | None:
    """The kept environment this interpreter belongs to, if any."""
    here = Path(sys.prefix).resolve()
    for name in ENVS:
        p = prefix(name)
        if p is not None and p.resolve() == here:
            return name
    return None


def add_argument(parser) -> None:
    """``--python`` for a command whose originals may be Python libraries."""
    parser.add_argument(
        "--python",
        choices=tuple(ENVS),
        default=None,
        help="the Python environment numpy, SciPy, nitransforms and fMRIPrep's resampler run "
        f"in: newest ({ENVS[NEWEST]}: {RELEASES[NEWEST]}) or shared ({ENVS[SHARED]}: "
        f"{RELEASES[SHARED]}); sets {ENV} (the default: newest for "
        f"{', '.join(SUITES)}, else the running interpreter); the command starts again under "
        "the chosen environment's interpreter",
    )


def selected(names: tuple[str, ...] = ()) -> str | None:
    """The environment to run in: ``--python``/``LARMORX_PYTHON``, else ``newest`` when every
    one of ``names`` is a suite or tool of these originals and it exists, else ``None`` (the
    running interpreter: a run of many tools, random testing's night among them, stays where
    it runs; there the cases the step reaches are expected divergences)."""
    if env := os.environ.get(ENV):
        if env not in ENVS:
            raise SystemExit(f"{ENV}={env!r}: choose one of {', '.join(ENVS)}")
        return env
    if names and all(n in SUITES or n in TOOLS for n in names) and prefix(NEWEST) is not None:
        return NEWEST
    return None


def reexec(args, names: tuple[str, ...] = ()) -> None:
    """Start the command again under the selected environment's interpreter, when it is not
    the one running (``--python`` or the default for ``names``)."""
    if getattr(args, "python", None):
        os.environ[ENV] = args.python
    want = selected(names)
    if want is None or running() == want:
        return
    p = prefix(want)
    if p is None:
        raise SystemExit(f"the {want} Python environment ({ENVS[want]}) was not found")
    os.environ[ENV] = want
    py = str(p / "bin" / "python")
    sys.stdout.flush()
    sys.stderr.flush()
    os.execv(py, [py, "-m", "larmorx_validation", *sys.argv[1:]])


@functools.cache
def versions() -> dict[str, str]:
    """numpy, SciPy (and the OpenBLAS numpy bundles), nitransforms, nibabel: as running."""
    out: dict[str, str] = {}
    for name in ("numpy", "scipy", "nitransforms", "nibabel"):
        try:
            mod = __import__(name)
            out[name] = str(mod.__version__)
        except ImportError:
            out[name] = "not installed"
    try:
        import threadpoolctl

        for info in threadpoolctl.threadpool_info():
            if info.get("internal_api") == "openblas" and "numpy" in info.get("filepath", ""):
                out["numpy's OpenBLAS"] = f"{info.get('version')} ({info.get('architecture')})"
    except ImportError:
        pass
    return out


def describe() -> str:
    v = versions()
    env = running()
    where = f" ({ENVS[env]})" if env else ""
    blas_version = v.get("numpy's OpenBLAS")
    blas = f" (OpenBLAS {blas_version})" if blas_version else ""
    return f"numpy {v['numpy']}{blas}, SciPy {v['scipy']}{where}"


def older_scipy() -> bool:
    """Whether the running SciPy predates 1.18 (the step's SCI-03 change)."""
    m = re.match(r"(\d+)\.(\d+)", versions().get("scipy", ""))
    return bool(m) and (int(m.group(1)), int(m.group(2))) < (1, 18)


def reasons(mode: str | None, order: int | None) -> list[str]:
    """The known changes (:data:`CHANGES`) a spline call of ``mode`` and ``order`` meets."""
    if mode in REFLECT_MODES and order is not None and order >= 2:
        return ["SCI-03"]
    return []


# ------------------------------------------------------------------------------------------------
# The step: the originals' outputs by digest, per case and comparison

_case: contextvars.ContextVar[str | None] = contextvars.ContextVar("python_case", default=None)


@contextlib.contextmanager
def case(case_id: str) -> Iterator[None]:
    token = _case.set(case_id)
    try:
        yield
    finally:
        _case.reset(token)


def _file(directory: str, case_id: str) -> Path:
    return Path(directory) / (re.sub(r"[^A-Za-z0-9_.-]+", "_", case_id) + ".json")


def digest(a: np.ndarray) -> str:
    a = np.ascontiguousarray(a)
    h = hashlib.sha256(f"{a.dtype.str}{a.shape}".encode())
    h.update(a.tobytes())
    return h.hexdigest()


def record(name: str, expected: np.ndarray) -> None:
    """In a ``shared`` step run: remember the original's output of comparison ``name``."""
    directory, cid = os.environ.get(STEP_RECORD), _case.get()
    if not directory or cid is None:
        return
    f = _file(directory, cid)
    data = json.loads(f.read_text(encoding="utf-8")) if f.is_file() else {}
    data[name] = digest(expected)
    f.write_text(json.dumps(data, indent=0), encoding="utf-8")


def previous(name: str) -> str | None:
    """In a ``newest`` run: the shared releases' digest of comparison ``name``, if recorded."""
    directory, cid = os.environ.get(STEP_FROM), _case.get()
    if not directory or cid is None:
        return None
    f = _file(directory, cid)
    if not f.is_file():
        return None
    return json.loads(f.read_text(encoding="utf-8")).get(name)


STEP_CHECK = f"the originals' own step: {RELEASES[SHARED]} → {RELEASES[NEWEST]}"
