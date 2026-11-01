# SPDX-License-Identifier: Apache-2.0
"""Parity of the FLIRT replica (``larmorx-nc fsl flirt`` and ``convert_xfm``, FSL Licence) with
FSL 6.0.7.23's ``flirt`` and ``convert_xfm``, file by file and bit by bit.

The replica runs as a separate program (``LARMORX_NC_BIN``, else ``crates-nc/target/release``
or ``debug``, else ``larmorx-nc`` on the ``PATH``); this module imports nothing from it. Kinds of
cases:

- **fmriprep**: fMRIPrep 25.2.6's own boundary-based registration calls (``fsl_bbr_wf/flt_bbr``)
  of the oracle runs, their inputs taken from the runs' directories and FSL's ``bbr.sch``; three
  runs in the standard tier, every call in the full tier; fMRIPrep's own matrices (its
  container's FSL) compared too, as a report;
- **real**: one BOLD reference and T1w of ds005040 (fMRIPrep's oracle run) with every cost
  function, degrees of freedom, search setting, interpolation, output type, weighting,
  ``-applyxfm`` and ``-applyisoxfm``, the boundary-based options (types, slope, coordinates and
  normals, a field map), every schedule FSL ships, and the 2D mode on one slice;
- **schedule**: written schedules that use every command and option of FLIRT's schedule
  language, and its errors;
- **edge**: the command line's errors and messages, missing files, verbose output;
- **convert_xfm**: every option and failure.

Every case compares, with the replica in ``reproduce`` mode (D13): the exit status (an abort,
signal 6, is the replica's exit 1 after the same text), standard output and error (the run
directory and FSL's program path replaced, the replica's ``larmorx-nc:`` lines left out), the set
of files written and every file's bytes (images decompressed, ``descrip`` left out). A second
pass runs the default ``fix`` mode and checks D13's rule. Every original run goes through the
oracle-output cache (:func:`run_oracle`).
"""

from __future__ import annotations

import functools
import gzip
import os
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from larmorx_validation.parity import defects, fsl_versions
from larmorx_validation.parity.fsl_mcflirt_replica import (
    REPLICA_PREFIX,
    Completed,
    _change,
    _differing_values,
    _files,
    _head,
    _is_image,
    _same_bytes,
    _same_streams,
    _without_descrip,
    replica_binary,
    workspace,
)
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    Case,
    Check,
    CheckList,
    Outcome,
    Suite,
)

SUITE = "fsl-flirt-replica"
FMRIPREP_RUNS = "oracles/fmriprep-25.2.6"
#: The fMRIPrep runs of the standard tier (slice timing, multi-echo, a child).
STANDARD_RUNS = (
    "ds005040-sub-002-noreconall",
    "ds003592-sub-280-noreconall",
    "ds000228-sub-pixar008-noreconall",
)
#: The run the real cases' images come from.
SOURCE_RUN = "ds005040-sub-002-noreconall"
_SILENT = ()


# ------------------------------------------------------------------------------------------------
# The programs


def fsldir() -> Path:
    """FSL 6.0.7.23 (the newest), never on the ``PATH``."""
    return fsl_versions.fsldir(fsl_versions.NEWEST)


def _environment(settings: dict[str, str | None]) -> dict[str, str]:
    env = {
        "HOME": os.environ.get("HOME", str(Path.home())),
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(fsldir()),
        "FSLOUTPUTTYPE": "NIFTI_GZ",
        **fsl_versions.openblas_env(),
    }
    for k, v in settings.items():
        if v is None:
            env.pop(k, None)
        else:
            env[k] = v
    return env


def run_oracle(
    program: str, args: list[str], *, cwd: Path, settings: dict[str, str | None]
) -> Completed:
    """FSL's ``program`` (``flirt`` or ``convert_xfm``) with ``args`` in ``cwd``, through the
    oracle-output cache: FLIRT is single-threaded and draws no random numbers, so a run is
    served once it ran; every file it writes is in ``cwd`` and every file there counts by its
    content."""
    from larmorx_validation import oracle_cache

    argv = [str(fsldir() / "bin" / program), *args]
    p = oracle_cache.run(
        argv,
        env=_environment(settings),
        cwd=cwd,
        outputs=[cwd],
        policy=oracle_cache.Policy(threads=1, family="fsl"),
        runner=SUITE,
        inputs=[p for p in sorted(cwd.rglob("*")) if p.is_file()],
        timeout=3600,
    )
    return Completed(p.returncode, p.stdout, p.stderr)


def run_replica(
    program: str,
    args: list[str],
    *,
    cwd: Path,
    settings: dict[str, str | None],
    mode: str,
    threads: int = 4,
) -> Completed:
    exe = replica_binary()
    assert exe is not None, "the larmorx-nc program is not built"
    env = _environment(settings)
    env[defects.ENV] = mode
    env["OMP_NUM_THREADS"] = str(threads)
    p = subprocess.run(
        [str(exe), "fsl", program, *args],
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        errors="replace",
        check=False,
        timeout=3600,
    )
    return Completed(p.returncode, p.stdout, p.stderr)


def _clean(text: str, run_dir: Path, program: str) -> str:
    lines = [ln for ln in text.split("\n") if not ln.startswith(REPLICA_PREFIX)]
    t = "\n".join(lines).replace(str(run_dir), "<run>")
    return t.replace(str(fsldir() / "bin" / program), program)


def compare(
    checks: CheckList,
    program: str,
    oracle: Completed,
    replica: Completed,
    oracle_dir: Path,
    replica_dir: Path,
) -> tuple[int, int]:
    """Exit status, streams, files and bytes; returns (identical files, files)."""
    if oracle.code < 0:
        checks(
            f"exit status ({program} is killed; the replica exits 1)",
            replica.code == 1,
            detail=f"{program} signal {-oracle.code}, replica {replica.code}",
        )
    else:
        checks(
            "exit status", oracle.code == replica.code, detail=f"{oracle.code} vs {replica.code}"
        )
    for stream in ("stdout", "stderr"):
        want = _clean(getattr(oracle, stream), oracle_dir, program)
        got = _clean(getattr(replica, stream), replica_dir, program)
        same = want == got or (oracle.code < 0 and got.startswith(want.rstrip("\n")))
        detail = ""
        if not same:
            wl, gl = want.splitlines(), got.splitlines()
            first = next(
                (i for i, (x, y) in enumerate(zip(wl, gl, strict=False)) if x != y),
                min(len(wl), len(gl)),
            )
            detail = (
                f"line {first + 1}: FSL {wl[first] if first < len(wl) else '<end>'!r}, "
                f"replica {gl[first] if first < len(gl) else '<end>'!r}"
            )
        checks(f"{stream} identical", same, detail=detail)
    theirs, ours = _files(oracle_dir, set()), _files(replica_dir, set())
    checks(
        "files written",
        set(theirs) == set(ours),
        detail=f"only FSL {sorted(set(theirs) - set(ours))[:6]}, "
        f"only the replica {sorted(set(ours) - set(theirs))[:6]}",
    )
    identical, different = 0, []
    for name in sorted(set(theirs) & set(ours)):
        a, b = theirs[name], ours[name]
        if _is_image(name):
            same = _same_streams(a, b, blank_descrip=True)
            if not same:
                different.append(f"{name}: {_differing_values(a, b)}")
        else:
            same = _same_bytes(a, b)
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
    program: str,
    prepare: Any,
    args: list[str],
    settings: dict[str, str | None],
    reproduced_dir: Path,
    reproduced: Completed,
) -> None:
    """D13: the replica's default ``fix`` run against its ``reproduce`` run (a defect named
    exactly when something changed)."""
    with tempfile.TemporaryDirectory(prefix="lx-nc-flirt-fix-") as name:
        tmp = Path(name)
        prepare(tmp)
        before = set(_files(tmp, set()))
        r = run_replica(program, args, cwd=tmp, settings=settings, mode=defects.FIX)
        entries = defects.notices_in(r.stderr)
        others = []
        if r.code != reproduced.code:
            others.append(f"exit {reproduced.code} -> {r.code}")
        a = {k: v for k, v in _files(reproduced_dir, set()).items()}
        b = {k: v for k, v in _files(tmp, set()).items()}
        if set(a) != set(b):
            others.append(f"files {sorted(set(a) ^ set(b))[:4]}")
        change = defects.Change(0, 0, 0.0)
        for key in sorted((set(a) & set(b)) - before):
            if _is_image(key):
                change = change + _change(b[key], a[key])
                ha, hb = _head(a[key]), _head(b[key])
                off = 352 if int.from_bytes(ha[:4], "little") == 348 else 544
                if _without_descrip(ha[:off]) != _without_descrip(hb[:off]):
                    others.append(f"{key} header")
            elif a[key].read_bytes() != b[key].read_bytes():
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


def _agree_on_errors(checks: CheckList, program: str, code: int, stderr: str) -> None:
    if code != 0 and all(c.passed for c in checks.checks):
        tail = " ".join(stderr.split())[:200]
        raise Outcome(BOTH_ERROR, f"{program} exits {code}; the same from the replica. {tail}")


def _rms80(m1: np.ndarray, m2: np.ndarray) -> float:
    """FSL's RMS deviation of two 4 x 4 matrices (radius 80 mm, about the origin)."""
    d = m1 @ np.linalg.inv(m2) - np.eye(4)
    a, t = d[:3, :3], d[:3, 3]
    return float(np.sqrt(t @ t + (80.0**2 / 5.0) * np.trace(a.T @ a)))


# ------------------------------------------------------------------------------------------------
# Inputs


def _source_dir() -> Path:
    ws = workspace()
    assert ws is not None
    return ws / FMRIPREP_RUNS / SOURCE_RUN


def _bbr_node(run_dir: Path) -> Path | None:
    hits = sorted(run_dir.glob("work/**/fsl_bbr_wf/flt_bbr/command.txt"))
    return hits[0].parent if hits else None


def _save(path: Path, data: np.ndarray, affine: np.ndarray, dtype: Any = np.float32) -> None:
    import nibabel as nib

    img = nib.Nifti1Image(np.asarray(data, dtype=dtype), affine)
    img.header.set_xyzt_units("mm", "sec")
    img.set_qform(affine, 1)
    img.set_sform(affine, 1)
    nib.save(img, str(path))


def _strided(img: Any, k: int) -> tuple[np.ndarray, np.ndarray]:
    data = np.asanyarray(img.dataobj)[::k, ::k, ::k]
    aff = img.affine.copy()
    aff[:3, :3] = aff[:3, :3] * k
    return data, aff


@functools.cache
def inputs() -> Path:
    """The derived inputs (``oracles/inputs/fsl-flirt-replica/``), made once, deterministically,
    from ds005040's fMRIPrep run: the BOLD reference, its 4D series' first volumes, the T1w and
    its white matter at half resolution, a label image, weights, a field map, one slice, boundary
    coordinates and normals, and copies with other data types and voxel sizes."""
    import nibabel as nib

    ws = workspace()
    assert ws is not None
    out = ws / "oracles" / "inputs" / SUITE
    stamp = out / "DONE-v1"
    if stamp.is_file():
        return out
    out.mkdir(parents=True, exist_ok=True)
    src = _source_dir()
    node = _bbr_node(src)
    assert node is not None
    bbr = node.parent
    epi = nib.load(str(src / "out/sub-002/func/sub-002_task-clip_run-1_desc-coreg_boldref.nii.gz"))
    t1 = nib.load(str(next((bbr / "mask_t1w_brain").glob("*_masked.nii.gz"))))
    wm = nib.load(str(next((bbr / "wm_mask").glob("*.nii.gz"))))
    dseg = nib.load(str(src / "out/sub-002/anat/sub-002_dseg.nii.gz"))
    shutil.copy(next((bbr / "lta_to_fsl").glob("out.mat")), out / "init.mat")
    e = np.asanyarray(epi.dataobj).astype(np.float32)
    _save(out / "epi.nii", e, epi.affine)
    _save(out / "epi_i16.nii", np.round(e), epi.affine, np.int16)
    t, ta = _strided(t1, 2)
    _save(out / "t1.nii", t, ta)
    w, wa = _strided(wm, 2)
    _save(out / "wm.nii", (w > 0.5).astype(np.uint8), wa, np.uint8)
    d, da = _strided(dseg, 2)
    _save(out / "dseg.nii", d, da, np.int16)
    _save(out / "dseg_shift.nii", np.roll(d, (2, -1, 1), axis=(0, 1, 2)), da, np.int16)
    _save(out / "refweight.nii", (t > 0).astype(np.float32), ta)
    _save(out / "inweight.nii", (e > np.percentile(e, 30)).astype(np.float32), epi.affine)
    # A smooth field map (rad/s) on the T1w's grid and its mask.
    ii, jj, kk = np.meshgrid(*[np.arange(n) for n in t.shape], indexing="ij")
    c = [n / 2 for n in t.shape]
    r2 = ((ii - c[0]) ** 2 + (jj - c[1] * 1.2) ** 2 + (kk - c[2] * 0.6) ** 2) / (
        0.3 * t.shape[0]
    ) ** 2
    _save(out / "fmap.nii", (120.0 * np.exp(-r2)).astype(np.float32), ta)
    _save(out / "fmapmask.nii", (t > 0).astype(np.float32), ta)
    # One axial slice (2D mode) and the same slice rotated by 6 degrees and shifted.
    z = t.shape[2] // 2
    sl = t[:, :, z : z + 1]
    _save(out / "slice_ref.nii", sl, ta)
    from scipy import ndimage

    rot = ndimage.rotate(sl[:, :, 0], 6.0, reshape=False, order=1)
    rot = ndimage.shift(rot, (2.5, -1.5), order=1)
    _save(out / "slice_in.nii", rot[:, :, None], ta)
    # Boundary coordinates and normals (FLIRT mm: voxel times voxel size) of the half-size WM.
    wb = (w > 0.5).astype(np.float32)
    g = np.gradient(ndimage.gaussian_filter(wb, 1.0))
    inner = ndimage.binary_erosion(wb > 0.5)
    pts = np.argwhere((wb > 0.5) & ~inner)[::7]
    vox = np.abs(np.diag(ta)[:3])
    norms = np.stack([-g[k][tuple(pts.T)] for k in range(3)], axis=1) / vox
    norms = norms / np.maximum(np.linalg.norm(norms, axis=1, keepdims=True), 1e-9)
    np.savetxt(out / "wmcoords.txt", pts * vox, fmt="%.6f")
    np.savetxt(out / "wmnorms.txt", norms, fmt="%.6f")
    # The first 4 volumes of the raw BOLD series (-applyxfm on 4D).
    info = (src / "RUN_INFO.txt").read_text(encoding="utf-8")
    bids = next(ln.split(": ", 1)[1] for ln in info.splitlines() if ln.startswith("BIDS input"))
    bold = nib.load(str(Path(bids) / "sub-002/func/sub-002_task-clip_run-1_bold.nii.gz"))
    b4 = np.asanyarray(bold.dataobj)[..., :4]
    bimg = nib.Nifti1Image(b4, bold.affine, bold.header)
    nib.save(bimg, str(out / "bold4.nii"))
    # Voxel sizes that change FLIRT's base scale: 0.6 mm and 14 mm copies of the T1w's grid.
    small = ta.copy()
    small[:3, :3] = small[:3, :3] * (0.6 / np.abs(small[0, 0]))
    _save(out / "t1_small.nii", t, small)
    big = epi.affine.copy()
    big[:3, :3] = big[:3, :3] * (14.0 / 3.0)
    _save(out / "epi_big.nii", e[::4, ::4, ::3], big)
    # A known rigid motion of the T1w (5 degrees about z, 3 voxels along x), and the full-size T1w.
    ang = np.deg2rad(5.0)
    rot3 = np.array([[np.cos(ang), -np.sin(ang), 0], [np.sin(ang), np.cos(ang), 0], [0, 0, 1]])
    centre = (np.array(t.shape) - 1) / 2
    moved = ndimage.affine_transform(
        t, rot3, offset=centre - rot3 @ centre + np.array([3.0, 0, 0]), order=1
    )
    _save(out / "t1_moved.nii", moved.astype(np.float32), ta)
    shutil.copy(next((bbr / "mask_t1w_brain").glob("*_masked.nii.gz")), out / "t1full.nii.gz")
    stamp.write_text("derived inputs of the fsl-flirt-replica suite, version 1\n", encoding="utf-8")
    return out


# ------------------------------------------------------------------------------------------------
# Cases


@dataclass(frozen=True)
class Run:
    """A case: the program, its arguments (``@name`` names an input copied into the run
    directory), the environment, and files written into the directory first (name, text)."""

    program: str
    args: tuple[str, ...]
    settings: tuple[tuple[str, str | None], ...] = ()
    files: tuple[tuple[str, str], ...] = ()
    fix: bool = True


def _prepare(run: Run, tmp: Path) -> list[str]:
    base: Path | None = None
    out = []
    for a in run.args:
        if a.startswith("@dt:"):
            # a random case's input (differential random testing), built from its recipe
            from larmorx_validation.difftest import inputs as dt_inputs

            src = dt_inputs.path(a[1:])
            dst = tmp / src.name
            if not dst.exists():
                shutil.copy(src, dst)
            out.append(src.name)
        elif a.startswith("@fslsch:"):
            # one of FSL's shipped schedules, read where FSL keeps it
            out.append(str(fsldir() / "etc" / "flirtsch" / a.removeprefix("@fslsch:")))
        elif a.startswith("@"):
            base = base or inputs()
            name = a[1:]
            src = base / name
            dst = tmp / name
            if not dst.exists():
                shutil.copy(src, dst)
            out.append(name)
        else:
            out.append(a)
    for name, text in run.files:
        (tmp / name).write_text(text, encoding="utf-8")
    return out


def _run_case(run: Run, checks: CheckList) -> None:
    settings = dict(run.settings)
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-flirt-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-flirt-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        args = _prepare(run, a)
        _prepare(run, b)
        oracle = run_oracle(run.program, args, cwd=a, settings=settings)
        replica = run_replica(run.program, args, cwd=b, settings=settings, mode=defects.REPRODUCE)
        compare(checks, run.program, oracle, replica, a, b)
        if run.fix and replica.code == 0:
            fix_pass(
                checks,
                run.program,
                lambda t: _prepare(run, t),
                args,
                settings,
                b,
                replica,
            )
    _agree_on_errors(checks, run.program, oracle.code, oracle.stderr)


def fmriprep_calls() -> list[tuple[str, Path]]:
    """``(run, flt_bbr node directory)`` of every BBR call of the fMRIPrep oracle runs."""
    ws = workspace()
    if ws is None:
        return []
    out = []
    for cmd in sorted((ws / FMRIPREP_RUNS).glob("*/work/**/fsl_bbr_wf/flt_bbr/command.txt")):
        run = cmd.relative_to(ws / FMRIPREP_RUNS).parts[0]
        out.append((run, cmd.parent))
    return out


def _fmriprep_case(payload: tuple[str, str], checks: CheckList) -> None:
    run, node = payload
    ws = workspace()
    assert ws is not None
    rdir = ws / FMRIPREP_RUNS / run
    argv = (Path(node) / "command.txt").read_text(encoding="utf-8").split()[1:]
    mounts = {"/out/": str(rdir / "out") + "/", "/work/": str(rdir / "work") + "/"}
    sched = fsldir() / "etc" / "flirtsch" / "bbr.sch"

    def mapped() -> list[str]:
        out = []
        for i, a in enumerate(argv):
            prev = argv[i - 1] if i else ""
            if prev in ("-out", "-omat"):
                out.append(Path(a).name)
                continue
            if prev == "-schedule":
                out.append(str(sched))
                continue
            for k, v in mounts.items():
                if a.startswith(k):
                    a = v + a[len(k) :]
            out.append(a)
        return out

    args = mapped()
    missing = [a for a in args if a.startswith("/") and not Path(a).exists()]
    if missing:
        raise Outcome("skipped", f"input not available: {missing[0]}")
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-flirt-fp-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-flirt-fp-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        oracle = run_oracle("flirt", args, cwd=a, settings={})
        replica = run_replica("flirt", args, cwd=b, settings={}, mode=defects.REPRODUCE)
        compare(checks, "flirt", oracle, replica, a, b)
        theirs = next(Path(node).glob("*.mat"), None)
        ours = next(b.glob("*.mat"), None)
        if theirs is not None and ours is not None:
            same = theirs.read_bytes() == ours.read_bytes()
            dev = _rms80(np.loadtxt(theirs), np.loadtxt(ours))
            checks.add(
                Check(
                    "fMRIPrep's matrix (its container's FSL) identical as text",
                    True,
                    metric="identical",
                    value="yes" if same else "no",
                    threshold="reported",
                    detail=f"RMS deviation {dev:.3g} mm (radius 80 mm, about the origin)",
                )
            )
        if oracle.code == 0 and replica.code == 0:
            fix_pass(checks, "flirt", lambda t: None, args, {}, b, replica)
    _agree_on_errors(checks, "flirt", oracle.code, oracle.stderr)


def _sch(name: str) -> str:
    return str(fsldir() / "etc" / "flirtsch" / name)


#: The real and edge cases: name -> (tier, description, run).
def _real_cases() -> dict[str, tuple[str, str, Run]]:
    E, T, W = "@epi.nii", "@t1.nii", "@wm.nii"
    F = "flirt"
    base = (F, ("-in", E, "-ref", T))
    c: dict[str, tuple[str, str, Run]] = {}

    def add(name: str, tier: str, desc: str, *args: str, **kw: Any) -> None:
        prog = kw.pop("program", F)
        c[name] = (tier, desc, Run(prog, tuple(args), **kw))

    s = "smoke"
    st = "standard"
    add(
        "real/default",
        s,
        "the default (12 dof, corratio) with -out and -omat",
        *base[1],
        "-out",
        "out",
        "-omat",
        "out.mat",
    )
    add(
        "real/default-full",
        st,
        "the full-size T1w as reference",
        "-in",
        E,
        "-ref",
        "@t1full.nii.gz",
        "-omat",
        "out.mat",
    )
    for dof in ("6", "7", "9"):
        add(f"real/dof{dof}", st, f"-dof {dof}", *base[1], "-dof", dof, "-omat", "out.mat")
    for cost in ("mutualinfo", "normmi", "normcorr", "leastsq", "woods", "labeldiff"):
        add(
            f"real/cost-{cost}",
            st,
            f"-cost {cost} (and -searchcost corratio)",
            *base[1],
            "-cost",
            cost,
            "-omat",
            "out.mat",
        )
    add(
        "real/searchcost-normmi",
        st,
        "-searchcost normmi",
        *base[1],
        "-searchcost",
        "normmi",
        "-omat",
        "out.mat",
    )
    add("real/nosearch", st, "-nosearch", *base[1], "-nosearch", "-omat", "out.mat")
    add(
        "real/searchr",
        st,
        "-searchrx/ry/rz with narrow ranges",
        *base[1],
        "-searchrx",
        "-20",
        "30",
        "-searchry",
        "15",
        "-15",
        "-searchrz",
        "-5",
        "5",
        "-omat",
        "out.mat",
    )
    add(
        "real/coarse-fine",
        st,
        "-coarsesearch 30 -finesearch 10",
        *base[1],
        "-coarsesearch",
        "30",
        "-finesearch",
        "10",
        "-dof",
        "6",
        "-omat",
        "out.mat",
    )
    add(
        "real/init",
        st,
        "-init (fMRIPrep's initial matrix)",
        *base[1],
        "-init",
        "@init.mat",
        "-omat",
        "out.mat",
    )
    add("real/usesqform", st, "-usesqform", *base[1], "-usesqform", "-omat", "out.mat")
    add("real/bins64", st, "-bins 64", *base[1], "-bins", "64", "-omat", "out.mat")
    add(
        "real/noresample",
        st,
        "-noresample",
        *base[1],
        "-noresample",
        "-dof",
        "6",
        "-omat",
        "out.mat",
    )
    add("real/noclamp", st, "-noclamp", *base[1], "-noclamp", "-omat", "out.mat")
    add("real/minsampling", st, "-minsampling 4", *base[1], "-minsampling", "4", "-omat", "out.mat")
    add(
        "real/forcescaling",
        st,
        "-forcescaling",
        *base[1],
        "-forcescaling",
        "-dof",
        "6",
        "-omat",
        "out.mat",
    )
    add(
        "real/weights",
        st,
        "-refweight and -inweight",
        *base[1],
        "-refweight",
        "@refweight.nii",
        "-inweight",
        "@inweight.nii",
        "-omat",
        "out.mat",
    )
    for cost in ("normcorr", "leastsq", "normmi", "mutualinfo", "woods"):
        add(
            f"real/weights-{cost}",
            st,
            f"weights with -cost {cost}",
            *base[1],
            "-refweight",
            "@refweight.nii",
            "-cost",
            cost,
            "-dof",
            "6",
            "-omat",
            "out.mat",
        )
    add(
        "real/labeldiff-dseg",
        st,
        "-cost labeldiff on a label image",
        "-in",
        "@dseg_shift.nii",
        "-ref",
        "@dseg.nii",
        "-cost",
        "labeldiff",
        "-searchcost",
        "labeldiff",
        "-dof",
        "6",
        "-interp",
        "nearestneighbour",
        "-out",
        "out",
        "-omat",
        "out.mat",
    )
    add(
        "real/known-motion",
        st,
        "a known rigid motion of the T1w (-dof 6)",
        "-in",
        "@t1_moved.nii",
        "-ref",
        T,
        "-dof",
        "6",
        "-omat",
        "out.mat",
    )
    # Outputs and interpolation
    for interp in ("nearestneighbour", "sinc", "spline", "trilinear"):
        add(
            f"real/interp-{interp}",
            st,
            f"-interp {interp} for -out",
            *base[1],
            "-init",
            "@init.mat",
            "-applyxfm",
            "-interp",
            interp,
            "-out",
            "out",
        )
    add(
        "real/sinc-window",
        st,
        "-sincwidth 5 -sincwindow blackman",
        *base[1],
        "-init",
        "@init.mat",
        "-applyxfm",
        "-interp",
        "sinc",
        "-sincwidth",
        "5",
        "-sincwindow",
        "blackman",
        "-out",
        "out",
    )
    add(
        "real/sinc-rect",
        st,
        "-sincwindow rectangular",
        *base[1],
        "-init",
        "@init.mat",
        "-applyxfm",
        "-interp",
        "sinc",
        "-sincwindow",
        "rectangular",
        "-out",
        "out",
    )
    for dt in ("char", "short", "int", "float", "double"):
        add(
            f"real/datatype-{dt}",
            st,
            f"-datatype {dt}",
            *base[1],
            "-init",
            "@init.mat",
            "-applyxfm",
            "-datatype",
            dt,
            "-out",
            "out",
        )
    add(
        "real/int16-input",
        st,
        "an int16 input (its type kept)",
        "-in",
        "@epi_i16.nii",
        "-ref",
        T,
        "-init",
        "@init.mat",
        "-applyxfm",
        "-out",
        "out",
    )
    add(
        "real/paddingsize",
        st,
        "-paddingsize 2",
        *base[1],
        "-init",
        "@init.mat",
        "-applyxfm",
        "-paddingsize",
        "2",
        "-out",
        "out",
    )
    add(
        "real/setbackground",
        st,
        "-setbackground 100",
        *base[1],
        "-init",
        "@init.mat",
        "-applyxfm",
        "-setbackground",
        "100",
        "-out",
        "out",
    )
    add(
        "real/noresampblur",
        st,
        "-noresampblur",
        *base[1],
        "-init",
        "@init.mat",
        "-applyxfm",
        "-noresampblur",
        "-out",
        "out",
    )
    add(
        "real/applyisoxfm",
        st,
        "-applyisoxfm 3",
        *base[1],
        "-init",
        "@init.mat",
        "-applyisoxfm",
        "3",
        "-out",
        "out",
        "-omat",
        "out.mat",
    )
    add(
        "real/applyxfm-4d",
        st,
        "-applyxfm on four volumes of the raw series, sinc",
        "-in",
        "@bold4.nii",
        "-ref",
        T,
        "-init",
        "@init.mat",
        "-applyxfm",
        "-interp",
        "sinc",
        "-out",
        "out",
    )
    # Onto a grid coarser than the input: the input is blurred first, beyond its edges by the
    # extrapolation FLIRT configured (extraslice with the background, or -setbackground's
    # constant): the raw series' edges are not zero (random testing, 2026-11-01).
    add(
        "real/applyisoxfm-4d-blur",
        st,
        "-applyisoxfm 6 on four volumes of the raw series (blurred first)",
        "-in",
        "@bold4.nii",
        "-ref",
        T,
        "-init",
        "@init.mat",
        "-applyisoxfm",
        "6",
        "-out",
        "out",
    )
    add(
        "real/applyisoxfm-4d-blur-setbackground",
        st,
        "-applyisoxfm 6 with -setbackground (the blur's outside a constant)",
        "-in",
        "@bold4.nii",
        "-ref",
        T,
        "-init",
        "@init.mat",
        "-applyisoxfm",
        "6",
        "-setbackground",
        "40",
        "-interp",
        "spline",
        "-out",
        "out",
    )
    add(
        "real/applyxfm-usesqform",
        st,
        "-applyxfm -usesqform",
        *base[1],
        "-usesqform",
        "-applyxfm",
        "-out",
        "out",
        "-omat",
        "out.mat",
    )
    add(
        "real/out-only",
        st,
        "a registration with -out and no -omat (the matrix printed)",
        *base[1],
        "-dof",
        "6",
        "-out",
        "out",
    )
    add(
        "real/basescale-small",
        st,
        "0.6 mm voxels (the base scale from the voxel size)",
        "-in",
        E,
        "-ref",
        "@t1_small.nii",
        "-dof",
        "6",
        "-omat",
        "out.mat",
    )
    add(
        "real/basescale-big",
        st,
        "14 mm voxels (the base scale from the voxel size)",
        "-in",
        "@epi_big.nii",
        "-ref",
        T,
        "-dof",
        "6",
        "-omat",
        "out.mat",
    )
    add(
        "real/basescale-forced",
        st,
        "-basescale 2",
        *base[1],
        "-basescale",
        "2",
        "-dof",
        "6",
        "-omat",
        "out.mat",
    )
    # Boundary-based registration on the half-size T1w
    bbr = (
        "-in",
        E,
        "-ref",
        T,
        "-init",
        "@init.mat",
        "-schedule",
        _sch("bbr.sch"),
        "-wmseg",
        W,
        "-dof",
        "6",
        "-omat",
        "out.mat",
    )
    add(
        "bbr/default", st, "BBR (FSL's bbr.sch) with -cost bbr", *bbr, "-cost", "bbr", "-out", "out"
    )
    add(
        "bbr/fmriprep-form",
        s,
        "fMRIPrep's form: -searchcost bbr -basescale 1",
        *bbr,
        "-searchcost",
        "bbr",
        "-basescale",
        "1",
    )
    for t in ("global_abs", "local_abs"):
        add(f"bbr/type-{t}", st, f"-bbrtype {t}", *bbr, "-cost", "bbr", "-bbrtype", t)
    add("bbr/slope", st, "-bbrslope -0.25", *bbr, "-cost", "bbr", "-bbrslope", "-0.25")
    add(
        "bbr/coords",
        st,
        "-wmcoords and -wmnorms",
        "-in",
        E,
        "-ref",
        T,
        "-init",
        "@init.mat",
        "-schedule",
        _sch("bbr.sch"),
        "-wmcoords",
        "@wmcoords.txt",
        "-wmnorms",
        "@wmnorms.txt",
        "-cost",
        "bbr",
        "-dof",
        "6",
        "-omat",
        "out.mat",
    )
    add(
        "bbr/fieldmap",
        st,
        "-fieldmap -pedir 2 -echospacing (the BBR resampling for -out)",
        *bbr,
        "-cost",
        "bbr",
        "-fieldmap",
        "@fmap.nii",
        "-pedir",
        "2",
        "-echospacing",
        "0.0007",
        "-out",
        "out",
    )
    add(
        "bbr/fieldmap-mask",
        st,
        "-fieldmapmask -pedir -2",
        *bbr,
        "-cost",
        "bbr",
        "-fieldmap",
        "@fmap.nii",
        "-fieldmapmask",
        "@fmapmask.nii",
        "-pedir",
        "-2",
    )
    add(
        "bbr/weights",
        st,
        "BBR with -refweight",
        *bbr,
        "-cost",
        "bbr",
        "-refweight",
        "@refweight.nii",
    )
    add(
        "bbr/bad-type",
        st,
        "-bbrtype unknown (FSL's exception)",
        *bbr,
        "-cost",
        "bbr",
        "-bbrtype",
        "nope",
    )
    # FSL's shipped schedules
    for sch in (
        "simple3D.sch",
        "xyztrans.sch",
        "ytransonly.sch",
        "ztransonly.sch",
        "sch3Dtrans_3dof",
        "measurecost1.sch",
        "pairreg1.sch",
        "pairreg2.sch",
        "pairreg3.sch",
    ):
        add(
            f"schedule/{sch}",
            st,
            f"FSL's {sch}",
            *base[1],
            "-init",
            "@init.mat",
            "-schedule",
            _sch(sch),
            "-omat",
            "out.mat",
        )
    for sch in ("sch2D_3dof", "sch2D_4dof", "sch2D_5dof", "sch2D_6dof"):
        add(
            f"schedule/{sch}",
            st,
            f"FSL's {sch} on one slice",
            "-in",
            "@slice_in.nii",
            "-ref",
            "@slice_ref.nii",
            "-2D",
            "-schedule",
            _sch(sch),
            "-omat",
            "out.mat",
        )
    add(
        "real/2d",
        st,
        "-2D's default schedule on one slice, with -out",
        "-in",
        "@slice_in.nii",
        "-ref",
        "@slice_ref.nii",
        "-2D",
        "-out",
        "out",
        "-omat",
        "out.mat",
    )
    # Verbose output
    add("verbose/v", st, "-v", *base[1], "-v", "-dof", "6", "-omat", "out.mat")
    for lvl in ("2", "3", "5"):
        add(
            f"verbose/{lvl}",
            st,
            f"-verbose {lvl}",
            *base[1],
            "-verbose",
            lvl,
            "-dof",
            "6",
            "-omat",
            "out.mat",
        )
    add(
        "verbose/printinit",
        st,
        "-printinit",
        *base[1],
        "-printinit",
        "-init",
        "@init.mat",
        "-applyxfm",
        "-omat",
        "out.mat",
    )
    add(
        "verbose/debugsave",
        st,
        "-debugsave -verbose 4 (the search's volumes saved)",
        *base[1],
        "-debugsave",
        "-verbose",
        "4",
        "-dof",
        "6",
        "-omat",
        "out.mat",
    )
    return c


_SCHED_ALL = """# every command of FLIRT's schedule language
setscale 8
setoption smoothing 8
setoption fuzzyfraction 0.5
setoption boundguess 10 1
clear S
clear P
search 6
print S:1
measurecost 6 S:1-2 0 0 0 0 0 0 rel
sort U
copy U UA
clear U
optimise 6 UA:1 0.0 0.0 0.0 0.0 0.0 0.0 rel 2
setrowparams UB 0.01 0.02 0.03 1 2 3 1 1 1 0 0 0
aligncog UB
setrow UC 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1
setrowqsform UC
dualsort S P
printparams U
saveparams U params.txt
save U rows.txt
readparams UD params.txt
read UE rows.txt
if MAXDOF >= 6
 setoption tolerance 0.01 0.01 0.01 0.4 0.4 0.4 0.004 0.004 0.004 0.002 0.002 0.002
if MINSAMPLING < 0
 setoption tolerance 1 1 1
setoption rescaletolerance 2
gridmeasurecost 6 U:1 -0.02 0.02 0.02  0 1 0  0 1 0  -1 1 1  abs
setscale 4
setoption optimisationtype powell
optimise 6 U:1 0.0 0.0 0.0 0.0 0.0 0.0 rel 2
setoption optimisationtype brent
setoption paramsubset 3  0 0 0 1 0 0 0 0 0 0 0 0  0 0 0 0 1 0 0 0 0 0 0 0  0 0 0 0 0 1 0 0 0 0 0 0
optimise 12 U:1 0.0 0.0 0.0 0.0 0.0 0.0 rel 1
setoption nosubset
setoption costfunction normmi
setoption smoothing 0
setoption fuzzyfraction 0
measurecost 12 U:1 0 0 0 0 0 0 rel
setoption costfunction corratio
setoption smoothing 2
setscale 3
measurecost 12 U:1 0 0 0 0 0 0 rel
setoption minsampling 2
setoption unknownopt 1
setoption smoothing
frobnicate U
sort U
print U:1-2
"""

_SCHED_ERRORS = {
    "schedule/args-copy": "copy U\n",
    "schedule/args-optimise": "optimise 6 U\n",
    "schedule/bad-if": "if 1 =< 2\n clear U\n",
    "schedule/exit": "setrow U 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1\nexit\n",
    "schedule/no-matrix": "clear U\n",
    "schedule/bad-costfunction": "setoption costfunction nope\n",
    "schedule/bad-matname": "clear Uq\n",
}


def _schedule_cases() -> dict[str, tuple[str, str, Run]]:
    c = {}
    c["schedule/every-command"] = (
        "standard",
        "a schedule using every command and option",
        Run(
            "flirt",
            ("-in", "@epi.nii", "-ref", "@t1.nii", "-schedule", "all.sch", "-omat", "out.mat"),
            files=(("all.sch", _SCHED_ALL),),
        ),
    )
    for name, text in _SCHED_ERRORS.items():
        c[name] = (
            "standard",
            f"a schedule: {text.splitlines()[0]}",
            Run(
                "flirt",
                ("-in", "@epi.nii", "-ref", "@t1.nii", "-schedule", "s.sch", "-omat", "out.mat"),
                files=(("s.sch", text),),
            ),
        )
    return c


def _edge_cases() -> dict[str, tuple[str, str, Run]]:
    F = "flirt"
    c: dict[str, tuple[str, str, Run]] = {}

    def add(name: str, desc: str, *args: str, program: str = F, **kw: Any) -> None:
        c[name] = (
            "standard" if not kw.pop("smoke", False) else "smoke",
            desc,
            Run(program, tuple(args), **kw),
        )

    add("edge/no-args", "no arguments (the usage, exit 1)", smoke=True)
    add("edge/help", "-help", "-help")
    add("edge/version", "-version", "-version")
    add("edge/no-in", "no -in", "-ref", "@t1.nii")
    add("edge/no-ref", "no -ref", "-in", "@epi.nii")
    add("edge/not-option", "a word that is not an option", "epi.nii")
    add("edge/unknown-1", "an unknown option with one word after it", "-frob", "x")
    add(
        "edge/unknown-2",
        "an unknown option with two words after it",
        "-in",
        "@epi.nii",
        "-frob",
        "x",
        "y",
    )
    add("edge/lacking", "an option lacking its argument", "-in", "@epi.nii", "-ref")
    add("edge/lacking-2", "a two-argument option lacking one", "-in", "@epi.nii", "-searchrx", "10")
    for opt, val in (
        ("-cost", "nope"),
        ("-searchcost", "nope"),
        ("-interp", "cubic"),
        ("-datatype", "long"),
        ("-sincwindow", "gauss"),
        ("-anglerep", "rodrigues"),
        ("-pedir", "0"),
    ):
        add(f"edge/bad{opt}", f"{opt} {val}", "-in", "@epi.nii", "-ref", "@t1.nii", opt, val)
    add(
        "edge/missing-in",
        "an input that does not exist",
        "-in",
        "nothere.nii.gz",
        "-ref",
        "@t1.nii",
        "-omat",
        "o.mat",
    )
    add(
        "edge/missing-ref",
        "a reference that does not exist",
        "-in",
        "@epi.nii",
        "-ref",
        "nothere",
        "-omat",
        "o.mat",
    )
    add(
        "edge/missing-init",
        "an -init file that does not exist",
        "-in",
        "@epi.nii",
        "-ref",
        "@t1.nii",
        "-init",
        "none.mat",
        "-omat",
        "o.mat",
    )
    add(
        "edge/missing-schedule",
        "a -schedule file that does not exist",
        "-in",
        "@epi.nii",
        "-ref",
        "@t1.nii",
        "-schedule",
        "none.sch",
        "-omat",
        "o.mat",
    )
    add(
        "edge/bad-init",
        "an -init file with too few numbers",
        "-in",
        "@epi.nii",
        "-ref",
        "@t1.nii",
        "-init",
        "bad.mat",
        "-omat",
        "o.mat",
        files=(("bad.mat", "1 0 0\n0 1\n"),),
    )
    add(
        "edge/coords-mismatch",
        "-wmcoords and -wmnorms of different lengths",
        "-in",
        "@epi.nii",
        "-ref",
        "@t1.nii",
        "-wmcoords",
        "@wmcoords.txt",
        "-wmnorms",
        "n.txt",
        "-cost",
        "bbr",
        files=(("n.txt", "1 0 0\n"),),
    )
    add(
        "edge/outtype-unset",
        "FSLOUTPUTTYPE unset",
        "-in",
        "@epi.nii",
        "-ref",
        "@t1.nii",
        "-init",
        "@init.mat",
        "-applyxfm",
        "-out",
        "out",
        settings=(("FSLOUTPUTTYPE", None),),
    )
    for ot in ("NIFTI", "NIFTI_PAIR", "NIFTI2_GZ", "ANALYZE"):
        add(
            f"edge/outtype-{ot}",
            f"FSLOUTPUTTYPE={ot}",
            "-in",
            "@epi.nii",
            "-ref",
            "@t1.nii",
            "-init",
            "@init.mat",
            "-applyxfm",
            "-out",
            "out",
            settings=(("FSLOUTPUTTYPE", ot),),
        )
    # convert_xfm
    X = "convert_xfm"
    add("convert_xfm/no-args", "no arguments", program=X, smoke=True)
    add("convert_xfm/help", "-help", "-help", program=X)
    add(
        "convert_xfm/inverse",
        "-omat -inverse",
        "-omat",
        "inv.mat",
        "-inverse",
        "@init.mat",
        program=X,
        smoke=True,
    )
    add(
        "convert_xfm/concat",
        "-omat -concat",
        "-omat",
        "cat.mat",
        "-concat",
        "@init.mat",
        "@init.mat",
        program=X,
    )
    add(
        "convert_xfm/fixscaleskew",
        "-fixscaleskew",
        "-omat",
        "f.mat",
        "-fixscaleskew",
        "s.mat",
        "@init.mat",
        program=X,
        files=(("s.mat", "1.1 0.05 0 0\n0 0.9 0.02 0\n0 0 1.05 0\n0 0 0 1\n"),),
    )
    add(
        "convert_xfm/verbose",
        "-v with -concat and -inverse (the matrices printed)",
        "-v",
        "-concat",
        "@init.mat",
        "-inverse",
        "@init.mat",
        program=X,
    )
    add(
        "convert_xfm/verbose-1",
        "-verbose 1 (the result printed)",
        "-verbose",
        "1",
        "@init.mat",
        program=X,
    )
    add(
        "convert_xfm/copy",
        "-omat only (the matrix written again)",
        "-omat",
        "c.mat",
        "@init.mat",
        program=X,
    )
    add(
        "convert_xfm/missing",
        "an input matrix that does not exist",
        "-omat",
        "o.mat",
        "none.mat",
        program=X,
    )
    add(
        "convert_xfm/short",
        "a 3-row input matrix",
        "-omat",
        "o.mat",
        "s.mat",
        program=X,
        files=(("s.mat", "1 0 0 0\n0 1 0 0\n0 0 1 0\n"),),
    )
    add("convert_xfm/no-input", "no input matrix", "-omat", "o.mat", program=X)
    add("convert_xfm/two-inputs", "two input matrices", "@init.mat", "x.mat", program=X)
    add("convert_xfm/unknown", "an unknown option", "-frob", "x", "@init.mat", program=X)
    add("convert_xfm/lacking", "-omat lacking its argument", "@init.mat", "-omat", program=X)
    add(
        "convert_xfm/singular",
        "-inverse of a singular matrix",
        "-inverse",
        "z.mat",
        program=X,
        files=(("z.mat", "1 0 0 0\n0 0 0 0\n0 0 1 0\n0 0 0 1\n"),),
    )
    add(
        "convert_xfm/missing-concat",
        "-concat with a file that does not exist",
        "-concat",
        "none.mat",
        "@init.mat",
        program=X,
    )
    return c


@functools.cache
def _all_cases() -> dict[str, tuple[str, str, Run]]:
    return {**_real_cases(), **_schedule_cases(), **_edge_cases()}


def cases(tier: str) -> list[Case]:
    from larmorx_validation.parity.harness import in_tier

    out = []
    seen: set[str] = set()
    for run, node in fmriprep_calls():
        # The first call of each standard run in the standard tier; every call in the full one.
        t = "standard" if run in STANDARD_RUNS and run not in seen else "full"
        seen.add(run)
        if in_tier(t, tier):
            out.append(
                Case(
                    f"fmriprep/{run}/{node.parent.parent.parent.parent.name}",
                    "fmriprep",
                    "fMRIPrep's BBR call",
                    ("fmriprep", (run, str(node))),
                )
            )
    for name, (t, desc, _run) in _all_cases().items():
        if in_tier(t, tier):
            out.append(Case(name, name.split("/")[0], desc, ("run", name)))
    return out


def run_case(case: Case, checks: CheckList) -> None:
    kind, payload = case.payload
    if kind == "fmriprep":
        _fmriprep_case(payload, checks)
    elif kind == "direct":
        # a Run given whole (differential random testing's cases)
        _run_case(payload, checks)
    else:
        _run_case(_all_cases()[payload][2], checks)


def suite() -> Suite:
    return Suite(
        name=SUITE,
        title="FLIRT replica (bit-exact, FSL Licence)",
        tool="`larmorx-nc fsl flirt` and `larmorx-nc fsl convert_xfm` (crate larmorx-nc-flirt, "
        f"translated from FSL {fsl_versions.NEWEST}'s source)",
        reference=f"flirt and convert_xfm from FSL {fsl_versions.NEWEST} (fsl-flirt 2111.4, "
        "fsl-newimage 2601.0, fsl-miscmaths 2412.6; the conda build at `~/fsl-6.0.7.23`, as "
        "built: glibc 2.35's maths, OpenBLAS on its Haswell kernels)",
        thresholds=[
            ("Exit status", "the same (an abort: the replica exits 1 after the same text)"),
            (
                "Standard output and error",
                "identical (the run directory and FSL's program path replaced; the replica's "
                "`larmorx-nc:` lines left out)",
            ),
            ("Files written", "the same names"),
            ("Every file", "bit-identical (images decompressed; `descrip` left out)"),
            ("D13 fix pass", "a defect named exactly when the default's outputs differ"),
        ],
        cases=cases,
        run_case=run_case,
        packages=("nibabel", "scipy"),
        notes=(
            "The replica runs as a separate program, the FSL replica package's (`larmorx-nc`), "
            "compared with FSL in `reproduce` mode (D13); the default `fix` pass is checked "
            "against the reproduced run. fMRIPrep's own matrices come from its container's FSL "
            "and are reported only. The derived inputs are made once from ds005040's fMRIPrep "
            "run (`oracles/inputs/fsl-flirt-replica/`)."
        ),
    )


def _gz(path: Path) -> None:
    with open(path, "rb") as f, gzip.GzipFile(str(path) + ".gz", "wb", mtime=0) as g:
        shutil.copyfileobj(f, g)
