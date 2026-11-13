# SPDX-License-Identifier: Apache-2.0
"""tCompCor's high-variance mask of a series left with three volumes (four, the first ignored):
``regress_poly(2, ...)`` fits three points exactly, so the voxels' standard deviations are the
fit's rounding residue and the mask follows the arithmetic of the numpy release (relay 2h-ii of the
confounds' part, 2026-11-12: on the golden registry's 4-volume ``bold()`` series and its BOLD mask,
576 voxels, 12 voxels with nothing ignored in every release, 236 with numpy 2.5.4 and 52 with numpy
2.2.6 when one volume is ignored). Why the golden cases of the workflow ignore no volume.

``PYTHONPATH=<repo>/python python -P probe_tcompcor_three_volumes.py``.
"""

import importlib.util
import sys
import tempfile
from pathlib import Path

import numpy as np

import larmorx as lx

REPO = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location(
    "larmorx_golden_registry", REPO / "tests" / "golden" / "registry.py"
)
assert spec is not None and spec.loader is not None
reg = importlib.util.module_from_spec(spec)
sys.modules["larmorx_golden_registry"] = reg
spec.loader.exec_module(reg)

with tempfile.TemporaryDirectory() as t:
    inputs = reg.Inputs(Path(t))
    series, mask = inputs.file("conf_bold.nii.gz"), inputs.file("conf_bold_mask.nii.gz")
    for mode in ("fix", "reproduce"):
        for nv in ("2.5.4", "2.2.6"):
            for skip in (0, 1):
                out = Path(t) / f"{mode}-{nv}-{skip}"
                out.mkdir()
                r = lx.confounds.tcompcor(
                    series, mask, percentile_threshold=0.02, pre_filter="cosine",
                    variance_threshold=0.5, ignore_initial_volumes=skip, failure_mode="NaN",
                    out_dir=out, numpy_version=nv, upstream_defects=mode,
                )  # fmt: skip
                voxels = int(np.asarray(lx.load(r.high_variance_masks[0]).data).sum())
                print(f"{mode:9s} numpy {nv} skip {skip}: {voxels} voxels")
