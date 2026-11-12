# SPDX-License-Identifier: Apache-2.0
"""Parity of the t2smap replica (``larmorx-gpl tedana t2smap``, GNU GPL version 3) with tedana
26.0.3's ``t2smap``, file by file and bit by bit, in both kept Python environments.

The replica runs as a separate program (``LARMORX_GPL_BIN``, else ``crates-gpl/target/release``
or ``debug``, else ``larmorx-gpl`` on the ``PATH``); this module imports nothing from it.
tedana runs as a program too, through the oracle-output cache: a launcher script per
environment (``oracles/inputs/tedana-t2smap-replica/launcher-<env>/t2smap``, with a recursion
guard) beside a manifest of that environment's interpreter and installed distributions (each
one's ``RECORD`` by its SHA-256), so the cache's key holds tedana's, numpy's and SciPy's exact
installation; the key's environment is the ``python`` family's (numpy's CPU dispatch, OpenBLAS,
the interpreter's variables, ``COLUMNS``).

Every run uses numpy's OpenBLAS on the Haswell kernels, numpy's AVX-512 kernels off
(``NPY_DISABLE_CPU_FEATURES``, decision D2: tedana's float64 ``exp``/``log`` are then glibc
2.35's), one BLAS thread and ``COLUMNS=80``, with the inputs at absolute paths and
``--out-dir out`` in a fresh run directory; tedana's progress bars are off (``TQDM_DISABLE=1``:
their frames follow the clock, and the replica's bars, its own, are not compared).

Kinds of cases:

- **fmriprep**: fMRIPrep 25.2.6's own ``t2smap`` calls (``bold_t2smap_wf/t2smap_node``) of the
  multi-echo oracle runs (ds003592, ds000210's two tasks, ds000258; ``--fittype curvefit``, and
  ds003592 also with ``loglin``): besides the comparisons below, tedana 26.0.3's and the
  replica's default T2* map and combined series against fMRIPrep's own files (tedana 25.0.1),
  reported with the known reasons;
- **syn**: seeded synthetic echoes (``validation/investigations/t2smap/replica``'s
  ``check_stages.py`` and ``check_outputs.py``): every option and path (fit types and modes,
  mask types, data types and scaling, z-concatenated data, no mask, dummy scans and excluded
  volumes, PAID, the verbose maps, prefixes and conventions, nine echoes, one volume, existing
  outputs), with infinite and NaN samples;
- **fail**: the failure cases and the TED defects' edge cases (``check_failures.py``);
- **spread**: what tedana itself varies on fMRIPrep's ds003592 call and two synthetic cases
  (a measurement: numpy's AVX-512 kernels on, four threads, the shared releases, tedana 25.0.1
  in fMRIPrep's image), against which the replica's own thread count changes nothing.

Every case of the first three kinds compares, with the replica in ``reproduce`` mode (D13), in
each environment (``newest``: numpy 2.5.4, SciPy 1.18.1; ``shared``: numpy 2.3.5, SciPy 1.15.3,
the replica with ``LARMORX_TEDANA_LIBRARIES=shared``): the exit status and the last line of a
failure; tedana's log lines (the run directory masked) and Python's warnings (their file named
from the package); the files written; every image's decompressed bytes (every header field and
value); the confounds table's text; the registry; ``dataset_description.json`` but for the
fields that describe the host and Python (D6); ``t2smap_call.sh``. Then tedana's own step from
the shared releases to the newest (reported: SciPy 1.18's ``least_squares`` moves curve fits'
last bits; numpy 2.5.4's LAPACK 3.12.1 now and then a log-linear fit's), and the default ``fix``
pass against the reproduced run (a defect named exactly when the outputs differ).
"""

from __future__ import annotations

import functools
import importlib.util
import os
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    SKIPPED,
    Case,
    CheckList,
    Outcome,
    Suite,
    in_tier,
)

SUITE = "tedana-t2smap-replica"
TEDANA = "26.0.3"
FMRIPREP_TEDANA = "25.0.1"
#: numpy's AVX-512 kernels off, for numpy 2.3.5 and 2.5.4 (decision D2, relay 2's names).
NPY_OFF = "X86_V4 AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL AVX512_SPR"
ENVS = {"newest": "oracles/python-newest", "shared": ".venv"}
RELEASES = {
    "newest": "numpy 2.5.4 (OpenBLAS 0.3.34), SciPy 1.18.1",
    "shared": "numpy 2.3.5 (OpenBLAS 0.3.30), SciPy 1.15.3",
}
FMRIPREP_IMAGE = "oracles/images/fmriprep-25.2.6.sif"
INPUTS = "oracles/inputs/tedana-t2smap-replica"
INVESTIGATION = Path(__file__).resolve().parents[3] / "investigations" / "t2smap" / "replica"
#: Why tedana's own outputs change from the shared releases to the newest.
STEP_REASON = (
    "SciPy 1.15.3 -> 1.18.1 changed least_squares' trust-region reflective method and "
    "approx_derivative (the order of its steps; the SVD factors' memory order): the curve fit's "
    "last bits (relay 4: 39,211 of ds003592's 52,826 fits, median relative 9e-10)"
)
#: Why a log-linear fit's outputs change from the shared releases to the newest (random testing,
#: 2026-11-08: none of the record's cases, a voxel now and then on random series).
LOGLIN_STEP_REASON = (
    "numpy 2.3.5 -> 2.5.4 changed LAPACK 3.12.0 -> 3.12.1 (dgelsd's QR applies its reflectors "
    "with dlarf1f instead of dlarf): the last bits of some voxels' log-linear slope, which T2* = "
    "1/slope magnifies where the slope is near zero (a flat or noisy voxel); the replica follows "
    "each release"
)
#: The known reasons tedana 26.0.3's outputs differ from fMRIPrep's (tedana 25.0.1's).
FMRIPREP_REASON = (
    "tedana 26.0.x caps only the limited T2* map (TED-05; 25.0.1 capped the full map, as "
    "larmorx's default does), loads the data as float32 (25.0.1: float64), applies the T2* "
    "floors to the full map and no longer clamps negative values; fMRIPrep's image has numpy "
    "2.2.6 (sums over more than 8,192 elements in chunks), SciPy 1.15.2 and conda-forge's "
    "OpenBLAS, and its run was made on another CPU"
)


def workspace() -> Path | None:
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles").is_dir():
            return parent
    return None


def replica_binary() -> Path | None:
    """``LARMORX_GPL_BIN``, else the repository's ``crates-gpl/target/{release,debug}``, else
    ``larmorx-gpl`` on the ``PATH``."""
    if env := os.environ.get("LARMORX_GPL_BIN"):
        return Path(env) if Path(env).is_file() else None
    root = Path(__file__).resolve().parents[4]
    exe = "larmorx-gpl.exe" if os.name == "nt" else "larmorx-gpl"
    for profile in ("release", "debug"):
        p = root / "crates-gpl" / "target" / profile / exe
        if p.is_file():
            return p
    found = shutil.which("larmorx-gpl")
    return Path(found) if found else None


def replica_version() -> str:
    exe = replica_binary()
    if exe is None:
        return "not found"
    p = subprocess.run([str(exe), "--version"], capture_output=True, text=True, check=False)
    return p.stdout.strip() or "unknown version"


def _investigation(name: str):
    """One of the investigation's scripts, as a module (their case recipes and comparisons)."""
    key = f"_t2smap_{name}"
    if key in sys.modules:
        return sys.modules[key]
    spec = importlib.util.spec_from_file_location(key, INVESTIGATION / f"{name}.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[key] = module
    spec.loader.exec_module(module)
    return module


# ------------------------------------------------------------------------------------------------
# The programs


@dataclass(frozen=True)
class Run:
    code: int
    stdout: str
    stderr: str
    cwd: Path

    @property
    def out(self) -> Path:
        return self.cwd / "out"


def python_of(env: str) -> Path | None:
    ws = workspace()
    if ws is None:
        return None
    p = ws / ENVS[env] / "bin" / "python"
    return p if p.exists() else None


_MANIFEST = r"""
import hashlib, importlib.metadata as md, sys
print("python", sys.version.replace("\n", " "))
for d in sorted(md.distributions(), key=lambda d: (d.metadata["Name"] or "").lower()):
    try:
        digest = hashlib.sha256((d._path / "RECORD").read_bytes()).hexdigest()
    except (AttributeError, OSError):
        digest = "-"
    print(d.metadata["Name"], d.version, digest)
"""

_LAUNCHER = """#!{python} -P
# The oracle's launcher (larmorx_validation.parity.tedana_t2smap_replica): tedana's t2smap as
# its program runs it; the manifest beside it names the environment (the oracle cache's key).
import os
import sys

if os.environ.get("T2SMAP_ORACLE_GUARD"):
    sys.exit("recursion guard: the t2smap launcher started itself")
os.environ["T2SMAP_ORACLE_GUARD"] = "1"
from tedana.workflows import t2smap

t2smap._main(sys.argv[1:])
"""


@functools.cache
def launcher(env: str) -> Path | None:
    """The launcher of ``env``'s tedana (written when missing or changed)."""
    ws, python = workspace(), python_of(env)
    if ws is None or python is None:
        return None
    d = ws / INPUTS / f"launcher-{env}"
    d.mkdir(parents=True, exist_ok=True)
    manifest = subprocess.run(
        [str(python), "-I", "-c", _MANIFEST], capture_output=True, text=True, check=True
    ).stdout
    if "tedana" not in manifest:
        return None
    for name, text, mode in (
        ("ENVIRONMENT.txt", manifest, 0o644),
        ("t2smap", _LAUNCHER.format(python=python), 0o755),
    ):
        p = d / name
        if not p.is_file() or p.read_text() != text:
            tmp = d / f".{name}.{os.getpid()}"
            tmp.write_text(text)
            tmp.chmod(mode)
            os.replace(tmp, p)
    return d / "t2smap"


def _base_env() -> dict[str, str]:
    env = {
        k: v
        for k, v in os.environ.items()
        if not k.startswith(("PYTHON", "LARMORX_", "NPY_", "OPENBLAS_", "OMP_", "MKL_", "TQDM_"))
        and k not in ("VIRTUAL_ENV", "T2SMAP_ORACLE_GUARD", "COLUMNS")
    }
    env.update(
        OPENBLAS_CORETYPE="Haswell",
        NPY_DISABLE_CPU_FEATURES=NPY_OFF,
        OPENBLAS_NUM_THREADS="1",
        COLUMNS="80",
        TQDM_DISABLE="1",
    )
    return env


#: tqdm's bars print the run's own timings (masked when runs are compared with their entries).
VOLATILE = (r"(\r[^\r\n]*\| ?\d+/\d+ \[[^\]]*\])+",)


def run_tedana(
    env: str,
    args: list[str],
    cwd: Path,
    *,
    avx512: bool = False,
    threads: int = 1,
    never: str = "",
) -> Run:
    """tedana's t2smap of ``env`` with ``args`` and ``--out-dir out`` in ``cwd``, through the
    oracle-output cache."""
    from larmorx_validation import oracle_cache

    prog = launcher(env)
    if prog is None:
        raise Outcome(SKIPPED, f"tedana is not installed in the {env} environment")
    e = _base_env()
    if avx512:
        e.pop("NPY_DISABLE_CPU_FEATURES")
    if threads != 1:
        e["OPENBLAS_NUM_THREADS"] = str(threads)
    p = oracle_cache.run(
        [str(prog), *args, "--out-dir", "out"],
        env=e,
        cwd=cwd,
        outputs=[cwd],
        policy=oracle_cache.Policy(
            threads=threads, family="python", volatile=VOLATILE, never=never
        ),
        runner=SUITE,
        timeout=3600,
    )
    code = -9 if p.returncode is None else p.returncode
    return Run(code, p.stdout, p.stderr, cwd)


def run_fmriprep_tedana(
    args: list[str], cwd: Path, binds: list[Path], *, avx512: bool = False
) -> Run:
    """fMRIPrep 25.2.6's tedana 25.0.1 (``t2smap`` in its image), with the Haswell kernels and
    numpy's AVX-512 kernels off (``avx512``: on, the CPU's default, as fMRIPrep's own oracle runs
    had them), through the oracle-output cache."""
    from larmorx_validation import oracle_cache

    ws = workspace()
    image = None if ws is None else ws / FMRIPREP_IMAGE
    if image is None or not image.is_file() or shutil.which("apptainer") is None:
        raise Outcome(SKIPPED, "fMRIPrep's image (or Apptainer) is not available")
    inner = {
        "OPENBLAS_CORETYPE": "Haswell",
        "NPY_DISABLE_CPU_FEATURES": NPY_OFF,
        "OPENBLAS_NUM_THREADS": "1",
        "COLUMNS": "80",
        "TQDM_DISABLE": "1",
    }
    if avx512:
        inner.pop("NPY_DISABLE_CPU_FEATURES")
    cmd = ["apptainer", "exec", "--cleanenv", "--no-home", "--pwd", str(cwd)]
    for b in sorted({str(cwd), *(str(b) for b in binds)}):
        cmd += ["--bind", f"{b}:{b}"]
    for k, v in inner.items():
        cmd += ["--env", f"{k}={v}"]
    full = [*args, "--out-dir", "out"]
    p = oracle_cache.run_apptainer(
        image,
        "t2smap",
        full,
        inner,
        [*cmd, str(image), "t2smap", *full],
        cwd=cwd,
        outputs=[cwd],
        policy=oracle_cache.Policy(threads=1, family="python", volatile=VOLATILE),
        runner="tedana-fmriprep",
        timeout=3600,
    )
    code = -9 if p.returncode is None else p.returncode
    return Run(code, p.stdout, p.stderr, cwd)


def run_replica(
    env: str, args: list[str], cwd: Path, mode: str = defects.REPRODUCE, threads: int = 1
) -> Run:
    exe = replica_binary()
    if exe is None:
        raise Outcome(
            SKIPPED,
            "larmorx-gpl is not built (cargo build --release in crates-gpl, or LARMORX_GPL_BIN)",
        )
    e = _base_env()
    e["LARMORX_UPSTREAM_DEFECTS"] = mode
    e["LARMORX_TEDANA_LIBRARIES"] = env
    extra = [] if threads == 1 else ["--n-threads", str(threads)]
    if threads == 1:
        # tedana's default, one thread: without t2smap's own --n-threads, larmorx-gpl takes the
        # run's thread count (OMP_NUM_THREADS, else every logical CPU; the user's decision of
        # 2026-11-11), so the one thread is said there
        e["OMP_NUM_THREADS"] = "1"
    p = subprocess.run(
        [str(exe), "tedana", "t2smap", *args, *extra, "--out-dir", "out"],
        cwd=cwd,
        env=e,
        capture_output=True,
        text=True,
        timeout=3600,
        check=False,
    )
    return Run(p.returncode, p.stdout, p.stderr, cwd)


def _twice(runner, args: list[str], cwd: Path, **kw) -> Run:
    """A case marked ``@twice`` runs twice in one directory (the existing outputs decide)."""
    twice = "@twice" in args
    args = [a for a in args if a != "@twice"]
    r = runner(args, cwd, **kw)
    if twice:
        r = runner(args, cwd, **kw)
    return r


# ------------------------------------------------------------------------------------------------
# Comparisons

_WARNING = re.compile(r"^(\S+?\.py):(\d+): (\w+): (.*)$")


def messages(r: Run) -> list[str]:
    """tedana's log lines (the run directory masked) and Python's warnings, their file named
    from its package (``tedana/decay.py``, ``numpy/...``) as the replica prints it."""
    out = []
    # a progress bar's frames end in carriage returns: each its own line, then left out
    lines = r.stderr.replace(str(r.cwd), "<run>").replace("\r", "\n").splitlines()
    for line in lines:
        if line.startswith(("INFO ", "WARNING ", "DEBUG ")):
            out.append(line)
            continue
        m = _WARNING.match(line)
        if m:
            path = m.group(1).replace("\\", "/")
            if "site-packages/" in path:
                path = path.split("site-packages/", 1)[1]
            out.append(f"{path}:{m.group(2)}: {m.group(3)}: {m.group(4)}")
    return out


_CHAINS = (
    "During handling of the above exception, another exception occurred:",
    "The above exception was the direct cause of the following exception:",
)


def exceptions(r: Run) -> list[str]:
    """The traceback's exceptions in order: each ``Traceback`` line and its exception's first
    line (the stack left out, the run directory masked), and the lines that chain them."""
    out: list[str] = []
    in_trace = False
    for line in r.stderr.replace(str(r.cwd), "<run>").splitlines():
        if line.startswith("Traceback (most recent call last):"):
            out.append(line)
            in_trace = True
        elif in_trace:
            if not line.startswith(" "):
                out.append(line)
                in_trace = False
        elif line in _CHAINS:
            out.append(line)
    return out


def last_error(r: Run) -> str:
    lines = [
        ln
        for ln in r.stderr.replace(str(r.cwd), "<run>").strip().splitlines()
        if ln and not ln.startswith("larmorx-gpl:")
    ]
    return lines[-1] if lines else ""


def compare_files(t_out: Path, r_out: Path) -> dict[str, str]:
    """tedana's output folder against the replica's (``check_outputs.compare_dirs``: every
    file; empty means identical)."""
    rep = _investigation("check_outputs").compare_dirs(t_out, r_out)
    if "files" in rep:
        # The oracle cache replays files, not empty directories (tedana's empty figures/): an
        # empty directory only the replica's run holds is no difference.
        tf = {p.name for p in t_out.iterdir()} if t_out.is_dir() else set()
        rf = {p.name for p in r_out.iterdir()} if r_out.is_dir() else set()
        odd = {n for n in tf ^ rf if not (n in rf and _empty_dir(r_out / n))}
        if odd:
            rep["files"] = f"differ: {sorted(odd)}"
        else:
            del rep["files"]
    return rep


def compare_runs(checks: CheckList, label: str, t: Run, r: Run) -> bool:
    """tedana's run against the replica's: exit, failure line, messages, every file."""
    ok = checks(f"{label}: exit status", t.code == r.code, detail=f"{t.code} / {r.code}").passed
    if t.code != 0 or r.code != 0:
        te, re_ = last_error(t), last_error(r)
        ok &= checks(
            f"{label}: last error line", te == re_, detail=f"{te!r} / {re_!r}"[:400]
        ).passed
        tx, rx = exceptions(t), exceptions(r)
        ok &= checks(
            f"{label}: traceback chain",
            tx == rx,
            detail=f"{(len(tx) + 1) // 3} exception(s)" if tx == rx else f"{tx} / {rx}"[:400],
        ).passed
    tm, rm = messages(t), messages(r)
    first = next(
        (i for i, (x, y) in enumerate(zip(tm, rm, strict=False)) if x != y), min(len(tm), len(rm))
    )
    same = tm == rm
    ok &= checks(
        f"{label}: messages",
        same,
        detail=f"{len(tm)} lines"
        if same
        else f"line {first}: {tm[first : first + 1]} / {rm[first : first + 1]}"[:400],
    ).passed
    rep = compare_files(t.out, r.out)
    names = sorted(p.name for p in t.out.iterdir()) if t.out.is_dir() else []
    ok &= checks(
        f"{label}: files",
        not rep,
        detail=(
            f"{len(names)} files identical"
            if not rep
            else "; ".join(f"{k}: {v}" for k, v in rep.items())
        )[:400],
    ).passed
    return ok


def _empty_dir(p: Path) -> bool:
    return p.is_dir() and not any(p.iterdir())


def _image_values(p: Path) -> np.ndarray:
    import nibabel as nb

    return np.asanyarray(nb.load(p).dataobj)


def value_differences(
    a: Path, b: Path, only: tuple[str, ...] = ()
) -> dict[str, tuple[int, int, float, float]]:
    """Per file common to two output folders: (values that differ, values, largest relative
    difference, largest absolute difference) of the images' stored values and the confounds
    table's cells (``n/a`` as NaN; a cell that is no number counts, without a difference)."""
    out: dict[str, tuple[int, int, float, float]] = {}
    if not a.is_dir() or not b.is_dir():
        return out
    for name in sorted({p.name for p in a.iterdir()} & {p.name for p in b.iterdir()}):
        if only and name not in only:
            continue
        pa, pb = a / name, b / name
        if name.endswith(".nii.gz"):
            x, y = _image_values(pa), _image_values(pb)
        elif name.endswith(".tsv"):
            x, y = _table_values(pa), _table_values(pb)
        else:
            continue
        if x.shape != y.shape:
            n = max(x.size, y.size)
            out[name] = (n, n, float("inf"), float("inf"))
            continue
        fx, fy = x.astype(np.float64), y.astype(np.float64)
        same = (fx == fy) | (np.isnan(fx) & np.isnan(fy))
        n = int(np.count_nonzero(~same))
        with np.errstate(invalid="ignore", divide="ignore", over="ignore"):
            d = np.abs(fx - fy)[~same]
            rel = d / np.maximum(np.abs(fx), np.abs(fy))[~same]
        d, rel = d[np.isfinite(d)], rel[np.isfinite(rel)]
        out[name] = (
            n,
            int(x.size),
            float(rel.max()) if rel.size else 0.0,
            float(d.max()) if d.size else (0.0 if n == 0 else float("inf")),
        )
    return out


def _table_values(p: Path) -> np.ndarray:
    rows = [ln.split("\t") for ln in p.read_text().splitlines()[1:]]

    def num(c: str) -> float:
        try:
            return float("nan") if c == "n/a" else float(c)
        except ValueError:
            return float("inf")

    return np.array([[num(c) for c in r] for r in rows], dtype=np.float64)


def _summarise(diffs: dict[str, tuple[int, int, float, float]]) -> tuple[int, str]:
    total = sum(d[0] for d in diffs.values())
    parts = [f"{k} {d[0]} of {d[1]} (max rel {d[2]:.3g})" if d[0] else "" for k, d in diffs.items()]
    return total, "; ".join(p for p in parts if p) or "identical"


def fix_pass(checks: CheckList, args: list[str], reproduced: Run, base: Path) -> Run:
    """D13: the replica's default run against its reproduced run (newest libraries); a case
    run twice names what either of its default runs named (the first writes the files)."""
    d = Path(tempfile.mkdtemp(prefix="fix_", dir=base))
    once = [a for a in args if a != "@twice"]
    first = run_replica("newest", once, d, defects.FIX)
    f = run_replica("newest", once, d, defects.FIX) if "@twice" in args else first
    named = first.stderr + f.stderr if f is not first else f.stderr
    entries = defects.notices_in(named)
    others = []
    if f is not first:
        # The first of two runs: its outcome is visible too (its error, before the second run
        # meets the files it wrote), so a defect it names is compared with reproduce's first run.
        d1 = Path(tempfile.mkdtemp(prefix="fix1_", dir=base))
        r1 = run_replica("newest", once, d1)
        if first.code != r1.code:
            others.append(f"the first run's exit {r1.code} -> {first.code}")
        elif first.code != 0 and last_error(first) != last_error(r1):
            others.append("the first run's error")
    if f.code != reproduced.code:
        others.append(f"exit {reproduced.code} -> {f.code}")
    elif f.code != 0 and last_error(f) != last_error(reproduced):
        others.append("the error")
    if messages(f) != messages(reproduced):
        others.append("messages")
    na = sorted(p.name for p in reproduced.out.iterdir()) if reproduced.out.is_dir() else []
    nb_ = sorted(p.name for p in f.out.iterdir()) if f.out.is_dir() else []
    if na != nb_:
        others.append(f"files {sorted(set(na) ^ set(nb_))[:4]}")
    diffs = value_differences(reproduced.out, f.out)
    total = sum(v[0] for v in diffs.values())
    cells = sum(v[1] for v in diffs.values())
    largest = max((v[3] for v in diffs.values() if v[0]), default=0.0)
    defects.add_fix_check(
        checks,
        entries,
        defects.Change(total, cells, largest),
        other_changes="; ".join(others),
    )
    return f


# ------------------------------------------------------------------------------------------------
# Cases


def _inputs() -> Path | None:
    ws = workspace()
    return None if ws is None else ws / INPUTS


def _absolute(args: list[str], base: Path) -> list[str]:
    """The failure cases' relative input names made absolute (under ``base``)."""
    return [str(base / a) if a.endswith(".nii.gz") and not os.path.isabs(a) else a for a in args]


#: Synthetic cases of the smoke tier (the rest: standard).
SMOKE = (
    "syn-loglin",
    "syn-curvefit",
    "syn-ts-curve",
    "syn-exclude",
    "syn-zcat",
    "syn-inf",
    "x-nomask-f32",
    "x-paid",
    "x-verbose-orig-prefix",
    "x-overwrite",
)


@functools.cache
def _all_cases() -> list[tuple[str, str, str, Any]]:
    """(id, tier, description, payload) of every case."""
    base = _inputs()
    out: list[tuple[str, str, str, Any]] = []
    if base is None:
        return out
    stages = _investigation("check_stages")
    failures = _investigation("check_failures")
    for name, args in stages.real_cases():
        out.append(
            (
                f"fmriprep/{name}",
                "standard",
                "fMRIPrep 25.2.6's t2smap call"
                + (" with --fittype loglin" if "loglin" in name else ""),
                ("fmriprep", name, args),
            )
        )
    for name, args in _synthetic_cases(base):
        tier = "smoke" if name in SMOKE else "standard"
        out.append((f"syn/{name}", tier, _describe(args), ("syn", name, args)))
    fdir = base / "failures"
    for k, (defect, args) in enumerate(failures.CASES):
        out.append(
            (
                f"fail/{k:02d}-{(defect or 'refused').lower()}",
                "smoke",
                f"{defect or 'a refused input'}: {_describe(args)}",
                ("fail", defect, _absolute(args, fdir)),
            )
        )
    # ANALYZE 7.5 and MGH/MGZ inputs (relay RS2): their inputs are written with the others
    formats = _investigation("check_formats")
    for name, tier, args in formats.CASES:
        full = formats.arguments(args, base / "formats")
        out.append((f"formats/{name}", tier, _describe(full), ("syn", name, full)))
    # tedana's own spread: fMRIPrep's ds003592 call and two synthetic cases in its form
    syn = base / "synthetic"
    te_ms = ["-e", "12", "28", "44"]
    mask = ["--mask", str(syn / "mask.nii.gz")]
    echoes = ["-d", *(str(syn / f"f32-e{i}.nii.gz") for i in (1, 2, 3))]
    for name, args in stages.real_cases():
        if name == "ds003592-sub-280-rest-curvefit":
            out.append(
                (
                    f"spread/{name}",
                    "standard",
                    "tedana's own spread on fMRIPrep's ds003592 call",
                    ("spread", name, args),
                )
            )
    for fit in ("curvefit", "loglin"):
        out.append(
            (
                f"spread/syn-{fit}",
                "standard",
                f"tedana's own spread on synthetic echoes (fMRIPrep's form, --fittype {fit})",
                ("spread", f"syn-{fit}", [*echoes, *te_ms, *mask, "--fittype", fit]),
            )
        )
    return out


def _generate(base: Path) -> list[tuple[str, list[str]]]:
    """Writes the synthetic and failure cases' inputs under ``base``; their (name, arguments)."""
    stages = _investigation("check_stages")
    outputs = _investigation("check_outputs")
    failures = _investigation("check_failures")
    base.mkdir(parents=True, exist_ok=True)
    extra = outputs.make_extra(base)
    cases = stages.make_synthetic(base) + extra + _with_masks(base, extra)
    fdir = base / "failures"
    fdir.mkdir(parents=True, exist_ok=True)
    failures.make(fdir)
    _investigation("check_formats").make(base / "formats")
    return cases


def _generator_digest() -> str:
    import hashlib

    h = hashlib.sha256()
    for path in (
        Path(__file__),
        INVESTIGATION / "check_stages.py",
        INVESTIGATION / "check_outputs.py",
        INVESTIGATION / "check_failures.py",
        INVESTIGATION / "check_formats.py",
    ):
        h.update(path.read_bytes())
    return h.hexdigest()[:16]


def _synthetic_cases(base: Path) -> list[tuple[str, list[str]]]:
    """The synthetic cases, their inputs written once: the first process (of pytest-xdist's
    workers, which all collect the suite) writes them under a lock and leaves a marker of the
    generators' code; the others draw the same cases in a temporary folder and name the files
    under ``base``, so no input is rewritten while another process reads it."""
    import fcntl

    base.mkdir(parents=True, exist_ok=True)
    marker = base / f".generated-{_generator_digest()}"
    with open(base / ".lock", "w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if not marker.exists():
            cases = _generate(base)
            marker.touch()
            return cases
    tmp = Path(tempfile.mkdtemp(prefix="lx-tedana-gen_"))
    try:
        cases = _generate(tmp)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    return [(n, [a.replace(str(tmp), str(base)) for a in args]) for n, args in cases]


def _with_masks(base: Path, extra: list[tuple[str, list[str]]]) -> list[tuple[str, list[str]]]:
    """The nine-echo and one-volume cases of ``check_outputs.py`` again with a mask of ones:
    without ``--mask`` nilearn finds no mask in their small images (TED-03), so they stop before
    the fit; with it they reach the paths they are for (nine echoes per volume and over all
    volumes, the weights' pairwise sum; a single volume)."""
    import nibabel as nb

    out = []
    for name, args in extra:
        if not name.startswith(("x-nine", "x-one-volume")):
            continue
        first = next(a for a in args if a.endswith(".nii.gz"))
        img = nb.load(first)
        mask = base / "extra" / f"ones-{name.split('-')[1]}.nii.gz"
        if not mask.is_file():
            nb.Nifti1Image(np.ones(img.shape[:3], np.uint8), img.affine).to_filename(mask)
        out.append((f"{name}-mask", [*args, "--mask", str(mask)]))
    return out


#: The ANALYZE and MGH file names of the ``formats`` cases (named by their file names).
_FORMATS = (".hdr", ".img", ".hdr.gz", ".img.gz", ".mgh", ".mgz", ".mgh.gz")


def _describe(args: list[str]) -> str:
    """The arguments with every file named by its name (``@twice``: run twice)."""
    words = []
    for a in args:
        if a == "@twice":
            words.append("(run twice)")
        elif a.endswith(".nii.gz"):
            words.append(Path(a).name.removesuffix(".nii.gz"))
        elif a.lower().endswith(_FORMATS):
            words.append(Path(a).name)
        else:
            words.append(a)
    return "t2smap " + " ".join(words)


def cases(tier: str) -> list[Case]:
    return [
        Case(cid, cid.split("/")[0], desc, payload=payload)
        for cid, case_tier, desc, payload in _all_cases()
        if in_tier(case_tier, tier)
    ]


def _run_dir(base: Path, label: str) -> Path:
    d = base / label
    d.mkdir(parents=True)
    return d


def _never(args: list[str]) -> str:
    return "runs twice in one directory (the existing outputs decide)" if "@twice" in args else ""


def _main_case(
    kind: str, args: list[str], checks: CheckList, name: str, defect: str | None
) -> None:
    root = Path(tempfile.mkdtemp(prefix="lx-tedana_"))
    try:
        runs: dict[str, tuple[Run, Run]] = {}
        for env in ("newest", "shared"):
            t = _twice(
                lambda a, c, env=env: run_tedana(env, a, c, never=_never(args)),
                args,
                _run_dir(root, f"tedana-{env}"),
            )
            r = _twice(
                lambda a, c, env=env: run_replica(env, a, c),
                args,
                _run_dir(root, f"replica-{env}"),
            )
            compare_runs(checks, env, t, r)
            runs[env] = (t, r)
        tn, ts = runs["newest"][0], runs["shared"][0]
        diffs = value_differences(ts.out, tn.out)
        total, detail = _summarise(diffs)
        reason = STEP_REASON if "curvefit" in args else LOGLIN_STEP_REASON
        checks(
            "tedana's step: shared -> newest releases",
            True,
            metric="values that differ",
            value=total,
            threshold="reported",
            detail=(detail if total == 0 else f"{detail}; {reason}")[:600],
        )
        fr = fix_pass(checks, args, runs["newest"][1], root)
        if defect is not None:
            checks(
                f"{defect} named by default",
                f"upstream defect {defect} " in fr.stderr,
                detail=last_error(fr)[:300],
            )
        if kind == "fmriprep":
            _fmriprep_files(checks, name, tn, root, args)
        if tn.code != 0 and all(c.passed for c in checks.checks):
            raise Outcome(
                BOTH_ERROR, f"tedana exits {tn.code}; the same from the replica. {last_error(tn)}"
            )
    finally:
        shutil.rmtree(root, ignore_errors=True)


def _node_dir(name: str) -> Path | None:
    ws = workspace()
    if ws is None:
        return None
    for node in sorted(
        (ws / "oracles" / "fmriprep-25.2.6").glob(
            "*/work/fmriprep_25_2_wf/*/*/bold_native_wf/bold_t2smap_wf/t2smap_node"
        )
    ):
        run = node.relative_to(ws / "oracles" / "fmriprep-25.2.6").parts[0].split("-noreconall")[0]
        task = re.search(r"task_([A-Za-z0-9]+)", str(node))
        if task and name.startswith(f"{run}-{task.group(1)}-"):
            return node
    return None


FMRIPREP_FILES = ("T2starmap.nii.gz", "desc-optcom_bold.nii.gz")


def _fmriprep_files(checks: CheckList, name: str, tn: Run, root: Path, args: list[str]) -> None:
    """tedana 26.0.3's and the replica's default outputs against fMRIPrep's own (reported)."""
    node = _node_dir(name)
    if node is None or "loglin" in name:
        return
    diffs = value_differences(node, tn.out, FMRIPREP_FILES)
    total, detail = _summarise(diffs)
    checks(
        f"fMRIPrep's files (tedana {FMRIPREP_TEDANA}) -> tedana {TEDANA}",
        True,
        metric="values that differ",
        value=total,
        threshold="reported",
        detail=f"{detail}; {FMRIPREP_REASON}"[:700],
    )
    d = Path(tempfile.mkdtemp(prefix="default_", dir=root))
    fr = run_replica("newest", args, d, defects.FIX)
    diffs = value_differences(node, fr.out, FMRIPREP_FILES)
    total, detail = _summarise(diffs)
    checks(
        f"fMRIPrep's files (tedana {FMRIPREP_TEDANA}) -> larmorx's default",
        fr.code == 0,
        metric="values that differ",
        value=total,
        threshold="reported",
        detail=f"{detail}; named: {', '.join(defects.notices_in(fr.stderr)) or 'none'}"[:700],
    )


def _spread_case(name: str, args: list[str], checks: CheckList) -> None:
    root = Path(tempfile.mkdtemp(prefix="lx-tedana_"))
    try:
        ref = run_tedana("newest", args, _run_dir(root, "reference"))
        if ref.code != 0:
            raise Outcome(SKIPPED, f"tedana exits {ref.code}: {last_error(ref)}")
        rep = run_replica("newest", args, _run_dir(root, "replica"))
        compare_runs(checks, "reference setup", ref, rep)
        rep4 = run_replica("newest", args, _run_dir(root, "replica-4"), threads=4)
        d = value_differences(rep.out, rep4.out)
        total, detail = _summarise(d)
        checks(
            "the replica at --n-threads 4",
            total == 0,
            metric="values that differ",
            value=total,
            threshold=0,
            detail=detail[:300],
        )
        binds = sorted({str(Path(a).parent) for a in args if a.endswith(".nii.gz")})
        variants = {
            "numpy's AVX-512 kernels on": lambda c: run_tedana("newest", args, c, avx512=True),
            "--n-threads 4": lambda c: run_tedana(
                "newest", [*args, "--n-threads", "4"], c, threads=4
            ),
            f"the shared releases ({RELEASES['shared']})": lambda c: run_tedana("shared", args, c),
            f"tedana {FMRIPREP_TEDANA} (fMRIPrep 25.2.6's image)": lambda c: run_fmriprep_tedana(
                args, c, [Path(b) for b in binds]
            ),
        }
        for k, (label, go) in enumerate(variants.items()):
            try:
                v = go(_run_dir(root, f"variant-{k}"))
            except Outcome as o:
                checks(f"tedana's spread: {label}", True, threshold="reported", detail=o.reason)
                continue
            d = value_differences(ref.out, v.out)
            total, detail = _summarise(d)
            checks(
                f"tedana's spread: {label}",
                v.code == 0,
                metric="values that differ from the reference setup",
                value=total,
                threshold="reported",
                detail=detail[:600],
            )
    finally:
        shutil.rmtree(root, ignore_errors=True)


def run_case(case: Case, checks: CheckList) -> None:
    kind, name, args = case.payload
    if kind == "spread":
        _spread_case(name, args, checks)
    elif kind == "fail":
        _main_case(kind, args, checks, case.id, name)
    else:
        _main_case(kind, args, checks, name, None)


def suite() -> Suite:
    return Suite(
        name=SUITE,
        title="t2smap replica (bit-exact, GNU GPL version 3)",
        tool=f"`larmorx-gpl tedana t2smap` (crate larmorx-gpl-tedana, translated from tedana "
        f"{TEDANA}'s source; {replica_version()})",
        reference=f"tedana {TEDANA}'s t2smap in both kept environments: newest "
        f"({RELEASES['newest']}) and shared ({RELEASES['shared']}); numpy's OpenBLAS on the "
        "Haswell kernels, numpy's AVX-512 kernels off (D2), one BLAS thread; fMRIPrep 25.2.6's "
        f"own files (tedana {FMRIPREP_TEDANA}) reported",
        thresholds=[
            ("Exit status, last error line", "the same"),
            (
                "Traceback chain",
                "the same exceptions in the same order (chained ones: a fallback's error after "
                "the error it handled), the stacks left out",
            ),
            (
                "Messages",
                "tedana's log lines and Python's warnings identical (the run directory masked; "
                "the replica's `larmorx-gpl:` lines left out; tqdm's bars not compared)",
            ),
            (
                "Files written",
                "the same names (an empty directory only the replica's run holds is none: the "
                "oracle cache does not replay tedana's empty `figures/`)",
            ),
            (
                "Every file",
                "bit-identical (images decompressed; `dataset_description.json` but for the "
                "host and Python fields, D6)",
            ),
            ("Both environments", "each against the replica with its libraries"),
            (
                "tedana's own step",
                "reported, with its reason (SciPy's trust-region method for a curve fit; LAPACK "
                "3.12.0 -> 3.12.1 in numpy's lstsq for a log-linear fit, now and then)",
            ),
            ("D13 fix pass", "a defect named exactly when the default's outputs differ"),
        ],
        cases=cases,
        run_case=run_case,
        packages=("nibabel",),
        notes=(
            "The replica runs as a separate program, the GPL package's (`larmorx-gpl`), "
            "compared with tedana in `reproduce` mode (D13); the default `fix` pass is checked "
            "against the reproduced run. tedana runs as a program through the oracle-output "
            "cache (a launcher per environment and a manifest of its installation)."
        ),
        highlights=defects.summary,
    )
