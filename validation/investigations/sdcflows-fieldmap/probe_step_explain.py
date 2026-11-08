# SPDX-License-Identifier: Apache-2.0
"""Run one sdcflows-fieldmap parity case and describe every object its scripts pass to
stepdigest.oracle, component by component (to explain a step change between environments).
Usage: OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python:<repo>/validation/src \
    python -P probe_step_explain.py <case id> <out.json>, in each kept environment; compare the
two JSON files (relay 8: the permanent case sdc02-single-voxel-no-notice gives an image in
numpy 2.3.5 and SDC-08's AttributeError in numpy 2.5.4)."""

import hashlib
import json
import os
import sys

import numpy as np

if os.environ.get("_STEP_EXPLAIN_GUARD"):
    sys.exit("recursion guard")
os.environ["_STEP_EXPLAIN_GUARD"] = "1"

from larmorx_validation.difftest import permanent
from larmorx_validation.parity import sdcflows_fieldmap as sf
from larmorx_validation.parity.harness import CheckList

case_id, out = sys.argv[1], sys.argv[2]
sf._load("check_weights.py")
import stepdigest  # noqa: E402

LOG = []


def h(b):
    return hashlib.sha256(b).hexdigest()[:16]


def describe(obj, depth=0):
    if depth > 6:
        return "..."
    if obj is None or isinstance(obj, bool | int | float):
        return repr(obj) if not isinstance(obj, float) else obj.hex()
    if isinstance(obj, np.ndarray | np.generic):
        a = np.asarray(obj)
        if a.dtype == object:
            return [describe(x, depth + 1) for x in a.ravel()[:20]]
        d = {"dtype": a.dtype.str, "shape": a.shape, "sha": h(np.ascontiguousarray(a).tobytes())}
        if a.size <= 16 and a.dtype.kind in "fiu":
            d["values"] = [float(x).hex() for x in a.ravel()]
        return d
    if isinstance(obj, bytes):
        return {"bytes": len(obj), "sha": h(obj), "hdr": header_fields(obj)}
    if isinstance(obj, str | os.PathLike):
        s = os.fspath(obj)
        c = stepdigest._file(s)
        if c is None:
            return "S:" + stepdigest._text(s)
        return {
            "file": os.path.basename(s),
            "sha": h(c),
            "hdr": header_fields(c),
            "data_sha": data_sha(s),
        }
    if hasattr(obj, "dataobj") and hasattr(obj, "header"):
        return {
            "image": type(obj).__name__,
            "data": describe(np.asanyarray(obj.dataobj)),
            "affine": describe(np.asarray(obj.affine)),
            "hdr": header_fields(obj.header.binaryblock),
        }
    if isinstance(obj, BaseException):
        return f"E{type(obj).__name__}:{stepdigest._text(str(obj))}"
    if isinstance(obj, dict):
        return {
            str(k): describe(v, depth + 1)
            for k, v in sorted(obj.items(), key=lambda kv: str(kv[0]))
        }
    if isinstance(obj, list | tuple):
        return [describe(x, depth + 1) for x in obj]
    if hasattr(obj, "trait_get"):
        return describe(
            {k: v for k, v in obj.trait_get().items() if not k.startswith("trait_")}, depth + 1
        )
    if hasattr(obj, "message") and hasattr(obj, "category"):
        return f"W{obj.category.__name__}:{stepdigest._text(str(obj.message))}"
    if hasattr(obj, "getMessage"):
        return f"L{obj.levelname}:{stepdigest._text(obj.getMessage())}"
    return "R:" + stepdigest._text(repr(obj))[:200]


def header_fields(b):
    try:
        import nibabel as nb

        if len(b) >= 348 and b[344:348] in (b"n+1\0", b"ni1\0"):
            hd = nb.Nifti1Header.from_fileobj(__import__("io").BytesIO(b[:348]))
            return {
                k: [
                    float(x).hex() if isinstance(x, float) else str(x)
                    for x in np.atleast_1d(hd[k]).tolist()
                ]
                for k in hd.structarr.dtype.names
            }
    except Exception as e:
        return f"err {e}"
    return None


def data_sha(path):
    try:
        import nibabel as nb

        img = nb.load(path)
        return {
            "data": h(np.ascontiguousarray(np.asanyarray(img.dataobj)).tobytes()),
            "affine": [float(x).hex() for x in np.asarray(img.affine).ravel()],
        }
    except Exception:
        return None


_orig = stepdigest.digest


def digest(obj, mask=False):
    if not mask:
        LOG.append(describe(obj))
    return _orig(obj, mask)


stepdigest.digest = digest

for c in permanent.cases("sdcflows-fieldmap", "full") + sf.cases("full"):
    if c.id == case_id or c.id.endswith("/" + case_id):
        checks = CheckList()
        try:
            sf.run_case(c, checks)
        except Exception as e:
            print("raised", type(e).__name__, e)
        for ch in checks.checks:
            print("OK " if ch.passed else "FAIL", ch.name[:120])
        break
else:
    sys.exit(f"no case {case_id}")
with open(out, "w", encoding="utf-8") as f:
    json.dump({"numpy": np.__version__, "log": LOG}, f, indent=1, default=str)
print("wrote", out, len(LOG))
