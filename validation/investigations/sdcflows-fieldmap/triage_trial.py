# SPDX-License-Identifier: Apache-2.0
"""Sorts a random-testing trial's differences of ``sdcflows-fieldmap`` by what differs: each
finding's drawn call is run again (sdcflows in this process, larmorx with
``upstream_defects="reproduce"``) and every output compared field by field, so the trial's
differences fall into causes (a header field, the affine, the data).

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python:<repo>/validation/src \\
        python -P triage_trial.py <trial directory> [--per-call 60] [--out triage.json]

Prints one line per cause: its count, the calls and an example case; ``--out`` keeps every
case's cause.
"""

from __future__ import annotations

import argparse
import collections
import json
import sys
import tempfile
import warnings
from pathlib import Path

import nibabel as nb
import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))


def _header_diff(a, b) -> list[str]:
    ha = a.header if hasattr(a, "header") else a
    hb = b.header if hasattr(b, "header") else b
    if not hasattr(hb, "keys"):  # a larmorx header
        hb = nb.Nifti1Header(hb.to_bytes()) if hb.version == 1 else nb.Nifti2Header(hb.to_bytes())
    skip = ("vox_offset", "scl_slope", "scl_inter")
    return [k for k in ha.keys() if k not in skip and ha[k].tobytes() != hb[k].tobytes()]


def _data(img) -> np.ndarray:
    return np.asanyarray(img.dataobj) if hasattr(img, "dataobj") else np.asarray(img.data)


def _compare_images(label: str, a, b) -> list[str]:
    out = []
    da, db = _data(a), _data(b)
    if da.shape != db.shape or da.dtype != db.dtype:
        out.append(f"{label}:shape/dtype")
    elif da.tobytes() != db.tobytes():
        both = (da == db) | (np.isnan(da) & np.isnan(db)) if da.dtype.kind == "f" else da == db
        out.append(
            f"{label}:data({int((~both).sum())} values)"
            if not both.all()
            else f"{label}:data(zero signs or NaN bits)"
        )
    aa, ab = np.asarray(a.affine), np.asarray(b.affine)
    if aa.tobytes() != ab.tobytes():
        out.append(f"{label}:affine(max {np.abs(aa - ab).max():.2g})")
    out += [f"{label}:header.{k}" for k in _header_diff(a, b)]
    return out


def _files(recipe: dict, tmp: Path) -> dict[str, str]:
    from larmorx_validation.difftest import gen

    return {
        name: str(gen.write_nifti(spec, tmp / f"{name}{spec.get('suffix', '.nii.gz')}"))
        for name, spec in recipe["inputs"].items()
    }


def cause(recipe: dict) -> list[str]:
    from larmorx_validation.parity import sdcflows_fieldmap as s

    import larmorx as lx

    e = recipe["extra"]
    call = e["call"]
    with tempfile.TemporaryDirectory() as t, warnings.catch_warnings():
        warnings.simplefilter("ignore")
        tmp = Path(t)
        f = _files(recipe, tmp)
        if call == "bspline_approx":
            from sdcflows.interfaces.bspline import BSplineApprox

            params = dict(e["params"])
            params["bs_spacing"] = [tuple(x) for x in params["bs_spacing"]]
            if isinstance(params["zooms_min"], list):
                params["zooms_min"] = tuple(params["zooms_min"])
            kw = {"in_mask": f["mask"]} if "mask" in f else {}
            try:
                o = BSplineApprox(in_data=f["fmap"], **kw, **params).run(cwd=t).outputs
            except Exception as exc:
                return [f"sdcflows raised {type(exc).__name__}"]
            m = lx.sdcflows.bspline_approx(f["fmap"], f.get("mask"), **params, out_dir=tmp / "m",
                                           upstream_defects="reproduce")  # fmt: skip
            oc = o.out_coeff if isinstance(o.out_coeff, list) else [o.out_coeff]
            pairs = [("field", o.out_field, m.out_field), ("error", o.out_error, m.out_error)]
            pairs += [(f"coeff{i}", a, b) for i, (a, b) in enumerate(zip(oc, m.out_coeff))]
            out = []
            for label, a, b in pairs:
                out += _compare_images(label, nb.load(a), nb.load(b))
            return out
        if call in ("fit", "apply"):
            coeffs = s._coefficient_files(f["fmap"], e["coefficients"], tmp)
            xfm = s._rigid(e.get("xfm"))
            from sdcflows.transform import B0FieldTransform as Sdc

            ref = Sdc(coeffs=[nb.load(c) for c in coeffs])
            mine = lx.sdcflows.B0FieldTransform(coeffs=coeffs)
            if call == "fit":
                try:
                    ref.fit(nb.load(f["moving"]), xfm_data2fmap=xfm, approx=e["approx"])
                except Exception as exc:
                    return [f"sdcflows raised {type(exc).__name__}"]
                mine.fit(f["moving"], xfm, e["approx"], upstream_defects="reproduce")
                return _compare_images("mapped", ref.mapped, mine.mapped)
            kw = dict(e["kw"])
            kw.update(xfm_data2fmap=xfm, approx=e["approx"])
            ro = s._ro_time(e["ro_time"])
            try:
                a = ref.apply(f["moving"], e["pe_dir"], ro, **kw)
            except Exception as exc:
                return [f"sdcflows raised {type(exc).__name__}"]
            b = mine.apply(f["moving"], e["pe_dir"], ro, **kw, upstream_defects="reproduce")
            return _compare_images("apply", a, b) + _compare_images(
                "field", ref.mapped, mine.mapped
            )
        if call == "topup":
            m = s._load("check_coeffs.py")
            coeff = m.topup_coeff(tmp / "c_fieldcoef.nii.gz", f["ref"], tuple(e["factors"]),
                                  e["wrong_shape"])  # fmt: skip
            from sdcflows.interfaces.bspline import TOPUPCoeffReorient

            try:
                o = TOPUPCoeffReorient(in_coeff=[str(coeff)], fmap_ref=f["ref"],
                                       pe_dir=e["pe_dir"]).run(cwd=t).outputs  # fmt: skip
            except Exception as exc:
                return [f"sdcflows raised {type(exc).__name__}"]
            r = lx.sdcflows.topup_coeff_reorient([str(coeff)], f["ref"], e["pe_dir"],
                                                 out_dir=tmp / "m", upstream_defects="reproduce")  # fmt: skip
            oc = o.out_coeff if isinstance(o.out_coeff, list) else [o.out_coeff]
            return _compare_images("coeff", nb.load(oc[0]), nb.load(r.out_coeff[0]))
        if call == "displacements":
            from sdcflows.transform import disp_to_fmap, fmap_to_disp

            epi = f.get("epi", f["fmap"])
            d1 = fmap_to_disp(nb.load(f["fmap"]), e["ro_time"], e["pe_dir"], itk_format=e["itk"])
            d2 = lx.sdcflows.fmap_to_disp(f["fmap"], e["ro_time"], e["pe_dir"], itk_format=e["itk"])
            out = _compare_images("disp", d1, d2)
            with np.errstate(all="ignore"):
                f1 = disp_to_fmap(d1, nb.load(epi), e["ro_time"], e["pe_dir"], itk_format=e["itk"])
                f2 = lx.sdcflows.disp_to_fmap(
                    d2, epi, e["ro_time"], e["pe_dir"], itk_format=e["itk"]
                )
            return out + _compare_images("fmap", f1, f2)
        return ["phase: not sorted here"]


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("trial")
    p.add_argument("--per-call", type=int, default=60)
    p.add_argument("--out", default=None)
    args = p.parse_args()
    found = sorted((Path(args.trial) / "findings" / "sdcflows-fieldmap").glob("*.json"))
    per_call: collections.Counter = collections.Counter()
    causes: collections.Counter = collections.Counter()
    example: dict = {}
    calls: dict = collections.defaultdict(collections.Counter)
    every = []
    for path in found:
        d = json.loads(path.read_text())
        recipe = d["recipe"]
        call = recipe["extra"]["call"]
        if per_call[call] >= args.per_call:
            continue
        per_call[call] += 1
        try:
            c = cause(recipe)
        except Exception as exc:  # the triage's own failure is a cause too
            c = [f"triage error {type(exc).__name__}: {str(exc)[:80]}"]
        key = " + ".join(sorted({x.split("(")[0] for x in c})) or "no difference now"
        causes[key] += 1
        calls[key][call] += 1
        example.setdefault(key, (recipe["n"], c[:6]))
        every.append({"n": recipe["n"], "call": call, "cause": c})
    for key, n in causes.most_common():
        print(f"{n:4d}  {key}  {dict(calls[key])}  e.g. #{example[key][0]} {example[key][1]}")
    if args.out:
        Path(args.out).write_text(json.dumps(every, indent=0))
    return 0


if __name__ == "__main__":
    sys.exit(main())
