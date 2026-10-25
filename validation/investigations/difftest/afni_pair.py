# SPDX-License-Identifier: Apache-2.0
"""An AFNI finding of the differential tests by hand (3dTshift, 3dAutomask, 3dUnifize): its
shrunk inputs rebuilt, AFNI's program, the GPL replica and the clean-room original run in both
D13 modes, each run's exit status and D13 notices, and every header field and output file that
differs from AFNI's (black-box: only the programs' outputs).

    python -P validation/investigations/difftest/afni_pair.py <finding.json> <dir> <program>

``<program>`` is AFNI's name (``3dUnifize``); the outputs are the files the arguments name.
"""

from __future__ import annotations

import difflib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.difftest import core, inputs
from larmorx_validation.parity import afni_tshift as at

FIELDS = (
    "dim",
    "pixdim",
    "qform_code",
    "sform_code",
    "quatern_b",
    "quatern_c",
    "quatern_d",
    "qoffset_x",
    "qoffset_y",
    "qoffset_z",
    "srow_x",
    "srow_y",
    "srow_z",
    "slice_code",
    "slice_start",
    "slice_end",
    "slice_duration",
    "toffset",
    "xyzt_units",
    "dim_info",
    "scl_slope",
    "scl_inter",
    "datatype",
)


def main() -> int:
    f = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    work, program = Path(sys.argv[2]).resolve(), sys.argv[3]
    work.mkdir(parents=True, exist_ok=True)
    recipe = f.get("shrunk") or f["recipe"]
    paths = {}
    for name, spec in recipe["inputs"].items():
        p = work / (name + inputs.suffix_of(spec))
        inputs.materialize(spec, p)
        paths[name] = str(p)
    args = core.substitute(core.arguments(recipe), lambda n: paths[n])
    outputs = [a for a in args if re.search(r"\.nii(\.gz)?$", a) and a not in paths.values()]
    env = {k: v for k, v in os.environ.items() if not k.startswith("AFNI_")}
    env.update(AFNI_DONT_LOGFILE="YES", AFNI_NIFTI_TYPE_WARN="NO", OMP_NUM_THREADS="1")
    runs = {}
    errs: dict[str, str] = {}
    for who in ("AFNI", "replica reproduce", "replica fix", "original reproduce", "original fix"):
        d = work / who.replace(" ", "-")
        d.mkdir(exist_ok=True)
        for x in d.iterdir():
            x.unlink()
        e = {**env, "HOME": str(d)}
        if who == "AFNI":
            cmd = [str(Path(at.afni_binary()).parent / program), *args]
        else:
            impl, mode = who.split()
            e.update(LARMORX_UPSTREAM_DEFECTS=mode, LARMORX_IMPLEMENTATION=impl)
            if impl == "replica":
                e["LARMORX_GPL_BIN"] = str(at.replica_binary())
            cmd = [sys.executable, "-m", "larmorx", "afni", program, *args]
        p = subprocess.run(cmd, cwd=d, env=e, capture_output=True, text=True, check=False)
        notes = re.findall(r"upstream defect ([A-Z]+-\d+) \(", p.stderr)
        runs[who] = d
        (d / "stderr.txt").write_text(p.stderr, encoding="utf-8")
        errs[who] = p.stderr.replace(str(d), "<dir>").replace(str(work), "<work>")
        print(f"{who}: exit {p.returncode}; notices {notes}")
    for out in outputs:
        ref = runs["AFNI"] / out
        if not ref.is_file():
            continue
        h0 = nib.load(ref).header
        d0 = np.asarray(nib.load(ref).dataobj)
        for who, d in runs.items():
            if who == "AFNI" or not (d / out).is_file():
                continue
            img = nib.load(d / out)
            diffs = []
            for field in FIELDS:
                a, b = np.asarray(h0[field]).tolist(), np.asarray(img.header[field]).tolist()
                if json.dumps(a) != json.dumps(b):
                    diffs.append(f"{field} AFNI {a} {who} {b}")
            data = np.asarray(img.dataobj)
            nd = int(np.count_nonzero(data != d0)) if data.shape == d0.shape else -1
            print(f"  {out} {who}: {nd} values differ; " + ("; ".join(diffs) or "header identical"))
    for who, text in errs.items():
        if who == "AFNI":
            continue
        diff = list(
            difflib.unified_diff(
                errs["AFNI"].splitlines(), text.splitlines(), "AFNI", who, n=0, lineterm=""
            )
        )
        print(f"  standard error, AFNI against {who}: " + ("identical" if not diff else ""))
        print("\n".join("    " + ln for ln in diff[2:40]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
