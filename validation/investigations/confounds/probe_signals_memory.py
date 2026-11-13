# SPDX-License-Identifier: Apache-2.0
"""The peak memory of niworkflows' ``SignalExtraction`` and of larmorx's ``signal_extraction`` on
fMRIPrep's real ``signals`` node of one oracle run (by default ds005454: 160 x 160 x 96 x 280
float32, five regions), each in its own process: ``probe_signals_memory.py lx|nw [run]``.

Relay 2g-iv (numpy 2.3.5, the shared environment, boxed): larmorx 2,790 MiB (the stored float32
series and the masks; the voxels read where they are), niworkflows 8,030 MiB (``get_fdata()``'s
float64 copy of the float32 series and each region's ``data[mask, :]`` copy).
"""

import os
import resource
import sys
import tempfile
from pathlib import Path

from nipype.utils.filemanip import loadpkl

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
run = sys.argv[2] if len(sys.argv) > 2 else "ds005454-sub-16-noreconall"
wf = next(RUNS.glob(f"{run}/work/fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_confounds_wf"))
inputs = loadpkl(wf / "signals" / "_inputs.pklz")


def host(v: str) -> str:
    return str(RUNS / run / v.lstrip("/"))


bold, labels = host(inputs["in_file"]), [host(v) for v in inputs["label_files"]]
d = tempfile.mkdtemp()
os.chdir(d)
if sys.argv[1] == "lx":
    from larmorx.niworkflows.images import signal_extraction

    signal_extraction(bold, labels, inputs["class_labels"], out_dir=d, n_threads=1)
else:
    from niworkflows.interfaces.images import SignalExtraction

    SignalExtraction(in_file=bold, label_files=labels, class_labels=inputs["class_labels"]).run()
peak = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024
print(f"{sys.argv[1]} {run}: peak {peak:.0f} MiB")
