# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's ``ResampleSeries`` (nipype) on one node's inputs, in the Python environment that
runs this module: the resample-series evidence runs it under each kept environment
(``parity.python_versions``: the newest numerical releases and the shared ones), so the page
shows fMRIPrep's resampler with numpy 2.5.4 and SciPy 1.18.1 beside numpy 2.3.5 and SciPy 1.15.3.

Usage: ``python -m larmorx_validation.evidence.resample_series_run <inputs.json> <out.nii.gz>``
(the JSON: the node's inputs, paths on this machine).
"""

from __future__ import annotations

import json
import shutil
import sys
import tempfile
from pathlib import Path


def main() -> int:
    from fmriprep.interfaces.resampling import ResampleSeries

    inputs = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    out = Path(sys.argv[2])
    with tempfile.TemporaryDirectory(prefix="lx-rs-run-") as tmp:
        node = ResampleSeries(**inputs)
        res = node.run(cwd=tmp)
        shutil.copyfile(res.outputs.out_file, out)
    import numpy
    import scipy

    print(f"numpy {numpy.__version__}, scipy {scipy.__version__}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
