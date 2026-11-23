# SPDX-License-Identifier: Apache-2.0
"""FS-269 at ``mri_binarize --bb``: the official program against the port in both modes on the
recorded heads' masks.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica; the official program is run as
a black box).

Runs ``validation/probes/fs-binarize/real.py`` (track D's runner: the official program by the
oracle-output cache, the port with ``--upstream-defects reproduce`` and ``fix``, every file
compared) with its calls replaced by five bounding-box calls: ``aseg.mgz --match 2 41`` with
``--bb 0`` and ``--bb 1``, ``brainmask.mgz --min 1`` with ``--bb 0`` and ``--bb 2``, ``aseg.mgz
--match 17 --bb 0``, on 3 heads x 2 streams. Expected: ``reproduce`` identical everywhere; ``fix``
identical with a pad of 1 or more (rule C) and, with a pad of 0, the box one voxel larger on each
axis the mask does not reach the volume's far edge on, noted FS-269 (rule A).

    scripts/orchestration/boxed.sh 16 python -P validation/investigations/freesurfer-fs269-bbox/binarize.py \\
        [--json out.json]
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]

CALLS = [
    ("fs269 wm bb0", "", ["--i", "{m}/aseg.mgz", "--match", "2", "41", "--bb", "0", "--o", "out.mgz"]),
    ("fs269 wm bb1", "", ["--i", "{m}/aseg.mgz", "--match", "2", "41", "--bb", "1", "--o", "out.mgz"]),
    ("fs269 brain bb0", "", ["--i", "{m}/brainmask.mgz", "--min", "1", "--bb", "0", "--o", "out.mgz"]),
    ("fs269 brain bb2", "", ["--i", "{m}/brainmask.mgz", "--min", "1", "--bb", "2", "--o", "out.mgz"]),
    ("fs269 hippo bb0", "", ["--i", "{m}/aseg.mgz", "--match", "17", "--bb", "0", "--o", "out.mgz"]),
]  # fmt: skip


def main(argv: list[str]) -> int:
    path = REPO / "validation" / "probes" / "fs-binarize" / "real.py"
    spec = importlib.util.spec_from_file_location("fs_binarize_real", path)
    mod = importlib.util.module_from_spec(spec)
    sys.path.insert(0, str(path.parent))
    spec.loader.exec_module(mod)
    mod.CALLS = CALLS
    return mod.main(argv)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
