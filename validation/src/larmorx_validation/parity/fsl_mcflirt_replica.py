# SPDX-License-Identifier: Apache-2.0
"""Parity of the mcflirt replica (``larmorx-nc fsl mcflirt``, FSL Licence) with FSL 6.0.7.17's
``mcflirt``, file by file and bit by bit.

The replica runs as a separate program (``LARMORX_NC_BIN``, else ``crates-nc/target/release``
or ``debug``, else ``larmorx-nc`` on the ``PATH``); this module imports nothing from it. Three
kinds of cases:

- **recorded**: the 137 recorded runs of ``<workspace>/oracles/fsl-6.0.7/mcflirt/``
  (``scripts/run_mcflirt_oracle.py``: real BOLD runs with fMRIPrep's command and every option,
  synthetic series, data types, geometry, orientation, NaN, the command line's errors), the
  replica replayed with the same arguments and environment;
- **fmriprep**: fMRIPrep 25.2.6's own mcflirt calls from the five oracle runs
  (``oracles/fmriprep-25.2.6/*/work/.../mcflirt/command.txt``), run here with FSL 6.0.7.17
  and with the replica; fMRIPrep's matrices (its container's FSL, newer libraries) are
  compared too, as a report;
- **edge**: synthetic edge cases run here with mcflirt: ``-gdt`` with a relative name, the
  histogram costs with few bins, ``-init`` failures, ``-verbose 20``, unwritable and missing
  directories, every ``FSLOUTPUTTYPE``, input types, ANALYZE input, NaN references, odd
  values.

Every case compares, with the replica in ``reproduce`` mode (D13): the exit status (an abort,
signal 6, is the replica's exit 1 after the same text), standard output and standard error
(paths of the run directories replaced, the replica's ``larmorx-nc:`` lines left out), the set
of files written and every file's bytes (images decompressed, the ``descrip`` field left out:
the replica writes its own name there). A second pass runs the default ``fix`` mode and checks
D13's rule (a defect is named exactly when the outputs change). The clean-room original's
agreement with mcflirt is taken from its record (``docs/validation/mri-hmc.json``), not
computed again: the replica is its third column.

Every mcflirt run goes through :func:`run_oracle` (the oracle-output cache's entry point).
"""

from __future__ import annotations

import functools
import gzip
import json
import os
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    Case,
    CaseResult,
    Check,
    CheckList,
    Outcome,
    Suite,
)
from larmorx_validation.parity.mri_hmc import (
    RECORDED,
    TESTDATA,
    manifest,
    oracle_dir,
    rms_deviation,
)

#: The replica's own lines on standard error (notices, its abort line).
REPLICA_PREFIX = "larmorx-nc:"
#: The program's name in usage errors (`larmorx-nc fsl mcflirt` prints mcflirt's usage).
FMRIPREP_RUNS = "oracles/fmriprep-25.2.6"
_TIERS = ("smoke", "standard", "full")
#: Notices about a condition whose effect the comparison may not see.
_SILENT = ("FSL-01", "FSL-05", "FSL-13")


# ------------------------------------------------------------------------------------------------
# The programs


@functools.cache
def workspace() -> Path | None:
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles").is_dir() and (parent / "larmorx-testdata").exists():
            return parent
    return None


def fsldir() -> Path:
    """The FSL 6.0.7.17 installation: ``LARMORX_FSLDIR``, else ``~/fsl``. Never on the
    ``PATH``."""
    return Path(os.environ.get("LARMORX_FSLDIR") or Path.home() / "fsl")


def replica_binary() -> Path | None:
    """``LARMORX_NC_BIN``, else the repository's ``crates-nc/target/{release,debug}``, else
    ``larmorx-nc`` on the ``PATH``."""
    if env := os.environ.get("LARMORX_NC_BIN"):
        return Path(env) if Path(env).is_file() else None
    root = Path(__file__).resolve().parents[4]
    exe = "larmorx-nc.exe" if os.name == "nt" else "larmorx-nc"
    for profile in ("release", "debug"):
        p = root / "crates-nc" / "target" / profile / exe
        if p.is_file():
            return p
    found = shutil.which("larmorx-nc")
    return Path(found) if found else None


def replica_version() -> str:
    """What the replica's program reports (``larmorx-nc <version>``)."""
    exe = replica_binary()
    if exe is None:
        return "not found"
    p = subprocess.run([str(exe), "--version"], capture_output=True, text=True, check=False)
    return p.stdout.strip() or "unknown version"


@dataclass(frozen=True)
class Completed:
    """A run: its exit status (negative: killed by that signal), standard output and error."""

    code: int
    stdout: str
    stderr: str


def _environment(settings: dict[str, str | None]) -> dict[str, str]:
    """mcflirt's clean environment (``oracles/fsl-6.0.7/mcflirt/README.md``): ``HOME``, a
    ``PATH`` without FSL, ``FSLDIR``, ``FSLOUTPUTTYPE=NIFTI_GZ`` unless ``settings`` change it
    (``None`` unsets a variable)."""
    env = {
        "HOME": os.environ.get("HOME", str(Path.home())),
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(fsldir()),
        "FSLOUTPUTTYPE": "NIFTI_GZ",
    }
    for k, v in settings.items():
        if v is None:
            env.pop(k, None)
        else:
            env[k] = v
    return env


def run_oracle(args: list[str], *, cwd: Path, settings: dict[str, str | None]) -> Completed:
    """FSL's ``mcflirt`` with ``args`` in ``cwd`` and its clean environment. Every oracle run of
    this suite goes through here, and through the oracle-output cache
    (:mod:`larmorx_validation.oracle_cache`): mcflirt is single-threaded and draws no random
    numbers, so a run is served once it ran (crashes and hangs always run); every file it writes
    is in ``cwd``."""
    from larmorx_validation import oracle_cache

    argv = [str(fsldir() / "bin" / "mcflirt"), *args]
    p = oracle_cache.run(
        argv,
        env=_environment(settings),
        cwd=cwd,
        outputs=[cwd],
        policy=oracle_cache.Policy(threads=1, family="fsl"),
        runner="fsl-mcflirt-replica",
    )
    return Completed(p.returncode, p.stdout, p.stderr)


def run_replica(
    args: list[str], *, cwd: Path, settings: dict[str, str | None], mode: str, threads: int = 3
) -> Completed:
    """``larmorx-nc fsl mcflirt args`` with the same environment as the oracle's, D13's
    ``mode`` and ``threads`` threads (the results do not depend on them)."""
    exe = replica_binary()
    assert exe is not None
    env = _environment(settings)
    env[defects.ENV] = mode
    env["OMP_NUM_THREADS"] = str(threads)
    p = subprocess.run(
        [str(exe), "fsl", "mcflirt", *args],
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        errors="replace",
        check=False,
    )
    return Completed(p.returncode, p.stdout, p.stderr)


# ------------------------------------------------------------------------------------------------
# Comparing two runs


def _image_bytes(path: Path) -> bytes:
    data = path.read_bytes()
    return gzip.decompress(data) if data[:2] == b"\x1f\x8b" else data


def _without_descrip(data: bytes) -> bytes:
    """A NIfTI-1/2 or ANALYZE header's bytes with ``descrip`` (80 bytes) blanked."""
    size = int.from_bytes(data[:4], "little")
    if size not in (348, 540):
        size = int.from_bytes(data[:4], "big")
    start = 240 if size == 540 else 148
    return data[:start] + b"\0" * 80 + data[start + 80 :]


def _is_image(name: str) -> bool:
    return name.endswith((".nii", ".nii.gz", ".hdr", ".hdr.gz", ".img", ".img.gz"))


def _files(d: Path, exclude: set[str]) -> dict[str, Path]:
    out = {}
    for p in sorted(d.rglob("*")):
        rel = p.relative_to(d).as_posix()
        if p.is_file() and rel not in exclude:
            out[rel] = p
    return out


def _image_values(path: Path) -> np.ndarray | None:
    import nibabel as nib

    try:
        return np.asanyarray(nib.load(path).dataobj)
    except Exception:
        return None


def _differing_values(a: Path, b: Path) -> str:
    va, vb = _image_values(a), _image_values(b)
    if va is None or vb is None or va.shape != vb.shape:
        return "differ"
    fa, fb = va.astype(np.float64), vb.astype(np.float64)
    same = (fa == fb) | (np.isnan(fa) & np.isnan(fb))
    n = int((~same).sum())
    if n == 0:
        return "header differs"
    d = np.abs(fa - fb)[~same]
    d = d[np.isfinite(d)]
    return f"{n} of {fa.size} values differ (max |diff| {d.max() if d.size else float('inf'):.3g})"


def _clean(text: str, run_dir: Path) -> str:
    lines = [ln for ln in text.split("\n") if not ln.startswith(REPLICA_PREFIX)]
    return "\n".join(lines).replace(str(run_dir), "<run>")


def compare(
    checks: CheckList,
    oracle: Completed,
    replica: Completed,
    oracle_dir_: Path,
    replica_dir: Path,
    exclude: set[str],
) -> tuple[int, int]:
    """Exit status, streams, files and bytes; returns (identical files, files)."""
    if oracle.code < 0:
        checks(
            "exit status (mcflirt aborts; the replica exits 1)",
            replica.code == 1,
            detail=f"mcflirt signal {-oracle.code}, replica {replica.code}",
        )
    else:
        checks(
            "exit status", oracle.code == replica.code, detail=f"{oracle.code} vs {replica.code}"
        )
    for stream in ("stdout", "stderr"):
        want = _clean(getattr(oracle, stream), oracle_dir_)
        got = _clean(getattr(replica, stream), replica_dir)
        same = want == got or (oracle.code < 0 and got.startswith(want.rstrip("\n")))
        detail = ""
        if not same:
            wl, gl = want.splitlines(), got.splitlines()
            first = next(
                (i for i, (x, y) in enumerate(zip(wl, gl, strict=False)) if x != y),
                min(len(wl), len(gl)),
            )
            detail = (
                f"line {first + 1}: mcflirt {wl[first] if first < len(wl) else '<end>'!r}, "
                f"replica {gl[first] if first < len(gl) else '<end>'!r}"
            )
        checks(f"{stream} identical", same, detail=detail)
    theirs, ours = _files(oracle_dir_, exclude), _files(replica_dir, exclude)
    checks(
        "files written",
        set(theirs) == set(ours),
        detail=f"only mcflirt {sorted(set(theirs) - set(ours))[:6]}, "
        f"only the replica {sorted(set(ours) - set(theirs))[:6]}",
    )
    identical, different = 0, []
    for name in sorted(set(theirs) & set(ours)):
        a, b = theirs[name], ours[name]
        if _is_image(name):
            da, db = _image_bytes(a), _image_bytes(b)
            if name.endswith((".img", ".img.gz")):
                same = da == db
            else:
                same = _without_descrip(da) == _without_descrip(db)
            if not same:
                different.append(f"{name}: {_differing_values(a, b)}")
        else:
            same = a.read_bytes() == b.read_bytes()
            if not same:
                different.append(name)
        identical += same
    total = len(set(theirs) & set(ours))
    checks(
        "every file bit-identical",
        not different,
        metric="files",
        value=f"{identical}/{total}",
        threshold="all",
        detail="; ".join(different[:6]),
    )
    return identical, total


def fix_pass(
    checks: CheckList,
    args_for: Any,
    settings: dict[str, str | None],
    reproduced_dir: Path,
    reproduced: Completed,
    exclude: set[str],
) -> None:
    """D13: the replica in its default ``fix`` mode against its ``reproduce`` run: the defects
    named and what changed (named exactly when something changed). ``args_for(dir)`` prepares
    a run directory and returns the arguments for it."""
    with tempfile.TemporaryDirectory(prefix="lx-nc-fix-") as name:
        tmp = Path(name)
        args = args_for(tmp)
        r = run_replica(args, cwd=tmp, settings=settings, mode=defects.FIX)
        entries = defects.notices_in(r.stderr)
        others = []
        if r.code != reproduced.code:
            others.append(f"exit {reproduced.code} -> {r.code}")
        a, b = _files(reproduced_dir, exclude), _files(tmp, exclude)
        if set(a) != set(b):
            others.append(f"files {sorted(set(a) ^ set(b))[:4]}")
        change = defects.Change(0, 0, 0.0)
        for key in sorted(set(a) & set(b)):
            if _is_image(key) and not key.endswith((".img", ".img.gz")):
                change = change + defects.Change.of(_image_values(b[key]), _image_values(a[key]))
                ha, hb = _image_bytes(a[key]), _image_bytes(b[key])
                off = 352 if int.from_bytes(ha[:4], "little") == 348 else 544
                if _without_descrip(ha[:off]) != _without_descrip(hb[:off]):
                    others.append(f"{key} header")
            elif not _is_image(key) and a[key].read_bytes() != b[key].read_bytes():
                ta = a[key].read_text(errors="replace").split()
                tb = b[key].read_text(errors="replace").split()
                try:
                    va = np.array([float(x) for x in ta])
                    vb = np.array([float(x) for x in tb])
                    change = change + defects.Change.of(vb, va)
                except ValueError:
                    others.append(key)
        defects.add_fix_check(
            checks, entries, change, other_changes="; ".join(others), silent_ok=_SILENT
        )


def _agree_on_errors(checks: CheckList, oracle_codes: list[int], stderr: str) -> None:
    """A case where mcflirt fails and the replica agrees in every check ends as
    ``both-error``: both reject the input, with the same messages."""
    if any(c != 0 for c in oracle_codes) and all(c.passed for c in checks.checks):
        tail = " ".join(stderr.split())[:200]
        raise Outcome(
            BOTH_ERROR, f"mcflirt exits {oracle_codes}; the same from the replica. {tail}"
        )


# ------------------------------------------------------------------------------------------------
# Recorded runs


def _testdata(rel: str) -> str:
    import larmorx_testdata as td

    return str(td.get(rel))


def _recorded_case(name: str, checks: CheckList) -> None:
    root = oracle_dir()
    if root is None:
        raise Outcome("skipped", "the recorded mcflirt runs were not found")
    rdir = root / "runs" / name
    run = json.loads((rdir / "run.json").read_text(encoding="utf-8"))
    marker = RECORDED + "runs/" + name + "/"
    outputs = set(run.get("outputs", {}))
    settings: dict[str, str | None] = {
        k: v for k, v in run.get("env", {}).items() if k not in ("HOME", "PATH", "FSLDIR")
    }
    if "FSLOUTPUTTYPE" not in run.get("env", {}):
        settings["FSLOUTPUTTYPE"] = None
    local_inputs: set[str] = set()

    def map_arg(arg: str, tmp: Path, is_out: bool) -> str:
        if marker in arg and arg.startswith("/"):
            rel = arg.split(marker, 1)[1]
            if not is_out:
                local_inputs.add(rel)
            return str(tmp / rel)
        if TESTDATA in arg and arg.startswith("/"):
            return _testdata(arg.split(TESTDATA, 1)[1])
        if RECORDED in arg and arg.startswith("/"):
            return str(root / arg.split(RECORDED, 1)[1])
        return arg

    steps = []
    for step in run["steps"]:
        argv = step["argv"][1:]
        steps.append(argv)

    def prepare(tmp: Path) -> list[list[str]]:
        mapped = [
            [map_arg(a, tmp, i > 0 and argv[i - 1] in ("-out", "-o")) for i, a in enumerate(argv)]
            for argv in steps
        ]
        # The inputs the recording made in the run directory (references, -init files).
        for f in rdir.iterdir():
            if f.is_file() and f.name not in outputs and not f.name.startswith(("std", "run.json")):
                shutil.copy2(f, tmp / f.name)
                local_inputs.add(f.name)
        for rel in sorted(local_inputs):
            src = rdir / rel
            if src.is_file() and not (tmp / rel).exists():
                (tmp / rel).parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(src, tmp / rel)
        return mapped

    try:
        with tempfile.TemporaryDirectory(prefix="lx-nc-") as name_:
            tmp = Path(name_)
            mapped = prepare(tmp)
            results = [
                run_replica(a, cwd=tmp, settings=settings, mode=defects.REPRODUCE) for a in mapped
            ]
            # The recorded streams, as one run per step.
            for k, step in enumerate(run["steps"]):
                out = rdir / f"stdout{k}.txt"
                err = rdir / f"stderr{k}.txt"
                oracle = Completed(
                    step["exit"],
                    out.read_text(errors="replace").replace(str(rdir), "<run>")
                    if out.is_file()
                    else "",
                    err.read_text(errors="replace").replace(str(rdir), "<run>")
                    if err.is_file()
                    else "",
                )
                rep = Completed(
                    results[k].code,
                    results[k].stdout.replace(str(tmp), "<run>"),
                    results[k].stderr.replace(str(tmp), "<run>"),
                )
                if k == len(run["steps"]) - 1:
                    exclude = {"run.json"} | {f for f in _files(tmp, set()) if f in local_inputs}
                    exclude |= {f.name for f in rdir.iterdir() if f.name.startswith("std")}
                    compare(checks, oracle, rep, rdir, tmp, exclude)
                else:
                    checks(f"step {k}: exit status", oracle.code == rep.code)
            if all(r.code == 0 for r in results) and len(mapped) == 1:
                fix_pass(checks, lambda t: prepare(t)[0], settings, tmp, results[0], exclude)
    except FileNotFoundError as e:
        raise Outcome("skipped", f"input not available: {e}") from e
    _agree_on_errors(checks, [s["exit"] for s in run["steps"]], "")


# ------------------------------------------------------------------------------------------------
# fMRIPrep's calls


@functools.cache
def fmriprep_calls() -> list[tuple[str, Path]]:
    """``(run, mcflirt node directory)`` of every mcflirt call of the fMRIPrep oracle runs."""
    ws = workspace()
    if ws is None:
        return []
    out = []
    for cmd in sorted((ws / FMRIPREP_RUNS).glob("*/work/**/bold_hmc_wf/mcflirt/command.txt")):
        run = cmd.relative_to(ws / FMRIPREP_RUNS).parts[0]
        out.append((run, cmd.parent))
    return out


def _fmriprep_case(payload: tuple[str, str], checks: CheckList) -> None:
    run, node = payload
    ws = workspace()
    assert ws is not None
    rdir = ws / FMRIPREP_RUNS / run
    info = (rdir / "RUN_INFO.txt").read_text(encoding="utf-8")
    bids = next(ln.split(": ", 1)[1] for ln in info.splitlines() if ln.startswith("BIDS input"))
    argv = (Path(node) / "command.txt").read_text(encoding="utf-8").split()[1:]
    mounts = {
        "/data/": bids + "/",
        "/out/": str(rdir / "out") + "/",
        "/work/": str(rdir / "work") + "/",
    }

    def mapped(tmp: Path) -> list[str]:
        out = []
        for i, a in enumerate(argv):
            if i > 0 and argv[i - 1] == "-out":
                out.append(Path(a).name)
                continue
            for k, v in mounts.items():
                if a.startswith(k):
                    a = v + a[len(k) :]
            out.append(a)
        return out

    missing = [a for a in mapped(Path()) if a.startswith("/") and not Path(a).exists()]
    if missing:
        raise Outcome("skipped", f"input not available: {missing[0]}")
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-fp-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-fp-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        oracle = run_oracle(mapped(a), cwd=a, settings={})
        replica = run_replica(mapped(b), cwd=b, settings={}, mode=defects.REPRODUCE)
        compare(
            checks,
            Completed(oracle.code, oracle.stdout.replace(str(a), "<run>"), oracle.stderr),
            Completed(replica.code, replica.stdout.replace(str(b), "<run>"), replica.stderr),
            a,
            b,
            set(),
        )
        # fMRIPrep's own matrices: its container's mcflirt (fsl-mcflirt 2111.0 with
        # fsl-newimage 2501.4 and fsl-miscmaths 2412.4).
        mats_dir = next(Path(node).glob("*.mat"), None)
        ours = next(b.glob("*.mat"), None)
        if mats_dir is not None and ours is not None:
            fa = sorted(mats_dir.glob("MAT_*"))
            same = sum(
                f.read_bytes() == (ours / f.name).read_bytes()
                for f in fa
                if (ours / f.name).is_file()
            )
            devs = []
            for f in fa:
                if (ours / f.name).is_file():
                    m1 = np.loadtxt(f)
                    m2 = np.loadtxt(ours / f.name)
                    devs.append(rms_deviation(m1, m2, np.zeros(3)))
            checks.add(
                Check(
                    "fMRIPrep's matrices (its container's FSL) identical as text",
                    True,
                    metric="files",
                    value=f"{same}/{len(fa)}",
                    threshold="reported",
                    detail=f"max RMS deviation {max(devs, default=0.0):.3g} mm (radius 80 mm, "
                    "about the origin)",
                )
            )
        if oracle.code == 0 and replica.code == 0:
            fix_pass(checks, mapped, {}, b, replica, set())
    _agree_on_errors(checks, [oracle.code], oracle.stderr)


# ------------------------------------------------------------------------------------------------
# Edge cases


@dataclass(frozen=True)
class Edge:
    """A synthetic case: mcflirt's arguments (``@in`` and ``@ref`` the generated inputs),
    the environment, and how its directory is prepared."""

    args: tuple[str, ...]
    settings: tuple[tuple[str, str | None], ...] = ()
    inputs: str = "small"
    description: str = ""
    setup: str = ""


def _write_like(src: Path, dst: Path, data: np.ndarray, dtype: Any, analyze: bool = False) -> None:
    import nibabel as nib

    img = nib.load(src)
    if analyze:
        out = nib.AnalyzeImage(data.astype(dtype), img.affine)
    else:
        hdr = img.header.copy()
        hdr.set_data_dtype(dtype)
        out = nib.Nifti1Image(data.astype(dtype), img.affine, hdr)
        out.header.set_slope_inter(1.0, 0.0)
    nib.save(out, dst)


def _make_inputs(kind: str, d: Path) -> dict[str, str]:
    """The generated inputs of ``kind`` in ``d`` (deterministic: derived from the test data's
    synthetic series)."""
    import nibabel as nib

    small = Path(_testdata("synthetic/hmc/motion/small.nii.gz"))
    ref = Path(_testdata("synthetic/hmc/phantom/ref.nii.gz"))
    data = np.asanyarray(nib.load(small).dataobj).astype(np.float32)
    files = {"in": str(small), "ref": str(ref)}
    if kind == "tiny":
        files["in"] = _testdata("synthetic/hmc/geometry/tiny.nii.gz")
    elif kind == "int8":
        scaled = np.clip(np.round(data / data.max() * 250 - 125), -128, 127)
        _write_like(small, d / "in.nii.gz", scaled, np.int8)
        files["in"] = "in.nii.gz"
    elif kind == "int32":
        _write_like(small, d / "in.nii.gz", np.round(data * 7), np.int32)
        files["in"] = "in.nii.gz"
    elif kind == "analyze":
        _write_like(small, d / "in.hdr", np.round(data), np.int16, analyze=True)
        files["in"] = "in.hdr"
    elif kind == "rgb":
        img = nib.load(small)
        shape = img.shape[:3]
        rgb = np.zeros(shape, dtype=[("R", "u1"), ("G", "u1"), ("B", "u1")])
        nib.save(nib.Nifti1Image(rgb, img.affine), d / "in.nii.gz")
        files["in"] = "in.nii.gz"
    elif kind == "nan-ref":
        r = np.asanyarray(nib.load(ref).dataobj).astype(np.float32)
        r[r.shape[0] // 2, r.shape[1] // 2, r.shape[2] // 2] = np.nan
        _write_like(ref, d / "ref.nii.gz", r, np.float32)
        files["ref"] = "ref.nii.gz"
    return files


EDGES: dict[str, Edge] = {
    "gdt": Edge(
        ("-in", "@in", "-out", "out", "-gdt", "-mats", "-plots"),
        description="-gdt with a relative -out: the gradient images are registered (FSL-04)",
    ),
    "bins4-mutualinfo": Edge(
        ("-in", "@in", "-out", "out", "-cost", "mutualinfo", "-bins", "4", "-stages", "1", "-mats"),
        description="mutual information with 4 bins",
    ),
    "bins1-normmi": Edge(
        ("-in", "@in", "-out", "out", "-cost", "normmi", "-bins", "1", "-stages", "1", "-mats"),
        description="normalised mutual information with 1 bin",
    ),
    "bins2-corratio": Edge(
        ("-in", "@in", "-out", "out", "-cost", "corratio", "-bins", "2", "-stages", "1", "-mats"),
        description="the correlation ratio with 2 bins",
    ),
    "in-missing": Edge(
        ("-in", "nosuch", "-out", "out", "-mats"),
        description="-in names no file",
    ),
    "init-missing": Edge(
        ("-in", "@in", "-out", "out", "-init", "nosuch.mat", "-mats"),
        description="-init names no file",
    ),
    "init-short": Edge(
        ("-in", "@in", "-out", "out", "-init", "short.mat", "-mats"),
        setup="short",
        description="-init with 3 numbers",
    ),
    "init-3x3": Edge(
        ("-in", "@in", "-out", "out", "-init", "m33.mat", "-mats"),
        setup="m33",
        description="-init with a 3 x 3 matrix",
    ),
    "verbose20": Edge(
        ("-in", "@in", "-out", "out", "-verbose", "20", "-stages", "1", "-mats"),
        inputs="tiny",
        description="-verbose 20: every cost evaluation's matrix printed",
    ),
    "verbose20-report": Edge(
        ("-in", "@in", "-out", "out", "-report", "-verbose", "20", "-stages", "2", "-plots"),
        inputs="tiny",
        description="-verbose 20 with -report (fixed notation after a matrix)",
    ),
    "out-dir-missing": Edge(
        ("-in", "@in", "-out", "missing/out", "-mats", "-plots"),
        description="-out in a directory that does not exist (FSL-20)",
    ),
    "out-dir-readonly": Edge(
        ("-in", "@in", "-out", "ro/out", "-mats"),
        setup="readonly",
        description="-out in a directory without write permission",
    ),
    "mat-exists": Edge(
        ("-in", "@in", "-out", "out", "-mats"),
        setup="matfile",
        description="a file named out.mat exists: the matrices go to out.mat+",
    ),
    "fsloutputtype-pair": Edge(
        ("-in", "@in", "-out", "out", "-mats"),
        (("FSLOUTPUTTYPE", "NIFTI_PAIR"),),
        description="FSLOUTPUTTYPE=NIFTI_PAIR",
    ),
    "fsloutputtype-pair-gz": Edge(
        ("-in", "@in", "-out", "out", "-mats"),
        (("FSLOUTPUTTYPE", "NIFTI_PAIR_GZ"),),
        description="FSLOUTPUTTYPE=NIFTI_PAIR_GZ",
    ),
    "fsloutputtype-nifti2-gz": Edge(
        ("-in", "@in", "-out", "out", "-mats"),
        (("FSLOUTPUTTYPE", "NIFTI2_GZ"),),
        description="FSLOUTPUTTYPE=NIFTI2_GZ",
    ),
    "fsloutputtype-analyze": Edge(
        ("-in", "@in", "-out", "out", "-mats"),
        (("FSLOUTPUTTYPE", "ANALYZE"),),
        description="FSLOUTPUTTYPE=ANALYZE",
    ),
    "fsloutputtype-bogus": Edge(
        ("-in", "@in", "-out", "out", "-mats"),
        (("FSLOUTPUTTYPE", "BOGUS"),),
        description="an unknown FSLOUTPUTTYPE",
    ),
    "stages0-meanvol": Edge(
        ("-in", "@in", "-out", "out", "-stages", "0", "-meanvol", "-mats", "-plots"),
        description="-stages 0 with -meanvol",
    ),
    "stages0-refvol": Edge(
        ("-in", "@in", "-out", "out", "-stages", "0", "-mats", "-rmsabs"),
        description="-stages 0 with a volume of the series as the reference",
    ),
    "dof13": Edge(
        ("-in", "@in", "-out", "out", "-dof", "13", "-stages", "1", "-mats"),
        description="-dof 13: 12 used, with a message per volume",
    ),
    "int8": Edge(
        ("-in", "@in", "-out", "out", "-mats"),
        inputs="int8",
        description="int8 input (written as uint8: FSL-09)",
    ),
    "int32": Edge(
        ("-in", "@in", "-out", "out", "-mats"), inputs="int32", description="int32 input"
    ),
    "analyze-input": Edge(
        ("-in", "@in", "-out", "out", "-mats"),
        inputs="analyze",
        description="ANALYZE 7.5 input (.hdr/.img)",
    ),
    "rgb-input": Edge(
        ("-in", "@in", "-out", "out", "-mats"),
        inputs="rgb",
        description="RGB input (a type FSL does not read)",
    ),
    "nan-reffile": Edge(
        ("-in", "@in", "-out", "out", "-reffile", "@ref", "-mats", "-plots"),
        inputs="nan-ref",
        description="a reference with a NaN voxel (FSL-14)",
    ),
    "refvol-last": Edge(
        ("-in", "@in", "-out", "out", "-refvol", "11", "-mats", "-rmsrel"),
        description="the last volume as the reference",
    ),
    "meanvol-reffile-sinc-stats": Edge(
        (
            "-in",
            "@in",
            "-out",
            "out.nii.gz",
            "-reffile",
            "@ref",
            "-meanvol",
            "-sinc_final",
            "-stats",
            "-mats",
        ),
        description="-meanvol from a reference file, sinc, -stats, -out with its extension",
    ),
    "rotation0": Edge(
        ("-in", "@in", "-out", "out", "-rotation", "0", "-stages", "1", "-mats"),
        description="-rotation 0 (infinite rotation tolerances)",
    ),
    "smooth-negative": Edge(
        ("-in", "@in", "-out", "out", "-smooth", "-1", "-stages", "1", "-mats"),
        description="a negative -smooth",
    ),
    "scaling-hist": Edge(
        ("-in", "@in", "-out", "out", "-scaling", "2", "-hist", "-stages", "1", "-mats"),
        description="-scaling and -hist (no effect: FSL-05)",
    ),
    "fov0-2d": Edge(
        ("-in", "@in", "-out", "out", "-fov", "0", "-2d", "-stages", "1", "-mats", "-plots"),
        description="-2d with -fov 0",
    ),
}
#: Cases whose run takes longer (sinc stages).
_EDGE_TIER = {"meanvol-reffile-sinc-stats": "standard"}


def _edge_case(name: str, checks: CheckList) -> None:
    edge = EDGES[name]
    settings = dict(edge.settings)

    def prepare(tmp: Path) -> list[str]:
        files = _make_inputs(edge.inputs, tmp)
        if edge.setup == "short":
            (tmp / "short.mat").write_text("1 0 0\n", encoding="utf-8")
        elif edge.setup == "m33":
            (tmp / "m33.mat").write_text("1 0 0\n0 1 0\n0 0 1\n", encoding="utf-8")
        elif edge.setup == "readonly":
            (tmp / "ro").mkdir()
            (tmp / "ro").chmod(0o555)
        elif edge.setup == "matfile":
            (tmp / "out.mat").write_text("not a directory\n", encoding="utf-8")
        return [files.get(a[1:], a) if a.startswith("@") else a for a in edge.args]

    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-edge-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-edge-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        try:
            oracle = run_oracle(prepare(a), cwd=a, settings=settings)
            args = prepare(b)
            replica = run_replica(args, cwd=b, settings=settings, mode=defects.REPRODUCE)
            generated = {
                f
                for f in _files(b, set())
                if f.startswith(("in.", "ref.", "short.", "m33.", "out.mat"))
            }
            # The generated inputs are compared too where the run rewrote them (out.mat).
            exclude = {f for f in generated if f != "out.mat"}
            compare(
                checks,
                Completed(
                    oracle.code,
                    oracle.stdout.replace(str(a), "<run>"),
                    oracle.stderr.replace(str(a), "<run>"),
                ),
                Completed(
                    replica.code,
                    replica.stdout.replace(str(b), "<run>"),
                    replica.stderr.replace(str(b), "<run>"),
                ),
                a,
                b,
                exclude,
            )
            if replica.code == 0:
                fix_pass(checks, prepare, settings, b, replica, exclude)
        finally:
            for d in (a, b):
                ro = d / "ro"
                if ro.exists():
                    ro.chmod(0o755)
    _agree_on_errors(checks, [oracle.code], oracle.stderr)


# ------------------------------------------------------------------------------------------------
# The suite


def _recorded_tier(name: str) -> str:
    if name.startswith("syn/"):
        return "smoke"
    if "stages4" in name or name.endswith("opt-sinc_final"):
        return "full"
    return "standard"


def cases(tier: str) -> list[Case]:
    rank = _TIERS.index(tier)
    out = []
    for name in sorted(manifest()["runs"]):
        if _TIERS.index(_recorded_tier(name)) <= rank:
            parts = name.split("/")
            category = (
                "recorded: real"
                if parts[0] == "real" and not parts[2].startswith("opt-")
                else "recorded: options (ds000005)"
                if parts[0] == "real"
                else f"recorded: synthetic/{parts[1]}"
            )
            out.append(Case(f"recorded/{name}", category, name, ("recorded", name)))
    for name, edge in EDGES.items():
        if _TIERS.index(_EDGE_TIER.get(name, "smoke")) <= rank:
            out.append(Case(f"edge/{name}", "edge cases", edge.description, ("edge", name)))
    if rank >= 1:
        for run, node in fmriprep_calls():
            if "ds005454" in run and rank < 2:
                continue
            bold = node.parent.parent.parent.name.removeprefix("bold_").removesuffix("_wf")
            out.append(
                Case(
                    f"fmriprep/{run}/{bold}",
                    "fMRIPrep's calls",
                    f"fMRIPrep 25.2.6's mcflirt call ({run}, {bold})",
                    ("fmriprep", (run, str(node))),
                )
            )
    return out


def run_case(case: Case, checks: CheckList) -> None:
    if replica_binary() is None:
        raise Outcome("skipped", "the larmorx-nc program was not found (LARMORX_NC_BIN)")
    kind, payload = case.payload
    if kind != "recorded" and not (fsldir() / "bin" / "mcflirt").is_file():
        raise Outcome("skipped", f"FSL's mcflirt not found ({fsldir()}/bin/mcflirt)")
    if kind == "recorded":
        _recorded_case(payload, checks)
    elif kind == "edge":
        _edge_case(payload, checks)
    else:
        _fmriprep_case(payload, checks)


def _original_record() -> dict[str, dict[str, Any]]:
    """The clean-room original's record (``docs/validation/mri-hmc.json``), by case."""
    path = Path(__file__).resolve().parents[4] / "docs" / "validation" / "mri-hmc.json"
    if not path.is_file():
        return {}
    data = json.loads(path.read_text(encoding="utf-8"))
    return {r["case"]: r for r in data["results"]}


def _highlights(results: list[CaseResult]) -> list[str]:
    identical = [r for r in results if r.status == "pass"]
    lines = [
        f"- **Bit-identical to mcflirt as built** (every file, the exit status and both streams; "
        f"the replica in `reproduce` mode): {len(identical)} of "
        f"{sum(r.status in ('pass', 'fail') for r in results)} compared cases; "
        f"{sum(r.status == BOTH_ERROR for r in results)} where both end with the same error.",
    ]
    original = _original_record()
    rows = []
    for r in results:
        if not r.case.startswith("recorded/"):
            continue
        o = original.get(r.case.removeprefix("recorded/"))
        if o is None:
            continue
        measured = o.get("measured", {})
        p95 = next((v for k, v in measured.items() if "95th percentile" in k), "–")
        same = next((v for k, v in o.get("reported", {}).items() if "identical as text" in k), "–")
        files = next((c.value for c in r.checks if c.name == "every file bit-identical"), "–")
        rows.append(
            f"| `{r.case.removeprefix('recorded/')}` | {r.status} ({files} files) | "
            f"{o['status']}: p95 RMS {p95} mm, MAT identical {same} |"
        )
    if rows:
        lines += [
            "",
            "The three columns of the head-motion validation (the clean-room original's numbers "
            "from its record, `docs/validation/mri-hmc.md`, not computed again):",
            "",
            "| Recorded run | Replica vs mcflirt | Clean-room original vs mcflirt |",
            "|---|---|---|",
            *rows,
        ]
    lines += ["", *defects.summary(results)]
    return lines


def suite() -> Suite:
    return Suite(
        name="fsl-mcflirt-replica",
        title="mcflirt replica (bit-exact, FSL Licence)",
        tool="`larmorx-nc fsl mcflirt` (crate larmorx-nc-mcflirt, translated from FSL 6.0.7.17's "
        'source; `lx.mri.hmc(implementation="replica")`)',
        reference="mcflirt from FSL 6.0.7.17 (fsl-mcflirt 2111.0, the conda build at `~/fsl`, "
        "as built: glibc 2.35's maths): the recorded runs in `oracles/fsl-6.0.7/mcflirt/` and "
        "runs made here",
        thresholds=[
            ("Exit status", "the same (an abort: the replica exits 1 after the same text)"),
            (
                "Standard output and error",
                "identical (paths of the run directories replaced; the replica's `larmorx-nc:` lines left out)",
            ),
            ("Files written", "the same names"),
            ("Every file", "bit-identical (images decompressed; `descrip` left out)"),
            ("D13 fix pass", "a defect named exactly when the default's outputs differ"),
        ],
        cases=cases,
        run_case=run_case,
        packages=("nibabel",),
        highlights=_highlights,
        notes=(
            "The replica runs as a separate program, the FSL replica package's (`larmorx-nc`). "
            "It is compared with mcflirt in `reproduce` mode (D13); the default `fix` pass is "
            "checked against the reproduced run. fMRIPrep's own matrices come from its "
            "container's FSL (fsl-mcflirt 2111.0 with newer fsl-newimage and fsl-miscmaths) and "
            "are reported only."
        ),
    )
