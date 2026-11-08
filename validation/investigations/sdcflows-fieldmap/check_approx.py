# SPDX-License-Identifier: Apache-2.0
"""Bit-compare the core of lx.sdcflows' BSplineApprox with sdcflows' interface.

Runs sdcflows' ``BSplineApprox`` (nipype interface, in a temporary directory) in this
interpreter's environment (run it in each kept one: ``oracles/python-newest`` and the shared
``.venv``):

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python LARMORX_WORKSPACE=<workspace> \\
        python -P check_approx.py [--quick]

and ``_core.sdcflows_bspline_approx`` on the same inputs, loaded as sdcflows loads them
(``get_fdata(dtype='float32')``, ``np.asanyarray(mask.dataobj) > 1e-4``, the header's
zooms). With a resize (``zooms_min`` above a zoom) the resampled field map and mask come from
sdcflows' own ``resample_to_zooms`` (the resampling is not part of this check). Compared: the
coefficients of every level, the field, the error, the extrapolated field (the arrays sdcflows
writes, as stored), the intercept, the log records sdcflows writes at INFO and DEBUG, and the
message of a failed fit. Cases: the 7 ``bs_filter`` calls of the oracle runs (fMRIPrep's
parameters) and synthetic fields for every option (recentring by median, mode, mean or none;
no mask, a full mask, a scaled mask; two levels; alpha; no extrapolation; a kernel-system grid;
a single slice; a resize; a failed fit). One line per case; exit status 1 if any differs.
"""

import logging
import os
import sys
import tempfile
import warnings
from pathlib import Path

import nibabel as nb
import numpy as np
from sdcflows.interfaces.bspline import BSplineApprox, bspline_grid
from sdcflows.utils.tools import resample_to_zooms

from larmorx import _core

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE") or Path(__file__).resolve().parents[4])
R = WORKSPACE / "oracles" / "fmriprep-25.2.6"
QUICK = "--quick" in sys.argv
FAILS: list[str] = []


class Capture(logging.Handler):
    def __init__(self) -> None:
        super().__init__(logging.DEBUG)
        self.records: list[tuple[str, str]] = []

    def emit(self, record: logging.LogRecord) -> None:
        if record.levelno in (logging.INFO, logging.DEBUG):
            self.records.append((record.levelname.lower(), record.getMessage()))


def same(a, b) -> bool:
    a, b = np.asarray(a), np.asarray(b)
    return a.shape == b.shape and a.dtype == b.dtype and a.tobytes() == b.tobytes()


def stored(path) -> np.ndarray:
    return np.asanyarray(nb.load(path).dataobj)


def run_sdcflows(tmp, in_data, in_mask, **kw):
    log = logging.getLogger("nipype.interface")
    cap = Capture()
    old = log.level
    log.setLevel(logging.DEBUG)
    log.addHandler(cap)
    try:
        node = BSplineApprox(in_data=str(in_data), **kw)
        if in_mask is not None:
            node.inputs.in_mask = str(in_mask)
        res = node.run(cwd=tmp)
        return res.outputs, cap.records, None
    except RuntimeError as e:
        return None, cap.records, str(e)
    finally:
        log.removeHandler(cap)
        log.setLevel(old)


def run_larmorx(in_data, in_mask, *, bs_spacing, ridge_alpha, recenter, extrapolate, zooms_min):
    fmap = nb.load(in_data)
    zooms = [float(z) for z in fmap.header.get_zooms()]
    mask_img = nb.load(in_mask) if in_mask is not None else None
    resampled = None
    if np.any(np.array(zooms) < zooms_min):
        target = np.maximum(zooms, zooms_min)
        rf = resample_to_zooms(fmap, target)
        rm = resample_to_zooms(mask_img, target) if mask_img is not None else None
        resampled = (
            np.ascontiguousarray(rf.get_fdata(dtype="float32").reshape(-1)),
            tuple(int(n) for n in rf.shape[:3]),
            np.ascontiguousarray(rf.affine, dtype=np.float64),
            None
            if rm is None
            else np.ascontiguousarray((np.asanyarray(rm.dataobj) > 1e-4).reshape(-1)),
        )
    data = fmap.get_fdata(dtype="float32")
    mask = None if mask_img is None else (np.asanyarray(mask_img.dataobj) > 1e-4).reshape(-1)
    rec = "none" if recenter is False else recenter
    pre = []
    if resampled is not None:
        # sdcflows' message before resampling (the Python wrapper's, not the core's)
        target = np.maximum(zooms, zooms_min)
        pre.append(
            (
                "info",
                "Resampling image with resolution exceeding 'zooms_min' "
                f"({'x'.join(str(s) for s in fmap.header.get_zooms())} → "
                f"{'x'.join(str(s) for s in target)}).",
            )
        )
    try:
        out = _core.sdcflows_bspline_approx(
            np.ascontiguousarray(data.reshape(-1)),
            tuple(int(n) for n in fmap.shape[:3]),
            np.ascontiguousarray(fmap.affine, dtype=np.float64),
            zooms,
            [tuple(float(v) for v in s) for s in bs_spacing],
            None if mask is None else np.ascontiguousarray(mask),
            ridge_alpha,
            rec,
            extrapolate,
            str(in_data),
            resampled,
            14,
        )
        out["log"] = pre + [tuple(r) for r in out["log"]]
        return out, None
    except RuntimeError as e:
        return None, str(e)


def case(name, in_data, in_mask=None, **kw):
    params = {
        "bs_spacing": [(16.0, 16.0, 10.0)],
        "ridge_alpha": 1e-4,
        "recenter": "median",
        "extrapolate": True,
        "zooms_min": 1.0,
    }
    params.update(kw)
    with tempfile.TemporaryDirectory() as tmp:
        outs, log_ref, err_ref = run_sdcflows(tmp, in_data, in_mask, **params)
        mine, err = run_larmorx(in_data, in_mask, **params)
        if err_ref is not None or err is not None:
            ok = err_ref == err
            print(f"{'OK  ' if ok else 'FAIL'} {name}: RuntimeError {err_ref!r} vs {err!r}")
            if not ok:
                FAILS.append(name)
            return
        shape = nb.load(in_data).shape[:3]
        checks = {}
        coeffs = outs.out_coeff if isinstance(outs.out_coeff, list) else [outs.out_coeff]
        checks["coefficients"] = len(coeffs) == len(mine["coefficients"]) and all(
            same(stored(f).reshape(-1), c)
            for f, c in zip(coeffs, mine["coefficients"], strict=False)
        )
        # the grids in memory (a file stores the affine in float32)
        fmap = nb.load(in_data)
        checks["grids"] = all(
            tuple(nb.load(f).shape) == tuple(g[0])
            and same(bspline_grid(fmap, control_zooms_mm=sp).affine, g[1])
            for f, g, sp in zip(coeffs, mine["grids"], params["bs_spacing"], strict=False)
        )
        checks["field"] = same(stored(outs.out_field), mine["field"].reshape(shape))
        err_stored = stored(outs.out_error)
        checks["error"] = same(err_stored, mine["error"].reshape(shape).astype(err_stored.dtype))
        if params["extrapolate"]:
            checks["extrapolated"] = mine["extrapolated"] is not None and same(
                stored(outs.out_extrapolated), mine["extrapolated"].reshape(shape)
            )
            if mask_all(in_mask, shape):
                checks["extrapolated-is-field"] = outs.out_extrapolated == outs.out_field
        else:
            checks["no-extrapolation"] = mine["extrapolated"] is None
        checks["intercept"] = outs.out_intercept == 0.0
        checks["log"] = log_ref == [tuple(r) for r in mine["log"]]
        ok = all(checks.values())
        bad = [k for k, v in checks.items() if not v]
        print(
            f"{'OK  ' if ok else 'FAIL'} {name}: {mine['n_inside']} inside, centre "
            f"{mine['center']!r}, {mine['n_iter']} iterations" + ("" if ok else f"; differ: {bad}")
        )
        if not ok:
            FAILS.append(name)
            if not checks["log"]:
                print("   sdcflows:", log_ref)
                print("   larmorx: ", mine["log"])


def mask_all(in_mask, shape) -> bool:
    if in_mask is None:
        return True
    return bool(np.all(np.asanyarray(nb.load(in_mask).dataobj) > 1e-4))


def synthetic(tmp, name, shape, zooms, seed, *, scale=1.0, quantise=None, flips=(1, 1, 1)):
    aff = np.diag([zooms[0] * flips[0], zooms[1] * flips[1], zooms[2] * flips[2], 1.0])
    aff[:3, 3] = -aff[:3, :3] @ (0.5 * (np.array(shape) - 1))
    rng = np.random.default_rng(seed)
    ii, jj, kk = np.meshgrid(*[np.linspace(-1, 1, n) for n in shape], indexing="ij")
    field = (80 * ii * jj - 40 * kk**2 + 25 * ii + 12 + rng.normal(0, 2, shape)) * scale
    if quantise:
        field = np.round(field / quantise) * quantise
    f = Path(tmp) / f"{name}_fmap.nii.gz"
    nb.Nifti1Image(field.astype(np.float32), aff).to_filename(f)
    mask = ((ii**2 + jj**2 + kk**2) < 0.8).astype(np.uint8)
    if not mask.any():
        mask[...] = 1
    m = Path(tmp) / f"{name}_mask.nii.gz"
    nb.Nifti1Image(mask, aff).to_filename(m)
    return f, m, aff


BS = "work/fmriprep_25_2_wf"
ORACLE = [
    ("ds005454-sub-16", "sub_16_wf/fmap_preproc_wf/wf_gre16"),
    ("ds002422-sub-02", "sub_02_wf/fmap_preproc_wf/wf_auto_00000"),
    ("ds002422-sub-02", "sub_02_wf/fmap_preproc_wf/wf_auto_00001"),
    ("ds001600-sub-1", "sub_1_wf/fmap_preproc_wf/wf_auto_00000"),
    ("ds001600-sub-1", "sub_1_wf/fmap_preproc_wf/wf_auto_00001"),
    ("ds001600-sub-1", "sub_1_wf/fmap_preproc_wf/wf_auto_00002"),
    ("ds002603-sub-01", "sub_01_wf/fmap_preproc_wf/wf_auto_00000"),
]


def main() -> int:
    warnings.simplefilter("ignore")
    print(f"numpy {np.__version__}")
    with tempfile.TemporaryDirectory() as tmp:
        f, m, aff = synthetic(tmp, "base", (40, 36, 20), (3.0, 3.0, 3.5), 1)
        case("synthetic-median", f, m)
        case("synthetic-mean", f, m, recenter="mean")
        case("synthetic-none", f, m, recenter=False)
        case("synthetic-no-mask", f, None)
        case("synthetic-no-extrapolation", f, m, extrapolate=False)
        case("synthetic-alpha-1e-2", f, m, ridge_alpha=1e-2)
        case("synthetic-two-levels", f, m, bs_spacing=[(40.0, 40.0, 20.0), (16.0, 16.0, 10.0)])
        q, qm, _ = synthetic(tmp, "quant", (30, 28, 12), (3.0, 3.0, 3.0), 2, quantise=2.5)
        case("synthetic-mode", q, qm, recenter="mode")
        full = Path(tmp) / "full_mask.nii.gz"
        nb.Nifti1Image(np.ones((40, 36, 20), np.uint8), aff).to_filename(full)
        case("synthetic-full-mask", f, full)
        scaled = Path(tmp) / "scaled_mask.nii.gz"
        sm = nb.Nifti1Image((np.asanyarray(nb.load(m).dataobj) * 200).astype(np.int16), aff)
        sm.header.set_slope_inter(1e-6, 0.0)  # stored 200 -> 2e-4 > 1e-4; 0 stays out
        sm.to_filename(scaled)
        case("synthetic-scaled-mask", f, scaled)
        o, om, _ = synthetic(tmp, "odd", (23, 31, 9), (2.7, 3.1, 4.4), 3, flips=(-1, 1, 1))
        case("synthetic-odd-LAS", o, om)
        k, km, _ = synthetic(tmp, "kernel", (3, 3, 2), (2.0, 2.0, 2.0), 4)
        case("synthetic-kernel-system", k, km)
        s, sm_, _ = synthetic(tmp, "slice", (32, 30, 1), (3.0, 3.0, 4.0), 5)
        case("synthetic-single-slice", s, sm_)
        z, zm, _ = synthetic(tmp, "fine", (36, 30, 20), (1.5, 1.5, 2.0), 6)
        case("synthetic-resize-zooms_min-4", z, zm, zooms_min=4.0)
        case("synthetic-resize-no-mask", z, None, zooms_min=(2.0, 2.0, 3.0))
        big, bm, _ = synthetic(tmp, "huge", (20, 18, 10), (3.0, 3.0, 3.0), 7, scale=1e5)
        case("synthetic-failed-fit", big, bm)
    for run, node in ORACLE:
        if QUICK and run == "ds002603-sub-01":
            continue
        base = R / f"{run}-noreconall" / BS / node
        datas = sorted((base / "phdiff_wf" / "compfmap").glob("*_fmap.nii.gz"))
        maskf = base / "magnitude_wf" / "brainextraction_wf" / "masker" / "clipped_mask.nii.gz"
        if not datas or not maskf.exists():
            print(f"skipped (no oracle run here): {run} {node}")
            continue
        case(f"{run}/{node.rsplit('/', 1)[-1]}", datas[0], maskf)
    print("ALL OK" if not FAILS else f"FAILED: {len(FAILS)}: {', '.join(FAILS)}")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    sys.exit(main())
