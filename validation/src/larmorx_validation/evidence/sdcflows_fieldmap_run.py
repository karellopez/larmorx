# SPDX-License-Identifier: Apache-2.0
"""One step of sdcflows' field-map estimation, by sdcflows itself or by ``lx.sdcflows``, in the
Python environment that runs this module: the sdcflows-fieldmap evidence runs it under each
kept environment (``parity.python_versions``: the newest numerical releases, larmorx's
reference, and the shared ones, for the originals' own step), as ``resample_series_run`` runs
fMRIPrep's resampler.

Usage::

    python -m larmorx_validation.evidence.sdcflows_fieldmap_run <task> <spec.json> <out_dir> \\
        <implementation> [--threads N] [--defects fix|reproduce]

Tasks (the spec: paths on this machine and the node's parameters):

- ``estimator``: fMRIPrep's phase-difference estimator, step by step on the oracle run's own
  inputs: ``PhaseMap2rads`` of each phase map, ``SubtractPhases``, ``Phasediff2Fieldmap`` of
  fMRIPrep's own PRELUDE output (``compfmap``), and ``BSplineApprox`` of fMRIPrep's own field
  map with its mask and parameters (``bs_filter``). Implementations: ``sdcflows``,
  ``larmorx`` (``lx.sdcflows``' functions), ``larmorx-workflow`` (``lx.sdcflows.phdiff_workflow``
  with ``lx.fsl.prelude``'s clean-room original: the whole chain, PRELUDE included).
- ``apply``: ``ApplyCoeffsField`` (``sdcflows``) or ``lx.sdcflows.apply_coeffs_field``
  (``larmorx``): a series unwarped with coefficient files and an ITK transform.
- ``fit``: ``B0FieldTransform.fit`` on a target with ``xfm_data2fmap`` (``sdcflows``,
  ``larmorx``), or the exact field where the transform puts the target (``exact``: the
  investigation's numpy construction, ``measure_sdc01_02.exact_field``).
- ``refit``: ``BSplineApprox`` with fMRIPrep's parameters on a field-map file (``sdcflows``,
  ``larmorx``), its exception kept when it fails.

``<out_dir>/outputs.json`` names each output file (role -> file name in ``out_dir``) and holds
the warnings, the register entries larmorx's notices name, the exception text of a failed call,
the seconds and the releases.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
import tempfile
import time
import warnings
from pathlib import Path

#: fMRIPrep's ``bs_filter`` parameters in every oracle run (``oracle_nodes.py``).
BS_FILTER = {
    "bs_spacing": [(16.0, 16.0, 10.0)],
    "ridge_alpha": 1e-4,
    "recenter": "median",
    "extrapolate": True,
    "zooms_min": 1.0,
}


def releases() -> dict[str, str]:
    import importlib.metadata as md

    names = ("sdcflows", "scikit-learn", "scipy", "numpy", "nibabel", "nitransforms")
    out = {n: md.version(n) for n in names}
    out["OPENBLAS_CORETYPE"] = os.environ.get("OPENBLAS_CORETYPE", "")
    return out


def _quiet() -> None:
    """nipype's log records off (time stamps); after nipype's import, which sets its loggers."""
    import logging

    for name in ("nipype", "nipype.interface", "nipype.workflow", "nipype.utils"):
        logging.getLogger(name).setLevel(logging.ERROR)


def _put(out: Path, role: str, src, files: dict) -> None:
    """Copy ``src`` into ``out`` under its own name (role prefixed when two roles share one;
    a nipype multi-output of one file is that file)."""
    if isinstance(src, (list, tuple)):
        (src,) = src
    src = Path(src)
    name = src.name
    if any(v == name for v in files.values()):
        name = f"{role}_{name}"
    shutil.copyfile(src, out / name)
    files[role] = name


def _bs(spec: dict) -> dict:
    p = dict(BS_FILTER)
    p.update(spec.get("bs") or {})
    p["bs_spacing"] = [tuple(float(v) for v in s) for s in p["bs_spacing"]]
    return p


def _estimator(spec: dict, out: Path, impl: str, threads: int, defects: str, rec: dict) -> None:
    files: dict[str, str] = {}
    with tempfile.TemporaryDirectory(prefix="lx-sdc-ev-") as t:
        tmp = Path(t)
        if impl == "sdcflows":
            from sdcflows.interfaces.bspline import BSplineApprox
            from sdcflows.interfaces.fmap import Phasediff2Fieldmap, PhaseMap2rads, SubtractPhases

            _quiet()
            rads = []
            for i, ph in enumerate(spec["phases"]):
                d = tmp / f"rads{i}"
                d.mkdir()
                rads.append(PhaseMap2rads(in_file=ph).run(cwd=str(d)).outputs.out_file)
            d = tmp / "diff"
            d.mkdir()
            res = SubtractPhases(in_phases=rads, in_meta=spec["metas"]).run(cwd=str(d)).outputs
            _put(out, "rads", res.phase_diff, files)
            rec["metadata_equal"] = res.metadata == spec["metadata"]
            d = tmp / "fmap"
            d.mkdir()
            f = Phasediff2Fieldmap(in_file=spec["unwrapped"], metadata=spec["metadata"])
            _put(out, "fmap", f.run(cwd=str(d)).outputs.out_file, files)
            d = tmp / "bs"
            d.mkdir()
            o = BSplineApprox(in_data=spec["fmap"], in_mask=spec["mask"], **_bs(spec))
            o = o.run(cwd=str(d)).outputs
            for role, src in (
                ("field", o.out_field),
                ("error", o.out_error),
                ("extrapolated", o.out_extrapolated),
                ("coeff", o.out_coeff[0] if isinstance(o.out_coeff, list) else o.out_coeff),
            ):
                _put(out, role, src, files)
        elif impl == "larmorx":
            from larmorx import sdcflows as lxs

            rads = [
                lxs.au2rads(ph, out_dir=tmp, upstream_defects=defects).out_file
                for ph in spec["phases"]
            ]
            res = lxs.subtract_phases(
                [str(r) for r in rads], spec["metas"], out_dir=tmp, upstream_defects=defects
            )
            _put(out, "rads", res.out_file, files)
            rec["metadata_equal"] = res.metadata == spec["metadata"]
            d = tmp / "fmap"
            d.mkdir()
            f = lxs.phdiff2fmap(spec["unwrapped"], metadata=spec["metadata"], out_dir=d)
            _put(out, "fmap", f.out_file, files)
            d = tmp / "bs"
            d.mkdir()
            o = lxs.bspline_approx(
                spec["fmap"],
                spec["mask"],
                **_bs(spec),
                out_dir=d,
                n_threads=threads,
                upstream_defects=defects,
            )
            for role, src in (
                ("field", o.out_field),
                ("error", o.out_error),
                ("extrapolated", o.out_extrapolated),
                ("coeff", o.out_coeff[0]),
            ):
                _put(out, role, src, files)
            rec["n_inside"], rec["n_iter"] = o.n_inside, o.n_iter
        elif impl == "larmorx-workflow":
            from larmorx import sdcflows as lxs

            d = tmp / "wf"
            r = lxs.phdiff_workflow(
                spec["magnitude"],
                spec["phases"],
                spec["metas"],
                spec["prelude_mask"],
                out_dir=d,
                prelude_implementation="original",
                n_threads=threads,
                upstream_defects=defects,
            )
            _put(out, "rads", r.phase_diff.out_file, files)
            _put(out, "fmap", r.fieldmap.out_file, files)
        else:
            raise SystemExit(f"estimator: unknown implementation {impl!r}")
    rec["files"] = files


def _apply(spec: dict, out: Path, impl: str, threads: int, defects: str, rec: dict) -> None:
    files: dict[str, str] = {}
    with tempfile.TemporaryDirectory(prefix="lx-sdc-ev-") as t:
        if impl == "sdcflows":
            from sdcflows.interfaces.bspline import ApplyCoeffsField

            _quiet()
            node = ApplyCoeffsField(
                in_data=spec["bold"],
                in_coeff=spec["coeffs"],
                fmap2data_xfm=spec["fmap2data_xfm"],
                ro_time=[float(spec["ro_time"])],
                pe_dir=[spec["pe_dir"]],
                num_threads=threads,
            )
            o = node.run(cwd=t).outputs
            _put(out, "corrected", o.out_corrected, files)
            _put(out, "field", o.out_field, files)
        elif impl == "larmorx":
            from larmorx import sdcflows as lxs

            o = lxs.apply_coeffs_field(
                spec["bold"],
                spec["coeffs"],
                [float(spec["ro_time"])],
                [spec["pe_dir"]],
                fmap2data_xfm=spec["fmap2data_xfm"],
                out_dir=t,
                n_threads=threads,
                upstream_defects=defects,
            )
            _put(out, "corrected", o.out_corrected, files)
            _put(out, "field", o.out_field, files)
        else:
            raise SystemExit(f"apply: unknown implementation {impl!r}")
    rec["files"] = files


def _fit(spec: dict, out: Path, impl: str, threads: int, defects: str, rec: dict) -> None:
    import nibabel as nb
    import numpy as np

    xfm = np.asarray(spec["xfm"], dtype=np.float64)
    if impl == "sdcflows":
        from sdcflows.transform import B0FieldTransform

        ref = B0FieldTransform(coeffs=[nb.load(c) for c in spec["coeffs"]])
        ref.fit(nb.load(spec["target"]), xfm_data2fmap=xfm, approx=True)
        ref.mapped.to_filename(out / "field.nii.gz")
    elif impl == "larmorx":
        from larmorx.io import save_nibabel
        from larmorx.sdcflows import B0FieldTransform

        mine = B0FieldTransform(coeffs=list(spec["coeffs"]))
        mine.fit(
            spec["target"], xfm_data2fmap=xfm, approx=True, n_threads=threads,
            upstream_defects=defects,
        )  # fmt: skip
        save_nibabel(mine.mapped, out / "field.nii.gz")
    elif impl == "exact":
        sys.path.insert(0, str(Path(spec["scripts"])))
        from measure_sdc01_02 import exact_field

        target = nb.load(spec["target"])
        ctrl = nb.load(spec["coeffs"][-1])
        projected = nb.Nifti1Image(np.zeros(target.shape[:3], np.float32), xfm @ target.affine)
        field = exact_field(projected, ctrl, np.asanyarray(ctrl.dataobj, dtype=np.float32))
        hdr = target.header.copy()
        hdr.set_data_dtype(np.float32)
        nb.Nifti1Image(field.astype(np.float32), target.affine, hdr).to_filename(
            out / "field.nii.gz"
        )
    else:
        raise SystemExit(f"fit: unknown implementation {impl!r}")
    rec["files"] = {"field": "field.nii.gz"}


def _refit(spec: dict, out: Path, impl: str, threads: int, defects: str, rec: dict) -> None:
    files: dict[str, str] = {}
    with tempfile.TemporaryDirectory(prefix="lx-sdc-ev-") as t:
        try:
            if impl == "sdcflows":
                from sdcflows.interfaces.bspline import BSplineApprox

                _quiet()
                node = BSplineApprox(in_data=spec["fmap"], in_mask=spec["mask"], **_bs(spec))
                o = node.run(cwd=t).outputs
                coeff = o.out_coeff[0] if isinstance(o.out_coeff, list) else o.out_coeff
                pairs = [("extrapolated", o.out_extrapolated), ("coeff", coeff)]
            elif impl == "larmorx":
                from larmorx import sdcflows as lxs

                o = lxs.bspline_approx(
                    spec["fmap"], spec["mask"], **_bs(spec), out_dir=t, n_threads=threads,
                    upstream_defects=defects,
                )  # fmt: skip
                pairs = [("extrapolated", o.out_extrapolated), ("coeff", o.out_coeff[0])]
            else:
                raise SystemExit(f"refit: unknown implementation {impl!r}")
        except Exception as e:  # the failure is the result
            text = str(e).strip().splitlines()
            rec["error"] = f"{type(e).__name__}: {text[-1] if text else ''}"
            rec["files"] = files
            return
        for role, src in pairs:
            _put(out, role, src, files)
    rec["files"] = files


TASKS = {"estimator": _estimator, "apply": _apply, "fit": _fit, "refit": _refit}


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("task", choices=sorted(TASKS))
    p.add_argument("spec")
    p.add_argument("out_dir")
    p.add_argument("implementation")
    p.add_argument("--threads", type=int, default=1)
    p.add_argument("--defects", choices=("fix", "reproduce"), default="reproduce")
    a = p.parse_args()
    spec = json.loads(Path(a.spec).read_text(encoding="utf-8"))
    out = Path(a.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    rec: dict = {
        "task": a.task,
        "implementation": a.implementation,
        "threads": a.threads,
        "defects": a.defects,
        "releases": releases(),
    }
    t0 = time.perf_counter()
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        TASKS[a.task](spec, out, a.implementation, a.threads, a.defects, rec)
    rec["seconds"] = round(time.perf_counter() - t0, 2)
    rec["named"] = sorted({getattr(x.message, "entry", "") for x in w} - {""})
    rec["warnings"] = sorted({str(x.message).splitlines()[0][:300] for x in w})
    (out / "outputs.json").write_text(json.dumps(rec, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
