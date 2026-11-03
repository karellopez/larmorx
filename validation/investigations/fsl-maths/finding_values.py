# SPDX-License-Identifier: Apache-2.0
"""A random-testing finding of fslmaths (``validation/difftest`` night's ``findings/fsl-maths/
*.json``), its shrunk case run by FSL (through the oracle-output cache) and by the replica in
``reproduce`` mode: the voxels whose values differ, with both values and their bits.

    python -P validation/investigations/fsl-maths/finding_values.py <finding.json> [drawn]
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.difftest import core, inputs
from larmorx_validation.parity import fsl_maths_replica as fm


def main(finding: str, which: str = "shrunk") -> None:
    j = json.loads(Path(finding).read_text(encoding="utf-8"))
    r = j["recipe"] if which == "drawn" else j["shrunk"]
    settings = {"FSLOUTPUTTYPE": "NIFTI_GZ"}
    outs = {}
    for who in ("fsl", "replica"):
        d = Path(tempfile.mkdtemp(prefix=f"lx-finding-{who}-"))
        names = {}
        for name, spec in r["inputs"].items():
            p = d / (name + inputs.suffix_of(spec))
            inputs.materialize(spec, p)
            names[name] = p.name
        words = core.substitute(core.arguments(r), lambda n: names[n])
        if who == "fsl":
            p = fm.oracle_run(words, cwd=d, settings=settings)
        else:
            env = dict(os.environ, LARMORX_UPSTREAM_DEFECTS="reproduce", **settings)
            exe = str(fm.replica_binary())
            p = subprocess.run(
                [exe, "fsl", "fslmaths", *words],
                cwd=d,
                env=env,
                capture_output=True,
                text=True,
                check=False,
            )
        print(who, "exit", p.returncode, " ".join(words))
        outs[who] = np.asarray(nib.load(next(d.glob("out*"))).dataobj)
    a, b = outs["fsl"], outs["replica"]
    same = (a == b) | (np.isnan(a) & np.isnan(b))
    diff = np.argwhere(~same)
    print(f"{len(diff)} of {a.size} voxels differ")
    for ix in diff[:10]:
        t = tuple(ix)
        print(t, repr(a[t]), repr(b[t]), a[t].tobytes().hex(), b[t].tobytes().hex())


if __name__ == "__main__":
    main(*sys.argv[1:])
