# SPDX-License-Identifier: Apache-2.0
"""A 3dTshift finding of the differential tests by hand: its shrunk input rebuilt, AFNI's
3dTshift, the GPL replica and the clean-room original run in both D13 modes, and the header
fields that differ between them printed (black-box: only the programs' outputs).

    python -P validation/investigations/difftest/inspect_tshift.py <finding.json> <dir>
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

from larmorx_validation.difftest import core, inputs
from larmorx_validation.parity import afni_tshift as at

FIELDS = ("qform_code", "sform_code", "quatern_b", "quatern_c", "quatern_d", "qoffset_x", "qoffset_y", "qoffset_z", "srow_x", "srow_y", "srow_z", "pixdim", "slice_code", "slice_duration", "toffset")


def main() -> int:
    f = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    work = Path(sys.argv[2])
    work.mkdir(parents=True, exist_ok=True)
    recipe = f.get("shrunk") or f["recipe"]
    spec = recipe["inputs"]["series"]
    src = inputs.materialize(spec, work / ("in" + inputs.suffix_of(spec)))
    args = core.arguments(recipe)
    env = {k: v for k, v in os.environ.items() if not k.startswith("AFNI_")}
    env.update(HOME=str(work), AFNI_DONT_LOGFILE="YES", AFNI_NIFTI_TYPE_WARN="NO")
    outs = {}
    afni = work / "afni.nii"
    subprocess.run([at.afni_binary(), *args, "-prefix", str(afni), str(src)], env=env, capture_output=True, check=False)
    outs["AFNI"] = afni
    for impl in ("replica", "original"):
        for mode in ("reproduce", "fix"):
            out = work / f"{impl}-{mode}.nii"
            e = {**env, "LARMORX_UPSTREAM_DEFECTS": mode, "LARMORX_IMPLEMENTATION": impl}
            if impl == "replica":
                e["LARMORX_GPL_BIN"] = str(at.replica_binary())
            p = subprocess.run([sys.executable, "-m", "larmorx", "afni", "3dTshift", *args, "-prefix", str(out), str(src)], env=e, capture_output=True, text=True, check=False)
            outs[f"{impl} {mode}"] = out
            notes = [ln for ln in p.stderr.splitlines() if "upstream defect" in ln]
            print(f"{impl} {mode}: exit {p.returncode}; " + " | ".join(notes)[:300])
    h = {k: nib.load(v).header for k, v in outs.items() if v.is_file()}
    print("args:", " ".join(args), "| input:", spec["shape"], spec["geometry"])
    for field in FIELDS:
        vals = {k: np.asarray(x[field]).tolist() for k, x in h.items()}
        if len({json.dumps(v) for v in vals.values()}) > 1:
            print(f"  {field}: " + "; ".join(f"{k} {v}" for k, v in vals.items()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
