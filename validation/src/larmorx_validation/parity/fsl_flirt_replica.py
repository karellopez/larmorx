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
- **convert_xfm**: every option and failure;
- **programs**: FLIRT's other programs (part 2: avscale, the coordinate programs, rmsdiff,
  makerot; part 3: midtrans, pointflirt, applyxfm4D in every data type and interpolation, and the
  scripts pairreg, extracttxt and aff2rigid), every option, message and failure.

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
    EXPECTED,
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
#: Named whenever an input holds NaN or infinite values, which may lie outside the cost's points.
_SILENT = ("FSL-14", "FSL-22", "FSL-75")


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


def oracle_run(program: str, args: list[str], *, cwd: Path, settings: dict[str, str | None]):
    """FSL's ``program`` (``flirt`` or ``convert_xfm``) with ``args`` in ``cwd``, through the
    oracle-output cache: FLIRT is single-threaded and draws no random numbers, so a run is
    served once it ran; every file it writes is in ``cwd`` and every file there counts by its
    content. Returns the cache's ``Completed`` (``cached``: served from it)."""
    from larmorx_validation import oracle_cache

    argv = [str(fsldir() / "bin" / program), *args]
    return oracle_cache.run(
        argv,
        env=_environment(settings),
        cwd=cwd,
        outputs=[cwd],
        policy=oracle_cache.Policy(threads=1, family="fsl"),
        runner=SUITE,
        inputs=[p for p in sorted(cwd.rglob("*")) if p.is_file()],
        timeout=3600,
    )


def run_oracle(
    program: str, args: list[str], *, cwd: Path, settings: dict[str, str | None]
) -> Completed:
    """:func:`oracle_run` as the suite's ``Completed``."""
    p = oracle_run(program, args, cwd=cwd, settings=settings)
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
    import re

    lines = [ln for ln in text.split("\n") if not ln.startswith(REPLICA_PREFIX)]
    t = "\n".join(lines).replace(str(run_dir), "<run>")
    # -debug prints the cost function's address (`Pointer to impair is 0x...`), which the
    # address-space layout decides: the replica prints `(set)`.
    t = re.sub(r"Pointer to impair is 0x[0-9a-f]+", "Pointer to impair is (set)", t)
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
    refused_75 = any(
        ln.startswith(f"{REPLICA_PREFIX} error:") and "FSL-75" in ln
        for ln in replica.stderr.splitlines()
    )
    if oracle.code < 0 and (refused_75 or "FSL-75" in defects.notices_in(replica.stderr)):
        # FSL-75: FSL wrote past a buffer (or read an empty one); it went on with a corrupted
        # heap (its later messages and files are those of undefined behaviour) until the C
        # library or the kernel stopped it. The replica goes on with the values FSL's buffer
        # holds (or refuses the empty buffer of a single voxel), naming the entry.
        # (going on, the replica ends as FLIRT would without the corruption: exit 0, or an
        # error FLIRT meets later, such as an output type it cannot write)
        checks(
            f"{program} is killed after FSL-75's heap corruption; the replica "
            + ("refuses (exit 1)" if refused_75 else "goes on and names FSL-75"),
            replica.code == 1 if refused_75 else replica.code >= 0,
            detail=f"{program} signal {-oracle.code}, replica {replica.code}",
        )
        return 0, 0
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
        if r.stdout != reproduced.stdout:
            # the programs whose results are printed (avscale, rmsdiff, makerot, ...)
            others.append("standard output")
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
    #: FLIRT never finishes on this case (the register entry): FSL runs under a timeout (and,
    #: when it grows its memory for ever, ``aslimit``: an address-space limit that refuses it),
    #: never cached; the replica must refuse in both modes.
    hang: str = ""
    aslimit: str = ""
    #: FSL crashes on this case (the register entry: undefined behaviour, a signal), never
    #: cached; the replica must refuse in both modes, or with ``crash_finishes`` finish in both
    #: naming the entry (FSL-75: the values FSL's buffer holds, or the whole border). FSL's last
    #: messages (the C library's heap diagnostics) are not compared.
    crash: str = ""
    crash_finishes: bool = False


#: How long FSL runs on a case where it never finishes.
HANG_SECONDS = 20


def _constant_image(path: Path, shape: str, vox: str, noform: bool) -> None:
    import nibabel as nib

    dims = tuple(int(x) for x in shape.split("x"))
    sizes = [float(x) for x in vox.split("x")]
    aff = np.diag([*sizes, 1.0])
    img = nib.Nifti1Image(np.ones(dims, np.float32), aff)
    img.header.set_xyzt_units("mm")
    if noform:
        img.header.set_sform(None, code=0)
        img.header.set_qform(None, code=0)
    else:
        img.set_qform(aff, 1)
        img.set_sform(aff, 1)
    nib.save(img, str(path))


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
        elif a.startswith("@fsldata:"):
            # one of FSL's standard images, read where FSL keeps it
            out.append(str(fsldir() / "data" / "standard" / a.removeprefix("@fsldata:")))
        elif a.startswith(("@const:", "@noform:")):
            # a constant image (value 1, float32): @const:<nx>x<ny>x<nz>:<dx>x<dy>x<dz>:<name>,
            # with its sform and qform the voxel sizes (@noform: neither set)
            kind, shape, vox, name = a[1:].split(":")
            dst = tmp / name
            if not dst.exists():
                _constant_image(dst, shape, vox, kind == "noform")
            out.append(name)
        elif a.startswith("@int8:"):
            # an input stored as int8 (its values divided by 16, rounded, clipped):
            # @int8:<input>:<name>
            import nibabel as nib

            _, src_name, name = a.split(":")
            dst = tmp / name
            if not dst.exists():
                base = base or inputs()
                img = nib.load(str(base / src_name))
                arr = np.clip(np.round(np.asanyarray(img.dataobj) / 16.0), -128, 127)
                h = img.header.copy()
                h.set_data_dtype(np.int8)
                nib.save(nib.Nifti1Image(arr.astype(np.int8), img.affine, h), str(dst))
            out.append(name)
        elif a.startswith("@double:"):
            # an input stored as float64, its values divided by 7 (not exact in single
            # precision): @double:<input>:<name>
            import nibabel as nib

            _, src_name, name = a.split(":")
            dst = tmp / name
            if not dst.exists():
                base = base or inputs()
                img = nib.load(str(base / src_name))
                arr = np.asanyarray(img.dataobj).astype(np.float64) / 7.0
                h = img.header.copy()
                h.set_data_dtype(np.float64)
                nib.save(nib.Nifti1Image(arr, img.affine, h), str(dst))
            out.append(name)
        elif a.startswith("@nanedge:"):
            # an input with NaN voxels in its 2-voxel border, at (0, 4, 5) and (1, 6, 7), and an
            # infinite one at (4, 4, 4): @nanedge:<input>:<name>
            import nibabel as nib

            _, src_name, name = a.split(":")
            dst = tmp / name
            if not dst.exists():
                base = base or inputs()
                img = nib.load(str(base / src_name))
                arr = np.asanyarray(img.dataobj).astype(np.float32).copy()
                arr[0, 4, 5] = np.nan
                arr[1, 6, 7] = np.nan
                arr[4, 4, 4] = np.inf
                nib.save(nib.Nifti1Image(arr, img.affine, img.header), str(dst))
            out.append(name)
        elif a.startswith("@nan:"):
            # an input with one NaN voxel at (3, 4, 5): @nan:<input>:<name>
            import nibabel as nib

            _, src_name, name = a.split(":")
            dst = tmp / name
            if not dst.exists():
                base = base or inputs()
                img = nib.load(str(base / src_name))
                arr = np.asanyarray(img.dataobj).astype(np.float32).copy()
                arr[3, 4, 5] = np.nan
                nib.save(nib.Nifti1Image(arr, img.affine, img.header), str(dst))
            out.append(name)
        elif "=@" in a and a.startswith("-"):
            # a long option's value, an input (--cov=@epi.nii)
            base = base or inputs()
            key, name = a.split("=@", 1)
            if not (tmp / name).exists():
                shutil.copy(base / name, tmp / name)
            out.append(f"{key}={name}")
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
        (tmp / name).parent.mkdir(parents=True, exist_ok=True)
        (tmp / name).write_text(text, encoding="utf-8")
    return out


def _hang_case(run: Run, checks: CheckList) -> None:
    """A case FLIRT never finishes (``run.hang``): FSL under a timeout (and an address-space
    limit where its memory grows for ever), never cached; the replica refuses in both modes."""
    from larmorx_validation import address_space, oracle_cache

    settings = dict(run.settings)
    with tempfile.TemporaryDirectory(prefix="lx-nc-flirt-") as a_:
        a = Path(a_)
        args = _prepare(run, a)
        old = os.environ.get(address_space.ENV)
        if run.aslimit:
            os.environ[address_space.ENV] = run.aslimit
        try:
            p = oracle_cache.run(
                [str(fsldir() / "bin" / run.program), *args],
                env=_environment(settings),
                cwd=a,
                outputs=[a],
                policy=oracle_cache.Policy(
                    threads=1, family="fsl", never=f"FLIRT never finishes ({run.hang})"
                ),
                runner=SUITE,
                inputs=[q for q in sorted(a.rglob("*")) if q.is_file()],
                timeout=HANG_SECONDS,
            )
        finally:
            if run.aslimit:
                if old is None:
                    os.environ.pop(address_space.ENV, None)
                else:
                    os.environ[address_space.ENV] = old
        if run.aslimit:
            checks(
                f"FSL's memory grows until the address-space limit ({run.aslimit}) refuses it",
                p.returncode == 0 and "bad_alloc" in p.stderr,
                detail=f"exit {p.returncode}: {' '.join(p.stderr.split())[-160:]}",
            )
        else:
            checks(
                "FSL does not finish",
                p.timed_out,
                detail=f"{'still running' if p.timed_out else f'exit {p.returncode}'} after "
                f"{HANG_SECONDS} s",
            )
    for mode in (defects.REPRODUCE, defects.FIX):
        with tempfile.TemporaryDirectory(prefix="lx-nc-flirt-") as b_:
            b = Path(b_)
            r = run_replica(run.program, _prepare(run, b), cwd=b, settings=settings, mode=mode)
            refusal = next(
                (ln for ln in r.stderr.splitlines() if ln.startswith(f"{REPLICA_PREFIX} error:")),
                "",
            )
            checks(
                f"the replica refuses ({mode})",
                r.code == 1 and f"({run.hang})" in refusal,
                detail=f"exit {r.code}: {refusal[:160]}",
            )
    if all(c.passed for c in checks.checks):
        raise Outcome(
            EXPECTED,
            f"FLIRT never finishes ({run.hang}: undefined behaviour); the replica refuses in "
            "both modes",
        )


def _crash_case(run: Run, checks: CheckList) -> None:
    """A case FSL crashes on (``run.crash``): FSL killed by a signal, the replica's refusal in
    both modes naming the entry (or, ``run.crash_finishes``, its result in both modes with the
    entry's notice)."""
    from larmorx_validation import oracle_cache

    settings = dict(run.settings)
    with tempfile.TemporaryDirectory(prefix="lx-nc-flirt-") as a_:
        a = Path(a_)
        args = _prepare(run, a)
        p = oracle_cache.run(
            [str(fsldir() / "bin" / run.program), *args],
            env=_environment(settings),
            cwd=a,
            outputs=[a],
            policy=oracle_cache.Policy(threads=1, family="fsl", never=f"FSL crashes ({run.crash})"),
            runner=SUITE,
            inputs=[q for q in sorted(a.rglob("*")) if q.is_file()],
            timeout=600,
        )
        checks(
            "FSL is killed by a signal",
            p.returncode is not None and p.returncode < 0,
            detail=f"exit {p.returncode}: {' '.join(p.stderr.split())[-160:]}",
        )
    for mode in (defects.REPRODUCE, defects.FIX):
        with tempfile.TemporaryDirectory(prefix="lx-nc-flirt-") as b_:
            b = Path(b_)
            r = run_replica(run.program, _prepare(run, b), cwd=b, settings=settings, mode=mode)
            if run.crash_finishes:
                named = defects.notices_in(r.stderr)
                checks(
                    f"the replica finishes and names {run.crash} ({mode})",
                    r.code == 0 and run.crash in named and bool(list(b.glob("out*"))),
                    detail=f"exit {r.code}: {', '.join(named) or 'nothing named'}",
                )
                continue
            refusal = next(
                (ln for ln in r.stderr.splitlines() if ln.startswith(f"{REPLICA_PREFIX} error:")),
                "",
            )
            checks(
                f"the replica refuses ({mode})",
                r.code == 1 and run.crash in refusal and not list(b.glob("out*")),
                detail=f"exit {r.code}: {refusal[:160]}",
            )
    if all(c.passed for c in checks.checks):
        what = (
            f"the replica finishes in both modes and names {run.crash}"
            if run.crash_finishes
            else "the replica refuses in both modes"
        )
        raise Outcome(EXPECTED, f"FSL crashes ({run.crash}: undefined behaviour); {what}")


def _run_case(run: Run, checks: CheckList) -> None:
    if run.hang:
        _hang_case(run, checks)
        return
    if run.crash:
        _crash_case(run, checks)
        return
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


def fmriprep_args(run: str, node: str | Path) -> list[str]:
    """fMRIPrep's BBR call of ``run`` (its ``flt_bbr`` node directory ``node``) as the suite
    runs it: the container's paths mapped onto the run's ``out`` and ``work`` directories, the
    outputs written into the working directory, FSL's own ``bbr.sch``."""
    ws = workspace()
    assert ws is not None
    rdir = ws / FMRIPREP_RUNS / run
    argv = (Path(node) / "command.txt").read_text(encoding="utf-8").split()[1:]
    mounts = {"/out/": str(rdir / "out") + "/", "/work/": str(rdir / "work") + "/"}
    sched = fsldir() / "etc" / "flirtsch" / "bbr.sch"
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


def _fmriprep_case(payload: tuple[str, str], checks: CheckList) -> None:
    run, node = payload
    args = fmriprep_args(run, node)
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
        "real/anglerep-quaternion",
        st,
        "-anglerep quaternion: the rotations as a quaternion's last three components (12 dof)",
        *base[1],
        "-anglerep",
        "quaternion",
        "-omat",
        "out.mat",
    )
    add(
        "real/anglerep-quaternion-dof6",
        st,
        "-anglerep quaternion at 6 dof, -out, verbose (the parameters printed)",
        *base[1],
        "-anglerep",
        "quaternion",
        "-dof",
        "6",
        "-out",
        "out",
        "-omat",
        "out.mat",
        "-v",
    )
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
    # The initial matrix is scaled by 1/basescale, a single-precision quotient (random testing,
    # 2026-11-01: the replica had divided in double; an identity -init hides it)
    add(
        "real/basescale-small-init",
        st,
        "0.6 mm voxels with -init (the initial matrix in the base scale's millimetres)",
        "-in",
        E,
        "-ref",
        "@t1_small.nii",
        "-init",
        "@init.mat",
        "-dof",
        "6",
        "-nosearch",
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
    add("bbr/debug", st, "-debug with -cost bbr (BBR's debug volume `debug`, part 3)", *bbr,
        "-cost", "bbr", "-debug")  # fmt: skip
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
    # -debug (part 3): the pyramid's volumes saved after the preparation, its final cost call
    add("verbose/debug", st, "-debug: the pyramid's volumes and weights saved (no weights: the "
        "one-voxel weight)", *base[1], "-debug", "-dof", "6", "-omat", "out.mat")  # fmt: skip
    add("verbose/debug-weights", st, "-debug with -refweight and -inweight (the weights' "
        "levels saved)", *base[1], "-refweight", "@refweight.nii", "-inweight", "@inweight.nii",
        "-debug", "-dof", "6", "-omat", "out.mat")  # fmt: skip
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
    # alignpaxes (the principal axes: LAPACK dsyevd) and a general parameter subset (the SVD
    # pseudo-inverse: LAPACK dgesdd), part 3: one case per path.
    pax = "setscale 8\nsetrow UA 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1\nalignpaxes UA\nprint U\n"
    for name, desc, ins in (
        ("schedule/alignpaxes", "alignpaxes on real images", ("@epi.nii", "@t1.nii")),
        (
            "schedule/alignpaxes-constant",
            "alignpaxes on constant images (zero moments, total 0)",
            ("@const:12x10x8:2x2x2:c.nii", "@const:12x10x8:2x2x2:c.nii"),
        ),
        (
            "schedule/alignpaxes-nan",
            "alignpaxes on an input with a NaN voxel (eig_sym fails: the exception)",
            ("@nan:epi.nii:epi_nan.nii", "@t1.nii"),
        ),
    ):
        c[name] = (
            "standard",
            desc,
            Run(
                "flirt",
                ("-in", ins[0], "-ref", ins[1], "-schedule", "s.sch", "-omat", "out.mat"),
                files=(("s.sch", pax),),
            ),
        )
    c["schedule/alignpaxes-2d"] = (
        "standard",
        "alignpaxes in 2D mode (one slice: a zero third moment)",
        Run(
            "flirt",
            (
                "-in",
                "@slice_in.nii",
                "-ref",
                "@slice_ref.nii",
                "-2D",
                "-schedule",
                "s.sch",
                "-omat",
                "out.mat",
            ),
            files=(("s.sch", pax),),
        ),
    )
    c["schedule/alignpaxes-args"] = (
        "standard",
        "alignpaxes without its matrix",
        Run(
            "flirt",
            ("-in", "@epi.nii", "-ref", "@t1.nii", "-schedule", "s.sch", "-omat", "out.mat"),
            files=(("s.sch", "alignpaxes\n"),),
        ),
    )
    sub = (
        "setscale 8\nsetrow UA 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1\n"
        "setoption paramsubset 2  0 0 1 0 0 0 0 0 0 0 0 0  0 0 0 1 1 0 0 0 0 0 0 0\n"
        "optimise 12 UA:1 0.0 0.0 0.0 0.0 0.0 0.0 rel 1\nprint U\n"
    )
    c["schedule/paramsubset-general"] = (
        "standard",
        "a parameter subset whose column has two ones in different rows of another (rz; tx+ty)",
        Run(
            "flirt",
            ("-in", "@epi.nii", "-ref", "@t1.nii", "-schedule", "s.sch", "-omat", "out.mat"),
            files=(("s.sch", sub),),
        ),
    )
    sub2 = (
        "setscale 8\nsetrow UA 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1\n"
        "setoption paramsubset 2  0 0 0.5 0.25 0 0 0 0 0 0 0 0  0 0 1 -1 2 0 0 0 0 0 0 0\n"
        "optimise 12 UA:1 0.0 0.0 0.0 0.0 0.0 0.0 rel 1\nprint U\n"
    )
    c["schedule/paramsubset-weighted"] = (
        "standard",
        "a parameter subset with weights other than 0 and 1 sharing rows (the SVD's QR path)",
        Run(
            "flirt",
            ("-in", "@epi.nii", "-ref", "@t1.nii", "-schedule", "s.sch", "-omat", "out.mat"),
            files=(("s.sch", sub2),),
        ),
    )
    # part 3's trial (part3-trial-57755): a subset whose line search reaches a zero scale on
    # one-voxel images: the cost's inverse of a singular matrix throws, FLIRT's main catches it
    sub3 = (
        "setscale 8\nsetrow UA 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1\n"
        "setoption paramsubset 2  0 0 0 0 0.5 2 0 1 0.5 0 2 0  1 0 0 0 0 0.5 2 0 -1 0 0 0.5\n"
        "optimise 12 UA:1 0.0 0.0 0.0 0.0 0.0 0.0 rel 1\n"
    )
    c["schedule/paramsubset-singular"] = (
        "standard",
        "a parameter subset reaching a singular matrix in a cost evaluation (the exception "
        "caught by FLIRT's main: FSL-68)",
        Run(
            "flirt",
            (
                "-in",
                "@const:1x1x1:1x1x1:one.nii",
                "-ref",
                "@const:1x1x1:1x1x1:one.nii",
                "-schedule",
                "s.sch",
                "-omat",
                "out.mat",
            ),
            files=(("s.sch", sub3),),
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
        "edge/bbrstep-0",
        "setoption bbrstep 0 with BBR: FLIRT's loop over the boundary points never ends (FSL-71)",
        "-in",
        "@epi.nii",
        "-ref",
        "@t1.nii",
        "-cost",
        "bbr",
        "-wmseg",
        "@wm.nii",
        "-schedule",
        "s.sch",
        "-omat",
        "out.mat",
        files=(
            (
                "s.sch",
                "setscale 8\nsetoption bbrstep 0\nclear UA\n"
                "setrow UA 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1\n"
                "measurecost 6 UA 0 0 0 0 0 0 abs\n",
            ),
        ),
        hang="FSL-71",
    )
    add(
        "edge/read-not-numbers",
        "a schedule's read of a file holding a word that is not a number: FLIRT reads for ever, "
        "its memory growing (FSL-72)",
        "-in",
        "@epi.nii",
        "-ref",
        "@t1.nii",
        "-schedule",
        "s.sch",
        "-omat",
        "out.mat",
        files=(("s.sch", "read UA bad.txt\n"), ("bad.txt", "1 2 abc\n")),
        hang="FSL-72",
        aslimit="8G",
    )
    # FSL-75: newimage's background value of an image 3 voxels wide (heap corruption), or of
    # a single voxel (an empty buffer read)
    add(
        "edge/background-overflow",
        "a 1-voxel input and an 8 x 1 x 1 reference: FLIRT's pyramid computes the background of a "
        "3-voxel-wide level, writing past its buffer (FSL-75)",
        "-in",
        "@const:1x1x1:2.5x2.5x2.5:in.nii",
        "-ref",
        "@const:8x1x1:1x1.5x0.5:ref.nii",
        "-omat",
        "out.mat",
        crash="FSL-75",
        crash_finishes=True,
    )
    add(
        "edge/background-empty",
        "-applyxfm of a 1-voxel input: its background value reads an empty buffer (FSL-75)",
        "-in",
        "@const:1x1x1:2x2x2:in.nii",
        "-ref",
        "@const:6x6x6:2x2x2:ref.nii",
        "-applyxfm",
        "-init",
        "id.mat",
        "-out",
        "out",
        files=(("id.mat", "1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n"),),
        crash="FSL-75",
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


_AFF = "1.05 0.02 -0.03 2\n-0.01 0.97 0.04 -3\n0.02 -0.05 1.1 1.5\n0 0 0 1\n"
_SING = "1 0 0 0\n0 0 0 0\n0 0 1 0\n0 0 0 1\n"
_FLIP = "-1 0 0 10\n0 1 0 0\n0 0 1 0\n0 0 0 1\n"
_COORDS = "10 20 30\n0 0 0\n-5.5 3.25 100\n1e3 2 3\n"


#: midtrans' inputs (part 3): three transforms of images to one template.
_MT1 = "1.02 0.05 -0.01 3.5\n-0.04 0.98 0.06 -2\n0.02 -0.03 1.05 1\n0 0 0 1\n"
_MT2 = "0.97 -0.08 0.02 -1.5\n0.07 1.01 -0.02 4\n-0.03 0.02 0.99 -3\n0 0 0 1\n"
_MT3 = "1.1 0.01 0.03 0.5\n0.02 0.92 -0.05 1\n-0.01 0.04 1.03 2.5\n0 0 0 1\n"
_ID = "1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n"
#: pointflirt's inputs: 8 points (one per line) and their images under an affine map.
_PTS_IN = "10 20 30\n-12 5 40\n33 -7 12\n0 0 0\n25 25 -10\n-8 -30 22\n14 9 -3\n-20 18 6\n"
_PTS_REF = (
    "13.31 18.62 35.25\n-9.33 7.41 44.05\n38.47 -8.32 13.9\n2.5 -1 1.5\n"
    "29.29 23.95 -9.68\n-5.24 -31.87 26.59\n17.35 7.55 -1.71\n-17.75 18.24 7.86\n"
)
#: 20 points: x·xᵀ by BLAS dsyrk (more than 48 elements).
_PTS20 = "".join(
    f"{(i * 7) % 23 - 11} {(i * 5) % 17 - 8} {(i * 11) % 29 - 14}\n" for i in range(20)
)
_PTS20_REF = "".join(
    f"{(i * 7) % 23 - 11 + 0.1 * i} {(i * 5) % 17 - 8 - 0.05 * i} {(i * 11) % 29 - 13.5}\n"
    for i in range(20)
)


def _program_cases() -> dict[str, tuple[str, str, Run]]:
    """avscale, img2imgcoord, std2imgcoord, img2stdcoord, rmsdiff, makerot, midtrans and
    pointflirt: one case per path."""
    c: dict[str, tuple[str, str, Run]] = {}
    st = "standard"
    E, T, W, M = "@epi.nii", "@t1.nii", "@wm.nii", "@init.mat"
    STD = "@fsldata:MNI152_T1_2mm_brain_mask.nii.gz"

    def add(name: str, desc: str, program: str, *args: str, tier: str = st, **kw: Any) -> None:
        c[f"programs/{name}"] = (tier, desc, Run(program, tuple(args), **kw))

    X = "applyxfm4D"
    rot = (
        "0.996 -0.087 0 3\n0.087 0.996 0 -2\n0 0 1 1.5\n0 0 0 1\n",
        "0.99 0.02 -0.03 1\n-0.02 1.01 0.01 0.5\n0.03 -0.01 0.98 -1\n0 0 0 1\n",
    )
    mats = tuple((f"mats/MAT_0{k:04d}", rot[k % 2]) for k in range(4))
    mats4 = tuple((f"m4/MAT_{k:04d}", rot[k % 2]) for k in range(4))
    matsu = tuple((f"mu/vol{k:04d}", rot[k % 2]) for k in range(4))
    one = (("one.mat", rot[0]),)
    B4 = "@bold4.nii"
    add("applyxfm4D-usage", "applyxfm4D without arguments (its usage, exit 255)", X)
    add("applyxfm4D-default", "a 4D int16 series, a matrix per volume (MAT_0000n), sinc", X,
        B4, E, "out.nii.gz", "mats", files=mats, tier="smoke")  # fmt: skip
    add("applyxfm4D-single-trilinear", "-singlematrix with trilinear interpolation, verbose", X,
        B4, E, "out.nii.gz", "one.mat", "-singlematrix", "-interp", "trilinear", "-v",
        files=one)  # fmt: skip
    add("applyxfm4D-fourdigit-nn", "--fourdigit (MAT_000n) with nearest neighbour (nn)", X,
        B4, E, "out.nii.gz", "m4", "--fourdigit", "--interp", "nn", files=mats4)  # fmt: skip
    add("applyxfm4D-userprefix-spline", "-userprefix with spline on an integer series (FSL-79)",
        X, B4, E, "out.nii.gz", "mu", "-userprefix", "vol", "-interp", "spline",
        files=matsu)  # fmt: skip
    add("applyxfm4D-float-sinc", "a float input (one volume), -singlematrix, sinc", X, E, T,
        "out.nii.gz", "one.mat", "-singlematrix", files=one)  # fmt: skip
    add("applyxfm4D-3d", "-3D (the rectangular sinc of width 9) on a float volume", X, E, E,
        "out.nii.gz", "one.mat", "-3D", files=one)  # fmt: skip
    add("applyxfm4D-3d-spline-float", "-3D with spline on a float volume", X, E, E,
        "out.nii.gz", "one.mat", "-3d", "-interp", "spline", files=one)  # fmt: skip
    add("applyxfm4D-int8", "an int8 input (FSL-09: written as uint8), trilinear", X,
        "@int8:epi.nii:epi_i8.nii", E, "out.nii.gz", "one.mat", "-singlematrix",
        "-interp", "trilinear", files=one)  # fmt: skip
    add("applyxfm4D-double-sinc", "a double series (sinc sums in double precision; the "
        "background value kept in double precision where the mask sets it)", X,
        "@double:bold4.nii:bold4_f64.nii", E, "out.nii.gz", "mats", files=mats)  # fmt: skip
    add("applyxfm4D-double-3d-spline", "-3D spline on a double volume (its coefficients in "
        "double precision)", X, "@double:epi.nii:epi_f64.nii", E, "out.nii.gz", "one.mat",
        "-3D", "-interp", "spline", files=one)  # fmt: skip
    add("applyxfm4D-double-nn", "a double volume, nearest neighbour (values through float)", X,
        "@double:epi.nii:epi_f64.nii", T, "out.nii.gz", "one.mat", "-singlematrix", "-interp",
        "nn", files=one)  # fmt: skip
    add("applyxfm4D-3d-spline-int", "-3D spline on an int16 volume (FSL-79: coefficients and "
        "values in the type)", X, B4, E, "out.nii.gz", "one.mat", "-3D", "-interp", "spline",
        files=one)  # fmt: skip
    add("applyxfm4D-nan-border", "NaN voxels in the input's border and an infinite one: the "
        "background value's NaN order (FSL-22), spline coefficients through them", X,
        "@nanedge:epi.nii:epi_nanedge.nii", E, "out.nii.gz", "one.mat", "-singlematrix",
        "-interp", "spline", files=one)  # fmt: skip
    add("applyxfm4D-nan-border-3d", "the same with -3D and sinc", X,
        "@nanedge:epi.nii:epi_nanedge.nii", E, "out.nii.gz", "one.mat", "-3D", files=one)  # fmt: skip
    add("applyxfm4D-bad-interp", "an unknown interpolation (its message, exit 1)", X, B4, E,
        "out.nii.gz", "one.mat", "-interp", "cubic", files=one)  # fmt: skip
    add("applyxfm4D-userprefix-last", "-userprefix as the last word (std::string of null)", X,
        B4, E, "out.nii.gz", "mu", "-userprefix", files=matsu)  # fmt: skip
    add("applyxfm4D-missing-matrix", "a directory without the matrices (FSL aborts)", X, B4,
        E, "out.nii.gz", "none")  # fmt: skip
    add("applyxfm4D-missing-input", "an input that does not exist (FSL aborts)", X,
        "none.nii.gz", E, "out.nii.gz", "one.mat", files=one)  # fmt: skip
    # FLIRT's scripts (part 3): pairreg, extracttxt, aff2rigid
    PY = (("PATH", f"{fsldir()}/bin:/usr/bin:/bin"),)  # aff2rigid runs `env fslpython`
    add("aff2rigid-usage", "aff2rigid without arguments (its usage, exit 1)", "aff2rigid",
        settings=PY)  # fmt: skip
    add("aff2rigid-default", "aff2rigid of an affine standard-space matrix (numpy's arithmetic)",
        "aff2rigid", "aff.mat", "rigid.mat", files=(("aff.mat", _AFF),), settings=PY)  # fmt: skip
    add("aff2rigid-fmriprep", "aff2rigid of fMRIPrep's initial matrix", "aff2rigid", M,
        "rigid.mat", settings=PY)  # fmt: skip
    txt = "".join(f"line {k} value {k * 3}\n" for k in range(1, 13)) + "the end\n"
    ET = "extracttxt"
    xt = (("t.txt", txt),)
    add("extracttxt-usage", "extracttxt with one argument (its usage, exit 255)", ET, "word")
    add("extracttxt-rest", "the lines from the first match to the end", ET, "value", "t.txt",
        files=xt)  # fmt: skip
    add("extracttxt-count", "three lines from the first match", ET, "e.4", "t.txt", "3",
        files=xt)  # fmt: skip
    add("extracttxt-regex-start", "a regular expression, two lines, starting 2 lines after", ET,
        "^line.[5-6]", "t.txt", "2", "2", files=xt)  # fmt: skip
    add("extracttxt-before", "a negative relative start (lines before the match)", ET, "end$",
        "t.txt", "4", "-3", files=xt)  # fmt: skip
    add("extracttxt-zero-count", "zero trailing lines (sed's range ends before it starts)", ET,
        "line.7", "t.txt", "0", files=xt)  # fmt: skip
    add("extracttxt-line0", "a relative start reaching line 0 (sed's error, exit 1)", ET,
        "^line.1.v", "t.txt", "2", "-1", files=xt)  # fmt: skip
    add("extracttxt-dc-negative", "a start before line 1: dc reads its own negative result as "
        "a subtraction (stack empty), sed the range as an option", ET, "e.2", "t.txt", "3",
        "-4", files=xt)  # fmt: skip
    add("extracttxt-sed-option", "a start on line 0 with no trailing line: the range -1,-1 "
        "(sed's unknown option)", ET, "e.4", "t.txt", "0", "-4", files=xt)  # fmt: skip
    add("extracttxt-signed", "signed arguments (+2, -0): dc reads each sign as a command",
        ET, "e.4", "t.txt", "-0", "+2", files=xt)  # fmt: skip
    add("extracttxt-nomatch", "a word that never matches (nothing printed, exit 0)", ET,
        "nothing", "t.txt", files=xt)  # fmt: skip
    add("extracttxt-missing", "a file that does not exist (grep's message, exit 0)", ET, "x",
        "none.txt")  # fmt: skip
    PR = "pairreg"
    add("pairreg-usage", "pairreg with four arguments (its usage, exit 1)", PR, "a", "b", "c",
        "d")  # fmt: skip
    add("pairreg-default", "pairreg of the T1w and its moved copy (brains and skulls the same)",
        PR, T, "@t1_moved.nii", T, "@t1_moved.nii", "pair.mat", "-dof", "6")  # fmt: skip
    K = "midtrans"
    mt = (("a.mat", _MT1), ("b.mat", _MT2), ("c.mat", _MT3), ("id.mat", _ID))
    add("midtrans-usage", "midtrans without transforms (its message and usage, exit 1)", K)
    add("midtrans-help", "midtrans -h (its usage, exit 1)", K, "-h")
    add("midtrans-one", "one transform only (at least 2 needed, exit 1)", K, "a.mat", files=mt)
    add("midtrans-default", "three transforms, printed (the MNI origin assumed)", K, "a.mat",
        "b.mat", "c.mat", files=mt, tier="smoke")  # fmt: skip
    add("midtrans-out-verbose", "-o with -v (every stage printed, the matrix written)", K, "-v",
        "-o", "mid.mat", "a.mat", "b.mat", "id.mat", files=mt)  # fmt: skip
    add("midtrans-template-separate", "--template (its centre of gravity) and --separate", K,
        "--template", E, "--separate", "img2mid", "-o", "mid.mat", "a.mat", "b.mat", "c.mat",
        files=mt)  # fmt: skip
    add("midtrans-debug", "--debug (the origin at 0 and the debug matrices)", K, "--debug",
        "a.mat", "c.mat", files=mt)  # fmt: skip
    add("midtrans-singular", "a transform with a determinant below 1e-6 (its message, exit 1)",
        K, "a.mat", "sing.mat", files=(*mt, ("sing.mat", _SING)))  # fmt: skip
    add("midtrans-token", "an option after the transforms (read as a file: an empty matrix, "
        "the size error caught)", K, "a.mat", "b.mat", "-v", files=mt)  # fmt: skip
    add("midtrans-not-square", "a 3 x 4 matrix (the determinant's error, caught)", K, "a.mat",
        "r.mat", files=(*mt, ("r.mat", "1 0 0 1\n0 1 0 2\n0 0 1 3\n")))  # fmt: skip
    add("midtrans-3x3", "a 3 x 3 matrix (the product's size error, caught)", K, "a.mat",
        "r.mat", files=(*mt, ("r.mat", "1 0 0\n0 1 0\n0 0 1\n")))  # fmt: skip
    P = "pointflirt"
    pf = (("in.txt", _PTS_IN), ("ref.txt", _PTS_REF))
    add("pointflirt-usage", "pointflirt without arguments (the compulsory options, exit 1)", P)
    add("pointflirt-default", "eight points, the matrix printed", P, "-i", "in.txt", "-r",
        "ref.txt", files=pf, tier="smoke")  # fmt: skip
    add("pointflirt-out-verbose", "-v and -o (the coordinates printed, the matrix written)", P,
        "-v", "-i", "in.txt", "-r", "ref.txt", "-o", "aff.mat", files=pf)  # fmt: skip
    add("pointflirt-vox", "--vox with --invol and --refvol (voxel coordinates scaled)", P, "-i",
        "in.txt", "-r", "ref.txt", "--vox", "--invol=@epi.nii", "--refvol=@t1.nii", files=pf,
        )  # fmt: skip
    add("pointflirt-vox-missing", "--vox without the volumes (its error, exit 2)", P, "-i",
        "in.txt", "-r", "ref.txt", "--vox", files=pf)  # fmt: skip
    add("pointflirt-nullspace", "four points (a nullspace: the warning, a non-unique solution)",
        P, "-i", "in4.txt", "-r", "ref4.txt",
        files=(("in4.txt", "".join(_PTS_IN.splitlines(True)[:4])),
               ("ref4.txt", "".join(_PTS_REF.splitlines(True)[:4]))))  # fmt: skip
    add("pointflirt-count", "files with different numbers of points (its error, exit 255)", P,
        "-i", "in.txt", "-r", "ref4.txt",
        files=(*pf, ("ref4.txt", "".join(_PTS_REF.splitlines(True)[:4]))))  # fmt: skip
    add("pointflirt-many", "twenty points (x·xᵀ by BLAS dsyrk)", P, "-i", "in20.txt", "-r",
        "ref20.txt", files=(("in20.txt", _PTS20), ("ref20.txt", _PTS20_REF)))  # fmt: skip
    add("pointflirt-missing", "a coordinate file that does not exist (FSL aborts)", P, "-i",
        "none.txt", "-r", "ref.txt", files=pf)  # fmt: skip
    three = "".join(_PTS_IN.splitlines(True)[:3])
    three_ref = "".join(_PTS_REF.splitlines(True)[:3])
    add("pointflirt-three", "three points: a square file is read with points as columns", P,
        "-i", "in3.txt", "-r", "ref3.txt", files=(("in3.txt", three), ("ref3.txt", three_ref)))  # fmt: skip
    add("pointflirt-two", "two points (two rows of three: FSL reads past its rows, aborts)", P,
        "-i", "in2.txt", "-r", "ref2.txt",
        files=(("in2.txt", "".join(_PTS_IN.splitlines(True)[:2])),
               ("ref2.txt", "".join(_PTS_REF.splitlines(True)[:2]))))  # fmt: skip
    add("pointflirt-four-columns", "points of four coordinates (FSL's block sizes clash, aborts)",
        P, "-i", "in4c.txt", "-r", "ref4c.txt",
        files=(("in4c.txt", "".join(f"{ln} 1\n" for ln in _PTS_IN.splitlines())),
               ("ref4c.txt", "".join(f"{ln} 1\n" for ln in _PTS_REF.splitlines()))))  # fmt: skip

    A = "avscale"
    add("avscale-usage", "avscale without arguments (its usage, exit 255)", A)
    add("avscale-default", "avscale of fMRIPrep's initial matrix", A, M, tier="smoke")
    add("avscale-allparams", "--allparams, the translations about a volume's centre", A,
        "--allparams", "aff.mat", E, files=(("aff.mat", _AFF),))  # fmt: skip
    add("avscale-inverteddies", "--inverteddies (the check and the inverted eddy matrix)", A,
        "--inverteddies", "aff.mat", files=(("aff.mat", _AFF),))  # fmt: skip
    add("avscale-flip", "a matrix that swaps left and right (the half transforms of the swap)",
        A, "flip.mat", files=(("flip.mat", _FLIP),))  # fmt: skip
    add("avscale-singular", "a singular matrix: Armadillo's solve throws, avscale aborts", A,
        "sing.mat", files=(("sing.mat", _SING),))  # fmt: skip
    add("avscale-missing-matrix", "a matrix file that does not exist (exit 254)", A, "none.mat")
    add("avscale-empty-volume", "an empty volume name: the centre of an empty volume", A, M, "")
    add("avscale-missing-volume", "a volume that does not exist: newimage's exception, abort",
        A, M, "none.nii.gz")  # fmt: skip
    for prog, opts in (
        ("img2imgcoord", ("-src", E, "-dest", T)),
        ("std2imgcoord", ("-img", T, "-std", E)),
        ("img2stdcoord", ("-img", T, "-std", E)),
    ):
        files = (("coords.txt", _COORDS),)
        add(f"{prog}-usage", f"{prog} without arguments (its usage, exit 1)", prog)
        add(f"{prog}-xfm", f"{prog} through fMRIPrep's initial matrix", prog, *opts, "-xfm", M,
            "coords.txt", files=files, tier="smoke" if prog == "img2imgcoord" else st)  # fmt: skip
        add(f"{prog}-vox", f"{prog} -vox", prog, *opts, "-xfm", M, "-vox", "coords.txt",
            files=files)  # fmt: skip
        add(f"{prog}-mm", f"{prog} -mm", prog, *opts, "-xfm", M, "-mm", "coords.txt",
            files=files)  # fmt: skip
        add(f"{prog}-verbose", f"{prog} verbose (sizes, matrices; fixed notation after them)",
            prog, *opts, "-xfm", M, "-verbose" if prog != "img2imgcoord" else "-v",
            "coords.txt", files=files)  # fmt: skip
        add(f"{prog}-missing-coords", f"{prog}: a coordinate file that does not exist", prog,
            *opts, "none.txt")  # fmt: skip
        add(f"{prog}-not-numbers", f"{prog}: coordinates that stop at a word", prog, *opts,
            "bad.txt", files=(("bad.txt", "1 2 x 4\n5 6 7\n"),))  # fmt: skip
        add(f"{prog}-lacking", f"{prog}: an option lacking its argument (a message, the run "
            "goes on)", prog, *opts, "coords.txt", "-xfm", files=files)  # fmt: skip
        add(f"{prog}-unknown", f"{prog}: an unknown option (exit 255)", prog, *opts, "-frob",
            "x", "coords.txt", files=files)  # fmt: skip
        add(f"{prog}-missing-image", f"{prog}: an image that does not exist (abort)", prog,
            opts[0], "none.nii", opts[2], opts[3], "coords.txt", files=files)  # fmt: skip
    add("img2imgcoord-flirt-premat", "img2imgcoord -flirt and -premat", "img2imgcoord", "-src",
        E, "-dest", T, "-flirt", "-premat", M, "coords.txt",
        files=(("coords.txt", _COORDS),))  # fmt: skip
    add("std2imgcoord-standard", "std2imgcoord from FSL's MNI152 2 mm", "std2imgcoord", "-img",
        T, "-std", STD, "coords.txt", files=(("coords.txt", _COORDS),))  # fmt: skip
    add("std2imgcoord-no-std", "std2imgcoord without -std (its warning; 1 mm voxels)",
        "std2imgcoord", "-img", T, "coords.txt", files=(("coords.txt", _COORDS),))  # fmt: skip
    add("std2imgcoord-singular", "std2imgcoord with a singular matrix: its inverse throws",
        "std2imgcoord", "-img", T, "-std", E, "-xfm", "sing.mat", "coords.txt",
        files=(("coords.txt", _COORDS), ("sing.mat", _SING)))  # fmt: skip
    add("img2stdcoord-noform", "img2stdcoord to a standard image without sform or qform",
        "img2stdcoord", "-img", T, "-std", "@noform:5x6x7:2x3x4:noform.nii", "coords.txt",
        files=(("coords.txt", _COORDS),))  # fmt: skip
    add("img2stdcoord-warp-missing", "img2stdcoord -warp of a file that does not exist",
        "img2stdcoord", "-img", T, "-std", E, "-warp", "none.nii.gz", "coords.txt",
        files=(("coords.txt", _COORDS),))  # fmt: skip
    R = "rmsdiff"
    add("rmsdiff-usage", "rmsdiff without arguments (its usage, exit 255)", R)
    add("rmsdiff-rms", "the RMS deviation over 80 mm about a volume's centre", R, M, "id.mat",
        E, files=(("id.mat", "1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n"),), tier="smoke")  # fmt: skip
    add("rmsdiff-mask", "the largest and the RMS displacement over a mask", R, M, "aff.mat", T,
        W, files=(("aff.mat", _AFF),))  # fmt: skip
    add("rmsdiff-low-determinant", "a matrix of low determinant (its warning, the matrix on "
        "standard error)", R, "small.mat", M, E,
        files=(("small.mat", "0.1 0 0 0\n0 0.5 0 0\n0 0 1 0\n0 0 0 1\n"),))  # fmt: skip
    add("rmsdiff-singular", "a singular second matrix: rms_deviation's message, exit 251", R, M,
        "sing.mat", E, files=(("sing.mat", _SING),))  # fmt: skip
    add("rmsdiff-missing-matrix", "a matrix file that does not exist", R, "none.mat", M, E)
    add("rmsdiff-4d", "a 4D reference (its first volume, the warning)", R, M, "aff.mat",
        "@bold4.nii", files=(("aff.mat", _AFF),))  # fmt: skip
    K = "makerot"
    add("makerot-usage", "makerot without arguments (the compulsory -t, its usage, exit 1)", K)
    add("makerot-help", "makerot -h (its usage, exit 1)", K, "-h")
    add("makerot-default", "30 degrees about z through the origin", K, "-t", "30", tier="smoke")
    add("makerot-axis-centre", "an axis that is not of unit length (FSL-76) through a centre",
        K, "-t", "30", "-a", "1,2,3", "-c", "4,5,6")  # fmt: skip
    add("makerot-long", "the long options with their values after =", K, "--theta=-45.5",
        "--axis=0,0,4", "--centre=1:2:3")  # fmt: skip
    add("makerot-cov", "the centre of an image's volume (--cov)", K, "-t", "20", "-a", "0,1,1",
        f"--cov={E}")  # fmt: skip
    add("makerot-cov-missing", "--cov of a file that is not an image (FSL aborts)", K, "-t",
        "20", "--cov=none.nii")  # fmt: skip
    add("makerot-zero-axis", "a zero axis (its error, exit 1)", K, "-t", "10", "-a", "0,0,0")
    add("makerot-short-axis", "an axis of one value (FSL-77: the buffer's zeros)", K, "-t", "30",
        "-a", "5")  # fmt: skip
    add("makerot-stale-axis", "the axis given twice, the second of one value (FSL-77: the "
        "first's values)", K, "-t", "30", "-a", "1,2,3", "-a", "7")  # fmt: skip
    add("makerot-bad-theta", "a theta that is not a number (Couldn't set_value, exit 1)", K, "-t",
        "abc")  # fmt: skip
    add("makerot-out", "the matrix written to a file (-o)", K, "-t", "30", "-a", "1,1,0", "-o",
        "out.mat")  # fmt: skip
    add("makerot-out-unwritable", "an output file that cannot be written (the message, exit 0)",
        K, "-t", "30", "-o", "nodir/out.mat")  # fmt: skip
    add("makerot-token", "a word that is not an option (unrecognised token, exit 1)", K, "-t",
        "30", "extra")  # fmt: skip
    return c


@functools.cache
def _all_cases() -> dict[str, tuple[str, str, Run]]:
    return {**_real_cases(), **_schedule_cases(), **_edge_cases(), **_program_cases()}


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
        tool="`larmorx-nc fsl flirt`, `convert_xfm`, `avscale`, `img2imgcoord`, `std2imgcoord`, "
        "`img2stdcoord`, `rmsdiff`, `makerot`, `midtrans`, `pointflirt`, `applyxfm4D` and the "
        "scripts `pairreg`, `extracttxt` and `aff2rigid` (crate larmorx-nc-flirt, translated from "
        f"FSL {fsl_versions.NEWEST}'s source)",
        reference=f"flirt, convert_xfm, avscale, img2imgcoord, std2imgcoord, img2stdcoord, rmsdiff, "
        "makerot, midtrans, pointflirt, applyxfm4D, pairreg, extracttxt and aff2rigid from FSL "
        f"{fsl_versions.NEWEST} (fsl-flirt 2111.4, fsl-newimage 2601.0, fsl-miscmaths 2412.6, "
        "fsl-utils 2412.6, fsl-warpfns 2501.0, fsl-armawrap 0.7.0 with Armadillo 14.2.1; the conda "
        "build at `~/fsl-6.0.7.23`, as built: glibc 2.35's maths, OpenBLAS 0.3.33 with its LAPACK "
        "on its Haswell kernels; the scripts under dash, GNU grep, dc and sed of Ubuntu 22.04 and "
        "FSL's Python with NumPy 2.4.6)",
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
