# SPDX-License-Identifier: Apache-2.0
"""bbre.py <dir>: FSL (oracle cache) and the original on small BBR schedules with an empty
-wmseg: the error and NaN-warning lines of each."""

import sys
from pathlib import Path

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

d = Path(sys.argv[1])
base = "setscale 1 force\n"
scheds = {
    "a": base,
    "b": base + "setoption costfunction bbr\n",
    "c": base + "setoption costfunction bbr\nsetoption bbrstep 200\n",
    "d": base
    + "setoption costfunction bbr\nsetoption bbrstep 200\nclear U\nsetrow UU 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1\nmeasurecost 6 UU:1 0 0 0 0 0 0 0 rel 8\n",
    "e": base
    + "setoption costfunction bbr\nclear U\nsetrow UU 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1\nmeasurecost 6 UU:1 0 0 0 0 0 0 0 rel 8\nmeasurecost 6 UU:1 0 0 0 0 0 0 0 rel 8\n",
}
for k, text in scheds.items():
    (d / f"{k}.sch").write_text(text)
    args = [
        "-in",
        "in.nii.gz",
        "-ref",
        "ref.nii.gz",
        "-omat",
        f"{k}.mat",
        "-cost",
        "bbr",
        "-wmseg",
        "wm.nii.gz",
        "-init",
        "init.mat",
        "-schedule",
        f"{k}.sch",
    ]
    st = {"FSLOUTPUTTYPE": "NIFTI_GZ"}
    f = rep.run_oracle("flirt", args, cwd=d, settings=st)
    o = orig.run_original("flirt", args, cwd=d, settings=st, mode=defects.REPRODUCE)

    def seq(t):
        return "".join(
            "E" if "ERROR" in x else "W" if "NaN" in x else "?" for x in t.splitlines() if x.strip()
        )

    print(k, "FSL", f.code, seq(f.stderr), "| orig", o.code, seq(orig._own(o.stderr)))
