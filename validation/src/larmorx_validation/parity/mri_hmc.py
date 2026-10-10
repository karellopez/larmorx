# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx mri hmc`` (clean-room, Apache-2.0) with FSL 6.0.7's ``mcflirt``.

The reference is the set of recorded ``mcflirt`` runs in ``<workspace>/oracles/fsl-6.0.7/
mcflirt/`` (``manifest.json``; each run directory holds ``run.json`` with the exact argv,
environment and exit code, and the output files). Every recorded run is replayed with
``larmorx mri hmc`` (in-process, through the Python console entry point) with the same
arguments and ``FSLOUTPUTTYPE``, and the outputs are compared:

- the set of files written and the exit status;
- the matrices (``-mats``): per volume the RMS deviation (Jenkinson 1999, radius 80 mm, about
  the centre of the reference's field of view), the parameter differences (mm, degrees) and the
  number of ``MAT_*`` files identical as text;
- framewise displacement (Power, radius 50 mm) from the parameters: correlation and maximum
  difference;
- ``.par`` and ``.rms`` files;
- the corrected series and the ``-stats`` and ``-meanvol`` images: header fields and voxel
  values (normalised cross-correlation);
- for synthetic series with a known motion, the RMS deviation from the truth, for both.

``mcflirt`` is deterministic (repeated runs are byte-identical), so the thresholds of
``specs/mcflirt.md`` §14 and PLAN.md §11.3 are compared with a variability band measured by
perturbing the input slightly (``larmorx_validation.parity.mri_hmc_band``).
"""

from __future__ import annotations

import contextlib
import functools
import io
import json
import os
import shutil
import tempfile
from pathlib import Path
from typing import Any

import numpy as np

from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    PASS,
    Case,
    CaseResult,
    Check,
    CheckList,
    Outcome,
    Suite,
)
from larmorx_validation.report import table

ORACLE = "oracles/fsl-6.0.7/mcflirt"
RECORDED = "/oracles/fsl-6.0.7/mcflirt/"
TESTDATA = "larmorx-testdata/data/"

#: specs/mcflirt.md §14 and PLAN.md §11.3 ("Motion parameters").
P95_RMS_MM = 0.1
MEDIAN_MM = 0.05
MEDIAN_DEG = 0.05
FD_R = 0.99
#: FD correlation is only meaningful with some motion (spec §14): it is required where the
#: largest FD reaches this, and reported otherwise.
FD_MIN_MM = 0.5
IMAGE_NCC = 0.999
#: sform/qform agreement of output images (mm).
XFORM_ATOL = 1e-4
#: The synthetic ground truth: larmorx may not be worse than mcflirt by more than this.
TRUTH_SLACK_MM = 0.05

#: Series that only one parameter can align (index into rx ry rz tx ty tz): a ramp along x
#: (spec §6.4). The other parameters wander along flat directions of the cost.
#: ``None``: nothing is identifiable (the centred correlation of ``-smooth 0`` cannot see the
#: shift of a ramp), so the run is only reported.
DEGENERATE: dict[str, int | None] = {
    "syn/content/ramp/reffile": 3,
    "syn/content/ramp/reffile-smooth0": None,
    # Woods' criterion does not register BOLD volumes: mcflirt's matrices are off by tens of
    # millimetres (spec §14), and so are larmorx's, differently.
    "real/ds000005/opt-cost-woods": None,
}
DEGENERATE_MM = 0.3

#: Deliberate differences (docs/api/mri-hmc.md).
EXPECTED_CASES = {
    "syn/motion/small/no-fsloutputtype": (
        "Without FSLOUTPUTTYPE, mcflirt writes the .mat files and fails (exit 1); larmorx writes "
        "every output as .nii.gz (its default)."
    ),
}


@functools.cache
def oracle_dir() -> Path | None:
    """The recorded mcflirt runs: ``LARMORX_MCFLIRT_ORACLE``, else the workspace's
    ``oracles/fsl-6.0.7/mcflirt``."""
    if env := os.environ.get("LARMORX_MCFLIRT_ORACLE"):
        path = Path(env)
        return path if (path / "manifest.json").is_file() else None
    for parent in Path(__file__).resolve().parents:
        candidate = parent / ORACLE
        if (candidate / "manifest.json").is_file():
            return candidate
    return None


@functools.cache
def manifest() -> dict[str, Any]:
    root = oracle_dir()
    if root is None:
        return {"runs": {}}
    return json.loads((root / "manifest.json").read_text(encoding="utf-8"))


def _tier(name: str) -> str:
    if name.startswith("syn/"):
        return "smoke"
    if "stages4" in name or name.endswith("opt-sinc_final"):
        return "full"
    return "standard"


def _category(name: str) -> str:
    parts = name.split("/")
    if parts[0] == "real":
        return "real" if not parts[2].startswith("opt-") else "options (ds000005)"
    return f"synthetic/{parts[1]}"


_TIERS = ("smoke", "standard", "full")


def cases(tier: str) -> list[Case]:
    rank = _TIERS.index(tier)
    root = oracle_dir()
    out = []
    for name in sorted(manifest()["runs"]):
        if _TIERS.index(_tier(name)) > rank:
            continue
        notes = ""
        if root is not None and (root / "runs" / name / "run.json").is_file():
            run = json.loads((root / "runs" / name / "run.json").read_text(encoding="utf-8"))
            notes = run.get("notes") or ""
        out.append(Case(name, _category(name), notes or name, name))
    return out


# ------------------------------------------------------------------------------------------------
# Replaying a recorded run


def _resolve(arg: str) -> str:
    """A recorded argument with machine paths mapped to this machine: test data through
    ``larmorx_testdata``, oracle files (the median references) to the oracle directory."""
    import larmorx_testdata as td

    root = oracle_dir()
    assert root is not None
    if TESTDATA in arg and arg.startswith("/"):
        return str(td.get(arg.split(TESTDATA, 1)[1]))
    if RECORDED in arg and arg.startswith("/"):
        return str(root / arg.split(RECORDED, 1)[1])
    return arg


def _run_larmorx(args: list[str], env: dict[str, str]) -> tuple[int, str, str]:
    from larmorx.cli import run

    saved = {k: os.environ.get(k) for k in ("FSLOUTPUTTYPE",)}
    out, err = io.StringIO(), io.StringIO()
    try:
        for k in saved:
            if k in env:
                os.environ[k] = env[k]
            else:
                os.environ.pop(k, None)
        with contextlib.redirect_stderr(err), contextlib.redirect_stdout(out):
            code = run(["larmorx", "mri", "hmc", *args])
    finally:
        for k, v in saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
    return code, out.getvalue(), err.getvalue()


def _outputs(directory: Path, exclude: set[str]) -> dict[str, Path]:
    """Output files (``.mat`` directories as one entry) by name."""
    out = {}
    for p in sorted(directory.iterdir()):
        if p.name in exclude or p.name == "run.json" or p.name.startswith(("stdout", "stderr")):
            continue
        out[p.name] = p
    return out


# ------------------------------------------------------------------------------------------------
# Comparisons


def _read_mats(d: Path) -> tuple[list[np.ndarray], list[str]]:
    files = sorted(d.glob("MAT_*"), key=lambda p: int(p.name[4:]))
    texts = [f.read_text(encoding="utf-8") for f in files]
    mats = [
        np.array([[float(x) for x in line.split()] for line in t.splitlines()[:4]]) for t in texts
    ]
    return mats, texts


def rms_deviation(
    m1: np.ndarray, m2: np.ndarray, centre: np.ndarray, radius: float = 80.0
) -> float:
    d = m1 @ np.linalg.inv(m2) - np.eye(4)
    a = d[:3, :3]
    t = d[:3, 3] + a @ centre
    return float(np.sqrt(t @ t + radius * radius / 5.0 * np.trace(a.T @ a)))


def decompose(m: np.ndarray, centre: np.ndarray) -> np.ndarray:
    """``rx ry rz tx ty tz`` of a rigid matrix about ``centre`` (specs/mcflirt.md §4.4)."""
    r = m[:3, :3]
    cy = np.hypot(r[0, 0], r[0, 1])
    if cy < 1e-4:
        angles = [np.arctan2(-r[2, 1], r[1, 1]), np.arctan2(-r[0, 2], 0.0), 0.0]
    else:
        angles = [
            np.arctan2(r[1, 2] / cy, r[2, 2] / cy),
            np.arctan2(-r[0, 2], cy),
            np.arctan2(r[0, 1] / cy, r[0, 0] / cy),
        ]
    t = r @ centre + m[:3, 3] - centre
    return np.array([*angles, *t])


def framewise_displacement(params: np.ndarray, radius: float = 50.0) -> np.ndarray:
    d = np.abs(np.diff(params, axis=0))
    fd = (d[:, :3] * radius).sum(axis=1) + d[:, 3:].sum(axis=1)
    return np.concatenate([[0.0], fd])


def _fov_centre(path: Path) -> np.ndarray:
    import nibabel as nib

    h = nib.load(path).header
    shape = np.array(h.get_data_shape()[:3], dtype=float)
    zooms = np.abs(np.array(h["pixdim"][1:4], dtype=float))
    zooms[zooms == 0] = 1.0
    return 0.5 * (shape - 1) * zooms


def _ncc(a: np.ndarray, b: np.ndarray) -> float:
    ok = np.isfinite(a) & np.isfinite(b)
    a, b = a[ok] - a[ok].mean(), b[ok] - b[ok].mean()
    den = np.sqrt((a * a).sum() * (b * b).sum())
    return float((a * b).sum() / den) if den > 0 else (1.0 if np.array_equal(a, b) else 0.0)


HEADER_FIELDS = ("dim", "datatype", "pixdim", "qform_code", "sform_code", "xyzt_units")


def _compare_image(checks: CheckList, name: str, a_path: Path, b_path: Path) -> None:
    import nibabel as nib

    a, b = nib.load(a_path), nib.load(b_path)
    ha, hb = a.header, b.header
    differing = [
        f"{k}: mcflirt {np.asarray(ha[k]).tolist()}, larmorx {np.asarray(hb[k]).tolist()}"
        for k in HEADER_FIELDS
        if not np.array_equal(np.asarray(ha[k]), np.asarray(hb[k]), equal_nan=True)
    ]
    # mcflirt writes the qform quaternion back from its own matrix: its last float32 bits
    # differ, which moves the qform by up to ~1e-5 mm where the quaternion's w is small.
    for label, fa, fb in [
        ("sform", ha.get_sform(), hb.get_sform()),
        ("qform", ha.get_qform(), hb.get_qform()),
    ]:
        if not np.allclose(fa, fb, atol=XFORM_ATOL):
            differing.append(f"{label} differs by {np.abs(fa - fb).max():.2g}")
    checks(
        f"{name}: header (type, shape, voxel sizes, units, xforms)",
        not differing,
        detail="; ".join(differing),
    )
    da = np.asanyarray(a.dataobj).astype(np.float64)
    db = np.asanyarray(b.dataobj).astype(np.float64)
    if da.shape != db.shape:
        return
    ncc = _ncc(da, db)
    diff = np.abs(da - db)
    diff = diff[np.isfinite(diff)]
    scale = float(np.nanmax(np.abs(da))) if da.size else 0.0
    checks(
        f"{name}: voxel values",
        ncc >= IMAGE_NCC,
        metric="NCC",
        value=f"{ncc:.6f}",
        threshold=f"≥ {IMAGE_NCC}",
        detail=f"max |diff| {diff.max() if diff.size else 0:.4g} (max |mcflirt| {scale:.4g}); "
        f"{int((diff > 0).sum())} of {da.size} values differ",
    )


def _text_numbers(path: Path) -> np.ndarray:
    rows = [line.split() for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    return np.array([[float(x) for x in r] for r in rows])


def _thresholds(case: str) -> tuple[float, float, float, float, str]:
    """``(p95 RMS mm, median mm, median deg, FD r, source)``: the spec's thresholds, widened to
    mcflirt's own variability for this case where that is larger."""
    from larmorx_validation.parity.mri_hmc_band import band_for

    # Runs that repeat an estimation with more output files share its band.
    base = case.removesuffix("-reports")
    for suffix in ("-repeat2", "-repeat3"):
        base = base.removesuffix(suffix)
    band = band_for(base)
    if band is None:
        return P95_RMS_MM, MEDIAN_MM, MEDIAN_DEG, FD_R, "spec"
    p95 = max(P95_RMS_MM, band["rms_p95"])
    mm = max(MEDIAN_MM, band["param_mm_median"])
    deg = max(MEDIAN_DEG, band["param_deg_median"])
    fd_r = min(FD_R, band["fd_r"])
    widened = (p95, mm, deg, fd_r) != (P95_RMS_MM, MEDIAN_MM, MEDIAN_DEG, FD_R)
    return p95, mm, deg, fd_r, "band" if widened else "spec"


def _compare_degenerate(
    checks: CheckList, a: list, b: list, centre: np.ndarray, axis: int | None
) -> dict[str, Any]:
    """A series that at most one parameter can align (the ramp): compare that parameter."""
    pa = np.array([decompose(m, centre) for m in a])
    pb = np.array([decompose(m, centre) for m in b])
    if axis is None:
        checks.add(
            Check(
                "no parameter is identifiable",
                True,
                metric="max |Δ| (° / mm)",
                value=f"{np.degrees(np.abs(pa[:, :3] - pb[:, :3])).max():.2f} / "
                f"{np.abs(pa[:, 3:] - pb[:, 3:]).max():.2f}",
                threshold="reported",
            )
        )
        return {}
    d = float(np.abs(pa[:, axis] - pb[:, axis]).max())
    checks(
        "the identifiable translation",
        d <= DEGENERATE_MM,
        metric="max |diff| (mm)",
        value=f"{d:.4f}",
        threshold=f"≤ {DEGENERATE_MM}",
        detail="the other parameters are not constrained by the data (reported only): max "
        f"|Δ| {np.degrees(np.abs(pa[:, :3] - pb[:, :3])).max():.2f}°, "
        f"{np.abs(np.delete(pa[:, 3:] - pb[:, 3:], axis - 3, axis=1)).max():.2f} mm",
    )
    return {}


def _compare_run(
    checks: CheckList,
    case: str,
    ours: dict[str, Path],
    theirs: dict[str, Path],
    reference: Path,
    truth: Any,
) -> dict[str, Any]:
    metrics: dict[str, Any] = {}
    checks(
        "files written",
        set(ours) == set(theirs),
        detail=f"mcflirt {sorted(theirs)}; larmorx {sorted(ours)}",
    )
    centre = _fov_centre(reference)
    p95_max, mm_max, deg_max, fd_min, source = _thresholds(case)
    is_degenerate = case in DEGENERATE
    degenerate = DEGENERATE.get(case)
    mat_dirs = [k for k in theirs if k.endswith(".mat") or ".mat+" in k]
    for key in mat_dirs:
        if key not in ours:
            continue
        a, ta = _read_mats(theirs[key])
        b, tb = _read_mats(ours[key])
        checks(f"{key}: number of matrices", len(a) == len(b), detail=f"{len(a)} vs {len(b)}")
        if len(a) != len(b) or not a:
            continue
        if is_degenerate:
            return _compare_degenerate(checks, a, b, centre, degenerate)
        nan_a = [bool(np.isnan(m).any()) for m in a]
        nan_b = [bool(np.isnan(m).any()) for m in b]
        checks(
            f"{key}: NaN matrices in the same volumes",
            nan_a == nan_b,
            detail=f"{sum(nan_a)} vs {sum(nan_b)}",
        )
        idx = [i for i in range(len(a)) if not nan_a[i] and not nan_b[i]]
        if not idx:
            continue
        devs = np.array([rms_deviation(a[i], b[i], centre) for i in idx])
        identical = sum(ta[i] == tb[i] for i in range(len(a)))
        pa = np.array([decompose(a[i], centre) for i in idx])
        pb = np.array([decompose(b[i], centre) for i in idx])
        drot = np.degrees(np.abs(pa[:, :3] - pb[:, :3]))
        dtr = np.abs(pa[:, 3:] - pb[:, 3:])
        p95 = float(np.percentile(devs, 95))
        checks(
            f"{key}: matrix RMS deviation, 95th percentile",
            p95 <= p95_max,
            metric="mm",
            value=f"{p95:.4f}",
            threshold=f"≤ {p95_max:.4g} ({source})",
            detail=f"median {np.median(devs):.4f}, max {devs.max():.4f} mm over {len(idx)} volumes",
        )
        med_tr, med_rot = float(np.median(dtr)), float(np.median(drot))
        checks(
            f"{key}: median parameter difference",
            med_tr <= mm_max and med_rot <= deg_max,
            metric="mm / deg",
            value=f"{med_tr:.4f} / {med_rot:.4f}",
            threshold=f"≤ {mm_max:.4g} / {deg_max:.4g} ({source})",
            detail=f"max {dtr.max():.4f} mm, {drot.max():.4f}°",
        )
        checks.add(
            Check(
                f"{key}: matrices identical as text",
                True,
                metric="files",
                value=f"{identical}/{len(a)}",
                threshold="reported",
            )
        )
        metrics.update(
            volumes=len(idx),
            rms_median=float(np.median(devs)),
            rms_p95=p95,
            rms_max=float(devs.max()),
            param_mm_median=med_tr,
            param_deg_median=med_rot,
            param_mm_max=float(dtr.max()),
            param_deg_max=float(drot.max()),
            identical=identical,
            mats=len(a),
            threshold_source=source,
        )
        if len(idx) >= 3:
            fa, fb = framewise_displacement(pa), framewise_displacement(pb)
            fd_max = float(fa.max())
            r = float(np.corrcoef(fa, fb)[0, 1]) if fa.std() > 0 and fb.std() > 0 else 1.0
            fd_diff = float(np.abs(fa - fb).max())
            needed = fd_max >= FD_MIN_MM
            checks(
                "FD (Power, 50 mm) correlation",
                r >= fd_min or not needed,
                metric="r",
                value=f"{r:.4f}",
                threshold=(
                    f"≥ {fd_min:.4g} ({source})"
                    if needed
                    else f"reported (max FD {fd_max:.3f} mm < {FD_MIN_MM})"
                ),
                detail=f"max |ΔFD| {fd_diff:.4f} mm, max FD {fd_max:.3f} mm",
            )
            metrics.update(fd_r=r, fd_max_diff=fd_diff, fd_max=fd_max)
        if truth is not None and len(truth) == len(a):
            ta_dev = np.array([rms_deviation(np.array(truth[i]), a[i], centre) for i in idx])
            tb_dev = np.array([rms_deviation(np.array(truth[i]), b[i], centre) for i in idx])
            checks(
                "accuracy against the synthetic truth (mean RMS deviation)",
                tb_dev.mean() <= ta_dev.mean() + TRUTH_SLACK_MM,
                metric="mm",
                value=f"{tb_dev.mean():.4f}",
                threshold=f"≤ mcflirt's {ta_dev.mean():.4f} + {TRUTH_SLACK_MM}",
                detail=f"max larmorx {tb_dev.max():.4f}, mcflirt {ta_dev.max():.4f} mm",
            )
            metrics.update(truth_larmorx=float(tb_dev.mean()), truth_mcflirt=float(ta_dev.mean()))
    if is_degenerate:
        return metrics
    for key in theirs:
        if key not in ours:
            continue
        if key.endswith(".par"):
            a, b = _text_numbers(theirs[key]), _text_numbers(ours[key])
            if a.shape == b.shape and a.size:
                ok = np.isfinite(a) & np.isfinite(b)
                d = np.abs(a - b)
                nan_same = np.array_equal(np.isnan(a), np.isnan(b))
                rot = np.degrees(d[:, :3][ok[:, :3]]) if ok[:, :3].any() else np.zeros(1)
                tr = d[:, 3:][ok[:, 3:]] if ok[:, 3:].any() else np.zeros(1)
                checks(
                    f"{key}: median difference",
                    nan_same and np.median(tr) <= mm_max and np.median(rot) <= deg_max,
                    metric="mm / deg",
                    value=f"{np.median(tr):.4f} / {np.median(rot):.4f}",
                    threshold=f"≤ {mm_max:.4g} / {deg_max:.4g} ({source})",
                    detail=f"max {tr.max():.4f} mm, {rot.max():.4f}°; NaN in the same places: {nan_same}",
                )
            else:
                checks(f"{key}: shape", False, detail=f"{a.shape} vs {b.shape}")
        elif key.endswith(".rms"):
            a, b = _text_numbers(theirs[key]), _text_numbers(ours[key])
            if a.shape == b.shape:
                d = np.abs(a - b)
                d = d[np.isfinite(d)]
                worst = float(d.max()) if d.size else 0.0
                checks(
                    f"{key}: values",
                    worst <= p95_max and np.array_equal(np.isnan(a), np.isnan(b)),
                    metric="max |diff| (mm)",
                    value=f"{worst:.4f}",
                    threshold=f"≤ {p95_max:.4g} ({source})",
                )
            else:
                checks(f"{key}: shape", False, detail=f"{a.shape} vs {b.shape}")
        elif key.endswith((".nii.gz", ".nii", ".hdr")) and theirs[key].is_file():
            _compare_image(checks, key, theirs[key], ours[key])
    return metrics


def _truth(run: dict[str, Any]) -> tuple[list | None, bool]:
    """The synthetic ground truth when the run registers to the phantom at rest."""
    inp = run["inputs"].get("in", {}).get("path", "")
    ref = run["inputs"].get("ref", {}).get("path", "")
    if TESTDATA not in inp or "synthetic/hmc/" not in inp or "phantom/ref.nii.gz" not in ref:
        return None, False
    rel = inp.split(TESTDATA, 1)[1].replace(".nii.gz", ".json")
    import larmorx_testdata as td

    try:
        sidecar = json.loads(Path(td.get(rel)).read_text(encoding="utf-8"))
    except Exception:
        return None, False
    return sidecar.get("expected_fsl_mat"), True


def run_case(case: Case, checks: CheckList) -> dict[str, Any]:
    root = oracle_dir()
    if root is None:
        raise Outcome(
            "skipped", "the recorded mcflirt runs were not found (LARMORX_MCFLIRT_ORACLE)"
        )
    name: str = case.payload
    rdir = root / "runs" / name
    run = json.loads((rdir / "run.json").read_text(encoding="utf-8"))
    marker = RECORDED + "runs/" + name + "/"
    copied: set[str] = set()
    metrics: dict[str, Any] = {}

    def local(arg: str, tmp: Path, is_out: bool) -> str:
        """The run's own directory maps to ``tmp`` (inputs found there are copied first)."""
        if marker in arg and arg.startswith("/"):
            rel = arg.split(marker, 1)[1]
            if not is_out and (rdir / rel).is_file():
                (tmp / rel).parent.mkdir(parents=True, exist_ok=True)
                shutil.copy(rdir / rel, tmp / rel)
                copied.add(rel.split("/")[0])
            return str(tmp / rel)
        try:
            return _resolve(arg)
        except Exception as e:  # test data not available here
            raise Outcome("skipped", f"input not available: {e}") from e

    with tempfile.TemporaryDirectory(prefix="lx-hmc-") as tmp_name:
        tmp = Path(tmp_name)
        codes, logs = [], []
        for step in run["steps"]:
            argv = step["argv"][1:]
            args = [
                local(arg, tmp, i > 0 and argv[i - 1] in ("-out", "-o"))
                for i, arg in enumerate(argv)
            ]
            code, out, err = _run_larmorx(args, run["env"])
            codes.append(code)
            logs.append((out, err))
        oracle_codes = [s["exit"] for s in run["steps"]]
        oracle_ok = all(c == 0 for c in oracle_codes)
        ours_ok = all(c == 0 for c in codes)
        last_err = " ".join(logs[-1][1].split())[-200:]
        if name in EXPECTED_CASES:
            checks(
                "larmorx succeeds where mcflirt fails",
                ours_ok and not oracle_ok,
                detail=f"mcflirt exit {oracle_codes}, larmorx {codes}",
            )
            raise Outcome(EXPECTED, EXPECTED_CASES[name])
        if not oracle_ok and not ours_ok:
            # mcflirt aborts (signal 6) where larmorx exits 1 with a message.
            same = all(
                a == b or (a < 0 and b == 1) for a, b in zip(oracle_codes, codes, strict=False)
            )
            err_path = rdir / "stderr0.txt"
            stderr0 = (
                err_path.read_text(encoding="utf-8", errors="replace") if err_path.is_file() else ""
            )
            reason = (
                f"mcflirt exit {oracle_codes}: {' '.join(stderr0.split())[:160]}; "
                f"larmorx exit {codes}: {last_err}"
            )
            if same:
                raise Outcome(BOTH_ERROR, reason)
            checks("exit status", False, detail=reason)
            return metrics
        checks(
            "exit status",
            oracle_codes == codes,
            detail=f"mcflirt {oracle_codes}, larmorx {codes}: {last_err}",
        )
        if not (oracle_ok and ours_ok):
            return metrics
        theirs = _outputs(rdir, copied)
        ours = _outputs(tmp, copied)
        ref_arg = run["inputs"].get("ref", run["inputs"]["in"])["path"]
        reference = Path(local(ref_arg, tmp, False))
        truth, _ = _truth(run)
        metrics = _compare_run(checks, name, ours, theirs, reference, truth)
        out_path = rdir / "stdout0.txt"
        stdout0 = out_path.read_text(encoding="utf-8") if out_path.is_file() else ""
        refnum = [ln for ln in stdout0.splitlines() if ln.startswith(("refnum", "Original_refvol"))]
        if refnum:
            ours_refnum = [
                ln for ln in logs[0][0].splitlines() if ln.startswith(("refnum", "Original_refvol"))
            ]
            checks(
                "stdout: refnum and Original_refvol",
                refnum == ours_refnum,
                detail=f"{refnum} vs {ours_refnum}",
            )
    return metrics


_METRICS: dict[str, dict[str, Any]] = {}


def _run_case_recording(case: Case, checks: CheckList) -> None:
    _METRICS[case.id] = run_case(case, checks) or {}


# ------------------------------------------------------------------------------------------------
# Report


def _band_rows() -> list[dict[str, Any]]:
    root = oracle_dir()
    path = None if root is None else root / "band" / "band.json"
    if path is None or not path.is_file():
        return []
    return json.loads(path.read_text(encoding="utf-8"))["runs"]


def _band_lines() -> list[str]:
    runs = _band_rows()
    if not runs:
        return ["- The variability band (perturbation runs of mcflirt) has not been measured here."]
    rows = [
        (
            f"`{b['case']}`",
            b["perturbation"],
            f"{b['rms_median']:.4f}",
            f"{b['rms_p95']:.4f}",
            f"{b['param_mm_median']:.4f} / {b['param_deg_median']:.4f}",
            f"{b.get('fd_r', float('nan')):.4f}",
        )
        for b in sorted(runs, key=lambda b: (b["case"], b["perturbation"]))
    ]
    return [
        "## mcflirt's own variability band",
        "",
        "mcflirt is deterministic, so its variability is measured by perturbing its input slightly "
        "and comparing its matrices with its own unperturbed ones "
        "(`python -m larmorx_validation.parity.mri_hmc_band`): uniform noise of ±0.5 intensity "
        "units, the volumes in reverse order (runs with a separate reference), the top slice "
        "cropped. Noise and reverse order set the widened thresholds; cropping changes the "
        "problem more (it breaks thin images) and is only reported.",
        "",
        table(
            (
                "Run",
                "Perturbation",
                "RMS dev. median (mm)",
                "95th pct (mm)",
                "Median Δparam (mm / °)",
                "FD r",
            ),
            rows,
        ),
    ]


def _highlights(results: list[CaseResult]) -> list[str]:
    from larmorx_validation.parity.mri_hmc_band import band_for

    compared = [r for r in results if r.status == PASS and _METRICS.get(r.case, {}).get("mats")]
    lines: list[str] = []
    if not compared:
        return [*lines, "", *_band_lines()]
    allm = [_METRICS[r.case] for r in compared]
    ident = sum(m["identical"] for m in allm)
    total = sum(m["mats"] for m in allm)
    fmriprep = [r for r in compared if r.case.startswith("real/") and r.case.endswith("/fmriprep")]
    if fmriprep:
        rows = []
        for r in fmriprep:
            m = _METRICS[r.case]
            band = band_for(r.case) or {}
            rows.append(
                (
                    r.case.split("/")[1],
                    m["volumes"],
                    f"{m['rms_median']:.4f}",
                    f"**{m['rms_p95']:.4f}**",
                    f"{band.get('rms_p95', float('nan')):.4f}",
                    f"{m['param_mm_median']:.4f} / {m['param_deg_median']:.4f}",
                    f"{m.get('fd_r', float('nan')):.4f}",
                    f"{band.get('fd_r', float('nan')):.4f}",
                )
            )
        lines += [
            "**fMRIPrep's command on real BOLD runs** (`-reffile <HMC reference> -mats`): "
            "larmorx's matrices against mcflirt's, next to mcflirt's own deviation when its "
            "input is perturbed slightly (the band, worst of noise and reverse order):",
            "",
            table(
                (
                    "Run",
                    "Volumes",
                    "RMS dev. median (mm)",
                    "95th pct (mm)",
                    "mcflirt band 95th pct (mm)",
                    "Median Δparam (mm / °)",
                    "FD r",
                    "mcflirt band FD r",
                ),
                rows,
            ),
            "",
        ]
    lines += [
        f"**All runs with `-mats`** ({len(compared)}): the worst 95th-percentile RMS deviation "
        f"is {max(m['rms_p95'] for m in allm):.4f} mm and the worst median parameter difference "
        f"{max(m['param_mm_median'] for m in allm):.4f} mm / "
        f"{max(m['param_deg_median'] for m in allm):.4f}° (thresholds per run: "
        f"{P95_RMS_MM} mm and {MEDIAN_MM} mm / {MEDIAN_DEG}°, or mcflirt's band where wider; "
        f"{sum(m.get('threshold_source') == 'band' for m in allm)} runs use the band). "
        f"{ident} of {total} matrix files are identical to mcflirt's as text (6 decimals).",
        "",
    ]
    rows = []
    for cat in sorted({r.category for r in compared}):
        ms = [_METRICS[r.case] for r in compared if r.category == cat]
        fdr = [m["fd_r"] for m in ms if "fd_r" in m and m.get("fd_max", 0) >= FD_MIN_MM]
        rows.append(
            (
                cat,
                len(ms),
                f"{np.median([m['rms_median'] for m in ms]):.4f}",
                f"{max(m['rms_p95'] for m in ms):.4f}",
                f"{max(m['param_mm_median'] for m in ms):.4f} / "
                f"{max(m['param_deg_median'] for m in ms):.4f}",
                f"{max(m['param_mm_max'] for m in ms):.3f} / {max(m['param_deg_max'] for m in ms):.3f}",
                f"{min(fdr):.4f}" if fdr else "–",
                f"{sum(m['identical'] for m in ms)}/{sum(m['mats'] for m in ms)}",
            )
        )
    lines += [
        table(
            (
                "Category",
                "Runs",
                "RMS dev. median (mm)",
                "Worst 95th pct (mm)",
                "Worst median Δparam (mm / °)",
                "Max Δparam (mm / °)",
                "Lowest FD r",
                "Identical .mat",
            ),
            rows,
        ),
        "",
    ]
    truth = [_METRICS[r.case] for r in compared if "truth_larmorx" in _METRICS[r.case]]
    if truth:
        lines += [
            "**Against the synthetic ground truth** (mean RMS deviation from the true motion, "
            f"{len(truth)} runs): larmorx {np.mean([m['truth_larmorx'] for m in truth]):.4f} mm, "
            f"mcflirt {np.mean([m['truth_mcflirt'] for m in truth]):.4f} mm.",
            "",
        ]
    return [*lines, *_band_lines()]


def suite() -> Suite:
    return Suite(
        name="mri-hmc",
        title="Head-motion correction (mcflirt-compatible)",
        tool="`larmorx mri hmc` / `lx.mri.hmc` (crate larmorx-mri, clean-room from "
        "`specs/mcflirt.md`, Apache-2.0)",
        reference="mcflirt from FSL 6.0.7.17 (fsl-mcflirt 2111.0), the recorded runs in "
        "`oracles/fsl-6.0.7/mcflirt/`",
        thresholds=[
            ("Exit status and files written", "the same (errors: both reject)"),
            ("Matrix RMS deviation (radius 80 mm), 95th percentile per run", f"≤ {P95_RMS_MM} mm"),
            ("Median parameter difference per run", f"≤ {MEDIAN_MM} mm and ≤ {MEDIAN_DEG}°"),
            (
                "FD (Power, 50 mm) correlation",
                f"≥ {FD_R} where the largest FD is ≥ {FD_MIN_MM} mm (band: mcflirt's own, if lower)",
            ),
            (
                "Thresholds marked *band*",
                "widened to mcflirt's own deviation under small perturbations of that run's input "
                "(the variability band), where that is larger",
            ),
            (
                "Ramp series (only x-translation identifiable)",
                f"x translation within {DEGENERATE_MM} mm; the rest reported",
            ),
            (
                "`.par` median differences; `.rms` values",
                f"≤ {MEDIAN_MM} mm / {MEDIAN_DEG}°; ≤ {P95_RMS_MM} mm",
            ),
            (
                "Images: header (type, shape, voxel sizes, units, xforms); values",
                f"identical; NCC ≥ {IMAGE_NCC}",
            ),
            ("Synthetic truth: mean RMS deviation", f"larmorx ≤ mcflirt + {TRUTH_SLACK_MM} mm"),
        ],
        cases=cases,
        run_case=_run_case_recording,
        packages=("nibabel",),
        highlights=_highlights,
        notes=(
            "Each recorded mcflirt run is replayed with the same arguments and FSLOUTPUTTYPE; "
            "outputs are read with nibabel. Parameters are decomposed about the centre of the "
            "reference's field of view (FSL-mm) for the comparison; the `.par` files are "
            "compared as written (about the reference's intensity-weighted centre). The `descrip` "
            "header field differs by design (larmorx writes its own name). larmorx uses all "
            "logical CPUs; its results do not depend on the thread count."
        ),
    )
