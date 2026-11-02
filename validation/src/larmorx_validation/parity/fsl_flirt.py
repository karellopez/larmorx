# SPDX-License-Identifier: Apache-2.0
"""Parity of FLIRT's clean-room original (``larmorx fsl flirt`` and ``convert_xfm``, crate
larmorx-fsl, Apache-2.0, written from ``specs/flirt.md``) with FSL 6.0.7.23's ``flirt`` and
``convert_xfm``.

The cases are the replica suite's (``fsl_flirt_replica``: fMRIPrep's three BBR calls, the real
cases on ds005040's images, the schedules, the edge cases and ``convert_xfm``), run here against
the original; FLIRT's other programs (avscale, the coordinate programs, ...) have no original yet
and are left out. Every FSL run goes through the oracle-output cache with the replica suite's
keys, so the runs that suite made are served from it.

What is compared (``specs/flirt.md`` §13):

- the exit status (where FSL aborts, the original exits 1), the files written, standard output
  and error with the numbers in them read as numbers (identical text reported apart; the
  original's ``larmorx:`` lines left out);
- matrices: identical text, or the 95th-percentile displacement over the reference's brain (its
  voxels above 0) between FSL's matrix and the original's within 0.1 mm for the boundary-based
  and 6-dof registrations and 0.5 mm for the others, the RMS deviation (radius 80 mm) reported;
  ``-applyxfm`` and ``convert_xfm``: identical text;
- images: bit-identical when the matrices are identical (decompressed, ``descrip`` left out),
  otherwise the normalised correlation with FSL's image inside the reference's mask at least
  0.999;
- the D13 fix pass: a defect named exactly when the default's outputs differ from ``reproduce``.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt_replica as rep
from larmorx_validation.parity.fsl_mcflirt_replica import (
    Completed,
    _files,
    _is_image,
    _same_streams,
)
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    SKIPPED,
    Case,
    Check,
    CheckList,
    Outcome,
    Suite,
)

SUITE = "fsl-flirt"
#: The programs with a clean-room original.
PROGRAMS = ("flirt", "convert_xfm")
#: Options the original does not have yet (part 2): its cases are skipped.
PENDING = {
    "-anglerep": "-anglerep quaternion (part 2)",
    "alignpaxes": "alignpaxes (part 2)",
    "-debug": "-debug (part 2)",
    "-debugsave": "-debugsave (part 2)",
}

_NUMBER = re.compile(r"[-+]?(?:nan|inf|(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?)")


def original_command() -> list[str]:
    """The original's command line: ``LARMORX_FLIRT_BIN`` (a ``larmorx`` binary), else this
    Python's ``larmorx`` package."""
    exe = os.environ.get("LARMORX_FLIRT_BIN")
    return [exe] if exe else [sys.executable, "-m", "larmorx"]


def run_original(
    program: str,
    args: list[str],
    *,
    cwd: Path,
    settings: dict[str, str | None],
    mode: str,
    threads: int = 4,
) -> Completed:
    env = rep._environment(settings)
    env[defects.ENV] = mode
    env["OMP_NUM_THREADS"] = str(threads)
    env["LARMORX_IMPLEMENTATION"] = "original"
    for k in ("PYTHONPATH", "VIRTUAL_ENV"):
        if k in os.environ:
            env[k] = os.environ[k]
    p = subprocess.run(
        [*original_command(), "fsl", program, *args],
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        errors="replace",
        check=False,
        timeout=3600,
    )
    return Completed(p.returncode, p.stdout, p.stderr)


def _own(text: str) -> str:
    """The text without the original's own lines (notices, ``(larmorx)`` messages)."""
    keep = [
        ln
        for ln in text.split("\n")
        if not ln.startswith(("larmorx: ", "larmorx fsl ")) and not ln.endswith("(larmorx)")
    ]
    return "\n".join(keep)


def _masked(text: str) -> str:
    return _NUMBER.sub("#", text)


def _numbers(text: str) -> list[float]:
    out = []
    for m in _NUMBER.findall(text):
        try:
            out.append(float(m))
        except ValueError:
            pass
    return out


def _reference_mask(path: Path) -> tuple[np.ndarray, np.ndarray] | None:
    """The reference's brain (voxels above 0, all voxels when none) as FSL millimetres (voxel
    index times voxel size, radiological order), and the image's voxel sizes."""
    import nibabel as nib

    try:
        img = nib.load(str(path))
    except Exception:  # noqa: BLE001 - an image FSL reads that nibabel does not
        return None
    data = np.asanyarray(img.dataobj)
    if data.ndim > 3:
        data = data[..., 0]
    while data.ndim < 3:
        data = data[..., None]
    zooms = np.abs(np.array(img.header.get_zooms()[:3], dtype=np.float64))
    zooms[zooms == 0] = 1.0
    h = img.header
    aff = img.get_sform() if int(h["sform_code"]) > 0 else img.get_qform()
    flipped = (int(h["sform_code"]) > 0 or int(h["qform_code"]) > 0) and np.linalg.det(
        aff[:3, :3]
    ) > 0
    idx = np.argwhere(np.isfinite(data) & (data > 0))
    if len(idx) == 0:
        idx = np.argwhere(np.ones(data.shape, bool))
    if len(idx) > 200_000:
        idx = idx[:: len(idx) // 200_000 + 1]
    idx = idx.astype(np.float64)
    if flipped:
        idx[:, 0] = data.shape[0] - 1 - idx[:, 0]
    return idx * zooms, zooms


def displacement(m1: np.ndarray, m2: np.ndarray, pts: np.ndarray) -> tuple[float, float]:
    """The 95th percentile and the maximum of `|M1⁻¹ y − M2⁻¹ y|` over the reference points
    `y` (mm): how far apart the two matrices take each reference point in the input."""
    h = np.c_[pts, np.ones(len(pts))]
    a = h @ np.linalg.inv(m1).T
    b = h @ np.linalg.inv(m2).T
    d = np.linalg.norm((a - b)[:, :3], axis=1)
    return float(np.percentile(d, 95)), float(d.max())


def _threshold(args: list[str]) -> float:
    """0.1 mm for the boundary-based and 6-dof registrations, 0.5 mm otherwise (§13)."""
    j = " ".join(args)
    if "bbr" in j or re.search(r"-dof 6\b", j):
        return 0.1
    return 0.5


def _ncc(a: np.ndarray, b: np.ndarray, mask: np.ndarray) -> float:
    x, y = a[mask].astype(np.float64), b[mask].astype(np.float64)
    x, y = x - x.mean(), y - y.mean()
    d = np.sqrt((x * x).sum() * (y * y).sum())
    return float((x * y).sum() / d) if d > 0 else (1.0 if np.array_equal(a, b) else 0.0)


def compare(
    checks: CheckList,
    program: str,
    args: list[str],
    oracle: Completed,
    ours: Completed,
    a: Path,
    b: Path,
) -> None:
    """Exit status, streams, files, matrices and images (the module's rules)."""
    if oracle.code < 0:
        checks(
            f"exit status ({program} is killed; the original exits 1)",
            ours.code == 1,
            detail=f"{program} signal {-oracle.code}, original {ours.code}",
        )
    else:
        checks("exit status", oracle.code == ours.code, detail=f"{oracle.code} vs {ours.code}")
    for stream in ("stdout", "stderr"):
        want = rep._clean(getattr(oracle, stream), a, program)
        got = rep._clean(_own(getattr(ours, stream)), b, program)
        if oracle.code < 0:
            want = re.sub(r"terminate called after[^\n]*\n(  what\(\):[^\n]*\n)?", "", want)
            got = re.sub(r"terminate called after[^\n]*\n(  what\(\):[^\n]*\n)?", "", got)
        same = want == got
        close = _masked(want) == _masked(got)
        detail = ""
        if not close:
            wl, gl = want.splitlines(), got.splitlines()
            first = next(
                (i for i, (x, y) in enumerate(zip(wl, gl, strict=False)) if _masked(x) != _masked(y)),
                min(len(wl), len(gl)),
            )
            detail = (
                f"line {first + 1}: FSL {wl[first] if first < len(wl) else '<end>'!r}, "
                f"original {gl[first] if first < len(gl) else '<end>'!r}"
            )
        checks(
            f"{stream}: the same lines (numbers aside)",
            close,
            metric="identical",
            value="yes" if same else ("numbers differ" if close else "no"),
            threshold="the same text, numbers read as numbers",
            detail=detail,
        )
    theirs, mine = _files(a, set()), _files(b, set())
    checks(
        "files written",
        set(theirs) == set(mine),
        detail=f"only FSL {sorted(set(theirs) - set(mine))[:6]}, "
        f"only the original {sorted(set(mine) - set(theirs))[:6]}",
    )
    applying = "-applyxfm" in args or "-applyisoxfm" in args or program == "convert_xfm"
    mats_identical = True
    ref_path = _reference_path(args, a)
    pts = None
    for name in sorted(set(theirs) & set(mine)):
        fa, fb = theirs[name], mine[name]
        if _is_image(name):
            continue
        ta, tb = fa.read_text(errors="replace"), fb.read_text(errors="replace")
        if ta == tb:
            continue
        mats_identical = False
        na, nb = _numbers(ta), _numbers(tb)
        is_mat = name.endswith(".mat") and len(na) == 16 and len(nb) == 16
        if applying or not is_mat:
            if is_mat or len(na) != len(nb):
                checks(f"{name} identical", False, detail="the text differs")
                continue
            dev = float(np.max(np.abs(np.array(na) - np.array(nb)))) if na else 0.0
            checks(
                f"{name}: the same numbers within 1e-4",
                bool(np.allclose(na, nb, rtol=1e-4, atol=1e-4, equal_nan=True)),
                metric="max abs difference",
                value=round(dev, 6),
                threshold=1e-4,
            )
            continue
        m1, m2 = np.array(na).reshape(4, 4), np.array(nb).reshape(4, 4)
        if pts is None and ref_path is not None:
            pts = _reference_mask(ref_path)
        if pts is None or not (np.all(np.isfinite(m1)) and np.all(np.isfinite(m2))):
            checks(f"{name}: the matrices", np.allclose(m1, m2, equal_nan=True))
            continue
        try:
            p95, mx = displacement(m1, m2, pts[0])
        except np.linalg.LinAlgError:
            checks(f"{name}: invertible matrices", False)
            continue
        thr = _threshold(args)
        checks(
            f"{name}: 95th-percentile displacement over the reference's brain",
            p95 <= thr,
            metric="p95 displacement (mm)",
            value=round(p95, 4),
            threshold=thr,
            detail=f"maximum {mx:.4f} mm, RMS deviation {rep._rms80(m1, m2):.4f} mm",
        )
    for name in sorted(set(theirs) & set(mine)):
        if not _is_image(name):
            continue
        fa, fb = theirs[name], mine[name]
        if fa.read_bytes() == fb.read_bytes():
            continue
        if mats_identical:
            same = _same_streams(fa, fb, blank_descrip=True)
            detail = "" if same else _image_difference(fa, fb)
            checks(f"{name} bit-identical", same, detail=detail)
        else:
            import nibabel as nib

            ia, ib = nib.load(str(fa)), nib.load(str(fb))
            da = np.asanyarray(ia.dataobj).astype(np.float64)
            db = np.asanyarray(ib.dataobj).astype(np.float64)
            if da.shape != db.shape:
                checks(f"{name}: the same grid", False, detail=f"{da.shape} vs {db.shape}")
                continue
            rm = None
            if ref_path is not None:
                try:
                    r = np.asanyarray(nib.load(str(ref_path)).dataobj)
                    if r.ndim > 3:
                        r = r[..., 0]
                    r = r.reshape(da.shape[:3])
                    rm = r > 0
                except Exception:  # noqa: BLE001
                    rm = None
            if rm is None or not rm.any():
                rm = np.ones(da.shape[:3], bool)
            if da.ndim > 3:
                rm = np.broadcast_to(rm[..., None], da.shape)
            ok = np.isfinite(da) & np.isfinite(db)
            c = _ncc(np.where(ok, da, 0), np.where(ok, db, 0), rm)
            checks(
                f"{name}: correlation with FSL's image inside the reference's mask",
                c >= 0.999,
                metric="NCC",
                value=round(c, 6),
                threshold=0.999,
            )


def _image_difference(fa: Path, fb: Path) -> str:
    import nibabel as nib

    try:
        da = np.asanyarray(nib.load(str(fa)).dataobj).astype(np.float64)
        db = np.asanyarray(nib.load(str(fb)).dataobj).astype(np.float64)
    except Exception as e:  # noqa: BLE001
        return f"cannot read: {e}"
    if da.shape != db.shape:
        return f"shapes {da.shape} vs {db.shape}"
    diff = ~((da == db) | (np.isnan(da) & np.isnan(db)))
    if not diff.any():
        return "the headers differ"
    return f"{int(diff.sum())} of {diff.size} values differ (max {np.nanmax(np.abs(da - db)):.4g})"


def _reference_path(args: list[str], cwd: Path) -> Path | None:
    for i, x in enumerate(args[:-1]):
        if x == "-ref":
            p = Path(args[i + 1])
            if not p.is_absolute():
                p = cwd / p
            for cand in (p, Path(str(p) + ".nii.gz"), Path(str(p) + ".nii")):
                if cand.is_file():
                    return cand
    return None


def fix_pass(
    checks: CheckList,
    program: str,
    prepare,
    args: list[str],
    settings: dict[str, str | None],
    reproduced_dir: Path,
    reproduced: Completed,
) -> None:
    """D13: the original's default run against its ``reproduce`` run (a defect named exactly
    when something changed)."""
    with tempfile.TemporaryDirectory(prefix="lx-flirt-fix-") as name:
        tmp = Path(name)
        if prepare is not None:
            prepare(tmp)
        before = set(_files(tmp, set()))
        r = run_original(program, args, cwd=tmp, settings=settings, mode=defects.FIX)
        entries = defects.notices_in(r.stderr)
        others = []
        if r.code != reproduced.code:
            others.append(f"exit {reproduced.code} -> {r.code}")
        a = dict(_files(reproduced_dir, set()))
        bb = dict(_files(tmp, set()))
        if set(a) != set(bb):
            others.append(f"files {sorted(set(a) ^ set(bb))[:4]}")
        change = defects.Change(0, 0, 0.0)
        for key in sorted((set(a) & set(bb)) - before):
            if _is_image(key):
                from larmorx_validation.parity.fsl_mcflirt_replica import _change

                change = change + _change(bb[key], a[key])
            elif a[key].read_bytes() != bb[key].read_bytes():
                try:
                    va = np.array([float(x) for x in a[key].read_text().split()])
                    vb = np.array([float(x) for x in bb[key].read_text().split()])
                    change = change + defects.Change.of(vb, va)
                except ValueError:
                    others.append(key)
        defects.add_fix_check(
            checks, entries, change, other_changes="; ".join(others), silent_ok=rep._SILENT
        )


def _pending(args: list[str], run: rep.Run) -> str | None:
    words = list(args) + [t for _, t in run.files]
    for k, why in PENDING.items():
        if any(k == w or (k == "alignpaxes" and "alignpaxes" in w) for w in words):
            if k == "-anglerep" and "quaternion" not in " ".join(args):
                continue
            return why
    return None


def _run_case(run: rep.Run, checks: CheckList) -> None:
    pending = _pending(list(run.args), run)
    if pending:
        raise Outcome(SKIPPED, f"not in the clean-room original yet: {pending}")
    if run.hang or run.crash:
        _undefined_case(run, checks)
        return
    settings = dict(run.settings)
    with (
        tempfile.TemporaryDirectory(prefix="lx-flirt-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-flirt-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        args = rep._prepare(run, a)
        rep._prepare(run, b)
        oracle = rep.run_oracle(run.program, args, cwd=a, settings=settings)
        ours = run_original(run.program, args, cwd=b, settings=settings, mode=defects.REPRODUCE)
        compare(checks, run.program, args, oracle, ours, a, b)
        if run.fix and ours.code == 0:
            fix_pass(checks, run.program, lambda t: rep._prepare(run, t), args, settings, b, ours)
    if oracle.code != 0 and all(c.passed for c in checks.checks):
        tail = " ".join(oracle.stderr.split())[:200]
        raise Outcome(BOTH_ERROR, f"{run.program} exits {oracle.code}; the same from the original. {tail}")


def _undefined_case(run: rep.Run, checks: CheckList) -> None:
    """FSL never finishes or crashes (the register entry): FSL is not run again here (the
    replica suite measures it, never cached); the original refuses in both modes (or, where
    FSL-75's result is defined, finishes naming it)."""
    settings = dict(run.settings)
    entry = run.hang or run.crash
    for mode in (defects.REPRODUCE, defects.FIX):
        with tempfile.TemporaryDirectory(prefix="lx-flirt-") as b_:
            b = Path(b_)
            r = run_original(run.program, rep._prepare(run, b), cwd=b, settings=settings, mode=mode)
            if run.crash_finishes:
                named = defects.notices_in(r.stderr)
                checks(
                    f"the original finishes and names {entry} ({mode})",
                    r.code == 0 and entry in named,
                    detail=f"exit {r.code}: {', '.join(named) or 'nothing named'}",
                )
            else:
                checks(
                    f"the original refuses naming {entry} ({mode})",
                    r.code == 1 and entry in r.stderr,
                    detail=f"exit {r.code}: {' '.join(r.stderr.split())[-160:]}",
                )
    if all(c.passed for c in checks.checks):
        raise Outcome(
            EXPECTED,
            f"FSL {'never finishes' if run.hang else 'crashes'} ({entry}: undefined behaviour); "
            "the original " + ("finishes naming it" if run.crash_finishes else "refuses in both modes"),
        )


def _fmriprep_case(payload: tuple[str, str], checks: CheckList) -> None:
    run, node = payload
    args = rep.fmriprep_args(run, node)
    missing = [x for x in args if x.startswith("/") and not Path(x).exists()]
    if missing:
        raise Outcome(SKIPPED, f"input not available: {missing[0]}")
    with (
        tempfile.TemporaryDirectory(prefix="lx-flirt-fp-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-flirt-fp-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        oracle = rep.run_oracle("flirt", args, cwd=a, settings={})
        ours = run_original("flirt", args, cwd=b, settings={}, mode=defects.REPRODUCE)
        compare(checks, "flirt", args, oracle, ours, a, b)
        theirs = next(Path(node).glob("*.mat"), None)
        mine = next(b.glob("*.mat"), None)
        ref = _reference_path(args, a)
        if theirs is not None and mine is not None and ref is not None:
            pts = _reference_mask(ref)
            if pts is not None:
                p95, _ = displacement(np.loadtxt(theirs), np.loadtxt(mine), pts[0])
                checks.add(
                    Check(
                        "fMRIPrep's own matrix (its container's FSL): displacement",
                        True,
                        metric="p95 displacement (mm)",
                        value=round(p95, 4),
                        threshold="reported",
                    )
                )
        if oracle.code == 0 and ours.code == 0:
            fix_pass(checks, "flirt", None, args, {}, b, ours)


def cases(tier: str) -> list[Case]:
    out = []
    for c in rep.cases(tier):
        kind, payload = c.payload
        if kind == "run":
            run = rep._all_cases()[payload][2]
            if run.program not in PROGRAMS:
                continue
        out.append(c)
    return out


def run_case(case: Case, checks: CheckList) -> None:
    kind, payload = case.payload
    if kind == "fmriprep":
        _fmriprep_case(payload, checks)
    elif kind == "direct":
        _run_case(payload, checks)
    else:
        _run_case(rep._all_cases()[payload][2], checks)


def suite() -> Suite:
    import larmorx

    return Suite(
        name=SUITE,
        title="FLIRT (clean-room original)",
        tool=f"`larmorx fsl flirt` and `convert_xfm` (larmorx {larmorx.__version__}, crate "
        "larmorx-fsl: the clean-room original written from specs/flirt.md on the shared "
        "registration engine of larmorx-mri, Apache-2.0)",
        reference="flirt and convert_xfm from FSL 6.0.7.23 (fsl-flirt 2111.4), the conda build at "
        "`~/fsl-6.0.7.23`",
        thresholds=[
            ("Exit status", "the same (where FSL aborts, the original exits 1)"),
            ("Standard output and error", "the same lines, the numbers in them read as numbers"),
            ("Files written", "the same names"),
            (
                "Matrices",
                "identical text, or the 95th-percentile displacement over the reference's brain "
                "within 0.1 mm (boundary-based, 6 dof) or 0.5 mm (other registrations); "
                "`-applyxfm` and `convert_xfm` identical",
            ),
            (
                "Images",
                "bit-identical from the same matrix; from the original's own matrix, NCC with "
                "FSL's image at least 0.999 inside the reference's mask",
            ),
            ("D13 fix pass", "a defect named exactly when the default's outputs differ"),
        ],
        cases=cases,
        run_case=run_case,
        packages=("nibabel", "scipy"),
        notes=(
            "The cases are the replica suite's (`fsl-flirt-replica`) for `flirt` and "
            "`convert_xfm`; FSL's runs are served from the oracle cache under that suite's keys. "
            "The original is compared in `reproduce` mode (D13) and its default `fix` pass "
            "against the reproduced run. Cases of options the original has not yet "
            "(`-anglerep quaternion`, `alignpaxes`, `-debug`, `-debugsave`: part 2) are skipped."
        ),
    )
