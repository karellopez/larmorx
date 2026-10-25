# SPDX-License-Identifier: Apache-2.0
"""A resample-series finding of the differential tests by hand: its shrunk inputs rebuilt,
fMRIPrep's resampler (the reference) and ``lx.transforms.resample_series`` in ``reproduce`` on
them, and every voxel that differs: its index, both values, their difference in float ulps,
and the input voxel coordinate it was sampled at when the series can be mapped (black-box: the
two programs' outputs).

    python -P validation/investigations/difftest/resample_pair.py <finding.json>
"""

from __future__ import annotations

import json
import sys
import tempfile
import warnings
from pathlib import Path

import numpy as np
from larmorx_validation.difftest import tools
from larmorx_validation.parity import defects
from larmorx_validation.parity import resample_series as rs


def main() -> int:
    import larmorx as lx

    finding = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    recipe = finding.get("shrunk") or finding["recipe"]
    case = tools.get("resample-series").case(recipe, "port")
    s = case.payload[1]
    with tempfile.TemporaryDirectory(prefix="lx-resample-pair-") as name:
        tmp = Path(name)
        src, ref = rs._resolve(s.source, tmp), rs._resolve(s.target, tmp)
        xfms = [rs._resolve(p, tmp) for p in s.transforms]
        fmap = rs._resolve(s.fieldmap, tmp) if s.fieldmap else None
        expected, _ = rs._run_fmriprep(s, src, ref, xfms, fmap, tmp)
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            got = lx.transforms.resample_series(
                src,
                ref,
                xfms,
                upstream_defects=defects.REPRODUCE,
                inverse=list(s.inverse) if len(s.inverse) > 1 else s.inverse[0],
                fieldmap=fmap,
                pe_dir=s.pe_dir,
                ro_time=s.ro_time,
                jacobian=s.jacobian,
                order=s.order,
                mode=s.mode,
                n_threads=1,
            )
        a = np.asarray(expected)
        b = np.asarray(got.data)
        print(f"shapes {a.shape} {b.shape}, dtypes {a.dtype} {b.dtype}")
        diff = np.argwhere(~((a == b) | (np.isnan(a) & np.isnan(b))))
        print(f"{len(diff)} of {a.size} voxels differ")
        for idx in diff[:20]:
            x, y = a[tuple(idx)], b[tuple(idx)]
            ulps = abs(int(np.float32(x).view(np.int32)) - int(np.float32(y).view(np.int32)))
            print(
                f"  {tuple(int(i) for i in idx)}: fMRIPrep {x!r}, larmorx {y!r} ({ulps} float ulps)"
            )
    return 0


if __name__ == "__main__":
    sys.exit(main())
