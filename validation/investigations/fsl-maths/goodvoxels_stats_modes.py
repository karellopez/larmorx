# SPDX-License-Identifier: Apache-2.0
"""goodvoxels' four fslstats calls: the replica in both modes, the defects named."""

import shutil
import tempfile
from pathlib import Path

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_maths_replica as m
from larmorx_validation.parity.fsl_flirt_replica import run_replica

base = m._goodvoxels_chain(m.GOODVOXELS_RUN)
for node, prog, t in m.GOODVOXELS_STEPS:
    if prog != "fslstats":
        continue
    src = t[0][1:-1]
    img = base / src / f"{src}.nii.gz"
    out = {}
    for mode in (defects.REPRODUCE, defects.FIX):
        with tempfile.TemporaryDirectory() as d_:
            d = Path(d_)
            shutil.copy(img, d / img.name)
            out[mode] = run_replica("fslstats", [img.name, *t[1:]], cwd=d, settings={}, mode=mode)
    print(
        node,
        out[defects.REPRODUCE].stdout.strip(),
        "|",
        out[defects.FIX].stdout.strip(),
        defects.notices_in(out[defects.FIX].stderr),
    )
